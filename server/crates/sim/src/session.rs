//! 세션 상태 — **tick 루프만** 만진다.
//!
//! # 여기 들어오지 않는 것
//!
//! 송신 채널 핸들도 소켓도 들어오지 않는다. 가장 자연스러운 구현이
//! `Session { actor_id, tx: Sender<...> }` 인데, 그 순간 이 크레이트가 tokio 에 묶이고
//! AC-6(a) 의 "런타임과 소켓 없이 도는 수동 step 테스트"가 불가능해진다.
//! 라우팅 표(`session_id -> Sender`)는 게이트웨이 쪽 tick 드라이버가 소유한다.
//!
//! 미응답 명령 수(in-flight)도 여기 없다. 상한 검사는 큐에 **넣기 전에** 일어나야 하므로
//! 게이트웨이 쪽 원자 카운터가 맡는다 (ADR-0006 §5, B-4).

use std::collections::{HashSet, VecDeque};

use starfall_contracts::{SetShipControlPayload, UuidV7};

/// 세션마다 기억하는 최근 `command_id` 수 (ADR-0006 §6).
///
/// # 이것은 지속 멱등성이 **아니다**
///
/// 연결이 끊기면 기억이 사라지고, 이 수를 넘어가면 오래된 것부터 잊는다. 따라서
/// "같은 명령을 재연결 후 다시 보내면 한 번만 적용된다"는 **보장되지 않는다** — 그래서
/// ADR-0005 §5 가 재연결 시 재전송을 금지한다.
///
/// 진짜 멱등성(`processed_commands` 테이블 + 명령·상태 변경 동일 트랜잭션)은 **상태를
/// 바꾸는 첫 명령이 생기는 슬라이스(p1)의 필수 항목**이다. 이번 슬라이스는 상태 변경이 없어
/// 중복이 만들 수 있는 피해가 "PING_REPLY 2개"뿐이라 절반짜리로 둔다.
/// **이것을 진짜 멱등성으로 착각하지 말 것.**
pub const DEDUP_CAPACITY: usize = 1024;

/// 한 tick 동안 이 세션의 입력 판정 진행 상태 — [`SessionState::step_ship_control`] 가
/// 매 `SET_SHIP_CONTROL` 마다 갱신하고, tick 끝의 물리 적분 단계가 읽는다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ShipControlAdmission {
    /// 후보가 아니다(`input_seq` 가 세션의 마지막 적용값보다 크지 않다) — `STALE_INPUT`.
    Stale,
    /// 이 tick의 세션별 처리 한도를 넘었다 — `RATE_LIMITED`.
    RateLimited,
    /// 후보로 접수됐다. 이전에 이 tick에서 이긴 후보가 있었으면 그것은 방금 밀려났다
    /// (`superseded = true`).
    Accepted { superseded_previous: bool },
}

/// 한 세션의 시뮬레이션 상태.
#[derive(Debug)]
pub(crate) struct SessionState {
    /// 검증된 자격 증명에서 서버가 정한 행위자 (I-10).
    pub(crate) actor_id: UuidV7,
    /// 이 세션의 correlation. `SESSION_OPENED`·`SESSION_CLOSED`·`SESSION_READY` 가 공유한다.
    pub(crate) correlation_id: UuidV7,
    /// 최근 `command_id` 의 삽입 순서 (가장 오래된 것이 앞).
    order: VecDeque<UuidV7>,
    /// 같은 집합의 조회용 색인. **순회하지 않으므로** 결정성에 영향이 없다.
    seen: HashSet<UuidV7>,
    /// 서버가 이 세션에 대해 마지막으로 **적용한** `input_seq`. 재개 포함, 새 세션은
    /// 언제나 `None` 에서 시작한다(ADR-0011 §4·§6).
    pub(crate) last_applied_input_seq: Option<u32>,
    /// 이번 tick 동안 제출된 `SET_SHIP_CONTROL` 중 지금 이기고 있는 것 — `(tick, payload)`.
    /// tick이 끝나면 물리 적분이 이 값을 읽고, 다음 tick 시작 시 [`SessionState::last_applied_input_seq`]
    /// 로 확정된다. tick 번호를 함께 들고 있는 이유: 이 세션에 이번 tick 입력이 **없었던**
    /// 경우와 구분하기 위해서다(값을 매 tick 지우지 않고 "이 tick 것이 맞는가"만 본다).
    pub(crate) tick_winner: Option<(u64, SetShipControlPayload)>,
    /// 이번 tick 동안 이 세션에서 몇 건의 `SET_SHIP_CONTROL` 을 후보 판정에 넣었는가
    /// (세션별 tick당 처리 한도 — RATE_LIMITED 판정용). tick마다 리셋된다.
    tick_processed_count: u32,
    tick_processed_at: u64,
    /// 이 세션이 지금 조종하는 함선(있다면) — R4 S-2, architect 1.6-1(b).
    ///
    /// 세션을 닫을 때 자기가 조종하던 함선을 **이 필드로** 찾는다. `Simulation::actor_ship`
    /// (actor → 함선 표)를 거치지 않는 이유: 동시 접속(같은 actor 의 두 번째 `OpenSession`,
    /// I-29 R3 §6.7)이 있으면 그 표는 **다른** 세션이 조종하는 함선을 가리킬 수 있다 —
    /// QA 가 찾은 원인 2가 정확히 이것이었다("표가 다른 함선을 가리키자 전이가 조용히
    /// 건너뛰어졌다"). 이 필드는 그 세션 자신의 기록이라 흔들리지 않는다.
    pub(crate) controlling_ship: Option<UuidV7>,
}

impl SessionState {
    pub(crate) fn new(actor_id: UuidV7, correlation_id: UuidV7) -> Self {
        Self {
            actor_id,
            correlation_id,
            order: VecDeque::with_capacity(64),
            seen: HashSet::with_capacity(64),
            last_applied_input_seq: None,
            tick_winner: None,
            tick_processed_count: 0,
            tick_processed_at: 0,
            controlling_ship: None,
        }
    }

    /// 처음 보는 `command_id` 면 기억하고 `true`, 이미 본 것이면 `false`.
    pub(crate) fn remember(&mut self, command_id: UuidV7) -> bool {
        if !self.seen.insert(command_id) {
            return false;
        }
        self.order.push_back(command_id);
        if self.order.len() > DEDUP_CAPACITY
            && let Some(evicted) = self.order.pop_front()
        {
            self.seen.remove(&evicted);
        }
        true
    }

    /// `SET_SHIP_CONTROL` 한 건을 이번 tick의 후보 판정에 넣는다(ADR-0011 §4).
    ///
    /// 세션 안에서 **도착 순서대로** 불러야 한다 — "가장 마지막에 도착한 것이 이긴다"가
    /// 이 순서에 의존한다. `per_tick_cap` 은 `rate_limit_hz` 에서 유도한 세션당 tick 처리
    /// 상한이다.
    pub(crate) fn step_ship_control(
        &mut self,
        tick: u64,
        payload: SetShipControlPayload,
        per_tick_cap: u32,
    ) -> ShipControlAdmission {
        if self.tick_processed_at != tick {
            self.tick_processed_at = tick;
            self.tick_processed_count = 0;
        }
        self.tick_processed_count += 1;
        if self.tick_processed_count > per_tick_cap {
            return ShipControlAdmission::RateLimited;
        }

        let seq = payload.input_seq.get();
        let is_candidate = self.last_applied_input_seq.is_none_or(|last| seq > last);
        if !is_candidate {
            return ShipControlAdmission::Stale;
        }

        let superseded_previous =
            matches!(&self.tick_winner, Some((winner_tick, _)) if *winner_tick == tick);
        self.tick_winner = Some((tick, payload));
        self.last_applied_input_seq = Some(seq);
        ShipControlAdmission::Accepted {
            superseded_previous,
        }
    }

    /// 이번 tick에 이 세션이 확정한 입력(있다면). 물리 적분 단계가 tick 끝에 부른다.
    pub(crate) fn winning_input_for_tick(&self, tick: u64) -> Option<&SetShipControlPayload> {
        match &self.tick_winner {
            Some((winner_tick, payload)) if *winner_tick == tick => Some(payload),
            _ => None,
        }
    }

    pub(crate) fn snapshot(&self) -> SessionSnapshot {
        SessionSnapshot {
            actor_id: self.actor_id,
            correlation_id: self.correlation_id,
            remembered_commands: self.order.len(),
            last_applied_input_seq: self.last_applied_input_seq,
        }
    }
}

/// 테스트·진단용 읽기 전용 스냅샷.
///
/// 게이트웨이가 세션 상태를 **바꿀** 길은 없다. 읽기만 가능하다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionSnapshot {
    /// 행위자.
    pub actor_id: UuidV7,
    /// 세션 correlation.
    pub correlation_id: UuidV7,
    /// 지금 기억 중인 `command_id` 수.
    pub remembered_commands: usize,
    /// 서버가 이 세션에 대해 마지막으로 적용한 `input_seq`(`WORLD_SNAPSHOT.ack_input_seq` 와
    /// 같은 값 — ADR-0011 §4).
    pub last_applied_input_seq: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 결정적 UUIDv7 생성 (테스트 전용). 버전 니블 7, variant 10.
    fn id(n: u64) -> UuidV7 {
        let text = format!(
            "01a0b1c2-0000-7{:03x}-8{:03x}-{:012x}",
            n & 0xfff,
            (n >> 12) & 0xfff,
            n
        );
        UuidV7::parse(&text).expect("정규 UUIDv7 이어야 한다")
    }

    #[test]
    fn remember_detects_duplicates() {
        let mut state = SessionState::new(id(1), id(2));
        assert!(state.remember(id(10)));
        assert!(!state.remember(id(10)));
        assert!(state.remember(id(11)));
        assert_eq!(state.snapshot().remembered_commands, 2);
    }

    #[test]
    fn remember_evicts_oldest_beyond_capacity() {
        let mut state = SessionState::new(id(1), id(2));
        for n in 0..(DEDUP_CAPACITY as u64 + 1) {
            assert!(state.remember(id(1000 + n)));
        }
        assert_eq!(state.snapshot().remembered_commands, DEDUP_CAPACITY);
        // 가장 오래된 것은 잊혔다 — 이것이 "지속 멱등성이 아니다"의 구체적 모습이다.
        assert!(state.remember(id(1000)));
    }
}

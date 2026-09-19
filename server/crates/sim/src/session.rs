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

use starfall_contracts::UuidV7;

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
}

impl SessionState {
    pub(crate) fn new(actor_id: UuidV7, correlation_id: UuidV7) -> Self {
        Self {
            actor_id,
            correlation_id,
            order: VecDeque::with_capacity(64),
            seen: HashSet::with_capacity(64),
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

    pub(crate) fn snapshot(&self) -> SessionSnapshot {
        SessionSnapshot {
            actor_id: self.actor_id,
            correlation_id: self.correlation_id,
            remembered_commands: self.order.len(),
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

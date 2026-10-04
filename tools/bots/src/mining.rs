//! p1-02 채굴 관측 — 봇이 받은 `INVENTORY_STATE`·`DEPOSIT_FIELD_STATE`·`HISTORICAL_EVENT_NOTICE`
//! 를 **받은 순서 그대로** 쌓는다. 판정은 이 기록 위에서 한다(시나리오가 서버 상태를 추측하지
//! 않는다 — 판정 입력은 봇이 받은 메시지뿐이다, 계약 SC-82).
//!
//! 명령 결과(`COMMAND_RESULT`)는 기존 `Ledger` 가 쌓는다. 두 기록을 잇는 키는 **수신 순번
//! `frame_seq`** 다 — 같은 연결 안에서 단조 증가하므로 "결과 **뒤**의 `INVENTORY_STATE`"
//! (계약 SC-99, 스펙 §5.1a)를 벽시계 없이 판정할 수 있다.

use uuid::Uuid;

use crate::wire::{DepositFieldStateMessage, HistoricalEventNoticeMessage, InventoryStateMessage};

/// 받은 메시지 1건과 그 수신 좌표.
#[derive(Debug, Clone)]
pub struct Observed<T> {
    /// 이 연결에서 받은 텍스트 프레임 순번(0 부터, 모든 타입 공통).
    pub frame_seq: u64,
    /// 봇 단조 시계(µs).
    pub at_us: u64,
    /// 서버 envelope 의 tick.
    pub tick: u64,
    pub msg: T,
}

/// 명령 결과 1건의 수신 좌표 — `frame_seq` 로 채굴 관측과 순서를 맞춘다.
#[derive(Debug, Clone)]
pub struct ObservedResult {
    pub frame_seq: u64,
    pub tick: u64,
    pub command_id: Uuid,
    pub status: String,
    pub reason_code: Option<String>,
}

#[derive(Debug, Default)]
pub struct MiningObs {
    pub inventory: Vec<Observed<InventoryStateMessage>>,
    pub deposits: Vec<Observed<DepositFieldStateMessage>>,
    pub notices: Vec<Observed<HistoricalEventNoticeMessage>>,
    pub results: Vec<ObservedResult>,
    /// 누출 검사(계약 SC-40)용 원문. `None` 이면 저장하지 않는다(부하 실행에서 메모리 절약).
    pub raw_frames: Option<Vec<(u64, String)>>,
    /// 이 연결이 보낸 `MINE_RESOURCE` — (command_id, 보낸 시각 µs, 그때 본 마지막 스냅샷 tick).
    /// 재전송은 같은 id 가 두 번 들어간다(보낸 사실 그대로).
    pub mine_sent: Vec<SentMine>,
    /// 비행 기록 — 채굴 전에 광맥으로 간 경로의 요약.
    pub nav: Option<NavTrace>,
    /// 원문으로 보낸 프레임(주입 케이스) — (command_id, 원문).
    pub raw_sent: Vec<(Uuid, String)>,
    /// 케이스 끝에 연결이 열려 있었는가(클라이언트가 닫기 **전**).
    pub open_at_end: Option<bool>,
    /// `SESSION_READY` 를 받은 프레임 순번(BACKFILL 이 그 **뒤**인지 — 계약 SC-61).
    pub session_ready_frame_seq: Option<u64>,
    /// `WaitSignal` 이 시간 초과로 끝난 수(0 이 아니면 조율이 깨진 실행).
    pub signal_timeouts: u32,
    /// `WaitActorGone` — 그 actor 의 함선을 한 번이라도 봤는가, 사라진 시각·tick.
    pub actor_seen: bool,
    pub actor_gone_at_us: Option<u64>,
    pub actor_gone_tick: Option<u64>,
    /// 마지막 스냅샷의 자기 함선 id(재접속이 **이어받기**였는지 — 같은 함선이면 잔류 창 안 재개).
    pub own_ship_id: Option<Uuid>,
    /// 원격 조종으로 보낸 SET_SHIP_CONTROL (command_id, 보낸 시각 µs).
    pub control_sent: Vec<(Uuid, u64)>,
    /// 스냅샷 수신 시각(µs) — 기록 지연 구간 안에 스냅샷이 흘렀는지(SC-30). 스냅샷당 8 바이트.
    pub snapshot_at_us: Vec<u64>,
}

/// 보낸 `MINE_RESOURCE` 1건.
#[derive(Debug, Clone)]
pub struct SentMine {
    pub command_id: Uuid,
    pub deposit_id: String,
    pub at_us: u64,
    /// 보낼 때까지 받은 텍스트 프레임 수 — 이 값보다 작은 `frame_seq` 는 "보내기 전" 에 받은 것이다
    /// (계약 SC-40 의 "첫 채굴 전" 경계).
    pub frame_seq_at_send: u64,
    /// 보낼 때 알던 마지막 스냅샷 tick(없으면 `None`).
    pub last_snapshot_tick: Option<u64>,
    /// 보낼 때 알던 자기 함선의 광맥 중심 거리(m)·속력(m/s) — 스냅샷 기준, 판정 입력이 아니다.
    pub dist_m: Option<f64>,
    pub speed_mps: Option<f64>,
}

/// 광맥까지의 비행 요약. `arrived_at_us` 가 `None` 이면 제한 시간 안에 도착하지 못했다.
#[derive(Debug, Clone, Default)]
pub struct NavTrace {
    pub deposit_id: String,
    pub start_dist_m: Option<f64>,
    pub arrived_at_us: Option<u64>,
    pub arrive_dist_m: Option<f64>,
    pub arrive_speed_mps: Option<f64>,
    pub control_frames: u64,
    pub max_speed_mps: f64,
}

impl MiningObs {
    pub fn with_raw_capture() -> Self {
        Self {
            raw_frames: Some(Vec::new()),
            ..Self::default()
        }
    }

    pub fn capture_raw(&mut self, frame_seq: u64, text: &str) {
        if let Some(v) = self.raw_frames.as_mut() {
            v.push((frame_seq, text.to_owned()));
        }
    }
}

/// 스펙 §5.1a / 계약 SC-99 — **수락된 `MINE_RESOURCE` 수 == 그 결과 뒤 같은 tick 의
/// `INVENTORY_STATE` 수.** 채굴 명령 id 집합을 받아 짝을 센다.
///
/// 짝짓기: 수락 결과마다 **그 뒤(frame_seq 가 더 큼), 같은 tick** 의 `INVENTORY_STATE` 중
/// 아직 쓰이지 않은 첫 것. 한 인벤토리 메시지를 두 수락에 쓰지 않는다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResponsePairing {
    pub accepted_mine: usize,
    pub paired: usize,
    pub unpaired_command_ids: Vec<Uuid>,
    pub inventory_messages: usize,
}

impl ResponsePairing {
    /// **수락 0 이면 FAIL** — `0 == 0` 은 검사가 아니다(스펙 §5.1a, p1-01 R3 봇 짝 게이트 사례).
    pub fn ok(&self) -> bool {
        self.accepted_mine > 0 && self.paired == self.accepted_mine
    }
}

pub fn pair_mine_responses(obs: &MiningObs, mine_command_ids: &[Uuid]) -> ResponsePairing {
    let mut used = vec![false; obs.inventory.len()];
    let mut accepted = 0usize;
    let mut unpaired = Vec::new();
    for r in &obs.results {
        if r.status != "ACCEPTED" || !mine_command_ids.contains(&r.command_id) {
            continue;
        }
        accepted += 1;
        let hit = obs
            .inventory
            .iter()
            .enumerate()
            .find(|(i, inv)| !used[*i] && inv.frame_seq > r.frame_seq && inv.tick == r.tick);
        match hit {
            Some((i, _)) => used[i] = true,
            None => unpaired.push(r.command_id),
        }
    }
    ResponsePairing {
        accepted_mine: accepted,
        paired: accepted - unpaired.len(),
        unpaired_command_ids: unpaired,
        inventory_messages: obs.inventory.len(),
    }
}

/// 누출 검사(계약 SC-40): 원문 프레임에서 금지 문자열을 찾는다. 판정자가 검출기 생존을
/// 보이려면 **같은 함수로 양성 대조**(첫 채굴 뒤 프레임에서 적중 ≥ 1)를 돌린다.
pub fn leak_hits(frames: &[(u64, String)], forbidden: &[String]) -> Vec<(u64, String)> {
    let mut out = Vec::new();
    for (seq, text) in frames {
        for f in forbidden {
            if text.contains(f.as_str()) {
                out.push((*seq, f.clone()));
            }
        }
    }
    out
}

/// 광맥 상태의 모양 위반 1건 (계약 SC-39/SC-40, 스펙 I-68).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DepositShapeViolation {
    pub frame_seq: u64,
    pub deposit_id: String,
    pub why: String,
}

/// 원문 프레임에서 `DEPOSIT_FIELD_STATE` 를 찾아 항목마다 **"네 필드 키가 모두 있고, 넷 다
/// null 이거나 넷 다 값"** 인지 본다(스펙 I-68). 타입(`Option`)으로 역직렬화하면 "키 없음" 과
/// "null" 이 구분되지 않으므로 원문 `Value` 로 본다. 반환: (검사한 메시지 수, 검사한 항목 수,
/// 미확인 항목 수, 위반 목록) — 분모를 함께 돌려준다.
pub fn deposit_state_shape(
    frames: &[(u64, String)],
) -> (usize, usize, usize, Vec<DepositShapeViolation>) {
    const FIELDS: [&str; 4] = [
        "mineral_id",
        "initial_reserve_kg",
        "remaining_kg",
        "first_extracted_tick",
    ];
    let mut messages = 0usize;
    let mut entries = 0usize;
    let mut unrevealed = 0usize;
    let mut out = Vec::new();
    for (seq, text) in frames {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(text) else {
            continue;
        };
        if v.get("message_type").and_then(|t| t.as_str()) != Some(crate::wire::DEPOSIT_FIELD_STATE)
        {
            continue;
        }
        messages += 1;
        let deposits = v
            .pointer("/payload/deposits")
            .and_then(|d| d.as_array())
            .cloned()
            .unwrap_or_default();
        for d in deposits {
            entries += 1;
            let id = d
                .get("deposit_id")
                .and_then(|x| x.as_str())
                .unwrap_or("?")
                .to_owned();
            let missing: Vec<&str> = FIELDS
                .iter()
                .copied()
                .filter(|f| d.get(*f).is_none())
                .collect();
            if !missing.is_empty() {
                out.push(DepositShapeViolation {
                    frame_seq: *seq,
                    deposit_id: id,
                    why: format!("키 없음: {missing:?}"),
                });
                continue;
            }
            let nulls = FIELDS.iter().filter(|f| d[**f].is_null()).count();
            match nulls {
                4 => unrevealed += 1,
                0 => {}
                n => out.push(DepositShapeViolation {
                    frame_seq: *seq,
                    deposit_id: id,
                    why: format!("넷 중 {n} 개만 null — 반쯤 드러남"),
                }),
            }
        }
    }
    (messages, entries, unrevealed, out)
}

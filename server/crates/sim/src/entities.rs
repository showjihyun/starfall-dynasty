//! 함선 엔티티 — tick 루프가 소유하는 월드 상태의 단위.
//!
//! `crate::world` 의 순수 물리(`ShipPhysicsState`)에 존재 구간·소유·입력 이월 부기를
//! 더한다. **상태는 `Simulation::step` 안에서만 바뀐다**(I-13) — 이 타입 자체는 규칙을
//! 모르고, 규칙은 `simulation.rs` 가 적용한다.

use starfall_contracts::{DataId, SetShipControlPayload, ShipPresence, UuidV7};

use crate::world::ShipPhysicsState;

/// 함선 한 척.
#[derive(Debug, Clone)]
pub(crate) struct ShipEntity {
    /// 엔티티 식별자. 잔류·재개에도 그대로다(캐릭터 ≠ 함선, GDD §4).
    pub(crate) ship_id: UuidV7,
    /// 이 함선이 속한 행위자. `Simulation::actor_ship` 의 역방향 조회 키와 같은 값이다.
    pub(crate) actor_id: UuidV7,
    /// `SHIP_CLASS` 테이블의 행.
    pub(crate) ship_class_id: DataId,
    /// `ACTIVE` 또는 `LINGERING`.
    pub(crate) presence: ShipPresence,
    /// 물리 상태.
    pub(crate) physics: ShipPhysicsState,
    /// 지금 이 함선을 조종 중인 세션. `presence == ACTIVE` 일 때만 `Some` 이다.
    pub(crate) controlling_session: Option<UuidV7>,
    /// 이 함선을 마지막으로 몰았던 세션(`SHIP_DESPAWNED.last_session_id`).
    pub(crate) last_session_id: UuidV7,
    /// 마지막 "진짜"(이월이나 휴면이 아닌) 입력 — 이월의 원본이다. 재개 시 버려진다
    /// (`None`, ADR-0011 §6.2 "이월 입력은 버린다").
    pub(crate) last_real_input: Option<SetShipControlPayload>,
    /// 마지막 진짜 입력 이후 지난 tick 수. `carry_forward_max_ticks` 와 비교해 이월 만료를
    /// 판정한다.
    pub(crate) ticks_since_real_input: u32,
    /// 잔류가 시작된 tick(디스폰 만료 판정용). `presence == LINGERING` 일 때만 `Some`.
    pub(crate) linger_started_tick: Option<u64>,
    /// 이 잔류를 시작시킨 `SESSION_CLOSED.event_id` — 나중에 디스폰할 때
    /// `SHIP_DESPAWNED.causation_id` 로 쓴다(ADR-0011 §6, 여러 tick 전일 수 있다).
    pub(crate) linger_cause_event_id: Option<UuidV7>,
    /// 위와 같은 시점의 `SESSION_CLOSED.correlation_id` — `SHIP_DESPAWNED.correlation_id` 로
    /// 쓴다(그 세션이 잔류를 시작시켰다는 사실은 상관관계로도 남는다).
    pub(crate) linger_cause_correlation_id: Option<UuidV7>,
}

impl ShipEntity {
    /// 세션이 열릴 때(스폰이든 재개든) 함선을 "지금 조종되는 중"으로 만든다.
    pub(crate) fn activate(&mut self, session_id: UuidV7) {
        self.presence = ShipPresence::Active;
        self.controlling_session = Some(session_id);
        self.last_session_id = session_id;
        self.linger_started_tick = None;
        self.linger_cause_event_id = None;
        self.linger_cause_correlation_id = None;
    }

    /// 재개(resume) 전용 — 월드 상태(물리)는 그대로 두고 입력 이월만 버린다
    /// (ADR-0011 §6.2). `activate` 와 분리한 이유: 최초 스폰은 이 초기화가 필요 없다
    /// (애초에 입력 이력이 없다).
    pub(crate) fn discard_carried_input_for_resume(&mut self) {
        self.last_real_input = None;
        self.ticks_since_real_input = 0;
    }

    /// 세션이 닫힐 때 함선을 잔류로 돌린다. 입력 이월 부기는 **건드리지 않는다** — 이월
    /// 창이 세션 경계를 가로질러 이어지는 것이 설계다(architect: "이월 창이 먼저, 그 뒤
    /// 휴면 입력 — 두 구간은 서로 다른 입력을 쓴다").
    pub(crate) fn start_lingering(
        &mut self,
        tick: u64,
        cause_event_id: UuidV7,
        cause_correlation_id: UuidV7,
    ) {
        self.presence = ShipPresence::Lingering;
        self.controlling_session = None;
        self.linger_started_tick = Some(tick);
        self.linger_cause_event_id = Some(cause_event_id);
        self.linger_cause_correlation_id = Some(cause_correlation_id);
    }
}

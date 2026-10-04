//! 코어에 들어오는 도메인 이벤트.
//!
//! S1 이 만든 계약 타입(`starfall_contracts::events::MineralMinedEvent`)을 그대로 감싼다.
//! 이 규칙(`mineral-discovery@1`)이 판정하는 타입은 `MINERAL_MINED` 하나뿐이다(ADR-0014
//! §2) — 나머지는 원칙 4(모든 이벤트가 역사는 아니다)에 따라 [`DomainEventKind::Unjudged`]
//! 로 들어와 정렬·워터마크에만 참여하고 기록을 만들지 않는다(SC-49, Level 0 필터).
//! `starfall-contracts` 에는 `SESSION_*`·`SHIP_*` 등을 하나로 묶는 합 타입이 없으므로(계약은
//! peek 기반 디스패치를 쓴다, `dispatch.rs`), 러너(H2)가 좌표만 뽑아 `unjudged()` 로 넘긴다.

use starfall_contracts::events::MineralMinedEvent;
use starfall_contracts::primitives::{Sequence, Tick, UuidV7};

/// 판정 대상 도메인 이벤트. 좌표(`event_id`·`world_id`·`tick`·`sequence`)는 원본 이벤트에서
/// 그대로 옮긴다 — [`DomainEventKind::Unjudged`] 에는 구체 타입이 없어 따로 들고 있어야
/// 정렬·역행 검사(SC-48)를 모든 타입에 똑같이 적용할 수 있다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainEvent {
    pub event_id: UuidV7,
    pub world_id: UuidV7,
    pub tick: Tick,
    pub sequence: Sequence,
    pub kind: DomainEventKind,
}

/// 이벤트 종류. `MineralMined` 만 `Box` 로 감싼다 — `MineralMinedEvent` 가 `Unjudged`(0
/// 바이트)보다 훨씬 커서(clippy `large_enum_variant`), 박싱하지 않으면 판정 대상이 아닌
/// 이벤트까지 그 크기를 물게 된다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DomainEventKind {
    MineralMined(Box<MineralMinedEvent>),
    /// `SESSION_*`·`SHIP_*` 등 이 규칙이 판정하지 않는 타입.
    Unjudged,
}

impl DomainEvent {
    /// `MINERAL_MINED` 이벤트에서 좌표를 그대로 옮겨 코어 입력을 만든다.
    #[must_use]
    pub fn mineral_mined(event: MineralMinedEvent) -> Self {
        Self {
            event_id: event.event_id,
            world_id: event.world_id,
            tick: event.tick,
            sequence: event.sequence,
            kind: DomainEventKind::MineralMined(Box::new(event)),
        }
    }

    /// 이 규칙이 판정하지 않는 타입 — 좌표만 있으면 정렬·워터마크 검사에 참여할 수 있다.
    #[must_use]
    pub fn unjudged(event_id: UuidV7, world_id: UuidV7, tick: Tick, sequence: Sequence) -> Self {
        Self {
            event_id,
            world_id,
            tick,
            sequence,
            kind: DomainEventKind::Unjudged,
        }
    }
}

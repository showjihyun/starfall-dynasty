//! 코어가 만드는 `MINERAL_DISCOVERED` 초안과 그 증거.
//!
//! `MineralDiscoveredDraft` 는 계약 `historical::MineralDiscoveredEvent` 에서 `recorded_at`
//! 만 뺀 모양이다 — ADR-0014 §1: "결과에는 실제 시각이 없다. `recorded_at` 은 러너가
//! 붙인다"(코어는 시계를 읽지 않는다). 러너(H2)가 커밋 시각을 붙여 [`Self::into_event`] 로
//! 와이어 타입을 완성한다.
//!
//! `EvidenceDraft` 는 계약에 대응 와이어 타입이 없다 — evidence 는 DB 내부고
//! `authenticity_status` 등은 와이어에 싣지 않는다(ADR-0014 §3). H2 가 DB 행으로 옮긴다.

use starfall_contracts::data::EvidenceType;
use starfall_contracts::historical::{
    FactStatus, HistoricalLocation, HistoricalParticipant, HistoricalVisibility,
    MineralDiscoveredEvent, MineralDiscoveredPayload, MineralDiscoveredType,
};
use starfall_contracts::primitives::{
    ConstSchemaVersion, GameTime, RealTime, RuleVersion, Tick, UuidV5, UuidV7,
};

/// `MINERAL_DISCOVERED` 초안 — 계약 타입에서 `recorded_at` 만 뺐다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MineralDiscoveredDraft {
    pub historical_event_id: UuidV5,
    pub world_id: UuidV7,
    /// `"{star_system_id}/{mineral_id}"` — `historical_event_id` 파생에 쓰인 것과 같은 값
    /// (H1). H2 가 `UNIQUE (world_id, event_type, dedupe_key)` 에 그대로 바인딩한다 —
    /// 여기 없으면 러너가 같은 공식을 두 번째로 손으로 다시 계산해야 해서 드리프트 위험이
    /// 생긴다.
    pub dedupe_key: String,
    pub rule_version: RuleVersion,
    pub importance_level: u8,
    pub tick: Tick,
    pub occurred_at: GameTime,
    pub visibility: HistoricalVisibility,
    /// 정확히 1개 — 이긴 `MINERAL_MINED` 의 `event_id`.
    pub source_event_ids: Vec<UuidV7>,
    pub location: HistoricalLocation,
    /// 정확히 2개, `[DISCOVERER, VESSEL]` 순서 고정(jsonb 배열 동등은 순서를 본다 —
    /// SC-41·SC-106).
    pub participants: Vec<HistoricalParticipant>,
    pub payload: MineralDiscoveredPayload,
}

impl MineralDiscoveredDraft {
    /// 러너가 실제 커밋 시각을 붙여 와이어 타입을 완성한다.
    #[must_use]
    pub fn into_event(self, recorded_at: RealTime) -> MineralDiscoveredEvent {
        MineralDiscoveredEvent {
            historical_event_id: self.historical_event_id,
            event_type: MineralDiscoveredType::MineralDiscovered,
            schema_version: ConstSchemaVersion,
            world_id: self.world_id,
            rule_version: self.rule_version,
            importance_level: self.importance_level,
            tick: self.tick,
            occurred_at: self.occurred_at,
            recorded_at,
            visibility: self.visibility,
            fact_status: FactStatus::Confirmed,
            source_event_ids: self.source_event_ids,
            location: self.location,
            participants: self.participants,
            payload: self.payload,
        }
    }
}

/// 발견 1건마다 시스템이 자동으로 만드는 증거(HSE §90). `authenticity_status`·
/// `creation_method`·`derived_from_evidence_ids` 는 고정값(H2 가 DB 스키마의 기본값/상수로
/// 채운다) — 코어는 규칙이 실제로 고르는 값만 낸다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceDraft {
    pub evidence_id: UuidV5,
    pub evidence_type: EvidenceType,
    pub visibility: HistoricalVisibility,
    /// 원본 증거가 가리키는 엔티티. 이 규칙에서는 `ship_id`(designer 결정,
    /// `01_history_review.md` §9.4 — 함선은 영속하지 않으므로 `PARTICIPANTS_ONLY` 열람
    /// 판정은 PLAYER 참가자 기준이지만 `source_entity_id` 자체는 유지한다).
    pub source_entity_id: UuidV7,
}

/// 코어 판정 결과 한 건 — 역사 기록 초안 + 그 증거.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MineralDiscoveredRecord {
    pub event: MineralDiscoveredDraft,
    pub evidence: EvidenceDraft,
}

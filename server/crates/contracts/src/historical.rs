//! 역사 이벤트(Historical Event) 타입 — **도메인 이벤트가 아니다**.
//!
//! 역사 이벤트는 중요도 판정을 통과한 기록이다(절대 원칙 4, ADR-0014). `sequence`·
//! `correlation_id`·`causation_id` 가 없다 — 순서와 원인은 `source_event_ids` 가 대신한다.
//! 자유 서술 필드도 없다 — 헤드라인·문장은 표현 계층이 `event_type` + `payload` 로 만든다
//! (절대 원칙 2·6).

use serde::{Deserialize, Deserializer, Serialize};

use crate::primitives::{
    ConstSchemaVersion, DataId, GameTime, MassKg, RealTime, RuleVersion, Tick, UuidV5, UuidV7,
    de_positive_mass_kg,
};

/// `1 ..= 5`(HSE §11: 1 Local ~ 5 Epoch-defining). Level 0(Ephemeral)은 "역사가 아니다"라는
/// 뜻이라 저장·전송될 수 없다 — 스키마 범위 자체가 그것을 표현한다.
fn de_importance_level<'de, D>(deserializer: D) -> Result<u8, D::Error>
where
    D: Deserializer<'de>,
{
    let raw = u8::deserialize(deserializer)?;
    if (1..=5).contains(&raw) {
        Ok(raw)
    } else {
        Err(serde::de::Error::custom(format!(
            "importance_level 은 1 ..= 5 여야 한다 (받음: {raw})"
        )))
    }
}

/// `source_event_ids`: 정확히 1개(스키마 `minItems: 1, maxItems: 1` — MINERAL_DISCOVERED
/// 는 판정 근거 도메인 이벤트가 언제나 하나다).
fn de_exactly_one_source_event<'de, D>(deserializer: D) -> Result<Vec<UuidV7>, D::Error>
where
    D: Deserializer<'de>,
{
    let raw = Vec::<UuidV7>::deserialize(deserializer)?;
    if raw.len() == 1 {
        Ok(raw)
    } else {
        Err(serde::de::Error::custom(format!(
            "source_event_ids 는 정확히 1개여야 한다 (받음: {})",
            raw.len()
        )))
    }
}

/// `participants`: 정확히 2개(스키마 `minItems: 2, maxItems: 2` —
/// `{actor, PLAYER, DISCOVERER}` 다음 `{ship, SHIP, VESSEL}`).
fn de_exactly_two_participants<'de, D>(
    deserializer: D,
) -> Result<Vec<HistoricalParticipant>, D::Error>
where
    D: Deserializer<'de>,
{
    let raw = Vec::<HistoricalParticipant>::deserialize(deserializer)?;
    if raw.len() == 2 {
        Ok(raw)
    } else {
        Err(serde::de::Error::custom(format!(
            "participants 는 정확히 2개여야 한다 (받음: {})",
            raw.len()
        )))
    }
}

/// 이 기록을 누가 알 수 있는가(historical-engine 스킬 §5). p1-02 는 이벤트에 `PUBLIC` 만
/// 만든다. 닫힌 집합이지만 늘어날 수 있다 — 모르는 값은 "표시 불가"로 취급하고 계속
/// 동작해야 한다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum HistoricalVisibility {
    /// 누구나 볼 수 있다.
    Public,
    /// 소속 진영만.
    FactionOnly,
    /// 관계자만.
    ParticipantsOnly,
    /// 분류.
    Classified,
    /// 기밀.
    Secret,
    /// 조건부 발견 가능.
    Discoverable,
}

/// 사실 자체의 상태(HSE §13) — 해석의 상태가 아니다. p1-02 는 `CONFIRMED` 만 만든다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FactStatus {
    /// 서버가 직접 관측했다.
    Confirmed,
}

/// 역사 이벤트의 위치.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HistoricalLocation {
    /// 사건이 일어난 성계.
    pub star_system_id: DataId,
}

/// 참가자 종류. 닫힌 집합이지만 늘어날 수 있다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum HistoricalEntityKind {
    /// 행위자(캐릭터).
    Player,
    /// 함선 엔티티.
    Ship,
}

/// 참가자 역할. 닫힌 집합이지만 늘어날 수 있다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum HistoricalRole {
    /// 발견을 이룬 PLAYER.
    Discoverer,
    /// 그 발견에 쓰인 SHIP.
    Vessel,
}

/// 역사 이벤트의 참가자 한 명.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HistoricalParticipant {
    /// 참가자의 엔티티 id.
    pub entity_id: UuidV7,
    /// PLAYER 또는 SHIP.
    pub entity_kind: HistoricalEntityKind,
    /// 이 사건에서의 역할.
    pub role: HistoricalRole,
}

// ---------------------------------------------------------------------------
// MINERAL_DISCOVERED
// ---------------------------------------------------------------------------

/// `MINERAL_DISCOVERED` 의 타입 상수.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum MineralDiscoveredType {
    /// 유일한 값.
    #[default]
    #[serde(rename = "MINERAL_DISCOVERED")]
    MineralDiscovered,
}

/// `MINERAL_DISCOVERED` payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MineralDiscoveredPayload {
    /// 발견된 `MINERAL` 행. `location.star_system_id` 와 함께 발견의 정체성이다
    /// (dedupe 키 `"{star_system_id}/{mineral_id}"`).
    pub mineral_id: DataId,
    /// 발견을 일으킨 채굴이 일어난 `DEPOSIT` 행.
    pub deposit_id: DataId,
    /// 발견 채굴의 질량(원본 `MINERAL_MINED` 에서 복사). 절대 0이 아니다.
    #[serde(deserialize_with = "de_positive_mass_kg")]
    pub quantity_kg: MassKg,
}

/// `MINERAL_DISCOVERED` — Level 2 Regional 역사 이벤트(HSE §11, MVP 10종의 첫째).
///
/// 규칙 `mineral-discovery@1`: 같은 `(world_id, star_system_id, mineral_id)` 로 커밋된
/// `MINERAL_MINED` 전체 중 `(tick, sequence)` 가 가장 작은 **단 하나만** 이 기록을 만든다.
/// 같은 tick 이면 더 작은 sequence(게이트웨이 제출 순서)가 유일한 발견자다 — 공동 발견은
/// 없다. `(world, star_system, mineral)` 당 최대 1건, 판정 규칙 상태와 DB unique 키
/// 둘 다로 강제한다(ADR-0014 §4).
///
/// 대응 스키마: `contracts/events/historical/MINERAL_DISCOVERED.schema.json`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MineralDiscoveredEvent {
    /// `UUIDv5(NS_HISTORY, "{world_id}|{event_type}|{dedupe_key}")` — 의미에서 재현 가능한
    /// id(ADR-0014 §1). 재구축이 같은 id를 낸다.
    pub historical_event_id: UuidV5,
    /// 언제나 `MINERAL_DISCOVERED`.
    pub event_type: MineralDiscoveredType,
    /// 언제나 1.
    pub schema_version: ConstSchemaVersion<1>,
    /// 월드(샤드) id.
    pub world_id: UuidV7,
    /// 이 기록을 만든 판정 규칙 버전. 규칙이 바뀌어도 과거 기록은 다시 계산하지 않는다
    /// (절대 원칙 5·9).
    pub rule_version: RuleVersion,
    /// HSE §11: 1 Local ~ 5 Epoch-defining. 0(Ephemeral)은 "역사가 아니다"이므로 저장·
    /// 전송될 수 없다.
    #[serde(deserialize_with = "de_importance_level")]
    pub importance_level: u8,
    /// 결정적 소스 도메인 이벤트의 tick.
    pub tick: Tick,
    /// 결정적 소스 도메인 이벤트의 게임 시간.
    pub occurred_at: GameTime,
    /// 이 기록을 커밋한 실제 시각. 감사 전용 — 결정성 비교에서 제외.
    pub recorded_at: RealTime,
    /// 누가 이 기록의 존재를 알 수 있는가.
    pub visibility: HistoricalVisibility,
    /// 사실의 상태. p1-02는 `CONFIRMED` 만 만든다.
    pub fact_status: FactStatus,
    /// 판정 근거가 된 커밋된 도메인 이벤트. 정확히 1건 — 발견을 이룬 `MINERAL_MINED`.
    #[serde(deserialize_with = "de_exactly_one_source_event")]
    pub source_event_ids: Vec<UuidV7>,
    /// 사건 위치.
    pub location: HistoricalLocation,
    /// 정확히 둘: `{actor, PLAYER, DISCOVERER}` 다음 `{ship, SHIP, VESSEL}`.
    #[serde(deserialize_with = "de_exactly_two_participants")]
    pub participants: Vec<HistoricalParticipant>,
    /// 타입별 payload.
    pub payload: MineralDiscoveredPayload,
}

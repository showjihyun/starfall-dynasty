//! 레지스트리 이름 → Rust 타입 대응표.
//!
//! `contracts/registry/types.json` 에 `server` 태그(producers 또는 consumers)로 올라간 타입은
//! 전부 이 표에 있어야 한다. 계약 테스트가 그것을 강제한다 (ADR-0002 §3 테스트 5).
//!
//! 새 타입을 추가하는 순서: architect 가 스키마·레지스트리·fixture 를 만든다 →
//! 여기 [`CONTRACT_TYPES`] 에 한 줄 추가 → 테스트가 나머지(왕복·반례·변이)를 자동으로 덮는다.
//!
//! # 타입 이름은 리터럴 상수여야 한다
//!
//! QA 계약 커버리지 스크립트는 **코드에서 타입 이름 문자열을 찾는다.** `concat!` 이나
//! 매크로 조합으로 만들면 검색에 걸리지 않아 "코드에 구현이 없다"로 잘못 보고된다.

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::commands::{MineResourceCommand, PingServerCommand, SetShipControlCommand};
use crate::data::{
    DepositFieldTable, MineralTable, MiningRulesTable, ShipClassTable, SignificanceRuleTable,
    StarSystemTable, SyncTuningTable,
};
use crate::events::{
    MineralMinedEvent, SessionClosedEvent, SessionOpenedEvent, ShipDespawnedEvent, ShipSpawnedEvent,
};
use crate::historical::MineralDiscoveredEvent;
use crate::messages::{
    CommandResultMessage, DepositFieldStateMessage, HistoricalEventNoticeMessage,
    InventoryStateMessage, PingReplyMessage, SessionReadyMessage, WorldSnapshotMessage,
};

/// `PING_SERVER` 레지스트리 이름.
pub const PING_SERVER: &str = "PING_SERVER";
/// `PING_REPLY` 레지스트리 이름.
pub const PING_REPLY: &str = "PING_REPLY";
/// `COMMAND_RESULT` 레지스트리 이름.
pub const COMMAND_RESULT: &str = "COMMAND_RESULT";
/// `SESSION_READY` 레지스트리 이름.
pub const SESSION_READY: &str = "SESSION_READY";
/// `SESSION_OPENED` 레지스트리 이름.
pub const SESSION_OPENED: &str = "SESSION_OPENED";
/// `SESSION_CLOSED` 레지스트리 이름.
pub const SESSION_CLOSED: &str = "SESSION_CLOSED";
/// `SET_SHIP_CONTROL` 레지스트리 이름.
pub const SET_SHIP_CONTROL: &str = "SET_SHIP_CONTROL";
/// `WORLD_SNAPSHOT` 레지스트리 이름.
pub const WORLD_SNAPSHOT: &str = "WORLD_SNAPSHOT";
/// `SHIP_SPAWNED` 레지스트리 이름.
pub const SHIP_SPAWNED: &str = "SHIP_SPAWNED";
/// `SHIP_DESPAWNED` 레지스트리 이름.
pub const SHIP_DESPAWNED: &str = "SHIP_DESPAWNED";
/// `SHIP_CLASS` 레지스트리 이름.
pub const SHIP_CLASS: &str = "SHIP_CLASS";
/// `STAR_SYSTEM` 레지스트리 이름.
pub const STAR_SYSTEM: &str = "STAR_SYSTEM";
/// `SYNC_TUNING` 레지스트리 이름.
pub const SYNC_TUNING: &str = "SYNC_TUNING";
/// `MINE_RESOURCE` 레지스트리 이름.
pub const MINE_RESOURCE: &str = "MINE_RESOURCE";
/// `MINERAL_MINED` 레지스트리 이름.
pub const MINERAL_MINED: &str = "MINERAL_MINED";
/// `MINERAL_DISCOVERED` 레지스트리 이름.
pub const MINERAL_DISCOVERED: &str = "MINERAL_DISCOVERED";
/// `DEPOSIT_FIELD_STATE` 레지스트리 이름.
pub const DEPOSIT_FIELD_STATE: &str = "DEPOSIT_FIELD_STATE";
/// `INVENTORY_STATE` 레지스트리 이름.
pub const INVENTORY_STATE: &str = "INVENTORY_STATE";
/// `HISTORICAL_EVENT_NOTICE` 레지스트리 이름.
pub const HISTORICAL_EVENT_NOTICE: &str = "HISTORICAL_EVENT_NOTICE";
/// `MINERAL` 레지스트리 이름.
pub const MINERAL: &str = "MINERAL";
/// `DEPOSIT_FIELD` 레지스트리 이름.
pub const DEPOSIT_FIELD: &str = "DEPOSIT_FIELD";
/// `MINING_RULES` 레지스트리 이름.
pub const MINING_RULES: &str = "MINING_RULES";
/// `SIGNIFICANCE_RULE` 레지스트리 이름.
pub const SIGNIFICANCE_RULE: &str = "SIGNIFICANCE_RULE";

/// JSON 값을 해당 Rust 타입으로 역직렬화한 뒤 다시 직렬화하는 함수.
///
/// 계약 테스트 네 종류가 이 함수 하나를 공유한다.
///
/// | 테스트 | 기대 |
/// |--------|------|
/// | 유효 fixture 왕복 | `Ok(원본과 같은 Value)` |
/// | 반례 serde 거부 | `Err` |
/// | `required` 변이 | `Err` |
/// | 정수 상한 위반 | `Err` |
///
/// 한 함수로 묶어 두면 새 타입을 표에 넣는 순간 네 테스트가 모두 그 타입을 덮는다.
pub type RoundTripFn = fn(&Value) -> Result<Value, String>;

/// 레지스트리 한 항목에 대응하는 Rust 쪽 정보.
#[derive(Debug, Clone, Copy)]
pub struct ContractType {
    /// 레지스트리 `name`.
    pub name: &'static str,
    /// 레지스트리 `kind`.
    pub kind: &'static str,
    /// `contracts/` 기준 스키마 경로.
    pub schema_path: &'static str,
    /// 레지스트리 `schema_version`.
    pub schema_version: u32,
    /// 대응하는 Rust 타입의 경로(문서·리포트용).
    pub rust_type: &'static str,
    /// 역직렬화 → 재직렬화.
    pub round_trip: RoundTripFn,
    /// 명령 타입의 기대 응답, 송신 순서대로 `(레지스트리 이름, "always"|"accepted")`
    /// (스펙 §5.1a). 명령이 아닌 kind 는 빈 슬라이스 — "선택 필드라 없음"으로 조용히
    /// 통과하지 않는다, SC-38 이 명령 kind 수와 이 필드가 채워진 수를 둘 다 찍어 대조한다.
    pub responses: &'static [(&'static str, &'static str)],
}

fn round_trip_as<T>(value: &Value) -> Result<Value, String>
where
    T: DeserializeOwned + Serialize,
{
    let typed: T = serde_json::from_value(value.clone()).map_err(|error| error.to_string())?;
    serde_json::to_value(&typed).map_err(|error| error.to_string())
}

/// 서버가 다루는 계약 타입 전부.
pub static CONTRACT_TYPES: &[ContractType] = &[
    ContractType {
        name: PING_SERVER,
        kind: "command",
        schema_path: "commands/PING_SERVER.schema.json",
        schema_version: 1,
        rust_type: "starfall_contracts::commands::PingServerCommand",
        round_trip: round_trip_as::<PingServerCommand>,
        responses: &[(COMMAND_RESULT, "always"), (PING_REPLY, "accepted")],
    },
    ContractType {
        name: PING_REPLY,
        kind: "server_message",
        schema_path: "messages/PING_REPLY.schema.json",
        schema_version: 1,
        rust_type: "starfall_contracts::messages::PingReplyMessage",
        round_trip: round_trip_as::<PingReplyMessage>,
        responses: &[],
    },
    ContractType {
        name: COMMAND_RESULT,
        kind: "server_message",
        schema_path: "messages/COMMAND_RESULT.schema.json",
        schema_version: 1,
        rust_type: "starfall_contracts::messages::CommandResultMessage",
        round_trip: round_trip_as::<CommandResultMessage>,
        responses: &[],
    },
    ContractType {
        name: SESSION_READY,
        kind: "server_message",
        schema_path: "messages/SESSION_READY.schema.json",
        schema_version: 1,
        rust_type: "starfall_contracts::messages::SessionReadyMessage",
        round_trip: round_trip_as::<SessionReadyMessage>,
        responses: &[],
    },
    ContractType {
        name: SESSION_OPENED,
        kind: "domain_event",
        schema_path: "events/domain/SESSION_OPENED.schema.json",
        schema_version: 1,
        rust_type: "starfall_contracts::events::SessionOpenedEvent",
        round_trip: round_trip_as::<SessionOpenedEvent>,
        responses: &[],
    },
    ContractType {
        name: SESSION_CLOSED,
        kind: "domain_event",
        schema_path: "events/domain/SESSION_CLOSED.schema.json",
        schema_version: 1,
        rust_type: "starfall_contracts::events::SessionClosedEvent",
        round_trip: round_trip_as::<SessionClosedEvent>,
        responses: &[],
    },
    ContractType {
        name: SET_SHIP_CONTROL,
        kind: "command",
        schema_path: "commands/SET_SHIP_CONTROL.schema.json",
        schema_version: 1,
        rust_type: "starfall_contracts::commands::SetShipControlCommand",
        round_trip: round_trip_as::<SetShipControlCommand>,
        responses: &[(COMMAND_RESULT, "always")],
    },
    ContractType {
        name: WORLD_SNAPSHOT,
        kind: "server_message",
        schema_path: "messages/WORLD_SNAPSHOT.schema.json",
        schema_version: 1,
        rust_type: "starfall_contracts::messages::WorldSnapshotMessage",
        round_trip: round_trip_as::<WorldSnapshotMessage>,
        responses: &[],
    },
    ContractType {
        name: SHIP_SPAWNED,
        kind: "domain_event",
        schema_path: "events/domain/SHIP_SPAWNED.schema.json",
        schema_version: 1,
        rust_type: "starfall_contracts::events::ShipSpawnedEvent",
        round_trip: round_trip_as::<ShipSpawnedEvent>,
        responses: &[],
    },
    ContractType {
        name: SHIP_DESPAWNED,
        kind: "domain_event",
        schema_path: "events/domain/SHIP_DESPAWNED.schema.json",
        schema_version: 1,
        rust_type: "starfall_contracts::events::ShipDespawnedEvent",
        round_trip: round_trip_as::<ShipDespawnedEvent>,
        responses: &[],
    },
    ContractType {
        name: SHIP_CLASS,
        kind: "data",
        schema_path: "data/ship-class.schema.json",
        schema_version: 1,
        rust_type: "starfall_contracts::data::ShipClassTable",
        round_trip: round_trip_as::<ShipClassTable>,
        responses: &[],
    },
    ContractType {
        name: STAR_SYSTEM,
        kind: "data",
        schema_path: "data/star-system.schema.json",
        schema_version: 1,
        rust_type: "starfall_contracts::data::StarSystemTable",
        round_trip: round_trip_as::<StarSystemTable>,
        responses: &[],
    },
    ContractType {
        name: SYNC_TUNING,
        kind: "data",
        schema_path: "data/sync-tuning.schema.json",
        schema_version: 1,
        rust_type: "starfall_contracts::data::SyncTuningTable",
        round_trip: round_trip_as::<SyncTuningTable>,
        responses: &[],
    },
    ContractType {
        name: MINE_RESOURCE,
        kind: "command",
        schema_path: "commands/MINE_RESOURCE.schema.json",
        schema_version: 1,
        rust_type: "starfall_contracts::commands::MineResourceCommand",
        round_trip: round_trip_as::<MineResourceCommand>,
        // DEPOSIT_FIELD_STATE(월드 브로드캐스트)·HISTORICAL_EVENT_NOTICE(비동기)는
        // 응답이 아니다(스펙 §5.1a 비고) — 여기 넣지 않는다.
        responses: &[(COMMAND_RESULT, "always"), (INVENTORY_STATE, "accepted")],
    },
    ContractType {
        name: MINERAL_MINED,
        kind: "domain_event",
        schema_path: "events/domain/MINERAL_MINED.schema.json",
        schema_version: 1,
        rust_type: "starfall_contracts::events::MineralMinedEvent",
        round_trip: round_trip_as::<MineralMinedEvent>,
        responses: &[],
    },
    ContractType {
        name: MINERAL_DISCOVERED,
        kind: "historical_event",
        schema_path: "events/historical/MINERAL_DISCOVERED.schema.json",
        schema_version: 1,
        rust_type: "starfall_contracts::historical::MineralDiscoveredEvent",
        round_trip: round_trip_as::<MineralDiscoveredEvent>,
        responses: &[],
    },
    ContractType {
        name: INVENTORY_STATE,
        kind: "server_message",
        schema_path: "messages/INVENTORY_STATE.schema.json",
        schema_version: 1,
        rust_type: "starfall_contracts::messages::InventoryStateMessage",
        round_trip: round_trip_as::<InventoryStateMessage>,
        responses: &[],
    },
    ContractType {
        name: DEPOSIT_FIELD_STATE,
        kind: "server_message",
        schema_path: "messages/DEPOSIT_FIELD_STATE.schema.json",
        schema_version: 1,
        rust_type: "starfall_contracts::messages::DepositFieldStateMessage",
        round_trip: round_trip_as::<DepositFieldStateMessage>,
        responses: &[],
    },
    ContractType {
        name: HISTORICAL_EVENT_NOTICE,
        kind: "server_message",
        schema_path: "messages/HISTORICAL_EVENT_NOTICE.schema.json",
        schema_version: 1,
        rust_type: "starfall_contracts::messages::HistoricalEventNoticeMessage",
        round_trip: round_trip_as::<HistoricalEventNoticeMessage>,
        responses: &[],
    },
    ContractType {
        name: MINERAL,
        kind: "data",
        schema_path: "data/mineral.schema.json",
        schema_version: 1,
        rust_type: "starfall_contracts::data::MineralTable",
        round_trip: round_trip_as::<MineralTable>,
        responses: &[],
    },
    ContractType {
        name: DEPOSIT_FIELD,
        kind: "data",
        schema_path: "data/deposit-field.schema.json",
        schema_version: 1,
        rust_type: "starfall_contracts::data::DepositFieldTable",
        round_trip: round_trip_as::<DepositFieldTable>,
        responses: &[],
    },
    ContractType {
        name: MINING_RULES,
        kind: "data",
        schema_path: "data/mining-rules.schema.json",
        schema_version: 1,
        rust_type: "starfall_contracts::data::MiningRulesTable",
        round_trip: round_trip_as::<MiningRulesTable>,
        responses: &[],
    },
    ContractType {
        name: SIGNIFICANCE_RULE,
        kind: "data",
        schema_path: "data/significance-rule.schema.json",
        schema_version: 1,
        rust_type: "starfall_contracts::data::SignificanceRuleTable",
        round_trip: round_trip_as::<SignificanceRuleTable>,
        responses: &[],
    },
];

/// 이름으로 계약 타입을 찾는다.
#[must_use]
pub fn find(name: &str) -> Option<&'static ContractType> {
    CONTRACT_TYPES.iter().find(|entry| entry.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_has_no_duplicate_names() {
        let mut names: Vec<&str> = CONTRACT_TYPES.iter().map(|entry| entry.name).collect();
        let total = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), total, "대응표에 중복된 타입 이름이 있다");
    }

    #[test]
    fn find_returns_registered_types() {
        for name in [
            PING_SERVER,
            PING_REPLY,
            COMMAND_RESULT,
            SESSION_READY,
            SESSION_OPENED,
            SESSION_CLOSED,
            SET_SHIP_CONTROL,
            WORLD_SNAPSHOT,
            SHIP_SPAWNED,
            SHIP_DESPAWNED,
            SHIP_CLASS,
            STAR_SYSTEM,
            SYNC_TUNING,
            MINE_RESOURCE,
            MINERAL_MINED,
            MINERAL_DISCOVERED,
            INVENTORY_STATE,
            DEPOSIT_FIELD_STATE,
            HISTORICAL_EVENT_NOTICE,
            MINERAL,
            DEPOSIT_FIELD,
            MINING_RULES,
            SIGNIFICANCE_RULE,
        ] {
            assert!(find(name).is_some(), "{name} 이 대응표에 없다");
        }
        assert_eq!(CONTRACT_TYPES.len(), 23);
        assert!(find("NOT_A_CONTRACT_TYPE").is_none());
    }
}

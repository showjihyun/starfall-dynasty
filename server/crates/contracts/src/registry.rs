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

use crate::commands::PingServerCommand;
use crate::messages::PingReplyMessage;

/// `PING_SERVER` 레지스트리 이름.
pub const PING_SERVER: &str = "PING_SERVER";
/// `PING_REPLY` 레지스트리 이름.
pub const PING_REPLY: &str = "PING_REPLY";

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
    },
    ContractType {
        name: PING_REPLY,
        kind: "server_message",
        schema_path: "messages/PING_REPLY.schema.json",
        schema_version: 1,
        rust_type: "starfall_contracts::messages::PingReplyMessage",
        round_trip: round_trip_as::<PingReplyMessage>,
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
        assert!(find(PING_SERVER).is_some());
        assert!(find(PING_REPLY).is_some());
        assert!(find("NOT_A_CONTRACT_TYPE").is_none());
    }
}

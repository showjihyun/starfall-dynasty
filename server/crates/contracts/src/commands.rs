//! 명령 타입 — 클라이언트 → 서버 의도.
//!
//! # envelope 을 `#[serde(flatten)]` 으로 공유하지 않는다 (ADR-0002 §3)
//!
//! serde 의 `deny_unknown_fields` 는 `flatten` 과 함께 동작하지 않고, 내부 태그 열거형
//! (`#[serde(tag = "...")]`)도 같은 버퍼링 경로라 마찬가지다. 둘 중 하나라도 쓰면
//! `actor-field-injected` 반례를 **스키마는 거부하는데 서버는 받아들인다** — 원칙 1의
//! 계약 수준 방어가 운영 경로에서 무너진다. 그래서 타입마다 envelope 필드를 펼쳐 쓰고
//! `deny_unknown_fields` 를 건다. 반복은 타입이 늘면 선언 매크로로 줄이되 이 속성은 유지한다.
//!
//! # 명령 envelope 에는 행위자 필드가 없다 (I-6)
//!
//! 행위자는 인증된 세션에서 서버가 정한다. 클라이언트가 `player_id` 를 주장할 자리를 만들지 않는다.

use serde::{Deserialize, Serialize};

use crate::primitives::{ConstSchemaVersion, ProbeSeq, RealTime, UuidV7, required_nullable};

/// `PING_SERVER` 의 타입 상수.
///
/// 단일 변형 열거형이라 다른 문자열은 역직렬화에서 거부되고, 직렬화하면 언제나
/// `"PING_SERVER"` 가 나간다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum PingServerType {
    /// 유일한 값.
    #[default]
    #[serde(rename = "PING_SERVER")]
    PingServer,
}

/// `PING_SERVER` payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PingServerPayload {
    /// 클라이언트가 고른 프로브 카운터. 응답 대응과 클라이언트 시계 기준 왕복 시간 계산에 쓴다.
    pub probe_seq: ProbeSeq,
}

/// `PING_SERVER` — 연결·명령 경로 프로브. 게임 상태를 바꾸지 않는다.
///
/// 대응 스키마: `contracts/commands/PING_SERVER.schema.json`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PingServerCommand {
    /// 클라이언트가 만든 UUIDv7. 멱등 키다.
    ///
    /// 서버는 이 값을 **멱등 키로만** 쓰고 내장 타임스탬프를 신뢰하거나 해석하지 않는다.
    /// 클라이언트 시계는 신뢰 대상이 아니다 (ADR-0002 §5).
    pub command_id: UuidV7,
    /// 언제나 `PING_SERVER`.
    pub command_type: PingServerType,
    /// 언제나 1.
    pub schema_version: ConstSchemaVersion<1>,
    /// 전송 시점의 클라이언트 시계, 또는 null. 참고용이며 규칙에 쓰지 않는다.
    ///
    /// `deserialize_with` 가 붙은 이유는 [`required_nullable`] 문서 참고 — **키 자체는 반드시
    /// 있어야 한다**(I-5). 직렬화 쪽에는 아무 속성도 달지 않는다(`skip_serializing_if` 금지).
    #[serde(deserialize_with = "required_nullable")]
    pub client_sent_at: Option<RealTime>,
    /// 타입별 payload.
    pub payload: PingServerPayload,
}

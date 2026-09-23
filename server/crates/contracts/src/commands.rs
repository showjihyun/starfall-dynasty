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

use crate::primitives::{
    ConstSchemaVersion, ControlAxisMilli, InputSeq, ProbeSeq, QuaternionComponentMicro, RealTime,
    UuidV7, required_nullable,
};

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

// ---------------------------------------------------------------------------
// SET_SHIP_CONTROL
// ---------------------------------------------------------------------------

/// `SET_SHIP_CONTROL` 의 타입 상수.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SetShipControlType {
    /// 유일한 값.
    #[default]
    #[serde(rename = "SET_SHIP_CONTROL")]
    SetShipControl,
}

/// `SET_SHIP_CONTROL` payload — 한 tick의 조작 의도 전체.
///
/// **위치·속도·현재 자세 필드가 없다**(I-26). `additionalProperties: false` +
/// `deny_unknown_fields` 가 함께, 그것들을 주입한 명령을 스키마와 serde 양쪽에서 거부한다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SetShipControlPayload {
    /// 세션의 입력 카운터. 서버는 마지막으로 적용한 값을 `WORLD_SNAPSHOT.ack_input_seq` 로
    /// 돌려준다.
    pub input_seq: InputSeq,
    /// 함선 로컬 +X(우현) 추력 의도.
    pub thrust_x_milli: ControlAxisMilli,
    /// 함선 로컬 +Y(위) 추력 의도.
    pub thrust_y_milli: ControlAxisMilli,
    /// 함선 로컬 +Z(전방) 추력 의도. 양수는 주 엔진, 음수는 후진 추력.
    pub thrust_z_milli: ControlAxisMilli,
    /// 함선 로컬 +Z 축 둘레 수동 롤 의도. 0이 아니면 그 tick 오토레벨이 쉰다.
    pub roll_milli: ControlAxisMilli,
    /// 목표 자세 쿼터니언 x. **의도이지 현재 상태에 대한 주장이 아니다**(I-26).
    pub aim_x_micro: QuaternionComponentMicro,
    /// 목표 자세 쿼터니언 y.
    pub aim_y_micro: QuaternionComponentMicro,
    /// 목표 자세 쿼터니언 z.
    pub aim_z_micro: QuaternionComponentMicro,
    /// 목표 자세 쿼터니언 w.
    pub aim_w_micro: QuaternionComponentMicro,
    /// 브레이크. 켜져 있으면 추력이 무시되고 감쇠 하나만 적용된다.
    pub brake: bool,
    /// 비행 보조. `false` 면 순수 뉴턴 비행(감쇠·오토레벨 없음).
    pub flight_assist: bool,
}

/// `SET_SHIP_CONTROL` — 세션의 함선에 대한 한 tick의 조작 의도.
///
/// 대응 스키마: `contracts/commands/SET_SHIP_CONTROL.schema.json`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SetShipControlCommand {
    /// 클라이언트가 만든 UUIDv7. 멱등 키다.
    pub command_id: UuidV7,
    /// 언제나 `SET_SHIP_CONTROL`.
    pub command_type: SetShipControlType,
    /// 언제나 1.
    pub schema_version: ConstSchemaVersion<1>,
    /// 전송 시점의 클라이언트 시계, 또는 null. 참고용이며 규칙에 쓰지 않는다.
    #[serde(deserialize_with = "required_nullable")]
    pub client_sent_at: Option<RealTime>,
    /// 타입별 payload.
    pub payload: SetShipControlPayload,
}

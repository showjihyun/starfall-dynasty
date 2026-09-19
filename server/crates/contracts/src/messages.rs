//! 서버 메시지 타입 — 서버 → 클라이언트 실시간 메시지.
//!
//! envelope 을 펼쳐 쓰는 이유는 [`crate::commands`] 모듈 문서와 같다.

use serde::{Deserialize, Serialize};

use crate::primitives::{
    ConstSchemaVersion, ProbeSeq, ServerVersion, Tick, TickHz, UuidV7, required_nullable,
};

/// `PING_REPLY` 의 타입 상수.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum PingReplyType {
    /// 유일한 값.
    #[default]
    #[serde(rename = "PING_REPLY")]
    PingReply,
}

/// `PING_REPLY` payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PingReplyPayload {
    /// 응답 대상 `PING_SERVER` 의 `command_id`.
    ///
    /// 응답 대응은 envelope 의 `correlation_id` 가 아니라 이 필드로 한다 — envelope 쪽은
    /// "하나의 게임플레이 트랜잭션"을 묶는 용도다.
    pub command_id: UuidV7,
    /// `PING_SERVER` payload 에서 그대로 복사한 값.
    pub probe_seq: ProbeSeq,
}

/// `PING_REPLY` — `PING_SERVER` 에 대한 서버 응답.
///
/// 대응 스키마: `contracts/messages/PING_REPLY.schema.json`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PingReplyMessage {
    /// 추적·로그 상관용 서버 생성 UUIDv7.
    pub message_id: UuidV7,
    /// 언제나 `PING_REPLY`.
    pub message_type: PingReplyType,
    /// 언제나 1.
    pub schema_version: ConstSchemaVersion<1>,
    /// 이 메시지를 만든 서버 시뮬레이션 tick.
    pub tick: Tick,
    /// 이 메시지가 속한 게임플레이 트랜잭션, 또는 null.
    ///
    /// 키는 항상 존재한다 (I-5).
    #[serde(deserialize_with = "required_nullable")]
    pub correlation_id: Option<UuidV7>,
    /// 타입별 payload.
    pub payload: PingReplyPayload,
}

// ---------------------------------------------------------------------------
// COMMAND_RESULT
// ---------------------------------------------------------------------------

/// `COMMAND_RESULT` 의 타입 상수.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum CommandResultType {
    /// 유일한 값.
    #[default]
    #[serde(rename = "COMMAND_RESULT")]
    CommandResult,
}

/// 명령 판정 결과.
///
/// `ACCEPTED` 는 **시뮬레이션에 들어갔다**는 뜻이지 의도가 완료됐다는 뜻이 아니다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CommandStatus {
    /// envelope 의 tick 에서 시뮬레이션에 접수됐다.
    Accepted,
    /// 접수되지 않았다. `reason_code` 가 이유이고 월드 상태는 바뀌지 않았다.
    Rejected,
}

/// 거부 사유. `schema_version` 1의 닫힌 집합.
///
/// # 왜 Rust 는 닫힌 열거형이고 C# 은 `string` 인가 (ADR-0005 §4)
///
/// 서버는 모르는 값을 **거부**해야 하고(클라이언트를 믿지 않는다), 클라이언트는 모르는 값을
/// 받아도 **죽지 않아야 한다**(서버가 먼저 배포된다). 값 추가는 같은 `schema_version` 의
/// 호환 변경이다 — 그래서 여기 변형을 추가하는 것은 계약 변경이 아니라 계약 **따라가기**다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RejectReasonCode {
    /// 프레임을 계약 타입으로 읽을 수 없었다.
    MalformedCommand,
    /// 레지스트리에 없는 `command_type`.
    UnknownCommandType,
    /// 서버가 지원하지 않는 `schema_version`.
    SchemaVersionUnsupported,
    /// 세션 안에서 이미 처리한 `command_id`.
    DuplicateCommandId,
    /// 전역 명령 큐가 가득 찼다.
    ServerBusy,
    /// 세션의 미응답 명령 상한을 넘었다.
    TooManyInFlight,
}

/// `COMMAND_RESULT` payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandResultPayload {
    /// 답하는 명령의 `command_id`. 서버는 이 값을 **멱등 키로만** 쓴다 (I-11).
    pub command_id: UuidV7,
    /// 판정 결과.
    pub status: CommandStatus,
    /// `ACCEPTED` 면 `null`, `REJECTED` 면 사유 (I-14).
    ///
    /// 스키마는 이 상관을 표현하지 못한다(ADR-0005 §4). 생성 측은
    /// [`CommandResultPayload::accepted`] / [`CommandResultPayload::rejected`] 만 쓴다.
    #[serde(deserialize_with = "required_nullable")]
    pub reason_code: Option<RejectReasonCode>,
}

impl CommandResultPayload {
    /// 접수 결과. `reason_code` 는 언제나 `None` 이다 (I-14).
    #[must_use]
    pub const fn accepted(command_id: UuidV7) -> Self {
        Self {
            command_id,
            status: CommandStatus::Accepted,
            reason_code: None,
        }
    }

    /// 거부 결과. `reason_code` 는 언제나 `Some` 이다 (I-14).
    #[must_use]
    pub const fn rejected(command_id: UuidV7, reason: RejectReasonCode) -> Self {
        Self {
            command_id,
            status: CommandStatus::Rejected,
            reason_code: Some(reason),
        }
    }

    /// I-14 를 만족하는가. 테스트와 발행 직전 단언에 쓴다.
    #[must_use]
    pub const fn invariant_holds(&self) -> bool {
        matches!(
            (self.status, self.reason_code),
            (CommandStatus::Accepted, None) | (CommandStatus::Rejected, Some(_))
        )
    }
}

/// `COMMAND_RESULT` — 명령 1건의 판정 결과.
///
/// 대응 스키마: `contracts/messages/COMMAND_RESULT.schema.json`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandResultMessage {
    /// 추적·로그 상관용 서버 생성 UUIDv7.
    pub message_id: UuidV7,
    /// 언제나 `COMMAND_RESULT`.
    pub message_type: CommandResultType,
    /// 언제나 1.
    pub schema_version: ConstSchemaVersion<1>,
    /// 판정한 tick. 큐에 넣지 못해 게이트웨이가 만든 거부는 게이트웨이가 읽은 현재 tick.
    pub tick: Tick,
    /// **이 슬라이스에서는 언제나 `null`** (스펙 §5.2). 명령은 게임플레이 트랜잭션을
    /// 시작하지 않았고, 세션 correlation 을 여기 넣으면 I-12 가 경고한 오용의 여지가 생긴다.
    #[serde(deserialize_with = "required_nullable")]
    pub correlation_id: Option<UuidV7>,
    /// 타입별 payload.
    pub payload: CommandResultPayload,
}

// ---------------------------------------------------------------------------
// SESSION_READY
// ---------------------------------------------------------------------------

/// `SESSION_READY` 의 타입 상수.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SessionReadyType {
    /// 유일한 값.
    #[default]
    #[serde(rename = "SESSION_READY")]
    SessionReady,
}

/// `SESSION_READY` payload.
///
/// 클라이언트는 여기 있는 값 중 **어느 것도 주장하지 않는다.** 전부 통보받는다 (I-10).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionReadyPayload {
    /// 이 연결의 세션 id. 재연결은 **새 세션**이다 (I-23).
    pub session_id: UuidV7,
    /// 세션이 붙은 월드.
    pub world_id: UuidV7,
    /// 검증된 자격 증명에서 **서버가 정한** 행위자 (I-10).
    pub actor_id: UuidV7,
    /// 이 월드의 tick 주기. 월드 수명 동안 불변 (I-19).
    pub tick_hz: TickHz,
    /// 서버 빌드 버전. 로그·버그 리포트 전용.
    pub server_version: ServerVersion,
}

/// `SESSION_READY` — 인증된 실시간 연결의 **첫 계약 메시지**.
///
/// 대응 스키마: `contracts/messages/SESSION_READY.schema.json`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionReadyMessage {
    /// 추적·로그 상관용 서버 생성 UUIDv7.
    pub message_id: UuidV7,
    /// 언제나 `SESSION_READY`.
    pub message_type: SessionReadyType,
    /// 언제나 1.
    pub schema_version: ConstSchemaVersion<1>,
    /// 세션을 수락한 tick. `SESSION_OPENED` 의 tick 과 같다.
    pub tick: Tick,
    /// 세션 correlation. `SESSION_OPENED`·`SESSION_CLOSED` 와 **같은 값**이다.
    #[serde(deserialize_with = "required_nullable")]
    pub correlation_id: Option<UuidV7>,
    /// 타입별 payload.
    pub payload: SessionReadyPayload,
}

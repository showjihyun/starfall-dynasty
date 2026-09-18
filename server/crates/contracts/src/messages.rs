//! 서버 메시지 타입 — 서버 → 클라이언트 실시간 메시지.
//!
//! envelope 을 펼쳐 쓰는 이유는 [`crate::commands`] 모듈 문서와 같다.

use serde::{Deserialize, Serialize};

use crate::primitives::{ConstSchemaVersion, ProbeSeq, Tick, UuidV7, required_nullable};

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

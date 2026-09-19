//! 봇이 보는 와이어 타입 — **서버 코드와 독립으로 쓴 것**.
//!
//! # 왜 `starfall-contracts` 를 path 의존하지 않는가
//!
//! `01_architect_tasks.md` T12 는 "`starfall-contracts` 를 path 의존"이라고 적었다. 여기서는
//! 그렇게 하지 않고 같은 모양을 독립으로 썼다. 이유 두 가지:
//!
//! 1. **관측자의 독립성**(스펙 I-25). 봇은 3자 대조의 한 축이다. 서버가 쓰는 바로 그 serde
//!    타입으로 서버의 출력을 읽으면, 타입이 틀렸을 때 봇도 똑같이 틀려서 아무 차이도 나지 않는다.
//!    같은 실수를 두 곳에서 하면 대조가 통과한다.
//! 2. 계약이 코드보다 앞서 있어 `server/crates/contracts` 가 지금 바뀌는 중이다. 여기에 묶이면
//!    서버가 컴파일되지 않는 동안 QA 도구도 서지 않는다.
//!
//! 대신 **정본과의 일치는 `tests/wire_fixtures.rs` 가 계약 fixture 로 직접 검증한다** —
//! `contracts/fixtures/**` 를 이 타입들로 역직렬화 → 재직렬화 → 원본과 `Value` 비교한다.
//! 즉 봇은 서버가 아니라 **계약**에 맞춰져 있고, 그것이 독립 관측의 조건이다.
//!
//! 이 결정은 architect 통지 대상이다(`04_qa_report_r1.md` 에 기록).

use serde::{Deserialize, Serialize};
use uuid::Uuid;

// 계약 커버리지 스크립트(`check_contract_coverage.py`)가 `tools/bots/**/*.rs` 에서
// 이 리터럴들을 찾는다. 레지스트리의 `bots` 태그 4건이 여기에 대응한다.
pub const PING_SERVER: &str = "PING_SERVER";
pub const PING_REPLY: &str = "PING_REPLY";
pub const COMMAND_RESULT: &str = "COMMAND_RESULT";
pub const SESSION_READY: &str = "SESSION_READY";

/// 명령 envelope (contracts/common/command-envelope.schema.json).
///
/// `client_sent_at` 은 **키가 항상 존재하고 값이 null 일 수 있다**(I-5). 그래서
/// `skip_serializing_if` 를 쓰지 않는다. 서버는 이 값을 판정·정렬에 쓰지 않는다(I-11).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PingServerCommand {
    pub command_id: Uuid,
    pub command_type: String,
    pub schema_version: u32,
    pub client_sent_at: Option<String>,
    pub payload: PingServerPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PingServerPayload {
    pub probe_seq: u32,
}

impl PingServerCommand {
    pub fn new(command_id: Uuid, probe_seq: u32) -> Self {
        Self {
            command_id,
            command_type: PING_SERVER.to_owned(),
            schema_version: 1,
            // 봇은 실제 시각을 넣지 않는다. 왕복은 봇의 단조 시계로 재고(ADR-0006 §4),
            // 이 필드는 서버 로그 전용이다. null 을 넣어 "판정에 쓰지 않는다"를 구조로 만든다.
            client_sent_at: None,
            payload: PingServerPayload { probe_seq },
        }
    }
}

/// 서버 메시지 envelope. `sequence`·`world_id`·`occurred_at` 은 **없다**
/// (도메인 이벤트 전용 — 스펙 §5.2). 여기 넣으면 계약과 어긋난다.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SessionReadyMessage {
    pub message_id: Uuid,
    pub message_type: String,
    pub schema_version: u32,
    pub tick: u64,
    pub correlation_id: Option<Uuid>,
    pub payload: SessionReadyPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SessionReadyPayload {
    pub session_id: Uuid,
    pub world_id: Uuid,
    pub actor_id: Uuid,
    pub tick_hz: u32,
    pub server_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CommandResultMessage {
    pub message_id: Uuid,
    pub message_type: String,
    pub schema_version: u32,
    pub tick: u64,
    pub correlation_id: Option<Uuid>,
    pub payload: CommandResultPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CommandResultPayload {
    pub command_id: Uuid,
    /// `ACCEPTED` | `REJECTED`. **닫힌 열거형으로 만들지 않는다** — 봇은 소비자이고,
    /// 값이 추가되어도 죽지 않아야 한다(ADR-0005 §4의 C# 쪽과 같은 이유).
    /// 모르는 값은 `summary.json` 의 `unknown_status` 로 드러난다.
    pub status: String,
    pub reason_code: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PingReplyMessage {
    pub message_id: Uuid,
    pub message_type: String,
    pub schema_version: u32,
    pub tick: u64,
    pub correlation_id: Option<Uuid>,
    pub payload: PingReplyPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PingReplyPayload {
    pub command_id: Uuid,
    pub probe_seq: u32,
}

/// 수신 프레임 1개를 해석한 결과.
///
/// `Unknown` 은 **경고이지 오류가 아니다**(ADR-0005 §5 와 같은 관용). `Malformed` 는
/// 계수되어 `summary.json` 의 `wire_errors` 로 드러난다 — 조용히 버리면 손실 계측이 거짓이 된다.
#[derive(Debug, Clone)]
pub enum Inbound {
    SessionReady(Box<SessionReadyMessage>),
    CommandResult(Box<CommandResultMessage>),
    PingReply(Box<PingReplyMessage>),
    Unknown { message_type: String },
    Malformed { reason: String, raw: String },
}

/// `message_type` 을 먼저 보고(peek) 그 타입으로만 역직렬화한다.
/// `#[serde(flatten)]` 이나 내부 태그 열거형을 쓰지 않는 이유는 계약 규약과 같다
/// (ADR-0002 §3: `deny_unknown_fields` 가 무력화된다).
pub fn parse_inbound(text: &str) -> Inbound {
    let value: serde_json::Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(e) => {
            return Inbound::Malformed {
                reason: format!("not json: {e}"),
                raw: truncate(text),
            };
        }
    };
    let Some(mt) = value.get("message_type").and_then(|v| v.as_str()) else {
        return Inbound::Malformed {
            reason: "no message_type".to_owned(),
            raw: truncate(text),
        };
    };
    match mt {
        SESSION_READY => match serde_json::from_value(value) {
            Ok(m) => Inbound::SessionReady(Box::new(m)),
            Err(e) => Inbound::Malformed {
                reason: format!("SESSION_READY: {e}"),
                raw: truncate(text),
            },
        },
        COMMAND_RESULT => match serde_json::from_value(value) {
            Ok(m) => Inbound::CommandResult(Box::new(m)),
            Err(e) => Inbound::Malformed {
                reason: format!("COMMAND_RESULT: {e}"),
                raw: truncate(text),
            },
        },
        PING_REPLY => match serde_json::from_value(value) {
            Ok(m) => Inbound::PingReply(Box::new(m)),
            Err(e) => Inbound::Malformed {
                reason: format!("PING_REPLY: {e}"),
                raw: truncate(text),
            },
        },
        other => Inbound::Unknown {
            message_type: other.to_owned(),
        },
    }
}

fn truncate(s: &str) -> String {
    const MAX: usize = 400;
    if s.len() <= MAX {
        s.to_owned()
    } else {
        let mut end = MAX;
        while !s.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}…", &s[..end])
    }
}

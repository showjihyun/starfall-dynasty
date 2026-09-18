//! 메시지 디스패치용 peek 구조체.
//!
//! # 왜 이것이 따로 있어야 하는가 (ADR-0002 §3)
//!
//! "들어온 JSON 의 `command_type` 을 먼저 보고 알맞은 타입으로 역직렬화한다"를 구현하는
//! 가장 자연스러운 방법은 내부 태그 열거형이다.
//!
//! ```ignore
//! #[serde(tag = "command_type")]   // <- 이렇게 하면 안 된다
//! enum Command { PingServer(PingServerCommand) }
//! ```
//!
//! 이 방식은 `deny_unknown_fields` 를 무력화해서 `actor-field-injected` 반례가
//! **스키마는 거부하는데 서버는 통과시키는** 구멍을 만든다. `#[serde(flatten)]` 도 같다.
//!
//! 그래서 디스패치는 두 단계로 한다: (1) 타입 이름만 읽는 이 peek 구조체, (2) 알아낸 이름으로
//! 고른 **`deny_unknown_fields` 가 살아 있는** 구체 타입으로 전체를 다시 역직렬화.
//! 이 모듈이 `deny_unknown_fields` 를 달지 않는 **유일한** 예외다 — peek 는 나머지 필드를
//! 의도적으로 무시해야 하기 때문이다.

use serde::Deserialize;

/// 명령 JSON 에서 `command_type` 만 읽는다.
#[derive(Debug, Clone, Deserialize)]
pub struct CommandTypePeek {
    /// 레지스트리 타입 이름.
    pub command_type: String,
}

/// 서버 메시지 JSON 에서 `message_type` 만 읽는다.
#[derive(Debug, Clone, Deserialize)]
pub struct MessageTypePeek {
    /// 레지스트리 타입 이름.
    pub message_type: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::PING_SERVER;

    #[test]
    fn peek_reads_type_without_consuming_the_rest() {
        let raw = r#"{
            "command_id": "01a0afaf-7e83-7f49-adfa-58ba04c2fd5a",
            "command_type": "PING_SERVER",
            "schema_version": 1,
            "client_sent_at": null,
            "payload": { "probe_seq": 7 }
        }"#;
        let peek: CommandTypePeek = serde_json::from_str(raw).unwrap();
        assert_eq!(peek.command_type, PING_SERVER);
    }

    #[test]
    fn peek_tolerates_unknown_fields_but_the_concrete_type_does_not() {
        // 이 두 단언이 함께 있어야 의미가 있다: peek 는 통과시키고, 구체 타입은 막는다.
        let raw = r#"{
            "command_id": "01a0afaf-7e83-7f49-adfa-58ba04c2fd5a",
            "command_type": "PING_SERVER",
            "schema_version": 1,
            "client_sent_at": null,
            "player_id": "01a0afaf-7f7d-7c16-b3d0-f1a4053c530b",
            "payload": { "probe_seq": 1 }
        }"#;
        assert!(serde_json::from_str::<CommandTypePeek>(raw).is_ok());
        assert!(
            serde_json::from_str::<crate::commands::PingServerCommand>(raw).is_err(),
            "행위자 필드가 주입된 명령을 구체 타입이 통과시켰다 — I-6 위반"
        );
    }
}

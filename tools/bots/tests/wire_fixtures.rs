//! **봇의 와이어 타입이 계약과 같은 모양인가.**
//!
//! 봇은 서버의 serde 타입을 쓰지 않는다(독립 관측자여야 하므로 — `src/wire.rs` 머리말).
//! 그 대가로 "봇이 서버 메시지를 잘못 읽고 있다"는 위험이 생기는데, 여기서 **계약 fixture** 로
//! 막는다. 정본은 `contracts/` 이고 서버도 봇도 각자 거기에 맞춘다.
//!
//! 빈 순회 방지: 실제로 읽은 fixture 개수를 단언한다(계약 §0.3).

use std::path::{Path, PathBuf};

use starfall_bots::wire::{self, Inbound};

fn repo_root() -> PathBuf {
    // tools/bots/ -> tools/ -> repo root. 마커로 확인한다(p0-01 의 로더와 같은 방식).
    let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    loop {
        if dir.join("contracts/registry/types.json").is_file() {
            return dir;
        }
        assert!(dir.pop(), "contracts/registry/types.json 을 찾지 못했다");
    }
}

fn read_valid_fixtures(type_name: &str) -> Vec<(String, String)> {
    let dir = repo_root().join("contracts/fixtures").join(type_name);
    let mut out = Vec::new();
    let entries = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("fixture 디렉토리를 열 수 없다 {}: {e}", dir.display()));
    for e in entries.flatten() {
        let p = e.path();
        if p.is_file() && p.extension().and_then(|s| s.to_str()) == Some("json") {
            let text = std::fs::read_to_string(&p)
                .unwrap_or_else(|e| panic!("읽기 실패 {}: {e}", p.display()));
            out.push((file_name(&p), text));
        }
    }
    out.sort();
    out
}

fn file_name(p: &Path) -> String {
    p.file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("<?>")
        .to_owned()
}

fn read_invalid_fixtures(type_name: &str) -> Vec<(String, String)> {
    let dir = repo_root()
        .join("contracts/fixtures")
        .join(type_name)
        .join("invalid");
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for e in entries.flatten() {
            let p = e.path();
            let is_json = p.is_file() && p.extension().and_then(|s| s.to_str()) == Some("json");
            if let (true, Ok(text)) = (is_json, std::fs::read_to_string(&p)) {
                out.push((file_name(&p), text));
            }
        }
    }
    out.sort();
    out
}

/// 서버가 보내는 3종: 봇이 읽고 다시 써서 **원본과 같은 JSON** 이 되는가.
#[test]
fn server_message_fixtures_round_trip() {
    let mut checked = 0usize;
    for type_name in ["SESSION_READY", "COMMAND_RESULT", "PING_REPLY"] {
        let fixtures = read_valid_fixtures(type_name);
        assert_eq!(
            fixtures.len(),
            2,
            "{type_name}: 유효 fixture 가 2건이어야 한다 (계약 §5.3). 수가 바뀌었으면 \
             architect 에게 알리고 이 상수를 함께 고친다"
        );
        for (name, text) in fixtures {
            let original: serde_json::Value =
                serde_json::from_str(&text).unwrap_or_else(|e| panic!("{name}: {e}"));
            let parsed = wire::parse_inbound(&text);
            let round_tripped = match parsed {
                Inbound::SessionReady(m) => serde_json::to_value(&*m),
                Inbound::CommandResult(m) => serde_json::to_value(&*m),
                Inbound::PingReply(m) => serde_json::to_value(&*m),
                Inbound::Unknown { message_type } => {
                    panic!("{name}: 모르는 타입으로 읽혔다: {message_type}")
                }
                Inbound::Malformed { reason, .. } => {
                    panic!("{name}: 봇이 계약 fixture 를 읽지 못한다: {reason}")
                }
            }
            .unwrap_or_else(|e| panic!("{name}: 재직렬화 실패 {e}"));
            assert_eq!(
                round_tripped, original,
                "{name}: 왕복 결과가 원본과 다르다 (필드 누락·이름 불일치·널 처리 차이)"
            );
            checked += 1;
        }
    }
    assert_eq!(
        checked, 6,
        "서버 메시지 유효 fixture 6건을 전부 검사해야 한다"
    );
    eprintln!("checked {checked} valid server-message fixtures");
}

/// 봇이 **보내는** 명령이 계약 fixture 와 같은 모양인가.
#[test]
fn ping_server_fixtures_round_trip() {
    let fixtures = read_valid_fixtures("PING_SERVER");
    assert_eq!(fixtures.len(), 2, "PING_SERVER 유효 fixture 는 2건이다");
    for (name, text) in &fixtures {
        let original: serde_json::Value =
            serde_json::from_str(text).unwrap_or_else(|e| panic!("{name}: {e}"));
        let cmd: wire::PingServerCommand =
            serde_json::from_str(text).unwrap_or_else(|e| panic!("{name}: 봇이 읽지 못한다: {e}"));
        let back = serde_json::to_value(&cmd).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(back, original, "{name}: 왕복 결과가 원본과 다르다");
    }
    eprintln!("checked {} valid PING_SERVER fixtures", fixtures.len());
}

/// 봇이 만드는 명령이 계약의 필수 키를 전부 갖는가.
///
/// 특히 `client_sent_at` 은 **키가 있고 값이 null** 이어야 한다(I-5). `skip_serializing_if` 로
/// 키를 빼면 서버의 `deny_unknown_fields`/`required` 와 어긋난다.
#[test]
fn generated_command_has_all_required_keys_including_null_client_sent_at() {
    let cmd = wire::PingServerCommand::new(uuid::Uuid::now_v7(), 42);
    let v = serde_json::to_value(&cmd).expect("serialize");
    let obj = v.as_object().expect("object");
    for key in [
        "command_id",
        "command_type",
        "schema_version",
        "client_sent_at",
        "payload",
    ] {
        assert!(obj.contains_key(key), "필수 키가 빠졌다: {key}");
    }
    assert!(
        obj["client_sent_at"].is_null(),
        "client_sent_at 은 키가 있고 값이 null 이어야 한다 (I-5)"
    );
    assert_eq!(obj["command_type"], serde_json::json!(wire::PING_SERVER));
    assert_eq!(obj["schema_version"], serde_json::json!(1));
    assert_eq!(obj["payload"]["probe_seq"], serde_json::json!(42));
    assert_eq!(obj.len(), 5, "명령 envelope 에 없는 키를 넣지 않는다");
}

/// 반례 중 **봇이 반드시 거부해야 하는 것**: 필수 필드 누락, 널 불가 필드의 널.
///
/// 봇이 이것을 통과시키면(예: 빈 Guid 로) 서버의 잘못된 메시지를 정상으로 계수하게 된다 —
/// C# `Runtime` 프로필에 대한 ADR-0005 §5 의 요구와 같은 이유다.
#[test]
fn malformed_server_messages_are_rejected_not_silently_accepted() {
    let cases = [
        ("SESSION_READY", "missing-session-id.json"),
        ("PING_REPLY", "missing-tick.json"),
        ("PING_REPLY", "payload-unknown-field.json"),
        ("COMMAND_RESULT", "payload-unknown-field.json"),
    ];
    let mut checked = 0usize;
    for (type_name, file) in cases {
        let found = read_invalid_fixtures(type_name)
            .into_iter()
            .find(|(n, _)| n == file);
        let Some((name, text)) = found else {
            panic!("반례 fixture 가 없다: {type_name}/invalid/{file} (계약이 바뀌었는가?)");
        };
        match wire::parse_inbound(&text) {
            Inbound::Malformed { .. } => checked += 1,
            other => panic!(
                "{name}: 봇이 깨진 메시지를 받아들였다 → {other:?}. \
                 계측기가 거짓 데이터를 정상으로 셀 수 있다"
            ),
        }
    }
    assert_eq!(checked, 4);
}

/// 모르는 `message_type` 은 **죽지 않고 경고로** 드러난다(ADR-0005 §5).
#[test]
fn unknown_message_type_is_tolerated() {
    let text = r#"{"message_id":"01a0b1c2-3d4e-7f01-8a2b-9c0d1e2f3a4b",
                   "message_type":"FUTURE_MESSAGE","schema_version":1,"tick":1,
                   "correlation_id":null,"payload":{}}"#;
    match wire::parse_inbound(text) {
        Inbound::Unknown { message_type } => assert_eq!(message_type, "FUTURE_MESSAGE"),
        other => panic!("모르는 타입을 관용해야 한다: {other:?}"),
    }
}

/// 계약 커버리지 스크립트가 찾는 리터럴이 실제로 코드에 있는가(SC-70 의 warnings 4건).
#[test]
fn contract_type_literals_are_present() {
    assert_eq!(wire::PING_SERVER, "PING_SERVER");
    assert_eq!(wire::PING_REPLY, "PING_REPLY");
    assert_eq!(wire::COMMAND_RESULT, "COMMAND_RESULT");
    assert_eq!(wire::SESSION_READY, "SESSION_READY");
}

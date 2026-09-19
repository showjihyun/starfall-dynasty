//! 개발용 토큰의 **독립 교차 검증** — server 정본에 정렬됨(2026-09-19).
//!
//! 기준값의 출처가 셋이고 셋이 일치한다:
//! 1. **server** 가 `03_server_impl.md` 로 준 `bot-000`·`bot-029` 토큰 문자열(정본),
//! 2. **Python(hashlib/hmac)** 으로 qa 가 따로 계산한 값,
//! 3. 이 크레이트의 Rust 구현.
//!
//! 같은 코드로 만든 기대값을 같은 코드로 확인하면 아무것도 증명하지 못하므로 1·2를 먼저 맞췄다.
//!
//! ```python
//! import hashlib, hmac
//! def subject(label):
//!     h = hashlib.sha256(b"starfall-dev-subject:" + label.encode()).digest()
//!     b = bytearray(h[:16]); b[6] = (b[6] & 0x0F) | 0x70; b[8] = (b[8] & 0x3F) | 0x80
//!     s = b.hex(); return f"{s[0:8]}-{s[8:12]}-{s[12:16]}-{s[16:20]}-{s[20:32]}"
//! def token(secret, subj):
//!     return f"{subj}.{hmac.new(secret.encode(), subj.encode(), hashlib.sha256).hexdigest()}"
//! ```
//!
//! **server 의 `03_server_impl.md` 가 나오면 이 벡터를 서버 구현과 대조한다.** 다르면
//! 서버가 정본이고 이 파일과 `src/token.rs` 를 고친다(리더 지시).

use starfall_bots::token;

const SECRET: &str = "dev_only_not_a_secret"; // .env.example 의 공개 개발값 (사용자 결정 Q4)

// server 정본 (`03_server_impl.md`). Python 으로 독립 재계산해 같음을 확인했다.
const BOT_000_SUBJECT: &str = "01a0b1c2-b010-7000-8000-000000000000";
const BOT_029_SUBJECT: &str = "01a0b1c2-b010-7000-8000-000000000029";
const BOT_000_TOKEN: &str = "01a0b1c2-b010-7000-8000-000000000000.f0561d67b8c71be38992925b16251a890c508770f48f47a63eb51ebf5a880fc6";
const BOT_029_TOKEN: &str = "01a0b1c2-b010-7000-8000-000000000029.c93a0e52ad2926a417dff00599b0dfc818bef26c343e24b34e4daad597dc3d71";

/// 주체 공식이 server 정본과 같은가. 다르면 서버가 401 을 주고 **부하가 0 연결로 끝난다**.
#[test]
fn subject_matches_server_definition() {
    assert_eq!(token::subject_for("bot-000"), BOT_000_SUBJECT);
    assert_eq!(token::subject_for("bot-029"), BOT_029_SUBJECT);
    // 접두사가 고정이고 뒤 12자리가 10진 번호다.
    assert_eq!(
        token::subject_for("bot-007"),
        "01a0b1c2-b010-7000-8000-000000000007"
    );
}

/// 서명이 server 기준값과 **문자열로** 같은가 (HMAC 입력 = 주체 UUID 의 ASCII 문자열).
#[test]
fn token_matches_server_reference_vectors() {
    let (_s0, t0) = token::identity(SECRET, "bot-000");
    let (_s29, t29) = token::identity(SECRET, "bot-029");
    assert_eq!(t0, BOT_000_TOKEN);
    assert_eq!(t29, BOT_029_TOKEN);
}

/// `bot-NNN` 파싱 — 판정에 쓰는 신원과 그 외를 구분한다.
#[test]
fn bot_index_parses_only_the_canonical_form() {
    assert_eq!(token::bot_index("bot-000"), Some(0));
    assert_eq!(token::bot_index("bot-029"), Some(29));
    assert_eq!(token::bot_index("bot-29"), None);
    assert_eq!(token::bot_index("probe-000"), None);
    assert_eq!(token::bot_index("bot-abc"), None);
}

/// 서버는 주체가 UUIDv7 이 아니면 401 을 준다(AC-5b). 30개 전부 계약 패턴을 만족해야 한다.
#[test]
fn all_thirty_subjects_are_valid_uuid_v7_and_distinct() {
    let mut seen = std::collections::BTreeSet::new();
    for i in 0..30 {
        let label = token::bot_label(i);
        let subject = token::subject_for(&label);
        assert!(
            token::is_uuid_v7(&subject),
            "{label} → {subject} 가 계약의 UuidV7 패턴이 아니다"
        );
        assert!(seen.insert(subject.clone()), "주체가 겹친다: {subject}");
    }
    assert_eq!(seen.len(), 30);
}

#[test]
fn bot_labels_are_zero_padded() {
    assert_eq!(token::bot_label(0), "bot-000");
    assert_eq!(token::bot_label(9), "bot-009");
    assert_eq!(token::bot_label(29), "bot-029");
}

#[test]
fn identity_is_deterministic_across_calls() {
    let a = token::identity(SECRET, "bot-007");
    let b = token::identity(SECRET, "bot-007");
    assert_eq!(a.0, b.0);
    assert_eq!(a.1, b.1);
}

/// 비밀이 다르면 토큰이 다르다 — 서명이 실제로 비밀에 의존하는가.
#[test]
fn token_depends_on_secret() {
    let (_s, t1) = token::identity(SECRET, "bot-000");
    let (_s2, t2) = token::identity("some-other-secret", "bot-000");
    assert_ne!(t1, t2);
    // 주체는 비밀과 무관하다(신원은 그대로, 서명만 달라진다).
    assert_eq!(token::subject_for("bot-000"), BOT_000_SUBJECT);
}

#[test]
fn uuid_v7_checker_rejects_the_shapes_the_server_must_reject() {
    assert!(token::is_uuid_v7(BOT_000_SUBJECT));
    // 버전 4
    assert!(!token::is_uuid_v7("0560757c-0a74-49e9-901b-418d06fdbe15"));
    // 변이 비트가 틀림 (c)
    assert!(!token::is_uuid_v7("0560757c-0a74-79e9-c01b-418d06fdbe15"));
    // 대문자
    assert!(!token::is_uuid_v7("0560757C-0A74-79E9-901B-418D06FDBE15"));
    // 길이
    assert!(!token::is_uuid_v7("0560757c-0a74-79e9-901b-418d06fdbe1"));
    assert!(!token::is_uuid_v7("not-a-uuid"));
}

/// client 가 확정한 Unity 주체(02_client_ack.md §2)가 봇 30개와 **겹치지 않는가**.
/// 겹치면 §0.6 의 집합 대조에서 31번째 세션을 봇의 것과 구분할 수 없다.
#[test]
fn unity_subject_is_valid_and_disjoint_from_bot_subjects() {
    const UNITY_SUBJECT: &str = "01a0b1c2-7e57-7c11-8e57-000000000001";
    assert!(
        token::is_uuid_v7(UNITY_SUBJECT),
        "client 가 고른 주체가 계약의 UuidV7 패턴이 아니다 — 서버가 401 을 준다"
    );
    for i in 0..30 {
        assert_ne!(
            UNITY_SUBJECT,
            token::subject_for(&token::bot_label(i)),
            "bot-{i:03} 와 Unity 주체가 겹친다"
        );
    }
    // 두 주체가 같은 접두사 계열이라 눈으로는 헷갈린다. 구분되는 지점을 고정해 둔다.
    assert!(UNITY_SUBJECT.starts_with("01a0b1c2-7e57-"));
    assert!(token::subject_for("bot-000").starts_with(token::BOT_SUBJECT_PREFIX));
}

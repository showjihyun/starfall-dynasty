//! 개발용 토큰 (ADR-0008 §1).
//!
//! ```text
//! token = "<subject_uuid_v7>.<hex(HMAC_SHA256(key = STARFALL_DEV_AUTH_SECRET, msg = subject_uuid_v7))>"
//! ```
//!
//! **server 정본에 정렬됨 (2026-09-19, `03_server_impl.md`).**
//!
//! 1. **HMAC 메시지**: 주체 UUID 의 **정규 소문자 하이픈 36자 ASCII 문자열**(16바이트 이진이 아니다).
//!    키는 비밀의 UTF-8 바이트, 서명은 소문자 hex 64자. — QA 의 최초 가정과 같았다(변경 없음).
//! 2. **`bot-NNN` → 주체**: **`01a0b1c2-b010-7000-8000-000000000NNN`** (NNN = 000..029).
//!    QA 는 원래 sha256 파생을 썼으나 server 정의가 정본이므로 교체했다. 서버가 준 기준값
//!    (`bot-000`·`bot-029`)을 `tests/token_vectors.rs` 가 고정한다.
//!
//! 기대값은 Rust 구현이 아니라 **Python(hashlib/hmac)으로 독립 계산**해 대조했다(2026-09-19).

use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

pub const SUBJECT_DOMAIN: &str = "starfall-dev-subject:";

/// server 가 정한 봇 주체의 고정 접두사 (`03_server_impl.md`).
pub const BOT_SUBJECT_PREFIX: &str = "01a0b1c2-b010-7000-8000-";

/// 라벨 → 주체 UUIDv7 문자열.
///
/// `bot-NNN` 은 **server 정본 공식**을 쓴다. 그 외 라벨(probe 등)은 QA 로컬 파생이다 —
/// 서버는 서명만 검증하므로 유효한 v7 이면 받지만, **판정에 쓰는 신원은 언제나 `bot-NNN`**이다.
pub fn subject_for(label: &str) -> String {
    if let Some(n) = bot_index(label) {
        return format!("{BOT_SUBJECT_PREFIX}{n:012}");
    }
    subject_derived(label)
}

/// `bot-007` → `Some(7)`. 그 외는 `None`.
pub fn bot_index(label: &str) -> Option<u32> {
    let rest = label.strip_prefix("bot-")?;
    if rest.len() != 3 || !rest.bytes().all(|c| c.is_ascii_digit()) {
        return None;
    }
    rest.parse::<u32>().ok()
}

/// `bot-NNN` 이 아닌 라벨용 결정적 파생(QA 로컬). 판정 대상 신원이 아니다.
fn subject_derived(label: &str) -> String {
    use sha2::Digest;
    let mut hasher = Sha256::new();
    hasher.update(SUBJECT_DOMAIN.as_bytes());
    hasher.update(label.as_bytes());
    let digest = hasher.finalize();

    let mut b = [0u8; 16];
    b.copy_from_slice(&digest[..16]);
    b[6] = (b[6] & 0x0F) | 0x70; // version 7
    b[8] = (b[8] & 0x3F) | 0x80; // variant 10xx

    let h = hex::encode(b);
    format!(
        "{}-{}-{}-{}-{}",
        &h[0..8],
        &h[8..12],
        &h[12..16],
        &h[16..20],
        &h[20..32]
    )
}

/// `bot-000` … `bot-{n-1}`
pub fn bot_label(index: usize) -> String {
    format!("bot-{index:03}")
}

/// 주체와 비밀에서 토큰을 만든다.
pub fn token_for_subject(secret: &str, subject: &str) -> String {
    let mut mac =
        HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC accepts keys of any length");
    mac.update(subject.as_bytes());
    let tag = mac.finalize().into_bytes();
    format!("{subject}.{}", hex::encode(tag))
}

/// 라벨에서 바로 `(subject, token)`.
pub fn identity(secret: &str, label: &str) -> (String, String) {
    let subject = subject_for(label);
    let token = token_for_subject(secret, &subject);
    (subject, token)
}

/// 계약의 `UuidV7` 패턴을 정규식 없이 확인한다(의존성을 늘리지 않기 위해).
/// `^[0-9a-f]{8}-[0-9a-f]{4}-7[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$`
pub fn is_uuid_v7(s: &str) -> bool {
    let groups: Vec<&str> = s.split('-').collect();
    if groups.len() != 5 {
        return false;
    }
    let lens = [8usize, 4, 4, 4, 12];
    for (g, want) in groups.iter().zip(lens.iter()) {
        if g.len() != *want
            || !g
                .bytes()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        {
            return false;
        }
    }
    groups[2].starts_with('7') && matches!(groups[3].as_bytes()[0], b'8' | b'9' | b'a' | b'b')
}

/// `STARFALL_DEV_AUTH_SECRET` 를 읽는다. 없으면 실행을 멈춘다 —
/// **기본값을 코드에 두지 않는다**(ADR-0008 §3).
pub fn secret_from_env() -> Result<String, String> {
    match std::env::var("STARFALL_DEV_AUTH_SECRET") {
        Ok(s) if !s.is_empty() => Ok(s),
        _ => Err("STARFALL_DEV_AUTH_SECRET 가 비어 있거나 없다. \
                  `.env.example` 의 값을 셸에 로드해라 (기본값은 코드에 두지 않는다 — ADR-0008 §3)."
            .to_owned()),
    }
}

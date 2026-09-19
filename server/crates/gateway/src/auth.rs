//! 개발용 인증 토큰 (ADR-0008).
//!
//! ```text
//! token = "<subject_uuid_v7>.<hex(HMAC_SHA256(key = STARFALL_DEV_AUTH_SECRET, msg = subject_uuid_v7))>"
//! ```
//!
//! # 이 토큰이 하지 **않는** 것
//!
//! 만료·폐기·키 회전·계정·레이트 리밋·비대칭 키·전송 암호화·권한이 전부 없다 (ADR-0008 §4).
//! 이 토큰을 신뢰할 수 있게 만드는 것은 암호학이 아니라 **배포 경계**다: 서버가 `127.0.0.1`
//! 에만 바인딩되고 비밀이 개발 머신에만 있다. 그 전제가 깨지는 순간(LAN 노출·공유 개발
//! 서버·스테이징) 이 토큰은 즉시 부적절해진다.
//!
//! # 지금 성립하는 성질 (스파이크가 실제로 증명하는 것)
//!
//! 1. **행위자는 서버가 정한다** — `actor_id` 는 검증된 토큰의 주체에서만 나온다 (I-10).
//! 2. **인증되지 않으면 세션이 없다** — 401 이면 소켓이 열리지 않는다 (I-24).
//! 3. 클라이언트는 자기 신원을 `SESSION_READY` 로 **통보받는다**.
//! 4. 인증 실패는 도메인 이벤트가 아니다 (ADR-0007 §1).
//!
//! 진짜 인증으로 바꿀 때 바뀌는 것은 [`extract_credential`] 과 [`DevAuth::verify`] 뿐이고,
//! 위 네 성질은 그대로다.

use axum::http::{HeaderMap, header};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use starfall_contracts::UuidV7;
use subtle::ConstantTimeEq;

type HmacSha256 = Hmac<Sha256>;

/// 개발용 토큰 검증기.
///
/// **기본 비밀값을 코드에 두지 않는다** (ADR-0008 §3). 기본값이 있으면 그 값이 배포까지 간다.
#[derive(Clone)]
pub enum DevAuth {
    /// `STARFALL_DEV_AUTH_SECRET` 이 설정됐다.
    Enabled {
        /// HMAC 키.
        secret: Vec<u8>,
    },
    /// 비밀이 없다. 프로세스는 살고 `/ws` 만 503 이다.
    Disabled,
}

impl std::fmt::Debug for DevAuth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // 비밀이 로그·패닉 메시지로 새지 않게 직접 구현한다.
        match self {
            Self::Enabled { .. } => f.write_str("DevAuth::Enabled"),
            Self::Disabled => f.write_str("DevAuth::Disabled"),
        }
    }
}

impl DevAuth {
    /// 환경 값에서 만든다. 비어 있거나 없으면 [`DevAuth::Disabled`].
    #[must_use]
    pub fn from_secret(secret: Option<String>) -> Self {
        match secret {
            Some(value) if !value.is_empty() => Self::Enabled {
                secret: value.into_bytes(),
            },
            _ => Self::Disabled,
        }
    }

    /// 인증 표면이 켜져 있는가.
    #[must_use]
    pub const fn is_enabled(&self) -> bool {
        matches!(self, Self::Enabled { .. })
    }

    /// 이 주체에 대한 토큰을 만든다. **테스트와 도구용**이다 — 서버는 발급하지 않는다.
    #[must_use]
    pub fn mint(&self, subject: UuidV7) -> Option<String> {
        let Self::Enabled { secret } = self else {
            return None;
        };
        Some(format!(
            "{subject}.{}",
            hex(&sign(secret, &subject.to_string()))
        ))
    }

    /// 토큰을 검증하고 행위자 id 를 돌려준다.
    ///
    /// 실패 이유를 구분해 돌려주지 않는다 — 호출자는 401 하나만 낸다. 구분은 로그에 남긴다.
    #[must_use]
    pub fn verify(&self, token: &str) -> Option<UuidV7> {
        let Self::Enabled { secret } = self else {
            return None;
        };
        let (subject, signature) = token.split_once('.')?;
        // 주체는 **정규 표기 UUIDv7 이어야 한다.** serde 경로와 같은 규칙이라
        // "헤더로는 통과하고 계약으로는 거부되는" 관용이 생기지 않는다.
        let actor_id = UuidV7::parse(subject)?;
        let expected = sign(secret, subject);
        let actual = unhex(signature)?;
        // 상수 시간 비교 (ADR-0008 §1).
        (actual.len() == expected.len() && bool::from(actual.ct_eq(&expected))).then_some(actor_id)
    }
}

fn sign(secret: &[u8], message: &str) -> Vec<u8> {
    let mut mac = <HmacSha256 as Mac>::new_from_slice(secret)
        .unwrap_or_else(|_| unreachable!("HMAC-SHA256 은 임의 길이 키를 받는다"));
    mac.update(message.as_bytes());
    mac.finalize().into_bytes().to_vec()
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes
        .iter()
        .fold(String::with_capacity(bytes.len() * 2), |mut out, byte| {
            let _ = write!(out, "{byte:02x}");
            out
        })
}

fn unhex(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) {
        return None;
    }
    text.as_bytes()
        .chunks(2)
        .map(|pair| {
            let high = (pair[0] as char).to_digit(16)?;
            let low = (pair[1] as char).to_digit(16)?;
            u8::try_from(high * 16 + low).ok()
        })
        .collect()
}

/// 요청에서 자격 증명을 꺼내는 **유일한 함수** (ADR-0008 §2).
///
/// WebGL(브라우저 WebSocket 은 임의 헤더를 못 넣는다)과 진짜 인증이 바꿀 지점이 여기 하나뿐이도록
/// 모아 둔다 (ADR-0005 §6). 쿼리 문자열은 쓰지 않는다 — 접근 로그·리퍼러·프록시에 남는다.
#[must_use]
pub fn extract_credential(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
        .map(str::trim)
        .filter(|token| !token.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    const SUBJECT: &str = "01a0b1c2-2c01-7a45-8b67-89abcdef0123";

    fn auth() -> DevAuth {
        DevAuth::from_secret(Some("dev_only_not_a_secret".to_owned()))
    }

    #[test]
    fn mint_then_verify_round_trips() {
        let auth = auth();
        let subject = UuidV7::parse(SUBJECT).unwrap();
        let token = auth.mint(subject).unwrap();
        assert_eq!(auth.verify(&token), Some(subject));
    }

    #[test]
    fn token_shape_matches_adr_0008() {
        // QA 봇과 Unity 가 같은 규칙으로 토큰을 만든다. 이 값이 그 규칙의 고정점이다.
        let token = auth().mint(UuidV7::parse(SUBJECT).unwrap()).unwrap();
        let (subject, signature) = token.split_once('.').unwrap();
        assert_eq!(subject, SUBJECT);
        assert_eq!(signature.len(), 64, "HMAC-SHA256 hex = 64자");
        assert!(
            signature
                .chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        );
        // 독립 구현(python hmac)과 같은 값이어야 한다. QA 봇·Unity 가 이 벡터로 자기
        // 구현을 맞춘다 — `03_server_impl.md` 의 토큰 생성 절차에 같은 값이 적혀 있다.
        assert_eq!(
            signature, "5349d15b96d416cf11360384089d7a5d3d47d172575738caf40748d9bad190bf",
            "토큰 형식이 바뀌면 봇·클라이언트가 전부 깨진다"
        );
    }

    /// QA 봇 30개의 주체 파생 규칙을 **코드로 고정한다**.
    ///
    /// 서버는 "정규 소문자 UUIDv7 + 유효 HMAC"만 요구하고 주체를 어떻게 만드는지는 상관하지
    /// 않는다. 하지만 봇 30개가 서로 겹치지 않고 재실행마다 같은 신원을 쓰려면 규칙이
    /// 하나여야 하므로, **server 가 정본을 정하고 여기서 고정한다**:
    ///
    /// ```text
    /// subject(bot-NNN) = 01a0b1c2-b010-7000-8000-000000000NNN     (NNN = 000..029, 10진 3자리)
    /// token(bot-NNN)   = subject + "." + hex(HMAC_SHA256(secret, subject))
    /// ```
    ///
    /// Unity(`01a0b1c2-7e57-7c11-8e57-000000000001`)·라이브 스모크
    /// (`01a0b1c2-5a11-7c01-8d01-00000000000*`)와 겹치지 않는다.
    #[test]
    fn bot_subjects_follow_the_documented_rule() {
        let auth = auth();
        let mut seen = std::collections::BTreeSet::new();
        for index in 0..30u32 {
            let subject = format!("01a0b1c2-b010-7000-8000-000000000{index:03}");
            let parsed = UuidV7::parse(&subject).unwrap_or_else(|| {
                panic!("bot-{index:03} 주체가 정규 UUIDv7 이 아니다: {subject}")
            });
            let token = auth.mint(parsed).expect("토큰 발급");
            assert_eq!(auth.verify(&token), Some(parsed), "bot-{index:03}");
            assert!(seen.insert(subject.clone()), "주체가 겹친다: {subject}");
            if index == 0 || index == 29 {
                println!("bot-{index:03} token = {token}");
            }
        }
        assert_eq!(seen.len(), 30);
        // 다른 역할의 주체와 겹치지 않는다.
        for other in [
            "01a0b1c2-7e57-7c11-8e57-000000000001", // Unity 클라이언트
            "01a0b1c2-5a11-7c01-8d01-000000000001", // 라이브 스모크
            "01a0b1c2-5a11-7c01-8d01-000000000002", // 라이브 종료 스모크
        ] {
            assert!(UuidV7::parse(other).is_some(), "{other}");
            assert!(!seen.contains(other), "봇 주체와 겹친다: {other}");
        }
    }

    #[test]
    fn verify_rejects_the_three_ac5b_cases() {
        let auth = auth();
        // (b-1) 헤더 없음은 extract_credential 이 None 을 준다.
        assert!(extract_credential(&HeaderMap::new()).is_none());
        // (b-2) 서명 불일치.
        assert!(
            auth.verify(&format!("{SUBJECT}.{}", "0".repeat(64)))
                .is_none()
        );
        // (b-3) 주체가 UUIDv7 이 아니다 (v4).
        let v4 = "a013f529-f8a4-4c84-be34-a740670f9fb8";
        let token = format!("{v4}.{}", hex(&sign(b"dev_only_not_a_secret", v4)));
        assert!(
            auth.verify(&token).is_none(),
            "서명이 맞아도 v7 이 아니면 거부한다"
        );
    }

    #[test]
    fn verify_rejects_non_canonical_subject_spelling() {
        let auth = auth();
        let upper = SUBJECT.to_uppercase();
        let token = format!("{upper}.{}", hex(&sign(b"dev_only_not_a_secret", &upper)));
        assert!(auth.verify(&token).is_none());
    }

    #[test]
    fn disabled_auth_never_verifies() {
        let auth = DevAuth::from_secret(None);
        assert!(!auth.is_enabled());
        assert!(auth.mint(UuidV7::parse(SUBJECT).unwrap()).is_none());
        assert!(auth.verify("anything").is_none());
        assert!(
            DevAuth::from_secret(Some(String::new()))
                .is_enabled()
                .eq(&false)
        );
    }

    #[test]
    fn extract_credential_only_accepts_bearer() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::AUTHORIZATION,
            HeaderValue::from_static("Bearer abc"),
        );
        assert_eq!(extract_credential(&headers), Some("abc"));

        headers.insert(header::AUTHORIZATION, HeaderValue::from_static("Basic abc"));
        assert_eq!(extract_credential(&headers), None);

        headers.insert(header::AUTHORIZATION, HeaderValue::from_static("Bearer "));
        assert_eq!(extract_credential(&headers), None);
    }
}

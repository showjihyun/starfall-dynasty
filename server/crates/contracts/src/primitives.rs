//! `contracts/common/primitives.schema.json` 에 대응하는 값 타입.
//!
//! # 왜 전부 newtype 인가
//!
//! 계약의 정수·문자열은 "범위가 곧 의미"다. `u64` 나 `String` 을 그대로 쓰면 스키마가
//! 거부하는 값을 **서버는 받아들인다.** 운영 중 명령을 막는 것은 스키마 검증기가 아니라
//! serde 이므로(ADR-0002 §3 테스트 6), 제약은 타입에 들어가야 한다.
//!
//! # 재직렬화 왕복이 깨지지 않게 하는 규칙 (ADR-0002 §3)
//!
//! - `RealTime`·`GameTime` 은 **문자열 newtype** 이다. `chrono::DateTime` 은 재직렬화에서
//!   소수 자리를 0/3/6/9 로 정규화해(`.1` -> `.100`) 1~9자리를 허용하는 계약 패턴과 왕복이 깨진다.
//! - 널 가능 envelope 필드에 `skip_serializing_if` 를 쓰지 않는다. 붙이면 `client_sent_at: null`
//!   이 재직렬화에서 사라지고 envelope 규칙(I-5)이 깨진다. 이 모듈의 [`required_nullable`] 참고.

use std::fmt;

use serde::de::{Error as DeError, Unexpected};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use uuid::Uuid;

/// IEEE-754 가 정확히 표현하는 정수의 상한 (2^53 - 1).
///
/// 계약의 모든 정수는 이 범위 안이다. JS·Python 등 어떤 소비자도 값을 정확히 표현할 수 있어야
/// 로그·대시보드·분석 도구에서 값이 조용히 바뀌지 않는다 (ADR-0002 §5).
pub const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

// ---------------------------------------------------------------------------
// UuidV7
// ---------------------------------------------------------------------------

/// 정규 소문자 하이픈 표기의 UUIDv7 (RFC 9562).
///
/// 역직렬화는 다음을 모두 만족해야 통과한다.
///
/// 1. UUID 로 파싱된다
/// 2. 버전 니블이 **7** 이다 — `uuid` 크레이트의 기본 역직렬화는 v4 도 그대로 받는다.
///    그래서 `command-id-not-v7` 반례가 스키마에서만 막히고 serde 에서는 통과하는 구멍이 생긴다.
/// 3. 입력 문자열이 **정규 소문자 하이픈 표기**와 글자 그대로 같다 — `uuid` 는 대문자·중괄호
///    표기도 받아 주므로, 그대로 두면 재직렬화가 입력과 달라져 왕복 테스트가 깨진다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UuidV7(Uuid);

impl UuidV7 {
    /// 새 UUIDv7 을 만든다.
    ///
    /// 결정적 코어(sim·history) 안에서는 이것을 직접 부르지 않는다. ID 는 주입된 생성기를
    /// 통해서만 얻는다 (ADR-0002 §5).
    #[must_use]
    pub fn new_v7() -> Self {
        Self(Uuid::now_v7())
    }

    /// 내부 UUID.
    #[must_use]
    pub const fn get(self) -> Uuid {
        self.0
    }

    /// UUID 가 v7 이면 감싼다.
    #[must_use]
    pub fn from_uuid(uuid: Uuid) -> Option<Self> {
        (uuid.get_version_num() == 7).then_some(Self(uuid))
    }
}

impl fmt::Display for UuidV7 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.as_hyphenated())
    }
}

impl Serialize for UuidV7 {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        // `Hyphenated` 의 Display 는 소문자 하이픈 표기다 = 계약 패턴.
        serializer.collect_str(self.0.as_hyphenated())
    }
}

impl<'de> Deserialize<'de> for UuidV7 {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        let uuid = Uuid::parse_str(&raw)
            .map_err(|_| D::Error::invalid_value(Unexpected::Str(&raw), &"UUID"))?;

        if uuid.get_version_num() != 7 {
            return Err(D::Error::custom(format!(
                "UuidV7 는 버전 7이어야 한다 (받은 값의 버전: {}, 값: {raw})",
                uuid.get_version_num()
            )));
        }

        let mut buf = [0u8; uuid::fmt::Hyphenated::LENGTH];
        let canonical: &str = uuid.as_hyphenated().encode_lower(&mut buf);
        if raw != canonical {
            return Err(D::Error::custom(format!(
                "UuidV7 는 정규 소문자 하이픈 표기여야 한다 (기대: {canonical}, 받음: {raw})"
            )));
        }

        Ok(Self(uuid))
    }
}

// ---------------------------------------------------------------------------
// 타임스탬프 문자열
// ---------------------------------------------------------------------------

/// `####-##-##T##:##:##` 모양 검사용 마스크. `#` 은 ASCII 숫자 한 자리.
const TIMESTAMP_SHAPE: &[u8; 19] = b"####-##-##T##:##:##";

/// 앞 19바이트가 `YYYY-MM-DDTHH:MM:SS` 모양인지, 그리고 뒤가 `Z` 또는 `.<1~9자리>Z` 인지 본다.
///
/// 달력상 실재하는 날짜인지는 보지 않는다. 계약 스키마도 `pattern` 으로만 제약하므로
/// 검증 범위를 스키마와 정확히 같게 맞춘다 — 한쪽이 더 엄격하면 왕복 테스트가 엉뚱한 곳에서 깨진다.
fn is_valid_timestamp(value: &str, allow_fraction: bool) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() < 20 {
        return false;
    }

    let shape_ok = bytes[..19]
        .iter()
        .zip(TIMESTAMP_SHAPE.iter())
        .all(|(actual, expected)| match *expected {
            b'#' => actual.is_ascii_digit(),
            other => *actual == other,
        });
    if !shape_ok {
        return false;
    }

    let rest = &bytes[19..];
    if rest == b"Z" {
        return true;
    }
    if !allow_fraction {
        return false;
    }
    // `.` + 1~9자리 + `Z`
    if rest.first() != Some(&b'.') || rest.last() != Some(&b'Z') {
        return false;
    }
    let fraction = &rest[1..rest.len() - 1];
    (1..=9).contains(&fraction.len()) && fraction.iter().all(u8::is_ascii_digit)
}

/// 실제 시간(UTC). RFC 3339 프로파일, `Z` 필수, 소수 1~9자리 선택.
///
/// **감사·진단 전용이다.** 게임 규칙에 쓰지 않는다. 순서·판정은 `tick` 으로만 한다.
///
/// 시간 타입이 아니라 문자열인 이유는 모듈 문서 참고 — 소수 자리 정규화로 왕복이 깨진다.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RealTime(String);

/// 게임 달력 시각. tick 에서 결정적으로 파생한다.
///
/// **실제 시간이 아니다.** 파싱하거나 정렬에 쓰지 않는다 (ADR-0002 §5). 표시·역사 문장 전용.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GameTime(String);

macro_rules! timestamp_newtype {
    ($name:ident, $fraction:expr, $label:literal) => {
        impl $name {
            /// 검증을 통과하면 감싼다.
            #[must_use]
            pub fn parse(value: impl Into<String>) -> Option<Self> {
                let value = value.into();
                is_valid_timestamp(&value, $fraction).then_some(Self(value))
            }

            /// 원본 문자열.
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl Serialize for $name {
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.serialize_str(&self.0)
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let raw = String::deserialize(deserializer)?;
                Self::parse(raw.clone())
                    .ok_or_else(|| D::Error::custom(format!("{} 형식이 아니다: {raw}", $label)))
            }
        }
    };
}

timestamp_newtype!(RealTime, true, "RealTime(RFC 3339 UTC, Z 필수)");
timestamp_newtype!(GameTime, false, "GameTime(게임 달력, 소수 자리 없음)");

// ---------------------------------------------------------------------------
// 정수
// ---------------------------------------------------------------------------

/// 시뮬레이션 tick 번호. `0 ..= 2^53-1`.
///
/// `u64` 를 그대로 쓰면 상한(2^53-1)을 넘는 값이 통과한다. 그래서 범위 검증 newtype 이다
/// (`tick-above-safe-integer` 반례가 이것을 본다).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Tick(u64);

/// 같은 tick 안의 결정적 순서. `0 ..= 2^53-1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Sequence(u64);

macro_rules! safe_u64_newtype {
    ($name:ident, $label:literal) => {
        impl $name {
            /// 범위 안이면 감싼다.
            #[must_use]
            pub const fn new(value: u64) -> Option<Self> {
                if value <= MAX_SAFE_INTEGER {
                    Some(Self(value))
                } else {
                    None
                }
            }

            /// 내부 값.
            #[must_use]
            pub const fn get(self) -> u64 {
                self.0
            }
        }

        impl Serialize for $name {
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.serialize_u64(self.0)
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let raw = u64::deserialize(deserializer)?;
                Self::new(raw).ok_or_else(|| {
                    D::Error::custom(format!(
                        "{} 는 0 ..= {MAX_SAFE_INTEGER} 범위여야 한다 (받음: {raw})",
                        $label
                    ))
                })
            }
        }
    };
}

safe_u64_newtype!(Tick, "Tick");
safe_u64_newtype!(Sequence, "Sequence");

/// 클라이언트가 고른 프로브 카운터. 계약 범위 `0 ..= 4294967295` = `u32` 와 정확히 같다.
///
/// `u32` 가 범위를 정확히 덮으므로 별도 검증이 필요 없다. 음수(`probe-seq-negative`)와
/// 상한 초과(`probe-seq-above-u32`)는 `u32` 역직렬화가 그대로 거부한다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProbeSeq(pub u32);

/// payload 스키마 버전이 컴파일 타임 상수 `V` 와 같은지 검증하는 필드 타입.
///
/// 계약 스키마의 `"schema_version": {"const": 1}` 에 대응한다. 직렬화하면 `V` 가 그대로 나가고,
/// 역직렬화는 `V` 가 아닌 값을 거부한다. 버전이 올라가면 타입 파라미터만 바꾸면 되고,
/// 구버전 문서가 새 타입으로 조용히 들어오는 일이 없다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ConstSchemaVersion<const V: u32>;

impl<const V: u32> Serialize for ConstSchemaVersion<V> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u32(V)
    }
}

impl<'de, const V: u32> Deserialize<'de> for ConstSchemaVersion<V> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = u32::deserialize(deserializer)?;
        if raw == V {
            Ok(Self)
        } else {
            Err(D::Error::custom(format!(
                "schema_version 은 {V} 여야 한다 (받음: {raw})"
            )))
        }
    }
}

// ---------------------------------------------------------------------------
// 널 가능 envelope 필드
// ---------------------------------------------------------------------------

/// **키는 항상 있고 값만 null 일 수 있는** envelope 필드용 역직렬화기 (I-5).
///
/// # 왜 필요한가
///
/// serde 는 `Option<T>` 필드가 입력에 **없으면** 조용히 `None` 으로 채운다. 그러면
/// `client_sent_at` 키를 통째로 빼먹은 문서가 통과해 "필드 없음"과 "null" 이 구분되지 않는다.
/// `deserialize_with` 를 달면 serde 가 그 지름길을 쓰지 못하고 키가 없을 때 `missing_field`
/// 오류를 낸다. `required` 변이 테스트(ADR-0002 §3 테스트 7)가 이 동작을 검증한다.
///
/// 직렬화에는 **아무 속성도 달지 않는다.** `skip_serializing_if` 를 붙이면 `null` 이 사라진다.
///
/// # Errors
///
/// 값이 `null` 도 `T` 도 아니면 실패한다.
pub fn required_nullable<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uuid_v7_accepts_canonical_lowercase() {
        let value: UuidV7 =
            serde_json::from_str("\"01a0afaf-7e83-7f49-adfa-58ba04c2fd5a\"").unwrap();
        assert_eq!(
            serde_json::to_string(&value).unwrap(),
            "\"01a0afaf-7e83-7f49-adfa-58ba04c2fd5a\""
        );
    }

    #[test]
    fn uuid_v7_rejects_v4() {
        // contracts/fixtures/PING_SERVER/invalid/command-id-not-v7.json 의 값.
        let err =
            serde_json::from_str::<UuidV7>("\"a013f529-f8a4-4c84-be34-a740670f9fb8\"").unwrap_err();
        assert!(err.to_string().contains("버전 7"), "{err}");
    }

    #[test]
    fn uuid_v7_rejects_non_canonical_spelling() {
        // uuid 크레이트 자체는 대문자·중괄호 표기를 받는다. 계약 pattern 은 받지 않는다.
        for raw in [
            "\"01A0AFAF-7E83-7F49-ADFA-58BA04C2FD5A\"",
            "\"{01a0afaf-7e83-7f49-adfa-58ba04c2fd5a}\"",
            "\"01a0afaf7e837f49adfa58ba04c2fd5a\"",
        ] {
            assert!(
                serde_json::from_str::<UuidV7>(raw).is_err(),
                "정규 표기가 아닌 값이 통과했다: {raw}"
            );
        }
    }

    #[test]
    fn real_time_accepts_contract_shapes() {
        for raw in [
            "2026-09-17T14:05:09.123Z",
            "2026-09-17T14:05:09Z",
            "2026-09-17T14:05:09.1Z",
            "2026-09-17T14:05:09.123456789Z",
        ] {
            assert!(RealTime::parse(raw).is_some(), "거부됐다: {raw}");
        }
    }

    #[test]
    fn real_time_rejects_out_of_contract_shapes() {
        for raw in [
            "2026-09-17T14:05:09",             // Z 없음
            "2026-09-17T14:05:09+09:00",       // 오프셋 표기
            "2026-09-17T14:05:09.1234567890Z", // 소수 10자리
            "2026-09-17 14:05:09Z",            // 공백 구분자
            "26-09-17T14:05:09Z",              // 연도 2자리
        ] {
            assert!(RealTime::parse(raw).is_none(), "통과했다: {raw}");
        }
    }

    #[test]
    fn game_time_rejects_fractional_seconds() {
        assert!(GameTime::parse("3827-04-13T18:32:11Z").is_some());
        assert!(GameTime::parse("3827-04-13T18:32:11.5Z").is_none());
    }

    #[test]
    fn real_time_round_trips_without_normalising_fraction() {
        // chrono 를 썼다면 ".1" 이 ".100" 으로 정규화되어 이 단언이 깨진다.
        let original = "\"2026-09-17T14:05:09.1Z\"";
        let value: RealTime = serde_json::from_str(original).unwrap();
        assert_eq!(serde_json::to_string(&value).unwrap(), original);
    }

    #[test]
    fn tick_rejects_above_safe_integer() {
        assert!(serde_json::from_str::<Tick>("9007199254740991").is_ok());
        let err = serde_json::from_str::<Tick>("9007199254740992").unwrap_err();
        assert!(err.to_string().contains("범위"), "{err}");
    }

    #[test]
    fn probe_seq_rejects_out_of_u32_range() {
        assert!(serde_json::from_str::<ProbeSeq>("4294967295").is_ok());
        assert!(serde_json::from_str::<ProbeSeq>("4294967296").is_err());
        assert!(serde_json::from_str::<ProbeSeq>("-1").is_err());
    }

    #[test]
    fn const_schema_version_rejects_other_values() {
        assert!(serde_json::from_str::<ConstSchemaVersion<1>>("1").is_ok());
        assert!(serde_json::from_str::<ConstSchemaVersion<1>>("2").is_err());
    }
}

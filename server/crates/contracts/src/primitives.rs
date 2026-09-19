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

    /// 정규 소문자 하이픈 표기의 UUIDv7 문자열을 감싼다.
    ///
    /// 역직렬화와 **같은 규칙**이다(v7 검사 + 정규 표기 검사). 그래서 토큰 주체처럼
    /// 바깥에서 들어온 문자열을 받을 때 serde 경로와 다른 관용이 생기지 않는다.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        let uuid = Uuid::parse_str(value).ok()?;
        if uuid.get_version_num() != 7 {
            return None;
        }
        let mut buf = [0u8; uuid::fmt::Hyphenated::LENGTH];
        (uuid.as_hyphenated().encode_lower(&mut buf) == value).then_some(Self(uuid))
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
// 달력 산술 (의존성 없음)
// ---------------------------------------------------------------------------
//
// `chrono` 를 쓰지 않는 이유는 두 가지다.
//
// 1. `starfall-sim` 은 의존성 0 을 유지해야 한다 (AC-1 / SC-04 가 Cargo.toml 로 검증한다).
//    `occurred_at` 파생은 sim 의 핵심 계산이므로 여기 있어야 하고, 여기 있으려면 의존성이
//    없어야 한다.
// 2. 같은 헬퍼가 `recorded_at`(RealTime) 포맷에도 쓰인다. 시계 출처가 하나가 된다.
//
// 알고리즘은 Howard Hinnant 의 `days_from_civil` / `civil_from_days` (proleptic Gregorian,
// 1970-01-01 = day 0). 정수 연산만 쓰므로 결정적이다.

/// 하루의 초 수.
const SECS_PER_DAY: i64 = 86_400;

/// `(year, month, day)` → 1970-01-01 기준 일수.
const fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400; // [0, 399]
    let mp = if month > 2 { month - 3 } else { month + 9 } as i64;
    let doy = (153 * mp + 2) / 5 + day as i64 - 1; // [0, 365]
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy; // [0, 146096]
    era * 146_097 + doe - 719_468
}

/// 1970-01-01 기준 일수 → `(year, month, day)`.
const fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32; // [1, 12]
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// `YYYY-MM-DDTHH:MM:SS` 모양의 앞 19바이트에서 구성요소를 뽑는다.
///
/// 모양 검증은 [`is_valid_timestamp`] 가 이미 했다고 가정한다.
fn components(value: &str) -> Option<(i64, u32, u32, u32, u32, u32)> {
    let b = value.as_bytes();
    if b.len() < 19 {
        return None;
    }
    let num = |from: usize, to: usize| -> i64 {
        let mut acc = 0i64;
        let mut i = from;
        while i < to {
            acc = acc * 10 + i64::from(b[i] - b'0');
            i += 1;
        }
        acc
    };
    Some((
        num(0, 4),
        num(5, 7) as u32,
        num(8, 10) as u32,
        num(11, 13) as u32,
        num(14, 16) as u32,
        num(17, 19) as u32,
    ))
}

/// 1970-01-01T00:00:00Z 기준 초 수를 `YYYY-MM-DDTHH:MM:SS` + 접미사로 포맷한다.
fn format_epoch_seconds(total: i64, suffix: &str) -> String {
    let days = total.div_euclid(SECS_PER_DAY);
    let sod = total.rem_euclid(SECS_PER_DAY);
    let (y, m, d) = civil_from_days(days);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}{suffix}",
        sod / 3600,
        (sod % 3600) / 60,
        sod % 60,
    )
}

impl GameTime {
    /// 1970-01-01T00:00:00Z 기준 초 수로 환산한다.
    ///
    /// 게임 달력은 실제 시간축과 무관하지만, **산술을 하려면 원점이 하나 필요하다.**
    /// 그 원점을 유닉스 에포크로 쓰는 것은 내부 표현일 뿐이고 의미를 부여하지 않는다.
    #[must_use]
    pub fn to_epoch_seconds(&self) -> Option<i64> {
        let (y, mo, d, h, mi, s) = components(&self.0)?;
        Some(
            days_from_civil(y, mo, d) * SECS_PER_DAY
                + i64::from(h) * 3600
                + i64::from(mi) * 60
                + i64::from(s),
        )
    }

    /// 1970-01-01T00:00:00Z 기준 초 수에서 `GameTime` 을 만든다.
    ///
    /// 연도가 4자리를 넘으면(`>= 10000`) 계약 패턴을 만족할 수 없으므로 `None`.
    #[must_use]
    pub fn from_epoch_seconds(total: i64) -> Option<Self> {
        let (y, _, _) = civil_from_days(total.div_euclid(SECS_PER_DAY));
        if !(0..10_000).contains(&y) {
            return None;
        }
        Self::parse(format_epoch_seconds(total, "Z"))
    }
}

impl RealTime {
    /// 유닉스 에포크 기준 `(초, 나노초)` 에서 밀리초 3자리 표기의 `RealTime` 을 만든다.
    ///
    /// 계약 패턴은 소수 1~9자리를 허용하지만 **3자리로 고정**한다 — 자리 수가 입력에 따라
    /// 흔들리면 로그·fixture 비교가 매번 달라진다.
    #[must_use]
    pub fn from_unix(seconds: i64, nanos: u32) -> Option<Self> {
        let (y, _, _) = civil_from_days(seconds.div_euclid(SECS_PER_DAY));
        if !(0..10_000).contains(&y) {
            return None;
        }
        let millis = nanos / 1_000_000;
        Self::parse(format_epoch_seconds(seconds, &format!(".{millis:03}Z")))
    }
}

// ---------------------------------------------------------------------------
// 게임 시간 파생 (ADR-0006 §3)
// ---------------------------------------------------------------------------

/// 한 월드의 게임 달력 상수. `worlds` 행에 있고 **월드 수명 동안 불변**이다 (I-19).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameCalendar {
    /// 실제 1초당 tick 수. `1 ..= 1000`.
    tick_hz: u32,
    /// 달력 기준점.
    epoch: GameTime,
    /// 실제 1초 = 게임 몇 초인가.
    scale: u32,
    /// `epoch` 를 미리 환산해 둔 값 (tick 마다 문자열을 파싱하지 않기 위해).
    epoch_seconds: i64,
}

impl GameCalendar {
    /// 상수에서 달력을 만든다.
    ///
    /// `tick_hz` 가 `1..=1000` 밖이거나 `scale` 이 0이면, 또는 `epoch` 를 환산할 수 없으면 `None`.
    #[must_use]
    pub fn new(tick_hz: u32, epoch: GameTime, scale: u32) -> Option<Self> {
        if !(1..=1000).contains(&tick_hz) || scale == 0 {
            return None;
        }
        let epoch_seconds = epoch.to_epoch_seconds()?;
        Some(Self {
            tick_hz,
            epoch,
            scale,
            epoch_seconds,
        })
    }

    /// 이 월드의 tick 주기.
    #[must_use]
    pub const fn tick_hz(&self) -> u32 {
        self.tick_hz
    }

    /// 달력 기준점.
    #[must_use]
    pub const fn epoch(&self) -> &GameTime {
        &self.epoch
    }

    /// 실제 1초당 게임 초.
    #[must_use]
    pub const fn scale(&self) -> u32 {
        self.scale
    }

    /// `tick` → `occurred_at` (ADR-0006 §3).
    ///
    /// ```text
    /// game_seconds = (tick / tick_hz) * calendar_scale     # 정수 나눗셈(내림)
    /// occurred_at  = calendar_epoch + game_seconds
    /// ```
    ///
    /// **`tick_hz` 로 먼저 나눈다.** 순서를 바꾸면 tick 주기를 올렸을 때 게임 달력 속도가
    /// 함께 빨라진다 — 두 축을 분리한 이유가 사라진다.
    #[must_use]
    pub fn occurred_at(&self, tick: Tick) -> Option<GameTime> {
        let game_seconds =
            (tick.get() / u64::from(self.tick_hz)).checked_mul(u64::from(self.scale))?;
        let total = self
            .epoch_seconds
            .checked_add(i64::try_from(game_seconds).ok()?)?;
        GameTime::from_epoch_seconds(total)
    }
}

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

/// 한 월드의 실제 1초당 tick 수. 계약 범위 `1 ..= 1000`.
///
/// `i32`/`u32` 를 그대로 쓰면 `tick-hz-zero` 반례가 serde 를 통과한다(C# 은 실제로 통과한다 —
/// 스펙 §5.4 의 "감지 불가"). Rust 쪽은 여기서 막는다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TickHz(u32);

impl TickHz {
    /// 범위 안이면 감싼다.
    #[must_use]
    pub const fn new(value: u32) -> Option<Self> {
        if 1 <= value && value <= 1000 {
            Some(Self(value))
        } else {
            None
        }
    }

    /// 내부 값.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

impl Serialize for TickHz {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u32(self.0)
    }
}

impl<'de> Deserialize<'de> for TickHz {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = u32::deserialize(deserializer)?;
        Self::new(raw).ok_or_else(|| {
            D::Error::custom(format!("tick_hz 는 1 ..= 1000 이어야 한다 (받음: {raw})"))
        })
    }
}

/// 서버 빌드 버전 문자열. 계약 패턴 `^[0-9A-Za-z][0-9A-Za-z.+_-]{0,63}$`.
///
/// **로그·버그 리포트 전용이다.** 기능 게이팅에 쓰지 않는다 — 호환성은 타입별
/// `schema_version` 이 정한다.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ServerVersion(String);

impl ServerVersion {
    /// 패턴을 만족하면 감싼다.
    #[must_use]
    pub fn parse(value: impl Into<String>) -> Option<Self> {
        let value = value.into();
        let bytes = value.as_bytes();
        if bytes.is_empty() || bytes.len() > 64 {
            return None;
        }
        if !bytes[0].is_ascii_alphanumeric() {
            return None;
        }
        bytes[1..]
            .iter()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'+' | b'_' | b'-'))
            .then_some(Self(value))
    }

    /// 원본 문자열.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ServerVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for ServerVersion {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for ServerVersion {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        Self::parse(raw.clone())
            .ok_or_else(|| D::Error::custom(format!("server_version 패턴에 맞지 않는다: {raw}")))
    }
}

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

    fn spike_calendar() -> GameCalendar {
        GameCalendar::new(20, GameTime::parse("3800-01-01T00:00:00Z").unwrap(), 60).unwrap()
    }

    #[test]
    fn occurred_at_matches_contract_fixtures() {
        // contracts/fixtures/SESSION_OPENED/basic.json, SESSION_CLOSED/client-closed.json,
        // SESSION_OPENED/sequence-nonzero.json 의 값이다. 문자열로 같아야 한다 (AC-16d).
        let calendar = spike_calendar();
        for (tick, expected) in [
            (0u64, "3800-01-01T00:00:00Z"),
            (1, "3800-01-01T00:00:00Z"),
            (19, "3800-01-01T00:00:00Z"),
            (20, "3800-01-01T00:01:00Z"),
            (1200, "3800-01-01T01:00:00Z"),
            (24000, "3800-01-01T20:00:00Z"),
            (86400, "3800-01-04T00:00:00Z"),
        ] {
            let got = calendar.occurred_at(Tick::new(tick).unwrap()).unwrap();
            assert_eq!(got.as_str(), expected, "tick {tick}");
        }
    }

    #[test]
    fn occurred_at_divides_by_tick_hz_first() {
        // tick_hz 를 두 배로 올려도 게임 달력 속도는 그대로여야 한다 (ADR-0006 §3).
        let slow = GameCalendar::new(20, GameTime::parse("3800-01-01T00:00:00Z").unwrap(), 60);
        let fast = GameCalendar::new(40, GameTime::parse("3800-01-01T00:00:00Z").unwrap(), 60);
        let (slow, fast) = (slow.unwrap(), fast.unwrap());
        // 실제 60초 = 20Hz 에서 1200 tick, 40Hz 에서 2400 tick. 같은 게임 시각이어야 한다.
        assert_eq!(
            slow.occurred_at(Tick::new(1200).unwrap()).unwrap(),
            fast.occurred_at(Tick::new(2400).unwrap()).unwrap()
        );
    }

    #[test]
    fn calendar_rejects_out_of_range_constants() {
        let epoch = GameTime::parse("3800-01-01T00:00:00Z").unwrap();
        assert!(GameCalendar::new(0, epoch.clone(), 60).is_none());
        assert!(GameCalendar::new(1001, epoch.clone(), 60).is_none());
        assert!(GameCalendar::new(20, epoch.clone(), 0).is_none());
        assert!(GameCalendar::new(20, epoch, 60).is_some());
    }

    #[test]
    fn civil_round_trips_over_a_wide_range() {
        // 1970-01-01 = day 0 규약과 역변환 일치를 확인한다.
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        for day in [-719_162i64, -1, 0, 1, 10_957, 20_000, 700_000, 800_000] {
            let (y, m, d) = civil_from_days(day);
            assert_eq!(days_from_civil(y, m, d), day, "day {day}");
        }
    }

    #[test]
    fn real_time_from_unix_uses_three_fraction_digits() {
        let value = RealTime::from_unix(1_758_200_662, 417_000_000).unwrap();
        assert_eq!(value.as_str(), "2025-09-18T13:04:22.417Z");
        assert!(RealTime::parse(value.as_str()).is_some());
    }

    #[test]
    fn const_schema_version_rejects_other_values() {
        assert!(serde_json::from_str::<ConstSchemaVersion<1>>("1").is_ok());
        assert!(serde_json::from_str::<ConstSchemaVersion<1>>("2").is_err());
    }
}

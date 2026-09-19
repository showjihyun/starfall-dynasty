//! 환경 변수 설정 로드.
//!
//! 설정은 환경 변수로만 읽고 기본값을 둔다. `.env.example` 이 값의 정본이다.
//! 여기서는 `.env` 파일을 읽지 않는다 — 셸이나 compose 가 환경에 넣어 준 값을 쓴다.
//! (dotenv 크레이트를 들이지 않는 이유: 설정 출처가 두 곳이 되면 "왜 이 값이 쓰였나"를
//! 추적하기 어려워진다. 개발자는 `.env` 를 셸에 로드하거나 직접 export 한다.)

use std::env::{self, VarError};
use std::net::SocketAddr;

use starfall_contracts::UuidV7;

/// 설정 로드 실패.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// 환경 변수 값이 소켓 주소로 파싱되지 않았다.
    #[error("환경 변수 {name} 의 값 {value:?} 을(를) 주소로 해석할 수 없다: {source}")]
    Address {
        /// 변수 이름.
        name: &'static str,
        /// 넣은 값.
        value: String,
        /// 파싱 오류.
        source: std::net::AddrParseError,
    },
    /// 환경 변수 값이 UTF-8 이 아니다.
    #[error("환경 변수 {name} 의 값이 올바른 UTF-8 이 아니다")]
    NotUnicode {
        /// 변수 이름.
        name: &'static str,
    },
    /// 로그 형식 값이 알려진 값이 아니다.
    #[error("STARFALL_LOG_FORMAT 의 값 {value:?} 은(는) pretty 또는 json 이어야 한다")]
    LogFormat {
        /// 넣은 값.
        value: String,
    },
    /// tick 주기 값이 계약 범위를 벗어난다.
    #[error("STARFALL_TICK_HZ 의 값 {value:?} 은(는) 1 ..= 1000 의 정수여야 한다")]
    TickHz {
        /// 넣은 값.
        value: String,
    },
    /// 월드 id 가 정규 UUIDv7 이 아니다.
    #[error(
        "STARFALL_WORLD_ID 의 값 {value:?} 은(는) 정규 소문자 하이픈 표기의 UUIDv7 이어야 한다"
    )]
    WorldId {
        /// 넣은 값.
        value: String,
    },
}

/// 로그 출력 형식.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogFormat {
    /// 사람이 읽는 형식. 로컬 개발 기본값.
    Pretty,
    /// 한 줄 JSON. 수집기에 넣을 때 쓴다.
    Json,
}

/// 서버 설정.
#[derive(Debug, Clone)]
pub struct Config {
    /// HTTP 바인딩 주소. 기본 `127.0.0.1:8080`.
    pub http_addr: SocketAddr,
    /// 로그 형식. 기본 `pretty`.
    pub log_format: LogFormat,
    /// PostgreSQL 접속 문자열. `/readyz` 가 쓴다.
    pub database_url: String,
    /// Redis 접속 문자열. `/readyz` 가 쓴다.
    pub redis_url: String,
    /// 실제 1초당 tick 수. 기본 20.
    ///
    /// **`worlds.tick_hz` 와 다르면 서버가 기동을 거부한다** (I-19, AC-2). 이 검사가
    /// "환경 변수 하나로 저장된 이벤트의 게임 시간 의미가 조용히 달라지는" 경로를 막는다.
    pub tick_hz: u32,
    /// 붙을 월드. 기본은 마이그레이션이 시드한 스파이크 월드다.
    pub world_id: UuidV7,
    /// 개발용 토큰 HMAC 비밀 (ADR-0008).
    ///
    /// **기본값이 없다.** 기본값을 코드에 두면 그 값이 배포까지 따라간다. 없으면 프로세스는
    /// 정상 기동하고 `/ws` 만 503 `auth_not_configured` 가 된다.
    pub dev_auth_secret: Option<String>,
}

/// 기본 바인딩 주소.
///
/// `localhost` 가 아니라 `127.0.0.1` 이다. Windows 에서 `localhost` 가 `::1` 로 먼저 풀리면
/// IPv4 에만 바인딩된 Docker 포트에 붙지 못하거나 지연이 생긴다 (ADR-0003 §4).
const DEFAULT_HTTP_ADDR: &str = "127.0.0.1:8080";
const DEFAULT_DATABASE_URL: &str = "postgres://starfall:starfall_dev_only@127.0.0.1:15432/starfall";
const DEFAULT_REDIS_URL: &str = "redis://127.0.0.1:16379/0";
/// 마이그레이션 0001 이 시드하는 스파이크 월드 (ADR-0007 §2).
const DEFAULT_WORLD_ID: &str = "01a0b1c2-3d4e-7f01-8a2b-9c0d1e2f3a4b";
/// 기본 tick 주기 (ADR-0006 §1).
const DEFAULT_TICK_HZ: &str = "20";

fn var_opt(name: &'static str) -> Result<Option<String>, ConfigError> {
    match env::var(name) {
        Ok(value) => Ok(Some(value)),
        Err(VarError::NotPresent) => Ok(None),
        Err(VarError::NotUnicode(_)) => Err(ConfigError::NotUnicode { name }),
    }
}

fn var_or(name: &'static str, default: &str) -> Result<String, ConfigError> {
    Ok(var_opt(name)?.unwrap_or_else(|| default.to_owned()))
}

impl Config {
    /// 환경에서 설정을 읽는다.
    ///
    /// # Errors
    ///
    /// 값이 UTF-8 이 아니거나, 주소·로그 형식으로 해석되지 않으면 실패한다.
    /// 기동 시점의 설정 오류는 조용히 기본값으로 넘어가지 않고 즉시 드러나야 한다.
    pub fn from_env() -> Result<Self, ConfigError> {
        let raw_addr = var_or("STARFALL_HTTP_ADDR", DEFAULT_HTTP_ADDR)?;
        let http_addr = raw_addr.parse().map_err(|source| ConfigError::Address {
            name: "STARFALL_HTTP_ADDR",
            value: raw_addr.clone(),
            source,
        })?;

        let raw_format = var_or("STARFALL_LOG_FORMAT", "pretty")?;
        let log_format = match raw_format.as_str() {
            "pretty" => LogFormat::Pretty,
            "json" => LogFormat::Json,
            _ => return Err(ConfigError::LogFormat { value: raw_format }),
        };

        let raw_tick_hz = var_or("STARFALL_TICK_HZ", DEFAULT_TICK_HZ)?;
        let tick_hz = raw_tick_hz
            .parse::<u32>()
            .ok()
            .filter(|value| (1..=1000).contains(value))
            .ok_or_else(|| ConfigError::TickHz {
                value: raw_tick_hz.clone(),
            })?;

        let raw_world_id = var_or("STARFALL_WORLD_ID", DEFAULT_WORLD_ID)?;
        let world_id = UuidV7::parse(&raw_world_id).ok_or_else(|| ConfigError::WorldId {
            value: raw_world_id.clone(),
        })?;

        Ok(Self {
            http_addr,
            log_format,
            database_url: var_or("DATABASE_URL", DEFAULT_DATABASE_URL)?,
            redis_url: var_or("REDIS_URL", DEFAULT_REDIS_URL)?,
            tick_hz,
            world_id,
            // 빈 문자열은 "설정하지 않음"과 같게 다룬다 — `.env` 에서 값을 지웠을 때
            // 절반만 설정된 상태가 생기지 않게 한다.
            dev_auth_secret: var_opt("STARFALL_DEV_AUTH_SECRET")?.filter(|s| !s.is_empty()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 기본값은 마이그레이션 시드와 ADR-0006 §1 을 그대로 따라야 한다.
    #[test]
    fn defaults_match_the_seeded_world() {
        assert_eq!(DEFAULT_TICK_HZ, "20");
        assert!(UuidV7::parse(DEFAULT_WORLD_ID).is_some());
        assert_eq!(DEFAULT_WORLD_ID, "01a0b1c2-3d4e-7f01-8a2b-9c0d1e2f3a4b");
    }
}

//! `GET /readyz` — readiness.
//!
//! "지금 트래픽을 받을 수 있는가"를 답한다. `/healthz`(liveness)와 **용도를 섞지 않는다**
//! (ADR-0003 §3.1 제약 4): 배포 슬라이스에서 이것을 재시작 트리거로 쓰면 DB 장애가
//! 서버 재시작 루프가 된다.
//!
//! # 이 엔드포인트가 이 슬라이스에 있는 이유
//!
//! `docker compose ps` 의 `healthy` 는 **컨테이너 안에서** `pg_isready` 가 돌았다는 뜻일 뿐이다.
//! "Rust 프로세스가 `.env` 의 자격 증명으로, 호스트 포트 15432 를 통해, IPv4 로 붙을 수 있는가"는
//! 전혀 증명하지 않는다. Windows 에서 실제로 깨지는 지점이 정확히 거기다.
//!
//! # 구현 제약 (ADR-0003 §3.1)
//!
//! 1. **지연 연결** — DB 가 꺼져 있어도 프로세스는 기동한다. 기동 시 연결을 강제하면
//!    "인프라 없이도 서버는 살아 있다"를 만족할 수 없다.
//! 2. **점검마다 2초 타임아웃, 두 점검은 동시에** — `/readyz` 가 매달리면 readiness 신호가
//!    아니라 장애다.
//! 3. **본문에 드라이버 에러·DSN·비밀번호를 넣지 않는다** — [`CheckStatus`] 가 닫힌 집합이라
//!    타입 수준에서 불가능하다. 원인은 `tracing` 로그에만 남는다.

use std::sync::Arc;
use std::time::Duration;

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use redis::aio::ConnectionManager;
use serde::Serialize;
use sqlx::postgres::{PgPool, PgPoolOptions};
use tokio::sync::OnceCell;

use crate::state::AppState;

/// 점검 하나에 허용하는 시간.
const CHECK_TIMEOUT: Duration = Duration::from_secs(2);

/// readiness 점검 하나의 결과.
///
/// **닫힌 집합이다.** 드라이버 에러 문자열이 여기 들어올 자리가 없다 — 개발 편의로 에러를
/// 본문에 실으면 그 습관이 배포까지 가고, DSN·비밀번호가 응답으로 새어 나간다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckStatus {
    /// 의존성에 붙어 명령이 왕복했다.
    Ok,
    /// 붙지 못했거나 제한 시간 안에 응답하지 않았다.
    Unavailable,
}

impl CheckStatus {
    const fn is_ok(self) -> bool {
        matches!(self, Self::Ok)
    }
}

/// 점검 1회의 결과. **즉시 실패와 타임아웃을 구분한다** — 재시도 정책이 여기 달려 있다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CheckOutcome {
    /// 왕복 성공.
    Ok,
    /// 즉시 실패(끊긴 연결·접속 거부). 1회 재시도 대상이다.
    FailedFast,
    /// 제한 시간 초과. 재시도하지 않는다.
    TimedOut,
}

/// 의존성 점검 핸들.
///
/// `Clone` 이 싸다 — `PgPool` 과 `redis::Client` 는 내부가 `Arc` 다.
#[derive(Clone)]
pub struct Probes {
    postgres: PgPool,
    redis_client: redis::Client,
    /// 첫 성공한 점검에서 만들어지는 연결. 실패하면 비어 있는 채로 남아 다음 점검이 다시 시도한다.
    ///
    /// `ConnectionManager` 를 기동 시점에 만들면 Redis 가 꺼져 있을 때 프로세스가 뜨지 못한다.
    /// 그래서 `OnceCell` 로 미루고, 한 번 만들어진 뒤에는 매니저가 자동 재연결을 맡는다.
    redis_connection: Arc<OnceCell<ConnectionManager>>,
}

impl std::fmt::Debug for Probes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // 접속 문자열이 로그·패닉 메시지로 새지 않게 직접 구현한다.
        f.debug_struct("Probes")
            .field("redis_connected", &self.redis_connection.initialized())
            .finish_non_exhaustive()
    }
}

/// 의존성 핸들 생성 실패.
#[derive(Debug, thiserror::Error)]
pub enum ProbeSetupError {
    /// `DATABASE_URL` 을 해석할 수 없다.
    #[error("DATABASE_URL 을 해석할 수 없다")]
    Database(#[source] sqlx::Error),
    /// `REDIS_URL` 을 해석할 수 없다.
    #[error("REDIS_URL 을 해석할 수 없다")]
    Redis(#[source] redis::RedisError),
}

impl Probes {
    /// 접속 문자열에서 점검 핸들을 만든다. **여기서 연결하지 않는다.**
    ///
    /// # Panics
    ///
    /// **Tokio 런타임 컨텍스트 안에서 불러야 한다.** `sqlx` 의 지연 풀은 유휴 연결을 정리하는
    /// 백그라운드 태스크를 즉시 spawn 하므로, 런타임 밖에서 부르면
    /// "this functionality requires a Tokio context" 로 패닉한다.
    /// 연결을 열지 않는 것과 런타임이 필요 없는 것은 다른 이야기다.
    ///
    /// # Errors
    ///
    /// 접속 문자열 자체가 형식에 맞지 않으면 실패한다. 이것은 설정 오류이므로 기동 시점에
    /// 드러나야 한다 — 의존성이 꺼져 있는 것과는 다른 문제다.
    pub fn new(database_url: &str, redis_url: &str) -> Result<Self, ProbeSetupError> {
        let postgres = PgPoolOptions::new()
            .max_connections(2)
            .acquire_timeout(CHECK_TIMEOUT)
            .connect_lazy(database_url)
            .map_err(ProbeSetupError::Database)?;
        let redis_client = redis::Client::open(redis_url).map_err(ProbeSetupError::Redis)?;

        Ok(Self {
            postgres,
            redis_client,
            redis_connection: Arc::new(OnceCell::new()),
        })
    }

    /// 각 점검을 한 번 돌리고, **즉시 실패했을 때만** 1회 재시도한다 (ADR-0007 §9).
    ///
    /// # 왜 타임아웃에는 재시도하지 않는가
    ///
    /// p0-01 에서 컨테이너 재생성 직후 첫 `/readyz` 가 Redis broken pipe 로 1회 503 을 냈다.
    /// 원인은 풀에 남은 **끊긴 연결**이고, 그 실패는 마이크로초 단위로 즉시 돌아온다. 그래서
    /// 즉시 실패만 재시도하면 총 소요가 사실상 점검당 2초를 넘지 않는다.
    ///
    /// 타임아웃까지 재시도하면 점검당 최악 4초가 되어 ADR-0003 §3.1 제약 2(점검당 2초)와
    /// 충돌한다. 그리고 느린(그러나 살아 있는) DB 의 동작이 조용히 바뀐다 —
    /// readiness 가 매달리면 그 자체가 장애다.
    async fn check_with_retry<F, Fut>(check: F) -> CheckStatus
    where
        F: Fn() -> Fut,
        Fut: Future<Output = CheckOutcome>,
    {
        match check().await {
            CheckOutcome::Ok => CheckStatus::Ok,
            // 타임아웃 = 재시도 없음.
            CheckOutcome::TimedOut => CheckStatus::Unavailable,
            CheckOutcome::FailedFast => match check().await {
                CheckOutcome::Ok => CheckStatus::Ok,
                _ => CheckStatus::Unavailable,
            },
        }
    }

    async fn check_postgres(&self) -> CheckStatus {
        Self::check_with_retry(|| self.check_postgres_once()).await
    }

    async fn check_redis(&self) -> CheckStatus {
        Self::check_with_retry(|| self.check_redis_once()).await
    }

    async fn check_postgres_once(&self) -> CheckOutcome {
        // `SELECT 1` 은 풀에서 연결을 하나 얻어 실제로 왕복한다.
        // sqlx 의 test_before_acquire(기본 true)가 죽은 연결을 걸러 주므로,
        // 컨테이너를 내렸다 올려도 다음 점검이 새 연결로 복구된다.
        let query = sqlx::query("SELECT 1").execute(&self.postgres);
        match tokio::time::timeout(CHECK_TIMEOUT, query).await {
            Ok(Ok(_)) => CheckOutcome::Ok,
            Ok(Err(error)) => {
                tracing::warn!(%error, "readiness: PostgreSQL 점검 실패");
                CheckOutcome::FailedFast
            }
            Err(_) => {
                tracing::warn!(
                    timeout_ms = CHECK_TIMEOUT.as_millis(),
                    "readiness: PostgreSQL 점검 타임아웃"
                );
                CheckOutcome::TimedOut
            }
        }
    }

    async fn check_redis_once(&self) -> CheckOutcome {
        let ping = async {
            let manager = self
                .redis_connection
                .get_or_try_init(|| ConnectionManager::new(self.redis_client.clone()))
                .await?;
            let mut connection = manager.clone();
            redis::cmd("PING")
                .query_async::<String>(&mut connection)
                .await
        };

        match tokio::time::timeout(CHECK_TIMEOUT, ping).await {
            Ok(Ok(_)) => CheckOutcome::Ok,
            Ok(Err(error)) => {
                tracing::warn!(%error, "readiness: Redis 점검 실패");
                CheckOutcome::FailedFast
            }
            Err(_) => {
                tracing::warn!(
                    timeout_ms = CHECK_TIMEOUT.as_millis(),
                    "readiness: Redis 점검 타임아웃"
                );
                CheckOutcome::TimedOut
            }
        }
    }
}

/// `/readyz` 의 `checks` 객체.
#[derive(Debug, Clone, Serialize)]
pub struct Checks {
    /// PostgreSQL 점검 결과.
    pub postgres: CheckStatus,
    /// Redis 점검 결과.
    pub redis: CheckStatus,
}

/// `/readyz` 응답 본문.
#[derive(Debug, Clone, Serialize)]
pub struct ReadyBody {
    /// `"ready"` 또는 `"not_ready"`.
    pub status: &'static str,
    /// 의존성별 결과.
    pub checks: Checks,
}

/// readiness 핸들러.
///
/// 두 점검을 **동시에** 돌리고 각각 2초로 제한한다. 전체 응답 시간은 최악의 경우에도
/// 약 2초를 넘지 않는다(직렬이면 4초가 된다).
pub async fn readyz(State(state): State<AppState>) -> (StatusCode, Json<ReadyBody>) {
    let (postgres, redis) = tokio::join!(state.probes.check_postgres(), state.probes.check_redis());

    let ready = postgres.is_ok() && redis.is_ok();
    let code = if ready {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };

    (
        code,
        Json(ReadyBody {
            status: if ready { "ready" } else { "not_ready" },
            checks: Checks { postgres, redis },
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_status_serializes_to_the_closed_set() {
        assert_eq!(serde_json::to_string(&CheckStatus::Ok).unwrap(), "\"ok\"");
        assert_eq!(
            serde_json::to_string(&CheckStatus::Unavailable).unwrap(),
            "\"unavailable\""
        );
    }

    #[test]
    fn ready_body_matches_spec_shape() {
        let body = ReadyBody {
            status: "ready",
            checks: Checks {
                postgres: CheckStatus::Ok,
                redis: CheckStatus::Ok,
            },
        };
        assert_eq!(
            serde_json::to_string(&body).unwrap(),
            r#"{"status":"ready","checks":{"postgres":"ok","redis":"ok"}}"#
        );
    }

    #[test]
    fn failure_body_never_carries_driver_detail() {
        // 타입이 닫힌 집합이라 에러 문자열이 들어갈 자리가 없다. 이 테스트는 그 성질을 고정한다.
        let body = ReadyBody {
            status: "not_ready",
            checks: Checks {
                postgres: CheckStatus::Unavailable,
                redis: CheckStatus::Ok,
            },
        };
        let json = serde_json::to_string(&body).unwrap();
        assert_eq!(
            json,
            r#"{"status":"not_ready","checks":{"postgres":"unavailable","redis":"ok"}}"#
        );
        for secret in ["postgres://", "redis://", "password", "starfall_dev_only"] {
            assert!(!json.contains(secret), "{secret} 가 응답 본문에 있다");
        }
    }

    #[tokio::test]
    async fn probes_construct_without_connecting() {
        // 아무 것도 떠 있지 않은 포트를 줘도 생성은 성공해야 한다 (지연 연결).
        let probes = Probes::new(
            "postgres://starfall:pw@127.0.0.1:1/starfall",
            "redis://127.0.0.1:1/0",
        );
        assert!(probes.is_ok(), "지연 연결이어야 하는데 생성에서 실패했다");
    }

    #[tokio::test]
    async fn probes_reject_malformed_urls() {
        assert!(Probes::new("not-a-dsn", "redis://127.0.0.1:1/0").is_err());
        assert!(Probes::new("postgres://starfall:pw@127.0.0.1:1/starfall", "nope://x").is_err());
    }

    #[tokio::test]
    async fn readyz_reports_unavailable_when_nothing_is_listening() {
        // 닫힌 포트를 향한 점검은 2초 안에 unavailable 로 끝나야 한다 — 매달리면 안 된다.
        let probes = Probes::new(
            "postgres://starfall:pw@127.0.0.1:1/starfall",
            "redis://127.0.0.1:1/0",
        )
        .unwrap();
        let state = AppState::new("test", probes);

        let started = std::time::Instant::now();
        let (code, Json(body)) = readyz(State(state)).await;
        let elapsed = started.elapsed();

        assert_eq!(code, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body.checks.postgres, CheckStatus::Unavailable);
        assert_eq!(body.checks.redis, CheckStatus::Unavailable);
        assert!(
            elapsed < Duration::from_secs(5),
            "두 점검이 동시에 돌지 않았거나 타임아웃이 걸리지 않았다: {elapsed:?}"
        );
    }
}

//! `GET /healthz` — liveness.
//!
//! **의존성을 보지 않는다.** 이 응답은 "프로세스가 살아 있고 HTTP 를 처리한다"만 뜻한다.
//! PostgreSQL·Redis 상태는 `/readyz` 가 본다 (ADR-0003 §3.1 제약 4).
//!
//! 이 구분이 중요한 이유: 배포 시 liveness 프로브가 의존성을 보면, DB 가 잠깐 흔들릴 때
//! 오케스트레이터가 멀쩡한 서버를 죽이고 재시작 루프가 돈다.

use axum::Json;
use axum::extract::State;
use serde::Serialize;

use crate::state::AppState;

/// `/healthz` 응답 본문. 스펙 §5 의 형태를 그대로 따른다.
#[derive(Debug, Clone, Serialize)]
pub struct HealthBody {
    /// 항상 `"ok"`. 이 핸들러가 응답했다는 것 자체가 liveness 의 증거다.
    pub status: &'static str,
    /// 서버 빌드 버전 (`CARGO_PKG_VERSION`).
    pub version: &'static str,
}

/// liveness 핸들러. 언제나 200 을 준다.
pub async fn healthz(State(state): State<AppState>) -> Json<HealthBody> {
    Json(HealthBody {
        status: "ok",
        version: state.version,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn healthz_reports_ok_and_version() {
        let probes = crate::readiness::Probes::new(
            "postgres://starfall:pw@127.0.0.1:1/starfall",
            "redis://127.0.0.1:1/0",
        )
        .unwrap();
        let state = AppState::new("9.9.9-test", probes);
        let Json(body) = healthz(State(state)).await;
        assert_eq!(body.status, "ok");
        assert_eq!(body.version, "9.9.9-test");
    }

    #[test]
    fn healthz_body_serializes_to_spec_shape() {
        let body = HealthBody {
            status: "ok",
            version: "0.1.0",
        };
        let json = serde_json::to_string(&body).unwrap();
        assert_eq!(json, r#"{"status":"ok","version":"0.1.0"}"#);
    }
}

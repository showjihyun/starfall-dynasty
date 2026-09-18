//! STARFALL 게이트웨이 — HTTP 표면.
//!
//! # 이 크레이트에 넣지 않는 것 (ADR-0001 §2)
//!
//! 게임 규칙과 월드 상태는 여기에 두지 않는다. 다음 슬라이스에서 게임 로직을 넣을 가장 쉬운
//! 자리가 이미 존재하는 이 크레이트인데, 거기에 넣기 시작하면 `domain <- sim <- gateway`
//! 의존 방향이 문서에만 남는다. 첫 게임 상태 코드가 `starfall-domain` 을 만든다.
//!
//! 현재 표면은 운영 엔드포인트 두 개뿐이다.
//!
//! | 경로 | 용도 | 의존성을 보는가 |
//! |------|------|----------------|
//! | `GET /healthz` | liveness — 프로세스가 살아 있는가 | 보지 않는다 |
//! | `GET /readyz` | readiness — 트래픽을 받을 수 있는가 | PostgreSQL·Redis |
//!
//! 둘의 용도를 섞지 않는다 (ADR-0003 §3.1 제약 4).

pub mod health;
pub mod readiness;
pub mod state;

pub use readiness::{CheckStatus, ProbeSetupError, Probes};
pub use state::AppState;

use axum::Router;
use axum::routing::get;

/// 운영 엔드포인트 라우터.
///
/// 모듈별 `Router` 를 조립하는 지점이다. 명령·세션 라우터는 p0-02 에서 여기에 합쳐진다.
pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(health::healthz))
        .route("/readyz", get(readiness::readyz))
        .with_state(state)
}

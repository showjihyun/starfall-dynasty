//! STARFALL 게이트웨이 — HTTP/WebSocket 표면.
//!
//! # 이 크레이트에 넣지 않는 것 (ADR-0001 §2)
//!
//! 게임 규칙과 월드 상태는 여기에 두지 않는다. 다음 슬라이스에서 게임 로직을 넣을 가장 쉬운
//! 자리가 이미 존재하는 이 크레이트인데, 거기에 넣기 시작하면 `sim <- gateway` 의존 방향이
//! 문서에만 남는다. 여기서 볼 수 있는 것은 [`runtime::SubmitHandle`] 뿐이고, 그 핸들에는
//! 시뮬레이션 상태 타입이 보이지 않는다 (I-13).
//!
//! **`starfall-persistence` 도 의존하지 않는다.** tick 결과는 채널로 나가고, 그 채널을
//! 영속화에 연결하는 것은 조립 바이너리의 일이다.
//!
//! | 경로 | 용도 | 의존성을 보는가 |
//! |------|------|----------------|
//! | `GET /healthz` | liveness — 프로세스가 살아 있는가 | 보지 않는다 |
//! | `GET /readyz` | readiness — 트래픽을 받을 수 있는가 | PostgreSQL·Redis |
//! | `GET /ws` | 실시간 게이트웨이 (ADR-0005) | tick 루프·영속화 백로그 |
//! | `GET /debug/stats` | 운영 관측 (ADR-0007 §8). **계약이 아니다** | 보지 않는다 |
//!
//! `/healthz` 와 `/readyz` 의 용도를 섞지 않는다 (ADR-0003 §3.1 제약 4).

pub mod auth;
pub mod health;
pub mod readiness;
pub mod runtime;
pub mod state;
pub mod stats;
pub mod ws;

pub use auth::{DevAuth, extract_credential};
pub use readiness::{CheckStatus, ProbeSetupError, Probes};
pub use runtime::SubmitHandle;
pub use state::AppState;
pub use stats::Stats;

use axum::Router;
use axum::routing::{any, get};

/// 운영 엔드포인트 라우터 (p0-01 표면).
pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(health::healthz))
        .route("/readyz", get(readiness::readyz))
        .route("/debug/stats", get(stats::debug_stats))
        .with_state(state)
}

/// 운영 + 실시간 표면.
///
/// `/ws` 를 `any` 로 등록하는 이유: axum 의 `WebSocketUpgrade` 는 HTTP/1.1 에서 GET 을
/// 요구하지만 HTTP/2 확장 CONNECT 도 받을 수 있어야 하고, 메서드 라우팅으로 걸러 내면
/// 405 가 업그레이드 실패로 보인다 (axum 문서 권장).
pub fn realtime_router(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(health::healthz))
        .route("/readyz", get(readiness::readyz))
        .route("/debug/stats", get(stats::debug_stats))
        .route("/ws", any(ws::ws_handler))
        .with_state(state)
}

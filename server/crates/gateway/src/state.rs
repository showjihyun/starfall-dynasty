//! 게이트웨이 공유 상태.
//!
//! 핸들러가 공유하는 것은 여기에만 둔다. 내부는 전부 싸게 clone 되는 형태여야 한다
//! (axum 이 요청마다 `AppState` 를 clone 한다).

use crate::readiness::Probes;

/// 핸들러가 공유하는 상태.
///
/// 월드 상태는 여기에 **들어가지 않는다.** 시뮬레이션 상태는 tick 루프 소유이고,
/// 게이트웨이는 명령을 큐에 넣기만 한다 (rust-authoritative-server §2).
#[derive(Debug, Clone)]
pub struct AppState {
    /// 서버 빌드 버전. `/healthz` 가 그대로 돌려준다.
    pub version: &'static str,
    /// `/readyz` 의 의존성 점검 핸들.
    pub probes: Probes,
}

impl AppState {
    /// 운영 엔드포인트용 상태를 만든다.
    #[must_use]
    pub fn new(version: &'static str, probes: Probes) -> Self {
        Self { version, probes }
    }
}

//! 게이트웨이 공유 상태.
//!
//! 핸들러가 공유하는 것은 여기에만 둔다. 내부는 전부 싸게 clone 되는 형태여야 한다
//! (axum 이 요청마다 `AppState` 를 clone 한다).

use crate::auth::DevAuth;
use crate::readiness::Probes;
use crate::runtime::SubmitHandle;
use crate::stats::Stats;

/// 핸들러가 공유하는 상태.
///
/// **월드 상태는 여기 들어가지 않는다.** 시뮬레이션 상태는 tick 루프 소유이고, 게이트웨이는
/// [`SubmitHandle`] 로 제출만 한다 (I-13). 그 핸들에는 상태 타입이 보이지 않는다.
#[derive(Debug, Clone)]
pub struct AppState {
    /// 서버 빌드 버전. `/healthz` 가 그대로 돌려준다.
    pub version: &'static str,
    /// `/readyz` 의 의존성 점검 핸들.
    pub probes: Probes,
    /// 개발용 토큰 검증기 (ADR-0008).
    pub auth: DevAuth,
    /// 관측 값. `/debug/stats` 가 읽고 게이트웨이·tick 드라이버가 쓴다.
    pub stats: Stats,
    /// tick 루프 제출 통로. 운영 엔드포인트만 쓰는 조립에서는 `None` 이다.
    pub submit: Option<SubmitHandle>,
}

impl AppState {
    /// 운영 엔드포인트만 있는 상태 (p0-01 호환 — tick 루프 없이 `/healthz`·`/readyz` 만).
    #[must_use]
    pub fn new(version: &'static str, probes: Probes) -> Self {
        Self {
            version,
            probes,
            auth: DevAuth::Disabled,
            stats: Stats::new(),
            submit: None,
        }
    }

    /// 실시간 표면을 붙인다.
    #[must_use]
    pub fn with_realtime(mut self, auth: DevAuth, stats: Stats, submit: SubmitHandle) -> Self {
        self.auth = auth;
        self.stats = stats;
        self.submit = Some(submit);
        self
    }
}

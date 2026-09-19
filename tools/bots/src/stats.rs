//! 분위수. **nearest-rank**(가장 가까운 순위) 방식으로 고정한다.
//!
//! 보간 방식을 쓰면 "p99 = 실제로 관측된 표본"이 아니게 되어, 리포트의 숫자를 원본
//! `commands.csv` 에서 되짚을 수 없다. QA 숫자는 언제나 원본으로 되짚을 수 있어야 한다.
//!
//! `p(q)` = 정렬된 표본의 `ceil(q * n)` 번째(1-based). n = 0 이면 `None`.

use serde::Serialize;

#[derive(Debug, Clone, Default, Serialize)]
pub struct Summary {
    pub count: usize,
    pub min_ms: Option<f64>,
    pub p50_ms: Option<f64>,
    pub p90_ms: Option<f64>,
    pub p99_ms: Option<f64>,
    pub max_ms: Option<f64>,
    pub mean_ms: Option<f64>,
}

/// `samples` 는 밀리초. 호출자가 소유권을 넘기면 여기서 정렬한다.
pub fn summarize(mut samples: Vec<f64>) -> Summary {
    if samples.is_empty() {
        return Summary::default();
    }
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = samples.len();
    let sum: f64 = samples.iter().sum();
    Summary {
        count: n,
        min_ms: Some(samples[0]),
        p50_ms: Some(nearest_rank(&samples, 0.50)),
        p90_ms: Some(nearest_rank(&samples, 0.90)),
        p99_ms: Some(nearest_rank(&samples, 0.99)),
        max_ms: Some(samples[n - 1]),
        mean_ms: Some(sum / n as f64),
    }
}

/// 정렬된 표본에서 nearest-rank 분위수. `q` 는 0.0..=1.0.
pub fn nearest_rank(sorted: &[f64], q: f64) -> f64 {
    debug_assert!(!sorted.is_empty());
    let n = sorted.len();
    // ceil(q * n), 최소 1, 최대 n
    let rank = (q * n as f64).ceil().max(1.0) as usize;
    sorted[rank.min(n) - 1]
}

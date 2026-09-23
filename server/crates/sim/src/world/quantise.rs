//! 와이어 양자화 — `q(x, scale, lo, hi) = clamp(round_half_away_from_zero(x*scale), lo, hi)`
//! (ADR-0009 §2).
//!
//! `f64::round()` 는 Rust 에서 이미 "half away from zero" 다 — C# 은
//! `Math.Round(x, MidpointRounding.AwayFromZero)` 를 명시해야 같은 결과가 나온다(ADR-0009 §2
//! 실측 표). 역양자화는 같은 상수로 나누는 부동소수 나눗셈이라 양쪽이 같은 비트를 낸다.

/// `x`(SI 단위, 예: m)를 `scale` 배율로 정수 양자화하고 `[lo, hi]` 로 클램프한다.
///
/// `f64 as i64` 캐스트는 Rust 1.45 부터 **포화 변환**이다(`NaN` → 0, 범위 밖은 `i64::MIN`/
/// `MAX` 로 잘린다) — 그 위에 계약 범위(`lo..=hi`, 대개 `i64` 전체보다 훨씬 좁다)를 다시
/// 클램프한다.
#[must_use]
pub fn quantise(x: f64, scale: f64, lo: i64, hi: i64) -> i64 {
    let scaled = (x * scale).round();
    #[allow(clippy::cast_possible_truncation)]
    let as_int = scaled as i64;
    as_int.clamp(lo, hi)
}

/// 정수 양자화 값을 `scale` 배율로 역양자화한다.
#[must_use]
pub fn dequantise(i: i64, scale: f64) -> f64 {
    #[allow(clippy::cast_precision_loss)]
    let as_f64 = i as f64;
    as_f64 / scale
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_half_away_from_zero_matches_adr_0009_table() {
        // ADR-0009 §2 의 Mono AwayFromZero 실측 표와 같은 값이어야 한다(Rust 쪽 기준선).
        assert_eq!(quantise(0.5, 1.0, -10, 10), 1);
        assert_eq!(quantise(2.5, 1.0, -10, 10), 3);
        assert_eq!(quantise(-2.5, 1.0, -10, 10), -3);
        assert_eq!(quantise(1234.5, 1.0, -10_000, 10_000), 1235);
    }

    #[test]
    fn clamps_to_contract_range() {
        assert_eq!(
            quantise(
                1_000_000_000.0,
                1000.0,
                -1_000_000_000_000,
                1_000_000_000_000
            ),
            1_000_000_000_000
        );
        assert_eq!(
            quantise(
                -1_000_000_000.0,
                1000.0,
                -1_000_000_000_000,
                1_000_000_000_000
            ),
            -1_000_000_000_000
        );
    }

    #[test]
    fn quantise_dequantise_round_trip_within_half_ulp() {
        let original = 18.375;
        let scale = 1000.0; // mm
        let q = quantise(original, scale, -1_000_000_000_000, 1_000_000_000_000);
        assert_eq!(q, 18375);
        let back = dequantise(q, scale);
        assert!((back - original).abs() < 1e-9);
    }
}

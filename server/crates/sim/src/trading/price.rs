//! 가격 규칙 — 구간형 포화 + tick 배수 회복, 순수 함수 (디자인 §3.3, 스펙 §4.2a,
//! ADR-0016 §4).
//!
//! `starfall-sim` 은 IO 가 없다(I-58). 이 모듈은 더 좁혀서 **시뮬레이션 상태를 전혀
//! 모른다** — 포화·판매량·상수만 받아 값을 돌려주는 함수뿐이다. 시계도 난수도 읽지
//! 않는다(I-58). `Simulation`(S3)이 이 함수를 호출해 실제 시장 상태를 바꾼다.
//!
//! 정수 연산만, 모든 나눗셈은 피연산자가 음이 아닌 0 쪽 버림(div)이다 — 반올림이
//! 크레딧을 만들거나 없앨 수 있는 자리를 닫는다(ADR-0016 §2).

/// 가격 규칙 상수 — 시장 상장 하나(`MARKET.listings[]` + `MARKET.rules` 의 교집합,
/// `bins/game-server/src/data.rs` 가 기동 검산을 끝낸 값).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PriceRule {
    /// band 0(포화 < `band_kg`)의 kg 당 가격.
    pub base_price_cr_per_kg: i64,
    /// 가격 구간 하나의 포화 너비.
    pub band_kg: i64,
    /// 바닥 가격 비율(%, 기준가 대비).
    pub price_floor_pct: i64,
    /// 가격 구간 하나당 하락률(%p).
    pub price_drop_pct_per_band: i64,
}

impl PriceRule {
    /// `floor_band = ceil((100 - price_floor_pct) / price_drop_pct_per_band)`
    /// (디자인 §3.3). 0 쪽 올림 — `i64::div_ceil` 이 이 툴체인에서 아직 안정화되지
    /// 않아 손으로 쓴다(`bins/game-server/src/data.rs::floor_band` 와 같은 식).
    #[must_use]
    pub const fn floor_band(&self) -> i64 {
        let numerator = 100 - self.price_floor_pct;
        (numerator + self.price_drop_pct_per_band - 1) / self.price_drop_pct_per_band
    }

    /// 포화 상한 — `floor_band * band_kg`. 어떤 판매도 이 값을 넘기지 않는다.
    #[must_use]
    pub const fn saturation_cap(&self) -> i64 {
        self.floor_band() * self.band_kg
    }

    /// `band(s) = min(s div band_kg, floor_band)`.
    #[must_use]
    pub const fn band(&self, saturation_kg: i64) -> i64 {
        let raw = saturation_kg / self.band_kg;
        if raw > self.floor_band() {
            self.floor_band()
        } else {
            raw
        }
    }

    /// `price(b) = base_price_cr_per_kg * max(price_floor_pct, 100 - b * price_drop_pct_per_band) div 100`.
    #[must_use]
    pub const fn price_at_band(&self, band: i64) -> i64 {
        let dropped_pct = 100 - band * self.price_drop_pct_per_band;
        let pct = if dropped_pct < self.price_floor_pct {
            self.price_floor_pct
        } else {
            dropped_pct
        };
        self.base_price_cr_per_kg * pct / 100
    }

    /// 바닥 가격 — `price_at_band(floor_band())`. 기동 검증(§4.8 m10)은 이 값이
    /// `>= 1` 임을 보장한다 — 이 모듈은 그 전제를 가정한다(검산하지 않는다).
    #[must_use]
    pub const fn floor_price_cr_per_kg(&self) -> i64 {
        self.price_at_band(self.floor_band())
    }
}

/// 구간 걷기 결과(스펙 §4.2a) — `MINERAL_SOLD`·`SALE_RECEIPT`·`MARKET_STATE` 가
/// 그대로 싣는 필드와 1:1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SaleResult {
    /// 받은 대금.
    pub proceeds_cr: i64,
    /// 구간 걷기 뒤의 포화.
    pub saturation_after_kg: i64,
    /// 판매 직전 시세(`price(band(saturation_before_kg))`).
    pub price_before_cr_per_kg: i64,
    /// 판매 직후 시세.
    pub price_after_cr_per_kg: i64,
    /// 구간 반복 횟수(ADR-0016 §4 — kg 수와 무관, `floor_band + 1` 이하).
    pub band_walk_iterations: u32,
}

/// 판매 `quantity_kg` 을 `saturation_kg`(회복 적용 후 — 호출자가 [`effective_saturation`]
/// 으로 이미 맞춘 값)에서 구간을 걸으며 계산한다(스펙 §4.2a). **kg 마다 도는 루프가
/// 아니다** — 반복은 `floor_band + 1` 회 이하다(ADR-0016 §4, `MassKg` 상한 `2^31-1`
/// 까지도).
#[must_use]
pub fn sell(rule: &PriceRule, saturation_kg: i64, quantity_kg: i64) -> SaleResult {
    let price_before = rule.price_at_band(rule.band(saturation_kg));
    let floor_band = rule.floor_band();
    let cap = rule.saturation_cap();

    let mut saturation = saturation_kg;
    let mut remaining = quantity_kg;
    let mut proceeds: i64 = 0;
    let mut iterations: u32 = 0;

    loop {
        iterations += 1;
        let band = rule.band(saturation);
        if band >= floor_band {
            // 바닥: 남은 양 전부 바닥값, 포화는 상한에 머문다.
            proceeds += remaining * rule.floor_price_cr_per_kg();
            saturation = cap;
            break;
        }
        let next_boundary = (band + 1) * rule.band_kg;
        let take = remaining.min(next_boundary - saturation);
        proceeds += take * rule.price_at_band(band);
        saturation += take;
        remaining -= take;
        if remaining == 0 {
            break;
        }
    }

    SaleResult {
        proceeds_cr: proceeds,
        saturation_after_kg: saturation,
        price_before_cr_per_kg: price_before,
        price_after_cr_per_kg: rule.price_at_band(rule.band(saturation)),
        band_walk_iterations: iterations,
    }
}

/// 게으른 회복(스펙 I-87): `effective(t) = max(0, s - recovery_kg * (floor(t/I) -
/// floor(as_of/I)))`. `mining::effective_remaining` 과 같은 꼴(ADR-0013 §1)이지만
/// 반대 방향(회복이 아니라 소모)이고 상한 클램프가 없다 — 포화의 상한은
/// [`PriceRule::saturation_cap`] 이 판매 시점에 이미 강제한다. DB 쓰기도 도메인
/// 이벤트도 만들지 않는다 — 순수 계산.
#[must_use]
pub fn effective_saturation(
    saturation_kg: i64,
    as_of_tick: u64,
    now_tick: u64,
    recovery_kg: i64,
    interval_ticks: u64,
) -> i64 {
    if interval_ticks == 0 || now_tick <= as_of_tick {
        return saturation_kg.max(0);
    }
    let elapsed_intervals =
        i128::from(now_tick / interval_ticks) - i128::from(as_of_tick / interval_ticks);
    if elapsed_intervals <= 0 {
        return saturation_kg.max(0);
    }
    let recovered = i128::from(recovery_kg).saturating_mul(elapsed_intervals);
    let shrunk = i128::from(saturation_kg).saturating_sub(recovered).max(0);
    i64::try_from(shrunk).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 디자인 §3.6 의 네 상장(기준가, 구간 kg) — 테스트 전용 상수, `data/` 를 읽지
    /// 않는다(스프린트 계약 §0.6 — 기대 숫자는 디자인 문서·식에서 독립적으로 온다).
    const FERROSITE: PriceRule = PriceRule {
        base_price_cr_per_kg: 10,
        band_kg: 2000,
        price_floor_pct: 30,
        price_drop_pct_per_band: 10,
    };
    const GLACINE: PriceRule = PriceRule {
        base_price_cr_per_kg: 14,
        band_kg: 1500,
        price_floor_pct: 30,
        price_drop_pct_per_band: 10,
    };
    const COBALTINE: PriceRule = PriceRule {
        base_price_cr_per_kg: 40,
        band_kg: 600,
        price_floor_pct: 30,
        price_drop_pct_per_band: 10,
    };
    const STARFALL_GLASS: PriceRule = PriceRule {
        base_price_cr_per_kg: 250,
        band_kg: 100,
        price_floor_pct: 30,
        price_drop_pct_per_band: 10,
    };

    #[test]
    fn floor_band_is_seven_for_the_common_rule_shape() {
        // ceil((100-30)/10) = 7 — 네 상장이 공통으로 쓰는 디자인 §3.3 값.
        assert_eq!(FERROSITE.floor_band(), 7);
        assert_eq!(GLACINE.floor_band(), 7);
        assert_eq!(COBALTINE.floor_band(), 7);
        assert_eq!(STARFALL_GLASS.floor_band(), 7);
    }

    // -------------------------------------------------------------------
    // design_table_* — §0.6 대금 표 P1~P10, 디자인 문서에서 옮긴 상수(AC-4(a)).
    // -------------------------------------------------------------------

    #[test]
    fn design_table_p1_ferrosite_12000kg_from_empty() {
        let result = sell(&FERROSITE, 0, 12_000);
        assert_eq!(result.proceeds_cr, 90_000);
    }

    #[test]
    fn design_table_p2_ferrosite_14000kg_from_empty() {
        let result = sell(&FERROSITE, 0, 14_000);
        assert_eq!(result.proceeds_cr, 98_000);
    }

    #[test]
    fn design_table_p3_glacine_10000kg_from_empty() {
        let result = sell(&GLACINE, 0, 10_000);
        assert_eq!(result.proceeds_cr, 96_500);
    }

    #[test]
    fn design_table_p4_glacine_10500kg_from_empty() {
        let result = sell(&GLACINE, 0, 10_500);
        assert_eq!(result.proceeds_cr, 99_000);
    }

    #[test]
    fn design_table_p5_cobaltine_3200kg_from_empty() {
        let result = sell(&COBALTINE, 0, 3_200);
        assert_eq!(result.proceeds_cr, 100_000);
    }

    #[test]
    fn design_table_p6_cobaltine_2400kg_from_empty() {
        let result = sell(&COBALTINE, 0, 2_400);
        assert_eq!(result.proceeds_cr, 81_600);
    }

    #[test]
    fn design_table_p7_starfall_glass_500kg_from_empty() {
        let result = sell(&STARFALL_GLASS, 0, 500);
        assert_eq!(result.proceeds_cr, 100_000);
    }

    #[test]
    fn design_table_p8_starfall_glass_700kg_from_empty() {
        let result = sell(&STARFALL_GLASS, 0, 700);
        assert_eq!(result.proceeds_cr, 122_500);
    }

    #[test]
    fn design_table_p9_ferrosite_2000kg_from_empty() {
        let result = sell(&FERROSITE, 0, 2_000);
        assert_eq!(result.proceeds_cr, 20_000);
        // P10 의 전제 — P9 뒤 포화가 2,000이다.
        assert_eq!(result.saturation_after_kg, 2_000);
    }

    #[test]
    fn design_table_p10_ferrosite_2000kg_after_p9() {
        let result = sell(&FERROSITE, 2_000, 2_000);
        assert_eq!(result.proceeds_cr, 18_000);
    }

    // -------------------------------------------------------------------
    // split_invariance_* — 분할 무관(I-92(1), AC-4(b), T-7).
    // -------------------------------------------------------------------

    #[test]
    fn split_invariance_arbitrary_splits_sum_to_the_same_proceeds() {
        // 시드 — 의사난수 없이 손으로 고른 분할 집합. 구간 경계(2,000의 배수)를
        // 걸치는 분할을 포함한다(1,500 + 1,000 등, qa 요구).
        let total = sell(&FERROSITE, 0, 12_000).proceeds_cr;
        let splits: &[&[i64]] = &[
            &[12_000],
            &[6_000, 6_000],
            &[1_500, 1_000, 9_500],
            &[2_000, 2_000, 2_000, 2_000, 2_000, 2_000],
            &[1, 11_999],
            &[11_999, 1],
            &[3_333, 3_333, 3_334, 2_000],
        ];
        for split in splits {
            let mut saturation = 0;
            let mut sum = 0;
            let mut crossed_a_boundary = false;
            for &chunk in *split {
                let before_band = FERROSITE.band(saturation);
                let result = sell(&FERROSITE, saturation, chunk);
                if FERROSITE.band(result.saturation_after_kg) != before_band {
                    crossed_a_boundary = true;
                }
                sum += result.proceeds_cr;
                saturation = result.saturation_after_kg;
            }
            assert_eq!(sum, total, "분할 {split:?} 의 합이 한 번 판매와 달라졌다");
            if split.len() > 1 {
                assert!(
                    crossed_a_boundary || split == &[1, 11_999] || split == &[11_999, 1],
                    "분할 {split:?} 이 구간 경계를 하나도 안 걸쳘 걸치면 분할 무관의 \
                     의미 있는 검사가 아니다"
                );
            }
        }
    }

    // -------------------------------------------------------------------
    // monotonic_* — 단조(I-92(2), AC-4(c)).
    // -------------------------------------------------------------------

    #[test]
    fn monotonic_same_quantity_higher_saturation_yields_less_or_equal_proceeds() {
        // 같은 양(1,000kg), 포화가 다른 구간(0 과 6,000 — 서로 다른 band).
        let low = sell(&FERROSITE, 0, 1_000).proceeds_cr;
        let high = sell(&FERROSITE, 6_000, 1_000).proceeds_cr;
        assert!(
            FERROSITE.band(0) != FERROSITE.band(6_000),
            "비교 쌍이 같은 구간 안이면 안 된다"
        );
        assert!(
            high <= low,
            "포화가 높은데 대금이 더 크다: low={low} high={high}"
        );
    }

    #[test]
    fn monotonic_same_saturation_more_quantity_yields_greater_or_equal_proceeds() {
        let small = sell(&FERROSITE, 0, 1_000).proceeds_cr;
        let large = sell(&FERROSITE, 0, 5_000).proceeds_cr;
        assert!(
            large >= small,
            "양이 많은데 대금이 더 작다: small={small} large={large}"
        );
    }

    // -------------------------------------------------------------------
    // floor_and_cap_* — 유계·바닥(I-92(3), AC-4(d)).
    // -------------------------------------------------------------------

    #[test]
    fn floor_and_cap_overflowing_sale_takes_the_floor_price_and_caps_saturation() {
        let cap = FERROSITE.saturation_cap();
        assert_eq!(cap, 14_000);
        // 상한 − 시작 포화(0)보다 큰 양 — 반드시 바닥 구간에 닿는다.
        let result = sell(&FERROSITE, 0, 20_000);
        assert_eq!(result.saturation_after_kg, cap);
        assert_eq!(
            result.price_after_cr_per_kg,
            FERROSITE.floor_price_cr_per_kg()
        );
        // 바닥 take 가 실제로 0 보다 크다(양성 — 바닥에 안 닿는 양만 시험하지 않는다).
        let at_floor_start = sell(&FERROSITE, cap - 1, 20_000);
        assert_eq!(at_floor_start.saturation_after_kg, cap);
    }

    #[test]
    fn floor_and_cap_saturation_stays_within_0_and_cap() {
        for &(s, q) in &[
            (0, 1),
            (0, 20_000),
            (13_999, 1),
            (13_999, 2),
            (7_000, 7_000),
        ] {
            let result = sell(&FERROSITE, s, q);
            assert!(result.saturation_after_kg >= 0);
            assert!(result.saturation_after_kg <= FERROSITE.saturation_cap());
        }
    }

    // -------------------------------------------------------------------
    // recovery_formula_equals_per_tick_simulation — I-87, AC-4(e), T-8.
    // -------------------------------------------------------------------

    #[test]
    fn recovery_formula_equals_per_tick_simulation() {
        let recovery_kg = 1_000;
        let interval_ticks = 1_200; // 60s @ 20Hz.
        let starting_saturation = 14_000;
        let as_of_tick: u64 = 0;

        // tick 별 메모리 시뮬레이션(닫힌 식을 호출하지 않는 별도 구현).
        let mut stepwise = starting_saturation;
        let mut last_interval_index = as_of_tick / interval_ticks;
        let mut boundaries_crossed = 0u32;

        for now_tick in as_of_tick..=(as_of_tick + 3 * interval_ticks) {
            let interval_index = now_tick / interval_ticks;
            if interval_index > last_interval_index {
                stepwise = (stepwise
                    - recovery_kg * i64::try_from(interval_index - last_interval_index).unwrap())
                .max(0);
                boundaries_crossed += u32::try_from(interval_index - last_interval_index).unwrap();
                last_interval_index = interval_index;
            }
            let closed_form = effective_saturation(
                starting_saturation,
                as_of_tick,
                now_tick,
                recovery_kg,
                interval_ticks,
            );
            assert_eq!(
                closed_form, stepwise,
                "tick {now_tick}: 닫힌 식({closed_form}) != tick별 시뮬레이션({stepwise})"
            );
        }
        assert!(
            boundaries_crossed >= 2,
            "실제로 배수 tick 을 2 개 이상 지나야 한다"
        );
        // T-8 — 바닥 14,000 → 1,200 tick 뒤 13,000(시세 4) → 2,400 tick 뒤 12,000.
        let after_one = effective_saturation(
            starting_saturation,
            as_of_tick,
            interval_ticks,
            recovery_kg,
            interval_ticks,
        );
        assert_eq!(after_one, 13_000);
        assert_eq!(FERROSITE.price_at_band(FERROSITE.band(after_one)), 4);
        let after_two = effective_saturation(
            starting_saturation,
            as_of_tick,
            2 * interval_ticks,
            recovery_kg,
            interval_ticks,
        );
        assert_eq!(after_two, 12_000);
    }

    // -------------------------------------------------------------------
    // proceeds_at_least_quantity_* — I-92(4), AC-4(f).
    // -------------------------------------------------------------------

    #[test]
    fn proceeds_at_least_quantity_across_boundary_saturations_and_quantities() {
        let rules = [FERROSITE, GLACINE, COBALTINE, STARFALL_GLASS];
        let mut checked = 0usize;
        for rule in rules {
            let cap = rule.saturation_cap();
            for saturation in [0, cap - 1, cap] {
                for quantity in [1, rule.band_kg, i64::from(i32::MAX)] {
                    let result = sell(&rule, saturation, quantity);
                    assert!(
                        result.proceeds_cr >= quantity,
                        "대금이 판매량보다 작다: rule={rule:?} s={saturation} q={quantity} proceeds={}",
                        result.proceeds_cr
                    );
                    checked += 1;
                }
            }
        }
        assert_eq!(
            checked, 36,
            "네 상장 × 경계 포화 3 × 양 3 = 36 조합을 모두 검사해야 한다"
        );
    }

    // -------------------------------------------------------------------
    // band_walk_iterations_* — ADR-0016 §4, AC-4(g), SC-20.
    // -------------------------------------------------------------------

    #[test]
    fn band_walk_iterations_never_exceeds_floor_band_plus_one_even_at_mass_kg_max() {
        let result = sell(&FERROSITE, 0, i64::from(i32::MAX));
        assert!(result.band_walk_iterations <= FERROSITE.floor_band() as u32 + 1);
        assert_eq!(
            result.band_walk_iterations, 8,
            "포화 0, i32::MAX kg 판매는 8회여야 한다"
        );
    }

    #[test]
    fn band_walk_iterations_one_exact_band_fill() {
        let result = sell(&FERROSITE, 0, 2_000);
        assert_eq!(result.band_walk_iterations, 1);
    }

    #[test]
    fn band_walk_iterations_eight_when_crossing_every_band_plus_one_kg() {
        let result = sell(&FERROSITE, 0, 14_001);
        assert_eq!(result.band_walk_iterations, 8);
    }
}

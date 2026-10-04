//! 채굴 판정에 필요한 정적 상수·순수 계산 (스펙 p1-02 §4.2·§4.6·§4.3, ADR-0013).
//!
//! 이 모듈은 `Simulation` 의 상태를 모른다 — 값을 받아 값을 돌려주는 순수 함수와, `data/`
//! 에서 온 불변 상수 타입만 있다. 판정 흐름 자체(8단계, 이벤트·메시지 생성, 재고·매장지
//! 상태 변경)는 `simulation.rs` 가 소유한다(그 파일이 이미 `Simulation::step` 의 유일한
//! 소유자다, I-13). 여기 있는 함수는 시계를 읽지 않고 난수도 쓰지 않는다(I-58) — `tick`
//! 은 언제나 호출자가 넘긴다.

use starfall_contracts::primitives::{DataId, UuidV7};

use crate::world::Vec3;

/// `data/minerals/*.json` 에서 오는 광물 하나의 채굴 상수.
#[derive(Debug, Clone, Copy)]
pub struct MineralConstants {
    /// 채굴 1회당 산출(스키마 최소 1, S2 검산 완료).
    pub yield_per_extraction_kg: i64,
    /// 회복 간격마다 늘어나는 양.
    pub regen_kg: i64,
    /// 회복 간격(tick). `regen_interval_s * tick_hz`를 S2가 유도해 넣는다(정수여야
    /// 한다는 기동 검산도 S2 몫).
    pub regen_interval_ticks: u64,
}

/// `data/world/deposits/*.json` 의 매장지 하나.
#[derive(Debug, Clone)]
pub struct DepositConstants {
    /// 이 매장지가 내는 광물 — **서버 전용 진실**(I-68, 드러나기 전 클라이언트에 닿지
    /// 않는다).
    pub mineral_id: DataId,
    /// 성계 로컬 좌표.
    pub position_m: Vec3,
    /// 채굴 판정용 반지름.
    pub radius_m: f64,
    /// 원 매장량 — **서버 전용 진실**.
    pub initial_reserve_kg: i64,
}

/// `data/mining/mining-rules.json` — 채굴 판정 세 수치(스펙 §4.2 #4~6).
#[derive(Debug, Clone, Copy)]
pub struct MiningRuleConstants {
    /// 매장지 표면에서부터의 채굴 사거리.
    pub mining_range_from_surface_m: f64,
    /// 채굴 허용 최대 함선 속력.
    pub max_ship_speed_mps: f64,
    /// 채굴 쿨다운(tick). `cooldown_s * tick_hz`를 S2가 유도해 넣는다.
    pub cooldown_ticks: u64,
}

/// 매장지의 지금까지 채굴 이력. **첫 채굴 전에는 존재하지 않는다**(I-68 "드러남" —
/// `Simulation` 이 맵에 항목이 없는 것으로 미확인을 표현한다).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DepositRuntimeState {
    /// `as_of_tick` 시점 기준 잔량(회복 미적용 — [`effective_remaining`] 이 적용한다).
    pub remaining_kg: i64,
    /// `remaining_kg` 를 기록한 tick.
    pub as_of_tick: u64,
    /// 이 매장지를 처음 드러낸 채굴의 tick. 드러난 뒤에는 절대 바뀌지 않는다.
    pub first_extracted_tick: u64,
}

/// 게으른 회복(I-69): `effective(t) = min(initial, remaining + regen_kg *
/// (floor(t/I) - floor(as_of/I)))`. DB 쓰기도 도메인 이벤트도 만들지 않는다 — 순수 계산.
///
/// `interval_ticks == 0` 이거나 `now_tick <= as_of_tick` 이면 회복 없이 그대로(단, 초기
/// 매장량 상한은 항상 지킨다) 돌려준다. 서버가 내려가 있던 동안은 tick 이 흐르지
/// 않았으므로 그 구간의 "실시간"은 회복에 반영되지 않는다(스펙 §4.6 I-69 마지막 줄).
#[must_use]
pub fn effective_remaining(
    remaining_kg: i64,
    as_of_tick: u64,
    now_tick: u64,
    regen_kg: i64,
    interval_ticks: u64,
    initial_reserve_kg: i64,
) -> i64 {
    if interval_ticks == 0 || now_tick <= as_of_tick {
        return remaining_kg.min(initial_reserve_kg);
    }
    let elapsed_intervals =
        i128::from(now_tick / interval_ticks) - i128::from(as_of_tick / interval_ticks);
    if elapsed_intervals <= 0 {
        return remaining_kg.min(initial_reserve_kg);
    }
    let regenerated = i128::from(regen_kg).saturating_mul(elapsed_intervals);
    let grown = i128::from(remaining_kg).saturating_add(regenerated);
    let capped = grown.min(i128::from(initial_reserve_kg));
    i64::try_from(capped).unwrap_or(initial_reserve_kg)
}

/// 사거리 판정: `|ship - deposit|² <= (radius_m + mining_range_from_surface_m)²`
/// (스펙 §4.2 #4) — 제곱 비교, `sqrt` 없음(ADR-0010 §3).
#[must_use]
pub fn within_mining_range(
    ship_position_m: Vec3,
    deposit_position_m: Vec3,
    deposit_radius_m: f64,
    mining_range_from_surface_m: f64,
) -> bool {
    let dx = ship_position_m.x - deposit_position_m.x;
    let dy = ship_position_m.y - deposit_position_m.y;
    let dz = ship_position_m.z - deposit_position_m.z;
    let distance_squared = dx * dx + dy * dy + dz * dz;
    let limit = deposit_radius_m + mining_range_from_surface_m;
    distance_squared <= limit * limit
}

/// 속도 판정: `|velocity|² <= max_ship_speed_mps²`(스펙 §4.2 #5).
#[must_use]
pub fn within_speed_limit(ship_velocity_mps: Vec3, max_ship_speed_mps: f64) -> bool {
    let speed_squared = ship_velocity_mps.x * ship_velocity_mps.x
        + ship_velocity_mps.y * ship_velocity_mps.y
        + ship_velocity_mps.z * ship_velocity_mps.z;
    speed_squared <= max_ship_speed_mps * max_ship_speed_mps
}

/// 영속화가 비교 후 쓰기(compare-and-set)에 쓸 상태 변경 하나(ADR-0013 §5, T0/S3 합의
/// K3 — `_workspace/p1-02-mining/02_server_ack.md` §1).
///
/// **여기에는 게임 규칙이 없다** — 저장된 표현과 비교할 "before/after" 값만 담는다.
/// 영속화 계층은 이 값을 그대로 SQL 조건으로 옮길 뿐, 회복식이나 checked 덧셈을 다시
/// 구현하지 않는다(persistence 는 `data/` 를 모른다).
#[derive(Debug, Clone)]
pub enum StateWrite {
    /// 인벤토리 행 하나.
    Inventory {
        /// 행위자.
        actor_id: UuidV7,
        /// 광물.
        mineral_id: DataId,
        /// 저장된 값이 이것과 같아야 쓴다. `None` = "행이 없어야 한다"(`INSERT`).
        expected: Option<i64>,
        /// 새 값.
        new: i64,
    },
    /// 매장지 상태 행 하나.
    Deposit {
        /// 매장지.
        deposit_id: DataId,
        /// 저장된 `(remaining_kg, as_of_tick)` 이 이것과 같아야 쓴다. `None` = "행이 없어야
        /// 한다"(첫 채굴 — `INSERT`).
        expected: Option<(i64, u64)>,
        /// 새 `(remaining_kg, as_of_tick)`.
        new: (i64, u64),
        /// 그 매장지의 `first_extracted_tick`(첫 채굴이면 지금 tick, 아니면 기존 값 — 절대
        /// 바뀌지 않는다).
        first_extracted_tick: u64,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effective_remaining_caps_at_initial_reserve() {
        // 10 tick 간격마다 5kg 회복, 초기 100kg, 40kg @ as_of=0 → now=1000(100 구간)
        // → 40 + 5*100 = 540 → 100으로 클램프.
        let value = effective_remaining(40, 0, 1000, 5, 10, 100);
        assert_eq!(value, 100);
    }

    #[test]
    fn effective_remaining_is_identity_within_the_same_interval() {
        let value = effective_remaining(40, 995, 999, 5, 10, 100);
        assert_eq!(value, 40, "같은 회복 구간 안에서는 회복이 일어나지 않는다");
    }

    #[test]
    fn effective_remaining_applies_partial_intervals() {
        // as_of=0, now=25, interval=10 → floor(25/10)-floor(0/10) = 2 구간 → +10.
        let value = effective_remaining(50, 0, 25, 5, 10, 1000);
        assert_eq!(value, 60);
    }

    #[test]
    fn effective_remaining_never_regresses_when_now_is_before_as_of() {
        // 재생·재시도 방어 — 미래 시점에서 계산한 값을 과거로 다시 물으면 그대로.
        let value = effective_remaining(60, 25, 0, 5, 10, 1000);
        assert_eq!(value, 60);
    }

    /// SC-14(qa 지적) — 닫힌 식(`effective_remaining`)이 **tick 별로 하나씩 회복을
    /// 적용하는 메모리 시뮬레이션**과 모든 tick에서 같은 값을 낸다. `as_of_tick`을
    /// 회복 경계(10의 배수)에 걸치지 않게 둬서(`⌊as_of/I⌋` 항이 0이 아닌 경우 — 스펙이
    /// 명시한 경우) 첫 부분 구간도 함께 검사하고, 구간 경계를 2개 이상 지나 상한
    /// (100)에도 실제로 도달하는 범위까지 본다.
    #[test]
    fn regen_closed_form_matches_stepwise_simulation() {
        let regen_kg = 5;
        let interval_ticks = 10;
        let initial_reserve = 100;
        let as_of_tick: u64 = 13; // 경계(10의 배수)가 아니다 — ⌊13/10⌋ = 1.
        let starting_remaining = 40;

        // tick 별 메모리 시뮬레이션: 매 tick마다 "이 tick이 경계를 막 지났는가"만
        // 본다(`effective_remaining`을 호출하지 않는 별도 구현 — 이게 없으면 함수를
        // 자기 자신과 비교하는 것일 뿐이다).
        let mut stepwise = starting_remaining;
        let mut last_interval_index = as_of_tick / interval_ticks;
        let boundaries_crossed_at_least = 2;
        let mut boundaries_crossed = 0u32;
        let mut reached_cap = false;

        for now_tick in as_of_tick..=(as_of_tick + 250) {
            let interval_index = now_tick / interval_ticks;
            if interval_index > last_interval_index {
                stepwise = (stepwise
                    + regen_kg * i64::try_from(interval_index - last_interval_index).unwrap())
                .min(initial_reserve);
                boundaries_crossed += u32::try_from(interval_index - last_interval_index).unwrap();
                last_interval_index = interval_index;
            }
            if stepwise == initial_reserve {
                reached_cap = true;
            }

            let closed_form = effective_remaining(
                starting_remaining,
                as_of_tick,
                now_tick,
                regen_kg,
                interval_ticks,
                initial_reserve,
            );
            assert_eq!(
                closed_form, stepwise,
                "tick {now_tick}: 닫힌 식({closed_form}) != tick별 시뮬레이션({stepwise})"
            );
        }

        assert!(
            boundaries_crossed >= boundaries_crossed_at_least,
            "⊘: 실제로 구간 경계를 2개 이상 지나야 한다(지난 수: {boundaries_crossed})"
        );
        assert!(reached_cap, "⊘: 실제로 상한(100)에 도달해야 한다");
    }

    #[test]
    fn within_mining_range_uses_squared_comparison_and_includes_the_boundary() {
        let deposit = Vec3::ZERO;
        assert!(within_mining_range(
            Vec3::new(10.0, 0.0, 0.0),
            deposit,
            5.0,
            5.0
        ));
        assert!(!within_mining_range(
            Vec3::new(10.01, 0.0, 0.0),
            deposit,
            5.0,
            5.0
        ));
    }

    #[test]
    fn within_speed_limit_includes_the_boundary() {
        assert!(within_speed_limit(Vec3::new(10.0, 0.0, 0.0), 10.0));
        assert!(!within_speed_limit(Vec3::new(10.001, 0.0, 0.0), 10.0));
    }
}

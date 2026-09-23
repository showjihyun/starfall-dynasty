//! ADR-0010 §2 의 적분 12단계 — **이 순서가 계약이다.** 손으로 바꾸지 않는다.
//!
//! 서버와 클라이언트 예측이 문장 하나하나를 대조할 수 있는 같은 법을 공유한다. 여기 없는
//! 것: `sin`·`cos`·`atan2`·`exp`·`mul_add`·`hypot`·`to_radians`·`to_degrees`·`signum`
//! (ADR-0010 §3 — 이 크레이트의 `Cargo.toml` 이 이미 수학 크레이트를 막는다).

use std::ops::{Add, Mul, Sub};

use super::quat::Quat;
use super::ship::ShipPhysicsState;
use super::vec3::Vec3;

/// 목표 자세 쿼터니언의 노름이 이 아래면 퇴화로 본다(현재 자세로 대체) — ADR-0009 §2,
/// server B-12 로 `1e-3` 에서 올렸다.
pub const AIM_MIN_NORM: f64 = 0.1;

/// 0 나눗셈 가드. `> 0` 이 아니라 `> EPS` 다(ADR-0010 §2 9단계 주석).
pub const EPS: f64 = 1e-9;

/// `deg → rad`. 리터럴을 손으로 적지 않고 `to_radians` 도 쓰지 않는다(ADR-0010 §2 고정 사항).
/// `std::f64::consts::PI` 는 두 언어 모두 올바르게 반올림된 같은 비트를 낸다(client 실측).
const DEG_TO_RAD: f64 = std::f64::consts::PI / 180.0;

/// 한 tick 의 조작 입력(전부 `f64`, ADR-0009 §2 의 역양자화를 거친 값).
#[derive(Debug, Clone, Copy)]
pub struct ControlInput {
    /// 함선 로컬 추력 의도, 각 성분 `-1.0..=1.0`.
    pub thrust: Vec3,
    /// 수동 롤 의도, `-1.0..=1.0`.
    pub roll: f64,
    /// 목표 자세(정규화 전 원값) — 퇴화 검사는 1단계가 한다.
    pub aim_raw: Quat,
    /// 브레이크.
    pub brake: bool,
    /// 비행 보조.
    pub assist: bool,
}

impl ControlInput {
    /// 휴면 입력(dormant input) — 이월 만료·잔류 중 쓰는 고정 입력(ADR-0011 §6.1).
    /// 추력·롤 0, 목표 자세는 **호출자가 넘긴 현재 자세**(그 tick의 `q`), `brake=false`,
    /// **`flight_assist=true`**. 조종사의 마지막 토글을 잇지 않는다 — 같은 잔류가
    /// 클라이언트 설정에 따라 달라지면 재현할 수 없기 때문이다.
    #[must_use]
    pub const fn dormant(current_attitude: Quat) -> Self {
        Self {
            thrust: Vec3::ZERO,
            roll: 0.0,
            aim_raw: current_attitude,
            brake: false,
            assist: true,
        }
    }
}

/// 함선 클래스의 적분 상수(`data/ships/*.json` 의 `movement` 블록 그대로, SI 단위).
#[derive(Debug, Clone, Copy)]
pub struct ShipClassConstants {
    /// 속도 상한(m/s).
    pub max_speed_mps: f64,
    /// 전방 최대 가속(m/s²). 대각선 클램프의 상한이기도 하다.
    pub main_thrust_mps2: f64,
    /// 후진 최대 가속(m/s²).
    pub reverse_thrust_mps2: f64,
    /// 측면 최대 가속(m/s²).
    pub lateral_thrust_mps2: f64,
    /// 브레이크 감쇠(m/s²).
    pub brake_decel_mps2: f64,
    /// 무추력 보조 감쇠(m/s²).
    pub assist_linear_decel_mps2: f64,
    /// 추력 방향 수직 성분 보조 감쇠(m/s²).
    pub assist_lateral_decel_mps2: f64,
    /// 자세 슬루 상한(deg/s).
    pub turn_rate_max_deg_s: f64,
    /// 각속도 변화 상한(deg/s²).
    pub turn_accel_deg_s2: f64,
    /// 자세 오차 비례 게인(`sin(θ/2)` 당 deg/s).
    pub turn_gain_deg_s_per_sin_half: f64,
    /// 자세 오차 불감대(`sin(θ/2)`).
    pub turn_deadzone_sin_half: f64,
    /// 수동 롤 상한(deg/s).
    pub roll_rate_max_deg_s: f64,
    /// 롤 각속도 변화 상한(deg/s²).
    pub roll_accel_deg_s2: f64,
    /// 오토레벨 상한(deg/s).
    pub auto_level_rate_deg_s: f64,
    /// 오토레벨 불감대(`sin`).
    pub auto_level_deadzone_sin: f64,
}

/// 경계 상수(`data/world/systems/*.json` 의 `play_area` 블록).
#[derive(Debug, Clone, Copy)]
pub struct BoundaryConstants {
    /// soft 경계(m). 이 밖이면 원점 방향 가속이 더해진다.
    pub soft_boundary_radius_m: f64,
    /// hard 경계(m). 벽.
    pub hard_boundary_radius_m: f64,
    /// soft 경계 밖에서 더해지는 가속(m/s²).
    pub boundary_pull_mps2: f64,
}

/// 한 tick 적분의 결과.
#[derive(Debug, Clone, Copy)]
pub struct IntegrateOutcome {
    /// 새 물리 상태.
    pub state: ShipPhysicsState,
    /// 1단계에서 목표 자세가 퇴화해 현재 자세로 대체됐는가(`aim_degenerate_total`, I-39).
    pub aim_degenerate: bool,
}

/// 한 tick, 한 함선 — ADR-0010 §2 의 12단계를 그대로 실행한다.
#[must_use]
pub fn step(
    state: ShipPhysicsState,
    input: &ControlInput,
    class: &ShipClassConstants,
    boundary: &BoundaryConstants,
    dt: f64,
) -> IntegrateOutcome {
    let mut p = state.p;
    let mut v = state.v;
    let mut q = state.q;
    let mut omega_aim = state.omega_aim;
    let mut omega_roll = state.omega_roll;

    // 1. 목표 자세 정규화 ----------------------------------------------------
    let n = input.aim_raw.norm();
    let (q_aim, aim_degenerate) = if n < AIM_MIN_NORM {
        (q, true)
    } else {
        (input.aim_raw.scale(1.0 / n), false)
    };

    // 2. 자세 오차 → 목표 각속도 (삼각함수 없음) ------------------------------
    let mut q_err = q_aim.mul(q.conj());
    if q_err.w < 0.0 {
        q_err = q_err.neg();
    }
    let e = q_err.vec();
    let s = e.length();
    let mut omega_t = if s < class.turn_deadzone_sin_half {
        Vec3::ZERO
    } else {
        let capped_gain = (class.turn_gain_deg_s_per_sin_half * s).min(class.turn_rate_max_deg_s);
        e.scale(capped_gain / s)
    };
    // 롤 축 권한 분리(server B-3) — 자세 제어기는 전방축에 손대지 않는다.
    let f = q.rotate(Vec3::FORWARD);
    omega_t = omega_t.sub(f.scale(omega_t.dot(f)));

    // 3. 각속도 슬루 (가속 제한) ----------------------------------------------
    let mut delta = omega_t.sub(omega_aim);
    let slew_limit = class.turn_accel_deg_s2 * dt;
    let delta_len = delta.length();
    if delta_len > slew_limit {
        delta = delta.scale(slew_limit / delta_len);
    }
    omega_aim = omega_aim.add(delta);

    // 4. 자세 적분 (1차 + 재정규화) --------------------------------------------
    let w1 = omega_aim.scale(DEG_TO_RAD);
    q = q.add(Quat::pure(w1).mul(q).scale(0.5 * dt));
    q = q.renormalize(EPS);

    // 5. 롤 / 오토레벨 (결과는 전방축 둘레의 회전 하나) -------------------------
    let fwd = q.rotate(Vec3::FORWARD);
    let omega_roll_t = if input.roll != 0.0 || !input.assist {
        input.roll * class.roll_rate_max_deg_s
    } else {
        let u = Vec3::WORLD_UP.sub(fwd.scale(Vec3::WORLD_UP.dot(fwd)));
        let nu = u.length();
        if nu < class.auto_level_deadzone_sin {
            0.0 // 기수가 성계 up 과 나란함 — 롤 기준 없음
        } else {
            let up_s = q.rotate(Vec3::WORLD_UP);
            // §2.1 주의: 이 외적의 부호가 손 방향이 새는 유일한 지점이다.
            let sin_err = up_s.cross(u.scale(1.0 / nu)).dot(fwd);
            if sin_err.abs() < class.auto_level_deadzone_sin {
                0.0
            } else {
                class.auto_level_rate_deg_s * sin_err
            }
        }
    };
    let mut roll_delta = omega_roll_t - omega_roll;
    let roll_limit = class.roll_accel_deg_s2 * dt;
    if roll_delta.abs() > roll_limit {
        // 부호 함수를 쓰지 않는다 — 0에서 두 언어가 갈린다(Rust 는 1.0, C# 의 Math.Sign 은 0).
        roll_delta = if roll_delta > 0.0 { 1.0 } else { -1.0 } * roll_limit;
    }
    omega_roll += roll_delta;
    let w2 = fwd.scale(omega_roll * DEG_TO_RAD);
    q = q.add(Quat::pure(w2).mul(q).scale(0.5 * dt));
    q = q.renormalize(EPS);

    // 6. 추력 → 월드 가속 ------------------------------------------------------
    let mut a_local = Vec3::new(
        input.thrust.x * class.lateral_thrust_mps2,
        input.thrust.y * class.lateral_thrust_mps2,
        if input.thrust.z >= 0.0 {
            input.thrust.z * class.main_thrust_mps2
        } else {
            input.thrust.z * class.reverse_thrust_mps2
        },
    );
    if input.brake {
        a_local = Vec3::ZERO; // 브레이크 중 추력 무시
    }
    let la = a_local.length();
    if la > class.main_thrust_mps2 {
        a_local = a_local.scale(class.main_thrust_mps2 / la); // 대각선 클램프
    }
    let a_thrust = q.rotate(a_local); // 5단계 후의 자세로 변환

    // 7. 경계 당김 --------------------------------------------------------------
    let r = p.length();
    let a_bound = if r > boundary.soft_boundary_radius_m && r > EPS {
        p.scale(-boundary.boundary_pull_mps2 / r)
    } else {
        Vec3::ZERO
    };

    // 8. 속도 적분 ----------------------------------------------------------------
    v = v.add(a_thrust.add(a_bound).scale(dt));

    // 9. 감쇠 — 한 tick에 정확히 하나만, 0을 지나치지 않게 ----------------------------
    let a_thrust_len = a_thrust.length();
    let (damp_u, damp_lim) = if input.brake {
        (v, class.brake_decel_mps2 * dt)
    } else if input.assist && a_thrust_len > EPS {
        let d = a_thrust.scale(1.0 / a_thrust_len);
        (
            v.sub(d.scale(v.dot(d))),
            class.assist_lateral_decel_mps2 * dt,
        )
    } else if input.assist {
        (v, class.assist_linear_decel_mps2 * dt)
    } else {
        (Vec3::ZERO, 0.0)
    };
    let damp_len = damp_u.length();
    if damp_len > EPS {
        v = v.sub(damp_u.scale(1.0 / damp_len).scale(damp_lim.min(damp_len)));
    }

    // 10. 속도 상한 ------------------------------------------------------------------
    let sv = v.length();
    if sv > class.max_speed_mps {
        v = v.scale(class.max_speed_mps / sv);
    }

    // 11. 위치 적분 -------------------------------------------------------------------
    p = p.add(v.scale(dt));

    // 12. 하드 경계 -------------------------------------------------------------------
    let r = p.length();
    if r > boundary.hard_boundary_radius_m && r > EPS {
        let n_dir = p.scale(1.0 / r);
        p = n_dir.scale(boundary.hard_boundary_radius_m);
        let vr = v.dot(n_dir);
        if vr > 0.0 {
            v = v.sub(n_dir.scale(vr));
        }
    }

    IntegrateOutcome {
        state: ShipPhysicsState {
            p,
            v,
            q,
            omega_aim,
            omega_roll,
        },
        aim_degenerate,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::quantise::quantise;

    /// `contracts/fixtures/SHIP_CLASS/example-scout.json` 의 실제 값.
    fn scout() -> ShipClassConstants {
        ShipClassConstants {
            max_speed_mps: 140.0,
            main_thrust_mps2: 35.0,
            reverse_thrust_mps2: 18.0,
            lateral_thrust_mps2: 18.0,
            brake_decel_mps2: 50.0,
            assist_linear_decel_mps2: 7.0,
            assist_lateral_decel_mps2: 22.0,
            turn_rate_max_deg_s: 75.0,
            turn_accel_deg_s2: 220.0,
            turn_gain_deg_s_per_sin_half: 290.0,
            turn_deadzone_sin_half: 0.0009,
            roll_rate_max_deg_s: 90.0,
            roll_accel_deg_s2: 300.0,
            auto_level_rate_deg_s: 40.0,
            auto_level_deadzone_sin: 0.002,
        }
    }

    fn cradle_boundary() -> BoundaryConstants {
        BoundaryConstants {
            soft_boundary_radius_m: 10_000.0,
            hard_boundary_radius_m: 12_000.0,
            boundary_pull_mps2: 25.0,
        }
    }

    const TICK_HZ: f64 = 20.0;
    const DT: f64 = 1.0 / TICK_HZ;

    fn no_input() -> ControlInput {
        ControlInput {
            thrust: Vec3::ZERO,
            roll: 0.0,
            aim_raw: Quat::IDENTITY,
            brake: false,
            assist: true,
        }
    }

    /// SC-15 / AC-4(a) — **이 값이 C3(클라이언트)의 첫 테스트와 같은 정수여야 한다.**
    /// 항등 자세에서 `thrust_z_milli=1000` 을 20 tick 적용하면 위치의 z 만 증가하고,
    /// 손계산한 양자화 정수와 일치한다.
    ///
    /// 손계산: 매 tick 가속 = 35.0 m/s² (`main_thrust_mps2`, 대각선 클램프 미적용 —
    /// 단일 축이라 `la == main_thrust_mps2`). 추력 방향과 속도가 평행하므로
    /// `assist_lateral` 감쇠의 수직 성분은 0 — 감쇠가 전진을 깎지 않는다(9단계).
    /// `v(n) = n * 35.0 * dt`, `p(n) = dt * Σ v(k) = dt² * 35.0 * n(n+1)/2`.
    /// `n=20`: `dt=0.05`, `p = 0.05² * 35.0 * 210 = 18.375 m = 18375 mm`.
    #[test]
    fn forward_thrust_only_moves_z_and_matches_hand_calculation() {
        let mut state = ShipPhysicsState::at_rest(Vec3::ZERO, Quat::IDENTITY);
        let class = scout();
        let boundary = cradle_boundary();
        let input = ControlInput {
            thrust: Vec3::new(0.0, 0.0, 1.0),
            roll: 0.0,
            aim_raw: Quat::IDENTITY, // 목표 자세 = 현재 자세(항등) — 회전 없음
            brake: false,
            assist: true,
        };

        for _ in 0..20 {
            let outcome = step(state, &input, &class, &boundary, DT);
            assert!(!outcome.aim_degenerate);
            state = outcome.state;
        }

        assert!(
            state.p.x.abs() < 1e-9,
            "x 는 그대로여야 한다: {}",
            state.p.x
        );
        assert!(
            state.p.y.abs() < 1e-9,
            "y 는 그대로여야 한다: {}",
            state.p.y
        );

        let quantised_z = quantise(state.p.z, 1000.0, -1_000_000_000_000, 1_000_000_000_000);
        println!(
            "[SC-15] 20 tick 후 p.z = {} m (양자화 {} mm), 손계산 기대 18375 mm",
            state.p.z, quantised_z
        );
        assert_eq!(quantised_z, 18_375, "C3 의 첫 테스트와 같은 정수여야 한다");
    }

    /// AC-4(b) — 최대 추력을 오래 넣어도 `|v|` 가 `max_speed_mps` 를 넘지 않는다.
    #[test]
    fn speed_never_exceeds_max_speed() {
        let mut state = ShipPhysicsState::at_rest(Vec3::ZERO, Quat::IDENTITY);
        let class = scout();
        let boundary = cradle_boundary();
        let input = ControlInput {
            thrust: Vec3::new(0.0, 0.0, 1.0),
            roll: 0.0,
            aim_raw: Quat::IDENTITY,
            brake: false,
            assist: true,
        };
        let mut max_seen: f64 = 0.0;
        for _ in 0..2000 {
            let outcome = step(state, &input, &class, &boundary, DT);
            state = outcome.state;
            max_seen = max_seen.max(state.v.length());
            assert!(
                state.v.length() <= class.max_speed_mps + 1e-9,
                "속도 상한 초과: {}",
                state.v.length()
            );
        }
        println!(
            "[AC-4b] 2000 tick 후 도달 최대 속도 = {max_seen} m/s (상한 {})",
            class.max_speed_mps
        );
        assert!(
            max_seen > class.max_speed_mps - 1.0,
            "충분히 오래 가속했는데 상한 근처에 못 갔다"
        );
    }

    /// AC-4(c) — 세 축 전부 1000인 입력의 로컬 가속 크기가 `main_thrust_mps2` 를 넘지 않는다
    /// (대각선 클램프). 클램프가 없으면 `sqrt(18²+18²+35²) ≈ 43.28` 가 나온다(server 실측).
    #[test]
    fn diagonal_thrust_is_clamped_to_main_thrust_magnitude() {
        let state = ShipPhysicsState::at_rest(Vec3::ZERO, Quat::IDENTITY);
        let class = scout();
        let boundary = cradle_boundary();
        let input = ControlInput {
            thrust: Vec3::new(1.0, 1.0, 1.0),
            roll: 0.0,
            aim_raw: Quat::IDENTITY,
            brake: false,
            assist: false, // 감쇠 없이 가속 크기만 본다
        };
        let outcome = step(state, &input, &class, &boundary, DT);
        // 첫 tick 의 가속 크기 = |Δv| / dt.
        let accel_mag = outcome.state.v.length() / DT;
        println!(
            "[AC-4c] 대각선 가속 크기 = {accel_mag} m/s² (상한 {})",
            class.main_thrust_mps2
        );
        assert!(
            accel_mag <= class.main_thrust_mps2 + 1e-6,
            "클램프가 동작하지 않았다: {accel_mag}"
        );
        assert!(
            accel_mag > 35.0 - 1e-6,
            "클램프가 과도하게 깎았다: {accel_mag}"
        );
    }

    /// AC-4(d) — hard 경계에서 반경 속도 성분이 0이 되고 접선 성분은 남는다(I-34). soft
    /// 경계 쪽(원점 방향 가속이 더해지는 것 + 그러면서도 조작이 계속 먹는 것)은
    /// `soft_boundary_pulls_toward_origin_without_blocking_thrust` 가 따로, 수치로 본다
    /// (QA 04_qa_report_r1.md §6.3 — SC-18: 전에는 이 테스트가 soft 가지도 타면서 아무것도
    /// 단언하지 않았다).
    #[test]
    fn hard_boundary_removes_only_radial_velocity() {
        let class = scout();
        let boundary = cradle_boundary();
        // 하드 경계 바로 안쪽, 바깥쪽을 향하는 속도로 시작.
        let state = ShipPhysicsState {
            p: Vec3::new(11_999.0, 0.0, 0.0),
            v: Vec3::new(100.0, 0.0, 50.0), // 반경 성분(+x) + 접선 성분(+z)
            q: Quat::IDENTITY,
            omega_aim: Vec3::ZERO,
            omega_roll: 0.0,
        };
        let outcome = step(state, &no_input(), &class, &boundary, DT);
        let r = outcome.state.p.length();
        println!(
            "[AC-4d] 경계 접촉 후 r={r} (하드 {}), v={:?}",
            boundary.hard_boundary_radius_m, outcome.state.v
        );
        assert!(r <= boundary.hard_boundary_radius_m + 1e-6);
        // 반경 방향 속도 성분은 0 이하(더 못 나간다), 접선(z) 성분은 남아 있어야 한다.
        let n_dir = outcome.state.p.normalize(1e-9).unwrap_or(Vec3::FORWARD);
        let radial_v = outcome.state.v.dot(n_dir);
        assert!(radial_v <= 1e-6, "반경 속도가 남아 있다: {radial_v}");
        assert!(outcome.state.v.z.abs() > 1e-6, "접선 성분이 사라졌다");
    }

    /// AC-4(d) / SC-18 — soft 경계를 넘으면 (a) 원점 방향 가속이 더해지고, (b) **같은
    /// tick에 추력 입력을 주면 그 추력도 여전히 반영된다**(조작은 계속 먹는다 — 경계가
    /// 조작을 빼앗지 않는다는 게임 규칙, QA §6.3 이 요구한 관찰). (b) 가 이 항목의 핵심:
    /// (a)만 있고 (b)가 없으면 이 테스트는 아무것도 증명하지 못한다.
    #[test]
    fn soft_boundary_pulls_toward_origin_without_blocking_thrust() {
        let class = scout();
        let boundary = cradle_boundary();
        // soft(10000) 밖, hard(12000) 안 — a_bound 가지가 반드시 돈다.
        let state = ShipPhysicsState {
            p: Vec3::new(10_500.0, 0.0, 0.0),
            v: Vec3::ZERO,
            q: Quat::IDENTITY,
            omega_aim: Vec3::ZERO,
            omega_roll: 0.0,
        };

        // (a) 추력 없이 한 tick — assist 를 꺼서 감쇠를 배제하고 경계 가속만 순수하게
        // 관찰한다. 시작 v=0 이므로 결과 v 는 정확히 a_bound * dt 다.
        let no_thrust_input = ControlInput {
            thrust: Vec3::ZERO,
            roll: 0.0,
            aim_raw: Quat::IDENTITY,
            brake: false,
            assist: false,
        };
        let pulled = step(state, &no_thrust_input, &class, &boundary, DT);
        let expected_pull_vx = -boundary.boundary_pull_mps2 * DT;
        println!(
            "[AC-4d-soft-a] 추력 없이 soft 밖 1 tick 후 v={:?} (기대 vx={expected_pull_vx}, 원점(−x) 방향)",
            pulled.state.v
        );
        assert!(
            (pulled.state.v.x - expected_pull_vx).abs() < 1e-9,
            "경계 가속이 boundary_pull_mps2 와 다르다: vx={}",
            pulled.state.v.x
        );
        assert!(pulled.state.v.x < 0.0, "가속이 원점(−x) 방향이 아니다");
        assert!(
            pulled.state.v.y.abs() < 1e-9 && pulled.state.v.z.abs() < 1e-9,
            "경계 가속이 x 축 밖으로 새고 있다: v={:?}",
            pulled.state.v
        );

        // (b) 같은 상황에서 전방 추력을 같이 준다 — 경계 당김과 추력이 **같은 tick에
        // 둘 다** 반영돼야 한다("조작은 계속 먹는다").
        let thrust_input = ControlInput {
            thrust: Vec3::new(0.0, 0.0, 1.0),
            roll: 0.0,
            aim_raw: Quat::IDENTITY,
            brake: false,
            assist: false,
        };
        let controlled = step(state, &thrust_input, &class, &boundary, DT);
        let expected_thrust_vz = class.main_thrust_mps2 * DT;
        println!(
            "[AC-4d-soft-b] soft 밖에서 전방 추력을 동시에 준 뒤 v={:?} (기대 vx={expected_pull_vx}, vz={expected_thrust_vz})",
            controlled.state.v
        );
        assert!(
            (controlled.state.v.z - expected_thrust_vz).abs() < 1e-9,
            "soft 경계가 조작을 막았다 — 추력이 반영되지 않았다: vz={}",
            controlled.state.v.z
        );
        assert!(
            (controlled.state.v.x - expected_pull_vx).abs() < 1e-9,
            "추력을 주니 경계 당김이 사라졌다: vx={}",
            controlled.state.v.x
        );
    }

    /// AC-4(e) — 조작을 멈추고 이월이 만료되면 가속이 0이 되지만 속도는 감쇠만 적용되어
    /// 즉시 0이 되지 않는다.
    #[test]
    fn coasting_decays_gradually_not_instantly() {
        let class = scout();
        let boundary = cradle_boundary();
        let state = ShipPhysicsState {
            p: Vec3::ZERO,
            v: Vec3::new(0.0, 0.0, 100.0),
            q: Quat::IDENTITY,
            omega_aim: Vec3::ZERO,
            omega_roll: 0.0,
        };
        let outcome = step(state, &no_input(), &class, &boundary, DT);
        let speed_after = outcome.state.v.length();
        println!("[AC-4e] 1 tick 후 속도 = {speed_after} (시작 100)");
        assert!(speed_after < 100.0, "감쇠가 전혀 없다");
        assert!(speed_after > 100.0 - class.assist_linear_decel_mps2 * DT - 1e-6);
    }

    /// AC-4(f) — 브레이크 중에는 추력이 무시되고 감쇠가 하나만 적용된다.
    #[test]
    fn brake_ignores_thrust_and_applies_single_damping() {
        let class = scout();
        let boundary = cradle_boundary();
        let state = ShipPhysicsState {
            p: Vec3::ZERO,
            v: Vec3::new(0.0, 0.0, 100.0),
            q: Quat::IDENTITY,
            omega_aim: Vec3::ZERO,
            omega_roll: 0.0,
        };
        let input = ControlInput {
            thrust: Vec3::new(0.0, 0.0, 1.0), // 추력을 넣어도
            roll: 0.0,
            aim_raw: Quat::IDENTITY,
            brake: true, // 브레이크가 이긴다
            assist: true,
        };
        let outcome = step(state, &input, &class, &boundary, DT);
        let expected = 100.0 - class.brake_decel_mps2 * DT;
        println!(
            "[AC-4f] 브레이크 1 tick 후 속도 = {} (기대 {expected})",
            outcome.state.v.z
        );
        assert!((outcome.state.v.z - expected).abs() < 1e-9);
    }

    /// AC-4(g) — 목표 자세를 정반대로 주면 오버슈트 없이 안착한다: 매 tick
    /// `|ω_aim| ≤ turn_rate_max_deg_s`, 자세 오차가 불감대 아래로 내려간 뒤 40 tick 동안
    /// 다시 올라가지 않는다. 안착 tick 수를 출력한다(U-21).
    #[test]
    fn attitude_settles_without_overshoot() {
        let class = scout();
        let boundary = cradle_boundary();
        // 180도 반대 목표: aim = (0,1,0,0) (Y축 둘레 180도) — 현재(항등)와 정반대.
        let mut state = ShipPhysicsState::at_rest(Vec3::ZERO, Quat::IDENTITY);
        let input = ControlInput {
            thrust: Vec3::ZERO,
            roll: 0.0,
            aim_raw: Quat::new(0.0, 1.0, 0.0, 0.0),
            brake: false,
            assist: true,
        };

        let mut settled_at: Option<usize> = None;
        let mut still_settled_since: usize = 0;
        for tick in 0..2000 {
            let outcome = step(state, &input, &class, &boundary, DT);
            state = outcome.state;
            assert!(
                state.omega_aim.length() <= class.turn_rate_max_deg_s + 1e-6,
                "tick {tick}: |ω_aim| 이 상한을 넘었다: {}",
                state.omega_aim.length()
            );

            let q_err = {
                let mut e = input
                    .aim_raw
                    .scale(1.0 / input.aim_raw.norm())
                    .mul(state.q.conj());
                if e.w < 0.0 {
                    e = e.neg();
                }
                e.vec().length()
            };

            if q_err < class.turn_deadzone_sin_half {
                if settled_at.is_none() {
                    settled_at = Some(tick);
                }
                still_settled_since += 1;
            } else if settled_at.is_some() {
                // 다시 올라갔다 — 안착이 아니었다.
                settled_at = None;
                still_settled_since = 0;
            }

            if still_settled_since >= 40 {
                break;
            }
        }

        let settled = settled_at.expect("2000 tick 안에 안착하지 못했다");
        println!("[AC-4g / U-21] 안착 tick 수 = {settled}");
        assert!(
            still_settled_since >= 40,
            "40 tick 동안 다시 올라가지 않아야 한다"
        );
    }

    /// AC-4(g2) — 오토레벨이 켜진 상태에서 목표 자세를 유지할 때 자세가 진동하지 않는다.
    #[test]
    fn auto_level_does_not_oscillate_when_holding_attitude() {
        let class = scout();
        let boundary = cradle_boundary();
        let mut state = ShipPhysicsState::at_rest(Vec3::ZERO, Quat::IDENTITY);
        let input = no_input(); // aim = 항등 = 현재 자세, roll=0, assist=true
        let mut max_roll_speed: f64 = 0.0;
        for _ in 0..200 {
            let outcome = step(state, &input, &class, &boundary, DT);
            state = outcome.state;
            max_roll_speed = max_roll_speed.max(state.omega_roll.abs());
        }
        println!("[AC-4g2] 200 tick 동안 최대 |ω_roll| = {max_roll_speed}");
        assert!(
            max_roll_speed < 1e-6,
            "이미 수평인데 오토레벨이 움직였다 — 진동 신호"
        );
    }

    /// AC-4(h) / I-39 — 퇴화 쿼터니언(전 성분 0)은 거부되지 않고 현재 자세 유지 +
    /// `aim_degenerate` 로 센다. 자세 제어기(2~4단계)의 목표 각속도가 0이어야 한다 —
    /// **오토레벨(5단계)은 별도로 계속 돈다**(시작 자세가 이미 수평이 아니면 그쪽은
    /// 움직인다. 그건 이 항목이 보는 것이 아니다).
    #[test]
    fn degenerate_aim_keeps_current_attitude_and_is_flagged() {
        let class = scout();
        let boundary = cradle_boundary();
        // 이미 수평인 자세로 시작한다 — 그래야 5단계(오토레벨)도 조용하고, 1~4단계만의
        // 효과(퇴화 처리)를 깨끗하게 관찰할 수 있다.
        let state = ShipPhysicsState::at_rest(Vec3::ZERO, Quat::IDENTITY);
        let input = ControlInput {
            thrust: Vec3::ZERO,
            roll: 0.0,
            aim_raw: Quat::new(0.0, 0.0, 0.0, 0.0),
            brake: false,
            assist: true,
        };
        let outcome = step(state, &input, &class, &boundary, DT);
        assert!(outcome.aim_degenerate);
        // 목표가 현재 자세와 같아졌으니 자세 제어기의 각속도는 0 이어야 한다.
        assert!(
            outcome.state.omega_aim.length() < 1e-9,
            "퇴화 시 자세 제어기가 움직였다: {:?}",
            outcome.state.omega_aim
        );
        // 이미 수평이므로 오토레벨도 조용해야 한다 — 자세가 거의 그대로다.
        let drift = outcome.state.q.vec().sub(state.q.vec()).length();
        assert!(drift < 1e-6, "퇴화 시 자세가 표류했다: {drift}");
    }
}

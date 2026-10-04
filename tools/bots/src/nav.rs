//! p1-02 채굴 봇의 비행 — 광맥 사거리 안으로 가서 멈춘다.
//!
//! 서버가 채굴을 판정하는 조건(`data/mining/*.json` 의 `extraction`): 함선 중심과 광맥 중심의 거리
//! `≤ radius_m + mining_range_from_surface_m`(150 m) 이고 `|v| ≤ max_ship_speed_mps`(10 m/s). 봇은
//! 위치를 보낼 수 없다(서버 권위) — `SET_SHIP_CONTROL` 의 **의도**(목표 자세·추력·브레이크)만으로
//! 거기에 도달해야 한다. 그래서 이 모듈은 스냅샷 한 장(위치·속도·자세)에서 다음 조작 하나를 고르는
//! **순수 함수**다. 테스트는 이 함수에 간단한 운동 모델을 물려 수렴을 본다 — 서버 물리와 같다는
//! 주장은 하지 않는다. 실서버에서 사거리 안 정지가 실제로 일어났는지는 채굴 수락으로 드러난다.
//!
//! 자세 규약: 쿼터니언 `q` 는 로컬 → 월드 회전(`world = q · local · q*`), 전진 축은 로컬 `+z`
//! (p1-01 스펙 AC-4(a) — 항등 자세에서 `thrust_z` 는 월드 `+z`). 스폰 자세 실측(원점을 향한다)과 맞다.

/// 3 성분 벡터 (m 또는 m/s).
pub type V3 = [f64; 3];

fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn dot(a: V3, b: V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn norm(a: V3) -> f64 {
    dot(a, a).sqrt()
}

/// 튜닝 값. 기본값은 `scout-s01`(브레이크 50 m/s², 최고 140 m/s)에 여유를 둔 것이다.
#[derive(Debug, Clone, Copy)]
pub struct NavTuning {
    /// 계획에 쓰는 감속 (m/s²) — 실제 브레이크(50)보다 작게 잡아 지연(스냅샷 100 ms + 송신)을 흡수한다.
    pub plan_decel_mps2: f64,
    /// 순항 속도 상한 (m/s).
    pub cruise_mps: f64,
    /// 이 각도(도) 안으로 기수가 목표를 향할 때만 추력을 준다.
    pub thrust_cone_deg: f64,
    /// 도착으로 볼 속도 (m/s) — 서버 조건 10 보다 충분히 작게.
    pub arrive_speed_mps: f64,
}

impl Default for NavTuning {
    fn default() -> Self {
        Self {
            plan_decel_mps2: 30.0,
            cruise_mps: 110.0,
            thrust_cone_deg: 15.0,
            arrive_speed_mps: 3.0,
        }
    }
}

/// 한 번의 조작 결정.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NavCmd {
    /// 목표 자세 (x, y, z, w) × 10⁶ — 계약 `aim_*_micro`.
    pub aim_micro: [i32; 4],
    /// `thrust_z_milli` (0 또는 1000).
    pub thrust_z_milli: i32,
    pub brake: bool,
    /// 정지 반경 안이고 충분히 느리다 — 채굴을 보내도 된다.
    pub arrived: bool,
}

/// 로컬 `+z` 를 월드 방향 `d` 로 보내는 최소 회전 쿼터니언 (x, y, z, w).
pub fn aim_toward(d: V3) -> [f64; 4] {
    let n = norm(d);
    if n < 1e-9 {
        return [0.0, 0.0, 0.0, 1.0];
    }
    let u = [d[0] / n, d[1] / n, d[2] / n];
    let c = u[2]; // (0,0,1)·u
    if c < -0.999_999 {
        // 정반대 — 아무 수직축으로 180°. y 축을 쓴다.
        return [0.0, 1.0, 0.0, 0.0];
    }
    // 축 = z × u = (-u.y, u.x, 0), |축| = sin θ. 반각 공식: q = (축, 1 + cos θ) 정규화.
    let q = [-u[1], u[0], 0.0, 1.0 + c];
    let qn = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt();
    [q[0] / qn, q[1] / qn, q[2] / qn, q[3] / qn]
}

/// 쿼터니언으로 로컬 `+z` 를 돌린 월드 방향.
pub fn forward_of(q: [f64; 4]) -> V3 {
    let [x, y, z, w] = q;
    // R · (0,0,1) = 회전행렬의 셋째 열.
    [
        2.0 * (x * z + w * y),
        2.0 * (y * z - w * x),
        1.0 - 2.0 * (x * x + y * y),
    ]
}

fn to_micro(q: [f64; 4]) -> [i32; 4] {
    q.map(|c| (c * 1_000_000.0).round().clamp(-1_000_000.0, 1_000_000.0) as i32)
}

/// 스냅샷 한 장에서 다음 조작을 고른다.
///
/// `stop_radius_m` — 광맥 중심에서 이 거리 안이면 "사거리 안" 으로 본다(호출자가
/// `radius_m + 150` 보다 충분히 작게 준다).
pub fn plan(
    pos: V3,
    vel: V3,
    orientation: [f64; 4],
    target: V3,
    stop_radius_m: f64,
    t: &NavTuning,
) -> NavCmd {
    let rel = sub(target, pos);
    let dist = norm(rel);
    let speed = norm(vel);
    let aim = aim_toward(rel);
    let remaining = dist - stop_radius_m;
    if remaining <= 0.0 {
        // 반경 안 — 멈춘다. 멈추면 도착.
        return NavCmd {
            aim_micro: to_micro(aim),
            thrust_z_milli: 0,
            brake: speed > 0.0,
            arrived: speed <= t.arrive_speed_mps,
        };
    }
    // 남은 거리에서 멈출 수 있는 속도. 목표 쪽 성분이 아니라 **전체 속력**으로 비교한다 —
    // 옆으로 흐르는 속도도 브레이크로만 없어진다.
    let v_allow = (2.0 * t.plan_decel_mps2 * remaining)
        .sqrt()
        .min(t.cruise_mps);
    // 목표에서 멀어지는 중이면 먼저 세운다(선회 중 관성으로 흘러가는 것을 막는다).
    let closing = if dist > 0.0 {
        dot(vel, rel) / dist
    } else {
        0.0
    };
    if speed > v_allow || (closing < -1.0 && speed > t.arrive_speed_mps) {
        return NavCmd {
            aim_micro: to_micro(aim),
            thrust_z_milli: 0,
            brake: true,
            arrived: false,
        };
    }
    let fwd = forward_of(orientation);
    let cos_err = dot(fwd, rel) / dist.max(1e-9) / norm(fwd).max(1e-9);
    let facing = cos_err >= t.thrust_cone_deg.to_radians().cos();
    NavCmd {
        aim_micro: to_micro(aim),
        thrust_z_milli: if facing { 1000 } else { 0 },
        brake: false,
        arrived: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aim_toward_sends_local_z_to_the_direction() {
        for d in [
            [1.0, 0.0, 0.0],
            [0.0, -1.0, 0.0],
            [0.3, 0.4, -0.866],
            [0.0, 0.0, 1.0],
            [0.0, 0.0, -1.0],
        ] {
            let f = forward_of(aim_toward(d));
            let n = norm(d);
            for i in 0..3 {
                assert!((f[i] - d[i] / n).abs() < 1e-9, "d={d:?} f={f:?}");
            }
        }
    }

    /// 실측 스폰 자세(원점을 향한다, 스폰 (2500, 250, 0) m)가 이 규약에서 원점 쪽을 가리킨다 —
    /// 규약이 서버와 같다는 약한 확인(수치는 SC-04 월드의 SHIP_SPAWNED payload).
    #[test]
    // 실측 값 그대로 둔다 — 0.707107 이 1/√2 인 것은 우연(90° 회전)이지 상수를 쓰려던 것이 아니다.
    #[allow(clippy::approx_constant)]
    fn observed_spawn_orientation_faces_origin_under_this_convention() {
        let q = [0.070_360, -0.703_598, 0.0, 0.707_107];
        let f = forward_of(q);
        let to_origin = [-2500.0, -250.0, 0.0];
        let c = dot(f, to_origin) / norm(f) / norm(to_origin);
        assert!(c > 0.999, "cos={c} f={f:?}");
    }

    /// 간이 운동 모델(추력 35 · 브레이크 50 · 선회 즉시)로 3 km 밖에서 출발해 수렴하는가.
    /// ⊘: 도착이 "처음부터 반경 안" 이어서가 아님 — 시작 거리를 단언한다.
    #[test]
    fn converges_inside_radius_and_slow_in_a_simple_model() {
        let target = [3000.0, 200.0, -1500.0];
        let (mut p, mut v): (V3, V3) = ([0.0; 3], [0.0; 3]);
        let stop = 120.0;
        assert!(norm(sub(target, p)) > 3000.0);
        let t = NavTuning::default();
        let dt: f64 = 0.05;
        let mut q = [0.0, 0.0, 0.0, 1.0];
        let mut arrived_at = None;
        for step in 0..20_000 {
            // 지연 없는 이상 모델이다(선회도 즉시) — 실서버의 지연·선회율은 여기서 재지 않는다.
            let cmd = plan(p, v, q, target, stop, &t);
            if cmd.arrived {
                arrived_at = Some(step);
                break;
            }
            q = cmd.aim_micro.map(|c| f64::from(c) / 1e6);
            let f = forward_of(q);
            let s = norm(v);
            if cmd.brake {
                let dv = (50.0_f64 * dt).min(s);
                if s > 0.0 {
                    v = [
                        v[0] - v[0] / s * dv,
                        v[1] - v[1] / s * dv,
                        v[2] - v[2] / s * dv,
                    ];
                }
            } else if cmd.thrust_z_milli > 0 {
                v = [
                    v[0] + f[0] * 35.0 * dt,
                    v[1] + f[1] * 35.0 * dt,
                    v[2] + f[2] * 35.0 * dt,
                ];
                let s2 = norm(v);
                if s2 > 140.0 {
                    v = v.map(|c| c / s2 * 140.0);
                }
            }
            p = [p[0] + v[0] * dt, p[1] + v[1] * dt, p[2] + v[2] * dt];
        }
        let step = arrived_at.expect("수렴하지 못했다");
        let d = norm(sub(target, p));
        assert!(d <= stop, "도착 거리 {d}");
        assert!(norm(v) <= 10.0, "도착 속도 {}", norm(v));
        assert!(step < 2000, "{step} 스텝(100 s) 안에 도착해야 한다");
    }

    #[test]
    fn inside_radius_but_fast_brakes_and_is_not_arrived() {
        let c = plan(
            [0.0; 3],
            [20.0, 0.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
            [10.0, 0.0, 0.0],
            100.0,
            &NavTuning::default(),
        );
        assert!(c.brake && !c.arrived && c.thrust_z_milli == 0);
    }

    #[test]
    fn not_facing_means_no_thrust() {
        // 기수가 +z, 목표는 +x — 선회가 먼저다.
        let c = plan(
            [0.0; 3],
            [0.0; 3],
            [0.0, 0.0, 0.0, 1.0],
            [2000.0, 0.0, 0.0],
            100.0,
            &NavTuning::default(),
        );
        assert_eq!(c.thrust_z_milli, 0);
        assert!(!c.brake && !c.arrived);
    }
}

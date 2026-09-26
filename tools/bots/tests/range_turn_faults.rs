//! `range_turn` 분석기 자체의 검증 (계약 §3.3 SC-85 규율 + §7a).
//!
//! 합성 시계열로 **틀렸을 때 빨간불이 켜지는가**를 본다. 특히 "조건이 안 생겼으면 통과가 아니다":
//! 리미터가 한 번도 안 걸린 선회, 거부가 없는 주입, 빈 입력은 전부 `holds = false` 여야 한다.

use starfall_bots::ledger::CommandOutcome;
use starfall_bots::range_turn::{Injected, Params, analyze, rotation_deg};
use starfall_bots::snapshot::OwnSample;

const P: Params = Params {
    tick_hz: 20,
    turn_rate_max_deg_s: 75.0,
    carry_forward_max_ticks: 10,
    tolerance_deg_per_tick: 0.001,
};

fn yaw_q(deg: f64) -> (i32, i32, i32, i32) {
    let h = deg.to_radians() / 2.0;
    (
        0,
        (h.sin() * 1e6).round() as i32,
        0,
        (h.cos() * 1e6).round() as i32,
    )
}

fn sample(
    tick: u64,
    speed_mm_s: i32,
    z_mm: i64,
    yaw_deg: f64,
    omega_y: i32,
    ack: u64,
) -> OwnSample {
    OwnSample {
        tick,
        position_mm: (0, 0, z_mm),
        velocity_mm_s: (0, 0, speed_mm_s),
        orientation_micro: yaw_q(yaw_deg),
        angular_velocity_mdeg_s: (0, omega_y, 0),
        angular_velocity_roll_mdeg_s: 0,
        ack_input_seq: Some(ack),
    }
}

fn rejected(tick: u64) -> Option<CommandOutcome> {
    Some(CommandOutcome {
        status: "REJECTED".to_owned(),
        reason_code: Some("MALFORMED_COMMAND".to_owned()),
        tick,
    })
}

/// 정상 실행의 모양: tick 100~118 가속, 120~123 에 주입 4건 거부(이월로 계속 가속),
/// 140 부터 한도(3.75°/tick)로 180° 선회 후 정지.
fn healthy() -> (Vec<OwnSample>, Vec<Injected>, Option<u64>) {
    let mut s = Vec::new();
    let mut z = 0i64;
    let mut yaw = 0.0;
    for tick in (100..=220).step_by(2) {
        let speed = if tick < 140 {
            1750 * (tick as i32 - 98) / 2
        } else {
            36_000
        };
        z += i64::from(speed) / 10;
        let mut omega = 0;
        if tick > 140 && yaw < 180.0 {
            yaw = (yaw + 7.5f64).min(180.0);
            omega = 75_000;
        }
        s.push(sample(tick, speed, z, yaw, omega, 20));
    }
    let inj = (0..4)
        .map(|i| Injected {
            input_seq: 21 + i,
            outcome: rejected(120 + i),
        })
        .collect();
    (s, inj, Some(140))
}

#[test]
fn rotation_angle_is_exact_and_sign_invariant() {
    let id = (0, 0, 0, 1_000_000);
    let flip = (0, 1_000_000, 0, 0);
    assert!((rotation_deg(id, flip).unwrap() - 180.0).abs() < 1e-6);
    assert!(
        rotation_deg(flip, (0, -1_000_000, 0, 0)).unwrap() < 1e-6,
        "q 와 -q 는 같은 방향"
    );
    let a = rotation_deg(yaw_q(10.0), yaw_q(13.75)).unwrap();
    assert!((a - 3.75).abs() < 1e-3, "작은 각도 정확해야 한다: {a}");
    assert!(
        rotation_deg((0, 0, 0, 0), id).is_none(),
        "퇴화 쿼터니언은 비교하지 않는다"
    );
}

#[test]
fn a_healthy_run_holds_both() {
    let (s, inj, start) = healthy();
    let r = analyze(&s, &inj, start, P);
    assert!(r.carry.holds, "{:?}", r.carry.why_not);
    assert!(r.turn.holds, "{:?}", r.turn.why_not);
    assert!(r.carry.window_pairs > 0 && r.turn.saturated_pairs > 0);
}

/// 입력이 전부 0 — 어떤 것도 통과하면 안 된다(계약 §7a).
#[test]
fn an_all_zero_input_holds_nothing() {
    let r = analyze(&[], &[], None, P);
    assert!(!r.carry.holds);
    assert!(!r.turn.holds);
}

#[test]
fn a_turn_that_never_reaches_the_limiter_is_not_evidence() {
    let (mut s, inj, start) = healthy();
    // 선회를 쌍당 1° 로 줄인다 — 한도에 닿지 않았으니 "넘지 않았다"는 아무것도 뜻하지 않는다.
    let mut yaw = 0.0;
    for x in s.iter_mut().filter(|x| x.tick > 140) {
        yaw += 1.0;
        x.orientation_micro = yaw_q(yaw);
        x.angular_velocity_mdeg_s = (0, 10_000, 0);
    }
    let r = analyze(&s, &inj, start, P);
    assert_eq!(r.turn.saturated_pairs, 0);
    assert_eq!(
        r.turn.over_limit_pairs, 0,
        "넘지는 않았다 — 그래도 통과가 아니다"
    );
    assert!(!r.turn.holds);
}

#[test]
fn one_pair_over_the_limit_fails_the_turn() {
    let (mut s, inj, start) = healthy();
    let i = s.iter().position(|x| x.tick == 150).unwrap();
    let base = 7.5 * 5.0; // tick 150 의 정상 yaw
    s[i].orientation_micro = yaw_q(base + 0.6); // 이 쌍만 8.1° / 2 tick = 4.05°/tick
    let r = analyze(&s, &inj, start, P);
    assert!(r.turn.over_limit_pairs >= 1, "{:?}", r.turn);
    assert!(!r.turn.holds);
}

#[test]
fn omega_field_over_the_limit_fails_the_turn() {
    let (mut s, inj, start) = healthy();
    let i = s.iter().position(|x| x.tick == 160).unwrap();
    s[i].angular_velocity_mdeg_s = (0, 75_002, 0);
    let r = analyze(&s, &inj, start, P);
    assert_eq!(r.turn.omega_aim_over_limit, 1);
    assert!(!r.turn.holds);
}

#[test]
fn a_speed_drop_while_rejected_means_the_carry_broke() {
    let (mut s, inj, start) = healthy();
    let i = s.iter().position(|x| x.tick == 122).unwrap();
    s[i].velocity_mm_s.2 -= 3_000; // 추력이 끊겨 감쇠만 걸린 모양
    let r = analyze(&s, &inj, start, P);
    assert!(r.carry.speed_drops >= 1);
    assert!(!r.carry.holds);
}

#[test]
fn an_injected_seq_showing_up_as_ack_is_a_clamp_trace() {
    let (mut s, inj, start) = healthy();
    let i = s.iter().position(|x| x.tick == 124).unwrap();
    s[i].ack_input_seq = Some(22); // 주입한 번호
    let r = analyze(&s, &inj, start, P);
    assert_eq!(r.carry.injected_seq_acked, 1);
    assert!(!r.carry.holds);
}

#[test]
fn an_injected_frame_that_was_accepted_fails_the_carry() {
    let (s, mut inj, start) = healthy();
    inj[1].outcome = Some(CommandOutcome {
        status: "ACCEPTED".to_owned(),
        reason_code: None,
        tick: 121,
    });
    let r = analyze(&s, &inj, start, P);
    assert_eq!(r.carry.other_outcomes, 1);
    assert!(!r.carry.holds);
}

#[test]
fn injected_frames_with_no_result_fail_the_carry() {
    let (s, mut inj, start) = healthy();
    for i in &mut inj {
        i.outcome = None;
    }
    let r = analyze(&s, &inj, start, P);
    assert_eq!(r.carry.missing_results, 4);
    assert_eq!(r.carry.window_pairs, 0, "거부가 없으면 창도 없다");
    assert!(!r.carry.holds);
}

#[test]
fn a_reject_span_longer_than_the_carry_window_is_a_probe_design_error() {
    let (s, mut inj, start) = healthy();
    inj[3].outcome = rejected(131); // 120..131 = 11 tick ≥ 10
    let r = analyze(&s, &inj, start, P);
    assert!(!r.carry.span_within_carry);
    assert!(!r.carry.holds);
}

/// 라운드 3 실서버에서 본 모양: 조준(요)이 한도로 도는 **동안** 오토레벨 롤이 같이 돈다.
/// 전체 자세 회전은 조준 한도를 넘지만 뱃머리 회전은 넘지 않는다 — **(d) 는 뱃머리로 판정한다.**
#[test]
fn roll_on_top_of_a_saturated_yaw_is_not_an_aim_violation() {
    let (mut s, inj, start) = healthy();
    let mut roll = 0.0f64;
    for x in s.iter_mut().filter(|x| x.tick > 140) {
        roll += 1.6; // 2 tick 당 1.6° = 16 °/s 롤
        let (yx, yy, yz, yw) = x.orientation_micro;
        let q_yaw = [f64::from(yx), f64::from(yy), f64::from(yz), f64::from(yw)].map(|v| v / 1e6);
        let h = roll.to_radians() / 2.0;
        let q_roll = [0.0, 0.0, h.sin(), h.cos()];
        // q = q_yaw * q_roll (로컬 Z 둘레 롤을 요 뒤에 합성)
        let [ax, ay, az, aw] = q_yaw;
        let [bx, by, bz, bw] = q_roll;
        let q = [
            aw * bx + ax * bw + ay * bz - az * by,
            aw * by - ax * bz + ay * bw + az * bx,
            aw * bz + ax * by - ay * bx + az * bw,
            aw * bw - ax * bx - ay * by - az * bz,
        ];
        x.orientation_micro = (
            (q[0] * 1e6).round() as i32,
            (q[1] * 1e6).round() as i32,
            (q[2] * 1e6).round() as i32,
            (q[3] * 1e6).round() as i32,
        );
    }
    let r = analyze(&s, &inj, start, P);
    assert!(
        r.turn.total_over_limit_pairs > 0,
        "롤을 합친 전체 회전은 한도를 넘는다: {:?}",
        r.turn
    );
    assert_eq!(
        r.turn.over_limit_pairs, 0,
        "뱃머리 회전은 넘지 않는다: {:?}",
        r.turn
    );
    assert!(r.turn.holds, "{:?}", r.turn.why_not);
}

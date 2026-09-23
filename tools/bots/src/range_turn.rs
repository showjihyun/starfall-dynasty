//! SC-24 (c)(d) 를 **한 실행 안에서** 판정한다 (probe `cheat-range-turn`). **IO 가 없고 시계를 읽지 않는다.**
//!
//! 라운드 3 까지 `probe cheat-range` 는 주입 프레임만 보내고 유효 명령을 한 건도 보내지 않아(`sent=0`)
//! (d) 를 관측할 수 없었고, (c) 의 "거부"와 "이월 지속"은 **서로 다른 실행**(`cheat-range` / `cheat-seq`)에
//! 나뉘어 있었다. 이 probe 는 한 연결에서 ① 유효 추력 → ② 범위 초과 주입 → ②' 유효 재개 →
//! ③ `aim_*` 극단값 을 차례로 보내고, 여기서 자기 함선 스냅샷 시계열로 둘을 판정한다.
//!
//! **조건이 실제로 발생했는지를 함께 단언한다**(계약 §7a):
//! - (c) 주입이 **실제로 `MALFORMED_COMMAND` 로 거부됐고**, 그 구간에 **비교한 스냅샷 쌍이 있다**.
//! - (d) 선회 구간에 **리미터가 실제로 걸린 쌍(한도의 90 % 이상)이 있다.** 목표가 가까워 한도에
//!   닿지도 않았다면 "넘지 않았다"는 아무것도 뜻하지 않는다.
//!
//! **해상도 한계(리포트에 적는다)**: 스냅샷이 2 tick 마다라 쌍 하나는 2 tick 의 평균 회전이다.
//! 한 tick 에 한도의 2배를 돌고 다음 tick 에 0 을 도는 경우는 쌍으로는 안 보인다 — 그래서 매 스냅샷의
//! `|ω_aim|` 필드(서버가 그 tick 에 쓴 각속도)도 한도와 대조한다.

use serde::Serialize;

use crate::ledger::CommandOutcome;
use crate::snapshot::OwnSample;

/// 원장 표지 — 범위 초과로 주입한 프레임.
pub const MARK_INJECTED: &str = "range-injected";
/// 원장 표지 — ③ 선회 구간의 첫 명령.
pub const MARK_TURN_START: &str = "turn-start";

pub const REASON_MALFORMED: &str = "MALFORMED_COMMAND";

/// 한도의 이 비율 이상 돈 쌍을 "리미터가 걸렸다"로 센다.
pub const SATURATION_RATIO: f64 = 0.9;
/// 이 이하의 회전은 "정지"로 본다(도/tick).
pub const MOVING_EPS_DEG_PER_TICK: f64 = 0.01;
/// 속도 감소로 치지 않는 여유 (mm/s). 속도 성분이 1 mm/s 로 양자화된다.
pub const SPEED_DROP_EPS_MM_S: f64 = 2.0;

#[derive(Debug, Clone, Copy)]
pub struct Params {
    pub tick_hz: u32,
    /// `data/ships/*.json` 의 `turn_rate_max_deg_s` — **측정 시점 값을 증거에 함께 적는다**(계약 §0.7).
    pub turn_rate_max_deg_s: f64,
    /// `data/movement/sync-tuning.json` 의 `input.carry_forward_max_ticks`.
    pub carry_forward_max_ticks: u64,
    /// 방향 양자화(1e-6) 여유. 도/tick.
    pub tolerance_deg_per_tick: f64,
}

/// 주입한 프레임 1건: 우리가 넣은 `input_seq` 와 서버가 준 결과.
#[derive(Debug, Clone)]
pub struct Injected {
    pub input_seq: u64,
    pub outcome: Option<CommandOutcome>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct CarryReport {
    pub injected: u64,
    pub rejected_malformed: u64,
    /// 거부가 아닌 결과(수락 = 클램프해서 적용했다는 뜻) 또는 다른 사유.
    pub other_outcomes: u64,
    pub missing_results: u64,
    pub reject_tick_first: Option<u64>,
    pub reject_tick_last: Option<u64>,
    /// `ack_input_seq` 가 주입한 번호가 된 스냅샷 수. **클램프해서 적용했다면 여기가 0 이 아니다.**
    pub injected_seq_acked: u64,
    /// 거부 구간을 덮는 스냅샷 쌍 수(검사 건수).
    pub window_pairs: u64,
    pub speed_drops: u64,
    pub speed_first_mps: Option<f64>,
    pub speed_last_mps: Option<f64>,
    pub path_m: f64,
    /// 거부 구간 길이가 이월 창보다 짧은가(probe 설계 확인 — 길면 이월 만료가 정당하다).
    pub span_within_carry: bool,
    pub holds: bool,
    pub why_not: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct TurnReport {
    pub turn_start_tick: Option<u64>,
    /// 비교한 스냅샷 쌍 수(검사 건수). 아래 회전 값은 전부 **뱃머리 방향(조준 채널)** 기준이다.
    pub pairs: u64,
    pub moving_pairs: u64,
    /// 한도의 `SATURATION_RATIO` 이상 — **리미터가 실제로 걸린** 쌍.
    pub saturated_pairs: u64,
    pub over_limit_pairs: u64,
    pub limit_deg_per_tick: f64,
    pub max_deg_per_tick: f64,
    /// 참고(판정 아님): 롤까지 합친 **전체 자세** 회전의 최대와 한도 초과 쌍 수. 조준과 롤이 동시에
    /// 돌면 전체 회전은 조준 한도를 넘을 수 있다 — 그것은 롤 권한이지 (d) 위반이 아니다.
    pub max_total_deg_per_tick: f64,
    pub total_over_limit_pairs: u64,
    pub omega_samples: u64,
    pub omega_aim_max_deg_s: f64,
    pub omega_aim_over_limit: u64,
    pub omega_roll_max_deg_s: f64,
    pub holds: bool,
    pub why_not: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct RangeTurnReport {
    pub carry: CarryReport,
    pub turn: TurnReport,
}

fn speed_mm_s(s: &OwnSample) -> f64 {
    let (x, y, z) = s.velocity_mm_s;
    let (x, y, z) = (f64::from(x), f64::from(y), f64::from(z));
    (x * x + y * y + z * z).sqrt()
}

fn dist_mm(a: &OwnSample, b: &OwnSample) -> f64 {
    let dx = (b.position_mm.0 - a.position_mm.0) as f64;
    let dy = (b.position_mm.1 - a.position_mm.1) as f64;
    let dz = (b.position_mm.2 - a.position_mm.2) as f64;
    (dx * dx + dy * dy + dz * dz).sqrt()
}

fn unit(q: (i32, i32, i32, i32)) -> Option<[f64; 4]> {
    let v = [
        f64::from(q.0),
        f64::from(q.1),
        f64::from(q.2),
        f64::from(q.3),
    ];
    let n = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2] + v[3] * v[3]).sqrt();
    if n == 0.0 {
        return None;
    }
    Some([v[0] / n, v[1] / n, v[2] / n, v[3] / n])
}

/// 뱃머리(로컬 +Z, ADR-0009 §1) 방향 벡터.
fn forward(q: (i32, i32, i32, i32)) -> Option<[f64; 3]> {
    let [x, y, z, w] = unit(q)?;
    Some([
        2.0 * (x * z + w * y),
        2.0 * (y * z - w * x),
        1.0 - 2.0 * (x * x + y * y),
    ])
}

/// **조준 채널의 회전** — 두 자세의 뱃머리 방향 사이 각(도). 뱃머리 축 둘레의 롤은 뱃머리를 움직이지
/// 않으므로 여기에 들어가지 않는다. `turn_rate_max_deg_s` 는 조준 채널의 한도이고 롤은 별도 권한
/// (`roll_rate_max_deg_s`, ADR-0010 §2 의 롤 축 권한 분리)이므로, (d) 는 이 값으로 판정한다.
pub fn nose_rotation_deg(a: (i32, i32, i32, i32), b: (i32, i32, i32, i32)) -> Option<f64> {
    let fa = forward(a)?;
    let fb = forward(b)?;
    let cross = [
        fa[1] * fb[2] - fa[2] * fb[1],
        fa[2] * fb[0] - fa[0] * fb[2],
        fa[0] * fb[1] - fa[1] * fb[0],
    ];
    let c = (cross[0] * cross[0] + cross[1] * cross[1] + cross[2] * cross[2]).sqrt();
    let d = fa[0] * fb[0] + fa[1] * fb[1] + fa[2] * fb[2];
    Some(c.atan2(d).to_degrees())
}

/// 두 방향 사이의 회전각(도). `conj(a) · b` 의 각을 `atan2` 로 구한다 — `acos` 는 작은 각에서 불안정하다.
/// `q` 와 `-q` 는 같은 방향이므로 `|w|` 를 쓴다.
pub fn rotation_deg(a: (i32, i32, i32, i32), b: (i32, i32, i32, i32)) -> Option<f64> {
    let [ax, ay, az, aw] = unit(a)?;
    let [bx, by, bz, bw] = unit(b)?;
    let (ax, ay, az) = (-ax, -ay, -az);
    let rw = aw * bw - ax * bx - ay * by - az * bz;
    let rx = aw * bx + ax * bw + ay * bz - az * by;
    let ry = aw * by - ax * bz + ay * bw + az * bx;
    let rz = aw * bz + ax * by - ay * bx + az * bw;
    let v = (rx * rx + ry * ry + rz * rz).sqrt();
    Some((2.0 * v.atan2(rw.abs())).to_degrees())
}

pub fn analyze_carry(series: &[OwnSample], injected: &[Injected], p: Params) -> CarryReport {
    let mut r = CarryReport {
        injected: injected.len() as u64,
        ..Default::default()
    };
    let mut reject_ticks = Vec::new();
    for i in injected {
        match &i.outcome {
            None => r.missing_results += 1,
            Some(o)
                if o.status == "REJECTED" && o.reason_code.as_deref() == Some(REASON_MALFORMED) =>
            {
                r.rejected_malformed += 1;
                reject_ticks.push(o.tick);
            }
            Some(_) => r.other_outcomes += 1,
        }
    }
    let seqs: Vec<u64> = injected.iter().map(|i| i.input_seq).collect();
    r.injected_seq_acked = series
        .iter()
        .filter(|s| s.ack_input_seq.is_some_and(|a| seqs.contains(&a)))
        .count() as u64;
    r.reject_tick_first = reject_ticks.iter().copied().min();
    r.reject_tick_last = reject_ticks.iter().copied().max();

    if let (Some(first), Some(last)) = (r.reject_tick_first, r.reject_tick_last) {
        r.span_within_carry = last - first < p.carry_forward_max_ticks;
        // 거부 구간을 덮는 스냅샷: first 이하의 마지막 것부터 last 이상의 첫 것까지.
        let lo = series.iter().rposition(|s| s.tick <= first).unwrap_or(0);
        let hi = series
            .iter()
            .position(|s| s.tick >= last)
            .unwrap_or(series.len().saturating_sub(1));
        if lo < hi {
            let win = &series[lo..=hi];
            r.speed_first_mps = Some(speed_mm_s(&win[0]) / 1000.0);
            r.speed_last_mps = Some(speed_mm_s(&win[win.len() - 1]) / 1000.0);
            for w in win.windows(2) {
                r.window_pairs += 1;
                r.path_m += dist_mm(&w[0], &w[1]) / 1000.0;
                if speed_mm_s(&w[1]) < speed_mm_s(&w[0]) - SPEED_DROP_EPS_MM_S {
                    r.speed_drops += 1;
                }
            }
        }
    }

    let mut why = Vec::new();
    if r.injected == 0 {
        why.push("주입 0건 — 조건이 만들어지지 않았다".to_owned());
    }
    if r.rejected_malformed != r.injected {
        why.push(format!(
            "MALFORMED 거부 {}/{} (다른 결과 {}, 결과 없음 {})",
            r.rejected_malformed, r.injected, r.other_outcomes, r.missing_results
        ));
    }
    if r.injected_seq_acked > 0 {
        why.push(format!(
            "주입한 input_seq 가 ack 됐다 {}회 — 클램프 적용 흔적",
            r.injected_seq_acked
        ));
    }
    if r.window_pairs == 0 {
        why.push("거부 구간을 덮는 스냅샷 쌍이 없다 — 이월을 관측하지 못했다".to_owned());
    }
    if r.speed_drops > 0 {
        why.push(format!(
            "거부 구간에서 속도 감소 {}쌍 — 이월이 끊겼다",
            r.speed_drops
        ));
    }
    if r.window_pairs > 0 && r.path_m <= 0.0 {
        why.push("거부 구간 이동 0 m".to_owned());
    }
    if r.reject_tick_first.is_some() && !r.span_within_carry {
        why.push(format!(
            "거부 구간이 이월 창({} tick) 이상 — probe 설계가 틀렸다(만료가 정당해진다)",
            p.carry_forward_max_ticks
        ));
    }
    r.holds = why.is_empty();
    r.why_not = why;
    r
}

pub fn analyze_turn(series: &[OwnSample], turn_start_tick: Option<u64>, p: Params) -> TurnReport {
    let limit_tick = p.turn_rate_max_deg_s / f64::from(p.tick_hz.max(1));
    let mut r = TurnReport {
        turn_start_tick,
        limit_deg_per_tick: limit_tick,
        ..Default::default()
    };
    let Some(start) = turn_start_tick else {
        r.why_not
            .push("선회 시작 tick 을 모른다(첫 선회 명령의 결과가 없다)".to_owned());
        return r;
    };
    let seg: Vec<&OwnSample> = series.iter().filter(|s| s.tick >= start).collect();
    for s in &seg {
        r.omega_samples += 1;
        let (x, y, z) = s.angular_velocity_mdeg_s;
        let (x, y, z) = (f64::from(x), f64::from(y), f64::from(z));
        let w = (x * x + y * y + z * z).sqrt() / 1000.0;
        r.omega_aim_max_deg_s = r.omega_aim_max_deg_s.max(w);
        // ω 는 1 mdeg/s 로 양자화된다.
        if w > p.turn_rate_max_deg_s + 0.001 {
            r.omega_aim_over_limit += 1;
        }
        let roll = f64::from(s.angular_velocity_roll_mdeg_s).abs() / 1000.0;
        r.omega_roll_max_deg_s = r.omega_roll_max_deg_s.max(roll);
    }
    for w in seg.windows(2) {
        let dt = w[1].tick.saturating_sub(w[0].tick);
        if dt == 0 {
            continue;
        }
        let (Some(deg), Some(total)) = (
            nose_rotation_deg(w[0].orientation_micro, w[1].orientation_micro),
            rotation_deg(w[0].orientation_micro, w[1].orientation_micro),
        ) else {
            continue;
        };
        r.pairs += 1;
        let total_per_tick = total / dt as f64;
        r.max_total_deg_per_tick = r.max_total_deg_per_tick.max(total_per_tick);
        if total_per_tick > limit_tick + p.tolerance_deg_per_tick {
            r.total_over_limit_pairs += 1;
        }
        let per_tick = deg / dt as f64;
        r.max_deg_per_tick = r.max_deg_per_tick.max(per_tick);
        if per_tick > MOVING_EPS_DEG_PER_TICK {
            r.moving_pairs += 1;
        }
        if per_tick >= SATURATION_RATIO * limit_tick {
            r.saturated_pairs += 1;
        }
        if per_tick > limit_tick + p.tolerance_deg_per_tick {
            r.over_limit_pairs += 1;
        }
    }
    if r.pairs == 0 {
        r.why_not
            .push("선회 구간 스냅샷 쌍 0 — 비교하지 못했다".to_owned());
    }
    if r.pairs > 0 && r.saturated_pairs == 0 {
        r.why_not.push(format!(
            "리미터가 걸린 쌍 0 (최대 {:.4}°/tick < 한도 {:.4}의 {:.0}%) — 조건이 만들어지지 않았다",
            r.max_deg_per_tick,
            limit_tick,
            SATURATION_RATIO * 100.0
        ));
    }
    if r.over_limit_pairs > 0 {
        r.why_not.push(format!(
            "한도 초과 쌍 {} (최대 {:.4}°/tick > {:.4})",
            r.over_limit_pairs, r.max_deg_per_tick, limit_tick
        ));
    }
    if r.omega_aim_over_limit > 0 {
        r.why_not.push(format!(
            "|ω_aim| 한도 초과 {}건 (최대 {:.3}°/s > {})",
            r.omega_aim_over_limit, r.omega_aim_max_deg_s, p.turn_rate_max_deg_s
        ));
    }
    r.holds = r.why_not.is_empty();
    r
}

pub fn analyze(
    series: &[OwnSample],
    injected: &[Injected],
    turn_start_tick: Option<u64>,
    p: Params,
) -> RangeTurnReport {
    RangeTurnReport {
        carry: analyze_carry(series, injected, p),
        turn: analyze_turn(series, turn_start_tick, p),
    }
}

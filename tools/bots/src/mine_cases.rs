//! p1-02 계약 채굴 케이스 — 행동 조립과 **판정**(계약 SC-73~78).
//!
//! 판정 입력은 봇이 받은 메시지뿐이다(`MiningObs`). 판정 함수는 순수 함수이고, 각 함수는
//! 계약 ⊘ 칸의 "조건이 실제로 일어났다" 단언을 **먼저** 하고 분모를 찍는다. 조건이 일어나지
//! 않았으면 PASS 가 아니라 `미검증` 이다(도구 exit 4) — 빨간불과 구분한다.

use std::path::Path;
use std::time::Duration;

use serde_json::{Value, json};
use uuid::Uuid;

use crate::conn::{Behavior, FlyMode, MineStep};
use crate::mine_run::DepositSite;
use crate::mining::MiningObs;
use crate::scenario::ProbeCase;

/// 스펙 AC-15 / 디자인 §2 의 사거리(광맥 **표면**에서, m)와 속도 한계(m/s). `data/` 에서 읽지 않는다
/// — 판정 기준값은 계약·스펙 문구가 정본이다(계약 §0.6).
pub const MINING_RANGE_FROM_SURFACE_M: f64 = 150.0;
pub const MAX_SHIP_SPEED_MPS: f64 = 10.0;
/// SC-76 상한: ⌈60 / 3⌉ + 1.
pub const COOLDOWN_ACCEPT_MAX: usize = 21;

/// 판정 결과.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Pass,
    Fail,
    /// 조건이 일어나지 않았다(⊘) — 초록도 빨강도 아니다.
    Undecided,
}

impl Verdict {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pass => "PASS",
            Self::Fail => "FAIL",
            Self::Undecided => "미검증(조건 미발생)",
        }
    }
    pub fn exit_code(self) -> u8 {
        match self {
            Self::Pass => 0,
            Self::Fail => 1,
            Self::Undecided => 4,
        }
    }
}

/// 주입 케이스 4 경우 — (계약 반례 파일, 주입된 키가 어디 있는가).
pub const INJECT_FIXTURES: [(&str, &str); 4] = [
    ("quantity-field-injected.json", "payload.quantity_kg"),
    ("mineral-field-injected.json", "payload.mineral_id"),
    ("position-field-injected.json", "payload.position_x_mm"),
    ("actor-field-injected.json", "actor_id"),
];

/// 계약 반례 파일을 원문 템플릿으로 — `command_id` 는 `{COMMAND_ID}`, `deposit_id` 는 이 광맥.
/// 주입 필드는 **그대로** 둔다(봇 타입을 거치면 어휘 밖 필드를 시험할 수 없다).
pub fn inject_templates(contracts_dir: &Path, deposit_id: &str) -> Result<Vec<String>, String> {
    let dir = contracts_dir
        .join("fixtures")
        .join("MINE_RESOURCE")
        .join("invalid");
    INJECT_FIXTURES
        .iter()
        .map(|(file, _)| {
            let path = dir.join(file);
            let text =
                std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            let mut v: Value =
                serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
            v["command_id"] = json!("{COMMAND_ID}");
            v["payload"]["deposit_id"] = json!(deposit_id);
            serde_json::to_string(&v).map_err(|e| e.to_string())
        })
        .collect()
}

/// 형식은 유효한(케밥) 없는 id — 스키마 위반으로 `MALFORMED_COMMAND` 가 먼저 나지 않게.
pub const UNKNOWN_DEPOSIT_ID: &str = "qa-no-such-deposit";

/// 케이스 → 연결 행동.
pub fn behavior(case: ProbeCase, site: &DepositSite, inject: &[String]) -> Option<Behavior> {
    let stop_r = site.radius_m + 90.0;
    let base = |fly, steps| Behavior::MineScript {
        deposit_id: site.id.clone(),
        target_m: site.position_m,
        stop_radius_m: stop_r,
        fly,
        steps,
        grace: Duration::from_secs(2),
    };
    let stop = FlyMode::Stop {
        timeout: Duration::from_secs(240),
    };
    Some(match case {
        ProbeCase::MineDup => base(
            stop,
            vec![
                MineStep::Mine,
                MineStep::Wait(Duration::from_millis(1500)),
                MineStep::Resend(0),
                MineStep::Wait(Duration::from_millis(1500)),
            ],
        ),
        ProbeCase::CheatMineInject => {
            let mut steps = vec![MineStep::Mine, MineStep::Wait(Duration::from_millis(1500))];
            for t in inject {
                steps.push(MineStep::Raw(t.clone()));
                steps.push(MineStep::Wait(Duration::from_millis(400)));
            }
            steps.push(MineStep::Wait(Duration::from_millis(1500)));
            base(stop, steps)
        }
        ProbeCase::CheatMineRange => {
            let mut steps = Vec::new();
            for _ in 0..5 {
                steps.push(MineStep::Mine);
                steps.push(MineStep::Wait(Duration::from_millis(500)));
            }
            base(FlyMode::Stay, steps)
        }
        ProbeCase::CheatMineFast => Behavior::MineScript {
            deposit_id: site.id.clone(),
            target_m: site.position_m,
            // 사거리보다 충분히 안쪽에서 보낸다 — 거리 사유가 먼저 나지 않게.
            stop_radius_m: site.radius_m + 80.0,
            fly: FlyMode::PassThrough {
                speed_mps: 30.0,
                timeout: Duration::from_secs(240),
            },
            steps: vec![MineStep::Mine, MineStep::Wait(Duration::from_millis(1000))],
            grace: Duration::from_secs(2),
        },
        ProbeCase::CheatMineCooldown => base(
            stop,
            vec![MineStep::Burst {
                every: Duration::from_millis(300),
                total: Duration::from_secs(60),
            }],
        ),
        ProbeCase::LeakScan => base(
            stop,
            vec![MineStep::Mine, MineStep::Wait(Duration::from_millis(1500))],
        ),
        ProbeCase::CheatMineUnknown => base(
            FlyMode::Stay,
            vec![
                MineStep::MineOther(UNKNOWN_DEPOSIT_ID.to_owned()),
                MineStep::Wait(Duration::from_millis(1000)),
            ],
        ),
        _ => return None,
    })
}

fn total_kg(obs: &MiningObs, idx: usize) -> i64 {
    obs.inventory[idx]
        .msg
        .payload
        .items
        .iter()
        .map(|i| i.quantity_kg)
        .sum()
}

/// 보낸 순서대로 각 채굴의 결과들(같은 id 재전송이면 결과가 둘).
fn results_for(obs: &MiningObs, id: Uuid) -> Vec<(String, Option<String>, u64)> {
    obs.results
        .iter()
        .filter(|r| r.command_id == id)
        .map(|r| (r.status.clone(), r.reason_code.clone(), r.frame_seq))
        .collect()
}

/// 인벤토리 총량이 바뀐 횟수(세션 시작 상태 대비, 메시지 순서대로).
fn inventory_changes(obs: &MiningObs) -> (usize, Option<i64>, Option<i64>) {
    if obs.inventory.is_empty() {
        return (0, None, None);
    }
    let first = total_kg(obs, 0);
    let mut prev = first;
    let mut changes = 0;
    for i in 1..obs.inventory.len() {
        let t = total_kg(obs, i);
        if t != prev {
            changes += 1;
            prev = t;
        }
    }
    (changes, Some(first), Some(prev))
}

fn is_rejected_with(r: &(String, Option<String>, u64), code: &str) -> bool {
    r.0 == "REJECTED" && r.1.as_deref() == Some(code)
}

/// 케이스 판정. `site` 는 거리 판정에 쓴다.
pub fn evaluate(
    case: ProbeCase,
    site: &DepositSite,
    obs: &MiningObs,
    terms: &LeakTerms,
) -> (Verdict, Value) {
    match case {
        ProbeCase::LeakScan => eval_leak(obs, terms),
        ProbeCase::MineDup => eval_dup(obs),
        ProbeCase::CheatMineInject => eval_inject(obs),
        ProbeCase::CheatMineRange => eval_simple_reject(obs, "TARGET_OUT_OF_RANGE", |s| {
            s.dist_m
                .is_some_and(|d| d - site.radius_m > MINING_RANGE_FROM_SURFACE_M)
        }),
        ProbeCase::CheatMineFast => eval_simple_reject(obs, "SHIP_TOO_FAST", |s| {
            s.speed_mps.is_some_and(|v| v > MAX_SHIP_SPEED_MPS)
                && s.dist_m
                    .is_some_and(|d| d - site.radius_m <= MINING_RANGE_FROM_SURFACE_M)
        }),
        ProbeCase::CheatMineCooldown => eval_cooldown(obs),
        ProbeCase::CheatMineUnknown => {
            let (v, mut d) = eval_simple_reject(obs, "TARGET_UNKNOWN", |s| {
                s.deposit_id == UNKNOWN_DEPOSIT_ID
            });
            d["unknown_id_is_kebab"] = json!(is_kebab(UNKNOWN_DEPOSIT_ID));
            (v, d)
        }
        _ => (Verdict::Undecided, json!({"error": "채굴 케이스가 아니다"})),
    }
}

/// SC-40 검색어 — 광맥 표의 `mineral_id` 값과 매장량 수치. `data/` 에서 읽는다: 이것은 **검출기의
/// 입력**(무엇을 찾을지)이지 판정 기대값이 아니다.
#[derive(Debug, Clone, Default)]
pub struct LeakTerms {
    pub mineral_ids: Vec<String>,
    pub reserves_kg: Vec<i64>,
}

impl LeakTerms {
    pub fn from_data(data_dir: &Path) -> Result<Self, String> {
        let dir = data_dir.join("world").join("deposits");
        let mut t = LeakTerms::default();
        for entry in std::fs::read_dir(&dir)
            .map_err(|e| format!("{}: {e}", dir.display()))?
            .flatten()
        {
            let path = entry.path();
            if path.extension().is_none_or(|x| x != "json") {
                continue;
            }
            let text =
                std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            let v: Value =
                serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
            for d in v["deposits"].as_array().into_iter().flatten() {
                if let Some(m) = d["mineral_id"].as_str()
                    && !t.mineral_ids.iter().any(|x| x == m)
                {
                    t.mineral_ids.push(m.to_owned());
                }
                if let Some(r) = d["initial_reserve_kg"].as_i64()
                    && !t.reserves_kg.contains(&r)
                {
                    t.reserves_kg.push(r);
                }
            }
        }
        if t.mineral_ids.is_empty() || t.reserves_kg.is_empty() {
            return Err(format!("{} 에서 검색어를 하나도 못 읽었다", dir.display()));
        }
        Ok(t)
    }
}

/// 운동·시간 필드 — 매장량과 같은 수가 우연히 나와도 누출이 아니다(위치 mm, tick 등).
fn is_motion_or_time_key(key: &str) -> bool {
    key == "tick"
        || key.ends_with("_tick")
        || key.ends_with("_ticks")
        || key.ends_with("_seq")
        || key == "tick_hz"
        || key == "schema_version"
        || key.starts_with("position_")
        || key.starts_with("velocity_")
        || key.starts_with("orientation_")
        || key.starts_with("angular_velocity_")
        || key.ends_with("_radius_mm")
}

/// 원문 프레임 하나를 **구조로** 훑는다. 문자열 값이 광물 id 와 같으면(키 무관), 운동·시간이 아닌
/// 키의 수 값이 매장량과 같으면 적중. 반환: (경로, 값) 목록.
pub fn structural_hits(text: &str, terms: &LeakTerms) -> Vec<(String, String)> {
    fn walk(v: &Value, path: &str, key: &str, t: &LeakTerms, out: &mut Vec<(String, String)>) {
        match v {
            Value::Object(m) => {
                for (k, x) in m {
                    walk(x, &format!("{path}/{k}"), k, t, out);
                }
            }
            Value::Array(a) => {
                for (i, x) in a.iter().enumerate() {
                    walk(x, &format!("{path}/{i}"), key, t, out);
                }
            }
            Value::String(s) if t.mineral_ids.iter().any(|m| m == s) => {
                out.push((path.to_owned(), s.clone()));
            }
            Value::Number(n)
                if !is_motion_or_time_key(key)
                    && n.as_i64().is_some_and(|x| t.reserves_kg.contains(&x)) =>
            {
                out.push((path.to_owned(), n.to_string()));
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    match serde_json::from_str::<Value>(text) {
        Ok(v) => walk(&v, "", "", terms, &mut out),
        // 파싱 못 하는 프레임은 원문 부분 문자열로라도 본다(누출을 놓치지 않게).
        Err(_) => {
            for m in &terms.mineral_ids {
                if text.contains(m.as_str()) {
                    out.push(("<unparsed>".into(), m.clone()));
                }
            }
        }
    }
    out
}

fn message_type(text: &str) -> String {
    serde_json::from_str::<Value>(text)
        .ok()
        .and_then(|v| v["message_type"].as_str().map(str::to_owned))
        .unwrap_or_else(|| "<unparsed>".into())
}

/// SC-40.
pub fn eval_leak(obs: &MiningObs, terms: &LeakTerms) -> (Verdict, Value) {
    let Some(frames) = obs.raw_frames.as_ref() else {
        return (
            Verdict::Undecided,
            json!({"why": "원문 프레임을 저장하지 않았다"}),
        );
    };
    let Some(first) = obs.mine_sent.first() else {
        return (
            Verdict::Undecided,
            json!({"why": "채굴을 보내지 못했다(도착 실패?)", "frames": frames.len()}),
        );
    };
    let boundary = first.frame_seq_at_send;
    let first_accepted = results_for(obs, first.command_id)
        .first()
        .is_some_and(|r| r.0 == "ACCEPTED");
    let pre: Vec<(u64, String)> = frames
        .iter()
        .filter(|(q, _)| *q < boundary)
        .cloned()
        .collect();
    let post: Vec<&(u64, String)> = frames.iter().filter(|(q, _)| *q >= boundary).collect();
    let mut types: std::collections::BTreeMap<String, usize> = Default::default();
    for (_, t) in &pre {
        *types.entry(message_type(t)).or_default() += 1;
    }
    let (dfs_msgs, dfs_entries, unrevealed, shape_violations) =
        crate::mining::deposit_state_shape(&pre);
    let mut pre_hits = Vec::new();
    for (q, t) in &pre {
        for (path, val) in structural_hits(t, terms) {
            pre_hits
                .push(json!({"frame_seq": q, "type": message_type(t), "path": path, "value": val}));
        }
    }
    let mut post_hits = 0usize;
    for (_, t) in &post {
        post_hits += structural_hits(t, terms).len();
    }
    let inv_pre = types
        .get(crate::wire::INVENTORY_STATE)
        .copied()
        .unwrap_or(0);
    let details = json!({
        "frames_total": frames.len(), "frames_before_first_mine": pre.len(), "frames_after": post.len(),
        "types_before_first_mine": types, "inventory_state_before": inv_pre,
        "deposit_field_state_before": {"messages": dfs_msgs, "entries": dfs_entries, "unrevealed": unrevealed,
                                       "shape_violations": shape_violations.len()},
        "first_mine_accepted": first_accepted,
        "search_terms": {"mineral_ids": terms.mineral_ids, "reserves_kg": terms.reserves_kg},
        "leak_hits_before_first_mine": pre_hits,
        "positive_control_hits_after_first_mine": post_hits,
    });
    // ⊘(r2): 누출이 가장 날 만한 두 메시지를 첫 채굴 전에 실제로 받았다 — 광맥 8 항목, 첫 채굴 수락.
    if inv_pre == 0 || dfs_msgs == 0 || dfs_entries < 8 || !first_accepted {
        return (Verdict::Undecided, details);
    }
    // 검출기 사망(양성 적중 0)은 FAIL(계약 SC-40). 구조 위반·누출도 FAIL.
    let pass = pre_hits.is_empty()
        && shape_violations.is_empty()
        && unrevealed == dfs_entries
        && post_hits >= 1;
    (if pass { Verdict::Pass } else { Verdict::Fail }, details)
}

fn is_kebab(s: &str) -> bool {
    !s.is_empty()
        && s.split('-').all(|p| {
            !p.is_empty()
                && p.chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        })
}

/// SC-78 (a).
pub fn eval_dup(obs: &MiningObs) -> (Verdict, Value) {
    let Some(first) = obs.mine_sent.first() else {
        return (
            Verdict::Undecided,
            json!({"why": "채굴을 한 번도 보내지 못했다(도착 실패?)"}),
        );
    };
    let rs = results_for(obs, first.command_id);
    let sends = obs
        .mine_sent
        .iter()
        .filter(|s| s.command_id == first.command_id)
        .count();
    let first_accepted = rs.first().is_some_and(|r| r.0 == "ACCEPTED");
    let (changes, before, after) = inventory_changes(obs);
    let increased = matches!((before, after), (Some(b), Some(a)) if a > b);
    let details = json!({
        "sends_of_same_id": sends, "results": rs.iter().map(|r| json!([r.0, r.1])).collect::<Vec<_>>(),
        "first_accepted": first_accepted, "inventory_before_kg": before, "inventory_after_kg": after,
        "inventory_changes": changes,
    });
    // ⊘: 첫 전송 ACCEPTED + 인벤토리 증가, 재전송이 실제로 나갔다.
    if !first_accepted || !increased || sends < 2 {
        return (Verdict::Undecided, details);
    }
    let dup = rs.len() == 2 && is_rejected_with(&rs[1], "DUPLICATE_COMMAND_ID");
    let once = changes == 1;
    (
        if dup && once {
            Verdict::Pass
        } else {
            Verdict::Fail
        },
        details,
    )
}

/// SC-73.
pub fn eval_inject(obs: &MiningObs) -> (Verdict, Value) {
    let Some(first) = obs.mine_sent.first() else {
        return (Verdict::Undecided, json!({"why": "채굴을 보내지 못했다"}));
    };
    let first_ok = results_for(obs, first.command_id)
        .first()
        .is_some_and(|r| r.0 == "ACCEPTED");
    let (changes, before, after) = inventory_changes(obs);
    let mut cases = Vec::new();
    let mut malformed = 0;
    let mut injected_present = 0;
    for ((id, raw), (_, key)) in obs.raw_sent.iter().zip(INJECT_FIXTURES.iter()) {
        let v: Value = serde_json::from_str(raw).unwrap_or(Value::Null);
        let present = match key.strip_prefix("payload.") {
            Some(k) => v["payload"].get(k).is_some(),
            None => v.get(*key).is_some(),
        };
        injected_present += usize::from(present);
        let rs = results_for(obs, *id);
        let ok = rs.len() == 1 && is_rejected_with(&rs[0], "MALFORMED_COMMAND");
        malformed += usize::from(ok);
        cases.push(json!({"injected": key, "present_in_sent_frame": present,
                          "results": rs.iter().map(|r| json!([r.0, r.1])).collect::<Vec<_>>()}));
    }
    let details = json!({"first_mine_accepted": first_ok, "inventory_before_kg": before,
        "inventory_after_kg": after, "inventory_changes": changes, "cases": cases,
        "malformed": malformed, "injected_present": injected_present, "denominator": INJECT_FIXTURES.len()});
    // ⊘: 주입 전 정상 채굴 1 회로 인벤토리 > 0, 보낸 원문에 주입 필드가 있다, 4 경우 모두 보냈다.
    if !first_ok
        || after.unwrap_or(0) <= 0
        || injected_present != INJECT_FIXTURES.len()
        || obs.raw_sent.len() != INJECT_FIXTURES.len()
    {
        return (Verdict::Undecided, details);
    }
    // 인벤토리 불변 = 정상 채굴 1 회 외의 변화 없음.
    let pass = malformed == INJECT_FIXTURES.len() && changes == 1;
    (if pass { Verdict::Pass } else { Verdict::Fail }, details)
}

/// 거부 사유 하나를 보는 케이스(SC-74·75·77). `cond` 는 보낸 시점 조건(⊘).
pub fn eval_simple_reject(
    obs: &MiningObs,
    code: &str,
    cond: impl Fn(&crate::mining::SentMine) -> bool,
) -> (Verdict, Value) {
    let mut rows = Vec::new();
    let mut exact = 0;
    let mut cond_ok = 0;
    for s in &obs.mine_sent {
        let rs = results_for(obs, s.command_id);
        let hit = rs.len() == 1 && is_rejected_with(&rs[0], code);
        exact += usize::from(hit);
        let c = cond(s);
        cond_ok += usize::from(c);
        rows.push(
            json!({"dist_m": s.dist_m, "speed_mps": s.speed_mps, "deposit_id": s.deposit_id,
                         "condition_at_send": c,
                         "results": rs.iter().map(|r| json!([r.0, r.1])).collect::<Vec<_>>()}),
        );
    }
    let n = obs.mine_sent.len();
    let details = json!({"expected_reason": code, "sent": n, "exact_reason": exact,
                         "condition_held_at_send": cond_ok, "rows": rows});
    // ⊘: 보냈다, 그리고 보낸 시점에 겨냥한 조건이 성립했다(전부).
    if n == 0 || cond_ok != n {
        return (Verdict::Undecided, details);
    }
    (
        if exact == n {
            Verdict::Pass
        } else {
            Verdict::Fail
        },
        details,
    )
}

/// SC-76.
pub fn eval_cooldown(obs: &MiningObs) -> (Verdict, Value) {
    let n = obs.mine_sent.len();
    let mut accepted = 0;
    let mut by_reason: std::collections::BTreeMap<String, usize> = Default::default();
    let mut no_result = 0;
    for s in &obs.mine_sent {
        let rs = results_for(obs, s.command_id);
        match rs.first() {
            None => no_result += 1,
            Some(r) if r.0 == "ACCEPTED" => accepted += 1,
            Some(r) => {
                *by_reason
                    .entry(r.1.clone().unwrap_or_else(|| "<null>".into()))
                    .or_default() += 1
            }
        }
    }
    let cooldown = by_reason.get("COOLDOWN_ACTIVE").copied().unwrap_or(0);
    let rate_limited = by_reason.get("RATE_LIMITED").copied().unwrap_or(0);
    let open = obs.open_at_end.unwrap_or(false);
    let details = json!({"sent": n, "accepted": accepted, "rejected_by_reason": by_reason,
        "no_result": no_result, "cooldown_active": cooldown, "rate_limited": rate_limited,
        "open_at_end": open, "accept_max": COOLDOWN_ACCEPT_MAX});
    // ⊘: 보낸 수 ≈ 200(≥ 150), 쿨다운 거부가 실제로 났다, 재수락이 됐다(≥ 2), 레이트 리밋에 먼저 걸리지 않았다.
    if n < 150 || cooldown == 0 || accepted < 2 || rate_limited > 0 {
        return (Verdict::Undecided, details);
    }
    let other = n - accepted - cooldown - no_result;
    let pass = accepted <= COOLDOWN_ACCEPT_MAX && other == 0 && no_result == 0 && open;
    (if pass { Verdict::Pass } else { Verdict::Fail }, details)
}

/// 케이스 하나를 실서버에 돌리고 판정한다. 반환 JSON 의 `item` 은 `contract_item()` 문자열 그대로다.
pub async fn run(
    case: ProbeCase,
    site: &DepositSite,
    url: &str,
    secret: &str,
    label: &str,
    contracts_dir: &Path,
    data_dir: &Path,
) -> (Verdict, Value) {
    let terms = if case == ProbeCase::LeakScan {
        match LeakTerms::from_data(data_dir) {
            Ok(t) => t,
            Err(e) => return (Verdict::Undecided, json!({"error": e})),
        }
    } else {
        LeakTerms::default()
    };
    let inject = if case == ProbeCase::CheatMineInject {
        match inject_templates(contracts_dir, &site.id) {
            Ok(t) => t,
            Err(e) => return (Verdict::Undecided, json!({"error": e})),
        }
    } else {
        Vec::new()
    };
    let Some(behavior) = behavior(case, site, &inject) else {
        return (Verdict::Undecided, json!({"error": "채굴 케이스가 아니다"}));
    };
    let (_subject, tok) = crate::token::identity(secret, label);
    let outcome = crate::conn::run_connection(crate::conn::BotSpec {
        label: label.to_owned(),
        url: url.to_owned(),
        token: tok,
        behavior,
        clock: crate::conn::Clock::start(),
        live_corr: None,
        capture_raw: case == ProbeCase::LeakScan,
    })
    .await;
    let (verdict, details) = evaluate(case, site, &outcome.mining, &terms);
    let nav = outcome.mining.nav.as_ref().map(|n| {
        json!({"start_dist_m": n.start_dist_m, "arrived": n.arrived_at_us.is_some(),
               "arrive_dist_m": n.arrive_dist_m, "arrive_speed_mps": n.arrive_speed_mps,
               "max_speed_mps": n.max_speed_mps, "control_frames": n.control_frames})
    });
    let session = outcome.ledger.session.as_ref().map(|s| {
        json!({"session_id": s.session_id.to_string(), "actor_id": s.actor_id.to_string(),
               "close_code": s.close_code, "close_initiator": s.close_initiator})
    });
    (
        verdict,
        json!({
            "item": case.contract_item().to_string(),
            "case": case.as_str(),
            "verdict": verdict.as_str(),
            "label": label,
            "deposit_id": site.id,
            "session": session,
            "nav": nav,
            "details": details,
            "connect_error": outcome.connect_error,
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mining::{Observed, ObservedResult, SentMine};
    use crate::wire::{InventoryItem, InventoryStateMessage, InventoryStatePayload};

    fn sent(id: Uuid, dist: f64, speed: f64) -> SentMine {
        SentMine {
            command_id: id,
            deposit_id: "d".into(),
            at_us: 0,
            frame_seq_at_send: 0,
            last_snapshot_tick: Some(1),
            dist_m: Some(dist),
            speed_mps: Some(speed),
        }
    }
    fn res(seq: u64, id: Uuid, status: &str, reason: Option<&str>) -> ObservedResult {
        ObservedResult {
            frame_seq: seq,
            tick: seq,
            command_id: id,
            status: status.into(),
            reason_code: reason.map(Into::into),
        }
    }
    fn inv(seq: u64, kg: i64) -> Observed<InventoryStateMessage> {
        Observed {
            frame_seq: seq,
            at_us: 0,
            tick: seq,
            msg: InventoryStateMessage {
                message_id: Uuid::now_v7(),
                message_type: "INVENTORY_STATE".into(),
                schema_version: 1,
                tick: seq,
                correlation_id: None,
                payload: InventoryStatePayload {
                    actor_id: Uuid::nil(),
                    items: if kg == 0 {
                        vec![]
                    } else {
                        vec![InventoryItem {
                            mineral_id: "glacine".into(),
                            quantity_kg: kg,
                        }]
                    },
                },
            },
        }
    }

    #[test]
    fn dup_pass_and_its_negative_controls() {
        let a = Uuid::now_v7();
        let good = MiningObs {
            mine_sent: vec![sent(a, 100.0, 0.0), sent(a, 100.0, 0.0)],
            results: vec![
                res(2, a, "ACCEPTED", None),
                res(4, a, "REJECTED", Some("DUPLICATE_COMMAND_ID")),
            ],
            inventory: vec![inv(1, 0), inv(3, 150)],
            ..MiningObs::default()
        };
        assert_eq!(eval_dup(&good).0, Verdict::Pass);
        // 음성: 재전송이 다시 수락되고 인벤토리가 두 번 늘었다 → FAIL.
        let twice = MiningObs {
            results: vec![res(2, a, "ACCEPTED", None), res(4, a, "ACCEPTED", None)],
            inventory: vec![inv(1, 0), inv(3, 150), inv(5, 300)],
            ..good_clone(&good)
        };
        assert_eq!(eval_dup(&twice).0, Verdict::Fail);
        // ⊘: 첫 전송부터 거부 → 미검증(초록 아님).
        let first_rejected = MiningObs {
            results: vec![
                res(2, a, "REJECTED", Some("TARGET_OUT_OF_RANGE")),
                res(4, a, "REJECTED", Some("DUPLICATE_COMMAND_ID")),
            ],
            inventory: vec![inv(1, 0)],
            ..good_clone(&good)
        };
        assert_eq!(eval_dup(&first_rejected).0, Verdict::Undecided);
        // ⊘: 아무것도 안 보냄.
        assert_eq!(eval_dup(&MiningObs::default()).0, Verdict::Undecided);
    }

    fn good_clone(o: &MiningObs) -> MiningObs {
        MiningObs {
            mine_sent: o.mine_sent.clone(),
            raw_sent: o.raw_sent.clone(),
            ..MiningObs::default()
        }
    }

    #[test]
    fn simple_reject_requires_the_condition_at_send() {
        let a = Uuid::now_v7();
        let obs = MiningObs {
            mine_sent: vec![sent(a, 900.0, 0.0)],
            results: vec![res(2, a, "REJECTED", Some("TARGET_OUT_OF_RANGE"))],
            ..MiningObs::default()
        };
        let site = DepositSite {
            id: "d".into(),
            position_m: [0.0; 3],
            radius_m: 50.0,
        };
        assert_eq!(
            evaluate(
                ProbeCase::CheatMineRange,
                &site,
                &obs,
                &LeakTerms::default()
            )
            .0,
            Verdict::Pass
        );
        // 음성: 다른 사유로 거부 → FAIL.
        let other = MiningObs {
            results: vec![res(2, a, "REJECTED", Some("COOLDOWN_ACTIVE"))],
            ..good_clone(&obs)
        };
        assert_eq!(
            evaluate(
                ProbeCase::CheatMineRange,
                &site,
                &other,
                &LeakTerms::default()
            )
            .0,
            Verdict::Fail
        );
        // ⊘: 보낸 시점에 사실 사거리 안이었다 → 미검증.
        let inside = MiningObs {
            mine_sent: vec![sent(a, 120.0, 0.0)],
            results: vec![res(2, a, "REJECTED", Some("TARGET_OUT_OF_RANGE"))],
            ..MiningObs::default()
        };
        assert_eq!(
            evaluate(
                ProbeCase::CheatMineRange,
                &site,
                &inside,
                &LeakTerms::default()
            )
            .0,
            Verdict::Undecided
        );
        // SHIP_TOO_FAST ⊘: 속도가 한계 이하였다 → 미검증.
        let slow = MiningObs {
            mine_sent: vec![sent(a, 100.0, 5.0)],
            results: vec![res(2, a, "REJECTED", Some("SHIP_TOO_FAST"))],
            ..MiningObs::default()
        };
        assert_eq!(
            evaluate(
                ProbeCase::CheatMineFast,
                &site,
                &slow,
                &LeakTerms::default()
            )
            .0,
            Verdict::Undecided
        );
    }

    #[test]
    fn cooldown_bounds_and_controls() {
        let mut sent_v = Vec::new();
        let mut results = Vec::new();
        for i in 0..200u64 {
            let id = Uuid::now_v7();
            sent_v.push(sent(id, 100.0, 0.0));
            if i % 10 == 0 {
                results.push(res(i, id, "ACCEPTED", None));
            } else {
                results.push(res(i, id, "REJECTED", Some("COOLDOWN_ACTIVE")));
            }
        }
        let obs = MiningObs {
            mine_sent: sent_v.clone(),
            results: results.clone(),
            open_at_end: Some(true),
            ..MiningObs::default()
        };
        assert_eq!(eval_cooldown(&obs).0, Verdict::Pass); // 수락 20
        // 음성: 연결이 닫혔다 → FAIL.
        let closed = MiningObs {
            mine_sent: sent_v.clone(),
            results: results.clone(),
            open_at_end: Some(false),
            ..MiningObs::default()
        };
        assert_eq!(eval_cooldown(&closed).0, Verdict::Fail);
        // 음성: 수락 22 → FAIL.
        let mut many = results.clone();
        many[1] = res(1, sent_v[1].command_id, "ACCEPTED", None);
        many[2] = res(2, sent_v[2].command_id, "ACCEPTED", None);
        let over = MiningObs {
            mine_sent: sent_v.clone(),
            results: many,
            open_at_end: Some(true),
            ..MiningObs::default()
        };
        assert_eq!(eval_cooldown(&over).0, Verdict::Fail);
        // ⊘: 보낸 수가 적다 → 미검증.
        let few = MiningObs {
            mine_sent: sent_v[..20].to_vec(),
            results: results[..20].to_vec(),
            open_at_end: Some(true),
            ..MiningObs::default()
        };
        assert_eq!(eval_cooldown(&few).0, Verdict::Undecided);
    }

    fn terms() -> LeakTerms {
        LeakTerms {
            mineral_ids: vec!["glacine".into(), "ferrosite".into()],
            reserves_kg: vec![10000, 500],
        }
    }

    #[test]
    fn structural_scan_hits_ids_and_reserves_but_not_motion_fields() {
        let t = terms();
        // 미확인 광맥(넷 다 null) — 적중 없음.
        let clean = r#"{"message_type":"DEPOSIT_FIELD_STATE","tick":500,"payload":{"deposits":[{"deposit_id":"vela-orbit-2","mineral_id":null,"initial_reserve_kg":null,"remaining_kg":null,"first_extracted_tick":null}]}}"#;
        assert!(structural_hits(clean, &t).is_empty());
        // tick·위치에 우연히 같은 수 — 누출 아님.
        let motion = r#"{"message_type":"WORLD_SNAPSHOT","tick":10000,"payload":{"ships":[{"position_x_mm":500}]}}"#;
        assert!(structural_hits(motion, &t).is_empty());
        // 누출: mineral_id 값, 매장량 수치.
        let leak = r#"{"message_type":"DEPOSIT_FIELD_STATE","payload":{"deposits":[{"deposit_id":"x","mineral_id":"glacine","initial_reserve_kg":10000}]}}"#;
        assert_eq!(structural_hits(leak, &t).len(), 2);
        // 다른 키에 숨어 있어도(문자열) 잡는다.
        let hidden = r#"{"message_type":"SESSION_READY","payload":{"note":"glacine"}}"#;
        assert_eq!(structural_hits(hidden, &t).len(), 1);
    }

    #[test]
    fn leak_eval_controls() {
        let a = Uuid::now_v7();
        let dfs = |vals: &str| {
            let mut d = Vec::new();
            for i in 0..8 {
                d.push(format!(r#"{{"deposit_id":"d{i}",{vals}}}"#));
            }
            format!(
                r#"{{"message_type":"DEPOSIT_FIELD_STATE","payload":{{"deposits":[{}]}}}}"#,
                d.join(",")
            )
        };
        let nulls = r#""mineral_id":null,"initial_reserve_kg":null,"remaining_kg":null,"first_extracted_tick":null"#;
        let inv0 = r#"{"message_type":"INVENTORY_STATE","payload":{"items":[]}}"#.to_owned();
        let inv1 = r#"{"message_type":"INVENTORY_STATE","payload":{"items":[{"mineral_id":"glacine","quantity_kg":150}]}}"#.to_owned();
        let mut sent_a = sent(a, 100.0, 0.0);
        sent_a.frame_seq_at_send = 2;
        let mk = |pre_dfs: String, post: String| MiningObs {
            raw_frames: Some(vec![(0, inv0.clone()), (1, pre_dfs), (2, post)]),
            mine_sent: vec![sent_a.clone()],
            results: vec![res(3, a, "ACCEPTED", None)],
            ..MiningObs::default()
        };
        assert_eq!(
            eval_leak(&mk(dfs(nulls), inv1.clone()), &terms()).0,
            Verdict::Pass
        );
        // 음성: 첫 채굴 전 DEPOSIT_FIELD_STATE 에 mineral_id 가 보인다 → FAIL.
        let leaky = r#""mineral_id":"glacine","initial_reserve_kg":null,"remaining_kg":null,"first_extracted_tick":null"#;
        assert_eq!(
            eval_leak(&mk(dfs(leaky), inv1.clone()), &terms()).0,
            Verdict::Fail
        );
        // 음성: 검출기 사망 — 첫 채굴 뒤에도 적중 0 → FAIL.
        assert_eq!(
            eval_leak(&mk(dfs(nulls), inv0.clone()), &terms()).0,
            Verdict::Fail
        );
        // ⊘: DEPOSIT_FIELD_STATE 를 첫 채굴 전에 못 받았다 → 미검증.
        let no_dfs = MiningObs {
            raw_frames: Some(vec![(0, inv0.clone()), (2, inv1.clone())]),
            mine_sent: vec![sent_a.clone()],
            results: vec![res(3, a, "ACCEPTED", None)],
            ..MiningObs::default()
        };
        assert_eq!(eval_leak(&no_dfs, &terms()).0, Verdict::Undecided);
    }

    #[test]
    fn inject_templates_keep_the_injected_field() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../contracts");
        let t = inject_templates(&dir, "vela-orbit-2").expect("계약 반례 파일");
        assert_eq!(t.len(), 4);
        for (text, (_, key)) in t.iter().zip(INJECT_FIXTURES.iter()) {
            let v: Value = serde_json::from_str(&text.replace("{COMMAND_ID}", "x")).unwrap();
            let present = match key.strip_prefix("payload.") {
                Some(k) => v["payload"].get(k).is_some(),
                None => v.get(*key).is_some(),
            };
            assert!(present, "{key} 가 템플릿에 없다: {text}");
            assert_eq!(v["payload"]["deposit_id"], "vela-orbit-2");
            assert!(text.contains("{COMMAND_ID}"));
        }
        assert!(is_kebab(UNKNOWN_DEPOSIT_ID));
    }
}

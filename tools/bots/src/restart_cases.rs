//! p1-02 재기동·재접속 계열 채굴 케이스 — `mine-dup-reconnect`(SC-78 b) · `mine-dup-restart`
//! (SC-78 c) · `cas-halt`(SC-25) · `cas-reload`(SC-26) · `mine-dup-cross-actor`(SC-108).
//!
//! 서버 재기동·SQL 변조는 봇이 하지 않는다 — 오케스트레이터(`tests/e2e/restart_cases.py`)가
//! 하고, 봇과는 **파일**로 손잡는다. 한 케이스가 재기동을 사이에 두면 단계(`--phase`)로 나누고
//! 단계 사이 상태는 `--state` JSON 에 남긴다. 판정(verdict)은 마지막 단계의 봇이 낸다 — 계약이
//! 이 케이스들의 판정 도구로 `bots probe` 를 지명했다. 봇은 증거 DB 에 **쓰지 않는다**(읽기 전용
//! `sql` 모듈만 쓴다).

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};
use uuid::Uuid;

use crate::conn::{Behavior, BotSpec, Clock, ConnectionOutcome, FlyMode, MineStep, run_connection};
use crate::mine_cases::Verdict;
use crate::mine_run::DepositSite;
use crate::mining::MiningObs;
use crate::{sql, token};

/// 오케스트레이터가 넘기는 단계 입력.
#[derive(Debug, Clone, Default)]
pub struct PhaseArgs {
    pub phase: String,
    pub state: Option<PathBuf>,
    pub world: String,
    /// 재기동 뒤 서버 로그(재기동이 실제로 일어났다는 증거 — "tick 을 이어서 시작한다" 줄).
    pub restart_log: Option<PathBuf>,
    /// cas-halt 판정: 변조 실행의 서버 로그·종료 코드, 대조 실행의 상태·로그·종료 코드.
    pub server_log: Option<PathBuf>,
    pub exit_code: Option<i32>,
    pub control_state: Option<PathBuf>,
    pub control_log: Option<PathBuf>,
    pub control_exit_code: Option<i32>,
    pub ready_file: Option<PathBuf>,
    pub tampered_file: Option<PathBuf>,
    /// cas-reload: cas-halt 의 상태 파일(변조 값).
    pub halt_state: Option<PathBuf>,
    /// cas-reload: 오케스트레이터가 넣은 변조 값.
    pub tampered_value: Option<i64>,
    /// mine-dup-cross-actor 재기동 변형: 첫 단계에서 쓴 command_id.
    pub label_b: String,
}

fn spec(secret: &str, url: &str, label: &str, behavior: Behavior, clock: Clock) -> BotSpec {
    let (_s, tok) = token::identity(secret, label);
    BotSpec {
        label: label.to_owned(),
        url: url.to_owned(),
        token: tok,
        behavior,
        clock,
        live_corr: None,
        capture_raw: false,
    }
}

fn script(site: &DepositSite, fly: FlyMode, steps: Vec<MineStep>) -> Behavior {
    Behavior::MineScript {
        deposit_id: site.id.clone(),
        target_m: site.position_m,
        stop_radius_m: site.radius_m + 90.0,
        fly,
        steps,
        grace: Duration::from_millis(500),
    }
}

fn stop() -> FlyMode {
    FlyMode::Stop {
        timeout: Duration::from_secs(240),
    }
}

/// (상태, 사유) — 그 id 의 결과들.
fn results(o: &MiningObs, id: Uuid) -> Vec<(String, Option<String>)> {
    o.results
        .iter()
        .filter(|r| r.command_id == id)
        .map(|r| (r.status.clone(), r.reason_code.clone()))
        .collect()
}

fn total(o: &MiningObs, i: usize) -> i64 {
    o.inventory[i]
        .msg
        .payload
        .items
        .iter()
        .map(|x| x.quantity_kg)
        .sum()
}

/// (세션 시작 총량, 마지막 총량, 변화 횟수).
fn inv_track(o: &MiningObs) -> (Option<i64>, Option<i64>, usize) {
    if o.inventory.is_empty() {
        return (None, None, 0);
    }
    let first = total(o, 0);
    let mut prev = first;
    let mut changes = 0;
    for i in 1..o.inventory.len() {
        let t = total(o, i);
        if t != prev {
            changes += 1;
            prev = t;
        }
    }
    (Some(first), Some(prev), changes)
}

fn is_dup(r: &[(String, Option<String>)]) -> bool {
    r.len() == 1 && r[0].0 == "REJECTED" && r[0].1.as_deref() == Some("DUPLICATE_COMMAND_ID")
}

fn accepted(r: &[(String, Option<String>)]) -> bool {
    r.first().is_some_and(|x| x.0 == "ACCEPTED")
}

fn read_state(p: &Option<PathBuf>) -> Result<Value, String> {
    let p = p.as_ref().ok_or("--state 가 필요하다")?;
    let t = std::fs::read_to_string(p).map_err(|e| format!("{}: {e}", p.display()))?;
    serde_json::from_str(&t).map_err(|e| format!("{}: {e}", p.display()))
}

fn write_state(p: &Option<PathBuf>, v: &Value) -> Result<(), String> {
    let p = p.as_ref().ok_or("--state 가 필요하다")?;
    std::fs::write(p, serde_json::to_string_pretty(v).unwrap_or_default())
        .map_err(|e| format!("{}: {e}", p.display()))
}

fn world_ok(w: &str) -> Result<&str, String> {
    if sql::is_uuid(w) {
        Ok(w)
    } else {
        Err(format!("--world 가 UUID 가 아니다: {w:?}"))
    }
}

fn processed_rows(w: &str) -> Result<i64, String> {
    sql::scalar_i64(&format!(
        "select count(*) from processed_commands where world_id = '{w}'"
    ))
}

/// 재기동 증거: 두 번째 서버 로그에 "tick 을 이어서 시작한다 … last_tick=Some(" 줄.
fn restart_evidence(log: &Option<PathBuf>) -> Option<String> {
    let t = std::fs::read_to_string(log.as_ref()?).ok()?;
    t.lines()
        .find(|l| l.contains("tick 을 이어서 시작한다") && l.contains("last_tick=Some("))
        .map(|l| l.chars().take(300).collect())
}

// ───────────────────────────── SC-78 (b) ─────────────────────────────

pub async fn run_reconnect(
    site: &DepositSite,
    url: &str,
    secret: &str,
    label: &str,
) -> (Verdict, Value) {
    let x = Uuid::now_v7();
    let clock = Clock::start();
    let o1 = run_connection(spec(
        secret,
        url,
        label,
        script(
            site,
            stop(),
            vec![
                MineStep::MineId(x),
                MineStep::Wait(Duration::from_millis(1500)),
            ],
        ),
        clock,
    ))
    .await;
    // 잔류 창(30 s) 안에 곧바로 다시 붙는다.
    let o2 = run_connection(spec(
        secret,
        url,
        label,
        script(
            site,
            FlyMode::Stay,
            vec![
                MineStep::MineId(x),
                MineStep::Wait(Duration::from_millis(1500)),
            ],
        ),
        clock,
    ))
    .await;
    judge_reconnect(&o1, &o2, x)
}

pub fn judge_reconnect(
    o1: &ConnectionOutcome,
    o2: &ConnectionOutcome,
    x: Uuid,
) -> (Verdict, Value) {
    let (m1, m2) = (&o1.mining, &o2.mining);
    let r1 = results(m1, x);
    let r2 = results(m2, x);
    let (b1, a1, _) = inv_track(m1);
    let (b2, a2, c2) = inv_track(m2);
    let first_ok = accepted(&r1) && matches!((b1, a1), (Some(b), Some(a)) if a > b);
    let resumed = m1.own_ship_id.is_some() && m1.own_ship_id == m2.own_ship_id;
    let sent2 = m2.mine_sent.iter().any(|s| s.command_id == x);
    let d = json!({"command_id": x.to_string(), "first": r1, "resend": r2,
        "inventory_first_conn": [b1, a1], "inventory_second_conn_start": b2, "inventory_second_conn_end": a2,
        "inventory_changes_second_conn": c2, "resumed_same_ship": resumed,
        "ship_ids": [m1.own_ship_id.map(|s| s.to_string()), m2.own_ship_id.map(|s| s.to_string())]});
    // ⊘: 첫 전송 ACCEPTED·인벤토리 증가, 둘째 연결이 같은 함선을 이어받았다(잔류 창 안), 재전송이 나갔다.
    if !first_ok || !resumed || !sent2 {
        return (Verdict::Undecided, d);
    }
    let pass = is_dup(&r2) && c2 == 0 && b2 == a1;
    (if pass { Verdict::Pass } else { Verdict::Fail }, d)
}

// ───────────────────────────── SC-78 (c) ─────────────────────────────

pub async fn run_restart(
    site: &DepositSite,
    url: &str,
    secret: &str,
    label: &str,
    pa: &PhaseArgs,
) -> (Verdict, Value) {
    let w = match world_ok(&pa.world) {
        Ok(w) => w,
        Err(e) => return (Verdict::Undecided, json!({"error": e})),
    };
    match pa.phase.as_str() {
        "run" => {
            let x = Uuid::now_v7();
            let o = run_connection(spec(
                secret,
                url,
                label,
                script(
                    site,
                    stop(),
                    vec![
                        MineStep::MineId(x),
                        MineStep::Wait(Duration::from_millis(1500)),
                    ],
                ),
                Clock::start(),
            ))
            .await;
            let r = results(&o.mining, x);
            let (b, a, _) = inv_track(&o.mining);
            let st = json!({"command_id": x.to_string(), "results": r, "inventory_before": b,
                            "inventory_after": a, "processed_rows_after_run": processed_rows(w).ok()});
            if let Err(e) = write_state(&pa.state, &st) {
                return (Verdict::Undecided, json!({"error": e}));
            }
            let ok = accepted(&r) && matches!((b, a), (Some(b), Some(a)) if a > b);
            (
                if ok {
                    Verdict::Pass
                } else {
                    Verdict::Undecided
                },
                json!({"phase": "run", "state": st, "note": "단계 기록 — 판정은 resume 단계"}),
            )
        }
        "resume" => {
            let st = match read_state(&pa.state) {
                Ok(v) => v,
                Err(e) => return (Verdict::Undecided, json!({"error": e})),
            };
            let Some(x) = st["command_id"]
                .as_str()
                .and_then(|s| Uuid::parse_str(s).ok())
            else {
                return (
                    Verdict::Undecided,
                    json!({"error": "상태에 command_id 가 없다"}),
                );
            };
            let before = processed_rows(w);
            let o = run_connection(spec(
                secret,
                url,
                label,
                script(
                    site,
                    FlyMode::Stay,
                    vec![
                        MineStep::MineId(x),
                        MineStep::Wait(Duration::from_millis(1500)),
                    ],
                ),
                Clock::start(),
            ))
            .await;
            let after = processed_rows(w);
            let r = results(&o.mining, x);
            let (b2, a2, c2) = inv_track(&o.mining);
            let first_ok = st["results"][0][0].as_str() == Some("ACCEPTED")
                && st["inventory_after"].as_i64() > st["inventory_before"].as_i64();
            let restarted = restart_evidence(&pa.restart_log);
            let d = json!({"command_id": x.to_string(), "run_state": st, "resend": r,
                "inventory_after_restart_start": b2, "inventory_after_restart_end": a2, "inventory_changes": c2,
                "processed_rows_before_resend": before.clone().ok(), "processed_rows_after_resend": after.clone().ok(),
                "restart_log_line": restarted});
            // ⊘: 첫 전송 ACCEPTED·증가, 재기동이 실제로 일어났다(로그 줄), SQL 값이 있다.
            if !first_ok || restarted.is_none() || before.is_err() || after.is_err() {
                return (Verdict::Undecided, d);
            }
            let pass = is_dup(&r)
                && c2 == 0
                && b2 == st["inventory_after"].as_i64()
                && before.ok() == after.ok();
            (if pass { Verdict::Pass } else { Verdict::Fail }, d)
        }
        other => (
            Verdict::Undecided,
            json!({"error": format!("모르는 --phase {other}")}),
        ),
    }
}

// ───────────────────────────── SC-25 ─────────────────────────────

/// cas-halt 실행 단계(변조 실행·대조 실행 공통): 채굴 1 → ready 파일 → tampered 파일 대기 → 채굴 2.
pub async fn run_cas_halt(
    site: &DepositSite,
    url: &str,
    secret: &str,
    label: &str,
    pa: &PhaseArgs,
) -> (Verdict, Value) {
    if pa.phase == "judge" {
        return judge_cas_halt_files(pa);
    }
    let (Some(ready), Some(tampered)) = (pa.ready_file.clone(), pa.tampered_file.clone()) else {
        return (
            Verdict::Undecided,
            json!({"error": "--ready-file 와 --tampered-file 이 필요하다"}),
        );
    };
    let (x1, x2) = (Uuid::now_v7(), Uuid::now_v7());
    let o = run_connection(spec(
        secret,
        url,
        label,
        script(
            site,
            stop(),
            vec![
                MineStep::MineId(x1),
                MineStep::Wait(Duration::from_millis(1500)),
                MineStep::TouchFile(ready),
                MineStep::WaitFile {
                    path: tampered,
                    timeout: Duration::from_secs(180),
                },
                // 쿨다운(3 s) — 손잡기가 3 s 보다 빨리 끝나면 둘째 채굴이 COOLDOWN_ACTIVE 로 거부되어
                // 변조 경로가 아예 안 탄다(1차 실행에서 실제로 그랬다). 그래서 명시적으로 기다린다.
                MineStep::Wait(Duration::from_millis(3500)),
                MineStep::MineId(x2),
                // SC-25 (a): 정지가 결정된 **뒤** 게이트웨이에 도착한 채굴이 수락되지 않는가 — 정지 tick
                // 전후에 걸치도록 25 ms 간격으로 1 초. 대부분은 쿨다운·기록 지연·연결 종료로 끝난다.
                MineStep::Burst {
                    every: Duration::from_millis(25),
                    total: Duration::from_secs(1),
                },
                MineStep::Wait(Duration::from_secs(4)),
            ],
        ),
        Clock::start(),
    ))
    .await;
    let m = &o.mining;
    let (b, a, _) = inv_track(m);
    let mineral = m
        .inventory
        .last()
        .and_then(|i| i.msg.payload.items.first())
        .map(|i| i.mineral_id.clone());
    let st = json!({
        "x1": x1.to_string(), "x2": x2.to_string(), "x1_results": results(m, x1), "x2_results": results(m, x2),
        "inventory_before": b, "inventory_after": a, "mineral_id": mineral,
        "actor_id": o.ledger.session.as_ref().map(|s| s.actor_id.to_string()),
        "handshake_timeouts": m.signal_timeouts,
        "closed_by": o.ledger.session.as_ref().map(|s| s.close_initiator.to_string()),
        // 보낸 채굴 전부의 (id, 상태, 사유, tick) — 판정 단계가 정지 tick 으로 가른다.
        "all_mines": m.mine_sent.iter().map(|s| {
            let r = m.results.iter().find(|r| r.command_id == s.command_id);
            json!([s.command_id.to_string(), r.map(|r| r.status.clone()), r.and_then(|r| r.reason_code.clone()), r.map(|r| r.tick)])
        }).collect::<Vec<_>>(),
    });
    if let Err(e) = write_state(&pa.state, &st) {
        return (Verdict::Undecided, json!({"error": e}));
    }
    (
        Verdict::Pass,
        json!({"phase": "run", "state": st, "note": "단계 기록 — 판정은 judge 단계"}),
    )
}

fn judge_cas_halt_files(pa: &PhaseArgs) -> (Verdict, Value) {
    let st = read_state(&pa.state);
    let ct = read_state(&pa.control_state);
    let log = pa
        .server_log
        .as_ref()
        .and_then(|p| std::fs::read_to_string(p).ok());
    let clog = pa
        .control_log
        .as_ref()
        .and_then(|p| std::fs::read_to_string(p).ok());
    let (Ok(mut st), Ok(ct), Some(log), Some(clog)) = (st, ct, log, clog) else {
        return (
            Verdict::Undecided,
            json!({"error": "--state · --control-state · --server-log · --control-log 가 모두 필요하다"}),
        );
    };
    // 변조 값은 오케스트레이터만 안다(봇은 DB 에 쓰지 않는다) — 인자로 받는다.
    st["tampered_value"] = json!(pa.tampered_value);
    let w = match world_ok(&pa.world) {
        Ok(w) => w.to_owned(),
        Err(e) => return (Verdict::Undecided, json!({"error": e})),
    };
    let count_mined = |id: &str| -> Option<i64> {
        if !sql::is_uuid(id) {
            return None;
        }
        sql::scalar_i64(&format!(
            "select count(*) from domain_events where world_id = '{w}' and event_type = 'MINERAL_MINED' \
             and causation_id = '{id}'"
        ))
        .ok()
    };
    let x1_rows = st["x1"].as_str().and_then(count_mined);
    let x2_rows = st["x2"].as_str().and_then(count_mined);
    judge_cas_halt(
        &st,
        &ct,
        &log,
        &clog,
        pa.exit_code,
        pa.control_exit_code,
        x1_rows,
        x2_rows,
    )
}

/// 순수 판정.
#[allow(clippy::too_many_arguments)]
pub fn judge_cas_halt(
    st: &Value,
    ct: &Value,
    log: &str,
    clog: &str,
    exit_code: Option<i32>,
    control_exit: Option<i32>,
    x1_rows: Option<i64>,
    x2_rows: Option<i64>,
) -> (Verdict, Value) {
    let actor = st["actor_id"].as_str().unwrap_or("?");
    let mineral = st["mineral_id"].as_str().unwrap_or("?");
    let tampered = st["tampered_value"].as_i64();
    let halt_line = log
        .lines()
        .find(|l| l.contains("복구 불가 영속화 실패"))
        .map(str::to_owned);
    let fields = halt_line.as_ref().map(|l| {
        json!({
            "reason_cas": l.contains("비교 후 쓰기"),
            "key_actor": l.contains(actor),
            "key_mineral": l.contains(mineral),
            "expected": l.contains("expected="),
            "actual_db_value": tampered.is_some_and(|t| l.contains(&format!("actual={t}")) || l.contains(&format!("actual=Some({t})"))),
            "persist_fatal_total": l.contains("persist_fatal_total="),
        })
    });
    let all_fields = fields
        .as_ref()
        .is_some_and(|f| f.as_object().is_some_and(|o| o.values().all(|v| v == true)));
    // SC-25 (a) — 두 층(리더 판정 틀 2026-09-30): 정지 tick **뒤**에 판정된 채굴의 ACCEPTED 는 0 이어야
    // 한다. 정지 tick **에** 판정된 ACCEPTED 는 ADR-0013 이 정한 "설계상 손실 창"(응답이 영속보다 먼저
    // 간다) — 크기를 따로 적고 FAIL 로 세지 않는다.
    let halt_tick: Option<u64> = halt_line.as_ref().and_then(|l| {
        let i = l.find(" tick=")? + 6;
        l[i..]
            .split(|c: char| !c.is_ascii_digit())
            .next()?
            .parse()
            .ok()
    });
    let mines = st["all_mines"].as_array().cloned().unwrap_or_default();
    let tick_of = |m: &Value| m[3].as_u64();
    let after: Vec<&Value> = mines
        .iter()
        .filter(|m| halt_tick.is_some_and(|h| tick_of(m).is_some_and(|t| t > h)))
        .collect();
    let accepted_after = after.iter().filter(|m| m[1] == "ACCEPTED").count();
    let loss_window = mines
        .iter()
        .filter(|m| halt_tick.is_some() && tick_of(m) == halt_tick && m[1] == "ACCEPTED")
        .count();
    let no_result = mines.iter().filter(|m| m[1].is_null()).count();
    let x1_accepted = st["x1_results"][0][0].as_str() == Some("ACCEPTED");
    let control_x2_ok = ct["x2_results"][0][0].as_str() == Some("ACCEPTED");
    // 변조 실행의 둘째 채굴이 sim 판정을 통과해 **영속화까지 갔다** — 거부(쿨다운 등)면 정지 경로가
    // 안 탄 실행이다. 결과가 없을 수도 있다(정지로 연결이 먼저 닫힘) — 그건 거부가 아니다.
    let x2_reached_commit = st["x2_results"][0][0].as_str() != Some("REJECTED");
    let control_halted = clog.contains("복구 불가 영속화 실패");
    let d = json!({
        "exit_code": exit_code, "halt_log_line": halt_line.as_ref().map(|l| l.chars().take(400).collect::<String>()),
        "halt_line_fields": fields, "tampered_value": tampered,
        "x1_accepted_and_committed": [x1_accepted, x1_rows], "x2_mineral_mined_rows": x2_rows,
        "x2_results": st["x2_results"].clone(), "x2_reached_commit": x2_reached_commit,
        "control": {"x2_accepted": control_x2_ok, "halted": control_halted, "exit_code": control_exit},
        "handshake_timeouts": [st["handshake_timeouts"].clone(), ct["handshake_timeouts"].clone()],
        "sc25a_after_halt": {"halt_tick": halt_tick, "mines_sent": mines.len(),
            "results_after_halt_tick": after.len(), "accepted_after_halt_tick": accepted_after,
            "no_result_connection_closed": no_result},
        "designed_loss_window_accepted_in_halt_tick": loss_window,
    });
    // ⊘: 변조 **전** 채굴이 수락·커밋됐다(다른 이유로 죽은 것이 아님), 변조 값을 안다, 손잡기가 됐다,
    // 변조 없는 대조 실행이 있다.
    if !x1_accepted
        || !x2_reached_commit
        || !control_x2_ok
        || x1_rows != Some(1)
        || tampered.is_none()
        || st["handshake_timeouts"].as_u64() != Some(0)
        || ct["handshake_timeouts"].as_u64() != Some(0)
    {
        return (Verdict::Undecided, d);
    }
    let pass = exit_code.is_some_and(|c| c != 0)
        && accepted_after == 0
        && all_fields
        && x2_rows == Some(0)
        && control_x2_ok
        && !control_halted
        && control_exit == Some(0);
    (if pass { Verdict::Pass } else { Verdict::Fail }, d)
}

// ───────────────────────────── SC-26 ─────────────────────────────

pub async fn run_cas_reload(
    site: &DepositSite,
    url: &str,
    secret: &str,
    label: &str,
    pa: &PhaseArgs,
) -> (Verdict, Value) {
    let w = match world_ok(&pa.world) {
        Ok(w) => w.to_owned(),
        Err(e) => return (Verdict::Undecided, json!({"error": e})),
    };
    let hs = match read_state(&pa.halt_state) {
        Ok(v) => v,
        Err(e) => return (Verdict::Undecided, json!({"error": e})),
    };
    let tampered = pa.tampered_value.or(hs["tampered_value"].as_i64());
    let memory = hs["inventory_after"].as_i64();
    let x = Uuid::now_v7();
    let o = run_connection(spec(
        secret,
        url,
        label,
        script(
            site,
            stop(),
            vec![
                MineStep::MineId(x),
                MineStep::Wait(Duration::from_millis(1500)),
            ],
        ),
        Clock::start(),
    ))
    .await;
    let r = results(&o.mining, x);
    let start_inv = o.mining.inventory.first().map(|_| total(&o.mining, 0));
    let before_kg = sql::scalar(&format!(
        "select coalesce(payload->>'quantity_before_kg', '<missing>') from domain_events \
         where world_id = '{w}' and event_type = 'MINERAL_MINED' and causation_id = '{x}'"
    ));
    let before_kg_v = before_kg
        .as_ref()
        .ok()
        .and_then(|v| v.as_ref())
        .and_then(|v| v.parse::<i64>().ok());
    let restarted = restart_evidence(&pa.restart_log);
    // 계약 SC-26 은 "SC-25 직후" — 변조 실행의 서버가 **스스로 정지**(0 아닌 종료)한 뒤여야 한다.
    let halted = pa.exit_code.is_some_and(|c| c != 0);
    let d = json!({"tampered_value": tampered, "pre_halt_memory_value": memory, "mine": r,
        "preceding_halt_exit_code": pa.exit_code,
        "session_start_inventory": start_inv, "mineral_mined_quantity_before_kg": before_kg.map_err(|e| e.to_string()),
        "restart_log_line": restarted});
    // ⊘: 변조 값 ≠ 정지 전 메모리 값(같으면 "DB 가 정본" 을 못 가른다), 재기동이 실제로 일어났다.
    if tampered.is_none() || tampered == memory || restarted.is_none() || !halted {
        return (Verdict::Undecided, d);
    }
    let pass = accepted(&r) && before_kg_v == tampered && start_inv == tampered;
    (if pass { Verdict::Pass } else { Verdict::Fail }, d)
}

// ───────────────────────────── SC-108 ─────────────────────────────

/// `/debug/stats` 한 번 — 의존성 없이 HTTP/1.1 GET(Connection: close).
pub async fn fetch_stats(url: &str) -> Result<Value, String> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let host = url
        .trim_start_matches("ws://")
        .split('/')
        .next()
        .unwrap_or("127.0.0.1:8080")
        .to_owned();
    let mut s = tokio::time::timeout(
        Duration::from_secs(3),
        tokio::net::TcpStream::connect(&host),
    )
    .await
    .map_err(|_| "stats 연결 시간 초과".to_owned())?
    .map_err(|e| format!("stats 연결 실패: {e}"))?;
    let req = format!("GET /debug/stats HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n");
    s.write_all(req.as_bytes())
        .await
        .map_err(|e| e.to_string())?;
    let mut buf = Vec::new();
    tokio::time::timeout(Duration::from_secs(5), s.read_to_end(&mut buf))
        .await
        .map_err(|_| "stats 읽기 시간 초과".to_owned())?
        .map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&buf).to_string();
    let (head, body) = text
        .split_once("\r\n\r\n")
        .ok_or("HTTP 응답 형식이 아니다")?;
    let body = if head
        .to_ascii_lowercase()
        .contains("transfer-encoding: chunked")
    {
        let mut out = String::new();
        let mut rest = body;
        while let Some((len, tail)) = rest.split_once("\r\n") {
            let n = usize::from_str_radix(len.trim(), 16).map_err(|_| "chunk 길이")?;
            if n == 0 {
                break;
            }
            out.push_str(tail.get(..n).ok_or("chunk 가 짧다")?);
            rest = tail.get(n + 2..).unwrap_or("");
        }
        out
    } else {
        body.to_owned()
    };
    serde_json::from_str(&body).map_err(|e| format!("stats JSON: {e}"))
}

async fn liveness(url: &str) -> Value {
    tokio::time::sleep(Duration::from_secs(10)).await;
    let s1 = fetch_stats(url).await;
    tokio::time::sleep(Duration::from_secs(2)).await;
    let s2 = fetch_stats(url).await;
    let pick = |s: &Result<Value, String>, k: &str| s.as_ref().ok().map(|v| v[k].clone());
    json!({
        "stats_ok": s1.is_ok() && s2.is_ok(),
        "error": s1.as_ref().err().or(s2.as_ref().err()),
        "persist_halted": [pick(&s1, "persist_halted"), pick(&s2, "persist_halted")],
        "domain_events_persist_failed_total": pick(&s2, "domain_events_persist_failed_total"),
        "last_committed_tick": [pick(&s1, "last_committed_tick"), pick(&s2, "last_committed_tick")],
    })
}

fn alive(l: &Value) -> bool {
    l["stats_ok"] == true
        && l["persist_halted"][0] == false
        && l["persist_halted"][1] == false
        && l["domain_events_persist_failed_total"] == 0
        && l["last_committed_tick"][1].as_u64() > l["last_committed_tick"][0].as_u64()
}

pub async fn run_cross_actor(
    site: &DepositSite,
    url: &str,
    secret: &str,
    label_a: &str,
    pa: &PhaseArgs,
) -> (Verdict, Value) {
    let label_b = pa.label_b.as_str();
    if label_b.is_empty() || label_b == label_a {
        return (
            Verdict::Undecided,
            json!({"error": "--label-b(A 와 다른 신원)가 필요하다"}),
        );
    }
    match pa.phase.as_str() {
        "run" | "" => {
            let x = Uuid::now_v7();
            let clock = Clock::start();
            let go = Arc::new(tokio::sync::Notify::new());
            let a = script(
                site,
                stop(),
                vec![
                    MineStep::MineId(x),
                    MineStep::Wait(Duration::from_millis(1500)),
                    MineStep::Signal(go.clone()),
                    MineStep::Wait(Duration::from_millis(3000)),
                ],
            );
            let b = script(
                site,
                FlyMode::Stay,
                vec![
                    MineStep::WaitSignal {
                        notify: go.clone(),
                        timeout: Duration::from_secs(300),
                    },
                    MineStep::MineId(x),
                    MineStep::Wait(Duration::from_millis(1500)),
                ],
            );
            let (oa, ob) = tokio::join!(
                run_connection(spec(secret, url, label_a, a, clock)),
                run_connection(spec(secret, url, label_b, b, clock))
            );
            let live = liveness(url).await;
            if pa.state.is_some() {
                let _ = write_state(&pa.state, &json!({"command_id": x.to_string()}));
            }
            judge_cross(&oa, Some(&ob), x, &live, None)
        }
        "resume" => {
            let st = match read_state(&pa.state) {
                Ok(v) => v,
                Err(e) => return (Verdict::Undecided, json!({"error": e})),
            };
            let Some(x) = st["command_id"]
                .as_str()
                .and_then(|s| Uuid::parse_str(s).ok())
            else {
                return (
                    Verdict::Undecided,
                    json!({"error": "상태에 command_id 가 없다"}),
                );
            };
            // 재기동 뒤: B(A 와 다른 actor)가 A 의 옛 id 를 보낸다 — 정지가 아니라 DUPLICATE 여야 한다.
            let ob = run_connection(spec(
                secret,
                url,
                label_b,
                script(
                    site,
                    FlyMode::Stay,
                    vec![
                        MineStep::MineId(x),
                        MineStep::Wait(Duration::from_millis(1500)),
                    ],
                ),
                Clock::start(),
            ))
            .await;
            let live = liveness(url).await;
            let restarted = restart_evidence(&pa.restart_log);
            let (v, mut d) = judge_cross_resume(&ob, x, &live);
            d["restart_log_line"] = json!(restarted);
            if restarted.is_none() {
                return (Verdict::Undecided, d);
            }
            (v, d)
        }
        other => (
            Verdict::Undecided,
            json!({"error": format!("모르는 --phase {other}")}),
        ),
    }
}

pub fn judge_cross(
    oa: &ConnectionOutcome,
    ob: Option<&ConnectionOutcome>,
    x: Uuid,
    live: &Value,
    _unused: Option<()>,
) -> (Verdict, Value) {
    let Some(ob) = ob else {
        return (Verdict::Undecided, json!({"error": "B 결과 없음"}));
    };
    let ra = results(&oa.mining, x);
    let rb = results(&ob.mining, x);
    let actor_a = oa.ledger.session.as_ref().map(|s| s.actor_id);
    let actor_b = ob.ledger.session.as_ref().map(|s| s.actor_id);
    let a_sent = oa.mining.mine_sent.iter().any(|s| s.command_id == x);
    let b_sent = ob.mining.mine_sent.iter().any(|s| s.command_id == x);
    let d = json!({"command_id": x.to_string(), "a": {"actor": actor_a.map(|u| u.to_string()), "sent": a_sent, "results": ra},
        "b": {"actor": actor_b.map(|u| u.to_string()), "sent": b_sent, "results": rb}, "liveness": live,
        "signal_timeouts": [oa.mining.signal_timeouts, ob.mining.signal_timeouts]});
    // ⊘: 둘 다 같은 id 를 보냈고, actor 가 다르고, A 가 먼저 수락됐다.
    if !a_sent || !b_sent || actor_a.is_none() || actor_a == actor_b || !accepted(&ra) {
        return (Verdict::Undecided, d);
    }
    let pass = is_dup(&rb) && alive(live);
    (if pass { Verdict::Pass } else { Verdict::Fail }, d)
}

pub fn judge_cross_resume(ob: &ConnectionOutcome, x: Uuid, live: &Value) -> (Verdict, Value) {
    let rb = results(&ob.mining, x);
    let sent = ob.mining.mine_sent.iter().any(|s| s.command_id == x);
    let d = json!({"variant": "재기동 뒤", "command_id": x.to_string(), "b_sent": sent, "b_results": rb, "liveness": live});
    if !sent {
        return (Verdict::Undecided, d);
    }
    let pass = is_dup(&rb) && alive(live);
    (if pass { Verdict::Pass } else { Verdict::Fail }, d)
}

/// 경로 인자 헬퍼(main.rs 용).
pub fn opt_path(s: &str) -> Option<PathBuf> {
    if s.is_empty() {
        None
    } else {
        Some(Path::new(s).to_path_buf())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cas_halt_judge_controls() {
        let st = json!({"actor_id": "A1", "mineral_id": "glacine", "tampered_value": 927,
            "x1_results": [["ACCEPTED", null]], "handshake_timeouts": 0});
        let ct = json!({"x2_results": [["ACCEPTED", null]], "handshake_timeouts": 0});
        let good = "ERROR x: persist_fatal_total=1 error=비교 후 쓰기 불일치: inventory_items(A1, glacine) expected=Some(150) actual=Some(927) 복구 불가 영속화 실패 — 월드를 멈춘다";
        let clean = "INFO 정상 종료";
        assert_eq!(
            judge_cas_halt(&st, &ct, good, clean, Some(1), Some(0), Some(1), Some(0)).0,
            Verdict::Pass
        );
        // 음성: DB 값(actual) 없는 로그 줄 → FAIL(지금 서버 코드의 모양).
        let no_actual = good.replace(" actual=Some(927)", "");
        assert_eq!(
            judge_cas_halt(
                &st,
                &ct,
                &no_actual,
                clean,
                Some(1),
                Some(0),
                Some(1),
                Some(0)
            )
            .0,
            Verdict::Fail
        );
        // 음성: 대조 실행도 정지 → FAIL("무엇이든 멈춘다").
        assert_eq!(
            judge_cas_halt(&st, &ct, good, good, Some(1), Some(1), Some(1), Some(0)).0,
            Verdict::Fail
        );
        // 음성: 그 채굴의 MINERAL_MINED 가 DB 에 있다 → FAIL.
        assert_eq!(
            judge_cas_halt(&st, &ct, good, clean, Some(1), Some(0), Some(1), Some(1)).0,
            Verdict::Fail
        );
        // SC-25 (a): 정지 tick(1017) **뒤**에 수락된 채굴 → FAIL. 정지 tick **에** 수락된 것은 손실 창(통과).
        let ticked = good.replace(
            "persist_fatal_total=1",
            "tick=1017 events=1 persist_fatal_total=1",
        );
        let mut in_window = st.clone();
        in_window["all_mines"] = json!([
            ["a", "ACCEPTED", null, 1017],
            ["b", "REJECTED", "RECORDING_BACKLOG", 1018]
        ]);
        let (v, d) = judge_cas_halt(
            &in_window,
            &ct,
            &ticked,
            clean,
            Some(1),
            Some(0),
            Some(1),
            Some(0),
        );
        assert_eq!(v, Verdict::Pass);
        assert_eq!(d["designed_loss_window_accepted_in_halt_tick"], 1);
        let mut leaked = st.clone();
        leaked["all_mines"] = json!([["a", "ACCEPTED", null, 1017], ["b", "ACCEPTED", null, 1018]]);
        assert_eq!(
            judge_cas_halt(
                &leaked,
                &ct,
                &ticked,
                clean,
                Some(1),
                Some(0),
                Some(1),
                Some(0)
            )
            .0,
            Verdict::Fail
        );
        // ⊘: 변조 실행의 둘째 채굴이 쿨다운으로 거부 → 정지 경로 미발생 → 미검증(1차 실행의 실제 모양).
        let mut cool = st.clone();
        cool["x2_results"] = json!([["REJECTED", "COOLDOWN_ACTIVE"]]);
        assert_eq!(
            judge_cas_halt(&cool, &ct, "", clean, Some(0), Some(0), Some(1), Some(0)).0,
            Verdict::Undecided
        );
        // ⊘: 변조 전 채굴이 커밋되지 않았다 → 미검증.
        assert_eq!(
            judge_cas_halt(&st, &ct, good, clean, Some(1), Some(0), Some(0), Some(0)).0,
            Verdict::Undecided
        );
    }

    #[test]
    fn alive_needs_advancing_commit() {
        let ok = json!({"stats_ok": true, "persist_halted": [false, false], "domain_events_persist_failed_total": 0,
                        "last_committed_tick": [100, 140]});
        assert!(alive(&ok));
        let stuck = json!({"stats_ok": true, "persist_halted": [false, false], "domain_events_persist_failed_total": 0,
                           "last_committed_tick": [100, 100]});
        assert!(!alive(&stuck));
        let halted = json!({"stats_ok": true, "persist_halted": [false, true], "domain_events_persist_failed_total": 0,
                            "last_committed_tick": [100, 140]});
        assert!(!alive(&halted));
    }
}

//! p1-02 채굴 부하·경합 케이스 — `backlog-idle`(SC-104). (race-same-tick·last-kg·notice-gap·backlog 는
//! 이어서 여기에 둔다.)
//!
//! `/debug/stats` 를 **연결과 동시에** 폴링한다 — 톱니(하트비트 20 tick 주기의 `persist_backlog`)가
//! 실제로 생겼는지를 같은 실행에서 보여야 "옛 지표였다면 거부됐을 조건" 이 있었다고 말할 수 있다.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use serde_json::{Value, json};
use tokio::sync::Notify;
use uuid::Uuid;

use crate::conn::{Behavior, BotSpec, Clock, FlyMode, MineStep, run_connection};
use crate::mine_cases::Verdict;
use crate::mine_run::DepositSite;
use crate::restart_cases::fetch_stats;
use crate::token;

/// 폴링 요약.
#[derive(Debug, Default, Clone)]
pub struct StatsPoll {
    pub samples: u64,
    pub errors: u64,
    pub max_persist_backlog: u64,
    pub max_recording_lag: u64,
    pub persist_backlog_limit: Option<u64>,
    pub recording_lag_limit: Option<u64>,
}

/// `stop` 이 켜질 때까지 `every` 마다 폴링한다.
pub async fn poll_stats(url: String, every: Duration, stop: Arc<AtomicBool>) -> StatsPoll {
    let mut p = StatsPoll::default();
    while !stop.load(Ordering::Relaxed) {
        match fetch_stats(&url).await {
            Ok(v) => {
                p.samples += 1;
                p.max_persist_backlog = p
                    .max_persist_backlog
                    .max(v["persist_backlog"].as_u64().unwrap_or(0));
                p.max_recording_lag = p
                    .max_recording_lag
                    .max(v["recording_lag"].as_u64().unwrap_or(0));
                p.persist_backlog_limit = v["persist_backlog_limit"].as_u64();
                p.recording_lag_limit = v["recording_lag_limit"].as_u64();
            }
            Err(_) => p.errors += 1,
        }
        tokio::time::sleep(every).await;
    }
    p
}

/// SC-104: DB 정상·한가한 서버, 봇 1대가 쿨다운마다 채굴 60 초 → `RECORDING_BACKLOG` 0.
pub async fn run_backlog_idle(
    site: &DepositSite,
    url: &str,
    secret: &str,
    label: &str,
) -> (Verdict, Value) {
    let (_s, tok) = token::identity(secret, label);
    let stop = Arc::new(AtomicBool::new(false));
    let poller = tokio::spawn(poll_stats(
        url.to_owned(),
        Duration::from_millis(30),
        stop.clone(),
    ));
    let o = run_connection(BotSpec {
        label: label.to_owned(),
        url: url.to_owned(),
        token: tok,
        behavior: Behavior::MineScript {
            deposit_id: site.id.clone(),
            target_m: site.position_m,
            stop_radius_m: site.radius_m + 90.0,
            fly: FlyMode::Stop {
                timeout: Duration::from_secs(240),
            },
            // 쿨다운 3 s 보다 조금 길게 — 쿨다운 거부가 아니라 기록 지연 거부만 보려는 것이다.
            steps: vec![MineStep::Burst {
                every: Duration::from_millis(3100),
                total: Duration::from_secs(62),
            }],
            grace: Duration::from_secs(1),
        },
        clock: Clock::start(),
        live_corr: None,
        capture_raw: false,
    })
    .await;
    stop.store(true, Ordering::Relaxed);
    let poll = poller.await.unwrap_or_default();
    judge_backlog_idle(&o.mining, &poll)
}

pub fn judge_backlog_idle(m: &crate::mining::MiningObs, poll: &StatsPoll) -> (Verdict, Value) {
    let sent = m.mine_sent.len();
    let mut by: std::collections::BTreeMap<String, usize> = Default::default();
    for s in &m.mine_sent {
        if let Some(r) = m.results.iter().find(|r| r.command_id == s.command_id) {
            let k = if r.status == "ACCEPTED" {
                "ACCEPTED".to_owned()
            } else {
                r.reason_code.clone().unwrap_or_else(|| "<null>".into())
            };
            *by.entry(k).or_default() += 1;
        } else {
            *by.entry("<no result>".into()).or_default() += 1;
        }
    }
    let backlog_rejects = by.get("RECORDING_BACKLOG").copied().unwrap_or(0);
    let d = json!({"sent": sent, "results_by": by, "stats_samples": poll.samples, "stats_errors": poll.errors,
        "max_persist_backlog": poll.max_persist_backlog, "max_recording_lag": poll.max_recording_lag,
        "recording_lag_limit": poll.recording_lag_limit, "persist_backlog_limit": poll.persist_backlog_limit,
        // architect S11 판정 기준(관측 전 등록) — SC-104 판정에 넣지 않는 **별도 기록**.
        "s11_criterion_max_lag_le_5_with_sawtooth": poll.max_recording_lag <= 5 && poll.max_persist_backlog >= 20});
    // ⊘: 채굴을 충분히 보냈다(≥ 15), 그리고 톱니가 실제로 생겼다(persist_backlog 최대 ≥ 20).
    if sent < 15 || poll.max_persist_backlog < 20 {
        return (Verdict::Undecided, d);
    }
    (
        if backlog_rejects == 0 {
            Verdict::Pass
        } else {
            Verdict::Fail
        },
        d,
    )
}

/// SC-84 시도 한 번의 입력: 같은 광물의 두 광맥.
pub const RACE_PAIRS: [(&str, &str, &str); 3] = [
    ("ferrosite", "inner-belt-1", "inner-belt-3"),
    ("glacine", "vela-orbit-1", "vela-orbit-2"),
    ("cobaltine", "outer-field-1", "outer-field-2"),
];

type Outcome = crate::conn::ConnectionOutcome;

/// SC-84 한 시도: A·B 가 각자 광맥에 서서 코디네이터의 "발사" 신호를 받고 **동시에** 캔다.
pub async fn race_attempt(
    a_site: &DepositSite,
    b_site: &DepositSite,
    url: &str,
    secret: &str,
    label_a: &str,
    label_b: &str,
) -> (Outcome, Outcome, Uuid, Uuid) {
    let clock = Clock::start();
    let (arr_a, arr_b) = (Arc::new(Notify::new()), Arc::new(Notify::new()));
    let (go_a, go_b) = (Arc::new(Notify::new()), Arc::new(Notify::new()));
    let (xa, xb) = (Uuid::now_v7(), Uuid::now_v7());
    let beh =
        |site: &DepositSite, arr: Arc<Notify>, go: Arc<Notify>, x: Uuid| Behavior::MineScript {
            deposit_id: site.id.clone(),
            target_m: site.position_m,
            stop_radius_m: site.radius_m + 90.0,
            fly: FlyMode::Stop {
                timeout: Duration::from_secs(300),
            },
            steps: vec![
                MineStep::Signal(arr),
                MineStep::WaitSignal {
                    notify: go,
                    timeout: Duration::from_secs(400),
                },
                MineStep::MineId(x),
                MineStep::Wait(Duration::from_secs(3)),
            ],
            grace: Duration::from_millis(500),
        };
    let spec = |label: &str, behavior| {
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
    };
    let coordinator = {
        let (arr_a, arr_b, go_a, go_b) = (arr_a.clone(), arr_b.clone(), go_a.clone(), go_b.clone());
        async move {
            let both = async {
                arr_a.notified().await;
                arr_b.notified().await;
            };
            let _ = tokio::time::timeout(Duration::from_secs(400), both).await;
            // 두 신호를 연달아 — 두 봇의 전송 사이 간격은 스케줄링 지연뿐이다.
            go_a.notify_one();
            go_b.notify_one();
        }
    };
    let (oa, ob, ()) = tokio::join!(
        run_connection(spec(label_a, beh(a_site, arr_a.clone(), go_a.clone(), xa))),
        run_connection(spec(label_b, beh(b_site, arr_b.clone(), go_b.clone(), xb))),
        coordinator
    );
    (oa, ob, xa, xb)
}

type Res = Option<(String, Option<String>, u64)>;

fn result_of(m: &crate::mining::MiningObs, id: Uuid) -> Res {
    m.results
        .iter()
        .find(|r| r.command_id == id)
        .map(|r| (r.status.clone(), r.reason_code.clone(), r.tick))
}

/// SC-84 전체: 쌍마다 시도, 같은 tick 이 된 첫 시도로 판정. 무효 시도 수를 찍는다.
pub async fn run_race(
    data_dir: &std::path::Path,
    url: &str,
    secret: &str,
    label_base: u32,
    world: &str,
) -> (Verdict, Value) {
    if !crate::sql::is_uuid(world) {
        return (
            Verdict::Undecided,
            json!({"error": "--world 가 UUID 가 아니다"}),
        );
    }
    let mut attempts = Vec::new();
    for (i, (mineral, da, db_)) in RACE_PAIRS.iter().enumerate() {
        let (Ok(sa), Ok(sb)) = (
            crate::mine_run::find_deposit(data_dir, da),
            crate::mine_run::find_deposit(data_dir, db_),
        ) else {
            return (
                Verdict::Undecided,
                json!({"error": "광맥을 data/ 에서 못 찾았다"}),
            );
        };
        // 이 광물이 이미 발견된 월드면 시도 무효(발견 판정이 안 걸린다).
        let before = crate::sql::scalar_i64(&format!(
            "select count(*) from historical_events where world_id = '{world}' and payload->>'mineral_id' = '{mineral}'"
        ))
        .ok();
        let la = format!("bot-{:03}", label_base + 2 * i as u32);
        let lb = format!("bot-{:03}", label_base + 2 * i as u32 + 1);
        let (oa, ob, xa, xb) = race_attempt(&sa, &sb, url, secret, &la, &lb).await;
        let (ra, rb) = (result_of(&oa.mining, xa), result_of(&ob.mining, xb));
        let facts = race_sql(world, mineral, xa, xb);
        let (v, d) = judge_race(mineral, before, &ra, &rb, &facts);
        attempts.push(d);
        if v != Verdict::Undecided {
            let invalid = attempts.len() - 1;
            return (
                v,
                json!({"attempts": attempts, "invalid_attempts": invalid, "world_id": world}),
            );
        }
    }
    let n = attempts.len();
    (
        Verdict::Undecided,
        json!({"attempts": attempts, "invalid_attempts": n, "world_id": world,
               "why": "같은 tick 에 처리된 시도가 없다 — 새 월드로 다시"}),
    )
}

/// SQL 사실: 두 채굴의 (sequence, actor), 그 광물의 역사 기록 수와 발견자.
#[derive(Debug, Clone, Default)]
pub struct RaceFacts {
    pub seq_a: Option<(i64, String)>,
    pub seq_b: Option<(i64, String)>,
    pub records: Option<i64>,
    pub discoverer: Option<String>,
}

fn race_sql(world: &str, mineral: &str, xa: Uuid, xb: Uuid) -> RaceFacts {
    let seq = |x: Uuid| -> Option<(i64, String)> {
        let r = crate::sql::rows(&format!(
            "select sequence, actor_id from domain_events where world_id = '{world}' \
             and event_type = 'MINERAL_MINED' and causation_id = '{x}'"
        ))
        .ok()?;
        let row = r.into_iter().next()?;
        Some((row.first()?.parse().ok()?, row.get(1)?.clone()))
    };
    let recs = crate::sql::rows(&format!(
        "select p->>'entity_id' from historical_events h, jsonb_array_elements(h.participants) p \
         where h.world_id = '{world}' and h.payload->>'mineral_id' = '{mineral}' and p->>'role' = 'DISCOVERER'"
    ))
    .ok();
    RaceFacts {
        seq_a: seq(xa),
        seq_b: seq(xb),
        records: recs.as_ref().map(|r| r.len() as i64),
        discoverer: recs.and_then(|r| r.into_iter().next().and_then(|x| x.into_iter().next())),
    }
}

pub fn judge_race(
    mineral: &str,
    before: Option<i64>,
    ra: &Res,
    rb: &Res,
    f: &RaceFacts,
) -> (Verdict, Value) {
    let same_tick = matches!((ra, rb), (Some(a), Some(b)) if a.2 == b.2);
    let both_acc = matches!((ra, rb), (Some(a), Some(b)) if a.0 == "ACCEPTED" && b.0 == "ACCEPTED");
    let expect = match (&f.seq_a, &f.seq_b) {
        (Some(a), Some(b)) => Some(if a.0 < b.0 { a.1.clone() } else { b.1.clone() }),
        _ => None,
    };
    let d = json!({"mineral": mineral, "records_before": before,
        "result_a": ra, "result_b": rb, "same_tick": same_tick,
        "seq_a": f.seq_a, "seq_b": f.seq_b, "expected_discoverer_smaller_sequence": expect,
        "records_after": f.records, "discoverer": f.discoverer});
    // ⊘: 새 광물(기록 0), 둘 다 수락, **같은 tick** — 아니면 무효 시도.
    if before != Some(0) || !both_acc || !same_tick || expect.is_none() {
        return (Verdict::Undecided, d);
    }
    let pass = f.records == Some(1) && f.discoverer == expect;
    (if pass { Verdict::Pass } else { Verdict::Fail }, d)
}

/// SC-85 대상 — 잔량이 정확히 100 이 될 수 있는 유일한 광물(glacine: 10000 ≡ 100 mod 150, 산출 150 ≥ 100).
pub const LASTKG_DEPOSIT: &str = "vela-orbit-2";
const LASTKG_MINERAL: &str = "glacine";
const PREP_BOTS: usize = 8;

/// 회복식(I-69) 입력 — 광물 표에서 읽는다. **판정 기대값이 아니라** "처리 tick 에 잔량이 100 이었다"
/// 는 ⊘ 전제를 계산하는 입력이다(계약 SC-85 ⊘: 실제 tick 으로 식에 넣어 단언).
#[derive(Debug, Clone, Copy)]
pub struct Regen {
    pub initial: i64,
    pub regen_kg: i64,
    pub interval_ticks: i64,
}

pub fn effective(r: Regen, remaining: i64, as_of: i64, t: i64) -> i64 {
    let k = t.div_euclid(r.interval_ticks) - as_of.div_euclid(r.interval_ticks);
    r.initial.min(remaining + r.regen_kg * k.max(0))
}

fn regen_from_data(data_dir: &std::path::Path, initial: i64) -> Result<Regen, String> {
    let p = data_dir
        .join("minerals")
        .join(format!("{LASTKG_MINERAL}.json"));
    let v: Value = serde_json::from_str(
        &std::fs::read_to_string(&p).map_err(|e| format!("{}: {e}", p.display()))?,
    )
    .map_err(|e| e.to_string())?;
    let regen_kg = v["regeneration"]["regen_kg"].as_i64().ok_or("regen_kg")?;
    let secs = v["regeneration"]["regen_interval_s"]
        .as_i64()
        .ok_or("regen_interval_s")?;
    Ok(Regen {
        initial,
        regen_kg,
        interval_ticks: secs * 20,
    })
}

/// DB 의 광맥 상태 (remaining_kg, as_of_tick). 행이 없으면 초기값.
fn deposit_row(world: &str) -> Option<(i64, i64)> {
    let r = crate::sql::rows(&format!(
        "select remaining_kg, as_of_tick from deposit_states where world_id = '{world}' and deposit_id = '{LASTKG_DEPOSIT}'"
    ))
    .ok()?;
    let row = r.into_iter().next()?;
    Some((row.first()?.parse().ok()?, row.get(1)?.parse().ok()?))
}

async fn server_tick(url: &str) -> Option<i64> {
    fetch_stats(url).await.ok()?["tick"].as_i64()
}

type RemoteTx = tokio::sync::mpsc::UnboundedSender<crate::conn::RemoteCmd>;

fn remote_bot(site: &DepositSite, arrived: Arc<Notify>) -> (Behavior, RemoteTx) {
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    let beh = Behavior::MineScript {
        deposit_id: site.id.clone(),
        target_m: site.position_m,
        stop_radius_m: site.radius_m + 90.0,
        fly: FlyMode::Stop {
            timeout: Duration::from_secs(300),
        },
        steps: vec![
            MineStep::Signal(arrived),
            MineStep::Remote(Arc::new(tokio::sync::Mutex::new(rx))),
            MineStep::Wait(Duration::from_millis(500)),
        ],
        grace: Duration::from_millis(500),
    };
    (beh, tx)
}

/// SC-85 한 실행.
pub async fn run_last_kg(
    data_dir: &std::path::Path,
    url: &str,
    secret: &str,
    label_base: u32,
    world: &str,
) -> (Verdict, Value) {
    if !crate::sql::is_uuid(world) {
        return (
            Verdict::Undecided,
            json!({"error": "--world 가 UUID 가 아니다"}),
        );
    }
    let site = match crate::mine_run::find_deposit(data_dir, LASTKG_DEPOSIT) {
        Ok(s) => s,
        Err(e) => return (Verdict::Undecided, json!({"error": e})),
    };
    let initial = deposit_row(world).map(|r| r.0).unwrap_or(10_000);
    let regen = match regen_from_data(data_dir, 10_000) {
        Ok(r) => r,
        Err(e) => return (Verdict::Undecided, json!({"error": e})),
    };
    let clock = Clock::start();
    let n = PREP_BOTS + 3;
    let mut conns = Vec::new();
    let mut txs = Vec::new();
    let mut arrivals = Vec::new();
    for i in 0..n {
        let arr = Arc::new(Notify::new());
        let (beh, tx) = remote_bot(&site, arr.clone());
        let label = format!("bot-{:03}", label_base + i as u32);
        let (_s, tok) = token::identity(secret, &label);
        conns.push(tokio::spawn(run_connection(BotSpec {
            label,
            url: url.to_owned(),
            token: tok,
            behavior: beh,
            clock,
            live_corr: None,
            capture_raw: false,
        })));
        txs.push(tx);
        arrivals.push(arr);
    }
    let mut log: Vec<Value> = Vec::new();
    let arrived_all = tokio::time::timeout(Duration::from_secs(360), async {
        for a in &arrivals {
            a.notified().await;
        }
    })
    .await
    .is_ok();
    let (prep, shooters) = txs.split_at(PREP_BOTS);
    let mut fired: Option<(i64, i64, i64)> = None; // (발사 직전 remaining, as_of, 그때 tick)
    if arrived_all {
        let mut rr = 0usize;
        let deadline = tokio::time::Instant::now() + Duration::from_secs(600);
        let mut last_sent: Vec<Option<tokio::time::Instant>> = vec![None; PREP_BOTS];
        while tokio::time::Instant::now() < deadline {
            let (Some((rem, asof)), Some(t)) = (
                deposit_row(world).or(Some((initial, 0))),
                server_tick(url).await,
            ) else {
                tokio::time::sleep(Duration::from_millis(200)).await;
                continue;
            };
            let e = effective(regen, rem, asof, t);
            let to_boundary = regen.interval_ticks - t.rem_euclid(regen.interval_ticks);
            if e == 100 {
                if to_boundary > 60 {
                    fired = Some((rem, asof, t));
                    for s in shooters {
                        let _ = s.send(crate::conn::RemoteCmd::Mine);
                    }
                    break;
                }
                // 경계 직전 — 회복이 끼어들기 전에 쏠 수 없다. 경계를 지나 다시 내린다.
                tokio::time::sleep(Duration::from_millis(500)).await;
                continue;
            }
            // 남은 양이 많으면 여러 봇을 겹쳐 빠르게, 550 이하에서는 한 발씩 커밋을 확인하며.
            let single = e <= 550;
            let idx = rr % PREP_BOTS;
            let ready = last_sent[idx].is_none_or(|t0| t0.elapsed() >= Duration::from_millis(3300));
            if ready {
                let _ = prep[idx].send(crate::conn::RemoteCmd::Mine);
                last_sent[idx] = Some(tokio::time::Instant::now());
                rr += 1;
                if single {
                    // 그 채굴이 커밋될 때까지(as_of 가 바뀔 때까지) 기다린다.
                    let before = deposit_row(world);
                    let wait_until = tokio::time::Instant::now() + Duration::from_secs(5);
                    while deposit_row(world) == before && tokio::time::Instant::now() < wait_until {
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                    log.push(json!({"step": "single", "effective_before": e, "tick": t}));
                }
            }
            tokio::time::sleep(Duration::from_millis(if single { 50 } else { 400 })).await;
        }
        tokio::time::sleep(Duration::from_secs(3)).await;
    }
    for tx in &txs {
        let _ = tx.send(crate::conn::RemoteCmd::Stop);
    }
    let mut outs = Vec::new();
    for c in conns {
        if let Ok(o) = c.await {
            outs.push(o);
        }
    }
    let shooter_obs: Vec<&crate::mining::MiningObs> =
        outs.iter().skip(PREP_BOTS).map(|o| &o.mining).collect();
    let shots: Vec<Value> = shooter_obs
        .iter()
        .map(|m| {
            let s = m.mine_sent.first();
            let r = s.and_then(|s| m.results.iter().find(|r| r.command_id == s.command_id));
            let inv: Vec<i64> = m
                .inventory
                .iter()
                .map(|i| i.msg.payload.items.iter().map(|x| x.quantity_kg).sum())
                .collect();
            json!({"command_id": s.map(|s| s.command_id.to_string()),
                   "status": r.map(|r| r.status.clone()), "reason": r.and_then(|r| r.reason_code.clone()),
                   "tick": r.map(|r| r.tick), "inventory_totals": inv})
        })
        .collect();
    let accepted_qty = shots
        .iter()
        .find(|s| s["status"] == "ACCEPTED")
        .and_then(|s| s["command_id"].as_str())
        .and_then(|id| {
            crate::sql::rows(&format!(
                "select payload->>'quantity_kg', payload->>'deposit_remaining_before_kg', payload->>'deposit_remaining_after_kg' \
                 from domain_events where world_id = '{world}' and event_type = 'MINERAL_MINED' and causation_id = '{id}'"
            ))
            .ok()
        });
    judge_last_kg(arrived_all, fired, regen, &shots, accepted_qty, &log)
}

pub fn judge_last_kg(
    arrived_all: bool,
    fired: Option<(i64, i64, i64)>,
    regen: Regen,
    shots: &[Value],
    accepted_row: Option<Vec<Vec<String>>>,
    log: &[Value],
) -> (Verdict, Value) {
    let ticks: Vec<Option<u64>> = shots.iter().map(|s| s["tick"].as_u64()).collect();
    let same_tick = ticks.len() == 3 && ticks.iter().all(|t| t.is_some() && *t == ticks[0]);
    let proc_tick = ticks.first().copied().flatten().map(|t| t as i64);
    let eff_at_proc = match (fired, proc_tick) {
        (Some((rem, asof, _)), Some(t)) => Some(effective(regen, rem, asof, t)),
        _ => None,
    };
    let accepted = shots.iter().filter(|s| s["status"] == "ACCEPTED").count();
    let depleted = shots
        .iter()
        .filter(|s| s["status"] == "REJECTED" && s["reason"] == "RESOURCE_DEPLETED")
        .count();
    let gain: i64 = shots
        .iter()
        .map(|s| {
            let v: Vec<i64> = s["inventory_totals"]
                .as_array()
                .map(|a| a.iter().filter_map(Value::as_i64).collect())
                .unwrap_or_default();
            v.last().copied().unwrap_or(0) - v.first().copied().unwrap_or(0)
        })
        .sum();
    let row = accepted_row.and_then(|r| r.into_iter().next());
    let row_vals: Option<(i64, i64, i64)> = row.as_ref().and_then(|r| {
        Some((
            r.first()?.parse().ok()?,
            r.get(1)?.parse().ok()?,
            r.get(2)?.parse().ok()?,
        ))
    });
    let d = json!({"all_arrived": arrived_all, "fired_state": fired, "processing_tick": proc_tick,
        "effective_remaining_at_processing_tick": eff_at_proc, "regen": {"regen_kg": regen.regen_kg, "interval_ticks": regen.interval_ticks},
        "shots": shots, "same_tick": same_tick, "accepted": accepted, "resource_depleted": depleted,
        "inventory_gain_sum": gain, "accepted_mineral_mined_row(quantity,before,after)": row_vals,
        "single_steps": log.len()});
    // ⊘: 모두 도착, 세 명령이 같은 tick, 처리 tick 의 식(I-69) 잔량 = 100.
    if !arrived_all || !same_tick || eff_at_proc != Some(100) {
        return (Verdict::Undecided, d);
    }
    let pass = accepted == 1 && depleted == 2 && gain == 100 && row_vals == Some((100, 100, 0));
    (if pass { Verdict::Pass } else { Verdict::Fail }, d)
}

/// 폴링 표본 하나(벽시계 ms, tick, recording_lag, persist_backlog).
#[derive(Debug, Clone, Copy)]
pub struct StatSample {
    pub wall_ms: u64,
    pub tick: u64,
    pub recording_lag: u64,
    pub persist_backlog: u64,
    /// S13 — 커밋 대기 중 가장 오래된 배치의 tick(없으면 `None`). ADR-0013 K2b 직접 기준용.
    pub oldest_pending_tick: Option<u64>,
}

async fn sample_series(url: String, every: Duration, stop: Arc<AtomicBool>) -> Vec<StatSample> {
    let mut v = Vec::new();
    while !stop.load(Ordering::Relaxed) {
        if let Ok(s) = fetch_stats(&url).await {
            let wall_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0);
            v.push(StatSample {
                wall_ms,
                tick: s["tick"].as_u64().unwrap_or(0),
                recording_lag: s["recording_lag"].as_u64().unwrap_or(0),
                persist_backlog: s["persist_backlog"].as_u64().unwrap_or(0),
                oldest_pending_tick: s["oldest_pending_tick"].as_u64(),
            });
        }
        tokio::time::sleep(every).await;
    }
    v
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// `docker compose <verb> postgres` — 계약 SC-29~31 의 qa 절차(stop/start). `down` 은 절대 쓰지 않는다.
fn compose_postgres(verb: &str) -> Result<(), String> {
    assert!(verb == "stop" || verb == "start", "stop/start 만 허용");
    let out = std::process::Command::new("docker")
        .args(["compose", verb, "postgres"])
        .output()
        .map_err(|e| format!("docker compose {verb}: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(format!(
            "docker compose {verb} postgres 실패: {}",
            String::from_utf8_lossy(&out.stderr)
        ))
    }
}

/// 증거 DB 지문(리더 규칙 2026-09-30): spike 월드 행 수 + event_id md5, `_sqlx_migrations` 행 수.
/// postgres 를 멈췄다 켜기 **전후**로 같아야 한다 — 멈춤이 증거를 건드리지 않았다는 단언.
const SPIKE_WORLD: &str = "01a0b1c2-3d4e-7f01-8a2b-9c0d1e2f3a4b";

fn db_fingerprint() -> Result<String, String> {
    let r = crate::sql::rows(&format!(
        "select (select count(*) from domain_events where world_id = '{SPIKE_WORLD}'),          (select md5(coalesce(string_agg(event_id::text, ',' order by event_id), '')) from domain_events           where world_id = '{SPIKE_WORLD}'), (select count(*) from _sqlx_migrations)"
    ))?;
    let row = r
        .into_iter()
        .next()
        .ok_or("지문 질의가 행을 돌려주지 않았다")?;
    Ok(row.join("|"))
}

/// DB 가 다시 받을 때까지(최대 `secs`) 지문을 재시도한다.
async fn db_fingerprint_retry(secs: u64) -> Result<String, String> {
    let end = tokio::time::Instant::now() + Duration::from_secs(secs);
    loop {
        match tokio::task::spawn_blocking(db_fingerprint).await {
            Ok(Ok(v)) => return Ok(v),
            Ok(Err(e)) if tokio::time::Instant::now() >= end => return Err(e),
            Err(e) => return Err(e.to_string()),
            _ => tokio::time::sleep(Duration::from_millis(500)).await,
        }
    }
}

/// SC-29·30·31 한 실행(+ SC-32 용 수락 id 파일).
pub async fn run_backlog(
    site: &DepositSite,
    url: &str,
    secret: &str,
    label: &str,
    accepted_out: Option<&std::path::Path>,
) -> (Verdict, Value) {
    let clock = Clock::start();
    let arrived = Arc::new(Notify::new());
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    let (_s, tok) = token::identity(secret, label);
    let conn = tokio::spawn(run_connection(BotSpec {
        label: label.to_owned(),
        url: url.to_owned(),
        token: tok,
        behavior: Behavior::MineScript {
            deposit_id: site.id.clone(),
            target_m: site.position_m,
            stop_radius_m: site.radius_m + 90.0,
            fly: FlyMode::Stop {
                timeout: Duration::from_secs(300),
            },
            steps: vec![
                MineStep::Signal(arrived.clone()),
                MineStep::Remote(Arc::new(tokio::sync::Mutex::new(rx))),
                MineStep::Wait(Duration::from_millis(500)),
            ],
            grace: Duration::from_millis(500),
        },
        clock,
        live_corr: None,
        capture_raw: false,
    }));
    let ok_arrive = tokio::time::timeout(Duration::from_secs(360), arrived.notified())
        .await
        .is_ok();
    let stop_flag = Arc::new(AtomicBool::new(false));
    let poller = tokio::spawn(sample_series(
        url.to_owned(),
        Duration::from_millis(100),
        stop_flag.clone(),
    ));
    // 조작 프레임은 구간 전체에 10 Hz 로 흐른다.
    let ctl_stop = Arc::new(AtomicBool::new(false));
    let ctl_task = {
        let tx = tx.clone();
        let ctl_stop = ctl_stop.clone();
        tokio::spawn(async move {
            while !ctl_stop.load(Ordering::Relaxed) {
                let _ = tx.send(crate::conn::RemoteCmd::Control);
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        })
    };
    let mine_every = |secs: u64| {
        let tx = tx.clone();
        async move {
            let end = tokio::time::Instant::now() + Duration::from_secs(secs);
            while tokio::time::Instant::now() < end {
                let _ = tx.send(crate::conn::RemoteCmd::Mine);
                tokio::time::sleep(Duration::from_millis(3300)).await;
            }
        }
    };
    let mut marks = json!({});
    let mut compose_err: Option<String> = None;
    if ok_arrive {
        mine_every(8).await; // 정지 전 수락 ≥ 1
        marks["fingerprint_before"] = json!(db_fingerprint_retry(10).await);
        marks["stop_begin_ms"] = json!(now_ms());
        let stop_res = tokio::task::spawn_blocking(|| compose_postgres("stop"))
            .await
            .unwrap_or_else(|e| Err(e.to_string()));
        marks["stop_done_ms"] = json!(now_ms());
        if let Err(e) = stop_res {
            compose_err = Some(e);
        }
        // DB 가 **실제로 내려간 뒤** 곧바로 한 번 캔다 — 기록 지연이 20 을 넘기 전의 "정지 중 수락"
        // (SC-32 ⊘ 분모). 1차 실행은 stop 명령 **전**에 보내 정지 전 수락으로 섞였다(ms 반올림).
        let _ = tx.send(crate::conn::RemoteCmd::Mine);
        if compose_err.is_none() {
            tokio::time::sleep(Duration::from_millis(3300)).await;
            mine_every(22).await; // 정지 구간: RECORDING_BACKLOG 기대
        }
        marks["start_begin_ms"] = json!(now_ms());
        // 무엇이 실패했든 postgres 는 반드시 다시 올린다.
        let start_res = tokio::task::spawn_blocking(|| compose_postgres("start"))
            .await
            .unwrap_or_else(|e| Err(e.to_string()));
        marks["start_done_ms"] = json!(now_ms());
        if let Err(e) = start_res {
            compose_err = Some(compose_err.map_or(e.clone(), |p| format!("{p}; {e}")));
        }
        mine_every(45).await; // 재개 뒤: 백로그가 빠지고 다시 수락
        marks["fingerprint_after"] = json!(db_fingerprint_retry(60).await);
    }
    ctl_stop.store(true, Ordering::Relaxed);
    let _ = ctl_task.await;
    let _ = tx.send(crate::conn::RemoteCmd::Stop);
    let o = conn.await.ok();
    stop_flag.store(true, Ordering::Relaxed);
    let series = poller.await.unwrap_or_default();
    let Some(o) = o else {
        return (Verdict::Undecided, json!({"error": "연결 태스크 실패"}));
    };
    let m = &o.mining;
    let mines: Vec<(u64, String, Option<String>, Uuid)> = m
        .mine_sent
        .iter()
        .filter_map(|s| {
            m.results
                .iter()
                .find(|r| r.command_id == s.command_id)
                .map(|r| {
                    (
                        clock.wall_ms_at(s.at_us),
                        r.status.clone(),
                        r.reason_code.clone(),
                        s.command_id,
                    )
                })
        })
        .collect();
    let controls: Vec<(u64, bool)> = m
        .control_sent
        .iter()
        .map(|(id, at)| {
            let acc = m
                .results
                .iter()
                .any(|r| r.command_id == *id && r.status == "ACCEPTED");
            (clock.wall_ms_at(*at), acc)
        })
        .collect();
    let snaps: Vec<u64> = m
        .snapshot_at_us
        .iter()
        .map(|u| clock.wall_ms_at(*u))
        .collect();
    if let Some(p) = accepted_out {
        let ids: Vec<String> = mines
            .iter()
            .filter(|x| x.1 == "ACCEPTED")
            .map(|x| x.3.to_string())
            .collect();
        let _ = std::fs::write(p, ids.join("\n") + "\n");
    }
    judge_backlog(
        ok_arrive,
        &marks,
        compose_err,
        &mines,
        &controls,
        &snaps,
        &series,
    )
}

pub fn judge_backlog(
    arrived: bool,
    marks: &Value,
    compose_err: Option<String>,
    mines: &[(u64, String, Option<String>, Uuid)],
    controls: &[(u64, bool)],
    snaps: &[u64],
    series: &[StatSample],
) -> (Verdict, Value) {
    let stop_b = marks["stop_begin_ms"].as_u64().unwrap_or(0);
    let start_d = marks["start_done_ms"].as_u64().unwrap_or(0);
    let start_b = marks["start_begin_ms"].as_u64().unwrap_or(0);
    let lag_at = |t: u64| {
        series
            .iter()
            .min_by_key(|s| s.wall_ms.abs_diff(t))
            .map(|s| s.recording_lag)
    };
    // SC-29: 정지 전 수락 ≥ 1, RECORDING_BACKLOG ≥ 1, 거부 시점 lag > 20.
    let pre_acc = mines
        .iter()
        .filter(|m| m.0 < stop_b && m.1 == "ACCEPTED")
        .count();
    let backlog: Vec<(u64, Option<u64>)> = mines
        .iter()
        .filter(|m| m.2.as_deref() == Some("RECORDING_BACKLOG"))
        .map(|m| (m.0, lag_at(m.0)))
        .collect();
    let lag_ok = !backlog.is_empty() && backlog.iter().all(|b| b.1.is_some_and(|l| l > 20));
    // SC-30: 구간 [stop_b, start_b] 안 조작 수락 ≥ 1, 스냅샷 ≥ 1.
    let window_s = start_b.saturating_sub(stop_b) as f64 / 1000.0;
    let ctl_in = controls
        .iter()
        .filter(|c| c.0 >= stop_b && c.0 <= start_b && c.1)
        .count();
    let snap_in = snaps
        .iter()
        .filter(|t| **t >= stop_b && **t <= start_b)
        .count();
    // SC-31: 재개 뒤 lag ≤ 20 이 된 시점 이후 수락 ≥ 1.
    let recovered_at = series
        .iter()
        .find(|s| s.wall_ms > start_d && s.recording_lag <= 20)
        .map(|s| s.wall_ms);
    let post_acc = recovered_at.map_or(0, |r| {
        mines
            .iter()
            .filter(|m| m.0 >= r && m.1 == "ACCEPTED")
            .count()
    });
    // "정지 중" = stop 명령이 **끝난 뒤**(DB 가 실제로 내려감)부터 start 전까지.
    let stop_d = marks["stop_done_ms"].as_u64().unwrap_or(0);
    // 시각은 command_id(UUIDv7)의 ms 로 잰다 — 표지(marks)와 **같은 시계**(SystemTime)다. 단조 시계를
    // 벽시계로 환산한 값과 섞으면 같은 ms 경계에서 빠진다(2차 실행이 그렇게 0 으로 셌다: 그 채굴의 id ms
    // 가 stop_done 과 정확히 같았다). 경계는 포함(≥): stop 이 **돌아온 뒤** 보냈다.
    let id_ms = |u: &Uuid| {
        u.get_timestamp()
            .map(|t| {
                let (s, n) = t.to_unix();
                s * 1000 + u64::from(n) / 1_000_000
            })
            .unwrap_or(0)
    };
    let during_acc = mines
        .iter()
        .filter(|m| {
            let t = id_ms(&m.3);
            t >= stop_d && t <= start_b && m.1 == "ACCEPTED"
        })
        .count();
    let max_lag_window = series
        .iter()
        .filter(|s| s.wall_ms >= stop_b && s.wall_ms <= start_d)
        .map(|s| s.recording_lag)
        .max();
    let d = json!({"arrived": arrived, "marks": marks, "compose_error": compose_err,
        "window_seconds": window_s, "mines_total": mines.len(), "stats_samples": series.len(),
        "sc_29": {"pre_stop_accepted": pre_acc, "recording_backlog_rejects": backlog.len(),
                  "lag_at_rejects": backlog.iter().map(|b| b.1).collect::<Vec<_>>(), "all_lag_over_20": lag_ok,
                  "max_lag_in_window": max_lag_window},
        "sc_30": {"controls_accepted_in_window": ctl_in, "snapshots_in_window": snap_in},
        "sc_31": {"recovered_at_ms": recovered_at, "accepted_after_recovery": post_acc},
        "sc_32_input": {"accepted_total": mines.iter().filter(|m| m.1 == "ACCEPTED").count(),
                        "accepted_during_outage": during_acc},
        "mines": mines.iter().map(|m| json!([m.3.to_string(), id_ms(&m.3), m.1, m.2])).collect::<Vec<_>>()});
    // 증거 DB 지문 — 전후가 같아야 한다(값이 없으면 판정 불가, 다르면 FAIL).
    let fp_b = marks["fingerprint_before"]["Ok"].as_str();
    let fp_a = marks["fingerprint_after"]["Ok"].as_str();
    // ADR-0013 K2b 직접 기준(architect 2026-09-30, 관측 전 문장 그대로 — 계약 항목이 아니라 **별도
    // 기록**): "첫 대기 배치의 tick 부터 21 tick 안에 recording_lag > 20". ⊘: 정지 뒤 대기 배치가 실제로
    // 생겼다(oldest_pending_tick 이 null 이 아닌 값으로 전이). 표본 간격(100 ms ≈ 2 tick) 때문에 잰 값은
    // 참값보다 최대 한 간격 늦다 — 판정은 `잰 tick − P ≤ 21 + 직전 표본과의 tick 간격` 으로 하고 둘 다 찍는다.
    let after_stop: Vec<&StatSample> = series.iter().filter(|s| s.wall_ms >= stop_b).collect();
    let first_pending = after_stop.iter().find_map(|s| s.oldest_pending_tick);
    let onset_idx = after_stop.iter().position(|s| s.recording_lag > 20);
    let k2b = match (first_pending, onset_idx) {
        (Some(p), Some(i)) => {
            let s = after_stop[i];
            let gap = if i > 0 {
                s.tick.saturating_sub(after_stop[i - 1].tick)
            } else {
                0
            };
            let delta = s.tick.saturating_sub(p);
            json!({"first_pending_tick": p, "onset_sample_tick": s.tick, "onset_sample_lag": s.recording_lag,
                   "onset_sample_oldest_pending": s.oldest_pending_tick, "delta_ticks": delta,
                   "sampling_gap_ticks": gap, "holds": delta <= 21 + gap})
        }
        (None, _) => {
            json!({"why": "정지 뒤 oldest_pending_tick 이 한 번도 null 이 아니었다 — 대기 배치 미발생(⊘)", "holds": null})
        }
        (Some(p), None) => {
            json!({"first_pending_tick": p, "why": "lag > 20 표본 없음", "holds": false})
        }
    };
    // architect 선택 단언 둘(관측 전 등록, 2026-09-30) — 역시 별도 기록.
    // ① 정의 항등식: 대기 배치가 있는 표본은 lag == tick − oldest, 없는 표본은 lag 0.
    let with_p: Vec<&&StatSample> = after_stop
        .iter()
        .filter(|s| s.oldest_pending_tick.is_some())
        .collect();
    let identity_bad = after_stop
        .iter()
        .filter(|s| match s.oldest_pending_tick {
            Some(o) => s.recording_lag != s.tick.saturating_sub(o),
            None => s.recording_lag != 0,
        })
        .count();
    // ② 이른 교차 없음: tick − P ≤ 20 인 표본에서 lag ≤ 20.
    let early = first_pending.map(|p| {
        after_stop
            .iter()
            .filter(|s| s.tick.saturating_sub(p) <= 20 && s.tick >= p && s.recording_lag > 20)
            .count()
    });
    let k2b_extra = json!({
        "identity": {"samples_with_pending": with_p.len(), "samples_after_stop": after_stop.len(),
                     "violations": identity_bad,
                     "holds": if with_p.is_empty() { Value::Null } else { json!(identity_bad == 0) }},
        "no_early_crossing": {"violations": early, "holds": early.map(|n| n == 0)},
    });
    let mut d = d;
    d["k2b_optional_assertions"] = k2b_extra;
    d["evidence_db_fingerprint"] =
        json!({"before": fp_b, "after": fp_a, "same": fp_b.is_some() && fp_b == fp_a});
    d["k2b_direct_criterion"] = k2b;
    // ⊘: 도착, stop/start 성공, 구간 길이 > 0, 지문을 전후로 읽었다.
    if !arrived || compose_err.is_some() || window_s <= 0.0 || fp_b.is_none() || fp_a.is_none() {
        return (Verdict::Undecided, d);
    }
    if fp_b != fp_a {
        return (Verdict::Fail, d);
    }
    let v29 = pre_acc >= 1 && !backlog.is_empty() && lag_ok;
    let v30 = ctl_in >= 1 && snap_in >= 1;
    let v31 = recovered_at.is_some() && post_acc >= 1;
    d["items"] = json!({"sc_29": v29, "sc_30": v30, "sc_31": v31});
    (
        if v29 && v30 && v31 {
            Verdict::Pass
        } else {
            Verdict::Fail
        },
        d,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mining::{MiningObs, ObservedResult, SentMine};

    fn obs(n: usize, reason: Option<&str>) -> MiningObs {
        let mut m = MiningObs::default();
        for i in 0..n {
            let id = Uuid::now_v7();
            m.mine_sent.push(SentMine {
                command_id: id,
                deposit_id: "d".into(),
                at_us: 0,
                frame_seq_at_send: 0,
                last_snapshot_tick: None,
                dist_m: None,
                speed_mps: None,
            });
            let rej = i == 0 && reason.is_some();
            m.results.push(ObservedResult {
                frame_seq: i as u64,
                tick: i as u64,
                command_id: id,
                status: if rej { "REJECTED" } else { "ACCEPTED" }.into(),
                reason_code: if rej { reason.map(Into::into) } else { None },
            });
        }
        m
    }

    #[test]
    fn regen_formula_and_last_kg_controls() {
        let r = Regen {
            initial: 10_000,
            regen_kg: 150,
            interval_ticks: 600,
        };
        // 경계를 안 넘으면 그대로, 하나 넘으면 +150, 상한은 초기값.
        assert_eq!(effective(r, 100, 1210, 1790), 100);
        assert_eq!(effective(r, 100, 1210, 1800), 250);
        assert_eq!(effective(r, 9950, 0, 6000), 10_000);
        let shot = |status: &str, reason: Option<&str>, t: u64, inv: [i64; 2]| json!({"status": status, "reason": reason, "tick": t, "inventory_totals": inv});
        let shots = vec![
            shot("ACCEPTED", None, 1700, [0, 100]),
            shot("REJECTED", Some("RESOURCE_DEPLETED"), 1700, [0, 0]),
            shot("REJECTED", Some("RESOURCE_DEPLETED"), 1700, [0, 0]),
        ];
        let row = Some(vec![vec!["100".into(), "100".into(), "0".into()]]);
        let fired = Some((100, 1210, 1690));
        assert_eq!(
            judge_last_kg(true, fired, r, &shots, row.clone(), &[]).0,
            Verdict::Pass
        );
        // 음성: 둘째도 수락(복사) → FAIL.
        let mut two = shots.clone();
        two[1] = shot("ACCEPTED", None, 1700, [0, 100]);
        assert_eq!(
            judge_last_kg(true, fired, r, &two, row.clone(), &[]).0,
            Verdict::Fail
        );
        // ⊘: 다른 tick → 무효.
        let mut other = shots.clone();
        other[2] = shot("REJECTED", Some("RESOURCE_DEPLETED"), 1701, [0, 0]);
        assert_eq!(
            judge_last_kg(true, fired, r, &other, row.clone(), &[]).0,
            Verdict::Undecided
        );
        // ⊘: 처리 tick 이 회복 경계를 넘었다(잔량 250) → 무효.
        let late: Vec<Value> = shots
            .iter()
            .map(|s| {
                let mut s = s.clone();
                s["tick"] = json!(1800);
                s
            })
            .collect();
        assert_eq!(
            judge_last_kg(true, fired, r, &late, row, &[]).0,
            Verdict::Undecided
        );
    }

    #[test]
    fn backlog_judge_controls() {
        let marks = json!({"stop_begin_ms": 1000, "stop_done_ms": 2000, "start_begin_ms": 30000, "start_done_ms": 32000,
                           "fingerprint_before": {"Ok": "4805|abc|3"}, "fingerprint_after": {"Ok": "4805|abc|3"}});
        let s = |t: u64, lag: u64| StatSample {
            wall_ms: t,
            tick: t / 50,
            recording_lag: lag,
            persist_backlog: lag,
            oldest_pending_tick: None,
        };
        let series = vec![
            s(500, 5),
            s(5000, 80),
            s(20000, 400),
            s(33000, 300),
            s(40000, 10),
        ];
        let id = Uuid::now_v7;
        let mines = vec![
            (500, "ACCEPTED".to_owned(), None, id()),
            (
                20000,
                "REJECTED".to_owned(),
                Some("RECORDING_BACKLOG".to_owned()),
                id(),
            ),
            (41000, "ACCEPTED".to_owned(), None, id()),
        ];
        let controls = vec![(10000, true)];
        let snaps = vec![15000];
        assert_eq!(
            judge_backlog(true, &marks, None, &mines, &controls, &snaps, &series).0,
            Verdict::Pass
        );
        // 음성: 정지 구간에 조작이 거부됐다 → FAIL.
        assert_eq!(
            judge_backlog(
                true,
                &marks,
                None,
                &mines,
                &[(10000, false)],
                &snaps,
                &series
            )
            .0,
            Verdict::Fail
        );
        // 음성: RECORDING_BACKLOG 가 한 번도 안 났다 → FAIL.
        let no_rej = vec![mines[0].clone(), mines[2].clone()];
        assert_eq!(
            judge_backlog(true, &marks, None, &no_rej, &controls, &snaps, &series).0,
            Verdict::Fail
        );
        // 음성: 증거 DB 지문이 달라졌다 → FAIL.
        let mut changed = marks.clone();
        changed["fingerprint_after"] = json!({"Ok": "4804|zzz|3"});
        assert_eq!(
            judge_backlog(true, &changed, None, &mines, &controls, &snaps, &series).0,
            Verdict::Fail
        );
        // ⊘: stop/start 실패 → 미검증.
        assert_eq!(
            judge_backlog(
                true,
                &marks,
                Some("x".into()),
                &mines,
                &controls,
                &snaps,
                &series
            )
            .0,
            Verdict::Undecided
        );
    }

    #[test]
    fn race_judge_controls() {
        let r = |t: u64| Some(("ACCEPTED".to_owned(), None, t));
        let f = RaceFacts {
            seq_a: Some((3, "A".into())),
            seq_b: Some((1, "B".into())),
            records: Some(1),
            discoverer: Some("B".into()),
        };
        assert_eq!(
            judge_race("m", Some(0), &r(10), &r(10), &f).0,
            Verdict::Pass
        );
        // 음성: 발견자가 sequence 큰 쪽 → FAIL.
        let wrong = RaceFacts {
            discoverer: Some("A".into()),
            ..f.clone()
        };
        assert_eq!(
            judge_race("m", Some(0), &r(10), &r(10), &wrong).0,
            Verdict::Fail
        );
        // 음성: 기록 2 → FAIL.
        let two = RaceFacts {
            records: Some(2),
            ..f.clone()
        };
        assert_eq!(
            judge_race("m", Some(0), &r(10), &r(10), &two).0,
            Verdict::Fail
        );
        // ⊘: 다른 tick → 무효(미검증).
        assert_eq!(
            judge_race("m", Some(0), &r(10), &r(11), &f).0,
            Verdict::Undecided
        );
        // ⊘: 이미 발견된 광물 → 무효.
        assert_eq!(
            judge_race("m", Some(1), &r(10), &r(10), &f).0,
            Verdict::Undecided
        );
    }

    #[test]
    fn backlog_idle_controls() {
        let saw = StatsPoll {
            samples: 1000,
            max_persist_backlog: 20,
            ..StatsPoll::default()
        };
        assert_eq!(judge_backlog_idle(&obs(20, None), &saw).0, Verdict::Pass);
        // 음성: RECORDING_BACKLOG 1 건 → FAIL.
        assert_eq!(
            judge_backlog_idle(&obs(20, Some("RECORDING_BACKLOG")), &saw).0,
            Verdict::Fail
        );
        // ⊘: 톱니가 안 보였다(최대 19) → 미검증.
        let flat = StatsPoll {
            max_persist_backlog: 19,
            ..saw.clone()
        };
        assert_eq!(
            judge_backlog_idle(&obs(20, None), &flat).0,
            Verdict::Undecided
        );
        // ⊘: 적게 보냈다 → 미검증.
        assert_eq!(
            judge_backlog_idle(&obs(10, None), &saw).0,
            Verdict::Undecided
        );
    }
}

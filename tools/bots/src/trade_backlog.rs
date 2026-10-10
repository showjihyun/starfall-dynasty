//! p1-03 계약 SC-39 (AC-7) — 판매 중 postgres 를 멈췄다 재개한다(사람 단계 `db_stop`).
//!
//! 봇이 Ferrosite 2,000 kg 을 들고 도킹한 뒤 원격 조종으로: 정지 전 판매(수락 ≥ 1) → `docker compose stop
//! postgres` → 정지 중 판매(`recording_lag > 20` 뒤 `RECORDING_BACKLOG` ≥ 1)와 10 Hz 조작(수락) → `start` →
//! 회복 뒤 판매 수락. 수락된 판매 id 를 `<out>.accepted.txt` 에 쓴다 — 그 판매가 전부 DB 에 있는지는
//! `trade_ledger.py accepted` 가 판정한다(같은 SC-39 행의 둘째 도구).
//! **볼륨을 지우지 않는다** — `stop`/`start` 만(`compose_postgres` 가 다른 동사를 거부한다).

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use serde_json::{Value, json};
use tokio::sync::Notify;
use uuid::Uuid;

use crate::conn::{Behavior, BotSpec, Clock, FlyMode, MineStep, RemoteCmd, run_connection};
use crate::extra_cases::{StatSample, compose_postgres, now_ms, sample_series};
use crate::mine_cases::Verdict;
use crate::mine_run::DepositSite;
use crate::token;
use crate::trade::{Exchange, FERROSITE, mine_and_dock};

pub async fn run_sell_backlog(
    site: &DepositSite,
    ex: &Exchange,
    url: &str,
    secret: &str,
    label: &str,
    accepted_out: Option<&Path>,
) -> (Verdict, Value) {
    let clock = Clock::start();
    let arrived = Arc::new(Notify::new());
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    let mut steps = mine_and_dock(site, ex, FERROSITE, 2000);
    steps.push(MineStep::Signal(arrived.clone()));
    steps.push(MineStep::Remote(Arc::new(tokio::sync::Mutex::new(rx))));
    steps.push(MineStep::Wait(Duration::from_millis(500)));
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
            steps,
            grace: Duration::from_millis(500),
        },
        clock,
        live_corr: None,
        capture_raw: false,
    }));
    let ok_arrive = tokio::time::timeout(Duration::from_secs(600), arrived.notified())
        .await
        .is_ok();
    let stop_flag = Arc::new(AtomicBool::new(false));
    let poller = tokio::spawn(sample_series(
        url.to_owned(),
        Duration::from_millis(100),
        stop_flag.clone(),
    ));
    let ctl_stop = Arc::new(AtomicBool::new(false));
    let ctl_task = {
        let (tx, ctl_stop) = (tx.clone(), ctl_stop.clone());
        tokio::spawn(async move {
            while !ctl_stop.load(Ordering::Relaxed) {
                let _ = tx.send(RemoteCmd::Control);
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        })
    };
    let sell_every = |secs: u64| {
        let tx = tx.clone();
        async move {
            let end = tokio::time::Instant::now() + Duration::from_secs(secs);
            while tokio::time::Instant::now() < end {
                let _ = tx.send(RemoteCmd::SellFerrosite100);
                tokio::time::sleep(Duration::from_millis(1300)).await;
            }
        }
    };
    let mut marks = json!({});
    let mut compose_err: Option<String> = None;
    if ok_arrive {
        sell_every(4).await; // 정지 전 수락 ≥ 1
        marks["stop_begin_ms"] = json!(now_ms());
        let r = tokio::task::spawn_blocking(|| compose_postgres("stop"))
            .await
            .unwrap_or_else(|e| Err(e.to_string()));
        marks["stop_done_ms"] = json!(now_ms());
        if let Err(e) = r {
            compose_err = Some(e);
        } else {
            sell_every(20).await; // 정지 구간: RECORDING_BACKLOG 기대(lag > 20 = 1 s 뒤)
        }
        marks["start_begin_ms"] = json!(now_ms());
        // 무엇이 실패했든 postgres 는 반드시 다시 올린다.
        let r = tokio::task::spawn_blocking(|| compose_postgres("start"))
            .await
            .unwrap_or_else(|e| Err(e.to_string()));
        marks["start_done_ms"] = json!(now_ms());
        if let Err(e) = r {
            compose_err = Some(compose_err.map_or(e.clone(), |p| format!("{p}; {e}")));
        }
        sell_every(30).await; // 재개 뒤: 백로그가 빠지고 다시 수락
    }
    ctl_stop.store(true, Ordering::Relaxed);
    let _ = ctl_task.await;
    let _ = tx.send(RemoteCmd::Stop);
    let o = conn.await.ok();
    stop_flag.store(true, Ordering::Relaxed);
    let series = poller.await.unwrap_or_default();
    let Some(o) = o else {
        return (Verdict::Undecided, json!({"error": "연결 태스크 실패"}));
    };
    let m = &o.mining;
    let id_ms = |u: &Uuid| {
        u.get_timestamp()
            .map(|t| {
                let (s, n) = t.to_unix();
                s * 1000 + u64::from(n) / 1_000_000
            })
            .unwrap_or(0)
    };
    let sells: Vec<(u64, String, Option<String>, Uuid)> = m
        .sell_sent
        .iter()
        .filter_map(|s| {
            m.result_of(s.command_id).map(|r| {
                (
                    id_ms(&s.command_id),
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
        .map(|(id, _)| {
            (
                id_ms(id),
                m.results
                    .iter()
                    .any(|r| r.command_id == *id && r.status == "ACCEPTED"),
            )
        })
        .collect();
    if let Some(p) = accepted_out {
        let ids: Vec<String> = sells
            .iter()
            .filter(|x| x.1 == "ACCEPTED")
            .map(|x| x.3.to_string())
            .collect();
        let _ = std::fs::write(p, ids.join("\n") + "\n");
    }
    judge_sell_backlog(ok_arrive, &marks, compose_err, &sells, &controls, &series)
}

/// 순수 판정. ⊘: 정지 전 판매 수락 ≥ 1 **과** `RECORDING_BACKLOG` 거부 ≥ 1(정지 동안 판매를 실제로 보냈다).
pub fn judge_sell_backlog(
    arrived: bool,
    marks: &Value,
    compose_err: Option<String>,
    sells: &[(u64, String, Option<String>, Uuid)],
    controls: &[(u64, bool)],
    series: &[StatSample],
) -> (Verdict, Value) {
    let stop_b = marks["stop_begin_ms"].as_u64().unwrap_or(0);
    let start_b = marks["start_begin_ms"].as_u64().unwrap_or(0);
    let start_d = marks["start_done_ms"].as_u64().unwrap_or(0);
    let pre_acc = sells
        .iter()
        .filter(|s| s.0 < stop_b && s.1 == "ACCEPTED")
        .count();
    let backlog = sells
        .iter()
        .filter(|s| s.2.as_deref() == Some("RECORDING_BACKLOG"))
        .count();
    let ctl_in = controls
        .iter()
        .filter(|c| c.0 >= stop_b && c.0 <= start_b && c.1)
        .count();
    let recovered_at = series
        .iter()
        .find(|s| s.wall_ms > start_d && s.recording_lag <= 20)
        .map(|s| s.wall_ms);
    let post_acc = recovered_at.map_or(0, |r| {
        sells
            .iter()
            .filter(|s| s.0 >= r && s.1 == "ACCEPTED")
            .count()
    });
    let max_lag = series.iter().map(|s| s.recording_lag).max();
    let d = json!({"arrived": arrived, "marks": marks, "compose_error": compose_err,
        "sells_sent": sells.len(), "accepted_before_stop": pre_acc, "recording_backlog_rejects": backlog,
        "controls_accepted_during_stop": ctl_in, "recovered_at_ms": recovered_at,
        "accepted_after_recovery": post_acc, "max_recording_lag": max_lag, "samples": series.len()});
    if !arrived || compose_err.is_some() || pre_acc == 0 || backlog == 0 {
        return (Verdict::Undecided, d);
    }
    let pass = ctl_in >= 1 && post_acc >= 1;
    (if pass { Verdict::Pass } else { Verdict::Fail }, d)
}

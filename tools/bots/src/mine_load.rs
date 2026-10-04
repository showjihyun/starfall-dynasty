//! p1-02 `bots run --stage mine-load` — 31 연결이 이동 + 쿨다운마다 채굴 **10 분**(계약 SC-87·89·99·64).
//!
//! 봇마다 광맥 8 곳 중 하나(차례로)에 서서, 코디네이터가 봇마다 3.1 s 간격(위상 분산)으로 채굴을,
//! 100 ms 간격으로 제자리 조작(브레이크)을 보낸다. `/debug/stats` 를 측정 구간 시작·끝에 떠서 tick
//! 초과 비율을, 1 s 마다 폴링해 `ws_connections` 최소·`recording_lag` 최대를 잰다.
//! LIVE 지연(SC-64)은 봇이 받은 LIVE 마다 그 기록의 근거 `MINERAL_MINED.recorded_at`(SQL)과 수신
//! 벽시계를 뺀다 — 새 월드라 러너 첫 따라잡기 표본이 섞이지 않는다(러너 기동 뒤의 채굴만 있다).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

use serde_json::{Value, json};
use tokio::sync::Notify;

use crate::conn::{Behavior, BotSpec, Clock, FlyMode, MineStep, RemoteCmd, run_connection};
use crate::mine_cases::Verdict;
use crate::mining::pair_mine_responses;
use crate::restart_cases::fetch_stats;
use crate::token;

fn now_unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub const DEPOSITS: [&str; 8] = [
    "inner-belt-1",
    "inner-belt-2",
    "inner-belt-3",
    "vela-orbit-1",
    "vela-orbit-2",
    "outer-field-1",
    "outer-field-2",
    "far-reach",
];

#[derive(Debug, Clone)]
pub struct LoadCfg {
    pub bots: usize,
    pub duration: Duration,
    pub label_base: u32,
    pub world: String,
}

pub async fn run_mine_load(
    data_dir: &std::path::Path,
    url: &str,
    secret: &str,
    cfg: &LoadCfg,
) -> (Verdict, Value) {
    let clock = Clock::start();
    let mut conns = Vec::new();
    let mut txs = Vec::new();
    let mut arrivals = Vec::new();
    for i in 0..cfg.bots {
        let site = match crate::mine_run::find_deposit(data_dir, DEPOSITS[i % DEPOSITS.len()]) {
            Ok(s) => s,
            Err(e) => return (Verdict::Undecided, json!({"error": e})),
        };
        let arr = Arc::new(Notify::new());
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let label = format!("bot-{:03}", cfg.label_base + i as u32);
        let (_s, tok) = token::identity(secret, &label);
        conns.push(tokio::spawn(run_connection(BotSpec {
            label,
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
                    MineStep::Signal(arr.clone()),
                    MineStep::Remote(Arc::new(tokio::sync::Mutex::new(rx))),
                ],
                grace: Duration::from_millis(500),
            },
            clock,
            live_corr: None,
            capture_raw: false,
        })));
        txs.push(tx);
        arrivals.push(arr);
    }
    let arrived_all = tokio::time::timeout(Duration::from_secs(400), async {
        for a in &arrivals {
            a.notified().await;
        }
    })
    .await
    .is_ok();
    let stats0 = fetch_stats(url).await.ok();
    let window_start_ms = now_unix_ms();
    let stop = Arc::new(AtomicBool::new(false));
    let min_ws = Arc::new(AtomicU64::new(u64::MAX));
    let max_lag = Arc::new(AtomicU64::new(0));
    let poller = {
        let (stop, min_ws, max_lag, url) = (
            stop.clone(),
            min_ws.clone(),
            max_lag.clone(),
            url.to_owned(),
        );
        tokio::spawn(async move {
            while !stop.load(Ordering::Relaxed) {
                if let Ok(s) = fetch_stats(&url).await {
                    min_ws.fetch_min(s["ws_connections"].as_u64().unwrap_or(0), Ordering::Relaxed);
                    max_lag.fetch_max(s["recording_lag"].as_u64().unwrap_or(0), Ordering::Relaxed);
                }
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        })
    };
    if arrived_all {
        let mut tasks = Vec::new();
        for (i, tx) in txs.iter().enumerate() {
            let tx = tx.clone();
            let stop = stop.clone();
            let phase = Duration::from_millis((i as u64 * 97) % 3100);
            tasks.push(tokio::spawn(async move {
                tokio::time::sleep(phase).await;
                let mut next_mine = tokio::time::Instant::now();
                while !stop.load(Ordering::Relaxed) {
                    if tokio::time::Instant::now() >= next_mine {
                        let _ = tx.send(RemoteCmd::Mine);
                        next_mine += Duration::from_millis(3100);
                    }
                    let _ = tx.send(RemoteCmd::Control);
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            }));
        }
        tokio::time::sleep(cfg.duration).await;
        stop.store(true, Ordering::Relaxed);
        for t in tasks {
            let _ = t.await;
        }
    }
    stop.store(true, Ordering::Relaxed);
    let _ = poller.await;
    let stats1 = fetch_stats(url).await.ok();
    let window_end_ms = now_unix_ms();
    for tx in &txs {
        let _ = tx.send(RemoteCmd::Stop);
    }
    let mut outs = Vec::new();
    for c in conns {
        if let Ok(o) = c.await {
            outs.push(o);
        }
    }
    // 집계.
    let mut by: std::collections::BTreeMap<String, u64> = Default::default();
    let mut pair_bad = 0usize;
    let mut accepted_total = 0usize;
    let mut live: Vec<(u64, uuid::Uuid)> = Vec::new(); // (수신 벽시계 ms, 근거 이벤트 id)
    for o in &outs {
        let m = &o.mining;
        for s in &m.mine_sent {
            let k = match m.results.iter().find(|r| r.command_id == s.command_id) {
                Some(r) if r.status == "ACCEPTED" => "ACCEPTED".to_owned(),
                Some(r) => r.reason_code.clone().unwrap_or_else(|| "<null>".into()),
                None => "<no result>".into(),
            };
            *by.entry(k).or_default() += 1;
        }
        let ids: Vec<uuid::Uuid> = m.mine_sent.iter().map(|s| s.command_id).collect();
        let p = pair_mine_responses(m, &ids);
        accepted_total += p.accepted_mine;
        if !p.ok() {
            pair_bad += 1;
        }
        for n in &m.notices {
            if n.msg.payload.delivery == "LIVE"
                && let Some(src) = n.msg.payload.historical_event.source_event_ids.first()
            {
                live.push((clock.wall_ms_at(n.at_us), *src));
            }
        }
    }
    // SC-64: 근거 이벤트의 recorded_at(ms).
    let mut lat_ms: Vec<i64> = Vec::new();
    let mut lookup_err = 0usize;
    let mut cache: std::collections::BTreeMap<uuid::Uuid, Option<i64>> = Default::default();
    for (rx_ms, src) in &live {
        let rec = *cache.entry(*src).or_insert_with(|| {
            crate::sql::scalar_i64(&format!(
                "select (extract(epoch from recorded_at) * 1000)::bigint from domain_events where event_id = '{src}'"
            ))
            .ok()
        });
        match rec {
            Some(r) => lat_ms.push(*rx_ms as i64 - r),
            None => lookup_err += 1,
        }
    }
    lat_ms.sort_unstable();
    let pct = |p: f64| -> Option<i64> {
        if lat_ms.is_empty() {
            None
        } else {
            Some(lat_ms[((lat_ms.len() as f64 - 1.0) * p).round() as usize])
        }
    };
    let d_tick = |k: &str| -> Option<u64> {
        Some(
            stats1.as_ref()?[k]
                .as_u64()?
                .saturating_sub(stats0.as_ref()?[k].as_u64()?),
        )
    };
    let facts = LoadFacts {
        arrived_all,
        bots: cfg.bots,
        min_ws: min_ws.load(Ordering::Relaxed),
        accepted: accepted_total,
        ticks: d_tick("tick_total"),
        overruns: d_tick("tick_overrun_total"),
        backlog_rejects: by.get("RECORDING_BACKLOG").copied().unwrap_or(0),
        max_recording_lag: max_lag.load(Ordering::Relaxed),
        pair_bad,
        latency_samples: lat_ms.len(),
        distinct_records: cache.len(),
        latency_max_ms: lat_ms.last().copied(),
        duration_s: cfg.duration.as_secs(),
    };
    let (v, mut d) = judge_mine_load(&facts);
    d["results_by_reason"] = json!(by);
    d["latency_ms"] = json!({"p50": pct(0.5), "p95": pct(0.95), "max": lat_ms.last(), "lookup_errors": lookup_err});
    d["window_ms"] = json!([window_start_ms, window_end_ms]);
    d["world_id"] = json!(cfg.world);
    (v, d)
}

#[derive(Debug, Clone)]
pub struct LoadFacts {
    pub arrived_all: bool,
    pub bots: usize,
    pub min_ws: u64,
    pub accepted: usize,
    pub ticks: Option<u64>,
    pub overruns: Option<u64>,
    pub backlog_rejects: u64,
    pub max_recording_lag: u64,
    pub pair_bad: usize,
    pub latency_samples: usize,
    pub distinct_records: usize,
    pub latency_max_ms: Option<i64>,
    pub duration_s: u64,
}

/// 순수 판정. 항목별 결과는 `items` 에(라벨은 `contract_item` 하나).
pub fn judge_mine_load(f: &LoadFacts) -> (Verdict, Value) {
    // SC-87 ⊘: 31 연결 유지, 수락 채굴 ≥ 30 × (구간/3) × 0.5.
    let need_acc = (30.0 * (f.duration_s as f64 / 3.0) * 0.5) as usize;
    let load_ok =
        f.arrived_all && f.min_ws >= f.bots as u64 && f.bots >= 31 && f.accepted >= need_acc;
    let ratio = match (f.ticks, f.overruns) {
        (Some(t), Some(o)) if t > 0 => Some(o as f64 / t as f64),
        _ => None,
    };
    let v87 = if !load_ok || ratio.is_none() {
        Verdict::Undecided
    } else if ratio.is_some_and(|r| r <= 0.005) {
        Verdict::Pass
    } else {
        Verdict::Fail
    };
    // SC-89: 부하 아래 RECORDING_BACKLOG 0(검출기 생존은 같은 빌드의 SC-29 PASS 로 — 외부 증거).
    let v89 = if !load_ok {
        Verdict::Undecided
    } else if f.backlog_rejects == 0 {
        Verdict::Pass
    } else {
        Verdict::Fail
    };
    // SC-99: 봇마다 수락 수 == 짝 지은 INVENTORY_STATE 수(수락 0 이면 짝 실패로 센다).
    let v99 = if f.pair_bad == 0 && f.accepted > 0 {
        Verdict::Pass
    } else {
        Verdict::Fail
    };
    // SC-64: max ≤ 5 s, 기록 ≥ 4(광물 4 종).
    let v64 = if f.distinct_records < 4 || f.latency_samples == 0 {
        Verdict::Undecided
    } else if f.latency_max_ms.is_some_and(|m| m <= 5000) {
        Verdict::Pass
    } else {
        Verdict::Fail
    };
    let all = [v87, v89, v99, v64];
    let overall = if all.contains(&Verdict::Fail) {
        Verdict::Fail
    } else if all.contains(&Verdict::Undecided) {
        Verdict::Undecided
    } else {
        Verdict::Pass
    };
    (
        overall,
        json!({
            "items": {
                "sc_87": {"verdict": v87.as_str(), "tick_overrun_ratio": ratio, "ticks": f.ticks, "overruns": f.overruns,
                          "min_ws_connections": f.min_ws, "bots": f.bots, "accepted_mines": f.accepted, "needed_accepted": need_acc},
                "sc_89": {"verdict": v89.as_str(), "recording_backlog_rejects": f.backlog_rejects,
                          "max_recording_lag": f.max_recording_lag, "detector_alive_evidence": "같은 빌드의 backlog 케이스 PASS(RECORDING_BACKLOG 검출기 생존)를 리포트에서 인용"},
                "sc_99": {"verdict": v99.as_str(), "bots_with_unpaired_responses": f.pair_bad, "accepted_mines": f.accepted},
                "sc_64": {"verdict": v64.as_str(), "samples": f.latency_samples, "distinct_records": f.distinct_records,
                          "max_ms": f.latency_max_ms},
            }
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn good() -> LoadFacts {
        LoadFacts {
            arrived_all: true,
            bots: 31,
            min_ws: 31,
            accepted: 5000,
            ticks: Some(12000),
            overruns: Some(10),
            backlog_rejects: 0,
            max_recording_lag: 2,
            pair_bad: 0,
            latency_samples: 124,
            distinct_records: 4,
            latency_max_ms: Some(300),
            duration_s: 600,
        }
    }

    #[test]
    fn mine_load_controls() {
        assert_eq!(judge_mine_load(&good()).0, Verdict::Pass);
        // 음성: tick 초과 1 % → FAIL.
        let slow = LoadFacts {
            overruns: Some(120),
            ..good()
        };
        assert_eq!(judge_mine_load(&slow).0, Verdict::Fail);
        // 음성: RECORDING_BACKLOG 1 → FAIL.
        let rej = LoadFacts {
            backlog_rejects: 1,
            ..good()
        };
        assert_eq!(judge_mine_load(&rej).0, Verdict::Fail);
        // ⊘: 연결 30 으로 떨어짐 → 미검증(부하가 아니다).
        let thin = LoadFacts {
            min_ws: 30,
            ..good()
        };
        assert_eq!(judge_mine_load(&thin).0, Verdict::Undecided);
        // ⊘: 채굴이 적다 → 미검증.
        let idle = LoadFacts {
            accepted: 1000,
            ..good()
        };
        assert_eq!(judge_mine_load(&idle).0, Verdict::Undecided);
        // 음성: LIVE 지연 6 s → FAIL.
        let late = LoadFacts {
            latency_max_ms: Some(6000),
            ..good()
        };
        assert_eq!(judge_mine_load(&late).0, Verdict::Fail);
    }
}

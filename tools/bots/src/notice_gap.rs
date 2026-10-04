//! p1-02 `notice-gap` — **누락 틈 없음**(계약 SC-63, ADR-0014 §6): 역사 기록이 커밋·중계되는 순간과
//! 세션이 열리는 순간이 겹치게 만든 반복에서, LIVE 도 BACKFILL 도 못 받은 세션이 0 인가.
//!
//! 한 라운드 = 광물 하나: 채굴 봇이 새 광물의 광맥에 서 있고, 코디네이터가 짧게 사는 "여는 봇" 들을
//! 80 ms 간격으로 붙이다가 그 한가운데서 채굴을 발사한다. 여는 봇마다 그 발견을 LIVE 또는 BACKFILL
//! 로 **받았는지** 센다. 겹침이 실제로 일어났는지는 서버 카운터(`notice_open_overlap_ticks_total`)의
//! 실행 전후 델타로 본다 — 델타 0 이면 "틈이 없었다" 가 아니라 "겹침이 없었다" 이므로 **무효**.
//!
//! 잔류(30 s) 함선도 월드 정원을 차지하므로 라운드마다 여는 봇 수를 줄이고 라운드 사이를 35 s 띄운다.

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};
use tokio::sync::Notify;

use crate::conn::{Behavior, BotSpec, Clock, FlyMode, MineStep, RemoteCmd, run_connection};
use crate::mine_cases::Verdict;
use crate::mine_run::DepositSite;
use crate::restart_cases::fetch_stats;
use crate::token;

/// 라운드마다 새 광물 — 새 월드에서 네 번 발견이 난다.
pub const ROUNDS: [(&str, &str); 4] = [
    ("ferrosite", "inner-belt-1"),
    ("glacine", "vela-orbit-2"),
    ("cobaltine", "outer-field-1"),
    ("starfall-glass", "far-reach"),
];
const OPENERS: usize = 25;
const OPENER_GAP: Duration = Duration::from_millis(80);
/// 채굴 발사를 여는 봇 몇 번째 뒤에 둘지(창의 한가운데 앞쪽 — 중계는 커밋 뒤 몇 tick 늦다).
const FIRE_AFTER: usize = 8;

async fn overlap_total(url: &str) -> Option<u64> {
    fetch_stats(url).await.ok()?["notice_open_overlap_ticks_total"].as_u64()
}

/// 여는 봇 한 대의 관측: 그 광물의 발견을 (LIVE 수, BACKFILL 수) 받았는가.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpenerSeen {
    pub ready: bool,
    pub live: usize,
    pub backfill: usize,
}

async fn round(
    site: &DepositSite,
    mineral: &str,
    url: &str,
    secret: &str,
    miner_label: &str,
    opener_base: u32,
) -> (bool, Vec<OpenerSeen>) {
    let clock = Clock::start();
    let arrived = Arc::new(Notify::new());
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    let (_s, tok) = token::identity(secret, miner_label);
    let miner = tokio::spawn(run_connection(BotSpec {
        label: miner_label.to_owned(),
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
            ],
            grace: Duration::from_millis(300),
        },
        clock,
        live_corr: None,
        capture_raw: false,
    }));
    let ok = tokio::time::timeout(Duration::from_secs(360), arrived.notified())
        .await
        .is_ok();
    let mut openers = Vec::new();
    if ok {
        for k in 0..OPENERS {
            let label = format!("bot-{:03}", opener_base + k as u32);
            let (_s, tok) = token::identity(secret, &label);
            openers.push(tokio::spawn(run_connection(BotSpec {
                label,
                url: url.to_owned(),
                token: tok,
                behavior: Behavior::MineScript {
                    deposit_id: site.id.clone(),
                    target_m: site.position_m,
                    stop_radius_m: site.radius_m + 90.0,
                    fly: FlyMode::Stay,
                    // 기록이 커밋된 뒤에도 LIVE 가 올 시간을 준다.
                    steps: vec![MineStep::Wait(Duration::from_millis(3500))],
                    grace: Duration::from_millis(200),
                },
                clock,
                live_corr: None,
                capture_raw: false,
            })));
            if k == FIRE_AFTER {
                let _ = tx.send(RemoteCmd::Mine);
            }
            tokio::time::sleep(OPENER_GAP).await;
        }
    }
    let mut seen = Vec::new();
    for h in openers {
        if let Ok(o) = h.await {
            let m = &o.mining;
            let count = |d: &str| {
                m.notices
                    .iter()
                    .filter(|n| {
                        n.msg.payload.delivery == d
                            && n.msg.payload.historical_event.payload.mineral_id == mineral
                    })
                    .count()
            };
            seen.push(OpenerSeen {
                ready: m.session_ready_frame_seq.is_some(),
                live: count("LIVE"),
                backfill: count("BACKFILL"),
            });
        }
    }
    let _ = tx.send(RemoteCmd::Stop);
    let _ = miner.await;
    (ok, seen)
}

pub async fn run_notice_gap(
    data_dir: &std::path::Path,
    url: &str,
    secret: &str,
    label_base: u32,
) -> (Verdict, Value) {
    let before = overlap_total(url).await;
    let mut rounds = Vec::new();
    let mut all: Vec<OpenerSeen> = Vec::new();
    for (i, (mineral, dep)) in ROUNDS.iter().enumerate() {
        let site = match crate::mine_run::find_deposit(data_dir, dep) {
            Ok(s) => s,
            Err(e) => return (Verdict::Undecided, json!({"error": e})),
        };
        let base = label_base + (i as u32) * 40;
        let (ok, seen) = round(
            &site,
            mineral,
            url,
            secret,
            &format!("bot-{base:03}"),
            base + 1,
        )
        .await;
        rounds.push(
            json!({"mineral": mineral, "miner_arrived": ok, "openers": seen.len(),
            "ready": seen.iter().filter(|s| s.ready).count(),
            "got_live": seen.iter().filter(|s| s.live > 0).count(),
            "got_backfill": seen.iter().filter(|s| s.backfill > 0).count(),
            "got_neither": seen.iter().filter(|s| s.ready && s.live + s.backfill == 0).count(),
            "got_more_than_once": seen.iter().filter(|s| s.live + s.backfill > 1).count()}),
        );
        all.extend(seen);
        // 잔류 함선(30 s)이 빠질 때까지.
        if i + 1 < ROUNDS.len() {
            tokio::time::sleep(Duration::from_secs(35)).await;
        }
    }
    let after = overlap_total(url).await;
    judge_notice_gap(before, after, &all, rounds)
}

pub fn judge_notice_gap(
    before: Option<u64>,
    after: Option<u64>,
    all: &[OpenerSeen],
    rounds: Vec<Value>,
) -> (Verdict, Value) {
    let delta = match (before, after) {
        (Some(b), Some(a)) => Some(a.saturating_sub(b)),
        _ => None,
    };
    let ready: Vec<&OpenerSeen> = all.iter().filter(|s| s.ready).collect();
    let neither = ready.iter().filter(|s| s.live + s.backfill == 0).count();
    let live_n = ready.iter().filter(|s| s.live > 0).count();
    let back_n = ready.iter().filter(|s| s.backfill > 0).count();
    let d = json!({"overlap_ticks_delta": delta, "sessions_ready": ready.len(), "got_neither": neither,
        "got_live": live_n, "got_backfill": back_n,
        "got_more_than_once": ready.iter().filter(|s| s.live + s.backfill > 1).count(),
        "rounds": rounds});
    // ⊘: 겹침이 실제로 일어났다(델타 ≥ 1), 창이 기록 시점을 걸쳤다(LIVE 받은 세션·BACKFILL 받은 세션 둘 다 ≥ 1).
    if delta.is_none_or(|x| x == 0) || live_n == 0 || back_n == 0 {
        return (Verdict::Undecided, d);
    }
    (
        if neither == 0 {
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

    fn s(live: usize, backfill: usize) -> OpenerSeen {
        OpenerSeen {
            ready: true,
            live,
            backfill,
        }
    }

    #[test]
    fn notice_gap_controls() {
        let all = vec![s(1, 0), s(0, 1), s(1, 0)];
        assert_eq!(
            judge_notice_gap(Some(3), Some(5), &all, vec![]).0,
            Verdict::Pass
        );
        // 음성: 둘 다 못 받은 세션 1 → FAIL.
        let gap = vec![s(1, 0), s(0, 1), s(0, 0)];
        assert_eq!(
            judge_notice_gap(Some(3), Some(5), &gap, vec![]).0,
            Verdict::Fail
        );
        // ⊘: 겹침 델타 0 → 무효.
        assert_eq!(
            judge_notice_gap(Some(3), Some(3), &all, vec![]).0,
            Verdict::Undecided
        );
        // ⊘: 창이 기록 시점을 안 걸침(전부 BACKFILL) → 무효.
        let late = vec![s(0, 1), s(0, 1)];
        assert_eq!(
            judge_notice_gap(Some(3), Some(5), &late, vec![]).0,
            Verdict::Undecided
        );
    }
}

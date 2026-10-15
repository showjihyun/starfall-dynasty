//! Real-socket load test: `cargo run -p bot --release --example swarm -- [url] [captains] [battle-secs]`.
//! Every captain logs in over its own WebSocket, warps three jumps into the Vesta Front war zone
//! and fights whoever is nearest. With a `--dev` server a third grow into cruisers and a third
//! into capital ships (`/grow`). Prints per-captain snapshot rate and bandwidth at the end.

use bot::Bot;
use shared::galaxy::Species;
use shared::glam::Vec3;
use shared::protocol::*;
use shared::ships::HullClass;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use std::time::{Duration, Instant};

#[derive(Default)]
struct Live {
    connected: AtomicU64,
    in_zone: AtomicU64,
    failed: AtomicU64,
    deaths: AtomicU64,
}

struct Report {
    snaps_per_sec: f64,
    kb_per_sec: f64,
    max_seen: usize,
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let url = args.get(1).cloned().unwrap_or_else(|| "ws://127.0.0.1:7777".into());
    let n: usize = args.get(2).and_then(|v| v.parse().ok()).unwrap_or(1000);
    let battle = Duration::from_secs(args.get(3).and_then(|v| v.parse().ok()).unwrap_or(60));
    let live = Arc::new(Live::default());
    let start = Instant::now();
    // Everyone fights until the same wall-clock moment, however long their arrival took.
    let ramp = Duration::from_millis(8);
    let fight_until = start + ramp * n as u32 + Duration::from_secs(40) + battle;

    let mut tasks = vec![];
    for i in 0..n {
        let (url, live) = (url.clone(), live.clone());
        tasks.push(tokio::spawn(async move {
            tokio::time::sleep(ramp * i as u32).await;
            match captain(i, &url, &live, fight_until).await {
                Ok(r) => Some(r),
                Err(e) => {
                    live.failed.fetch_add(1, Relaxed);
                    eprintln!("captain {i}: {e}");
                    None
                }
            }
        }));
    }
    let monitor = {
        let live = live.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(5)).await;
                println!(
                    "[{:>4.0}s] connected {} | in war zone {} | failed {} | deaths {}",
                    start.elapsed().as_secs_f32(),
                    live.connected.load(Relaxed),
                    live.in_zone.load(Relaxed),
                    live.failed.load(Relaxed),
                    live.deaths.load(Relaxed)
                );
            }
        })
    };
    let mut reports = vec![];
    for t in tasks {
        if let Ok(Some(r)) = t.await {
            reports.push(r);
        }
    }
    monitor.abort();
    let k = reports.len().max(1) as f64;
    let mut rates: Vec<f64> = reports.iter().map(|r| r.snaps_per_sec).collect();
    rates.sort_by(|a, b| a.total_cmp(b));
    println!("== swarm: {n} captains, {} fought to the end, {} failed, {} deaths", reports.len(), live.failed.load(Relaxed), live.deaths.load(Relaxed));
    if !rates.is_empty() {
        println!(
            "snapshots/s per captain: avg {:.1}, worst {:.1} (server sends 20) | inbound {:.1} KB/s per captain | most ships seen at once {}",
            rates.iter().sum::<f64>() / k,
            rates[0],
            reports.iter().map(|r| r.kb_per_sec).sum::<f64>() / k,
            reports.iter().map(|r| r.max_seen).max().unwrap_or(0)
        );
    }
}

async fn captain(i: usize, url: &str, live: &Live, fight_until: Instant) -> Result<Report, String> {
    let mut bot = Bot::connect(url, &format!("Swarm{i:04}")).await?;
    bot.login_and_create(HullClass::ALL[i % 5], Species::ALL[i % 3], Profession::CombatPilot).await?;
    live.connected.fetch_add(1, Relaxed);
    if i % 3 != 0 {
        bot.send(ClientMsg::Chat { text: format!("/grow {}", if i % 3 == 1 { 5 } else { 9 }) }).await;
        bot.pump(0.5).await;
    }
    // Sol Haven → Kharz Forge → Vesta Corridor → Vesta Front (war zone).
    for sys in [2, 4, 7] {
        if bot.system != sys {
            bot.warp_to(sys).await?;
        }
    }
    live.in_zone.fetch_add(1, Relaxed);
    let (t0, b0, s0) = (Instant::now(), bot.rx_bytes, bot.snapshots);
    let mut max_seen = 0;
    let mut k = 0u32;
    while Instant::now() < fight_until {
        k += 1;
        if bot.dead {
            live.deaths.fetch_add(1, Relaxed);
            // Rebuilt at home: fly back to the front.
            bot.send(ClientMsg::Respawn).await;
            bot.until(5.0, |m| matches!(m, ServerMsg::EnterSystem { .. })).await;
            for sys in [2, 4, 7] {
                if Instant::now() >= fight_until || (bot.system != sys && bot.warp_to(sys).await.is_err()) {
                    break;
                }
            }
            continue;
        }
        max_seen = max_seen.max(bot.ents.len());
        let me = bot.my_state().cloned();
        let enemy = me.as_ref().and_then(|m| {
            let p = Vec3::from(m.pos);
            bot.ents.values().filter(|e| e.owner != m.owner && matches!(e.kind, EntityKind::Player | EntityKind::Npc(_)) && !e.docked).min_by(|a, b| Vec3::from(a.pos).distance(p).total_cmp(&Vec3::from(b.pos).distance(p))).map(|e| (e.id, Vec3::from(e.pos)))
        });
        let mut c = match enemy {
            Some((_, pos)) => bot.aim_controls(pos, 1.0, 350.0),
            None => Controls { thrust: 0.5, yaw: ((k + i as u32) as f32 * 0.1).sin(), ..Default::default() },
        };
        c.fire = enemy.is_some();
        if k % 20 == 1 {
            bot.send(ClientMsg::SetTarget(enemy.map(|e| e.0))).await;
        }
        bot.send(ClientMsg::Controls(c)).await;
        bot.pump(0.1).await;
    }
    let secs = t0.elapsed().as_secs_f64().max(1.0);
    Ok(Report { snaps_per_sec: (bot.snapshots - s0) as f64 / secs, kb_per_sec: (bot.rx_bytes - b0) as f64 / secs / 1e3, max_seen })
}

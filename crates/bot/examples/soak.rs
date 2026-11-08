//! Live-server soak test: the real server binary with captains logging in and out for minutes to a
//! day of wall clock — does it stay up, stay fast and stop growing? Every 30 s: server memory (RSS),
//! the server's own tick cost (`[stats]` lines), saved data size, connected captains, reconnect
//! failures. At the end the server is stopped and started again on the same data, and every captain
//! must log back in with its token and find its captain (persistence).
//!
//!   cargo build --release -p server
//!   cargo run -p bot --release --example soak -- [--minutes 30] [--captains 40] [--session 4] [--port 7795] [--log FILE]
//!   … --server-exe COPY  run a copy of the server (long runs: rebuilding target/release stays possible)
//!   (`--log`: the server runs with `--prof` and every line it prints, span profiles included, goes to FILE)
//!
//! Captains hunt pirates in the Frontier ring (Orion Drift, Vesta Corridor, Kharz Forge), warp on every
//! few minutes, rebuild when sunk, and disconnect after `--session` minutes (staggered) to come back
//! a few seconds later with their token. Exit code 1 on a failed check. Results: docs/QA_SOAK.md.

use bot::Bot;
use shared::galaxy::Species;
use shared::glam::Vec3;
use shared::protocol::{ClientMsg, EntityKind, Profession, ServerMsg};
use shared::ships::HullClass;
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Default)]
struct Live {
    connected: AtomicUsize,
    sessions: AtomicUsize,
    reconnect_failed: AtomicUsize,
    deaths: AtomicUsize,
}

/// One 30 s sample.
#[derive(Clone, Debug)]
struct Sample {
    t_min: f64,
    rss_mb: f64,
    data_kb: f64,
    connected: usize,
    tick_avg: f64,
    tick_max: f64,
}

/// `--server-exe PATH` (a copy, so `cargo build -p server` keeps working during a day-long run —
/// Windows locks a running binary), else target/release.
fn server_exe() -> std::path::PathBuf {
    let args: Vec<String> = std::env::args().collect();
    if let Some(p) = args.iter().position(|a| a == "--server-exe").and_then(|i| args.get(i + 1)) {
        return p.into();
    }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    root.join("target/release").join(if cfg!(windows) { "server.exe" } else { "server" })
}

/// Start the server; its `[stats]` lines feed `stats` (tick avg, max ms).
/// `log`: also write every server line there (with `--prof` the server adds its span profile).
fn start_server(port: u16, data: &std::path::Path, stats: Arc<Mutex<Vec<(f64, f64)>>>, log: Option<std::path::PathBuf>) -> Child {
    let mut args = vec!["--port".to_string(), port.to_string(), "--data".into(), data.to_string_lossy().into_owned()];
    if log.is_some() {
        args.push("--prof".into());
    }
    let mut child = Command::new(server_exe()).args(&args).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().expect("server binary (cargo build --release -p server)");
    let out = child.stdout.take().unwrap();
    std::thread::spawn(move || {
        let mut file = log.and_then(|p| std::fs::File::create(p).ok());
        for line in BufReader::new(out).lines().map_while(Result::ok) {
            if let Some(f) = file.as_mut() {
                use std::io::Write;
                let _ = writeln!(f, "{line}");
            }
            // "[stats] 40 conns, 812 entities | tick+encode avg 1.23 ms, max 4.56 ms (budget 50 ms) | …"
            if let Some(rest) = line.strip_prefix("[stats]")
                && let (Some(a), Some(m)) = (rest.split("avg ").nth(1), rest.split("max ").nth(1))
            {
                let num = |s: &str| s.split_whitespace().next().and_then(|v| v.parse::<f64>().ok());
                if let (Some(a), Some(m)) = (num(a), num(m)) {
                    stats.lock().unwrap().push((a, m));
                }
            }
        }
    });
    child
}

/// Resident memory of a process in MB (Windows: tasklist; Linux: /proc).
fn rss_mb(pid: u32) -> f64 {
    if cfg!(windows) {
        let out = Command::new("tasklist").args(["/FI", &format!("PID eq {pid}"), "/FO", "CSV", "/NH"]).output().map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default();
        // "server.exe","1234","Console","1","123,456 K"
        out.rsplit("\",\"").next().map(|k| k.chars().filter(|c| c.is_ascii_digit()).collect::<String>()).and_then(|k| k.parse::<f64>().ok()).map_or(0.0, |kb| kb / 1024.0)
    } else {
        std::fs::read_to_string(format!("/proc/{pid}/status")).ok().and_then(|s| s.lines().find(|l| l.starts_with("VmRSS:")).and_then(|l| l.split_whitespace().nth(1)?.parse::<f64>().ok())).map_or(0.0, |kb| kb / 1024.0)
    }
}

fn dir_kb(dir: &std::path::Path) -> f64 {
    std::fs::read_dir(dir).map(|rd| rd.filter_map(Result::ok).map(|e| if e.path().is_dir() { dir_kb(&e.path()) } else { e.metadata().map_or(0.0, |m| m.len() as f64 / 1024.0) }).sum()).unwrap_or(0.0)
}

/// Least-squares slope of y over x.
fn slope(points: &[(f64, f64)]) -> f64 {
    let n = points.len() as f64;
    if n < 2.0 {
        return 0.0;
    }
    let (mx, my) = (points.iter().map(|p| p.0).sum::<f64>() / n, points.iter().map(|p| p.1).sum::<f64>() / n);
    let (num, den) = points.iter().fold((0.0, 0.0), |(a, b), p| (a + (p.0 - mx) * (p.1 - my), b + (p.0 - mx).powi(2)));
    if den == 0.0 { 0.0 } else { num / den }
}

const RING: [u16; 3] = [3, 4, 2];

/// One captain for the whole run: play a session, drop the connection, come back with the token.
async fn captain(i: usize, url: String, live: Arc<Live>, until: Instant, session: Duration) -> Option<String> {
    let name = format!("Soak{i:03}");
    let mut token: Option<String> = None;
    let mut first = true;
    while Instant::now() < until {
        let mut bot = match Bot::connect(&url, &name).await {
            Ok(b) => b,
            Err(_) => {
                live.reconnect_failed.fetch_add(1, Relaxed);
                tokio::time::sleep(Duration::from_secs(3)).await;
                continue;
            }
        };
        bot.token = token.clone();
        if bot.login_and_create(HullClass::ALL[i % HullClass::ALL.len()], Species::ALL[i % Species::ALL.len()], Profession::CombatPilot).await.is_err() || (!first && bot.profile.is_none()) {
            live.reconnect_failed.fetch_add(1, Relaxed);
            tokio::time::sleep(Duration::from_secs(3)).await;
            continue;
        }
        first = false;
        token = bot.token.clone();
        live.connected.fetch_add(1, Relaxed);
        live.sessions.fetch_add(1, Relaxed);
        // Staggered sessions so the server always has some captains arriving and leaving.
        let end = (Instant::now() + session + Duration::from_secs((i as u64 * 7) % 60)).min(until);
        let mut hop = Instant::now();
        let mut k = i;
        while Instant::now() < end {
            bot.log.clear();
            if bot.dead {
                live.deaths.fetch_add(1, Relaxed);
                bot.send(ClientMsg::Respawn).await;
                bot.until(5.0, |m| matches!(m, ServerMsg::EnterSystem { .. })).await;
                continue;
            }
            if hop.elapsed() > Duration::from_secs(150) || !RING.contains(&bot.system) {
                k += 1;
                let _ = bot.warp_to(RING[k % RING.len()]).await;
                hop = Instant::now();
                continue;
            }
            // Nearest pirate: turn to it and fire; none: cruise toward the station's belt.
            let me = bot.my_state().map(|m| Vec3::from(m.pos));
            let foe = me.and_then(|p| bot.ents.values().filter(|e| matches!(e.kind, EntityKind::Npc(_))).map(|e| (e.id, Vec3::from(e.pos))).min_by(|a, b| a.1.distance(p).total_cmp(&b.1.distance(p))));
            let ctl = match (me, foe) {
                (Some(p), Some((id, at))) => {
                    bot.send(ClientMsg::SetTarget(Some(id))).await;
                    let mut c = bot.aim_controls(at, 1.0, 500.0);
                    c.fire = at.distance(p) < 900.0;
                    c
                }
                (Some(_), None) => bot.aim_controls(Vec3::new(0.0, 200.0, 3000.0), 0.6, 300.0),
                _ => Default::default(),
            };
            bot.send(ClientMsg::Undock).await;
            bot.send(ClientMsg::Controls(ctl)).await;
            bot.pump(0.5).await;
        }
        live.connected.fetch_sub(1, Relaxed);
        drop(bot);
        tokio::time::sleep(Duration::from_secs(2 + (i as u64 % 5))).await;
    }
    token
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let arg = |k: &str| args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).and_then(|v| v.parse::<f64>().ok());
    let minutes = arg("--minutes").unwrap_or(30.0);
    let n = arg("--captains").unwrap_or(40.0) as usize;
    let session = Duration::from_secs_f64(arg("--session").unwrap_or(4.0) * 60.0);
    let port = arg("--port").unwrap_or(7795.0) as u16;
    let data = std::env::temp_dir().join(format!("starfall_soak_{port}"));
    let _ = std::fs::remove_dir_all(&data);
    std::fs::create_dir_all(&data).unwrap();
    let url = format!("ws://127.0.0.1:{port}");
    let stats = Arc::new(Mutex::new(vec![]));
    let log = args.iter().position(|a| a == "--log").and_then(|i| args.get(i + 1)).map(std::path::PathBuf::from);
    let mut server = start_server(port, &data, stats.clone(), log);
    tokio::time::sleep(Duration::from_secs(2)).await;
    let start = Instant::now();
    let until = start + Duration::from_secs_f64(minutes * 60.0);
    let live = Arc::new(Live::default());
    let mut tasks = vec![];
    for i in 0..n {
        let (url, live) = (url.clone(), live.clone());
        tasks.push(tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(150 * i as u64)).await;
            (i, captain(i, url, live, until, session).await)
        }));
    }
    let mut samples: Vec<Sample> = vec![];
    while Instant::now() < until {
        tokio::time::sleep(Duration::from_secs(30).min(until.saturating_duration_since(Instant::now()).max(Duration::from_millis(1)))).await;
        let window: Vec<(f64, f64)> = std::mem::take(&mut *stats.lock().unwrap());
        let (avg, max) = if window.is_empty() { (0.0, 0.0) } else { (window.iter().map(|w| w.0).sum::<f64>() / window.len() as f64, window.iter().map(|w| w.1).fold(0.0, f64::max)) };
        let s = Sample { t_min: start.elapsed().as_secs_f64() / 60.0, rss_mb: rss_mb(server.id()), data_kb: dir_kb(&data), connected: live.connected.load(Relaxed), tick_avg: avg, tick_max: max };
        println!(
            "[{:>6.1} min] rss {:>7.1} MB | data {:>7.0} KB | connected {:>3} | tick avg {:>5.2} ms max {:>6.2} ms | sessions {} | reconnect failures {} | deaths {}",
            s.t_min, s.rss_mb, s.data_kb, s.connected, s.tick_avg, s.tick_max, live.sessions.load(Relaxed), live.reconnect_failed.load(Relaxed), live.deaths.load(Relaxed)
        );
        if server.try_wait().ok().flatten().is_some() {
            println!("SERVER EXITED");
            break;
        }
        samples.push(s);
    }
    let mut tokens = vec![];
    for t in tasks {
        if let Ok((i, Some(tok))) = t.await {
            tokens.push((i, tok));
        }
    }
    let alive = server.try_wait().ok().flatten().is_none();
    let _ = server.kill();
    let _ = server.wait();
    // Restart on the same data: every captain comes back with its token and finds its captain.
    // (Killed, not shut down: what survives is what the 15 s autosave wrote — a crash, not a clean stop.)
    let mut server = start_server(port, &data, Arc::new(Mutex::new(vec![])), None);
    tokio::time::sleep(Duration::from_secs(2)).await;
    let mut back = 0;
    // The restarted server may take a moment to load the data and listen.
    let listening = Instant::now() + Duration::from_secs(20);
    while Instant::now() < listening && Bot::connect(&url, "Probe").await.is_err() {
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    let mut why = None;
    for (i, tok) in &tokens {
        let conn = Bot::connect(&url, &format!("Soak{i:03}")).await;
        if let Err(e) = &conn {
            why.get_or_insert_with(|| format!("connect: {e}"));
        }
        if let Ok(mut b) = conn {
            b.token = Some(tok.clone());
            match b.login_and_create(HullClass::Talon, Species::Human, Profession::CombatPilot).await {
                Ok(()) if b.profile.as_ref().is_some_and(|p| p.player_id > 0) && b.log.iter().all(|m| !matches!(m, ServerMsg::Error { .. })) => back += 1,
                Ok(()) => {
                    why.get_or_insert_with(|| format!("login answered with {:?}", b.log.iter().find(|m| matches!(m, ServerMsg::Error { .. }))));
                }
                Err(e) => {
                    why.get_or_insert(e);
                }
            }
        }
    }
    let _ = server.kill();

    // Verdict. Memory: slope over the run after the first fifth (warm-up), in MB per hour.
    let warm: Vec<&Sample> = samples.iter().filter(|s| s.t_min >= minutes / 5.0).collect();
    let rss_slope = slope(&warm.iter().map(|s| (s.t_min / 60.0, s.rss_mb)).collect::<Vec<_>>());
    let data_slope = slope(&warm.iter().map(|s| (s.t_min / 60.0, s.data_kb)).collect::<Vec<_>>());
    let quarter = |first: bool| {
        let q: Vec<&&Sample> = if first { warm.iter().take(warm.len().div_ceil(4)).collect() } else { warm.iter().rev().take(warm.len().div_ceil(4)).collect() };
        q.iter().map(|s| s.tick_avg).sum::<f64>() / q.len().max(1) as f64
    };
    let (tick_first, tick_last) = (quarter(true), quarter(false));
    let peak_tick = samples.iter().map(|s| s.tick_max).fold(0.0, f64::max);
    let fails = live.reconnect_failed.load(Relaxed);
    println!("== soak: {n} captains, {minutes:.0} min, {} sessions, {} deaths", live.sessions.load(Relaxed), live.deaths.load(Relaxed));
    println!("rss {:.1} → {:.1} MB (peak {:.1}), slope {:+.1} MB/h after warm-up | data {:.0} → {:.0} KB, slope {:+.0} KB/h", samples.first().map_or(0.0, |s| s.rss_mb), samples.last().map_or(0.0, |s| s.rss_mb), samples.iter().map(|s| s.rss_mb).fold(0.0, f64::max), rss_slope, samples.first().map_or(0.0, |s| s.data_kb), samples.last().map_or(0.0, |s| s.data_kb), data_slope);
    println!("tick avg first quarter {tick_first:.2} ms → last quarter {tick_last:.2} ms, worst tick {peak_tick:.1} ms (budget 50)");
    let checks = [
        ("server stayed up", alive),
        ("no failed reconnects", fails == 0),
        ("every captain back after a restart", back == tokens.len() && !tokens.is_empty()),
        // A day at this slope stays under +256 MB.
        ("memory not climbing (< 10.7 MB/h)", rss_slope < 10.7),
        ("tick cost not climbing (last quarter ≤ first × 1.5 + 0.5 ms)", tick_last <= tick_first * 1.5 + 0.5),
        ("no tick over budget", peak_tick < 50.0),
    ];
    let mut failed = 0;
    for (what, ok) in checks {
        println!("{} {what}", if ok { "PASS" } else { "FAIL" });
        failed += (!ok) as usize;
    }
    println!("restart: {back}/{} captains back{}; reconnect failures {fails}", tokens.len(), why.map_or(String::new(), |w| format!(" (first problem: {w})")));
    let _ = std::fs::remove_dir_all(&data);
    std::process::exit(if failed > 0 { 1 } else { 0 });
}

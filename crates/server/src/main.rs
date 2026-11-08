//! STARFALL DYNASTY authoritative server.
//!
//! `server [--port 7777] [--data ./data] [--dev] [--record DIR] [--metrics FILE.jsonl] [--prof]`
//!
//! WebSocket transport, JSON frames. One game loop task owns all state at 20 Hz;
//! connection tasks only shuttle frames.

use futures_util::{SinkExt, StreamExt};
use sim::game::{ConnId, Game};
use sim::store::Store;
use rayon::prelude::*;
use shared::protocol::{ClientMsg, DEFAULT_PORT, ServerMsg, TICK_HZ, decode, encode, encode_snapshot};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;

/// Protecting a public server from one bad client (not game rules). A client frame larger than this
/// closes the connection (the biggest real message, a chat line or a research thesis, is a few KB).
const MAX_FRAME: usize = 64 * 1024;
/// Messages a connection may send per second, and the burst: mouse steering sends Controls every frame,
/// so 144 fps fits. Beyond it messages are dropped; `FLOOD_KICK` dropped in a row closes it.
const MSG_RATE: f64 = 240.0;
const MSG_BURST: f64 = 480.0;
const FLOOD_KICK: u32 = 2000;
/// Frames queued for a client that stopped reading (20 s of snapshots): it is disconnected instead of
/// the server holding its backlog in memory.
const SEND_BACKLOG: usize = 400;

/// A connection's outbound channel and how many frames wait in it.
struct Out {
    tx: mpsc::UnboundedSender<Message>,
    queued: std::sync::Arc<std::sync::atomic::AtomicUsize>,
}

enum Inbound {
    Open(ConnId, Out),
    Msg(ConnId, ClientMsg),
    Close(ConnId),
}

fn unix_now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let arg = |k: &str| args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).cloned();
    let port: u16 = arg("--port").and_then(|p| p.parse().ok()).unwrap_or(DEFAULT_PORT);
    let data = PathBuf::from(arg("--data").unwrap_or_else(|| "data".into()));

    let store = Store::open(data.join("profiles.json"))?;
    let history = history::HistoryEngine::open(&data.join("history.jsonl"))?;
    println!("[starfall] data dir {} — {} accounts, {} historical events", data.display(), store.data.accounts.len(), history.events.len());
    match shared::tuning::Tuning::locate() {
        Some(d) => println!("[starfall] tuning {}", d.display()),
        None => println!("[starfall] no tuning/ directory found (set {})", shared::tuning::DIR_ENV),
    }
    let start_unix = unix_now();
    let mut game = Game::new(store, history, start_unix);
    game.dev_mode = args.iter().any(|a| a == "--dev");
    // `--prof` (or STARFALL_PROF=1): a text profile of the tick with every [stats] report.
    shared::prof::enable_from_env();
    if args.iter().any(|a| a == "--prof") {
        shared::prof::enable(true);
    }
    // `--record DIR`: keep a replayable copy of this session (docs/HARNESS.md).
    let mut recorder = match arg("--record") {
        Some(dir) => {
            let r = sim::harness::Recorder::start(std::path::Path::new(&dir), &data, start_unix, game.dev_mode)?;
            println!("[starfall] recording session to {dir} (replay: cargo run -p sim --release --example replay -- {dir})");
            Some(r)
        }
        None => None,
    };
    if game.dev_mode {
        println!("[starfall] DEV MODE: /grow and /cash chat commands enabled");
    }
    // `--metrics FILE`: gameplay time series (sim::metrics) — a line per captain per second in combat, kills, rewards.
    let mut metrics = match arg("--metrics") {
        Some(path) => {
            use std::io::Write as _;
            let file = std::fs::OpenOptions::new().create(true).append(true).open(&path)?;
            sim::metrics::MetricsLog::attach(&mut game);
            println!("[starfall] gameplay metrics to {path}");
            let mut w = std::io::BufWriter::new(file);
            writeln!(w, "{}", serde_json::json!({ "ev": "start", "unix": start_unix }))?;
            Some((sim::metrics::MetricsLog::new(1.0, false), w))
        }
        None => None,
    };

    let listener = TcpListener::bind(("0.0.0.0", port)).await?;
    println!("[starfall] listening on ws://0.0.0.0:{port}");
    let (tx, mut rx) = mpsc::unbounded_channel::<Inbound>();

    tokio::spawn(async move {
        let mut next: ConnId = 1;
        loop {
            match listener.accept().await {
                Ok((stream, addr)) => {
                    let id = next;
                    next += 1;
                    let tx = tx.clone();
                    tokio::spawn(async move {
                        if let Err(e) = connection(id, stream, tx).await {
                            eprintln!("[conn {id} {addr}] {e}");
                        }
                    });
                }
                Err(e) => eprintln!("accept: {e}"),
            }
        }
    });

    let mut outs: HashMap<ConnId, Out> = HashMap::new();
    let dt = 1.0 / TICK_HZ;
    let mut interval = tokio::time::interval(Duration::from_secs_f32(dt));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut last_save = std::time::Instant::now();
    // Load report every 10 s while anyone is connected: tick cost and outbound bandwidth.
    let (mut stat_at, mut stat_ticks, mut stat_ns, mut stat_max, mut stat_bytes) = (std::time::Instant::now(), 0u32, 0u128, 0u128, 0usize);
    let shutdown = tokio::signal::ctrl_c();
    tokio::pin!(shutdown);
    loop {
        tokio::select! {
            _ = interval.tick() => {
                while let Ok(ev) = rx.try_recv() {
                    match ev {
                        Inbound::Open(id, out) => { outs.insert(id, out); }
                        Inbound::Msg(id, m) => {
                            if let Some(r) = recorder.as_mut() {
                                r.event(sim::harness::Event::Msg(id, m.clone()));
                            }
                            game.handle(id, m)
                        }
                        Inbound::Close(id) => {
                            if let Some(r) = recorder.as_mut() {
                                r.event(sim::harness::Event::Close(id));
                            }
                            outs.remove(&id);
                            game.disconnect(id);
                        }
                    }
                }
                let t0 = std::time::Instant::now();
                let unix = unix_now();
                game.tick(dt, unix);
                if let Some(r) = recorder.as_mut()
                    && let Err(e) = r.end_tick(&mut game, unix)
                {
                    eprintln!("[starfall] recording stopped: {e}");
                    recorder = None;
                }
                if recorder.is_none() {
                    game.issued_tokens.clear();
                }
                if let Some((log, w)) = metrics.as_mut() {
                    use std::io::Write as _;
                    for line in log.observe(&mut game) {
                        let _ = writeln!(w, "{line}");
                    }
                    if game.tick % TICK_HZ as u64 == 0 {
                        let _ = w.flush();
                    }
                }
                // Per-viewer snapshots are binary; encode the batch in parallel.
                let enc = shared::prof::span("encode");
                // In batches of 32: a few frames are encoded on one thread (see PARALLEL_SNAPSHOT_WORK).
                let frames: Vec<(ConnId, Message)> = game.out.par_drain(..).with_min_len(32).map(|(c, m)| (c, wire(&m))).collect();
                drop(enc);
                let mut slow = vec![];
                for (conn, frame) in frames {
                    if let Some(o) = outs.get(&conn) {
                        if o.queued.load(std::sync::atomic::Ordering::Relaxed) > SEND_BACKLOG {
                            slow.push(conn);
                            continue;
                        }
                        stat_bytes += frame.len();
                        o.queued.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        let _ = o.tx.send(frame);
                    }
                }
                slow.sort_unstable();
                slow.dedup();
                for conn in slow {
                    eprintln!("[conn {conn}] not reading ({SEND_BACKLOG}+ frames queued): disconnected");
                    if let Some(o) = outs.remove(&conn) {
                        let _ = o.tx.send(Message::Close(None));
                    }
                    game.disconnect(conn);
                }
                let ns = t0.elapsed().as_nanos();
                (stat_ticks, stat_ns, stat_max) = (stat_ticks + 1, stat_ns + ns, stat_max.max(ns));
                let secs = stat_at.elapsed().as_secs_f64();
                if secs >= 10.0 {
                    // The span profile covers the same window as the stats (idle windows are dropped).
                    let window_prof = shared::prof::take();
                    if !outs.is_empty() {
                        let phases: Vec<String> = game.prof.iter().zip(sim::game::PROF_PHASES).filter(|(ns, _)| **ns as f64 / stat_ticks as f64 > 5e4).map(|(ns, name)| format!("{name} {:.2}", *ns as f64 / stat_ticks as f64 / 1e6)).collect();
                        println!("[stats] phases ms/tick: {}", phases.join(", "));
                        let ents: usize = game.systems.values().map(|s| s.ents.len()).sum();
                        println!(
                            "[stats] {} conns, {} entities | tick+encode avg {:.2} ms, max {:.2} ms (budget {:.0} ms) | out {:.1} KB/s per conn",
                            outs.len(), ents, stat_ns as f64 / stat_ticks as f64 / 1e6, stat_max as f64 / 1e6, dt * 1000.0, stat_bytes as f64 / secs / 1e3 / outs.len() as f64
                        );
                        if !window_prof.is_empty() {
                            print!("[prof] last {stat_ticks} ticks:\n{}", window_prof.report(stat_ticks as u64, "tick"));
                        }
                    }
                    (stat_at, stat_ticks, stat_ns, stat_max, stat_bytes) = (std::time::Instant::now(), 0, 0, 0, 0);
                    game.prof = [0; 8];
                }
                for k in game.kicked.drain(..) {
                    if let Some(o) = outs.remove(&k) {
                        let _ = o.tx.send(Message::Close(None));
                    }
                }
                if last_save.elapsed() > Duration::from_secs(15) {
                    game.persist_tick();
                    last_save = std::time::Instant::now();
                }
            }
            _ = &mut shutdown => {
                println!("[starfall] shutting down, saving…");
                game.store.dirty = true;
                game.persist_tick();
                return Ok(());
            }
        }
    }
}

fn wire(m: &ServerMsg) -> Message {
    match encode_snapshot(m) {
        Some(b) => Message::binary(b),
        None => Message::text(encode(m)),
    }
}

async fn connection(id: ConnId, stream: TcpStream, tx: mpsc::UnboundedSender<Inbound>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    stream.set_nodelay(true)?;
    let config = tokio_tungstenite::tungstenite::protocol::WebSocketConfig::default().max_message_size(Some(MAX_FRAME)).max_frame_size(Some(MAX_FRAME));
    let ws = tokio_tungstenite::accept_async_with_config(stream, Some(config)).await?;
    let (mut sink, mut source) = ws.split();
    let (otx, mut orx) = mpsc::unbounded_channel::<Message>();
    let queued = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    // Pings are answered right here so the HUD shows network round-trip, not tick queueing.
    let pong = otx.clone();
    tx.send(Inbound::Open(id, Out { tx: otx, queued: queued.clone() })).ok();
    let writer = tokio::spawn(async move {
        while let Some(m) = orx.recv().await {
            let close = matches!(m, Message::Close(_));
            let r = sink.send(m).await;
            queued.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
            if r.is_err() || close {
                break;
            }
        }
    });
    // Token bucket over the messages this client sends (MSG_RATE, MSG_BURST).
    let (mut tokens, mut last, mut dropped) = (MSG_BURST, std::time::Instant::now(), 0u32);
    while let Some(frame) = source.next().await {
        let now = std::time::Instant::now();
        tokens = (tokens + now.duration_since(last).as_secs_f64() * MSG_RATE).min(MSG_BURST);
        last = now;
        if tokens < 1.0 {
            dropped += 1;
            if dropped >= FLOOD_KICK {
                eprintln!("[conn {id}] flooding: disconnected");
                break;
            }
            continue;
        }
        tokens -= 1.0;
        dropped = 0;
        // A reset connection must still reach the game loop as a Close, or the ship stays as a ghost.
        let frame = match frame {
            Ok(f) => f,
            Err(e) => {
                eprintln!("[conn {id}] closed: {e}");
                break;
            }
        };
        match frame {
            Message::Text(t) => match decode::<ClientMsg>(&t) {
                Ok(ClientMsg::Ping { t }) => {
                    pong.send(Message::text(encode(&ServerMsg::Pong { t }))).ok();
                }
                Ok(m) => {
                    tx.send(Inbound::Msg(id, m)).ok();
                }
                Err(e) => eprintln!("[conn {id}] bad frame: {e}"),
            },
            Message::Close(_) => break,
            _ => {}
        }
    }
    tx.send(Inbound::Close(id)).ok();
    writer.abort();
    Ok(())
}

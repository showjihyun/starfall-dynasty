//! STARFALL DYNASTY authoritative server.
//!
//! `server [--port 7777] [--data ./data]`
//!
//! WebSocket transport, JSON frames. One game loop task owns all state at 20 Hz;
//! connection tasks only shuttle frames.

use futures_util::{SinkExt, StreamExt};
use server::game::{ConnId, Game};
use server::store::Store;
use rayon::prelude::*;
use shared::protocol::{ClientMsg, DEFAULT_PORT, ServerMsg, TICK_HZ, decode, encode, encode_snapshot};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;

enum Inbound {
    Open(ConnId, mpsc::UnboundedSender<Message>),
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
    // `--record DIR`: keep a replayable copy of this session (docs/HARNESS.md).
    let mut recorder = match arg("--record") {
        Some(dir) => {
            let r = server::harness::Recorder::start(std::path::Path::new(&dir), &data, start_unix, game.dev_mode)?;
            println!("[starfall] recording session to {dir} (replay: cargo run -p server --release --example replay -- {dir})");
            Some(r)
        }
        None => None,
    };
    if game.dev_mode {
        println!("[starfall] DEV MODE: /grow and /cash chat commands enabled");
    }

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

    let mut outs: HashMap<ConnId, mpsc::UnboundedSender<Message>> = HashMap::new();
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
                                r.event(server::harness::Event::Msg(id, m.clone()));
                            }
                            game.handle(id, m)
                        }
                        Inbound::Close(id) => {
                            if let Some(r) = recorder.as_mut() {
                                r.event(server::harness::Event::Close(id));
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
                // Per-viewer snapshots are binary; encode the batch in parallel.
                let frames: Vec<(ConnId, Message)> = game.out.par_drain(..).map(|(c, m)| (c, wire(&m))).collect();
                for (conn, frame) in frames {
                    if let Some(o) = outs.get(&conn) {
                        stat_bytes += frame.len();
                        let _ = o.send(frame);
                    }
                }
                let ns = t0.elapsed().as_nanos();
                (stat_ticks, stat_ns, stat_max) = (stat_ticks + 1, stat_ns + ns, stat_max.max(ns));
                let secs = stat_at.elapsed().as_secs_f64();
                if secs >= 10.0 {
                    if !outs.is_empty() {
                        let phases: Vec<String> = game.prof.iter().zip(server::game::PROF_PHASES).filter(|(ns, _)| **ns as f64 / stat_ticks as f64 > 5e4).map(|(ns, name)| format!("{name} {:.2}", *ns as f64 / stat_ticks as f64 / 1e6)).collect();
                        println!("[stats] phases ms/tick: {}", phases.join(", "));
                        let ents: usize = game.systems.values().map(|s| s.ents.len()).sum();
                        println!(
                            "[stats] {} conns, {} entities | tick+encode avg {:.2} ms, max {:.2} ms (budget {:.0} ms) | out {:.1} KB/s per conn",
                            outs.len(), ents, stat_ns as f64 / stat_ticks as f64 / 1e6, stat_max as f64 / 1e6, dt * 1000.0, stat_bytes as f64 / secs / 1e3 / outs.len() as f64
                        );
                    }
                    (stat_at, stat_ticks, stat_ns, stat_max, stat_bytes) = (std::time::Instant::now(), 0, 0, 0, 0);
                    game.prof = [0; 8];
                }
                for k in game.kicked.drain(..) {
                    if let Some(o) = outs.remove(&k) {
                        let _ = o.send(Message::Close(None));
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
    let ws = tokio_tungstenite::accept_async(stream).await?;
    let (mut sink, mut source) = ws.split();
    let (otx, mut orx) = mpsc::unbounded_channel::<Message>();
    // Pings are answered right here so the HUD shows network round-trip, not tick queueing.
    let pong = otx.clone();
    tx.send(Inbound::Open(id, otx)).ok();
    let writer = tokio::spawn(async move {
        while let Some(m) = orx.recv().await {
            let close = matches!(m, Message::Close(_));
            if sink.send(m).await.is_err() || close {
                break;
            }
        }
    });
    while let Some(frame) = source.next().await {
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

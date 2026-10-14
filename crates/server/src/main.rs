//! STARFALL DYNASTY authoritative server.
//!
//! `server [--port 7777] [--data ./data]`
//!
//! WebSocket transport, JSON frames. One game loop task owns all state at 20 Hz;
//! connection tasks only shuttle frames.

use futures_util::{SinkExt, StreamExt};
use server::game::{ConnId, Game};
use server::store::Store;
use shared::protocol::{ClientMsg, TICK_HZ, decode, encode, DEFAULT_PORT};
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
    let mut game = Game::new(store, history, unix_now());
    game.dev_mode = args.iter().any(|a| a == "--dev");
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
    let shutdown = tokio::signal::ctrl_c();
    tokio::pin!(shutdown);
    loop {
        tokio::select! {
            _ = interval.tick() => {
                while let Ok(ev) = rx.try_recv() {
                    match ev {
                        Inbound::Open(id, out) => { outs.insert(id, out); }
                        Inbound::Msg(id, m) => game.handle(id, m),
                        Inbound::Close(id) => { outs.remove(&id); game.disconnect(id); }
                    }
                }
                game.tick(dt, unix_now());
                for (conn, msg) in game.out.drain(..) {
                    if let Some(o) = outs.get(&conn) {
                        let _ = o.send(Message::text(encode(&msg)));
                    }
                }
                for (conns, msg) in game.out_shared.drain(..) {
                    let frame = Message::text(encode(&msg));
                    for c in conns {
                        if let Some(o) = outs.get(&c) {
                            let _ = o.send(frame.clone());
                        }
                    }
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

async fn connection(id: ConnId, stream: TcpStream, tx: mpsc::UnboundedSender<Inbound>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    stream.set_nodelay(true)?;
    let ws = tokio_tungstenite::accept_async(stream).await?;
    let (mut sink, mut source) = ws.split();
    let (otx, mut orx) = mpsc::unbounded_channel::<Message>();
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

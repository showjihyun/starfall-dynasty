//! 3D replay of a recorded session (docs/HARNESS.md, "재생 시각화").
//!
//! `client --replay DIR [--as NAME] [--from SECS] [--speed X] [--shot out.png --shot-at T]`
//!
//! The recorded simulation runs inside the client, advanced by the client clock, and every message
//! it sends to the viewed captain goes through the wire encoding into the same channel the network
//! feeds — so the screen is exactly what that captain saw live, from the same code path. Recorded
//! digests are checked as the replay passes them, and the verdict is printed as one line. With
//! `STARFALL_FIXED_FPS` the same command always gives the same image, so a bug report folder
//! becomes a reproducible picture as well as a reproducible state.
//!
//! Input is ignored: the client's own commands (controls, pings) are discarded.

use crate::net::{Net, NetEvent};
use bevy::prelude::*;
use crossbeam_channel::{Receiver, Sender};
use sim::harness::{Replayer, Session};
use shared::protocol::{ClientMsg, PlayerId, ServerMsg, TICK_HZ, decode, decode_snapshot, encode, encode_snapshot};

#[derive(Resource)]
pub struct Replay {
    r: Replayer,
    feed: Sender<NetEvent>,
    sink: Receiver<ClientMsg>,
    /// Captain to watch (`--as`); the first to log in when not given.
    want: Option<String>,
    pid: Option<PlayerId>,
    from_tick: u64,
    speed: f64,
    t0: Option<f64>,
    reported: bool,
}

/// Loads the recording and returns the replay plus the `Net` the rest of the client reads from.
pub fn open(dir: &str) -> (Replay, Net) {
    let a: Vec<String> = std::env::args().collect();
    let get = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
    let session = Session::load(std::path::Path::new(dir)).unwrap_or_else(|e| panic!("cannot load recording {dir}: {e}"));
    let r = session.replayer().unwrap_or_else(|e| panic!("cannot rebuild the recorded start state: {e}"));
    let from_tick = get("--from").and_then(|v| v.parse::<f64>().ok()).map(|s| (s * TICK_HZ as f64) as u64).unwrap_or(0);
    let speed = get("--speed").and_then(|v| v.parse::<f64>().ok()).filter(|s| *s > 0.0).unwrap_or(1.0);
    let want = get("--as");
    info!("[replay] {dir}: {} ticks ({:.0} s), watching {}, from {:.0} s at {speed}x", r.last, r.last as f64 / TICK_HZ as f64, want.as_deref().unwrap_or("the first captain"), from_tick as f64 / TICK_HZ as f64);
    let (net, feed, sink) = Net::local();
    (Replay { r, feed, sink, want, pid: None, from_tick, speed, t0: None, reported: false }, net)
}

/// Before the network pump: run the recording up to the clock and forward the viewed captain's messages.
pub fn drive(mut rp: ResMut<Replay>, time: Res<Time<Real>>) {
    let _prof = shared::prof::span("replay::drive");
    let now = time.elapsed_secs_f64();
    let t0 = *rp.t0.get_or_insert(now);
    let target = rp.from_tick + ((now - t0) * TICK_HZ as f64 * rp.speed) as u64;
    while rp.r.game.tick < target && rp.r.step() {
        forward(&mut rp);
    }
    while rp.sink.try_recv().is_ok() {}
    if rp.r.done() && !rp.reported {
        rp.reported = true;
        let rep = rp.r.finish();
        // Plain stdout, one line: agents grep for it next to the screenshot.
        println!("[replay] end of recording at tick {}: {} — final state {}", rep.ticks, rep.verdict(), rep.final_digest);
        if rp.pid.is_none() {
            println!("[replay] nobody to watch: {} never logged in", rp.want.as_deref().unwrap_or("no captain"));
        }
    }
}

fn forward(rp: &mut Replay) {
    let g = &rp.r.game;
    if rp.pid.is_none() {
        rp.pid = match &rp.want {
            Some(name) => g.store.find_by_name(name),
            None => g.conn_of.keys().next().copied(),
        };
        if let Some(p) = rp.pid {
            info!("[replay] tick {}: watching player {p}", g.tick);
        }
    }
    let Some(conn) = rp.pid.and_then(|p| g.conn_of.get(&p)).copied() else { return };
    for (c, m) in &g.out {
        if *c != conn {
            continue;
        }
        let Some(mut m) = wire(m) else { continue };
        // Recorded tokens are pseudonyms: never let the client store one as a login.
        if let ServerMsg::Welcome { token, .. } = &mut m {
            token.clear();
        }
        let _ = rp.feed.send(NetEvent::Msg(Box::new(m)));
    }
}

/// The message as the client would have decoded it off the socket.
pub(crate) fn wire(m: &ServerMsg) -> Option<ServerMsg> {
    match encode_snapshot(m) {
        Some(b) => decode_snapshot(&b),
        None => decode(&encode(m)).ok(),
    }
}

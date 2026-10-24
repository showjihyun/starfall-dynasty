//! The real battle behind `client --bench` (docs/STUDIO_PIPELINE.md, 성능): two alliances at war,
//! each captain holding its place in its side's line and engaging at range (`sim::battle::Doctrine::WIDE`,
//! main goal "epic spread-out battles"), run by the actual simulation on its
//! own thread at the server's tick rate — a local server, in effect. Only the watching captain's
//! messages reach the client, through the wire encoding and the same channel the network feeds,
//! with the server's real interest management (snapshot budgets, effect radius). So the frame
//! cost measured is that of a player in a 1000-captain fight, escorts, losses and all.
//!
//! The captains are spread over the war zone's fronts (`galaxy::fronts`), unevenly, so each front
//! has its own fight and its own winner, and the HUD's front report has something to say.
//!
//! The sim thread runs on wall-clock time: frame timings are what they are, but two runs do not
//! give the same picture (the synthetic scene, `STARFALL_BENCH_SYNTH=1`, does).

use crate::net::{Net, NetEvent};
use bevy::prelude::*;
use crossbeam_channel::Sender;
use shared::protocol::*;
use sim::battle::SYSTEM;
use sim::game::Game;
use std::time::{Duration, Instant};

/// Starts the battle with `captains` (half on each side) and returns the client's end of it.
pub fn start(captains: usize) -> Net {
    let (net, feed, sink) = Net::local();
    std::thread::Builder::new()
        .name("bench-battle".into())
        .spawn(move || {
            run(captains.max(4), feed);
            drop(sink);
        })
        .expect("battle thread");
    net
}

fn run(n: usize, feed: Sender<NetEvent>) {
    let setup = Instant::now();
    let (mut g, mut caps) = sim::battle::setup(n, &sim::battle::Doctrine::WIDE);
    let nf = g.systems[&SYSTEM].fronts.len().clamp(1, 4);
    flush(&mut g, &feed);
    let on: Vec<usize> = (0..nf).map(|f| caps.iter().filter(|c| c.front == f).count()).collect();
    info!("bench battle: {n} captains on {nf} fronts {on:?}, {} ships in system {SYSTEM} (set up in {:.1} s)", g.systems[&SYSTEM].ents.len(), setup.elapsed().as_secs_f32());

    let dt = 1.0 / TICK_HZ;
    let start = Instant::now();
    let mut tick = 0u64;
    loop {
        sim::battle::steer(&mut g, &mut caps, tick, &sim::battle::Doctrine::WIDE);
        g.tick(dt, sim::battle::T0 + tick / TICK_HZ as u64);
        tick += 1;
        if !flush(&mut g, &feed) {
            return; // the client has gone
        }
        let due = start + Duration::from_secs_f64(tick as f64 * dt as f64);
        if let Some(wait) = due.checked_duration_since(Instant::now()) {
            std::thread::sleep(wait);
        }
    }
}

/// Sends the watching captain's messages to the client and drops everyone else's.
/// False once the client has closed its end.
fn flush(g: &mut Game, feed: &Sender<NetEvent>) -> bool {
    let mut ok = true;
    for (c, m) in g.out.drain(..) {
        if c == 1
            && let Some(m) = crate::replay::wire(&m)
        {
            ok &= feed.send(NetEvent::Msg(Box::new(m))).is_ok();
        }
    }
    ok
}


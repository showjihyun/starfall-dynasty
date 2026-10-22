//! The real battle behind `client --bench` (docs/STUDIO_PIPELINE.md, 성능): two alliances at war,
//! each captain steering at and firing on the nearest enemy, run by the actual simulation on its
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
use shared::galaxy::{Species, SystemId};
use shared::protocol::*;
use shared::ships::{HullClass, Module};
use sim::game::{ConnId, Game};
use sim::store::Store;
use std::time::{Duration, Instant};

/// The war-zone system the battle is fought in.
const SYSTEM: SystemId = 7;
/// Gap between two sides' lines on a front at the start (u); fronts are further apart than this.
const GAP: f32 = 1100.0;
/// Spacing between captains in a battle line (u).
const SPACING: f32 = 150.0;
/// How each side spreads its captains over the fronts (weights per front, in `galaxy::fronts`
/// order): uneven on purpose, so each front has its own winner.
const SPREAD: [[usize; 4]; 2] = [[5, 2, 3, 2], [3, 4, 2, 3]];

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

struct Captain {
    conn: ConnId,
    side: usize,
    front: usize,
    target: Option<EntityId>,
}

fn run(n: usize, feed: Sender<NetEvent>) {
    let t0 = 1_800_000_000u64;
    let mut g = Game::new(Store::memory(), history::HistoryEngine::new(), t0);
    let setup = Instant::now();
    let nf = g.systems[&SYSTEM].fronts.len().clamp(1, 4);
    // Conn 1 watches: a capital behind side 0's line on the first front.
    let mut caps = vec![];
    let mut pids = vec![];
    for i in 0..n {
        let conn = i as ConnId + 1;
        let side = i % 2;
        g.handle(conn, ClientMsg::Hello { version: PROTOCOL_VERSION, name: format!("{}{:04}", ["Aster", "Ember"][side], i), token: None });
        let hull = HullClass::ALL[(i / 2) % 5];
        g.handle(conn, ClientMsg::CreateCharacter { species: Species::ALL[i % 3], profession: Profession::CombatPilot, hull, ship_name: format!("S{i}"), livery: if side == 0 { 1 } else { 4 } });
        let pid = g.sessions[&conn].player.unwrap();
        // A third fighters, a third cruisers, a third capitals (the viewer flies a capital).
        let lvl = if i == 0 { 9 } else { [1u8, 5, 9][(i / 2) % 3] };
        let p = g.store.profile_mut(pid).unwrap();
        for m in Module::ALL {
            p.ship.modules.insert(m, lvl);
        }
        p.wallet.credits += 10_000;
        let w = &SPREAD[side][..nf];
        let mut k = (i / 2) % w.iter().sum::<usize>();
        let front = w.iter().position(|x| if k < *x { true } else { k -= x; false }).unwrap();
        caps.push(Captain { conn, side, front: if i == 0 { 0 } else { front }, target: None });
        pids.push(pid);
    }
    // Two alliances at war, joined directly (the invite/accept round trip tells every member about
    // every new one: minutes for 500 a side). Their ships carry the alliance from spawning.
    for side in 0..2 {
        g.handle(caps[side].conn, ClientMsg::AllianceCreate { name: ["Aster Pact", "Ember Pact"][side].into(), tag: ["AST", "EMB"][side].into() });
        let aid = g.alliance_of(pids[side]).expect("alliance created");
        let a = g.store.data.alliances.iter_mut().find(|a| a.id == aid).unwrap();
        a.members.extend(pids.iter().enumerate().filter(|(i, _)| i % 2 == side && *i != side).map(|(_, p)| *p));
    }
    let enemy = g.alliance_of(pids[1]).unwrap();
    g.handle(caps[0].conn, ClientMsg::AllianceDeclareWar { target: enemy });
    for pid in &pids {
        g.spawn_player(*pid, SYSTEM, false);
    }
    place(&mut g, &caps);
    flush(&mut g, &feed);
    let on: Vec<usize> = (0..nf).map(|f| caps.iter().filter(|c| c.front == f).count()).collect();
    info!("bench battle: {n} captains on {nf} fronts {on:?}, {} ships in system {SYSTEM} (set up in {:.1} s)", g.systems[&SYSTEM].ents.len(), setup.elapsed().as_secs_f32());

    let dt = 1.0 / TICK_HZ;
    let start = Instant::now();
    let mut tick = 0u64;
    loop {
        steer(&mut g, &mut caps, tick);
        g.tick(dt, t0 + tick / TICK_HZ as u64);
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

/// On each front, two facing blocks, Homeworld-style: rows across, layers in height, ranks in
/// depth, escorts beside their captain. The lines face each other across the front's centre, along
/// the direction from the system's middle out to the front, so neighbouring fronts stay apart.
fn place(g: &mut Game, caps: &[Captain]) {
    let fronts = g.systems[&SYSTEM].fronts.clone();
    let middle = fronts.iter().map(|f| f.pos).sum::<Vec3>() / fronts.len().max(1) as f32;
    let mut k = vec![[0usize; 2]; fronts.len().max(1)];
    for c in caps {
        let Some(sess) = g.sessions.get(&c.conn) else { continue };
        let (ent, escorts) = (sess.entity, sess.escorts.clone());
        let Some(ent) = ent else { continue };
        let f = &fronts[c.front.min(fronts.len() - 1)];
        // Across the front: side 0 on the inner side, side 1 on the outer.
        let axis = (f.pos - middle).with_y(0.0).normalize_or(Vec3::X);
        let side_dir = axis.cross(Vec3::Y);
        let face = if c.side == 0 { 1.0 } else { -1.0 };
        let rot = Quat::from_rotation_arc(Vec3::NEG_Z, axis * face);
        let n_here = caps.iter().filter(|x| x.front == c.front && x.side == c.side).count();
        let across = (n_here as f32).cbrt().ceil().max(1.0) as usize;
        let pos = if c.conn == 1 {
            // The watcher: high above its own (inner) line, looking across the front at the enemy;
            // still nearer this front than any other, so the HUD marks it as ours.
            f.pos - axis * (GAP / 2.0) + Vec3::Y * (across as f32 * SPACING * 0.6 + 250.0)
        } else {
            let i = k[c.front][c.side];
            k[c.front][c.side] += 1;
            let (row, layer, rank) = (i % across, (i / across) % across, i / (across * across));
            let half = across as f32 / 2.0;
            f.pos - axis * face * (GAP / 2.0 + rank as f32 * SPACING) + Vec3::Y * ((layer as f32 - half) * SPACING * 0.8) + side_dir * ((row as f32 - half) * SPACING)
        };
        let ents = &mut g.systems.get_mut(&SYSTEM).unwrap().ents;
        if let Some(e) = ents.get_mut(&ent) {
            (e.pos, e.rot) = (pos, rot);
        }
        for (j, id) in escorts.iter().enumerate() {
            if let Some(e) = ents.get_mut(id) {
                (e.pos, e.rot) = (pos + Vec3::new(0.0, -40.0, 0.0) + side_dir * (50.0 * (j as f32 + 1.0)), rot);
            }
        }
    }
}

/// Every captain flies at its target and fires; a twentieth of them pick the nearest enemy again
/// each tick (once a second each), and anyone whose target is gone picks at once.
fn steer(g: &mut Game, caps: &mut [Captain], tick: u64) {
    let sys = &g.systems[&SYSTEM];
    let side_of: std::collections::BTreeMap<PlayerId, usize> = caps.iter().filter_map(|c| Some((g.sessions[&c.conn].player?, c.side))).collect();
    let ships: Vec<(EntityId, Vec3, usize)> = sys.ents.values().filter_map(|e| Some((e.id, e.pos, *side_of.get(&e.owner?)?))).collect();
    let mut orders = vec![];
    for (i, c) in caps.iter_mut().enumerate() {
        if c.conn == 1 {
            continue;
        }
        let Some(me) = g.sessions[&c.conn].entity.and_then(|id| sys.ents.get(&id)) else { continue };
        let alive = c.target.is_some_and(|t| sys.ents.contains_key(&t));
        let mut retarget = None;
        if !alive || (i as u64 + tick).is_multiple_of(TICK_HZ as u64) {
            c.target = ships.iter().filter(|s| s.2 != c.side).min_by(|a, b| a.1.distance_squared(me.pos).total_cmp(&b.1.distance_squared(me.pos))).map(|s| s.0);
            retarget = Some(c.target);
        }
        let Some(t) = c.target.and_then(|t| sys.ents.get(&t)) else { continue };
        let to = t.pos - me.pos;
        let local = me.rot.inverse() * to.normalize_or_zero();
        let mut ctl = Controls { yaw: (local.x * 4.0).clamp(-1.0, 1.0), pitch: (local.y * 4.0).clamp(-1.0, 1.0), thrust: if to.length() > 700.0 { 0.7 } else { 0.1 }, fire: local.z < -0.8, ..Default::default() };
        if local.z > 0.0 {
            ctl.yaw = if local.x >= 0.0 { 1.0 } else { -1.0 };
        }
        orders.push((c.conn, retarget, ctl));
    }
    for (conn, retarget, ctl) in orders {
        if let Some(t) = retarget {
            g.handle(conn, ClientMsg::SetTarget(t));
        }
        g.handle(conn, ClientMsg::Controls(ctl));
    }
}

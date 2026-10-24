//! The two-alliance war behind `client --bench` and `examples/battle` (docs/STUDIO_PIPELINE.md, 성능;
//! main goal "epic spread-out battles"): captains spread over a war zone's fronts in Homeworld-style
//! blocks, each steering at and firing on the nearest enemy. The client runs it on its own thread
//! for the frame-cost bench; `examples/battle` runs it headless and measures how far the fighting
//! spreads ([`spread`]).

use crate::game::{ConnId, Game};
use crate::store::Store;
use shared::galaxy::{Species, SystemId};
use shared::glam::{Quat, Vec2, Vec3};
use shared::protocol::*;
use shared::ships::{HullClass, Module};

/// The war-zone system the battle is fought in.
pub const SYSTEM: SystemId = 7;
/// Gap between two sides' lines on a front at the start (u); fronts are further apart than this.
pub const GAP: f32 = 1100.0;
/// Spacing between captains in a battle line (u).
pub const SPACING: f32 = 150.0;
/// How each side spreads its captains over the fronts (weights per front, in `galaxy::fronts`
/// order): uneven on purpose, so each front has its own winner.
const SPREAD: [[usize; 4]; 2] = [[5, 2, 3, 2], [3, 4, 2, 3]];

pub struct Captain {
    pub conn: ConnId,
    pub side: usize,
    pub front: usize,
    pub target: Option<EntityId>,
    /// Its place in its side's block on the front, kept while fighting under [`Doctrine::lanes`]:
    /// the front's centre, the axis across the front toward the enemy, and the offset along the
    /// line (x) and in height (y).
    pub lane: (Vec3, Vec3, Vec2),
    /// Direction away from the nearest ally, when one is closer than [`Doctrine::separation`].
    pub crowd: Option<Vec3>,
}

/// How the captains fight (main goal "epic spread-out battles"; `examples/battle --doctrine`).
#[derive(Clone, Copy, Debug)]
pub struct Doctrine {
    /// Spacing between captains in a block (u).
    pub spacing: f32,
    /// Hold the target at this share of the own weapon range (0: fly straight at it).
    pub keep: f32,
    /// Keep the own place along the line and in height, and prefer enemies facing it.
    pub lanes: bool,
    /// Slide away from an ally closer than this (0: never).
    pub separation: f32,
    /// Captains on a front form fleets of five (`shared::formation`): leaders fly by the rest of the
    /// doctrine, the others keep their post around their leader. Free: no fleets.
    pub formation: shared::formation::Formation,
}

impl Doctrine {
    /// Everyone at the nearest enemy (the bench until 2026-10-24).
    pub const OLD: Doctrine = Doctrine { spacing: SPACING, keep: 0.0, lanes: false, separation: 0.0, formation: shared::formation::Formation::Free };
    pub const WIDE: Doctrine = Doctrine { spacing: 300.0, keep: 0.8, lanes: true, separation: 120.0, formation: shared::formation::Formation::Free };
}

/// Unix time the battle starts at.
pub const T0: u64 = 1_800_000_000;

/// Sets up the war with `n` captains (half a side; conn 1 is the watcher, a capital above side 0's
/// line on the first front) and places them on the fronts.
pub fn setup(n: usize, d: &Doctrine) -> (Game, Vec<Captain>) {
    let mut g = Game::new(Store::memory(), history::HistoryEngine::new(), T0);
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
        caps.push(Captain { conn, side, front: if i == 0 { 0 } else { front }, target: None, lane: (Vec3::ZERO, Vec3::X, Vec2::ZERO), crowd: None });
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
    place(&mut g, &mut caps, d.spacing);
    if d.formation != shared::formation::Formation::Free {
        // Fleets of five per side and front, in placement order (neighbours in the block).
        let mut groups: std::collections::BTreeMap<(usize, usize), Vec<PlayerId>> = Default::default();
        for (c, pid) in caps.iter().zip(&pids).skip(1) {
            groups.entry((c.side, c.front)).or_default().push(*pid);
        }
        for members in groups.values() {
            for five in members.chunks(5).filter(|f| f.len() > 1) {
                let fid = g.new_fleet(five[0]);
                let f = g.fleets.get_mut(&fid).unwrap();
                f.members.extend_from_slice(&five[1..]);
                f.formation = d.formation;
                g.broadcast_fleet(five[0]);
            }
        }
    }
    (g, caps)
}

/// On each front, two facing blocks, Homeworld-style: rows across, layers in height, ranks in
/// depth, escorts beside their captain. The lines face each other across the front's centre, along
/// the direction from the system's middle out to the front, so neighbouring fronts stay apart.
pub fn place(g: &mut Game, caps: &mut [Captain], spacing: f32) {
    let fronts = g.systems[&SYSTEM].fronts.clone();
    let middle = fronts.iter().map(|f| f.pos).sum::<Vec3>() / fronts.len().max(1) as f32;
    let mut k = vec![[0usize; 2]; fronts.len().max(1)];
    let counts: Vec<(usize, usize)> = caps.iter().map(|c| (c.front, c.side)).collect();
    for c in caps.iter_mut() {
        let Some(sess) = g.sessions.get(&c.conn) else { continue };
        let (ent, escorts) = (sess.entity, sess.escorts.clone());
        let Some(ent) = ent else { continue };
        let f = &fronts[c.front.min(fronts.len() - 1)];
        // Across the front: side 0 on the inner side, side 1 on the outer.
        let axis = (f.pos - middle).with_y(0.0).normalize_or(Vec3::X);
        let side_dir = axis.cross(Vec3::Y);
        let face = if c.side == 0 { 1.0 } else { -1.0 };
        let rot = Quat::from_rotation_arc(Vec3::NEG_Z, axis * face);
        let n_here = counts.iter().filter(|x| **x == (c.front, c.side)).count();
        let across = (n_here as f32).cbrt().ceil().max(1.0) as usize;
        let pos = if c.conn == 1 {
            // The watcher: high above its own (inner) line, looking across the front at the enemy;
            // still nearer this front than any other, so the HUD marks it as ours.
            f.pos - axis * (GAP / 2.0) + Vec3::Y * (across as f32 * spacing * 0.6 + 250.0)
        } else {
            let i = k[c.front][c.side];
            k[c.front][c.side] += 1;
            let (row, layer, rank) = (i % across, (i / across) % across, i / (across * across));
            let half = across as f32 / 2.0;
            let lane = Vec2::new((row as f32 - half) * spacing, (layer as f32 - half) * spacing * 0.8);
            c.lane = (f.pos, axis * face, lane);
            f.pos - axis * face * (GAP / 2.0 + rank as f32 * spacing) + Vec3::Y * lane.y + side_dir * lane.x
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

/// Every captain flies at its target and fires; a twentieth of them pick a target again each tick
/// (once a second each), and anyone whose target is gone picks at once. Under [`Doctrine::OLD`]
/// that is the nearest enemy, flown straight at; otherwise see [`Doctrine`].
pub fn steer(g: &mut Game, caps: &mut [Captain], tick: u64, d: &Doctrine) {
    if d.keep > 0.0 || d.lanes || d.separation > 0.0 || d.formation != shared::formation::Formation::Free {
        return steer_doctrine(g, caps, tick, d);
    }
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

fn steer_doctrine(g: &mut Game, caps: &mut [Captain], tick: u64, d: &Doctrine) {
    let sys = &g.systems[&SYSTEM];
    // Fleet members (not leaders) of fleets in a formation.
    let posts: std::collections::BTreeSet<ConnId> = g.fleets.values().filter(|f| f.formation != shared::formation::Formation::Free).flat_map(|f| f.members.iter().filter(|m| **m != f.leader).filter_map(|m| g.conn_of.get(m).copied())).collect();
    let side_of: std::collections::BTreeMap<PlayerId, usize> = caps.iter().filter_map(|c| Some((g.sessions[&c.conn].player?, c.side))).collect();
    let ships: Vec<(EntityId, Vec3, usize, bool)> = sys.ents.values().filter_map(|e| Some((e.id, e.pos, *side_of.get(&e.owner?)?, e.kind == EntityKind::Player))).collect();
    let mut orders = vec![];
    for (i, c) in caps.iter_mut().enumerate() {
        if c.conn == 1 {
            continue;
        }
        let Some(me) = g.sessions[&c.conn].entity.and_then(|id| sys.ents.get(&id)) else { continue };
        let (centre, axis, lane) = c.lane;
        let across = axis.cross(Vec3::Y);
        let home = centre + across * lane.x + Vec3::Y * lane.y;
        let alive = c.target.is_some_and(|t| sys.ents.contains_key(&t));
        let mut retarget = None;
        if !alive || (i as u64 + tick).is_multiple_of(TICK_HZ as u64) {
            // The enemy nearest, counting how far off our lane it is twice when keeping lanes.
            let off = |p: Vec3| if d.lanes { ((p - home).dot(across).abs() + (p.y - home.y).abs()) * 2.0 } else { 0.0 };
            c.target = ships.iter().filter(|s| s.2 != c.side).min_by(|a, b| (a.1.distance(me.pos) + off(a.1)).total_cmp(&(b.1.distance(me.pos) + off(b.1)))).map(|s| s.0);
            retarget = Some(c.target);
            c.crowd = (d.separation > 0.0)
                .then(|| ships.iter().filter(|s| s.2 == c.side && s.3 && s.0 != me.id).map(|s| (s.1, s.1.distance(me.pos))).filter(|s| s.1 < d.separation).min_by(|a, b| a.1.total_cmp(&b.1)))
                .flatten()
                .map(|(p, _)| (me.pos - p).normalize_or(Vec3::Y));
        }
        let Some(t) = c.target.and_then(|t| sys.ents.get(&t)) else { continue };
        let to = t.pos - me.pos;
        let dist = to.length();
        let local = me.rot.inverse() * to.normalize_or_zero();
        let mut ctl = Controls { yaw: (local.x * 4.0).clamp(-1.0, 1.0), pitch: (local.y * 4.0).clamp(-1.0, 1.0), fire: local.z < -0.8, ..Default::default() };
        if local.z > 0.0 {
            ctl.yaw = if local.x >= 0.0 { 1.0 } else { -1.0 };
        }
        // A fleet member keeps its formation post the way a player does: P (`hold_post`) flies it there,
        // the captain only aims and fires.
        if posts.contains(&c.conn) {
            ctl.hold_post = true;
            orders.push((c.conn, retarget, ctl));
            continue;
        }
        let keep = me.stats.weapon_range * d.keep;
        ctl.thrust = if d.keep <= 0.0 { if dist > 700.0 { 0.7 } else { 0.1 } } else if dist > keep * 1.1 { 0.7 } else if dist < keep * 0.8 { -0.5 } else { 0.0 };
        // Slide (not turn) back toward our lane and away from a crowding ally; the nose stays on the target.
        let mut slide = Vec3::ZERO;
        if d.lanes {
            let err = home - me.pos;
            let dir = to.normalize_or_zero();
            slide += (err - dir * err.dot(dir)) / 150.0;
        }
        if let Some(away) = c.crowd {
            slide += away * 1.5;
        }
        let sl = me.rot.inverse() * slide;
        ctl.strafe_x = sl.x.clamp(-1.0, 1.0);
        ctl.strafe_y = sl.y.clamp(-1.0, 1.0);
        orders.push((c.conn, retarget, ctl));
    }
    for (conn, retarget, ctl) in orders {
        if let Some(t) = retarget {
            g.handle(conn, ClientMsg::SetTarget(t));
        }
        g.handle(conn, ClientMsg::Controls(ctl));
    }
}

/// How far one front's fight is spread (main goal "epic spread-out battles"). Distances in u.
#[derive(Clone, Debug, Default)]
pub struct Spread {
    pub front: String,
    /// Fighting ships on this front (captains and escorts, both sides).
    pub ships: usize,
    /// RMS distance from the fight's centre, across (horizontal) and up (vertical).
    pub across: f32,
    pub up: f32,
    /// The fight's footprint: 90th-percentile horizontal distance from its centre.
    pub reach90: f32,
    /// Median distance to the nearest other ship (either side) — the clumping number.
    pub nearest: f32,
    /// Share of ships with another within 60 u (touching hulls, at this scale).
    pub crowded: f32,
    /// Median distance from a ship to the target it is on.
    pub engage: f32,
}

/// The spread of the fight on each front of the battle system, ships assigned to their nearest front.
pub fn spread(g: &Game) -> Vec<Spread> {
    let sys = &g.systems[&SYSTEM];
    let fronts = &sys.fronts;
    let ships: Vec<&crate::game::Ent> = sys.ents.values().filter(|e| !e.dead && e.owner.is_some() && matches!(e.kind, EntityKind::Player | EntityKind::Escort)).collect();
    let median = |mut v: Vec<f32>| -> f32 {
        if v.is_empty() {
            return 0.0;
        }
        v.sort_by(f32::total_cmp);
        v[v.len() / 2]
    };
    (0..fronts.len())
        .map(|f| {
            let here: Vec<&&crate::game::Ent> = ships.iter().filter(|e| shared::galaxy::front_of(fronts, e.pos) == Some(f)).collect();
            let n = here.len().max(1) as f32;
            let c = here.iter().map(|e| e.pos).sum::<Vec3>() / n;
            let across = (here.iter().map(|e| (e.pos - c).with_y(0.0).length_squared()).sum::<f32>() / n).sqrt();
            let up = (here.iter().map(|e| (e.pos.y - c.y).powi(2)).sum::<f32>() / n).sqrt();
            let mut h: Vec<f32> = here.iter().map(|e| (e.pos - c).with_y(0.0).length()).collect();
            h.sort_by(f32::total_cmp);
            let reach90 = h.get((h.len() * 9 / 10).min(h.len().saturating_sub(1))).copied().unwrap_or(0.0);
            let nn: Vec<f32> = here.iter().map(|a| here.iter().filter(|b| b.id != a.id).map(|b| a.pos.distance(b.pos)).fold(f32::MAX, f32::min)).filter(|d| *d < f32::MAX).collect();
            let crowded = nn.iter().filter(|d| **d < 60.0).count() as f32 / nn.len().max(1) as f32;
            let engage = median(here.iter().filter_map(|e| sys.ents.get(&e.target?).map(|t| t.pos.distance(e.pos))).collect());
            Spread { front: fronts[f].name.clone(), ships: here.len(), across, up, reach90, nearest: median(nn), crowded, engage }
        })
        .collect()
}

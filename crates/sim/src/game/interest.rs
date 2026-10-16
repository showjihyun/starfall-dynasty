//! Interest management for big battles (1000 captains in one system).
//! - A uniform spatial grid answers "ships near here" for targeting without scanning everyone.
//! - Every viewer gets its own snapshot: its own ship, escorts and target every tick, new or
//!   changed entities at once, and everything else by a priority accumulator (closer and bigger
//!   ships climb faster), capped at a per-tick budget. The client dead-reckons the rest.
//! - Effects are filtered to the viewer's neighbourhood and capped.
//!
//! Viewers are independent, so their snapshots are built in parallel.

use super::*;
use rayon::prelude::*;

pub const GRID_CELL: f32 = 1000.0;
/// Motion records per viewer per tick once a system is busier than this.
pub const SNAPSHOT_BUDGET: usize = 192;
/// Effects farther than this from the viewer are not sent.
pub const FX_RADIUS: f32 = 9000.0;
pub const FX_BUDGET: usize = 96;
/// Entities introduced to a viewer per tick (nearest first). Arriving in a crowded system then
/// costs a few ticks of trickle instead of one multi-megabyte frame.
pub const NEW_BUDGET: usize = 128;

type Cell = (i32, i32, i32);

/// What targeting needs to know about a ship, packed so a search touches no hash maps.
#[derive(Clone, Copy)]
pub struct Probe {
    pub id: EntityId,
    pub pos: Vec3,
    pub radius: f32,
    pub side: Option<PlayerId>,
    pub fleet: Option<FleetId>,
    pub alliance: Option<AllianceId>,
    pub docked: bool,
    /// An NPC with a target (pirates hunting someone).
    pub hunting: bool,
}

#[derive(Default)]
pub struct Grid {
    map: HashMap<Cell, Vec<Probe>>,
}

impl Grid {
    fn cell(p: Vec3) -> Cell {
        ((p.x / GRID_CELL).floor() as i32, (p.y / GRID_CELL).floor() as i32, (p.z / GRID_CELL).floor() as i32)
    }

    /// Live ships only (the only things weapons target).
    pub fn rebuild<'a>(&mut self, ents: impl Iterator<Item = &'a Ent>) {
        // Drop cells that stayed empty, keep the allocations of the rest.
        self.map.retain(|_, v| {
            let keep = !v.is_empty();
            v.clear();
            keep
        });
        for e in ents {
            if e.is_ship() && !e.dead {
                let p = Probe { id: e.id, pos: e.pos, radius: e.stats.radius, side: e.side(), fleet: e.fleet, alliance: e.alliance, docked: e.docked, hunting: e.ai.as_ref().is_some_and(|a| a.target.is_some()) };
                self.map.entry(Self::cell(e.pos)).or_default().push(p);
            }
        }
    }

    /// Visits ships in cells overlapping the sphere (a superset: callers still check distance).
    pub fn visit(&self, p: Vec3, r: f32, mut f: impl FnMut(&Probe)) {
        let (lo, hi) = (Self::cell(p - Vec3::splat(r)), Self::cell(p + Vec3::splat(r)));
        for x in lo.0..=hi.0 {
            for y in lo.1..=hi.1 {
                for z in lo.2..=hi.2 {
                    if let Some(v) = self.map.get(&(x, y, z)) {
                        v.iter().for_each(&mut f);
                    }
                }
            }
        }
    }
}

/// What one viewer has been told: entity id per system slot (0 = nothing) and the priority
/// each slot has accumulated since it was last sent.
#[derive(Default)]
pub struct View {
    known: Vec<EntityId>,
    prio: Vec<f32>,
    eye: Vec3,
}

/// Stable small indices for a system's entities so viewer state can be plain arrays.
#[derive(Default)]
pub struct Slots {
    ids: Vec<EntityId>,
    of: HashMap<EntityId, usize>,
    free: Vec<usize>,
}

struct Rec {
    id: EntityId,
    info: EntityInfo,
    info_changed: bool,
    dynamic: DynState,
    pos: Vec3,
    radius: f32,
    /// Never moves (wrecks): once introduced it needs no motion updates.
    still: bool,
    owner: Option<PlayerId>,
    fleet: Option<FleetId>,
    alliance: Option<AllianceId>,
}

struct FxAt {
    fx: Fx,
    pos: Vec3,
    /// Entities the effect is drawn on (must be known to the viewer), and whether it involves them.
    a: Option<(usize, EntityId)>,
    b: Option<(usize, EntityId)>,
}

struct Viewer {
    conn: ConnId,
    view: View,
    full: bool,
    me: Option<EntityId>,
    me_slot: Option<usize>,
    pid: Option<PlayerId>,
    target: Option<EntityId>,
}

impl Game {
    pub(super) fn send_snapshots(&mut self) {
        let mut by_sys: BTreeMap<SystemId, Vec<ConnId>> = BTreeMap::new();
        for (c, s) in &self.sessions {
            if s.player.is_some() && s.entity.is_some() || s.dead {
                by_sys.entry(s.system).or_default().push(*c);
            }
        }
        let fx = std::mem::take(&mut self.pending_fx);
        let tick = self.tick;
        for (sys, conns) in by_sys {
            let p = shared::prof::span("records");
            let sysm = self.systems.get_mut(&sys).unwrap();
            // Free slots of departed entities first, then seat newcomers (a reused slot reads as gone + new).
            for slot in 0..sysm.slots.ids.len() {
                let id = sysm.slots.ids[slot];
                if id != 0 && sysm.ents.get(&id).is_none_or(|e| e.dead) {
                    sysm.slots.ids[slot] = 0;
                    sysm.slots.of.remove(&id);
                    sysm.slots.free.push(slot);
                }
            }
            let mut recs: Vec<Option<Rec>> = Vec::new();
            for e in sysm.ents.values().filter(|e| !e.dead) {
                let slot = match sysm.slots.of.get(&e.id) {
                    Some(s) => *s,
                    None => {
                        let s = sysm.slots.free.pop().unwrap_or_else(|| {
                            sysm.slots.ids.push(0);
                            sysm.slots.ids.len() - 1
                        });
                        sysm.slots.ids[s] = e.id;
                        sysm.slots.of.insert(e.id, s);
                        s
                    }
                };
                let (info, dynamic) = pack(&e.state());
                let info_changed = sysm.sent_infos.get(&e.id) != Some(&info);
                if info_changed {
                    sysm.sent_infos.insert(e.id, info.clone());
                }
                if recs.len() <= slot {
                    recs.resize_with(slot + 1, || None);
                }
                recs[slot] = Some(Rec { id: e.id, info, info_changed, dynamic, pos: e.pos, radius: e.stats.radius, still: e.kind == EntityKind::Wreck, owner: e.owner, fleet: e.fleet, alliance: e.alliance });
            }
            recs.resize_with(sysm.slots.ids.len(), || None);
            let slots = &sysm.slots;
            sysm.sent_infos.retain(|id, _| slots.of.contains_key(id));

            let at = |id: EntityId| slots.of.get(&id).map(|s| (*s, id));
            let pos_of = |id: EntityId| sysm.ents.get(&id).map(|e| e.pos);
            let fx_at: Vec<FxAt> = fx
                .get(&sys)
                .into_iter()
                .flatten()
                .filter_map(|f| {
                    let (pos, a, b) = match f {
                        Fx::Shot { from, to, .. } => (pos_of(*from)?, at(*from), if *to != 0 { at(*to) } else { None }),
                        Fx::MissileLaunch { from, missile } => (pos_of(*from)?, at(*from), at(*missile)),
                        Fx::Explosion { pos, .. } => (Vec3::from(*pos), None, None),
                        Fx::ShieldHit { entity, .. } | Fx::WarpOut { entity } | Fx::WarpIn { entity } => (pos_of(*entity)?, at(*entity), None),
                    };
                    Some(FxAt { fx: f.clone(), pos, a, b })
                })
                .collect();

            let mut viewers: Vec<Viewer> = conns
                .iter()
                .map(|c| {
                    let s = self.sessions.get_mut(c).unwrap();
                    let me = s.entity;
                    let me_slot = me.and_then(|m| sysm.slots.of.get(&m).copied());
                    Viewer { conn: *c, view: std::mem::take(&mut s.view), full: std::mem::take(&mut s.needs_full), me, me_slot, pid: s.player, target: me.and_then(|m| sysm.ents.get(&m)).and_then(|e| e.target) }
                })
                .collect();
            drop(p);
            // Per-viewer snapshots on the rayon pool (one span for the whole parallel batch).
            let p = shared::prof::span("per-viewer");
            let msgs: Vec<ServerMsg> = viewers.par_iter_mut().map(|v| build_snapshot(v, tick, &recs, &fx_at)).collect();
            drop(p);
            for (v, m) in viewers.into_iter().zip(msgs) {
                self.sessions.get_mut(&v.conn).unwrap().view = v.view;
                self.out.push((v.conn, m));
            }
        }
    }
}

fn build_snapshot(v: &mut Viewer, tick: u64, recs: &[Option<Rec>], fx: &[FxAt]) -> ServerMsg {
    let view = &mut v.view;
    if v.full {
        view.known.clear();
        view.prio.clear();
    }
    view.known.resize(recs.len(), 0);
    view.prio.resize(recs.len(), 0.0);
    let mine = v.me_slot.and_then(|s| recs.get(s)?.as_ref());
    if let Some(m) = mine {
        view.eye = m.pos;
    }
    let (eye, my_fleet, my_alliance) = (view.eye, mine.and_then(|m| m.fleet), mine.and_then(|m| m.alliance));
    let mut infos = vec![];
    let mut gone = vec![];
    let mut forced: Vec<usize> = vec![];
    let mut cands: Vec<(usize, f32)> = vec![];
    let mut news: Vec<(usize, f32)> = vec![];
    let must = |r: &Rec| Some(r.id) == v.me || Some(r.id) == v.target || (v.pid.is_some() && r.owner == v.pid);
    let weight = |r: &Rec| {
        let d = r.pos.distance(eye);
        let friend = (my_fleet.is_some() && r.fleet == my_fleet) || (my_alliance.is_some() && r.alliance == my_alliance);
        ((r.radius + 30.0) / (d + 300.0) * 60.0 + 0.04) * if friend { 2.0 } else { 1.0 }
    };
    for (slot, rec) in recs.iter().enumerate() {
        let known = view.known[slot];
        let Some(r) = rec else {
            if known != 0 {
                gone.push(known);
                view.known[slot] = 0;
                view.prio[slot] = 0.0;
            }
            continue;
        };
        if known != r.id {
            if known != 0 {
                gone.push(known);
                view.known[slot] = 0;
                view.prio[slot] = 0.0;
            }
            if must(r) {
                view.known[slot] = r.id;
                infos.push(r.info.clone());
                forced.push(slot);
            } else {
                // Waiting to be introduced: closer ones climb the queue faster.
                view.prio[slot] += weight(r) + 0.5;
                news.push((slot, view.prio[slot]));
            }
            continue;
        }
        if r.info_changed {
            infos.push(r.info.clone());
            forced.push(slot);
            continue;
        }
        if must(r) {
            forced.push(slot);
            continue;
        }
        if r.still {
            continue;
        }
        view.prio[slot] += weight(r);
        cands.push((slot, view.prio[slot]));
    }
    if news.len() > NEW_BUDGET {
        news.select_nth_unstable_by(NEW_BUDGET - 1, |a, b| b.1.total_cmp(&a.1));
        news.truncate(NEW_BUDGET);
    }
    let budget = SNAPSHOT_BUDGET.saturating_sub(forced.len());
    if cands.len() > budget {
        if budget > 0 {
            cands.select_nth_unstable_by(budget - 1, |a, b| b.1.total_cmp(&a.1));
        }
        cands.truncate(budget);
    }
    for &(slot, _) in &news {
        let r = recs[slot].as_ref().unwrap();
        view.known[slot] = r.id;
        infos.push(r.info.clone());
    }
    let mut dynamic = Vec::with_capacity(forced.len() + cands.len() + news.len());
    for slot in forced.into_iter().chain(cands.into_iter().map(|c| c.0)).chain(news.into_iter().map(|c| c.0)) {
        view.prio[slot] = 0.0;
        dynamic.push(recs[slot].as_ref().unwrap().dynamic);
    }
    // Effects: always those on me, else nearby ones drawn on ships this viewer knows.
    let knows = |e: Option<(usize, EntityId)>| e.is_none_or(|(s, id)| view.known.get(s) == Some(&id));
    let mut near: Vec<(f32, &Fx)> = fx
        .iter()
        .filter_map(|f| {
            let involves_me = v.me.is_some() && [f.a, f.b].iter().flatten().any(|(_, id)| Some(*id) == v.me);
            let d = if involves_me { 0.0 } else { f.pos.distance(eye) };
            (d < FX_RADIUS && f.a.is_none_or(|a| knows(Some(a)))).then_some((d, &f.fx))
        })
        .collect();
    if near.len() > FX_BUDGET {
        near.select_nth_unstable_by(FX_BUDGET - 1, |a, b| a.0.total_cmp(&b.0));
        near.truncate(FX_BUDGET);
    }
    ServerMsg::Snapshot { tick, full: v.full, infos, gone, dynamic, fx: near.into_iter().map(|(_, f)| f.clone()).collect() }
}

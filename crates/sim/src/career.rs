//! Open-galaxy QA: a captain lives off station job boards and the goods market the way a player
//! would — dock, read the board, take the job that pays best for the warps it costs, fly it (warp
//! hops, the fight at the target, the dock at the far station, the goods bought elsewhere for a
//! supply run), come back for more; with nothing worth taking, carry goods along the best trade
//! route in range, or warp to the next station's board. The report says per kind of job how many
//! were done and failed, the minutes from accepting to the pay, the credits and the deaths, and the
//! CR per minute over the whole career (looking for work included) — the played number that
//! `examples/board.rs` only estimates, to hold against the mining loop (`sim::mining`).
//!
//! The pilot holds one job at a time, so each minute belongs to one kind. It does not take
//! prospect orders (they are mining with a delivery address: `sim::mining` covers the mining).
//! Deterministic like the rest of the simulation (`cargo run -p sim --release --example career`).

use crate::game::{COMBAT_LOCK, ConnId, Game};
use crate::playthrough::{THINK, next_hop, steer};
use crate::store::Store;
use shared::galaxy::{self, Species, SystemId};
use shared::jobs::{self, Job, JobId, JobKind, JobTag};
use shared::market::{self, Good};
use shared::protocol::{ClientMsg, Controls, EntityId, PROTOCOL_VERSION, Profession, TICK_HZ};
use shared::ships::{HullClass, Module};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug)]
pub struct Options {
    pub hull: HullClass,
    pub species: Species,
    /// Every module at this level.
    pub level: u8,
    /// Kinds of job the captain takes (prospect orders are never taken).
    pub kinds: Vec<JobTag>,
    /// Carry trade goods when no job is worth taking.
    pub trade: bool,
    /// Credits to start with (None: what a new captain has).
    pub credits: Option<u64>,
    /// Game time to play.
    pub secs: f32,
    pub unix: u64,
}

impl Default for Options {
    fn default() -> Self {
        Options { hull: HullClass::Talon, species: Species::Human, level: 1, kinds: vec![JobTag::Bounty, JobTag::Haul, JobTag::Survey, JobTag::Armada, JobTag::Supply], trade: false, credits: None, secs: 3.0 * 3600.0, unix: 1_800_000_000 }
    }
}

/// What came of one kind of work.
#[derive(Clone, Debug, Default)]
pub struct Tally {
    pub taken: u32,
    pub done: u32,
    pub failed: u32,
    /// Seconds from accepting to the end (or, for trade, from buying to selling).
    pub secs: f32,
    /// Net credits while it ran (pay, bounties on kills, goods bought and sold).
    pub credits: i64,
    pub deaths: u32,
}

impl Tally {
    pub fn per_min(&self) -> f32 {
        self.credits as f32 / (self.secs.max(1.0) / 60.0)
    }
}

#[derive(Clone, Debug, Default)]
pub struct Report {
    pub cargo: f32,
    pub warp_range: f32,
    /// Per kind of job, and the trade runs.
    pub jobs: BTreeMap<JobTag, Tally>,
    pub trade: Tally,
    /// Seconds docked with nothing to take or flying to another board.
    pub idle: f32,
    pub secs: f32,
    pub credits: i64,
    pub deaths: u32,
    pub boards_read: u32,
    pub systems: usize,
    /// What happened, one line per job or run.
    pub log: Vec<String>,
}

impl Report {
    pub fn per_min(&self) -> f32 {
        self.credits as f32 / (self.secs.max(1.0) / 60.0)
    }

    pub fn text(&self) -> String {
        let mut s = format!("hold {:.0}, warp {:.1} ly · {:.0} min played, {} boards read, {} systems seen\n", self.cargo, self.warp_range, self.secs / 60.0, self.boards_read, self.systems);
        s += &format!("  {:<10} {:>5} {:>5} {:>6} {:>8} {:>9} {:>8} {:>6}\n", "work", "taken", "done", "failed", "avg min", "net CR", "CR/min", "deaths");
        let row = |name: &str, t: &Tally| format!("  {:<10} {:>5} {:>5} {:>6} {:>8.1} {:>9} {:>8.0} {:>6}\n", name, t.taken, t.done, t.failed, t.secs / 60.0 / t.taken.max(1) as f32, t.credits, t.per_min(), t.deaths);
        for (k, t) in &self.jobs {
            s += &row(k.name(), t);
        }
        if self.trade.taken > 0 {
            s += &row("교역", &self.trade);
        }
        s += &format!("  looking for work {:.0} min ({:.0}%) · career {} CR net, {:.0} CR/min overall, {} deaths\n", self.idle / 60.0, self.idle / self.secs.max(1.0) * 100.0, self.credits, self.per_min(), self.deaths);
        s
    }
}

/// A job that has cost this many deaths is given up.
const MAX_DEATHS: u32 = 3;

/// Warp hops between two systems at this range (None: out of reach).
fn hops(from: SystemId, to: SystemId, range: f32) -> Option<u32> {
    let mut at = from;
    let mut n = 0;
    while at != to {
        at = next_hop(at, to, range)?;
        n += 1;
        if n > 64 {
            return None;
        }
    }
    Some(n)
}

/// Where the job sends the captain, in order, from `here` (survey: nearest first).
fn route(here: SystemId, j: &Job, range: f32) -> Option<u32> {
    let mut at = here;
    let mut total = 0;
    let mut left = j.kind.destinations(j.station);
    if let JobKind::Supply { good, .. } = &j.kind {
        // Out to where it is made and back.
        let src = source(j.station, *good, range)?;
        return Some(hops(here, src, range)? + hops(src, j.station, range)?);
    }
    while !left.is_empty() {
        left.sort_by_key(|s| hops(at, *s, range).unwrap_or(u32::MAX));
        let next = left.remove(0);
        total += hops(at, next, range)?;
        at = next;
    }
    Some(total)
}

/// The nearest station (in warps) that sells `good`.
fn source(to: SystemId, good: Good, range: f32) -> Option<SystemId> {
    let t = shared::tuning::get();
    galaxy::systems().iter().filter(|s| s.id != to && market::buy_price(&t.trade.market, s.id, good, 0.0).is_some()).filter_map(|s| hops(to, s.id, range).map(|h| (h, s.id))).min().map(|(_, s)| s)
}

/// The trade run worth most from `here` at today's prices (what the exchange shows): the good, where
/// to sell it within one warp, and how many to buy — as many as the hold and the wallet take while
/// the run still pays at least a tenth over its cost.
fn live_route(g: &Game, here: SystemId, range: f32, room: u32, credits: u64) -> Option<(Good, SystemId, u32)> {
    let t = &g.tuning.trade.market;
    let mut best: Option<(f32, Good, SystemId, u32)> = None;
    for good in market::economy(t, here).produce {
        let off = g.market_offset(here, good);
        for s in galaxy::systems() {
            if s.id == here || hops(here, s.id, range) != Some(1) {
                continue;
            }
            let to_off = g.market_offset(s.id, good);
            let mut n = room;
            while n > 0 {
                if let (Some((cost, _)), Some((gain, _))) = (market::buy_total(t, here, good, off, n), market::sell_total(t, s.id, good, to_off, n))
                    && cost <= credits as f32
                    && gain > cost * 1.1
                {
                    if best.is_none_or(|b| gain - cost > b.0) {
                        best = Some((gain - cost, good, s.id, n));
                    }
                    break;
                }
                n = n * 3 / 4;
            }
        }
    }
    best.map(|(_, g, s, n)| (g, s, n))
}

/// What the pilot is working on.
#[derive(Clone, Debug)]
enum Work {
    Job { id: JobId, tag: JobTag, start: f64, wallet: i64, deaths: u32, what: String, size: Option<(u8, u32)> },
    Trade { good: Good, to: SystemId, start: f64, wallet: i64, deaths: u32 },
}

/// Where to be next.
enum Goal {
    /// Docked at this system's station.
    Dock(SystemId),
    /// Anywhere in this system.
    Visit(SystemId),
    /// Destroy these.
    Fight(Vec<EntityId>),
}

/// Plays `o.secs` of a career on the open galaxy and reports it.
pub fn run(o: &Options) -> Report {
    let mut g = Game::new(Store::memory(), history::HistoryEngine::new(), o.unix);
    let conn: ConnId = 1;
    g.handle(conn, ClientMsg::Hello { version: PROTOCOL_VERSION, name: "의뢰QA".into(), token: None });
    g.handle(conn, ClientMsg::CreateCharacter { species: o.species, profession: Profession::CombatPilot, hull: o.hull, ship_name: "떠돌이호".into(), livery: 0 });
    g.out.clear();
    let mut rep = Report::default();
    let Some(pid) = g.sessions.get(&conn).and_then(|s| s.player) else { return rep };
    let home = g.store.profile(pid).map_or(0, |p| p.home);
    if let Some(p) = g.store.profile_mut(pid) {
        p.scenarios_completed = 5;
        for m in Module::ALL {
            p.ship.modules.insert(m, o.level.max(1));
        }
        if let Some(c) = o.credits {
            p.wallet.credits = c;
        }
    }
    g.spawn_player(pid, home, true);
    g.out.clear();
    let t = shared::tuning::get();
    let stats = g.store.profile(pid).unwrap().ship.stats_as(o.species);
    (rep.cargo, rep.warp_range) = (stats.cargo, stats.warp_range);
    let range = stats.warp_range;
    let start_wallet = g.store.profile(pid).unwrap().wallet.credits as i64;
    let wallet = |g: &Game| g.store.profile(pid).map_or(0, |p| p.wallet.credits as i64);

    let dt = 1.0 / TICK_HZ;
    let mut work: Option<Work> = None;
    // Boards read this shift (period, system): with nothing there, the next board is elsewhere.
    let mut read: BTreeSet<(u64, SystemId)> = BTreeSet::new();
    let mut hop_to: Option<SystemId> = None;
    // Jobs taken once are not taken again (a failed one stays on its board for the shift).
    let mut tried: BTreeSet<JobId> = BTreeSet::new();
    // Fights lost (target level, ships): a captain does not take one at least as hard again.
    let mut lost: Vec<(u8, u32)> = vec![];
    let fight_size = |k: &JobKind| match k {
        JobKind::Bounty { level, escorts, .. } => Some((*level, 1 + *escorts)),
        JobKind::Armada { level, wave, .. } => Some((*level, wave.iter().map(|w| w.1).sum())),
        _ => None,
    };
    let mut was_dead = false;
    let mut missile_cd = 0.0f32;
    for tick in 0..(o.secs * TICK_HZ) as u64 {
        let s = &g.sessions[&conn];
        let dead = s.dead;
        if dead && !was_dead {
            rep.deaths += 1;
            match &mut work {
                Some(Work::Job { deaths, .. } | Work::Trade { deaths, .. }) => *deaths += 1,
                None => {}
            }
        }
        was_dead = dead;
        if work.is_none() {
            rep.idle += dt;
        }
        if tick % THINK == 0 {
            let here = s.system;
            if dead {
                g.handle(conn, ClientMsg::Respawn);
            } else if s.warp.is_some() {
                g.handle(conn, ClientMsg::Controls(Controls::default()));
            } else if let Some(e) = s.entity.and_then(|id| g.ent(here, id)) {
                let (pos, rot, docked) = (e.pos, e.rot, e.docked);
                let in_combat = g.now - e.last_hit < COMBAT_LOCK;
                let shooting: Vec<EntityId> = e.attackers.iter().rev().filter(|(a, _, at)| g.now - at < 8.0 && g.ent(here, *a).is_some_and(|x| !x.dead)).map(|(a, _, _)| *a).collect();
                let prof = g.store.profile(pid).unwrap().clone();

                // ---- Is the work over?
                let mut finished = None;
                match &work {
                    Some(Work::Job { id, tag, start, wallet: w0, deaths, what, size }) => {
                        let gone = !prof.jobs.iter().any(|a| a.job.id == *id);
                        if gone || *deaths >= MAX_DEATHS {
                            if !gone {
                                g.handle(conn, ClientMsg::AbandonJob { id: *id });
                            }
                            let done = gone && prof.career.jobs.get(tag).copied().unwrap_or(0) > rep.jobs.get(tag).map_or(0, |t| t.done);
                            if !done && let Some(sz) = size {
                                lost.push(*sz);
                            }
                            finished = Some((Some(*tag), *start, *w0, *deaths, done, what.clone()));
                        }
                    }
                    Some(Work::Trade { good, start, wallet: w0, deaths, to }) => {
                        let held = prof.cargo.get(good.key()).copied().unwrap_or(0);
                        if (held == 0 && docked && here == *to) || *deaths >= MAX_DEATHS || (held == 0 && here != *to) {
                            let what = format!("교역 {} → {}", good.name(), galaxy::system(*to).name);
                            finished = Some((None, *start, *w0, *deaths, held == 0 && here == *to, what));
                        }
                    }
                    None => {}
                }
                if let Some((tag, start, w0, deaths, done, what)) = finished {
                    let tally = match tag {
                        Some(k) => rep.jobs.entry(k).or_default(),
                        None => &mut rep.trade,
                    };
                    let net = wallet(&g) - w0;
                    tally.taken += 1;
                    tally.secs += (g.now - start) as f32;
                    tally.credits += net;
                    tally.deaths += deaths;
                    if done {
                        tally.done += 1;
                    } else {
                        tally.failed += 1;
                    }
                    rep.log.push(format!("{:>5.1} min  {} {what} — {:.1} min, {net:+} CR{}", start / 60.0, if done { "완수" } else { "실패" }, (g.now - start) / 60.0, if deaths > 0 { format!(", {deaths} deaths") } else { String::new() }));
                    work = None;
                }

                // ---- Pick up work at the station docked at.
                if work.is_none() && docked {
                    let period = jobs::period(&t, g.unix);
                    if read.insert((period, here)) {
                        rep.boards_read += 1;
                    }
                    let board = jobs::board(&t, here, period);
                    let best = board
                        .iter()
                        .filter(|j| o.kinds.contains(&j.kind.tag()) && j.kind.tag() != JobTag::Prospect && !tried.contains(&j.id))
                        .filter(|j| fight_size(&j.kind).is_none_or(|(l, n)| !lost.iter().any(|(fl, fn_)| l >= *fl && n >= *fn_)))
                        .filter(|j| !matches!(j.kind, JobKind::Haul { units, .. } if prof.cargo_used() + units > stats.cargo as u32))
                        .filter(|j| match &j.kind {
                            JobKind::Supply { good, units } => {
                                *units + prof.cargo_used() <= stats.cargo as u32 && source(j.station, *good, range).and_then(|s| market::buy_price(&t.trade.market, s, *good, 0.0)).is_some_and(|p| p * (*units as f32) * 1.2 < prof.wallet.credits as f32)
                            }
                            _ => true,
                        })
                        .filter_map(|j| route(here, j, range).map(|h| (j.reward.credits as f32 / (h as f32 + 1.0), j)))
                        .max_by(|a, b| a.0.total_cmp(&b.0))
                        .map(|(_, j)| j.clone());
                    if let Some(j) = best {
                        tried.insert(j.id);
                        g.handle(conn, ClientMsg::AcceptJob { id: j.id });
                        if g.store.profile(pid).unwrap().jobs.iter().any(|a| a.job.id == j.id) {
                            work = Some(Work::Job { id: j.id, tag: j.kind.tag(), start: g.now, wallet: wallet(&g), deaths: 0, what: format!("[{}] {}", j.kind.tag().name(), j.describe()), size: fight_size(&j.kind) });
                            hop_to = None;
                        }
                    } else if o.trade
                        && let Some((good, to, units)) = live_route(&g, here, range, (stats.cargo as u32).saturating_sub(prof.cargo_used()), prof.wallet.credits)
                    {
                        let w0 = wallet(&g);
                        {
                            g.handle(conn, ClientMsg::BuyGood { good, units });
                            if g.store.profile(pid).unwrap().cargo.get(good.key()).copied().unwrap_or(0) > 0 {
                                work = Some(Work::Trade { good, to, start: g.now, wallet: w0, deaths: 0 });
                                hop_to = None;
                            }
                        }
                    }
                    if work.is_none() && hop_to.is_none() {
                        // Nothing here: the nearest board not read this shift.
                        hop_to = galaxy::systems().iter().filter(|x| !read.contains(&(period, x.id))).filter_map(|x| hops(here, x.id, range).map(|h| (h, x.id))).min().map(|(_, x)| x);
                    }
                }

                // ---- Where to be.
                let prof = g.store.profile(pid).unwrap().clone();
                let goal = match &work {
                    Some(Work::Job { id, .. }) => prof.jobs.iter().find(|a| a.job.id == *id).map(|a| match &a.job.kind {
                        JobKind::Haul { to, .. } => Goal::Dock(*to),
                        JobKind::Survey { .. } => {
                            let mut left = a.remaining();
                            left.sort_by_key(|x| hops(here, *x, range).unwrap_or(u32::MAX));
                            Goal::Visit(left.first().copied().unwrap_or(here))
                        }
                        JobKind::Bounty { system, .. } | JobKind::Armada { system, .. } => {
                            let alive: Vec<EntityId> = g.sessions[&conn].job_waves.get(id).filter(|w| w.0 == here).map(|w| w.1.iter().copied().filter(|x| g.ent(here, *x).is_some_and(|n| !n.dead)).collect()).unwrap_or_default();
                            if *system == here && !alive.is_empty() { Goal::Fight(alive) } else { Goal::Visit(*system) }
                        }
                        JobKind::Supply { good, units } => {
                            if prof.cargo.get(good.key()).copied().unwrap_or(0) >= *units {
                                Goal::Dock(a.job.station)
                            } else {
                                Goal::Dock(source(a.job.station, *good, range).unwrap_or(a.job.station))
                            }
                        }
                        JobKind::Prospect { .. } => Goal::Dock(a.job.station),
                    }),
                    Some(Work::Trade { to, .. }) => Some(Goal::Dock(*to)),
                    None => hop_to.map(Goal::Dock).or(Some(Goal::Dock(here))),
                };

                // ---- Fly there.
                let station = g.systems[&here].layout.station;
                let fight = |g: &mut Game, target: EntityId, missile_cd: &mut f32| {
                    let Some(tp) = g.ent(here, target).map(|e| e.pos) else { return };
                    g.handle(conn, ClientMsg::SetTarget(Some(target)));
                    let mut c = steer(pos, rot, tp, stats.weapon_range * 0.55, stats.turn);
                    let local = rot.inverse() * (tp - pos).normalize_or_zero();
                    c.fire = -local.z > 0.9 && pos.distance(tp) < stats.weapon_range * 1.05;
                    c.boost &= pos.distance(tp) > stats.weapon_range.max(stats.turret_range) * 1.1;
                    g.handle(conn, ClientMsg::Controls(c));
                    if *missile_cd <= 0.0 && c.fire {
                        g.handle(conn, ClientMsg::FireMissile);
                        *missile_cd = 3.0;
                    }
                };
                missile_cd -= THINK as f32 / TICK_HZ;
                let travel = |g: &mut Game, to: SystemId| {
                    if docked {
                        g.handle(conn, ClientMsg::Undock);
                    } else if in_combat {
                        // Can't warp while fighting: get clear.
                        g.handle(conn, ClientMsg::Controls(Controls { thrust: 1.0, boost: true, ..Default::default() }));
                    } else if let Some(hop) = next_hop(here, to, range) {
                        g.handle(conn, ClientMsg::Warp { system: hop });
                    }
                };
                match goal {
                    _ if !shooting.is_empty() && !docked => fight(&mut g, shooting[0], &mut missile_cd),
                    Some(Goal::Fight(ids)) => {
                        let near = ids.iter().copied().min_by(|a, b| {
                            let d = |x: &EntityId| g.ent(here, *x).map_or(f32::MAX, |n| n.pos.distance(pos));
                            d(a).total_cmp(&d(b))
                        });
                        if docked {
                            g.handle(conn, ClientMsg::Undock);
                        } else if let Some(n) = near {
                            fight(&mut g, n, &mut missile_cd);
                        }
                    }
                    Some(Goal::Visit(to)) if to != here => travel(&mut g, to),
                    Some(Goal::Visit(_)) => {
                        // Arrived; the job moves on by itself. Undocked at the target with no wave yet: wait.
                        if docked {
                            g.handle(conn, ClientMsg::Undock);
                        } else {
                            g.handle(conn, ClientMsg::Controls(Controls::default()));
                        }
                    }
                    Some(Goal::Dock(to)) if to != here => travel(&mut g, to),
                    Some(Goal::Dock(_)) => {
                        if docked {
                            if hop_to == Some(here) {
                                hop_to = None;
                            }
                            // At a supply source: buy what the job needs.
                            if let Some(Work::Job { id, .. }) = &work
                                && let Some(a) = prof.jobs.iter().find(|a| a.job.id == *id)
                                && let JobKind::Supply { good, units } = &a.job.kind
                                && a.job.station != here
                            {
                                let held = prof.cargo.get(good.key()).copied().unwrap_or(0);
                                if held < *units {
                                    g.handle(conn, ClientMsg::BuyGood { good: *good, units: units - held });
                                    if g.store.profile(pid).unwrap().cargo.get(good.key()).copied().unwrap_or(0) == held {
                                        // Can't buy (credits, room): give it up.
                                        g.handle(conn, ClientMsg::AbandonJob { id: *id });
                                    }
                                }
                            }
                            // At the trade destination: sell it all.
                            if let Some(Work::Trade { good, to, .. }) = &work
                                && *to == here
                            {
                                let held = prof.cargo.get(good.key()).copied().unwrap_or(0);
                                if held > 0 {
                                    g.handle(conn, ClientMsg::SellGood { good: *good, units: held });
                                }
                            }
                        } else if pos.distance(station) > stats.radius + 120.0 {
                            g.handle(conn, ClientMsg::Controls(steer(pos, rot, station, stats.radius + 60.0, stats.turn)));
                        } else {
                            g.handle(conn, ClientMsg::Controls(Controls::default()));
                            g.handle(conn, ClientMsg::Dock);
                        }
                    }
                    None => {}
                }
            }
        }
        g.tick(dt, o.unix + tick / TICK_HZ as u64);
        g.out.clear();
        g.kicked.clear();
    }
    rep.secs = o.secs;
    rep.credits = wallet(&g) - start_wallet;
    rep.systems = g.store.profile(pid).map_or(0, |p| p.career.visited.len());
    // Work still under way at the end counts its time but not as done or failed.
    if let Some(Work::Job { tag, start, wallet: w0, .. }) = &work {
        let tl = rep.jobs.entry(*tag).or_default();
        tl.secs += (g.now - *start) as f32;
        tl.credits += wallet(&g) - w0;
    }
    rep
}

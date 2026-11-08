//! Long war QA: alliances of AI captains contest the galaxy for hours or days of game time under
//! the real rules — influence from pirates hunted and ore sold, control at the threshold, hourly
//! decay, tax, upkeep, overreach, wars and battles — and the report says how the map moved: who
//! held what each hour, how often systems changed hands, how far one alliance pulled ahead, what
//! the treasuries did and what the chronicle recorded. A living-world harness, as `campaign` is
//! for the story and `mining` for the economy loop.
//!
//! The leaders play a simple, readable strategy: take the home system, then the nearest system
//! not yet held; defend a held system whose influence sags; declare war on the alliance holding
//! the most systems once it holds more than they do, or on one contesting their target; make
//! peace after `war_hours` without gaining ground. Captains are hunters (pirates and enemy
//! captains in the target system) or miners (mine there, sell at its station, shoot back).
//!
//! Deterministic like the rest of the simulation (`cargo run -p sim --release --example war`).

use crate::game::{COMBAT_LOCK, ConnId, Game};
use crate::playthrough::{THINK, next_hop, steer};
use crate::store::Store;
use shared::galaxy::{self, Ore, RiskTier, SystemId};
use shared::protocol::{AllianceId, ClientMsg, Controls, EntityId, PROTOCOL_VERSION, PlayerId, Profession, TICK_HZ};
use shared::ships::{HullClass, Module};
use std::collections::BTreeMap;

#[derive(Clone, Debug)]
pub struct Options {
    pub alliances: usize,
    /// Captains per alliance.
    pub captains: usize,
    /// Game hours to run.
    pub hours: f32,
    /// Every module of every ship at this level.
    pub level: u8,
    /// Hours at war without gaining a system before a leader makes peace.
    pub war_hours: f32,
    /// The world runs this many seconds before the captains start (another world, as `campaign --seed`).
    pub seed: u32,
    pub unix: u64,
}

impl Default for Options {
    fn default() -> Self {
        Options { alliances: 4, captains: 6, hours: 24.0, level: 5, war_hours: 6.0, seed: 0, unix: 1_760_000_000 }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Role {
    Hunter,
    Miner,
}

struct Captain {
    conn: ConnId,
    pid: PlayerId,
    side: usize,
    role: Role,
    /// Which asteroid a hunter sweeps for pirates (moves on every few minutes).
    patrol: usize,
    target: Option<EntityId>,
}

struct Side {
    id: AllianceId,
    tag: String,
    leader: ConnId,
    home: SystemId,
    target: SystemId,
    /// Enemy → (game hour the war began, systems held then).
    wars: BTreeMap<AllianceId, (f32, usize)>,
    /// Most systems held since the last coup.
    peak: usize,
    /// Game hour of the last coup or secession (alliances are not shaken more than every 6 h).
    shaken: f32,
    /// Systems held at the last plan, and the latest one taken (with the hour): held a while longer.
    last_held: Vec<SystemId>,
    fresh: Option<(SystemId, f32)>,
    /// Game hour this side last made peace with each alliance (no new war on it for `war_hours`).
    peace_at: BTreeMap<AllianceId, f32>,
    /// Captains lost so far, and at the last plan (war weariness).
    losses: usize,
    losses_at_plan: usize,
}

/// A conquest is held this long (h) before the captains move on (QA_WAR: conquerors left at once
/// and 99 of 288 changes of hands were retaken within the hour).
const CONSOLIDATE_HOURS: f32 = 0.5;
/// Captains lost per captain between two plans (15 min) without gaining ground: the leader makes
/// peace (QA_WAR: two alliances traded 1100 losses in 5 h over a system neither took).
const WEARY_LOSSES: usize = 1;

/// Hours between an alliance's coups or secessions.
const UNREST_HOURS: f32 = 6.0;

/// One game hour, at its end.
#[derive(Clone, Debug, Default)]
pub struct Hour {
    /// Systems held, per alliance (in `Report::tags` order).
    pub held: Vec<usize>,
    pub treasury: Vec<u64>,
    /// Sum of each alliance's influence over all systems.
    pub influence: Vec<f32>,
    pub wars: usize,
}

#[derive(Clone, Debug, Default)]
pub struct Report {
    pub tags: Vec<String>,
    pub homes: Vec<SystemId>,
    pub hours: Vec<Hour>,
    /// Chronicle events by kind over the run.
    pub events: BTreeMap<String, usize>,
    /// Per system: how many times its controller changed, and the alliances that ever held it.
    pub turnover: BTreeMap<SystemId, (usize, Vec<String>)>,
    pub deaths: u32,
    /// Captain losses by killer ("captain" or "pirate") and by system.
    pub deaths_by: BTreeMap<String, usize>,
    pub deaths_in: BTreeMap<SystemId, usize>,
    /// Ships the captains destroyed (pirates and enemy captains).
    pub kills: u32,
    /// Changes of hands that followed a siege (station batteries silenced) in that system.
    pub by_siege: usize,
    /// A system taken back by the alliance that held it before, within the hour (siege ping-pong).
    pub retaken: usize,
    /// …of which the system had fallen to nobody in between (influence flickering at the threshold),
    /// not to another alliance.
    pub retaken_from_nobody: usize,
    /// Captain losses per (system, victim's tag, killer's tag) — tags from the captains' names, i.e.
    /// the alliance they started in.
    pub fights: BTreeMap<(SystemId, String, String), usize>,
    /// Station battery levels bought, per alliance (at the end).
    pub fortified: Vec<u8>,
    pub wall_secs: f32,
}

impl Report {
    /// Control changes per game hour, over all systems.
    pub fn changes_per_hour(&self) -> f32 {
        self.turnover.values().map(|(n, _)| *n).sum::<usize>() as f32 / self.hours.len().max(1) as f32
    }

    /// The largest share of all held systems one alliance had at any hour end (1 = it held them all).
    pub fn peak_share(&self) -> f32 {
        self.hours.iter().map(|h| {
            let total = h.held.iter().sum::<usize>().max(1);
            *h.held.iter().max().unwrap_or(&0) as f32 / total as f32
        }).fold(0.0, f32::max)
    }

    /// How many times the alliance holding the most systems changed (ties keep the old leader).
    pub fn lead_changes(&self) -> usize {
        let mut lead: Option<usize> = None;
        let mut n = 0;
        for h in &self.hours {
            let best = *h.held.iter().max().unwrap_or(&0);
            if best == 0 || lead.is_some_and(|l| h.held[l] == best) {
                continue;
            }
            let now = h.held.iter().position(|x| *x == best);
            if lead.is_some() && now != lead {
                n += 1;
            }
            lead = now;
        }
        n
    }

    pub fn text(&self) -> String {
        let mut s = String::new();
        let homes: Vec<String> = self.tags.iter().zip(&self.homes).map(|(t, h)| format!("[{t}] {}", galaxy::system(*h).name)).collect();
        s += &format!("alliances (home): {}\n", homes.join(" · "));
        s += &format!("{:>4}  {}  wars  treasury\n", "hour", self.tags.iter().map(|t| format!("{t:>4}")).collect::<Vec<_>>().join(""));
        for (i, h) in self.hours.iter().enumerate() {
            let step = (self.hours.len() / 24).max(1);
            if (i + 1) % step != 0 && i + 1 != self.hours.len() {
                continue;
            }
            s += &format!("{:>4}  {}  {:>4}  {}\n", i + 1, h.held.iter().map(|x| format!("{x:>4}")).collect::<Vec<_>>().join(""), h.wars, h.treasury.iter().map(|x| format!("{x}")).collect::<Vec<_>>().join("/"));
        }
        let held_ever = self.turnover.values().filter(|(_, by)| !by.is_empty()).count();
        let contested = self.turnover.values().filter(|(_, by)| by.len() >= 2).count();
        s += &format!(
            "control changes {} ({:.2}/h) · systems ever held {held_ever}, by 2+ alliances {contested} · lead changes {} · peak share {:.0}%\n",
            self.turnover.values().map(|(n, _)| *n).sum::<usize>(),
            self.changes_per_hour(),
            self.lead_changes(),
            self.peak_share() * 100.0
        );
        s += &format!("taken by siege {} · retaken within the hour {} (from nobody {}) · battery levels held at the end {}\n", self.by_siege, self.retaken, self.retaken_from_nobody, self.fortified.iter().map(|x| x.to_string()).collect::<Vec<_>>().join("/"));
        s += &format!("captain deaths {} · kills {} · chronicle: {}\n", self.deaths, self.kills, self.events.iter().map(|(k, n)| format!("{k} {n}")).collect::<Vec<_>>().join(", "));
        let mut worst: Vec<(&SystemId, &usize)> = self.deaths_in.iter().collect();
        worst.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
        s += &format!(
            "losses by {} · deadliest: {}\n",
            self.deaths_by.iter().map(|(k, n)| format!("{k} {n}")).collect::<Vec<_>>().join(", "),
            worst.iter().take(4).map(|(x, n)| format!("{} ({}) {n}", galaxy::system(**x).name, galaxy::system(**x).tier.label())).collect::<Vec<_>>().join(", ")
        );
        let mut fights: Vec<(&(SystemId, String, String), &usize)> = self.fights.iter().collect();
        fights.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
        s += &format!("worst fights (system: victim ← killer): {}\n", fights.iter().take(5).map(|((x, v, k), n)| format!("{}: {v} ← {k} {n}", galaxy::system(*x).name)).collect::<Vec<_>>().join(", "));
        s += &format!("({:.1} s wall clock)\n", self.wall_secs);
        s
    }
}

/// Non-Core systems spread out: the first nearest Sol Haven, then each farthest from those taken.
fn homes(n: usize) -> Vec<SystemId> {
    let cands: Vec<SystemId> = galaxy::systems().iter().filter(|s| matches!(s.tier, RiskTier::Frontier | RiskTier::Wild)).map(|s| s.id).collect();
    let mut out: Vec<SystemId> = vec![];
    while out.len() < n.min(cands.len()) {
        let next = cands
            .iter()
            .filter(|c| !out.contains(c))
            .max_by(|a, b| {
                let score = |c: SystemId| if out.is_empty() { -galaxy::distance_ly(0, c) } else { out.iter().map(|o| galaxy::distance_ly(*o, c)).fold(f32::MAX, f32::min) };
                score(**a).total_cmp(&score(**b)).then(b.cmp(a))
            })
            .copied();
        match next {
            Some(c) => out.push(c),
            None => break,
        }
    }
    out
}

pub fn run(o: &Options) -> Report {
    let start = std::time::Instant::now();
    let mut g = Game::new(Store::memory(), history::HistoryEngine::new(), o.unix);
    let dt = 1.0 / TICK_HZ;
    for tick in 0..o.seed as u64 * TICK_HZ as u64 {
        g.tick(dt, o.unix + tick / TICK_HZ as u64);
        g.out.clear();
    }
    let unix = o.unix + o.seed as u64;
    let home_of = homes(o.alliances);
    let tags = ["ORN", "VEL", "KHZ", "SOL", "AST", "NYX", "IRI", "TAU"];
    let mut sides: Vec<Side> = vec![];
    let mut caps: Vec<Captain> = vec![];
    for (si, &home) in home_of.iter().enumerate() {
        let tag = tags[si % tags.len()].to_string();
        for k in 0..o.captains {
            let conn = (si * 100 + k + 1) as ConnId;
            let name = format!("{tag}-{k:02}");
            let hull = HullClass::ALL[(si + k) % HullClass::ALL.len()];
            let role = if k % 2 == 0 { Role::Hunter } else { Role::Miner };
            let profession = if role == Role::Miner { Profession::Miner } else { Profession::CombatPilot };
            g.handle(conn, ClientMsg::Hello { version: PROTOCOL_VERSION, name: name.clone(), token: None });
            g.handle(conn, ClientMsg::CreateCharacter { species: galaxy::Species::ALL[si % galaxy::Species::ALL.len()], profession, hull, ship_name: format!("{name}호"), livery: si as u8 });
            let Some(pid) = g.sessions.get(&conn).and_then(|s| s.player) else { continue };
            if let Some(p) = g.store.profile_mut(pid) {
                p.scenarios_completed = 5;
                p.home = home;
                for m in Module::ALL {
                    p.ship.modules.insert(m, o.level.max(1));
                }
            }
            g.spawn_player(pid, home, true);
            if k == 0 {
                g.handle(conn, ClientMsg::AllianceCreate { name: format!("{tag} 연합"), tag: tag.clone() });
            } else if let Some(&leader) = caps.iter().find(|c| c.side == si).map(|c| &c.conn) {
                let aid = g.alliance_of(caps.iter().find(|c| c.conn == leader).unwrap().pid);
                g.handle(leader, ClientMsg::AllianceInvite { name: name.clone() });
                if let Some(aid) = aid {
                    g.handle(conn, ClientMsg::AllianceAccept { alliance: aid });
                }
            }
            caps.push(Captain { conn, pid, side: si, role, patrol: k, target: None });
        }
        let leader = caps.iter().find(|c| c.side == si).unwrap();
        let id = g.alliance_of(leader.pid).expect("alliance founded");
        sides.push(Side { id, tag, leader: leader.conn, home, target: home, wars: BTreeMap::new(), peak: 0, shaken: 0.0, last_held: vec![], fresh: None, peace_at: BTreeMap::new(), losses: 0, losses_at_plan: 0 });
    }
    g.out.clear();
    let mut rep = Report { tags: sides.iter().map(|s| s.tag.clone()).collect(), homes: home_of.clone(), ..Default::default() };
    let first_event = g.history.events.len();
    let mut was_dead: BTreeMap<ConnId, bool> = BTreeMap::new();
    let mut holders: BTreeMap<SystemId, Option<AllianceId>> = BTreeMap::new();
    // System → (tick of its last change of hands, the holder before it).
    let mut last_change: BTreeMap<SystemId, (u64, Option<AllianceId>)> = BTreeMap::new();
    let total = (o.hours * 3600.0 * TICK_HZ) as u64;
    let per_hour = (3600.0 * TICK_HZ) as u64;
    let per_plan = (900.0 * TICK_HZ) as u64;
    for c in &caps {
        g.handle(c.conn, ClientMsg::Undock);
    }
    for tick in 0..total {
        let hour = tick as f32 / per_hour as f32;
        if tick % per_plan == 0 {
            plan(&mut g, &mut sides, hour, o.war_hours);
            coups(&mut g, &mut sides, &caps, hour);
            if let Some((tag, home)) = secession(&mut g, &mut sides, &mut caps, hour, &tags) {
                rep.tags.push(tag);
                rep.homes.push(home);
            }
        }
        if tick % THINK == 0 {
            for c in caps.iter_mut() {
                let dead = g.sessions[&c.conn].dead;
                if dead && !was_dead.get(&c.conn).copied().unwrap_or(false) {
                    rep.deaths += 1;
                    sides[c.side].losses += 1;
                }
                was_dead.insert(c.conn, dead);
                let target = sides[c.side].target;
                fly(&mut g, c, target);
            }
        }
        g.tick(dt, unix + tick / TICK_HZ as u64);
        g.out.clear();
        g.kicked.clear();
        // Who holds what, once a game second: each change of hands, and who ever held a system.
        if tick % TICK_HZ as u64 == 0 {
            for x in galaxy::systems() {
                let now = g.controller(x.id);
                let before = holders.get(&x.id).copied().flatten();
                if before != now {
                    holders.insert(x.id, now);
                    if let Some((at, prev)) = last_change.get(&x.id)
                        && now.is_some()
                        && *prev == now
                        && tick - at < per_hour
                    {
                        rep.retaken += 1;
                        if before.is_none() {
                            rep.retaken_from_nobody += 1;
                        }
                    }
                    last_change.insert(x.id, (tick, before));
                    let t = rep.turnover.entry(x.id).or_default();
                    t.0 += 1;
                    if let Some(tag) = now.and_then(|a| sides.iter().find(|s| s.id == a)).map(|s| s.tag.clone())
                        && !t.1.contains(&tag)
                    {
                        t.1.push(tag);
                    }
                }
            }
        }
        if (tick + 1) % per_hour == 0 {
            rep.hours.push(Hour {
                held: sides.iter().map(|s| g.held_by(s.id).len()).collect(),
                treasury: sides.iter().map(|s| g.alliance(s.id).map_or(0, |a| a.treasury)).collect(),
                influence: sides.iter().map(|s| galaxy::systems().iter().map(|x| g.influence(x.id, s.id)).sum()).collect(),
                wars: sides.iter().map(|s| g.alliance(s.id).map_or(0, |a| a.wars.len())).sum::<usize>() / 2,
            });
        }
    }
    for e in &g.history.events[first_event..] {
        *rep.events.entry(e.kind.clone()).or_default() += 1;
        // Captain losses: to whom (another captain or the rest), and where.
        if e.kind == "SHIP_DESTROYED" {
            let killer = e.actors.get(1).cloned().unwrap_or_default();
            // No attacker entity: an enemy's station batteries (social.station_guard) or the star.
            let by = if sides.iter().any(|s| killer.contains(&format!("{}-", s.tag))) {
                "captain"
            } else if killer == "우주의 위험" {
                "station"
            } else {
                "pirate"
            };
            *rep.deaths_by.entry(by.into()).or_default() += 1;
            if let Some(s) = e.actors.first().and_then(|v| sides.iter().find(|s| v.contains(&format!("{}-", s.tag)))) {
                *rep.deaths_by.entry(format!("lost by {}", s.tag)).or_default() += 1;
            }
            *rep.deaths_in.entry(e.system).or_default() += 1;
            let tag_of = |v: &str| sides.iter().find(|s| v.contains(&format!("{}-", s.tag))).map_or("-".to_string(), |s| s.tag.clone());
            let victim = e.actors.first().map(|v| tag_of(v)).unwrap_or_default();
            *rep.fights.entry((e.system, victim, if by == "captain" { tag_of(&killer) } else { by.to_string() })).or_default() += 1;
        }
    }
    let sieges: Vec<u64> = g.history.events[first_event..].iter().filter(|e| e.kind == "SIEGE").map(|e| e.id).collect();
    rep.by_siege = g.history.events[first_event..].iter().filter(|e| e.kind == "SYSTEM_CONTROL" && e.causes.iter().any(|c| sieges.contains(c))).count();
    rep.fortified = sides.iter().map(|s| g.held_by(s.id).iter().map(|x| g.battery(*x).map_or(0, |b| b.level)).sum()).collect();
    rep.kills = caps.iter().filter_map(|c| g.store.profile(c.pid)).map(|p| p.kills).sum();
    rep.wall_secs = start.elapsed().as_secs_f32();
    rep
}

/// The leaders' quarter-hourly decisions: where to push, whom to fight, when to stop.
fn plan(g: &mut Game, sides: &mut [Side], hour: f32, war_hours: f32) {
    let t = g.tuning.social;
    let ids: Vec<AllianceId> = sides.iter().map(|s| s.id).collect();
    let held: BTreeMap<AllianceId, Vec<SystemId>> = ids.iter().map(|a| (*a, g.held_by(*a))).collect();
    for s in sides.iter_mut() {
        let mine = &held[&s.id];
        // Defend a held system slipping towards losing it, else take home, else expand to the nearest.
        let sagging = mine.iter().copied().filter(|x| g.influence(*x, s.id) < t.control_threshold * 0.8).min_by(|a, b| g.influence(*a, s.id).total_cmp(&g.influence(*b, s.id)));
        // A new system someone else still has a stake in (taken from them, or contested) is held a while.
        let contested = |x: SystemId| ids.iter().any(|o| *o != s.id && g.influence(x, *o) >= t.control_threshold * 0.3);
        if let Some(x) = mine.iter().copied().find(|x| !s.last_held.contains(x) && contested(*x)) {
            s.fresh = Some((x, hour));
        }
        let fresh = s.fresh.filter(|(x, at)| mine.contains(x) && hour - at < CONSOLIDATE_HOURS).map(|(x, _)| x);
        s.target = if let Some(x) = sagging {
            x
        } else if let Some(x) = fresh {
            x
        } else if !mine.contains(&s.home) {
            s.home
        } else {
            galaxy::systems()
                .iter()
                .filter(|x| x.tier != RiskTier::Core && !mine.contains(&x.id))
                .min_by(|a, b| {
                    // Nearer is better; a system someone else holds counts 1.5 ly further.
                    let cost = |x: SystemId| galaxy::distance_ly(s.home, x) + if g.controller(x).is_some() { 1.5 } else { 0.0 };
                    cost(a.id).total_cmp(&cost(b.id)).then(a.id.cmp(&b.id))
                })
                .map_or(s.home, |x| x.id)
        };
        // War on the biggest holder once it holds more than we do, or on whoever holds our target.
        let biggest = ids.iter().filter(|a| **a != s.id).max_by_key(|a| (held[*a].len(), std::cmp::Reverse(**a))).copied();
        let rival = g.controller(s.target).filter(|c| *c != s.id).or(biggest.filter(|b| held[b].len() >= 2 && held[b].len() > mine.len()));
        // A coalition: everyone else goes to war with the hegemon (free, social.hegemon_share).
        let rested = |s: &Side, r: AllianceId| s.peace_at.get(&r).is_none_or(|at| hour - at >= war_hours);
        if let Some(h) = g.hegemon().filter(|h| *h != s.id && !g.at_war(Some(s.id), Some(*h)) && rested(s, *h)) {
            g.handle(s.leader, ClientMsg::AllianceDeclareWar { target: h });
            if g.at_war(Some(s.id), Some(h)) {
                s.wars.insert(h, (hour, mine.len()));
            }
        }
        // Inside a truce only a much bigger rival is worth breaking the treaty for (social.betrayal).
        let unix = g.unix;
        let truce = |r: AllianceId| g.alliance(s.id).is_some_and(|a| a.truces.iter().any(|(x, end)| *x == r && *end > unix));
        let worth = |r: AllianceId| !truce(r) || held[&r].len() * 2 >= mine.len() * 3 + 2;
        if let Some(r) = rival
            && !g.at_war(Some(s.id), Some(r))
            && worth(r)
            && rested(s, r)
        {
            // Refused when the treasury cannot pay (social.war_cost per held system).
            g.handle(s.leader, ClientMsg::AllianceDeclareWar { target: r });
            if g.at_war(Some(s.id), Some(r)) {
                s.wars.insert(r, (hour, mine.len()));
            }
        }
        // Spare treasury (beyond the next system's upkeep for three hours, a rebuild and a war) goes into
        // the batteries of the held system nearest an enemy, lowest level first.
        let guard = t.station_guard;
        let treasury = g.alliance(s.id).map_or(0, |a| a.treasury);
        let reserve = 3 * g.upkeep(mine.len() + 1) + guard.rebuild_cost + t.war_cost * mine.len() as u64;
        let enemies: Vec<SystemId> = g.alliance(s.id).map(|a| a.wars.clone()).unwrap_or_default().iter().flat_map(|e| held.get(e).cloned().unwrap_or_default()).collect();
        let exposed = |x: SystemId| enemies.iter().map(|e| galaxy::distance_ly(x, *e)).fold(f32::MAX, f32::min);
        let level = |x: SystemId| g.battery(x).map_or(0, |b| b.level);
        if let Some(x) = mine.iter().copied().filter(|x| level(*x) < guard.fortify_max).min_by(|a, b| level(*a).cmp(&level(*b)).then(exposed(*a).total_cmp(&exposed(*b))).then(a.cmp(b)))
            && treasury >= reserve + guard.fortify_cost * (level(x) as u64 + 1)
        {
            g.handle(s.leader, ClientMsg::AllianceFortify { system: x });
        }
        // Wars declared on us count too (only our own declarations were remembered: the side attacked
        // never tired of a war).
        for r in g.alliance(s.id).map(|a| a.wars.clone()).unwrap_or_default() {
            s.wars.entry(r).or_insert((hour, mine.len()));
        }
        // Peace after `war_hours` without gaining ground, or at once when bleeding captains for nothing.
        let members = g.alliance(s.id).map_or(1, |a| a.members.len());
        let bleeding = s.losses - s.losses_at_plan >= WEARY_LOSSES * members && mine.len() <= s.last_held.len();
        let tired: Vec<AllianceId> = s.wars.iter().filter(|(_, (since, had))| (hour - since >= war_hours && mine.len() <= *had) || (bleeding && hour - since >= 0.5)).map(|(a, _)| *a).collect();
        for r in tired {
            if g.at_war(Some(s.id), Some(r)) {
                g.handle(s.leader, ClientMsg::AlliancePeace { target: r });
                s.peace_at.insert(r, hour);
            }
            s.wars.remove(&r);
        }
        // Wars the other side ended.
        s.wars.retain(|r, _| g.at_war(Some(s.id), Some(*r)));
        s.last_held = mine.clone();
        s.losses_at_plan = s.losses;
    }
}

/// Coups: an alliance down to half the systems it held at its peak raises its most decorated captain
/// (the rest of the members back them, `AllianceVote`); the new leader makes peace with everyone.
fn coups(g: &mut Game, sides: &mut [Side], caps: &[Captain], hour: f32) {
    for (si, s) in sides.iter_mut().enumerate() {
        let held = g.held_by(s.id).len();
        s.peak = s.peak.max(held);
        let Some(a) = g.alliance(s.id).cloned() else { continue };
        if hour - s.shaken < UNREST_HOURS || s.peak < 4 || held * 2 > s.peak || a.members.len() < 3 {
            continue;
        }
        let kills = |p: PlayerId| g.store.profile(p).map_or(0, |x| x.kills);
        let Some(challenger) = a.members.iter().copied().filter(|m| *m != a.leader).max_by(|x, y| kills(*x).cmp(&kills(*y)).then(y.cmp(x))) else { continue };
        let name = g.store.profile(challenger).map(|p| p.name.clone()).unwrap_or_default();
        for c in caps.iter().filter(|c| c.side == si && c.pid != a.leader && c.pid != challenger) {
            g.handle(c.conn, ClientMsg::AllianceVote { name: name.clone() });
        }
        if g.alliance(s.id).map(|x| x.leader) == Some(challenger) {
            s.leader = caps.iter().find(|c| c.pid == challenger).map_or(s.leader, |c| c.conn);
            s.shaken = hour;
            s.peak = held;
            for r in g.alliance(s.id).map(|x| x.wars.clone()).unwrap_or_default() {
                g.handle(s.leader, ClientMsg::AlliancePeace { target: r });
            }
            s.wars.clear();
        }
    }
}

/// Secession: an alliance that cannot pay its upkeep loses a governor — a member docked at one of its
/// stations away from home leaves with the influence there and founds an alliance of their own.
fn secession(g: &mut Game, sides: &mut Vec<Side>, caps: &mut [Captain], hour: f32, tags: &[&str]) -> Option<(String, SystemId)> {
    if sides.len() >= tags.len() {
        return None;
    }
    for si in 0..sides.len() {
        let s = &sides[si];
        let held = g.held_by(s.id);
        let treasury = g.alliance(s.id).map_or(0, |a| a.treasury);
        if hour - s.shaken < UNREST_HOURS || held.len() < 6 || treasury >= g.upkeep(held.len()) {
            continue;
        }
        let leader = g.alliance(s.id).map(|a| a.leader);
        let gov = caps.iter().position(|c| {
            let sess = &g.sessions[&c.conn];
            c.side == si && Some(c.pid) != leader && held.contains(&sess.system) && sess.system != s.home && sess.entity.and_then(|e| g.ent(sess.system, e)).is_some_and(|e| e.docked)
        });
        let Some(k) = gov else { continue };
        let (conn, pid) = (caps[k].conn, caps[k].pid);
        let sys = g.sessions[&conn].system;
        let tag = tags[sides.len()].to_string();
        // A rebel governor can afford a flag.
        if let Some(p) = g.store.profile_mut(pid) {
            p.wallet.credits = p.wallet.credits.max(crate::game::ALLIANCE_COST);
        }
        g.handle(conn, ClientMsg::AllianceLeave);
        g.handle(conn, ClientMsg::AllianceCreate { name: format!("{tag} 자유령"), tag: tag.clone() });
        let id = g.alliance_of(pid)?;
        if let Some(p) = g.store.profile_mut(pid) {
            p.home = sys;
        }
        sides[si].shaken = hour;
        caps[k].side = sides.len();
        sides.push(Side { id, tag: tag.clone(), leader: conn, home: sys, target: sys, wars: BTreeMap::new(), peak: 1, shaken: hour, last_held: vec![sys], fresh: None, peace_at: BTreeMap::new(), losses: 0, losses_at_plan: 0 });
        return Some((tag, sys));
    }
    None
}

/// A captain stays docked while an enemy captain is this close to the station (u).
const CAMP_RANGE: f32 = 2500.0;
/// Captains gathered off an enemy station before they storm its batteries together (four silence
/// fresh ones before the batteries sink them one by one: station_siege, station_siege_pair).
const SIEGE_PARTY: usize = 4;
/// The rally point, this far beyond the batteries' reach (u).
const RALLY: f32 = 900.0;

/// One captain's decision (every `THINK` ticks): fight what is hostile near, else do its job in the target system.
fn fly(g: &mut Game, c: &mut Captain, goal: SystemId) {
    let s = &g.sessions[&c.conn];
    if s.dead {
        g.handle(c.conn, ClientMsg::Respawn);
        return;
    }
    if s.warp.is_some() {
        return;
    }
    let here = s.system;
    let Some(me) = s.entity else { return };
    let Some(e) = g.ent(here, me) else { return };
    let (pos, rot, docked, mining) = (e.pos, e.rot, e.docked, e.mining.is_some());
    let stats = e.stats;
    let in_combat = g.now - e.last_hit < COMBAT_LOCK;
    let prof = g.store.profile(c.pid).unwrap().clone();
    if docked {
        for ore in Ore::ALL {
            let have = prof.cargo.get(ore.name()).copied().unwrap_or(0);
            if have > 0 {
                g.handle(c.conn, ClientMsg::Sell { ore, units: have });
            }
        }
        // Enemy captains waiting off the station: a player stays in dock (undocking into them was a death
        // loop — 2354 of 2801 deaths in 8 h at one alliance's home station; nothing defends a station).
        let mine = g.alliance_of(c.pid);
        let station = g.systems[&here].layout.station;
        let players = || g.systems[&here].ents.values().filter(|t| t.kind == shared::protocol::EntityKind::Player && !t.dead && t.pos.distance(station) < CAMP_RANGE);
        let campers = players().filter(|t| g.at_war(mine, t.alliance)).count();
        // Our batteries silenced: the system is being taken — sortie once we are as many as they are.
        let besieged = mine.is_some() && g.controller(here) == mine && g.battery(here).is_some_and(|b| b.silenced);
        if campers == 0 || (besieged && players().filter(|t| t.alliance == mine).count() >= campers) {
            g.handle(c.conn, ClientMsg::Undock);
        }
        return;
    }
    // Something hostile in reach: hunters look far for pirates, everyone answers what is close. Like a
    // player who is no outlaw: pirates, captains of alliances at war with us, and whoever shot at us —
    // not a neutral captain in lawless space (that is a crime, `wild_crime_bounty`).
    let look = if c.role == Role::Hunter { 4000.0 } else { 1200.0 };
    let mine = g.alliance_of(c.pid);
    // An enemy's station batteries (social.station_guard): a player keeps out of their reach.
    let station = g.systems[&here].layout.station;
    let reach = g.tuning.social.station_guard.range;
    let mut guarded = if g.controller(here).is_some_and(|o| g.at_war(mine, Some(o))) { reach + 300.0 } else { 0.0 };
    // The siege (social.station_guard hp): at the target, gather off the station and storm the batteries
    // together; once they are silenced, hold the approach.
    let at_siege = guarded > 0.0 && here == goal;
    let party = g.systems[&here].ents.values().filter(|t| t.kind == shared::protocol::EntityKind::Player && !t.dead && !t.docked && t.alliance == mine && t.pos.distance(station) < reach + RALLY + 1500.0).count();
    let storm = at_siege && (party >= SIEGE_PARTY || g.battery(here).is_some_and(|b| b.silenced));
    if storm {
        guarded = 0.0;
    }
    let shot_me: Vec<EntityId> = e.attackers.iter().filter(|(_, _, at)| g.now - at < 10.0).map(|(a, _, _)| *a).collect();
    if guarded > 0.0 && pos.distance(station) < guarded {
        let away = pos + (pos - station).normalize_or(shared::glam::Vec3::Z) * 600.0;
        g.handle(c.conn, ClientMsg::Controls(steer(pos, rot, away, 0.0, stats.turn)));
        return;
    }
    let foe = g.systems[&here]
        .ents
        .values()
        .filter(|t| t.is_ship() && !t.dead && t.pos.distance(pos) < look && t.pos.distance(station) >= guarded && g.can_damage(here, me, t.id))
        .filter(|t| t.owner.is_none() || shot_me.contains(&t.id) || g.at_war(mine, t.alliance))
        .min_by(|a, b| a.pos.distance(pos).total_cmp(&b.pos.distance(pos)).then(a.id.cmp(&b.id)))
        .map(|t| (t.id, t.pos));
    if let Some((tid, tp)) = foe {
        if c.target != Some(tid) {
            g.handle(c.conn, ClientMsg::SetTarget(Some(tid)));
            c.target = Some(tid);
        }
        let mut ctl = steer(pos, rot, tp, stats.weapon_range * 0.55, stats.turn);
        let local = rot.inverse() * (tp - pos).normalize_or_zero();
        // The target panel's stray-fire warning (view::hud::stray_fire_warning): with a neutral captain in
        // turret reach in lawless space, holding the trigger lets the spare batteries shoot them — a crime.
        // A player lets go and leaves the turrets to what is hunting them.
        let bystander = galaxy::system(here).tier.crime_tracked()
            && stats.turrets > 0
            && g.systems[&here].ents.values().any(|t| {
                t.id != tid && t.side().is_some() && t.side() != Some(c.pid) && !t.dead && !t.docked && !t.wanted && !g.at_war(mine, t.alliance) && t.pos.distance(pos) <= stats.turret_range + t.stats.radius && g.can_damage(here, me, t.id)
            });
        ctl.fire = !bystander && -local.z > 0.9 && pos.distance(tp) < stats.weapon_range * 1.05;
        ctl.boost &= pos.distance(tp) > stats.weapon_range.max(stats.turret_range) * 1.1;
        g.handle(c.conn, ClientMsg::Controls(ctl));
        return;
    }
    if here != goal {
        if !in_combat && let Some(hop) = next_hop(here, goal, stats.warp_range) {
            g.handle(c.conn, ClientMsg::Warp { system: hop });
        }
        return;
    }
    if storm {
        g.handle(c.conn, ClientMsg::Controls(steer(pos, rot, station, reach * 0.5, stats.turn)));
        return;
    }
    if at_siege {
        let rally = station + (pos - station).normalize_or(shared::glam::Vec3::Z) * (reach + RALLY);
        g.handle(c.conn, ClientMsg::Controls(steer(pos, rot, rally, 100.0, stats.turn)));
        return;
    }
    let layout = &g.systems[&here].layout;
    // Rocks outside an enemy station's batteries.
    let mut rocks: Vec<(f32, Vec3Key)> = layout.asteroids.iter().map(|a| (a.pos.distance(station), Vec3Key(a.pos, a.radius, a.id))).filter(|(d, _)| *d >= guarded).collect();
    rocks.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.2.cmp(&b.1.2)));
    if rocks.is_empty() {
        return;
    }
    // A miner cannot sell at an enemy's station: in enemy-held space it hunts instead.
    let role = if guarded > 0.0 { Role::Hunter } else { c.role };
    match role {
        Role::Hunter => {
            // Sweep the belts (pirates anchor at asteroids), a different rock every few minutes.
            let k = (c.patrol + (g.now / 240.0) as usize) % rocks.len().min(6);
            let Vec3Key(at, r, _) = rocks[k].1;
            g.handle(c.conn, ClientMsg::Controls(steer(pos, rot, at, r + stats.radius + 150.0, stats.turn)));
        }
        Role::Miner => {
            let full = prof.cargo_used() as f32 + 1.0 > stats.cargo;
            if full {
                if pos.distance(station) > stats.radius + 120.0 {
                    g.handle(c.conn, ClientMsg::Controls(steer(pos, rot, station, stats.radius + 60.0, stats.turn)));
                } else {
                    g.handle(c.conn, ClientMsg::Controls(Controls::default()));
                    g.handle(c.conn, ClientMsg::Dock);
                }
            } else {
                let Vec3Key(at, r, id) = rocks[c.patrol % rocks.len().min(3)].1;
                let reach = r + stats.radius + 60.0;
                if pos.distance(at) > reach + 80.0 {
                    g.handle(c.conn, ClientMsg::Controls(steer(pos, rot, at, reach, stats.turn)));
                } else {
                    g.handle(c.conn, ClientMsg::Controls(Controls::default()));
                    if !mining {
                        g.handle(c.conn, ClientMsg::Mine { asteroid: Some(id) });
                    }
                }
            }
        }
    }
}

/// An asteroid's position, radius and id (copied out so the layout borrow ends).
#[derive(Clone, Copy)]
struct Vec3Key(shared::glam::Vec3, f32, u32);

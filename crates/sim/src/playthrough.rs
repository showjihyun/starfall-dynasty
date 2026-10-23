//! Campaign playthrough (QA): a fresh captain plays the five campaign chapters the way a player
//! would — no dev commands, no teleports — and the run reports, per chapter, whether it was
//! finished, how long it took in game time, how often the captain died and where it got stuck.
//!
//! The pilot flies with the same controls a client sends (`ClientMsg` → `Game::handle`): it warps
//! hop by hop within its warp range, flies to sites and scans them, fights the waves (turning toward
//! the target and firing), docks to repair, sells what it mined and buys module upgrades. When a wave
//! beats it twice it goes home to earn credits (mining and selling ore) and upgrades before trying
//! again; when upgrades no longer help it reports the chapter as stuck.
//!
//! Deterministic like the rest of the simulation (`cargo run -p sim --release --example campaign`).

use crate::game::{COMBAT_LOCK, ConnId, Game};
use crate::store::Store;
use shared::galaxy::{self, Ore, PoiKind, Species, SystemId};
use shared::glam::Vec3;
use shared::missions::{Step, scenarios};
use shared::protocol::{ClientMsg, Controls, EntityId, EntityKind, PROTOCOL_VERSION, PlayerId, Profession, TICK_HZ};
use shared::ships::{HullClass, Materials, Module, tier_cap, upgrade_cost};

/// Who plays and for how long.
#[derive(Clone, Debug)]
pub struct Options {
    pub species: Species,
    pub profession: Profession,
    pub hull: HullClass,
    /// Give up after this much game time (seconds) in total.
    pub max_secs: f32,
    /// Give up on a chapter after this many deaths in it.
    pub max_deaths: u32,
    pub unix: u64,
}

impl Default for Options {
    fn default() -> Self {
        Options { species: Species::Human, profession: Profession::CombatPilot, hull: HullClass::Talon, max_secs: 10.0 * 3600.0, max_deaths: 20, unix: 1_760_000_000 }
    }
}

/// One chapter as played.
#[derive(Clone, Debug, Default)]
pub struct Chapter {
    pub id: u32,
    pub title: String,
    /// Game time (s) when the chapter was started and finished.
    pub started: f32,
    pub finished: Option<f32>,
    /// Deaths in the chapter's fights, and while earning upgrades between tries.
    pub deaths: u32,
    pub grind_deaths: u32,
    /// Seconds spent earning credits instead of playing the chapter.
    pub grind_secs: f32,
    pub upgrades: u32,
    /// Ship rank and module levels when it ended.
    pub rank: u8,
    pub levels: String,
    /// What happened, in order (step reached, deaths, retreats, stuck).
    pub log: Vec<String>,
}

#[derive(Clone, Debug, Default)]
pub struct Report {
    pub chapters: Vec<Chapter>,
    pub secs: f32,
    /// Why the run stopped before the end, if it did.
    pub stuck: Option<String>,
    pub credits: u64,
}

impl Report {
    pub fn completed(&self) -> usize {
        self.chapters.iter().filter(|c| c.finished.is_some()).count()
    }

    pub fn text(&self) -> String {
        let mut s = String::new();
        for c in &self.chapters {
            let took = c.finished.map(|f| f - c.started);
            s += &format!(
                "{} {}  {}  deaths {} (+{} earning)  grind {}  upgrades {}  rank {}  [{}]\n",
                if c.finished.is_some() { "PASS" } else { "FAIL" },
                c.title,
                took.map_or("not finished".to_string(), mins),
                c.deaths,
                c.grind_deaths,
                mins(c.grind_secs),
                c.upgrades,
                c.rank,
                c.levels
            );
            for l in &c.log {
                s += &format!("       {l}\n");
            }
        }
        s += &format!("campaign: {}/{} chapters in {} of game time, {} CR left\n", self.completed(), scenarios().len(), mins(self.secs), self.credits);
        if let Some(why) = &self.stuck {
            s += &format!("STUCK: {why}\n");
        }
        s
    }
}

fn mins(secs: f32) -> String {
    format!("{}m{:02}s", (secs / 60.0) as u32, (secs % 60.0) as u32)
}

/// What the pilot is doing between chapter steps.
#[derive(Clone, Debug, PartialEq)]
enum Mode {
    Play,
    /// Mine, refine and sell until the wallet covers this (the next upgrades).
    Grind { goal: Materials },
}

struct Pilot {
    conn: ConnId,
    pid: PlayerId,
    mode: Mode,
    /// Deaths on the current step (reset when the step changes).
    step_deaths: u32,
    step: Option<(u32, usize)>,
    upgrades_at_last_retreat: u32,
    upgrades: u32,
    missile_cd: f32,
    /// Who last shot at us, and where (for the death log).
    last_attacker: String,
    /// Where we are, and the systems we died in while earning (no longer mined).
    at: SystemId,
    avoid: std::collections::BTreeSet<SystemId>,
}

const THINK: u64 = 5;

/// Plays the campaign from character creation to the end of chapter 5 (or until stuck).
pub fn play(o: &Options) -> Report {
    let mut g = Game::new(Store::memory(), history::HistoryEngine::new(), o.unix);
    let conn: ConnId = 1;
    g.handle(conn, ClientMsg::Hello { version: PROTOCOL_VERSION, name: "QA함장".into(), token: None });
    g.handle(conn, ClientMsg::CreateCharacter { species: o.species, profession: o.profession, hull: o.hull, ship_name: "시험호".into(), livery: 0 });
    g.out.clear();
    let mut rep = Report::default();
    let Some(pid) = g.sessions.get(&conn).and_then(|s| s.player) else {
        rep.stuck = Some("character could not be created".into());
        return rep;
    };
    let mut p = Pilot { conn, pid, mode: Mode::Play, step_deaths: 0, step: None, upgrades_at_last_retreat: 0, upgrades: 0, missile_cd: 0.0, last_attacker: String::new(), at: 0, avoid: Default::default() };
    let dt = 1.0 / TICK_HZ;
    let total = (o.max_secs * TICK_HZ) as u64;
    let mut was_dead = false;
    for tick in 0..total {
        let t = tick as f32 / TICK_HZ;
        if tick % THINK == 0 {
            let dead = g.sessions[&conn].dead;
            if dead && !was_dead {
                let grinding = matches!(p.mode, Mode::Grind { .. });
                if !grinding {
                    p.step_deaths += 1;
                }
                if let Some(c) = rep.chapters.last_mut().filter(|c| c.finished.is_none()) {
                    if grinding {
                        p.avoid.insert(p.at);
                        c.grind_deaths += 1;
                    } else {
                        c.deaths += 1;
                    }
                    c.log.push(format!("{} died {}to {} ({})", mins(t), if grinding { "while earning " } else { "" }, p.last_attacker, step_text(&g, pid)));
                }
            }
            was_dead = dead;
            if let Some(why) = p.think(&mut g, &mut rep, t, o) {
                rep.stuck = Some(why);
                break;
            }
            if rep.chapters.len() == scenarios().len() && rep.chapters.iter().all(|c| c.finished.is_some()) {
                break;
            }
        }
        if let (Mode::Grind { .. }, Some(c)) = (&p.mode, rep.chapters.last_mut()) {
            c.grind_secs += dt;
        }
        g.tick(dt, o.unix + tick / TICK_HZ as u64);
        g.out.clear();
        g.kicked.clear();
        rep.secs = t + dt;
    }
    if rep.stuck.is_none() && rep.completed() < scenarios().len() {
        rep.stuck = Some(format!("out of time after {} ({})", mins(rep.secs), step_text(&g, pid)));
    }
    rep.credits = g.store.profile(pid).map(|p| p.wallet.credits).unwrap_or(0);
    let (rank, levels) = ship_summary(&g, pid);
    if let Some(c) = rep.chapters.last_mut()
        && c.finished.is_none()
    {
        (c.rank, c.levels) = (rank, levels);
    }
    rep
}

fn step_text(g: &Game, pid: PlayerId) -> String {
    let Some(sp) = g.store.profile(pid).and_then(|p| p.scenario.clone()) else { return "between chapters".into() };
    let step = scenarios().into_iter().find(|s| s.id == sp.id).and_then(|s| s.steps.get(sp.step).cloned());
    step.map_or("?".into(), |s| format!("step {}: {}", sp.step + 1, s.describe()))
}

fn ship_summary(g: &Game, pid: PlayerId) -> (u8, String) {
    let Some(p) = g.store.profile(pid) else { return (0, String::new()) };
    let levels = Module::ALL.iter().map(|m| format!("{}{}", short(*m), p.ship.level(*m))).collect::<Vec<_>>().join(" ");
    (p.ship.rank(), levels)
}

fn short(m: Module) -> &'static str {
    match m {
        Module::Hull => "H",
        Module::Reactor => "R",
        Module::Engine => "E",
        Module::WarpCore => "W",
        Module::Shield => "S",
        Module::Weapons => "G",
        Module::Scanner => "Sc",
        Module::Cargo => "C",
    }
}

/// Fewest-hop route to `to` using jumps no longer than `range` light-years: the next system to warp to.
fn next_hop(from: SystemId, to: SystemId, range: f32) -> Option<SystemId> {
    let n = galaxy::systems().len();
    let mut prev: Vec<Option<SystemId>> = vec![None; n];
    let mut seen = vec![false; n];
    let mut queue = std::collections::VecDeque::from([from]);
    seen[from as usize] = true;
    while let Some(s) = queue.pop_front() {
        if s == to {
            let mut at = to;
            while let Some(p) = prev[at as usize] {
                if p == from {
                    return Some(at);
                }
                at = p;
            }
            return None;
        }
        for d in galaxy::systems() {
            if !seen[d.id as usize] && galaxy::distance_ly(s, d.id) <= range + 1e-3 {
                seen[d.id as usize] = true;
                prev[d.id as usize] = Some(s);
                queue.push_back(d.id);
            }
        }
    }
    None
}

/// Controls that turn the nose toward `goal` and fly there, stopping `stop` short of it.
fn steer(pos: Vec3, rot: shared::glam::Quat, goal: Vec3, stop: f32) -> Controls {
    let to = goal - pos;
    let local = rot.inverse() * to.normalize_or_zero();
    let mut c = Controls { yaw: (local.x * 4.0).clamp(-1.0, 1.0), pitch: (local.y * 4.0).clamp(-1.0, 1.0), ..Default::default() };
    if local.z > 0.0 {
        c.yaw = if local.x >= 0.0 { 1.0 } else { -1.0 };
    }
    let d = to.length();
    // Facing it: full ahead while far, easing off near the stop distance (flight assist brakes).
    let facing = -local.z;
    c.thrust = if d > stop { ((d - stop) / 300.0).clamp(0.2, 1.0) * facing.max(0.2) } else { 0.0 };
    c.boost = d > stop + 2500.0 && facing > 0.9;
    c
}

impl Pilot {
    fn send(&self, g: &mut Game, m: ClientMsg) {
        g.handle(self.conn, m);
    }

    /// One decision. Returns why the run must stop, if it must.
    fn think(&mut self, g: &mut Game, rep: &mut Report, t: f32, o: &Options) -> Option<String> {
        let pid = self.pid;
        let prof = g.store.profile(pid)?.clone();
        let sess = &g.sessions[&self.conn];
        if sess.dead {
            self.send(g, ClientMsg::Respawn);
            return None;
        }
        let (sys, Some(eid)) = (sess.system, sess.entity) else { return None };
        self.at = sys;
        let (warping, scanning, wave) = (sess.warp.is_some(), sess.scan.is_some(), sess.wave.clone());
        let me = g.ent(sys, eid)?;
        let (pos, rot, docked, stats, hull_frac, last_hit) = (me.pos, me.rot, me.docked, me.stats, me.hull / me.stats.max_hull.max(1.0), me.last_hit);
        let attacker = me.attackers.iter().rev().find(|(a, _, at)| g.now - at < 8.0 && g.ent(sys, *a).is_some_and(|e| !e.dead)).map(|(a, _, _)| *a);
        if let Some(a) = attacker.and_then(|a| g.ent(sys, a)) {
            let npcs = g.systems[&sys].ents.values().filter(|e| !e.dead && matches!(e.kind, EntityKind::Npc(_))).count();
            self.last_attacker = format!("{} in system {sys} ({npcs} pirates there)", a.name);
        }
        let in_combat = g.now - last_hit < COMBAT_LOCK;

        // Chapter bookkeeping: a new chapter starts, a step changes, a chapter ends.
        let current = prof.scenario.clone().map(|s| (s.id, s.step));
        if current != self.step {
            if let Some((id, step)) = current {
                if rep.chapters.iter().all(|c| c.id != id) {
                    let sc = &scenarios()[id as usize];
                    rep.chapters.push(Chapter { id, title: sc.title.to_string(), started: t, ..Default::default() });
                }
                if let Some(c) = rep.chapters.iter_mut().find(|c| c.id == id) {
                    c.log.push(format!("{} {}", mins(t), step_text(g, pid)));
                }
                let _ = step;
            }
            if let Some((id, _)) = self.step
                && current.is_none_or(|(c, _)| c != id)
                && prof.scenarios_completed > id
                && let Some(c) = rep.chapters.iter_mut().find(|c| c.id == id)
            {
                c.finished = Some(t);
                (c.rank, c.levels) = ship_summary(g, pid);
            }
            self.step = current;
            self.step_deaths = 0;
        }
        if let Some(c) = rep.chapters.last()
            && c.finished.is_none()
            && c.deaths >= o.max_deaths
        {
            return Some(format!("{}: {} deaths ({})", c.title, c.deaths, step_text(g, pid)));
        }

        // Losing a fight twice: go home, earn credits and upgrade before the next try.
        let cap = tier_cap(prof.scenarios_completed);
        if self.mode == Mode::Play && self.step_deaths >= 2 {
            let room = Module::ALL.iter().any(|m| prof.ship.level(*m) < cap);
            if !room {
                return Some(format!("{}: still losing with every module at the level cap Lv{cap} ({})", rep.chapters.last().map_or("", |c| c.title.as_str()), step_text(g, pid)));
            }
            let want = self.next_upgrades_cost(&prof, cap, 3);
            self.upgrades_at_last_retreat = self.upgrades;
            self.step_deaths = 0;
            if let Some(c) = rep.chapters.last_mut() {
                c.log.push(format!("{} retreats to earn the next 3 upgrades: {} (has {})", mins(t), want.describe(), prof.wallet.describe()));
            }
            if want.relic > prof.wallet.relic {
                return Some(format!("{}: the next upgrades need {} relics, which mining cannot earn ({})", rep.chapters.last().map_or("", |c| c.title.as_str()), want.relic, step_text(g, pid)));
            }
            self.mode = Mode::Grind { goal: want };
        }

        // Docked: repair happened on docking; refine what the upgrades need, sell the rest, upgrade, go.
        if docked {
            let goal = match &self.mode {
                Mode::Grind { goal } => goal.clone(),
                Mode::Play => self.next_upgrades_cost(&prof, cap, 3),
            };
            for ore in Ore::ALL {
                let have = prof.cargo.get(ore.name()).copied().unwrap_or(0);
                let (ratio, short) = refine_need(ore, &goal, &g.store.profile(pid)?.wallet);
                let refine = (short * ratio).min(have / ratio * ratio);
                if refine > 0 {
                    self.send(g, ClientMsg::Refine { ore, units: refine });
                }
                let left = g.store.profile(pid)?.cargo.get(ore.name()).copied().unwrap_or(0);
                if left > 0 {
                    self.send(g, ClientMsg::Sell { ore, units: left });
                }
            }
            self.buy_upgrades(g, rep, cap);
            if let Mode::Grind { goal } = &self.mode
                && (g.store.profile(pid)?.wallet.covers(goal) || self.upgrades >= self.upgrades_at_last_retreat + 3)
            {
                self.mode = Mode::Play;
                if let Some(c) = rep.chapters.last_mut() {
                    c.log.push(format!("{} back to the chapter ({})", mins(t), ship_summary(g, pid).1));
                }
            }
            self.send(g, ClientMsg::Undock);
            return None;
        }
        if warping {
            self.send(g, ClientMsg::Controls(Controls::default()));
            return None;
        }

        // Someone shooting at us outside a planned fight: answer.
        let fight = |p: &mut Pilot, g: &mut Game, target: EntityId| {
            let Some(tp) = g.ent(sys, target).map(|e| e.pos) else { return };
            g.handle(p.conn, ClientMsg::SetTarget(Some(target)));
            let mut c = steer(pos, rot, tp, stats.weapon_range * 0.55);
            let local = rot.inverse() * (tp - pos).normalize_or_zero();
            c.fire = -local.z > 0.9 && pos.distance(tp) < stats.weapon_range * 1.05;
            g.handle(p.conn, ClientMsg::Controls(c));
            if p.missile_cd <= 0.0 && c.fire {
                g.handle(p.conn, ClientMsg::FireMissile);
                p.missile_cd = 3.0;
            }
            p.missile_cd -= THINK as f32 / TICK_HZ;
        };

        match self.mode.clone() {
            Mode::Grind { goal } => {
                if let Some(a) = attacker {
                    fight(self, g, a);
                    return None;
                }
                // Mine the ore the upgrades are shortest of, in the system fewest warps from home that
                // has it; when only credits are short, the ore that sells best within two warps of
                // home (prices fall as a station buys). Fill the hold, dock there.
                let short = |o: &Ore| refine_need(*o, &goal, &prof.wallet).1;
                let (at, ore) = match Ore::ALL.into_iter().filter(|o| short(o) > 0).max_by_key(short) {
                    Some(o) => (mining_system(prof.home, Some(o), stats.warp_range, &self.avoid), Some(o)),
                    None => best_market(g, prof.home, stats.warp_range, prof.profession == Profession::Trader, prof.scenarios_completed, &self.avoid),
                };
                if sys != at {
                    return self.travel(g, sys, at, &stats, in_combat);
                }
                // Sites not scanned today pay credits and materials: scan them before mining.
                if scanning {
                    self.send(g, ClientMsg::Controls(Controls::default()));
                    return None;
                }
                let fresh = g.systems[&sys].layout.pois.iter().filter(|p| !prof.scanned.contains(&(sys, p.id))).min_by(|a, b| a.pos.distance(pos).total_cmp(&b.pos.distance(pos))).map(|p| (p.id, p.pos));
                if let Some((poi, at)) = fresh {
                    if pos.distance(at) > stats.scan_range * 0.7 {
                        self.send(g, ClientMsg::Controls(steer(pos, rot, at, stats.scan_range * 0.5)));
                    } else {
                        self.send(g, ClientMsg::Controls(Controls::default()));
                        self.send(g, ClientMsg::Scan { poi });
                    }
                    return None;
                }
                let layout = &g.systems[&sys].layout;
                let station = layout.station;
                let used: u32 = prof.cargo.values().sum();
                // Whole units: full once another one would not fit.
                if used as f32 + 1.0 > stats.cargo {
                    return self.dock(g, pos, rot, station, stats.radius);
                }
                let a = layout.asteroids.iter().filter(|a| ore.is_none_or(|o| a.ore == o)).min_by(|a, b| a.pos.distance(station).total_cmp(&b.pos.distance(station))).cloned()?;
                let reach = a.radius + stats.radius + 60.0;
                if pos.distance(a.pos) > reach + 80.0 {
                    self.send(g, ClientMsg::Controls(steer(pos, rot, a.pos, reach)));
                } else {
                    self.send(g, ClientMsg::Controls(Controls::default()));
                    if g.ent(sys, eid).is_some_and(|e| e.mining.is_none()) {
                        self.send(g, ClientMsg::Mine { asteroid: Some(a.id) });
                    }
                }
                None
            }
            Mode::Play => {
                let Some(step) = g_step(g, pid) else {
                    // Between chapters: spend the reward first, then start the next one.
                    if prof.scenarios_completed as usize >= scenarios().len() {
                        return None;
                    }
                    if self.affordable_upgrade(&prof, cap).is_some() && !in_combat {
                        let station = g.systems[&sys].layout.station;
                        return self.dock(g, pos, rot, station, stats.radius);
                    }
                    self.send(g, ClientMsg::StartScenario { id: prof.scenarios_completed });
                    return None;
                };
                let target_sys = match &step {
                    Step::Reach { system } | Step::ScanPoi { system, .. } | Step::Defeat { system, .. } => *system,
                };
                if sys != target_sys {
                    if let Some(a) = attacker {
                        fight(self, g, a);
                        return None;
                    }
                    return self.travel(g, sys, target_sys, &stats, in_combat);
                }
                match step {
                    Step::Reach { .. } => None,
                    Step::ScanPoi { kind, .. } => {
                        if let Some(a) = attacker.filter(|_| hull_frac < 0.9 || !scanning) {
                            fight(self, g, a);
                            return None;
                        }
                        if scanning {
                            self.send(g, ClientMsg::Controls(Controls::default()));
                            return None;
                        }
                        let Some(poi) = pick_poi(g, sys, kind, &prof.scanned, pos) else { return Some(format!("no {kind:?} site in system {sys}")) };
                        if pos.distance(poi.1) > stats.scan_range * 0.7 {
                            self.send(g, ClientMsg::Controls(steer(pos, rot, poi.1, stats.scan_range * 0.5)));
                        } else {
                            self.send(g, ClientMsg::Controls(Controls::default()));
                            self.send(g, ClientMsg::Scan { poi: poi.0 });
                        }
                        None
                    }
                    Step::Defeat { .. } => {
                        if !self.escorts_attacking(g) {
                            self.send(g, ClientMsg::EscortCommand { attack: true });
                        }
                        let target = wave.iter().filter_map(|w| g.ent(sys, *w).filter(|e| !e.dead).map(|e| (e.id, e.pos.distance(pos)))).min_by(|a, b| a.1.total_cmp(&b.1)).map(|(id, _)| id).or(attacker);
                        match target {
                            Some(tid) => fight(self, g, tid),
                            // The wave spawns on arrival; after a death it is waiting where we left it.
                            None => self.send(g, ClientMsg::Controls(Controls::default())),
                        }
                        None
                    }
                }
            }
        }
    }

    fn escorts_attacking(&self, g: &Game) -> bool {
        g.sessions[&self.conn].escort_attack
    }

    fn travel(&mut self, g: &mut Game, sys: SystemId, to: SystemId, stats: &shared::ships::Stats, in_combat: bool) -> Option<String> {
        self.send(g, ClientMsg::Controls(Controls::default()));
        if in_combat {
            return None;
        }
        match next_hop(sys, to, stats.warp_range) {
            Some(hop) => {
                self.send(g, ClientMsg::Warp { system: hop });
                None
            }
            None => {
                // Out of range: a warp core upgrade is needed; earn it at home.
                let prof = g.store.profile(self.pid)?.clone();
                let cap = tier_cap(prof.scenarios_completed);
                if prof.ship.level(Module::WarpCore) >= cap {
                    return Some(format!("system {to} is out of warp range ({:.1} ly) with the warp core at the level cap", stats.warp_range));
                }
                self.mode = Mode::Grind { goal: upgrade_cost(Module::WarpCore, prof.ship.level(Module::WarpCore)) };
                None
            }
        }
    }

    fn dock(&mut self, g: &mut Game, pos: Vec3, rot: shared::glam::Quat, station: Vec3, radius: f32) -> Option<String> {
        if pos.distance(station) > radius + 120.0 {
            self.send(g, ClientMsg::Controls(steer(pos, rot, station, radius + 60.0)));
        } else {
            self.send(g, ClientMsg::Controls(Controls::default()));
            self.send(g, ClientMsg::Dock);
        }
        None
    }

    /// Upgrade order: what wins fights first, then the rest; cheapest level first within that.
    fn order(prof: &shared::protocol::Profile, cap: u8) -> Vec<Module> {
        let weight = |m: Module| match m {
            Module::Weapons => 0,
            Module::Hull | Module::Shield => 1,
            Module::Reactor | Module::Engine => 2,
            Module::WarpCore => 3,
            Module::Scanner | Module::Cargo => 4,
        };
        let mut ms: Vec<Module> = Module::ALL.into_iter().filter(|m| prof.ship.level(*m) < cap).collect();
        ms.sort_by_key(|m| (prof.ship.level(*m), weight(*m)));
        ms
    }

    fn affordable_upgrade(&self, prof: &shared::protocol::Profile, cap: u8) -> Option<Module> {
        Self::order(prof, cap).into_iter().find(|m| prof.wallet.covers(&upgrade_cost(*m, prof.ship.level(*m))))
    }

    /// What the next `n` upgrades in order cost together.
    fn next_upgrades_cost(&self, prof: &shared::protocol::Profile, cap: u8, n: usize) -> Materials {
        let mut sum = Materials::default();
        for m in Self::order(prof, cap).into_iter().take(n) {
            let c = upgrade_cost(m, prof.ship.level(m));
            sum.credits += c.credits;
            sum.alloy += c.alloy;
            sum.crystal += c.crystal;
            sum.datacore += c.datacore;
            sum.relic += c.relic;
        }
        sum
    }

    fn buy_upgrades(&mut self, g: &mut Game, rep: &mut Report, cap: u8) {
        for _ in 0..40 {
            let Some(prof) = g.store.profile(self.pid).cloned() else { return };
            let Some(m) = self.affordable_upgrade(&prof, cap) else { return };
            let before = prof.ship.level(m);
            self.send(g, ClientMsg::Upgrade { module: m });
            if g.store.profile(self.pid).is_none_or(|p| p.ship.level(m) == before) {
                return;
            }
            self.upgrades += 1;
            if let Some(c) = rep.chapters.last_mut() {
                c.upgrades += 1;
            }
        }
    }
}

/// Ore units per refined unit, and how many refined units of this ore's material `goal` still lacks.
fn refine_need(ore: Ore, goal: &Materials, wallet: &Materials) -> (u32, u32) {
    match ore {
        Ore::Ferrite => (5, goal.alloy.saturating_sub(wallet.alloy)),
        Ore::Lumen => (4, goal.crystal.saturating_sub(wallet.crystal)),
        Ore::Xenite => (2, goal.datacore.saturating_sub(wallet.datacore)),
    }
}

/// Where to mine `ore` (any ore: home): home if it has an asteroid of it, else the system that has
/// one fewest warps from home.
fn mining_system(home: SystemId, ore: Option<Ore>, range: f32, avoid: &std::collections::BTreeSet<SystemId>) -> SystemId {
    let Some(ore) = ore else { return home };
    let has = |s: SystemId| !avoid.contains(&s) && galaxy::system_layout(s).asteroids.iter().any(|a| a.ore == ore);
    if has(home) {
        return home;
    }
    let hops = |to: SystemId| {
        let (mut at, mut n) = (home, 0);
        while at != to && n < 12 {
            at = next_hop(at, to, range)?;
            n += 1;
        }
        (at == to).then_some(n)
    };
    galaxy::systems().iter().filter(|d| has(d.id)).filter_map(|d| hops(d.id).map(|n| (n, d.id))).min().map_or(home, |b| b.1)
}

/// The (system, ore) worth the most per unit among home and the systems at most two warps away
/// whose pirates are no stronger than the chapters cleared (a new captain mines in safe space).
fn best_market(g: &Game, home: SystemId, range: f32, trader: bool, cleared: u32, avoid: &std::collections::BTreeSet<SystemId>) -> (SystemId, Option<Ore>) {
    let near = |s: SystemId| s == home || next_hop(home, s, range).is_some_and(|h| h == s || galaxy::distance_ly(h, s) <= range + 1e-3);
    let safe = |d: &&galaxy::SystemDef| d.id == home || (d.tier.pirate_level() as u32 <= cleared && !avoid.contains(&d.id));
    let mut best: Option<(f32, SystemId, Ore)> = None;
    for d in galaxy::systems().iter().filter(|d| near(d.id)).filter(safe) {
        for a in &galaxy::system_layout(d.id).asteroids {
            let v = g.price(d.id, a.ore, trader);
            if best.is_none_or(|b| v > b.0) {
                best = Some((v, d.id, a.ore));
            }
        }
    }
    best.map_or((home, None), |b| (b.1, Some(b.2)))
}

fn g_step(g: &Game, pid: PlayerId) -> Option<Step> {
    let sp = g.store.profile(pid)?.scenario.clone()?;
    scenarios().into_iter().find(|s| s.id == sp.id)?.steps.get(sp.step).cloned()
}

/// The nearest site of `kind`, preferring ones not scanned yet.
fn pick_poi(g: &Game, sys: SystemId, kind: PoiKind, scanned: &[(SystemId, u32)], pos: Vec3) -> Option<(u32, Vec3)> {
    let pois = &g.systems[&sys].layout.pois;
    let near = |fresh: bool| pois.iter().filter(|p| p.kind == kind && (!fresh || !scanned.contains(&(sys, p.id)))).min_by(|a, b| a.pos.distance(pos).total_cmp(&b.pos.distance(pos))).map(|p| (p.id, p.pos));
    near(true).or_else(|| near(false))
}

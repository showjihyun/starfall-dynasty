//! Data-driven gameplay specs, run headless and deterministically (docs/HARNESS.md, "결정론 개발 구조").
//!
//! A scenario is a JSON file: who is where (captains with hull/levels/positions, NPC groups), which
//! commands arrive when (any `ClientMsg`, as JSON), how long to run, and what must be true at given
//! moments. The runner builds the simulation without network or engine, plays it at the fixed tick
//! and reports every check with the measured value — so a new gameplay test or a balance probe is a
//! data file, not Rust code, and the same file always gives the same result.
//!
//! `cargo run -p sim --release --example scenario -- crates/sim/scenarios [--trace 5]`

use crate::game::{ConnId, Game};
use crate::metrics::{MetricsLog, Totals};
use crate::store::Store;
use serde::{Deserialize, Serialize};
use shared::galaxy::{self as galaxy, Species, SystemId};
use shared::glam::{Quat, Vec3};
use shared::missions::NpcArchetype;
use shared::protocol::*;
use shared::ships::{HullClass, Module};

fn default_unix() -> u64 {
    1_800_000_000
}
fn yes() -> bool {
    true
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Scenario {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_unix")]
    pub unix: u64,
    /// Lets `/grow`-style dev commands through, like `server --dev`.
    #[serde(default)]
    pub dev: bool,
    /// Background pirate spawning in Frontier and wilder systems. Off by default so duels stay clean.
    #[serde(default)]
    pub ambient_pirates: bool,
    #[serde(default)]
    pub captains: Vec<Captain>,
    #[serde(default)]
    pub npcs: Vec<NpcGroup>,
    #[serde(default)]
    pub script: Vec<Step>,
    pub run_secs: f32,
    #[serde(default)]
    pub expect: Vec<Expect>,
    /// Optional golden fingerprint of the final state: any rule change that alters the outcome shows up.
    #[serde(default)]
    pub digest: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Captain {
    pub name: String,
    pub hull: HullClass,
    #[serde(default = "human")]
    pub species: Species,
    #[serde(default = "pilot")]
    pub profession: Profession,
    /// Every module at this level (1 = fresh fighter, 5 = cruiser, 9 = capital).
    #[serde(default = "one")]
    pub level: u8,
    pub system: SystemId,
    /// World position; defaults to the system's spawn point.
    #[serde(default)]
    pub pos: Option<[f32; 3]>,
    /// Place the captain at the system's first point of interest of this kind (e.g. `"Ruin"`)…
    #[serde(default)]
    pub near_poi: Option<shared::galaxy::PoiKind>,
    /// Start next to the asteroid nearest the station whose deposit is at least this rare.
    #[serde(default)]
    pub near_deposit: Option<shared::minerals::Rarity>,
    /// …plus this offset (metres).
    #[serde(default)]
    pub offset: Option<[f32; 3]>,
    /// Yaw in degrees (0 = facing -Z).
    #[serde(default)]
    pub heading: f32,
    #[serde(default = "yes")]
    pub undocked: bool,
    /// Credits on top of the starting wallet.
    #[serde(default)]
    pub credits: u64,
    /// Cargo in the hold at the start (ore name → units).
    #[serde(default)]
    pub cargo: std::collections::BTreeMap<String, u32>,
}

fn human() -> Species {
    Species::Human
}
fn pilot() -> Profession {
    Profession::CombatPilot
}
fn one() -> u8 {
    1
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NpcGroup {
    pub archetype: NpcArchetype,
    #[serde(default = "one")]
    pub level: u8,
    pub system: SystemId,
    pub pos: [f32; 3],
    #[serde(default = "one_u")]
    pub count: u32,
    #[serde(default = "spacing")]
    pub spacing: f32,
}

fn one_u() -> u32 {
    1
}
fn spacing() -> f32 {
    60.0
}

/// A command at a time. `captain` names who sends it; `repeat_every` resends it periodically
/// (e.g. held controls) until `until`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Step {
    pub at: f32,
    pub captain: String,
    pub msg: ClientMsg,
    #[serde(default)]
    pub repeat_every: Option<f32>,
    #[serde(default)]
    pub until: Option<f32>,
    /// Target the nearest entity of this kind before sending (`"npc"`, `"player"`).
    #[serde(default)]
    pub target_nearest: Option<String>,
    /// For `Controls`: steer yaw/pitch toward the current target, as a pilot would (scripted aim).
    #[serde(default)]
    pub aim: bool,
    /// For `Controls`: roll so the current target sits level with the hull, where every turret battery
    /// reaches it (a capital captain's answer to an attacker diving from above or below).
    #[serde(default)]
    pub broadside: bool,
    /// For `Controls`: strafe and thrust to stay on the target's own up/down axis at the current range
    /// (the side it is already on), so a rolling capital keeps only half its batteries on you.
    #[serde(default)]
    pub hold_over: bool,
    /// Fill the message's id with the nearest `"wreck"` (`InvestigateWreck`), `"poi:<Kind>"`
    /// (`Scan`) or the newest open `"contract"` (`AcceptContract`, …), so a spec needs no ids.
    #[serde(default)]
    pub fill_nearest: Option<String>,
    /// Target this captain's flagship before sending; the step is skipped once it is down.
    #[serde(default)]
    pub target_captain: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Expect {
    /// Seconds into the run (default: the end).
    #[serde(default)]
    pub at: Option<f32>,
    #[serde(flatten)]
    pub check: Check,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "check", rename_all = "snake_case")]
pub enum Check {
    /// The captain's flagship is (not) in space.
    Alive { captain: String, #[serde(default = "yes")] alive: bool },
    /// Hull fraction 0..1 of the flagship (0 when destroyed).
    Hull { captain: String, #[serde(default)] min: Option<f32>, #[serde(default)] max: Option<f32> },
    /// Hull points lost so far (max hull − hull; the whole hull when destroyed).
    HullLost { captain: String, #[serde(default)] min: Option<f32>, #[serde(default)] max: Option<f32> },
    Credits { captain: String, #[serde(default)] min: Option<u64>, #[serde(default)] max: Option<u64> },
    Kills { captain: String, #[serde(default)] min: Option<u32>, #[serde(default)] max: Option<u32> },
    Deaths { captain: String, #[serde(default)] min: Option<u32>, #[serde(default)] max: Option<u32> },
    /// Ships of a kind in a system: kind = "npc" | "player" | "escort" | "wreck".
    Count { system: SystemId, kind: String, #[serde(default)] min: Option<u32>, #[serde(default)] max: Option<u32> },
    /// The captain is in this system.
    InSystem { captain: String, system: SystemId },
    /// Chronicle entries whose kind contains this text (case-insensitive, e.g. "bounty", "SHIP_DESTROYED").
    History { kind: String, #[serde(default)] min: Option<u32>, #[serde(default)] max: Option<u32> },
    /// An engagement is (not) running in the system.
    Engagement { system: SystemId, #[serde(default = "yes")] active: bool },
    /// Damage points (shield + hull) the captain's side (flagship and escorts) has dealt so far.
    Dealt { captain: String, #[serde(default)] min: Option<f32>, #[serde(default)] max: Option<f32> },
    /// Damage points the captain's ships have taken so far.
    Taken { captain: String, #[serde(default)] min: Option<f32>, #[serde(default)] max: Option<f32> },
    /// Average damage per second dealt, from the captain's first hit to the last.
    Dps { captain: String, #[serde(default)] min: Option<f32>, #[serde(default)] max: Option<f32> },
    /// Mean time-to-kill (s) of the captain's kills: first hit on the victim in the fight to its death.
    Ttk { captain: String, #[serde(default)] min: Option<f32>, #[serde(default)] max: Option<f32> },
    /// Share (0–1) of the captain's side's hits that came from above or below the target
    /// (|elevation| ≥ 20° off the system plane): how much the fight used height.
    Vertical { captain: String, #[serde(default)] min: Option<f32>, #[serde(default)] max: Option<f32> },
    /// Artifacts the captain holds.
    Artifacts { captain: String, #[serde(default)] min: Option<u32>, #[serde(default)] max: Option<u32> },
    /// Mineral specimens the captain holds.
    Specimens { captain: String, #[serde(default)] min: Option<u32>, #[serde(default)] max: Option<u32> },
    Reputation { captain: String, #[serde(default)] min: Option<i32>, #[serde(default)] max: Option<i32> },
    /// Influence of the alliance with this tag in a system.
    Influence { system: SystemId, tag: String, #[serde(default)] min: Option<f32>, #[serde(default)] max: Option<f32> },
    /// Who controls a system (`tag` omitted: nobody).
    Controller { system: SystemId, #[serde(default)] tag: Option<String> },
    /// Credits in the treasury of the alliance with this tag.
    Treasury { tag: String, #[serde(default)] min: Option<u64>, #[serde(default)] max: Option<u64> },
    /// Records (evidence) the captain holds.
    Archive { captain: String, #[serde(default)] min: Option<u32>, #[serde(default)] max: Option<u32> },
    /// An institute by name: how many exhibits and distinct visits it has.
    Institute {
        name: String,
        #[serde(default)]
        exhibits_min: Option<u32>,
        #[serde(default)]
        visitors_min: Option<u32>,
        #[serde(default)]
        visitors_max: Option<u32>,
    },
}

/// Outcome of one scenario.
#[derive(Debug, Default)]
pub struct Report {
    pub name: String,
    pub lines: Vec<String>,
    pub failed: usize,
    pub passed: usize,
    pub digest: String,
    /// Seconds until no NPC was left (None: never).
    pub npcs_cleared_at: Option<f32>,
    /// Seconds until each captain's flagship first went down.
    pub down_at: Vec<(String, f32)>,
    /// Flagship hull fraction of each captain at the end (0 when destroyed).
    pub final_hull: Vec<(String, f32)>,
    /// Hull points each captain's flagship lost by the end (its whole hull when destroyed).
    pub final_hull_lost: Vec<(String, f32)>,
    /// Gameplay metrics as JSONL lines (`crate::metrics`): a sample per captain per second, kills, rewards.
    pub metrics: Vec<String>,
    /// Each captain's damage, kill and reward totals at the end.
    pub totals: Vec<(String, Totals)>,
}

impl Report {
    pub fn ok(&self) -> bool {
        self.failed == 0
    }
}

struct Run {
    g: Game,
    conns: Vec<(String, ConnId)>,
    log: MetricsLog,
}

impl Run {
    fn conn(&self, name: &str) -> Option<ConnId> {
        self.conns.iter().find(|(n, _)| n == name).map(|(_, c)| *c)
    }
    fn pid(&self, name: &str) -> Option<PlayerId> {
        self.g.store.find_by_name(name)
    }
    fn totals(&self, name: &str) -> Totals {
        self.pid(name).and_then(|p| self.log.totals.get(&p)).cloned().unwrap_or_default()
    }
    fn flagship(&self, name: &str) -> Option<&crate::game::Ent> {
        let (sys, eid) = self.g.player_entity(self.pid(name)?)?;
        self.g.ent(sys, eid).filter(|e| !e.dead)
    }
}

fn kind_matches(kind: &str, k: EntityKind) -> bool {
    match kind {
        "npc" => matches!(k, EntityKind::Npc(_)),
        "player" => k == EntityKind::Player,
        "escort" => k == EntityKind::Escort,
        "wreck" => k == EntityKind::Wreck,
        "missile" => k == EntityKind::Missile,
        _ => false,
    }
}

fn range<T: PartialOrd + Copy + std::fmt::Display>(v: T, min: Option<T>, max: Option<T>) -> (bool, String) {
    let ok = min.is_none_or(|m| v >= m) && max.is_none_or(|m| v <= m);
    let want = match (min, max) {
        (Some(a), Some(b)) => format!("{a}..={b}"),
        (Some(a), None) => format!(">= {a}"),
        (None, Some(b)) => format!("<= {b}"),
        (None, None) => "any".into(),
    };
    (ok, format!("{v} (want {want})"))
}

fn evaluate(run: &Run, c: &Check) -> (bool, String) {
    let g = &run.g;
    match c {
        Check::Alive { captain, alive } => {
            let is = run.flagship(captain).is_some();
            (is == *alive, format!("{captain} alive={is} (want {alive})"))
        }
        Check::Hull { captain, min, max } => {
            let h = run.flagship(captain).map(|e| e.hull / e.stats.max_hull.max(1.0)).unwrap_or(0.0);
            let (ok, s) = range((h * 1000.0).round() / 1000.0, *min, *max);
            (ok, format!("{captain} hull {s}"))
        }
        Check::HullLost { captain, min, max } => {
            let lost = run.flagship(captain).map(|e| e.stats.max_hull - e.hull).unwrap_or(f32::INFINITY);
            let (ok, s) = range((lost * 10.0).round() / 10.0, *min, *max);
            (ok, format!("{captain} hull lost {s}"))
        }
        Check::Credits { captain, min, max } => {
            let v = run.pid(captain).and_then(|p| g.store.profile(p)).map(|p| p.wallet.credits).unwrap_or(0);
            let (ok, s) = range(v, *min, *max);
            (ok, format!("{captain} credits {s}"))
        }
        Check::Kills { captain, min, max } => {
            let v = run.pid(captain).and_then(|p| g.store.profile(p)).map(|p| p.kills).unwrap_or(0);
            let (ok, s) = range(v, *min, *max);
            (ok, format!("{captain} kills {s}"))
        }
        Check::Deaths { captain, min, max } => {
            let v = run.pid(captain).and_then(|p| g.store.profile(p)).map(|p| p.deaths).unwrap_or(0);
            let (ok, s) = range(v, *min, *max);
            (ok, format!("{captain} deaths {s}"))
        }
        Check::Count { system, kind, min, max } => {
            let v = g.systems.get(system).map(|s| s.ents.values().filter(|e| !e.dead && kind_matches(kind, e.kind)).count() as u32).unwrap_or(0);
            let (ok, s) = range(v, *min, *max);
            (ok, format!("system {system} {kind} count {s}"))
        }
        Check::InSystem { captain, system } => {
            let at = run.pid(captain).and_then(|p| g.player_entity(p)).map(|(s, _)| s);
            (at == Some(*system), format!("{captain} in system {at:?} (want {system})"))
        }
        Check::History { kind, min, max } => {
            let k = kind.to_lowercase();
            let v = g.history.events.iter().filter(|e| e.kind.to_lowercase().contains(&k)).count() as u32;
            let (ok, s) = range(v, *min, *max);
            (ok, format!("history '{kind}' entries {s}"))
        }
        Check::Engagement { system, active } => {
            let is = g.systems.get(system).is_some_and(|s| s.engagement.is_some());
            (is == *active, format!("system {system} engagement={is} (want {active})"))
        }
        Check::Dealt { captain, min, max } => {
            let (ok, s) = range((run.totals(captain).dealt * 10.0).round() / 10.0, *min, *max);
            (ok, format!("{captain} damage dealt {s}"))
        }
        Check::Taken { captain, min, max } => {
            let (ok, s) = range((run.totals(captain).taken * 10.0).round() / 10.0, *min, *max);
            (ok, format!("{captain} damage taken {s}"))
        }
        Check::Dps { captain, min, max } => {
            let (ok, s) = range((run.totals(captain).dps() * 10.0).round() / 10.0, *min, *max);
            (ok, format!("{captain} dps {s}"))
        }
        Check::Influence { system, tag, min, max } => {
            let v = g.store.data.alliances.iter().find(|a| a.tag == *tag).map(|a| g.influence(*system, a.id)).unwrap_or(0.0);
            let (ok, s) = range((v * 10.0).round() / 10.0, *min, *max);
            (ok, format!("system {system} influence of [{tag}] {s}"))
        }
        Check::Archive { captain, min, max } => {
            let v = run.pid(captain).and_then(|p| g.store.profile(p)).map(|p| p.archive.len() as u32).unwrap_or(0);
            let (ok, s) = range(v, *min, *max);
            (ok, format!("{captain} archive {s}"))
        }
        Check::Institute { name, exhibits_min, visitors_min, visitors_max } => match g.institute_by_name(name) {
            Some(i) => {
                let ex = i.exhibits.len() as u32;
                let ok = exhibits_min.is_none_or(|m| ex >= m) && visitors_min.is_none_or(|m| i.visitors >= m) && visitors_max.is_none_or(|m| i.visitors <= m);
                (ok, format!("institute 「{name}」 exhibits {ex}, visitors {} (want exhibits >= {exhibits_min:?}, visitors {visitors_min:?}..={visitors_max:?})", i.visitors))
            }
            None => (false, format!("institute 「{name}」 does not exist")),
        },
        Check::Treasury { tag, min, max } => {
            let v = g.store.data.alliances.iter().find(|a| a.tag == *tag).map(|a| a.treasury).unwrap_or(0);
            let (ok, s) = range(v, *min, *max);
            (ok, format!("[{tag}] treasury {s}"))
        }
        Check::Controller { system, tag } => {
            let is = g.controller(*system).and_then(|a| g.alliance(a)).map(|a| a.tag.clone());
            (is == *tag, format!("system {system} controlled by {is:?} (want {tag:?})"))
        }
        Check::Reputation { captain, min, max } => {
            let v = run.pid(captain).and_then(|p| g.store.profile(p)).map(|p| p.reputation).unwrap_or(0);
            let (ok, s) = range(v, *min, *max);
            (ok, format!("{captain} reputation {s}"))
        }
        Check::Specimens { captain, min, max } => {
            let v = run.pid(captain).and_then(|p| g.store.profile(p)).map(|p| p.specimens.values().sum::<u32>()).unwrap_or(0);
            let (ok, s) = range(v, *min, *max);
            (ok, format!("{captain} specimens {s}"))
        }
        Check::Artifacts { captain, min, max } => {
            let v = run.pid(captain).map(|p| g.artifacts_of(p).len() as u32).unwrap_or(0);
            let (ok, s) = range(v, *min, *max);
            (ok, format!("{captain} artifacts {s}"))
        }
        Check::Vertical { captain, min, max } => match run.totals(captain).high_angle_share() {
            Some(v) => {
                let (ok, s) = range((v * 100.0).round() / 100.0, *min, *max);
                (ok, format!("{captain} hits from above/below {s}"))
            }
            None => (false, format!("{captain} vertical: no hits")),
        },
        Check::Ttk { captain, min, max } => match run.totals(captain).mean_ttk() {
            Some(v) => {
                let (ok, s) = range((v * 100.0).round() / 100.0, *min, *max);
                (ok, format!("{captain} mean time-to-kill {s} s"))
            }
            None => (false, format!("{captain} mean time-to-kill: no kills")),
        },
    }
}

/// One-line state of every captain, for `--trace`.
fn trace_line(run: &Run, t: f32) -> String {
    let mut s = format!("  t={t:>6.1}s");
    for (name, _) in &run.conns {
        match run.flagship(name) {
            Some(e) => s += &format!(" | {name}: hull {:.0}% shield {:.0}%", e.hull / e.stats.max_hull.max(1.0) * 100.0, e.shield / e.stats.max_shield.max(1.0) * 100.0),
            None => s += &format!(" | {name}: down"),
        }
    }
    let npcs: usize = run.g.systems.values().map(|sm| sm.ents.values().filter(|e| !e.dead && matches!(e.kind, EntityKind::Npc(_))).count()).sum();
    s += &format!(" | npcs {npcs}");
    s
}

/// Plays a scenario. `trace_every` (seconds) adds a captain status line at that interval.
pub fn run(sc: &Scenario, trace_every: Option<f32>) -> Report {
    run_opts(sc, trace_every, true)
}

/// `run` with gameplay metrics optional (they never change the outcome; `tests/metrics.rs` checks that).
pub fn run_opts(sc: &Scenario, trace_every: Option<f32>, metrics: bool) -> Report {
    let mut g = Game::new(Store::memory(), history::HistoryEngine::new(), sc.unix);
    g.dev_mode = sc.dev;
    let mut rep = Report { name: sc.name.clone(), ..Default::default() };
    let mut run = Run { g, conns: vec![], log: MetricsLog::new(1.0, true) };

    for (i, c) in sc.captains.iter().enumerate() {
        let conn = 100 + i as ConnId;
        run.g.handle(conn, ClientMsg::Hello { version: PROTOCOL_VERSION, name: c.name.clone(), token: None });
        run.g.handle(conn, ClientMsg::CreateCharacter { species: c.species, profession: c.profession, hull: c.hull, ship_name: format!("{} 호", c.name), livery: (i % 7) as u8 });
        let Some(pid) = run.g.sessions.get(&conn).and_then(|s| s.player) else {
            rep.lines.push(format!("SETUP FAIL: captain {} could not be created", c.name));
            rep.failed += 1;
            return rep;
        };
        if let Some(p) = run.g.store.profile_mut(pid) {
            for m in Module::ALL {
                p.ship.modules.insert(m, c.level.max(1));
            }
            p.wallet.credits += c.credits;
            p.cargo.extend(c.cargo.clone());
        }
        run.g.spawn_player(pid, c.system, !c.undocked);
        if let Some((sys, eid)) = run.g.player_entity(pid)
            && let Some(e) = run.g.ent_mut(sys, eid)
        {
            if let Some(p) = c.pos {
                e.pos = Vec3::from(p);
            }
            if let Some(kind) = c.near_poi
                && let Some(poi) = galaxy::system_layout(c.system).pois.iter().find(|p| p.kind == kind)
            {
                e.pos = poi.pos + Vec3::from(c.offset.unwrap_or_default());
            }
            if let Some(min) = c.near_deposit {
                let layout = galaxy::system_layout(c.system);
                let rare = layout.asteroids.iter().filter(|a| shared::minerals::mineral(a.mineral).is_some_and(|m| m.rarity >= min)).min_by(|a, b| a.pos.distance(layout.station).total_cmp(&b.pos.distance(layout.station)));
                if let Some(a) = rare {
                    e.pos = a.pos + Vec3::new(0.0, 0.0, a.radius + 60.0) + Vec3::from(c.offset.unwrap_or_default());
                }
            }
            e.rot = Quat::from_rotation_y(-c.heading.to_radians());
        }
        // Escorts take formation around the (moved) flagship.
        run.g.spawn_escorts(pid);
        run.conns.push((c.name.clone(), conn));
    }
    for n in &sc.npcs {
        for k in 0..n.count {
            let off = Vec3::new((k as f32 - (n.count as f32 - 1.0) * 0.5) * n.spacing, 0.0, 0.0);
            run.g.spawn_npc(n.system, n.archetype, n.level, Vec3::from(n.pos) + off, None);
        }
    }
    if !sc.ambient_pirates {
        for s in run.g.systems.values_mut() {
            s.pirate_cd = f32::INFINITY;
        }
    }
    run.g.out.clear();
    if metrics {
        MetricsLog::attach(&mut run.g);
    }

    let dt = 1.0 / TICK_HZ;
    let total = (sc.run_secs * TICK_HZ).round() as u64;
    let mut checks: Vec<(u64, &Expect)> = sc.expect.iter().map(|e| (((e.at.unwrap_or(sc.run_secs)) * TICK_HZ).round() as u64, e)).collect();
    checks.sort_by_key(|c| c.0);
    let mut next_check = 0;
    let trace_ticks = trace_every.map(|s| (s * TICK_HZ).round().max(1.0) as u64);
    for tick in 0..=total {
        let t = tick as f32 / TICK_HZ;
        for st in &sc.script {
            let start = (st.at * TICK_HZ).round() as u64;
            let fire = match st.repeat_every {
                Some(every) => {
                    let every = (every * TICK_HZ).round().max(1.0) as u64;
                    tick >= start && (tick - start) % every == 0 && st.until.is_none_or(|u| t <= u)
                }
                None => tick == start,
            };
            if !fire {
                continue;
            }
            let Some(conn) = run.conn(&st.captain) else { continue };
            if let Some(name) = &st.target_captain {
                let Some(t) = run.flagship(name).map(|e| e.id) else { continue };
                run.g.handle(conn, ClientMsg::SetTarget(Some(t)));
            }
            if let Some(kind) = &st.target_nearest
                && let Some(me) = run.flagship(&st.captain).map(|e| (e.pos, e.id))
                && let Some((sys, _)) = run.pid(&st.captain).and_then(|p| run.g.player_entity(p))
            {
                let best = run.g.systems[&sys].ents.values().filter(|e| !e.dead && e.id != me.1 && kind_matches(kind, e.kind)).min_by(|a, b| a.pos.distance(me.0).total_cmp(&b.pos.distance(me.0))).map(|e| e.id);
                run.g.handle(conn, ClientMsg::SetTarget(best));
            }
            let mut msg = st.msg.clone();
            for what in st.fill_nearest.iter().flat_map(|w| w.split(',')).map(str::trim).collect::<Vec<_>>() {
                let Some(me) = run.flagship(&st.captain).map(|e| e.pos) else { continue };
                let Some((pid, sys)) = run.pid(&st.captain).and_then(|p| run.g.player_entity(p).map(|(s, _)| (p, s))) else { continue };
                let dist = |p: Vec3| p.distance(me);
                match (&mut msg, what.strip_prefix("poi:")) {
                    (ClientMsg::LeakEvidence { evidence }, None) if what == "evidence" => {
                        if let Some(e) = run.g.store.profile(pid).and_then(|p| p.archive.last().copied()) {
                            *evidence = e;
                        }
                    }
                    (ClientMsg::DonateToInstitute { artifact, .. }, None) if what == "artifact" => {
                        if let Some(a) = run.g.artifacts_of(pid).first() {
                            *artifact = a.id;
                        }
                    }
                    (ClientMsg::DonateToInstitute { institute, .. }, None) if what == "institute" => {
                        if let Some(i) = run.g.store.data.institutes.values().rev().find(|i| i.system == sys) {
                            *institute = i.id;
                        }
                    }
                    (ClientMsg::Scan { poi }, Some(kind)) => {
                        let best = run.g.systems[&sys].layout.pois.iter().filter(|p| format!("{:?}", p.kind) == kind).min_by(|a, b| dist(a.pos).total_cmp(&dist(b.pos)));
                        if let Some(p) = best {
                            *poi = p.id;
                        }
                    }
                    (ClientMsg::Mine { asteroid }, None) if what == "asteroid" => {
                        let best = run.g.systems[&sys].layout.asteroids.iter().min_by(|a, b| dist(a.pos).total_cmp(&dist(b.pos)));
                        if let Some(a) = best {
                            *asteroid = Some(a.id);
                        }
                    }
                    (ClientMsg::InvestigateWreck { wreck }, None) if what == "wreck" => {
                        let best = run.g.systems[&sys].ents.values().filter(|e| e.kind == EntityKind::Wreck).min_by(|a, b| dist(a.pos).total_cmp(&dist(b.pos)));
                        if let Some(w) = best {
                            *wreck = w.id;
                        }
                    }
                    (ClientMsg::AcceptContract { id } | ClientMsg::CancelContract { id } | ClientMsg::DeliverContract { id }, None) if what == "contract" => {
                        // Newest open contract (or, for delivery, the newest running one).
                        let best = run.g.store.data.contracts.values().rev().find(|c| matches!(c.state, shared::contracts::ContractState::Open | shared::contracts::ContractState::Accepted));
                        if let Some(c) = best {
                            *id = c.id;
                        }
                    }
                    _ => rep.lines.push(format!("SCRIPT: fill_nearest {what:?} does not fit {:?}", st.msg)),
                }
            }
            if st.aim
                && let ClientMsg::Controls(c) = &mut msg
                && let Some((pid, me)) = run.pid(&st.captain).and_then(|p| run.flagship(&st.captain).map(|e| (p, (e.pos, e.rot, e.target))))
                && let Some((sys, _)) = run.g.player_entity(pid)
                && let Some(t) = me.2.and_then(|t| run.g.ent(sys, t)).map(|t| t.pos)
            {
                // Same steering law the bots use: turn the nose toward the target.
                let local = me.1.inverse() * (t - me.0).normalize_or_zero();
                c.yaw = (local.x * 4.0).clamp(-1.0, 1.0);
                c.pitch = (local.y * 4.0).clamp(-1.0, 1.0);
                if local.z > 0.0 {
                    c.yaw = if local.x >= 0.0 { 1.0 } else { -1.0 };
                }
            }
            if st.hold_over
                && let ClientMsg::Controls(c) = &mut msg
                && let Some((pid, me)) = run.pid(&st.captain).and_then(|p| run.flagship(&st.captain).map(|e| (p, (e.pos, e.rot, e.target))))
                && let Some((sys, _)) = run.g.player_entity(pid)
                && let Some((tp, tr)) = me.2.and_then(|t| run.g.ent(sys, t)).map(|t| (t.pos, t.rot))
            {
                // The point straight over (or under) the target's hull at this range; fly there in my frame.
                let t_up = tr * Vec3::Y;
                let side = if (me.0 - tp).dot(t_up) >= 0.0 { 1.0 } else { -1.0 };
                let want = tp + t_up * side * me.0.distance(tp);
                let local = me.1.inverse() * (want - me.0) / 150.0;
                c.strafe_x = local.x.clamp(-1.0, 1.0);
                c.strafe_y = local.y.clamp(-1.0, 1.0);
                c.thrust = (-local.z).clamp(-1.0, 1.0);
            }
            if st.broadside
                && let ClientMsg::Controls(c) = &mut msg
                && let Some((pid, me)) = run.pid(&st.captain).and_then(|p| run.flagship(&st.captain).map(|e| (p, (e.pos, e.rot, e.target))))
                && let Some((sys, _)) = run.g.player_entity(pid)
                && let Some(t) = me.2.and_then(|t| run.g.ent(sys, t)).map(|t| t.pos)
            {
                // Roll the target's bearing down into the hull's plane (toward whichever side it already leans).
                let local = me.1.inverse() * (t - me.0).normalize_or_zero();
                let side = if local.x >= 0.0 { 1.0 } else { -1.0 };
                c.roll = (-local.y * side * 4.0).clamp(-1.0, 1.0);
            }
            run.g.handle(conn, msg);
        }
        while next_check < checks.len() && checks[next_check].0 == tick {
            let (ok, s) = evaluate(&run, &checks[next_check].1.check);
            rep.lines.push(format!("{} t={:.1}s {s}", if ok { "PASS" } else { "FAIL" }, t));
            if ok {
                rep.passed += 1;
            } else {
                rep.failed += 1;
            }
            next_check += 1;
        }
        if let Some(every) = trace_ticks
            && tick % every == 0
        {
            rep.lines.push(trace_line(&run, t));
        }
        if rep.npcs_cleared_at.is_none() && !sc.npcs.is_empty() && !run.g.systems.values().any(|sm| sm.ents.values().any(|e| !e.dead && matches!(e.kind, EntityKind::Npc(_)))) {
            rep.npcs_cleared_at = Some(t);
        }
        for (name, _) in &run.conns {
            if run.flagship(name).is_none() && !rep.down_at.iter().any(|(n, _)| n == name) {
                rep.down_at.push((name.clone(), t));
            }
        }
        if tick < total {
            run.g.tick(dt, sc.unix + tick / TICK_HZ as u64);
            run.g.out.clear();
            run.g.kicked.clear();
            let lines = run.log.observe(&mut run.g);
            rep.metrics.extend(lines);
        }
    }
    rep.digest = crate::harness::digest(&run.g);
    rep.final_hull = run.conns.iter().map(|(n, _)| (n.clone(), run.flagship(n).map(|e| e.hull / e.stats.max_hull.max(1.0)).unwrap_or(0.0))).collect();
    rep.totals = run.conns.iter().map(|(n, _)| (n.clone(), run.totals(n))).collect();
    rep.final_hull_lost = run.conns.iter().map(|(n, _)| (n.clone(), run.flagship(n).map(|e| e.stats.max_hull - e.hull).unwrap_or(f32::INFINITY))).collect();
    if let Some(want) = &sc.digest {
        let ok = *want == rep.digest;
        rep.lines.push(format!("{} digest {} (want {want})", if ok { "PASS" } else { "FAIL" }, rep.digest));
        if ok {
            rep.passed += 1;
        } else {
            rep.failed += 1;
        }
    }
    rep
}

pub fn load(path: &std::path::Path) -> Result<Scenario, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
}

/// Scenario files under a directory (or the single file given), sorted by name.
pub fn collect(path: &std::path::Path) -> Vec<std::path::PathBuf> {
    if path.is_file() {
        return vec![path.to_path_buf()];
    }
    let mut v: Vec<_> = std::fs::read_dir(path).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "json")).collect();
    v.sort();
    v
}

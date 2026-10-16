//! Data-driven gameplay specs, run headless and deterministically (docs/HARNESS.md, "결정론 개발 구조").
//!
//! A scenario is a JSON file: who is where (captains with hull/levels/positions, NPC groups), which
//! commands arrive when (any `ClientMsg`, as JSON), how long to run, and what must be true at given
//! moments. The runner builds the simulation without network or engine, plays it at the fixed tick
//! and reports every check with the measured value — so a new gameplay test or a balance probe is a
//! data file, not Rust code, and the same file always gives the same result.
//!
//! `cargo run -p server --release --example scenario -- crates/server/scenarios [--trace 5]`

use crate::game::{ConnId, Game};
use crate::store::Store;
use serde::{Deserialize, Serialize};
use shared::galaxy::{Species, SystemId};
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
    /// Yaw in degrees (0 = facing -Z).
    #[serde(default)]
    pub heading: f32,
    #[serde(default = "yes")]
    pub undocked: bool,
    /// Credits on top of the starting wallet.
    #[serde(default)]
    pub credits: u64,
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
}

impl Report {
    pub fn ok(&self) -> bool {
        self.failed == 0
    }
}

struct Run {
    g: Game,
    conns: Vec<(String, ConnId)>,
}

impl Run {
    fn conn(&self, name: &str) -> Option<ConnId> {
        self.conns.iter().find(|(n, _)| n == name).map(|(_, c)| *c)
    }
    fn pid(&self, name: &str) -> Option<PlayerId> {
        self.g.store.find_by_name(name)
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
    let mut g = Game::new(Store::memory(), history::HistoryEngine::new(), sc.unix);
    g.dev_mode = sc.dev;
    let mut rep = Report { name: sc.name.clone(), ..Default::default() };
    let mut run = Run { g, conns: vec![] };

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
        }
        run.g.spawn_player(pid, c.system, !c.undocked);
        if let Some((sys, eid)) = run.g.player_entity(pid)
            && let Some(e) = run.g.ent_mut(sys, eid)
        {
            if let Some(p) = c.pos {
                e.pos = Vec3::from(p);
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
            if let Some(kind) = &st.target_nearest
                && let Some(me) = run.flagship(&st.captain).map(|e| (e.pos, e.id))
                && let Some((sys, _)) = run.pid(&st.captain).and_then(|p| run.g.player_entity(p))
            {
                let best = run.g.systems[&sys].ents.values().filter(|e| !e.dead && e.id != me.1 && kind_matches(kind, e.kind)).min_by(|a, b| a.pos.distance(me.0).total_cmp(&b.pos.distance(me.0))).map(|e| e.id);
                run.g.handle(conn, ClientMsg::SetTarget(best));
            }
            let mut msg = st.msg.clone();
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
        }
    }
    rep.digest = crate::harness::digest(&run.g);
    rep.final_hull = run.conns.iter().map(|(n, _)| (n.clone(), run.flagship(n).map(|e| e.hull / e.stats.max_hull.max(1.0)).unwrap_or(0.0))).collect();
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

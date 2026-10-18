//! Gameplay numbers kept out of the code (docs/HARNESS.md, "데이터를 코드 밖으로").
//!
//! Hull base stats, module growth, size-class scaling, upgrade costs, NPC archetypes, escorts and
//! combat rules live in `tuning/*.ron` at the workspace root and are read at run time, so a balance
//! change is an edit to a text file followed by a scenario run or a balance sheet — no recompile.
//!
//! The directory is found in this order: `STARFALL_TUNING`, then a `tuning/` folder in the current
//! directory or any parent, then next to the executable or any parent. Unknown or missing fields
//! are load errors, so a typo never silently falls back to a default.
//!
//! The simulation reads one process-wide copy (`get`). The server sends it to clients on login so
//! the HUD shows the server's numbers; a recording keeps a copy so replays use the same numbers.

use crate::artifacts::Culture;
use crate::minerals::{Property, Rarity};
use crate::missions::NpcArchetype;
use crate::ships::{BaseStats, HullClass, Materials, Module, SizeClass};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

pub const DIR_ENV: &str = "STARFALL_TUNING";
pub const DIR_NAME: &str = "tuning";
pub const FILES: [&str; 9] = ["ships.ron", "npcs.ron", "combat.ron", "artifacts.ron", "history.ron", "social.ron", "economy.ron", "galaxy.ron", "minerals.ron"];

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Tuning {
    pub ships: ShipTuning,
    pub npcs: NpcTuning,
    pub combat: CombatTuning,
    pub artifacts: ArtifactTuning,
    pub history: HistoryTuning,
    pub social: SocialTuning,
    pub economy: EconomyTuning,
    pub galaxy: GalaxyTuning,
    pub minerals: MineralTuning,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShipTuning {
    pub hulls: BTreeMap<HullClass, BaseStats>,
    pub growth: Growth,
    pub classes: BTreeMap<SizeClass, ClassDef>,
    pub batteries: Batteries,
    pub upgrade: UpgradeCosts,
    pub escort: EscortTuning,
}

/// What one module level beyond the first adds (fractions of the base value unless noted).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Growth {
    pub hull: f32,
    pub shield: f32,
    /// Shield regeneration per second as a share of the base shield, at reactor level 1…
    pub shield_regen: f32,
    /// …plus this share per reactor level.
    pub reactor_regen: f32,
    /// Reactor levels shorten gun and turret intervals by this factor each.
    pub reactor_fire_rate: f32,
    /// Seconds between missiles at reactor level 1, shortened by `reactor_missile` per level.
    pub missile_cooldown: f32,
    pub reactor_missile: f32,
    /// Speed and acceleration.
    pub engine: f32,
    pub engine_turn: f32,
    /// Light-years per warp-core level (added, not multiplied).
    pub warp_ly: f32,
    pub weapons_damage: f32,
    pub weapons_range: f32,
    pub scanner_range: f32,
    /// Seconds to scan at scanner level 1, shortened by `scanner_speed` per level.
    pub scan_time: f32,
    pub scanner_speed: f32,
    pub scanner_mining: f32,
    pub cargo: f32,
}

/// One size class: the rank it starts at, how it scales the hull's stats and which batteries it carries.
/// Every `*_per_rank` counts ranks above `min_rank`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClassDef {
    pub min_rank: u8,
    pub size: f32,
    pub size_per_rank: f32,
    pub hp: f32,
    pub hp_per_rank: f32,
    pub speed: f32,
    pub accel: f32,
    pub turn: f32,
    pub range: f32,
    pub turrets: u8,
    pub turrets_per_rank: u8,
    /// Bow main-gun volley as a multiple of the gun damage (0 = no main gun).
    pub main_gun: f32,
    pub main_interval: f32,
    pub escorts: u8,
    /// One more escort every this many ranks (0 = never).
    pub escorts_every: u8,
}

/// Turret and main-gun numbers relative to the forward guns.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Batteries {
    pub turret_damage: f32,
    pub turret_interval: f32,
    pub turret_range: f32,
    pub main_range: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpgradeCosts {
    /// Credits to leave level `l` = round(credits × l^credits_exp).
    pub credits: f32,
    pub credits_exp: f32,
    pub modules: BTreeMap<Module, MaterialRates>,
}

/// `(per, after)`: the material costs `per × (level − after)` once the level passes `after`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rate(pub u32, pub u8);

impl Rate {
    pub fn at(self, level: u8) -> u32 {
        self.0 * level.saturating_sub(self.1) as u32
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MaterialRates {
    pub alloy: Rate,
    pub crystal: Rate,
    pub datacore: Rate,
    pub relic: Rate,
}

/// Escort ships as shares of their flagship's stats, with floors so a small flagship still gets useful escorts.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EscortTuning {
    pub hp: f32,
    pub speed: f32,
    pub min_speed: f32,
    pub accel: f32,
    pub min_accel: f32,
    pub turn: f32,
    pub damage: f32,
    pub fire_interval: f32,
    pub range: f32,
    pub min_range: f32,
    pub radius: f32,
    pub min_radius: f32,
    pub max_radius: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NpcTuning {
    /// Hull, shield and damage grow by this share per NPC level.
    pub level_scale: f32,
    pub weapon_range: f32,
    pub weapon_range_per_radius: f32,
    pub shield_regen: f32,
    pub accel: f32,
    pub turret_damage: f32,
    pub turret_interval: f32,
    pub turret_range: f32,
    pub archetypes: BTreeMap<NpcArchetype, NpcDef>,
    /// How NPCs use 3D space. Missing (recordings made before it existed): the old flat behaviour.
    #[serde(default)]
    pub tactics: Tactics,
}

/// 3D tactics for NPCs and escorts (main goal: fights that use height). The default is the old
/// flat behaviour, so a recording made before this existed replays unchanged.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tactics {
    /// Closing in, a pirate aims this × distance above or below the target (its dive side), so the
    /// attack run comes in at an angle (0.55 ≈ 29°).
    pub dive_slope: f32,
    /// In gun range, the orbit strafe is tilted out of the ship's horizontal by an angle in this range
    /// (degrees, fixed per ship)…
    pub orbit_tilt: (f32, f32),
    /// …and the tilt turns at this many radians per second (a corkscrew around the target).
    pub orbit_spin: f32,
    /// Pirate groups spawn within this horizontal and vertical half-extent of their anchor (u).
    pub spawn_xz: f32,
    pub spawn_y: f32,
    /// Idle patrol circle: vertical swing as a share of its radius.
    pub wander_tilt: f32,
}

impl Default for Tactics {
    fn default() -> Self {
        Tactics { dive_slope: 0.0, orbit_tilt: (0.0, 0.0), orbit_spin: 0.0, spawn_xz: 300.0, spawn_y: 80.0, wander_tilt: 0.0 }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NpcDef {
    pub hull: f32,
    pub shield: f32,
    pub damage: f32,
    pub fire_interval: f32,
    pub speed: f32,
    pub turn: f32,
    pub radius: f32,
    pub turrets: u8,
    /// Credits are multiplied by (1 + level); the materials are fixed.
    pub bounty: Materials,
}

/// Ore market (`economy.ron`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EconomyTuning {
    pub ore_prices: BTreeMap<crate::galaxy::Ore, f32>,
    pub tier_base: f32,
    pub tier_step: f32,
    pub scarcity_supply: f32,
    pub trader_bonus: f32,
    pub miner_bonus: f32,
    /// Per risk tier: scan, artifact and battle-win rewards…
    pub risk_reward: BTreeMap<crate::galaxy::RiskTier, f32>,
    /// …and pirate bounties.
    pub bounty_risk: BTreeMap<crate::galaxy::RiskTier, f32>,
}

/// The procedural galaxy around the hand-made core (`galaxy.ron`, see `galaxy::systems`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GalaxyTuning {
    pub seed: u64,
    pub generated: u32,
    /// x min, x max, z min, z max (ly).
    pub bounds: (f32, f32, f32, f32),
    /// Distance range (ly) from the system a new one grows from.
    pub hop: (f32, f32),
    pub min_spacing: f32,
    pub core_clearance: f32,
    pub deep_clearance: f32,
    /// (up to this many ly from Sol Haven, tier), nearest first.
    pub tiers: Vec<(f32, crate::galaxy::RiskTier)>,
    pub stars: BTreeMap<crate::galaxy::StarKind, u32>,
    pub names_first: Vec<String>,
    pub names_second: Vec<String>,
    pub lore: BTreeMap<crate::galaxy::RiskTier, Vec<String>>,
    pub chart_reward: u64,
    /// How far a system's places rise out of its plane. Missing (recordings made before it existed):
    /// the old flat layout.
    #[serde(default)]
    pub relief: Relief,
}

/// Height in a system's layout (main goal "3D freedom", docs/BALANCE.md 「성계 지형」): each value is a
/// half-range in units (or a share of the horizontal spread) for the same random draw as before, so
/// only heights change — ids, ores and minerals stay put.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Relief {
    pub station_y: f32,
    pub jump_y: f32,
    /// Asteroid belt centre height, and the belt's half-thickness (its half-width is 520).
    pub belt_y: f32,
    pub belt_thickness: f32,
    /// Points of interest: height spread as a share of their horizontal spread.
    pub poi_rise: f32,
}

impl Default for Relief {
    fn default() -> Self {
        Relief { station_y: 60.0, jump_y: 120.0, belt_y: 150.0, belt_thickness: 140.0, poi_rise: 0.2 }
    }
}

/// The mineral catalogue and deposits (`minerals.ron`, see `minerals`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MineralTuning {
    pub seed: u64,
    /// Minerals per family of each rarity.
    pub counts: BTreeMap<Rarity, u32>,
    pub property_mean: BTreeMap<Rarity, u32>,
    pub spread: u32,
    pub affinity_bonus: u32,
    pub family_affinity: BTreeMap<crate::galaxy::Ore, Vec<Property>>,
    pub star_affinity: BTreeMap<crate::galaxy::StarKind, Property>,
    /// Rarity weights of an asteroid's deposit, per system tier.
    pub deposit: BTreeMap<crate::galaxy::RiskTier, BTreeMap<Rarity, u32>>,
    /// Asteroids far above or below the station hold rarer deposits. Missing (recordings made before
    /// it existed): no bonus.
    #[serde(default)]
    pub height_bonus: HeightBonus,
    pub candidates: u32,
    /// Chance per ore unit mined of also getting a specimen (rarities not listed: never).
    pub specimen_chance: BTreeMap<Rarity, f32>,
    pub specimen_value: BTreeMap<Rarity, u64>,
    pub discovery_reputation: BTreeMap<Rarity, i32>,
    pub syllables: Vec<String>,
    pub endings: BTreeMap<crate::galaxy::Ore, Vec<String>>,
}

/// Main goal "3D freedom": a reason to climb and dive. An asteroid whose route from the station
/// climbs or dives more than `from_deg` gets its rare-and-above deposit weights multiplied by up to
/// `1 + weight`, reached at `full_deg` (the tier's rarities stay the same: no new top rarity).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HeightBonus {
    pub from_deg: f32,
    pub full_deg: f32,
    pub weight: f32,
}

impl Default for HeightBonus {
    fn default() -> Self {
        HeightBonus { from_deg: 20.0, full_deg: 45.0, weight: 0.0 }
    }
}

impl HeightBonus {
    /// Multiplier for non-common deposit weights at this elevation (degrees, either sign).
    pub fn factor(&self, elev_deg: f32) -> f32 {
        let k = ((elev_deg.abs() - self.from_deg) / (self.full_deg - self.from_deg).max(1e-3)).clamp(0.0, 1.0);
        1.0 + self.weight * k
    }
}

/// Player contracts (`social.ron`).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SocialTuning {
    pub min_reward: u64,
    pub max_hours: u32,
    pub max_open: u32,
    pub fulfil_reputation: i32,
    pub betrayal_reputation: i32,
    pub influence_pirate: f32,
    pub influence_sale_unit: f32,
    pub influence_contract: f32,
    pub influence_donation: f32,
    pub control_threshold: f32,
    pub control_margin: f32,
    pub influence_decay_hourly: f32,
    pub control_bonus: f32,
    pub influence_battle_win: f32,
    pub influence_battle_loss: f32,
    pub tax_rate: f32,
    pub overreach: f32,
    pub free_systems: u32,
    pub upkeep_base: u64,
    pub upkeep_exponent: f32,
    pub institute_cost: u64,
    pub visit_base: u64,
    pub visit_per_exhibit: u64,
    pub exhibit_reputation: i32,
}

impl SocialTuning {
    /// Hourly treasury upkeep for holding `n` systems (the first `free_systems` cost nothing).
    pub fn upkeep(&self, n: usize) -> u64 {
        let over = n.saturating_sub(self.free_systems as usize);
        if over == 0 { 0 } else { (self.upkeep_base as f32 * (over as f32).powf(self.upkeep_exponent)).round() as u64 }
    }
}

/// Forged evidence and authenticity checks (`history.ron`).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HistoryTuning {
    pub forge_cost: u64,
    pub authenticate_cost: u64,
    pub forger_reputation: i32,
    pub examiner_reputation: i32,
    pub examiner_reward: u64,
    pub discredit: f32,
    /// Journalists: paid on filing, then per distinct reader up to the cap; a correction costs reputation.
    pub story_fee: u64,
    pub story_per_reader: u64,
    pub story_reader_cap: u32,
    pub retraction_reputation: i32,
    pub seize_max: u32,
    pub leak_reputation: i32,
    pub scoop_bonus: u64,
    pub herald_articles: u32,
    pub herald_kind_cap: u32,
}

/// Artifact names, values and station services (`artifacts.ron`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactTuning {
    pub value_spread: f32,
    pub appraisal_cost: u64,
    pub unappraised_sale: f32,
    pub donation_reputation: i32,
    pub donation_share: f32,
    pub cultures: BTreeMap<Culture, CultureDef>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CultureDef {
    pub value: u64,
    pub nouns: Vec<String>,
    pub epithets: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CombatTuning {
    /// Share of every hit that passes through shields straight to the hull.
    pub shield_bleed: f32,
    /// Hull points every landed hit costs at least.
    pub min_hull_damage: f32,
    /// Aim cones in radians: player fighter guns, keeping a locked target, AI guns, bow main gun.
    pub laser_cone: f32,
    pub lock_cone: f32,
    pub npc_cone: f32,
    pub main_cone: f32,
    /// Turret firing arcs (3D step 3): batteries alternate dorsal and ventral; a dorsal battery
    /// engages targets whose direction in the ship's frame has up-component ≥ −this (ventral ≤ +this).
    /// 1.0 = no arcs, the behaviour before arcs existed (and the default for recordings without it).
    #[serde(default = "no_arcs")]
    pub turret_arc_overlap: f32,
}

fn no_arcs() -> f32 {
    1.0
}

impl Tuning {
    /// Reads every file in [`FILES`] from `dir` and checks every hull, class, module, archetype and
    /// culture is present.
    pub fn load(dir: &Path) -> Result<Tuning, String> {
        Self::load_with_fallback(dir, None)
    }

    /// Like `load`, but a file missing from `dir` is read from `fallback` instead (a recording made
    /// before that file existed replays with today's numbers for it).
    pub fn load_with_fallback(dir: &Path, fallback: Option<&Path>) -> Result<Tuning, String> {
        fn read<T: for<'de> Deserialize<'de>>(dir: &Path, fallback: Option<&Path>, file: &str) -> Result<T, String> {
            let mut path = dir.join(file);
            if !path.is_file()
                && let Some(f) = fallback
            {
                path = f.join(file);
            }
            let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            ron::from_str(&text).map_err(|e| format!("{}:{e}", path.display()))
        }
        let t = Tuning { ships: read(dir, fallback, FILES[0])?, npcs: read(dir, fallback, FILES[1])?, combat: read(dir, fallback, FILES[2])?, artifacts: read(dir, fallback, FILES[3])?, history: read(dir, fallback, FILES[4])?, social: read(dir, fallback, FILES[5])?, economy: read(dir, fallback, FILES[6])?, galaxy: read(dir, fallback, FILES[7])?, minerals: read(dir, fallback, FILES[8])? };
        t.validate().map_err(|e| format!("{}: {e}", dir.display()))?;
        Ok(t)
    }

    fn validate(&self) -> Result<(), String> {
        for h in HullClass::ALL {
            self.ships.hulls.get(&h).ok_or(format!("ships.ron: hulls has no {h:?}"))?;
        }
        for m in Module::ALL {
            self.ships.upgrade.modules.get(&m).ok_or(format!("ships.ron: upgrade.modules has no {m:?}"))?;
        }
        let mut prev: Option<u8> = None;
        for c in SizeClass::ALL {
            let d = self.ships.classes.get(&c).ok_or(format!("ships.ron: classes has no {c:?}"))?;
            if prev.is_none() && d.min_rank != 0 || prev.is_some_and(|p| d.min_rank <= p) {
                return Err(format!("ships.ron: class min_rank must start at 0 and rise (Small, Cruiser, Capital); {c:?} has {}", d.min_rank));
            }
            prev = Some(d.min_rank);
        }
        for a in NpcArchetype::ALL {
            self.npcs.archetypes.get(&a).ok_or(format!("npcs.ron: archetypes has no {a:?}"))?;
        }
        for t in [crate::galaxy::RiskTier::Core, crate::galaxy::RiskTier::Frontier, crate::galaxy::RiskTier::Wild, crate::galaxy::RiskTier::WarZone, crate::galaxy::RiskTier::Deep] {
            self.economy.risk_reward.get(&t).ok_or(format!("economy.ron: risk_reward has no {t:?}"))?;
            self.economy.bounty_risk.get(&t).ok_or(format!("economy.ron: bounty_risk has no {t:?}"))?;
        }
        for o in crate::galaxy::Ore::ALL {
            self.economy.ore_prices.get(&o).ok_or(format!("economy.ron: ore_prices has no {o:?}"))?;
        }
        let g = &self.galaxy;
        if g.tiers.is_empty() || g.stars.values().sum::<u32>() == 0 || g.names_first.is_empty() || g.names_second.is_empty() {
            return Err("galaxy.ron: tiers, stars, names_first and names_second must not be empty".into());
        }
        for (_, t) in &g.tiers {
            if g.lore.get(t).is_none_or(|l| l.is_empty()) {
                return Err(format!("galaxy.ron: lore has no {t:?}"));
            }
        }
        if g.hop.0 <= 0.0 || g.hop.0 > g.hop.1 || g.min_spacing > g.hop.1 {
            return Err("galaxy.ron: need 0 < hop.0 <= hop.1 and min_spacing <= hop.1".into());
        }
        let m = &self.minerals;
        let family: u32 = m.counts.values().sum();
        for o in crate::galaxy::Ore::ALL {
            let n = m.endings.get(&o).map(|e| e.len()).unwrap_or(0);
            if n == 0 || (m.syllables.len() * m.syllables.len() * n) < family as usize {
                return Err(format!("minerals.ron: {o:?} needs endings, and syllables² × endings ≥ {family} (unique names)"));
            }
        }
        for t in [crate::galaxy::RiskTier::Core, crate::galaxy::RiskTier::Frontier, crate::galaxy::RiskTier::Wild, crate::galaxy::RiskTier::WarZone, crate::galaxy::RiskTier::Deep] {
            if m.deposit.get(&t).is_none_or(|w| w.values().sum::<u32>() == 0) {
                return Err(format!("minerals.ron: deposit has no weights for {t:?}"));
            }
        }
        for c in Culture::ALL {
            let d = self.artifacts.cultures.get(&c).ok_or(format!("artifacts.ron: cultures has no {c:?}"))?;
            if d.nouns.is_empty() || d.epithets.is_empty() {
                return Err(format!("artifacts.ron: {c:?} needs at least one noun and one epithet"));
            }
        }
        Ok(())
    }

    /// The directory `get` loads from when nothing was installed (see the module docs).
    pub fn locate() -> Option<PathBuf> {
        if let Some(d) = std::env::var_os(DIR_ENV) {
            return Some(PathBuf::from(d));
        }
        let found = |start: &Path| start.ancestors().map(|a| a.join(DIR_NAME)).find(|d| d.join(FILES[0]).is_file());
        std::env::current_dir()
            .ok()
            .and_then(|d| found(&d))
            .or_else(|| std::env::current_exe().ok().and_then(|e| found(e.parent()?)))
    }

    /// Copies the tuning files to `dir` (recordings keep the numbers they were made with).
    pub fn copy_files(from: &Path, dir: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(dir)?;
        for f in FILES {
            std::fs::copy(from.join(f), dir.join(f))?;
        }
        Ok(())
    }
}

static CURRENT: RwLock<Option<Arc<Tuning>>> = RwLock::new(None);

/// The numbers in force. Loads `Tuning::locate()` on first use and panics with the reason if that fails.
pub fn get() -> Arc<Tuning> {
    if let Some(t) = CURRENT.read().unwrap().as_ref() {
        return t.clone();
    }
    let mut w = CURRENT.write().unwrap();
    w.get_or_insert_with(|| {
        let dir = Tuning::locate().unwrap_or_else(|| panic!("no {DIR_NAME}/ directory found (set {DIR_ENV} or run from the workspace)"));
        Arc::new(Tuning::load(&dir).unwrap_or_else(|e| panic!("tuning: {e}")))
    })
    .clone()
}

/// Replaces the numbers in force (a client adopting the server's, a replay adopting a recording's).
pub fn install(t: Tuning) {
    *CURRENT.write().unwrap() = Some(Arc::new(t));
    crate::galaxy::tuning_changed();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_tuning_loads_and_round_trips() {
        let dir = Tuning::locate().expect("tuning dir");
        let t = Tuning::load(&dir).unwrap();
        let js = serde_json::to_string(&t).unwrap();
        assert_eq!(serde_json::from_str::<Tuning>(&js).unwrap(), t);
    }

    #[test]
    fn typos_are_errors() {
        let dir = std::env::temp_dir().join(format!("starfall-tuning-typo-{}", std::process::id()));
        Tuning::copy_files(&Tuning::locate().unwrap(), &dir).unwrap();
        let p = dir.join("combat.ron");
        let text = std::fs::read_to_string(&p).unwrap().replace("shield_bleed", "shield_bleeed");
        std::fs::write(&p, text).unwrap();
        let err = Tuning::load(&dir).unwrap_err();
        let _ = std::fs::remove_dir_all(&dir);
        assert!(err.contains("shield_bleeed"), "{err}");
    }

    #[test]
    fn rate_starts_after_its_level() {
        assert_eq!(Rate(2, 3).at(3), 0);
        assert_eq!(Rate(2, 3).at(5), 4);
        assert_eq!(Rate(4, 0).at(5), 20);
    }
}

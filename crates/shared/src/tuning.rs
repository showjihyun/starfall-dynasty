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
use crate::missions::NpcArchetype;
use crate::ships::{BaseStats, HullClass, Materials, Module, SizeClass};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

pub const DIR_ENV: &str = "STARFALL_TUNING";
pub const DIR_NAME: &str = "tuning";
pub const FILES: [&str; 6] = ["ships.ron", "npcs.ron", "combat.ron", "artifacts.ron", "history.ron", "social.ron"];

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Tuning {
    pub ships: ShipTuning,
    pub npcs: NpcTuning,
    pub combat: CombatTuning,
    pub artifacts: ArtifactTuning,
    pub history: HistoryTuning,
    pub social: SocialTuning,
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
        let t = Tuning { ships: read(dir, fallback, FILES[0])?, npcs: read(dir, fallback, FILES[1])?, combat: read(dir, fallback, FILES[2])?, artifacts: read(dir, fallback, FILES[3])?, history: read(dir, fallback, FILES[4])?, social: read(dir, fallback, FILES[5])? };
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

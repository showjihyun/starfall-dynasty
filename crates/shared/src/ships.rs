//! Hull classes, modules, derived stats and upgrade economics.
//!
//! A player owns exactly one ship for life. It never gets swapped; it grows by
//! upgrading its eight modules. The hull designation (Scout → … → Leviathan)
//! is derived from total module levels, and the client grows the model with it.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum HullClass {
    /// Fast scout, longest warp, light armour.
    Wren,
    /// Balanced interceptor built for dogfights.
    Talon,
    /// Heavy frigate: slow, tough, big guns.
    Bastion,
    /// Prospector hauler: mining lasers and a deep hold.
    Mule,
    /// Veyr-style survey ship: scanners, shields, relic hunting.
    Seer,
}

impl HullClass {
    pub const ALL: [HullClass; 5] = [HullClass::Wren, HullClass::Talon, HullClass::Bastion, HullClass::Mule, HullClass::Seer];

    pub fn name(self) -> &'static str {
        match self {
            HullClass::Wren => "Wren S-01 Scout",
            HullClass::Talon => "Talon C-3 Interceptor",
            HullClass::Bastion => "Bastion F-7 Frigate",
            HullClass::Mule => "Mule P-2 Prospector",
            HullClass::Seer => "Seer V-9 Surveyor",
        }
    }

    pub fn blurb(self) -> &'static str {
        match self {
            HullClass::Wren => "가장 빠르고 워프 거리가 길다. 장갑이 얇아 치고 빠지는 탐험가용.",
            HullClass::Talon => "균형 잡힌 요격기. 기동성과 화력 모두 준수한 근접 교전 특화.",
            HullClass::Bastion => "느리지만 단단한 프리깃. 높은 내구도와 중화기로 함대의 방패가 된다.",
            HullClass::Mule => "채굴 레이저와 대형 화물칸. 광물로 부를 쌓는 상인/광부의 배.",
            HullClass::Seer => "Veyr 생체 조사선. 강력한 스캐너와 실드로 유적과 이상현상을 쫓는다.",
        }
    }

    /// Level-1 stats from `tuning/ships.ron`.
    pub fn base(self) -> BaseStats {
        crate::tuning::get().ships.hulls[&self]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BaseStats {
    pub hull: f32,
    pub shield: f32,
    pub speed: f32,
    pub accel: f32,
    pub turn: f32,
    /// Light-years.
    pub warp: f32,
    pub damage: f32,
    pub fire_interval: f32,
    pub weapon_range: f32,
    pub missile_damage: f32,
    pub scan_range: f32,
    pub mine_rate: f32,
    pub cargo: f32,
    pub radius: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Module {
    Hull,
    Reactor,
    Engine,
    WarpCore,
    Shield,
    Weapons,
    Scanner,
    Cargo,
}

impl Module {
    pub const ALL: [Module; 8] = [
        Module::Hull,
        Module::Reactor,
        Module::Engine,
        Module::WarpCore,
        Module::Shield,
        Module::Weapons,
        Module::Scanner,
        Module::Cargo,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Module::Hull => "선체 (Hull)",
            Module::Reactor => "반응로 (Reactor)",
            Module::Engine => "엔진 (Engine)",
            Module::WarpCore => "워프 코어 (Warp Core)",
            Module::Shield => "실드 (Shield)",
            Module::Weapons => "무장 (Weapons)",
            Module::Scanner => "스캐너 (Scanner)",
            Module::Cargo => "화물칸 (Cargo)",
        }
    }

    pub fn effect(self) -> &'static str {
        match self {
            Module::Hull => "최대 내구도 +15%/lv",
            Module::Reactor => "실드 재충전·연사 속도 향상",
            Module::Engine => "속도·가속·선회 +7%/lv",
            Module::WarpCore => "워프 거리 +0.55 ly/lv",
            Module::Shield => "최대 실드 +18%/lv",
            Module::Weapons => "레이저·미사일 피해 +14%/lv",
            Module::Scanner => "스캔 범위·속도, 채굴 효율",
            Module::Cargo => "화물 용량 +25%/lv",
        }
    }
}

pub const MAX_MODULE_LEVEL: u8 = 10;
/// Levels 1..=BASE_TIER_CAP are always available; each completed scenario chapter lifts the cap by one.
pub const BASE_TIER_CAP: u8 = 5;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Materials {
    pub credits: u64,
    pub alloy: u32,
    pub crystal: u32,
    pub datacore: u32,
    pub relic: u32,
}

impl Materials {
    pub fn covers(&self, cost: &Materials) -> bool {
        self.credits >= cost.credits && self.alloy >= cost.alloy && self.crystal >= cost.crystal && self.datacore >= cost.datacore && self.relic >= cost.relic
    }

    pub fn pay(&mut self, cost: &Materials) -> bool {
        if !self.covers(cost) {
            return false;
        }
        self.credits -= cost.credits;
        self.alloy -= cost.alloy;
        self.crystal -= cost.crystal;
        self.datacore -= cost.datacore;
        self.relic -= cost.relic;
        true
    }

    pub fn add(&mut self, other: &Materials) {
        self.credits += other.credits;
        self.alloy += other.alloy;
        self.crystal += other.crystal;
        self.datacore += other.datacore;
        self.relic += other.relic;
    }

    pub fn describe(&self) -> String {
        let mut parts = Vec::new();
        if self.credits > 0 {
            parts.push(format!("{} CR", self.credits));
        }
        if self.alloy > 0 {
            parts.push(format!("합금 {}", self.alloy));
        }
        if self.crystal > 0 {
            parts.push(format!("결정 {}", self.crystal));
        }
        if self.datacore > 0 {
            parts.push(format!("데이터코어 {}", self.datacore));
        }
        if self.relic > 0 {
            parts.push(format!("유물 {}", self.relic));
        }
        if parts.is_empty() { "-".into() } else { parts.join(", ") }
    }
}

/// Cost to raise `module` from `level` to `level + 1` (`tuning/ships.ron`, `upgrade`).
pub fn upgrade_cost(module: Module, level: u8) -> Materials {
    let t = crate::tuning::get();
    let u = &t.ships.upgrade;
    let r = u.modules[&module];
    Materials {
        credits: (u.credits * (level as f32).powf(u.credits_exp)).round() as u64,
        alloy: r.alloy.at(level),
        crystal: r.crystal.at(level),
        datacore: r.datacore.at(level),
        relic: r.relic.at(level),
    }
}

pub fn tier_cap(scenarios_completed: u32) -> u8 {
    (BASE_TIER_CAP as u32 + scenarios_completed).min(MAX_MODULE_LEVEL as u32) as u8
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Ship {
    pub name: String,
    pub hull: HullClass,
    pub modules: BTreeMap<Module, u8>,
    /// Accent colour index the client uses for the livery.
    pub livery: u8,
}

impl Ship {
    pub fn new(name: String, hull: HullClass, livery: u8) -> Self {
        Ship { name, hull, modules: Module::ALL.iter().map(|m| (*m, 1)).collect(), livery }
    }

    pub fn level(&self, m: Module) -> u8 {
        *self.modules.get(&m).unwrap_or(&1)
    }

    pub fn total_levels(&self) -> u32 {
        Module::ALL.iter().map(|m| self.level(*m) as u32).sum()
    }

    /// 0 = Scout … 10 = Leviathan. Fresh ship (8 levels) is rank 0, fully upgraded (80) is rank 10.
    pub fn rank(&self) -> u8 {
        ((self.total_levels().saturating_sub(8)) / 7).min(10) as u8
    }

    pub fn designation(&self) -> &'static str {
        RANKS[self.rank() as usize]
    }

    /// The growth line: a small fighter, then a cruiser, then a capital warship.
    pub fn size_class(&self) -> SizeClass {
        SizeClass::of_rank(self.rank())
    }

    pub fn stats(&self) -> Stats {
        let t = crate::tuning::get();
        let (g, bat) = (&t.ships.growth, &t.ships.batteries);
        let b = t.ships.hulls[&self.hull];
        let lv = |m: Module| (self.level(m) as f32 - 1.0).max(0.0);
        let engine = 1.0 + g.engine * lv(Module::Engine);
        let reactor = lv(Module::Reactor);
        let rank = self.rank();
        let class = self.size_class();
        let c = &t.ships.classes[&class];
        let k = class.scaling(rank as f32);
        let damage = b.damage * (1.0 + g.weapons_damage * lv(Module::Weapons));
        let weapon_range = b.weapon_range * (1.0 + g.weapons_range * lv(Module::Weapons)) * k.range;
        let above = rank - c.min_rank;
        let escorts = c.escorts + if c.escorts_every > 0 { above / c.escorts_every } else { 0 };
        Stats {
            max_hull: b.hull * (1.0 + g.hull * lv(Module::Hull)) * k.hp,
            max_shield: b.shield * (1.0 + g.shield * lv(Module::Shield)) * k.hp,
            shield_regen: b.shield * (g.shield_regen + g.reactor_regen * reactor) * k.hp,
            speed: b.speed * engine * k.speed,
            accel: b.accel * engine * k.accel,
            turn: b.turn * (1.0 + g.engine_turn * lv(Module::Engine)) * k.turn,
            warp_range: b.warp + g.warp_ly * lv(Module::WarpCore),
            damage,
            fire_interval: b.fire_interval / (1.0 + g.reactor_fire_rate * reactor),
            weapon_range,
            missile_damage: b.missile_damage * (1.0 + g.weapons_damage * lv(Module::Weapons)) * k.hp.sqrt(),
            missile_cooldown: g.missile_cooldown / (1.0 + g.reactor_missile * reactor),
            scan_range: b.scan_range * (1.0 + g.scanner_range * lv(Module::Scanner)) * k.range,
            scan_time: g.scan_time / (1.0 + g.scanner_speed * lv(Module::Scanner)),
            mine_rate: b.mine_rate * (1.0 + g.scanner_mining * lv(Module::Scanner)) * k.hp.sqrt(),
            cargo: b.cargo * (1.0 + g.cargo * lv(Module::Cargo)) * k.hp.sqrt(),
            radius: b.radius * k.size,
            turrets: c.turrets + c.turrets_per_rank * above,
            turret_damage: damage * bat.turret_damage,
            turret_interval: bat.turret_interval / (1.0 + g.reactor_fire_rate * reactor),
            turret_range: weapon_range * bat.turret_range,
            main_damage: damage * c.main_gun,
            main_interval: c.main_interval,
            main_range: weapon_range * bat.main_range,
            escorts,
        }
    }

    /// Rough combat power used for matchmaking NPC difficulty and engagement summaries.
    pub fn power(&self) -> f32 {
        let s = self.stats();
        (s.max_hull + s.max_shield) * (s.damage / s.fire_interval) / 1000.0
    }
}

pub const RANKS: [&str; 11] = [
    "Scout", "Courier", "Corvette", "Frigate", "Destroyer", "Cruiser", "Battlecruiser", "Battleship", "Dreadnought", "Titan", "Leviathan",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum SizeClass {
    /// Rank 0–2: one-person fighter. Forward guns, nimble.
    Small,
    /// Rank 3–6: cruiser. Turret batteries, bow cannon, first escort.
    Cruiser,
    /// Rank 7–10: capital warship. Main beam battery, many turrets, an escort squadron.
    Capital,
}

pub struct ClassScaling {
    pub size: f32,
    pub hp: f32,
    pub speed: f32,
    pub accel: f32,
    pub turn: f32,
    pub range: f32,
}

impl SizeClass {
    pub const ALL: [SizeClass; 3] = [SizeClass::Small, SizeClass::Cruiser, SizeClass::Capital];

    /// The largest class whose `min_rank` (`tuning/ships.ron`) the rank has reached.
    pub fn of_rank(rank: u8) -> SizeClass {
        let t = crate::tuning::get();
        SizeClass::ALL.into_iter().rev().find(|c| rank >= t.ships.classes[c].min_rank).unwrap_or(SizeClass::Small)
    }

    pub fn name(self) -> &'static str {
        match self {
            SizeClass::Small => "소형함",
            SizeClass::Cruiser => "순양함",
            SizeClass::Capital => "주력함",
        }
    }

    /// Bigger ships are vastly tougher and reach further, but turn like mountains.
    pub fn scaling(self, rank: f32) -> ClassScaling {
        let c = crate::tuning::get().ships.classes[&self];
        let above = rank - c.min_rank as f32;
        ClassScaling { size: c.size + c.size_per_rank * above, hp: c.hp + c.hp_per_rank * above, speed: c.speed, accel: c.accel, turn: c.turn, range: c.range }
    }
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct Stats {
    pub max_hull: f32,
    pub max_shield: f32,
    pub shield_regen: f32,
    pub speed: f32,
    pub accel: f32,
    pub turn: f32,
    pub warp_range: f32,
    pub damage: f32,
    pub fire_interval: f32,
    pub weapon_range: f32,
    pub missile_damage: f32,
    pub missile_cooldown: f32,
    pub scan_range: f32,
    pub scan_time: f32,
    pub mine_rate: f32,
    pub cargo: f32,
    pub radius: f32,
    /// Auto-aiming turret batteries (cruisers and capitals).
    pub turrets: u8,
    pub turret_damage: f32,
    pub turret_interval: f32,
    pub turret_range: f32,
    /// Bow main-gun volley (0 = none).
    pub main_damage: f32,
    pub main_interval: f32,
    pub main_range: f32,
    /// Escort ships that fly in formation.
    pub escorts: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WeaponKind {
    /// Forward pulse lasers (fighters).
    Laser,
    /// Turret battery cannon.
    Pulse,
    /// Bow main-gun beam volley.
    MainBeam,
    Missile,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_ship_is_scout_and_maxed_is_leviathan() {
        let mut s = Ship::new("A".into(), HullClass::Talon, 0);
        assert_eq!(s.rank(), 0);
        assert_eq!(s.designation(), "Scout");
        for m in Module::ALL {
            s.modules.insert(m, MAX_MODULE_LEVEL);
        }
        assert_eq!(s.designation(), "Leviathan");
    }

    #[test]
    fn upgrades_improve_stats_and_costs_grow() {
        let mut s = Ship::new("A".into(), HullClass::Wren, 0);
        let before = s.stats();
        s.modules.insert(Module::Weapons, 4);
        s.modules.insert(Module::WarpCore, 3);
        let after = s.stats();
        assert!(after.damage > before.damage);
        assert!(after.warp_range > before.warp_range + 1.0);
        for m in Module::ALL {
            let mut prev = 0;
            for l in 1..MAX_MODULE_LEVEL {
                let c = upgrade_cost(m, l).credits;
                assert!(c > prev);
                prev = c;
            }
        }
    }

    #[test]
    fn ship_grows_from_fighter_to_capital() {
        let mut s = Ship::new("A".into(), HullClass::Bastion, 0);
        let small = s.stats();
        assert_eq!(s.size_class(), SizeClass::Small);
        assert_eq!(small.turrets, 0);
        for m in Module::ALL {
            s.modules.insert(m, 5);
        }
        assert_eq!(s.size_class(), SizeClass::Cruiser);
        let cruiser = s.stats();
        for m in Module::ALL {
            s.modules.insert(m, 9);
        }
        assert_eq!(s.size_class(), SizeClass::Capital);
        let cap = s.stats();
        assert!(cruiser.radius > small.radius * 4.0 && cap.radius > cruiser.radius * 2.0);
        assert!(cap.max_hull > cruiser.max_hull * 2.0);
        assert!(cap.turn < cruiser.turn && cruiser.turn < small.turn);
        assert!(cap.turrets >= 10 && cap.main_damage > 0.0 && cap.escorts >= 3);
        // A capital ship length lands in the hundreds of metres.
        assert!(cap.radius * 2.2 > 400.0, "len {}", cap.radius * 2.2);
    }

    #[test]
    fn materials_pay_is_atomic() {
        let mut have = Materials { credits: 100, alloy: 1, ..Default::default() };
        let cost = Materials { credits: 50, alloy: 2, ..Default::default() };
        assert!(!have.pay(&cost));
        assert_eq!(have.credits, 100);
    }

    #[test]
    fn tier_cap_unlocks_with_scenarios() {
        assert_eq!(tier_cap(0), 5);
        assert_eq!(tier_cap(3), 8);
        assert_eq!(tier_cap(99), 10);
    }
}

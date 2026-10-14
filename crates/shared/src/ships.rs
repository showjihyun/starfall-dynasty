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

    pub fn base(self) -> BaseStats {
        match self {
            HullClass::Wren => BaseStats { hull: 260., shield: 180., speed: 130., accel: 95., turn: 2.6, warp: 2.2, damage: 14., fire_interval: 0.30, weapon_range: 520., missile_damage: 70., scan_range: 900., mine_rate: 1.2, cargo: 30., radius: 9. },
            HullClass::Talon => BaseStats { hull: 360., shield: 220., speed: 115., accel: 85., turn: 2.3, warp: 1.7, damage: 19., fire_interval: 0.24, weapon_range: 560., missile_damage: 95., scan_range: 600., mine_rate: 1.0, cargo: 25., radius: 11. },
            HullClass::Bastion => BaseStats { hull: 620., shield: 300., speed: 80., accel: 50., turn: 1.4, warp: 1.6, damage: 30., fire_interval: 0.42, weapon_range: 640., missile_damage: 140., scan_range: 600., mine_rate: 1.0, cargo: 40., radius: 16. },
            HullClass::Mule => BaseStats { hull: 440., shield: 200., speed: 90., accel: 60., turn: 1.7, warp: 1.7, damage: 15., fire_interval: 0.34, weapon_range: 500., missile_damage: 80., scan_range: 700., mine_rate: 2.4, cargo: 90., radius: 14. },
            HullClass::Seer => BaseStats { hull: 320., shield: 340., speed: 105., accel: 75., turn: 2.0, warp: 2.0, damage: 16., fire_interval: 0.28, weapon_range: 600., missile_damage: 85., scan_range: 1400., mine_rate: 1.3, cargo: 35., radius: 12. },
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
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

/// Cost to raise `module` from `level` to `level + 1`.
pub fn upgrade_cost(module: Module, level: u8) -> Materials {
    let l = level as f32;
    let credits = (350.0 * l.powf(1.65)).round() as u64;
    let mut m = Materials { credits, alloy: 4 * level as u32, ..Default::default() };
    match module {
        Module::Shield | Module::Reactor => m.crystal = 3 * level as u32,
        Module::WarpCore => {
            m.crystal = 2 * level as u32;
            m.datacore = level.saturating_sub(2) as u32;
        }
        Module::Weapons | Module::Scanner => m.datacore = level.saturating_sub(3) as u32 * 2,
        _ => {}
    }
    if level >= 7 {
        m.relic = (level - 6) as u32;
    }
    m
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

    pub fn stats(&self) -> Stats {
        let b = self.hull.base();
        let lv = |m: Module| (self.level(m) as f32 - 1.0).max(0.0);
        let engine = 1.0 + 0.07 * lv(Module::Engine);
        let reactor = lv(Module::Reactor);
        Stats {
            max_hull: b.hull * (1.0 + 0.15 * lv(Module::Hull)),
            max_shield: b.shield * (1.0 + 0.18 * lv(Module::Shield)),
            shield_regen: b.shield * (0.06 + 0.012 * reactor),
            speed: b.speed * engine,
            accel: b.accel * engine,
            turn: b.turn * (1.0 + 0.05 * lv(Module::Engine)),
            warp_range: b.warp + 0.55 * lv(Module::WarpCore),
            damage: b.damage * (1.0 + 0.14 * lv(Module::Weapons)),
            fire_interval: b.fire_interval / (1.0 + 0.05 * reactor),
            weapon_range: b.weapon_range * (1.0 + 0.03 * lv(Module::Weapons)),
            missile_damage: b.missile_damage * (1.0 + 0.14 * lv(Module::Weapons)),
            missile_cooldown: 7.0 / (1.0 + 0.04 * reactor),
            scan_range: b.scan_range * (1.0 + 0.12 * lv(Module::Scanner)),
            scan_time: 4.0 / (1.0 + 0.1 * lv(Module::Scanner)),
            mine_rate: b.mine_rate * (1.0 + 0.1 * lv(Module::Scanner)),
            cargo: b.cargo * (1.0 + 0.25 * lv(Module::Cargo)),
            radius: b.radius * (1.0 + 0.04 * self.rank() as f32),
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

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
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

//! The galaxy: ten hand-placed star systems (the historical core, [`CORE`]) and more grown around
//! them from `tuning/galaxy.ron`; every system's contents (planets, belts, stations, anomalies,
//! ruins, palettes) are generated deterministically from its id. Server and client both call
//! [`systems`] and [`system_layout`] with the same tuning and get identical worlds.

use crate::rng::Rng;
use glam::Vec3;
use serde::{Deserialize, Serialize};

pub type SystemId = u16;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum RiskTier {
    /// Safe space. No PvP.
    Core,
    /// NPC pirates. No PvP.
    Frontier,
    /// PvP allowed, but attacking an innocent makes you Wanted.
    Wild,
    /// Open fleet warfare. No penalties.
    WarZone,
    /// Highest risk, highest reward.
    Deep,
}

impl RiskTier {
    pub fn pvp_allowed(self) -> bool {
        matches!(self, RiskTier::Wild | RiskTier::WarZone | RiskTier::Deep)
    }

    pub fn crime_tracked(self) -> bool {
        matches!(self, RiskTier::Wild)
    }

    pub fn pirate_level(self) -> u8 {
        match self {
            RiskTier::Core => 0,
            RiskTier::Frontier => 1,
            RiskTier::Wild => 2,
            RiskTier::WarZone => 3,
            RiskTier::Deep => 4,
        }
    }

    /// Reward multiplier for scans, artifacts and battle wins (`tuning/economy.ron` `risk_reward`).
    pub fn reward_mult(self) -> f32 {
        crate::tuning::get().economy.risk_reward[&self]
    }

    /// Pirate bounty multiplier (`tuning/economy.ron` `bounty_risk`).
    pub fn bounty_mult(self) -> f32 {
        crate::tuning::get().economy.bounty_risk[&self]
    }

    pub fn label(self) -> &'static str {
        match self {
            RiskTier::Core => "CORE · 안전",
            RiskTier::Frontier => "FRONTIER · 해적 출몰",
            RiskTier::Wild => "WILD · PvP (범죄 기록)",
            RiskTier::WarZone => "WAR ZONE · 함대전",
            RiskTier::Deep => "DEEP SPACE · 최고 위험",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Species {
    Human,
    Veyr,
    Kharz,
}

impl Species {
    pub const ALL: [Species; 3] = [Species::Human, Species::Veyr, Species::Kharz];

    pub fn name(self) -> &'static str {
        match self {
            Species::Human => "Human",
            Species::Veyr => "Veyr",
            Species::Kharz => "Kharz",
        }
    }

    pub fn motto(self) -> &'static str {
        match self {
            Species::Human => "역사 = 기록. 문서와 로그가 진실을 증명한다.",
            Species::Veyr => "역사 = 기억. 집단 기억이 기록보다 오래 남는다.",
            Species::Kharz => "역사 = 계약과 혈통. 피로 맺은 약속이 권리를 만든다.",
        }
    }

    pub fn home_system(self) -> SystemId {
        match self {
            Species::Human => 0,
            Species::Veyr => 1,
            Species::Kharz => 2,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum StarKind {
    YellowDwarf,
    RedGiant,
    BlueGiant,
    WhiteDwarf,
    Neutron,
}

impl StarKind {
    pub fn color(self) -> [f32; 3] {
        match self {
            StarKind::YellowDwarf => [1.0, 0.86, 0.55],
            StarKind::RedGiant => [1.0, 0.45, 0.25],
            StarKind::BlueGiant => [0.55, 0.72, 1.0],
            StarKind::WhiteDwarf => [0.92, 0.95, 1.0],
            StarKind::Neutron => [0.7, 0.55, 1.0],
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct SystemDef {
    pub id: SystemId,
    pub name: &'static str,
    pub tier: RiskTier,
    /// Galaxy map position in light-years (x, z) plus slight height.
    pub map_pos: [f32; 3],
    pub star: StarKind,
    pub lore: &'static str,
}

/// The hand-made historical core, ids 0–9. Generated systems follow from id `CORE.len()`.
pub const CORE: &[SystemDef] = &[
    SystemDef { id: 0, name: "Sol Haven", tier: RiskTier::Core, map_pos: [0.0, 0.0, 0.0], star: StarKind::YellowDwarf, lore: "인류 연합의 수도 성계. 제국 기록보관소가 있다." },
    SystemDef { id: 1, name: "Lumen Reach", tier: RiskTier::Core, map_pos: [-1.3, 0.2, 0.7], star: StarKind::BlueGiant, lore: "Veyr 기억의 집들이 공명하는 청색 성운." },
    SystemDef { id: 2, name: "Kharz Forge", tier: RiskTier::Core, map_pos: [1.2, -0.1, 0.8], star: StarKind::RedGiant, lore: "Kharz 전사 가문들의 조선소. 계약 길드 본부." },
    SystemDef { id: 3, name: "Orion Drift", tier: RiskTier::Frontier, map_pos: [0.2, 0.1, 1.6], star: StarKind::YellowDwarf, lore: "희귀 광맥이 발견된 변경 성계. 해적이 노린다." },
    SystemDef { id: 4, name: "Vesta Corridor", tier: RiskTier::Frontier, map_pos: [1.6, 0.3, 2.1], star: StarKind::WhiteDwarf, lore: "호송선단이 지나는 항로. 오래된 전쟁의 잔해." },
    SystemDef { id: 5, name: "Cinder Expanse", tier: RiskTier::Wild, map_pos: [-0.9, -0.1, 2.0], star: StarKind::RedGiant, lore: "법이 닿지 않는 붉은 황무지. 현상금 사냥꾼의 무대." },
    SystemDef { id: 6, name: "Hollow Crown", tier: RiskTier::Wild, map_pos: [-0.4, 0.4, 3.3], star: StarKind::Neutron, lore: "고대 문명의 왕관 유적이 떠도는 중성자성 성계." },
    SystemDef { id: 7, name: "Vesta Front", tier: RiskTier::WarZone, map_pos: [1.7, -0.2, 3.3], star: StarKind::BlueGiant, lore: "Vesta 전쟁의 최전선. 함대끼리 끝없이 충돌한다." },
    SystemDef { id: 8, name: "Ashen Gate", tier: RiskTier::WarZone, map_pos: [0.5, 0.1, 4.2], star: StarKind::RedGiant, lore: "Kharz 군벌이 장악한 관문 성계." },
    SystemDef { id: 9, name: "The Deep", tier: RiskTier::Deep, map_pos: [-0.2, 0.6, 6.6], star: StarKind::Neutron, lore: "고대 워프 게이트 너머. 아무도 돌아오지 못했다." },
];

pub fn system(id: SystemId) -> &'static SystemDef {
    let all = systems();
    &all[id as usize % all.len()]
}

/// Every system: the core, then the generated ones (ids are indices).
pub fn systems() -> &'static [SystemDef] {
    if let Some(s) = GALAXY.read().unwrap().current {
        return s;
    }
    let t = crate::tuning::get();
    let mut w = GALAXY.write().unwrap();
    // Reinstalling the same numbers keeps the galaxy (and does not leak another copy).
    let s = match &w.built {
        Some((g, s)) if *g == t.galaxy => *s,
        _ => {
            let s: &'static [SystemDef] = Box::leak(generate(&t.galaxy).into_boxed_slice());
            w.built = Some((t.galaxy.clone(), s));
            s
        }
    };
    w.current = Some(s);
    s
}

struct Galaxy {
    current: Option<&'static [SystemDef]>,
    built: Option<(crate::tuning::GalaxyTuning, &'static [SystemDef])>,
}

static GALAXY: std::sync::RwLock<Galaxy> = std::sync::RwLock::new(Galaxy { current: None, built: None });

/// New numbers were installed: the next `systems` call checks whether the galaxy changed.
pub(crate) fn tuning_changed() {
    GALAXY.write().unwrap().current = None;
}

/// Grows `g.generated` systems outwards from the core, each a hop from one already placed.
fn generate(g: &crate::tuning::GalaxyTuning) -> Vec<SystemDef> {
    let mut out: Vec<SystemDef> = CORE.to_vec();
    let deep = Vec3::from(CORE[9].map_pos);
    let mut r = Rng::from_parts(&[0x6A1A_C7, g.seed]);
    let stars: Vec<(StarKind, u32)> = g.stars.iter().map(|(k, w)| (*k, *w)).collect();
    let total: u32 = stars.iter().map(|s| s.1).sum();
    let (x0, x1, z0, z1) = g.bounds;
    let mut names: std::collections::BTreeSet<String> = CORE.iter().map(|s| s.name.to_string()).collect();
    let mut firsts: Vec<String> = Vec::new();
    let mut tries = 0;
    while out.len() < CORE.len() + g.generated as usize && tries < 100_000 {
        tries += 1;
        // Grow from any placed system but The Deep.
        let from = loop {
            let i = r.below(out.len() as u32) as usize;
            if i != 9 {
                break Vec3::from(out[i].map_pos);
            }
        };
        let ang = r.range(0.0, std::f32::consts::TAU);
        let d = r.range(g.hop.0, g.hop.1);
        let p = Vec3::new(from.x + ang.cos() * d, r.range(-0.4, 0.6), from.z + ang.sin() * d);
        if p.x < x0 || p.x > x1 || p.z < z0 || p.z > z1 || p.distance(deep) < g.deep_clearance {
            continue;
        }
        if out.iter().any(|s| Vec3::from(s.map_pos).distance(p) < if is_generated(s.id) { g.min_spacing } else { g.core_clearance }) {
            continue;
        }
        let from_sol = Vec3::new(p.x, 0.0, p.z).length();
        let tier = g.tiers.iter().find(|(d, _)| from_sol <= *d).unwrap_or(g.tiers.last().unwrap()).1;
        let mut roll = r.below(total);
        let star = stars.iter().find(|(_, w)| if roll < *w { true } else { roll -= w; false }).unwrap().0;
        if firsts.is_empty() {
            firsts = g.names_first.clone();
        }
        let first = firsts.remove(r.below(firsts.len() as u32) as usize);
        let mut name = format!("{first} {}", r.pick(&g.names_second));
        let base = name.clone();
        let mut n = 2;
        while names.contains(&name) {
            name = format!("{base} {}", roman(n));
            n += 1;
        }
        names.insert(name.clone());
        let lore = r.pick(&g.lore[&tier]).clone();
        out.push(SystemDef { id: out.len() as SystemId, name: Box::leak(name.into_boxed_str()), tier, map_pos: p.into(), star, lore: Box::leak(lore.into_boxed_str()) });
    }
    out
}

fn roman(n: u32) -> &'static str {
    ["", "I", "II", "III", "IV", "V", "VI", "VII", "VIII", "IX", "X"].get(n as usize).copied().unwrap_or("X+")
}

/// A system grown by the generator rather than placed by hand.
pub fn is_generated(id: SystemId) -> bool {
    id as usize >= CORE.len()
}

pub fn distance_ly(a: SystemId, b: SystemId) -> f32 {
    let (pa, pb) = (system(a).map_pos, system(b).map_pos);
    Vec3::from(pa).distance(Vec3::from(pb))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlanetKind {
    Rocky,
    Ocean,
    Gas,
    Ice,
    Lava,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Planet {
    pub pos: Vec3,
    pub radius: f32,
    pub kind: PlanetKind,
    pub seed: u32,
    pub ring: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Ore {
    Ferrite,
    Lumen,
    Xenite,
}

impl Ore {
    pub const ALL: [Ore; 3] = [Ore::Ferrite, Ore::Lumen, Ore::Xenite];
    pub fn name(self) -> &'static str {
        match self {
            Ore::Ferrite => "Ferrite",
            Ore::Lumen => "Lumen",
            Ore::Xenite => "Xenite",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Asteroid {
    pub id: u32,
    pub pos: Vec3,
    pub radius: f32,
    pub ore: Ore,
    pub seed: u32,
    /// The exact mineral (`minerals::deposit`).
    pub mineral: crate::minerals::MineralId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PoiKind {
    /// Scannable anomaly: data cores.
    Anomaly,
    /// Ancient ruin: relics and history.
    Ruin,
    /// Derelict hulk: alloy and logs.
    Derelict,
    /// Unknown signal (scenario beacons).
    Signal,
    /// Ancient warp gate.
    AncientGate,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Poi {
    pub id: u32,
    pub kind: PoiKind,
    pub name: String,
    pub pos: Vec3,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Layout {
    pub id: SystemId,
    pub star_radius: f32,
    pub planets: Vec<Planet>,
    pub asteroids: Vec<Asteroid>,
    pub station: Vec3,
    pub jump_point: Vec3,
    pub pois: Vec<Poi>,
    /// Two nebula colours + density for the skybox shader.
    pub nebula: [[f32; 3]; 2],
    pub nebula_density: f32,
}

pub const STAR_POS: Vec3 = Vec3::ZERO;
pub const DOCK_RANGE: f32 = 220.0;
pub const MINE_RANGE: f32 = 320.0;

/// Where the star sits relative to the playable area: the action happens around
/// the station 0..3000 u away from the star so the sun is always on screen as a backdrop.
pub fn system_layout(id: SystemId) -> Layout {
    let def = system(id);
    let mut r = Rng::from_parts(&[0x5747_A11, id as u64]);
    let star_radius = match def.star {
        StarKind::RedGiant | StarKind::BlueGiant => 900.0,
        StarKind::YellowDwarf => 600.0,
        _ => 260.0,
    };

    let tuning = crate::tuning::get();
    let rel = tuning.galaxy.relief;
    let station = Vec3::new(r.range(-200., 200.), r.range(-rel.station_y, rel.station_y), 3400.0);
    let jump_point = station + Vec3::new(r.range(-700., 700.), r.range(-rel.jump_y, rel.jump_y), r.range(500., 900.));

    let mut planets = Vec::new();
    let n_planets = 2 + r.below(3);
    for i in 0..n_planets {
        let ang = r.range(0., std::f32::consts::TAU);
        let dist = 2600.0 + i as f32 * 2400.0 + r.range(0., 900.);
        let kind = *r.pick(&[PlanetKind::Rocky, PlanetKind::Ocean, PlanetKind::Gas, PlanetKind::Ice, PlanetKind::Lava]);
        let radius = match kind {
            PlanetKind::Gas => r.range(420., 700.),
            _ => r.range(140., 320.),
        };
        planets.push(Planet {
            pos: Vec3::new(ang.cos() * dist, r.range(-400., 400.), ang.sin() * dist + 1800.0),
            radius,
            kind,
            seed: r.below(1_000_000),
            ring: kind == PlanetKind::Gas && r.f32() < 0.7,
        });
    }

    let mut asteroids = Vec::new();
    let belts = if def.tier == RiskTier::Core { 1 } else { 2 };
    let mut aid = 1;
    for b in 0..belts {
        let center = station + Vec3::new(if b == 0 { -1300.0 } else { 1400.0 }, r.range(-rel.belt_y, rel.belt_y), r.range(-500., 900.));
        let count = 26 + r.below(14);
        for _ in 0..count {
            let p = center + r.in_sphere() * Vec3::new(520., rel.belt_thickness, 520.);
            let roll = r.f32() * (1.0 + def.tier.pirate_level() as f32 * 0.15);
            let ore = if roll > 1.0 { Ore::Xenite } else if roll > 0.62 { Ore::Lumen } else { Ore::Ferrite };
            let mineral = crate::minerals::deposit(&tuning.minerals, id, def.star, def.tier, b, aid, ore, climb_deg(station, p));
            asteroids.push(Asteroid { id: aid, pos: p, radius: r.range(10., 42.), ore, seed: r.below(1_000_000), mineral });
            aid += 1;
        }
    }

    let mut pois = Vec::new();
    let mut pid = 1000;
    let mut push_poi = |r: &mut Rng, kind: PoiKind, name: String, near: Vec3, spread: f32| {
        let p = near + Vec3::new(r.range(-spread, spread), r.range(-spread * rel.poi_rise, spread * rel.poi_rise), r.range(-spread, spread));
        pois.push(Poi { id: pid, kind, name, pos: p });
        pid += 1;
    };
    let n_anom = 1 + def.tier.pirate_level() as u32 / 2 + r.below(2);
    for i in 0..n_anom {
        push_poi(&mut r, PoiKind::Anomaly, format!("{} 이상현상 {}", def.name, greek(i)), station, 1800.0);
    }
    if def.tier != RiskTier::Core {
        let hull_no = 100 + r.below(800);
        push_poi(&mut r, PoiKind::Derelict, format!("버려진 호송선 {hull_no}"), station, 1500.0);
    }
    if matches!(id, 6 | 9) || r.f32() < 0.35 {
        for i in 0..(if id == 6 { 3 } else { 1 }) {
            push_poi(&mut r, PoiKind::Ruin, format!("고대 유적 {}", greek(i)), station + Vec3::new(0., 0., 1600.), 1300.0);
        }
    }
    if id == 9 {
        push_poi(&mut r, PoiKind::AncientGate, "The Gate Beyond".into(), station + Vec3::new(0., 200., 2400.), 50.0);
    }
    // Scenario beacons live in specific systems.
    if matches!(id, 3 | 4 | 8) {
        push_poi(&mut r, PoiKind::Signal, "미확인 신호".into(), station + Vec3::new(900., 0., 1200.), 300.0);
    }

    let hue = |r: &mut Rng, base: [f32; 3]| -> [f32; 3] { [(base[0] * r.range(0.4, 1.0)).min(1.0), (base[1] * r.range(0.3, 1.0)).min(1.0), (base[2] * r.range(0.5, 1.0)).min(1.0)] };
    let sc = def.star.color();
    let nebula = [hue(&mut r, [sc[2], sc[0] * 0.6, sc[1]]), hue(&mut r, [sc[0], sc[1] * 0.5, 1.0])];
    let nebula_density = 0.35 + def.tier.pirate_level() as f32 * 0.1 + r.range(0., 0.2);

    Layout { id, star_radius, planets, asteroids, station, jump_point, pois, nebula, nebula_density }
}

fn greek(i: u32) -> &'static str {
    ["Alpha", "Beta", "Gamma", "Delta", "Epsilon"][i as usize % 5]
}

/// Starfall coordinates.
/// - Galactic: light-years on the galaxy chart (x east, y up, z rimward), origin at Sol Haven.
/// - Sector: the chart is cut into 1 ly squares named column letter (x, A at -3 ly) + row number
///   (z, 1 at 0 ly), e.g. Sol Haven sits in `D1`.
/// - Local: inside a system, world units are meters from the star; shown in km.
pub mod coords {
    use super::*;

    pub const SECTOR_ORIGIN_X: f32 = -3.0;

    pub fn sector_of(map_pos: [f32; 3]) -> String {
        let col = ((map_pos[0] - SECTOR_ORIGIN_X).floor() as i32).clamp(0, 25) as u8;
        let row = map_pos[2].floor() as i32 + 1;
        format!("{}{}", (b'A' + col) as char, row)
    }

    /// Sector square bounds on the chart (x0, z0, x1, z1) in ly.
    pub fn sector_bounds(name: &str) -> Option<[f32; 4]> {
        let mut c = name.chars();
        let col = c.next()? as i32 - 'A' as i32;
        let row: i32 = c.as_str().parse().ok()?;
        let x0 = SECTOR_ORIGIN_X + col as f32;
        let z0 = (row - 1) as f32;
        Some([x0, z0, x0 + 1.0, z0 + 1.0])
    }

    /// Bearing (0° = chart north, i.e. world -Z, clockwise) and elevation of a direction, degrees.
    pub fn bearing(dir: Vec3) -> (f32, f32) {
        let d = dir.normalize_or(Vec3::NEG_Z);
        let b = d.x.atan2(-d.z).to_degrees().rem_euclid(360.0);
        (b, d.y.clamp(-1.0, 1.0).asin().to_degrees())
    }

    /// One-line position readout: `D1 · Sol Haven · L(+1.23, -0.40, +3.40) km`.
    pub fn describe(sys: SystemId, local: Vec3) -> String {
        let def = system(sys);
        format!("{} · {} · L({:+.2}, {:+.2}, {:+.2}) km", sector_of(def.map_pos), def.name, local.x / 1000.0, local.y / 1000.0, local.z / 1000.0)
    }
}

/// Climb from the station to each place a captain flies to (jump point, points of interest,
/// asteroids): degrees above (+) or below (−) the system plane. A flat layout keeps these near 0.
pub fn route_elevations(l: &Layout) -> Vec<f32> {
    let dest = std::iter::once(l.jump_point).chain(l.pois.iter().map(|p| p.pos)).chain(l.asteroids.iter().map(|a| a.pos));
    dest.map(|p| climb_deg(l.station, p)).collect()
}

/// Degrees above (+) or below (−) the system plane of the straight route from `from` to `to`.
pub fn climb_deg(from: Vec3, to: Vec3) -> f32 {
    let d = to - from;
    d.y.atan2(Vec3::new(d.x, 0.0, d.z).length()).to_degrees()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steep_asteroids_hold_rarer_deposits() {
        // A reason to use height: in every tier with rare deposits, asteroids 20°+ above or below the
        // station are richer than the flat ones.
        let m = &crate::tuning::get().minerals;
        for tier in [RiskTier::Frontier, RiskTier::Wild, RiskTier::WarZone] {
            let (mut flat, mut steep) = ((0u32, 0u32), (0u32, 0u32));
            for s in systems().iter().filter(|s| s.tier == tier) {
                let l = system_layout(s.id);
                for (a, e) in l.asteroids.iter().zip(route_elevations(&l).into_iter().skip(1 + l.pois.len())) {
                    let rare = crate::minerals::mineral_in(m, a.mineral).unwrap().rarity >= crate::minerals::Rarity::Rare;
                    let c = if e.abs() >= 20.0 { &mut steep } else { &mut flat };
                    *c = (c.0 + rare as u32, c.1 + 1);
                }
            }
            let share = |c: (u32, u32)| c.0 as f32 / c.1.max(1) as f32;
            assert!(share(steep) > share(flat) * 1.3, "{tier:?}: rare+ share steep {:.2} vs flat {:.2}", share(steep), share(flat));
        }
    }

    #[test]
    fn places_rise_out_of_the_plane() {
        // Main goal "3D freedom": flying to work means climbing and diving, not a flat map.
        let e: Vec<f32> = systems().iter().flat_map(|s| route_elevations(&system_layout(s.id))).collect();
        let steep = e.iter().filter(|a| a.abs() >= 20.0).count() as f32 / e.len() as f32;
        let (up, down) = (e.iter().filter(|a| **a >= 20.0).count(), e.iter().filter(|a| **a <= -20.0).count());
        assert!(steep >= 0.25, "only {:.0}% of routes climb or dive 20°+", steep * 100.0);
        assert!(up > 0 && down > 0 && up.min(down) * 3 > up.max(down), "both above and below: {up} up, {down} down");
    }

    #[test]
    fn layouts_are_deterministic() {
        for s in systems() {
            let a = system_layout(s.id);
            let b = system_layout(s.id);
            assert_eq!(a.asteroids.len(), b.asteroids.len());
            assert_eq!(a.station, b.station);
            assert_eq!(a.pois.len(), b.pois.len());
        }
    }

    #[test]
    fn deep_needs_upgraded_warp() {
        // The Deep is far from every other system: a fresh ship cannot reach it.
        let nearest = systems().iter().filter(|s| s.id != 9).map(|s| distance_ly(s.id, 9)).fold(f32::MAX, f32::min);
        assert!(nearest > 2.3, "nearest {nearest}");
        // …but every system is reachable from some neighbour with a level-1 Wren (2.0 ly) except The Deep.
        for s in systems().iter().filter(|s| s.id != 9) {
            let n = systems().iter().filter(|o| o.id != s.id).map(|o| distance_ly(s.id, o.id)).fold(f32::MAX, f32::min);
            assert!(n <= 1.5, "{} isolated ({n})", s.name);
        }
    }

    #[test]
    fn generated_galaxy() {
        let all = systems();
        let g = &crate::tuning::get().galaxy;
        assert_eq!(all.len(), CORE.len() + g.generated as usize, "every generated system found room");
        for (i, s) in all.iter().enumerate() {
            assert_eq!(s.id as usize, i);
        }
        // The core is untouched and names are unique.
        assert_eq!((all[9].name, all[9].map_pos), (CORE[9].name, CORE[9].map_pos));
        let names: std::collections::BTreeSet<&str> = all.iter().map(|s| s.name).collect();
        assert_eq!(names.len(), all.len());
        assert!(all[CORE.len()..].iter().all(|s| s.tier != RiskTier::Core && s.tier != RiskTier::Deep));
        // Same numbers, same galaxy.
        assert_eq!(generate(g).iter().map(|s| (s.name, s.map_pos)).collect::<Vec<_>>(), all.iter().map(|s| (s.name, s.map_pos)).collect::<Vec<_>>());
    }

    #[test]
    fn scenario_systems_have_needed_pois() {
        assert!(system_layout(6).pois.iter().filter(|p| p.kind == PoiKind::Ruin).count() >= 3);
        assert!(system_layout(9).pois.iter().any(|p| p.kind == PoiKind::AncientGate));
        for s in [3, 4, 8] {
            assert!(system_layout(s).pois.iter().any(|p| p.kind == PoiKind::Signal));
        }
        assert!(system_layout(4).pois.iter().any(|p| p.kind == PoiKind::Derelict));
    }

    #[test]
    fn coordinates() {
        assert_eq!(coords::sector_of(system(0).map_pos), "D1");
        assert_eq!(coords::sector_of(system(9).map_pos), "C7");
        let b = coords::sector_bounds("D1").unwrap();
        assert!(b[0] <= 0.0 && 0.0 < b[2] && b[1] <= 0.0 && 0.0 < b[3]);
        // Every system lies inside its own sector square.
        for s in systems() {
            let b = coords::sector_bounds(&coords::sector_of(s.map_pos)).unwrap();
            assert!(b[0] <= s.map_pos[0] && s.map_pos[0] < b[2] && b[1] <= s.map_pos[2] && s.map_pos[2] < b[3], "{}", s.name);
        }
        let (n, _) = coords::bearing(Vec3::NEG_Z);
        let (e, _) = coords::bearing(Vec3::X);
        let (_, up) = coords::bearing(Vec3::Y);
        assert!(n.abs() < 1e-3 && (e - 90.0).abs() < 1e-3 && (up - 90.0).abs() < 1e-3);
        assert!(coords::describe(0, Vec3::new(1230.0, -400.0, 3400.0)).contains("L(+1.23, -0.40, +3.40) km"));
    }
}

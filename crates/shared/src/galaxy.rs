//! The galaxy: hand-placed star systems whose contents (planets, belts, stations,
//! anomalies, ruins, palettes) are generated deterministically from the system seed.
//! Server and client both call [`system_layout`] and get identical worlds.

use crate::rng::Rng;
use glam::Vec3;
use serde::{Deserialize, Serialize};

pub type SystemId = u16;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
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

    pub fn reward_mult(self) -> f32 {
        match self {
            RiskTier::Core => 0.6,
            RiskTier::Frontier => 1.0,
            RiskTier::Wild => 1.5,
            RiskTier::WarZone => 1.8,
            RiskTier::Deep => 2.6,
        }
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
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

pub struct SystemDef {
    pub id: SystemId,
    pub name: &'static str,
    pub tier: RiskTier,
    /// Galaxy map position in light-years (x, z) plus slight height.
    pub map_pos: [f32; 3],
    pub star: StarKind,
    pub lore: &'static str,
}

pub const SYSTEMS: &[SystemDef] = &[
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
    &SYSTEMS[id as usize % SYSTEMS.len()]
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

    let station = Vec3::new(r.range(-200., 200.), r.range(-60., 60.), 3400.0);
    let jump_point = station + Vec3::new(r.range(-700., 700.), r.range(-120., 120.), r.range(500., 900.));

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
        let center = station + Vec3::new(if b == 0 { -1300.0 } else { 1400.0 }, r.range(-150., 150.), r.range(-500., 900.));
        let count = 26 + r.below(14);
        for _ in 0..count {
            let p = center + r.in_sphere() * Vec3::new(520., 140., 520.);
            let roll = r.f32() * (1.0 + def.tier.pirate_level() as f32 * 0.15);
            let ore = if roll > 1.0 { Ore::Xenite } else if roll > 0.62 { Ore::Lumen } else { Ore::Ferrite };
            asteroids.push(Asteroid { id: aid, pos: p, radius: r.range(10., 42.), ore, seed: r.below(1_000_000) });
            aid += 1;
        }
    }

    let mut pois = Vec::new();
    let mut pid = 1000;
    let mut push_poi = |r: &mut Rng, kind: PoiKind, name: String, near: Vec3, spread: f32| {
        let p = near + Vec3::new(r.range(-spread, spread), r.range(-spread * 0.2, spread * 0.2), r.range(-spread, spread));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layouts_are_deterministic() {
        for s in SYSTEMS {
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
        let nearest = SYSTEMS.iter().filter(|s| s.id != 9).map(|s| distance_ly(s.id, 9)).fold(f32::MAX, f32::min);
        assert!(nearest > 2.3, "nearest {nearest}");
        // …but every system is reachable from some neighbour with a level-1 Wren (2.0 ly) except The Deep.
        for s in SYSTEMS.iter().filter(|s| s.id != 9) {
            let n = SYSTEMS.iter().filter(|o| o.id != s.id).map(|o| distance_ly(s.id, o.id)).fold(f32::MAX, f32::min);
            assert!(n <= 1.5, "{} isolated ({n})", s.name);
        }
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
        for s in SYSTEMS {
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

//! The mineral catalogue (design doc §7.1–7.2): ten thousand minerals nobody wrote by hand.
//!
//! Ids are laid out family by family (Ferrite, Lumen, Xenite), rarity by rarity within a family, using
//! the counts in `tuning/minerals.ron`; a mineral's name and properties follow from the seed and its id.
//! Every asteroid holds one mineral ([`deposit`]): its rarity rolls by the system's risk tier, and the
//! mineral is the best fit for the star among a few candidates, shared by the whole belt — so a belt is
//! known for its minerals and the same conditions always give the same rocks.

use crate::galaxy::{Ore, RiskTier, StarKind, SystemId};
use crate::rng::Rng;
use crate::tuning::MineralTuning;
use serde::{Deserialize, Serialize};

pub type MineralId = u32;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Rarity {
    Common,
    Uncommon,
    Rare,
    Exotic,
    Legendary,
}

impl Rarity {
    pub const ALL: [Rarity; 5] = [Rarity::Common, Rarity::Uncommon, Rarity::Rare, Rarity::Exotic, Rarity::Legendary];

    pub fn label(self) -> &'static str {
        match self {
            Rarity::Common => "일반",
            Rarity::Uncommon => "고급",
            Rarity::Rare => "희귀",
            Rarity::Exotic => "이색",
            Rarity::Legendary => "전설",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Property {
    Density,
    Hardness,
    Energy,
    Thermal,
    Radiation,
    Conductivity,
    Warp,
    Weapon,
    Industrial,
}

impl Property {
    pub const ALL: [Property; 9] = [Property::Density, Property::Hardness, Property::Energy, Property::Thermal, Property::Radiation, Property::Conductivity, Property::Warp, Property::Weapon, Property::Industrial];

    pub fn label(self) -> &'static str {
        match self {
            Property::Density => "밀도",
            Property::Hardness => "경도",
            Property::Energy => "에너지 용량",
            Property::Thermal => "내열성",
            Property::Radiation => "방사능",
            Property::Conductivity => "전도성",
            Property::Warp => "워프 적합성",
            Property::Weapon => "무기 적합성",
            Property::Industrial => "산업 가치",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Mineral {
    pub id: MineralId,
    pub family: Ore,
    pub rarity: Rarity,
    pub name: String,
    /// 1–100, in [`Property::ALL`] order.
    pub props: [u8; 9],
    /// 0–100: what traders pay for it relative to its rarity (from industrial, energy, warp and weapon use).
    pub market: u8,
}

impl Mineral {
    pub fn prop(&self, p: Property) -> u8 {
        self.props[p as usize]
    }

    /// Station price of one specimen (none below Rare).
    pub fn specimen_value(&self, t: &MineralTuning) -> u64 {
        let base = t.specimen_value.get(&self.rarity).copied().unwrap_or(0) as f32;
        (base * (0.5 + self.market as f32 / 100.0)).round() as u64
    }

    /// "Karvelite (희귀 Xenite)".
    pub fn label(&self) -> String {
        format!("{} ({} {})", self.name, self.rarity.label(), self.family.name())
    }
}

fn family_size(t: &MineralTuning) -> u32 {
    t.counts.values().sum()
}

/// Every mineral in the catalogue.
pub fn catalogue_size(t: &MineralTuning) -> u32 {
    family_size(t) * Ore::ALL.len() as u32
}

/// The ids of one family and rarity.
pub fn bucket(t: &MineralTuning, family: Ore, rarity: Rarity) -> std::ops::Range<MineralId> {
    let mut start = family as u32 * family_size(t);
    for r in Rarity::ALL {
        let n = t.counts.get(&r).copied().unwrap_or(0);
        if r == rarity {
            return start..start + n;
        }
        start += n;
    }
    start..start
}

/// A mineral from the current tuning (`None` past the end of the catalogue).
pub fn mineral(id: MineralId) -> Option<Mineral> {
    mineral_in(&crate::tuning::get().minerals, id)
}

pub fn mineral_in(t: &MineralTuning, id: MineralId) -> Option<Mineral> {
    let fs = family_size(t);
    if fs == 0 || id >= catalogue_size(t) {
        return None;
    }
    let family = Ore::ALL[(id / fs) as usize];
    let index = id % fs;
    let rarity = Rarity::ALL.into_iter().find(|r| bucket(t, family, *r).contains(&id))?;
    let mut r = Rng::from_parts(&[0x313E_7A15, t.seed, id as u64]);
    let mean = t.property_mean.get(&rarity).copied().unwrap_or(30) as f32;
    let favoured = t.family_affinity.get(&family).cloned().unwrap_or_default();
    let mut props = [0u8; 9];
    for p in Property::ALL {
        let bonus = if favoured.contains(&p) { t.affinity_bonus as f32 } else { 0.0 };
        props[p as usize] = (mean + bonus + r.range(-(t.spread as f32), t.spread as f32)).round().clamp(1.0, 100.0) as u8;
    }
    let use_value = [Property::Industrial, Property::Energy, Property::Warp, Property::Weapon].iter().map(|p| props[*p as usize] as f32).sum::<f32>() / 4.0;
    let market = use_value.round().clamp(1.0, 100.0) as u8;
    Some(Mineral { id, family, rarity, name: name(t, family, index), props, market })
}

/// Names are a bijection of the index within the family over syllable × syllable × ending, so no two
/// minerals of a family share one (validated: the combinations outnumber the family).
fn name(t: &MineralTuning, family: Ore, index: u32) -> String {
    let s = &t.syllables;
    let endings = t.endings.get(&family).map(|e| e.as_slice()).unwrap_or(&[]);
    let n = (s.len() * s.len() * endings.len().max(1)) as u64;
    let mut stride = 1777u64;
    while gcd(stride, n) != 1 {
        stride += 2;
    }
    let k = (index as u64 * stride + t.seed) % n;
    let (a, b, e) = (k as usize % s.len(), (k as usize / s.len()) % s.len(), k as usize / (s.len() * s.len()));
    let second = s[b].to_lowercase();
    format!("{}{}{}", s[a], second, endings.get(e).map(|x| x.as_str()).unwrap_or(""))
}

fn gcd(a: u64, b: u64) -> u64 {
    if b == 0 { a } else { gcd(b, a % b) }
}

/// The mineral in asteroid `asteroid` of belt `belt` in system `sys` (ore family `ore`).
/// `elev_deg`: the asteroid's climb from the station (`galaxy::route_elevations`); steep ones are richer
/// (`height_bonus`).
#[allow(clippy::too_many_arguments)]
pub fn deposit(t: &MineralTuning, sys: SystemId, star: StarKind, tier: RiskTier, belt: u32, asteroid: u32, ore: Ore, elev_deg: f32) -> MineralId {
    let lift = t.height_bonus.factor(elev_deg);
    let weights: Vec<(Rarity, u32)> = t.deposit.get(&tier).map(|w| w.iter().map(|(r, n)| (*r, if *r < Rarity::Rare { *n } else { (*n as f32 * lift).round() as u32 })).collect()).unwrap_or_default();
    let total: u32 = weights.iter().map(|w| w.1).sum();
    let mut r = Rng::from_parts(&[0xDE90_5175, t.seed, sys as u64, asteroid as u64]);
    let mut roll = r.below(total.max(1));
    let rarity = weights.iter().find(|(_, w)| if roll < *w { true } else { roll -= w; false }).map(|w| w.0).unwrap_or(Rarity::Common);
    // The belt's mineral of that family and rarity: the best of a few candidates for this star.
    let ids = bucket(t, ore, rarity);
    if ids.is_empty() {
        return bucket(t, ore, Rarity::Common).start;
    }
    let favoured = t.star_affinity.get(&star).copied().unwrap_or(Property::Industrial);
    let mut b = Rng::from_parts(&[0xBE17, t.seed, sys as u64, belt as u64, ore as u64, rarity as u64]);
    let mut best = (ids.start, 0u8);
    for _ in 0..t.candidates.max(1) {
        let id = ids.start + b.below(ids.end - ids.start);
        let v = mineral_in(t, id).map(|m| m.prop(favoured)).unwrap_or(0);
        if v > best.1 {
            best = (id, v);
        }
    }
    best.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalogue() {
        let t = &crate::tuning::get().minerals;
        assert!(catalogue_size(t) >= 10_000, "design doc: 10,000+ minerals");
        for f in Ore::ALL {
            let mut names = std::collections::BTreeSet::new();
            for r in Rarity::ALL {
                for id in bucket(t, f, r) {
                    let m = mineral_in(t, id).unwrap();
                    assert_eq!((m.family, m.rarity), (f, r));
                    assert!(m.props.iter().all(|p| (1..=100).contains(p)));
                    names.insert(m.name);
                }
            }
            assert_eq!(names.len() as u32, family_size(t), "{f:?}: names are unique within a family");
        }
        assert_eq!(mineral_in(t, catalogue_size(t)), None);
        // Same id, same mineral.
        assert_eq!(mineral_in(t, 4242), mineral_in(t, 4242));
    }

    #[test]
    fn deposits_follow_risk_and_star() {
        let t = &crate::tuning::get().minerals;
        let rarity = |tier| {
            (0..400).map(|a| mineral_in(t, deposit(t, 3, StarKind::Neutron, tier, a % 2, a, Ore::Xenite, 0.0)).unwrap().rarity).collect::<Vec<_>>()
        };
        let core = rarity(RiskTier::Core);
        assert!(core.iter().all(|r| *r <= Rarity::Uncommon), "the core has no rare deposits");
        let deep = rarity(RiskTier::Deep);
        assert!(deep.iter().filter(|r| **r >= Rarity::Rare).count() > core.len() / 3);
        // A belt's rare Xenite is one mineral, rich in what a neutron star favours.
        let rare: std::collections::BTreeSet<MineralId> = (0..400).map(|a| deposit(t, 3, StarKind::Neutron, RiskTier::Deep, 0, a, Ore::Xenite, 0.0)).filter(|id| mineral_in(t, *id).unwrap().rarity == Rarity::Rare).collect();
        assert_eq!(rare.len(), 1);
        let m = mineral_in(t, *rare.first().unwrap()).unwrap();
        assert!(m.prop(Property::Radiation) as u32 > t.property_mean[&Rarity::Rare], "{m:?}");
    }
}

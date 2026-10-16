//! Named artifacts dug out of ruins and ancient gates (design doc §21 "Archaeology & Artifact Economy").
//!
//! Every ruin or gate holds one artifact per UTC day, generated deterministically from the site and
//! the day, with names and values from `tuning/artifacts.ron`. Only Archaeologists can excavate them.
//! An artifact keeps its provenance — who found it, who appraised, carried, lost, recovered, sold or
//! donated it — so its story is part of the galaxy's history.

use crate::chronicle::{BioEntry, galactic_date};
use crate::galaxy::{self, PoiKind, SystemId};
use crate::rng::Rng;
use serde::{Deserialize, Serialize};

pub type ArtifactId = u64;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Culture {
    Human,
    Veyr,
    Kharz,
    /// The builders of the ancient gates.
    Precursor,
}

impl Culture {
    pub const ALL: [Culture; 4] = [Culture::Human, Culture::Veyr, Culture::Kharz, Culture::Precursor];

    pub fn name(self) -> &'static str {
        match self {
            Culture::Human => "인류",
            Culture::Veyr => "Veyr",
            Culture::Kharz => "Kharz",
            Culture::Precursor => "선구자",
        }
    }
}

/// Where an artifact is now.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Holder {
    Player(u64),
    /// Went down with a ship; whoever investigates that wreck recovers it (store wreck index).
    Wreck(usize),
    /// On display at a station museum.
    Museum(SystemId),
    /// Sold to collectors at a station market.
    Sold(SystemId),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Artifact {
    pub id: ArtifactId,
    /// True name, known once appraised (`label` shows what others see before that).
    pub name: String,
    pub culture: Culture,
    pub origin_system: SystemId,
    pub origin_site: String,
    /// Credits a collector pays once it is appraised.
    pub value: u64,
    pub appraised: bool,
    pub holder: Holder,
    pub provenance: Vec<BioEntry>,
}

impl Artifact {
    /// What the holder sees: the true name once appraised, otherwise an unidentified find.
    pub fn label(&self) -> String {
        if self.appraised { self.name.clone() } else { format!("미감정 유물 ({})", self.origin_site) }
    }

    pub fn note(&mut self, unix: u64, text: String) {
        self.provenance.push(BioEntry { date: galactic_date(unix), text, event: None });
    }
}

/// Sites that hold artifacts.
pub fn yields_artifacts(kind: PoiKind) -> bool {
    matches!(kind, PoiKind::Ruin | PoiKind::AncientGate)
}

/// The artifact a site gives up on a given day: same site and day → same artifact.
pub fn excavate(id: ArtifactId, system: SystemId, site: &str, kind: PoiKind, poi: u32, day: u32, holder: u64, unix: u64) -> Artifact {
    let t = crate::tuning::get();
    let a = &t.artifacts;
    let mut r = Rng::from_parts(&[0xA27_1FAC7, system as u64, poi as u64, day as u64]);
    let culture = if kind == PoiKind::AncientGate { Culture::Precursor } else { [Culture::Human, Culture::Veyr, Culture::Kharz][r.below(3) as usize] };
    let c = &a.cultures[&culture];
    let noun = r.pick(&c.nouns).clone();
    let epithet = r.pick(&c.epithets).clone();
    let tier = galaxy::system(system).tier.reward_mult().max(1.0);
    let spread = 1.0 + a.value_spread * (r.f32() * 2.0 - 1.0);
    let value = (c.value as f32 * tier * spread).round() as u64;
    let mut art = Artifact {
        id,
        name: format!("{}의 {epithet} {noun}", galaxy::system(system).name),
        culture,
        origin_system: system,
        origin_site: site.to_string(),
        value,
        appraised: false,
        holder: Holder::Player(holder),
        provenance: vec![],
    };
    art.note(unix, format!("{}의 {site}에서 발굴되었다.", galaxy::system(system).name));
    art
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn excavation_is_deterministic_per_site_and_day() {
        let a = excavate(1, 6, "고대 유적 Alpha", PoiKind::Ruin, 3, 100, 9, 1_800_000_000);
        let b = excavate(1, 6, "고대 유적 Alpha", PoiKind::Ruin, 3, 100, 9, 1_800_000_000);
        assert_eq!(a, b);
        assert!(!a.appraised && a.label().starts_with("미감정 유물"));
        assert!(a.name.starts_with("Hollow Crown의 "), "{}", a.name);
        assert!(a.value > 0);
        let days: Vec<String> = (100..110).map(|d| excavate(1, 6, "x", PoiKind::Ruin, 3, d, 9, 0).name).collect();
        assert!(days.iter().any(|n| *n != a.name), "a new day brings a different find");
        assert_eq!(excavate(1, 9, "gate", PoiKind::AncientGate, 1, 100, 9, 0).culture, Culture::Precursor);
    }
}

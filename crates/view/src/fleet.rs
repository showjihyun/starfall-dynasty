//! The four fleet styles (docs/ART_DIRECTION.md §4, stage 3): who flies which, and the model of each
//! ship in it. A cruiser or capital is built to its people's design; small ships stay their hull
//! family's (the hull a captain chose still shows), so a style has no small model.
//!
//! | style | flown by | Blender source (tools/blender/styles/) |
//! |---|---|---|
//! | Human | Human captains and their escorts | `global`, `global_cruiser` |
//! | Veyr | Veyr captains and their escorts | `japan`, `japan_cruiser` |
//! | Kharz | Kharz captains and their escorts | `west`, `west_cruiser` |
//! | Aster | the ancient leviathan that guards the ruins | `unknown` |

use shared::galaxy::Species;
use shared::missions::NpcArchetype;
use shared::protocol::EntityKind;
use shared::ships::SizeClass;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FleetStyle {
    /// Flowing spearhead hulls, broad paint sweeps, bands of windows.
    Human,
    /// Battleship lines: a bow cannon, stacked turrets, a pagoda bridge, white armour.
    Veyr,
    /// Armoured citadels, hangar pods, gun decks, bare structure.
    Kharz,
    /// Not built by any living people: obsidian blades around a lit core, engraved light.
    Aster,
}

impl FleetStyle {
    pub const ALL: [FleetStyle; 4] = [FleetStyle::Human, FleetStyle::Veyr, FleetStyle::Kharz, FleetStyle::Aster];

    pub fn of_species(s: Species) -> FleetStyle {
        match s {
            Species::Human => FleetStyle::Human,
            Species::Veyr => FleetStyle::Veyr,
            Species::Kharz => FleetStyle::Kharz,
        }
    }

    /// The style a ship is built in and the size it is built at: a captain's or an escort's by its
    /// species and growth stage, the Aster leviathan its own. None for everything else (pirates and
    /// the lesser guardians have models of their own; ships without a species their hull family's).
    pub fn of(kind: EntityKind, species: Option<Species>, class: Option<SizeClass>) -> Option<(FleetStyle, SizeClass)> {
        match kind {
            EntityKind::Player | EntityKind::Escort => Some((FleetStyle::of_species(species?), class.unwrap_or(SizeClass::Small))),
            EntityKind::Npc(NpcArchetype::Sentinel) => Some((FleetStyle::Aster, SizeClass::Capital)),
            _ => None,
        }
    }

    /// Model (file stem under assets/models) and its nominal length in metres, as
    /// `tools/blender/build_styles.py --game` exports it. None where the style has no ship of that size.
    pub fn model(self, class: SizeClass) -> Option<(&'static str, f32)> {
        Some(match (self, class) {
            (FleetStyle::Human, SizeClass::Capital) => ("human_capital", 420.0),
            (FleetStyle::Veyr, SizeClass::Capital) => ("veyr_capital", 420.0),
            (FleetStyle::Kharz, SizeClass::Capital) => ("kharz_capital", 420.0),
            (FleetStyle::Human, SizeClass::Cruiser) => ("human_cruiser", 150.0),
            (FleetStyle::Veyr, SizeClass::Cruiser) => ("veyr_cruiser", 150.0),
            (FleetStyle::Kharz, SizeClass::Cruiser) => ("kharz_cruiser", 150.0),
            // The leviathan: one ship, as long as its radius asks (tuning/npcs.ron Sentinel).
            (FleetStyle::Aster, SizeClass::Capital) => ("npc_sentinel", 130.0),
            (FleetStyle::Aster, SizeClass::Cruiser) | (_, SizeClass::Small) => return None,
        })
    }

    /// Every model the style ships with.
    pub fn models(self) -> impl Iterator<Item = (&'static str, f32)> {
        [SizeClass::Cruiser, SizeClass::Capital].into_iter().filter_map(move |c| self.model(c))
    }

    /// The Blender style the models are built from (tools/blender/styles/NAME.py).
    pub fn source(self) -> &'static str {
        match self {
            FleetStyle::Human => "global",
            FleetStyle::Veyr => "japan",
            FleetStyle::Kharz => "west",
            FleetStyle::Aster => "unknown",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn there_are_four_styles_and_each_has_its_capital() {
        assert_eq!(FleetStyle::ALL.len(), 4);
        let capitals: Vec<&str> = FleetStyle::ALL.iter().map(|s| s.model(SizeClass::Capital).expect("a capital").0).collect();
        assert_eq!(capitals, ["human_capital", "veyr_capital", "kharz_capital", "npc_sentinel"]);
        // No two ships share a model, and none is built from another's source.
        let mut all: Vec<&str> = FleetStyle::ALL.iter().flat_map(|s| s.models()).map(|(n, _)| n).collect();
        assert_eq!(all.len(), 7, "three fleets with a cruiser and a capital, and the leviathan");
        all.sort();
        all.dedup();
        assert_eq!(all.len(), 7);
        let mut sources: Vec<&str> = FleetStyle::ALL.iter().map(|s| s.source()).collect();
        sources.sort();
        assert_eq!(sources, ["global", "japan", "unknown", "west"]);
    }

    #[test]
    fn every_people_flies_its_own_style_and_the_leviathan_is_the_asters() {
        for s in Species::ALL {
            let style = FleetStyle::of_species(s);
            assert_eq!(format!("{style:?}"), format!("{s:?}"));
            for kind in [EntityKind::Player, EntityKind::Escort] {
                assert_eq!(FleetStyle::of(kind, Some(s), Some(SizeClass::Cruiser)), Some((style, SizeClass::Cruiser)));
                assert_eq!(FleetStyle::of(kind, Some(s), None), Some((style, SizeClass::Small)), "no stage yet: a small ship");
            }
            assert!(style.model(SizeClass::Cruiser).is_some() && style.model(SizeClass::Capital).is_some());
            assert_eq!(style.model(SizeClass::Small), None, "small ships are their hull family's");
        }
        assert_eq!(FleetStyle::of(EntityKind::Npc(NpcArchetype::Sentinel), None, None), Some((FleetStyle::Aster, SizeClass::Capital)));
        // Pirates and the lesser guardian keep their own models; a ship without a species its hull family's.
        for npc in [NpcArchetype::Raider, NpcArchetype::Gunship, NpcArchetype::Warden, NpcArchetype::Warlord] {
            assert_eq!(FleetStyle::of(EntityKind::Npc(npc), None, None), None);
        }
        assert_eq!(FleetStyle::of(EntityKind::Player, None, Some(SizeClass::Capital)), None);
    }
}

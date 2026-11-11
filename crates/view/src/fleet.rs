//! The four fleet styles (docs/ART_DIRECTION.md §4, stage 3): who flies which, and the model of each
//! ship in it. Every ship of a people — small ship, cruiser, capital — is built to that people's
//! design; the hull a captain chose decides how it flies (tuning/ships.ron), not how it looks.
//!
//! | style | flown by | Blender source (tools/blender/styles/) |
//! |---|---|---|
//! | Human | Human captains and their escorts | `global_small`, `global_cruiser`, `global` |
//! | Veyr | Veyr captains and their escorts | `japan_small`, `japan_cruiser`, `japan` |
//! | Kharz | Kharz captains and their escorts | `west_small`, `west_cruiser`, `west` |
//! | Aster | the ancient guardians of the ruins: wardens and the leviathan | `unknown_small`, `unknown` |

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
    /// species and growth stage, the Aster guardians their own (wardens small, the leviathan a capital).
    /// None for everything else (pirates have models of their own; ships without a species their hull family's).
    pub fn of(kind: EntityKind, species: Option<Species>, class: Option<SizeClass>) -> Option<(FleetStyle, SizeClass)> {
        match kind {
            EntityKind::Player | EntityKind::Escort => Some((FleetStyle::of_species(species?), class.unwrap_or(SizeClass::Small))),
            EntityKind::Npc(NpcArchetype::Sentinel) => Some((FleetStyle::Aster, SizeClass::Capital)),
            EntityKind::Npc(NpcArchetype::Warden) => Some((FleetStyle::Aster, SizeClass::Small)),
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
            (FleetStyle::Human, SizeClass::Small) => ("human_small", 24.0),
            (FleetStyle::Veyr, SizeClass::Small) => ("veyr_small", 24.0),
            (FleetStyle::Kharz, SizeClass::Small) => ("kharz_small", 24.0),
            // The guardians: the warden and the leviathan, as long as their radii ask (tuning/npcs.ron).
            (FleetStyle::Aster, SizeClass::Small) => ("npc_warden", 26.0),
            (FleetStyle::Aster, SizeClass::Capital) => ("npc_sentinel", 130.0),
            (FleetStyle::Aster, SizeClass::Cruiser) => return None,
        })
    }

    /// A big hull's beam and height as shares of its length, measured off the model (the client test
    /// `fleet_hull_proportions_match_the_models` keeps this table honest). Small ships have none: with
    /// their wings they keep the one wide shield and hull shape every fighter wears.
    pub fn proportions(self, class: SizeClass) -> Option<[f32; 2]> {
        Some(match (self, class) {
            (FleetStyle::Human, SizeClass::Capital) => [0.44, 0.25],
            (FleetStyle::Veyr, SizeClass::Capital) => [0.43, 0.30],
            (FleetStyle::Kharz, SizeClass::Capital) => [0.38, 0.25],
            (FleetStyle::Human, SizeClass::Cruiser) => [0.45, 0.24],
            (FleetStyle::Veyr, SizeClass::Cruiser) => [0.29, 0.26],
            (FleetStyle::Kharz, SizeClass::Cruiser) => [0.39, 0.25],
            (FleetStyle::Aster, SizeClass::Capital) => [0.46, 0.35],
            (FleetStyle::Aster, SizeClass::Cruiser) | (_, SizeClass::Small) => return None,
        })
    }

    /// Every model the style ships with.
    pub fn models(self) -> impl Iterator<Item = (&'static str, f32)> {
        [SizeClass::Small, SizeClass::Cruiser, SizeClass::Capital].into_iter().filter_map(move |c| self.model(c))
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

/// A ship is drawn 2.2 radii long (client `sync_visuals`): its half-length in radii.
pub const HALF_LENGTH: f32 = 1.1;
/// Half-extents (across, up, along; in ship radii) taken for a hull whose model has no measured
/// proportions: winged small ships and the pirates.
pub const DEFAULT_HALF: [f32; 3] = [0.42, 0.24, 1.0];
/// The shield bubble of such a ship, in radii: wide for its wings.
pub const DEFAULT_SHIELD: [f32; 3] = [1.15, 0.7, 1.35];

/// Half-extents of a ship's hull in ship radii (across, up, along): where shots land on it
/// (`anim::impact_point`) and what its shield has to clear.
pub fn hull_half(kind: EntityKind, species: Option<Species>, class: Option<SizeClass>) -> [f32; 3] {
    FleetStyle::of(kind, species, class).and_then(|(style, class)| style.proportions(class)).map_or(DEFAULT_HALF, |[beam, height]| [beam * HALF_LENGTH, height * HALF_LENGTH, HALF_LENGTH])
}

/// The shield bubble's half-axes in ship radii: it hugs a fleet-style hull (a margin around its
/// beam, height and length) instead of the one wide shape every ship used to wear, which stood
/// far off a long warship's flanks.
pub fn shield_shape(kind: EntityKind, species: Option<Species>, class: Option<SizeClass>) -> [f32; 3] {
    match FleetStyle::of(kind, species, class).and_then(|(style, class)| style.proportions(class)) {
        Some(_) => {
            let [x, y, z] = hull_half(kind, species, class);
            [x * 1.4, y * 1.6, z * 1.2]
        }
        None => DEFAULT_SHIELD,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shield_hugs_a_fleet_hull_and_small_ships_keep_theirs() {
        for s in Species::ALL {
            for class in [SizeClass::Cruiser, SizeClass::Capital] {
                let half = hull_half(EntityKind::Player, Some(s), Some(class));
                let shield = shield_shape(EntityKind::Escort, Some(s), Some(class));
                for k in 0..3 {
                    assert!(shield[k] > half[k] * 1.15, "clear of the hull: {s:?} {class:?}");
                }
                assert!(shield[0] < 0.8 && shield[1] < 0.6, "and close to it: {shield:?}");
                assert_eq!(half[2], HALF_LENGTH);
            }
            assert_eq!(shield_shape(EntityKind::Player, Some(s), Some(SizeClass::Small)), DEFAULT_SHIELD);
            assert_eq!(hull_half(EntityKind::Player, Some(s), None), DEFAULT_HALF);
        }
        assert_eq!(shield_shape(EntityKind::Npc(NpcArchetype::Raider), None, None), DEFAULT_SHIELD);
        let lev = shield_shape(EntityKind::Npc(NpcArchetype::Sentinel), None, None);
        assert!(lev[0] < DEFAULT_SHIELD[0] && lev[2] < DEFAULT_SHIELD[2] + 0.01, "{lev:?}");
    }

    #[test]
    fn there_are_four_styles_and_each_has_its_capital() {
        assert_eq!(FleetStyle::ALL.len(), 4);
        let capitals: Vec<&str> = FleetStyle::ALL.iter().map(|s| s.model(SizeClass::Capital).expect("a capital").0).collect();
        assert_eq!(capitals, ["human_capital", "veyr_capital", "kharz_capital", "npc_sentinel"]);
        // No two ships share a model, and none is built from another's source.
        let mut all: Vec<&str> = FleetStyle::ALL.iter().flat_map(|s| s.models()).map(|(n, _)| n).collect();
        assert_eq!(all.len(), 11, "three fleets of three sizes, the warden and the leviathan");
        all.sort();
        all.dedup();
        assert_eq!(all.len(), 11);
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
            for class in [SizeClass::Small, SizeClass::Cruiser, SizeClass::Capital] {
                assert!(style.model(class).is_some(), "{style:?} {class:?}");
            }
        }
        assert_eq!(FleetStyle::of(EntityKind::Npc(NpcArchetype::Sentinel), None, None), Some((FleetStyle::Aster, SizeClass::Capital)));
        assert_eq!(FleetStyle::of(EntityKind::Npc(NpcArchetype::Warden), None, None), Some((FleetStyle::Aster, SizeClass::Small)));
        assert_eq!(FleetStyle::Aster.model(SizeClass::Small), Some(("npc_warden", 26.0)));
        // Pirates keep their own models; a ship without a species its hull family's.
        for npc in [NpcArchetype::Raider, NpcArchetype::Gunship, NpcArchetype::Warlord] {
            assert_eq!(FleetStyle::of(EntityKind::Npc(npc), None, None), None);
        }
        assert_eq!(FleetStyle::of(EntityKind::Player, None, Some(SizeClass::Capital)), None);
    }
}

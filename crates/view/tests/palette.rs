//! Colour rules from docs/ART_DIRECTION.md that code can check.

use shared::galaxy::Species;
use view::palette::{HOSTILE_LIVERY, LIVERIES, PLAYER_LIVERIES, species_livery};

#[test]
fn each_species_has_its_own_selectable_default_paint() {
    let picks: Vec<u8> = Species::ALL.iter().map(|s| species_livery(*s)).collect();
    for (i, p) in picks.iter().enumerate() {
        assert!((*p as usize) < PLAYER_LIVERIES, "{:?} default must be one the captain can pick", Species::ALL[i]);
        assert_ne!(*p, HOSTILE_LIVERY, "pirate red is never a captain's paint");
        assert!(!picks[..i].contains(p), "{:?} shares a default with another species", Species::ALL[i]);
    }
    // Kharz reads as brass/gold, not red (red means hostile, §3.4).
    let k = LIVERIES[species_livery(Species::Kharz) as usize];
    assert!(k[1] > 0.5, "Kharz default {k:?} too red");
    // Human keeps the old default (navy) so existing captains' look is unchanged in spirit.
    assert_eq!(species_livery(Species::Human), 1);
}

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

/// §12.1: interface colours come only from `palette.rs`. A numeric `Color32` literal anywhere else in
/// the view or client sources fails here with its file and line.
#[test]
fn no_ui_colour_literals_outside_the_palette() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut found = vec![];
    for dir in ["view/src", "client/src"] {
        let mut stack = vec![root.join(dir)];
        while let Some(d) = stack.pop() {
            for e in std::fs::read_dir(&d).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                } else if p.extension().is_some_and(|x| x == "rs") && p.file_name().is_some_and(|n| n != "palette.rs") {
                    for (i, line) in std::fs::read_to_string(&p).unwrap().lines().enumerate() {
                        for ctor in ["Color32::from_rgb(", "Color32::from_rgba_unmultiplied(", "Color32::from_rgba_premultiplied(", "Color32::from_gray("] {
                            if let Some(k) = line.find(ctor)
                                && line[k + ctor.len()..].trim_start().starts_with(|c: char| c.is_ascii_digit())
                            {
                                found.push(format!("{}:{}: {}", p.display(), i + 1, line.trim()));
                            }
                        }
                    }
                }
            }
        }
    }
    assert!(found.is_empty(), "colour literals outside palette.rs:\n{}", found.join("\n"));
}

#[test]
fn every_hull_readout_shares_one_ramp() {
    use view::palette::{CAUTION, GREEN, hull_color};
    assert_eq!(hull_color(0.8, 1.0), GREEN);
    assert_eq!(hull_color(0.5, 1.0), CAUTION);
    assert_eq!(hull_color(0.2, 1.0), view::palette::RED, "steady red at full pulse");
}

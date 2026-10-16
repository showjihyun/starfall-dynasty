//! Gameplay numbers come from `tuning/*.ron`, not the code (docs/HARNESS.md): changing one value
//! changes the outcome of a scenario, and restoring it restores the exact state.
//! Its own test binary because it swaps the process-wide tuning.

use sim::scenario;

#[test]
fn tuning_drives_the_simulation() {
    let spec = scenario::load(std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/scenarios/fighter_duel_gunship.json"))).unwrap();
    let original = (*shared::tuning::get()).clone();
    let base = scenario::run(&spec, None).digest;

    let mut t = original.clone();
    t.combat.shield_bleed = 0.5;
    t.npcs.archetypes.get_mut(&shared::missions::NpcArchetype::Gunship).unwrap().hull *= 2.0;
    shared::tuning::install(t);
    let changed = scenario::run(&spec, None).digest;
    assert_ne!(changed, base, "a tuning edit must reach the simulation");

    shared::tuning::install(original);
    assert_eq!(scenario::run(&spec, None).digest, base, "same numbers, same outcome");
}

//! The long-war harness (`sim::war`) on a small, short run: the alliances take ground and the run
//! repeats exactly (it is the measure for every politics change, so it must be deterministic).

use sim::war::{Options, run};

#[test]
fn alliances_take_ground_and_the_war_run_repeats() {
    let o = Options { alliances: 2, captains: 2, hours: 1.0, ..Default::default() };
    let a = run(&o);
    let b = run(&o);
    assert_eq!(a.hours.len(), 1);
    assert!(a.hours[0].held.iter().all(|h| *h >= 1), "each alliance holds ground after an hour: {:?}", a.hours[0].held);
    assert_eq!(a.hours[0].held, b.hours[0].held);
    assert_eq!(a.events, b.events);
    assert_eq!(a.kills, b.kills);
}

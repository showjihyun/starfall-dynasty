//! The campaign playthrough bot (`sim::playthrough`) plays like a player and is deterministic.
//! The full run (`cargo run -p sim --release --example campaign`) takes minutes; these check the
//! first stretch of it in seconds.

use sim::playthrough::{Options, play};

fn short() -> Options {
    Options { max_secs: 15.0 * 60.0, ..Options::default() }
}

#[test]
fn a_fresh_captain_flies_chapter_one_without_dev_commands() {
    let rep = play(&short());
    let c = &rep.chapters[0];
    assert_eq!(c.id, 0, "{}", rep.text());
    // Warped to Orion Drift, scanned the signal and met the wave: all three steps were reached.
    for want in ["step 1", "step 2", "step 3"] {
        assert!(c.log.iter().any(|l| l.contains(want)), "never reached {want}:\n{}", rep.text());
    }
    // Losing twice sends the captain to earn upgrades, and it buys some.
    assert!(c.upgrades > 0, "{}", rep.text());
    assert!(c.log.iter().any(|l| l.contains("retreats")), "{}", rep.text());
}

#[test]
fn the_same_playthrough_twice_reads_the_same() {
    assert_eq!(play(&short()).text(), play(&short()).text());
}

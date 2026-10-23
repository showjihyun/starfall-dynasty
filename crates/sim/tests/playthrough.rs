//! The campaign playthrough bot (`sim::playthrough`) plays like a player and is deterministic.
//! The full run (`cargo run -p sim --release --example campaign`) takes minutes; these check the
//! first stretch of it in seconds.

use sim::playthrough::{Options, play};

fn short() -> Options {
    Options { max_secs: 15.0 * 60.0, ..Options::default() }
}

#[test]
fn a_fresh_captain_clears_chapter_one_first_try() {
    // Campaign QA once found chapter 1 a wall: a new ship could not beat its pirates and spent
    // 2 h 25 min earning upgrades first (docs/QA_CAMPAIGN.md). Every hull clears it now.
    for hull in shared::ships::HullClass::ALL {
        let rep = play(&Options { hull, only: true, ..short() });
        let c = &rep.chapters[0];
        for want in ["step 1", "step 2", "step 3"] {
            assert!(c.log.iter().any(|l| l.contains(want)), "{hull:?} never reached {want}:\n{}", rep.text());
        }
        assert!(c.finished.is_some_and(|f| f - c.started < 5.0 * 60.0), "{hull:?}:\n{}", rep.text());
        assert_eq!(c.deaths + c.grind_deaths, 0, "{hull:?}:\n{}", rep.text());
    }
}

#[test]
fn the_same_playthrough_twice_reads_the_same() {
    assert_eq!(play(&short()).text(), play(&short()).text());
}

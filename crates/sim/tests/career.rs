//! The open-galaxy pilot (`sim::career`) can live off the boards: a short career takes and
//! finishes couriers' and surveyors' work, and trade runs pay, deterministically.

use shared::jobs::JobTag;
use shared::ships::HullClass;
use sim::career::{Options, run};

#[test]
fn a_short_career_finishes_hauls_and_surveys_and_trade_pays() {
    let o = Options { level: 5, kinds: vec![JobTag::Haul, JobTag::Survey], secs: 20.0 * 60.0, ..Default::default() };
    let r = run(&o);
    let done = |k| r.jobs.get(&k).map_or(0, |t| t.done);
    assert!(done(JobTag::Haul) + done(JobTag::Survey) >= 5, "{}", r.text());
    assert!(r.credits > 0, "{}", r.text());
    // Same options, same career.
    assert_eq!(run(&o).credits, r.credits);
    let t = run(&Options { hull: HullClass::Mule, level: 5, kinds: vec![], trade: true, credits: Some(20_000), secs: 10.0 * 60.0, ..Default::default() });
    assert!(t.trade.done >= 5 && t.trade.credits > 0, "{}", t.text());
}

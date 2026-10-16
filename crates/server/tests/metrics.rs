//! Gameplay metrics (docs/HARNESS.md): collecting them never changes the simulation, they are
//! deterministic, and the time series adds up to the totals.

use server::scenario;

fn spec(name: &str) -> scenario::Scenario {
    scenario::load(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("scenarios").join(format!("{name}.json"))).unwrap()
}

#[test]
fn metrics_never_change_the_outcome() {
    for name in ["fighter_duel_gunship", "capital_vs_raiders", "wild_crime_bounty"] {
        let sc = spec(name);
        let with = scenario::run_opts(&sc, None, true);
        let without = scenario::run_opts(&sc, None, false);
        assert_eq!(with.digest, without.digest, "{name}");
        assert!(without.metrics.is_empty() && !with.metrics.is_empty());
        assert_eq!(with.metrics, scenario::run(&sc, None).metrics, "{name}: metrics are deterministic");
    }
}

#[test]
fn time_series_adds_up_to_the_totals() {
    let r = scenario::run(&spec("fighter_duel_gunship"), None);
    let lines: Vec<serde_json::Value> = r.metrics.iter().map(|l| serde_json::from_str(l).unwrap()).collect();
    let (_, tot) = &r.totals[0];
    let samples: Vec<&serde_json::Value> = lines.iter().filter(|l| l["ev"] == "sample").collect();
    assert_eq!(samples.len(), 45, "one sample per second for the captain");
    let dealt: f64 = samples.iter().map(|s| s["dealt"].as_f64().unwrap()).sum();
    let taken: f64 = samples.iter().map(|s| s["taken"].as_f64().unwrap()).sum();
    assert!((dealt - tot.dealt as f64).abs() < 0.5, "{dealt} vs {}", tot.dealt);
    assert!((taken - tot.taken as f64).abs() < 0.5, "{taken} vs {}", tot.taken);
    // The gunship died once, to Wing, and its bounty was paid.
    let kills: Vec<&serde_json::Value> = lines.iter().filter(|l| l["ev"] == "kill").collect();
    assert_eq!(kills.len(), 1);
    assert_eq!(kills[0]["killer"], "Wing");
    assert!(kills[0]["victim"].as_str().unwrap().starts_with("Gunship"));
    assert_eq!(tot.kills, 1);
    assert!(lines.iter().any(|l| l["ev"] == "reward" && l["credits"].as_u64() == Some(tot.credits_earned)));
    // Killing it took at least its whole hull and shield.
    let (hull, shield, ..) = shared::missions::NpcArchetype::Gunship.stats(1);
    assert!(tot.dealt >= hull + shield, "{} < {}", tot.dealt, hull + shield);
    // Hull fraction in the samples only falls while under fire and ends where the run ends.
    let last = samples.last().unwrap();
    assert_eq!(last["hull"].as_f64().unwrap(), (r.final_hull[0].1 as f64 * 100.0).round() / 100.0);
}

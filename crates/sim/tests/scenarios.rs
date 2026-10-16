//! Every gameplay spec in `crates/sim/scenarios/*.json` must pass, and run deterministically.
//! Adding a test is adding a JSON file (docs/HARNESS.md).

use sim::scenario;

#[test]
fn all_scenarios_pass_deterministically() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("scenarios");
    let files = scenario::collect(&dir);
    assert!(!files.is_empty(), "no scenarios in {}", dir.display());
    let mut failures = vec![];
    for f in &files {
        let sc = scenario::load(f).unwrap_or_else(|e| panic!("{e}"));
        let a = scenario::run(&sc, None);
        let b = scenario::run(&sc, None);
        if !a.ok() {
            failures.push(format!("{}:\n    {}", a.name, a.lines.join("\n    ")));
        }
        if a.digest != b.digest {
            failures.push(format!("{}: nondeterministic ({} vs {})", a.name, a.digest, b.digest));
        }
    }
    assert!(failures.is_empty(), "{} of {} scenarios failed:\n{}", failures.len(), files.len(), failures.join("\n"));
}

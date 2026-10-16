//! Run gameplay scenario specs headless (docs/HARNESS.md).
//!
//! `cargo run -p server --release --example scenario -- [FILE|DIR]... [--trace SECS] [--print-digest] [--metrics DIR]`
//!
//! Prints every check with its measured value and a one-line verdict per scenario; exits 1 if any fail.
//! `--print-digest` shows the final-state fingerprint to pin in the file's `"digest"` field.
//! `--metrics DIR` writes `DIR/<name>.jsonl`: per-captain samples every second, kills and rewards.

use server::scenario;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let trace = args.windows(2).find(|w| w[0] == "--trace").and_then(|w| w[1].parse::<f32>().ok());
    let print_digest = args.iter().any(|a| a == "--print-digest");
    let metrics_dir = args.windows(2).find(|w| w[0] == "--metrics").map(|w| std::path::PathBuf::from(&w[1]));
    let mut paths: Vec<std::path::PathBuf> = vec![];
    let mut skip = false;
    for a in &args {
        if skip {
            skip = false;
            continue;
        }
        if a == "--trace" || a == "--metrics" {
            skip = true;
            continue;
        }
        if a.starts_with("--") {
            continue;
        }
        paths.extend(scenario::collect(std::path::Path::new(a)));
    }
    if paths.is_empty() {
        paths = scenario::collect(std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/scenarios")));
    }
    println!("tuning {}", shared::tuning::Tuning::locate().map(|d| d.display().to_string()).unwrap_or_default());
    let mut failed = 0;
    for p in &paths {
        let sc = match scenario::load(p) {
            Ok(s) => s,
            Err(e) => {
                println!("ERROR {e}");
                failed += 1;
                continue;
            }
        };
        let t0 = std::time::Instant::now();
        let rep = scenario::run(&sc, trace);
        println!("== {} — {}", rep.name, sc.description);
        for l in &rep.lines {
            println!("  {l}");
        }
        println!(
            "{} {}: {} passed, {} failed ({:.0} s simulated in {:.2} s){}",
            if rep.ok() { "OK  " } else { "FAIL" },
            rep.name,
            rep.passed,
            rep.failed,
            sc.run_secs,
            t0.elapsed().as_secs_f64(),
            if print_digest { format!(" digest {}", rep.digest) } else { String::new() }
        );
        for (name, t) in &rep.totals {
            if t.hits_dealt + t.hits_taken > 0 {
                let ttk = t.mean_ttk().map(|v| format!(" · mean ttk {v:.1} s")).unwrap_or_default();
                println!("  metrics {name}: dealt {:.0} ({} hits, {:.1} dps) · taken {:.0} ({} hits) · kills {}{ttk} · bounty {} CR", t.dealt, t.hits_dealt, t.dps(), t.taken, t.hits_taken, t.kills, t.credits_earned);
            }
        }
        if let Some(dir) = &metrics_dir {
            std::fs::create_dir_all(dir).expect("metrics dir");
            let path = dir.join(format!("{}.jsonl", rep.name));
            std::fs::write(&path, rep.metrics.join("\n") + "\n").expect("write metrics");
            println!("  wrote {} ({} lines)", path.display(), rep.metrics.len());
        }
        if !rep.ok() {
            failed += 1;
        }
    }
    println!("{} scenario(s), {} failed", paths.len(), failed);
    if failed > 0 {
        std::process::exit(1);
    }
}

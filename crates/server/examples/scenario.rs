//! Run gameplay scenario specs headless (docs/HARNESS.md).
//!
//! `cargo run -p server --release --example scenario -- [FILE|DIR]... [--trace SECS] [--print-digest]`
//!
//! Prints every check with its measured value and a one-line verdict per scenario; exits 1 if any fail.
//! `--print-digest` shows the final-state fingerprint to pin in the file's `"digest"` field.

use server::scenario;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let trace = args.windows(2).find(|w| w[0] == "--trace").and_then(|w| w[1].parse::<f32>().ok());
    let print_digest = args.iter().any(|a| a == "--print-digest");
    let mut paths: Vec<std::path::PathBuf> = vec![];
    let mut skip = false;
    for a in &args {
        if skip {
            skip = false;
            continue;
        }
        if a == "--trace" {
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
        if !rep.ok() {
            failed += 1;
        }
    }
    println!("{} scenario(s), {} failed", paths.len(), failed);
    if failed > 0 {
        std::process::exit(1);
    }
}

//! Replay a recorded session headless and verify it reproduces (docs/HARNESS.md).
//!
//! `cargo run -p sim --release --example replay -- DIR [--dump SYSTEM@TICK]... [--twice]`
//!
//! - Prints one verdict line: `REPLAY OK` when every recorded digest matched, otherwise
//!   `REPLAY DIVERGED at tick T` with both digests (exit code 1).
//! - `--dump 7@1200` prints that system's entity table at that tick (text, for agents and bug reports).
//! - `--twice` replays a second time and checks both runs end in the same state (determinism).

use sim::harness::{Session, dump_system};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(dir) = args.first() else {
        eprintln!("usage: replay DIR [--dump SYSTEM@TICK]... [--twice]");
        std::process::exit(2);
    };
    let dumps: Vec<(u16, u64)> = args
        .windows(2)
        .filter(|w| w[0] == "--dump")
        .filter_map(|w| {
            let (s, t) = w[1].split_once('@')?;
            Some((s.parse().ok()?, t.parse().ok()?))
        })
        .collect();
    let session = Session::load(std::path::Path::new(dir)).unwrap_or_else(|e| {
        eprintln!("cannot load {dir}: {e}");
        std::process::exit(2);
    });
    let t0 = std::time::Instant::now();
    let report = session
        .replay(|g| {
            for (sys, tick) in &dumps {
                if g.tick == *tick {
                    print!("{}", dump_system(g, *sys));
                }
            }
        })
        .expect("replay");
    let secs = t0.elapsed().as_secs_f64();
    println!(
        "replayed {} ticks ({:.0} s of play) in {:.2} s — {} commands, {} digests checked, final state {}",
        report.ticks,
        report.ticks as f64 / shared::protocol::TICK_HZ as f64,
        secs,
        report.events,
        report.digests_checked,
        report.final_digest
    );
    println!("{}", report.verdict());
    let mut ok = report.divergence.is_none();
    if args.iter().any(|a| a == "--twice") {
        let again = session.replay(|_| {}).expect("replay");
        if again.final_digest == report.final_digest {
            println!("DETERMINISTIC: second replay ended in the same state");
        } else {
            println!("NONDETERMINISTIC: second replay ended in {} (first {})", again.final_digest, report.final_digest);
            ok = false;
        }
    }
    if !ok {
        std::process::exit(1);
    }
}

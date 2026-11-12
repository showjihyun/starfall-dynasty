//! Campaign playthrough QA: a fresh captain plays chapters 1–5 as a player would, in accelerated
//! game time, and the report says per chapter whether it was finished, how long it took, how often
//! the captain died and where it got stuck (`sim::playthrough`).
//!
//!   cargo run -p sim --release --example campaign [-- --hull Talon --species Human --profession CombatPilot --hours 10]
//!   … --chapter 4 [--level 8]   one chapter on its own (chapters before it done, every module at that level)
//!   … --over                     fight turret ships from over their hull (3D), not head-on
//!   … --seed N                   another world start; single runs swing ±10 min, compare medians
//!   … --operations [--level L]   each major operation on its own (missions::operations), at L or at the
//!                                module cap its unlock gives; one line each
//!
//! Exit code 1 when the campaign was not finished.

use sim::playthrough::{Options, play};

/// An enum variant by name, as in the JSON specs (`Talon`, `Human`, `CombatPilot`).
fn parse<T: serde::de::DeserializeOwned>(name: &str) -> T {
    serde_json::from_str(&format!("\"{name}\"")).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let arg = |k: &str| args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).cloned();
    let mut o = Options::default();
    if let Some(h) = arg("--hull") {
        o.hull = parse(&h);
    }
    if let Some(s) = arg("--species") {
        o.species = parse(&s);
    }
    if let Some(p) = arg("--profession") {
        o.profession = parse(&p);
    }
    if let Some(c) = arg("--chapter").and_then(|v| v.parse::<u32>().ok()) {
        // Chapters are numbered from 1 here, as in game.
        o.chapter = c.saturating_sub(1);
        o.only = true;
    }
    if let Some(l) = arg("--level").and_then(|v| v.parse::<u8>().ok()) {
        o.level = l;
    }
    o.over = args.iter().any(|a| a == "--over");
    if let Some(h) = arg("--hours").and_then(|v| v.parse::<f32>().ok()) {
        o.max_secs = h * 3600.0;
    }
    if let Some(s) = arg("--seed").and_then(|v| v.parse::<u32>().ok()) {
        o.seed = s;
    }
    if args.iter().any(|a| a == "--operations") {
        let fixed = arg("--level").and_then(|v| v.parse::<u8>().ok());
        let mut failed = false;
        for op in shared::missions::operations() {
            let level = fixed.unwrap_or_else(|| shared::ships::tier_cap(op.unlock));
            let rep = play(&Options { operation: Some(op.id), level, max_secs: 2.0 * 3600.0, ..o.clone() });
            let c = rep.chapters.first();
            let done = c.is_some_and(|c| c.finished.is_some());
            failed |= !done;
            println!(
                "{} {:<26} Lv{level} · {} · deaths {} · low hull {:.0}%{}",
                if done { "DONE" } else { "FAIL" },
                op.title,
                c.and_then(|c| c.finished.map(|f| format!("{:.1} min", (f - c.started) / 60.0))).unwrap_or_else(|| "—".into()),
                c.map_or(0, |c| c.deaths),
                c.map_or(100.0, |c| c.low_hull * 100.0),
                rep.stuck.as_ref().map_or(String::new(), |w| format!(" — {w}")),
            );
            if args.iter().any(|a| a == "--log") {
                for l in c.map(|c| c.log.clone()).unwrap_or_default() {
                    println!("      {l}");
                }
            }
        }
        std::process::exit(if failed { 1 } else { 0 });
    }
    println!("campaign playthrough: {:?} {:?} {:?}, up to {:.1} h of game time", o.hull, o.species, o.profession, o.max_secs / 3600.0);
    let start = std::time::Instant::now();
    let rep = play(&o);
    print!("{}", rep.text());
    println!("({:.1} s wall clock)", start.elapsed().as_secs_f32());
    if rep.stuck.is_some() {
        std::process::exit(1);
    }
}

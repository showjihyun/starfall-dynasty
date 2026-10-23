//! Campaign playthrough QA: a fresh captain plays chapters 1–5 as a player would, in accelerated
//! game time, and the report says per chapter whether it was finished, how long it took, how often
//! the captain died and where it got stuck (`sim::playthrough`).
//!
//!   cargo run -p sim --release --example campaign [-- --hull Talon --species Human --profession CombatPilot --hours 10]
//!   … --chapter 4 [--level 8]   one chapter on its own (chapters before it done, every module at that level)
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
    if let Some(h) = arg("--hours").and_then(|v| v.parse::<f32>().ok()) {
        o.max_secs = h * 3600.0;
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

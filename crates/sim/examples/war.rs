//! Long war QA: alliances of AI captains contest the galaxy for hours or days of game time under
//! the real rules, and the report says how the map moved (`sim::war`).
//!
//!   cargo run -p sim --release --example war [-- --alliances 4 --captains 6 --hours 24 --level 5]
//!   … --war-hours 6    hours at war without gaining ground before a leader makes peace
//!   … --seed N         another world start

use sim::war::{Options, run};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let arg = |k: &str| args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).cloned();
    let mut o = Options::default();
    if let Some(v) = arg("--alliances").and_then(|v| v.parse().ok()) {
        o.alliances = v;
    }
    if let Some(v) = arg("--captains").and_then(|v| v.parse().ok()) {
        o.captains = v;
    }
    if let Some(v) = arg("--hours").and_then(|v| v.parse().ok()) {
        o.hours = v;
    }
    if let Some(v) = arg("--level").and_then(|v| v.parse().ok()) {
        o.level = v;
    }
    if let Some(v) = arg("--war-hours").and_then(|v| v.parse().ok()) {
        o.war_hours = v;
    }
    if let Some(v) = arg("--seed").and_then(|v| v.parse().ok()) {
        o.seed = v;
    }
    println!("war: {} alliances × {} captains, Lv{}, {:.0} h of game time", o.alliances, o.captains, o.level, o.hours);
    print!("{}", run(&o).text());
}

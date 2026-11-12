//! Open-galaxy QA: a captain takes jobs from station boards (and trades when nothing is worth
//! taking) for a few hours of game time; per kind of job the played minutes, credits and deaths, and
//! the CR per minute over the whole career (`sim::career`). Hold it against the mining loop
//! (`--example mining -- --all`) and the estimate (`--example board`).
//!
//!   cargo run -p sim --release --example career [-- --hull Talon --level 1 --hours 3 --kinds bounty,haul --trade --credits 20000 --log]
//!   … --threat   what each ship level (1–8) wins, by the fight's threat (to set trade.ron `threat_by_level`)
//!   … --all      each kind on its own at Lv1 and Lv5 (Talon; trade on a Mule), one line each

use shared::jobs::JobTag;
use shared::ships::HullClass;
use sim::career::{Options, run};

fn parse<T: serde::de::DeserializeOwned>(name: &str) -> T {
    serde_json::from_str(&format!("\"{name}\"")).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn kind(s: &str) -> JobTag {
    let mut c = s.chars();
    let cap: String = c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default();
    parse(&cap)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let arg = |k: &str| args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).cloned();
    let flag = |k: &str| args.iter().any(|a| a == k);
    let mut o = Options::default();
    if let Some(h) = arg("--hull") {
        o.hull = parse(&h);
    }
    if let Some(l) = arg("--level").and_then(|v| v.parse().ok()) {
        o.level = l;
    }
    if let Some(h) = arg("--hours").and_then(|v| v.parse::<f32>().ok()) {
        o.secs = h * 3600.0;
    }
    if let Some(k) = arg("--kinds") {
        o.kinds = if k == "none" { vec![] } else { k.split(',').map(kind).collect() };
    }
    o.trade = flag("--trade");
    o.credits = arg("--credits").and_then(|v| v.parse().ok());
    if flag("--threat") {
        // What each ship level wins: every combat job taken, briefing or no; wins by threat band.
        let bands = [0.0f32, 2.0, 3.0, 4.0, 5.5, 7.0, 9.0, 11.0, 14.0, 18.0, 24.0, 32.0, 99.0];
        println!("wins / fights by threat (sim::career, {:.0} h each, bounty and armada, no briefing, no learning)", o.secs / 3600.0);
        print!("{:<5}", "Lv");
        for w in bands.windows(2) {
            print!(" {:>9}", format!("{}-{}", w[0], w[1]));
        }
        println!();
        for level in 1u8..=8 {
            let r = run(&Options { level, kinds: vec![JobTag::Bounty, JobTag::Armada], heed: false, learn: false, ..o.clone() });
            print!("{:<5}", format!("Lv{level}"));
            for w in bands.windows(2) {
                let f: Vec<&(f32, bool)> = r.fights.iter().filter(|(t, _)| *t >= w[0] && *t < w[1]).collect();
                let won = f.iter().filter(|x| x.1).count();
                print!(" {:>9}", if f.is_empty() { "·".to_string() } else { format!("{won}/{}", f.len()) });
            }
            println!();
        }
        return;
    }
    if flag("--all") {
        println!("each line: {:.0} h of game time, one kind of work only (a fresh captain's credits; supply and trade with 20000 CR)", o.secs / 3600.0);
        for level in [1u8, 5] {
            for k in [JobTag::Bounty, JobTag::Haul, JobTag::Survey, JobTag::Armada, JobTag::Supply] {
                let credits = (k == JobTag::Supply).then_some(20_000);
                let r = run(&Options { level, kinds: vec![k], credits, ..o.clone() });
                let t = r.jobs.get(&k).cloned().unwrap_or_default();
                println!("Lv{level} {:<10} done {:>2} failed {:>2} · {:>5.1} min a job · {:>6.0} CR/min working · {:>6.0} CR/min overall (looking {:>3.0}%) · {} deaths", k.name(), t.done, t.failed, t.secs / 60.0 / t.taken.max(1) as f32, t.per_min(), r.per_min(), r.idle / r.secs * 100.0, r.deaths);
            }
            let r = run(&Options { level, kinds: vec![], trade: true, hull: HullClass::Mule, credits: Some(20_000), ..o.clone() });
            println!("Lv{level} 교역(Mule)  runs {:>2} · {:>5.1} min a run · {:>6.0} CR/min working · {:>6.0} CR/min overall · {} deaths", r.trade.taken, r.trade.secs / 60.0 / r.trade.taken.max(1) as f32, r.trade.per_min(), r.per_min(), r.deaths);
        }
        return;
    }
    let r = run(&o);
    print!("{}", r.text());
    if flag("--log") {
        for l in &r.log {
            println!("  {l}");
        }
    }
}

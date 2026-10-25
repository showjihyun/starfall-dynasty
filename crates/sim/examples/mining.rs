//! Mining loop QA: mining trips station → asteroid → station, timed per leg, with what they earn
//! (`sim::mining`).
//!
//!   cargo run -p sim --release --example mining [-- --hull Mule --level 1 --trips 3]
//!   … --system 3 --ore Xenite --pick nearest|rarest|steepest --refine --profession Miner
//!   … --all       every hull at Lv1 and Lv5, nearest asteroid at home, one line each
//!
//! Exit code 1 when a run could not finish its trips.

use shared::ships::HullClass;
use sim::mining::{Options, Pick, Report, run};

fn parse<T: serde::de::DeserializeOwned>(name: &str) -> T {
    serde_json::from_str(&format!("\"{name}\"")).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn line(o: &Options, r: &Report) -> String {
    let t = r.total();
    let secs = t.secs().max(1e-3);
    let (cr, refined) = r.per_min();
    format!(
        "{:<8} Lv{:<2} hold {:>4.0} full in {:>4.0} s · trip {:>4.0} s (mine {:>3.0}% fly {:>3.0}% fight {:>3.0}%) · {:>5.0} CR/min (ore {:>4.0}){} · {} fled · {} deaths",
        format!("{:?}", o.hull),
        o.level,
        r.cargo,
        r.cargo / r.mine_rate.max(1e-3),
        secs / r.trips.len().max(1) as f32,
        t.mine / secs * 100.0,
        (t.out + t.back) / secs * 100.0,
        (t.combat + t.dead) / secs * 100.0,
        cr,
        t.ore_credits as f32 / (secs / 60.0),
        if refined > 0.0 { format!(" {refined:.1} refined/min") } else { String::new() },
        t.fled,
        t.deaths,
    )
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let arg = |k: &str| args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).cloned();
    let mut o = Options::default();
    if let Some(h) = arg("--hull") {
        o.hull = parse(&h);
    }
    if let Some(p) = arg("--profession") {
        o.profession = parse(&p);
    }
    if let Some(l) = arg("--level").and_then(|v| v.parse().ok()) {
        o.level = l;
    }
    if let Some(s) = arg("--system").and_then(|v| v.parse().ok()) {
        o.system = Some(s);
    }
    if let Some(ore) = arg("--ore") {
        o.ore = Some(parse(&ore));
    }
    if let Some(n) = arg("--trips").and_then(|v| v.parse().ok()) {
        o.trips = n;
    }
    o.pick = match arg("--pick").as_deref() {
        Some("rarest") => Pick::Rarest,
        Some("steepest") => Pick::Steepest,
        _ => Pick::Nearest,
    };
    o.refine = args.iter().any(|a| a == "--refine");
    let mut failed = false;
    if args.iter().any(|a| a == "--all") {
        for level in [1, 5] {
            for hull in HullClass::ALL {
                let o = Options { hull, level, ..o.clone() };
                let r = run(&o);
                println!("{}", line(&o, &r));
                failed |= r.stuck.is_some();
            }
        }
    } else {
        let r = run(&o);
        println!("mining: {:?} Lv{} {:?}", o.hull, o.level, o.profession);
        print!("{}", r.text());
        failed = r.stuck.is_some();
    }
    if failed {
        std::process::exit(1);
    }
}

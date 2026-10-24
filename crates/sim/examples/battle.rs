//! How far a big fight spreads (main goal "epic spread-out battles"):
//! `cargo run -p sim --release --example battle [captains] [--secs S] [--every S] [--doctrine old|wide]
//!  [--spacing U] [--keep SHARE] [--lanes 0|1] [--separation U]`
//! Runs the `client --bench` war (`sim::battle`, default 1020 captains) headless and prints, per
//! front, how wide the fighting is: ships, RMS spread across and up, 90% footprint, median distance
//! to the nearest ship, share of ships within 60 u of another, median distance to the target.

use shared::protocol::TICK_HZ;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let arg = |k: &str| args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).and_then(|v| v.parse::<f32>().ok());
    let n = args.first().and_then(|v| v.parse().ok()).unwrap_or(1020);
    let secs = arg("--secs").unwrap_or(120.0);
    let every = arg("--every").unwrap_or(20.0);
    let mut d = match args.iter().position(|a| a == "--doctrine").and_then(|i| args.get(i + 1)).map(String::as_str) {
        Some("wide") => sim::battle::Doctrine::WIDE,
        _ => sim::battle::Doctrine::OLD,
    };
    d.spacing = arg("--spacing").unwrap_or(d.spacing);
    d.keep = arg("--keep").unwrap_or(d.keep);
    d.lanes = arg("--lanes").map_or(d.lanes, |v| v > 0.0);
    d.separation = arg("--separation").unwrap_or(d.separation);
    println!("doctrine {d:?}");
    let (mut g, mut caps) = sim::battle::setup(n, &d);
    let fronts = &g.systems[&sim::battle::SYSTEM].fronts;
    let gaps: Vec<String> = fronts.iter().enumerate().flat_map(|(i, a)| fronts[i + 1..].iter().map(move |b| format!("{:.0}", a.pos.distance(b.pos)))).collect();
    println!("battle: {n} captains, system {}, {} fronts (front-to-front {} u)", sim::battle::SYSTEM, fronts.len(), gaps.join(" / "));
    println!("{:>5}  {:<22} {:>5} {:>7} {:>6} {:>8} {:>8} {:>7} {:>7}", "t", "front", "ships", "across", "up", "reach90", "nearest", "<60u", "engage");
    let dt = 1.0 / TICK_HZ;
    let total = (secs * TICK_HZ) as u64;
    let every = ((every * TICK_HZ) as u64).max(1);
    let count = |g: &sim::game::Game| sim::battle::spread(g).iter().map(|s| s.ships).sum::<usize>();
    let captains = |g: &sim::game::Game| g.systems[&sim::battle::SYSTEM].ents.values().filter(|e| !e.dead && e.kind == shared::protocol::EntityKind::Player).count();
    let c0 = captains(&g);
    let mut c_half = None;
    let start = count(&g);
    let (mut half, mut ninety) = (None, None);
    for tick in 0..=total {
        if tick % (TICK_HZ as u64) == 0 {
            let left = count(&g);
            let t = tick as f32 / TICK_HZ;
            if half.is_none() && left * 2 <= start {
                half = Some(t);
            }
            if c_half.is_none() && captains(&g) * 2 <= c0 {
                c_half = Some(t);
            }
            if ninety.is_none() && left * 10 <= start {
                ninety = Some(t);
            }
        }
        if tick % every == 0 {
            for s in sim::battle::spread(&g) {
                println!("{:>4.0}s  {:<22} {:>5} {:>7.0} {:>6.0} {:>8.0} {:>8.0} {:>6.0}% {:>7.0}", tick as f32 / TICK_HZ, s.front, s.ships, s.across, s.up, s.reach90, s.nearest, s.crowded * 100.0, s.engage);
            }
        }
        sim::battle::steer(&mut g, &mut caps, tick, &d);
        g.tick(dt, sim::battle::T0 + tick / TICK_HZ as u64);
        g.out.clear();
    }
    let at = |t: Option<f32>| t.map_or(format!("not in {secs:.0}s"), |t| format!("{t:.0}s"));
    println!("half the ships sunk at {}, 90% at {}; half the {c0} captains at {}", at(half), at(ninety), at(c_half));
}

//! How far a big fight spreads (main goal "epic spread-out battles"):
//! `cargo run -p sim --release --example battle [captains] [--secs S] [--every S]`
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
    let (mut g, mut caps) = sim::battle::setup(n);
    let fronts = &g.systems[&sim::battle::SYSTEM].fronts;
    let gaps: Vec<String> = fronts.iter().enumerate().flat_map(|(i, a)| fronts[i + 1..].iter().map(move |b| format!("{:.0}", a.pos.distance(b.pos)))).collect();
    println!("battle: {n} captains, system {}, {} fronts (front-to-front {} u)", sim::battle::SYSTEM, fronts.len(), gaps.join(" / "));
    println!("{:>5}  {:<22} {:>5} {:>7} {:>6} {:>8} {:>8} {:>7} {:>7}", "t", "front", "ships", "across", "up", "reach90", "nearest", "<60u", "engage");
    let dt = 1.0 / TICK_HZ;
    let total = (secs * TICK_HZ) as u64;
    let every = ((every * TICK_HZ) as u64).max(1);
    for tick in 0..=total {
        if tick % every == 0 {
            for s in sim::battle::spread(&g) {
                println!("{:>4.0}s  {:<22} {:>5} {:>7.0} {:>6.0} {:>8.0} {:>8.0} {:>6.0}% {:>7.0}", tick as f32 / TICK_HZ, s.front, s.ships, s.across, s.up, s.reach90, s.nearest, s.crowded * 100.0, s.engage);
            }
        }
        sim::battle::steer(&mut g, &mut caps, tick);
        g.tick(dt, sim::battle::T0 + tick / TICK_HZ as u64);
        g.out.clear();
    }
}

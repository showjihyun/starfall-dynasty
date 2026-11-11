//! What the open galaxy pays, next to mining (`tuning/trade.ron`).
//!
//!     cargo run -p sim --release --example board [-- --range LY --hop SECS]
//!
//! Every station's board over a day of shifts: per kind of job, how many come up, the average reward
//! and an estimate of the minutes it takes (warp hops at `--range` ly a jump, `--hop` seconds per
//! system for the jump, the arrival and the flight in, plus a fight for bounties and armadas), so the
//! CR per minute can be held against the mining loop (`cargo run -p sim --release --example mining
//! -- --all`: a Lv1 hull 220–460 CR/min, Lv5 700–1100). Then the trade routes: per station the best
//! good to carry within the range and its margin per unit, and what a full Lv1 Mule hold (90) earns.
//! Estimates, not a played career: the travel model is coarse and specimens for prospect orders are
//! not counted as found.

use shared::galaxy::{self, RiskTier, SystemId};
use shared::jobs::{self, JobKind, JobTag};
use shared::market;
use std::collections::BTreeMap;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let arg = |k: &str| args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).and_then(|v| v.parse::<f32>().ok());
    let range = arg("--range").unwrap_or(1.7);
    let hop = arg("--hop").unwrap_or(75.0);
    let t = shared::tuning::get();
    let shifts = 86_400 / t.trade.jobs.period_secs.max(1);
    let dist = |a: SystemId, b: SystemId| glam_dist(a, b);
    let trip = |ly: f32| (ly / range).ceil().max(1.0) * hop;

    // tag → (count, credits, seconds)
    let mut by_kind: BTreeMap<JobTag, (u32, f64, f64)> = BTreeMap::new();
    let mut by_tier: BTreeMap<(RiskTier, JobTag), u32> = BTreeMap::new();
    for s in galaxy::systems() {
        for p in 0..shifts {
            for j in jobs::board(&t, s.id, p) {
                let secs = match &j.kind {
                    // There and back to the board's station is not needed: the reward comes on the kill.
                    JobKind::Bounty { system, escorts, .. } => trip(dist(s.id, *system)) * (system != &s.id) as u8 as f32 + 45.0 + 15.0 * *escorts as f32,
                    JobKind::Armada { system, wave, .. } => trip(dist(s.id, *system)) * (system != &s.id) as u8 as f32 + 20.0 * wave.iter().map(|w| w.1).sum::<u32>() as f32,
                    JobKind::Haul { to, .. } => trip(dist(s.id, *to)) + 30.0,
                    // Visiting each in turn, nearest first.
                    JobKind::Survey { systems } => {
                        let mut at = s.id;
                        let mut left = systems.clone();
                        let mut total = 0.0;
                        while !left.is_empty() {
                            left.sort_by(|a, b| dist(at, *a).total_cmp(&dist(at, *b)));
                            let next = left.remove(0);
                            total += trip(dist(at, next));
                            at = next;
                        }
                        total
                    }
                    // Fetch the goods where they are cheapest within the range and come back.
                    JobKind::Supply { .. } => 2.0 * hop + 60.0,
                    JobKind::Prospect { .. } => 0.0,
                };
                let e = by_kind.entry(j.kind.tag()).or_default();
                e.0 += 1;
                e.1 += j.reward.credits as f64;
                e.2 += secs as f64;
                *by_tier.entry((s.tier, j.kind.tag())).or_default() += 1;
            }
        }
    }
    println!("jobs over a day ({shifts} shifts × {} stations), warp {range} ly a jump, {hop} s a system", galaxy::systems().len());
    println!("{:<10} {:>6} {:>10} {:>9} {:>10}", "kind", "count", "avg CR", "est min", "CR/min");
    for (k, (n, cr, secs)) in &by_kind {
        let n = *n as f64;
        let mins = secs / n / 60.0;
        let per = if mins > 0.0 { format!("{:.0}", cr / n / mins) } else { "—".into() };
        println!("{:<10} {:>6} {:>10.0} {:>9.1} {:>10}", k.name(), n, cr / n, mins, per);
    }
    println!();
    println!("share of each kind by the posting station's tier:");
    for tier in [RiskTier::Core, RiskTier::Frontier, RiskTier::Wild, RiskTier::WarZone, RiskTier::Deep] {
        let total: u32 = by_tier.iter().filter(|((t2, _), _)| *t2 == tier).map(|(_, n)| *n).sum();
        if total == 0 {
            continue;
        }
        let parts: Vec<String> = jobs::JobTag::ALL.iter().map(|k| format!("{} {:.0}%", k.name(), 100.0 * by_tier.get(&(tier, *k)).copied().unwrap_or(0) as f32 / total as f32)).collect();
        println!("  {:<9} {}", tier.label(), parts.join(" · "));
    }

    println!();
    println!("trade routes within {range} ly (one jump): best good per station, margin a unit, a 90-unit hold");
    let mut margins = vec![];
    for s in galaxy::systems() {
        if let Some((g, to, m)) = market::best_route(&t.trade.market, s.id, range) {
            margins.push(m);
            let hold = market::buy_total(&t.trade.market, s.id, g, 0.0, 90).zip(market::sell_total(&t.trade.market, to, g, 0.0, 90)).map(|((c, _), (p, _))| p - c).unwrap_or(0.0);
            println!("  {:<22} {:<8} → {:<22} +{:>5.1}/unit · hold {:>6.0} CR · {:>5.0} CR/min", s.name, g.name(), galaxy::system(to).name, m, hold, hold / ((2.0 * hop + 60.0) / 60.0));
        }
    }
    margins.sort_by(f32::total_cmp);
    let n = margins.len();
    if n > 0 {
        println!("{} of {} stations have a route; margin median {:.1}, best {:.1} CR a unit", n, galaxy::systems().len(), margins[n / 2], margins[n - 1]);
    }
}

fn glam_dist(a: SystemId, b: SystemId) -> f32 {
    shared::glam::Vec3::from(galaxy::system(a).map_pos).distance(shared::glam::Vec3::from(galaxy::system(b).map_pos))
}

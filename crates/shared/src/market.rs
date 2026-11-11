//! The trade goods market (`tuning/trade.ron` `market`): what every station makes and needs.
//!
//! A station sells what its system produces, cheaply, and buys every legal good, dearly when it
//! needs it. Who makes and needs what follows the star and the risk tier (a blue giant powers fuel
//! cells, a war zone needs arms), so the map has trade routes without anyone drawing them. Prices
//! move as captains trade — buying out a station raises its price, flooding one lowers it — and
//! drift back with a half-life. Contraband sells well at the edge of the law and is confiscated in
//! the core. Everything here is a pure function of the tuning, the system and the stock offset, so
//! the server, the client and the tests compute the same numbers.

use crate::galaxy::{self, RiskTier, SystemId};
use crate::rng::Rng;
use crate::tuning::MarketTuning;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Good {
    Rations,
    Medicine,
    Machinery,
    FuelCells,
    Electronics,
    Luxuries,
    Arms,
    Stims,
}

impl Good {
    pub const ALL: [Good; 8] = [Good::Rations, Good::Medicine, Good::Machinery, Good::FuelCells, Good::Electronics, Good::Luxuries, Good::Arms, Good::Stims];

    pub fn name(self) -> &'static str {
        match self {
            Good::Rations => "식량",
            Good::Medicine => "의약품",
            Good::Machinery => "기계 부품",
            Good::FuelCells => "연료 전지",
            Good::Electronics => "전자 장비",
            Good::Luxuries => "사치품",
            Good::Arms => "무기 부품",
            Good::Stims => "전투 자극제",
        }
    }

    /// Key in the ship's hold (`Profile::cargo`), next to the ores.
    pub fn key(self) -> &'static str {
        match self {
            Good::Rations => "Rations",
            Good::Medicine => "Medicine",
            Good::Machinery => "Machinery",
            Good::FuelCells => "FuelCells",
            Good::Electronics => "Electronics",
            Good::Luxuries => "Luxuries",
            Good::Arms => "Arms",
            Good::Stims => "Stims",
        }
    }

    /// One line of what it is (the exchange's detail card).
    pub fn blurb(self) -> &'static str {
        match self {
            Good::Rations => "농업 성계의 동결 건조 식량. 변경과 전장은 언제나 굶주린다.",
            Good::Medicine => "재생 젤과 나노 봉합제. 싸움이 있는 곳마다 값이 뛴다.",
            Good::Machinery => "채굴 드릴과 반응로 부품. 개척지가 자라는 만큼 필요하다.",
            Good::FuelCells => "청색 거성과 중성자성의 열로 채운 고밀도 전지. 중심 성계가 소비한다.",
            Good::Electronics => "백색 왜성 공방의 정밀 회로. 무법 지대에서는 귀하다.",
            Good::Luxuries => "Veyr 공명 결정과 진귀한 향료. 부유한 중심 성계만 산다.",
            Good::Arms => "Kharz 공방의 포신과 탄두. 전쟁이 끝나지 않는 한 팔린다.",
            Good::Stims => "전투 자극제. 중심 성계에서는 불법 — 세관에 걸리면 압수와 벌금.",
        }
    }

    pub fn of_key(key: &str) -> Option<Good> {
        Good::ALL.into_iter().find(|g| g.key() == key)
    }
}

/// What a station makes and needs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Economy {
    pub produce: Vec<Good>,
    pub demand: Vec<Good>,
}

impl Economy {
    pub fn role(&self, g: Good) -> Role {
        if self.produce.contains(&g) {
            Role::Produce
        } else if self.demand.contains(&g) {
            Role::Demand
        } else {
            Role::Neutral
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Role {
    Produce,
    Neutral,
    Demand,
}

pub fn contraband(t: &MarketTuning, g: Good) -> bool {
    t.contraband.contains(&g)
}

/// Contraband is seized at this tier's stations (and they never trade in it).
pub fn seized(t: &MarketTuning, tier: RiskTier) -> bool {
    t.seize_in.contains(&tier)
}

/// The system's economy: `produce_count` goods from its star's industries, `demand_count` from its
/// tier's needs (never one it makes), and contraband from the black market where the law is thin.
/// Fixed per system (seeded by its id).
pub fn economy(t: &MarketTuning, sys: SystemId) -> Economy {
    let def = galaxy::system(sys);
    let mut r = Rng::from_parts(&[0x7_4ADE, sys as u64]);
    let mut pick = |pool: &[Good], n: usize, not: &[Good]| -> Vec<Good> {
        let mut left: Vec<Good> = pool.iter().copied().filter(|g| !not.contains(g)).collect();
        let mut out = vec![];
        while out.len() < n && !left.is_empty() {
            out.push(left.remove(r.below(left.len() as u32) as usize));
        }
        out
    };
    let mut produce = pick(t.industries.get(&def.star).map_or(&[][..], |v| v), t.produce_count, &[]);
    if t.black_market.contains(&def.tier) {
        for g in &t.contraband {
            if !produce.contains(g) {
                produce.push(*g);
            }
        }
    }
    let demand = pick(t.needs.get(&def.tier).map_or(&[][..], |v| v), t.demand_count, &produce);
    let mut e = Economy { produce, demand };
    e.produce.sort();
    e.demand.sort();
    e
}

/// The station's middle price for `g` with the stock moved by `offset` units (positive: captains have
/// sold it more than bought it, so it is cheaper).
pub fn mid_price(t: &MarketTuning, sys: SystemId, g: Good, offset: f32) -> f32 {
    let e = economy(t, sys);
    let role = match e.role(g) {
        Role::Produce => t.produce_price,
        Role::Neutral => 1.0,
        Role::Demand => t.demand_price,
    };
    let tier = t.tier.get(&galaxy::system(sys).tier).copied().unwrap_or(1.0);
    t.base.get(&g).copied().unwrap_or(0.0) * role * tier * (-offset / t.depth).exp()
}

/// Price one more unit costs to buy here, or None if the station does not sell it.
pub fn buy_price(t: &MarketTuning, sys: SystemId, g: Good, offset: f32) -> Option<f32> {
    let e = economy(t, sys);
    (e.role(g) == Role::Produce && !(contraband(t, g) && seized(t, galaxy::system(sys).tier))).then(|| mid_price(t, sys, g, offset) * (1.0 + t.spread))
}

/// Price one more unit fetches when sold here, or None if the station will not take it.
pub fn sell_price(t: &MarketTuning, sys: SystemId, g: Good, offset: f32) -> Option<f32> {
    (!(contraband(t, g) && seized(t, galaxy::system(sys).tier))).then(|| mid_price(t, sys, g, offset) * (1.0 - t.spread))
}

/// The stock offset `secs` after it was `offset`: it drifts back toward the station's own level.
pub fn relax(t: &MarketTuning, offset: f32, secs: f64) -> f32 {
    if secs <= 0.0 || t.half_life <= 0.0 {
        return offset;
    }
    offset * 0.5f32.powf((secs / t.half_life as f64) as f32)
}

/// Total cost of buying `units` one at a time (each moves the stock), and the offset after.
pub fn buy_total(t: &MarketTuning, sys: SystemId, g: Good, offset: f32, units: u32) -> Option<(f32, f32)> {
    let mut total = 0.0;
    let mut o = offset;
    for _ in 0..units {
        total += buy_price(t, sys, g, o)?;
        o -= 1.0;
    }
    Some((total, o))
}

/// Total paid for selling `units` one at a time, and the offset after.
pub fn sell_total(t: &MarketTuning, sys: SystemId, g: Good, offset: f32, units: u32) -> Option<(f32, f32)> {
    let mut total = 0.0;
    let mut o = offset;
    for _ in 0..units {
        total += sell_price(t, sys, g, o)?;
        o += 1.0;
    }
    Some((total, o))
}

/// One line of a station's board: what it sells for and buys at, now.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Quote {
    pub good: Good,
    pub role: Role,
    pub buy: Option<f32>,
    pub sell: Option<f32>,
}

pub fn quotes(t: &MarketTuning, sys: SystemId, offset: impl Fn(Good) -> f32) -> Vec<Quote> {
    let e = economy(t, sys);
    Good::ALL.iter().map(|&g| Quote { good: g, role: e.role(g), buy: buy_price(t, sys, g, offset(g)), sell: sell_price(t, sys, g, offset(g)) }).collect()
}

/// The best place within `range` ly to sell what `from` sells, by profit per unit at the stations'
/// own levels: a captain's first trade route ("buy X here, it fetches Y at Z").
pub fn best_route(t: &MarketTuning, from: SystemId, range: f32) -> Option<(Good, SystemId, f32)> {
    let here = glam::Vec3::from(galaxy::system(from).map_pos);
    let mut best: Option<(Good, SystemId, f32)> = None;
    for g in economy(t, from).produce {
        let Some(cost) = buy_price(t, from, g, 0.0) else { continue };
        for s in galaxy::systems() {
            if s.id == from || glam::Vec3::from(s.map_pos).distance(here) > range {
                continue;
            }
            if let Some(p) = sell_price(t, s.id, g, 0.0) {
                let margin = p - cost;
                if best.is_none_or(|b| margin > b.2) {
                    best = Some((g, s.id, margin));
                }
            }
        }
    }
    best.filter(|b| b.2 > 0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t() -> MarketTuning {
        crate::tuning::get().trade.market.clone()
    }

    #[test]
    fn every_station_makes_something_and_needs_something_else() {
        let t = t();
        for s in galaxy::systems() {
            let e = economy(&t, s.id);
            assert!(!e.produce.is_empty() && !e.demand.is_empty(), "{}: {e:?}", s.name);
            assert!(e.demand.iter().all(|g| !e.produce.contains(g)), "{}: makes what it needs", s.name);
            assert_eq!(e, economy(&t, s.id), "fixed per system");
        }
        // The map has variety: not every station makes the same.
        let mut kinds: Vec<Vec<Good>> = galaxy::systems().iter().map(|s| economy(&t, s.id).produce).collect();
        kinds.sort();
        kinds.dedup();
        assert!(kinds.len() >= 6, "{kinds:?}");
    }

    #[test]
    fn buying_raises_the_price_selling_lowers_it_and_both_drift_back() {
        let t = t();
        let sys = galaxy::systems().iter().find(|s| !economy(&t, s.id).produce.is_empty() && !seized(&t, s.tier)).unwrap().id;
        let g = economy(&t, sys).produce[0];
        let p0 = buy_price(&t, sys, g, 0.0).unwrap();
        let (cost, after) = buy_total(&t, sys, g, 0.0, 40).unwrap();
        assert!(buy_price(&t, sys, g, after).unwrap() > p0, "bought out");
        assert!(cost > p0 * 40.0 && cost < p0 * 40.0 * 1.2, "{cost} for 40 at {p0}");
        let (_, flooded) = sell_total(&t, sys, g, 0.0, 40).unwrap();
        assert!(sell_price(&t, sys, g, flooded).unwrap() < sell_price(&t, sys, g, 0.0).unwrap());
        assert!((relax(&t, after, t.half_life as f64) - after / 2.0).abs() < 1e-3);
        // Buying and selling back at once always loses the spread.
        let (back, _) = sell_total(&t, sys, g, after, 40).unwrap();
        assert!(back < cost);
    }

    #[test]
    fn contraband_is_seized_in_the_core_and_sold_on_the_black_market() {
        let t = t();
        for s in galaxy::systems() {
            for g in Good::ALL.into_iter().filter(|g| contraband(&t, *g)) {
                if seized(&t, s.tier) {
                    assert_eq!(buy_price(&t, s.id, g, 0.0), None, "{}", s.name);
                    assert_eq!(sell_price(&t, s.id, g, 0.0), None, "{}", s.name);
                }
                if t.black_market.contains(&s.tier) {
                    assert!(buy_price(&t, s.id, g, 0.0).is_some(), "{} sells {g:?}", s.name);
                }
            }
        }
    }

    #[test]
    fn there_is_a_profitable_route_from_most_stations() {
        let t = t();
        let with = galaxy::systems().iter().filter(|s| best_route(&t, s.id, 2.5).is_some()).count();
        assert!(with * 10 >= galaxy::systems().len() * 7, "{with} of {}", galaxy::systems().len());
        // From Sol Haven, a route exists and pays a real margin.
        let (g, to, margin) = best_route(&t, 0, 2.5).expect("a route from Sol Haven");
        assert!(margin > 5.0, "{g:?} to {to}: {margin}");
    }
}

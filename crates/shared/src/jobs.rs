//! Station job boards (`tuning/trade.ron` `jobs`): every station posts its own work, and the work
//! changes with the shift.
//!
//! A board is a pure function of the station and the shift (`period`), so every captain docked at
//! the same station in the same shift sees the same jobs, and the client can draw what the server
//! will accept. A captain carries a few jobs at once, from any stations, and works them in any
//! order: hunt a named pirate two systems out, haul sealed cargo across the map, chart systems no
//! one has visited, bring back a rare specimen, break a pirate armada, or stock a station with goods
//! bought somewhere cheaper. What a station asks for follows what it is: the core wants goods and
//! couriers, the frontier wants guns, war zones want fleets.

use crate::galaxy::{self, RiskTier, SystemId};
use crate::market::{self, Good};
use crate::minerals::Rarity;
use crate::missions::NpcArchetype;
use crate::rng::Rng;
use crate::ships::Materials;
use crate::tuning::Tuning;
use serde::{Deserialize, Serialize};

pub type JobId = u64;

/// Hold key of a haul's sealed cargo (`Profile::cargo`).
pub const SEALED: &str = "Sealed";
/// Hold key of a smuggling run's unmarked crates: core customs seize them.
pub const CONTRABAND: &str = "Contraband";
/// Hold key of a salvaged flight recorder, until it is handed in.
pub const BLACKBOX: &str = "BlackBox";

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum JobTag {
    Bounty,
    Haul,
    Survey,
    Prospect,
    Armada,
    Supply,
    Smuggle,
    Patrol,
    Salvage,
}

impl JobTag {
    pub const ALL: [JobTag; 9] = [JobTag::Bounty, JobTag::Haul, JobTag::Survey, JobTag::Prospect, JobTag::Armada, JobTag::Supply, JobTag::Smuggle, JobTag::Patrol, JobTag::Salvage];

    pub fn name(self) -> &'static str {
        match self {
            JobTag::Bounty => "현상금",
            JobTag::Haul => "운송",
            JobTag::Survey => "탐사",
            JobTag::Prospect => "광물 주문",
            JobTag::Armada => "함대 방어전",
            JobTag::Supply => "보급",
            JobTag::Smuggle => "밀수 운송",
            JobTag::Patrol => "순찰",
            JobTag::Salvage => "잔해 회수",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum JobKind {
    /// Destroy a named pirate (and its escort) waiting in `system`.
    Bounty { system: SystemId, target: String, archetype: NpcArchetype, level: u8, escorts: u32 },
    /// Carry sealed cargo from the posting station to `to`'s station.
    Haul { to: SystemId, units: u32 },
    /// Visit every one of these systems (they need not be in order).
    Survey { systems: Vec<SystemId> },
    /// Bring `count` mineral specimens of at least `rarity` back to the posting station.
    Prospect { rarity: Rarity, count: u32 },
    /// Break the pirate fleet that warps in at `system`.
    Armada { system: SystemId, level: u8, wave: Vec<(NpcArchetype, u32)> },
    /// Bring `units` of `good` to the posting station.
    Supply { good: Good, units: u32 },
    /// Carry unmarked crates to `to`'s black market; docking at a core station on the way loses them to customs.
    Smuggle { to: SystemId, units: u32 },
    /// Sweep these systems: in each, a pirate pack (`trade.ron` `patrol_wave`) to break. `level`: the highest.
    Patrol { systems: Vec<SystemId>, level: u8 },
    /// Scan the wreck `site` (site `poi` of `system`) for its flight recorder and bring it back to the posting station.
    Salvage { system: SystemId, poi: u32, site: String },
}

impl JobKind {
    pub fn tag(&self) -> JobTag {
        match self {
            JobKind::Bounty { .. } => JobTag::Bounty,
            JobKind::Haul { .. } => JobTag::Haul,
            JobKind::Survey { .. } => JobTag::Survey,
            JobKind::Prospect { .. } => JobTag::Prospect,
            JobKind::Armada { .. } => JobTag::Armada,
            JobKind::Supply { .. } => JobTag::Supply,
            JobKind::Smuggle { .. } => JobTag::Smuggle,
            JobKind::Patrol { .. } => JobTag::Patrol,
            JobKind::Salvage { .. } => JobTag::Salvage,
        }
    }

    pub fn describe(&self, station: SystemId) -> String {
        let sys = |s: &SystemId| galaxy::system(*s).name;
        match self {
            JobKind::Bounty { system, target, archetype, escorts, .. } => {
                let with = if *escorts > 0 { format!(" (호위 {escorts}척)") } else { String::new() };
                format!("{}에 숨은 「{target}」({}) 격침{with}", sys(system), archetype.name())
            }
            JobKind::Haul { to, units } => format!("봉인 화물 {units}단위를 {} 정거장으로 운송", sys(to)),
            JobKind::Survey { systems } => format!("{} 측량 방문", systems.iter().map(sys).collect::<Vec<_>>().join(" · ")),
            JobKind::Prospect { rarity, count } => format!("{} 이상 광물 표본 {count}점을 {} 정거장에 납품", rarity.label(), sys(&station)),
            JobKind::Armada { system, wave, .. } => {
                let n: u32 = wave.iter().map(|w| w.1).sum();
                format!("{}에 집결하는 해적 함대 {n}척 격파", sys(system))
            }
            JobKind::Supply { good, units } => format!("{} {units}단위를 {} 정거장에 보급", good.name(), sys(&station)),
            JobKind::Smuggle { to, units } => format!("밀수품 {units}상자를 {} 암시장으로 — 중심 성계 세관을 피할 것", sys(to)),
            JobKind::Patrol { systems, .. } => format!("{} 순찰 — 성계마다 해적 무리 소탕", systems.iter().map(sys).collect::<Vec<_>>().join(" · ")),
            JobKind::Salvage { system, site, .. } => format!("{}의 「{site}」에서 블랙박스를 회수해 {} 정거장에 반납", sys(system), sys(&station)),
        }
    }

    /// Systems a captain has to go to (for the star map and the tracker); the turn-in station last.
    pub fn destinations(&self, station: SystemId) -> Vec<SystemId> {
        match self {
            JobKind::Bounty { system, .. } | JobKind::Armada { system, .. } => vec![*system],
            JobKind::Haul { to, .. } => vec![*to],
            JobKind::Survey { systems } => systems.clone(),
            JobKind::Prospect { .. } | JobKind::Supply { .. } => vec![station],
            JobKind::Smuggle { to, .. } => vec![*to],
            JobKind::Patrol { systems, .. } => systems.clone(),
            JobKind::Salvage { system, .. } => {
                if *system == station { vec![station] } else { vec![*system, station] }
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Job {
    pub id: JobId,
    pub station: SystemId,
    pub kind: JobKind,
    pub reward: Materials,
    pub reputation: i32,
    /// Seconds from accepting to deliver.
    pub limit_secs: u64,
}

impl Job {
    pub fn describe(&self) -> String {
        self.kind.describe(self.station)
    }
}

/// A job a captain has taken.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ActiveJob {
    pub job: Job,
    pub accepted: u64,
    /// Survey systems already visited.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub visited: Vec<SystemId>,
}

impl ActiveJob {
    pub fn deadline(&self) -> u64 {
        self.accepted + self.job.limit_secs
    }

    pub fn expired(&self, unix: u64) -> bool {
        unix > self.deadline()
    }

    /// Where the captain still has to go.
    pub fn remaining(&self) -> Vec<SystemId> {
        let mut d = self.job.kind.destinations(self.job.station);
        d.retain(|s| !self.visited.contains(s));
        d
    }

    /// One line for the tracker: what is left and how long.
    pub fn status(&self, unix: u64) -> String {
        let left = self.deadline().saturating_sub(unix);
        let time = if left >= 3600 { format!("{}시간 {}분", left / 3600, left % 3600 / 60) } else { format!("{}분", left.div_ceil(60)) };
        let progress = match &self.job.kind {
            JobKind::Survey { systems } => format!(" {}/{}", self.visited.len(), systems.len()),
            _ => String::new(),
        };
        format!("[{}{progress}] {} · {time}", self.job.kind.tag().name(), self.job.describe())
    }
}

/// The shift a moment falls in: boards change every `period_secs`.
pub fn period(t: &Tuning, unix: u64) -> u64 {
    unix / t.trade.jobs.period_secs.max(1)
}

fn distance(a: SystemId, b: SystemId) -> f32 {
    glam::Vec3::from(galaxy::system(a).map_pos).distance(glam::Vec3::from(galaxy::system(b).map_pos))
}

fn pick_weighted<'a, T>(items: &'a [T], weight: impl Fn(&T) -> u32, r: &mut Rng) -> Option<&'a T> {
    let total: u32 = items.iter().map(&weight).sum();
    if total == 0 {
        return None;
    }
    let mut x = r.below(total);
    items.iter().find(|i| {
        let w = weight(i);
        if x < w {
            true
        } else {
            x -= w;
            false
        }
    })
}

/// How hard a combat job's fight is (`tuning/trade.ron` `threat`): None for other jobs.
pub fn threat(t: &Tuning, kind: &JobKind) -> Option<f32> {
    let j = &t.trade.jobs;
    let one = |a: NpcArchetype, level: u8| j.threat.get(&a).copied().unwrap_or(1.0) * (1.0 + j.threat_level_step * level as f32);
    match kind {
        JobKind::Bounty { archetype, level, escorts, .. } => Some(one(*archetype, *level) + *escorts as f32 * one(NpcArchetype::Raider, *level)),
        JobKind::Armada { level, wave, .. } => Some(wave.iter().map(|(a, n)| *n as f32 * one(*a, *level)).sum()),
        // One pack at a time, the hardest system's.
        JobKind::Patrol { level, .. } => Some(j.patrol_wave.iter().map(|(a, n)| *n as f32 * one(*a, *level)).sum()),
        _ => None,
    }
}

/// The ship level (modules' average) a combat job is meant for: the first whose measured reach
/// (`threat_by_level`) covers its threat; past the table, one over its end.
pub fn recommended(t: &Tuning, kind: &JobKind) -> Option<u8> {
    let th = threat(t, kind)?;
    let table = &t.trade.jobs.threat_by_level;
    Some(table.iter().position(|cap| th <= *cap).map_or(table.len() + 1, |i| i + 1) as u8)
}

fn risk(t: &Tuning, s: SystemId) -> f32 {
    t.economy.risk_reward.get(&galaxy::system(s).tier).copied().unwrap_or(1.0)
}

/// Systems a job may send a captain to: every one but The Deep (beyond the gate, the campaign's end).
fn reachable() -> impl Iterator<Item = &'static galaxy::SystemDef> {
    galaxy::systems().iter().filter(|s| s.tier != RiskTier::Deep)
}

/// The nearest system to `from` (itself included) that pirates haunt, within `range` ly.
fn nearest_lawless(from: SystemId, range: f32, r: &mut Rng) -> Option<SystemId> {
    let mut near: Vec<&galaxy::SystemDef> = reachable().filter(|s| s.tier != RiskTier::Core && distance(from, s.id) <= range).collect();
    near.sort_by(|a, b| distance(from, a.id).total_cmp(&distance(from, b.id)).then(a.id.cmp(&b.id)));
    // The closest few, one of them: not always the same system for the same station.
    let n = near.len().min(3);
    (n > 0).then(|| near[r.below(n as u32) as usize].id)
}

/// The station's board for a shift. Same station and shift, same jobs.
pub fn board(t: &Tuning, station: SystemId, period: u64) -> Vec<Job> {
    let j = &t.trade.jobs;
    let tier = galaxy::system(station).tier;
    let mut r = Rng::from_parts(&[0x10B5, station as u64, period]);
    let weights: Vec<(JobTag, u32)> = j.kinds.get(&tier).map(|m| m.iter().map(|(k, w)| (*k, *w)).collect()).unwrap_or_default();
    let total: u32 = weights.iter().map(|w| w.1).sum();
    let mut out: Vec<Job> = vec![];
    let mut tries = 0;
    while out.len() < j.per_board && tries < j.per_board * 8 && total > 0 {
        tries += 1;
        let mut roll = r.below(total);
        let tag = weights.iter().find(|(_, w)| if roll < *w { true } else { roll -= w; false }).map(|w| w.0).unwrap();
        // At most two of a kind on one board.
        if out.iter().filter(|o| o.kind.tag() == tag).count() >= 2 {
            continue;
        }
        let id = Rng::from_parts(&[0x0010_B51D, station as u64, period, out.len() as u64]).next_u64();
        if let Some(job) = make(t, station, tag, id, &mut r)
            && !out.iter().any(|o| same_errand(&o.kind, &job.kind, station))
        {
            out.push(job);
        }
    }
    out
}

/// Two jobs that send a captain on the same errand (one board never posts both).
fn same_errand(a: &JobKind, b: &JobKind, station: SystemId) -> bool {
    match (a, b) {
        (JobKind::Supply { good: x, .. }, JobKind::Supply { good: y, .. }) => x == y,
        (JobKind::Prospect { rarity: x, .. }, JobKind::Prospect { rarity: y, .. }) => x == y,
        _ => a.tag() == b.tag() && a.destinations(station) == b.destinations(station),
    }
}

fn make(t: &Tuning, station: SystemId, tag: JobTag, id: JobId, r: &mut Rng) -> Option<Job> {
    let j = &t.trade.jobs;
    let m = |credits: f32, alloy: u32, crystal: u32, datacore: u32| Materials { credits: (credits / 50.0).round() as u64 * 50, alloy, crystal, datacore, relic: 0 };
    let (kind, reward, reputation, limit) = match tag {
        JobTag::Bounty => {
            let system = nearest_lawless(station, j.bounty_range, r)?;
            let tier = galaxy::system(system).tier;
            let grade = pick_weighted(&j.bounty_grades, |g| g.weight, r)?;
            let level = (tier.pirate_level() as i32 + grade.level).max(0) as u8;
            // The war zones' warlords take the place of their hardest gunships.
            let archetype = if matches!(tier, RiskTier::WarZone) && grade.archetype == NpcArchetype::Gunship && grade.escorts.0 >= 2 && r.below(3) == 0 { NpcArchetype::Warlord } else { grade.archetype };
            let escorts = grade.escorts.0 + r.below(grade.escorts.1.saturating_sub(grade.escorts.0) + 1);
            let target = format!("{} {}", r.pick(&j.names_first), r.pick(&j.names_last));
            let boss = if archetype == NpcArchetype::Warlord { 2.2 } else { 1.0 };
            let credits = j.bounty * risk(t, system) * boss * (1.0 + 0.35 * level as f32) * (1.0 + 0.25 * escorts as f32);
            (JobKind::Bounty { system, target, archetype, level, escorts }, m(credits, 4 + 2 * level as u32, 1 + level as u32, 0), 2, j.limit_secs)
        }
        JobTag::Haul => {
            let to: Vec<&galaxy::SystemDef> = reachable().filter(|s| s.id != station && (j.haul_range.0..=j.haul_range.1).contains(&distance(station, s.id))).collect();
            let to = (!to.is_empty()).then(|| r.pick(&to).id)?;
            let units = j.haul_units.0 + r.below(j.haul_units.1 - j.haul_units.0 + 1);
            let ly = distance(station, to);
            let credits = j.haul_base + j.haul_per_unit_ly * units as f32 * ly * risk(t, to);
            let limit = j.haul_secs_base + (j.haul_secs_per_ly * ly) as u64;
            (JobKind::Haul { to, units }, m(credits, 0, 0, 0), 1, limit)
        }
        JobTag::Survey => {
            let mut far: Vec<&galaxy::SystemDef> = reachable().filter(|s| s.id != station && (j.survey_range.0..=j.survey_range.1).contains(&distance(station, s.id))).collect();
            if far.len() < 2 {
                return None;
            }
            let n = (2 + r.below(2) as usize).min(far.len());
            let mut systems = vec![];
            for _ in 0..n {
                systems.push(far.remove(r.below(far.len() as u32) as usize).id);
            }
            systems.sort();
            let credits: f32 = systems.iter().map(|s| j.survey_per_system * risk(t, *s)).sum();
            (JobKind::Survey { systems }, m(credits, 0, 2 * n as u32, n as u32), 2, j.limit_secs * 2)
        }
        JobTag::Prospect => {
            let rarity = if r.below(4) == 0 { Rarity::Exotic } else { Rarity::Rare };
            let count = if rarity == Rarity::Exotic { 1 } else { 1 + r.below(2) };
            let each = t.minerals.specimen_value.get(&rarity).copied().unwrap_or(0) as f32;
            (JobKind::Prospect { rarity, count }, m(each * count as f32 * j.prospect_premium, 0, 3 * count, 1), 2, j.limit_secs * 3)
        }
        JobTag::Armada => {
            let system = nearest_lawless(station, j.bounty_range, r)?;
            let tier = galaxy::system(system).tier;
            let scale = pick_weighted(&j.armada_scale, |s| s.0, r)?.1;
            let wave: Vec<(NpcArchetype, u32)> = j.armada.get(&tier)?.iter().map(|(a, n)| (*a, (*n as f32 * scale).round() as u32)).filter(|w| w.1 > 0).collect();
            if wave.is_empty() {
                return None;
            }
            let level = tier.pirate_level();
            let ships: u32 = wave.iter().map(|w| w.1).sum();
            let credits = j.armada_per_ship * ships as f32 * risk(t, system) * (1.0 + 0.3 * level as f32);
            (JobKind::Armada { system, level, wave }, m(credits, 6 * ships, 2 * ships, ships / 2), 5, j.armada_secs)
        }
        JobTag::Smuggle => {
            let mt = &t.trade.market;
            let to: Vec<&galaxy::SystemDef> = reachable().filter(|s| s.id != station && mt.black_market.contains(&s.tier) && (j.haul_range.0..=j.haul_range.1).contains(&distance(station, s.id))).collect();
            let to = (!to.is_empty()).then(|| r.pick(&to).id)?;
            let units = j.haul_units.0 + r.below(j.haul_units.1 - j.haul_units.0 + 1);
            let ly = distance(station, to);
            let credits = (j.haul_base + j.haul_per_unit_ly * units as f32 * ly * risk(t, to)) * j.smuggle_premium;
            let limit = j.haul_secs_base + (j.haul_secs_per_ly * ly) as u64;
            (JobKind::Smuggle { to, units }, m(credits, 0, 0, 0), 0, limit)
        }
        JobTag::Patrol => {
            let mut near: Vec<&galaxy::SystemDef> = reachable().filter(|s| s.tier != RiskTier::Core && distance(station, s.id) <= j.bounty_range).collect();
            if near.len() < 2 {
                return None;
            }
            let n = (j.patrol_systems.0 + r.below(j.patrol_systems.1.saturating_sub(j.patrol_systems.0) + 1)).min(near.len() as u32) as usize;
            let mut systems = vec![];
            for _ in 0..n {
                systems.push(near.remove(r.below(near.len() as u32) as usize).id);
            }
            systems.sort();
            let level = systems.iter().map(|s| galaxy::system(*s).tier.pirate_level()).max().unwrap_or(0);
            let credits: f32 = systems.iter().map(|s| j.patrol_per_system * risk(t, *s)).sum();
            (JobKind::Patrol { systems, level }, m(credits, 3 * n as u32, n as u32, 0), 2, j.limit_secs)
        }
        JobTag::Salvage => {
            let near: Vec<&galaxy::SystemDef> = reachable().filter(|s| distance(station, s.id) <= j.haul_range.1).collect();
            let system = r.pick(&near).id;
            let wrecks: Vec<galaxy::Poi> = galaxy::system_layout(system).pois.into_iter().filter(|p| p.kind == galaxy::PoiKind::Derelict).collect();
            let wreck = (!wrecks.is_empty()).then(|| wrecks[r.below(wrecks.len() as u32) as usize].clone())?;
            let credits = j.salvage * risk(t, system) * (1.0 + 0.25 * distance(station, system));
            (JobKind::Salvage { system, poi: wreck.id, site: wreck.name }, m(credits, 6, 0, 1), 2, j.limit_secs)
        }
        JobTag::Supply => {
            let mt = &t.trade.market;
            let needs = market::economy(mt, station).demand;
            let needs: Vec<Good> = needs.into_iter().filter(|g| market::sell_price(mt, station, *g, 0.0).is_some()).collect();
            let good = *needs.get(r.below(needs.len().max(1) as u32) as usize)?;
            let units = j.supply_units.0 + r.below(j.supply_units.1 - j.supply_units.0 + 1);
            let price = market::sell_price(mt, station, good, 0.0)?;
            (JobKind::Supply { good, units }, m(price * units as f32 * j.supply_premium, 0, 0, 0), 1, j.limit_secs * 2)
        }
    };
    Some(Job { id, station, kind, reward, reputation, limit_secs: limit })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boards_post_fights_for_small_ships_and_big_ones() {
        // Over a day of boards, a Lv1 ship finds bounties it is meant for, and the hardest fights are for big ships.
        let t = crate::tuning::get();
        let mut recs = vec![];
        for s in 0..galaxy::systems().len() as SystemId {
            for p in 0..48 {
                recs.extend(board(&t, s, p).iter().filter_map(|j| recommended(&t, &j.kind).map(|r| (j.kind.tag(), r))));
            }
        }
        let share = |k: JobTag, f: &dyn Fn(u8) -> bool| recs.iter().filter(|(t, r)| *t == k && f(*r)).count() as f32 / recs.iter().filter(|(t, _)| *t == k).count().max(1) as f32;
        assert!(share(JobTag::Bounty, &|r| r == 1) > 0.3, "Lv1 bounties: {}", share(JobTag::Bounty, &|r| r == 1));
        // Armadas are the big fleet battles: only the smallest frontier ones are for small ships.
        assert!(share(JobTag::Armada, &|r| r <= 3) > 0.05, "small armadas: {}", share(JobTag::Armada, &|r| r <= 3));
        assert!(recs.iter().any(|(_, r)| *r >= 6), "some fights for big ships");
        // Recommendations grow with the fight.
        let lone = JobKind::Bounty { system: 3, target: String::new(), archetype: NpcArchetype::Raider, level: 1, escorts: 0 };
        let pack = JobKind::Bounty { system: 3, target: String::new(), archetype: NpcArchetype::Gunship, level: 3, escorts: 3 };
        assert!(recommended(&t, &lone) < recommended(&t, &pack));
        assert_eq!(recommended(&t, &JobKind::Haul { to: 3, units: 9 }), None);
    }

    #[test]
    fn a_board_is_fixed_for_its_shift_and_changes_with_the_next() {
        let t = crate::tuning::get();
        for s in [0, 3, 5, 7, 12, 30] {
            let a = board(&t, s, 100);
            assert_eq!(a, board(&t, s, 100));
            assert!(a.len() >= t.trade.jobs.per_board - 1, "{}: {} jobs", galaxy::system(s).name, a.len());
            assert!((101..106).any(|p| board(&t, s, p) != a));
            for (i, x) in a.iter().enumerate() {
                assert!(a[i + 1..].iter().all(|y| !same_errand(&x.kind, &y.kind, s)), "the same errand twice: {x:?}");
            }
            let mut ids: Vec<JobId> = a.iter().map(|j| j.id).collect();
            ids.sort();
            ids.dedup();
            assert_eq!(ids.len(), a.len(), "ids unique on a board");
            for job in &a {
                assert!(job.reward.credits > 0, "{job:?}");
                assert!(!job.kind.destinations(s).contains(&9), "nothing sends a captain beyond the gate");
            }
        }
    }

    #[test]
    fn stations_ask_for_what_they_are() {
        let t = crate::tuning::get();
        let kinds = |s: SystemId| -> Vec<JobTag> { (0..40).flat_map(|p| board(&t, s, p)).map(|j| j.kind.tag()).collect() };
        let core = kinds(0);
        let war = kinds(7);
        let count = |v: &Vec<JobTag>, k: JobTag| v.iter().filter(|x| **x == k).count();
        assert!(count(&core, JobTag::Haul) + count(&core, JobTag::Supply) > count(&core, JobTag::Armada) * 3, "the core wants goods and couriers");
        assert!(count(&war, JobTag::Armada) + count(&war, JobTag::Bounty) > count(&war, JobTag::Haul), "war zones want guns");
        // Over a day every kind of work shows up somewhere.
        let all: Vec<JobTag> = (0..galaxy::systems().len() as SystemId).flat_map(|s| board(&t, s, 7)).map(|j| j.kind.tag()).collect();
        for k in JobTag::ALL {
            assert!(all.contains(&k), "{k:?}");
        }
    }

    #[test]
    fn farther_and_riskier_pays_more() {
        let t = crate::tuning::get();
        let hauls: Vec<(f32, f32, u64)> = (0..galaxy::systems().len() as SystemId)
            .flat_map(|s| (0..20).flat_map(move |p| board(&crate::tuning::get(), s, p)))
            .filter_map(|j| match j.kind {
                JobKind::Haul { to, units } => Some((distance(j.station, to), units as f32, j.reward.credits)),
                _ => None,
            })
            .collect();
        assert!(hauls.len() > 20);
        let (near, far): (Vec<_>, Vec<_>) = hauls.iter().partition(|h| h.0 < 2.0);
        let per_unit = |v: &Vec<&(f32, f32, u64)>| v.iter().map(|h| h.2 as f32 / h.1).sum::<f32>() / v.len().max(1) as f32;
        assert!(per_unit(&far) > per_unit(&near), "far {} near {}", per_unit(&far), per_unit(&near));
        let _ = t;
    }

    #[test]
    fn a_job_reports_what_is_left_and_when_it_runs_out() {
        let t = crate::tuning::get();
        let job = (0..50).flat_map(|p| board(&t, 0, p)).find(|j| matches!(j.kind, JobKind::Survey { .. })).expect("a survey from Sol Haven");
        let JobKind::Survey { systems } = job.kind.clone() else { unreachable!() };
        let mut a = ActiveJob { job, accepted: 1000, visited: vec![] };
        assert_eq!(a.remaining(), systems);
        a.visited.push(systems[0]);
        assert_eq!(a.remaining(), systems[1..].to_vec());
        assert!(!a.expired(a.deadline()) && a.expired(a.deadline() + 1));
        assert!(a.status(1000).contains(&format!("1/{}", systems.len())), "{}", a.status(1000));
    }
}

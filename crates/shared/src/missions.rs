//! Daily missions (deterministic from the UTC day + player id) and the scenario campaign.

use crate::galaxy::{PoiKind, SystemId};
use crate::rng::Rng;
use crate::ships::Materials;
use serde::{Deserialize, Serialize};

pub const SECONDS_PER_DAY: u64 = 86_400;

pub fn day_index(unix_secs: u64) -> u32 {
    (unix_secs / SECONDS_PER_DAY) as u32
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Objective {
    KillPirates { count: u32 },
    KillPlayers { count: u32 },
    MineOre { units: u32 },
    Scan { count: u32 },
    Visit { system: SystemId },
    SellCargo { units: u32 },
    WinEngagement,
}

impl Objective {
    pub fn target(&self) -> u32 {
        match self {
            Objective::KillPirates { count } | Objective::KillPlayers { count } | Objective::Scan { count } => *count,
            Objective::MineOre { units } | Objective::SellCargo { units } => *units,
            Objective::Visit { .. } | Objective::WinEngagement => 1,
        }
    }

    pub fn describe(&self) -> String {
        match self {
            Objective::KillPirates { count } => format!("해적 {count}척 격파"),
            Objective::KillPlayers { count } => format!("적대 함장 {count}명 격파 (PvP)"),
            Objective::MineOre { units } => format!("광석 {units}단위 채굴"),
            Objective::Scan { count } => format!("이상현상/유적/잔해 {count}곳 스캔"),
            Objective::Visit { system } => format!("{} 성계 방문", crate::galaxy::system(*system).name),
            Objective::SellCargo { units } => format!("정거장에 화물 {units}단위 판매"),
            Objective::WinEngagement => "함대 교전 승리".into(),
        }
    }
}

/// Progress signals the server feeds into missions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Progress {
    KilledPirate,
    KilledPlayer,
    Mined(u32),
    Scanned,
    Visited(SystemId),
    Sold(u32),
    WonEngagement,
}

impl Progress {
    pub fn amount_for(&self, obj: &Objective) -> u32 {
        match (self, obj) {
            (Progress::KilledPirate, Objective::KillPirates { .. }) => 1,
            (Progress::KilledPlayer, Objective::KillPlayers { .. }) => 1,
            (Progress::Mined(n), Objective::MineOre { .. }) => *n,
            (Progress::Scanned, Objective::Scan { .. }) => 1,
            (Progress::Visited(s), Objective::Visit { system }) if s == system => 1,
            (Progress::Sold(n), Objective::SellCargo { .. }) => *n,
            (Progress::WonEngagement, Objective::WinEngagement) => 1,
            _ => 0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DailyMission {
    pub objective: Objective,
    pub progress: u32,
    pub reward: Materials,
    pub claimed: bool,
}

impl DailyMission {
    pub fn complete(&self) -> bool {
        self.progress >= self.objective.target()
    }
}

pub const DAILY_COUNT: usize = 4;

/// The same day and player always roll the same missions; a new UTC day rolls new ones.
pub fn daily_missions(day: u32, player_id: u64) -> Vec<DailyMission> {
    let mut r = Rng::from_parts(&[0xDA11_7, day as u64, player_id]);
    let mut out: Vec<DailyMission> = Vec::new();
    let pool = |r: &mut Rng| -> Objective {
        match r.below(7) {
            0 => Objective::KillPirates { count: 3 + r.below(5) },
            1 => Objective::MineOre { units: 30 + 10 * r.below(5) },
            2 => Objective::Scan { count: 2 + r.below(2) },
            3 => Objective::Visit { system: 3 + r.below(5) as SystemId },
            4 => Objective::SellCargo { units: 25 + 5 * r.below(6) },
            5 => Objective::KillPlayers { count: 1 },
            _ => Objective::WinEngagement,
        }
    };
    // Always one combat mission so the fight loop is part of every day.
    let mut first = true;
    while out.len() < DAILY_COUNT {
        let obj = if first { Objective::KillPirates { count: 4 + r.below(4) } } else { pool(&mut r) };
        first = false;
        if out.iter().any(|m| std::mem::discriminant(&m.objective) == std::mem::discriminant(&obj)) {
            continue;
        }
        let weight = match &obj {
            Objective::KillPlayers { .. } | Objective::WinEngagement => 3,
            Objective::KillPirates { count } => 1 + count / 4,
            Objective::Visit { .. } => 1,
            _ => 1,
        };
        let reward = Materials {
            credits: 900 * weight as u64 + 100 * r.below(6) as u64,
            alloy: 6 * weight + r.below(5),
            crystal: 3 * weight + r.below(3),
            datacore: weight.saturating_sub(1) + r.below(2),
            relic: 0,
        };
        out.push(DailyMission { objective: obj, progress: 0, reward, claimed: false });
    }
    out
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum NpcArchetype {
    Raider,
    Gunship,
    Warden,
    Warlord,
    Sentinel,
}

impl NpcArchetype {
    pub const ALL: [NpcArchetype; 5] = [NpcArchetype::Raider, NpcArchetype::Gunship, NpcArchetype::Warden, NpcArchetype::Warlord, NpcArchetype::Sentinel];

    pub fn name(self) -> &'static str {
        match self {
            NpcArchetype::Raider => "Raider",
            NpcArchetype::Gunship => "Gunship",
            NpcArchetype::Warden => "Veyr Warden",
            NpcArchetype::Warlord => "Kharz Warlord",
            NpcArchetype::Sentinel => "Leviathan Sentinel",
        }
    }
    /// (hull, shield, damage, fire interval, speed, radius) at `level` (`tuning/npcs.ron`).
    pub fn stats(self, level: u8) -> (f32, f32, f32, f32, f32, f32) {
        let t = crate::tuning::get();
        let l = 1.0 + t.npcs.level_scale * level as f32;
        let d = &t.npcs.archetypes[&self];
        (d.hull * l, d.shield * l, d.damage * l, d.fire_interval, d.speed, d.radius)
    }
    pub fn bounty(self, level: u8) -> Materials {
        let mut m = crate::tuning::get().npcs.archetypes[&self].bounty.clone();
        m.credits *= 1 + level as u64;
        m
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Step {
    /// Arrive in a system.
    Reach { system: SystemId },
    /// Scan N POIs of a kind in the current system.
    ScanPoi { system: SystemId, kind: PoiKind, count: u32 },
    /// Defeat a spawned wave (spawned near the player when the step starts).
    Defeat { system: SystemId, wave: Vec<(NpcArchetype, u32)>, level: u8 },
}

impl Step {
    pub fn describe(&self) -> String {
        let sys = |s: &SystemId| crate::galaxy::system(*s).name;
        match self {
            Step::Reach { system } => format!("{} 성계로 워프", sys(system)),
            Step::ScanPoi { system, kind, count } => format!("{}에서 {} {}곳 스캔", sys(system), poi_name(*kind), count),
            Step::Defeat { system, wave, .. } => {
                let w: Vec<String> = wave.iter().map(|(a, n)| format!("{} x{}", a.name(), n)).collect();
                format!("{}에서 격파: {}", sys(system), w.join(", "))
            }
        }
    }
    pub fn system(&self) -> SystemId {
        match self {
            Step::Reach { system } | Step::ScanPoi { system, .. } | Step::Defeat { system, .. } => *system,
        }
    }
}

pub fn poi_name(k: PoiKind) -> &'static str {
    match k {
        PoiKind::Anomaly => "이상현상",
        PoiKind::Ruin => "고대 유적",
        PoiKind::Derelict => "잔해선",
        PoiKind::Signal => "미확인 신호",
        PoiKind::AncientGate => "고대 게이트",
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Scenario {
    pub id: u32,
    pub title: &'static str,
    pub briefing: &'static str,
    pub steps: Vec<Step>,
    pub reward: Materials,
}

pub fn scenarios() -> Vec<Scenario> {
    use NpcArchetype::*;
    vec![
        Scenario {
            id: 0,
            title: "1장 · 오리온의 신호",
            briefing: "Orion Drift 변경에서 정체불명의 신호가 잡혔다. 신호를 추적하고, 그것을 노리는 해적을 몰아내라.",
            steps: vec![
                Step::Reach { system: 3 },
                Step::ScanPoi { system: 3, kind: PoiKind::Signal, count: 1 },
                Step::Defeat { system: 3, wave: vec![(Raider, 3)], level: 1 },
            ],
            reward: Materials { credits: 3000, alloy: 25, crystal: 12, datacore: 3, relic: 0 },
        },
        Scenario {
            id: 1,
            title: "2장 · 재의 장부",
            briefing: "Vesta 회랑에서 사라진 호송선단의 항해 기록이 발견됐다. 잔해에서 로그를 회수하고 매복을 버텨라.",
            steps: vec![
                Step::Reach { system: 4 },
                Step::ScanPoi { system: 4, kind: PoiKind::Derelict, count: 1 },
                Step::ScanPoi { system: 4, kind: PoiKind::Signal, count: 1 },
                Step::Defeat { system: 4, wave: vec![(Raider, 3), (Gunship, 1)], level: 2 },
            ],
            reward: Materials { credits: 6000, alloy: 40, crystal: 20, datacore: 6, relic: 1 },
        },
        Scenario {
            id: 2,
            title: "3장 · 세 번째 배신의 기억",
            briefing: "Hollow Crown의 왕관 유적은 Veyr 기억과 인류 기록이 엇갈리는 장소다. 유적 셋을 조사하고 유적의 수호자를 물리쳐라.",
            steps: vec![
                Step::Reach { system: 6 },
                Step::ScanPoi { system: 6, kind: PoiKind::Ruin, count: 3 },
                Step::Defeat { system: 6, wave: vec![(Warden, 4)], level: 3 },
            ],
            reward: Materials { credits: 10000, alloy: 60, crystal: 40, datacore: 10, relic: 2 },
        },
        Scenario {
            id: 3,
            title: "4장 · 피의 계약",
            briefing: "Ashen Gate의 Kharz 군벌이 계약을 깨고 관문을 봉쇄했다. 신호를 따라 군벌 기함을 찾아 격침하라.",
            steps: vec![
                Step::Reach { system: 8 },
                Step::ScanPoi { system: 8, kind: PoiKind::Signal, count: 1 },
                Step::Defeat { system: 8, wave: vec![(Gunship, 2), (Warlord, 1)], level: 3 },
            ],
            reward: Materials { credits: 16000, alloy: 90, crystal: 60, datacore: 16, relic: 3 },
        },
        Scenario {
            id: 4,
            title: "5장 · 너머의 관문",
            briefing: "The Deep의 고대 워프 게이트가 깨어나고 있다. 게이트를 스캔하고, 그것을 지키는 리바이어던 센티넬을 쓰러뜨려라.",
            steps: vec![
                Step::Reach { system: 9 },
                Step::ScanPoi { system: 9, kind: PoiKind::AncientGate, count: 1 },
                Step::Defeat { system: 9, wave: vec![(Warden, 3), (Sentinel, 1)], level: 4 },
            ],
            reward: Materials { credits: 40000, alloy: 150, crystal: 120, datacore: 30, relic: 6 },
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dailies_are_deterministic_and_vary_by_day() {
        let a = daily_missions(100, 7);
        assert_eq!(a, daily_missions(100, 7));
        assert_eq!(a.len(), DAILY_COUNT);
        assert!(matches!(a[0].objective, Objective::KillPirates { .. }));
        let days_differ = (101..110).any(|d| daily_missions(d, 7) != a);
        assert!(days_differ);
    }

    #[test]
    fn dailies_have_no_duplicate_kinds() {
        for d in 0..200 {
            let m = daily_missions(d, 3);
            for i in 0..m.len() {
                for j in i + 1..m.len() {
                    assert_ne!(std::mem::discriminant(&m[i].objective), std::mem::discriminant(&m[j].objective));
                }
            }
        }
    }

    #[test]
    fn progress_matching() {
        assert_eq!(Progress::Visited(4).amount_for(&Objective::Visit { system: 4 }), 1);
        assert_eq!(Progress::Visited(5).amount_for(&Objective::Visit { system: 4 }), 0);
        assert_eq!(Progress::Mined(7).amount_for(&Objective::MineOre { units: 30 }), 7);
    }

    #[test]
    fn five_scenarios_with_rising_rewards() {
        let s = scenarios();
        assert_eq!(s.len(), 5);
        for w in s.windows(2) {
            assert!(w[1].reward.credits > w[0].reward.credits);
        }
    }
}

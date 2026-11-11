//! Titles (`tuning/trade.ron` `titles`): what a captain's career has made of them.
//!
//! Earned, never bought: by clearing the campaign, and by the record a captain builds in the open
//! galaxy — bounties collected, cargo carried, systems charted, armadas broken, fortunes made. A
//! captain wears one (`Profile::title`); the rest stay in the record. Thresholds are tuning.

use crate::galaxy::SystemId;
use crate::jobs::JobTag;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Title {
    /// Campaign chapter 1.
    Tracker,
    /// The whole campaign.
    GateWalker,
    BountyHunter,
    PirateBane,
    Courier,
    Merchant,
    Pathfinder,
    Cartographer,
    Prospector,
    Defender,
    Admiral,
    Smuggler,
    Quartermaster,
    /// One of every kind of job.
    Freelancer,
    /// This many other titles.
    Legend,
}

impl Title {
    pub const ALL: [Title; 15] = [
        Title::Tracker,
        Title::GateWalker,
        Title::BountyHunter,
        Title::PirateBane,
        Title::Courier,
        Title::Merchant,
        Title::Pathfinder,
        Title::Cartographer,
        Title::Prospector,
        Title::Defender,
        Title::Admiral,
        Title::Smuggler,
        Title::Quartermaster,
        Title::Freelancer,
        Title::Legend,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Title::Tracker => "오리온의 추적자",
            Title::GateWalker => "관문 너머",
            Title::BountyHunter => "현상금 사냥꾼",
            Title::PirateBane => "해적의 재앙",
            Title::Courier => "성간 운송인",
            Title::Merchant => "대상인",
            Title::Pathfinder => "개척자",
            Title::Cartographer => "지도 밖의 사람",
            Title::Prospector => "광맥의 주인",
            Title::Defender => "성계의 방패",
            Title::Admiral => "제독",
            Title::Smuggler => "그림자 상인",
            Title::Quartermaster => "보급관",
            Title::Freelancer => "만능 함장",
            Title::Legend => "전설",
        }
    }

    /// How it is earned, with its threshold `n`.
    pub fn how(self, n: u32) -> String {
        match self {
            Title::Tracker => format!("캠페인 {n}장 돌파"),
            Title::GateWalker => format!("캠페인 {n}장 모두 돌파"),
            Title::BountyHunter => format!("현상금 의뢰 {n}건 완수"),
            Title::PirateBane => format!("해적 {n}척 격파"),
            Title::Courier => format!("운송 의뢰 {n}건 완수"),
            Title::Merchant => format!("교역 순이익 {n} CR"),
            Title::Pathfinder => format!("성계 {n}곳 방문"),
            Title::Cartographer => format!("성계 {n}곳 방문"),
            Title::Prospector => format!("광물 주문 {n}건 완수"),
            Title::Defender => format!("함대 방어전 {n}회 승리"),
            Title::Admiral => format!("함대 방어전 {n}회 승리"),
            Title::Smuggler => format!("밀수품 {n}단위 판매"),
            Title::Quartermaster => format!("보급 의뢰 {n}건 완수"),
            Title::Freelancer => format!("모든 종류의 의뢰를 {n}건 이상씩 완수"),
            Title::Legend => format!("다른 칭호 {n}개 획득"),
        }
    }
}

/// A captain's record in the open galaxy.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Career {
    /// Jobs completed, by kind.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub jobs: BTreeMap<JobTag, u32>,
    /// Credits from selling trade goods minus what buying them cost.
    #[serde(default)]
    pub trade_profit: i64,
    /// Systems this captain has been to.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub visited: BTreeSet<SystemId>,
    /// Units of contraband sold.
    #[serde(default)]
    pub smuggled: u32,
}

impl Career {
    pub fn jobs_done(&self, tag: JobTag) -> u32 {
        self.jobs.get(&tag).copied().unwrap_or(0)
    }
}

/// Every title this record has earned (in [`Title::ALL`] order).
pub fn earned(thresholds: &BTreeMap<Title, u32>, chapters: u32, kills: u32, c: &Career) -> Vec<Title> {
    let n = |t: Title| thresholds.get(&t).copied().unwrap_or(u32::MAX);
    let has = |t: Title| -> bool {
        let need = n(t);
        match t {
            Title::Tracker | Title::GateWalker => chapters >= need,
            Title::BountyHunter => c.jobs_done(JobTag::Bounty) >= need,
            Title::PirateBane => kills >= need,
            Title::Courier => c.jobs_done(JobTag::Haul) >= need,
            Title::Merchant => c.trade_profit >= need as i64,
            Title::Pathfinder | Title::Cartographer => c.visited.len() as u32 >= need,
            Title::Prospector => c.jobs_done(JobTag::Prospect) >= need,
            Title::Defender | Title::Admiral => c.jobs_done(JobTag::Armada) >= need,
            Title::Smuggler => c.smuggled >= need,
            Title::Quartermaster => c.jobs_done(JobTag::Supply) >= need,
            Title::Freelancer => JobTag::ALL.iter().all(|k| c.jobs_done(*k) >= need),
            Title::Legend => false,
        }
    };
    let mut out: Vec<Title> = Title::ALL.into_iter().filter(|t| has(*t)).collect();
    if out.len() as u32 >= n(Title::Legend) {
        out.push(Title::Legend);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles_follow_the_record() {
        let th = crate::tuning::get().trade.titles.clone();
        for t in Title::ALL {
            assert!(th.contains_key(&t), "trade.ron titles has no {t:?}");
        }
        let mut c = Career::default();
        assert!(earned(&th, 0, 0, &c).is_empty());
        assert_eq!(earned(&th, th[&Title::Tracker], 0, &c), vec![Title::Tracker]);
        assert!(earned(&th, 5, 0, &c).contains(&Title::GateWalker) == (5 >= th[&Title::GateWalker]));
        c.jobs.insert(JobTag::Bounty, th[&Title::BountyHunter]);
        assert!(earned(&th, 0, 0, &c).contains(&Title::BountyHunter));
        for k in JobTag::ALL {
            c.jobs.entry(k).or_insert(0);
            *c.jobs.get_mut(&k).unwrap() = c.jobs[&k].max(th[&Title::Freelancer]);
        }
        assert!(earned(&th, 0, 0, &c).contains(&Title::Freelancer));
        c.visited = (0..th[&Title::Cartographer] as SystemId).collect();
        let e = earned(&th, 0, 0, &c);
        assert!(e.contains(&Title::Pathfinder) && e.contains(&Title::Cartographer));
        // A captain with everything is a legend.
        let all = Career { jobs: JobTag::ALL.iter().map(|k| (*k, 99)).collect(), trade_profit: 10_000_000, visited: (0..99).collect(), smuggled: 999 };
        let e = earned(&th, 99, 9999, &all);
        assert_eq!(e.len(), Title::ALL.len());
        assert_eq!(*e.last().unwrap(), Title::Legend);
    }
}

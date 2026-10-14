//! History data model shared by server (producer) and client (reader).
//! Mirrors the design doc: Fact (HistoricalEvent) ≠ Evidence ≠ Claim (Research).

use crate::galaxy::SystemId;
use serde::{Deserialize, Serialize};

pub type EventId = u64;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Importance {
    Trivial = 0,
    Minor = 1,
    Notable = 2,
    Major = 3,
    Historic = 4,
}

impl Importance {
    pub fn label(self) -> &'static str {
        match self {
            Importance::Trivial => "사소",
            Importance::Minor => "소",
            Importance::Notable => "주목",
            Importance::Major => "중대",
            Importance::Historic => "역사적",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EvidenceKind {
    ShipLog,
    Testimony,
    SensorRecord,
    VeyrMemory,
    KharzContract,
    ArchiveRecord,
}

impl EvidenceKind {
    pub fn name(self) -> &'static str {
        match self {
            EvidenceKind::ShipLog => "함선 로그",
            EvidenceKind::Testimony => "목격자 증언",
            EvidenceKind::SensorRecord => "센서 기록",
            EvidenceKind::VeyrMemory => "Veyr 기억",
            EvidenceKind::KharzContract => "Kharz 계약서",
            EvidenceKind::ArchiveRecord => "기록보관소 문서",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Evidence {
    pub id: u64,
    pub kind: EvidenceKind,
    pub event: EventId,
    pub source: String,
    pub text: String,
    /// 0..1 — how much the source tends to be right; not "truth".
    pub reliability: f32,
    /// Whether it has been discovered by anyone yet (fog of history).
    pub discovered_by: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HistoricalEvent {
    pub id: EventId,
    /// Galactic year/day string, e.g. "3827.142".
    pub date: String,
    pub unix: u64,
    pub system: SystemId,
    pub kind: String,
    pub title: String,
    pub summary: String,
    pub actors: Vec<String>,
    pub ships: Vec<String>,
    pub importance: Importance,
    pub causes: Vec<EventId>,
    pub evidence: Vec<u64>,
    /// Research claims published about this event.
    pub research: Vec<Research>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Research {
    pub author: String,
    pub thesis: String,
    pub cited: Vec<u64>,
    /// Derived from cited evidence reliability; disputes lower it.
    pub confidence: f32,
    pub disputed_by: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BioEntry {
    pub date: String,
    pub text: String,
    pub event: Option<EventId>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Headline {
    pub date: String,
    pub headline: String,
    pub body: String,
}

/// Game calendar: 1 real day = 1 galactic year starting at 3827; day-of-year from seconds.
pub fn galactic_date(unix: u64) -> String {
    let epoch = 1_788_000_000u64; // ~2026-08
    let t = unix.saturating_sub(epoch);
    let year = 3827 + t / 86_400;
    let day = (t % 86_400) / 240 + 1; // 360 "days" per real day
    format!("{year}.{day:03}")
}

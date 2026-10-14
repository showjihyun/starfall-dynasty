//! Wire protocol (JSON text frames over WebSocket). The server is authoritative:
//! clients only send intents and control axes; every state change comes back from the server.

use crate::chronicle::{BioEntry, Evidence, EventId, Headline, HistoricalEvent};
use crate::galaxy::{Ore, Species, SystemId};
use crate::missions::{DailyMission, NpcArchetype};
use crate::ships::{HullClass, Materials, Module, Ship, SizeClass, WeaponKind};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const PROTOCOL_VERSION: u32 = 1;
pub const DEFAULT_PORT: u16 = 7777;
pub const TICK_HZ: f32 = 20.0;

pub type EntityId = u32;
pub type PlayerId = u64;
pub type FleetId = u32;
pub type AllianceId = u32;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Profession {
    Explorer,
    Miner,
    Trader,
    CombatPilot,
    Historian,
}

impl Profession {
    pub const ALL: [Profession; 5] = [Profession::Explorer, Profession::Miner, Profession::Trader, Profession::CombatPilot, Profession::Historian];
    pub fn name(self) -> &'static str {
        match self {
            Profession::Explorer => "Explorer · 탐험가",
            Profession::Miner => "Miner · 광부",
            Profession::Trader => "Trader · 상인",
            Profession::CombatPilot => "Combat Pilot · 전투 조종사",
            Profession::Historian => "Historian · 역사학자",
        }
    }
    pub fn perk(self) -> &'static str {
        match self {
            Profession::Explorer => "스캔 보상 +50%, 스캔 시간 -25%",
            Profession::Miner => "채굴 속도 +40%",
            Profession::Trader => "판매 가격 +20%",
            Profession::CombatPilot => "격파 보상 +30%, 피해 +8%",
            Profession::Historian => "잔해 조사 시 증거 2배, 연구 신뢰도 +",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Controls {
    /// -1..1 forward/back.
    pub thrust: f32,
    /// -1..1 right/left, up/down.
    pub strafe_x: f32,
    pub strafe_y: f32,
    /// -1..1 rotation rates.
    pub yaw: f32,
    pub pitch: f32,
    pub roll: f32,
    pub boost: bool,
    pub fire: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ClientMsg {
    Hello { version: u32, name: String, token: Option<String> },
    CreateCharacter { species: Species, profession: Profession, hull: HullClass, ship_name: String, livery: u8 },
    Controls(Controls),
    SetTarget(Option<EntityId>),
    FireMissile,
    Warp { system: SystemId },
    Dock,
    Undock,
    Mine { asteroid: Option<u32> },
    Scan { poi: u32 },
    InvestigateWreck { wreck: EntityId },
    Upgrade { module: Module },
    Sell { ore: Ore, units: u32 },
    Refine { ore: Ore, units: u32 },
    ClaimDaily { index: usize },
    StartScenario { id: u32 },
    AbandonScenario,
    FleetInvite { name: String },
    FleetAccept { fleet: FleetId },
    FleetLeave,
    Respawn,
    Chat { text: String },
    RequestChronicle,
    RequestBiography { name: String },
    PublishResearch { event: EventId, thesis: String, cite: Vec<u64> },
    DisputeResearch { event: EventId, author: String },
    AllianceCreate { name: String, tag: String },
    AllianceInvite { name: String },
    AllianceAccept { alliance: AllianceId },
    AllianceLeave,
    AllianceDeclareWar { target: AllianceId },
    AlliancePeace { target: AllianceId },
    RequestAlliances,
    EscortCommand { attack: bool },
    Ping { t: f64 },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ScenarioProgress {
    pub id: u32,
    pub step: usize,
    pub step_progress: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Profile {
    pub player_id: PlayerId,
    pub name: String,
    pub species: Species,
    pub profession: Profession,
    pub ship: Ship,
    pub wallet: Materials,
    pub cargo: BTreeMap<String, u32>,
    pub home: SystemId,
    pub dailies: Vec<DailyMission>,
    pub daily_day: u32,
    pub scenarios_completed: u32,
    pub scenario: Option<ScenarioProgress>,
    pub kills: u32,
    pub deaths: u32,
    pub reputation: i32,
    pub bounty: u64,
    /// Evidence this player has personally recovered (ids).
    pub archive: Vec<u64>,
    /// System the ship was last in (login spawns there, docked).
    #[serde(default)]
    pub last_system: SystemId,
    /// (system, poi) scanned today — reset with the dailies.
    #[serde(default)]
    pub scanned: Vec<(SystemId, u32)>,
}

impl Profile {
    pub fn cargo_used(&self) -> u32 {
        self.cargo.values().sum()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EntityKind {
    Player,
    Npc(NpcArchetype),
    /// Friendly escort flying with a player's flagship (`owner`).
    Escort,
    Missile,
    Wreck,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EntityState {
    pub id: EntityId,
    pub kind: EntityKind,
    pub name: String,
    pub hull_class: Option<HullClass>,
    pub rank: u8,
    pub livery: u8,
    pub pos: [f32; 3],
    pub rot: [f32; 4],
    pub vel: [f32; 3],
    pub hull: f32,
    pub shield: f32,
    pub radius: f32,
    pub fleet: Option<FleetId>,
    pub wanted: bool,
    pub boosting: bool,
    pub mining: Option<u32>,
    pub docked: bool,
    pub owner: Option<PlayerId>,
    #[serde(default)]
    pub alliance: Option<AllianceId>,
    /// Size class drives the model tier (fighter / cruiser / capital).
    #[serde(default)]
    pub class: Option<SizeClass>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Fx {
    /// `to == 0` means a miss along the barrel. `mount` picks the muzzle/turret on the model.
    Shot { from: EntityId, to: EntityId, weapon: WeaponKind, mount: u8, hit: bool },
    MissileLaunch { from: EntityId, missile: EntityId },
    Explosion { pos: [f32; 3], size: f32 },
    ShieldHit { entity: EntityId, dir: [f32; 3] },
    WarpOut { entity: EntityId },
    WarpIn { entity: EntityId },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AllianceInfo {
    pub id: AllianceId,
    pub name: String,
    pub tag: String,
    pub leader: String,
    pub members: Vec<String>,
    pub wars: Vec<AllianceId>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FleetMember {
    pub name: String,
    pub hull: HullClass,
    pub system: SystemId,
    pub online: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ServerMsg {
    Welcome { player_id: PlayerId, token: String, profile: Option<Profile>, server_unix: u64 },
    Error { msg: String },
    Toast { text: String },
    Profile(Profile),
    EnterSystem { system: SystemId, you: EntityId },
    Snapshot { tick: u64, entities: Vec<EntityState> },
    Fx(Vec<Fx>),
    WarpCharging { system: SystemId, seconds: f32 },
    ScanProgress { poi: u32, frac: f32 },
    /// Pois already scanned by this player in the current system.
    Scanned { pois: Vec<u32> },
    Died { killer: String, wreck: EntityId },
    Chronicle { events: Vec<HistoricalEvent>, evidence: Vec<Evidence>, herald: Vec<Headline> },
    Biography { name: String, character: Vec<BioEntry>, ship: Vec<BioEntry> },
    FleetInvite { from: String, fleet: FleetId },
    Fleet { id: Option<FleetId>, members: Vec<FleetMember> },
    Engagement { active: bool, sides: Vec<String> },
    Chat { from: String, text: String },
    AllianceInvite { from: String, alliance: AllianceId, name: String },
    Alliances { mine: Option<AllianceId>, list: Vec<AllianceInfo> },
    Pong { t: f64 },
}

pub fn encode<T: Serialize>(m: &T) -> String {
    serde_json::to_string(m).expect("serialize")
}

pub fn decode<'a, T: Deserialize<'a>>(s: &'a str) -> Result<T, serde_json::Error> {
    serde_json::from_str(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let m = ClientMsg::CreateCharacter { species: Species::Veyr, profession: Profession::Historian, hull: HullClass::Seer, ship_name: "Aurora".into(), livery: 2 };
        let s = encode(&m);
        let back: ClientMsg = decode(&s).unwrap();
        assert!(matches!(back, ClientMsg::CreateCharacter { hull: HullClass::Seer, .. }));
    }
}

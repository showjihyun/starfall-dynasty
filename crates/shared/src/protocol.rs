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
    /// Compact world state. `infos` carries static descriptions only when an entity is new or
    /// changed (`full` = complete set, e.g. after entering a system); `dynamic` carries every
    /// live entity's quantized motion each tick. Rebuild with [`SnapshotCache`].
    Snapshot { tick: u64, full: bool, infos: Vec<EntityInfo>, dynamic: Vec<DynState> },
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

/// Rarely-changing part of an entity.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EntityInfo {
    pub id: EntityId,
    pub kind: EntityKind,
    pub name: String,
    pub hull_class: Option<HullClass>,
    pub rank: u8,
    pub livery: u8,
    pub radius: f32,
    pub fleet: Option<FleetId>,
    pub owner: Option<PlayerId>,
    pub alliance: Option<AllianceId>,
    pub class: Option<SizeClass>,
}

/// Per-tick motion, quantized to integers:
/// `[id, px, py, pz (×10), qx, qy, qz, qw (×10000), vx, vy, vz (×10), hull, shield (×1000), flags, mining]`.
pub type DynState = [i32; 15];

const F_BOOST: i32 = 1;
const F_DOCKED: i32 = 2;
const F_WANTED: i32 = 4;

pub fn pack(e: &EntityState) -> (EntityInfo, DynState) {
    let q = |v: f32, k: f32| (v * k).round() as i32;
    let info = EntityInfo { id: e.id, kind: e.kind, name: e.name.clone(), hull_class: e.hull_class, rank: e.rank, livery: e.livery, radius: e.radius, fleet: e.fleet, owner: e.owner, alliance: e.alliance, class: e.class };
    let flags = (e.boosting as i32 * F_BOOST) | (e.docked as i32 * F_DOCKED) | (e.wanted as i32 * F_WANTED);
    let d = [
        e.id as i32,
        q(e.pos[0], 10.0),
        q(e.pos[1], 10.0),
        q(e.pos[2], 10.0),
        q(e.rot[0], 10000.0),
        q(e.rot[1], 10000.0),
        q(e.rot[2], 10000.0),
        q(e.rot[3], 10000.0),
        q(e.vel[0], 10.0),
        q(e.vel[1], 10.0),
        q(e.vel[2], 10.0),
        q(e.hull, 1000.0),
        q(e.shield, 1000.0),
        flags,
        e.mining.map(|m| m as i32).unwrap_or(-1),
    ];
    (info, d)
}

pub fn unpack(i: &EntityInfo, d: &DynState) -> EntityState {
    let f = |v: i32, k: f32| v as f32 / k;
    let rot = [f(d[4], 1e4), f(d[5], 1e4), f(d[6], 1e4), f(d[7], 1e4)];
    let n = (rot.iter().map(|x| x * x).sum::<f32>()).sqrt().max(1e-6);
    EntityState {
        id: i.id,
        kind: i.kind,
        name: i.name.clone(),
        hull_class: i.hull_class,
        rank: i.rank,
        livery: i.livery,
        pos: [f(d[1], 10.0), f(d[2], 10.0), f(d[3], 10.0)],
        rot: [rot[0] / n, rot[1] / n, rot[2] / n, rot[3] / n],
        vel: [f(d[8], 10.0), f(d[9], 10.0), f(d[10], 10.0)],
        hull: f(d[11], 1000.0),
        shield: f(d[12], 1000.0),
        radius: i.radius,
        fleet: i.fleet,
        wanted: d[13] & F_WANTED != 0,
        boosting: d[13] & F_BOOST != 0,
        mining: (d[14] >= 0).then_some(d[14] as u32),
        docked: d[13] & F_DOCKED != 0,
        owner: i.owner,
        alliance: i.alliance,
        class: i.class,
    }
}

/// Receiver side: remembers entity infos and turns compact snapshots back into full states.
#[derive(Default)]
pub struct SnapshotCache {
    pub infos: std::collections::HashMap<EntityId, EntityInfo>,
}

impl SnapshotCache {
    pub fn apply(&mut self, full: bool, infos: Vec<EntityInfo>, dynamic: &[DynState]) -> Vec<EntityState> {
        if full {
            self.infos.clear();
        }
        for i in infos {
            self.infos.insert(i.id, i);
        }
        let live: std::collections::HashSet<EntityId> = dynamic.iter().map(|d| d[0] as EntityId).collect();
        self.infos.retain(|id, _| live.contains(id));
        dynamic.iter().filter_map(|d| self.infos.get(&(d[0] as EntityId)).map(|i| unpack(i, d))).collect()
    }
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
    fn snapshot_pack_roundtrip_and_cache() {
        let e = EntityState { id: 7, kind: EntityKind::Player, name: "A".into(), hull_class: Some(HullClass::Talon), rank: 3, livery: 2, pos: [1234.56, -7.25, 9000.0], rot: [0.0, 0.7071, 0.0, 0.7071], vel: [12.3, 0.0, -4.5], hull: 0.5, shield: 0.25, radius: 11.0, fleet: None, wanted: true, boosting: true, mining: Some(3), docked: false, owner: Some(1), alliance: Some(2), class: Some(SizeClass::Cruiser) };
        let (info, d) = pack(&e);
        let back = unpack(&info, &d);
        assert!((back.pos[0] - e.pos[0]).abs() < 0.06 && back.wanted && back.boosting && back.mining == Some(3) && !back.docked);
        let mut c = SnapshotCache::default();
        assert_eq!(c.apply(true, vec![info], &[d]).len(), 1);
        // Later ticks carry no info: the cache supplies it. Missing from dynamic = gone.
        assert_eq!(c.apply(false, vec![], &[d]).len(), 1);
        assert!(c.apply(false, vec![], &[]).is_empty());
        assert!(c.infos.is_empty());
    }

    #[test]
    fn roundtrip() {
        let m = ClientMsg::CreateCharacter { species: Species::Veyr, profession: Profession::Historian, hull: HullClass::Seer, ship_name: "Aurora".into(), livery: 2 };
        let s = encode(&m);
        let back: ClientMsg = decode(&s).unwrap();
        assert!(matches!(back, ClientMsg::CreateCharacter { hull: HullClass::Seer, .. }));
    }
}

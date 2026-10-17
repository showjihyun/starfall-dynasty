//! Wire protocol over WebSocket: JSON text frames, except snapshots, which travel as compact
//! binary frames ([`encode_snapshot`]). The server is authoritative: clients only send intents
//! and control axes; every state change comes back from the server.

use crate::artifacts::ArtifactId;
use crate::chronicle::{BioEntry, Evidence, EventId, Headline, HistoricalEvent};
use crate::galaxy::{Ore, Species, SystemId};
use crate::missions::{DailyMission, NpcArchetype};
use crate::ships::{HullClass, Materials, Module, Ship, SizeClass, WeaponKind};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const PROTOCOL_VERSION: u32 = 2;
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
    Archaeologist,
    Journalist,
}

impl Profession {
    pub const ALL: [Profession; 7] = [Profession::Explorer, Profession::Miner, Profession::Trader, Profession::CombatPilot, Profession::Historian, Profession::Archaeologist, Profession::Journalist];
    pub fn name(self) -> &'static str {
        match self {
            Profession::Explorer => "Explorer · 탐험가",
            Profession::Miner => "Miner · 광부",
            Profession::Trader => "Trader · 상인",
            Profession::CombatPilot => "Combat Pilot · 전투 조종사",
            Profession::Historian => "Historian · 역사학자",
            Profession::Archaeologist => "Archaeologist · 고고학자",
            Profession::Journalist => "Journalist · 기자",
        }
    }
    pub fn perk(self) -> &'static str {
        match self {
            Profession::Explorer => "스캔 보상 +50%, 스캔 시간 -25%",
            Profession::Miner => "채굴 속도 +40%",
            Profession::Trader => "판매 가격 +20%",
            Profession::CombatPilot => "격파 보상 +30%, 피해 +8%",
            Profession::Historian => "잔해 조사 시 증거 2배, 연구 신뢰도 +",
            Profession::Archaeologist => "유적·고대 게이트에서 이름 있는 유물 발굴, 유물 감정 무료",
            Profession::Journalist => "현장을 취재해 Herald에 기사 송고, 읽은 독자 수만큼 원고료",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
// Missing axes default to zero, so scripts and scenario specs can write just `{"fire": true}`.
#[serde(default)]
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
    /// Sell every mineral specimen held (docked).
    SellSpecimens,
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
    /// Station services for artifacts (docked): reveal the true name and value, sell to collectors,
    /// or donate to the station museum.
    AppraiseArtifact { id: ArtifactId },
    SellArtifact { id: ArtifactId },
    DonateArtifact { id: ArtifactId },
    /// Make a forged copy of evidence you hold, with your own text. It looks like the original.
    ForgeEvidence { evidence: u64, text: String },
    /// Have evidence examined: forgeries are exposed, genuine evidence is marked authentic.
    AuthenticateEvidence { evidence: u64 },
    /// Journalists: file a Herald article about an event, from the scene or holding its evidence.
    FileStory { event: EventId, headline: String, body: String, cite: Vec<u64> },
    /// Make a record you hold public (a leak): everyone can read it, sealed or not.
    LeakEvidence { evidence: u64 },
    /// Contracts: post a job (reward goes into escrow), take one, withdraw an open one, hand over a delivery.
    PostContract { kind: crate::contracts::ContractKind, reward: u64, hours: u32 },
    AcceptContract { id: crate::contracts::ContractId },
    CancelContract { id: crate::contracts::ContractId },
    DeliverContract { id: crate::contracts::ContractId },
    RequestContracts,
    /// Alliance leader: pay the treasury out to the members in equal shares.
    AllianceDividend,
    /// Found a research institute / museum at the station you are docked at (one per captain).
    FoundInstitute { name: String },
    /// Put an artifact you hold on display at an institute in this system (docked).
    DonateToInstitute { artifact: ArtifactId, institute: crate::institutes::InstituteId },
    RequestInstitutes,
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
    /// Mineral specimens held (mineral id → count); they take no cargo space.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub specimens: BTreeMap<crate::minerals::MineralId, u32>,
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
    /// Credits collected as tax in the systems it controls.
    #[serde(default)]
    pub treasury: u64,
}

/// One system's political standing: who controls it and every alliance's influence there.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SystemStanding {
    pub system: SystemId,
    pub controller: Option<(AllianceId, String)>,
    /// (alliance, tag, influence), strongest first.
    pub standings: Vec<(AllianceId, String, f32)>,
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
    /// Every player institution with its exhibits.
    Institutes { list: Vec<crate::institutes::InstituteView> },
    /// Generated systems someone has reached (charted); the rest show as unknown on the map. Sent on
    /// login and to everyone whenever a system is charted. Core systems are always known.
    Charted { systems: Vec<SystemId> },
    /// Alliance influence and control per system (`threshold`: influence needed to take control).
    Influence { systems: Vec<SystemStanding>, threshold: f32 },
    /// The contract board: every open contract plus the ones this captain is party to.
    Contracts { list: Vec<crate::contracts::Contract> },
    /// The artifacts this captain holds (sent on login and whenever they change).
    Artifacts { held: Vec<crate::artifacts::Artifact> },
    /// `tuning`: the server's gameplay numbers, so the client's derived stats (HUD, warp range) match it.
    Welcome {
        player_id: PlayerId,
        token: String,
        profile: Option<Profile>,
        server_unix: u64,
        #[serde(default)]
        tuning: Option<Box<crate::tuning::Tuning>>,
    },
    Error { msg: String },
    Toast { text: String },
    Profile(Profile),
    EnterSystem { system: SystemId, you: EntityId },
    /// Compact world state. `infos` carries static descriptions only when an entity is new or
    /// changed (`full` = complete set, e.g. after entering a system); `dynamic` carries every
    /// live entity's quantized motion each tick. Rebuild with [`SnapshotCache`].
    /// Per-viewer partial update. Entities absent from `dynamic` keep their last state (the
    /// receiver extrapolates them); only `gone` removes. Busy battles send the most relevant
    /// ships every tick and the rest less often. Rebuild with [`SnapshotCache`].
    Snapshot { tick: u64, full: bool, infos: Vec<EntityInfo>, gone: Vec<EntityId>, dynamic: Vec<DynState>, fx: Vec<Fx> },
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

/// Receiver side: remembers entity infos and last motion, turns partial snapshots back into
/// full states. Entities not refreshed this tick are dead-reckoned from their velocity.
#[derive(Default)]
pub struct SnapshotCache {
    pub infos: std::collections::HashMap<EntityId, EntityInfo>,
    dyns: std::collections::HashMap<EntityId, (DynState, u64)>,
}

impl SnapshotCache {
    fn ingest(&mut self, tick: u64, full: bool, infos: Vec<EntityInfo>, gone: &[EntityId], dynamic: &[DynState]) -> Vec<EntityId> {
        if full {
            self.infos.clear();
            self.dyns.clear();
        }
        let changed = infos.iter().map(|i| i.id).collect();
        for i in infos {
            self.infos.insert(i.id, i);
        }
        for id in gone {
            self.infos.remove(id);
            self.dyns.remove(id);
        }
        for d in dynamic {
            self.dyns.insert(d[0] as EntityId, (*d, tick));
        }
        changed
    }

    /// Cheap incremental form for headless clients: updates `out` in place, touching only what
    /// this snapshot carried (no dead reckoning of the rest).
    pub fn apply_into(&mut self, tick: u64, full: bool, infos: Vec<EntityInfo>, gone: &[EntityId], dynamic: &[DynState], out: &mut std::collections::HashMap<EntityId, EntityState>) {
        let changed = self.ingest(tick, full, infos, gone, dynamic);
        if full {
            out.clear();
        }
        for id in gone {
            out.remove(id);
        }
        let ids = dynamic.iter().map(|d| d[0] as EntityId).chain(changed);
        for id in ids {
            if let (Some(i), Some((d, _))) = (self.infos.get(&id), self.dyns.get(&id)) {
                out.insert(id, unpack(i, d));
            }
        }
    }

    pub fn apply(&mut self, tick: u64, full: bool, infos: Vec<EntityInfo>, gone: &[EntityId], dynamic: &[DynState]) -> Vec<EntityState> {
        self.ingest(tick, full, infos, gone, dynamic);
        let dt = 1.0 / TICK_HZ;
        self.dyns
            .iter()
            .filter_map(|(id, (d, at))| {
                let mut st = unpack(self.infos.get(id)?, d);
                let age = tick.saturating_sub(*at).min(40) as f32 * dt;
                if age > 0.0 {
                    for k in 0..3 {
                        st.pos[k] += st.vel[k] * age;
                    }
                }
                Some(st)
            })
            .collect()
    }
}

/// Binary snapshot frame: `[0xA5, tick u32, flags u8, infos (u32 len + JSON), gone (u16 n + u32 ids),
/// dynamic (u16 n + 39-byte records), fx (u16 n + tagged records)]`, little-endian.
pub const SNAPSHOT_MAGIC: u8 = 0xA5;
/// Bytes per entity motion record on the wire.
pub const DYN_WIRE_BYTES: usize = 39;

pub fn encode_snapshot(m: &ServerMsg) -> Option<Vec<u8>> {
    let ServerMsg::Snapshot { tick, full, infos, gone, dynamic, fx } = m else { return None };
    let cap = |n: usize| n.min(u16::MAX as usize);
    let mut b = Vec::with_capacity(16 + gone.len() * 4 + dynamic.len() * DYN_WIRE_BYTES + fx.len() * 14);
    b.push(SNAPSHOT_MAGIC);
    b.extend_from_slice(&(*tick as u32).to_le_bytes());
    b.push(*full as u8);
    let js = if infos.is_empty() { vec![] } else { serde_json::to_vec(infos).expect("infos") };
    b.extend_from_slice(&(js.len() as u32).to_le_bytes());
    b.extend_from_slice(&js);
    b.extend_from_slice(&(cap(gone.len()) as u16).to_le_bytes());
    for id in &gone[..cap(gone.len())] {
        b.extend_from_slice(&id.to_le_bytes());
    }
    b.extend_from_slice(&(cap(dynamic.len()) as u16).to_le_bytes());
    for d in &dynamic[..cap(dynamic.len())] {
        put_dyn(&mut b, d);
    }
    b.extend_from_slice(&(cap(fx.len()) as u16).to_le_bytes());
    for f in &fx[..cap(fx.len())] {
        put_fx(&mut b, f);
    }
    Some(b)
}

pub fn decode_snapshot(b: &[u8]) -> Option<ServerMsg> {
    let mut r = Reader { b, at: 0 };
    if r.u8()? != SNAPSHOT_MAGIC {
        return None;
    }
    let tick = r.u32()? as u64;
    let full = r.u8()? != 0;
    let jl = r.u32()? as usize;
    let js = r.take(jl)?;
    let infos = if jl == 0 { vec![] } else { serde_json::from_slice(js).ok()? };
    let gone = (0..r.u16()?).map(|_| r.u32()).collect::<Option<Vec<_>>>()?;
    let dynamic = (0..r.u16()?).map(|_| get_dyn(&mut r)).collect::<Option<Vec<_>>>()?;
    let fx = (0..r.u16()?).map(|_| get_fx(&mut r)).collect::<Option<Vec<_>>>()?;
    Some(ServerMsg::Snapshot { tick, full, infos, gone, dynamic, fx })
}

fn i16c(v: i32) -> i16 {
    v.clamp(i16::MIN as i32, i16::MAX as i32) as i16
}

fn put_dyn(b: &mut Vec<u8>, d: &DynState) {
    b.extend_from_slice(&(d[0] as u32).to_le_bytes());
    for v in &d[1..4] {
        b.extend_from_slice(&v.to_le_bytes());
    }
    for v in &d[4..8] {
        b.extend_from_slice(&i16c(*v).to_le_bytes());
    }
    // Velocity at 0.5 u/s resolution (d is ×10).
    for v in &d[8..11] {
        b.extend_from_slice(&i16c((*v as f32 / 5.0).round() as i32).to_le_bytes());
    }
    b.extend_from_slice(&(d[11].clamp(0, u16::MAX as i32) as u16).to_le_bytes());
    b.extend_from_slice(&(d[12].clamp(0, u16::MAX as i32) as u16).to_le_bytes());
    b.push(d[13] as u8);
    b.extend_from_slice(&d[14].to_le_bytes());
}

fn get_dyn(r: &mut Reader) -> Option<DynState> {
    let mut d = [0i32; 15];
    d[0] = r.u32()? as i32;
    for v in &mut d[1..4] {
        *v = r.i32()?;
    }
    for v in &mut d[4..8] {
        *v = r.i16()? as i32;
    }
    for v in &mut d[8..11] {
        *v = r.i16()? as i32 * 5;
    }
    d[11] = r.u16()? as i32;
    d[12] = r.u16()? as i32;
    d[13] = r.u8()? as i32;
    d[14] = r.i32()?;
    Some(d)
}

fn weapon_byte(w: WeaponKind) -> u8 {
    match w {
        WeaponKind::Laser => 0,
        WeaponKind::Pulse => 1,
        WeaponKind::MainBeam => 2,
        WeaponKind::Missile => 3,
    }
}

fn put_fx(b: &mut Vec<u8>, f: &Fx) {
    let f32s = |b: &mut Vec<u8>, v: &[f32]| v.iter().for_each(|x| b.extend_from_slice(&x.to_le_bytes()));
    match f {
        Fx::Shot { from, to, weapon, mount, hit } => {
            b.push(0);
            b.extend_from_slice(&from.to_le_bytes());
            b.extend_from_slice(&to.to_le_bytes());
            b.extend_from_slice(&[weapon_byte(*weapon), *mount, *hit as u8]);
        }
        Fx::MissileLaunch { from, missile } => {
            b.push(1);
            b.extend_from_slice(&from.to_le_bytes());
            b.extend_from_slice(&missile.to_le_bytes());
        }
        Fx::Explosion { pos, size } => {
            b.push(2);
            f32s(b, pos);
            f32s(b, &[*size]);
        }
        Fx::ShieldHit { entity, dir } => {
            b.push(3);
            b.extend_from_slice(&entity.to_le_bytes());
            b.extend(dir.iter().map(|v| (v.clamp(-1.0, 1.0) * 127.0).round() as i8 as u8));
        }
        Fx::WarpOut { entity } => {
            b.push(4);
            b.extend_from_slice(&entity.to_le_bytes());
        }
        Fx::WarpIn { entity } => {
            b.push(5);
            b.extend_from_slice(&entity.to_le_bytes());
        }
    }
}

fn get_fx(r: &mut Reader) -> Option<Fx> {
    Some(match r.u8()? {
        0 => {
            let (from, to) = (r.u32()?, r.u32()?);
            let weapon = match r.u8()? {
                0 => WeaponKind::Laser,
                1 => WeaponKind::Pulse,
                2 => WeaponKind::MainBeam,
                _ => WeaponKind::Missile,
            };
            Fx::Shot { from, to, weapon, mount: r.u8()?, hit: r.u8()? != 0 }
        }
        1 => Fx::MissileLaunch { from: r.u32()?, missile: r.u32()? },
        2 => Fx::Explosion { pos: [r.f32()?, r.f32()?, r.f32()?], size: r.f32()? },
        3 => {
            let entity = r.u32()?;
            let mut dir = [0.0; 3];
            for v in &mut dir {
                *v = r.u8()? as i8 as f32 / 127.0;
            }
            Fx::ShieldHit { entity, dir }
        }
        4 => Fx::WarpOut { entity: r.u32()? },
        5 => Fx::WarpIn { entity: r.u32()? },
        _ => return None,
    })
}

struct Reader<'a> {
    b: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        let s = self.b.get(self.at..self.at.checked_add(n)?)?;
        self.at += n;
        Some(s)
    }
    fn u8(&mut self) -> Option<u8> {
        Some(self.take(1)?[0])
    }
    fn u16(&mut self) -> Option<u16> {
        Some(u16::from_le_bytes(self.take(2)?.try_into().ok()?))
    }
    fn i16(&mut self) -> Option<i16> {
        Some(i16::from_le_bytes(self.take(2)?.try_into().ok()?))
    }
    fn u32(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes(self.take(4)?.try_into().ok()?))
    }
    fn i32(&mut self) -> Option<i32> {
        Some(i32::from_le_bytes(self.take(4)?.try_into().ok()?))
    }
    fn f32(&mut self) -> Option<f32> {
        Some(f32::from_le_bytes(self.take(4)?.try_into().ok()?))
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
        assert_eq!(c.apply(1, true, vec![info.clone()], &[], &[d]).len(), 1);
        // Later ticks carry no info: the cache supplies it. Not refreshed = dead-reckoned, not dropped.
        let later = c.apply(3, false, vec![], &[], &[]);
        assert_eq!(later.len(), 1);
        assert!((later[0].pos[0] - (e.pos[0] + e.vel[0] * 0.1)).abs() < 0.1);
        assert!(c.apply(4, false, vec![], &[7], &[]).is_empty());
        assert!(c.infos.is_empty());

        // Binary wire round trip.
        let fx = vec![
            Fx::Shot { from: 7, to: 9, weapon: WeaponKind::MainBeam, mount: 255, hit: true },
            Fx::Explosion { pos: [1.0, 2.0, 3.0], size: 2.5 },
            Fx::ShieldHit { entity: 7, dir: [0.0, -1.0, 0.0] },
            Fx::WarpIn { entity: 3 },
        ];
        let m = ServerMsg::Snapshot { tick: 42, full: true, infos: vec![info], gone: vec![5, 6], dynamic: vec![d], fx: fx.clone() };
        let bin = encode_snapshot(&m).unwrap();
        let Some(ServerMsg::Snapshot { tick, full, infos, gone, dynamic, fx: fx2 }) = decode_snapshot(&bin) else { panic!("decode") };
        assert!(tick == 42 && full && infos.len() == 1 && gone == vec![5, 6] && fx2 == fx);
        let back = unpack(&infos[0], &dynamic[0]);
        assert!((back.pos[2] - e.pos[2]).abs() < 0.06 && (back.vel[0] - e.vel[0]).abs() < 0.3 && back.mining == Some(3));
        assert!(decode_snapshot(&bin[..bin.len() - 3]).is_none());
    }

    #[test]
    fn roundtrip() {
        let m = ClientMsg::CreateCharacter { species: Species::Veyr, profession: Profession::Historian, hull: HullClass::Seer, ship_name: "Aurora".into(), livery: 2 };
        let s = encode(&m);
        let back: ClientMsg = decode(&s).unwrap();
        assert!(matches!(back, ClientMsg::CreateCharacter { hull: HullClass::Seer, .. }));
    }
}

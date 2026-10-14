//! Authoritative game state. Pure and synchronous: the network layer feeds it
//! messages and drains `out`; tests drive it directly.

mod actions;
mod sim;

use crate::store::Store;
use history::HistoryEngine;
use shared::galaxy::{self, Layout, SystemId};
use shared::glam::{Quat, Vec3};
use shared::missions::{self, NpcArchetype, Progress};
use shared::protocol::*;
use shared::ships::Stats;
use std::collections::{BTreeMap, HashMap, HashSet};

pub type ConnId = u64;

pub const RESPAWN_FEE_PER_RANK: u64 = 150;
pub const WARP_CHARGE: f32 = 4.0;
pub const COMBAT_LOCK: f64 = 5.0;
pub const ENGAGEMENT_TIMEOUT: f64 = 20.0;
pub const WRECK_TTL_SECS: u64 = 7 * 86_400;

#[derive(Clone, Debug)]
pub struct Ai {
    pub archetype: NpcArchetype,
    pub level: u8,
    pub home: Vec3,
    pub target: Option<EntityId>,
    /// Scenario waves hunt their owner's fleet regardless of distance.
    pub scenario_owner: Option<PlayerId>,
    pub think_cd: f32,
    pub orbit_sign: f32,
}

#[derive(Clone, Debug)]
pub struct Missile {
    pub owner: EntityId,
    pub target: EntityId,
    pub damage: f32,
    pub life: f32,
}

#[derive(Clone, Debug)]
pub struct Ent {
    pub id: EntityId,
    pub kind: EntityKind,
    pub name: String,
    pub owner: Option<PlayerId>,
    pub hull_class: Option<shared::ships::HullClass>,
    pub rank: u8,
    pub livery: u8,
    pub pos: Vec3,
    pub rot: Quat,
    pub vel: Vec3,
    pub stats: Stats,
    pub hull: f32,
    pub shield: f32,
    pub last_hit: f64,
    pub fire_cd: f32,
    pub missile_cd: f32,
    pub controls: Controls,
    pub target: Option<EntityId>,
    pub fleet: Option<FleetId>,
    pub wanted: bool,
    pub mining: Option<u32>,
    pub mine_acc: f32,
    pub docked: bool,
    pub ai: Option<Ai>,
    pub missile: Option<Missile>,
    /// (attacker entity, attacker player, time) — for kill credit, assists, self-defence.
    pub attackers: Vec<(EntityId, Option<PlayerId>, f64)>,
    pub dead: bool,
}

impl Ent {
    pub fn forward(&self) -> Vec3 {
        self.rot * Vec3::NEG_Z
    }

    pub fn state(&self) -> EntityState {
        EntityState {
            id: self.id,
            kind: self.kind,
            name: self.name.clone(),
            hull_class: self.hull_class,
            rank: self.rank,
            livery: self.livery,
            pos: self.pos.to_array(),
            rot: self.rot.to_array(),
            vel: self.vel.to_array(),
            hull: (self.hull / self.stats.max_hull.max(1.0)).clamp(0.0, 1.0),
            shield: (self.shield / self.stats.max_shield.max(1.0)).clamp(0.0, 1.0),
            radius: self.stats.radius,
            fleet: self.fleet,
            wanted: self.wanted,
            boosting: self.controls.boost,
            mining: self.mining,
            docked: self.docked,
            owner: self.owner,
        }
    }

    pub fn is_ship(&self) -> bool {
        matches!(self.kind, EntityKind::Player | EntityKind::Npc(_))
    }
}

pub struct Wreck {
    pub entity: EntityId,
    pub stored: usize,
}

pub struct Engagement {
    pub started: f64,
    pub last_fire: f64,
    /// side key → (display name, members, losses)
    pub sides: BTreeMap<String, (String, HashSet<String>, u32)>,
}

pub struct Sys {
    pub id: SystemId,
    pub layout: Layout,
    pub ents: BTreeMap<EntityId, Ent>,
    pub pirate_cd: f32,
    pub engagement: Option<Engagement>,
    pub wrecks: HashMap<EntityId, usize>,
}

#[derive(Default)]
pub struct Session {
    pub player: Option<PlayerId>,
    pub entity: Option<EntityId>,
    pub system: SystemId,
    pub warp: Option<(SystemId, f32)>,
    pub scan: Option<(u32, f32)>,
    pub invite: Option<FleetId>,
    pub wave: Vec<EntityId>,
    pub wave_system: Option<SystemId>,
    pub dead: bool,
}

pub struct Fleet {
    pub leader: PlayerId,
    pub members: Vec<PlayerId>,
}

pub struct Game {
    pub store: Store,
    pub history: HistoryEngine,
    pub systems: BTreeMap<SystemId, Sys>,
    pub sessions: HashMap<ConnId, Session>,
    pub conn_of: HashMap<PlayerId, ConnId>,
    pub fleets: HashMap<FleetId, Fleet>,
    pub out: Vec<(ConnId, ServerMsg)>,
    pub now: f64,
    pub unix: u64,
    pub tick: u64,
    next_entity: EntityId,
    next_fleet: FleetId,
    pub kicked: Vec<ConnId>,
    pub pending_fx: BTreeMap<SystemId, Vec<Fx>>,
}

impl Game {
    pub fn new(store: Store, history: HistoryEngine, unix: u64) -> Self {
        let mut g = Game {
            store,
            history,
            systems: BTreeMap::new(),
            sessions: HashMap::new(),
            conn_of: HashMap::new(),
            fleets: HashMap::new(),
            out: Vec::new(),
            now: 0.0,
            unix,
            tick: 0,
            next_entity: 1,
            next_fleet: 1,
            kicked: Vec::new(),
            pending_fx: BTreeMap::new(),
        };
        for def in galaxy::SYSTEMS {
            g.systems.insert(def.id, Sys { id: def.id, layout: galaxy::system_layout(def.id), ents: BTreeMap::new(), pirate_cd: 1.0, engagement: None, wrecks: HashMap::new() });
        }
        // Restore persisted wrecks (expired ones are dropped).
        let wrecks: Vec<(usize, SystemId, Vec3, String)> = g
            .store
            .data
            .wrecks
            .iter()
            .enumerate()
            .filter(|(_, w)| unix.saturating_sub(w.created_unix) < WRECK_TTL_SECS)
            .map(|(i, w)| (i, w.system, Vec3::from(w.pos), format!("{}의 잔해 「{}」", w.victim, w.ship)))
            .collect();
        for (i, sys, pos, name) in wrecks {
            g.spawn_wreck_entity(sys, pos, name, i);
        }
        g
    }

    pub fn alloc_entity(&mut self) -> EntityId {
        let id = self.next_entity;
        self.next_entity += 1;
        id
    }

    pub fn send(&mut self, conn: ConnId, msg: ServerMsg) {
        self.out.push((conn, msg));
    }

    pub fn send_player(&mut self, pid: PlayerId, msg: ServerMsg) {
        if let Some(c) = self.conn_of.get(&pid).copied() {
            self.send(c, msg);
        }
    }

    pub fn toast(&mut self, pid: PlayerId, text: impl Into<String>) {
        self.send_player(pid, ServerMsg::Toast { text: text.into() });
    }

    pub fn error(&mut self, conn: ConnId, msg: impl Into<String>) {
        self.send(conn, ServerMsg::Error { msg: msg.into() });
    }

    pub fn push_profile(&mut self, pid: PlayerId) {
        if let Some(p) = self.store.profile(pid).cloned() {
            self.send_player(pid, ServerMsg::Profile(p));
        }
    }

    pub fn player_entity(&self, pid: PlayerId) -> Option<(SystemId, EntityId)> {
        let conn = self.conn_of.get(&pid)?;
        let s = self.sessions.get(conn)?;
        Some((s.system, s.entity?))
    }

    pub fn ent(&self, sys: SystemId, id: EntityId) -> Option<&Ent> {
        self.systems.get(&sys)?.ents.get(&id)
    }

    pub fn ent_mut(&mut self, sys: SystemId, id: EntityId) -> Option<&mut Ent> {
        self.systems.get_mut(&sys)?.ents.get_mut(&id)
    }

    pub fn fleet_of(&self, pid: PlayerId) -> Option<FleetId> {
        self.fleets.iter().find(|(_, f)| f.members.contains(&pid)).map(|(id, _)| *id)
    }

    /// Players who share credit with `pid`: itself plus fleet mates in the same system.
    pub fn credit_group(&self, pid: PlayerId, sys: SystemId) -> Vec<PlayerId> {
        let mut v = vec![pid];
        if let Some(f) = self.fleet_of(pid).and_then(|f| self.fleets.get(&f)) {
            for m in &f.members {
                if *m != pid && self.player_entity(*m).map(|(s, _)| s) == Some(sys) {
                    v.push(*m);
                }
            }
        }
        v
    }

    pub fn progress(&mut self, pid: PlayerId, p: Progress) {
        let day = missions::day_index(self.unix);
        self.ensure_dailies(pid, day);
        let mut done = Vec::new();
        if let Some(prof) = self.store.profile_mut(pid) {
            for m in prof.dailies.iter_mut() {
                let before = m.complete();
                let add = p.amount_for(&m.objective);
                if add > 0 {
                    m.progress = (m.progress + add).min(m.objective.target());
                    if !before && m.complete() {
                        done.push(m.objective.describe());
                    }
                }
            }
        }
        for d in done {
            self.toast(pid, format!("일일 미션 완료: {d} — 보상을 수령하세요"));
        }
        self.push_profile(pid);
    }

    pub fn ensure_dailies(&mut self, pid: PlayerId, day: u32) {
        if let Some(p) = self.store.profile_mut(pid)
            && (p.daily_day != day || p.dailies.is_empty())
        {
            p.daily_day = day;
            p.dailies = missions::daily_missions(day, pid);
            p.scanned.clear();
        }
    }

    pub fn handle(&mut self, conn: ConnId, msg: ClientMsg) {
        self.sessions.entry(conn).or_default();
        match msg {
            ClientMsg::Hello { version, name, token } => self.hello(conn, version, name, token),
            ClientMsg::Ping { t } => self.send(conn, ServerMsg::Pong { t }),
            ClientMsg::CreateCharacter { species, profession, hull, ship_name, livery } => self.create_character(conn, species, profession, hull, ship_name, livery),
            other => {
                let Some(pid) = self.sessions[&conn].player else {
                    return self.error(conn, "먼저 로그인하세요");
                };
                if self.store.profile(pid).is_none() {
                    return self.error(conn, "함장을 먼저 생성하세요");
                }
                self.handle_player(conn, pid, other);
            }
        }
    }

    pub fn disconnect(&mut self, conn: ConnId) {
        if let Some(s) = self.sessions.remove(&conn) {
            if let Some(pid) = s.player {
                if self.conn_of.get(&pid) == Some(&conn) {
                    self.conn_of.remove(&pid);
                }
                if let Some(e) = s.entity {
                    if let Some(sys) = self.systems.get_mut(&s.system) {
                        sys.ents.remove(&e);
                    }
                }
                for w in s.wave {
                    if let Some(sys) = self.systems.get_mut(&s.wave_system.unwrap_or(s.system)) {
                        sys.ents.remove(&w);
                    }
                }
                self.broadcast_fleet(pid);
            }
        }
        self.store.dirty = true;
    }

    fn hello(&mut self, conn: ConnId, version: u32, name: String, token: Option<String>) {
        if version != PROTOCOL_VERSION {
            return self.error(conn, format!("프로토콜 버전 불일치 (서버 {PROTOCOL_VERSION})"));
        }
        let name = name.trim().to_string();
        if name.chars().count() < 2 || name.chars().count() > 20 || name.contains(['/', '"']) {
            return self.error(conn, "함장 이름은 2~20자여야 합니다");
        }
        let pid = match self.store.find_by_name(&name) {
            Some(pid) => {
                let acc = self.store.account(pid).unwrap();
                if !token.as_deref().is_some_and(|t| crate::store::token_eq(t, &acc.token)) {
                    return self.error(conn, "이미 사용 중인 이름입니다 (토큰 불일치)");
                }
                pid
            }
            None => {
                let t = crate::store::new_token();
                self.store.create_account(&name, t)
            }
        };
        // One live session per player: kick the old one.
        if let Some(old) = self.conn_of.get(&pid).copied()
            && old != conn
        {
            self.send(old, ServerMsg::Error { msg: "다른 곳에서 접속하여 연결을 종료합니다".into() });
            self.disconnect(old);
            self.kicked.push(old);
        }
        self.conn_of.insert(pid, conn);
        self.sessions.get_mut(&conn).unwrap().player = Some(pid);
        let day = missions::day_index(self.unix);
        self.ensure_dailies(pid, day);
        let acc = self.store.account(pid).unwrap().clone();
        self.send(conn, ServerMsg::Welcome { player_id: pid, token: acc.token.clone(), profile: acc.profile.clone(), server_unix: self.unix });
        if let Some(p) = acc.profile {
            self.spawn_player(pid, p.last_system, true);
            self.broadcast_fleet(pid);
        }
    }

    fn create_character(&mut self, conn: ConnId, species: galaxy::Species, profession: Profession, hull: shared::ships::HullClass, ship_name: String, livery: u8) {
        let Some(pid) = self.sessions[&conn].player else {
            return self.error(conn, "먼저 로그인하세요");
        };
        if self.store.profile(pid).is_some() {
            return self.error(conn, "함선은 단 한 척만 보유할 수 있습니다. 이미 함선이 있습니다.");
        }
        let ship_name = ship_name.trim().to_string();
        if ship_name.chars().count() < 2 || ship_name.chars().count() > 24 {
            return self.error(conn, "함선 이름은 2~24자여야 합니다");
        }
        let name = self.store.account(pid).unwrap().name.clone();
        let home = species.home_system();
        let day = missions::day_index(self.unix);
        let profile = Profile {
            player_id: pid,
            name: name.clone(),
            species,
            profession,
            ship: shared::ships::Ship::new(ship_name.clone(), hull, livery % 8),
            wallet: shared::ships::Materials { credits: 1500, alloy: 10, crystal: 5, ..Default::default() },
            cargo: BTreeMap::new(),
            home,
            dailies: missions::daily_missions(day, pid),
            daily_day: day,
            scenarios_completed: 0,
            scenario: None,
            kills: 0,
            deaths: 0,
            reputation: 0,
            bounty: 0,
            archive: vec![],
            last_system: home,
            scanned: vec![],
        };
        self.store.set_profile(pid, profile.clone());
        self.history.record(self.unix, history::DomainEvent::PlayerFounded { player: name, species: species.name().into(), ship: ship_name, hull: hull.name().into() });
        self.send(conn, ServerMsg::Profile(profile));
        self.spawn_player(pid, home, true);
    }

    pub fn spawn_player(&mut self, pid: PlayerId, system: SystemId, docked: bool) {
        let Some(conn) = self.conn_of.get(&pid).copied() else { return };
        let prof = self.store.profile(pid).unwrap().clone();
        // Remove an old entity if any.
        if let Some(s) = self.sessions.get(&conn)
            && let Some(e) = s.entity
            && let Some(sys) = self.systems.get_mut(&s.system)
        {
            sys.ents.remove(&e);
        }
        let id = self.alloc_entity();
        let layout = &self.systems[&system].layout;
        let station = layout.station;
        let pos = if docked { station + Vec3::new(0.0, -40.0, 60.0) } else { layout.jump_point };
        let stats = prof.ship.stats();
        let fleet = self.fleet_of(pid);
        let ent = Ent {
            id,
            kind: EntityKind::Player,
            name: format!("{} 「{}」", prof.name, prof.ship.name),
            owner: Some(pid),
            hull_class: Some(prof.ship.hull),
            rank: prof.ship.rank(),
            livery: prof.ship.livery,
            pos,
            rot: Quat::from_rotation_y(std::f32::consts::PI),
            vel: Vec3::ZERO,
            stats,
            hull: stats.max_hull,
            shield: stats.max_shield,
            last_hit: -100.0,
            fire_cd: 0.0,
            missile_cd: 0.0,
            controls: Controls::default(),
            target: None,
            fleet,
            wanted: prof.bounty > 0,
            mining: None,
            mine_acc: 0.0,
            docked,
            ai: None,
            missile: None,
            attackers: vec![],
            dead: false,
        };
        self.systems.get_mut(&system).unwrap().ents.insert(id, ent);
        {
            let s = self.sessions.get_mut(&conn).unwrap();
            s.entity = Some(id);
            s.system = system;
            s.warp = None;
            s.scan = None;
            s.dead = false;
        }
        if let Some(p) = self.store.profile_mut(pid) {
            p.last_system = system;
        }
        self.send(conn, ServerMsg::EnterSystem { system, you: id });
        let scanned: Vec<u32> = prof.scanned.iter().filter(|(s, _)| *s == system).map(|(_, p)| *p).collect();
        self.send(conn, ServerMsg::Scanned { pois: scanned });
        self.progress(pid, Progress::Visited(system));
        self.scenario_on_enter(pid);
    }

    pub fn spawn_wreck_entity(&mut self, sys: SystemId, pos: Vec3, name: String, stored: usize) -> EntityId {
        let id = self.alloc_entity();
        let mut s = shared::ships::Ship::new(String::new(), shared::ships::HullClass::Wren, 0).stats();
        s.radius = 14.0;
        let ent = Ent {
            id,
            kind: EntityKind::Wreck,
            name,
            owner: None,
            hull_class: None,
            rank: 0,
            livery: 0,
            pos,
            rot: Quat::from_rotation_z((id as f32) * 0.7),
            vel: Vec3::ZERO,
            stats: s,
            hull: 0.0,
            shield: 0.0,
            last_hit: 0.0,
            fire_cd: 0.0,
            missile_cd: 0.0,
            controls: Controls::default(),
            target: None,
            fleet: None,
            wanted: false,
            mining: None,
            mine_acc: 0.0,
            docked: false,
            ai: None,
            missile: None,
            attackers: vec![],
            dead: false,
        };
        let sysm = self.systems.get_mut(&sys).unwrap();
        sysm.ents.insert(id, ent);
        sysm.wrecks.insert(id, stored);
        id
    }

    pub fn spawn_npc(&mut self, sys: SystemId, arch: NpcArchetype, level: u8, pos: Vec3, scenario_owner: Option<PlayerId>) -> EntityId {
        let id = self.alloc_entity();
        let (hull, shield, damage, interval, speed, radius) = arch.stats(level);
        let stats = Stats {
            max_hull: hull,
            max_shield: shield,
            shield_regen: shield * 0.05,
            speed,
            accel: speed * 0.8,
            turn: if radius > 25.0 { 0.7 } else { 1.6 },
            warp_range: 0.0,
            damage,
            fire_interval: interval,
            weapon_range: 520.0 + radius * 4.0,
            missile_damage: 0.0,
            missile_cooldown: 99.0,
            scan_range: 0.0,
            scan_time: 0.0,
            mine_rate: 0.0,
            cargo: 0.0,
            radius,
        };
        let r = (id as f32 * 1.618).fract();
        let ent = Ent {
            id,
            kind: EntityKind::Npc(arch),
            name: format!("{} Lv{}", arch.name(), level),
            owner: None,
            hull_class: None,
            rank: level,
            livery: 7,
            pos,
            rot: Quat::from_rotation_y(r * std::f32::consts::TAU),
            vel: Vec3::ZERO,
            stats,
            hull,
            shield,
            last_hit: -100.0,
            fire_cd: 1.0 + r,
            missile_cd: 0.0,
            controls: Controls::default(),
            target: None,
            fleet: None,
            wanted: false,
            mining: None,
            mine_acc: 0.0,
            docked: false,
            ai: Some(Ai { archetype: arch, level, home: pos, target: None, scenario_owner, think_cd: 0.0, orbit_sign: if r > 0.5 { 1.0 } else { -1.0 } }),
            missile: None,
            attackers: vec![],
            dead: false,
        };
        self.systems.get_mut(&sys).unwrap().ents.insert(id, ent);
        id
    }

    pub fn broadcast_fleet(&mut self, pid: PlayerId) {
        let Some(fid) = self.fleet_of(pid) else {
            self.send_player(pid, ServerMsg::Fleet { id: None, members: vec![] });
            return;
        };
        let members_ids = self.fleets[&fid].members.clone();
        let members: Vec<FleetMember> = members_ids
            .iter()
            .filter_map(|m| {
                let p = self.store.profile(*m)?;
                Some(FleetMember { name: p.name.clone(), hull: p.ship.hull, system: self.player_entity(*m).map(|x| x.0).unwrap_or(p.last_system), online: self.conn_of.contains_key(m) })
            })
            .collect();
        for m in &members_ids {
            self.send_player(*m, ServerMsg::Fleet { id: Some(fid), members: members.clone() });
            if let Some((s, e)) = self.player_entity(*m)
                && let Some(ent) = self.ent_mut(s, e)
            {
                ent.fleet = Some(fid);
            }
        }
    }

    pub fn new_fleet(&mut self, leader: PlayerId) -> FleetId {
        let id = self.next_fleet;
        self.next_fleet += 1;
        self.fleets.insert(id, Fleet { leader, members: vec![leader] });
        id
    }

    pub fn persist_tick(&mut self) {
        if self.store.dirty
            && let Err(e) = self.store.save()
        {
            eprintln!("store save failed: {e}");
        }
    }
}

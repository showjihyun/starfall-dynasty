//! Client-side mirror of server state, fed by network messages. Rendering reads from here.

use crate::net::{Net, NetEvent};
use bevy::prelude::*;
use shared::chronicle::{BioEntry, Evidence, Headline, HistoricalEvent};
use shared::galaxy::SystemId;
use shared::protocol::*;
use std::collections::{HashMap, HashSet, VecDeque};

#[derive(States, Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Screen {
    #[default]
    Connecting,
    Login,
    Create,
    Playing,
}

pub struct Snap {
    pub at: f64,
    pub ents: HashMap<EntityId, EntityState>,
}

#[derive(Default)]
pub struct UiFlags {
    pub map: bool,
    pub station: bool,
    pub missions: bool,
    pub chronicle: bool,
    pub fleet: bool,
    pub help: bool,
    pub chat_input: String,
    pub invite_name: String,
    pub research_thesis: String,
    pub research_event: Option<u64>,
    pub bio_query: String,
}

#[derive(Resource, Default)]
pub struct Session {
    pub name: String,
    pub token: Option<String>,
    pub player_id: Option<PlayerId>,
    pub profile: Option<Profile>,
    pub system: SystemId,
    pub system_epoch: u32,
    pub me: Option<EntityId>,
    pub snaps: VecDeque<Snap>,
    pub toasts: VecDeque<(String, f64)>,
    pub last_error: Option<(String, f64)>,
    pub warp: Option<(SystemId, f64, f32)>,
    pub scan: Option<(u32, f32)>,
    pub scanned: HashSet<u32>,
    pub dead: Option<String>,
    pub chronicle: Option<(Vec<HistoricalEvent>, Vec<Evidence>, Vec<Headline>)>,
    pub biography: Option<(String, Vec<BioEntry>, Vec<BioEntry>)>,
    pub fleet: Option<FleetId>,
    pub fleet_members: Vec<FleetMember>,
    pub invite: Option<(String, FleetId)>,
    pub engagement: Option<Vec<String>>,
    pub chat: VecDeque<(String, String)>,
    pub target: Option<EntityId>,
    pub fx: Vec<Fx>,
    pub ui: UiFlags,
    pub disconnected: Option<String>,
    pub ping_ms: f32,
    pub server_url: String,
}

impl Session {
    pub fn latest(&self) -> Option<&Snap> {
        self.snaps.back()
    }

    pub fn my_state(&self) -> Option<&EntityState> {
        self.me.and_then(|id| self.latest()?.ents.get(&id))
    }

    pub fn toast(&mut self, text: impl Into<String>, now: f64) {
        self.toasts.push_back((text.into(), now));
        while self.toasts.len() > 6 {
            self.toasts.pop_front();
        }
    }

    /// Interpolated entity state at render time `t` (seconds, same clock as `Snap::at`).
    pub fn sample(&self, id: EntityId, t: f64) -> Option<(Vec3, Quat, &EntityState)> {
        let n = self.snaps.len();
        if n == 0 {
            return None;
        }
        let last = &self.snaps[n - 1];
        let cur = last.ents.get(&id)?;
        // Own ship: extrapolate from the newest snapshot for responsiveness.
        if Some(id) == self.me || n < 2 {
            let dt = ((t + INTERP_DELAY - last.at).clamp(0.0, 0.12)) as f32;
            return Some((Vec3::from(cur.pos) + Vec3::from(cur.vel) * dt, Quat::from_array(cur.rot), cur));
        }
        for i in (0..n - 1).rev() {
            let (a, b) = (&self.snaps[i], &self.snaps[i + 1]);
            if a.at <= t && t <= b.at {
                let (Some(ea), Some(eb)) = (a.ents.get(&id), b.ents.get(&id)) else { break };
                let k = ((t - a.at) / (b.at - a.at).max(1e-4)) as f32;
                let p = Vec3::from(ea.pos).lerp(Vec3::from(eb.pos), k);
                let r = Quat::from_array(ea.rot).slerp(Quat::from_array(eb.rot), k);
                return Some((p, r, eb));
            }
        }
        Some((Vec3::from(cur.pos), Quat::from_array(cur.rot), cur))
    }
}

pub const INTERP_DELAY: f64 = 0.1;

#[derive(Resource, Clone)]
pub struct Args {
    pub server: String,
    pub name: Option<String>,
    pub auto: bool,
    pub hull: Option<shared::ships::HullClass>,
    pub shot: Option<String>,
    pub shot_at: f32,
    pub showroom: bool,
    pub script: Vec<String>,
}

pub fn token_path(name: &str) -> std::path::PathBuf {
    let base = std::env::var("APPDATA").map(std::path::PathBuf::from).unwrap_or_else(|_| std::env::temp_dir());
    base.join("StarfallDynasty").join(format!("token_{}.txt", name.to_lowercase()))
}

pub fn load_token(name: &str) -> Option<String> {
    std::fs::read_to_string(token_path(name)).ok().map(|s| s.trim().to_string())
}

pub fn save_token(name: &str, token: &str) {
    let p = token_path(name);
    if let Some(d) = p.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    // The token is a login credential: owner-only on Unix (on Windows %APPDATA% is already per-user).
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    if let Ok(mut f) = opts.open(p) {
        use std::io::Write;
        let _ = f.write_all(token.as_bytes());
    }
}

pub fn pump_network(net: Option<Res<Net>>, mut s: ResMut<Session>, time: Res<Time<Real>>, mut next: ResMut<NextState<Screen>>, screen: Res<State<Screen>>) {
    let Some(net) = net else { return };
    let now = time.elapsed_secs_f64();
    while let Ok(ev) = net.rx.try_recv() {
        match ev {
            NetEvent::Connected => {
                if *screen.get() == Screen::Connecting {
                    next.set(Screen::Login);
                }
            }
            NetEvent::Disconnected(r) => s.disconnected = Some(r),
            NetEvent::Msg(m) => apply(&mut s, *m, now, &mut next),
        }
    }
}

fn apply(s: &mut Session, m: ServerMsg, now: f64, next: &mut NextState<Screen>) {
    match m {
        ServerMsg::Welcome { player_id, token, profile, .. } => {
            s.player_id = Some(player_id);
            save_token(&s.name, &token);
            s.token = Some(token);
            let has = profile.is_some();
            s.profile = profile;
            next.set(if has { Screen::Playing } else { Screen::Create });
        }
        ServerMsg::Error { msg } => {
            s.last_error = Some((msg.clone(), now));
            s.toast(format!("⚠ {msg}"), now);
        }
        ServerMsg::Toast { text } => s.toast(text, now),
        ServerMsg::Profile(p) => {
            s.profile = Some(p);
            next.set(Screen::Playing);
        }
        ServerMsg::EnterSystem { system, you } => {
            s.system = system;
            s.me = Some(you);
            s.system_epoch += 1;
            s.snaps.clear();
            s.dead = None;
            s.target = None;
            s.warp = None;
            s.scan = None;
            next.set(Screen::Playing);
        }
        ServerMsg::Snapshot { entities, .. } => {
            s.snaps.push_back(Snap { at: now, ents: entities.into_iter().map(|e| (e.id, e)).collect() });
            while s.snaps.len() > 12 {
                s.snaps.pop_front();
            }
        }
        ServerMsg::Fx(f) => s.fx.extend(f),
        ServerMsg::WarpCharging { system, seconds } => {
            s.warp = if seconds > 0.0 { Some((system, now + seconds as f64, seconds)) } else { None };
        }
        ServerMsg::ScanProgress { poi, frac } => {
            s.scan = if (0.0..1.0).contains(&frac) { Some((poi, frac)) } else { None };
        }
        ServerMsg::Scanned { pois } => s.scanned = pois.into_iter().collect(),
        ServerMsg::Died { killer, .. } => {
            s.dead = Some(killer);
            s.me = None;
        }
        ServerMsg::Chronicle { events, evidence, herald } => s.chronicle = Some((events, evidence, herald)),
        ServerMsg::Biography { name, character, ship } => s.biography = Some((name, character, ship)),
        ServerMsg::FleetInvite { from, fleet } => {
            s.toast(format!("{from}의 함대 초대 — 함대 창(K)에서 수락"), now);
            s.invite = Some((from, fleet));
        }
        ServerMsg::Fleet { id, members } => {
            s.fleet = id;
            s.fleet_members = members;
        }
        ServerMsg::Engagement { active, sides } => s.engagement = if active { Some(sides) } else { None },
        ServerMsg::Chat { from, text } => {
            s.chat.push_back((from, text));
            while s.chat.len() > 50 {
                s.chat.pop_front();
            }
        }
        ServerMsg::Pong { t } => s.ping_ms = ((now - t) * 1000.0) as f32,
    }
}

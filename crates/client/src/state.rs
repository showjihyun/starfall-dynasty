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

pub use view::interp::{INTERP_DELAY, Snap};

#[derive(Default)]
pub struct UiFlags {
    pub map: bool,
    pub station: bool,
    pub missions: bool,
    pub contracts: bool,
    pub chronicle: bool,
    pub fleet: bool,
    pub help: bool,
    pub alliance: bool,
    pub alliance_name: String,
    pub alliance_tag: String,
    pub chat_input: String,
    pub invite_name: String,
    /// Draft research thesis per event (each event keeps its own text).
    pub research_drafts: HashMap<u64, String>,
    /// Draft forged text per evidence id (only while the forge field is open).
    pub forge_drafts: HashMap<u64, String>,
    /// Journalists: draft (headline, body) per event, and events already filed this session.
    pub story_drafts: HashMap<u64, (String, String)>,
    pub stories_filed: HashSet<u64>,
    /// The contract board's "new contract" form.
    pub contract_form: view::panels::ContractForm,
    /// Name typed for a new institute.
    pub institute_name: String,
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
    pub alliances: Vec<AllianceInfo>,
    pub my_alliance: Option<AllianceId>,
    pub alliance_invite: Option<(String, AllianceId, String)>,
    pub invite: Option<(String, FleetId)>,
    pub engagement: Option<Vec<String>>,
    pub chat: VecDeque<(String, String)>,
    pub target: Option<EntityId>,
    pub fx: Vec<Fx>,
    pub ui: UiFlags,
    pub disconnected: Option<String>,
    pub ping_ms: f32,
    pub server_url: String,
    /// Last time shots involved our ship (drives battle music).
    pub last_combat: f64,
    pub snap_cache: SnapshotCache,
    /// Smoothed frames per second for the HUD.
    pub fps: f32,
    /// HUD gauges: shield/hull trailing behind the real values, and the hull-damage flash.
    pub gauge: view::gauge::GaugeTrail,
    /// Star map (M) view state.
    pub map: view::mapview::MapView,
    /// Artifacts this captain holds (server-sent).
    pub artifacts: Vec<shared::artifacts::Artifact>,
    /// The contract board (server-sent).
    pub contracts: Vec<shared::contracts::Contract>,
    /// Player institutions (server-sent).
    pub institutes: Vec<shared::institutes::InstituteView>,
    /// Alliance influence and control per system (server-sent), and the control threshold.
    pub influence: Vec<SystemStanding>,
    pub influence_threshold: f32,
    /// Generated systems someone has charted (server-sent); others show as unknown on the map.
    pub charted: std::collections::BTreeSet<SystemId>,
    /// A cutscene the UI asked for; `cinematic::request` plays it, then sends `then`.
    pub cine_request: Option<CineRequest>,
}

pub struct CineRequest {
    pub name: String,
    pub then: Option<ClientMsg>,
}

impl Session {
    /// Tag of the alliance controlling `sys`, if any.
    pub fn controller_tag(&self, sys: SystemId) -> Option<&str> {
        self.influence.iter().find(|s| s.system == sys).and_then(|s| s.controller.as_ref()).map(|c| c.1.as_str())
    }

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
        view::interp::sample(&self.snaps, self.me, id, t)
    }
}

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
    /// Showroom: orbit a point of interest (`gate`, `ruin`, `station`).
    pub look: Option<String>,
    /// `--replay DIR`: play a recorded session instead of connecting.
    pub replay: Option<String>,
    /// `--no-create`: with `--auto`, log in but stop at character creation (QA screenshots).
    pub no_create: bool,
    /// `--token T`: log in with this token instead of the saved one (QA: look at a bot's captain).
    pub token: Option<String>,
}

/// Per-user config dir only (never a shared temp dir); `None` means the token is not persisted.
pub fn token_path(name: &str) -> Option<std::path::PathBuf> {
    user_file("token", name)
}

/// `<config>/StarfallDynasty/<kind>_<name>.txt`, one per captain.
fn user_file(kind: &str, name: &str) -> Option<std::path::PathBuf> {
    let base = std::env::var_os("APPDATA")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("XDG_CONFIG_HOME").map(std::path::PathBuf::from))
        .or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".config")))?;
    // The name is user input: keep it to letters/digits so it can't escape the directory.
    let safe: String = name.to_lowercase().chars().filter(|c| c.is_alphanumeric()).collect();
    if safe.is_empty() {
        return None;
    }
    Some(base.join("StarfallDynasty").join(format!("{kind}_{safe}.txt")))
}

/// Whether this captain has seen the campaign opening on this machine. A client setting, not game
/// state: the server and the recorded world never know about it.
pub fn opening_seen(name: &str) -> bool {
    user_file("opening", name).is_some_and(|p| p.exists())
}

pub fn mark_opening_seen(name: &str) {
    let Some(p) = user_file("opening", name) else { return };
    if let Some(d) = p.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let _ = std::fs::write(p, "seen\n");
}

pub fn load_token(name: &str) -> Option<String> {
    std::fs::read_to_string(token_path(name)?).ok().map(|s| s.trim().to_string())
}

pub fn save_token(name: &str, token: &str) {
    let Some(p) = token_path(name) else { return };
    if let Some(d) = p.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    // The token is a login credential: owner-only on Unix, never through a symlink.
    // (On Windows the per-user %APPDATA% ACL already restricts access.)
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600).custom_flags(libc_nofollow());
    }
    if let Ok(mut f) = opts.open(&p) {
        #[cfg(unix)]
        {
            // `mode` only applies on creation; tighten a pre-existing file too.
            use std::os::unix::fs::PermissionsExt;
            let _ = f.set_permissions(std::fs::Permissions::from_mode(0o600));
        }
        use std::io::Write;
        let _ = f.write_all(token.as_bytes());
    }
}

#[cfg(unix)]
fn libc_nofollow() -> i32 {
    // O_NOFOLLOW: Linux 0o400000, macOS/BSD 0x100.
    if cfg!(target_os = "linux") { 0o400000 } else { 0x100 }
}

pub fn pump_network(net: Option<Res<Net>>, mut s: ResMut<Session>, time: Res<Time<Real>>, mut next: ResMut<NextState<Screen>>, screen: Res<State<Screen>>, mut cine: Option<ResMut<crate::cinematic::Cine>>) {
    let _prof = shared::prof::span("state::pump_network");
    let dt = time.delta_secs().max(1e-4);
    s.fps += (1.0 / dt - s.fps) * 0.05;
    let Some(net) = net else { return };
    let now = time.elapsed_secs_f64();
    // While the campaign opening plays in a live game, the world keeps arriving in the stashed session.
    let s = match cine.as_mut().and_then(|c| c.live_mut()) {
        Some(live) => live,
        None => &mut *s,
    };
    while let Ok(ev) = net.rx.try_recv() {
        match ev {
            NetEvent::Connected => {
                if *screen.get() == Screen::Connecting {
                    next.set(Screen::Login);
                }
            }
            NetEvent::Disconnected(r) => s.disconnected = Some(r),
            NetEvent::Msg(m) => apply(s, *m, now, &mut next),
        }
    }
}

fn apply(s: &mut Session, m: ServerMsg, now: f64, next: &mut NextState<Screen>) {
    match m {
        ServerMsg::Welcome { player_id, token, profile, tuning, .. } => {
            if let Some(t) = tuning {
                shared::tuning::install(*t);
            }
            s.player_id = Some(player_id);
            // Replays hand over an empty token: nothing to remember.
            if !token.is_empty() {
                save_token(&s.name, &token);
            }
            s.token = Some(token);
            let has = profile.is_some();
            s.profile = profile;
            next.set(if has { Screen::Playing } else { Screen::Create });
        }
        ServerMsg::Artifacts { held } => s.artifacts = held,
        ServerMsg::Contracts { list } => s.contracts = list,
        ServerMsg::Institutes { list } => s.institutes = list,
        ServerMsg::Charted { systems } => s.charted = systems.into_iter().collect(),
        ServerMsg::Influence { systems, threshold } => {
            s.influence = systems;
            s.influence_threshold = threshold;
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
        ServerMsg::Snapshot { tick, full, infos, gone, dynamic, fx } => {
            let entities = s.snap_cache.apply(tick, full, infos, &gone, &dynamic);
            s.fx.extend(fx);
            s.snaps.push_back(Snap { at: now, ents: entities.into_iter().map(|e| (e.id, e)).collect() });
            while s.snaps.len() > 12 {
                s.snaps.pop_front();
            }
        }
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
        ServerMsg::Alliances { mine, list } => {
            s.my_alliance = mine;
            s.alliances = list;
        }
        ServerMsg::AllianceInvite { from, alliance, name } => {
            s.toast(format!("{from}이(가) 동맹 {name}에 초대했습니다 — 동맹 창(L)에서 수락"), now);
            s.alliance_invite = Some((from, alliance, name));
        }
    }
}

//! In-engine cutscenes (docs/STORY.md §5). Everything on screen comes from
//! `view::cinematic::frame_at(file, species, t)`: this module only rebuilds the star system when the
//! shot changes system, places the captain's ship and the battle fleets, sets the camera and draws the
//! subtitle layer.
//!
//! Two ways in:
//! - `client --cinematic opening [--species S] [--hull H] [--class C]`: no server, like the showroom. The
//!   clock starts at CINE_START real seconds, so with STARFALL_FIXED_FPS the same command gives the same
//!   frame (`--shot-at` counts from the start of the cinematic).
//! - In a live game (`Session::cine_request`, the campaign's first start): the player's session is stashed
//!   and keeps receiving the world (`state::pump_network` writes into it) while the screen shows a fresh
//!   one. At the end, on Esc, or as soon as the captain's ship takes damage, the stash comes back and the
//!   request's follow-up message (StartScenario) is sent. Nothing reaches the server until then.

use crate::net::Net;
use crate::state::{Args, CineRequest, Screen, Session, Snap};
use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass};
use shared::galaxy::Species;
use shared::protocol::{ClientMsg, EntityKind, EntityState, Fx};
use shared::ships::{HullClass, Module, Ship, SizeClass, WeaponKind};
use std::collections::HashMap;
use view::cinematic::{Cinematic, Frame, frame_at};
use view::voice::Cue;

/// Real seconds before the offline cinematic clock starts: time for the models to load, fixed so frames repeat.
pub const CINE_START: f64 = 3.0;
const SHIP_ID: u32 = 1;
const FLEET_ID: u32 = 2000;

#[derive(Resource)]
pub struct Cine {
    pub cin: Cinematic,
    pub species: Species,
    ship: EntityState,
    /// Real time the clock started (once the first scene is built, not before `earliest`).
    start: Option<f64>,
    earliest: f64,
    skipped: bool,
    prev_t: f32,
    live: Option<Live>,
}

/// The player's own session while a cutscene plays over a live game.
struct Live {
    session: Box<Session>,
    then: Option<ClientMsg>,
    /// Hull + shield of the captain's ship when the cutscene began (any loss ends it).
    health: Option<f32>,
}

impl Cine {
    /// Real time the cinematic clock started, once the first scene is built.
    pub fn started_at(&self) -> Option<f64> {
        self.start
    }

    pub fn time(&self, now: f64) -> f32 {
        self.start.map_or(0.0, |s| (now - s) as f32)
    }

    /// The stashed live session, while one is playing over a game.
    pub fn live_mut(&mut self) -> Option<&mut Session> {
        self.live.as_mut().map(|l| &mut *l.session)
    }

    fn frame(&self, now: f64) -> Frame<'_> {
        let t = if self.skipped { self.cin.duration() } else { self.time(now) };
        frame_at(&self.cin, self.species, t)
    }

    /// The live game wants the screen back: the captain's ship was hit or destroyed.
    fn interrupted(&self) -> bool {
        let Some(l) = &self.live else { return false };
        l.session.dead.is_some() || l.health.zip(health(&l.session)).is_some_and(|(was, now)| now < was - 1e-4)
    }
}

fn health(s: &Session) -> Option<f32> {
    s.my_state().map(|m| m.hull + m.shield)
}

pub struct CinematicPlugin;

impl Plugin for CinematicPlugin {
    fn build(&self, app: &mut App) {
        // Kept out of the Update graph: adding ordered systems there reorders the game's own systems
        // and moved the showroom art scenes by a frame (artcheck fleet 99.83% -> 98.87%). The camera is
        // set after Update's chase camera and before transforms propagate; the overlay is its own layer.
        app.add_systems(PreUpdate, (request, drive.run_if(resource_exists::<Cine>)))
            .add_systems(PostUpdate, camera.before(bevy::transform::TransformSystems::Propagate).run_if(resource_exists::<Cine>))
            .add_systems(EguiPrimaryContextPass, overlay.run_if(resource_exists::<Cine>));
    }
}

fn arg(name: &str) -> Option<String> {
    std::env::args().skip_while(|a| a != name).nth(1)
}

/// `assets/cinematics/NAME.ron`, with any problems `view::cinematic::check` finds printed.
pub fn load(name: &str) -> Cinematic {
    let path = crate::ships::assets_dir().join(format!("cinematics/{name}.ron"));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let cin = Cinematic::parse(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    for p in view::cinematic::check(&cin) {
        eprintln!("cinematic {name}: {p}");
    }
    cin
}

/// `--cinematic NAME`: sets the offline session up for it (called from `startup`).
pub fn start(commands: &mut Commands, s: &mut Session, args: &Args, name: &str) {
    let cin = load(name);
    let species = arg("--species").and_then(|v| Species::ALL.into_iter().find(|x| format!("{x:?}").eq_ignore_ascii_case(&v))).unwrap_or(Species::Human);
    let hull = args.hull.unwrap_or(HullClass::Talon);
    let class = arg("--class").and_then(|v| [SizeClass::Cruiser, SizeClass::Capital].into_iter().find(|x| format!("{x:?}").eq_ignore_ascii_case(&v)));
    let ship = captain_ship(&grown(hull, species, class), species);
    s.system = cin.sequence(species)[0].system;
    s.system_epoch = 1;
    s.me = None;
    s.profile = None;
    s.snaps.push_back(Snap { at: 0.0, ents: HashMap::new() });
    commands.insert_resource(Cine { cin, species, ship, start: None, earliest: CINE_START, skipped: false, prev_t: 0.0, live: None });
}

/// Plays `cin` over the live game in `s` (the captain's own species and ship), then sends `then`.
pub fn begin_live(s: &mut Session, cin: Cinematic, then: Option<ClientMsg>, now: f64) -> Cine {
    let (species, ship) = match &s.profile {
        Some(p) => (p.species, captain_ship(&p.ship, p.species)),
        None => (Species::Human, captain_ship(&grown(HullClass::Talon, Species::Human, None), Species::Human)),
    };
    let mut shown = Session {
        name: s.name.clone(),
        server_url: s.server_url.clone(),
        last_combat: -1e9,
        system: cin.sequence(species)[0].system,
        system_epoch: s.system_epoch + 1,
        ..default()
    };
    shown.snaps.push_back(Snap { at: now, ents: HashMap::new() });
    let session = Box::new(std::mem::replace(s, shown));
    let health = health(&session);
    Cine { cin, species, ship, start: None, earliest: now, skipped: false, prev_t: 0.0, live: Some(Live { session, then, health }) }
}

/// Gives the player's session back (a new epoch, so the scene rebuilds their system) and returns the
/// message to send now.
fn end_live(s: &mut Session, live: Live) -> Option<ClientMsg> {
    let epoch = s.system_epoch + 1;
    *s = *live.session;
    s.system_epoch = epoch;
    // Effects that arrived during the cutscene are history by now.
    s.fx.clear();
    live.then
}

/// The UI asked for a cutscene (`Session::cine_request`): start it over the live game.
fn request(mut commands: Commands, mut s: ResMut<Session>, cine: Option<Res<Cine>>, time: Res<Time<Real>>) {
    if cine.is_some() {
        return;
    }
    let Some(CineRequest { name, then }) = s.cine_request.take() else { return };
    let cin = load(&name);
    commands.insert_resource(begin_live(&mut s, cin, then, time.elapsed_secs_f64()));
}

/// A ship of `hull` grown to `class`, as the showroom builds it.
fn grown(hull: HullClass, species: Species, class: Option<SizeClass>) -> Ship {
    let mut ship = Ship::new("Captain".into(), hull, view::palette::species_livery(species));
    if let Some(class) = class {
        'grow: for _ in 0..20 {
            for m in Module::ALL {
                if ship.size_class() == class {
                    break 'grow;
                }
                *ship.modules.entry(m).or_insert(1) += 1;
            }
        }
    }
    ship
}

fn captain_ship(ship: &Ship, species: Species) -> EntityState {
    entity(SHIP_ID, EntityKind::Player, Some(ship.hull), ship, ship.livery, Some(species))
}

fn entity(id: u32, kind: EntityKind, hull: Option<HullClass>, ship: &Ship, livery: u8, species: Option<Species>) -> EntityState {
    EntityState { id, kind, name: String::new(), hull_class: hull, rank: ship.rank(), livery, pos: [0.0; 3], rot: [0.0, 0.0, 0.0, 1.0], vel: [0.0, 0.0, -40.0], hull: 1.0, shield: 1.0, radius: ship.stats().radius, fleet: None, wanted: false, boosting: false, mining: None, docked: false, owner: None, alliance: None, class: Some(ship.size_class()), species }
}

/// Two battle lines facing each other across `center` (the war shot).
fn fleets(center: Vec3) -> Vec<EntityState> {
    let mut out = vec![];
    for (side, livery, face) in [(-1.0f32, 1u8, Quat::from_rotation_y(-1.2)), (1.0, 4, Quat::from_rotation_y(1.9))] {
        for i in 0..9 {
            let hull = HullClass::ALL[i % 5];
            let mut ship = Ship::new(String::new(), hull, livery);
            for m in Module::ALL {
                ship.modules.insert(m, if i < 2 { 9 } else { 4 });
            }
            let id = FLEET_ID + out.len() as u32;
            let mut e = entity(id, EntityKind::Player, Some(hull), &ship, livery, Some(Species::ALL[i % 3]));
            let row = i as f32 - 4.0;
            e.pos = (center + Vec3::new(side * (360.0 + (i % 3) as f32 * 90.0), row * 45.0, row * 130.0)).to_array();
            e.rot = face.to_array();
            e.vel = (face * Vec3::NEG_Z * 60.0).to_array();
            e.boosting = i % 3 == 0;
            out.push(e);
        }
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn drive(
    mut commands: Commands,
    mut cine: ResMut<Cine>,
    mut s: ResMut<Session>,
    env: Res<crate::scene::Env>,
    time: Res<Time<Real>>,
    keys: Res<ButtonInput<KeyCode>>,
    args: Res<Args>,
    net: Option<Res<Net>>,
    mut exit: MessageWriter<AppExit>,
) {
    let now = time.elapsed_secs_f64();
    let ready = env.epoch == s.system_epoch && env.layout.is_some();
    if cine.start.is_none() && ready && now >= cine.earliest {
        if cine.live.is_none() && now > cine.earliest + 0.5 {
            eprintln!("cinematic: scene was ready only at {now:.1} s; frames may differ from run to run");
        }
        cine.start = Some(now);
    }
    if !cine.skipped && (keys.just_pressed(KeyCode::Escape) || cine.interrupted()) {
        cine.skipped = true;
        s.voice.push(Cue::Stop);
    }
    let t = cine.time(now);
    let (system, ship, battle, look_at, done) = {
        let f = cine.frame(now);
        (f.system, f.ship, f.battle, f.look_at, f.done)
    };
    if done && let Some(live) = cine.live.take() {
        if let Some(name) = live.session.profile.as_ref().map(|p| p.name.clone()) {
            crate::state::mark_opening_seen(&name);
        }
        if let (Some(m), Some(net)) = (end_live(&mut s, live), &net) {
            net.send(m);
        }
        s.voice.push(Cue::Stop);
        commands.remove_resource::<Cine>();
        return;
    }
    if system != s.system {
        s.system = system;
        s.system_epoch += 1;
    }
    let mut ents = HashMap::new();
    if let Some(p) = ship {
        let mut e = cine.ship.clone();
        e.pos = p.to_array();
        ents.insert(e.id, e);
    }
    if battle {
        let fl = fleets(look_at);
        // Volleys and explosions on beats of the cinematic clock (the same frames every run).
        let crossed = |every: f32, phase: f32| ((t - phase) / every).floor() > ((cine.prev_t - phase) / every).floor();
        if crossed(0.35, 0.0) {
            let k = (t / 0.35) as usize;
            let (a, b) = (fl[k % 9].id, fl[9 + (k * 7) % 9].id);
            s.fx.push(Fx::Shot { from: a, to: b, weapon: WeaponKind::Pulse, mount: (k % 6) as u8, hit: true });
            let beam = k.is_multiple_of(4);
            s.fx.push(Fx::Shot { from: b, to: a, weapon: if beam { WeaponKind::MainBeam } else { WeaponKind::Laser }, mount: if beam { 255 } else { 0 }, hit: !k.is_multiple_of(3) });
        }
        if crossed(2.2, 1.0) {
            let k = (t / 2.2) as usize;
            let p = Vec3::from(fl[(k * 5) % 18].pos) + Vec3::new(30.0, 10.0, -40.0);
            s.fx.push(Fx::Explosion { pos: p.to_array(), size: 2.5 });
        }
        ents.extend(fl.into_iter().map(|e| (e.id, e)));
    }
    // Each caption's voice line starts on the frame its subtitle comes up.
    if !cine.skipped {
        for c in view::cinematic::caption_cues(&cine.cin, cine.species, cine.prev_t, t) {
            s.voice.push(Cue::Play { voice: c.voice().into(), text: c.text.clone() });
        }
    }
    cine.prev_t = t;
    if let Some(snap) = s.snaps.back_mut() {
        snap.at = now;
        snap.ents = ents;
    }
    // Finished (or skipped) without a screenshot to take: leave, like a replay that ran out.
    if done && args.shot.is_none() {
        exit.write(AppExit::Success);
    }
}

fn camera(cine: Res<Cine>, time: Res<Time<Real>>, mut cam: Query<&mut Transform, With<crate::control::MainCamera>>) {
    let Ok(mut ct) = cam.single_mut() else { return };
    let f = cine.frame(time.elapsed_secs_f64());
    let eye = view::cinematic::eye_for_ship(&f, cine.ship.radius);
    *ct = Transform::from_translation(eye).looking_at(f.look_at, Vec3::Y);
}

fn overlay(mut contexts: EguiContexts, cine: Res<Cine>, time: Res<Time<Real>>, screen: Res<State<Screen>>) -> Result {
    if *screen.get() != Screen::Playing {
        return Ok(());
    }
    let ctx = contexts.ctx_mut()?;
    let f = cine.frame(time.elapsed_secs_f64());
    view::cinematic::draw(ctx, &f, !f.done);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::NetEvent;
    use bevy::state::app::StatesPlugin;
    use shared::protocol::ServerMsg;

    /// A logged-in captain floating in Orion Drift, with a link the test feeds and reads.
    fn live_app() -> (App, crossbeam_channel::Sender<NetEvent>, crossbeam_channel::Receiver<ClientMsg>) {
        let (net, feed, out) = Net::local();
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, StatesPlugin))
            .init_state::<Screen>()
            .init_resource::<crate::scene::Env>()
            .init_resource::<ButtonInput<KeyCode>>()
            .insert_resource(Args { server: String::new(), name: None, auto: false, hull: None, shot: None, shot_at: 0.0, showroom: false, script: vec![], look: None, replay: None, no_create: false, token: None })
            .insert_resource(net)
            .add_systems(PreUpdate, (request, drive.run_if(resource_exists::<Cine>), crate::state::pump_network).chain());
        let mut s = Session { name: "Tester".into(), system: 3, system_epoch: 7, me: Some(42), ..default() };
        s.ui.missions = true;
        s.snaps.push_back(Snap { at: 0.0, ents: [(42, captain(1.0))].into_iter().collect() });
        app.insert_resource(s);
        (app, feed, out)
    }

    fn captain(hull: f32) -> EntityState {
        let mut e = captain_ship(&Ship::new("Tester".into(), HullClass::Talon, 0), Species::Human);
        e.id = 42;
        e.hull = hull;
        e
    }

    fn play_opening(app: &mut App) {
        app.world_mut().resource_mut::<Session>().cine_request = Some(CineRequest { name: "opening".into(), then: Some(ClientMsg::StartScenario { id: 0 }) });
        app.update();
        app.update();
    }

    #[test]
    fn the_opening_shows_a_fresh_scene_and_keeps_the_live_session_aside() {
        let (mut app, _feed, out) = live_app();
        play_opening(&mut app);
        let w = app.world_mut();
        let s = w.resource::<Session>();
        assert_eq!(s.system, 9, "the opening starts at The Deep");
        assert!(s.system_epoch > 7, "a new epoch, so the scene rebuilds");
        assert!(s.me.is_none() && !s.ui.missions, "no captain ship to fly and no windows over the cutscene");
        let mut cine = w.resource_mut::<Cine>();
        let live = cine.live_mut().unwrap();
        assert_eq!((live.system, live.me, live.ui.missions), (3, Some(42), true));
        assert!(out.try_iter().all(|m| !matches!(m, ClientMsg::StartScenario { .. })), "nothing is started until the opening ends");
    }

    #[test]
    fn the_world_keeps_arriving_in_the_live_session_during_the_opening() {
        let (mut app, feed, _out) = live_app();
        play_opening(&mut app);
        feed.send(NetEvent::Msg(Box::new(ServerMsg::Toast { text: "누군가 도착했습니다".into() }))).unwrap();
        feed.send(NetEvent::Msg(Box::new(ServerMsg::EnterSystem { system: 5, you: 43 }))).unwrap();
        app.update();
        let shown = app.world().resource::<Session>();
        assert!(shown.toasts.is_empty() && shown.system == 9, "the cutscene's screen is untouched");
        let mut cine = app.world_mut().resource_mut::<Cine>();
        let live = cine.live_mut().unwrap();
        assert_eq!((live.system, live.me, live.toasts.len()), (5, Some(43), 1));
    }

    #[test]
    fn esc_gives_the_game_back_and_starts_the_scenario() {
        let (mut app, _feed, out) = live_app();
        play_opening(&mut app);
        let shown_epoch = app.world().resource::<Session>().system_epoch;
        app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::Escape);
        app.update();
        assert!(app.world().get_resource::<Cine>().is_none());
        let s = app.world().resource::<Session>();
        assert_eq!((s.system, s.me, s.ui.missions), (3, Some(42), true), "the player's own session is back");
        assert!(s.system_epoch > shown_epoch, "and their system is rebuilt");
        assert!(s.voice.contains(&Cue::Stop), "the narrator falls silent with the cutscene");
        let sent: Vec<_> = out.try_iter().collect();
        assert!(sent.iter().any(|m| matches!(m, ClientMsg::StartScenario { id: 0 })), "{sent:?}");
    }

    #[test]
    fn taking_damage_ends_the_opening_at_once() {
        let (mut app, feed, out) = live_app();
        play_opening(&mut app);
        assert!(app.world().get_resource::<Cine>().is_some());
        // The server's next snapshot says the captain was hit (fed as the live session would receive it).
        app.world_mut().resource_mut::<Cine>().live_mut().unwrap().snaps.push_back(Snap { at: 1.0, ents: [(42, captain(0.8))].into_iter().collect() });
        drop(feed);
        app.update();
        assert!(app.world().get_resource::<Cine>().is_none(), "a hit captain gets the controls back");
        assert_eq!(app.world().resource::<Session>().me, Some(42));
        assert!(out.try_iter().any(|m| matches!(m, ClientMsg::StartScenario { id: 0 })));
    }
}

//! `client --cinematic opening [--species S] [--hull H] [--class C]`: plays an in-engine cutscene
//! (docs/STORY.md §5) with no server, like the showroom. Everything on screen comes from
//! `view::cinematic::frame_at(file, species, t)`: this module only rebuilds the star system when the
//! shot changes system, places the captain's ship and the battle fleets, sets the camera and draws the
//! subtitle layer. The clock starts at CINE_START real seconds, so with STARFALL_FIXED_FPS the same
//! command gives the same frame (`--shot-at` counts from the start of the cinematic).

use crate::state::{Args, Screen, Session, Snap};
use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass};
use shared::galaxy::Species;
use shared::protocol::{EntityKind, EntityState, Fx};
use shared::ships::{HullClass, Module, Ship, SizeClass, WeaponKind};
use std::collections::HashMap;
use view::cinematic::{Cinematic, Frame, frame_at};

/// Real seconds before the cinematic clock starts: time for the models to load, fixed so frames repeat.
pub const CINE_START: f64 = 3.0;
const SHIP_ID: u32 = 1;
const FLEET_ID: u32 = 2000;

#[derive(Resource)]
pub struct Cine {
    pub cin: Cinematic,
    pub species: Species,
    ship: EntityState,
    /// Real time the clock started (CINE_START, or later if the scene was not ready yet).
    start: Option<f64>,
    skipped: bool,
    prev_t: f32,
}

impl Cine {
    /// Real time the cinematic clock started, once the first scene is built.
    pub fn started_at(&self) -> Option<f64> {
        self.start
    }

    pub fn time(&self, now: f64) -> f32 {
        self.start.map_or(0.0, |s| (now - s) as f32)
    }

    fn frame(&self, now: f64) -> Frame<'_> {
        let t = if self.skipped { self.cin.duration() } else { self.time(now) };
        frame_at(&self.cin, self.species, t)
    }
}

pub struct CinematicPlugin;

impl Plugin for CinematicPlugin {
    fn build(&self, app: &mut App) {
        // Kept out of the Update graph: adding ordered systems there reorders the game's own systems
        // and moved the showroom art scenes by a frame (artcheck fleet 99.83% -> 98.87%). The camera is
        // set after Update's chase camera and before transforms propagate; the overlay is its own layer.
        app.add_systems(PreUpdate, drive.run_if(resource_exists::<Cine>))
            .add_systems(PostUpdate, camera.before(bevy::transform::TransformSystems::Propagate).run_if(resource_exists::<Cine>))
            .add_systems(EguiPrimaryContextPass, overlay.run_if(resource_exists::<Cine>));
    }
}

fn arg(name: &str) -> Option<String> {
    std::env::args().skip_while(|a| a != name).nth(1)
}

/// Loads `assets/cinematics/NAME.ron` and sets the session up for it (called from `startup`).
pub fn start(commands: &mut Commands, s: &mut Session, args: &Args, name: &str) {
    let path = crate::ships::assets_dir().join(format!("cinematics/{name}.ron"));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let cin = Cinematic::parse(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let problems = view::cinematic::check(&cin);
    for p in &problems {
        eprintln!("cinematic {name}: {p}");
    }
    let species = arg("--species").and_then(|v| Species::ALL.into_iter().find(|x| format!("{x:?}").eq_ignore_ascii_case(&v))).unwrap_or(Species::Human);
    let hull = args.hull.unwrap_or(HullClass::Talon);
    let class = arg("--class").and_then(|v| [SizeClass::Cruiser, SizeClass::Capital].into_iter().find(|x| format!("{x:?}").eq_ignore_ascii_case(&v)));
    let ship = captain_ship(hull, species, class);
    let first = cin.sequence(species)[0].system;
    s.system = first;
    s.system_epoch = 1;
    s.me = None;
    s.profile = None;
    s.snaps.push_back(Snap { at: 0.0, ents: HashMap::new() });
    commands.insert_resource(Cine { cin, species, ship, start: None, skipped: false, prev_t: 0.0 });
}

/// The captain's ship as the showroom builds it: hull grown to `class`, species kit and livery.
fn captain_ship(hull: HullClass, species: Species, class: Option<SizeClass>) -> EntityState {
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
    entity(SHIP_ID, EntityKind::Player, Some(hull), &ship, view::palette::species_livery(species), Some(species))
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

fn drive(mut cine: ResMut<Cine>, mut s: ResMut<Session>, env: Res<crate::scene::Env>, time: Res<Time<Real>>, keys: Res<ButtonInput<KeyCode>>, args: Res<Args>, mut exit: MessageWriter<AppExit>) {
    let now = time.elapsed_secs_f64();
    let ready = env.epoch == s.system_epoch && env.layout.is_some();
    if cine.start.is_none() && ready && now >= CINE_START {
        if now > CINE_START + 0.5 {
            eprintln!("cinematic: scene was ready only at {now:.1} s; frames may differ from run to run");
        }
        cine.start = Some(now);
    }
    if keys.just_pressed(KeyCode::Escape) {
        cine.skipped = true;
    }
    let t = cine.time(now);
    let (system, ship, battle, look_at, done) = {
        let f = cine.frame(now);
        (f.system, f.ship, f.battle, f.look_at, f.done)
    };
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

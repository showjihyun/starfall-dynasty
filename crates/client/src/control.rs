//! Chase camera and flight controls. Keyboard + mouse map to `Controls`; action keys map to intents.

use crate::meshes;
use crate::net::Net;
use crate::scene::Env;
use crate::ships::{ShipVis, Visuals};
use crate::state::{Screen, Session};
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::input::mouse::AccumulatedMouseScroll;
use bevy::light::GeneratedEnvironmentMapLight;
use bevy::pbr::{DistanceFog, FogFalloff};
use bevy::post_process::bloom::{Bloom, BloomCompositeMode, BloomPrefilter};
use bevy::prelude::*;
use bevy::camera::Hdr;
use bevy_egui::input::EguiWantsInput;
use shared::galaxy::{DOCK_RANGE, MINE_RANGE};
use shared::protocol::{ClientMsg, Controls, EntityKind};

pub struct ControlPlugin;

impl Plugin for ControlPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CamState>()
            .add_systems(Startup, spawn_camera)
            .add_systems(Update, (flight_input, action_keys.run_if(not(resource_exists::<crate::cinematic::Cine>)), chase_camera).run_if(in_state(Screen::Playing)))
            .add_systems(Update, idle_camera.run_if(not(in_state(Screen::Playing))));
    }
}

/// The player's view camera (the interior panel and the bridge feed add more cameras).
#[derive(Component)]
pub struct MainCamera;

#[derive(Resource)]
pub struct CamState {
    pub zoom: f32,
    /// First person (bridge view) vs third person (chase).
    pub first_person: bool,
    pub last_sent: Controls,
    pub last_send_t: f64,
}

impl Default for CamState {
    fn default() -> Self {
        CamState { zoom: 1.0, first_person: false, last_sent: Controls::default(), last_send_t: 0.0 }
    }
}

/// The main camera's bloom (removed at Low quality, put back above it).
pub fn main_bloom() -> Bloom {
    Bloom { intensity: 0.2, low_frequency_boost: 0.55, prefilter: BloomPrefilter { threshold: 0.9, threshold_softness: 0.45 }, composite_mode: BloomCompositeMode::Additive, ..Bloom::NATURAL }
}

fn spawn_camera(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let cube = images.add(meshes::sky_cubemap([0.2, 0.3, 0.6], [0.5, 0.2, 0.4], Vec3::Z, [1.0, 0.9, 0.7], 1));
    commands.spawn((
        Camera3d::default(),
        MainCamera,
        // The HUD belongs to this camera, not the off-screen interior/feed cameras.
        bevy_egui::PrimaryEguiContext,
        Hdr,
        Camera { clear_color: ClearColorConfig::Custom(Color::BLACK), ..default() },
        Tonemapping::TonyMcMapface,
        // Thresholded bloom: only genuinely bright things (stars, plumes, beams, blasts) glow;
        // lit hulls and nebulae stay crisp instead of sitting in a haze.
        main_bloom(),
        // Space haze, tuned per system in scene::build_system (none until then).
        DistanceFog { color: Color::BLACK, falloff: FogFalloff::Exponential { density: 0.0 }, ..default() },
        Projection::Perspective(PerspectiveProjection { fov: 62f32.to_radians(), near: 0.5, far: 1_000_000.0, ..default() }),
        GeneratedEnvironmentMapLight { environment_map: cube, intensity: 700.0, ..default() },
        Transform::from_xyz(0.0, 40.0, 3700.0).looking_at(Vec3::new(0.0, 0.0, 3400.0), Vec3::Y),
    ));
    commands.insert_resource(GlobalAmbientLight { color: Color::srgb(0.5, 0.55, 0.7), brightness: 12.0, ..default() });
}

fn idle_camera(time: Res<Time>, s: Res<Session>, vis: Res<Visuals>, ships: Query<(&ShipVis, &Transform), Without<MainCamera>>, mut cam: Query<&mut Transform, With<MainCamera>>) {
    let Ok(mut t) = cam.single_mut() else { return };
    // The character creation screen frames the ship on show in the empty lower middle of the screen.
    if let Some((sv, ship)) = s.preview.and_then(|id| vis.map.get(&id)).and_then(|e| ships.get(*e).ok()) {
        let (pos, look) = view::anim::preview_camera(ship.translation, sv.radius.max(8.0));
        t.translation = pos;
        t.look_at(look, Vec3::Y);
        return;
    }
    let a = time.elapsed_secs() * 0.03;
    let center = Vec3::new(0.0, 0.0, 3400.0);
    t.translation = center + Vec3::new(a.cos() * 900.0, 180.0, a.sin() * 900.0);
    t.look_at(center, Vec3::Y);
}

#[allow(clippy::too_many_arguments)]
fn chase_camera(
    time: Res<Time>,
    s: Res<Session>,
    vis: Res<Visuals>,
    env: Res<Env>,
    ships: Query<(&ShipVis, &Transform), Without<MainCamera>>,
    globals: Query<&GlobalTransform>,
    mut cam: Query<&mut Transform, With<MainCamera>>,
    mut cs: ResMut<CamState>,
    scroll: Res<AccumulatedMouseScroll>,
    wants: Option<Res<EguiWantsInput>>,
    args: Res<crate::state::Args>,
) {
    let _prof = shared::prof::span("control::chase_camera");
    let Ok(mut ct) = cam.single_mut() else { return };
    // A screenshot run ignores the real mouse wheel: scrolling over the window as it opens must not
    // change the shot (`--shot`, docs/HARNESS.md: same command → identical image).
    if args.shot.is_none() && !wants.as_ref().is_some_and(|w| w.is_pointer_over_area()) {
        cs.zoom = (cs.zoom * (1.0 - scroll.delta.y * 0.1)).clamp(0.45, 5.0);
    }
    let dt = time.delta_secs();
    let me = s.me.and_then(|id| vis.map.get(&id)).and_then(|e| ships.get(*e).ok());
    let (target_pos, target_rot) = match me {
        Some((sv, t)) if cs.first_person && !s.my_state().is_some_and(|m| m.docked) => {
            // Bridge view: from the Bridge node if the model has one, else a cockpit estimate.
            let pos = sv.bridge.and_then(|b| globals.get(b).ok()).map(|g| g.translation()).unwrap_or(t.translation + t.rotation * Vec3::new(0.0, sv.radius * 0.22, -sv.radius * 0.45));
            let rot = t.rotation;
            // Snap tightly: the bridge is rigidly attached to the hull.
            ct.translation = pos;
            ct.rotation = rot;
            return;
        }
        Some((sv, t)) if !s.my_state().is_some_and(|m| m.docked) => {
            let r = sv.radius.max(8.0) * cs.zoom;
            let offset = t.rotation * Vec3::new(0.0, r * 1.05, r * 3.8);
            let look = t.translation + t.rotation * Vec3::NEG_Z * r * 6.0;
            let pos = t.translation + offset;
            let rot = Transform::from_translation(pos).looking_at(look, t.rotation * Vec3::Y).rotation;
            (pos, rot)
        }
        _ if args.look.as_deref() == Some("bow") => {
            // Showroom species check: a fixed bow three-quarter view of the `--hull` ship, so its species
            // kit (masts, ribs, horns) faces the camera instead of the engines.
            let focus = args.hull.map(crate::showroom_id);
            let Some((sv, t)) = ships.iter().find(|(sv, _)| Some(sv.id) == focus) else { return };
            let pos = t.translation + t.rotation * Vec3::new(1.0, 0.5, -0.75).normalize() * sv.radius.max(8.0) * 2.3 * cs.zoom;
            (pos, Transform::from_translation(pos).looking_at(t.translation, t.rotation * Vec3::Y).rotation)
        }
        _ => {
            // Docked or dead: slow orbit around the station (or a showroom point of interest).
            let look = args.look.as_deref().and_then(|k| {
                if k == "leviathan" {
                    // Showroom `--leviathan`: orbit the Sentinel and its three targets.
                    return ships.iter().find(|(sv, _)| sv.id == 300).map(|(sv, t)| (t.translation, sv.radius.max(60.0) * 4.0));
                }
                if k == "ship" {
                    // Showroom art check of one model (`--class`): orbit the captain's ship (the only one with a species).
                    return ships.iter().find(|(sv, _)| sv.species_of.is_some()).map(|(sv, t)| (t.translation, sv.radius.max(8.0) * 2.0));
                }
                let l = env.layout.as_ref()?;
                let kind = match k {
                    "gate" => shared::galaxy::PoiKind::AncientGate,
                    "ruin" => shared::galaxy::PoiKind::Ruin,
                    "derelict" => shared::galaxy::PoiKind::Derelict,
                    _ => return None,
                };
                l.pois.iter().find(|p| p.kind == kind).map(|p| (p.pos, if kind == shared::galaxy::PoiKind::AncientGate { 1500.0 } else { 300.0 }))
            });
            let (center, dist) = look.unwrap_or((env.layout.as_ref().map(|l| l.station).unwrap_or(Vec3::new(0.0, 0.0, 3400.0)), 520.0));
            let a = time.elapsed_secs() * 0.05;
            let pos = center + Vec3::new(a.cos() * dist * cs.zoom, dist * 0.27 * cs.zoom, a.sin() * dist * cs.zoom);
            (pos, Transform::from_translation(pos).looking_at(center, Vec3::Y).rotation)
        }
    };
    let k = 1.0 - (-dt * 7.0).exp();
    let kr = 1.0 - (-dt * 5.0).exp();
    if ct.translation.distance(target_pos) > 3000.0 {
        ct.translation = target_pos;
        ct.rotation = target_rot;
    } else {
        ct.translation = ct.translation.lerp(target_pos, k);
        ct.rotation = ct.rotation.slerp(target_rot, kr);
    }
}

#[allow(clippy::too_many_arguments)]
fn flight_input(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    wants: Option<Res<EguiWantsInput>>,
    time: Res<Time<Real>>,
    net: Option<Res<Net>>,
    mut cs: ResMut<CamState>,
    s: Res<Session>,
    binds: Res<crate::settings::KeyBinds>,
    pads: Query<&Gamepad>,
) {
    let _prof = shared::prof::span("control::flight_input");
    let Some(net) = net else { return };
    let typing = wants.as_ref().is_some_and(|w| w.wants_any_keyboard_input());
    let over_ui = wants.as_ref().is_some_and(|w| w.is_pointer_over_area());
    let mut c = Controls::default();
    if !typing && s.me.is_some() {
        use view::keymap::Action as A;
        let b = |a| binds.key(a);
        let k = |code: KeyCode| if keys.pressed(code) { 1.0 } else { 0.0 };
        c.thrust = k(b(A::ThrustForward)) - k(b(A::ThrustBack));
        // Alt + the turn keys slides sideways instead of turning (6DOF: strafe on all three axes).
        let alt = keys.pressed(KeyCode::AltLeft) || keys.pressed(KeyCode::AltRight);
        let ad = k(b(A::TurnRight)) - k(b(A::TurnLeft));
        c.yaw = if alt { 0.0 } else { ad } + k(KeyCode::ArrowRight) - k(KeyCode::ArrowLeft);
        c.strafe_x = if alt { ad } else { 0.0 };
        c.pitch = k(KeyCode::ArrowUp) - k(KeyCode::ArrowDown) + k(b(A::NoseUp)) - k(b(A::NoseDown));
        c.roll = k(b(A::RollRight)) - k(b(A::RollLeft));
        // Rise and sink (Z and Ctrl by default; C is the camera toggle).
        c.strafe_y = k(b(A::Rise)) - k(b(A::Sink));
        c.boost = keys.pressed(b(A::Boost));
        c.fire = keys.pressed(b(A::Fire)) || (mouse.pressed(MouseButton::Left) && !over_ui);
        // A gamepad adds to the keys (view::pad: sticks, triggers, bumpers).
        if let Some(pad) = pads.iter().next() {
            let f = view::pad::flight(&pad_state(pad));
            c.thrust = (c.thrust + f.thrust).clamp(-1.0, 1.0);
            c.yaw = (c.yaw + f.yaw).clamp(-1.0, 1.0);
            c.pitch = (c.pitch + f.pitch).clamp(-1.0, 1.0);
            c.roll = (c.roll + f.roll).clamp(-1.0, 1.0);
            c.strafe_y = (c.strafe_y + f.rise).clamp(-1.0, 1.0);
            c.fire |= f.fire;
            c.boost |= f.boost;
        }
        c.hold_post = s.hold_post;
        // Mouse steering: hold the right button, steer toward the cursor.
        if mouse.pressed(MouseButton::Right)
            && let Ok(w) = windows.single()
            && let Some(p) = w.cursor_position()
        {
            let dx = (p.x / w.width() - 0.5) * 2.0;
            let dy = (p.y / w.height() - 0.5) * 2.0;
            let dz = |v: f32| if v.abs() < 0.06 { 0.0 } else { (v - 0.06 * v.signum()) * 2.2 };
            c.yaw = (c.yaw + dz(dx)).clamp(-1.0, 1.0);
            c.pitch = (c.pitch - dz(dy)).clamp(-1.0, 1.0);
        }
    }
    let now = time.elapsed_secs_f64();
    if c != cs.last_sent || now - cs.last_send_t > 0.25 {
        net.send(ClientMsg::Controls(c));
        cs.last_sent = c;
        cs.last_send_t = now;
    }
}

/// A gamepad's sticks, triggers and bumpers (`view::pad`).
pub fn pad_state(pad: &Gamepad) -> view::pad::PadState {
    let l = pad.left_stick();
    let r = pad.right_stick();
    view::pad::PadState {
        left: (l.x, l.y),
        right: (r.x, r.y),
        left_trigger: pad.get(GamepadButton::LeftTrigger2).unwrap_or(0.0),
        right_trigger: pad.get(GamepadButton::RightTrigger2).unwrap_or(0.0),
        left_bumper: pad.pressed(GamepadButton::LeftTrigger),
        right_bumper: pad.pressed(GamepadButton::RightTrigger),
    }
}

/// The pad button for an action, where it has one (`view::pad` docs).
fn pad_button(a: view::keymap::Action) -> Option<GamepadButton> {
    use view::keymap::Action as A;
    Some(match a {
        A::Target | A::Rebuild => GamepadButton::South,
        A::Dock => GamepadButton::East,
        A::Scan => GamepadButton::West,
        A::Pulse => GamepadButton::DPadUp,
        A::Missile => GamepadButton::North,
        A::Map => GamepadButton::Select,
        _ => return None,
    })
}

/// Discrete actions: targeting, missiles, docking, mining, scanning, investigating, windows.
#[allow(clippy::too_many_arguments)]
fn action_keys(keys: Res<ButtonInput<KeyCode>>, wants: Option<Res<EguiWantsInput>>, net: Option<Res<Net>>, mut s: ResMut<Session>, env: Res<Env>, time: Res<Time<Real>>, mut cs: ResMut<CamState>, binds: Res<crate::settings::KeyBinds>, pads: Query<&Gamepad>) {
    let _prof = shared::prof::span("control::action_keys");
    let Some(net) = net else { return };
    if wants.as_ref().is_some_and(|w| w.wants_any_keyboard_input()) {
        return;
    }
    let now = time.elapsed_secs_f64();
    use view::keymap::Action as A;
    let pressed = |k: KeyCode| keys.just_pressed(k);
    let pad = pads.iter().next();
    let act = |a: A| keys.just_pressed(binds.key(a)) || pad.zip(pad_button(a)).is_some_and(|(p, b)| p.just_pressed(b));
    // Windows.
    if act(A::Map) {
        s.ui.map = !s.ui.map;
    }
    if act(A::Station) {
        s.ui.station = !s.ui.station;
    }
    if act(A::Exchange) {
        s.ui.exchange = !s.ui.exchange;
    }
    if act(A::Jobs) {
        s.ui.jobs = !s.ui.jobs;
    }
    if act(A::Missions) {
        s.ui.missions = !s.ui.missions;
    }
    if act(A::Contracts) {
        s.ui.contracts = !s.ui.contracts;
        if s.ui.contracts {
            net.send(ClientMsg::RequestContracts);
        }
    }
    if act(A::Chronicle) {
        s.ui.chronicle = !s.ui.chronicle;
        if s.ui.chronicle {
            net.send(ClientMsg::RequestChronicle);
        }
    }
    if act(A::Fleet) {
        s.ui.fleet = !s.ui.fleet;
    }
    if act(A::Help) {
        s.ui.help = !s.ui.help;
    }
    if act(A::Alliance) {
        s.ui.alliance = !s.ui.alliance;
        if s.ui.alliance {
            net.send(ClientMsg::RequestAlliances);
        }
    }
    if act(A::Camera) {
        cs.first_person = !cs.first_person;
        s.toast(if cs.first_person { "시점: 1인칭 (함교)" } else { "시점: 3인칭 (추적)" }, now);
    }
    if act(A::HoldPost) {
        s.hold_post = !s.hold_post;
        let what = if s.fleet_formation == shared::formation::Formation::Free { " — 함대장이 진형을 정하면 작동" } else { "" };
        let msg = if s.hold_post { format!("진형 자리 지키기: 켬 (조준·사격은 직접){what}") } else { "진형 자리 지키기: 끔".into() };
        s.toast(msg, now);
    }
    if act(A::EscortAttack) {
        net.send(ClientMsg::EscortCommand { attack: true });
    }
    if act(A::EscortReturn) {
        net.send(ClientMsg::EscortCommand { attack: false });
    }
    if s.dead.is_some() {
        if act(A::Rebuild) {
            net.send(ClientMsg::Respawn);
        }
        return;
    }
    let Some(me) = s.my_state().cloned() else { return };
    let my_pos = Vec3::from(me.pos);
    let fwd = Quat::from_array(me.rot) * Vec3::NEG_Z;
    let latest = s.latest().map(|l| l.ents.values().cloned().collect::<Vec<_>>()).unwrap_or_default();
    if act(A::Target) {
        // Target the hostile closest to the crosshair, weighted by distance.
        let best = latest
            .iter()
            .filter(|e| e.id != me.id && matches!(e.kind, EntityKind::Npc(_) | EntityKind::Player) && !e.docked && (me.fleet.is_none() || e.fleet != me.fleet))
            .map(|e| {
                let d = Vec3::from(e.pos) - my_pos;
                let ang = fwd.angle_between(d.normalize_or_zero());
                (e.id, ang * 3.0 + d.length() / 1500.0)
            })
            .min_by(|a, b| a.1.total_cmp(&b.1));
        s.target = best.map(|b| b.0);
        net.send(ClientMsg::SetTarget(s.target));
    }
    // Esc: drop the target, else close windows, else the system menu (settings, quit).
    if pressed(KeyCode::Escape) || pad.is_some_and(|p| p.just_pressed(GamepadButton::Start)) {
        if s.target.is_some() {
            s.target = None;
            net.send(ClientMsg::SetTarget(None));
        } else {
            s.ui.escape();
        }
    }
    if act(A::Pulse) {
        net.send(ClientMsg::Pulse);
    }
    if act(A::Missile) {
        net.send(ClientMsg::FireMissile);
    }
    let Some(layout) = &env.layout else { return };
    if act(A::Dock) {
        if me.docked {
            net.send(ClientMsg::Undock);
        } else if my_pos.distance(layout.station) < DOCK_RANGE + 80.0 + me.radius {
            net.send(ClientMsg::Dock);
            s.ui.station = true;
        } else {
            s.toast(format!("정거장까지 {:.0}u — 더 접근하세요", my_pos.distance(layout.station) - DOCK_RANGE), now);
        }
    }
    if act(A::Mine) {
        if me.mining.is_some() {
            net.send(ClientMsg::Mine { asteroid: None });
        } else {
            let a = layout.asteroids.iter().min_by(|a, b| a.pos.distance(my_pos).total_cmp(&b.pos.distance(my_pos)));
            match a {
                Some(a) if a.pos.distance(my_pos) < MINE_RANGE + a.radius + me.radius => {
                    net.send(ClientMsg::Mine { asteroid: Some(a.id) });
                    if let Some(m) = shared::minerals::mineral(a.mineral) {
                        s.toast(format!("채굴: {}", m.label()), now);
                    }
                }
                Some(a) => s.toast(format!("가장 가까운 소행성까지 {:.0}u — 채굴 사거리 {:.0}u", a.pos.distance(my_pos), MINE_RANGE), now),
                None => s.toast("이 성계엔 소행성이 없습니다", now),
            }
        }
    }
    if act(A::Scan) {
        // Scan the nearest point of interest, or investigate the nearest wreck if closer.
        let poi = layout.pois.iter().filter(|p| s.revealed.contains(&p.id) || s.scanned.contains(&p.id)).min_by(|a, b| a.pos.distance(my_pos).total_cmp(&b.pos.distance(my_pos)));
        let wreck = latest.iter().filter(|e| e.kind == EntityKind::Wreck).min_by(|a, b| Vec3::from(a.pos).distance(my_pos).total_cmp(&Vec3::from(b.pos).distance(my_pos)));
        let wd = wreck.map(|w| Vec3::from(w.pos).distance(my_pos)).unwrap_or(f32::MAX);
        if wd < 400.0 + me.radius {
            net.send(ClientMsg::InvestigateWreck { wreck: wreck.unwrap().id });
        } else if let Some(p) = poi {
            net.send(ClientMsg::Scan { poi: p.id });
        }
    }
}

#[cfg(test)]
mod pad_tests {
    use super::*;

    #[test]
    fn a_gamepad_reads_its_sticks_triggers_and_bumpers() {
        let mut pad = Gamepad::default();
        pad.analog_mut().set(GamepadAxis::LeftStickY, 1.0);
        pad.analog_mut().set(GamepadAxis::RightStickX, -0.5);
        pad.analog_mut().set(GamepadButton::RightTrigger2, 0.9);
        pad.digital_mut().press(GamepadButton::LeftTrigger);
        let st = pad_state(&pad);
        assert_eq!((st.left.1, st.right.0, st.right_trigger, st.left_bumper, st.right_bumper), (1.0, -0.5, 0.9, true, false));
        let f = view::pad::flight(&st);
        assert!(f.thrust == 1.0 && f.roll < 0.0 && f.fire && f.rise == -1.0, "{f:?}");
        assert_eq!(pad_button(view::keymap::Action::Dock), Some(GamepadButton::East));
        assert_eq!(pad_button(view::keymap::Action::Chronicle), None);
    }
}

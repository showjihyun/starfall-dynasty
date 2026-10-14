//! Chase camera and flight controls. Keyboard + mouse map to `Controls`; action keys map to intents.

use crate::meshes;
use crate::net::Net;
use crate::scene::Env;
use crate::ships::{ShipVis, Visuals};
use crate::state::{Screen, Session};
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::input::mouse::AccumulatedMouseScroll;
use bevy::light::GeneratedEnvironmentMapLight;
use bevy::post_process::bloom::Bloom;
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
            .add_systems(Update, (flight_input, action_keys, chase_camera).run_if(in_state(Screen::Playing)))
            .add_systems(Update, idle_camera.run_if(not(in_state(Screen::Playing))));
    }
}

#[derive(Resource)]
pub struct CamState {
    pub zoom: f32,
    pub last_sent: Controls,
    pub last_send_t: f64,
}

impl Default for CamState {
    fn default() -> Self {
        CamState { zoom: 1.0, last_sent: Controls::default(), last_send_t: 0.0 }
    }
}

fn spawn_camera(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let cube = images.add(meshes::sky_cubemap([0.2, 0.3, 0.6], [0.5, 0.2, 0.4], Vec3::Z, [1.0, 0.9, 0.7], 1));
    commands.spawn((
        Camera3d::default(),
        Hdr,
        Camera { clear_color: ClearColorConfig::Custom(Color::BLACK), ..default() },
        Tonemapping::TonyMcMapface,
        Bloom { intensity: 0.15, ..Bloom::NATURAL },
        Projection::Perspective(PerspectiveProjection { fov: 62f32.to_radians(), near: 0.5, far: 1_000_000.0, ..default() }),
        GeneratedEnvironmentMapLight { environment_map: cube, intensity: 700.0, ..default() },
        Transform::from_xyz(0.0, 40.0, 3700.0).looking_at(Vec3::new(0.0, 0.0, 3400.0), Vec3::Y),
    ));
    commands.insert_resource(GlobalAmbientLight { color: Color::srgb(0.5, 0.55, 0.7), brightness: 12.0, ..default() });
}

fn idle_camera(time: Res<Time>, mut cam: Query<&mut Transform, With<Camera3d>>) {
    let Ok(mut t) = cam.single_mut() else { return };
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
    ships: Query<(&ShipVis, &Transform), Without<Camera3d>>,
    mut cam: Query<&mut Transform, With<Camera3d>>,
    mut cs: ResMut<CamState>,
    scroll: Res<AccumulatedMouseScroll>,
    wants: Option<Res<EguiWantsInput>>,
) {
    let Ok(mut ct) = cam.single_mut() else { return };
    if !wants.as_ref().is_some_and(|w| w.is_pointer_over_area()) {
        cs.zoom = (cs.zoom * (1.0 - scroll.delta.y * 0.1)).clamp(0.45, 5.0);
    }
    let dt = time.delta_secs();
    let me = s.me.and_then(|id| vis.map.get(&id)).and_then(|e| ships.get(*e).ok());
    let (target_pos, target_rot) = match me {
        Some((sv, t)) if !s.my_state().is_some_and(|m| m.docked) => {
            let r = sv.radius.max(8.0) * cs.zoom;
            let offset = t.rotation * Vec3::new(0.0, r * 1.05, r * 3.8);
            let look = t.translation + t.rotation * Vec3::NEG_Z * r * 6.0;
            let pos = t.translation + offset;
            let rot = Transform::from_translation(pos).looking_at(look, t.rotation * Vec3::Y).rotation;
            (pos, rot)
        }
        _ => {
            // Docked or dead: slow orbit around the station or the wreck site.
            let center = env.layout.as_ref().map(|l| l.station).unwrap_or(Vec3::new(0.0, 0.0, 3400.0));
            let a = time.elapsed_secs() * 0.05;
            let pos = center + Vec3::new(a.cos() * 520.0 * cs.zoom, 140.0 * cs.zoom, a.sin() * 520.0 * cs.zoom);
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
) {
    let Some(net) = net else { return };
    let typing = wants.as_ref().is_some_and(|w| w.wants_any_keyboard_input());
    let over_ui = wants.as_ref().is_some_and(|w| w.is_pointer_over_area());
    let mut c = Controls::default();
    if !typing && s.me.is_some() {
        let k = |code: KeyCode| if keys.pressed(code) { 1.0 } else { 0.0 };
        c.thrust = k(KeyCode::KeyW) - k(KeyCode::KeyS);
        c.yaw = k(KeyCode::KeyD) - k(KeyCode::KeyA) + k(KeyCode::ArrowRight) - k(KeyCode::ArrowLeft);
        c.pitch = k(KeyCode::ArrowUp) - k(KeyCode::ArrowDown) + k(KeyCode::KeyR) - k(KeyCode::KeyF);
        c.roll = k(KeyCode::KeyE) - k(KeyCode::KeyQ);
        c.strafe_y = k(KeyCode::KeyZ) - k(KeyCode::KeyC) * 0.0;
        c.boost = keys.pressed(KeyCode::ShiftLeft);
        c.fire = keys.pressed(KeyCode::Space) || (mouse.pressed(MouseButton::Left) && !over_ui);
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

/// Discrete actions: targeting, missiles, docking, mining, scanning, investigating, windows.
#[allow(clippy::too_many_arguments)]
fn action_keys(keys: Res<ButtonInput<KeyCode>>, wants: Option<Res<EguiWantsInput>>, net: Option<Res<Net>>, mut s: ResMut<Session>, env: Res<Env>, time: Res<Time<Real>>) {
    let Some(net) = net else { return };
    if wants.as_ref().is_some_and(|w| w.wants_any_keyboard_input()) {
        return;
    }
    let now = time.elapsed_secs_f64();
    let pressed = |k: KeyCode| keys.just_pressed(k);
    // Windows.
    if pressed(KeyCode::KeyM) {
        s.ui.map = !s.ui.map;
    }
    if pressed(KeyCode::KeyU) {
        s.ui.station = !s.ui.station;
    }
    if pressed(KeyCode::KeyJ) {
        s.ui.missions = !s.ui.missions;
    }
    if pressed(KeyCode::KeyH) {
        s.ui.chronicle = !s.ui.chronicle;
        if s.ui.chronicle {
            net.send(ClientMsg::RequestChronicle);
        }
    }
    if pressed(KeyCode::KeyK) {
        s.ui.fleet = !s.ui.fleet;
    }
    if pressed(KeyCode::F1) {
        s.ui.help = !s.ui.help;
    }
    if s.dead.is_some() {
        if pressed(KeyCode::Enter) {
            net.send(ClientMsg::Respawn);
        }
        return;
    }
    let Some(me) = s.my_state().cloned() else { return };
    let my_pos = Vec3::from(me.pos);
    let fwd = Quat::from_array(me.rot) * Vec3::NEG_Z;
    let latest = s.latest().map(|l| l.ents.values().cloned().collect::<Vec<_>>()).unwrap_or_default();
    if pressed(KeyCode::KeyT) {
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
    if pressed(KeyCode::Escape) {
        s.target = None;
        net.send(ClientMsg::SetTarget(None));
    }
    if pressed(KeyCode::KeyX) {
        net.send(ClientMsg::FireMissile);
    }
    let Some(layout) = &env.layout else { return };
    if pressed(KeyCode::KeyG) {
        if me.docked {
            net.send(ClientMsg::Undock);
        } else if my_pos.distance(layout.station) < DOCK_RANGE + 80.0 {
            net.send(ClientMsg::Dock);
            s.ui.station = true;
        } else {
            s.toast(format!("정거장까지 {:.0}u — 더 접근하세요", my_pos.distance(layout.station) - DOCK_RANGE), now);
        }
    }
    if pressed(KeyCode::KeyN) {
        if me.mining.is_some() {
            net.send(ClientMsg::Mine { asteroid: None });
        } else {
            let a = layout.asteroids.iter().min_by(|a, b| a.pos.distance(my_pos).total_cmp(&b.pos.distance(my_pos)));
            match a {
                Some(a) if a.pos.distance(my_pos) < MINE_RANGE + a.radius => net.send(ClientMsg::Mine { asteroid: Some(a.id) }),
                Some(a) => s.toast(format!("가장 가까운 소행성까지 {:.0}u — 채굴 사거리 {:.0}u", a.pos.distance(my_pos), MINE_RANGE), now),
                None => s.toast("이 성계엔 소행성이 없습니다", now),
            }
        }
    }
    if pressed(KeyCode::KeyV) {
        // Scan the nearest point of interest, or investigate the nearest wreck if closer.
        let poi = layout.pois.iter().min_by(|a, b| a.pos.distance(my_pos).total_cmp(&b.pos.distance(my_pos)));
        let wreck = latest.iter().filter(|e| e.kind == EntityKind::Wreck).min_by(|a, b| Vec3::from(a.pos).distance(my_pos).total_cmp(&Vec3::from(b.pos).distance(my_pos)));
        let wd = wreck.map(|w| Vec3::from(w.pos).distance(my_pos)).unwrap_or(f32::MAX);
        if wd < 400.0 {
            net.send(ClientMsg::InvestigateWreck { wreck: wreck.unwrap().id });
        } else if let Some(p) = poi {
            net.send(ClientMsg::Scan { poi: p.id });
        }
    }
}

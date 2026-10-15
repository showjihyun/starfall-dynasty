//! Space dust: faint motes fixed in space around the camera that streak along the ship's velocity,
//! so flight reads as motion even with nothing else nearby. The field wraps around the camera and
//! scales with the flagship (a fighter's dust is a few hundred metres across, a capital's kilometres).
//! All motes share one mesh and material, so they draw as a single instanced batch.

use crate::control::MainCamera;
use crate::state::{Screen, Session};
use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::prelude::*;

const COUNT: usize = 320;

pub struct DustPlugin;

impl Plugin for DustPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup).add_systems(Update, drift.run_if(in_state(Screen::Playing)));
    }
}

#[derive(Component)]
struct Mote;

fn setup(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut mats: ResMut<Assets<StandardMaterial>>) {
    let mesh = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let mat = mats.add(StandardMaterial { base_color: Color::linear_rgb(0.55, 0.65, 0.85), unlit: true, alpha_mode: AlphaMode::Add, fog_enabled: false, ..default() });
    let mut seed = 0x2545_f491_u32;
    let mut rnd = move || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        seed as f32 / u32::MAX as f32 - 0.5
    };
    for _ in 0..COUNT {
        // Unit-cube positions; `drift` maps them into the camera's box.
        commands.spawn((Mote, NotShadowCaster, NotShadowReceiver, Mesh3d(mesh.clone()), MeshMaterial3d(mat.clone()), Transform::from_xyz(rnd(), rnd(), rnd()), Visibility::Hidden));
    }
}

fn drift(s: Res<Session>, cam: Query<&GlobalTransform, With<MainCamera>>, mut motes: Query<(&mut Transform, &mut Visibility), With<Mote>>, mut last_box: Local<f32>) {
    // `last_box` is 0 until the field has been laid out around the camera (first visible frame).
    let Ok(c) = cam.single() else { return };
    let me = s.my_state();
    let show = me.is_some_and(|m| !m.docked);
    let radius = me.map(|m| m.radius).unwrap_or(10.0);
    let vel = me.map(|m| Vec3::from(m.vel)).unwrap_or(Vec3::ZERO);
    let speed = vel.length();
    let size = (radius * 45.0).clamp(380.0, 9_000.0);
    let half = size * 0.5;
    let width = size * 0.0005;
    // Streak length grows with speed relative to the ship's own size.
    let len = width * (1.0 + (speed / (radius * 0.6)).min(30.0));
    let dir = if speed > 1.0 { vel / speed } else { Vec3::Z };
    let rot = Quat::from_rotation_arc(Vec3::Z, dir);
    let cp = c.translation();
    // The field was laid out for another ship size: rescale it about the camera.
    let first = *last_box == 0.0;
    let rescale = if !first && (*last_box - size).abs() > 1.0 { size / *last_box } else { 1.0 };
    if show {
        *last_box = size;
    }
    for (mut t, mut v) in &mut motes {
        let want = if show { Visibility::Inherited } else { Visibility::Hidden };
        if *v != want {
            *v = want;
        }
        if !show {
            continue;
        }
        let mut rel = if first { t.translation * size } else { (t.translation - cp) * rescale };
        rel = Vec3::new((rel.x + half).rem_euclid(size) - half, (rel.y + half).rem_euclid(size) - half, (rel.z + half).rem_euclid(size) - half);
        t.translation = cp + rel;
        // Shrink toward the box edges so wrapping never pops, and right at the camera.
        let r = rel.length() / half;
        let k = (1.0 - r * r).clamp(0.0, 1.0) * ((r - 0.06) * 4.0).clamp(0.0, 1.0);
        t.rotation = rot;
        t.scale = Vec3::new(width * k, width * k, len * k);
    }
}

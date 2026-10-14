//! Transient effects driven by server `Fx` events: lasers, explosions, shield hits,
//! warp flashes; plus continuous mining beams and the warp tunnel.

use crate::materials::*;
use crate::meshes;
use crate::scene::Env;
use crate::ships::{ShipVis, Visuals};
use crate::state::{Screen, Session};
use bevy::light::NotShadowCaster;
use bevy::prelude::*;
use crate::audio::{Sfx, SfxQueue};
use shared::protocol::{EntityKind, Fx};
use shared::ships::WeaponKind;

pub struct FxPlugin;

impl Plugin for FxPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup).add_systems(Update, (spawn_fx, tick_fx, mining_beams, warp_tunnel).run_if(in_state(Screen::Playing)));
    }
}

#[derive(Resource)]
struct FxAssets {
    beam: Handle<Mesh>,
    sphere: Handle<Mesh>,
    tunnel: Handle<Mesh>,
}

#[derive(Component)]
struct Ttl {
    left: f32,
    total: f32,
}

#[derive(Component)]
struct BeamFx(Handle<BeamMat>, f32);

#[derive(Component)]
struct Boom {
    mat: Handle<ExplosionMat>,
    size: f32,
}

#[derive(Component)]
struct FlashLight(f32);

#[derive(Component)]
struct GlowFx(Handle<GlowMat>, f32);

#[derive(Component)]
struct MiningBeam(u32, Handle<BeamMat>);

#[derive(Component)]
struct Tunnel(Handle<WarpMat>);

fn setup(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>) {
    commands.insert_resource(FxAssets { beam: meshes.add(meshes::beam()), sphere: meshes.add(Sphere::new(1.0).mesh().ico(4).unwrap()), tunnel: meshes.add(meshes::tunnel()) });
}

#[allow(clippy::too_many_arguments)]
fn spawn_fx(
    mut commands: Commands,
    mut s: ResMut<Session>,
    vis: Res<Visuals>,
    assets: Res<FxAssets>,
    mut ships: Query<(&mut ShipVis, &Transform)>,
    gt: Query<&GlobalTransform>,
    mut beams: ResMut<Assets<BeamMat>>,
    mut booms: ResMut<Assets<ExplosionMat>>,
    mut glows: ResMut<Assets<GlowMat>>,
    mut muzzle_toggle: Local<usize>,
    mut sfx: ResMut<SfxQueue>,
    time: Res<Time<Real>>,
) {
    let now = time.elapsed_secs_f64();
    let fx: Vec<Fx> = std::mem::take(&mut s.fx);
    for f in fx {
        match f {
            Fx::Shot { from, to, weapon, mount, hit } => {
                let Some(&fe) = vis.map.get(&from) else { continue };
                let Ok((sv, stf)) = ships.get(fe) else { continue };
                // Colour by relation to me: friends blue/amber, enemies red.
                let me = s.latest().and_then(|l| l.ents.get(&s.me.unwrap_or(0))).cloned();
                let shooter = s.latest().and_then(|l| l.ents.get(&from)).cloned();
                let friendly = match (&me, &shooter) {
                    (Some(m), Some(sh)) => sh.owner.is_some() && (sh.owner == m.owner || (m.fleet.is_some() && sh.fleet == m.fleet) || (m.alliance.is_some() && sh.alliance == m.alliance)),
                    _ => !matches!(sv.kind, EntityKind::Npc(_)),
                };
                let target_pos = vis.map.get(&to).and_then(|e| ships.get(*e).ok()).map(|(_, t)| t.translation);
                let fwd = stf.rotation * Vec3::NEG_Z;
                let pick = |list: &Vec<Entity>, k: usize| list.get(k % list.len().max(1)).and_then(|e| gt.get(*e).ok()).map(|g| g.translation());
                match weapon {
                    WeaponKind::Laser => {
                        *muzzle_toggle += 1;
                        let start = pick(&sv.muzzles, *muzzle_toggle).unwrap_or(stf.translation + fwd * sv.radius);
                        let end = target_pos.unwrap_or(start + fwd * 650.0);
                        let col = if friendly { [0.25, 0.75, 1.0] } else { [1.0, 0.25, 0.12] };
                        spawn_beam(&mut commands, &assets, &mut beams, start, end, col, 1.8, 0.14);
                        if hit && to != 0 {
                            spawn_boom(&mut commands, &assets, &mut booms, end + (start - end).normalize_or_zero() * 4.0, 0.15, 0.35);
                        }
                        sfx.play(Sfx::Laser, Some(start));
                    }
                    WeaponKind::Pulse => {
                        let k = mount as usize;
                        let start = pick(&sv.turret_muzzles, k).or_else(|| pick(&sv.muzzles, k)).unwrap_or_else(|| {
                            // No turret nodes: spread shots along the hull.
                            let a = k as f32 * 2.399;
                            stf.translation + stf.rotation * Vec3::new(a.cos() * sv.radius * 0.25, sv.radius * 0.12, (k as f32 / 8.0 - 0.5) * sv.radius * 1.4)
                        });
                        let Some(end) = target_pos else { continue };
                        let col = if friendly { [1.0, 0.75, 0.3] } else { [1.0, 0.3, 0.15] };
                        let w = (sv.radius * 0.012).clamp(2.0, 6.0);
                        spawn_beam(&mut commands, &assets, &mut beams, start, end, col, w, 0.12);
                        spawn_boom(&mut commands, &assets, &mut booms, end + (start - end).normalize_or_zero() * 6.0, 0.25, 0.4);
                        sfx.play(Sfx::Pulse, Some(start));
                        if let Ok((mut svm, _)) = ships.get_mut(fe) {
                            svm.aim = Some((to, now));
                        }
                    }
                    WeaponKind::MainBeam => {
                        // Every bow port fires together: a thick neutron-beam volley.
                        let ports: Vec<Vec3> = if sv.mains.is_empty() { vec![stf.translation + fwd * sv.radius * 1.05] } else { sv.mains.iter().filter_map(|e| gt.get(*e).ok()).map(|g| g.translation()).collect() };
                        let reach = sv.radius * 2.2 * 1.8 + 1500.0;
                        let w = (sv.radius * 0.05).clamp(5.0, 26.0);
                        for (i, p) in ports.iter().enumerate() {
                            let end = target_pos.map(|t| t + (*p - ports[0]) * 0.3).unwrap_or(*p + fwd * reach);
                            let col = if friendly { [0.55, 0.8, 1.0] } else { [1.0, 0.45, 0.3] };
                            spawn_beam(&mut commands, &assets, &mut beams, *p, end, col, w, 0.75);
                            spawn_boom(&mut commands, &assets, &mut booms, *p + fwd * w, w * 0.04, 0.5);
                            if hit && i == 0 {
                                spawn_boom(&mut commands, &assets, &mut booms, end, (w * 0.12).max(1.0), 1.4);
                            }
                        }
                        sfx.play(Sfx::MainBeam, Some(stf.translation));
                    }
                    WeaponKind::Missile => sfx.play(Sfx::Missile, Some(stf.translation)),
                }
            }
            Fx::Explosion { pos, size } => {
                let p = Vec3::from(pos);
                sfx.play(if size >= 3.0 { Sfx::ExplosionBig } else { Sfx::ExplosionSmall }, Some(p));
                spawn_boom(&mut commands, &assets, &mut booms, p, size, 1.6);
                commands.spawn((Ttl { left: 0.9, total: 0.9 }, FlashLight(4.0e9 * size), PointLight { color: Color::srgb(1.0, 0.7, 0.4), intensity: 4.0e9 * size, range: 900.0 * size.sqrt(), ..default() }, Transform::from_translation(p)));
                // Debris ring of smaller blasts.
                for k in 0..(3 + size as usize) {
                    let a = k as f32 * 2.4;
                    let off = Vec3::new(a.cos(), (a * 1.7).sin() * 0.5, a.sin()) * 10.0 * size;
                    spawn_boom(&mut commands, &assets, &mut booms, p + off, size * 0.35, 1.0 + k as f32 * 0.1);
                }
            }
            Fx::ShieldHit { entity, dir } => {
                if let Some(e) = vis.map.get(&entity)
                    && let Ok((mut sv, _)) = ships.get_mut(*e)
                {
                    sv.hit_dir = Vec3::from(dir);
                    sv.hit_age = 0.0;
                    sfx.play(Sfx::ShieldHit, ships.get(*e).ok().map(|(_, t)| t.translation));
                }
            }
            Fx::WarpOut { entity } | Fx::WarpIn { entity } => {
                let Some(e) = vis.map.get(&entity) else { continue };
                let Ok((sv, t)) = ships.get(*e) else { continue };
                sfx.play(Sfx::WarpJump, Some(t.translation));
                let r = sv.radius * 6.0;
                let m = glows.add(GlowMat { p: GlowParams { color: Vec4::new(0.5, 0.75, 1.0, 8.0), center: t.translation.extend(sv.radius), shape: Vec4::new(2.5, 0.0, r, 0.0) } });
                commands.spawn((Ttl { left: 0.9, total: 0.9 }, GlowFx(m.clone(), sv.radius), NotShadowCaster, Mesh3d(assets.sphere.clone()), MeshMaterial3d(m), Transform::from_translation(t.translation).with_scale(Vec3::splat(r))));
            }
            Fx::MissileLaunch { from, .. } => {
                let at = vis.map.get(&from).and_then(|e| ships.get(*e).ok()).map(|(_, t)| t.translation);
                sfx.play(Sfx::Missile, at);
            }
        }
    }
    // Hull hits (no shield left) flash the hull.
    if let Some(latest) = s.latest() {
        for (mut sv, _) in &mut ships {
            if let Some(st) = latest.ents.get(&sv.id)
                && sv.hit_age < 0.05
                && st.shield < 0.02
            {
                sv.hit_flash = 1.0;
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn spawn_beam(commands: &mut Commands, assets: &FxAssets, beams: &mut Assets<BeamMat>, start: Vec3, end: Vec3, col: [f32; 3], width: f32, ttl: f32) {
    let d = end - start;
    let len = d.length().max(1.0);
    let m = beams.add(BeamMat { p: BeamParams { color: v4(col, 1.0), flow: Vec4::new(0.08, 30.0, len, 0.0) } });
    commands.spawn((
        Ttl { left: ttl, total: ttl },
        BeamFx(m.clone(), 1.0),
        NotShadowCaster,
        Mesh3d(assets.beam.clone()),
        MeshMaterial3d(m),
        Transform::from_translation(start).with_rotation(Quat::from_rotation_arc(Vec3::Z, d / len)).with_scale(Vec3::new(width, width, len)),
    ));
}

fn spawn_boom(commands: &mut Commands, assets: &FxAssets, booms: &mut Assets<ExplosionMat>, p: Vec3, size: f32, ttl: f32) {
    let m = booms.add(ExplosionMat { p: ExplosionParams { center: p.extend(1.0), state: Vec4::new(0.0, p.x * 0.013 + p.z * 0.007, 1.0, 0.0) } });
    commands.spawn((Ttl { left: ttl, total: ttl }, Boom { mat: m.clone(), size }, NotShadowCaster, Mesh3d(assets.sphere.clone()), MeshMaterial3d(m), Transform::from_translation(p).with_scale(Vec3::splat(1.0))));
}

#[allow(clippy::type_complexity)]
fn tick_fx(
    mut commands: Commands,
    time: Res<Time>,
    mut q: Query<(Entity, &mut Ttl, Option<&BeamFx>, Option<&Boom>, Option<&FlashLight>, Option<&GlowFx>, &mut Transform, Option<&mut PointLight>)>,
    mut beams: ResMut<Assets<BeamMat>>,
    mut booms: ResMut<Assets<ExplosionMat>>,
    mut glows: ResMut<Assets<GlowMat>>,
) {
    let dt = time.delta_secs();
    for (e, mut ttl, beam, boom, flash, glow, mut tf, light) in &mut q {
        ttl.left -= dt;
        if ttl.left <= 0.0 {
            commands.entity(e).despawn();
            continue;
        }
        let age = 1.0 - ttl.left / ttl.total;
        if let Some(b) = beam
            && let Some(mut m) = beams.get_mut(&b.0)
        {
            m.p.color.w = (1.0 - age).powf(1.5) * b.1;
        }
        if let Some(b) = boom {
            let r = b.size * (6.0 + 30.0 * (1.0 - (1.0 - age).powf(3.0)));
            tf.scale = Vec3::splat(r);
            if let Some(mut m) = booms.get_mut(&b.mat) {
                m.p.center = tf.translation.extend(r);
                m.p.state.x = age;
                m.p.state.z = 1.2;
            }
        }
        if let (Some(f), Some(mut l)) = (flash, light) {
            l.intensity = f.0 * (1.0 - age).powf(2.0);
        }
        if let Some(g) = glow
            && let Some(mut m) = glows.get_mut(&g.0)
        {
            m.p.color.w = 8.0 * (1.0 - age);
            m.p.center.w = g.1 * (1.0 + age * 2.0);
        }
    }
}

fn mining_beams(
    mut commands: Commands,
    s: Res<Session>,
    env: Res<Env>,
    vis: Res<Visuals>,
    assets: Res<FxAssets>,
    ships: Query<(&ShipVis, &Transform), Without<MiningBeam>>,
    mut existing: Query<(Entity, &MiningBeam, &mut Transform)>,
    mut beams: ResMut<Assets<BeamMat>>,
) {
    let Some(latest) = s.latest() else { return };
    let Some(layout) = &env.layout else { return };
    let mut alive = vec![];
    for (id, st) in &latest.ents {
        let Some(aid) = st.mining else { continue };
        let Some(a) = layout.asteroids.iter().find(|a| a.id == aid) else { continue };
        let Some((sv, tf)) = vis.map.get(id).and_then(|e| ships.get(*e).ok()) else { continue };
        let start = tf.translation + tf.rotation * Vec3::NEG_Z * sv.radius * 0.8;
        let d = a.pos - start;
        let len = d.length().max(1.0);
        let t = Transform::from_translation(start).with_rotation(Quat::from_rotation_arc(Vec3::Z, d / len)).with_scale(Vec3::new(2.5, 2.5, len - a.radius * 0.6));
        alive.push(*id);
        if let Some((_, _, mut et)) = existing.iter_mut().find(|(_, mb, _)| mb.0 == *id) {
            *et = t;
        } else {
            let m = beams.add(BeamMat { p: BeamParams { color: Vec4::new(0.3, 1.0, 0.5, 0.8), flow: Vec4::new(0.05, 12.0, len, 0.0) } });
            commands.spawn((MiningBeam(*id, m.clone()), NotShadowCaster, Mesh3d(assets.beam.clone()), MeshMaterial3d(m), t));
        }
    }
    for (e, mb, _) in &existing {
        if !alive.contains(&mb.0) {
            commands.entity(e).despawn();
        }
        let _ = &mb.1;
    }
}

fn warp_tunnel(
    mut commands: Commands,
    s: Res<Session>,
    time: Res<Time<Real>>,
    assets: Res<FxAssets>,
    cam: Query<Entity, With<Camera3d>>,
    mut tunnel: Query<(Entity, &Tunnel)>,
    mut warps: ResMut<Assets<WarpMat>>,
    mut after: Local<f32>,
    mut last_epoch: Local<u32>,
    mut was_charging: Local<bool>,
    mut sfx: ResMut<SfxQueue>,
) {
    if s.warp.is_some() && !*was_charging {
        sfx.play(Sfx::WarpCharge, None);
    }
    *was_charging = s.warp.is_some();
    let now = time.elapsed_secs_f64();
    let charge = s.warp.map(|(_, end, total)| (1.0 - ((end - now) as f32 / total)).clamp(0.0, 1.0));
    if *last_epoch != s.system_epoch {
        *last_epoch = s.system_epoch;
        *after = 1.2;
    }
    *after = (*after - time.delta_secs()).max(0.0);
    let k = match charge {
        Some(c) => c * c,
        None => *after / 1.2,
    };
    if k <= 0.001 {
        for (e, _) in &tunnel {
            commands.entity(e).despawn();
        }
        return;
    }
    if let Ok((_, t)) = tunnel.single_mut() {
        if let Some(mut m) = warps.get_mut(&t.0) {
            m.p.color.w = k;
        }
    } else if let Ok(c) = cam.single() {
        let m = warps.add(WarpMat { p: WarpParams { color: Vec4::new(0.55, 0.75, 1.0, k), accent: Vec4::new(0.35, 0.2, 0.9, 1.0) } });
        commands.entity(c).with_children(|p| {
            p.spawn((Tunnel(m.clone()), NotShadowCaster, Mesh3d(assets.tunnel.clone()), MeshMaterial3d(m), Transform::from_xyz(0.0, 0.0, -300.0).with_scale(Vec3::new(60.0, 60.0, 900.0))));
        });
    }
}

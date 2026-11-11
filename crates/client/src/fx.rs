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
use shared::protocol::{EntityId, Fx};
use shared::ships::WeaponKind;
use std::collections::HashMap;

pub struct FxPlugin;

impl Plugin for FxPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup).add_systems(Update, (spawn_fx, tick_fx, mining_beams, warp_tunnel).run_if(in_state(Screen::Playing)));
        // Outside the Update graph: a new Update system reorders it and moves the art-check scenes a frame.
        app.add_systems(PostUpdate, warp_spool.before(bevy::transform::TransformSystems::Propagate).run_if(in_state(Screen::Playing)));
    }
}

#[derive(Resource)]
struct FxAssets {
    beam: Handle<Mesh>,
    sphere: Handle<Mesh>,
    tunnel: Handle<Mesh>,
}

/// Lifetime on the shader clock (`Time`, what the shaders see as `globals.time`): age is
/// `(now - born) / total`, never a per-frame countdown (docs/ART_DIRECTION.md §8).
#[derive(Component)]
struct Ttl {
    born: f64,
    total: f32,
}

#[derive(Component)]
struct BeamFx(Handle<BeamMat>);

/// Expired beams and fireballs are hidden and reused instead of despawned, so a big battle
/// doesn't allocate entities, materials and bind groups for every shot.
#[derive(Resource, Default)]
struct FxPool {
    beams: Vec<(Entity, Handle<BeamMat>)>,
    booms: Vec<(Entity, Handle<ExplosionMat>)>,
}

#[derive(Component)]
struct Boom {
    mat: Handle<ExplosionMat>,
    size: f32,
}

#[derive(Component)]
struct FlashLight(f32);

/// At most this many explosion flash lights are live at once.
const MAX_FLASHES: usize = 6;
/// At most this many debris blasts ring a dying ship.
const MAX_DEBRIS: usize = 8;

/// How far a fireball of `size` swells (times its size; size ≈ hull radius / 10): a fighter's to
/// about 3.6 hull radii as before, a titan's only to about 1.3, so it doesn't fill the screen.
/// Hit sparks (size < 1) are unchanged.
fn boom_growth(size: f32) -> f32 {
    30.0 / size.max(1.0).sqrt()
}

#[derive(Component)]
struct GlowFx(Handle<GlowMat>, f32);

#[derive(Component)]
struct MiningBeam(u32, Handle<BeamMat>);

#[derive(Component)]
struct Tunnel(Handle<WarpMat>);

fn setup(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>) {
    commands.init_resource::<FxPool>();
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
    shader_time: Res<Time>,
    mut pool: ResMut<FxPool>,
    flashes: Query<(), With<FlashLight>>,
) {
    let _prof = shared::prof::span("fx::spawn_fx");
    let mut lit = flashes.iter().count();
    let now = time.elapsed_secs_f64();
    // The clock the shaders see as `globals.time`.
    let t = shader_time.elapsed_secs_wrapped();
    let born = shader_time.elapsed_secs_f64();
    let fx: Vec<Fx> = std::mem::take(&mut s.fx);
    for f in fx {
        match f {
            Fx::Shot { from, to, weapon, mount, hit } => {
                if s.me.is_some_and(|m| m == from || m == to) {
                    s.last_combat = now;
                }
                let Some(&fe) = vis.map.get(&from) else { continue };
                let Ok((sv, stf)) = ships.get(fe) else { continue };
                // Shots take the shooter's side colours (palette::side): whoever is hostile to me fires red.
                let side = view::palette::side::of(sv.hostile);
                let target = vis.map.get(&to).and_then(|e| ships.get(*e).ok()).map(|(v, t)| (t.translation, t.rotation, v.radius, v.hull_half));
                let target_pos = target.map(|t| t.0);
                // Where a shot from `start` lands: on the near side of the target's hull, scattered — the same
                // shot (shooter, mount, moment) always in the same place (view::anim::impact_point).
                let seed = from.wrapping_mul(31) ^ ((mount as u32) << 8) ^ ((born * 8.0) as u32).wrapping_mul(2_654_435_761);
                let strike = |start: Vec3| target.map(|(p, rot, _, half)| p + rot * view::anim::impact_point(rot.inverse() * (start - p), half, seed));
                // A flash at the muzzle, for the ships close enough to show it.
                let near = sv.lod == view::anim::EngineLod::Full;
                let fwd = stf.rotation * Vec3::NEG_Z;
                let pick = |list: &Vec<Entity>, k: usize| list.get(k % list.len().max(1)).and_then(|e| gt.get(*e).ok()).map(|g| g.translation());
                match weapon {
                    WeaponKind::Laser => {
                        *muzzle_toggle += 1;
                        let start = pick(&sv.muzzles, *muzzle_toggle).unwrap_or(stf.translation + fwd * sv.radius);
                        let end = strike(start).unwrap_or(start + fwd * 650.0);
                        let col = side.laser;
                        spawn_beam(&mut commands, &assets, &mut beams, &mut pool, (t, born), start, end, col, 1.8, 0.14);
                        if near {
                            spawn_boom(&mut commands, &assets, &mut booms, &mut pool, (t, born), start, 0.06, 0.18);
                        }
                        if hit && to != 0 {
                            spawn_boom(&mut commands, &assets, &mut booms, &mut pool, (t, born), end + (start - end).normalize_or_zero() * 4.0, 0.15, 0.35);
                        }
                        sfx.play(Sfx::Laser, Some(start));
                    }
                    WeaponKind::Pulse => {
                        let k = mount as usize;
                        // Firing arcs (server: even batteries dorsal, odd ventral): fire from a muzzle on
                        // that side of the hull, so shots never come through the ship.
                        let dorsal = k % 2 == 0;
                        let on_side: Vec<Entity> = sv.turret_muzzles.iter().copied().filter(|m| gt.get(*m).is_ok_and(|g| ((stf.rotation.inverse() * (g.translation() - stf.translation)).y >= 0.0) == dorsal)).collect();
                        let list = if on_side.is_empty() { &sv.turret_muzzles } else { &on_side };
                        // No turret nodes (the leviathans): fire from the side of the hull that battery covers
                        // (beam mounts on the flanks), not from the bow guns.
                        let mount_at = || {
                            let st = s.latest().and_then(|sn| sn.ents.get(&from));
                            let n = st.map_or(0, shared::ships::turret_batteries) as usize;
                            let beam = st.map_or(0, shared::ships::beam_batteries);
                            let tx = target_pos.map_or(0.0, |tp| (stf.rotation.inverse() * (tp - stf.translation)).x);
                            stf.translation + stf.rotation * view::anim::battery_mount(k, n.max(k + 1), beam, sv.radius, tx)
                        };
                        let start = pick(list, k / 2).unwrap_or_else(mount_at);
                        let Some(end) = strike(start) else { continue };
                        let col = side.pulse;
                        let w = (sv.radius * 0.012).clamp(2.0, 6.0);
                        // Turret fire is bolts in flight, not instant lines: a broadside reads as streams of shot.
                        spawn_shot(&mut commands, &assets, &mut beams, &mut pool, (t, born), start, end, col, w, view::anim::BOLT_SECS, true);
                        if near {
                            spawn_boom(&mut commands, &assets, &mut booms, &mut pool, (t, born), start, w * 0.04, 0.2);
                        }
                        // Flak: some of a close ship's shots burst short of the hull they are aimed at.
                        if (near || target.is_some_and(|_| vis.map.get(&to).and_then(|e| ships.get(*e).ok()).is_some_and(|(v, _)| v.lod == view::anim::EngineLod::Full)))
                            && let (Some((short, side)), Some((_, _, r, _))) = (view::anim::flak_burst(seed), target)
                        {
                            let back = (start - end).normalize_or_zero();
                            let across = back.cross(Vec3::Y).normalize_or(Vec3::X);
                            let at = end + (back * short + across * side[0] + back.cross(across) * side[1]) * r;
                            spawn_boom(&mut commands, &assets, &mut booms, &mut pool, (t, born), at, (r * 0.0012).clamp(0.12, 0.5), 0.55);
                        }
                        spawn_boom(&mut commands, &assets, &mut booms, &mut pool, (t, born), end + (start - end).normalize_or_zero() * 6.0, 0.25, 0.4);
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
                            let col = side.beam;
                            spawn_beam(&mut commands, &assets, &mut beams, &mut pool, (t, born), *p, end, col, w, 0.75);
                            spawn_boom(&mut commands, &assets, &mut booms, &mut pool, (t, born), *p + fwd * w, w * 0.04, 0.5);
                            if hit && i == 0 {
                                spawn_boom(&mut commands, &assets, &mut booms, &mut pool, (t, born), end, (w * 0.12).max(1.0), 1.4);
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
                spawn_boom(&mut commands, &assets, &mut booms, &mut pool, (t, born), p, size, 1.6);
                // A few lit blasts at a time: in a mass battle dozens of these lights at once wash every hull white.
                if lit < MAX_FLASHES {
                    lit += 1;
                    commands.spawn((Ttl { born, total: 0.9 }, FlashLight(4.0e9 * size), PointLight { color: Color::srgb(1.0, 0.7, 0.4), intensity: 4.0e9 * size, range: 900.0 * size.sqrt(), ..default() }, Transform::from_translation(p)));
                }
                // Debris ring of smaller blasts.
                for k in 0..(3 + size as usize).min(MAX_DEBRIS) {
                    let a = k as f32 * 2.4;
                    let off = Vec3::new(a.cos(), (a * 1.7).sin() * 0.5, a.sin()) * 10.0 * size;
                    spawn_boom(&mut commands, &assets, &mut booms, &mut pool, (t, born), p + off, size * 0.35, 1.0 + k as f32 * 0.1);
                }
            }
            Fx::ShieldHit { entity, dir } => {
                if let Some(e) = vis.map.get(&entity)
                    && let Ok((mut sv, _)) = ships.get_mut(*e)
                {
                    sv.hit_dir = Vec3::from(dir);
                    sv.hit_at = now;
                    sfx.play(Sfx::ShieldHit, ships.get(*e).ok().map(|(_, t)| t.translation));
                }
            }
            // Jumping away: the ship is already gone, the event says where it was. A flash, and a streak
            // shooting off ahead of it the way it went.
            Fx::WarpOut { entity, pos } => {
                // Heading and size from the last snapshot that still had it.
                let last = s.snaps.iter().rev().find_map(|sn| sn.ents.get(&entity));
                let d = last.map_or(Vec3::NEG_Z, |e| Quat::from_array(e.rot) * Vec3::NEG_Z);
                let radius = last.map_or(12.0, |e| e.radius);
                let p = Vec3::from(pos);
                sfx.play(Sfx::WarpJump, Some(p));
                warp_flash(&mut commands, &assets, &mut glows, born, p, radius);
                spawn_beam(&mut commands, &assets, &mut beams, &mut pool, (t, born), p, p + d * warp_streak_len(radius), view::palette::WARP, (radius * 0.5).max(3.0), 0.35);
            }
            // Dropping in: the ship is drawn stretched along its way while it settles (ships.rs), with a
            // flash where it appears and a streak trailing back the way it came.
            Fx::WarpIn { entity, dir } => {
                s.arrivals.insert(entity, now);
                let Some(st) = s.latest().and_then(|sn| sn.ents.get(&entity)) else { continue };
                let (p, d, radius) = (Vec3::from(st.pos), Vec3::from(dir).normalize_or(Vec3::NEG_Z), st.radius);
                sfx.play(Sfx::WarpJump, Some(p));
                warp_flash(&mut commands, &assets, &mut glows, born, p, radius);
                spawn_beam(&mut commands, &assets, &mut beams, &mut pool, (t, born), p - d * warp_streak_len(radius), p, view::palette::WARP, (radius * 0.5).max(3.0), 0.45);
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
                && sv.hit_at == now
                && st.shield < 0.02
            {
                sv.flash_at = now;
            }
        }
    }
}

/// The streak a jumping ship leaves: long for a fighter against its size, a few hulls for a capital.
fn warp_streak_len(radius: f32) -> f32 {
    300.0 + radius * 25.0
}

/// The blue-white flash of a ship entering or leaving warp (fades and swells over 0.9 s, `tick_fx`).
fn warp_flash(commands: &mut Commands, assets: &FxAssets, glows: &mut Assets<GlowMat>, born: f64, p: Vec3, radius: f32) {
    let r = radius * 6.0;
    let c = view::palette::WARP;
    let m = glows.add(GlowMat { p: GlowParams { color: Vec4::new(c[0], c[1], c[2], 8.0), center: p.extend(radius), shape: Vec4::new(2.5, 0.0, r, 0.0) } });
    commands.spawn((Ttl { born, total: 0.9 }, GlowFx(m.clone(), radius), NotShadowCaster, Mesh3d(assets.sphere.clone()), MeshMaterial3d(m), Transform::from_translation(p).with_scale(Vec3::splat(r))));
}

/// A charging warp drive: a glow round the ship that builds and beats faster towards the jump
/// (`view::anim::warp_spool`), one per charging ship, gone when it jumps or the charge breaks.
#[allow(clippy::type_complexity)]
fn warp_spool(
    mut commands: Commands,
    mut s: ResMut<Session>,
    time: Res<Time<Real>>,
    assets: Res<FxAssets>,
    ships: Query<(&ShipVis, &Transform)>,
    mut glows: ResMut<Assets<GlowMat>>,
    mut live: Local<HashMap<EntityId, (Entity, Handle<GlowMat>)>>,
) {
    let _prof = shared::prof::span("fx::warp_spool");
    let now = time.elapsed_secs_f64();
    // Arrival stretches last under a second.
    s.arrivals.retain(|_, at| now - *at < 3.0);
    let charge = shared::tuning::get().combat.warp_charge;
    let mut seen = vec![];
    for (sv, tf) in &ships {
        let Some(at) = sv.warp_since else { continue };
        seen.push(sv.id);
        let k = view::anim::warp_spool(now - at, charge);
        let r = sv.radius * (2.0 + 2.5 * k);
        let c = view::palette::WARP;
        let params = GlowParams { color: Vec4::new(c[0], c[1], c[2], 6.0 * k), center: tf.translation.extend(sv.radius), shape: Vec4::new(2.5, 0.0, r, 0.0) };
        match live.get(&sv.id) {
            Some((e, h)) => {
                if let Some(mut m) = glows.get_mut(h) {
                    m.p = params;
                }
                commands.entity(*e).insert(Transform::from_translation(tf.translation).with_scale(Vec3::splat(r)));
            }
            None => {
                let h = glows.add(GlowMat { p: params });
                let e = commands.spawn((NotShadowCaster, Mesh3d(assets.sphere.clone()), MeshMaterial3d(h.clone()), Transform::from_translation(tf.translation).with_scale(Vec3::splat(r)))).id();
                live.insert(sv.id, (e, h));
            }
        }
    }
    live.retain(|id, (e, _)| {
        let keep = seen.contains(id);
        if !keep {
            commands.entity(*e).despawn();
        }
        keep
    });
}

#[allow(clippy::too_many_arguments)]
fn spawn_beam(commands: &mut Commands, assets: &FxAssets, beams: &mut Assets<BeamMat>, pool: &mut FxPool, clock: (f32, f64), start: Vec3, end: Vec3, col: [f32; 3], width: f32, ttl: f32) {
    spawn_shot(commands, assets, beams, pool, clock, start, end, col, width, ttl, false);
}

/// A beam, or (`bolt`) a bright streak that flies from `start` to `end` over `ttl` (beam.wesl).
#[allow(clippy::too_many_arguments)]
fn spawn_shot(commands: &mut Commands, assets: &FxAssets, beams: &mut Assets<BeamMat>, pool: &mut FxPool, clock: (f32, f64), start: Vec3, end: Vec3, col: [f32; 3], width: f32, ttl: f32, bolt: bool) {
    let d = end - start;
    let len = d.length().max(1.0);
    let tail = if bolt { view::anim::bolt_tail(len) } else { 0.0 };
    let params = BeamParams { color: v4(col, 1.0), flow: Vec4::new(0.08, 30.0, len, 0.0), life: Vec4::new(clock.0, ttl, tail, 0.0) };
    let tf = Transform::from_translation(start).with_rotation(Quat::from_rotation_arc(Vec3::Z, d / len)).with_scale(Vec3::new(width, width, len));
    let life = Ttl { born: clock.1, total: ttl };
    if let Some((e, h)) = pool.beams.pop() {
        if let Some(mut m) = beams.get_mut(&h) {
            m.p = params;
        }
        commands.entity(e).insert((life, tf, Visibility::Inherited));
        return;
    }
    let m = beams.add(BeamMat { p: params });
    commands.spawn((life, BeamFx(m.clone()), NotShadowCaster, Mesh3d(assets.beam.clone()), MeshMaterial3d(m), tf));
}

#[allow(clippy::too_many_arguments)]
fn spawn_boom(commands: &mut Commands, assets: &FxAssets, booms: &mut Assets<ExplosionMat>, pool: &mut FxPool, clock: (f32, f64), p: Vec3, size: f32, ttl: f32) {
    let params = ExplosionParams { life: Vec4::new(clock.0, ttl, 0.0, 0.0), state: Vec4::new(0.0, p.x * 0.013 + p.z * 0.007, 1.2, 0.0) };
    let tf = Transform::from_translation(p).with_scale(Vec3::splat(size * 6.0));
    let life = Ttl { born: clock.1, total: ttl };
    if let Some((e, h)) = pool.booms.pop() {
        if let Some(mut m) = booms.get_mut(&h) {
            m.p = params;
        }
        commands.entity(e).insert((life, Boom { mat: h, size }, tf, Visibility::Inherited));
        return;
    }
    let m = booms.add(ExplosionMat { p: params });
    commands.spawn((life, Boom { mat: m.clone(), size }, NotShadowCaster, Mesh3d(assets.sphere.clone()), MeshMaterial3d(m), tf));
}

#[allow(clippy::type_complexity)]
fn tick_fx(
    mut commands: Commands,
    time: Res<Time>,
    mut q: Query<(Entity, &Ttl, Option<&BeamFx>, Option<&Boom>, Option<&FlashLight>, Option<&GlowFx>, &mut Transform, Option<&mut PointLight>)>,
    mut glows: ResMut<Assets<GlowMat>>,
    mut pool: ResMut<FxPool>,
) {
    let _prof = shared::prof::span("fx::tick_fx");
    let now = time.elapsed_secs_f64();
    for (e, ttl, beam, boom, flash, glow, mut tf, light) in &mut q {
        let age = ((now - ttl.born) / ttl.total as f64) as f32;
        if age >= 1.0 {
            // Beam/fireball fades run in the shader; here they only go back to the pool.
            if let Some(b) = beam {
                commands.entity(e).remove::<Ttl>().insert(Visibility::Hidden);
                pool.beams.push((e, b.0.clone()));
            } else if let Some(b) = boom {
                commands.entity(e).remove::<Ttl>().insert(Visibility::Hidden);
                pool.booms.push((e, b.mat.clone()));
            } else {
                commands.entity(e).despawn();
            }
            continue;
        }
        if let Some(b) = boom {
            tf.scale = Vec3::splat(b.size * (6.0 + boom_growth(b.size) * (1.0 - (1.0 - age).powf(3.0))));
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
    let _prof = shared::prof::span("fx::mining_beams");
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
            let m = beams.add(BeamMat { p: BeamParams { color: Vec4::new(0.3, 1.0, 0.5, 0.8), flow: Vec4::new(0.05, 12.0, len, 0.0), life: Vec4::ZERO } });
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
    cam: Query<Entity, With<crate::control::MainCamera>>,
    mut tunnel: Query<(Entity, &Tunnel)>,
    mut warps: ResMut<Assets<WarpMat>>,
    mut arrived: Local<Option<f64>>,
    mut last_epoch: Local<u32>,
    mut was_charging: Local<bool>,
    mut sfx: ResMut<SfxQueue>,
) {
    let _prof = shared::prof::span("fx::warp_tunnel");
    if s.warp.is_some() && !*was_charging {
        sfx.play(Sfx::WarpCharge, None);
    }
    *was_charging = s.warp.is_some();
    let now = time.elapsed_secs_f64();
    let charge = s.warp.map(|(_, end, total)| (1.0 - ((end - now) as f32 / total)).clamp(0.0, 1.0));
    if *last_epoch != s.system_epoch {
        *last_epoch = s.system_epoch;
        *arrived = Some(now);
    }
    let k = match charge {
        Some(c) => c * c,
        None => arrived.map_or(0.0, |at| view::anim::fade(now - at, 1.0 / 1.2)),
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

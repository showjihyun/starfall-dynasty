//! Ship visuals: Blender-made GLB hulls re-skinned with the hull shader, engine plumes on
//! `Engine_*` nodes, rank-gated upgrade parts (`Rank3_/Rank6_/Rank9_`), shields, missiles, wrecks.

use crate::materials::*;
use crate::meshes;
use crate::state::{INTERP_DELAY, Screen, Session};
use bevy::gltf::GltfMaterialName;
use bevy::light::NotShadowCaster;
use bevy::prelude::*;
use bevy::world_serialization::WorldInstanceReady;
use shared::missions::NpcArchetype;
use shared::protocol::{EntityId, EntityKind, EntityState};
use shared::ships::HullClass;
use std::collections::HashMap;

pub struct ShipsPlugin;

impl Plugin for ShipsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Visuals>().add_systems(Startup, load_models).add_systems(Update, (sync_visuals, update_ship_fx).chain().run_if(in_state(Screen::Playing)));
    }
}

#[derive(Resource, Default)]
pub struct Visuals {
    pub map: HashMap<EntityId, Entity>,
    pub epoch: u32,
}

#[derive(Resource)]
pub struct Models {
    pub glb: HashMap<&'static str, Handle<WorldAsset>>,
    pub plume: Handle<Mesh>,
    pub shield: Handle<Mesh>,
}

pub type WorldAsset = bevy::world_serialization::WorldAsset;

#[derive(Component)]
pub struct ShipVis {
    pub id: EntityId,
    pub kind: EntityKind,
    pub radius: f32,
    pub livery: [f32; 3],
    pub glow: [f32; 3],
    pub hull_mats: Vec<Handle<HullMat>>,
    pub plumes: Vec<Handle<EngineMat>>,
    pub shield: Option<Handle<ShieldMat>>,
    pub hit_flash: f32,
    pub hit_age: f32,
    pub hit_dir: Vec3,
    pub muzzles: Vec<Entity>,
    pub rank: u8,
    pub boost: f32,
    pub throttle: f32,
    pub last_pos: Vec3,
    pub seed: f32,
}

#[derive(Component)]
pub struct RankPart(pub u8);

#[derive(Component)]
struct ModelRoot(Entity);

pub const LIVERIES: [[f32; 3]; 8] = [
    [0.85, 0.85, 0.88], // titanium
    [0.15, 0.45, 0.95], // navy blue
    [0.95, 0.55, 0.1],  // amber
    [0.1, 0.75, 0.55],  // jade
    [0.75, 0.15, 0.2],  // crimson
    [0.55, 0.3, 0.9],   // violet
    [0.95, 0.85, 0.2],  // gold
    [0.8, 0.12, 0.1],   // hostile red
];

pub fn model_for(hull: Option<HullClass>, kind: EntityKind) -> (&'static str, f32) {
    let h = match kind {
        EntityKind::Npc(NpcArchetype::Raider) => HullClass::Wren,
        EntityKind::Npc(NpcArchetype::Gunship) | EntityKind::Npc(NpcArchetype::Warlord) => HullClass::Bastion,
        EntityKind::Npc(NpcArchetype::Warden) | EntityKind::Npc(NpcArchetype::Sentinel) => HullClass::Seer,
        _ => hull.unwrap_or(HullClass::Talon),
    };
    match h {
        HullClass::Wren => ("wren", 20.0),
        HullClass::Talon => ("talon", 24.0),
        HullClass::Bastion => ("bastion", 36.0),
        HullClass::Mule => ("mule", 30.0),
        HullClass::Seer => ("seer", 26.0),
    }
}

pub fn assets_dir() -> std::path::PathBuf {
    let dev = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets");
    if dev.exists() {
        return dev;
    }
    std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.join("assets"))).unwrap_or_else(|| "assets".into())
}

fn load_models(mut commands: Commands, server: Res<AssetServer>, mut meshes: ResMut<Assets<Mesh>>) {
    let mut glb = HashMap::new();
    for name in ["wren", "talon", "bastion", "mule", "seer"] {
        let file = format!("models/{name}.glb");
        if assets_dir().join(&file).exists() {
            glb.insert(name, server.load(GltfAssetLabel::Scene(0).from_asset(file)));
        } else {
            warn!("model {file} missing — using procedural fallback");
        }
    }
    commands.insert_resource(Models { glb, plume: meshes.add(meshes::plume(0.25)), shield: meshes.add(Sphere::new(1.0).mesh().ico(4).unwrap()) });
}

#[allow(clippy::too_many_arguments)]
fn sync_visuals(
    mut commands: Commands,
    s: Res<Session>,
    time: Res<Time<Real>>,
    mut vis: ResMut<Visuals>,
    models: Res<Models>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut std_mats: ResMut<Assets<StandardMaterial>>,
    mut shield_mats: ResMut<Assets<ShieldMat>>,
    mut engine_mats: ResMut<Assets<EngineMat>>,
    mut q: Query<(&mut ShipVis, &mut Transform, &mut Visibility)>,
) {
    if vis.epoch != s.system_epoch {
        for (_, e) in vis.map.drain() {
            commands.entity(e).despawn();
        }
        vis.epoch = s.system_epoch;
    }
    let Some(latest) = s.latest() else { return };
    let t = time.elapsed_secs_f64() - INTERP_DELAY;
    // Remove vanished entities.
    let gone: Vec<EntityId> = vis.map.keys().filter(|id| !latest.ents.contains_key(id)).copied().collect();
    for id in gone {
        if let Some(e) = vis.map.remove(&id) {
            commands.entity(e).despawn();
        }
    }
    for (id, st) in &latest.ents {
        let Some((pos, rot, st2)) = s.sample(*id, t) else { continue };
        match vis.map.get(id).copied() {
            Some(e) => {
                if let Ok((mut v, mut tf, mut visibility)) = q.get_mut(e) {
                    let dt = time.delta_secs().max(1e-3);
                    let speed = Vec3::from(st2.vel).length();
                    v.last_pos = pos;
                    tf.translation = pos;
                    tf.rotation = rot;
                    *visibility = if st2.docked { Visibility::Hidden } else { Visibility::Inherited };
                    let max_speed = 140.0;
                    v.throttle = v.throttle + ((speed / max_speed).clamp(0.05, 1.0) - v.throttle) * 0.1;
                    v.boost = if st2.boosting { (v.boost + dt * 3.0).min(1.0) } else { (v.boost - dt * 2.0).max(0.0) };
                    v.rank = st2.rank;
                }
            }
            None => {
                let e = spawn_visual(&mut commands, st, pos, rot, &models, &mut meshes, &mut std_mats, &mut shield_mats, &mut engine_mats);
                vis.map.insert(*id, e);
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn spawn_visual(
    commands: &mut Commands,
    st: &EntityState,
    pos: Vec3,
    rot: Quat,
    models: &Models,
    meshes: &mut Assets<Mesh>,
    std_mats: &mut Assets<StandardMaterial>,
    shield_mats: &mut Assets<ShieldMat>,
    engine_mats: &mut Assets<EngineMat>,
) -> Entity {
    let hostile = matches!(st.kind, EntityKind::Npc(_));
    let livery = if hostile { LIVERIES[7] } else { LIVERIES[st.livery as usize % 8] };
    let glow = if hostile { [1.0, 0.25, 0.15] } else { [0.45, 0.8, 1.0] };
    let tf = Transform::from_translation(pos).with_rotation(rot);
    let seed = (st.id as f32 * 0.618).fract() * 10.0;
    let mut v = ShipVis {
        id: st.id,
        kind: st.kind,
        radius: st.radius,
        livery,
        glow,
        hull_mats: vec![],
        plumes: vec![],
        shield: None,
        hit_flash: 0.0,
        hit_age: 10.0,
        hit_dir: Vec3::Z,
        muzzles: vec![],
        rank: st.rank,
        boost: 0.0,
        throttle: 0.0,
        last_pos: pos,
        seed,
    };
    match st.kind {
        EntityKind::Missile => {
            let body = std_mats.add(StandardMaterial { base_color: Color::srgb(0.6, 0.6, 0.62), metallic: 0.9, perceptual_roughness: 0.3, ..default() });
            let plume = engine_mats.add(EngineMat { p: EngineParams { color: Vec4::new(1.0, 0.45, 0.1, 1.0), core: Vec4::new(1.0, 0.9, 0.7, 0.6), seed: Vec4::splat(seed) } });
            v.plumes.push(plume.clone());
            let root = commands.spawn((v, tf, Visibility::default())).id();
            commands.entity(root).with_children(|c| {
                c.spawn((Mesh3d(meshes.add(Capsule3d::new(0.6, 3.5))), MeshMaterial3d(body), Transform::from_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2))));
                c.spawn((NotShadowCaster, Mesh3d(models.plume.clone()), MeshMaterial3d(plume), Transform::from_xyz(0.0, 0.0, 2.2).with_scale(Vec3::new(0.7, 0.7, 14.0))));
            });
            root
        }
        EntityKind::Wreck => {
            let hull = std_mats.add(StandardMaterial { base_color: Color::srgb(0.16, 0.15, 0.15), metallic: 0.7, perceptual_roughness: 0.75, ..default() });
            let ember = std_mats.add(StandardMaterial { base_color: Color::BLACK, emissive: LinearRgba::rgb(3.0, 0.7, 0.1), ..default() });
            let root = commands.spawn((v, tf, Visibility::default())).id();
            commands.entity(root).with_children(|c| {
                for k in 0..9 {
                    let f = k as f32;
                    let m = if k % 4 == 0 { ember.clone() } else { hull.clone() };
                    c.spawn((
                        crate::scene::Spin(Vec3::new(0.05 * (f * 1.3).sin(), 0.07, 0.03 * f.cos())),
                        Mesh3d(meshes.add(Cuboid::new(3.0 + (f * 1.7) % 6.0, 0.8 + f % 2.0, 4.0 + (f * 2.3) % 8.0))),
                        MeshMaterial3d(m),
                        Transform::from_xyz((f * 2.1).sin() * 12.0, (f * 1.3).cos() * 5.0, (f * 0.9).sin() * 12.0).with_rotation(Quat::from_euler(EulerRot::XYZ, f, f * 0.7, f * 0.3)),
                    ));
                }
            });
            root
        }
        _ => {
            let (name, len) = model_for(st.hull_class, st.kind);
            let scale = st.radius * 2.2 / len;
            let shield = shield_mats.add(ShieldMat { p: ShieldParams { color: if hostile { Vec4::new(1.0, 0.35, 0.2, 1.0) } else { Vec4::new(0.35, 0.7, 1.0, 1.0) }, hit: Vec4::new(0.0, 0.0, 1.0, 10.0), center: pos.extend(st.radius) } });
            v.shield = Some(shield.clone());
            let root = commands.spawn((v, tf, Visibility::default())).id();
            commands.entity(root).with_children(|c| {
                c.spawn((NotShadowCaster, Mesh3d(models.shield.clone()), MeshMaterial3d(shield), Transform::from_scale(Vec3::new(1.15, 0.7, 1.35) * st.radius)));
            });
            if let Some(h) = models.glb.get(name) {
                let model = commands.spawn((WorldAssetRoot(h.clone()), ModelRoot(root), Transform::from_scale(Vec3::splat(scale)))).observe(on_model_ready).id();
                commands.entity(root).add_child(model);
            } else {
                fallback_model(commands, root, st.radius, livery, meshes, std_mats, engine_mats);
            }
            root
        }
    }
}

use bevy::world_serialization::WorldAssetRoot;

/// Procedural stand-in used when a GLB is missing.
fn fallback_model(commands: &mut Commands, root: Entity, r: f32, livery: [f32; 3], meshes: &mut Assets<Mesh>, mats: &mut Assets<StandardMaterial>, engine_mats: &mut Assets<EngineMat>) {
    let hull = mats.add(StandardMaterial { base_color: Color::srgb(0.6, 0.62, 0.66), metallic: 0.8, perceptual_roughness: 0.35, ..default() });
    let accent = mats.add(StandardMaterial { base_color: Color::srgb(livery[0], livery[1], livery[2]), metallic: 0.4, perceptual_roughness: 0.4, ..default() });
    let plume = engine_mats.add(EngineMat { p: EngineParams { color: Vec4::new(0.3, 0.6, 1.0, 0.5), core: Vec4::new(0.9, 0.95, 1.0, 0.0), seed: Vec4::ZERO } });
    commands.entity(root).with_children(|c| {
        c.spawn((Mesh3d(meshes.add(Cone { radius: r * 0.35, height: r * 2.0 })), MeshMaterial3d(hull.clone()), Transform::from_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2))));
        c.spawn((Mesh3d(meshes.add(Cuboid::new(r * 2.0, r * 0.08, r * 0.7))), MeshMaterial3d(accent), Transform::from_xyz(0.0, 0.0, r * 0.4)));
        c.spawn((NotShadowCaster, Mesh3d(meshes.add(meshes::plume(0.25))), MeshMaterial3d(plume), Transform::from_xyz(0.0, 0.0, r).with_scale(Vec3::new(r * 0.25, r * 0.25, r * 2.0))));
    });
}

#[allow(clippy::too_many_arguments)]
fn on_model_ready(
    ev: On<WorldInstanceReady>,
    mut commands: Commands,
    roots: Query<&ModelRoot>,
    children: Query<&Children>,
    names: Query<&Name>,
    mesh_q: Query<(&MeshMaterial3d<StandardMaterial>, Option<&GltfMaterialName>)>,
    std_mats: Res<Assets<StandardMaterial>>,
    mut hull_mats: ResMut<Assets<HullMat>>,
    mut engine_mats: ResMut<Assets<EngineMat>>,
    models: Res<Models>,
    mut ships: Query<&mut ShipVis>,
) {
    let Ok(ModelRoot(root)) = roots.get(ev.entity) else { return };
    let Ok(mut ship) = ships.get_mut(*root) else { return };
    let hostile = matches!(ship.kind, EntityKind::Npc(_));
    let mut cache: HashMap<AssetId<StandardMaterial>, Handle<HullMat>> = HashMap::new();
    for d in children.iter_descendants(ev.entity) {
        if let Ok(name) = names.get(d) {
            let n = name.as_str();
            if n.starts_with("Engine_") {
                let (col, core) = if hostile { ([1.0, 0.3, 0.1], [1.0, 0.8, 0.5]) } else { ([0.3, 0.6, 1.0], [0.85, 0.95, 1.0]) };
                let m = engine_mats.add(EngineMat { p: EngineParams { color: v4(col, 0.4), core: v4(core, 0.0), seed: Vec4::splat(ship.seed + ship.plumes.len() as f32) } });
                ship.plumes.push(m.clone());
                commands.entity(d).with_children(|c| {
                    c.spawn((NotShadowCaster, Mesh3d(models.plume.clone()), MeshMaterial3d(m), Transform::from_scale(Vec3::new(0.95, 0.95, 9.0))));
                });
            } else if n.starts_with("Gun_") {
                ship.muzzles.push(d);
            } else if let Some(rank) = n.strip_prefix("Rank").and_then(|r| r.split('_').next()).and_then(|r| r.parse::<u8>().ok()) {
                commands.entity(d).insert(RankPart(rank));
            }
        }
        if let Ok((MeshMaterial3d(h), gname)) = mesh_q.get(d) {
            let hm = cache.entry(h.id()).or_insert_with(|| {
                let base = std_mats.get(h).cloned().unwrap_or_default();
                let gname = gname.map(|g| g.0.to_lowercase()).unwrap_or_default();
                let kind = if gname.contains("livery") {
                    1.0
                } else if gname.contains("enginecore") {
                    3.0
                } else if gname.contains("glow") {
                    2.0
                } else {
                    0.0
                };
                hull_mats.add(HullMat { base, extension: HullExt { p: HullParams { livery: v4(ship.livery, kind), glow: v4(ship.glow, if kind == 3.0 { 4.0 } else { 6.0 }), state: Vec4::new(0.0, 0.0, 0.0, ship.seed) } } })
            });
            ship.hull_mats.push(hm.clone());
            commands.entity(d).remove::<MeshMaterial3d<StandardMaterial>>().insert(MeshMaterial3d(hm.clone()));
        }
    }
    ship.hull_mats.dedup();
}

#[allow(clippy::too_many_arguments)]
fn update_ship_fx(
    time: Res<Time>,
    s: Res<Session>,
    mut ships: Query<(&mut ShipVis, &Transform)>,
    mut hull_mats: ResMut<Assets<HullMat>>,
    mut engine_mats: ResMut<Assets<EngineMat>>,
    mut shield_mats: ResMut<Assets<ShieldMat>>,
    mut parts: Query<(&RankPart, &mut Visibility, &ChildOf)>,
    parents: Query<&ChildOf>,
) {
    let dt = time.delta_secs();
    let Some(latest) = s.latest() else { return };
    for (mut v, tf) in &mut ships {
        let Some(st) = latest.ents.get(&v.id) else { continue };
        v.hit_flash = (v.hit_flash - dt * 4.0).max(0.0);
        v.hit_age += dt;
        let damage = 1.0 - st.hull;
        for h in &v.hull_mats {
            if let Some(mut m) = hull_mats.get_mut(h) {
                m.extension.p.state = Vec4::new(damage, v.hit_flash, v.boost, v.seed);
            }
        }
        for h in &v.plumes {
            if let Some(mut m) = engine_mats.get_mut(h) {
                m.p.color.w = v.throttle;
                m.p.core.w = v.boost;
            }
        }
        if let Some(h) = &v.shield
            && let Some(mut m) = shield_mats.get_mut(h)
        {
            m.p.color.w = st.shield;
            m.p.center = tf.translation.extend(v.radius);
            let local = tf.rotation.inverse() * v.hit_dir;
            m.p.hit = local.extend(v.hit_age);
            // Shield direction uniform is compared against world normals in the shader.
            m.p.hit = v.hit_dir.extend(v.hit_age);
        }
    }
    // Rank parts: visible once the ship reaches the part's rank.
    for (part, mut vis, parent) in &mut parts {
        let mut e = parent.parent();
        let mut rank = None;
        for _ in 0..16 {
            if let Ok((v, _)) = ships.get(e) {
                rank = Some(v.rank);
                break;
            }
            match parents.get(e) {
                Ok(p) => e = p.parent(),
                Err(_) => break,
            }
        }
        if let Some(r) = rank {
            *vis = if r >= part.0 { Visibility::Inherited } else { Visibility::Hidden };
        }
    }
}

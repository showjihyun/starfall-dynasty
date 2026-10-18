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
use shared::ships::{HullClass, SizeClass};
use std::collections::HashMap;

pub struct ShipsPlugin;

impl Plugin for ShipsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Visuals>().add_systems(Startup, load_models).add_systems(Update, (sync_visuals, update_ship_fx, aim_turrets, face_nozzle_glows).chain().run_if(in_state(Screen::Playing)));
    }
}

#[derive(Resource, Default)]
pub struct Visuals {
    pub map: HashMap<EntityId, Entity>,
    pub epoch: u32,
}

/// Ship models are loaded lazily: a 600 m capital's 4096² textures cost hundreds of MB of VRAM,
/// so only hulls that actually appear on screen get loaded.
#[derive(Resource)]
pub struct Models {
    available: std::collections::HashSet<String>,
    cache: std::sync::Mutex<HashMap<String, Handle<WorldAsset>>>,
    server: AssetServer,
    pub plume: Handle<Mesh>,
    pub shield: Handle<Mesh>,
    pub quad: Handle<Mesh>,
    /// Dark plug that closes big-ship engine bores (their mirrored metal walls read as glass bulbs).
    pub bore: (Handle<Mesh>, Handle<StandardMaterial>),
}

pub type WorldAsset = bevy::world_serialization::WorldAsset;

#[derive(Component)]
pub struct ShipVis {
    pub id: EntityId,
    pub radius: f32,
    pub livery: [f32; 3],
    pub glow: [f32; 3],
    /// Drawn in hostile colours (glow, plumes, shield): my relation to it, kept up to date.
    pub hostile: bool,
    /// Species style for the hull shader (`palette::species_style`).
    pub species: [f32; 4],
    /// The captain's species: which `Species_<Name>_Kit` node the model shows (none for NPCs).
    pub species_of: Option<shared::galaxy::Species>,
    pub hull_mats: Vec<Handle<HullMat>>,
    pub plumes: Vec<Handle<EngineMat>>,
    /// Camera-facing glow at every nozzle exit (one material per ship).
    pub nozzle_glow: Option<(Handle<FlareMat>, f32)>,
    pub shield: Option<Handle<ShieldMat>>,
    /// When the hull was last hit with no shield left (the hull flash fades from here).
    pub flash_at: f64,
    /// When the shield was last hit (the ripple runs from here).
    pub hit_at: f64,
    pub hit_dir: Vec3,
    pub muzzles: Vec<Entity>,
    pub rank: u8,
    pub boost: view::anim::Ramp,
    pub throttle: f32,
    pub last_pos: Vec3,
    pub seed: f32,
    pub class: SizeClass,
    /// Turret mesh nodes with their rest rotation, and the muzzle under each.
    pub turrets: Vec<(Entity, Quat)>,
    pub turret_muzzles: Vec<Entity>,
    /// Bow main-gun ports.
    pub mains: Vec<Entity>,
    /// First-person camera anchor.
    pub bridge: Option<Entity>,
    /// What the turrets are tracking (from the latest turret shots) and when.
    pub aim: Option<(EntityId, f64)>,
    /// Turret swing: what they last turned toward, since when, and their rotations at that moment.
    pub swing: (Option<EntityId>, f64, Vec<Quat>),
    /// Last values written to (hull, engine, shield) materials.
    pub sent: (Vec4, Vec2, Vec4),
}

/// Boost glow ramp rates (per second): up in a third of a second, down in half.
/// `ShipVis::sent` before anything was written to the materials.
const UNSENT: (Vec4, Vec2, Vec4) = (Vec4::splat(-1.0), Vec2::splat(-1.0), Vec4::splat(-1.0));
const BOOST_UP: f32 = 3.0;
const BOOST_DOWN: f32 = 2.0;

#[derive(Component)]
pub struct RankPart(pub u8);

#[derive(Component)]
struct ModelRoot(Entity);

pub use view::palette::LIVERIES;

/// Dedicated enemy model (name, nominal length) if the archetype has one.
pub fn npc_model(kind: EntityKind) -> Option<(&'static str, f32)> {
    match kind {
        EntityKind::Npc(NpcArchetype::Raider) => Some(("npc_raider", 20.0)),
        EntityKind::Npc(NpcArchetype::Gunship) => Some(("npc_gunship", 32.0)),
        EntityKind::Npc(NpcArchetype::Warden) => Some(("npc_warden", 26.0)),
        EntityKind::Npc(NpcArchetype::Warlord) => Some(("npc_warlord", 66.0)),
        EntityKind::Npc(NpcArchetype::Sentinel) => Some(("npc_sentinel", 130.0)),
        _ => None,
    }
}

/// Nominal model lengths (m) of each hull family by growth stage; must match the Blender build scripts.
fn hull_lengths(h: HullClass) -> (&'static str, [f32; 3]) {
    match h {
        HullClass::Wren => ("wren", [20.0, 130.0, 360.0]),
        HullClass::Talon => ("talon", [24.0, 150.0, 420.0]),
        HullClass::Bastion => ("bastion", [36.0, 180.0, 600.0]),
        HullClass::Mule => ("mule", [30.0, 170.0, 520.0]),
        HullClass::Seer => ("seer", [26.0, 150.0, 450.0]),
    }
}

/// Model for a player/escort ship at its growth stage, falling back to a smaller stage if a model is missing.
pub fn stage_model(models: &Models, hull: HullClass, class: SizeClass) -> (String, f32) {
    let (base, lens) = hull_lengths(hull);
    let stages = [(format!("{base}_capital"), lens[2]), (format!("{base}_cruiser"), lens[1]), (base.to_string(), lens[0])];
    let start = match class {
        SizeClass::Capital => 0,
        SizeClass::Cruiser => 1,
        SizeClass::Small => 2,
    };
    for (name, len) in stages[start..].iter() {
        if models.has(name) {
            return (name.clone(), *len);
        }
    }
    (base.to_string(), lens[0])
}

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

impl Models {
    pub fn has(&self, name: &str) -> bool {
        self.available.contains(name)
    }

    pub fn handle(&self, name: &str) -> Option<Handle<WorldAsset>> {
        if !self.has(name) {
            return None;
        }
        let mut c = self.cache.lock().unwrap();
        Some(c.entry(name.to_string()).or_insert_with(|| self.server.load(GltfAssetLabel::Scene(0).from_asset(format!("models/{name}.glb")))).clone())
    }
}

fn load_models(mut commands: Commands, server: Res<AssetServer>, mut meshes: ResMut<Assets<Mesh>>, mut mats: ResMut<Assets<StandardMaterial>>) {
    let mut available = std::collections::HashSet::new();
    let mut names: Vec<String> = ["npc_raider", "npc_gunship", "npc_warden", "npc_warlord", "npc_sentinel", "wreck", "bridge_interior", "cockpit_interior"].iter().map(|s| s.to_string()).collect();
    for h in HullClass::ALL {
        let base = hull_lengths(h).0;
        names.extend([base.to_string(), format!("{base}_cruiser"), format!("{base}_capital")]);
    }
    for name in names {
        let file = format!("models/{name}.glb");
        if assets_dir().join(&file).exists() {
            available.insert(name);
        } else {
            warn!("model {file} missing — using procedural fallback");
        }
    }
    // STARFALL_PRELOAD_ALL=1 restores eager loading (for comparing memory/load time).
    let preload = std::env::var("STARFALL_PRELOAD_ALL").is_ok();
    let m = Models { available, cache: Default::default(), server: server.clone(), plume: meshes.add(meshes::plume(0.25)), shield: meshes.add(Sphere::new(1.0).mesh().ico(4).unwrap()), quad: meshes.add(Rectangle::new(1.0, 1.0)), bore: (meshes.add(Circle::new(0.97)), mats.add(StandardMaterial { base_color: Color::srgb(0.015, 0.02, 0.035), unlit: true, ..default() })) };
    if preload {
        for n in m.available.clone() {
            m.handle(&n);
        }
    }
    commands.insert_resource(m);
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
    cam: Res<crate::control::CamState>,
) {
    let _prof = shared::prof::span("ships::sync_visuals");
    if vis.epoch != s.system_epoch {
        for (_, e) in vis.map.drain() {
            commands.entity(e).despawn();
        }
        vis.epoch = s.system_epoch;
    }
    let Some(latest) = s.latest() else { return };
    let now = time.elapsed_secs_f64();
    let t = now - INTERP_DELAY;
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
            Some(e) if q.get(e).is_ok_and(|(v, _, _)| matches!(st.kind, EntityKind::Player) && v.class != st.class.unwrap_or(SizeClass::Small)) => {
                // Grew into a cruiser / capital ship: swap the model.
                commands.entity(e).despawn();
                let ne = spawn_visual(&mut commands, st, pos, rot, &models, &mut meshes, &mut std_mats, &mut shield_mats, &mut engine_mats);
                vis.map.insert(*id, ne);
            }
            Some(e) => {
                if let Ok((mut v, mut tf, mut visibility)) = q.get_mut(e) {
                    v.last_pos = pos;
                    tf.translation = pos;
                    tf.rotation = rot;
                    // Docked ships vanish into the station; in a fighter's cockpit view our own hull is hidden.
                    let cockpit = Some(*id) == s.me && cam.first_person && v.class == SizeClass::Small;
                    let want = if st2.docked || cockpit { Visibility::Hidden } else { Visibility::Inherited };
                    if *visibility != want {
                        *visibility = want;
                    }
                    // Throttle: mean speed over the last 0.4 s of snapshots; boost glow ramps from its switch.
                    let speeds: Vec<f32> = (0..5).filter_map(|k| s.sample(*id, t - k as f64 * 0.1)).map(|(_, _, e)| Vec3::from(e.vel).length()).collect();
                    v.throttle = view::anim::throttle(&speeds, 140.0);
                    v.boost.set(st2.boosting, now, BOOST_UP, BOOST_DOWN);
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
    let livery = if hostile { LIVERIES[view::palette::HOSTILE_LIVERY as usize] } else { LIVERIES[st.livery as usize % LIVERIES.len()] };
    let glow = view::palette::side::of(hostile).glow;
    let tf = Transform::from_translation(pos).with_rotation(rot);
    let seed = (st.id as f32 * 0.618).fract() * 10.0;
    let mut v = ShipVis {
        id: st.id,
        radius: st.radius,
        livery,
        glow,
        hostile,
        species: view::palette::species_style(st.species),
        species_of: st.species,
        hull_mats: vec![],
        plumes: vec![],
        nozzle_glow: None,
        shield: None,
        flash_at: f64::NEG_INFINITY,
        hit_at: f64::NEG_INFINITY,
        hit_dir: Vec3::Z,
        muzzles: vec![],
        rank: st.rank,
        boost: view::anim::Ramp::default(),
        throttle: 0.0,
        last_pos: pos,
        seed,
        class: st.class.unwrap_or(SizeClass::Small),
        turrets: vec![],
        turret_muzzles: vec![],
        mains: vec![],
        bridge: None,
        aim: None,
        swing: (None, 0.0, vec![]),
        sent: UNSENT,
    };
    match st.kind {
        EntityKind::Missile => {
            let body = std_mats.add(StandardMaterial { base_color: Color::srgb(0.6, 0.6, 0.62), metallic: 0.9, perceptual_roughness: 0.3, ..default() });
            let plume = engine_mats.add(EngineMat { p: EngineParams { color: Vec4::new(1.0, 0.45, 0.1, 1.0), core: Vec4::new(1.0, 0.9, 0.7, 0.6), seed: Vec4::new(seed, 1.0, 0.0, 0.0) } });
            v.plumes.push(plume.clone());
            let root = commands.spawn((v, tf, Visibility::default())).id();
            commands.entity(root).with_children(|c| {
                c.spawn((Mesh3d(meshes.add(Capsule3d::new(0.6, 3.5))), MeshMaterial3d(body), Transform::from_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2))));
                c.spawn((NotShadowCaster, Mesh3d(models.plume.clone()), MeshMaterial3d(plume), Transform::from_xyz(0.0, 0.0, 2.2).with_scale(Vec3::new(0.7, 0.7, 14.0))));
            });
            root
        }
        EntityKind::Wreck if models.has("wreck") => {
            let root = commands.spawn((v, tf, Visibility::default())).id();
            let model = commands.spawn((WorldAssetRoot(models.handle("wreck").unwrap()), crate::scene::Spin(Vec3::new(0.01, 0.03, 0.008)), Transform::default())).id();
            commands.entity(root).add_child(model);
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
            let (name, len) = match (st.kind, st.hull_class) {
                (EntityKind::Player | EntityKind::Escort, Some(h)) => stage_model(models, h, st.class.unwrap_or(SizeClass::Small)),
                _ => npc_model(st.kind).filter(|(n, _)| models.has(n)).map(|(n, l)| (n.to_string(), l)).unwrap_or_else(|| {
                    let (n, l) = model_for(st.hull_class, st.kind);
                    (n.to_string(), l)
                }),
            };
            let scale = st.radius * 2.2 / len;
            let shield = shield_mats.add(ShieldMat { p: ShieldParams { color: v4(view::palette::side::of(hostile).shield, 1.0), hit: Vec4::new(0.0, 0.0, 1.0, 10.0), center: pos.extend(st.radius) } });
            v.shield = Some(shield.clone());
            let root = commands.spawn((v, tf, Visibility::default())).id();
            commands.entity(root).with_children(|c| {
                c.spawn((NotShadowCaster, Mesh3d(models.shield.clone()), MeshMaterial3d(shield), Transform::from_scale(Vec3::new(1.15, 0.7, 1.35) * st.radius)));
            });
            if let Some(h) = models.handle(&name) {
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
    let plume = engine_mats.add(EngineMat { p: EngineParams { color: Vec4::new(0.3, 0.6, 1.0, 0.5), core: Vec4::new(0.9, 0.95, 1.0, 0.0), seed: Vec4::new(0.0, 1.0, 0.0, 0.0) } });
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
    mut flare_mats: ResMut<Assets<FlareMat>>,
    models: Res<Models>,
    mut ships: Query<&mut ShipVis>,
    transforms: Query<&Transform>,
) {
    let Ok(ModelRoot(root)) = roots.get(ev.entity) else { return };
    let Ok(mut ship) = ships.get_mut(*root) else { return };
    let hostile = ship.hostile;
    let mut cache: HashMap<AssetId<StandardMaterial>, Handle<HullMat>> = HashMap::new();
    let mut turrets: Vec<(usize, Entity)> = vec![];
    let mut muzzles: Vec<(usize, Entity)> = vec![];
    for d in children.iter_descendants(ev.entity) {
        if let Ok(name) = names.get(d) {
            let n = name.as_str();
            if n.starts_with("Engine_") {
                let side = view::palette::side::of(hostile);
                let (col, core) = (side.engine, side.engine_core);
                // Capital-ship drives read as broad glowing banks, not kilometre-long torches.
                // (length, width, gain, style): fighters burn a short torch; big hulls a long, slender ion wake.
                let (len, width, gain, style) = match ship.class {
                    SizeClass::Small => (13.0, 0.9, 1.0, 0.0),
                    SizeClass::Cruiser => (10.0, 0.75, 0.9, 1.0),
                    SizeClass::Capital => (9.0, 0.7, 0.8, 1.0),
                };
                let m = engine_mats.add(EngineMat { p: EngineParams { color: v4(col, 0.4), core: v4(core, 0.0), seed: Vec4::new(ship.seed + ship.plumes.len() as f32, gain, style, 0.0) } });
                ship.plumes.push(m.clone());
                let glow_gain = match ship.class {
                    SizeClass::Small => 1.0,
                    SizeClass::Cruiser => 0.85,
                    SizeClass::Capital => 0.75,
                };
                let seed = ship.seed;
                let glow = ship.nozzle_glow.get_or_insert_with(|| (flare_mats.add(FlareMat { p: FlareParams { color: v4(col, glow_gain * 1.0), shape: Vec4::new(0.0, style, 1.0, seed) } }), glow_gain * 1.0)).0.clone();
                commands.entity(d).with_children(|c| {
                    c.spawn((NotShadowCaster, Mesh3d(models.plume.clone()), MeshMaterial3d(m), Transform::from_scale(Vec3::new(width, width, len))));
                    let sprite = if style > 0.5 { 2.0 } else { 3.2 };
                    c.spawn((NozzleGlow, NotShadowCaster, Mesh3d(models.quad.clone()), MeshMaterial3d(glow), Transform::from_xyz(0.0, 0.0, 0.15).with_scale(Vec3::splat(sprite))));
                    if style > 0.5 {
                        // Circle faces +Z = out of the nozzle; sits just inside the exit plane.
                        c.spawn((NotShadowCaster, Mesh3d(models.bore.0.clone()), MeshMaterial3d(models.bore.1.clone()), Transform::from_xyz(0.0, 0.0, -0.04)));
                    }
                });
            } else if n.starts_with("Gun_") {
                ship.muzzles.push(d);
            } else if n.starts_with("Main_") {
                ship.mains.push(d);
            } else if n == "Bridge" {
                ship.bridge = Some(d);
            } else if let Some(k) = n.strip_prefix("Turret_").and_then(|k| k.parse::<usize>().ok()) {
                turrets.push((k, d));
                commands.entity(d).insert(TurretTag);
            } else if let Some(k) = n.strip_prefix("Muzzle_").and_then(|k| k.parse::<usize>().ok()) {
                muzzles.push((k, d));
            } else if let Some(rank) = n.strip_prefix("Rank").and_then(|r| r.split('_').next()).and_then(|r| r.parse::<u8>().ok()) {
                commands.entity(d).insert(RankPart(rank));
            } else if let Some(kit) = n.strip_prefix("Species_").and_then(|r| r.split('_').next()) {
                // Species kit (docs/ART_DIRECTION.md §4): only the captain's own species is shown.
                let mine = ship.species_of.is_some_and(|s| format!("{s:?}") == kit);
                commands.entity(d).insert(if mine { Visibility::Inherited } else { Visibility::Hidden });
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
                hull_mats.add(HullMat { base, extension: HullExt { p: HullParams { livery: v4(ship.livery, kind), glow: v4(ship.glow, if kind == 3.0 { 1.6 } else { 4.0 }), state: Vec4::new(0.0, 0.0, 0.0, ship.seed), species: Vec4::from_array(ship.species) } } })
            });
            ship.hull_mats.push(hm.clone());
            commands.entity(d).remove::<MeshMaterial3d<StandardMaterial>>().insert(MeshMaterial3d(hm.clone()));
        }
    }
    ship.hull_mats.dedup();
    turrets.sort_by_key(|t| t.0);
    muzzles.sort_by_key(|m| m.0);
    ship.turrets = turrets.iter().map(|(_, e)| (*e, transforms.get(*e).map(|t| t.rotation).unwrap_or_default())).collect();
    ship.turret_muzzles = muzzles.into_iter().map(|(_, e)| e).collect();
    // The new materials start from their defaults: send hull, engine and shield state again next frame,
    // or a model that finishes loading after the state settled keeps the defaults (wrong plume/damage).
    ship.sent = UNSENT;
    info!("model ready for {}: {} plumes, {} hull mats, {} muzzles", ship.id, ship.plumes.len(), ship.hull_mats.len(), ship.muzzles.len());
}

#[allow(clippy::too_many_arguments)]
fn update_ship_fx(
    time: Res<Time<Real>>,
    s: Res<Session>,
    mut ships: Query<&mut ShipVis>,
    mut hull_mats: ResMut<Assets<HullMat>>,
    mut engine_mats: ResMut<Assets<EngineMat>>,
    mut flare_mats: ResMut<Assets<FlareMat>>,
    mut shield_mats: ResMut<Assets<ShieldMat>>,
    mut parts: Query<(&RankPart, &mut Visibility, &ChildOf)>,
    parents: Query<&ChildOf>,
) {
    let _prof = shared::prof::span("ships::update_ship_fx");
    // Touching a material asset re-uploads its bind group, so only write values that changed.
    let now = time.elapsed_secs_f64();
    let Some(latest) = s.latest() else { return };
    let viewer = view::relation::Viewer { me: s.me, player: s.player_id, fleet: s.fleet, alliance: s.my_alliance, alliances: &s.alliances, tier: shared::galaxy::system(s.system).tier };
    for mut v in &mut ships {
        let Some(st) = latest.ents.get(&v.id) else { continue };
        // A captain who becomes hostile (PvP space, a declared war) turns red like a pirate, and back.
        let hostile = matches!(viewer.relation(st), view::relation::Relation::Hostile);
        if hostile != v.hostile {
            v.hostile = hostile;
            let side = view::palette::side::of(hostile);
            v.glow = side.glow;
            let rgb = |c: [f32; 3], w: f32| Vec4::new(c[0], c[1], c[2], w);
            for h in &v.hull_mats {
                if let Some(mut m) = hull_mats.get_mut(h) {
                    m.extension.p.glow = rgb(side.glow, m.extension.p.glow.w);
                }
            }
            for h in &v.plumes {
                if let Some(mut m) = engine_mats.get_mut(h) {
                    (m.p.color, m.p.core) = (rgb(side.engine, m.p.color.w), rgb(side.engine_core, m.p.core.w));
                }
            }
            if let Some((h, _)) = &v.nozzle_glow
                && let Some(mut m) = flare_mats.get_mut(h)
            {
                m.p.color = rgb(side.engine, m.p.color.w);
            }
            if let Some(h) = &v.shield
                && let Some(mut m) = shield_mats.get_mut(h)
            {
                m.p.color = rgb(side.shield, 1.0);
            }
        }
        let flash = view::anim::fade(now - v.flash_at, 4.0);
        let hit_age = (now - v.hit_at).min(3.0) as f32;
        let boost = v.boost.value(now, BOOST_UP, BOOST_DOWN);
        let q = |x: f32, step: f32| (x / step).round() * step;
        let hull_state = Vec4::new(q(1.0 - st.hull, 0.02), q(flash, 0.02), q(boost, 0.05), v.seed);
        if hull_state != v.sent.0 {
            v.sent.0 = hull_state;
            for h in &v.hull_mats {
                if let Some(mut m) = hull_mats.get_mut(h) {
                    m.extension.p.state = hull_state;
                }
            }
        }
        let engine = Vec2::new(q(v.throttle, 0.05), q(boost, 0.05));
        if engine != v.sent.1 {
            v.sent.1 = engine;
            if let Some((h, gain)) = &v.nozzle_glow
                && let Some(mut m) = flare_mats.get_mut(h)
            {
                m.p.color.w = gain * (0.8 + engine.x * 1.2 + engine.y * 1.5);
            }
            for h in &v.plumes {
                if let Some(mut m) = engine_mats.get_mut(h) {
                    m.p.color.w = engine.x;
                    m.p.core.w = engine.y;
                }
            }
        }
        let shield = Vec4::new(q(st.shield, 0.02), v.hit_dir.x, v.hit_dir.y, hit_age);
        if shield != v.sent.2
            && let Some(h) = &v.shield
            && let Some(mut m) = shield_mats.get_mut(h)
        {
            v.sent.2 = shield;
            m.p.color.w = shield.x;
            m.p.hit = v.hit_dir.extend(shield.w);
        }
    }
    // Rank parts: visible once the ship reaches the part's rank.
    for (part, mut vis, parent) in &mut parts {
        let mut e = parent.parent();
        let mut rank = None;
        for _ in 0..16 {
            if let Ok(v) = ships.get(e) {
                rank = Some(v.rank);
                break;
            }
            match parents.get(e) {
                Ok(p) => e = p.parent(),
                Err(_) => break,
            }
        }
        if let Some(r) = rank {
            let want = if r >= part.0 { Visibility::Inherited } else { Visibility::Hidden };
            if *vis != want {
                *vis = want;
            }
        }
    }
}

/// Engine nozzle glow sprite (child of an `Engine_N` node).
#[derive(Component)]
pub struct NozzleGlow;

/// Keep nozzle glow sprites facing the camera.
fn face_nozzle_glows(cam: Query<&GlobalTransform, With<crate::control::MainCamera>>, mut q: Query<(&mut Transform, &ChildOf), With<NozzleGlow>>, globals: Query<&GlobalTransform, Without<NozzleGlow>>) {
    let _prof = shared::prof::span("ships::face_nozzle_glows");
    let Ok(c) = cam.single() else { return };
    let cam_rot = c.rotation();
    for (mut t, parent) in &mut q {
        if let Ok(pg) = globals.get(parent.parent()) {
            t.rotation = pg.rotation().inverse() * cam_rot;
        }
    }
}

/// Turrets swing around their local up axis to track whatever the ship is shooting at.
fn aim_turrets(
    vis: Res<Visuals>,
    time: Res<Time<Real>>,
    mut ships: Query<&mut ShipVis>,
    targets: Query<&Transform, Without<TurretTag>>,
    globals: Query<&GlobalTransform>,
    parents: Query<&ChildOf>,
    mut turret_tf: Query<&mut Transform, With<TurretTag>>,
) {
    let _prof = shared::prof::span("ships::aim_turrets");
    let now = time.elapsed_secs_f64();
    for mut v in &mut ships {
        if v.turrets.is_empty() {
            continue;
        }
        let aim_id = v.aim.filter(|(_, t)| now - t < 3.0).map(|(id, _)| id).filter(|id| vis.map.get(id).is_some_and(|e| targets.contains(*e)));
        let aim = aim_id.and_then(|id| vis.map.get(&id)).and_then(|e| targets.get(*e).ok()).map(|t| t.translation);
        // A new target (or none): swing from where the turrets point now.
        if aim_id != v.swing.0 || v.swing.2.len() != v.turrets.len() {
            let from = v.turrets.iter().map(|(e, rest)| turret_tf.get(*e).map(|t| t.rotation).unwrap_or(*rest)).collect();
            v.swing = (aim_id, now, from);
        }
        let since = now - v.swing.1;
        for (k, (turret, rest)) in v.turrets.iter().enumerate() {
            let Ok(mut tf) = turret_tf.get_mut(*turret) else { continue };
            let desired = match (aim, parents.get(*turret).ok().and_then(|p| globals.get(p.parent()).ok())) {
                (Some(target), Some(pg)) => {
                    let (_, prot, ppos) = pg.to_scale_rotation_translation();
                    let world_pivot = ppos + prot * tf.translation;
                    let d_parent = prot.inverse() * (target - world_pivot);
                    let d = rest.inverse() * d_parent;
                    // Barrels point along local -Z at rest; swing around local +Y.
                    let yaw = (-d.x).atan2(-d.z);
                    *rest * Quat::from_rotation_y(yaw)
                }
                _ => *rest,
            };
            tf.rotation = view::anim::swing(v.swing.2[k], desired, since, 1.5);
        }
    }
}

#[derive(Component)]
pub struct TurretTag;

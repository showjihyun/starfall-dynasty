//! Ship visuals: Blender-made GLB hulls re-skinned with the hull shader, engine plumes on
//! `Engine_*` nodes, rank-gated upgrade parts (`Rank3_/Rank6_/Rank9_`), shields, missiles, wrecks.

use crate::materials::*;
use crate::meshes;
use crate::state::{INTERP_DELAY, Screen, Session};
use bevy::gltf::GltfMaterialName;
use bevy::light::NotShadowCaster;
use bevy::mesh::MeshTag;
use bevy::prelude::*;
use bevy::render::storage::ShaderBuffer;
use bevy::ecs::system::SystemParam;
use bevy::world_serialization::WorldInstanceReady;
use shared::missions::NpcArchetype;
use shared::protocol::{EntityId, EntityKind, EntityState};
use shared::ships::{HullClass, SizeClass};
use std::collections::HashMap;

pub struct ShipsPlugin;

impl Plugin for ShipsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Visuals>().add_observer(release_slot).add_systems(Startup, load_models).add_systems(Update, (sync_visuals, update_ship_fx, aim_turrets, face_nozzle_glows).chain().run_if(in_state(Screen::Playing)))
            .add_systems(PostUpdate, engine_lod.before(bevy::camera::visibility::VisibilitySystems::VisibilityPropagate).run_if(in_state(Screen::Playing)));
    }
}

/// Per-ship state for the ship shaders (hull, engine plumes, nozzle glows, shield): one slot per ship
/// (`ShipVis::slot`), found by each primitive's `MeshTag` (low 24 bits; a plume's index on its ship
/// above). The materials are shared — hulls by every ship of one model part, livery and species,
/// plumes and nozzle glows per size class, one shield for all — so the GPU draws them in batches;
/// per-ship hull materials made the 1000-ship bench about 22% slower (Tracy, docs/STUDIO_PIPELINE.md).
/// One buffer write per frame when any ship changed.
#[derive(Resource)]
pub struct ShipStates {
    pub buffer: Handle<ShaderBuffer>,
    /// Per slot, `SLOT` vec4s in the order of the shaders' `Ship` struct (`HULL` … `HIT`).
    slots: Vec<[f32; 4 * SLOT]>,
    free: Vec<u32>,
    used: u32,
    dirty: bool,
    grown: bool,
    materials: HashMap<HullKey, Handle<HullMat>>,
    engines: HashMap<[u32; 12], Handle<EngineMat>>,
    nozzles: HashMap<[u32; 4], Handle<NozzleMat>>,
    shield: Option<Handle<ShieldMat>>,
}

/// A shared hull material: the model's part material, livery and species style (as bits).
type HullKey = (AssetId<StandardMaterial>, [u32; 3], [u32; 4]);

/// Vec4s per slot and their meaning (hull.wesl `struct Ship`).
const SLOT: usize = 7;
/// Damage, hit flash, boost, pattern seed.
const HULL: usize = 0;
/// Hull glow rgb.
const GLOW: usize = 1;
/// Throttle, boost (plumes and nozzle glows).
const DRIVE: usize = 2;
/// Plume sheath rgb.
const ENGINE: usize = 3;
/// Plume core rgb.
const CORE: usize = 4;
/// Shield rgb, shield fraction.
const SHIELD: usize = 5;
/// Local hit direction, seconds since the hit.
const HIT: usize = 6;

/// Every material that reads the state buffer, for spawning ships and rebinding a grown buffer.
#[derive(SystemParam)]
pub struct ShipMats<'w> {
    pub buffers: ResMut<'w, Assets<ShaderBuffer>>,
    pub hull: ResMut<'w, Assets<HullMat>>,
    pub engine: ResMut<'w, Assets<EngineMat>>,
    pub nozzle: ResMut<'w, Assets<NozzleMat>>,
    pub shield: ResMut<'w, Assets<ShieldMat>>,
}

fn bits<const N: usize>(v: [Vec4; N]) -> Vec<u32> {
    v.iter().flat_map(|x| x.to_array()).map(f32::to_bits).collect()
}

impl ShipStates {
    const START: usize = 256;

    pub fn new(buffers: &mut Assets<ShaderBuffer>) -> Self {
        let slots = vec![[0.0; 4 * SLOT]; Self::START];
        ShipStates { buffer: buffers.add(ShaderBuffer::from(slots.clone())), slots, free: vec![], used: 0, dirty: false, grown: false, materials: HashMap::new(), engines: HashMap::new(), nozzles: HashMap::new(), shield: None }
    }

    pub fn alloc(&mut self) -> u32 {
        if let Some(s) = self.free.pop() {
            return s;
        }
        let s = self.used;
        self.used += 1;
        if s as usize >= self.slots.len() {
            let n = self.slots.len() * 2;
            self.slots.resize(n, [0.0; 4 * SLOT]);
            self.grown = true;
            self.dirty = true;
        }
        s
    }

    fn put(&mut self, slot: u32, at: usize, v: Vec4) {
        self.slots[slot as usize][at * 4..at * 4 + 4].copy_from_slice(&v.to_array());
        self.dirty = true;
    }

    /// What the shaders read for `slot` at `at` (`HULL` … `HIT`).
    #[cfg(test)]
    pub fn get(&self, slot: u32, at: usize) -> Vec4 {
        Vec4::from_slice(&self.slots[slot as usize][at * 4..at * 4 + 4])
    }

    /// The plume material for these parameters, shared by every plume that uses them.
    fn engine(&mut self, mats: &mut Assets<EngineMat>, p: EngineParams) -> Handle<EngineMat> {
        let ships = self.buffer.clone();
        self.engines.entry(bits([p.color, p.core, p.seed]).try_into().unwrap()).or_insert_with(|| mats.add(EngineMat { p, ships })).clone()
    }

    fn nozzle(&mut self, mats: &mut Assets<NozzleMat>, p: NozzleParams) -> Handle<NozzleMat> {
        let ships = self.buffer.clone();
        self.nozzles.entry(bits([p.shape]).try_into().unwrap()).or_insert_with(|| mats.add(NozzleMat { p, ships })).clone()
    }

    fn shield(&mut self, mats: &mut Assets<ShieldMat>) -> Handle<ShieldMat> {
        let ships = self.buffer.clone();
        self.shield.get_or_insert_with(|| mats.add(ShieldMat { p: ShieldParams { look: Vec4::new(3.0, 0.0, 0.0, 0.0) }, ships })).clone()
    }

    /// Uploads this frame's changes (same size: written in place; grown: a new buffer, so the shared
    /// materials are touched to rebind it).
    fn flush(&mut self, mats: &mut ShipMats) {
        if !self.dirty {
            return;
        }
        if let Some(mut b) = mats.buffers.get_mut(&self.buffer) {
            *b = ShaderBuffer::from(self.slots.clone());
        }
        if self.grown {
            for h in self.materials.values() {
                let _ = mats.hull.get_mut(h);
            }
            for h in self.engines.values() {
                let _ = mats.engine.get_mut(h);
            }
            for h in self.nozzles.values() {
                let _ = mats.nozzle.get_mut(h);
            }
            if let Some(h) = &self.shield {
                let _ = mats.shield.get_mut(h);
            }
            self.grown = false;
        }
        self.dirty = false;
    }
}

/// `MeshTag` of a ship's `index`-th plume: its slot, and the index for the plume's flicker phase.
fn plume_tag(slot: u32, index: usize) -> MeshTag {
    MeshTag::new(slot | ((index.min(255) as u32) << 24))
}

/// A ship's visual is gone: its state slot can be reused.
fn release_slot(ev: On<Remove<ShipVis>>, ships: Query<&ShipVis>, states: Option<ResMut<ShipStates>>) {
    if let (Ok(v), Some(mut states)) = (ships.get(ev.entity), states) {
        states.free.push(v.slot);
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
    /// Slot in `ShipStates` (the hull shader's per-ship state), set when the visual is spawned.
    pub slot: u32,
    /// Engine plumes on the model (their state is in the ship's slot).
    pub plumes: usize,
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
    /// Last values written to the ship's slot: (hull, drive, shield).
    pub sent: (Vec4, Vec2, Vec4),
    /// The shield bubble, hidden while it would draw nothing (`view::anim::shield_shown`).
    pub shield: Option<(Entity, bool)>,
    /// Engine effects by detail level (`view::anim::engine_lod`): nozzle glows and bores, plumes.
    pub nozzles: Vec<Entity>,
    pub plume_fx: Vec<Entity>,
    pub lod: view::anim::EngineLod,
}

/// `ShipVis::sent` before anything was written to the slot.
const UNSENT: (Vec4, Vec2, Vec4) = (Vec4::splat(-1.0), Vec2::splat(-1.0), Vec4::splat(-1.0));
/// Boost glow ramp rates (per second): up in a third of a second, down in half.
const BOOST_UP: f32 = 3.0;
const BOOST_DOWN: f32 = 2.0;

impl ShipVis {
    /// A ship's visual state before its model exists (its slot is written from the first frame, so a
    /// model that finishes loading late shows the same state).
    pub fn new(st: &EntityState, pos: Vec3) -> Self {
        let hostile = matches!(st.kind, EntityKind::Npc(_));
        ShipVis {
            id: st.id,
            radius: st.radius,
            livery: if hostile { LIVERIES[view::palette::HOSTILE_LIVERY as usize] } else { LIVERIES[st.livery as usize % LIVERIES.len()] },
            glow: view::palette::side::of(hostile).glow,
            hostile,
            species: view::palette::species_style(st.species),
            species_of: st.species,
            slot: 0,
            plumes: 0,
            flash_at: f64::NEG_INFINITY,
            hit_at: f64::NEG_INFINITY,
            hit_dir: Vec3::Z,
            muzzles: vec![],
            rank: st.rank,
            boost: view::anim::Ramp::default(),
            throttle: 0.0,
            last_pos: pos,
            seed: (st.id as f32 * 0.618).fract() * 10.0,
            class: st.class.unwrap_or(SizeClass::Small),
            turrets: vec![],
            turret_muzzles: vec![],
            mains: vec![],
            bridge: None,
            aim: None,
            swing: (None, 0.0, vec![]),
            sent: UNSENT,
            shield: None,
            nozzles: vec![],
            plume_fx: vec![],
            lod: view::anim::EngineLod::Full,
        }
    }
}

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

fn load_models(mut commands: Commands, server: Res<AssetServer>, mut meshes: ResMut<Assets<Mesh>>, mut mats: ResMut<Assets<StandardMaterial>>, mut buffers: ResMut<Assets<ShaderBuffer>>) {
    commands.insert_resource(ShipStates::new(&mut buffers));
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
    mut mats: ShipMats,
    mut q: Query<(&mut ShipVis, &mut Transform, &mut Visibility)>,
    cam: Res<crate::control::CamState>,
    mut states: ResMut<ShipStates>,
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
                let ne = spawn_visual(&mut commands, st, pos, rot, &models, &mut meshes, &mut std_mats, &mut mats, &mut states);
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
                let e = spawn_visual(&mut commands, st, pos, rot, &models, &mut meshes, &mut std_mats, &mut mats, &mut states);
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
    mats: &mut ShipMats,
    states: &mut ShipStates,
) -> Entity {
    let mut v = ShipVis::new(st, pos);
    v.slot = states.alloc();
    let (livery, slot) = (v.livery, v.slot);
    let tf = Transform::from_translation(pos).with_rotation(rot);
    match st.kind {
        EntityKind::Missile => {
            let body = std_mats.add(StandardMaterial { base_color: Color::srgb(0.6, 0.6, 0.62), metallic: 0.9, perceptual_roughness: 0.3, ..default() });
            let plume = states.engine(&mut mats.engine, EngineParams { color: Vec4::new(1.0, 0.45, 0.1, 0.0), core: Vec4::new(1.0, 0.9, 0.7, 0.0), seed: Vec4::new(0.0, 1.0, 0.0, 1.0) });
            v.plumes = 1;
            let root = commands.spawn((v, tf, Visibility::default())).id();
            commands.entity(root).with_children(|c| {
                c.spawn((Mesh3d(meshes.add(Capsule3d::new(0.6, 3.5))), MeshMaterial3d(body), Transform::from_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2))));
                c.spawn((NotShadowCaster, Mesh3d(models.plume.clone()), MeshMaterial3d(plume), plume_tag(slot, 0), Transform::from_xyz(0.0, 0.0, 2.2).with_scale(Vec3::new(0.7, 0.7, 14.0))));
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
            let shield = states.shield(&mut mats.shield);
            let bubble = commands.spawn((NotShadowCaster, Mesh3d(models.shield.clone()), MeshMaterial3d(shield), MeshTag::new(slot), Visibility::Hidden, Transform::from_scale(Vec3::new(1.15, 0.7, 1.35) * st.radius))).id();
            v.shield = Some((bubble, false));
            let root = commands.spawn((v, tf, Visibility::default())).id();
            commands.entity(root).add_child(bubble);
            if let Some(h) = models.handle(&name) {
                let model = commands.spawn((WorldAssetRoot(h.clone()), ModelRoot(root), Transform::from_scale(Vec3::splat(scale)))).observe(on_model_ready).id();
                commands.entity(root).add_child(model);
            } else {
                let plume = states.engine(&mut mats.engine, EngineParams { color: Vec4::new(0.3, 0.6, 1.0, 0.0), core: Vec4::new(0.9, 0.95, 1.0, 0.0), seed: Vec4::new(0.0, 1.0, 0.0, 1.0) });
                fallback_model(commands, root, slot, st.radius, livery, meshes, std_mats, plume);
            }
            root
        }
    }
}

use bevy::world_serialization::WorldAssetRoot;
use view::anim::EngineLod;

/// Procedural stand-in used when a GLB is missing.
#[allow(clippy::too_many_arguments)]
fn fallback_model(commands: &mut Commands, root: Entity, slot: u32, r: f32, livery: [f32; 3], meshes: &mut Assets<Mesh>, mats: &mut Assets<StandardMaterial>, plume: Handle<EngineMat>) {
    let hull = mats.add(StandardMaterial { base_color: Color::srgb(0.6, 0.62, 0.66), metallic: 0.8, perceptual_roughness: 0.35, ..default() });
    let accent = mats.add(StandardMaterial { base_color: Color::srgb(livery[0], livery[1], livery[2]), metallic: 0.4, perceptual_roughness: 0.4, ..default() });
    commands.entity(root).with_children(|c| {
        c.spawn((Mesh3d(meshes.add(Cone { radius: r * 0.35, height: r * 2.0 })), MeshMaterial3d(hull.clone()), Transform::from_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2))));
        c.spawn((Mesh3d(meshes.add(Cuboid::new(r * 2.0, r * 0.08, r * 0.7))), MeshMaterial3d(accent), Transform::from_xyz(0.0, 0.0, r * 0.4)));
        c.spawn((NotShadowCaster, Mesh3d(meshes.add(meshes::plume(0.25))), MeshMaterial3d(plume), plume_tag(slot, 0), Transform::from_xyz(0.0, 0.0, r).with_scale(Vec3::new(r * 0.25, r * 0.25, r * 2.0))));
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
    mut mats: ShipMats,
    models: Res<Models>,
    mut ships: Query<&mut ShipVis>,
    transforms: Query<&Transform>,
    mut states: ResMut<ShipStates>,
) {
    let Ok(ModelRoot(root)) = roots.get(ev.entity) else { return };
    let Ok(mut ship) = ships.get_mut(*root) else { return };
    let slot = ship.slot;
    let mut hull_parts = std::collections::HashSet::new();
    let buffer = states.buffer.clone();
    let mut turrets: Vec<(usize, Entity)> = vec![];
    let mut muzzles: Vec<(usize, Entity)> = vec![];
    for d in children.iter_descendants(ev.entity) {
        if let Ok(name) = names.get(d) {
            let n = name.as_str();
            if n.starts_with("Engine_") {
                // Colours, throttle and boost are the ship's (its slot); the materials are per size class.
                // Capital-ship drives read as broad glowing banks, not kilometre-long torches.
                // (length, width, gain, style): fighters burn a short torch; big hulls a long, slender ion wake.
                let (len, width, gain, style) = match ship.class {
                    SizeClass::Small => (13.0, 0.9, 1.0, 0.0),
                    SizeClass::Cruiser => (10.0, 0.75, 0.9, 1.0),
                    SizeClass::Capital => (9.0, 0.7, 0.8, 1.0),
                };
                let m = states.engine(&mut mats.engine, EngineParams { color: Vec4::ZERO, core: Vec4::ZERO, seed: Vec4::new(0.0, gain, style, 0.0) });
                let tag = plume_tag(slot, ship.plumes);
                ship.plumes += 1;
                let glow_gain = match ship.class {
                    SizeClass::Small => 1.0,
                    SizeClass::Cruiser => 0.85,
                    SizeClass::Capital => 0.75,
                };
                let glow = states.nozzle(&mut mats.nozzle, NozzleParams { shape: Vec4::new(glow_gain, style, 0.0, 0.0) });
                // Spawned at the ship's current engine detail (engine_lod may have run before the model loaded).
                let shown = |on: bool| if on { Visibility::Inherited } else { Visibility::Hidden };
                let (plume_vis, nozzle_vis) = (shown(ship.lod != EngineLod::Off), shown(ship.lod == EngineLod::Full));
                let (mut plumes, mut nozzles) = (vec![], vec![]);
                commands.entity(d).with_children(|c| {
                    plumes.push(c.spawn((NotShadowCaster, Mesh3d(models.plume.clone()), MeshMaterial3d(m), tag, plume_vis, Transform::from_scale(Vec3::new(width, width, len)))).id());
                    let sprite = if style > 0.5 { 2.0 } else { 3.2 };
                    nozzles.push(c.spawn((NozzleGlow, NotShadowCaster, Mesh3d(models.quad.clone()), MeshMaterial3d(glow), MeshTag::new(slot), nozzle_vis, Transform::from_xyz(0.0, 0.0, 0.15).with_scale(Vec3::splat(sprite)))).id());
                    if style > 0.5 {
                        // Circle faces +Z = out of the nozzle; sits just inside the exit plane.
                        nozzles.push(c.spawn((NotShadowCaster, Mesh3d(models.bore.0.clone()), MeshMaterial3d(models.bore.1.clone()), nozzle_vis, Transform::from_xyz(0.0, 0.0, -0.04))).id());
                    }
                });
                ship.plume_fx.extend(plumes);
                ship.nozzles.extend(nozzles);
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
            let key = (h.id(), ship.livery.map(f32::to_bits), ship.species.map(f32::to_bits));
            hull_parts.insert(h.id());
            let hm = states.materials.entry(key).or_insert_with(|| {
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
                mats.hull.add(HullMat { base, extension: HullExt { p: HullParams { livery: v4(ship.livery, kind), glow: Vec4::new(0.0, 0.0, 0.0, if kind == 3.0 { 1.6 } else { 4.0 }), species: Vec4::from_array(ship.species) }, ships: buffer.clone() } })
            });
            commands.entity(d).remove::<MeshMaterial3d<StandardMaterial>>().insert((MeshMaterial3d(hm.clone()), MeshTag::new(slot)));
        }
    }
    turrets.sort_by_key(|t| t.0);
    muzzles.sort_by_key(|m| m.0);
    ship.turrets = turrets.iter().map(|(_, e)| (*e, transforms.get(*e).map(|t| t.rotation).unwrap_or_default())).collect();
    ship.turret_muzzles = muzzles.into_iter().map(|(_, e)| e).collect();
    info!("model ready for {}: {} plumes, {} hull parts (slot {}), {} muzzles", ship.id, ship.plumes, hull_parts.len(), ship.slot, ship.muzzles.len());
}

#[allow(clippy::too_many_arguments)]
fn update_ship_fx(
    time: Res<Time<Real>>,
    s: Res<Session>,
    mut ships: Query<&mut ShipVis>,
    mut mats: ShipMats,
    mut parts: Query<(&RankPart, &mut Visibility, &ChildOf)>,
    parents: Query<&ChildOf>,
    mut states: ResMut<ShipStates>,
    mut bubbles: Query<&mut Visibility, Without<RankPart>>,
) {
    let _prof = shared::prof::span("ships::update_ship_fx");
    // Only write values that changed: an unchanged frame uploads nothing.
    let now = time.elapsed_secs_f64();
    let Some(latest) = s.latest() else { return };
    let viewer = view::relation::Viewer { me: s.me, player: s.player_id, fleet: s.fleet, alliance: s.my_alliance, alliances: &s.alliances, tier: shared::galaxy::system(s.system).tier };
    for mut v in &mut ships {
        let Some(st) = latest.ents.get(&v.id) else { continue };
        // A captain who becomes hostile (PvP space, a declared war) turns red like a pirate, and back.
        let hostile = matches!(viewer.relation(st), view::relation::Relation::Hostile);
        if hostile != v.hostile {
            v.hostile = hostile;
            v.glow = view::palette::side::of(hostile).glow;
            // The side colours are in the ship's slot: rewrite them below.
            (v.sent.0, v.sent.2) = (UNSENT.0, UNSENT.2);
        }
        let side = view::palette::side::of(v.hostile);
        let flash = view::anim::fade(now - v.flash_at, 4.0);
        let hit_age = (now - v.hit_at).min(3.0) as f32;
        let boost = v.boost.value(now, BOOST_UP, BOOST_DOWN);
        let q = |x: f32, step: f32| (x / step).round() * step;
        let hull_state = Vec4::new(q(1.0 - st.hull, 0.02), q(flash, 0.02), q(boost, 0.05), v.seed);
        if hull_state != v.sent.0 {
            v.sent.0 = hull_state;
            states.put(v.slot, HULL, hull_state);
            states.put(v.slot, GLOW, v4(v.glow, 0.0));
            states.put(v.slot, ENGINE, v4(side.engine, 0.0));
            states.put(v.slot, CORE, v4(side.engine_core, 0.0));
        }
        let drive = Vec2::new(q(v.throttle, 0.05), q(boost, 0.05));
        if drive != v.sent.1 {
            v.sent.1 = drive;
            states.put(v.slot, DRIVE, drive.extend(0.0).extend(0.0));
        }
        let shield = Vec4::new(q(st.shield, 0.02), v.hit_dir.x, v.hit_dir.y, hit_age);
        if shield != v.sent.2 {
            v.sent.2 = shield;
            states.put(v.slot, SHIELD, v4(side.shield, shield.x));
            states.put(v.slot, HIT, v.hit_dir.extend(shield.w));
        }
        let show = view::anim::shield_shown(st.shield, hit_age);
        if let Some((b, shown)) = v.shield
            && shown != show
            && let Ok(mut vis) = bubbles.get_mut(b)
        {
            *vis = if show { Visibility::Inherited } else { Visibility::Hidden };
            v.shield = Some((b, show));
        }
    }
    states.flush(&mut mats);
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

/// Engine detail by size on screen (`view::anim::engine_lod`): far ships drop their nozzle glows
/// and bores, dots drop their plumes. Runs before visibility is propagated, after the frame's moves.
fn engine_lod(cam: Query<(&GlobalTransform, &Camera, &Projection), With<crate::control::MainCamera>>, mut ships: Query<(&mut ShipVis, &Transform)>, mut fx: Query<&mut Visibility, Without<ShipVis>>) {
    let _prof = shared::prof::span("ships::engine_lod");
    let Ok((c, camera, Projection::Perspective(p))) = cam.single() else { return };
    let h = camera.logical_viewport_size().map_or(720.0, |v| v.y);
    for (mut v, tf) in &mut ships {
        let px = view::anim::screen_radius(v.radius, tf.translation.distance(c.translation()), h, p.fov);
        let lod = view::anim::engine_lod(px, v.lod);
        if lod == v.lod {
            continue;
        }
        v.lod = lod;
        let shown = |on: bool| if on { Visibility::Inherited } else { Visibility::Hidden };
        for (list, on) in [(&v.plume_fx, lod != EngineLod::Off), (&v.nozzles, lod == EngineLod::Full)] {
            for e in list {
                if let Ok(mut vis) = fx.get_mut(*e) {
                    vis.set_if_neq(shown(on));
                }
            }
        }
    }
}

/// Keep nozzle glow sprites facing the camera.
fn face_nozzle_glows(cam: Query<&GlobalTransform, With<crate::control::MainCamera>>, mut q: Query<(&mut Transform, &ChildOf, &Visibility), With<NozzleGlow>>, globals: Query<&GlobalTransform, Without<NozzleGlow>>) {
    let _prof = shared::prof::span("ships::face_nozzle_glows");
    let Ok(c) = cam.single() else { return };
    let cam_rot = c.rotation();
    for (mut t, parent, vis) in &mut q {
        if *vis == Visibility::Hidden {
            continue;
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn ship(id: EntityId) -> EntityState {
        EntityState { id, kind: EntityKind::Player, name: "T".into(), hull_class: Some(HullClass::Bastion), rank: 1, livery: 0, pos: [0.0; 3], rot: [0.0, 0.0, 0.0, 1.0], vel: [0.0, 0.0, -90.0], hull: 0.55, shield: 0.8, radius: 20.0, fleet: None, wanted: false, boosting: true, mining: None, docked: false, owner: None, alliance: None, class: Some(SizeClass::Capital), species: None }
    }

    /// Everything the shaders get from one ship's slot, without its pattern seed.
    fn shown(world: &World, e: Entity) -> Vec<Vec4> {
        let v = world.get::<ShipVis>(e).unwrap();
        let st = world.resource::<ShipStates>();
        let mut out: Vec<Vec4> = (0..SLOT).map(|at| st.get(v.slot, at)).collect();
        out[HULL].w = 0.0;
        out
    }

    /// Runs the real `update_ship_fx` without a window or GPU: ship 1 is there from the start, ship 2
    /// (`hostile` as first drawn) from `late_frame`. Returns both at 8 s.
    fn run(late_frame: u32, hostile: bool) -> (Vec<Vec4>, Vec<Vec4>) {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()))
            .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f64(1.0 / 60.0)))
            .init_asset::<EngineMat>()
            .init_asset::<NozzleMat>()
            .init_asset::<ShieldMat>()
            .init_asset::<HullMat>()
            .init_asset::<ShaderBuffer>()
            .add_systems(Update, update_ship_fx);
        let states = ShipStates::new(&mut app.world_mut().resource_mut::<Assets<ShaderBuffer>>());
        app.insert_resource(states);
        let mut s = Session::default();
        s.snaps.push_back(crate::state::Snap { at: 0.0, ents: [(1, ship(1)), (2, ship(2))].into_iter().collect() });
        app.insert_resource(s);
        let spawn = |app: &mut App, id, hostile| {
            let mut v = ShipVis::new(&ship(id), Vec3::ZERO);
            v.throttle = 0.7;
            v.boost.set(true, 0.0, BOOST_UP, BOOST_DOWN);
            v.hostile = hostile;
            v.slot = app.world_mut().resource_mut::<ShipStates>().alloc();
            app.world_mut().spawn(v).id()
        };
        let a = spawn(&mut app, 1, false);
        let mut b = None;
        for frame in 0..480 {
            if frame == late_frame {
                b = Some(spawn(&mut app, 2, hostile));
            }
            app.update();
        }
        (shown(app.world(), a), shown(app.world(), b.unwrap()))
    }

    #[test]
    fn ships_keep_their_own_state_slots_and_freed_slots_are_reused() {
        let mut buffers = Assets::<ShaderBuffer>::default();
        let mut st = ShipStates::new(&mut buffers);
        let slots: Vec<u32> = (0..300).map(|_| st.alloc()).collect();
        assert_eq!(slots.iter().copied().collect::<std::collections::HashSet<_>>().len(), 300, "every ship its own slot");
        assert!(st.slots.len() >= 300 && st.grown, "the buffer grows past its first 256 slots");
        st.put(7, SHIELD, Vec4::new(0.5, 0.0, 1.0, 3.0));
        assert_eq!(st.get(7, SHIELD), Vec4::new(0.5, 0.0, 1.0, 3.0));
        assert_eq!(st.get(7, HIT), Vec4::ZERO, "other fields untouched");
        assert_eq!(st.get(8, SHIELD), Vec4::ZERO, "neighbours untouched");
        st.free.push(7);
        assert_eq!(st.alloc(), 7, "a despawned ship's slot is reused");
    }

    #[test]
    fn fx_materials_are_shared_between_ships() {
        let mut st = ShipStates::new(&mut Assets::<ShaderBuffer>::default());
        let (mut engines, mut nozzles, mut shields) = (Assets::<EngineMat>::default(), Assets::<NozzleMat>::default(), Assets::<ShieldMat>::default());
        let torch = EngineParams { color: Vec4::ZERO, core: Vec4::ZERO, seed: Vec4::new(0.0, 1.0, 0.0, 0.0) };
        let ion = EngineParams { seed: Vec4::new(0.0, 0.8, 1.0, 0.0), ..torch };
        let a = st.engine(&mut engines, torch);
        assert_eq!(a, st.engine(&mut engines, torch), "same style, same plume material");
        assert_ne!(a, st.engine(&mut engines, ion));
        let n = NozzleParams { shape: Vec4::new(0.75, 1.0, 0.0, 0.0) };
        assert_eq!(st.nozzle(&mut nozzles, n), st.nozzle(&mut nozzles, n));
        assert_eq!(st.shield(&mut shields), st.shield(&mut shields));
        assert_eq!((engines.len(), nozzles.len(), shields.len()), (2, 1, 1));
        assert_eq!(plume_tag(5, 3), MeshTag::new(5 | 3 << 24), "slot low, plume index high");
    }

    #[test]
    fn a_ship_drawn_late_shows_the_same_state() {
        // docs/HARNESS.md "same command → identical image": the capital art scene once flipped between
        // two images because a model that loaded after the engine state settled kept default plumes.
        // The state is per slot now, written whether or not the model has loaded.
        for late in [1, 30, 200] {
            let (early, late_shown) = run(late, false);
            assert_eq!(early, late_shown, "drawn from frame {late}");
        }
        let (friendly, ship) = run(1, false);
        assert!((ship[DRIVE].x - 0.7).abs() < 1e-5 && ship[DRIVE].y == 1.0, "throttle and boost");
        assert!((ship[SHIELD].w - 0.8).abs() < 1e-5, "shield fraction");
        assert_eq!(ship[ENGINE], v4(view::palette::side::of(false).engine, 0.0));
        // Drawn hostile first, then known as a friend: plumes, nozzle glows and shield turn friendly.
        let (_, was_hostile) = run(30, true);
        assert_eq!(friendly, was_hostile);
        assert_ne!(view::palette::side::of(true).engine, view::palette::side::of(false).engine, "the test can see the difference");
    }

    #[test]
    fn a_shield_bubble_is_drawn_only_while_it_shows_something() {
        // 2700 idle, fully transparent bubbles were drawn every frame of the 1000-captain battle.
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()))
            .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f64(1.0 / 60.0)))
            .init_asset::<EngineMat>()
            .init_asset::<NozzleMat>()
            .init_asset::<ShieldMat>()
            .init_asset::<HullMat>()
            .init_asset::<ShaderBuffer>()
            .add_systems(Update, update_ship_fx);
        let states = ShipStates::new(&mut app.world_mut().resource_mut::<Assets<ShaderBuffer>>());
        app.insert_resource(states);
        let mut s = Session::default();
        s.snaps.push_back(crate::state::Snap { at: 0.0, ents: [(1, ship(1))].into_iter().collect() });
        app.insert_resource(s);
        let bubble = app.world_mut().spawn(Visibility::Hidden).id();
        let mut v = ShipVis::new(&ship(1), Vec3::ZERO);
        v.shield = Some((bubble, false));
        let e = app.world_mut().spawn(v).id();
        let vis = |app: &App| *app.world().get::<Visibility>(bubble).unwrap();
        for _ in 0..60 {
            app.update();
        }
        assert_eq!(vis(&app), Visibility::Hidden, "a full shield nobody hits draws nothing");
        // A hit: the ripple shows, then the bubble is hidden again once it has faded.
        let now = app.world().resource::<Time<Real>>().elapsed_secs_f64();
        app.world_mut().get_mut::<ShipVis>(e).unwrap().hit_at = now;
        app.update();
        assert_eq!(vis(&app), Visibility::Inherited);
        for _ in 0..150 {
            app.update();
        }
        assert_eq!(vis(&app), Visibility::Hidden);
        // Nearly empty: it flickers, so it is drawn.
        app.world_mut().resource_mut::<Session>().snaps.back_mut().unwrap().ents.get_mut(&1).unwrap().shield = 0.1;
        app.update();
        assert_eq!(vis(&app), Visibility::Inherited);
    }
}

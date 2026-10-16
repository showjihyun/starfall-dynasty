//! Ship interior panel (third person): a live view of the command bridge (cruisers, capitals) or
//! the cockpit (fighters). Built in Blender (`tools/blender/build_bridge.py`).
//!
//! - The interior lives on its own render layer far from the battle and is filmed by a panel
//!   camera into a texture that egui shows on the left of the HUD.
//! - Its big viewscreen / canopy shows the real battle: a feed camera on the ship's `Bridge`
//!   node (or the nose) renders forward into a second texture.
//! - The holo table spins; in combat the bridge goes to red alert (pulsing strips, red lights)
//!   and the panel camera shakes when the hull is hit.
//! - `I` toggles the panel (off by default). It is hidden in first person (that *is* the bridge view).

use crate::control::{CamState, MainCamera};
use crate::ships::{Models, ShipVis, Visuals};
use crate::state::{Screen, Session};
use bevy::camera::visibility::RenderLayers;
use bevy::camera::{Hdr, RenderTarget};
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::gltf::GltfMaterialName;
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use bevy::world_serialization::{WorldAssetRoot, WorldInstanceReady};
use bevy_egui::{EguiContexts, EguiTextureHandle, egui};
use shared::ships::SizeClass;

const LAYER: usize = 1;
/// Far below the play area, so nothing in space ever comes near it.
const ORIGIN: Vec3 = Vec3::new(0.0, -80_000.0, 0.0);
const PANEL_PX: UVec2 = UVec2::new(800, 450);
/// The viewscreens are built about 3:1.
const FEED_PX: UVec2 = UVec2::new(900, 300);

pub struct InteriorPlugin;

impl Plugin for InteriorPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup)
            .add_systems(Update, (toggle, manage, drive_cameras, animate).chain().run_if(in_state(Screen::Playing)))
            .add_systems(bevy_egui::EguiPrimaryContextPass, panel_ui.after(crate::ui::ui_system).run_if(in_state(Screen::Playing)));
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Bridge,
    Cockpit,
}

#[derive(Resource)]
pub struct Interior {
    pub show: bool,
    panel: Handle<Image>,
    feed_mat: Handle<StandardMaterial>,
    tex: Option<egui::TextureId>,
    kind: Option<Kind>,
    root: Option<Entity>,
    ready: bool,
    anchor: Option<Entity>,
    look: Option<Entity>,
    holo: Vec<Entity>,
    alert_mats: Vec<Handle<StandardMaterial>>,
    lights: Vec<Entity>,
    visible: bool,
}

#[derive(Component)]
struct PanelCam;

#[derive(Component)]
struct FeedCam;

#[derive(Component)]
struct InteriorRoot;

#[derive(Component)]
struct InteriorLight(f32);

fn target(images: &mut Assets<Image>, size: UVec2) -> Handle<Image> {
    images.add(Image::new_target_texture(size.x, size.y, TextureFormat::Rgba8UnormSrgb, None))
}

fn setup(mut commands: Commands, mut images: ResMut<Assets<Image>>, mut mats: ResMut<Assets<StandardMaterial>>) {
    let panel = target(&mut images, PANEL_PX);
    let feed = target(&mut images, FEED_PX);
    let feed_mat = mats.add(StandardMaterial { base_color_texture: Some(feed.clone()), unlit: true, ..default() });
    commands.spawn((
        PanelCam,
        Camera3d::default(),
        Hdr,
        Camera { order: -2, is_active: false, clear_color: ClearColorConfig::Custom(Color::srgb(0.01, 0.012, 0.02)), ..default() },
        RenderTarget::Image(panel.clone().into()),
        Tonemapping::TonyMcMapface,
        Bloom { intensity: 0.12, ..Bloom::NATURAL },
        Projection::Perspective(PerspectiveProjection { fov: 62f32.to_radians(), near: 0.05, far: 200.0, ..default() }),
        RenderLayers::layer(LAYER),
        Transform::from_translation(ORIGIN),
    ));
    commands.spawn((
        FeedCam,
        Camera3d::default(),
        Camera { order: -3, is_active: false, clear_color: ClearColorConfig::Custom(Color::BLACK), ..default() },
        RenderTarget::Image(feed.into()),
        Tonemapping::TonyMcMapface,
        // Vertical FOV for a ~100° horizontal panorama at 3:1.
        Projection::Perspective(PerspectiveProjection { fov: 36f32.to_radians(), near: 1.0, far: 1_000_000.0, ..default() }),
        Transform::default(),
    ));
    // Off by default for now (still being polished); `I` opens it.
    commands.insert_resource(Interior { show: false, panel, feed_mat, tex: None, kind: None, root: None, ready: false, anchor: None, look: None, holo: vec![], alert_mats: vec![], lights: vec![], visible: false });
}

fn toggle(keys: Res<ButtonInput<KeyCode>>, mut it: ResMut<Interior>, wants: Option<Res<bevy_egui::input::EguiWantsInput>>) {
    if wants.is_some_and(|w| w.wants_any_keyboard_input()) {
        return;
    }
    if keys.just_pressed(KeyCode::KeyI) {
        it.show = !it.show;
    }
}

/// Keeps the right interior loaded for the current ship size and decides whether it is shown.
fn manage(mut commands: Commands, s: Res<Session>, cam: Res<CamState>, models: Res<Models>, mut it: ResMut<Interior>) {
    let _prof = shared::prof::span("interior::manage");
    let kind = s.profile.as_ref().map(|p| if p.ship.size_class() == SizeClass::Small { Kind::Cockpit } else { Kind::Bridge });
    if kind != it.kind {
        if let Some(r) = it.root.take() {
            commands.entity(r).despawn();
        }
        it.kind = kind;
        it.ready = false;
        it.anchor = None;
        it.look = None;
        it.holo.clear();
        it.alert_mats.clear();
        it.lights.clear();
        let name = match kind {
            Some(Kind::Bridge) => "bridge_interior",
            Some(Kind::Cockpit) => "cockpit_interior",
            None => "",
        };
        if let Some(h) = models.handle(name) {
            let root = commands.spawn((InteriorRoot, WorldAssetRoot(h), Transform::from_translation(ORIGIN), RenderLayers::layer(LAYER))).observe(on_ready).id();
            it.root = Some(root);
        }
    }
    it.visible = it.show && it.ready && !cam.first_person && s.me.is_some() && s.dead.is_none();
}

#[allow(clippy::too_many_arguments)]
fn on_ready(
    ev: On<WorldInstanceReady>,
    mut commands: Commands,
    mut it: ResMut<Interior>,
    children: Query<&Children>,
    names: Query<&Name>,
    mesh_mats: Query<(&MeshMaterial3d<StandardMaterial>, Option<&GltfMaterialName>)>,
) {
    if it.root != Some(ev.entity) {
        return;
    }
    let cockpit = it.kind == Some(Kind::Cockpit);
    for d in children.iter_descendants(ev.entity) {
        commands.entity(d).insert(RenderLayers::layer(LAYER));
        if let Ok(name) = names.get(d) {
            let n = name.as_str();
            if n == "CamAnchor" {
                it.anchor = Some(d);
            } else if n == "CamLook" {
                it.look = Some(d);
            } else if n == "Holo" {
                it.holo.push(d);
            } else if n.starts_with("Light_") {
                let (intensity, range) = if cockpit { (60_000.0, 6.0) } else { (2_500_000.0, 32.0) };
                let l = commands.spawn((InteriorLight(intensity), PointLight { color: Color::srgb(1.0, 0.92, 0.82), intensity, range, shadow_maps_enabled: false, ..default() }, RenderLayers::layer(LAYER), Transform::default())).id();
                commands.entity(d).add_child(l);
                it.lights.push(l);
            }
        }
        if let Ok((MeshMaterial3d(h), Some(g))) = mesh_mats.get(d) {
            match g.0.as_str() {
                "ViewScreen" => {
                    commands.entity(d).insert(MeshMaterial3d(it.feed_mat.clone()));
                }
                "Alert" if !it.alert_mats.contains(h) => it.alert_mats.push(h.clone()),
                _ => {}
            }
        }
    }
    it.ready = true;
    info!("interior ready: {:?}, {} lights", it.kind, it.lights.len());
}

#[allow(clippy::too_many_arguments)]
fn drive_cameras(
    time: Res<Time<Real>>,
    s: Res<Session>,
    vis: Res<Visuals>,
    it: Res<Interior>,
    ships: Query<(&ShipVis, &Transform), (Without<PanelCam>, Without<FeedCam>)>,
    globals: Query<&GlobalTransform>,
    mut panel: Query<(&mut Camera, &mut Transform), (With<PanelCam>, Without<FeedCam>)>,
    mut feed: Query<(&mut Camera, &mut Transform), (With<FeedCam>, Without<PanelCam>)>,
    mut frame: Local<u32>,
) {
    let _prof = shared::prof::span("interior::drive_cameras");
    *frame += 1;
    let t = time.elapsed_secs();
    if let Ok((mut cam, mut tf)) = panel.single_mut() {
        // 30 fps is plenty for a slow-drifting inset; odd frames only.
        cam.is_active = it.visible && *frame % 2 == 1;
        if let (Some(a), Some(l)) = (it.anchor.and_then(|e| globals.get(e).ok()), it.look.and_then(|e| globals.get(e).ok())) {
            // Slow handheld drift; a jolt when the hull takes a hit.
            let span = if it.kind == Some(Kind::Bridge) { 0.25 } else { 0.03 };
            let drift = Vec3::new((t * 0.21).sin(), (t * 0.17).cos() * 0.5, (t * 0.13).sin() * 0.6) * span;
            let shake = Vec3::new((t * 53.0).sin(), (t * 47.0).cos(), 0.0) * s.gauge.flash * span * 0.8;
            tf.translation = a.translation() + drift + shake;
            tf.look_at(l.translation() + drift * 0.3, Vec3::Y);
        }
    }
    if let Ok((mut cam, mut tf)) = feed.single_mut() {
        // Every third even frame: the screen is a small part of a small panel, and never
        // renders on the same frame as the panel itself.
        cam.is_active = it.visible && *frame % 6 == 0;
        let Some((sv, stf)) = s.me.and_then(|m| vis.map.get(&m)).and_then(|e| ships.get(*e).ok()) else { return };
        let fwd = stf.rotation * Vec3::NEG_Z;
        let up = stf.rotation * Vec3::Y;
        let pos = sv.bridge.and_then(|b| globals.get(b).ok()).map(|g| g.translation() + up * sv.radius * 0.04).unwrap_or(stf.translation + fwd * sv.radius * 1.05 + up * sv.radius * 0.12);
        *tf = Transform::from_translation(pos).looking_to(fwd, up);
    }
}

fn animate(time: Res<Time<Real>>, s: Res<Session>, it: Res<Interior>, mut tfs: Query<&mut Transform>, mut mats: ResMut<Assets<StandardMaterial>>, mut lights: Query<(&InteriorLight, &mut PointLight)>, mut last_alert: Local<Option<bool>>) {
    let _prof = shared::prof::span("interior::animate");
    if !it.visible {
        return;
    }
    let t = time.elapsed_secs();
    for h in &it.holo {
        if let Ok(mut tf) = tfs.get_mut(*h) {
            tf.rotation = Quat::from_rotation_y(t * 0.35);
        }
    }
    let now = time.elapsed_secs_f64();
    let alert = s.engagement.is_some() || now - s.last_combat < 8.0 || s.gauge.flash > 0.0;
    let pulse = 0.5 + 0.5 * (t * 5.0).sin();
    // Strips and lights change every frame only while alarmed; otherwise once on the transition.
    if alert || *last_alert != Some(alert) {
        for h in &it.alert_mats {
            if let Some(mut m) = mats.get_mut(h) {
                m.emissive = if alert { LinearRgba::rgb(14.0, 0.8, 0.4) * (0.25 + pulse) } else { LinearRgba::rgb(0.4, 0.03, 0.02) };
            }
        }
        for (il, mut l) in &mut lights {
            l.color = if alert { Color::srgb(1.0, 0.35 + 0.15 * pulse, 0.3) } else { Color::srgb(1.0, 0.92, 0.82) };
            l.intensity = il.0 * if alert { 0.45 + 0.25 * pulse } else { 1.0 };
        }
        *last_alert = Some(alert);
    }
}

fn panel_ui(mut contexts: EguiContexts, mut it: ResMut<Interior>, s: Res<Session>, main: Query<&Camera, With<MainCamera>>) -> Result {
    let _prof = shared::prof::span("interior::panel_ui");
    if it.tex.is_none() {
        it.tex = Some(contexts.add_image(EguiTextureHandle::Strong(it.panel.clone())));
    }
    if !it.visible || main.is_empty() {
        return Ok(());
    }
    let ctx = contexts.ctx_mut()?;
    let ship = s.profile.as_ref().map(|p| p.ship.name.clone()).unwrap_or_default();
    let (title, w) = match it.kind {
        Some(Kind::Bridge) => (format!("함교 — 「{ship}」"), 440.0),
        _ => (format!("조종석 — 「{ship}」"), 360.0),
    };
    let tex = it.tex.unwrap();
    egui::Area::new("interior".into()).anchor(egui::Align2::LEFT_CENTER, egui::Vec2::new(14.0, 40.0)).show(ctx, |ui| {
        egui::Frame::popup(ui.style()).inner_margin(6.0).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(title).size(13.0).strong().color(egui::Color32::from_rgb(255, 205, 110)));
                let alert = s.engagement.is_some() || s.gauge.flash > 0.0;
                if alert {
                    ui.label(egui::RichText::new("● 적색 경보").size(11.0).color(egui::Color32::from_rgb(255, 80, 70)));
                }
                ui.label(egui::RichText::new("I 숨기기").size(10.0).color(egui::Color32::from_rgb(140, 150, 170)));
            });
            ui.add(egui::Image::new(egui::load::SizedTexture::new(tex, [w, w * PANEL_PX.y as f32 / PANEL_PX.x as f32])).corner_radius(4.0));
        });
    });
    Ok(())
}

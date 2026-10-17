//! Experimental ray-traced lighting (Bevy Solari), off by default: build with `--features raytracing`
//! (plus `raytracing_dlss` for the DLSS Ray Reconstruction denoiser, which needs the NVIDIA DLSS SDK,
//! the Vulkan SDK and clang — see docs/ART_DIRECTION.md §7) and run with `STARFALL_RT=1`.
//!
//! Spike findings (2026-10-17, RTX 4070 SUPER): Vulkan only (wgpu's DX12 backend has no ray query);
//! without DLSS the image is unusably noisy; Solari traces `StandardMaterial` meshes only, so our ships
//! (custom hull material), planets, star, plumes and shields stay outside the ray-traced scene. Raster
//! is the look of record; this exists to measure, not to ship.

use bevy::camera::CameraMainTextureUsages;
use bevy::mesh::Indices;
use bevy::prelude::*;
use bevy::render::render_resource::TextureUsages;
use bevy::solari::prelude::{RaytracingMesh3d, SolariLighting, SolariPlugins};

/// Whether this run asked for ray tracing.
pub fn wanted() -> bool {
    std::env::var("STARFALL_RT").is_ok_and(|v| v == "1")
}

/// DLSS needs the app's project id before `DefaultPlugins` (whether or not RT is switched on).
#[cfg(feature = "raytracing_dlss")]
pub fn dlss_project_id() -> bevy::anti_alias::dlss::DlssProjectId {
    bevy::anti_alias::dlss::DlssProjectId(bevy::asset::uuid::uuid!("8f3c2a7e-5b1d-4e62-9c0a-3d7b6f1e2a94"))
}

pub struct RaytracingPlugin;

impl Plugin for RaytracingPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(SolariPlugins).add_systems(Update, (setup_camera, trace_standard_meshes));
        info!("ray tracing (Solari) requested: needs Vulkan and a ray-query GPU");
    }
}

/// The main camera needs Solari's lighting, storage-capable main textures and no MSAA.
fn setup_camera(
    mut commands: Commands,
    cams: Query<Entity, (With<crate::control::MainCamera>, Without<SolariLighting>)>,
    #[cfg(feature = "raytracing_dlss")] dlss: Option<Res<bevy::anti_alias::dlss::DlssRayReconstructionSupported>>,
) {
    for cam in &cams {
        let mut c = commands.entity(cam);
        c.insert((SolariLighting::default(), CameraMainTextureUsages::default().with(TextureUsages::STORAGE_BINDING), Msaa::Off));
        #[cfg(feature = "raytracing_dlss")]
        if dlss.is_some() {
            c.insert(bevy::anti_alias::dlss::Dlss::<bevy::anti_alias::dlss::DlssRayReconstructionFeature>::default());
        }
    }
}

/// Solari accepts triangle lists with exactly position/normal/uv/tangent and 32-bit indices: convert
/// every `StandardMaterial` mesh (station, asteroids, props) and add it to the ray-traced scene.
fn trace_standard_meshes(mut commands: Commands, q: Query<(Entity, &Mesh3d), (With<MeshMaterial3d<StandardMaterial>>, Without<RaytracingMesh3d>)>, mut meshes: ResMut<Assets<Mesh>>) {
    for (e, Mesh3d(h)) in &q {
        let Some(mut mesh) = meshes.get_mut(h) else { continue };
        if !mesh.contains_attribute(Mesh::ATTRIBUTE_NORMAL) {
            continue;
        }
        let keep = [Mesh::ATTRIBUTE_POSITION.id, Mesh::ATTRIBUTE_NORMAL.id, Mesh::ATTRIBUTE_UV_0.id, Mesh::ATTRIBUTE_TANGENT.id];
        let extra: Vec<_> = mesh.attributes().map(|(a, _)| a.id).filter(|id| !keep.contains(id)).collect();
        for id in extra {
            mesh.remove_attribute(id);
        }
        let n = mesh.count_vertices();
        if !mesh.contains_attribute(Mesh::ATTRIBUTE_UV_0) {
            mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0f32, 0.0]; n]);
        }
        if let Some(Indices::U16(i)) = mesh.indices() {
            let v: Vec<u32> = i.iter().map(|x| *x as u32).collect();
            mesh.insert_indices(Indices::U32(v));
        }
        if mesh.indices().is_none() {
            mesh.insert_indices(Indices::U32((0..n as u32).collect()));
        }
        if !mesh.contains_attribute(Mesh::ATTRIBUTE_TANGENT) && mesh.generate_tangents().is_err() {
            mesh.insert_attribute(Mesh::ATTRIBUTE_TANGENT, vec![[1.0f32, 0.0, 0.0, 1.0]; n]);
        }
        commands.entity(e).insert(RaytracingMesh3d(h.clone()));
    }
}

//! Custom materials. Every shader is embedded in the binary (see `src/shaders/*.wesl`).

use bevy::{
    mesh::MeshVertexBufferLayoutRef,
    pbr::{ExtendedMaterial, MaterialExtension, MaterialPipeline, MaterialPipelineKey},
    prelude::*,
    render::render_resource::{AsBindGroup, RenderPipelineDescriptor, ShaderType, SpecializedMeshPipelineError},
    shader::ShaderRef,
};

pub struct MaterialsPlugin;

impl Plugin for MaterialsPlugin {
    fn build(&self, app: &mut App) {
        bevy::asset::embedded_asset!(app, "shaders/space.wesl");
        bevy::asset::embedded_asset!(app, "shaders/star.wesl");
        bevy::asset::embedded_asset!(app, "shaders/glow.wesl");
        bevy::asset::embedded_asset!(app, "shaders/planet.wesl");
        bevy::asset::embedded_asset!(app, "shaders/ring.wesl");
        bevy::asset::embedded_asset!(app, "shaders/hull.wesl");
        bevy::asset::embedded_asset!(app, "shaders/engine.wesl");
        bevy::asset::embedded_asset!(app, "shaders/shield.wesl");
        bevy::asset::embedded_asset!(app, "shaders/beam.wesl");
        bevy::asset::embedded_asset!(app, "shaders/explosion.wesl");
        bevy::asset::embedded_asset!(app, "shaders/warp.wesl");
        app.add_plugins((
            MaterialPlugin::<SpaceMat>::default(),
            MaterialPlugin::<StarMat>::default(),
            MaterialPlugin::<GlowMat>::default(),
            MaterialPlugin::<PlanetMat>::default(),
            MaterialPlugin::<RingMat>::default(),
            MaterialPlugin::<HullMat>::default(),
            MaterialPlugin::<EngineMat>::default(),
            MaterialPlugin::<ShieldMat>::default(),
            MaterialPlugin::<BeamMat>::default(),
            MaterialPlugin::<ExplosionMat>::default(),
            MaterialPlugin::<WarpMat>::default(),
        ));
    }
}

fn no_cull(d: &mut RenderPipelineDescriptor) {
    d.primitive.cull_mode = None;
}

fn no_depth_write(d: &mut RenderPipelineDescriptor) {
    if let Some(ds) = d.depth_stencil.as_mut() {
        ds.depth_write_enabled = Some(false).into();
    }
}

/// Declares a simple uniform-driven material with one params block at binding 0.
macro_rules! simple_material {
    ($name:ident, $params:ident { $($field:ident),* $(,)? }, $shader:literal, $alpha:expr, cull: $cull:expr, zwrite: $zw:expr) => {
        #[derive(Clone, Copy, Debug, Default, ShaderType, Reflect)]
        pub struct $params {
            $(pub $field: Vec4,)*
        }

        #[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
        pub struct $name {
            #[uniform(0)]
            pub p: $params,
        }

        impl Material for $name {
            fn fragment_shader() -> ShaderRef {
                concat!("embedded://client/shaders/", $shader).into()
            }
            fn alpha_mode(&self) -> AlphaMode {
                $alpha
            }
            fn enable_prepass() -> bool {
                false
            }
            fn enable_shadows() -> bool {
                false
            }
            fn specialize(_p: &MaterialPipeline, d: &mut RenderPipelineDescriptor, _l: &MeshVertexBufferLayoutRef, _k: MaterialPipelineKey<Self>) -> Result<(), SpecializedMeshPipelineError> {
                if !$cull {
                    no_cull(d);
                }
                if !$zw {
                    no_depth_write(d);
                }
                Ok(())
            }
        }
    };
}

simple_material!(SpaceMat, SpaceParams { neb_a, neb_b, star_dir, star_col }, "space.wesl", AlphaMode::Opaque, cull: false, zwrite: false);
simple_material!(StarMat, StarParams { color, center }, "star.wesl", AlphaMode::Opaque, cull: true, zwrite: true);
simple_material!(GlowMat, GlowParams { color, center, shape }, "glow.wesl", AlphaMode::Add, cull: true, zwrite: false);
simple_material!(PlanetMat, PlanetParams { center, kind, col_a, col_b, atmo, star }, "planet.wesl", AlphaMode::Opaque, cull: true, zwrite: true);
simple_material!(RingMat, RingParams { center, color, radii, star }, "ring.wesl", AlphaMode::Premultiplied, cull: false, zwrite: false);
simple_material!(EngineMat, EngineParams { color, core, seed }, "engine.wesl", AlphaMode::Add, cull: false, zwrite: false);
simple_material!(ShieldMat, ShieldParams { color, hit, center }, "shield.wesl", AlphaMode::Add, cull: true, zwrite: false);
simple_material!(BeamMat, BeamParams { color, flow, life }, "beam.wesl", AlphaMode::Add, cull: false, zwrite: false);
simple_material!(ExplosionMat, ExplosionParams { life, state }, "explosion.wesl", AlphaMode::Add, cull: true, zwrite: false);
simple_material!(WarpMat, WarpParams { color, accent }, "warp.wesl", AlphaMode::Add, cull: false, zwrite: false);

/// Hull extension over the Blender-baked StandardMaterial.
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone, Default)]
pub struct HullExt {
    #[uniform(100)]
    pub p: HullParams,
}

#[derive(Clone, Copy, Debug, Default, ShaderType, Reflect)]
pub struct HullParams {
    /// rgb livery, w = part kind (0 hull, 1 livery, 2 glow, 3 engine core).
    pub livery: Vec4,
    /// rgb glow tint, w = strength.
    pub glow: Vec4,
    /// x damage, y hit flash, z boost, w seed.
    pub state: Vec4,
}

impl MaterialExtension for HullExt {
    fn fragment_shader() -> ShaderRef {
        "embedded://client/shaders/hull.wesl".into()
    }
}

pub type HullMat = ExtendedMaterial<StandardMaterial, HullExt>;

pub fn v4(c: [f32; 3], w: f32) -> Vec4 {
    Vec4::new(c[0], c[1], c[2], w)
}

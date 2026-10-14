use bevy::{
    mesh::MeshVertexBufferLayoutRef,
    pbr::{MaterialPipeline, MaterialPipelineKey},
    prelude::*,
    render::render_resource::{AsBindGroup, RenderPipelineDescriptor, SpecializedMeshPipelineError},
    render::view::screenshot::{save_to_disk, Screenshot},
    shader::ShaderRef,
};

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
struct Spike {
    #[uniform(0)]
    tint: LinearRgba,
}

impl Material for Spike {
    fn fragment_shader() -> ShaderRef {
        "embedded://client/shaders/spike.wesl".into()
    }
    fn specialize(_p: &MaterialPipeline, d: &mut RenderPipelineDescriptor, _l: &MeshVertexBufferLayoutRef, _k: MaterialPipelineKey<Self>) -> Result<(), SpecializedMeshPipelineError> {
        d.primitive.cull_mode = None;
        Ok(())
    }
}

fn main() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins);
    bevy::asset::embedded_asset!(app, "shaders/spike.wesl");
    app.add_plugins(MaterialPlugin::<Spike>::default())
        .add_systems(Startup, setup)
        .add_systems(Update, shot)
        .run();
}

fn setup(mut c: Commands, mut meshes: ResMut<Assets<Mesh>>, mut mats: ResMut<Assets<Spike>>) {
    c.spawn((Mesh3d(meshes.add(Sphere::new(50.0).mesh().uv(32, 16))), MeshMaterial3d(mats.add(Spike { tint: LinearRgba::rgb(1.0, 0.5, 0.8) }))));
    c.spawn((Camera3d::default(), Transform::from_xyz(0.0, 0.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y)));
}

fn shot(mut c: Commands, mut n: Local<u32>, mut exit: MessageWriter<AppExit>) {
    *n += 1;
    if *n == 60 {
        c.spawn(Screenshot::primary_window()).observe(save_to_disk("spike.png"));
    }
    if *n == 90 {
        exit.write(AppExit::Success);
    }
}

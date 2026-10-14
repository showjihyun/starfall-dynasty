//! Builds the 3D environment of the current star system from the shared deterministic layout.

use crate::materials::*;
use crate::meshes;
use crate::state::{Screen, Session};
use bevy::light::{GeneratedEnvironmentMapLight, NotShadowCaster};
use bevy::prelude::*;
use shared::galaxy::{self, Layout, Ore, PlanetKind, PoiKind};

pub struct ScenePlugin;

impl Plugin for ScenePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Env>().add_systems(Update, (build_system, follow_sky, spin_things).run_if(in_state(Screen::Playing)));
    }
}

#[derive(Component)]
pub struct SystemScene;

#[derive(Component)]
pub struct Skybox;

#[derive(Component)]
pub struct Spin(pub Vec3);

#[derive(Component)]
pub struct PoiMarker(pub u32);

#[derive(Resource, Default)]
pub struct Env {
    pub epoch: u32,
    pub layout: Option<Layout>,
    pub env_map: Option<Handle<Image>>,
}

pub const SKY_RADIUS: f32 = 400_000.0;

#[allow(clippy::too_many_arguments)]
fn build_system(
    mut commands: Commands,
    s: Res<Session>,
    mut env: ResMut<Env>,
    old: Query<Entity, With<SystemScene>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut std_mats: ResMut<Assets<StandardMaterial>>,
    mut space: ResMut<Assets<SpaceMat>>,
    mut stars: ResMut<Assets<StarMat>>,
    mut glows: ResMut<Assets<GlowMat>>,
    mut planets: ResMut<Assets<PlanetMat>>,
    mut rings: ResMut<Assets<RingMat>>,
    mut cams: Query<&mut GeneratedEnvironmentMapLight, With<Camera3d>>,
) {
    if s.system_epoch == env.epoch || s.system_epoch == 0 {
        return;
    }
    env.epoch = s.system_epoch;
    for e in &old {
        commands.entity(e).despawn();
    }
    let def = galaxy::system(s.system);
    let lay = galaxy::system_layout(s.system);
    let star_col = def.star.color();
    let star_dir = (Vec3::ZERO - lay.station).normalize();

    // Sky.
    commands.spawn((
        SystemScene,
        Skybox,
        NotShadowCaster,
        Mesh3d(meshes.add(Sphere::new(SKY_RADIUS).mesh().uv(96, 48))),
        MeshMaterial3d(space.add(SpaceMat {
            p: SpaceParams {
                neb_a: v4(lay.nebula[0], lay.nebula_density),
                neb_b: v4(lay.nebula[1], s.system as f32 * 13.7 + 3.0),
                star_dir: star_dir.extend(1.0),
                star_col: v4(star_col, 1.0),
            },
        })),
    ));

    // Image-based lighting from a matching cubemap.
    let cube = images.add(meshes::sky_cubemap(lay.nebula[0], lay.nebula[1], star_dir, star_col, s.system as u32 * 7 + 1));
    env.env_map = Some(cube.clone());
    for mut l in &mut cams {
        l.environment_map = cube.clone();
    }

    // Star with corona.
    let sr = lay.star_radius;
    commands.spawn((SystemScene, NotShadowCaster, Spin(Vec3::Y * 0.01), Mesh3d(meshes.add(Sphere::new(sr).mesh().ico(6).unwrap())), MeshMaterial3d(stars.add(StarMat { p: StarParams { color: v4(star_col, 14.0), center: Vec4::new(0.0, 0.0, 0.0, sr) } }))));
    commands.spawn((
        SystemScene,
        NotShadowCaster,
        Mesh3d(meshes.add(Sphere::new(sr * 4.0).mesh().ico(4).unwrap())),
        MeshMaterial3d(glows.add(GlowMat { p: GlowParams { color: v4(star_col, 2.2), center: Vec4::new(0.0, 0.0, 0.0, sr * 0.98), shape: Vec4::new(7.0, 0.5, sr * 4.0, 0.0) } })),
    ));
    commands.spawn((
        SystemScene,
        DirectionalLight { color: Color::linear_rgb(star_col[0], star_col[1], star_col[2]), illuminance: 2_600.0, shadow_maps_enabled: true, ..default() },
        Transform::from_translation(Vec3::ZERO).looking_at(lay.station, Vec3::Y),
    ));

    // Planets.
    for (i, pl) in lay.planets.iter().enumerate() {
        let (kind, a, b, atmo, aw) = match pl.kind {
            PlanetKind::Rocky => (0.0, [0.45, 0.38, 0.3], [0.62, 0.56, 0.5], [0.6, 0.6, 0.7], 0.25),
            PlanetKind::Ocean => (1.0, [0.04, 0.17, 0.42], [0.22, 0.36, 0.16], [0.35, 0.6, 1.0], 1.0),
            PlanetKind::Gas => {
                let h = (pl.seed % 3) as usize;
                let pal = [([0.82, 0.66, 0.46], [0.55, 0.33, 0.22]), ([0.45, 0.6, 0.85], [0.2, 0.3, 0.6]), ([0.75, 0.5, 0.7], [0.35, 0.2, 0.45])][h];
                (2.0, pal.0, pal.1, [pal.0[0], pal.0[1], pal.0[2]], 0.6)
            }
            PlanetKind::Ice => (3.0, [0.82, 0.9, 0.96], [0.45, 0.62, 0.8], [0.6, 0.8, 1.0], 0.5),
            PlanetKind::Lava => (4.0, [0.32, 0.26, 0.22], [2.2, 0.6, 0.1], [1.0, 0.4, 0.1], 0.5),
        };
        let center = pl.pos;
        commands.spawn((
            SystemScene,
            Mesh3d(meshes.add(Sphere::new(pl.radius).mesh().ico(6).unwrap())),
            Transform::from_translation(center),
            MeshMaterial3d(planets.add(PlanetMat {
                p: PlanetParams {
                    center: center.extend(pl.radius),
                    kind: Vec4::new(kind, (pl.seed % 997) as f32 * 0.37, 0.004 + (i as f32) * 0.002, 0.0),
                    col_a: v4(a, 1.0),
                    col_b: v4(b, 1.0),
                    atmo: v4(atmo, aw),
                    star: v4(star_col, 1.6),
                },
            })),
        ));
        commands.spawn((
            SystemScene,
            NotShadowCaster,
            Mesh3d(meshes.add(Sphere::new(pl.radius * 1.12).mesh().ico(5).unwrap())),
            Transform::from_translation(center),
            MeshMaterial3d(glows.add(GlowMat { p: GlowParams { color: v4(atmo, aw * 0.6), center: center.extend(pl.radius), shape: Vec4::new(6.0, 0.0, pl.radius * 1.12, 0.0) } })),
        ));
        if pl.ring {
            let (ri, ro) = (pl.radius * 1.35, pl.radius * 2.3);
            commands.spawn((
                SystemScene,
                NotShadowCaster,
                Mesh3d(meshes.add(meshes::annulus(ri, ro))),
                Transform::from_translation(center).with_rotation(Quat::from_euler(EulerRot::XYZ, 0.35, 0.0, 0.2)),
                MeshMaterial3d(rings.add(RingMat { p: RingParams { center: center.extend(pl.radius), color: v4(b, pl.seed as f32 * 0.01), radii: Vec4::new(ri, ro, 0.0, 0.0), star: v4(star_col, 1.4) } })),
            ));
        }
    }

    // Asteroid belts: a few mesh variants, materials per ore.
    let variants: Vec<Handle<Mesh>> = (0..6).map(|k| meshes.add(meshes::asteroid(k * 101 + s.system as u32))).collect();
    let rock = |c: [f32; 3], e: [f32; 3], r: f32| StandardMaterial { base_color: Color::srgb(c[0], c[1], c[2]), perceptual_roughness: r, metallic: 0.05, emissive: LinearRgba::rgb(e[0], e[1], e[2]), ..default() };
    let ferrite = std_mats.add(rock([0.34, 0.29, 0.26], [0.0; 3], 0.92));
    let lumen = std_mats.add(rock([0.3, 0.33, 0.36], [0.0, 0.012, 0.02], 0.85));
    let xenite = std_mats.add(rock([0.27, 0.23, 0.29], [0.02, 0.003, 0.035], 0.8));
    for a in &lay.asteroids {
        let m = match a.ore {
            Ore::Ferrite => ferrite.clone(),
            Ore::Lumen => lumen.clone(),
            Ore::Xenite => xenite.clone(),
        };
        let r = a.seed as f32;
        commands.spawn((
            SystemScene,
            AsteroidVis(a.id),
            Spin(Vec3::new((r * 0.13).sin(), (r * 0.71).cos(), (r * 0.37).sin()) * 0.05),
            Mesh3d(variants[(a.seed % 6) as usize].clone()),
            MeshMaterial3d(m),
            Transform::from_translation(a.pos).with_scale(Vec3::splat(a.radius)).with_rotation(Quat::from_euler(EulerRot::XYZ, r, r * 0.3, r * 0.7)),
        ));
    }

    spawn_station(&mut commands, &mut meshes, &mut std_mats, &mut glows, lay.station);
    spawn_jump_point(&mut commands, &mut meshes, &mut std_mats, &mut glows, lay.jump_point);

    for p in &lay.pois {
        let (col, size): ([f32; 3], f32) = match p.kind {
            PoiKind::Anomaly => ([0.3, 0.9, 1.0], 30.0),
            PoiKind::Ruin => ([1.0, 0.75, 0.35], 60.0),
            PoiKind::Derelict => ([0.9, 0.5, 0.3], 45.0),
            PoiKind::Signal => ([1.0, 0.3, 0.4], 20.0),
            PoiKind::AncientGate => ([0.7, 0.5, 1.0], 420.0),
        };
        let emis = |k: f32| StandardMaterial { base_color: Color::srgb(0.08, 0.08, 0.1), emissive: LinearRgba::rgb(col[0] * k, col[1] * k, col[2] * k), perceptual_roughness: 0.4, metallic: 0.6, ..default() };
        let mut e = commands.spawn((SystemScene, PoiMarker(p.id), Transform::from_translation(p.pos), Visibility::default()));
        e.with_children(|c| {
            match p.kind {
                PoiKind::Anomaly => {
                    c.spawn((Spin(Vec3::new(0.2, 0.5, 0.1)), Mesh3d(meshes.add(Sphere::new(size * 0.4).mesh().ico(1).unwrap())), MeshMaterial3d(std_mats.add(emis(8.0)))));
                }
                PoiKind::Ruin => {
                    let stone = std_mats.add(StandardMaterial { base_color: Color::srgb(0.42, 0.38, 0.33), perceptual_roughness: 0.95, ..default() });
                    for k in 0..7 {
                        let a = k as f32 / 7.0 * std::f32::consts::TAU;
                        let h = 40.0 + (k * 37 % 50) as f32;
                        c.spawn((Mesh3d(meshes.add(Cuboid::new(9.0, h, 9.0))), MeshMaterial3d(stone.clone()), Transform::from_xyz(a.cos() * size, h * 0.3, a.sin() * size).with_rotation(Quat::from_rotation_z(0.15 * (k as f32).sin()))));
                    }
                    c.spawn((Mesh3d(meshes.add(Torus::new(size * 0.35, size * 0.42))), MeshMaterial3d(std_mats.add(emis(4.0))), Transform::from_rotation(Quat::from_rotation_x(1.2))));
                }
                PoiKind::Derelict => {
                    let hull = std_mats.add(StandardMaterial { base_color: Color::srgb(0.25, 0.24, 0.23), metallic: 0.8, perceptual_roughness: 0.6, ..default() });
                    for k in 0..5 {
                        let f = k as f32;
                        c.spawn((Mesh3d(meshes.add(Cuboid::new(14.0 + f * 3.0, 8.0, 30.0 - f * 3.0))), MeshMaterial3d(hull.clone()), Transform::from_xyz(f * 12.0 - 20.0, (f * 1.7).sin() * 6.0, f * 9.0).with_rotation(Quat::from_euler(EulerRot::XYZ, f * 0.4, f * 0.9, f * 0.2))));
                    }
                }
                PoiKind::Signal => {
                    c.spawn((Spin(Vec3::Y), Mesh3d(meshes.add(Cone { radius: 6.0, height: 26.0 })), MeshMaterial3d(std_mats.add(emis(10.0)))));
                }
                PoiKind::AncientGate => {
                    let stone = std_mats.add(StandardMaterial { base_color: Color::srgb(0.2, 0.18, 0.24), metallic: 0.3, perceptual_roughness: 0.7, ..default() });
                    c.spawn((Mesh3d(meshes.add(Torus::new(size * 0.85, size))), MeshMaterial3d(stone), Transform::from_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2))));
                    c.spawn((Spin(Vec3::Z * 0.2), Mesh3d(meshes.add(Torus::new(size * 0.8, size * 0.82))), MeshMaterial3d(std_mats.add(emis(6.0))), Transform::from_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2))));
                }
            }
            c.spawn((NotShadowCaster, Mesh3d(meshes.add(Sphere::new(size * 3.0).mesh().ico(3).unwrap())), MeshMaterial3d(glows.add(GlowMat { p: GlowParams { color: v4(col, 0.5), center: p.pos.extend(size * 0.3), shape: Vec4::new(3.5, 2.0, size * 3.0, 0.0) } }))));
        });
    }
    env.layout = Some(lay);
}

#[derive(Component)]
pub struct AsteroidVis(pub u32);

fn spawn_station(commands: &mut Commands, meshes: &mut Assets<Mesh>, mats: &mut Assets<StandardMaterial>, glows: &mut Assets<GlowMat>, pos: Vec3) {
    let metal = mats.add(StandardMaterial { base_color: Color::srgb(0.55, 0.57, 0.6), metallic: 0.85, perceptual_roughness: 0.35, ..default() });
    let dark = mats.add(StandardMaterial { base_color: Color::srgb(0.12, 0.13, 0.15), metallic: 0.7, perceptual_roughness: 0.5, ..default() });
    let lights = mats.add(StandardMaterial { base_color: Color::BLACK, emissive: LinearRgba::rgb(3.0, 2.2, 1.1), ..default() });
    let blue = mats.add(StandardMaterial { base_color: Color::BLACK, emissive: LinearRgba::rgb(0.6, 2.5, 6.0), ..default() });
    let panel = mats.add(StandardMaterial { base_color: Color::srgb(0.05, 0.08, 0.2), metallic: 0.9, perceptual_roughness: 0.2, ..default() });
    commands
        .spawn((SystemScene, Transform::from_translation(pos), Visibility::default()))
        .with_children(|c| {
            c.spawn((Spin(Vec3::Y * 0.05), Transform::default(), Visibility::default())).with_children(|r| {
                r.spawn((Mesh3d(meshes.add(Torus::new(118.0, 132.0))), MeshMaterial3d(metal.clone())));
                r.spawn((Mesh3d(meshes.add(Torus::new(131.0, 133.0))), MeshMaterial3d(lights.clone())));
                for k in 0..6 {
                    let a = k as f32 / 6.0 * std::f32::consts::TAU;
                    r.spawn((Mesh3d(meshes.add(Cuboid::new(110.0, 5.0, 5.0))), MeshMaterial3d(dark.clone()), Transform::from_xyz(a.cos() * 62.0, 0.0, a.sin() * 62.0).with_rotation(Quat::from_rotation_y(-a))));
                    r.spawn((Mesh3d(meshes.add(Cuboid::new(16.0, 14.0, 26.0))), MeshMaterial3d(metal.clone()), Transform::from_xyz(a.cos() * 125.0, 0.0, a.sin() * 125.0).with_rotation(Quat::from_rotation_y(-a))));
                }
            });
            c.spawn((Mesh3d(meshes.add(Cylinder::new(24.0, 150.0))), MeshMaterial3d(metal.clone())));
            c.spawn((Mesh3d(meshes.add(Cylinder::new(30.0, 30.0))), MeshMaterial3d(dark.clone()), Transform::from_xyz(0.0, 60.0, 0.0)));
            c.spawn((Mesh3d(meshes.add(Cylinder::new(31.0, 2.0))), MeshMaterial3d(blue.clone()), Transform::from_xyz(0.0, 48.0, 0.0)));
            c.spawn((Mesh3d(meshes.add(Sphere::new(26.0).mesh().ico(3).unwrap())), MeshMaterial3d(dark.clone()), Transform::from_xyz(0.0, -80.0, 0.0)));
            for side in [-1.0f32, 1.0] {
                c.spawn((Mesh3d(meshes.add(Cuboid::new(140.0, 1.5, 34.0))), MeshMaterial3d(panel.clone()), Transform::from_xyz(side * 105.0, 95.0, 0.0)));
                c.spawn((Mesh3d(meshes.add(Cuboid::new(70.0, 3.0, 3.0))), MeshMaterial3d(dark.clone()), Transform::from_xyz(side * 35.0, 95.0, 0.0)));
            }
            // Docking bay glow.
            c.spawn((Mesh3d(meshes.add(Cuboid::new(30.0, 12.0, 4.0))), MeshMaterial3d(blue.clone()), Transform::from_xyz(0.0, -40.0, 25.0)));
            c.spawn((PointLight { color: Color::srgb(1.0, 0.85, 0.6), intensity: 1.5e8, range: 700.0, ..default() }, Transform::from_xyz(0.0, 0.0, 180.0)));
            c.spawn((NotShadowCaster, Mesh3d(meshes.add(Sphere::new(260.0).mesh().ico(3).unwrap())), MeshMaterial3d(glows.add(GlowMat { p: GlowParams { color: Vec4::new(0.5, 0.7, 1.0, 0.08), center: pos.extend(60.0), shape: Vec4::new(3.0, 0.0, 260.0, 0.0) } }))));
        });
}

fn spawn_jump_point(commands: &mut Commands, meshes: &mut Assets<Mesh>, mats: &mut Assets<StandardMaterial>, glows: &mut Assets<GlowMat>, pos: Vec3) {
    let ring = mats.add(StandardMaterial { base_color: Color::srgb(0.2, 0.22, 0.28), metallic: 0.9, perceptual_roughness: 0.3, ..default() });
    let glow = mats.add(StandardMaterial { base_color: Color::BLACK, emissive: LinearRgba::rgb(1.0, 3.0, 8.0), ..default() });
    commands.spawn((SystemScene, Transform::from_translation(pos), Visibility::default())).with_children(|c| {
        c.spawn((Mesh3d(meshes.add(Torus::new(105.0, 120.0))), MeshMaterial3d(ring), Transform::from_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2))));
        c.spawn((Spin(Vec3::Z * 0.6), Mesh3d(meshes.add(Torus::new(99.0, 101.0))), MeshMaterial3d(glow), Transform::from_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2))));
        c.spawn((NotShadowCaster, Mesh3d(meshes.add(Sphere::new(300.0).mesh().ico(3).unwrap())), MeshMaterial3d(glows.add(GlowMat { p: GlowParams { color: Vec4::new(0.3, 0.6, 1.0, 0.6), center: pos.extend(40.0), shape: Vec4::new(3.0, 1.0, 300.0, 0.0) } }))));
    });
}

fn follow_sky(cam: Query<&GlobalTransform, With<Camera3d>>, mut sky: Query<&mut Transform, With<Skybox>>) {
    let Ok(c) = cam.single() else { return };
    for mut t in &mut sky {
        t.translation = c.translation();
    }
}

fn spin_things(time: Res<Time>, mut q: Query<(&Spin, &mut Transform)>) {
    let dt = time.delta_secs();
    for (s, mut t) in &mut q {
        t.rotate_local(Quat::from_euler(EulerRot::XYZ, s.0.x * dt, s.0.y * dt, s.0.z * dt));
    }
}

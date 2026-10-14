//! Procedural meshes: asteroids, plume cones, beam cross-quads, warp tunnel, ring annulus,
//! plus a CPU-generated cubemap used as the image-based-lighting source for each system.

use bevy::{
    asset::RenderAssetUsages,
    image::{Image, ImageSampler},
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureViewDescriptor, TextureViewDimension},
};

fn hash3(x: i32, y: i32, z: i32, seed: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x8da6_b343) ^ (y as u32).wrapping_mul(0xd816_3841) ^ (z as u32).wrapping_mul(0xcb1a_b31f) ^ seed.wrapping_mul(0x9E37_79B9);
    h ^= h >> 13;
    h = h.wrapping_mul(0x5bd1_e995);
    h ^= h >> 15;
    (h & 0xffff) as f32 / 65535.0
}

pub fn vnoise(p: Vec3, seed: u32) -> f32 {
    let i = p.floor();
    let f = p - i;
    let u = f * f * (Vec3::splat(3.0) - 2.0 * f);
    let (x, y, z) = (i.x as i32, i.y as i32, i.z as i32);
    let c = |dx, dy, dz| hash3(x + dx, y + dy, z + dz, seed);
    let l = |a: f32, b: f32, t: f32| a + (b - a) * t;
    l(
        l(l(c(0, 0, 0), c(1, 0, 0), u.x), l(c(0, 1, 0), c(1, 1, 0), u.x), u.y),
        l(l(c(0, 0, 1), c(1, 0, 1), u.x), l(c(0, 1, 1), c(1, 1, 1), u.x), u.y),
        u.z,
    )
}

pub fn fbm(mut p: Vec3, seed: u32, oct: u32) -> f32 {
    let (mut a, mut s) = (0.5, 0.0);
    for _ in 0..oct {
        s += a * vnoise(p, seed);
        p = p * 2.03 + Vec3::new(1.7, 9.2, 3.1);
        a *= 0.5;
    }
    s
}

/// Lumpy, cratered rock. Unit-ish radius; scale with Transform.
pub fn asteroid(seed: u32) -> Mesh {
    let mut mesh = Sphere::new(1.0).mesh().ico(5).expect("ico");
    let stretch = Vec3::new(1.0 + (seed % 7) as f32 * 0.08, 0.75 + (seed % 5) as f32 * 0.06, 0.9);
    if let Some(bevy::mesh::VertexAttributeValues::Float32x3(pos)) = mesh.attribute_mut(Mesh::ATTRIBUTE_POSITION) {
        for p in pos.iter_mut() {
            let v = Vec3::from(*p);
            let n = fbm(v * 1.6, seed, 5);
            let ridges = 1.0 - (fbm(v * 3.5, seed + 9, 3) - 0.5).abs() * 2.0;
            // Craters: carve where a coarse cell noise peaks.
            let crater = (vnoise(v * 2.7, seed + 31) - 0.72).max(0.0) * 1.6;
            let r = 0.72 + n * 0.55 + ridges * 0.08 - crater;
            *p = (v * stretch * r).to_array();
        }
    }
    mesh.compute_smooth_normals();
    mesh
}

/// Open cone along +Z from the nozzle (z=0, radius 1) to the tail (z=1, radius `tail`).
pub fn plume(tail: f32) -> Mesh {
    let (segs, rings) = (24, 16);
    let mut pos = vec![];
    let mut nor = vec![];
    let mut uv = vec![];
    for j in 0..=rings {
        let v = j as f32 / rings as f32;
        let r = 1.0 + (tail - 1.0) * v.powf(0.7) - 0.15 * (v * 6.0).sin() * (1.0 - v);
        for i in 0..=segs {
            let a = i as f32 / segs as f32 * std::f32::consts::TAU;
            pos.push([a.cos() * r, a.sin() * r, v]);
            nor.push([a.cos(), a.sin(), 0.0]);
            uv.push([i as f32 / segs as f32, v]);
        }
    }
    let mut idx = vec![];
    for j in 0..rings {
        for i in 0..segs {
            let a = j * (segs + 1) + i;
            let b = a + segs + 1;
            idx.extend_from_slice(&[a as u32, b as u32, a as u32 + 1, a as u32 + 1, b as u32, b as u32 + 1]);
        }
    }
    build(pos, nor, uv, idx)
}

/// Two crossed quads along +Z (0..1), width 1. uv.x across, uv.y along.
pub fn beam() -> Mesh {
    let mut pos = vec![];
    let mut nor = vec![];
    let mut uv = vec![];
    let mut idx = vec![];
    for (k, axis) in [Vec3::X, Vec3::Y].iter().enumerate() {
        let base = (k * 4) as u32;
        let n = axis.cross(Vec3::Z);
        for (x, z) in [(-0.5, 0.0), (0.5, 0.0), (-0.5, 1.0), (0.5, 1.0)] {
            pos.push((*axis * x + Vec3::Z * z).to_array());
            nor.push(n.to_array());
            uv.push([x + 0.5, z]);
        }
        idx.extend_from_slice(&[base, base + 2, base + 1, base + 1, base + 2, base + 3]);
    }
    build(pos, nor, uv, idx)
}

/// Open cylinder around the Z axis, z from -0.5 to 0.5, normals inward.
pub fn tunnel() -> Mesh {
    let (segs, rings) = (64, 32);
    let mut pos = vec![];
    let mut nor = vec![];
    let mut uv = vec![];
    for j in 0..=rings {
        let v = j as f32 / rings as f32;
        for i in 0..=segs {
            let a = i as f32 / segs as f32 * std::f32::consts::TAU;
            pos.push([a.cos(), a.sin(), 0.5 - v]);
            nor.push([-a.cos(), -a.sin(), 0.0]);
            uv.push([i as f32 / segs as f32, v]);
        }
    }
    let mut idx = vec![];
    for j in 0..rings {
        for i in 0..segs {
            let a = (j * (segs + 1) + i) as u32;
            let b = a + segs as u32 + 1;
            idx.extend_from_slice(&[a, a + 1, b, a + 1, b + 1, b]);
        }
    }
    build(pos, nor, uv, idx)
}

/// Flat annulus in the XZ plane.
pub fn annulus(inner: f32, outer: f32) -> Mesh {
    let segs = 128;
    let mut pos = vec![];
    let mut nor = vec![];
    let mut uv = vec![];
    for i in 0..=segs {
        let a = i as f32 / segs as f32 * std::f32::consts::TAU;
        for (k, r) in [inner, outer].iter().enumerate() {
            pos.push([a.cos() * r, 0.0, a.sin() * r]);
            nor.push([0.0, 1.0, 0.0]);
            uv.push([k as f32, i as f32 / segs as f32]);
        }
    }
    let mut idx = vec![];
    for i in 0..segs {
        let a = (i * 2) as u32;
        idx.extend_from_slice(&[a, a + 2, a + 1, a + 1, a + 2, a + 3]);
    }
    build(pos, nor, uv, idx)
}

fn build(pos: Vec<[f32; 3]>, nor: Vec<[f32; 3]>, uv: Vec<[f32; 2]>, idx: Vec<u32>) -> Mesh {
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, nor)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uv)
        .with_inserted_indices(Indices::U32(idx))
}

/// Small cubemap approximating the system's sky (nebula colours + star) for image-based lighting,
/// so metallic hulls pick up the environment instead of reflecting black.
pub fn sky_cubemap(neb_a: [f32; 3], neb_b: [f32; 3], star_dir: Vec3, star_col: [f32; 3], seed: u32) -> Image {
    let size = 64u32;
    let mut data = Vec::with_capacity((size * size * 6 * 4) as usize);
    let faces: [(Vec3, Vec3, Vec3); 6] = [
        (Vec3::X, Vec3::NEG_Z, Vec3::NEG_Y),
        (Vec3::NEG_X, Vec3::Z, Vec3::NEG_Y),
        (Vec3::Y, Vec3::X, Vec3::Z),
        (Vec3::NEG_Y, Vec3::X, Vec3::NEG_Z),
        (Vec3::Z, Vec3::X, Vec3::NEG_Y),
        (Vec3::NEG_Z, Vec3::NEG_X, Vec3::NEG_Y),
    ];
    let sd = star_dir.normalize_or(Vec3::Z);
    for (fwd, right, down) in faces {
        for y in 0..size {
            for x in 0..size {
                let u = (x as f32 + 0.5) / size as f32 * 2.0 - 1.0;
                let v = (y as f32 + 0.5) / size as f32 * 2.0 - 1.0;
                let dir = (fwd + right * u + down * v).normalize();
                let n = fbm(dir * 2.5, seed, 4);
                let cloud = ((n - 0.4) / 0.45).clamp(0.0, 1.0);
                let mut c = Vec3::from(neb_a) * cloud * 0.35 + Vec3::from(neb_b) * (1.0 - cloud) * 0.06 + Vec3::splat(0.012);
                let s = dir.dot(sd).max(0.0);
                c += Vec3::from(star_col) * (s.powf(48.0) * 1.2 + s.powf(4.0) * 0.12);
                let c = c.min(Vec3::ONE);
                data.extend_from_slice(&[(c.x.powf(1.0 / 2.2) * 255.0) as u8, (c.y.powf(1.0 / 2.2) * 255.0) as u8, (c.z.powf(1.0 / 2.2) * 255.0) as u8, 255]);
            }
        }
    }
    let mut img = Image::new(Extent3d { width: size, height: size, depth_or_array_layers: 6 }, TextureDimension::D2, data, TextureFormat::Rgba8UnormSrgb, RenderAssetUsages::RENDER_WORLD);
    img.texture_view_descriptor = Some(TextureViewDescriptor { dimension: Some(TextureViewDimension::Cube), ..default() });
    img.sampler = ImageSampler::linear();
    img
}

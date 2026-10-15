// Procedural skybox: layered starfield, domain-warped nebula, galactic dust band.
// Rendered on a large sphere that follows the camera; colours come from the system's palette.
import bevy_pbr::render::forward_io::VertexOutput;
import bevy_pbr::render::mesh_view_bindings::{globals, view};

struct SpaceParams {
    neb_a: vec4<f32>,      // rgb, w = density
    neb_b: vec4<f32>,      // rgb, w = seed
    star_dir: vec4<f32>,   // direction to the system's star, w = star glow strength
    star_col: vec4<f32>,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var<uniform> p: SpaceParams;

fn hash3(q: vec3<f32>) -> f32 {
    var x = fract(q * vec3<f32>(0.1031, 0.1030, 0.0973));
    x += dot(x, x.yxz + 33.33);
    return fract((x.x + x.y) * x.z);
}

// Exact integer lattice hash (PCG3D). The float hash above loses precision at the coordinate
// magnitudes the upper octaves reach, so neighbouring cells could disagree about a shared corner
// and the nebula showed hard, torn cliffs.
fn hash_i(c: vec3<i32>) -> f32 {
    var v = vec3<u32>(c) * 1664525u + 1013904223u;
    v.x += v.y * v.z;
    v.y += v.z * v.x;
    v.z += v.x * v.y;
    v = v ^ (v >> vec3<u32>(16u));
    v.x += v.y * v.z;
    v.y += v.z * v.x;
    v.z += v.x * v.y;
    return f32(v.x >> 8u) / 16777215.0;
}

fn noise(x: vec3<f32>) -> f32 {
    let fi = floor(x);
    let i = vec3<i32>(fi);
    let f = x - fi;
    // Quintic fade: C2-continuous, so thresholded clouds show no creases along lattice cells.
    let u = f * f * f * (f * (f * 6.0 - 15.0) + 10.0);
    return mix(
        mix(mix(hash_i(i), hash_i(i + vec3<i32>(1, 0, 0)), u.x),
            mix(hash_i(i + vec3<i32>(0, 1, 0)), hash_i(i + vec3<i32>(1, 1, 0)), u.x), u.y),
        mix(mix(hash_i(i + vec3<i32>(0, 0, 1)), hash_i(i + vec3<i32>(1, 0, 1)), u.x),
            mix(hash_i(i + vec3<i32>(0, 1, 1)), hash_i(i + vec3<i32>(1, 1, 1)), u.x), u.y),
        u.z);
}

fn fbm(x0: vec3<f32>) -> f32 {
    var x = x0;
    var a = 0.5;
    var s = 0.0;
    // Each octave is rotated so lattice axes never line up into straight, torn-looking edges.
    let m = mat3x3<f32>(0.00, 0.80, 0.60, -0.80, 0.36, -0.48, -0.60, -0.48, 0.64);
    for (var i = 0; i < 6; i++) {
        s += a * noise(x);
        x = m * x * 2.03 + vec3<f32>(1.7, 9.2, 3.1);
        a *= 0.5;
    }
    return s;
}

fn star_layer(dir: vec3<f32>, scale: f32, threshold: f32, seed: f32) -> vec3<f32> {
    let q = dir * scale + seed;
    let cell = floor(q);
    let h = hash3(cell);
    if h < threshold {
        return vec3<f32>(0.0);
    }
    // Star position jittered inside its cell.
    let jitter = vec3<f32>(hash3(cell + 7.1), hash3(cell + 3.7), hash3(cell + 1.3)) * 0.6 + 0.2;
    let d = length(q - (cell + jitter));
    let mag = (h - threshold) / (1.0 - threshold);
    let core = exp(-d * d * 90.0) * (0.6 + 3.5 * mag * mag);
    // Temperature tint: red dwarfs to blue giants.
    let t = hash3(cell + 11.0);
    let tint = mix(vec3<f32>(1.0, 0.72, 0.55), vec3<f32>(0.65, 0.8, 1.0), t);
    let twinkle = 0.85 + 0.15 * sin(globals.time * (1.0 + 3.0 * t) + h * 40.0);
    return tint * core * twinkle;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    // Exact per-pixel view ray, rebuilt from the screen position. Taking it from the interpolated
    // sphere surface kinks the noise along every triangle edge, which the nebula's density
    // threshold turned into torn, polygon-shaped cloud edges.
    let uv = (in.position.xy - view.viewport.xy) / view.viewport.zw;
    let ndc = vec2<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0);
    let near = view.world_from_clip * vec4<f32>(ndc, 1.0, 1.0);
    let dir = normalize(near.xyz / near.w - view.world_position);
    let seed = p.neb_b.w;

    // Galactic plane: tilted band where dust and stars concentrate.
    let band_n = normalize(vec3<f32>(0.25, 1.0, 0.35));
    let band = exp(-abs(dot(dir, band_n)) * 5.0);

    // Domain-warped nebula.
    let w = vec3<f32>(fbm(dir * 2.0 + seed), fbm(dir * 2.0 + seed + 5.2), fbm(dir * 2.0 + seed + 9.7));
    let n1 = fbm(dir * 2.6 + w * 0.9 + seed);
    let n2 = fbm(dir * 5.5 - w * 0.7 + seed * 0.5);
    let dens = p.neb_a.w;
    let cloud = smoothstep(0.32, 0.9, n1) * dens;
    let wisps = smoothstep(0.45, 0.92, n2) * dens * 0.6;
    var col = p.neb_a.rgb * cloud * cloud * 0.32 + p.neb_b.rgb * wisps * wisps * 0.3;
    col += mix(p.neb_a.rgb, p.neb_b.rgb, n2) * band * 0.05;
    // Dark dust lanes cut through the band.
    // Soft-edged: a narrow threshold here drew crisp, geometric-looking lane borders.
    let dust = smoothstep(0.38, 0.82, fbm(dir * 7.0 + seed * 2.0)) * band;
    col *= 1.0 - dust * 0.55;

    // Stars: three layers, denser along the band.
    var stars = star_layer(dir, 180.0, 0.985 - band * 0.02, seed);
    stars += star_layer(dir, 420.0, 0.993 - band * 0.012, seed + 17.0) * 0.8;
    stars += star_layer(dir, 900.0, 0.996 - band * 0.006, seed + 41.0) * 0.6;
    col += stars * (1.0 - dust * 0.85);

    // Soft glow around the system's star direction (the star mesh itself is drawn separately).
    let sd = max(dot(dir, normalize(p.star_dir.xyz)), 0.0);
    col += p.star_col.rgb * (pow(sd, 64.0) * 0.6 + pow(sd, 8.0) * 0.08) * p.star_dir.w;

    return vec4<f32>(col, 1.0);
}

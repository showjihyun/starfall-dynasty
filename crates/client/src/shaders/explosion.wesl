// Explosion fireball: turbulent noise sphere whose temperature ramp cools from
// white-hot through orange to dark smoke as it ages; additive.
import bevy_pbr::render::forward_io::VertexOutput;
import bevy_pbr::render::mesh_view_bindings::{globals, view};

struct ExplosionParams {
    life: vec4<f32>,    // x = spawn time (globals.time), y = lifetime
    state: vec4<f32>,   // y = seed, z = brightness
}

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var<uniform> p: ExplosionParams;

fn hash3(q: vec3<f32>) -> f32 {
    var x = fract(q * vec3<f32>(0.1031, 0.1030, 0.0973));
    x += dot(x, x.yxz + 33.33);
    return fract((x.x + x.y) * x.z);
}

fn noise(x: vec3<f32>) -> f32 {
    let i = floor(x);
    let f = fract(x);
    let u = f * f * (3.0 - 2.0 * f);
    return mix(
        mix(mix(hash3(i), hash3(i + vec3<f32>(1.0, 0.0, 0.0)), u.x),
            mix(hash3(i + vec3<f32>(0.0, 1.0, 0.0)), hash3(i + vec3<f32>(1.0, 1.0, 0.0)), u.x), u.y),
        mix(mix(hash3(i + vec3<f32>(0.0, 0.0, 1.0)), hash3(i + vec3<f32>(1.0, 0.0, 1.0)), u.x),
            mix(hash3(i + vec3<f32>(0.0, 1.0, 1.0)), hash3(i + vec3<f32>(1.0, 1.0, 1.0)), u.x), u.y),
        u.z);
}

fn fbm(x0: vec3<f32>) -> f32 {
    var x = x0;
    var a = 0.5;
    var s = 0.0;
    for (var i = 0; i < 5; i++) {
        s += a * noise(x);
        x = x * 2.05 + vec3<f32>(2.1, 5.3, 1.7);
        a *= 0.5;
    }
    return s;
}

fn ramp(t: f32) -> vec3<f32> {
    // 1 = hottest.
    let smoke = vec3<f32>(0.05, 0.04, 0.04);
    let red = vec3<f32>(0.9, 0.15, 0.03);
    let orange = vec3<f32>(2.5, 0.9, 0.2);
    let white = vec3<f32>(6.0, 5.0, 3.5);
    if t < 0.33 {
        return mix(smoke, red, t / 0.33);
    }
    if t < 0.66 {
        return mix(red, orange, (t - 0.33) / 0.33);
    }
    return mix(orange, white, (t - 0.66) / 0.34);
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    // Unit sphere mesh: the normal is the radial direction, so no centre uniform is needed.
    let n = normalize(in.world_normal);
    let v = normalize(view.world_position - in.world_position.xyz);
    let age = clamp((globals.time - p.life.x) / max(p.life.y, 1e-3), 0.0, 1.0);
    let facing = abs(dot(n, v));
    let turb = fbm(n * 3.0 + p.state.y + vec3<f32>(age * 2.0));
    let heat = clamp(facing * 1.2 - age * 1.1 + turb * 0.6 - 0.2, 0.0, 1.0);
    let alpha = smoothstep(0.0, 0.35, facing) * (1.0 - smoothstep(0.6, 1.0, age));
    let c = ramp(heat) * alpha * p.state.z;
    return vec4<f32>(c, 0.0);
}

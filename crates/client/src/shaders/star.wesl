// Star surface: boiling granulation, sunspots, limb darkening. Emits HDR light for bloom.
import bevy_pbr::render::forward_io::VertexOutput;
import bevy_pbr::render::mesh_view_bindings::{globals, view};

struct StarParams {
    color: vec4<f32>,   // rgb, w = intensity
    center: vec4<f32>,  // xyz, w = radius
}

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var<uniform> p: StarParams;

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
        x = x * 2.1 + vec3<f32>(3.1, 1.7, 7.3);
        a *= 0.5;
    }
    return s;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let n = normalize(in.world_position.xyz - p.center.xyz);
    let v = normalize(view.world_position - in.world_position.xyz);
    let t = globals.time * 0.05;
    let warp = fbm(n * 3.0 + vec3<f32>(t, -t, t * 0.5));
    let gran = fbm(n * 14.0 + warp * 2.0 + vec3<f32>(0.0, t * 2.0, 0.0));
    let spots = smoothstep(0.62, 0.72, fbm(n * 2.2 + 11.0 + t * 0.3));
    let mu = max(dot(n, v), 0.0);
    let limb = 0.35 + 0.65 * pow(mu, 0.45);
    var c = p.color.rgb * (0.65 + 0.7 * gran) * limb;
    c = mix(c, c * 0.25, spots);
    // Hot, whiter core facing the viewer.
    c += vec3<f32>(1.0, 0.95, 0.85) * pow(mu, 3.0) * 0.4;
    return vec4<f32>(c * p.color.w, 1.0);
}

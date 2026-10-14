// Planetary ring: banded, gapped, translucent, shadowed by the planet.
import bevy_pbr::render::forward_io::VertexOutput;
import bevy_pbr::render::mesh_view_bindings::view;

struct RingParams {
    center: vec4<f32>,  // xyz planet centre, w = planet radius
    color: vec4<f32>,   // rgb, w = seed
    radii: vec4<f32>,   // x inner, y outer
    star: vec4<f32>,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var<uniform> p: RingParams;

fn hash1(x: f32) -> f32 {
    return fract(sin(x * 127.1) * 43758.5453);
}

fn bands(r: f32, seed: f32) -> f32 {
    var s = 0.0;
    var f = 1.0;
    var a = 0.5;
    for (var i = 0; i < 5; i++) {
        let x = r * f * 40.0 + seed;
        let i0 = floor(x);
        s += a * mix(hash1(i0), hash1(i0 + 1.0), smoothstep(0.0, 1.0, fract(x)));
        f *= 2.3;
        a *= 0.55;
    }
    return s;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let rel = in.world_position.xyz - p.center.xyz;
    let r = (length(rel) - p.radii.x) / (p.radii.y - p.radii.x);
    if r < 0.0 || r > 1.0 {
        discard;
    }
    let b = bands(r, p.color.w);
    let gap = smoothstep(0.02, 0.0, abs(r - 0.62)) + smoothstep(0.015, 0.0, abs(r - 0.3));
    var alpha = clamp(b * 1.3 - 0.25, 0.0, 1.0) * (1.0 - gap) * smoothstep(0.0, 0.05, r) * smoothstep(1.0, 0.9, r);
    // Planet shadow: is the segment toward the star blocked by the planet sphere?
    let l = normalize(-p.center.xyz);
    let along = dot(rel, l);
    let perp = length(rel - l * along);
    let shadow = select(1.0, 0.08, along < 0.0 && perp < p.center.w);
    let c = p.color.rgb * (0.6 + 0.6 * b) * shadow * p.star.rgb * p.star.w;
    return vec4<f32>(c * alpha, alpha * 0.9);
}

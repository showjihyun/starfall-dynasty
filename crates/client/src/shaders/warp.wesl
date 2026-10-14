// Warp tunnel: a long open cylinder around the camera. Streaking starlines rush past,
// a swirling blue-violet field compresses toward the vanishing point.
import bevy_pbr::render::forward_io::VertexOutput;
import bevy_pbr::render::mesh_view_bindings::globals;

struct WarpParams {
    color: vec4<f32>,  // rgb, w = intensity 0..1
    accent: vec4<f32>,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var<uniform> p: WarpParams;

fn hash2(q: vec2<f32>) -> f32 {
    return fract(sin(dot(q, vec2<f32>(127.1, 311.7))) * 43758.5453);
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    var uv = vec2<f32>(0.5, 0.5);
@if(VERTEX_UVS_A) {
    uv = in.uv;
}
    let t = globals.time;
    let k = p.color.w;
    // uv.x around the tube, uv.y along it (0 near the camera's back → 1 ahead).
    let a = uv.x * 96.0;
    let lane = floor(a);
    let speed = 2.0 + hash2(vec2<f32>(lane, 3.0)) * 4.0;
    let s = fract(uv.y * 3.0 - t * speed * (0.3 + k) + hash2(vec2<f32>(lane, 1.0)));
    let width = smoothstep(0.5, 0.0, abs(fract(a) - 0.5) * 2.0);
    let streak = smoothstep(0.0, 0.05, s) * smoothstep(0.35 * k + 0.05, 0.0, s) * width;
    let lit = step(0.55, hash2(vec2<f32>(lane, floor(uv.y * 3.0 - t * speed * (0.3 + k)))));
    let swirl = 0.5 + 0.5 * sin(uv.x * 25.0 + uv.y * 12.0 - t * 6.0);
    let fade = smoothstep(0.0, 0.25, uv.y) * smoothstep(1.0, 0.6, uv.y);
    let c = p.color.rgb * streak * lit * 3.0 + p.accent.rgb * swirl * 0.25;
    return vec4<f32>(c * fade * k, 0.0);
}

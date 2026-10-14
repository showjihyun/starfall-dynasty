// Engine plume: additive cone with a white-hot core, coloured sheath, shock diamonds
// and turbulent flicker. uv.y runs 0 at the nozzle → 1 at the tail.
import bevy_pbr::render::forward_io::VertexOutput;
import bevy_pbr::render::mesh_view_bindings::{globals, view};

struct EngineParams {
    color: vec4<f32>,  // rgb sheath, w = throttle 0..1
    core: vec4<f32>,   // rgb core, w = boost 0..1
    seed: vec4<f32>,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var<uniform> p: EngineParams;

fn hash1(x: f32) -> f32 {
    return fract(sin(x * 91.17) * 43758.5453);
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    var uv = vec2<f32>(0.5, 0.5);
@if(VERTEX_UVS_A) {
    uv = in.uv;
}
    let along = uv.y;
    let v = normalize(view.world_position - in.world_position.xyz);
    let n = normalize(in.world_normal);
    // Facing ratio approximates "distance from the plume axis" on screen.
    let facing = abs(dot(n, v));
    let t = globals.time;
    let throttle = clamp(p.color.w, 0.0, 1.0);
    let boost = p.core.w;
    let reach = 0.25 + 0.75 * throttle + 0.5 * boost;
    let fade = 1.0 - smoothstep(0.0, reach, along);
    let flicker = 0.85 + 0.15 * sin(t * 40.0 + p.seed.x * 10.0) * hash1(floor(t * 30.0) + p.seed.x);
    let diamonds = 0.75 + 0.25 * cos(along * 48.0 - t * 30.0) * (1.0 - along);
    // View-independent body so the plume reads from straight behind (chase camera) as well as from the side.
    let body = pow(1.0 - along, 2.2) * (0.35 + 0.65 * facing);
    let core = (pow(facing, 6.0) + 0.25) * pow(1.0 - along, 5.0);
    let sheath = body * fade;
    let c = p.core.rgb * core * 4.0 + p.color.rgb * sheath * diamonds * 1.6;
    return vec4<f32>(c * flicker * (0.45 + throttle * 0.9 + boost * 1.2), 0.0);
}

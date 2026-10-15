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
    // Soft body: from the side the silhouette fades to a dim coloured edge (no hard white cone);
    // looking down the axis (chase camera) every normal is grazing, so a coloured floor keeps it visible.
    let edge = 0.25 + 0.75 * smoothstep(0.05, 0.75, facing);
    let body = pow(1.0 - along, 2.4) * edge;
    // White-hot core only toward the plume axis, near the nozzle.
    let core = pow(facing, 18.0) * pow(1.0 - along, 6.0);
    let sheath = body * fade;
    // Temperature gradient: core colour at the nozzle cooling to the sheath colour, then to a dimmer, redder tail.
    let tail = mix(p.color.rgb, p.color.rgb * vec3<f32>(1.1, 0.75, 0.9), smoothstep(0.2, 0.9, along));
    // Exit-plane glow, independent of view, so a nozzle seen from straight behind is lit too.
    let exit_glow = pow(1.0 - along, 14.0);
    let c = p.core.rgb * (core * 2.2 + exit_glow * 1.2) + tail * sheath * diamonds * 1.7;
    return vec4<f32>(c * flicker * (0.4 + throttle * 0.8 + boost * 1.1) * p.seed.y, 0.0);
}

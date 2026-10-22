// Engine nozzle glow on a camera-facing quad at every nozzle exit: white-hot centre fading to the
// exhaust colour, gently flickering, brighter with throttle and boost. Additive. Shared by every
// nozzle of one size class; colour, throttle and boost come from the ship's slot.
import bevy_pbr::render::forward_io::VertexOutput;
import bevy_pbr::render::mesh_view_bindings::globals;
import bevy_pbr::render::mesh_functions::get_tag;

struct NozzleParams {
    shape: vec4<f32>,  // x = gain, y > 0: tight core (big-ship ion ports)
}

// Per ship (ships::ShipStates; layout as in hull.wesl), indexed by the MeshTag's low 24 bits.
struct Ship {
    state: vec4<f32>,
    glow: vec4<f32>,
    drive: vec4<f32>,   // x throttle 0..1, y boost 0..1
    engine: vec4<f32>,  // rgb plume sheath
    core: vec4<f32>,    // rgb plume core
    shield: vec4<f32>,  // rgb, w = shield fraction
    hit: vec4<f32>,     // xyz local hit direction, w = seconds since hit
}

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var<uniform> p: NozzleParams;
@group(constants::MATERIAL_BIND_GROUP) @binding(1) var<storage, read> ships: array<Ship>;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    var uv = vec2<f32>(0.5, 0.5);
@if(VERTEX_UVS_A) {
    uv = in.uv;
}
    let ship = ships[get_tag(in.instance_index) & 0xffffffu];
    let d = (uv - 0.5) * 2.0;
    let r = length(d);
    let intensity = p.shape.x * (0.8 + ship.drive.x * 1.2 + ship.drive.y * 1.5);
    let flick = 0.9 + 0.1 * sin(globals.time * 37.0 + ship.state.w);
    // A hot centre in a dark bore, not a lit ball.
    let tight = p.shape.y;
    let g = mix(exp(-r * 3.2) * 1.3 + exp(-r * 1.5) * 0.3, exp(-r * 7.0) * 1.6 + exp(-r * 2.6) * 0.18, tight);
    let col = mix(ship.engine.rgb, vec3<f32>(1.0, 1.0, 1.0), exp(-r * mix(5.0, 9.0, tight)));
    return vec4<f32>(col * g * intensity * flick * smoothstep(1.0, 0.7, r), 0.0);
}

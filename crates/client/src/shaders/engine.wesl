// Engine plume, uv.y runs 0 at the nozzle → 1 at the tail. Two styles:
// - torch (fighters): additive cone with a white-hot core, coloured sheath, shock diamonds, flicker;
// - ion drive (cruisers, capitals; seed.z = 1): long, slender, translucent blue exhaust that fades out
//   smoothly with a faint turbulent shimmer and no hard silhouette. The hot exit disc seen from
//   behind is drawn by the nozzle glow sprite.
import bevy_pbr::render::forward_io::VertexOutput;
import bevy_pbr::render::mesh_view_bindings::{globals, view};
import bevy_pbr::render::mesh_functions::get_tag;

// Shared by every plume of one style; throttle, boost and colours come from the ship's slot.
struct EngineParams {
    color: vec4<f32>,  // rgb sheath when seed.w = 1 (missiles, fallback), else the ship's
    core: vec4<f32>,   // rgb core, likewise
    seed: vec4<f32>,   // y = gain, z = style (0 torch, 1 ion drive), w = 1: own colours
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

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var<uniform> p: EngineParams;
@group(constants::MATERIAL_BIND_GROUP) @binding(1) var<storage, read> ships: array<Ship>;

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
    // MeshTag: slot in the low 24 bits, the plume's index on its ship above (flicker phase).
    let tag = get_tag(in.instance_index);
    let ship = ships[tag & 0xffffffu];
    let throttle = clamp(ship.drive.x, 0.0, 1.0);
    let boost = ship.drive.y;
    let seed = ship.state.w + f32(tag >> 24u);
    let own = p.seed.w > 0.5;
    let sheath_c = select(ship.engine.rgb, p.color.rgb, own);
    let core_c = select(ship.core.rgb, p.core.rgb, own);
    if p.seed.z > 0.5 {
        // Soft both across (no outline) and along (exponential fade, nothing at the tail).
        let across = smoothstep(0.1, 0.95, facing);
        let reach_i = 0.55 + 0.35 * throttle + 0.3 * boost;
        let along_fade = exp(-along * 3.2 / reach_i) * (1.0 - smoothstep(0.75 * reach_i, reach_i, along));
        let shimmer = 0.82 + 0.18 * sin(along * 26.0 - t * 18.0 + uv.x * 25.13 + seed * 3.0);
        // Faint brighter filament near the axis, close to the nozzle.
        let filament = pow(facing, 10.0) * exp(-along * 7.0);
        let deep = sheath_c * vec3<f32>(0.75, 0.9, 1.25);
        let c = deep * across * across * along_fade * shimmer * 1.1 + core_c * filament * 0.9;
        return vec4<f32>(c * (0.45 + throttle * 0.75 + boost * 0.9) * p.seed.y, 0.0);
    }
    let reach = 0.25 + 0.75 * throttle + 0.5 * boost;
    let fade = 1.0 - smoothstep(0.0, reach, along);
    let flicker = 0.85 + 0.15 * sin(t * 40.0 + seed * 10.0) * hash1(floor(t * 30.0) + seed);
    let diamonds = 0.75 + 0.25 * cos(along * 48.0 - t * 30.0) * (1.0 - along);
    // Soft body: from the side the silhouette fades to a dim coloured edge (no hard white cone);
    // looking down the axis (chase camera) every normal is grazing, so a coloured floor keeps it visible.
    let edge = 0.25 + 0.75 * smoothstep(0.05, 0.75, facing);
    let body = pow(1.0 - along, 2.4) * edge;
    // White-hot core only toward the plume axis, near the nozzle.
    let core = pow(facing, 18.0) * pow(1.0 - along, 6.0);
    let sheath = body * fade;
    // Temperature gradient: core colour at the nozzle cooling to the sheath colour, then to a dimmer, redder tail.
    let tail = mix(sheath_c, sheath_c * vec3<f32>(1.1, 0.75, 0.9), smoothstep(0.2, 0.9, along));
    // Exit-plane glow, independent of view, so a nozzle seen from straight behind is lit too.
    let exit_glow = pow(1.0 - along, 14.0);
    let c = core_c * (core * 2.2 + exit_glow * 1.2) + tail * sheath * diamonds * 1.7;
    return vec4<f32>(c * flicker * (0.4 + throttle * 0.8 + boost * 1.1) * p.seed.y, 0.0);
}

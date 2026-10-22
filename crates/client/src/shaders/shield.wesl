// Energy shield bubble: faint hex lattice and fresnel shell, a ripple expanding
// from the latest impact point, flickering when the shield is nearly depleted.
import bevy_pbr::render::forward_io::VertexOutput;
import bevy_pbr::render::mesh_view_bindings::{globals, view};
import bevy_pbr::render::mesh_functions::get_tag;

// One material for every shield; colour, strength and the latest hit come from the ship's slot.
struct ShieldParams {
    look: vec4<f32>,  // x = brightness
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

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var<uniform> p: ShieldParams;
@group(constants::MATERIAL_BIND_GROUP) @binding(1) var<storage, read> ships: array<Ship>;

fn hex(uv: vec2<f32>) -> f32 {
    let r = vec2<f32>(1.0, 1.7320508);
    let h = r * 0.5;
    let a = (uv - r * floor(uv / r)) - h;
    let b = ((uv - h) - r * floor((uv - h) / r)) - h;
    let g = select(b, a, dot(a, a) < dot(b, b));
    let e = max(abs(g.x) * 0.866 + abs(g.y) * 0.5, abs(g.y));
    return smoothstep(0.44, 0.5, e);
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    // The shield mesh is a sphere: its normal is the radial direction, no world centre needed.
    let n = normalize(in.world_normal);
    let v = normalize(view.world_position - in.world_position.xyz);
    let fres = pow(1.0 - abs(dot(n, v)), 3.0);
    let t = globals.time;
    let ship = ships[get_tag(in.instance_index) & 0xffffffu];
    let lat = vec2<f32>(atan2(n.z, n.x) * 3.0, n.y * 6.0);
    let lattice = hex(lat * 2.0);
    let frac = ship.shield.w;
    // Impact ripple.
    let age = ship.hit.w;
    let d = acos(clamp(dot(n, normalize(ship.hit.xyz)), -1.0, 1.0));
    let ring = smoothstep(0.25, 0.0, abs(d - age * 4.0)) * exp(-age * 3.0);
    let impact = exp(-d * 5.0) * exp(-age * 6.0);
    var base = 0.0;
    if frac < 0.25 && frac > 0.0 {
        base = fres * 0.05;
        base *= 0.5 + 0.5 * step(0.5, fract(t * 9.0));
    }
    let intensity = base * frac + (ring * 0.9 + impact * 1.6) * (0.4 + lattice * 0.6) * max(frac, 0.15);
    return vec4<f32>(ship.shield.rgb * intensity * p.look.x, 0.0);
}

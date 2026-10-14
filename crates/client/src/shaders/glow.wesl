// Volumetric-looking glow halo (star corona, planet atmosphere scatter, beacons).
// Drawn on a larger sphere with additive blending: brightness depends on how close
// the view ray passes to the centre, so it looks the same from every angle.
import bevy_pbr::render::forward_io::VertexOutput;
import bevy_pbr::render::mesh_view_bindings::{globals, view};

struct GlowParams {
    color: vec4<f32>,   // rgb, w = intensity
    center: vec4<f32>,  // xyz, w = inner radius (the body)
    shape: vec4<f32>,   // x = falloff, y = pulse speed, z = outer radius
}

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var<uniform> p: GlowParams;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let cam = view.world_position;
    let dir = normalize(in.world_position.xyz - cam);
    let to_c = p.center.xyz - cam;
    let along = dot(to_c, dir);
    let closest = length(to_c - dir * along);
    let r = p.center.w;
    let outer = p.shape.z;
    let x = clamp((closest - r) / max(outer - r, 1e-3), 0.0, 1.0);
    var g = exp(-x * p.shape.x) * (1.0 - x);
    // Hide the part of the halo that is in front of the body (the body covers it anyway)
    // and behind the camera.
    if along < 0.0 {
        g = 0.0;
    }
    let pulse = 1.0 + 0.15 * sin(globals.time * p.shape.y);
    return vec4<f32>(p.color.rgb * g * p.color.w * pulse, 0.0);
}

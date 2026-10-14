// Laser / mining beam on a cross-quad: white core, coloured glow, energy pulses.
// uv.x across the beam (0..1), uv.y along it.
import bevy_pbr::render::forward_io::VertexOutput;
import bevy_pbr::render::mesh_view_bindings::globals;

struct BeamParams {
    color: vec4<f32>,  // rgb, w = intensity (fades to 0)
    flow: vec4<f32>,   // x = pulse frequency, y = scroll speed, z = length
}

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var<uniform> p: BeamParams;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    var uv = vec2<f32>(0.5, 0.5);
@if(VERTEX_UVS_A) {
    uv = in.uv;
}
    let x = abs(uv.x - 0.5) * 2.0;
    let core = exp(-x * x * 60.0);
    let glow = exp(-x * x * 6.0);
    let pulse = 0.7 + 0.3 * sin(uv.y * p.flow.x * p.flow.z - globals.time * p.flow.y);
    let ends = smoothstep(0.0, 0.02, uv.y) * smoothstep(1.0, 0.97, uv.y);
    let c = vec3<f32>(1.0) * core * 4.0 + p.color.rgb * glow * pulse * 2.5;
    return vec4<f32>(c * p.color.w * ends, 0.0);
}

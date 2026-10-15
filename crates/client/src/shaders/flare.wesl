// Star glare on a camera-facing quad just in front of the star: hot core, anamorphic horizontal
// streak, six diffraction spikes and a faint chromatic halo ring. Additive; ships passing in front
// occlude it through the depth test.
import bevy_pbr::render::forward_io::VertexOutput;
import bevy_pbr::render::mesh_view_bindings::globals;

struct FlareParams {
    color: vec4<f32>,  // rgb star colour, w = intensity
    shape: vec4<f32>,  // x = spike rotation, y = streak strength, z = 1: plain soft disc (engine nozzle glow)
}

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var<uniform> p: FlareParams;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    var uv = vec2<f32>(0.5, 0.5);
@if(VERTEX_UVS_A) {
    uv = in.uv;
}
    let d = (uv - 0.5) * 2.0;
    let r = length(d);
    if p.shape.z > 0.5 {
        // Nozzle glow: white-hot centre fading to the exhaust colour, gently flickering.
        let flick = 0.9 + 0.1 * sin(globals.time * 37.0 + p.shape.w);
        let g = exp(-r * 3.2) * 1.3 + exp(-r * 1.5) * 0.3;
        let col = mix(p.color.rgb, vec3<f32>(1.0, 1.0, 1.0), exp(-r * 5.0));
        return vec4<f32>(col * g * p.color.w * flick * smoothstep(1.0, 0.7, r), 0.0);
    }
    let tint = mix(p.color.rgb, vec3<f32>(1.0, 1.0, 1.0), 0.45);
    let core = exp(-r * 14.0) * 1.2 + exp(-r * 5.0) * 0.1;
    let streak = exp(-abs(d.y) * 110.0) * exp(-abs(d.x) * 2.4) * smoothstep(1.0, 0.55, abs(d.x)) * p.shape.y;
    let ang = atan2(d.y, d.x) + p.shape.x;
    let spikes = pow(abs(cos(ang * 3.0)), 200.0) * exp(-r * 6.0) * 0.6;
    let shimmer = 0.9 + 0.1 * sin(globals.time * 1.7 + r * 30.0);
    let ring = exp(-pow((r - 0.45) * 22.0, 2.0)) * 0.012;
    let ring_c = vec3<f32>(0.55 + 0.45 * cos(r * 40.0), 0.75, 0.55 + 0.45 * sin(r * 40.0)) * ring;
    let edge = smoothstep(1.0, 0.85, max(abs(d.x), abs(d.y)));
    let c = tint * (core + spikes * shimmer) + vec3<f32>(0.65, 0.8, 1.0) * streak + ring_c;
    return vec4<f32>(c * p.color.w * edge, 0.0);
}

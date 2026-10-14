import bevy_pbr::render::forward_io::VertexOutput;
import bevy_pbr::render::mesh_view_bindings::{globals, view};

struct SpikeParams {
    tint: vec4<f32>,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var<uniform> params: SpikeParams;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let dir = normalize(in.world_position.xyz - view.world_position);
    let t = globals.time;
    let c = 0.5 + 0.5 * sin(dir * 6.0 + vec3<f32>(t, t * 1.3, t * 0.7));
    return vec4<f32>(c * params.tint.rgb, 1.0);
}

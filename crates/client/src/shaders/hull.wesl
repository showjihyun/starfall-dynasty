// Ship hull: StandardMaterial PBR extension (Blender-baked textures as the base) plus
//  - per-ship livery tint on "Livery" parts,
//  - animated emissive on "Glow" parts (running lights pulse, brighter when boosting),
//  - battle damage: procedural scorch + glowing embers that spread as hull integrity drops,
//  - hit flash and a cool fresnel rim so silhouettes read against dark space.
import bevy_pbr::render::{
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{alpha_discard, apply_pbr_lighting, main_pass_post_lighting_processing},
    forward_io::{VertexOutput, FragmentOutput},
    pbr_types::STANDARD_MATERIAL_FLAGS_UNLIT_BIT,
};
import bevy_pbr::render::mesh_view_bindings::{globals, view};

struct HullExt {
    livery: vec4<f32>,  // rgb livery, w = part kind (0 hull, 1 livery, 2 glow, 3 engine core)
    glow: vec4<f32>,    // rgb glow tint, w = glow strength
    state: vec4<f32>,   // x damage 0..1, y hit flash 0..1, z boost 0..1, w seed
}

@group(constants::MATERIAL_BIND_GROUP) @binding(100) var<uniform> ext: HullExt;

fn hash3(q: vec3<f32>) -> f32 {
    var x = fract(q * vec3<f32>(0.1031, 0.1030, 0.0973));
    x += dot(x, x.yxz + 33.33);
    return fract((x.x + x.y) * x.z);
}

fn noise(x: vec3<f32>) -> f32 {
    let i = floor(x);
    let f = fract(x);
    let u = f * f * (3.0 - 2.0 * f);
    return mix(
        mix(mix(hash3(i), hash3(i + vec3<f32>(1.0, 0.0, 0.0)), u.x),
            mix(hash3(i + vec3<f32>(0.0, 1.0, 0.0)), hash3(i + vec3<f32>(1.0, 1.0, 0.0)), u.x), u.y),
        mix(mix(hash3(i + vec3<f32>(0.0, 0.0, 1.0)), hash3(i + vec3<f32>(1.0, 0.0, 1.0)), u.x),
            mix(hash3(i + vec3<f32>(0.0, 1.0, 1.0)), hash3(i + vec3<f32>(1.0, 1.0, 1.0)), u.x), u.y),
        u.z);
}

fn fbm(x0: vec3<f32>) -> f32 {
    var x = x0;
    var a = 0.5;
    var s = 0.0;
    for (var i = 0; i < 4; i++) {
        s += a * noise(x);
        x = x * 2.17 + vec3<f32>(1.3, 7.1, 4.2);
        a *= 0.5;
    }
    return s;
}

@fragment
fn fragment(vertex_output: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var in = vertex_output;
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    pbr_input.material.base_color = alpha_discard(pbr_input.material.flags, pbr_input.material.alpha_cutoff, pbr_input.material.base_color);

    let kind = i32(ext.livery.w + 0.5);
    let t = globals.time;

    // Damage pattern lives in UV space so it sticks to the hull as the ship moves.
    var uvp = vec3<f32>(in.world_normal * 0.5);
@if(VERTEX_UVS_A) {
    uvp = vec3<f32>(in.uv * 9.0, ext.state.w);
}
    let dmg = ext.state.x;
    let scorch_n = fbm(uvp * 1.3 + ext.state.w);
    let scorch = smoothstep(1.0 - dmg * 0.9, 1.05 - dmg * 0.9, scorch_n + 0.25) * step(0.02, dmg);

    if kind == 1 {
        pbr_input.material.base_color = vec4<f32>(pbr_input.material.base_color.rgb * ext.livery.rgb, pbr_input.material.base_color.a);
    }
    if kind == 2 {
        let blink = 0.75 + 0.25 * sin(t * 2.0 + ext.state.w * 6.0);
        pbr_input.material.emissive = vec4<f32>(ext.glow.rgb * ext.glow.w * blink * (1.0 + ext.state.z), 1.0);
    }
    if kind == 3 {
        pbr_input.material.emissive = vec4<f32>(ext.glow.rgb * ext.glow.w * (1.5 + 2.5 * ext.state.z), 1.0);
    }
    if kind <= 1 {
        pbr_input.material.base_color = vec4<f32>(mix(pbr_input.material.base_color.rgb, vec3<f32>(0.02, 0.018, 0.016), scorch * 0.85), pbr_input.material.base_color.a);
        pbr_input.material.perceptual_roughness = mix(pbr_input.material.perceptual_roughness, 0.95, scorch);
        pbr_input.material.metallic = mix(pbr_input.material.metallic, 0.1, scorch);
    }

    var out: FragmentOutput;
    if (pbr_input.material.flags & STANDARD_MATERIAL_FLAGS_UNLIT_BIT) == 0u {
        out.color = apply_pbr_lighting(pbr_input);
    } else {
        out.color = pbr_input.material.base_color;
    }

    // Embers at the edges of heavy scorching.
    if kind <= 1 && dmg > 0.45 {
        let edge = smoothstep(0.08, 0.0, abs(scorch_n + 0.25 - (1.0 - dmg * 0.9))) * (dmg - 0.45) * 2.0;
        let flicker = 0.6 + 0.4 * noise(uvp * 4.0 + vec3<f32>(0.0, 0.0, t * 3.0));
        out.color = vec4<f32>(out.color.rgb + vec3<f32>(3.0, 0.9, 0.2) * edge * flicker, out.color.a);
    }
    // Cool rim light so ships read against black space, tinted by livery.
    let v = normalize(view.world_position - in.world_position.xyz);
    let rim = pow(1.0 - max(dot(normalize(in.world_normal), v), 0.0), 4.0);
    out.color = vec4<f32>(out.color.rgb + mix(vec3<f32>(0.35, 0.5, 0.8), ext.livery.rgb, 0.4) * rim * 0.35, out.color.a);
    // Hit flash.
    out.color = vec4<f32>(out.color.rgb + vec3<f32>(1.5, 1.7, 2.2) * ext.state.y * (0.3 + rim), out.color.a);

    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}

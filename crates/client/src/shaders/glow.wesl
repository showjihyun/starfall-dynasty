// Volumetric-looking glow halo (star corona, planet atmosphere scatter, beacons).
// Drawn on a larger sphere with additive blending: brightness depends on how close
// the view ray passes to the centre, so it looks the same from every angle.
import bevy_pbr::render::forward_io::VertexOutput;
import bevy_pbr::render::mesh_view_bindings::{globals, view};

struct GlowParams {
    color: vec4<f32>,   // rgb, w = intensity
    center: vec4<f32>,  // xyz, w = inner radius (the body)
    shape: vec4<f32>,   // x = falloff, y = pulse speed, z = outer radius,
                        // w > 0: corona streamers (0..1); w != 0: fade out while the camera is inside the halo
}

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
    // Corona streamers: brightness varies with the angle around the centre as seen on screen,
    // stretching outward. Uses the camera basis so the rays stay radial from any view.
    if p.shape.w > 0.0 {
        let off = (to_c - dir * along) * -1.0;
        let right = view.world_from_view[0].xyz;
        let up = view.world_from_view[1].xyz;
        let ang = atan2(dot(off, up), dot(off, right));
        let t = globals.time * 0.02;
        let k = vec3<f32>(cos(ang) * 3.0, sin(ang) * 3.0, t);
        let rays = noise(k) * 0.65 + noise(k * 2.7 + 5.0) * 0.35;
        g *= mix(1.0, 0.35 + 1.6 * rays * rays, p.shape.w * smoothstep(0.0, 0.5, x));
    }
    if p.shape.w != 0.0 {
        // Don't fog the whole view when the camera flies inside the halo.
        g *= clamp((length(to_c) - r) / max(outer - r, 1e-3), 0.0, 1.0);
    }
    let pulse = 1.0 + 0.15 * sin(globals.time * p.shape.y);
    return vec4<f32>(p.color.rgb * g * p.color.w * pulse, 0.0);
}

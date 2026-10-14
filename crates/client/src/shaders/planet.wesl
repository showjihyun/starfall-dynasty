// Procedural planets: rocky, ocean (with clouds), gas giant bands, ice, lava.
// Lit by the system's star at the origin with a soft terminator and atmospheric rim.
import bevy_pbr::render::forward_io::VertexOutput;
import bevy_pbr::render::mesh_view_bindings::{globals, view};

struct PlanetParams {
    center: vec4<f32>,   // xyz, w = radius
    kind: vec4<f32>,     // x = kind (0 rocky,1 ocean,2 gas,3 ice,4 lava), y = seed, z = spin speed
    col_a: vec4<f32>,
    col_b: vec4<f32>,
    atmo: vec4<f32>,     // rgb, w = strength
    star: vec4<f32>,     // star colour rgb, w = intensity
}

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var<uniform> p: PlanetParams;

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
    for (var i = 0; i < 6; i++) {
        s += a * noise(x);
        x = x * 2.02 + vec3<f32>(5.1, 1.3, 2.7);
        a *= 0.5;
    }
    return s;
}

fn rot_y(v: vec3<f32>, a: f32) -> vec3<f32> {
    let c = cos(a);
    let s = sin(a);
    return vec3<f32>(c * v.x + s * v.z, v.y, -s * v.x + c * v.z);
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let wn = normalize(in.world_position.xyz - p.center.xyz);
    let seed = p.kind.y;
    let t = globals.time * p.kind.z;
    let n = rot_y(wn, t);
    let kind = i32(p.kind.x + 0.5);
    let q = n * 2.2 + seed;

    var albedo = vec3<f32>(0.5);
    var emissive = vec3<f32>(0.0);
    var spec = 0.0;
    if kind == 2 {
        // Gas giant: latitude bands with turbulent edges and a storm.
        let turb = fbm(q * 3.0) * 0.35;
        let lat = n.y + turb * 0.4;
        let bands = sin(lat * 18.0 + fbm(vec3<f32>(lat * 4.0, seed, 0.0)) * 4.0) * 0.5 + 0.5;
        albedo = mix(p.col_a.rgb, p.col_b.rgb, bands);
        let storm_c = normalize(vec3<f32>(0.6, -0.25, 0.75));
        let sd = distance(n, storm_c);
        albedo = mix(albedo, p.col_b.rgb * 1.3 + vec3<f32>(0.15, 0.05, 0.0), smoothstep(0.18, 0.05, sd) * 0.8);
    } else {
        let h = fbm(q);
        let detail = fbm(q * 6.0);
        if kind == 1 {
            let land = smoothstep(0.5, 0.53, h);
            let sea = mix(p.col_a.rgb * 0.6, p.col_a.rgb, smoothstep(0.35, 0.5, h));
            let ground = mix(p.col_b.rgb, p.col_b.rgb * 0.6 + vec3<f32>(0.25, 0.2, 0.1), detail);
            albedo = mix(sea, ground, land);
            // Polar caps.
            albedo = mix(albedo, vec3<f32>(0.92), smoothstep(0.72, 0.8, abs(n.y) + detail * 0.1));
            spec = (1.0 - land) * 0.6;
        } else if kind == 3 {
            albedo = mix(p.col_a.rgb, p.col_b.rgb, detail);
            let cracks = smoothstep(0.02, 0.0, abs(fbm(q * 3.0) - 0.5));
            albedo = mix(albedo, p.col_b.rgb * 0.5, cracks);
            spec = 0.3;
        } else if kind == 4 {
            let crust = smoothstep(0.42, 0.55, h + detail * 0.2);
            albedo = mix(vec3<f32>(0.05, 0.03, 0.02), p.col_a.rgb * 0.3, crust);
            let lava = 1.0 - smoothstep(0.0, 0.06, abs(fbm(q * 2.5 + 3.0) - 0.5));
            emissive = p.col_b.rgb * lava * (2.5 + sin(globals.time * 0.7 + h * 20.0));
        } else {
            albedo = mix(p.col_a.rgb, p.col_b.rgb, smoothstep(0.3, 0.7, h));
            albedo *= 0.7 + 0.6 * detail;
            // Craters.
            let cr = noise(q * 9.0);
            albedo *= 1.0 - smoothstep(0.85, 0.95, cr) * 0.35;
        }
    }

    // Clouds on ocean and rocky worlds.
    var cloud = 0.0;
    if kind == 1 || kind == 0 {
        let cn = rot_y(wn, t * 1.6);
        cloud = smoothstep(0.55, 0.8, fbm(cn * 3.5 + seed + vec3<f32>(fbm(cn * 2.0) * 2.0)));
        cloud *= select(0.35, 0.9, kind == 1);
    }
    albedo = mix(albedo, vec3<f32>(0.95), cloud);

    // Lighting: star at the world origin.
    let l = normalize(-p.center.xyz);
    let v = normalize(view.world_position - in.world_position.xyz);
    let ndl = dot(wn, l);
    let diffuse = smoothstep(-0.15, 0.6, ndl);
    let h = normalize(l + v);
    let sp = pow(max(dot(wn, h), 0.0), 60.0) * spec * (1.0 - cloud);
    var c = albedo * diffuse * p.star.rgb * p.star.w + vec3<f32>(sp) * p.star.rgb * p.star.w;
    c += albedo * 0.015;
    c += emissive * (1.0 - diffuse * 0.6);

    // Atmospheric rim: brightest at the limb, mostly on the day side.
    let rim = pow(1.0 - max(dot(wn, v), 0.0), 3.0);
    let day = smoothstep(-0.3, 0.4, ndl);
    c += p.atmo.rgb * rim * p.atmo.w * (0.15 + day);
    return vec4<f32>(c, 1.0);
}

//! Asset pipeline for the game's GLB models (docs/STUDIO_PIPELINE.md): GPU textures and tangents.
//!
//!     cargo run -p assetpipe --release -- MODEL.glb [OUT.glb] [--max-px N]
//!
//! Blender exports PNG textures; Bevy uploads a PNG as uncompressed RGBA with one mip level, so a
//! 4096 px map costs 64 MB of VRAM and shimmers at a distance. This rewrites every PNG/JPEG image in
//! the GLB as KTX2 the GPU samples directly: BC7 for colour/metal-roughness/emissive/occlusion, BC5
//! for normal maps (Bevy rebuilds Z), a full mip chain (colour filtered in linear light, normals
//! renormalised), zstd supercompression. Images already in KTX2 are kept, so running it twice is a
//! no-op. Bevy reads the KTX2 through the image `mimeType` (it ignores KHR_texture_basisu, and BC7 is
//! not Basis anyway), so the output is a Bevy runtime asset, not a portable glTF; the Blender scripts
//! stay the source of truth.
//!
//! Tangents: Blender exports none, so Bevy ran MikkTSpace on every normal-mapped primitive at every
//! load. This runs the same code (`bevy_mikktspace`, same sign flip as `bevy_mesh`) once and stores
//! the result as TANGENT, so the shading is unchanged and the load skips it.

use serde_json::{Value, json};
use std::collections::BTreeMap;

// intel_tex_2 ships prebuilt C++ (ISPC) objects; MSVC links its C++ runtime on its own, GNU ld needs
// libstdc++ named (CI on Ubuntu failed with `undefined symbol: __gxx_personality_v0`).
#[cfg(target_os = "linux")]
#[link(name = "stdc++")]
unsafe extern "C" {}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Usage {
    Srgb,
    Linear,
    Normal,
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flag = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned();
    let max_px: Option<u32> = flag("--max-px").map(|v| v.parse().expect("--max-px N"));
    let files: Vec<&String> = args.iter().enumerate().filter(|(i, a)| !a.starts_with("--") && (*i == 0 || args[i - 1] != "--max-px")).map(|(_, a)| a).collect();
    let Some(input) = files.first() else {
        eprintln!("usage: assetpipe MODEL.glb [OUT.glb] [--max-px N]");
        std::process::exit(2);
    };
    let output = files.get(1).unwrap_or(input);
    let bytes = std::fs::read(input).expect("read GLB");
    let (out, report, tangents) = optimize(&bytes, max_px);
    for r in &report {
        println!("  {:28} {:6} {:>5} px  file {:6.2} -> {:6.2} MB  gpu {:5.1} -> {:4.1} MB  decode {:4.0} -> {:3.0} ms", r.name, format!("{:?}", r.usage), r.px, r.file_before as f64 / 1e6, r.file_after as f64 / 1e6, r.gpu_before as f64 / 1e6, r.gpu_after as f64 / 1e6, r.load_before, r.load_after);
    }
    let gpu: (usize, usize) = report.iter().fold((0, 0), |a, r| (a.0 + r.gpu_before, a.1 + r.gpu_after));
    println!("{input}: {:.1} -> {:.1} MB on disk, textures {:.0} -> {:.0} MB VRAM", bytes.len() as f64 / 1e6, out.len() as f64 / 1e6, gpu.0 as f64 / 1e6, gpu.1 as f64 / 1e6);
    if tangents.primitives > 0 {
        println!("  tangents: {} primitives, {} vertices, {:.0} ms (Bevy spent this at every load)", tangents.primitives, tangents.vertices, tangents.ms);
    }
    if out != bytes {
        std::fs::write(output, &out).expect("write GLB");
    }
}

struct Report {
    name: String,
    usage: Usage,
    px: u32,
    file_before: usize,
    file_after: usize,
    gpu_before: usize,
    gpu_after: usize,
    load_before: f64,
    load_after: f64,
}

#[derive(Default)]
struct TangentReport {
    primitives: usize,
    vertices: usize,
    ms: f64,
}

fn read_glb(bytes: &[u8]) -> (Value, Vec<u8>) {
    let u32_at = |o: usize| u32::from_le_bytes(bytes[o..o + 4].try_into().unwrap()) as usize;
    assert!(&bytes[0..4] == b"glTF" && u32_at(4) == 2, "not a glTF 2.0 binary");
    let (mut off, mut doc, mut bin) = (12, None, Vec::new());
    while off < u32_at(8).min(bytes.len()) {
        let (len, kind) = (u32_at(off), &bytes[off + 4..off + 8]);
        let chunk = &bytes[off + 8..off + 8 + len];
        match kind {
            b"JSON" => doc = Some(serde_json::from_slice(chunk).expect("GLB JSON")),
            b"BIN\0" => bin = chunk.to_vec(),
            _ => {}
        }
        off += 8 + len;
    }
    (doc.expect("GLB without JSON"), bin)
}

fn write_glb(doc: &Value, bin: &[u8]) -> Vec<u8> {
    let mut json = serde_json::to_vec(doc).unwrap();
    while !json.len().is_multiple_of(4) {
        json.push(b' ');
    }
    let mut out = Vec::with_capacity(28 + json.len() + bin.len());
    out.extend_from_slice(b"glTF");
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&((12 + 8 + json.len() + 8 + bin.len()) as u32).to_le_bytes());
    out.extend_from_slice(&(json.len() as u32).to_le_bytes());
    out.extend_from_slice(b"JSON");
    out.extend_from_slice(&json);
    out.extend_from_slice(&(bin.len() as u32).to_le_bytes());
    out.extend_from_slice(b"BIN\0");
    out.extend_from_slice(bin);
    out
}

/// How each image is sampled, from the material slots that use it (normal wins over linear over sRGB
/// if a texture is shared, which the exporter never does).
fn usages(doc: &Value) -> BTreeMap<usize, Usage> {
    let image_of = |tex: &Value| -> Option<usize> { doc["textures"][tex["index"].as_u64()? as usize]["source"].as_u64().map(|i| i as usize) };
    let mut out = BTreeMap::new();
    let mut mark = |slot: &Value, usage: Usage| {
        if let Some(img) = image_of(slot) {
            let e = out.entry(img).or_insert(usage);
            if usage == Usage::Normal || (usage == Usage::Linear && *e == Usage::Srgb) {
                *e = usage;
            }
        }
    };
    for m in doc["materials"].as_array().into_iter().flatten() {
        mark(&m["pbrMetallicRoughness"]["baseColorTexture"], Usage::Srgb);
        mark(&m["emissiveTexture"], Usage::Srgb);
        mark(&m["pbrMetallicRoughness"]["metallicRoughnessTexture"], Usage::Linear);
        mark(&m["occlusionTexture"], Usage::Linear);
        mark(&m["normalTexture"], Usage::Normal);
    }
    out
}

fn optimize(bytes: &[u8], max_px: Option<u32>) -> (Vec<u8>, Vec<Report>, TangentReport) {
    let (mut doc, bin) = read_glb(bytes);
    let views: Vec<Value> = doc["bufferViews"].as_array().cloned().unwrap_or_default();
    let view_bytes = |v: &Value| {
        let off = v["byteOffset"].as_u64().unwrap_or(0) as usize;
        bin[off..off + v["byteLength"].as_u64().unwrap() as usize].to_vec()
    };
    let mut data: Vec<Vec<u8>> = views.iter().map(view_bytes).collect();
    let usage = usages(&doc);
    let images = doc["images"].as_array().cloned().unwrap_or_default();
    // Encode images in parallel: a 4096 px BC7 map with mips takes a few seconds on one core.
    let jobs: Vec<(usize, usize, Usage)> = images
        .iter()
        .enumerate()
        .filter(|(_, img)| img["mimeType"] != "image/ktx2")
        .filter_map(|(i, img)| Some((i, img["bufferView"].as_u64()? as usize, usage.get(&i).copied().unwrap_or(Usage::Srgb))))
        .collect();
    let encoded: Vec<(usize, Vec<u8>, Report)> = std::thread::scope(|s| {
        let handles: Vec<_> = jobs
            .iter()
            .map(|&(i, view, u)| {
                let src = &data[view];
                let name = images[i]["name"].as_str().unwrap_or("").to_string();
                s.spawn(move || {
                    let t = std::time::Instant::now();
                    let img = image::load_from_memory(src).expect("decode texture").to_rgba8();
                    let load_before = t.elapsed().as_secs_f64() * 1e3;
                    let gpu_before = (img.width() * img.height() * 4) as usize;
                    let (ktx, px, gpu_after) = encode_ktx2(img, u, max_px);
                    // What the game pays at load: PNG decode vs unpacking the zstd levels (BC data uploads as is).
                    let t = std::time::Instant::now();
                    let r = ktx2::Reader::new(&ktx[..]).expect("own KTX2");
                    for l in r.levels() {
                        zstd::bulk::decompress(l.data, l.uncompressed_byte_length as usize).expect("own zstd");
                    }
                    let load_after = t.elapsed().as_secs_f64() * 1e3;
                    (i, ktx.clone(), Report { name, usage: u, px, file_before: src.len(), file_after: ktx.len(), gpu_before, gpu_after, load_before, load_after })
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });
    let mut report = Vec::new();
    for (i, ktx, r) in encoded {
        let view = images[i]["bufferView"].as_u64().unwrap() as usize;
        data[view] = ktx;
        doc["images"][i]["mimeType"] = json!("image/ktx2");
        report.push(r);
    }
    let tangents = add_tangents(&mut doc, &mut data);
    if report.is_empty() && tangents.primitives == 0 {
        return (bytes.to_vec(), report, tangents);
    }
    // Repack the binary chunk: every view keeps its index, offsets move, 4-byte aligned.
    let mut out_bin = Vec::new();
    for (i, d) in data.iter().enumerate() {
        while !out_bin.len().is_multiple_of(4) {
            out_bin.push(0);
        }
        doc["bufferViews"][i]["byteOffset"] = json!(out_bin.len());
        doc["bufferViews"][i]["byteLength"] = json!(d.len());
        out_bin.extend_from_slice(d);
    }
    while !out_bin.len().is_multiple_of(4) {
        out_bin.push(0);
    }
    doc["buffers"][0]["byteLength"] = json!(out_bin.len());
    (write_glb(&doc, &out_bin), report, tangents)
}

/// One accessor's bytes as (view data, start, stride, count); `elem` is the tightly packed element size.
fn accessor_bytes<'a>(doc: &Value, data: &'a [Vec<u8>], acc: usize, elem: usize) -> Option<(&'a [u8], usize, usize, usize)> {
    let a = &doc["accessors"][acc];
    if a.get("sparse").is_some() {
        return None;
    }
    let view = a["bufferView"].as_u64()? as usize;
    let stride = doc["bufferViews"][view]["byteStride"].as_u64().map(|s| s as usize).unwrap_or(elem);
    Some((&data[view], a["byteOffset"].as_u64().unwrap_or(0) as usize, stride, a["count"].as_u64()? as usize))
}

fn floats<const N: usize>(doc: &Value, data: &[Vec<u8>], acc: usize) -> Option<Vec<[f32; N]>> {
    if doc["accessors"][acc]["componentType"] != 5126 {
        return None;
    }
    let (d, start, stride, count) = accessor_bytes(doc, data, acc, 4 * N)?;
    Some((0..count).map(|i| std::array::from_fn(|c| f32::from_le_bytes(d[start + i * stride + c * 4..][..4].try_into().unwrap()))).collect())
}

fn indices(doc: &Value, data: &[Vec<u8>], acc: usize) -> Option<Vec<u32>> {
    let size = match doc["accessors"][acc]["componentType"].as_u64()? {
        5121 => 1,
        5123 => 2,
        5125 => 4,
        _ => return None,
    };
    let (d, start, stride, count) = accessor_bytes(doc, data, acc, size)?;
    Some(
        (0..count)
            .map(|i| {
                let b = &d[start + i * stride..];
                match size {
                    1 => b[0] as u32,
                    2 => u16::from_le_bytes([b[0], b[1]]) as u32,
                    _ => u32::from_le_bytes(b[..4].try_into().unwrap()),
                }
            })
            .collect(),
    )
}

/// Mirrors bevy_mesh's `MikktspaceGeometryHelper`: one tangent per vertex, the last face to touch a
/// shared vertex wins.
struct Geometry<'a> {
    idx: &'a [u32],
    pos: &'a [[f32; 3]],
    nrm: &'a [[f32; 3]],
    uv: &'a [[f32; 2]],
    out: Vec<[f32; 4]>,
}

impl bevy_mikktspace::Geometry for Geometry<'_> {
    fn num_faces(&self) -> usize {
        self.idx.len() / 3
    }
    fn num_vertices_of_face(&self, _: usize) -> usize {
        3
    }
    fn position(&self, face: usize, vert: usize) -> [f32; 3] {
        self.pos[self.idx[face * 3 + vert] as usize]
    }
    fn normal(&self, face: usize, vert: usize) -> [f32; 3] {
        self.nrm[self.idx[face * 3 + vert] as usize]
    }
    fn tex_coord(&self, face: usize, vert: usize) -> [f32; 2] {
        self.uv[self.idx[face * 3 + vert] as usize]
    }
    fn set_tangent(&mut self, ts: Option<bevy_mikktspace::TangentSpace>, face: usize, vert: usize) {
        self.out[self.idx[face * 3 + vert] as usize] = ts.unwrap_or_default().tangent_encoded();
    }
}

/// Tangents exactly as Bevy's glTF loader would generate them (same condition: triangles with
/// normals, a material with a normal map, no TANGENT yet), stored as a TANGENT accessor. Primitives
/// that share the same geometry accessors share one result.
fn add_tangents(doc: &mut Value, data: &mut Vec<Vec<u8>>) -> TangentReport {
    let mut report = TangentReport::default();
    let mut done: BTreeMap<(u64, u64, u64, Option<u64>), usize> = BTreeMap::new();
    let meshes = doc["meshes"].as_array().map(Vec::len).unwrap_or(0);
    for m in 0..meshes {
        let prims = doc["meshes"][m]["primitives"].as_array().map(Vec::len).unwrap_or(0);
        for p in 0..prims {
            let prim = &doc["meshes"][m]["primitives"][p];
            let attr = &prim["attributes"];
            let normal_mapped = prim["material"].as_u64().is_some_and(|mat| doc["materials"][mat as usize]["normalTexture"].is_object());
            if !normal_mapped || attr.get("TANGENT").is_some() || prim["mode"].as_u64().unwrap_or(4) != 4 {
                continue;
            }
            let (Some(pos), Some(nrm), Some(uv)) = (attr["POSITION"].as_u64(), attr["NORMAL"].as_u64(), attr["TEXCOORD_0"].as_u64()) else {
                continue;
            };
            let key = (pos, nrm, uv, prim["indices"].as_u64());
            let acc = if let Some(&acc) = done.get(&key) {
                acc
            } else {
                let (Some(p3), Some(n3), Some(t2)) = (floats::<3>(doc, data, pos as usize), floats::<3>(doc, data, nrm as usize), floats::<2>(doc, data, uv as usize)) else {
                    continue;
                };
                let idx = match key.3 {
                    Some(i) => match indices(doc, data, i as usize) {
                        Some(v) => v,
                        None => continue,
                    },
                    None => (0..p3.len() as u32).collect(),
                };
                let t = std::time::Instant::now();
                let mut g = Geometry { idx: &idx, pos: &p3, nrm: &n3, uv: &t2, out: vec![[0.0; 4]; p3.len()] };
                if bevy_mikktspace::generate_tangents(&mut g).is_err() {
                    continue;
                }
                // bevy_mesh: mikktspace assumes left-handedness, so Bevy flips the sign.
                for tangent in &mut g.out {
                    tangent[3] = -tangent[3];
                }
                report.ms += t.elapsed().as_secs_f64() * 1e3;
                report.vertices += g.out.len();
                data.push(g.out.iter().flatten().flat_map(|f| f.to_le_bytes()).collect());
                doc["bufferViews"].as_array_mut().unwrap().push(json!({ "buffer": 0, "byteLength": g.out.len() * 16, "target": 34962 }));
                let accessors = doc["accessors"].as_array_mut().unwrap();
                accessors.push(json!({ "bufferView": data.len() - 1, "componentType": 5126, "count": g.out.len(), "type": "VEC4" }));
                done.insert(key, accessors.len() - 1);
                accessors.len() - 1
            };
            doc["meshes"][m]["primitives"][p]["attributes"]["TANGENT"] = json!(acc);
            report.primitives += 1;
        }
    }
    report
}

fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

fn linear_to_srgb(c: f32) -> f32 {
    if c <= 0.0031308 { c * 12.92 } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 }
}

/// The mip chain as RGBA8 levels, largest first. Colour is averaged in linear light, normals are
/// averaged as vectors and renormalised, everything else is a plain 2x2 box.
fn mips(img: image::RgbaImage, usage: Usage) -> Vec<image::RgbaImage> {
    let decode = |v: u8| match usage {
        Usage::Srgb => srgb_to_linear(v as f32 / 255.0),
        Usage::Normal => v as f32 / 255.0 * 2.0 - 1.0,
        Usage::Linear => v as f32 / 255.0,
    };
    let encode = |v: f32| {
        let u = match usage {
            Usage::Srgb => linear_to_srgb(v),
            Usage::Normal => v * 0.5 + 0.5,
            Usage::Linear => v,
        };
        (u.clamp(0.0, 1.0) * 255.0).round() as u8
    };
    let mut levels = vec![img];
    loop {
        let prev = levels.last().unwrap();
        let (w, h) = prev.dimensions();
        if w == 1 && h == 1 {
            break;
        }
        let (nw, nh) = ((w / 2).max(1), (h / 2).max(1));
        let mut next = image::RgbaImage::new(nw, nh);
        for y in 0..nh {
            for x in 0..nw {
                let mut acc = [0f32; 4];
                let mut n = 0.0;
                for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    let (sx, sy) = ((x * 2 + dx).min(w - 1), (y * 2 + dy).min(h - 1));
                    let p = prev.get_pixel(sx, sy).0;
                    for c in 0..3 {
                        acc[c] += decode(p[c]);
                    }
                    acc[3] += p[3] as f32 / 255.0;
                    n += 1.0;
                }
                let mut px = [0u8; 4];
                if usage == Usage::Normal {
                    let len = (acc[0] * acc[0] + acc[1] * acc[1] + acc[2] * acc[2]).sqrt().max(1e-6);
                    for c in 0..3 {
                        px[c] = encode(acc[c] / len);
                    }
                } else {
                    for c in 0..3 {
                        px[c] = encode(acc[c] / n);
                    }
                }
                px[3] = ((acc[3] / n).clamp(0.0, 1.0) * 255.0).round() as u8;
                next.put_pixel(x, y, image::Rgba(px));
            }
        }
        levels.push(next);
    }
    levels
}

/// Block encoders need whole 4x4 blocks: pad the 2x2 and 1x1 mips by repeating their edge.
fn padded(img: &image::RgbaImage) -> image::RgbaImage {
    let (w, h) = img.dimensions();
    let (pw, ph) = (w.div_ceil(4) * 4, h.div_ceil(4) * 4);
    if (pw, ph) == (w, h) {
        return img.clone();
    }
    image::RgbaImage::from_fn(pw, ph, |x, y| *img.get_pixel(x.min(w - 1), y.min(h - 1)))
}

// Vulkan format numbers (KTX2 vkFormat) and Khronos data format descriptor values.
const VK_BC5_UNORM: u32 = 141;
const VK_BC7_UNORM: u32 = 145;
const VK_BC7_SRGB: u32 = 146;
const KHR_DF_MODEL_BC5: u32 = 132;
const KHR_DF_MODEL_BC7: u32 = 134;
const ZSTD_LEVEL: i32 = 19;

/// One image as a KTX2 file: (bytes, final edge in px, VRAM bytes of all levels).
fn encode_ktx2(mut img: image::RgbaImage, usage: Usage, max_px: Option<u32>) -> (Vec<u8>, u32, usize) {
    if let Some(m) = max_px {
        while img.width() > m || img.height() > m {
            img = mips(img, usage).swap_remove(1);
        }
    }
    let (w, h) = img.dimensions();
    let opaque = img.pixels().all(|p| p.0[3] == 255);
    let levels: Vec<Vec<u8>> = mips(img, usage)
        .iter()
        .map(|level| {
            let p = padded(level);
            if usage == Usage::Normal {
                let rg: Vec<u8> = p.pixels().flat_map(|px| [px.0[0], px.0[1]]).collect();
                intel_tex_2::bc5::compress_blocks(&intel_tex_2::RgSurface { data: &rg, width: p.width(), height: p.height(), stride: p.width() * 2 })
            } else {
                let settings = if opaque { intel_tex_2::bc7::opaque_basic_settings() } else { intel_tex_2::bc7::alpha_basic_settings() };
                intel_tex_2::bc7::compress_blocks(&settings, &intel_tex_2::RgbaSurface { data: p.as_raw(), width: p.width(), height: p.height(), stride: p.width() * 4 })
            }
        })
        .collect();
    let gpu: usize = levels.iter().map(Vec::len).sum();
    let (vk, model, transfer) = match usage {
        Usage::Srgb => (VK_BC7_SRGB, KHR_DF_MODEL_BC7, 2),
        Usage::Linear => (VK_BC7_UNORM, KHR_DF_MODEL_BC7, 1),
        Usage::Normal => (VK_BC5_UNORM, KHR_DF_MODEL_BC5, 1),
    };
    (ktx2_file(vk, model, transfer, w, h, &levels), w, gpu)
}

/// KTX2 container (Khronos KTX 2.0 spec): header, level index, basic DFD, zstd-compressed levels
/// stored smallest first.
fn ktx2_file(vk: u32, model: u32, transfer: u32, w: u32, h: u32, levels: &[Vec<u8>]) -> Vec<u8> {
    let compressed: Vec<Vec<u8>> = levels.iter().map(|l| zstd::bulk::compress(l, ZSTD_LEVEL).expect("zstd")).collect();
    // Samples: BC7 is one 128-bit colour sample; BC5 is red then green, 64 bits each.
    let samples: Vec<[u32; 4]> = if model == KHR_DF_MODEL_BC5 { vec![[63 << 16, 0, 0, u32::MAX], [64 | (63 << 16) | (1 << 24), 0, 0, u32::MAX]] } else { vec![[127 << 16, 0, 0, u32::MAX]] };
    let block_size = 24 + 16 * samples.len() as u32;
    let mut dfd: Vec<u32> = vec![4 + block_size, 0, 2 | (block_size << 16), model | (1 << 8) | (transfer << 16), 3 | (3 << 8), 16, 0];
    for s in &samples {
        dfd.extend_from_slice(s);
    }
    let header_len = 80 + 24 * levels.len();
    let dfd_bytes: Vec<u8> = dfd.iter().flat_map(|v| v.to_le_bytes()).collect();
    let mut out = Vec::new();
    out.extend_from_slice(&[0xAB, 0x4B, 0x54, 0x58, 0x20, 0x32, 0x30, 0xBB, 0x0D, 0x0A, 0x1A, 0x0A]);
    for v in [vk, 1, w, h, 0, 0, 1, levels.len() as u32, 2] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    for v in [header_len as u32, dfd_bytes.len() as u32, 0, 0] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out.extend_from_slice(&0u64.to_le_bytes());
    out.extend_from_slice(&0u64.to_le_bytes());
    // Level data follows the DFD, smallest level first; the index lists the largest first.
    let mut offsets = vec![0usize; levels.len()];
    let mut at = header_len + dfd_bytes.len();
    for i in (0..levels.len()).rev() {
        offsets[i] = at;
        at += compressed[i].len();
    }
    for i in 0..levels.len() {
        for v in [offsets[i] as u64, compressed[i].len() as u64, levels[i].len() as u64] {
            out.extend_from_slice(&v.to_le_bytes());
        }
    }
    out.extend_from_slice(&dfd_bytes);
    for c in compressed.iter().rev() {
        out.extend_from_slice(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ktx2_round_trips_through_the_reader_bevy_uses() {
        let img = image::RgbaImage::from_fn(64, 64, |x, y| image::Rgba([(x * 4) as u8, (y * 4) as u8, 128, 255]));
        let (file, px, gpu) = encode_ktx2(img, Usage::Srgb, None);
        assert_eq!(px, 64);
        let r = ktx2::Reader::new(&file[..]).expect("valid KTX2");
        let head = r.header();
        assert_eq!(head.format, Some(ktx2::Format::BC7_SRGB_BLOCK));
        assert_eq!((head.pixel_width, head.pixel_height, head.level_count), (64, 64, 7));
        assert_eq!(head.supercompression_scheme, Some(ktx2::SupercompressionScheme::Zstandard));
        let sizes: Vec<usize> = r.levels().map(|l| zstd::bulk::decompress(l.data, 1 << 20).unwrap().len()).collect();
        // 16x16 blocks, 8x8, 4x4, 2x2, then the 4x4 and smaller mips are one block each.
        assert_eq!(sizes, vec![4096, 1024, 256, 64, 16, 16, 16]);
        assert_eq!(gpu, sizes.iter().sum::<usize>());
        assert_eq!(r.color_model(), Some(ktx2::ColorModel::BC7));
        assert_eq!(r.transfer_function(), Some(ktx2::TransferFunction::SRGB));
    }

    #[test]
    fn normal_maps_become_bc5_and_mips_stay_unit_length() {
        let img = image::RgbaImage::from_fn(8, 8, |x, _| if x % 2 == 0 { image::Rgba([255, 128, 128, 255]) } else { image::Rgba([128, 128, 255, 255]) });
        let levels = mips(img.clone(), Usage::Normal);
        let p = levels[1].get_pixel(0, 0).0;
        let v: Vec<f32> = p[..3].iter().map(|c| *c as f32 / 255.0 * 2.0 - 1.0).collect();
        let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        assert!((len - 1.0).abs() < 0.02, "averaged normal renormalised, got {len}");
        let (file, ..) = encode_ktx2(img, Usage::Normal, None);
        assert_eq!(ktx2::Reader::new(&file[..]).unwrap().header().format, Some(ktx2::Format::BC5_UNORM_BLOCK));
    }

    #[test]
    fn tangents_follow_u_and_only_for_normal_mapped_primitives() {
        // A quad in the XY plane facing +Z, U along +X: the tangent is +X. Two primitives share it,
        // one with a normal-mapped material, one without.
        let f = |v: &[f32]| v.iter().flat_map(|x| x.to_le_bytes()).collect::<Vec<u8>>();
        let mut data = vec![
            f(&[0., 0., 0., 1., 0., 0., 1., 1., 0., 0., 1., 0.]),
            f(&[0., 0., 1., 0., 0., 1., 0., 0., 1., 0., 0., 1.]),
            f(&[0., 1., 1., 1., 1., 0., 0., 0.]),
            [0u16, 1, 2, 0, 2, 3].iter().flat_map(|i| i.to_le_bytes()).collect(),
        ];
        let mut doc = json!({
            "bufferViews": [{"byteLength": 48}, {"byteLength": 48}, {"byteLength": 32}, {"byteLength": 12}],
            "accessors": [
                {"bufferView": 0, "componentType": 5126, "count": 4, "type": "VEC3"},
                {"bufferView": 1, "componentType": 5126, "count": 4, "type": "VEC3"},
                {"bufferView": 2, "componentType": 5126, "count": 4, "type": "VEC2"},
                {"bufferView": 3, "componentType": 5123, "count": 6, "type": "SCALAR"}
            ],
            "materials": [{"normalTexture": {"index": 0}}, {}],
            "meshes": [{"primitives": [
                {"attributes": {"POSITION": 0, "NORMAL": 1, "TEXCOORD_0": 2}, "indices": 3, "material": 0},
                {"attributes": {"POSITION": 0, "NORMAL": 1, "TEXCOORD_0": 2}, "indices": 3, "material": 1}
            ]}]
        });
        let r = add_tangents(&mut doc, &mut data);
        assert_eq!((r.primitives, r.vertices), (1, 4));
        let acc = doc["meshes"][0]["primitives"][0]["attributes"]["TANGENT"].as_u64().unwrap() as usize;
        assert!(doc["meshes"][0]["primitives"][1]["attributes"].get("TANGENT").is_none());
        for t in floats::<4>(&doc, &data, acc).unwrap() {
            assert!((t[0] - 1.0).abs() < 1e-5 && t[1].abs() < 1e-5 && t[2].abs() < 1e-5 && t[3].abs() == 1.0, "{t:?}");
        }
        assert_eq!(add_tangents(&mut doc, &mut data).primitives, 0, "a second run adds nothing");
    }

    #[test]
    fn srgb_mips_average_in_linear_light() {
        // Black and white stripes average to linear 0.5, which is sRGB 188, not 128.
        let img = image::RgbaImage::from_fn(2, 2, |x, _| if x == 0 { image::Rgba([0, 0, 0, 255]) } else { image::Rgba([255, 255, 255, 255]) });
        assert_eq!(mips(img, Usage::Srgb)[1].get_pixel(0, 0).0[0], 188);
    }
}

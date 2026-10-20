#!/usr/bin/env python3
"""Asset budgets for the 3D models (docs/STUDIO_PIPELINE.md): measure every GLB and fail the ones
over their budget, so a regenerated or AI-made model cannot quietly double the download or the GPU load.

    python tools/assetcheck.py                 # table + PASS/FAIL per model, exit 1 on any FAIL
    python tools/assetcheck.py --json out.json # also write the measurements
    python tools/assetcheck.py --only talon    # models whose file name contains "talon"
    python tools/assetcheck.py --budgets F     # another budget file (what-if)
    python tools/assetcheck.py --dir D         # models in another folder (e.g. a fresh Blender export)

Reads the GLB container directly (JSON chunk + embedded image headers): no Blender, no GPU, no
packages. Budgets live in tools/asset_budgets.json (first matching glob wins); raise one only on
purpose, in the same commit as the model that needs it.
"""
import fnmatch
import json
import os
import struct
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
MODELS = os.path.join(ROOT, "crates", "client", "assets", "models")
BUDGETS = os.path.join(ROOT, "tools", "asset_budgets.json")


def read_glb(path):
    with open(path, "rb") as f:
        data = f.read()
    magic, version, length = struct.unpack_from("<4sII", data, 0)
    if magic != b"glTF" or version != 2:
        raise ValueError("not a glTF 2.0 binary")
    off, doc, binary = 12, None, b""
    while off < length:
        clen, ctype = struct.unpack_from("<I4s", data, off)
        chunk = data[off + 8 : off + 8 + clen]
        if ctype == b"JSON":
            doc = json.loads(chunk.decode("utf-8"))
        elif ctype == b"BIN\x00":
            binary = chunk
        off += 8 + clen
    return doc, binary, len(data)


LOD_SUFFIX = "__lod1"
KTX2_MAGIC = b"\xabKTX 20\xbb\r\n\x1a\n"


def image_size(blob):
    """(width, height) of a PNG, JPEG or KTX2, from its header."""
    if blob[:12] == KTX2_MAGIC:
        return struct.unpack_from("<II", blob, 20)
    if blob[:8] == b"\x89PNG\r\n\x1a\n":
        return struct.unpack(">II", blob[16:24])
    if blob[:2] == b"\xff\xd8":
        i = 2
        while i < len(blob) - 9:
            if blob[i] != 0xFF:
                i += 1
                continue
            marker = blob[i + 1]
            if marker in (0xC0, 0xC1, 0xC2):
                h, w = struct.unpack(">HH", blob[i + 5 : i + 9])
                return w, h
            i += 2 + struct.unpack(">H", blob[i + 2 : i + 4])[0]
    return 0, 0


def gpu_bytes(blob):
    """VRAM one texture takes in the game. Bevy uploads PNG/JPEG as RGBA8 with one mip level; a KTX2
    (cargo run -p assetpipe) uploads its block-compressed levels as stored."""
    if blob[:12] == KTX2_MAGIC:
        levels = struct.unpack_from("<I", blob, 40)[0]
        return sum(struct.unpack_from("<Q", blob, 80 + 24 * i + 16)[0] for i in range(max(levels, 1)))
    w, h = image_size(blob)
    return w * h * 4


def measure(path):
    doc, binary, size = read_glb(path)
    acc = doc.get("accessors", [])
    views = doc.get("bufferViews", [])
    positions, indices = set(), 0
    for mesh in doc.get("meshes", []):
        for prim in mesh.get("primitives", []):
            positions.add(prim["attributes"]["POSITION"])
            if "indices" in prim:
                indices += acc[prim["indices"]]["count"]
            else:
                indices += acc[prim["attributes"]["POSITION"]]["count"]
    # Instanced draws (the same mesh on several nodes) render several times: count what the GPU draws,
    # up close (full detail) and far (every `X__lod1` from crates/assetpipe --lod in place of X).
    nodes = doc.get("nodes", [])
    def tris(m):
        return sum((acc[p["indices"]]["count"] if "indices" in p else acc[p["attributes"]["POSITION"]]["count"]) // 3 for p in doc["meshes"][m].get("primitives", []))
    drawn, drawn_far, lods = 0, 0, 0
    for node in nodes:
        if "mesh" not in node or node.get("name", "").endswith(LOD_SUFFIX):
            continue
        lod = [nodes[c] for c in node.get("children", []) if nodes[c].get("name", "").endswith(LOD_SUFFIX) and "mesh" in nodes[c]]
        drawn += tris(node["mesh"])
        drawn_far += tris(lod[0]["mesh"]) if lod else tris(node["mesh"])
        lods += bool(lod)
    tex_max, tex_bytes, tex_gpu, ktx2 = 0, 0, 0, 0
    for img in doc.get("images", []):
        if "bufferView" in img:
            v = views[img["bufferView"]]
            blob = binary[v.get("byteOffset", 0) : v.get("byteOffset", 0) + v["byteLength"]]
            w, h = image_size(blob)
            tex_max = max(tex_max, w, h)
            tex_bytes += v["byteLength"]
            tex_gpu += gpu_bytes(blob)
            ktx2 += blob[:12] == KTX2_MAGIC
    return {
        "file_mb": round(size / 1e6, 2),
        "vertices": sum(acc[a]["count"] for a in positions),
        "triangles": indices // 3,
        "drawn_triangles": drawn,
        "drawn_far": drawn_far,
        "lod_nodes": lods,
        "materials": len(doc.get("materials", [])),
        "textures": len(doc.get("images", [])),
        "texture_max_px": tex_max,
        "texture_mb": round(tex_bytes / 1e6, 2),
        "texture_gpu_mb": round(tex_gpu / 1e6, 1),
        "textures_ktx2": ktx2,
        "nodes": len(doc.get("nodes", [])),
    }


def budget_for(name, budgets):
    for rule in budgets["rules"]:
        if fnmatch.fnmatch(name, rule["match"]):
            return rule
    return None


def main(argv):
    only = argv[argv.index("--only") + 1] if "--only" in argv else None
    out = argv[argv.index("--json") + 1] if "--json" in argv else None
    budgets_path = argv[argv.index("--budgets") + 1] if "--budgets" in argv else BUDGETS
    models = argv[argv.index("--dir") + 1] if "--dir" in argv else MODELS
    with open(budgets_path, encoding="utf-8") as f:
        budgets = json.load(f)
    keys = ["file_mb", "vertices", "drawn_triangles", "materials", "texture_max_px", "texture_mb", "texture_gpu_mb"]
    rows, failed, results = [], 0, {}
    for name in sorted(os.listdir(models)):
        if not name.endswith(".glb") or (only and only not in name):
            continue
        m = measure(os.path.join(models, name))
        rule = budget_for(name, budgets)
        over = [k for k in keys if rule and k in rule["max"] and m[k] > rule["max"][k]]
        if rule and "lod" in rule and not m["lod_nodes"]:
            over.append(f"no LOD (run assetpipe --lod {rule['lod']})")
        results[name] = {"budget": rule["match"] if rule else None, "over": over, **m}
        if rule is None:
            over = ["no budget"]
        failed += bool(over)
        rows.append((name, rule["match"] if rule else "-", m, over))
    head = f"{'model':24} {'budget':16} {'MB':>6} {'verts':>8} {'drawn tris':>10} {'far tris':>8} {'mats':>4} {'tex px':>6} {'tex MB':>6} {'VRAM MB':>7} {'ktx2':>5}  result"
    print(head)
    print("-" * len(head))
    for name, rule, m, over in rows:
        verdict = "PASS" if not over else "FAIL " + ", ".join(over)
        print(f"{name:24} {rule:16} {m['file_mb']:6.1f} {m['vertices']:8d} {m['drawn_triangles']:10d} {m['drawn_far']:8d} {m['materials']:4d} {m['texture_max_px']:6d} {m['texture_mb']:6.1f} {m['texture_gpu_mb']:7.1f} {m['textures_ktx2']:2d}/{m['textures']:<2d}  {verdict}")
    total = sum(m["file_mb"] for _, _, m, _ in rows)
    gpu = sum(m["texture_gpu_mb"] for _, _, m, _ in rows)
    print(f"\n{len(rows)} models, {total:.0f} MB on disk, textures {gpu:.0f} MB VRAM if all loaded; budget total {budgets['total_mb']} MB")
    if not only and total > budgets["total_mb"]:
        print(f"FAIL total {total:.0f} MB > {budgets['total_mb']} MB")
        failed += 1
    if out:
        with open(out, "w", encoding="utf-8") as f:
            json.dump(results, f, indent=1)
    print("ASSETS OK" if not failed else f"ASSETS FAIL: {failed}")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))

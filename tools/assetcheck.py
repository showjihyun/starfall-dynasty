#!/usr/bin/env python3
"""Asset budgets for the 3D models (docs/STUDIO_PIPELINE.md): measure every GLB and fail the ones
over their budget, so a regenerated or AI-made model cannot quietly double the download or the GPU load.

    python tools/assetcheck.py                 # table + PASS/FAIL per model, exit 1 on any FAIL
    python tools/assetcheck.py --json out.json # also write the measurements
    python tools/assetcheck.py --only talon    # models whose file name contains "talon"
    python tools/assetcheck.py --budgets F     # another budget file (what-if)

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


def image_size(blob):
    """(width, height) of a PNG or JPEG, from its header."""
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
    # Instanced draws (the same mesh on several nodes) render several times: count what the GPU draws.
    uses = {}
    for node in doc.get("nodes", []):
        if "mesh" in node:
            uses[node["mesh"]] = uses.get(node["mesh"], 0) + 1
    drawn = 0
    for m, n in uses.items():
        for prim in doc["meshes"][m].get("primitives", []):
            count = acc[prim["indices"]]["count"] if "indices" in prim else acc[prim["attributes"]["POSITION"]]["count"]
            drawn += n * count // 3
    tex_max, tex_bytes = 0, 0
    for img in doc.get("images", []):
        if "bufferView" in img:
            v = views[img["bufferView"]]
            blob = binary[v.get("byteOffset", 0) : v.get("byteOffset", 0) + v["byteLength"]]
            w, h = image_size(blob)
            tex_max = max(tex_max, w, h)
            tex_bytes += v["byteLength"]
    return {
        "file_mb": round(size / 1e6, 2),
        "vertices": sum(acc[a]["count"] for a in positions),
        "triangles": indices // 3,
        "drawn_triangles": drawn,
        "materials": len(doc.get("materials", [])),
        "textures": len(doc.get("images", [])),
        "texture_max_px": tex_max,
        "texture_mb": round(tex_bytes / 1e6, 2),
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
    with open(budgets_path, encoding="utf-8") as f:
        budgets = json.load(f)
    keys = ["file_mb", "vertices", "drawn_triangles", "materials", "texture_max_px", "texture_mb"]
    rows, failed, results = [], 0, {}
    for name in sorted(os.listdir(MODELS)):
        if not name.endswith(".glb") or (only and only not in name):
            continue
        m = measure(os.path.join(MODELS, name))
        rule = budget_for(name, budgets)
        over = [k for k in keys if rule and k in rule["max"] and m[k] > rule["max"][k]]
        results[name] = {"budget": rule["match"] if rule else None, "over": over, **m}
        if rule is None:
            over = ["no budget"]
        failed += bool(over)
        rows.append((name, rule["match"] if rule else "-", m, over))
    head = f"{'model':24} {'budget':16} {'MB':>6} {'verts':>8} {'drawn tris':>10} {'mats':>4} {'tex px':>6} {'tex MB':>6}  result"
    print(head)
    print("-" * len(head))
    for name, rule, m, over in rows:
        verdict = "PASS" if not over else "FAIL " + ", ".join(over)
        print(f"{name:24} {rule:16} {m['file_mb']:6.1f} {m['vertices']:8d} {m['drawn_triangles']:10d} {m['materials']:4d} {m['texture_max_px']:6d} {m['texture_mb']:6.1f}  {verdict}")
    total = sum(m["file_mb"] for _, _, m, _ in rows)
    print(f"\n{len(rows)} models, {total:.0f} MB on disk; budget total {budgets['total_mb']} MB")
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

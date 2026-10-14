"""Print node names, materials, triangle counts and textures of GLB files.
Usage: python tools/blender/inspect_glb.py crates/client/assets/models/*.glb
"""
import json
import os
import struct
import sys


def load(path):
    with open(path, "rb") as f:
        data = f.read()
    magic, ver, length = struct.unpack_from("<III", data, 0)
    assert magic == 0x46546C67, "not a glb"
    clen, ctype = struct.unpack_from("<II", data, 12)
    return json.loads(data[20:20 + clen])


def main():
    for path in sys.argv[1:]:
        g = load(path)
        nodes = g.get("nodes", [])
        meshes = g.get("meshes", [])
        acc = g.get("accessors", [])
        print("==", os.path.basename(path), "%.2f MB" % (os.path.getsize(path) / 1e6))
        total = 0
        for n in nodes:
            extra = ""
            if "mesh" in n:
                tris = 0
                for p in meshes[n["mesh"]]["primitives"]:
                    if "indices" in p:
                        tris += acc[p["indices"]]["count"] // 3
                total += tris
                extra = "mesh tris=%d" % tris
            else:
                extra = "empty scale=%s" % [round(s, 3) for s in n.get("scale", [1, 1, 1])]
            t = [round(v, 2) for v in n.get("translation", [0, 0, 0])]
            print("  node %-28s %s pos=%s" % (n.get("name"), extra, t))
        print("  total tris", total)
        print("  materials", [m.get("name") for m in g.get("materials", [])])
        print("  images", len(g.get("images", [])), [i.get("mimeType") for i in g.get("images", [])])
        for m in g.get("materials", []):
            pbr = m.get("pbrMetallicRoughness", {})
            print("   ", m["name"], "base" if "baseColorTexture" in pbr else "", "mr" if "metallicRoughnessTexture" in pbr else "",
                  "normal" if "normalTexture" in m else "", "emissive=%s" % m.get("emissiveFactor"), m.get("extensions", {}))


if __name__ == "__main__":
    main()

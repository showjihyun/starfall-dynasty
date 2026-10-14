"""Print node and material names of GLB files: python tools/glb_names.py a.glb [b.glb ...]"""
import json
import struct
import sys

for path in sys.argv[1:]:
    data = open(path, "rb").read()
    length = struct.unpack("<I", data[12:16])[0]
    j = json.loads(data[20:20 + length])
    print(path)
    print("  nodes:", [n.get("name") for n in j.get("nodes", [])])
    print("  materials:", [m.get("name") for m in j.get("materials", [])])

"""Deposit asteroids (unit radius) + mineral crystal clusters (4 minerals x full/stump).

Run:  blender --background --factory-startup --python art/scripts/asteroids.py
Out:  art/export/asteroid_rock_{0..3}.fbx      unit-radius deposit rock, LOD0/LOD1, 3-4 satellite
                                               rocks, Socket_Crystal_{0..5} (+Z = surface normal)
      art/export/crystal_{mineral}.fbx         cluster (full) + _stump, sized for a unit-radius rock
      art/textures/asteroids/asteroid_rock_{i}_normal.png   baked high->low tangent normal (OpenGL)
      art/source/asteroids.blend, art/previews/asteroid_*.png

Data mapping (01_art_direction.md 2, 3):
- Unity scales the whole deposit root uniformly by deposits[].radius_m. Main rock bounding
  radius = 0.78, satellites stay inside 1.0 (asserted) -> nothing pokes out of the data radius.
- Rock variant = hash(deposit id) % 4. NEVER the mineral: unrevealed deposits must look alike.
- Crystals attach to Socket_Crystal_* only after DEPOSIT_FIELD_STATE.mineral_id is non-null;
  scale with remaining_kg / initial_reserve_kg, swap to *_stump at 0.
- Crystal emission is a vertex colour attribute "Emit" (R = mask) so one shared Unity shader
  can light veins/edges without per-mineral textures.
"""
import math
import os
import random
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import bpy
import bmesh
from mathutils import Vector, Matrix, noise
from mathutils.bvhtree import BVHTree

import common as C

SEED = 4242
N_ROCKS = 4
MAIN_R = 0.78
OUTER_R = 1.0
TEXOUT = os.path.join(C.TEX, "asteroids")

MINERALS = {
    # id: base colour, metallic, roughness, emission colour, emission strength, transmission
    "ferrosite": ("#B8703F", 0.55, 0.32, "#FF8A3A", 0.0, 0.0),
    "glacine": ("#BFE9F2", 0.0, 0.08, "#9FE8FF", 1.2, 0.85),
    "cobaltine": ("#1F49B8", 0.45, 0.22, "#2F7BFF", 1.6, 0.0),
    "starfall-glass": ("#0B0A10", 0.1, 0.03, "#FF6A3D", 5.0, 0.0),
}


# ----------------------------------------------------------------- rock sculpt

def rand_dir(rng):
    z = rng.uniform(-1, 1)
    a = rng.uniform(0, 2 * math.pi)
    r = math.sqrt(1 - z * z)
    return Vector((r * math.cos(a), r * math.sin(a), z))


def sculpt_rock(i, subdiv=6):
    rng = random.Random(SEED + i * 97)
    bm = bmesh.new()
    bmesh.ops.create_icosphere(bm, subdivisions=subdiv, radius=1.0)
    off = Vector((rng.uniform(-50, 50), rng.uniform(-50, 50), rng.uniform(-50, 50)))
    craters = []
    for _ in range(rng.randint(4, 6)):
        craters.append((rand_dir(rng), rng.uniform(0.18, 0.45), rng.uniform(0.05, 0.11)))
    for _ in range(8):
        craters.append((rand_dir(rng), rng.uniform(0.06, 0.12), rng.uniform(0.012, 0.025)))
    planes = [(rand_dir(rng), rng.uniform(0.74, 0.88)) for _ in range(rng.randint(6, 9))]
    stretch = Vector((1.0, rng.uniform(0.72, 0.9), rng.uniform(0.6, 0.78)))
    rot = Matrix.Rotation(rng.uniform(0, 6.28), 3, "Z") @ Matrix.Rotation(rng.uniform(0, 6.28), 3, "X")
    for v in bm.verts:
        d = v.co.normalized()
        p = d * 1.3 + off
        h = 0.2 * noise.fractal(p, 0.5, 2.0, 3, noise_basis="PERLIN_NEW")
        h += 0.018 * noise.ridged_multi_fractal(d * 3.0 + off, 0.7, 2.1, 4, 0.9, 2.0, noise_basis="PERLIN_NEW")
        h += 0.006 * noise.fractal(d * 14.0 + off, 0.6, 2.0, 3, noise_basis="PERLIN_NEW")
        # embedded boulders / gravel: voronoi cells raised into lumps
        vd = noise.voronoi(d * 11.0 + off)[0]
        h += 0.022 * max(0.0, 0.42 - vd[0]) / 0.42
        vd2 = noise.voronoi(d * 29.0 + off * 1.7)[0]
        h += 0.009 * max(0.0, 0.4 - vd2[0]) / 0.4
        # faint strata
        h += 0.004 * math.sin(d.dot(Vector((0.3, 0.8, 0.52))) * 70.0 + 3.0 * noise.noise(d * 5.0 + off))
        for c, r, dep in craters:
            a = math.acos(max(-1.0, min(1.0, d.dot(c))))
            x = a / r
            if x < 1.0:
                h -= dep * (1.0 - x * x)
            if 0.75 < x < 1.4:
                h += dep * 0.3 * math.exp(-((x - 1.0) / 0.14) ** 2)
        pos = d * (1.0 + h)
        pos = Vector((pos.x * stretch.x, pos.y * stretch.y, pos.z * stretch.z))
        for n, o in planes:  # chipped flat facets
            k = pos.dot(n)
            if k > o:
                pos -= n * (k - o) * 0.97
        v.co = rot @ pos
    # normalise to MAIN_R
    rmax = max(v.co.length for v in bm.verts)
    for v in bm.verts:
        v.co *= MAIN_R / rmax
    return bm


def rock_material():
    """Basalt: CC0 dark_rock_02 box-projected (Unity: triplanar) + baked normal on UV0."""
    m = bpy.data.materials.new("Rock_Basalt")
    m.use_nodes = True
    nt = m.node_tree
    b = nt.nodes["Principled BSDF"]
    tc = nt.nodes.new("ShaderNodeTexCoord")
    mp = nt.nodes.new("ShaderNodeMapping")
    mp.inputs["Scale"].default_value = (5.0, 5.0, 5.0)
    nt.links.new(tc.outputs["Object"], mp.inputs["Vector"])

    def boxtex(fn, nc=False):
        t = nt.nodes.new("ShaderNodeTexImage")
        t.image = C._img(os.path.join(C.PH, fn), nc)
        t.projection = "BOX"
        t.projection_blend = 0.3
        nt.links.new(mp.outputs["Vector"], t.inputs["Vector"])
        return t

    diff = boxtex("dark_rock_02_diff_2k.jpg")
    rough = boxtex("dark_rock_02_rough_2k.jpg", True)
    hsv = nt.nodes.new("ShaderNodeHueSaturation")
    hsv.inputs["Saturation"].default_value = 0.3
    hsv.inputs["Value"].default_value = 1.25
    nt.links.new(diff.outputs["Color"], hsv.inputs["Color"])
    tint = nt.nodes.new("ShaderNodeMix")
    tint.data_type = "RGBA"
    tint.blend_type = "MULTIPLY"
    tint.inputs["Factor"].default_value = 1.0
    a, bb, o = C._mix_rgba_sockets(tint)
    nt.links.new(hsv.outputs["Color"], a)
    bb.default_value = C.srgb("#C2BEBA")  # neutral basalt, no mineral hint
    tint.name = "Tint"
    nt.links.new(o, b.inputs["Base Color"])
    mr = nt.nodes.new("ShaderNodeMapRange")
    mr.inputs["To Min"].default_value = 0.6
    mr.inputs["To Max"].default_value = 0.95
    nt.links.new(rough.outputs["Color"], mr.inputs["Value"])
    nt.links.new(mr.outputs["Result"], b.inputs["Roughness"])
    nmt = nt.nodes.new("ShaderNodeTexImage")
    nmt.name = "BakedNormal"
    nm = nt.nodes.new("ShaderNodeNormalMap")
    nm.name = "BakedNormalMap"
    nt.links.new(nmt.outputs["Color"], nm.inputs["Color"])
    return m


def set_rock_tint(m, hexcol):
    for n in m.node_tree.nodes:
        if n.name == "Tint":
            _, bb, _ = C._mix_rgba_sockets(n)
            bb.default_value = C.srgb(hexcol)


def bake_normal(high, low, img_name, size=2048):
    img = bpy.data.images.new(img_name, size, size, alpha=False, float_buffer=False)
    img.colorspace_settings.name = "Non-Color"
    mat = bpy.data.materials.new(img_name + "_bake")
    mat.use_nodes = True
    t = mat.node_tree.nodes.new("ShaderNodeTexImage")
    t.image = img
    mat.node_tree.nodes.active = t
    low.data.materials.clear()
    low.data.materials.append(mat)
    bpy.ops.object.select_all(action="DESELECT")
    high.select_set(True)
    low.select_set(True)
    bpy.context.view_layer.objects.active = low
    sc = bpy.context.scene
    sc.render.bake.use_selected_to_active = True
    sc.render.bake.cage_extrusion = 0.03
    sc.render.bake.max_ray_distance = 0.08
    sc.render.bake.margin = 8
    sc.render.bake.normal_space = "TANGENT"
    bpy.ops.object.bake(type="NORMAL")
    path = os.path.join(TEXOUT, img_name + ".png")
    img.filepath_raw = path
    img.file_format = "PNG"
    img.save()
    print("BAKED", path)
    return img


# ----------------------------------------------------------------- crystals

def set_emit(bm, fn):
    """Per-corner colour attribute 'Emit': R=G=B=fn(vertex co)."""
    lay = bm.loops.layers.float_color.get("Emit") or bm.loops.layers.float_color.new("Emit")
    for f in bm.faces:
        for l in f.loops:
            e = max(0.0, min(1.0, fn(l.vert.co)))
            l[lay] = (e, e, e, 1.0)


def _xform(bm, verts, m):
    bmesh.ops.transform(bm, matrix=m, verts=verts)


def orient(z):
    z = z.normalized()
    up = Vector((0, 0, 1)) if abs(z.z) < 0.95 else Vector((1, 0, 0))
    x = up.cross(z).normalized()
    y = z.cross(x)
    return Matrix((x, y, z)).transposed().to_4x4()


def cluster_ferrosite(rng):
    """Pyrite-like interpenetrating cubes sharing one orientation."""
    bm = bmesh.new()
    base = Matrix.Rotation(rng.uniform(0, 1), 4, "Z") @ Matrix.Rotation(0.35, 4, "X")
    for k in range(15):
        s = rng.uniform(0.07, 0.16) * (1.4 if k == 0 else 1.0)
        vs = bmesh.ops.create_cube(bm, size=1.0)["verts"]
        c = Vector((rng.uniform(-0.13, 0.13), rng.uniform(-0.13, 0.13), rng.uniform(0.0, 0.1) + s * 0.4))
        _xform(bm, vs, Matrix.Translation(c) @ base @ Matrix.Scale(s, 4))
    bmesh.ops.bevel(bm, geom=list(bm.edges), offset=0.004, segments=1, affect="EDGES")
    set_emit(bm, lambda co: 0.0)
    return bm


def cluster_glacine(rng):
    """Long hexagonal prisms with pointed tips, fanned out from a common base."""
    bm = bmesh.new()
    for k in range(13):
        L = rng.uniform(0.18, 0.42)
        r = rng.uniform(0.022, 0.05)
        body = bmesh.ops.create_cone(bm, cap_ends=True, segments=6, radius1=r, radius2=r, depth=L)["verts"]
        tip = bmesh.ops.create_cone(bm, cap_ends=True, segments=6, radius1=r, radius2=0.0, depth=r * 2.6)["verts"]
        _xform(bm, tip, Matrix.Translation((0, 0, L / 2 + r * 1.3)))
        tilt = Matrix.Rotation(rng.uniform(-0.75, 0.75), 4, "X") @ Matrix.Rotation(rng.uniform(-0.75, 0.75), 4, "Y")
        m = Matrix.Translation((rng.uniform(-0.05, 0.05), rng.uniform(-0.05, 0.05), 0)) @ tilt @ \
            Matrix.Translation((0, 0, L / 2 - 0.02))
        _xform(bm, body + tip, m)
    set_emit(bm, lambda co: 0.25 * min(1.0, co.length / 0.45))
    return bm


def cluster_cobaltine(rng):
    """Bladed plates in an overlapping fan; glowing rims via Emit."""
    bm = bmesh.new()
    for k in range(10):
        w = rng.uniform(0.07, 0.13)
        h = rng.uniform(0.2, 0.38)
        t = 0.028
        prof = [(-0.5, 0.0), (0.5, 0.0), (0.42, 0.35), (0.22, 0.75), (0.0, 1.0), (-0.22, 0.75), (-0.42, 0.35)]
        front = [bm.verts.new((x * w, 0.0, z * h)) for x, z in prof]
        back = [bm.verts.new((x * w, t, z * h)) for x, z in prof]
        n = len(prof)
        faces = [bm.faces.new(front), bm.faces.new(list(reversed(back)))]
        for i in range(n):
            faces.append(bm.faces.new((front[i], back[i], back[(i + 1) % n], front[(i + 1) % n])))
        m = Matrix.Rotation(rng.uniform(-1.3, 1.3), 4, "Z") @ Matrix.Rotation(rng.uniform(-0.55, 0.55), 4, "X") @ \
            Matrix.Rotation(rng.uniform(-0.35, 0.35), 4, "Y")
        _xform(bm, front + back, Matrix.Translation((rng.uniform(-0.05, 0.05), rng.uniform(-0.05, 0.05), -0.02)) @ m)
        lay = bm.loops.layers.float_color.get("Emit") or bm.loops.layers.float_color.new("Emit")
        for f in faces:
            side = len(f.verts) == 4  # thin rim faces glow, broad faces stay dark-blue
            for l in f.loops:
                e = 1.0 if side else 0.06
                l[lay] = (e, e, e, 1.0)
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
    return bm


def cluster_starfall(rng):
    """Black impact glass: curved glass spikes + a few smooth drops; straight glowing fracture planes."""
    bm = bmesh.new()
    for k in range(8):
        L = rng.uniform(0.16, 0.36)
        r = rng.uniform(0.025, 0.05)
        sb = bmesh.new()
        rings = []
        for j in range(7):  # lofted tapering spike, bent along its length
            tt = j / 6
            rr = r * (1.0 - tt) + 0.002
            rings.append([sb.verts.new((rr * math.cos(2 * math.pi * q / 10), rr * math.sin(2 * math.pi * q / 10),
                                        tt * L)) for q in range(10)])
        for ra, rb in zip(rings, rings[1:]):
            for q in range(10):
                sb.faces.new((ra[q], ra[(q + 1) % 10], rb[(q + 1) % 10], rb[q]))
        sb.faces.new(list(reversed(rings[0])))
        sb.faces.new(rings[-1])
        bend = rng.uniform(-1.2, 1.2)
        tilt = Matrix.Rotation(rng.uniform(-0.8, 0.8), 4, "X") @ Matrix.Rotation(rng.uniform(-0.8, 0.8), 4, "Y")
        base = Vector((rng.uniform(-0.07, 0.07), rng.uniform(-0.07, 0.07), -0.02))
        for v in sb.verts:
            tt = max(0.0, v.co.z / L)
            v.co.x += bend * 0.25 * L * tt * tt
            v.co = (Matrix.Translation(base) @ tilt) @ v.co
        me = bpy.data.meshes.new("spk")
        sb.to_mesh(me)
        sb.free()
        bm.from_mesh(me)
        bpy.data.meshes.remove(me)
    for k in range(3):
        vs = bmesh.ops.create_icosphere(bm, subdivisions=3, radius=1.0)["verts"]
        sc = rng.uniform(0.05, 0.08)
        for v in vs:
            if v.co.z > 0:
                v.co.z *= 1.0 + 1.2 * v.co.z
                v.co.x *= 1.0 - 0.4 * v.co.z
                v.co.y *= 1.0 - 0.4 * v.co.z
        _xform(bm, vs, Matrix.Translation((rng.uniform(-0.09, 0.09), rng.uniform(-0.09, 0.09), sc * 0.9))
               @ Matrix.Scale(sc, 4))
    for v in bm.verts:
        v.tag = False
    planes = [(rand_dir(rng), Vector((rng.uniform(-0.05, 0.05), rng.uniform(-0.05, 0.05), rng.uniform(0.0, 0.15))))
              for _ in range(5)]

    def crack(co):
        root = max(0.0, 1.0 - max(0.0, co.z) / 0.03) ** 2
        e = 0.0
        for n_, p_ in planes[:2]:
            dd = abs((co - p_).dot(n_))
            e = max(e, 1.0 - min(1.0, dd / 0.006))
        return max(root, 0.6 * e)

    set_emit(bm, crack)
    return bm


CLUSTER_SEED = {"ferrosite": 11, "glacine": 23, "cobaltine": 37, "starfall-glass": 53}

CLUSTERS = {"ferrosite": cluster_ferrosite, "glacine": cluster_glacine, "cobaltine": cluster_cobaltine,
            "starfall-glass": cluster_starfall}


def crystal_material(mid):
    base, met, rough, ehex, estr, trans = MINERALS[mid]
    m = bpy.data.materials.new(f"Crystal_{mid}")
    m.use_nodes = True
    nt = m.node_tree
    b = nt.nodes["Principled BSDF"]
    b.inputs["Base Color"].default_value = C.srgb(base)
    b.inputs["Metallic"].default_value = met
    b.inputs["Roughness"].default_value = rough
    b.inputs["Transmission Weight"].default_value = trans
    b.inputs["Coat Weight"].default_value = 0.6 if mid in ("glacine", "starfall-glass") else 0.0
    if mid == "ferrosite":  # brassy speckle on the rust
        tc = nt.nodes.new("ShaderNodeTexCoord")
        nz = nt.nodes.new("ShaderNodeTexNoise")
        nz.inputs["Scale"].default_value = 140
        nz.inputs["Detail"].default_value = 6
        nt.links.new(tc.outputs["Object"], nz.inputs["Vector"])
        r = nt.nodes.new("ShaderNodeValToRGB")
        r.color_ramp.elements[0].position = 0.5
        r.color_ramp.elements[0].color = C.srgb("#8E4A2A")
        r.color_ramp.elements[1].position = 0.7
        r.color_ramp.elements[1].color = C.srgb("#D8B070")
        nt.links.new(nz.outputs["Fac"], r.inputs["Fac"])
        nt.links.new(r.outputs["Color"], b.inputs["Base Color"])
    ca = nt.nodes.new("ShaderNodeVertexColor")
    ca.layer_name = "Emit"
    sep = nt.nodes.new("ShaderNodeSeparateColor")
    nt.links.new(ca.outputs["Color"], sep.inputs[0])
    mul = nt.nodes.new("ShaderNodeMath")
    mul.operation = "MULTIPLY"
    nt.links.new(sep.outputs[0], mul.inputs[0])
    mul.inputs[1].default_value = estr
    b.inputs["Emission Color"].default_value = C.srgb(ehex)
    nt.links.new(mul.outputs[0], b.inputs["Emission Strength"])
    return m


def make_stump(bm, keep_h=0.06):
    """Depleted: everything above keep_h removed, cut faces filled."""
    geom = list(bm.verts) + list(bm.edges) + list(bm.faces)
    r = bmesh.ops.bisect_plane(bm, geom=geom, plane_co=(0, 0, keep_h), plane_no=(0, 0, 1), clear_outer=True)
    edges = [e for e in r["geom_cut"] if isinstance(e, bmesh.types.BMEdge)]
    try:
        bmesh.ops.holes_fill(bm, edges=edges, sides=0)
    except Exception:
        pass
    return bm


# ----------------------------------------------------------------- build

def build_rocks(coll, rmat):
    rocks = []
    for i in range(N_ROCKS):
        hb = sculpt_rock(i, subdiv=7)
        high = C.new_obj(f"rock{i}_high", hb, [rmat], coll)
        for p in high.data.polygons:
            p.use_smooth = True
        # LOD0 = the same sculpt function on a coarser icosphere (5,120 tris; bmesh subdivisions=5). Decimate was not
        # reproducible run to run; re-evaluating the seeded sculpt is.
        low = C.new_obj(f"asteroid_rock_{i}_LOD0", sculpt_rock(i, subdiv=5), [rmat], coll)
        bpy.context.view_layer.objects.active = low
        bpy.ops.object.select_all(action="DESELECT")
        low.select_set(True)
        bpy.ops.object.mode_set(mode="EDIT")
        bpy.ops.mesh.select_all(action="SELECT")
        bpy.ops.uv.smart_project(angle_limit=math.radians(66), island_margin=0.004)
        bpy.ops.object.mode_set(mode="OBJECT")
        rocks.append((high, low))
    return rocks


def main():
    os.makedirs(TEXOUT, exist_ok=True)
    C.reset_scene()
    C.setup_cycles(samples=160, res=(1920, 1080))
    sky = os.path.join(C.TEX, "skybox", "cradle_sky_equirect.png")
    C.world_space("#04060C", 1.0, hdri=sky if os.path.exists(sky) else None)
    C.studio_lights()
    coll = bpy.data.collections.new("asteroids")
    bpy.context.scene.collection.children.link(coll)
    rmat = rock_material()
    rocks = build_rocks(coll, rmat)

    roots = []
    for i, (high, low) in enumerate(rocks):
        img = bake_normal(high, low, f"asteroid_rock_{i}_normal")
        # per-rock copy of the rock material with its baked normal
        m = rmat.copy()
        m.name = f"Rock_Basalt_{i}"
        nt = m.node_tree
        nt.nodes["BakedNormal"].image = img
        nt.links.new(nt.nodes["BakedNormalMap"].outputs["Normal"], nt.nodes["Principled BSDF"].inputs["Normal"])
        low.data.materials.clear()
        low.data.materials.append(m)
        for p in low.data.polygons:
            p.use_smooth = True
        high.hide_render = True
        high.hide_set(True)
        # LOD1
        l1 = C.new_obj(f"asteroid_rock_{i}_LOD1", sculpt_rock(i, subdiv=4), [rmat], coll)  # 1,280 tris, no baked normal
        for p in l1.data.polygons:
            p.use_smooth = True
        l1.hide_render = True
        root = bpy.data.objects.new(f"asteroid_rock_{i}", None)
        coll.objects.link(root)
        low.parent = root
        l1.parent = root
        # satellites: smaller copies of the other rocks' LOD0 meshes
        rng = random.Random(SEED + 1000 + i)
        sats = []
        for k in range(rng.randint(3, 4)):
            src = rocks[(i + k + 1) % N_ROCKS][1]
            s = bpy.data.objects.new(f"asteroid_rock_{i}_sat{k}", src.data)
            coll.objects.link(s)
            sc = rng.uniform(0.07, 0.16)
            dirv = rand_dir(rng)
            dirv.z *= 0.5
            dirv.normalize()
            s.scale = (sc, sc, sc)
            s.rotation_euler = (rng.uniform(0, 6), rng.uniform(0, 6), rng.uniform(0, 6))
            s.location = dirv * (OUTER_R - sc * MAIN_R - 0.02)
            s.parent = root
            sats.append(s)
        # crystal sockets: ray cast onto the low mesh from random outward directions
        bm = bmesh.new()
        bm.from_mesh(low.data)
        tree = BVHTree.FromBMesh(bm)
        bm.free()
        for k in range(6):
            zz = 1 - (k + 0.5) / 6 * 2
            ph = k * 2.39996 + rng.uniform(-0.3, 0.3)
            rr = math.sqrt(max(0.0, 1 - zz * zz))
            dv = Vector((rr * math.cos(ph), rr * math.sin(ph), zz)).normalized()
            loc, nor, _, _ = tree.ray_cast(dv * 2.0, -dv, 4.0)
            e = C.add_empty(f"Socket_Crystal_{k}", loc, size=0.08)
            e.matrix_world = Matrix.Translation(loc) @ orient(nor)
            e.parent = root
        r = C.bounding_radius([low] + sats)
        assert r <= OUTER_R + 1e-3, f"rock {i}: bounding radius {r:.3f} > 1.0"
        print("ROCKREPORT", i, "r=%.3f" % r, "tris", C.tri_count(low), C.tri_count(l1))
        roots.append(root)
        C.export_fbx(os.path.join(C.EXPORT, f"asteroid_rock_{i}.fbx"), [root])

    # crystals
    ccoll = bpy.data.collections.new("crystals")
    bpy.context.scene.collection.children.link(ccoll)
    cobs = {}
    for mid, fn in CLUSTERS.items():
        mat = crystal_material(mid)
        full = C.new_obj(f"crystal_{mid}", fn(random.Random(SEED + CLUSTER_SEED[mid])), [mat], ccoll)
        for p in full.data.polygons:
            p.use_smooth = mid in ("starfall-glass",)
        stump_bm = bmesh.new()
        stump_bm.from_mesh(full.data)
        make_stump(stump_bm)
        stump = C.new_obj(f"crystal_{mid}_stump", stump_bm, [mat], ccoll)
        for p in stump.data.polygons:
            p.use_smooth = mid in ("starfall-glass",)
        C.export_fbx(os.path.join(C.EXPORT, f"crystal_{mid}.fbx"), [full, stump])
        cobs[mid] = (full, stump, mat)
        print("CRYSTALREPORT", mid, "tris", C.tri_count(full), C.tri_count(stump))
    C.save_blend(os.path.join(C.SOURCE, "asteroids.blend"))
    previews(roots, rocks, cobs, rmat)


def attach(root, cluster, scale=1.0, n=4, view=None):
    """Instance a crystal cluster on the root's sockets (preview only; Unity does this at runtime).
    view: a point; the n sockets facing it are used so the preview shows them."""
    out = []
    socks = sorted([c for c in root.children if c.name.startswith("Socket_Crystal")], key=lambda c: c.name)
    if view is not None:
        bpy.context.view_layer.update()

        def facing(sk):
            m = sk.matrix_world
            nrm = (m.to_3x3() @ Vector((0, 0, 1))).normalized()
            return -nrm.dot((Vector(view) - m.translation).normalized())
        socks = sorted(socks, key=facing)
    socks = socks[:n]
    for k, s in enumerate(socks):
        o = bpy.data.objects.new(f"{cluster.name}_on_{root.name}_{k}", cluster.data)
        bpy.context.scene.collection.objects.link(o)
        o.matrix_world = (s.matrix_world @ Matrix.Translation((0, 0, -0.03)) @ Matrix.Rotation(k * 1.3, 4, "Z")
                          @ Matrix.Scale(scale * (1.7 - 0.2 * k), 4))
        out.append(o)
    return out


def previews(roots, rocks, cobs, rmat):
    sc = bpy.context.scene
    for _, (full, stump, _) in cobs.items():
        full.hide_render = True
        stump.hide_render = True
    cam = C.add_camera((0, -9, 2), (0, 0, 0), lens=50)
    # 2 x 2 grid in the plane facing the lit-side camera
    C.cam_lit(cam, (0, 0, 0), 9.0, yaw_deg=50, pitch_deg=5, lens=42)
    right = (cam.matrix_world.to_3x3() @ Vector((1, 0, 0))).normalized()
    up = (cam.matrix_world.to_3x3() @ Vector((0, 1, 0))).normalized()
    grid = [(-1, 1), (1, 1), (-1, -1), (1, -1)]
    for r, (gx, gy) in zip(roots, grid):
        r.location = right * gx * 1.15 + up * gy * 1.0
    C.render(os.path.join(C.PREVIEWS, "asteroid_unrevealed_variants.png"))
    tints = {"ferrosite": "#D0B4A2", "glacine": "#B8C6D0", "cobaltine": "#A8B0C8", "starfall-glass": "#8E8890"}
    made = []
    for (mid, (full, stump, mat)), r in zip(cobs.items(), roots):
        made += attach(r, full, view=cam.location)
        for ch in r.children:
            if ch.name.endswith("_LOD0"):
                set_rock_tint(ch.data.materials[0], tints[mid])
    C.render(os.path.join(C.PREVIEWS, "asteroid_revealed_minerals.png"))
    looks = [i.identifier for i in sc.view_settings.bl_rna.properties["look"].enum_items]
    if "AgX - Greyscale" in looks:
        sc.view_settings.look = "AgX - Greyscale"
        C.render(os.path.join(C.PREVIEWS, "asteroid_revealed_minerals_grey.png"))
        sc.view_settings.look = "AgX - Medium High Contrast"
    for (mid, (full, stump, mat)), r in zip(cobs.items(), roots):
        others = [o for o in roots if o is not r]
        for o in others:
            o.hide_render = True
            for ch in o.children_recursive:
                ch.hide_render = True
        C.cam_lit(cam, r.location, 4.2, yaw_deg=40, pitch_deg=10, lens=50)
        C.render(os.path.join(C.TEX, "_tmp", f"mineral_{mid}.png"), samples=128)
        for o in others:
            o.hide_render = False
            for ch in o.children_recursive:
                ch.hide_render = ch.name.endswith("_LOD1") or ch.name.endswith("_high")
    for o in made:
        o.hide_render = True
    for r in roots:
        for ch in r.children:
            if ch.name.endswith("_LOD0"):
                set_rock_tint(ch.data.materials[0], "#C2BEBA")
    # depletion: cobaltine at 100 % / 55 % / 0 % (stump)
    full, stump, _ = cobs["cobaltine"]
    C.cam_lit(cam, (0, 0, 0), 8.0, yaw_deg=30, pitch_deg=10, lens=45)
    right = (cam.matrix_world.to_3x3() @ Vector((1, 0, 0))).normalized()
    for r, gx in zip(roots[:3], (-2.2, 0, 2.2)):
        r.location = right * gx
    roots[3].location = (0, 0, -60)
    attach(roots[0], full, 1.0, view=cam.location)
    attach(roots[1], full, 0.55, view=cam.location)
    attach(roots[2], stump, 1.0, view=cam.location)
    C.render(os.path.join(C.PREVIEWS, "asteroid_depletion_cobaltine.png"))


if __name__ == "__main__":
    main()

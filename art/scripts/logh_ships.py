"""v3 art direction (2026-10-09): LoGH-inspired ORIGINAL ship designs.

Design language only (proportion, long prow, smooth armour, symmetry, colour systems, ornament
principles) - no shape, crest or logo of any existing ship is reproduced. See 01_art_direction.md 11.

Ships (nose = Blender -Y, up = +Z, 1 unit = 1 m):
  Player 1-person craft (bounding radius <= 12 m = data/ships/scout-s01.json hull_radius_m, asserted)
    lancet_imperial   - "Imperial" family: white lacquer + silver, gold trim, long spindle prow, swept fins
    cutter_alliance   - "Alliance" family: green-grey angular blocks, practical, stencil numbers
  Background battleships (scene dressing only, not gameplay objects)
    bs_imperial       - ~620 m spindle with lateral armour wings, gold bands
    bs_alliance       - ~560 m long slab with stepped superstructure

Run:  blender --background --factory-startup --python art/scripts/logh_ships.py
Out:  art/export/v3/ship_{lancet_imperial,cutter_alliance}.fbx (LOD0/1 + Socket_*),
      art/export/v3/bg_{bs_imperial,bs_alliance}.fbx, art/source/logh_ships.blend,
      art/previews/v3_*.png (when LOGH_PREVIEW != "0")
Smooth hulls: a lofted superellipse cage + Catmull-Clark (applied), so the armour reads as large
continuous plates; trims are separate lofted bands that follow the same section function.
"""
import math
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import bpy
import bmesh
from mathutils import Vector, Matrix

import common as C

HULL_RADIUS_M = 12.0
EXPORT = os.path.join(C.EXPORT, "v3")

MATS = ["white", "silver", "gold", "glass", "window", "engine_b", "engine_w", "green", "green_dark",
        "marking", "dark", "nav_r", "nav_g", "laser", "warn", "green_light"]
KITS = ("standard", "prospector", "courier")  # v2 module kits carried over to the v3 hulls


def I(name):
    return MATS.index(name)


def materials():
    M = {}
    M["white"] = C.mat_painted("Imp_White", "#ECE8DF", metallic=0.25, rough=(0.18, 0.32),
                               tex_set="blue_metal_plate", uv_scale=0.08, normal_strength=0.12, grime=0.06)
    M["white"].node_tree.nodes["Principled BSDF"].inputs["Coat Weight"].default_value = 0.6
    M["silver"] = C.mat_simple("Imp_Silver", "#B9BEC6", metallic=0.95, rough=0.22)
    M["gold"] = C.mat_simple("Imp_Gold", "#D9A84A", metallic=1.0, rough=0.2)
    M["glass"] = C.mat_simple("Canopy_Glass", "#03070C", metallic=0.6, rough=0.03, coat=1.0,
                              emission_hex="#3A8FB8", emission=0.08)
    M["window"] = C.mat_emit("Window_Warm", "#FFD9A0", 8.0)
    M["engine_b"] = C.mat_emit("Engine_Imperial", "#5FB2FF", 3.5)
    M["engine_w"] = C.mat_emit("Engine_Alliance", "#FFC27A", 7.0)
    M["green"] = C.mat_painted("All_Green", "#6E7D68", metallic=0.2, rough=(0.38, 0.62),
                               tex_set="metal_plate_02", uv_scale=0.12, normal_strength=0.35, grime=0.2)
    M["green_dark"] = C.mat_painted("All_GreenDark", "#3D463B", metallic=0.35, rough=(0.4, 0.7),
                                    tex_set="metal_plate_02", uv_scale=0.15, normal_strength=0.45, grime=0.15)
    M["marking"] = C.mat_simple("All_Marking", "#E2DFCC", metallic=0.0, rough=0.5)
    M["dark"] = C.mat_simple("Dark_Metal", "#24272C", metallic=0.7, rough=0.45)
    M["nav_r"] = C.mat_emit("Nav_Red", "#FF3B30", 25.0)
    M["nav_g"] = C.mat_emit("Nav_Green", "#30FF6A", 25.0)
    M["laser"] = C.mat_emit("Mining_Lens", "#5FF0FF", 12.0)
    M["warn"] = C.mat_simple("Warning_Amber", "#D99A2B", metallic=0.1, rough=0.45)
    M["green_light"] = C.mat_painted("All_GreenLight", "#8A9883", metallic=0.2, rough=(0.4, 0.62),
                                     tex_set="metal_plate_02", uv_scale=0.12, normal_strength=0.35, grime=0.2)
    return M


# ----------------------------------------------------------------- geometry helpers

def smoothstep(a, b, x):
    t = max(0.0, min(1.0, (x - a) / (b - a)))
    return t * t * (3 - 2 * t)


def interp(keys, t):
    """keys: [(t, hw, hh, z0), ...] - smooth interpolation between keyframes."""
    for k0, k1 in zip(keys, keys[1:]):
        if k0[0] <= t <= k1[0]:
            u = smoothstep(k0[0], k1[0], t)
            return tuple(k0[i] + (k1[i] - k0[i]) * u for i in range(1, 4))
    return keys[-1][1:]


def supersection(hw, hh, z0, y, n, exp_x=2.5, exp_top=None, x0=0.0):
    """Superellipse ring in the XZ plane at Y=y. exp_top lets the top be flatter/pointier than the bottom."""
    pts = []
    for i in range(n):
        a = 2 * math.pi * i / n
        c, s = math.cos(a), math.sin(a)
        e = exp_x if (exp_top is None or s < 0) else exp_top
        x = hw * math.copysign(abs(c) ** (2 / e), c)
        z = hh * math.copysign(abs(s) ** (2 / e), s)
        pts.append(Vector((x0 + x, y, z0 + z)))
    return pts


def loft(rings, mat, cap=True):
    bm = bmesh.new()
    vr = [[bm.verts.new(p) for p in r] for r in rings]
    n = len(rings[0])
    for a, b in zip(vr, vr[1:]):
        for j in range(n):
            f = bm.faces.new((a[j], a[(j + 1) % n], b[(j + 1) % n], b[j]))
            f.material_index = I(mat)
    if cap:
        for r, rev in ((vr[0], True), (vr[-1], False)):
            f = bm.faces.new(list(reversed(r)) if rev else r)
            f.material_index = I(mat)
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
    return bm


def hull(keys, L, n=24, samples=36, exp_x=2.5, exp_top=None, mat="white", y0=None):
    y0 = -L / 2 if y0 is None else y0
    rings = []
    for i in range(samples + 1):
        t = i / samples
        hw, hh, z0 = interp(keys, t)
        rings.append(supersection(max(hw, 0.02), max(hh, 0.02), z0, y0 + t * L, n, exp_x, exp_top))
    return loft(rings, mat)


def band(keys, L, t, width_t, mat="gold", grow=1.012, n=24, exp_x=2.5, exp_top=None, y0=None):
    """A thin trim band hugging the hull at parameter t."""
    y0 = -L / 2 if y0 is None else y0
    rings = []
    for tt in (t - width_t / 2, t + width_t / 2):
        hw, hh, z0 = interp(keys, tt)
        rings.append(supersection(hw * grow, hh * grow, z0, y0 + tt * L, n, exp_x, exp_top))
    return loft(rings, mat, cap=False)


def merge(dst, src):
    me = bpy.data.meshes.new("tmp")
    src.to_mesh(me)
    src.free()
    dst.from_mesh(me)
    bpy.data.meshes.remove(me)


def box(bm, c, s, mat, rot=None):
    vs = bmesh.ops.create_cube(bm, size=1.0)["verts"]
    bmesh.ops.scale(bm, vec=s, verts=vs)
    if rot is not None:
        bmesh.ops.rotate(bm, verts=vs, cent=(0, 0, 0), matrix=rot)
    bmesh.ops.translate(bm, vec=c, verts=vs)
    for f in {f for v in vs for f in v.link_faces}:
        f.material_index = I(mat)
    return vs


def cyl(bm, c, r, depth, mat, axis="Y", segs=16, r2=None):
    vs = bmesh.ops.create_cone(bm, cap_ends=True, segments=segs, radius1=r, radius2=r if r2 is None else r2,
                               depth=depth)["verts"]
    rot = {"Y": Matrix.Rotation(math.radians(-90), 3, "X"), "X": Matrix.Rotation(math.radians(90), 3, "Y"),
           "Z": Matrix.Identity(3)}[axis]
    bmesh.ops.rotate(bm, verts=vs, cent=(0, 0, 0), matrix=rot)
    bmesh.ops.translate(bm, vec=c, verts=vs)
    for f in {f for v in vs for f in v.link_faces}:
        f.material_index = I(mat)
    return vs


def wing(root_y0, root_y1, tip_y0, tip_y1, x_root, x_tip, z, thick, side, mat="white", dz_tip=0.0, n=10):
    """Swept airfoil-ish plate from x_root to x_tip (mirrored by side)."""
    def sec(x, y0, y1, th, zz):
        pts = []
        for i in range(n):
            a = 2 * math.pi * i / n
            yc = (y0 + y1) / 2 + (y1 - y0) / 2 * math.cos(a)
            sa = math.sin(a)
            # lens airfoil: thickest a third back from the leading edge (-Y), knife-sharp trailing edge
            zc = zz + th / 2 * math.copysign(abs(sa) ** 1.5, sa) * (0.75 - 0.35 * math.cos(a))
            pts.append(Vector((side * x, yc, zc)))
        return pts if side > 0 else list(reversed(pts))
    rings = [sec(x_root, root_y0, root_y1, thick, z),
             sec((x_root + x_tip) / 2, (root_y0 + tip_y0) / 2, (root_y1 + tip_y1) / 2, thick * 0.7, z + dz_tip / 2),
             sec(x_tip, tip_y0, tip_y1, thick * 0.35, z + dz_tip)]
    return loft(rings, mat)


def plate(keys, L, t0, t1, a0, a1, grow, mat, exp_x=2.3, exp_top=2.0, n=10, samples=8, thick=0.06, y0=None):
    """Raised armour plate: a patch of the hull surface between angles a0..a1 (radians, 0 = +X, pi/2 = top),
    pushed out by `grow`, closed into a thin slab so the gap to the hull reads as a seam."""
    y0 = -L / 2 if y0 is None else y0
    outer, inner = [], []
    for i in range(samples + 1):
        t = t0 + (t1 - t0) * i / samples
        hw, hh, z0 = interp(keys, t)
        ro, ri = [], []
        for k in range(n + 1):
            a = a0 + (a1 - a0) * k / n
            c, s_ = math.cos(a), math.sin(a)
            e = exp_x if s_ < 0 else exp_top
            x = math.copysign(abs(c) ** (2 / e), c)
            z = math.copysign(abs(s_) ** (2 / e), s_)
            ro.append(Vector((hw * grow * x, y0 + t * L, z0 + hh * grow * z)))
            ri.append(Vector((hw * (grow - thick) * x, y0 + t * L, z0 + hh * (grow - thick) * z)))
        outer.append(ro)
        inner.append(ri)
    bm = bmesh.new()
    vo = [[bm.verts.new(p) for p in r] for r in outer]
    vi = [[bm.verts.new(p) for p in r] for r in inner]
    m = I(mat)

    def q(a, b, c, d):
        f = bm.faces.new((a, b, c, d))
        f.material_index = m
    for i in range(samples):
        for k in range(n):
            q(vo[i][k], vo[i][k + 1], vo[i + 1][k + 1], vo[i + 1][k])
            q(vi[i][k], vi[i + 1][k], vi[i + 1][k + 1], vi[i][k + 1])
    for i in range(samples):
        q(vo[i][0], vo[i + 1][0], vi[i + 1][0], vi[i][0])
        q(vo[i][n], vi[i][n], vi[i + 1][n], vo[i + 1][n])
    for k in range(n):
        q(vo[0][k], vi[0][k], vi[0][k + 1], vo[0][k + 1])
        q(vo[samples][k], vo[samples][k + 1], vi[samples][k + 1], vi[samples][k])
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
    return bm


def make_object(name, bm, M, coll, subsurf=2, smooth=True):
    ob = C.new_obj(name, bm, [M[k] for k in MATS], coll)
    if subsurf:
        m = ob.modifiers.new("Sub", "SUBSURF")
        m.levels = subsurf
        m.render_levels = subsurf
    C.apply_modifiers(ob)
    for p in ob.data.polygons:
        p.use_smooth = smooth
    return ob


def join(objs, name):
    return C.join(objs, name)


# ----------------------------------------------------------------- player craft (<= 12 m radius)

LANCET_L = 21.0
LANCET_KEYS = [(0.0, 0.04, 0.04, -0.1), (0.1, 0.42, 0.3, -0.08), (0.27, 1.1, 0.8, 0.0),
               (0.45, 1.6, 1.15, 0.1), (0.6, 1.75, 1.25, 0.1), (0.76, 2.15, 1.35, 0.05),
               (0.86, 2.0, 1.2, 0.0), (0.95, 1.4, 0.95, 0.0), (1.0, 1.25, 0.85, 0.0)]


def ellipsoid(bm, c, r, mat, u=20, v=10):
    vs = bmesh.ops.create_uvsphere(bm, u_segments=u, v_segments=v, radius=1.0)["verts"]
    bmesh.ops.scale(bm, vec=r, verts=vs)
    bmesh.ops.translate(bm, vec=c, verts=vs)
    for f in {f for v_ in vs for f in v_.link_faces}:
        f.material_index = I(mat)
    return vs


def lancet_kit(g, kit):
    """Imperial-style module kits: elegant, gold-banded, silver housings."""
    socks = {}
    if kit == "prospector":
        # ventral mining turret: silver drum, gold ring, long barrel with a cyan lens
        cyl(g, (0, 0.6, -1.32), 0.62, 0.32, "silver", axis="Z", segs=24)
        cyl(g, (0, 0.6, -1.5), 0.66, 0.06, "gold", axis="Z", segs=24)
        ellipsoid(g, (0, 0.2, -1.62), (0.42, 0.9, 0.34), "white")
        cyl(g, (0, -1.0, -1.62), 0.16, 2.2, "silver", axis="Y", segs=12)
        cyl(g, (0, -2.15, -1.62), 0.26, 0.2, "gold", axis="Y", segs=16)
        cyl(g, (0, -2.27, -1.62), 0.18, 0.04, "laser", axis="Y", segs=16)
        # slim ore pods under the fins
        for sx in (1, -1):
            ellipsoid(g, (sx * 2.7, 4.6, -1.05), (0.55, 2.7, 0.5), "silver")
            for y in (3.2, 6.0):
                cyl(g, (sx * 2.7, y, -1.05), 0.57, 0.12, "gold", axis="Y", segs=20)
            box(g, (sx * 2.0, 4.6, -0.75), (0.9, 2.2, 0.2), "silver")
        socks["Socket_Mining_0"] = (0, -2.3, -1.62)
        socks["Socket_Cargo_0"] = (2.7, 4.6, -1.05)
        socks["Socket_Cargo_1"] = (-2.7, 4.6, -1.05)
    elif kit == "courier":
        # dorsal cargo capsule with gold bands and a silver saddle
        ellipsoid(g, (0, 3.9, 1.75), (0.95, 3.1, 0.72), "white", u=24, v=12)
        for y in (2.2, 3.9, 5.6):
            cyl(g, (0, y, 1.75), 0.9, 0.1, "gold", axis="Y", segs=24)
        box(g, (0, 3.9, 1.15), (0.6, 4.6, 0.4), "silver")
        cyl(g, (0, 8.1, 1.9), 0.035, 1.6, "silver", axis="Z", segs=6)
        socks["Socket_Cargo_0"] = (0, 3.9, 1.75)
    return socks


def build_lancet(M, coll, livery="white", kit="standard"):
    """Imperial-family 1-person craft: long needle prow, spindle body, two swept fins, ventral fin."""
    L = LANCET_L
    main = hull(LANCET_KEYS, L, n=20, samples=26, exp_x=2.3, exp_top=2.0, mat=livery)
    for t, w, g, m in ((0.33, 0.016, 1.03, "gold"), (0.58, 0.012, 1.03, "gold"), (0.865, 0.05, 1.05, "silver"),
                       (0.9, 0.012, 1.08, "gold")):
        merge(main, band(LANCET_KEYS, L, t, w, m, grow=g, n=20, exp_x=2.3, exp_top=2.0))
    # layered armour (own mesh, one subdivision level): dorsal saddles and flank plates over silver seams
    plates = bmesh.new()
    for t0, t1 in ((0.2, 0.33), (0.36, 0.56), (0.6, 0.84)):
        merge(plates, plate(LANCET_KEYS, L, t0, t1, math.radians(55), math.radians(125), 1.05, "white"))
        for s in (1, -1):
            a0, a1 = (math.radians(-20), math.radians(40)) if s > 0 else (math.radians(140), math.radians(200))
            merge(plates, plate(LANCET_KEYS, L, t0 + 0.02, t1 - 0.02, a0, a1, 1.045, "white"))
        merge(plates, band(LANCET_KEYS, L, t1 + 0.008, 0.006, "gold", grow=1.065, n=20, exp_x=2.3, exp_top=2.0))
    # dorsal gold spine line
    spine_keys = [(0.12, 0.08, 0.05, 0.0), (0.9, 0.08, 0.05, 0.0)]
    rings = []
    for i in range(13):
        t = 0.12 + 0.78 * i / 12
        hw, hh, z0 = interp(LANCET_KEYS, t)
        rings.append(supersection(0.09, 0.06, z0 + hh * 1.0, -L / 2 + t * L, 8, 2.0))
    merge(main, loft(rings, "gold"))
    # swept fins (aft), slightly anhedral, silver
    wings = bmesh.new()
    for s in (1, -1):
        # thick swept fin with a raised armour panel, gold leading edge and a wingtip pod
        merge(main, wing(2.0, 8.6, 8.2, 9.9, 1.8, 6.2, -0.1, 0.9, s, mat="white", dz_tip=-0.9))
        merge(wings, wing(3.6, 8.3, 7.9, 9.6, 2.6, 5.4, -0.08, 1.02, s, mat="white", dz_tip=-0.75))
        merge(wings, wing(1.95, 2.9, 8.15, 8.6, 1.9, 6.15, -0.1, 0.96, s, mat="gold", dz_tip=-0.9))
        merge(wings, wing(8.2, 8.75, 9.65, 9.95, 1.9, 6.15, -0.1, 0.5, s, mat="silver", dz_tip=-0.9))
    # ventral keel fin
    rings = []
    for z, y0, y1 in ((-1.0, 3.0, 8.6), (-1.9, 6.0, 9.0), (-2.4, 8.2, 9.3)):
        rings.append([Vector((0.18 * math.cos(2 * math.pi * i / 8), (y0 + y1) / 2 + (y1 - y0) / 2 * math.sin(2 * math.pi * i / 8), z))
                      for i in range(8)])
    merge(main, loft(rings, "silver"))
    body = make_object("lancet_body", main, M, coll, subsurf=2)
    pl = make_object("lancet_plates", plates, M, coll, subsurf=1)
    wg = make_object("lancet_wings", wings, M, coll, subsurf=1)
    # hard parts (no subsurf): canopy, engine, lights
    g = bmesh.new()
    vs = bmesh.ops.create_uvsphere(g, u_segments=24, v_segments=12, radius=1.0)["verts"]
    bmesh.ops.scale(g, vec=(0.8, 2.5, 0.42), verts=vs)
    bmesh.ops.translate(g, vec=(0, -2.6, 1.0), verts=vs)
    for f in {f for v in vs for f in v.link_faces}:
        f.material_index = I("glass")
    # canopy gold frame ring
    frame_rings = []
    for k, (sx, sy, dz) in enumerate(((0.86, 2.22, 0.98), (0.98, 2.36, 0.92), (0.98, 2.36, 0.82), (0.86, 2.22, 0.88))):
        frame_rings.append([Vector((sx * math.cos(2 * math.pi * i / 40), -2.6 + sy * math.sin(2 * math.pi * i / 40), dz))
                            for i in range(40)])
    fr = loft(frame_rings + [frame_rings[0]], "gold", cap=False)
    merge(g, fr)
    # engine: silver bell + glow
    # main engine stands proud of the hull's aft cap: gold bell, dark throat, bright core
    cyl(g, (0, 10.75, 0.0), 0.95, 0.7, "gold", axis="Y", segs=32, r2=1.02)
    cyl(g, (0, 10.8, 0.0), 0.86, 0.66, "dark", axis="Y", segs=32)
    cyl(g, (0, 11.12, 0.0), 0.8, 0.04, "engine_b", axis="Y", segs=32)
    for s in (1, -1):
        cyl(g, (s * 1.25, 9.9, -0.35), 0.32, 0.6, "silver", axis="Y", segs=12)
        cyl(g, (s * 1.25, 10.18, -0.35), 0.26, 0.04, "engine_b", axis="Y", segs=12)
        tip = bmesh.ops.create_uvsphere(g, u_segments=12, v_segments=8, radius=1.0)["verts"]
        bmesh.ops.scale(g, vec=(0.22, 0.9, 0.22), verts=tip)
        bmesh.ops.translate(g, vec=(s * 6.2, 9.0, -1.0), verts=tip)
        for f in {f for v in tip for f in v.link_faces}:
            f.material_index = I("silver")
        box(g, (s * 6.2, 9.85, -1.0), (0.14, 0.2, 0.14), "nav_r" if s > 0 else "nav_g")
    # small ventral mining emitter (gameplay: Socket_Mining_0)
    cyl(g, (0, -1.0, -1.25), 0.35, 0.4, "silver", axis="Z", segs=16)
    cyl(g, (0, -1.0, -1.48), 0.2, 0.06, "engine_b", axis="Z", segs=16)
    kit_socks = lancet_kit(g, kit)
    hard = make_object("lancet_hard", g, M, coll, subsurf=0)
    for p in hard.data.polygons:
        p.use_smooth = True
    suffix = "" if kit == "standard" else f"_{kit}"
    ob = join([body, pl, wg, hard], f"ship_lancet_imperial{suffix}_LOD0")
    sockets = {"Socket_Engine_0": (0, 11.15, 0), "Socket_Engine_1": (1.25, 10.2, -0.35),
               "Socket_Engine_2": (-1.25, 10.2, -0.35), "Socket_Mining_0": (0, -1.0, -1.5),
               "Socket_Cockpit": (0, -2.6, 1.1), "Socket_Nav_L": (6.2, 9.85, -1.0), "Socket_Nav_R": (-6.2, 9.85, -1.0)}
    sockets.update(kit_socks)
    return ob, sockets


CUTTER_L = 20.0
CUTTER_KEYS = [(0.0, 0.5, 0.35, -0.2), (0.1, 1.4, 0.8, -0.1), (0.28, 2.2, 1.2, 0.0), (0.5, 2.4, 1.3, 0.05),
               (0.85, 2.4, 1.3, 0.05), (1.0, 2.2, 1.2, 0.05)]


SEG7 = {"0": "abcdef", "1": "bc", "2": "abged", "3": "abgcd", "4": "fgbc", "5": "afgcd", "6": "afgedc",
        "7": "abc", "8": "abcdefg", "9": "abcfgd"}


def stencil(bm, text, origin, right, up, height, mat):
    """Seven-segment stencil numerals made of boxes on a plane (hull numbers; no insignia)."""
    origin, right, up = Vector(origin), Vector(right).normalized(), Vector(up).normalized()
    n = right.cross(up)
    w, t = height * 0.55, height * 0.13
    x = 0.0
    m3 = Matrix((right, up, n)).transposed()
    for ch in text:
        seg = {"a": ((w / 2, height), (w, t)), "g": ((w / 2, height / 2), (w, t)), "d": ((w / 2, 0), (w, t)),
               "f": ((0, height * 0.75), (t, height / 2)), "b": ((w, height * 0.75), (t, height / 2)),
               "e": ((0, height * 0.25), (t, height / 2)), "c": ((w, height * 0.25), (t, height / 2))}
        for sname in SEG7[ch]:
            (px, py), (sw, sh) = seg[sname]
            vs = bmesh.ops.create_cube(bm, size=1.0)["verts"]
            bmesh.ops.scale(bm, vec=(sw, sh, 0.03), verts=vs)
            m = Matrix.Translation(origin + right * (x + px) + up * (py - height / 2)) @ m3.to_4x4()
            bmesh.ops.transform(bm, matrix=m, verts=vs)
            for f in {f for v_ in vs for f in v_.link_faces}:
                f.material_index = I(mat)
        x += w + t * 2.5


def cutter_detail(g):
    """Alliance practicality as density: seams, belts, vents, RCS, antenna, stencils, hazard stripes."""
    for sgn in (1, -1):
        x = sgn * 2.43
        for y in (-5.0, -1.5, 2.0, 5.5):                      # vertical panel seams
            box(g, (x, y, 0.15), (0.04, 0.06, 2.2), "dark")
        box(g, (x, 1.0, 0.55), (0.04, 12.0, 0.05), "dark")     # longitudinal seam
        box(g, (sgn * 2.47, 2.0, -0.55), (0.1, 9.0, 0.7), "green_light")   # armour belt
        for y in (-2.5, 0.5, 3.5, 6.5):
            box(g, (sgn * 2.52, y, -0.55), (0.05, 0.12, 0.72), "dark")
        stencil(g, "07", (x + sgn * 0.03, -4.3, 0.35), (0, sgn, 0), (0, 0, 1), 0.7, "marking")  # reads left-to-right from outside
        for i in range(5):                                    # side vents
            box(g, (x + sgn * 0.02, 6.5 + i * 0.35, 0.8), (0.05, 0.18, 0.5), "dark")
        box(g, (sgn * 2.3, -6.2, 0.7), (0.5, 0.6, 0.4), "green_dark")       # RCS blocks
        box(g, (sgn * 2.3, 8.0, 0.7), (0.5, 0.6, 0.4), "green_dark")
        box(g, (sgn * 3.4, 3.0, 0.62), (0.8, 0.3, 0.06), "warn")           # hazard stripes on pods
        box(g, (sgn * 3.4, 3.5, 0.62), (0.8, 0.3, 0.06), "dark")
    for y in (-6.5, -4.0, 4.5, 7.5):                           # dorsal seams
        box(g, (0, y, 1.37), (4.2, 0.06, 0.04), "dark")
    box(g, (0, 1.0, 1.36), (0.06, 15.0, 0.04), "dark")
    for i in range(6):                                         # dorsal grille
        box(g, (0, 5.6 + i * 0.32, 1.42), (1.6, 0.14, 0.08), "dark")
    cyl(g, (0.9, 6.8, 1.6), 0.03, 1.4, "dark", axis="Z", segs=6)            # antenna
    cyl(g, (-0.9, 1.0, 1.45), 0.35, 0.12, "green_light", axis="Z", segs=16, r2=0.2)   # sensor dish
    box(g, (0, -8.2, 0.2), (1.6, 0.6, 0.2), "warn")                         # nose hazard band


def cutter_kit(g, kit):
    socks = {}
    if kit == "prospector":
        box(g, (0, 0.3, -1.7), (1.7, 3.2, 0.65), "green_dark")
        box(g, (0, 0.3, -2.06), (1.4, 2.8, 0.08), "warn")
        box(g, (0, -1.8, -1.7), (0.6, 1.2, 0.45), "dark")
        cyl(g, (0, -2.9, -1.7), 0.18, 1.2, "dark", axis="Y", segs=10)
        cyl(g, (0, -3.55, -1.7), 0.15, 0.04, "laser", axis="Y", segs=12)
        for sgn in (1, -1):
            box(g, (sgn * 3.4, 5.2, -1.35), (1.3, 4.2, 0.8), "green_light")
            box(g, (sgn * 3.4, 5.2, -1.78), (1.0, 3.8, 0.06), "warn")
            for y in (3.6, 5.2, 6.8):
                box(g, (sgn * 3.4, y, -1.35), (1.36, 0.1, 0.84), "dark")
        socks["Socket_Mining_0"] = (0, -3.6, -1.7)
        socks["Socket_Cargo_0"] = (3.4, 5.2, -1.35)
        socks["Socket_Cargo_1"] = (-3.4, 5.2, -1.35)
    elif kit == "courier":
        for y in (1.6, 4.8):
            box(g, (0, y, 2.0), (2.4, 2.8, 1.25), "green_light")
            for k_ in (-1, 0, 1):
                box(g, (0, y + k_ * 0.9, 2.0), (2.46, 0.08, 1.3), "dark")
            box(g, (0, y, 2.64), (2.0, 2.4, 0.04), "marking")
        socks["Socket_Cargo_0"] = (0, 1.6, 2.0)
        socks["Socket_Cargo_1"] = (0, 4.8, 2.0)
    return socks


def build_cutter(M, coll, livery="green", kit="standard"):
    """Alliance-family 1-person craft: angular block hull, side engine pods, flat armoured canopy."""
    L = CUTTER_L
    main = hull(CUTTER_KEYS, L, n=16, samples=12, exp_x=7.0, exp_top=7.0, mat=livery)
    # stepped dorsal block (bridge) and ventral sensor block
    rings = [supersection(1.3, 0.6, 1.4, y, 16, 7.0) for y in (-3.5, -2.8, 3.0, 5.0)]
    rings[0] = supersection(0.9, 0.3, 1.25, -3.5, 16, 7.0)
    merge(main, loft(rings, "green_dark"))
    for s in (1, -1):
        prs = [supersection(0.95, 0.85, -0.3, y, 16, 6.0, x0=s * 3.4) for y in (0.5, 1.2, 9.0, 9.6)]
        prs[0] = supersection(0.6, 0.55, -0.3, 0.5, 16, 6.0, x0=s * 3.4)
        merge(main, loft(prs, livery))
    body = make_object("cutter_body", main, M, coll, subsurf=0, smooth=False)
    C.add_bevel(body, width=0.06, segments=2, angle=30)
    C.add_weighted_normals(body)
    C.apply_modifiers(body)
    C.shade_smooth_by_angle(body, 35)
    g = bmesh.new()
    # pylons, armoured canopy slit, stripes, stencil numbers, nozzles
    for s in (1, -1):
        box(g, (s * 2.75, 4.5, -0.25), (1.3, 5.0, 0.35), "green_dark")
        cyl(g, (s * 3.4, 10.0, -0.3), 0.72, 0.8, "dark", axis="Y", segs=16, r2=0.8)
        cyl(g, (s * 3.4, 10.35, -0.3), 0.62, 0.05, "engine_w", axis="Y", segs=16)
        box(g, (s * 3.4, 5.0, 0.6), (1.4, 2.0, 0.05), "marking")
        box(g, (s * 4.45, 2.0, -0.3), (0.12, 0.35, 0.12), "nav_r" if s > 0 else "nav_g")
    box(g, (0, -3.0, 1.75), (1.5, 0.9, 0.12), "glass")
    box(g, (0, -1.6, 1.98), (2.2, 0.25, 0.05), "marking")
    box(g, (0, 9.2, 0.0), (3.6, 1.2, 1.6), "dark")
    for x in (-0.9, 0.9):
        cyl(g, (x, 10.0, 0.0), 0.5, 0.5, "dark", axis="Y", segs=12)
        cyl(g, (x, 10.27, 0.0), 0.42, 0.04, "engine_w", axis="Y", segs=12)
    cyl(g, (0, -1.0, -1.35), 0.35, 0.4, "dark", axis="Z", segs=12)
    cyl(g, (0, -1.0, -1.58), 0.2, 0.06, "engine_w", axis="Z", segs=12)
    cutter_detail(g)
    kit_socks = cutter_kit(g, kit)
    hard = make_object("cutter_hard", g, M, coll, subsurf=0, smooth=False)
    suffix = "" if kit == "standard" else f"_{kit}"
    ob = join([body, hard], f"ship_cutter_alliance{suffix}_LOD0")
    sockets = {"Socket_Engine_0": (3.4, 10.4, -0.3), "Socket_Engine_1": (-3.4, 10.4, -0.3),
               "Socket_Mining_0": (0, -1.0, -1.6), "Socket_Cockpit": (0, -3.0, 1.8),
               "Socket_Nav_L": (4.5, 2.0, -0.3), "Socket_Nav_R": (-4.5, 2.0, -0.3)}
    sockets.update(kit_socks)
    return ob, sockets


# ----------------------------------------------------------------- background battleships

BSI_L = 620.0
# keyframes in metres for a 620 m hull; other classes scale them (k = L / 620)
BSI_KEYS = [(0.0, 0.4, 0.4, 0.0), (0.04, 3.5, 2.6, 0.0), (0.1, 9.0, 6.5, 0.5), (0.2, 19.0, 13.0, 1.5),
            (0.34, 29.0, 19.5, 2.5), (0.52, 36.0, 24.0, 3.5), (0.7, 35.0, 23.0, 3.5), (0.86, 31.0, 21.0, 3.0),
            (0.96, 27.0, 18.0, 2.0), (1.0, 25.0, 16.0, 2.0)]


def scaled(keys, k):
    return [(t, hw * k, hh * k, z * k) for t, hw, hh, z in keys]


def windows_row(bm, keys, L, t0, t1, z_frac, side, count, mat="window", exp_x=2.4, y0=None, size=(0.8, 2.4, 0.9),
                gap_every=0):
    y0 = -L / 2 if y0 is None else y0
    for i in range(count):
        if gap_every and i % gap_every == gap_every - 1:
            continue
        t = t0 + (t1 - t0) * (i + 0.5) / count
        hw, hh, z0 = interp(keys, t)
        z = z0 + hh * z_frac
        x = hw * (1 - abs(z_frac) ** exp_x) ** (1 / exp_x) if abs(z_frac) < 1 else 0
        box(bm, (side * (x * 1.045 + 0.05), y0 + t * L, z), size, mat)


def surface_point(keys, L, t, z_frac, side, exp_x=2.4, grow=1.0):
    hw, hh, z0 = interp(keys, t)
    z = z0 + hh * z_frac
    x = hw * (1 - abs(z_frac) ** exp_x) ** (1 / exp_x)
    return Vector((side * x * grow, -L / 2 + t * L, z))


BRIDGE_TIERS = ((22, 46, 8), (16, 34, 8), (11, 22, 7), (6, 12, 6))


def build_bs_imperial(M, coll, L=BSI_L, cls="battleship", name="bg_bs_imperial"):
    """Imperial-family capital ship. cls: 'flagship' (taller gilded crest, twin bridges),
    'battleship', 'cruiser' (no lateral wings, fewer batteries)."""
    k = L / BSI_L
    keys = scaled(BSI_KEYS, k)
    EX, ET = 2.4, 2.1
    main = hull(keys, L, n=28, samples=44, exp_x=EX, exp_top=ET, mat="white")
    plates = bmesh.new()
    segs = [(0.1, 0.2), (0.22, 0.36), (0.38, 0.52), (0.54, 0.68), (0.7, 0.84), (0.86, 0.95)]
    for i, (t0, t1) in enumerate(segs):
        g = 1.03 + 0.012 * (i % 2)
        merge(plates, plate(keys, L, t0, t1, math.radians(52), math.radians(128), g, "white", EX, ET, n=12,
                            samples=10, thick=0.025))
        for sgn in (1, -1):
            for (a0, a1, gg) in ((8, 46, g - 0.006), (-38, 4, g + 0.004)):
                if sgn < 0:
                    a0, a1 = 180 - a1, 180 - a0
                merge(plates, plate(keys, L, t0 + 0.012, t1 - 0.012, math.radians(a0), math.radians(a1), gg, "white",
                                    EX, ET, n=8, samples=8, thick=0.025))
        merge(plates, band(keys, L, t1 + 0.006, 0.004, "gold", grow=1.06, n=28, exp_x=EX, exp_top=ET))
    # long prow ram: a silver needle beyond the bow
    rings = [supersection(0.15 * k, 0.15 * k, 0, -L / 2 - 40 * k, 12, 2.0),
             supersection(1.6 * k, 1.2 * k, 0, -L / 2 + 6 * k, 12, 2.0),
             supersection(3.4 * k, 2.5 * k, 0, -L / 2 + 24 * k, 12, 2.0)]
    merge(main, loft(rings, "silver"))
    if cls != "cruiser":
        for sgn in (1, -1):
            merge(main, wing(-60 * k, 230 * k, 140 * k, 300 * k, 30 * k, 92 * k, 0.0, 12.0 * k, sgn, mat="white",
                             dz_tip=-6 * k))
            merge(plates, wing(-20 * k, 200 * k, 150 * k, 280 * k, 40 * k, 84 * k, 0.4 * k, 13.0 * k, sgn,
                               mat="white", dz_tip=-5.5 * k))
            merge(plates, wing(-62 * k, -46 * k, 136 * k, 146 * k, 31 * k, 91 * k, 0.0, 12.6 * k, sgn, mat="gold",
                               dz_tip=-6 * k))
    crest_h = 34 if cls == "flagship" else 22
    rings = []
    for z_off, y0, y1 in ((0, -150, 250), (crest_h * 0.6, -60, 230), (crest_h, 40, 210)):
        rings.append([Vector((1.8 * k * math.cos(2 * math.pi * i / 10) * (1 - z_off / (crest_h * 1.4)),
                              (y0 + y1) / 2 * k + (y1 - y0) / 2 * k * math.sin(2 * math.pi * i / 10),
                              interp(keys, 0.55)[1] + 2 * k + z_off * k)) for i in range(10)])
    merge(main, loft(rings, "gold" if cls == "flagship" else "silver"))
    body = make_object(f"{name}_body", main, M, coll, subsurf=2)
    pl = make_object(f"{name}_plates", plates, M, coll, subsurf=1)
    g = bmesh.new()
    for sgn in (1, -1):
        for zf, n_, t0, t1 in ((0.42, 150, 0.16, 0.92), (0.28, 170, 0.14, 0.93), (-0.12, 140, 0.2, 0.9)):
            windows_row(g, keys, L, t0, t1, zf, sgn, int(n_ * (0.6 if cls == "cruiser" else 1)),
                        size=(0.6 * k, 1.9 * k, 0.8 * k), gap_every=7)
    nb = 10 if cls == "cruiser" else 18
    for sgn in (1, -1):
        for zf in (0.05, -0.32):
            for i in range(nb):
                t = 0.26 + 0.56 * (i + 0.5) / nb
                p = surface_point(keys, L, t, zf, sgn, grow=1.04)
                box(g, tuple(p), (2.6 * k, 7.0 * k, 3.4 * k), "dark")
                for dz in (-0.7, 0.7):
                    cyl(g, (p.x + sgn * 4.0 * k, p.y, p.z + dz * k), 0.42 * k, 8.0 * k, "silver", axis="X", segs=8)
    bridges = (0.6, 0.38) if cls == "flagship" else (0.62,)
    for tb in bridges:
        hw, hh, z0 = interp(keys, tb)
        base_z = z0 + hh * 1.02
        yb = -L / 2 + tb * L
        zacc = base_z
        for (w, d, h) in BRIDGE_TIERS:
            zc = zacc + h * k / 2
            box(g, (0, yb, zc), (w * k, d * k, h * k), "white")
            box(g, (0, yb, zc + h * k / 2), ((w + 1.2) * k, (d + 1.2) * k, 0.6 * k), "gold")
            for sgn in (1, -1):
                box(g, (sgn * (w / 2 + 0.05) * k, yb, zc + 0.6 * k), (0.3 * k, d * 0.8 * k, 1.2 * k), "window")
            box(g, (0, yb - d / 2 * k - 0.05 * k, zc + 0.6 * k), (w * 0.8 * k, 0.3 * k, 1.2 * k), "window")
            zacc += h * k
        cyl(g, (0, yb + 4 * k, zacc + 9 * k), 0.4 * k, 18 * k, "silver", axis="Z", segs=8)
    for tt in (0.3, 0.42, 0.78, 0.86):
        if cls == "cruiser" and tt in (0.42, 0.78):
            continue
        hw, hh, z0 = interp(keys, tt)
        zc = z0 + hh * 1.03
        yc = -L / 2 + tt * L
        cyl(g, (0, yc, zc + 1.5 * k), 6.5 * k, 3.0 * k, "silver", axis="Z", segs=20)
        box(g, (0, yc - 2 * k, zc + 4 * k), (7 * k, 8 * k, 3 * k), "white")
        for dx in (-1.6, 1.6):
            cyl(g, (dx * k, yc - 12 * k, zc + 4 * k), 0.7 * k, 16 * k, "silver", axis="Y", segs=8)
    ys = L / 2
    cyl(g, (0, ys - 2 * k, 2 * k), 20 * k, 14 * k, "silver", axis="Y", segs=40, r2=22 * k)
    cyl(g, (0, ys + 5.2 * k, 2 * k), 18.5 * k, 0.6 * k, "engine_b", axis="Y", segs=40)
    for x, z in ((24, 10), (-24, 10), (24, -8), (-24, -8)):
        cyl(g, (x * k, ys - 8 * k, z * k), 7.5 * k, 12 * k, "silver", axis="Y", segs=24, r2=8.5 * k)
        cyl(g, (x * k, ys - 1.8 * k, z * k), 6.8 * k, 0.5 * k, "engine_b", axis="Y", segs=24)
    hard = make_object(f"{name}_hard", g, M, coll, subsurf=0)
    for p_ in hard.data.polygons:
        p_.use_smooth = False
    return join([body, pl, hard], name)


BSA_L = 560.0
BSA_KEYS = [(0.0, 8.0, 6.0, -2.0), (0.06, 26.0, 14.0, 0.0), (0.16, 34.0, 18.0, 0.0), (0.6, 34.0, 18.0, 0.0),
            (0.7, 40.0, 22.0, 0.0), (1.0, 40.0, 22.0, 0.0)]


def build_bs_alliance(M, coll, L=BSA_L, name="bg_bs_alliance"):
    k = L / BSA_L
    keys = scaled(BSA_KEYS, k)
    main = hull(keys, L, n=16, samples=14, exp_x=8.0, exp_top=8.0, mat="green")
    for (y0, y1, hw, hh, z) in ((-120, 60, 22, 8, 22), (-60, 20, 14, 7, 34), (-40, 0, 9, 6, 45),
                                (120, 250, 30, 9, 26)):
        rings = [supersection(hw * k, hh * k, z * k, y * k, 16, 8.0) for y in (y0, y0 + 6, y1 - 6, y1)]
        rings[0] = supersection(hw * 0.7 * k, hh * 0.6 * k, (z - 1) * k, y0 * k, 16, 8.0)
        merge(main, loft(rings, "green"))
    for sgn in (1, -1):
        rings = [supersection(8 * k, 10 * k, -4 * k, y * k, 16, 8.0, x0=sgn * 40 * k) for y in (-40, -30, 180, 190)]
        merge(main, loft(rings, "green_dark"))
    body = make_object(f"{name}_body", main, M, coll, subsurf=0, smooth=False)
    C.add_bevel(body, width=0.4 * k, segments=1, angle=30)
    C.apply_modifiers(body)
    C.shade_smooth_by_angle(body, 35)
    g = bmesh.new()
    for sgn in (1, -1):
        for z in (4.0, 9.0, -2.0):
            for i in range(60):
                if i % 9 == 8:
                    continue
                box(g, (sgn * 34.3 * k, (-200 + i * 7) * k, z * k), (0.6 * k, 2.2 * k, 1.0 * k), "window")
        box(g, (sgn * 34.3 * k, -120 * k, -6.0 * k), (0.3 * k, 120 * k, 2.5 * k), "marking")
        for i in range(14):
            box(g, (sgn * 34.6 * k, (-230 + i * 34) * k, -11 * k), (0.8 * k, 32 * k, 9 * k), "green_dark")
        for i in range(10):
            yc = (-30 + i * 21) * k
            box(g, (sgn * 48.4 * k, yc, -4 * k), (1.0 * k, 9 * k, 5 * k), "dark")
            for dz in (-1.2, 1.2):
                cyl(g, (sgn * 53 * k, yc, (-4 + dz) * k), 0.5 * k, 9 * k, "dark", axis="X", segs=8)
        for j in range(3):  # hull number blocks (no real insignia)
            box(g, (sgn * 34.5 * k, (-235 + j * 8) * k, 10 * k), (0.4 * k, 5 * k, 7 * k), "marking")
    box(g, (0, 0, 22.5 * k), (40 * k, 400 * k, 0.6 * k), "green_dark")
    # two-tone armour panels with seams over the whole dorsal deck and flanks
    for i in range(18):
        y = (-250 + i * 30) * k
        for sgn in (1, -1):
            box(g, (sgn * 16 * k, y, 18.3 * k), (14 * k, 26 * k, 0.5 * k), "green_light" if (i + (sgn > 0)) % 2 else "green")
            box(g, (sgn * 34.35 * k, y, 14 * k), (0.5 * k, 26 * k, 6 * k), "green_light" if i % 3 == 0 else "green")
        box(g, (0, y + 14 * k, 18.4 * k), (64 * k, 1.0 * k, 0.6 * k), "dark")
    # superstructure: window bands, antenna array, sensor domes, hangar doors, stencil hull number
    for z, y0, y1, hw in ((24, -118, 58, 22), (36, -58, 18, 14), (47, -38, -2, 9), (28, 122, 248, 30)):
        for sgn in (1, -1):
            box(g, (sgn * (hw + 0.05) * k, (y0 + y1) / 2 * k, z * k), (0.3 * k, (y1 - y0 - 8) * k, 1.0 * k), "window")
    for i in range(6):
        cyl(g, ((-10 + i * 4) * k, -10 * k, 56 * k), 0.25 * k, (10 + (i % 3) * 4) * k, "dark", axis="Z", segs=6)
    for y in (-90, 150, 220):
        bmv = bmesh.ops.create_uvsphere(g, u_segments=16, v_segments=8, radius=4.5 * k)["verts"]
        bmesh.ops.scale(g, vec=(1, 1, 0.6), verts=bmv)
        bmesh.ops.translate(g, vec=(0, y * k, (30 if y > 100 else 26) * k), verts=bmv)
        for f in {f for v_ in bmv for f in v_.link_faces}:
            f.material_index = I("green_light")
    for sgn in (1, -1):
        box(g, (sgn * 40.3 * k, 230 * k, -2 * k), (0.4 * k, 40 * k, 14 * k), "dark")          # hangar doors
        box(g, (sgn * 40.35 * k, 230 * k, 5.6 * k), (0.4 * k, 40 * k, 0.8 * k), "warn")
        stencil(g, "112", (sgn * 34.9 * k, (-170 if sgn > 0 else -150) * k, 10 * k), (0, sgn, 0), (0, 0, 1),
                8 * k, "marking")
    box(g, (0, -40.6 * k, 48 * k), (16 * k, 0.4 * k, 2.2 * k), "window")
    cyl(g, (0, -20 * k, 62 * k), 0.5 * k, 22 * k, "dark", axis="Z", segs=8)
    for x in (-24, -8, 8, 24):
        for z in (-10, 10):
            box(g, (x * k, 282 * k, z * k), (13 * k, 6 * k, 15 * k), "dark")
            box(g, (x * k, 285.2 * k, z * k), (11 * k, 0.4 * k, 13 * k), "engine_w")
    hard = make_object(f"{name}_hard", g, M, coll, subsurf=0, smooth=False)
    return join([body, hard], name)


def lod1(ob, coll, ratio=0.35):
    cp = ob.copy()
    cp.data = ob.data.copy()
    cp.name = ob.name.replace("_LOD0", "_LOD1")
    coll.objects.link(cp)
    d = cp.modifiers.new("Dec", "DECIMATE")
    d.ratio = ratio
    C.apply_modifiers(cp)
    cp.hide_render = True
    return cp


def build_all():
    M = materials()
    coll = bpy.data.collections.new("v3")
    bpy.context.scene.collection.children.link(coll)
    out = {}
    jobs = []
    for kit in KITS:
        suffix = "" if kit == "standard" else f"_{kit}"
        jobs.append((f"lancet_imperial{suffix}", lambda M_, c, k_=kit: build_lancet(M_, c, kit=k_)))
        jobs.append((f"cutter_alliance{suffix}", lambda M_, c, k_=kit: build_cutter(M_, c, kit=k_)))
    for name, fn in jobs:
        ob, socks = fn(M, coll)
        C.uv_box(ob, 1.0)
        r = C.bounding_radius([ob])
        assert r <= HULL_RADIUS_M, f"{name}: bounding radius {r:.2f} > {HULL_RADIUS_M}"
        root = bpy.data.objects.new(f"ship_{name}", None)
        coll.objects.link(root)
        ob.parent = root
        l1 = lod1(ob, coll)
        l1.parent = root
        for sname, loc in socks.items():
            e = C.add_empty(sname, loc, parent=root, size=0.4)
        print("V3REPORT", name, "r=%.2f" % r, "tris", C.tri_count(ob), C.tri_count(l1))
        out[name] = root
    for name, fn in (("bs_imperial", lambda M_, c: build_bs_imperial(M_, c)),
                     ("fs_imperial", lambda M_, c: build_bs_imperial(M_, c, L=820, cls="flagship", name="bg_fs_imperial")),
                     ("cr_imperial", lambda M_, c: build_bs_imperial(M_, c, L=320, cls="cruiser", name="bg_cr_imperial")),
                     ("bs_alliance", build_bs_alliance)):
        ob = fn(M, coll)
        C.uv_box(ob, 1.0)
        print("V3REPORT", name, "len=%.0f" % (ob.dimensions.y), "tris", C.tri_count(ob))
        out[name] = ob
    return M, out


def main():
    C.reset_scene()
    C.setup_cycles(samples=160, res=(1920, 1080))
    sky = os.path.join(C.TEX, "skybox", "cradle_sky_equirect.png")
    C.world_space("#04060C", 1.0, hdri=sky)
    C.studio_lights()
    M, out = build_all()
    os.makedirs(EXPORT, exist_ok=True)
    for n in [k_ for k_ in out if k_.startswith(("lancet", "cutter"))]:
        C.export_fbx(os.path.join(EXPORT, f"ship_{n}.fbx"), [out[n]])
    for n in ("bs_imperial", "fs_imperial", "cr_imperial", "bs_alliance"):
        C.export_fbx(os.path.join(EXPORT, f"bg_{n}.fbx"), [out[n]])
    C.save_blend(os.path.join(C.SOURCE, "logh_ships.blend"))
    if os.environ.get("LOGH_PREVIEW", "0") == "0":
        return
    previews(M, out)


def show_only(objs, out):
    keep = set()
    for o in objs:
        keep.add(o)
        keep.update(o.children_recursive)
    for o in bpy.data.objects:
        if o.type == "MESH":
            o.hide_render = o not in keep or o.name.endswith("_LOD1")


def previews(M, out):
    cam = C.add_camera((0, 0, 0), (0, 0, 0), lens=40)
    li, ca = out["lancet_imperial"], out["cutter_alliance"]
    for o in out.values():
        o.location = (0, 0, 0)
    # close-ups of the imperial lancet
    show_only([li], out)
    for i, (loc, tgt, lens) in enumerate((((14, -20, 6), (0, -1, 0), 40), ((-13, 17, 7), (0, 2, 0), 40),
                                          ((5, -9, 2.5), (0, -3, 0.8), 30))):
        cam.location = loc
        C.look_at(cam, tgt)
        cam.data.lens = lens
        C.render(os.path.join(C.PREVIEWS, f"v3_lancet_imperial_close_{i}.png"))
    show_only([ca], out)
    cam.location = (14, -20, 6)
    C.look_at(cam, (0, -1, 0))
    cam.data.lens = 40
    C.render(os.path.join(C.PREVIEWS, "v3_cutter_alliance_close_0.png"))
    # faction comparison: craft side by side + battleships behind
    li.location = (-7.5, 0, 0)
    ca.location = (7.5, 0, 0)
    bi, ba = out["bs_imperial"], out["bs_alliance"]
    bi.location = (-950, 2600, -150)
    ba.location = (950, 2900, -260)
    bi.rotation_euler = (0, 0, math.radians(-115))  # broadside, bow toward screen right
    ba.rotation_euler = (0, 0, math.radians(110))   # broadside, bow toward screen left
    show_only([li, ca, bi, ba], out)
    cam.location = (0, -36, 9)
    C.look_at(cam, (0, 0, 1))
    cam.data.lens = 35
    cam.data.clip_end = 100000
    C.render(os.path.join(C.PREVIEWS, "v3_faction_compare.png"))
    # battleships close
    for o, nm in ((bi, "imperial"), (ba, "alliance")):
        show_only([o], out)
        o.location = (0, 0, 0)
        o.rotation_euler = (0, 0, 0)
        cam.location = (520, -560, 180)
        C.look_at(cam, (0, -20, 0))
        cam.data.lens = 40
        C.render(os.path.join(C.PREVIEWS, f"v3_bg_bs_{nm}.png"))


if __name__ == "__main__":
    main()

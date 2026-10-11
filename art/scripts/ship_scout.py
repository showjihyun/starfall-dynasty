"""Scout S-01 hull + 3 module-kit variants (Standard / Prospector / Courier) + 6 liveries.

Run:  blender --background --factory-startup --python art/scripts/ship_scout.py
Out:  art/export/ship_scout_{variant}.fbx (LOD0..2 children + Socket_* empties),
      art/source/ship_scout.blend, art/previews/ship_scout_*.png

Scale: data/ships/scout-s01.json hull_radius_m = 12 -> every variant's bounding radius
from the pivot must stay <= 12 m (asserted). Nose = Blender -Y, up = +Z.
Construction: lofted hull sections -> panel groups inset & separated by real gaps over a dark
inner core -> greebles snapped to the surface by ray casts -> module kit per variant.
"""
import math
import os
import random
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import bpy
import bmesh
from mathutils import Vector, Matrix
from mathutils.bvhtree import BVHTree

import common as C

SEED = 1701
LOW = False  # True while building the hand-made LOD2: no panel gaps, cores, greebles or canopy frame
HULL_RADIUS_M = 12.0

LIVERIES = [
    ("amber", "#D08A1C"),
    ("cobalt", "#2A55B8"),
    ("crimson", "#A82A2A"),
    ("olive", "#66733A"),
    ("graphite", "#34373E"),
    ("teal", "#1F8C8A"),
]

ORDER = ["hull", "dark", "accent", "glass", "engine", "nav_r", "nav_g", "nav_w", "warn", "laser", "window",
         "core", "metal"]


def idx(name):
    return ORDER.index(name)


def materials():
    M = {}
    M["hull"] = C.mat_painted("Hull_Light", "#CFCBC2", metallic=0.05, rough=(0.34, 0.6),
                              tex_set="blue_metal_plate", uv_scale=0.3, normal_strength=0.25, grime=0.2)
    M["dark"] = C.mat_painted("Hull_Dark", "#33363C", metallic=0.55, rough=(0.38, 0.7),
                              tex_set="metal_plate_02", uv_scale=0.45, normal_strength=0.5, grime=0.12)
    M["accent"] = C.mat_painted("Livery", LIVERIES[0][1], metallic=0.05, rough=(0.3, 0.5),
                                tex_set="blue_metal_plate", uv_scale=0.3, normal_strength=0.25, grime=0.25)
    M["glass"] = C.mat_simple("Canopy_Glass", "#020406", metallic=0.6, rough=0.04, coat=1.0,
                              emission_hex="#3A8FB8", emission=0.05)
    M["engine"] = C.mat_emit("Engine_Glow", "#9FD8FF", 9.0)
    M["nav_r"] = C.mat_emit("Nav_Red", "#FF3B30", 25.0)
    M["nav_g"] = C.mat_emit("Nav_Green", "#30FF6A", 25.0)
    M["nav_w"] = C.mat_emit("Nav_White", "#FFFFFF", 20.0)
    M["warn"] = C.mat_painted("Warning_Amber", "#E0A12A", metallic=0.05, rough=(0.3, 0.55),
                              tex_set="blue_metal_plate", uv_scale=0.3, grime=0.3)
    M["laser"] = C.mat_emit("Mining_Lens", "#5FF0FF", 22.0)
    M["window"] = C.mat_emit("Window_Warm", "#FFC98A", 6.0)
    M["core"] = C.mat_simple("Hull_Core", "#0B0C0E", metallic=0.3, rough=0.7)
    M["metal"] = C.mat_painted("Bare_Metal", "#8C8F94", metallic=0.95, rough=(0.22, 0.45),
                               tex_set="metal_plate_02", uv_scale=0.6, normal_strength=0.3, grime=0.1)
    return M


# ----------------------------------------------------------------- low-level bmesh helpers

def merge(dst, src):
    me = bpy.data.meshes.new("tmp")
    src.to_mesh(me)
    src.free()
    dst.from_mesh(me)
    bpy.data.meshes.remove(me)


def _mark(bm, verts, mat):
    for f in {f for v in verts for f in v.link_faces}:
        f.material_index = idx(mat)


def box(bm, center, size, mat, mtx=None):
    vs = bmesh.ops.create_cube(bm, size=1.0)["verts"]
    bmesh.ops.scale(bm, vec=size, verts=vs)
    bmesh.ops.translate(bm, vec=center, verts=vs) if mtx is None else bmesh.ops.transform(bm, matrix=mtx @ Matrix.Translation(center), verts=vs)
    _mark(bm, vs, mat)
    return vs


def cyl(bm, center, r, depth, mat, axis="Y", segs=12, r2=None, mtx=None):
    vs = bmesh.ops.create_cone(bm, cap_ends=True, cap_tris=False, segments=segs, radius1=r,
                               radius2=r if r2 is None else r2, depth=depth)["verts"]
    rot = {"Y": Matrix.Rotation(math.radians(-90), 4, "X"), "X": Matrix.Rotation(math.radians(90), 4, "Y"),
           "Z": Matrix.Identity(4)}[axis]
    m = Matrix.Translation(center) @ rot
    if mtx is not None:
        m = mtx @ m
    bmesh.ops.transform(bm, matrix=m, verts=vs)
    _mark(bm, vs, mat)
    return vs


def section(w, h, c, y, z0=0.0, x0=0.0, tc=None):
    """16-point chamfered section (each of the 8 sides split in two) in the XZ plane at Y=y."""
    tc = c if tc is None else tc
    hw, hh = w / 2, h / 2
    p8 = [(-hw + c, -hh), (hw - c, -hh), (hw, -hh + c), (hw, hh - tc), (hw - tc, hh), (-hw + tc, hh),
          (-hw, hh - tc), (-hw, -hh + c)]
    pts = []
    for i in range(8):
        a, b = p8[i], p8[(i + 1) % 8]
        pts.append(a)
        pts.append(((a[0] + b[0]) / 2, (a[1] + b[1]) / 2))
    return [Vector((x0 + x, y, z0 + z)) for x, z in pts]


def grid_loft(sections, mat="hull", cap=True):
    """Loft with a face->(ring, seg) map so panels can be grouped."""
    bm = bmesh.new()
    rings = [[bm.verts.new(p) for p in s] for s in sections]
    n = len(sections[0])
    grid = {}
    for i, (a, b) in enumerate(zip(rings, rings[1:])):
        for j in range(n):
            f = bm.faces.new((a[j], a[(j + 1) % n], b[(j + 1) % n], b[j]))
            f.material_index = idx(mat)
            grid[f] = (i, j)
    caps = []
    if cap:
        caps.append(bm.faces.new(list(reversed(rings[0]))))
        caps.append(bm.faces.new(rings[-1]))
        for f in caps:
            f.material_index = idx(mat)
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
    return bm, grid, caps


def panel_shell(bm, grid, rng, gap=0.05, ring_span=(2, 3), seg_span=2, skip=lambda f: False):
    """Group grid faces into panels, inset each group and delete the inset border -> real gaps."""
    if LOW:
        return
    by_ring = {}
    for f, (i, j) in grid.items():
        by_ring.setdefault(i, {})[j] = f
    nrings = max(by_ring) + 1
    nseg = max(j for _, j in grid.values()) + 1
    groups = []
    i = 0
    while i < nrings:
        span = rng.randint(*ring_span)
        for j0 in range(0, nseg, seg_span):
            g = [by_ring[ii][jj] for ii in range(i, min(i + span, nrings)) for jj in range(j0, j0 + seg_span)
                 if jj in by_ring.get(ii, {})]
            g = [f for f in g if not skip(f)]
            if g:
                groups.append(g)
        i += span
    border = []
    for g in groups:
        g = [f for f in g if f.is_valid]
        if not g:
            continue
        r = bmesh.ops.inset_region(bm, faces=g, thickness=gap, depth=0.0, use_even_offset=True,
                                   use_boundary=True)
        border.extend(r["faces"])
    bmesh.ops.delete(bm, geom=[f for f in border if f.is_valid], context="FACES")


def core_from(sections, inset=0.06):
    """Dark inner core = the same loft pushed inward so panel gaps read as dark seams."""
    if LOW:
        return bmesh.new()
    bm, _, _ = grid_loft(sections, mat="core")
    bm.normal_update()
    for v in bm.verts:
        v.co -= v.normal * inset
    return bm


# ----------------------------------------------------------------- hull parts

FUSE = [  # y, width, height, chamfer, z0, top chamfer
    (-10.3, 0.5, 0.3, 0.12, -0.3, None),
    (-9.6, 1.3, 0.75, 0.3, -0.25, None),
    (-8.6, 2.2, 1.25, 0.5, -0.18, None),
    (-7.4, 2.9, 1.7, 0.65, -0.1, None),
    (-6.0, 3.4, 2.0, 0.72, -0.02, 0.85),
    (-4.6, 3.8, 2.3, 0.75, 0.05, 0.95),
    (-3.2, 4.1, 2.5, 0.75, 0.1, 1.0),
    (-1.8, 4.3, 2.65, 0.72, 0.13, 0.95),
    (-0.4, 4.4, 2.7, 0.7, 0.15, 0.92),
    (1.0, 4.4, 2.72, 0.7, 0.15, 0.9),
    (2.4, 4.4, 2.72, 0.7, 0.15, 0.88),
    (3.8, 4.35, 2.7, 0.7, 0.15, 0.86),
    (5.2, 4.2, 2.6, 0.7, 0.13, 0.82),
    (6.6, 3.95, 2.45, 0.68, 0.1, 0.78),
    (7.8, 3.6, 2.25, 0.62, 0.07, 0.72),
    (8.8, 3.2, 2.0, 0.56, 0.05, 0.64),
    (9.3, 2.9, 1.8, 0.5, 0.05, 0.58),
]


def fuse_at(y):
    """Interpolated (half width, half height, z0) of the fuselage at y."""
    for a, b in zip(FUSE, FUSE[1:]):
        if a[0] <= y <= b[0]:
            t = (y - a[0]) / (b[0] - a[0])
            return ((a[1] + (b[1] - a[1]) * t) / 2, (a[2] + (b[2] - a[2]) * t) / 2, a[4] + (b[4] - a[4]) * t)
    return FUSE[-1][1] / 2, FUSE[-1][2] / 2, FUSE[-1][4]


def fuselage(rng):
    secs = [section(w, h, c, y, z0=z0, tc=tc) for y, w, h, c, z0, tc in FUSE]
    bm, grid, caps = grid_loft(secs)
    # canopy well: leave the top faces under the canopy unpanelled
    def skip(f):
        cen = f.calc_center_median()
        return cen.y < -9.0
    panel_shell(bm, grid, rng, gap=0.045, skip=skip)
    for f in bm.faces:
        cen = f.calc_center_median()
        n = f.normal
        if n.z < -0.55:
            f.material_index = idx("dark")
        if cen.y > 9.25 and n.y > 0.9:
            f.material_index = idx("dark")
    core = core_from(secs)
    merge(bm, core)
    return bm, secs


CANOPY_PROF = [(-7.0, 0.15, 0.05), (-6.3, 0.75, 0.42), (-5.4, 1.15, 0.7), (-4.4, 1.32, 0.82),
               (-3.4, 1.28, 0.78), (-2.5, 1.05, 0.6), (-1.9, 0.7, 0.3)]


def _canopy_loft(prof, arc, grow=0.0, mat="glass"):
    bm = bmesh.new()
    secs = []
    for y, hw, hz in prof:
        _, hh2, z0 = fuse_at(y)
        zb = z0 + hh2 - 0.08
        secs.append([Vector((-(hw + grow) * math.cos(math.pi * k / arc), y,
                             zb + (hz + grow) * math.sin(math.pi * k / arc))) for k in range(arc + 1)])
    rings = [[bm.verts.new(p) for p in s] for s in secs]
    for a, b in zip(rings, rings[1:]):
        for j in range(arc):
            bm.faces.new((a[j], a[j + 1], b[j + 1], b[j]))
    bm.faces.new(rings[0][::-1])
    bm.faces.new(rings[-1])
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
    for f in bm.faces:
        f.material_index = idx(mat)
    return bm


def seg_box(bm, p, q, t, mat):
    """Square tube of side t from p to q."""
    d = q - p
    L_ = d.length
    if L_ < 1e-6:
        return
    z = d.normalized()
    up = Vector((0, 0, 1)) if abs(z.z) < 0.9 else Vector((1, 0, 0))
    x = up.cross(z).normalized()
    y = z.cross(x)
    m = Matrix((x, y, z)).transposed().to_4x4()
    m.translation = (p + q) / 2
    vs = bmesh.ops.create_cube(bm, size=1.0)["verts"]
    bmesh.ops.scale(bm, vec=(t, t, L_ + t), verts=vs)
    bmesh.ops.transform(bm, matrix=m, verts=vs)
    _mark(bm, vs, mat)


def canopy():
    """Smooth glass (7 rings x 12 arc segments) + frame tubes following the glass surface."""
    arc = 12
    glass = _canopy_loft(CANOPY_PROF, arc)
    frame = bmesh.new()
    pts = []
    for y, hw, hz in CANOPY_PROF:
        _, hh2, z0 = fuse_at(y)
        zb = z0 + hh2 - 0.08
        pts.append([Vector((-(hw + 0.025) * math.cos(math.pi * k / arc), y,
                            zb + (hz + 0.025) * math.sin(math.pi * k / arc))) for k in range(arc + 1)])
    for k in (0, 3, 6, 9, 12):               # longitudinal bars
        for a_, b_ in zip(pts, pts[1:]):
            seg_box(frame, a_[k], b_[k], 0.075, "dark")
    for ri in (1, 3, 5):                      # hoops
        for k in range(arc):
            seg_box(frame, pts[ri][k], pts[ri][k + 1], 0.075, "dark")
    return glass, frame


def wing(side, rng):
    sx = 1 if side > 0 else -1

    def sec(x, y0, y1, t, z):
        pts = [(y0, z), (y0 + 0.5, z + t * 0.45), (y0 + (y1 - y0) * 0.35, z + t / 2), (y1 - 0.5, z + t * 0.4),
               (y1, z), (y1 - 0.5, z - t * 0.4), (y0 + (y1 - y0) * 0.35, z - t / 2), (y0 + 0.5, z - t * 0.45)]
        return [Vector((sx * x, y, zz)) for y, zz in pts]

    secs = [sec(1.9, -3.6, 7.2, 0.75, -0.05), sec(2.8, -1.8, 7.3, 0.62, -0.08), sec(3.7, -0.2, 7.4, 0.5, -0.1),
            sec(4.1, 0.5, 7.4, 0.45, -0.1)]
    if sx < 0:
        secs = [list(reversed(s)) for s in secs]
    bm, grid, _ = grid_loft(secs)
    panel_shell(bm, grid, rng, gap=0.04, ring_span=(1, 1), seg_span=2)
    for f in bm.faces:
        cen = f.calc_center_median()
        if f.normal.z < -0.5:
            f.material_index = idx("dark")
        elif f.normal.z > 0.5 and 4.0 < cen.y < 6.6 and abs(cen.x) > 2.6:
            f.material_index = idx("accent")
    merge(bm, core_from(secs, inset=0.05))
    return bm


def nacelle(side, rng):
    sx = 1 if side > 0 else -1
    x0 = sx * 4.6
    prof = [(-1.1, 0.62), (-0.5, 0.92), (0.4, 1.08), (2.0, 1.12), (3.6, 1.12), (5.2, 1.12), (6.8, 1.1),
            (7.9, 1.02), (8.6, 0.9)]
    secs = [section(2 * r, 2 * r, r * 0.42, y, x0=x0) for y, r in prof]
    bm, grid, caps = grid_loft(secs, cap=False)
    panel_shell(bm, grid, rng, gap=0.045, ring_span=(1, 2), seg_span=4)
    for f in bm.faces:
        cen = f.calc_center_median()
        if f.normal.z < -0.5:
            f.material_index = idx("dark")
        if 0.4 < cen.y < 2.0 and f.normal.z > 0.3:
            f.material_index = idx("accent")
    merge(bm, core_from(secs, inset=0.06))
    g = bmesh.new()
    # intake: dark lip + grille fins + dark fan disc
    cyl(g, (x0, -1.15, 0), 0.62, 0.12, "dark", segs=16)
    cyl(g, (x0, -0.9, 0), 0.5, 0.4, "core", segs=16)
    for k in range(8):
        a = math.pi * k / 8
        box(g, (x0, -1.2, 0), (1.05, 0.06, 0.05), "metal",
            mtx=Matrix.Translation((x0, -1.2, 0)) @ Matrix.Rotation(a, 4, "Y") @ Matrix.Translation((-x0, 1.2, 0)))
    # segmented reinforcement rings
    for y in (2.8, 6.0):
        cyl(g, (x0, y, 0), 1.17, 0.16, "dark", segs=16)
    # nozzle: throat ring, petals, glowing core
    cyl(g, (x0, 8.75, 0), 0.92, 0.3, "dark", segs=20)
    for k in range(12):
        a = 2 * math.pi * k / 12
        m = (Matrix.Translation((x0, 9.35, 0)) @ Matrix.Rotation(a, 4, "Y") @ Matrix.Translation((0, 0, 0.88))
             @ Matrix.Rotation(math.radians(9), 4, "X"))
        vs = bmesh.ops.create_cube(g, size=1.0)["verts"]
        bmesh.ops.scale(g, vec=(0.44, 1.05, 0.05), verts=vs)
        bmesh.ops.transform(g, matrix=m, verts=vs)
        _mark(g, vs, "metal")
    cyl(g, (x0, 9.2, 0), 0.76, 0.04, "engine", segs=20)
    cyl(g, (x0, 8.95, 0), 0.5, 0.5, "engine", segs=16, r2=0.7)
    # outer radiator fins
    for k in range(6):
        box(g, (x0 + sx * 1.18, 3.4 + k * 0.48, 0.0), (0.32, 0.07, 1.25), "metal")
    box(g, (x0 + sx * 1.05, 4.6, 0.0), (0.12, 3.2, 0.25), "dark")
    merge(bm, g)
    return bm


# ----------------------------------------------------------------- surface snapping

class Snapper:
    def __init__(self, bms):
        tmp = bmesh.new()
        for b in bms:
            me = bpy.data.meshes.new("snap")
            b.to_mesh(me)
            tmp.from_mesh(me)
            bpy.data.meshes.remove(me)
        self.tree = BVHTree.FromBMesh(tmp)
        tmp.free()

    def hit(self, origin, direction):
        loc, nor, _, _ = self.tree.ray_cast(Vector(origin), Vector(direction).normalized(), 50.0)
        return loc, nor

    def frame(self, origin, direction):
        """Matrix whose +Z = surface normal at the hit, +Y roughly along the hull."""
        loc, nor = self.hit(origin, direction)
        if loc is None:
            return None
        z = nor.normalized()
        y = Vector((0, 1, 0))
        if abs(z.dot(y)) > 0.95:
            y = Vector((1, 0, 0))
        x = y.cross(z).normalized()
        y = z.cross(x).normalized()
        m = Matrix((x, y, z)).transposed().to_4x4()
        m.translation = loc
        return m


def place(bm, frame, kind, rng, mat="dark", scale=1.0):
    """Greeble library: built around origin with +Z out of the surface, then transformed."""
    if frame is None:
        return
    s = scale
    if kind == "vent":
        box(bm, (0, 0, 0.04 * s), (0.9 * s, 1.3 * s, 0.1 * s), mat, mtx=frame)
        for k in range(5):
            box(bm, (0, (-0.5 + k * 0.25) * s, 0.1 * s), (0.75 * s, 0.07 * s, 0.08 * s), "metal", mtx=frame)
    elif kind == "hatch":
        box(bm, (0, 0, 0.02 * s), (0.8 * s, 1.1 * s, 0.05 * s), mat, mtx=frame)
        box(bm, (0, 0.4 * s, 0.05 * s), (0.4 * s, 0.06 * s, 0.05 * s), "warn", mtx=frame)
    elif kind == "rcs":
        box(bm, (0, 0, 0.15 * s), (0.45 * s, 0.55 * s, 0.32 * s), mat, mtx=frame)
        for d, ax in (((0.26 * s, 0, 0.17 * s), "X"), ((0, 0.31 * s, 0.17 * s), "Y"), ((0, 0, 0.34 * s), "Z")):
            cyl(bm, d, 0.08 * s, 0.09 * s, "metal", axis=ax, segs=8, mtx=frame)
    elif kind == "dome":
        vs = bmesh.ops.create_uvsphere(bm, u_segments=12, v_segments=6, radius=0.3 * s)["verts"]
        bmesh.ops.scale(bm, vec=(1, 1, 0.5), verts=vs)
        bmesh.ops.transform(bm, matrix=frame, verts=vs)
        _mark(bm, vs, "metal")
    elif kind == "pipe":
        cyl(bm, (0, 0, 0.09 * s), 0.07 * s, 2.2 * s, "metal", axis="Y", segs=8, mtx=frame)
        for k in (-0.9, 0, 0.9):
            box(bm, (0, k * s, 0.05 * s), (0.22 * s, 0.1 * s, 0.1 * s), mat, mtx=frame)
    elif kind == "plate":
        box(bm, (0, 0, 0.02 * s), (rng.uniform(0.5, 1.0) * s, rng.uniform(0.6, 1.6) * s, 0.04 * s), mat, mtx=frame)
    elif kind == "light_r":
        box(bm, (0, 0, 0.06), (0.18, 0.4, 0.12), "nav_r", mtx=frame)
    elif kind == "light_g":
        box(bm, (0, 0, 0.06), (0.18, 0.4, 0.12), "nav_g", mtx=frame)
    elif kind == "light_w":
        box(bm, (0, 0, 0.06), (0.25, 0.25, 0.1), "nav_w", mtx=frame)


SEG7 = {  # stencil glyphs from boxes, cell 1 x 1.6 : segments a..g
    "S": "afgcd", "0": "abcdef", "1": "bc", "-": "g",
}


def stencil(bm, text, frame, height=0.45, mat="dark"):
    w = height * 0.55
    t = 0.09 * height / 0.45
    x = -(len(text) * (w + t * 2)) / 2
    for ch in text:
        segs = SEG7[ch]
        cx = x + w / 2
        S = {"a": ((cx, height, 0), (w, t)), "g": ((cx, height / 2, 0), (w, t)), "d": ((cx, 0, 0), (w, t)),
             "f": ((x, height * 0.75, 0), (t, height / 2)), "b": ((x + w, height * 0.75, 0), (t, height / 2)),
             "e": ((x, height * 0.25, 0), (t, height / 2)), "c": ((x + w, height * 0.25, 0), (t, height / 2))}
        for s in segs:
            (px, py, _), (sw, sh) = S[s]
            box(bm, (px, py - height / 2, 0.012), (sw, sh, 0.02), mat, mtx=frame)
        x += w + t * 2.2


def greebles(snap, rng):
    bm = bmesh.new()
    up, dn = (0, 0, -1), (0, 0, 1)
    # dorsal: vents, hatches, pipes
    for y in (0.4, 1.9):
        place(bm, snap.frame((0, y, 5), up), "vent", rng)
    place(bm, snap.frame((0, 3.6, 5), up), "hatch", rng)
    for sx in (1, -1):
        place(bm, snap.frame((sx * 1.45, 4.0, 5), up), "pipe", rng)
        place(bm, snap.frame((sx * 1.45, 6.6, 5), up), "plate", rng)
        place(bm, snap.frame((sx * 1.2, -0.8, 5), up), "plate", rng)
    place(bm, snap.frame((0, 7.2, 5), up), "dome", rng)
    # RCS clusters at nose and tail corners, snapped to the chamfer
    for sx in (1, -1):
        for y in (-6.3, 8.0):
            hw, hh, z0 = fuse_at(y)
            place(bm, snap.frame((sx * (hw + 3), y, z0 + hh * 0.75 + 3), (-sx, 0, -1)), "rcs", rng, scale=0.9)
        # side hatches and hull number
        place(bm, snap.frame((sx * 6, -1.4, 0.25), (-sx, 0, 0)), "hatch", rng)
        fr = snap.frame((sx * 6, -4.4, 0.2), (-sx, 0, 0))
        if fr is not None:
            # readable from outside: flip the stencil on the -X side
            rot = Matrix.Rotation(math.radians(90 if sx > 0 else -90), 4, "Z")
            stencil(bm, "S-01", fr @ rot, height=0.42, mat="dark")
        # wingtip nav lights (port = left = +X here, since nose -Y & up +Z make right = -X)
        place(bm, snap.frame((sx * 4.3, 6.5, 5), up), "plate", rng, mat="warn", scale=0.6)
    place(bm, snap.frame((9, 0.8, 0), (-1, 0, 0)), "light_r", rng)
    place(bm, snap.frame((-9, 0.8, 0), (1, 0, 0)), "light_g", rng)
    place(bm, snap.frame((0, 9.0, 5), up), "light_w", rng)
    # belly: sensor dome, skid pads, plates
    place(bm, snap.frame((0, -7.0, -5), dn), "dome", rng, scale=1.2)
    for x, y in ((1.2, -4.6), (-1.2, -4.6), (1.4, 5.4), (-1.4, 5.4)):
        place(bm, snap.frame((x, y, -5), dn), "plate", rng, scale=0.8)
    for y in (-2.0, 1.0, 3.5):
        place(bm, snap.frame((0, y, -5), dn), "hatch", rng)
    # two thin livery stripes along the spine (nose to canopy, then canopy to tail)
    for sx in (1, -1):
        for y0, y1 in ((-8.4, -7.2), (-1.4, 8.6)):
            for k in range(int((y1 - y0) / 0.5)):
                y = y0 + k * 0.5 + 0.25
                fr = snap.frame((sx * 0.85, y, 5), up)
                if fr is not None:
                    box(bm, (0, 0, 0.012), (0.18, 0.52, 0.02), "accent", mtx=fr)
    # central main nozzle
    cyl(bm, (0, 9.55, 0.12), 0.72, 0.6, "dark", axis="Y", segs=20, r2=0.86)
    cyl(bm, (0, 9.86, 0.12), 0.6, 0.04, "engine", axis="Y", segs=20)
    # pylons fuselage <-> nacelle
    for sx in (1, -1):
        box(bm, (sx * 3.25, 4.4, 0.0), (2.3, 3.6, 0.32), "dark")
        box(bm, (sx * 3.25, 4.4, 0.18), (2.1, 3.2, 0.06), "metal")
    # cockpit side windows
    for sx in (1, -1):
        hw, hh, z0 = fuse_at(-4.6)
        fr = snap.frame((sx * 6, -4.4, z0 + hh * 0.55), (-sx, 0, 0))
        if fr is not None:
            box(bm, (0, 0, 0.01), (0.9, 0.16, 0.03), "window", mtx=fr @ Matrix.Rotation(math.radians(90), 4, "Z"))
    return bm


# ----------------------------------------------------------------- module kits

def kit_standard(bm, snap, rng):
    # single cannon in a chin fairing (GDD 6.1 Weapon 1)
    secs = [section(0.5, 0.35, 0.12, -9.3, z0=-0.82), section(0.95, 0.6, 0.2, -8.4, z0=-0.86),
            section(1.05, 0.66, 0.22, -5.2, z0=-0.86), section(0.8, 0.4, 0.15, -4.2, z0=-0.72)]
    pod, _, _ = grid_loft(secs, mat="dark")
    merge(bm, pod)
    cyl(bm, (0, -9.6, -0.86), 0.16, 0.9, "metal", axis="Y", segs=12)
    cyl(bm, (0, -10.1, -0.86), 0.2, 0.22, "dark", axis="Y", segs=12)
    box(bm, (0, -7.0, -1.2), (0.5, 1.6, 0.06), "warn")
    # radar mast
    box(bm, (0, 6.8, 2.0), (0.14, 0.4, 0.8), "dark")
    cyl(bm, (0, 6.8, 2.45), 0.5, 0.06, "hull", axis="Z", segs=20, r2=0.35)
    return {"Socket_Weapon_0": (0, -10.25, -0.86), "Socket_Scanner_0": (0, 6.8, 2.5)}


def kit_prospector(bm, snap, rng):
    # belly mining laser turret
    cyl(bm, (0, -1.6, -1.52), 0.95, 0.3, "dark", axis="Z", segs=24)
    cyl(bm, (0, -1.6, -1.72), 0.75, 0.25, "metal", axis="Z", segs=24)
    box(bm, (0, -2.2, -2.05), (0.95, 1.9, 0.6), "warn")
    box(bm, (0, -2.2, -2.36), (0.75, 1.6, 0.06), "dark")
    cyl(bm, (0, -3.55, -2.05), 0.3, 1.1, "dark", axis="Y", segs=14)
    cyl(bm, (0, -4.15, -2.05), 0.36, 0.14, "metal", axis="Y", segs=14)
    cyl(bm, (0, -4.24, -2.05), 0.22, 0.04, "laser", axis="Y", segs=14)
    # ore pods under the wings
    for sx in (1, -1):
        secs = [section(1.0, 0.9, 0.3, y, x0=sx * 3.1, z0=-0.95) for y in (1.4, 1.8, 5.4, 5.9)]
        secs[0] = section(0.6, 0.5, 0.18, 1.4, x0=sx * 3.1, z0=-0.95)
        secs[-1] = section(0.7, 0.6, 0.2, 5.9, x0=sx * 3.1, z0=-0.95)
        pod, _, _ = grid_loft(secs)
        merge(bm, pod)
        box(bm, (sx * 3.1, 3.6, -1.42), (0.6, 3.0, 0.06), "warn")
        for y in (2.6, 4.6):
            cyl(bm, (sx * 3.1, y, -0.95), 0.53, 0.1, "dark", axis="Y", segs=16)
    # sensor dish on the spine
    cyl(bm, (0, -0.6, 1.62), 0.6, 0.14, "metal", axis="Z", segs=20, r2=0.3)
    return {"Socket_Mining_0": (0, -4.3, -2.05), "Socket_Cargo_0": (3.1, 3.6, -0.95),
            "Socket_Cargo_1": (-3.1, 3.6, -0.95)}


def kit_courier(bm, snap, rng):
    # two dorsal cargo containers + long-range antenna
    for y in (2.0, 5.2):
        box(bm, (0, y, 2.2), (2.3, 2.9, 1.25), "accent")
        for k in (-1, 0, 1):
            box(bm, (0, y + k * 0.9, 2.2), (2.36, 0.08, 1.3), "dark")
        box(bm, (0, y, 2.85), (1.9, 2.4, 0.05), "dark")
        for sx in (1, -1):
            box(bm, (sx * 1.18, y - 0.9, 2.45), (0.03, 0.5, 0.3), "warn")
    box(bm, (0, 3.6, 1.62), (2.5, 6.4, 0.18), "metal")
    box(bm, (0, 7.6, 2.0), (0.14, 0.34, 1.4), "dark")
    cyl(bm, (0, 7.6, 3.2), 0.035, 1.6, "metal", axis="Z", segs=6)
    box(bm, (0, 7.6, 4.0), (0.1, 0.1, 0.1), "nav_w")
    return {"Socket_Cargo_0": (0, 2.0, 2.2), "Socket_Cargo_1": (0, 5.2, 2.2), "Socket_Scanner_0": (0, 7.6, 4.0)}


KITS = {"standard": kit_standard, "prospector": kit_prospector, "courier": kit_courier}

COMMON_SOCKETS = {
    "Socket_Engine_0": (4.6, 9.7, 0.0), "Socket_Engine_1": (-4.6, 9.7, 0.0), "Socket_Engine_2": (0, 9.9, 0.12),
    "Socket_Nav_L": (5.85, 0.8, 0.0), "Socket_Nav_R": (-5.85, 0.8, 0.0), "Socket_Nav_T": (0, 9.0, 1.2),
    "Socket_Cockpit": (0, -4.4, 1.3),
}


def build_variant(name, M, coll, low=False):
    global LOW
    LOW = low
    rng = random.Random(SEED)
    mats = [M[k] for k in ORDER]
    hull, secs = fuselage(rng)
    wl, wr = wing(1, rng), wing(-1, rng)
    nl, nr = nacelle(1, rng), nacelle(-1, rng)
    merge(hull, wl)
    merge(hull, wr)
    merge(hull, nl)
    merge(hull, nr)
    can, can_frame = canopy()
    # snapping target = everything built so far
    me = bpy.data.meshes.new("snaptmp")
    hull.to_mesh(me)
    snapbm = bmesh.new()
    snapbm.from_mesh(me)
    bpy.data.meshes.remove(me)
    snap = Snapper([snapbm])
    snapbm.free()

    hob = C.new_obj(f"{name}_hull", hull, mats, coll)
    if low:
        sockets = dict(COMMON_SOCKETS)
        g = bmesh.new()
        KITS[name](g, snap, rng)  # module kits stay: they carry the variant silhouette
        gob = C.new_obj(f"{name}_greeble", g, mats, coll)
        cob = C.new_obj(f"{name}_canopy", can, mats, coll)
        can_frame.free()
        parts = [hob, gob, cob]
        full = C.join(parts, f"ship_scout_{name}_LOD2")
        C.shade_smooth_by_angle(full, 30)
        C.uv_box(full, scale=1.0)
        LOW = False
        return full, sockets
    sol = hob.modifiers.new("Solid", "SOLIDIFY")
    sol.thickness = 0.04
    sol.offset = -1.0
    sol.use_even_offset = True
    C.add_bevel(hob, width=0.012, segments=1, angle=40)
    C.add_weighted_normals(hob)

    g = greebles(snap, rng)
    sockets = dict(COMMON_SOCKETS)
    sockets.update(KITS[name](g, snap, rng))
    gob = C.new_obj(f"{name}_greeble", g, mats, coll)
    C.add_bevel(gob, width=0.012, segments=1, angle=40)
    C.add_weighted_normals(gob)

    cob = C.new_obj(f"{name}_canopy", can, mats, coll)
    frame = C.new_obj(f"{name}_canopy_frame", can_frame, mats, coll)
    parts = [hob, gob, cob, frame]
    for p in parts:
        C.apply_modifiers(p)
    full = C.join(parts, f"ship_scout_{name}_LOD0")
    C.shade_smooth_by_angle(full, 30)
    C.uv_box(full, scale=1.0)
    return full, sockets


def make_lods(lod0, coll, name, M):
    """LOD1 = Decimate 0.3 of LOD0 (reproducible at this ratio - checked across runs with geo_digest);
    LOD2 = hand-built low hull (Decimate at ~0.08 breaks ties differently on every run)."""
    lods = [lod0]
    for i, ratio in ((1, 0.3),):
        cp = lod0.copy()
        cp.data = lod0.data.copy()
        cp.name = lod0.name.replace("_LOD0", f"_LOD{i}")
        coll.objects.link(cp)
        d = cp.modifiers.new("Dec", "DECIMATE")
        d.ratio = ratio
        d.use_collapse_triangulate = True
        C.apply_modifiers(cp)
        lods.append(cp)
    low, _ = build_variant(name, M, coll, low=True)
    lods.append(low)
    return lods


def set_livery(M, hexcol):
    nt = M["accent"].node_tree
    for n in nt.nodes:
        if n.type == "MIX":
            a, _, _ = C._mix_rgba_sockets(n)
            a.default_value = C.srgb(hexcol)


def main():
    C.reset_scene()
    C.setup_cycles(samples=192, res=(1920, 1080))
    sky = os.path.join(C.TEX, "skybox", "cradle_sky_equirect.png")
    C.world_space("#04060C", 1.0, hdri=sky if os.path.exists(sky) else None)
    C.studio_lights()
    M = materials()
    report = []
    roots = {}
    for name in KITS:
        coll = bpy.data.collections.new(name)
        bpy.context.scene.collection.children.link(coll)
        lod0, sockets = build_variant(name, M, coll)
        r = C.bounding_radius([lod0])
        assert r <= HULL_RADIUS_M, f"{name}: bounding radius {r:.2f} m > hull_radius_m {HULL_RADIUS_M}"
        lods = make_lods(lod0, coll, name, M)
        root = bpy.data.objects.new(f"ship_scout_{name}", None)
        coll.objects.link(root)
        for l in lods:
            l.parent = root
        for sname, loc in sockets.items():
            e = C.add_empty(sname, loc, parent=root, size=0.4)
            bpy.context.scene.collection.objects.unlink(e)
            coll.objects.link(e)
        for l in lods[1:]:
            l.hide_render = True
        report.append((name, round(r, 2), [C.tri_count(l) for l in lods]))
        roots[name] = root
        C.export_fbx(os.path.join(C.EXPORT, f"ship_scout_{name}.fbx"), [root])
    for line in report:
        print("SHIPREPORT", line)
    C.save_blend(os.path.join(C.SOURCE, "ship_scout.blend"))

    only = os.environ.get("SHIP_PREVIEW", "all")
    cam = C.add_camera((0, 0, 0), (0, 0, 0), lens=40)

    def show(names, lod=0):
        for n, root in roots.items():
            for ch in root.children:
                ch.hide_render = (n not in names) or not ch.name.endswith(f"_LOD{lod}")

    # 1) three variants in a row, 3/4 top view
    layout = {"courier": (-15.5, 4.0, 0), "standard": (0, 0, 0), "prospector": (15.5, -4.0, 0)}
    for n, p in layout.items():
        roots[n].location = p
    show(list(roots))
    cam.location = (-6.0, -42.0, 22.0)
    C.look_at(cam, (0, 0, -1.5))
    cam.data.lens = 38
    C.render(os.path.join(C.PREVIEWS, "ship_scout_variants.png"))
    if only == "variants":
        return
    for root in roots.values():
        root.location = (0, 0, 0)
    # 2) hero front 3/4 and rear 3/4 per variant
    for n in roots:
        show([n])
        cam.location = (13.5, -19.0, 6.0)
        C.look_at(cam, (0.0, -0.5, -0.3))
        cam.data.lens = 40
        C.render(os.path.join(C.PREVIEWS, f"ship_scout_{n}_front.png"))
        cam.location = (-12.5, 19.5, 7.5)
        C.look_at(cam, (0.0, 1.0, 0.0))
        C.render(os.path.join(C.PREVIEWS, f"ship_scout_{n}_rear.png"))
    show(["prospector"])
    star = bpy.data.objects["Star"]
    keep = tuple(star.rotation_euler)
    star.rotation_euler = (math.radians(140), 0, math.radians(120))  # light from below for this view only
    cam.location = (11.0, -15.0, -9.0)
    C.look_at(cam, (0.0, -0.5, -0.8))
    C.render(os.path.join(C.PREVIEWS, "ship_scout_prospector_belly.png"))
    star.rotation_euler = keep
    # 3) liveries: 6 standard hulls in a 3x2 grid (one shared material recoloured per render tile)
    show(["standard"])
    cam.location = (14.0, -22.0, 11.0)
    C.look_at(cam, (0.0, 0.0, -0.5))
    cam.data.lens = 45
    bpy.context.scene.render.resolution_x, bpy.context.scene.render.resolution_y = (960, 540)
    for lid, hexcol in LIVERIES:
        set_livery(M, hexcol)
        C.render(os.path.join(C.TEX, "_tmp", f"livery_{lid}.png"), samples=96)
    # 4) LOD comparison
    bpy.context.scene.render.resolution_x, bpy.context.scene.render.resolution_y = (960, 540)
    set_livery(M, LIVERIES[0][1])
    for lod in (0, 1, 2):
        show(["standard"], lod)
        C.render(os.path.join(C.TEX, "_tmp", f"lod{lod}.png"), samples=64)


if __name__ == "__main__":
    main()

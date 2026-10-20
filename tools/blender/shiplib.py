"""STARFALL DYNASTY ship-building helpers for headless Blender (5.2+).

Conventions (contract with the Bevy client):
  * nose -> Blender +Y, up -> +Z, meters.
  * Material names: Hull (baked textures), Livery, Glow, Glass, EngineCore.
  * Empties: Engine_N (scale = nozzle radius), Gun_N.
  * Rank parts: separate objects named Rank3_*, Rank6_*, Rank9_*.
"""
import bpy
import bmesh
import math
import os
import time
import numpy as np
from mathutils import Vector, Matrix

# material slot order used by every part
MAT_NAMES = ["Hull", "Livery", "Glow", "Glass", "EngineCore"]
H, L, G, GL, E = range(5)

V = Vector


def log(*a):
    print("[shiplib]", *a, flush=True)


# ---------------------------------------------------------------- scene

def reset_scene():
    bpy.ops.wm.read_factory_settings(use_empty=True)
    for coll in (bpy.data.meshes, bpy.data.materials, bpy.data.images,
                 bpy.data.objects, bpy.data.cameras, bpy.data.lights):
        for b in list(coll):
            coll.remove(b)
    sc = bpy.context.scene
    sc.unit_settings.system = 'METRIC'
    sc.unit_settings.scale_length = 1.0
    sc.render.engine = 'CYCLES'
    setup_gpu()


def setup_gpu():
    sc = bpy.context.scene
    try:
        prefs = bpy.context.preferences.addons['cycles'].preferences
        for t in ('OPTIX', 'CUDA', 'HIP', 'ONEAPI', 'METAL'):
            try:
                prefs.compute_device_type = t
                prefs.get_devices()
                gpus = [d for d in prefs.devices if d.type == t]
                if gpus:
                    for d in prefs.devices:
                        d.use = (d.type == t)
                    sc.cycles.device = 'GPU'
                    log("cycles device", t, [d.name for d in gpus])
                    return
            except Exception:
                continue
    except Exception as e:
        log("gpu setup failed", e)
    sc.cycles.device = 'CPU'
    log("cycles device CPU")


def set_active(obj):
    bpy.ops.object.select_all(action='DESELECT')
    obj.select_set(True)
    bpy.context.view_layer.objects.active = obj


# ---------------------------------------------------------------- bmesh geometry

def _is_point(r):
    return isinstance(r, Vector) or (len(r) == 3 and isinstance(r[0], (int, float)))


def loft(bm, rings, mat=H, matfn=None, cap_start=True, cap_end=True, closed=True):
    """Skin consecutive rings (lists of points, equal length) with quads.
    A ring may be a single point -> fan (pointed tip).
    matfn(i, k) -> material index for band i, segment k.
    Returns dict with 'bands' (list of face lists), 'start', 'end' caps."""
    vr = []
    for r in rings:
        if _is_point(r):
            vr.append([bm.verts.new(V(r))])
        else:
            vr.append([bm.verts.new(V(p)) for p in r])
    bands = []
    for i in range(len(vr) - 1):
        a, b = vr[i], vr[i + 1]
        fl = []
        if len(a) == 1 or len(b) == 1:
            ring, tip, rev = (b, a[0], False) if len(a) == 1 else (a, b[0], True)
            n = len(ring)
            segs = n if closed else n - 1
            for k in range(segs):
                k2 = (k + 1) % n
                f = bm.faces.new((ring[k], ring[k2], tip) if not rev else (ring[k2], ring[k], tip))
                f.material_index = matfn(i, k) if matfn else mat
                fl.append(f)
        else:
            n = len(a)
            segs = n if closed else n - 1
            for k in range(segs):
                k2 = (k + 1) % n
                f = bm.faces.new((a[k], a[k2], b[k2], b[k]))
                f.material_index = matfn(i, k) if matfn else mat
                fl.append(f)
        bands.append(fl)
    res = {'bands': bands, 'start': None, 'end': None, 'rings': vr}
    if closed and cap_start and len(vr[0]) > 2:
        f = bm.faces.new(list(reversed(vr[0])))
        f.material_index = mat
        res['start'] = f
    if closed and cap_end and len(vr[-1]) > 2:
        f = bm.faces.new(vr[-1])
        f.material_index = mat
        res['end'] = f
    return res


def ring10(y, hw, hh, cz=0.0, cx=0.0, top=0.55, bot=0.6, hi=0.35, lo=-0.35, ridge=0.0, keel=0.0):
    """10-point chamfered cross-section in the XZ plane at station y.
    Segment index symmetry: k and 9-k are mirrors.
    0/9 top inner, 1/8 upper chamfer, 2/7 side, 3/6 lower chamfer, 4/5 bottom."""
    pts = [
        (0, cz + hh + ridge),
        (hw * top, cz + hh),
        (hw, cz + hh * hi),
        (hw, cz + hh * lo),
        (hw * bot, cz - hh),
        (0, cz - hh - keel),
        (-hw * bot, cz - hh),
        (-hw, cz + hh * lo),
        (-hw, cz + hh * hi),
        (-hw * top, cz + hh),
    ]
    return [V((cx + x, y, z)) for x, z in pts]


def seg_sym(k, n=10):
    return min(k, n - 1 - k)


def circle_ring(center, axis, radius, n=24, rx=None, ry=None, phase=0.0):
    """Ring of n points around axis through center."""
    axis = V(axis).normalized()
    ref = V((0, 0, 1)) if abs(axis.z) < 0.9 else V((1, 0, 0))
    u = axis.cross(ref).normalized()
    v = axis.cross(u).normalized()
    rx = radius if rx is None else rx
    ry = radius if ry is None else ry
    c = V(center)
    return [c + u * (rx * math.cos(phase + 2 * math.pi * i / n)) + v * (ry * math.sin(phase + 2 * math.pi * i / n))
            for i in range(n)]


def lathe(bm, center, axis, profile, n=32, mats=None, mat=H, cap_start=True, cap_end=True, phase=None):
    """profile: list of (radius, t) along axis from center. r==0 -> point.
    mats: per-band material list (len(profile)-1)."""
    axis = V(axis).normalized()
    c = V(center)
    rings = []
    ph = (math.pi / n) if phase is None else phase
    for r, t in profile:
        if r <= 1e-6:
            rings.append(c + axis * t)
        else:
            rings.append(circle_ring(c + axis * t, axis, r, n, phase=ph))
    fn = (lambda i, k: mats[i]) if mats else None
    return loft(bm, rings, mat=mat, matfn=fn, cap_start=cap_start, cap_end=cap_end)


def box(bm, center, size, mat=H, rot=None, taper=None):
    """Axis-aligned box (optionally rotated by Matrix rot). taper=(sx,sz) scales the +Y face."""
    cx, cy, cz = center
    sx, sy, sz = [s / 2 for s in size]
    tx, tz = taper if taper else (1.0, 1.0)
    back = [V((-sx, -sy, -sz)), V((sx, -sy, -sz)), V((sx, -sy, sz)), V((-sx, -sy, sz))]
    front = [V((-sx * tx, sy, -sz * tz)), V((sx * tx, sy, -sz * tz)), V((sx * tx, sy, sz * tz)), V((-sx * tx, sy, sz * tz))]
    R = rot if rot is not None else Matrix.Identity(3)
    back = [R @ p + V(center) for p in back]
    front = [R @ p + V(center) for p in front]
    return loft(bm, [back, front], mat=mat)


def prism(bm, p0, p1, w0, h0, w1=None, h1=None, mat=H, chamfer=0.25, up=(0, 0, 1)):
    """Chamfered (octagonal) beam from p0 to p1."""
    w1 = w0 if w1 is None else w1
    h1 = h0 if h1 is None else h1
    p0, p1 = V(p0), V(p1)
    ax = (p1 - p0).normalized()
    upv = V(up)
    if abs(ax.dot(upv)) > 0.95:
        upv = V((1, 0, 0)) if abs(ax.x) < 0.9 else V((0, 1, 0))
    right = ax.cross(upv).normalized()
    upv = right.cross(ax).normalized()

    def rg(p, w, h):
        c = chamfer
        pts = [(w * (1 - c), h), (w, h * (1 - c)), (w, -h * (1 - c)), (w * (1 - c), -h),
               (-w * (1 - c), -h), (-w, -h * (1 - c)), (-w, h * (1 - c)), (-w * (1 - c), h)]
        return [p + right * x + upv * z for x, z in pts]
    return loft(bm, [rg(p0, w0, h0), rg(p1, w1, h1)], mat=mat)


def cyl(bm, p0, p1, r0, r1=None, n=16, mat=H, cap=True):
    r1 = r0 if r1 is None else r1
    p0, p1 = V(p0), V(p1)
    ax = (p1 - p0)
    rings = [circle_ring(p0, ax, r0, n), circle_ring(p1, ax, r1, n) if r1 > 1e-6 else p1]
    return loft(bm, rings, mat=mat, cap_start=cap, cap_end=cap)


def sphere(bm, center, radius, mat=H, seg=12, rings=6, scale=(1, 1, 1)):
    m = Matrix.Diagonal((*scale, 1.0))
    ret = bmesh.ops.create_uvsphere(bm, u_segments=seg, v_segments=rings, radius=radius,
                                    matrix=Matrix.Translation(V(center)) @ m)
    for v in ret['verts']:
        for f in v.link_faces:
            f.material_index = mat
    return ret


def wing(bm, stations, mat=H, matfn=None, flat_te=0.15):
    """stations: list of dict(x, z, le, te, t) along the span (right side).
    Hexagonal airfoil: LE edge, upper/lower plateau, thin trailing edge.
    Thickness is applied perpendicular to the span direction in XZ, so the
    same helper builds canted / vertical fins."""
    s0, s1 = stations[0], stations[-1]
    sd = V((s1['x'] - s0['x'], s1['z'] - s0['z']))
    sd.normalize()
    nx, nz = -sd.y, sd.x
    if nz < 0:
        nx, nz = -nx, -nz
    rings = []
    for s in stations:
        x, z, le, te, t = s['x'], s['z'], s['le'], s['te'], s['t']
        ch = le - te
        tt = t * flat_te
        pts = [
            (le, 0.0),
            (le - ch * 0.22, t * 0.5),
            (le - ch * 0.72, t * 0.42),
            (te, tt * 0.5),
            (te, -tt * 0.5),
            (le - ch * 0.72, -t * 0.36),
            (le - ch * 0.22, -t * 0.45),
        ]
        rings.append([V((x + nx * dz, y, z + nz * dz)) for y, dz in pts])
    return loft(bm, rings, mat=mat, matfn=matfn)


def inset_faces(bm, faces, thickness=0.05, depth=-0.02, mat=None):
    faces = [f for f in faces if f.is_valid]
    if not faces:
        return []
    ret = bmesh.ops.inset_individual(bm, faces=faces, thickness=thickness, depth=depth,
                                     use_even_offset=True, use_relative_offset=False)
    if mat is not None:
        for f in faces:
            if f.is_valid:
                f.material_index = mat
    return faces


def extrude_face(bm, face, dist, scale=1.0, mat=None):
    """Extrude a face along its normal; returns the new cap face."""
    r = bmesh.ops.extrude_discrete_faces(bm, faces=[face])
    nf = r['faces'][0]
    n = nf.normal.copy()
    c = nf.calc_center_median()
    for v in nf.verts:
        v.co = c + (v.co - c) * scale + n * dist
    if mat is not None:
        nf.material_index = mat
    return nf


def transform_verts(bm, verts, M):
    bmesh.ops.transform(bm, matrix=M, verts=list(verts))


def mirror_bm_x(bm):
    """Duplicate all geometry mirrored in X (for parts not touching x=0)."""
    geom = list(bm.verts) + list(bm.edges) + list(bm.faces)
    d = bmesh.ops.duplicate(bm, geom=geom)
    nv = [g for g in d['geom'] if isinstance(g, bmesh.types.BMVert)]
    bmesh.ops.scale(bm, vec=V((-1, 1, 1)), verts=nv)
    nf = [g for g in d['geom'] if isinstance(g, bmesh.types.BMFace)]
    bmesh.ops.reverse_faces(bm, faces=nf)


# ---------------------------------------------------------------- objects / parts

class Ship:
    def __init__(self, name, mats):
        self.name = name
        self.mats = mats  # dict name->material
        self.groups = {}  # group name -> [objects]
        self.empties = []
        self.coll = bpy.context.scene.collection

    def part(self, build, group="body", bevel=0.03, segs=2, angle=32, mirror=False,
             subsurf=0, harden=True, weighted=True, name=None, sharp_all=False, recalc=True):
        """build(bm) fills a bmesh. Returns finished object (modifiers applied)."""
        bm = bmesh.new()
        build(bm)
        if mirror:
            mirror_bm_x(bm)
        bmesh.ops.remove_doubles(bm, verts=bm.verts, dist=1e-5)
        if recalc:
            bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
        me = bpy.data.meshes.new(name or (group + "_part"))
        bm.to_mesh(me)
        bm.free()
        me.uv_layers.new(name="UVMap")
        obj = bpy.data.objects.new(name or (group + "_part"), me)
        self.coll.objects.link(obj)
        for mn in MAT_NAMES:
            me.materials.append(self.mats[mn])
        set_active(obj)
        if subsurf:
            m = obj.modifiers.new("sub", 'SUBSURF')
            m.levels = subsurf
            m.render_levels = subsurf
            bpy.ops.object.modifier_apply(modifier="sub")
        if sharp_all:
            bpy.ops.object.shade_flat()
        else:
            bpy.ops.object.shade_smooth_by_angle(angle=math.radians(angle), keep_sharp_edges=True)
        if bevel > 0:
            m = obj.modifiers.new("bevel", 'BEVEL')
            m.width = bevel
            m.segments = segs
            m.limit_method = 'ANGLE'
            m.angle_limit = math.radians(angle)
            m.harden_normals = harden
            m.use_clamp_overlap = True
            m.miter_outer = 'MITER_ARC'
            bpy.ops.object.modifier_apply(modifier="bevel")
        if weighted and not sharp_all:
            m = obj.modifiers.new("wn", 'WEIGHTED_NORMAL')
            m.keep_sharp = True
            m.weight = 50
            bpy.ops.object.modifier_apply(modifier="wn")
        self.groups.setdefault(group, []).append(obj)
        return obj

    def empty(self, name, loc, radius=0.25):
        e = bpy.data.objects.new(name, None)
        e.empty_display_type = 'SPHERE'
        e.empty_display_size = 1.0
        e.location = V(loc)
        e.scale = (radius, radius, radius)
        self.coll.objects.link(e)
        self.empties.append(e)
        return e

    def finalize(self):
        """Join groups into objects; body -> 'Hull'. Center origin at bbox center."""
        result = {}
        for gname, objs in self.groups.items():
            objs = [o for o in objs if o.name in bpy.data.objects]
            bpy.ops.object.select_all(action='DESELECT')
            for o in objs:
                o.select_set(True)
            bpy.context.view_layer.objects.active = objs[0]
            if len(objs) > 1:
                bpy.ops.object.join()
            o = bpy.context.view_layer.objects.active
            o.name = "Hull" if gname == "body" else gname
            o.data.name = o.name
            bpy.ops.object.material_slot_remove_unused()
            result[o.name] = o
        self.objects = result
        # center
        lo = V((1e9, 1e9, 1e9))
        hi = V((-1e9, -1e9, -1e9))
        for o in result.values():
            for v in o.data.vertices:
                p = o.matrix_world @ v.co
                lo = V((min(lo.x, p.x), min(lo.y, p.y), min(lo.z, p.z)))
                hi = V((max(hi.x, p.x), max(hi.y, p.y), max(hi.z, p.z)))
        c = (lo + hi) / 2
        T = Matrix.Translation(-c)
        for o in result.values():
            o.data.transform(T)
            o.data.update()
        for e in self.empties:
            e.location = e.location - c
        self.bbox = (lo - c, hi - c)
        log(self.name, "bbox size", tuple(round(x, 2) for x in (hi - lo)))
        return result

    def tri_count(self):
        n = 0
        per = {}
        for o in self.objects.values():
            o.data.calc_loop_triangles()
            per[o.name] = len(o.data.loop_triangles)
            n += per[o.name]
        return n, per


# ---------------------------------------------------------------- materials

class NB:
    """tiny node-builder"""

    def __init__(self, mat):
        self.nt = mat.node_tree
        self.n = self.nt.nodes
        self.l = self.nt.links
        self.x = 0

    def node(self, t, **kw):
        nd = self.n.new(t)
        for k, v in kw.items():
            setattr(nd, k, v)
        nd.location = (self.x, 0)
        self.x += 30
        return nd

    def link(self, a, b):
        self.l.new(a, b)

    def val(self, s, slot):
        """s: socket or number -> set into input slot"""
        if isinstance(s, (int, float)):
            slot.default_value = s
        elif isinstance(s, (tuple, list)):
            slot.default_value = s
        else:
            self.link(s, slot)

    def math(self, op, a, b=None, clamp=False):
        nd = self.node('ShaderNodeMath', operation=op)
        nd.use_clamp = clamp
        self.val(a, nd.inputs[0])
        if b is not None:
            self.val(b, nd.inputs[1])
        return nd.outputs[0]

    def mrange(self, v, a, b, c=0.0, d=1.0, clamp=True, smooth=False):
        nd = self.node('ShaderNodeMapRange')
        nd.clamp = clamp
        if smooth:
            nd.interpolation_type = 'SMOOTHSTEP'
        self.val(v, nd.inputs['Value'])
        nd.inputs['From Min'].default_value = a
        nd.inputs['From Max'].default_value = b
        nd.inputs['To Min'].default_value = c
        nd.inputs['To Max'].default_value = d
        return nd.outputs['Result']

    def mix(self, fac, a, b):
        nd = self.node('ShaderNodeMix', data_type='RGBA')
        self.val(fac, nd.inputs[0])
        self.val(a, nd.inputs[6])
        self.val(b, nd.inputs[7])
        return nd.outputs[2]

    def mixf(self, fac, a, b):
        nd = self.node('ShaderNodeMix', data_type='FLOAT')
        self.val(fac, nd.inputs[0])
        self.val(a, nd.inputs[2])
        self.val(b, nd.inputs[3])
        return nd.outputs[0]

    def vmath(self, op, a, b=None):
        nd = self.node('ShaderNodeVectorMath', operation=op)
        self.val(a, nd.inputs[0])
        if b is not None:
            self.val(b, nd.inputs[1])
        return nd.outputs[0] if op not in ('DOT_PRODUCT', 'LENGTH', 'DISTANCE') else nd.outputs['Value']


def _new_mat(name):
    m = bpy.data.materials.new(name)
    try:
        m.use_nodes = True
    except Exception:
        pass
    return m


def make_flat_materials():
    mats = {}
    m = _new_mat("Livery")
    p = m.node_tree.nodes["Principled BSDF"]
    p.inputs['Base Color'].default_value = (0.8, 0.8, 0.8, 1)
    p.inputs['Metallic'].default_value = 0.3
    p.inputs['Roughness'].default_value = 0.4
    mats["Livery"] = m

    m = _new_mat("Glow")
    p = m.node_tree.nodes["Principled BSDF"]
    p.inputs['Base Color'].default_value = (0, 0, 0, 1)
    p.inputs['Metallic'].default_value = 0.0
    p.inputs['Roughness'].default_value = 0.5
    p.inputs['Emission Color'].default_value = (1, 1, 1, 1)
    p.inputs['Emission Strength'].default_value = 5.0
    mats["Glow"] = m

    m = _new_mat("Glass")
    p = m.node_tree.nodes["Principled BSDF"]
    p.inputs['Base Color'].default_value = (0.015, 0.03, 0.045, 1)
    p.inputs['Metallic'].default_value = 0.5
    p.inputs['Roughness'].default_value = 0.05
    mats["Glass"] = m

    m = _new_mat("EngineCore")
    p = m.node_tree.nodes["Principled BSDF"]
    p.inputs['Base Color'].default_value = (0, 0, 0, 1)
    p.inputs['Metallic'].default_value = 0.0
    p.inputs['Roughness'].default_value = 0.5
    p.inputs['Emission Color'].default_value = (1.0, 0.62, 0.32, 1)
    p.inputs['Emission Strength'].default_value = 5.0
    mats["EngineCore"] = m
    return mats


def make_hull_procedural(style):
    """Hull material driven by procedural nodes (bake source).
    Returns (material, sockets dict: base, rough, metal, normal)."""
    m = _new_mat("Hull")
    nb = NB(m)
    nt = m.node_tree
    for n in list(nt.nodes):
        nt.nodes.remove(n)
    out = nb.node('ShaderNodeOutputMaterial')
    bsdf = nb.node('ShaderNodeBsdfPrincipled')
    tc = nb.node('ShaderNodeTexCoord')
    pos = tc.outputs['Object']
    geo = nb.node('ShaderNodeNewGeometry')
    nrm = geo.outputs['Normal']
    sx_ = nb.node('ShaderNodeSeparateXYZ')
    nb.link(pos, sx_.inputs[0])
    X, Y, Z = sx_.outputs
    sn = nb.node('ShaderNodeSeparateXYZ')
    nb.link(nrm, sn.inputs[0])
    NX, NY, NZ = sn.outputs

    paint = style.get('paint', (0.32, 0.34, 0.36))
    paint2 = style.get('paint2', tuple(c * 0.8 for c in paint))
    seam_w = style.get('seam_w', 0.022)
    sx, sy, sz = style.get('spacing', (1.3, 1.7, 0.9))

    organic = style.get('organic', False)

    def axis_lines(c, s, ncomp, stagger_src=None, stagger_s=None, k=0.37):
        cc = c
        if stagger_src is not None:
            cell = nb.math('FLOOR', nb.math('DIVIDE', stagger_src, stagger_s))
            cc = nb.math('ADD', c, nb.math('MULTIPLY', cell, s * k))
        t = nb.math('DIVIDE', cc, s)
        d = nb.math('ABSOLUTE', nb.math('SUBTRACT', nb.math('FRACT', nb.math('ADD', t, 0.5)), 0.5))
        d = nb.math('MULTIPLY', d, s)
        line = nb.mrange(d, seam_w * 0.35, seam_w, 1.0, 0.0, smooth=True)
        mask = nb.mrange(nb.math('ABSOLUTE', ncomp), 0.55, 0.85, 1.0, 0.0)
        cellid = nb.math('FLOOR', t)
        return nb.math('MULTIPLY', line, mask), cellid

    if not organic:
        ly, cy = axis_lines(Y, sy, NY, X, sx, 0.31)
        lx, cx = axis_lines(X, sx, NX, Y, sy, 0.37)
        lz, cz = axis_lines(Z, sz, NZ, Y, sy, 0.41)
        seams = nb.math('MAXIMUM', nb.math('MAXIMUM', ly, lx), lz)
        # secondary irregular sub-panels
        vor = nb.node('ShaderNodeTexVoronoi', voronoi_dimensions='3D', feature='DISTANCE_TO_EDGE', distance='CHEBYCHEV')
        nb.link(pos, vor.inputs['Vector'])
        vor.inputs['Scale'].default_value = style.get('sub_scale', 0.32)
        vor.inputs['Randomness'].default_value = 0.85
        vline = nb.mrange(vor.outputs['Distance'], seam_w * 0.25 * 0.55, seam_w * 0.55 * 0.9, 1.0, 0.0, smooth=True)
        vcol = nb.node('ShaderNodeTexVoronoi', voronoi_dimensions='3D', feature='F1', distance='CHEBYCHEV')
        nb.link(pos, vcol.inputs['Vector'])
        vcol.inputs['Scale'].default_value = style.get('sub_scale', 0.32)
        vcol.inputs['Randomness'].default_value = 0.85
        sep = nb.node('ShaderNodeSeparateColor')
        nb.link(vcol.outputs['Color'], sep.inputs[0])
        seams = nb.math('MAXIMUM', seams, nb.math('MULTIPLY', vline, style.get('sub_amt', 0.6)))
        # panel id -> variation
        cid = nb.node('ShaderNodeCombineXYZ')
        nb.link(cx, cid.inputs[0])
        nb.link(cy, cid.inputs[1])
        nb.link(cz, cid.inputs[2])
        wn = nb.node('ShaderNodeTexWhiteNoise', noise_dimensions='3D')
        nb.link(cid.outputs[0], wn.inputs['Vector'])
        pvar = wn.outputs['Value']
        pvar = nb.math('ADD', nb.math('MULTIPLY', pvar, 0.6), nb.math('MULTIPLY', sep.outputs[1], 0.4))
        # hazard / accent panels: a few panels get paint2
        accent = nb.math('GREATER_THAN', pvar, style.get('accent_thresh', 0.86))
        base = nb.mix(nb.math('MULTIPLY', pvar, 0.35), (*paint, 1), (*paint2, 1))
        base = nb.mix(accent, base, (*style.get('accent', paint2), 1))
        height = nb.math('SUBTRACT', nb.math('MULTIPLY', pvar, 0.35), nb.math('MULTIPLY', seams, 1.0))
    else:
        # Veyr bio-shell: scales + ribs + vein grooves
        vor = nb.node('ShaderNodeTexVoronoi', voronoi_dimensions='3D', feature='SMOOTH_F1', distance='EUCLIDEAN')
        nb.link(pos, vor.inputs['Vector'])
        vor.inputs['Scale'].default_value = 1.6
        vor.inputs['Smoothness'].default_value = 0.6
        scales = nb.mrange(vor.outputs['Distance'], 0.0, 0.55, 1.0, 0.0, smooth=True)
        rib = nb.math('SINE', nb.math('MULTIPLY', Y, 2 * math.pi / 0.55))
        rib = nb.mrange(rib, 0.2, 1.0, 0.0, 1.0, smooth=True)
        rmask = nb.mrange(nb.math('ABSOLUTE', NY), 0.6, 0.9, 1.0, 0.0)
        rib = nb.math('MULTIPLY', rib, rmask)
        noise = nb.node('ShaderNodeTexNoise')
        nb.link(pos, noise.inputs['Vector'])
        noise.inputs['Scale'].default_value = 0.6
        noise.inputs['Detail'].default_value = 4
        veinv = nb.node('ShaderNodeTexVoronoi', voronoi_dimensions='3D', feature='DISTANCE_TO_EDGE')
        nb.link(nb.vmath('ADD', pos, noise.outputs['Color']), veinv.inputs['Vector'])
        veinv.inputs['Scale'].default_value = 0.45
        seams = nb.mrange(veinv.outputs['Distance'], 0.01, 0.04, 1.0, 0.0, smooth=True)
        pvar = noise.outputs['Fac']
        base = nb.mix(nb.mrange(pvar, 0.35, 0.65), (*paint, 1), (*paint2, 1))
        base = nb.mix(nb.math('MULTIPLY', scales, 0.25), base, (*style.get('accent', paint2), 1))
        height = nb.math('ADD', nb.math('MULTIPLY', scales, 0.4), nb.math('MULTIPLY', rib, 0.5))
        height = nb.math('SUBTRACT', height, nb.math('MULTIPLY', seams, 1.2))

    # grime: AO + streaks
    ao = nb.node('ShaderNodeAmbientOcclusion')
    ao.samples = 16
    ao.inputs['Distance'].default_value = style.get('ao_dist', 0.8)
    streak = nb.node('ShaderNodeTexNoise')
    smap = nb.node('ShaderNodeMapping')
    nb.link(pos, smap.inputs['Vector'])
    smap.inputs['Scale'].default_value = (3.0, 0.25, 3.0)
    nb.link(smap.outputs[0], streak.inputs['Vector'])
    streak.inputs['Scale'].default_value = 2.0
    streak.inputs['Detail'].default_value = 6
    blot = nb.node('ShaderNodeTexNoise')
    nb.link(pos, blot.inputs['Vector'])
    blot.inputs['Scale'].default_value = 0.9
    blot.inputs['Detail'].default_value = 8
    grime = nb.math('ADD', nb.mrange(ao.outputs['AO'], 0.35, 1.0, 1.0, 0.0),
                    nb.mrange(streak.outputs['Fac'], 0.5, 0.75, 0.0, 0.45))
    grime = nb.math('ADD', grime, nb.mrange(blot.outputs['Fac'], 0.55, 0.8, 0.0, 0.15))
    grime = nb.math('MULTIPLY', grime, style.get('grime', 1.0), clamp=True)
    # edge wear
    bev = nb.node('ShaderNodeBevel')
    bev.samples = 8
    bev.inputs['Radius'].default_value = 0.035
    edge = nb.vmath('DOT_PRODUCT', bev.outputs[0], geo.outputs['True Normal'])
    edge = nb.mrange(edge, 0.985, 0.85, 0.0, 1.0)
    wnoise = nb.node('ShaderNodeTexNoise')
    nb.link(pos, wnoise.inputs['Vector'])
    wnoise.inputs['Scale'].default_value = 7.0
    wnoise.inputs['Detail'].default_value = 8
    wear = nb.math('MULTIPLY', edge, nb.mrange(wnoise.outputs['Fac'], 0.4, 0.6), clamp=True)
    wear = nb.math('MULTIPLY', wear, style.get('wear', 1.0), clamp=True)

    grime_col = style.get('grime_col', (0.05, 0.045, 0.04))
    col = nb.mix(nb.math('MULTIPLY', grime, 0.4), base, (*grime_col, 1))
    col = nb.mix(nb.math('MULTIPLY', seams, 0.75), col, (*[c * 0.25 for c in paint], 1))
    col = nb.mix(wear, col, (*style.get('bare', (0.55, 0.55, 0.56)), 1))

    r0 = style.get('rough', 0.5)
    rough = nb.math('ADD', r0, nb.math('MULTIPLY', nb.math('SUBTRACT', pvar, 0.5), 0.18))
    rough = nb.math('ADD', rough, nb.math('MULTIPLY', grime, 0.08))
    rough = nb.math('ADD', rough, nb.math('MULTIPLY', seams, 0.2))
    rough = nb.mixf(wear, rough, 0.28)
    rough = nb.math('MINIMUM', nb.math('MAXIMUM', rough, 0.05), 1.0)
    metal = nb.mixf(wear, style.get('metal', 0.35), 1.0)
    metal = nb.mixf(nb.math('MULTIPLY', grime, 0.4), metal, 0.1)

    # fine surface noise for normal
    fine = nb.node('ShaderNodeTexNoise')
    nb.link(pos, fine.inputs['Vector'])
    fine.inputs['Scale'].default_value = 18.0
    fine.inputs['Detail'].default_value = 4
    height = nb.math('ADD', height, nb.math('MULTIPLY', fine.outputs['Fac'], 0.06))
    bump = nb.node('ShaderNodeBump')
    bump.inputs['Strength'].default_value = style.get('bump', 0.9)
    bump.inputs['Distance'].default_value = 0.012
    nb.link(height, bump.inputs['Height'])

    nb.link(col, bsdf.inputs['Base Color'])
    nb.link(rough, bsdf.inputs['Roughness'])
    nb.link(metal, bsdf.inputs['Metallic'])
    nb.link(bump.outputs['Normal'], bsdf.inputs['Normal'])
    nb.link(bsdf.outputs[0], out.inputs['Surface'])
    em = nb.node('ShaderNodeEmission')
    socks = {'base': col, 'rough': rough, 'metal': metal, 'normal': bump.outputs['Normal'],
             'bsdf': bsdf, 'out': out, 'emit': em}
    return m, socks


def make_materials(style):
    mats = make_flat_materials()
    hull, socks = make_hull_procedural(style)
    mats["Hull"] = hull
    return mats, socks


# ---------------------------------------------------------------- UV + bake

def unwrap_hull(objs, margin=0.0035):
    bpy.ops.object.select_all(action='DESELECT')
    for o in objs:
        o.select_set(True)
    bpy.context.view_layer.objects.active = objs[0]
    for o in objs:
        me = o.data
        idx = [i for i, s in enumerate(o.material_slots) if s.material and s.material.name == "Hull"]
        sel = [p.material_index in idx for p in me.polygons]
        for v in me.vertices:
            v.select = False
        for e in me.edges:
            e.select = False
        for p, s in zip(me.polygons, sel):
            p.select = s
            if s:
                for vi in p.vertices:
                    me.vertices[vi].select = True
        me.update()
    bpy.ops.object.mode_set(mode='EDIT')
    bpy.ops.mesh.select_mode(type='FACE')
    t = time.time()
    r = bpy.ops.uv.smart_project(angle_limit=math.radians(55), island_margin=margin,
                                 area_weight=0.0, correct_aspect=True, scale_to_bounds=False)
    log("smart_project", r)
    bpy.ops.object.mode_set(mode='OBJECT')
    log("unwrap done %.1fs" % (time.time() - t))
    for o in objs:
        me = o.data
        uv = me.uv_layers.active.data
        idx = [i for i, s in enumerate(o.material_slots) if s.material and s.material.name == "Hull"]
        area = 0.0
        n = 0
        for p in me.polygons:
            if p.material_index not in idx:
                continue
            n += 1
            ls = list(p.loop_indices)
            a = uv[ls[0]].uv
            for j in range(1, len(ls) - 1):
                b, c = uv[ls[j]].uv, uv[ls[j + 1]].uv
                area += abs((b - a).cross(c - a)) / 2
        log("uv", o.name, "layers", [u.name for u in me.uv_layers], "hull faces", n, "uv area %.3f" % area)


def _img(name, size, noncolor):
    img = bpy.data.images.new(name, size, size, alpha=False, float_buffer=False)
    img.colorspace_settings.name = 'Non-Color' if noncolor else 'sRGB'
    return img


def bake_hull(objs, mats, socks, outdir, prefix, size=2048, samples=48):
    os.makedirs(outdir, exist_ok=True)
    sc = bpy.context.scene
    sc.render.engine = 'CYCLES'
    sc.cycles.samples = samples
    sc.render.bake.margin = 16
    sc.render.bake.use_clear = True
    hull = mats["Hull"]
    nt = hull.node_tree
    tex = nt.nodes.new('ShaderNodeTexImage')
    # dummy targets for other materials
    dummy = bpy.data.images.new("_dummy", 16, 16)
    tmp_nodes = []
    for mn in MAT_NAMES:
        if mn == "Hull":
            continue
        mt = mats[mn].node_tree
        nd = mt.nodes.new('ShaderNodeTexImage')
        nd.image = dummy
        mt.nodes.active = nd
        tmp_nodes.append((mt, nd))

    bpy.ops.object.select_all(action='DESELECT')
    for o in objs:
        o.select_set(True)
    bpy.context.view_layer.objects.active = objs[0]

    out, bsdf, em = socks['out'], socks['bsdf'], socks['emit']
    imgs = {}

    def bake_emit(key, sock, noncolor):
        img = _img(prefix + "_" + key, size, noncolor)
        tex.image = img
        nt.nodes.active = tex
        for lk in list(out.inputs['Surface'].links):
            nt.links.remove(lk)
        nt.links.new(sock, em.inputs['Color'])
        nt.links.new(em.outputs[0], out.inputs['Surface'])
        t = time.time()
        bpy.ops.object.bake(type='EMIT', margin=16, use_clear=True, target='IMAGE_TEXTURES')
        log("baked", key, "%.1fs" % (time.time() - t))
        imgs[key] = img

    bake_emit('base', socks['base'], False)
    bake_emit('rough', socks['rough'], True)
    bake_emit('metal', socks['metal'], True)
    # normal
    img = _img(prefix + "_normal", size, True)
    tex.image = img
    nt.nodes.active = tex
    for lk in list(out.inputs['Surface'].links):
        nt.links.remove(lk)
    nt.links.new(bsdf.outputs[0], out.inputs['Surface'])
    t = time.time()
    sc.cycles.samples = max(8, samples // 2)
    bpy.ops.object.bake(type='NORMAL', normal_space='TANGENT', margin=16, use_clear=True, target='IMAGE_TEXTURES')
    log("baked normal %.1fs" % (time.time() - t))
    imgs['normal'] = img

    # pack metallic-roughness: G=rough, B=metal
    n = size * size * 4
    r = np.empty(n, dtype=np.float32)
    mt_ = np.empty(n, dtype=np.float32)
    imgs['rough'].pixels.foreach_get(r)
    imgs['metal'].pixels.foreach_get(mt_)
    mr = np.ones(n, dtype=np.float32)
    mr[1::4] = r[0::4]
    mr[2::4] = mt_[0::4]
    mrimg = _img(prefix + "_mr", size, True)
    mrimg.pixels.foreach_set(mr)

    paths = {}
    for key, im in (('base', imgs['base']), ('mr', mrimg), ('normal', imgs['normal'])):
        p = os.path.join(outdir, "%s_%s.png" % (prefix, key))
        im.filepath_raw = p
        im.file_format = 'PNG'
        im.save()
        paths[key] = p
    # cleanup temp
    for mt, nd in tmp_nodes:
        mt.nodes.remove(nd)
    bpy.data.images.remove(dummy)
    for k in ('rough', 'metal'):
        bpy.data.images.remove(imgs[k])
    for im in (imgs['base'], mrimg, imgs['normal']):
        bpy.data.images.remove(im)
    nt.nodes.remove(tex)
    return paths


def rebuild_hull_textured(hull_mat, paths):
    nt = hull_mat.node_tree
    for n in list(nt.nodes):
        nt.nodes.remove(n)
    out = nt.nodes.new('ShaderNodeOutputMaterial')
    out.location = (600, 0)
    bsdf = nt.nodes.new('ShaderNodeBsdfPrincipled')
    bsdf.location = (300, 0)
    nt.links.new(bsdf.outputs[0], out.inputs['Surface'])

    def load(p, noncolor):
        im = bpy.data.images.load(p)
        im.colorspace_settings.name = 'Non-Color' if noncolor else 'sRGB'
        return im
    tb = nt.nodes.new('ShaderNodeTexImage')
    tb.image = load(paths['base'], False)
    tb.location = (-400, 300)
    nt.links.new(tb.outputs['Color'], bsdf.inputs['Base Color'])
    tm = nt.nodes.new('ShaderNodeTexImage')
    tm.image = load(paths['mr'], True)
    tm.location = (-400, 0)
    sep = nt.nodes.new('ShaderNodeSeparateColor')
    sep.location = (-100, 0)
    nt.links.new(tm.outputs['Color'], sep.inputs[0])
    nt.links.new(sep.outputs[1], bsdf.inputs['Roughness'])
    nt.links.new(sep.outputs[2], bsdf.inputs['Metallic'])
    tn = nt.nodes.new('ShaderNodeTexImage')
    tn.image = load(paths['normal'], True)
    tn.location = (-400, -300)
    nm = nt.nodes.new('ShaderNodeNormalMap')
    nm.location = (-100, -300)
    nt.links.new(tn.outputs['Color'], nm.inputs['Color'])
    nt.links.new(nm.outputs[0], bsdf.inputs['Normal'])


# ---------------------------------------------------------------- export

def export_glb(path, objs, empties):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    bpy.ops.object.select_all(action='DESELECT')
    for o in list(objs) + list(empties):
        o.select_set(True)
    bpy.context.view_layer.objects.active = list(objs)[0]
    bpy.ops.export_scene.gltf(
        filepath=path, export_format='GLB', use_selection=True, export_yup=True,
        export_apply=True, export_materials='EXPORT', export_image_format='AUTO',
        export_cameras=False, export_lights=False, export_animations=False,
        export_extras=False, export_texcoords=True, export_normals=True, export_tangents=False)
    log("exported", path, "%.2f MB" % (os.path.getsize(path) / 1e6))
    gpu_textures(path)


def gpu_textures(path):
    """PNG maps -> KTX2 (BC7/BC5, mips, zstd) and stored tangents, in place: crates/assetpipe,
    docs/STUDIO_PIPELINE.md. The asset budgets fail a model that skips this (its VRAM is ~3x over)."""
    import fnmatch, json, subprocess
    repo = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
    # Mesh LODs for the models whose budget rule asks for one (tools/asset_budgets.json "lod").
    args = [path]
    with open(os.path.join(repo, "tools", "asset_budgets.json"), encoding="utf-8") as f:
        rule = next((r for r in json.load(f)["rules"] if fnmatch.fnmatch(os.path.basename(path), r["match"])), {})
    if "lod" in rule:
        args += ["--lod", str(rule["lod"])]
    try:
        r = subprocess.run(["cargo", "run", "-q", "-p", "assetpipe", "--release", "--", *args], cwd=repo, capture_output=True, text=True)
        log((r.stdout.strip().splitlines() or ["assetpipe: no output"])[-1] if r.returncode == 0 else "assetpipe FAILED: " + r.stderr[-500:])
    except OSError as e:
        log("assetpipe not run (%s): run `cargo run -p assetpipe --release -- %s`" % (e, path))


# ---------------------------------------------------------------- preview

def setup_world():
    w = bpy.data.worlds.new("Space")
    bpy.context.scene.world = w
    try:
        w.use_nodes = True
    except Exception:
        pass
    nt = w.node_tree
    for n in list(nt.nodes):
        nt.nodes.remove(n)
    out = nt.nodes.new('ShaderNodeOutputWorld')
    tc = nt.nodes.new('ShaderNodeTexCoord')
    vor = nt.nodes.new('ShaderNodeTexVoronoi')
    vor.voronoi_dimensions = '3D'
    vor.inputs['Scale'].default_value = 260
    nt.links.new(tc.outputs['Generated'], vor.inputs['Vector'])
    sep = nt.nodes.new('ShaderNodeSeparateColor')
    nt.links.new(vor.outputs['Color'], sep.inputs[0])
    mr = nt.nodes.new('ShaderNodeMapRange')
    mr.inputs['From Min'].default_value = 0.0
    mr.inputs['From Max'].default_value = 0.06
    mr.inputs['To Min'].default_value = 1.0
    mr.inputs['To Max'].default_value = 0.0
    nt.links.new(vor.outputs['Distance'], mr.inputs['Value'])
    gt = nt.nodes.new('ShaderNodeMath')
    gt.operation = 'GREATER_THAN'
    gt.inputs[1].default_value = 0.82
    nt.links.new(sep.outputs[0], gt.inputs[0])
    st = nt.nodes.new('ShaderNodeMath')
    st.operation = 'MULTIPLY'
    nt.links.new(mr.outputs[0], st.inputs[0])
    nt.links.new(gt.outputs[0], st.inputs[1])
    st2 = nt.nodes.new('ShaderNodeMath')
    st2.operation = 'MULTIPLY'
    nt.links.new(st.outputs[0], st2.inputs[0])
    nt.links.new(sep.outputs[1], st2.inputs[1])
    st3 = nt.nodes.new('ShaderNodeMath')
    st3.operation = 'MULTIPLY'
    st3.inputs[1].default_value = 3.0
    nt.links.new(st2.outputs[0], st3.inputs[0])
    # nebula
    nz = nt.nodes.new('ShaderNodeTexNoise')
    nz.inputs['Scale'].default_value = 1.6
    nz.inputs['Detail'].default_value = 8
    nt.links.new(tc.outputs['Generated'], nz.inputs['Vector'])
    ramp = nt.nodes.new('ShaderNodeValToRGB')
    ramp.color_ramp.elements[0].position = 0.45
    ramp.color_ramp.elements[0].color = (0.0, 0.0, 0.0, 1)
    ramp.color_ramp.elements[1].position = 0.85
    ramp.color_ramp.elements[1].color = (0.05, 0.025, 0.09, 1)
    nt.links.new(nz.outputs['Fac'], ramp.inputs[0])
    add = nt.nodes.new('ShaderNodeMix')
    add.data_type = 'RGBA'
    add.blend_type = 'ADD'
    add.inputs[0].default_value = 1.0
    nt.links.new(ramp.outputs[0], add.inputs[6])
    nt.links.new(st3.outputs[0], add.inputs[7])
    bg_cam = nt.nodes.new('ShaderNodeBackground')
    nt.links.new(add.outputs[2], bg_cam.inputs['Color'])
    bg_amb = nt.nodes.new('ShaderNodeBackground')
    bg_amb.inputs['Color'].default_value = (0.02, 0.025, 0.035, 1)
    lp = nt.nodes.new('ShaderNodeLightPath')
    mx = nt.nodes.new('ShaderNodeMixShader')
    nt.links.new(lp.outputs['Is Camera Ray'], mx.inputs[0])
    nt.links.new(bg_amb.outputs[0], mx.inputs[1])
    nt.links.new(bg_cam.outputs[0], mx.inputs[2])
    nt.links.new(mx.outputs[0], out.inputs['Surface'])


def add_sun(name, direction_from, strength, color, angle=2.0):
    ld = bpy.data.lights.new(name, 'SUN')
    ld.energy = strength
    ld.color = color
    ld.angle = math.radians(angle)
    ob = bpy.data.objects.new(name, ld)
    bpy.context.scene.collection.objects.link(ob)
    d = -V(direction_from).normalized()
    ob.rotation_euler = d.to_track_quat('-Z', 'Y').to_euler()
    return ob


def setup_preview_scene():
    setup_world()
    add_sun("Key", (1.0, 0.8, 1.1), 3.0, (1.0, 0.94, 0.86))
    add_sun("Rim", (-1.1, -1.0, 0.5), 6.0, (0.55, 0.72, 1.0), angle=1.0)
    add_sun("Fill", (-0.6, 0.6, -0.7), 0.5, (0.7, 0.7, 0.9))
    sc = bpy.context.scene
    sc.render.engine = 'CYCLES'
    sc.cycles.use_denoising = True
    try:
        sc.cycles.denoiser = 'OPENIMAGEDENOISE'
    except Exception:
        pass
    try:
        sc.view_settings.view_transform = 'AgX'
        try:
            sc.view_settings.look = 'AgX - Medium High Contrast'
        except Exception:
            sc.view_settings.look = 'Medium High Contrast'
    except Exception as e:
        log("look", e)
    cd = bpy.data.cameras.new("Cam")
    cd.lens = 50
    cam = bpy.data.objects.new("Cam", cd)
    sc.collection.objects.link(cam)
    sc.camera = cam
    return cam


def frame(cam, objs, direction, ortho=False, margin=1.12, aspect=16 / 9):
    pts = []
    for o in objs:
        if o.type != 'MESH' or o.hide_render:
            continue
        vs = o.data.vertices
        step = max(1, len(vs) // 4000)
        for i in range(0, len(vs), step):
            pts.append(o.matrix_world @ vs[i].co)
    lo = V((min(p.x for p in pts), min(p.y for p in pts), min(p.z for p in pts)))
    hi = V((max(p.x for p in pts), max(p.y for p in pts), max(p.z for p in pts)))
    c = (lo + hi) / 2
    d = V(direction).normalized()
    up0 = V((0, 0, 1)) if abs(d.z) < 0.95 else V((0, 1, 0))
    right = up0.cross(d).normalized()
    up = d.cross(right).normalized()
    cd = cam.data
    sw = 36.0
    tanH = (sw / 2) / cd.lens / margin
    tanV = tanH / aspect
    if ortho:
        cd.type = 'ORTHO'
        xs = [abs((p - c).dot(right)) for p in pts]
        ys = [abs((p - c).dot(up)) for p in pts]
        cd.ortho_scale = max(max(xs) * 2, max(ys) * 2 * aspect) * margin
        dist = (hi - lo).length * 2
    else:
        cd.type = 'PERSP'
        dist = 0
        for p in pts:
            q = p - c
            dd = q.dot(d)
            dist = max(dist, abs(q.dot(right)) / tanH + dd, abs(q.dot(up)) / tanV + dd)
    cam.location = c + d * dist
    M = Matrix((right, up, d)).transposed()
    cam.rotation_euler = M.to_euler()
    cd.clip_start = 0.1
    cd.clip_end = dist * 4


def render(path, w=1280, h=720, samples=96):
    sc = bpy.context.scene
    sc.render.resolution_x = w
    sc.render.resolution_y = h
    sc.render.resolution_percentage = 100
    sc.cycles.samples = samples
    sc.render.image_settings.file_format = 'PNG'
    sc.render.filepath = path
    t = time.time()
    bpy.ops.render.render(write_still=True)
    log("render", path, "%.1fs" % (time.time() - t))


def compose_sheet(paths, out, cols=2, cw=640, ch=360):
    rows = (len(paths) + cols - 1) // cols
    W, Hh = cw * cols, ch * rows
    canvas = np.zeros((Hh, W, 4), dtype=np.float32)
    canvas[..., 3] = 1
    for i, p in enumerate(paths):
        im = bpy.data.images.load(p)
        a = np.empty(cw * ch * 4, dtype=np.float32)
        im.pixels.foreach_get(a)
        a = a.reshape(ch, cw, 4)
        r, c = divmod(i, cols)
        # blender pixel rows start at bottom
        y0 = Hh - (r + 1) * ch
        canvas[y0:y0 + ch, c * cw:(c + 1) * cw] = a
        bpy.data.images.remove(im)
    img = bpy.data.images.new("sheet", W, Hh, alpha=False)
    img.pixels.foreach_set(canvas.ravel())
    img.filepath_raw = out
    img.file_format = 'PNG'
    img.save()
    bpy.data.images.remove(img)

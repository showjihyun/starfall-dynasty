"""STARFALL DYNASTY prop / enemy helpers (extends shiplib, never modifies it).

Adds:
  * Prop: a shiplib.Ship with a configurable material-slot order and booleans.
  * extra geometry helpers: slab (extruded outline), nozzle, jag (notched outlines),
    greebles (kitbash scatter on a source surface), cables, jitter.
  * material helpers: hull space scaling, rust/scorch overlays, Stone / Rock sources.
  * generic Cycles bake (same-object or selected-to-active) for any material name.
"""
import bpy
import bmesh
import math
import os
import random
import time
import numpy as np
from mathutils import Vector, Matrix, noise

import shiplib as S
from shiplib import V, loft, circle_ring, lathe, box, prism, cyl, sphere, wing, NB, log


# ================================================================ objects

class Prop(S.Ship):
    """Ship with arbitrary material order. order: list of material names (slot index = list index)."""

    def __init__(self, name, mats, order):
        super().__init__(name, mats)
        self.order = order

    def part(self, build, group="body", bevel=0.03, segs=2, angle=32, mirror=False,
             subsurf=0, harden=True, weighted=True, name=None, sharp_all=False, recalc=True,
             post=None, bevel_limit='ANGLE'):
        bm = bmesh.new()
        build(bm)
        bad = [v for v in bm.verts if not all(math.isfinite(c) and abs(c) < 1e5 for c in v.co)]
        if bad:
            log("WARNING part", group, getattr(build, '__name__', '?'), "has", len(bad), "bad verts; removing")
            bmesh.ops.delete(bm, geom=bad, context='VERTS')
        if mirror:
            S.mirror_bm_x(bm)
        bmesh.ops.remove_doubles(bm, verts=bm.verts, dist=1e-5)
        if recalc:
            bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
        nm = name or (group + "_part")
        me = bpy.data.meshes.new(nm)
        bm.to_mesh(me)
        bm.free()
        me.uv_layers.new(name="UVMap")
        obj = bpy.data.objects.new(nm, me)
        self.coll.objects.link(obj)
        for mn in self.order:
            me.materials.append(self.mats[mn])
        S.set_active(obj)
        if post:
            post(obj)
            S.set_active(obj)
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
            backup = obj.data.copy()
            for attempt, (miter, wscale) in enumerate((('MITER_ARC', 1.0), ('MITER_SHARP', 0.5), (None, 0))):
                if miter is None:
                    log("WARNING bevel skipped on", nm)
                    break
                m = obj.modifiers.new("bevel", 'BEVEL')
                m.width = bevel * wscale
                m.segments = segs
                m.limit_method = bevel_limit
                m.angle_limit = math.radians(angle)
                m.harden_normals = harden
                m.use_clamp_overlap = True
                m.miter_outer = miter
                bpy.ops.object.modifier_apply(modifier="bevel")
                co = np.empty(len(obj.data.vertices) * 3, dtype=np.float64)
                obj.data.vertices.foreach_get("co", co)
                if np.all(np.isfinite(co)) and np.abs(co).max() < 1e5:
                    break
                log("WARNING bevel blew up on", nm, "attempt", attempt, "- retrying")
                bad = obj.data
                obj.data = backup.copy()
                bpy.data.meshes.remove(bad)
            bpy.data.meshes.remove(backup)
        if weighted and not sharp_all:
            m = obj.modifiers.new("wn", 'WEIGHTED_NORMAL')
            m.keep_sharp = True
            m.weight = 50
            bpy.ops.object.modifier_apply(modifier="wn")
        self.groups.setdefault(group, []).append(obj)
        return obj

    def finalize(self, center=True, body_name="Hull"):
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
            o.name = body_name if gname == "body" else gname
            o.data.name = o.name
            bpy.ops.object.material_slot_remove_unused()
            result[o.name] = o
        self.objects = result
        lo = V((1e9, 1e9, 1e9))
        hi = V((-1e9, -1e9, -1e9))
        for o in result.values():
            for v in o.data.vertices:
                p = o.matrix_world @ v.co
                lo = V((min(lo.x, p.x), min(lo.y, p.y), min(lo.z, p.z)))
                hi = V((max(hi.x, p.x), max(hi.y, p.y), max(hi.z, p.z)))
        c = (lo + hi) / 2 if center else V((0, 0, 0))
        if center:
            T = Matrix.Translation(-c)
            for o in result.values():
                o.data.transform(T)
                o.data.update()
            for e in self.empties:
                e.location = e.location - c
        self.bbox = (lo - c, hi - c)
        log(self.name, "bbox size", tuple(round(x, 2) for x in (hi - lo)))
        return result


def bm_to_obj(bm, name, mats, order, coll=None):
    me = bpy.data.meshes.new(name)
    bm.to_mesh(me)
    obj = bpy.data.objects.new(name, me)
    (coll or bpy.context.scene.collection).objects.link(obj)
    for mn in order:
        me.materials.append(mats[mn])
    return obj


def boolean(obj, cutter_build, mats, order, op='DIFFERENCE', solver='EXACT'):
    """cutter_build(bm) fills the cutter (material indices = target slot indices)."""
    bm = bmesh.new()
    cutter_build(bm)
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
    cut = bm_to_obj(bm, obj.name + "_cut", mats, order)
    bm.free()
    m = obj.modifiers.new("bool", 'BOOLEAN')
    m.object = cut
    m.operation = op
    m.solver = solver
    try:
        m.material_mode = 'INDEX'
    except Exception:
        pass
    S.set_active(obj)
    bpy.ops.object.modifier_apply(modifier="bool")
    me = cut.data
    bpy.data.objects.remove(cut)
    bpy.data.meshes.remove(me)


def solidify(obj, thickness, offset=-1.0, rim_mat_offset=0):
    m = obj.modifiers.new("solid", 'SOLIDIFY')
    m.thickness = thickness
    m.offset = offset
    m.use_even_offset = True
    m.material_offset_rim = rim_mat_offset
    S.set_active(obj)
    bpy.ops.object.modifier_apply(modifier="solid")


# ================================================================ geometry helpers

def frame_axes(n, hint=(0, 1, 0)):
    """Orthonormal (t, b, n) with t ~ projection of hint onto the plane."""
    n = V(n).normalized()
    h = V(hint)
    if abs(h.normalized().dot(n)) > 0.95:
        h = V((1, 0, 0)) if abs(n.x) < 0.9 else V((0, 0, 1))
    t = (h - n * h.dot(n)).normalized()
    b = t.cross(n).normalized()
    return t, b, n


def rot_tbn(t, b, n):
    """Matrix whose local x->b, y->t, z->n."""
    return Matrix((b, t, n)).transposed()


def slab(bm, pts2d, thick, origin=(0, 0, 0), u=(1, 0, 0), v=(0, 1, 0), mat=0, bevel_top=0.0, matfn=None):
    """Extrude a 2D outline (in the u,v plane through origin) symmetrically by thick.
    bevel_top shrinks the top/bottom faces toward the centroid (chamfered slab)."""
    o, u, v = V(origin), V(u).normalized(), V(v).normalized()
    nrm = u.cross(v).normalized()
    cx = sum(p[0] for p in pts2d) / len(pts2d)
    cy = sum(p[1] for p in pts2d) / len(pts2d)

    def ring(off, shrink):
        out = []
        for x, y in pts2d:
            if shrink > 0:
                dx, dy = x - cx, y - cy
                L = math.hypot(dx, dy) or 1.0
                s = max(0.0, (L - shrink) / L)
                x, y = cx + dx * s, cy + dy * s
            out.append(o + u * x + v * y + nrm * off)
        return out
    if bevel_top > 0:
        rings = [ring(-thick / 2, bevel_top), ring(-thick / 2 + bevel_top * 0.5, 0), ring(thick / 2 - bevel_top * 0.5, 0), ring(thick / 2, bevel_top)]
    else:
        rings = [ring(-thick / 2, 0), ring(thick / 2, 0)]
    return loft(bm, rings, mat=mat, matfn=matfn)


def jag(outline, rng, depth=0.3, seg=1.2, closed=True, prob=0.7, inward=True):
    """Insert sawtooth notches along an outline (list of 2D points, CCW)."""
    out = []
    n = len(outline)
    rngs = range(n) if closed else range(n - 1)
    for i in rngs:
        a = V((*outline[i], 0))
        b = V((*outline[(i + 1) % n], 0))
        out.append((a.x, a.y))
        L = (b - a).length
        k = int(L / seg)
        if k < 2:
            continue
        d = (b - a) / L
        nn = V((-d.y, d.x, 0)) if inward else V((d.y, -d.x, 0))  # CCW outline: left normal = inward
        for j in range(1, k):
            if rng.random() > prob:
                continue
            t0 = (j - 0.35) / k
            t1 = (j + 0.15) / k
            p0 = a + (b - a) * t0
            p1 = a + (b - a) * t1
            dd = depth * rng.uniform(0.4, 1.0)
            out.append((p0.x, p0.y))
            out.append(((p0 + nn * dd).x, (p0 + nn * dd).y))
            out.append((p1.x, p1.y))
    if not closed:
        out.append(tuple(outline[-1]))
    return out


def nozzle(bm, exit_c, r, length, front=(0, 1, 0), n=24, rimmat=0, coremat=4, hullmat=0, flare=1.08, ribs=0):
    """Engine bell. exit_c = exit plane center; front points into the ship."""
    prof = [
        (r * 0.8, length),
        (r * 0.96, length * 0.62),
        (r * flare, length * 0.12),
        (r * (flare - 0.03), 0.0),
        (r * 0.9, 0.0),
        (r * 0.86, length * 0.1),
        (r * 0.64, length * 0.6),
        (r * 0.4, length * 0.62),
    ]
    mats = [hullmat, hullmat, hullmat, rimmat, hullmat, coremat, coremat]
    prof.append((0.0, length * 0.3))
    mats.append(coremat)
    lathe(bm, exit_c, front, prof, n=n, mats=mats, cap_start=False, cap_end=False)
    f = V(front).normalized()
    for i in range(ribs):
        a = 2 * math.pi * i / ribs
        t, b, _ = frame_axes(f, (0, 0, 1))
        d = t * math.cos(a) + b * math.sin(a)
        p0 = V(exit_c) + f * (length * 0.15) + d * r * 1.02
        p1 = V(exit_c) + f * (length * 0.85) + d * r * 0.86
        prism(bm, p0, p1, r * 0.08, r * 0.06, mat=hullmat)


def sweep(bm, pts, radii, n=12, mat=0, squash=1.0, up=(0, 0, 1), profile=None, cap_start=True, cap_end=True,
          tip_end=False, matfn=None):
    """Loft a cross-section along a polyline. radii: list (per point) or callable(u).
    profile: list of unit 2D points (x=right, y=up) instead of a circle; squash scales y."""
    P_ = [V(p) for p in pts]
    N = len(P_)
    if profile is None:
        profile = [(math.cos(2 * math.pi * k / n + math.pi / n), math.sin(2 * math.pi * k / n + math.pi / n)) for k in range(n)]
    rings = []
    upv = V(up)
    for i in range(N):
        u = i / (N - 1)
        r = radii(u) if callable(radii) else radii[i]
        d = (P_[min(N - 1, i + 1)] - P_[max(0, i - 1)]).normalized()
        h = upv if abs(d.dot(upv)) < 0.95 else V((1, 0, 0))
        right = d.cross(h).normalized()
        upl = right.cross(d).normalized()
        if tip_end and i == N - 1:
            rings.append(P_[i])
            continue
        rx, ry = (r if not isinstance(r, tuple) else r[0]), (r if not isinstance(r, tuple) else r[1])
        rings.append([P_[i] + right * (x * rx) + upl * (y * ry * squash) for x, y in profile])
    return loft(bm, rings, mat=mat, matfn=matfn, cap_start=cap_start, cap_end=cap_end)


def bezier(p0, p1, p2, p3, n=16):
    p0, p1, p2, p3 = V(p0), V(p1), V(p2), V(p3)
    out = []
    for i in range(n + 1):
        t = i / n
        a = 1 - t
        out.append(p0 * a ** 3 + p1 * 3 * a * a * t + p2 * 3 * a * t * t + p3 * t ** 3)
    return out


def spike(bm, base, direction, length, r, n=6, mat=0):
    d = V(direction).normalized()
    cyl(bm, V(base) - d * r * 0.5, V(base) + d * length, r, 0.0, n=n, mat=mat)


def jitter(verts, amt, rng):
    for v in verts:
        v.co += V((rng.uniform(-amt, amt), rng.uniform(-amt, amt), rng.uniform(-amt, amt)))


def noise_displace(verts, amp, scale, seed=0, along_normal=True, octaves=4):
    off = V((seed * 17.3, seed * 5.1, seed * 11.7))
    for v in verts:
        p = v.co * scale + off
        d = noise.fractal(p, 0.6, 2.0, octaves, noise_basis='PERLIN_ORIGINAL')
        if along_normal and v.normal.length > 0:
            v.co += v.normal * d * amp
        else:
            v.co += V((noise.noise(p), noise.noise(p + V((7, 3, 1))), noise.noise(p + V((2, 9, 4))))) * amp


def sample_surface(bm, count, rng, mask=None, hint=(0, 1, 0)):
    """Area-weighted random points on faces. Returns [(p, t, b, n, face_area)]."""
    tris = []
    for f in bm.faces:
        if mask is not None and not mask(f):
            continue
        vs = [v.co.copy() for v in f.verts]
        for i in range(1, len(vs) - 1):
            a, b_, c = vs[0], vs[i], vs[i + 1]
            ar = (b_ - a).cross(c - a).length / 2
            if ar > 1e-6:
                tris.append((a, b_, c, ar, f.normal.copy(), f.calc_area()))
    if not tris:
        return []
    w = np.array([t[3] for t in tris])
    cw = np.cumsum(w)
    out = []
    for _ in range(count):
        k = int(np.searchsorted(cw, rng.random() * cw[-1]))
        a, b_, c, ar, nn, fa = tris[min(k, len(tris) - 1)]
        r1, r2 = rng.random(), rng.random()
        if r1 + r2 > 1:
            r1, r2 = 1 - r1, 1 - r2
        p = a + (b_ - a) * r1 + (c - a) * r2
        t, b, n = frame_axes(nn, hint)
        out.append((p, t, b, n, fa))
    return out


def greebles(bm, samples, rng, scale=1.0, mats=(0, 1, 2), kinds=None, sink=0.04, maxsize=None):
    """Kitbash scatter. samples from sample_surface. mats=(hull, livery, glow).
    Sizes are in units of `scale` meters."""
    H_, L_, G_ = mats
    kinds = kinds or ['box', 'box', 'stack', 'vent', 'cyl', 'pipe', 'tank', 'plate']
    for p, t, b, n, fa in samples:
        kind = rng.choice(kinds)
        R = rot_tbn(t, b, n)
        s = scale * rng.uniform(0.6, 1.4)
        if maxsize:
            s = min(s, maxsize)
        if kind == 'box':
            w, l, h = s * rng.uniform(0.3, 1.0), s * rng.uniform(0.4, 1.6), s * rng.uniform(0.1, 0.35)
            box(bm, p + n * (h / 2 - sink * s), (w, l, h), mat=H_ if rng.random() > 0.2 else L_, rot=R)
        elif kind == 'plate':
            w, l, h = s * rng.uniform(0.8, 1.8), s * rng.uniform(0.8, 2.2), s * 0.06
            box(bm, p + n * (h / 2 - sink * s * 0.3), (w, l, h), mat=H_ if rng.random() > 0.35 else L_, rot=R)
        elif kind == 'stack':
            w, l, h = s * rng.uniform(0.4, 0.9), s * rng.uniform(0.5, 1.2), s * rng.uniform(0.15, 0.3)
            box(bm, p + n * (h / 2 - sink * s), (w, l, h), mat=H_, rot=R)
            h2 = h * rng.uniform(0.5, 1.0)
            box(bm, p + n * (h + h2 / 2 - sink * s) + t * l * 0.1, (w * 0.6, l * 0.5, h2), mat=H_, rot=R)
            if rng.random() < 0.5:
                box(bm, p + n * (h + h2 + 0.02 * s - sink * s) + t * l * 0.1, (w * 0.4, l * 0.08, 0.04 * s), mat=G_, rot=R)
        elif kind == 'vent':
            w, l, h = s * rng.uniform(0.5, 0.9), s * rng.uniform(0.6, 1.2), s * 0.12
            box(bm, p + n * (h / 2 - sink * s), (w, l, h), mat=H_, rot=R)
            k = rng.randint(3, 6)
            for i in range(k):
                y = -l * 0.4 + l * 0.8 * i / max(1, k - 1)
                box(bm, p + n * (h + 0.03 * s - sink * s) + t * y, (w * 0.8, l * 0.05, 0.06 * s), mat=H_, rot=R)
        elif kind == 'cyl':
            r = s * rng.uniform(0.12, 0.3)
            h = s * rng.uniform(0.15, 0.5)
            cyl(bm, p - n * sink * s, p + n * h, r, r * rng.uniform(0.7, 1.0), n=10, mat=H_)
            if rng.random() < 0.3:
                cyl(bm, p + n * h, p + n * (h + 0.04 * s), r * 0.5, n=8, mat=G_)
        elif kind == 'pipe':
            L = s * rng.uniform(1.0, 3.0)
            r = s * rng.uniform(0.04, 0.08)
            a = p + n * (r * 0.8) - t * L / 2
            c = p + n * (r * 0.8) + t * L / 2
            cyl(bm, a, c, r, n=8, mat=H_)
            for q in (a, c):
                box(bm, q - n * (r * 0.3), (r * 3, r * 3, r * 2.4), mat=H_, rot=R)
        elif kind == 'tank':
            r = s * rng.uniform(0.15, 0.28)
            L = s * rng.uniform(0.6, 1.4)
            c0 = p + n * r * 0.9
            cyl(bm, c0 - t * L / 2, c0 + t * L / 2, r, n=12, mat=H_ if rng.random() > 0.3 else L_)
            for q in (c0 - t * L / 2, c0 + t * L / 2):
                sphere(bm, q, r, mat=H_, seg=12, rings=6, scale=(1, 1, 1))


def cable(bm, pts, r, n=6, mat=0, sag=0.0, seg=12, rng=None, wobble=0.0):
    """Tube along a polyline (Catmull-Rom smoothed) with optional sag (-Z) and wobble."""
    P = [V(p) for p in pts]
    path = []
    for i in range(len(P) - 1):
        p0 = P[max(0, i - 1)]
        p1, p2 = P[i], P[i + 1]
        p3 = P[min(len(P) - 1, i + 2)]
        for k in range(seg):
            t = k / seg
            t2, t3 = t * t, t * t * t
            q = 0.5 * ((2 * p1) + (-p0 + p2) * t + (2 * p0 - 5 * p1 + 4 * p2 - p3) * t2 + (-p0 + 3 * p1 - 3 * p2 + p3) * t3)
            path.append(q)
    path.append(P[-1])
    N = len(path)
    for i in range(N):
        u = i / (N - 1)
        path[i] = path[i] - V((0, 0, sag * 4 * u * (1 - u)))
        if rng and wobble:
            path[i] += V((rng.uniform(-1, 1), rng.uniform(-1, 1), rng.uniform(-1, 1))) * wobble * math.sin(math.pi * u)
    rings = []
    prev_ref = None
    for i in range(N):
        d = (path[min(N - 1, i + 1)] - path[max(0, i - 1)]).normalized()
        ref = prev_ref if prev_ref is not None else (V((0, 0, 1)) if abs(d.z) < 0.9 else V((1, 0, 0)))
        uu = d.cross(ref).normalized()
        vv = d.cross(uu).normalized()
        prev_ref = vv
        rings.append([path[i] + uu * (r * math.cos(2 * math.pi * k / n)) + vv * (r * math.sin(2 * math.pi * k / n)) for k in range(n)])
    loft(bm, rings, mat=mat)


def tagged_faces(bm, fn):
    return [f for f in bm.faces if fn(f)]


# ================================================================ materials

def flat_mat(name, base=(0.8, 0.8, 0.8), metal=0.0, rough=0.5, emit=None, strength=5.0):
    m = S._new_mat(name)
    p = m.node_tree.nodes["Principled BSDF"]
    p.inputs['Base Color'].default_value = (*base, 1)
    p.inputs['Metallic'].default_value = metal
    p.inputs['Roughness'].default_value = rough
    if emit is not None:
        p.inputs['Emission Color'].default_value = (*emit, 1)
        p.inputs['Emission Strength'].default_value = strength
    return m


def scale_space(mat, k):
    """Make an object-space procedural material k times larger (for big hulls)."""
    nt = mat.node_tree
    tcs = [n for n in nt.nodes if n.type == 'TEX_COORD']
    for tc in tcs:
        sock = tc.outputs['Object']
        links = [lk for lk in nt.links if lk.from_socket == sock]
        mp = nt.nodes.new('ShaderNodeMapping')
        mp.inputs['Scale'].default_value = (1.0 / k, 1.0 / k, 1.0 / k)
        for lk in links:
            to = lk.to_socket
            nt.links.remove(lk)
            nt.links.new(mp.outputs[0], to)
        nt.links.new(sock, mp.inputs['Vector'])
    for n in nt.nodes:
        if n.type == 'BEVEL':
            n.inputs['Radius'].default_value *= k
        elif n.type == 'AMBIENT_OCCLUSION':
            n.inputs['Distance'].default_value *= k
        elif n.type == 'BUMP':
            n.inputs['Distance'].default_value *= k


def _pos(nb):
    """Object position socket (through the scale_space Mapping if present)."""
    nt = nb.nt
    for n in nt.nodes:
        if n.type == 'MAPPING':
            return n.outputs[0]
    for n in nt.nodes:
        if n.type == 'TEX_COORD':
            return n.outputs['Object']


def overlay_hull(mat, socks, rust=0.0, scorch=0.0, rust_col=(0.16, 0.07, 0.03), soot_col=(0.012, 0.011, 0.01),
                 heat=0.0, seed=0.0, stripes=None):
    """Add rust streaks / scorch blotches / heat tint on top of shiplib's hull material.
    Updates socks in place and relinks the BSDF."""
    nb = NB(mat)
    pos = _pos(nb)
    base, rough, metal = socks['base'], socks['rough'], socks['metal']
    bsdf = socks['bsdf']
    if rust > 0:
        mp = nb.node('ShaderNodeMapping')
        nb.link(pos, mp.inputs['Vector'])
        mp.inputs['Scale'].default_value = (2.5, 2.5, 0.35)
        mp.inputs['Location'].default_value = (seed, seed * 2, 0)
        nz = nb.node('ShaderNodeTexNoise')
        nb.link(mp.outputs[0], nz.inputs['Vector'])
        nz.inputs['Scale'].default_value = 1.4
        nz.inputs['Detail'].default_value = 10
        nz.inputs['Roughness'].default_value = 0.65
        streak = nb.mrange(nz.outputs['Fac'], 0.52, 0.7, 0.0, 1.0, smooth=True)
        bl = nb.node('ShaderNodeTexNoise')
        nb.link(pos, bl.inputs['Vector'])
        bl.inputs['Scale'].default_value = 0.55
        bl.inputs['Detail'].default_value = 12
        bl.inputs['Roughness'].default_value = 0.7
        blot = nb.mrange(bl.outputs['Fac'], 0.56, 0.68, 0.0, 1.0, smooth=True)
        ao = nb.node('ShaderNodeAmbientOcclusion')
        ao.samples = 8
        ao.inputs['Distance'].default_value = 0.6
        cav = nb.mrange(ao.outputs['AO'], 0.3, 0.9, 1.0, 0.0)
        rm = nb.math('MAXIMUM', nb.math('MULTIPLY', streak, 0.7), blot)
        rm = nb.math('ADD', rm, nb.math('MULTIPLY', cav, 0.6))
        rm = nb.math('MULTIPLY', rm, rust, clamp=True)
        dn = nb.node('ShaderNodeTexNoise')
        nb.link(pos, dn.inputs['Vector'])
        dn.inputs['Scale'].default_value = 9.0
        dn.inputs['Detail'].default_value = 6
        rc2 = nb.mix(dn.outputs['Fac'], (*[c * 0.6 for c in rust_col], 1), (*[min(1, c * 1.7) for c in rust_col], 1))
        base = nb.mix(rm, base, rc2)
        rough = nb.mixf(rm, rough, 0.85)
        metal = nb.mixf(rm, metal, 0.05)
    if scorch > 0:
        sc = nb.node('ShaderNodeTexNoise')
        nb.link(pos, sc.inputs['Vector'])
        sc.inputs['Scale'].default_value = 0.35
        sc.inputs['Detail'].default_value = 12
        sc.inputs['Roughness'].default_value = 0.72
        sc.noise_dimensions = '4D'
        sc.inputs['W'].default_value = seed
        sm = nb.mrange(sc.outputs['Fac'], 0.47, 0.62, 0.0, 1.0, smooth=True)
        sm = nb.math('MULTIPLY', sm, scorch, clamp=True)
        if heat > 0:
            # blue/purple heat-tint band at the edge of the scorch
            band = nb.math('MULTIPLY', nb.mrange(sc.outputs['Fac'], 0.42, 0.47, 0.0, 1.0, smooth=True),
                           nb.mrange(sc.outputs['Fac'], 0.47, 0.52, 1.0, 0.0, smooth=True))
            base = nb.mix(nb.math('MULTIPLY', band, heat), base, (0.12, 0.1, 0.22, 1))
        base = nb.mix(sm, base, (*soot_col, 1))
        rough = nb.mixf(sm, rough, 0.92)
        metal = nb.mixf(sm, metal, 0.0)
    if stripes:
        # painted hazard / clan stripes along an axis (axis, period, width, color)
        ax, period, width, col = stripes
        sp = nb.node('ShaderNodeSeparateXYZ')
        nb.link(pos, sp.inputs[0])
        c = sp.outputs[ax]
        d = nb.math('ABSOLUTE', nb.math('SUBTRACT', nb.math('FRACT', nb.math('DIVIDE', c, period)), 0.5))
        st = nb.mrange(d, width * 0.5 - 0.01, width * 0.5, 1.0, 0.0)
        msk = nb.node('ShaderNodeTexNoise')
        nb.link(pos, msk.inputs['Vector'])
        msk.inputs['Scale'].default_value = 0.4
        st = nb.math('MULTIPLY', st, nb.mrange(msk.outputs['Fac'], 0.5, 0.52), clamp=True)
        base = nb.mix(st, base, (*col, 1))
    socks['base'], socks['rough'], socks['metal'] = base, rough, metal
    nb.link(base, bsdf.inputs['Base Color'])
    nb.link(rough, bsdf.inputs['Roughness'])
    nb.link(metal, bsdf.inputs['Metallic'])
    return socks


def _wear_edge(nb, nrm, radius, lo=0.985, hi=0.85):
    bev = nb.node('ShaderNodeBevel')
    bev.samples = 8
    bev.inputs['Radius'].default_value = radius
    e = nb.vmath('DOT_PRODUCT', bev.outputs[0], nrm)
    return nb.mrange(e, lo, hi, 0.0, 1.0)


def make_stone_src(name="Stone", k=1.0, col_a=(0.23, 0.22, 0.2), col_b=(0.14, 0.135, 0.13),
                   patina=(0.06, 0.12, 0.11), crack_scale=0.06, bump=1.0):
    """Ancient carved stone: weathered tone variation, cracks, pitting, patina in cavities,
    chipped lighter edges. k = spatial scale (1 -> features sized for ~10 m objects)."""
    m = S._new_mat(name)
    nb = NB(m)
    nt = m.node_tree
    for n in list(nt.nodes):
        nt.nodes.remove(n)
    out = nb.node('ShaderNodeOutputMaterial')
    bsdf = nb.node('ShaderNodeBsdfPrincipled')
    tc = nb.node('ShaderNodeTexCoord')
    geo = nb.node('ShaderNodeNewGeometry')
    nrm = geo.outputs['Normal']
    mp = nb.node('ShaderNodeMapping')
    nb.link(tc.outputs['Object'], mp.inputs['Vector'])
    mp.inputs['Scale'].default_value = (1 / k, 1 / k, 1 / k)
    pos = mp.outputs[0]
    # large tonal variation
    n1 = nb.node('ShaderNodeTexNoise')
    nb.link(pos, n1.inputs['Vector'])
    n1.inputs['Scale'].default_value = 0.12
    n1.inputs['Detail'].default_value = 10
    n1.inputs['Roughness'].default_value = 0.62
    tone = nb.mrange(n1.outputs['Fac'], 0.35, 0.7)
    # strata
    sx = nb.node('ShaderNodeSeparateXYZ')
    nb.link(pos, sx.inputs[0])
    strata = nb.math('SINE', nb.math('ADD', nb.math('MULTIPLY', sx.outputs[2], 2.3), nb.math('MULTIPLY', n1.outputs['Fac'], 6.0)))
    strata = nb.mrange(strata, -0.2, 1.0, 0.0, 1.0, smooth=True)
    # cracks: noise-distorted voronoi edges
    dn = nb.node('ShaderNodeTexNoise')
    nb.link(pos, dn.inputs['Vector'])
    dn.inputs['Scale'].default_value = 0.5
    dn.inputs['Detail'].default_value = 6
    scn = nb.node('ShaderNodeVectorMath', operation='SCALE')
    nb.link(dn.outputs['Color'], scn.inputs[0])
    scn.inputs['Scale'].default_value = 0.8
    dpos = nb.vmath('ADD', pos, scn.outputs[0])
    cv = nb.node('ShaderNodeTexVoronoi', voronoi_dimensions='3D', feature='DISTANCE_TO_EDGE')
    nb.link(dpos, cv.inputs['Vector'])
    cv.inputs['Scale'].default_value = crack_scale * 10
    crack = nb.mrange(cv.outputs['Distance'], 0.004, 0.025, 1.0, 0.0, smooth=True)
    cn = nb.node('ShaderNodeTexNoise')
    nb.link(pos, cn.inputs['Vector'])
    cn.inputs['Scale'].default_value = 0.3
    crack = nb.math('MULTIPLY', crack, nb.mrange(cn.outputs['Fac'], 0.45, 0.6), clamp=True)
    # pitting
    pv = nb.node('ShaderNodeTexVoronoi', voronoi_dimensions='3D', feature='SMOOTH_F1')
    nb.link(pos, pv.inputs['Vector'])
    pv.inputs['Scale'].default_value = 3.5
    pits = nb.mrange(pv.outputs['Distance'], 0.0, 0.18, 1.0, 0.0, smooth=True)
    pm = nb.node('ShaderNodeTexNoise')
    nb.link(pos, pm.inputs['Vector'])
    pm.inputs['Scale'].default_value = 0.8
    pits = nb.math('MULTIPLY', pits, nb.mrange(pm.outputs['Fac'], 0.45, 0.6), clamp=True)
    # fine grain
    fg = nb.node('ShaderNodeTexNoise')
    nb.link(pos, fg.inputs['Vector'])
    fg.inputs['Scale'].default_value = 14.0
    fg.inputs['Detail'].default_value = 8
    # cavity / AO
    ao = nb.node('ShaderNodeAmbientOcclusion')
    ao.samples = 16
    ao.inputs['Distance'].default_value = 1.5 * k
    cav = nb.mrange(ao.outputs['AO'], 0.25, 0.95, 1.0, 0.0)
    edge = _wear_edge(nb, nrm, 0.06 * k)
    en = nb.node('ShaderNodeTexNoise')
    nb.link(pos, en.inputs['Vector'])
    en.inputs['Scale'].default_value = 4.0
    en.inputs['Detail'].default_value = 8
    chip = nb.math('MULTIPLY', edge, nb.mrange(en.outputs['Fac'], 0.4, 0.62), clamp=True)

    col = nb.mix(tone, (*col_b, 1), (*col_a, 1))
    col = nb.mix(nb.math('MULTIPLY', strata, 0.25), col, (*[c * 1.25 for c in col_a], 1))
    col = nb.mix(nb.math('MULTIPLY', fg.outputs['Fac'], 0.3), col, (*[c * 0.7 for c in col_b], 1))
    col = nb.mix(nb.math('MULTIPLY', cav, 0.85), col, (*patina, 1))
    col = nb.mix(nb.math('MULTIPLY', crack, 0.9), col, (0.02, 0.02, 0.02, 1))
    col = nb.mix(nb.math('MULTIPLY', pits, 0.5), col, (*[c * 0.5 for c in col_b], 1))
    col = nb.mix(nb.math('MULTIPLY', chip, 0.7), col, (*[min(1, c * 1.9) for c in col_a], 1))
    rough = nb.math('ADD', 0.72, nb.math('MULTIPLY', fg.outputs['Fac'], 0.2))
    rough = nb.mixf(cav, rough, 0.95)
    rough = nb.mixf(chip, rough, 0.6)
    metal = nb.math('MULTIPLY', tone, 0.0)
    height = nb.math('ADD', nb.math('MULTIPLY', tone, 0.25), nb.math('MULTIPLY', fg.outputs['Fac'], 0.25))
    height = nb.math('ADD', height, nb.math('MULTIPLY', strata, 0.15))
    height = nb.math('SUBTRACT', height, nb.math('MULTIPLY', crack, 0.9))
    height = nb.math('SUBTRACT', height, nb.math('MULTIPLY', pits, 0.45))
    height = nb.math('SUBTRACT', height, nb.math('MULTIPLY', chip, 0.3))
    bp = nb.node('ShaderNodeBump')
    bp.inputs['Strength'].default_value = bump
    bp.inputs['Distance'].default_value = 0.05 * k
    nb.link(height, bp.inputs['Height'])
    nb.link(col, bsdf.inputs['Base Color'])
    nb.link(rough, bsdf.inputs['Roughness'])
    nb.link(metal, bsdf.inputs['Metallic'])
    nb.link(bp.outputs['Normal'], bsdf.inputs['Normal'])
    nb.link(bsdf.outputs[0], out.inputs['Surface'])
    em = nb.node('ShaderNodeEmission')
    return m, {'base': col, 'rough': rough, 'metal': metal, 'normal': bp.outputs['Normal'], 'bsdf': bsdf, 'out': out, 'emit': em}


def make_rock_src(name="RockSrc", col_a=(0.36, 0.33, 0.3), col_b=(0.2, 0.185, 0.17), seed=0.0):
    """Asteroid rock (applied on the high-poly): neutral grey-brown, cavity dust,
    pointiness edges, regolith speckle, fine pitting bump."""
    m = S._new_mat(name)
    nb = NB(m)
    nt = m.node_tree
    for n in list(nt.nodes):
        nt.nodes.remove(n)
    out = nb.node('ShaderNodeOutputMaterial')
    bsdf = nb.node('ShaderNodeBsdfPrincipled')
    tc = nb.node('ShaderNodeTexCoord')
    pos = tc.outputs['Object']
    geo = nb.node('ShaderNodeNewGeometry')
    point = geo.outputs['Pointiness']
    n1 = nb.node('ShaderNodeTexNoise')
    nb.link(pos, n1.inputs['Vector'])
    n1.inputs['Scale'].default_value = 0.9
    n1.inputs['Detail'].default_value = 10
    n1.inputs['Roughness'].default_value = 0.6
    n1.noise_dimensions = '4D'
    n1.inputs['W'].default_value = seed
    tone = nb.mrange(n1.outputs['Fac'], 0.35, 0.68)
    n2 = nb.node('ShaderNodeTexNoise')
    nb.link(pos, n2.inputs['Vector'])
    n2.inputs['Scale'].default_value = 6.0
    n2.inputs['Detail'].default_value = 12
    n2.inputs['Roughness'].default_value = 0.7
    ridge = nb.mrange(point, 0.5, 0.56, 0.0, 1.0, smooth=True)
    cavp = nb.mrange(point, 0.49, 0.44, 0.0, 1.0, smooth=True)
    ao = nb.node('ShaderNodeAmbientOcclusion')
    ao.samples = 16
    ao.inputs['Distance'].default_value = 0.25
    cav = nb.mrange(ao.outputs['AO'], 0.3, 0.95, 1.0, 0.0)
    cav = nb.math('MAXIMUM', cav, nb.math('MULTIPLY', cavp, 0.6))
    sp = nb.node('ShaderNodeTexVoronoi', voronoi_dimensions='3D', feature='F1')
    nb.link(pos, sp.inputs['Vector'])
    sp.inputs['Scale'].default_value = 40.0
    speck = nb.mrange(sp.outputs['Distance'], 0.0, 0.12, 1.0, 0.0)
    col = nb.mix(tone, (*col_b, 1), (*col_a, 1))
    col = nb.mix(nb.math('MULTIPLY', n2.outputs['Fac'], 0.35), col, (*[c * 0.6 for c in col_b], 1))
    col = nb.mix(nb.math('MULTIPLY', cav, 0.7), col, (*[c * 0.35 for c in col_b], 1))
    col = nb.mix(nb.math('MULTIPLY', ridge, 0.45), col, (*[min(1, c * 1.45) for c in col_a], 1))
    col = nb.mix(nb.math('MULTIPLY', speck, 0.25), col, (*[min(1, c * 1.6) for c in col_a], 1))
    rough = nb.math('ADD', 0.78, nb.math('MULTIPLY', n2.outputs['Fac'], 0.18))
    rough = nb.mixf(ridge, rough, 0.62)
    metal = nb.math('MULTIPLY', tone, 0.0)
    pv = nb.node('ShaderNodeTexVoronoi', voronoi_dimensions='3D', feature='SMOOTH_F1')
    nb.link(pos, pv.inputs['Vector'])
    pv.inputs['Scale'].default_value = 9.0
    pits = nb.mrange(pv.outputs['Distance'], 0.0, 0.25, 1.0, 0.0, smooth=True)
    height = nb.math('ADD', nb.math('MULTIPLY', n2.outputs['Fac'], 0.6), nb.math('MULTIPLY', pits, -0.5))
    fg = nb.node('ShaderNodeTexNoise')
    nb.link(pos, fg.inputs['Vector'])
    fg.inputs['Scale'].default_value = 30.0
    fg.inputs['Detail'].default_value = 6
    height = nb.math('ADD', height, nb.math('MULTIPLY', fg.outputs['Fac'], 0.25))
    bp = nb.node('ShaderNodeBump')
    bp.inputs['Strength'].default_value = 0.8
    bp.inputs['Distance'].default_value = 0.01
    nb.link(height, bp.inputs['Height'])
    nb.link(col, bsdf.inputs['Base Color'])
    nb.link(rough, bsdf.inputs['Roughness'])
    nb.link(metal, bsdf.inputs['Metallic'])
    nb.link(bp.outputs['Normal'], bsdf.inputs['Normal'])
    nb.link(bsdf.outputs[0], out.inputs['Surface'])
    em = nb.node('ShaderNodeEmission')
    return m, {'base': col, 'rough': rough, 'metal': metal, 'normal': bp.outputs['Normal'], 'bsdf': bsdf, 'out': out, 'emit': em}


# ================================================================ UV + bake

def unwrap(objs, matname, angle=55, margin=0.003):
    bpy.ops.object.select_all(action='DESELECT')
    for o in objs:
        o.select_set(True)
    bpy.context.view_layer.objects.active = objs[0]
    for o in objs:
        me = o.data
        if not me.uv_layers:
            me.uv_layers.new(name="UVMap")
        idx = [i for i, s in enumerate(o.material_slots) if s.material and s.material.name == matname]
        for v in me.vertices:
            v.select = False
        for e in me.edges:
            e.select = False
        for p in me.polygons:
            s = p.material_index in idx
            p.select = s
            if s:
                for vi in p.vertices:
                    me.vertices[vi].select = True
        me.update()
    bpy.ops.object.mode_set(mode='EDIT')
    bpy.ops.mesh.select_mode(type='FACE')
    t = time.time()
    bpy.ops.uv.smart_project(angle_limit=math.radians(angle), island_margin=margin,
                             area_weight=0.0, correct_aspect=True, scale_to_bounds=False)
    bpy.ops.object.mode_set(mode='OBJECT')
    log("unwrap", matname, [o.name for o in objs], "%.1fs" % (time.time() - t))
    for o in objs:
        uv = o.data.uv_layers.active.data
        us = [d.uv.x for d in uv]
        vs = [d.uv.y for d in uv]
        nsel = sum(1 for p in o.data.polygons if p.select)
        log("  uv", o.name, "faces", len(o.data.polygons), "sel", nsel,
            "u[%.3f,%.3f] v[%.3f,%.3f]" % (min(us), max(us), min(vs), max(vs)) if us else "no loops")


def cubemap_uv(bm):
    """Equi-angular cube-map UVs from vertex directions (call while the mesh is still a
    sphere around the origin). 6 charts laid out 3x2 in [0,1]."""
    uvl = bm.loops.layers.uv.verify()
    cells = {(0, 1): (0, 0), (0, -1): (1, 0), (1, 1): (2, 0), (1, -1): (0, 1), (2, 1): (1, 1), (2, -1): (2, 1)}
    for f in bm.faces:
        c = f.calc_center_median().normalized()
        ax = max(range(3), key=lambda k: abs(c[k]))
        sg = 1 if c[ax] >= 0 else -1
        u_ax, v_ax = [k for k in range(3) if k != ax]
        cx, cy = cells[(ax, sg)]
        for l in f.loops:
            d = l.vert.co.normalized()
            a = d[ax] * sg
            u = math.atan(d[u_ax] / a) * 4 / math.pi * sg
            v = math.atan(d[v_ax] / a) * 4 / math.pi
            l[uvl].uv = ((cx + 0.5 + u * 0.45) / 3.0, (cy + 0.5 + v * 0.45) / 2.0)


def pack_uv(objs, margin=0.005):
    bpy.ops.object.select_all(action='DESELECT')
    for o in objs:
        o.select_set(True)
    bpy.context.view_layer.objects.active = objs[0]
    bpy.ops.object.mode_set(mode='EDIT')
    bpy.ops.mesh.select_all(action='SELECT')
    bpy.ops.uv.select_all(action='SELECT')
    bpy.ops.uv.average_islands_scale()
    bpy.ops.uv.pack_islands(rotate=True, margin=margin)
    bpy.ops.object.mode_set(mode='OBJECT')


def bake(objs, src_mat, socks, outdir, prefix, size=2048, samples=24, dst_mat=None, pairs=None,
         cage=0.05, maxray=0.12):
    """Bake base/rough/metal/normal of src_mat into images on dst_mat (default src_mat).
    objs: target objects (all selected) when pairs is None.
    pairs: [(hi, lo)] selected-to-active bakes into each lo (shared image)."""
    os.makedirs(outdir, exist_ok=True)
    sc = bpy.context.scene
    sc.render.engine = 'CYCLES'
    sc.cycles.samples = samples
    rb = sc.render.bake
    rb.margin = 16
    dst_mat = dst_mat or src_mat
    nt = dst_mat.node_tree
    snt = src_mat.node_tree
    tex = nt.nodes.new('ShaderNodeTexImage')
    dummy = bpy.data.images.new("_dummy", 16, 16)
    tmp = []
    targets = [lo for _, lo in pairs] if pairs else objs
    seen = set()
    for o in targets:
        for s in o.material_slots:
            m = s.material
            if m is None or m == dst_mat or m.name in seen:
                continue
            seen.add(m.name)
            nd = m.node_tree.nodes.new('ShaderNodeTexImage')
            nd.image = dummy
            m.node_tree.nodes.active = nd
            tmp.append((m.node_tree, nd))
    out, bsdf, em = socks['out'], socks['bsdf'], socks['emit']

    def do_bake(btype, first_img):
        t = time.time()
        if pairs:
            rb.use_selected_to_active = True
            rb.cage_extrusion = cage
            rb.max_ray_distance = maxray
            for i, (hi, lo) in enumerate(pairs):
                bpy.ops.object.select_all(action='DESELECT')
                hi.select_set(True)
                lo.select_set(True)
                bpy.context.view_layer.objects.active = lo
                kw = dict(type=btype, margin=16, use_clear=(i == 0), target='IMAGE_TEXTURES',
                          use_selected_to_active=True, cage_extrusion=cage, max_ray_distance=maxray)
                if btype == 'NORMAL':
                    kw['normal_space'] = 'TANGENT'
                bpy.ops.object.bake(**kw)
            rb.use_selected_to_active = False
        else:
            bpy.ops.object.select_all(action='DESELECT')
            for o in objs:
                o.select_set(True)
            bpy.context.view_layer.objects.active = objs[0]
            kw = dict(type=btype, margin=16, use_clear=True, target='IMAGE_TEXTURES')
            if btype == 'NORMAL':
                kw['normal_space'] = 'TANGENT'
            bpy.ops.object.bake(**kw)
        log("baked", prefix, btype, "%.1fs" % (time.time() - t))

    imgs = {}
    for key, noncolor in (('base', False), ('rough', True), ('metal', True)):
        img = S._img(prefix + "_" + key, size, noncolor)
        tex.image = img
        nt.nodes.active = tex
        for lk in list(out.inputs['Surface'].links):
            snt.links.remove(lk)
        snt.links.new(socks[key], em.inputs['Color'])
        snt.links.new(em.outputs[0], out.inputs['Surface'])
        sc.cycles.samples = samples
        do_bake('EMIT', img)
        imgs[key] = img
    img = S._img(prefix + "_normal", size, True)
    tex.image = img
    nt.nodes.active = tex
    for lk in list(out.inputs['Surface'].links):
        snt.links.remove(lk)
    snt.links.new(bsdf.outputs[0], out.inputs['Surface'])
    sc.cycles.samples = max(8, samples // 2)
    do_bake('NORMAL', img)
    imgs['normal'] = img

    n = size * size * 4
    r = np.empty(n, dtype=np.float32)
    mt_ = np.empty(n, dtype=np.float32)
    imgs['rough'].pixels.foreach_get(r)
    imgs['metal'].pixels.foreach_get(mt_)
    mr = np.ones(n, dtype=np.float32)
    mr[1::4] = r[0::4]
    mr[2::4] = mt_[0::4]
    mrimg = S._img(prefix + "_mr", size, True)
    mrimg.pixels.foreach_set(mr)
    paths = {}
    for key, im in (('base', imgs['base']), ('mr', mrimg), ('normal', imgs['normal'])):
        p = os.path.join(outdir, "%s_%s.png" % (prefix, key))
        im.filepath_raw = p
        im.file_format = 'PNG'
        im.save()
        paths[key] = p
    for mt, nd in tmp:
        mt.nodes.remove(nd)
    bpy.data.images.remove(dummy)
    for k in ('rough', 'metal'):
        bpy.data.images.remove(imgs[k])
    for im in (imgs['base'], mrimg, imgs['normal']):
        bpy.data.images.remove(im)
    nt.nodes.remove(tex)
    return paths


def tris(objs):
    n = 0
    per = {}
    for o in objs:
        if o.type != 'MESH':
            continue
        o.data.calc_loop_triangles()
        per[o.name] = len(o.data.loop_triangles)
        n += per[o.name]
    return n, per


def preview_set(objs, name, outdir, tmpdir, hero_dir, views, samples=96, hero_samples=128):
    """hero render + 2x2 sheet. views: list of (direction, ortho)."""
    bpy.context.view_layer.update()
    cam = bpy.context.scene.camera
    os.makedirs(outdir, exist_ok=True)
    os.makedirs(tmpdir, exist_ok=True)
    S.frame(cam, objs, hero_dir)
    S.render(os.path.join(outdir, "props_%s.png" % name), samples=hero_samples)
    ps = []
    for i, (d, ortho) in enumerate(views):
        p = os.path.join(tmpdir, "%s_v%d.png" % (name, i))
        S.frame(cam, objs, d, ortho=ortho)
        S.render(p, 640, 360, samples // 2)
        ps.append(p)
    S.compose_sheet(ps, os.path.join(outdir, "props_%s_sheet.png" % name))
    bpy.context.scene.camera.data.type = 'PERSP'

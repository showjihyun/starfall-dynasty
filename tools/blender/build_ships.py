"""Build STARFALL DYNASTY player hulls -> GLB + preview renders.

Usage (from repo root):
  "<blender.exe>" -b --factory-startup -P tools/blender/build_ships.py -- [ships...] [--no-bake] [--no-preview] [--tex 2048]
Ships: wren talon bastion mule seer (default: all)
"""
import bpy
import bmesh
import math
import os
import sys
import time
from mathutils import Vector, Matrix

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import importlib
import shiplib
importlib.reload(shiplib)
from shiplib import (V, H, L, G, GL, E, loft, ring10, seg_sym, circle_ring, lathe, box, prism, cyl,
                     sphere, wing, inset_faces, extrude_face, Ship, log)

REPO = os.path.abspath(os.path.join(HERE, "..", ".."))
MODELS = os.path.join(REPO, "crates", "client", "assets", "models")
PREVIEWS = os.path.join(HERE, "previews")
BUILD = os.path.join(HERE, "_build")


# ================================================================ shared greebles

def lerp(a, b, t):
    return a + (b - a) * t


def interp(stations, y, idx):
    """stations sorted by descending y: tuples whose [0] is y."""
    st = sorted(stations, key=lambda s: s[0])
    if y <= st[0][0]:
        return st[0][idx]
    for a, b in zip(st, st[1:]):
        if a[0] <= y <= b[0]:
            t = (y - a[0]) / (b[0] - a[0]) if b[0] != a[0] else 0
            return lerp(a[idx], b[idx], t)
    return st[-1][idx]


def nozzle(bm, exit_c, r, length, front=(0, 1, 0), n=24, rim=True, flare=1.06, cone=True):
    """Engine bell. exit_c = center of exit plane; 'front' points into the ship."""
    prof = [
        (r * 0.84, length),
        (r * 1.0, length * 0.6),
        (r * flare, length * 0.14),
        (r * (flare - 0.02), 0.0),
        (r * 0.88, 0.0),
        (r * 0.84, length * 0.08),
        (r * 0.62, length * 0.55),
        (r * 0.44, length * 0.55),
    ]
    mats = [H, H, H, G if rim else H, H, E, E]
    if cone:
        prof.append((0.0, length * 0.08))
        mats.append(E)
    else:
        prof.append((0.0, length * 0.55))
        mats.append(E)
    lathe(bm, exit_c, front, prof, n=n, mats=mats)


def canopy(bm, stations, n=10, mat=GL):
    """stations: (y, half_width, height, z_base); a station with width 0 -> point."""
    rings = []
    for y, w, h, zb in stations:
        if w <= 1e-4:
            rings.append(V((0, y, zb + h * 0.3)))
            continue
        pts = []
        for i in range(n + 1):
            a = math.pi * i / n
            pts.append(V((w * math.cos(a), y, zb + h * math.sin(a) ** 0.8)))
        rings.append(pts)
    loft(bm, rings, mat=mat)


def canopy_frame(bm, y, w, h, zb, n=10, t=0.06, mat=H):
    """An arched frame band at station y around a canopy section."""
    def arc(yy, s):
        return [V((w * s * math.cos(math.pi * i / n), yy, zb + h * s * math.sin(math.pi * i / n) ** 0.8))
                for i in range(n + 1)]
    outer0, outer1 = arc(y - t, 1.06), arc(y + t, 1.06)
    inner0, inner1 = arc(y - t, 0.97), arc(y + t, 0.97)
    ring_a = outer0 + list(reversed(inner0))
    ring_b = outer1 + list(reversed(inner1))
    loft(bm, [ring_a, ring_b], mat=mat)


def rcs(bm, p, out_dir, s=0.22, mat=H):
    """RCS thruster quad: small block with two nozzles pointing out_dir."""
    p = V(p)
    d = V(out_dir).normalized()
    box(bm, p, (s * 1.6 if abs(d.x) < 0.5 else s, s * 1.6 if abs(d.y) < 0.5 and abs(d.x) > 0.5 else s * 1.2, s), mat=mat)
    side = V((0, 1, 0)) if abs(d.y) < 0.5 else V((1, 0, 0))
    for o in (-0.35, 0.35):
        c = p + side * (o * s) + d * (s * 0.45)
        cyl(bm, c, c + d * s * 0.35, s * 0.18, s * 0.24, n=8, mat=mat)


def vent(bm, center, sx, sy, n=6, h=0.06, mat=H, axis='y'):
    """Slatted vent on a +Z facing surface."""
    c = V(center)
    box(bm, c, (sx + 0.08, sy + 0.08, h * 0.6), mat=mat)
    for i in range(n):
        t = (i + 0.5) / n - 0.5
        if axis == 'y':
            box(bm, c + V((0, t * sy, h * 0.5)), (sx, sy / n * 0.45, h), mat=mat)
        else:
            box(bm, c + V((t * sx, 0, h * 0.5)), (sx / n * 0.45, sy, h), mat=mat)


def antenna(bm, base, height, r=0.03, tip=True, lean=(0, -0.15, 1)):
    b = V(base)
    d = V(lean).normalized()
    cyl(bm, b, b + d * height * 0.15, r * 2.2, r * 1.6, n=8)
    cyl(bm, b, b + d * height, r, r * 0.5, n=6)
    if tip:
        sphere(bm, b + d * height, r * 1.8, mat=G, seg=8, rings=4)


def light(bm, p, r=0.07):
    sphere(bm, p, r, mat=G, seg=10, rings=5)


def pipe(bm, pts, r=0.05, n=8, mat=H):
    pts = [V(p) for p in pts]
    for a, b in zip(pts, pts[1:]):
        cyl(bm, a, b, r, n=n, mat=mat)
    for p in pts[1:-1]:
        sphere(bm, p, r * 1.05, mat=mat, seg=n, rings=4)


def strip(bm, p0, p1, w=0.05, h=0.03, mat=G):
    """Thin glowing strip (window band / light line)."""
    prism(bm, p0, p1, w, h, mat=mat, chamfer=0.3)


def hull_part_inset(res, pick, thickness=0.06, depth=-0.025, bm=None, mat=None):
    faces = []
    for i, band in enumerate(res['bands']):
        for k, f in enumerate(band):
            if pick(i, k):
                faces.append(f)
    if faces:
        inset_faces(bm, faces, thickness, depth, mat=mat)


# ================================================================ TALON

def build_talon(ship):
    # fuselage stations: y, hw, hh, cz
    FS = [
        (11.0, 0.32, 0.24, -0.12),
        (9.6, 0.62, 0.42, -0.08),
        (7.8, 0.92, 0.6, -0.02),
        (6.0, 1.12, 0.74, 0.04),
        (4.0, 1.28, 0.84, 0.08),
        (2.0, 1.42, 0.9, 0.1),
        (0.0, 1.5, 0.92, 0.1),
        (-2.0, 1.4, 0.9, 0.1),
        (-4.0, 1.18, 0.86, 0.12),
        (-6.0, 1.05, 0.8, 0.14),
        (-8.2, 0.95, 0.72, 0.14),
        (-9.8, 0.8, 0.6, 0.12),
    ]
    LIV_BANDS = {4, 5}

    def fus(bm):
        rings = [V((0, 12.2, -0.16))]
        for y, hw, hh, cz in FS:
            rings.append(ring10(y, hw, hh, cz, top=0.42, bot=0.5, hi=0.32, lo=-0.42,
                                ridge=0.14 if y < 8 else 0.06, keel=0.1))

        def mf(i, k):
            s = seg_sym(k)
            if s == 0 and (i - 1) in LIV_BANDS:
                return L
            if s == 1 and i in (2, 3):
                return L
            return H
        res = loft(bm, rings, matfn=mf)
        # recessed panels: sides & upper chamfers on mid bands
        hull_part_inset(res, lambda i, k: (seg_sym(k) == 2 and i in (3, 5, 7, 9)) or
                        (seg_sym(k) == 1 and i in (6, 8, 10)) or (seg_sym(k) == 4 and i in (4, 6, 8)),
                        thickness=0.07, depth=-0.03, bm=bm)
        # rear bulkhead inset
        if res['end'] is not None:
            f = res['end']
            inset_faces(bm, [f], 0.12, -0.25)
    ship.part(fus, bevel=0.03)

    # canopy (Glass) + frames
    def zb(y):
        return interp(FS, y, 3) + interp(FS, y, 2) * 0.98
    CS = [(8.6, 0.0, 0.0, zb(8.6)), (7.9, 0.36, 0.3, zb(7.9) - 0.02), (6.6, 0.6, 0.52, zb(6.6) - 0.02),
          (5.0, 0.68, 0.62, zb(5.0)), (3.6, 0.6, 0.56, zb(3.6)), (2.4, 0.38, 0.36, zb(2.4)),
          (1.4, 0.0, 0.0, zb(1.4) + 0.1)]

    def cnp(bm):
        canopy(bm, CS, n=12)
    ship.part(cnp, bevel=0.0, angle=50)

    def cframe(bm):
        for y in (6.0, 4.0):
            w = interp([(c[0], c[1]) for c in CS], y, 1)
            h = interp([(c[0], c[2]) for c in CS], y, 1)
            canopy_frame(bm, y, w, h, zb(y) - 0.02, n=12, t=0.05)
        # center spine strip of the canopy
        prism(bm, (0, 7.2, zb(7.2) + 0.42), (0, 2.6, zb(2.6) + 0.42), 0.05, 0.05)
    ship.part(cframe, bevel=0.01, segs=1)

    # engine nacelles
    NX = 1.85
    NS = [(-1.2, 0.72, 0.7, -0.05), (-2.6, 0.86, 0.82, -0.02), (-6.5, 0.9, 0.84, 0.0),
          (-9.6, 0.84, 0.78, 0.0), (-10.4, 0.76, 0.72, 0.0)]

    def nacelle(bm):
        rings = [ring10(y, hw, hh, cz, cx=NX, top=0.5, bot=0.55, hi=0.4, lo=-0.4) for y, hw, hh, cz in NS]
        res = loft(bm, rings, matfn=lambda i, k: L if (i == 2 and seg_sym(k) in (0, 1) and k < 5) else H)
        # intake mouth: inset + push back the front cap
        inset_faces(bm, [res['start']], 0.1, -0.9)
        hull_part_inset(res, lambda i, k: i == 2 and k in (2, 3), 0.08, -0.03, bm=bm)
        nozzle(bm, (NX, -12.0, 0.0), 0.66, 1.75, n=24)
        # nozzle petals / actuators
        for a in range(6):
            ang = a * math.pi / 3 + math.pi / 6
            d = V((math.cos(ang), 0, math.sin(ang)))
            p0 = V((NX, -10.2, 0)) + d * 0.78
            p1 = V((NX, -11.6, 0)) + d * 0.74
            prism(bm, p0, p1, 0.06, 0.05)
        # intake splitter lip
        box(bm, (NX, -1.05, 0.0), (0.06, 0.4, 1.3))
    ship.part(nacelle, bevel=0.03, mirror=True)
    for i, sx in enumerate((NX, -NX)):
        ship.empty("Engine_%d" % i, (sx, -12.0, 0.0), 0.62)

    # forward-swept main wings
    def wings(bm):
        st = [
            dict(x=1.9, z=-0.25, le=-1.6, te=-8.2, t=0.42),
            dict(x=3.2, z=-0.3, le=-1.0, te=-6.9, t=0.36),
            dict(x=5.3, z=-0.42, le=0.6, te=-4.2, t=0.27),
            dict(x=6.0, z=-0.46, le=1.1, te=-3.3, t=0.24),
            dict(x=7.6, z=-0.55, le=2.2, te=-1.4, t=0.18),
            dict(x=8.4, z=-0.6, le=2.8, te=-0.5, t=0.14),
        ]
        res = wing(bm, st, matfn=lambda i, k: L if i == 2 else H)
        hull_part_inset(res, lambda i, k: i in (1, 3) and k in (1, 2), 0.08, -0.02, bm=bm)
        # wingtip rail + light
        prism(bm, (8.45, 3.8, -0.6), (8.45, -1.2, -0.6), 0.1, 0.12, w1=0.08, h1=0.1)
        light(bm, (8.55, 3.8, -0.6), 0.08)
        # leading-edge root strake (LERX)
        st2 = [dict(x=1.3, z=0.0, le=3.5, te=-1.8, t=0.18), dict(x=2.3, z=-0.18, le=-0.6, te=-2.4, t=0.14)]
        wing(bm, st2)
        # flap actuator fairings under the wing
        for x in (3.6, 5.6):
            prism(bm, (x, -2.0 + (x - 3.6) * 0.5, -0.55), (x, -5.0 + (x - 3.6) * 0.8, -0.55), 0.09, 0.08)
    ship.part(wings, bevel=0.02, mirror=True)

    # canted tail fins on nacelles + ventral strakes
    def fins(bm):
        st = [dict(x=NX + 0.1, z=0.7, le=-6.6, te=-10.1, t=0.2),
              dict(x=NX + 0.75, z=2.0, le=-8.4, te=-10.6, t=0.14),
              dict(x=NX + 1.1, z=2.7, le=-9.3, te=-10.9, t=0.1)]
        wing(bm, st, matfn=lambda i, k: L if i == 1 else H)
        light(bm, (NX + 1.12, -10.9, 2.72), 0.06)
        st = [dict(x=NX + 0.2, z=-0.6, le=-7.0, te=-9.8, t=0.14),
              dict(x=NX + 0.8, z=-1.45, le=-8.6, te=-10.2, t=0.09)]
        wing(bm, st)
    ship.part(fins, bevel=0.015, mirror=True)

    # twin nose cannons
    def cannons(bm):
        x, z = 1.05, -0.42
        prism(bm, (x, 2.8, z), (x, 8.2, z), 0.26, 0.24, w1=0.2, h1=0.18)
        prism(bm, (x, 8.2, z), (x, 8.6, z), 0.16, 0.14)
        cyl(bm, (x, 8.6, z), (x, 12.3, z), 0.085, n=12)
        for y in (9.2, 10.3, 11.4):
            cyl(bm, (x, y, z), (x, y + 0.35, z), 0.12, n=12)
        cyl(bm, (x, 12.3, z), (x, 12.6, z), 0.13, 0.12, n=12)
        cyl(bm, (x, 12.58, z), (x, 12.6, z), 0.08, n=12, mat=E)
        # feed housing
        box(bm, (x - 0.05, 3.6, z - 0.22), (0.3, 1.2, 0.2))
    ship.part(cannons, bevel=0.015, mirror=True)
    ship.empty("Gun_0", (1.05, 12.62, -0.42), 0.1)
    ship.empty("Gun_1", (-1.05, 12.62, -0.42), 0.1)

    # greebles on fuselage
    def greebles(bm):
        topz = lambda y: interp(FS, y, 3) + interp(FS, y, 2) + 0.12
        vent(bm, (0.48, -1.5, topz(-1.5) - 0.12), 0.5, 1.2, n=6)
        vent(bm, (-0.48, -1.5, topz(-1.5) - 0.12), 0.5, 1.2, n=6)
        antenna(bm, (0, -3.6, topz(-3.6)), 1.2, r=0.025)
        antenna(bm, (0.3, -7.0, topz(-7.0) - 0.05), 0.6, r=0.02, tip=False)
        # dorsal hatch
        box(bm, (0, -5.2, topz(-5.2) - 0.06), (0.7, 1.4, 0.08))
        # spine conduit
        pipe(bm, [(0.42, 0.8, topz(0.8) - 0.12), (0.42, -6.5, topz(-6.5) - 0.12)], r=0.045)
        pipe(bm, [(-0.42, 0.8, topz(0.8) - 0.12), (-0.42, -6.5, topz(-6.5) - 0.12)], r=0.045)
        # rcs blocks at nose sides & tail
        for sx in (1, -1):
            rcs(bm, (sx * interp(FS, 8.4, 1) * 0.92, 8.4, 0.0), (sx, 0, 0), 0.2)
            rcs(bm, (sx * 0.55, -9.6, interp(FS, -9.6, 2) + 0.12), (0, 0, 1), 0.18)
            # glow strip window line on fuselage side
            strip(bm, (sx * interp(FS, 1.6, 1) * 1.0, 1.6, 0.25), (sx * interp(FS, -3.5, 1) * 1.0, -3.5, 0.25), 0.03, 0.03)
            light(bm, (sx * 0.2, 11.0, -0.12 - 0.2), 0.05)
        # sensor blister under nose
        sphere(bm, (0, 7.5, -0.72), 0.22, seg=12, rings=6, scale=(1, 1.8, 0.7))
        # tail cone detail
        cyl(bm, (0, -9.8, 0.12), (0, -10.6, 0.12), 0.38, 0.28, n=12)
        cyl(bm, (0, -10.6, 0.12), (0, -10.65, 0.12), 0.2, n=12, mat=G)
    ship.part(greebles, bevel=0.012, segs=1)

    # ---------------- rank parts
    def r3(bm):
        for x in (4.6,):
            # pylon
            prism(bm, (x, -1.6, -0.62), (x, -3.6, -0.62), 0.06, 0.18)
            # pod
            lathe(bm, (x, -4.2, -1.05), (0, 1, 0),
                  [(0.0, -0.3), (0.26, 0.0), (0.34, 0.5), (0.34, 3.8), (0.3, 4.4), (0.18, 4.7)], n=14)
            for dx, dz in ((-0.13, 0.13), (0.13, 0.13), (-0.13, -0.13), (0.13, -0.13)):
                c = V((x + dx, 0.5, -1.05 + dz))
                cyl(bm, c - V((0, 0.05, 0)), c + V((0, 0.08, 0)), 0.075, n=8, mat=E)
            light(bm, (x, -4.55, -1.05), 0.05)
    ship.part(r3, group="Rank3_Missile_Pods", bevel=0.015, mirror=True)

    def r6(bm):
        # layered armor slabs on fuselage flanks and nacelle tops
        for y0, y1, z in ((5.6, 2.2, 0.15), (1.8, -1.0, 0.18)):
            w0 = interp(FS, y0, 1)
            w1 = interp(FS, y1, 1)
            prism(bm, (w0 + 0.06, y0, z), (w1 + 0.06, y1, z), 0.1, 0.42, w1=0.1, h1=0.5, chamfer=0.4)
        prism(bm, (NX, -2.4, 0.95), (NX, -6.2, 0.98), 0.62, 0.08, chamfer=0.45)
        prism(bm, (NX, -6.5, 0.98), (NX, -8.8, 0.95), 0.6, 0.07, chamfer=0.45)
        # wing root glacis plate
        st = [dict(x=2.1, z=-0.02, le=-1.2, te=-4.0, t=0.12), dict(x=4.6, z=-0.16, le=-0.2, te=-2.6, t=0.1)]
        wing(bm, st, mat=L)
    ship.part(r6, group="Rank6_Armor_Plates", bevel=0.02, mirror=True)

    def r9(bm):
        topz = lambda y: interp(FS, y, 3) + interp(FS, y, 2) + 0.14
        # dorsal rail cannon
        prism(bm, (0, -7.5, topz(-7.5) + 0.25), (0, 0.6, topz(0.6) + 0.25), 0.42, 0.28, w1=0.36, h1=0.24)
        for sx in (-1, 1):
            prism(bm, (sx * 0.2, 0.6, topz(0.6) + 0.3), (sx * 0.2, 9.6, topz(0.6) + 0.42), 0.07, 0.12, chamfer=0.3)
        for y in (1.4, 3.0, 4.6, 6.2, 7.8):
            prism(bm, (0, y, topz(0.6) + 0.34), (0, y + 0.3, topz(0.6) + 0.35), 0.36, 0.2, chamfer=0.5)
            prism(bm, (0, y + 0.32, topz(0.6) + 0.35), (0, y + 0.42, topz(0.6) + 0.35), 0.3, 0.15, mat=G)
        cyl(bm, (0, 9.5, topz(0.6) + 0.36), (0, 9.7, topz(0.6) + 0.37), 0.08, n=10, mat=E)
        # capacitor drums
        for sx in (-1, 1):
            cyl(bm, (sx * 0.62, -6.8, topz(-6.8) + 0.12), (sx * 0.62, -3.0, topz(-3.0) + 0.12), 0.2, n=12)
            for y in (-6.2, -5.0, -3.8):
                cyl(bm, (sx * 0.62, y, topz(y) + 0.12), (sx * 0.62, y + 0.12, topz(y) + 0.12), 0.23, n=12, mat=G)
        # booster pods on wings
        for sx in (1, -1):
            x = sx * 6.6
            lathe(bm, (x, -5.0, -0.42), (0, 1, 0), [(0.3, 0.0), (0.34, 0.6), (0.3, 4.4), (0.12, 5.0), (0.0, 5.1)], n=14)
            nozzle(bm, (x, -5.0, -0.42), 0.26, 0.4, n=14)
    ship.part(r9, group="Rank9_Heavy_Cannon", bevel=0.015)


# ================================================================ plate helpers

def top_plate(bm, FS, y0, y1, t, top, ridge=0.0, inset=0.95, lift=0.0, mat=H, n=2):
    """Armor slab conforming to the top of a ring10 loft between stations y0>y1."""
    rings = []
    for i in range(n + 1):
        y = lerp(y0, y1, i / n)
        hw, hh, cz = interp(FS, y, 1), interp(FS, y, 2), interp(FS, y, 3)
        wt = hw * top * inset
        z0 = cz + hh + lift
        pts = [(wt, z0 - 0.05), (wt * 1.03, z0 + t * 0.6), (wt * 0.86, z0 + t), (0, z0 + ridge + t),
               (-wt * 0.86, z0 + t), (-wt * 1.03, z0 + t * 0.6), (-wt, z0 - 0.05)]
        rings.append([V((x, y, z)) for x, z in pts])
    return loft(bm, rings, mat=mat)


def side_plate(bm, FS, y0, y1, z0, z1, t, off=0.02, mat=H, n=2, bevel_in=0.12):
    """Vertical armor skirt on the right flank (+X) of a ring10 loft."""
    rings = []
    for i in range(n + 1):
        y = lerp(y0, y1, i / n)
        x = interp(FS, y, 1) + off
        rings.append([V((x, y, z0)), V((x + t, y, z0 + bevel_in)), V((x + t, y, z1 - bevel_in)), V((x, y, z1))])
    return loft(bm, rings, mat=mat)


def container(bm, c, size, mat=H, rib_mat=None, rib=0.32):
    """Corrugated cargo container (ribs on sides and top)."""
    c = V(c)
    sx, sy, sz = size
    rib_mat = mat if rib_mat is None else rib_mat
    box(bm, c, (sx, sy, sz), mat=mat)
    # end frames
    for e in (-1, 1):
        box(bm, c + V((0, e * (sy / 2 - 0.08), 0)), (sx + 0.1, 0.16, sz + 0.1), mat=H)
    nrib = int((sy - 0.4) / rib)
    for i in range(nrib):
        y = -sy / 2 + 0.2 + (i + 0.5) * (sy - 0.4) / nrib
        box(bm, c + V((0, y, 0)), (sx + 0.08, rib * 0.45, sz + 0.06), mat=rib_mat)


# ================================================================ WREN

def build_wren(ship):
    FS = [
        (9.0, 0.22, 0.17, 0.0),
        (7.2, 0.4, 0.3, 0.02),
        (5.0, 0.58, 0.44, 0.05),
        (2.6, 0.76, 0.56, 0.08),
        (0.2, 0.92, 0.64, 0.08),
        (-2.4, 1.04, 0.7, 0.06),
        (-4.8, 1.1, 0.74, 0.04),
        (-6.8, 1.08, 0.74, 0.02),
        (-8.2, 1.0, 0.72, 0.0),
    ]

    def fus(bm):
        rings = [V((0, 10.6, 0.0))]
        for y, hw, hh, cz in FS:
            rings.append(ring10(y, hw, hh, cz, top=0.5, bot=0.45, hi=0.42, lo=-0.38, ridge=0.08, keel=0.06))

        def mf(i, k):
            s = seg_sym(k)
            if i == 5 and s in (0, 1, 2):
                return L
            if i in (1, 2) and s == 1:
                return L
            return H
        res = loft(bm, rings, matfn=mf)
        hull_part_inset(res, lambda i, k: (seg_sym(k) == 2 and i in (4, 6, 8)) or
                        (seg_sym(k) == 4 and i in (5, 7)) or (seg_sym(k) == 1 and i in (6, 8)),
                        0.06, -0.025, bm=bm)
        inset_faces(bm, [res['end']], 0.08, -0.2)
        # main engine
        nozzle(bm, (0, -9.9, 0.0), 0.82, 1.9, n=28)
        for a in range(8):
            ang = a * math.pi / 4 + math.pi / 8
            d = V((math.cos(ang), 0, math.sin(ang)))
            prism(bm, V((0, -8.2, 0)) + d * 0.92, V((0, -9.6, 0)) + d * 0.9, 0.05, 0.05)
        # nose probe
        cyl(bm, (0, 10.5, 0), (0, 11.4, 0), 0.035, 0.02, n=8)
        for y in (10.75, 11.0):
            cyl(bm, (0, y, 0), (0, y + 0.06, 0), 0.06, n=8)
    ship.part(fus, bevel=0.025)
    ship.empty("Engine_0", (0, -9.9, 0.0), 0.74)

    def zb(y):
        return interp(FS, y, 3) + interp(FS, y, 2) * 0.97
    CS = [(6.6, 0.0, 0.0, zb(6.6)), (5.8, 0.26, 0.2, zb(5.8)), (4.6, 0.4, 0.36, zb(4.6)),
          (3.2, 0.46, 0.42, zb(3.2)), (1.8, 0.38, 0.34, zb(1.8)), (0.6, 0.0, 0.0, zb(0.6) + 0.12)]

    def cnp(bm):
        canopy(bm, CS, n=12)
    ship.part(cnp, bevel=0.0, angle=50)

    def cfr(bm):
        for y in (4.4, 2.8):
            w = interp([(c[0], c[1]) for c in CS], y, 1)
            h = interp([(c[0], c[2]) for c in CS], y, 1)
            canopy_frame(bm, y, w, h, zb(y) - 0.02, n=12, t=0.04)
    ship.part(cfr, bevel=0.008, segs=1)

    # forward-swept canards
    def canards(bm):
        st = [dict(x=0.45, z=0.0, le=4.0, te=2.6, t=0.12),
              dict(x=1.6, z=0.12, le=4.8, te=3.9, t=0.08),
              dict(x=2.7, z=0.24, le=5.8, te=5.15, t=0.05)]
        wing(bm, st, matfn=lambda i, k: L if i == 1 else H)
        light(bm, (2.72, 5.8, 0.24), 0.045)
    ship.part(canards, bevel=0.01, mirror=True)

    # swept main wings + small engine pods
    PX = 2.55

    def wings(bm):
        st = [dict(x=0.9, z=-0.2, le=-0.8, te=-7.6, t=0.26),
              dict(x=2.2, z=-0.24, le=-3.0, te=-7.9, t=0.2),
              dict(x=3.4, z=-0.28, le=-5.0, te=-8.1, t=0.14),
              dict(x=4.6, z=-0.32, le=-6.8, te=-8.4, t=0.09)]
        res = wing(bm, st, matfn=lambda i, k: L if i == 2 and k in (0, 1, 6) else H)
        hull_part_inset(res, lambda i, k: i == 0 and k in (1, 2), 0.06, -0.02, bm=bm)
        # down-turned tip fins
        st = [dict(x=4.55, z=-0.32, le=-6.7, te=-8.4, t=0.07),
              dict(x=4.9, z=-1.25, le=-7.9, te=-8.9, t=0.05)]
        wing(bm, st)
        light(bm, (4.92, -7.9, -1.25), 0.05)
        # engine pod
        lathe(bm, (PX, -8.4, -0.25), (0, 1, 0),
              [(0.3, 0.0), (0.36, 0.5), (0.36, 3.6), (0.3, 4.4), (0.16, 5.0), (0.0, 5.15)], n=16)
        nozzle(bm, (PX, -8.9, -0.25), 0.3, 0.6, n=16)
        cyl(bm, (PX, -4.0, -0.25), (PX, -3.7, -0.25), 0.38, n=16, mat=L)
    ship.part(wings, bevel=0.015, mirror=True)
    ship.empty("Engine_1", (PX, -8.9, -0.25), 0.27)
    ship.empty("Engine_2", (-PX, -8.9, -0.25), 0.27)

    def fin(bm):
        st = [dict(x=0.0, z=0.7, le=-4.2, te=-8.1, t=0.16),
              dict(x=0.0, z=1.5, le=-6.4, te=-8.6, t=0.1),
              dict(x=0.0, z=2.0, le=-7.7, te=-8.9, t=0.06)]
        # vertical fin: thickness along X
        rings = []
        for s in st:
            ch = s['le'] - s['te']
            t = s['t']
            pts = [(s['le'], 0), (s['le'] - ch * 0.25, t / 2), (s['le'] - ch * 0.75, t * 0.4), (s['te'], 0.01),
                   (s['te'], -0.01), (s['le'] - ch * 0.75, -t * 0.4), (s['le'] - ch * 0.25, -t / 2)]
            rings.append([V((dx, y, s['z'])) for y, dx in pts])
        loft(bm, rings, matfn=lambda i, k: L if i == 1 else H)
        light(bm, (0, -7.75, 2.02), 0.05)
    ship.part(fin, bevel=0.01)

    def guns(bm):
        for sx in (1, -1):
            x, z = sx * 0.42, -0.32
            prism(bm, (x, 3.2, z), (x, 6.4, z), 0.12, 0.12, w1=0.09, h1=0.09)
            cyl(bm, (x, 6.4, z), (x, 8.3, z), 0.04, n=8)
            cyl(bm, (x, 8.1, z), (x, 8.35, z), 0.065, n=8)
    ship.part(guns, bevel=0.01, segs=1)
    ship.empty("Gun_0", (0.42, 8.4, -0.32), 0.06)
    ship.empty("Gun_1", (-0.42, 8.4, -0.32), 0.06)

    def greebles(bm):
        topz = lambda y: interp(FS, y, 3) + interp(FS, y, 2) + 0.08
        vent(bm, (0.32, -2.6, topz(-2.6) - 0.08), 0.36, 1.1, n=6)
        vent(bm, (-0.32, -2.6, topz(-2.6) - 0.08), 0.36, 1.1, n=6)
        box(bm, (0, -0.6, topz(-0.6) - 0.05), (0.5, 0.9, 0.08))
        antenna(bm, (0.25, -1.2, topz(-1.2) - 0.05), 0.7, r=0.018)
        sphere(bm, (0, 8.0, -0.2), 0.12, seg=10, rings=5, scale=(1, 1.6, 0.6))
        for sx in (1, -1):
            strip(bm, (sx * interp(FS, 1.6, 1), 1.6, 0.2), (sx * interp(FS, -3.0, 1) * 1.0, -3.0, 0.2), 0.025, 0.025)
            rcs(bm, (sx * interp(FS, 6.4, 1) * 0.95, 6.4, 0.03), (sx, 0, 0), 0.13)
            rcs(bm, (sx * 0.55, -7.6, interp(FS, -7.6, 2) + 0.02), (0, 0, 1), 0.13)
            pipe(bm, [(sx * 0.75, -2.0, -0.42), (sx * 0.86, -7.8, -0.5)], r=0.04)
        light(bm, (0, 9.6, -0.1), 0.04)
        # belly intake scoop
        b = box(bm, (0, -1.6, -0.82), (0.9, 2.2, 0.3), taper=(0.9, 0.7))
        inset_faces(bm, [b['end']], 0.06, -0.5)
    ship.part(greebles, bevel=0.01, segs=1)

    # ranks
    def r3(bm):
        topz = lambda y: interp(FS, y, 3) + interp(FS, y, 2) + 0.08
        prism(bm, (0, -4.6, topz(-4.6) - 0.05), (0, -4.6, topz(-4.6) + 0.9), 0.08, 0.12, w1=0.05, h1=0.08)
        lathe(bm, (0, -4.6, topz(-4.6) + 0.9), (0, 0.4, 1),
              [(0.0, -0.05), (0.15, -0.02), (0.45, 0.12), (0.5, 0.16), (0.44, 0.14), (0.12, 0.04), (0.0, 0.02)], n=20)
        cyl(bm, (0, -4.6, topz(-4.6) + 0.9), (0, -4.48, topz(-4.6) + 1.2), 0.025, n=6)
        sphere(bm, (0, -4.48, topz(-4.6) + 1.22), 0.04, mat=G, seg=8, rings=4)
        for sx in (1, -1):
            antenna(bm, (sx * 0.4, -6.4, topz(-6.4) - 0.06), 0.9, r=0.015, lean=(sx * 0.3, -0.2, 1))
            # underwing missile rails
            prism(bm, (sx * 1.6, -4.0, -0.38), (sx * 1.6, -6.4, -0.38), 0.05, 0.1)
            cyl(bm, (sx * 1.6, -3.6, -0.56), (sx * 1.6, -6.6, -0.56), 0.1, n=10)
            cyl(bm, (sx * 1.6, -3.6, -0.56), (sx * 1.6, -3.2, -0.56), 0.1, 0.0, n=10, mat=L)
    ship.part(r3, group="Rank3_Sensor_Mast", bevel=0.01, segs=1)

    def r6(bm):
        # wingtip ECM pods + conformal tanks
        lathe(bm, (4.62, -8.6, -0.32), (0, 1, 0),
              [(0.0, -0.2), (0.14, 0.0), (0.2, 0.5), (0.2, 4.4), (0.12, 5.4), (0.0, 5.8)], n=14)
        cyl(bm, (4.62, -6.0, -0.32), (4.62, -5.7, -0.32), 0.22, n=14, mat=G)
        top_plate(bm, FS, 1.0, -4.0, 0.12, 0.5, ridge=0.08, inset=0.85, lift=0.02, mat=H)
        st = [dict(x=1.0, z=-0.1, le=2.5, te=-1.0, t=0.1), dict(x=1.8, z=-0.18, le=0.4, te=-1.6, t=0.07)]
        wing(bm, st, mat=L)
    ship.part(r6, group="Rank6_Wing_Pods", bevel=0.012, mirror=True)

    def r9(bm):
        # afterburner halo ring with three struts and boosters
        c = V((0, -7.6, 0.0))
        ring_pts = 40
        prof = [(1.95, -0.6), (2.1, -0.2), (2.1, 0.5), (1.95, 0.7), (1.85, 0.5), (1.85, -0.4)]
        lathe(bm, c, (0, 1, 0), prof + [prof[0]], n=ring_pts, mats=[H, L, H, H, H, H], cap_start=False, cap_end=False)
        # close the ring cross-section (last->first)
        for a in (90, 210, 330):
            r = math.radians(a)
            d = V((math.cos(r), 0, math.sin(r)))
            prism(bm, c + d * 0.95, c + d * 1.9, 0.12, 0.25, chamfer=0.4, up=(0, 1, 0))
            p = c + d * 2.35
            lathe(bm, p + V((0, -0.4, 0)), (0, 1, 0), [(0.26, 0.0), (0.3, 0.4), (0.3, 2.2), (0.0, 2.9)], n=14)
            nozzle(bm, p + V((0, -0.8, 0)), 0.24, 0.45, n=14)
            prism(bm, c + d * 2.05, p, 0.1, 0.1, up=(0, 1, 0))
        for a in range(0, 360, 30):
            r = math.radians(a + 15)
            d = V((math.cos(r), 0, math.sin(r)))
            light(bm, c + d * 2.12 + V((0, 0.2, 0)), 0.05)
    ship.part(r9, group="Rank9_Afterburner_Halo", bevel=0.012)


# ================================================================ BASTION

def build_bastion(ship):
    FS = [
        (18.0, 1.4, 0.7, -0.4),
        (15.5, 2.7, 1.1, -0.25),
        (12.0, 3.8, 1.45, -0.1),
        (7.0, 4.8, 1.75, 0.0),
        (1.0, 5.6, 1.95, 0.0),
        (-5.0, 6.1, 2.05, 0.0),
        (-11.0, 6.3, 2.05, 0.0),
        (-14.5, 6.1, 1.95, 0.0),
        (-16.2, 5.6, 1.8, 0.0),
    ]
    TOP, BOT = 0.8, 0.82

    def hull(bm):
        rings = []
        for y, hw, hh, cz in FS:
            rings.append(ring10(y, hw, hh, cz, top=TOP, bot=BOT, hi=0.45, lo=-0.5, ridge=0.0, keel=0.0))
        # prow: a narrow vertical blade wedge
        rings.insert(0, ring10(19.2, 0.35, 0.4, -0.55, top=0.6, bot=0.6, hi=0.4, lo=-0.4))

        def mf(i, k):
            s = seg_sym(k)
            if i == 1 and s in (1, 2):
                return L
            return H
        res = loft(bm, rings, matfn=mf)
        hull_part_inset(res, lambda i, k: (seg_sym(k) == 2 and i in (3, 4, 5, 6, 7)) or (seg_sym(k) == 4 and i in (3, 5, 7)) or
                        (seg_sym(k) == 3 and i in (4, 6)), 0.12, -0.05, bm=bm)
        inset_faces(bm, [res['end']], 0.25, -0.3)
    ship.part(hull, bevel=0.05)

    # layered armor plates on top (overlapping, stepping up toward the stern)
    def plates(bm):
        spans = [(17.2, 11.4, 0.0), (12.2, 5.4, 0.06), (6.2, -0.6, 0.12), (0.2, -6.8, 0.18), (-6.0, -12.6, 0.24)]
        for j, (y0, y1, lift) in enumerate(spans):
            top_plate(bm, FS, y0, y1, 0.22, TOP, inset=0.93 - j * 0.01, lift=lift, mat=L if j == 1 else H)
        for j, (y0, y1) in enumerate(((14.8, 8.2), (8.8, 1.6), (2.2, -5.0), (-4.4, -11.4), (-10.8, -15.6))):
            side_plate(bm, FS, y0, y1, -1.1, 1.0, 0.22, off=0.0 + j * 0.03, mat=L if j == 3 else H)
    ship.part(plates, bevel=0.03, mirror=False)

    def plates_mirror(bm):
        for j, (y0, y1) in enumerate(((14.8, 8.2), (8.8, 1.6), (2.2, -5.0), (-4.4, -11.4), (-10.8, -15.6))):
            side_plate(bm, FS, y0, y1, -1.1, 1.0, 0.22, off=0.0 + j * 0.03, mat=L if j == 3 else H)
        bmesh.ops.scale(bm, vec=V((-1, 1, 1)), verts=bm.verts)
        bmesh.ops.reverse_faces(bm, faces=bm.faces)
    ship.part(plates_mirror, bevel=0.03)

    # stern engine block: 4 engines (2x2)
    EX, EZ, ER = 2.3, 0.95, 0.82

    def engines(bm):
        for sx in (EX, -EX):
            for sz in (EZ, -EZ):
                box(bm, (sx, -16.6, sz), (2.0, 1.2, 1.8))
                nozzle(bm, (sx, -18.3, sz), ER, 1.8, n=24)
        box(bm, (0, -16.8, 0), (1.6, 1.0, 3.2))
        for sx in (1, -1):
            for y in (-14.0, -15.0):
                box(bm, (sx * 6.25, y, 0.0), (0.4, 0.6, 2.4))  # heat-sink fins on flanks
    ship.part(engines, bevel=0.03)
    i = 0
    for sz in (EZ, -EZ):
        for sx in (EX, -EX):
            ship.empty("Engine_%d" % i, (sx, -18.3, sz), ER * 0.88)
            i += 1

    def topz(y):
        return interp(FS, y, 3) + interp(FS, y, 2)

    def turret(bm, y, z, scale=1.0):
        lathe(bm, (0, y, z), (0, 0, 1), [(0.0, 0.0), (1.75 * scale, 0.0), (1.85 * scale, 0.25), (1.6 * scale, 0.55), (0.0, 0.55)], n=28)
        hz = z + 0.55
        rings = []
        for yy, w, h in ((y - 1.8 * scale, 1.25, 0.75), (y + 0.4 * scale, 1.4, 0.85), (y + 1.7 * scale, 0.95, 0.6)):
            pts = [(w, 0), (w, h * 0.7), (w * 0.75, h), (-w * 0.75, h), (-w, h * 0.7), (-w, 0)]
            rings.append([V((x * scale, yy, hz + zz * scale)) for x, zz in pts])
        res = loft(bm, rings)
        for sx in (0.45, -0.45):
            bz = hz + 0.4 * scale
            prism(bm, (sx * scale, y + 1.4 * scale, bz), (sx * scale, y + 2.4 * scale, bz), 0.24 * scale, 0.24 * scale)
            cyl(bm, (sx * scale, y + 2.4 * scale, bz), (sx * scale, y + 5.4 * scale, bz), 0.12 * scale, n=12)
            cyl(bm, (sx * scale, y + 5.0 * scale, bz), (sx * scale, y + 5.6 * scale, bz), 0.17 * scale, n=12)
            cyl(bm, (sx * scale, y + 5.58 * scale, bz), (sx * scale, y + 5.6 * scale, bz), 0.1 * scale, n=12, mat=E)
        # sight / sensor box
        box(bm, (0.9 * scale, y - 0.6 * scale, hz + 0.95 * scale), (0.35, 0.6, 0.3))
        strip(bm, (-0.6 * scale, y + 1.1 * scale, hz + 0.68 * scale), (0.6 * scale, y + 1.1 * scale, hz + 0.68 * scale), 0.03, 0.03)
        return [(sx * scale, y + 5.65 * scale, hz + 0.4 * scale) for sx in (0.45, -0.45)]

    guns = []

    def turrets(bm):
        guns.extend(turret(bm, 7.0, topz(7.0) + 0.2))
        # aft superfiring turret on barbette
        lathe(bm, (0, -2.4, topz(-2.4)), (0, 0, 1), [(0.0, 0.0), (2.2, 0.0), (2.3, 0.4), (2.0, 1.0), (0.0, 1.0)], n=28)
        guns.extend(turret(bm, -2.4, topz(-2.4) + 1.0, 1.1))
    ship.part(turrets, bevel=0.025)
    for i, g in enumerate(guns):
        ship.empty("Gun_%d" % i, g, 0.12)

    # bridge tower
    def bridge(bm):
        y0 = -9.5
        z0 = topz(y0) + 0.2
        rings = []
        for zz, w, d in ((0.0, 2.6, 4.4), (1.2, 2.3, 3.8), (1.25, 2.0, 3.4), (2.2, 1.7, 3.0)):
            rings.append([V((w, y0 + d / 2, z0 + zz)), V((w, y0 - d / 2, z0 + zz)), V((-w, y0 - d / 2, z0 + zz)), V((-w, y0 + d / 2, z0 + zz))])
        loft(bm, rings)
        # window band (glow) around the top tier
        strip(bm, (-1.6, y0 + 1.42, z0 + 1.75), (1.6, y0 + 1.42, z0 + 1.75), 0.04, 0.08)
        for sx in (1, -1):
            strip(bm, (sx * 1.82, y0 + 1.2, z0 + 1.75), (sx * 1.82, y0 - 1.2, z0 + 1.75), 0.04, 0.08)
        antenna(bm, (0.8, y0 - 0.6, z0 + 2.2), 2.4, r=0.04)
        antenna(bm, (-0.8, y0 - 1.0, z0 + 2.2), 1.6, r=0.03)
        sphere(bm, (-0.6, y0 + 0.6, z0 + 2.4), 0.4, seg=14, rings=7)
        box(bm, (0, y0 - 2.8, z0 - 0.05), (2.0, 1.6, 0.6))
    ship.part(bridge, bevel=0.03)

    def greebles(bm):
        for sx in (1, -1):
            for y in (13.0, 9.0, 3.5, -1.0, -6.0):
                vent(bm, (sx * 2.3, y, topz(y) + 0.22), 1.0, 1.4, n=5, axis='x')
            # rows of small windows along hull flanks under the skirts
            for y in range(-13, 12, 2):
                x = interp(FS, y, 1) * 0.84
                box(bm, (sx * x, y, -interp(FS, y, 2) * 0.98 + 0.01 - 0.05), (0.35, 0.8, 0.06), mat=H)
            rcs(bm, (sx * (interp(FS, 16.4, 1) + 0.05), 16.4, -0.3), (sx, 0, 0), 0.3)
            rcs(bm, (sx * (interp(FS, -15.6, 1) + 0.25), -15.6, 1.4), (0, 0, 1), 0.3)
            light(bm, (sx * (interp(FS, -14.8, 1) + 0.28), -14.8, 0.0), 0.12)
            pipe(bm, [(sx * 1.2, -13.5, topz(-13.5) + 0.42), (sx * 1.2, -15.6, topz(-15.6) + 0.42),
                      (sx * 1.2, -16.0, 1.4)], r=0.09)
        for y in range(-12, 16, 3):
            light(bm, (0, y + 0.5, -interp(FS, y + 0.5, 2) - 0.02), 0.08)
        # keel sensor and prow light
        sphere(bm, (0, 12.0, -1.6), 0.55, seg=16, rings=8, scale=(1, 1.4, 0.6))
        light(bm, (0, 19.3, -0.3), 0.12)
    ship.part(greebles, bevel=0.015, segs=1)

    # ranks
    def r3(bm):
        for sx in (1, -1):
            for j, y in enumerate((-12.0, -13.6)):
                c = V((sx * 3.6, y, topz(y) + 0.38))
                box(bm, c, (2.2, 1.4, 0.4))
                for a in range(4):
                    for b in range(2):
                        box(bm, c + V(((a - 1.5) * 0.48, (b - 0.5) * 0.62, 0.21)), (0.38, 0.5, 0.04), mat=L if (a + b) % 3 == 0 else H)
            # side missile rack
            prism(bm, (sx * (interp(FS, 4.0, 1) + 0.6), 6.0, 0.0), (sx * (interp(FS, 0.0, 1) + 0.6), 0.0, 0.0), 0.3, 0.5)
            for k in range(3):
                cyl(bm, (sx * (interp(FS, 3.0, 1) + 0.62), 6.4, (k - 1) * 0.3), (sx * (interp(FS, 3.0, 1) + 0.62), 6.6, (k - 1) * 0.3), 0.11, n=8, mat=E)
    ship.part(r3, group="Rank3_Missile_Battery", bevel=0.015, segs=1)

    def r6(bm):
        # armored sponsons with casemate guns
        for sx in (1, -1):
            rings = []
            for y, w in ((8.0, 0.0), (6.0, 1.4), (-4.0, 1.6), (-8.0, 0.8)):
                x = interp(FS, y, 1) - 0.4
                w = max(w, 0.3)
                pts = [(x, -1.2), (x + w, -0.7), (x + w * 1.1, 0.5), (x + w * 0.6, 1.1), (x, 1.2)]
                rings.append([V((sx * px, y, pz)) for px, pz in pts])
            loft(bm, rings)
            for y in (3.5, -0.5):
                x = interp(FS, y, 1) + 1.55
                lathe(bm, (sx * x, y, 0.0), (sx, 0, 0), [(0.0, 0.0), (0.55, 0.0), (0.5, 0.3), (0.0, 0.3)], n=16)
                cyl(bm, (sx * (x + 0.1), y, 0.0), (sx * (x + 0.1), y + 3.0, 0.0), 0.1, n=10)
                cyl(bm, (sx * (x + 0.1), y + 2.9, 0.0), (sx * (x + 0.1), y + 3.0, 0.0), 0.06, n=10, mat=E)
    ship.part(r6, group="Rank6_Side_Armor", bevel=0.03, recalc=True)

    def r9(bm):
        y0 = -9.5
        z0 = topz(y0) + 2.4
        rings = []
        for zz, w, d in ((0.0, 1.5, 2.6), (2.2, 1.1, 2.0), (2.3, 1.5, 2.6), (3.1, 1.3, 2.2), (5.0, 0.5, 0.9)):
            rings.append([V((w, y0 + d / 2, z0 + zz)), V((w, y0 - d / 2, z0 + zz)), V((-w, y0 - d / 2, z0 + zz)), V((-w, y0 + d / 2, z0 + zz))])
        loft(bm, rings)
        strip(bm, (-1.3, y0 + 1.33, z0 + 2.65), (1.3, y0 + 1.33, z0 + 2.65), 0.04, 0.12)
        antenna(bm, (0, y0, z0 + 6.0), 1.8, r=0.05, lean=(0, 0, 1))
        for sx in (1, -1):
            prism(bm, (sx * 1.1, y0, z0 + 3.6), (sx * 3.2, y0 - 0.6, z0 + 4.2), 0.08, 0.15)
            lathe(bm, (sx * 3.3, y0 - 0.6, z0 + 4.2), (sx * 0.3, 0.8, 0.4),
                  [(0.0, -0.1), (0.2, -0.05), (0.75, 0.25), (0.8, 0.3), (0.72, 0.28), (0.15, 0.05), (0.0, 0.0)], n=20)
        # bow heavy turret
        g = turret(bm, 13.3, topz(13.3) + 0.25, 0.8)
    ship.part(r9, group="Rank9_Command_Spire", bevel=0.025)


# ================================================================ MULE

def build_mule(ship):
    # cab stations (y, hw, hh, cz)
    CAB = [(14.6, 1.3, 0.9, 0.5), (14.2, 1.7, 1.25, 0.45), (13.0, 1.95, 1.45, 0.4), (10.8, 2.0, 1.5, 0.4),
           (9.2, 1.85, 1.4, 0.4), (8.6, 1.4, 1.1, 0.4)]

    def cab(bm):
        rings = [ring10(y, hw, hh, cz, top=0.7, bot=0.75, hi=0.55, lo=-0.5) for y, hw, hh, cz in CAB]
        res = loft(bm, rings, matfn=lambda i, k: L if (i == 3 and seg_sym(k) in (2, 3)) else H)
        hull_part_inset(res, lambda i, k: i == 2 and seg_sym(k) in (2,), 0.08, -0.04, bm=bm)
        inset_faces(bm, [res['start']], 0.15, -0.08)
    ship.part(cab, bevel=0.06)

    def glass(bm):
        # wraparound windscreen panels (thin slabs on the cab nose)
        y = 14.0
        for sx in (1, -1):
            rings = []
            for yy, xa, xb in ((14.45, 0.15, 1.0), (13.75, 0.15, 1.55)):
                rings.append([V((sx * xa, yy, 1.05 + (14.45 - yy) * 0.55)), V((sx * xb, yy, 1.05 + (14.45 - yy) * 0.55)),
                              V((sx * xb, yy - 0.05, 0.75 + (14.45 - yy) * 0.55)), V((sx * xa, yy - 0.05, 0.75 + (14.45 - yy) * 0.55))])
            loft(bm, rings, mat=GL)
            # side windows
            box(bm, (sx * 1.98, 12.3, 1.05), (0.06, 1.6, 0.45), mat=GL)
    ship.part(glass, bevel=0.0, angle=60)

    # spine truss
    def spine(bm):
        prism(bm, (0, 9.0, 0.4), (0, -8.5, 0.4), 0.55, 0.55, chamfer=0.35)
        for sx in (1, -1):
            for sz in (1, -1):
                prism(bm, (sx * 0.62, 9.0, 0.4 + sz * 0.62), (sx * 0.62, -8.5, 0.4 + sz * 0.62), 0.08, 0.08)
        for k in range(12):
            y = 8.6 - k * 1.42
            for sx in (1, -1):
                prism(bm, (sx * 0.64, y, 1.02), (sx * 0.64, y - 1.42, -0.22), 0.05, 0.05)
            prism(bm, (-0.64, y, 1.04), (0.64, y, 1.04), 0.05, 0.05)
            prism(bm, (-0.64, y, -0.24), (0.64, y, -0.24), 0.05, 0.05)
        pipe(bm, [(0.0, 9.0, 1.15), (0.0, -8.5, 1.15)], r=0.12)
        pipe(bm, [(0.3, 9.0, 1.1), (0.3, -8.5, 1.1)], r=0.07)
    ship.part(spine, bevel=0.01, segs=1)

    # cargo containers + clamp frames
    def cargo(bm):
        for j, y in enumerate((5.4, 0.4, -4.6)):
            for sx in (1, -1):
                container(bm, (sx * 1.92, y, 0.4), (2.4, 4.6, 2.4), mat=L if (j + (sx > 0)) % 2 == 0 else H)
            # clamp frame across the spine
            for dy in (-2.4, 2.4):
                prism(bm, (-3.25, y + dy, 1.72), (3.25, y + dy, 1.72), 0.12, 0.1)
                prism(bm, (-3.25, y + dy, -0.92), (3.25, y + dy, -0.92), 0.12, 0.1)
    ship.part(cargo, bevel=0.012, segs=1)

    # engine block
    ES = [(-8.0, 2.4, 1.7, 0.4), (-9.0, 3.0, 1.95, 0.4), (-12.0, 3.1, 1.95, 0.4), (-13.0, 2.8, 1.75, 0.4)]

    def eblock(bm):
        rings = [ring10(y, hw, hh, cz, top=0.75, bot=0.8, hi=0.5, lo=-0.5) for y, hw, hh, cz in ES]
        res = loft(bm, rings, matfn=lambda i, k: L if (i == 1 and seg_sym(k) == 2) else H)
        hull_part_inset(res, lambda i, k: i == 1 and seg_sym(k) in (0, 4), 0.1, -0.05, bm=bm)
        nozzle(bm, (0, -14.6, 0.75), 1.05, 2.0, n=28)
        for sx in (1, -1):
            nozzle(bm, (sx * 1.95, -14.3, 0.0), 0.8, 1.6, n=24)
        # radiator fins on top
        for k in range(7):
            x = (k - 3) * 0.55
            box(bm, (x, -10.5, 2.75), (0.08, 2.6, 0.9))
        box(bm, (0, -10.5, 2.36), (3.8, 2.8, 0.12))
        # fuel tanks on flanks
        for sx in (1, -1):
            cyl(bm, (sx * 3.25, -8.6, 0.4), (sx * 3.25, -12.4, 0.4), 0.55, n=16)
            for y in (-9.2, -11.8):
                cyl(bm, (sx * 3.25, y, 0.4), (sx * 3.25, y - 0.15, 0.4), 0.6, n=16, mat=L)
    ship.part(eblock, bevel=0.04)
    ship.empty("Engine_0", (0, -14.6, 0.75), 0.95)
    ship.empty("Engine_1", (1.95, -14.3, 0.0), 0.72)
    ship.empty("Engine_2", (-1.95, -14.3, 0.0), 0.72)

    # mining arms with claws and laser emitters
    TIP = []

    def arms(bm):
        sh = V((1.5, 10.2, -0.9))
        el = V((2.05, 13.4, -1.75))
        wr = V((1.45, 15.6, -1.45))
        box(bm, sh + V((0, 0, 0.2)), (0.9, 1.2, 0.6))
        sphere(bm, sh, 0.42, seg=14, rings=7)
        prism(bm, sh, el, 0.26, 0.3, w1=0.22, h1=0.26, up=(1, 0, 0))
        sphere(bm, el, 0.34, seg=14, rings=7)
        prism(bm, el, wr, 0.2, 0.24, w1=0.18, h1=0.2, up=(1, 0, 0))
        # hydraulic pistons
        cyl(bm, sh + V((0.0, 0.3, -0.38)), el + V((0.0, -0.8, -0.18)), 0.08, n=8)
        cyl(bm, el + V((0.0, -0.4, -0.3)), el + V((-0.15, 0.8, -0.32)), 0.11, n=8)
        cyl(bm, el + V((-0.15, 0.6, -0.32)), wr + V((0.0, -0.3, -0.3)), 0.06, n=8)
        # claw head
        lathe(bm, wr, (wr - el).normalized(), [(0.0, -0.1), (0.36, -0.1), (0.4, 0.2), (0.3, 0.45), (0.0, 0.45)], n=16)
        ax = (wr - el).normalized()
        for a in (90, 210, 330):
            r = math.radians(a)
            d = V((math.cos(r), 0, math.sin(r)))
            d = (d - ax * d.dot(ax)).normalized()
            p0 = wr + ax * 0.3 + d * 0.3
            p1 = p0 + ax * 0.75 + d * 0.32
            p2 = p1 + ax * 0.55 - d * 0.28
            prism(bm, p0, p1, 0.09, 0.07, w1=0.07, h1=0.06, up=d)
            prism(bm, p1, p2, 0.07, 0.06, w1=0.035, h1=0.03, up=d)
            sphere(bm, p1, 0.08, seg=8, rings=4)
        # laser emitter
        e0 = wr + ax * 0.45
        e1 = wr + ax * 1.15
        cyl(bm, e0, e1, 0.12, 0.08, n=12)
        cyl(bm, e1, e1 + ax * 0.05, 0.06, n=12, mat=G)
        TIP.append(e1 + ax * 0.06)
    ship.part(arms, bevel=0.015, mirror=True)
    ship.empty("Gun_0", TIP[0], 0.08)
    ship.empty("Gun_1", (-TIP[0].x, TIP[0].y, TIP[0].z), 0.08)

    def greebles(bm):
        topz = 0.4 + 1.5
        light(bm, (1.2, 14.65, -0.25), 0.11)
        light(bm, (-1.2, 14.65, -0.25), 0.11)
        antenna(bm, (1.0, 10.0, topz - 0.02), 1.0, r=0.03)
        vent(bm, (0, 10.4, topz - 0.04), 1.6, 1.2, n=6)
        for sx in (1, -1):
            rcs(bm, (sx * 2.0, 10.0, 0.4), (sx, 0, 0), 0.26)
            rcs(bm, (sx * 3.1, -12.8, 1.9), (0, 0, 1), 0.26)
            # hazard/beacon lights on container rack corners
            light(bm, (sx * 3.25, 7.9, 1.75), 0.08)
            light(bm, (sx * 3.25, -7.1, 1.75), 0.08)
            pipe(bm, [(sx * 1.6, 9.0, -0.6), (sx * 1.2, 8.3, -0.3), (sx * 0.7, 7.6, -0.1)], r=0.07)
        box(bm, (0, 11.6, -1.15), (1.6, 3.0, 0.3))
        strip(bm, (-1.0, 14.62, 0.3), (1.0, 14.62, 0.3), 0.03, 0.03)
    ship.part(greebles, bevel=0.015, segs=1)

    # ranks
    def r3(bm):
        topz = 0.4 + 1.5
        prism(bm, (-1.5, 12.4, topz + 0.3), (1.5, 12.4, topz + 0.3), 0.08, 0.1)
        for sx in (-1, 1):
            prism(bm, (sx * 1.2, 12.0, topz - 0.05), (sx * 1.2, 12.4, topz + 0.3), 0.06, 0.06)
        for k in range(5):
            x = (k - 2) * 0.6
            box(bm, (x, 12.55, topz + 0.3), (0.36, 0.2, 0.26))
            box(bm, (x, 12.66, topz + 0.3), (0.28, 0.03, 0.18), mat=G)
        antenna(bm, (-1.0, 9.6, topz), 1.8, r=0.03)
        lathe(bm, (0.9, 9.2, topz + 0.1), (0, 0.6, 1), [(0.0, 0.0), (0.4, 0.08), (0.45, 0.12), (0.38, 0.1), (0.0, 0.04)], n=16)
    ship.part(r3, group="Rank3_Floodlight_Rack", bevel=0.01, segs=1)

    def r6(bm):
        for j, y in enumerate((5.4, 0.4, -4.6)):
            container(bm, (0, y, 3.05), (2.4, 4.6, 2.2), mat=L if j % 2 else H)
            for dy in (-2.4, 2.4):
                prism(bm, (-1.35, y + dy, 1.8), (-1.35, y + dy, 4.2), 0.08, 0.1)
                prism(bm, (1.35, y + dy, 1.8), (1.35, y + dy, 4.2), 0.08, 0.1)
    ship.part(r6, group="Rank6_Cargo_Pods", bevel=0.012, segs=1)

    def r9(bm):
        # refinery under the spine: two big drums + smelter + pipework
        for sx in (1, -1):
            lathe(bm, (sx * 1.6, -6.0, -2.4), (0, 1, 0),
                  [(0.0, -0.3), (0.9, 0.0), (1.2, 0.6), (1.2, 10.0), (0.9, 10.6), (0.0, 10.9)], n=24)
            for y in (-4.0, -1.0, 2.0):
                cyl(bm, (sx * 1.6, y, -2.4), (sx * 1.6, y + 0.25, -2.4), 1.28, n=24, mat=L)
            for y in (-3.0, 1.0):
                prism(bm, (sx * 1.2, y, -1.4), (sx * 0.6, y, -0.5), 0.2, 0.25)
        # smelter furnace with glowing vents
        box(bm, (0, -7.0, -2.6), (1.8, 2.6, 1.6))
        for k in range(4):
            box(bm, (0, -7.9 + k * 0.6, -1.78), (1.2, 0.18, 0.04), mat=E)
        cyl(bm, (0, -7.0, -3.4), (0, -7.0, -4.2), 0.4, 0.3, n=16)
        cyl(bm, (0, -7.0, -4.2), (0, -7.0, -4.22), 0.22, n=16, mat=E)
        pipe(bm, [(1.6, 4.6, -2.0), (0.4, 5.6, -1.2), (0.4, 8.6, -0.6)], r=0.15)
        pipe(bm, [(-1.6, 4.6, -2.0), (-0.4, 5.6, -1.2), (-0.4, 8.6, -0.6)], r=0.15)
    ship.part(r9, group="Rank9_Refinery_Module", bevel=0.03)


# ================================================================ SEER (Veyr bio-tech)

def build_seer(ship):
    BS = [  # body: y, hw, hh, cz
        (11.6, 0.5, 0.3, 0.0),
        (9.6, 1.4, 0.65, 0.05),
        (6.5, 2.2, 0.95, 0.1),
        (2.5, 2.6, 1.1, 0.15),
        (-2.0, 2.5, 1.05, 0.15),
        (-6.5, 2.0, 0.9, 0.12),
        (-10.0, 1.3, 0.62, 0.1),
        (-12.2, 0.7, 0.4, 0.08),
    ]

    def body(bm):
        rings = [V((0, 12.6, -0.05))]
        for y, hw, hh, cz in BS:
            rings.append(ring10(y, hw, hh, cz, top=0.6, bot=0.65, hi=0.4, lo=-0.4, ridge=0.25, keel=0.1))
        rings.append(V((0, -13.0, 0.06)))
        loft(bm, rings)
    ship.part(body, bevel=0, subsurf=2, angle=180, weighted=False)

    WING = [
        dict(x=1.8, z=0.0, le=6.0, te=-8.5, t=0.9),
        dict(x=4.2, z=-0.15, le=3.4, te=-7.2, t=0.6),
        dict(x=6.8, z=-0.3, le=1.6, te=-5.2, t=0.42),
        dict(x=9.0, z=-0.32, le=2.6, te=-2.4, t=0.3),
        dict(x=10.6, z=-0.22, le=5.6, te=1.4, t=0.2),
        dict(x=11.2, z=-0.1, le=8.6, te=5.6, t=0.12),
    ]

    def wings(bm):
        wing(bm, WING, flat_te=0.4)
    ship.part(wings, bevel=0, subsurf=2, angle=180, weighted=False, mirror=True)

    def wz(x):
        st = [(s['x'], s['z'] + s['t'] * 0.45) for s in WING]
        return interp(st, x, 1)

    def wle(x):
        return interp([(s['x'], s['le']) for s in WING], x, 1)

    def wte(x):
        return interp([(s['x'], s['te']) for s in WING], x, 1)

    # ribs over the body (hoops) and along the wings; glowing veins between
    def ribs(bm):
        for y in (8.0, 5.0, 2.0, -1.0, -4.0, -7.0):
            hw, hh, cz = interp(BS, y, 1), interp(BS, y, 2), interp(BS, y, 3)
            n = 9
            pts = []
            for i in range(n + 1):
                a = math.radians(-25 + 230 * i / n)
                pts.append(V((hw * 0.97 * math.cos(a), y, cz + (hh + 0.1) * math.sin(a))))
            # tube along arc
            for a_, b_ in zip(pts, pts[1:]):
                cyl(bm, a_, b_, 0.13, n=8)
            for p in pts:
                sphere(bm, p, 0.13, seg=8, rings=4)
        # wing ribs: radial from root to tip region
        for f in (0.2, 0.45, 0.7):
            pts = []
            for x in (2.0, 4.0, 6.0, 8.0, 9.6):
                y = lerp(wle(x), wte(x), f)
                pts.append(V((x, y, wz(x) - 0.05)))
            for a_, b_ in zip(pts, pts[1:]):
                cyl(bm, a_, b_, 0.11, 0.08, n=8)
            for p in pts[1:-1]:
                sphere(bm, p, 0.1, seg=8, rings=4)
    ship.part(ribs, bevel=0, angle=60, weighted=False, mirror=False)

    def ribs_m(bm):
        for f in (0.2, 0.45, 0.7):
            pts = []
            for x in (2.0, 4.0, 6.0, 8.0, 9.6):
                y = lerp(wle(x), wte(x), f)
                pts.append(V((-x, y, wz(x) - 0.05)))
            for a_, b_ in zip(pts, pts[1:]):
                cyl(bm, a_, b_, 0.11, 0.08, n=8)
            for p in pts[1:-1]:
                sphere(bm, p, 0.1, seg=8, rings=4)
    ship.part(ribs_m, bevel=0, angle=60, weighted=False)

    def veins(bm):
        for f in (0.33, 0.58):
            pts = []
            for x in (2.2, 4.0, 6.0, 8.0, 9.8, 10.7):
                y = lerp(wle(x), wte(x), f)
                pts.append(V((x, y, wz(x) - 0.08)))
            for a_, b_ in zip(pts, pts[1:]):
                cyl(bm, a_, b_, 0.05, 0.04, n=6, mat=G)
        # body veins along the spine
        for sx in (0.9, -0.9):
            pts = []
            for y in (9.0, 6.0, 3.0, 0.0, -3.0, -6.0, -9.0):
                pts.append(V((sx * interp(BS, y, 1) * 0.42, y, interp(BS, y, 3) + interp(BS, y, 2) + 0.03)))
            for a_, b_ in zip(pts, pts[1:]):
                cyl(bm, a_, b_, 0.05, n=6, mat=G)
        # wing-edge glow line
        pts = [V((x, lerp(wle(x), wte(x), 0.06), wz(x) - 0.12)) for x in (4.0, 6.0, 8.0, 9.6, 10.6, 11.0)]
        for a_, b_ in zip(pts, pts[1:]):
            cyl(bm, a_, b_, 0.04, n=6, mat=G)
    ship.part(veins, bevel=0, angle=60, weighted=False, mirror=True)

    # eye canopy
    def eye(bm):
        sphere(bm, (0, 9.4, 0.62), 0.6, mat=GL, seg=20, rings=10, scale=(1.0, 2.4, 0.5))
        lathe(bm, (0, 9.4, 0.5), (0, 0, 1), [(0.0, -0.05), (0.75, -0.05), (0.7, 0.12), (0.0, 0.12)], n=20, mat=H)
    ship.part(eye, bevel=0, angle=60, weighted=False)

    # organic engine orifices
    EX = 1.25

    def engines(bm):
        for sx in (EX, -EX):
            lathe(bm, (sx, -10.0, 0.05), (0, 1, 0), [(0.55, -1.2), (0.75, 0.0), (0.9, 1.5), (0.7, 3.0), (0.0, 3.4)], n=20)
            nozzle(bm, (sx, -11.6, 0.05), 0.55, 0.7, n=20)
    ship.part(engines, bevel=0, subsurf=1, angle=60, weighted=False)
    ship.empty("Engine_0", (EX, -11.6, 0.05), 0.5)
    ship.empty("Engine_1", (-EX, -11.6, 0.05), 0.5)

    # sensor dish + antenna fins
    def sensors(bm):
        c = V((0, -4.5, interp(BS, -4.5, 3) + interp(BS, -4.5, 2) + 0.15))
        cyl(bm, c, c + V((0, 0.2, 0.9)), 0.22, 0.14, n=12)
        dc = c + V((0, 0.2, 0.9))
        lathe(bm, dc, (0, 0.55, 1), [(0.0, -0.1), (0.4, -0.05), (1.2, 0.25), (1.55, 0.5), (1.6, 0.56),
                                      (1.5, 0.5), (1.1, 0.3), (0.35, 0.06), (0.0, 0.03)], n=28,
              mats=[H, H, H, H, L, H, H, H])
        cyl(bm, dc, dc + V((0, 0.4, 0.8)), 0.04, n=8)
        sphere(bm, dc + V((0, 0.42, 0.84)), 0.1, mat=G, seg=10, rings=5)
        # swept antenna fins (dorsal, canted)
        for sx in (1, -1):
            st = [dict(x=sx * 1.0, z=0.9, le=-6.0, te=-9.6, t=0.18),
                  dict(x=sx * 1.9, z=2.4, le=-8.6, te=-10.6, t=0.1),
                  dict(x=sx * 2.3, z=3.0, le=-10.2, te=-11.2, t=0.05)]
            wing(bm, st)
            sphere(bm, (sx * 2.32, -10.2, 3.02), 0.08, mat=G, seg=8, rings=4)
    ship.part(sensors, bevel=0, subsurf=1, angle=50, weighted=False)

    def guns(bm):
        for sx in (1, -1):
            sphere(bm, (sx * 1.9, 8.2, -0.2), 0.32, seg=14, rings=7, scale=(1, 1.6, 0.8))
            sphere(bm, (sx * 1.9, 8.72, -0.2), 0.12, mat=G, seg=10, rings=5)
    ship.part(guns, bevel=0, angle=60, weighted=False)
    ship.empty("Gun_0", (1.9, 8.85, -0.2), 0.12)
    ship.empty("Gun_1", (-1.9, 8.85, -0.2), 0.12)

    # ranks
    def r3(bm):
        for sx in (1,):
            st = [dict(x=sx * 1.6, z=-0.5, le=-4.0, te=-7.8, t=0.14),
                  dict(x=sx * 2.4, z=-1.9, le=-6.4, te=-8.6, t=0.08),
                  dict(x=sx * 2.6, z=-2.5, le=-7.8, te=-9.0, t=0.04)]
            wing(bm, st)
            sphere(bm, (sx * 2.62, -7.8, -2.52), 0.08, mat=G, seg=8, rings=4)
            for x in (10.0,):
                cyl(bm, (x, wle(x) - 0.4, wz(x)), (x + 0.3, wle(x) + 1.6, wz(x) + 0.5), 0.05, 0.02, n=6)
                sphere(bm, (x + 0.3, wle(x) + 1.6, wz(x) + 0.5), 0.06, mat=G, seg=8, rings=4)
    ship.part(r3, group="Rank3_Sensor_Fins", bevel=0, subsurf=1, angle=50, weighted=False, mirror=True)

    def r6(bm):
        # carapace plates hovering over the wing roots
        st = [dict(x=2.2, z=0.62, le=3.2, te=-6.6, t=0.18),
              dict(x=4.6, z=0.38, le=1.4, te=-5.4, t=0.14),
              dict(x=6.6, z=0.12, le=0.4, te=-3.4, t=0.1)]
        wing(bm, st, mat=H, flat_te=0.5)
        st = [dict(x=1.4, z=1.2, le=6.0, te=1.0, t=0.18), dict(x=2.4, z=0.7, le=4.6, te=0.6, t=0.12)]
        wing(bm, st, flat_te=0.5)
    ship.part(r6, group="Rank6_Shell_Plates", bevel=0, subsurf=2, angle=180, weighted=False, mirror=True)

    def r9(bm):
        # crown: a crescent halo arching over the body with spines and glowing nodes
        c = V((0, -1.5, 1.2))
        pts = []
        for i in range(17):
            a = math.radians(-10 + 200 * i / 16)
            pts.append(c + V((5.2 * math.cos(a), -0.8 * math.sin(a) * 0.0 + 1.6 * math.sin(a) * 0.2, 3.4 * math.sin(a))))
        for i, (a_, b_) in enumerate(zip(pts, pts[1:])):
            r0 = 0.12 + 0.12 * math.sin(math.pi * i / 16)
            r1 = 0.12 + 0.12 * math.sin(math.pi * (i + 1) / 16)
            cyl(bm, a_, b_, r0, r1, n=10)
        for i, p in enumerate(pts[1:-1], 1):
            sphere(bm, p, 0.12 + 0.12 * math.sin(math.pi * i / 16), seg=10, rings=5)
            if i % 2 == 0:
                d = (p - c).normalized()
                cyl(bm, p, p + d * (0.6 + 0.5 * math.sin(math.pi * i / 16)), 0.08, 0.0, n=8)
            else:
                sphere(bm, p + (p - c).normalized() * 0.18, 0.11, mat=G, seg=8, rings=4)
        # struts down to the body
        for sx in (1, -1):
            cyl(bm, c + V((sx * 5.1, 0, -0.9)), V((sx * 2.2, -1.5, 0.6)), 0.12, 0.2, n=10)
        sphere(bm, c + V((0, 0, 3.4 + 0.5)), 0.32, mat=G, seg=14, rings=7)
    ship.part(r9, group="Rank9_Crown_Array", bevel=0, angle=50, weighted=False)


# ================================================================ registry

# ================================================================ species kits (docs/ART_DIRECTION.md §4)
# Separate nodes `Species_<Human|Veyr|Kharz>_Kit`; the client shows the one matching the captain's
# species. Parts are anchored by ray casts onto the finished hull so they sit on its real surface.

def _body_bounds(ship):
    lo = Vector((1e9, 1e9, 1e9))
    hi = Vector((-1e9, -1e9, -1e9))
    for o in ship.groups.get("body", []):
        for v in o.data.vertices:
            p = o.matrix_world @ v.co
            lo = Vector((min(lo.x, p.x), min(lo.y, p.y), min(lo.z, p.z)))
            hi = Vector((max(hi.x, p.x), max(hi.y, p.y), max(hi.z, p.z)))
    return lo, hi


def _hit(ship, origin, direction):
    """Nearest hit on the body from origin along direction: (point, normal) or None."""
    best = None
    for o in ship.groups.get("body", []):
        inv = o.matrix_world.inverted()
        ok, loc, nrm, _ = o.ray_cast(inv @ Vector(origin), inv.to_3x3() @ Vector(direction))
        if ok:
            p = o.matrix_world @ loc
            d = (p - Vector(origin)).length
            if best is None or d < best[0]:
                best = (d, p, (o.matrix_world.to_3x3() @ nrm).normalized())
    return None if best is None else (best[1], best[2])


def species_kits(ship, big=False):
    """big: cruiser/capital hulls — parts at half the relative size, repeated along the hull."""
    lo, hi = _body_bounds(ship)
    length = hi.y - lo.y
    u = length / (60.0 if big else 30.0)  # ~1 m on a 30 m hull
    st = dict(
        masts=((0.32, 3.2), (0.4, 2.0), (0.56, 2.6), (0.62, 1.6)) if big else ((0.32, 3.2), (0.4, 2.0)),
        hatch=(0.66, 0.24) if big else (0.66,),
        fins=(0.3, 0.45, 0.6) if big else (0.45,),
        ribs=(0.3, 0.4, 0.5, 0.6, 0.7) if big else (0.36, 0.5, 0.64),
        plates=(0.22, 0.32, 0.42, 0.52, 0.62) if big else (0.3, 0.44, 0.58),
    )
    zc = (lo.z + hi.z) / 2
    yat = lambda f: lo.y + f * length  # f: 0 stern .. 1 nose (+Y)

    def top(x, f):
        h = _hit(ship, (x, yat(f), hi.z + 10 * u), (0, 0, -1))
        return h[0] if h else None

    def seat(p, hx, hy):
        """Where a plate centred over p (half-extents hx, hy) rests: on small hulls p itself; on big
        ones sunk to its lowest corner so it hugs a rounded back, or None when a corner drops off a
        tower's or a turret roof's edge (it would hang in the air)."""
        if not big or p is None:
            return p
        zs = []
        for dx, dy in ((-hx, -hy), (hx, -hy), (-hx, hy), (hx, hy)):
            q = top(p.x + dx, (p.y + dy - lo.y) / length)
            if q is None:
                return None
            zs.append(q.z)
        if p.z - min(zs) > 2.0 * max(hx, hy):
            return None
        return Vector((p.x, p.y, min(p.z, min(zs) + 0.3 * u)))

    def side(f, z=None):
        h = _hit(ship, (hi.x + 10 * u, yat(f), zc if z is None else z), (-1, 0, 0))
        return h[0] if h else None

    # --- Human: industrial — antenna masts with running lights, side radiator fins, a top hatch.
    def human_top(bm):
        for f, hgt in st['masts']:
            p = top(0.0, f)
            if p:
                antenna(bm, p - Vector((0, 0, 0.1 * u)), hgt * u, r=0.07 * u)
        for f in st['hatch']:
            p = next((q for q in (seat(top(0.0, f + d), 0.85 * u, 0.85 * u) for d in (0, 0.03, -0.03, 0.06)) if q), None)
            if p:
                box(bm, p + Vector((0, 0, 0.05 * u)), (1.7 * u, 1.7 * u, 0.16 * u))
                box(bm, p + Vector((0, 0, 0.16 * u)), (1.1 * u, 1.1 * u, 0.1 * u), mat=L)

    def human_side(bm):
        for p in (side(f) for f in st['fins']):
            if not p:
                continue
            for k in range(3):
                c = p + Vector((0.5 * u, (k - 1) * 0.9 * u, 0))
                box(bm, c, (1.0 * u, 0.12 * u, 1.3 * u))
            light(bm, p + Vector((0.2 * u, 1.6 * u, 0.4 * u)), r=0.12 * u)

    ship.part(human_top, group="Species_Human_Kit", bevel=0.01, segs=1)
    ship.part(human_side, group="Species_Human_Kit", bevel=0.01, segs=1, mirror=True)

    # --- Veyr: bio-tech — hoop ribs following the fuselage section, a glowing spine vein.
    def section(f, n=24, win=7):
        """The hull's outer cross-section at f: n points cast in from all round (towers and
        turrets that poke out are smoothed away by a circular median), or None."""
        t = top(0.0, f)
        b = _hit(ship, (0, yat(f), lo.z - 10 * u), (0, 0, 1))
        if not (t and b):
            return None
        c = Vector((0, yat(f), (t.z + b[0].z) / 2))
        far = (hi - lo).length
        rs = []
        for k in range(n):
            a = 2 * math.pi * k / n
            d = Vector((math.cos(a), 0, math.sin(a)))
            h = _hit(ship, c + d * far, -d)
            rs.append((h[0] - c).length if h else None)
        known = [r for r in rs if r is not None]
        if len(known) < n // 2:
            return None
        rs = [r if r is not None else min(known) for r in rs]
        sm = [sorted(rs[(k + j) % n] for j in range(-(win // 2), win // 2 + 1))[win // 2] for k in range(n)]
        return [c + Vector((math.cos(2 * math.pi * k / n), 0, math.sin(2 * math.pi * k / n))) * sm[k] for k in range(n)]

    def veyr_big(bm):
        tops = []
        for f in st['ribs']:
            pts = section(f)
            if not pts:
                continue
            c = sum(pts, Vector()) / len(pts)
            pts = [p + (p - c).normalized() * 0.35 * u for p in pts]
            for p0, p1 in zip(pts, pts[1:] + pts[:1]):
                prism(bm, p0, p1, 0.4 * u, 0.3 * u, chamfer=0.4, up=(0, 1, 0))
            tops.append(pts[len(pts) // 4] + Vector((0, 0, 0.15 * u)))
        for p0, p1 in zip(tops, tops[1:]):
            # only where the deck between two ribs stays under the vein (no tower in the way)
            clear = all((lambda q: q is None or q.z < p0.z + (p1.z - p0.z) * s + 0.1 * u)(top(0.0, (p0.y + (p1.y - p0.y) * s - lo.y) / length)) for s in (0.25, 0.5, 0.75))
            if clear and abs(p1.z - p0.z) < 1.0 * u:
                strip(bm, p0, p1, w=0.22 * u, h=0.12 * u, mat=G)

    def veyr(bm):
        if big:
            return veyr_big(bm)
        tops = []
        for f in st['ribs']:
            t, s = top(0.0, f), side(f)
            b = _hit(ship, (0, yat(f), lo.z - 10 * u), (0, 0, 1))
            if not (t and s and b):
                continue
            a = s.x + 0.35 * u
            zt, zb = t.z + 0.35 * u, b[0].z - 0.35 * u
            cz, bz = (zt + zb) / 2, (zt - zb) / 2
            pts = [Vector((a * math.cos(2 * math.pi * k / 24), yat(f), cz + bz * math.sin(2 * math.pi * k / 24))) for k in range(25)]
            for p0, p1 in zip(pts, pts[1:]):
                prism(bm, p0, p1, 0.16 * u, 0.12 * u, chamfer=0.4, up=(0, 1, 0))
            tops.append(Vector((0, yat(f), zt + 0.05 * u)))
        for p0, p1 in zip(tops, tops[1:]):
            strip(bm, p0, p1, w=0.09 * u, h=0.05 * u, mat=G)

    ship.part(veyr, group="Species_Veyr_Kit", bevel=0, subsurf=0 if big else 1, angle=60, weighted=False)

    # --- Kharz: forged war hull — forward horns at the bow, riveted patch armour on the back.
    def kharz_horns(bm):
        p = top(0.18 * (hi.x - lo.x), 0.78) or side(0.78)
        if p:
            base = p + Vector((0, 0, -0.1 * u))
            tip = base + Vector((0.6 * u, 4.2 * u, 1.6 * u))
            cyl(bm, base, tip, 0.45 * u, 0.0, n=10)
            cyl(bm, base - Vector((0, 0.3 * u, 0)), base + Vector((0, 0.5 * u, 0)), 0.6 * u, 0.6 * u, n=10)

    def kharz_plates(bm):
        for f in st['plates']:
            k = 1.8 if big else 1.0
            p = next((q for q in (seat(top(0.0, f + d), 1.2 * k * u, 0.85 * k * u) for d in (0, 0.03, -0.03, 0.06)) if q), None)
            if not p:
                continue
            c = p + Vector((0, 0, 0.06 * k * u))
            box(bm, c, (2.4 * k * u, 1.7 * k * u, 0.18 * k * u))
            for sx in (-1, 1):
                for sy in (-1, 1):
                    box(bm, c + Vector((sx * 1.0 * k * u, sy * 0.65 * k * u, 0.12 * k * u)), (0.16 * k * u, 0.16 * k * u, 0.12 * k * u))

    ship.part(kharz_horns, group="Species_Kharz_Kit", bevel=0.015, segs=1, mirror=True)
    ship.part(kharz_plates, group="Species_Kharz_Kit", bevel=0.01, segs=1)


STYLES = {
    'talon': dict(paint=(0.16, 0.17, 0.19), paint2=(0.1, 0.11, 0.12), accent=(0.21, 0.22, 0.24),
                  metal=0.45, rough=0.42, spacing=(1.2, 1.6, 0.9), wear=0.9, grime=0.9, sub_amt=0.25),
    'wren': dict(paint=(0.55, 0.56, 0.56), paint2=(0.42, 0.43, 0.44), accent=(0.62, 0.62, 0.6),
                 metal=0.3, rough=0.38, spacing=(1.0, 1.4, 0.8), wear=0.7, grime=0.7, sub_amt=0.2),
    'bastion': dict(paint=(0.2, 0.22, 0.19), paint2=(0.15, 0.16, 0.14), accent=(0.26, 0.27, 0.23),
                    metal=0.3, rough=0.6, spacing=(1.8, 2.2, 1.2), wear=1.0, grime=1.1, sub_amt=0.3, seam_w=0.03),
    'mule': dict(paint=(0.42, 0.38, 0.3), paint2=(0.32, 0.3, 0.25), accent=(0.62, 0.42, 0.06),
                 accent_thresh=0.93, metal=0.35, rough=0.62, spacing=(1.4, 1.8, 1.0), wear=1.3, grime=1.4,
                 sub_amt=0.25, seam_w=0.028, grime_col=(0.06, 0.045, 0.03)),
    'seer': dict(organic=True, paint=(0.07, 0.09, 0.11), paint2=(0.12, 0.08, 0.15), accent=(0.16, 0.24, 0.24),
                 metal=0.55, rough=0.3, wear=0.25, grime=0.5, bare=(0.45, 0.5, 0.52), bump=0.7),
}
BUILDERS = {'wren': build_wren, 'talon': build_talon, 'bastion': build_bastion, 'mule': build_mule, 'seer': build_seer}


def run(name, bake=True, preview=True, tex=2048):
    t0 = time.time()
    shiplib.reset_scene()
    mats, socks = shiplib.make_materials(STYLES[name])
    ship = Ship(name, mats)
    BUILDERS[name](ship)
    species_kits(ship)
    objs = ship.finalize()
    tris, per = ship.tri_count()
    log(name, "triangles", tris, per)
    meshes = list(objs.values())
    if bake:
        shiplib.unwrap_hull(meshes)
        paths = shiplib.bake_hull(meshes, mats, socks, BUILD, name, size=tex)
        shiplib.rebuild_hull_textured(mats["Hull"], paths)
        out = os.path.join(MODELS, name + ".glb")
        shiplib.export_glb(out, meshes, ship.empties)
    if preview:
        os.makedirs(PREVIEWS, exist_ok=True)
        # preview-only tints so livery/glow read like in-game
        lv = mats["Livery"].node_tree.nodes["Principled BSDF"]
        lv.inputs['Base Color'].default_value = (0.55, 0.06, 0.04, 1)
        gl = mats["Glow"].node_tree.nodes["Principled BSDF"]
        gl.inputs['Emission Color'].default_value = (0.35, 0.8, 1.0, 1)
        cam = shiplib.setup_preview_scene()
        ranks = [o for n_, o in objs.items() if n_.startswith("Rank")]
        kits = {n_: o for n_, o in objs.items() if n_.startswith("Species_")}
        allo = list(objs.values())
        for o in ranks + list(kits.values()):
            o.hide_render = True
        shiplib.frame(cam, allo, (1.0, 1.15, 0.55))
        shiplib.render(os.path.join(PREVIEWS, name + ".png"))
        tmp = os.path.join(BUILD, "views")
        os.makedirs(tmp, exist_ok=True)
        views = []
        p = os.path.join(tmp, name + "_v0.png")
        shiplib.frame(cam, allo, (-1.0, 1.1, 0.6))
        shiplib.render(p, 640, 360, 48)
        views.append(p)
        for o in ranks:
            o.hide_render = False
        for i, (d, ortho) in enumerate((((1.0, 1.15, 0.55), False), ((-0.9, -1.2, 0.5), False), ((0.0, 0.0001, 1.0), True)), 1):
            p = os.path.join(tmp, "%s_v%d.png" % (name, i))
            shiplib.frame(cam, allo, d, ortho=ortho)
            shiplib.render(p, 640, 360, 48)
            views.append(p)
        shiplib.compose_sheet(views, os.path.join(PREVIEWS, name + "_sheet.png"))
        # Species kits: one render each, same angle (docs/ART_DIRECTION.md §4).
        for o in ranks:
            o.hide_render = True
        kviews = []
        for kname in ("Species_Human_Kit", "Species_Veyr_Kit", "Species_Kharz_Kit"):
            for n_, o in kits.items():
                o.hide_render = n_ != kname
            p = os.path.join(tmp, "%s_%s.png" % (name, kname))
            shiplib.frame(cam, allo, (1.0, 0.9, 0.7))
            shiplib.render(p, 640, 360, 48)
            kviews.append(p)
        for o in kits.values():
            o.hide_render = True
        shiplib.compose_sheet(kviews, os.path.join(PREVIEWS, name + "_species.png"), cols=3)
    log(name, "done in %.1fs" % (time.time() - t0))
    return tris


def main():
    argv = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
    bake = "--no-bake" not in argv
    preview = "--no-preview" not in argv
    tex = 2048
    if "--tex" in argv:
        tex = int(argv[argv.index("--tex") + 1])
    names = [a for a in argv if a in BUILDERS] or list(BUILDERS)
    for n in names:
        run(n, bake=bake, preview=preview, tex=tex)


if __name__ == "__main__":
    main()

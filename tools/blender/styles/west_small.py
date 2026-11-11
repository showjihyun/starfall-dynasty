"""`west_small`: the ~24 m one-seat gunship a captain of the `west` fleet starts in (styles/west.py is its capital,
styles/west_cruiser.py its cruiser). Modelled 60 units long, 1 unit = 0.4 m.

The capital's grammar at the size of a gunship: the armoured wedge bow, the pinched neck with tanks and longerons,
a ribbed engine block, ribbed pods with faction bands, slatted radiators, gunmetal with red bands and hazard
markings. What tells the size is the cockpit: a framed, faceted canopy on a tub over the bow, boarding rungs under
it, RCS blocks, intake grilles. Two fixed cannons in ribbed cheek pods reach past the prow; the pods are engine
nacelles with intakes, and a third nozzle closes the engine block. No windows, turrets, tower or hangars.
"""
import math
import random

import bmesh

from shiplib import V, H, L, G, GL, E, loft, lathe, box, prism, cyl, sphere, inset_faces, log

import build_ships as bs
from styles import west as cap
from styles.west import (Body, P, D, tag, paint, shell, rib, obox, pipe, quad, hollow, greeble, frustum, slats,
                         engine, frange, lerp, HZ, BM, DK, WT)

# The capital's paint with plates three times the size: a gunship has a few big plates, not a battleship's hundreds.
STYLE = dict(cap.STYLE, spacing=(3.0, 4.0, 2.4), seam_w=0.09, sub_scale=0.3)
TINT = cap.TINT


def materials():
    old = cap.STYLE
    cap.STYLE = STYLE
    try:
        return cap.materials()
    finally:
        cap.STYLE = old


# ================================================================ helpers

def cring(y, w, h, zb, g=0.0):
    """Faceted canopy section: sill, tumblehome pane, flat roof."""
    w, h = w + g, h + g
    return [V((x, y, zb + z)) for x, z in ((w, -0.05), (w * 0.9, h * 0.6), (w * 0.45, h), (-w * 0.45, h),
                                           (-w * 0.9, h * 0.6), (-w, -0.05))]


def bar(bm, a, b, w=0.1, h=0.08):
    prism(bm, a, b, w, h, chamfer=0.3)


def cannon(bm, x, y0, y1, z):
    """Fixed cannon: recoil sleeve, jacketed barrel, slotted muzzle brake. Muzzle at y1."""
    with tag(bm, BM):
        cyl(bm, (x, y0, z), (x, y0 + 2.8, z), 0.78, 0.62, n=14)
        cyl(bm, (x, y0 + 2.8, z), (x, y0 + 3.3, z), 0.72, n=14)
        cyl(bm, (x, y0 + 3.3, z), (x, y1 - 1.7, z), 0.36, 0.33, n=12)
        for y in frange(y0 + 4.0, y1 - 3.0, 1.15):
            cyl(bm, (x, y, z), (x, y + 0.5, z), 0.5, n=12)
        cyl(bm, (x, y1 - 1.7, z), (x, y1 - 0.02, z), 0.58, 0.53, n=12)
        # gas tube under the barrel, on two brackets
        cyl(bm, (x, y0 + 1.0, z - 0.74), (x, y1 - 4.2, z - 0.74), 0.14, n=6)
        for y in (y0 + 3.6, y1 - 4.4):
            box(bm, (x, y, z - 0.45), (0.22, 0.4, 0.7))
    with tag(bm, DK):
        for y in (y1 - 1.25, y1 - 0.6):
            box(bm, (x, y, z), (1.24, 0.24, 0.36))
    cyl(bm, (x, y1 - 0.03, z), (x, y1, z), 0.33, n=12, mat=E)


def nozzle(bm, c, r, ln, N=12):
    """The fleet's engine with its closed pieces turned right side out: the lathes come out of `engine` facing
    inwards (unseen in a two-sided render, culled in the game); the petals are open skins that face themselves."""
    n0 = len(bm.faces)
    engine(bm, c, r, ln, N=N)
    bmesh.ops.remove_doubles(bm, verts=list({v for f in list(bm.faces)[n0:] for v in f.verts}), dist=1e-5)
    bm.faces.ensure_lookup_table()
    todo = set(list(bm.faces)[n0:])
    while todo:
        comp, stack = set(), [todo.pop()]
        while stack:
            f = stack.pop()
            comp.add(f)
            for e in f.edges:
                for g in e.link_faces:
                    if g in todo:
                        todo.discard(g)
                        stack.append(g)
        if all(len(e.link_faces) == 2 for f in comp for e in f.edges):
            bmesh.ops.recalc_face_normals(bm, faces=list(comp))


def rcs(bm, p, n, s=0.6):
    """RCS quad on a surface at p (normal n): a block and two bare nozzles."""
    p, n = V(p), V(n).normalized()
    obox(bm, p + n * (s * 0.3), n, (s * 1.1, s * 2.0, s * 0.9))
    with tag(bm, BM):
        for o in (-0.5, 0.5):
            c = p + V((0, o * s, 0)) + n * (s * 0.7)
            cyl(bm, c, c + n * (s * 0.45), s * 0.2, s * 0.32, n=8)


# ================================================================ the ship

HS = [(27.0, 1.7, 1.3, -0.8), (25.4, 3.0, 2.2, -0.6), (20.0, 4.4, 3.1, -0.2), (13.0, 5.2, 3.6, 0.0),
      (6.0, 5.4, 3.7, 0.0), (4.8, 3.8, 2.9, 0.0), (1.4, 3.8, 2.9, 0.0), (0.2, 5.8, 4.0, 0.0),
      (-19.6, 6.0, 4.1, 0.0), (-21.0, 4.0, 3.2, 0.0), (-25.2, 3.2, 2.8, 0.0)]
CK = [(23.6, 1.4, 0.5, 1.9), (22.4, 2.6, 0.8, 2.1), (12.0, 3.1, 0.9, 3.2), (8.0, 3.1, 0.9, 3.35), (6.3, 2.2, 0.6, 3.2)]
CS = [(22.2, 1.0, 0.3), (18.6, 2.05, 2.2), (16.0, 2.2, 2.45), (13.2, 2.2, 2.4), (10.4, 1.95, 1.9)]
PX = 12.4
PS = [(9.1, 1.9, 1.7, -0.7), (5.0, 3.2, 3.0, -0.4), (-7.0, 3.2, 3.0, -0.4), (-8.2, 2.8, 2.65, -0.4),
      (-19.5, 2.8, 2.65, -0.4), (-20.9, 2.6, 2.45, -0.4)]
BX = 5.6
GZ = -1.0
BS = [(19.4, 0.6, 0.8, GZ), (18.2, 1.25, 1.5, GZ), (8.6, 1.35, 1.6, GZ), (7.4, 0.7, 0.9, GZ)]
SC = [(-0.4, 2.2, 0.9, 4.5), (-1.6, 2.8, 1.2, 4.6), (-8.0, 2.8, 1.2, 4.6), (-12.5, 1.6, 0.5, 4.1)]
ENGS = [((PX, -25.5, -0.4), 2.4, 4.9), ((-PX, -25.5, -0.4), 2.4, 4.9), ((0.0, -30.0, 0.0), 2.0, 5.0)]
RY = -13.4                                           # radiators
RIBS = (-1.8, -8.4, -15.3)                           # pod ribs, each 1.2 long, under the pylons
PYL = (-2.4, -9.0, -15.9)


def build(ship):
    for i in range(5):
        cap.AREA[i] = 0.0
    cap.TRIS.clear()
    rnd = random.Random(37)
    hull = Body(HS, top=0.62, bot=0.55, hi=0.5, lo=-0.5, keel=0.25)
    pod = Body(PS, cx=PX, top=0.7, bot=0.7, hi=0.6, lo=-0.6)
    bow = Body(BS, cx=BX, top=0.7, bot=0.7, hi=0.6, lo=-0.6)
    boxy = dict(top=0.82, bot=0.9, hi=0.75, lo=-0.9)
    ck = Body(CK, **boxy)
    sc = Body(SC, **boxy)

    def zb(y):
        return ck.top(y) - 0.1

    # ---------------------------------------------------------------- main hull
    def f_hull(bm):
        res = loft(bm, hull.rings())
        b = res['bands']
        paint(b[4] + b[5] + b[6], DK)                               # the neck is bare structure
        paint(b[8] + b[9], BM)                                      # tail cone
        with tag(bm, DK):
            inset_faces(bm, [res['start']], 0.2, -0.3)
        paint([res['start']], DK)
        paint([res['end']], BM)
    P(ship, f_hull, bevel=0.1)

    # ---------------------------------------------------------------- cockpit tub, hood, dorsal intake
    def f_tub(bm):
        loft(bm, ck.rings())
        # armoured hood behind the canopy, running down to the neck
        res = loft(bm, [cring(10.9, 2.02, 2.0, zb(10.9), 0.14), cring(9.6, 2.0, 1.85, zb(9.6), 0.14),
                        cring(6.8, 1.5, 0.4, zb(6.8))])
        for f in res['bands'][0][1:4] + res['bands'][1][1:4]:
            f.material_index = L
        # windscreen cowl
        box(bm, (0, 22.8, ck.top(22.8) + 0.05), (2.2, 1.4, 0.3), taper=(0.6, 0.6))
        # dorsal intake scoop on the engine block
        res = loft(bm, sc.rings())
        hollow(bm, res['start'], (0, 1, 0), 0.14, 1.0, rim_col=HZ)
        with tag(bm, BM):
            for dx in (-1.0, 0.0, 1.0):
                box(bm, (dx, -0.75, 4.5), (0.14, 0.5, 1.6))
    P(ship, f_tub, bevel=0.06)

    def f_canopy(bm):
        loft(bm, [cring(y, w, h, zb(y)) for y, w, h in CS], mat=GL)
    P(ship, f_canopy, bevel=0.0, weighted=False)

    def f_pilot(bm):
        z = zb(14.0)
        with tag(bm, DK):
            box(bm, (0, 13.0, z + 0.75), (1.5, 0.7, 1.9))                # seat back
            box(bm, (0, 14.2, z + 0.25), (1.5, 2.0, 0.5))
            box(bm, (0, 14.1, z + 0.95), (1.25, 0.8, 0.9))               # shoulders
            box(bm, (0, 18.6, zb(18.6) + 0.5), (3.0, 1.6, 1.0), taper=(0.7, 0.5))   # instrument coaming
        with tag(bm, WT):
            sphere(bm, (0, 14.15, z + 1.62), 0.5, seg=10, rings=6)       # helmet
        box(bm, (0, 14.52, z + 1.62), (0.6, 0.3, 0.26), mat=GL)          # visor
    D(ship, f_pilot)

    def f_frame(bm):
        rings = [cring(y, w, h, zb(y), 0.05) for y, w, h in CS]
        for r in rings[1:]:
            for a, b in zip(r, r[1:]):
                bar(bm, a, b, 0.15, 0.08)
        for a, b in zip(rings, rings[1:]):
            for i in range(6):
                bar(bm, a[i], b[i], 0.13, 0.08)
        for (y0, w0, h0), (y1, w1, h1) in zip(CS, CS[1:]):           # sill rails
            for sx in (1, -1):
                bar(bm, (sx * (w0 + 0.12), y0, zb(y0)), (sx * (w1 + 0.12), y1, zb(y1)), 0.2, 0.14)
    P(ship, f_frame, bevel=0.0, weighted=False)

    # ---------------------------------------------------------------- engine nacelles
    def f_pod(bm):
        res = loft(bm, pod.rings())
        hollow(bm, res['start'], (0, 1, 0), 0.2, 1.4, rim_col=HZ)    # intake
        with tag(bm, BM):
            for dz in (-0.8, 0.0, 0.8):
                box(bm, (PX, 8.7, -0.7 + dz), (3.3, 0.8, 0.14))
            lathe(bm, (PX, 7.8, -0.7), (0, 1, 0), [(0.8, 0.0), (0.65, 1.0), (0.0, 2.0)], n=10)
        paint([res['end']], BM)
        for y in RIBS:
            rib(bm, pod, y, y - 1.2, 0.28)
        rib(bm, pod, 4.9, 4.1, 0.36)
        rib(bm, pod, -18.5, -19.4, 0.36)
        for y0, y1 in ((1.6, -1.4), (-12.6, -14.9)):                # faction bands
            rib(bm, pod, y0, y1, 0.1, mat=L)
        # service door on the outer flank
        p, n = pod.surf(-5.0, 2.5, 0.2)
        with tag(bm, DK):
            obox(bm, p + n * 0.04, n, (2.0, 1.8, 0.1))
        with tag(bm, HZ):
            obox(bm, p + n * 0.06 + V((0, 0, 1.2)), n, (0.2, 1.8, 0.1))
        # outrigger with the roll thrusters
        prism(bm, (PX + 2.5, -11.0, -0.4), (PX + 4.6, -11.0, -0.4), 1.0, 0.4, w1=0.7, h1=0.32, chamfer=0.25)
        box(bm, (PX + 4.85, -11.0, -0.4), (0.9, 2.4, 1.3))
        with tag(bm, HZ):
            box(bm, (PX + 4.9, -11.0, -0.4), (0.92, 0.6, 1.34))
        with tag(bm, BM):
            for sz in (1, -1):
                for dy in (-0.75, 0.75):
                    cyl(bm, (PX + 4.85, -11.0 + dy, -0.4 + sz * 0.6), (PX + 4.85, -11.0 + dy, -0.4 + sz * 1.05), 0.17, 0.26, n=8)
            pipe(bm, [(PX + 2.7, -10.3, 0.05), (PX + 4.5, -10.3, 0.05)], r=0.08)
    P(ship, f_pod, bevel=0.07, segs=1, mirror=True)

    # ---------------------------------------------------------------- pylons, cheek pods
    def f_pylons(bm):
        for y in PYL:
            prism(bm, (5.4, y, 0.0), (PX - 2.2, y, -0.3), 1.6, 1.05, w1=1.4, h1=0.9, chamfer=0.2)
            prism(bm, (7.0, y, -0.1), (8.2, y, -0.2), 1.85, 1.25, chamfer=0.2)
        for z in (0.8, -0.8):                                        # raked braces to the nacelle nose
            prism(bm, (3.4, 3.05, z), (PX - 2.8, 3.05, z - 0.3), 0.36, 0.36, chamfer=0.2)
        res = loft(bm, bow.rings())
        paint([res['start'], res['end']], DK)
        for y in frange(17.4, 9.9, 2.5):
            rib(bm, bow, y, y - 0.7, 0.14)
        rib(bm, bow, 18.05, 17.7, 0.16, col=HZ)
        with tag(bm, BM):                                            # feed chute from the hull
            prism(bm, (4.2, 9.4, 1.5), (BX + 0.3, 10.4, 0.3), 0.5, 0.32, chamfer=0.2)
    P(ship, f_pylons, bevel=0.05, segs=1, mirror=True)

    # ---------------------------------------------------------------- cannons, plumbing, radiators: no bevel
    def f_mech(bm):
        cannon(bm, BX, 18.6, 30.0, GZ)
        with tag(bm, BM):
            for y in PYL:
                for dy in (-1.2, 1.2):
                    pipe(bm, [(5.4, y + dy, 2.3), (PX - 2.2, y + dy, 1.4)], r=0.14)
            pipe(bm, [(7.6, PYL[0], 0.85), (7.6, PYL[1], 0.85), (7.6, PYL[2], 0.85)], r=0.19, n=6)
        for sz, ln in ((1, 5.6), (-1, 2.8)):
            d = V((0.69, 0, 0.72 * sz))
            n = V((-0.72 * sz, 0, 0.69))
            base = V((4.5, RY, 2.9 * sz))
            with tag(bm, DK):
                obox(bm, base + d * (ln / 2), n, (ln, 5.4, 0.14))
            with tag(bm, BM):
                for y in frange(RY + 2.2, RY - 2.2, 1.1):
                    obox(bm, V((4.5, y, 2.9 * sz)) + d * (ln / 2), n, (ln - 0.1, 0.2, 0.36))
                for q in (0.05, 1.0):
                    obox(bm, base + d * (ln * q), n, (0.24, 5.6, 0.4))
                pipe(bm, [base + V((0, 2.9, 0)), base + V((0, -2.9, 0))], r=0.22, n=6)
            obox(bm, base + d * (ln * 0.66), n, (ln * 0.36, 5.42, 0.2), mat=L)
            with tag(bm, WT):
                obox(bm, base + d * (ln * 0.4), n, (0.16, 5.42, 0.2))
    D(ship, f_mech, mirror=True)

    def f_block_c(bm):
        # neck: collars and longerons over the bare core
        for y in (4.5, 2.2):
            rib(bm, hull, y, y - 0.6, 0.26)
        with tag(bm, BM):
            for f in (0.5, 1.5, 2.5, 3.5, 4.5, 5.5, 6.5, 7.5, 8.5, 9.5):
                pipe(bm, [hull.at(4.8, f, 0.34), hull.at(1.2, f, 0.34)], r=0.17)
        # engine block: heavy ribs, a faction band
        for y in frange(-1.4, -18.2, 4.2):
            rib(bm, hull, y, y - 1.0, 0.26)
        rib(bm, hull, -6.65, -9.55, 0.12, mat=L)
        rib(bm, hull, -15.05, -17.95, 0.12, mat=L)
        rib(bm, hull, -23.6, -24.4, 0.18)
    P(ship, f_block_c, bevel=0.06, segs=1)

    def f_eng(bm):
        c, r, ln = ENGS[0]
        nozzle(bm, c, r, ln)
    P(ship, f_eng, bevel=0.0, mirror=True, weighted=False, recalc=False)

    def f_eng_c(bm):
        c, r, ln = ENGS[2]
        nozzle(bm, c, r, ln)
    P(ship, f_eng_c, bevel=0.0, weighted=False, recalc=False)
    for i, (c, r, ln) in enumerate(ENGS):
        ship.empty("Engine_%d" % i, c, r)
    ship.empty("Gun_0", (BX, 30.0, GZ), 0.25)
    ship.empty("Gun_1", (-BX, 30.0, GZ), 0.25)

    # ---------------------------------------------------------------- armour
    def f_armor(bm):
        # prow cap, faction band with stencil edges
        shell(bm, hull, 26.7, 24.5, [0.12, 1, 2, 3, 3.9], off=0.22)
        shell(bm, hull, 24.0, 21.0, [1.05, 2, 3, 3.9], off=0.14, mat=L)
        shell(bm, hull, 24.36, 24.1, [1.05, 2, 3, 3.9], off=0.14, col=WT)
        shell(bm, hull, 20.9, 20.64, [1.05, 2, 3, 3.9], off=0.14, col=WT)
        # bow: shoulder and bilge plates
        shell(bm, hull, 20.1, 14.2, [1.06, 1.94], off=0.16)
        shell(bm, hull, 13.7, 6.6, [1.06, 1.94], off=0.16, mat=L)
        shell(bm, hull, 20.1, 14.2, [3.08, 3.92], off=0.14)
        shell(bm, hull, 13.7, 6.6, [3.08, 3.92], off=0.14, mat=L)
        shell(bm, hull, 20.1, 19.6, [2.08, 2.92], off=0.16)
        # engine block: shoulder and bilge plates between the ribs
        for i, y in enumerate(frange(-1.4, -18.2, 4.2)[:-1]):
            if i != 2:
                shell(bm, hull, y - 1.2, y - 4.0, [1.07, 1.93], off=0.16)
            shell(bm, hull, y - 1.2, y - 4.0, [3.07, 3.93], off=0.16, mat=L if i == 3 else H)
        # cheek strake, hull number, pennant blocks
        shell(bm, bow, 16.5, 10.1, [1.1, 1.9], off=0.1, mat=L)
        for y in (19.2, 18.1):
            shell(bm, hull, y, y - 0.8, [1.25, 1.75], off=0.2, col=WT)
        for y in (3.5, 2.6):
            shell(bm, pod, y, y - 0.65, [2.2, 2.8], off=0.05, col=WT)
        # nacelle plates between the ribs
        shell(bm, pod, -3.4, -6.6, [1.08, 1.92], off=0.2)
        shell(bm, pod, -3.4, -6.6, [3.08, 3.92], off=0.2)
        shell(bm, pod, -3.4, -6.6, [2.08, 2.92], off=0.2)
        shell(bm, pod, -10.0, -12.0, [0.2, 1, 1.9], off=0.16)
        shell(bm, pod, -10.0, -12.0, [3.1, 4, 4.8], off=0.16)
        shell(bm, pod, 8.2, 5.3, [0.15, 1, 1.9], off=0.18)
        shell(bm, pod, 5.75, 5.5, [0.15, 1, 1.9], off=0.2, col=WT)
        with tag(bm, HZ):
            shell(bm, hull, 6.45, 6.05, [1.1, 2, 2.9], off=0.2)
            shell(bm, hull, -0.25, -0.7, [1.1, 2, 2.9], off=0.2)
        shell(bm, sc, -8.3, -8.6, [0.2, 1, 2, 2.9], off=0.06, col=WT)
    P(ship, f_armor, bevel=0.04, segs=1, mirror=True, recalc=False)

    def f_armor_c(bm):
        shell(bm, hull, 26.6, 23.8, [9.7, 10, 10.3], off=0.3)              # prow spine
        shell(bm, hull, 24.0, 7.0, [4.5, 5, 5.5], off=0.2)                 # keel
        shell(bm, ck, 8.6, 6.6, [9.3, 10, 10.7], off=0.08)
        shell(bm, sc, -2.4, -7.4, [9.2, 10, 10.8], off=0.12)
        shell(bm, hull, -2.6, -18.0, [4.5, 5, 5.5], off=0.3, mat=L)
    P(ship, f_armor_c, bevel=0.04, segs=1, recalc=False)

    # ---------------------------------------------------------------- tanks, heat sinks, rungs, thrusters
    def f_side(bm):
        for x, z, r in ((4.3, 0.0, 1.0), (3.3, 2.65, 0.72), (3.3, -2.65, 0.72)):
            sphere(bm, (x, 3.05, z), r, seg=12, rings=6)
        # heat sinks on plinths on the engine block
        for sz in (1, -1):
            zt = hull.top(-15.9) * sz
            box(bm, (2.5, -15.9, zt), (2.8, 3.2, 0.5))
            slats(bm, (2.5, -15.9, zt + 0.25 * sz), (0, 0, sz), 2.5, 3.0, 5, h=0.3, t=0.3)
        # boarding rungs and the rescue panel under the cockpit
        with tag(bm, BM):
            for f in (1.2, 1.45, 1.7):
                p, n = hull.surf(15.2, f, 0.16)
                obox(bm, p + n * 0.12, n, (0.12, 0.9, 0.24))
        p, n = hull.surf(17.0, 1.5, 0.16)
        with tag(bm, HZ):
            obox(bm, p + n * 0.02, n, (1.0, 1.0, 0.06))
        with tag(bm, DK):
            obox(bm, p + n * 0.03, n, (0.7, 0.7, 0.07))
        # RCS blocks
        rcs(bm, *hull.surf(22.5, 2.5, 0.14))
        rcs(bm, *hull.surf(25.0, 0.6, 0.2), s=0.5)
        rcs(bm, *hull.surf(23.0, 4.3, 0.14), s=0.5)
        rcs(bm, *pod.surf(-5.0, 0.5, 0.0))
        rcs(bm, *pod.surf(-5.0, 4.5, 0.0))
        rcs(bm, *pod.surf(-17.5, 2.5, 0.0))
        rcs(bm, *pod.surf(-17.5, 4.5, 0.0))
        slats(bm, (PX, 2.9, pod.top(2.9)), (0, 0, 1), 2.6, 1.5, 4, h=0.18, t=0.18)   # nacelle bleed vents
        slats(bm, (PX, -17.0, pod.top(-17.0)), (0, 0, 1), 2.2, 1.4, 4, h=0.18, t=0.18)
        # louvres in the block's shoulder and a sensor box on the cheek
        p, n = hull.surf(-3.9, 2.5)
        slats(bm, p, n, 2.6, 2.0, 5, h=0.14, t=0.14)
        box(bm, (2.9, 24.6, hull.bot(24.6) + 0.1), (0.7, 1.6, 0.5), taper=(0.6, 0.5))
    D(ship, f_side, mirror=True)

    def f_small(bm):
        # running lights
        for p in ((PX + 5.32, -11.0, -0.4), (PX + 3.6, 4.5, -0.4), (3.3, 25.3, -0.6), (8.45, RY, 7.05)):
            sphere(bm, p, 0.16, mat=G, seg=8, rings=4)
    D(ship, f_small, mirror=True)

    def f_lamps(bm):                                                 # single quads, wound by hand
        for y in (0.1, -13.75):
            p, n = pod.surf(y, 2.5, 0.1)
            quad(bm, p, n, (0, 1, 0), 0.14, 1.6)
        for f in (2.2, 2.8):
            p, n = hull.surf(-8.1, f, 0.12)
            quad(bm, p, n, (0, 1, 0), 0.12, 1.4)
        quad(bm, (0.45, 17.78, zb(18.6) + 0.55), (0, -1, 0), (1, 0, 0), 0.5, 0.8)   # instrument glow
    P(ship, f_lamps, bevel=0.0, mirror=True, weighted=False, recalc=False)

    def f_greeb(bm):
        greeble(bm, hull, -2.6, -18.0, 0.55, 0.98, 70, rnd, smax=1.0, hmax=0.3)
        greeble(bm, hull, -2.6, -18.0, 4.1, 4.9, 60, rnd, smax=1.1, hmax=0.3)
        greeble(bm, hull, 20.0, 7.0, 4.05, 4.4, 40, rnd, smax=0.9, hmax=0.25)
        greeble(bm, hull, 4.3, 1.6, 0.2, 4.8, 60, rnd, smax=0.9, hmax=0.4)
        greeble(bm, hull, -21.2, -23.4, 0.2, 4.8, 50, rnd, smax=0.8, hmax=0.3)
        greeble(bm, pod, 4.0, -18.0, 0.1, 0.9, 60, rnd, smax=1.0, hmax=0.3)
        greeble(bm, pod, 4.0, -18.0, 4.1, 4.9, 50, rnd, smax=1.0, hmax=0.3)
        greeble(bm, pod, 4.0, -18.0, 7.05, 7.95, 60, rnd, smax=1.0, hmax=0.3)
        greeble(bm, ck, 21.0, 12.0, 2.1, 2.9, 30, rnd, smax=0.6, hmax=0.2)
        greeble(bm, sc, -2.0, -8.0, 2.1, 2.9, 30, rnd, smax=0.9, hmax=0.25)
    P(ship, f_greeb, bevel=0.0, mirror=True, weighted=False, recalc=False)

    # ---------------------------------------------------------------- centre line
    def f_center(bm):
        box(bm, (0, 27.25, -0.85), (0.5, 0.9, 2.4))                  # armoured stem
        box(bm, (0, 27.0, 0.36), (3.0, 0.7, 0.3))
        with tag(bm, BM):
            for x in (0.75, -0.75):                                  # pitot probes
                cyl(bm, (x, 26.9, -1.5), (x, 29.2, -1.5), 0.08, 0.03, n=6)
        # chin sensor
        z = hull.bot(22.6)
        cyl(bm, (0, 22.6, z + 0.2), (0, 22.6, z - 0.3), 0.85, 0.75, n=12)
        sphere(bm, (0, 22.6, z - 0.35), 0.62, mat=GL, seg=12, rings=6)
        # masts behind the hood
        with tag(bm, BM):
            bs.antenna(bm, (0, 7.6, ck.top(7.6) + 0.2), 3.2, r=0.07, lean=(0, -0.25, 1))
            for x in (3.0, -3.0):
                bs.antenna(bm, (x, -11.2, hull.top(-11.2) - 0.05), 1.8, r=0.05, tip=False)
        # belly: louvres under the neck end of the block, ammunition hatch aft
        slats(bm, (0, -3.9, hull.bot(-3.9) + 0.06), (0, 0, -1), 3.0, 2.2, 5, h=0.16, t=0.16)
        zb_ = hull.bot(-16.2)
        with tag(bm, DK):
            box(bm, (0, -16.2, zb_ - 0.02), (3.0, 3.4, 0.1))
        with tag(bm, HZ):
            for dy in (-1.8, 1.8):
                box(bm, (0, -16.2 + dy, zb_ - 0.03), (3.2, 0.24, 0.12))
        # stern: collar struts round the centre nozzle
        with tag(bm, BM):
            for a in (0.25, 0.75, 1.25, 1.75):
                d = V((math.cos(a * math.pi), 0, math.sin(a * math.pi)))
                pipe(bm, [V((0, -20.6, 0)) + d * 3.6, V((0, -25.4, 0)) + d * 2.25], r=0.16, n=6)
    P(ship, f_center, bevel=0.0, weighted=False)

    tot = sum(cap.AREA) or 1.0
    log("west_small parts", cap.TRIS)
    log("west_small area %.0f" % tot, "hull %.0f%% livery %.1f%% glow %.1f%% glass %.1f%% core %.1f%%" % tuple(100 * a / tot for a in cap.AREA))

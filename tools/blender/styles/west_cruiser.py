"""`west_cruiser`: the ~150 m gun cruiser of the `west` fleet (styles/west.py is its capital). Modelled 60 units
long, 1 unit ~ 2.5 m.

The capital's little sister, not a scaled copy: the same armoured wedge bow, pinched neck with tanks and longerons,
ribbed pods with faction bands, gunmetal with red bands and hazard markings -- but one superstructure tier under a
low, wide bridge, three twin turrets (two superfiring forward, one aft) and two fixed cheek lances, a single hangar
(the mouth of the ventral gondola, under the neck), and the pods moved aft to become engine nacelles: two big main
nozzles in a wide engine block and one on each nacelle. Everything is bigger for the hull than on the capital
(plates, windows, point defence), because the hull is a third of the size.
"""
import math
import random

from shiplib import V, H, L, G, GL, loft, lathe, box, prism, cyl, sphere, inset_faces, log

from styles import west as cap
from styles.west import (Body, P, D, tag, paint, shell, rib, obox, pipe, quad, hollow, greeble, frustum, slats,
                         turret, engine, frange, HZ, BM, DK, WT)

# The capital's paint, with plates half as many again across the hull: the same plate, a smaller ship.
STYLE = dict(cap.STYLE, spacing=(1.5, 2.0, 1.2), seam_w=0.06, sub_scale=0.4)
TINT = cap.TINT


def materials():
    old = cap.STYLE
    cap.STYLE = STYLE
    try:
        return cap.materials()
    finally:
        cap.STYLE = old


# ================================================================ helpers

def windows(bm, body, y0, y1, fs, rnd, step=0.5, lit=0.6, grow=0.0, w=0.26, h=0.11):
    """The capital's window rows at this hull's scale: a window is 0.65 m here too, so it is far bigger on the hull."""
    cap.windows(bm, body, y0, y1, fs, rnd, step=step, lit=lit, grow=grow, w=w, h=h)


def pd(bm, p, n, s=1.0):
    """Point-defence mount, close enough to read: base, drum, twin barrels trained forward (aft with s < 0)."""
    p, n = V(p), V(n).normalized()
    fwd = 1.0 if s > 0 else -1.0
    s = abs(s)
    f = V((0, fwd, 0)) - n * (n.y * fwd)
    f.normalize()
    t = f.cross(n)
    frustum(bm, p, n, 0.8 * s, 1.0 * s, 0.14 * s)
    cyl(bm, p + n * (0.1 * s), p + n * (0.42 * s), 0.27 * s, 0.22 * s, n=8)
    obox(bm, p + n * (0.36 * s) + f * (0.16 * s), n, (0.3 * s, 0.5 * s, 0.2 * s), yv=f)
    with tag(bm, BM):
        for sx in (1, -1):
            a = p + n * (0.38 * s) + t * (sx * 0.08 * s) + f * (0.3 * s)
            cyl(bm, a, a + f * (0.75 * s), 0.035 * s, n=4)


def strut(bm, a, b, w=0.5, h=0.3):
    prism(bm, a, b, w, h, chamfer=0.2)


# ================================================================ the ship

HS = [(28.8, 1.5, 1.2, -0.5), (27.4, 2.4, 2.1, -0.35), (23.0, 3.4, 2.7, -0.1), (16.0, 4.0, 2.9, 0.0),
      (11.0, 4.2, 3.0, 0.0), (9.9, 3.1, 2.5, 0.0), (6.9, 3.1, 2.5, 0.0), (5.7, 5.2, 3.8, 0.0),
      (-9.0, 5.4, 3.9, 0.0), (-10.8, 4.4, 3.4, 0.0), (-13.6, 4.4, 3.4, 0.0), (-15.0, 6.0, 4.4, 0.0),
      (-25.6, 6.0, 4.4, 0.0), (-26.5, 5.4, 4.0, 0.0)]
PX = 9.7
PS = [(-6.6, 1.2, 1.1, 0.0), (-7.8, 1.9, 1.75, 0.0), (-26.0, 1.9, 1.75, 0.0), (-27.0, 1.62, 1.5, 0.0)]
T1 = [(4.4, 2.5, 0.7, 4.35), (3.4, 3.4, 0.8, 4.45), (-7.2, 3.4, 0.8, 4.45), (-8.2, 2.7, 0.7, 4.35)]
T2 = [(2.9, 1.8, 0.4, 6.2), (2.2, 2.6, 0.52, 6.25), (-2.4, 2.6, 0.52, 6.25), (-3.1, 1.9, 0.4, 6.2)]
BX = 4.1
BS = [(22.2, 0.45, 0.6, -0.5), (21.0, 0.9, 1.2, -0.5), (13.4, 1.0, 1.25, -0.5), (12.3, 0.5, 0.7, -0.5)]
GS = [(6.9, 1.7, 0.75, -4.2), (6.3, 2.3, 0.95, -4.3), (-6.5, 2.3, 0.95, -4.3), (-8.6, 1.5, 0.6, -3.9)]
ENGS = [((3.05, -30.0, 0.0), 2.6), ((-3.05, -30.0, 0.0), 2.6), ((PX, -29.2, 0.0), 1.5), ((-PX, -29.2, 0.0), 1.5)]


def build(ship):
    for i in range(5):
        cap.AREA[i] = 0.0
    cap.TRIS.clear()
    rnd = random.Random(23)
    hull = Body(HS, top=0.62, bot=0.55, hi=0.5, lo=-0.5, keel=0.25)
    pod = Body(PS, cx=PX, top=0.7, bot=0.7, hi=0.6, lo=-0.6)
    boxy = dict(top=0.82, bot=0.9, hi=0.75, lo=-0.9)
    t1 = Body(T1, **boxy)
    t2 = Body(T2, top=0.85, bot=0.7, hi=0.6, lo=-0.5)
    bow = Body(BS, cx=BX, top=0.7, bot=0.7, hi=0.6, lo=-0.6)
    gon = Body(GS, top=0.9, bot=0.75, hi=0.9, lo=-0.5)

    # ---------------------------------------------------------------- main hull
    def f_hull(bm):
        res = loft(bm, hull.rings())
        b = res['bands']
        paint(b[4] + b[5] + b[6], DK)                           # the neck is bare structure
        with tag(bm, DK):
            inset_faces(bm, [res['start']], 0.12, -0.25)
        paint([res['start']], DK)
        with tag(bm, BM):
            inset_faces(bm, [res['end']], 0.3, -0.5)
        paint([res['end']], BM)
    P(ship, f_hull, bevel=0.06)

    # ---------------------------------------------------------------- superstructure: one tier, a low bridge
    def f_tiers(bm):
        loft(bm, t1.rings())
        loft(bm, t2.rings())
        box(bm, (0, -0.2, 5.5), (3.6, 4.4, 0.6))                    # the bridge neck
        box(bm, (0, 2.915, 6.27), (3.0, 0.03, 0.16), mat=G)         # bridge windows: one bright band
        box(bm, (0, 2.6, 5.55), (2.2, 0.8, 0.5), taper=(0.8, 0.7))
        box(bm, (0, -4.6, 5.55), (2.4, 2.2, 0.6))                   # deckhouse behind the bridge
        with tag(bm, BM):                                           # exhaust stacks
            for sx in (0.75, -0.75):
                box(bm, (sx, -4.8, 6.4), (0.8, 1.3, 1.2), taper=(0.8, 1.0))
        with tag(bm, DK):
            for sx in (0.75, -0.75):
                box(bm, (sx, -4.8, 7.01), (0.6, 1.0, 0.03))
        # barbette deckhouses for the superfiring and aft turrets
        prism(bm, (0, 16.2, hull.top(15.0) + 0.55), (0, 11.2, hull.top(12.0) + 0.55), 2.2, 0.75, chamfer=0.2)
        prism(bm, (0, -8.4, 4.0), (0, -13.4, 4.0), 2.3, 0.75, chamfer=0.2)
        # the gondola: its bow face is the hangar mouth
        res = loft(bm, gon.rings())
        inset_faces(bm, [res['bands'][1][k] for k in (2, 7)], 0.16, -0.06)
        hollow(bm, res['start'], (0, 1, 0), 0.1, 2.2, floor_col=WT)
        box(bm, (0, 7.3, -4.88), (2.9, 1.5, 0.16))                  # deck lip
        with tag(bm, HZ):
            box(bm, (0, 8.02, -4.88), (2.9, 0.1, 0.18))
        for d in (0.5, 1.1, 1.7):
            box(bm, (0, 6.9 - d, -3.6), (2.2, 0.06, 0.04), mat=G)   # ceiling light bars
        for dx in (-0.8, 0.8):
            box(bm, (dx, 6.6, -4.79), (0.05, 2.6, 0.04), mat=G)     # deck guide lights
    P(ship, f_tiers, bevel=0.035)

    # ---------------------------------------------------------------- engine nacelles
    def f_pod(bm):
        res = loft(bm, pod.rings())
        hollow(bm, res['start'], (0, 1, 0), 0.1, 0.7, rim_col=BM)   # intake
        with tag(bm, BM):
            for dz in (-0.5, 0.0, 0.5):
                box(bm, (PX, -6.9, dz), (1.9, 0.5, 0.07))
        paint([res['end']], BM)
        for y in frange(-9.6, -24.6, 2.5):
            rib(bm, pod, y, y - 0.5, 0.16)
        rib(bm, pod, -7.85, -8.3, 0.24)
        rib(bm, pod, -25.5, -25.95, 0.24)
        for y in (-10.1, -12.6, -22.6):                             # faction bands between ribs
            rib(bm, pod, y - 0.12, y - 1.88, 0.06, mat=L)
        # service doors along the outer flank
        for y in (-16.1, -18.6):
            p, n = pod.surf(y, 2.5)
            with tag(bm, DK):
                obox(bm, p + n * 0.03, n, (1.1, 1.3, 0.07))
            with tag(bm, HZ):
                obox(bm, p + n * 0.045 + V((0, 0, 0.66)), n, (0.1, 1.3, 0.07))
    P(ship, f_pod, bevel=0.03, segs=1, mirror=True)

    # ---------------------------------------------------------------- pylons hull -> nacelle, radiators, cheeks
    def f_pylons(bm):
        for y, x0 in ((-9.0, 4.6), (-17.2, 5.6), (-23.4, 5.6)):
            prism(bm, (x0, y, 0.0), (PX - 1.5, y, 0.0), 1.1, 0.75, w1=0.9, h1=0.6, chamfer=0.2)
            prism(bm, (6.4, y, 0.0), (7.3, y, 0.0), 1.25, 0.88, chamfer=0.2)
            with tag(bm, BM):
                for dy in (-0.9, 0.9):
                    pipe(bm, [(x0 - 0.2, y + dy, 2.3), (PX - 1.2, y + dy, 1.0)], r=0.1)
                    pipe(bm, [(x0 - 0.2, y + dy, -2.3), (PX - 1.2, y + dy, -1.0)], r=0.1)
        for z in (0.55, -0.55):                                     # raked braces to the nacelle nose
            strut(bm, (5.1, -4.4, z), (8.0, -8.9, z), 0.26, 0.26)
        with tag(bm, BM):                                           # fuel lines along the gap
            pipe(bm, [(6.9, -9.0, 0.5), (6.9, -17.2, 0.5), (6.9, -23.4, 0.5)], r=0.13, n=6)
        # cheek pods with the fixed lances
        res = loft(bm, bow.rings())
        paint([res['start']], DK)
        for y in frange(20.2, 14.2, 2.0):
            rib(bm, bow, y, y - 0.4, 0.1)
        with tag(bm, BM):
            cyl(bm, (BX, 21.6, -0.5), (BX, 23.6, -0.5), 0.36, 0.28, n=12)
            cyl(bm, (BX, 23.6, -0.5), (BX, 27.0, -0.5), 0.17, 0.14, n=10)
            cyl(bm, (BX, 25.0, -0.5), (BX, 25.4, -0.5), 0.24, n=10)
            cyl(bm, (BX, 27.0, -0.5), (BX, 27.5, -0.5), 0.22, n=10)
        cyl(bm, (BX, 27.48, -0.5), (BX, 27.51, -0.5), 0.12, n=8, mat=G)
        # radiators off the waist
        for sz in (1, -1):
            d = V((0.78, 0, 0.62 * sz))
            n = V((-0.62 * sz, 0, 0.78))
            base = V((3.3, -12.2, 2.5 * sz))
            with tag(bm, DK):
                obox(bm, base + d * 1.5, n, (3.0, 2.4, 0.08))
            with tag(bm, BM):
                for y in (-11.2, -12.2, -13.2):
                    obox(bm, V((3.3, y, 2.5 * sz)) + d * 1.5, n, (2.9, 0.12, 0.22))
                for q in (0.03, 1.0):
                    obox(bm, base + d * (3.0 * q), n, (0.14, 2.5, 0.24))
                pipe(bm, [base + V((0, 1.3, 0)), base + V((0, -1.3, 0))], r=0.16, n=6)
            with tag(bm, WT):
                obox(bm, base + d * 1.5, n, (0.3, 2.42, 0.1))
    P(ship, f_pylons, bevel=0.025, segs=1, mirror=True)

    def f_block_c(bm):
        # neck: collars and longerons over the bare core
        for y in (9.5, 7.6):
            rib(bm, hull, y, y - 0.45, 0.2)
        with tag(bm, BM):
            for f in (0.5, 1.5, 2.5, 3.5, 4.5, 5.5, 6.5, 7.5, 8.5, 9.5):
                pipe(bm, [hull.at(10.0, f, 0.28), hull.at(6.8, f, 0.28)], r=0.13)
        # engine block: heavy ribs, a faction band
        for y in frange(-16.0, -24.4, 2.1):
            rib(bm, hull, y, y - 0.6, 0.2)
        rib(bm, hull, -25.7, -26.2, 0.12, col=BM)
        rib(bm, hull, -18.85, -20.45, 0.12, mat=L)
        rib(bm, hull, -20.95, -22.55, 0.12, mat=L)
        # waist: longerons between citadel and engine block
        with tag(bm, BM):
            for f in (0.5, 1.5, 3.5, 4.5, 5.5, 6.5, 8.5, 9.5):
                pipe(bm, [hull.at(-10.4, f, 0.3), hull.at(-14.4, f, 0.3)], r=0.14)
    P(ship, f_block_c, bevel=0.035, segs=2)

    def f_eng(bm):
        engine(bm, ENGS[0][0], ENGS[0][1], 3.4, N=12)
        engine(bm, ENGS[2][0], ENGS[2][1], 2.0, N=8)
    P(ship, f_eng, bevel=0.0, mirror=True, weighted=False, recalc=False)
    for i, (c, r) in enumerate(ENGS):
        ship.empty("Engine_%d" % i, c, r * 0.8)

    # ---------------------------------------------------------------- main battery
    guns = []

    def f_turrets(bm):
        guns.extend(turret(bm, (0, 20.4, hull.top(20.4) + 0.05), s=1.45))
        guns.extend(turret(bm, (0, 13.7, hull.top(13.7) + 1.35), s=1.45))
        guns.extend(turret(bm, (0, -11.0, 4.8), s=1.45, fwd=-1))
    P(ship, f_turrets, bevel=0.025, segs=1)
    guns += [V((BX, 27.5, -0.5)), V((-BX, 27.5, -0.5))]
    for i, g in enumerate(guns):
        ship.empty("Gun_%d" % i, g, 0.2)

    # ---------------------------------------------------------------- armour
    def f_armor(bm):
        # prow
        shell(bm, hull, 28.4, 26.0, [0.12, 1, 2, 3, 3.9], off=0.16)
        shell(bm, hull, 25.6, 23.4, [0.1, 1, 2, 3, 3.9], off=0.1, mat=L)
        shell(bm, hull, 25.85, 25.65, [0.1, 1, 2, 3, 3.9], off=0.1, col=WT)
        shell(bm, hull, 23.35, 23.15, [0.1, 1, 2, 3, 3.9], off=0.1, col=WT)
        for y0, y1 in ((22.8, 19.4), (19.0, 15.4), (15.0, 11.3)):
            shell(bm, hull, y0, y1, [1.06, 1.94], off=0.12)
            shell(bm, hull, y0, y1, [3.08, 3.92], off=0.1)
        # citadel: belt, shoulder and bilge strakes in ribbed lengths
        for i, y in enumerate(frange(5.3, -8.7, 2.8)[:-1]):
            shell(bm, hull, y, y - 2.45, [2.3, 2.96], off=0.2, mat=L if i in (1, 2) else H)
            if i != 3:
                shell(bm, hull, y, y - 2.45, [1.07, 1.93], off=0.12)
            if i != 1:
                shell(bm, hull, y, y - 2.45, [3.07, 3.93], off=0.12)
        shell(bm, hull, 5.4, -8.8, [1.95, 2, 2.07], off=0.13, mat=L)
        # waist and engine block
        shell(bm, hull, -11.0, -13.4, [1.1, 2, 2.95], off=0.12)
        shell(bm, hull, -15.2, -15.6, [0.1, 1, 2, 3, 3.9], off=0.16, col=WT)
        # cheek strake
        shell(bm, bow, 20.6, 16.2, [2.15, 2.85], off=0.12, mat=L)
        # stencil blocks: hull number on the bow, pennant block on the nacelles
        for y in (22.4, 21.5):
            shell(bm, hull, y, y - 0.65, [2.1, 2.4], off=0.04, col=WT)
        for y in (-8.6, -9.3):
            shell(bm, pod, y, y - 0.5, [2.2, 2.8], off=0.04, col=WT)
        # tier edges
        shell(bm, t1, 3.2, -7.0, [1.85, 2.25], off=0.09)
        shell(bm, t1, -0.6, -3.0, [2.3, 2.95], off=0.06, mat=L)
        shell(bm, t2, 2.0, -2.2, [2.7, 2.98], off=0.06)
        with tag(bm, HZ):
            shell(bm, hull, 11.0, 10.75, [1.1, 2, 2.9], off=0.14)
            shell(bm, hull, 5.95, 5.7, [1.1, 2, 2.9], off=0.16)
    P(ship, f_armor, bevel=0.025, segs=1, mirror=True, recalc=False)

    def f_armor_c(bm):
        shell(bm, hull, 28.2, 22.0, [9.75, 10, 10.25], off=0.18)           # prow spine
        shell(bm, hull, 22.0, 11.4, [4.5, 5, 5.5], off=0.18)               # keel
        shell(bm, hull, -11.0, -13.4, [4.55, 5, 5.45], off=0.18, mat=L)
        shell(bm, t2, 2.1, -2.3, [9.3, 10, 10.7], off=0.06)
    P(ship, f_armor_c, bevel=0.025, segs=1, recalc=False)

    # ---------------------------------------------------------------- neck tanks, casemates, heat sinks
    def f_side(bm):
        for y in (8.45,):
            for x, z, r in ((3.75, 0.0, 0.8), (2.9, 2.55, 0.58), (2.9, -2.55, 0.58)):
                sphere(bm, (x, y, z), r, seg=12, rings=8)
        # broadside casemates on the belt
        for y in frange(5.3, -8.7, 2.8)[:-1]:
            p, n = hull.surf(y - 1.22, 2.66, 0.2)
            obox(bm, p + n * 0.14, n, (0.7, 1.2, 0.4))
            with tag(bm, BM):
                cyl(bm, p + n * 0.25, p + n * 1.7, 0.12, 0.1, n=6)
        # missile hatches in the open shoulder bay
        for y in (-3.65, -4.4, -5.15):
            for f in (1.28, 1.72):
                p, n = hull.surf(y, f)
                with tag(bm, DK):
                    frustum(bm, p, n, 0.62, 0.62, 0.12)
        with tag(bm, HZ):
            for y in (-3.12, -5.68):
                p, n = hull.surf(y, 1.5)
                obox(bm, p + n * 0.03, n, (1.6, 0.14, 0.08))
        slats(bm, (2.1, -5.0, t1.top(-5.0)), (0, 0, 1), 1.4, 2.4, 4, h=0.16, t=0.2)
        # heat sinks on the engine block, on plinths that stand on the hull between the ribs
        for x in (1.7, 4.0):
            for sz in (1, -1):
                box(bm, (x, -22.6, 4.42 * sz), (1.9, 2.8, 0.4))
                slats(bm, (x, -22.6, 4.62 * sz), (0, 0, sz), 1.7, 2.6, 4, h=0.22, t=0.3)
        p, n = hull.surf(-17.35, 2.5)
        slats(bm, p, n, 2.6, 1.2, 3, h=0.2, t=0.26)
        # breakwater ahead of the fore turret
        obox(bm, (0.97, 22.85, hull.top(22.85) + 0.16), (0, 0, 1), (2.0, 0.16, 0.5), yv=(0.4, 1, 0))
        # chin sensor under the bow
        box(bm, (1.0, 24.6, hull.bot(24.6) - 0.1), (0.9, 2.4, 0.5), taper=(0.6, 0.5))
    P(ship, f_side, bevel=0.015, segs=1, mirror=True)

    # ---------------------------------------------------------------- lit windows
    def f_win(bm):
        windows(bm, hull, 22.6, 11.6, [2.1], rnd)
        windows(bm, hull, 26.6, 23.6, [1.5], rnd, lit=0.5)
        windows(bm, hull, 5.0, -8.6, [2.12, 2.22], rnd, grow=0.0)
        windows(bm, hull, 5.0, 3.1, [3.4], rnd, lit=0.6, grow=0.12)
        windows(bm, hull, -0.6, -8.4, [3.4], rnd, lit=0.45, grow=0.12)
        windows(bm, hull, -11.2, -13.2, [2.4, 2.6], rnd, lit=0.7, grow=0.12)
        windows(bm, hull, -19.05, -20.05, [2.2, 2.4], rnd, grow=0.12, lit=0.8)
        windows(bm, hull, -21.15, -22.15, [2.2, 2.4], rnd, grow=0.12, lit=0.8)
        windows(bm, t1, 3.2, -7.0, [2.5, 2.75], rnd, step=0.45)
        windows(bm, t2, 2.0, -2.2, [2.45], rnd, step=0.36, lit=0.9)
        windows(bm, pod, -9.0, -25.0, [1.9], rnd, lit=0.45)
        windows(bm, gon, 6.0, -6.2, [2.4], rnd, lit=0.5)
        windows(bm, bow, 20.8, 13.6, [1.5], rnd, lit=0.5)
        windows(bm, hull, 27.0, -25.0, [1.02], rnd, step=2.6, lit=1.0, w=0.1, h=0.1)    # deck-edge lamps
        windows(bm, hull, 27.0, -25.0, [3.98], rnd, step=2.6, lit=1.0, w=0.1, h=0.1)
        for x in frange(0.25, 2.3, 0.45):                            # front face of the tier
            if rnd.random() < 0.75:
                quad(bm, (x, 4.42, 4.4), (0, 1, 0), (1, 0, 0), 0.11, 0.26)
    P(ship, f_win, bevel=0.0, mirror=True, weighted=False, recalc=False)

    # ---------------------------------------------------------------- small stuff, mirrored: PD, lights
    def f_small(bm):
        for y in (25.0, 17.4, 3.6, -1.6, -6.8, -19.6, -21.8):
            pd(bm, *hull.surf(y, 1.5, 0.12))
        for y in (18.0, 3.9, -4.8, -20.0):
            pd(bm, *hull.surf(y, 3.5, 0.12))
        for y in (24.4, 13.4):                                       # under the bow
            pd(bm, *hull.surf(y, 4.22))
        for y in (-8.95, -16.1, -21.1):
            pd(bm, *pod.surf(y, 0.6), s=-0.9 if y < -15 else 0.9)
            pd(bm, *pod.surf(y, 4.4), s=-0.9 if y < -15 else 0.9)
        pd(bm, *t1.surf(1.0, 0.92), s=0.8)
        pd(bm, *t1.surf(-6.6, 0.5), s=-0.8)
        for p in ((hull.dims(27.4)[0], 27.4, -0.35), (PX + 1.95, -7.8, 0.0), (PX + 1.95, -26.0, 0.0), (5.75, -12.2, 4.35)):
            sphere(bm, p, 0.1, mat=G, seg=6, rings=4)
    D(ship, f_small, mirror=True)

    def f_greeb(bm):
        greeble(bm, hull, 5.0, -8.6, 1.05, 1.95, 90, rnd, smax=0.8, hmax=0.2)
        greeble(bm, hull, 5.0, -8.6, 3.05, 3.95, 70, rnd, smax=0.8, hmax=0.2)
        greeble(bm, hull, 5.0, -8.6, 0.84, 0.99, 50, rnd, smax=0.6, hmax=0.2)
        greeble(bm, hull, 26.0, 11.6, 0.35, 0.95, 60, rnd, smax=0.7, hmax=0.2)
        greeble(bm, hull, 26.0, 11.6, 3.5, 3.95, 50, rnd, smax=0.7, hmax=0.2)
        greeble(bm, hull, 26.0, 11.6, 4.05, 4.42, 70, rnd, smax=0.7, hmax=0.2)
        greeble(bm, hull, 9.8, 7.0, 0.2, 4.8, 60, rnd, smax=0.8, hmax=0.4)
        greeble(bm, hull, -11.0, -13.4, 0.1, 0.95, 40, rnd, smax=0.8, hmax=0.3)
        greeble(bm, hull, -11.0, -13.4, 3.1, 4.4, 40, rnd, smax=0.8, hmax=0.3)
        for y0, y1 in ((-16.75, -17.95), (-23.05, -24.25)):          # between the block's ribs, on the hull itself
            greeble(bm, hull, y0, y1, 0.2, 0.95, 40, rnd, smax=0.5, hmax=0.25)
            greeble(bm, hull, y0, y1, 1.05, 1.95, 40, rnd, smax=0.5, hmax=0.2)
            greeble(bm, hull, y0, y1, 3.05, 4.9, 50, rnd, smax=0.5, hmax=0.2)
        greeble(bm, t1, 3.2, -7.2, 0.1, 0.95, 90, rnd, smax=0.6, hmax=0.2)
        greeble(bm, t2, 2.0, -2.2, 0.1, 0.9, 30, rnd, smax=0.5, hmax=0.14)
        greeble(bm, gon, 5.8, -6.4, 4.05, 4.95, 50, rnd, smax=0.7, hmax=0.2)
        greeble(bm, pod, -8.6, -25.4, 0.1, 0.95, 60, rnd, smax=0.6, hmax=0.2)
        greeble(bm, pod, -8.6, -25.4, 4.1, 4.9, 50, rnd, smax=0.6, hmax=0.2)
        greeble(bm, pod, -8.6, -25.4, 7.05, 7.95, 50, rnd, smax=0.6, hmax=0.2)
        greeble(bm, bow, 20.6, 13.6, 0.1, 0.9, 30, rnd, smax=0.5, hmax=0.16)
    P(ship, f_greeb, bevel=0.0, mirror=True, weighted=False, recalc=False)

    # ---------------------------------------------------------------- centre line: masts, sensors, stem, stern
    def f_center(bm):
        zt = t2.top(0.0)
        with tag(bm, BM):
            for x, y, h, r in ((0.0, -1.9, 2.6, 0.1), (1.5, -1.5, 1.0, 0.04), (-1.5, -1.5, 1.0, 0.04),
                               (2.0, 1.2, 0.7, 0.035), (-2.0, 1.2, 0.7, 0.035)):
                cyl(bm, (x, y, zt), (x, y, zt + h), r, r * 0.4, n=5)
                cyl(bm, (x, y, zt), (x, y, zt + 0.16), r * 3, r * 2, n=6)
            box(bm, (0, -1.9, zt + 1.3), (1.8, 0.06, 0.06))            # tripod mast: yards, struts, radar
            box(bm, (0, -1.9, zt + 1.9), (1.1, 0.06, 0.06))
            for sx in (0.7, -0.7):
                cyl(bm, (sx, -2.2, zt), (0, -1.9, zt + 1.7), 0.045, n=4)
            box(bm, (0, -1.75, zt + 2.3), (0.7, 0.12, 0.3))
            cyl(bm, (0, 0.2, zt), (0, 0.2, zt + 0.4), 0.16, n=8)     # radar bar
            box(bm, (0, 0.2, zt + 0.62), (2.6, 0.2, 0.44))
            lathe(bm, (1.3, -6.2, 5.3), (0, -0.4, 1), [(0.07, 0.0), (0.85, 0.28), (0.8, 0.34), (0.0, 0.1)], n=14)
            cyl(bm, (1.3, -6.2, 5.2), (1.3, -6.2, 5.34), 0.1, n=6)
            lathe(bm, (-1.3, -6.2, 5.3), (0, -0.4, 1), [(0.07, 0.0), (0.85, 0.28), (0.8, 0.34), (0.0, 0.1)], n=14)
            cyl(bm, (-1.3, -6.2, 5.2), (-1.3, -6.2, 5.34), 0.1, n=6)
        sphere(bm, (0, -1.9, zt + 2.63), 0.09, mat=G, seg=6, rings=4)
        box(bm, (0, 28.95, -0.55), (0.3, 0.9, 2.6))                  # armoured stem
        box(bm, (0, 28.75, 0.75), (2.2, 0.6, 0.2))
        with tag(bm, BM):
            for x, z in ((0.0, -1.5), (0.9, -1.0), (-0.9, -1.0)):
                cyl(bm, (x, 28.7, z), (x, 30.0, z), 0.08, 0.03, n=6)
        for x in (-0.55, 0.55):                                      # torpedo tubes in the bow face
            for z in (-0.9, -0.3):
                with tag(bm, BM):
                    cyl(bm, (x, 28.7, z), (x, 28.92, z), 0.22, n=8)
                cyl(bm, (x, 28.9, z), (x, 28.94, z), 0.07, n=6, mat=G)
        # stern face between the nozzles
        with tag(bm, BM):
            box(bm, (0, -26.6, 0.0), (0.5, 1.2, 7.4))
            cyl(bm, (0, -26.6, 3.2), (0, -27.6, 3.2), 0.55, 0.45, n=10)
            cyl(bm, (0, -26.6, -3.2), (0, -27.6, -3.2), 0.55, 0.45, n=10)
        for z in (3.2, -3.2):
            cyl(bm, (0, -27.6, z), (0, -27.64, z), 0.28, n=8, mat=G)
    P(ship, f_center, bevel=0.0, weighted=False)

    tot = sum(cap.AREA) or 1.0
    log("west_cruiser parts", cap.TRIS)
    log("west_cruiser area %.0f" % tot, "hull %.0f%% livery %.1f%% glow %.1f%% glass %.1f%% core %.1f%%" % tuple(100 * a / tot for a in cap.AREA))

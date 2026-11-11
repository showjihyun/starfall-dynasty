"""Style candidate `japan_small`: the Veyr captain's first ship, a 24 m one-seat fighter (60 units, 1 unit = 0.4 m).

A hero machine in the fleet's colours: a blade nose with a hard chine, a framed canopy ahead of a dorsal spine
that carries the yellow V-fin, canards over raked box intakes, two under-slung engine legs with exposed nozzles,
cranked wings with tip pods, canted twin tails and two long guns in cheek fairings beside the nose. White armour,
blue blocks, a red belly; the dark frame shows on the fuselage flanks, the leg knees and the wing pivots.
Helpers and the `tone` material are the flagship's (styles/japan.py); the panel grid is three times coarser,
a fighter is skinned in a few big plates.
"""
import math

from shiplib import V, H, L, G, GL, E, loft, ring10, seg_sym, lathe, box, prism, cyl, sphere, wing

import build_ships as bs
from build_ships import interp

from styles import japan as cap
from styles.japan import FRAME, YEL, RED, GREY, P, tone, inset, paint, pick, wst, slab, vernier, engine

STYLE = dict(cap.STYLE, spacing=(2.4, 3.2, 2.1), seam_w=0.022, grime=0.3)
TINT = cap.TINT


def materials():
    """The flagship's Hull graph with the tone repaint, on this ship's coarser panel grid."""
    old = cap.STYLE
    cap.STYLE = STYLE
    try:
        return cap.materials()
    finally:
        cap.STYLE = old


FS = [  # fuselage: y, half beam, half depth, centre z
    (28.2, 0.55, 0.36, -0.34),
    (25.5, 1.08, 0.7, -0.32),
    (23.0, 1.45, 0.98, -0.3),
    (21.0, 1.7, 1.18, -0.28),
    (19.0, 1.92, 1.36, -0.26),
    (17.0, 2.1, 1.5, -0.24),
    (13.0, 2.5, 1.8, -0.18),
    (8.0, 2.9, 2.0, -0.1),
    (2.0, 3.3, 2.1, 0.0),
    (-6.0, 3.4, 2.1, 0.0),
    (-14.0, 3.0, 1.9, 0.1),
    (-20.0, 2.4, 1.6, 0.2),
    (-24.0, 1.6, 1.2, 0.3),
]
YS = [28.2, 25.5, 23, 21, 19, 17, 15, 13, 11, 9, 6, 3, 0, -3, -6, -9, -12, -15, -18, -21, -24]
NX, NZ = 4.6, -1.0   # engine leg centreline
NS = [(5.0, 1.65, 1.5, NZ), (2.0, 2.0, 1.85, NZ - 0.05), (-6.0, 2.1, 2.1, NZ - 0.2), (-14.0, 2.1, 2.05, NZ - 0.15), (-20.0, 2.1, 1.9, NZ),
      (-25.6, 1.95, 1.78, NZ)]
NYS = [5.0, 2.0, -2.0, -6.0, -10.0, -14.0, -17.0, -20.0, -23.0, -25.6]
NK = dict(top=0.55, bot=0.6, hi=0.4, lo=-0.4)
ER, EL = 1.7, 4.6    # nozzle radius and length
W0 = dict(x=2.4, z=0.75, le=6.0, te=-17.6, t=1.0)      # wing root (inside the fuselage)
W1 = dict(x=6.6, z=0.65, le=-3.0, te=-18.4, t=0.8)     # crank
W2 = dict(x=20.4, z=0.25, le=-16.6, te=-21.6, t=0.26)  # tip


def fs(y, i):
    return interp(FS, y, i)


def ftop(y):
    return fs(y, 3) + fs(y, 2)


def fbot(y):
    return fs(y, 3) - fs(y, 2)


def fk(y):
    """The nose is a blade (a sharp chine at the waterline); aft of the windscreen the flanks stand up."""
    t = min(1.0, max(0.0, (22.0 - y) / 7.0))
    return dict(top=0.4 + 0.22 * t, bot=0.42 + 0.08 * t, hi=0.08 + 0.37 * t, lo=-0.08 - 0.22 * t, keel=0.15)


DT, DTH = 9.6, 1.5   # leading-edge dog-tooth: span station and step


def wing_at(x, tooth=True):
    """Outer wing at span x: z, le, te, t; outboard of the dog-tooth the leading edge steps forward."""
    f = (x - W1['x']) / (W2['x'] - W1['x'])
    c = {k: W1[k] + (W2[k] - W1[k]) * f for k in W1}
    if tooth and x > DT:
        c['le'] += DTH * (W2['x'] - x) / (W2['x'] - DT)
    return c


def build(ship):
    cap.AREA.clear()

    # ---------------------------------------------------------------- fuselage
    def fuselage(bm):
        rings = [V((0, 30.0, -0.36))] + [ring10(y, fs(y, 1), fs(y, 2), fs(y, 3), **fk(y)) for y in YS]

        def mf(i, k):
            s = seg_sym(k)
            if s == 1 and 5 <= i <= 11:
                return L
            if s in (0, 1) and i == 2:
                return L
            return H
        res = loft(bm, rings, matfn=mf)
        paint(bm, pick(res, lambda i, k: seg_sym(k) in (3, 4)), RED)
        paint(bm, pick(res, lambda i, k: i == 0 and seg_sym(k) <= 2), GREY)            # radome
        paint(bm, pick(res, lambda i, k: seg_sym(k) == 0 and i in (3, 4)), GREY)       # anti-glare panel
        paint(bm, pick(res, lambda i, k: seg_sym(k) == 2 and 8 <= i <= 12), FRAME)     # frame under the flank plates
        inset(bm, pick(res, lambda i, k: seg_sym(k) == 1 and i in (3, 4)), 0.14, -0.05)
        inset(bm, pick(res, lambda i, k: seg_sym(k) == 1 and i in (13, 15, 17, 19)), 0.16, -0.06)
        inset(bm, pick(res, lambda i, k: seg_sym(k) == 1 and i == 12), 0.2, -0.3, t=FRAME)
        inset(bm, pick(res, lambda i, k: seg_sym(k) == 3 and i in (4, 6, 8, 14, 16, 18)), 0.16, -0.06, t=RED)
        inset(bm, pick(res, lambda i, k: seg_sym(k) == 4 and i in (6, 7)), 0.14, -0.05, t=RED)    # nose gear doors
        inset(bm, pick(res, lambda i, k: seg_sym(k) == 4 and i in (11, 15)), 0.3, -0.16, t=FRAME)
        inset(bm, [res['end']], 0.22, -0.5, t=FRAME)
    P(ship, fuselage, bevel=0.07)

    def flank(bm):
        for j, (y0, y1) in enumerate(((12.7, 9.5), (9.1, 5.9))):
            res = bs.side_plate(bm, FS, y0, y1, -0.6, 0.62, 0.24, n=2, bevel_in=0.16)
            if j == 1:
                for f in pick(res, lambda i, k: k == 1):
                    f.material_index = L
    P(ship, flank, bevel=0.035, segs=1, mirror=True)

    def flank_bits(bm):
        with tone(bm, FRAME):
            for o in (0.2, 0.75):   # conduits behind the plates, seen in the gaps
                cyl(bm, (fs(13.0, 1) - 0.02, 13.0, -0.4 + o), (fs(5.4, 1) - 0.02, 5.4, -0.4 + o), 0.1, n=6)
            for y in (26.0, 25.0):   # nose verniers on the upper chine
                vernier(bm, (fs(y, 1) * 0.62, y, fs(y, 3) + fs(y, 2) * 0.62), (0.45, 0, 1), 0.27, 0.3)
            for y in (22.4,):
                vernier(bm, (fs(y, 1) * 0.3, y, fbot(y) - 0.06), (0, 0, -1), 0.24, 0.3)
            for y in (-21.5, -22.6):
                vernier(bm, (fs(y, 1) * 0.5, y, ftop(y) - 0.04), (0.3, 0, 1), 0.22, 0.28)
    P(ship, flank_bits, bevel=0.0, mirror=True)

    # ---------------------------------------------------------------- canopy
    def zb(y):
        return ftop(y) - 0.06
    CS = [(20.7, 0.0, 0.0), (19.5, 0.6, 0.62), (17.6, 1.02, 1.22), (15.0, 1.25, 1.6), (12.6, 1.2, 1.52), (10.2, 0.95, 1.12)]

    def glass(bm):
        bs.canopy(bm, [(y, w, h, zb(y)) for y, w, h in CS], n=14)
    P(ship, glass, bevel=0.0, angle=50)

    def canopy_frame(bm):
        with tone(bm, GREY):
            for y in (17.6, 13.4):
                w, h = interp(CS, y, 1), interp(CS, y, 2)
                bs.canopy_frame(bm, y, w, h, zb(y), n=14, t=0.11)
            for sx in (1, -1):   # sill rails
                for (ya, wa, _), (yb, wb, _) in zip(CS[1:], CS[2:]):
                    prism(bm, (sx * (wa + 0.04), ya, zb(ya) + 0.04), (sx * (wb + 0.04), yb, zb(yb) + 0.04), 0.1, 0.1)
            prism(bm, (0, 17.5, zb(17.5) + 1.22), (0, 20.5, zb(20.5) + 0.06), 0.07, 0.06)    # windscreen centre post
    P(ship, canopy_frame, bevel=0.012, segs=1)

    # ---------------------------------------------------------------- dorsal spine and the V-fin
    SP = [(10.5, 1.12, 1.3), (7.5, 1.2, 1.3), (3.0, 1.15, 1.1), (-4.0, 1.05, 0.8), (-12.0, 0.85, 0.5), (-19.0, 0.55, 0.25)]
    SYS = [10.5, 9.0, 7.5, 3.0, 0.5, -4.0, -8.0, -12.0, -15.5, -19.0]

    def sp_top(y):
        return ftop(y) + interp(SP, y, 2)

    def spine(bm):
        def ring(y):
            hw, h, z0 = interp(SP, y, 1), interp(SP, y, 2), ftop(y)
            return [V((hw * 0.5, y, z0 + h)), V((hw, y, z0 + h * 0.5)), V((hw, y, z0 - 0.5)),
                    V((-hw, y, z0 - 0.5)), V((-hw, y, z0 + h * 0.5)), V((-hw * 0.5, y, z0 + h))]
        res = loft(bm, [ring(y) for y in SYS], matfn=lambda i, k: L if (k in (0, 4, 5) and 3 <= i <= 6) else H)
        paint(bm, [res['start']], GREY)
        inset(bm, pick(res, lambda i, k: k in (1, 3) and i in (0, 1)), 0.14, -0.05)
        inset(bm, pick(res, lambda i, k: k in (1, 3) and i == 2), 0.2, -0.3, t=FRAME)
        inset(bm, pick(res, lambda i, k: k == 5 and i in (4, 6)), 0.14, -0.05, mat=L)
        inset(bm, pick(res, lambda i, k: k in (1, 3) and i in (4, 5, 6)), 0.12, -0.04)
    P(ship, spine, bevel=0.05)

    ZV = sp_top(8.3)

    def vfin(bm):
        with tone(bm, YEL):
            for sx in (1, -1):
                wing(bm, [dict(x=sx * 0.12, z=ZV - 0.2, le=9.6, te=7.2, t=0.26),
                          dict(x=sx * 1.5, z=ZV + 0.9, le=8.2, te=6.6, t=0.2),
                          dict(x=sx * 3.4, z=ZV + 2.3, le=5.4, te=4.9, t=0.07)], flat_te=0.3)
        with tone(bm, RED):
            box(bm, (0, 8.9, ZV + 0.1), (0.55, 1.3, 0.34))
        with tone(bm, FRAME):
            bs.vent(bm, (0, 1.6, sp_top(1.6) + 0.0), 0.9, 1.8, n=6, h=0.1)
            bs.antenna(bm, (0, -9.5, sp_top(-9.5) - 0.05), 2.2, r=0.05)
            bs.antenna(bm, (0, 5.2, sp_top(5.2) - 0.05), 1.0, r=0.04, tip=False)
            for sx in (1, -1):
                cyl(bm, (sx * 0.62, 3.0, sp_top(3.0) - 0.42), (sx * 0.5, -11.0, sp_top(-11.0) - 0.2), 0.09, n=6)
    P(ship, vfin, bevel=0.012, segs=1)

    # ---------------------------------------------------------------- engine legs
    def leg(bm):
        rings = [ring10(y, interp(NS, y, 1), interp(NS, y, 2), interp(NS, y, 3), NX, **NK) for y in NYS]
        rings[0] = [V((p.x, p.y + (p.z - NZ) * 0.55, p.z)) for p in rings[0]]     # raked intake
        res = loft(bm, rings, matfn=lambda i, k: L if (i <= 1 and seg_sym(k) <= 2) else H)
        paint(bm, pick(res, lambda i, k: seg_sym(k) in (3, 4)), RED)
        paint(bm, pick(res, lambda i, k: k == 2 and i in (3, 4, 5)), FRAME)
        paint(bm, pick(res, lambda i, k: i == 8), GREY)
        inset(bm, [res['start']], 0.16, -2.4, t=GREY)                    # intake
        inset(bm, pick(res, lambda i, k: i == 1 and k == 2), 0.2, -0.06, mat=L)
        inset(bm, pick(res, lambda i, k: i in (6, 7) and k in (1, 2)), 0.16, -0.06)
        inset(bm, pick(res, lambda i, k: i == 8 and k in (1, 2, 3)), 0.14, -0.07, t=FRAME)
        inset(bm, pick(res, lambda i, k: i in (2, 3, 4) and k in (4, 5)), 0.16, -0.06, t=RED)    # gear doors
        inset(bm, pick(res, lambda i, k: i in (1, 6) and k == 3), 0.2, -0.14, t=FRAME)
        inset(bm, [res['end']], 0.18, -0.3, t=FRAME)
        # flank plates over the dark knee
        for y0, y1 in ((-6.3, -9.8), (-10.2, -13.8), (-14.2, -16.8)):
            x = [NX + interp(NS, y, 1) + 0.02 for y in (y0, y1)]
            loft(bm, [[V((xx, y, NZ - 0.66)), V((xx + 0.24, y, NZ - 0.5)), V((xx + 0.24, y, NZ + 0.5)), V((xx, y, NZ + 0.66))]
                      for xx, y in zip(x, (y0, y1))])
    P(ship, leg, bevel=0.06, mirror=True)

    def leg_bits(bm):
        with tone(bm, FRAME):
            for o in (-0.3, 0.3):
                cyl(bm, (NX + 2.08, -6.0, NZ + o), (NX + 2.08, -17.0, NZ + o), 0.1, n=6)
            for y in (-21.0, -22.4):
                vernier(bm, (NX + interp(NS, y, 1) - 0.03, y, NZ + 0.3), (1, 0, 0), 0.34, 0.4)
            vernier(bm, (NX + 0.3, 2.6, NZ - 1.74), (0, 0, -1), 0.26, 0.3)
            bs.vent(bm, (NX, 3.3, NZ + interp(NS, 3.3, 2) + 0.05), 1.3, 1.7, n=6, h=0.1)      # auxiliary louvres
            bs.vent(bm, (1.62, -12.5, ftop(-12.5) + 0.03), 0.7, 2.4, n=7, h=0.09)
        with tone(bm, YEL):
            box(bm, (NX, 5.76, NZ + 1.52), (1.85, 0.3, 0.08))
        bs.light(bm, (NX + 2.14, -18.8, NZ + 0.9), 0.1)
        # dorsal booster on each leg, inboard of the tail: a vernier pack with its own nozzle
        bx, bz = NX - 0.75, NZ + 2.1
        lathe(bm, (bx, -25.0, bz), (0, 1, 0), [(0.5, 0.0), (0.66, 0.5), (0.7, 1.4), (0.7, 5.6), (0.5, 7.2), (0.0, 8.3)], n=12,
              mats=[H, H, L, H, H])
        with tone(bm, FRAME):
            vernier(bm, (bx, -25.0, bz), (0, -1, 0), 0.5, 0.6, hot=True)
            lathe(bm, (bx, -23.75, bz), (0, 1, 0), [(0.73, 0.0), (0.73, 0.3)], n=12, cap_start=False, cap_end=False)
            vernier(bm, (bx, -21.0, bz + 0.66), (0, 0, 1), 0.22, 0.26)
        with tone(bm, YEL):   # rescue marker on the flank plate under the canopy sill
            box(bm, (fs(11.6, 1) + 0.27, 11.6, 0.2), (0.06, 0.7, 0.36))
        with tone(bm, GREY):  # canopy hinge fairings, boarding steps
            box(bm, (1.12, 10.9, ftop(10.9) + 0.1), (0.3, 1.1, 0.3))
            for y in (14.6, 15.6):
                box(bm, (fs(y, 1) + 0.0, y, -0.25), (0.12, 0.45, 0.16))
    P(ship, leg_bits, bevel=0.0, mirror=True)

    ENG = [(NX, -30.0, NZ), (-NX, -30.0, NZ)]

    def engines(bm):
        for c in ENG:
            engine(bm, c, ER, EL, petals=14, n=24)
        # tail sting between the nozzles
        with tone(bm, GREY):
            lathe(bm, (0, -23.7, 0.3), (0, -1, 0), [(1.0, 0.0), (0.9, 0.9), (0.55, 2.4), (0.2, 3.6), (0.0, 3.9)], n=12)
        bs.light(bm, (0, -27.6, 0.3), 0.14)
    P(ship, engines, bevel=0.0)
    for i, c in enumerate(ENG):
        ship.empty("Engine_%d" % i, c, ER)

    # ---------------------------------------------------------------- cranked wings, tip pods
    def wings(bm):
        st = wst(W0, W1, 2) + [wing_at(DT, False)] + [wing_at(x) for x in (DT + 0.02, 12.3, 15.0, 17.7, W2['x'])]
        res = wing(bm, st, matfn=lambda i, k: L if (i in (5, 6) and k in (0, 1, 2)) else H)
        paint(bm, pick(res, lambda i, k: k in (4, 5, 6)), RED)
        paint(bm, pick(res, lambda i, k: i == 7 and k in (0, 1, 2)), GREY)
        inset(bm, pick(res, lambda i, k: i in (2, 4) and k == 2), 0.16, -0.05)              # flaps
        inset(bm, pick(res, lambda i, k: i in (5, 6) and k == 2), 0.16, -0.05, mat=L)       # aileron
        inset(bm, pick(res, lambda i, k: i in (2, 4) and k == 1), 0.2, -0.04)
        inset(bm, pick(res, lambda i, k: i in (4, 5, 6) and k == 0), 0.12, -0.04)           # slats
        inset(bm, pick(res, lambda i, k: i in (2, 4, 6) and k == 5), 0.2, -0.05, t=RED)
        inset(bm, pick(res, lambda i, k: i in (0, 1) and k == 1), 0.3, -0.12, t=FRAME)      # glove well
    P(ship, wings, bevel=0.035, mirror=True)

    def wing_bits(bm):
        # armour plates floating in the glove well, the pivot between them
        def floor(x, y):
            f = (x - W0['x']) / (W1['x'] - W0['x'])
            return W0['z'] + (W1['z'] - W0['z']) * f + (W0['t'] + (W1['t'] - W0['t']) * f) * 0.44 - 0.13
        slab(bm, [(3.75, -2.7), (5.95, -6.1), (5.95, -6.5), (3.75, -6.5)], floor, 0.2, chamf=0.06)
        slab(bm, [(3.75, -9.6), (6.2, -9.6), (6.2, -13.0), (3.75, -11.1)], floor, 0.2, chamf=0.06, mat=L)
        zt = W1['z'] + 0.3
        with tone(bm, FRAME):
            cyl(bm, (5.4, -8.0, zt - 0.1), (5.4, -8.0, zt + 0.1), 1.25, n=20)
        cyl(bm, (5.4, -8.0, zt), (5.4, -8.0, zt + 0.2), 0.85, n=20)
        with tone(bm, GREY):
            cyl(bm, (5.4, -8.0, zt + 0.1), (5.4, -8.0, zt + 0.27), 0.3, n=10)
        # hinge line of the flaps and ailerons: the frame shows in the gap
        def hinge(x, lift=0.0):
            c = wing_at(x)
            return V((x, c['le'] - (c['le'] - c['te']) * 0.72, c['z'] + c['t'] * 0.42 + lift))
        with tone(bm, FRAME):
            prism(bm, hinge(DT + 0.3), hinge(20.0), 0.09, 0.035)
        with tone(bm, GREY):
            for x in (11.0, 13.6, 16.3, 19.0):
                box(bm, hinge(x, 0.03) + V((0, -0.25, 0)), (0.5, 0.9, 0.1))
        # tip pod
        c = wing_at(20.4)
        zc = c['z']
        lathe(bm, (20.75, -23.2, zc), (0, 1, 0), [(0.22, 0.0), (0.46, 0.5), (0.5, 1.4), (0.5, 6.4), (0.36, 7.8), (0.0, 9.0)], n=12,
              mats=[H, H, L, H, H])
        with tone(bm, FRAME):
            vernier(bm, (20.75, -23.2, zc), (0, -1, 0), 0.3, 0.36, hot=True)
            vernier(bm, (20.75, -19.0, zc + 0.46), (0, 0, 1), 0.2, 0.26)
            vernier(bm, (20.75, -19.0, zc - 0.46), (0, 0, -1), 0.2, 0.26)
        bs.light(bm, (20.75, -14.15, zc), 0.12)
        with tone(bm, YEL):
            a, b = wing_at(17.3), wing_at(17.75)
            wing(bm, [dict(x=a['x'], z=a['z'], le=a['le'] + 0.04, te=a['te'] - 0.04, t=a['t'] + 0.05),
                      dict(x=b['x'], z=b['z'], le=b['le'] + 0.04, te=b['te'] - 0.04, t=b['t'] + 0.05)], flat_te=0.5)
        # flap actuator fairings and pylons with missiles
        with tone(bm, GREY):
            for x in (8.6, 12.4):
                c = wing_at(x)
                prism(bm, (x, c['te'] + 4.6, c['z'] - c['t'] * 0.42), (x, c['te'] + 0.4, c['z'] - c['t'] * 0.3), 0.16, 0.2)
        for x in (10.4, 14.6):
            c = wing_at(x)
            zw = c['z'] - c['t'] * 0.36
            ym = (c['le'] + c['te']) / 2 + 0.6
            zm = zw - 0.85
            with tone(bm, GREY):
                box(bm, (x, ym, (zw + zm) / 2 + 0.1), (0.2, 3.0, zw - zm + 0.3))
            lathe(bm, (x, ym - 3.2, zm), (0, 1, 0), [(0.2, 0.0), (0.34, 0.2), (0.34, 4.9), (0.22, 5.8), (0.0, 6.5)], n=10)
            with tone(bm, YEL):
                lathe(bm, (x, ym + 1.0, zm), (0, 1, 0), [(0.355, 0.0), (0.355, 0.3)], n=10, cap_start=False, cap_end=False)
            with tone(bm, GREY):
                lathe(bm, (x, ym + 2.61, zm), (0, 1, 0), [(0.225, 0.0), (0.0, 0.72)], n=10, cap_start=False)
                for a in range(4):
                    ang = a * math.pi / 2 + math.pi / 4
                    d = V((math.cos(ang), 0, math.sin(ang)))
                    prism(bm, V((x, ym - 2.9, zm)) + d * 0.3, V((x, ym - 2.9, zm)) + d * 0.72, 0.03, 0.4, up=(0, 1, 0))
    P(ship, wing_bits, bevel=0.0, mirror=True)

    # ---------------------------------------------------------------- canards, tails
    def fins(bm):
        res = wing(bm, wst(dict(x=2.5, z=0.9, le=11.6, te=6.6, t=0.44), dict(x=7.4, z=1.8, le=7.2, te=5.2, t=0.14), 2), mat=L)
        paint(bm, pick(res, lambda i, k: k in (4, 5, 6)), RED)
        inset(bm, pick(res, lambda i, k: i == 0 and k == 1), 0.14, -0.04, mat=L)
        with tone(bm, FRAME):
            cyl(bm, (2.6, 8.9, 0.95), (3.3, 8.9, 1.08), 0.42, n=12)          # canard pivot
        # canted tails
        a = dict(x=NX + 0.7, z=0.6, le=-14.6, te=-25.6, t=0.62)
        b = dict(x=NX + 3.3, z=7.2, le=-24.4, te=-28.2, t=0.2)
        res = wing(bm, wst(a, b, 4), matfn=lambda i, k: L if i in (1, 2) else H)
        paint(bm, res['bands'][3], GREY)
        inset(bm, pick(res, lambda i, k: i == 0 and k in (1, 5)), 0.16, -0.05)
        inset(bm, pick(res, lambda i, k: i in (1, 2) and k in (2, 4)), 0.14, -0.04, mat=L)   # rudder
        bs.light(bm, (NX + 3.33, -28.2, 7.23), 0.12)
        with tone(bm, YEL):   # tail band, a quarter of the way up
            wing(bm, [dict(x=a['x'] + 2.6 * f, z=a['z'] + 6.6 * f, le=a['le'] - 9.8 * f + 0.04, te=a['te'] - 2.6 * f - 0.04,
                           t=a['t'] - 0.42 * f + 0.05) for f in (0.2, 0.235)], flat_te=0.5)
        with tone(bm, FRAME):
            bs.antenna(bm, (NX + 3.25, -26.0, 7.1), 0.9, r=0.035, tip=False, lean=(0.3, -0.3, 1))
        # ventral strakes
        a = dict(x=NX + 0.9, z=NZ - 1.5, le=-18.5, te=-25.4, t=0.42)
        b = dict(x=NX + 1.9, z=NZ - 3.7, le=-22.8, te=-26.6, t=0.16)
        wing(bm, wst(a, b, 2), mat=L)
    P(ship, fins, bevel=0.03, mirror=True)

    # ---------------------------------------------------------------- guns in cheek fairings
    GX, GZ, GY = 1.9, -1.1, 24.0

    def guns(bm):
        res = prism(bm, (GX, 10.5, GZ), (GX, 18.6, GZ), 0.66, 0.52, w1=0.5, h1=0.4)
        paint(bm, pick(res, lambda i, k: k in (3, 4)), RED)
        with tone(bm, FRAME):
            prism(bm, (GX, 18.6, GZ), (GX, 19.2, GZ), 0.34, 0.3)
        with tone(bm, GREY):
            cyl(bm, (GX, 19.2, GZ), (GX, GY - 0.7, GZ), 0.17, n=12)
            for y in (19.8, 20.9, 22.0):
                cyl(bm, (GX, y, GZ), (GX, y + 0.5, GZ), 0.25, n=12)
        with tone(bm, FRAME):
            cyl(bm, (GX, GY - 0.8, GZ), (GX, GY, GZ), 0.27, 0.24, n=12)
        cyl(bm, (GX, GY - 0.02, GZ), (GX, GY + 0.01, GZ), 0.15, n=12, mat=E)
        with tone(bm, GREY):
            box(bm, (GX + 0.2, 13.0, GZ - 0.42), (0.6, 2.4, 0.3))      # feed housing
    P(ship, guns, bevel=0.025, segs=1, mirror=True)
    ship.empty("Gun_0", (GX, GY, GZ), 0.25)
    ship.empty("Gun_1", (-GX, GY, GZ), 0.25)

    # ---------------------------------------------------------------- small things
    def bits(bm):
        with tone(bm, GREY):
            sphere(bm, (0, 22.6, fbot(22.6) - 0.02), 0.42, seg=12, rings=6, scale=(1, 1.9, 0.6))     # chin sensor
            box(bm, (0, -16.5, sp_top(-16.5) + 0.02), (0.9, 1.6, 0.14))
        for sx in (1, -1):
            bs.light(bm, (sx * fs(21.6, 1), 21.6, fs(21.6, 3)), 0.09)
    P(ship, bits, bevel=0.0)

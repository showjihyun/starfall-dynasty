"""Style candidate `japan_cruiser`: the flagship's little sister, about 150 m (60 units, 1 unit ~ 2.5 m).

An aviation cruiser in the same white armour over a red lower hull: the bow cannon muzzle under a longer blade
prow, both triple turrets superfiring forward, a three-storey pagoda bridge with rangefinder arms and the V-fin,
a raked funnel, a hangar that opens aft onto one centreline catapult deck, and an engine group that is a third
of the ship: one large nozzle in the stern and two intake-fed nacelles on stub pylons carrying the tail fins.
Everything reuses the flagship's helpers and its `tone` material (styles/japan.py); a window here is 2.8 times
bigger against the hull, so each deck gets one row.
"""
import math

from mathutils import Matrix

from shiplib import V, H, L, G, loft, ring10, seg_sym, lathe, box, prism, cyl, sphere, wing

import build_ships as bs
from build_ships import interp

from styles import japan as cap
from styles.japan import FRAME, YEL, RED, GREY, P, tone, inset, paint, pick, wst, slab, house, vernier, engine, turret, aa

STYLE = cap.STYLE
TINT = cap.TINT


def materials():
    return cap.materials()


HS0 = [  # main hull: y, half beam, half depth, centre z (deck level is 2.0 everywhere)
    (28.6, 1.6, 1.55, 0.45),
    (27.0, 1.85, 1.75, 0.25),
    (24.0, 2.3, 2.0, 0.0),
    (19.0, 3.0, 2.3, -0.3),
    (12.0, 3.8, 2.6, -0.6),
    (4.0, 4.3, 2.75, -0.75),
    (-4.0, 4.4, 2.8, -0.8),
    (-12.0, 4.2, 2.75, -0.75),
    (-18.0, 3.8, 2.6, -0.6),
    (-22.0, 3.4, 2.45, -0.45),
]
YS = [28.6, 27, 25, 23, 21, 19, 16, 13, 10, 7, 4, 0, -4, -8, -12, -15, -18, -20, -22]
HK = dict(top=0.8, bot=0.35, hi=0.7, lo=-0.1, keel=0.25)
DECK = 2.0
BELTS = [(18.6, 13.2), (12.8, 7.2), (6.8, 1.2), (0.8, -4.8), (-5.2, -10.8), (-11.2, -15.2)]
PX = 6.5   # nacelle centreline
PZ = -0.3
PS = [(-12.6, 1.15, 1.05, PZ), (-14.5, 1.5, 1.35, PZ), (-17.0, 1.7, 1.55, PZ), (-23.0, 1.7, 1.55, PZ), (-26.2, 1.55, 1.42, PZ)]
PYS = [-12.6, -14.5, -17.0, -20.0, -23.0, -26.2]
PK = dict(top=0.7, bot=0.7, hi=0.6, lo=-0.6)


def hs(y, i):
    return interp(HS0, y, i)


def hull_bottom(y):
    return hs(y, 3) - hs(y, 2)


def wrow(bm, a, b, out, seed=0, skip=4, pitch=0.62, w=0.3, h=0.16):
    """A deck of windows at cruiser scale."""
    cap.win_row(bm, a, b, out, pitch=pitch, w=w, h=h, skip=skip, seed=seed)


def hwin(bm, y0, y1, hw, z, ch=0.4, seed=0, front=False, x=0.0):
    for sx in (1, -1):
        wrow(bm, (x + sx * hw, y0 - ch - 0.25, z), (x + sx * hw, y1 + ch + 0.25, z), (sx, 0, 0), seed=seed + (sx > 0))
    if front:
        wrow(bm, (x - hw + ch + 0.15, y0, z), (x + hw - ch - 0.15, y0, z), (0, 1, 0), seed=seed, skip=0)


def build(ship):
    cap.AREA.clear()

    # ---------------------------------------------------------------- hull
    def hull(bm):
        rings = [ring10(y, hs(y, 1), hs(y, 2), hs(y, 3), **HK) for y in YS]
        res = loft(bm, rings, matfn=lambda i, k: L if ((seg_sym(k) == 1 and 1 <= i <= 12 and i != 8) or (seg_sym(k) == 2 and i in (3, 4))) else H)
        paint(bm, pick(res, lambda i, k: seg_sym(k) in (3, 4)), RED)
        paint(bm, pick(res, lambda i, k: seg_sym(k) == 2 and i >= 5), FRAME)
        paint(bm, [res['start']], FRAME)
        inset(bm, pick(res, lambda i, k: seg_sym(k) == 2 and i in (1, 2)), 0.16, -0.05)
        inset(bm, pick(res, lambda i, k: seg_sym(k) == 1 and i in (14, 16)), 0.14, -0.05)
        inset(bm, pick(res, lambda i, k: seg_sym(k) == 1 and i in (8, 13)), 0.2, -0.2, t=FRAME)
        inset(bm, pick(res, lambda i, k: seg_sym(k) == 3 and i % 2 == 0 and i > 2), 0.16, -0.06, t=RED)
        inset(bm, pick(res, lambda i, k: seg_sym(k) == 4 and i in (6, 9, 12, 15)), 0.3, -0.14, t=FRAME)
        inset(bm, pick(res, lambda i, k: seg_sym(k) == 0 and i in (0, 1, 2, 16, 17)), 0.12, -0.04)
        inset(bm, [res['end']], 0.2, -0.3, t=FRAME)
    P(ship, hull, bevel=0.04)

    def belts(bm):
        for j, (y0, y1) in enumerate(BELTS):
            res = bs.side_plate(bm, HS0, y0, y1, -0.45, 1.12, 0.2, n=2, bevel_in=0.12)
            if j in (2, 4):
                paint(bm, pick(res, lambda i, k: True), GREY)
            if j in (0, 3, 5):
                for f in pick(res, lambda i, k: k == 1):
                    f.material_index = L
        # deck-edge sponson under the secondary guns
        for y0, y1 in ((6.5, 1.5),):
            xs = [(y, hs(y, 1) * 0.8) for y in (y0, y1)]
            loft(bm, [[V((x - 0.3, y, DECK - 0.7)), V((x + 0.95, y, DECK - 0.5)), V((x + 0.95, y, DECK + 0.16)), V((x - 0.3, y, DECK + 0.16))]
                      for y, x in xs])
        # torpedo flat amidships: a shoulder on the belt with four tubes in a dark bay
        def sec(y, f):
            x = hs(y, 1)
            return [V((x - 0.25, y, -0.8)), V((x + 0.8 * f, y, -0.5)), V((x + 0.95 * f, y, 0.95)), V((x + 0.6 * f, y, 1.5)), V((x - 0.25, y, 1.62))]
        res = loft(bm, [sec(0.9, 0.1), sec(-0.3, 1.0), sec(-6.5, 1.0), sec(-7.7, 0.1)], matfn=lambda i, k: L if k in (2, 3) else H)
        inset(bm, [res['bands'][1][1]], 0.22, -0.55, t=FRAME)
        with tone(bm, GREY):
            for y in (-1.5, -2.8, -4.0, -5.3):
                cyl(bm, (hs(y, 1) + 0.2, y, 0.22), (hs(y, 1) + 0.62, y, 0.24), 0.4, n=10)
        with tone(bm, FRAME):
            for y in (-1.5, -2.8, -4.0, -5.3):
                cyl(bm, (hs(y, 1) + 0.6, y, 0.24), (hs(y, 1) + 0.66, y, 0.24), 0.28, n=10)
    P(ship, belts, bevel=0.03, segs=1, mirror=True)

    def hull_bits(bm):
        with tone(bm, FRAME):
            for i in (8, 13):   # conduits in the hull trenches
                ya, yb = YS[i] - 0.35, YS[i + 1] + 0.35
                for o in (0.35, 0.65):
                    pa = V((hs(ya, 1) * (0.8 + 0.2 * o) - 0.14, ya, DECK - (DECK - hs(ya, 3) - hs(ya, 2) * 0.7) * o - 0.08))
                    pb = V((hs(yb, 1) * (0.8 + 0.2 * o) - 0.14, yb, DECK - (DECK - hs(yb, 3) - hs(yb, 2) * 0.7) * o - 0.08))
                    cyl(bm, pa, pb, 0.1, n=6)
            for y in (25.2,):   # bow thrusters
                for z in (0.75, 0.0):
                    vernier(bm, (hs(y, 1) - 0.03, y, z), (1, 0, 0), 0.22, 0.28)
            for y in (22.0, -10.0):
                for o in (0.0, 1.0):
                    vernier(bm, (hs(y, 1) * 0.3, y - o, hull_bottom(y - o) - 0.12), (0, 0, -1), 0.34, 0.4)
    P(ship, hull_bits, bevel=0.0, mirror=True)

    # ---------------------------------------------------------------- bow: cannon muzzle, long blade prow, bulb
    BC = V((0, 27.9, 0.45))
    k = 0.85

    def bow(bm):
        def pr(*pts):
            return [(r * k, t) for r, t in pts]
        lathe(bm, BC, (0, 1, 0), pr((1.6, 0.0), (1.6, 1.0), (1.5, 1.15), (1.5, 1.45), (1.36, 1.75), (1.12, 1.75)),
              n=24, cap_start=False, cap_end=False)
        with tone(bm, YEL):
            lathe(bm, BC, (0, 1, 0), pr((1.64, 0.82), (1.64, 1.0)), n=24, cap_start=False, cap_end=False)
        with tone(bm, FRAME):
            lathe(bm, BC, (0, 1, 0), pr((1.12, 1.75), (1.04, 1.5), (1.04, -0.9), (0.62, -1.0)), n=24, cap_start=False, cap_end=False)
            lathe(bm, BC, (0, 1, 0), pr((0.3, -1.0), (0.0, -0.7)), n=24, cap_start=False)
            for a in range(10):
                ang = a * math.pi / 5
                d = V((math.cos(ang), 0, math.sin(ang)))
                prism(bm, BC + V((0, -0.85, 0)) + d * k, BC + V((0, 1.4, 0)) + d * k, 0.07, 0.05, up=d)
        lathe(bm, BC, (0, 1, 0), pr((0.62, -1.0), (0.3, -1.0)), n=24, mat=G, cap_start=False, cap_end=False)
        for t in (0.1, 0.75):
            lathe(bm, BC, (0, 1, 0), pr((1.045, t), (0.98, t + 0.04), (0.98, t + 0.12), (1.045, t + 0.16)), n=24, mat=G,
                  cap_start=False, cap_end=False)
        # blade prow: longer and finer than the flagship's
        loft(bm, [ring10(27.3, 1.75, 0.34, 1.86, top=0.7, bot=0.5, hi=0.4, lo=-0.5),
                  ring10(28.6, 1.1, 0.3, 1.98, top=0.65, bot=0.4, hi=0.4, lo=-0.5),
                  ring10(29.45, 0.45, 0.2, 2.14, top=0.6, bot=0.3, hi=0.4, lo=-0.5),
                  V((0, 30.0, 2.3))], matfn=lambda i, k_: L if seg_sym(k_) in (1, 2) else H)
        with tone(bm, RED):
            lathe(bm, (0, 26.6, -1.45), (0, 1, 0), [(0.7, 0.0), (0.85, 0.8), (0.8, 1.5), (0.52, 2.05), (0.0, 2.4)], n=14)
        for sx in (1, -1):
            with tone(bm, FRAME):   # anchor recess, chain
                cyl(bm, (sx * hs(26.2, 1) - sx * 0.14, 26.2, 1.0), (sx * hs(26.2, 1) + sx * 0.08, 26.2, 1.0), 0.4, n=10)
                prism(bm, (sx * 0.75, 27.2, DECK + 0.03), (sx * 0.6, 24.3, DECK + 0.03), 0.045, 0.04)
                cyl(bm, (sx * 0.75, 27.2, DECK), (sx * 0.75, 27.2, DECK + 0.2), 0.17, n=8)
            with tone(bm, GREY):   # capstans, breakwater, hatches
                cyl(bm, (sx * 0.6, 24.3, DECK), (sx * 0.6, 24.3, DECK + 0.3), 0.28, n=10)
                prism(bm, (0, 23.9, DECK + 0.18), (sx * 2.0, 23.0, DECK + 0.18), 0.06, 0.22)
                box(bm, (sx * 0.75, 25.8, DECK + 0.04), (0.7, 0.8, 0.1))
    P(ship, bow, bevel=0.025, segs=1)

    def bow_planes(bm):
        wing(bm, wst(dict(x=2.0, z=0.1, le=25.2, te=22.4, t=0.34), dict(x=4.3, z=0.1, le=22.9, te=21.7, t=0.12), 2),
             matfn=lambda i, k_: L if i >= 1 else H)
        bs.light(bm, (4.35, 22.3, 0.1), 0.1)
    P(ship, bow_planes, bevel=0.02, mirror=True)

    # ---------------------------------------------------------------- superstructure
    BH = (16.6, 11.6, 2.25, DECK, 3.3)       # B barbette house
    T1 = (10.9, -7.0, 2.7, DECK, 3.05)
    T2 = (7.4, -7.0, 1.9, 3.05, 4.05)
    HG = (-7.0, -13.6, 2.55, DECK, 4.25)     # hangar, open aft

    def tiers(bm):
        for (y0, y1, hw, z0, z1), ch, liv in ((T1, 0.7, False), (T2, 0.55, False), (BH, 0.6, True)):
            with tone(bm, FRAME):
                house(bm, y0 - 0.12, y1 + 0.12, hw - 0.12, z0 - 0.05, z0 + 0.24, ch=ch, rf=0, rb=0)
            res = house(bm, y0, y1, hw, z0 + 0.2, z1, ch=ch, rf=0.3, mat=L if liv else H)
            for f in (res['start'], res['end']):
                f.material_index = H
            inset(bm, [res['end']], 0.2, -0.04, t=GREY)
        y0, y1, hw, z0, z1 = HG
        with tone(bm, FRAME):
            house(bm, y0 - 0.12, y1 + 0.12, hw - 0.12, z0 - 0.05, z0 + 0.24, ch=0.6, rf=0, rb=0)
        res = house(bm, y0, y1, hw, z0 + 0.2, z1, ch=0.6, rf=0.0, rb=0.2, matfn=lambda i, k_: L if k_ in (1, 5) else H)
        inset(bm, [res['bands'][0][3]], 0.24, -2.4, t=GREY)       # the hangar mouth
        inset(bm, [res['end']], 0.22, -0.05, t=GREY)
        inset(bm, [res['bands'][0][1], res['bands'][0][5]], 0.3, -0.05)
        with tone(bm, FRAME):
            for sx in (1, -1):   # recessed gallery under the tier roof
                box(bm, (sx * (T1[2] + 0.02), (T1[0] + T1[1]) / 2 - 0.2, 2.42), (0.05, T1[0] - T1[1] - 2.0, 0.07))
    P(ship, tiers, bevel=0.035)

    # ---------------------------------------------------------------- pagoda bridge (three storeys) and funnel
    TW = [  # y0, y1, hw, z0, z1, ins, rf
        (6.0, 1.2, 1.4, 4.05, 5.25, 0.14, 0.3),
        (6.6, 1.3, 1.8, 5.25, 5.42, 0.0, 0.0),
        (5.6, 1.7, 1.1, 5.42, 6.4, 0.1, 0.22),
        (6.15, 1.8, 1.5, 6.4, 6.55, 0.0, 0.0),
        (5.3, 2.0, 0.92, 6.55, 7.3, 0.08, 0.18),
        (5.85, 2.1, 1.3, 7.3, 7.42, 0.0, 0.0),
        (5.7, 2.2, 1.15, 7.42, 8.2, 0.0, -0.35),
        (4.8, 2.4, 0.75, 8.2, 8.8, 0.08, 0.18),
    ]
    BZ, TZ = 7.42, 8.8
    TY = 3.6   # tower mid

    def tower(bm):
        for j, (y0, y1, hw, z0, z1, ins, rf) in enumerate(TW):
            with tone(bm, GREY if j in (1, 3, 5) else 0):
                res = house(bm, y0, y1, hw, z0, z1, ch=0.35 if hw < 1.3 else 0.45, rf=rf, rb=0.12, ins=ins, mat=L if j == 0 else H)
                res['end'].material_index = H
            if j in (1, 3, 5):
                for sx in (1, -1):
                    with tone(bm, GREY):
                        cyl(bm, (sx * (hw + 0.12), TY - 0.6, z0), (sx * (hw + 0.12), TY - 0.6, z1 + 0.18), 0.42 if j < 5 else 0.3, n=10)
        # funnel, raked aft, blue with a dark cap
        res = house(bm, -1.2, -4.4, 1.0, 4.05, 6.5, ch=0.35, rf=1.4, rb=-1.1, ins=0.2, mat=L)
        res['end'].material_index = H
        inset(bm, [res['end']], 0.14, -0.45, t=FRAME)
        with tone(bm, GREY):
            house(bm, -2.5, -5.58, 0.86, 6.28, 6.46, ch=0.3, rf=0.05, rb=-0.05)
        with tone(bm, FRAME):
            house(bm, -1.0, -4.6, 1.2, 4.03, 4.35, ch=0.35, rf=0, rb=0)
        # flight control on the hangar roof
        house(bm, -8.2, -10.2, 0.95, 4.25, 5.25, ch=0.3, ins=0.1)
        with tone(bm, GREY):
            house(bm, -8.0, -10.35, 1.2, 5.25, 5.4, ch=0.32, rf=0, rb=0)
        # ventral bridge on its fin
        with tone(bm, RED):
            house(bm, 4.6, -1.6, 0.8, -3.3, -4.15, ch=0.3, rf=0.9, rb=0.6, ins=0.28)
        house(bm, 3.3, -0.7, 0.75, -4.1, -4.75, ch=0.3, rf=-0.35, rb=0.1, ins=0.0)
    P(ship, tower, bevel=0.03)

    def tower_bits(bm):
        with tone(bm, GREY):
            prism(bm, (-2.4, 3.5, TZ - 0.3), (2.4, 3.5, TZ - 0.3), 0.16, 0.16)   # rangefinder arms
            for sx in (1, -1):
                box(bm, (sx * 2.4, 3.5, TZ - 0.3), (0.36, 0.6, 0.44))
            box(bm, (0, 2.8, TZ + 0.55), (1.5, 0.1, 0.75))                    # array panel
            box(bm, (0, 2.8, TZ + 0.1), (0.24, 0.24, 0.3))
            lathe(bm, (0, -9.2, 5.85), (0, -0.5, 1), [(0.0, 0.0), (0.55, 0.1), (0.8, 0.3), (0.75, 0.33), (0.5, 0.15), (0.0, 0.06)], n=14)
            cyl(bm, (0, -9.2, 5.4), (0, -9.2, 5.9), 0.09, n=6)
            for sx in (1, -1):   # directors on the first platform
                sphere(bm, (sx * 1.45, 5.3, 5.42 + 0.32), 0.34, seg=10, rings=5)
        with tone(bm, FRAME):
            cyl(bm, (0, 2.6, TZ), (0, 2.4, TZ + 1.7), 0.09, 0.04, n=6)   # main mast
            for z, w in ((TZ + 0.85, 0.95), (TZ + 1.3, 0.55)):
                prism(bm, (-w, 2.6 - (z - TZ) * 0.12, z), (w, 2.6 - (z - TZ) * 0.12, z), 0.035, 0.035)
            bs.antenna(bm, (0.5, 2.5, TZ), 1.0, r=0.03, tip=False)
            bs.antenna(bm, (-0.5, 2.5, TZ), 1.0, r=0.03, tip=False)
            bs.antenna(bm, (0, -4.9, 6.45), 1.5, r=0.035)
            for sx in (1, -1):   # tower braces
                prism(bm, (sx * 1.75, 2.2, 4.05), (sx * 1.05, 2.2, 5.3), 0.07, 0.16)
                prism(bm, (sx * 1.4, 2.6, 5.42), (sx * 1.0, 2.6, 6.4), 0.06, 0.13)
        bs.light(bm, (0, 2.4, TZ + 1.73), 0.08)
        with tone(bm, YEL):
            for sx in (1, -1):
                wing(bm, [dict(x=sx * 0.1, z=TZ - 0.1, le=5.0, te=4.3, t=0.14),
                          dict(x=sx * 0.9, z=TZ + 0.5, le=4.45, te=4.05, t=0.1),
                          dict(x=sx * 1.8, z=TZ + 1.15, le=3.75, te=3.58, t=0.04)], flat_te=0.3)
        with tone(bm, RED):
            box(bm, (0, 4.65, TZ + 0.07), (0.34, 0.5, 0.16))
    P(ship, tower_bits, bevel=0.01, segs=1)

    # ---------------------------------------------------------------- batteries
    guns = []

    def batteries(bm):
        guns.extend(turret(bm, (0, 21.0, DECK), 1, s=1.42))
        guns.extend(turret(bm, (0, 14.2, 3.3), 1, s=1.42))
    P(ship, batteries, bevel=0.02, segs=1)

    def secondaries(bm):
        for sx in (1, -1):
            turret(bm, (sx * 1.55, 9.2, 3.05), 1, s=0.58)
            turret(bm, (sx * 1.45, -11.6, 4.25), -1, s=0.62)
            turret(bm, (sx * 3.8, 4.0, DECK + 0.16), 1, s=0.5, elev=12)
    P(ship, secondaries, bevel=0.0)
    for i, g in enumerate(guns):
        ship.empty("Gun_%d" % i, g, 0.2)

    def flak(bm):
        for i, y in enumerate((8.6, 0.2, -2.6, -5.4)):
            aa(bm, (3.02, y, DECK), (1, 0.3 if i < 2 else -0.3, 0), s=0.55)
        for y in (5.6, -0.2, -5.8):
            aa(bm, (2.3, y, 3.05), (1, 0.2, 0), s=0.5)
        for y in (24.6, 18.0, -15.8):
            aa(bm, (hs(y, 1) * 0.8 - 0.55, y, DECK), (1, 0.4 if y > 0 else -0.4, 0), s=0.55)
        for y in (12.0, -6.0):
            aa(bm, (0.9, y, hull_bottom(y) + 0.03), (1, 0, 0), s=0.55, up=-1)
        aa(bm, (2.0, -9.0, 4.25), (1, -0.3, 0), s=0.5)
        with tone(bm, GREY):
            for ix in range(2):   # launch cells abaft the funnel
                for y in (-5.4, -6.3):
                    box(bm, (0.45 + ix * 0.8, y, 4.05 + 0.05), (0.6, 0.6, 0.1))
            for y in (-0.3, -2.9):   # boats along the tier
                box(bm, (2.25, y - 1.3, 3.2), (0.5, 1.6, 0.26))
            sphere(bm, (1.5, -6.2, 4.05 + 0.3), 0.4, seg=10, rings=5)
            box(bm, (1.6, -8.5, 4.3), (0.9, 1.2, 0.12))
    P(ship, flak, bevel=0.0, mirror=True)

    # ---------------------------------------------------------------- catapult deck
    CZ = 2.32

    def catapult(bm):
        with tone(bm, GREY):
            r2 = slab(bm, [(2.1, -13.3), (2.5, -14.2), (2.5, -25.3), (1.7, -26.6), (-1.7, -26.6), (-2.5, -25.3), (-2.5, -14.2), (-2.1, -13.3)],
                      lambda x, y: 1.9, CZ - 1.9, chamf=0.1)
            inset(bm, [r2['end']], 0.3, -0.06, t=FRAME)
        zt = CZ - 0.06
        with tone(bm, YEL):
            for y in (-14.6, -25.0):
                box(bm, (0, y, zt), (3.6, 0.18, 0.05))
            for sx in (1, -1):
                box(bm, (sx * 1.75, -19.8, zt), (0.1, 10.2, 0.04))
        with tone(bm, FRAME):
            for o in (-0.35, 0.35):
                prism(bm, (o, -14.9, zt + 0.02), (o, -24.7, zt + 0.02), 0.07, 0.05)
            box(bm, (0, -17.0, zt + 0.1), (1.0, 0.8, 0.16))                       # shuttle
            box(bm, (0, -14.2, CZ + 0.3), (3.2, 0.1, 0.7), rot=Matrix.Rotation(math.radians(25), 3, 'X'))   # blast deflector
        with tone(bm, GREY):
            for sx in (1, -1):   # deck-edge crane and lockers
                box(bm, (sx * 2.2, -22.0, CZ + 0.12), (0.5, 1.4, 0.3))
                cyl(bm, (sx * 2.2, -16.0, CZ - 0.1), (sx * 2.2, -16.0, CZ + 0.9), 0.14, n=8)
                prism(bm, (sx * 2.2, -16.0, CZ + 0.85), (sx * 1.1, -17.6, CZ + 1.15), 0.06, 0.08)
    P(ship, catapult, bevel=0.015, segs=1)

    # ---------------------------------------------------------------- nacelles on stub pylons, tail fins
    def nacelle(bm):
        rings = [ring10(y, interp(PS, y, 1), interp(PS, y, 2), interp(PS, y, 3), PX, **PK) for y in PYS]
        res = loft(bm, rings, matfn=lambda i, k_: L if ((seg_sym(k_) in (0, 1) and i <= 3) or (seg_sym(k_) == 2 and i <= 2)) else H)
        paint(bm, pick(res, lambda i, k_: seg_sym(k_) in (3, 4)), RED)
        paint(bm, pick(res, lambda i, k_: i == 4), GREY)
        inset(bm, [res['start']], 0.16, -1.7, t=GREY)                 # intake
        inset(bm, pick(res, lambda i, k_: i == 3 and seg_sym(k_) == 2), 0.2, -0.07)
        inset(bm, pick(res, lambda i, k_: i == 4 and seg_sym(k_) in (1, 2, 3)), 0.14, -0.06, t=FRAME)
        inset(bm, pick(res, lambda i, k_: i in (2, 3) and seg_sym(k_) == 0), 0.16, -0.05, t=GREY)
        inset(bm, [res['end']], 0.16, -0.3, t=FRAME)
        with tone(bm, YEL):
            box(bm, (PX, -12.63, PZ + 1.05), (1.7, 0.12, 0.08))
        # pylon to the hull: a thick grey root with a blue cap and a red sole
        with tone(bm, GREY):
            r2 = slab(bm, [(3.0, -14.8), (5.2, -16.4), (5.2, -24.6), (3.0, -24.6)], lambda x, y: -1.1, 1.6, chamf=0.12)
            inset(bm, [r2['end']], 0.3, -0.07, t=FRAME)
        slab(bm, [(3.9, -17.0), (5.0, -17.6), (5.0, -22.4), (3.9, -22.4)], lambda x, y: 0.45, 0.3, chamf=0.08, mat=L)
        with tone(bm, RED):
            slab(bm, [(3.2, -15.6), (5.1, -17.0), (5.1, -24.0), (3.2, -24.0)], lambda x, y: -1.4, 0.35, chamf=0.03)
        with tone(bm, FRAME):
            bs.vent(bm, (4.45, -23.4, 0.52), 1.0, 1.3, n=5, h=0.1)
            for y in (-16.0, -22.5):
                vernier(bm, (PX + interp(PS, y, 1) - 0.03, y, PZ), (1, 0, 0), 0.26, 0.3)
        # tail fins, canted, off the nacelle shoulders
        a = dict(x=PX + 0.5, z=PZ + 1.3, le=-18.6, te=-25.6, t=0.55)
        b = dict(x=PX + 2.0, z=PZ + 4.9, le=-23.6, te=-26.9, t=0.2)
        res = wing(bm, wst(a, b, 3), matfn=lambda i, k_: L if i <= 1 else H)
        paint(bm, res['bands'][2], GREY)
        inset(bm, pick(res, lambda i, k_: i == 0 and k_ in (2, 4)), 0.14, -0.04)
        bs.light(bm, (PX + 2.03, -26.9, PZ + 4.93), 0.1)
        a = dict(x=PX + 0.5, z=PZ - 1.3, le=-20.0, te=-25.6, t=0.45)
        b = dict(x=PX + 1.3, z=PZ - 3.3, le=-23.8, te=-26.5, t=0.16)
        wing(bm, wst(a, b, 2), mat=L)
    P(ship, nacelle, bevel=0.03, mirror=True)

    # ---------------------------------------------------------------- engine block
    ES = [(-21.0, 3.3, 2.4, -0.5), (-22.6, 3.85, 2.7, -0.6), (-27.0, 3.7, 2.6, -0.6)]
    ENG = [((0, -30.0, -0.6), 2.25, 3.8), ((PX, -29.3, PZ), 1.42, 3.3), ((-PX, -29.3, PZ), 1.42, 3.3)]

    def block(bm):
        res = loft(bm, [ring10(y, hw, hh, cz, top=0.72, bot=0.72, hi=0.62, lo=-0.62) for y, hw, hh, cz in ES],
                   matfn=lambda i, k_: L if (seg_sym(k_) in (1, 2) and i == 1) else H)
        paint(bm, pick(res, lambda i, k_: seg_sym(k_) in (3, 4)), RED)
        inset(bm, [res['end']], 0.25, -0.5, t=FRAME)
        inset(bm, pick(res, lambda i, k_: i == 1 and seg_sym(k_) == 2), 0.3, -0.08, mat=L)
    P(ship, block, bevel=0.03, segs=1)

    def engines(bm):
        for c, r, ln in ENG:
            engine(bm, c, r, ln, petals=20 if r > 2 else 14, n=32 if r > 2 else 24)
        with tone(bm, FRAME):
            for a in range(8):
                ang = a * math.pi / 4 + math.pi / 8
                d = V((math.cos(ang), 0, math.sin(ang)))
                cyl(bm, V((0, -26.4, -0.6)) + d * 3.0, V((0, -27.9, -0.6)) + d * 2.85, 0.12, n=6)
    P(ship, engines, bevel=0.0)
    for i, (c, r, ln) in enumerate(ENG):
        ship.empty("Engine_%d" % i, c, r)

    # ---------------------------------------------------------------- lit windows and deck lights
    def lights(bm):
        for j, (y0, y1) in enumerate(BELTS):
            for sx in (1, -1):
                wrow(bm, (sx * (hs(y0, 1) + 0.22), y0 - 0.5, 0.45), (sx * (hs(y1, 1) + 0.22), y1 + 0.5, 0.45), (sx, 0, 0),
                     seed=j * 3 + (sx > 0))
        hwin(bm, T1[0], T1[1], T1[2], 2.72, ch=0.7, seed=1)
        hwin(bm, T2[0], T2[1], T2[2], 3.6, ch=0.55, seed=2)
        hwin(bm, BH[0], BH[1], BH[2], 2.78, ch=0.6, seed=4)
        hwin(bm, 5.7, 1.3, 1.4 - 0.14 * 0.5, 4.65, ch=0.45, seed=5)
        hwin(bm, 5.4, 1.8, 1.1 - 0.1 * 0.5, 5.9, ch=0.35, seed=6)
        hwin(bm, 5.1, 2.1, 0.92 - 0.04, 6.92, ch=0.35, seed=8)
        # the bridge: one unbroken band of glass
        yb = 5.7 + 0.35 * 0.5
        bs.strip(bm, (-0.8, yb + 0.02, BZ + 0.45), (0.8, yb + 0.02, BZ + 0.45), 0.04, 0.13)
        for sx in (1, -1):
            bs.strip(bm, (sx * 1.15, 5.2, BZ + 0.45), (sx * 1.15, 2.8, BZ + 0.45), 0.04, 0.13)
        wrow(bm, (-0.45, 4.8 - 0.09, TZ - 0.3), (0.45, 4.8 - 0.09, TZ - 0.3), (0, 1, 0), skip=0, pitch=0.3, w=0.2, h=0.12)
        hwin(bm, -8.2, -10.2, 0.9, 4.8, ch=0.3, seed=7, front=True)
        # ventral bridge
        bs.strip(bm, (-0.5, 3.3 + 0.175 + 0.02, -4.42), (0.5, 3.3 + 0.175 + 0.02, -4.42), 0.04, 0.1)
        hwin(bm, 3.0, -0.4, 0.75, -4.42, ch=0.3, seed=9)
        # hangar interior: lit back wall, ceiling strips, floor lights
        yh = HG[1] + 2.4
        box(bm, (0, yh - 0.12, 3.1), (3.2, 0.05, 0.9), mat=G)
        for o in (-1.0, 1.0):
            bs.strip(bm, (o, yh - 0.3, 4.0), (o, HG[1] + 0.5, 4.0), 0.08, 0.03)
        # catapult guide lights
        for o in (-1.3, 1.3):
            wrow(bm, (o, -15.2, CZ - 0.05), (o, -24.6, CZ - 0.05), (0, 0, 1), pitch=1.3, w=0.2, h=0.1, skip=0)
        for sx in (1, -1):
            # nacelle and engine block galleries
            wrow(bm, (sx * (PX + 1.7), -17.4, PZ + 0.35), (sx * (PX + 1.7), -22.6, PZ + 0.35), (sx, 0, 0), seed=11 + (sx > 0))
            wrow(bm, (sx * 3.85, -23.0, 0.3), (sx * 3.72, -26.6, 0.3), (sx, 0, 0), seed=13 + (sx > 0))
    P(ship, lights, bevel=0.0)

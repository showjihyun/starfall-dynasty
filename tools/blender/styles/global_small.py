"""Style `global`, small ship: the one-seat fighter a Human captain starts in, 24 m.

One silhouette: the flagship's trident at fighter size. A chined spearhead hull with a big bubble canopy at the
front of its dorsal spine, a swept wing carrying two engine nacelles with pointed prows, outer wing panels that
droop to two long wingtip cannons, canted tail fins on the nacelles and one stern engine between them.
Same language as styles/global.py (lofted sections, white body, a crown sweep of faction colour whose edges are
mesh edges, one warm pinstripe, ring nozzles, thin light lines), but nothing that says "big": no windows, no
turrets, no bridge; the canopy is the scale cue. 60 units long (1 unit is 0.4 m).
"""
import importlib
import math

import bmesh
from mathutils import Vector

import build_ships as bs
from shiplib import H, L, G, GL, E, loft, box, prism, cyl, sphere

cap = importlib.import_module("styles.global")
crom, at, sec_pt, fsec, lathe_y, foil, foil_h, frange = (cap.crom, cap.at, cap.sec_pt, cap.fsec, cap.lathe_y,
                                                         cap.foil, cap.foil_h, cap.frange)
V = Vector
D = math.radians

STYLE = dict(cap.STYLE, spacing=(4.4, 6.0, 3.6), seam_w=0.036, sub_scale=0.11, sub_amt=0.2)
TINT = dict(cap.TINT)

NX, NZ = 12.5, 0.2                  # nacelle axis
NY0, NY1, NR = -25.0, -30.0, 1.9    # nacelle nozzle: collar, exit, radius
CY0, CY1, CR, CZ = -24.5, -28.4, 1.7, 0.4  # stern nozzle
GX, GZ = 21.1, -1.2                 # wingtip cannon axis
GY = -0.6                           # its muzzle
EX, EZT, EZB = cap.EX, cap.EZT, cap.EZB

# ================================================================ sections

FS = [  # y, half width, height above / below the chine, chine z
    (29.8, 0.06, 0.05, 0.05, -0.5),
    (28.2, 0.7, 0.42, 0.4, -0.5),
    (25.0, 1.7, 0.95, 0.9, -0.45),
    (20.0, 2.9, 1.5, 1.45, -0.36),
    (14.0, 4.1, 2.0, 1.95, -0.25),
    (7.0, 5.1, 2.4, 2.3, -0.12),
    (0.0, 5.7, 2.6, 2.5, 0.0),
    (-7.0, 5.7, 2.65, 2.5, 0.1),
    (-13.0, 5.1, 2.6, 2.35, 0.2),
    (-18.0, 4.1, 2.45, 2.2, 0.3),
    (-22.0, 3.0, 2.2, 2.05, 0.36),
    (-24.5, 2.3, 2.0, 1.92, 0.4),
]
FM, FN, FNB, FNC = 60, 48, 7, 5
FSS = crom(FS, FM)

WS = [  # wing: x, z, leading edge y, trailing edge y, thickness
    (3.0, 0.0, 9.0, -20.0, 1.7),
    (6.0, 0.05, 3.0, -20.5, 1.9),
    (9.5, 0.12, -3.5, -21.2, 1.8),
    (12.5, 0.2, -8.5, -22.0, 1.6),
    (16.6, -0.35, -11.8, -23.4, 1.1),
    (21.0, -1.2, -15.0, -25.2, 0.6),
]
WM = 30
WSS = crom(WS, WM)
WL0, WL1 = WSS[23][0], WSS[28][0]   # the outer panel's paint band

OS = [  # nacelles: y, half width, height above / below, centre z
    (4.0, 0.05, 0.05, 0.05, NZ - 0.1),
    (2.6, 0.62, 0.66, 0.6, NZ - 0.1),
    (-0.5, 1.25, 1.35, 1.2, NZ - 0.06),
    (-6.0, 1.8, 1.95, 1.75, NZ),
    (-13.0, 2.05, 2.2, 2.0, NZ),
    (-20.0, 2.05, 2.2, 2.0, NZ),
    (-25.0, 1.98, 2.1, 1.95, NZ),
]
OM, ON, ONB, ONC = 30, 24, 3, 3
OSS = crom(OS, OM)
OKW = dict(ex=0.5, ezt=0.6, ezb=0.6)

SUP = [  # dorsal spine: y, half width, top above the hull crown
    (23.6, 0.15, 0.05), (21.5, 1.3, 0.4), (17.0, 2.3, 0.8), (10.0, 2.8, 1.15), (2.0, 2.6, 1.35), (-6.0, 2.2, 1.3),
    (-14.0, 1.7, 1.0), (-20.0, 1.1, 0.6), (-23.6, 0.15, 0.05),
]
SUPS = crom(SUP, 36)
SKW = dict(ex=0.5, ezt=0.62)

VS = [(12.0, 0.1, 0.05), (9.6, 1.2, 0.5), (3.0, 2.2, 0.95), (-8.0, 2.4, 1.05), (-16.0, 1.8, 0.8), (-21.0, 0.9, 0.4),
      (-23.4, 0.1, 0.05)]  # ventral fairing: y, half width, depth below the hull
VSS = crom(VS, 28)
VKW = dict(ex=0.5, ezt=0.6, ezb=0.6)

CAN = crom([(21.3, 0.5, 0.3), (20.3, 1.0, 0.8), (17.8, 1.75, 1.6), (14.8, 2.1, 2.15), (11.8, 2.15, 2.3),
            (9.2, 1.85, 1.9), (7.2, 1.1, 0.95), (6.4, 0.5, 0.35)], 28)  # canopy: y, half width, height
CF = (17.0, 10.2)                    # its two hoops

FIN = [(NX + 0.45, 1.5, -12.5, -23.8, 1.0), (NX + 1.1, 3.4, -16.0, -24.6, 0.8), (NX + 1.8, 5.4, -19.4, -25.5, 0.55),
       (NX + 2.4, 7.1, -22.2, -26.3, 0.25)]
FINS = crom(FIN, 12)
FL0, FL1 = FINS[5][1], FINS[9][1]   # its paint band (z)


def fus(y):
    return at(FSS, y)


def _arch(y, x):
    hw = fus(y)[1]
    c = min(abs(x) / hw, 1.0) ** (1 / EX)
    return math.sqrt(max(1 - c * c, 0.0))


def fus_top(y, x=0.0):
    _, hw, ht, hb, cz = fus(y)
    return cz + ht * _arch(y, x) ** EZT


def fus_bot(y, x=0.0):
    _, hw, ht, hb, cz = fus(y)
    return cz - hb * _arch(y, x) ** EZB


def liv_lim(y):
    """How far down from the crown (fraction of the girth) the paint field reaches at station y."""
    pts = [(-24.6, 0.05), (-20.0, 0.12), (-11.0, 0.175), (2.0, 0.165), (10.0, 0.11), (21.0, 0.09), (25.0, 0.085), (27.7, 0.0)]
    return at(pts, y)[1]


def S_hull(y, a):
    return sec_pt(fus(y), a)


def sup_st(s):
    y, hw, top = s
    return (y, max(hw, 0.05), max(top, 0.02) + 0.9, 0.5, fus_top(y) - 0.9)


def sup_top(y):
    return fus_top(y) + at(SUPS, y)[2]


def sup_z(y, x):
    """Height of the spine's surface at (x, y)."""
    _, hw, ht, _, cz = sup_st(at(SUPS, y))
    c = min(abs(x) / hw, 1.0) ** 2
    return cz + ht * math.sqrt(max(1 - c * c, 0.0)) ** 0.62


def S_sup(y, a):
    return sec_pt(sup_st(at(SUPS, y)), a, **SKW)


def v_st(q):
    y, hw, d = q
    return (y, max(hw, 0.05), 0.4, max(d, 0.02) + 0.7, fus_bot(y) + 0.7)


def out_lim(y):
    pts = [(-26.0, 0.012), (-15.0, 0.012), (-7.0, 0.235), (5.0, 0.235)]
    return at(pts, y)[1]


def S_out(y, a):
    return sec_pt(at(OSS, y), a, **OKW) + V((NX, 0, 0))


def wing_z(x, y, top=True):
    _, z, le, te, t = at(WSS, x)
    u = min(max((le - y) / (le - te), 0.0), 1.0)
    return z + foil_h(u, t) * (1.0 if top else -0.8)


# ================================================================ material: the flagship's, with this hull's regions

DARK_REGIONS = [
    ((NX - 2.3, NX + 2.3), (None, NY0 - 0.42), (NZ - 2.4, NZ + 2.4)),          # nacelle nozzles
    ((None, 2.2), (None, CY0 - 0.42), (CZ - 2.2, CZ + 2.2)),                   # stern nozzle
    ((GX - 0.5, GX + 0.5), (-12.58, None), (GZ - 0.5, GZ + 0.5)),               # cannon barrels
]
PIN_REGIONS = [
    ((WL0 - 0.34, WL0 - 0.1), (None, None), (None, 1.2)),                      # outer wing
    ((None, 2.4), (24.5, 24.76), (None, None)),                                # nose ring
    ((NX - 1.7, NX + 1.7), (-0.2, 0.06), (None, None)),                        # nacelle prows
    ((NX, None), (None, None), (FL1 + 0.1, FL1 + 0.3)),                        # fins
]
LINE_REGIONS = [((None, None), (y - 0.04, y + 0.04), (None, None)) for y in (21.6, 3.6, -15.6)]


def materials():
    saved = cap.DARK_REGIONS, cap.PIN_REGIONS, cap.LINE_REGIONS
    cap.DARK_REGIONS, cap.PIN_REGIONS, cap.LINE_REGIONS = DARK_REGIONS, PIN_REGIONS, LINE_REGIONS
    try:
        return cap.materials()
    finally:
        cap.DARK_REGIONS, cap.PIN_REGIONS, cap.LINE_REGIONS = saved


# ================================================================ helpers

def nozzle(bm, cx, cz, y0, y1, r, n, npet, w):
    """The fleet's ring nozzle: light band at the collar, bell with petals, glowing rim, burning throat."""
    npf, nm = cap.nozzle_prof(y0, y1, r, glow=0.12)
    lathe_y(bm, cx, cz, [(y0 + 1.0, 0.0), (y0 + 1.0, r * 0.96)] + npf, n=n, mats=[H, H] + nm)
    ln = y0 - y1
    for a in range(npet):
        ang = (a + 0.5) * 2 * math.pi / npet
        d = V((math.cos(ang), 0, math.sin(ang)))
        c = V((cx, 0, cz))
        prism(bm, c + d * (r * 1.05) + V((0, y0 - ln * 0.12, 0)), c + d * (r * 0.94) + V((0, y1 + ln * 0.04, 0)),
              w, 0.085, w1=w * 0.84, h1=0.06, up=d, chamfer=0.15)


def chain(bm, pts, w, h):
    for a, b in zip(pts, pts[1:]):
        bs.strip(bm, a, b, w, h)


# ================================================================ the ship

def build(ship):
    # ---------------------------------------------------------------- hull
    def hull(bm):
        rings = [V((0, 30.0, -0.5))] + [fsec(st, FN, fb=max(liv_lim(st[0]), 0.012), nb=FNB, nc=FNC) for st in FSS]

        def mf(i, k):
            if i == 0:
                return H
            y = (FSS[i - 1][0] + FSS[i][0]) / 2
            return L if FNB <= k < FNB + 2 * FNC and liv_lim(y) > 0.012 else H
        loft(bm, rings, matfn=mf)
    ship.part(hull, bevel=0.05, segs=2, angle=38)

    # ---------------------------------------------------------------- dorsal spine
    def spine(bm):
        rings = [V((0, 23.8, fus_top(23.8) - 0.1))] + [fsec(sup_st(s), 28, **SKW) for s in SUPS] + [V((0, -23.8, fus_top(-23.8) - 0.1))]
        loft(bm, rings)
    ship.part(spine, bevel=0.05, segs=2, angle=38)

    # ---------------------------------------------------------------- ventral fairing: the gear bay
    def ventral(bm):
        rings = [V((0, 12.2, fus_bot(12.2) + 0.1))] + [fsec(v_st(q), 24, **VKW) for q in VSS] + [V((0, -23.6, fus_bot(-23.6) + 0.1))]
        loft(bm, rings)
    ship.part(ventral, bevel=0.04, segs=1, angle=34)

    # ---------------------------------------------------------------- canopy: one bubble, two hoops, a sill
    CN = 16
    ys = [c[0] for c in CAN]
    for f in CF:
        ys = [y for y in ys if abs(y - f) > 0.3] + [f + 0.16, f - 0.16]
    ys.sort(reverse=True)

    def can_ring(y, grow=1.0, lift=0.0):
        _, w, h = at(CAN, y)
        zb = sup_z(y, w) - 0.1
        angs = [0.0, D(17)] + [D(17) + D(146) * j / (CN - 2) for j in range(1, CN - 2)] + [D(163), math.pi]
        return [V((w * grow * math.cos(a), y, zb + lift + h * grow * math.sin(a) ** 0.85)) for a in angs]

    def canopy(bm):
        rings = [V((0, ys[0] + 0.5, sup_top(ys[0] + 0.5) - 0.06))] + [can_ring(y) for y in ys] + [V((0, ys[-1] - 0.5, sup_top(ys[-1] - 0.5) - 0.06))]

        def mf(i, k):
            if i == 0 or i == len(ys) or k in (0, CN - 1):
                return H
            ym = (ys[i - 1] + ys[i]) / 2
            return H if any(abs(ym - f) < 0.1 for f in CF) else GL
        loft(bm, rings, matfn=mf)
    ship.part(canopy, bevel=0.0, angle=50)

    # ---------------------------------------------------------------- wing
    def wing_paint(s, u, upper):
        return L if WL0 < s[0] < WL1 else H

    def wings(bm):
        foil(bm, WS, WM, paint=wing_paint,
             groove=lambda st: 0.09 * min(max((st[0] - 6.4) / 0.5, 0.0), 1.0) * min(max((20.0 - st[0]) / 2.5, 0.0), 1.0))
    ship.part(wings, bevel=0.04, segs=1, mirror=True, angle=40)

    # ---------------------------------------------------------------- nacelles
    def nacelle(bm):
        rings = [V((0, 4.1, NZ - 0.1))] + [fsec(st, ON, fb=out_lim(st[0]), nb=ONB, nc=ONC, **OKW) for st in OSS]
        for r in rings[1:]:
            for p in r:
                p.x += NX
        rings[0].x += NX
        loft(bm, rings, matfn=lambda i, k: L if ONB <= k < ONB + 2 * ONC and i > 0 and out_lim(OSS[i - 1][0]) > 0.02 else H)
    ship.part(nacelle, bevel=0.04, segs=1, mirror=True, angle=34)

    def nactrim(bm):
        nozzle(bm, NX, NZ, NY0, NY1, NR, 28, 12, 0.46)
        # canted tail fin and a short ventral stabiliser
        foil(bm, FIN, 12, sym=True, paint=lambda s, u, upper: L if FL0 < s[1] < FL1 else H)
        tip = FINS[-1]
        bs.light(bm, (tip[0] + 0.02, tip[3] - 0.02, tip[1] + 0.04), 0.14)
        foil(bm, [(NX + 0.5, -1.2, -15.0, -23.5, 0.7), (NX + 1.0, -2.3, -18.0, -24.3, 0.5), (NX + 1.5, -3.3, -21.0, -25.2, 0.22)],
             8, sym=True, paint=lambda s, u, upper: L if s[1] < -2.2 else H)
    ship.part(nactrim, bevel=0.0, mirror=True, angle=34)
    ship.empty("Engine_1", (NX, NY1, NZ), NR * 0.87)
    ship.empty("Engine_2", (-NX, NY1, NZ), NR * 0.87)

    # ---------------------------------------------------------------- stern engine
    def stern(bm):
        nozzle(bm, 0, CZ, CY0, CY1, CR, 28, 12, 0.42)
    ship.part(stern, bevel=0.0, angle=30)
    ship.empty("Engine_0", (0, CY1, CZ), CR * 0.87)

    # ---------------------------------------------------------------- wingtip cannons
    def cannons(bm):
        lathe_y(bm, GX, GZ, [(-26.6, 0.0), (-26.0, 0.36), (-23.6, 0.72), (-19.0, 0.82), (-15.0, 0.72), (-13.0, 0.5),
                             (-12.6, 0.4), (-12.6, 0.0)], n=16,
                mats=[H, H, L, L, H, H, H])
        cyl(bm, (GX, -12.8, GZ), (GX, GY - 0.9, GZ), 0.26, n=12)
        cyl(bm, (GX, -12.7, GZ), (GX, -9.2, GZ), 0.4, 0.37, n=12)
        for y in (-8.0, -6.6, -5.2):
            cyl(bm, (GX, y, GZ), (GX, y + 0.5, GZ), 0.34, n=12)
        cyl(bm, (GX, GY - 1.0, GZ), (GX, GY, GZ), 0.38, 0.41, n=12)
        bs.light(bm, (GX + 0.55, -25.4, GZ), 0.14)
    ship.part(cannons, bevel=0.0, mirror=True, angle=30)
    ship.empty("Gun_0", (GX, GY, GZ), 0.25)
    ship.empty("Gun_1", (-GX, GY, GZ), 0.25)

    # ---------------------------------------------------------------- fittings: few and large
    def fittings(bm):
        for y in frange(-7.6, -12.4, 0.8):  # heat exchanger louvres on the shoulders, lying on the crown
            a, b = V((3.0, y, fus_top(y, 3.0) + 0.02)), V((4.5, y - 0.5, fus_top(y - 0.5, 4.5) + 0.02))
            prism(bm, a, b, 0.2, 0.09, up=(0.45, 0, 1), chamfer=0.3)
        # manoeuvring thrusters: nose flanks and stern quarters
        p = S_hull(23.4, D(24))
        bs.rcs(bm, (p.x - 0.12, p.y, p.z - 0.05), (1, 0, 0.5), 0.42)
        p = S_hull(-20.5, D(30))
        bs.rcs(bm, (p.x - 0.14, p.y, p.z - 0.06), (1, 0, 0.6), 0.46)
        # access hatches on the inner wing
        for u in (0.36, 0.52):
            x = 7.9
            _, z, le, te, t = at(WSS, x)
            y = le - (le - te) * u
            prism(bm, (x, y + 1.1, wing_z(x, y + 1.1)), (x, y - 1.1, wing_z(x, y - 1.1)), 0.75, 0.06, mat=L, chamfer=0.12)
        # sensor blister under the nose
        sphere(bm, (0.9, 22.6, fus_bot(22.6, 0.9) + 0.05), 0.42, seg=12, rings=6, scale=(1, 2.1, 0.6))
        # flap actuator fairings under the inner wing
        for x in (7.4, 10.0):
            prism(bm, (x, -13.5, wing_z(x, -13.5, False) - 0.02), (x, -20.4, wing_z(x, -20.4, False) - 0.1), 0.22, 0.2, w1=0.14, h1=0.1)
    ship.part(fittings, bevel=0.0, mirror=True, angle=30)

    def centre(bm):
        bs.antenna(bm, (0, 4.6, sup_top(4.6) - 0.1), 1.5, r=0.06, tip=True, lean=(0, -0.45, 1))
        # refuelling port and a hatch on the spine
        y = -9.0
        cyl(bm, (0, y, sup_top(y) - 0.1), (0, y, sup_top(y) + 0.05), 0.5, 0.44, n=16)
        cyl(bm, (0, y, sup_top(y) + 0.05), (0, y, sup_top(y) + 0.07), 0.26, n=16, mat=G)
    ship.part(centre, bevel=0.0, angle=30)

    # ---------------------------------------------------------------- light lines
    def strips(bm):
        for sx in (1, -1):
            chain(bm, [(sx * fus(y)[1], y, fus(y)[4]) for y in frange(26.6, 9.8, 1.4)], 0.05, 0.1)   # the fore chine
            pts = [S_out(y, 0.0) for y in frange(2.4, -8.0, 1.3)]                                    # nacelle prows
            chain(bm, [(sx * p.x, p.y, p.z) for p in pts], 0.045, 0.09)
            xs = frange(6.6, 10.2, 0.9)                                                              # the wing's trailing edge
            chain(bm, [(sx * x, at(WSS, x)[3] - 0.01, at(WSS, x)[1]) for x in xs], 0.07, 0.07)
            rail = [can_ring(y)[1] for y in frange(18.6, 8.6, 1.25)]                                 # the canopy sill
            chain(bm, [(sx * p.x, p.y, p.z) for p in rail], 0.035, 0.05)
        chain(bm, [(0, y, sup_top(y)) for y in frange(-15.5, -20.5, 1.25)], 0.08, 0.05)
    ship.part(strips, bevel=0.0)

    cap._finish(ship)

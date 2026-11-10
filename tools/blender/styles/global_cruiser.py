"""Style `global`, cruiser: the little sister of styles/global.py's flagship, about 150 m.

One silhouette: a single broad spearhead hull that tapers into one great stern engine, short swept shoulders
that end in two engine pods tucked close to the hull, a low dorsal sail with a glazed bridge blister standing
clear at its front, a short keel fin. Same language as the flagship (chined lofted sections, white body, a
crown sweep of faction colour whose edges are mesh edges, one warm pinstripe, ring nozzles, thin light lines),
but a compact ship: the engines are a third of the beam and the windows are few and large.
60 units long (1 unit is 2.5 m).
"""
import importlib
import math

import bmesh
from mathutils import Vector

import build_ships as bs
from shiplib import H, L, G, GL, E, loft, box, prism, cyl, sphere

cap = importlib.import_module("styles.global")
crom, at, sec_pt, fsec, lathe_y, foil, foil_h, frange, rnd = (cap.crom, cap.at, cap.sec_pt, cap.fsec, cap.lathe_y,
                                                              cap.foil, cap.foil_h, cap.frange, cap.rnd)
V = Vector
D = math.radians

STYLE = dict(cap.STYLE, spacing=(2.0, 2.7, 1.6), seam_w=0.022)
TINT = dict(cap.TINT)

NX = 11.3                       # engine pod axis
NZ = 0.3
NY0, NY1, NR = -24.6, -27.6, 1.9   # pod nozzle: collar, exit, radius
CY0, CY1, CR, CZ = -26.2, -30.0, 2.45, 0.3  # stern nozzle
TY0, TY1, TD = -22.5, 23.0, 0.11    # gallery trench along the hull
VH0, VH1 = 3.0, 8.4            # ventral hangar bays
GY0, GY1 = -22.0, -15.6        # radiator grilles on the pods' outboard flanks

# ================================================================ sections

FS = [  # y, half width, height above / below the chine, chine z
    (29.8, 0.05, 0.04, 0.04, -0.4),
    (28.4, 0.6, 0.36, 0.34, -0.4),
    (25.2, 1.6, 0.85, 0.82, -0.36),
    (20.0, 2.95, 1.45, 1.45, -0.28),
    (14.0, 4.25, 2.05, 2.1, -0.18),
    (7.0, 5.35, 2.55, 2.7, -0.08),
    (0.0, 6.0, 2.85, 3.0, 0.0),
    (-7.0, 6.1, 2.95, 3.05, 0.08),
    (-13.0, 5.7, 2.9, 2.9, 0.15),
    (-18.0, 5.0, 2.8, 2.7, 0.22),
    (-22.5, 4.1, 2.7, 2.6, 0.28),
    (-26.2, 3.3, 2.62, 2.55, 0.3),
]
FM, FN, FNB, FNC = 84, 56, 8, 6
FSS = crom(FS, FM)
EX, EZT, EZB = cap.EX, cap.EZT, cap.EZB

WS = [  # shoulders: x, z, leading edge y, trailing edge y, thickness
    (2.8, 0.1, 13.0, -21.6, 1.5),
    (5.6, 0.14, 5.6, -21.9, 1.9),
    (8.4, 0.22, -2.6, -22.4, 2.0),
    (11.4, 0.3, -9.6, -23.0, 1.9),
]
WM = 24
WSS = crom(WS, WM)

OS = [  # engine pods: y, half width, height above / below, centre z
    (-1.4, 0.05, 0.05, 0.05, NZ - 0.05),
    (-2.6, 0.66, 0.72, 0.64, NZ - 0.05),
    (-5.5, 1.32, 1.48, 1.3, NZ - 0.03),
    (-10.0, 1.84, 2.08, 1.86, NZ),
    (-16.0, 2.05, 2.3, 2.06, NZ),
    (-21.0, 2.05, 2.28, 2.06, NZ),
    (-24.6, 1.98, 2.16, 2.0, NZ),
]
OM, ON, ONB, ONC = 36, 32, 4, 4
OSS = crom(OS, OM)
OKW = dict(ex=0.5, ezt=0.6, ezb=0.6)

SUP = [  # dorsal sail: y, half width, top above the hull crown
    (11.0, 0.15, 0.05), (8.5, 1.0, 0.5), (4.0, 1.75, 0.95), (0.0, 2.15, 1.35), (-3.0, 2.3, 2.3), (-6.0, 2.3, 2.7),
    (-10.0, 2.1, 2.5), (-14.0, 1.8, 1.5), (-19.0, 1.25, 0.8), (-23.5, 0.15, 0.05),
]
SUPS = crom(SUP, 48)
SKW = dict(ex=0.5, ezt=0.62)

VS = [(17.5, 0.1, 0.05), (15.0, 0.9, 0.55), (9.5, 1.55, 1.0), (3.0, 1.75, 1.15), (-7.0, 1.65, 1.1), (-13.0, 1.2, 0.75),
      (-17.5, 0.55, 0.35), (-20.0, 0.1, 0.05)]  # ventral gallery: y, half width, depth below the hull
VM, VN = 30, 32
VSS = crom(VS, VM)
VKW = dict(ex=0.5, ezt=0.6, ezb=0.6)

KS = [(0.0, -2.0, 1.0, -13.4, 1.4), (0.0, -3.6, -2.4, -14.0, 1.1), (0.0, -5.2, -6.0, -14.8, 0.72), (0.0, -6.5, -9.2, -15.6, 0.3)]
KSS = crom(KS, 16)
DS = [(0.0, 3.3, -10.6, -20.4, 1.0), (0.0, 4.9, -14.2, -21.2, 0.8), (0.0, 6.5, -17.8, -22.1, 0.5), (0.0, 7.7, -20.5, -22.9, 0.2)]


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
    pts = [(-26.4, 0.06), (-21.0, 0.135), (-12.0, 0.19), (1.0, 0.175), (9.0, 0.108), (21.0, 0.082), (27.6, 0.0)]
    return at(pts, y)[1]


def hull_ab(y):
    return math.pi / 2 - max(liv_lim(y), 0.012) * 2 * math.pi


def S_hull(y, a):
    return sec_pt(fus(y), a)


def S_hull_f(y, f):
    """Hull point at fraction f of the way from the chine to the paint edge: follows the mesh rows."""
    return sec_pt(fus(y), f * hull_ab(y))


def sup_st(s):
    y, hw, top = s
    return (y, max(hw, 0.05), max(top, 0.02) + 0.9, 0.5, fus_top(y) - 0.9)


def sup_top(y):
    return fus_top(y) + at(SUPS, y)[2]


def S_sup(y, a):
    return sec_pt(sup_st(at(SUPS, y)), a, **SKW)


def out_lim(y):
    pts = [(-26.0, 0.012), (-15.0, 0.012), (-8.0, 0.235), (0.0, 0.235)]
    return at(pts, y)[1]


def S_out(y, a):
    return sec_pt(at(OSS, y), a, **OKW) + V((NX, 0, 0))


def S_wing(x, u):
    _, z, le, te, t = at(WSS, x)
    return V((x, le - (le - te) * u, z + foil_h(u, t)))


def wing_z(x, y, top=True):
    _, z, le, te, t = at(WSS, x)
    u = min(max((le - y) / (le - te), 0.0), 1.0)
    return z + foil_h(u, t) * (1.0 if top else -0.8)


def v_st(q):
    y, hw, d = q
    return (y, max(hw, 0.05), 0.4, max(d, 0.02) + 0.7, fus_bot(y) + 0.7)


def S_v(y, a):
    return sec_pt(v_st(at(VSS, y)), a, **VKW)


# ================================================================ material: the flagship's, with this hull's regions

DARK_REGIONS = [
    ((NX - 2.1, NX + 2.1), (None, NY0 - 0.34), (None, None)),                 # pod nozzles
    ((None, 3.2), (None, CY0 - 0.4), (CZ - 3.0, CZ + 3.0)),                   # stern nozzle
    ((NX + 1.3, None), (GY0, GY1), (NZ - 0.5, NZ + 0.5)),                       # radiator grilles on the pods
    ((0.3, 1.42), (VH0 + 0.14, VH1 - 0.14), (S_v(5.5, -D(33.75)).z, S_v(5.5, -D(11.25)).z)),  # hangar interiors
]
PIN_REGIONS = [
    ((6.55, 6.73), (None, None), (None, None)),                                # shoulder
    ((None, 2.4), (25.0, 25.2), (None, None)),                                # nose ring
    ((NX - 1.75, None), (-5.2, -5.02), (None, None)),                         # pod prows
    ((None, 0.9), (None, None), (-6.02, -5.88)),                              # keel
    ((None, 0.9), (None, -12.0), (6.8, 6.93)),                                # dorsal fin
]
LINE_REGIONS = [((None, None), (y - 0.028, y + 0.028), (None, None)) for y in (19.0, 10.0, 1.0, -8.0, -17.0)]
LINE_REGIONS += [((None, 2.6), (-15.0, 1.5), (z - 0.022, z + 0.022)) for z in (4.05, 4.75)]  # deck lines on the sail


def materials():
    saved = cap.DARK_REGIONS, cap.PIN_REGIONS, cap.LINE_REGIONS
    cap.DARK_REGIONS, cap.PIN_REGIONS, cap.LINE_REGIONS = DARK_REGIONS, PIN_REGIONS, LINE_REGIONS
    try:
        return cap.materials()
    finally:
        cap.DARK_REGIONS, cap.PIN_REGIONS, cap.LINE_REGIONS = saved


# ================================================================ helpers

def windows(bm, S, ys, a, **kw):
    """The flagship's windows at this ship's scale: a deck is 1.2 units, so they are few and large."""
    kw.setdefault('hl', 0.13)
    kw.setdefault('hh', 0.06)
    kw.setdefault('lift', 0.016)
    cap.windows(bm, S, ys, a, **kw)


def nozzle(bm, cx, cz, y0, y1, r, n, npet, w):
    npf, nm = cap.nozzle_prof(y0, y1, r, glow=0.085)
    lathe_y(bm, cx, cz, [(y0 + 0.8, 0.0), (y0 + 0.8, r * 0.96)] + npf, n=n, mats=[H, H] + nm)
    ln = y0 - y1
    for a in range(npet):
        ang = (a + 0.5) * 2 * math.pi / npet
        d = V((math.cos(ang), 0, math.sin(ang)))
        c = V((cx, 0, cz))
        prism(bm, c + d * (r * 1.04) + V((0, y0 - ln * 0.1, 0)), c + d * (r * 0.93) + V((0, y1 + ln * 0.04, 0)),
              w, 0.05, w1=w * 0.86, h1=0.04, up=d, chamfer=0.15)


# ================================================================ the ship

def build(ship):
    # ---------------------------------------------------------------- main hull
    def hull(bm):
        rings = [V((0, 30.0, -0.4))] + [fsec(st, FN, fb=max(liv_lim(st[0]), 0.012), nb=FNB, nc=FNC) for st in FSS]

        def mf(i, k):
            if i == 0:
                return H
            y = (FSS[i - 1][0] + FSS[i][0]) / 2
            return L if FNB <= k < FNB + 2 * FNC and liv_lim(y) > 0.012 else H
        res = loft(bm, rings, matfn=mf)
        bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
        # the gallery: a shallow trench above the chine, where the two window rows sit
        up = 2 * (FNB + FNC) - 1
        for ks in ((2, 3), (up - 2, up - 3)):
            fs = [res['bands'][i][k] for i in range(1, FM + 1) for k in ks if TY0 < (FSS[i - 1][0] + FSS[i][0]) / 2 < TY1]
            bmesh.ops.inset_region(bm, faces=fs, thickness=0.05, depth=-TD, use_even_offset=True, use_boundary=True)
    ship.part(hull, bevel=0.03, segs=2, angle=38)

    # ---------------------------------------------------------------- dorsal sail
    def sail(bm):
        rings = [V((0, 11.2, fus_top(11.2) - 0.1))] + [fsec(sup_st(s), 32, **SKW) for s in SUPS] + [V((0, -23.7, fus_top(-23.7) - 0.1))]
        loft(bm, rings)
    ship.part(sail, bevel=0.03, segs=2, angle=38)

    # ---------------------------------------------------------------- bridge blister
    BZ = sup_top(-5.5) + 0.12
    BR = crom([(-1.9, 0.05, 0.04, 0.04, BZ - 0.3), (-2.5, 0.7, 0.36, 0.34, BZ - 0.2), (-3.9, 1.2, 0.56, 0.5, BZ - 0.04),
               (-5.5, 1.36, 0.64, 0.56, BZ), (-7.2, 1.24, 0.6, 0.5, BZ), (-8.7, 0.8, 0.42, 0.36, BZ - 0.06),
               (-9.8, 0.05, 0.04, 0.04, BZ - 0.14)], 26)

    def bridge(bm):
        rings = [V((0, -1.8, BZ - 0.3))] + [fsec(st, 28, ex=0.7) for st in BR] + [V((0, -9.9, BZ - 0.14))]
        loft(bm, rings, matfn=lambda i, k: GL if 1 <= i <= 15 and k in ((1, 2, 11, 12) if i <= 7 else (1, 12)) else H)
    ship.part(bridge, bevel=0.012, segs=1, angle=38)

    def trim(bm):
        for sx in (1, -1):
            for a, b in zip(BR[2:-2:2], BR[4:-2:2]):  # the observation band along the blister's edge
                bs.strip(bm, (sx * a[1], a[0], a[4]), (sx * b[1], b[0], b[4]), 0.03, 0.04)
            ys = frange(-2.4, -11.0, 1.1)  # the visor: a line of light wrapping the front of the sail
            for a, b in zip(ys, ys[1:]):
                pa, pb = S_sup(a, D(58)), S_sup(b, D(58))
                bs.strip(bm, (sx * pa.x, pa.y, pa.z), (sx * pb.x, pb.y, pb.z), 0.03, 0.045)
        angs = frange(58, 122, 8)
        for a, b in zip(angs, angs[1:]):
            bs.strip(bm, S_sup(-2.4, D(a)), S_sup(-2.4, D(b)), 0.045, 0.03)
        # sensor dome, mast, dish
        for sx in (1, -1):
            sphere(bm, (sx * 1.05, -12.6, sup_top(-12.6) + 0.05), 0.32, seg=12, rings=6, scale=(1, 1, 0.8))
        cyl(bm, (0, -8.3, BZ + 0.3), (0, -8.3, BZ + 0.75), 0.3, 0.2, n=12)
        cyl(bm, (0, -8.3, BZ + 0.75), (0, -8.5, BZ + 1.9), 0.07, 0.035, n=8)
        for z, w in ((1.05, 0.6), (1.45, 0.38)):
            prism(bm, (-w, -8.32 - z * 0.17, BZ + z), (w, -8.32 - z * 0.17, BZ + z), 0.03, 0.03)
        bs.light(bm, (0, -8.5, BZ + 1.93), 0.07)
        yd, zd = -11.0, sup_top(-11.0)
        lathe_y(bm, 0, zd + 0.8, [(yd - 0.12, 0.0), (yd - 0.2, 0.22), (yd + 0.05, 0.56), (yd + 0.13, 0.6), (yd + 0.1, 0.5),
                                  (yd - 0.1, 0.2), (yd - 0.13, 0.0)], n=20)
        cyl(bm, (0, yd - 0.2, zd - 0.1), (0, yd - 0.14, zd + 0.8), 0.09, 0.07, n=8)
    ship.part(trim, bevel=0.0, angle=38)

    # ---------------------------------------------------------------- shoulders
    def wings(bm):
        foil(bm, WS, WM, paint=lambda s, u, upper: L if 6.95 < s[0] < 8.9 else H,
             groove=lambda st: 0.08 * min(max((st[0] - 6.3) / 0.4, 0.0), 1.0))
    ship.part(wings, bevel=0.025, segs=1, mirror=True, angle=40)

    # ---------------------------------------------------------------- engine pods
    def pod(bm):
        rings = [V((0, -1.3, NZ - 0.05))] + [fsec(st, ON, fb=out_lim(st[0]), nb=ONB, nc=ONC, **OKW) for st in OSS]
        for r in rings[1:]:
            for p in r:
                p.x += NX
        rings[0].x += NX
        loft(bm, rings, matfn=lambda i, k: L if ONB <= k < ONB + 2 * ONC and i > 0 and out_lim(OSS[i - 1][0]) > 0.02 else H)
    ship.part(pod, bevel=0.022, segs=1, mirror=True, angle=34)

    def podtrim(bm):
        nozzle(bm, NX, NZ + 0.05, NY0, NY1, NR, 28, 14, 0.4)
        # intake scoop on the pod's back
        sp = crom([(-9.0, 0.08, 0.05), (-10.6, 0.5, 0.42), (-14.0, 0.7, 0.62), (-19.0, 0.66, 0.56), (-22.6, 0.4, 0.3), (-24.0, 0.08, 0.05)], 14)

        def st(q):
            y, hw, top = q
            o = at(OSS, y)
            return (y, max(hw, 0.05), max(top, 0.02) + 0.6, 0.3, o[4] + o[2] - 0.6)
        rr = [V((NX, -8.9, st(sp[0])[4] + 0.55))] + [[q + V((NX, 0, 0)) for q in fsec(st(q), 16, **SKW)] for q in sp]
        loft(bm, rr + [V((NX, -24.1, st(sp[-1])[4] + 0.55))])
        ys = frange(GY1 - 0.2, GY0 + 0.2, 1.0)  # radiator slats
        for a in (-1.5, 0.0, 1.5):
            for ya, yb in zip(ys, ys[1:]):
                prism(bm, S_out(ya, D(a)), S_out(yb, D(a)), 0.05, 0.06, up=(1, 0, 0), chamfer=0.2)
        # ventral stabiliser
        foil(bm, [(NX + 0.5, -1.2, -14.6, -23.0, 0.7), (NX + 1.05, -2.5, -17.8, -23.9, 0.5), (NX + 1.6, -3.8, -21.0, -24.9, 0.22)],
             8, sym=True, paint=lambda s, u, upper: L if s[1] < -2.3 else H)
    ship.part(podtrim, bevel=0.0, mirror=True, angle=34)
    ship.empty("Engine_1", (NX, NY1, NZ + 0.05), NR * 0.87)
    ship.empty("Engine_2", (-NX, NY1, NZ + 0.05), NR * 0.87)

    # ---------------------------------------------------------------- stern engine
    def stern(bm):
        nozzle(bm, 0, CZ, CY0, CY1, CR, 36, 18, 0.5)
    ship.part(stern, bevel=0.0, angle=30)
    ship.empty("Engine_0", (0, CY1, CZ), CR * 0.87)

    # ---------------------------------------------------------------- keel fin
    def keel(bm):
        foil(bm, KS, 16, paint=lambda s, u, upper: L if -5.9 < s[1] < -4.4 else H, sym=True)
        bs.light(bm, (0, -15.55, -6.5), 0.08)
    ship.part(keel, bevel=0.016, segs=1, angle=40)

    # ---------------------------------------------------------------- dorsal fin: the cruiser's own mark
    def fin(bm):
        foil(bm, DS, 14, paint=lambda s, u, upper: L if 5.2 < s[1] < 6.8 else H, sym=True)
        bs.light(bm, (0, -22.85, 7.67), 0.08)
    ship.part(fin, bevel=0.016, segs=1, angle=40)

    # ---------------------------------------------------------------- ventral gallery and its hangar bays
    def ventral(bm):
        rings = [V((0, 17.7, fus_bot(17.7) + 0.1))] + [fsec(v_st(q), VN, **VKW) for q in VSS] + [V((0, -20.2, fus_bot(-20.2) + 0.1))]
        res = loft(bm, rings)
        bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
        for ks in ((VN - 2, VN - 3), (VN // 2 + 1, VN // 2 + 2)):
            fs = [res['bands'][i][k] for i in range(1, VM + 1) for k in ks if VH0 < (VSS[i - 1][0] + VSS[i][0]) / 2 < VH1]
            bmesh.ops.inset_region(bm, faces=fs, thickness=0.08, depth=-0.85, use_even_offset=True, use_boundary=True)
        zt, zb = S_v(5.5, -D(11.25)).z, S_v(5.5, -D(33.75)).z
        for sx in (1, -1):
            bs.strip(bm, (sx * 1.02, VH0 + 0.4, zt - 0.1), (sx * 1.02, VH1 - 0.4, zt - 0.1), 0.2, 0.02)
            for i in range(3):
                y = VH0 + 1.0 + i * 1.7
                box(bm, (sx * (1.0 + 0.1 * (i % 2)), y, zb + 0.14), (0.36, 0.8, 0.14), taper=(0.5, 1.0))
                bs.light(bm, (sx * 1.36, y + 0.85, zb + 0.1), 0.04)
    ship.part(ventral, bevel=0.022, segs=1, angle=34)

    # ---------------------------------------------------------------- batteries
    muzzles = []

    def guns(bm):
        for x, y, up, s in ((1.9, 17.5, 1, 1.0), (4.2, 4.6, 1, 1.25), (2.9, 12.5, -1, 1.1)):
            z = fus_top(y, x) if up > 0 else fus_bot(y, x)
            muzzles.append(cap.turret(bm, (x, y, z - up * 0.05), up, s))
        # point defence: small domes
        for x, y, up in ((1.0, 23.2, 1), (4.4, -16.0, 1), (8.2, -15.0, 1), (4.0, -12.0, -1)):
            z = (wing_z(x, y, up > 0) if x > 6.2 else (fus_top(y, x) if up > 0 else fus_bot(y, x)))
            p = V((x, y, z))
            sphere(bm, p, 0.22, seg=10, rings=6, scale=(1, 1, 0.8))
            cyl(bm, p + V((0, 0.1, up * 0.1)), p + V((0, 0.6, up * 0.13)), 0.04, n=6)
    ship.part(guns, bevel=0.0, mirror=True, angle=30)
    n = 0
    for m in muzzles:
        for sx in (1, -1):
            ship.empty("Gun_%d" % n, (sx * m.x, m.y, m.z), 0.2)
            n += 1

    # ---------------------------------------------------------------- deck fittings
    def fittings(bm):
        for y in frange(12.8, 14.9, 0.7):  # missile hatches on the fore deck
            for x in (0.32, 0.86):
                box(bm, (x, y, fus_top(y, x) + 0.005), (0.34, 0.44, 0.05), mat=L)
        for x in frange(7.3, 8.5, 0.6):  # hatches on the shoulder
            _, z, le, te, t = at(WSS, x)
            for u in (0.4, 0.52):
                y = le - (le - te) * u
                box(bm, (x, y, wing_z(x, y)), (0.42, 0.7, 0.07), mat=L)
        bs.vent(bm, (7.9, -18.2, wing_z(7.9, -18.2) - 0.02), 0.6, 2.4, n=6, h=0.07)
        for i, (x, y, h) in enumerate(((0.7, -14.2, 1.3), (1.05, -15.6, 0.8))):
            bs.antenna(bm, (x, y, sup_top(y) - 0.3), h, r=0.03, tip=(i == 0), lean=(0, -0.1, 1))
        pc = S_hull(9.0, D(-11))  # docking collar on the flank
        cyl(bm, pc - V((0.25, 0, 0.06)), pc + V((0.12, 0, 0.03)), 0.42, n=16)
        cyl(bm, pc + V((0.12, 0, 0.03)), pc + V((0.15, 0, 0.04)), 0.28, n=16, mat=G)
        sphere(bm, (0.7, 22.0, fus_bot(22.0, 0.7)), 0.32, seg=12, rings=6, scale=(1, 1.9, 0.6))  # sensor blisters
        for y in (17.6, 18.8, 20.0):  # torpedo tubes under the fore chine
            pc = S_hull(y, D(-16))
            d = V((0.5, 0.86, -0.1)).normalized()
            cyl(bm, pc - d * 0.7, pc + d * 0.34, 0.27, 0.24, n=12)
            cyl(bm, pc + d * 0.34, pc + d * 0.345, 0.17, n=12, mat=GL)
    ship.part(fittings, bevel=0.0, mirror=True, angle=30)

    # ---------------------------------------------------------------- windows: few and large
    def lights(bm):
        for r, (f, y1) in enumerate(((2.5 / FNB, TY1 - 0.5), (3.5 / FNB, 18.0))):
            windows(bm, S_hull_f, frange(TY0 + 0.5, y1, 0.5), f, row=r, lift=0.016 - TD, fill=0.7)
        vh = lambda y: VH0 - 0.5 < y < VH1 + 0.5
        windows(bm, S_hull, frange(-21.0, 21.0, 0.5), D(-10), row=10, fill=0.55)
        for r, (a, y0, y1) in enumerate(((20, -17.0, 4.0),)):
            windows(bm, S_sup, frange(y0, y1, 0.5), D(a), row=20 + r, fill=0.75)
        for r, a in enumerate((34, 146)):
            windows(bm, S_out, frange(-23.0, -7.5, 0.5), D(a), row=30 + r, fill=0.6, out=(1, 0, 0) if r == 0 else (-1, 0, 0))
        windows(bm, S_v, frange(-15.0, 13.5, 0.5), D(-20), row=70, fill=0.66, skip=vh)
        windows(bm, S_wing, frange(6.6, 9.2, 0.42), 0.035, row=40, fill=0.7, out=(0, 0.5, 1), hl=0.09)
    ship.part(lights, bevel=0.0, mirror=True, recalc=False, weighted=False)

    # ---------------------------------------------------------------- light lines
    def strips(bm):
        for sx in (1, -1):
            ys = frange(26.8, 7.3, 1.3)  # the fore chine
            for a, b in zip(ys, ys[1:]):
                fa, fb = fus(a), fus(b)
                bs.strip(bm, (sx * fa[1], a, fa[4]), (sx * fb[1], b, fb[4]), 0.03, 0.055)
            ys = frange(-2.6, -12.0, 1.2)  # pod prows
            for a, b in zip(ys, ys[1:]):
                pa, pb = S_out(a, 0.0), S_out(b, 0.0)
                bs.strip(bm, (sx * pa.x, a, pa.z), (sx * pb.x, b, pb.z), 0.026, 0.05)
            xs = frange(5.6, 9.4, 0.95)  # the shoulder's trailing edge
            for a, b in zip(xs, xs[1:]):
                wa, wb = at(WSS, a), at(WSS, b)
                bs.strip(bm, (sx * a, wa[3] - 0.008, wa[1]), (sx * b, wb[3] - 0.008, wb[1]), 0.036, 0.04)
    ship.part(strips, bevel=0.0)

    cap._finish(ship)

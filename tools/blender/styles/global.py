"""Style candidate `global`, capital ship: the stylised-realistic look that sells everywhere (Normandy and the
Alliance cruisers, Enterprise's graceful hulls, Mon Calamari flow, Infinite Lagrange paint).

One silhouette: a long chined spearhead hull that swells aft, a swept delta wing carrying two great engine
nacelles with glowing caps and ring nozzles, a flowing dorsal sail with a cantilevered bridge deck, a deep keel
fin. Smooth lofted surfaces; white body, large clean faction-colour sweeps whose edges are mesh edges, one thin
warm pinstripe. It reads as huge through rows of tiny windows that follow the hull's flow lines, hangar bays
with lit interiors, small turrets and sensor masts. 60 units long (1 unit is about 7 m).
"""
import math

import bmesh
import numpy as np
from mathutils import Vector

import build_ships as bs
import shiplib
from shiplib import H, L, G, GL, E, loft, box, prism, cyl, sphere, inset_faces, log

V = Vector

STYLE = dict(paint=(0.44, 0.455, 0.47), paint2=(0.35, 0.365, 0.39), accent=(0.47, 0.48, 0.49), accent_thresh=0.9,
             metal=0.12, rough=0.3, spacing=(1.1, 1.5, 0.9), wear=0.14, grime=0.32, sub_amt=0.3, seam_w=0.015,
             grime_col=(0.09, 0.095, 0.11), bump=0.5, bare=(0.6, 0.6, 0.62))

TINT = dict(livery=(0.008, 0.06, 0.33), glow=(0.45, 0.86, 1.0), engine=(0.42, 0.74, 1.0),
            livery_metal=0.15, livery_rough=0.22, enemy_livery=(0.5, 0.09, 0.015))

PIN = (0.85, 0.26, 0.02)       # the thin accent stripe
BELLY = (0.52, 0.55, 0.62)     # multiplied onto the underside
DARK = (0.1, 0.105, 0.125)     # multiplied onto nozzles and barrels

OX = 11.6                      # outrigger hull axis
SX_, CZ_ = 2.38, 0.3           # stern engines
HY0, HY1 = -19.6, -12.4        # hangar bays on the outriggers
TY0, TY1, TD = -22.6, 24.6, 0.07  # the gallery trench along the hull
LINE = (0.5, 0.5, 0.52)        # section joints

_MAP = None                    # Mapping node: object coords -> build coords (set at the end of build)

DARK_REGIONS = [
    ((OX - 1.5, OX + 1.5), (None, -27.84), (None, None)),            # outrigger nozzles
    ((None, 3.6), (None, -24.94), (CZ_ - 2.0, CZ_ + 2.0)),           # stern nozzles
    ((OX + 0.3, OX + 1.5), (HY0 + 0.1, HY1 - 0.1), (-0.35, 1.2)),    # hangar interiors
]
PIN_REGIONS = [
    ((5.12, 5.26), (None, None), (None, None)),                      # wing
    ((None, 2.0), (26.2, 26.36), (None, None)),                      # nose ring
    ((OX - 2.0, None), (1.5, 1.64), (None, None)),                   # outrigger prows
    ((None, 0.8), (None, None), (-5.52, -5.4)),                      # keel
]
LINE_REGIONS = [((None, None), (y - 0.018, y + 0.018), (None, None)) for y in (21.0, 14.5, 8.0, 1.5, -5.0, -11.5, -18.0, -23.0)]
LINE_REGIONS += [((None, 2.3), (-17.5, 3.0), (z - 0.014, z + 0.014)) for z in (3.35, 3.85, 4.35, 4.85)]  # deck lines on the sail


# ================================================================ material

def materials():
    """shiplib's hull, plus paint that follows the design: a grey belly, gunmetal nozzles, the pinstripe."""
    global _MAP
    mats, socks = shiplib.make_materials(STYLE)
    nb = shiplib.NB(mats['Hull'])
    tc = nb.node('ShaderNodeTexCoord')
    _MAP = nb.node('ShaderNodeMapping')
    nb.link(tc.outputs['Object'], _MAP.inputs['Vector'])
    sp = nb.node('ShaderNodeSeparateXYZ')
    nb.link(_MAP.outputs[0], sp.inputs[0])
    axes = (nb.math('ABSOLUTE', sp.outputs[0]), sp.outputs[1], sp.outputs[2])
    geo = nb.node('ShaderNodeNewGeometry')
    sn = nb.node('ShaderNodeSeparateXYZ')
    nb.link(geo.outputs['Normal'], sn.inputs[0])

    def region(bounds):
        """bounds: ((xlo, xhi), (ylo, yhi), (zlo, zhi)) on |x|, y, z; None = open."""
        m = None
        for v, (lo, hi) in zip(axes, bounds):
            for lim, op in ((lo, 'GREATER_THAN'), (hi, 'LESS_THAN')):
                if lim is None:
                    continue
                s = nb.math(op, v, lim)
                m = s if m is None else nb.math('MULTIPLY', m, s)
        return m

    def any_of(regs):
        m = None
        for r in regs:
            s = region(r)
            m = s if m is None else nb.math('MAXIMUM', m, s)
        return m

    def mul(fac, col, tint):
        o = nb.mix(fac, col, (*tint, 1))
        o.node.blend_type = 'MULTIPLY'
        return o

    base, rough, metal = socks['base'], socks['rough'], socks['metal']
    belly = nb.mrange(sn.outputs[2], -0.22, -0.5, 0.0, 1.0, smooth=True)
    base = mul(belly, base, BELLY)
    dark = any_of(DARK_REGIONS)
    base = mul(dark, base, DARK)
    metal = nb.mixf(dark, metal, 0.9)
    rough = nb.mixf(dark, rough, 0.36)
    base = mul(any_of(LINE_REGIONS), base, LINE)
    pin = any_of(PIN_REGIONS)
    base = nb.mix(pin, base, (*PIN, 1))
    metal = nb.mixf(pin, metal, 0.1)
    rough = nb.mixf(pin, rough, 0.25)
    bsdf = socks['bsdf']
    nb.link(base, bsdf.inputs['Base Color'])
    nb.link(rough, bsdf.inputs['Roughness'])
    nb.link(metal, bsdf.inputs['Metallic'])
    socks.update(base=base, rough=rough, metal=metal)
    return mats, socks


# ================================================================ helpers

def crom(st, m):
    """Catmull-Rom through tuples `st`, m+1 samples uniform in station index."""
    n = len(st)
    out = []
    for j in range(m + 1):
        t = j / m * (n - 1)
        i = min(int(t), n - 2)
        f = t - i
        p0, p1, p2, p3 = st[max(i - 1, 0)], st[i], st[i + 1], st[min(i + 2, n - 1)]
        out.append(tuple(0.5 * (2 * b + (c - a) * f + (2 * a - 5 * b + 4 * c - d) * f * f + (3 * b - a - 3 * c + d) * f ** 3)
                         for a, b, c, d in zip(p0, p1, p2, p3)))
    return out


def at(samples, v, key=0):
    """Linear lookup in samples by component `key`."""
    s = sorted(samples, key=lambda q: q[key])
    if v <= s[0][key]:
        return s[0]
    for a, b in zip(s, s[1:]):
        if a[key] <= v <= b[key]:
            t = (v - a[key]) / (b[key] - a[key]) if b[key] != a[key] else 0.0
            return tuple(p + (q - p) * t for p, q in zip(a, b))
    return s[-1]


def sgn(v):
    return 1.0 if v >= 0 else -1.0


def rnd(i, j=0):
    """Deterministic 0..1 hash."""
    v = math.sin(i * 12.9898 + j * 78.233) * 43758.5453
    return v - math.floor(v)


EX, EZT, EZB = 0.9, 1.3, 1.4


def sec_pt(st, a, ex=EX, ezt=EZT, ezb=EZB):
    """Point of a chined section st = (y, hw, ht, hb, cz) at angle a (0 = +x edge, 90 deg = crown)."""
    y, hw, ht, hb, cz = st
    c, s = math.cos(a), math.sin(a)
    return V((hw * sgn(c) * abs(c) ** ex, y, cz + (ht * abs(s) ** ezt if s >= 0 else -hb * abs(s) ** ezb)))


def fsec(st, n, fb=None, nb=0, nc=0, **kw):
    """Chined section ring: round crown, a crisp edge at the half-breadth.
    fb: girth fraction from the crown where the paint field ends; a vertex row (index nb in the upper quadrant)
    follows it, so the paint edge is a mesh edge."""
    if fb is None:
        angs = [2 * math.pi * i / n for i in range(n)]
    else:
        ab = math.pi / 2 - fb * 2 * math.pi
        q = [ab * j / nb for j in range(nb + 1)] + [ab + (math.pi / 2 - ab) * j / nc for j in range(1, nc + 1)]
        angs = q + [math.pi - a for a in reversed(q[:-1])] + [math.pi + math.pi * j / (n // 2) for j in range(1, n // 2)]
    return [sec_pt(st, a, **kw) for a in angs]


def lathe_y(bm, cx, cz, prof, n=32, mats=None, mat=H):
    """prof: (y, r) around the axis through (cx, *, cz); r == 0 -> point."""
    rings = []
    ph = math.pi / n
    for y, r in prof:
        if r <= 1e-6:
            rings.append(V((cx, y, cz)))
        else:
            rings.append([V((cx + r * math.cos(ph + 2 * math.pi * i / n), y, cz + r * math.sin(ph + 2 * math.pi * i / n)))
                          for i in range(n)])
    return loft(bm, rings, mat=mat, matfn=(lambda i, k: mats[i]) if mats else None)


AU0 = [0.012, 0.045, 0.1, 0.18, 0.28, 0.4, 0.53, 0.66, 0.79, 0.91]


def foil_h(u, t):
    return t * 0.5 * 2.6 * math.sqrt(u) * (1 - u) ** 0.92


def foil(bm, stations, m, paint=None, sym=False, mat=H, groove=None):
    """Smooth wing through stations (x, z, le, te, t). paint(station, u, upper) -> material.
    groove(station) -> depth of a trench along the 72 % chord line."""
    ss = crom(stations, m)
    rings = []
    AU = AU0 if groove is None else AU0[:8] + [0.715, 0.72, 0.737, 0.742] + AU0[8:]
    gi = (9, 10)
    for i, (x, z, le, te, t) in enumerate(ss):
        a, b = ss[max(i - 1, 0)], ss[min(i + 1, m)]
        d = V((b[0] - a[0], b[1] - a[1])).normalized()
        nx, nz = -d.y, d.x
        ch = le - te
        gd = groove(ss[i]) if groove else 0.0
        lo = 1.0 if sym else 0.8
        pts = [(le, 0.0)]
        pts += [(le - ch * u, foil_h(u, t) - (gd if groove and j in gi else 0.0)) for j, u in enumerate(AU)]
        pts += [(te, 0.0)]
        pts += [(le - ch * u, -(foil_h(u, t) * lo - (gd if groove and j in gi else 0.0))) for j, u in reversed(list(enumerate(AU)))]
        rings.append([V((x + nx * h, y, z + nz * h)) for y, h in pts])
    nu = len(AU)

    def mf(i, k):
        if paint is None:
            return mat
        upper = k <= nu
        j = k if upper else 2 * nu + 1 - k
        u = 0.5 * ((AU[j - 1] if j > 0 else 0.0) + (AU[j] if j < nu else 1.0))
        mid = tuple((p + q) / 2 for p, q in zip(ss[i], ss[i + 1]))
        return paint(mid, u, upper)
    return loft(bm, rings, matfn=mf), ss


def windows(bm, S, ys, a, fill=0.72, row=0, hl=0.04, hh=0.022, lift=0.012, skip=None, out=(1, 0, 0)):
    """A row of tiny lit windows on the surface S(y, a) -> point, following its flow line at parameter a."""
    for i, y in enumerate(ys):
        if rnd(i // 3, row) > fill + 0.2 or rnd(i, row + 31) > fill or (skip and skip(y)):
            continue
        p = S(y, a)
        t = (S(y + 0.05, a) - S(y - 0.05, a)).normalized()
        s = S(y, a + 0.02) - S(y, a - 0.02)
        n = t.cross(s).normalized()
        if n.dot(V(out)) < 0:
            n = -n
        b = n.cross(t)
        p = p + n * lift
        bm.faces.new([bm.verts.new(p - t * hl - b * hh), bm.verts.new(p + t * hl - b * hh),
                      bm.verts.new(p + t * hl + b * hh), bm.verts.new(p - t * hl + b * hh)]).material_index = G


def frange(a, b, step):
    n = int(abs(b - a) / step)
    return [a + (b - a) * i / n for i in range(n + 1)]


def nozzle_prof(y0, y1, r, glow=0.05):
    """Ring nozzle from the collar at y0 to the exit plane at y1 (< y0): light band, bell, glowing rim, dark throat,
    burning back wall, plug. Returns (profile, materials)."""
    ln = y0 - y1
    prof = [(y0, r * 1.0), (y0, r * 0.9), (y0 - glow * 2.2, r * 0.9), (y0 - glow * 2.2, r * 1.02), (y1 + ln * 0.42, r * 1.01),
            (y1 + 0.03 * ln, r * 0.9), (y1, r * 0.87), (y1 + 0.05 * ln, r * 0.75), (y1 + ln * 0.16, r * 0.71),
            (y1 + ln * 0.56, r * 0.62), (y1 + ln * 0.56, r * 0.3), (y1 + ln * 0.24, r * 0.18), (y1 + ln * 0.1, 0.0)]
    mats = [H, G, H, H, H, H, G, H, E, E, H, H]
    return prof, mats


def petals(bm, cx, cz, y0, y1, r, n, w):
    for a in range(n):
        ang = (a + 0.5) * 2 * math.pi / n
        d = V((math.cos(ang), 0, math.sin(ang)))
        c = V((cx, 0, cz))
        ln = y0 - y1
        prism(bm, c + d * (r * 1.035) + V((0, y0 - ln * 0.1, 0)), c + d * (r * 0.925) + V((0, y1 + ln * 0.04, 0)),
              w, 0.03, w1=w * 0.86, h1=0.024, up=d, chamfer=0.15)


def turret(bm, p, up=1.0, s=1.0):
    """Twin-barrel battery on a +Z (up=1) or -Z facing surface; returns the muzzle point."""
    p = V(p)
    u = V((0, 0, up))
    cyl(bm, p - u * 0.1, p + u * 0.1 * s, 0.5 * s, 0.46 * s, n=20)
    lathe_y(bm, p.x, p.z + up * 0.26 * s, [(p.y - 0.62 * s, 0.0), (p.y - 0.55 * s, 0.14 * s), (p.y - 0.3 * s, 0.24 * s),
                                           (p.y + 0.3 * s, 0.25 * s), (p.y + 0.5 * s, 0.17 * s), (p.y + 0.56 * s, 0.0)], n=12)
    for dx in (-0.13, 0.13):
        b = p + V((dx * s, 0.45 * s, up * 0.27 * s))
        cyl(bm, b, b + V((0, 0.95 * s, 0)), 0.05 * s, n=8)
        cyl(bm, b + V((0, 0.05 * s, 0)), b + V((0, 0.4 * s, 0)), 0.075 * s, n=8)
        cyl(bm, b + V((0, 0.85 * s, 0)), b + V((0, 0.97 * s, 0)), 0.07 * s, n=8)
    return p + V((0, 1.42 * s, up * 0.27 * s))


# ================================================================ the ship

FS = [  # y, half width, height above / below the chine, chine z
    (29.8, 0.05, 0.04, 0.04, -0.3),
    (28.6, 0.46, 0.3, 0.28, -0.3),
    (26.0, 1.1, 0.7, 0.66, -0.28),
    (22.0, 1.8, 1.15, 1.12, -0.22),
    (17.0, 2.5, 1.6, 1.62, -0.15),
    (11.0, 3.2, 2.0, 2.1, -0.05),
    (5.0, 3.9, 2.3, 2.5, 0.0),
    (-1.0, 4.4, 2.5, 2.72, 0.05),
    (-7.0, 4.6, 2.56, 2.74, 0.1),
    (-13.0, 4.4, 2.48, 2.56, 0.15),
    (-18.0, 3.8, 2.3, 2.26, 0.2),
    (-22.0, 3.55, 2.08, 1.98, 0.26),
    (-24.6, 3.3, 1.82, 1.72, 0.3),
]
FM, FN, FNB, FNC = 128, 64, 9, 7
FSS = crom(FS, FM)

WS = [  # x, z, leading edge y, trailing edge y, thickness
    (2.4, 0.1, 7.5, -20.8, 1.3),
    (4.6, 0.12, 2.6, -21.0, 1.6),
    (7.5, 0.18, -3.0, -21.4, 1.6),
    (10.0, 0.26, -7.6, -22.0, 1.5),
    (11.8, 0.3, -10.8, -22.4, 1.4),
]
WM = 40
WSS = crom(WS, WM)

OS = [  # outrigger hulls: y, half width, height above / below, centre z
    (5.9, 0.05, 0.05, 0.05, 0.2),
    (4.7, 0.5, 0.56, 0.5, 0.2),
    (2.0, 1.0, 1.2, 1.05, 0.22),
    (-3.0, 1.4, 1.75, 1.5, 0.25),
    (-10.0, 1.55, 1.95, 1.7, 0.3),
    (-18.0, 1.55, 1.95, 1.7, 0.3),
    (-24.0, 1.52, 1.88, 1.66, 0.3),
    (-27.6, 1.45, 1.74, 1.54, 0.3),
]
OM, ON, ONB, ONC = 72, 40, 5, 5
OSS = crom(OS, OM)
OKW = dict(ex=0.5, ezt=0.6, ezb=0.6)

SUP = [  # dorsal sail: y, half width, top above the hull crown
    (13.0, 0.15, 0.05), (10.5, 0.85, 0.55), (5.0, 1.45, 0.95), (-1.5, 1.8, 1.25), (-4.2, 1.9, 2.75), (-7.0, 1.9, 3.3),
    (-11.0, 1.7, 3.15), (-14.5, 1.5, 1.7), (-19.0, 1.0, 0.95), (-22.6, 0.15, 0.05),
]
SUPS = crom(SUP, 60)
SKW = dict(ex=0.5, ezt=0.62)

def fus(y):
    return at(FSS, y)


def fus_top(y, x=0.0):
    _, hw, ht, hb, cz = fus(y)
    c = min(abs(x) / hw, 1.0) ** (1 / EX)
    return cz + ht * math.sqrt(max(1 - c * c, 0.0)) ** EZT


def fus_bot(y, x=0.0):
    _, hw, ht, hb, cz = fus(y)
    c = min(abs(x) / hw, 1.0) ** (1 / EX)
    return cz - hb * math.sqrt(max(1 - c * c, 0.0)) ** EZB


def S_hull(y, a):
    return sec_pt(fus(y), a)


def sup_st(s):
    y, hw, top = s
    crown = fus_top(y)
    return (y, max(hw, 0.05), max(top, 0.02) + 0.9, 0.5, crown - 0.9)


def sup_top(y):
    return fus_top(y) + at(SUPS, y)[2]


def S_sup(y, a):
    return sec_pt(sup_st(at(SUPS, y)), a, **SKW)


def hull_ab(y):
    return math.pi / 2 - max(liv_lim(y), 0.012) * 2 * math.pi


def S_hull_f(y, f):
    """Hull point at fraction f of the way from the chine to the paint edge: follows the mesh rows."""
    return sec_pt(fus(y), f * hull_ab(y))


OSP = crom([(-3.2, 0.1, 0.05), (-6.0, 0.5, 0.4), (-12.0, 0.64, 0.56), (-20.0, 0.62, 0.52), (-25.0, 0.42, 0.3), (-27.0, 0.1, 0.05)], 30)


def osp_st(q):
    y, hw, top = q
    o = at(OSS, y)
    return (y, max(hw, 0.05), max(top, 0.02) + 0.6, 0.3, o[4] + o[2] - 0.6)


def S_osp(y, a):
    return sec_pt(osp_st(at(OSP, y)), a, **SKW) + V((OX, 0, 0))


VS = [(21.5, 0.1, 0.05), (18.5, 0.75, 0.6), (12.0, 1.2, 1.2), (4.0, 1.35, 1.35), (-8.0, 1.3, 1.3), (-15.0, 1.0, 0.9),
      (-20.0, 0.5, 0.4), (-22.5, 0.1, 0.05)]  # ventral gallery: y, half width, depth below the hull
VM, VN = 52, 32
VSS = crom(VS, VM)
VKW = dict(ex=0.5, ezt=0.6, ezb=0.6)
VH0, VH1 = 5.6, 11.2           # its hangar bays


def v_st(q):
    y, hw, d = q
    return (y, max(hw, 0.05), 0.4, max(d, 0.02) + 0.7, fus_bot(y) + 0.7)


def S_v(y, a):
    return sec_pt(v_st(at(VSS, y)), a, **VKW)


DARK_REGIONS.append(((0.3, 1.1), (VH0 + 0.12, VH1 - 0.12), (S_v(8.4, -math.radians(33.75)).z, S_v(8.4, -math.radians(11.25)).z)))


def S_out(y, a):
    return sec_pt(at(OSS, y), a, **OKW) + V((OX, 0, 0))


def S_wing(x, u):
    _, z, le, te, t = at(WSS, x)
    return V((x, le - (le - te) * u, z + foil_h(u, t)))


def out_lim(y):
    pts = [(-27.7, 0.012), (-10.5, 0.012), (-3.5, 0.235), (5.0, 0.235), (6.0, 0.2)]
    return at(pts, y)[1]


def wing_z(x, y, top=True):
    _, z, le, te, t = at(WSS, x)
    u = min(max((le - y) / (le - te), 0.0), 1.0)
    return z + foil_h(u, t) * (1.0 if top else -0.8)


def liv_lim(y):
    """How far down from the crown (fraction of the girth) the paint field reaches at station y."""
    pts = [(-25.0, 0.03), (-21.0, 0.09), (-13.0, 0.16), (0.0, 0.16), (8.0, 0.085), (21.0, 0.07), (27.4, 0.0)]
    return at(pts, y)[1]


def build(ship):
    D = math.radians

    # ---------------------------------------------------------------- main hull
    def hull(bm):
        rings = [V((0, 30.0, -0.3))] + [fsec(st, FN, fb=max(liv_lim(st[0]), 0.012), nb=FNB, nc=FNC) for st in FSS]

        def mf(i, k):
            if i == 0:
                return H
            y = (FSS[i - 1][0] + FSS[i][0]) / 2
            return L if FNB <= k < FNB + 2 * FNC and liv_lim(y) > 0.012 else H
        res = loft(bm, rings, matfn=mf)
        bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
        # the gallery: a long shallow trench above the chine, where the window rows sit
        up = 2 * (FNB + FNC) - 1
        for ks in ((2, 3, 4), (up - 2, up - 3, up - 4)):
            fs = [res['bands'][i][k] for i in range(1, FM + 1) for k in ks if TY0 < (FSS[i - 1][0] + FSS[i][0]) / 2 < TY1]
            bmesh.ops.inset_region(bm, faces=fs, thickness=0.03, depth=-TD, use_even_offset=True, use_boundary=True)
    ship.part(hull, bevel=0.02, segs=2, angle=38)

    # ---------------------------------------------------------------- dorsal sail and bridge deck
    def sail(bm):
        rings = [V((0, 13.2, fus_top(13.2) - 0.1))] + [fsec(sup_st(s), 36, **SKW) for s in SUPS] + [V((0, -22.8, fus_top(-22.8) - 0.1))]
        loft(bm, rings)
    ship.part(sail, bevel=0.02, segs=2, angle=38)

    BZ = sup_top(-6.0) + 0.05
    BR = crom([(-4.9, 0.05, 0.04, 0.04, BZ - 0.1), (-5.4, 0.6, 0.26, 0.3, BZ - 0.05), (-6.4, 1.0, 0.34, 0.4, BZ),
               (-7.6, 1.08, 0.36, 0.42, BZ), (-8.8, 0.95, 0.32, 0.36, BZ), (-9.8, 0.5, 0.22, 0.24, BZ - 0.03),
               (-10.3, 0.05, 0.04, 0.04, BZ - 0.06)], 30)

    def bridge(bm):
        rings = [V((0, -4.8, BZ - 0.1))] + [fsec(st, 28, ex=0.7) for st in BR] + [V((0, -10.4, BZ - 0.06))]
        loft(bm, rings)
        # the observation band: one unbroken line of light along the deck's edge
        for sx in (1, -1):
            for a, b in zip(BR[2:-2], BR[3:-2]):
                bs.strip(bm, (sx * (a[1] + 0.0), a[0], a[4]), (sx * (b[1] + 0.0), b[0], b[4]), 0.014, 0.022)
            # the bridge visor: a band of light wrapping the front of the sail
            ys = frange(-3.5, -11.6, 0.4)
            for a, b in zip(ys, ys[1:]):
                pa, pb = S_sup(a, D(62)), S_sup(b, D(62))
                bs.strip(bm, (sx * pa.x, pa.y, pa.z), (sx * pb.x, pb.y, pb.z), 0.016, 0.03)
        angs = frange(62, 118, 4)
        for a, b in zip(angs, angs[1:]):
            bs.strip(bm, S_sup(-3.5, D(a)), S_sup(-3.5, D(b)), 0.03, 0.016)
        # sensor domes, mast, dish
        for sx in (1, -1):
            sphere(bm, (sx * 0.5, -7.2, BZ + 0.42), 0.2, seg=16, rings=8, scale=(1, 1, 0.8))
            cyl(bm, (sx * 0.5, -7.2, BZ + 0.2), (sx * 0.5, -7.2, BZ + 0.44), 0.22, 0.2, n=16)
        cyl(bm, (0, -8.6, BZ + 0.3), (0, -8.6, BZ + 0.7), 0.22, 0.14, n=12)
        cyl(bm, (0, -8.6, BZ + 0.7), (0, -8.75, BZ + 1.95), 0.045, 0.02, n=8)
        for z, w in ((1.05, 0.5), (1.35, 0.36), (1.62, 0.22)):
            prism(bm, (-w, -8.66 - z * 0.1, BZ + z), (w, -8.66 - z * 0.1, BZ + z), 0.018, 0.018)
        bs.light(bm, (0, -8.75, BZ + 1.97), 0.045)
        lathe_y(bm, 0, BZ + 0.78, [(-6.1, 0.0), (-6.15, 0.16), (-5.98, 0.36), (-5.92, 0.38), (-5.94, 0.32), (-6.07, 0.14), (-6.09, 0.0)], n=20)
        cyl(bm, (0, -6.2, BZ + 0.3), (0, -6.14, BZ + 0.78), 0.06, 0.045, n=8)
    ship.part(bridge, bevel=0.01, segs=1, angle=38)

    # ---------------------------------------------------------------- bridge wing
    def wing_paint(s, u, upper):
        return L if 5.4 < s[0] < 8.1 else H

    def wings(bm):
        foil(bm, WS, WM, paint=wing_paint, groove=lambda st: 0.05 * min(max((st[0] - 4.9) / 0.4, 0.0), 1.0))
    ship.part(wings, bevel=0.02, segs=1, mirror=True, angle=40)

    # ---------------------------------------------------------------- outrigger hulls
    def outrigger(bm):
        rings = [V((0, 6.0, 0.2))] + [fsec(st, ON, fb=out_lim(st[0]), nb=ONB, nc=ONC, **OKW) for st in OSS]
        for r in rings[1:]:
            for p in r:
                p.x += OX
        rings[0].x += OX
        res = loft(bm, rings, matfn=lambda i, k: L if ONB <= k < ONB + 2 * ONC and i > 0 else H)
        bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
        # hangar bay: the outboard flank opens into a lit deck
        fs = [res['bands'][i][k] for i in range(1, OM + 1) for k in (0, ON - 1)
              if HY0 < (OSS[i - 1][0] + OSS[i][0]) / 2 < HY1]
        bmesh.ops.inset_region(bm, faces=fs, thickness=0.1, depth=-1.05, use_even_offset=True, use_boundary=True)
        top = S_out(-16.0, 2 * math.pi * (0.25 - out_lim(-16.0)) / ONB).z
        bot = S_out(-16.0, -math.pi / (ON // 2)).z
        x = OX + 1.0
        bs.strip(bm, (x, HY0 + 0.3, top - 0.1), (x, HY1 - 0.3, top - 0.1), 0.16, 0.012)
        bs.strip(bm, (OX + 0.56, HY0 + 0.4, (top + bot) / 2), (OX + 0.56, HY1 - 0.4, (top + bot) / 2), 0.012, 0.025)
        for i in range(7):  # parked craft, cargo and deck lights
            y = HY0 + 0.8 + i * 0.93
            box(bm, (OX + 0.85 + 0.12 * (i % 2), y, bot + 0.12), (0.32, 0.5 - 0.14 * (i % 2), 0.1), taper=(0.5, 1.0))
            bs.light(bm, (OX + 1.36, y + 0.45, bot + 0.09), 0.022)
        # one great nozzle
        npf, nm = nozzle_prof(-27.6, -29.9, 1.35, glow=0.06)
        lathe_y(bm, OX, 0.4, [(-27.0, 0.0), (-27.0, 1.3)] + npf, n=40, mats=[H, H] + nm)
        petals(bm, OX, 0.4, -27.74, -29.9, 1.35, 22, 0.16)
        # dorsal spine
        rr = [V((OX, -3.1, osp_st(OSP[0])[4] + 0.55))] + [[q + V((OX, 0, 0)) for q in fsec(osp_st(q), 20, **SKW)] for q in OSP]
        loft(bm, rr + [V((OX, -27.1, osp_st(OSP[-1])[4] + 0.55))])
        # ventral stabiliser
        foil(bm, [(OX + 0.5, -1.1, -16.5, -25.6, 0.56), (OX + 0.95, -2.3, -20.0, -26.6, 0.4), (OX + 1.45, -3.6, -23.4, -27.7, 0.18)],
             12, sym=True, paint=lambda s, u, upper: L if s[1] < -2.2 else H)
    ship.part(outrigger, bevel=0.016, segs=1, mirror=True, angle=34)
    ship.empty("Engine_3", (OX, -29.9, 0.4), 1.17)
    ship.empty("Engine_4", (-OX, -29.9, 0.4), 1.17)

    # ---------------------------------------------------------------- stern engines
    def stern(bm):
        npf, nm = nozzle_prof(-24.7, -27.4, 1.5, glow=0.06)
        lathe_y(bm, 0, CZ_, [(-24.0, 0.0), (-24.0, 1.45)] + npf, n=40, mats=[H, H] + nm)
        petals(bm, 0, CZ_, -24.84, -27.4, 1.5, 24, 0.16)
        for sx in (1, -1):
            npf, nm = nozzle_prof(-24.7, -26.5, 1.0, glow=0.05)
            lathe_y(bm, sx * SX_, CZ_, [(-24.0, 0.0), (-24.0, 0.96)] + npf, n=36, mats=[H, H] + nm)
            petals(bm, sx * SX_, CZ_, -24.82, -26.5, 1.0, 16, 0.16)
    ship.part(stern, bevel=0.016, segs=1, angle=30)
    ship.empty("Engine_0", (0, -27.4, CZ_), 1.3)
    ship.empty("Engine_1", (SX_, -26.5, CZ_), 0.87)
    ship.empty("Engine_2", (-SX_, -26.5, CZ_), 0.87)

    # ---------------------------------------------------------------- keel fin
    KS = [(0.0, -1.6, 2.5, -14.0, 1.3), (0.0, -3.4, -0.8, -14.6, 1.05), (0.0, -5.2, -4.6, -15.4, 0.7), (0.0, -7.2, -8.8, -16.4, 0.3)]
    KSS = crom(KS, 20)

    def keel(bm):
        foil(bm, KS, 20, paint=lambda s, u, upper: L if -5.3 < s[1] < -3.3 else H, sym=True)
        bs.light(bm, (0, -16.35, -7.22), 0.05)
    ship.part(keel, bevel=0.012, segs=1, angle=40)

    # ---------------------------------------------------------------- ventral gallery
    def ventral(bm):
        rings = [V((0, 21.7, fus_bot(21.7) + 0.1))] + [fsec(v_st(q), VN, **VKW) for q in VSS] + [V((0, -22.7, fus_bot(-22.7) + 0.1))]
        res = loft(bm, rings)
        bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
        for ks in ((VN - 2, VN - 3), (VN // 2 + 1, VN // 2 + 2)):
            fs = [res['bands'][i][k] for i in range(1, VM + 1) for k in ks if VH0 < (VSS[i - 1][0] + VSS[i][0]) / 2 < VH1]
            bmesh.ops.inset_region(bm, faces=fs, thickness=0.06, depth=-0.75, use_even_offset=True, use_boundary=True)
        zt, zb = S_v(8.4, -D(11.25)).z, S_v(8.4, -D(33.75)).z
        for sx in (1, -1):
            bs.strip(bm, (sx * 0.85, VH0 + 0.3, zt - 0.07), (sx * 0.85, VH1 - 0.3, zt - 0.07), 0.12, 0.01)
            for i in range(6):
                y = VH0 + 0.6 + i * 0.88
                box(bm, (sx * (0.78 + 0.08 * (i % 2)), y, zb + 0.1), (0.26, 0.4, 0.08), taper=(0.5, 1.0))
                bs.light(bm, (sx * 1.1, y + 0.44, zb + 0.07), 0.02)
    ship.part(ventral, bevel=0.016, segs=1, angle=34)

    # ---------------------------------------------------------------- batteries
    muzzles = []

    def guns(bm):
        for x, y, up, s in ((1.3, 20.5, 1, 0.55), (2.7, 9.0, 1, 0.7), (7.0, -11.0, 1, 0.7), (2.3, 15.0, -1, 0.65)):
            z = (wing_z(x, y) if x > 5 else fus_top(y, x)) if up > 0 else fus_bot(y, x)
            muzzles.append(turret(bm, (x, y, z - up * 0.03), up, s))
        # point defence: small domes
        for x, y, up in ((0.9, 24.5, 1), (3.3, 2.0, 1), (3.6, -15.5, 1), (2.2, -21.0, 1), (5.0, -7.0, 1), (9.0, -16.0, 1),
                         (4.0, -19.0, 1), (2.5, 15.0, 1), (1.6, 22.0, -1), (3.2, 4.0, -1), (3.3, -17.0, -1), (7.5, -14.0, -1)):
            z = (wing_z(x, y, up > 0) if x > 4.7 else (fus_top(y, x) if up > 0 else fus_bot(y, x)))
            p = V((x, y, z))
            sphere(bm, p, 0.12, seg=10, rings=6, scale=(1, 1, 0.8))
            cyl(bm, p + V((0, 0.06, up * 0.05)), p + V((0, 0.32, up * 0.07)), 0.02, n=6)
    ship.part(guns, bevel=0.008, segs=1, mirror=True, angle=30)

    # ---------------------------------------------------------------- deck fittings
    def fittings(bm):
        # missile hatches on the fore deck
        for y in frange(14.2, 19.6, 0.36):
            for x in (0.22, 0.5):
                box(bm, (x, y, fus_top(y, x) + 0.005), (0.16, 0.2, 0.04), mat=L)
        # hatches and a vent bank on the bridge wing
        for i, x in enumerate(frange(5.75, 7.75, 0.4)):
            _, z, le, te, t = at(WSS, x)
            y = le - (le - te) * 0.42
            box(bm, (x, y, wing_z(x, y) + 0.0), (0.2, 0.36, 0.04), mat=L)
            y = le - (le - te) * 0.56
            box(bm, (x, y, wing_z(x, y) + 0.0), (0.2, 0.36, 0.04), mat=L)
        for x in (8.9, 9.5):
            bs.vent(bm, (x, -15.5, wing_z(x, -15.5) - 0.01), 0.4, 1.6, n=9, h=0.04)
        # antenna farm behind the bridge
        for i, (x, y, h) in enumerate(((0.45, -12.6, 1.1), (0.75, -13.6, 0.7), (0.3, -14.4, 0.5))):
            bs.antenna(bm, (x, y, sup_top(y) - 0.12), h, r=0.018, tip=(i == 0), lean=(0, -0.1, 1))
        # docking collars on the hull flank, ahead of the wing
        for y in (12.5, 17.5):
            pc = S_hull(y, D(-11))
            cyl(bm, pc - V((0.15, 0, 0.04)), pc + V((0.07, 0, 0.02)), 0.2, n=14)
            cyl(bm, pc + V((0.07, 0, 0.02)), pc + V((0.09, 0, 0.025)), 0.13, n=14, mat=G)
        # sensor blisters under the nose
        sphere(bm, (0.45, 24.0, fus_bot(24.0, 0.45)), 0.2, seg=12, rings=6, scale=(1, 1.9, 0.6))
    ship.part(fittings, bevel=0.008, segs=1, mirror=True, angle=30)
    n = 0
    for m in muzzles:
        for sx in (1, -1):
            ship.empty("Gun_%d" % n, (sx * m.x, m.y, m.z), 0.2)
            n += 1

    # ---------------------------------------------------------------- windows
    def lights(bm):
        for r, (f, y1) in enumerate(((2.5 / FNB, TY1 - 0.3), (3.5 / FNB, 21.0), (4.5 / FNB, 16.0))):
            windows(bm, S_hull_f, frange(TY0 + 0.3, y1, 0.17), f, row=r, lift=0.012 - TD, fill=0.74)
        windows(bm, S_hull_f, frange(-21.0, 19.0, 0.17), 0.8 / FNB, row=4, fill=0.5)
        windows(bm, S_hull_f, frange(-20.0, 20.0, 0.17), 6.6 / FNB, row=5, fill=0.5)
        for r, a in enumerate((20, 160)):
            windows(bm, S_osp, frange(-25.5, -5.0, 0.17), D(a), row=60 + r, fill=0.7, out=(1, 0, 0) if r == 0 else (-1, 0, 0))
        for r, (a, y0, y1) in enumerate(((-7, -23.0, 25.5), (-12, -22.0, 23.5), (-17, -20.0, 20.0), (-22.5, -17.0, 15.0))):
            windows(bm, S_hull, frange(y0, y1, 0.17), D(a), row=10 + r, fill=0.6)
        for r, (a, y0, y1) in enumerate(((16, -19.0, 9.0), (30, -13.5, -2.6), (41, -12.0, -3.6), (52, -11.0, -4.4))):
            windows(bm, S_sup, frange(y0, y1, 0.17), D(a), row=20 + r, fill=0.8)
        hang = lambda y: HY0 - 0.4 < y < HY1 + 0.4
        for r, a in enumerate((33, 42, -30, 147, 138, 210)):
            windows(bm, S_out, frange(-26.0, 2.5 if r % 3 == 0 else 0.5, 0.17), D(a), row=30 + r, fill=0.62,
                    out=(1, 0, 0) if r < 3 else (-1, 0, 0))
        for r, a in enumerate((-9, 9)):
            windows(bm, S_out, frange(-26.0, 2.0, 0.17), D(a), row=37 + r, fill=0.62, skip=hang)
        vh = lambda y: VH0 - 0.3 < y < VH1 + 0.3
        for r, (a, y0, y1) in enumerate(((-17, -18.0, 17.5), (-28, -16.0, 16.0), (-40, -13.0, 13.5))):
            windows(bm, S_v, frange(y0, y1, 0.17), D(a), row=70 + r, fill=0.66, skip=vh if r < 2 else None)
        for r, u in enumerate((0.03, 0.055)):
            windows(bm, S_wing, frange(3.6, 10.2, 0.14), u, row=40 + r, fill=0.7, out=(0, 0.5, 1))
        # keel fin
        for r, z in enumerate((-2.6, -3.0)):
            _, _, le, te, t = at(KSS, z, key=1)
            for i, y in enumerate(frange(te + 1.5, le - 1.2, 0.17)):
                if rnd(i, 50 + r) > 0.66:
                    continue
                x = foil_h((le - y) / (le - te), t) + 0.012
                bm.faces.new([bm.verts.new(V((x, y - 0.04, z - 0.022))), bm.verts.new(V((x, y + 0.04, z - 0.022))),
                              bm.verts.new(V((x, y + 0.04, z + 0.022))), bm.verts.new(V((x, y - 0.04, z + 0.022)))]).material_index = G
    ship.part(lights, bevel=0.0, mirror=True, recalc=False, weighted=False)

    # ---------------------------------------------------------------- light lines
    def strips(bm):
        for sx in (1, -1):
            ys = frange(27.2, 8.2, 0.5)
            for a, b in zip(ys, ys[1:]):
                fa, fb = fus(a), fus(b)
                bs.strip(bm, (sx * fa[1], a, fa[4]), (sx * fb[1], b, fb[4]), 0.016, 0.03)
            ys = frange(4.6, -11.5, 0.5)
            for a, b in zip(ys, ys[1:]):
                pa, pb = S_out(a, 0.0), S_out(b, 0.0)
                bs.strip(bm, (sx * pa.x, a, pa.z), (sx * pb.x, b, pb.z), 0.014, 0.028)
            # the bridge wing's trailing edge
            xs = frange(3.9, 10.2, 0.45)
            for a, b in zip(xs, xs[1:]):
                wa, wb = at(WSS, a), at(WSS, b)
                bs.strip(bm, (sx * a, wa[3] - 0.005, wa[1]), (sx * b, wb[3] - 0.005, wb[1]), 0.02, 0.022)
        bs.strip(bm, (0, -15.5, sup_top(-15.5) + 0.0), (0, -18.0, sup_top(-18.0) + 0.0), 0.04, 0.02)
    ship.part(strips, bevel=0.0)

    _finish(ship)


def _finish(ship):
    """Tell the paint where the model's bounding-box centre is (finalize moves it to the origin) and log paint share."""
    lo, hi = np.full(3, 1e9), np.full(3, -1e9)
    area = {}
    for o in ship.groups.get("body", []):
        me = o.data
        co = np.empty(len(me.vertices) * 3)
        me.vertices.foreach_get("co", co)
        co = co.reshape(-1, 3)
        lo, hi = np.minimum(lo, co.min(axis=0)), np.maximum(hi, co.max(axis=0))
        for p in me.polygons:
            area[p.material_index] = area.get(p.material_index, 0.0) + p.area
    c = (lo + hi) / 2
    if _MAP is not None:
        _MAP.inputs['Location'].default_value = tuple(float(v) for v in c)
    tot = sum(area.values()) or 1.0
    log("global bbox centre", [round(float(v), 3) for v in c],
        "share", {shiplib.MAT_NAMES[k]: round(v / tot, 3) for k, v in sorted(area.items())})

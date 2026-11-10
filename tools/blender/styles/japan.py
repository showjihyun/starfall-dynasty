"""Style candidate `japan`: a heroic flagship, about 420 m (60 units, 1 unit ~ 7 m).

Naval battleship lines in white armour over a red lower hull: bow cannon muzzle under a blade prow, two
superfiring triple turrets forward and one aft, a pagoda bridge with rangefinder arms and a V-fin, a raked
funnel, two catapult legs with lit hangar mouths, radiator wings and a five-nozzle engine block.
The dark frame, the red, the yellow and the grey are not separate materials but a per-face float attribute
`tone` that the Hull graph reads (see `materials`).
"""
import math

from mathutils import Matrix

import bmesh

import shiplib
from shiplib import V, H, L, G, GL, E, loft, ring10, seg_sym, lathe, box, prism, cyl, sphere, wing

import build_ships as bs
from build_ships import interp

TONE = "tone"
FRAME, YEL, RED, GREY, ARM = 1, 2, 3, 4, 5   # 0 = untouched = armour; ARM forces armour inside a toned block

STYLE = dict(
    paint=(0.60, 0.615, 0.64), paint2=(0.52, 0.54, 0.575), accent=(0.40, 0.43, 0.48), accent_thresh=0.88,
    metal=0.04, rough=0.34, spacing=(0.8, 1.05, 0.7), wear=0.1, grime=0.35, sub_amt=0.0, seam_w=0.013,
    grime_col=(0.10, 0.11, 0.13), bump=1.0, bare=(0.75, 0.76, 0.78),
)

TINT = dict(
    livery=(0.012, 0.07, 0.48),       # hero blue
    livery_metal=0.1, livery_rough=0.3,
    glow=(1.0, 0.8, 0.45),            # lit windows
    engine=(0.45, 0.72, 1.0),         # blue-white burn
    glass=(0.02, 0.05, 0.06),
    rim=(0.6, 0.75, 1.0),
    enemy_livery=(0.45, 0.02, 0.03),
    enemy_glow=(1.0, 0.25, 0.12),
)

TONES = {  # code: (colour, roughness, metallic)
    FRAME: ((0.030, 0.033, 0.040), 0.42, 0.85),
    YEL: ((0.85, 0.48, 0.012), 0.35, 0.0),
    RED: ((0.52, 0.03, 0.022), 0.4, 0.0),
    GREY: ((0.17, 0.185, 0.22), 0.4, 0.35),
}

AREA = {}


def materials():
    """shiplib's Hull, then the `tone` attribute repaints frame / yellow / red / grey faces. The new colour
    multiplies the armour, so panel lines and shading carry through."""
    mats, socks = shiplib.make_materials(STYLE)
    m = mats["Hull"]
    nb = shiplib.NB(m)
    at = nb.node('ShaderNodeAttribute')
    at.attribute_name = TONE
    t = at.outputs['Fac']
    base0 = socks['base']
    base, rough, metal = socks['base'], socks['rough'], socks['metal']
    p0 = STYLE['paint']
    for code, (rgb, r, mt) in TONES.items():
        k = nb.node('ShaderNodeMath', operation='COMPARE')
        nb.link(t, k.inputs[0])
        k.inputs[1].default_value = code
        k.inputs[2].default_value = 0.3
        mul = nb.node('ShaderNodeMix', data_type='RGBA', blend_type='MULTIPLY')
        mul.inputs[0].default_value = 1.0
        nb.link(base0, mul.inputs[6])
        mul.inputs[7].default_value = (*[c / p for c, p in zip(rgb, p0)], 1)
        base = nb.mix(k.outputs[0], base, mul.outputs[2])
        rough = nb.mixf(k.outputs[0], rough, r)
        metal = nb.mixf(k.outputs[0], metal, mt)
    bsdf = socks['bsdf']
    nb.link(base, bsdf.inputs['Base Color'])
    nb.link(rough, bsdf.inputs['Roughness'])
    nb.link(metal, bsdf.inputs['Metallic'])
    socks.update(base=base, rough=rough, metal=metal)
    return mats, socks


# ================================================================ helpers

class tone:
    """with tone(bm, FRAME): every face made inside (and not toned yet) gets that tone."""

    def __init__(self, bm, t):
        self.bm, self.t = bm, t

    def __enter__(self):
        self.n = len(self.bm.faces)
        return self

    def __exit__(self, *a):
        lay = self.bm.faces.layers.float[TONE]
        for f in list(self.bm.faces)[self.n:]:
            if f[lay] == 0:
                f[lay] = self.t


def P(ship, fn, t=0, **kw):
    """ship.part with the tone layer (created before any face, a new layer invalidates face handles)."""
    def b(bm):
        bm.faces.layers.float.new(TONE)
        fn(bm)
        if t:
            lay = bm.faces.layers.float[TONE]
            for f in bm.faces:
                if f[lay] == 0:
                    f[lay] = t
    o = ship.part(b, **kw)
    me = o.data
    me.calc_loop_triangles()
    for p in me.polygons:
        AREA[p.material_index] = AREA.get(p.material_index, 0.0) + p.area
    tot = sum(AREA.values())
    shiplib.log("  part %-12s %6d tris   livery %.1f%% glow %.1f%%" % (
        fn.__name__, len(me.loop_triangles), 100 * AREA.get(L, 0) / tot, 100 * AREA.get(G, 0) / tot))
    return o


def inset(bm, faces, th, depth, t=None, mat=None):
    """Panel inset; t tones the sunk face and its walls."""
    faces = [f for f in faces if f is not None and f.is_valid]
    if not faces:
        return
    bmesh.ops.recalc_face_normals(bm, faces=list(bm.faces))   # loft winding varies; depth < 0 must mean inward
    n = len(bm.faces)
    shiplib.inset_faces(bm, faces, th, depth, mat=mat)
    if t is not None:
        lay = bm.faces.layers.float[TONE]
        for f in faces + list(bm.faces)[n:]:
            f[lay] = t


def paint(bm, faces, t):
    lay = bm.faces.layers.float[TONE]
    for f in faces:
        if f is not None and f.is_valid:
            f[lay] = t


def pick(res, fn):
    return [f for i, band in enumerate(res['bands']) for k, f in enumerate(band) if fn(i, k)]


def wst(a, b, n):
    return [{k: a[k] + (b[k] - a[k]) * i / n for k in a} for i in range(n + 1)]


def slab(bm, outline, zf, t, chamf=0.07, mat=H):
    """Armour plate: (x, y) outline, underside at zf(x, y), thickness t, chamfered top edge."""
    cx = sum(p[0] for p in outline) / len(outline)
    cy = sum(p[1] for p in outline) / len(outline)

    def inn(p, d):
        v = V((cx - p[0], cy - p[1]))
        ln = max(v.length, 1e-6)
        return (p[0] + v.x / ln * d, p[1] + v.y / ln * d)
    bot = [V((x, y, zf(x, y))) for x, y in outline]
    mid = [V((x, y, zf(x, y) + t - chamf)) for x, y in outline]
    top = [V((x, y, zf(x, y) + t)) for x, y in (inn(p, chamf * 1.4) for p in outline)]
    return loft(bm, [bot, mid, top], mat=mat)


def house(bm, y0, y1, hw, z0, z1, ch=0.4, rf=0.2, rb=0.15, ins=0.0, x=0.0, mat=H, matfn=None):
    """Deckhouse: chamfered-corner block from y0 (front) to y1, raked front (rf) and back (rb), sides leaning
    in by `ins`. Side faces of band 0: 0 front-right corner, 1 right, 2 rear-right, 3 rear, 5 left, 7 front."""
    def ring(ya, yb, w, z, c):
        return [V((x + px, py, z)) for px, py in ((w - c, ya), (w, ya - c), (w, yb + c), (w - c, yb),
                                                  (-w + c, yb), (-w, yb + c), (-w, ya - c), (-w + c, ya))]
    return loft(bm, [ring(y0, y1, hw, z0, ch), ring(y0 - rf, y1 + rb, hw - ins, z1, ch * (hw - ins) / hw)], mat=mat, matfn=matfn)


def _h(i, seed):
    x = (i * 2654435761 + seed * 40503 + 12345) & 0xffffffff
    x ^= x >> 13
    x = (x * 1274126177) & 0xffffffff
    return (x >> 8) & 0xffff


def win_row(bm, a, b, out, pitch=0.26, w=0.12, h=0.065, skip=4, seed=0, mat=G):
    """A row of tiny lit windows from a to b on a surface facing `out`; every `skip`-th or so is dark."""
    a, b = V(a), V(b)
    out = V(out).normalized()
    al = (b - a).normalized()
    up = al.cross(out).normalized()
    R = Matrix((al, out, up)).transposed()
    n = max(1, int((b - a).length / pitch))
    for i in range(n):
        if skip and (_h(i, seed) % skip == 0 or _h(i // 6, seed + 77) % 4 == 0):
            continue
        box(bm, a + (b - a) * ((i + 0.5) / n), (w, 0.04, h), mat=mat, rot=R)


def house_windows(bm, y0, y1, hw, zs, ch=0.4, x=0.0, front=False, seed=0):
    for j, z in enumerate(zs):
        for sx in (1, -1):
            win_row(bm, (x + sx * hw, y0 - ch - 0.15, z), (x + sx * hw, y1 + ch + 0.15, z), (sx, 0, 0), seed=seed + j + (sx > 0))
        if front:
            win_row(bm, (x - hw + ch + 0.1, y0, z), (x + hw - ch - 0.1, y0, z), (0, 1, 0), seed=seed + j)


def vernier(bm, p, d, r=0.12, ln=0.2, hot=False):
    """Vernier cup on a surface at p, thrusting along d."""
    hm = E if hot else H
    lathe(bm, p, d, [(r * 0.7, -ln * 0.5), (r * 0.72, 0.0), (r * 0.8, ln * 0.45), (r, ln), (r * 0.86, ln),
                     (r * 0.62, ln * 0.3), (0.0, ln * 0.25)], n=10, mats=[H, H, H, H, H, hm])


def engine(bm, c, r, ln, petals=16, n=32):
    """Nozzle: frame collar with a red band, armour petals, glowing bell and a dark plug on struts."""
    c = V(c)
    f = V((0, 1, 0))
    with tone(bm, FRAME):
        lathe(bm, c + f * (0.04 * ln), f, [(r * 0.84, ln), (r, ln * 0.6), (r * 1.06, ln * 0.14), (r * 1.04, 0.0), (r * 0.9, 0.0),
                                           (r * 0.86, ln * 0.08), (r * 0.78, ln * 0.3), (r * 0.5, ln * 0.6), (0.0, ln * 0.6)],
              n=n, mats=[H, H, H, G, H, H, E, E])
        lathe(bm, c, f, [(r * 1.08, ln * 1.0), (r * 1.34, ln * 0.97), (r * 1.34, ln * 0.84), (r * 1.22, ln * 0.82),
                         (r * 1.22, ln * 0.74), (r * 1.3, ln * 0.72), (r * 1.3, ln * 0.64), (r * 1.1, ln * 0.62)],
              n=n, cap_start=False, cap_end=False)
        lathe(bm, c, f, [(0.0, ln * 0.06), (r * 0.24, ln * 0.24), (r * 0.32, ln * 0.5), (r * 0.24, ln * 0.6)], n=16)
        for a in range(6):
            ang = a * math.pi / 3
            d = V((math.cos(ang), 0, math.sin(ang)))
            prism(bm, c + f * (ln * 0.5) + d * (r * 0.3), c + f * (ln * 0.5) + d * (r * 0.6), r * 0.03, r * 0.25, up=(0, 1, 0))
    with tone(bm, RED):
        lathe(bm, c, f, [(r * 1.36, ln * 0.93), (r * 1.36, ln * 0.88)], n=n, cap_start=False, cap_end=False)
    with tone(bm, GREY):
        for a in range(petals):
            ang = (a + 0.5) * 2 * math.pi / petals
            d = V((math.cos(ang), 0, math.sin(ang)))
            s = math.sin(math.pi / petals)
            prism(bm, c + f * (ln * 0.74) + d * (r * 1.24), c + f * (ln * 0.12) + d * (r * 1.12),
                  r * 1.24 * s * 0.9, r * 0.05, w1=r * 1.12 * s * 0.84, h1=r * 0.035, up=d)


def turret(bm, c, fwd=1, s=1.0, elev=5.0):
    """Triple turret on a barbette at c (deck level), firing along fwd*Y. Returns the three muzzle points."""
    c = V(c)

    def pt(x, y, z):
        return c + V((x * s, y * s * fwd, z * s))
    with tone(bm, FRAME):
        cyl(bm, pt(0, 0, -0.2), pt(0, 0, 0.22), 1.3 * s, n=20)
    low = [(0.6, 1.35), (1.15, 0.65), (1.2, -0.9), (0.8, -1.55), (-0.8, -1.55), (-1.2, -0.9), (-1.15, 0.65), (-0.6, 1.35)]
    top = [(0.5, 0.75), (0.88, 0.3), (0.92, -0.8), (0.6, -1.3), (-0.6, -1.3), (-0.92, -0.8), (-0.88, 0.3), (-0.5, 0.75)]
    res = loft(bm, [[pt(x, y, 0.2) for x, y in low], [pt(x, y, 0.5) for x, y in low], [pt(x, y, 1.05) for x, y in top]])
    inset(bm, [res['end']], 0.12 * s, -0.03 * s, t=GREY)
    inset(bm, pick(res, lambda i, k: i == 1 and k in (1, 5)), 0.08 * s, -0.03 * s)
    with tone(bm, GREY):
        prism(bm, pt(-1.45, -0.75, 0.72), pt(1.45, -0.75, 0.72), 0.13 * s, 0.13 * s, up=(0, 0, 1))   # rangefinder
        box(bm, pt(0, -1.0, 1.12), (0.5 * s, 0.4 * s, 0.14 * s))
    e = math.radians(elev)
    d = V((0, fwd * math.cos(e), math.sin(e)))
    out = []
    for dx in (-0.5, 0.0, 0.5):
        b0 = pt(dx, 0.9, 0.6)
        with tone(bm, GREY):
            cyl(bm, b0 - d * (0.4 * s), b0 + d * (0.75 * s), 0.21 * s, 0.16 * s, n=10)
            cyl(bm, b0, b0 + d * (3.4 * s), 0.1 * s, 0.085 * s, n=10)
        with tone(bm, FRAME):
            cyl(bm, b0 + d * (3.15 * s), b0 + d * (3.5 * s), 0.13 * s, n=10)
        out.append(b0 + d * (3.5 * s))
    return out


def aa(bm, p, d=(0, 1, 0), s=0.3, up=1):
    """Point-defence mount: tiny against the hull, there are dozens."""
    p = V(p)
    u = V((0, 0, up))
    d = V(d).normalized()
    with tone(bm, FRAME):
        cyl(bm, p - u * (0.1 * s), p + u * (0.3 * s), 0.55 * s, n=8)
    sphere(bm, p + u * (0.45 * s), 0.5 * s, seg=8, rings=4, scale=(1, 1.2, 0.8))
    with tone(bm, GREY):
        for o in (-0.2, 0.2):
            b0 = p + u * (0.5 * s) + d.cross(u) * (o * s)
            cyl(bm, b0, b0 + (d + u * 0.25).normalized() * (1.7 * s), 0.055 * s, n=5)


# ================================================================ the flagship

HS0 = [  # main hull: y, half beam, half depth, centre z (deck level is 2.6 everywhere)
    (28.8, 1.9, 1.9, 0.7),
    (27.0, 2.25, 2.1, 0.5),
    (25.0, 2.6, 2.3, 0.3),
    (21.0, 3.3, 2.7, -0.1),
    (15.0, 4.3, 3.0, -0.4),
    (8.0, 5.0, 3.2, -0.6),
    (0.0, 5.4, 3.3, -0.7),
    (-8.0, 5.4, 3.3, -0.7),
    (-15.0, 5.0, 3.2, -0.6),
    (-21.0, 4.4, 3.0, -0.4),
    (-25.0, 3.8, 2.8, -0.2),
]
YS = [28.8, 27, 25, 23, 21, 18, 15, 12, 10, 8, 4, 0, -4, -8, -10, -12, -15, -18, -21, -23, -25]
HK = dict(top=0.8, bot=0.35, hi=0.7, lo=-0.1, keel=0.25)
DECK = 2.6
LX = 8.3   # catapult leg centreline
LS = [(25.5, 1.45, 0.95, 0.3), (22.0, 1.8, 1.2, 0.2), (15.0, 1.9, 1.3, 0.2), (8.0, 1.9, 1.3, 0.2), (3.0, 1.7, 1.2, 0.2),
      (-1.5, 0.9, 0.7, 0.3)]
LK = dict(top=0.8, bot=0.75, hi=0.55, lo=-0.55)
BELTS = [(19.0, 14.6), (14.1, 9.6), (9.1, 4.6), (4.1, -0.4), (-0.9, -5.4), (-5.9, -10.4), (-10.9, -15.4), (-15.9, -20.4)]


def hs(y, i):
    return interp(HS0, y, i)


def hull_bottom(y):
    return hs(y, 3) - hs(y, 2)


def build(ship):
    AREA.clear()

    # ---------------------------------------------------------------- hull
    def hull(bm):
        rings = [ring10(y, hs(y, 1), hs(y, 2), hs(y, 3), **HK) for y in YS]
        res = loft(bm, rings, matfn=lambda i, k: L if (seg_sym(k) == 1 and 1 <= i <= 13 and i != 10) else H)
        paint(bm, pick(res, lambda i, k: seg_sym(k) in (3, 4)), RED)
        paint(bm, pick(res, lambda i, k: seg_sym(k) == 2 and i >= 4), FRAME)
        paint(bm, [res['start']], FRAME)
        inset(bm, pick(res, lambda i, k: seg_sym(k) == 2 and i in (1, 2, 3)), 0.12, -0.04)
        inset(bm, pick(res, lambda i, k: seg_sym(k) == 1 and i in (15, 17, 19)), 0.1, -0.035)
        inset(bm, pick(res, lambda i, k: seg_sym(k) == 1 and i in (10, 14, 18)), 0.16, -0.14, t=FRAME)
        inset(bm, pick(res, lambda i, k: seg_sym(k) == 3 and i % 2 == 0 and i > 2), 0.12, -0.04, t=RED)
        inset(bm, pick(res, lambda i, k: seg_sym(k) == 4 and i in (6, 9, 12, 15)), 0.2, -0.1, t=FRAME)
        inset(bm, [res['end']], 0.2, -0.3, t=FRAME)
    P(ship, hull, bevel=0.03)

    def belts(bm):
        # armour belt over the dark side strake, in plates with gaps
        for j, (y0, y1) in enumerate(BELTS):
            res = bs.side_plate(bm, HS0, y0, y1, -0.3, 1.58, 0.14, n=1, bevel_in=0.08)
            if j in (2, 5):
                paint(bm, pick(res, lambda i, k: True), GREY)
            if j in (0, 3, 7):
                for f in pick(res, lambda i, k: k == 1):
                    f.material_index = L
        # upper strake of sponsons along the deck edge
        for y0, y1 in ((11.5, 7.0), (-13.0, -17.5)):
            xs = [(y, hs(y, 1) * 0.8) for y in (y0, y1)]
            loft(bm, [[V((x - 0.2, y, DECK - 0.5)), V((x + 0.75, y, DECK - 0.35)), V((x + 0.75, y, DECK + 0.12)), V((x - 0.2, y, DECK + 0.12))]
                      for y, x in xs])
        with tone(bm, FRAME):
            # conduits in the deep hull trenches
            for i in (10, 14, 18):
                ya, yb = YS[i] - 0.3, YS[i + 1] + 0.3
                for o in (0.3, 0.55):
                    pa = V((hs(ya, 1) * (0.8 + 0.2 * o) - 0.1, ya, DECK - (DECK - hs(ya, 3) - hs(ya, 2) * 0.7) * o - 0.06))
                    pb = V((hs(yb, 1) * (0.8 + 0.2 * o) - 0.1, yb, DECK - (DECK - hs(yb, 3) - hs(yb, 2) * 0.7) * o - 0.06))
                    cyl(bm, pa, pb, 0.07, n=6)
            # manoeuvring verniers: bow and quarter clusters
            for y in (24.2, 23.6, -22.0, -22.6):
                for z in (0.9, 0.35):
                    vernier(bm, (hs(y, 1) - 0.02, y, z), (1, 0, 0), 0.2, 0.26)
            for y in (22.0, -19.0):
                for o in (0.0, 0.6):
                    vernier(bm, (hs(y, 1) * 0.3, y - o, hull_bottom(y - o) - 0.1), (0, 0, -1), 0.24, 0.3)
    P(ship, belts, bevel=0.02, segs=1, mirror=True)

    # ---------------------------------------------------------------- bow: cannon muzzle, blade prow, bulb
    BC = V((0, 28.0, 0.7))

    def bow(bm):
        lathe(bm, BC, (0, 1, 0), [(1.6, 0.0), (1.6, 1.0), (1.5, 1.15), (1.5, 1.45), (1.36, 1.75), (1.12, 1.75)],
              n=28, cap_start=False, cap_end=False)
        with tone(bm, YEL):
            lathe(bm, BC, (0, 1, 0), [(1.63, 0.86), (1.63, 0.98)], n=28, cap_start=False, cap_end=False)
        with tone(bm, FRAME):
            lathe(bm, BC, (0, 1, 0), [(1.12, 1.75), (1.04, 1.5), (1.04, -0.9), (0.62, -1.0)], n=28, cap_start=False, cap_end=False)
            lathe(bm, BC, (0, 1, 0), [(0.3, -1.0), (0.0, -0.7)], n=28, cap_start=False)
            for a in range(14):
                ang = a * math.pi / 7
                d = V((math.cos(ang), 0, math.sin(ang)))
                prism(bm, BC + V((0, -0.85, 0)) + d * 1.0, BC + V((0, 1.4, 0)) + d * 1.0, 0.05, 0.04, up=d)
        lathe(bm, BC, (0, 1, 0), [(0.62, -1.0), (0.3, -1.0)], n=28, mat=G, cap_start=False, cap_end=False)
        for t in (0.1, 0.75):   # charge rings in the bore
            lathe(bm, BC, (0, 1, 0), [(1.045, t), (1.0, t + 0.03), (1.0, t + 0.09), (1.045, t + 0.12)], n=28, mat=G, cap_start=False, cap_end=False)
        # blade prow over the muzzle
        loft(bm, [ring10(28.3, 1.75, 0.36, 2.42, top=0.7, bot=0.5, hi=0.4, lo=-0.5),
                  ring10(29.1, 1.0, 0.3, 2.56, top=0.65, bot=0.4, hi=0.4, lo=-0.5),
                  ring10(29.65, 0.4, 0.18, 2.74, top=0.6, bot=0.3, hi=0.4, lo=-0.5),
                  V((0, 30.0, 2.9))], matfn=lambda i, k: L if seg_sym(k) in (1, 2) else H)
        # bulb under it
        with tone(bm, RED):
            lathe(bm, (0, 27.2, -1.5), (0, 1, 0), [(0.8, 0.0), (0.95, 0.9), (0.9, 1.7), (0.6, 2.3), (0.0, 2.65)], n=16)
        for sx in (1, -1):
            with tone(bm, FRAME):   # anchor recesses and hawse pipes
                cyl(bm, (sx * hs(26.0, 1) - sx * 0.1, 26.0, 1.2), (sx * hs(26.0, 1) + sx * 0.06, 26.0, 1.2), 0.3, n=10)
                prism(bm, (sx * 0.9, 27.6, DECK + 0.02), (sx * 0.55, 23.4, DECK + 0.02), 0.03, 0.03)
                cyl(bm, (sx * 0.9, 27.6, DECK), (sx * 0.9, 27.6, DECK + 0.14), 0.12, n=8)
            with tone(bm, GREY):   # capstans, breakwater, hatches
                cyl(bm, (sx * 0.55, 23.4, DECK), (sx * 0.55, 23.4, DECK + 0.2), 0.2, n=10)
                prism(bm, (0, 23.0, DECK + 0.12), (sx * 2.2, 22.1, DECK + 0.12), 0.04, 0.14)
                for y in (25.6, 24.6):
                    box(bm, (sx * 0.7, y, DECK + 0.03), (0.5, 0.5, 0.07))
                for y in (-22.6, -23.6, -24.4):
                    box(bm, (sx * 1.4, y, DECK + 0.03), (0.7, 0.55, 0.07))
    P(ship, bow, bevel=0.02, segs=1)

    def bow_planes(bm):
        res = wing(bm, wst(dict(x=2.5, z=0.3, le=24.6, te=21.6, t=0.3), dict(x=5.2, z=0.3, le=22.4, te=21.0, t=0.1), 3),
                   matfn=lambda i, k: L if i >= 1 else H)
        bs.light(bm, (5.25, 21.7, 0.3), 0.07)
    P(ship, bow_planes, bevel=0.015, mirror=True)

    # ---------------------------------------------------------------- superstructure tiers
    T1 = (9.0, -15.0, 3.3, DECK, 3.7)
    T2 = (6.5, -12.5, 2.5, 3.7, 4.7)
    T3 = (3.5, -5.5, 1.8, 4.7, 5.7)
    BH = (17.2, 12.2, 1.9, DECK, 3.5)

    def tiers(bm):
        for (y0, y1, hw, z0, z1), ch, liv in ((T1, 0.6, False), (T2, 0.5, False), (T3, 0.4, True), (BH, 0.5, True)):
            with tone(bm, FRAME):   # each tier floats on a dark plinth
                house(bm, y0 - 0.1, y1 + 0.1, hw - 0.1, z0 - 0.05, z0 + 0.2, ch=ch, rf=0, rb=0)
            res = house(bm, y0, y1, hw, z0 + 0.16, z1, ch=ch, rf=0.25, mat=L if liv else H)
            for f in (res['start'], res['end']):
                f.material_index = H
            inset(bm, [res['end']], 0.14, -0.03, t=GREY)
        with tone(bm, FRAME):
            # recessed galleries between the tiers
            for y0, y1, hw, z in ((8.2, -14.2, 3.33, 3.18), (5.8, -11.8, 2.53, 4.22)):
                for sx in (1, -1):
                    box(bm, (sx * hw, (y0 + y1) / 2, z), (0.04, y0 - y1, 0.05))
    P(ship, tiers, bevel=0.025)

    # ---------------------------------------------------------------- pagoda bridge
    TW = [  # y0, y1, hw, z0, z1, ins, rf
        (2.4, -2.6, 1.3, 5.7, 7.2, 0.15, 0.25),
        (2.9, -2.5, 1.65, 7.2, 7.34, 0.0, 0.0),
        (2.0, -2.1, 1.0, 7.34, 8.5, 0.1, 0.2),
        (2.5, -2.0, 1.4, 8.5, 8.62, 0.0, 0.0),
        (1.8, -1.8, 0.85, 8.62, 9.6, 0.08, 0.15),
        (2.35, -1.7, 1.25, 9.6, 9.7, 0.0, 0.0),
        (2.2, -1.6, 1.1, 9.7, 10.45, 0.0, -0.3),
        (1.3, -1.4, 0.7, 10.45, 11.0, 0.08, 0.15),
    ]
    BZ, TZ = 9.7, 11.0

    def tower(bm):
        for j, (y0, y1, hw, z0, z1, ins, rf) in enumerate(TW):
            with tone(bm, GREY if j in (1, 3, 5) else 0):
                res = house(bm, y0, y1, hw, z0, z1, ch=0.3 if hw < 1.2 else 0.4, rf=rf, rb=0.1, ins=ins)
            if j in (1, 3, 5):   # platform wings: searchlight tubs
                for sx in (1, -1):
                    with tone(bm, GREY):
                        cyl(bm, (sx * (hw + 0.1), -0.3, z0), (sx * (hw + 0.1), -0.3, z1 + 0.12), 0.32, n=10)
        # funnel, raked aft, blue with a dark cap
        res = house(bm, -6.5, -9.0, 0.9, 4.7, 7.7, ch=0.3, rf=1.3, rb=-1.0, ins=0.18, mat=L)
        res['end'].material_index = H
        inset(bm, [res['end']], 0.1, -0.35, t=FRAME)
        with tone(bm, GREY):
            house(bm, -7.72, -10.08, 0.78, 7.45, 7.62, ch=0.25, rf=0.05, rb=-0.05)
        with tone(bm, FRAME):
            house(bm, -6.3, -9.2, 1.1, 4.68, 4.95, ch=0.3, rf=0, rb=0)
        # aft control tower
        house(bm, -10.7, -12.3, 1.0, 4.7, 6.3, ch=0.25, ins=0.1)
        with tone(bm, GREY):
            house(bm, -10.5, -12.4, 1.2, 6.3, 6.42, ch=0.3, rf=0, rb=0)
        house(bm, -10.9, -12.1, 0.7, 6.42, 7.0, ch=0.2, ins=0.05)
        # ventral bridge on its fin
        with tone(bm, RED):
            house(bm, 1.6, -4.6, 0.75, -3.75, -4.5, ch=0.3, rf=0.7, rb=0.5, ins=0.25)
        house(bm, 0.6, -3.8, 0.7, -4.45, -5.05, ch=0.3, rf=-0.3, rb=0.1, ins=0.0)
    P(ship, tower, bevel=0.02)

    def tower_bits(bm):
        with tone(bm, GREY):
            prism(bm, (-2.6, 0.3, TZ - 0.28), (2.6, 0.3, TZ - 0.28), 0.13, 0.13)   # rangefinder arms
            for sx in (1, -1):
                box(bm, (sx * 2.6, 0.3, TZ - 0.28), (0.3, 0.5, 0.36))
            box(bm, (0, -0.7, TZ + 0.5), (1.5, 0.08, 0.7))                  # array panel
            box(bm, (0, -0.7, TZ + 0.1), (0.2, 0.2, 0.3))
            # dish on the aft tower
            lathe(bm, (0, -11.5, 7.35), (0, -0.5, 1), [(0.0, 0.0), (0.5, 0.1), (0.7, 0.28), (0.66, 0.3), (0.45, 0.14), (0.0, 0.06)], n=14)
            cyl(bm, (0, -11.5, 6.95), (0, -11.5, 7.4), 0.07, n=6)
            # secondary directors on the tower shoulders
            for sx in (1, -1):
                for y, z in ((1.2, 7.34), (-1.6, 7.34)):
                    sphere(bm, (sx * 1.3, y, z + 0.25), 0.26, seg=10, rings=5)
        with tone(bm, FRAME):
            cyl(bm, (0, -0.9, TZ), (0, -1.1, TZ + 1.6), 0.07, 0.03, n=6)   # main mast
            for z, w in ((TZ + 0.75, 0.9), (TZ + 1.2, 0.55)):
                prism(bm, (-w, -0.9 - (z - TZ) * 0.125, z), (w, -0.9 - (z - TZ) * 0.125, z), 0.025, 0.025)
            bs.antenna(bm, (0.5, -1.2, TZ), 1.0, r=0.02, tip=False)
            bs.antenna(bm, (-0.5, -1.2, TZ), 1.0, r=0.02, tip=False)
            bs.antenna(bm, (0, -9.4, 7.6), 1.6, r=0.025)
            bs.antenna(bm, (0, -11.9, 7.0), 1.3, r=0.02, tip=False)
            for sx in (1, -1):   # tower braces
                prism(bm, (sx * 1.6, -1.0, 5.7), (sx * 0.95, -1.0, 7.3), 0.05, 0.12)
                prism(bm, (sx * 1.25, -0.6, 7.34), (sx * 0.9, -0.6, 8.5), 0.04, 0.1)
                prism(bm, (sx * 1.05, -0.5, 8.62), (sx * 0.8, -0.5, 9.6), 0.035, 0.09)
        bs.light(bm, (0, -1.1, TZ + 1.62), 0.06)
        with tone(bm, YEL):
            for sx in (1, -1):
                wing(bm, [dict(x=sx * 0.1, z=TZ - 0.1, le=1.45, te=0.85, t=0.12),
                          dict(x=sx * 0.9, z=TZ + 0.5, le=0.95, te=0.6, t=0.08),
                          dict(x=sx * 1.9, z=TZ + 1.2, le=0.3, te=0.15, t=0.03)], flat_te=0.3)
        with tone(bm, RED):
            box(bm, (0, 1.2, TZ + 0.06), (0.3, 0.5, 0.14))
    P(ship, tower_bits, bevel=0.008, segs=1)

    # ---------------------------------------------------------------- main and secondary batteries
    guns = []

    def batteries(bm):
        guns.extend(turret(bm, (0, 20.6, DECK), 1))
        guns.extend(turret(bm, (0, 14.7, 3.5), 1))
        guns.extend(turret(bm, (0, -19.6, DECK), -1))
        for sx in (1, -1):
            turret(bm, (sx * 5.6, 8.2, 1.0), 1, s=0.5)
            turret(bm, (sx * 2.9, -16.9, DECK), -1, s=0.5)
            turret(bm, (sx * 1.65, 7.8, 3.7), 1, s=0.5)
            turret(bm, (sx * 1.65, -13.8, 3.7), -1, s=0.5)
    P(ship, batteries, bevel=0.015, segs=1)
    for i, g in enumerate(guns):
        ship.empty("Gun_%d" % i, g, 0.2)

    def flak(bm):
        for i in range(10):
            y = 7.6 - i * 2.4
            aa(bm, (3.78, y, DECK), (1, 0.3 if i < 5 else -0.3, 0))
        for i in range(6):
            y = 5.0 - i * 3.3
            aa(bm, (2.92, y, 3.7), (1, 0.3 if i < 3 else -0.3, 0))
        for y in (2.6, -0.4, -3.4):
            aa(bm, (2.15, y, 4.7), (1, 0, 0), s=0.26)
        for y in (23.5, 17.9, -16.6, -22.3):
            aa(bm, (hs(y, 1) * 0.8 - 0.45, y, DECK), (1, 0.4 if y > 0 else -0.4, 0))
        for y in (14.0, 8.0, 2.0, -9.0, -15.0):
            aa(bm, (1.25, y, hull_bottom(y) + 0.02), (1, 0, 0), up=-1)
        for y in (10.0, 4.5):
            aa(bm, (5.4, y, 0.9), (0, 1, 0), s=0.26)
        with tone(bm, GREY):
            # launch cells between B barbette and the citadel
            for ix in range(4):
                for iy in range(5):
                    box(bm, (0.45 + ix * 0.5, 11.7 - iy * 0.5, DECK + 0.03), (0.36, 0.36, 0.08))
            for y in (-6.2, -7.6, -9.0):
                box(bm, (1.8, y, 4.8), (0.5, 0.8, 0.2))
            sphere(bm, (1.75, -11.5, 4.78), 0.3, seg=10, rings=5)
            sphere(bm, (1.5, 4.6, 4.78), 0.3, seg=10, rings=5)
            for y in (-24.9, -25.9, -26.9):
                sphere(bm, (2.95, y, 3.02), 0.33, seg=10, rings=5)
            # boats and lockers along tier 1
            for y in (-1.0, -4.0, -7.0):
                box(bm, (2.92, y + 1.5, 3.78), (0.3, 0.9, 0.16))
    P(ship, flak, bevel=0.0, mirror=True)

    # ---------------------------------------------------------------- catapult legs
    LYS = [25.5, 24.0, 22.0, 20.0, 18.0, 16.0, 14.0, 12.0, 10.0, 8.0, 6.0, 3.0, -1.5]
    BAYS = (4, 7)   # leg bands with a side hangar bay

    def leg(bm):
        rings = [ring10(y, interp(LS, y, 1), interp(LS, y, 2), interp(LS, y, 3), LX, **LK) for y in LYS]
        res = loft(bm, rings, matfn=lambda i, k: L if ((2 <= i <= 9 and seg_sym(k) == 2 and i not in BAYS) or
                                                       (i <= 1 and seg_sym(k) in (0, 1, 2))) else H)
        paint(bm, pick(res, lambda i, k: seg_sym(k) == 4), RED)
        paint(bm, pick(res, lambda i, k: seg_sym(k) == 3), FRAME)
        inset(bm, [res['start']], 0.14, -2.6, t=GREY)
        inset(bm, pick(res, lambda i, k: 2 <= i <= 8 and seg_sym(k) == 0), 0.14, -0.04, t=GREY)
        inset(bm, pick(res, lambda i, k: i in BAYS and k == 2), 0.16, -0.7, t=GREY)        # outboard bays
        inset(bm, pick(res, lambda i, k: i in (3, 5, 6, 8) and seg_sym(k) == 1), 0.1, -0.035)
        inset(bm, pick(res, lambda i, k: i in (9, 10) and seg_sym(k) in (1, 2)), 0.1, -0.035)
        inset(bm, pick(res, lambda i, k: i in (2, 5, 8) and seg_sym(k) == 3), 0.12, -0.1, t=FRAME)
        # lower armour skirts over the dark chine
        for y0, y1 in ((21.5, 17.2), (16.8, 12.2), (11.8, 7.2), (6.8, 3.4)):
            loft(bm, [[V((LX + interp(LS, y, 1) + 0.02, y, -0.35)), V((LX + interp(LS, y, 1) + 0.14, y, -0.42)),
                       V((LX + interp(LS, y, 1) * 0.86, y, -1.02)), V((LX + interp(LS, y, 1) * 0.8, y, -0.95))] for y in (y0, y1)])
        # sponson to the hull
        with tone(bm, GREY):
            r2 = slab(bm, [(3.8, 12.5), (7.0, 11.0), (7.0, 1.0), (3.8, -1.5)], lambda x, y: -0.65, 1.5, chamf=0.1)
            inset(bm, [r2['end']], 0.25, -0.06, t=FRAME)
        slab(bm, [(4.6, 10.6), (6.6, 9.8), (6.6, 6.2), (4.6, 6.2)], lambda x, y: 0.7, 0.3, chamf=0.06, mat=L)
        with tone(bm, RED):
            slab(bm, [(4.0, 11.5), (6.9, 10.2), (6.9, 1.8), (4.0, -0.4)], lambda x, y: -0.95, 0.35, chamf=0.02)
    P(ship, leg, bevel=0.025, mirror=True)

    def leg_bits(bm):
        zt = lambda y: interp(LS, y, 3) + interp(LS, y, 2)
        with tone(bm, YEL):
            box(bm, (LX, 25.5, 1.2), (2.1, 0.1, 0.07))
            box(bm, (LX, 24.2, -0.5), (0.1, 2.4, 0.03))
            for y in (21.6, 8.5):
                box(bm, (LX, y, zt(y) - 0.03), (2.2, 0.12, 0.03))
        with tone(bm, FRAME):
            # catapult rails in the track and the shuttle
            for o in (-0.25, 0.25):
                prism(bm, (LX + o, 21.8, zt(21.8) - 0.03), (LX + o, 15.0, zt(15) - 0.03), 0.04, 0.03)
                prism(bm, (LX + o, 15.0, zt(15) - 0.03), (LX + o, 8.3, zt(8.3) - 0.03), 0.04, 0.03)
            box(bm, (LX, 12.0, 1.5), (0.7, 0.5, 0.1))
            # blast deflector, raised
            box(bm, (LX, 7.6, 1.72), (2.0, 0.08, 0.5), rot=Matrix.Rotation(math.radians(-25), 3, 'X'))
            bs.vent(bm, (5.6, 4.6, 0.82), 1.2, 2.2, n=8, h=0.07)
            for y in (20.0, 13.0, 6.0):
                vernier(bm, (LX + interp(LS, y, 1) - 0.02, y, -0.3), (1, 0, -0.2), 0.16, 0.2)
        # leg control house
        house(bm, 6.6, 4.4, 0.7, 1.45, 2.15, ch=0.2, x=LX)
        with tone(bm, GREY):
            house(bm, 6.75, 4.3, 0.85, 2.15, 2.24, ch=0.22, x=LX, rf=0, rb=0)
        bs.antenna(bm, (LX, 5.0, 2.24), 1.0, r=0.02)
        aa(bm, (LX + 0.9, 3.0, zt(3.0)), (1, 0.2, 0), s=0.28)
        aa(bm, (LX - 0.9, 3.0, zt(3.0)), (-1, 0.2, 0), s=0.28)
        aa(bm, (LX, 0.5, zt(0.5)), (1, -0.3, 0), s=0.28)
        bs.light(bm, (LX + 1.2, 25.45, 0.3), 0.07)
        with tone(bm, FRAME):
            cyl(bm, (LX, -1.3, 0.3), (LX, -2.0, 0.3), 0.6, 0.5, n=12)
            vernier(bm, (LX, -2.0, 0.3), (0, -1, 0), 0.4, 0.4, hot=True)
        with tone(bm, YEL):
            box(bm, (LX, 2.2, zt(2.2) + 0.0), (2.0, 0.12, 0.05))
    P(ship, leg_bits, bevel=0.0, mirror=True)

    # ---------------------------------------------------------------- radiator wings and tail fins
    def wings(bm):
        a = dict(x=4.6, z=0.3, le=-10.5, te=-21.5, t=0.95)
        b = dict(x=12.3, z=-1.3, le=-18.8, te=-23.4, t=0.3)
        res = wing(bm, wst(a, b, 5), matfn=lambda i, k: L if i >= 1 else H)
        inset(bm, pick(res, lambda i, k: i in (0, 1, 2) and k in (2, 4)), 0.12, -0.05, t=FRAME)
        inset(bm, pick(res, lambda i, k: i in (1, 2) and k in (1, 5)), 0.1, -0.03)
        with tone(bm, FRAME):
            for j in range(11):
                fr = 0.27 + j * 0.04
                pa = V((a['x'] + 0.4 + 0.2 * 0.44, a['le'] - fr * (a['le'] - a['te']) - 0.3, a['z'] + 0.98 * 0.44 - 0.07))
                pb = V((b['x'] - 0.6 + 0.2 * 0.14, b['le'] - fr * (b['le'] - b['te']), b['z'] + 0.98 * 0.14 + 0.1))
                prism(bm, pa, pb, 0.05, 0.03, up=(0.2, 0, 0.98))
        lathe(bm, (12.35, -24.2, -1.32), (0, 1, 0), [(0.2, 0.0), (0.38, 0.4), (0.42, 1.2), (0.42, 4.4), (0.3, 5.6), (0.0, 6.6)], n=12,
              mats=[H, H, H, L, H])
        with tone(bm, FRAME):
            vernier(bm, (12.35, -24.2, -1.32), (0, -1, 0), 0.24, 0.3)
        bs.light(bm, (12.35, -17.55, -1.32), 0.08)
        # tail fins, canted
        a = dict(x=2.7, z=2.5, le=-20.5, te=-27.2, t=0.55)
        b = dict(x=5.4, z=6.8, le=-25.4, te=-28.4, t=0.2)
        res = wing(bm, wst(a, b, 4), matfn=lambda i, k: L if i <= 2 else H)
        paint(bm, res['bands'][3], GREY)
        inset(bm, pick(res, lambda i, k: i in (0, 1) and k in (2, 4)), 0.1, -0.03)
        bs.light(bm, (5.43, -28.4, 6.83), 0.07)
        # ventral stabilisers
        a = dict(x=3.0, z=-3.0, le=-21.0, te=-27.0, t=0.45)
        b = dict(x=4.4, z=-5.0, le=-24.8, te=-27.8, t=0.16)
        res = wing(bm, wst(a, b, 2), mat=L)
    P(ship, wings, bevel=0.02, mirror=True)

    # ---------------------------------------------------------------- engine block
    ES = [(-23.4, 4.2, 3.0, -0.3), (-24.6, 4.75, 3.3, -0.3), (-27.6, 4.6, 3.2, -0.3)]
    ENG = [((0, -30.0, -0.3), 2.1, 3.6), ((3.15, -29.3, 1.55), 0.95, 2.6), ((-3.15, -29.3, 1.55), 0.95, 2.6),
           ((3.15, -29.3, -2.15), 0.95, 2.6), ((-3.15, -29.3, -2.15), 0.95, 2.6)]

    def block(bm):
        res = loft(bm, [ring10(y, hw, hh, cz, top=0.72, bot=0.72, hi=0.62, lo=-0.62) for y, hw, hh, cz in ES],
                   matfn=lambda i, k: L if seg_sym(k) in (1, 2) else H)
        paint(bm, pick(res, lambda i, k: seg_sym(k) in (3, 4)), RED)
        inset(bm, [res['end']], 0.22, -0.45, t=FRAME)
        inset(bm, pick(res, lambda i, k: i == 1 and seg_sym(k) == 0), 0.2, -0.1, t=FRAME)
        for c, r, ln in ENG:
            engine(bm, c, r, ln, petals=20 if r > 1.5 else 12, n=32 if r > 1.5 else 20)
        with tone(bm, FRAME):
            for sx in (1, -1):
                bs.vent(bm, (sx * 1.6, -26.0, 2.82), 1.6, 2.0, n=8, h=0.08)
            for a in range(8):
                ang = a * math.pi / 4 + math.pi / 8
                d = V((math.cos(ang), 0, math.sin(ang)))
                cyl(bm, V((0, -27.2, -0.3)) + d * 2.95, V((0, -28.3, -0.3)) + d * 2.75, 0.09, n=6)
    P(ship, block, bevel=0.02, segs=1)
    for i, (c, r, ln) in enumerate(ENG):
        ship.empty("Engine_%d" % i, c, r)

    # ---------------------------------------------------------------- lit windows and deck lights
    def lights(bm):
        for j, (y0, y1) in enumerate(BELTS):
            for sx in (1, -1):
                for z in (1.2, 0.75):
                    win_row(bm, (sx * (hs(y0, 1) + 0.16), y0 - 0.35, z), (sx * (hs(y1, 1) + 0.16), y1 + 0.35, z), (sx, 0, 0),
                            seed=j * 3 + int(z * 10) + (sx > 0), skip=3 if z < 0.5 else 4)
        for i in range(1, 10):   # portholes along the blue sheer strake
            ya, yb_ = YS[i] - 0.3, YS[i + 1] + 0.3
            for sx in (1, -1):
                pa = V((sx * 0.9 * hs(ya, 1), ya, (DECK + hs(ya, 3) + 0.7 * hs(ya, 2)) / 2))
                pb = V((sx * 0.9 * hs(yb_, 1), yb_, (DECK + hs(yb_, 3) + 0.7 * hs(yb_, 2)) / 2))
                out = V((sx * (DECK - hs(ya, 3) - 0.7 * hs(ya, 2)), 0, 0.2 * hs(ya, 1)))
                win_row(bm, pa, pb, out, pitch=0.4, w=0.1, h=0.06, skip=3, seed=i + (sx > 0))
        house_windows(bm, T1[0], T1[1], T1[2], (2.95, 3.38), ch=0.6, seed=1)
        house_windows(bm, T2[0], T2[1], T2[2], (4.0, 4.4), ch=0.5, seed=2)
        house_windows(bm, T3[0], T3[1], T3[2], (5.0, 5.38), ch=0.4, seed=3)
        house_windows(bm, BH[0], BH[1], BH[2], (3.05,), ch=0.5, seed=4)
        for z in (6.1, 6.5, 6.9):
            house_windows(bm, 2.1, -2.3, 1.3 - 0.15 * (z - 5.7) / 1.5, (z,), ch=0.3, seed=int(z * 10))
        for z in (7.7, 8.1):
            house_windows(bm, 1.8, -1.9, 1.0 - 0.1 * (z - 7.34) / 1.16, (z,), ch=0.3, seed=int(z * 10))
        for z in (8.95, 9.3):
            house_windows(bm, 1.6, -1.6, 0.85 - 0.08 * (z - 8.62) / 0.98, (z,), ch=0.3, seed=int(z * 10))
        # the bridge: one unbroken band of glass
        yb = 2.2 + 0.3 * 0.5
        bs.strip(bm, (-0.75, yb + 0.02, BZ + 0.4), (0.75, yb + 0.02, BZ + 0.4), 0.03, 0.09)
        for sx in (1, -1):
            bs.strip(bm, (sx * 1.1, 1.75, BZ + 0.4), (sx * 1.1, -0.6, BZ + 0.4), 0.03, 0.09)
        win_row(bm, (-0.4, 1.3 - 0.07, TZ - 0.3), (0.4, 1.3 - 0.07, TZ - 0.3), (0, 1, 0), skip=0)
        house_windows(bm, -10.7, -12.3, 0.95, (5.3, 5.8), ch=0.25, seed=7)
        house_windows(bm, -10.9, -12.1, 0.7, (6.7,), ch=0.2, seed=8)
        for z in (5.4, 6.0, 6.6):
            t = (z - 4.7) / 3.0
            house_windows(bm, -6.5 - 1.3 * t - 0.1, -9.0 - 1.0 * t + 0.1, 0.9 - 0.18 * t, (z,), ch=0.3, seed=int(z * 10))
        # ventral bridge
        bs.strip(bm, (-0.5, 0.6 + 0.15 + 0.02, -4.75), (0.5, 0.6 + 0.15 + 0.02, -4.75), 0.03, 0.07)
        house_windows(bm, 0.4, -3.6, 0.7, (-4.75,), ch=0.3, seed=9)
        for sx in (1, -1):
            x = sx * LX
            # hangar interior: ceiling strips, lit back wall
            for o in (-0.6, 0.6):
                bs.strip(bm, (x + o, 25.2, 1.09), (x + o, 23.1, 1.09), 0.05, 0.02)
            box(bm, (x, 22.93, 0.45), (2.2, 0.04, 0.7), mat=G)
            for o in (-0.8, 0.8):
                win_row(bm, (x + o, 25.3, -0.5), (x + o, 23.0, -0.5), (0, 0, 1), pitch=0.3, w=0.14, h=0.08, skip=0)
            for z in (-0.1, 0.15):
                win_row(bm, (x - 1.0, 22.93, z), (x + 1.0, 22.93, z), (0, 1, 0), pitch=0.3, w=0.16, h=0.1, skip=5, seed=int(z * 10))
            # outboard bays: a lit strip under each lintel
            for i in BAYS:
                ya, yb_ = LYS[i] - 0.3, LYS[i + 1] + 0.3
                xa = x + sx * (interp(LS, (ya + yb_) / 2, 1) - 0.66)
                box(bm, (xa, (ya + yb_) / 2, 0.55), (0.04, ya - yb_, 0.12), mat=G)
                win_row(bm, (xa, ya, 0.1), (xa, yb_, 0.1), (sx, 0, 0), skip=0, pitch=0.35)
            # catapult guide lights
            for o in (-0.8, 0.8):
                for ya, yb_ in ((21.6, 15.0), (15.0, 8.6)):
                    za = interp(LS, ya, 3) + interp(LS, ya, 2) - 0.04
                    zb = interp(LS, yb_, 3) + interp(LS, yb_, 2) - 0.04
                    win_row(bm, (x + o, ya, za), (x + o, yb_, zb), (0, 0, 1), pitch=0.7, w=0.14, h=0.07, skip=0)
            for ya, yb_ in ((21.8, 18.2), (15.8, 15.1), (14.9, 12.2), (9.8, 8.2)):
                for z in (0.6, 0.3):
                    for o in (1, -1):
                        win_row(bm, (x + o * interp(LS, ya, 1), ya, z), (x + o * interp(LS, yb_, 1), yb_, z), (o, 0, 0),
                                seed=int(ya) + int(z * 10) + o)
            # leg side galleries
            for z in (0.55, 0.15):
                win_row(bm, (x + sx * interp(LS, 7.6, 1), 7.6, z), (x + sx * interp(LS, 3.4, 1), 3.4, z), (sx, 0, 0), seed=int(z * 10))
            house_windows(bm, 6.6, 4.4, 0.7, (1.85,), ch=0.2, x=x, front=True, seed=5)
            # engine block galleries
            for z in (1.0, 0.5):
                win_row(bm, (sx * 4.72, -24.9, z), (sx * 4.62, -27.3, z), (sx, 0, 0), seed=int(z * 10))
    P(ship, lights, bevel=0.0)

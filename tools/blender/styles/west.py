"""`west`: a ~420 m battlecruiser as a Western AAA hard-surface design (Battlestar Galactica, UNSC cruisers,
the Donnager, Star Citizen Idris, Hiigaran battlecruiser). Modelled 60 units long, 1 unit ~ 7 m.

An armoured wedge bow, a pinched neck with exposed structure, a blocky citadel under three superstructure tiers
and a bridge tower, ribbed flight pods on docking pylons with lit hangar mouths, a ribbed engine block with four
main and two sponson nozzles, radiator panels, five twin main-battery turrets, secondaries, point defence and
rows of lit windows. Faces are tagged in a corner colour attribute (`wmask`) that the Hull material reads:
R hazard stripes, G bare heat-stained steel, B matt black, A grey-white stencil paint -- all baked into Hull.
"""
import math
import random

from mathutils import Matrix

import shiplib
from shiplib import V, H, L, G, GL, E, loft, ring10, circle_ring, lathe, box, prism, cyl, sphere, inset_faces, log

STYLE = dict(paint=(0.1, 0.108, 0.1), paint2=(0.045, 0.05, 0.048), accent=(0.185, 0.19, 0.165), accent_thresh=0.78,
             metal=0.5, rough=0.54, spacing=(1.0, 1.35, 0.8), wear=1.5, grime=1.25, sub_amt=0.5, sub_scale=0.55,
             seam_w=0.045, grime_col=(0.03, 0.027, 0.024), bump=1.2, bare=(0.46, 0.45, 0.43))

TINT = dict(livery=(0.5, 0.055, 0.03), livery_metal=0.25, livery_rough=0.5, glow=(1.0, 0.82, 0.56),
            engine=(1.0, 0.5, 0.2), glass=(0.11, 0.065, 0.02))

MASK = "wmask"
HZ, BM, DK, WT = (1, 0, 0, 0), (0, 1, 0, 0), (0, 0, 1, 0), (0, 0, 0, 1)
_TAGS = {}
AREA = [0.0] * 5
TRIS = []


# ================================================================ material

def materials():
    mats, socks = shiplib.make_materials(STYLE)
    hull = mats["Hull"]
    nb = shiplib.NB(hull)
    nt = hull.node_tree
    fin = socks['base'].node                       # wear over (seams over (grime over base))
    grime_mix = fin.inputs[6].links[0].from_node.inputs[6].links[0].from_node
    base = grime_mix.inputs[6].links[0].from_socket
    pos = next(n for n in nt.nodes if n.type == 'TEX_COORD').outputs['Object']

    geo = next(n for n in nt.nodes if n.type == 'NEW_GEOMETRY')
    for n in nt.nodes:
        if n.type == 'VECT_MATH' and n.operation == 'DOT_PRODUCT':
            for lk in list(n.inputs[1].links):
                nt.links.remove(lk)
            nb.link(geo.outputs['Normal'], n.inputs[1])

    att = nb.node('ShaderNodeAttribute')
    att.attribute_name = MASK
    sep = nb.node('ShaderNodeSeparateColor')
    nb.link(att.outputs['Color'], sep.inputs[0])
    hz, bm_, dk = sep.outputs[0], sep.outputs[1], sep.outputs[2]
    wt = att.outputs['Alpha']

    xyz = nb.node('ShaderNodeSeparateXYZ')
    nb.link(pos, xyz.inputs[0])
    d = nb.math('ADD', nb.math('ABSOLUTE', xyz.outputs[0]), nb.math('ADD', xyz.outputs[1], xyz.outputs[2]))
    stripe = nb.math('GREATER_THAN', nb.math('FRACT', nb.math('MULTIPLY', d, 2.2)), 0.5)
    hazard = nb.mix(stripe, (0.62, 0.36, 0.01, 1), (0.012, 0.012, 0.012, 1))

    heat = nb.node('ShaderNodeTexNoise')
    nb.link(pos, heat.inputs['Vector'])
    heat.inputs['Scale'].default_value = 0.9
    heat.inputs['Detail'].default_value = 5
    steel = nb.mix(nb.mrange(heat.outputs['Fac'], 0.42, 0.62, smooth=True), (0.05, 0.052, 0.058, 1), (0.15, 0.095, 0.05, 1))
    steel = nb.mix(nb.mrange(heat.outputs['Fac'], 0.6, 0.75, smooth=True), steel, (0.05, 0.07, 0.12, 1))

    col = nb.mix(bm_, base, steel)
    col = nb.mix(dk, col, (0.016, 0.017, 0.019, 1))
    col = nb.mix(wt, col, (0.4, 0.4, 0.37, 1))
    col = nb.mix(hz, col, hazard)
    for lk in list(grime_mix.inputs[6].links):
        nt.links.remove(lk)
    nb.link(col, grime_mix.inputs[6])

    rough = nb.mixf(bm_, socks['rough'], 0.36)
    rough = nb.mixf(dk, rough, 0.78)
    metal = nb.mixf(bm_, socks['metal'], 0.95)
    metal = nb.mixf(nb.math('MAXIMUM', dk, nb.math('MAXIMUM', wt, hz)), metal, 0.12)
    bsdf = socks['bsdf']
    for name, s in (('Roughness', rough), ('Metallic', metal)):
        for lk in list(bsdf.inputs[name].links):
            nt.links.remove(lk)
        nb.link(s, bsdf.inputs[name])
    socks['rough'], socks['metal'] = rough, metal
    return mats, socks


# ================================================================ helpers

def lerp(a, b, t):
    return a + (b - a) * t


def interp(st, y, idx):
    st = sorted(st, key=lambda s: s[0])
    if y <= st[0][0]:
        return st[0][idx]
    for a, b in zip(st, st[1:]):
        if a[0] <= y <= b[0]:
            t = (y - a[0]) / (b[0] - a[0]) if b[0] != a[0] else 0
            return lerp(a[idx], b[idx], t)
    return st[-1][idx]


class tag:
    """Faces made inside the block get the mask colour."""

    def __init__(self, bm, col):
        self.bm, self.col = bm, col

    def __enter__(self):
        self.n = len(self.bm.faces)

    def __exit__(self, *a):
        if self.col:
            for f in list(self.bm.faces)[self.n:]:
                _TAGS[f] = self.col


def paint(faces, col):
    for f in faces:
        if f.is_valid:
            _TAGS[f] = col


def P(ship, fn, **kw):
    """ship.part that also writes the mask attribute and adds up the area per material."""
    def wrapped(bm):
        _TAGS.clear()
        bm.loops.layers.float_color.new(MASK)
        fn(bm)
        lay = bm.loops.layers.float_color[MASK]
        for f in bm.faces:
            c = _TAGS.get(f, (0, 0, 0, 0))
            for lp in f.loops:
                lp[lay] = c
        _TAGS.clear()
    o = ship.part(wrapped, **kw)
    for p in o.data.polygons:
        AREA[p.material_index] += p.area
    TRIS.append((fn.__name__, sum(len(p.vertices) - 2 for p in o.data.polygons)))
    return o


def D(ship, fn, mirror=False):
    """Small stuff: no bevel."""
    return P(ship, fn, bevel=0.0, mirror=mirror, weighted=False)


class Body:
    """A ring10 loft described by stations (y, hw, hh, cz): points and normals on its skin."""

    def __init__(self, st, cx=0.0, **kw):
        self.st, self.cx, self.kw = st, cx, kw

    def dims(self, y):
        return interp(self.st, y, 1), interp(self.st, y, 2), interp(self.st, y, 3)

    def ring(self, y, grow=0.0):
        hw, hh, cz = self.dims(y)
        return ring10(y, hw + grow, hh + grow, cz, cx=self.cx, **self.kw)

    def rings(self):
        return [self.ring(s[0]) for s in self.st]

    def at(self, y, f, grow=0.0):
        pts = self.ring(y, grow)
        i = int(math.floor(f)) % 10
        return pts[i].lerp(pts[(i + 1) % 10], f - math.floor(f))

    def surf(self, y, f, grow=0.0):
        pts = self.ring(y, grow)
        i = int(math.floor(f)) % 10
        a, b = pts[i], pts[(i + 1) % 10]
        p = a.lerp(b, f - math.floor(f))
        t = (b - a).normalized()
        n = V((t.z, 0, -t.x))
        if n.dot(p - V((self.cx, y, self.dims(y)[2]))) < 0:
            n = -n
        return p, n

    def top(self, y):
        hw, hh, cz = self.dims(y)
        return cz + hh

    def bot(self, y):
        hw, hh, cz = self.dims(y)
        return cz - hh

    def ys(self, y0, y1):
        return [y0] + [s[0] for s in sorted(self.st, key=lambda s: -s[0]) if y1 < s[0] < y0] + [y1]


def frange(a, b, step):
    n = max(1, int(round(abs(b - a) / step)))
    return [lerp(a, b, i / n) for i in range(n + 1)]


def shell(bm, body, y0, y1, fs, off=0.08, mat=H, col=None, sink=0.1):
    """An armour slab lying on a body between y0 > y1 over ring positions fs."""
    # One open skin with a 45-degree skirt down into the body: a single UV island, no hidden faces.
    e = max(off, 0.05)

    def seglen(y, f):
        i = int(math.floor(f))
        return max(1e-3, (body.at(y, i + 1.0) - body.at(y, float(i))).length)

    def row(y, grow):
        f0, f1 = fs[0], fs[-1]
        a = body.at(y, f0 - e / seglen(y, f0), -0.03)
        b = body.at(y, f1 + e / seglen(y, f1 - 1e-4), -0.03)
        return [a] + [body.at(y, f, grow) for f in fs] + [b]
    ys = body.ys(y0, y1)
    rows = [row(ys[0] + e, -0.03)] + [row(y, off) for y in ys] + [row(ys[-1] - e, -0.03)]
    yy = [ys[0]] + ys + [ys[-1]]
    ff = [fs[0]] + list(fs) + [fs[-1]]
    with tag(bm, col):
        vr = [[bm.verts.new(p) for p in r] for r in rows]
        for j in range(len(vr) - 1):
            for i in range(len(vr[j]) - 1):
                f = bm.faces.new((vr[j][i], vr[j][i + 1], vr[j + 1][i + 1], vr[j + 1][i]))
                f.material_index = mat
                n = body.surf((yy[j] + yy[j + 1]) / 2, (ff[i] + ff[i + 1]) / 2)[1]
                f.normal_update()
                if f.normal.dot(n) < 0:
                    f.normal_flip()


def rib(bm, body, y0, y1, grow=0.1, mat=H, col=None):
    """A band right round a body."""
    with tag(bm, col):
        loft(bm, [body.ring(y, grow) for y in body.ys(y0, y1)], mat=mat)


def obox(bm, p, n, size, yv=(0, 1, 0), mat=H):
    """Box whose local Z is n (centre at p), local Y as close to yv as it goes. size = (across, along yv, along n)."""
    n = V(n).normalized()
    t = V(yv).cross(n).normalized()
    yy = n.cross(t).normalized()
    return box(bm, V(p), size, mat=mat, rot=Matrix((t, yy, n)).transposed())


def pipe(bm, pts, r=0.05, n=4, mat=H):
    pts = [V(p) for p in pts]
    for a, b in zip(pts, pts[1:]):
        cyl(bm, a, b, r, n=n, mat=mat)


def windows(bm, body, y0, y1, fs, rnd, step=0.21, lit=0.6, grow=0.0, w=0.1, h=0.05):
    """Rows of small lit windows along a body; runs of dark ones so it reads as decks, not a dotted line."""
    for f in fs:
        run = 0
        for y in frange(y0, y1, step):
            if run <= 0 and rnd.random() > lit:
                run = rnd.randint(1, 5)
            if run > 0:
                run -= 1
                continue
            p, n = body.surf(y, f, grow)
            yy = (body.at(y + 0.05, f, grow) - body.at(y - 0.05, f, grow)).normalized()
            quad(bm, p, n, yy, h, w)


def quad(bm, p, n, yy, h, w, mat=G, lift=0.012):
    """One flat lit rectangle just off a surface, facing n."""
    n, yy = V(n).normalized(), V(yy).normalized()
    t = yy.cross(n).normalized()
    n = t.cross(yy).normalized() * (1 if t.cross(yy).dot(n) > 0 else -1)
    a = V(p) + n * lift
    f = bm.faces.new([bm.verts.new(a + t * (sx * h / 2) + yy * (sy * w / 2)) for sx, sy in ((-1, -1), (1, -1), (1, 1), (-1, 1))])
    f.normal_update()
    if f.normal.dot(n) < 0:
        f.normal_flip()
    f.material_index = mat


def hollow(bm, cap, out, rim, depth, rim_col=HZ, wall_col=DK, floor_col=DK):
    """Turn a cap face into a framed opening `depth` deep; `out` is the way the opening faces."""
    cap.normal_update()
    s = -1.0 if cap.normal.dot(V(out)) > 0 else 1.0
    with tag(bm, rim_col):
        inset_faces(bm, [cap], rim, 0.0)
    with tag(bm, wall_col):
        inset_faces(bm, [cap], 0.03, s * depth)
    paint([cap], floor_col)


def greeble(bm, body, y0, y1, f0, f1, count, rnd, grow=0.0, smax=0.5, hmax=0.14):
    for _ in range(max(1, int(count * 0.1))):
        p, n = body.surf(rnd.uniform(y1, y0), rnd.uniform(f0, f1), grow)
        sy, st, h = rnd.uniform(0.35, smax * 2.4), rnd.uniform(0.25, smax * 1.3), rnd.uniform(0.05, hmax * 1.5)
        h = min(h, sy * 0.22, st * 0.22)
        with tag(bm, rnd.choice((None, None, None, BM, DK))):
            frustum(bm, p, n, st, sy, h)


def frustum(bm, p, n, a, b, h, mat=H):
    """A low block with 45-degree sides and no bottom: one UV island."""
    n = V(n).normalized()
    t = V((0, 1, 0)).cross(n).normalized()
    yy = n.cross(t).normalized()
    p = V(p) - n * 0.02
    lo = [bm.verts.new(p + t * (sx * a / 2) + yy * (sy * b / 2)) for sx, sy in ((-1, -1), (1, -1), (1, 1), (-1, 1))]
    k = (h + 0.02) * 1.3
    hi = [bm.verts.new(p + n * (h + 0.02) + t * (sx * (a / 2 - k)) + yy * (sy * (b / 2 - k))) for sx, sy in ((-1, -1), (1, -1), (1, 1), (-1, 1))]
    faces = [bm.faces.new(hi)] + [bm.faces.new((lo[i], lo[(i + 1) % 4], hi[(i + 1) % 4], hi[i])) for i in range(4)]
    for f in faces:
        f.material_index = mat
        f.normal_update()
        if f.normal.dot(n) < 0:
            f.normal_flip()


def rplate(bm, c, ang, t0, r0, w0, t1, r1, w1, th):
    """A plate lying on a cone around the +Y axis through c (t forward of c)."""
    d = V((math.cos(ang), 0, math.sin(ang)))
    tg = V((-math.sin(ang), 0, math.cos(ang)))
    k = th * 1.3

    def rect(t, r, w, up):
        b = c + V((0, t, 0)) + d * (r + up)
        return b - tg * (w / 2), b + tg * (w / 2)
    a0, a1 = rect(t0, r0, w0, 0.0)
    b0, b1 = rect(t1, r1, w1, 0.0)
    c0, c1 = rect(t0 - k, r0, w0 - 2 * k, th)
    e0, e1 = rect(t1 + k, r1, w1 - 2 * k, th)
    lo = [bm.verts.new(p) for p in (a0, a1, b1, b0)]
    hi = [bm.verts.new(p) for p in (c0, c1, e1, e0)]
    for f in [bm.faces.new(hi)] + [bm.faces.new((lo[i], lo[(i + 1) % 4], hi[(i + 1) % 4], hi[i])) for i in range(4)]:
        f.normal_update()
        if f.normal.dot(d) < 0:
            f.normal_flip()


def engine(bm, c, r, Lh, N=16):
    """Exposed engine: ribbed mount collar, bell, two rows of petals, actuators, flame holder. c = exit centre."""
    c = V(c)
    ax = (0, 1, 0)
    with tag(bm, BM):
        lathe(bm, c, ax, [(r * 1.0, Lh + 0.25), (r * 1.0, Lh * 0.97), (r * 1.1, Lh * 0.95), (r * 1.1, Lh * 0.87),
                          (r * 1.0, Lh * 0.85), (r * 1.0, Lh * 0.78), (r * 1.07, Lh * 0.76), (r * 1.07, Lh * 0.7),
                          (r * 0.97, Lh * 0.68), (r * 0.97, Lh * 0.6)], n=32)
        lathe(bm, c, ax, [(r * 0.95, Lh * 0.64), (r * 0.9, Lh * 0.3), (r * 0.86, 0.0), (r * 0.79, 0.0),
                          (r * 0.77, Lh * 0.22), (r * 0.66, Lh * 0.5), (r * 0.5, Lh * 0.62), (0.0, Lh * 0.62)],
              n=32, mats=[H, H, H, H, E, E, E])
        w = 2 * math.pi / N
        for i in range(N):
            a = (i + 0.5) * w
            rplate(bm, c, a, Lh * 0.62, r * 1.0, r * w * 0.88, 0.06 * r, r * 0.89, r * 0.89 * w * 0.9, r * 0.035)
            a2 = i * w
            rplate(bm, c, a2, Lh * 0.66, r * 1.06, r * 1.06 * w * 0.6, Lh * 0.36, r * 1.0, r * w * 0.42, r * 0.04)
        t0 = Lh * 0.36
        lathe(bm, c, ax, [(r * 0.4, t0), (r * 0.44, t0 - 0.05 * r), (r * 0.4, t0 - 0.1 * r), (r * 0.36, t0 - 0.05 * r), (r * 0.4, t0)],
              n=20, cap_start=False, cap_end=False)
        lathe(bm, c, ax, [(0.0, t0 - 0.5 * r), (r * 0.15, t0 - 0.12 * r), (r * 0.17, t0 + 0.2 * r), (r * 0.12, Lh * 0.6)], n=12)
        for i in range(4):
            a = math.pi / 4 + i * math.pi / 2
            d = V((math.cos(a), 0, math.sin(a)))
            obox(bm, c + V((0, t0 - 0.02, 0)) + d * (r * 0.45), V((-d.z, 0, d.x)), (r * 0.62, r * 0.16, r * 0.03))


def slats(bm, p, n, w, length, count, h=0.1, t=0.045, yv=(0, 1, 0)):
    """Heat-sink fins standing on a base plate at p (normal n), spread along yv."""
    p, n = V(p), V(n).normalized()
    yy = V(yv).normalized()
    with tag(bm, DK):
        obox(bm, p + n * 0.015, n, (w + 0.1, length + 0.1, 0.05), yv=yv)
    with tag(bm, BM):
        for i in range(count):
            o = ((i + 0.5) / count - 0.5) * length
            obox(bm, p + yy * o + n * (h / 2 + 0.02), n, (w, t, h), yv=yv)


def turret(bm, c, s=1.0, up=1, fwd=1, lite=False):
    """Twin main-battery turret on a barbette at c. Returns the two muzzle points."""
    c = V(c)

    def T(x, y, z):
        return c + V((x * s, y * s * fwd, z * s * up))

    def B(ctr, size):
        box(bm, T(*ctr), (size[0] * s, size[1] * s, size[2] * s))
    cyl(bm, T(0, 0, -0.3), T(0, 0, 0.3), 1.2 * s, 1.1 * s, n=8 if lite else 24)
    if not lite:
        cyl(bm, T(0, 0, 0.26), T(0, 0, 0.34), 1.0 * s, n=24)
    rings = []
    for y, w, z1 in ((-1.45, 0.75, 0.86), (-1.1, 0.95, 1.0), (0.4, 1.0, 1.04), (1.3, 0.72, 0.84)):
        rings.append([T(-w, y, 0.3), T(w, y, 0.3), T(w, y, z1 - 0.22), T(w - 0.26, y, z1), T(-(w - 0.26), y, z1), T(-w, y, z1 - 0.22)])
    res = loft(bm, rings)
    if lite:
        with tag(bm, BM):
            for sx in (1, -1):
                cyl(bm, T(sx * 0.38, 1.2, 0.58), T(sx * 0.38, 4.6, 0.58), 0.13 * s, n=4)
        return []
    inset_faces(bm, [res['bands'][1][k] for k in (1, 3, 5)], 0.08 * s, -0.025 * s)
    B((0, -0.5, 1.1), (2.5, 0.22, 0.16))                    # rangefinder arms
    for sx in (1, -1):
        B((sx * 1.27, -0.5, 1.1), (0.14, 0.34, 0.26))
        B((sx * 0.5, 0.3, 1.07), (0.3, 0.4, 0.08))          # hatches
        B((sx * 0.38, 1.3, 0.58), (0.44, 0.3, 0.44))        # mantlets
    B((0, -1.5, 0.62), (1.1, 0.3, 0.4))
    with tag(bm, DK):
        B((0, -1.0, 1.04), (0.9, 0.5, 0.06))
    muz = []
    with tag(bm, BM):
        for sx in (1, -1):
            x, z = sx * 0.38, 0.58
            cyl(bm, T(x, 1.3, z), T(x, 2.3, z), 0.19 * s, 0.16 * s, n=12)
            cyl(bm, T(x, 2.3, z), T(x, 4.5, z), 0.105 * s, 0.095 * s, n=10)
            cyl(bm, T(x, 3.1, z), T(x, 3.3, z), 0.14 * s, n=10)
            cyl(bm, T(x, 4.5, z), T(x, 4.85, z), 0.14 * s, n=10)
            muz.append(T(x, 4.86, z))
    for m in muz:
        cyl(bm, m - V((0, 0.02 * fwd, 0)), m, 0.08 * s, n=8, mat=E)
    return muz


def pd(bm, p, n, s=1.0):
    """Point-defence mount: base, dome, twin barrels trained forward."""
    p, n = V(p), V(n).normalized()
    f = V((0, 1, 0)) - n * n.y
    f.normalize()
    t = f.cross(n)
    frustum(bm, p, n, 0.36 * s, 0.56 * s, 0.07 * s)


# ================================================================ the ship

HS = [(30.0, 1.3, 1.1, -0.4), (28.6, 2.0, 1.9, -0.3), (25.0, 2.7, 2.5, -0.1), (20.0, 3.2, 2.9, 0.0),
      (14.0, 3.5, 3.1, 0.0), (12.6, 2.9, 2.6, 0.0), (9.0, 2.9, 2.6, 0.0), (7.5, 4.6, 3.5, 0.0),
      (-12.0, 4.8, 3.6, 0.0), (-16.0, 4.2, 3.3, 0.0), (-19.0, 4.2, 3.3, 0.0), (-20.5, 5.4, 4.2, 0.0),
      (-27.0, 5.4, 4.2, 0.0), (-28.0, 4.9, 3.8, 0.0)]
PX = 9.6
PS = [(9.0, 1.3, 1.0, -0.6), (8.0, 1.9, 1.45, -0.6), (-13.0, 1.9, 1.45, -0.6), (-14.0, 1.3, 1.0, -0.6)]
T1 = [(5.6, 1.9, 0.7, 3.9), (4.6, 2.6, 0.8, 4.0), (-12.0, 2.6, 0.8, 4.0), (-13.4, 2.0, 0.7, 3.9)]
T2 = [(1.0, 1.3, 0.6, 5.2), (0.2, 1.8, 0.7, 5.3), (-10.0, 1.8, 0.7, 5.3), (-11.0, 1.4, 0.6, 5.2)]
T3 = [(-4.4, 0.7, 1.3, 7.0), (-4.9, 0.95, 1.4, 7.1), (-8.0, 0.95, 1.4, 7.1), (-8.6, 0.7, 1.3, 7.0)]
T4 = [(-4.0, 1.5, 0.34, 8.62), (-4.6, 1.95, 0.45, 8.7), (-7.3, 1.95, 0.45, 8.7), (-7.9, 1.4, 0.34, 8.62)]
BX = 3.35
BS = [(24.0, 0.35, 0.5, -0.3), (23.0, 0.7, 0.95, -0.3), (15.6, 0.78, 1.0, -0.3), (14.7, 0.4, 0.6, -0.3)]
GS = [(6.0, 1.0, 0.5, -3.9), (4.8, 1.7, 0.75, -4.05), (-9.5, 1.7, 0.75, -4.05), (-11.5, 1.1, 0.5, -3.9)]
CH = [(27.2, 0.6, 0.4, -2.5), (26.2, 1.0, 0.6, -2.75), (22.5, 1.1, 0.65, -3.05), (21.4, 0.7, 0.4, -2.95)]
SX = 6.2
SP = [(-19.4, 0.7, 0.9, 0.0), (-21.0, 1.15, 1.35, 0.0), (-27.0, 1.15, 1.35, 0.0), (-27.9, 0.95, 1.1, 0.0)]
ENGS = [((2.5, -30.4, 1.85), 1.72), ((-2.5, -30.4, 1.85), 1.72), ((2.5, -30.4, -1.85), 1.72), ((-2.5, -30.4, -1.85), 1.72),
        ((SX, -29.6, 0.0), 0.95), ((-SX, -29.6, 0.0), 0.95)]


def build(ship):
    for i in range(5):
        AREA[i] = 0.0
    TRIS.clear()
    rnd = random.Random(11)
    hull = Body(HS, top=0.62, bot=0.55, hi=0.5, lo=-0.5, keel=0.25)
    pod = Body(PS, cx=PX, top=0.7, bot=0.7, hi=0.6, lo=-0.6)
    boxy = dict(top=0.82, bot=0.9, hi=0.75, lo=-0.9)
    t1, t2, t3, t4 = Body(T1, **boxy), Body(T2, **boxy), Body(T3, **boxy), Body(T4, top=0.85, bot=0.7, hi=0.6, lo=-0.5)
    spn = Body(SP, cx=SX, top=0.7, bot=0.7, hi=0.6, lo=-0.6)
    bow = Body(BS, cx=BX, top=0.7, bot=0.7, hi=0.6, lo=-0.6)
    gon = Body(GS, top=0.9, bot=0.75, hi=0.9, lo=-0.5)
    chin = Body(CH, top=0.9, bot=0.7, hi=0.9, lo=-0.4)

    # ---------------------------------------------------------------- main hull
    def f_hull(bm):
        res = loft(bm, hull.rings())
        b = res['bands']
        paint(b[4] + b[5] + b[6], DK)                           # the neck is bare structure
        with tag(bm, DK):
            inset_faces(bm, [res['start']], 0.12, -0.25)
        paint([res['start']], DK)
        with tag(bm, BM):
            inset_faces(bm, [res['end']], 0.25, -0.5)
        paint([res['end']], BM)
    P(ship, f_hull, bevel=0.05)

    # ---------------------------------------------------------------- superstructure tiers and bridge
    def f_tiers(bm):
        for t in (t1, t2, t3):
            loft(bm, t.rings())
        res = loft(bm, t4.rings())
        res = loft(bm, gon.rings())
        inset_faces(bm, [res['bands'][1][k] for k in (2, 7)], 0.12, -0.05)
        loft(bm, chin.rings())
        box(bm, (0, 24.4, -3.72), (1.1, 1.6, 0.12), mat=GL)        # the chin sensor window
        # bridge windows: one bright band round the head
        box(bm, (0, -3.99, 8.66), (2.5, 0.03, 0.07), mat=G)
        # stepped deckhouses fore and aft of the tower
        box(bm, (0, -2.4, 6.25), (2.2, 2.6, 0.7))
        box(bm, (0, -9.4, 6.2), (1.6, 1.4, 0.6))
        box(bm, (0, -3.2, 6.8), (1.2, 1.0, 0.5))
        # funnel-like exhaust stacks behind the tower
        with tag(bm, BM):
            for sx in (0.5, -0.5):
                box(bm, (sx, -9.3, 6.9), (0.5, 0.9, 1.1), taper=(0.8, 1.0))
        with tag(bm, DK):
            for sx in (0.5, -0.5):
                box(bm, (sx, -9.3, 7.46), (0.38, 0.72, 0.03))
        # barbette deckhouses for the superfiring and aft turrets
        prism(bm, (0, 17.6, hull.top(16) + 0.4), (0, 13.4, hull.top(14) + 0.4), 1.75, 0.62, chamfer=0.2)
        prism(bm, (0, -15.4, 3.7), (0, -19.6, 3.7), 1.85, 0.55, chamfer=0.2)
    P(ship, f_tiers, bevel=0.03)

    # ---------------------------------------------------------------- flight pods
    def f_pod(bm):
        res = loft(bm, pod.rings())
        for cap, y, s in ((res['start'], 9.0, 1), (res['end'], -14.0, -1)):
            hollow(bm, cap, (0, s, 0), 0.09, 1.7, floor_col=WT)
            # deck lip, ceiling light bars, deck guide lights
            box(bm, (PX, y + s * 0.35, -1.52), (2.1, 1.3, 0.07))
            with tag(bm, HZ):
                box(bm, (PX, y + s * 0.98, -1.52), (2.1, 0.08, 0.08))
            for d in (0.35, 0.8, 1.25):
                box(bm, (PX, y - s * d, 0.26), (1.5, 0.05, 0.03), mat=G)
            for dx in (-0.55, 0.55):
                box(bm, (PX + dx, y - s * 0.3, -1.47), (0.03, 1.9, 0.02), mat=G)
            for dx in frange(-0.8, 0.8, 0.2):
                for dz in (-0.3, 0.0):
                    box(bm, (PX + dx, y - s * 1.66, -0.6 + dz), (0.08, 0.02, 0.05), mat=G)
        # ribs
        for y in frange(7.2, -12.6, 1.65):
            rib(bm, pod, y, y - 0.34, 0.13)
        rib(bm, pod, 8.05, 7.75, 0.2)
        rib(bm, pod, -12.75, -13.05, 0.2)
        # faction bands between ribs
        for y in (5.25, 3.6, -8.0, -9.65):
            rib(bm, pod, y - 0.42, y - 1.57, 0.05, mat=L)
        # launch-tube doors along the outer flank
        for y in frange(6.35, -11.8, 1.65):
            if 1.5 < y < 6.0 or -10.5 < y < -7.0:
                continue
            p, n = pod.surf(y - 1.0, 2.5)
            with tag(bm, DK):
                obox(bm, p + n * 0.02, n, (0.7, 0.9, 0.05))
            with tag(bm, HZ):
                obox(bm, p + n * 0.03 + V((0, 0, 0.42)), n, (0.07, 0.9, 0.05))
    P(ship, f_pod, bevel=0.025, segs=2, mirror=True)

    # ---------------------------------------------------------------- docking pylons hull -> pod
    def f_pylons(bm):
        for y in (5.2, -2.6, -10.4):
            prism(bm, (4.3, y, -0.5), (7.9, y, -0.6), 1.0, 0.5, w1=0.8, h1=0.42, chamfer=0.2)
            prism(bm, (5.4, y, -0.5), (6.6, y, -0.55), 1.12, 0.58, chamfer=0.2)
            with tag(bm, BM):
                for dy in (-0.8, 0.8):
                    pipe(bm, [(4.5, y + dy, 1.4), (7.9, y + dy, 0.2)], r=0.07)
        # docking arms under the bow: boom, elbow, clamp head
        for y in (19.0, 16.0):
            x0 = hull.dims(y)[0] * 0.8
            prism(bm, (x0, y, -1.9), (x0 + 2.4, y, -2.5), 0.14, 0.14)
            with tag(bm, BM):
                pipe(bm, [(x0, y + 0.5, -1.4), (x0 + 2.3, y, -2.4)], r=0.04)
                pipe(bm, [(x0, y - 0.5, -1.4), (x0 + 2.3, y, -2.4)], r=0.04)
            box(bm, (x0 + 2.55, y, -2.55), (0.4, 0.7, 0.4))
            with tag(bm, HZ):
                box(bm, (x0 + 2.78, y, -2.55), (0.08, 0.76, 0.46))
            sphere(bm, (x0 + 2.55, y, -2.8), 0.05, mat=G, seg=6, rings=4)
    P(ship, f_pylons, bevel=0.02, segs=1, mirror=True)

    # ---------------------------------------------------------------- engine block: sponsons, ribs, radiators
    def f_block(bm):
        res = loft(bm, bow.rings())
        paint([res['start']], DK)
        for y in frange(22.4, 16.4, 1.5):
            rib(bm, bow, y, y - 0.25, 0.07)
        with tag(bm, BM):                                         # fixed lance in each cheek
            cyl(bm, (BX, 23.6, -0.3), (BX, 25.2, -0.3), 0.26, 0.2, n=12)
            cyl(bm, (BX, 25.2, -0.3), (BX, 27.6, -0.3), 0.12, 0.1, n=10)
            cyl(bm, (BX, 26.2, -0.3), (BX, 26.5, -0.3), 0.17, n=10)
            cyl(bm, (BX, 27.6, -0.3), (BX, 27.95, -0.3), 0.16, n=10)
        res = loft(bm, spn.rings())
        paint([res['end']], BM)
        for y in frange(-21.6, -26.4, 1.2):
            rib(bm, spn, y, y - 0.3, 0.08)
        rib(bm, spn, -22.7, -23.9, 0.05, mat=L)
        rib(bm, spn, -25.1, -26.3, 0.05, mat=L)
        zt = 2.45
        for sz in (1, -1):
            d = V((0.78, 0, 0.62 * sz))
            n = V((-0.62 * sz, 0, 0.78))
            base = V((3.2, -17.5, zt * sz))
            with tag(bm, DK):
                obox(bm, base + d * 1.8, n, (3.6, 3.4, 0.06))
            with tag(bm, BM):
                for y in frange(-15.9, -19.1, 0.8):
                    obox(bm, V((3.2, y, zt * sz)) + d * 1.8, n, (3.5, 0.08, 0.16))
                for q in (0.02, 1.0):
                    obox(bm, base + d * (3.6 * q), n, (0.1, 3.5, 0.18))
                pipe(bm, [base + V((0, 1.8, 0)), base + V((0, -1.8, 0))], r=0.12, n=6)
            with tag(bm, WT):
                obox(bm, base + d * 1.8, n, (0.25, 3.42, 0.075))
    P(ship, f_block, bevel=0.02, segs=1, mirror=True)

    def f_block_c(bm):
        # neck: collars and longerons over the bare core
        for y in (12.2, 10.8, 9.4):
            rib(bm, hull, y, y - 0.3, 0.16)
        with tag(bm, BM):
            for f in (0.5, 1.5, 2.5, 3.5, 4.5, 5.5, 6.5, 7.5, 8.5, 9.5):
                pipe(bm, [hull.at(12.5, f, 0.22), hull.at(9.0, f, 0.22)], r=0.09)
        for y in frange(-21.3, -26.5, 1.3):
            rib(bm, hull, y, y - 0.42, 0.16)
        rib(bm, hull, -27.1, -27.5, 0.1, col=BM)
        rib(bm, hull, -22.5, -23.4, 0.1, mat=L)
    P(ship, f_block_c, bevel=0.03, segs=2)

    def f_eng(bm):
        engine(bm, ENGS[0][0], ENGS[0][1], 2.3, N=12)
        engine(bm, ENGS[2][0], ENGS[2][1], 2.3, N=12)
        engine(bm, ENGS[4][0], ENGS[4][1], 1.5, N=8)
    P(ship, f_eng, bevel=0.012, segs=1, mirror=True, recalc=False)
    for i, (c, r) in enumerate(ENGS):
        ship.empty("Engine_%d" % i, c, r * 0.8)

    # ---------------------------------------------------------------- main battery
    guns = []

    def f_turrets(bm):
        guns.extend(turret(bm, (0, 21.5, hull.top(21.5) + 0.05), s=1.25))
        guns.extend(turret(bm, (0, 15.4, hull.top(15.4) + 1.05), s=1.25))
        guns.extend(turret(bm, (0, -17.5, 4.3), s=1.25, fwd=-1))
        guns.extend(turret(bm, (0, 18.0, hull.bot(18.0) - 0.3), up=-1))
        guns.extend(turret(bm, (0, -15.6, hull.bot(-15.6) - 0.3), up=-1, fwd=-1))
    P(ship, f_turrets, bevel=0.02, segs=1)
    for i, g in enumerate(guns):
        ship.empty("Gun_%d" % i, g, 0.2)

    # ---------------------------------------------------------------- armour
    def f_armor(bm):
        # prow
        shell(bm, hull, 29.6, 26.2, [0.12, 1, 2, 3, 3.9], off=0.12)
        shell(bm, hull, 25.9, 24.5, [1.08, 2, 2.95], off=0.09)
        shell(bm, hull, 24.2, 22.0, [0.1, 1, 2, 3, 3.9], off=0.08, mat=L)
        shell(bm, hull, 24.38, 24.24, [0.1, 1, 2, 3, 3.9], off=0.08, col=WT)
        shell(bm, hull, 21.96, 21.82, [0.1, 1, 2, 3, 3.9], off=0.08, col=WT)
        for y0, y1 in ((21.6, 19.6), (19.3, 16.8), (16.5, 14.2)):
            shell(bm, hull, y0, y1, [1.06, 1.94], off=0.09)
            shell(bm, hull, y0, y1, [2.3, 2.95], off=0.12)
            shell(bm, hull, y0, y1, [3.08, 3.92], off=0.08)
        # citadel: belt, shoulder and bilge strakes in ribbed lengths
        for i, y in enumerate(frange(7.2, -11.2, 1.84)[:-1]):
            shell(bm, hull, y, y - 1.62, [2.36, 2.96], off=0.14, mat=L if i in (2, 3, 4, 5) else H)
            if i % 3 != 1:
                shell(bm, hull, y, y - 1.62, [1.07, 1.93], off=0.09)
            if i % 4 != 2:
                shell(bm, hull, y, y - 1.62, [3.07, 3.93], off=0.09)
        shell(bm, hull, 7.3, -11.4, [1.95, 2, 2.07], off=0.1, mat=L)
        # waist and engine block cheeks
        shell(bm, hull, -12.6, -15.6, [1.1, 2, 2.95], off=0.09)
        shell(bm, hull, -13.2, -14.6, [0.1, 1, 2, 3, 3.9], off=0.12, mat=L)
        # bow cheek galleries: a stepped strake down each side of the wedge
        shell(bm, bow, 22.0, 16.3, [2.2, 2.8], off=0.1, mat=L)
        # stencil blocks: hull number on the bow, pennant blocks on the pods
        for y in (21.4, 20.7, 20.0):
            shell(bm, hull, y, y - 0.5, [2.08, 2.26], off=0.03, col=WT)
        for y in (7.0, 6.45):
            shell(bm, pod, y, y - 0.4, [2.25, 2.75], off=0.03, col=WT)
        shell(bm, hull, -16.2, -18.8, [3.08, 3.92], off=0.09)
        # tier edges
        shell(bm, t1, 4.3, -11.8, [1.9, 2.3], off=0.07)
        shell(bm, t2, 0.0, -9.8, [1.9, 2.3], off=0.06)
        shell(bm, t2, -2.0, -3.6, [2.3, 2.95], off=0.05, mat=L)
        shell(bm, t3, -5.0, -7.9, [2.7, 2.98], off=0.05)
        shell(bm, t3, -5.3, -6.1, [2.1, 2.66], off=0.04, mat=L)
        with tag(bm, HZ):
            shell(bm, hull, 14.1, 13.9, [1.1, 2, 2.9], off=0.1)
            shell(bm, hull, 7.75, 7.55, [1.1, 2, 2.9], off=0.12)
    P(ship, f_armor, bevel=0.02, segs=1, mirror=True, recalc=False)

    def f_armor_c(bm):
        shell(bm, hull, 29.0, 22.6, [9.75, 10, 10.25], off=0.14)          # prow spine
        shell(bm, hull, 21.0, 9.2, [4.55, 5, 5.45], off=0.14)             # keel
        shell(bm, hull, -12.5, -19.0, [4.6, 5, 5.4], off=0.16, mat=L)
        shell(bm, t4, -4.7, -7.2, [9.3, 10, 10.7], off=0.05)
    P(ship, f_armor_c, bevel=0.02, segs=1, recalc=False)

    # ---------------------------------------------------------------- neck tanks, secondaries, broadside guns
    def f_side(bm):
        for y in (11.85, 10.75, 9.65):
            for x, z, r in ((3.3, 0.0, 0.5), (2.5, 2.5, 0.36), (2.5, -2.5, 0.36)):
                sphere(bm, (x, y, z), r, seg=10, rings=6)
        # secondary twin turrets on the pods
        for y in (6.2, -2.4, -11.0):
            turret(bm, (PX, y, pod.top(y) + 0.02), s=0.42, lite=True)
        for y in (2.0, -6.5):
            turret(bm, (PX, y, pod.bot(y) - 0.02), s=0.42, up=-1, lite=True)
        # broadside casemates on the belt
        for y in frange(6.4, -10.2, 1.84):
            p, n = hull.surf(y - 0.8, 2.66, 0.14)
            obox(bm, p + n * 0.1, n, (0.42, 0.7, 0.24))
            with tag(bm, BM):
                cyl(bm, p + n * 0.15, p + n * 1.05, 0.07, 0.06, n=4)
        slats(bm, (1.1, -1.4, t2.top(-1.4)), (0, 0, 1), 0.7, 1.6, 4, h=0.1, t=0.12)
        slats(bm, (0.9, 3.0, t1.top(3.0)), (0, 0, 1), 1.0, 1.4, 4, h=0.1, t=0.12)
        # heat sinks on the engine block
        for x in (1.5, 3.3):
            slats(bm, (x, -24.0, 4.2 + 0.16), (0, 0, 1), 1.3, 3.6, 4, h=0.16, t=0.28)
            slats(bm, (x, -24.0, -4.2 - 0.16), (0, 0, -1), 1.3, 3.6, 4, h=0.16, t=0.28)
        p, n = hull.surf(-24.0, 2.5, 0.16)
        slats(bm, p, n, 2.2, 3.6, 4, h=0.14, t=0.28)
    P(ship, f_side, bevel=0.012, segs=1, mirror=True)

    # ---------------------------------------------------------------- lit windows: single quads, wound by hand
    def f_win(bm):
        windows(bm, hull, 27.0, 14.6, [2.06, 2.13, 2.2], rnd, step=0.17)
        windows(bm, hull, 27.0, 22.4, [1.3, 1.6], rnd, lit=0.4)
        windows(bm, hull, 26.0, 14.6, [3.2, 3.45], rnd, lit=0.45)
        windows(bm, hull, 7.0, -11.6, [2.1, 2.16, 2.22, 2.28], rnd, step=0.17)
        windows(bm, hull, 7.0, -11.6, [3.25, 3.5, 3.75], rnd, lit=0.45, grow=0.09)
        windows(bm, hull, 7.0, -11.6, [1.3, 1.6], rnd, lit=0.4, grow=0.09)
        windows(bm, hull, -12.6, -18.8, [2.3, 2.42, 2.54], rnd)
        windows(bm, hull, -21.0, -26.5, [2.12, 2.2], rnd, grow=0.16, lit=0.4)
        windows(bm, t1, 4.4, -11.8, [2.42, 2.62, 2.82], rnd, step=0.17)
        windows(bm, t2, 0.0, -9.8, [2.45, 2.68], rnd)
        windows(bm, t3, -5.0, -7.9, [2.15, 2.27, 2.39, 2.51, 2.63], rnd, step=0.19, lit=0.7)
        windows(bm, t4, -4.7, -7.2, [2.5], rnd, step=0.13, lit=0.95)
        windows(bm, pod, 7.4, -12.4, [1.94, 2.94, 1.5, 3.5], rnd, grow=0.0, lit=0.5)
        windows(bm, pod, 7.4, -12.4, [7.06, 7.94], rnd, lit=0.5)
        windows(bm, spn, -21.2, -26.8, [2.4], rnd, lit=0.5)
        windows(bm, gon, 4.6, -9.3, [2.25, 2.5], rnd, lit=0.6)
        windows(bm, bow, 22.6, 16.0, [1.5, 2.08, 2.92], rnd, lit=0.55)
        windows(bm, hull, 8.85, 7.65, [2.12, 2.24, 2.36, 2.48, 2.6, 2.72, 2.84], rnd, lit=0.75)
        windows(bm, pod, 7.4, -12.4, [8.3], rnd, lit=0.4)
        windows(bm, hull, 28.0, -27.0, [1.02], rnd, step=1.3, lit=1.0, w=0.05, h=0.05)   # deck-edge lamps
        windows(bm, hull, 28.0, -27.0, [3.98], rnd, step=1.3, lit=1.0, w=0.05, h=0.05)
        windows(bm, chin, 26.0, 22.6, [2.3], rnd, lit=0.7)
        # front faces of the tiers
        for t, y, zs in ((t1, 5.62, (3.7, 4.0)), (t2, 1.02, (5.0, 5.3)), (t3, -4.38, (6.4, 6.7, 7.0, 7.3, 7.6))):
            hw = t.dims(y)[0] * 0.75
            for z in zs:
                for x in frange(0.1, hw, 0.2):
                    if rnd.random() < 0.7:
                        quad(bm, (x, y, z), (0, 1, 0), (1, 0, 0), 0.05, 0.1)
    P(ship, f_win, bevel=0.0, mirror=True, weighted=False, recalc=False)

    # ---------------------------------------------------------------- small stuff, mirrored: PD, greebles
    def f_small(bm):
        for y in frange(3.5, -8.5, 4.0):
            pd(bm, *gon.surf(y, 3.5))
        # point defence
        for y in frange(27.0, 15.0, 2.4) + frange(6.2, -11.0, 2.15) + frange(-21.5, -26.5, 2.5):
            pd(bm, *hull.surf(y, 1.5, 0.1))
        for y in frange(26.0, 15.0, 3.6) + frange(5.3, -10.0, 3.06) + frange(-21.5, -26.5, 2.5):
            pd(bm, *hull.surf(y, 3.5, 0.1))
        for y in frange(3.6, -11.0, 2.9):
            pd(bm, *t1.surf(y, 0.9), s=0.8)
        for y in frange(7.0, -12.0, 3.8):
            pd(bm, *pod.surf(y, 1.5, 0.13))
            pd(bm, *pod.surf(y, 3.5, 0.13))
            pd(bm, *pod.surf(y, 8.5, 0.13))
        for y in (21.5, 19.0, 16.8):
            pd(bm, *bow.surf(y, 1.5))
        # running lights
        for p in ((hull.dims(28.6)[0], 28.6, -0.3), (PX + 1.95, 8.0, -0.6), (PX + 1.95, -13.0, -0.6), (SX + 1.2, -27.0, 0.0), (6.05, -17.5, 4.75)):
            sphere(bm, p, 0.07, mat=G, seg=6, rings=4)
    D(ship, f_small, mirror=True)

    def f_greeb(bm):
        greeble(bm, gon, 4.6, -9.4, 4.05, 4.95, 50, rnd, smax=0.4)
        greeble(bm, hull, 7.0, -11.5, 0.86, 0.99, 80, rnd, smax=0.4)
        greeble(bm, hull, 7.0, -11.5, 1.05, 1.95, 100, rnd)
        greeble(bm, hull, 7.0, -11.5, 3.05, 3.95, 70, rnd)
        greeble(bm, hull, 27.0, 14.5, 0.3, 0.95, 70, rnd)
        greeble(bm, hull, 12.4, 9.2, 0.2, 4.8, 80, rnd, smax=0.6, hmax=0.3)
        greeble(bm, hull, -12.5, -19.0, 0.6, 4.6, 130, rnd, smax=0.6, hmax=0.2)
        greeble(bm, hull, -20.8, -26.8, 1.05, 1.95, 60, rnd, grow=0.16)
        greeble(bm, hull, -20.8, -26.8, 3.05, 3.95, 60, rnd, grow=0.16)
        greeble(bm, t1, 4.4, -12.0, 0.72, 0.98, 70, rnd, smax=0.35)
        greeble(bm, hull, 26.0, 14.5, 3.55, 3.95, 60, rnd)
        greeble(bm, hull, 26.0, 14.5, 1.1, 1.9, 40, rnd, grow=0.09, smax=0.4)
        greeble(bm, spn, -21.2, -26.8, 0.1, 0.9, 30, rnd, smax=0.4)
        greeble(bm, t2, 0.0, -10.0, 0.1, 0.95, 110, rnd, smax=0.4)
        greeble(bm, t1, 4.2, 1.4, 0.1, 0.7, 40, rnd, smax=0.4)
        greeble(bm, t4, -4.8, -7.2, 0.1, 0.9, 20, rnd, smax=0.3, hmax=0.1)
        greeble(bm, bow, 22.8, 15.8, 0.1, 0.9, 30, rnd, smax=0.35)
        greeble(bm, pod, 7.5, -12.5, 0.1, 0.95, 60, rnd, smax=0.4)
        greeble(bm, pod, 7.5, -12.5, 4.1, 4.9, 50, rnd, smax=0.4)
        greeble(bm, pod, 7.5, -12.5, 7.05, 7.95, 50, rnd, smax=0.4)
    P(ship, f_greeb, bevel=0.0, mirror=True, weighted=False, recalc=False)

    # ---------------------------------------------------------------- centre line: antenna farm, sensors, bow tubes
    def f_center(bm):
        zt = t4.top(-6.0)
        with tag(bm, BM):
            for x, y, h, r in ((0.0, -6.9, 1.4, 0.035), (0.9, -6.6, 1.3, 0.025), (-0.9, -6.6, 1.3, 0.025), (1.5, -5.2, 0.8, 0.02),
                               (-1.5, -5.2, 0.8, 0.02), (0.45, -7.4, 1.0, 0.02), (-0.45, -7.4, 1.0, 0.02), (0.0, -4.9, 0.6, 0.02)):
                cyl(bm, (x, y, zt), (x, y, zt + h), r, r * 0.4, n=5)
                cyl(bm, (x, y, zt), (x, y, zt + 0.12), r * 3, r * 2, n=6)
            box(bm, (0, -6.9, zt + 0.95), (0.9, 0.03, 0.03))
            box(bm, (0, -6.9, zt + 1.2), (0.6, 0.03, 0.03))
            # radar bar and dishes
            cyl(bm, (0, -5.8, zt), (0, -5.8, zt + 0.35), 0.1, n=8)
            box(bm, (0, -5.8, zt + 0.5), (1.8, 0.12, 0.3))
            for sx in (1.15, -1.15):
                cyl(bm, (sx, -6.0, zt), (sx, -6.0, zt + 0.3), 0.05, n=6)
                lathe(bm, (sx, -6.0, zt + 0.3), (0, 0.5, 1), [(0.03, 0.0), (0.4, 0.14), (0.38, 0.17), (0.0, 0.05)], n=12)
        sphere(bm, (0, -6.9, zt + 1.42), 0.06, mat=G, seg=6, rings=4)
        # mast on the fore tier and comms array on the aft roof
        with tag(bm, BM):
            cyl(bm, (0, 3.6, 4.8), (0, 3.6, 6.3), 0.06, 0.03, n=6)
            box(bm, (0, 3.6, 5.6), (1.2, 0.04, 0.04))
            for x in (-0.9, -0.3, 0.3, 0.9):
                cyl(bm, (x, -11.4, 4.75), (x, -11.4, 5.6 + abs(x) * 0.4), 0.025, n=5)
            lathe(bm, (0, -10.3, 6.0), (0, -0.4, 1), [(0.05, 0.0), (0.6, 0.2), (0.57, 0.24), (0.0, 0.08)], n=14)
        box(bm, (0, 30.05, -0.45), (0.18, 0.75, 2.4))                # armoured stem
        box(bm, (0, 29.75, 0.78), (1.9, 0.5, 0.14))
        # bow: sensor booms and the torpedo tubes in the face
        with tag(bm, BM):
            for x, z in ((0.0, -1.3), (0.7, -0.9), (-0.7, -0.9)):
                cyl(bm, (x, 29.6, z), (x, 31.0, z), 0.05, 0.02, n=6)
        for x in (-0.6, -0.2, 0.2, 0.6):
            for z in (-0.75, -0.25):
                cyl(bm, (x, 29.7, z), (x, 29.76, z), 0.12, n=8, mat=G if (x > 0) == (z > -0.5) else H)
        # ventral: sensor keel and the belly hangar door
        with tag(bm, DK):
            box(bm, (0, -3.0, -4.8), (2.0, 3.4, 0.06))
        with tag(bm, HZ):
            for dy in (-1.75, 1.75):
                box(bm, (0, -3.0 + dy, -4.81), (2.1, 0.1, 0.07))
        for dx in (-0.9, 0.9):
            box(bm, (dx, -3.0, -4.84), (0.04, 3.0, 0.02), mat=G)
        # stern face between the nozzles
        with tag(bm, BM):
            box(bm, (0, -28.5, 0.0), (0.9, 0.3, 7.0))
            box(bm, (0, -28.5, 0.0), (9.0, 0.3, 0.7))
            cyl(bm, (0, -28.4, 0), (0, -29.3, 0), 0.6, 0.5, n=12)
        cyl(bm, (0, -29.3, 0), (0, -29.33, 0), 0.3, n=10, mat=G)
    P(ship, f_center, bevel=0.0, weighted=False)

    tot = sum(AREA) or 1.0
    log("west parts", TRIS)
    log("west area %.0f" % tot, "hull %.0f%% livery %.1f%% glow %.1f%% glass %.1f%% core %.1f%%" % tuple(100 * a / tot for a in AREA))

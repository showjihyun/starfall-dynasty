"""`unknown` — the unknown cosmos: a Precursor keyship (capital, 60 units long, 1 unit ~ 7 m).

A caged star in an armillary, held in the eye of a split obsidian lance (two half-hulls that never touch: the
slit between them is lit from inside and bridged by a crystal needle and rungs of light), cradled by two vast
drooping crescent blades, crowned by a broken halo whose arcs are tied by threads of light, and driven by ring
apertures. Curves are sampled splines, edges are razors; scale comes from engraved channels in every section
whose floors carry hundreds of light dashes in a Fibonacci rhythm, from lit trenches, satellite shards on
spines, and concentric glyph arcs in the Hull material.
"""
import math
import random

import bmesh
from mathutils import Matrix, Vector

import shiplib
from shiplib import E, G, GL, H, L, circle_ring, lathe, loft, prism

V = Vector

STYLE = dict(paint=(0.03, 0.05, 0.07), paint2=(0.05, 0.03, 0.08), accent=(0.03, 0.05, 0.07), accent_thresh=2.0,
             metal=0.85, rough=0.3, spacing=(400.0, 400.0, 400.0), wear=0.5, grime=0.4, sub_amt=0.0, seam_w=0.0004,
             grime_col=(0.008, 0.01, 0.016), bump=0.8, bare=(0.5, 0.66, 0.7), ao_dist=2.0)

TINT = dict(
    glow=(0.28, 1.0, 0.8),       # mint-white core light
    engine=(0.62, 0.4, 1.0),     # violet drive apertures
    glass=(0.05, 0.02, 0.1),     # amethyst crystal
    livery_metal=0.85,
    livery_rough=0.32,
    rim=(0.5, 0.62, 1.0),
)


# ================================================================ material

def materials():
    """Hull: pearlescent dark shell (hue by facing and position: survives the bake), engraved concentric
    arcs and a sparse circuit lattice inlaid with pale metal, a little edge glint; no panel seams."""
    mats, socks = shiplib.make_materials(STYLE)
    hull = mats["Hull"]
    nb = shiplib.NB(hull)
    # what the stock graph already computes and is worth keeping: grime (AO, streaks) and edge wear
    last = socks['base'].node
    wear = last.inputs[0].links[0].from_socket
    seam_mix = last.inputs[6].links[0].from_node
    grime_mix = seam_mix.inputs[6].links[0].from_node
    grime = grime_mix.inputs[0].links[0].from_node.inputs[0].links[0].from_socket

    tc = nb.node('ShaderNodeTexCoord')
    pos = tc.outputs['Object']
    geo = nb.node('ShaderNodeNewGeometry')
    sp = nb.node('ShaderNodeSeparateXYZ')
    nb.link(pos, sp.inputs[0])
    X, Y, Z = sp.outputs
    sn = nb.node('ShaderNodeSeparateXYZ')
    nb.link(geo.outputs['Normal'], sn.inputs[0])
    NX, NY, NZ = sn.outputs

    # ---- engraving: a polar maze about the core — concentric arcs broken into segments, short radial ticks
    # between neighbours. One family lies in plan (on faces that look up or down), one in profile (on the sails).
    SP, LW = 0.8, 0.04
    yy = nb.math('MULTIPLY', Y, 0.5)

    def polar(A, mask, seed, K=26.0, arc_gate=0.45, tick_gate=0.84):
        r = nb.math('SQRT', nb.math('ADD', nb.math('MULTIPLY', A, A), nb.math('MULTIPLY', yy, yy)))
        rt = nb.math('DIVIDE', r, SP)
        ring_id = nb.math('FLOOR', nb.math('ADD', rt, 0.5))
        d = nb.math('MULTIPLY', nb.math('ABSOLUTE', nb.math('SUBTRACT', nb.math('FRACT', nb.math('ADD', rt, 0.5)), 0.5)), SP)
        arc = nb.mrange(d, LW * 0.45, LW, 1.0, 0.0, smooth=True)
        ang = nb.math('ARCTAN2', yy, nb.math('ABSOLUTE', A))
        seg = nb.math('FLOOR', nb.math('ADD', nb.math('MULTIPLY', ang, 7.0), nb.math('MULTIPLY', ring_id, 0.37)))
        cid = nb.node('ShaderNodeCombineXYZ')
        nb.link(ring_id, cid.inputs[0])
        nb.link(seg, cid.inputs[1])
        cid.inputs[2].default_value = seed
        wn = nb.node('ShaderNodeTexWhiteNoise', noise_dimensions='3D')
        nb.link(cid.outputs[0], wn.inputs['Vector'])
        arc = nb.math('MULTIPLY', arc, nb.math('GREATER_THAN', wn.outputs['Value'], arc_gate))
        at = nb.math('MULTIPLY', ang, K)
        da = nb.math('ABSOLUTE', nb.math('SUBTRACT', nb.math('FRACT', nb.math('ADD', at, 0.5)), 0.5))
        dist = nb.math('DIVIDE', nb.math('MULTIPLY', da, r), K)
        tick = nb.mrange(dist, LW * 0.45, LW, 1.0, 0.0, smooth=True)
        tid = nb.node('ShaderNodeCombineXYZ')
        nb.link(nb.math('FLOOR', rt), tid.inputs[0])
        nb.link(nb.math('FLOOR', nb.math('ADD', at, 0.5)), tid.inputs[1])
        tid.inputs[2].default_value = seed + 7.0
        tw = nb.node('ShaderNodeTexWhiteNoise', noise_dimensions='3D')
        nb.link(tid.outputs[0], tw.inputs['Vector'])
        tick = nb.math('MULTIPLY', tick, nb.math('GREATER_THAN', tw.outputs['Value'], tick_gate))
        band = nb.node('ShaderNodeTexWhiteNoise', noise_dimensions='1D')
        nb.link(nb.math('FLOOR', nb.math('MULTIPLY', rt, 0.5)), band.inputs['W'])
        return nb.math('MULTIPLY', nb.math('MAXIMUM', arc, tick), mask), band.outputs['Value']

    flat = nb.mrange(nb.math('ABSOLUTE', NZ), 0.45, 0.7, 0.0, 1.0)
    wall = nb.mrange(nb.math('ABSOLUTE', NX), 0.72, 0.88, 0.0, 1.0)
    g_plan, band = polar(X, flat, 0.0)
    g_prof, _ = polar(Z, wall, 3.0, K=20.0, arc_gate=0.6, tick_gate=0.9)
    glyph = nb.math('MAXIMUM', g_plan, g_prof)

    # ---- pearlescence
    big = nb.node('ShaderNodeTexNoise')
    nb.link(pos, big.inputs['Vector'])
    big.inputs['Scale'].default_value = 0.09
    big.inputs['Detail'].default_value = 3
    q = nb.math('ADD', nb.math('MULTIPLY', NZ, 0.30), 0.5)
    q = nb.math('ADD', q, nb.math('MULTIPLY', nb.math('ABSOLUTE', NX), -0.16))
    q = nb.math('ADD', q, nb.math('MULTIPLY', nb.math('SUBTRACT', big.outputs['Fac'], 0.5), 0.55))
    q = nb.math('ADD', q, nb.math('MULTIPLY', nb.math('SUBTRACT', band, 0.5), 0.1))
    q = nb.math('ADD', q, nb.math('MULTIPLY', Y, 0.003))
    ramp = nb.node('ShaderNodeValToRGB')
    els = ramp.color_ramp.elements
    els[0].position, els[0].color = 0.08, (0.11, 0.06, 0.03, 1)       # ember bronze, undersides
    els[1].position, els[1].color = 0.95, (0.04, 0.17, 0.19, 1)       # deep teal, crowns
    for p, c in ((0.3, (0.09, 0.035, 0.14, 1)), (0.52, (0.04, 0.05, 0.16, 1)), (0.74, (0.03, 0.11, 0.17, 1))):
        e = els.new(p)
        e.color = c
    nb.link(q, ramp.inputs[0])
    base = ramp.outputs[0]

    # ---- assemble
    col = nb.mix(nb.math('MULTIPLY', grime, 0.45), base, (*STYLE['grime_col'], 1))
    col = nb.mix(glyph, col, (0.42, 0.7, 0.66, 1))
    col = nb.mix(wear, col, (*STYLE['bare'], 1))
    fine = nb.node('ShaderNodeTexNoise')
    nb.link(pos, fine.inputs['Vector'])
    fine.inputs['Scale'].default_value = 2.0
    fine.inputs['Detail'].default_value = 6
    rough = nb.math('ADD', STYLE['rough'], nb.math('MULTIPLY', nb.math('SUBTRACT', fine.outputs['Fac'], 0.5), 0.22))
    rough = nb.math('ADD', rough, nb.math('MULTIPLY', nb.math('SUBTRACT', band, 0.5), 0.1))
    rough = nb.math('ADD', rough, nb.math('MULTIPLY', grime, 0.18))
    rough = nb.mixf(glyph, rough, 0.2)
    rough = nb.mixf(wear, rough, 0.22)
    rough = nb.math('MINIMUM', nb.math('MAXIMUM', rough, 0.06), 1.0)
    metal = nb.mixf(nb.math('MULTIPLY', grime, 0.3), STYLE['metal'], 0.3)
    metal = nb.mixf(nb.math('MAXIMUM', glyph, wear), metal, 1.0)
    grain = nb.node('ShaderNodeTexNoise')
    nb.link(pos, grain.inputs['Vector'])
    grain.inputs['Scale'].default_value = 14.0
    grain.inputs['Detail'].default_value = 3
    height = nb.math('SUBTRACT', nb.math('MULTIPLY', grain.outputs['Fac'], 0.03), glyph)
    bump = nb.node('ShaderNodeBump')
    bump.inputs['Strength'].default_value = STYLE['bump']
    bump.inputs['Distance'].default_value = 0.02
    nb.link(height, bump.inputs['Height'])

    bsdf = socks['bsdf']
    nb.link(col, bsdf.inputs['Base Color'])
    nb.link(rough, bsdf.inputs['Roughness'])
    nb.link(metal, bsdf.inputs['Metallic'])
    nb.link(bump.outputs['Normal'], bsdf.inputs['Normal'])
    socks.update(base=col, rough=rough, metal=metal, normal=bump.outputs['Normal'])
    return mats, socks


# ================================================================ geometry helpers

def cr(ctrl, n):
    """Catmull-Rom through tuples of floats: n + 1 samples."""
    pts = [tuple(float(c) for c in p) for p in ctrl]
    P = [pts[0]] + pts + [pts[-1]]
    segs = len(pts) - 1
    out = []
    for i in range(n + 1):
        u = i / n * segs
        k = min(int(u), segs - 1)
        t = u - k
        out.append(tuple(
            0.5 * (2 * b + (c - a) * t + (2 * a - 5 * b + 4 * c - d) * t * t + (3 * b - a - 3 * c + d) * t ** 3)
            for a, b, c, d in zip(P[k], P[k + 1], P[k + 2], P[k + 3])))
    return out


def rhythm(seed, n, on=(1, 1, 1, 2), gap=(1, 2, 3, 5, 8), start=0, end=None):
    """Light dashes along a channel: runs of `on` stations separated by Fibonacci gaps."""
    rnd = random.Random(seed)
    out = [False] * (n + 1)
    i = start + rnd.choice(gap)
    end = n if end is None else end
    while i < end:
        k = rnd.choice(on)
        for j in range(i, min(i + k, end)):
            out[j] = True
        i += k + rnd.choice(gap)
    return out


def spans(n, *ranges):
    out = [False] * (n + 1)
    for a, b in ranges:
        for j in range(max(0, int(a * n)), min(n, int(b * n)) + 1):
            out[j] = True
    return out


def fade(n, a, b):
    """An inlaid band from a to b (0..1) that ends by breaking up: shorter pieces, longer gaps."""
    out = spans(n, (a, b))
    for start, d in ((int(a * n), -1), (int(b * n), 1)):
        i = start
        for gap, run in ((1, 6), (2, 4), (3, 2), (5, 1)):
            for _ in range(gap):
                i += d
                if 0 < i < n:
                    out[i] = False
            for _ in range(run):
                i += d
                if 0 < i < n:
                    out[i] = True
    return out


def outline(poly):
    """poly: counter-clockwise [((x, z), spec)]; spec is the material of the edge leaving that point, or a list
    of channels engraved across that edge: (centre 0..1, width, depth, floor material). -> (points, materials)."""
    pts, mats = [], []
    n = len(poly)
    for i in range(n):
        (ax, az), spec = poly[i]
        bx, bz = poly[(i + 1) % n][0]
        pts.append((ax, az))
        if isinstance(spec, int):
            mats.append(spec)
            continue
        mats.append(H)
        ex, ez = bx - ax, bz - az
        ln = math.hypot(ex, ez) or 1e-9
        nx, nz = -ez / ln, ex / ln
        for fc, wd, depth, fm in spec:
            hw = min(wd, ln * 0.3 / len(spec)) / 2 / ln
            dd = min(depth, ln * 0.2)
            s = min(dd * 0.3 / ln, hw * 0.6)
            pts += [(ax + ex * (fc - hw), az + ez * (fc - hw)),
                    (ax + ex * (fc - hw + s) + nx * dd, az + ez * (fc - hw + s) + nz * dd),
                    (ax + ex * (fc + hw - s) + nx * dd, az + ez * (fc + hw - s) + nz * dd),
                    (ax + ex * (fc + hw), az + ez * (fc + hw))]
            mats += [H, fm, H, H]
    return pts, mats


def sweep(bm, rings, mats):
    def mf(i, k):
        m = mats[i] if mats[i] is not None else mats[i + 1]
        return m[k]
    return loft(bm, rings, matfn=mf)


def frames(ctrl, n, up=(0, 0, 1)):
    """Stations of a swept blade: (centre, chord dir, normal, tangent, half chord, half thickness)."""
    S = cr(ctrl, n)
    C = [V(s[:3]) for s in S]
    U = V(up).normalized()
    out = []
    for i, s in enumerate(S):
        T = (C[min(i + 1, n)] - C[max(i - 1, 0)]).normalized()
        D = T.cross(U).normalized()
        N = D.cross(T).normalized()
        if len(s) > 5 and abs(s[5]) > 1e-6:
            R = Matrix.Rotation(math.radians(s[5]), 3, T)
            D, N = R @ D, R @ N
        out.append((C[i], D, N, T, max(s[3], 0.012), max(s[4], 0.006)))
    return out


def blade(bm, ctrl, poly, n=64, up=(0, 0, 1)):
    """A swept blade. ctrl: (x, y, z, half_chord, half_thickness[, roll_deg]) along the centreline, the first and
    last are points. poly(a, h, i) -> outline spec in (chord, normal) units; +chord is centreline_dir x up."""
    F = frames(ctrl, n, up)
    rings, mats = [], []
    for i, (C, D, N, T, a, h) in enumerate(F):
        if i in (0, n):
            rings.append(C)
            mats.append(None)
            continue
        p2, m = outline(poly(a, h, i))
        rings.append([C + D * x + N * z for x, z in p2])
        mats.append(m)
    sweep(bm, rings, mats)
    return F


def torus(bm, c, axis, R, sec, n=48, phase=0.0):
    """sec: [(dr, da, mat)] closed section (radial, axial offsets); mat may be a function of the segment index."""
    axis = V(axis).normalized()
    ref = V((0, 0, 1)) if abs(axis.z) < 0.9 else V((1, 0, 0))
    u = axis.cross(ref).normalized()
    v = axis.cross(u).normalized()
    c = V(c)
    grid = []
    for i in range(n):
        th = phase + 2 * math.pi * i / n
        rad = u * math.cos(th) + v * math.sin(th)
        grid.append([bm.verts.new(c + rad * (R + dr) + axis * da) for dr, da, _ in sec])
    m = len(sec)
    for i in range(n):
        a, b = grid[i], grid[(i + 1) % n]
        for k in range(m):
            k2 = (k + 1) % m
            f = bm.faces.new((a[k], a[k2], b[k2], b[k]))
            mt = sec[k][2]
            f.material_index = mt(i) if callable(mt) else mt


def tube(bm, pts, r, n=6, mat=G, r1=None):
    pts = [V(p) for p in pts]
    rings = []
    for i, p in enumerate(pts):
        T = (pts[min(i + 1, len(pts) - 1)] - pts[max(i - 1, 0)]).normalized()
        rr = r if r1 is None else r + (r1 - r) * i / (len(pts) - 1)
        rings.append(circle_ring(p, T, rr, n))
    return loft(bm, rings, mat=mat)


def shard(bm, p0, p1, w, mat=GL, n=4, belly=0.3):
    """A crystal: bipyramid from p0 to p1, widest at `belly` of the way."""
    p0, p1 = V(p0), V(p1)
    mid = p0 + (p1 - p0) * belly
    loft(bm, [p0, circle_ring(mid, p1 - p0, w, n, phase=math.pi / 4), p1], mat=mat)


def caged_star(bm, c, r):
    """Geodesic orb: luminous facets behind a dark lattice."""
    ret = bmesh.ops.create_icosphere(bm, subdivisions=3, radius=r, matrix=Matrix.Translation(V(c)))
    faces = list({f for v in ret['verts'] for f in v.link_faces})
    for f in faces:
        f.material_index = H
    bmesh.ops.inset_individual(bm, faces=faces, thickness=r * 0.022, depth=-r * 0.03, use_even_offset=True)
    for f in faces:
        if f.is_valid:
            f.material_index = G


def drive(bm, c, R, horn=3.8):
    """A ring aperture: the mouth of a long hyperbolic horn (its thorn points at the bow), a knife-lipped ring
    around it, a lit dish deep inside, stator vanes across the light."""
    c = V(c)
    k = R / 2.2
    Ln = horn * R
    sec = [(0.0, 0.95 * k, H), (0.22 * k, 0.2 * k, H), (0.25 * k, -0.5 * k, H), (0.1 * k, -0.85 * k, H),
           (-0.04 * k, -0.82 * k, G), (-0.12 * k, -0.55 * k, H), (-0.16 * k, 0.0, H), (-0.12 * k, 0.5 * k, H)]
    torus(bm, c, (0, 1, 0), R, sec, n=72)
    prof = [(0.0, -Ln)]
    mats = []
    for q in range(1, 15):
        t = q / 15
        prof.append((R * (0.012 + 0.9 * t ** 1.9), -Ln * (1 - t)))
        mats.append(H)
    prof += [(R - 0.08 * k, 0.0), (R - 0.08 * k, 0.12 * k), (0.62 * R, 0.4 * k), (0.22 * R, 0.55 * k), (0.0, 0.45 * k)]
    mats += [H, H, E, E, E]
    lathe(bm, c, (0, -1, 0), prof, n=72, mats=mats)
    for j in range(18):
        th = 2 * math.pi * (j + 0.5) / 18
        rad = V((math.cos(th), 0, math.sin(th)))
        p = c + V((0, -0.62 * k, 0))
        prism(bm, p + rad * (0.2 * R), p + rad * (R - 0.1 * k), 0.02 * k, 0.12 * k, w1=0.028 * k, h1=0.17 * k,
              up=(0, 1, 0), chamfer=0.6)


# ================================================================ the ship

# split lance, right half: y, inner x (gap), width, crown height, keel depth, centre z
SHELL = [
    (28.2, 0.55, 0.32, 0.3, 0.25, -0.1),
    (23.0, 0.58, 0.9, 0.75, 0.65, -0.06),
    (17.0, 0.6, 1.5, 1.25, 1.1, 0.0),
    (11.0, 0.72, 2.3, 1.85, 1.7, 0.05),
    (5.5, 1.5, 3.0, 1.9, 2.4, 0.1),
    (0.0, 2.6, 3.4, 1.8, 2.9, 0.12),
    (-5.5, 1.6, 3.3, 3.5, 3.1, 0.12),
    (-11.0, 0.76, 2.7, 3.5, 2.7, 0.1),
    (-17.0, 0.6, 1.8, 2.5, 1.9, 0.06),
    (-22.0, 0.58, 1.0, 1.4, 1.0, 0.03),
    (-25.0, 0.56, 0.45, 0.55, 0.45, 0.0),
]
NOSE = (0.6, 30.0, -0.1)
TAIL = (0.6, -25.55, 0.0)
CORE = (0.0, 0.0, 0.45)
DRIVE = (5.15, -25.2, 0.15)
DRIVE_R = 2.2
MID_DRIVE = (0.0, -25.4, 0.0)
MID_R = 1.35

# crescent blade, right: x, y, z, half chord, half thickness, roll
CRESCENT = [
    (7.2, -30.0, 0.5, 0.0, 0.0, 0),
    (8.3, -25.0, 0.4, 1.35, 0.42, 0),
    (9.5, -17.0, 0.25, 2.45, 0.7, -4),
    (10.1, -7.0, 0.0, 2.85, 0.85, -8),
    (10.0, 3.0, -0.5, 2.55, 0.75, -12),
    (9.2, 11.0, -1.1, 1.8, 0.55, -18),
    (7.9, 17.5, -1.9, 0.95, 0.32, -25),
    (6.6, 22.5, -2.6, 0.0, 0.0, -30),
]


def shell_at(y):
    st = sorted(SHELL)
    if y <= st[0][0]:
        return st[0]
    for a, b in zip(st, st[1:]):
        if a[0] <= y <= b[0]:
            t = (y - a[0]) / (b[0] - a[0])
            return tuple(p + (q - p) * t for p, q in zip(a, b))
    return st[-1]


def gap_at(y):
    return max(shell_at(y)[1], 0.54)


def build(ship):
    def lit(flags, i):
        return G if flags[i] else H

    ONE = (1,)

    # ---------------------------------------------------------------- the split lance
    def shell(bm):
        n = 300
        S = cr(SHELL, n)
        low = rhythm(11, n, on=ONE)
        low2 = rhythm(14, n, on=ONE, gap=(2, 3, 5, 8, 13))
        up1 = rhythm(12, n, on=ONE, gap=(1, 1, 2, 3, 5))
        up2 = rhythm(13, n, on=ONE, gap=(1, 2, 3, 5, 8))
        up3 = rhythm(15, n, on=(1, 2), gap=(3, 5, 8, 13))
        bays = rhythm(16, n, on=(2,), gap=(1, 1, 1, 2, 9, 14), start=int(n * 0.3), end=int(n * 0.84))
        slit = spans(n, (0.04, 0.3), (0.4, 0.6), (0.66, 0.9))
        hangar = spans(n, (0.455, 0.475), (0.56, 0.585), (0.61, 0.625))
        crown = fade(n, 0.28, 0.76)
        chine = fade(n, 0.4, 0.66)
        rings, mats = [V(NOSE)], [None]
        for i, (y, g, w, zt, zb, cz) in enumerate(S):
            g, w = max(g, 0.54), max(w, 0.05)
            poly = [
                ((g, cz - 0.72 * zb), H),
                ((g + 0.4 * w, cz - zb), [(0.36, 0.045, 0.035, lit(low, i)), (0.68, 0.045, 0.035, lit(low2, i))]),
                ((g + 0.93 * w, cz - 0.45 * zb), L if chine[i] else H),
                ((g + w, cz - 0.05 * zt), [(0.5, 0.55, 0.32, G)] if hangar[i] else [(0.5, 0.16, 0.2, lit(bays, i))]),
                ((g + 0.9 * w, cz + 0.36 * zt), H),
                ((g + 0.8 * w, cz + 0.4 * zt), [(0.22, 0.045, 0.035, lit(up1, i)), (0.5, 0.045, 0.035, lit(up2, i)),
                                                (0.78, 0.045, 0.035, lit(up3, i))]),
                ((g + 0.5 * w, cz + 0.93 * zt), H),
                ((g + 0.4 * w, cz + zt), L if crown[i] else H),
                ((g + 0.07 * w, cz + 0.86 * zt), H),
                ((g, cz + 0.76 * zt), [(0.5, 0.12, 0.06, lit(slit, i))]),
            ]
            p2, m = outline(poly)
            rings.append([V((x, y, z)) for x, z in p2])
            mats.append(m)
        rings.append(V(TAIL))
        mats.append(None)
        sweep(bm, rings, mats)
        # pendants under the keel: crystals hanging aft on spines, longest under the eye
        for j, y in enumerate((12.0, 9.2, 6.6, 4.2, 2.0, 0.0, -2.2, -4.6, -7.2, -10.0, -13.0)):
            _, g, w, zt, zb, cz = shell_at(y)
            s = 0.45 + 0.75 * math.sin(math.pi * (j + 0.5) / 11)
            root = V((g + 0.42 * w, y, cz - zb + 0.05))
            p = root + V((0.0, -0.5 * s, -0.7 * s))
            tube(bm, [root, p], 0.02, n=5, mat=H)
            shard(bm, p + V((0, 0.3 * s, 0.25 * s)), p + V((0.0, -1.3 * s, -1.6 * s)), 0.12 * s, belly=0.3)
        # prow emitter crystal
        shard(bm, (NOSE[0], 29.3, NOSE[2]), (NOSE[0], 30.0, NOSE[2]), 0.09, belly=0.4)
    ship.part(shell, bevel=0.0, mirror=True)

    # ---------------------------------------------------------------- the core: needle, caged star, armillary, rungs
    def core(bm):
        z = CORE[2]
        beads = (24.0, 18.5, 13.5, 8.5, 4.6, -4.6, -8.5, -13.0, -17.5, -21.5)
        prof = [(0.0, -24.2)]
        for b in sorted(beads):
            prof += [(0.1, b - 1.0), (0.4, b - 0.25), (0.4, b + 0.25), (0.1, b + 1.0)]
        prof.append((0.0, 27.5))
        lathe(bm, (0, 0, z), (0, 1, 0), prof, n=6, mat=GL)
        caged_star(bm, CORE, 1.6)
        sec = [(0.1, 0.0, H), (0.0, 0.13, H), (-0.1, 0.0, H), (0.0, -0.13, H)]
        for a in (90, 30, 150):
            ax = (math.cos(math.radians(a)), 0, math.sin(math.radians(a)))
            torus(bm, CORE, ax, 2.42, sec, n=72)
        # rungs of light across the prong slit, closing up toward the prow
        y = 11.5
        step = 2.4
        while y < 28.0:
            g = gap_at(y) + 0.06
            tube(bm, [(-g, y, z - 0.3), (g, y, z - 0.3)], 0.03, n=6, mat=G)
            y += step
            step = max(0.5, step * 0.82)
        for y in (-12.0, -14.0, -16.4, -19.4, -23.0):
            g = gap_at(y) + 0.06
            tube(bm, [(-g, y, z - 0.3), (g, y, z - 0.3)], 0.03, n=6, mat=G)
        # solid ties from the beads to the hull halves
        for y in (8.5, 4.6, -4.6, -8.5):
            g = gap_at(y) + 0.15
            for s in (1, -1):
                shard(bm, (0, y, z), (s * g, y, z - 0.2), 0.12, mat=H, belly=0.5)
    ship.part(core, bevel=0.0, angle=40)

    # ---------------------------------------------------------------- crescent blades
    NC = 210
    c_low = rhythm(21, NC, on=ONE)
    c_low2 = rhythm(24, NC, on=ONE, gap=(2, 3, 5, 8))
    c_a = rhythm(22, NC, on=ONE, gap=(1, 2, 3, 5))
    c_b = rhythm(23, NC, on=ONE, gap=(1, 1, 2, 3, 5, 8))
    c_vein = spans(NC, (0.16, 0.3), (0.34, 0.5), (0.54, 0.62), (0.66, 0.84))
    c_liv = fade(NC, 0.14, 0.88)

    def crescent_poly(a, h, i):
        return [
            ((-a, 0.0), H),
            ((-0.35 * a, -0.8 * h), [(0.3, 0.045, 0.035, lit(c_low, i)), (0.7, 0.045, 0.035, lit(c_low2, i))]),
            ((0.45 * a, -0.6 * h), H),
            ((a, 0.0), [(0.22, 0.045, 0.035, lit(c_a, i)), (0.45, 0.045, 0.035, lit(c_b, i)),
                        (0.7, 0.07, 0.05, lit(c_vein, i))]),
            ((0.05 * a, h), [(0.5, 0.05, 0.04, H)]),
            ((-0.56 * a, 0.7 * h), H),
            ((-0.59 * a, 0.58 * h), L if c_liv[i] else H),
        ]

    def crescents(bm):
        F = blade(bm, CRESCENT, crescent_poly, n=NC)
        # fang crystal
        C, D, N, T, a, h = F[NC - 2]
        shard(bm, C - T * 0.5, C + T * 2.4, 0.16, belly=0.2)
        # satellite shards flying off the outer edge on spines, largest amidships
        for j, i in enumerate(range(34, 160, 10)):
            C, D, N, T, a, h = F[i]
            s = 0.5 + 0.7 * math.sin(math.pi * (j + 0.5) / 13)
            p = C + D * (a + 0.4 + 0.3 * s) - N * 0.05
            shard(bm, p - T * 1.5 * s, p + T * 1.1 * s, 0.12 * s, belly=0.62)
            tube(bm, [C + D * (a - 0.06), p], 0.02, n=5, mat=H)
    ship.part(crescents, bevel=0.0, mirror=True)
    Cf = frames(CRESCENT, NC)[NC - 2]
    tipg = Cf[0] + Cf[3] * 2.4
    ship.empty("Gun_0", NOSE, 0.2)
    ship.empty("Gun_1", (-NOSE[0], NOSE[1], NOSE[2]), 0.2)
    ship.empty("Gun_2", tipg, 0.2)
    ship.empty("Gun_3", (-tipg.x, tipg.y, tipg.z), 0.2)

    # ---------------------------------------------------------------- spars and bridges: lance -> blades, lance -> drives
    def simple_poly(flags=None):
        def poly(a, h, i):
            fm = G if (flags and flags[i]) else H
            return [((-a, 0.0), H), ((0.0, -0.8 * h), H), ((a, 0.0), [(0.5, 0.05, 0.04, fm)]), ((0.0, h), H)]
        return poly

    def spars(bm):
        blade(bm, [(5.6, 1.0, 0.1, 0.0, 0.0), (6.4, 2.4, 0.0, 0.5, 0.2), (7.4, 4.2, -0.2, 0.4, 0.16),
                   (8.4, 6.0, -0.5, 0.0, 0.0)], simple_poly(rhythm(32, 60, on=ONE, gap=(1, 2))), n=60)
        blade(bm, [(5.2, -3.0, 0.1, 0.0, 0.0), (6.3, -5.0, 0.05, 0.55, 0.2), (7.5, -7.6, 0.0, 0.45, 0.16),
                   (8.6, -10.0, 0.0, 0.0, 0.0)], simple_poly(rhythm(33, 60, on=ONE, gap=(1, 2))), n=60)
        # yoke: lance to the drive ring, ring to the blade
        blade(bm, [(2.6, -12.0, 0.1, 0.0, 0.0), (2.75, -17.0, 0.12, 0.5, 0.24), (2.85, -22.0, 0.15, 0.42, 0.2),
                   (2.9, -25.6, 0.15, 0.0, 0.0)], simple_poly(rhythm(31, 80, on=ONE)), n=80)
        blade(bm, [(7.0, -26.0, 0.2, 0.0, 0.0), (7.6, -24.6, 0.3, 0.4, 0.16), (8.3, -22.6, 0.36, 0.0, 0.0)],
              simple_poly(), n=24)
        # threads of light across the cradle gap
        for y in (-1.6, 0.0, 1.6):
            sh = shell_at(y)
            tube(bm, [(sh[1] + sh[2] - 0.1, y, 0.0), (7.5, y * 1.25, -0.2)], 0.025, n=5, mat=G)
    ship.part(spars, bevel=0.0, mirror=True)

    # ---------------------------------------------------------------- drives
    def drives(bm):
        drive(bm, DRIVE, DRIVE_R)
    ship.part(drives, bevel=0.0, mirror=True)

    def mid_drive(bm):
        drive(bm, MID_DRIVE, MID_R, horn=2.6)
    ship.part(mid_drive, bevel=0.0)
    k = DRIVE_R / 2.2
    ship.empty("Engine_0", (DRIVE[0], DRIVE[1] - 0.85 * k, DRIVE[2]), DRIVE_R * 0.86)
    ship.empty("Engine_1", (-DRIVE[0], DRIVE[1] - 0.85 * k, DRIVE[2]), DRIVE_R * 0.86)
    ship.empty("Engine_2", (MID_DRIVE[0], MID_DRIVE[1] - 0.85 * MID_R / 2.2, MID_DRIVE[2]), MID_R * 0.86)

    # ---------------------------------------------------------------- dorsal and ventral great shards
    NF = 110
    f_a = rhythm(41, NF, on=ONE)
    f_b = rhythm(42, NF, on=ONE, gap=(1, 2, 3, 5))
    f_liv = fade(NF, 0.3, 0.8)

    def fin_poly(a, h, i):
        return [((-a, 0.0), H), ((0.1 * a, -h), [(0.45, 0.045, 0.035, lit(f_a, i))]),
                ((a, 0.0), [(0.3, 0.045, 0.035, lit(f_b, i)), (0.6, 0.045, 0.035, lit(f_a, NF - i))]),
                ((0.1 * a, h), [(0.25, 0.05, 0.04, H)]),
                ((-0.36 * a, 0.56 * h), L if f_liv[i] else H)]

    def fins(bm):
        c = math.radians(14)
        blade(bm, [(3.7, -0.5, 1.9, 0.0, 0.0), (3.5, -3.5, 3.7, 1.3, 0.2), (3.35, -8.0, 6.9, 2.2, 0.28),
                   (3.35, -13.5, 9.2, 1.8, 0.24), (3.6, -19.0, 10.5, 0.9, 0.13), (3.9, -23.5, 11.1, 0.0, 0.0)],
              fin_poly, n=NF, up=(math.cos(c), 0, -math.sin(c)))
        c = math.radians(-14)
        blade(bm, [(2.2, 9.0, -1.7, 0.0, 0.0), (3.4, 3.0, -3.5, 1.5, 0.2), (3.5, -2.0, -6.0, 2.0, 0.25),
                   (3.4, -9.0, -7.8, 1.4, 0.2), (3.6, -17.0, -8.8, 0.0, 0.0)],
              fin_poly, n=NF, up=(-math.cos(c), 0, math.sin(c)))
        for tip, d in (((3.9, -23.5, 11.1), (0.08, -0.95, 0.3)), ((3.6, -17.0, -8.8), (0.03, -0.97, -0.22))):
            tip, d = V(tip), V(d).normalized()
            shard(bm, tip - d * 0.6, tip + d * 1.6, 0.13, belly=0.3)
    ship.part(fins, bevel=0.0, mirror=True)
    g = 4
    for tip, d in (((3.9, -23.5, 11.1), (0.08, -0.95, 0.3)), ((3.6, -17.0, -8.8), (0.03, -0.97, -0.22))):
        e = V(tip) + V(d).normalized() * 1.6
        for sx in (1, -1):
            ship.empty("Gun_%d" % g, (sx * e.x, e.y, e.z), 0.2)
            g += 1

    # ---------------------------------------------------------------- report: paint share by area
    area = [0.0] * 5
    for o in ship.groups.get("body", []):
        for p in o.data.polygons:
            area[min(p.material_index, 4)] += p.area
    tot = sum(area) or 1.0
    shiplib.log("unknown area share", {nm: "%.1f%%" % (100 * a / tot) for nm, a in zip(shiplib.MAT_NAMES, area)})

"""`unknown_small` — the Aster Warden: the leviathan's makers' small sentinel (60 units long, standing for ~26 m).

One shape: a caged star held in the eye of a split obsidian dart, between two talon blades that reach forward
past it. The dart is two half-hulls that never touch (a crystal needle and a few rungs of light bridge the slit),
the talons hang from a single spine each and from the ring drives' yokes, back-swept shards stand above and
below the drives. No cockpit, no windows: everything the leviathan says in hundreds of dashes is said here in a dozen
long ones, so it still reads at 26 m and in a pack.
"""
import math

import bmesh
from mathutils import Matrix, Vector

import shiplib
from shiplib import E, G, GL, H, L, lathe, prism
from styles import unknown as cap

V = Vector

STYLE = cap.STYLE
TINT = cap.TINT

ENGRAVE = 1.6  # > 1: the leviathan's engraved maze drawn that many times coarser on this hull


def materials():
    """The leviathan's pearlescent hull and engraving; the maze can be drawn coarser for a hull this small."""
    mats, socks = cap.materials()
    if abs(ENGRAVE - 1.0) > 1e-3:
        nt = mats["Hull"].node_tree
        for node in nt.nodes:
            if node.bl_idname != 'ShaderNodeSeparateXYZ':
                continue
            links = list(node.inputs[0].links)
            if not links or links[0].from_node.bl_idname != 'ShaderNodeTexCoord':
                continue
            src = links[0].from_socket
            sc = nt.nodes.new('ShaderNodeVectorMath')
            sc.operation = 'SCALE'
            sc.inputs['Scale'].default_value = 1.0 / ENGRAVE
            nt.links.new(src, sc.inputs[0])
            nt.links.new(sc.outputs['Vector'], node.inputs[0])
    return mats, socks


# ================================================================ the ship

# split dart, right half: y, inner x (gap), width, crown height, keel depth, centre z
SHELL = [
    (28.6, 0.62, 0.4, 0.35, 0.3, 0.0),
    (22.0, 0.72, 1.35, 0.95, 0.75, 0.0),
    (14.0, 0.8, 2.4, 1.6, 1.3, 0.0),
    (6.5, 1.3, 3.2, 2.1, 1.9, 0.0),
    (1.0, 3.3, 3.3, 2.2, 2.3, 0.0),
    (-4.0, 4.25, 3.1, 2.4, 2.5, 0.0),
    (-9.0, 3.3, 3.1, 3.0, 2.4, 0.0),
    (-15.0, 1.1, 2.8, 2.8, 2.0, 0.0),
    (-21.0, 0.6, 1.7, 1.7, 1.2, 0.0),
    (-25.5, 0.5, 0.6, 0.6, 0.5, 0.0),
]
NOSE = (0.68, 30.0, 0.0)
TAIL = (0.55, -26.6, 0.0)
CORE = (0.0, -4.0, 0.8)
CORE_R = 2.9
DRIVE = (8.4, -25.5, 0.5)
DRIVE_R = 2.2

# talon blade, right: x, y, z, half chord, half thickness, roll
TALON = [
    (10.5, -30.0, 2.6, 0.0, 0.0, 0),
    (11.9, -28.0, 2.5, 0.7, 0.24, 4),
    (13.6, -25.0, 2.2, 1.35, 0.34, 8),
    (17.0, -16.0, 1.2, 2.9, 0.62, 16),
    (18.5, -5.0, -0.4, 3.3, 0.7, 24),
    (17.6, 5.0, -2.4, 2.7, 0.6, 32),
    (15.0, 13.5, -4.6, 1.7, 0.5, 40),
    (11.9, 19.5, -6.6, 0.8, 0.28, 48),
    (9.6, 23.0, -7.8, 0.0, 0.0, 52),
]

# dorsal shard, right: x, y, z, half chord, half thickness
KEEL = [(4.6, -2.0, -2.0, 0.0, 0.0), (5.0, -6.5, -3.6, 1.3, 0.2), (5.4, -12.0, -5.4, 1.8, 0.26),
        (6.0, -17.5, -6.6, 1.2, 0.2), (6.7, -22.5, -7.4, 0.0, 0.0)]
FIN = [(4.4, -5.0, 2.0, 0.0, 0.0), (4.9, -10.0, 4.0, 1.5, 0.22), (5.5, -16.0, 6.6, 2.2, 0.3),
       (6.3, -22.0, 8.4, 1.5, 0.22), (7.2, -28.0, 9.6, 0.0, 0.0)]


def shell_at(y):
    st = sorted(SHELL)
    if y <= st[0][0]:
        return st[0]
    for a, b in zip(st, st[1:]):
        if a[0] <= y <= b[0]:
            t = (y - a[0]) / (b[0] - a[0])
            return tuple(p + (q - p) * t for p, q in zip(a, b))
    return st[-1]


def caged_star(bm, c, r, sub=2):
    """The leviathan's geodesic orb with fewer, larger facets."""
    ret = bmesh.ops.create_icosphere(bm, subdivisions=sub, radius=r, matrix=Matrix.Translation(V(c)))
    faces = list({f for v in ret['verts'] for f in v.link_faces})
    for f in faces:
        f.material_index = H
    bmesh.ops.inset_individual(bm, faces=faces, thickness=r * 0.04, depth=-r * 0.04, use_even_offset=True)
    for f in faces:
        if f.is_valid:
            f.material_index = G


def drive(bm, c, R, horn=3.0, n=32, vanes=8):
    """The leviathan's ring aperture (horn, knife-lipped ring, lit dish, stator vanes) at a small ship's budget."""
    c = V(c)
    k = R / 2.2
    Ln = horn * R
    sec = [(0.0, 0.95 * k, H), (0.22 * k, 0.2 * k, H), (0.25 * k, -0.5 * k, H), (0.1 * k, -0.85 * k, H),
           (-0.04 * k, -0.82 * k, G), (-0.12 * k, -0.55 * k, H), (-0.16 * k, 0.0, H), (-0.12 * k, 0.5 * k, H)]
    cap.torus(bm, c, (0, 1, 0), R, sec, n=n)
    prof = [(0.0, -Ln)]
    mats = []
    for q in range(1, 9):
        t = q / 9
        prof.append((R * (0.012 + 0.9 * t ** 1.9), -Ln * (1 - t)))
        mats.append(H)
    prof += [(R - 0.08 * k, 0.0), (R - 0.08 * k, 0.12 * k), (0.62 * R, 0.4 * k), (0.22 * R, 0.55 * k), (0.0, 0.45 * k)]
    mats += [H, H, E, E, E]
    lathe(bm, c, (0, -1, 0), prof, n=n, mats=mats)
    for j in range(vanes):
        th = 2 * math.pi * (j + 0.5) / vanes
        rad = V((math.cos(th), 0, math.sin(th)))
        p = c + V((0, -0.62 * k, 0))
        prism(bm, p + rad * (0.2 * R), p + rad * (R - 0.1 * k), 0.035 * k, 0.12 * k, w1=0.05 * k, h1=0.17 * k,
              up=(0, 1, 0), chamfer=0.6)


def build(ship):
    def lit(flags, i):
        return G if flags[i] else H

    # ---------------------------------------------------------------- the split dart
    def shell(bm):
        n = 64
        S = cap.cr(SHELL, n)
        low = cap.rhythm(11, n, on=(2,), gap=(8, 13))
        bays = cap.spans(n, (0.6, 0.68))
        up1 = cap.spans(n, (0.12, 0.2), (0.27, 0.3), (0.66, 0.74))
        slit = cap.spans(n, (0.05, 0.3), (0.76, 0.92))
        crown = cap.spans(n, (0.1, 0.16), (0.2, 0.74), (0.78, 0.82))
        chine = cap.spans(n, (0.2, 0.8))
        rings, mats = [V(NOSE)], [None]
        for i, (y, g, w, zt, zb, cz) in enumerate(S):
            g, w = max(g, 0.5), max(w, 0.05)
            poly = [
                ((g, cz - 0.72 * zb), H),
                ((g + 0.4 * w, cz - zb), [(0.5, 0.14, 0.08, lit(low, i))]),
                ((g + 0.93 * w, cz - 0.45 * zb), L if chine[i] else H),
                ((g + w, cz - 0.05 * zt), [(0.5, 0.26, 0.16, lit(bays, i))]),
                ((g + 0.9 * w, cz + 0.36 * zt), H),
                ((g + 0.8 * w, cz + 0.4 * zt), [(0.36, 0.13, 0.08, lit(up1, i)), (0.72, 0.1, 0.07, H)]),
                ((g + 0.5 * w, cz + 0.93 * zt), H),
                ((g + 0.4 * w, cz + zt), L if crown[i] else H),
                ((g + 0.07 * w, cz + 0.86 * zt), H),
                ((g, cz + 0.76 * zt), [(0.5, 0.22, 0.08, lit(slit, i))]),
            ]
            p2, m = cap.outline(poly)
            rings.append([V((x, y, z)) for x, z in p2])
            mats.append(m)
        rings.append(V(TAIL))
        mats.append(None)
        cap.sweep(bm, rings, mats)
        cap.shard(bm, (NOSE[0], 28.8, NOSE[2]), (NOSE[0], 30.0, NOSE[2]), 0.16, belly=0.4)
    ship.part(shell, bevel=0.0, mirror=True)

    # ---------------------------------------------------------------- the core: needle, caged star, armillary, rungs
    def core(bm):
        z = CORE[2]
        beads = (24.0, 17.0, 10.5, -14.0, -19.5)
        prof = [(0.0, -24.5)]
        for b in sorted(beads):
            prof += [(0.12, b - 1.6), (0.36, b - 0.4), (0.36, b + 0.4), (0.12, b + 1.6)]
        prof.append((0.0, 28.0))
        lathe(bm, (0, 0, 0.3), (0, 1, 0), prof, n=6, mat=GL)
        caged_star(bm, CORE, CORE_R)
        sec = [(0.16, 0.0, H), (0.0, 0.2, H), (-0.16, 0.0, H), (0.0, -0.2, H)]
        for a in (90, 28, 152):
            ax = (math.cos(math.radians(a)), 0, math.sin(math.radians(a)))
            cap.torus(bm, CORE, ax, CORE_R + 0.75, sec, n=48)
        for y in (13.5, 18.0, 21.6, 24.4, 26.6, -17.0, -21.0, -24.0):
            g = max(shell_at(y)[1], 0.5) + 0.08
            cap.tube(bm, [(-g, y, 0.05), (g, y, 0.05)], 0.055, n=6, mat=G)
        for s in (1, -1):
            g = shell_at(CORE[1])[1] + 0.2
            cap.shard(bm, (s * (CORE_R + 0.5), CORE[1], z), (s * g, CORE[1], 0.3), 0.22, mat=H, belly=0.5)
    ship.part(core, bevel=0.0, angle=40)

    # ---------------------------------------------------------------- talon blades
    NC = 56
    c_low = cap.spans(NC, (0.36, 0.42))
    c_a = cap.spans(NC, (0.16, 0.2), (0.3, 0.4))
    c_vein = cap.spans(NC, (0.5, 0.92))
    c_liv = cap.spans(NC, (0.1, 0.14), (0.18, 0.8), (0.84, 0.88))

    def talon_poly(a, h, i):
        return [
            ((-a, 0.0), H),
            ((-0.35 * a, -0.8 * h), [(0.5, 0.14, 0.08, lit(c_low, i))]),
            ((0.45 * a, -0.6 * h), H),
            ((a, 0.0), [(0.3, 0.13, 0.08, lit(c_a, i)), (0.66, 0.18, 0.1, lit(c_vein, i))]),
            ((0.05 * a, h), [(0.5, 0.12, 0.08, H)]),
            ((-0.56 * a, 0.7 * h), H),
            ((-0.6 * a, 0.56 * h), L if c_liv[i] else H),
        ]

    def talons(bm):
        F = cap.blade(bm, TALON, talon_poly, n=NC)
        C, D, N, T, a, h = F[NC - 2]
        cap.shard(bm, C - T * 1.0, C + T * 3.0, 0.3, belly=0.25)
    ship.part(talons, bevel=0.0, mirror=True)
    Tf = cap.frames(TALON, NC)[NC - 2]
    tip = Tf[0] + Tf[3] * 3.0
    ship.empty("Gun_0", tip, 0.3)
    ship.empty("Gun_1", (-tip.x, tip.y, tip.z), 0.3)

    # ---------------------------------------------------------------- spines: dart -> talons, dart -> drives
    def simple_poly(flags=None):
        def poly(a, h, i):
            fm = G if (flags and flags[i]) else H
            return [((-a, 0.0), H), ((0.0, -0.8 * h), H), ((a, 0.0), [(0.5, 0.1, 0.07, fm)]), ((0.0, h), H)]
        return poly

    def spines(bm):
        cap.blade(bm, [(6.6, -3.0, 0.0, 0.0, 0.0), (9.4, -4.4, 0.0, 0.55, 0.24), (13.0, -6.0, -0.1, 0.45, 0.2),
                       (16.0, -7.2, -0.2, 0.0, 0.0)], simple_poly(cap.spans(24, (0.25, 0.75))), n=24)
        cap.blade(bm, [(3.4, -12.0, 0.2, 0.0, 0.0), (4.9, -17.0, 0.3, 0.6, 0.26), (5.9, -22.0, 0.4, 0.5, 0.22),
                       (6.4, -26.2, 0.5, 0.0, 0.0)], simple_poly(), n=24)
        cap.blade(bm, [(10.2, -26.4, 0.5, 0.0, 0.0), (11.3, -25.2, 0.7, 0.45, 0.2), (12.6, -23.4, 0.9, 0.0, 0.0)],
                  simple_poly(), n=10)
        for y0, y1 in ((0.5, 4.0), (-8.5, -15.0)):
            sh = shell_at(y0)
            cap.tube(bm, [(sh[1] + sh[2] - 0.1, y0, 0.0), (15.4, y1, -0.5)], 0.045, n=5, mat=G)
    ship.part(spines, bevel=0.0, mirror=True)

    # ---------------------------------------------------------------- drives
    def drives(bm):
        drive(bm, DRIVE, DRIVE_R)
    ship.part(drives, bevel=0.0, mirror=True)
    k = DRIVE_R / 2.2
    ship.empty("Engine_0", (DRIVE[0], DRIVE[1] - 0.85 * k, DRIVE[2]), DRIVE_R * 0.86)
    ship.empty("Engine_1", (-DRIVE[0], DRIVE[1] - 0.85 * k, DRIVE[2]), DRIVE_R * 0.86)

    # ---------------------------------------------------------------- dorsal shards
    NF = 36
    f_a = cap.spans(NF, (0.5, 0.72))
    f_liv = cap.spans(NF, (0.22, 0.84))

    def fin_poly(a, h, i):
        return [((-a, 0.0), H), ((0.1 * a, -h), [(0.45, 0.12, 0.07, H)]),
                ((a, 0.0), [(0.45, 0.12, 0.07, lit(f_a, i))]),
                ((0.1 * a, h), H),
                ((-0.36 * a, 0.56 * h), L if f_liv[i] else H)]

    def fins(bm):
        c = math.radians(14)
        cap.blade(bm, FIN, fin_poly, n=NF, up=(math.cos(c), 0, -math.sin(c)))
        c = math.radians(-14)
        cap.blade(bm, KEEL, fin_poly, n=NF, up=(-math.cos(c), 0, math.sin(c)))
        for tip, d in ((FIN[-1][:3], (0.12, -0.95, 0.25)), (KEEL[-1][:3], (0.1, -0.96, -0.2))):
            tip, d = V(tip), V(d).normalized()
            cap.shard(bm, tip - d * 0.8, tip + d * 1.6, 0.2, belly=0.3)
    ship.part(fins, bevel=0.0, mirror=True)

    # ---------------------------------------------------------------- report: paint share by area
    area = [0.0] * 5
    for o in ship.groups.get("body", []):
        for p in o.data.polygons:
            area[min(p.material_index, 4)] += p.area
    tot = sum(area) or 1.0
    shiplib.log("unknown_small area share", {nm: "%.1f%%" % (100 * a / tot) for nm, a in zip(shiplib.MAT_NAMES, area)})

"""Build STARFALL DYNASTY enemy ships + environment props -> GLB + preview renders.

Usage (from repo root):
  "<blender.exe>" -b --factory-startup -P tools/blender/build_props.py -- [names...] [--no-bake] [--no-preview] [--tex 2048]
Names: raider gunship warden warlord sentinel station asteroids wreck gate ruin (default: all)
Outputs: crates/client/assets/models/<file>.glb, tools/blender/previews/props_<name>.png
"""
import bpy
import bmesh
import math
import os
import random
import sys
import time
from mathutils import Vector, Matrix, noise

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import importlib
import shiplib as S
import proplib as P
importlib.reload(S)
importlib.reload(P)
from shiplib import V, H, L, G, GL, E, loft, ring10, circle_ring, lathe, box, prism, cyl, sphere, wing, log
from proplib import (Prop, slab, jag, nozzle, greebles, sample_surface, cable, frame_axes, rot_tbn,
                     flat_mat, boolean, solidify)

REPO = os.path.abspath(os.path.join(HERE, "..", ".."))
MODELS = os.path.join(REPO, "crates", "client", "assets", "models")
PREVIEWS = os.path.join(HERE, "previews")
BUILD = os.path.join(HERE, "_build_props")
VIEWS = os.path.join(BUILD, "views")

OPTS = dict(bake=True, preview=True, tex=2048)
REPORT = {}


def lerp(a, b, t):
    return a + (b - a) * t


def clamp(x, a, b):
    return max(a, min(b, x))


def report(name, objs, path):
    n, per = P.tris(objs)
    REPORT[name] = (n, per, os.path.getsize(path) / 1e6 if path and os.path.exists(path) else 0)
    log("REPORT", name, n, per, "%.2f MB" % REPORT[name][2])


def tint_preview(mats, livery=(0.55, 0.05, 0.04), glow=(1.0, 0.35, 0.18), core=None, glow_strength=None):
    if "Livery" in mats:
        mats["Livery"].node_tree.nodes["Principled BSDF"].inputs['Base Color'].default_value = (*livery, 1)
    if "Glow" in mats:
        p = mats["Glow"].node_tree.nodes["Principled BSDF"]
        p.inputs['Emission Color'].default_value = (*glow, 1)
        if glow_strength:
            p.inputs['Emission Strength'].default_value = glow_strength
    if core and "EngineCore" in mats:
        mats["EngineCore"].node_tree.nodes["Principled BSDF"].inputs['Emission Color'].default_value = (*core, 1)


def preview_scene(key=3.5, rim=2.5, fill=0.6):
    cam = S.setup_preview_scene()
    bpy.data.objects["Key"].data.energy = key
    bpy.data.objects["Rim"].data.energy = rim
    bpy.data.objects["Fill"].data.energy = fill
    return cam


# ================================================================ C. asteroids

def rock_shape(bm, seed, radius=2.0, nveins=0):
    rng = random.Random(seed)
    off = V((rng.uniform(-50, 50), rng.uniform(-50, 50), rng.uniform(-50, 50)))
    stretch = V((rng.uniform(0.85, 1.25), rng.uniform(0.75, 1.15), rng.uniform(0.65, 0.95)))
    # fracture planes -> chipped facets
    planes = []
    for _ in range(rng.randint(5, 9)):
        n = V((rng.gauss(0, 1), rng.gauss(0, 1), rng.gauss(0, 1))).normalized()
        planes.append((n, rng.uniform(0.62, 0.9)))
    craters = []
    for _ in range(rng.randint(4, 9)):
        c = V((rng.gauss(0, 1), rng.gauss(0, 1), rng.gauss(0, 1))).normalized()
        a = rng.uniform(0.12, 0.42)
        craters.append((c, a, a * rng.uniform(0.25, 0.45)))
    for _ in range(rng.randint(10, 22)):
        c = V((rng.gauss(0, 1), rng.gauss(0, 1), rng.gauss(0, 1))).normalized()
        a = rng.uniform(0.04, 0.11)
        craters.append((c, a, a * rng.uniform(0.3, 0.5)))
    veins = []
    for _ in range(nveins):
        a0 = V((rng.gauss(0, 1), rng.gauss(0, 1), rng.gauss(0, 1))).normalized()
        ax = a0.cross(V((rng.gauss(0, 1), rng.gauss(0, 1), rng.gauss(0, 1)))).normalized()
        veins.append((a0, ax, rng.uniform(0.9, 1.5)))
    strata_dir = V((rng.gauss(0, 1), rng.gauss(0, 1), rng.gauss(0, 1))).normalized()
    k_soft = 0.025
    scoop_s = rng.uniform(2.6, 3.6)
    for v in bm.verts:
        p = v.co.normalized()
        r = 1.0 + noise.fractal(p * 0.9 + off, 0.5, 2.0, 3, noise_basis='PERLIN_ORIGINAL') * 0.34
        # soft-min against fracture planes
        for n, d in planes:
            pn = p.dot(n)
            if pn > 0.05:
                lim = d / pn
                h = clamp(0.5 + 0.5 * (lim - r) / k_soft, 0.0, 1.0)
                r = lerp(r, lim, 1 - h) - k_soft * h * (1 - h)
        # conchoidal scoops (concave voronoi dishes, sharp ridges between)
        dd, _pts = noise.voronoi(p * scoop_s + off, distance_metric='DISTANCE', exponent=2.5)
        r += (dd[0] * dd[0]) * 0.11 - 0.03
        dd2, _ = noise.voronoi(p * scoop_s * 3.1 + off * 1.7, distance_metric='DISTANCE', exponent=2.5)
        r += (dd2[0] * dd2[0]) * 0.035
        # terraced strata ledges
        hs = p.dot(strata_dir) * 9.0 + noise.noise(p * 2.0 + off) * 0.8
        r += (hs - math.floor(hs)) * 0.012
        q = p * 2.2 + off * 0.37
        r += (noise.ridged_multi_fractal(q, 0.85, 2.1, 5, 1.0, 2.0, noise_basis='PERLIN_ORIGINAL') - 0.9) * 0.05
        r += noise.fractal(p * 9.0 + off, 0.6, 2.0, 5, noise_basis='PERLIN_ORIGINAL') * 0.022
        for c, a, dep in craters:
            dist = (p - c).length
            if dist < a * 2.0:
                x = dist / a
                if x < 1.0:
                    r -= dep * (1 - x * x) * 0.55
                r += dep * 0.28 * math.exp(-((x - 1.0) / 0.25) ** 2)
        # fissures for crystal veins
        for a0, ax, span in veins:
            dpl = abs(p.dot(ax))
            if dpl < 0.08:
                ang = math.atan2(p.dot(ax.cross(a0)), p.dot(a0))
                if 0.0 < ang < span:
                    taper = math.sin(math.pi * ang / span) ** 0.5
                    r -= 0.05 * taper * math.exp(-(dpl / 0.025) ** 2)
        v.co = V((p.x * stretch.x, p.y * stretch.y, p.z * stretch.z)) * r * radius
    return veins, stretch


def crystal(bm, base, axis, length, r, mat, rng, sides=6, tipmat=None):
    axis = V(axis).normalized()
    t, b, n = frame_axes(axis, (0, 0, 1))
    tw = rng.uniform(0, math.pi)
    ring0, ring1 = [], []
    for k in range(sides):
        a = tw + 2 * math.pi * k / sides
        d = t * math.cos(a) + b * math.sin(a)
        ring0.append(V(base) - axis * r * 1.5 + d * r * 0.85)
        ring1.append(V(base) + axis * length * 0.72 + d * r)
    tip = V(base) + axis * length + t * rng.uniform(-0.1, 0.1) * r
    loft(bm, [ring0, ring1, tip], mat=mat, matfn=(lambda i, k: tipmat if i == 1 else mat) if tipmat is not None else None)


def crystal_cluster(bm, p, n, rng, scale, mat, count):
    for i in range(count):
        d = (V(n) + V((rng.gauss(0, 0.45), rng.gauss(0, 0.45), rng.gauss(0, 0.45)))).normalized()
        L = scale * rng.uniform(0.35, 1.0) * (1.3 if i == 0 else 1.0)
        off = V((rng.gauss(0, 1), rng.gauss(0, 1), rng.gauss(0, 1))) * scale * 0.12
        crystal(bm, V(p) + off, d, L, L * rng.uniform(0.14, 0.22), mat, rng)


def build_asteroids():
    S.reset_scene()
    mats = {"Rock": flat_mat("Rock", (0.4, 0.37, 0.34), 0.0, 0.85),
            "Crystal": flat_mat("Crystal", (0.4, 0.4, 0.42), 0.0, 0.1, emit=(1, 1, 1), strength=2.5)}
    src, socks = P.make_rock_src("RockSrc")
    coll = bpy.context.scene.collection
    pairs, los, crystals = [], [], []
    for i in range(6):
        seed = 1000 + i * 77
        nv = 2 if i >= 3 else 0
        bm = bmesh.new()
        bmesh.ops.create_icosphere(bm, subdivisions=8, radius=1.0)  # 327k tris
        veins, stretch = rock_shape(bm, seed, nveins=nv)
        # center at bbox center
        lo_ = V([min(v.co[j] for v in bm.verts) for j in range(3)])
        hi_ = V([max(v.co[j] for v in bm.verts) for j in range(3)])
        c = (lo_ + hi_) / 2
        bmesh.ops.translate(bm, vec=-c, verts=bm.verts)
        hi = P.bm_to_obj(bm, "RockHi_%d" % i, {"RockSrc": src}, ["RockSrc"])
        for poly in hi.data.polygons:
            poly.use_smooth = True
        bm.free()
        # low poly: same shape on a 5120-tri icosphere, UVs from an equi-angular cube map
        bm = bmesh.new()
        bmesh.ops.create_icosphere(bm, subdivisions=5, radius=1.0)
        P.cubemap_uv(bm)
        rock_shape(bm, seed, nveins=nv)
        bmesh.ops.translate(bm, vec=-c, verts=bm.verts)
        lo = P.bm_to_obj(bm, "Rock_%d" % i, mats, ["Rock", "Crystal"])
        bm.free()
        for poly in lo.data.polygons:
            poly.use_smooth = True
        loc = V(((i % 3) * 6.0 - 6.0, 0.0, (i // 3) * -6.0 + 3.0))
        hi.location = loc
        lo.location = loc
        pairs.append((hi, lo))
        los.append(lo)
        # crystals on Rock_3..5 (built now in lo-local space, joined after the bake)
        if i >= 3:
            rng = random.Random(seed + 5)
            lbm = bmesh.new()
            lbm.from_mesh(lo.data)
            cbm = bmesh.new()
            pts = sample_surface(lbm, 3 + (i - 3), rng)
            for j, (p, t, b, n, fa) in enumerate(pts):
                crystal_cluster(cbm, p - n * 0.05, n, rng, rng.uniform(0.8, 1.4) if j == 0 else rng.uniform(0.4, 0.8), 1,
                                rng.randint(5, 9) if j == 0 else rng.randint(3, 6))
            # veins: dense seam of shards packed into the carved fissures
            from mathutils.bvhtree import BVHTree
            tree = BVHTree.FromBMesh(lbm)
            sv = V(stretch)
            for a0, ax, span in veins:
                steps = int(span / 0.035)
                for s in range(steps):
                    ang = (s + rng.uniform(0, 0.7)) * span / steps
                    taper = math.sin(math.pi * ang / span) ** 0.6
                    if taper < 0.15:
                        continue
                    d = (Matrix.Rotation(-ang, 3, ax) @ a0).normalized()
                    ds = V((d.x * sv.x, d.y * sv.y, d.z * sv.z)).normalized()
                    o = -c
                    hit = tree.ray_cast(o + ds * 8.0, -ds)
                    if hit[0] is None:
                        continue
                    hp, hn = hit[0], hit[1]
                    tang = ax.cross(d).normalized()
                    for _k in range(2):
                        L = rng.uniform(0.1, 0.32) * (0.5 + 0.8 * taper)
                        lean = (hn * rng.uniform(0.6, 1.0) + tang * rng.uniform(-0.9, 0.9) + ax * rng.uniform(-0.4, 0.4)).normalized()
                        crystal(cbm, hp - hn * L * 0.25, lean, L, L * rng.uniform(0.16, 0.26), 1, rng)
            lbm.free()
            co = P.bm_to_obj(cbm, "Crys_%d" % i, mats, ["Rock", "Crystal"])
            cbm.free()
            co.location = loc
            for poly in co.data.polygons:
                poly.use_smooth = False
            crystals.append((lo, co))
    tex = OPTS['tex']
    if OPTS['bake']:
        P.pack_uv(los, margin=0.006)
        paths = P.bake(los, src, socks, BUILD, "asteroids", size=tex, samples=32, dst_mat=mats["Rock"],
                       pairs=pairs, cage=0.06, maxray=0.14)
        S.rebuild_hull_textured(mats["Rock"], paths)
    for hi, lo in pairs:
        me = hi.data
        bpy.data.objects.remove(hi)
        bpy.data.meshes.remove(me)
    for lo, co in crystals:
        bpy.ops.object.select_all(action='DESELECT')
        co.select_set(True)
        lo.select_set(True)
        bpy.context.view_layer.objects.active = lo
        bpy.ops.object.join()
    for lo in los:
        lo.data.name = lo.name
    out = os.path.join(MODELS, "asteroids.glb")
    if OPTS['bake']:
        S.export_glb(out, los, [])
    report("asteroids", los, out)
    if OPTS['preview']:
        mats["Crystal"].node_tree.nodes["Principled BSDF"].inputs['Emission Color'].default_value = (0.25, 0.8, 1.0, 1)
        mats["Crystal"].node_tree.nodes["Principled BSDF"].inputs["Emission Strength"].default_value = 1.6
        mats["Crystal"].node_tree.nodes["Principled BSDF"].inputs["Base Color"].default_value = (0.1, 0.35, 0.5, 1)
        cam = preview_scene()
        P.preview_set(los, "asteroids", PREVIEWS, VIEWS, (0.15, -1.0, 0.25),
                      [((0.15, -1.0, 0.25), False), ((-0.6, 1.0, 0.5), False), ((0.8, -0.3, -0.6), False), ((0, 0.0001, 1), True)])
        # close-up of a crystal rock
        S.frame(cam, [los[4]], (0.5, -1.0, 0.4), margin=1.05)
        S.render(os.path.join(PREVIEWS, "props_asteroids_closeup.png"), samples=96)


# ================================================================ A. enemy ships

# glow = palette::side::HOSTILE.glow (docs/ART_DIRECTION.md §3.4); the client re-tints it by relation anyway.
def run_ship(key, builder, style, k=1.0, overlay=None, glow=(1.0, 0.25, 0.15), core=(1.0, 0.45, 0.2), glow_strength=None,
             hero=(1.0, 1.15, 0.55), views=None, length=None):
    S.reset_scene()
    mats, socks = S.make_materials(style)
    if k != 1.0:
        P.scale_space(mats["Hull"], k)
    if overlay:
        P.overlay_hull(mats["Hull"], socks, **overlay)
    ship = S.Ship(key, mats)
    builder(ship)
    objs = ship.finalize()
    meshes = list(objs.values())
    if length:
        f = length / (ship.bbox[1].y - ship.bbox[0].y)
        M = Matrix.Scale(f, 4)
        for o in meshes:
            o.data.transform(M)
            o.data.update()
        for e in ship.empties:
            e.location = e.location * f
            e.scale = e.scale * f
        log(key, "scaled by %.3f to length %.1f" % (f, length))
    out = os.path.join(MODELS, "npc_%s.glb" % key)
    if OPTS['bake']:
        S.unwrap_hull(meshes)
        paths = P.bake(meshes, mats["Hull"], socks, BUILD, "npc_" + key, size=OPTS['tex'], samples=24)
        S.rebuild_hull_textured(mats["Hull"], paths)
        S.export_glb(out, meshes, ship.empties)
    report("npc_" + key, meshes, out)
    if OPTS['preview']:
        tint_preview(mats, glow=glow, core=core, glow_strength=glow_strength)
        preview_scene()
        views = views or [((-1.0, 1.1, 0.6), False), ((-0.9, -1.2, 0.5), False), ((1.0, 0.2, -0.5), False), ((0.0, 0.0001, 1.0), True)]
        P.preview_set(meshes, key, PREVIEWS, VIEWS, hero, views)


PIRATE = dict(paint=(0.25, 0.24, 0.23), paint2=(0.15, 0.145, 0.14), accent=(0.3, 0.17, 0.08), accent_thresh=0.9,
              metal=0.45, rough=0.55, spacing=(1.3, 1.8, 1.0), wear=1.5, grime=1.5, sub_amt=0.45, seam_w=0.026,
              grime_col=(0.05, 0.035, 0.025), bare=(0.5, 0.48, 0.45))


def _src(fn):
    src = bmesh.new()
    fn(src)
    bmesh.ops.remove_doubles(src, verts=src.verts, dist=1e-5)
    bmesh.ops.recalc_face_normals(src, faces=src.faces)
    src.normal_update()
    return src


def plates_on(bm_src_fn, bm, rng, count, mask, size=(0.8, 2.0), thick=0.08, livery=0.4, tilt=6.0):
    """Welded scrap armor plates sampled on a source surface."""
    src = _src(bm_src_fn)
    for p, t, b, n, fa in sample_surface(src, count, rng, mask):
        R = rot_tbn(t, b, n) @ Matrix.Rotation(math.radians(rng.uniform(-tilt, tilt)), 3, 'X') @ \
            Matrix.Rotation(math.radians(rng.uniform(-25, 25)), 3, 'Z')
        w, l = rng.uniform(*size) * 0.7, rng.uniform(*size)
        box(bm, p + n * (thick * 0.3), (w, l, thick), mat=L if rng.random() < livery else H, rot=R, taper=(0.85, 1.0))
    src.free()


def greeble_on(bm_src_fn, bm, rng, count, mask, scale, kinds=None):
    src = _src(bm_src_fn)
    greebles(bm, sample_surface(src, count, rng, mask), rng, scale=scale, mats=(H, L, G), kinds=kinds)
    src.free()


# ---------------------------------------------------------------- raider

def build_raider(ship):
    rng = random.Random(11)
    RST = [(-9.0, 1.05, 0.8, 0.15, 0.0), (-7.5, 1.55, 1.05, 0.2, 0.05), (-3.5, 1.8, 1.15, 0.15, 0.1),
           (0.5, 1.7, 1.05, 0.05, 0.1), (3.5, 1.3, 0.85, -0.05, 0.05), (6.0, 0.8, 0.55, -0.15, 0.0),
           (7.6, 0.3, 0.3, -0.2, 0.0)]

    def body_geo(bm):
        rings = [ring10(y, hw, hh, cz, cx, top=0.5, bot=0.75, hi=0.25, lo=-0.45, ridge=0.18, keel=0.25)
                 for y, hw, hh, cz, cx in RST]
        rings.append(V((0, 8.3, -0.22)))
        return loft(bm, rings, matfn=lambda i, k: H)

    def body(bm):
        r = body_geo(bm)
        side = [f for i, band in enumerate(r['bands'][1:4]) for k, f in enumerate(band) if k in (2, 7, 1, 8)]
        S.inset_faces(bm, side, thickness=0.12, depth=-0.04)
    ship.part(body, bevel=0.04, angle=30)

    # mandible prongs (asymmetric: left one broken)
    def prongs(bm):
        R = [(0.55, 3.2), (1.45, 3.9), (1.6, 5.6), (1.2, 8.2), (0.78, 10.6), (0.6, 8.8), (0.32, 6.6)]
        Lp = [(-0.32, 6.6), (-0.6, 8.0), (-0.9, 8.6), (-1.05, 8.1), (-1.2, 8.7), (-1.35, 7.4), (-1.6, 5.6), (-1.45, 3.9), (-0.55, 3.2)]
        Lp = list(reversed(Lp))
        for pts, sx in ((R, 1), (Lp, -1)):
            pts = jag(pts, rng, depth=0.18, seg=0.55, prob=0.6, inward=False)
            slab(bm, pts, 0.36, origin=(0, 0, -0.2), u=(1, 0, 0), v=V((0, 1, -0.07)).normalized(), bevel_top=0.08)
            slab(bm, [(sx * 0.9, 4.2), (sx * 1.42, 4.6), (sx * 1.48, 5.6), (sx * 1.0, 5.4)][::sx], 0.06,
                 origin=(0, 0, -0.2 + 0.2), u=(1, 0, 0), v=V((0, 1, -0.07)).normalized(), mat=L)
        for i in range(3):
            P.spike(bm, (1.5 - i * 0.12, 5.0 + i * 1.3, -0.25), (1, 0.6, 0.1), 0.6 - i * 0.12, 0.09, n=5)
    ship.part(prongs, bevel=0.03, angle=35)

    # wings: big forward-swept blade right, stub + gun pod left
    an_r = math.radians(-8)
    an_l = math.radians(-14)

    def wings(bm):
        pr = [(1.2, -6.0), (4.6, -3.6), (7.4, -0.6), (8.6, 0.9), (8.9, 2.4), (7.9, 2.1), (4.6, 0.9), (1.2, -0.5)]
        pr = jag(pr[:3], rng, depth=0.35, seg=0.7, prob=0.8, closed=False, inward=True) + pr[3:]
        ur = V((math.cos(an_r), 0, math.sin(an_r)))
        slab(bm, pr, 0.3, origin=(0, 0, -0.25), u=ur, v=(0, 1, 0), bevel_top=0.09)
        nr = ur.cross(V((0, 1, 0)))
        slab(bm, [(5.4, -1.6), (6.4, -0.5), (6.9, 1.4), (5.9, 1.1)], 0.04, origin=V((0, 0, -0.25)) + nr * 0.16, u=ur, v=(0, 1, 0), mat=L)
        pl = [(-1.2, 0.0), (-5.5, -1.1), (-5.9, -3.0), (-1.2, -5.0)]
        slab(bm, pl, 0.32, origin=(0, 0, -0.35), u=V((1, 0, -math.tan(an_l))).normalized(), v=(0, 1, 0), bevel_top=0.09)
        # wingtip missile rail (right)
        tip = V((8.3 * math.cos(an_r), 0.0, -0.25 + 8.3 * math.sin(an_r)))
        cyl(bm, tip + V((0, -1.6, -0.2)), tip + V((0, 2.6, -0.2)), 0.1, n=8)
        for dy in (-0.6, 0.9):
            cyl(bm, tip + V((0, dy - 1.0, -0.42)), tip + V((0, dy + 0.9, -0.42)), 0.13, n=10, mat=H)
            cyl(bm, tip + V((0, dy + 0.9, -0.42)), tip + V((0, dy + 1.3, -0.42)), 0.13, 0.0, n=10, mat=L)
        sphere(bm, tip + V((0.15, 2.5, 0.0)), 0.07, mat=G, seg=8, rings=4)
        P.spike(bm, tip + V((0.05, 2.0, 0.1)), (0.3, 1, 0.1), 1.2, 0.1, n=5)
    ship.part(wings, bevel=0.035, angle=35)


    def gunpod(bm):
        c = V((-5.8, 0.0, -0.35 - 5.8 * math.tan(-an_l)))
        lathe(bm, c, (0, 1, 0), [(0.0, -4.4), (0.3, -4.2), (0.42, -3.4), (0.42, 2.8), (0.3, 3.6), (0.18, 3.8)], n=14)
        cyl(bm, c + V((0, 3.6, 0)), c + V((0, 6.6, 0)), 0.11, n=10)
        cyl(bm, c + V((0, 5.8, 0)), c + V((0, 6.9, 0)), 0.17, 0.15, n=10)
        for dy in (-2.0, 0.0, 1.8):
            cyl(bm, c + V((0, dy - 0.12, 0)), c + V((0, dy + 0.12, 0)), 0.47, n=14, mat=L if dy == 0 else H)
        box(bm, c + V((0.0, -0.8, 0.5)), (0.25, 2.2, 0.25))
        sphere(bm, c + V((-0.43, 1.5, 0.0)), 0.06, mat=G, seg=8, rings=4)
    ship.part(gunpod, bevel=0.02, angle=40)
    gp = V((-5.8, 0.0, -0.35 - 5.8 * math.tan(-an_l)))
    ship.empty("Gun_0", gp + V((0, 7.0, 0)), 0.15)

    def cockpit(bm):
        cx = -0.55
        rings = []
        prof = [(0.6, 0.0, 0.0), (1.2, 0.36, 0.28), (2.4, 0.46, 0.4), (3.6, 0.38, 0.33), (4.4, 0.0, 0.0)]
        for y, rx, rz in prof:
            ztop = interp_body(RST, y)
            if rx == 0:
                rings.append(V((cx, y, ztop + 0.02)))
            else:
                rings.append([V((cx + rx * math.cos(a), y, ztop - 0.15 + rz * math.sin(a)))
                              for a in [math.pi * k / 9 for k in range(10)]])
        loft(bm, rings, mat=GL, closed=False, cap_start=False, cap_end=False)
        # welded cage bars over the canopy
        for y in (1.4, 2.4, 3.4):
            ztop = interp_body(RST, y)
            arc = [V((cx + 0.55 * math.cos(a), y, ztop - 0.18 + 0.48 * math.sin(a))) for a in [math.pi * k / 8 for k in range(9)]]
            for a_, b_ in zip(arc, arc[1:]):
                prism(bm, a_, b_, 0.05, 0.05)
        prism(bm, (cx, 0.9, interp_body(RST, 0.9) + 0.25), (cx, 4.0, interp_body(RST, 4.0) + 0.12), 0.05, 0.05)
    ship.part(cockpit, bevel=0, angle=40, weighted=False)

    def fin(bm):
        pts = [(-8.8, 0.0), (-3.0, 0.0), (-5.0, 1.2), (-6.6, 3.2), (-7.4, 3.6), (-7.9, 3.2), (-8.4, 1.5)]
        pts = pts[:2] + jag(pts[1:4], rng, depth=0.3, seg=0.6, prob=0.8, closed=False, inward=True)[1:] + pts[4:]
        vv = V((0.28, 0, 1)).normalized()
        slab(bm, pts, 0.2, origin=(0.55, 0, 1.0), u=(0, 1, 0), v=vv, bevel_top=0.06)
        tipp = V((0.55, 0, 1.0)) + V((0, -7.4, 0)) + vv * 3.6
        sphere(bm, tipp, 0.08, mat=G, seg=8, rings=4)
        slab(bm, [(-7.6, 2.2), (-5.9, 2.2), (-6.3, 2.75), (-7.1, 3.1)], 0.04, origin=V((0.55, 0, 1.0)) + V((1, 0, -0.28)).normalized() * 0.12,
             u=(0, 1, 0), v=vv, mat=L)
        # antenna
        cyl(bm, (-0.8, -6.2, 1.15), (-1.1, -8.4, 3.0), 0.04, 0.015, n=6)
        sphere(bm, (-1.1, -8.4, 3.0), 0.06, mat=G, seg=8, rings=4)
    ship.part(fin, bevel=0.03, angle=35)

    def engines(bm):
        # main engine collar + nozzle
        lathe(bm, (0, -8.6, 0.15), (0, 1, 0), [(0.0, -0.9), (1.0, -0.9), (1.08, -0.6), (1.08, 0.4), (0.95, 0.8)], n=24, cap_end=False)
        nozzle(bm, (0, -10.4, 0.15), 0.85, 1.0, n=24, rimmat=H, coremat=E, hullmat=H, ribs=8)
        # strapped-on booster (left, low)
        c = V((-1.95, 0, -0.8))
        lathe(bm, c, (0, 1, 0), [(0.48, -8.6), (0.55, -7.8), (0.55, -3.6), (0.4, -2.6), (0.0, -2.2)], n=16, cap_start=True, cap_end=False)
        nozzle(bm, c + V((0, -9.2, 0)), 0.45, 0.7, n=16, rimmat=H, coremat=E, hullmat=H)
        for y in (-7.0, -4.4):
            box(bm, c + V((0.55, y, 0.25)), (0.9, 0.35, 0.2))
            cyl(bm, c + V((0, y - 0.15, 0)), c + V((0, y + 0.15, 0)), 0.6, n=16, mat=L)
    ship.part(engines, bevel=0, subsurf=0, angle=50, weighted=False)
    ship.empty("Engine_0", (0, -10.4, 0.15), 0.85)
    ship.empty("Engine_1", (-1.95, -9.2, -0.8), 0.45)

    def chin(bm):
        box(bm, (0, 3.4, -0.85), (0.5, 2.6, 0.4), taper=(0.7, 0.7))
        cyl(bm, (0, 4.6, -0.8), (0, 8.6, -0.72), 0.1, n=10)
        cyl(bm, (0, 7.9, -0.73), (0, 8.7, -0.71), 0.15, n=10)
        # glowing eye slits
        for sx in (1, -1):
            R = Matrix.Rotation(math.radians(sx * 28), 3, 'Y') @ Matrix.Rotation(math.radians(-sx * 20), 3, 'Z')
            box(bm, (sx * 0.5, 5.9, 0.05), (0.42, 0.09, 0.07), mat=G, rot=R)
        for i in range(4):
            box(bm, (1.79 + 0.0, -5.2 + i * 1.1, 0.1), (0.06, 0.6, 0.07), mat=G)
            box(bm, (-1.79, -4.6 + i * 1.3, 0.25), (0.06, 0.45, 0.06), mat=G)
    ship.part(chin, bevel=0.015, angle=40)
    ship.empty("Gun_1", (0, 8.8, -0.71), 0.12)

    def menace(bm):
        for i in range(3):
            y = -2.6 + i * 1.1
            z = interp_body(RST, y) + 0.18
            P.spike(bm, (0.15, y, z - 0.15), (0.1, -0.9, 1), 0.95 - i * 0.2, 0.17, n=4)
        for sx in (1, -1):
            box(bm, (sx * 1.6, 1.0, -0.3), (0.75, 3.4, 0.95), taper=(0.55, 0.75))
            box(bm, (sx * 1.6, 2.68, -0.3), (0.42, 0.06, 0.55), mat=GL)
            for j in range(4):
                P.spike(bm, (sx * (1.42 + j * 0.1), 2.7, -0.72), (sx * 0.15, 1, -0.25), 0.45, 0.07, n=4)
            box(bm, (sx * 1.98, 0.6, -0.3), (0.05, 1.6, 0.08), mat=L)
    ship.part(menace, bevel=0.02, angle=35)

    def scrap(bm):
        plates_on(body_geo, bm, rng, 12, lambda f: abs(f.normal.x) > 0.55 and -8 < f.calc_center_median().y < 5,
                  size=(0.9, 2.0), livery=0.4)
        plates_on(body_geo, bm, rng, 5, lambda f: f.normal.z > 0.6 and -8 < f.calc_center_median().y < 0.5,
                  size=(0.8, 1.6), livery=0.3)
        greeble_on(body_geo, bm, rng, 26, lambda f: f.normal.z > 0.5 and f.calc_center_median().y < 0.3, 0.45)
        greeble_on(body_geo, bm, rng, 10, lambda f: f.normal.z < -0.5, 0.45)
    ship.part(scrap, bevel=0.012, angle=40)


def interp_body(st, y):
    """top-surface z of a ring10 station list at y (cz + hh)."""
    s = sorted(st, key=lambda a: a[0])
    if y <= s[0][0]:
        return s[0][3] + s[0][2]
    for a, b in zip(s, s[1:]):
        if a[0] <= y <= b[0]:
            t = (y - a[0]) / (b[0] - a[0])
            return lerp(a[3] + a[2], b[3] + b[2], t)
    return s[-1][3] + s[-1][2]


def turret(bm, base, yaw=0.0, r=1.4, barrels=2, blen=5.0, br=0.18, spread=0.5, h=0.9, mats=(H, L, G)):
    """Pirate/Kharz turret: ring base, armored housing, barrels along +Y (rotated by yaw).
    Returns list of muzzle points."""
    Hm, Lm, Gm = mats
    b = V(base)
    Rz = Matrix.Rotation(yaw, 3, 'Z')
    cyl(bm, b - V((0, 0, 0.3)), b + V((0, 0, 0.25)), r * 1.05, r, n=20, mat=Hm)
    hz = b + V((0, 0, 0.25 + h / 2))
    box(bm, hz + Rz @ V((0, -0.15 * r, 0)), (r * 1.5, r * 1.9, h), mat=Hm, rot=Rz, taper=(0.75, 0.6))
    box(bm, hz + Rz @ V((0, -0.4 * r, h * 0.55)), (r * 0.8, r * 0.9, h * 0.25), mat=Lm, rot=Rz)
    muzzles = []
    for i in range(barrels):
        x = (i - (barrels - 1) / 2) * spread
        p0 = hz + Rz @ V((x, r * 0.6, -0.05))
        p1 = hz + Rz @ V((x, r * 0.6 + blen, -0.05))
        cyl(bm, p0, p1, br, br * 0.85, n=10, mat=Hm)
        cyl(bm, p0 + Rz @ V((0, blen * 0.08, 0)), p0 + Rz @ V((0, blen * 0.35, 0)), br * 1.45, n=10, mat=Hm)
        cyl(bm, p1 - Rz @ V((0, blen * 0.12, 0)), p1, br * 1.35, br * 1.25, n=10, mat=Hm)
        muzzles.append(p1 + Rz @ V((0, 0.05, 0)))
    sphere(bm, hz + Rz @ V((r * 0.55, r * 0.5, h * 0.2)), 0.12 * r, mat=Gm, seg=8, rings=4)
    return muzzles


# ---------------------------------------------------------------- gunship

def build_gunship(ship):
    rng = random.Random(23)
    GST = [(-14.0, 2.7, 1.9, 0.2, 0.0), (-11.0, 3.4, 2.4, 0.3, 0.0), (-3.0, 3.6, 2.5, 0.3, 0.15),
           (5.0, 3.4, 2.3, 0.1, 0.1), (10.5, 2.8, 1.8, -0.1, 0.0)]

    def body_geo(bm):
        rings = [ring10(y, hw, hh, cz, cx, top=0.62, bot=0.8, hi=0.45, lo=-0.5, ridge=0.25, keel=0.35) for y, hw, hh, cz, cx in GST]
        return loft(bm, rings)

    def body(bm):
        r = body_geo(bm)
        side = [f for band in r['bands'] for k, f in enumerate(band) if k in (2, 7, 3, 6)]
        S.inset_faces(bm, side, thickness=0.18, depth=-0.06)
        top = [f for band in r['bands'][1:3] for k, f in enumerate(band) if k in (1, 8)]
        S.inset_faces(bm, top, thickness=0.3, depth=0.08)
    ship.part(body, bevel=0.05, angle=30)

    def prow(bm):
        # armored ram plow
        rings = [ring10(10.0, 2.95, 1.95, -0.1, 0.0, top=0.6, bot=0.85, hi=0.4, lo=-0.55, ridge=0.2, keel=0.3),
                 ring10(13.5, 2.1, 1.5, -0.35, 0.0, top=0.5, bot=0.8, hi=0.3, lo=-0.6, ridge=0.15, keel=0.5),
                 ring10(16.4, 0.35, 1.0, -0.7, 0.0, top=0.5, bot=0.6, hi=0.4, lo=-0.6, ridge=0.0, keel=0.6)]
        loft(bm, rings, matfn=lambda i, k: L if (i == 0 and k in (1, 8)) else H)
        # saw teeth along the lower prow edges
        for sx in (1, -1):
            for j in range(6):
                t = j / 5
                y = lerp(10.6, 15.8, t)
                hw = lerp(2.75, 0.5, t)
                z = lerp(-1.45, -1.6, t)
                P.spike(bm, (sx * hw * 0.8, y, z), (sx * 0.45, 0.8, -0.55), 2.0 - 0.7 * t, 0.36, n=4)
        P.spike(bm, (0, 16.0, -0.6), (0, 1, -0.1), 2.2, 0.35, n=6)
        # eye slits
        for sx in (1, -1):
            box(bm, (sx * 1.6, 12.6, 0.55), (0.9, 0.12, 0.12), mat=G,
                rot=Matrix.Rotation(math.radians(-sx * 32), 3, 'Z') @ Matrix.Rotation(math.radians(sx * 12), 3, 'Y'))
    ship.part(prow, bevel=0.04, angle=30)

    def bridge(bm):
        # offset command tower (right rear)
        bx, by = 1.7, -6.0
        zt = interp_body(GST, by) + 0.2
        box(bm, (bx, by, zt + 0.7), (2.4, 4.2, 1.6), taper=(0.8, 0.7))
        box(bm, (bx, by + 0.4, zt + 1.85), (1.8, 2.6, 0.9), taper=(0.75, 0.6))
        box(bm, (bx, by + 1.62, zt + 1.9), (1.3, 0.1, 0.25), mat=GL, rot=Matrix.Rotation(math.radians(-25), 3, 'X'))
        for sx in (1, -1):
            box(bm, (bx + sx * 0.93, by + 0.4, zt + 1.9), (0.06, 1.8, 0.15), mat=GL)
        cyl(bm, (bx + 0.5, by - 1.0, zt + 2.3), (bx + 0.6, by - 1.6, zt + 5.0), 0.06, 0.03, n=6)
        sphere(bm, (bx + 0.6, by - 1.6, zt + 5.0), 0.1, mat=G, seg=8, rings=4)
        cyl(bm, (bx - 0.5, by - 0.8, zt + 2.3), (bx - 0.5, by - 0.8, zt + 3.4), 0.05, n=6)
        lathe(bm, V((bx - 0.5, by - 0.8, zt + 3.4)), (0.3, 0.6, 1), [(0.0, -0.05), (0.5, 0.0), (0.75, 0.25), (0.7, 0.3), (0.3, 0.08), (0.0, 0.05)], n=16)
    ship.part(bridge, bevel=0.035, angle=35)

    muz = []

    def guns(bm):
        zt = interp_body(GST, 3.5) + 0.25
        muz.extend(turret(bm, (-0.6, 3.5, zt), r=1.6, barrels=2, blen=6.5, br=0.22, spread=0.7, h=1.0))
        # left missile sponson
        c = V((-4.3, 0.5, -0.4))
        box(bm, c, (1.8, 7.0, 2.0), taper=(0.85, 0.85))
        box(bm, c + V((0.6, -1.2, 0)), (1.6, 3.2, 0.5))
        for ix in range(2):
            for iz in range(3):
                p = c + V((-0.42 + ix * 0.84, 3.5, -0.62 + iz * 0.62))
                cyl(bm, p - V((0, 0.3, 0)), p + V((0, 0.08, 0)), 0.26, n=10, mat=H)
                cyl(bm, p, p + V((0, 0.02, 0)), 0.17, n=10, mat=G)
        box(bm, c + V((-0.92, 0, 0.2)), (0.06, 5.0, 0.12), mat=L)
    ship.part(guns, bevel=0.03, angle=35)
    for i, m in enumerate(muz):
        ship.empty("Gun_%d" % i, m, 0.2)
    ship.empty("Gun_2", (-4.3, 4.1, -0.4), 0.25)

    def junk(bm):
        # welded cargo container (right)
        c = V((4.3, -2.0, -0.6))
        box(bm, c, (2.2, 7.0, 2.2), mat=L)
        for j in range(12):
            box(bm, c + V((1.12, -3.2 + j * 0.58, 0)), (0.08, 0.2, 2.0))
        for y in (-2.2, 2.2):
            box(bm, c + V((-1.0, y, 0)), (1.2, 0.35, 0.4))
            cyl(bm, c + V((0, y, 1.15)), c + V((-1.6, y, 1.6)), 0.08, n=6)
        # ventral tanks + keel fin
        for sx in (1, -1):
            cyl(bm, (sx * 1.4, -8.5, -2.6), (sx * 1.4, -1.0, -2.6), 0.55, n=16)
            for y in (-8.5, -1.0):
                sphere(bm, (sx * 1.4, y, -2.6), 0.55, seg=16, rings=8)
            box(bm, (sx * 1.4, -4.75, -2.05), (0.3, 4.0, 0.6))
        slab(bm, jag([(-12.5, 0.0), (-4.0, 0.0), (-7.0, -2.0), (-11.5, -2.6)], rng, depth=0.3, seg=0.8, inward=True), 0.25,
             origin=(0, 0, -2.6), u=(0, 1, 0), v=(0, 0, 1), bevel_top=0.08)
        # dorsal spike ridge
        for j in range(5):
            y = -12.5 + j * 1.6
            z = interp_body(GST, y) + 0.25
            P.spike(bm, (-0.6, y, z - 0.2), (-0.15, -0.7, 1), 1.5 - j * 0.15, 0.25, n=4)
        # light slits
        for sx in (1, -1):
            for j in range(6):
                box(bm, (sx * 3.62, -10 + j * 2.6, 0.9), (0.06, 1.0, 0.1), mat=G)
    ship.part(junk, bevel=0.03, angle=35)

    def engines(bm):
        hx = [(1.5, 0.9, 1.1), (-1.5, 0.9, 1.1), (0.0, -1.0, 1.1), (2.9, -1.1, 0.6)]
        box(bm, (0, -15.0, 0.1), (5.6, 2.4, 4.2), taper=(1.05, 1.05))
        for i, (x, z, r) in enumerate(hx):
            cyl(bm, (x, -15.4, z), (x, -16.2, z), r * 1.12, n=20)
            nozzle(bm, (x, -17.4, z), r, 1.3, n=20, rimmat=H, coremat=E, hullmat=H, ribs=6 if r > 1 else 0)
            ship.empty("Engine_%d" % i, (x, -17.4, z), r)
        box(bm, (2.9, -14.6, -1.1), (1.4, 2.6, 1.4))
    ship.part(engines, bevel=0, angle=50, weighted=False)

    def scrap(bm):
        plates_on(body_geo, bm, rng, 18, lambda f: abs(f.normal.x) > 0.55, size=(1.2, 2.8), livery=0.35, thick=0.12)
        plates_on(body_geo, bm, rng, 8, lambda f: f.normal.z > 0.6 and f.calc_center_median().y < 1.0, size=(1.2, 2.4), livery=0.25, thick=0.1)
        greeble_on(body_geo, bm, rng, 45, lambda f: f.normal.z > 0.5 and not (-8.5 < f.calc_center_median().y < -3.5 and f.calc_center_median().x > 0.4)
                   and not (1.5 < f.calc_center_median().y < 5.5), 0.7)
        greeble_on(body_geo, bm, rng, 16, lambda f: f.normal.z < -0.5, 0.6)
    ship.part(scrap, bevel=0.015, angle=40)


# ---------------------------------------------------------------- warden (Veyr)

VEYR = dict(organic=True, paint=(0.07, 0.085, 0.1), paint2=(0.13, 0.08, 0.16), accent=(0.18, 0.24, 0.26),
            metal=0.6, rough=0.28, wear=0.25, grime=0.45, bare=(0.45, 0.5, 0.55), bump=0.7)


def build_warden(ship):
    rng = random.Random(31)
    from mathutils.bvhtree import BVHTree

    def body(bm):
        prof = [(0.0, -12.6), (0.9, -12.0), (2.2, -9.5), (3.3, -5.0), (3.5, -1.0), (3.0, 3.5), (2.0, 8.0), (0.9, 11.6), (0.0, 13.0)]
        rings = []
        for r, y in prof:
            if r == 0:
                rings.append(V((0, y, 0)))
            else:
                n = 16
                pts = []
                for k in range(n):
                    a = 2 * math.pi * k / n
                    x, z = math.cos(a), math.sin(a)
                    zz = z * (0.55 if z > 0 else 0.42)
                    ridge = 0.25 * max(0.0, z) ** 8
                    pts.append(V((x * r, y, (zz + ridge) * r)))
                rings.append(pts)
        loft(bm, rings)
    bobj = ship.part(body, bevel=0, subsurf=2, angle=180, weighted=False)
    tb = bmesh.new()
    tb.from_mesh(bobj.data)
    tree = BVHTree.FromBMesh(tb)

    def surf(x, y, up=True):
        o = V((x, y, 20.0 if up else -20.0))
        hit = tree.ray_cast(o, V((0, 0, -1 if up else 1)))
        return hit[0], hit[1]

    def shells(bm):
        # overlapping carapace segments across the back (beetle-like), lightly raised
        for j in range(6):
            y = -8.0 + j * 2.9
            pts, rad = [], []
            for q in range(15):
                x = -3.6 + q * 7.2 / 14
                p, n = surf(x, y)
                if p is None:
                    continue
                pts.append(p + n * 0.12)
            if len(pts) < 4:
                continue
            width = 1.7 - abs(j - 2.5) * 0.12
            P.sweep(bm, pts, lambda u: (0.18, width * (1 - 0.5 * abs(u - 0.5) ** 2)), n=8, up=(0, 1, 0),
                    matfn=lambda i, k: L if k in (1, 2) else H)
        # crystal-lined keel ridge
        pts = []
        for q in range(14):
            y = -10.0 + q * 1.6
            p, n = surf(0.0, y)
            if p is not None:
                pts.append(p + n * 0.05)
        P.sweep(bm, pts, lambda u: (0.25, 0.35 * math.sin(math.pi * u) + 0.05), n=6)
    ship.part(shells, bevel=0, subsurf=1, angle=180, weighted=False)

    def arms(bm):
        for sx in (1, -1):
            path = P.bezier((sx * 2.4, -4.0, 0.0), (sx * 7.5, -2.5, -0.2), (sx * 8.2, 6.0, -0.4), (sx * 3.6, 12.8, -0.3), 18)
            P.sweep(bm, path, lambda u: (lerp(1.1, 0.12, u) * (1 + 0.6 * math.sin(math.pi * u)), lerp(0.6, 0.1, u)), n=10, tip_end=True)
            for j in range(4):
                u = 0.2 + j * 0.14
                i = int(u * (len(path) - 1))
                d = (path[i + 1] - path[i]).normalized()
                rr = lerp(1.1, 0.12, u) * (1 + 0.6 * math.sin(math.pi * u)) * 1.08
                P.sweep(bm, [path[i] - d * 0.12, path[i] + d * 0.12], [(rr, rr * 0.6)] * 2, n=12, mat=L)
    ship.part(arms, bevel=0, subsurf=1, angle=180, weighted=False)

    def crystals(bm):
        # crown: irregular clusters erupting from the spine
        for j in range(5):
            y = -5.5 + j * 2.4
            p, n = surf(0.0, y)
            hmax = 2.4 + 3.6 * math.sin(math.pi * (j + 0.5) / 5)
            for q in range(rng.randint(4, 6)):
                ax = (n + V((rng.gauss(0, 0.4), rng.gauss(0, 0.25) - 0.3, 0))).normalized()
                L_ = hmax * (1.0 if q == 0 else rng.uniform(0.35, 0.75))
                crystal(bm, p + V((rng.gauss(0, 0.35), rng.gauss(0, 0.35), -0.4)), ax, L_, 0.22 + 0.07 * L_, GL, rng, tipmat=G)
        # flank clusters near the arm roots
        for sx in (1, -1):
            for y in (-4.5, -2.0):
                p, n = surf(sx * 2.6, y)
                if p is None:
                    continue
                for q in range(4):
                    ax = (n + V((sx * 0.6, rng.gauss(0, 0.3), 0.1))).normalized()
                    crystal(bm, p - n * 0.2, ax, rng.uniform(1.2, 2.4), 0.24, GL, rng, tipmat=G)
            for j in range(5):
                crystal(bm, V((sx * (5.6 + j * 0.3), -0.8 + j * 2.1, 0.25)), (sx * 0.5, -0.2, 1), 0.9 - j * 0.1, 0.16, GL, rng, tipmat=G)
        # glowing heart lens near the nose
        p, n = surf(0.0, 5.5)
        sphere(bm, p - n * 0.25, 0.85, mat=G, seg=16, rings=8, scale=(1, 1.7, 0.55))
    ship.part(crystals, bevel=0, angle=20, weighted=False, sharp_all=True)

    def veins(bm):
        for sx in (1, -1):
            for off in (2.2, 3.0):
                pts = []
                for q in range(16):
                    y = -10.0 + q * 1.35
                    x = sx * min(off, 0.6 + off * math.sin(math.pi * (q + 1) / 17))
                    p, n = surf(x, y)
                    if p is not None:
                        pts.append(p + n * 0.02)
                if len(pts) > 3:
                    P.sweep(bm, pts, [0.06] * len(pts), n=6, mat=G)
            # gill slits on the flanks
            for j in range(4):
                y = 1.0 + j * 1.2
                hw = 3.3 - j * 0.15
                box(bm, (sx * hw * 0.97, y, -0.05), (0.12, 0.9, 0.12), mat=G,
                    rot=Matrix.Rotation(math.radians(sx * 20), 3, 'Z'))
    ship.part(veins, bevel=0, angle=180, weighted=False)

    def engines(bm):
        for sx in (1, -1):
            c = V((sx * 1.6, -10.5, -0.2))
            lathe(bm, c, (0, 1, 0), [(0.0, -1.6), (0.7, -1.5), (0.95, -0.6), (1.0, 1.2), (0.6, 3.0), (0.0, 3.6)], n=18)
            nozzle(bm, c + V((0, -1.9, 0)), 0.62, 0.6, n=18, rimmat=G, coremat=E, hullmat=H)
            ship.empty("Engine_%d" % (0 if sx > 0 else 1), c + V((0, -1.9, 0)), 0.62)
    ship.part(engines, bevel=0, subsurf=1, angle=180, weighted=False)

    def emitters(bm):
        for sx in (1, -1):
            p = V((sx * 3.65, 12.9, -0.3))
            sphere(bm, p, 0.3, mat=G, seg=12, rings=6)
            for k in range(3):
                a = 2 * math.pi * k / 3
                crystal(bm, p - V((0, 0.4, 0)), (sx * 0.4 * math.cos(a), 1, 0.4 * math.sin(a)), 0.7, 0.08, GL, rng, tipmat=G)
    ship.part(emitters, bevel=0, angle=180, weighted=False)
    tb.free()
    ship.empty("Gun_0", (3.65, 13.3, -0.3), 0.3)
    ship.empty("Gun_1", (-3.65, 13.3, -0.3), 0.3)


# ---------------------------------------------------------------- warlord (Kharz)

KHARZ = dict(paint=(0.13, 0.12, 0.115), paint2=(0.085, 0.08, 0.078), accent=(0.32, 0.2, 0.08), accent_thresh=0.88,
             metal=0.6, rough=0.45, spacing=(1.6, 2.4, 1.2), wear=1.2, grime=1.2, sub_amt=0.35, seam_w=0.03,
             grime_col=(0.035, 0.03, 0.025), bare=(0.5, 0.47, 0.42))


def build_warlord(ship):
    rng = random.Random(47)
    WST = [(-30.0, 8.5, 6.0, 0.0, 0.0), (-22.0, 10.5, 7.2, 0.3, 0.0), (-6.0, 10.2, 7.0, 0.2, 0.0),
           (8.0, 8.6, 6.0, -0.2, 0.0), (18.0, 6.5, 4.6, -0.6, 0.0), (25.0, 4.2, 3.3, -1.0, 0.0)]

    def body_geo(bm):
        rings = [ring10(y, hw, hh, cz, cx, top=0.55, bot=0.75, hi=0.5, lo=-0.45, ridge=0.6, keel=0.8) for y, hw, hh, cz, cx in WST]
        return loft(bm, rings)

    def body(bm):
        r = body_geo(bm)
        side = [f for band in r['bands'] for k, f in enumerate(band) if k in (2, 7, 3, 6)]
        S.inset_faces(bm, side, thickness=0.5, depth=-0.18)
        topf = [f for band in r['bands'] for k, f in enumerate(band) if k in (1, 8)]
        S.inset_faces(bm, topf, thickness=0.6, depth=0.2)
    ship.part(body, bevel=0.12, angle=30)

    def prow(bm):
        # massive axe-blade ram
        pts = [(18.0, -4.8), (24.0, -5.0), (30.0, -6.4), (34.5, -5.6), (35.5, -3.0), (33.5, -0.8), (35.0, 1.6), (31.0, 3.4), (24.0, 4.2), (18.0, 3.8)]
        slab(bm, pts, 3.0, origin=(0, 0, -0.5), u=(0, 1, 0), v=(0, 0, 1), bevel_top=0.5)
        slab(bm, [(26, -4.0), (31.5, -5.0), (33.0, -4.4), (27.0, -3.2)], 0.2, origin=(1.55, 0, -0.5), u=(0, 1, 0), v=(0, 0, 1), mat=L)
        slab(bm, [(26, -4.0), (31.5, -5.0), (33.0, -4.4), (27.0, -3.2)], 0.2, origin=(-1.55, 0, -0.5), u=(0, 1, 0), v=(0, 0, 1), mat=L)
        # tusks
        for sx in (1, -1):
            path = P.bezier((sx * 5.0, 18.0, -3.5), (sx * 6.5, 24.0, -5.0), (sx * 5.5, 29.0, -6.5), (sx * 3.2, 33.0, -6.0), 14)
            P.sweep(bm, path, lambda u: lerp(1.5, 0.05, u ** 1.2), n=8, tip_end=True)
        # armored prow cheek plates
        for sx in (1, -1):
            for j in range(3):
                y = 12.0 + j * 4.0
                hw = lerp(8.6, 4.2, (y - 8) / 17)
                R = Matrix.Rotation(math.radians(sx * 8), 3, 'Z') @ Matrix.Rotation(math.radians(-sx * 10), 3, 'Y')
                box(bm, (sx * (hw + 0.3), y, 0.0), (1.0, 4.6, lerp(9.5, 6.0, j / 2)), rot=R, taper=(0.9, 0.8),
                    mat=L if j == 0 else H)
    ship.part(prow, bevel=0.12, angle=30)

    def superstructure(bm):
        tiers = [(-26.0, -6.0, 7.0, 8.0, 2.2), (-24.0, -10.0, 5.5, 10.0, 2.0), (-21.5, -13.5, 3.6, 12.0, 2.2)]
        for y0, y1, hw, z0, h in tiers:
            box(bm, (0, (y0 + y1) / 2, z0 + h / 2), (hw * 2, y1 - y0, h), taper=(0.85, 0.75))
            for sx in (1, -1):
                for j in range(int((y1 - y0) / 1.2)):
                    box(bm, (sx * hw * 0.93, y0 + 0.8 + j * 1.2, z0 + h * 0.55), (0.1, 0.55, 0.22), mat=G)
        # bridge with glass slit
        box(bm, (0, -17.0, 15.2), (5.0, 5.0, 2.4), taper=(0.7, 0.6))
        box(bm, (0, -14.75, 15.5), (3.0, 0.15, 0.4), mat=GL, rot=Matrix.Rotation(math.radians(-30), 3, 'X'))
        # horns
        for sx in (1, -1):
            path = P.bezier((sx * 2.0, -17.5, 15.5), (sx * 4.5, -18.0, 17.0), (sx * 5.5, -20.0, 20.0), (sx * 5.0, -23.0, 22.5), 12)
            P.sweep(bm, path, lambda u: lerp(0.8, 0.04, u), n=8, tip_end=True)
        cyl(bm, (0, -19.0, 16.4), (0, -19.5, 24.0), 0.18, 0.06, n=8)
        sphere(bm, (0, -19.5, 24.0), 0.25, mat=G, seg=8, rings=4)
        # broadside armor belts
        for sx in (1, -1):
            for j in range(6):
                y = -26.0 + j * 7.0
                hw = interp_hw(WST, y)
                R = Matrix.Rotation(math.radians(sx * 4), 3, 'Y')
                box(bm, (sx * (hw + 0.35), y, -0.6), (0.9, 6.4, 4.2), rot=R, taper=(1.0, 0.9), mat=L if j % 3 == 1 else H)
                for q in range(3):
                    sphere(bm, (sx * (hw + 0.85), y - 2.2 + q * 2.2, 1.0), 0.18, seg=8, rings=4)
    ship.part(superstructure, bevel=0.08, angle=30)

    muz = []

    def guns(bm):
        for y in (14.0, 6.0, -2.0):
            zt = interp_body(WST, y) + 0.6 + 0.6
            m = turret(bm, (0, y, zt), r=2.4, barrels=2, blen=8.0, br=0.35, spread=1.1, h=1.5)
            if y == 14.0:
                muz.extend(m)
        for sx in (1, -1):
            for y in (-14.0, 2.0):
                hw = interp_hw(WST, y)
                box(bm, (sx * (hw - 0.5), y, 3.3), (3.4, 6.0, 1.2))
                m = turret(bm, (sx * (hw - 0.2), y, 4.0), yaw=-sx * math.radians(25), r=1.7, barrels=3, blen=5.5, br=0.22, spread=0.6, h=1.1)
                if y == 2.0:
                    muz.append(m[1])
    ship.part(guns, bevel=0.04, angle=35)
    for i, m in enumerate(muz):
        ship.empty("Gun_%d" % i, m, 0.35)

    def engines(bm):
        box(bm, (0, -32.0, 0.2), (15.0, 4.5, 10.0), taper=(1.05, 1.05))
        k = 0
        for x in (-4.6, 0.0, 4.6):
            for z in (2.4, -2.2):
                cyl(bm, (x, -33.8, z), (x, -34.8, z), 2.45, n=24)
                nozzle(bm, (x, -36.6, z), 2.2, 2.0, n=24, rimmat=H, coremat=E, hullmat=H, ribs=8)
                ship.empty("Engine_%d" % k, (x, -36.6, z), 2.2)
                k += 1
        for sx in (1, -1):
            cyl(bm, (sx * 9.4, -29.0, -3.5), (sx * 9.4, -33.0, -3.5), 1.4, n=16)
            nozzle(bm, (sx * 9.4, -34.0, -3.5), 1.1, 1.0, n=16, rimmat=H, coremat=E, hullmat=H)
            ship.empty("Engine_%d" % k, (sx * 9.4, -34.0, -3.5), 1.1)
            k += 1
        # heat fins
        for sx in (1, -1):
            for j in range(5):
                box(bm, (sx * 7.6, -31.0 + 0.1 * j, 5.4 + j * 0.0), (0.25, 3.6, 2.0 + 0.0), rot=Matrix.Rotation(math.radians(sx * 20), 3, 'Y'))
    ship.part(engines, bevel=0, angle=50, weighted=False)

    def kitbash(bm):
        greeble_on(body_geo, bm, rng, 120, lambda f: f.normal.z > 0.45 and abs(f.calc_center_median().x) > 1.5, 1.4)
        greeble_on(body_geo, bm, rng, 50, lambda f: f.normal.z < -0.45, 1.4)
        greeble_on(body_geo, bm, rng, 50, lambda f: abs(f.normal.x) > 0.6, 1.0, kinds=['vent', 'box', 'pipe', 'plate'])
        # keel fin
        slab(bm, jag([(-28.0, 0.0), (6.0, 0.0), (-6.0, -3.0), (-24.0, -4.5)], rng, depth=0.6, seg=1.8, inward=True), 1.2,
             origin=(0, 0, -7.0), u=(0, 1, 0), v=(0, 0, 1), bevel_top=0.3)
        # spine ridge spikes
        for j in range(5):
            y = 1.0 + j * 3.5
            z = interp_body(WST, y) + 0.6
            P.spike(bm, (0, y, z - 0.3), (0, -0.5, 1), 2.6 - j * 0.3, 0.5, n=4)
    ship.part(kitbash, bevel=0.03, angle=40)


def interp_hw(st, y):
    s = sorted(st, key=lambda a: a[0])
    if y <= s[0][0]:
        return s[0][1]
    for a, b in zip(s, s[1:]):
        if a[0] <= y <= b[0]:
            return lerp(a[1], b[1], (y - a[0]) / (b[0] - a[0]))
    return s[-1][1]


# ---------------------------------------------------------------- sentinel (ancient leviathan)

ANCIENT = dict(organic=True, paint=(0.17, 0.14, 0.11), paint2=(0.09, 0.09, 0.1), accent=(0.38, 0.28, 0.12),
               metal=0.7, rough=0.38, wear=0.6, grime=0.8, bare=(0.55, 0.45, 0.3), bump=0.8)


def build_sentinel(ship):
    rng = random.Random(59)

    def spine(y):
        return V((4.5 * math.sin(y / 22.0), y, 2.5 * math.sin(y / 15.0 + 1.0)))

    def radius(y):
        u = (y + 66) / 116.0  # 0 tail .. 1 neck(+50)
        return lerp(1.6, 10.5, clamp(u, 0, 1) ** 0.8)
    # segmented armor collars
    ys = [50.0 - i * 7.6 for i in range(15)]
    prof = [(0.0, 1.25), (0.55, 0.9), (1.0, 0.25), (0.92, -0.4), (0.45, -0.85), (0.0, -1.0),
            (-0.45, -0.85), (-0.92, -0.4), (-1.0, 0.25), (-0.55, 0.9)]

    def collars(bm):
        for i, y in enumerate(ys):
            L_ = 6.4
            r = radius(y)
            pts = [spine(y + L_ * 0.5 - j * L_ / 4) for j in range(5)]
            rad = [r * 1.18, r * 1.12, r * 1.04, r * 0.96, r * 0.86]
            P.sweep(bm, pts, rad, profile=prof, matfn=lambda a, k: L if (k in (4, 5) and a == 0) else H)
            # dorsal blade fin, swept back
            top = spine(y) + V((0, 0, r * 1.2))
            h = r * 0.9
            slab(bm, jag([(2.0, 0.0), (-3.0, 0.0), (-6.5, h), (-4.0, h * 0.85)], rng, depth=0.4, seg=1.0, inward=True),
                 0.5 + r * 0.04, origin=top - V((0, y, 0)) + V((0, y, -0.3)), u=(0, 1, 0), v=(0, 0, 1), bevel_top=0.15)
            # lateral spines
            for sx in (1, -1):
                b = spine(y) + V((sx * r * 0.95, 0, r * 0.1))
                P.spike(bm, b, (sx * 1.0, -0.8, -0.15), r * 0.55, r * 0.12, n=5)
    ship.part(collars, bevel=0.1, angle=30)

    def core(bm):
        pts = [spine(y) for y in [56.0 - i * 4.0 for i in range(31)]]
        P.sweep(bm, pts, lambda u: max(0.4, radius(56 - u * 120) * 0.72), n=14, mat=G)
        for i, y in enumerate(ys):
            c = spine(y - 3.8)
            P.sweep(bm, [c + V((0, 0.4, 0)), c - V((0, 0.4, 0))], [radius(y) * 0.86] * 2, n=16, mat=H)
    ship.part(core, bevel=0, angle=50, weighted=False)

    def head(bm):
        c = spine(58.0)
        rings = []
        for y, w, h, cz in [(50.0, 9.5, 8.0, 0.5), (55.0, 11.0, 8.4, 1.2), (60.0, 10.5, 7.0, 1.0), (64.5, 8.0, 5.0, 0.3), (67.5, 4.0, 2.6, -0.4)]:
            rings.append(ring10(y, w, h, c.z + cz, c.x, top=0.45, bot=0.6, hi=0.2, lo=-0.5, ridge=1.5, keel=0.6))
        rings.append(V((c.x, 68.6, c.z - 0.6)))
        loft(bm, rings)
        # brow plates over the eyes + swept-back horns
        for sx in (1, -1):
            slab(bm, [(-6.0, 0.0), (6.0, 0.0), (8.0, 1.6), (-4.0, 2.6)], 1.2, origin=c + V((sx * 7.6, 6.0, 3.6)),
                 u=(0, 1, 0), v=V((sx * 0.8, 0, 0.6)).normalized(), bevel_top=0.3)
            path = P.bezier(c + V((sx * 8.0, 0.0, 4.0)), c + V((sx * 13.0, -4.0, 6.0)), c + V((sx * 15.0, -12.0, 8.0)),
                            c + V((sx * 13.0, -20.0, 12.0)), 14)
            P.sweep(bm, path, lambda u: lerp(1.8, 0.08, u), n=8, tip_end=True)
        # crest blades
        for sx in (1, -1, 0):
            o = c + V((sx * 5.5, 0, 7.0 if sx else 9.0))
            pts = jag([(10.0, 0.0), (-4.0, 0.0), (-16.0, 7.0 if sx else 9.5), (-8.0, 4.5)], rng, depth=0.8, seg=2.0, inward=True)
            slab(bm, pts, 1.0, origin=o, u=(0, 1, 0), v=V((sx * 0.5, 0, 1)).normalized(), bevel_top=0.3)
        # mandibles
        for sx in (1, -1):
            path = P.bezier(c + V((sx * 7.5, -2.0, -4.0)), c + V((sx * 10.5, 6.0, -5.5)), c + V((sx * 8.5, 14.0, -5.0)),
                            c + V((sx * 3.5, 19.0, -3.5)), 16)
            P.sweep(bm, path, lambda u: (lerp(2.4, 0.15, u), lerp(1.4, 0.1, u)), n=8, tip_end=True)
            for j in range(4):
                q = path[3 + j * 3]
                P.spike(bm, q, (-sx * 1.0, 0.4, 0.2), 1.6, 0.35, n=4)
        # eyes
        for sx in (1, -1):
            for j in range(3):
                R = Matrix.Rotation(math.radians(sx * 25), 3, 'Z') @ Matrix.Rotation(math.radians(sx * (20 + j * 8)), 3, 'Y')
                box(bm, c + V((sx * (8.6 - j * 1.1), 4.0 + j * 2.4, 2.4 - j * 0.6)), (2.6 - j * 0.5, 0.3, 0.45), mat=G, rot=R)
        sphere(bm, c + V((0, 9.0, -3.4)), 2.2, mat=G, seg=20, rings=10, scale=(1.2, 1.0, 0.7))
    ship.part(head, bevel=0.1, angle=30)
    hc = spine(58.0)
    ship.empty("Gun_0", hc + V((0, 11.5, -3.4)), 1.2)

    def fins(bm):
        for (y, s) in ((40.0, 1.0), (8.0, 0.75)):
            for sx in (1, -1):
                b = spine(y)
                r = radius(y)
                outline = [(0.0, 3.0), (0.0, -4.0), (9.0 * s * 2.2, -16.0 * s), (14.0 * s * 2.0, -24.0 * s), (12.0 * s * 2.0, -12.0 * s),
                           (9.0 * s * 2.0, -2.0 * s)]
                outline = [(sx * x, yy) for x, yy in outline]
                if sx < 0:
                    outline = list(reversed(outline))
                o = b + V((sx * r * 0.8, 0, -r * 0.2))
                uu = V((1, 0, -0.25)).normalized()
                slab(bm, jag(outline, rng, depth=0.9, seg=2.4, prob=0.5, inward=True), 0.9, origin=o - V((0, 0, 0)), u=uu, v=(0, 1, 0), bevel_top=0.25)
                # glowing ray lines on the fin
                nrm = uu.cross(V((0, 1, 0)))
                for j in range(3):
                    a = V((0, 0, 0))
                    tip = V((sx * (18 - j * 4) * s * 1.4, (-20 + j * 6) * s, 0))
                    p0 = o + uu * (sx * 2.0) + nrm * 0.48
                    p1 = o + uu * tip.x + V((0, tip.y, 0)) + nrm * 0.48
                    prism(bm, p0, p1, 0.18, 0.06, mat=G)
                if y == 40.0:
                    sphere(bm, o + uu * (sx * 1.5) + V((0, 1.5, 1.5)), 1.0, mat=G, seg=12, rings=6)
    ship.part(fins, bevel=0.08, angle=30)
    for i, sx in enumerate((1, -1)):
        o = spine(40.0) + V((sx * radius(40.0) * 0.8, 0, -radius(40.0) * 0.2))
        ship.empty("Gun_%d" % (i + 1), o + V((sx * 1.4, 2.6, 1.2)), 1.0)

    def tail(bm):
        t = spine(-62.0)
        for ang in (-35, 0, 35):
            R = Matrix.Rotation(math.radians(ang), 3, 'Y')
            u = R @ V((1, 0, 0))
            slab(bm, jag([(0.0, 2.0), (0.0, -2.0), (6.0, -14.0), (2.0, -9.0)], rng, depth=0.5, seg=1.4, inward=True), 0.6,
                 origin=t, u=u, v=(0, 1, 0), bevel_top=0.15)
            slab(bm, jag([(0.0, 2.0), (-2.0, -9.0), (-6.0, -14.0), (0.0, -2.0)], rng, depth=0.5, seg=1.4, inward=True), 0.6,
                 origin=t, u=u, v=(0, 1, 0), bevel_top=0.15)
        for sx in (1, -1):
            c = spine(-58.0) + V((sx * 2.2, 0, -0.5))
            lathe(bm, c, (0, 1, 0), [(0.0, -4.0), (1.4, -3.6), (1.7, -1.0), (1.2, 2.5), (0.0, 3.5)], n=16)
            nozzle(bm, c + V((0, -4.6, 0)), 1.2, 1.0, n=16, rimmat=G, coremat=E, hullmat=H)
    ship.part(tail, bevel=0.06, angle=30)
    for i, sx in enumerate((1, -1)):
        ship.empty("Engine_%d" % i, spine(-58.0) + V((sx * 2.2, -4.6, -0.5)), 1.2)

    def greeb(bm):
        def src(b):
            for y in ys:
                r = radius(y)
                pts = [spine(y + 3.2 - j * 1.6) for j in range(5)]
                P.sweep(b, pts, [r * 1.18, r * 1.12, r * 1.04, r * 0.96, r * 0.86], profile=prof)
        sb = _src(src)
        greebles(bm, sample_surface(sb, 70, rng, lambda f: f.normal.z > 0.2), rng, scale=1.6, mats=(H, H, G),
                 kinds=['plate', 'plate', 'box', 'cyl'])
        sb.free()
    ship.part(greeb, bevel=0.03, angle=40)


def build_gunship_run():
    run_ship("gunship", build_gunship, PIRATE, k=1.4, length=32.0, hero=(-1.0, 1.2, 0.65),
             overlay=dict(rust=1.0, scorch=0.6, heat=0.5, seed=7.0))


def build_warden_run():
    run_ship("warden", build_warden, VEYR, k=1.2, length=26.0, glow=(0.6, 0.25, 1.0), core=(0.7, 0.3, 1.0), glow_strength=2.0, hero=(1.0, 1.1, 0.7))


def build_warlord_run():
    run_ship("warlord", build_warlord, KHARZ, k=2.6, length=66.0, hero=(-1.0, 1.0, 0.55),
             overlay=dict(rust=0.35, scorch=0.5, heat=0.8, seed=11.0, rust_col=(0.12, 0.06, 0.03)))


def build_sentinel_run():
    run_ship("sentinel", build_sentinel, ANCIENT, k=4.0, length=130.0, glow=(0.2, 0.9, 0.75), core=(0.3, 1.0, 0.8), glow_strength=3.0, hero=(1.0, 0.9, 0.6),
             views=[((-1.0, 0.6, 0.5), False), ((-0.9, -1.2, 0.5), False), ((1.0, 0.0, 0.0), True), ((0.0, 0.0001, 1.0), True)])


def build_raider_run():
    run_ship("raider", build_raider, PIRATE, k=1.0, length=20.0, hero=(-1.0, 1.25, 0.7),
             overlay=dict(rust=0.9, scorch=0.7, heat=0.6, seed=3.0))


# ================================================================ B. station

def truss(bm, p0, p1, w, seg_len, mat=H, r=0.22, hint=(0, 0, 1)):
    p0, p1 = V(p0), V(p1)
    ax = p1 - p0
    L_ = ax.length
    d = ax / L_
    t, b, _ = frame_axes(d, hint)
    corners = [(t + b) * (w / 2), (t - b) * (w / 2), (-t - b) * (w / 2), (-t + b) * (w / 2)]
    n = max(1, round(L_ / seg_len))
    for c in corners:
        cyl(bm, p0 + c, p1 + c, r * 1.3, n=6, mat=mat)
    for i in range(n + 1):
        q = p0 + ax * (i / n)
        for k in range(4):
            cyl(bm, q + corners[k], q + corners[(k + 1) % 4], r, n=4, mat=mat, cap=False)
        if i < n:
            q2 = p0 + ax * ((i + 1) / n)
            for k in range(4):
                a, c2 = (k, (k + 1) % 4) if i % 2 == 0 else ((k + 1) % 4, k)
                cyl(bm, q + corners[a], q2 + corners[c2], r * 0.8, n=4, mat=mat, cap=False)


def dish(bm, c, axis, R, depth, mat_in=H, glow=G, struts=3):
    axis = V(axis).normalized()
    prof = [(0.0, -depth * 0.25), (R * 0.15, -depth * 0.2), (R * 0.5, depth * 0.22), (R * 0.85, depth * 0.7), (R, depth),
            (R * 1.03, depth * 0.98), (R * 0.95, depth * 0.85), (R * 0.5, depth * 0.35), (R * 0.12, depth * 0.05), (0.0, 0.0)]
    lathe(bm, c, axis, prof, n=40, mats=[H, H, H, H, L, H, H, H, H])
    f = V(c) + axis * (depth + R * 0.55)
    t, b, _ = frame_axes(axis, (0, 0, 1))
    for k in range(struts):
        a = 2 * math.pi * k / struts
        rim = V(c) + axis * depth + (t * math.cos(a) + b * math.sin(a)) * R * 0.92
        cyl(bm, rim, f, R * 0.012, n=6)
    cyl(bm, f - axis * R * 0.08, f + axis * R * 0.06, R * 0.05, n=12)
    sphere(bm, f + axis * R * 0.07, R * 0.03, mat=glow, seg=10, rings=5)
    cyl(bm, V(c) - axis * depth * 0.2, V(c) - axis * (depth * 0.2 + R * 0.35), R * 0.08, R * 0.12, n=12)


STATION = dict(paint=(0.4, 0.405, 0.41), paint2=(0.28, 0.29, 0.31), accent=(0.55, 0.33, 0.08), accent_thresh=0.9,
               metal=0.5, rough=0.45, spacing=(1.2, 1.2, 1.0), wear=0.6, grime=0.9, sub_amt=0.35, seam_w=0.024,
               grime_col=(0.06, 0.055, 0.05), bare=(0.6, 0.6, 0.6))


def build_station():
    S.reset_scene()
    mats, socks = S.make_materials(STATION)
    P.scale_space(mats["Hull"], 5.0)
    P.overlay_hull(mats["Hull"], socks, rust=0.25, rust_col=(0.12, 0.08, 0.05), seed=2.0,
                   stripes=(2, 14.0, 0.06, (0.6, 0.35, 0.06)))
    gl = mats["Glow"].node_tree.nodes["Principled BSDF"]
    gl.inputs['Emission Color'].default_value = (1.0, 0.82, 0.58, 1)
    order = ["Hull", "Livery", "Glow", "Glass"]
    st = Prop("station", mats, order)
    rng = random.Random(101)
    RR = 125.0  # ring centre radius

    # ---------------- rotating ring (separate object)
    ring_prof = [(-8.0, -5.0), (-6.0, -7.0), (6.0, -7.0), (8.0, -4.5), (8.0, 4.5), (6.0, 7.0), (-6.0, 7.0), (-8.0, 5.0)]
    NSEG = 120

    def ring_geo(bm):
        rings = []
        for i in range(NSEG + 1):
            a = 2 * math.pi * i / NSEG
            d = V((math.cos(a), math.sin(a), 0))
            rings.append([d * (RR + x) + V((0, 0, z)) for x, z in ring_prof])
        return loft(bm, rings, matfn=lambda i, k: L if (k == 5 and i % 10 in (4, 5)) else H, cap_start=False, cap_end=False)

    def ring(bm):
        r = ring_geo(bm)
        sel = [f for i, band in enumerate(r['bands']) if i % 4 != 0 for k, f in enumerate(band) if k in (1, 2, 6)]
        S.inset_faces(bm, sel, thickness=0.5, depth=-0.25)
        # bulkhead collars
        for j in range(12):
            a = 2 * math.pi * j / 12
            d = V((math.cos(a), math.sin(a), 0))
            tg = V((-d.y, d.x, 0))
            R = Matrix((tg, d, V((0, 0, 1)))).transposed()
            box(bm, d * RR, (6.0, 19.0, 17.0), rot=R)
            box(bm, d * RR + V((0, 0, 8.8)), (4.0, 12.0, 1.2), rot=R)
            box(bm, d * (RR + 9.6), (6.2, 0.3, 1.0), mat=G, rot=R)
        # windows: outer wall rows, side rows
        for i in range(NSEG):
            if i % 10 == 0:
                continue
            a = 2 * math.pi * (i + 0.5) / NSEG
            d = V((math.cos(a), math.sin(a), 0))
            tg = V((-d.y, d.x, 0))
            R = Matrix((tg, d, V((0, 0, 1)))).transposed()
            seglen = 2 * math.pi * (RR + 8) / NSEG
            for z in (2.2, -0.6):
                if rng.random() < 0.85:
                    box(bm, d * (RR + 8.05) + V((0, 0, z)), (seglen * 0.62, 0.2, 0.7), mat=G, rot=R)
            seglen_i = 2 * math.pi * (RR - 8) / NSEG
            if rng.random() < 0.7:
                box(bm, d * (RR - 8.05) + V((0, 0, 1.0)), (seglen_i * 0.55, 0.2, 0.6), mat=G, rot=R)
            for zs in (1, -1):
                if rng.random() < 0.6:
                    box(bm, d * (RR + 1.5) + V((0, 0, zs * 7.02)), (seglen * 0.4, 0.8, 0.12), mat=G, rot=R)
        # spokes + rotating bearing collar
        lathe(bm, (0, 0, 0), (0, 0, 1), [(27.0, -5.5), (31.0, -4.5), (32.0, -2.0), (32.0, 2.0), (31.0, 4.5), (27.0, 5.5)], n=48,
              cap_start=False, cap_end=False)
        for j in range(4):
            a = 2 * math.pi * j / 4 + math.pi / 4
            d = V((math.cos(a), math.sin(a), 0))
            tg = V((-d.y, d.x, 0))
            prism(bm, d * 31.0, d * (RR - 7.5), 2.2, 2.6)
            for sx in (1, -1):
                cyl(bm, d * 31.0 + tg * sx * 3.0 + V((0, 0, 1.0)), d * (RR - 7.5) + tg * sx * 3.0 + V((0, 0, 1.0)), 0.6, n=8)
            box(bm, d * ((31 + RR - 7.5) / 2) + V((0, 0, 2.62)), (0.35, RR - 40.0, 0.1), mat=G,
                rot=Matrix((tg, d, V((0, 0, 1)))).transposed())
            for q in range(4):
                c = d * (40.0 + q * 19.0)
                cyl(bm, c - d * 1.2, c + d * 1.2, 3.4, n=12)
    st.part(ring, group="Ring", bevel=0.25, angle=35)

    def ring_greeb(bm):
        greeble_on(ring_geo, bm, rng, 140, lambda f: abs(f.normal.z) > 0.7, 2.6)
        greeble_on(ring_geo, bm, rng, 60, lambda f: abs(f.normal.z) < 0.3 and (f.calc_center_median().length > RR), 2.0,
                   kinds=['box', 'vent', 'plate', 'pipe'])
    st.part(ring_greeb, group="Ring", bevel=0.06, angle=40)

    # ---------------- static station
    def hub_geo(bm):
        return lathe(bm, (0, 0, 0), (0, 0, 1), [(7.0, -31.0), (15.0, -27.0), (22.0, -21.0), (26.0, -15.0), (26.0, 15.0),
                                               (22.0, 21.0), (15.0, 27.0), (7.0, 31.0)], n=48)

    def hub(bm):
        r = hub_geo(bm)
        sel = [f for i, band in enumerate(r['bands']) if i in (1, 5) for k, f in enumerate(band) if k % 3 != 0]
        S.inset_faces(bm, sel, thickness=0.6, depth=-0.3)

    def hub_post(obj):
        def cutter(bm):
            for j in range(4):
                a = 2 * math.pi * j / 4
                d = V((math.cos(a), math.sin(a), 0))
                tg = V((-d.y, d.x, 0))
                R = Matrix((tg, d, V((0, 0, 1)))).transposed()
                box(bm, d * 26.0 + V((0, 0, 9.5)), (13.0, 5.0, 8.0), rot=R)
        boolean(obj, cutter, mats, order)
    st.part(hub, bevel=0.2, angle=30, post=hub_post)

    def hub_detail(bm):
        for j in range(4):
            a = 2 * math.pi * j / 4
            d = V((math.cos(a), math.sin(a), 0))
            tg = V((-d.y, d.x, 0))
            R = Matrix((tg, d, V((0, 0, 1)))).transposed()
            # hangar door (two leaves) recessed in the cut + light frame
            for sx in (-1, 1):
                box(bm, d * 24.1 + tg * sx * 3.1 + V((0, 0, 9.5)), (6.0, 0.6, 7.6), rot=R)
                for q in range(4):
                    box(bm, d * 24.45 + tg * sx * 3.1 + V((0, 0, 6.6 + q * 1.9)), (5.6, 0.2, 0.3), rot=R)
            box(bm, d * 26.3 + V((0, 0, 13.8)), (13.0, 0.4, 0.35), mat=G, rot=R)
            for sx in (-1, 1):
                box(bm, d * 26.3 + tg * sx * 6.8 + V((0, 0, 9.5)), (0.35, 0.4, 8.0), mat=G, rot=R)
            box(bm, d * 26.4 + V((0, 0, 5.2)), (13.0, 0.4, 0.5), mat=L, rot=R)
        # window rows around the hub
        for z in (-12.0, -9.0, 17.5):
            rr = 26.0 if abs(z) < 15 else 24.4
            for i in range(64):
                if (i + int(z)) % 8 == 0:
                    continue
                a = 2 * math.pi * (i + 0.5) / 64
                d = V((math.cos(a), math.sin(a), 0))
                tg = V((-d.y, d.x, 0))
                R = Matrix((tg, d, V((0, 0, 1)))).transposed()
                if abs(z) > 15:
                    R = R @ Matrix.Rotation(math.radians(-34), 3, 'X')
                box(bm, d * (rr + 0.05) + V((0, 0, z)), (1.4, 0.3, 0.7), mat=G, rot=R)
    st.part(hub_detail, bevel=0.04, angle=40)

    def spine(bm):
        # upper/lower spine trunks (octagonal), with collars
        for z0, z1 in ((30.0, 112.0), (-30.0, -118.0)):
            prism(bm, (0, 0, z0), (0, 0, z1), 6.0, 6.0, chamfer=0.3, up=(1, 0, 0))
            step = 8.0 if z1 > 0 else -8.0
            z = z0 + step
            while abs(z) < abs(z1) - 4:
                prism(bm, (0, 0, z - 0.8), (0, 0, z + 0.8), 7.2, 7.2, chamfer=0.3, up=(1, 0, 0))
                z += step
            for ang in (0, 90, 180, 270):
                d = V((math.cos(math.radians(ang + 22.5)), math.sin(math.radians(ang + 22.5)), 0))
                box(bm, d * 6.0 + V((0, 0, (z0 + z1) / 2)), (0.4, 0.4, abs(z1 - z0) - 6), mat=G,
                    rot=Matrix.Rotation(math.radians(ang + 22.5), 3, 'Z'))
        # reactor / thruster cone at the bottom
        lathe(bm, (0, 0, -118.0), (0, 0, -1), [(7.0, 0.0), (11.0, 3.0), (11.0, 8.0), (8.0, 11.0), (5.0, 11.5), (0.0, 11.5)], n=32,
              mats=[H, L, H, H, G])
        # top cap
        lathe(bm, (0, 0, 112.0), (0, 0, 1), [(6.5, 0.0), (9.0, 2.0), (9.0, 6.0), (5.0, 9.0), (0.0, 9.5)], n=32)
    st.part(spine, bevel=0.15, angle=30)

    def cargo(bm):
        # container racks around the upper spine
        for layer, z in enumerate((44.0, 59.0)):
            lathe(bm, (0, 0, z - 7.5), (0, 0, 1), [(5.0, -0.6), (19.0, -0.6), (19.0, 0.6), (5.0, 0.6)], n=24)
            for j in range(14):
                a = 2 * math.pi * (j + 0.5 * layer) / 14
                if rng.random() < 0.12:
                    continue
                for ring_r in (10.5, 15.5):
                    d = V((math.cos(a), math.sin(a), 0))
                    R = Matrix.Rotation(a, 3, 'Z')
                    m = L if rng.random() < 0.35 else H
                    c = d * ring_r + V((0, 0, z))
                    box(bm, c, (3.6, 3.6, 13.0), mat=m, rot=R)
                    for q in range(5):
                        box(bm, c + V((0, 0, -5.0 + q * 2.5)), (3.75, 3.75, 0.25), mat=H, rot=R)
        lathe(bm, (0, 0, 66.5), (0, 0, 1), [(5.0, -0.6), (19.0, -0.6), (19.0, 0.6), (5.0, 0.6)], n=24)
    st.part(cargo, bevel=0.08, angle=40)

    def arrays(bm):
        z = 84.0
        for sx in (1, -1):
            truss(bm, (sx * 6.0, 0, z), (sx * 140.0, 0, z), 3.0, 6.0, hint=(0, 0, 1))
            for k in range(3):
                x0 = sx * (24.0 + k * 39.0)
                for sy in (1, -1):
                    c = V((x0 + sx * 17.0, sy * 10.0, z))
                    box(bm, c, (34.0, 15.0, 0.25), mat=GL)
                    box(bm, c + V((0, sy * 7.6, 0)), (34.4, 0.5, 0.6))
                    box(bm, c - V((0, sy * 7.6, 0)), (34.4, 0.5, 0.6))
                    for q in range(5):
                        box(bm, c + V((-17.0 + q * 8.5, 0, 0)), (0.4, 15.0, 0.5))
                    box(bm, c + V((0, 0, 0)), (34.0, 0.3, 0.45))
                    cyl(bm, V((x0 + sx * 17.0, sy * 2.0, z)), V((x0 + sx * 17.0, sy * 2.6, z)), 1.2, n=10)
            sphere(bm, (sx * 140.0, 0, z), 0.8, mat=G, seg=8, rings=4)
        prism(bm, (-6.5, 0, z - 3), (6.5, 0, z - 3), 2.5, 2.5)
        prism(bm, (-6.5, 0, z + 3), (6.5, 0, z + 3), 2.5, 2.5)
    st.part(arrays, bevel=0.05, angle=40)

    def comms(bm):
        dish(bm, V((14.0, 0, 104.0)), (0.6, 0.3, 0.8), 9.0, 2.6)
        truss(bm, (6.0, 0, 102.0), (13.0, 0, 103.0), 1.6, 3.0, hint=(0, 0, 1))
        dish(bm, V((-10.0, 6.0, 98.0)), (-0.6, 0.5, 0.6), 5.5, 1.6)
        truss(bm, (-5.5, 2.5, 97.0), (-9.5, 5.5, 97.6), 1.2, 2.0, hint=(0, 0, 1))
        dish(bm, V((0.0, -14.0, -104.0)), (0.0, -0.7, -0.7), 7.0, 2.0)
        truss(bm, (0, -6.0, -104.0), (0, -12.5, -104.0), 1.4, 2.5, hint=(0, 0, 1))
        # antenna masts
        for (x, y, h, tilt) in ((0, 0, 30.0, 0.0), (4.0, 3.0, 16.0, 0.15), (-3.5, -3.0, 20.0, -0.12)):
            b = V((x, y, 120.0))
            t = b + V((tilt * h, 0, h))
            cyl(bm, b, t, 0.5, 0.15, n=8)
            for q in range(4):
                p = b.lerp(t, 0.3 + q * 0.17)
                cyl(bm, p - V((2.5 - q * 0.5, 0, 0)), p + V((2.5 - q * 0.5, 0, 0)), 0.12, n=6)
            sphere(bm, t, 0.4, mat=G, seg=8, rings=4)
        for (x, y) in ((3.0, 0.0), (-2.0, 2.5)):
            b = V((x, y, -129.0))
            cyl(bm, b, b - V((0, 0, 14.0)), 0.4, 0.12, n=8)
            sphere(bm, b - V((0, 0, 14.0)), 0.35, mat=G, seg=8, rings=4)
    st.part(comms, bevel=0.03, angle=40)

    def docks(bm):
        # four docking arms with approach light strips and docking collars
        z = -52.0
        lathe(bm, (0, 0, z), (0, 0, 1), [(6.0, -4.0), (12.0, -3.0), (13.0, 0.0), (12.0, 3.0), (6.0, 4.0)], n=32)
        for j in range(4):
            a = 2 * math.pi * j / 4 + math.pi / 4
            d = V((math.cos(a), math.sin(a), 0))
            tg = V((-d.y, d.x, 0))
            R = Matrix((tg, d, V((0, 0, 1)))).transposed()
            box(bm, d * 34.0 + V((0, 0, z)), (5.0, 44.0, 3.6), rot=R, taper=(0.8, 0.8))
            for q in range(10):
                box(bm, d * (16.0 + q * 4.2) + V((0, 0, z + 1.85)), (0.8, 1.6, 0.2), mat=G, rot=R)
                for sx in (1, -1):
                    box(bm, d * (16.0 + q * 4.2) + tg * sx * 2.52 + V((0, 0, z)), (0.12, 1.0, 0.5), mat=G, rot=R)
            c = d * 58.0 + V((0, 0, z))
            cyl(bm, c - d * 2.5, c + d * 1.5, 5.0, n=24)
            cyl(bm, c + d * 1.5, c + d * 2.0, 4.0, n=24, mat=G)
            cyl(bm, c + d * 2.0, c + d * 2.4, 4.4, n=24)
            for k in range(6):
                aa = 2 * math.pi * k / 6
                off = tg * math.cos(aa) * 5.2 + V((0, 0, math.sin(aa) * 5.2))
                box(bm, c + off - d * 0.5, (1.0, 1.0, 1.0), rot=R)
            # docking clamp pylons
            for sx in (1, -1):
                box(bm, d * 52.0 + tg * sx * 4.5 + V((0, 0, z - 4.0)), (1.2, 3.0, 8.0), rot=R)
        # shipyard frame below the docks
        zf = -82.0
        for sx in (1, -1):
            truss(bm, (sx * 14.0, -30.0, zf - 12), (sx * 14.0, 30.0, zf - 12), 2.6, 5.0, hint=(0, 0, 1))
            truss(bm, (sx * 14.0, -30.0, zf + 12), (sx * 14.0, 30.0, zf + 12), 2.6, 5.0, hint=(0, 0, 1))
            for y in (-30.0, 0.0, 30.0):
                truss(bm, (sx * 14.0, y, zf - 12), (sx * 14.0, y, zf + 12), 2.6, 5.0, hint=(0, 1, 0))
        for y in (-30.0, 30.0):
            truss(bm, (-14.0, y, zf + 12), (14.0, y, zf + 12), 2.6, 5.0, hint=(0, 0, 1))
        for q in range(9):
            y = -28.0 + q * 7.0
            for sx in (1, -1):
                box(bm, (sx * 15.6, y, zf + 12), (0.4, 1.4, 0.4), mat=G)
                box(bm, (sx * 15.6, y, zf - 12), (0.4, 1.4, 0.4), mat=G)
        # gantry crane
        box(bm, (0, 6.0, zf + 13.5), (30.0, 3.0, 2.0))
        box(bm, (3.0, 6.0, zf + 9.5), (2.0, 2.0, 6.0))
        # half-built hull in the frame
        rings = [ring10(y, 5.0, 3.0, zf - 1.0, 0.0) for y in (-20.0, -8.0, 6.0)]
        loft(bm, rings, mat=H)
        for y in (8.0, 12.0, 16.0, 20.0):
            pts = ring10(y, 5.0 - (y - 6) * 0.2, 3.0 - (y - 6) * 0.1, zf - 1.0, 0.0)
            for a_, b_ in zip(pts, pts[1:] + pts[:1]):
                prism(bm, a_, b_, 0.25, 0.25)
        # connect frame to spine
        truss(bm, (0, 0, zf + 12), (0, 0, zf + 24), 3.0, 4.0, hint=(1, 0, 0))
    st.part(docks, bevel=0.05, angle=40)

    def greeb(bm):
        greeble_on(hub_geo, bm, rng, 90, lambda f: abs(f.normal.z) > 0.5, 2.2)
        greeble_on(hub_geo, bm, rng, 40, lambda f: abs(f.normal.z) < 0.3 and not (4.0 < f.calc_center_median().z < 15.0), 1.6,
                   kinds=['vent', 'box', 'pipe', 'plate'])
    st.part(greeb, bevel=0.05, angle=40)

    objs = st.finalize(center=False, body_name="Station")
    meshes = list(objs.values())
    out = os.path.join(MODELS, "station.glb")
    if OPTS['bake']:
        S.unwrap_hull(meshes, margin=0.002)
        paths = P.bake(meshes, mats["Hull"], socks, BUILD, "station", size=OPTS.get('station_tex', 2048), samples=16)
        S.rebuild_hull_textured(mats["Hull"], paths)
        S.export_glb(out, meshes, [])
    report("station", meshes, out)
    if OPTS['preview']:
        tint_preview(mats, livery=(0.5, 0.08, 0.05), glow=(1.0, 0.8, 0.55))
        cam = preview_scene()
        P.preview_set(meshes, "station", PREVIEWS, VIEWS, (1.0, -0.8, 0.45),
                      [((1.0, 0.3, 0.12), False), ((-0.7, -1.0, -0.5), False), ((0.0, 0.0, 1.0), True), ((0.0, -1.0, 0.0), True)])
        # close-up of the hub / docking area
        cam.location = V((95.0, -120.0, -20.0))
        cam.rotation_euler = (V((0, 0, -25.0)) - cam.location).to_track_quat('-Z', 'Y').to_euler()
        cam.data.lens = 35
        cam.data.clip_end = 2000
        S.render(os.path.join(PREVIEWS, "props_station_closeup.png"), samples=96)


# ================================================================ D. wreck

def jag_cutter(bm, M, w, h, depth, amp, rng, n=16, glow_frac=0.4, gmat=1, seed=0.0):
    """Closed box whose +Y face is a torn, jagged sheet (local y~0); the solid fills y<0 down to -depth.
    Face materials on the torn sheet are randomly Hull/Glow (become ember rims after a boolean)."""
    off = V((seed * 13.1, seed * 7.7, seed * 3.3))
    grid = []
    for i in range(n + 1):
        row = []
        for j in range(n + 1):
            x = -w / 2 + w * i / n
            z = -h / 2 + h * j / n
            dy = noise.fractal(V((x * 0.35, z * 0.35, 0)) + off, 0.7, 2.0, 4) * amp
            dy += rng.uniform(-0.35, 0.35) * amp * (1 if (i + j) % 2 else -0.4)
            if i in (0, n) or j in (0, n):
                dy = 0.0
            row.append(bm.verts.new(M @ V((x, dy, z))))
        grid.append(row)
    for i in range(n):
        for j in range(n):
            f = bm.faces.new((grid[i][j], grid[i + 1][j], grid[i + 1][j + 1], grid[i][j + 1]))
            f.material_index = gmat if rng.random() < glow_frac else 0
    bnd = [grid[i][0] for i in range(n)] + [grid[n][j] for j in range(n)] + \
          [grid[i][n] for i in range(n, 0, -1)] + [grid[0][j] for j in range(n, 0, -1)]
    back = [bm.verts.new(M @ (M.inverted() @ v.co - V((0, depth, 0)))) for v in bnd]
    for k in range(len(bnd)):
        k2 = (k + 1) % len(bnd)
        bm.faces.new((bnd[k], bnd[k2], back[k2], back[k]))
    bm.faces.new(list(reversed(back)))


def blast_cutter(bm, c, r, rng, glow_frac=0.5, gmat=1, seed=0.0):
    ret = bmesh.ops.create_icosphere(bm, subdivisions=3, radius=r, matrix=Matrix.Translation(V(c)))
    vs = ret['verts']
    off = V((seed, seed * 2, seed * 3))
    for v in vs:
        d = (v.co - V(c))
        v.co = V(c) + d * (1 + 0.45 * noise.fractal(d.normalized() * 2.0 + off, 0.6, 2.0, 4))
    fs = set(f for v in vs for f in v.link_faces)
    for f in fs:
        f.material_index = gmat if rng.random() < glow_frac else 0


WRECK = dict(paint=(0.15, 0.15, 0.155), paint2=(0.09, 0.09, 0.095), accent=(0.3, 0.12, 0.08), accent_thresh=0.9,
             metal=0.45, rough=0.6, spacing=(1.0, 1.4, 0.8), wear=1.6, grime=1.8, sub_amt=0.4,
             grime_col=(0.03, 0.025, 0.02), bare=(0.45, 0.42, 0.4))


def build_wreck():
    S.reset_scene()
    mats, socks = S.make_materials(WRECK)
    P.overlay_hull(mats["Hull"], socks, rust=0.3, scorch=1.4, heat=1.0, seed=5.0)
    mats["Glow"].node_tree.nodes["Principled BSDF"].inputs['Emission Color'].default_value = (1.0, 0.55, 0.22, 1)
    order = ["Hull", "Glow"]
    WG = 1
    st = Prop("wreck", mats, order)
    rng = random.Random(77)
    R10 = dict(top=0.55, bot=0.75, hi=0.35, lo=-0.4, ridge=0.15, keel=0.2)

    def shell(rings_spec, tip=None):
        def b(bm):
            rings = [ring10(y, hw, hh, cz, 0.0, **R10) for y, hw, hh, cz in rings_spec]
            if tip:
                rings.append(V(tip))
            loft(bm, rings, cap_start=False, cap_end=False)
        return b

    def ribs(bm, spec, ys, scale=0.93):
        for y in ys:
            hw = interp_hw([(a, b_, c_, d_, 0) for a, b_, c_, d_ in spec], y)
            hh = interp_body([(a, b_, c_, 0.0, 0) for a, b_, c_, d_ in spec], y)
            pts = ring10(y, hw * scale, hh * scale, spec[0][3], 0.0, **R10)
            for a_, b_ in zip(pts, pts[1:] + pts[:1]):
                prism(bm, a_, b_, 0.09, 0.14, up=(0, 1, 0))

    def torn(cuts, holes=(), thick=0.16):
        def post(obj):
            solidify(obj, thick)
            for M, w, h, amp, gf in cuts:
                boolean(obj, lambda bm: jag_cutter(bm, M, w, h, 20.0, amp, rng, glow_frac=gf, seed=rng.random() * 50), mats, order)
            for c, r, gf in holes:
                boolean(obj, lambda bm: blast_cutter(bm, c, r, rng, glow_frac=gf, seed=rng.random() * 50), mats, order)
        return post

    def Mcut(y, flip=False, tilt=(0.0, 0.0)):
        R = Matrix.Rotation(math.radians(tilt[0]), 4, 'X') @ Matrix.Rotation(math.radians(tilt[1]), 4, 'Z')
        if flip:
            R = R @ Matrix.Rotation(math.pi, 4, 'Z')
        return Matrix.Translation(V((0, y, 0))) @ R

    # bow: nose section torn off behind the cockpit, blast hole in the flank
    BOW = [(-1.5, 2.4, 1.6, 0.0), (2.0, 2.3, 1.55, 0.0), (5.0, 1.7, 1.15, -0.1), (7.6, 0.8, 0.55, -0.2)]
    st.part(shell(BOW, tip=(0, 8.6, -0.25)), group="Wreck_Bow", bevel=0, angle=35,
            post=torn([(Mcut(0.4, tilt=(12, 8)), 8, 6, 1.4, 0.45)], holes=[((2.3, 3.4, 0.4), 1.1, 0.6)]))

    def bow_inner(bm):
        ribs(bm, BOW, [0.6, 1.8, 3.0, 4.2])
        # deck plate + dangling cables out of the tear
        box(bm, (0, 2.0, -0.7), (3.2, 4.4, 0.08))
        for k in range(5):
            a = V((rng.uniform(-1.4, 1.4), rng.uniform(0.6, 1.8), rng.uniform(-0.8, 0.9)))
            b = a + V((rng.uniform(-1, 1), -rng.uniform(1.2, 3.0), rng.uniform(-1.5, 0.2)))
            cable(bm, [a, a.lerp(b, 0.5) + V((0, 0, -0.4)), b], rng.uniform(0.03, 0.07), n=6, rng=rng, wobble=0.2)
            sphere(bm, b, 0.07, mat=WG, seg=6, rings=3)
    st.part(bow_inner, group="Wreck_Bow", bevel=0, angle=40)

    # midsection: both ends torn, roof blown open, ribs protruding
    MID = [(-6.5, 2.7, 1.8, 0.1), (-3.0, 2.8, 1.85, 0.1), (0.5, 2.6, 1.7, 0.05)]
    st.part(shell(MID), group="Wreck_Mid", bevel=0, angle=35,
            post=torn([(Mcut(-0.4, flip=True, tilt=(-10, 5)), 9, 7, 1.6, 0.4), (Mcut(-5.6, tilt=(8, -12)), 9, 7, 1.5, 0.4)],
                      holes=[((0.6, -3.0, 1.9), 1.9, 0.35), ((-2.6, -2.0, -0.6), 1.2, 0.6)]))

    def mid_inner(bm):
        ribs(bm, MID, [-6.4, -5.2, -4.0, -2.8, -1.6, -0.4, 0.4])
        for sx in (1, -1):
            prism(bm, (sx * 1.6, -6.6, -1.4), (sx * 1.6, 0.6, -1.4), 0.12, 0.2)
            prism(bm, (sx * 2.2, -6.6, 0.8), (sx * 2.2, 0.6, 0.8), 0.1, 0.16)
        box(bm, (0.0, -3.0, -1.0), (3.6, 5.6, 0.1))
        for k in range(8):
            y0 = rng.choice((-0.6, -5.4))
            dirn = 1 if y0 > -3 else -1
            a = V((rng.uniform(-1.8, 1.8), y0, rng.uniform(-1.0, 1.2)))
            b = a + V((rng.uniform(-1.5, 1.5), dirn * rng.uniform(1.5, 3.5), rng.uniform(-2.0, 0.5)))
            cable(bm, [a, a.lerp(b, 0.4) + V((0, 0, -0.3)), b], rng.uniform(0.03, 0.08), n=6, rng=rng, wobble=0.25)
        # broken conduit, glowing
        cyl(bm, (-0.9, -6.0, 1.2), (-0.9, 0.2, 1.2), 0.18, n=10)
        cyl(bm, (-0.9, -3.2, 1.2), (-0.9, -2.6, 1.2), 0.21, n=10, mat=WG)
    st.part(mid_inner, group="Wreck_Mid", bevel=0, angle=40)

    # stern: engine block with one intact nozzle and one crushed
    STERN = [(-12.5, 2.6, 1.9, 0.1), (-9.0, 2.8, 1.9, 0.1), (-7.4, 2.7, 1.85, 0.1)]
    st.part(shell(STERN), group="Wreck_Stern", bevel=0, angle=35,
            post=torn([(Mcut(-8.0, flip=True, tilt=(15, -6)), 9, 7, 1.3, 0.45)], holes=[((-2.4, -10.2, 0.8), 1.0, 0.5)]))

    def stern_parts(bm):
        box(bm, (0, -12.7, 0.1), (5.0, 0.6, 3.6))
        nozzle(bm, (1.3, -14.2, 0.1), 0.95, 1.4, n=20, rimmat=WG, coremat=0, hullmat=0)
        n2 = bmesh.new()
        nozzle(n2, (-1.3, -14.2, 0.1), 0.95, 1.4, n=20, rimmat=0, coremat=0, hullmat=0)
        for v in n2.verts:
            v.co.z -= 0.25 * max(0.0, (-13.0 - v.co.y)) * (1 + 0.4 * math.sin(v.co.x * 3))
            v.co.x += 0.15 * math.sin(v.co.z * 4)
        me = bpy.data.meshes.new("tmp")
        n2.to_mesh(me)
        n2.free()
        bm.from_mesh(me)
        bpy.data.meshes.remove(me)
        ribs(bm, STERN, [-8.2, -7.2, -6.6])
        for k in range(4):
            a = V((rng.uniform(-1.8, 1.8), -7.6, rng.uniform(-1.0, 1.2)))
            b = a + V((rng.uniform(-1, 1), rng.uniform(1.5, 3.0), rng.uniform(-1.6, 0.2)))
            cable(bm, [a, a.lerp(b, 0.5) + V((0, 0, -0.3)), b], 0.05, n=6, rng=rng, wobble=0.2)
    st.part(stern_parts, group="Wreck_Stern", bevel=0, angle=45, weighted=False)

    # torn wing: jagged root edge with an ember band along the tear
    def wing_geo(bm):
        pts = [(1.2, -2.3), (6.5, -1.0), (7.5, 1.0), (0.7, 2.6)]
        pts = [pts[0], pts[1], pts[2]] + jag([pts[3], pts[0]], rng, depth=0.55, seg=0.32, prob=0.95, closed=False, inward=True)
        slab(bm, pts, 0.3, origin=(0, 0, 0), u=(1, 0, 0), v=(0, 1, 0), bevel_top=0.06)
        root_pts = pts[3:]
        band = root_pts + [(x + 0.18, y) for x, y in reversed(root_pts)]
        slab(bm, band, 0.34, origin=(0, 0, 0), u=(1, 0, 0), v=(0, 1, 0), mat=WG)
        for q in range(3):
            prism(bm, (1.6 + q * 1.9, -1.6 + q * 0.4, 0.17), (1.6 + q * 1.9 + 1.6, -1.6 + q * 0.4 + 0.3, 0.17), 0.08, 0.05)
    st.part(wing_geo, group="Wreck_Wing", bevel=0, angle=35)

    # loose debris plates / beams
    def debris(bm):
        for k in range(16):
            c = V((rng.gauss(0, 6.0), rng.gauss(0, 6.0), rng.gauss(0, 3.0)))
            R = Matrix.Rotation(rng.uniform(0, 6.28), 3, V((rng.gauss(0, 1), rng.gauss(0, 1), rng.gauss(0, 1))).normalized())
            u = R @ V((1, 0, 0))
            vv = R @ V((0, 1, 0))
            s = rng.uniform(0.4, 1.4)
            outline = [(s * math.cos(a) * rng.uniform(0.5, 1.2), s * math.sin(a) * rng.uniform(0.5, 1.2))
                       for a in [2 * math.pi * q / 7 for q in range(7)]]
            slab(bm, outline, 0.12, origin=c, u=u, v=vv)
            if rng.random() < 0.4:
                prism(bm, c, c + R @ V((0, rng.uniform(1.5, 3.0), 0.2)), 0.08, 0.12)
            if rng.random() < 0.3:
                sphere(bm, c + R @ V((s * 0.6, 0, 0.08)), 0.06, mat=WG, seg=6, rings=3)
    st.part(debris, group="Wreck_Debris", bevel=0, angle=40)

    objs = st.finalize(center=False)
    # scatter the pieces into a ~25 m debris field
    place = {"Wreck_Bow": ((4.5, 9.0, 1.5), (14, -8, 25)), "Wreck_Mid": ((0.0, 0.0, 0.0), (-6, 4, -10)),
             "Wreck_Stern": ((-3.5, -4.5, -1.5), (-8, -15, 18)), "Wreck_Wing": ((7.5, -3.0, 2.5), (40, 20, 10)),
             "Wreck_Debris": ((0.0, 1.0, 0.0), (0, 0, 0))}
    for n_, (loc, rot) in place.items():
        o = objs[n_]
        o.location = loc
        o.rotation_euler = [math.radians(a) for a in rot]
    meshes = list(objs.values())
    out = os.path.join(MODELS, "wreck.glb")
    if OPTS['bake']:
        S.unwrap_hull(meshes)
        paths = P.bake(meshes, mats["Hull"], socks, BUILD, "wreck", size=OPTS['tex'], samples=24)
        S.rebuild_hull_textured(mats["Hull"], paths)
        S.export_glb(out, meshes, [])
    report("wreck", meshes, out)
    if OPTS['preview']:
        preview_scene()
        P.preview_set(meshes, "wreck", PREVIEWS, VIEWS, (1.0, 0.9, 0.6),
                      [((-1.0, 0.8, 0.4), False), ((-0.6, -1.0, 0.5), False), ((1.0, -0.3, -0.5), False), ((0, 0.0001, 1), True)])


# ================================================================ E. ancient gate + ruin

def glyph(bm, o, u, v, n, cell, rng, mat, th=0.4, raise_=0.1):
    """Alien glyph: 2-4 strokes between nodes of a 3x3 lattice, plus optional ring/dot."""
    o, u, v, n = V(o), V(u).normalized(), V(v).normalized(), V(n).normalized()
    nodes = [(i, j) for i in range(3) for j in range(3)]
    R = Matrix((u, v, n)).transposed()

    def pt(ij):
        return o + u * ((ij[0] - 1) * cell * 0.38) + v * ((ij[1] - 1) * cell * 0.38)
    for _ in range(rng.randint(2, 4)):
        a, b = rng.sample(nodes, 2)
        pa, pb = pt(a), pt(b)
        d = pb - pa
        L_ = d.length
        if L_ < 1e-3:
            continue
        ang = math.atan2(d.dot(v), d.dot(u))
        Rs = R @ Matrix.Rotation(ang, 3, 'Z')
        box(bm, (pa + pb) / 2 + n * raise_, (L_ + cell * 0.09, cell * 0.09, th), mat=mat, rot=Rs)
    if rng.random() < 0.45:
        c = pt(rng.choice(nodes))
        rr = cell * 0.16
        for k in range(8):
            a0 = 2 * math.pi * k / 8
            p = c + (u * math.cos(a0) + v * math.sin(a0)) * rr
            Rs = R @ Matrix.Rotation(a0 + math.pi / 2, 3, 'Z')
            box(bm, p + n * raise_, (rr * 0.85, cell * 0.07, th), mat=mat, rot=Rs)


def rough_block(bm, c, size, rng, R=None, jit=0.12, mat=0, sub=1):
    """Chipped stone block: subdivided box with jittered verts."""
    R = R if R is not None else Matrix.Identity(3)
    tb = bmesh.new()
    bmesh.ops.create_cube(tb, size=1.0)
    if sub:
        bmesh.ops.subdivide_edges(tb, edges=list(tb.edges), cuts=sub, use_grid_fill=True)
    for v in tb.verts:
        p = V((v.co.x * size[0], v.co.y * size[1], v.co.z * size[2]))
        p += V((rng.uniform(-1, 1), rng.uniform(-1, 1), rng.uniform(-1, 1))) * jit * min(size)
        v.co = R @ p + V(c)
    for f in tb.faces:
        f.material_index = mat
    me = bpy.data.meshes.new("_tmpblock")
    tb.to_mesh(me)
    tb.free()
    bm.from_mesh(me)
    bpy.data.meshes.remove(me)


def build_gate():
    S.reset_scene()
    stone, socks = P.make_stone_src("Stone", k=7.0, col_a=(0.2, 0.195, 0.185), col_b=(0.11, 0.105, 0.1),
                                    patina=(0.05, 0.11, 0.1), bump=1.2)
    mats = {"Stone": stone, "Glow": flat_mat("Glow", (0, 0, 0), 0.0, 0.5, emit=(1, 1, 1), strength=6.0)}
    order = ["Stone", "Glow"]
    SG = 1
    gp = Prop("gate", mats, order)
    rng = random.Random(303)
    NSEGS = 12
    span = 2 * math.pi / NSEGS
    gap = math.radians(3.2)
    # cross-section in (r, y): y = hole axis
    prof = [(330.0, -22.0), (336.0, -30.0), (350.0, -30.0), (352.0, -36.0), (392.0, -36.0), (402.0, -26.0),
            (402.0, 26.0), (392.0, 36.0), (352.0, 36.0), (350.0, 30.0), (336.0, 30.0), (330.0, 22.0)]

    def P3(r, y, a):
        # ring in Blender XZ plane, angle a from +Z
        return V((r * math.sin(a), y, r * math.cos(a)))

    def seg_geo(bm):
        rings = []
        N = 28
        for i in range(N + 1):
            a = -span / 2 + gap / 2 + (span - gap) * i / N
            rings.append([P3(r, y, a) for r, y in prof])
        return loft(bm, rings)

    def segment(bm):
        r = seg_geo(bm)
        bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
        # carved channel bands on both faces + inner face
        for k in (3, 7, 11):
            fl = [band[k] for band in r['bands'][1:-1]]
            bmesh.ops.inset_region(bm, faces=fl, thickness=4.0 if k != 11 else 3.0, depth=-1.8, use_even_offset=True)
        fl = [band[5] for band in r['bands'][2:-2:3]]
        S.inset_faces(bm, fl, thickness=1.5, depth=-1.0)
        # glyph rows in the channels
        for ys, nrm_sign in ((36.0 - 1.8, 1), (-36.0 + 1.8, -1)):
            nglyph = 14
            for g in range(nglyph):
                a = -span / 2 + gap / 2 + (span - gap) * (g + 0.5) / nglyph
                o = P3(372.0, ys, a)
                u = V((math.cos(a), 0, -math.sin(a)))
                vv = V((math.sin(a), 0, math.cos(a)))
                glyph(bm, o, u, vv, V((0, nrm_sign, 0)), 15.0, rng, SG, th=0.6, raise_=0.1)
        for g in range(16):
            a = -span / 2 + gap / 2 + (span - gap) * (g + 0.5) / 16
            o = P3(331.8, 0.0, a)
            u = V((math.cos(a), 0, -math.sin(a)))
            glyph(bm, o, u, V((0, 1, 0)), -V((math.sin(a), 0, math.cos(a))), 10.0, rng, SG, th=0.6, raise_=0.15)
        # buttress ribs on the outer face
        for q in range(5):
            a = -span / 2 + gap / 2 + (span - gap) * (q + 0.5) / 5
            c = P3(404.0, 0.0, a)
            R = Matrix.Rotation(a, 3, 'Y')
            box(bm, c, (5.0, 60.0, 6.0), rot=R, taper=(0.8, 0.7))
            box(bm, P3(407.2, 0.0, a), (1.2, 40.0, 0.6), mat=SG, rot=R)
    gp.part(segment, group="Gate_Seg_0", bevel=0.6, angle=30, segs=2)

    def keystone(bm):
        # monolithic wedge spanning the gap with an energy slit
        a = 0.0
        outline = [(-12.0, 318.0), (12.0, 318.0), (15.0, 414.0), (0.0, 422.0), (-15.0, 414.0)]
        slab(bm, outline, 84.0, origin=(0, 0, 0), u=(1, 0, 0), v=(0, 0, 1), bevel_top=4.0)
        for y in (42.0, -42.0):
            box(bm, (0, y, 368.0), (2.2, 0.8, 80.0), mat=SG)
            for q in range(5):
                box(bm, (0, y, 330.0 + q * 18.0), (14.0, 0.8, 1.0), mat=SG)
        box(bm, (0, 0, 317.5), (3.0, 60.0, 1.0), mat=SG)
    gp.part(keystone, group="Gate_Key_0", bevel=0.8, angle=30, segs=2)

    def pylon(bm):
        # great spire keystone (cardinal)
        outline = [(-16.0, 316.0), (16.0, 316.0), (20.0, 420.0), (9.0, 470.0), (0.0, 520.0), (-9.0, 470.0), (-20.0, 420.0)]
        slab(bm, outline, 96.0, origin=(0, 0, 0), u=(1, 0, 0), v=(0, 0, 1), bevel_top=6.0)
        for sx in (1, -1):
            slab(bm, [(0.0, 400.0), (34.0, 440.0), (40.0, 470.0), (14.0, 450.0)][::sx] if sx > 0 else
                 [(14.0 * -1, 450.0), (-40.0, 470.0), (-34.0, 440.0), (0.0, 400.0)], 30.0, origin=(0, 0, 0), u=(1, 0, 0), v=(0, 0, 1),
                 bevel_top=3.0)
        for y in (48.5, -48.5):
            box(bm, (0, y, 420.0), (3.0, 0.8, 180.0), mat=SG)
            for q in range(8):
                glyph(bm, (0, y, 340.0 + q * 20.0), (1, 0, 0), (0, 0, 1), (0, 1 if y > 0 else -1, 0), 16.0, rng, SG, th=0.6, raise_=0.1)
        sphere(bm, (0, 0, 512.0), 6.0, mat=SG, seg=16, rings=8, scale=(1, 1, 1.6))
    gp.part(pylon, group="Gate_Pylon_0", bevel=0.8, angle=30, segs=2)

    objs = gp.finalize(center=False)
    seg0, key0, pyl0 = objs["Gate_Seg_0"], objs["Gate_Key_0"], objs["Gate_Pylon_0"]
    # gap i sits at angle -span/2 - span*i (rotation about Blender Y); pylons on every third gap
    gap_rot = lambda i: (0, -span / 2 - span * i, 0)
    pyl0.rotation_euler = gap_rot(0)
    key0.rotation_euler = gap_rot(1)
    meshes = [seg0, key0, pyl0]
    out = os.path.join(MODELS, "ancient_gate.glb")
    if OPTS['bake']:
        P.unwrap(meshes, "Stone", angle=55, margin=0.002)
        paths = P.bake(meshes, stone, socks, BUILD, "gate", size=OPTS.get('gate_tex', 2048), samples=16)
        S.rebuild_hull_textured(stone, paths)
    # linked duplicates (shared mesh data -> shared glTF meshes) around the ring
    coll = bpy.context.scene.collection
    allo = [seg0, key0, pyl0]
    for i in range(1, NSEGS):
        o = bpy.data.objects.new("Gate_Seg_%d" % i, seg0.data)
        o.rotation_euler = (0, -span * i, 0)
        coll.objects.link(o)
        allo.append(o)
    nk, npy = 1, 1
    for i in range(2, NSEGS):
        if i % 3 == 0:
            o = bpy.data.objects.new("Gate_Pylon_%d" % npy, pyl0.data)
            npy += 1
        else:
            o = bpy.data.objects.new("Gate_Key_%d" % nk, key0.data)
            nk += 1
        o.rotation_euler = gap_rot(i)
        coll.objects.link(o)
        allo.append(o)
    if OPTS['bake']:
        S.export_glb(out, allo, [])
    report("ancient_gate", allo, out)
    if OPTS['preview']:
        mats["Glow"].node_tree.nodes["Principled BSDF"].inputs['Emission Color'].default_value = (0.25, 0.85, 1.0, 1)
        cam = preview_scene(key=4.0, rim=2.0)
        P.preview_set(allo, "gate", PREVIEWS, VIEWS, (0.55, 1.0, 0.35),
                      [((0.0, 1.0, 0.0), True), ((1.0, 0.25, 0.2), False), ((-0.4, -1.0, -0.3), False), ((0.0, 0.0001, 1.0), True)])
        cam.location = V((260.0, 240.0, 330.0))
        cam.rotation_euler = (V((0, 0, 395.0)) - cam.location).to_track_quat('-Z', 'Y').to_euler()
        cam.data.type = 'PERSP'
        cam.data.lens = 35
        cam.data.clip_end = 5000
        S.render(os.path.join(PREVIEWS, "props_gate_closeup.png"), samples=96)


def build_ruin():
    S.reset_scene()
    stone, socks = P.make_stone_src("Stone", k=1.6, col_a=(0.21, 0.2, 0.19), col_b=(0.115, 0.11, 0.105),
                                    patina=(0.05, 0.11, 0.1), bump=1.2)
    mats = {"Stone": stone, "Glow": flat_mat("Glow", (0, 0, 0), 0.0, 0.5, emit=(1, 1, 1), strength=6.0)}
    order = ["Stone", "Glow"]
    SG = 1
    rp = Prop("ruin", mats, order)
    rng = random.Random(404)

    # broken floating platform (three drifting shards)
    def platform(bm):
        Rr = 30.0
        cuts = [0.3, 2.4, 4.3, 0.3 + 2 * math.pi]
        for s in range(3):
            a0, a1 = cuts[s] + 0.03, cuts[s + 1] - 0.03
            outline = [(0.0, 0.0)]
            nn = 9
            for q in range(nn + 1):
                a = a0 + (a1 - a0) * q / nn
                rr = Rr * (0.88 + 0.12 * math.cos(4 * a) ** 2) * rng.uniform(0.93, 1.0)
                outline.append((rr * math.cos(a), rr * math.sin(a)))
            outline = jag(outline, rng, depth=1.2, seg=2.6, prob=0.6, inward=True)
            mid = (a0 + a1) / 2
            drift = V((math.cos(mid), math.sin(mid), 0)) * 2.5 + V((0, 0, rng.uniform(-2.5, 1.5)))
            tilt = Matrix.Rotation(math.radians(rng.uniform(-5, 5)), 3, V((-math.sin(mid), math.cos(mid), 0)))
            u = tilt @ V((1, 0, 0))
            vv = tilt @ V((0, 1, 0))
            slab(bm, outline, 5.0, origin=V((0, 0, -22.0)) + drift, u=u, v=vv, bevel_top=0.8)
            # carved glyph ring on top
            for q in range(6):
                a = a0 + (a1 - a0) * (q + 0.5) / 6
                o = V((0, 0, -22.0)) + drift + tilt @ V((22.0 * math.cos(a), 22.0 * math.sin(a), 2.5))
                glyph(bm, o, tilt @ V((-math.sin(a), math.cos(a), 0)), tilt @ V((math.cos(a), math.sin(a), 0)), tilt @ V((0, 0, 1)),
                      4.0, rng, SG, th=0.25, raise_=0.02)
            # stepped dais pieces
        # central dais + altar orb
        for k, (r, h) in enumerate(((9.0, 1.6), (6.5, 1.6), (4.0, 1.4))):
            lathe(bm, (0, 0, -19.5 + k * 1.6), (0, 0, 1), [(r, 0.0), (r, h), (0.0, h)], n=8)
        sphere(bm, (0, 0, -9.0), 2.2, mat=SG, seg=16, rings=8)
        for k in range(4):
            a = math.pi / 4 + k * math.pi / 2
            c = V((5.0 * math.cos(a), 5.0 * math.sin(a), -12.0))
            P.spike(bm, c - V((0, 0, 3.0)), (-(c.x) * 0.15, -(c.y) * 0.15, 1), 8.0, 0.6, n=4)
    rp.part(platform, group="Ruin_Platform", bevel=0.15, angle=30)

    # monoliths (two snapped, tops drifting nearby)
    def monoliths(bm):
        spots = [(-22.0, 14.0, 34.0, 0.0), (18.0, 20.0, 40.0, 5.0), (26.0, -10.0, 28.0, -8.0), (-12.0, -26.0, 36.0, 12.0),
                 (-34.0, -8.0, 22.0, -16.0), (6.0, 33.0, 26.0, 9.0)]
        for i, (x, y, h, tilt) in enumerate(spots):
            w, t = rng.uniform(5.0, 7.0), rng.uniform(2.4, 3.2)
            broken = i in (2, 4)
            hh = h * (0.55 if broken else 1.0)
            base = V((x, y, -19.5))
            R = Matrix.Rotation(math.atan2(y, x) + math.pi / 2, 3, 'Z') @ Matrix.Rotation(math.radians(tilt), 3, 'X')
            u, up, nn = R @ V((1, 0, 0)), R @ V((0, 0, 1)), R @ V((0, 1, 0))
            if broken:
                top = [(-w / 2, 0.0), (w / 2, 0.0), (w / 2, hh * 0.9), (w * 0.2, hh * 1.05), (-w * 0.1, hh * 0.86), (-w / 2, hh * 1.1)]
            else:
                top = [(-w / 2, 0.0), (w / 2, 0.0), (w / 2, hh * 0.9), (0.0, hh), (-w / 2, hh * 0.9)]
            slab(bm, top, t, origin=base, u=u, v=up, bevel_top=0.4)
            # glyph column on both faces
            for sgn in (1, -1):
                for q in range(int(hh * 0.8 / 4.2)):
                    o = base + up * (3.0 + q * 4.2) + nn * sgn * (t / 2)
                    glyph(bm, o, u * sgn, up, nn * sgn, 3.6, rng, SG, th=0.18, raise_=0.03)
                box(bm, base + up * (hh * 0.45) + nn * sgn * (t / 2 + 0.02), (0.25, 0.1, hh * 0.85), mat=SG, rot=Matrix((u, nn, up)).transposed())
            if broken:
                c = base + up * (h * 0.9) + u * rng.uniform(-3, 3) + V((0, 0, 4.0))
                R2 = R @ Matrix.Rotation(math.radians(rng.uniform(20, 50)), 3, V((rng.gauss(0, 1), rng.gauss(0, 1), rng.gauss(0, 1))).normalized())
                slab(bm, [(-w / 2, 0.0), (w * 0.2, -0.6), (w / 2, 0.4), (w / 2, h * 0.3), (0.0, h * 0.36), (-w / 2, h * 0.3)], t,
                     origin=c, u=R2 @ V((1, 0, 0)), v=R2 @ V((0, 0, 1)), bevel_top=0.4)
    rp.part(monoliths, group="Ruin_Monoliths", bevel=0.12, angle=30)

    # arch with a missing voussoir
    def arch(bm):
        cx, cy = 0.0, -6.0
        R = Matrix.Rotation(math.radians(20), 3, 'Z')
        Rr, z0 = 13.0, -6.0
        for sx in (1, -1):
            box(bm, R @ V((sx * Rr, 0, -12.5)) + V((cx, cy, 0)), (4.6, 4.6, 14.0), rot=R)
            box(bm, R @ V((sx * Rr, 0, -19.0)) + V((cx, cy, 0)), (6.0, 6.0, 1.6), rot=R)
        nvs = 9
        for k in range(nvs):
            a0 = math.pi * k / nvs
            a1 = math.pi * (k + 1) / nvs
            if k == 5:
                drift = V((2.0, -3.5, 6.0))
                rot_extra = Matrix.Rotation(0.6, 3, 'X')
            else:
                drift = V((0, 0, 0))
                rot_extra = Matrix.Identity(3)
            ri, ro = Rr - 2.3, Rr + 2.6
            pts = [(ri * math.cos(a0 + 0.012), ri * math.sin(a0 + 0.012)), (ro * math.cos(a0 + 0.008), ro * math.sin(a0 + 0.008)),
                   (ro * math.cos(a1 - 0.008), ro * math.sin(a1 - 0.008)), (ri * math.cos(a1 - 0.012), ri * math.sin(a1 - 0.012))]
            mid = V(((ri + ro) / 2 * math.cos((a0 + a1) / 2), 0, (ri + ro) / 2 * math.sin((a0 + a1) / 2)))
            pts2 = [(p[0] - mid.x, p[1] - mid.z) for p in pts]
            origin = R @ (mid + V((0, 0, z0))) + V((cx, cy, 0)) + drift
            slab(bm, pts2, 4.4, origin=origin, u=R @ rot_extra @ V((1, 0, 0)), v=R @ rot_extra @ V((0, 0, 1)), bevel_top=0.3)
            a = (a0 + a1) / 2
            o = origin + R @ rot_extra @ V((0, 2.22, 0))
            glyph(bm, o, R @ rot_extra @ V((-math.sin(a), 0, math.cos(a))), R @ rot_extra @ V((math.cos(a), 0, math.sin(a))),
                  R @ rot_extra @ V((0, 1, 0)), 3.2, rng, SG, th=0.15, raise_=0.03)
        sphere(bm, R @ V((0, 0, -9.0)) + V((cx, cy, 0)), 1.2, mat=SG, seg=12, rings=6)
    rp.part(arch, group="Ruin_Arch", bevel=0.1, angle=30)

    # floating fragments
    def fragments(bm):
        for k in range(26):
            a = rng.uniform(0, 2 * math.pi)
            r = rng.uniform(14.0, 58.0)
            c = V((r * math.cos(a), r * math.sin(a), rng.uniform(-30.0, 26.0)))
            s = rng.uniform(1.0, 4.5)
            R = Matrix.Rotation(rng.uniform(0, 6.28), 3, V((rng.gauss(0, 1), rng.gauss(0, 1), rng.gauss(0, 1))).normalized())
            size = (s * rng.uniform(0.6, 1.4), s * rng.uniform(0.6, 1.4), s * rng.uniform(0.4, 1.0))
            rough_block(bm, c, size, rng, R=R, jit=0.12)
            if rng.random() < 0.4:
                n = R @ V((0, 0, 1))
                glyph(bm, c + n * size[2] * 0.5, R @ V((1, 0, 0)), R @ V((0, 1, 0)), n, s * 0.7, rng, SG, th=0.1, raise_=0.03)
    rp.part(fragments, group="Ruin_Fragments", bevel=0.08, angle=35)

    objs = rp.finalize(center=False)
    meshes = list(objs.values())
    out = os.path.join(MODELS, "ruin.glb")
    if OPTS['bake']:
        P.unwrap(meshes, "Stone", angle=55, margin=0.003)
        paths = P.bake(meshes, stone, socks, BUILD, "ruin", size=OPTS['tex'], samples=16)
        S.rebuild_hull_textured(stone, paths)
        S.export_glb(out, meshes, [])
    report("ruin", meshes, out)
    if OPTS['preview']:
        mats["Glow"].node_tree.nodes["Principled BSDF"].inputs['Emission Color'].default_value = (0.25, 0.85, 1.0, 1)
        preview_scene(key=4.0, rim=2.0)
        P.preview_set(meshes, "ruin", PREVIEWS, VIEWS, (1.0, -0.9, 0.45),
                      [((-1.0, 0.6, 0.3), False), ((0.2, 1.0, 0.8), False), ((1.0, 0.0, -0.4), False), ((0, 0.0001, 1), True)])


# ================================================================ registry

BUILDERS = {
    'asteroids': build_asteroids,
    'raider': build_raider_run,
    'gunship': build_gunship_run,
    'warden': build_warden_run,
    'warlord': build_warlord_run,
    'sentinel': build_sentinel_run,
    'station': build_station,
    'wreck': build_wreck,
    'gate': build_gate,
    'ruin': build_ruin,
}


def main():
    argv = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
    OPTS['bake'] = "--no-bake" not in argv
    OPTS['preview'] = "--no-preview" not in argv
    if "--tex" in argv:
        OPTS['tex'] = int(argv[argv.index("--tex") + 1])
    names = [a for a in argv if a in BUILDERS] or list(BUILDERS)
    os.makedirs(BUILD, exist_ok=True)
    for n in names:
        t = time.time()
        BUILDERS[n]()
        log(n, "done in %.1fs" % (time.time() - t))
    for k, (n, per, mb) in REPORT.items():
        log("SUMMARY", k, "tris", n, "%.2f MB" % mb, per)


if __name__ == "__main__":
    main()

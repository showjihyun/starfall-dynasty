"""v3 (2026-10-09) Cradle Exchange as a classical fortress-cathedral (LoGH design language, original design).

Data: data/market/cradle-exchange.json radius_m 120 -> bounding radius <= 120 m (asserted).
Up = Blender +Z (Unity +Y). Static: no rotating ring in v3 (a cathedral does not spin).
Parts: colonnaded drum + gold-ribbed dome + lantern spire, four campanile towers with flying
buttresses, an inverted keel spire below, four arcaded docking bridges ending in pavilions.

Run:  blender --background --factory-startup --python art/scripts/station_v3.py
Out:  art/export/v3/station_cradle_exchange_v3.fbx (LOD0/LOD1, Socket_Dock_0..3, Socket_Beacon_*),
      art/source/station_v3.blend, art/previews/v3_station_*.png
"""
import math
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import bpy
import bmesh
from mathutils import Vector, Matrix

import common as C
import logh_ships as LS

RADIUS_M = 120.0
I = LS.I


def ring_z(r, z, n, x0=0.0, y0=0.0):
    return [Vector((x0 + r * math.cos(2 * math.pi * i / n), y0 + r * math.sin(2 * math.pi * i / n), z)) for i in range(n)]


def lathe(profile, n, mat, x0=0.0, y0=0.0, cap=True):
    """Surface of revolution about a vertical axis: profile = [(r, z), ...] bottom->top."""
    return LS.loft([ring_z(max(r, 0.01), z, n, x0, y0) for r, z in profile], mat, cap=cap)


def tube(bm, pts, r, mat, segs=8):
    """Polyline tube (for buttresses / ribs)."""
    for a, b in zip(pts, pts[1:]):
        d = b - a
        L_ = d.length
        z = d.normalized()
        up = Vector((0, 0, 1)) if abs(z.z) < 0.9 else Vector((1, 0, 0))
        x = up.cross(z).normalized()
        y = z.cross(x)
        m = Matrix((x, y, z)).transposed().to_4x4()
        m.translation = (a + b) / 2
        vs = bmesh.ops.create_cone(bm, cap_ends=True, segments=segs, radius1=r, radius2=r, depth=L_ + r)["verts"]
        bmesh.ops.transform(bm, matrix=m, verts=vs)
        for f in {f for v in vs for f in v.link_faces}:
            f.material_index = I(mat)


def split_material(bm, mat):
    """Move every face using `mat` into a new bmesh (so the thousands of window boxes skip the bevel)."""
    idx = I(mat)
    faces = [f for f in bm.faces if f.material_index == idx]
    out = bmesh.new()
    vmap = {}
    for f in faces:
        vs = []
        for v in f.verts:
            if v not in vmap:
                vmap[v] = out.verts.new(v.co)
            vs.append(vmap[v])
        nf = out.faces.new(vs)
        nf.material_index = idx
    bmesh.ops.delete(bm, geom=faces, context="FACES_ONLY")
    loose = [v for v in bm.verts if not v.link_faces]
    bmesh.ops.delete(bm, geom=loose, context="VERTS")
    return out


def bezier(p0, p1, p2, steps=10):
    return [p0 * (1 - t) ** 2 + p1 * 2 * t * (1 - t) + p2 * t * t for t in (i / steps for i in range(steps + 1))]


def build(M, coll):
    bm = bmesh.new()
    smooth = bmesh.new()
    N = 48
    # --- fortress base: three stepped octagonal bastions with battlements, then a short heavy keel
    tiers = ((44, -20, -34), (36, -34, -50), (27, -50, -66))
    for r, z1, z0 in tiers:
        LS.merge(bm, lathe([(r * 0.92, z0), (r, z0 + 2), (r, z1 - 1.5), (r + 1.2, z1 - 1), (r + 1.2, z1)], 8, "white"))
        LS.merge(bm, lathe([(r + 1.3, z1 - 0.9), (r + 1.6, z1 - 0.5), (r + 1.3, z1 - 0.1)], 8, "gold", cap=False))
        # battlements around the rim of each bastion
        nm = int(2 * math.pi * r / 4.5)
        for m in range(nm):
            a = 2 * math.pi * m / nm
            vs = LS.box(bm, (r + 0.4, 0, z1 + 1.2), (2.2, 2.2, 2.4), "white")
            bmesh.ops.rotate(bm, verts=vs, cent=(0, 0, 0), matrix=Matrix.Rotation(a, 3, "Z"))
        # rows of small lit windows (gives the scale away: each is ~1 x 1.6 m)
        for zz in (z0 + 4, z0 + 8, z1 - 5):
            nw = int(2 * math.pi * r / 3.2)
            for m in range(nw):
                if m % 5 == 4:
                    continue
                a = 2 * math.pi * (m + 0.5) / nw
                vs = LS.box(bm, (r * 0.985, 0, zz), (0.4, 1.0, 1.6), "window")
                bmesh.ops.rotate(bm, verts=vs, cent=(0, 0, 0), matrix=Matrix.Rotation(a, 3, "Z"))
    LS.merge(smooth, lathe([(0.8, -106), (5, -98), (11, -86), (17, -74), (22, -66)], N, "white"))
    for z, r in ((-96, 6.0), (-84, 12.6), (-72, 18.6)):
        LS.merge(smooth, lathe([(r, z - 0.8), (r + 0.5, z), (r, z + 0.8)], N, "gold", cap=False))
    # buttress ribs running down the bastions to the keel
    for m in range(8):
        a = 2 * math.pi * (m + 0.5) / 8
        dv = Vector((math.cos(a), math.sin(a), 0))
        pts = [dv * 47 + Vector((0, 0, -20)), dv * 39 + Vector((0, 0, -36)), dv * 30 + Vector((0, 0, -52)),
               dv * 21 + Vector((0, 0, -68)), dv * 9 + Vector((0, 0, -90))]
        tube(bm, pts, 1.6, "white", segs=6)
    # --- drum
    LS.merge(smooth, lathe([(34, -20), (35, -19), (35, 9), (36, 10), (52, 10), (52, 13), (34, 13)], N, "white"))
    LS.merge(smooth, lathe([(35.3, -12.5), (35.6, -12), (35.3, -11.5)], N, "gold", cap=False))
    LS.merge(smooth, lathe([(52.3, 10.5), (52.6, 11.5), (52.3, 12.5)], N, "gold", cap=False))
    # many small windows in decks on the drum (scale cue)
    for zz in (-17, -13.5):
        for m in range(70):
            a = 2 * math.pi * (m + 0.5) / 70
            vs = LS.box(bm, (35.12, 0, zz), (0.3, 1.0, 1.4), "window")
            bmesh.ops.rotate(bm, verts=vs, cent=(0, 0, 0), matrix=Matrix.Rotation(a, 3, "Z"))
    # tall arched windows around the drum
    for k in range(32):
        a = 2 * math.pi * (k + 0.5) / 32
        rot = Matrix.Rotation(a, 3, "Z")
        vs = LS.box(bm, (35.15, 0, -4), (0.4, 2.4, 13), "window")
        bmesh.ops.rotate(bm, verts=vs, cent=(0, 0, 0), matrix=rot)
    # --- colonnade on the platform
    for k in range(28):
        a = 2 * math.pi * k / 28
        c = Vector((48 * math.cos(a), 48 * math.sin(a), 0))
        LS.merge(smooth, lathe([(2.2, 13), (2.2, 14.2), (1.5, 15), (1.35, 27), (2.0, 28), (2.4, 29)], 12, "white",
                               x0=c.x, y0=c.y))
    LS.merge(smooth, lathe([(46, 29), (51, 29), (51.5, 30.5), (51, 32), (36, 32)], N, "white", cap=False))
    LS.merge(smooth, lathe([(51.6, 30.2), (52.0, 30.8), (51.6, 31.4)], N, "gold", cap=False))
    # inner wall behind the colonnade (lit windows)
    LS.merge(smooth, lathe([(36, 13), (36, 32)], N, "white", cap=False))
    for k in range(28):
        a = 2 * math.pi * (k + 0.5) / 28
        rot = Matrix.Rotation(a, 3, "Z")
        vs = LS.box(bm, (36.15, 0, 22), (0.3, 3.0, 11), "window")
        bmesh.ops.rotate(bm, verts=vs, cent=(0, 0, 0), matrix=rot)
    # --- dome + gold ribs + lantern + spire
    prof = []
    for i in range(17):
        th = math.pi / 2 * i / 16
        prof.append((36 * math.cos(th), 32 + 34 * math.sin(th)))
    LS.merge(smooth, lathe(prof, 64, "white"))
    for k in range(12):
        a = 2 * math.pi * k / 12
        pts = [Vector((36.4 * math.cos(th) * math.cos(a), 36.4 * math.cos(th) * math.sin(a), 32 + 34.4 * math.sin(th)))
               for th in (math.pi / 2 * i / 14 for i in range(14))]
        tube(bm, pts, 0.7, "gold")
    LS.merge(smooth, lathe([(7, 64), (7, 76), (8.5, 77), (8.5, 78.5), (5, 79)], 24, "white"))
    for k in range(8):
        a = 2 * math.pi * k / 8
        vs = LS.box(bm, (7.1, 0, 70), (0.3, 1.6, 8), "window")
        bmesh.ops.rotate(bm, verts=vs, cent=(0, 0, 0), matrix=Matrix.Rotation(a, 3, "Z"))
    LS.merge(smooth, lathe([(5, 79), (3, 92), (1.2, 104), (0.05, 110)], 16, "gold"))
    LS.box(bm, (0, 0, 98), (1.4, 1.4, 1.4), "window")
    # --- four campaniles with spires and flying buttresses
    socks = {}
    for k in range(4):
        a = math.pi / 4 + k * math.pi / 2
        c = Vector((64 * math.cos(a), 64 * math.sin(a), 0))
        rot = Matrix.Rotation(a, 4, "Z")
        tw = bmesh.new()
        LS.box(tw, (0, 0, 10), (11, 11, 60), "white")
        LS.box(tw, (0, 0, 40.5), (12.5, 12.5, 1.2), "gold")
        LS.box(tw, (0, 0, -20.5), (12.5, 12.5, 1.2), "gold")
        for f_ in range(4):
            r4 = Matrix.Rotation(f_ * math.pi / 2, 3, "Z")
            for zz in (8, 28):
                vs = LS.box(tw, (5.6, 0, zz), (0.3, 2.2, 9), "window")
                bmesh.ops.rotate(tw, verts=vs, cent=(0, 0, 0), matrix=r4)
            for zz in range(-16, 40, 4):  # small floor windows between the tall ones
                for yy in (-3.6, 3.6):
                    vs = LS.box(tw, (5.56, yy, zz), (0.3, 0.9, 1.3), "window")
                    bmesh.ops.rotate(tw, verts=vs, cent=(0, 0, 0), matrix=r4)
        pyr = bmesh.ops.create_cone(tw, cap_ends=True, segments=4, radius1=8.2, radius2=0.1, depth=26)["verts"]
        bmesh.ops.rotate(tw, verts=pyr, cent=(0, 0, 0), matrix=Matrix.Rotation(math.pi / 4, 3, "Z"))
        bmesh.ops.translate(tw, verts=pyr, vec=(0, 0, 54))
        for f in {f for v in pyr for f in v.link_faces}:
            f.material_index = I("white")
        LS.box(tw, (0, 0, 67.6), (0.9, 0.9, 1.4), "gold")
        bmesh.ops.transform(tw, matrix=Matrix.Translation(c) @ Matrix.Rotation(a, 4, "Z"), verts=tw.verts)
        LS.merge(bm, tw)
        # flying buttresses (two per tower) to the colonnade entablature and to the dome base
        for zt, zd, rd in ((36, 31, 50), (22, 14, 50)):
            p0 = Vector((c.x, c.y, zt)) - Vector((math.cos(a), math.sin(a), 0)) * 5.5
            p2 = Vector((rd * math.cos(a), rd * math.sin(a), zd))
            p1 = (p0 + p2) / 2 + Vector((0, 0, 7))
            tube(bm, bezier(p0, p1, p2, 8), 1.1, "white")
            tube(bm, bezier(p0 + Vector((0, 0, 1.3)), p1 + Vector((0, 0, 1.3)), p2 + Vector((0, 0, 1.3)), 8), 0.35, "gold")
        LS.box(bm, (c.x * 1.0, c.y * 1.0, 69.2), (0.9, 0.9, 0.9), "window")
    # --- four arcaded docking bridges + pavilions
    for k in range(4):
        a = k * math.pi / 2
        rot = Matrix.Rotation(a, 4, "Z")
        br = bmesh.new()
        LS.box(br, (70, 0, -14), (70, 10, 2.0), "white")
        LS.box(br, (70, 0, -12.8), (70, 10.6, 0.4), "gold")
        LS.box(br, (70, 0, -3), (70, 10, 1.4), "white")
        for i in range(12):
            x = 38 + i * 5.6
            for y in (-4.6, 4.6):
                LS.box(br, (x, y, -8.5), (0.9, 0.9, 9.5), "white")
        for i in range(11):
            LS.box(br, (40.8 + i * 5.6, 0, -8.5), (3.6, 8.6, 7), "window")
        pav = lathe([(10, -16), (10.5, -15), (10.5, -4), (11, -3), (6, 4), (0.5, 7)], 24, "white", x0=106, y0=0)
        LS.merge(br, pav)
        bmesh.ops.transform(br, matrix=rot, verts=br.verts)
        LS.merge(bm, br)
        socks[f"Socket_Dock_{k}"] = (rot @ Vector((106, 0, 9)), a)
    # docking ring joining the four pavilions, with landing lights
    LS.merge(smooth, LS.loft([[Vector(((106 + 2.6 * math.cos(b)) * math.cos(a), (106 + 2.6 * math.cos(b)) * math.sin(a),
                                        -14 + 2.6 * math.sin(b))) for b in (2 * math.pi * j / 10 for j in range(10))]
                               for a in (2 * math.pi * i / 160 for i in range(161))], "silver", cap=False))
    for m in range(96):
        a = 2 * math.pi * (m + 0.5) / 96
        vs = LS.box(bm, (108.4, 0, -14), (0.5, 0.8, 0.8), "window")
        bmesh.ops.rotate(bm, verts=vs, cent=(0, 0, 0), matrix=Matrix.Rotation(a, 3, "Z"))
    lit = split_material(bm, "window")
    lob = LS.make_object("station_v3_windows", lit, M, coll, subsurf=0, smooth=False)
    sob = LS.make_object("station_v3_smooth", smooth, M, coll, subsurf=0)
    hob = LS.make_object("station_v3_hard", bm, M, coll, subsurf=0, smooth=False)
    C.add_bevel(hob, width=0.15, segments=1, angle=40)
    C.apply_modifiers(hob)
    C.shade_smooth_by_angle(hob, 35)
    ob = C.join([sob, hob, lob], "station_cradle_exchange_v3_LOD0")
    C.uv_box(ob, 1.0)
    socks["Socket_Beacon_Top"] = (Vector((0, 0, 110)), 0)
    socks["Socket_Beacon_Bottom"] = (Vector((0, 0, -104)), 0)
    return ob, socks


def materials():
    M = LS.materials()
    # marble-white for architecture: warmer and less glossy than the ship lacquer
    M["white"] = C.mat_painted("Cath_Marble", "#EDE6D8", metallic=0.05, rough=(0.3, 0.5),
                               tex_set="blue_metal_plate", uv_scale=0.05, normal_strength=0.15, grime=0.12)
    M["window"] = C.mat_emit("Cath_Window", "#FFC982", 7.0)
    return M


def main():
    C.reset_scene()
    C.setup_cycles(samples=160, res=(1920, 1080))
    C.world_space("#04060C", 1.0, hdri=os.path.join(C.TEX, "skybox", "cradle_sky_equirect.png"))
    C.add_sun("Star", (150, 0, 35), 4.5)
    C.add_sun("Rim", (-30, 0, -140), 0.4, color="#8FB4FF", angle_deg=4)
    M = materials()
    coll = bpy.data.collections.new("station_v3")
    bpy.context.scene.collection.children.link(coll)
    ob, socks = build(M, coll)
    r = C.bounding_radius([ob])
    assert r <= RADIUS_M, f"station v3 bounding radius {r:.1f} > {RADIUS_M}"
    root = bpy.data.objects.new("station_cradle_exchange_v3", None)
    coll.objects.link(root)
    ob.parent = root
    l1 = LS.lod1(ob, coll, ratio=0.3)
    l1.parent = root
    for name, (loc, a) in socks.items():
        C.add_empty(name, tuple(loc), parent=root, size=3, rot=(0, 0, a))
    print("V3STATION r=%.1f" % r, "tris", C.tri_count(ob), C.tri_count(l1))
    os.makedirs(LS.EXPORT, exist_ok=True)
    C.export_fbx(os.path.join(LS.EXPORT, "station_cradle_exchange_v3.fbx"), [root])
    C.save_blend(os.path.join(C.SOURCE, "station_v3.blend"))
    if os.environ.get("LOGH_PREVIEW", "0") == "0":
        return
    cam = C.add_camera((300, -380, 90), (0, 0, 0), lens=35, clip_end=100000)
    C.look_at(cam, (0, 0, 5))
    C.render(os.path.join(C.PREVIEWS, "v3_station_wide.png"))
    cam.location = (120, -150, 40)
    C.look_at(cam, (20, 0, 20))
    cam.data.lens = 28
    C.render(os.path.join(C.PREVIEWS, "v3_station_close.png"))


if __name__ == "__main__":
    main()

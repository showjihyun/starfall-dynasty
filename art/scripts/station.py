"""Cradle Exchange station (data/market/cradle-exchange.json: radius_m 120, dock range +200 m).

Run:  blender --background --factory-startup --python art/scripts/station.py
Out:  art/export/station_cradle_exchange.fbx  (Station_Static_LOD0..2, Station_Ring_LOD0..2 rotating
                                                part, Socket_Dock_{0..3}, Socket_Beacon_{Top,Bottom})
      art/source/station.blend, art/previews/station_*.png

Up = Blender +Z (Unity +Y). The station stands vertically "on" the star like a lighthouse
(p1-03 2: exchange sits 1,500 m straight above the star). Bounding radius <= 120 m (asserted).
Ring_* is a separate child so Unity can spin it (~1.5 deg/s) without touching the rest.
"""
import math
import os
import random
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import bpy
import bmesh
from mathutils import Vector, Matrix

import common as C
import ship_scout as S

SEED = 9001
RADIUS_M = 120.0
DOCK_RANGE_M = 200.0

S.ORDER.append("solar")
S.ORDER.append("beacon")


def materials():
    M = S.materials()
    M["hull"] = C.mat_painted("St_Hull", "#C4C0B8", metallic=0.05, rough=(0.38, 0.65),
                              tex_set="blue_metal_plate", uv_scale=0.12, normal_strength=0.3, grime=0.28)
    M["dark"] = C.mat_painted("St_Dark", "#30333A", metallic=0.55, rough=(0.4, 0.72),
                              tex_set="metal_plate_02", uv_scale=0.2, normal_strength=0.5, grime=0.15)
    M["metal"] = C.mat_painted("St_Metal", "#8C8F94", metallic=0.9, rough=(0.25, 0.5),
                               tex_set="metal_plate_02", uv_scale=0.25, normal_strength=0.3, grime=0.1)
    M["accent"] = C.mat_painted("St_Accent", "#C98A22", metallic=0.05, rough=(0.3, 0.55),
                                tex_set="blue_metal_plate", uv_scale=0.12, grime=0.3)
    M["window"] = C.mat_emit("St_Window", "#FFC98A", 5.0)
    M["solar"] = solar_material()
    M["beacon"] = C.mat_emit("St_Beacon", "#FFB648", 30.0)
    return M


def solar_material():
    m = bpy.data.materials.new("St_Solar")
    m.use_nodes = True
    nt = m.node_tree
    b = nt.nodes["Principled BSDF"]
    tc = nt.nodes.new("ShaderNodeTexCoord")
    br = nt.nodes.new("ShaderNodeTexBrick")
    br.inputs["Scale"].default_value = 1.0
    br.inputs["Mortar Size"].default_value = 0.04
    br.inputs["Brick Width"].default_value = 1.6
    br.inputs["Row Height"].default_value = 1.6
    br.offset = 0.0
    br.inputs["Color1"].default_value = C.srgb("#0E1A3A")
    br.inputs["Color2"].default_value = C.srgb("#13244A")
    br.inputs["Mortar"].default_value = C.srgb("#8C8F94")
    nt.links.new(tc.outputs["UV"], br.inputs["Vector"])
    nt.links.new(br.outputs["Color"], b.inputs["Base Color"])
    b.inputs["Metallic"].default_value = 0.4
    b.inputs["Roughness"].default_value = 0.18
    b.inputs["Coat Weight"].default_value = 0.6
    return m


I = S.idx


def rotate_bm(bm, mtx):
    bmesh.ops.transform(bm, matrix=mtx, verts=bm.verts)


def spine(rng):
    """Vertical octagonal spine, lofted along Y then stood up on Z."""
    prof = [(-104, 6, 0.3), (-100, 10, 0.3), (-86, 11, 0.32), (-60, 9, 0.3), (-44, 9, 0.3), (-36, 14, 0.3),
            (-24, 14, 0.3), (-16, 9, 0.3), (10, 9, 0.3), (14, 12, 0.3), (26, 12, 0.3), (30, 9, 0.3),
            (70, 8, 0.3), (84, 10, 0.3), (92, 10, 0.3), (98, 5, 0.3)]
    secs = [S.section(2 * r, 2 * r, r * c * 1.4, y) for y, r, c in prof]
    bm, grid, _ = S.grid_loft(secs)
    S.panel_shell(bm, grid, rng, gap=0.18, ring_span=(1, 1), seg_span=2)
    for f in bm.faces:
        cen = f.calc_center_median()
        if -36 < cen.y < -24 or 14 < cen.y < 26:
            f.material_index = I("dark")
        if 84 < cen.y < 92:
            f.material_index = I("accent")
    S.merge(bm, S.core_from(secs, inset=0.25))
    rotate_bm(bm, Matrix.Rotation(math.radians(90), 4, "X"))
    return bm


def ring_section(theta, R, w, h, c, z0):
    pts = S.section(w, h, c, 0.0, x0=R, z0=z0)
    rot = Matrix.Rotation(theta, 3, "Z")
    return [rot @ p for p in pts]


def habitat_ring(rng, R=80.0, w=13.0, h=9.0, z0=20.0, n=48):
    secs = [ring_section(2 * math.pi * k / n, R, w, h, 2.6, z0) for k in range(n + 1)]
    bm, grid, _ = S.grid_loft(secs, cap=False)
    S.panel_shell(bm, grid, rng, gap=0.16, ring_span=(1, 2), seg_span=2)
    for f in bm.faces:
        cen = f.calc_center_median()
        n_ = f.normal
        rad = Vector((cen.x, cen.y, 0)).normalized()
        if n_.dot(rad) < -0.6:  # inner face: dark
            f.material_index = I("dark")
        if abs(n_.z) > 0.8 and (int(math.degrees(math.atan2(cen.y, cen.x)) + 360) // 45) % 2 == 0 \
                and Vector((cen.x, cen.y, 0)).length > R + 2.0:
            f.material_index = I("accent")
    core = bmesh.new()
    csecs = [ring_section(2 * math.pi * k / n, R, w - 0.5, h - 0.5, 2.4, z0) for k in range(n + 1)]
    cbm, _, _ = S.grid_loft(csecs, mat="core", cap=False)
    S.merge(bm, cbm)
    g = bmesh.new()
    # window strips on the outer wall: two rows, broken into lit / dark bays
    for k in range(n * 3):
        th = 2 * math.pi * (k + 0.5) / (n * 3)
        if rng.random() < 0.18:
            continue
        rot = Matrix.Rotation(th, 4, "Z")
        for zz in (z0 + 1.6, z0 - 1.4):
            S.box(g, (R + w / 2 + 0.05, 0, zz), (0.2, 2 * math.pi * R / (n * 3) * 0.55, 0.9), "window", mtx=rot)
    # structural ribs every 15 degrees
    for k in range(24):
        rot = Matrix.Rotation(2 * math.pi * k / 24, 4, "Z")
        S.box(g, (R, 0, z0), (w + 0.8, 0.9, h + 0.8), "dark", mtx=rot)
    # 4 spokes to the hub collar
    for k in range(4):
        rot = Matrix.Rotation(math.pi / 4 + k * math.pi / 2, 4, "Z")
        S.box(g, ((R + 12) / 2, 0, z0), (R - 12 - w / 2 + 1, 3.6, 3.6), "hull", mtx=rot)
        S.box(g, ((R + 12) / 2, 0, z0 + 1.9), (R - 14 - w / 2, 1.2, 0.3), "dark", mtx=rot)
        S.cyl(g, ((R + 12) / 2, 2.4, z0), 0.4, R - 16 - w / 2, "metal", axis="X", segs=8, mtx=rot)
    # hub collar (rotates with the ring)
    secs2 = [S.section(30, 30, 9, y) for y in (-4, 4)]
    col, _, _ = S.grid_loft(secs2, mat="dark")
    rotate_bm(col, Matrix.Translation((0, 0, z0)) @ Matrix.Rotation(math.radians(90), 4, "X"))
    S.merge(g, col)
    S.merge(bm, g)
    return bm


def truss(bm, p0, p1, wdt, mat="metal", bay=10.0, t=0.5):
    p0, p1 = Vector(p0), Vector(p1)
    d = (p1 - p0)
    L_ = d.length
    z = d.normalized()
    up = Vector((0, 0, 1)) if abs(z.z) < 0.9 else Vector((1, 0, 0))
    x = up.cross(z).normalized()
    y = z.cross(x)
    corners = [(x + y) * wdt / 2, (x - y) * wdt / 2, (-x - y) * wdt / 2, (-x + y) * wdt / 2]
    for c in corners:
        S.seg_box(bm, p0 + c, p1 + c, t, mat)
    nb = max(1, int(L_ / bay))
    for i in range(nb + 1):
        q = p0 + d * (i / nb)
        for a, b in zip(corners, corners[1:] + corners[:1]):
            S.seg_box(bm, q + a, q + b, t * 0.8, mat)
        if i < nb:
            q2 = p0 + d * ((i + 1) / nb)
            S.seg_box(bm, q + corners[0], q2 + corners[2], t * 0.6, mat)
            S.seg_box(bm, q + corners[1], q2 + corners[3], t * 0.6, mat)


def docking_arms(rng, z=-30.0):
    bm = bmesh.new()
    socks = {}
    for k in range(4):
        th = k * math.pi / 2
        dvec = Vector((math.cos(th), math.sin(th), 0))
        truss(bm, dvec * 13 + Vector((0, 0, z)), dvec * 88 + Vector((0, 0, z)), 6.0)
        # pressurised tube along the truss
        rot = Matrix.Rotation(th, 4, "Z")
        S.cyl(bm, (50, 0, z + 4.5), 1.8, 74, "hull", axis="X", segs=12, mtx=rot)
        for xx in range(18, 88, 10):
            S.cyl(bm, (xx, 0, z + 4.5), 2.1, 0.8, "dark", axis="X", segs=12, mtx=rot)
        # docking cradle: U frame + port ring + amber lights
        # cradle sized for a Scout (20 x 12 m) with ~4 m clearance each side
        S.box(bm, (98, 0, z), (28, 21, 2.0), "dark", mtx=rot)
        S.box(bm, (98, 10.0, z + 4), (28, 1.5, 8), "hull", mtx=rot)
        S.box(bm, (98, -10.0, z + 4), (28, 1.5, 8), "hull", mtx=rot)
        S.box(bm, (111.8, 0, z + 4), (0.6, 21.5, 9.5), "accent", mtx=rot)
        S.cyl(bm, (89, 0, z + 4.5), 2.6, 1.2, "dark", axis="X", segs=16, mtx=rot)
        for yy in (-10.0, 10.0):
            S.box(bm, (112.2, yy, z + 8.6), (0.6, 0.6, 0.6), "beacon", mtx=rot)
            for xx in (88.0, 94.0, 100.0, 106.0):
                S.box(bm, (xx, yy * 0.88, z + 1.05), (1.0, 0.5, 0.1), "window", mtx=rot)
        socks[f"Socket_Dock_{k}"] = (rot @ Vector((98, 0, z + 4.5)), th)
    return bm, socks


def solar_and_radiators(rng):
    bm = bmesh.new()
    # two solar wings facing the star (below), on a boom along X
    for sx in (1, -1):
        truss(bm, (sx * 14, 0, -6), (sx * 104, 0, -6), 3.0)
        for i in range(4):
            x0 = sx * (24 + i * 20)
            S.box(bm, (x0, 0, -8.2), (18.5, 26.0, 0.25), "solar")
            S.box(bm, (x0, 0, -8.0), (18.8, 26.3, 0.12), "dark")
    # radiator fins below the reactor
    for k in range(4):
        rot = Matrix.Rotation(math.pi / 4 + k * math.pi / 2, 4, "Z")
        S.box(bm, (26, 0, -76), (34, 0.5, 26), "metal", mtx=rot)
        for j in range(6):
            S.box(bm, (12 + j * 5.6, 0, -76), (0.6, 0.9, 26.5), "dark", mtx=rot)
        S.box(bm, (10, 0, -76), (6, 3, 10), "dark", mtx=rot)
    # spine greebles: vertical conduits, service boxes, maintenance lights
    for k in range(8):
        th = k * math.pi / 4 + math.pi / 8
        rot = Matrix.Rotation(th, 4, "Z")
        for z0, z1 in ((-58, -38), (-14, 8), (32, 68)):
            S.cyl(bm, (9.6, 0, (z0 + z1) / 2), 0.45, z1 - z0, "metal", axis="Z", segs=8, mtx=rot)
            for zz in range(int(z0) + 3, int(z1), 6):
                S.box(bm, (9.6, 0, zz), (1.3, 1.3, 0.6), "dark", mtx=rot)
        if k % 2 == 0:
            for zz in (-50, 0, 50):
                S.box(bm, (9.4, 0, zz + 4), (1.6, 3.2, 4.5), "dark", mtx=rot)
                S.box(bm, (10.25, 0, zz + 5.6), (0.2, 2.4, 0.4), "window", mtx=rot)
    # comm mast + dishes on top
    S.cyl(bm, (0, 0, 104), 1.2, 12, "metal", axis="Z", segs=10)
    S.cyl(bm, (0, 0, 109.5), 0.25, 2.0, "metal", axis="Z", segs=6)
    S.box(bm, (0, 0, 110.7), (1.4, 1.4, 1.4), "beacon")
    for k in range(3):
        rot = Matrix.Rotation(k * 2.094, 4, "Z")
        S.cyl(bm, (9, 0, 90), 4.5, 1.2, "hull", axis="Z", segs=20, r2=1.5, mtx=rot @ Matrix.Rotation(0.5, 4, "Y"))
        S.cyl(bm, (7, 0, 88.8), 0.4, 3, "metal", axis="Z", segs=8, mtx=rot)
    # bottom beacon
    S.box(bm, (0, 0, -105.5), (1.6, 1.6, 1.6), "beacon")
    # hull number + EXCHANGE band (stencil-ish amber blocks) on the docking collar
    for k in range(8):
        rot = Matrix.Rotation(k * math.pi / 4, 4, "Z")
        S.box(bm, (14.1, 0, -30), (0.3, 6.0, 1.2), "accent", mtx=rot)
        S.box(bm, (14.2, 0, -20), (0.3, 4.0, 0.8), "window", mtx=rot)
    return bm


def build(M, coll):
    rng = random.Random(SEED)
    mats = [M[k] for k in S.ORDER]
    static = spine(rng)
    arms, socks = docking_arms(rng)
    S.merge(static, arms)
    S.merge(static, solar_and_radiators(rng))
    st = C.new_obj("Station_Static_LOD0", static, mats, coll)
    sol = st.modifiers.new("Solid", "SOLIDIFY")
    sol.thickness = 0.12
    sol.offset = -1
    C.add_bevel(st, width=0.05, segments=1, angle=40)
    C.add_weighted_normals(st)
    rb = habitat_ring(rng)
    ring = C.new_obj("Station_Ring_LOD0", rb, mats, coll)
    sol = ring.modifiers.new("Solid", "SOLIDIFY")
    sol.thickness = 0.12
    sol.offset = -1
    C.add_bevel(ring, width=0.05, segments=1, angle=40)
    C.add_weighted_normals(ring)
    for o in (st, ring):
        C.apply_modifiers(o)
        C.shade_smooth_by_angle(o, 30)
        C.uv_box(o, 1.0)
    return st, ring, socks


def lods(ob, coll):
    out = [ob]
    for i, ratio in ((1, 0.25), (2, 0.06)):
        cp = ob.copy()
        cp.data = ob.data.copy()
        cp.name = ob.name.replace("_LOD0", f"_LOD{i}")
        coll.objects.link(cp)
        d = cp.modifiers.new("Dec", "DECIMATE")
        d.ratio = ratio
        C.apply_modifiers(cp)
        cp.hide_render = True
        out.append(cp)
    return out


def append_ship(variant="standard"):
    path = os.path.join(C.SOURCE, "ship_scout.blend")
    if not os.path.exists(path):
        return None
    name = f"ship_scout_{variant}_LOD0"
    with bpy.data.libraries.load(path, link=False) as (src, dst):
        dst.objects = [n for n in src.objects if n == name]
    ob = dst.objects[0]
    bpy.context.scene.collection.objects.link(ob)
    ob.parent = None
    return ob


def main():
    C.reset_scene()
    C.setup_cycles(samples=160, res=(1920, 1080))
    sky = os.path.join(C.TEX, "skybox", "cradle_sky_equirect.png")
    C.world_space("#04060C", 1.0, hdri=sky if os.path.exists(sky) else None)
    # the star is BELOW the station: key light from below-side
    C.add_sun("Star", (150, 0, 35), 4.5)
    C.add_sun("Rim", (-30, 0, -140), 0.3, color="#8FB4FF", angle_deg=4)
    C.add_sun("Fill", (60, 0, 210), 0.08, color="#5C7AB8", angle_deg=20)
    M = materials()
    coll = bpy.data.collections.new("station")
    bpy.context.scene.collection.children.link(coll)
    st, ring, socks = build(M, coll)
    r = C.bounding_radius([st, ring])
    assert r <= RADIUS_M, f"station bounding radius {r:.1f} > {RADIUS_M}"
    root = bpy.data.objects.new("station_cradle_exchange", None)
    coll.objects.link(root)
    stl = lods(st, coll)
    rgl = lods(ring, coll)
    for o in stl + rgl:
        o.parent = root
    for name, (loc, th) in socks.items():
        e = C.add_empty(name, loc, parent=root, size=3, rot=(0, 0, th))
    C.add_empty("Socket_Beacon_Top", (0, 0, 110.7), parent=root, size=3)
    C.add_empty("Socket_Beacon_Bottom", (0, 0, -105.5), parent=root, size=3)
    print("STATIONREPORT r=%.1f" % r, "static", [C.tri_count(o) for o in stl], "ring", [C.tri_count(o) for o in rgl])
    C.export_fbx(os.path.join(C.EXPORT, "station_cradle_exchange.fbx"), [root])
    C.save_blend(os.path.join(C.SOURCE, "station.blend"))

    # previews
    ship = append_ship()
    star = bpy.data.objects.new("CradleStar", None)
    sm = bpy.data.meshes.new("star")
    bm = bmesh.new()
    bmesh.ops.create_uvsphere(bm, u_segments=64, v_segments=32, radius=600)
    bm.to_mesh(sm)
    star = bpy.data.objects.new("CradleStar", sm)
    star.location = (0, 0, -1500)
    sm.materials.append(C.mat_emit("StarSurface", "#FFE6C0", 40.0))
    bpy.context.scene.collection.objects.link(star)
    cam = C.add_camera((330, -420, 60), (0, 0, -20), lens=35, clip_end=50000)
    C.render(os.path.join(C.PREVIEWS, "station_wide.png"))
    if ship:
        ship.location = (98, 0, -26.3)  # hovering over the cradle, about to settle
        ship.rotation_euler = (0, 0, math.radians(90))  # nose (-Y) pointing outward (+X)
        cam.location = (150, -62, 8)
        C.look_at(cam, (92, 0, -20))
        cam.data.lens = 30
        C.render(os.path.join(C.PREVIEWS, "station_dock_with_scout.png"))
    cam.location = (1300, -1700, 150)
    C.look_at(cam, (0, 0, -350))
    cam.data.lens = 35
    C.render(os.path.join(C.PREVIEWS, "station_above_star.png"))


if __name__ == "__main__":
    main()

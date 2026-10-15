"""STARFALL DYNASTY ship interiors: the capital/cruiser command bridge and the fighter cockpit.

Shown in a picture-in-picture panel while flying in third person. The client renders the real
battle outside onto the `ViewScreen` mesh, spins `Holo`, drives `Alert_*` on red alert and places
point lights at `Light_*`; the panel camera sits at `CamAnchor` looking at `CamLook`.

Run (repo root, Git Bash):
  "C:/Program Files/Blender Foundation/Blender 5.2/blender.exe" -b --factory-startup -P tools/blender/build_bridge.py -- [--no-preview]

Conventions as the ships: forward = Blender +Y (glTF -Z), up = +Z, meters. No baking: flat PBR
materials (the interior is seen small and lit live), so the build takes seconds.
"""
import bpy
import bmesh
import math
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import shiplib as S  # noqa: E402
from shiplib import V, box, cyl, sphere, lathe, circle_ring, loft  # noqa: E402

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
OUT = os.path.join(ROOT, "crates", "client", "assets", "models")
PREV = os.path.join(os.path.dirname(os.path.abspath(__file__)), "previews")

# Material slot order shared by every part.
MATS = [
    # name, base rgb, metallic, roughness, emission rgb, emission strength, alpha
    ("Floor", (0.055, 0.06, 0.07), 0.85, 0.42, None, 0, 1),
    ("Wall", (0.085, 0.095, 0.115), 0.6, 0.45, None, 0, 1),
    ("Trim", (0.38, 0.40, 0.44), 0.95, 0.3, None, 0, 1),
    ("Console", (0.09, 0.10, 0.13), 0.6, 0.4, None, 0, 1),
    ("Screen", (0.02, 0.05, 0.08), 0.0, 0.3, (0.25, 0.75, 1.0), 4.0, 1),
    ("ScreenAmber", (0.06, 0.03, 0.01), 0.0, 0.3, (1.0, 0.55, 0.15), 3.5, 1),
    ("LightStrip", (0.9, 0.9, 0.85), 0.0, 0.5, (1.0, 0.93, 0.82), 8.0, 1),
    ("Uniform", (0.05, 0.065, 0.11), 0.0, 0.7, None, 0, 1),
    ("Officer", (0.02, 0.02, 0.025), 0.1, 0.55, None, 0, 1),
    ("Silver", (0.75, 0.76, 0.8), 1.0, 0.2, None, 0, 1),
    ("Cape", (0.42, 0.03, 0.04), 0.0, 0.8, None, 0, 1),
    ("Skin", (0.78, 0.6, 0.5), 0.0, 0.55, None, 0, 1),
    ("Hair", (0.12, 0.08, 0.05), 0.0, 0.6, None, 0, 1),
    ("Gold", (0.95, 0.72, 0.3), 1.0, 0.25, None, 0, 1),
    ("Holo", (0.0, 0.0, 0.0), 0.0, 0.5, (0.3, 0.85, 1.0), 5.0, 0.35),
    ("Alert", (0.1, 0.0, 0.0), 0.0, 0.5, (1.0, 0.1, 0.05), 0.4, 1),
    ("ViewScreen", (0.0, 0.0, 0.0), 0.0, 1.0, (0.05, 0.08, 0.14), 1.0, 1),
    ("Glass", (0.1, 0.14, 0.18), 0.0, 0.05, None, 0, 0.25),
]
IDX = {m[0]: i for i, m in enumerate(MATS)}
(FLOOR, WALL, TRIM, CONSOLE, SCREEN, AMBER, LIGHT, UNIFORM, OFFICER, SILVER, CAPE, SKIN, HAIR, GOLD,
 HOLO, ALERT, VIEW, GLASS) = range(len(MATS))


def make_materials():
    mats = []
    for name, rgb, metal, rough, emit, strength, alpha in MATS:
        m = bpy.data.materials.new(name)
        m.use_nodes = True
        b = m.node_tree.nodes.get("Principled BSDF")
        b.inputs["Base Color"].default_value = (*rgb, 1)
        b.inputs["Metallic"].default_value = metal
        b.inputs["Roughness"].default_value = rough
        if emit:
            b.inputs["Emission Color"].default_value = (*emit, 1)
            b.inputs["Emission Strength"].default_value = strength
        if alpha < 1:
            b.inputs["Alpha"].default_value = alpha
            m.surface_render_method = 'BLENDED'
        mats.append(m)
    return mats


class Builder:
    def __init__(self, name, mats):
        self.name = name
        self.mats = mats
        self.objs = []
        self.empties = []
        self.coll = bpy.context.scene.collection

    def mesh(self, name, build, recalc=True, smooth=False, parent=None):
        bm = bmesh.new()
        build(bm)
        bmesh.ops.remove_doubles(bm, verts=bm.verts, dist=1e-5)
        if recalc:
            bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
        me = bpy.data.meshes.new(name)
        bm.to_mesh(me)
        bm.free()
        if not me.uv_layers:
            me.uv_layers.new(name="UVMap")
        for m in self.mats:
            me.materials.append(m)
        if smooth:
            for p in me.polygons:
                p.use_smooth = True
        o = bpy.data.objects.new(name, me)
        self.coll.objects.link(o)
        if parent is not None:
            o.parent = parent
        self.objs.append(o)
        return o

    def empty(self, name, loc, scale=1.0, parent=None):
        e = bpy.data.objects.new(name, None)
        e.location = V(loc)
        e.scale = (scale, scale, scale)
        self.coll.objects.link(e)
        if parent is not None:
            e.parent = parent
        self.empties.append(e)
        return e


def quad(bm, pts, mat, toward):
    """Single face whose normal points along `toward` (rooms are seen from inside)."""
    vs = [bm.verts.new(V(p)) for p in pts]
    f = bm.faces.new(vs)
    f.normal_update()
    if f.normal.dot(V(toward)) < 0:
        f.normal_flip()
    f.material_index = mat
    return f


def curved_screen(bm, center, radius, a0, a1, z0, z1, mat, n=32, inward=True):
    """Cylinder section facing its axis, with UVs u: left→right as seen from the axis, v: bottom→top."""
    uv = bm.loops.layers.uv.verify()
    rows = []
    for z in (z0, z1):
        row = []
        for i in range(n + 1):
            a = a1 + (a0 - a1) * i / n  # left (larger angle) to right
            row.append(bm.verts.new(V((center[0] + radius * math.cos(a), center[1] + radius * math.sin(a), z))))
        rows.append(row)
    for i in range(n):
        f = bm.faces.new((rows[0][i], rows[0][i + 1], rows[1][i + 1], rows[1][i]))
        f.material_index = mat
        f.normal_update()
        mid = sum((v.co for v in f.verts), V((0, 0, 0))) / 4
        to_axis = V((center[0], center[1], mid.z)) - mid
        if (f.normal.dot(to_axis) < 0) == inward:
            f.normal_flip()
        for loop in f.loops:
            k = rows[0].index(loop.vert) if loop.vert in rows[0] else rows[1].index(loop.vert)
            loop[uv].uv = (k / n, 0.0 if loop.vert in rows[0] else 1.0)


def figure(bm, base, seated=True, mats=(UNIFORM, SKIN, HAIR), facing=0.0, scale=1.0, arms_forward=True, cape=False, trim=False):
    """Stylised crew member. `base` = floor point under the seat/feet, facing +Y rotated by `facing` rad."""
    from mathutils import Matrix
    uni, skin, hair = mats
    R = Matrix.Rotation(facing, 3, 'Z')
    b = V(base)

    def P(x, y, z):
        return b + R @ (V((x, y, z)) * scale)

    def bx(c, s, m, taper=None):
        box(bm, P(*c), [v * scale for v in s], mat=m, rot=R, taper=taper)

    hip = 0.5 if seated else 0.95
    # Legs.
    for sx in (-0.12, 0.12):
        if seated:
            bx((sx, 0.22, hip), (0.15, 0.48, 0.16), uni)
            bx((sx, 0.44, hip / 2), (0.13, 0.14, hip), uni)
            bx((sx, 0.5, 0.04), (0.12, 0.26, 0.08), OFFICER)
        else:
            bx((sx, 0.0, hip / 2), (0.15, 0.16, hip), uni)
            bx((sx, 0.05, 0.04), (0.12, 0.26, 0.08), OFFICER)
    # Torso (tapered toward the waist), shoulders, head.
    bx((0, 0, hip + 0.32), (0.42, 0.24, 0.62), uni, taper=None)
    bx((0, 0, hip + 0.62), (0.5, 0.26, 0.1), uni)
    if trim:
        bx((0, 0.125, hip + 0.42), (0.07, 0.02, 0.5), SILVER)
        for sx in (-0.22, 0.22):
            bx((sx, 0, hip + 0.68), (0.12, 0.2, 0.03), GOLD)
    cyl(bm, P(0, 0, hip + 0.66), P(0, 0, hip + 0.76), 0.06 * scale, n=8, mat=skin)
    sphere(bm, P(0, 0.01, hip + 0.88), 0.115 * scale, mat=skin, seg=12, rings=8, scale=(0.95, 1.05, 1.15))
    sphere(bm, P(0, -0.02, hip + 0.93), 0.122 * scale, mat=hair, seg=12, rings=6, scale=(1.0, 1.0, 0.75))
    # Arms: forward to a console, or at the sides.
    for sx in (-0.29, 0.29):
        if arms_forward:
            cyl(bm, P(sx, 0.0, hip + 0.6), P(sx * 0.9, 0.18, hip + 0.3), 0.055 * scale, n=8, mat=uni)
            cyl(bm, P(sx * 0.9, 0.18, hip + 0.3), P(sx * 0.6, 0.5, hip + 0.32), 0.05 * scale, n=8, mat=uni)
            sphere(bm, P(sx * 0.6, 0.53, hip + 0.32), 0.045 * scale, mat=skin, seg=8, rings=5)
        else:
            cyl(bm, P(sx, 0.0, hip + 0.6), P(sx * 1.05, 0.02, hip + 0.05), 0.055 * scale, n=8, mat=uni)
            sphere(bm, P(sx * 1.05, 0.02, hip + 0.0), 0.045 * scale, mat=skin, seg=8, rings=5)
    if cape:
        # Draped from the shoulders down the back.
        top = [P(x, -0.15, hip + 0.66) for x in (-0.27, -0.09, 0.09, 0.27)]
        bot = [P(x * 1.5, -0.32 - abs(x) * 0.3, 0.12 if not seated else hip - 0.05) for x in (-0.27, -0.09, 0.09, 0.27)]
        loft(bm, [top, bot], mat=CAPE, closed=False, cap_start=False, cap_end=False)


def chair(bm, base, facing=0.0, high=False, mat=CONSOLE):
    from mathutils import Matrix
    R = Matrix.Rotation(facing, 3, 'Z')
    b = V(base)

    def P(x, y, z):
        return b + R @ V((x, y, z))
    cyl(bm, P(0, 0, 0), P(0, 0, 0.42), 0.06, n=10, mat=TRIM)
    box(bm, P(0, 0.05, 0.46), (0.56, 0.56, 0.1), mat=mat, rot=R)
    h = 1.35 if high else 0.75
    box(bm, P(0, -0.25, 0.46 + h / 2), (0.56 if not high else 0.7, 0.1, h), mat=mat, rot=R)
    for sx in (-0.3, 0.3):
        box(bm, P(sx * (1.2 if high else 1.0), 0.05, 0.68), (0.07, 0.5, 0.06), mat=TRIM, rot=R)


def console(bm, base, width=1.6, facing=0.0, amber=False):
    """Crew desk: body, angled glowing work surface and a raised monitor (operator stands/sits at -Y)."""
    from mathutils import Matrix
    R = Matrix.Rotation(facing, 3, 'Z')
    b = V(base)

    def P(x, y, z):
        return b + R @ V((x, y, z))
    box(bm, P(0, 0.15, 0.38), (width, 0.6, 0.76), mat=CONSOLE, rot=R)
    tilt = Matrix.Rotation(math.radians(-22), 3, 'X')
    box(bm, P(0, 0.05, 0.8), (width * 0.98, 0.62, 0.05), mat=CONSOLE, rot=R @ tilt)
    box(bm, P(0, 0.05, 0.83), (width * 0.86, 0.5, 0.012), mat=AMBER if amber else SCREEN, rot=R @ tilt)
    box(bm, P(0, 0.42, 1.25), (width * 0.8, 0.06, 0.62), mat=CONSOLE, rot=R)
    box(bm, P(0, 0.385, 1.25), (width * 0.72, 0.012, 0.52), mat=SCREEN if not amber else AMBER, rot=R)
    box(bm, P(0, -0.13, 0.79), (width, 0.05, 0.03), mat=TRIM, rot=R)


def build_bridge(mats):
    B = Builder("bridge", mats)
    W, D, Hh = 13.0, 11.0, 8.0
    dais = 1.6

    def shell(bm):
        # Floors: crew pit (front) and the raised command dais (rear) with its riser.
        quad(bm, [(-W, -4, 0), (W, -4, 0), (W, D, 0), (-W, D, 0)], FLOOR, (0, 0, 1))
        quad(bm, [(-W, -D, dais), (W, -D, dais), (W, -4, dais), (-W, -4, dais)], FLOOR, (0, 0, 1))
        quad(bm, [(-W, -4, 0), (W, -4, 0), (W, -4, dais), (-W, -4, dais)], WALL, (0, 1, 0))
        # Ceiling, walls (front wall around the screen opening is built separately).
        quad(bm, [(-W, -D, Hh), (W, -D, Hh), (W, D, Hh), (-W, D, Hh)], WALL, (0, 0, -1))
        quad(bm, [(-W, -D, 0), (W, -D, 0), (W, -D, Hh), (-W, -D, Hh)], WALL, (0, 1, 0))
        for sx in (-1, 1):
            quad(bm, [(sx * W, -D, 0), (sx * W, D, 0), (sx * W, D, Hh), (sx * W, -D, Hh)], WALL, (-sx, 0, 0))
        quad(bm, [(-W, D, 0), (W, D, 0), (W, D, Hh), (-W, D, Hh)], WALL, (0, -1, 0))
    B.mesh("Shell", shell, recalc=False)

    def structure(bm):
        # Ceiling ribs and light strips, wall pillars, side wall panelling.
        for y in (-8, -4, 0, 4, 8):
            box(bm, (0, y, Hh - 0.25), (2 * W, 0.5, 0.5), mat=TRIM)
        for x in (-6, 0, 6):
            box(bm, (x, 0, Hh - 0.08), (0.35, 2 * D - 1, 0.1), mat=LIGHT)
        for sx in (-1, 1):
            for y in (-8, -4, 0, 4, 8):
                box(bm, (sx * (W - 0.3), y, Hh / 2), (0.6, 0.7, Hh), mat=TRIM)
            for y in (-6, -2, 2, 6):
                box(bm, (sx * (W - 0.15), y, 3.4), (0.2, 3.2, 3.6), mat=WALL)
                box(bm, (sx * (W - 0.27), y, 3.4), (0.06, 2.6, 0.12), mat=LIGHT)
                box(bm, (sx * (W - 0.3), y, 2.2), (0.05, 2.2, 1.0), mat=SCREEN if (y // 4) % 2 == 0 else AMBER)
        # Dais edge: trim, two steps down the middle, railings either side.
        box(bm, (0, -4.05, dais - 0.04), (2 * W, 0.1, 0.08), mat=TRIM)
        for k, z in enumerate((dais * 2 / 3, dais / 3)):
            box(bm, (0, -3.6 + k * 0.45, z - dais / 6), (2.4, 0.45, z + dais / 6 - 0.0), mat=FLOOR)
        for sx in (-1, 1):
            x0, x1 = sx * 1.5, sx * (W - 0.8)
            box(bm, ((x0 + x1) / 2, -4.25, dais + 1.0), (abs(x1 - x0), 0.08, 0.08), mat=SILVER)
            for x in range(2, int(W) - 1, 2):
                box(bm, (sx * x, -4.25, dais + 0.5), (0.06, 0.06, 1.0), mat=SILVER)
        # Front: frame around the giant curved viewscreen and a lower bulkhead with status panels.
        box(bm, (0, D - 0.4, 0.6), (2 * W, 0.8, 1.2), mat=WALL)
        for x in range(-10, 11, 4):
            box(bm, (x, D - 0.82, 0.75), (2.6, 0.04, 0.5), mat=SCREEN)
        box(bm, (0, D - 0.6, Hh - 0.5), (2 * W, 1.2, 1.0), mat=WALL)
    B.mesh("Structure", structure)

    def screen(bm):
        # Curved panoramic screen: radius 12.8 around (0,-1.8), ±45° → about 3:1 like the feed.
        curved_screen(bm, (0, -1.8), 12.8, math.radians(45), math.radians(135), 1.2, 7.4, VIEW, n=40)
    B.mesh("ViewScreen", screen, recalc=False)

    def screen_frame(bm):
        for a in (45, 135):
            r = math.radians(a)
            p = V((12.8 * math.cos(r), -1.8 + 12.8 * math.sin(r), 4.3))
            box(bm, p, (0.5, 0.5, 6.6), mat=TRIM)
        arc_top = [V((13.0 * math.cos(math.radians(a)), -1.8 + 13.0 * math.sin(math.radians(a)), 7.5)) for a in range(45, 136, 5)]
        for i in range(len(arc_top) - 1):
            box(bm, (arc_top[i] + arc_top[i + 1]) / 2, ((arc_top[i + 1] - arc_top[i]).length + 0.05, 0.3, 0.25), mat=TRIM,
                rot=__import__('mathutils').Matrix.Rotation(math.atan2((arc_top[i + 1] - arc_top[i]).y, (arc_top[i + 1] - arc_top[i]).x), 3, 'Z'))
    B.mesh("ScreenFrame", screen_frame)

    # Crew pit: two rows of stations facing the screen.
    seats = [(-9, 6.0), (-6, 6.0), (-3, 6.0), (3, 6.0), (6, 6.0), (9, 6.0), (-8, 2.4), (-4.6, 2.4), (4.6, 2.4), (8, 2.4)]

    def stations(bm):
        for i, (x, y) in enumerate(seats):
            console(bm, (x, y, 0), width=2.0, amber=(i % 3 == 1))
            chair(bm, (x, y - 0.75, 0))
    B.mesh("Stations", stations)

    def crew(bm):
        for i, (x, y) in enumerate(seats):
            figure(bm, (x, y - 0.95, 0), seated=True, mats=(UNIFORM, SKIN, HAIR if i % 4 else GOLD))
    B.mesh("Crew", crew, smooth=True)

    # Command dais: the commander's high-backed chair, the commander (black and silver, red cape)
    # and two staff officers standing behind.
    def command(bm):
        chair(bm, (0, -7.2, dais), high=True, mat=OFFICER)
        box(bm, (0, -7.2, dais + 0.03), (2.6, 2.6, 0.06), mat=TRIM)
        for sx in (-1, 1):
            console(bm, (sx * 3.2, -6.4, dais), width=1.4, facing=sx * math.radians(-15), amber=True)
    B.mesh("Command", command)

    def officers(bm):
        figure(bm, (0, -7.35, dais), seated=True, mats=(OFFICER, SKIN, GOLD), arms_forward=False, cape=True, trim=True)
        for sx in (-1, 1):
            figure(bm, (sx * 1.7, -8.7, dais), seated=False, mats=(OFFICER, SKIN, HAIR), arms_forward=False, trim=True)
    B.mesh("Officers", officers, smooth=True)

    # Tactical holo table between the pit rows.
    def table(bm):
        lathe(bm, (0, 0.2, 0), (0, 0, 1), [(1.6, 0), (1.6, 0.15), (1.1, 0.3), (1.0, 0.85), (1.7, 0.95), (1.7, 1.05)], n=40, mat=CONSOLE)
        lathe(bm, (0, 0.2, 1.05), (0, 0, 1), [(1.72, 0), (1.72, 0.04), (1.5, 0.04)], n=40, mat=SILVER, cap_start=False)
        lathe(bm, (0, 0.2, 1.02), (0, 0, 1), [(1.45, 0), (1.45, 0.03)], n=40, mat=SCREEN, cap_start=False)
    B.mesh("HoloTable", table)

    holo_pivot = B.empty("Holo", (0, 0.2, 1.1))

    def holo(bm):
        # A miniature battle: two fleets of chevrons above a translucent star map.
        lathe(bm, (0, 0, 0), (0, 0, 1), [(0.05, 0), (1.35, 0.9), (1.35, 0.92), (0.05, 0.02)], n=40, mat=HOLO, cap_start=False, cap_end=False)
        for ring_r in (0.5, 0.9, 1.3):
            lathe(bm, (0, 0, 0.45), (0, 0, 1), [(ring_r, 0), (ring_r + 0.02, 0), (ring_r + 0.02, 0.01)], n=40, mat=HOLO, cap_start=False)
        for side, col in ((-1, HOLO), (1, AMBER)):
            for k in range(7):
                x = side * (0.35 + (k % 3) * 0.25)
                y = (k - 3) * 0.22
                z = 0.45 + (k % 2) * 0.12
                cyl(bm, (x + side * 0.08, y, z), (x - side * 0.08, y, z), 0.05, 0.0, n=4, mat=col)
    B.mesh("HoloProjection", holo, recalc=False).parent = holo_pivot

    # Red-alert strips along the upper walls and over the screen.
    for sx in (-1, 1):
        B.mesh("Alert_%s" % ("L" if sx < 0 else "R"), lambda bm, sx=sx: box(bm, (sx * (W - 0.45), 0, Hh - 1.0), (0.12, 2 * D - 3, 0.18), mat=ALERT))
    B.mesh("Alert_F", lambda bm: box(bm, (0, D - 1.25, Hh - 1.05), (2 * W - 2, 0.1, 0.16), mat=ALERT))

    for i, p in enumerate([(0, 4, Hh - 0.6), (0, -6, Hh - 0.6), (-9, 0, Hh - 1.4), (9, 0, Hh - 1.4)]):
        B.empty("Light_%d" % i, p)
    B.empty("CamAnchor", (-8.2, -10.2, 5.6))
    B.empty("CamLook", (1.2, 5.0, 2.4))
    return B


def build_cockpit(mats):
    B = Builder("cockpit", mats)

    def shell(bm):
        quad(bm, [(-1.4, -2.2, 0), (1.4, -2.2, 0), (1.4, 1.6, 0), (-1.4, 1.6, 0)], FLOOR, (0, 0, 1))
        quad(bm, [(-1.4, -2.2, 0), (1.4, -2.2, 0), (1.4, -2.2, 2.2), (-1.4, -2.2, 2.2)], WALL, (0, 1, 0))
        for sx in (-1, 1):
            quad(bm, [(sx * 1.4, -2.2, 0), (sx * 1.4, 1.6, 0), (sx * 1.4, 1.6, 0.9), (sx * 1.4, -2.2, 0.9)], WALL, (-sx, 0, 0))
    B.mesh("Shell", shell, recalc=False)

    def canopy(bm):
        # Wraparound canopy glass (the feed) over a frame of ribs.
        curved_screen(bm, (0, -0.6), 1.75, math.radians(38), math.radians(142), 0.75, 1.75, VIEW, n=32)
    B.mesh("ViewScreen", canopy, recalc=False)

    def frame(bm):
        for a in (38, 90, 142):
            r = math.radians(a)
            p = V((1.75 * math.cos(r), -0.6 + 1.75 * math.sin(r), 1.25))
            box(bm, p, (0.07, 0.07, 1.05), mat=TRIM)
        pts = [V((1.8 * math.cos(math.radians(a)), -0.6 + 1.8 * math.sin(math.radians(a)), 1.78)) for a in range(30, 151, 10)]
        for i in range(len(pts) - 1):
            cyl(bm, pts[i], pts[i + 1], 0.04, n=6, mat=TRIM)
        for sx in (-1, 1):
            cyl(bm, (sx * 1.25, -2.0, 1.95), (sx * 1.0, 0.9, 1.8), 0.045, n=6, mat=TRIM)
        # Dashboard: curved console with three displays and switch banks.
        box(bm, (0, 0.75, 0.45), (2.4, 0.7, 0.9), mat=CONSOLE)
        tilt = __import__('mathutils').Matrix.Rotation(math.radians(-35), 3, 'X')
        box(bm, (0, 0.55, 0.92), (2.3, 0.6, 0.06), mat=CONSOLE, rot=tilt)
        for k, x in enumerate((-0.7, 0, 0.7)):
            box(bm, (x, 0.53, 0.955), (0.55, 0.42, 0.012), mat=AMBER if k == 1 else SCREEN, rot=tilt)
        for x in (-1.05, 1.05):
            for k in range(4):
                box(bm, (x, 0.35 + k * 0.08, 0.86 + k * 0.05), (0.18, 0.05, 0.02), mat=LIGHT if k % 2 else ALERT)
        # Side consoles and throttle / stick.
        for sx in (-1, 1):
            box(bm, (sx * 1.1, -0.6, 0.5), (0.45, 1.6, 1.0), mat=CONSOLE)
            box(bm, (sx * 1.1, -0.35, 1.005), (0.28, 0.45, 0.012), mat=SCREEN)
            box(bm, (sx * 1.1, -0.95, 1.005), (0.28, 0.3, 0.012), mat=AMBER)
        cyl(bm, (0.45, -0.25, 0.55), (0.45, -0.2, 0.85), 0.03, n=8, mat=TRIM)
        sphere(bm, (0.45, -0.2, 0.88), 0.05, mat=OFFICER, seg=8, rings=5)
    B.mesh("Cockpit", frame)

    def pilot(bm):
        chair(bm, (0, -0.95, 0.1))
        figure(bm, (0, -1.1, 0.1), seated=True, mats=(UNIFORM, SKIN, HAIR))
        # Helmet over the head.
        sphere(bm, (0, -1.09, 1.48), 0.16, mat=SILVER, seg=14, rings=8, scale=(1.0, 1.05, 1.0))
    B.mesh("Pilot", pilot, smooth=True)

    holo_pivot = B.empty("Holo", (0, 0.65, 1.15))
    B.mesh("HoloProjection", lambda bm: lathe(bm, (0, 0, 0), (0, 0, 1), [(0.02, 0), (0.22, 0.12), (0.22, 0.13), (0.02, 0.01)], n=24, mat=HOLO, cap_start=False, cap_end=False), recalc=False).parent = holo_pivot
    B.mesh("Alert_D", lambda bm: box(bm, (0, 1.05, 0.93), (1.6, 0.04, 0.03), mat=ALERT))
    B.empty("Light_0", (0, -0.6, 2.0))
    B.empty("CamAnchor", (0.95, -1.75, 2.05))
    B.empty("CamLook", (-0.25, 1.6, 0.95))
    return B


def preview(B, path, mats):
    """Cycles render from the panel camera with a stand-in starfield on the screen."""
    sc = bpy.context.scene
    S.setup_world()
    anchor = next(e for e in B.empties if e.name == "CamAnchor")
    look = next(e for e in B.empties if e.name == "CamLook")
    cd = bpy.data.cameras.new("PanelCam")
    cd.lens = 18 if B.name == "bridge" else 16
    cam = bpy.data.objects.new("PanelCam", cd)
    sc.collection.objects.link(cam)
    cam.location = anchor.location
    d = (look.location - anchor.location).normalized()
    cam.rotation_euler = d.to_track_quat('-Z', 'Y').to_euler()
    sc.camera = cam
    for e in B.empties:
        if e.name.startswith("Light_"):
            ld = bpy.data.lights.new(e.name + "_L", 'POINT')
            ld.energy = 700 if B.name == "bridge" else 40
            ld.color = (1.0, 0.92, 0.82)
            lo = bpy.data.objects.new(e.name + "_L", ld)
            lo.location = e.location
            sc.collection.objects.link(lo)
    # Stand-in feed: deep blue gradient with a bright nebula smear.
    vs = mats[VIEW]
    nt = vs.node_tree
    b = nt.nodes.get("Principled BSDF")
    tex = nt.nodes.new("ShaderNodeTexNoise")
    tex.inputs["Scale"].default_value = 6.0
    ramp = nt.nodes.new("ShaderNodeValToRGB")
    ramp.color_ramp.elements[0].color = (0.01, 0.02, 0.05, 1)
    ramp.color_ramp.elements[1].color = (0.5, 0.35, 0.9, 1)
    nt.links.new(tex.outputs["Fac"], ramp.inputs["Fac"])
    nt.links.new(ramp.outputs["Color"], b.inputs["Emission Color"])
    b.inputs["Emission Strength"].default_value = 2.0
    try:
        sc.view_settings.view_transform = 'AgX'
    except Exception:
        pass
    S.render(path, 1280, 720, samples=64)
    nt.links.clear()


def main():
    argv = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
    for build, out in ((build_bridge, "bridge_interior"), (build_cockpit, "cockpit_interior")):
        S.reset_scene()
        mats = make_materials()
        B = build(mats)
        S.export_glb(os.path.join(OUT, out + ".glb"), B.objs, B.empties)
        if "--no-preview" not in argv:
            os.makedirs(PREV, exist_ok=True)
            preview(B, os.path.join(PREV, out + ".png"), mats)


main()

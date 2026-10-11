"""v3 preview scenes (2nd pass): hero shot, craft close-ups, faction comparison, battleship detail, station scale.

STAGING, NOT GAMEPLAY LAYOUT: Vela is scaled up x6 and the fleets are scene dressing. In game, Vela keeps
visual_radius_m 900 at its data position and there are no capital ships (01_art_direction.md 11.5).

Lighting rig (all shots): warm key from the star side, a strong star rim light from behind the subject so
gold/silver trims catch highlights, nebula sky as fill so the shadow side keeps its form, exposure -0.3,
Cycles + OpenImageDenoise.

Run (after logh_ships.py, station_v3.py, planet_star.py, skybox):
      blender --background --factory-startup --python art/scripts/scene_hero_v3.py
      SHOTS=hero  (comma list: hero,lancet,compare,battleship,station; default all)
Out:  art/previews/v3_hero.png, v3_lancet_close_{0,1,2}.png, v3_faction_compare.png,
      v3_bs_imperial_detail.png, v3_station_scale.png
"""
import math
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import bpy
import bmesh
from mathutils import Vector, Matrix

import common as C

SAMPLES = int(os.environ.get("HERO_SAMPLES", "384"))


def append(blend, names):
    path = os.path.join(C.SOURCE, blend)
    with bpy.data.libraries.load(path, link=False) as (src, dst):
        dst.objects = [n for n in src.objects if n in names]
    out = {}
    for ob in dst.objects:
        bpy.context.scene.collection.objects.link(ob)
        ob.parent = None
        ob.hide_render = False
        out[ob.name] = ob
    return out


def instance(src, name, loc=(0, 0, 0)):
    ob = bpy.data.objects.new(name, src.data)
    bpy.context.scene.collection.objects.link(ob)
    ob.location = loc
    return ob


def nose_to(ob, direction, bank_deg=0.0):
    """Models face -Y: point the nose along `direction`, then bank about the ship's long axis."""
    q = (-Vector(direction)).normalized().to_track_quat("Y", "Z")
    ob.rotation_euler = (q.to_matrix() @ Matrix.Rotation(math.radians(bank_deg), 3, "Y")).to_euler()


def sun(name, travel, strength, hexcol, angle_deg=0.5):
    d = bpy.data.lights.new(name, "SUN")
    d.energy = strength
    d.color = C.srgb(hexcol)[:3]
    d.angle = math.radians(angle_deg)
    ob = bpy.data.objects.new(name, d)
    ob.rotation_euler = Vector(travel).normalized().to_track_quat("-Z", "Y").to_euler()
    bpy.context.scene.collection.objects.link(ob)
    return ob


def rig(cam_loc, focus, key=3.2, rim=7.5, key_yaw=55, rim_yaw=-30):
    for ob in [o for o in bpy.data.objects if o.type == "LIGHT"]:
        bpy.data.objects.remove(ob)
    view = (Vector(focus) - Vector(cam_loc)).normalized()
    kd = Matrix.Rotation(math.radians(key_yaw), 3, "Z") @ view
    kd.z -= 0.55
    sun("Key", kd, key, "#FFE6C0")
    rd = Matrix.Rotation(math.radians(rim_yaw), 3, "Z") @ (-view)  # from behind the subject toward the camera
    rd.z -= 0.35
    sun("Rim", rd, rim, "#FFF1DC", angle_deg=0.3)


def plume(mw, local, r, length, hexcol="#7CC6FF", strength=1.6):
    """Additive engine plume (stills only; in Unity this is the engine VFX). Bright at the nozzle, fading out."""
    bm = bmesh.new()
    vs = bmesh.ops.create_cone(bm, cap_ends=False, segments=24, radius1=r, radius2=r * 0.25, depth=length)["verts"]
    bmesh.ops.rotate(bm, verts=vs, cent=(0, 0, 0), matrix=Matrix.Rotation(math.radians(-90), 3, "X"))
    bmesh.ops.translate(bm, verts=vs, vec=(local[0], local[1] + length / 2, local[2]))
    me = bpy.data.meshes.new("plume")
    bm.to_mesh(me)
    ob = bpy.data.objects.new("plume", me)
    bpy.context.scene.collection.objects.link(ob)
    ob.matrix_world = mw
    m = bpy.data.materials.new("Plume")
    m.use_nodes = True
    nt = m.node_tree
    for n in list(nt.nodes):
        if n.type == "BSDF_PRINCIPLED":
            nt.nodes.remove(n)
    tc = nt.nodes.new("ShaderNodeTexCoord")
    sep = nt.nodes.new("ShaderNodeSeparateXYZ")
    nt.links.new(tc.outputs["Generated"], sep.inputs[0])
    fade = nt.nodes.new("ShaderNodeMapRange")  # generated Y: 0 at the nozzle -> 1 at the tip
    nt.links.new(sep.outputs["Y"], fade.inputs["Value"])
    fade.inputs["To Min"].default_value = 1.0
    fade.inputs["To Max"].default_value = 0.0
    lw = nt.nodes.new("ShaderNodeLayerWeight")
    lw.inputs["Blend"].default_value = 0.45
    core = nt.nodes.new("ShaderNodeMath")
    core.operation = "POWER"
    nt.links.new(lw.outputs["Facing"], core.inputs[0])
    core.inputs[1].default_value = 2.0
    mul = nt.nodes.new("ShaderNodeMath")
    mul.operation = "MULTIPLY"
    nt.links.new(core.outputs[0], mul.inputs[0])
    nt.links.new(fade.outputs[0], mul.inputs[1])
    mul2 = nt.nodes.new("ShaderNodeMath")
    mul2.operation = "MULTIPLY"
    nt.links.new(mul.outputs[0], mul2.inputs[0])
    mul2.inputs[1].default_value = strength
    e = nt.nodes.new("ShaderNodeEmission")
    e.inputs["Color"].default_value = C.srgb(hexcol)
    nt.links.new(mul2.outputs[0], e.inputs["Strength"])
    tr = nt.nodes.new("ShaderNodeBsdfTransparent")
    add = nt.nodes.new("ShaderNodeAddShader")
    nt.links.new(e.outputs[0], add.inputs[0])
    nt.links.new(tr.outputs[0], add.inputs[1])
    nt.links.new(add.outputs[0], nt.nodes["Material Output"].inputs["Surface"])
    me.materials.append(m)
    return ob


def lancet_plumes(lancet):
    bpy.context.view_layer.update()
    out = [plume(lancet.matrix_world, (0, 11.15, 0), 0.75, 7.0)]
    for x in (1.25, -1.25):
        out.append(plume(lancet.matrix_world, (x, 10.25, -0.35), 0.24, 2.6))
    return out


def planet_fill(objs, cam_loc):
    """Nebula-coloured bounce light that only the planet receives (Cycles light linking), so its night
    side keeps cloud and ice forms instead of going black."""
    coll = bpy.data.collections.new("PlanetOnly")
    bpy.context.scene.collection.children.link(coll)
    for ob in objs:
        coll.objects.link(ob)
    center = objs[0].matrix_world.translation
    light = sun("PlanetFill", (center - Vector(cam_loc)).normalized() + Vector((0.25, 0, -0.2)), 1.3, "#86A8E0",
                angle_deg=8)
    light.light_linking.receiver_collection = coll
    return light


def setup():
    C.reset_scene()
    sc = C.setup_cycles(samples=SAMPLES, res=(2560, 1440))
    sc.view_settings.exposure = -0.3
    C.world_space("#04060C", 1.35, hdri=os.path.join(C.TEX, "skybox", "cradle_sky_equirect.png"))
    return sc


def camera(loc, target, lens):
    cam = C.add_camera(loc, target, lens=lens, clip_end=300000)
    cam.data.clip_start = 0.2
    return cam


def load_assets():
    ships = append("logh_ships.blend", {"ship_lancet_imperial_LOD0", "ship_cutter_alliance_LOD0", "bg_bs_imperial",
                                        "bg_fs_imperial", "bg_cr_imperial", "bg_bs_alliance",
                                        "ship_lancet_imperial_prospector_LOD0", "ship_lancet_imperial_courier_LOD0",
                                        "ship_cutter_alliance_prospector_LOD0", "ship_cutter_alliance_courier_LOD0"})
    st = append("station_v3.blend", {"station_cradle_exchange_v3_LOD0"})
    pl = append("planet_star.blend", {"Vela", "VelaClouds", "VelaAtmo", "VelaRing"})
    return ships, st, pl


def hide_all_but(keep):
    keep = set(keep)
    for ob in bpy.data.objects:
        if ob.type == "MESH":
            ob.hide_render = ob not in keep


# ----------------------------------------------------------------- shots

def shot_hero(A):
    ships, st, pl = A
    station = st["station_cradle_exchange_v3_LOD0"]
    lancet = ships["ship_lancet_imperial_LOD0"]
    keep = [station, lancet] + list(pl.values())
    for ob in pl.values():
        ob.location = (16500, 30000, 7000)
        ob.scale = (6, 6, 6)
        ob.rotation_euler = (math.radians(20), math.radians(-25), math.radians(70))
    station.location = (-470, 1300, -30)
    station.rotation_euler = (0, 0, math.radians(20))
    # small craft docked and arriving: they give the cathedral its size
    docks = [Vector((106, 0, -4)), Vector((0, 106, -4)), Vector((-106, 0, -4)), Vector((0, -106, -4))]
    rot = Matrix.Rotation(math.radians(20), 3, "Z")
    for i, d in enumerate(docks):
        p = station.location + rot @ d
        sh = instance(ships["ship_cutter_alliance_LOD0"] if i % 2 else lancet, f"docked_{i}", p)
        nose_to(sh, (rot @ d).normalized() * -1 + Vector((0, 0, 0.0)))
        keep.append(sh)
    for i, (off, dirv) in enumerate(((Vector((180, -60, 40)), (-0.8, 0.5, -0.1)),
                                     (Vector((240, 120, 10)), (-0.9, -0.3, 0.0)))):
        sh = instance(lancet, f"arriving_{i}", station.location + off)
        nose_to(sh, dirv, bank_deg=15)
        keep.append(sh)
    # player lancet, foreground right, 3/4 front (nose toward camera-left)
    lancet.location = (8.5, 40, -2.4)
    nose_to(lancet, (-0.45, -0.89, 0.1), bank_deg=10)
    keep += lancet_plumes(lancet)
    # fleet: hero battleship close enough to read, flagship leading, trapezoid of battleships and cruisers
    fwd = Vector((-0.5, -0.86, -0.04)).normalized()
    right = Vector((0, 0, 1)).cross(fwd).normalized()
    hero = ships["bg_bs_imperial"]
    hero.location = (520, 1650, 230)
    nose_to(hero, fwd)
    keep.append(hero)
    flag = ships["bg_fs_imperial"]
    tip = Vector((-150, 3900, 520))
    flag.location = tip
    nose_to(flag, fwd)
    keep.append(flag)
    rows = [(1, 2, "bg_bs_imperial", 900), (2, 4, "bg_bs_imperial", 1000), (3, 6, "bg_cr_imperial", 950)]
    for r, n, src, spacing in rows:
        for j in range(n):
            s_ = (j - (n - 1) / 2)
            p = tip - fwd * (r * 1500) + right * (s_ * spacing) + Vector((0, 0, -r * 120))
            ob = instance(ships[src], f"fleet_{r}_{j}", p)
            nose_to(ob, fwd)
            keep.append(ob)
    for i in range(5):
        ob = instance(ships["bg_bs_alliance"], f"alliance_{i}",
                      (-9000 - i * 800, 19000 + i * 700, 2600 - i * 200))
        nose_to(ob, (0.9, -0.25, 0.0))
        keep.append(ob)
    hide_all_but(keep)
    cam = camera((0, 0, 2), (150, 2400, 230), 30)
    rig(cam.location, (300, 2000, 100), key=3.0, rim=8.0, key_yaw=60, rim_yaw=-35)
    planet_fill(list(pl.values()), cam.location)
    C.render(os.path.join(C.PREVIEWS, "v3_hero.png"))


def shot_lancet(A):
    ships, st, pl = A
    lancet = ships["ship_lancet_imperial_LOD0"]
    lancet.location = (0, 0, 0)
    lancet.rotation_euler = (0, 0, 0)
    keep = [lancet] + lancet_plumes(lancet)
    hide_all_but(keep)
    for i, (loc, tgt, lens) in enumerate((((-14, -19, 5.5), (0, -1.5, 0), 40),   # 3/4 front: prow + canopy
                                          ((13, 18, 6), (0, 2, 0), 38),          # 3/4 rear: engines, fins
                                          ((7.5, -6, 2.6), (2.5, 2.0, 0.3), 30))):  # wing + canopy detail
        for ob in [o for o in bpy.data.objects if o.type == "CAMERA"]:
            bpy.data.objects.remove(ob)
        cam = camera(loc, tgt, lens)
        rig(cam.location, tgt, key=3.2, rim=7.0)
        C.render(os.path.join(C.PREVIEWS, f"v3_lancet_close_{i}.png"))


def shot_compare(A):
    ships, st, pl = A
    li, ca = ships["ship_lancet_imperial_LOD0"], ships["ship_cutter_alliance_LOD0"]
    bi, ba = ships["bg_bs_imperial"], ships["bg_bs_alliance"]
    li.location, ca.location = (-8, 0, 0), (8, 2, 0)
    nose_to(li, (0.25, -1, 0.05), bank_deg=-8)
    nose_to(ca, (-0.25, -1, 0.05), bank_deg=8)
    bi.location = (-700, 1700, 150)
    ba.location = (760, 1900, 60)
    nose_to(bi, (0.95, -0.3, 0))
    nose_to(ba, (-0.95, -0.3, 0))
    hide_all_but([li, ca, bi, ba])
    cam = camera((0, -30, 4), (0, 60, 8), 32)
    rig(cam.location, (0, 0, 0), key=3.0, rim=7.5, key_yaw=40, rim_yaw=-20)
    C.render(os.path.join(C.PREVIEWS, "v3_faction_compare.png"))


def shot_battleship(A):
    ships, st, pl = A
    bi = ships["bg_bs_imperial"]
    bi.location = (0, 0, 0)
    bi.rotation_euler = (0, 0, 0)
    hide_all_but([bi])
    cam = camera((-260, -300, 95), (0, -60, 10), 35)
    rig(cam.location, (0, 0, 0), key=3.0, rim=8.0, key_yaw=50, rim_yaw=-25)
    C.render(os.path.join(C.PREVIEWS, "v3_bs_imperial_detail.png"))


def shot_station(A):
    ships, st, pl = A
    station = st["station_cradle_exchange_v3_LOD0"]
    station.location = (0, 0, 0)
    station.rotation_euler = (0, 0, 0)
    lancet = ships["ship_lancet_imperial_LOD0"]
    keep = [station]
    for i, a in enumerate((0, 90, 180, 270)):
        d = Matrix.Rotation(math.radians(a), 3, "Z") @ Vector((106, 0, -4))
        sh = instance(ships["ship_cutter_alliance_LOD0"] if i % 2 else lancet, f"dock_{i}", d)
        nose_to(sh, -d.normalized())
        keep.append(sh)
    for i, (p, dv) in enumerate(((Vector((150, -110, 30)), (-0.6, 0.8, -0.1)), (Vector((60, -180, -40)), (0.2, 1, 0.1)),
                                 (Vector((-40, -150, 70)), (0.5, 0.8, -0.2)))):
        sh = instance(lancet, f"traffic_{i}", p)
        nose_to(sh, dv, bank_deg=10)
        keep.append(sh)
    hide_all_but(keep)
    cam = camera((210, -330, 30), (0, 0, -5), 30)
    rig(cam.location, (0, 0, 0), key=3.0, rim=8.0, key_yaw=45, rim_yaw=-30)
    C.render(os.path.join(C.PREVIEWS, "v3_station_scale.png"))


def shot_kits(A):
    """Module kits on both hulls: rows = Imperial / Alliance, columns = standard / prospector / courier."""
    ships, st, pl = A
    names = [["ship_lancet_imperial_LOD0", "ship_lancet_imperial_prospector_LOD0", "ship_lancet_imperial_courier_LOD0"],
             ["ship_cutter_alliance_LOD0", "ship_cutter_alliance_prospector_LOD0", "ship_cutter_alliance_courier_LOD0"]]
    keep = []
    for r, row in enumerate(names):
        for c, n in enumerate(row):
            ob = ships[n]
            ob.location = ((c - 1) * 27, 0, -r * 15)
            nose_to(ob, (-1, -0.42, 0.0), bank_deg=-18)  # broadside-ish, rolled to show belly kits and dorsal cargo
            keep.append(ob)
    hide_all_but(keep)
    cam = camera((-6, -82, -1), (0, 0, -7.5), 40)
    rig(cam.location, (0, 0, -7), key=3.0, rim=7.0, key_yaw=45, rim_yaw=-25)
    C.render(os.path.join(C.PREVIEWS, "v3_kits.png"))


SHOTS = {"kits": shot_kits, "hero": shot_hero, "lancet": shot_lancet, "compare": shot_compare, "battleship": shot_battleship,
         "station": shot_station}


def main():
    want = os.environ.get("SHOTS", ",".join(SHOTS)).split(",")
    for name in want:
        setup()
        A = load_assets()
        SHOTS[name](A)


if __name__ == "__main__":
    main()

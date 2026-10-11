"""Mining equipment VFX source + in-scale composite previews (assets appended from the other .blend files).

Run (after ship_scout.py, asteroids.py, station.py, planet_star.py):
      blender --background --factory-startup --python art/scripts/scene_mining.py
Out:  art/textures/vfx/mining_beam_gradient.png  (256 x 64 RGBA: U along the beam, V across - LineRenderer)
      art/textures/vfx/mining_impact.png         (256 x 256 RGBA additive sprite)
      art/previews/scene_mining_glacine.png      Prospector mining a revealed deposit, true scale
      art/previews/scene_overview.png            deposit + ships + station + Vela, true scale

Scale (data): deposit radius 60 m (inner-belt-1), mining range = radius + 150 m from the surface
(data/mining/mining-rules.json mining_range_from_surface_m), Scout length 20 m.
"""
import math
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import bpy
import bmesh
import numpy as np
from mathutils import Vector, Matrix

import common as C

VFX = os.path.join(C.TEX, "vfx")
MINING_RANGE_FROM_SURFACE_M = 150.0


def append(blend, names):
    path = os.path.join(C.SOURCE, blend)
    with bpy.data.libraries.load(path, link=False) as (src, dst):
        dst.objects = [n for n in src.objects if n in names]
    out = {}
    for ob in dst.objects:
        bpy.context.scene.collection.objects.link(ob)
        out[ob.name] = ob
    return out


def beam_gradient(w=256, h=64):
    u = np.linspace(0, 1, w, dtype=np.float32)[None, :]
    v = np.linspace(-1, 1, h, dtype=np.float32)[:, None]
    core = np.exp(-(v / 0.18) ** 2)
    halo = np.exp(-(v / 0.6) ** 2) * 0.45
    pulse = 0.85 + 0.15 * np.cos(u * math.pi * 12)  # scrolls in the shader
    a = np.clip((core + halo) * pulse, 0, 1)
    a *= np.clip(u / 0.04, 0, 1)
    r = 0.55 + 0.45 * core
    g = 0.92 + 0.08 * core
    b = np.ones_like(core)
    img = np.stack([r * np.ones_like(a), g * np.ones_like(a), b * np.ones_like(a), a], axis=-1)
    return img


def impact_sprite(n=256):
    yy, xx = np.mgrid[0:n, 0:n].astype(np.float32)
    c = (n - 1) / 2
    r = np.sqrt((xx - c) ** 2 + (yy - c) ** 2) / c
    ang = np.arctan2(yy - c, xx - c)
    a = np.exp(-(r / 0.22) ** 2) + 0.35 * np.exp(-(r / 0.6) ** 2) * (0.7 + 0.3 * np.cos(ang * 9))
    a = np.clip(a, 0, 1) * np.clip((1 - r) / 0.2, 0, 1)
    return np.stack([np.full_like(a, 0.75), np.full_like(a, 0.97), np.ones_like(a), a], axis=-1)


def save_rgba(arr, name, path):
    h, w, _ = arr.shape
    img = bpy.data.images.new(name, w, h, alpha=True)
    img.pixels = arr[::-1].ravel().tolist()
    img.filepath_raw = path
    img.file_format = "PNG"
    img.save()
    print("SAVED_TEX", path)
    return img


def beam_object(p0, p1, grad_img):
    """Two crossed quads along the beam (preview stand-in for the LineRenderer)."""
    d = (p1 - p0)
    L_ = d.length
    z = d.normalized()
    up = Vector((0, 0, 1)) if abs(z.z) < 0.9 else Vector((1, 0, 0))
    x = up.cross(z).normalized()
    y = z.cross(x)
    bm = bmesh.new()
    uv = bm.loops.layers.uv.new("UVMap")
    for side in (x, y):
        w = side * 1.6
        vs = [bm.verts.new(p0 - w), bm.verts.new(p0 + w), bm.verts.new(p1 + w), bm.verts.new(p1 - w)]
        f = bm.faces.new(vs)
        for l, (uu, vv) in zip(f.loops, ((0, 0), (0, 1), (1, 1), (1, 0))):
            l[uv].uv = (uu, vv)
    me = bpy.data.meshes.new("beam")
    bm.to_mesh(me)
    ob = bpy.data.objects.new("MiningBeam", me)
    bpy.context.scene.collection.objects.link(ob)
    m = bpy.data.materials.new("MiningBeam")
    m.use_nodes = True
    nt = m.node_tree
    for n in list(nt.nodes):
        if n.type == "BSDF_PRINCIPLED":
            nt.nodes.remove(n)
    t = nt.nodes.new("ShaderNodeTexImage")
    t.image = grad_img
    e = nt.nodes.new("ShaderNodeEmission")
    e.inputs["Color"].default_value = C.srgb("#5FF0FF")
    mul = nt.nodes.new("ShaderNodeMath")
    mul.operation = "MULTIPLY"
    nt.links.new(t.outputs["Alpha"], mul.inputs[0])
    mul.inputs[1].default_value = 14.0
    nt.links.new(mul.outputs[0], e.inputs["Strength"])
    tr = nt.nodes.new("ShaderNodeBsdfTransparent")
    add = nt.nodes.new("ShaderNodeAddShader")
    nt.links.new(e.outputs[0], add.inputs[0])
    nt.links.new(tr.outputs[0], add.inputs[1])
    nt.links.new(add.outputs[0], nt.nodes["Material Output"].inputs["Surface"])
    me.materials.append(m)
    # impact glow: small emissive sphere + a point light so the rock is lit by the beam
    bpy.ops.mesh.primitive_uv_sphere_add(radius=1.4, location=p1)
    s = bpy.context.active_object
    s.data.materials.append(C.mat_emit("Impact", "#9FF6FF", 30.0))
    ld = bpy.data.lights.new("ImpactLight", "POINT")
    ld.energy = 3.0e4
    ld.color = C.srgb("#7FF0FF")[:3]
    lo = bpy.data.objects.new("ImpactLight", ld)
    lo.location = p1 + (p0 - p1).normalized() * 3.0
    bpy.context.scene.collection.objects.link(lo)
    return ob


def main():
    os.makedirs(VFX, exist_ok=True)
    C.reset_scene()
    C.setup_cycles(samples=192, res=(1920, 1080))
    sky = os.path.join(C.TEX, "skybox", "cradle_sky_equirect.png")
    C.world_space("#04060C", 1.0, hdri=sky if os.path.exists(sky) else None)
    C.studio_lights()
    grad = save_rgba(beam_gradient(), "mining_beam_gradient", os.path.join(VFX, "mining_beam_gradient.png"))
    save_rgba(impact_sprite(), "mining_impact", os.path.join(VFX, "mining_impact.png"))

    R = 60.0
    rock = append("asteroids.blend", {"asteroid_rock_1_LOD0", "asteroid_rock_1_sat0", "asteroid_rock_1_sat1",
                                      "asteroid_rock_1_sat2", "crystal_glacine"})
    rk = rock["asteroid_rock_1_LOD0"]
    rk.parent = None
    rk.matrix_world = Matrix.Scale(R, 4)
    k = 0
    for n, ob in rock.items():
        if "_sat" in n:
            ob.parent = None
            ob.location = ob.location * R
            ob.scale = ob.scale * R
    gl = rock["crystal_glacine"]
    # put 4 clusters on the side facing the ship using the rock's sockets is a Unity job; here: ray cast
    bm = bmesh.new()
    bm.from_mesh(rk.data)
    from mathutils.bvhtree import BVHTree
    tree = BVHTree.FromBMesh(bm)
    bm.free()
    ship = append("ship_scout.blend", {"ship_scout_prospector_LOD0"})["ship_scout_prospector_LOD0"]
    ship.parent = None
    # ship 105 m above the surface (inside the 150 m mining range), nose toward the rock
    ship_pos = Vector((-40.0, -150.0, 25.0))
    ship.location = ship_pos
    to_rock = (-ship_pos).normalized()
    ship.rotation_euler = (-to_rock).to_track_quat("Y", "Z").to_euler()  # nose = -Y
    bpy.context.view_layer.update()
    for i, dv in enumerate((Vector((-0.3, -1, 0.25)), Vector((0.2, -1, 0.5)), Vector((-0.55, -0.8, -0.2)),
                            Vector((0.45, -0.85, -0.1)))):
        dv.normalize()
        loc, nor, _, _ = tree.ray_cast(dv * 3.0, -dv, 6.0)
        o = bpy.data.objects.new(f"glacine_{i}", gl.data)
        bpy.context.scene.collection.objects.link(o)
        z = nor.normalized()
        up = Vector((0, 0, 1)) if abs(z.z) < 0.95 else Vector((1, 0, 0))
        x = up.cross(z).normalized()
        y = z.cross(x)
        o.matrix_world = Matrix.Scale(R, 4) @ Matrix.Translation(loc) @ Matrix((x, y, z)).transposed().to_4x4() \
            @ Matrix.Translation((0, 0, -0.03)) @ Matrix.Scale(1.0 - 0.1 * i, 4)
    gl.hide_render = True
    # beam from the laser socket (prospector Socket_Mining_0 = (0, -4.3, -2.05) in ship space)
    p0 = ship.matrix_world @ Vector((0, -4.3, -2.05))
    hit, _, _, _ = tree.ray_cast((p0 / R), (-(p0 / R)).normalized(), 10.0)
    p1 = hit * R
    beam_object(p0, p1, grad)
    surface_dist = (ship_pos.length - R)
    print("SCENE ship-surface distance ~%.0f m (range %.0f m)" % (surface_dist, MINING_RANGE_FROM_SURFACE_M))
    cam = C.add_camera((0, 0, 0), (0, 0, 0), lens=35, clip_end=100000)
    cam.location = ship_pos + Vector((-22, -42, 16))
    C.look_at(cam, Vector((-10, -40, 5)))
    C.render(os.path.join(C.PREVIEWS, "scene_mining_glacine.png"))
    # overview: Vela + station far away, a second ship passing
    pl = append("planet_star.blend", {"Vela", "VelaClouds", "VelaAtmo"})
    for ob in pl.values():
        ob.location = Vector((3000, -200 - 0, -900)) - Vector((-3200, 150, -2400))  # inner-belt-1 frame
        ob.location = Vector((ob.location.x, ob.location.z * -1, ob.location.y))  # data (Y up) -> Blender (Z up)
        ob.hide_render = False
    st = append("station.blend", {"Station_Static_LOD0", "Station_Ring_LOD0"})
    sp = Vector((0.0, 1500.0, 0.0)) - Vector((-3200, 150, -2400))
    for ob in st.values():
        ob.parent = None
        ob.location = Vector((sp.x, -sp.z, sp.y))
    cam.location = Vector((-330, 120, 30))
    C.look_at(cam, Vector((600, -150, -30)))
    cam.data.lens = 30
    C.render(os.path.join(C.PREVIEWS, "scene_overview.png"))


if __name__ == "__main__":
    main()

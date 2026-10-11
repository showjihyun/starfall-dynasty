"""Shared helpers for STARFALL art generation scripts (Blender 5.2, bpy).

Conventions (01_art_direction.md):
- 1 Blender unit = 1 m. Models face Blender -Y (nose) with +Z up; FBX export with
  forward=-Z / up=Y and bake_space_transform turns that into Unity +Z forward, +Y up.
- Every random choice goes through a seeded `random.Random` so a rerun is identical.
"""
import math
import os
import random

import bpy
import bmesh
from mathutils import Vector, Matrix

ART = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
TEX = os.path.join(ART, "textures")
PH = os.path.join(TEX, "polyhaven")
PREVIEWS = os.path.join(ART, "previews")
EXPORT = os.path.join(ART, "export")
SOURCE = os.path.join(ART, "source")


def srgb(hexstr):
    """'#RRGGBB' -> linear RGBA tuple."""
    h = hexstr.lstrip("#")
    out = []
    for i in range(3):
        c = int(h[i * 2:i * 2 + 2], 16) / 255.0
        out.append(c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4)
    return (out[0], out[1], out[2], 1.0)


# ----------------------------------------------------------------- scene

def reset_scene():
    bpy.ops.wm.read_factory_settings(use_empty=True)
    scene = bpy.context.scene
    scene.unit_settings.system = "METRIC"
    scene.unit_settings.scale_length = 1.0
    return scene


def setup_cycles(samples=128, res=(1600, 1000), transparent=False):
    scene = bpy.context.scene
    scene.render.engine = "CYCLES"
    prefs = bpy.context.preferences.addons["cycles"].preferences
    prefs.compute_device_type = "OPTIX"
    prefs.get_devices()
    for d in prefs.devices:
        d.use = d.type == "OPTIX"
    scene.cycles.device = "GPU"
    scene.cycles.samples = samples
    scene.cycles.use_denoising = True
    scene.cycles.max_bounces = 6
    scene.render.resolution_x, scene.render.resolution_y = res
    scene.render.resolution_percentage = 100
    scene.render.film_transparent = transparent
    scene.view_settings.view_transform = "AgX"
    scene.view_settings.look = "AgX - Medium High Contrast"
    scene.render.image_settings.file_format = "PNG"
    return scene


def world_space(color="#04060C", strength=1.0, hdri=None):
    w = bpy.data.worlds.new("World")
    bpy.context.scene.world = w
    w.use_nodes = True
    nt = w.node_tree
    bg = nt.nodes["Background"]
    if hdri and os.path.exists(hdri):
        env = nt.nodes.new("ShaderNodeTexEnvironment")
        env.image = bpy.data.images.load(hdri)
        nt.links.new(env.outputs["Color"], bg.inputs["Color"])
    else:
        bg.inputs["Color"].default_value = srgb(color)
    bg.inputs["Strength"].default_value = strength
    return w


def add_sun(name, rot_deg, strength, color="#FFE6C0", angle_deg=0.6):
    data = bpy.data.lights.new(name, "SUN")
    data.energy = strength
    data.color = srgb(color)[:3]
    data.angle = math.radians(angle_deg)
    ob = bpy.data.objects.new(name, data)
    ob.rotation_euler = [math.radians(a) for a in rot_deg]
    bpy.context.scene.collection.objects.link(ob)
    return ob


def add_area(name, loc, target, power, color="#7FA8FF", size=10.0):
    data = bpy.data.lights.new(name, "AREA")
    data.energy = power
    data.size = size
    data.color = srgb(color)[:3]
    ob = bpy.data.objects.new(name, data)
    ob.location = loc
    bpy.context.scene.collection.objects.link(ob)
    look_at(ob, target)
    return ob


def look_at(ob, target):
    d = Vector(target) - ob.location
    ob.rotation_euler = d.to_track_quat("-Z", "Y").to_euler()


def add_camera(loc, target, lens=50.0, clip_end=100000.0):
    cam = bpy.data.cameras.new("PreviewCam")
    cam.lens = lens
    cam.clip_start = 0.1
    cam.clip_end = clip_end
    ob = bpy.data.objects.new("PreviewCam", cam)
    ob.location = loc
    bpy.context.scene.collection.objects.link(ob)
    look_at(ob, target)
    bpy.context.scene.camera = ob
    return ob


def studio_lights(scale=1.0, sun_rot=(52, 8, 128)):
    """Key = warm star light from one side, very dim cool fill, cool rim."""
    add_sun("Star", sun_rot, 4.5)
    add_sun("Rim", (-60, 0, -40), 0.35, color="#8FB4FF", angle_deg=4)
    add_sun("Fill", (110, 0, 230), 0.08, color="#5C7AB8", angle_deg=20)


def sun_dir():
    """Direction the 'Star' sun light travels (world)."""
    ob = bpy.data.objects["Star"]
    return (ob.matrix_world.to_3x3() @ Vector((0, 0, -1))).normalized()


def cam_lit(cam, target, dist, yaw_deg=35.0, pitch_deg=12.0, lens=None):
    """Place the camera on the lit side: looking roughly along the light, offset by yaw/pitch."""
    d = -sun_dir()
    d = Matrix.Rotation(math.radians(yaw_deg), 3, "Z") @ d
    d.z += math.sin(math.radians(pitch_deg))
    d.normalize()
    cam.location = Vector(target) + d * dist
    look_at(cam, target)
    if lens:
        cam.data.lens = lens


def render(path, samples=None):
    scene = bpy.context.scene
    if samples:
        scene.cycles.samples = samples
    os.makedirs(os.path.dirname(path), exist_ok=True)
    scene.render.filepath = path
    bpy.ops.render.render(write_still=True)
    print("RENDERED", path)


# ----------------------------------------------------------------- materials

def _img(path, non_color=False):
    img = bpy.data.images.load(path, check_existing=True)
    if non_color:
        img.colorspace_settings.name = "Non-Color"
    return img


def _tex(nt, path, vec, non_color=False, x=-600, y=0):
    n = nt.nodes.new("ShaderNodeTexImage")
    n.image = _img(path, non_color)
    n.location = (x, y)
    nt.links.new(vec, n.inputs["Vector"])
    return n


def _mix_rgba_sockets(mix):
    a = [i for i in mix.inputs if i.name == "A" and i.type == "RGBA"][0]
    b = [i for i in mix.inputs if i.name == "B" and i.type == "RGBA"][0]
    o = [i for i in mix.outputs if i.type == "RGBA"][0]
    return a, b, o


def mat_painted(name, base_hex, metallic=0.15, rough=(0.32, 0.62), tex_set="blue_metal_plate",
                uv_scale=0.25, normal_strength=0.35, grime=0.18):
    """Painted hull panel: flat paint colour, roughness/normal detail from a CC0 metal set,
    grime from its AO/diffuse luminance. uv_scale = repeats per metre."""
    m = bpy.data.materials.new(name)
    m.use_nodes = True
    nt = m.node_tree
    bsdf = nt.nodes["Principled BSDF"]
    tc = nt.nodes.new("ShaderNodeTexCoord")
    mp = nt.nodes.new("ShaderNodeMapping")
    mp.inputs["Scale"].default_value = (uv_scale, uv_scale, uv_scale)
    nt.links.new(tc.outputs["UV"], mp.inputs["Vector"])
    v = mp.outputs["Vector"]
    diff = _tex(nt, os.path.join(PH, f"{tex_set}_diff_2k.jpg"), v, y=300)
    rgh = _tex(nt, os.path.join(PH, f"{tex_set}_rough_2k.jpg"), v, True, y=0)
    nor = _tex(nt, os.path.join(PH, f"{tex_set}_nor_gl_2k.jpg"), v, True, y=-300)
    # grime: luminance of diffuse -> darken paint a bit
    bw = nt.nodes.new("ShaderNodeRGBToBW")
    nt.links.new(diff.outputs["Color"], bw.inputs["Color"])
    ramp = nt.nodes.new("ShaderNodeValToRGB")
    ramp.color_ramp.elements[0].position = 0.15
    ramp.color_ramp.elements[0].color = (1 - grime, 1 - grime, 1 - grime, 1)
    ramp.color_ramp.elements[1].position = 0.6
    ramp.color_ramp.elements[1].color = (1, 1, 1, 1)
    nt.links.new(bw.outputs["Val"], ramp.inputs["Fac"])
    mix = nt.nodes.new("ShaderNodeMix")
    mix.data_type = "RGBA"
    mix.blend_type = "MULTIPLY"
    mix.inputs["Factor"].default_value = 1.0
    a_in, b_in, out = _mix_rgba_sockets(mix)
    a_in.default_value = srgb(base_hex)
    nt.links.new(ramp.outputs["Color"], b_in)
    nt.links.new(out, bsdf.inputs["Base Color"])
    mr = nt.nodes.new("ShaderNodeMapRange")
    mr.inputs["To Min"].default_value = rough[0]
    mr.inputs["To Max"].default_value = rough[1]
    nt.links.new(rgh.outputs["Color"], mr.inputs["Value"])
    nt.links.new(mr.outputs["Result"], bsdf.inputs["Roughness"])
    nm = nt.nodes.new("ShaderNodeNormalMap")
    nm.inputs["Strength"].default_value = normal_strength
    nt.links.new(nor.outputs["Color"], nm.inputs["Color"])
    nt.links.new(nm.outputs["Normal"], bsdf.inputs["Normal"])
    bsdf.inputs["Metallic"].default_value = metallic
    return m


def mat_simple(name, base_hex, metallic=0.0, rough=0.5, emission_hex=None, emission=0.0,
               transmission=0.0, ior=1.45, alpha=1.0, coat=0.0):
    m = bpy.data.materials.new(name)
    m.use_nodes = True
    b = m.node_tree.nodes["Principled BSDF"]
    b.inputs["Base Color"].default_value = srgb(base_hex)
    b.inputs["Metallic"].default_value = metallic
    b.inputs["Roughness"].default_value = rough
    b.inputs["IOR"].default_value = ior
    b.inputs["Transmission Weight"].default_value = transmission
    b.inputs["Coat Weight"].default_value = coat
    if emission_hex:
        b.inputs["Emission Color"].default_value = srgb(emission_hex)
        b.inputs["Emission Strength"].default_value = emission
    if alpha < 1.0:
        b.inputs["Alpha"].default_value = alpha
    return m


def mat_emit(name, hex_, strength):
    m = bpy.data.materials.new(name)
    m.use_nodes = True
    nt = m.node_tree
    for n in list(nt.nodes):
        if n.type == "BSDF_PRINCIPLED":
            nt.nodes.remove(n)
    e = nt.nodes.new("ShaderNodeEmission")
    e.inputs["Color"].default_value = srgb(hex_)
    e.inputs["Strength"].default_value = strength
    nt.links.new(e.outputs[0], nt.nodes["Material Output"].inputs["Surface"])
    return m


# ----------------------------------------------------------------- mesh helpers

def new_obj(name, bm, mats=None, collection=None):
    me = bpy.data.meshes.new(name)
    bm.to_mesh(me)
    bm.free()
    ob = bpy.data.objects.new(name, me)
    (collection or bpy.context.scene.collection).objects.link(ob)
    for m in mats or []:
        me.materials.append(m)
    return ob


def loft(sections, closed_ends=True, mat_index=0):
    """sections: list of lists of Vector, all same length, each a closed ring.
    Returns a bmesh with quads bridging consecutive rings."""
    bm = bmesh.new()
    rings = [[bm.verts.new(p) for p in ring] for ring in sections]
    n = len(sections[0])
    for a, b in zip(rings, rings[1:]):
        for i in range(n):
            f = bm.faces.new((a[i], a[(i + 1) % n], b[(i + 1) % n], b[i]))
            f.material_index = mat_index
    if closed_ends:
        f0 = bm.faces.new(list(reversed(rings[0])))
        f0.material_index = mat_index
        f1 = bm.faces.new(rings[-1])
        f1.material_index = mat_index
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
    return bm


def chamfer_rect(w, h, c, y, z0=0.0, x0=0.0, top_c=None):
    """Octagon-ish section in the XZ plane at Y=y. w,h = full width/height, c = chamfer."""
    tc = c if top_c is None else top_c
    hw, hh = w / 2, h / 2
    pts = [(-hw + c, -hh), (hw - c, -hh), (hw, -hh + c), (hw, hh - tc), (hw - tc, hh),
           (-hw + tc, hh), (-hw, hh - tc), (-hw, -hh + c)]
    return [Vector((x0 + x, y, z0 + z)) for x, z in pts]


def circle(r, y, n=24, x0=0.0, z0=0.0):
    return [Vector((x0 + r * math.cos(2 * math.pi * i / n), y, z0 + r * math.sin(2 * math.pi * i / n)))
            for i in range(n)]


def add_bevel(ob, width=0.05, segments=2, angle=35):
    mod = ob.modifiers.new("Bevel", "BEVEL")
    mod.width = width
    mod.segments = segments
    mod.limit_method = "ANGLE"
    mod.angle_limit = math.radians(angle)
    mod.harden_normals = True
    mod.miter_outer = "MITER_ARC"
    return mod


def add_weighted_normals(ob):
    mod = ob.modifiers.new("WN", "WEIGHTED_NORMAL")
    mod.keep_sharp = True
    return mod


def shade_smooth_by_angle(ob, angle=35):
    me = ob.data
    for p in me.polygons:
        p.use_smooth = True
    try:
        me.set_sharp_from_angle(angle=math.radians(angle))
    except AttributeError:
        pass


def uv_box(ob, scale=1.0):
    """World-scale cube projection-like UVs via Smart UV, then scale so 1 UV = 1 m * scale."""
    bpy.context.view_layer.objects.active = ob
    for o in bpy.context.selected_objects:
        o.select_set(False)
    ob.select_set(True)
    bpy.ops.object.mode_set(mode="EDIT")
    bpy.ops.mesh.select_all(action="SELECT")
    bpy.ops.uv.smart_project(angle_limit=math.radians(50), island_margin=0.0, scale_to_bounds=False,
                             correct_aspect=True)
    bpy.ops.object.mode_set(mode="OBJECT")
    # Smart UV normalises to 0..1 over the whole mesh; rescale to metres.
    me = ob.data
    uv = me.uv_layers.active.data
    # area ratio: sum face areas (world) vs uv area
    area3d = sum(p.area for p in me.polygons)
    area_uv = 0.0
    for p in me.polygons:
        pts = [uv[li].uv for li in p.loop_indices]
        a = 0.0
        for i in range(len(pts)):
            x1, y1 = pts[i]
            x2, y2 = pts[(i + 1) % len(pts)]
            a += x1 * y2 - x2 * y1
        area_uv += abs(a) / 2
    if area_uv > 0:
        k = math.sqrt(area3d / area_uv) * scale
        for d in uv:
            d.uv = d.uv * k


def apply_modifiers(ob):
    bpy.context.view_layer.objects.active = ob
    for m in list(ob.modifiers):
        bpy.ops.object.modifier_apply(modifier=m.name)


def join(objs, name):
    bpy.ops.object.select_all(action="DESELECT")
    for o in objs:
        o.select_set(True)
    bpy.context.view_layer.objects.active = objs[0]
    bpy.ops.object.join()
    objs[0].name = name
    objs[0].data.name = name
    return objs[0]


def add_empty(name, loc, parent=None, size=0.5, rot=(0, 0, 0)):
    e = bpy.data.objects.new(name, None)
    e.empty_display_size = size
    e.empty_display_type = "ARROWS"
    e.location = loc
    e.rotation_euler = rot
    bpy.context.scene.collection.objects.link(e)
    if parent:
        e.parent = parent
    return e


def tri_count(ob):
    deps = bpy.context.evaluated_depsgraph_get()
    ev = ob.evaluated_get(deps)
    me = ev.to_mesh()
    t = sum(len(p.vertices) - 2 for p in me.polygons)
    ev.to_mesh_clear()
    return t


def bounding_radius(objs, center=(0, 0, 0)):
    c = Vector(center)
    deps = bpy.context.evaluated_depsgraph_get()
    r = 0.0
    for ob in objs:
        if ob.type != "MESH":
            continue
        ev = ob.evaluated_get(deps)
        me = ev.to_mesh()
        mw = ob.matrix_world
        for v in me.vertices:
            r = max(r, ((mw @ v.co) - c).length)
        ev.to_mesh_clear()
    return r


def geo_digest(objs):
    """Order-independent digest of evaluated vertex positions (mm) - proves a rerun gives the same geometry
    even though the FBX file itself embeds a timestamp."""
    import hashlib
    deps = bpy.context.evaluated_depsgraph_get()
    h = hashlib.sha256()
    rows = []
    for ob in sorted(objs, key=lambda o: o.name):
        if ob.type != "MESH":
            continue
        ev = ob.evaluated_get(deps)
        me = ev.to_mesh()
        pts = sorted((round(v.co.x * 1000), round(v.co.y * 1000), round(v.co.z * 1000)) for v in me.vertices)
        rows.append((ob.name, len(me.polygons), pts))
        ev.to_mesh_clear()
    for name, nf, pts in rows:
        h.update(f"{name}:{nf}:".encode())
        h.update(repr(pts).encode())
    return h.hexdigest()[:16]


def export_fbx(path, objs):
    bpy.ops.object.select_all(action="DESELECT")
    for o in objs:
        o.select_set(True)
        for c in o.children_recursive:
            c.select_set(True)
    os.makedirs(os.path.dirname(path), exist_ok=True)
    bpy.ops.export_scene.fbx(filepath=path, use_selection=True, apply_unit_scale=True,
                             apply_scale_options="FBX_SCALE_UNITS", bake_space_transform=True,
                             axis_forward="-Z", axis_up="Y", object_types={"MESH", "EMPTY"},
                             use_mesh_modifiers=True, mesh_smooth_type="FACE", use_tspace=True,
                             add_leaf_bones=False, path_mode="STRIP")
    allobjs = set()
    for o in objs:
        allobjs.add(o)
        allobjs.update(o.children_recursive)
    print("EXPORTED", path, os.path.getsize(path), "geo", geo_digest(list(allobjs)))


def save_blend(path):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    bpy.ops.wm.save_as_mainfile(filepath=path, compress=True)
    print("SAVED", path)

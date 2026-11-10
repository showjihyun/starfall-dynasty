"""Style candidates for the ship art direction: one studio, one battle scene and one viewer export shared by
every candidate, so the four looks are compared under the same light (build_styles.py, styles/*.py).
Imports shiplib and never modifies it."""
import math
import os
import random

import bmesh
import bpy
from mathutils import Euler, Matrix, Vector

import shiplib
from shiplib import log

V = Vector

DEFAULT_TINT = dict(
    livery=(0.55, 0.06, 0.04),   # faction paint
    glow=(0.35, 0.8, 1.0),       # running lights, weapon bolts
    engine=(1.0, 0.62, 0.32),    # engine core and plume
    nebula=(0.05, 0.025, 0.09),  # backdrop
    enemy_livery=(0.08, 0.2, 0.5),
    enemy_glow=(1.0, 0.25, 0.12),
)


def tint_of(mod):
    t = dict(DEFAULT_TINT)
    t.update(getattr(mod, "TINT", {}))
    return t


def _bsdf(mat):
    return mat.node_tree.nodes["Principled BSDF"]


def apply_tint(mats, tint):
    _bsdf(mats["Livery"]).inputs['Base Color'].default_value = (*tint['livery'], 1)
    _bsdf(mats["Glow"]).inputs['Emission Color'].default_value = (*tint['glow'], 1)
    _bsdf(mats["EngineCore"]).inputs['Emission Color'].default_value = (*tint['engine'], 1)
    _bsdf(mats["EngineCore"]).inputs['Emission Strength'].default_value = 12.0
    if 'livery_metal' in tint:
        _bsdf(mats["Livery"]).inputs['Metallic'].default_value = tint['livery_metal']
    if 'livery_rough' in tint:
        _bsdf(mats["Livery"]).inputs['Roughness'].default_value = tint['livery_rough']
    if 'glass' in tint:
        _bsdf(mats["Glass"]).inputs['Base Color'].default_value = (*tint['glass'], 1)


# ---------------------------------------------------------------- studio

def setup_world(tint):
    """Stars and a nebula for the camera; for reflections a brighter, softer sky so metal has something to show."""
    w = bpy.data.worlds.new("Space")
    bpy.context.scene.world = w
    try:
        w.use_nodes = True
    except Exception:
        pass
    nt = w.node_tree
    for n in list(nt.nodes):
        nt.nodes.remove(n)
    N, Lk = nt.nodes.new, nt.links.new
    out = N('ShaderNodeOutputWorld')
    tc = N('ShaderNodeTexCoord')
    vor = N('ShaderNodeTexVoronoi')
    vor.inputs['Scale'].default_value = 300
    Lk(tc.outputs['Generated'], vor.inputs['Vector'])
    star = N('ShaderNodeMapRange')
    star.inputs['From Min'].default_value = 0.0
    star.inputs['From Max'].default_value = 0.05
    star.inputs['To Min'].default_value = 1.0
    star.inputs['To Max'].default_value = 0.0
    Lk(vor.outputs['Distance'], star.inputs['Value'])
    sep = N('ShaderNodeSeparateColor')
    Lk(vor.outputs['Color'], sep.inputs[0])
    gate = N('ShaderNodeMath')
    gate.operation = 'GREATER_THAN'
    gate.inputs[1].default_value = 0.8
    Lk(sep.outputs[0], gate.inputs[0])
    s1 = N('ShaderNodeMath')
    s1.operation = 'MULTIPLY'
    Lk(star.outputs[0], s1.inputs[0])
    Lk(gate.outputs[0], s1.inputs[1])
    s2 = N('ShaderNodeMath')
    s2.operation = 'MULTIPLY'
    s2.inputs[1].default_value = 5.0
    Lk(s1.outputs[0], s2.inputs[0])

    def cloud(scale, lo, hi, col):
        nz = N('ShaderNodeTexNoise')
        nz.inputs['Scale'].default_value = scale
        nz.inputs['Detail'].default_value = 9
        nz.inputs['Roughness'].default_value = 0.62
        Lk(tc.outputs['Generated'], nz.inputs['Vector'])
        r = N('ShaderNodeValToRGB')
        r.color_ramp.elements[0].position = lo
        r.color_ramp.elements[0].color = (0, 0, 0, 1)
        r.color_ramp.elements[1].position = hi
        r.color_ramp.elements[1].color = (*col, 1)
        Lk(nz.outputs['Fac'], r.inputs[0])
        return r.outputs[0]

    def add(a, b):
        m = N('ShaderNodeMix')
        m.data_type = 'RGBA'
        m.blend_type = 'ADD'
        m.inputs[0].default_value = 1.0
        Lk(a, m.inputs[6])
        Lk(b, m.inputs[7])
        return m.outputs[2]

    neb = tint['nebula']
    neb2 = tint.get('nebula2', (neb[2] * 0.6, neb[0] * 0.8, neb[1] * 1.6))
    sky = add(cloud(1.4, 0.42, 0.9, neb), cloud(2.3, 0.5, 0.95, neb2))
    cam = N('ShaderNodeBackground')
    Lk(add(sky, s2.outputs[0]), cam.inputs['Color'])
    amb = N('ShaderNodeBackground')
    amb.inputs['Strength'].default_value = 9.0
    Lk(add(sky, cloud(0.6, 0.3, 1.0, (0.012, 0.014, 0.02))), amb.inputs['Color'])
    lp = N('ShaderNodeLightPath')
    mx = N('ShaderNodeMixShader')
    Lk(lp.outputs['Is Camera Ray'], mx.inputs[0])
    Lk(amb.outputs[0], mx.inputs[1])
    Lk(cam.outputs[0], mx.inputs[2])
    Lk(mx.outputs[0], out.inputs['Surface'])


def setup_bloom(strength=0.5, size=0.45, threshold=2.0):
    sc = bpy.context.scene
    ng = bpy.data.node_groups.new("Comp", "CompositorNodeTree")
    sc.compositing_node_group = ng
    rl = ng.nodes.new("CompositorNodeRLayers")
    gl = ng.nodes.new("CompositorNodeGlare")
    out = ng.nodes.new("NodeGroupOutput")
    ng.interface.new_socket("Image", in_out="OUTPUT", socket_type="NodeSocketColor")
    for k, v in (('Type', 'Bloom'), ('Quality', 'High'), ('Threshold', threshold), ('Strength', strength), ('Size', size)):
        try:
            gl.inputs[k].default_value = v
        except Exception as e:
            log("bloom input", k, e)
    ng.links.new(rl.outputs[0], gl.inputs[0])
    ng.links.new(gl.outputs[0], out.inputs[0])
    sc.render.use_compositing = True


def setup_studio(tint):
    """Key, rim and fill suns, the camera and the bloom. Returns the camera."""
    setup_world(tint)
    shiplib.add_sun("Key", (1.0, 0.8, 1.1), 3.0, (1.0, 0.95, 0.88))
    shiplib.add_sun("Rim", (-1.1, -1.0, 0.5), 3.6, tint.get('rim', (0.55, 0.72, 1.0)), angle=1.0)
    shiplib.add_sun("Fill", (-0.6, 0.6, -0.7), 0.6, (0.7, 0.7, 0.9))
    sc = bpy.context.scene
    sc.render.engine = 'CYCLES'
    sc.cycles.use_denoising = True
    try:
        sc.cycles.denoiser = 'OPENIMAGEDENOISE'
    except Exception:
        pass
    sc.view_settings.view_transform = 'AgX'
    try:
        sc.view_settings.look = 'AgX - Medium High Contrast'
    except Exception:
        pass
    cd = bpy.data.cameras.new("Cam")
    cd.lens = 50
    cam = bpy.data.objects.new("Cam", cd)
    sc.collection.objects.link(cam)
    sc.camera = cam
    setup_bloom()
    return cam


def render_views(cam, objs, outdir, name, fast=False):
    """<name>_hero.png (front three-quarter) and <name>_sheet.png (front, rear, top, side)."""
    os.makedirs(outdir, exist_ok=True)
    s = 48 if fast else 128
    shiplib.frame(cam, objs, (1.0, 1.15, 0.55))
    shiplib.render(os.path.join(outdir, name + "_hero.png"), 1600, 900, s)
    views = []
    for i, (d, ortho) in enumerate((((-1.0, 1.1, 0.6), False), ((-0.9, -1.2, 0.5), False),
                                    ((0.0, 0.0001, 1.0), True), ((1.0, 0.0001, 0.0), True))):
        p = os.path.join(outdir, "_%s_v%d.png" % (name, i))
        shiplib.frame(cam, objs, d, ortho=ortho)
        shiplib.render(p, 960, 540, 32 if fast else 64)
        views.append(p)
    shiplib.compose_sheet(views, os.path.join(outdir, name + "_sheet.png"), cw=960, ch=540)
    for p in views:
        os.remove(p)


def export_viewer(path, objs, empties):
    """A GLB for the picker page: WebP textures, paint and lights already tinted. Not a game asset."""
    bpy.ops.object.select_all(action='DESELECT')
    for o in list(objs) + list(empties):
        o.select_set(True)
    bpy.context.view_layer.objects.active = list(objs)[0]
    bpy.ops.export_scene.gltf(
        filepath=path, export_format='GLB', use_selection=True, export_yup=True, export_apply=True,
        export_materials='EXPORT', export_image_format='WEBP', export_cameras=False, export_lights=False,
        export_animations=False, export_extras=False, export_texcoords=True, export_normals=True,
        export_tangents=False)
    log("viewer glb", path, "%.2f MB" % (os.path.getsize(path) / 1e6))


# ---------------------------------------------------------------- battle scene

def _emit(name, color, strength):
    m = bpy.data.materials.new(name)
    m.use_nodes = True
    p = _bsdf(m)
    p.inputs['Base Color'].default_value = (0, 0, 0, 1)
    p.inputs['Emission Color'].default_value = (*color, 1)
    p.inputs['Emission Strength'].default_value = strength
    return m


def _plume_mat(name, color):
    """Hot core fading along the cone and toward its edge."""
    m = bpy.data.materials.new(name)
    m.use_nodes = True
    nt = m.node_tree
    for n in list(nt.nodes):
        nt.nodes.remove(n)
    N, Lk = nt.nodes.new, nt.links.new
    out = N('ShaderNodeOutputMaterial')
    tc = N('ShaderNodeTexCoord')
    sep = N('ShaderNodeSeparateXYZ')
    Lk(tc.outputs['Object'], sep.inputs[0])
    along = N('ShaderNodeMath')  # 1 at the nozzle, 0 at the tip
    along.operation = 'ADD'
    along.inputs[1].default_value = 1.0
    Lk(sep.outputs[1], along.inputs[0])
    fade = N('ShaderNodeMath')
    fade.operation = 'POWER'
    fade.inputs[1].default_value = 2.2
    fade.use_clamp = True
    Lk(along.outputs[0], fade.inputs[0])
    lw = N('ShaderNodeLayerWeight')
    lw.inputs['Blend'].default_value = 0.35
    core = N('ShaderNodeMath')
    core.operation = 'SUBTRACT'
    core.inputs[0].default_value = 1.0
    core.use_clamp = True
    Lk(lw.outputs['Facing'], core.inputs[1])
    a = N('ShaderNodeMath')
    a.operation = 'MULTIPLY'
    Lk(fade.outputs[0], a.inputs[0])
    Lk(core.outputs[0], a.inputs[1])
    col = N('ShaderNodeMix')
    col.data_type = 'RGBA'
    col.inputs[6].default_value = (*color, 1)
    col.inputs[7].default_value = (1, 1, 1, 1)
    Lk(a.outputs[0], col.inputs[0])
    em = N('ShaderNodeEmission')
    em.inputs['Strength'].default_value = 3.5
    Lk(col.outputs[2], em.inputs['Color'])
    tr = N('ShaderNodeBsdfTransparent')
    mx = N('ShaderNodeMixShader')
    Lk(a.outputs[0], mx.inputs[0])
    Lk(tr.outputs[0], mx.inputs[1])
    Lk(em.outputs[0], mx.inputs[2])
    Lk(mx.outputs[0], out.inputs['Surface'])
    return m


def _fire_mat(name, seed):
    """Volume fireball: noisy density, blackbody-ish emission."""
    m = bpy.data.materials.new(name)
    m.use_nodes = True
    nt = m.node_tree
    for n in list(nt.nodes):
        nt.nodes.remove(n)
    N, Lk = nt.nodes.new, nt.links.new
    out = N('ShaderNodeOutputMaterial')
    tc = N('ShaderNodeTexCoord')
    mp = N('ShaderNodeMapping')
    mp.inputs['Location'].default_value = (seed, seed * 0.37, -seed)
    Lk(tc.outputs['Object'], mp.inputs[0])
    nz = N('ShaderNodeTexNoise')
    nz.inputs['Scale'].default_value = 1.7
    nz.inputs['Detail'].default_value = 7
    nz.inputs['Roughness'].default_value = 0.7
    Lk(mp.outputs[0], nz.inputs['Vector'])
    ln = N('ShaderNodeVectorMath')
    ln.operation = 'LENGTH'
    Lk(tc.outputs['Object'], ln.inputs[0])
    edge = N('ShaderNodeMapRange')  # 1 in the middle, 0 at the sphere's surface
    edge.inputs['From Min'].default_value = 0.25
    edge.inputs['From Max'].default_value = 1.0
    edge.inputs['To Min'].default_value = 1.0
    edge.inputs['To Max'].default_value = 0.0
    Lk(ln.outputs['Value'], edge.inputs['Value'])
    d = N('ShaderNodeMath')
    d.operation = 'MULTIPLY'
    Lk(nz.outputs['Fac'], d.inputs[0])
    Lk(edge.outputs[0], d.inputs[1])
    ramp = N('ShaderNodeValToRGB')
    ramp.color_ramp.elements[0].position = 0.2
    ramp.color_ramp.elements[0].color = (0, 0, 0, 1)
    ramp.color_ramp.elements[1].position = 0.62
    ramp.color_ramp.elements[1].color = (1, 1, 1, 1)
    Lk(d.outputs[0], ramp.inputs[0])
    heat = N('ShaderNodeValToRGB')
    e = heat.color_ramp.elements
    e[0].position, e[0].color = 0.0, (0.35, 0.02, 0.0, 1)
    e[1].position, e[1].color = 1.0, (1.0, 0.9, 0.6, 1)
    mid = e.new(0.55)
    mid.color = (1.0, 0.3, 0.03, 1)
    Lk(ramp.outputs[0], heat.inputs[0])
    dens = N('ShaderNodeMath')
    dens.operation = 'MULTIPLY'
    dens.inputs[1].default_value = 2.2
    Lk(ramp.outputs[0], dens.inputs[0])
    hot = N('ShaderNodeMath')
    hot.operation = 'POWER'
    hot.inputs[1].default_value = 2.6
    Lk(ramp.outputs[0], hot.inputs[0])
    es = N('ShaderNodeMath')
    es.operation = 'MULTIPLY'
    es.inputs[1].default_value = 34.0
    Lk(hot.outputs[0], es.inputs[0])
    vol = N('ShaderNodeVolumePrincipled')
    vol.inputs['Color'].default_value = (0.05, 0.04, 0.04, 1)
    Lk(dens.outputs[0], vol.inputs['Density'])
    Lk(heat.outputs[0], vol.inputs['Emission Color'])
    Lk(es.outputs[0], vol.inputs['Emission Strength'])
    Lk(vol.outputs[0], out.inputs['Volume'])
    return m


def _shield_mat(name, color):
    """A bubble seen only at its rim and where a hex lattice crosses it."""
    m = bpy.data.materials.new(name)
    m.use_nodes = True
    nt = m.node_tree
    for n in list(nt.nodes):
        nt.nodes.remove(n)
    N, Lk = nt.nodes.new, nt.links.new
    out = N('ShaderNodeOutputMaterial')
    lw = N('ShaderNodeLayerWeight')
    lw.inputs['Blend'].default_value = 0.32
    tc = N('ShaderNodeTexCoord')
    vor = N('ShaderNodeTexVoronoi')
    vor.feature = 'DISTANCE_TO_EDGE'
    vor.inputs['Scale'].default_value = 16.0
    Lk(tc.outputs['Object'], vor.inputs['Vector'])
    cell = N('ShaderNodeMapRange')
    cell.inputs['From Min'].default_value = 0.0
    cell.inputs['From Max'].default_value = 0.02
    cell.inputs['To Min'].default_value = 1.0
    cell.inputs['To Max'].default_value = 0.0
    Lk(vor.outputs['Distance'], cell.inputs['Value'])
    sep = N('ShaderNodeSeparateXYZ')
    Lk(tc.outputs['Object'], sep.inputs[0])
    near = N('ShaderNodeMapRange')  # strongest toward the struck side (+Z of the bubble)
    near.inputs['From Min'].default_value = 0.5
    near.inputs['From Max'].default_value = 1.0
    Lk(sep.outputs[2], near.inputs['Value'])
    lat = N('ShaderNodeMath')
    lat.operation = 'MULTIPLY'
    Lk(cell.outputs[0], lat.inputs[0])
    Lk(near.outputs[0], lat.inputs[1])
    rim = N('ShaderNodeMath')
    rim.operation = 'MULTIPLY'
    rim.inputs[1].default_value = 0.0
    Lk(lw.outputs['Fresnel'], rim.inputs[0])
    a = N('ShaderNodeMath')
    a.operation = 'ADD'
    a.use_clamp = True
    Lk(lat.outputs[0], a.inputs[0])
    Lk(rim.outputs[0], a.inputs[1])
    em = N('ShaderNodeEmission')
    em.inputs['Color'].default_value = (*color, 1)
    em.inputs['Strength'].default_value = 5.0
    tr = N('ShaderNodeBsdfTransparent')
    mx = N('ShaderNodeMixShader')
    Lk(a.outputs[0], mx.inputs[0])
    Lk(tr.outputs[0], mx.inputs[1])
    Lk(em.outputs[0], mx.inputs[2])
    Lk(mx.outputs[0], out.inputs['Surface'])
    return m


def _planet_mat():
    m = bpy.data.materials.new("Planet")
    m.use_nodes = True
    nt = m.node_tree
    N, Lk = nt.nodes.new, nt.links.new
    p = _bsdf(m)
    tc = N('ShaderNodeTexCoord')
    nz = N('ShaderNodeTexNoise')
    nz.inputs['Scale'].default_value = 3.2
    nz.inputs['Detail'].default_value = 12
    nz.inputs['Roughness'].default_value = 0.65
    Lk(tc.outputs['Object'], nz.inputs['Vector'])
    r = N('ShaderNodeValToRGB')
    e = r.color_ramp.elements
    e[0].position, e[0].color = 0.38, (0.012, 0.03, 0.07, 1)
    e[1].position, e[1].color = 0.7, (0.5, 0.52, 0.55, 1)
    mid = e.new(0.52)
    mid.color = (0.09, 0.075, 0.05, 1)
    Lk(nz.outputs['Fac'], r.inputs[0])
    Lk(r.outputs[0], p.inputs['Base Color'])
    p.inputs['Roughness'].default_value = 0.8
    lw = N('ShaderNodeLayerWeight')
    lw.inputs['Blend'].default_value = 0.22
    air = N('ShaderNodeMath')
    air.operation = 'POWER'
    air.inputs[1].default_value = 1.6
    Lk(lw.outputs['Facing'], air.inputs[0])
    p.inputs['Emission Color'].default_value = (0.2, 0.5, 1.0, 1)
    k = N('ShaderNodeMath')
    k.operation = 'MULTIPLY'
    k.inputs[1].default_value = 1.6
    Lk(air.outputs[0], k.inputs[0])
    Lk(k.outputs[0], p.inputs['Emission Strength'])
    return m


def _mesh_obj(name, build, mat):
    bm = bmesh.new()
    build(bm)
    me = bpy.data.meshes.new(name)
    bm.to_mesh(me)
    bm.free()
    for poly in me.polygons:
        poly.use_smooth = True
    me.materials.append(mat)
    o = bpy.data.objects.new(name, me)
    bpy.context.scene.collection.objects.link(o)
    return o


def _cone(bm):
    """Unit plume: mouth radius 1 at the origin, tip at -Y 1, bulging a little."""
    shiplib.lathe(bm, V((0, 0, 0)), V((0, -1, 0)), [(0.92, 0.0), (1.0, 0.08), (0.8, 0.35), (0.4, 0.7), (0.0, 1.0)], n=24)


def _capsule(bm):
    shiplib.lathe(bm, V((0, -0.5, 0)), V((0, 1, 0)), [(0.0, 0.0), (0.7, 0.04), (1.0, 0.2), (1.0, 0.8), (0.6, 0.96), (0.0, 1.0)], n=10)


def _ico(bm):
    bmesh.ops.create_icosphere(bm, subdivisions=3, radius=1.0)


def _uv(bm):
    bmesh.ops.create_uvsphere(bm, u_segments=96, v_segments=48, radius=1.0)


def battle(objs, empties, mats, tint, path, fast=False):
    """One fleet engagement still, the same staging for every candidate: our flagship close to the lens with a
    consort ahead, three of theirs crossing (one hit, one going up), beams and bolts both ways, plumes, a
    planet's limb. Every distance is in ship lengths, so any model scale works. Seeded, so it repeats."""
    sc = bpy.context.scene
    rnd = random.Random(7)
    objs = [o for o in objs if not o.name.startswith(("Rank", "Species_"))]
    bpy.context.view_layer.update()
    ys = [(o.matrix_world @ V(c)).y for o in objs for c in o.bound_box]
    L = max(ys) - min(ys)
    world = [(o, o.matrix_world.copy()) for o in objs]
    engines = [(e.matrix_world.translation.copy(), e.matrix_world.to_scale().x) for e in empties if e.name.startswith("Engine_")]
    guns = [e.matrix_world.translation.copy() for e in empties if e.name.startswith(("Gun_", "Main_"))]
    for o in objs:
        o.hide_render = True

    # Their paint and lights: copies of the two tinted materials.
    foe = {}
    for k, col, sock in (("Livery", tint['enemy_livery'], 'Base Color'), ("Glow", tint['enemy_glow'], 'Emission Color')):
        m = mats[k].copy()
        _bsdf(m).inputs[sock].default_value = (*col, 1)
        foe[mats[k].name] = m

    plume = {False: _plume_mat("Plume", tint['engine']), True: _plume_mat("PlumeFoe", tint.get('enemy_engine', tint['enemy_glow']))}
    bolt = {False: _emit("Bolt", tint['glow'], 22.0), True: _emit("BoltFoe", tint['enemy_glow'], 22.0)}
    beam = {False: _emit("Beam", tint['glow'], 9.0), True: _emit("BeamFoe", tint['enemy_glow'], 9.0)}
    spark = _emit("Spark", (1.0, 0.6, 0.2), 30.0)

    def ship(loc, pitch, roll, yaw, enemy=False, burn=1.0):
        """loc in ship lengths. Returns its matrix and its gun points in the world."""
        root = bpy.data.objects.new("Ship", None)
        root.location = V(loc) * L
        root.rotation_euler = Euler((math.radians(pitch), math.radians(roll), math.radians(yaw)), 'ZXY')
        sc.collection.objects.link(root)
        for o, mw in world:
            d = o.copy()
            if enemy:
                d.data = o.data.copy()
                for i, m in enumerate(d.data.materials):
                    if m and m.name in foe:
                        d.data.materials[i] = foe[m.name]
            d.hide_render = False
            d.parent = root
            d.matrix_parent_inverse = Matrix.Identity(4)
            d.matrix_basis = mw
            sc.collection.objects.link(d)
        bpy.context.view_layer.update()
        M = root.matrix_world.copy()
        for at, r in engines:
            if burn > 0:
                p = _mesh_obj("PlumeObj", _cone, plume[enemy])
                p.matrix_world = M @ Matrix.Translation(at) @ Matrix.Diagonal((r * 0.95, r * 9 * burn, r * 0.95, 1))
        return M, [M @ g for g in guns]

    def aim(M):
        """A point on that ship's hull, roughly."""
        return M @ V((rnd.uniform(-0.03, 0.03) * L, rnd.uniform(-0.3, 0.3) * L, rnd.uniform(-0.02, 0.04) * L))

    def line(a, b, radius, mat):
        d = b - a
        o = _mesh_obj("Shot", _capsule, mat)
        o.matrix_world = Matrix.Translation((a + b) / 2) @ d.to_track_quat('Y', 'Z').to_matrix().to_4x4() @ Matrix.Diagonal((radius, d.length, radius, 1))

    def blast(at, r, seed, light=True):
        f = _mesh_obj("Fire", _ico, _fire_mat("Fire%d" % seed, 3.1 * seed))
        f.matrix_world = Matrix.Translation(at) @ Matrix.Diagonal((r, r, r, 1))
        for _ in range(int(10 + r / L * 160)):
            d = V((rnd.uniform(-1, 1), rnd.uniform(-1, 1), rnd.uniform(-1, 1))).normalized()
            a = at + d * rnd.uniform(0.7, 1.9) * r
            line(a, a + d * rnd.uniform(0.15, 0.6) * r, 0.0012 * L, spark)
        if light:
            ld = bpy.data.lights.new("Blast", 'POINT')
            ld.energy = 260.0 * L * L * (r / (0.2 * L))
            ld.color = (1.0, 0.55, 0.2)
            ld.shadow_soft_size = r * 0.4
            lo = bpy.data.objects.new("Blast", ld)
            lo.location = at + (sc.camera.location - at).normalized() * r * 1.2
            sc.collection.objects.link(lo)

    def volley(src, M_to, enemy=False, beams=2, hits=0):
        """Bolts strung along each gun's line to the target; the first guns hold a beam on it."""
        for i, g in enumerate(src):
            t = aim(M_to)
            if i < beams:
                line(g, t, 0.0016 * L, beam[enemy])
                f = _mesh_obj("Flare", _ico, bolt[enemy])
                f.matrix_world = Matrix.Translation(t) @ Matrix.Diagonal((0.012 * L,) * 3 + (1,))
                if i < hits:
                    blast(t, rnd.uniform(0.035, 0.06) * L, 11 + i, light=False)
                continue
            for k in range(2):
                a = g.lerp(t, rnd.uniform(0.05, 0.9))
                line(a, a + (t - g).normalized() * 0.1 * L, 0.0035 * L, bolt[enemy])
            f = _mesh_obj("Flash", _ico, bolt[enemy])
            f.matrix_world = Matrix.Translation(g) @ Matrix.Diagonal((0.003 * L,) * 3 + (1,))

    cam = sc.camera
    cam.data.type = 'PERSP'
    cam.data.lens = 28
    cam.data.clip_start = 0.02 * L
    cam.data.clip_end = 500 * L
    cam.location = V((-0.8, -1.2, 0.45)) * L
    cam.rotation_euler = (V((0.35, 1.5, -0.1)) * L - cam.location).to_track_quat('-Z', 'Y').to_euler()

    # Ours: the flagship close to the lens, a consort ahead to port.
    lead, lead_guns = ship((0, 0, 0), 2, -7, 4)
    mate, mate_guns = ship((-0.95, 1.9, 0.3), -2, 9, 10)
    # Theirs: crossing ahead to starboard; one further off already going up.
    e1, e1_guns = ship((1.35, 2.7, 0.05), -2, 10, 205, enemy=True)
    e2, e2_guns = ship((0.2, 4.6, 0.75), 3, -12, 222, enemy=True)
    e3, _ = ship((-0.5, 3.6, -0.55), 14, 38, 160, enemy=True, burn=0.0)

    volley(lead_guns, e1, hits=2)
    volley(mate_guns, e3, beams=1)
    volley(e1_guns, lead, enemy=True, beams=1)
    volley(e2_guns, mate, enemy=True, beams=1)
    c3 = e3.translation
    blast(c3, 0.26 * L, 1)
    blast(c3 + V((0.16, -0.1, 0.1)) * L, 0.15 * L, 2, light=False)
    blast(c3 + V((-0.2, 0.12, -0.06)) * L, 0.12 * L, 3, light=False)

    # Strike craft: far too small to model at this range, so a hot head and a fading trail each, in flights.
    trail = {False: _emit("Trail", tint['engine'], 6.0), True: _emit("TrailFoe", tint.get('enemy_engine', tint['enemy_glow']), 6.0)}
    for flight in range(9):
        enemy = flight % 2 == 1
        a = V((rnd.uniform(-0.9, 1.6), rnd.uniform(0.2, 3.2), rnd.uniform(-0.5, 0.7))) * L
        d = V((rnd.uniform(-1, 1), rnd.uniform(-1, 1), rnd.uniform(-0.35, 0.35))).normalized()
        side = d.cross(V((0, 0, 1))).normalized()
        for k in range(rnd.randint(3, 5)):
            head = a + side * (k - 2) * 0.035 * L + d * rnd.uniform(-0.03, 0.03) * L + V((0, 0, rnd.uniform(-0.015, 0.015) * L))
            line(head - d * rnd.uniform(0.1, 0.2) * L, head, 0.0011 * L, trail[enemy])
            f = _mesh_obj("Craft", _ico, bolt[enemy])
            f.matrix_world = Matrix.Translation(head) @ Matrix.Diagonal((0.0022 * L,) * 3 + (1,))

    # Flak around the two ships trading broadsides.
    for i in range(16):
        c = (lead if i % 2 else e1).translation + V((rnd.uniform(-0.45, 0.45), rnd.uniform(-0.6, 0.6), rnd.uniform(0.08, 0.4) * rnd.choice((-1, 1)))) * L
        f = _mesh_obj("Flak", _ico, _fire_mat("Flak%d" % i, 1.7 * (i + 5)))
        r = rnd.uniform(0.012, 0.028) * L
        f.matrix_world = Matrix.Translation(c) @ Matrix.Diagonal((r, r, r, 1))

    # The flagship's shield takes the beam that reaches it.
    sh = _mesh_obj("Shield", _uv, _shield_mat("ShieldHit", tint['glow']))
    at = lead @ V((0.02 * L, 0.18 * L, 0.02 * L))
    out_dir = (e1.translation - at).normalized()
    sh.matrix_world = Matrix.Translation(at) @ out_dir.to_track_quat('Z', 'Y').to_matrix().to_4x4() @ Matrix.Diagonal((0.15 * L, 0.15 * L, 0.09 * L, 1))

    # Wreckage thrown from the ship going up: dark plates catching the fire.
    plate = bpy.data.materials.new("Wreck")
    plate.use_nodes = True
    _bsdf(plate).inputs['Base Color'].default_value = (0.03, 0.03, 0.035, 1)
    _bsdf(plate).inputs['Metallic'].default_value = 0.8
    _bsdf(plate).inputs['Roughness'].default_value = 0.45
    for i in range(70):
        d = V((rnd.uniform(-1, 1), rnd.uniform(-1, 1), rnd.uniform(-1, 1))).normalized()
        o = _mesh_obj("Shard", lambda bm: shiplib.box(bm, V((0, 0, 0)), (1, 1, 1)), plate)
        for poly in o.data.polygons:
            poly.use_smooth = False
        sz = rnd.uniform(0.004, 0.022) * L
        o.matrix_world = (Matrix.Translation(c3 + d * rnd.uniform(0.2, 0.75) * L)
                          @ Euler((rnd.uniform(0, 6.3), rnd.uniform(0, 6.3), rnd.uniform(0, 6.3))).to_matrix().to_4x4()
                          @ Matrix.Diagonal((sz * rnd.uniform(0.6, 2.5), sz * rnd.uniform(0.6, 2.5), sz * 0.25, 1)))

    # The rest of both fleets, far off: silhouettes, plumes and their own exchange of fire.
    far = []
    for i, (loc, yaw, enemy) in enumerate((((-2.6, 7.5, 1.2), 14, False), ((-3.6, 10.5, -0.4), 8, False), ((-1.2, 12.0, 2.2), 20, False),
                                           ((3.2, 8.5, 1.0), 200, True), ((2.0, 11.5, 2.0), 212, True), ((4.6, 12.5, -0.2), 196, True))):
        far.append((ship(loc, rnd.uniform(-4, 4), rnd.uniform(-12, 12), yaw, enemy=enemy), enemy))
    for i, ((M, g), enemy) in enumerate(far):
        target = far[(i + 3) % 6][0][0]
        volley(g[:5], target, enemy=enemy, beams=0)
    blast(far[4][0][0].translation + V((0.1, 0, 0.05)) * L, 0.09 * L, 21, light=False)

    # The sun breaking over the planet's limb.
    sun = _mesh_obj("SunDisc", _ico, _emit("SunDisc", (1.0, 0.86, 0.66), 900.0))
    sun.matrix_world = Matrix.Translation(V((-33, 262, 26)) * L) @ Matrix.Diagonal((1.6 * L,) * 3 + (1,))

    planet = _mesh_obj("PlanetObj", _uv, _planet_mat())
    planet.matrix_world = Matrix.Translation(V((-150, 260, -146)) * L) @ Matrix.Diagonal((138 * L,) * 3 + (1,))

    for o in sc.objects:  # light from ahead-right so the flagship's flank and the planet's limb both read
        if o.name == "Key":
            o.rotation_euler = (-V((0.9, 0.35, 0.6))).to_track_quat('-Z', 'Y').to_euler()
            o.data.energy = 3.8
        if o.name == "Rim":
            o.rotation_euler = (-V((-1.0, 0.9, 0.25))).to_track_quat('-Z', 'Y').to_euler()
    sc.cycles.volume_step_rate = 1.0
    shiplib.render(path, 1920, 1080, 64 if fast else 192)

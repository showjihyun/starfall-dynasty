"""Planet Vela (ice-ocean world) and star Cradle: baked equirect textures + preview renders.

Run:  blender --background --factory-startup --python art/scripts/planet_star.py
Out:  art/textures/planet/vela_{albedo,normal,roughness,clouds,height}.png  (4096 x 2048 equirect)
      art/textures/star/cradle_{surface}.png (2048 x 1024 equirect), cradle_corona.png (1024^2, RGBA)
      art/source/planet_star.blend, art/previews/planet_vela_*.png, star_cradle.png

Data (data/world/systems/cradle.json reference_markers): Vela visual_radius_m 900 at (3000,-200,-900);
Cradle visual_radius_m 600 at origin. Glacine is "volatile ice held in Vela's orbit" -> Vela is an
ice-shelf ocean world, so the ice deposits around it look like they belong to it.
Unity plan (01_art_direction.md 5.2): URP Lit sphere with these maps; cloud shell (x1.012, alpha
from vela_clouds, slow spin); atmosphere shell (x1.05, additive fresnel, brighter toward the star).
Deterministic: procedural nodes only, fixed W offsets, no random module.
"""
import math
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import bpy
import bmesh
import numpy as np

import common as C

PLANET_TEX = os.path.join(C.TEX, "planet")
STAR_TEX = os.path.join(C.TEX, "star")
VELA_R = 900.0
STAR_R = 600.0


def node(nt, kind, **kw):
    n = nt.nodes.new(kind)
    for k, v in kw.items():
        setattr(n, k, v)
    return n


def fbm(nt, vec, scale, w, detail=8, rough=0.55, dist=0.0, lac=2.0):
    n = node(nt, "ShaderNodeTexNoise", noise_dimensions="4D")
    n.inputs["Scale"].default_value = scale
    n.inputs["Detail"].default_value = detail
    n.inputs["Roughness"].default_value = rough
    n.inputs["Distortion"].default_value = dist
    n.inputs["Lacunarity"].default_value = lac
    n.inputs["W"].default_value = w
    nt.links.new(vec, n.inputs["Vector"])
    return n.outputs["Fac"]


def ramp(nt, src, stops):
    r = node(nt, "ShaderNodeValToRGB")
    nt.links.new(src, r.inputs["Fac"])
    els = r.color_ramp.elements
    els[0].position, els[0].color = stops[0]
    els[1].position, els[1].color = stops[-1]
    for pos, col in stops[1:-1]:
        e = els.new(pos)
        e.color = col
    return r.outputs["Color"]


def math_(nt, op, a, b=None):
    m = node(nt, "ShaderNodeMath", operation=op)
    for i, x in enumerate((a, b)):
        if x is None:
            continue
        if isinstance(x, (int, float)):
            m.inputs[i].default_value = x
        else:
            nt.links.new(x, m.inputs[i])
    return m.outputs[0]


def mixc(nt, fac, a, b):
    m = node(nt, "ShaderNodeMix", data_type="RGBA", blend_type="MIX")
    nt.links.new(fac, m.inputs["Factor"]) if not isinstance(fac, float) else setattr(m.inputs["Factor"], "default_value", fac)
    ai, bi, o = C._mix_rgba_sockets(m)
    for s, x in ((ai, a), (bi, b)):
        if isinstance(x, tuple):
            s.default_value = x
        else:
            nt.links.new(x, s)
    return o


def vela_fields(nt):
    """Returns dict of sockets: albedo (color), height, rough, clouds."""
    tc = node(nt, "ShaderNodeTexCoord")
    p = tc.outputs["Object"]
    sep = node(nt, "ShaderNodeSeparateXYZ")
    nt.links.new(p, sep.inputs[0])
    lat = math_(nt, "ABSOLUTE", sep.outputs["Z"])
    # ice shelves / continents
    n1 = fbm(nt, p, 1.4, 1.3, detail=10, rough=0.58, dist=0.25)
    capn = fbm(nt, p, 3.0, 7.7, detail=6, rough=0.6)
    cap = math_(nt, "ADD", lat, math_(nt, "MULTIPLY", math_(nt, "SUBTRACT", capn, 0.5), 0.35))
    capm = node(nt, "ShaderNodeMapRange", interpolation_type="SMOOTHSTEP")
    nt.links.new(cap, capm.inputs["Value"])
    capm.inputs["From Min"].default_value = 0.68
    capm.inputs["From Max"].default_value = 0.76
    landm = node(nt, "ShaderNodeMapRange", interpolation_type="SMOOTHSTEP")
    nt.links.new(n1, landm.inputs["Value"])
    landm.inputs["From Min"].default_value = 0.54
    landm.inputs["From Max"].default_value = 0.56
    ice = math_(nt, "MAXIMUM", landm.outputs[0], capm.outputs[0])
    # crevasses on ice: voronoi edges
    vor = node(nt, "ShaderNodeTexVoronoi", voronoi_dimensions="3D", feature="DISTANCE_TO_EDGE")
    vor.inputs["Scale"].default_value = 70
    vor.inputs["Randomness"].default_value = 1.0
    warpv = node(nt, "ShaderNodeVectorMath", operation="ADD")
    nt.links.new(p, warpv.inputs[0])
    wn = node(nt, "ShaderNodeTexNoise", noise_dimensions="3D")
    wn.inputs["Scale"].default_value = 8.0
    wn.inputs["Detail"].default_value = 4
    nt.links.new(p, wn.inputs["Vector"])
    wsc = node(nt, "ShaderNodeVectorMath", operation="SCALE")
    nt.links.new(wn.outputs["Color"], wsc.inputs[0])
    wsc.inputs["Scale"].default_value = 0.06
    nt.links.new(wsc.outputs[0], warpv.inputs[1])
    nt.links.new(warpv.outputs[0], vor.inputs["Vector"])
    crev = node(nt, "ShaderNodeMapRange")
    nt.links.new(vor.outputs["Distance"], crev.inputs["Value"])
    crev.inputs["From Min"].default_value = 0.0
    crev.inputs["From Max"].default_value = 0.035
    crev.inputs["To Min"].default_value = 1.0
    crev.inputs["To Max"].default_value = 0.0
    icecol = ramp(nt, fbm(nt, p, 9.0, 3.3, detail=6), [(0.35, C.srgb("#B9D9E2")), (0.65, C.srgb("#EAF6F8"))])
    icecol = mixc(nt, math_(nt, "MULTIPLY", crev.outputs[0], 0.35), icecol, C.srgb("#7FA8B6"))
    # ocean depth: shallower near the shelf edge
    shallow = node(nt, "ShaderNodeMapRange")
    nt.links.new(n1, shallow.inputs["Value"])
    shallow.inputs["From Min"].default_value = 0.42
    shallow.inputs["From Max"].default_value = 0.545
    ocol = ramp(nt, shallow.outputs[0], [(0.0, C.srgb("#06202F")), (0.7, C.srgb("#0E3E55")),
                                         (1.0, C.srgb("#2B8A92"))])
    # drifting pack ice speckles on the ocean
    pack = node(nt, "ShaderNodeMapRange")
    nt.links.new(fbm(nt, p, 22.0, 9.1, detail=4), pack.inputs["Value"])
    pack.inputs["From Min"].default_value = 0.62
    pack.inputs["From Max"].default_value = 0.7
    packf = math_(nt, "MULTIPLY", pack.outputs[0], shallow.outputs[0])
    ocol = mixc(nt, packf, ocol, C.srgb("#C8E2E8"))
    albedo = mixc(nt, ice, ocol, icecol)
    height = math_(nt, "MULTIPLY", ice, math_(nt, "SUBTRACT", 1.0, math_(nt, "MULTIPLY", crev.outputs[0], 0.25)))
    rough = math_(nt, "ADD", 0.12, math_(nt, "MULTIPLY", ice, 0.5))
    # clouds: latitude-stretched swirls, thin at the poles
    cvec = node(nt, "ShaderNodeVectorMath", operation="MULTIPLY")
    nt.links.new(p, cvec.inputs[0])
    cvec.inputs[1].default_value = (1.0, 1.0, 3.2)
    cn = fbm(nt, cvec.outputs[0], 2.2, 21.0, detail=10, rough=0.62, dist=1.6)
    cm = node(nt, "ShaderNodeMapRange", interpolation_type="SMOOTHSTEP")
    nt.links.new(cn, cm.inputs["Value"])
    cm.inputs["From Min"].default_value = 0.5
    cm.inputs["From Max"].default_value = 0.72
    clouds = math_(nt, "MULTIPLY", cm.outputs[0], math_(nt, "SUBTRACT", 1.0, math_(nt, "MULTIPLY", lat, 0.6)))
    return {"albedo": albedo, "height": height, "rough": rough, "clouds": clouds}


def star_fields(nt):
    tc = node(nt, "ShaderNodeTexCoord")
    p = tc.outputs["Object"]
    v = node(nt, "ShaderNodeTexVoronoi", voronoi_dimensions="3D", feature="F1")
    v.inputs["Scale"].default_value = 28
    nt.links.new(p, v.inputs["Vector"])
    gran = math_(nt, "ADD", math_(nt, "MULTIPLY", v.outputs["Distance"], 1.2),
                 math_(nt, "MULTIPLY", fbm(nt, p, 6.0, 2.0, detail=6), 0.6))
    col = ramp(nt, gran, [(0.2, C.srgb("#FFF8EA")), (0.7, C.srgb("#FFE2B0")), (1.1, C.srgb("#F2A860"))])
    return {"surface": col}


def bake_emit(ob, mat, sock, img, is_color=True):
    nt = mat.node_tree
    out = nt.nodes["Material Output"]
    em = node(nt, "ShaderNodeEmission")
    if is_color:
        nt.links.new(sock, em.inputs["Color"])
    else:
        cmb = node(nt, "ShaderNodeCombineXYZ")
        for i in range(3):
            nt.links.new(sock, cmb.inputs[i])
        nt.links.new(cmb.outputs[0], em.inputs["Color"])
    nt.links.new(em.outputs[0], out.inputs["Surface"])
    t = nt.nodes.new("ShaderNodeTexImage")
    t.image = img
    nt.nodes.active = t
    bpy.ops.object.select_all(action="DESELECT")
    ob.select_set(True)
    bpy.context.view_layer.objects.active = ob
    bpy.context.scene.render.bake.use_selected_to_active = False
    bpy.context.scene.render.bake.margin = 2
    bpy.ops.object.bake(type="EMIT")
    nt.nodes.remove(t)
    nt.nodes.remove(em)


def uv_sphere(name, r, seg=256, rings=128):
    bpy.ops.mesh.primitive_uv_sphere_add(segments=seg, ring_count=rings, radius=r)
    ob = bpy.context.active_object
    ob.name = name
    for p in ob.data.polygons:
        p.use_smooth = True
    return ob


def save(img, path, non_color=False):
    img.filepath_raw = path
    img.file_format = "PNG"
    img.save()
    print("SAVED_TEX", path)


def height_to_normal(height_img, out_name, strength=3.0):
    w, h = height_img.size
    px = np.array(height_img.pixels[:], dtype=np.float32).reshape(h, w, 4)[:, :, 0]
    dx = (np.roll(px, -1, axis=1) - np.roll(px, 1, axis=1)) * 0.5
    dy = (np.roll(px, -1, axis=0) - np.roll(px, 1, axis=0)) * 0.5
    # equirect: horizontal texels shrink toward the poles
    lat = (np.arange(h) + 0.5) / h * math.pi - math.pi / 2
    dx = dx / np.maximum(np.cos(lat)[:, None], 0.05)
    nx, ny, nz = -dx * strength, -dy * strength, np.ones_like(px)
    ln = np.sqrt(nx * nx + ny * ny + nz * nz)
    out = np.stack([nx / ln * 0.5 + 0.5, ny / ln * 0.5 + 0.5, nz / ln * 0.5 + 0.5, np.ones_like(px)], axis=-1)
    img = bpy.data.images.new(out_name, w, h, alpha=False)
    img.colorspace_settings.name = "Non-Color"
    img.pixels = out.ravel().tolist()
    return img


RING_IN, RING_OUT = 1.1, 1.3  # x VELA_R: 990..1170 m, clear of the Vela Orbit deposits at 1,288 / 1,304 m


def ring_image(w=2048, h=8):
    """Radial ring profile (U = inner -> outer): fine bands, a Cassini-like gap, soft edges. RGBA."""
    u = np.linspace(0, 1, w, dtype=np.float32)
    rng = np.random.default_rng(900)
    bands = np.zeros(w, dtype=np.float32)
    for f, a in ((7, 0.25), (19, 0.18), (47, 0.12), (113, 0.08)):
        ph = rng.random() * 6.28
        bands += a * (0.5 + 0.5 * np.sin(u * f * 6.28 + ph))
    dens = 0.35 + bands
    dens *= np.clip(u / 0.08, 0, 1) * np.clip((1 - u) / 0.06, 0, 1)
    dens *= 1.0 - 0.9 * np.exp(-((u - 0.62) / 0.025) ** 2)  # gap
    dens = np.clip(dens, 0, 1)
    col = np.stack([0.86 + 0.08 * u, 0.9 + 0.04 * u, 0.95 * np.ones_like(u)], axis=-1)  # icy white -> faint blue
    rgba = np.concatenate([col, dens[:, None]], axis=-1)
    img = np.repeat(rgba[None, :, :], h, axis=0)
    out = bpy.data.images.new("vela_ring", w, h, alpha=True)
    out.pixels = img.ravel().tolist()
    return out


def ring_object(img, r):
    """Flat annulus, UV.x = radial (0 inner, 1 outer); alpha from the ring profile."""
    bm = bmesh.new()
    uvl = bm.loops.layers.uv.new("UVMap")
    n = 256
    vin = [bm.verts.new((RING_IN * r * math.cos(2 * math.pi * i / n), RING_IN * r * math.sin(2 * math.pi * i / n), 0)) for i in range(n)]
    vout = [bm.verts.new((RING_OUT * r * math.cos(2 * math.pi * i / n), RING_OUT * r * math.sin(2 * math.pi * i / n), 0)) for i in range(n)]
    for i in range(n):
        j = (i + 1) % n
        f = bm.faces.new((vin[i], vout[i], vout[j], vin[j]))
        for l, uv in zip(f.loops, ((0, 0.5), (1, 0.5), (1, 0.5), (0, 0.5))):
            l[uvl].uv = uv
    me = bpy.data.meshes.new("VelaRing")
    bm.to_mesh(me)
    ob = bpy.data.objects.new("VelaRing", me)
    bpy.context.scene.collection.objects.link(ob)
    m = bpy.data.materials.new("VelaRing")
    m.use_nodes = True
    nt = m.node_tree
    b = nt.nodes["Principled BSDF"]
    t = nt.nodes.new("ShaderNodeTexImage")
    t.image = img
    nt.links.new(t.outputs["Color"], b.inputs["Base Color"])
    nt.links.new(t.outputs["Alpha"], b.inputs["Alpha"])
    b.inputs["Roughness"].default_value = 0.8
    me.materials.append(m)
    ob.rotation_euler = (math.radians(14), math.radians(-8), 0)
    return ob


def corona_image(size=1024):
    yy, xx = np.mgrid[0:size, 0:size].astype(np.float32)
    c = (size - 1) / 2
    dx, dy = (xx - c) / c, (yy - c) / c
    r = np.sqrt(dx * dx + dy * dy)
    ang = np.arctan2(dy, dx)
    # disc edge at r = 0.25 (quad is 4x the star diameter); glow falls off outside
    rays = 1.0 + 0.18 * np.cos(ang * 5.0 + 0.6) * np.cos(ang * 11.0) + 0.1 * np.cos(ang * 19.0 + 1.3)
    glow = 0.85 * np.exp(-np.maximum(r - 0.25, 0) * 11.0 / rays) + 0.15 * np.exp(-np.maximum(r - 0.25, 0) * 3.5)
    glow *= np.clip((1.0 - r) / 0.25, 0, 1) ** 2
    rng = np.random.default_rng(7)
    glow = np.clip(glow + (rng.random(glow.shape, dtype=np.float32) - 0.5) / 255.0, 0, 1)
    rgb = np.stack([np.ones_like(r), 0.86 * np.ones_like(r), 0.66 * np.ones_like(r)], axis=-1)
    alpha = glow
    out = np.concatenate([rgb, alpha[..., None]], axis=-1)[::-1]
    img = bpy.data.images.new("cradle_corona", size, size, alpha=True)
    img.pixels = out.ravel().tolist()
    return img


def main():
    os.makedirs(PLANET_TEX, exist_ok=True)
    os.makedirs(STAR_TEX, exist_ok=True)
    C.reset_scene()
    sc = C.setup_cycles(samples=4, res=(1920, 1080))
    sc.cycles.use_denoising = False
    # ---------------- bake Vela
    vela = uv_sphere("Vela", 1.0)
    vm = bpy.data.materials.new("VelaBake")
    vm.use_nodes = True
    vela.data.materials.append(vm)
    F = vela_fields(vm.node_tree)
    W, H = 4096, 2048
    imgs = {}
    for key, is_col in (("albedo", True), ("height", False), ("rough", False), ("clouds", False)):
        img = bpy.data.images.new(f"vela_{key}", W, H, alpha=False, float_buffer=(key == "height"))
        if key != "albedo":
            img.colorspace_settings.name = "Non-Color"
        bake_emit(vela, vm, F[key], img, is_color=is_col)
        imgs[key] = img
    save(imgs["albedo"], os.path.join(PLANET_TEX, "vela_albedo.png"))
    save(imgs["rough"], os.path.join(PLANET_TEX, "vela_roughness.png"))
    save(imgs["clouds"], os.path.join(PLANET_TEX, "vela_clouds.png"))
    nimg = height_to_normal(imgs["height"], "vela_normal")
    save(nimg, os.path.join(PLANET_TEX, "vela_normal.png"))
    # ---------------- bake Cradle surface + corona
    star = uv_sphere("Cradle", 1.0, 128, 64)
    smat = bpy.data.materials.new("StarBake")
    smat.use_nodes = True
    star.data.materials.append(smat)
    SF = star_fields(smat.node_tree)
    simg = bpy.data.images.new("cradle_surface", 2048, 1024, alpha=False)
    bake_emit(star, smat, SF["surface"], simg)
    save(simg, os.path.join(STAR_TEX, "cradle_surface.png"))
    cimg = corona_image()
    save(cimg, os.path.join(STAR_TEX, "cradle_corona.png"))
    rimg = ring_image()
    save(rimg, os.path.join(PLANET_TEX, "vela_ring.png"))

    # ---------------- preview scene (textures only -> what Unity will get)
    for ob in list(bpy.data.objects):
        bpy.data.objects.remove(ob)
    sc.cycles.samples = 160
    sc.cycles.use_denoising = True
    sky = os.path.join(C.TEX, "skybox", "cradle_sky_equirect.png")
    C.world_space("#04060C", 1.0, hdri=sky if os.path.exists(sky) else None)
    sun = C.add_sun("Star", (70, 0, 120), 4.5)
    planet = uv_sphere("Vela", VELA_R, 128, 64)
    pm = bpy.data.materials.new("Vela")
    pm.use_nodes = True
    nt = pm.node_tree
    b = nt.nodes["Principled BSDF"]

    def tex(img, nc):
        t = nt.nodes.new("ShaderNodeTexImage")
        t.image = img
        if nc:
            img.colorspace_settings.name = "Non-Color"
        return t
    ta = tex(imgs["albedo"], False)
    nt.links.new(ta.outputs["Color"], b.inputs["Base Color"])
    tr = tex(imgs["rough"], True)
    nt.links.new(tr.outputs["Color"], b.inputs["Roughness"])
    tn = tex(nimg, True)
    nm = nt.nodes.new("ShaderNodeNormalMap")
    nm.inputs["Strength"].default_value = 1.0
    nt.links.new(tn.outputs["Color"], nm.inputs["Color"])
    nt.links.new(nm.outputs["Normal"], b.inputs["Normal"])
    b.inputs["Specular IOR Level"].default_value = 0.6
    planet.data.materials.append(pm)
    # cloud shell
    cl = uv_sphere("VelaClouds", VELA_R * 1.012, 128, 64)
    cm = bpy.data.materials.new("VelaClouds")
    cm.use_nodes = True
    cnt = cm.node_tree
    cb = cnt.nodes["Principled BSDF"]
    ct = cnt.nodes.new("ShaderNodeTexImage")
    ct.image = imgs["clouds"]
    cb.inputs["Base Color"].default_value = (0.95, 0.97, 1.0, 1)
    cb.inputs["Roughness"].default_value = 0.9
    cnt.links.new(ct.outputs["Color"], cb.inputs["Alpha"])
    cl.data.materials.append(cm)
    # atmosphere shell: fresnel rim, emission tinted, transparent centre
    at = uv_sphere("VelaAtmo", VELA_R * 1.05, 128, 64)
    am = bpy.data.materials.new("VelaAtmo")
    am.use_nodes = True
    ant = am.node_tree
    for n in list(ant.nodes):
        if n.type == "BSDF_PRINCIPLED":
            ant.nodes.remove(n)
    lw = ant.nodes.new("ShaderNodeLayerWeight")
    lw.inputs["Blend"].default_value = 0.35
    pw = ant.nodes.new("ShaderNodeMath")
    pw.operation = "POWER"
    ant.links.new(lw.outputs["Facing"], pw.inputs[0])
    pw.inputs[1].default_value = 4.0
    em = ant.nodes.new("ShaderNodeEmission")
    em.inputs["Color"].default_value = C.srgb("#5FB8E8")
    mul = ant.nodes.new("ShaderNodeMath")
    mul.operation = "MULTIPLY"
    ant.links.new(pw.outputs[0], mul.inputs[0])
    mul.inputs[1].default_value = 2.0
    geo = ant.nodes.new("ShaderNodeNewGeometry")
    dp = ant.nodes.new("ShaderNodeVectorMath")
    dp.operation = "DOT_PRODUCT"
    ant.links.new(geo.outputs["Normal"], dp.inputs[0])
    dp.inputs[1].default_value = tuple(-C.sun_dir())
    litm = ant.nodes.new("ShaderNodeMapRange")
    ant.links.new(dp.outputs["Value"], litm.inputs["Value"])
    litm.inputs["From Min"].default_value = -0.35
    litm.inputs["From Max"].default_value = 0.6
    litm.inputs["To Min"].default_value = 0.3  # forward-scattered limb glow stays visible on the night side
    lmul = ant.nodes.new("ShaderNodeMath")
    lmul.operation = "MULTIPLY"
    ant.links.new(mul.outputs[0], lmul.inputs[0])
    ant.links.new(litm.outputs[0], lmul.inputs[1])
    ant.links.new(lmul.outputs[0], em.inputs["Strength"])
    tr_ = ant.nodes.new("ShaderNodeBsdfTransparent")
    add = ant.nodes.new("ShaderNodeAddShader")
    ant.links.new(em.outputs[0], add.inputs[0])
    ant.links.new(tr_.outputs[0], add.inputs[1])
    ant.links.new(add.outputs[0], ant.nodes["Material Output"].inputs["Surface"])
    at.data.materials.append(am)
    ring = ring_object(rimg, VELA_R)
    cam = C.add_camera((0, -3600, 900), (0, 0, 0), lens=50, clip_end=100000)
    C.cam_lit(cam, (0, 0, 0), 4800, yaw_deg=55, pitch_deg=8, lens=45)
    C.render(os.path.join(C.PREVIEWS, "planet_vela.png"))
    # crescent view (terminator) - more cinematic, what a player sees from the inner belt
    C.cam_lit(cam, (0, 0, 0), 4400, yaw_deg=115, pitch_deg=10, lens=45)
    C.render(os.path.join(C.PREVIEWS, "planet_vela_terminator.png"))
    # star
    for ob in (planet, cl, at, ring):
        ob.hide_render = True
    s = uv_sphere("Cradle", STAR_R, 128, 64)
    smt = bpy.data.materials.new("Cradle")
    smt.use_nodes = True
    snt = smt.node_tree
    for n in list(snt.nodes):
        if n.type == "BSDF_PRINCIPLED":
            snt.nodes.remove(n)
    st = snt.nodes.new("ShaderNodeTexImage")
    st.image = simg
    # limb darkening
    lw2 = snt.nodes.new("ShaderNodeLayerWeight")
    lw2.inputs["Blend"].default_value = 0.5
    inv = snt.nodes.new("ShaderNodeMath")
    inv.operation = "SUBTRACT"
    inv.inputs[0].default_value = 1.15
    snt.links.new(lw2.outputs["Facing"], inv.inputs[1])
    se = snt.nodes.new("ShaderNodeEmission")
    snt.links.new(st.outputs["Color"], se.inputs["Color"])
    sm = snt.nodes.new("ShaderNodeMath")
    sm.operation = "MULTIPLY"
    snt.links.new(inv.outputs[0], sm.inputs[0])
    sm.inputs[1].default_value = 9.0
    snt.links.new(sm.outputs[0], se.inputs["Strength"])
    snt.links.new(se.outputs[0], snt.nodes["Material Output"].inputs["Surface"])
    s.data.materials.append(smt)
    # corona billboard (camera facing), 4x star diameter
    bpy.ops.mesh.primitive_plane_add(size=STAR_R * 2 * 4)
    q = bpy.context.active_object
    qm = bpy.data.materials.new("Corona")
    qm.use_nodes = True
    qnt = qm.node_tree
    for n in list(qnt.nodes):
        if n.type == "BSDF_PRINCIPLED":
            qnt.nodes.remove(n)
    qt = qnt.nodes.new("ShaderNodeTexImage")
    qt.image = cimg
    qe = qnt.nodes.new("ShaderNodeEmission")
    qnt.links.new(qt.outputs["Color"], qe.inputs["Color"])
    qmul = qnt.nodes.new("ShaderNodeMath")
    qmul.operation = "MULTIPLY"
    qnt.links.new(qt.outputs["Alpha"], qmul.inputs[0])
    qmul.inputs[1].default_value = 1.2
    qnt.links.new(qmul.outputs[0], qe.inputs["Strength"])
    qtr = qnt.nodes.new("ShaderNodeBsdfTransparent")
    qadd = qnt.nodes.new("ShaderNodeAddShader")
    qnt.links.new(qe.outputs[0], qadd.inputs[0])
    qnt.links.new(qtr.outputs[0], qadd.inputs[1])
    qnt.links.new(qadd.outputs[0], qnt.nodes["Material Output"].inputs["Surface"])
    q.data.materials.append(qm)
    cam.location = (0, -5200, 0)
    C.look_at(cam, (0, 0, 0))
    cam.data.lens = 40
    q.rotation_euler = (math.radians(90), 0, 0)
    q.location = (0, 700, 0)  # behind the disc
    C.render(os.path.join(C.PREVIEWS, "star_cradle.png"))
    for ob in (planet, cl, at, ring):
        ob.hide_render = False
    C.save_blend(os.path.join(C.SOURCE, "planet_star.blend"))


if __name__ == "__main__":
    main()

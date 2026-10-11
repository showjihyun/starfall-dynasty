"""Cradle skybox: procedural starfield + multi-layer nebula band, rendered as an equirect panorama.

v2 (2026-10-08, user request "배경 성운 좀 더 화려하게"): brighter nebula (peak display value
~0.45-0.5 instead of 0.12), four colour layers (teal, rose, violet/indigo, warm orange dust),
two bright nebula cores, dark absorption lanes, clustered star density. Diffraction spikes and
glows on the brightest stars are added afterwards by skybox_post.py (system Python + Pillow).

Run (three steps, in order):
  blender --background --factory-startup --python art/scripts/skybox.py          # base 8K -> textures/_tmp
  python art/scripts/skybox_post.py                                              # spikes -> final 8K + 4K
  SKY_STAGE=previews blender --background --factory-startup --python art/scripts/skybox.py
Out:  art/textures/skybox/cradle_sky_equirect.png (8192 x 4096, PC), cradle_sky_equirect_4k.png (Web)
      art/previews/skybox_view_{0..3}.png, skybox_equirect_small.png

Unity: import as Texture Shape = Cube, Mapping = Latitude-Longitude Layout; Skybox/Cubemap.
Foreground rule (01_art_direction 1.1) still holds: the sky stays below 1.0 so bloom (threshold
1.0) never fires on it, and the brightest nebula sits in a band, leaving darker sky elsewhere.
Deterministic: every noise/voronoi node uses fixed W offsets / fixed directions (no random module).
"""
import math
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import bpy
from mathutils import Euler, Vector

import common as C

OUT = os.path.join(C.TEX, "skybox")
BASE = os.path.join(C.TEX, "_tmp", "cradle_sky_base.png")
BAND_EULER = (math.radians(62), math.radians(18), 0.0)
# two bright cores on the band: (angle along the band in degrees, angular radius in radians, colour)
CORES = [(35.0, 0.30, "#F4C8A8"), (205.0, 0.22, "#E8A0C8")]


def node(nt, kind, **kw):
    n = nt.nodes.new(kind)
    for k, v in kw.items():
        setattr(n, k, v)
    return n


def band_point(deg):
    """Direction on the band's great circle (inverse of the band rotation)."""
    R = Euler(BAND_EULER, "XYZ").to_matrix()
    v = Vector((math.cos(math.radians(deg)), math.sin(math.radians(deg)), 0.0))
    return (R.transposed() @ v).normalized()


def build_world():
    w = bpy.data.worlds.new("CradleSky")
    bpy.context.scene.world = w
    w.use_nodes = True
    nt = w.node_tree
    L = nt.links
    bg = nt.nodes["Background"]
    d = node(nt, "ShaderNodeTexCoord").outputs["Generated"]  # world: view direction

    def math_(op, a, b=None, c=None):
        m = node(nt, "ShaderNodeMath", operation=op)
        for i, x in enumerate((a, b, c)):
            if x is None:
                continue
            if isinstance(x, (int, float)):
                m.inputs[i].default_value = x
            else:
                L.new(x, m.inputs[i])
        return m.outputs[0]

    def fbm(scale, w_, detail=12, rough=0.62, dist=0.35, vec=None):
        n = node(nt, "ShaderNodeTexNoise", noise_dimensions="4D")
        n.inputs["Scale"].default_value = scale
        n.inputs["Detail"].default_value = detail
        n.inputs["Roughness"].default_value = rough
        n.inputs["Distortion"].default_value = dist
        n.inputs["W"].default_value = w_
        L.new(vec or d, n.inputs["Vector"])
        return n.outputs["Fac"]

    def ramp(src, stops):
        r = node(nt, "ShaderNodeValToRGB")
        L.new(src, r.inputs["Fac"])
        els = r.color_ramp.elements
        els[0].position, els[0].color = stops[0]
        els[1].position, els[1].color = stops[-1]
        for pos, col in stops[1:-1]:
            e = els.new(pos)
            e.color = col
        return r.outputs["Color"]

    def vop(op, a, b=None, scale=None):
        n = node(nt, "ShaderNodeVectorMath", operation=op)
        for i, x in enumerate((a, b)):
            if x is None:
                continue
            if isinstance(x, tuple):
                n.inputs[i].default_value = x
            else:
                L.new(x, n.inputs[i])
        if scale is not None:
            if isinstance(scale, (int, float)):
                n.inputs["Scale"].default_value = scale
            else:
                L.new(scale, n.inputs["Scale"])
        return n.outputs[0]

    def smooth(src, a, b, lo=0.0, hi=1.0):
        m = node(nt, "ShaderNodeMapRange", interpolation_type="SMOOTHSTEP")
        L.new(src, m.inputs["Value"])
        m.inputs["From Min"].default_value = a
        m.inputs["From Max"].default_value = b
        m.inputs["To Min"].default_value = lo
        m.inputs["To Max"].default_value = hi
        return m.outputs[0]

    # --- band mask: tilted great circle, edges warped by low-frequency noise
    rot = node(nt, "ShaderNodeVectorRotate", rotation_type="EULER_XYZ")
    rot.inputs["Rotation"].default_value = BAND_EULER
    L.new(d, rot.inputs["Vector"])
    sep = node(nt, "ShaderNodeSeparateXYZ")
    L.new(rot.outputs[0], sep.inputs[0])
    warp = fbm(1.6, 3.1, detail=4, rough=0.5, dist=0.0)
    zz = math_("ADD", sep.outputs["Z"], math_("MULTIPLY", math_("SUBTRACT", warp, 0.5), 0.55))
    absz = math_("ABSOLUTE", zz)
    band = smooth(absz, 0.0, 0.5, 1.0, 0.0)       # wide soft band
    core_band = smooth(absz, 0.0, 0.2, 1.0, 0.0)  # narrow inner band
    edge = math_("MULTIPLY", band, math_("SUBTRACT", 1.0, core_band))  # band shoulders

    # --- bright cores (fixed directions on the band)
    core_total = None
    core_col = None
    for deg, rad, hexcol in CORES:
        c = band_point(deg)
        dp = node(nt, "ShaderNodeVectorMath", operation="DOT_PRODUCT")
        L.new(d, dp.inputs[0])
        dp.inputs[1].default_value = tuple(c)
        # gaussian in angle: exp(-(1 - dot) / (rad^2 / 2))
        g = math_("EXPONENT", math_("MULTIPLY", math_("SUBTRACT", 1.0, dp.outputs["Value"]), -2.0 / (rad * rad)))
        cc = vop("SCALE", None, scale=g)
        node_ = cc.node
        node_.inputs[0].default_value = C.srgb(hexcol)[:3]
        core_total = g if core_total is None else math_("ADD", core_total, g)
        core_col = cc if core_col is None else vop("ADD", core_col, cc)
    core_total = math_("MINIMUM", core_total, 1.0)

    # --- colour layers (linear values; peaks land around 0.45-0.5 after sRGB encoding)
    nA = fbm(2.2, 11.0)
    nB = fbm(3.4, 27.5, rough=0.66, dist=0.6)
    nC = fbm(1.4, 33.3, rough=0.6, dist=0.9)
    nD = fbm(5.0, 47.0, rough=0.7, dist=0.4)
    teal = ramp(nA, [(0.38, (0, 0, 0, 1)), (0.5, C.srgb("#12404A")), (0.62, C.srgb("#2A8C98")),
                     (0.74, C.srgb("#7CD8DC"))])
    rose = ramp(nB, [(0.46, (0, 0, 0, 1)), (0.58, C.srgb("#3A1830")), (0.7, C.srgb("#A8406A")),
                     (0.82, C.srgb("#F08AA8"))])
    violet = ramp(nC, [(0.36, (0, 0, 0, 1)), (0.5, C.srgb("#1A1648")), (0.64, C.srgb("#4A3496")),
                       (0.78, C.srgb("#8C70D8"))])
    orange = ramp(nD, [(0.42, (0, 0, 0, 1)), (0.54, C.srgb("#4A2410")), (0.66, C.srgb("#C8682A")),
                       (0.84, C.srgb("#F0A860"))])

    teal_k = vop("SCALE", teal, scale=math_("MULTIPLY_ADD", band, 0.75, 0.08))
    rose_k = vop("SCALE", rose, scale=math_("MULTIPLY_ADD", band, 0.85, 0.06))
    violet_k = vop("SCALE", violet, scale=math_("MULTIPLY_ADD", band, 0.4, 0.32))  # also off-band: indigo veil
    orange_k = vop("SCALE", orange, scale=math_("MULTIPLY", edge, 1.3))            # warm dust on the shoulders
    neb = vop("ADD", vop("ADD", teal_k, rose_k), vop("ADD", violet_k, orange_k))
    # cores: brighten the nebula around them and add their own glow
    boost = math_("MULTIPLY_ADD", core_total, 2.6, 1.0)
    neb = vop("SCALE", neb, scale=boost)
    core_glow = vop("SCALE", core_col, scale=math_("MULTIPLY", fbm(4.0, 61.0, detail=8), 1.1))
    neb = vop("ADD", neb, core_glow)

    # --- absorption lanes: filamentary dark dust, strongest in the band, never over the core centre
    lane_n = fbm(3.0, 41.0, detail=8, rough=0.6, dist=1.1)
    lane = smooth(lane_n, 0.46, 0.7, 0.0, 1.0)
    lane = math_("MULTIPLY", lane, math_("MULTIPLY_ADD", band, 0.85, 0.1))
    lane = math_("MULTIPLY", lane, math_("SUBTRACT", 1.0, math_("MULTIPLY", core_total, 0.6)))
    absorb = math_("SUBTRACT", 1.0, math_("MULTIPLY", lane, 0.78))
    neb = vop("SCALE", neb, scale=absorb)
    neb = vop("SCALE", neb, scale=0.55)

    # --- unresolved star glow along the band core
    glow = vop("SCALE", None, scale=math_("MULTIPLY", math_("POWER", core_band, 1.5), absorb))
    glow.node.inputs[0].default_value = C.srgb("#C8B49A")[:3]
    glow = vop("SCALE", glow, scale=0.045)

    # --- star layers: voronoi cell points kept with a probability modulated by a cluster field
    cluster = smooth(fbm(3.2, 77.0, detail=3, rough=0.5), 0.45, 0.7, 0.0, 1.0)

    def stars(scale, keep, radius, bright, w_, tint_mix=0.35, band_boost=0.0, cluster_boost=0.0):
        v = node(nt, "ShaderNodeTexVoronoi", voronoi_dimensions="4D", feature="F1")
        v.inputs["Scale"].default_value = scale
        v.inputs["W"].default_value = w_
        v.inputs["Randomness"].default_value = 1.0
        L.new(d, v.inputs["Vector"])
        cs = node(nt, "ShaderNodeSeparateColor")
        L.new(v.outputs["Color"], cs.inputs[0])
        thr = math_("ADD", keep, math_("ADD", math_("MULTIPLY", band, band_boost),
                                       math_("MULTIPLY", cluster, cluster_boost)))
        k = math_("LESS_THAN", cs.outputs[0], thr)
        sz = math_("MULTIPLY_ADD", cs.outputs[1], radius, radius * 0.35)
        prof = node(nt, "ShaderNodeMapRange", interpolation_type="SMOOTHSTEP")
        L.new(v.outputs["Distance"], prof.inputs["Value"])
        prof.inputs["From Min"].default_value = 0.0
        L.new(sz, prof.inputs["From Max"])
        prof.inputs["To Min"].default_value = 1.0
        prof.inputs["To Max"].default_value = 0.0
        b = math_("MULTIPLY_ADD", math_("POWER", cs.outputs[2], 3.0), bright, bright * 0.12)
        m2 = math_("MULTIPLY", math_("MULTIPLY", prof.outputs[0], b), k)
        bb = node(nt, "ShaderNodeBlackbody")
        L.new(math_("MULTIPLY_ADD", cs.outputs[1], 8500, 3500), bb.inputs[0])
        tint = node(nt, "ShaderNodeMix", data_type="RGBA", blend_type="MIX")
        tint.inputs["Factor"].default_value = tint_mix
        ai, bi, o = C._mix_rgba_sockets(tint)
        ai.default_value = (1, 1, 1, 1)
        L.new(bb.outputs[0], bi)
        return vop("SCALE", o, scale=m2)

    s_dust = stars(520, 0.3, 0.24, 0.5, 1.0, band_boost=0.5, cluster_boost=0.35)
    s_mid = stars(170, 0.24, 0.13, 0.9, 2.0, band_boost=0.25, cluster_boost=0.3)
    s_bright = stars(48, 0.2, 0.06, 1.0, 3.0, tint_mix=0.5, cluster_boost=0.15)
    s_hero = stars(12, 0.2, 0.024, 1.0, 4.0, tint_mix=0.55)

    col = vop("ADD", vop("ADD", neb, glow), vop("ADD", vop("ADD", s_dust, s_mid), vop("ADD", s_bright, s_hero)))
    col = vop("ADD", col, C.srgb("#04060C")[:3])
    L.new(col, bg.inputs["Color"])
    bg.inputs["Strength"].default_value = 1.0
    return w


def pano_camera(scene):
    cam = bpy.data.cameras.new("Pano")
    cam.type = "PANO"
    cam.panorama_type = "EQUIRECTANGULAR"
    ob = bpy.data.objects.new("Pano", cam)
    ob.rotation_euler = (math.radians(90), 0, 0)
    scene.collection.objects.link(ob)
    scene.camera = ob
    return cam, ob


def main():
    C.reset_scene()
    stage = os.environ.get("SKY_STAGE", "base")
    draft = bool(os.environ.get("SKY_DRAFT"))  # quick look: 4K base, fewer samples
    scene = C.setup_cycles(samples=32 if draft else 96, res=(4096, 2048) if draft else (8192, 4096))
    scene.cycles.use_denoising = False
    scene.view_settings.view_transform = "Standard"
    scene.view_settings.look = "None"
    scene.render.image_settings.color_depth = "8"
    scene.render.image_settings.compression = 90
    cam, ob = pano_camera(scene)
    os.makedirs(OUT, exist_ok=True)
    if stage == "base":
        build_world()
        C.render(BASE)
        C.save_blend(os.path.join(C.SOURCE, "skybox.blend"))
        return
    # previews: the FINAL post-processed panorama as the world, so what we look at is what Unity gets
    final = os.path.join(OUT, "cradle_sky_equirect.png")
    C.world_space("#04060C", 1.0, hdri=final)
    scene.cycles.samples = 16
    scene.render.resolution_x, scene.render.resolution_y = 1920, 1080
    cam.type = "PERSP"
    cam.lens = 24
    for i, yaw in enumerate((0, 90, 180, 270)):
        ob.rotation_euler = (math.radians(90), 0, math.radians(yaw))
        C.render(os.path.join(C.PREVIEWS, f"skybox_view_{i}.png"))
    scene.render.resolution_x, scene.render.resolution_y = 2048, 1024
    cam.type = "PANO"
    ob.rotation_euler = (math.radians(90), 0, 0)
    C.render(os.path.join(C.PREVIEWS, "skybox_equirect_small.png"))


if __name__ == "__main__":
    main()

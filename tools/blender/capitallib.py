"""STARFALL DYNASTY capital-ship helpers (cruisers 130-180 m, capitals 360-600 m).

Builds on shiplib.py / build_ships.py (imported, never modified).

Extra contracts on top of shiplib's:
  * Main_N   : bow main-gun muzzles (scale = bore radius).
  * Bridge   : first-person camera at the bridge windows (identity rotation).
  * Turret_N : separate mesh nodes, origin = rotation pivot, barrels +Y at rest,
               local +Z points away from the hull.  Child empty Muzzle_N at the
               barrel tip.  Turrets of one class share one mesh.
"""
import bpy
import bmesh
import math
import os
import random
import time
import numpy as np
from mathutils import Vector, Matrix

# build_ships reloads shiplib on import, so import it first and shiplib after.
import build_ships as bs
import shiplib
from shiplib import (V, H, L, G, GL, E, loft, ring10, seg_sym, circle_ring, lathe, box, prism, cyl,
                     sphere, wing, inset_faces, Ship, log, NB, MAT_NAMES)
from build_ships import nozzle, antenna, light, pipe, strip, vent, rcs, interp, lerp, canopy

PI = math.pi


# ================================================================ capital hull shader

def make_capital_hull(style):
    """Scale-aware procedural Hull material (bake source) for ships 100-600 m.
    Returns (material, socks) in shiplib's socks format."""
    m = shiplib._new_mat("Hull")
    nb = NB(m)
    nt = m.node_tree
    for n in list(nt.nodes):
        nt.nodes.remove(n)
    out = nb.node('ShaderNodeOutputMaterial')
    bsdf = nb.node('ShaderNodeBsdfPrincipled')
    tc = nb.node('ShaderNodeTexCoord')
    pos = tc.outputs['Object']
    geo = nb.node('ShaderNodeNewGeometry')
    sp = nb.node('ShaderNodeSeparateXYZ')
    nb.link(pos, sp.inputs[0])
    X, Y, Z = sp.outputs
    sn = nb.node('ShaderNodeSeparateXYZ')
    nb.link(geo.outputs['Normal'], sn.inputs[0])
    ANX, ANY, ANZ = [nb.math('ABSOLUTE', s) for s in sn.outputs]

    paint = style.get('paint', (0.3, 0.32, 0.33))
    paint2 = style.get('paint2', tuple(c * 0.82 for c in paint))
    accent = style.get('accent', paint2)
    sw = style.get('seam', 0.3)

    def white(a, b, c=0.0):
        cid = nb.node('ShaderNodeCombineXYZ')
        nb.val(a, cid.inputs[0])
        nb.val(b, cid.inputs[1])
        nb.val(c, cid.inputs[2])
        wn = nb.node('ShaderNodeTexWhiteNoise', noise_dimensions='3D')
        nb.link(cid.outputs[0], wn.inputs['Vector'])
        sep = nb.node('ShaderNodeSeparateColor')
        nb.link(wn.outputs['Color'], sep.inputs[0])
        return wn.outputs['Value'], sep.outputs[0]

    def grid(u, v, su, sv, w, stag=0.5, salt=0.0):
        vv = nb.math('DIVIDE', v, sv)
        row = nb.math('FLOOR', vv)
        uu = nb.math('ADD', nb.math('DIVIDE', u, su), nb.math('MULTIPLY', row, stag))
        fu = nb.math('FRACT', uu)
        fv = nb.math('FRACT', vv)
        eu = nb.math('MULTIPLY', nb.math('SUBTRACT', 0.5, nb.math('ABSOLUTE', nb.math('SUBTRACT', fu, 0.5))), su)
        ev = nb.math('MULTIPLY', nb.math('SUBTRACT', 0.5, nb.math('ABSOLUTE', nb.math('SUBTRACT', fv, 0.5))), sv)
        e = nb.math('MINIMUM', eu, ev)
        line = nb.mrange(e, w * 0.3, w, 1.0, 0.0, smooth=True)
        r1, r2 = white(nb.math('FLOOR', uu), row, salt)
        return line, r1, r2, fu, fv

    def hatch(fu, fv, su, sv, sel_rand, thresh, w):
        a = nb.math('MULTIPLY', nb.math('ABSOLUTE', nb.math('SUBTRACT', fu, 0.5)), su)
        b = nb.math('MULTIPLY', nb.math('ABSOLUTE', nb.math('SUBTRACT', fv, 0.5)), sv)
        er = nb.math('MINIMUM', nb.math('SUBTRACT', su * 0.3, a), nb.math('SUBTRACT', sv * 0.22, b))
        sel = nb.math('GREATER_THAN', sel_rand, thresh)
        area = nb.math('MULTIPLY', nb.mrange(er, 0.0, w * 0.6), sel)
        rline = nb.math('MULTIPLY', nb.mrange(nb.math('ABSOLUTE', er), w * 0.2, w * 0.55, 1.0, 0.0, smooth=True), sel)
        return area, rline

    organic = style.get('organic', False)
    if not organic:
        big = style.get('big', (12.0, 18.0, 7.0))
        small = style.get('small', (4.0, 4.5, 2.4))
        wZ = nb.mrange(ANZ, 0.42, 0.68, smooth=True)
        wX = nb.mrange(ANX, 0.42, 0.68, smooth=True)
        wY = nb.mrange(ANY, 0.42, 0.68, smooth=True)
        wsum = nb.math('ADD', nb.math('ADD', wZ, wX), nb.math('ADD', wY, 1e-3))
        planes = [  # weight, u, v, (spacing u big, v big), (u small, v small)
            (wZ, X, Y, (big[0], big[1]), (small[0], small[1])),
            (wX, Y, Z, (big[1], big[2]), (small[1], small[2])),
            (wY, X, Z, (big[0], big[2]), (small[0], small[2])),
        ]
        seams = 0.0
        pvar = 0.0
        harea = 0.0
        hline = 0.0
        for j, (w, u, v, sb, ss) in enumerate(planes):
            lb, rb, _, _, _ = grid(u, v, sb[0], sb[1], sw, 0.37, salt=j * 3.0)
            ls, rs, rs2, fu, fv = grid(u, v, ss[0], ss[1], sw * 0.55, 0.5, salt=j * 3.0 + 1.0)
            ha, hl = hatch(fu, fv, ss[0], ss[1], rs2, style.get('hatch', 0.8), sw * 0.55)
            wn = nb.math('DIVIDE', w, wsum)
            s_ = nb.math('MULTIPLY', nb.math('MAXIMUM', lb, nb.math('MULTIPLY', ls, style.get('small_amt', 0.55))), wn)
            p_ = nb.math('MULTIPLY', nb.math('ADD', nb.math('MULTIPLY', rb, 0.55), nb.math('MULTIPLY', rs, 0.45)), wn)
            seams = s_ if j == 0 else nb.math('ADD', seams, s_)
            pvar = p_ if j == 0 else nb.math('ADD', pvar, p_)
            ha_ = nb.math('MULTIPLY', ha, wn)
            hl_ = nb.math('MULTIPLY', hl, wn)
            harea = ha_ if j == 0 else nb.math('ADD', harea, ha_)
            hline = hl_ if j == 0 else nb.math('ADD', hline, hl_)
        # irregular sub-panels (sparse)
        vor = nb.node('ShaderNodeTexVoronoi', voronoi_dimensions='3D', feature='DISTANCE_TO_EDGE', distance='CHEBYCHEV')
        nb.link(pos, vor.inputs['Vector'])
        vor.inputs['Scale'].default_value = style.get('sub_scale', 0.12)
        vor.inputs['Randomness'].default_value = 0.9
        vline = nb.mrange(vor.outputs['Distance'], sw * 0.15 * 0.12 / style.get('sub_scale', 0.12),
                          sw * 0.5 * 0.12 / style.get('sub_scale', 0.12), 1.0, 0.0, smooth=True)
        seams = nb.math('MAXIMUM', seams, nb.math('MULTIPLY', vline, style.get('sub_amt', 0.35)))
        accent_m = nb.math('GREATER_THAN', pvar, style.get('accent_thresh', 0.9))
        base = nb.mix(nb.math('MULTIPLY', pvar, 0.45), (*paint, 1), (*paint2, 1))
        base = nb.mix(accent_m, base, (*accent, 1))
        base = nb.mix(nb.math('MULTIPLY', harea, 0.7), base, (*[c * 0.45 for c in paint], 1))
        height = nb.math('SUBTRACT', nb.math('MULTIPLY', pvar, 0.25), seams)
        height = nb.math('SUBTRACT', height, nb.math('ADD', nb.math('MULTIPLY', harea, 0.35), nb.math('MULTIPLY', hline, 0.8)))
        seams = nb.math('MAXIMUM', seams, nb.math('MULTIPLY', hline, 0.8))
    else:
        cell = style.get('cell', 6.0)
        vor = nb.node('ShaderNodeTexVoronoi', voronoi_dimensions='3D', feature='SMOOTH_F1', distance='EUCLIDEAN')
        nb.link(pos, vor.inputs['Vector'])
        vor.inputs['Scale'].default_value = 1.0 / cell
        vor.inputs['Smoothness'].default_value = 0.6
        scales = nb.mrange(vor.outputs['Distance'], 0.0, 0.55, 1.0, 0.0, smooth=True)
        rib = nb.math('SINE', nb.math('MULTIPLY', Y, 2 * PI / style.get('rib', 9.0)))
        rib = nb.mrange(rib, 0.25, 1.0, 0.0, 1.0, smooth=True)
        rmask = nb.mrange(ANY, 0.6, 0.9, 1.0, 0.0)
        rib = nb.math('MULTIPLY', rib, rmask)
        noise = nb.node('ShaderNodeTexNoise')
        nb.link(pos, noise.inputs['Vector'])
        noise.inputs['Scale'].default_value = 1.0 / (cell * 4)
        noise.inputs['Detail'].default_value = 4
        warp = nb.vmath('SCALE', noise.outputs['Color'])
        warp.node.inputs['Scale'].default_value = cell * 3
        veinv = nb.node('ShaderNodeTexVoronoi', voronoi_dimensions='3D', feature='DISTANCE_TO_EDGE')
        nb.link(nb.vmath('ADD', pos, warp), veinv.inputs['Vector'])
        veinv.inputs['Scale'].default_value = 1.0 / (cell * 3.5)
        vw = cell * 3.5
        seams = nb.mrange(veinv.outputs['Distance'], 0.15 / vw, 0.6 / vw, 1.0, 0.0, smooth=True)
        pvar = noise.outputs['Fac']
        base = nb.mix(nb.mrange(pvar, 0.35, 0.65), (*paint, 1), (*paint2, 1))
        base = nb.mix(nb.math('MULTIPLY', scales, 0.3), base, (*accent, 1))
        height = nb.math('ADD', nb.math('MULTIPLY', scales, 0.4), nb.math('MULTIPLY', rib, 0.5))
        height = nb.math('SUBTRACT', height, nb.math('MULTIPLY', seams, 1.2))

    # large-scale tonal blotches: tell the eye the plates are big and old
    blot0 = nb.node('ShaderNodeTexNoise')
    nb.link(pos, blot0.inputs['Vector'])
    blot0.inputs['Scale'].default_value = style.get('blot_scale', 0.012)
    blot0.inputs['Detail'].default_value = 3
    tone = nb.mrange(blot0.outputs['Fac'], 0.35, 0.65, 0.85, 1.12)
    tmul = nb.node('ShaderNodeMix', data_type='RGBA', blend_type='MULTIPLY')
    nb.val(1.0, tmul.inputs[0])
    nb.link(base, tmul.inputs[6])
    tcol = nb.node('ShaderNodeCombineColor')
    nb.link(tone, tcol.inputs[0])
    nb.link(tone, tcol.inputs[1])
    nb.link(tone, tcol.inputs[2])
    nb.link(tcol.outputs[0], tmul.inputs[7])
    base = tmul.outputs[2]

    # grime: AO + streaks along the hull
    ao = nb.node('ShaderNodeAmbientOcclusion')
    ao.samples = style.get('ao_samples', 6)
    ao.inputs['Distance'].default_value = style.get('ao_dist', 5.0)
    streak = nb.node('ShaderNodeTexNoise')
    smap = nb.node('ShaderNodeMapping')
    nb.link(pos, smap.inputs['Vector'])
    ss_ = style.get('streak', 0.35)
    smap.inputs['Scale'].default_value = (ss_, ss_ * 0.05, ss_)
    nb.link(smap.outputs[0], streak.inputs['Vector'])
    streak.inputs['Scale'].default_value = 1.0
    streak.inputs['Detail'].default_value = 6
    blot = nb.node('ShaderNodeTexNoise')
    nb.link(pos, blot.inputs['Vector'])
    blot.inputs['Scale'].default_value = style.get('blot2', 0.06)
    blot.inputs['Detail'].default_value = 8
    grime = nb.math('ADD', nb.mrange(ao.outputs['AO'], 0.35, 1.0, 1.0, 0.0),
                    nb.mrange(streak.outputs['Fac'], 0.52, 0.75, 0.0, 0.4))
    grime = nb.math('ADD', grime, nb.mrange(blot.outputs['Fac'], 0.55, 0.8, 0.0, 0.15))
    grime = nb.math('MULTIPLY', grime, style.get('grime', 1.0), clamp=True)
    # edge wear
    bev = nb.node('ShaderNodeBevel')
    bev.samples = 4
    bev.inputs['Radius'].default_value = style.get('wear_r', 0.12)
    edge = nb.vmath('DOT_PRODUCT', bev.outputs[0], geo.outputs['True Normal'])
    edge = nb.mrange(edge, 0.985, 0.85, 0.0, 1.0)
    wnoise = nb.node('ShaderNodeTexNoise')
    nb.link(pos, wnoise.inputs['Vector'])
    wnoise.inputs['Scale'].default_value = style.get('wear_noise', 1.2)
    wnoise.inputs['Detail'].default_value = 8
    wear = nb.math('MULTIPLY', edge, nb.mrange(wnoise.outputs['Fac'], 0.4, 0.6), clamp=True)
    wear = nb.math('MULTIPLY', wear, style.get('wear', 0.7), clamp=True)

    grime_col = style.get('grime_col', (0.05, 0.045, 0.04))
    col = nb.mix(nb.math('MULTIPLY', grime, 0.4), base, (*grime_col, 1))
    col = nb.mix(nb.math('MULTIPLY', seams, 0.7), col, (*[c * 0.25 for c in paint], 1))
    col = nb.mix(wear, col, (*style.get('bare', (0.55, 0.55, 0.56)), 1))

    r0 = style.get('rough', 0.5)
    rough = nb.math('ADD', r0, nb.math('MULTIPLY', nb.math('SUBTRACT', pvar, 0.5), 0.2))
    rough = nb.math('ADD', rough, nb.math('MULTIPLY', grime, 0.08))
    rough = nb.math('ADD', rough, nb.math('MULTIPLY', seams, 0.2))
    rough = nb.mixf(wear, rough, 0.28)
    rough = nb.math('MINIMUM', nb.math('MAXIMUM', rough, 0.05), 1.0)
    metal = nb.mixf(wear, style.get('metal', 0.35), 1.0)
    metal = nb.mixf(nb.math('MULTIPLY', grime, 0.4), metal, 0.1)

    fine = nb.node('ShaderNodeTexNoise')
    nb.link(pos, fine.inputs['Vector'])
    fine.inputs['Scale'].default_value = style.get('fine', 1.5)
    fine.inputs['Detail'].default_value = 4
    height = nb.math('ADD', height, nb.math('MULTIPLY', fine.outputs['Fac'], style.get('fine_amt', 0.03)))
    bump = nb.node('ShaderNodeBump')
    bump.inputs['Strength'].default_value = style.get('bump', 0.8)
    bump.inputs['Distance'].default_value = style.get('bump_dist', 0.06)
    nb.link(height, bump.inputs['Height'])

    nb.link(col, bsdf.inputs['Base Color'])
    nb.link(rough, bsdf.inputs['Roughness'])
    nb.link(metal, bsdf.inputs['Metallic'])
    nb.link(bump.outputs['Normal'], bsdf.inputs['Normal'])
    nb.link(bsdf.outputs[0], out.inputs['Surface'])
    em = nb.node('ShaderNodeEmission')
    socks = {'base': col, 'rough': rough, 'metal': metal, 'normal': bump.outputs['Normal'],
             'bsdf': bsdf, 'out': out, 'emit': em, 'grime': grime, 'wear': wear}
    return m, socks


def make_capital_materials(style):
    mats = shiplib.make_flat_materials()
    hull, socks = make_capital_hull(style)
    mats["Hull"] = hull
    return mats, socks


# ================================================================ UV unwrap with repack

def _hull_mask(o):
    idx = [i for i, s in enumerate(o.material_slots) if s.material and s.material.name == "Hull"]
    return [p.material_index in idx for p in o.data.polygons]


def cap_unwrap(objs, boost=None, margin=0.0015, angle=55, shape='CONCAVE'):
    """Smart-UV the Hull faces of all objs into one atlas, then repack concavely.
    boost: {object: texel-density multiplier} (e.g. instanced turret prototypes)."""
    boost = boost or {}
    t = time.time()
    sc = bpy.context.scene
    sc.tool_settings.use_uv_select_sync = True
    bpy.ops.object.select_all(action='DESELECT')
    for o in objs:
        o.select_set(True)
        me = o.data
        sel = _hull_mask(o)
        for v in me.vertices:
            v.select = False
        for e in me.edges:
            e.select = False
        for p, s in zip(me.polygons, sel):
            p.select = s
            if s:
                for vi in p.vertices:
                    me.vertices[vi].select = True
        me.update()
    bpy.context.view_layer.objects.active = objs[0]
    bpy.ops.object.mode_set(mode='EDIT')
    bpy.ops.mesh.select_mode(type='FACE')
    bpy.ops.uv.smart_project(angle_limit=math.radians(angle), island_margin=margin, area_weight=0.0,
                             correct_aspect=True, scale_to_bounds=False)
    bpy.ops.object.mode_set(mode='OBJECT')
    for o, f in boost.items():
        uv = o.data.uv_layers.active.data
        for p, s in zip(o.data.polygons, _hull_mask(o)):
            if s:
                for li in p.loop_indices:
                    uv[li].uv = uv[li].uv * f
    bpy.ops.object.mode_set(mode='EDIT')
    try:
        bpy.ops.uv.pack_islands(udim_source='CLOSEST_UDIM', rotate=True, rotate_method='ANY', scale=True,
                                merge_overlap=False, margin_method='FRACTION', margin=margin, shape_method=shape)
    except Exception as ex:
        log("pack_islands failed", ex)
    bpy.ops.object.mode_set(mode='OBJECT')
    tot = 0.0
    for o in objs:
        uv = o.data.uv_layers.active.data
        area = 0.0
        for p, s in zip(o.data.polygons, _hull_mask(o)):
            if not s:
                continue
            ls = list(p.loop_indices)
            a = uv[ls[0]].uv
            for j in range(1, len(ls) - 1):
                b, c = uv[ls[j]].uv, uv[ls[j + 1]].uv
                area += abs((b - a).cross(c - a)) / 2
        tot += area
        log("uv", o.name, "area %.4f" % area)
    log("unwrap+pack %.1fs, total uv area %.3f" % (time.time() - t, tot))


# ================================================================ bake (3 passes: base, packed MR, normal)

def cap_bake(objs, mats, socks, outdir, prefix, size=4096, samples=(16, 4, 4), mr_size=None, budget=30e6):
    os.makedirs(outdir, exist_ok=True)
    sc = bpy.context.scene
    sc.render.engine = 'CYCLES'
    sc.render.bake.margin = 16
    sc.render.bake.use_clear = True
    try:
        sc.cycles.use_auto_tile = True
        sc.cycles.tile_size = 1024
    except Exception as ex:
        log("tile setup", ex)
    sc.cycles.use_denoising = False
    hull = mats["Hull"]
    nt = hull.node_tree
    tex = nt.nodes.new('ShaderNodeTexImage')
    dummy = bpy.data.images.new("_dummy", 16, 16)
    tmp_nodes = []
    for mn in MAT_NAMES:
        if mn == "Hull":
            continue
        mt = mats[mn].node_tree
        nd = mt.nodes.new('ShaderNodeTexImage')
        nd.image = dummy
        mt.nodes.active = nd
        tmp_nodes.append((mt, nd))
    bpy.ops.object.select_all(action='DESELECT')
    for o in objs:
        o.select_set(True)
    bpy.context.view_layer.objects.active = objs[0]
    out, bsdf, em = socks['out'], socks['bsdf'], socks['emit']
    # packed metallic-roughness colour: R=1 G=rough B=metal
    cc = nt.nodes.new('ShaderNodeCombineColor')
    cc.inputs[0].default_value = 1.0
    nt.links.new(socks['rough'], cc.inputs[1])
    nt.links.new(socks['metal'], cc.inputs[2])
    imgs = {}

    def bake_emit(key, sock, noncolor, spp, sz=None):
        img = shiplib._img(prefix + "_" + key, sz or size, noncolor)
        tex.image = img
        nt.nodes.active = tex
        for lk in list(out.inputs['Surface'].links):
            nt.links.remove(lk)
        nt.links.new(sock, em.inputs['Color'])
        nt.links.new(em.outputs[0], out.inputs['Surface'])
        sc.cycles.samples = spp
        t = time.time()
        bpy.ops.object.bake(type='EMIT', margin=16, use_clear=True, target='IMAGE_TEXTURES')
        log("baked", key, sz or size, "%.1fs" % (time.time() - t))
        imgs[key] = img

    # 1) the expensive ray-traced terms (AO grime, bevel edge wear) once, at half size
    extra = []
    if 'grime' in socks and 'wear' in socks:
        cgw = nt.nodes.new('ShaderNodeCombineColor')
        nt.links.new(socks['grime'], cgw.inputs[0])
        nt.links.new(socks['wear'], cgw.inputs[1])
        bake_emit('gw', cgw.outputs[0], True, samples[0], sz=max(512, size // 2))
        gw = imgs.pop('gw')
        tgw = nt.nodes.new('ShaderNodeTexImage')
        tgw.image = gw
        sgw = nt.nodes.new('ShaderNodeSeparateColor')
        nt.links.new(tgw.outputs['Color'], sgw.inputs[0])
        for lk in list(nt.links):
            if lk.to_node == cgw:
                continue
            if lk.from_socket == socks['grime']:
                nt.links.new(sgw.outputs[0], lk.to_socket)
            elif lk.from_socket == socks['wear']:
                nt.links.new(sgw.outputs[1], lk.to_socket)
        nt.nodes.remove(cgw)
        extra = [tgw, sgw, gw]

    bake_emit('base', socks['base'], False, samples[1])
    bake_emit('mr', cc.outputs[0], True, samples[1], sz=mr_size or size)
    img = shiplib._img(prefix + "_normal", size, True)
    tex.image = img
    nt.nodes.active = tex
    for lk in list(out.inputs['Surface'].links):
        nt.links.remove(lk)
    nt.links.new(bsdf.outputs[0], out.inputs['Surface'])
    sc.cycles.samples = samples[2]
    t = time.time()
    bpy.ops.object.bake(type='NORMAL', normal_space='TANGENT', margin=16, use_clear=True, target='IMAGE_TEXTURES')
    log("baked normal %.1fs" % (time.time() - t))
    imgs['normal'] = img
    paths = {}
    sc.view_settings.view_transform = 'Standard'
    sc.view_settings.look = 'None'
    ims = sc.render.image_settings
    ims.file_format = 'PNG'
    ims.color_mode = 'RGB'
    ims.color_depth = '8'
    ims.compression = 100

    def save(key):
        im = imgs[key]
        p = os.path.join(outdir, "%s_%s.png" % (prefix, key))
        im.save_render(p, scene=sc)
        paths[key] = p
        log("saved", p, im.size[0], "%.1f MB" % (os.path.getsize(p) / 1e6))
    for key in ('base', 'mr', 'normal'):
        save(key)
    # keep the GLB under budget: shrink the normal map first, then the base colour
    for key in ('normal', 'base'):
        tot = sum(os.path.getsize(p) for p in paths.values())
        if tot <= budget:
            break
        w = imgs[key].size[0]
        imgs[key].scale(w // 2, w // 2)
        log("over budget %.1f MB -> %s to %d" % (tot / 1e6, key, w // 2))
        save(key)
    for mt, nd in tmp_nodes:
        mt.nodes.remove(nd)
    bpy.data.images.remove(dummy)
    for im in imgs.values():
        bpy.data.images.remove(im)
    nt.nodes.remove(tex)
    nt.nodes.remove(cc)
    if extra:
        nt.nodes.remove(extra[0])
        nt.nodes.remove(extra[1])
        bpy.data.images.remove(extra[2])
    return paths


# ================================================================ capital ship container (turrets)

MOUNTS = {
    'top': Matrix.Identity(4),
    'bottom': Matrix.Rotation(PI, 4, 'Y'),
    'right': Matrix.Rotation(PI / 2, 4, 'Y'),
    'left': Matrix.Rotation(-PI / 2, 4, 'Y'),
}


class Capital(Ship):
    def __init__(self, name, mats):
        super().__init__(name, mats)
        self.protos = {}
        self.placements = []
        self.turret_objs = []
        self.muzzles = []

    def proto(self, key, build, muzzle, muzzle_r, **kw):
        """A turret class: geometry built around its pivot (origin), barrels +Y."""
        g = "_proto_" + key
        obj = self.part(build, group=g, **kw)
        self.groups.pop(g)
        obj.name = "Proto_" + key
        obj.data.name = "Turret_" + key
        self.protos[key] = dict(obj=obj, muzzle=V(muzzle), r=muzzle_r)
        return obj

    def turret(self, key, loc, mount='top', tilt=0.0):
        M = Matrix.Translation(V(loc)) @ MOUNTS[mount]
        if tilt:
            M = M @ Matrix.Rotation(tilt, 4, 'Y')
        self.placements.append((key, M))

    def finalize(self):
        result = {}
        for gname, objs in self.groups.items():
            objs = [o for o in objs if o.name in bpy.data.objects]
            bpy.ops.object.select_all(action='DESELECT')
            for o in objs:
                o.select_set(True)
            bpy.context.view_layer.objects.active = objs[0]
            if len(objs) > 1:
                bpy.ops.object.join()
            o = bpy.context.view_layer.objects.active
            o.name = "Hull" if gname == "body" else gname
            o.data.name = o.name
            bpy.ops.object.material_slot_remove_unused()
            result[o.name] = o
        self.objects = result
        lo = V((1e9, 1e9, 1e9))
        hi = V((-1e9, -1e9, -1e9))

        def acc(p):
            nonlocal lo, hi
            lo = V((min(lo.x, p.x), min(lo.y, p.y), min(lo.z, p.z)))
            hi = V((max(hi.x, p.x), max(hi.y, p.y), max(hi.z, p.z)))
        for o in result.values():
            for v in o.data.vertices:
                acc(o.matrix_world @ v.co)
        for key, M in self.placements:
            for c in self.protos[key]['obj'].bound_box:
                acc(M @ V(c))
        c = (lo + hi) / 2
        T = Matrix.Translation(-c)
        for o in result.values():
            o.data.transform(T)
            o.data.update()
        for e in self.empties:
            e.location = e.location - c
        self.placements = [(k, T @ M) for k, M in self.placements]
        self.bbox = (lo - c, hi - c)
        self.size = hi - lo
        tl = getattr(self, 'target_len', None)
        if tl:
            f = tl / self.size.y
            S = Matrix.Scale(f, 4)
            for o in result.values():
                o.data.transform(S)
                o.data.update()
            for e in self.empties:
                e.location = e.location * f
                e.scale = e.scale * f
            self.placements = [(k, Matrix.Translation(M.to_translation() * f) @ M.to_3x3().to_4x4()) for k, M in self.placements]
            for p in self.protos.values():
                p['obj'].data.transform(S)
                p['obj'].data.update()
                p['muzzle'] = p['muzzle'] * f
                p['r'] *= f
            self.bbox = (self.bbox[0] * f, self.bbox[1] * f)
            self.size = self.size * f
            log(self.name, "rescaled x%.3f to length %.1f" % (f, self.size.y))
        log(self.name, "bbox size", tuple(round(x, 2) for x in (hi - lo)), "turrets", len(self.placements))
        # keep the prototypes out of the way (AO during bake)
        for i, p in enumerate(self.protos.values()):
            p['obj'].location = (self.size.x * 3 + 400 * (i + 1), 0, 0)
        return result

    def tri_count(self):
        n, per = Ship.tri_count(self)
        for key, p in self.protos.items():
            p['obj'].data.calc_loop_triangles()
            t = len(p['obj'].data.loop_triangles)
            cnt = sum(1 for k, _ in self.placements if k == key)
            per["Turret_" + key] = "%d x %d" % (t, cnt)
            n += t * cnt
        return n, per

    def bake_objects(self):
        return list(self.objects.values()) + [p['obj'] for p in self.protos.values()]

    def instantiate_turrets(self):
        for k, (key, M) in enumerate(self.placements):
            p = self.protos[key]
            o = bpy.data.objects.new("Turret_%d" % k, p['obj'].data)
            self.coll.objects.link(o)
            o.matrix_world = M
            e = bpy.data.objects.new("Muzzle_%d" % k, None)
            e.empty_display_type = 'SPHERE'
            e.empty_display_size = 1.0
            self.coll.objects.link(e)
            e.parent = o
            e.location = p['muzzle']
            e.scale = (p['r'],) * 3
            self.turret_objs.append(o)
            self.muzzles.append(e)
        for p in self.protos.values():
            bpy.data.objects.remove(p['obj'])

    def export(self, path):
        objs = list(self.objects.values()) + self.turret_objs
        shiplib.export_glb(path, objs, self.empties + self.muzzles)


# ================================================================ geometry helpers

def frame_matrix(ex, ey):
    """Rotation whose local X = ex, local Y = ey (orthonormalised), Z = X x Y."""
    ex = V(ex).normalized()
    ey = V(ey)
    ey = (ey - ex * ey.dot(ex)).normalized()
    ez = ex.cross(ey)
    return Matrix((ex, ey, ez)).transposed()


def obox(bm, c, size, ex, ey, mat=H):
    return box(bm, c, size, mat=mat, rot=frame_matrix(ex, ey))


def windows(bm, p0, p1, out, pitch=3.0, w=1.6, h=0.9, t=0.3, rng=None, skip=0.15, block=6, mat=G):
    """A row of lit windows along p0->p1 on a surface whose outward normal is `out`.
    Windows are grouped in blocks; some blocks are dark (skipped)."""
    p0, p1 = V(p0), V(p1)
    d = p1 - p0
    n = max(1, int(d.length / pitch))
    rng = rng or random.Random(7)
    R = frame_matrix(out, d)
    dark = False
    for i in range(n):
        if i % block == 0:
            dark = rng.random() < skip
        if dark or (i % block == block - 1):
            continue
        c = p0 + d * ((i + 0.5) / n) + V(out).normalized() * (t * 0.3)
        box(bm, c, (t, w, h), mat=mat, rot=R)


def gun_port(bm, c, r, axis=(0, 1, 0), depth=None, collar=1.5, n=28, mat_ring=G):
    """Bow main-gun muzzle: armoured collar proud of the face, deep bore with
    glowing accelerator rings and a hot core. Returns the muzzle point."""
    ax = V(axis).normalized()
    depth = depth or r * 2.2
    f = r * 0.7  # how far the collar stands proud of the face
    prof = [
        (r * collar * 1.08, -r * 0.6),
        (r * collar * 1.08, f * 0.35),
        (r * collar, f * 0.75),
        (r * 1.18, f),
        (r * 1.0, f * 0.9),
        (r * 1.0, f - depth * 0.3),
        (r * 0.96, f - depth * 0.33),
        (r * 0.96, f - depth * 0.42),
        (r * 1.0, f - depth * 0.45),
        (r * 1.0, f - depth * 0.62),
        (r * 0.96, f - depth * 0.65),
        (r * 0.96, f - depth * 0.74),
        (r * 1.0, f - depth * 0.77),
        (r * 1.0, f - depth),
        (r * 0.55, f - depth),
        (0.0, f - depth * 0.9),
    ]
    mats = [H, H, H, mat_ring, H, mat_ring, H, H, H, mat_ring, H, H, H, mat_ring, E]
    lathe(bm, c, ax, prof, n=n, mats=mats, cap_start=True, cap_end=False)
    # bolt ring on the collar face
    for i in range(12):
        a = 2 * PI * i / 12
        ref = V((0, 0, 1)) if abs(ax.z) < 0.9 else V((1, 0, 0))
        u = ax.cross(ref).normalized()
        v = ax.cross(u).normalized()
        p = c + ax * (f * 0.6) + (u * math.cos(a) + v * math.sin(a)) * (r * collar * 1.1)
        box(bm, p, (r * 0.18, r * 0.18, r * 0.18), rot=frame_matrix(u * math.cos(a) + v * math.sin(a), ax))
    return c + ax * f


def hangar(bm, wall_x, y, z, w, h, depth, side=1, frame=None, embed=1.0):
    """Hangar module on a +-X wall at x=wall_x: a block standing `depth` proud of
    the wall with a deep recessed bay, dark force-field back wall and landing lights."""
    c = V((wall_x + side * depth, y, z))
    frame = frame or min(w, h) * 0.12
    bx = box(bm, c - V((side * (depth + embed) * 0.5, 0, 0)), (depth + embed, w, h))
    face = max(bx['bands'][0], key=lambda f: f.calc_center_median().x * side)
    bmesh.ops.inset_individual(bm, faces=[face], thickness=frame, depth=0.0, use_even_offset=True)
    # push the inner face into the hull
    for v in face.verts:
        v.co.x -= side * depth * 0.85
    face.material_index = GL
    # deck lights along the bottom and top of the opening
    nl = max(3, int(w / (h * 0.35)))
    for i in range(nl):
        y = c.y - w / 2 + frame + (i + 0.5) * (w - 2 * frame) / nl
        box(bm, V((c.x + side * 0.15, y, c.z - h / 2 + frame * 0.5)), (0.3, (w - 2 * frame) / nl * 0.4, frame * 0.25), mat=G)
    strip(bm, V((c.x + side * 0.1, c.y - w / 2 + frame, c.z + h / 2 - frame * 0.6)),
          V((c.x + side * 0.1, c.y + w / 2 - frame, c.z + h / 2 - frame * 0.6)), 0.12, frame * 0.08)
    # interior light bars on the recessed walls
    for k in range(3):
        x = c.x - side * depth * (0.25 + 0.2 * k)
        strip(bm, V((x, c.y - w / 2 + frame + 0.05, c.z + h / 2 - frame - 0.3)),
              V((x, c.y + w / 2 - frame - 0.05, c.z + h / 2 - frame - 0.3)), 0.15, 0.15)


def resample(FS, step):
    """Insert intermediate stations so no band is longer than `step` (FS sorted by descending y)."""
    out = []
    for a, b in zip(FS, FS[1:]):
        n = max(1, int(round(abs(a[0] - b[0]) / step)))
        for i in range(n):
            t = i / n
            out.append(tuple(lerp(a[j], b[j], t) for j in range(len(a))))
    out.append(FS[-1])
    return out


def ring_se(y, hw, hh, cz=0.0, n=16, p=2.6, ridge=0.0, keel=0.0, cx=0.0, flat_bot=1.0):
    """Superellipse cross-section (n points, starts at top centre, clockwise seen from +Y).
    Segment k and n-1-k are mirrors. ridge/keel sharpen top/bottom centre."""
    pts = []
    for i in range(n):
        a = PI / 2 - 2 * PI * (i + 0.5) / n
        c, s = math.cos(a), math.sin(a)
        x = hw * math.copysign(abs(c) ** (2 / p), c)
        z = hh * math.copysign(abs(s) ** (2 / p), s)
        if z > 0:
            z += ridge * (1 - abs(x) / hw) ** 2
        else:
            z = z * flat_bot - keel * (1 - abs(x) / hw) ** 2
        pts.append(V((cx + x, y, cz + z)))
    return pts


def seg_se(k, n=16):
    return min(k, n - 1 - k)


def at(FS, y, idx):
    return interp(FS, y, idx)


def deck_z(FS, y):
    return interp(FS, y, 3) + interp(FS, y, 2)


def keel_z(FS, y):
    return interp(FS, y, 3) - interp(FS, y, 2)


def oct_ring(cy, z, hw, hd, ch=0.18, cx=0.0):
    """Chamfered rectangle (8 points) in the XY plane at height z."""
    c = min(hw, hd) * ch
    pts = [(hw, hd - c), (hw, -hd + c), (hw - c, -hd), (-hw + c, -hd), (-hw, -hd + c), (-hw, hd - c), (-hw + c, hd), (hw - c, hd)]
    return [V((cx + x, cy + y, z)) for x, y in pts]


def tier(bm, cy, z0, z1, hw0, hd0, hw1=None, hd1=None, ch=0.18, mat=H, dy1=0.0, matfn=None, caps=True):
    hw1 = hw0 if hw1 is None else hw1
    hd1 = hd0 if hd1 is None else hd1
    return loft(bm, [oct_ring(cy, z0, hw0, hd0, ch), oct_ring(cy + dy1, z1, hw1, hd1, ch)], mat=mat, matfn=matfn,
                cap_start=caps, cap_end=caps)


def rows_along(bm, FS, y0, y1, xfn, z, side, rng, **kw):
    """Window row on a +-X flank between y0>y1, following x = xfn(y) piecewise along FS stations."""
    ys = [y0] + [s[0] for s in FS if y1 < s[0] < y0] + [y1]
    ys = sorted(set(ys), reverse=True)
    for a, b in zip(ys, ys[1:]):
        if a - b < 2:
            continue
        windows(bm, (side * xfn(a), a, z), (side * xfn(b), b, z), (side, 0, 0), rng=rng, **kw)


def lattice_mast(bm, base, height, r, lights=True, yard=True):
    """Three-legged lattice antenna mast with cross bracing, yardarm and light."""
    b = V(base)
    legs = []
    for i in range(3):
        a = 2 * PI * i / 3 + PI / 2
        legs.append((V((math.cos(a), math.sin(a), 0)) * r, V((math.cos(a), math.sin(a), 0)) * r * 0.25))
    tr = r * 0.05
    for lo, hi in legs:
        cyl(bm, b + lo, b + hi + V((0, 0, height)), tr, tr * 0.7, n=6)
    nseg = 5
    for s in range(1, nseg):
        t = s / nseg
        pts = [b + lo.lerp(hi, t) + V((0, 0, height * t)) for lo, hi in legs]
        for i in range(3):
            cyl(bm, pts[i], pts[(i + 1) % 3], tr * 0.6, n=5)
    top = b + V((0, 0, height))
    cyl(bm, top, top + V((0, 0, height * 0.35)), tr * 0.6, tr * 0.3, n=6)
    if yard:
        y0 = top + V((0, 0, -height * 0.15))
        cyl(bm, y0 + V((-r * 1.4, 0, 0)), y0 + V((r * 1.4, 0, 0)), tr * 0.6, n=5)
        for sx in (-1, 1):
            cyl(bm, y0 + V((sx * r * 1.3, 0, 0)), y0 + V((sx * r * 1.3, 0, -height * 0.12)), tr * 0.35, n=4)
    if lights:
        sphere(bm, top + V((0, 0, height * 0.35)), tr * 2.2, mat=G, seg=8, rings=4)


def dish(bm, c, d, r, mat_rim=L):
    c = V(c)
    lathe(bm, c, d, [(0.0, -0.1 * r), (0.25 * r, -0.06 * r), (0.75 * r, 0.16 * r), (0.97 * r, 0.32 * r), (1.0 * r, 0.36 * r),
                     (0.93 * r, 0.33 * r), (0.7 * r, 0.2 * r), (0.2 * r, 0.04 * r), (0.0, 0.02 * r)], n=24,
          mats=[H, H, H, mat_rim, H, H, H, H])
    dn = V(d).normalized()
    cyl(bm, c, c + dn * r * 0.6, r * 0.03, n=6)
    sphere(bm, c + dn * r * 0.62, r * 0.06, mat=G, seg=8, rings=4)


def greeble_field(bm, rng, x0, x1, y0, y1, zfn, count, smin, smax, avoid=(), up=1, kinds=(0.5, 0.2, 0.15, 0.15),
                  mat_l=0.06):
    """Scatter boxes / stacked blocks / tanks / vents on a deck. zfn(x, y) -> deck height.
    avoid: list of (x, y, radius) circles kept clear. up=-1 for the keel."""
    for _ in range(count):
        for _try in range(8):
            x = rng.uniform(x0, x1)
            y = rng.uniform(y0, y1)
            if all((x - ax) ** 2 + (y - ay) ** 2 > ar ** 2 for ax, ay, ar in avoid):
                break
        else:
            continue
        z = zfn(x, y)
        s = rng.uniform(smin, smax)
        k = rng.random()
        mat = L if rng.random() < mat_l else H
        if k < kinds[0]:
            sx, sy, sz = s * rng.uniform(0.6, 1.6), s * rng.uniform(0.8, 2.6), s * rng.uniform(0.25, 0.7)
            box(bm, (x, y, z + up * sz / 2 - up * 0.05), (sx, sy, sz), mat=mat)
            if rng.random() < 0.4:
                box(bm, (x + rng.uniform(-0.2, 0.2) * sx, y + rng.uniform(-0.2, 0.2) * sy, z + up * (sz + s * 0.15)),
                    (sx * 0.5, sy * 0.4, s * 0.35))
        elif k < kinds[0] + kinds[1]:
            # tank pair along Y
            r = s * 0.28
            for sxo in (-1, 1):
                cyl(bm, (x + sxo * r * 1.15, y - s, z + up * r), (x + sxo * r * 1.15, y + s, z + up * r), r, n=10)
        elif k < kinds[0] + kinds[1] + kinds[2]:
            # slatted vent block
            nv = 5
            box(bm, (x, y, z + up * s * 0.12), (s * 1.2, s * 1.5, s * 0.24))
            for i in range(nv):
                t = (i + 0.5) / nv - 0.5
                box(bm, (x, y + t * s * 1.3, z + up * s * 0.3), (s * 1.05, s * 1.3 / nv * 0.45, s * 0.14))
        else:
            # dome / sensor blister
            sphere(bm, (x, y, z), s * 0.45, seg=12, rings=5, scale=(1, 1, 0.55 * up))


# ================================================================ turret prototypes (local frame, pivot at origin)

def turret_heavy(s, barrels=3, length=3.6, house_mat_fn=None):
    """Heavy gunhouse: barbette ring, faceted house, `barrels` long guns."""
    zb = 0.3 * s
    bz = zb + 0.42 * s
    L_ = length * s

    def build(bm):
        lathe(bm, (0, 0, 0), (0, 0, 1), [(0.0, -0.2 * s), (1.32 * s, -0.2 * s), (1.32 * s, 0.05 * s), (1.38 * s, 0.12 * s),
                                         (1.25 * s, zb), (0.0, zb)], n=40)
        rings = []
        for yy, w, h in ((-1.35, 1.12, 0.78), (-1.1, 1.18, 0.86), (0.55, 1.2, 0.88), (1.2, 0.95, 0.6)):
            pts = [(w, 0), (w, h * 0.62), (w * 0.72, h), (-w * 0.72, h), (-w, h * 0.62), (-w, 0)]
            rings.append([V((x * s, yy * s, zb + z_ * s)) for x, z_ in pts])
        res = loft(bm, rings)
        # roof hatches / rangefinder
        box(bm, (0, -0.6 * s, zb + 0.9 * s), (0.9 * s, 0.6 * s, 0.08 * s))
        box(bm, (0.55 * s, 0.1 * s, zb + 0.92 * s), (0.3 * s, 0.4 * s, 0.1 * s))
        prism(bm, (-1.45 * s, -0.4 * s, zb + 0.55 * s), (1.45 * s, -0.4 * s, zb + 0.55 * s), 0.14 * s, 0.14 * s)
        for sx in (-1, 1):
            cyl(bm, (sx * 1.45 * s, -0.4 * s, zb + 0.55 * s), (sx * 1.5 * s, -0.4 * s, zb + 0.55 * s), 0.2 * s, n=12, mat=GL)
        strip(bm, (-0.9 * s, -1.37 * s, zb + 0.5 * s), (0.9 * s, -1.37 * s, zb + 0.5 * s), 0.03 * s, 0.03 * s)
        # barrels
        for i in range(barrels):
            x = (i - (barrels - 1) / 2) * 0.55 * s
            prism(bm, (x, 0.9 * s, bz), (x, 1.55 * s, bz), 0.19 * s, 0.19 * s, w1=0.16 * s, h1=0.16 * s, chamfer=0.35)
            cyl(bm, (x, 1.5 * s, bz), (x, 1.5 * s + L_, bz), 0.105 * s, 0.085 * s, n=12)
            cyl(bm, (x, 1.5 * s, bz), (x, 1.5 * s + L_ * 0.35, bz), 0.13 * s, 0.115 * s, n=12)
            cyl(bm, (x, 1.5 * s + L_ - 0.35 * s, bz), (x, 1.5 * s + L_, bz), 0.125 * s, n=12)
            cyl(bm, (x, 1.5 * s + L_ - 0.01 * s, bz), (x, 1.5 * s + L_ + 0.01 * s, bz), 0.07 * s, n=12, mat=E)
    return build, (0, 1.5 * s + L_ + 0.02 * s, bz), 0.1 * s


def turret_medium(s, barrels=2, length=3.0):
    """Medium twin mount: rounded low house with sloped glacis."""
    zb = 0.2 * s
    bz = zb + 0.38 * s
    L_ = length * s

    def build(bm):
        lathe(bm, (0, 0, 0), (0, 0, 1), [(0.0, -0.15 * s), (1.05 * s, -0.15 * s), (1.1 * s, 0.08 * s), (1.0 * s, zb), (0.0, zb)], n=32)
        rings = []
        for yy, w, h in ((-1.05, 0.85, 0.62), (-0.85, 0.95, 0.72), (0.35, 0.98, 0.74), (0.95, 0.7, 0.42)):
            pts = [(w, 0), (w * 1.02, h * 0.5), (w * 0.8, h * 0.9), (w * 0.5, h), (-w * 0.5, h), (-w * 0.8, h * 0.9), (-w * 1.02, h * 0.5), (-w, 0)]
            rings.append([V((x * s, yy * s, zb + z_ * s)) for x, z_ in pts])
        loft(bm, rings)
        box(bm, (0.45 * s, -0.4 * s, zb + 0.78 * s), (0.3 * s, 0.35 * s, 0.12 * s))
        cyl(bm, (-0.4 * s, -0.5 * s, zb + 0.72 * s), (-0.4 * s, -0.5 * s, zb + 0.95 * s), 0.12 * s, n=10)
        sphere(bm, (-0.4 * s, -0.5 * s, zb + 0.97 * s), 0.05 * s, mat=G, seg=8, rings=4)
        for i in range(barrels):
            x = (i - (barrels - 1) / 2) * 0.5 * s
            prism(bm, (x, 0.75 * s, bz), (x, 1.2 * s, bz), 0.15 * s, 0.15 * s, chamfer=0.35)
            cyl(bm, (x, 1.15 * s, bz), (x, 1.15 * s + L_, bz), 0.085 * s, 0.07 * s, n=10)
            cyl(bm, (x, 1.15 * s + L_ - 0.3 * s, bz), (x, 1.15 * s + L_, bz), 0.1 * s, n=10)
            cyl(bm, (x, 1.15 * s + L_ - 0.01 * s, bz), (x, 1.15 * s + L_ + 0.01 * s, bz), 0.055 * s, n=10, mat=E)
    return build, (0, 1.15 * s + L_ + 0.02 * s, bz), 0.08 * s


def turret_lance(s, length=3.2):
    """Sleek single-barrel beam turret (Wren/Talon): low wedge house, long emitter."""
    zb = 0.15 * s
    bz = zb + 0.32 * s
    L_ = length * s

    def build(bm):
        lathe(bm, (0, 0, 0), (0, 0, 1), [(0.0, -0.12 * s), (0.95 * s, -0.12 * s), (1.0 * s, 0.05 * s), (0.9 * s, zb), (0.0, zb)], n=32)
        rings = []
        for yy, w, h in ((-1.2, 0.5, 0.4), (-0.9, 0.85, 0.62), (0.4, 0.8, 0.6), (1.3, 0.3, 0.3)):
            pts = [(w, 0), (w, h * 0.55), (w * 0.55, h), (-w * 0.55, h), (-w, h * 0.55), (-w, 0)]
            rings.append([V((x * s, yy * s, zb + z_ * s)) for x, z_ in pts])
        loft(bm, rings, matfn=lambda i, k: L if (i == 1 and k in (1, 3)) else H)
        prism(bm, (0, 0.9 * s, bz), (0, 1.5 * s, bz), 0.2 * s, 0.17 * s, w1=0.14 * s, h1=0.12 * s, chamfer=0.4)
        cyl(bm, (0, 1.45 * s, bz), (0, 1.45 * s + L_, bz), 0.09 * s, 0.075 * s, n=12)
        for k in range(4):
            y = 1.7 * s + k * L_ * 0.2
            cyl(bm, (0, y, bz), (0, y + 0.12 * s, bz), 0.13 * s, n=12, mat=G if k == 3 else H)
        cyl(bm, (0, 1.45 * s + L_, bz), (0, 1.45 * s + L_ + 0.02 * s, bz), 0.06 * s, n=12, mat=E)
        for sx in (-1, 1):
            box(bm, (sx * 0.7 * s, -0.5 * s, zb + 0.45 * s), (0.18 * s, 0.6 * s, 0.25 * s))
    return build, (0, 1.45 * s + L_ + 0.04 * s, bz), 0.075 * s


def turret_organic(s, length=2.2):
    """Veyr bio-turret: a bulb on a fleshy collar with a crescent lance and glowing eye."""
    def build(bm):
        lathe(bm, (0, 0, 0), (0, 0, 1), [(0.0, -0.2 * s), (1.0 * s, -0.2 * s), (1.05 * s, 0.05 * s), (0.85 * s, 0.25 * s), (0.0, 0.3 * s)], n=24)
        sphere(bm, (0, 0.1 * s, 0.75 * s), 0.75 * s, seg=20, rings=10, scale=(1.0, 1.35, 0.8))
        # lance: tapering horn
        lathe(bm, (0, 0.6 * s, 0.8 * s), (0, 1, 0), [(0.32 * s, 0.0), (0.28 * s, 0.6 * s), (0.18 * s, length * s * 0.6),
                                                       (0.1 * s, length * s), (0.0, length * s + 0.25 * s)], n=14)
        sphere(bm, (0, 0.95 * s, 1.05 * s), 0.18 * s, mat=G, seg=12, rings=6)
        for sx in (-1, 1):
            cyl(bm, (sx * 0.55 * s, 0.6 * s, 0.6 * s), (sx * 0.35 * s, 1.6 * s, 0.75 * s), 0.08 * s, 0.03 * s, n=8)
            sphere(bm, (sx * 0.62 * s, -0.1 * s, 0.95 * s), 0.08 * s, mat=G, seg=8, rings=4)
    return build, (0, 0.6 * s + length * s + 0.3 * s, 0.8 * s), 0.1 * s


# ================================================================ preview

def look_at(cam, eye, target, lens=35.0):
    cam.data.type = 'PERSP'
    cam.data.lens = lens
    cam.location = V(eye)
    d = (V(eye) - V(target)).normalized()
    cam.rotation_euler = d.to_track_quat('Z', 'Y').to_euler()
    cam.data.clip_start = 0.5
    cam.data.clip_end = 20000


def cap_preview(name, ship, mats, previews, build, closeup=None, hero_dir=(1.0, 1.2, 0.5)):
    os.makedirs(previews, exist_ok=True)
    lv = mats["Livery"].node_tree.nodes["Principled BSDF"]
    lv.inputs['Base Color'].default_value = (0.55, 0.06, 0.04, 1)
    gl = mats["Glow"].node_tree.nodes["Principled BSDF"]
    gl.inputs['Emission Color'].default_value = (0.35, 0.8, 1.0, 1)
    cam = shiplib.setup_preview_scene()
    allo = list(ship.objects.values()) + ship.turret_objs
    ranks = [o for n_, o in ship.objects.items() if n_.startswith("Rank")]
    kits = {n_: o for n_, o in ship.objects.items() if n_.startswith("Species_")}
    for o in ranks + list(kits.values()):
        o.hide_render = True
    shiplib.frame(cam, allo, hero_dir, margin=1.06)
    cam.data.clip_end = 50000
    shiplib.render(os.path.join(previews, "cap_%s.png" % name), 1280, 720, 64)
    tmp = os.path.join(build, "views")
    os.makedirs(tmp, exist_ok=True)
    views = []
    for o in ranks:
        o.hide_render = False
    sz = ship.size
    Lh = sz.y
    spec = [
        ((-1.0, 1.05, -0.35), False, None),     # front low: bow ports, keel
        ((-0.9, -1.1, 0.45), False, None),      # rear 3/4: engines
        ((1.0, 0.0, 0.0), True, None),          # side ortho
        ((0.0, 0.0001, 1.0), True, None),       # top ortho
        ((0.25, 1.0, 0.18), False, None),       # bow-on
        (None, False, closeup),                 # close-up along the deck
    ]
    for i, (d, ortho, cu) in enumerate(spec):
        p = os.path.join(tmp, "%s_v%d.png" % (name, i))
        if cu is None and d is None:
            lo, hi = ship.bbox
            cu = (V((hi.x * 0.95, hi.y - Lh * 0.1, hi.z * 0.85)), V((0, lo.y + Lh * 0.3, lo.z * 0.2)), 24)
        if cu is not None:
            look_at(cam, cu[0], cu[1], cu[2])
        else:
            shiplib.frame(cam, allo, d, ortho=ortho, margin=1.05)
            cam.data.clip_end = 50000
        shiplib.render(p, 640, 360, 40)
        views.append(p)
    shiplib.compose_sheet(views, os.path.join(previews, "cap_%s_sheet.png" % name), cols=3)
    # Species kits: one render each, same angle (docs/ART_DIRECTION.md §4).
    for o in ranks:
        o.hide_render = True
    kviews = []
    for kname in ("Species_Human_Kit", "Species_Veyr_Kit", "Species_Kharz_Kit"):
        for n_, o in kits.items():
            o.hide_render = n_ != kname
        p = os.path.join(tmp, "%s_%s.png" % (name, kname))
        shiplib.frame(cam, allo, hero_dir, margin=1.06)
        cam.data.clip_end = 50000
        shiplib.render(p, 640, 360, 40)
        kviews.append(p)
    for o in kits.values():
        o.hide_render = True
    shiplib.compose_sheet(kviews, os.path.join(previews, "cap_%s_species.png" % name), cols=3)

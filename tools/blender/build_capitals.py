"""Build STARFALL DYNASTY cruiser / capital hulls -> GLB + preview renders.

Usage (from repo root):
  "<blender.exe>" -b --factory-startup -P tools/blender/build_capitals.py -- [ships...] [--no-bake] [--no-preview] [--tex 4096]
Ships: <family>_cruiser / <family>_capital for family in wren talon bastion mule seer
       (default: all ten). A bare family name builds both of its hulls.
"""
import bpy
import bmesh
import math
import os
import random
import sys
import time
from mathutils import Vector, Matrix

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import capitallib as cl
from capitallib import (V, H, L, G, GL, E, loft, ring10, seg_sym, circle_ring, lathe, box, prism, cyl, sphere, wing,
                        inset_faces, log, nozzle, antenna, light, pipe, strip, vent, rcs, interp, lerp,
                        resample, at, deck_z, keel_z, tier, oct_ring, windows, rows_along, gun_port, hangar,
                        lattice_mast, dish, greeble_field, obox, frame_matrix, PI, ring_se, seg_se)
import shiplib
import build_ships as bs

REPO = os.path.abspath(os.path.join(HERE, "..", ".."))
MODELS = os.path.join(REPO, "crates", "client", "assets", "models")
PREVIEWS = os.path.join(HERE, "previews")
BUILD = os.path.join(HERE, "_build_capitals")


def inset_bands(bm, res, pick, thickness, depth):
    faces = []
    for i, band in enumerate(res['bands']):
        for k, f in enumerate(band):
            if pick(i, k):
                faces.append(f)
    if faces:
        inset_faces(bm, faces, thickness, depth)


# ================================================================ BASTION — dreadnought battleship (600 m)
#
# Hammerhead layout: a tall armoured bow "head" carrying the main-gun face and
# two superfiring heavy turrets, a narrower waist with a spine citadel, armoured
# broadside sponsons and the bridge tower, then a heavy aft block with two more
# heavy turrets, hangars and the engine bank.

def build_bastion_capital(ship):
    rng = random.Random(600)
    LH0 = [
        (300, 40, 26, 0),
        (294, 44, 29, 0),
        (272, 47, 31, 0),
        (200, 48, 31, 0),
        (188, 46, 30, 0),
        (176, 40, 24, -2),
        (164, 38, 22, -3),
        (-148, 39, 22, -3),
        (-160, 44, 27, 0),
        (-172, 47, 30, 0),
        (-262, 47, 30, 0),
        (-276, 44, 28, 0),
    ]
    LH = resample(LH0, 26)
    RK = dict(top=0.8, bot=0.82, hi=0.45, lo=-0.5)
    HEAD = (188, 300)
    AFT = (-276, -160)

    def ltop(y):
        return deck_z(LH, y)

    # ---------------- main hull
    def lower(bm):
        rings = [ring10(y, hw, hh, cz, **RK) for y, hw, hh, cz in LH]
        ys = [s[0] for s in LH]

        def mf(i, k):
            s = seg_sym(k)
            y = (ys[i] + ys[i + 1]) / 2 if i + 1 < len(ys) else ys[i]
            if 250 < y < 300 and s == 1:
                return L
            return H
        res = loft(bm, rings, matfn=mf)
        n = len(res['bands'])
        inset_bands(bm, res, lambda i, k: 1 <= i < n - 1 and (
            (seg_sym(k) == 2 and i % 2 == 0) or (seg_sym(k) == 4 and i % 3 == 1) or (seg_sym(k) == 3 and i % 2 == 1) or
            (seg_sym(k) == 1 and i % 4 == 2)), 1.4, -0.7)
        inset_faces(bm, [res['end']], 2.0, -1.5)
        inset_faces(bm, [res['start']], 1.6, -1.2)
    ship.part(lower, bevel=0.35, segs=2)

    # ---------------- spine citadel over the waist
    CT = [(186, 16, 6, 24), (172, 21, 9, 22), (-150, 22, 9, 22), (-166, 18, 7, 24)]
    CTr = resample(CT, 32)
    CK = dict(top=0.78, bot=0.9, hi=0.4, lo=-0.4)

    def ctop(y):
        return deck_z(CT, y)

    def citadel(bm):
        rings = [ring10(y, hw, hh, cz, **CK) for y, hw, hh, cz in CTr]
        res = loft(bm, rings)
        n = len(res['bands'])
        inset_bands(bm, res, lambda i, k: 1 <= i < n - 1 and ((seg_sym(k) == 2 and i % 2 == 1) or (seg_sym(k) == 1 and i % 3 == 2)),
                    1.0, -0.5)
        # transverse armour ribs on the spine flanks
        for y in range(150, -150, -30):
            for sx in (1, -1):
                box(bm, (sx * (at(CT, y, 1) + 0.4), y, 22), (1.4, 2.4, 13))
    ship.part(citadel, bevel=0.3, segs=2)

    # ---------------- broadside sponsons along the waist
    XO = 50.0
    SP = [(168, 0.0), (152, 1.0), (-134, 1.0), (-150, 0.0)]

    def sponsons(bm):
        rings = []
        for y, f in SP:
            xi = at(LH, y, 1) - 3
            xo = lerp(xi + 1, XO, f)
            pts = [(xi, lerp(-8, -16, f)), (xo - 4 * f, lerp(-6, -14, f)), (xo, lerp(-3, -8, f)), (xo, lerp(3, 6, f)),
                   (xo - 3 * f, lerp(5, 10, f)), (xi, lerp(6, 11, f))]
            rings.append([V((x, y, z)) for x, z in pts])
        res = loft(bm, rings)
        inset_bands(bm, res, lambda i, k: i == 1 and k in (2, 3, 4), 1.0, -0.5)
        for y in range(140, -134, -22):
            box(bm, (XO + 0.3, y, -1), (1.0, 2.0, 15))
        # top deck edge rail
        prism(bm, (XO - 2.5, 150, 10.4), (XO - 2.5, -132, 10.4), 0.5, 0.5)
    ship.part(sponsons, bevel=0.25, segs=2, mirror=True)

    # ---------------- bow: visor, chin, gun face, cheek armour
    mains = []

    def bow(bm):
        rings = []
        for y, w, t in ((310, 26, 1.8), (300, 34, 3.0), (276, 38, 3.4), (250, 36, 2.6)):
            zt = ltop(min(y, 300)) - 0.3
            pts = [(w, zt), (w + 1.4, zt + t * 0.6), (w * 0.86, zt + t), (0, zt + t + 1.0), (-w * 0.86, zt + t), (-w - 1.4, zt + t * 0.6), (-w, zt)]
            rings.append([V((x, y, z)) for x, z in pts])
        loft(bm, rings, matfn=lambda i, k: L if (i == 1 and k in (1, 4)) else H)
        rings = []
        for y, w in ((307, 26), (296, 30), (252, 36)):
            zb_ = keel_z(LH, min(y, 300)) + 0.3
            rings.append([V((w, y, zb_)), V((w * 0.8, y, zb_ - 2.6)), V((-w * 0.8, y, zb_ - 2.6)), V((-w, y, zb_))])
        loft(bm, rings)
        # gun face frame
        for z in (-0.5,):
            box(bm, (0, 300.4, z), (66, 1.0, 1.6))
        for x in (-8.5, 8.5):
            box(bm, (x, 300.4, 0), (1.6, 1.0, 40))
        for x in (-17.0, 0.0, 17.0):
            for z in (9.0, -9.5):
                mains.append(gun_port(bm, V((x, 300.0, z)), 5.2, depth=11.0, collar=1.42, n=32))
        # side cheek armour on the head
        for sx in (1, -1):
            rings = []
            for y in (298, 262, 226):
                x = at(LH, y, 1) + 0.2
                rings.append([V((sx * x, y, -13)), V((sx * (x + 1.8), y, -11.4)), V((sx * (x + 1.8), y, -1.6)), V((sx * x, y, 0))])
            loft(bm, rings, mat=L)
    ship.part(bow, bevel=0.2, segs=1)
    for i, m in enumerate(mains):
        ship.empty("Main_%d" % i, m, 5.2)

    # ---------------- deck armour plates
    PT = 0.8

    def plates(bm):
        for y0, y1 in ((296, 270), (262, 226), (220, 192)):
            bs.top_plate(bm, LH, y0, y1, PT, RK['top'], inset=0.92)
        for y0, y1 in ((-176, -206), (-210, -232), (-262, -273)):
            bs.top_plate(bm, LH, y0, y1, PT, RK['top'], inset=0.92)
        spans = [(170, 118), (116, 64), (62, 10), (8, -44), (-46, -98), (-100, -148)]
        for j, (y0, y1) in enumerate(spans):
            bs.top_plate(bm, CT, y0, y1, PT, CK['top'], inset=0.94 - (j % 2) * 0.05, mat=L if j == 2 else H)
    ship.part(plates, bevel=0.2, segs=1)

    # ---------------- engines
    engines = []

    def stern(bm):
        ys = -276.0
        box(bm, (0, ys - 2.5, 0), (78, 5, 44))
        spec = ([(x, 0.0, 12.0) for x in (-25, 0, 25)] + [(x, 17.5, 6.2) for x in (-31, -11, 11, 31)] +
                [(x, -17.5, 6.2) for x in (-31, -11, 11, 31)])
        for x, z, r in spec:
            exit_y = ys - 5 - r * 1.25
            cyl(bm, (x, ys - 4.5, z), (x, exit_y + r * 0.5, z), r * 1.22, r * 1.15, n=32)
            for k in range(3):
                yy = ys - 5.5 - k * r * 0.33
                cyl(bm, (x, yy, z), (x, yy - r * 0.1, z), r * 1.3, n=32, mat=L if (k == 0 and r > 10) else H)
            nozzle(bm, (x, exit_y, z), r, r * 1.6, n=32)
            engines.append(((x, exit_y, z), r))
        for sx in (1, -1):
            for k in range(6):
                box(bm, (sx * 48.5, -236 - k * 6, 0), (4, 1.4, 22))
    ship.part(stern, bevel=0.15, segs=1)
    engines.sort(key=lambda e: (-e[1], -e[0][2], e[0][0]))
    for i, (p, r) in enumerate(engines):
        ship.empty("Engine_%d" % i, p, r * 0.86)

    # ---------------- bridge tower on the spine
    TY = -100.0
    Z0 = ctop(TY) + PT

    def tower(bm):
        tier(bm, TY, Z0 - 1, Z0 + 16, 18, 28, 15.5, 24, ch=0.28)
        tier(bm, TY - 2, Z0 + 16, Z0 + 19, 16.5, 25, 16.5, 25, ch=0.28, mat=L)
        tier(bm, TY - 2, Z0 + 19, Z0 + 32, 13, 20, 11, 16, ch=0.3)
        tier(bm, TY + 2, Z0 + 32, Z0 + 34, 20, 12, 21, 13, ch=0.35)
        tier(bm, TY + 2, Z0 + 34, Z0 + 39, 21, 13, 20.6, 12.6, ch=0.35, mat=GL, caps=False)
        tier(bm, TY + 2, Z0 + 39, Z0 + 41, 20.6, 12.6, 18, 10, ch=0.35)
        for k in range(-7, 8):
            box(bm, (k * 2.5, TY + 2 + 13.0, Z0 + 36.5), (0.35, 0.5, 5.2))
        for sx in (1, -1):
            for k in range(-4, 5):
                box(bm, (sx * 21.0, TY + 2 + k * 2.4, Z0 + 36.5), (0.5, 0.35, 5.2))
        tier(bm, TY - 1, Z0 + 41, Z0 + 48, 9, 8, 7, 6, ch=0.3)
        prism(bm, (-15, TY - 1, Z0 + 45), (15, TY - 1, Z0 + 45), 1.3, 1.3)
        for sx in (1, -1):
            cyl(bm, (sx * 15, TY - 1, Z0 + 45), (sx * 15.6, TY - 1, Z0 + 45), 1.9, n=16, mat=GL)
        box(bm, (0, TY - 1, Z0 + 49.5), (5, 5, 3))
        # aft funnel / sensor block
        tier(bm, TY - 34, Z0 - 1, Z0 + 10, 12, 8, 9.5, 6, ch=0.3)
    ship.part(tower, bevel=0.15, segs=1)
    ship.empty("Bridge", (0, TY + 2 + 13.6, Z0 + 36.5), 1.0)

    def tower_details(bm):
        for sx in (1, -1):
            for z in (Z0 + 6, Z0 + 11):
                windows(bm, (sx * (18 - (z - Z0) * 0.15), TY + 22, z), (sx * (18 - (z - Z0) * 0.15), TY - 22, z), (sx, 0, 0),
                        pitch=3.0, w=1.3, h=0.7, rng=rng, skip=0.3)
            windows(bm, (sx * 12.5, TY + 14, Z0 + 25), (sx * 12.5, TY - 16, Z0 + 25), (sx, 0, 0), pitch=3.0, w=1.3, h=0.7, rng=rng)
        for z in (Z0 + 6, Z0 + 11):
            windows(bm, (-14, TY + 27.6 - (z - Z0) * 0.24, z), (14, TY + 27.6 - (z - Z0) * 0.24, z), (0, 1, 0), pitch=3.0, w=1.3, h=0.7, rng=rng, skip=0.2)
        windows(bm, (-10, TY - 2 + 19.0, Z0 + 25), (10, TY - 2 + 19.0, Z0 + 25), (0, 1, 0), pitch=3.0, w=1.3, h=0.7, rng=rng)
        lattice_mast(bm, (0, TY - 4, Z0 + 51), 30, 2.6)
        lattice_mast(bm, (0, TY - 34, Z0 + 10), 16, 1.8, yard=False)
        for sx in (1, -1):
            dish(bm, (sx * 7.0, TY - 6, Z0 + 48.5), (sx * 0.6, -0.3, 0.75), 4.0)
            antenna(bm, (sx * 5.5, TY + 4, Z0 + 48), 10, r=0.25, lean=(sx * 0.2, 0.1, 1))
            light(bm, (sx * 21.4, TY + 2, Z0 + 40.2), 0.6)
        sphere(bm, (0, TY - 34, Z0 + 11.5), 3.6, seg=16, rings=8, scale=(1, 1, 0.7))
    ship.part(tower_details, bevel=0.0, sharp_all=True, weighted=False)

    # ---------------- turrets
    hb, hm, hr = cl.turret_heavy(9.0, barrels=3, length=3.6)
    ship.proto("heavy", hb, hm, hr, bevel=0.2, segs=1)
    mb, mm, mr = cl.turret_medium(4.6, barrels=2, length=3.0)
    ship.proto("medium", mb, mm, mr, bevel=0.1, segs=1)

    def barbettes(bm):
        for y, top in ((212, ltop(212) + 9.0), (-240, ltop(-240) + 9.0)):
            lathe(bm, (0, y, ltop(y) - 1), (0, 0, 1), [(0.0, 0.0), (14.5, 0.0), (14.5, top - ltop(y) - 1.4), (13.6, top - ltop(y) + 1), (0.0, top - ltop(y) + 1)], n=48)
            for k in range(20):
                a = 2 * PI * k / 20
                box(bm, (14.6 * math.cos(a), y + 14.6 * math.sin(a), ltop(y) + 4), (1.2, 1.2, 7), rot=Matrix.Rotation(a, 3, 'Z'))
    ship.part(barbettes, bevel=0.15, segs=1)
    ship.turret("heavy", (0, 262, ltop(262) + PT))
    ship.turret("heavy", (0, 212, ltop(212) + 10.0))
    ship.turret("heavy", (0, -192, ltop(-192) + PT))
    ship.turret("heavy", (0, -240, ltop(-240) + 10.0))
    SHY = (140, 72, 4, -64, -128)
    for y in SHY:
        for sx in (1, -1):
            ship.turret("medium", (sx * 27, y, ltop(y)))
    SWY = (110, -30)
    for y in SWY:
        for sx in (1, -1):
            ship.turret("medium", (sx * XO, y, -1.0), mount='right' if sx > 0 else 'left')
    for y in (246, -222):
        for sx in (1, -1):
            ship.turret("medium", (sx * 21, y, keel_z(LH, y) - 0.2), mount='bottom')

    # ---------------- hangars, windows, lights
    def flanks(bm):
        for sx in (1, -1):
            hangar(bm, at(LH, -215, 1), -215, -1.0, 40, 12.0, 6.0, side=sx)
            # sponson wall: one sparse deck row, skipping turrets
            for y0, y1 in ((150, 122), (98, -18), (-42, -132)):
                windows(bm, (sx * XO, y0, 2.5), (sx * XO, y1, 2.5), (sx, 0, 0), pitch=3.6, w=1.4, h=0.7, rng=rng, skip=0.35)
            # spine flank row
            rows_along(bm, CT, 168, -148, lambda y: at(CT, y, 1), 25.0, sx, rng, pitch=3.4, w=1.3, h=0.7, skip=0.4, block=7)
            # head flank rows above the cheek armour
            for z in (7.0, 3.0):
                rows_along(bm, LH, 290, 196, lambda y: at(LH, y, 1), z, sx, rng, pitch=3.4, w=1.4, h=0.7, skip=0.35)
            # aft block row
            rows_along(bm, LH, -174, -192, lambda y: at(LH, y, 1), 7.0, sx, rng, pitch=3.4, w=1.4, h=0.7, skip=0.2)
            rows_along(bm, LH, -238, -262, lambda y: at(LH, y, 1), 7.0, sx, rng, pitch=3.4, w=1.4, h=0.7, skip=0.2)
            light(bm, (sx * XO, 152, 9), 0.9)
            light(bm, (sx * XO, -134, 9), 0.9)
            light(bm, (sx * 35, 310, ltop(300) + 2.0), 0.8)
            light(bm, (sx * 47.5, -272, 14), 0.8)
            for y in (286, -268):
                rcs(bm, (sx * (at(LH, y, 1) + 0.6), y, 11), (sx, 0, 0), 3.0)
        for y in range(270, -270, -45):
            light(bm, (0, y, keel_z(LH, y) - 0.4), 0.7)
    ship.part(flanks, bevel=0.0, sharp_all=True, weighted=False)

    # ---------------- greebles
    def greebles(bm):
        avoid_spine = [(0, TY, 32), (0, TY - 34, 14)]
        greeble_field(bm, rng, -16, 16, -148, 168, lambda x, y: ctop(y) + PT, 70, 1.0, 3.2, avoid=avoid_spine)
        greeble_field(bm, rng, -34, 34, 192, 296, lambda x, y: ltop(y) + PT, 40, 1.2, 3.5, avoid=[(0, 262, 15), (0, 212, 17)])
        greeble_field(bm, rng, -36, 36, -273, -174, lambda x, y: ltop(y) + PT, 40, 1.2, 3.5, avoid=[(0, -192, 15), (0, -240, 17)])
        for sx in (1, -1):
            av = [(sx * 27, y, 7) for y in SHY]
            greeble_field(bm, rng, sx * 23, sx * 31, -146, 162, lambda x, y: ltop(y), 22, 0.8, 2.4, avoid=av)
            greeble_field(bm, rng, sx * 40, sx * 47, -132, 150, lambda x, y: 10.4, 20, 0.8, 2.2)
        keel_av = [(21, 246, 7), (-21, 246, 7), (21, -222, 7), (-21, -222, 7)]
        greeble_field(bm, rng, -30, 30, -268, 292, lambda x, y: keel_z(LH, y), 110, 1.2, 4.0, avoid=keel_av, up=-1)
        for sx in (1, -1):
            pipe(bm, [(sx * 17.2, 168, ctop(168) + 1.3), (sx * 17.2, -148, ctop(-148) + 1.3)], r=0.8, n=8)
        for y in (60, -40):
            lattice_mast(bm, (10 if y > 0 else -10, y, ctop(y) + PT), 10, 1.2)
    ship.part(greebles, bevel=0.0, sharp_all=True, weighted=False)

    # ---------------- rank 9
    def r9_lance(bm):
        zc = keel_z(LH, 250) - 8.5
        rings = []
        for y, w, h in ((-176, 2.0, 2.0), (-160, 8.5, 5.5), (280, 8.5, 5.5), (298, 7.5, 4.8)):
            pts = [(w, 0), (w * 0.8, -h), (-w * 0.8, -h), (-w, 0), (-w * 0.8, h), (w * 0.8, h)]
            rings.append([V((x, y, zc + z)) for x, z in pts])
        res = loft(bm, rings, matfn=lambda i, k: L if (i == 1 and k in (0, 3)) else H)
        inset_bands(bm, res, lambda i, k: i == 1 and k in (1, 2, 4, 5), 0.8, -0.4)
        for y in range(-130, 280, 45):
            prism(bm, (0, y, zc), (0, y + 5, zc), 9.4, 6.2, chamfer=0.3)
        for sx in (1, -1):
            strip(bm, (sx * 8.6, -150, zc), (sx * 8.6, 276, zc), 0.25, 0.4)
        for y in (-110, 30, 170, 270):
            for sx in (1, -1):
                prism(bm, (sx * 5, y, zc + 4), (sx * 12, y + 8, keel_z(LH, y + 8) + 0.6), 1.2, 2.2)
        gun_port(bm, V((0, 298, zc)), 4.4, depth=8, collar=1.4, n=32)
    ship.part(r9_lance, group="Rank9_Spinal_Lance", bevel=0.2, segs=1)

    def r9_command(bm):
        z = Z0 + 52
        tier(bm, TY - 1, z - 1, z + 6, 6.5, 6.5, 5.2, 5.2, ch=0.3)
        tier(bm, TY - 1, z + 6, z + 8, 8.5, 8.5, 8.5, 8.5, ch=0.3, mat=L)
        dish(bm, (0, TY + 2, z + 9), (0, 0.5, 1), 8.5)
        for sx in (1, -1):
            prism(bm, (sx * 6, TY - 1, z + 3), (sx * 19, TY - 4, z + 12), 0.6, 0.9)
            dish(bm, (sx * 20, TY - 4, z + 12), (sx * 0.5, 0.4, 0.8), 3.4)
            lattice_mast(bm, (sx * 5, TY - 6, z + 8), 14, 1.2)
    ship.part(r9_command, group="Rank9_Command_Array", bevel=0.1, segs=1)


# ================================================================ BASTION — heavy cruiser (180 m)
#
# The step between the Bastion gunboat and the dreadnought: the gunboat's
# blade prow and stepped armour plates, a chin battery with the main guns,
# three triple turrets (one superfiring), bridge tower, broadside sponsons
# and a 2x2 engine block.

def build_bastion_cruiser(ship):
    rng = random.Random(180)
    WW, WH, WP = 0.8, 0.42, 2.3
    K = 4.8
    FS0 = [(18.0, 1.4, 0.7, -0.4), (15.5, 2.7, 1.1, -0.25), (12.0, 3.8, 1.45, -0.1), (7.0, 4.8, 1.75, 0.0), (1.0, 5.6, 1.95, 0.0),
           (-5.0, 6.1, 2.05, 0.0), (-11.0, 6.3, 2.05, 0.0), (-14.5, 6.1, 1.95, 0.0), (-16.2, 5.6, 1.8, 0.0)]
    FS = resample([(y * K, hw * K, hh * K * 1.55, cz * K) for y, hw, hh, cz in FS0], 6)
    RK = dict(top=0.8, bot=0.82, hi=0.45, lo=-0.5)
    ST = FS[-1][0]

    def ltop(y):
        return deck_z(FS, y)

    def hull(bm):
        rings = [ring10(19.2 * K, 0.35 * K, 0.4 * K * 1.5, -0.55 * K, top=0.6, bot=0.6, hi=0.4, lo=-0.4)]
        rings += [ring10(y, hw, hh, cz, **RK) for y, hw, hh, cz in FS]
        ys = [19.2 * K] + [s_[0] for s_ in FS]

        def mf(i, kk):
            y = (ys[i] + ys[i + 1]) / 2
            if 55 < y < 80 and seg_sym(kk) in (1, 2):
                return L
            return H
        res = loft(bm, rings, matfn=mf)
        n = len(res['bands'])
        inset_bands(bm, res, lambda i, kk: 2 <= i < n - 1 and ((seg_sym(kk) == 2 and i % 2 == 0) or (seg_sym(kk) == 4 and i % 3 == 1) or
                                                              (seg_sym(kk) == 3 and i % 2 == 1)), 0.6, -0.3)
        inset_faces(bm, [res['end']], 1.2, -1.0)
    ship.part(hull, bevel=0.15, segs=2)

    def plates(bm):
        spans = [(84, 58, 0.0), (60, 30, 0.25), (32, 4, 0.5), (6, -30, 0.75), (-28, -62, 1.0)]
        for j, (y0, y1, lift) in enumerate(spans):
            bs.top_plate(bm, FS, y0, y1, 0.8, RK['top'], inset=0.93 - j * 0.01, lift=lift, mat=L if j == 1 else H)
    ship.part(plates, bevel=0.1, segs=1)

    def side_plates(bm):
        for j, (y0, y1) in enumerate(((70, 40), (42, 8), (10, -24), (-21, -55), (-52, -75))):
            bs.side_plate(bm, FS, y0, y1, -8.0, 7.0, 1.1, off=j * 0.15, mat=L if j == 3 else H)
    ship.part(side_plates, bevel=0.1, segs=1, mirror=True)

    mains = []

    def chin(bm):
        zc = at(FS, 70, 3) - at(FS, 70, 2) - 2.5
        rings = []
        for y, w, h in ((86, 7, 3.2), (78, 9, 4.0), (50, 10, 4.2), (40, 6, 2)):
            rings.append([V((w, y, zc + h)), V((w, y, zc - h * 0.6)), V((w * 0.7, y, zc - h)), V((-w * 0.7, y, zc - h)), V((-w, y, zc - h * 0.6)), V((-w, y, zc + h))])
        loft(bm, rings)
        for x in (-3.6, 3.6):
            mains.append(gun_port(bm, V((x, 86.0, zc)), 1.9, depth=5, collar=1.4, n=24))
    ship.part(chin, bevel=0.1, segs=1)
    for i, m in enumerate(mains):
        ship.empty("Main_%d" % i, m, 1.9)

    EX, EZ, ER = 11.0, 4.6, 4.0
    engines = []

    def stern(bm):
        for sx in (EX, -EX):
            for sz in (EZ, -EZ):
                box(bm, (sx, ST - 2.5, sz), (9.5, 6, 8.6))
                nozzle(bm, (sx, ST - 11, sz), ER, 8.5, n=28)
                engines.append(((sx, ST - 11, sz), ER))
        nozzle(bm, (0, ST - 7, 0), 2.4, 5, n=20)
        engines.append(((0, ST - 7, 0), 2.4))
        box(bm, (0, ST - 2, 0), (7, 4, 14))
        for sx in (1, -1):
            for k_ in range(4):
                box(bm, (sx * (at(FS, -66, 1) + 1.6), -62 - k_ * 3.2, 0), (2, 0.7, 11))
    ship.part(stern, bevel=0.08, segs=1)
    engines.sort(key=lambda e: (-e[1], -e[0][2], e[0][0]))
    for i, (p, r) in enumerate(engines):
        ship.empty("Engine_%d" % i, p, r * 0.88)

    TY = -22.0
    Z0 = ltop(TY) + 1.0

    def tower(bm):
        tier(bm, TY, Z0 - 0.5, Z0 + 7, 9, 11, 7.5, 9, ch=0.28)
        tier(bm, TY, Z0 + 7, Z0 + 8.2, 8, 9.6, 8, 9.6, ch=0.28, mat=L)
        tier(bm, TY + 1, Z0 + 8.2, Z0 + 10.2, 10, 6, 10, 6, ch=0.35)
        tier(bm, TY + 1, Z0 + 10.2, Z0 + 12.4, 10, 6, 9.7, 5.7, ch=0.35, mat=GL, caps=False)
        tier(bm, TY + 1, Z0 + 12.4, Z0 + 13.4, 9.7, 5.7, 8, 4.6, ch=0.35)
        for k_ in range(-6, 7):
            box(bm, (k_ * 1.4, TY + 1 + 5.95, Z0 + 11.3), (0.18, 0.25, 2.3))
        tier(bm, TY - 1, Z0 + 13.4, Z0 + 17, 4.5, 4, 3.6, 3.2, ch=0.3)
        prism(bm, (-8, TY - 1, Z0 + 15.5), (8, TY - 1, Z0 + 15.5), 0.6, 0.6)
    ship.part(tower, bevel=0.06, segs=1)
    ship.empty("Bridge", (0, TY + 1 + 6.2, Z0 + 11.3), 1.0)

    hb, hm, hr = cl.turret_heavy(4.2, barrels=3, length=3.6)
    ship.proto("heavy", hb, hm, hr, bevel=0.08, segs=1)
    mb, mm, mr = cl.turret_medium(2.6, barrels=2, length=3.0)
    ship.proto("medium", mb, mm, mr, bevel=0.05, segs=1)

    def barbette(bm):
        y = 20
        lathe(bm, (0, y, ltop(y)), (0, 0, 1), [(0.0, 0.0), (6.6, 0.0), (6.6, 4.4), (6.2, 5.2), (0.0, 5.2)], n=36)
    ship.part(barbette, bevel=0.08, segs=1)
    ship.turret("heavy", (0, 48, ltop(48) + 0.5))
    ship.turret("heavy", (0, 20, ltop(20) + 5.2))
    ship.turret("heavy", (0, -56, ltop(-56) + 1.0))
    for y in (30, -6):
        for sx in (1, -1):
            x = at(FS, y, 1) * 0.8 - 1.5
            ship.turret("medium", (sx * x, y, ltop(y) + 0.5))
    ship.turret("medium", (0, 30, at(FS, 30, 3) - at(FS, 30, 2) - 0.1), mount='bottom')

    def details(bm):
        for sx in (1, -1):
            for z in (3.0, -1.0):
                rows_along(bm, FS, 70, -70, lambda y: at(FS, y, 1) + 1.1 + 0.15 * 2, z, sx, rng, pitch=WP, w=WW, h=WH, skip=0.35)
            for z in (Z0 + 2.5, Z0 + 5):
                windows(bm, (sx * 8.9, TY + 9, z), (sx * 8.9, TY - 9, z), (sx, 0, 0), pitch=WP, w=WW, h=WH, rng=rng, skip=0.1)
            rcs(bm, (sx * (at(FS, 70, 1) + 0.3), 72, 0), (sx, 0, 0), 1.2)
            light(bm, (sx * (at(FS, -70, 1) + 1.5), -70, 5), 0.4)
        lattice_mast(bm, (0, TY - 1, Z0 + 17), 10, 1.0)
        dish(bm, (3, TY - 2, Z0 + 16.5), (0.5, -0.3, 0.8), 1.8)
        light(bm, (0, 19.3 * K, -0.3 * K), 0.5)
        greeble_field(bm, rng, -18, 18, -74, 82, lambda x, y: ltop(y) + 0.8, 130, 0.5, 1.8,
                      avoid=[(0, 48, 7), (0, 20, 8), (0, -56, 7), (0, TY, 12)] + [(sx * (at(FS, y, 1) * 0.8 - 1.5), y, 4) for y in (30, -6) for sx in (1, -1)])
        greeble_field(bm, rng, -18, 18, -74, 80, lambda x, y: at(FS, y, 3) - at(FS, y, 2), 110, 0.5, 2.0, avoid=[(0, 30, 5)], up=-1)
    ship.part(details, bevel=0.0, sharp_all=True, weighted=False)

    def r6(bm):
        for sx in (1, -1):
            rings = []
            for y, w in ((40, 0.0), (32, 3.2), (-30, 3.6), (-44, 1.2)):
                x = at(FS, y, 1) + 0.6
                w = max(w, 0.6)
                pts = [(x, -7), (x + w, -4.5), (x + w * 1.1, 2.5), (x + w * 0.6, 5.5), (x, 6)]
                rings.append([V((sx * px, y, pz)) for px, pz in pts])
            loft(bm, rings)
            for y in (20, -12):
                x = at(FS, y, 1) + 4.2
                lathe(bm, (sx * x, y, 0.0), (sx, 0, 0), [(0.0, 0.0), (1.7, 0.0), (1.55, 1.0), (0.0, 1.0)], n=16)
                cyl(bm, (sx * (x + 0.4), y, 0.0), (sx * (x + 0.4), y + 9.0, 0.0), 0.3, n=10)
                cyl(bm, (sx * (x + 0.4), y + 8.9, 0.0), (sx * (x + 0.4), y + 9.0, 0.0), 0.2, n=10, mat=E)
    ship.part(r6, group="Rank6_Side_Armor", bevel=0.08, segs=1)


# ================================================================ WREN — needle scout cruiser (130 m) / imperial battlecruiser (360 m)
#
# Evolves the Wren fighter: needle prow with probe, forward canards, swept
# blades with tip engine pods, dorsal tail fin, one big central engine,
# white hull with red strakes. The capital adds a flowing dorsal sail with the
# bridge, stacked cheek gun pods and long elegant blades.

def build_wren_family(ship, cap):
    rng = random.Random(360 if cap else 130)
    k = (360.0 if cap else 130.0) / 360.0
    NR = 20
    WW, WH, WP = (1.3, 0.7, 3.2) if cap else (0.8, 0.42, 2.2)
    FS0 = [
        (176, 1.6, 1.2, 0), (150, 5.5, 4.0, 0.3), (115, 11, 7.5, 0.6), (70, 18, 11.5, 1.0), (20, 24, 14, 1.5),
        (-30, 27, 16, 1.5), (-80, 28, 16.5, 1.0), (-125, 26.5, 15.5, 0.5), (-158, 23, 13.5, 0), (-170, 21, 12, 0),
    ]
    FS = resample([(y * k, hw * k, hh * k, cz * k) for y, hw, hh, cz in FS0], 22 * k if cap else 5)
    TIP = 186 * k
    ST = FS[-1][0]
    RIDGE = 0.16

    def zt(y):
        return at(FS, y, 3) + at(FS, y, 2) * (1 + RIDGE * 0.85)

    def zk(y):
        return at(FS, y, 3) - at(FS, y, 2) * 1.05

    def hwy(y):
        return at(FS, y, 1)

    def zsurf(x, y, up=1):
        """Shell surface height at (x, y) on the superellipse section (p=2.4)."""
        hw, hh, cz = at(FS, y, 1), at(FS, y, 2), at(FS, y, 3)
        u = min(abs(x) / hw, 0.999)
        z = hh * (1 - u ** 2.4) ** (1 / 2.4)
        if up > 0:
            return cz + z + hh * RIDGE * (1 - u) ** 2
        return cz - z - hh * 0.08 * (1 - u) ** 2

    ys = [s[0] for s in FS]

    # ---------------- fuselage
    def fus(bm):
        rings = [V((0, TIP, 0))]
        for y, hw, hh, cz in FS:
            rings.append(ring_se(y, hw, hh, cz, n=NR, p=2.4, ridge=hh * RIDGE, keel=hh * 0.08))

        def mf(i, kk):
            s = seg_se(kk, NR)
            if i == 0:
                return H
            y = (ys[i - 1] + ys[min(i, len(ys) - 1)]) / 2
            if 95 * k < y < 140 * k and s in (3, 4):
                return L
            if -60 * k < y < 0 and s == 0:
                return L
            return H
        res = loft(bm, rings, matfn=mf)
        n = len(res['bands'])
        inset_bands(bm, res, lambda i, kk: 2 <= i < n - 1 and (
            (seg_se(kk, NR) in (4, 5) and i % 2 == 0) or (seg_se(kk, NR) in (7, 8) and i % 3 == 1) or
            (seg_se(kk, NR) == 2 and i % 4 == 3)), 0.9 * k + 0.2, -0.35 * k - 0.1)
        inset_faces(bm, [res['end']], 1.4 * k, -1.0 * k)
        # nose probe
        cyl(bm, (0, TIP - 1, 0), (0, TIP + 14 * k, 0), 0.5 * k + 0.1, 0.25 * k + 0.05, n=10)
        for yy in (TIP + 4 * k, TIP + 8 * k):
            cyl(bm, (0, yy, 0), (0, yy + 0.8 * k, 0), 0.9 * k + 0.15, n=10)
    ship.part(fus, bevel=0.25 * k + 0.05, segs=2, angle=40)

    # ---------------- dorsal command sail (both; taller and tiered on the capital)
    if cap:
        SA = [(62, 0.5, 0.5), (42, 6, 6), (12, 11, 15), (-30, 12.5, 22), (-78, 10.5, 19), (-114, 3, 4)]
    else:
        SA = [(52 * k, 0.4, 0.4), (36 * k, 3.4, 2.6), (10 * k, 5.6, 6.2), (-28 * k, 6.0, 7.0), (-62 * k, 4.6, 5.2), (-84 * k, 1.5, 1.5)]
    SH = max(h for _, _, h in SA)

    def sail_z(y):
        return zt(y) - 2.5 + interp([(a, c) for a, _, c in SA], y, 1) * 1.1

    def sail(bm):
        rings = []
        for y, w, h in SA:
            rings.append(ring_se(y, w, h, zt(y) - 2.5, n=16, p=2.2, ridge=h * 0.1, flat_bot=0.4))
        res = loft(bm, rings, matfn=lambda i, kk: L if (i == 2 and seg_se(kk, 16) == 6) else H)
        inset_bands(bm, res, lambda i, kk: i in (2, 3) and seg_se(kk, 16) in (3, 4, 5), 0.6 * k + 0.1, -0.3 * k - 0.05)
        # tier ledges along the sail flanks
        for fz in ((0.35, 0.65) if cap else (0.5,)):
            for sx in (1, -1):
                pts = []
                for y, w, h in SA[1:-1]:
                    pts.append(V((sx * (w * 0.98 + 0.4 * k), y, zt(y) - 2.5 + h * fz)))
                for a, b in zip(pts, pts[1:]):
                    prism(bm, a, b, 0.7 * k + 0.15, 0.35 * k + 0.1, chamfer=0.4)
    ship.part(sail, bevel=0.2 * k + 0.05, segs=2, angle=40)
    BY = -12 if cap else 8 * k
    bz = sail_z(BY) - (1.5 if cap else 0.6)

    def bridge(bm):
        n = 14
        w = 7.0 if cap else 3.4
        h = 3.6 if cap else 2.0
        y0 = BY
        rings = [V((0, y0 + w * 1.4, bz + h * 0.1))]
        for dy, sw, sh in ((0.6, 0.55, 0.6), (0.0, 0.95, 0.95), (-1.6, 1.0, 1.0), (-3.0, 0.8, 0.75), (-4.2, 0.3, 0.3)):
            rings.append(ring_se(y0 + dy * w, w * sw, h * sh, bz, n=n, p=2.2, flat_bot=0.2))

        def mf(i, kk):
            s = seg_se(kk, n)
            if i in (0, 1, 2) and s <= 3:
                return GL
            if i == 3 and s <= 2:
                return GL
            return H
        loft(bm, rings, matfn=mf)
        # frames over the glass
        for dy in (0.3, -0.8):
            yy = y0 + dy * w
            pts = ring_se(yy, w * 1.02 * (0.85 if dy > 0 else 1.0), h * 1.02 * (0.85 if dy > 0 else 1.0), bz, n=n, p=2.2, flat_bot=0.2)
            for a, b in zip(pts[:4], pts[1:5]):
                cyl(bm, a, b, 0.12 * w / 7, n=6)
            for a, b in zip(pts[-5:-1], pts[-4:]):
                cyl(bm, a, b, 0.12 * w / 7, n=6)
    ship.part(bridge, bevel=0.05, segs=1, angle=40)
    bh = 3.6 if cap else 2.0
    ship.empty("Bridge", (0, BY + (7.0 if cap else 3.4) * 0.5, bz + bh * 0.55), 1.0)

    # ---------------- side chines: a deck ledge along the flanks
    CH0, CH1 = 128 * k, -150 * k

    def chines(bm):
        rings = []
        cys = [y for y in ys if CH1 <= y <= CH0]
        cys = [CH0] + [y for y in cys if CH1 < y < CH0] + [CH1]
        for j, y in enumerate(cys):
            hw, hh, cz = at(FS, y, 1), at(FS, y, 2), at(FS, y, 3)
            ext = (3.2 * k + 0.6) * (0.25 if j in (0, len(cys) - 1) else 1.0)
            z = cz - hh * 0.12
            t = hh * 0.12
            rings.append([V((hw * 0.9, y, z - t)), V((hw + ext, y, z - t * 0.35)), V((hw + ext, y, z + t * 0.25)), V((hw * 0.9, y, z + t * 0.8))])
        res = loft(bm, rings, matfn=lambda i, kk: L if (kk == 1 and 2 <= i <= 3) else H)
    ship.part(chines, bevel=0.1 * k + 0.03, segs=1, mirror=True)

    # ---------------- canards
    def canards(bm):
        y = 112 * k
        x0 = hwy(y) * 0.8
        st = [dict(x=x0, z=0.0, le=y + 10 * k, te=y - 18 * k, t=3.0 * k),
              dict(x=x0 + 18 * k, z=2 * k, le=y + 16 * k, te=y - 2 * k, t=1.8 * k),
              dict(x=x0 + 32 * k, z=4 * k, le=y + 26 * k, te=y + 16 * k, t=1.0 * k)]
        wing(bm, st, matfn=lambda i, kk: L if i == 1 else H)
        light(bm, (x0 + 32.3 * k, y + 26 * k, 4 * k), 0.4 + 0.3 * k)
    ship.part(canards, bevel=0.1 * k + 0.03, segs=1, mirror=True)

    # ---------------- main blades with tip engine pods
    PX = 74 * k
    PZ = -5 * k
    WST = [dict(x=hwy(-30 * k) * 0.85, z=-2 * k, le=40 * k, te=-150 * k, t=7 * k),
           dict(x=44 * k, z=-3.5 * k, le=-40 * k, te=-158 * k, t=4.5 * k),
           dict(x=62 * k, z=-4.5 * k, le=-92 * k, te=-163 * k, t=3.0 * k),
           dict(x=PX - 4 * k, z=PZ, le=-112 * k, te=-166 * k, t=2.2 * k)]

    def wz(x):
        return interp([(s['x'], s['z'] + s['t'] * 0.48) for s in WST], x, 1)

    def wle(x):
        return interp([(s['x'], s['le']) for s in WST], x, 1)

    def wte(x):
        return interp([(s['x'], s['te']) for s in WST], x, 1)

    engines = []

    def blades(bm):
        res = wing(bm, WST, matfn=lambda i, kk: L if (i == 1 and kk in (0, 1, 6)) else H)
        inset_bands(bm, res, lambda i, kk: i in (0, 2) and kk in (1, 2), 0.6 * k + 0.15, -0.25 * k - 0.05)
        # tip pod
        pr = 6.5 * k
        lathe(bm, (PX, -168 * k, PZ), (0, 1, 0), [(pr * 0.9, 0.0), (pr, 6 * k), (pr, 52 * k), (pr * 0.8, 66 * k), (pr * 0.35, 76 * k), (0.0, 80 * k)], n=20,
              mats=[H, H, H, L, H])
        nozzle(bm, (PX, -172 * k, PZ), pr * 0.85, 9 * k, n=20)
        cyl(bm, (PX, -118 * k, PZ), (PX, -115 * k, PZ), pr * 1.06, n=20, mat=L)
        # down-turned tip fin
        st = [dict(x=PX, z=PZ - pr * 0.6, le=-130 * k, te=-160 * k, t=1.6 * k),
              dict(x=PX + 4 * k, z=PZ - 26 * k, le=-150 * k, te=-168 * k, t=1.0 * k)]
        wing(bm, st)
        light(bm, (PX + 4.2 * k, -150 * k, PZ - 26 * k), 0.4 + 0.3 * k)
        # blade leading-edge strake into the hull
        st = [dict(x=hwy(60 * k) * 0.7, z=-1 * k, le=80 * k, te=0, t=3 * k), dict(x=hwy(0) * 0.95, z=-2 * k, le=20 * k, te=-20 * k, t=2.5 * k)]
        wing(bm, st)
    ship.part(blades, bevel=0.12 * k + 0.04, segs=1, mirror=True)
    for sx in (1, -1):
        engines.append(((sx * PX, -172 * k, PZ), 6.5 * k * 0.85))

    # ---------------- dorsal tail fin
    def fin(bm):
        h = 42 * k
        st = [(zt(-100 * k) - 1, -95 * k, -168 * k, 6 * k), (zt(-120 * k) + h * 0.55, -138 * k, -176 * k, 3.5 * k),
              (zt(-140 * k) + h, -162 * k, -182 * k, 1.6 * k)]
        rings = []
        for z, le, te, t in st:
            ch = le - te
            pts = [(le, 0), (le - ch * 0.25, t / 2), (le - ch * 0.75, t * 0.4), (te, 0.05), (te, -0.05), (le - ch * 0.75, -t * 0.4), (le - ch * 0.25, -t / 2)]
            rings.append([V((dx, y, z)) for y, dx in pts])
        loft(bm, rings, matfn=lambda i, kk: L if i == 1 else H)
        light(bm, (0, -162 * k, zt(-140 * k) + h + 0.3), 0.4 + 0.3 * k)
        if cap:
            for z in (zt(-120 * k) + 8, zt(-120 * k) + 16):
                windows(bm, (1.6, -112 * k, z), (1.6, -130 * k, z), (1, 0, 0), pitch=WP, w=WW, h=WH, rng=rng, skip=0.0)
                windows(bm, (-1.6, -112 * k, z), (-1.6, -130 * k, z), (-1, 0, 0), pitch=WP, w=WW, h=WH, rng=rng, skip=0.0)
    ship.part(fin, bevel=0.08 * k + 0.03, segs=1)

    # ---------------- stern engines
    def stern(bm):
        r0 = at(FS, ST, 1) * 0.72
        nozzle(bm, (0, ST - 18 * k, at(FS, ST, 3)), r0, 20 * k, n=36)
        for a in range(10):
            ang = a * 2 * PI / 10 + PI / 10
            d = V((math.cos(ang), 0, math.sin(ang)))
            prism(bm, V((0, ST + 2 * k, at(FS, ST, 3))) + d * r0 * 1.05, V((0, ST - 15 * k, at(FS, ST, 3))) + d * r0 * 1.02, 0.6 * k + 0.1, 0.6 * k + 0.1)
        if cap:
            for sx in (1, -1):
                x = sx * at(FS, ST, 1) * 0.86
                z = -at(FS, ST, 2) * 0.62
                lathe(bm, (x, ST + 30 * k, z), (0, -1, 0), [(0.0, -4 * k), (5.5 * k, 0.0), (6.0 * k, 20 * k), (6.0 * k, 36 * k)], n=20)
                nozzle(bm, (x, ST - 12 * k, z), 5.0 * k, 8 * k, n=20)
                engines.append(((x, ST - 12 * k, z), 5.0 * k))
    ship.part(stern, bevel=0.1 * k + 0.03, segs=1)
    engines.insert(0, ((0, ST - 18 * k, at(FS, ST, 3)), at(FS, ST, 1) * 0.72))
    for i, (p, r) in enumerate(engines):
        ship.empty("Engine_%d" % i, p, r * 0.88)

    # ---------------- cheek gun pods (bow main guns)
    mains = []

    def guns(bm):
        pods = [(-5 * k, 1.0)] if not cap else [(3 * k, 1.0), (-9 * k, 0.85)]
        for z, sc_ in pods:
            y0, y1 = 64 * k, 128 * k
            r = 5.2 * k * sc_ + (0.6 if not cap else 0)
            x = hwy(96 * k) * 0.92 + r * 0.6
            lathe(bm, (x, y0 - 14 * k, z), (0, 1, 0), [(0.0, 0.0), (r * 0.7, 3 * k), (r, 12 * k), (r, (y1 - y0) * 0.75 + 14 * k), (r * 0.82, y1 - y0 + 14 * k)], n=20,
                  mats=[H, H, H, L])
            mains.append(gun_port(bm, V((x, y1, z)), r * 0.5, depth=r * 1.4, collar=1.45, n=20))
            for yy in (y0 + 6 * k, y0 + 20 * k):
                cyl(bm, (x, yy, z), (x, yy + 1.5 * k, z), r * 1.06, n=20, mat=G if yy > y0 + 10 * k else H)
    ship.part(guns, bevel=0.06 * k + 0.02, segs=1, mirror=True)
    mains_all = []
    for m in mains:
        mains_all += [m, V((-m.x, m.y, m.z))]
    for i, m in enumerate(mains_all):
        ship.empty("Main_%d" % i, m, (5.2 * k * 0.5 + 0.3) if not cap else 5.2 * k * 0.5)

    # ---------------- turrets
    ts = 6.0 if cap else 3.2
    tb, tm, tr = cl.turret_lance(ts, length=3.4)
    ship.proto("lance", tb, tm, tr, bevel=0.06 * ts, segs=1)
    if cap:
        for y in (104, 76):
            ship.turret("lance", (0, y * k, zt(y * k) - 0.6))
        for y in (-128, -152):
            for sx in (1, -1):
                x = hwy(y * k) * 0.42
                ship.turret("lance", (sx * x, y * k, zsurf(x, y * k) - 0.8))
        for x in (36, 48, 60):
            xx = x * k
            yy = (wle(xx) + wte(xx)) / 2 + 6
            for sx in (1, -1):
                ship.turret("lance", (sx * xx, yy, wz(xx) - 0.3))
        for y in (95, 25, -60, -120):
            ship.turret("lance", (0, y * k, zk(y * k) + 0.6), mount='bottom')
    else:
        ship.turret("lance", (0, 92 * k, zt(92 * k) - 0.3))
        for sx in (1, -1):
            x = hwy(-118 * k) * 0.45
            ship.turret("lance", (sx * x, -118 * k, zsurf(x, -118 * k) - 0.4))
        xx = 52 * k
        yy = (wle(xx) + wte(xx)) / 2 + 3
        for sx in (1, -1):
            ship.turret("lance", (sx * xx, yy, wz(xx) - 0.2))
        for y in (60, -60):
            ship.turret("lance", (0, y * k, zk(y * k) + 0.3), mount='bottom')

    # ---------------- windows / lights / greebles
    def details(bm):
        for sx in (1, -1):
            for zf in ((0.35, 0.0) if cap else (0.25, -0.05)):
                rows_along(bm, FS, 60 * k, -150 * k, lambda y: hwy(y) * (1.0 - 0.0) * 0.985, at(FS, 0, 3) + at(FS, 0, 2) * zf, sx, rng,
                           pitch=WP, w=WW, h=WH, skip=0.35)
            light(bm, (sx * (PX + 0.1), -100 * k, PZ + 6.5 * k), 0.4 + 0.3 * k)
            rcs(bm, (sx * hwy(140 * k) * 0.95, 140 * k, 0.4 * k), (sx, 0, 0), 1.6 * k + 0.4)
            rcs(bm, (sx * hwy(-150 * k) * 0.6, -150 * k, zt(-150 * k) - 0.4), (0, 0, 1), 1.6 * k + 0.4)
            pipe(bm, [(sx * hwy(-20 * k) * 0.5, 20 * k, zt(20 * k) - 0.8 * k), (sx * hwy(-120 * k) * 0.5, -120 * k, zt(-120 * k) - 0.8 * k)], r=0.5 * k + 0.1)
        for sx in (1, -1):
            for fz in ((0.25, 0.5, 0.78) if cap else (0.3, 0.72)):
                pts = [(sx * (w * 1.0), y, zt(y) - 2.5 + h * fz) for y, w, h in SA[2:-1]]
                for a, b in zip(pts, pts[1:]):
                    windows(bm, a, b, (sx, 0, 0), pitch=WP, w=WW, h=WH, rng=rng, skip=0.2)
        av = [(0, y * k, 8 * k) for y in (104, 76, 92)] + [(0, y[0], 14 * k) for y in SA]
        greeble_field(bm, rng, -9 * k, 9 * k, -155 * k, 120 * k, lambda x, y: zsurf(x, y) - 0.3, 50 if cap else 240,
                      0.6, 2.2 * k + 0.5, avoid=av, kinds=(0.55, 0.1, 0.25, 0.1))
        greeble_field(bm, rng, -9 * k, 9 * k, -150 * k, 120 * k, lambda x, y: zsurf(x, y, -1) + 0.3, 40 if cap else 200,
                      0.6, 2.2 * k + 0.5, avoid=[(0, y * k, 6 * k) for y in (95, 25, -60, -120, 60)], up=-1)
        antenna(bm, (2 * k, -95 * k, zt(-95 * k) - 0.2), 14 * k, r=0.2 * k + 0.05)
        light(bm, (0, TIP + 14 * k, 0), 0.3 + 0.3 * k)
    ship.part(details, bevel=0.0, sharp_all=True, weighted=False)

    # ---------------- rank parts
    if cap:
        def r9(bm):
            c = V((0, ST - 2 * k, at(FS, ST, 3)))
            R = at(FS, ST, 1) * 1.75
            prof = [(R * 0.93, -10 * k), (R, -3 * k), (R, 10 * k), (R * 0.93, 14 * k), (R * 0.88, 10 * k), (R * 0.88, -8 * k)]
            lathe(bm, c, (0, 1, 0), prof + [prof[0]], n=64, mats=[H, L, H, H, H, H], cap_start=False, cap_end=False)
            for a in (90, 210, 330):
                r = math.radians(a)
                d = V((math.cos(r), 0, math.sin(r)))
                prism(bm, c + d * at(FS, ST, 1) * 0.8, c + d * R * 0.9, 1.6 * k, 4 * k, chamfer=0.4, up=(0, 1, 0))
                p = c + d * (R * 1.2)
                lathe(bm, p + V((0, -8 * k, 0)), (0, 1, 0), [(5 * k, 0.0), (6 * k, 8 * k), (6 * k, 40 * k), (0.0, 54 * k)], n=20)
                nozzle(bm, p + V((0, -14 * k, 0)), 4.8 * k, 7 * k, n=20)
                prism(bm, c + d * R * 1.0, p, 1.4 * k, 1.4 * k, up=(0, 1, 0))
            for a in range(0, 360, 20):
                r = math.radians(a + 10)
                d = V((math.cos(r), 0, math.sin(r)))
                light(bm, c + d * R * 1.005 + V((0, 4 * k, 0)), 0.6)
        ship.part(r9, group="Rank9_Afterburner_Halo", bevel=0.1, segs=1)
    else:
        def r6(bm):
            # ventral missile pods + wing ECM pods
            for sx in (1, -1):
                x = sx * hwy(-40 * k) * 0.55
                z = zk(-40 * k) - 2.0
                lathe(bm, (x, -110 * k, z), (0, 1, 0), [(0.0, -3), (2.2, 0), (2.6, 4), (2.6, 60 * k), (1.6, 70 * k), (0.0, 74 * k)], n=16)
                for j in range(3):
                    cyl(bm, (x + (j - 1) * 0.9, -110 * k + 74 * k - 6, z - 1.6), (x + (j - 1) * 0.9, -110 * k + 74 * k - 4.5, z - 1.6), 0.4, n=8, mat=E)
                prism(bm, (x, -80 * k, z + 2), (x, -50 * k, z + 2), 0.6, 1.2)
                xx = sx * (PX - 18 * k)
                lathe(bm, (xx, wte(abs(xx)) + 2, wz(abs(xx)) + 2.2), (0, 1, 0), [(0.0, -2), (1.4, 0), (1.6, 3), (1.6, 34 * k), (0.0, 40 * k)], n=14)
                cyl(bm, (xx, wte(abs(xx)) + 20 * k, wz(abs(xx)) + 2.2), (xx, wte(abs(xx)) + 21 * k, wz(abs(xx)) + 2.2), 1.7, n=14, mat=G)
        ship.part(r6, group="Rank6_Strike_Pods", bevel=0.05, segs=1)


def build_wren_cruiser(ship):
    build_wren_family(ship, False)


def build_wren_capital(ship):
    build_wren_family(ship, True)


# ================================================================ TALON — destroyer (150 m) / battlecruiser (420 m)
#
# Evolves the Talon fighter: dark chamfered fuselage with a dorsal ridge, the
# twin nose cannons grown into two swept bow prongs with main-gun muzzles,
# forward-swept wings, twin engine nacelles with intakes, canted tail fins and
# ventral strakes, red livery bands. Rank 9 adds the dorsal rail cannon.

def build_talon_family(ship, cap):
    rng = random.Random(420 if cap else 150)
    k = (420.0 if cap else 150.0) / 420.0
    WW, WH, WP = (1.3, 0.7, 3.4) if cap else (0.8, 0.42, 2.3)
    RK = dict(top=0.42, bot=0.5, hi=0.32, lo=-0.42)
    FS0 = [(150, 14, 10, -2), (128, 20, 14, 0), (70, 26, 17, 1), (0, 30, 19, 2), (-60, 30, 19, 2), (-120, 27, 18, 3),
           (-170, 22, 15, 3), (-195, 18, 12, 3)]
    FS = resample([(y * k, a * k, b * k, c * k) for y, a, b, c in FS0], 24 * k if cap else 6)
    ys = [s_[0] for s_ in FS]
    FRONT, ST = FS[0][0], FS[-1][0]

    def zt(y):
        return at(FS, y, 3) + at(FS, y, 2) + 0.14 * at(FS, y, 2) * 0.5

    def zk(y):
        return at(FS, y, 3) - at(FS, y, 2)

    def hwy(y):
        return at(FS, y, 1)

    # ---------------- central fuselage
    def fus(bm):
        rings = [ring10(y, hw, hh, cz, ridge=hh * 0.14, keel=hh * 0.06, **RK) for y, hw, hh, cz in FS]

        def mf(i, kk):
            s = seg_sym(kk)
            y = (ys[i] + ys[i + 1]) / 2
            if s == 0 and -40 * k < y < 40 * k:
                return L
            if s == 1 and 90 * k < y < 140 * k:
                return L
            return H
        res = loft(bm, rings, matfn=mf)
        n = len(res['bands'])
        inset_bands(bm, res, lambda i, kk: 1 <= i < n - 1 and ((seg_sym(kk) == 2 and i % 2 == 1) or (seg_sym(kk) == 1 and i % 3 == 0) or
                                                               (seg_sym(kk) == 4 and i % 2 == 0)), 1.0 * k + 0.15, -0.45 * k - 0.05)
        inset_faces(bm, [res['end']], 1.6 * k, -1.2 * k)
        inset_faces(bm, [res['start']], 1.2 * k, -0.8 * k)
    ship.part(fus, bevel=0.3 * k + 0.05, segs=2)

    # ---------------- bow prongs
    PR0 = [(30, 13, 12, 24, 0), (80, 11, 10, 31, -1), (140, 9, 8, 39, -2), (190, 8, 6.5, 46, -3), (214, 7, 5.5, 49, -3.5)]
    PR = [(y * k, a * k, b * k, c * k, d * k) for y, a, b, c, d in PR0]
    mains = []

    def prongs(bm):
        rings = [ring10(y, hw, hh, cz, cx=cx, ridge=hh * 0.12, **RK) for y, hw, hh, cx, cz in reversed(PR)]
        res = loft(bm, rings, matfn=lambda i, kk: L if (i == 0 and seg_sym(kk) in (1, 2) and kk < 5) else H)
        inset_bands(bm, res, lambda i, kk: i in (1, 3) and kk in (1, 2, 3), 0.8 * k + 0.15, -0.4 * k - 0.05)
        inset_faces(bm, [res['start']], 0.6 * k, -0.3 * k)
        y, hw, hh, cx, cz = PR[-1]
        mains.append(gun_port(bm, V((cx, y, cz)), 3.6 * k + (0.4 if not cap else 0), depth=8 * k + 1, collar=1.38, n=28))
        # sensor strip & lights along the prong
        for yy in (110 * k, 170 * k):
            cxx = interp([(a[0], a[3]) for a in PR], yy, 1)
            light(bm, (cxx + interp([(a[0], a[1]) for a in PR], yy, 1) + 0.2, yy, 0), 0.3 + 0.4 * k)
    ship.part(prongs, bevel=0.25 * k + 0.05, segs=2, mirror=True)

    def bowguns(bm):
        for sx in ((-1, 1) if cap else (0,)):
            mains.append(gun_port(bm, V((sx * 6 * k, FRONT, -2 * k)), (3.5 if cap else 3.0) * k + (0.4 if not cap else 0),
                                  depth=10 * k + 1, collar=1.4, n=28))
    ship.part(bowguns, bevel=0.1 * k + 0.03, segs=1)
    allm = []
    for m in mains:
        if abs(m.x) > 1e-3 and m.x > 15 * k:
            allm += [m, V((-m.x, m.y, m.z))]
        else:
            allm.append(m)
    allm.sort(key=lambda p: (p.x, p.z))
    for i, m in enumerate(allm):
        ship.empty("Main_%d" % i, m, (3.6 if abs(m.x) > 15 * k else 3.4) * k + (0.4 if not cap else 0))

    # ---------------- forward-swept wings
    WST = [dict(x=hwy(-50 * k) * 0.9, z=-6 * k, le=-15 * k, te=-115 * k, t=8 * k),
           dict(x=58 * k, z=-7 * k, le=8 * k, te=-62 * k, t=5.5 * k),
           dict(x=90 * k, z=-9 * k, le=34 * k, te=-4 * k, t=3.6 * k),
           dict(x=100 * k, z=-10 * k, le=42 * k, te=12 * k, t=2.8 * k)]

    def wz(x):
        return interp([(s_['x'], s_['z'] + s_['t'] * 0.45) for s_ in WST], x, 1)

    def wmid(x):
        return (interp([(s_['x'], s_['le']) for s_ in WST], x, 1) + interp([(s_['x'], s_['te']) for s_ in WST], x, 1)) / 2

    def wings(bm):
        res = wing(bm, WST, matfn=lambda i, kk: L if i == 2 else H)
        inset_bands(bm, res, lambda i, kk: i in (0, 1) and kk in (1, 2), 0.8 * k + 0.1, -0.3 * k - 0.05)
        # wingtip weapon rail + light
        prism(bm, (101 * k, 46 * k, -10 * k), (101 * k, 0, -10 * k), 2.2 * k + 0.2, 2.6 * k + 0.2, w1=1.8 * k + 0.2, h1=2.2 * k + 0.2)
        light(bm, (101 * k, 46.5 * k, -10 * k), 0.4 + 0.4 * k)
        for j in range(4 if cap else 2):
            y = (40 - j * 9) * k
            cyl(bm, (103.5 * k, y, -10 * k), (103.5 * k, y - 6 * k, -10 * k), 1.1 * k + 0.1, n=10)
            cyl(bm, (103.5 * k, y, -10 * k), (103.5 * k, y + 0.3, -10 * k), 0.7 * k, n=10, mat=E)
    ship.part(wings, bevel=0.15 * k + 0.04, segs=1, mirror=True)

    # ---------------- engine nacelles
    NX, NZ = 50 * k, -2 * k
    NS = [(-30 * k, 10 * k, 10 * k, NZ), (-50 * k, 13 * k, 12.5 * k, NZ), (-150 * k, 14 * k, 13 * k, NZ), (-195 * k, 13 * k, 12 * k, NZ),
          (-205 * k, 12 * k, 11 * k, NZ)]
    engines = []

    def nacelles(bm):
        rings = [ring10(y, hw, hh, cz, cx=NX, top=0.5, bot=0.55, hi=0.4, lo=-0.4) for y, hw, hh, cz in NS]
        res = loft(bm, rings, matfn=lambda i, kk: L if (i == 2 and seg_sym(kk) in (0, 1) and kk < 5) else H)
        inset_faces(bm, [res['start']], 1.0 * k, -9 * k)
        inset_bands(bm, res, lambda i, kk: i in (1, 2) and kk in (2, 3, 7), 0.8 * k + 0.1, -0.35 * k - 0.05)
        er = 10.5 * k
        nozzle(bm, (NX, -222 * k, NZ), er, 18 * k, n=32)
        for a in range(8):
            ang = a * PI / 4 + PI / 8
            d = V((math.cos(ang), 0, math.sin(ang)))
            prism(bm, V((NX, -200 * k, NZ)) + d * er * 1.12, V((NX, -218 * k, NZ)) + d * er * 1.08, 0.7 * k + 0.1, 0.6 * k + 0.1)
        box(bm, (NX, -29 * k, NZ), (0.8 * k, 4 * k, 18 * k))
    ship.part(nacelles, bevel=0.25 * k + 0.05, segs=2, mirror=True)
    for sx in (1, -1):
        engines.append(((sx * NX, -222 * k, NZ), 10.5 * k))

    def stern(bm):
        xs = (-10, 0, 10) if cap else (0,)
        for x in xs:
            r = (5.5 if x == 0 else 4.0) * k + (0.4 if not cap else 0)
            nozzle(bm, (x * k, ST - 9 * k, at(FS, ST, 3)), r, 10 * k, n=24)
            engines.append(((x * k, ST - 9 * k, at(FS, ST, 3)), r))
    ship.part(stern, bevel=0.1 * k + 0.03, segs=1)
    engines.sort(key=lambda e: (-e[1], e[0][0]))
    for i, (p, r) in enumerate(engines):
        ship.empty("Engine_%d" % i, p, r * 0.88)

    # ---------------- canted fins on the nacelles + ventral strakes
    def fins(bm):
        st = [dict(x=NX + 2 * k, z=NZ + 11 * k, le=-120 * k, te=-195 * k, t=4 * k),
              dict(x=NX + 18 * k, z=NZ + 44 * k, le=-170 * k, te=-208 * k, t=2.6 * k),
              dict(x=NX + 26 * k, z=NZ + 60 * k, le=-192 * k, te=-214 * k, t=1.6 * k)]
        wing(bm, st, matfn=lambda i, kk: L if i == 1 else H)
        light(bm, (NX + 26.3 * k, -192 * k, NZ + 60 * k), 0.4 + 0.4 * k)
        st = [dict(x=NX + 2 * k, z=NZ - 11 * k, le=-130 * k, te=-190 * k, t=3 * k),
              dict(x=NX + 16 * k, z=NZ - 36 * k, le=-172 * k, te=-202 * k, t=2 * k)]
        wing(bm, st)
    ship.part(fins, bevel=0.1 * k + 0.03, segs=1, mirror=True)

    # ---------------- armoured bridge with wedge canopy
    BY = -118 * k
    bz0 = zt(BY)

    def bridge(bm):
        tier(bm, BY, bz0 - 1.5 * k, bz0 + 5 * k, 13 * k, 30 * k, 10 * k, 25 * k, ch=0.3)
        z = bz0 + 5 * k
        CS = [(BY + 26 * k, 0.0, 0.0, z), (BY + 20 * k, 5 * k, 3.5 * k, z), (BY + 10 * k, 8.5 * k, 6.5 * k, z),
              (BY - 4 * k, 9 * k, 7 * k, z), (BY - 16 * k, 6.5 * k, 5 * k, z), (BY - 24 * k, 0.0, 0.0, z + 1.5 * k)]
        bs.canopy(bm, CS, n=12)
        for yy in (BY + 12 * k, BY + 2 * k, BY - 8 * k):
            w = interp([(c[0], c[1]) for c in CS], yy, 1)
            h = interp([(c[0], c[2]) for c in CS], yy, 1)
            bs.canopy_frame(bm, yy, w, h, z - 0.05, n=12, t=0.35 * k + 0.05)
        prism(bm, (0, BY + 20 * k, z + 6.2 * k), (0, BY - 14 * k, z + 6.6 * k), 0.4 * k + 0.05, 0.4 * k + 0.05)
    ship.part(bridge, bevel=0.1 * k + 0.03, segs=1)
    ship.empty("Bridge", (0, BY + 12 * k, bz0 + 5 * k + 4.5 * k), 1.0)

    # ---------------- turrets
    ts = 4.8 if cap else 3.2
    mb, mm, mr = cl.turret_medium(ts, barrels=2, length=3.2)
    ship.proto("twin", mb, mm, mr, bevel=0.03 * ts, segs=1)

    def prong_top(y):
        hh = interp([(a[0], a[2]) for a in PR], y, 1)
        cz = interp([(a[0], a[4]) for a in PR], y, 1)
        return cz + hh, interp([(a[0], a[3]) for a in PR], y, 1)

    if cap:
        for y in (95, 30):
            for sx in (1, -1):
                ship.turret("twin", (sx * 12 * k, y * k, zt(y * k) - 1.6 * k))
        for y in (95, 165):
            z, cx = prong_top(y * k)
            for sx in (1, -1):
                ship.turret("twin", (sx * cx, y * k, z - 0.2))
        for y in (-85, -145):
            for sx in (1, -1):
                ship.turret("twin", (sx * NX, y * k, NZ + 13 * k - 0.2))
        for y in (90, 0, -90):
            ship.turret("twin", (0, y * k, zk(y * k) + 0.3), mount='bottom')
        for sx in (1, -1):
            ship.turret("twin", (sx * 74 * k, wmid(74 * k) + 6 * k, wz(74 * k) - 0.2))
    else:
        for sx in (1, -1):
            ship.turret("twin", (sx * 12 * k, 40 * k, zt(40 * k) - 1.6 * k))
            ship.turret("twin", (sx * NX, -110 * k, NZ + 13 * k - 0.2))
        for y in (70, -40):
            ship.turret("twin", (0, y * k, zk(y * k) + 0.2), mount='bottom')

    # ---------------- details
    def details(bm):
        for sx in (1, -1):
            for zf in ((0.15, -0.2) if cap else (0.0,)):
                rows_along(bm, FS, 130 * k, -185 * k, lambda y: hwy(y), at(FS, 0, 3) + at(FS, 0, 2) * zf, sx, rng,
                           pitch=WP, w=WW, h=WH, skip=0.35)
            # nacelle flank windows
            windows(bm, (sx * (NX + 14 * k), -60 * k, NZ + 2 * k), (sx * (NX + 14 * k), -170 * k, NZ + 2 * k), (sx, 0, 0),
                    pitch=WP, w=WW, h=WH, rng=rng, skip=0.45)
            rcs(bm, (sx * hwy(120 * k) * 0.96, 120 * k, 2 * k), (sx, 0, 0), 2.0 * k + 0.3)
            rcs(bm, (sx * hwy(-180 * k) * 0.5, -180 * k, zt(-180 * k) - 0.2), (0, 0, 1), 2.0 * k + 0.3)
            pipe(bm, [(sx * 6 * k, 70 * k, zt(70 * k) - 1.0 * k), (sx * 6 * k, -90 * k, zt(-90 * k) - 1.0 * k)], r=0.6 * k + 0.08)
            vent(bm, (sx * 16 * k, -60 * k, zt(-60 * k) - 2.6 * k), 8 * k, 14 * k, n=6, h=0.8 * k)
        av = [(sx * 12 * k, y * k, 7 * k) for y in (95, 30, 40) for sx in (1, -1)] + [(0, BY, 30 * k)]
        greeble_field(bm, rng, -16 * k, 16 * k, -185 * k, 140 * k, lambda x, y: zt(y) - 0.6 * k - abs(x) * 0.12,
                      60 if cap else 80, 0.5, 2.4 * k + 0.6, avoid=av)
        greeble_field(bm, rng, -14 * k, 14 * k, -185 * k, 140 * k, lambda x, y: zk(y) + 0.3, 50 if cap else 70,
                      0.5, 2.4 * k + 0.6, avoid=[(0, y * k, 7 * k) for y in (90, 0, -90, 70, -40)], up=-1)
        antenna(bm, (4 * k, -150 * k, zt(-150 * k) - 0.4), 14 * k, r=0.25 * k + 0.05)
        antenna(bm, (-3 * k, -160 * k, zt(-160 * k) - 0.4), 9 * k, r=0.2 * k + 0.05, tip=False)
    ship.part(details, bevel=0.0, sharp_all=True, weighted=False)

    # ---------------- rank parts
    if cap:
        def r9(bm):
            z = zt(0) + 6 * k
            prism(bm, (0, -95 * k, zt(-95 * k) + 5 * k), (0, 10 * k, z), 10 * k, 6 * k, w1=8 * k, h1=5 * k)
            for sx in (-1, 1):
                prism(bm, (sx * 3.4 * k, 10 * k, z + 1 * k), (sx * 3.4 * k, 205 * k, z + 2.5 * k), 1.3 * k, 2.4 * k, chamfer=0.3)
            for j in range(9):
                y = (25 + j * 19) * k
                prism(bm, (0, y, z + 1.5 * k), (0, y + 5 * k, z + 1.8 * k), 6.5 * k, 3.8 * k, chamfer=0.5)
                prism(bm, (0, y + 5.2 * k, z + 1.8 * k), (0, y + 6.5 * k, z + 1.8 * k), 5.5 * k, 3.0 * k, mat=G)
            cyl(bm, (0, 200 * k, z + 2.4 * k), (0, 206 * k, z + 2.4 * k), 1.8 * k, n=16, mat=E)
            for sx in (-1, 1):
                cyl(bm, (sx * 9 * k, -80 * k, zt(-80 * k) + 2 * k), (sx * 9 * k, -20 * k, zt(-20 * k) + 2 * k), 3 * k, n=16)
                for y in (-70, -50, -30):
                    cyl(bm, (sx * 9 * k, y * k, zt(y * k) + 2 * k), (sx * 9 * k, y * k + 1.5 * k, zt(y * k) + 2 * k), 3.3 * k, n=16, mat=G)
            for y in (60, 140):
                prism(bm, (0, y * k, zt(y * k) - 1 * k), (0, y * k, z), 2 * k, 3 * k)
        ship.part(r9, group="Rank9_Rail_Cannon", bevel=0.08, segs=1)
    else:
        def r6(bm):
            for y0, y1, z in ((110, 40, 1), (30, -40, 2), (-50, -120, 2.5)):
                w0, w1 = hwy(y0 * k), hwy(y1 * k)
                prism(bm, (w0 + 0.5, y0 * k, z * k), (w1 + 0.5, y1 * k, z * k), 0.8, 5 * k, w1=0.8, h1=6 * k, chamfer=0.4)
            prism(bm, (NX, -40 * k, NZ + 13.5 * k), (NX, -120 * k, NZ + 14 * k), 8 * k, 1.0, chamfer=0.45)
            prism(bm, (NX, -125 * k, NZ + 14 * k), (NX, -180 * k, NZ + 13.5 * k), 8 * k, 0.9, chamfer=0.45)
        ship.part(r6, group="Rank6_Armor_Plates", bevel=0.05, segs=1, mirror=True)


def build_talon_cruiser(ship):
    build_talon_family(ship, False)


def build_talon_capital(ship):
    build_talon_family(ship, True)


# ================================================================ MULE — armed industrial carrier (170 m) / mobile shipyard carrier (520 m)
#
# Evolves the Mule hauler: blunt cab with a wraparound windscreen, open truss
# spine, ribbed cargo containers (alternating livery), engine block with
# radiator fins and flank fuel tanks, jointed mining arms with claws and laser
# emitters (the "main guns"). The capital adds twin hangar decks, a dry-dock
# cradle with a hull under construction and gantry cranes.

def ribbed_container(bm, c, size, mat=H, rib=1.6):
    c = V(c)
    sx, sy, sz = size
    box(bm, c, (sx, sy, sz), mat=mat)
    for e in (-1, 1):
        box(bm, c + V((0, e * (sy / 2 - rib * 0.3), 0)), (sx + rib * 0.2, rib * 0.6, sz + rib * 0.2), mat=H)
    nrib = max(2, int((sy - rib) / rib))
    for i in range(nrib):
        y = -sy / 2 + rib * 0.6 + (i + 0.5) * (sy - rib * 1.2) / nrib
        box(bm, c + V((0, y, 0)), (sx + rib * 0.15, rib * 0.4, sz + rib * 0.12), mat=mat)


def mining_arm(bm, sh, el, wr, a, side=1):
    """Jointed mining arm: shoulder, elbow, wrist with three claws and a laser emitter. Returns emitter tip."""
    sh, el, wr = V(sh), V(el), V(wr)
    box(bm, sh + V((0, -a * 0.2, 0)), (a * 2.0, a * 2.6, a * 1.6))
    sphere(bm, sh, a * 0.95, seg=16, rings=8)
    prism(bm, sh, el, a * 0.6, a * 0.7, w1=a * 0.5, h1=a * 0.6, up=(1, 0, 0))
    sphere(bm, el, a * 0.78, seg=16, rings=8)
    prism(bm, el, wr, a * 0.46, a * 0.55, w1=a * 0.4, h1=a * 0.46, up=(1, 0, 0))
    cyl(bm, sh + V((0, a * 0.7, -a * 0.85)), el + V((0, -a * 1.8, -a * 0.4)), a * 0.18, n=10)
    cyl(bm, el + V((0, -a * 0.9, -a * 0.7)), el + V((-side * a * 0.3, a * 1.8, -a * 0.72)), a * 0.25, n=10)
    cyl(bm, el + V((-side * a * 0.3, a * 1.4, -a * 0.72)), wr + V((0, -a * 0.7, -a * 0.68)), a * 0.14, n=10)
    ax = (wr - el).normalized()
    lathe(bm, wr, ax, [(0.0, -0.22 * a), (0.82 * a, -0.22 * a), (0.9 * a, 0.45 * a), (0.68 * a, 1.0 * a), (0.0, 1.0 * a)], n=20)
    ref = V((0, 0, 1)) if abs(ax.z) < 0.9 else V((1, 0, 0))
    u = ax.cross(ref).normalized()
    v = ax.cross(u).normalized()
    for ang in (90, 210, 330):
        r = math.radians(ang)
        d = u * math.cos(r) + v * math.sin(r)
        p0 = wr + ax * 0.7 * a + d * 0.68 * a
        p1 = p0 + ax * 1.7 * a + d * 0.72 * a
        p2 = p1 + ax * 1.25 * a - d * 0.64 * a
        prism(bm, p0, p1, 0.2 * a, 0.16 * a, w1=0.16 * a, h1=0.14 * a, up=d)
        prism(bm, p1, p2, 0.16 * a, 0.14 * a, w1=0.08 * a, h1=0.07 * a, up=d)
        sphere(bm, p1, 0.18 * a, seg=10, rings=5)
    e0 = wr + ax * 1.0 * a
    e1 = wr + ax * 2.6 * a
    cyl(bm, e0, e1, 0.27 * a, 0.18 * a, n=16)
    for t in (0.3, 0.6):
        c = e0.lerp(e1, t)
        cyl(bm, c, c + ax * 0.12 * a, 0.3 * a, n=16, mat=G)
    cyl(bm, e1, e1 + ax * 0.1 * a, 0.13 * a, n=16, mat=G)
    return e1 + ax * 0.12 * a


def build_mule_cruiser(ship):
    rng = random.Random(170)
    WW, WH, WP = 0.8, 0.42, 2.3
    RK = dict(top=0.7, bot=0.75, hi=0.55, lo=-0.5)
    CAB = [(85, 11, 8, 2), (82, 14, 10.5, 2), (74, 15.5, 12, 2), (58, 15.5, 12, 2), (52, 13, 10, 2)]

    def cab(bm):
        rings = [ring10(y, hw, hh, cz, **RK) for y, hw, hh, cz in CAB]
        res = loft(bm, rings, matfn=lambda i, kk: GL if (i == 0 and seg_sym(kk) in (0, 1)) else (L if (i == 2 and seg_sym(kk) == 2) else H))
        inset_bands(bm, res, lambda i, kk: i == 2 and seg_sym(kk) in (3, 4), 0.5, -0.25)
        inset_faces(bm, [res['start']], 1.0, -0.6)
        # windscreen mullions
        for x in (-6.0, -2.0, 2.0, 6.0):
            prism(bm, (x, 85.2, 2 + 8 * 0.98), (x * 1.25, 81.9, 2 + 10.5 * 1.0), 0.18, 0.18)
        for sx in (1, -1):
            prism(bm, (sx * 9.6, 85.1, 2 + 8 * 0.6), (sx * 12.4, 81.9, 2 + 10.5 * 0.85), 0.2, 0.2)
    ship.part(cab, bevel=0.15, segs=2)
    ship.empty("Bridge", (0, 83.0, 2 + 9.0), 1.0)

    def spine(bm):
        prism(bm, (0, 53, 2), (0, -49, 2), 3.6, 3.6, chamfer=0.35)
        for sx in (1, -1):
            for sz in (1, -1):
                prism(bm, (sx * 4.2, 53, 2 + sz * 4.2), (sx * 4.2, -49, 2 + sz * 4.2), 0.5, 0.5)
        for j in range(12):
            y = 50 - j * 8.4
            for sx in (1, -1):
                prism(bm, (sx * 4.3, y, 6.4), (sx * 4.3, y - 8.4, -2.4), 0.3, 0.3)
            prism(bm, (-4.3, y, 6.5), (4.3, y, 6.5), 0.3, 0.3)
        pipe(bm, [(0, 53, 7.2), (0, -49, 7.2)], r=0.8)
        pipe(bm, [(2.0, 53, 7.0), (2.0, -49, 7.0)], r=0.45)
    ship.part(spine, bevel=0.06, segs=1)

    def cargo(bm):
        for j, y in enumerate((30, 4, -22)):
            for sx in (1, -1):
                ribbed_container(bm, (sx * 10.5, y, 2), (11, 24, 10.5), mat=L if (j + (sx > 0)) % 2 == 0 else H, rib=1.5)
            for dy in (-12.4, 12.4):
                prism(bm, (-16.6, y + dy, 7.7), (16.6, y + dy, 7.7), 0.6, 0.5)
                prism(bm, (-16.6, y + dy, -3.7), (16.6, y + dy, -3.7), 0.6, 0.5)
    ship.part(cargo, bevel=0.05, segs=1)

    # ventral hangar deck
    HB = [(46, 9, 4, -13), (42, 13, 7, -15), (-38, 13, 7, -15), (-44, 10, 5, -14)]

    def hangar_deck(bm):
        rings = [ring10(y, hw, hh, cz, top=0.9, bot=0.8, hi=0.6, lo=-0.5) for y, hw, hh, cz in HB]
        res = loft(bm, rings, matfn=lambda i, kk: L if (i == 1 and seg_sym(kk) == 3) else H)
        inset_faces(bm, [res['start']], 1.4, -12, mat=GL)
        for sx in (1, -1):
            hangar(bm, 13.0, 0.0, -15.5, 34, 7.0, 2.5, side=sx)
        for y in range(40, -38, -6):
            light(bm, (0, y, -22.2), 0.25)
    ship.part(hangar_deck, bevel=0.12, segs=1)

    ES = [(-48, 15, 12, 0), (-52, 18, 14, 0), (-72, 18.5, 14, 0), (-78, 16.5, 12.5, 0)]
    engines = []

    def eblock(bm):
        rings = [ring10(y, hw, hh, cz, top=0.75, bot=0.8, hi=0.5, lo=-0.5) for y, hw, hh, cz in ES]
        res = loft(bm, rings, matfn=lambda i, kk: L if (i == 1 and seg_sym(kk) == 2) else H)
        inset_bands(bm, res, lambda i, kk: i == 1 and seg_sym(kk) in (0, 4), 0.8, -0.4)
        for x, z, r in ((0, 3.0, 7.0), (11.5, -4.5, 5.0), (-11.5, -4.5, 5.0)):
            nozzle(bm, (x, -86, z), r, 10, n=28)
            engines.append(((x, -86, z), r))
        for j in range(9):
            box(bm, ((j - 4) * 2.4, -62, 16.6), (0.4, 14, 5))
        box(bm, (0, -62, 14.4), (21, 15, 0.6))
        for sx in (1, -1):
            cyl(bm, (sx * 20.5, -50, 0), (sx * 20.5, -74, 0), 4.2, n=20)
            for y in (-53, -71):
                cyl(bm, (sx * 20.5, y, 0), (sx * 20.5, y - 1.2, 0), 4.5, n=20, mat=L)
    ship.part(eblock, bevel=0.15, segs=1)
    for i, (p, r) in enumerate(engines):
        ship.empty("Engine_%d" % i, p, r * 0.88)

    tips = []

    def arms(bm):
        tips.append(mining_arm(bm, (8.5, 66, -8), (13.5, 84, -14), (9.5, 98, -12), 2.6))
    ship.part(arms, bevel=0.05, segs=1, mirror=True)
    ship.empty("Main_0", tips[0], 0.45)
    ship.empty("Main_1", (-tips[0].x, tips[0].y, tips[0].z), 0.45)

    mb, mm, mr = cl.turret_medium(3.0, barrels=2, length=3.0)
    ship.proto("twin", mb, mm, mr, bevel=0.08, segs=1)
    for y in (30, -22):
        for sx in (1, -1):
            ship.turret("twin", (sx * 10.5, y, 7.25 + 0.1))
    for sx in (1, -1):
        ship.turret("twin", (sx * 7, 0, -22.0), mount='bottom')

    def details(bm):
        for sx in (1, -1):
            for z in (6.0, 2.5):
                windows(bm, (sx * 15.5, 80, z), (sx * 15.5, 56, z), (sx, 0, 0), pitch=WP, w=WW, h=WH, rng=rng, skip=0.2)
            windows(bm, (sx * 18.5, -54, 6), (sx * 18.5, -70, 6), (sx, 0, 0), pitch=WP, w=WW, h=WH, rng=rng, skip=0.2)
            light(bm, (sx * 16.6, 43, 7.8), 0.35)
            light(bm, (sx * 16.6, -35, 7.8), 0.35)
            light(bm, (sx * 11, 85.3, -2), 0.5)
            rcs(bm, (sx * 15.6, 66, 2), (sx, 0, 0), 1.2)
            rcs(bm, (sx * 17, -76, 13), (0, 0, 1), 1.2)
        vent(bm, (0, 66, 14.2), 9, 7, n=7, h=0.5)
        antenna(bm, (5, 62, 14), 7, r=0.12)
        lattice_mast(bm, (-5, 60, 14), 6, 0.7)
        greeble_field(bm, rng, -12, 12, 54, 80, lambda x, y: 14.0, 14, 0.5, 1.6)
        greeble_field(bm, rng, -14, 14, -76, -50, lambda x, y: -14.0 + 0.2, 16, 0.6, 1.8, up=-1)
    ship.part(details, bevel=0.0, sharp_all=True, weighted=False)

    def r6(bm):
        for j, y in enumerate((30, 4, -22)):
            ribbed_container(bm, (0, y, 13.4), (11, 24, 9.5), mat=L if j % 2 else H, rib=1.5)
            for dy in (-12.4, 12.4):
                for sx in (-1, 1):
                    prism(bm, (sx * 6, y + dy, 7.5), (sx * 6, y + dy, 18.4), 0.4, 0.5)
    ship.part(r6, group="Rank6_Cargo_Pods", bevel=0.05, segs=1)


def build_mule_capital(ship):
    rng = random.Random(520)
    WW, WH, WP = 1.3, 0.7, 3.4
    RK = dict(top=0.7, bot=0.75, hi=0.55, lo=-0.5)
    BK = [(262, 34, 22, 0), (256, 40, 27, 0), (240, 44, 30, 0), (205, 44, 30, 0), (196, 38, 26, 0)]

    # ---------------- bow block with wraparound bridge windscreen
    def bow(bm):
        rings = [ring10(y, hw, hh, cz, **RK) for y, hw, hh, cz in BK]
        res = loft(bm, rings, matfn=lambda i, kk: GL if (i == 0 and seg_sym(kk) in (0, 1)) else (L if (i == 2 and seg_sym(kk) == 2) else H))
        inset_bands(bm, res, lambda i, kk: i == 2 and seg_sym(kk) in (1, 3, 4), 1.2, -0.6)
        inset_faces(bm, [res['start']], 3.0, -6.0)
        for x in range(-20, 21, 5):
            prism(bm, (x, 262.3, 22 * 0.99), (x * 1.18, 255.8, 27 * 1.0), 0.35, 0.35)
        for sx in (1, -1):
            prism(bm, (sx * 29.5, 262.2, 22 * 0.62), (sx * 36, 255.8, 27 * 0.85), 0.4, 0.4)
        # ore intake maw + drill on the front face
        lathe(bm, (0, 256, -4), (0, 1, 0), [(14, 0), (14, 6), (12.5, 8), (10, 8), (3, 26), (0.0, 30)], n=32, mats=[H, L, H, H, H])
        for j in range(4):
            cyl(bm, (0, 266 + j * 4.2, -4), (0, 267.2 + j * 4.2, -4), 10.5 - j * 2.2, n=32, mat=G if j == 1 else H)
    ship.part(bow, bevel=0.3, segs=2)
    ship.empty("Bridge", (0, 259.5, 23.5), 1.0)

    tips = []

    def arms(bm):
        tips.append(mining_arm(bm, (30, 248, 15), (46, 284, 8), (32, 318, 10), 6.5, side=1))
        tips.append(mining_arm(bm, (30, 248, -17), (46, 284, -24), (32, 318, -20), 6.5, side=1))
    ship.part(arms, bevel=0.12, segs=1, mirror=True)
    mains = tips + [V((-t.x, t.y, t.z)) for t in tips]
    mains.sort(key=lambda p: (p.x, -p.z))
    for i, m in enumerate(mains):
        ship.empty("Main_%d" % i, m, 1.1)

    # ---------------- twin-beam spine + dry-dock cradle
    BX = 18.0

    def spine(bm):
        for sx in (1, -1):
            prism(bm, (sx * BX, 198, 0), (sx * BX, -182, 0), 5, 6, chamfer=0.3)
            for sz in (1, -1):
                prism(bm, (sx * BX, 198, sz * 7.5), (sx * BX, -182, sz * 7.5), 1.0, 1.0)
        for j in range(19):
            y = 190 - j * 20
            for sz in (1, -1):
                prism(bm, (-BX, y, sz * 7.2), (BX, y, sz * 7.2), 0.9, 0.9)
            for sx in (1, -1):
                prism(bm, (sx * BX, y, 7), (sx * BX, y - 20, -7), 0.6, 0.6)
        pipe(bm, [(BX + 4, 198, 7), (BX + 4, -182, 7)], r=1.2)
        pipe(bm, [(-BX - 4, 198, 7), (-BX - 4, -182, 7)], r=1.2)
    ship.part(spine, bevel=0.1, segs=1)

    def cradle(bm):
        # a destroyer hull taking shape inside the dock: keel, ribs, partial plating
        prism(bm, (0, 150, -12), (0, -110, -12), 2.2, 1.6)
        for j in range(17):
            y = 140 - j * 15
            w = 12 * (1 - abs(y - 20) / 170) + 3
            pts = []
            for i in range(9):
                a = PI + PI * i / 8
                pts.append(V((w * math.cos(a), y, -12 - 0.5 * w * math.sin(a) * -1 + 0.0)))
            for a, b in zip(pts, pts[1:]):
                cyl(bm, a, b, 0.6, n=6)
            if j % 3 != 0 and 2 < j < 15:
                rings = [[V((w * 0.98 * math.cos(PI + PI * i / 8), y + dy, -12 + 0.5 * w * math.sin(PI + PI * i / 8) * -1)) for i in range(1, 8)]
                         for dy in (-7, 7)]
                loft(bm, rings, closed=False, cap_start=False, cap_end=False, mat=L if j % 5 == 0 else H)
        # support arms from the beams
        for y in (120, 40, -40, -100):
            for sx in (1, -1):
                prism(bm, (sx * BX, y, -4), (sx * 6, y, -10), 0.7, 0.7)
    ship.part(cradle, bevel=0.0, sharp_all=True, weighted=False)

    # ---------------- twin hangar decks
    HX = 48.0
    HD = [(176, 12, 8, 0), (168, 20, 14, 0), (-142, 20, 14, 0), (-150, 14, 10, 0)]

    def decks(bm):
        rings = [ring10(y, hw, hh, cz, cx=HX, top=0.88, bot=0.8, hi=0.6, lo=-0.6) for y, hw, hh, cz in HD]
        res = loft(bm, rings, matfn=lambda i, kk: L if (i == 1 and kk == 3) else H)
        inset_faces(bm, [res['start']], 2.0, -30, mat=GL)
        inset_bands(bm, res, lambda i, kk: i == 1 and kk in (0, 9), 1.0, -0.4)
        for y in (110, 10, -90):
            hangar(bm, HX + 20, y, -1.0, 46, 12, 4.0, side=1)
        # flight-deck lights
        for y in range(160, -140, -10):
            for x in (HX - 12, HX + 12):
                light(bm, (x, y, 14.3), 0.45)
        # struts to the spine
        for y in (150, 60, -30, -120):
            for sz in (1, -1):
                prism(bm, (BX + 4, y, sz * 5), (HX - 20, y + 10, sz * 8), 1.4, 1.4)
    ship.part(decks, bevel=0.2, segs=1, mirror=True)

    # gantry cranes straddling the dock
    def cranes(bm):
        for y in (90, -50):
            for sx in (1, -1):
                prism(bm, (sx * (HX - 16), y, 14), (sx * (HX - 16), y, 44), 1.4, 1.4)
                prism(bm, (sx * (HX - 16), y - 6, 14), (sx * (HX - 16), y, 40), 0.8, 0.8)
            prism(bm, (-(HX - 14), y, 44), (HX - 14, y, 44), 2.0, 2.4)
            box(bm, (8, y, 41), (6, 6, 4), mat=L)
            cyl(bm, (8, y, 39), (8, y, 14), 0.25, n=6)
            box(bm, (8, y, 13), (3, 3, 1.4))
            light(bm, (0, y, 46.6), 0.6)
    ship.part(cranes, bevel=0.06, segs=1)

    # ventral cargo rack
    def cargo(bm):
        for j, y in enumerate((150, 112, 74, 36, -2, -40, -78, -116)):
            for sx in (1, -1):
                ribbed_container(bm, (sx * 9, y, -23), (14, 34, 12), mat=L if (j + (sx > 0)) % 3 == 0 else H, rib=2.4)
        prism(bm, (-BX, 170, -16), (BX, 170, -16), 1, 1)
        prism(bm, (-BX, -136, -16), (BX, -136, -16), 1, 1)
    ship.part(cargo, bevel=0.08, segs=1)

    # ---------------- engine block
    EB = [(-180, 46, 30, 0), (-190, 58, 36, 0), (-245, 60, 38, 0), (-258, 55, 34, 0)]
    engines = []

    def eblock(bm):
        rings = [ring10(y, hw, hh, cz, top=0.75, bot=0.8, hi=0.5, lo=-0.5) for y, hw, hh, cz in EB]
        res = loft(bm, rings, matfn=lambda i, kk: L if (i == 1 and seg_sym(kk) == 2) else H)
        inset_bands(bm, res, lambda i, kk: i == 1 and seg_sym(kk) in (0, 1, 3, 4), 1.4, -0.6)
        for x, z, r in [(x, 0.0, 16.0) for x in (-36, 0, 36)] + [(x, 26.0, 8.0) for x in (-24, 0, 24)] + [(x, -26.0, 8.0) for x in (-24, 0, 24)]:
            nozzle(bm, (x, -258 - r * 1.25, z), r, r * 1.9, n=32)
            cyl(bm, (x, -256, z), (x, -258 - r * 0.6, z), r * 1.18, n=32)
            engines.append(((x, -258 - r * 1.25, z), r))
        for bank in (-1, 1):
            for j in range(10):
                box(bm, (bank * 22 + (j - 4.5) * 3.2, -220, 44), (0.6, 40, 11))
            box(bm, (bank * 22, -220, 38.6), (34, 42, 1.0))
        for sx in (1, -1):
            cyl(bm, (sx * 62, -196, 0), (sx * 62, -246, 0), 10, n=24)
            for y in (-200, -242):
                cyl(bm, (sx * 62, y, 0), (sx * 62, y - 2, 0), 10.6, n=24, mat=L)
    ship.part(eblock, bevel=0.25, segs=1)
    engines.sort(key=lambda e: (-e[1], -e[0][2], e[0][0]))
    for i, (p, r) in enumerate(engines):
        ship.empty("Engine_%d" % i, p, r * 0.86)

    # ---------------- turrets
    mb, mm, mr = cl.turret_medium(5.0, barrels=2, length=3.0)
    ship.proto("twin", mb, mm, mr, bevel=0.12, segs=1)
    for y in (135, 55, -25, -105):
        for sx in (1, -1):
            ship.turret("twin", (sx * (HX + 8), y, 14.0))
    for sx in (1, -1):
        ship.turret("twin", (sx * 20, 222, 30.0))
        ship.turret("twin", (sx * 46, -206, 33.0))
    for y in (95, -65):
        for sx in (1, -1):
            ship.turret("twin", (sx * HX, y, -14.0), mount='bottom')

    def details(bm):
        for sx in (1, -1):
            for z in (14, 8, 2):
                windows(bm, (sx * 44, 252, z), (sx * 44, 200, z), (sx, 0, 0), pitch=WP, w=WW, h=WH, rng=rng, skip=0.25)
            for y0, y1 in ((165, 136), (84, 36), (-16, -64), (-116, -140)):
                for z in (7, -7):
                    windows(bm, (sx * (HX + 20), y0, z), (sx * (HX + 20), y1, z), (sx, 0, 0), pitch=WP, w=WW, h=WH, rng=rng, skip=0.3)
            windows(bm, (sx * 60, -192, 12), (sx * 60, -244, 12), (sx, 0, 0), pitch=WP, w=WW, h=WH, rng=rng, skip=0.3)
            light(bm, (sx * 44.5, 262, -20), 1.2)
            light(bm, (sx * (HX + 20.5), 176, 0), 0.9)
            for y in (230, -250):
                rcs(bm, (sx * (44.5 if y > 0 else 60.5), y, 0), (sx, 0, 0), 3.5)
        lattice_mast(bm, (12, 225, 30), 20, 2.4)
        lattice_mast(bm, (-12, 215, 30), 14, 1.8, yard=False)
        dish(bm, (-24, 232, 32), (-0.3, 0.6, 0.8), 6.0)
        greeble_field(bm, rng, -38, 38, 200, 250, lambda x, y: 30.0, 40, 1.2, 4.0, avoid=[(20, 222, 8), (-20, 222, 8), (12, 225, 4), (-12, 215, 4)])
        greeble_field(bm, rng, -38, 38, 200, 255, lambda x, y: -30.0, 30, 1.2, 4.0, up=-1)
        greeble_field(bm, rng, -45, 45, -250, -192, lambda x, y: -38.0, 40, 1.4, 4.5, up=-1)
        for sx in (1, -1):
            greeble_field(bm, rng, sx * (HX - 15), sx * (HX + 15), -138, 165, lambda x, y: 14.0, 34, 1.0, 3.2,
                          avoid=[(sx * (HX + 8), y, 8) for y in (135, 55, -25, -105)] + [(sx * (HX - 16), y, 4) for y in (90, -50)])
    ship.part(details, bevel=0.0, sharp_all=True, weighted=False)

    def r9(bm):
        for sx in (1, -1):
            lathe(bm, (sx * 14, -150, -44), (0, 1, 0),
                  [(0.0, -2), (7, 0), (10, 5), (10, 150), (7, 155), (0.0, 158)], n=28)
            for y in (-130, -80, -30, 20, 70, 110):
                cyl(bm, (sx * 14, y, -44), (sx * 14, y + 2, -44), 10.6, n=28, mat=L)
            for y in (-110, -10, 90):
                prism(bm, (sx * 12, y, -35), (sx * 8, y, -29), 1.6, 2.0)
        box(bm, (0, -170, -46), (16, 22, 14))
        for j in range(5):
            box(bm, (0, -178 + j * 4, -38.8), (11, 1.4, 0.4), mat=E)
        cyl(bm, (0, -170, -53), (0, -170, -60), 4, 3, n=16)
        cyl(bm, (0, -170, -60), (0, -170, -60.3), 2.2, n=16, mat=E)
        pipe(bm, [(14, 0, -34), (4, 20, -30), (4, 150, -30)], r=1.4)
        pipe(bm, [(-14, 0, -34), (-4, 20, -30), (-4, 150, -30)], r=1.4)
    ship.part(r9, group="Rank9_Refinery_Module", bevel=0.15, segs=1)


# ================================================================ SEER — Veyr organic cruiser (150 m) / flagship (450 m)
#
# Evolves the Seer: smooth bio-hull with hoop ribs, manta wings whose tips
# curl forward into a crescent, glowing veins, an eye canopy, organic engine
# orifices. The flagship adds an overlapping segmented carapace, forward
# horns and mandibles carrying the main lances, and a dorsal crest.

def build_seer_family(ship, cap):
    rng = random.Random(450 if cap else 150)
    KY = (450.0 if cap else 150.0) / 25.6
    KX = KY * (0.82 if cap else 0.9)
    KZ = KY * (1.15 if cap else 1.05)
    BS0 = [(11.6, 0.5, 0.3, 0.0), (9.6, 1.4, 0.65, 0.05), (6.5, 2.2, 0.95, 0.1), (2.5, 2.6, 1.1, 0.15), (-2.0, 2.5, 1.05, 0.15),
           (-6.5, 2.0, 0.9, 0.12), (-10.0, 1.3, 0.62, 0.1), (-12.2, 0.7, 0.4, 0.08)]
    BS = resample([(y * KY, hw * KX, hh * KZ, cz * KZ) for y, hw, hh, cz in BS0], 2.0 * KY)
    NOSE, TAIL = 12.6 * KY, -13.0 * KY
    NR = 16

    def hwy(y):
        return at(BS, y, 1)

    def zsurf(x, y, up=1):
        hw, hh, cz = at(BS, y, 1), at(BS, y, 2), at(BS, y, 3)
        u = min(abs(x) / hw, 0.999)
        z = hh * (1 - u ** 2.3) ** (1 / 2.3)
        return cz + (z + hh * 0.22 * (1 - u) ** 2 if up > 0 else -z - hh * 0.1 * (1 - u) ** 2)

    def body(bm):
        rings = [V((0, NOSE, -0.05 * KZ))]
        for y, hw, hh, cz in BS:
            rings.append(ring_se(y, hw, hh, cz, n=NR, p=2.3, ridge=hh * 0.22, keel=hh * 0.1))
        rings.append(V((0, TAIL, 0.06 * KZ)))
        loft(bm, rings)
    ship.part(body, bevel=0, subsurf=1, angle=180, weighted=False)

    WING0 = [dict(x=1.8, z=0.0, le=6.0, te=-8.5, t=0.9), dict(x=4.2, z=-0.15, le=3.4, te=-7.2, t=0.6),
             dict(x=6.8, z=-0.3, le=1.6, te=-5.2, t=0.42), dict(x=9.0, z=-0.32, le=2.6, te=-2.4, t=0.3),
             dict(x=10.6, z=-0.22, le=5.6, te=1.4, t=0.2), dict(x=11.2, z=-0.1, le=8.6, te=5.6, t=0.12)]
    if cap:
        WING0[-1] = dict(x=11.0, z=0.1, le=9.6, te=6.8, t=0.12)
    WING = [dict(x=w['x'] * KX * (1.0 if cap else 1.05), z=w['z'] * KZ, le=w['le'] * KY, te=w['te'] * KY, t=w['t'] * KZ) for w in WING0]

    def wz(x, top=True):
        st = [(s_['x'], s_['z'] + s_['t'] * (0.45 if top else -0.4)) for s_ in WING]
        return interp(st, x, 1)

    def wle(x):
        return interp([(s_['x'], s_['le']) for s_ in WING], x, 1)

    def wte(x):
        return interp([(s_['x'], s_['te']) for s_ in WING], x, 1)

    def wings(bm):
        wing(bm, WING, flat_te=0.4)
    ship.part(wings, bevel=0, subsurf=2, angle=180, weighted=False, mirror=True)

    rr = 0.13 * KX

    # hoop ribs over the body, wing ribs
    def ribs(bm):
        step = 1.5 if cap else 3.0
        y = 9.0
        while y > -10.5:
            yy = y * KY
            hw, hh, cz = hwy(yy), at(BS, yy, 2), at(BS, yy, 3)
            pts = []
            n = 11
            for i in range(n + 1):
                a = math.radians(-28 + 236 * i / n)
                pts.append(V((hw * 1.0 * math.cos(a), yy, cz + (hh * 1.1 + rr) * math.sin(a) + (hh * 0.2 if math.sin(a) > 0.9 else 0))))
            for a_, b_ in zip(pts, pts[1:]):
                cyl(bm, a_, b_, rr, n=8)
            for p in pts:
                sphere(bm, p, rr, seg=8, rings=4)
            y -= step
        for sx in (1, -1):
            for f in ((0.15, 0.32, 0.5, 0.68) if cap else (0.2, 0.45, 0.7)):
                pts = []
                for x in (2.0, 4.0, 6.0, 8.0, 9.6):
                    xx = x * KX
                    pts.append(V((sx * xx, lerp(wle(xx), wte(xx), f), wz(xx) - rr * 0.4)))
                for a_, b_ in zip(pts, pts[1:]):
                    cyl(bm, a_, b_, rr * 0.85, rr * 0.6, n=8)
                for p in pts[1:-1]:
                    sphere(bm, p, rr * 0.8, seg=8, rings=4)
    ship.part(ribs, bevel=0, angle=60, weighted=False)

    # glowing veins
    def veins(bm):
        vr = rr * 0.24
        for sx in (1, -1):
            for f in ((0.24, 0.41, 0.59, 0.76) if cap else (0.33, 0.58)):
                pts = []
                for x in (2.2, 4.0, 6.0, 8.0, 9.8, 10.7):
                    xx = x * KX
                    pts.append(V((sx * xx, lerp(wle(xx), wte(xx), f), wz(xx) - vr)))
                for a_, b_ in zip(pts, pts[1:]):
                    cyl(bm, a_, b_, vr, vr * 0.8, n=6, mat=G)
            pts = [V((sx * xx * KX, lerp(wle(xx * KX), wte(xx * KX), 0.06), wz(xx * KX) - vr * 2.5)) for xx in (4.0, 6.0, 8.0, 9.6, 10.6, 11.0)]
            for a_, b_ in zip(pts, pts[1:]):
                cyl(bm, a_, b_, vr, n=6, mat=G)
            pts = []
            for y in (9.0, 6.0, 3.0, 0.0, -3.0, -6.0, -9.0):
                yy = y * KY
                x = hwy(yy) * 0.42
                pts.append(V((sx * x, yy, zsurf(x, yy) - vr * 0.6)))
            for a_, b_ in zip(pts, pts[1:]):
                cyl(bm, a_, b_, vr, n=6, mat=G)
            # flank vein
            pts = [V((sx * hwy(y * KY) * 1.0, y * KY, at(BS, y * KY, 3) - at(BS, y * KY, 2) * 0.15)) for y in (9.0, 6.0, 3.0, 0.0, -3.0, -6.0, -9.5)]
            for a_, b_ in zip(pts, pts[1:]):
                cyl(bm, a_, b_, vr, n=6, mat=G)
    ship.part(veins, bevel=0, angle=60, weighted=False)

    # eye canopy (the bridge)
    EY = 9.0 * KY
    EZ = zsurf(0, EY) - 0.1 * KZ

    def eye(bm):
        sphere(bm, (0, EY, EZ), 0.6 * KX, mat=GL, seg=24, rings=12, scale=(1.0, 2.4, 0.5 * KZ / KX))
        lathe(bm, (0, EY, EZ - 0.12 * KZ), (0, 0, 1), [(0.0, -0.05 * KZ), (0.78 * KX, -0.05 * KZ), (0.72 * KX, 0.12 * KZ), (0.0, 0.12 * KZ)], n=24)
        for f in (-0.5, 0.0, 0.5):
            pts = [V((0.62 * KX * math.cos(a) * 1.0, EY + f * 1.2 * KX, EZ + 0.31 * KZ * math.sin(a) * (1 - abs(f) * 0.6)))
                   for a in [PI * i / 10 for i in range(11)]]
            for a_, b_ in zip(pts, pts[1:]):
                cyl(bm, a_, b_, rr * 0.25, n=5)
    ship.part(eye, bevel=0, angle=60, weighted=False)
    ship.empty("Bridge", (0, EY + 0.9 * KX, EZ + 0.18 * KZ), 1.0)

    # organic engine orifices
    engines = []

    def eng(bm):
        spots = [(1.25, 0.05, 1.0)] + ([(2.3, -0.25, 0.7)] if cap else [])
        for ex, ez, sc_ in spots:
            for sx in (1, -1):
                x, z = sx * ex * KX, ez * KZ
                y0 = -10.0 * KY
                lathe(bm, (x, y0, z), (0, 1, 0), [(0.55 * KX * sc_, -1.2 * KY), (0.75 * KX * sc_, 0.0), (0.9 * KX * sc_, 1.5 * KY),
                                                  (0.7 * KX * sc_, 3.0 * KY), (0.0, 3.4 * KY)], n=20)
                nozzle(bm, (x, y0 - 1.6 * KY, z), 0.55 * KX * sc_, 0.7 * KY, n=20)
                engines.append(((x, y0 - 1.6 * KY, z), 0.55 * KX * sc_))
    ship.part(eng, bevel=0, subsurf=1, angle=60, weighted=False)
    engines.sort(key=lambda e: (-e[1], e[0][0]))
    for i, (p, r) in enumerate(engines):
        ship.empty("Engine_%d" % i, p, r * 0.85)

    # forward horns (main lances) at the wing roots; mandibles on the flagship
    mains = []

    def horns(bm):
        specs = [(1.9 * KX, 8.0 * KY, -0.2 * KZ, 0.34 * KX, 2.4 * KY)]
        if cap:
            specs.append((1.0 * KX, 10.4 * KY, -0.75 * KZ, 0.24 * KX, 2.2 * KY))
        for x, y, z, r, ln in specs:
            sphere(bm, (x, y - ln * 0.3, z), r * 1.1, seg=16, rings=8, scale=(1, 2.0, 0.85))
            lathe(bm, (x, y - ln * 0.4, z), (0.02, 1, 0.0), [(r * 0.9, 0.0), (r, ln * 0.3), (r * 0.75, ln * 0.75), (r * 0.62, ln)], n=18, cap_end=False)
            mains.append(gun_port(bm, V((x + 0.02 * ln, y + ln * 0.6, z)), r * 0.42, axis=(0.02, 1, 0), depth=r * 1.2, collar=1.4, n=18))
            for t in (0.25, 0.55):
                c = V((x + 0.02 * ln * t, y - ln * 0.4 + ln * t, z))
                cyl(bm, c, c + V((0, r * 0.25, 0)), r * 1.02, n=18, mat=G)
    ship.part(horns, bevel=0, subsurf=0, angle=50, weighted=False, mirror=True)
    allm = []
    for m in mains:
        allm += [m, V((-m.x, m.y, m.z))]
    allm.sort(key=lambda p: (p.x, -p.z))
    for i, m in enumerate(allm):
        ship.empty("Main_%d" % i, m, (0.34 * KX if abs(m.x) > 1.5 * KX else 0.24 * KX) * 0.42)

    # sensor dish + canted antenna fins (from the Seer), dorsal crest on the flagship
    def sensors(bm):
        yy = -4.5 * KY
        c = V((0, yy, zsurf(0, yy) - 0.05 * KZ))
        cyl(bm, c, c + V((0, 0.2 * KY, 0.9 * KZ)), 0.22 * KX, 0.14 * KX, n=12)
        dc = c + V((0, 0.2 * KY, 0.9 * KZ))
        dish(bm, dc, (0, 0.55, 1), 1.6 * KX, mat_rim=L)
        for sx in (1, -1):
            st = [dict(x=sx * 1.0 * KX, z=zsurf(1.0 * KX, -8 * KY) - 0.1 * KZ, le=-6.0 * KY, te=-9.6 * KY, t=0.18 * KX),
                  dict(x=sx * 1.9 * KX, z=2.4 * KZ, le=-8.6 * KY, te=-10.6 * KY, t=0.1 * KX),
                  dict(x=sx * 2.3 * KX, z=3.0 * KZ, le=-10.2 * KY, te=-11.2 * KY, t=0.05 * KX)]
            wing(bm, st)
            sphere(bm, (sx * 2.32 * KX, -10.2 * KY, 3.02 * KZ), rr * 0.7, mat=G, seg=8, rings=4)
        if cap:
            rings = []
            for y, h in ((7.0, 0.0), (5.0, 0.7), (1.0, 1.4), (-2.5, 1.1), (-3.8, 0.0)):
                yy = y * KY
                zb = zsurf(0, yy) - 0.1 * KZ
                rings.append([V((0.25 * KX, yy, zb)), V((0.06 * KX, yy, zb + h * KZ)), V((-0.06 * KX, yy, zb + h * KZ)), V((-0.25 * KX, yy, zb))])
            loft(bm, rings, matfn=lambda i, kk: G if kk == 1 else H)
    ship.part(sensors, bevel=0, subsurf=1, angle=50, weighted=False)

    # segmented carapace over the body (the "ribbed shell")
    if cap:
        def carapace(bm):
            segs = [(8.4, 5.2), (5.8, 2.4), (3.0, -0.6), (0.0, -3.8), (-3.2, -7.0), (-6.4, -9.8)]
            for j, (y0, y1) in enumerate(segs):
                rings = []
                for t, bul in ((0.0, 1.03), (0.5, 1.09), (1.0, 1.05)):
                    yy = lerp(y0, y1, t) * KY
                    hw, hh, cz = hwy(yy), at(BS, yy, 2), at(BS, yy, 3)
                    outer, inner = [], []
                    for i in range(11):
                        a = math.radians(10 + 160 * i / 10)
                        outer.append(V((hw * bul * 1.02 * math.cos(a), yy, cz + hh * bul * 1.12 * math.sin(a) + hh * 0.18 * math.sin(a) ** 6)))
                        inner.append(V((hw * 0.97 * math.cos(a), yy, cz + hh * 1.02 * math.sin(a) + hh * 0.15 * math.sin(a) ** 6)))
                    rings.append(outer + list(reversed(inner)))
                loft(bm, rings, matfn=lambda i, kk: L if (j == 1 and kk in (4, 5)) else H)
        ship.part(carapace, bevel=0, subsurf=1, angle=60, weighted=False)

    # turrets
    ts = 7.5 if cap else 3.4
    ob, om, orr = cl.turret_organic(ts, length=2.4)
    ship.proto("bio", ob, om, orr, bevel=0.0, subsurf=1, segs=1)
    sink = 0.9 if cap else 0.3
    if cap:
        for y in (4.4, -1.6):
            for sx in (1, -1):
                x = hwy(y * KY) * 0.5
                ship.turret("bio", (sx * x, y * KY, zsurf(x, y * KY) + hwy(y * KY) * 0.05 - sink))
        for x in (4.6, 6.6, 8.6):
            xx = x * KX
            yy = lerp(wle(xx), wte(xx), 0.45)
            for sx in (1, -1):
                ship.turret("bio", (sx * xx, yy, wz(xx) - 0.6))
        for x in (5.4, 8.0):
            xx = x * KX
            yy = lerp(wle(xx), wte(xx), 0.5)
            for sx in (1, -1):
                ship.turret("bio", (sx * xx, yy, wz(xx, False) + 0.6), mount='bottom')
        for y in (5.0, -3.0):
            ship.turret("bio", (0, y * KY, zsurf(0, y * KY, -1) + sink), mount='bottom')
    else:
        ship.turret("bio", (0, 3.6 * KY, zsurf(0, 3.6 * KY) - sink))
        xx = 6.4 * KX
        yy = lerp(wle(xx), wte(xx), 0.45)
        for sx in (1, -1):
            ship.turret("bio", (sx * xx, yy, wz(xx) - 0.2))
        for y in (4.0, -4.0):
            ship.turret("bio", (0, y * KY, zsurf(0, y * KY, -1) + sink), mount='bottom')
        ship.turret("bio", (0, -2.2 * KY, zsurf(0, -2.2 * KY) - sink))

    # bioluminescent nodes
    def nodes(bm):
        for sx in (1, -1):
            for x in (3.0, 5.0, 7.0, 9.0, 10.5):
                xx = x * KX
                light(bm, (sx * xx, lerp(wle(xx), wte(xx), 0.02), wz(xx) - rr), rr * 0.9)
            for y in (8.0, 4.0, 0.0, -4.0, -8.0):
                yy = y * KY
                light(bm, (sx * hwy(yy) * 1.01, yy, at(BS, yy, 3) + at(BS, yy, 2) * 0.35), rr * 0.8)
        for y in range(9, -10, -3):
            light(bm, (0, y * KY, zsurf(0, y * KY, -1) - rr * 0.3), rr * 0.8)
    ship.part(nodes, bevel=0.0, sharp_all=True, weighted=False)

    if cap:
        def r9(bm):
            c = V((0, -1.5 * KY, 1.2 * KZ))
            pts = []
            for i in range(21):
                a = math.radians(-10 + 200 * i / 20)
                pts.append(c + V((5.2 * KX * math.cos(a), 0.32 * KY * math.sin(a), 3.4 * KZ * math.sin(a))))
            for i, (a_, b_) in enumerate(zip(pts, pts[1:])):
                r0 = rr * (1 + 1.2 * math.sin(PI * i / 20))
                r1 = rr * (1 + 1.2 * math.sin(PI * (i + 1) / 20))
                cyl(bm, a_, b_, r0, r1, n=12)
            for i, p in enumerate(pts[1:-1], 1):
                sphere(bm, p, rr * (1 + 1.2 * math.sin(PI * i / 20)), seg=12, rings=6)
                d = (p - c).normalized()
                if i % 2 == 0:
                    cyl(bm, p, p + d * (0.6 + 0.6 * math.sin(PI * i / 20)) * KX, rr * 0.7, 0.0, n=8)
                else:
                    sphere(bm, p + d * rr * 1.6, rr * 0.9, mat=G, seg=10, rings=5)
            for sx in (1, -1):
                cyl(bm, c + V((sx * 5.1 * KX, 0, -0.9 * KZ)), V((sx * 2.2 * KX, -1.5 * KY, zsurf(2.2 * KX, -1.5 * KY) - 0.2)), rr, rr * 1.6, n=10)
            sphere(bm, c + V((0, 0.3 * KY, 3.4 * KZ + 0.5 * KX)), rr * 2.6, mat=G, seg=16, rings=8)
        ship.part(r9, group="Rank9_Crown_Array", bevel=0, angle=50, weighted=False)
    else:
        def r6(bm):
            st = [dict(x=2.2 * KX, z=0.62 * KZ, le=3.2 * KY, te=-6.6 * KY, t=0.18 * KZ),
                  dict(x=4.6 * KX, z=0.38 * KZ, le=1.4 * KY, te=-5.4 * KY, t=0.14 * KZ),
                  dict(x=6.6 * KX, z=0.12 * KZ, le=0.4 * KY, te=-3.4 * KY, t=0.1 * KZ)]
            wing(bm, st, mat=H, flat_te=0.5)
            st = [dict(x=1.4 * KX, z=1.2 * KZ, le=6.0 * KY, te=1.0 * KY, t=0.18 * KZ), dict(x=2.4 * KX, z=0.7 * KZ, le=4.6 * KY, te=0.6 * KY, t=0.12 * KZ)]
            wing(bm, st, flat_te=0.5)
        ship.part(r6, group="Rank6_Shell_Plates", bevel=0, subsurf=2, angle=180, weighted=False, mirror=True)


def build_seer_cruiser(ship):
    build_seer_family(ship, False)


def build_seer_capital(ship):
    build_seer_family(ship, True)


# ================================================================ registry

STYLES = {
    'bastion': dict(paint=(0.24, 0.26, 0.23), paint2=(0.17, 0.185, 0.165), accent=(0.3, 0.31, 0.27),
                    metal=0.32, rough=0.58, big=(12.0, 20.0, 7.0), small=(4.0, 5.0, 2.33), seam=0.3,
                    wear=0.6, grime=1.0, ao_dist=6.0),
    'wren': dict(paint=(0.6, 0.61, 0.61), paint2=(0.48, 0.49, 0.5), accent=(0.68, 0.68, 0.66),
                 metal=0.3, rough=0.36, big=(9.0, 16.0, 6.0), small=(3.0, 4.0, 2.0), seam=0.22,
                 wear=0.4, grime=0.6, ao_dist=4.0, hatch=0.86),
    'talon': dict(paint=(0.17, 0.18, 0.2), paint2=(0.11, 0.12, 0.13), accent=(0.23, 0.24, 0.26),
                  metal=0.45, rough=0.42, big=(10.0, 16.0, 6.0), small=(3.3, 4.0, 2.0), seam=0.24,
                  wear=0.55, grime=0.8, ao_dist=4.5),
    'mule': dict(paint=(0.44, 0.4, 0.31), paint2=(0.34, 0.31, 0.25), accent=(0.62, 0.42, 0.06),
                 accent_thresh=0.92, metal=0.35, rough=0.6, big=(10.0, 16.0, 6.0), small=(3.4, 4.0, 2.0), seam=0.26,
                 wear=0.9, grime=1.3, ao_dist=5.0, grime_col=(0.06, 0.045, 0.03)),
    'seer': dict(organic=True, paint=(0.07, 0.09, 0.11), paint2=(0.12, 0.08, 0.15), accent=(0.16, 0.24, 0.24),
                 metal=0.55, rough=0.3, wear=0.2, grime=0.5, bare=(0.45, 0.5, 0.52), bump=0.7, cell=5.0, rib=7.0,
                 ao_dist=5.0, wear_r=0.2),
}

SHIPS = {
    'bastion_cruiser': ('bastion', build_bastion_cruiser, dict(tex=4096, length=180)),
    'bastion_capital': ('bastion', build_bastion_capital, dict(tex=4096, length=600)),
    'wren_cruiser': ('wren', build_wren_cruiser, dict(tex=4096, length=130)),
    'wren_capital': ('wren', build_wren_capital, dict(tex=4096, length=360)),
    'talon_cruiser': ('talon', build_talon_cruiser, dict(tex=4096, length=150)),
    'talon_capital': ('talon', build_talon_capital, dict(tex=4096, length=420)),
    'mule_cruiser': ('mule', build_mule_cruiser, dict(tex=4096, length=170)),
    'mule_capital': ('mule', build_mule_capital, dict(tex=4096, length=520)),
    'seer_cruiser': ('seer', build_seer_cruiser, dict(tex=4096, length=150)),
    'seer_capital': ('seer', build_seer_capital, dict(tex=4096, length=450)),
}


def run(name, bake=True, preview=True, tex=None):
    t0 = time.time()
    fam, builder, opts = SHIPS[name]
    tex = tex or opts.get('tex', 4096)
    shiplib.reset_scene()
    mats, socks = cl.make_capital_materials(STYLES[fam])
    ship = cl.Capital(name, mats)
    ship.target_len = opts.get('length')
    builder(ship)
    bs.species_kits(ship, big=True)
    objs = ship.finalize()
    tris, per = ship.tri_count()
    log(name, "triangles", tris, per, "build %.1fs" % (time.time() - t0))
    build_dir = BUILD
    os.makedirs(build_dir, exist_ok=True)
    if bake:
        bo = ship.bake_objects()
        cl.cap_unwrap(bo, boost={p['obj']: opts.get('turret_boost', 2.5) for p in ship.protos.values()})
        paths = cl.cap_bake(bo, mats, socks, build_dir, name, size=tex, samples=opts.get('samples', (16, 4, 4)),
                            mr_size=opts.get('mr_size', tex // 2))
        shiplib.rebuild_hull_textured(mats["Hull"], paths)
    ship.instantiate_turrets()
    if bake:
        ship.export(os.path.join(MODELS, name + ".glb"))
    if preview:
        cl.cap_preview(name, ship, mats, PREVIEWS, build_dir, closeup=opts.get('closeup'))
    log(name, "done in %.1fs" % (time.time() - t0))
    return tris


def main():
    argv = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
    bake = "--no-bake" not in argv
    preview = "--no-preview" not in argv
    tex = None
    if "--tex" in argv:
        tex = int(argv[argv.index("--tex") + 1])
    names = []
    for a in argv:
        if a in SHIPS:
            names.append(a)
        elif a + "_cruiser" in SHIPS or a + "_capital" in SHIPS:
            names += [n for n in (a + "_cruiser", a + "_capital") if n in SHIPS]
    names = names or list(SHIPS)
    for n in names:
        run(n, bake=bake, preview=preview, tex=tex)


if __name__ == "__main__":
    main()

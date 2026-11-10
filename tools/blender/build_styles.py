"""Art-direction candidates: the same capital ship in several styles, for picking one.

    blender -b --factory-startup -P tools/blender/build_styles.py -- [names] [--no-bake] [--battle] [--fast] [--tex N]
    blender -b --factory-startup -P tools/blender/build_styles.py -- [names] --game     the game assets (GAME below)

Each styles/<name>.py gives `build(ship)` (shiplib parts, nose +Y, 60 units long standing for ~420 m,
Engine_N and Gun_N empties),
`STYLE` (shiplib.make_hull_procedural's dict) and optionally `TINT` (stylelib.DEFAULT_TINT keys) and
`materials()` -> (mats, socks) in shiplib.make_materials's shape; or `run_build` for a model with its own
pipeline (styles/baseline.py).
Writes tools/blender/_build/styles/<name>_hero.png, _sheet.png, [_battle.png] and, unless --no-bake, <name>.glb
(WebP, tinted: for the picker page, not a game asset).
"""
import importlib
import os
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
if HERE not in sys.path:
    sys.path.insert(0, HERE)

import capitallib as cl
import shiplib
import stylelib
from shiplib import Ship, log

OUT = os.path.join(HERE, "_build", "styles")
MODELS = os.path.abspath(os.path.join(HERE, "..", "..", "crates", "client", "assets", "models"))
ALL = ["baseline", "west", "japan", "global", "unknown"]
# Each species flies its own style at capital size; the unknown one is the Aster leviathan (docs/ART_DIRECTION.md §4).
# style -> (model, length in metres, capital: guns become turret muzzles and main ports, a Bridge is marked)
GAME = {"global": ("human_capital", 420.0, True), "japan": ("veyr_capital", 420.0, True),
        "west": ("kharz_capital", 420.0, True), "unknown": ("npc_sentinel", 130.0, False)}


def game_asset(name, ship, meshes, mats, tex):
    """The candidate as the game loads it: metres, neutral paint and lights (the game tints them), KTX2."""
    import bpy
    from mathutils import Matrix
    model, length, capital = GAME[name]
    stylelib.neutral(mats)
    k = length / (ship.bbox[1].y - ship.bbox[0].y)
    for o in meshes:
        o.data.transform(Matrix.Scale(k, 4))
        o.data.update()
        o.location = o.location * k
    for e in ship.empties:
        e.location = e.location * k
        e.scale = e.scale * k
    empties = list(ship.empties)
    if capital:
        guns = sorted((e for e in empties if e.name.startswith("Gun_")), key=lambda e: -e.location.y)
        for i, e in enumerate(guns):
            e.name = "Muzzle_%d" % i
        for i, e in enumerate(guns[:2]):  # the two furthest forward also fire the main salvo
            empties.append(ship.empty("Main_%d" % i, e.location, e.scale.x))
        hull = ship.objects["Hull"]
        top = max(hull.data.vertices, key=lambda v: (round(v.co.z, 2), -abs(v.co.x)))
        empties.append(ship.empty("Bridge", (0.0, top.co.y, top.co.z * 0.92), 1.0))
    bpy.context.view_layer.update()
    shiplib.export_glb(os.path.join(MODELS, model + ".glb"), meshes, empties)


def run(name, bake=True, battle=False, fast=False, tex=2048, game=False):
    t0 = time.time()
    mod = importlib.import_module("styles." + name)
    shiplib.reset_scene()
    tint = stylelib.tint_of(mod)
    os.makedirs(OUT, exist_ok=True)
    if hasattr(mod, "run_build"):  # a model with its own pipeline (the shipped capital)
        meshes, empties, mats = mod.run_build(tint, OUT, bake, tex)
    else:
        mats, socks = mod.materials() if hasattr(mod, "materials") else shiplib.make_materials(mod.STYLE)
        stylelib.apply_tint(mats, tint)
        ship = Ship(name, mats)
        mod.build(ship)
        if game and GAME[name][2]:
            try:  # dish and engine vanes, measured off the hull like every other capital (build_ships.rig_auto)
                import build_ships
                build_ships.rig_auto(ship, big=True)
            except Exception as ex:
                log(name, "rig left out:", ex)
        objs = ship.finalize()
        tris, per = ship.tri_count()
        log(name, "triangles", tris, per)
        meshes, empties = list(objs.values()), ship.empties
        if bake:
            # capitallib's unwrap repacks the islands: shiplib's leaves a model this dense a sliver of the sheet.
            cl.cap_unwrap(meshes)
            paths = cl.cap_bake(meshes, mats, socks, OUT, name, size=tex, mr_size=tex // 2)
            shiplib.rebuild_hull_textured(mats["Hull"], paths)
        if game:
            game_asset(name, ship, meshes, mats, tex)
            return log(name, "game asset done in %.1fs" % (time.time() - t0))
    if bake:
        stylelib.export_viewer(os.path.join(OUT, name + ".glb"), meshes, empties)
    cam = stylelib.setup_studio(tint)
    stylelib.render_views(cam, meshes, OUT, name, fast=fast)
    if battle:
        stylelib.battle(meshes, empties, mats, tint, os.path.join(OUT, name + "_battle.png"), fast=fast)
    log(name, "done in %.1fs" % (time.time() - t0))


def main():
    argv = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
    tex = int(argv[argv.index("--tex") + 1]) if "--tex" in argv else 2048
    game = "--game" in argv
    for n in [a for a in argv if a in ALL and (not game or a in GAME)] or (list(GAME) if game else ALL):
        run(n, bake="--no-bake" not in argv, battle="--battle" in argv, fast="--fast" in argv,
            tex=(2048 if n == "unknown" else 4096) if game and "--tex" not in argv else tex, game=game)


if __name__ == "__main__":
    main()

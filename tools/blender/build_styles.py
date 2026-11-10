"""Art-direction candidates: the same capital ship in several styles, for picking one.

    blender -b --factory-startup -P tools/blender/build_styles.py -- [names] [--no-bake] [--battle] [--fast] [--tex N]

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
ALL = ["baseline", "west", "japan", "global", "unknown"]


def run(name, bake=True, battle=False, fast=False, tex=2048):
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
        objs = ship.finalize()
        tris, per = ship.tri_count()
        log(name, "triangles", tris, per)
        meshes, empties = list(objs.values()), ship.empties
        if bake:
            # capitallib's unwrap repacks the islands: shiplib's leaves a model this dense a sliver of the sheet.
            cl.cap_unwrap(meshes)
            paths = cl.cap_bake(meshes, mats, socks, OUT, name, size=tex, mr_size=tex // 2)
            shiplib.rebuild_hull_textured(mats["Hull"], paths)
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
    for n in [a for a in argv if a in ALL] or ALL:
        run(n, bake="--no-bake" not in argv, battle="--battle" in argv, fast="--fast" in argv, tex=tex)


if __name__ == "__main__":
    main()

"""The Talon capital as it ships today, without rank and species parts: the look every candidate is compared
with. Built by its own pipeline (build_capitals / capitallib), so it brings `run_build` instead of `build`."""
import build_capitals as bc
import capitallib as cl
import shiplib
import stylelib

NAME = 'talon_capital'


def run_build(tint, outdir, bake, tex):
    fam, builder, opts = bc.SHIPS[NAME]
    mats, socks = cl.make_capital_materials(bc.STYLES[fam])
    stylelib.apply_tint(mats, tint)
    ship = cl.Capital(NAME, mats)
    ship.target_len = opts.get('length')
    builder(ship)
    ship.finalize()
    shiplib.log("baseline triangles", ship.tri_count())
    if bake:
        bo = ship.bake_objects()
        cl.cap_unwrap(bo, boost={p['obj']: opts.get('turret_boost', 2.5) for p in ship.protos.values()})
        paths = cl.cap_bake(bo, mats, socks, outdir, "baseline", size=tex, samples=opts.get('samples', (16, 4, 4)), mr_size=tex // 2)
        shiplib.rebuild_hull_textured(mats["Hull"], paths)
    ship.instantiate_turrets()
    meshes = [o for o in list(ship.objects.values()) + ship.turret_objs if not o.name.startswith("Rank")]
    return meshes, ship.empties, mats

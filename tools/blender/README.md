# tools/blender — procedural ship models

Headless Blender (5.2 LTS) scripts that model, texture-bake and export the five
player hulls to `crates/client/assets/models/*.glb`.

| file | role |
|---|---|
| `shiplib.py` | shared helpers: bmesh lofts/lathes/wings, bevel + weighted-normal part pipeline, materials, procedural Hull shader, Smart-UV unwrap, Cycles bake, glTF export, preview renders |
| `build_ships.py` | the five hull builders (`wren`, `talon`, `bastion`, `mule`, `seer`) and the build driver |
| `inspect_glb.py` | prints node names, triangle counts, materials of GLBs (plain Python, no Blender) |
| `previews/` | `<ship>.png` hero render (base hull) and `<ship>_sheet.png` (base, all ranks front, rear, top) |
| `_build/` | intermediate baked PNGs and views (git-ignored) |

## Regenerate

From the repo root (Git Bash):

```
"C:/Program Files/Blender Foundation/Blender 5.2/blender.exe" -b --factory-startup -P tools/blender/build_ships.py --
```

Options after `--`:

- ship names (`wren talon bastion mule seer`) to build only those (the default is all of them)
- `--no-bake`: geometry and preview only, with the procedural Hull shader. Use it for fast iteration; it does not write a GLB.
- `--no-preview`: skip the renders
- `--tex 1024`: Hull texture size (the default is 2048)

Each ship takes about 1–2.5 min on an RTX 4070 SUPER (OptiX). The first Cycles render
after a Blender update compiles kernels, which takes about 3 min once. Check results with:

```
python tools/blender/inspect_glb.py crates/client/assets/models/*.glb
```

Every export then runs `cargo run -p assetpipe --release -- MODEL.glb` (`shiplib.gpu_textures`), which turns
the baked PNGs into KTX2 (BC7/BC5, mips, zstd) in place, and `python tools/assetcheck.py` fails a model whose
textures stayed PNG (VRAM budget). See docs/STUDIO_PIPELINE.md.

## Contracts the client depends on

- Modelled with the nose at Blender +Y, up +Z, in meters. The glTF +Y-up export puts the nose at **-Z** (Bevy forward).
  The origin is the bounding-box centre.
- `Engine_N` empties sit at the nozzle exit centres. Their uniform scale is the nozzle radius in meters.
- `Gun_N` empties sit at the weapon muzzles.
- Upgrade parts are separate mesh nodes named `Rank3_*`, `Rank6_*` and `Rank9_*`. The client toggles their visibility.
  Everything else is joined into the `Hull` node.
- Materials:
  - `Hull`: baked 2048² base colour, metallic-roughness (G = roughness, B = metallic) and tangent normal, embedded PNG.
  - `Livery`: flat 0.8 grey, metallic 0.3, roughness 0.4. The game tints it.
  - `Glow`: emissive white, strength 5.
  - `Glass`: dark, metallic 0.5, roughness 0.05.
  - `EngineCore`: warm emissive.
  - The preview renders tint Livery red and Glow cyan only to show how they read in game. The GLBs keep the neutral values.

## How the Hull texture is made

`make_hull_procedural` builds a node graph in object space:
- staggered panel seams on X/Y/Z planes, masked by surface orientation, plus sparse irregular Voronoi sub-panels
- per-panel tint and roughness variation
- AO and streak grime
- Bevel-node edge wear, compared against the true normal
- a bump height for the seams

For Seer, it builds an organic variant instead: scales, ribs and vein grooves.

`bake_hull` bakes base, roughness and metal through an Emission bake and the normal map through a NORMAL bake. The Hull node and the Rank nodes share one Smart-UV atlas. The script then packs metallic-roughness and rebuilds the material from plain image textures, so glTF carries everything.

---

# Enemy ships and environment props

`build_props.py` (with helpers in `proplib.py`, which imports `shiplib.py` and never modifies it) builds the
enemy ships and environment props.

| file | role |
|---|---|
| `proplib.py` | `Prop` (a `shiplib.Ship` with any material order, booleans, a bevel blow-up fallback), slab/jag/nozzle/sweep/bezier/spike/cable helpers, the kitbash `greebles` scatter, `scale_space` and `overlay_hull` (rust, scorch, heat tint, stripes), the `Stone` and `Rock` procedural sources, equi-angular cube-map UVs, and a generic Cycles bake (same object or selected-to-active) |
| `build_props.py` | builders and driver for `raider gunship warden warlord sentinel station asteroids wreck gate ruin` |
| `previews/props_<name>.png`, `props_<name>_sheet.png` | hero render and a 2x2 sheet. `station`, `gate` and `asteroids` also have `_closeup.png` |
| `_build_props/` | baked PNGs and per-asset logs (git-ignored) |

```
"C:/Program Files/Blender Foundation/Blender 5.2/blender.exe" -b --factory-startup -P tools/blender/build_props.py -- [names...] [--no-bake] [--no-preview] [--tex 2048]
```

Each asset takes 1–2 min. The station takes about 5 min because its AO and bevel bake is heavy. Run one asset per Blender call
if you want logs per asset.

| GLB | nodes | materials |
|---|---|---|
| `npc_raider/gunship/warden/warlord/sentinel.glb` | `Hull`, `Engine_N`, `Gun_N` | Hull (baked), Livery, Glow, Glass, EngineCore. Each uses only the materials it needs |
| `station.glb` | `Station` (static), `Ring` (spokes, bearing collar and habitat ring; spins about Blender Z / Bevy Y). The origin is the centre | Hull (baked), Livery, Glow (warm white), Glass (solar cells) |
| `asteroids.glb` | `Rock_0`…`Rock_5`, each with its origin at its own centre, radius about 2 m | Rock (baked neutral grey-brown, tint per ore), Crystal (emissive white, only on Rock_3–5) |
| `wreck.glb` | `Wreck_Bow`, `Wreck_Mid`, `Wreck_Stern`, `Wreck_Wing`, `Wreck_Debris` (a debris field of about 25 m) | Hull (baked, scorched), Glow (ember rims, warm orange) |
| `ancient_gate.glb` | `Gate_Seg_0..11`, `Gate_Key_0..7`, `Gate_Pylon_0..3`. These are linked duplicates of three meshes, so the 2048 atlas only covers one segment, one keystone and one pylon. The ring lies in the XZ plane, its hole axis is Blender Y and its diameter is about 800 m | Stone (baked), Glow |
| `ruin.glb` | `Ruin_Platform`, `Ruin_Monoliths`, `Ruin_Arch`, `Ruin_Fragments` (about 120 m) | Stone (baked), Glow |

Notes:
- The enemy ships are scaled to their exact target length after building (raider 20, gunship 32, warden 26, warlord 66, sentinel 130 m).
  Empty locations and scales are scaled with them.
- Rocks are baked from a 327k-tri high poly onto a 5k-tri low poly (selected-to-active), with a cube-map UV of 6 charts per rock.
  Crystals are joined in after the bake.
- Wreck tears are EXACT booleans against jagged cutter sheets. Each cutter face is randomly Hull or Glow, so the torn rims glow like embers.
- Gate and ruin glyphs are real geometry (Glow strokes) sitting in carved channels.

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

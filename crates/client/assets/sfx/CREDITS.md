# Sound effect credits

All sound effects are CC0 (public domain dedication, https://creativecommons.org/publicdomain/zero/1.0/),
checked on each pack's page and in its bundled License.txt. Attribution is not required; given as a courtesy.

| Pack | Author | Source | Files here |
|---|---|---|---|
| Sci-Fi Sounds 1.0 | Kenney (www.kenney.nl) | https://kenney.nl/assets/sci-fi-sounds | laser_small_*, turret_*, main_gun_charge, shield_hit_*, explosion_small_*, boom_low_*, warp_charge, engine_small/medium/large, boost_loop, scan, dock, undock |
| Impact Sounds 1.0 | Kenney | https://kenney.nl/assets/impact-sounds | hull_hit_heavy_*, hull_hit_medium_*, mining_hit |
| Interface Sounds 1.0 | Kenney | https://kenney.nl/assets/interface-sounds | ui_click, ui_confirm, ui_error |
| 50 CC0 Sci-Fi SFX | rubberduck | https://opengameart.org/content/50-cc0-sci-fi-sfx | missile_launch (rocket_01), warp_jump (teleport_02), station_ambience (loop_machine_01), ui_lock (beep_01), alarm (beep_02) |

Original file for each name:

| File | Original |
|---|---|
| laser_small_0..4 | laserSmall_000..004 |
| turret_0..2 | laserLarge_000..002 (played pitched down in game) |
| main_gun_charge | laserLarge_004 |
| boom_low_0, boom_low_1 | lowFrequency_explosion_000, _001 |
| shield_hit_0..2 | forceField_000..002 |
| hull_hit_heavy_0..2, hull_hit_medium_0..2 | impactMetal_heavy_000..002, impactMetal_medium_000..002 |
| explosion_small_0..2 | explosionCrunch_000..002 |
| warp_charge | engineCircular_000 |
| engine_small, engine_medium, engine_large | spaceEngineSmall_000, spaceEngine_000, spaceEngineLow_000 |
| boost_loop | thrusterFire_000 |
| mining_hit | impactMining_000 |
| scan | computerNoise_000 |
| dock, undock | doorClose_000, doorOpen_000 |
| ui_click, ui_confirm, ui_error | click_001, confirmation_001, error_004 |

Processing: down-mixed to mono 44.1 kHz (the game positions sounds itself), peak-normalized to −1 dBFS
and encoded as Ogg Vorbis q5 with ffmpeg; per-sound volume is set in `crates/client/src/audio.rs`.

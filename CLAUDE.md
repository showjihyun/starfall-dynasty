# STARFALL DYNASTY — agent notes

Rust workspace: `shared` (protocol, rules, galaxy), `history` (event-sourced chronicle), `sim` (authoritative
simulation `Game` as a pure library: rules, store, record/replay, scenarios, metrics), `server` (WebSocket binary around `sim`), `view` (client presentation logic, no engine), `client` (Bevy 0.20 renderer),
`bot` (headless clients, load tests).
The game is designed to be its own harness — read `docs/HARNESS.md` before changing simulation or visuals.

**Always read first:** [`docs/AI_게임_개발_지침.md`](docs/AI_게임_개발_지침.md) — how to build this 3D game with AI
(one clear goal per round, detailed prompts/specs, test early and often, review AI output, PC-first platform,
no feature creep, never trust untested output, keep measuring performance). Its checklist applies to every task.

## Verify every change (fast loop first)

- One command before every commit: `python tools/gate.py` picks the checks below from what changed and prints one
  verdict (`--all`, `--visual` for artcheck, `--list`). Studio tooling map and next steps: docs/STUDIO_PIPELINE.md.
  Model budgets: `python tools/assetcheck.py` (tools/asset_budgets.json; raise a budget only with the model that needs it).
  Models ship with KTX2 textures (BC7/BC5 + mips) and stored tangents: `cargo run -p assetpipe --release -- MODEL.glb` (Blender export runs it).
  CI: `.github/workflows/gate.yml` runs `gate.py --all` on push (no artcheck).
  Performance budget: `python tools/perfcheck.py [--client]` (per-machine baseline); dependencies: `cargo deny check`.
  Where the frame goes (engine included): `python tools/tracy.py` (Tracy capture of the 1000-ship bench, table per system).
  Story and cinematics: docs/STORY.md; `STARFALL_FIXED_FPS=60 client --cinematic opening --shot X.png --shot-at T` (T on the cinematic clock); in a live game the script command `opening` plays it as the campaign's first start does, `comms N` opens the call after chapter N+1 (lines in `assets/cinematics/comms.ron`, `view::comms`).
  Voice lines: after editing any subtitle or comms line run `python tools/voice.py` (Docker, Chatterbox Multilingual V3; presets in `assets/voice/voices.json`; Whisper listens to each take and misheard ones are redone, `--retake` redoes lines still flagged in the manifest); `cargo test -p view --test voice` fails until you do.

- Logic: `cargo test --release -p shared -p history -p sim -p view` (seconds, no engine). Client math and HUD
  drawing live in `view` (headless egui tests in `crates/view/tests/hud.rs`); keep `client` to Bevy glue; that glue is tested headless too (`cargo test --release -p client`: Bevy systems on `MinimalPlugins`, no window/GPU — e.g. material state must not depend on the frame a model finished loading). Network: `cargo build --release -p server && cargo test --release -p bot`.
- Reproduce server state: `server --dev --record DIR`, then `cargo run -p sim --release --example replay -- DIR [--twice] [--dump SYS@TICK]`.
  See it in 3D: `STARFALL_FIXED_FPS=60 client --replay DIR [--as NAME] [--from SECS] [--speed X] --shot X.png --shot-at T`
  (prints the digest verdict; same command → identical image).
- Visuals, by numbers: `STARFALL_FIXED_FPS=60 STARFALL_HIDE_UI=1 client --showroom 7 --shot X.png --shot-at 8` (same command → identical image), then `python tools/imgdiff.py before.png after.png [--crop X,Y,W,H]` or `--stats`.
- War zones are fought on named fronts (`galaxy::fronts`, tuning/galaxy.ron `fronts`); the server reports each side's ships, hull + shield and losses per front every second (`ServerMsg::Fronts`, sim/src/game/fronts.rs) and the HUD shows who is winning (`view::hud::war_report`). In a crowded battle only the target and the nearest ships get labels (`view::hud::declutter`, the rest are dots), and explosion flash lights and capital fireballs are capped (client/src/fx.rs). Engine effects follow size on screen (`view::anim::engine_lod`: nozzle glows and bores off below 24 px ship radius, plumes below 1.5 px) and shield bubbles are hidden while they would draw nothing (`view::anim::shield_shown`) — each is a transparent draw, thousands of them were the largest render cost.
- Generated systems (ids 10+) are named after real objects JWST observed: `tuning/catalog.ron` (name, designation, kind, constellation, suited star kind, one-line note shown in the system's lore); add or edit entries there, no recompile. The ten story systems keep their names; positions, tiers and stars do not depend on the catalog (`galaxy::generate`).
- 3D combat/terrain design (main goal): docs/3D_전투_설계.md — which 3D move beats which, with the numbers and scenarios behind it; rerun them after changing combat/npcs/galaxy/minerals tuning.
- Art direction: docs/ART_DIRECTION.md is the visual style guide (prompts, palette, rules); colours come only from
  `crates/view/src/palette.rs`. After a visual change run `python tools/artcheck.py` (renders the reference scenes and
  compares them with docs/art/reference/); approve an intended new look with `--update` in the same commit.
- Campaign QA: `cargo run -p sim --release --example campaign` — a fresh captain plays chapters 1–5 like a player (no dev commands; fights, retreats, mines, refines, upgrades) in accelerated game time and reports per chapter time, deaths and where it got stuck (`sim::playthrough`, results in docs/QA_CAMPAIGN.md); rerun after combat, economy or scenario changes.
- Play QA (no human needed): `cargo run -p bot --release --example qa` plays docs/QA_PHASE2_3.md with two headless captains
  against a real `--dev` server and prints PASS/FAIL per item; `--keep --data DIR` leaves the world up for
  `client --token … --auto --script "station 1" --shot …`; `client --clicks "x,y@t;…"` injects real mouse clicks.
- Performance, as text: `cargo run -p sim --release --example bench 1000`, `client --bench 15` (`STARFALL_BENCH_SCALE=15`: a real 1020-captain war on Vesta Front's fronts, the sim on its own thread — src/battle.rs; `STARFALL_BENCH_SYNTH=1` for the old synthetic 1020-ship scene), `cargo run -p bot --release --example swarm -- ws://127.0.0.1:PORT 1000 60`.
  Both benches end with a span-tree profile (`shared::prof`; live: `server --prof`). Add `shared::prof::span("name")`
  around new heavy phases so they show up there.

## Sim-first for gameplay

New rules or balance changes start as a scenario spec: write the expected behaviour as JSON in
`crates/sim/scenarios/` (format in docs/HARNESS.md), run it with
`cargo run -p sim --release --example scenario -- FILE --trace 5` until it passes, then refresh
`cargo run -p sim --release --example balance -- --write docs/BALANCE.md` and read the diff.
`cargo test -p sim --test scenarios` runs every spec and checks determinism. Specs can check fight numbers
(`dps`, `ttk`, `dealt`, `taken`); `--metrics DIR` writes per-second JSONL (`sim::metrics`; live: `server --metrics FILE`).
Balance numbers (hulls, module growth, size classes, upgrade costs, NPCs, escorts, combat rules) live in
`tuning/*.ron`, read at run time — edit them, rerun balance/scenarios, no recompile (`STARFALL_TUNING=DIR` for an alternative set).
Never add a new gameplay number as a code constant; put it in `tuning/` (`shared::tuning`).

## Invariants

- The simulation is deterministic: ordered maps (`BTreeMap`/`BTreeSet`) wherever iteration order can affect state; no wall clock or OS randomness in game logic (time arrives via `tick(dt, unix)`, gameplay randomness via `shared::rng`).
- All state changes enter through `ClientMsg` → `Game::handle`, so they are recorded and replayable.
- The client only renders server state; client-side animation is a function of the clock, not accumulated per frame.
- Kill a running `server.exe` before rebuilding it (Windows locks the binary).

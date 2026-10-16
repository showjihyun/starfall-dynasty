# STARFALL DYNASTY — agent notes

Rust workspace: `shared` (protocol, rules, galaxy), `history` (event-sourced chronicle), `server` (authoritative
simulation `Game` + WebSocket binary), `view` (client presentation logic, no engine), `client` (Bevy 0.20 renderer),
`bot` (headless clients, load tests).
The game is designed to be its own harness — read `docs/HARNESS.md` before changing simulation or visuals.

## Verify every change (fast loop first)

- Logic: `cargo test --release -p shared -p history -p server -p view` (seconds, no engine). Client math and HUD
  drawing live in `view` (headless egui tests in `crates/view/tests/hud.rs`); keep `client` to Bevy glue. Network: `cargo build --release -p server && cargo test --release -p bot`.
- Reproduce server state: `server --dev --record DIR`, then `cargo run -p server --release --example replay -- DIR [--twice] [--dump SYS@TICK]`.
  See it in 3D: `STARFALL_FIXED_FPS=60 client --replay DIR [--as NAME] [--from SECS] [--speed X] --shot X.png --shot-at T`
  (prints the digest verdict; same command → identical image).
- Visuals, by numbers: `STARFALL_FIXED_FPS=60 STARFALL_HIDE_UI=1 client --showroom 7 --shot X.png --shot-at 8` (same command → identical image), then `python tools/imgdiff.py before.png after.png [--crop X,Y,W,H]` or `--stats`.
- Performance, as text: `cargo run -p server --release --example bench 1000`, `client --bench 15` (`STARFALL_BENCH_SCALE=15` ≈ 1000 ships), `cargo run -p bot --release --example swarm -- ws://127.0.0.1:PORT 1000 60`.

## Sim-first for gameplay

New rules or balance changes start as a scenario spec: write the expected behaviour as JSON in
`crates/server/scenarios/` (format in docs/HARNESS.md), run it with
`cargo run -p server --release --example scenario -- FILE --trace 5` until it passes, then refresh
`cargo run -p server --release --example balance -- --write docs/BALANCE.md` and read the diff.
`cargo test -p server --test scenarios` runs every spec and checks determinism. Specs can check fight numbers
(`dps`, `ttk`, `dealt`, `taken`); `--metrics DIR` writes per-second JSONL (`server::metrics`; live: `server --metrics FILE`).
Balance numbers (hulls, module growth, size classes, upgrade costs, NPCs, escorts, combat rules) live in
`tuning/*.ron`, read at run time — edit them, rerun balance/scenarios, no recompile (`STARFALL_TUNING=DIR` for an alternative set).
Never add a new gameplay number as a code constant; put it in `tuning/` (`shared::tuning`).

## Invariants

- The simulation is deterministic: ordered maps (`BTreeMap`/`BTreeSet`) wherever iteration order can affect state; no wall clock or OS randomness in game logic (time arrives via `tick(dt, unix)`, gameplay randomness via `shared::rng`).
- All state changes enter through `ClientMsg` → `Game::handle`, so they are recorded and replayable.
- The client only renders server state; client-side animation is a function of the clock, not accumulated per frame.
- Kill a running `server.exe` before rebuilding it (Windows locks the binary).

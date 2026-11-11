//! STARFALL DYNASTY — client presentation logic without the engine.
//!
//! Everything here is a plain function of server state, input and the clock, so it is unit-tested
//! in seconds (`cargo test -p view`) without compiling Bevy. The client calls into it and only
//! moves the results onto entities and screens (docs/HARNESS.md, "클라이언트 순수 로직 분리").
//!
//! - [`anim`]: animation as a function of the clock (ramps, fades, turret swing, throttle).
//! - [`cinematic`]: in-engine cutscenes (camera, subtitles, cuts) as a function of the clock.
//! - [`comms`]: the chapter-end comms window (who speaks, typing, emblem), checked headless.
//! - [`voice`]: generated voice lines for those two (presets, manifest, timing checks).
//! - [`exchange`]: the station's trade goods market as its own screen.
//! - [`jobboard`]: the station's job board and the jobs held, as its own screen.
//! - [`fleet`]: the four fleet styles — who flies which, and each ship's model.
//! - [`interp`]: snapshot interpolation and own-ship extrapolation.
//! - [`relation`]: mine / friend / hostile / neutral, shared by the HUD and the star map.
//! - [`mapview`]: star map projection, zoom and pan between the galaxy and system charts.
//! - [`gauge`]: HUD shield/hull "just lost" trail and the hull-damage flash.
//! - [`dust`]: the space dust field that wraps around the camera.
//! - [`hud`]: egui drawing of the gauges and the ship status panel, checked headless.
//! - [`panels`]: window sections (station artifacts) that return what the player clicked.
//! - [`settings`]: sound levels, display mode and the Esc system menu.

pub mod anim;
pub mod cinematic;
pub mod comms;
pub mod dust;
pub mod exchange;
pub mod fleet;
pub mod fonts;
pub mod gauge;
pub mod hud;
pub mod jobboard;
pub mod interp;
pub mod keymap;
pub mod mapview;
pub mod pad;
pub mod palette;
pub mod panels;
pub mod relation;
pub mod settings;
pub mod voice;

pub use egui;

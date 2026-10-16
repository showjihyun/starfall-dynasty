//! STARFALL DYNASTY — client presentation logic without the engine.
//!
//! Everything here is a plain function of server state, input and the clock, so it is unit-tested
//! in seconds (`cargo test -p view`) without compiling Bevy. The client calls into it and only
//! moves the results onto entities and screens (docs/HARNESS.md, "클라이언트 순수 로직 분리").
//!
//! - [`interp`]: snapshot interpolation and own-ship extrapolation.
//! - [`relation`]: mine / friend / hostile / neutral, shared by the HUD and the star map.
//! - [`mapview`]: star map projection, zoom and pan between the galaxy and system charts.
//! - [`gauge`]: HUD shield/hull "just lost" trail and the hull-damage flash.
//! - [`dust`]: the space dust field that wraps around the camera.
//! - [`hud`]: egui drawing of the gauges and the ship status panel, checked headless.
//! - [`panels`]: window sections (station artifacts) that return what the player clicked.

pub mod dust;
pub mod gauge;
pub mod hud;
pub mod interp;
pub mod mapview;
pub mod palette;
pub mod panels;
pub mod relation;

pub use egui;

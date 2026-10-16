//! STARFALL DYNASTY — the authoritative simulation as a pure, synchronous library.
//!
//! No network, no async runtime, no engine: the `server` binary feeds it `ClientMsg`s and ships what
//! it emits over WebSockets, the client replays recordings with it, and every harness tool runs it
//! directly (docs/HARNESS.md).
//!
//! - [`game`]: state, rules and the fixed-step tick (`Game::handle`, `Game::tick`).
//! - [`store`]: accounts and profiles, persisted as JSON.
//! - [`harness`]: record and replay sessions, state digests.
//! - [`scenario`]: data-driven gameplay specs, run headless.
//! - [`metrics`]: hits, kills and rewards as time series.

pub mod game;
pub mod harness;
pub mod metrics;
pub mod scenario;
pub mod store;

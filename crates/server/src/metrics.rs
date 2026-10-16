//! Gameplay observability (docs/HARNESS.md, "게임플레이 관측성"): damage, kills and rewards as
//! time series, so balance curves are checked as numbers rather than by watching fights.
//!
//! When `Game::metrics` is `Some`, the simulation pushes a [`Metric`] for every landed hit, every
//! kill and every bounty paid. Collecting them never changes the simulation (the digest is the
//! same with or without). [`MetricsLog`] folds them into per-captain totals and JSONL lines:
//!
//! ```text
//! {"t":5.0,"ev":"sample","player":"Ares","dealt":96.0,"taken":12.5,"dps_out":96.0,"dps_in":12.5,"hull":0.97,"shield":0.88,"kills":1,"credits":1680}
//! {"t":6.35,"ev":"kill","sys":3,"victim":"Raider Lv1","killer":"Ares","ttk":4.1}
//! {"t":6.35,"ev":"reward","player":"Ares","credits":180}
//! ```
//!
//! `sample` lines cover one window (default 1 s): damage dealt by the captain's side (flagship and
//! escorts) and taken by its ships in that window, the same as per-second rates, and the
//! flagship's hull/shield fractions (0 when down). `ttk` is seconds from the victim's first hit in
//! the fight to its death.

use crate::game::Game;
use serde::{Deserialize, Serialize};
use shared::galaxy::SystemId;
use shared::protocol::{EntityId, PlayerId, TICK_HZ};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "ev", rename_all = "snake_case")]
pub enum Metric {
    /// A landed hit: shield points absorbed and hull points lost.
    Hit { t: f64, sys: SystemId, from: EntityId, from_side: Option<PlayerId>, to: EntityId, to_side: Option<PlayerId>, shield: f32, hull: f32 },
    Kill { t: f64, sys: SystemId, victim: EntityId, victim_name: String, victim_side: Option<PlayerId>, killer: Option<PlayerId>, ttk: f32 },
    /// Bounty credits paid to a player for an NPC kill.
    Reward { t: f64, player: PlayerId, credits: u64 },
}

/// One captain's running totals.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Totals {
    pub dealt: f32,
    pub taken: f32,
    pub hits_dealt: u32,
    pub hits_taken: u32,
    pub kills: u32,
    pub credits_earned: u64,
    /// Seconds of the first and last hit dealt.
    pub first_hit: Option<f64>,
    pub last_hit: Option<f64>,
    /// Time-to-kill of every victim this captain killed.
    pub ttk: Vec<f32>,
}

impl Totals {
    /// Average damage per second over the time this captain was dealing it (first to last hit).
    pub fn dps(&self) -> f32 {
        match (self.first_hit, self.last_hit) {
            (Some(a), Some(b)) => self.dealt / ((b - a) as f32 + 1.0 / TICK_HZ),
            _ => 0.0,
        }
    }

    pub fn mean_ttk(&self) -> Option<f32> {
        (!self.ttk.is_empty()).then(|| self.ttk.iter().sum::<f32>() / self.ttk.len() as f32)
    }
}

#[derive(Serialize)]
struct Sample<'a> {
    t: f64,
    ev: &'static str,
    player: &'a str,
    dealt: f32,
    taken: f32,
    dps_out: f32,
    dps_in: f32,
    hull: f32,
    shield: f32,
    kills: u32,
    credits: u64,
}

#[derive(Serialize)]
struct KillLine<'a> {
    t: f64,
    ev: &'static str,
    sys: SystemId,
    victim: &'a str,
    killer: Option<&'a str>,
    ttk: f32,
}

#[derive(Serialize)]
struct RewardLine<'a> {
    t: f64,
    ev: &'static str,
    player: &'a str,
    credits: u64,
}

/// Folds `Game::metrics` into totals and JSONL lines; call [`MetricsLog::observe`] after every tick.
pub struct MetricsLog {
    window_ticks: u64,
    /// Emit a sample for every online captain each window (scenarios), not only those in combat (live server).
    every_player: bool,
    window: BTreeMap<PlayerId, (f32, f32)>,
    pub totals: BTreeMap<PlayerId, Totals>,
}

/// Rounds for readable, stable JSON (values are already deterministic).
fn r2(v: f32) -> f32 {
    (v * 100.0).round() / 100.0
}

impl MetricsLog {
    pub fn new(window_secs: f32, every_player: bool) -> MetricsLog {
        MetricsLog { window_ticks: (window_secs * TICK_HZ).round().max(1.0) as u64, every_player, window: BTreeMap::new(), totals: BTreeMap::new() }
    }

    /// Starts collection in the game.
    pub fn attach(g: &mut Game) {
        g.metrics.get_or_insert_with(Vec::new);
    }

    /// Takes this tick's events from the game, updates the totals and returns the JSONL lines due now.
    pub fn observe(&mut self, g: &mut Game) -> Vec<String> {
        // Not attached: nothing is collected, so nothing to report.
        let Some(events) = g.metrics.as_mut().map(std::mem::take) else { return vec![] };
        let t = g.tick as f64 / TICK_HZ as f64;
        let name = |g: &Game, p: PlayerId| g.store.profile(p).map(|p| p.name.clone()).unwrap_or_else(|| format!("player {p}"));
        let mut lines = vec![];
        for ev in events {
            match ev {
                Metric::Hit { t: at, from_side, to_side, shield, hull, .. } => {
                    let d = shield + hull;
                    if let Some(p) = from_side {
                        let tot = self.totals.entry(p).or_default();
                        tot.dealt += d;
                        tot.hits_dealt += 1;
                        tot.first_hit.get_or_insert(at);
                        tot.last_hit = Some(at);
                        self.window.entry(p).or_default().0 += d;
                    }
                    if let Some(p) = to_side {
                        let tot = self.totals.entry(p).or_default();
                        tot.taken += d;
                        tot.hits_taken += 1;
                        self.window.entry(p).or_default().1 += d;
                    }
                }
                Metric::Kill { sys, victim_name, killer, ttk, .. } => {
                    if let Some(k) = killer {
                        let tot = self.totals.entry(k).or_default();
                        tot.kills += 1;
                        tot.ttk.push(ttk);
                    }
                    let kname = killer.map(|k| name(g, k));
                    lines.push(serde_json::to_string(&KillLine { t, ev: "kill", sys, victim: &victim_name, killer: kname.as_deref(), ttk: r2(ttk) }).unwrap());
                }
                Metric::Reward { player, credits, .. } => {
                    self.totals.entry(player).or_default().credits_earned += credits;
                    lines.push(serde_json::to_string(&RewardLine { t, ev: "reward", player: &name(g, player), credits }).unwrap());
                }
            }
        }
        if g.tick % self.window_ticks == 0 {
            let secs = self.window_ticks as f32 / TICK_HZ;
            let players: Vec<PlayerId> = if self.every_player { g.conn_of.keys().copied().collect() } else { self.window.keys().copied().collect() };
            for p in players {
                let (dealt, taken) = self.window.get(&p).copied().unwrap_or_default();
                let flag = g.player_entity(p).and_then(|(s, e)| g.ent(s, e)).filter(|e| !e.dead);
                let (hull, shield) = flag.map(|e| (e.hull / e.stats.max_hull.max(1.0), e.shield / e.stats.max_shield.max(1.0))).unwrap_or((0.0, 0.0));
                let prof = g.store.profile(p);
                let s = Sample {
                    t,
                    ev: "sample",
                    player: &name(g, p),
                    dealt: r2(dealt),
                    taken: r2(taken),
                    dps_out: r2(dealt / secs),
                    dps_in: r2(taken / secs),
                    hull: r2(hull),
                    shield: r2(shield),
                    kills: prof.map(|p| p.kills).unwrap_or(0),
                    credits: prof.map(|p| p.wallet.credits).unwrap_or(0),
                };
                lines.push(serde_json::to_string(&s).unwrap());
            }
            self.window.clear();
        }
        lines
    }
}

//! Development harness (docs/HARNESS.md): make a game session reproducible and checkable in text.
//!
//! - **Record** (`server --record DIR`): the data files at start-up are copied to `DIR/initial/` and the
//!   gameplay numbers (`tuning/*.ron`) to `DIR/tuning/`, then
//!   every command the simulation receives is appended to `DIR/session.jsonl`, tick by tick, together
//!   with the clock it saw, the account tokens it issued and a state digest every few seconds.
//! - **Replay** (`cargo run -p sim --release --example replay -- DIR`): rebuilds the simulation
//!   from `initial/` with the recorded tuning, feeds the same commands at the same ticks without any network or engine, and
//!   checks each recorded digest. A bug report becomes a folder that reproduces itself.
//! - **Digest**: a fingerprint of the whole simulation state (entities bit-exact, the store, the
//!   history), so "did my change alter the outcome?" is a one-line comparison.
//!
//! Recordings hold no credentials: every account token (in the copied store, in logins and in newly
//! issued ones) is replaced by a stable pseudonym, consistently, so logins still match on replay.
//! Replay builds its state in memory and writes nothing to disk.

use crate::game::{ConnId, Game};
use crate::store::Store;
use serde::{Deserialize, Serialize};
use shared::protocol::{ClientMsg, TICK_HZ};
use std::collections::HashMap;
use std::io::{BufRead, BufWriter, Write};
use std::path::{Path, PathBuf};

/// Something the network layer handed to the simulation.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Event {
    Msg(ConnId, ClientMsg),
    Close(ConnId),
}

/// First line of a session file.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Header {
    pub version: u32,
    pub unix: u64,
    pub dev_mode: bool,
    pub digest_every: u64,
}

/// One line per tick that had input, issued tokens, a clock change or a digest.
/// `events` are applied before the tick runs; `digest` is taken after it.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TickRecord {
    pub tick: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unix: Option<u64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub events: Vec<Event>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tokens: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub digest: Option<String>,
}

pub const SESSION_FILE: &str = "session.jsonl";
pub const FORMAT_VERSION: u32 = 1;
const DATA_FILES: [&str; 2] = ["profiles.json", "history.jsonl"];
/// Copy of `tuning/*.ron` inside a recording.
pub const TUNING_DIR: &str = "tuning";

/// FNV-1a, 64 bit: tiny, stable across platforms and runs (unlike the std hasher).
#[derive(Clone, Copy)]
struct Fnv(u64);

impl Fnv {
    fn new() -> Self {
        Fnv(0xcbf2_9ce4_8422_2325)
    }
    fn bytes(&mut self, b: &[u8]) {
        for x in b {
            self.0 ^= *x as u64;
            self.0 = self.0.wrapping_mul(0x0100_0000_01b3);
        }
    }
    fn u64(&mut self, v: u64) {
        self.bytes(&v.to_le_bytes());
    }
    fn f32(&mut self, v: f32) {
        self.bytes(&v.to_bits().to_le_bytes());
    }
}

/// Fingerprint of the simulation: every entity (positions bit-exact), every account and profile,
/// wrecks, market supply, alliances and the historical record.
pub fn digest(g: &Game) -> String {
    let mut h = Fnv::new();
    h.u64(g.tick);
    for (sys, s) in &g.systems {
        h.u64(*sys as u64);
        for e in s.ents.values() {
            h.u64(e.id as u64);
            h.bytes(format!("{:?}", e.kind).as_bytes());
            for v in [e.pos.x, e.pos.y, e.pos.z, e.rot.x, e.rot.y, e.rot.z, e.rot.w, e.vel.x, e.vel.y, e.vel.z, e.hull, e.shield] {
                h.f32(v);
            }
            h.u64(e.dead as u64 | (e.docked as u64) << 1);
        }
    }
    // Tokens are left out: recordings carry pseudonyms, the live server real ones.
    let mut data = g.store.data.clone();
    for a in data.accounts.values_mut() {
        a.token.clear();
    }
    h.bytes(serde_json::to_string(&data).unwrap_or_default().as_bytes());
    h.u64(g.history.events.len() as u64);
    if let Some(last) = g.history.events.last() {
        h.bytes(serde_json::to_string(last).unwrap_or_default().as_bytes());
    }
    format!("{:016x}", h.0)
}

/// Writes a session while a server runs.
pub struct Recorder {
    out: BufWriter<std::fs::File>,
    pending: Vec<Event>,
    last_unix: u64,
    digest_every: u64,
    /// Real token → pseudonym written to the recording.
    alias: HashMap<String, String>,
}

/// What a login with a token the recording never saw becomes: it fails on replay exactly as it did live.
const UNKNOWN_TOKEN: &str = "unknown-token";

impl Recorder {
    /// Copies the data files that `Game::new` is about to load, then opens the session file.
    pub fn start(dir: &Path, data_dir: &Path, unix: u64, dev_mode: bool) -> std::io::Result<Recorder> {
        let initial = dir.join("initial");
        std::fs::create_dir_all(&initial)?;
        let mut alias = HashMap::new();
        let profiles = data_dir.join(DATA_FILES[0]);
        if profiles.exists() {
            let mut data: crate::store::StoreData = serde_json::from_str(&std::fs::read_to_string(&profiles)?).map_err(std::io::Error::other)?;
            for a in data.accounts.values_mut() {
                let pseudo = format!("redacted-{}", a.id);
                alias.insert(std::mem::replace(&mut a.token, pseudo.clone()), pseudo);
            }
            std::fs::write(initial.join(DATA_FILES[0]), serde_json::to_string(&data)?)?;
        }
        let history = data_dir.join(DATA_FILES[1]);
        if history.exists() {
            std::fs::copy(&history, initial.join(DATA_FILES[1]))?;
        }
        // The numbers the simulation runs with, so a replay after a balance edit still reproduces.
        if let Some(src) = shared::tuning::Tuning::locate() {
            shared::tuning::Tuning::copy_files(&src, &dir.join(TUNING_DIR))?;
        }
        let mut out = BufWriter::new(std::fs::File::create(dir.join(SESSION_FILE))?);
        let digest_every = 100;
        writeln!(out, "{}", serde_json::to_string(&Header { version: FORMAT_VERSION, unix, dev_mode, digest_every })?)?;
        Ok(Recorder { out, pending: vec![], last_unix: unix, digest_every, alias })
    }

    pub fn event(&mut self, e: Event) {
        let e = match e {
            Event::Msg(c, ClientMsg::Hello { version, name, token: Some(t) }) => {
                let t = self.alias.get(&t).cloned().unwrap_or_else(|| UNKNOWN_TOKEN.into());
                Event::Msg(c, ClientMsg::Hello { version, name, token: Some(t) })
            }
            other => other,
        };
        self.pending.push(e);
    }

    /// Call right after `game.tick(..)`.
    pub fn end_tick(&mut self, g: &mut Game, unix: u64) -> std::io::Result<()> {
        let issued: Vec<String> = std::mem::take(&mut g.issued_tokens)
            .into_iter()
            .map(|t| {
                let n = self.alias.len();
                self.alias.entry(t).or_insert_with(|| format!("issued-{n}")).clone()
            })
            .collect();
        let mut rec = TickRecord { tick: g.tick, events: std::mem::take(&mut self.pending), tokens: issued, ..Default::default() };
        if unix != self.last_unix {
            rec.unix = Some(unix);
            self.last_unix = unix;
        }
        if g.tick % self.digest_every == 0 {
            rec.digest = Some(digest(g));
        }
        if rec.unix.is_some() || !rec.events.is_empty() || !rec.tokens.is_empty() || rec.digest.is_some() {
            writeln!(self.out, "{}", serde_json::to_string(&rec)?)?;
        }
        if rec.digest.is_some() {
            self.out.flush()?;
        }
        Ok(())
    }
}

/// Outcome of a replay.
#[derive(Clone, Debug, Default)]
pub struct ReplayReport {
    pub ticks: u64,
    pub events: usize,
    pub digests_checked: usize,
    /// First tick whose digest differed: (tick, recorded, replayed).
    pub divergence: Option<(u64, String, String)>,
    pub final_digest: String,
}

/// A loaded session: header plus tick records in order.
#[derive(Clone)]
pub struct Session {
    pub header: Header,
    pub records: Vec<TickRecord>,
    pub dir: PathBuf,
}

impl Session {
    pub fn load(dir: &Path) -> std::io::Result<Session> {
        let f = std::io::BufReader::new(std::fs::File::open(dir.join(SESSION_FILE))?);
        let mut lines = f.lines();
        let header: Header = serde_json::from_str(&lines.next().ok_or_else(|| std::io::Error::other("empty session"))??).map_err(std::io::Error::other)?;
        let lines: Vec<String> = lines.collect::<Result<_, _>>()?;
        let mut records = vec![];
        for (i, l) in lines.iter().enumerate() {
            if l.trim().is_empty() {
                continue;
            }
            match serde_json::from_str(l) {
                Ok(r) => records.push(r),
                // A server killed mid-write leaves a torn last line: replay up to it.
                Err(_) if i + 1 == lines.len() => break,
                Err(e) => return Err(std::io::Error::other(format!("line {}: {e}", i + 2))),
            }
        }
        Ok(Session { header, records, dir: dir.to_path_buf() })
    }

    /// Fresh simulation in the recorded starting state, built entirely in memory: the recording is
    /// read, never written, and no temporary files are involved. A recording that carries its
    /// tuning installs it process-wide first; older ones run with the current numbers.
    pub fn initial_game(&self) -> std::io::Result<Game> {
        let tuning = self.dir.join(TUNING_DIR);
        if tuning.is_dir() {
            shared::tuning::install(shared::tuning::Tuning::load(&tuning).map_err(std::io::Error::other)?);
        }
        let initial = self.dir.join("initial");
        let profiles = initial.join(DATA_FILES[0]);
        let store = if profiles.exists() {
            Store::from_data(serde_json::from_str(&std::fs::read_to_string(&profiles)?).map_err(std::io::Error::other)?)
        } else {
            Store::memory()
        };
        let log = initial.join(DATA_FILES[1]);
        let history = if log.exists() { history::HistoryEngine::from_log(std::io::BufReader::new(std::fs::File::open(&log)?))? } else { history::HistoryEngine::new() };
        let mut g = Game::new(store, history, self.header.unix);
        g.dev_mode = self.header.dev_mode;
        Ok(g)
    }

    /// Runs the whole session; `on_tick` sees the game after each tick (for dumps or custom checks).
    pub fn replay(&self, mut on_tick: impl FnMut(&Game)) -> std::io::Result<ReplayReport> {
        let mut r = self.clone().replayer()?;
        while r.step() {
            on_tick(&r.game);
        }
        Ok(r.finish())
    }

    /// A replay driven one tick at a time (the 3D client advances it with its clock).
    pub fn replayer(self) -> std::io::Result<Replayer> {
        let game = self.initial_game()?;
        let unix = self.header.unix;
        let last = self.records.last().map(|r| r.tick).unwrap_or(0);
        Ok(Replayer { game, records: self.records, next: 0, last, unix, report: ReplayReport::default() })
    }
}

/// Steps a recorded session tick by tick, checking each recorded digest as it passes.
pub struct Replayer {
    pub game: Game,
    records: Vec<TickRecord>,
    next: usize,
    /// Last recorded tick: the replay is over once the game reaches it.
    pub last: u64,
    unix: u64,
    pub report: ReplayReport,
}

impl Replayer {
    pub fn done(&self) -> bool {
        self.game.tick >= self.last
    }

    /// Feeds the recorded commands for the next tick and runs it; false once the recording is over.
    /// What the simulation sent (`game.out`: welcomes, snapshots, toasts…) stays there until the next
    /// step, so a viewer can pick out one connection's messages.
    pub fn step(&mut self) -> bool {
        if self.done() {
            return false;
        }
        let g = &mut self.game;
        g.out.clear();
        g.kicked.clear();
        g.issued_tokens.clear();
        let t = g.tick + 1;
        let rec = self.records.get(self.next).filter(|r| r.tick == t);
        if let Some(r) = rec {
            self.next += 1;
            g.token_queue.extend(r.tokens.iter().cloned());
            if let Some(u) = r.unix {
                self.unix = u;
            }
            for e in &r.events {
                self.report.events += 1;
                match e {
                    Event::Msg(c, m) => g.handle(*c, m.clone()),
                    Event::Close(c) => g.disconnect(*c),
                }
            }
        }
        g.tick(1.0 / TICK_HZ, self.unix);
        if let Some(want) = rec.and_then(|r| r.digest.as_ref()) {
            self.report.digests_checked += 1;
            let got = digest(g);
            if self.report.divergence.is_none() && &got != want {
                self.report.divergence = Some((g.tick, want.clone(), got));
            }
        }
        true
    }

    /// The report so far, with the current tick and state digest.
    pub fn finish(&self) -> ReplayReport {
        let mut rep = self.report.clone();
        rep.ticks = self.game.tick;
        rep.final_digest = digest(&self.game);
        rep
    }
}

impl ReplayReport {
    /// One-line verdict: `REPLAY OK: …` or `REPLAY DIVERGED at tick T: …`.
    pub fn verdict(&self) -> String {
        match &self.divergence {
            None => format!("REPLAY OK: every recorded digest matched ({} checked)", self.digests_checked),
            Some((tick, want, got)) => format!("REPLAY DIVERGED at tick {tick}: recorded {want}, replayed {got}"),
        }
    }
}

/// Text view of one system for agents and bug reports: one line per entity, sorted by id.
pub fn dump_system(g: &Game, sys: shared::galaxy::SystemId) -> String {
    let mut s = String::new();
    if let Some(sm) = g.systems.get(&sys) {
        s += &format!("tick {} system {} — {} entities\n", g.tick, sys, sm.ents.len());
        for e in sm.ents.values() {
            s += &format!(
                "{:>6} {:<18} {:<24} pos ({:>8.1},{:>8.1},{:>8.1}) hull {:>7.0}/{:<7.0} shield {:>6.0} owner {:?}{}{}\n",
                e.id,
                format!("{:?}", e.kind),
                e.name.chars().take(24).collect::<String>(),
                e.pos.x,
                e.pos.y,
                e.pos.z,
                e.hull,
                e.stats.max_hull,
                e.shield,
                e.owner,
                if e.docked { " docked" } else { "" },
                if e.dead { " dead" } else { "" }
            );
        }
    }
    s
}

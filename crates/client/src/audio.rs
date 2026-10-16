//! Sound effects. Recorded CC0 samples (assets/sfx, see its CREDITS.md) with variants, layers and
//! per-kind pitch ranges; the original procedural synth stays as a fallback for any missing file
//! (and for `--dump-sfx`). Also the continuous sounds: the flagship's engine and boost loops sized
//! to its hull class, station ambience while docked, and event cues (hull impacts, target lock,
//! docking, scanning, mining, low-hull alarm, errors and notices).

use crate::control::MainCamera;
use crate::state::{Screen, Session};
use bevy::audio::{AudioPlayer, AudioSink, AudioSinkPlayback, AudioSource, PlaybackSettings, Volume};
use bevy::prelude::*;
use shared::protocol::{EntityId, EntityKind};
use shared::ships::SizeClass;
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Sfx {
    Laser,
    Pulse,
    MainBeam,
    Missile,
    ExplosionSmall,
    ExplosionBig,
    ShieldHit,
    WarpCharge,
    WarpJump,
    HullHit,
    HullHitHeavy,
    MiningHit,
    Scan,
    Dock,
    Undock,
    Lock,
    Confirm,
    Error,
    Alarm,
}

/// One layer of a sound: variants (rotated so repeats differ), volume and a pitch/speed range.
struct LayerDef {
    files: &'static [&'static str],
    gain: f32,
    speed: (f32, f32),
}

const fn layer(files: &'static [&'static str], gain: f32, speed: (f32, f32)) -> LayerDef {
    LayerDef { files, gain, speed }
}

impl Sfx {
    /// (base volume, audible distance in world units)
    fn reach(self) -> (f32, f32) {
        match self {
            Sfx::Laser => (0.32, 3500.0),
            Sfx::Pulse => (0.42, 6500.0),
            Sfx::MainBeam => (0.95, 18000.0),
            Sfx::Missile => (0.5, 4500.0),
            Sfx::ExplosionSmall => (0.65, 7000.0),
            Sfx::ExplosionBig => (1.0, 20000.0),
            Sfx::ShieldHit => (0.38, 3500.0),
            Sfx::HullHit => (0.5, 3500.0),
            Sfx::HullHitHeavy => (0.65, 6000.0),
            Sfx::MiningHit => (0.25, 1500.0),
            Sfx::WarpCharge => (0.6, 1e9),
            Sfx::WarpJump => (0.8, 1e9),
            Sfx::Scan | Sfx::Dock | Sfx::Undock => (0.45, 1e9),
            Sfx::Lock => (0.35, 1e9),
            Sfx::Confirm => (0.3, 1e9),
            Sfx::Error => (0.4, 1e9),
            Sfx::Alarm => (0.4, 1e9),
        }
    }

    /// Recorded layers, played together.
    fn layers(self) -> &'static [LayerDef] {
        const LASER: &[LayerDef] = &[layer(&["laser_small_0", "laser_small_1", "laser_small_2", "laser_small_3", "laser_small_4"], 1.0, (0.95, 1.1))];
        // Turret batteries: the large laser pitched down reads as a heavy cannon.
        const PULSE: &[LayerDef] = &[layer(&["turret_0", "turret_1", "turret_2"], 1.0, (0.68, 0.8))];
        // Main gun: a deep charged discharge over a low thunderclap.
        const MAIN: &[LayerDef] = &[layer(&["main_gun_charge"], 0.8, (0.5, 0.56)), layer(&["boom_low_0", "boom_low_1"], 1.0, (0.85, 0.95))];
        const MISSILE: &[LayerDef] = &[layer(&["missile_launch"], 1.0, (0.95, 1.08))];
        const EXP_S: &[LayerDef] = &[layer(&["explosion_small_0", "explosion_small_1", "explosion_small_2"], 1.0, (0.9, 1.1))];
        const EXP_B: &[LayerDef] = &[layer(&["boom_low_0"], 1.0, (0.8, 0.9)), layer(&["explosion_small_1", "explosion_small_2"], 0.8, (0.62, 0.72))];
        const SHIELD: &[LayerDef] = &[layer(&["shield_hit_0", "shield_hit_1", "shield_hit_2"], 1.0, (1.0, 1.25))];
        const HULL: &[LayerDef] = &[layer(&["hull_hit_medium_0", "hull_hit_medium_1", "hull_hit_medium_2"], 1.0, (0.9, 1.1))];
        const HULL_H: &[LayerDef] = &[layer(&["hull_hit_heavy_0", "hull_hit_heavy_1", "hull_hit_heavy_2"], 1.0, (0.75, 0.9))];
        const MINING: &[LayerDef] = &[layer(&["mining_hit"], 1.0, (0.9, 1.15))];
        const WARP_C: &[LayerDef] = &[layer(&["warp_charge"], 1.0, (1.25, 1.25))];
        const WARP_J: &[LayerDef] = &[layer(&["warp_jump"], 1.0, (1.0, 1.0)), layer(&["boom_low_1"], 0.6, (0.75, 0.75))];
        const SCAN: &[LayerDef] = &[layer(&["scan"], 1.0, (1.0, 1.0))];
        const DOCK: &[LayerDef] = &[layer(&["dock"], 1.0, (0.8, 0.8))];
        const UNDOCK: &[LayerDef] = &[layer(&["undock"], 1.0, (0.8, 0.8))];
        const LOCK: &[LayerDef] = &[layer(&["ui_lock"], 1.0, (1.0, 1.0))];
        const CONFIRM: &[LayerDef] = &[layer(&["ui_confirm"], 1.0, (1.0, 1.0))];
        const ERROR: &[LayerDef] = &[layer(&["ui_error"], 1.0, (1.0, 1.0))];
        const ALARM: &[LayerDef] = &[layer(&["alarm"], 1.0, (0.9, 0.9))];
        match self {
            Sfx::Laser => LASER,
            Sfx::Pulse => PULSE,
            Sfx::MainBeam => MAIN,
            Sfx::Missile => MISSILE,
            Sfx::ExplosionSmall => EXP_S,
            Sfx::ExplosionBig => EXP_B,
            Sfx::ShieldHit => SHIELD,
            Sfx::HullHit => HULL,
            Sfx::HullHitHeavy => HULL_H,
            Sfx::MiningHit => MINING,
            Sfx::WarpCharge => WARP_C,
            Sfx::WarpJump => WARP_J,
            Sfx::Scan => SCAN,
            Sfx::Dock => DOCK,
            Sfx::Undock => UNDOCK,
            Sfx::Lock => LOCK,
            Sfx::Confirm => CONFIRM,
            Sfx::Error => ERROR,
            Sfx::Alarm => ALARM,
        }
    }
}

#[derive(Resource, Default)]
pub struct SfxQueue(pub Vec<(Sfx, Option<Vec3>)>);

impl SfxQueue {
    pub fn play(&mut self, s: Sfx, at: Option<Vec3>) {
        self.0.push((s, at));
    }
}

struct Layer {
    variants: Vec<Handle<AudioSource>>,
    gain: f32,
    speed: (f32, f32),
}

#[derive(Resource)]
struct SfxBank {
    recorded: HashMap<Sfx, Vec<Layer>>,
    synth: HashMap<Sfx, Handle<AudioSource>>,
}

pub struct AudioFxPlugin;

impl Plugin for AudioFxPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SfxQueue>()
            .add_systems(Startup, build_bank)
            .add_systems(Update, (play_queue, (event_sounds, ship_loops).run_if(in_state(Screen::Playing))));
    }
}

const RATE: u32 = 44_100;

struct Synth {
    seed: u32,
}

impl Synth {
    fn noise(&mut self) -> f32 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 17;
        self.seed ^= self.seed << 5;
        (self.seed as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
}

fn render(secs: f32, mut f: impl FnMut(f32, &mut Synth) -> f32) -> Vec<f32> {
    let n = (secs * RATE as f32) as usize;
    let mut s = Synth { seed: 0x1234_5678 };
    (0..n).map(|i| f(i as f32 / RATE as f32, &mut s)).collect()
}

/// One-pole low-pass, in place.
fn lowpass(x: &mut [f32], cutoff: f32) {
    let a = 1.0 - (-2.0 * std::f32::consts::PI * cutoff / RATE as f32).exp();
    let mut y = 0.0;
    for v in x.iter_mut() {
        y += a * (*v - y);
        *v = y;
    }
}

fn env(t: f32, attack: f32, decay: f32) -> f32 {
    if t < attack { t / attack } else { (-(t - attack) / decay).exp() }
}

fn wav(samples: &[f32]) -> Vec<u8> {
    let peak = samples.iter().fold(0.0f32, |m, v| m.max(v.abs())).max(1e-6);
    let data: Vec<u8> = samples.iter().flat_map(|v| (((v / peak) * 0.9 * i16::MAX as f32) as i16).to_le_bytes()).collect();
    let mut out = Vec::with_capacity(44 + data.len());
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data.len() as u32).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&1u16.to_le_bytes()); // mono
    out.extend_from_slice(&RATE.to_le_bytes());
    out.extend_from_slice(&(RATE * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out.extend_from_slice(&data);
    out
}

fn synth(kind: Sfx) -> Vec<f32> {
    use std::f32::consts::TAU;
    match kind {
        Sfx::Laser => {
            let mut ph = 0.0;
            render(0.22, |t, s| {
                let f = 2200.0 * (-t * 14.0).exp() + 280.0;
                ph += f / RATE as f32;
                let sq = if (ph * TAU).sin() > 0.0 { 1.0 } else { -1.0 };
                (sq * 0.5 + (ph * TAU * 2.01).sin() * 0.4 + s.noise() * 0.05) * env(t, 0.003, 0.06)
            })
        }
        Sfx::Pulse => {
            let mut ph = 0.0;
            let mut body = render(0.45, |t, s| {
                let f = 140.0 * (-t * 9.0).exp() + 45.0;
                ph += f / RATE as f32;
                (ph * TAU).sin() * env(t, 0.002, 0.12) + s.noise() * env(t, 0.001, 0.03) * 0.8
            });
            lowpass(&mut body, 2400.0);
            body
        }
        Sfx::MainBeam => {
            // Charge whine, then a deep thunderclap with a crackling tail.
            let mut ph = 0.0;
            let mut ph2 = 0.0;
            let mut x = render(2.2, |t, s| {
                let charge = if t < 0.35 {
                    let f = 300.0 + 1500.0 * (t / 0.35).powi(2);
                    ph += f / RATE as f32;
                    (ph * TAU).sin() * (t / 0.35) * 0.35
                } else {
                    0.0
                };
                let tb = (t - 0.35).max(0.0);
                let boom = if t >= 0.35 {
                    ph2 += (55.0 + 40.0 * (-tb * 3.0).exp()) / RATE as f32;
                    (ph2 * TAU).sin() * env(tb, 0.004, 0.55) * 1.2 + s.noise() * env(tb, 0.002, 0.35) * 0.9
                } else {
                    0.0
                };
                charge + boom
            });
            lowpass(&mut x, 3200.0);
            x
        }
        Sfx::Missile => {
            let mut x = render(0.9, |t, s| s.noise() * env(t, 0.05, 0.35) + (t * 900.0 * TAU).sin() * 0.05 * env(t, 0.01, 0.2));
            lowpass(&mut x, 1800.0);
            x
        }
        Sfx::ExplosionSmall | Sfx::ExplosionBig => {
            let big = kind == Sfx::ExplosionBig;
            let len = if big { 3.2 } else { 1.4 };
            let mut ph = 0.0;
            let mut x = render(len, |t, s| {
                ph += (if big { 38.0 } else { 70.0 }) * (1.0 - t / len * 0.5) / RATE as f32;
                s.noise() * env(t, 0.004, if big { 0.9 } else { 0.35 }) + (ph * TAU).sin() * env(t, 0.01, if big { 1.2 } else { 0.4 }) * 1.1
            });
            lowpass(&mut x, if big { 900.0 } else { 1600.0 });
            x
        }
        Sfx::ShieldHit => render(0.5, |t, s| {
            let partials = [620.0, 1137.0, 1611.0, 2390.0];
            let tone: f32 = partials.iter().enumerate().map(|(i, f)| (t * f * TAU).sin() / (i as f32 + 1.0)).sum();
            tone * env(t, 0.002, 0.12) * 0.6 + s.noise() * env(t, 0.001, 0.02) * 0.4
        }),
        Sfx::WarpCharge => {
            let mut ph = 0.0;
            render(4.0, |t, s| {
                let f = 80.0 + 520.0 * (t / 4.0).powi(2);
                ph += f / RATE as f32;
                ((ph * TAU).sin() + (ph * TAU * 1.5).sin() * 0.4) * (t / 4.0).min(1.0) * 0.6 + s.noise() * 0.03
            })
        }
        Sfx::WarpJump => {
            let mut x = render(1.6, |t, s| s.noise() * env(t, 0.02, 0.4) + (t * 60.0 * TAU).sin() * env(t, 0.01, 0.6));
            lowpass(&mut x, 1200.0);
            x
        }
        // Recorded-only cues have no procedural fallback.
        _ => vec![0.0; 64],
    }
}

/// `client --dump-sfx DIR`: write every synthesized effect as a WAV for offline inspection.
pub fn dump(dir: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    for k in ALL {
        std::fs::write(dir.join(format!("{k:?}.wav")), wav(&synth(k)))?;
    }
    Ok(())
}

const ALL: [Sfx; 9] = [Sfx::Laser, Sfx::Pulse, Sfx::MainBeam, Sfx::Missile, Sfx::ExplosionSmall, Sfx::ExplosionBig, Sfx::ShieldHit, Sfx::WarpCharge, Sfx::WarpJump];

const ALL_KINDS: [Sfx; 19] = [
    Sfx::Laser,
    Sfx::Pulse,
    Sfx::MainBeam,
    Sfx::Missile,
    Sfx::ExplosionSmall,
    Sfx::ExplosionBig,
    Sfx::ShieldHit,
    Sfx::WarpCharge,
    Sfx::WarpJump,
    Sfx::HullHit,
    Sfx::HullHitHeavy,
    Sfx::MiningHit,
    Sfx::Scan,
    Sfx::Dock,
    Sfx::Undock,
    Sfx::Lock,
    Sfx::Confirm,
    Sfx::Error,
    Sfx::Alarm,
];

fn sfx_path(name: &str) -> String {
    format!("sfx/{name}.ogg")
}

fn build_bank(mut commands: Commands, mut sources: ResMut<Assets<AudioSource>>, server: Res<AssetServer>) {
    let dir = crate::ships::assets_dir();
    let mut recorded = HashMap::new();
    let mut missing = 0;
    for k in ALL_KINDS {
        let mut layers = vec![];
        for l in k.layers() {
            let variants: Vec<Handle<AudioSource>> = l.files.iter().filter(|f| dir.join(sfx_path(f)).exists()).map(|f| server.load(sfx_path(f))).collect();
            missing += l.files.len() - variants.len();
            if !variants.is_empty() {
                layers.push(Layer { variants, gain: l.gain, speed: l.speed });
            }
        }
        if !layers.is_empty() {
            recorded.insert(k, layers);
        }
    }
    if missing > 0 {
        warn!("{missing} sound files missing under assets/sfx — using synthesized fallbacks");
    }
    let synth = ALL.iter().map(|k| (*k, sources.add(AudioSource { bytes: wav(&synth(*k)).into() }))).collect();
    commands.insert_resource(SfxBank { recorded, synth });
}

#[allow(clippy::too_many_arguments)]
fn play_queue(mut commands: Commands, mut q: ResMut<SfxQueue>, bank: Option<Res<SfxBank>>, cam: Query<&GlobalTransform, With<MainCamera>>, mut counter: Local<u32>, time: Res<Time<Real>>, mut log: Local<(f64, HashMap<Sfx, u32>)>) {
    let _prof = shared::prof::span("audio::play_queue");
    let Some(bank) = bank else { return };
    // Harness: STARFALL_SFX_LOG=1 prints how often each cue actually played, every 10 s (text, not ears).
    let logging = std::env::var_os("STARFALL_SFX_LOG").is_some();
    if logging && time.elapsed_secs_f64() - log.0 > 10.0 {
        let mut v: Vec<_> = log.1.drain().collect();
        v.sort_by_key(|(k, _)| format!("{k:?}"));
        info!("sfx played (10 s): {}", v.iter().map(|(k, n)| format!("{k:?}={n}")).collect::<Vec<_>>().join(" "));
        log.0 = time.elapsed_secs_f64();
    }
    let listener = cam.single().map(|c| c.translation()).unwrap_or_default();
    let mut per_kind: HashMap<Sfx, u32> = HashMap::new();
    for (kind, at) in q.0.drain(..) {
        // Don't let 20 turrets play 20 overlapping sounds in one frame.
        let n = per_kind.entry(kind).or_default();
        *n += 1;
        if *n > 3 {
            continue;
        }
        let (base, reach) = kind.reach();
        let gain = match at {
            Some(p) => (1.0 - p.distance(listener) / reach).clamp(0.0, 1.0).powf(1.6),
            None => 1.0,
        };
        if gain < 0.02 {
            continue;
        }
        *counter = counter.wrapping_add(1);
        let c = *counter;
        if logging {
            *log.1.entry(kind).or_default() += 1;
        }
        match bank.recorded.get(&kind) {
            Some(layers) => {
                for (li, l) in layers.iter().enumerate() {
                    let h = l.variants[(c as usize + li) % l.variants.len()].clone();
                    // Deterministic spread over the pitch range (no RNG needed).
                    let k = ((c.wrapping_mul(2654435761) >> 16) % 1000) as f32 / 999.0;
                    let speed = l.speed.0 + (l.speed.1 - l.speed.0) * k;
                    commands.spawn((AudioPlayer(h), PlaybackSettings::DESPAWN.with_volume(Volume::Linear(base * gain * l.gain)).with_speed(speed)));
                }
            }
            None => {
                if let Some(h) = bank.synth.get(&kind) {
                    let pitch = 0.92 + (c % 7) as f32 * 0.025;
                    commands.spawn((AudioPlayer(h.clone()), PlaybackSettings::DESPAWN.with_volume(Volume::Linear(base * gain)).with_speed(pitch)));
                }
            }
        }
    }
}

#[derive(Default)]
struct Watch {
    hull: HashMap<EntityId, f32>,
    docked: Option<bool>,
    target: Option<EntityId>,
    scanning: bool,
    alarm_at: f64,
    mining_at: f64,
    confirm_at: f64,
    last_error: f64,
    last_toast: f64,
    epoch: u32,
    impact_at: f64,
    own_impact_at: f64,
}

/// Cues derived from state changes: hull impacts (any ship near the camera whose hull dropped),
/// dock/undock, target lock, scan start, mining, low-hull alarm, errors and notices.
fn event_sounds(s: Res<Session>, time: Res<Time<Real>>, cam: Query<&GlobalTransform, With<MainCamera>>, mut q: ResMut<SfxQueue>, mut w: Local<Watch>) {
    let _prof = shared::prof::span("audio::event_sounds");
    let now = time.elapsed_secs_f64();
    let listener = cam.single().map(|c| c.translation()).unwrap_or_default();
    if let Some(latest) = s.latest() {
        let mut seen = HashMap::with_capacity(w.hull.len());
        for e in latest.ents.values() {
            if !matches!(e.kind, EntityKind::Player | EntityKind::Npc(_) | EntityKind::Escort) {
                continue;
            }
            let p = Vec3::from(e.pos);
            // Every hit now bleeds into the hull, so impacts are throttled to keep a brawl readable:
            // other ships share one cue per 0.15 s, our own ship has its own budget (one per 0.2 s).
            let mine = Some(e.id) == s.me;
            let free = if mine { now - w.own_impact_at > 0.2 } else { now - w.impact_at > 0.15 };
            if let Some(prev) = w.hull.get(&e.id)
                && e.hull < prev - 1e-4
                && p.distance(listener) < 6000.0
                && free
            {
                let heavy = prev - e.hull > 0.02 || e.class == Some(SizeClass::Capital);
                q.play(if heavy { Sfx::HullHitHeavy } else { Sfx::HullHit }, Some(p));
                if mine {
                    w.own_impact_at = now;
                } else {
                    w.impact_at = now;
                }
            }
            seen.insert(e.id, e.hull);
        }
        w.hull = seen;
    }
    // Arriving in a new system: our own jump (the warp effect needs the ship to be visible already).
    if s.system_epoch != w.epoch {
        if w.epoch != 0 && s.warp.is_none() {
            q.play(Sfx::WarpJump, None);
        }
        w.epoch = s.system_epoch;
        w.hull.clear();
    }
    let me = s.my_state();
    let docked = me.map(|m| m.docked);
    if let (Some(prev), Some(d)) = (w.docked, docked)
        && prev != d
    {
        q.play(if d { Sfx::Dock } else { Sfx::Undock }, None);
    }
    w.docked = docked;
    if s.target != w.target && s.target.is_some() {
        q.play(Sfx::Lock, None);
    }
    w.target = s.target;
    let scanning = s.scan.is_some();
    if scanning && !w.scanning {
        q.play(Sfx::Scan, None);
    }
    w.scanning = scanning;
    if let Some(m) = me {
        if m.hull < 0.3 && !m.docked && now - w.alarm_at > 1.6 {
            q.play(Sfx::Alarm, None);
            w.alarm_at = now;
        }
        if m.mining.is_some() && now - w.mining_at > 0.7 {
            q.play(Sfx::MiningHit, None);
            w.mining_at = now;
        }
    }
    if let Some((_, t)) = &s.last_error
        && *t > w.last_error
    {
        q.play(Sfx::Error, None);
        w.last_error = *t;
    }
    if let Some((_, t)) = s.toasts.back()
        && *t > w.last_toast
    {
        w.last_toast = *t;
        // Notices come in bursts (rewards, missions): one chime at most every 1.2 s.
        if now - w.confirm_at > 1.2 && s.last_error.as_ref().is_none_or(|(_, e)| (e - t).abs() > 1e-6) {
            q.play(Sfx::Confirm, None);
            w.confirm_at = now;
        }
    }
}

#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum LoopKind {
    Engine,
    Boost,
    Station,
}

/// Flagship loops: an engine hum sized to the hull class (fighter whine, cruiser drive, capital
/// rumble) whose volume and pitch follow the throttle, a thruster roar while boosting, and the
/// station's machinery while docked. Restarted with the right sample when the ship changes class.
fn ship_loops(mut commands: Commands, s: Res<Session>, server: Res<AssetServer>, time: Res<Time<Real>>, mut sinks: Query<(Entity, &LoopKind, Option<&mut AudioSink>)>, mut class: Local<Option<SizeClass>>) {
    let _prof = shared::prof::span("audio::ship_loops");
    let dir = crate::ships::assets_dir();
    let want = s.profile.as_ref().map(|p| p.ship.size_class());
    if want != *class {
        for (e, ..) in &sinks {
            commands.entity(e).despawn();
        }
        *class = want;
        if let Some(c) = want {
            let engine = match c {
                SizeClass::Small => "engine_small",
                SizeClass::Cruiser => "engine_medium",
                SizeClass::Capital => "engine_large",
            };
            for (kind, file) in [(LoopKind::Engine, engine), (LoopKind::Boost, "boost_loop"), (LoopKind::Station, "station_ambience")] {
                if dir.join(sfx_path(file)).exists() {
                    commands.spawn((kind, AudioPlayer::<AudioSource>(server.load(sfx_path(file))), PlaybackSettings::LOOP.with_volume(Volume::Linear(0.0))));
                }
            }
        }
        return;
    }
    let me = s.my_state();
    let max_speed = s.profile.as_ref().map(|p| p.ship.stats().speed).unwrap_or(120.0).max(1.0);
    let flying = me.is_some_and(|m| !m.docked);
    let throttle = me.map(|m| (Vec3::from(m.vel).length() / max_speed).clamp(0.0, 1.0)).unwrap_or(0.0);
    let boosting = me.is_some_and(|m| m.boosting);
    let docked = me.is_some_and(|m| m.docked);
    let (base_speed, span) = match *class {
        Some(SizeClass::Capital) => (0.8, 0.25),
        Some(SizeClass::Cruiser) => (0.85, 0.3),
        _ => (0.85, 0.4),
    };
    let k = (time.delta_secs() * 3.0).min(1.0);
    for (_, kind, sink) in &mut sinks {
        let Some(mut sink) = sink else { continue };
        let (target, speed) = match kind {
            LoopKind::Engine if flying => (0.06 + 0.2 * throttle, base_speed + span * throttle),
            LoopKind::Boost if flying && boosting => (0.22, 1.0),
            LoopKind::Station if docked => (0.16, 1.0),
            _ => (0.0, 1.0),
        };
        let cur = sink.volume().to_linear();
        let next = cur + (target - cur) * k;
        if (next - cur).abs() > 1e-4 {
            sink.set_volume(Volume::Linear(next.max(0.0)));
        }
        if (sink.speed() - speed).abs() > 0.01 {
            sink.set_speed(speed);
        }
    }
}

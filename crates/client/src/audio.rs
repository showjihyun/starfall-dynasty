//! Procedurally synthesized sound effects (no audio files needed). Every weapon has its own voice:
//! fighter lasers zap, turret batteries thump, the main gun whines up and lands like thunder.

use bevy::audio::{AudioPlayer, AudioSource, PlaybackSettings, Volume};
use bevy::prelude::*;
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
}

impl Sfx {
    /// (base volume, audible distance in world units)
    fn reach(self) -> (f32, f32) {
        match self {
            Sfx::Laser => (0.35, 3500.0),
            Sfx::Pulse => (0.45, 6000.0),
            Sfx::MainBeam => (0.9, 18000.0),
            Sfx::Missile => (0.4, 4000.0),
            Sfx::ExplosionSmall => (0.6, 7000.0),
            Sfx::ExplosionBig => (1.0, 20000.0),
            Sfx::ShieldHit => (0.35, 3000.0),
            Sfx::WarpCharge => (0.7, 1e9),
            Sfx::WarpJump => (0.8, 1e9),
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

#[derive(Resource)]
struct SfxBank(HashMap<Sfx, Handle<AudioSource>>);

pub struct AudioFxPlugin;

impl Plugin for AudioFxPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SfxQueue>().add_systems(Startup, build_bank).add_systems(Update, play_queue);
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
    }
}

fn build_bank(mut commands: Commands, mut sources: ResMut<Assets<AudioSource>>) {
    let kinds = [Sfx::Laser, Sfx::Pulse, Sfx::MainBeam, Sfx::Missile, Sfx::ExplosionSmall, Sfx::ExplosionBig, Sfx::ShieldHit, Sfx::WarpCharge, Sfx::WarpJump];
    let bank = kinds.iter().map(|k| (*k, sources.add(AudioSource { bytes: wav(&synth(*k)).into() }))).collect();
    commands.insert_resource(SfxBank(bank));
}

fn play_queue(mut commands: Commands, mut q: ResMut<SfxQueue>, bank: Option<Res<SfxBank>>, cam: Query<&GlobalTransform, With<Camera3d>>, mut counter: Local<u32>) {
    let Some(bank) = bank else { return };
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
            Some(p) => {
                let d = p.distance(listener);
                (1.0 - d / reach).clamp(0.0, 1.0).powf(1.6)
            }
            None => 1.0,
        };
        if gain < 0.02 {
            continue;
        }
        *counter = counter.wrapping_add(1);
        let pitch = 0.92 + (*counter % 7) as f32 * 0.025;
        commands.spawn((AudioPlayer(bank.0[&kind].clone()), PlaybackSettings::DESPAWN.with_volume(Volume::Linear(base * gain)).with_speed(pitch)));
    }
}

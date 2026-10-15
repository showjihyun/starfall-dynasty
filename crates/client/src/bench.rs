//! `client --bench [seconds]`: offline stress scene for rendering performance.
//! Two fleets (capitals, cruisers, fighters, pirates) circle each other while turrets,
//! main guns, lasers and explosions fire continuously. Prints frame-time stats and exits.
//! `STARFALL_BENCH_SCALE=N` multiplies the fleets (N=15 → ~1000 ships) for mass-battle tests.

use crate::state::{Screen, Session, Snap};
use bevy::prelude::*;
use shared::missions::NpcArchetype;
use shared::protocol::{EntityKind, EntityState, Fx};
use shared::ships::{HullClass, Module, Ship, WeaponKind};
use std::collections::HashMap;

pub struct BenchPlugin {
    pub seconds: f32,
}

#[derive(Resource)]
struct BenchCfg {
    seconds: f32,
    warmup: f32,
    frames: Vec<f32>,
    start: Vec<(u32, Vec3, f32)>,
}

impl Plugin for BenchPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(BenchCfg { seconds: self.seconds, warmup: 6.0, frames: vec![], start: vec![] })
            .add_systems(Startup, setup)
            .add_systems(Update, (animate, measure));
    }
}

fn setup(mut s: ResMut<Session>, mut cfg: ResMut<BenchCfg>, mut next: ResMut<NextState<Screen>>) {
    let sys = 7;
    let lay = shared::galaxy::system_layout(sys);
    s.system = sys;
    s.system_epoch = 1;
    let center = lay.station + Vec3::new(0.0, 300.0, 2500.0);
    let mut ents = HashMap::new();
    let mut id = 1000u32;
    let mut add = |ents: &mut HashMap<u32, EntityState>, kind: EntityKind, hull: Option<HullClass>, lvl: u8, pos: Vec3, livery: u8, cfg: &mut BenchCfg| {
        let (rank, class, radius) = match hull {
            Some(h) => {
                let mut ship = Ship::new(String::new(), h, livery);
                for m in Module::ALL {
                    ship.modules.insert(m, lvl);
                }
                (ship.rank(), Some(ship.size_class()), ship.stats().radius)
            }
            None => (2, None, 12.0),
        };
        id += 1;
        let phase = id as f32 * 0.37;
        cfg.start.push((id, pos, phase));
        ents.insert(id, EntityState { id, kind, name: format!("B{id}"), hull_class: hull, rank, livery, pos: pos.to_array(), rot: [0.0, 0.0, 0.0, 1.0], vel: [0.0, 0.0, -60.0], hull: 0.6 + (phase.sin() * 0.4).abs(), shield: 0.7, radius, fleet: None, wanted: false, boosting: id % 3 == 0, mining: None, docked: false, owner: Some(id as u64), alliance: None, class });
    };
    let scale: usize = std::env::var("STARFALL_BENCH_SCALE").ok().and_then(|v| v.parse().ok()).unwrap_or(1).max(1);
    for layer in 0..scale {
        // Extra layers stack further out and deeper, like a battle line seen end-on.
        let off = Vec3::new(0.0, (layer as f32 * 0.7).sin() * 900.0 * layer.min(1) as f32, -(layer as f32) * 2600.0);
        let spread = 1.0 + layer as f32 * 0.25;
        for side in [-1.0f32, 1.0] {
            for i in 0..4 {
                add(&mut ents, EntityKind::Player, Some(HullClass::ALL[i % 5]), 9, center + off + Vec3::new(side * 1400.0 * spread, (i as f32 - 1.5) * 220.0, i as f32 * 500.0 - 800.0), if side < 0.0 { 1 } else { 4 }, &mut cfg);
            }
            for i in 0..8 {
                add(&mut ents, EntityKind::Player, Some(HullClass::ALL[(i + 2) % 5]), 5, center + off + Vec3::new(side * 900.0 * spread, (i as f32 - 4.0) * 90.0, i as f32 * 220.0 - 900.0), if side < 0.0 { 1 } else { 4 }, &mut cfg);
            }
            for i in 0..12 {
                add(&mut ents, EntityKind::Escort, Some(HullClass::ALL[i % 5]), 1, center + off + Vec3::new(side * 500.0 * spread + (i as f32 * 1.7).sin() * 120.0, (i as f32 - 6.0) * 40.0, i as f32 * 120.0 - 700.0), if side < 0.0 { 1 } else { 4 }, &mut cfg);
            }
        }
        for i in 0..20 {
            let arch = [NpcArchetype::Raider, NpcArchetype::Gunship, NpcArchetype::Warden][i % 3];
            add(&mut ents, EntityKind::Npc(arch), None, 1, center + off + Vec3::new((i as f32 * 2.3).cos() * 300.0, 400.0 + (i as f32) * 15.0, (i as f32 * 2.3).sin() * 300.0), 7, &mut cfg);
        }
    }
    s.me = Some(1001);
    s.snaps.push_back(Snap { at: 0.0, ents });
    next.set(Screen::Playing);
    info!("bench: {} ships", cfg.start.len());
}

fn animate(time: Res<Time<Real>>, mut s: ResMut<Session>, cfg: Res<BenchCfg>, mut acc: Local<f32>, mut k: Local<u32>) {
    let t = time.elapsed_secs();
    let ids: Vec<u32> = cfg.start.iter().map(|x| x.0).collect();
    if let Some(snap) = s.snaps.back_mut() {
        snap.at = time.elapsed_secs_f64();
        for (id, p0, phase) in &cfg.start {
            if let Some(e) = snap.ents.get_mut(id) {
                // Slow weave so every ship moves and rotates each frame.
                let a = t * 0.12 + phase;
                let p = *p0 + Vec3::new(a.sin() * 120.0, (a * 0.7).sin() * 40.0, (a * 0.5).cos() * 200.0);
                let dir = Vec3::new(a.cos() * 120.0, (a * 0.7).cos() * 28.0, -(a * 0.5).sin() * 100.0).normalize_or(Vec3::NEG_Z);
                e.pos = p.to_array();
                e.rot = Quat::from_rotation_arc(Vec3::NEG_Z, dir).to_array();
                e.vel = (dir * 80.0).to_array();
            }
        }
    }
    // Fire at roughly the rate the server produces in a big battle (STARFALL_BENCH_NOFX=1 to isolate).
    if std::env::var("STARFALL_BENCH_NOFX").is_ok() {
        return;
    }
    *acc += time.delta_secs();
    while *acc > 0.05 {
        *acc -= 0.05;
        *k += 1;
        let n = ids.len() as u32;
        let pick = |x: u32| ids[(x % n) as usize];
        for c in 0..8u32 {
            let from = pick(c * 3 + *k);
            let to = pick(c * 7 + *k * 5 + 11);
            let (weapon, mount) = match (c + *k) % 6 {
                0 => (WeaponKind::Laser, 0),
                5 if *k % 40 == c => (WeaponKind::MainBeam, 255),
                _ => (WeaponKind::Pulse, (c + *k) as u8),
            };
            s.fx.push(Fx::Shot { from, to, weapon, mount, hit: true });
        }
        if *k % 30 == 0 {
            let p = Vec3::from(s.snaps.back().unwrap().ents[&pick(*k)].pos);
            s.fx.push(Fx::Explosion { pos: p.to_array(), size: 2.0 + (*k % 3) as f32 });
        }
        if *k % 7 == 0 {
            s.fx.push(Fx::ShieldHit { entity: pick(*k * 3), dir: [0.0, 0.0, 1.0] });
        }
    }
}

fn measure(time: Res<Time<Real>>, mut cfg: ResMut<BenchCfg>, mut exit: MessageWriter<AppExit>, ents: Query<(), With<Mesh3d>>, mut done: Local<bool>) {
    let t = time.elapsed_secs();
    if t < cfg.warmup || *done {
        return;
    }
    cfg.frames.push(time.delta_secs() * 1000.0);
    if t > cfg.warmup + cfg.seconds {
        *done = true;
        let mut f = cfg.frames.clone();
        f.sort_by(|a, b| a.total_cmp(b));
        let avg = f.iter().sum::<f32>() / f.len() as f32;
        let p99 = f[(f.len() * 99 / 100).min(f.len() - 1)];
        let worst = *f.last().unwrap();
        println!("BENCH frames {} | avg {:.2} ms ({:.0} fps) | 1% low {:.2} ms ({:.0} fps) | worst {:.2} ms | meshes {}", f.len(), avg, 1000.0 / avg, p99, 1000.0 / p99, worst, ents.iter().count());
        exit.write(AppExit::Success);
    }
}

//! STARFALL DYNASTY client (Bevy 0.20).
//!
//! `client [--server ws://127.0.0.1:7777] [--name NAME] [--auto] [--hull talon]`
//! `       [--shot out.png --shot-at 8] [--showroom [system]] [--script "undock;thrust 3;fire 2"]`

mod control;
mod fx;
mod materials;
mod meshes;
mod net;
mod scene;
mod ships;
mod state;
mod ui;

use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use bevy_egui::{EguiPlugin, EguiPrimaryContextPass};
use shared::protocol::*;
use state::{Args, Screen, Session};

fn parse_args() -> Args {
    let a: Vec<String> = std::env::args().collect();
    let get = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
    let has = |k: &str| a.iter().any(|x| x == k);
    let hull = get("--hull").and_then(|h| shared::ships::HullClass::ALL.into_iter().find(|c| format!("{c:?}").eq_ignore_ascii_case(&h)));
    Args {
        server: get("--server").unwrap_or_else(|| format!("ws://127.0.0.1:{DEFAULT_PORT}")),
        name: get("--name"),
        auto: has("--auto"),
        hull,
        shot: get("--shot"),
        shot_at: get("--shot-at").and_then(|v| v.parse().ok()).unwrap_or(8.0),
        showroom: has("--showroom"),
        script: get("--script").map(|s| s.split(';').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect()).unwrap_or_default(),
    }
}

fn main() {
    let args = parse_args();
    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(Window { title: "STARFALL DYNASTY".into(), resolution: (1600, 900).into(), ..default() }),
                ..default()
            })
            .set(AssetPlugin { file_path: ships::assets_dir().to_string_lossy().into_owned(), ..default() }),
    )
    .add_plugins(EguiPlugin::default())
    .add_plugins((materials::MaterialsPlugin, scene::ScenePlugin, ships::ShipsPlugin, fx::FxPlugin, control::ControlPlugin))
    .init_state::<Screen>()
    .insert_resource(Session { server_url: args.server.clone(), ..default() })
    .init_resource::<ui::Form>()
    .insert_resource(args.clone())
    .add_systems(Startup, startup)
    .add_systems(PreUpdate, state::pump_network)
    .add_systems(EguiPrimaryContextPass, ui::ui_system)
    .add_systems(Update, (automation, ping));
    app.run();
}

fn startup(mut commands: Commands, args: Res<Args>, mut s: ResMut<Session>, mut next: ResMut<NextState<Screen>>) {
    if args.showroom {
        showroom(&mut s, &args);
        next.set(Screen::Playing);
        return;
    }
    commands.insert_resource(net::connect(args.server.clone()));
}

fn ping(net: Option<Res<net::Net>>, time: Res<Time<Real>>, mut last: Local<f64>) {
    let now = time.elapsed_secs_f64();
    if let Some(n) = net
        && now - *last > 2.0
    {
        *last = now;
        n.send(ClientMsg::Ping { t: now });
    }
}

/// Offline scene for checking models, shaders and effects without a server.
fn showroom(s: &mut Session, args: &Args) {
    use shared::ships::{HullClass, Ship};
    let sys: u16 = std::env::args().skip_while(|a| a != "--showroom").nth(1).and_then(|v| v.parse().ok()).unwrap_or(0);
    let layout = shared::galaxy::system_layout(sys);
    s.system = sys;
    s.system_epoch = 1;
    let base = if args.hull.is_some() { layout.station + Vec3::new(0.0, 60.0, 420.0) } else { layout.station + Vec3::new(0.0, 170.0, 120.0) };
    let mut ents = std::collections::HashMap::new();
    let face = Quat::from_rotation_y(0.35);
    for (i, h) in HullClass::ALL.into_iter().enumerate() {
        let mut ship = Ship::new(format!("{h:?}"), h, (i % 7) as u8);
        if i % 2 == 1 {
            for m in shared::ships::Module::ALL {
                ship.modules.insert(m, 8);
            }
        }
        let st = ship.stats();
        let id = 100 + i as u32;
        let pos = base + Vec3::new((i as f32 - 2.0) * 70.0, (i as f32 * 1.3).sin() * 12.0, -(i as f32 % 2.0) * 50.0 - 120.0);
        ents.insert(
            id,
            EntityState {
                id,
                kind: EntityKind::Player,
                name: format!("{h:?}"),
                hull_class: Some(h),
                rank: ship.rank(),
                livery: (i % 7) as u8,
                pos: pos.to_array(),
                rot: face.to_array(),
                vel: (face * Vec3::NEG_Z * 40.0).to_array(),
                hull: if i == 4 { 0.3 } else { 1.0 },
                shield: 1.0,
                radius: st.radius,
                fleet: None,
                wanted: false,
                boosting: i == 2,
                mining: None,
                docked: false,
                owner: None,
            },
        );
    }
    // A pirate for scale and hostile livery.
    ents.insert(
        200,
        EntityState { id: 200, kind: EntityKind::Npc(shared::missions::NpcArchetype::Raider), name: "Raider".into(), hull_class: None, rank: 1, livery: 7, pos: (base + Vec3::new(40.0, 40.0, -320.0)).to_array(), rot: Quat::from_rotation_y(2.5).to_array(), vel: [0.0; 3], hull: 0.7, shield: 0.5, radius: 9.0, fleet: None, wanted: false, boosting: true, mining: None, docked: false, owner: None },
    );
    s.me = args.hull.map(|hc| 100 + shared::ships::HullClass::ALL.iter().position(|h| *h == hc).unwrap() as u32);
    s.profile = None;
    s.snaps.push_back(state::Snap { at: 0.0, ents });
}

/// Scripted input + screenshots for automated visual checks.
#[allow(clippy::too_many_arguments)]
fn automation(
    mut commands: Commands,
    args: Res<Args>,
    time: Res<Time<Real>>,
    mut exit: MessageWriter<AppExit>,
    mut done: Local<bool>,
    mut step: Local<usize>,
    mut step_t: Local<f64>,
    mut fired: Local<Option<usize>>,
    net: Option<Res<net::Net>>,
    mut s: ResMut<Session>,
    screen: Res<State<Screen>>,
) {
    let t = time.elapsed_secs_f64();
    if args.showroom
        && let Some(snap) = s.snaps.back_mut()
    {
        // Keep the showroom alive: refresh timestamps and fire demo effects.
        snap.at = t;
        if (t * 2.0) as u64 % 3 == 0 && ((t - 0.016) * 2.0) as u64 % 3 != 0 {
            s.fx.push(Fx::Laser { from: 200, to: 101, hit: true });
            s.fx.push(Fx::ShieldHit { entity: 101, dir: [0.3, 0.2, -1.0] });
            s.fx.push(Fx::Laser { from: 102, to: 200, hit: true });
        }
        if (t as u64) % 5 == 2 && ((t - 0.016) as u64) % 5 != 2 {
            let p = Vec3::from(s.snaps.back().unwrap().ents[&200].pos) + Vec3::new(-60.0, -10.0, -80.0);
            s.fx.push(Fx::Explosion { pos: p.to_array(), size: 2.0 });
        }
    }
    // Script runs once we're in the game.
    if *screen.get() == Screen::Playing
        && let Some(net) = &net
        && *step < args.script.len()
    {
        if *step_t == 0.0 {
            *step_t = t;
        }
        let cmd: Vec<&str> = args.script[*step].split_whitespace().collect();
        let dur = cmd.get(1).and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0);
        let first = *fired != Some(*step);
        *fired = Some(*step);
        match cmd.first().copied().unwrap_or("") {
            "undock" if first => net.send(ClientMsg::Undock),
            "warp" if first => net.send(ClientMsg::Warp { system: dur as u16 }),
            "thrust" => net.send(ClientMsg::Controls(Controls { thrust: 1.0, ..default() })),
            "boost" => net.send(ClientMsg::Controls(Controls { thrust: 1.0, boost: true, ..default() })),
            "fire" => net.send(ClientMsg::Controls(Controls { fire: true, thrust: 0.3, ..default() })),
            // Hunt the nearest NPC: steer at it, close in, fire when aligned.
            "attack" => {
                if let (Some(me), Some(latest)) = (s.my_state().cloned(), s.latest()) {
                    let mp = Vec3::from(me.pos);
                    let tgt = latest.ents.values().filter(|e| matches!(e.kind, EntityKind::Npc(_))).min_by(|a, b| Vec3::from(a.pos).distance(mp).total_cmp(&Vec3::from(b.pos).distance(mp))).map(|e| (e.id, Vec3::from(e.pos)));
                    if let Some((id, tp)) = tgt {
                        if s.target != Some(id) {
                            s.target = Some(id);
                            net.send(ClientMsg::SetTarget(Some(id)));
                        }
                        let rot = Quat::from_array(me.rot);
                        let local = rot.inverse() * (tp - mp).normalize_or_zero();
                        let mut c = Controls { yaw: (local.x * 4.0).clamp(-1.0, 1.0), pitch: (local.y * 4.0).clamp(-1.0, 1.0), ..default() };
                        if local.z > 0.0 {
                            c.yaw = if local.x >= 0.0 { 1.0 } else { -1.0 };
                        }
                        let d = tp.distance(mp);
                        c.thrust = if d > 350.0 { 1.0 } else { 0.1 };
                        c.boost = d > 1500.0;
                        c.fire = local.z < -0.97 && d < 600.0;
                        net.send(ClientMsg::Controls(c));
                        if c.fire && first {
                            net.send(ClientMsg::FireMissile);
                        }
                    }
                }
            }
            "turn" => net.send(ClientMsg::Controls(Controls { yaw: 1.0, thrust: 0.5, ..default() })),
            "map" if first => s.ui.map = true,
            "station" if first => s.ui.station = true,
            "missions" if first => s.ui.missions = true,
            "chronicle" if first => {
                s.ui.chronicle = true;
                net.send(ClientMsg::RequestChronicle);
            }
            "close" if first => s.ui = Default::default(),
            "scenario" if first => net.send(ClientMsg::StartScenario { id: dur as u32 }),
            _ => {}
        }
        let wait = if matches!(cmd.first().copied(), Some("warp")) { 6.0 } else { dur.max(0.1) };
        if t - *step_t >= wait {
            *step += 1;
            *step_t = t;
            net.send(ClientMsg::Controls(Controls::default()));
        }
    }
    if let Some(path) = &args.shot
        && !*done
        && t > args.shot_at as f64
    {
        *done = true;
        commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path.clone()));
    }
    if *done && t > args.shot_at as f64 + 2.0 {
        exit.write(AppExit::Success);
    }
}

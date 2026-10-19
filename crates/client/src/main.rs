//! STARFALL DYNASTY client (Bevy 0.20).
//!
//! `client [--server ws://127.0.0.1:7777] [--name NAME] [--auto] [--hull talon]`
//! `       [--shot out.png --shot-at 8] [--showroom [system]] [--script "undock;thrust 3;fire 2"]`
//! script steps: undock, warp N, thrust/boost/fire/turn/roll SECS, attack SECS, target NAME, say TEXT
//! (dev commands too: `say /grow 9`), map, galaxy, station, missions, alliance, contracts, chronicle, close, fp, tp, zoom N
//! `client --replay DIR [--as NAME] [--from SECS] [--speed X]` — 3D replay of a recorded session

mod audio;
mod bench;
mod clicks;
mod control;
mod dust;
mod fx;
mod interior;
mod materials;
mod meshes;
mod music;
mod net;
#[cfg(feature = "raytracing")]
mod raytracing;
mod replay;
mod scene;
mod ships;
mod starmap;
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
        look: get("--look"),
        replay: get("--replay"),
        no_create: has("--no-create"),
        token: get("--token"),
        script: get("--script").map(|s| s.split(';').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect()).unwrap_or_default(),
    }
}

fn main() {
    if let Some(i) = std::env::args().position(|a| a == "--dump-sfx") {
        let dir = std::env::args().nth(i + 1).unwrap_or_else(|| "sfx".into());
        audio::dump(std::path::Path::new(&dir)).expect("dump sfx");
        println!("wrote sound effects to {dir}");
        return;
    }
    let args = parse_args();
    shared::prof::enable_from_env();
    let bench_secs: Option<f32> = std::env::args().position(|a| a == "--bench").map(|i| std::env::args().nth(i + 1).and_then(|v| v.parse().ok()).unwrap_or(20.0));
    let mut app = App::new();
    #[cfg(feature = "raytracing_dlss")]
    app.insert_resource(raytracing::dlss_project_id());
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "STARFALL DYNASTY".into(),
                    resolution: std::env::var("STARFALL_RES").ok().and_then(|r| r.split_once('x').and_then(|(w, h)| Some((w.parse::<u32>().ok()?, h.parse::<u32>().ok()?)))).unwrap_or((1600, 900)).into(),
                    // Benchmarks measure uncapped frame times.
                    present_mode: if bench_secs.is_some() || std::env::var("STARFALL_NOVSYNC").is_ok() { bevy::window::PresentMode::AutoNoVsync } else { bevy::window::PresentMode::AutoVsync },
                    ..default()
                }),
                ..default()
            })
            .set(AssetPlugin { file_path: ships::assets_dir().to_string_lossy().into_owned(), ..default() }),
    )
    .add_plugins(EguiPlugin::default())
    .insert_resource(bevy_egui::EguiGlobalSettings { auto_create_primary_context: false, ..default() })
    .add_plugins((materials::MaterialsPlugin, scene::ScenePlugin, ships::ShipsPlugin, fx::FxPlugin, control::ControlPlugin, audio::AudioFxPlugin, music::MusicPlugin, interior::InteriorPlugin, dust::DustPlugin))
    .init_state::<Screen>()
    .insert_resource(Session { server_url: args.server.clone(), last_combat: -1e9, ..default() })
    .init_resource::<ui::Form>()
    .insert_resource(args.clone())
    .add_systems(Startup, startup)
    .add_systems(PreUpdate, (replay::drive.run_if(resource_exists::<replay::Replay>), state::pump_network).chain())
    .add_systems(EguiPrimaryContextPass, ui::ui_system)
    .add_systems(Update, (automation, ping));
    if let Some(spec) = std::env::args().skip_while(|a| a != "--clicks").nth(1) {
        app.insert_resource(clicks::parse(&spec)).add_systems(PreUpdate, clicks::inject.before(bevy_egui::EguiPreUpdateSet::ProcessInput));
    }
    // Harness (docs/HARNESS.md): STARFALL_FIXED_FPS=60 advances time by exactly 1/60 s per frame, so
    // `--shot-at 8` is always the same frame and before/after screenshots compare pixel for pixel.
    if let Some(fps) = std::env::var("STARFALL_FIXED_FPS").ok().and_then(|v| v.parse::<f64>().ok()).filter(|f| *f > 0.0) {
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f64(1.0 / fps)));
    }
    if let Some(secs) = bench_secs {
        app.add_plugins(bench::BenchPlugin { seconds: secs });
    }
    // Experimental ray-traced lighting (src/raytracing.rs): `--features raytracing` and STARFALL_RT=1.
    #[cfg(feature = "raytracing")]
    if raytracing::wanted() {
        app.add_plugins(raytracing::RaytracingPlugin);
    }
    app.run();
}

fn startup(mut commands: Commands, args: Res<Args>, mut s: ResMut<Session>, mut next: ResMut<NextState<Screen>>) {
    if std::env::args().any(|a| a == "--bench") {
        return;
    }
    if args.showroom {
        showroom(&mut s, &args);
        next.set(Screen::Playing);
        return;
    }
    if let Some(dir) = &args.replay {
        let (rp, net) = replay::open(dir);
        commands.insert_resource(rp);
        commands.insert_resource(net);
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
/// The showroom's entity id for a hull (one ship per hull class, in `HullClass::ALL` order).
pub(crate) fn showroom_id(h: shared::ships::HullClass) -> u32 {
    100 + shared::ships::HullClass::ALL.iter().position(|x| *x == h).unwrap_or(0) as u32
}

fn showroom(s: &mut Session, args: &Args) {
    use shared::ships::{HullClass, Ship};
    let sys: u16 = std::env::args().skip_while(|a| a != "--showroom").nth(1).and_then(|v| v.parse().ok()).unwrap_or(0);
    let layout = shared::galaxy::system_layout(sys);
    s.system = sys;
    s.system_epoch = 1;
    let base = if args.hull.is_some() { layout.station + Vec3::new(0.0, 60.0, 420.0) } else { layout.station + Vec3::new(0.0, 170.0, 120.0) };
    let mut ents = std::collections::HashMap::new();
    let face = Quat::from_rotation_y(0.35);
    // `--species human|veyr|kharz` dresses the whole showroom fleet in one species' style.
    let only_species = std::env::args().skip_while(|a| a != "--species").nth(1).and_then(|s| shared::galaxy::Species::ALL.into_iter().find(|x| format!("{x:?}").eq_ignore_ascii_case(&s)));
    // `--class cruiser|capital` (with `--hull`): only that hull, grown to the class (art checks of the big models).
    let only_class = std::env::args().skip_while(|a| a != "--class").nth(1).and_then(|s| [shared::ships::SizeClass::Cruiser, shared::ships::SizeClass::Capital].into_iter().find(|x| format!("{x:?}").eq_ignore_ascii_case(&s)));
    for (i, h) in HullClass::ALL.into_iter().enumerate() {
        if only_class.is_some() && args.hull != Some(h) {
            continue;
        }
        let mut ship = Ship::new(format!("{h:?}"), h, (i % 7) as u8);
        if let Some(class) = only_class {
            'grow: for _ in 0..20 {
                for m in shared::ships::Module::ALL {
                    if ship.size_class() == class {
                        break 'grow;
                    }
                    *ship.modules.entry(m).or_insert(1) += 1;
                }
            }
        } else if i % 2 == 1 && only_species.is_none() {
            // (species scenes keep all five small: their kits are the subject; big hulls have `--class`)
            for m in shared::ships::Module::ALL {
                ship.modules.insert(m, 8);
            }
        }
        let st = ship.stats();
        let id = showroom_id(h);
        let pos = if only_class.is_some() {
            layout.station + Vec3::new(0.0, 500.0, 2600.0)
        } else {
            base + Vec3::new((i as f32 - 2.0) * 70.0, (i as f32 * 1.3).sin() * 12.0, -(i as f32 % 2.0) * 50.0 - 120.0)
        };
        ents.insert(
            id,
            EntityState {
                id,
                kind: EntityKind::Player,
                name: format!("{h:?}"),
                hull_class: Some(h),
                rank: ship.rank(),
                livery: only_species.map(view::palette::species_livery).unwrap_or((i % 7) as u8),
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
                alliance: None,
                class: Some(ship.size_class()),
                // Every species' style on show (docs/ART_DIRECTION.md §4).
                species: Some(only_species.unwrap_or(shared::galaxy::Species::ALL[i % 3])),
            },
        );
    }
    // A pirate for scale and hostile livery.
    ents.insert(
        200,
        EntityState { id: 200, kind: EntityKind::Npc(shared::missions::NpcArchetype::Raider), name: "Raider".into(), hull_class: None, rank: 1, livery: 7, pos: (base + Vec3::new(40.0, 40.0, -320.0)).to_array(), rot: Quat::from_rotation_y(2.5).to_array(), vel: [0.0; 3], hull: 0.7, shield: 0.5, radius: 9.0, fleet: None, wanted: false, boosting: true, mining: None, docked: false, owner: None, alliance: None, class: None, species: None },
    );
    s.me = if args.look.is_some() { None } else { args.hull }.map(showroom_id);
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
    mut cam: ResMut<control::CamState>,
    mut prev: Local<f64>,
) {
    let _prof = shared::prof::span("main::automation");
    let t = time.elapsed_secs_f64();
    // Showroom demo beats fire on the frame that crosses them, whatever the frame length.
    let crossed = |every: f64, phase: f64| ((t - phase) / every).floor() > ((*prev - phase) / every).floor();
    if args.showroom
        && let Some(snap) = s.snaps.back_mut()
    {
        // Keep the showroom alive: refresh timestamps and fire demo effects.
        snap.at = t;
        if crossed(1.5, 0.0) {
            use shared::ships::WeaponKind;
            s.fx.push(Fx::Shot { from: 200, to: 101, weapon: WeaponKind::Laser, mount: 0, hit: true });
            s.fx.push(Fx::ShieldHit { entity: 101, dir: [0.3, 0.2, -1.0] });
            for m in 0..6 {
                s.fx.push(Fx::Shot { from: 103, to: 200, weapon: WeaponKind::Pulse, mount: m, hit: true });
            }
            s.fx.push(Fx::Shot { from: 101, to: 200, weapon: WeaponKind::MainBeam, mount: 255, hit: true });
        }
        if crossed(5.0, 2.0) {
            let p = Vec3::from(s.snaps.back().unwrap().ents[&200].pos) + Vec3::new(-60.0, -10.0, -80.0);
            s.fx.push(Fx::Explosion { pos: p.to_array(), size: 2.0 });
        }
    }
    *prev = t;
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
            "roll" => net.send(ClientMsg::Controls(Controls { roll: 1.0, ..default() })),
            // `target NAME`: select the ship whose name contains NAME (the target panel's subject).
            "target" => {
                let want = cmd.get(1).copied().unwrap_or("");
                let id = s.latest().and_then(|l| l.ents.values().filter(|e| Some(e.id) != s.me && e.name.contains(want)).map(|e| e.id).min());
                if id.is_some() && s.target != id {
                    s.target = id;
                    net.send(ClientMsg::SetTarget(id));
                }
            }
            "map" if first => s.ui.map = true,
            "galaxy" if first => s.map.level = starmap::Level::Galaxy,
            "station" if first => s.ui.station = true,
            "missions" if first => s.ui.missions = true,
            "ally" if first => net.send(ClientMsg::AllianceCreate { name: cmd.get(1).copied().unwrap_or("QA Pact").into(), tag: cmd.get(2).copied().unwrap_or("QA").into() }),
            "alliance" if first => {
                s.ui.alliance = true;
                net.send(ClientMsg::RequestAlliances);
            }
            "contracts" if first => {
                s.ui.contracts = true;
                net.send(ClientMsg::RequestContracts);
            }
            "chronicle" if first => {
                s.ui.chronicle = true;
                net.send(ClientMsg::RequestChronicle);
            }
            "close" if first => s.ui = Default::default(),
            "say" if first => net.send(ClientMsg::Chat { text: args.script[*step].trim_start_matches("say").trim().to_string() }),
            "fps" => info!("FPS {:.1}", s.fps),
            "fp" if first => cam.first_person = true,
            "tp" if first => cam.first_person = false,
            "zoom" if first => cam.zoom = dur as f32,
            "scenario" if first => net.send(ClientMsg::StartScenario { id: dur as u32 }),
            _ => {}
        }
        let wait = match cmd.first().copied() {
            Some("warp") => 6.0,
            Some("say") | Some("fp") | Some("tp") | Some("zoom") => 1.0,
            _ => dur.max(0.1),
        };
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

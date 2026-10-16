//! End-to-end over real sockets: a server process, two captains warp three jumps into
//! a war zone, fight; the battle and destruction show up in the Galactic Chronicle.
//! Plus a 30-captain concurrent load test (the design doc's Phase-0 exit criterion).

use bot::Bot;
use shared::galaxy::Species;
use shared::glam::Vec3;
use shared::protocol::*;
use shared::ships::HullClass;
use std::process::{Child, Command};
use std::time::Duration;

struct Server(Child, std::path::PathBuf);
impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = std::fs::remove_dir_all(&self.1);
    }
}

async fn start_server(port: u16) -> Server {
    // The server binary from the same profile as this test (target/<profile>/deps/e2e-*.exe → target/<profile>/server).
    let dir = std::env::current_exe().unwrap().parent().unwrap().parent().unwrap().to_path_buf();
    let exe = dir.join(if cfg!(windows) { "server.exe" } else { "server" });
    let data = std::env::temp_dir().join(format!("sfd-e2e-{port}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&data);
    let child = Command::new(exe).args(["--port", &port.to_string(), "--data", data.to_str().unwrap()]).spawn().expect("server binary (cargo build -p server first)");
    for _ in 0..50 {
        if tokio::net::TcpStream::connect(("127.0.0.1", port)).await.is_ok() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    Server(child, data)
}

#[tokio::test(flavor = "multi_thread")]
async fn two_captains_battle_over_network() {
    let port = 7811;
    let _srv = start_server(port).await;
    let url = format!("ws://127.0.0.1:{port}");
    let mut a = Bot::connect(&url, "Ares").await.unwrap();
    let mut b = Bot::connect(&url, "Bellona").await.unwrap();
    a.login_and_create(HullClass::Bastion, Species::Human, Profession::CombatPilot).await.unwrap();
    b.login_and_create(HullClass::Talon, Species::Human, Profession::Trader).await.unwrap();

    // Sol Haven → Kharz Forge → Vesta Corridor → Vesta Front (war zone).
    for sys in [2, 4, 7] {
        let (ra, rb) = tokio::join!(a.warp_to(sys), b.warp_to(sys));
        ra.unwrap();
        rb.unwrap();
    }
    // Fleet up? No — they are enemies. Find each other.
    a.pump(0.5).await;
    let bid = b.me.unwrap();
    a.send(ClientMsg::SetTarget(Some(bid))).await;
    let mut killed = false;
    for step in 0..1200 {
        a.pump(0.04).await;
        b.pump(0.01).await;
        if step % 100 == 0 {
            let me = a.my_state().map(|m| (m.pos, m.hull));
            let tb = a.ents.get(&bid).map(|e| (e.pos, e.hull, e.shield));
            println!("step {step}: A {me:?} B {tb:?}");
        }
        if b.dead {
            killed = true;
            break;
        }
        let Some(target) = a.ents.get(&bid).map(|e| Vec3::from(e.pos)) else { continue };
        let mut c = a.aim_controls(target, 0.8, 250.0);
        let aligned = a.my_state().map(|me| {
            let fwd = shared::glam::Quat::from_array(me.rot) * Vec3::NEG_Z;
            fwd.angle_between((target - Vec3::from(me.pos)).normalize_or_zero()) < 0.15
        });
        c.fire = aligned == Some(true);
        a.send(ClientMsg::Controls(c)).await;
        if aligned == Some(true) {
            a.send(ClientMsg::FireMissile).await;
        }
        // B just drifts (a trader caught in the war zone).
    }
    assert!(killed, "Ares should destroy Bellona over the network");
    assert!(b.log.iter().any(|m| matches!(m, ServerMsg::Died { killer, .. } if killer == "Ares")));

    // Wait for the engagement to conclude (20s timeout) and check the chronicle.
    a.send(ClientMsg::Controls(Controls::default())).await;
    let ended = a.until(30.0, |m| matches!(m, ServerMsg::Engagement { active: false, .. })).await;
    assert!(ended.is_some(), "engagement should end");
    a.pump(0.3).await;
    a.send(ClientMsg::RequestChronicle).await;
    let Some(ServerMsg::Chronicle { events, herald, .. }) = a.until(5.0, |m| matches!(m, ServerMsg::Chronicle { .. })).await else { panic!("no chronicle") };
    assert!(events.iter().any(|e| e.kind == "BATTLE"), "battle in chronicle: {:?}", events.iter().map(|e| &e.title).collect::<Vec<_>>());
    assert!(events.iter().any(|e| e.kind == "SHIP_DESTROYED"));
    assert!(!herald.is_empty());

    // Bellona respawns at home, ship intact (one ship for life, rebuilt).
    b.send(ClientMsg::Respawn).await;
    let r = b.until(5.0, |m| matches!(m, ServerMsg::EnterSystem { .. })).await;
    assert!(matches!(r, Some(ServerMsg::EnterSystem { system: 0, .. })));
    assert_eq!(b.profile.as_ref().unwrap().ship.hull, HullClass::Talon);
}

#[tokio::test(flavor = "multi_thread")]
async fn thirty_concurrent_captains() {
    let port = 7812;
    let _srv = start_server(port).await;
    let url = format!("ws://127.0.0.1:{port}");
    let mut handles = vec![];
    for i in 0..30 {
        let url = url.clone();
        handles.push(tokio::spawn(async move {
            let mut bot = Bot::connect(&url, &format!("Load{i:02}")).await.unwrap();
            let hull = HullClass::ALL[i % 5];
            let species = Species::ALL[i % 3];
            bot.login_and_create(hull, species, Profession::Explorer).await.unwrap();
            bot.send(ClientMsg::Undock).await;
            let mut snaps = 0u32;
            for k in 0..60 {
                let c = Controls { thrust: 1.0, yaw: ((k + i) as f32 * 0.3).sin(), fire: k % 5 == 0, ..Default::default() };
                bot.send(ClientMsg::Controls(c)).await;
                if bot.until(0.1, |m| matches!(m, ServerMsg::Snapshot { .. })).await.is_some() {
                    snaps += 1;
                }
            }
            // Under load the last captains can still be logging in when a fast bot finishes its
            // loop: give the home system up to 10 s to fill before counting who we see.
            let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
            while bot.ents.len() < 5 && tokio::time::Instant::now() < deadline {
                bot.pump(0.2).await;
            }
            (snaps, bot.ents.len())
        }));
    }
    let mut total = 0;
    for h in handles {
        let (snaps, seen) = h.await.unwrap();
        assert!(snaps >= 40, "bot starved of snapshots: {snaps}");
        assert!(seen >= 5, "bot sees others in its home system: {seen}");
        total += snaps;
    }
    println!("30 captains, {total} snapshots received");
}

/// A client that dies without closing the WebSocket (TCP reset) must not leave a ghost ship behind.
#[tokio::test(flavor = "multi_thread")]
async fn abrupt_disconnect_leaves_no_ghost() {
    use futures_util::{SinkExt, StreamExt};
    use tokio_tungstenite::tungstenite::Message;
    let port = 7813;
    let _srv = start_server(port).await;
    let url = format!("ws://127.0.0.1:{port}");
    {
        let tcp = tokio::net::TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        #[allow(deprecated)] // zero linger is exactly what we want: drop => RST, no close handshake
        tcp.set_linger(Some(Duration::ZERO)).unwrap();
        let (mut ws, _) = tokio_tungstenite::client_async(url.clone(), tcp).await.unwrap();
        ws.send(Message::text(encode(&ClientMsg::Hello { version: PROTOCOL_VERSION, name: "Ghost".into(), token: None }))).await.unwrap();
        ws.send(Message::text(encode(&ClientMsg::CreateCharacter { species: Species::Human, profession: Profession::Explorer, hull: HullClass::Talon, ship_name: "Phantom".into(), livery: 0 }))).await.unwrap();
        while let Some(Ok(m)) = ws.next().await {
            if let Message::Text(t) = m
                && t.contains("EnterSystem")
            {
                break;
            }
        }
    }
    tokio::time::sleep(Duration::from_millis(500)).await;
    let mut obs = Bot::connect(&url, "Watcher").await.unwrap();
    obs.login_and_create(HullClass::Wren, Species::Human, Profession::Explorer).await.unwrap();
    obs.until(3.0, |m| matches!(m, ServerMsg::Snapshot { .. })).await;
    assert!(obs.ents.values().all(|e| !e.name.contains("Phantom")), "ghost ship left behind: {:?}", obs.ents.values().map(|e| &e.name).collect::<Vec<_>>());
}

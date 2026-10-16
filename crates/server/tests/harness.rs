//! The harness contract (docs/HARNESS.md): a recorded session replays to exactly the same state.
//! Drives the simulation the way the network loop does, records it, then replays it headless.

use server::game::Game;
use server::harness::{Event, Recorder, Session, digest};
use server::store::Store;
use shared::galaxy::Species;
use shared::protocol::*;
use shared::ships::HullClass;

const T0: u64 = 1_800_000_000;

/// A small war-zone brawl: two captains log in, warp to Vesta Front, one reconnects with its token,
/// both fight with lasers and missiles while pirates join in.
fn script(tick: u64) -> Vec<Event> {
    let hello = |c: u64, n: &str, token: Option<String>| Event::Msg(c, ClientMsg::Hello { version: PROTOCOL_VERSION, name: n.into(), token });
    let mut ev = vec![];
    match tick {
        1 => {
            ev.push(hello(1, "Ares", None));
            ev.push(Event::Msg(1, ClientMsg::CreateCharacter { species: Species::Human, profession: Profession::CombatPilot, hull: HullClass::Bastion, ship_name: "Aegis".into(), livery: 1 }));
            ev.push(hello(2, "Bellona", None));
            ev.push(Event::Msg(2, ClientMsg::CreateCharacter { species: Species::Human, profession: Profession::Trader, hull: HullClass::Talon, ship_name: "Bulwark".into(), livery: 2 }));
        }
        2 => {
            ev.push(Event::Msg(1, ClientMsg::Chat { text: "/grow 9".into() }));
            ev.push(Event::Msg(1, ClientMsg::Undock));
            ev.push(Event::Msg(2, ClientMsg::Undock));
        }
        10 | 140 | 270 => {
            let sys = match tick {
                10 => 2,
                140 => 4,
                _ => 7,
            };
            ev.push(Event::Msg(1, ClientMsg::Warp { system: sys }));
            ev.push(Event::Msg(2, ClientMsg::Warp { system: sys }));
        }
        // Bellona drops and comes back on a new connection.
        420 => ev.push(Event::Close(2)),
        430 => {}
        t if t > 400 && t % 7 == 0 => {
            let c = Controls { thrust: 0.8, yaw: ((t as f32) * 0.05).sin(), pitch: 0.1, fire: true, boost: t % 50 < 10, ..Default::default() };
            ev.push(Event::Msg(1, ClientMsg::Controls(c)));
            if t % 70 == 0 {
                ev.push(Event::Msg(1, ClientMsg::FireMissile));
            }
        }
        _ => {}
    }
    ev
}

#[test]
fn recorded_session_replays_exactly() {
    let root = std::env::temp_dir().join(format!("sfd-harness-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (data, rec_dir) = (root.join("data"), root.join("rec"));
    std::fs::create_dir_all(&data).unwrap();

    // "Live" run, exactly like the server loop: events → tick → recorder.
    let mut g = Game::new(Store::open(data.join("profiles.json")).unwrap(), history::HistoryEngine::open(&data.join("history.jsonl")).unwrap(), T0);
    g.dev_mode = true;
    let mut rec = Recorder::start(&rec_dir, &data, T0, true).unwrap();
    let mut bellona_token = None;
    for tick in 1..=900u64 {
        let mut events = script(tick);
        if tick == 430 {
            events.push(Event::Msg(3, ClientMsg::Hello { version: PROTOCOL_VERSION, name: "Bellona".into(), token: bellona_token.clone() }));
        }
        for e in events {
            rec.event(e.clone());
            match e {
                Event::Msg(c, m) => g.handle(c, m),
                Event::Close(c) => g.disconnect(c),
            }
        }
        // The clock advances once a second, as on a real server.
        let unix = T0 + tick / 20;
        g.tick(0.05, unix);
        for (_, m) in g.out.drain(..) {
            if let ServerMsg::Welcome { token, .. } = m
                && bellona_token.is_none()
                && g.store.find_by_name("Bellona").is_some_and(|p| g.store.account(p).is_some_and(|a| a.token == token))
            {
                bellona_token = Some(token);
            }
        }
        rec.end_tick(&mut g, unix).unwrap();
    }
    let live = digest(&g);
    drop(rec);
    let tuning = rec_dir.join(server::harness::TUNING_DIR);
    assert_eq!(shared::tuning::Tuning::load(&tuning).unwrap(), *g.tuning, "the recording keeps the numbers it ran with");
    assert!(g.systems[&7].ents.len() > 2, "the fight happened in the war zone");
    assert!(g.sessions.values().any(|s| s.player.is_some() && s.entity.is_some() && s.system == 7));

    // The recording holds no credentials: neither issued tokens nor tokens used to log in appear in it.
    let recorded = std::fs::read_to_string(rec_dir.join("session.jsonl")).unwrap();
    let real_tokens: Vec<String> = g.store.data.accounts.values().map(|a| a.token.clone()).collect();
    assert!(real_tokens.len() >= 2 && bellona_token.is_some());
    for t in &real_tokens {
        assert!(!recorded.contains(t.as_str()), "token leaked into the recording");
    }
    assert!(recorded.contains("issued-"), "tokens are pseudonymized, not dropped");

    let session = Session::load(&rec_dir).unwrap();
    let a = session.replay(|_| {}).unwrap();
    assert!(a.digests_checked >= 9, "digests every 100 ticks: {}", a.digests_checked);
    assert_eq!(a.divergence, None, "replay must match the recording");
    assert_eq!(a.final_digest, live, "replay ends in the live state");
    let b = session.replay(|_| {}).unwrap();
    assert_eq!(a.final_digest, b.final_digest, "replays are deterministic");
    let _ = std::fs::remove_dir_all(&root);
}

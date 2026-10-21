//! Chapter-end comms call (docs/STORY.md §4): `state::apply` opens `Session::comms` when the server's
//! profile shows a chapter cleared for the first time; this draws it over the HUD with
//! `view::comms::draw` and moves it on with the clock and clicks. Client only: the server and the
//! recorded world never see it.

use crate::state::Session;
use shared::galaxy::Species;
use std::sync::OnceLock;
use view::comms::Comms;

/// `assets/cinematics/comms.ron`, read once (problems printed, a broken file shows no calls).
fn file() -> &'static Comms {
    static FILE: OnceLock<Comms> = OnceLock::new();
    FILE.get_or_init(|| {
        let path = crate::ships::assets_dir().join("cinematics/comms.ron");
        let parsed = std::fs::read_to_string(&path).map_err(|e| e.to_string()).and_then(|t| Comms::parse(&t));
        match parsed {
            Ok(c) => {
                for p in view::comms::check(&c) {
                    eprintln!("comms: {p}");
                }
                c
            }
            Err(e) => {
                eprintln!("{}: {e}", path.display());
                Comms { speakers: vec![], chapters: vec![] }
            }
        }
    })
}

pub fn show(ctx: &bevy_egui::egui::Context, s: &mut Session, now: f64) {
    let Some(mut panel) = s.comms.take() else { return };
    let species = s.profile.as_ref().map_or(Species::Human, |p| p.species);
    let c = file();
    let lines = c.lines(panel.after, species);
    let Some(line) = lines.get(panel.line) else { return };
    let Some(who) = c.speaker(&line.who) else { return };
    let dt = (now - panel.since) as f32;
    let shown = view::comms::typed(&line.text, dt);
    let clicked = view::comms::draw(ctx, who, shown, shown.len() < line.text.len(), (panel.line, lines.len()), now);
    let texts: Vec<&str> = lines.iter().map(|l| l.text.as_str()).collect();
    if panel.step(&texts, now, clicked) {
        s.comms = Some(panel);
    }
}

#[cfg(test)]
mod tests {
    use crate::net::{Net, NetEvent};
    use crate::state::{Screen, Session};
    use bevy::prelude::*;
    use bevy::state::app::StatesPlugin;
    use shared::protocol::{Profession, Profile, ServerMsg};

    fn profile(cleared: u32) -> Profile {
        let species = shared::galaxy::Species::Veyr;
        Profile {
            player_id: 1,
            name: "Tester".into(),
            species,
            profession: Profession::Explorer,
            ship: shared::ships::Ship::new("SNS Tester".into(), shared::ships::HullClass::Talon, 5),
            wallet: Default::default(),
            cargo: Default::default(),
            home: species.home_system(),
            dailies: vec![],
            daily_day: 0,
            scenarios_completed: cleared,
            scenario: None,
            kills: 0,
            deaths: 0,
            reputation: 0,
            bounty: 0,
            archive: vec![],
            last_system: 0,
            scanned: vec![],
            specimens: Default::default(),
        }
    }

    /// Feeds profile updates as the server sends them and returns the comms call after each.
    fn calls_after(updates: &[u32]) -> Vec<Option<u32>> {
        let (net, feed, _out) = Net::local();
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, StatesPlugin)).init_state::<Screen>().insert_resource(net).init_resource::<Session>().add_systems(PreUpdate, crate::state::pump_network);
        updates
            .iter()
            .map(|&n| {
                feed.send(NetEvent::Msg(Box::new(ServerMsg::Profile(profile(n))))).unwrap();
                app.update();
                app.world().resource::<Session>().comms.as_ref().map(|p| p.after)
            })
            .collect()
    }

    #[test]
    fn clearing_a_chapter_for_the_first_time_opens_its_call() {
        // Login (2 already cleared): no call. Other profile updates: no call. Chapter 3 cleared: its call.
        assert_eq!(calls_after(&[2, 2, 3]), [None, None, Some(2)]);
    }

    #[test]
    fn the_call_file_ships_with_the_client_and_is_valid() {
        let c = super::file();
        assert_eq!(c.chapters.len(), shared::missions::scenarios().len());
        assert!(view::comms::check(c).is_empty());
    }
}

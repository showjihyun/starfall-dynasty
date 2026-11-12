//! Daily missions against the authoritative `Game`: the PvP ones need a second captain, and a visit
//! daily points at a place the captain has not been.

use shared::galaxy::{self, RiskTier, Species};
use shared::glam::Vec3;
use shared::missions::{self, DailyMission, Objective};
use shared::protocol::*;
use shared::ships::{HullClass, Materials};
use sim::game::{ConnId, ENGAGEMENT_TIMEOUT, Game};
use sim::store::Store;

const DT: f32 = 0.05;
const T0: u64 = 1_800_000_000;

fn join(g: &mut Game, conn: ConnId, name: &str, hull: HullClass) -> u64 {
    g.handle(conn, ClientMsg::Hello { version: PROTOCOL_VERSION, name: name.into(), token: None });
    g.handle(conn, ClientMsg::CreateCharacter { species: Species::Human, profession: Profession::CombatPilot, hull, ship_name: format!("{name}-ship"), livery: 1 });
    g.sessions[&conn].player.unwrap()
}

#[test]
fn the_pvp_dailies_are_won_by_sinking_another_captain() {
    let mut g = Game::new(Store::memory(), history::HistoryEngine::new(), T0);
    let a = join(&mut g, 1, "Raven", HullClass::Bastion);
    let b = join(&mut g, 2, "Sparrow", HullClass::Talon);
    let day = missions::day_index(T0);
    g.ensure_dailies(a, day);
    let reward = Materials { credits: 2700, ..Default::default() };
    g.store.profile_mut(a).unwrap().dailies = vec![
        DailyMission { objective: Objective::KillPlayers { count: 1 }, progress: 0, reward: reward.clone(), claimed: false },
        DailyMission { objective: Objective::WinEngagement, progress: 0, reward: reward.clone(), claimed: false },
    ];
    // Both in a lawless system, guns in range.
    let wild = galaxy::systems().iter().find(|s| s.tier == RiskTier::Wild).unwrap().id;
    g.spawn_player(a, wild, false);
    g.spawn_player(b, wild, false);
    g.systems.get_mut(&wild).unwrap().ents.retain(|_, e| !matches!(e.kind, EntityKind::Npc(_)));
    let (ea, eb) = (g.sessions[&1].entity.unwrap(), g.sessions[&2].entity.unwrap());
    let at = g.ent(wild, ea).unwrap().pos;
    g.ent_mut(wild, eb).unwrap().pos = at + Vec3::Z * 300.0;
    let hp = g.ent(wild, eb).map(|e| e.hull + e.shield).unwrap();
    g.apply_damage(wild, ea, eb, hp + 1.0);
    g.tick(DT, T0);
    assert!(g.store.profile(a).unwrap().dailies[0].complete(), "a captain sunk");
    // The engagement ends once the guns fall quiet; the side that lost less wins it.
    for k in 0..((ENGAGEMENT_TIMEOUT as f32 + 2.0) / DT) as u64 {
        g.tick(DT, T0 + (k as f32 * DT) as u64);
    }
    assert!(g.store.profile(a).unwrap().dailies[1].complete(), "the engagement won");
    let before = g.store.profile(a).unwrap().wallet.credits;
    g.handle(1, ClientMsg::ClaimDaily { index: 0 });
    g.handle(1, ClientMsg::ClaimDaily { index: 1 });
    let p = g.store.profile(a).unwrap();
    assert_eq!(p.wallet.credits, before + 2 * reward.credits);
    assert!(p.dailies.iter().all(|m| m.claimed));
}

#[test]
fn a_visit_daily_sends_the_captain_somewhere_new() {
    let mut g = Game::new(Store::memory(), history::HistoryEngine::new(), T0);
    let a = join(&mut g, 1, "Wanderer", HullClass::Wren);
    let home = g.store.profile(a).unwrap().home;
    // Every day for a month: a visit daily is never home and never a place already seen.
    for d in 0..30u64 {
        let unix = T0 + d * 86_400;
        let p = g.store.profile_mut(a).unwrap();
        p.dailies.clear();
        let seen = p.career.visited.clone();
        g.ensure_dailies(a, missions::day_index(unix));
        for m in g.store.profile(a).unwrap().dailies.clone() {
            if let Objective::Visit { system } = m.objective {
                assert_ne!(system, home);
                assert!(!seen.contains(&system), "day {d}: {} already visited", galaxy::system(system).name);
                // Go there: it is seen from now on.
                g.store.profile_mut(a).unwrap().career.visited.insert(system);
            }
        }
    }
}

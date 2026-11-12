//! Major operations against the authoritative `Game`: unlocking, the convoy of an escort step (lost,
//! formed again, brought in by docking) and the clock of a survive step.

use shared::galaxy::Species;
use shared::glam::Vec3;
use shared::missions::{OP_BASE, operations};
use shared::protocol::*;
use shared::ships::HullClass;
use sim::game::{ConnId, Game};
use sim::store::Store;

const DT: f32 = 0.05;
const T0: u64 = 1_800_000_000;

fn join(g: &mut Game, conn: ConnId, name: &str) -> u64 {
    g.handle(conn, ClientMsg::Hello { version: PROTOCOL_VERSION, name: name.into(), token: None });
    g.handle(conn, ClientMsg::CreateCharacter { species: Species::Human, profession: Profession::CombatPilot, hull: HullClass::Bastion, ship_name: format!("{name}-ship"), livery: 1 });
    g.sessions[&conn].player.unwrap()
}

fn toasts(g: &Game) -> Vec<String> {
    g.out.iter().filter_map(|(_, m)| if let ServerMsg::Toast { text } = m { Some(text.clone()) } else { None }).collect()
}

fn no_pirates(g: &mut Game, sys: u16) {
    g.systems.get_mut(&sys).unwrap().ents.retain(|_, e| !matches!(e.kind, EntityKind::Npc(_)));
}

#[test]
fn an_operation_opens_with_its_chapter_and_is_played_once() {
    let mut g = Game::new(Store::memory(), history::HistoryEngine::new(), T0);
    let pid = join(&mut g, 1, "Cadet");
    let last = operations().last().unwrap().clone();
    g.handle(1, ClientMsg::StartScenario { id: last.id });
    assert!(g.store.profile(pid).unwrap().scenario.is_none(), "locked before chapter {}", last.unlock);
    g.store.profile_mut(pid).unwrap().scenarios_completed = last.unlock;
    g.handle(1, ClientMsg::StartScenario { id: last.id });
    assert_eq!(g.store.profile(pid).unwrap().scenario.as_ref().map(|s| s.id), Some(last.id));
    g.handle(1, ClientMsg::AbandonScenario);
    // Done once: not again.
    g.store.profile_mut(pid).unwrap().operations_done.push(last.id);
    g.handle(1, ClientMsg::StartScenario { id: last.id });
    assert!(g.store.profile(pid).unwrap().scenario.is_none());
}

#[test]
fn a_convoy_forms_is_lost_forms_again_and_is_brought_in_by_docking() {
    let mut g = Game::new(Store::memory(), history::HistoryEngine::new(), T0);
    let pid = join(&mut g, 1, "Shepherd");
    let op = operations().into_iter().find(|o| o.id == OP_BASE).unwrap();
    let step = op.steps.iter().position(|s| matches!(s, shared::missions::Step::Escort { .. })).unwrap();
    let shared::missions::Step::Escort { system, ships, .. } = op.steps[step].clone() else { unreachable!() };
    let p = g.store.profile_mut(pid).unwrap();
    p.scenarios_completed = op.unlock;
    p.scenario = Some(ScenarioProgress { id: op.id, step, step_progress: 0 });
    // Arrive: the freighters join behind the ship, the ambush comes out ahead.
    g.spawn_player(pid, system, false);
    assert_eq!(g.sessions[&1].convoy.len() as u32, ships, "the convoy formed");
    assert!(!g.sessions[&1].wave.is_empty(), "the ambush");
    // Every freighter sunk: the step waits for a new convoy.
    let me = g.sessions[&1].entity.unwrap();
    let pirate = g.sessions[&1].wave[0];
    for c in g.sessions[&1].convoy.clone() {
        let hp = g.ent(system, c).map(|e| e.hull + e.shield).unwrap();
        g.apply_damage(system, pirate, c, hp + 1.0);
        g.tick(DT, T0);
    }
    assert!(g.sessions[&1].convoy.is_empty());
    assert!(toasts(&g).iter().any(|t| t.contains("호송선단 전멸")));
    // Docking without them does nothing; setting out again forms a new convoy.
    no_pirates(&mut g, system);
    let station = g.systems[&system].layout.station;
    g.ent_mut(system, me).unwrap().pos = station + Vec3::Z * 150.0;
    g.ent_mut(system, me).unwrap().last_hit = -100.0;
    g.handle(1, ClientMsg::Dock);
    assert_eq!(g.store.profile(pid).unwrap().scenario.as_ref().map(|s| s.step), Some(step));
    g.handle(1, ClientMsg::Undock);
    assert_eq!(g.sessions[&1].convoy.len() as u32, ships, "a new convoy joined on setting out");
    // Bring them in: freighters near the station, the captain docks.
    no_pirates(&mut g, system);
    for c in g.sessions[&1].convoy.clone() {
        g.ent_mut(system, c).unwrap().pos = station + Vec3::X * 600.0;
    }
    let me = g.sessions[&1].entity.unwrap();
    g.ent_mut(system, me).unwrap().pos = station + Vec3::Z * 150.0;
    g.ent_mut(system, me).unwrap().last_hit = -100.0;
    let before = g.store.profile(pid).unwrap().wallet.credits;
    g.handle(1, ClientMsg::Dock);
    let p = g.store.profile(pid).unwrap();
    assert!(p.scenario.is_none() && p.operations_done.contains(&op.id), "the escort was the last step");
    assert_eq!(p.wallet.credits, before + op.reward.credits);
    assert!(g.sessions[&1].convoy.is_empty(), "the freighters are in");
}

#[test]
fn a_survive_step_counts_while_out_waves_on_the_beat_and_pauses_docked() {
    let mut g = Game::new(Store::memory(), history::HistoryEngine::new(), T0);
    let pid = join(&mut g, 1, "Bulwark");
    let op = operations().into_iter().find(|o| o.steps.iter().any(|s| matches!(s, shared::missions::Step::Survive { .. }))).unwrap();
    let step = op.steps.iter().position(|s| matches!(s, shared::missions::Step::Survive { .. })).unwrap();
    let shared::missions::Step::Survive { system, secs, every, .. } = op.steps[step].clone() else { unreachable!() };
    let p = g.store.profile_mut(pid).unwrap();
    p.scenarios_completed = op.unlock;
    p.scenario = Some(ScenarioProgress { id: op.id, step, step_progress: 0 });
    g.spawn_player(pid, system, false);
    assert!(!g.sessions[&1].wave.is_empty(), "the first wave comes at once");
    let me = g.sessions[&1].entity.unwrap();
    let progress = |g: &Game| g.store.profile(pid).unwrap().scenario.as_ref().map(|s| (s.step, s.step_progress));
    // Ten seconds out: ten on the clock (the pirates are cleared away so the ship lives).
    let mut waves = 0;
    let run = |g: &mut Game, secs: f32, waves: &mut usize| {
        for k in 0..(secs / DT) as u32 {
            g.out.clear();
            g.tick(DT, T0 + k as u64 / 20);
            *waves += toasts(g).iter().filter(|t| t.contains("새 습격")).count();
            no_pirates(g, system);
            if let Some(e) = g.ent_mut(system, me) {
                e.hull = e.stats.max_hull;
            }
        }
    };
    run(&mut g, 10.0, &mut waves);
    assert_eq!(progress(&g), Some((step, 10)));
    // Docked: the clock stops.
    let station = g.systems[&system].layout.station;
    g.ent_mut(system, me).unwrap().pos = station + Vec3::Z * 150.0;
    g.ent_mut(system, me).unwrap().last_hit = -100.0;
    g.handle(1, ClientMsg::Dock);
    run(&mut g, 5.0, &mut waves);
    assert_eq!(progress(&g), Some((step, 10)), "no hiding in the station");
    g.handle(1, ClientMsg::Undock);
    run(&mut g, secs as f32 - 10.0 + 1.5, &mut waves);
    assert_eq!(progress(&g).map(|p| p.0), Some(step + 1), "held: the next step");
    assert!(waves as u32 >= (secs - 1) / every, "a wave every {every} s: {waves}");
}

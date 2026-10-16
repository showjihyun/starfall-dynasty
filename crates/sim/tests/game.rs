//! Gameplay rules exercised directly against the authoritative `Game` (no sockets).

use sim::game::{ConnId, Game};
use sim::store::Store;
use shared::galaxy::{self, Ore, PoiKind, Species};
use shared::glam::{Quat, Vec3};
use shared::missions::{NpcArchetype, Objective};
use shared::protocol::*;
use shared::ships::{HullClass, Module};

const DT: f32 = 0.05;
const T0: u64 = 1_800_000_000;

fn game() -> Game {
    Game::new(Store::memory(), history::HistoryEngine::new(), T0)
}

fn run(g: &mut Game, secs: f32) {
    let n = (secs / DT).ceil() as usize;
    for _ in 0..n {
        g.tick(DT, T0);
    }
}

fn join(g: &mut Game, conn: ConnId, name: &str, species: Species, hull: HullClass, prof: Profession) -> PlayerId {
    g.handle(conn, ClientMsg::Hello { version: PROTOCOL_VERSION, name: name.into(), token: None });
    g.handle(conn, ClientMsg::CreateCharacter { species, profession: prof, hull, ship_name: format!("{name}-ship"), livery: 1 });
    let pid = g.sessions[&conn].player.unwrap();
    g.out.clear();
    pid
}

fn errors(g: &Game, conn: ConnId) -> Vec<String> {
    g.out.iter().filter(|(c, _)| *c == conn).filter_map(|(_, m)| if let ServerMsg::Error { msg } = m { Some(msg.clone()) } else { None }).collect()
}

fn ent_of(g: &Game, conn: ConnId) -> (u16, EntityId) {
    let s = &g.sessions[&conn];
    (s.system, s.entity.expect("entity"))
}

fn place(g: &mut Game, conn: ConnId, pos: Vec3, facing: Vec3) {
    let (s, e) = ent_of(g, conn);
    let ent = g.ent_mut(s, e).unwrap();
    ent.docked = false;
    ent.pos = pos;
    ent.vel = Vec3::ZERO;
    ent.rot = Quat::from_rotation_arc(Vec3::NEG_Z, (facing - pos).normalize());
}

/// Teleport a player into another system (test shortcut for warping).
fn move_to_system(g: &mut Game, conn: ConnId, sys: u16) {
    let pid = g.sessions[&conn].player.unwrap();
    g.spawn_player(pid, sys, false);
}

fn clear_npcs(g: &mut Game, sys: u16) {
    g.systems.get_mut(&sys).unwrap().ents.retain(|_, e| !matches!(e.kind, EntityKind::Npc(_)));
}

#[test]
fn only_one_ship_per_captain() {
    let mut g = game();
    join(&mut g, 1, "Arin", Species::Human, HullClass::Talon, Profession::CombatPilot);
    g.handle(1, ClientMsg::CreateCharacter { species: Species::Kharz, profession: Profession::Miner, hull: HullClass::Bastion, ship_name: "Second".into(), livery: 0 });
    assert!(errors(&g, 1).iter().any(|e| e.contains("단 한 척")));
    assert_eq!(g.store.profile(g.sessions[&1].player.unwrap()).unwrap().ship.hull, HullClass::Talon);
}

#[test]
fn token_login_and_name_protection() {
    let mut g = game();
    g.handle(1, ClientMsg::Hello { version: PROTOCOL_VERSION, name: "Elena".into(), token: None });
    let token = g.out.iter().find_map(|(_, m)| if let ServerMsg::Welcome { token, .. } = m { Some(token.clone()) } else { None }).unwrap();
    g.disconnect(1);
    g.out.clear();
    g.handle(2, ClientMsg::Hello { version: PROTOCOL_VERSION, name: "elena".into(), token: Some("wrong".into()) });
    assert!(!errors(&g, 2).is_empty());
    g.out.clear();
    g.handle(3, ClientMsg::Hello { version: PROTOCOL_VERSION, name: "Elena".into(), token: Some(token) });
    assert!(errors(&g, 3).is_empty());
    assert!(g.sessions[&3].player.is_some());
}

#[test]
fn warp_range_gates_exploration() {
    let mut g = game();
    join(&mut g, 1, "Arin", Species::Human, HullClass::Talon, Profession::Explorer);
    g.handle(1, ClientMsg::Undock);
    g.handle(1, ClientMsg::Warp { system: 9 });
    assert!(errors(&g, 1).iter().any(|e| e.contains("워프 거리 부족")));
    g.out.clear();
    g.handle(1, ClientMsg::Warp { system: 2 });
    assert!(errors(&g, 1).is_empty());
    run(&mut g, 4.5);
    assert_eq!(g.sessions[&1].system, 2);
    assert!(g.out.iter().any(|(_, m)| matches!(m, ServerMsg::EnterSystem { system: 2, .. })));
}

#[test]
fn pve_kill_pays_bounty_and_progresses_daily() {
    let mut g = game();
    let pid = join(&mut g, 1, "Arin", Species::Human, HullClass::Bastion, Profession::CombatPilot);
    move_to_system(&mut g, 1, 3);
    clear_npcs(&mut g, 3);
    let me = Vec3::new(0.0, 0.0, 9000.0);
    let raider = g.spawn_npc(3, NpcArchetype::Raider, 0, me + Vec3::new(0.0, 0.0, -300.0), None);
    place(&mut g, 1, me, me + Vec3::new(0.0, 0.0, -300.0));
    let credits0 = g.store.profile(pid).unwrap().wallet.credits;
    let (s, e) = ent_of(&g, 1);
    g.ent_mut(s, e).unwrap().target = Some(raider);
    g.handle(1, ClientMsg::Controls(Controls { fire: true, ..Default::default() }));
    for _ in 0..400 {
        g.tick(DT, T0);
        // Keep the nose on the raider, the way a pilot would.
        let Some(rp) = g.ent(3, raider).map(|r| r.pos) else { break };
        let ent = g.ent_mut(s, e).unwrap();
        ent.rot = Quat::from_rotation_arc(Vec3::NEG_Z, (rp - ent.pos).normalize());
    }
    assert!(g.ent(3, raider).is_none(), "raider should be destroyed");
    let p = g.store.profile(pid).unwrap();
    assert_eq!(p.kills, 1);
    assert!(p.wallet.credits > credits0);
    let kp = p.dailies.iter().find(|m| matches!(m.objective, Objective::KillPirates { .. })).unwrap();
    assert_eq!(kp.progress, 1);
}

#[test]
fn pvp_rules_battle_history_and_investigation() {
    let mut g = game();
    let a = join(&mut g, 1, "Alpha", Species::Human, HullClass::Talon, Profession::CombatPilot);
    let _b = join(&mut g, 2, "Bravo", Species::Kharz, HullClass::Bastion, Profession::CombatPilot);
    let c = join(&mut g, 3, "Clio", Species::Veyr, HullClass::Seer, Profession::Historian);

    // Core space: no PvP even point-blank.
    move_to_system(&mut g, 1, 0);
    move_to_system(&mut g, 2, 0);
    let (_, ea) = ent_of(&g, 1);
    let (_, eb) = ent_of(&g, 2);
    assert!(!g.can_damage(0, ea, eb));

    // War zone: free fire.
    for conn in [1, 2] {
        move_to_system(&mut g, conn, 7);
    }
    clear_npcs(&mut g, 7);
    let (_, ea) = ent_of(&g, 1);
    let (_, eb) = ent_of(&g, 2);
    assert!(g.can_damage(7, ea, eb));
    // Give Bravo some cargo to lose.
    g.store.profile_mut(g.sessions[&2].player.unwrap()).unwrap().cargo.insert("Xenite".into(), 12);
    let p = Vec3::new(0.0, 0.0, 9000.0);
    place(&mut g, 1, p, p + Vec3::Z * -200.0);
    place(&mut g, 2, p + Vec3::Z * -200.0, p);
    for _ in 0..200 {
        g.apply_damage(7, ea, eb, 50.0);
        g.tick(DT, T0);
        if g.ent(7, eb).is_none() {
            break;
        }
    }
    assert!(g.sessions[&2].dead, "Bravo destroyed");
    assert_eq!(g.store.profile(a).unwrap().kills, 1);
    // Engagement concludes after the timeout and becomes a battle in the chronicle.
    run(&mut g, 21.0);
    assert!(g.history.events.iter().any(|e| e.kind == "BATTLE"), "battle recorded");
    let destroyed = g.history.events.iter().find(|e| e.kind == "SHIP_DESTROYED").unwrap().id;

    // Days later, Clio flies to the wreck and investigates it.
    g.handle(2, ClientMsg::Respawn);
    move_to_system(&mut g, 3, 7);
    clear_npcs(&mut g, 7);
    let (wreck_id, wreck_pos) = g.systems[&7].ents.values().find(|e| e.kind == EntityKind::Wreck).map(|e| (e.id, e.pos)).unwrap();
    place(&mut g, 3, wreck_pos + Vec3::X * 100.0, wreck_pos);
    g.handle(3, ClientMsg::InvestigateWreck { wreck: wreck_id });
    let prof = g.store.profile(c).unwrap().clone();
    assert!(prof.archive.len() >= 2, "historian gets evidence: {:?}", prof.archive);
    assert_eq!(prof.cargo.get("Xenite"), Some(&12), "salvaged cargo");
    g.handle(3, ClientMsg::PublishResearch { event: destroyed, thesis: "Alpha가 선제 공격했다".into(), cite: prof.archive.clone() });
    let ev = g.history.event(destroyed).unwrap();
    assert_eq!(ev.research.len(), 1);
    assert!(ev.research[0].confidence > 0.5);
    // Bravo cannot dispute without holding evidence.
    g.out.clear();
    g.handle(2, ClientMsg::DisputeResearch { event: destroyed, author: "Clio".into() });
    assert!(!errors(&g, 2).is_empty());
}

#[test]
fn wild_space_crime_and_bounty_hunting() {
    let mut g = game();
    let pirate = join(&mut g, 1, "Vex", Species::Kharz, HullClass::Talon, Profession::CombatPilot);
    let hunter = join(&mut g, 2, "Rook", Species::Human, HullClass::Bastion, Profession::CombatPilot);
    join(&mut g, 3, "Mira", Species::Veyr, HullClass::Mule, Profession::Miner);
    for c in [1, 2, 3] {
        move_to_system(&mut g, c, 5);
    }
    clear_npcs(&mut g, 5);
    let (_, ev) = ent_of(&g, 1);
    let (_, eh) = ent_of(&g, 2);
    let (_, em) = ent_of(&g, 3);
    g.apply_damage(5, ev, em, 10.0);
    let vp = g.store.profile(pirate).unwrap();
    assert!(vp.bounty > 0, "attacking an innocent in Wild space is a crime");
    assert!(g.ent(5, ev).unwrap().wanted);
    assert!(g.history.events.iter().any(|e| e.kind == "BOUNTY"));
    // Shooting back at a wanted player is not a crime.
    g.apply_damage(5, eh, ev, 1.0);
    assert_eq!(g.store.profile(hunter).unwrap().bounty, 0);
    let bounty = g.store.profile(pirate).unwrap().bounty;
    let credits0 = g.store.profile(hunter).unwrap().wallet.credits;
    for _ in 0..100 {
        if g.ent(5, ev).is_none() {
            break;
        }
        g.apply_damage(5, eh, ev, 80.0);
        g.tick(DT, T0);
    }
    assert!(g.store.profile(hunter).unwrap().wallet.credits >= credits0 + bounty);
    assert_eq!(g.store.profile(pirate).unwrap().bounty, 0);
    let claimed = g.history.events.iter().find(|e| e.kind == "BOUNTY_CLAIMED").unwrap();
    assert!(!claimed.causes.is_empty(), "bounty claim linked to the original crime");
}

#[test]
fn fleets_do_not_friendly_fire_and_share_credit() {
    let mut g = game();
    let a = join(&mut g, 1, "Lead", Species::Human, HullClass::Talon, Profession::CombatPilot);
    let b = join(&mut g, 2, "Wing", Species::Human, HullClass::Talon, Profession::CombatPilot);
    g.handle(1, ClientMsg::FleetInvite { name: "Wing".into() });
    let fid = g.out.iter().find_map(|(c, m)| if let (2, ServerMsg::FleetInvite { fleet, .. }) = (*c, m) { Some(*fleet) } else { None }).unwrap();
    g.handle(2, ClientMsg::FleetAccept { fleet: fid });
    assert_eq!(g.fleet_of(a), g.fleet_of(b));
    for c in [1, 2] {
        move_to_system(&mut g, c, 7);
    }
    let (_, ea) = ent_of(&g, 1);
    let (_, eb) = ent_of(&g, 2);
    assert!(!g.can_damage(7, ea, eb));
    assert_eq!(g.credit_group(a, 7).len(), 2);
}

#[test]
fn upgrades_cost_resources_and_respect_tier_cap() {
    let mut g = game();
    let pid = join(&mut g, 1, "Arin", Species::Human, HullClass::Wren, Profession::Explorer);
    // Starts docked.
    g.store.profile_mut(pid).unwrap().wallet = shared::ships::Materials { credits: 10_000_000, alloy: 10_000, crystal: 10_000, datacore: 10_000, relic: 100 };
    let warp0 = g.store.profile(pid).unwrap().ship.stats().warp_range;
    for _ in 0..9 {
        g.handle(1, ClientMsg::Upgrade { module: Module::WarpCore });
    }
    let p = g.store.profile(pid).unwrap();
    assert_eq!(p.ship.level(Module::WarpCore), 5, "capped at Lv5 before scenarios");
    assert!(p.ship.stats().warp_range > warp0 + 2.0);
    // With enough warp, The Deep is reachable from Ashen Gate.
    let mut s = p.ship.clone();
    s.modules.insert(Module::WarpCore, 5);
    assert!(s.stats().warp_range >= galaxy::distance_ly(8, 9));
    // Not docked → refused.
    g.handle(1, ClientMsg::Undock);
    g.handle(1, ClientMsg::Upgrade { module: Module::Hull });
    assert_eq!(g.store.profile(pid).unwrap().ship.level(Module::Hull), 1);
}

#[test]
fn scenario_one_full_run_unlocks_tier() {
    let mut g = game();
    let pid = join(&mut g, 1, "Arin", Species::Human, HullClass::Bastion, Profession::CombatPilot);
    g.handle(1, ClientMsg::StartScenario { id: 1 });
    assert!(g.store.profile(pid).unwrap().scenario.is_none(), "chapters are sequential");
    g.handle(1, ClientMsg::StartScenario { id: 0 });
    assert_eq!(g.store.profile(pid).unwrap().scenario.as_ref().unwrap().step, 0);
    move_to_system(&mut g, 1, 3);
    clear_npcs(&mut g, 3);
    assert_eq!(g.store.profile(pid).unwrap().scenario.as_ref().unwrap().step, 1, "reach step done");
    let signal = g.systems[&3].layout.pois.iter().find(|p| p.kind == PoiKind::Signal).unwrap().clone();
    place(&mut g, 1, signal.pos + Vec3::X * 100.0, signal.pos);
    g.handle(1, ClientMsg::Scan { poi: signal.id });
    run(&mut g, 5.0);
    assert_eq!(g.store.profile(pid).unwrap().scenario.as_ref().unwrap().step, 2, "scan step done");
    let wave = g.sessions[&1].wave.clone();
    assert_eq!(wave.len(), 3, "raider wave spawned");
    let (_, me) = ent_of(&g, 1);
    for w in wave {
        let hp = g.ent(3, w).map(|e| e.hull + e.shield).unwrap();
        g.apply_damage(3, me, w, hp + 1.0);
    }
    g.tick(DT, T0);
    let p = g.store.profile(pid).unwrap();
    assert!(p.scenario.is_none());
    assert_eq!(p.scenarios_completed, 1);
    assert_eq!(shared::ships::tier_cap(p.scenarios_completed), 6);
    assert!(g.history.events.iter().any(|e| e.kind == "SCENARIO"));
}

#[test]
fn mining_selling_and_market_prices() {
    let mut g = game();
    let pid = join(&mut g, 1, "Mira", Species::Veyr, HullClass::Mule, Profession::Miner);
    move_to_system(&mut g, 1, 3);
    clear_npcs(&mut g, 3);
    let ast = g.systems[&3].layout.asteroids.iter().find(|a| a.ore == Ore::Ferrite).unwrap().clone();
    place(&mut g, 1, ast.pos + Vec3::Y * (ast.radius + 60.0), ast.pos);
    g.handle(1, ClientMsg::Mine { asteroid: Some(ast.id) });
    run(&mut g, 12.0);
    let mined = *g.store.profile(pid).unwrap().cargo.get("Ferrite").unwrap_or(&0);
    assert!(mined >= 20, "mined {mined}");
    // Dock and sell: price falls as supply rises.
    let station = g.systems[&3].layout.station;
    place(&mut g, 1, station + Vec3::X * 50.0, station);
    g.handle(1, ClientMsg::Dock);
    let c0 = g.store.profile(pid).unwrap().wallet.credits;
    g.handle(1, ClientMsg::Sell { ore: Ore::Ferrite, units: 10 });
    let first = g.store.profile(pid).unwrap().wallet.credits - c0;
    let c1 = g.store.profile(pid).unwrap().wallet.credits;
    g.handle(1, ClientMsg::Sell { ore: Ore::Ferrite, units: 10 });
    let second = g.store.profile(pid).unwrap().wallet.credits - c1;
    assert!(first > 0 && second < first, "first {first} second {second}");
    let sold = g.store.profile(pid).unwrap().dailies.iter().find(|m| matches!(m.objective, Objective::SellCargo { .. })).map(|m| m.progress);
    if let Some(sold) = sold {
        assert_eq!(sold, 20);
    }
}

#[test]
fn daily_claim_once_when_complete() {
    let mut g = game();
    let pid = join(&mut g, 1, "Arin", Species::Human, HullClass::Talon, Profession::CombatPilot);
    let target = g.store.profile(pid).unwrap().dailies[0].objective.target();
    g.handle(1, ClientMsg::ClaimDaily { index: 0 });
    assert!(!g.store.profile(pid).unwrap().dailies[0].claimed);
    for _ in 0..target {
        g.progress(pid, shared::missions::Progress::KilledPirate);
    }
    let c0 = g.store.profile(pid).unwrap().wallet.credits;
    g.handle(1, ClientMsg::ClaimDaily { index: 0 });
    let c1 = g.store.profile(pid).unwrap().wallet.credits;
    assert!(c1 > c0);
    g.handle(1, ClientMsg::ClaimDaily { index: 0 });
    assert_eq!(g.store.profile(pid).unwrap().wallet.credits, c1);
}

#[test]
fn persistence_roundtrip() {
    let dir = std::env::temp_dir().join(format!("sfd-store-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let token;
    {
        let store = Store::open(dir.join("profiles.json")).unwrap();
        let hist = history::HistoryEngine::open(&dir.join("history.jsonl")).unwrap();
        let mut g = Game::new(store, hist, T0);
        join(&mut g, 1, "Arin", Species::Human, HullClass::Seer, Profession::Historian);
        token = g.store.account(1).unwrap().token.clone();
        g.store.save().unwrap();
    }
    let store = Store::open(dir.join("profiles.json")).unwrap();
    let hist = history::HistoryEngine::open(&dir.join("history.jsonl")).unwrap();
    let mut g = Game::new(store, hist, T0);
    g.handle(9, ClientMsg::Hello { version: PROTOCOL_VERSION, name: "Arin".into(), token: Some(token) });
    let welcome_has_profile = g.out.iter().any(|(_, m)| matches!(m, ServerMsg::Welcome { profile: Some(p), .. } if p.ship.hull == HullClass::Seer));
    assert!(welcome_has_profile);
    assert!(!g.history.biography("Arin").is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

/// Give a player a ship upgraded to all modules at `lvl` and respawn it undocked in `sys`.
fn grow(g: &mut Game, conn: ConnId, lvl: u8, sys: u16) {
    let pid = g.sessions[&conn].player.unwrap();
    {
        let p = g.store.profile_mut(pid).unwrap();
        for m in Module::ALL {
            p.ship.modules.insert(m, lvl);
        }
    }
    g.spawn_player(pid, sys, false);
}

#[test]
fn capital_ship_turrets_fight_all_around_and_escorts_follow() {
    use shared::ships::SizeClass;
    let mut g = game();
    let pid = join(&mut g, 1, "Reinhard", Species::Human, HullClass::Bastion, Profession::CombatPilot);
    grow(&mut g, 1, 9, 3);
    clear_npcs(&mut g, 3);
    let (s, me) = ent_of(&g, 1);
    let flag = g.ent(s, me).unwrap().clone();
    assert_eq!(flag.class, Some(SizeClass::Capital));
    assert!(flag.stats.radius * 2.2 > 400.0, "capital ship is hundreds of metres long");
    let escorts = g.sessions[&1].escorts.clone();
    assert_eq!(escorts.len() as u8, flag.stats.escorts);
    assert!(escorts.iter().all(|e| g.ent(s, *e).is_some_and(|x| x.kind == EntityKind::Escort && x.owner == Some(pid))));
    // A pirate sits BEHIND the flagship: fighter lasers couldn't hit it, turrets can.
    let behind = flag.pos - flag.forward() * (flag.stats.radius + 400.0);
    let pirate = g.spawn_npc(3, NpcArchetype::Gunship, 1, behind, None);
    g.ent_mut(s, me).unwrap().target = Some(pirate);
    for _ in 0..600 {
        g.tick(DT, T0);
        if g.ent(3, pirate).is_none() {
            break;
        }
    }
    assert!(g.ent(3, pirate).is_none(), "turret batteries destroyed a target behind the ship");
    assert!(g.out.iter().any(|(_, m)| matches!(m, ServerMsg::Snapshot { fx, .. } if fx.iter().any(|f| matches!(f, Fx::Shot { weapon: shared::ships::WeaponKind::Pulse, .. })))));
    assert_eq!(g.store.profile(pid).unwrap().kills, 1);
    // Main gun: trigger with a target dead ahead fires a beam volley.
    let ahead = flag.pos + flag.forward() * (flag.stats.radius + 900.0);
    let p2 = g.spawn_npc(3, NpcArchetype::Raider, 0, ahead, None);
    g.ent_mut(s, me).unwrap().target = Some(p2);
    g.out.clear();
    g.out.clear();
    g.handle(1, ClientMsg::Controls(Controls { fire: true, ..Default::default() }));
    run(&mut g, 2.0);
    assert!(g.out.iter().any(|(_, m)| matches!(m, ServerMsg::Snapshot { fx, .. } if fx.iter().any(|f| matches!(f, Fx::Shot { weapon: shared::ships::WeaponKind::MainBeam, .. })))));
    // Escorts follow the flagship when it moves.
    g.handle(1, ClientMsg::Controls(Controls { thrust: 1.0, ..Default::default() }));
    run(&mut g, 20.0);
    let fpos = g.ent(s, me).unwrap().pos;
    let r = g.ent(s, me).unwrap().stats.radius;
    for e in g.sessions[&1].escorts.clone() {
        let d = g.ent(s, e).unwrap().pos.distance(fpos);
        assert!(d < r * 6.0, "escort kept formation ({d} vs radius {r})");
    }
}

#[test]
fn fighter_has_no_turrets_and_docking_scales_with_size() {
    let mut g = game();
    join(&mut g, 1, "Yang", Species::Human, HullClass::Talon, Profession::CombatPilot);
    grow(&mut g, 1, 5, 0);
    let (s, me) = ent_of(&g, 1);
    let e = g.ent(s, me).unwrap().clone();
    assert_eq!(e.class, Some(shared::ships::SizeClass::Cruiser));
    assert!(!e.turret_cds.is_empty());
    // A cruiser is too big to get within the small-ship docking radius of the hub, but can still dock.
    let station = g.systems[&0].layout.station;
    place(&mut g, 1, station + Vec3::X * (shared::galaxy::DOCK_RANGE + e.stats.radius), station);
    g.handle(1, ClientMsg::Dock);
    assert!(g.ent(s, me).unwrap().docked);
    assert!(g.sessions[&1].escorts.is_empty(), "escorts stand down while docked");
}

#[test]
fn alliances_share_colours_and_wars_open_frontier_space() {
    let mut g = game();
    let a = join(&mut g, 1, "Yang", Species::Human, HullClass::Talon, Profession::CombatPilot);
    let a2 = join(&mut g, 2, "Julian", Species::Human, HullClass::Wren, Profession::CombatPilot);
    let b = join(&mut g, 3, "Reinhard", Species::Kharz, HullClass::Bastion, Profession::CombatPilot);
    g.handle(1, ClientMsg::AllianceCreate { name: "Free Planets".into(), tag: "FPA".into() });
    g.handle(3, ClientMsg::AllianceCreate { name: "Galactic Empire".into(), tag: "EMP".into() });
    let fpa = g.alliance_of(a).unwrap();
    let emp = g.alliance_of(b).unwrap();
    g.handle(1, ClientMsg::AllianceInvite { name: "Julian".into() });
    g.handle(2, ClientMsg::AllianceAccept { alliance: fpa });
    assert_eq!(g.alliance_of(a2), Some(fpa));
    assert!(g.history.events.iter().any(|e| e.kind == "ALLIANCE"));

    for c in [1, 2, 3] {
        move_to_system(&mut g, c, 3); // Orion Drift: Frontier, normally no PvP
    }
    clear_npcs(&mut g, 3);
    let (_, ea) = ent_of(&g, 1);
    let (_, ea2) = ent_of(&g, 2);
    let (_, eb) = ent_of(&g, 3);
    assert!(g.ent(3, ea).unwrap().name.starts_with("[FPA]"));
    assert!(!g.can_damage(3, ea, eb), "no PvP in Frontier at peace");
    // Only the leader may declare war.
    g.handle(2, ClientMsg::AllianceDeclareWar { target: emp });
    assert!(!g.at_war(Some(fpa), Some(emp)));
    g.handle(1, ClientMsg::AllianceDeclareWar { target: emp });
    assert!(g.at_war(Some(fpa), Some(emp)) && g.at_war(Some(emp), Some(fpa)));
    assert!(g.history.events.iter().any(|e| e.kind == "WAR"));
    assert!(g.can_damage(3, ea, eb), "war opens Frontier space");
    assert!(!g.can_damage(3, ea, ea2), "allies never hurt each other");
    // Core space stays safe even at war.
    for c in [1, 3] {
        move_to_system(&mut g, c, 0);
    }
    let (_, ea) = ent_of(&g, 1);
    let (_, eb) = ent_of(&g, 3);
    assert!(!g.can_damage(0, ea, eb));

    // An alliance battle in Wild space is no crime, and is recorded with alliance names.
    for c in [1, 3] {
        move_to_system(&mut g, c, 5);
    }
    clear_npcs(&mut g, 5);
    let (_, ea) = ent_of(&g, 1);
    let (_, eb) = ent_of(&g, 3);
    for _ in 0..200 {
        if g.ent(5, eb).is_none() {
            break;
        }
        g.apply_damage(5, ea, eb, 80.0);
        g.tick(DT, T0);
    }
    assert_eq!(g.store.profile(a).unwrap().bounty, 0, "war is not a crime");
    run(&mut g, 21.0);
    let battle = g.history.events.iter().find(|e| e.kind == "BATTLE").expect("battle");
    assert!(battle.summary.contains("[FPA]") && battle.summary.contains("[EMP]"), "{}", battle.summary);

    g.handle(1, ClientMsg::AlliancePeace { target: emp });
    assert!(!g.at_war(Some(fpa), Some(emp)));
    assert!(g.history.events.iter().any(|e| e.kind == "PEACE"));
}

#[test]
fn no_ship_is_invulnerable() {
    // A 1-point hit on a full-shield capital still costs hull; shields take the rest of each hit.
    let mut g = game();
    let c = 1;
    let pid = join(&mut g, c, "Titan", Species::Human, HullClass::Bastion, Profession::CombatPilot);
    {
        let p = g.store.profile_mut(pid).unwrap();
        for m in Module::ALL {
            p.ship.modules.insert(m, 9);
        }
    }
    g.spawn_player(pid, 7, false);
    let (sys, me) = ent_of(&g, c);
    let npc = g.spawn_npc(sys, NpcArchetype::Raider, 1, g.ent(sys, me).unwrap().pos + Vec3::new(0.0, 0.0, -300.0), None);
    let (hull0, shield0) = { let e = g.ent(sys, me).unwrap(); (e.hull, e.shield) };
    g.apply_damage(sys, npc, me, 1.0);
    let e = g.ent(sys, me).unwrap();
    assert!(hull0 - e.hull >= 1.0, "hull must lose at least 1 point (lost {})", hull0 - e.hull);
    assert!((shield0 - e.shield - 0.9).abs() < 1e-4, "shield takes 90% of the hit (took {})", shield0 - e.shield);
    // A big hit: 10% bleeds through, the shield takes the rest.
    let (hull1, shield1) = (e.hull, e.shield);
    g.apply_damage(sys, npc, me, 200.0);
    let e = g.ent(sys, me).unwrap();
    assert!((hull1 - e.hull - 20.0).abs() < 1e-3 && (shield1 - e.shield - 180.0).abs() < 1e-3);
}

#[test]
fn archaeology_unearth_appraise_sell_and_donate() {
    use shared::artifacts::Holder;
    let mut g = game();
    let indy = join(&mut g, 1, "Indy", Species::Human, HullClass::Seer, Profession::Archaeologist);
    let lara = join(&mut g, 2, "Lara", Species::Veyr, HullClass::Seer, Profession::Archaeologist);
    let ex = join(&mut g, 3, "Ex", Species::Human, HullClass::Wren, Profession::Explorer);
    let ruins: Vec<galaxy::Poi> = galaxy::system_layout(6).pois.into_iter().filter(|p| p.kind == PoiKind::Ruin).collect();
    for c in [1, 2, 3] {
        move_to_system(&mut g, c, 6);
    }
    clear_npcs(&mut g, 6);
    g.systems.get_mut(&6).unwrap().pirate_cd = f32::INFINITY;
    let scan = |g: &mut Game, c: ConnId, poi: &galaxy::Poi| {
        place(g, c, poi.pos + Vec3::new(0.0, 0.0, 100.0), poi.pos);
        g.handle(c, ClientMsg::Scan { poi: poi.id });
    };
    // Only the Archaeologist digs anything up; the explorer gets the usual scan reward.
    scan(&mut g, 1, &ruins[0]);
    scan(&mut g, 3, &ruins[0]);
    run(&mut g, 6.0);
    assert_eq!(g.artifacts_of(indy).len(), 1);
    assert!(g.artifacts_of(ex).is_empty());
    // One find per site per day: a second Archaeologist at the same ruin comes away empty-handed.
    scan(&mut g, 2, &ruins[0]);
    run(&mut g, 6.0);
    assert!(g.artifacts_of(lara).is_empty());
    scan(&mut g, 1, &ruins[1]);
    run(&mut g, 6.0);
    let found: Vec<u64> = g.artifacts_of(indy).iter().map(|a| a.id).collect();
    assert_eq!(found.len(), 2);
    assert_eq!(g.history.events.iter().filter(|e| e.kind == "ARTIFACT_UNEARTHED").count(), 2);

    // Station services need a dock; appraisal is free for an Archaeologist.
    g.handle(1, ClientMsg::AppraiseArtifact { id: found[0] });
    assert!(!g.store.data.artifacts[&found[0]].appraised, "not docked");
    g.spawn_player(indy, 6, true);
    let credits = |g: &Game, p| g.store.profile(p).unwrap().wallet.credits;
    let before = credits(&g, indy);
    g.handle(1, ClientMsg::AppraiseArtifact { id: found[0] });
    let a0 = g.store.data.artifacts[&found[0]].clone();
    assert!(a0.appraised && credits(&g, indy) == before);
    g.handle(1, ClientMsg::SellArtifact { id: found[0] });
    assert_eq!(credits(&g, indy), before + a0.value);
    assert_eq!(g.store.data.artifacts[&found[0]].holder, Holder::Sold(6));
    let rep = g.store.profile(indy).unwrap().reputation;
    g.handle(1, ClientMsg::DonateArtifact { id: found[1] });
    let a1 = g.store.data.artifacts[&found[1]].clone();
    assert_eq!(a1.holder, Holder::Museum(6));
    assert_eq!(g.store.profile(indy).unwrap().reputation, rep + 10);
    let donated = g.history.events.iter().find(|e| e.kind == "ARTIFACT_DONATED").expect("donation is history");
    assert!(!donated.causes.is_empty(), "linked to the excavation");
    assert!(a1.provenance.len() >= 2, "{:?}", a1.provenance);
    assert!(g.artifacts_of(indy).is_empty());
    // Selling what you don't hold fails.
    g.handle(1, ClientMsg::SellArtifact { id: found[1] });
    assert!(errors(&g, 1).iter().any(|e| e.contains("가지고 있지 않은")));

    // Anyone else pays to appraise, and an unidentified find sells cheap.
    for id in &found {
        g.store.data.artifacts.get_mut(id).unwrap().holder = Holder::Player(ex);
        g.store.data.artifacts.get_mut(id).unwrap().appraised = false;
    }
    g.spawn_player(ex, 6, true);
    let before = credits(&g, ex);
    g.handle(3, ClientMsg::AppraiseArtifact { id: found[0] });
    assert_eq!(credits(&g, ex), before - 400);
    let v1 = g.store.data.artifacts[&found[1]].value;
    g.handle(3, ClientMsg::SellArtifact { id: found[1] });
    assert_eq!(credits(&g, ex), before - 400 + (v1 as f32 * 0.4) as u64);

    // Artifacts persist with the store.
    let js = serde_json::to_string(&g.store.data).unwrap();
    let back: sim::store::StoreData = serde_json::from_str(&js).unwrap();
    assert_eq!(back.artifacts, g.store.data.artifacts);
}

#[test]
fn forged_evidence_fools_research_until_authenticated() {
    use shared::chronicle::Authenticity;
    let mut g = game();
    let a = join(&mut g, 1, "Alpha", Species::Human, HullClass::Talon, Profession::CombatPilot);
    let _b = join(&mut g, 2, "Bravo", Species::Kharz, HullClass::Bastion, Profession::CombatPilot);
    let forger = join(&mut g, 3, "Mallory", Species::Human, HullClass::Wren, Profession::Trader);
    let hist = join(&mut g, 4, "Clio", Species::Veyr, HullClass::Seer, Profession::Historian);
    // Alpha destroys Bravo in the war zone; the wreck holds the black box.
    for conn in [1, 2] {
        move_to_system(&mut g, conn, 7);
    }
    clear_npcs(&mut g, 7);
    let (_, ea) = ent_of(&g, 1);
    let (_, eb) = ent_of(&g, 2);
    for _ in 0..200 {
        g.apply_damage(7, ea, eb, 50.0);
        g.tick(DT, T0);
        if g.ent(7, eb).is_none() {
            break;
        }
    }
    let destroyed = g.history.events.iter().find(|e| e.kind == "SHIP_DESTROYED").unwrap().id;
    let black_box = g.history.evidence.iter().find(|e| e.event == destroyed && e.kind == shared::chronicle::EvidenceKind::ShipLog).unwrap().clone();
    // Mallory investigates the wreck, then forges the black box to blame Bravo.
    move_to_system(&mut g, 3, 7);
    clear_npcs(&mut g, 7);
    let (wreck_id, wreck_pos) = g.systems[&7].ents.values().find(|e| e.kind == EntityKind::Wreck).map(|e| (e.id, e.pos)).unwrap();
    place(&mut g, 3, wreck_pos + Vec3::X * 100.0, wreck_pos);
    g.handle(3, ClientMsg::InvestigateWreck { wreck: wreck_id });
    assert!(g.store.profile(forger).unwrap().archive.contains(&black_box.id));
    // Forging needs the original in hand and costs credits.
    g.out.clear();
    g.handle(4, ClientMsg::ForgeEvidence { evidence: black_box.id, text: "x".repeat(10) });
    assert!(errors(&g, 4).iter().any(|e| e.contains("가지고 있는")), "Clio does not hold it");
    let credits_before = g.store.profile(forger).unwrap().wallet.credits;
    g.handle(3, ClientMsg::ForgeEvidence { evidence: black_box.id, text: "마지막 기록: Bravo가 먼저 발포. 응사함.".into() });
    let forged = g.history.evidence.last().unwrap().clone();
    assert_ne!(forged.id, black_box.id);
    assert_eq!((forged.kind, forged.event, &forged.source, forged.reliability), (black_box.kind, black_box.event, &black_box.source, black_box.reliability), "looks exactly like the original");
    assert_eq!(forged.authenticity, Authenticity::Unverified);
    assert!(g.store.profile(forger).unwrap().wallet.credits < credits_before);
    assert!(g.store.profile(forger).unwrap().archive.contains(&forged.id));
    assert!(!g.history.biography("Mallory").iter().any(|b| b.text.contains("위조")), "forging leaves no public trace");
    // It is cited like real evidence.
    g.handle(3, ClientMsg::PublishResearch { event: destroyed, thesis: "Bravo가 먼저 쐈다".into(), cite: vec![forged.id] });
    let conf = g.history.event(destroyed).unwrap().research.iter().find(|r| r.author == "Mallory").unwrap().confidence;
    assert!(conf > 0.5, "the forgery is convincing: {conf}");
    let rep = g.store.profile(forger).unwrap().reputation;

    // A historian examines it (free for Historians): the forgery is exposed.
    let hist_credits = g.store.profile(hist).unwrap().wallet.credits;
    let hist_rep = g.store.profile(hist).unwrap().reputation;
    g.handle(4, ClientMsg::AuthenticateEvidence { evidence: forged.id });
    let ev = g.history.evidence.iter().find(|e| e.id == forged.id).unwrap();
    assert_eq!(ev.authenticity, Authenticity::Forged);
    let exposed = g.history.events.iter().find(|e| e.kind == "FORGERY_EXPOSED").expect("exposure is history");
    assert!(exposed.causes.contains(&destroyed));
    assert!(exposed.actors.contains(&"Mallory".to_string()) && exposed.actors.contains(&"Clio".to_string()));
    let r = g.history.event(destroyed).unwrap().research.iter().find(|r| r.author == "Mallory").unwrap().clone();
    assert!(r.discredited && r.confidence < conf * 0.5, "{r:?}");
    assert!(g.store.profile(forger).unwrap().reputation < rep, "the forger pays in reputation");
    assert!(g.store.profile(hist).unwrap().reputation > hist_rep);
    assert!(g.store.profile(hist).unwrap().wallet.credits > hist_credits, "a reward for exposing it");
    assert!(g.history.herald(5).iter().any(|h| h.headline.contains("위조")));
    // Examining it again is pointless; the original checks out as authentic and makes no news.
    g.out.clear();
    g.handle(4, ClientMsg::AuthenticateEvidence { evidence: forged.id });
    assert!(!errors(&g, 4).is_empty());
    let n = g.history.events.len();
    g.handle(4, ClientMsg::AuthenticateEvidence { evidence: black_box.id });
    assert_eq!(g.history.evidence.iter().find(|e| e.id == black_box.id).unwrap().authenticity, Authenticity::Authentic);
    assert_eq!(g.history.events.len(), n);
    // Anyone else pays to have evidence examined.
    let before = g.store.profile(a).unwrap().wallet.credits;
    let other = g.history.evidence.iter().find(|e| e.event == destroyed && e.authenticity == Authenticity::Unverified).unwrap().id;
    g.handle(1, ClientMsg::AuthenticateEvidence { evidence: other });
    assert!(g.store.profile(a).unwrap().wallet.credits < before);
}

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
    // Long enough for 20 units at whatever rate tuning/ships.ron sets (miner bonus included).
    let rate = g.store.profile(pid).unwrap().ship.stats().mine_rate * g.tuning.economy.miner_bonus;
    run(&mut g, 22.0 / rate);
    let mined = *g.store.profile(pid).unwrap().cargo.get("Ferrite").unwrap_or(&0);
    assert!(mined >= 20, "mined {mined} at {rate}/s");
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

#[test]
fn journalists_file_stories_that_readers_pay_for() {
    let mut g = game();
    let _a = join(&mut g, 1, "Alpha", Species::Human, HullClass::Talon, Profession::CombatPilot);
    let _b = join(&mut g, 2, "Bravo", Species::Kharz, HullClass::Bastion, Profession::CombatPilot);
    let rep = join(&mut g, 3, "Lois", Species::Human, HullClass::Wren, Profession::Journalist);
    let reader = join(&mut g, 4, "Reader", Species::Veyr, HullClass::Seer, Profession::Explorer);
    let _forger = join(&mut g, 5, "Mallory", Species::Human, HullClass::Wren, Profession::Trader);
    let _hist = join(&mut g, 6, "Clio", Species::Veyr, HullClass::Seer, Profession::Historian);
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
    let story = |headline: &str| ClientMsg::FileStory { event: destroyed, headline: headline.into(), body: "Vesta Front에서 Alpha의 Talon이 Bravo의 Bastion을 격침했다. 현장 목격담에 따르면…".into(), cite: vec![] };

    // Only journalists file, and only from the scene (in the system) or holding its evidence.
    g.out.clear();
    g.handle(4, story("독자 투고"));
    assert!(errors(&g, 4).iter().any(|e| e.contains("기자")));
    g.handle(3, story("Vesta Front의 격침"));
    assert!(errors(&g, 3).iter().any(|e| e.contains("취재")), "Lois is not on the scene");
    move_to_system(&mut g, 3, 7);
    clear_npcs(&mut g, 7);
    let before = g.store.profile(rep).unwrap().wallet.credits;
    g.out.clear();
    g.handle(3, story("Vesta Front의 격침 — 현장 르포"));
    assert!(errors(&g, 3).is_empty(), "{:?}", errors(&g, 3));
    assert!(g.store.profile(rep).unwrap().wallet.credits > before, "a filing fee");
    // The article replaces the automatic headline for that event and carries a byline.
    let herald = g.history.herald(8);
    let top = herald.iter().find(|h| h.headline.contains("현장 르포")).expect("story in the Herald");
    assert_eq!(top.by.as_deref(), Some("Lois"));
    assert!(!herald.iter().any(|h| h.by.is_none() && h.headline.contains("또 하나의 함선")), "auto headline replaced: {herald:?}");
    g.handle(3, story("두 번째 기사"));
    assert!(errors(&g, 3).iter().any(|e| e.contains("이미")), "one story per event per journalist");

    // Readers pay: each other captain reading the Herald once.
    let paid = g.store.profile(rep).unwrap().wallet.credits;
    g.handle(4, ClientMsg::RequestChronicle);
    let after_one = g.store.profile(rep).unwrap().wallet.credits;
    assert!(after_one > paid);
    g.handle(4, ClientMsg::RequestChronicle);
    assert_eq!(g.store.profile(rep).unwrap().wallet.credits, after_one, "same reader counts once");
    g.handle(3, ClientMsg::RequestChronicle);
    assert_eq!(g.store.profile(rep).unwrap().wallet.credits, after_one, "the reporter reading their own story earns nothing");
    let _ = reader;

    // A story built on forged evidence is retracted when the forgery is exposed.
    move_to_system(&mut g, 5, 7);
    clear_npcs(&mut g, 7);
    let (wreck_id, wreck_pos) = g.systems[&7].ents.values().find(|e| e.kind == EntityKind::Wreck).map(|e| (e.id, e.pos)).unwrap();
    place(&mut g, 5, wreck_pos + Vec3::X * 100.0, wreck_pos);
    g.handle(5, ClientMsg::InvestigateWreck { wreck: wreck_id });
    let log = g.store.profile(g.sessions[&5].player.unwrap()).unwrap().archive[0];
    g.handle(5, ClientMsg::ForgeEvidence { evidence: log, text: "Bravo가 먼저 쐈다".into() });
    let fake = g.history.evidence.last().unwrap().id;
    // Lois got the forged copy from Mallory and reports it.
    g.store.profile_mut(rep).unwrap().archive.push(fake);
    let event2 = g.history.events.iter().find(|e| e.kind == "BATTLE" || e.kind == "SHIP_DESTROYED").unwrap().id;
    assert_eq!(event2, destroyed);
    let rep_before = g.store.profile(rep).unwrap().reputation;
    // (one story per event: file on the same event through a second journalist)
    let rep2 = join(&mut g, 7, "Clark", Species::Human, HullClass::Wren, Profession::Journalist);
    g.store.profile_mut(rep2).unwrap().archive.push(fake);
    g.handle(7, ClientMsg::FileStory { event: destroyed, headline: "충격 — Bravo 선제 발포".into(), body: "입수한 블랙박스 사본에 따르면 Bravo가 먼저 발포했다.".into(), cite: vec![fake] });
    assert!(g.history.stories.iter().any(|s| s.reporter == "Clark" && !s.retracted));
    g.handle(6, ClientMsg::AuthenticateEvidence { evidence: fake });
    let s = g.history.stories.iter().find(|s| s.reporter == "Clark").unwrap();
    assert!(s.retracted, "story retracted");
    assert!(g.store.profile(rep2).unwrap().reputation < 0, "the reporter answers for it");
    assert_eq!(g.store.profile(rep).unwrap().reputation, rep_before, "Lois cited nothing forged");
    assert!(g.history.herald(10).iter().any(|h| h.headline.starts_with("[정정]") && h.by.as_deref() == Some("Clark")));
}

#[test]
fn contracts_escrow_delivery_expiry_and_employer_betrayal() {
    use shared::contracts::{ContractKind, ContractState};
    let mut g = game();
    let boss = join(&mut g, 1, "Boss", Species::Kharz, HullClass::Bastion, Profession::Trader);
    let carrier = join(&mut g, 2, "Carrier", Species::Human, HullClass::Mule, Profession::Miner);
    let credits = |g: &Game, p| g.store.profile(p).unwrap().wallet.credits;
    let latest = |g: &Game| *g.store.data.contracts.keys().last().unwrap();
    g.store.profile_mut(boss).unwrap().wallet.credits = 10_000;
    let delivery = || ClientMsg::PostContract { kind: ContractKind::Delivery { ore: Ore::Xenite, units: 5, system: 1 }, reward: 1000, hours: 2 };

    // Bad postings are refused and cost nothing.
    g.handle(1, ClientMsg::PostContract { kind: ContractKind::Hunt { target: "Boss".into() }, reward: 1000, hours: 2 });
    g.handle(1, ClientMsg::PostContract { kind: ContractKind::Hunt { target: "Nobody".into() }, reward: 1000, hours: 2 });
    g.handle(1, ClientMsg::PostContract { kind: ContractKind::Hunt { target: "Carrier".into() }, reward: 10, hours: 2 });
    g.handle(1, ClientMsg::PostContract { kind: ContractKind::Hunt { target: "Carrier".into() }, reward: 1000, hours: 999 });
    assert!(g.store.data.contracts.is_empty());
    assert_eq!(credits(&g, boss), 10_000);

    // Cancel before acceptance: full refund.
    g.handle(1, delivery());
    assert_eq!(credits(&g, boss), 9_000, "escrow");
    let c1 = latest(&g);
    g.handle(1, ClientMsg::AcceptContract { id: c1 });
    assert_eq!(g.store.data.contracts[&c1].state, ContractState::Open, "cannot take your own");
    g.handle(1, ClientMsg::CancelContract { id: c1 });
    assert_eq!((g.store.data.contracts[&c1].state.clone(), credits(&g, boss)), (ContractState::Cancelled, 10_000));

    // Delivery: accept, carry the goods to the destination station, get paid; goods reach the employer.
    g.handle(1, delivery());
    let c2 = latest(&g);
    g.handle(2, ClientMsg::AcceptContract { id: c2 });
    g.handle(1, ClientMsg::CancelContract { id: c2 });
    assert_eq!(g.store.data.contracts[&c2].state, ContractState::Accepted, "no withdrawing a signed contract");
    g.store.profile_mut(carrier).unwrap().cargo.insert("Xenite".into(), 7);
    g.spawn_player(carrier, 0, true);
    g.out.clear();
    g.handle(2, ClientMsg::DeliverContract { id: c2 });
    assert!(errors(&g, 2).iter().any(|e| e.contains("정거장으로")), "wrong station");
    g.spawn_player(carrier, 1, true);
    let before = credits(&g, carrier);
    g.handle(2, ClientMsg::DeliverContract { id: c2 });
    assert_eq!(g.store.data.contracts[&c2].state, ContractState::Fulfilled);
    assert_eq!(credits(&g, carrier), before + 1000);
    assert_eq!(g.store.profile(carrier).unwrap().cargo.get("Xenite"), Some(&2));
    assert_eq!(g.store.profile(boss).unwrap().cargo.get("Xenite"), Some(&5));
    let done = g.history.events.iter().find(|e| e.kind == "CONTRACT_FULFILLED").unwrap();
    assert!(!done.causes.is_empty(), "linked to the signing");
    assert!(g.history.evidence.iter().any(|e| e.kind == shared::chronicle::EvidenceKind::KharzContract && e.text.contains("Carrier")), "the signed contract is evidence");

    // A deadline that passes refunds the employer.
    g.handle(1, delivery());
    let c3 = latest(&g);
    let b = credits(&g, boss);
    g.unix += 3 * 3600;
    for _ in 0..1200 {
        g.tick(DT, g.unix);
    }
    assert_eq!(g.store.data.contracts[&c3].state, ContractState::Expired);
    assert_eq!(credits(&g, boss), b + 1000);

    // The employer turning on the contractor is a betrayal too: the escrow goes to the wronged side.
    g.handle(1, delivery());
    let c4 = latest(&g);
    g.handle(2, ClientMsg::AcceptContract { id: c4 });
    for conn in [1, 2] {
        move_to_system(&mut g, conn, 7);
    }
    clear_npcs(&mut g, 7);
    let (_, eb) = ent_of(&g, 1);
    let (_, ec) = ent_of(&g, 2);
    let before = credits(&g, carrier);
    g.apply_damage(7, eb, ec, 5.0);
    assert_eq!(g.store.data.contracts[&c4].state, ContractState::Breached { by: "Boss".into() });
    assert_eq!(credits(&g, carrier), before + 1000, "compensation");
    assert!(g.store.profile(boss).unwrap().reputation < 0);
    let betrayal = g.history.events.iter().find(|e| e.kind == "BETRAYAL").unwrap();
    assert_eq!(betrayal.actors[0], "Boss");
    // Further fire is ordinary combat, not another betrayal.
    g.apply_damage(7, ec, eb, 5.0);
    assert_eq!(g.history.events.iter().filter(|e| e.kind == "BETRAYAL").count(), 1);
}

#[test]
fn influence_takes_control_with_a_margin_and_fades() {
    let mut g = game();
    let a = join(&mut g, 1, "Yang", Species::Human, HullClass::Mule, Profession::Miner);
    let b = join(&mut g, 2, "Reinhard", Species::Kharz, HullClass::Mule, Profession::Miner);
    let loner = join(&mut g, 3, "Loner", Species::Veyr, HullClass::Mule, Profession::Miner);
    for c in [1, 2, 3] {
        g.store.profile_mut(g.sessions[&c].player.unwrap()).unwrap().wallet.credits = 5000;
    }
    g.handle(1, ClientMsg::AllianceCreate { name: "Free Planets".into(), tag: "FPA".into() });
    g.handle(2, ClientMsg::AllianceCreate { name: "Galactic Empire".into(), tag: "EMP".into() });
    let fpa = g.alliance_of(a).unwrap();
    let emp = g.alliance_of(b).unwrap();
    // Selling ore at a Frontier station earns influence (0.2 per unit); nobody without an alliance gains any.
    let sell = |g: &mut Game, conn: ConnId, pid: PlayerId, sys: u16, units: u32| {
        g.spawn_player(pid, sys, true);
        g.store.profile_mut(pid).unwrap().cargo.insert("Ferrite".into(), units);
        g.handle(conn, ClientMsg::Sell { ore: Ore::Ferrite, units });
    };
    sell(&mut g, 3, loner, 3, 100);
    assert!(g.store.data.influence.is_empty());
    // Core space belongs to no one.
    sell(&mut g, 1, a, 0, 400);
    assert_eq!(g.influence(0, fpa), 0.0);
    // FPA passes the threshold (60) and takes Orion Drift.
    sell(&mut g, 1, a, 3, 350);
    assert!((g.influence(3, fpa) - 70.0).abs() < 0.01);
    assert_eq!(g.controller(3), Some(fpa));
    assert!(g.history.events.iter().any(|e| e.kind == "SYSTEM_CONTROL"));
    // Home turf: FPA members get 10% more for the same sale, and the outsider pays 5% tax on top.
    let price = |g: &mut Game, conn: ConnId, pid: PlayerId| {
        let before = g.store.profile(pid).unwrap().wallet.credits;
        g.store.data.supply.clear();
        sell(g, conn, pid, 3, 10);
        g.store.profile(pid).unwrap().wallet.credits - before
    };
    let member = price(&mut g, 1, a);
    let outsider = price(&mut g, 3, loner);
    assert!(member > outsider && (member as f32 / outsider as f32 - 1.1 / 0.95).abs() < 0.03, "{member} vs {outsider}");
    // EMP edges ahead but not by the 20% margin: control stays.
    sell(&mut g, 2, b, 3, 400);
    let (f, e) = (g.influence(3, fpa), g.influence(3, emp));
    assert!(e > f && e <= f * 1.2, "{e} vs {f}");
    assert_eq!(g.controller(3), Some(fpa), "no flicker on a narrow lead");
    // A clear lead flips it — a Historic change of hands.
    sell(&mut g, 2, b, 3, 100);
    assert_eq!(g.controller(3), Some(emp));
    let flip = g.history.events.iter().rev().find(|e| e.kind == "SYSTEM_CONTROL").unwrap();
    assert_eq!(flip.importance, shared::chronicle::Importance::Historic);
    assert!(!flip.causes.is_empty(), "linked to the previous change of control");
    // Neglected, influence fades hour by hour until nobody holds the system.
    let hour = 3600 * 20;
    let mut lost = false;
    for _ in 0..80 {
        for _ in 0..hour / 400 {
            g.tick(DT, T0);
        }
        // fast-forward: run whole hours by jumping the tick counter to the next boundary
        g.tick = (g.tick / hour + 1) * hour - 1;
        g.tick(DT, T0);
        if g.controller(3).is_none() {
            lost = true;
            break;
        }
    }
    assert!(lost, "influence decays: {}", g.influence(3, emp));
    assert!(g.influence(3, emp) < 30.0);
}

#[test]
fn controllers_tax_outsiders_and_pay_dividends() {
    let mut g = game();
    g.dev_mode = true;
    let lead = join(&mut g, 1, "Lead", Species::Human, HullClass::Mule, Profession::Miner);
    let mate = join(&mut g, 2, "Mate", Species::Human, HullClass::Mule, Profession::Miner);
    let out = join(&mut g, 3, "Outsider", Species::Veyr, HullClass::Talon, Profession::CombatPilot);
    g.store.profile_mut(lead).unwrap().wallet.credits = 5000;
    g.handle(1, ClientMsg::AllianceCreate { name: "Tax Guild".into(), tag: "TAX".into() });
    let tax = g.alliance_of(lead).unwrap();
    g.handle(1, ClientMsg::AllianceInvite { name: "Mate".into() });
    g.handle(2, ClientMsg::AllianceAccept { alliance: tax });
    g.spawn_player(lead, 3, false);
    g.handle(1, ClientMsg::Chat { text: "/influence 80".into() });
    assert_eq!(g.controller(3), Some(tax));
    let treasury = |g: &Game| g.alliance(tax).unwrap().treasury;
    // A member selling at home pays no tax.
    let sell = |g: &mut Game, conn: ConnId, pid: PlayerId| {
        g.spawn_player(pid, 3, true);
        g.store.profile_mut(pid).unwrap().cargo.insert("Ferrite".into(), 50);
        g.store.data.supply.clear();
        let before = g.store.profile(pid).unwrap().wallet.credits;
        g.handle(conn, ClientMsg::Sell { ore: Ore::Ferrite, units: 50 });
        g.store.profile(pid).unwrap().wallet.credits - before
    };
    sell(&mut g, 2, mate);
    assert_eq!(treasury(&g), 0);
    // An outsider pays 5%.
    let got = sell(&mut g, 3, out);
    let t1 = treasury(&g);
    assert!(t1 > 0 && ((t1 as f32) / (got + t1) as f32 - 0.05).abs() < 0.01, "{t1} of {}", got + t1);
    // Bounties are taxed too.
    g.spawn_player(out, 3, false);
    clear_npcs(&mut g, 3);
    g.systems.get_mut(&3).unwrap().pirate_cd = f32::INFINITY;
    let (_, eo) = ent_of(&g, 3);
    let pos = g.ent(3, eo).unwrap().pos;
    let npc = g.spawn_npc(3, NpcArchetype::Raider, 1, pos + Vec3::new(0.0, 0.0, -300.0), None);
    for _ in 0..400 {
        g.apply_damage(3, eo, npc, 40.0);
        g.tick(DT, T0);
        if g.ent(3, npc).is_none() {
            break;
        }
    }
    assert!(treasury(&g) > t1, "bounty tax");
    // Only the leader pays out, in equal shares.
    g.handle(2, ClientMsg::AllianceDividend);
    assert_eq!(treasury(&g) > 0, true, "a member cannot");
    let (cl, cm) = (g.store.profile(lead).unwrap().wallet.credits, g.store.profile(mate).unwrap().wallet.credits);
    let pot = treasury(&g);
    g.handle(1, ClientMsg::AllianceDividend);
    let share = pot / 2;
    assert_eq!(g.store.profile(lead).unwrap().wallet.credits, cl + share);
    assert_eq!(g.store.profile(mate).unwrap().wallet.credits, cm + share);
    assert_eq!(treasury(&g), pot - share * 2);
}

#[test]
fn upkeep_is_paid_from_the_treasury_while_it_lasts() {
    let mut g = game();
    g.dev_mode = true;
    let lord = join(&mut g, 1, "Lord", Species::Human, HullClass::Talon, Profession::CombatPilot);
    g.store.profile_mut(lord).unwrap().wallet.credits = 5000;
    g.handle(1, ClientMsg::AllianceCreate { name: "Grand Empire".into(), tag: "EMP".into() });
    let emp = g.alliance_of(lord).unwrap();
    g.spawn_player(lord, 3, false);
    for sys in [3, 4] {
        g.handle(1, ClientMsg::Chat { text: format!("/influence 100 {sys}") });
    }
    assert_eq!(g.held_by(emp), vec![3, 4]);
    assert_eq!(g.upkeep(1), 0, "the first system is free");
    assert_eq!(g.upkeep(2), 150);
    assert!(g.upkeep(3) > 2 * g.upkeep(2), "upkeep grows faster than territory");
    g.store.data.alliances.iter_mut().find(|a| a.id == emp).unwrap().treasury = 400;
    let hour = 3600 * 20;
    let run_hour = |g: &mut Game| {
        g.tick = (g.tick / hour + 1) * hour - 1;
        g.tick(DT, T0);
    };
    run_hour(&mut g);
    assert_eq!(g.alliance(emp).unwrap().treasury, 250, "paid");
    run_hour(&mut g);
    assert_eq!(g.alliance(emp).unwrap().treasury, 100);
    assert_eq!(g.held_by(emp).len(), 2, "still paying");
    run_hour(&mut g);
    assert_eq!(g.held_by(emp).len(), 1, "could not pay: the weakest system is abandoned");
    assert_eq!(g.alliance(emp).unwrap().treasury, 0);
    run_hour(&mut g);
    assert_eq!(g.held_by(emp).len(), 1, "the home system costs nothing");
}

#[test]
fn institutes_one_per_captain_paid_by_daily_visitors() {
    use shared::artifacts::Holder;
    let mut g = game();
    let founder = join(&mut g, 1, "Curator", Species::Human, HullClass::Seer, Profession::Archaeologist);
    let visitor = join(&mut g, 2, "Visitor", Species::Veyr, HullClass::Wren, Profession::Explorer);
    g.store.profile_mut(founder).unwrap().wallet.credits = 20_000;
    // Found at a docked station; one per captain; names are unique.
    g.spawn_player(founder, 6, false);
    g.handle(1, ClientMsg::FoundInstitute { name: "Crown Institute".into() });
    assert!(g.store.data.institutes.is_empty(), "must be docked");
    g.spawn_player(founder, 6, true);
    g.handle(1, ClientMsg::FoundInstitute { name: "Crown Institute".into() });
    assert_eq!(g.store.data.institutes.len(), 1);
    assert_eq!(g.store.profile(founder).unwrap().wallet.credits, 15_000);
    g.out.clear();
    g.handle(1, ClientMsg::FoundInstitute { name: "Second".into() });
    assert!(errors(&g, 1).iter().any(|e| e.contains("하나만")));
    let inst = *g.store.data.institutes.keys().next().unwrap();
    // An empty institute draws no paying visitors.
    let fee_before = g.store.profile(founder).unwrap().wallet.credits;
    g.spawn_player(visitor, 6, false);
    g.handle(2, ClientMsg::Dock);
    let (s, e) = ent_of(&g, 2);
    let st = g.systems[&s].layout.station;
    g.ent_mut(s, e).unwrap().pos = st + Vec3::new(0.0, 0.0, 100.0);
    g.handle(2, ClientMsg::Dock);
    assert_eq!(g.store.profile(founder).unwrap().wallet.credits, fee_before);
    // Give the curator an artifact; donating from another system fails, here it works.
    g.store.data.next_artifact += 1;
    let aid = g.store.data.next_artifact;
    let art = shared::artifacts::excavate(aid, 6, "고대 유적 Alpha", PoiKind::Ruin, 1, 1, founder, T0);
    g.store.data.artifacts.insert(aid, art);
    g.spawn_player(founder, 3, true);
    g.out.clear();
    g.handle(1, ClientMsg::DonateToInstitute { artifact: aid, institute: inst });
    assert!(errors(&g, 1).iter().any(|e| e.contains("정거장에 있습니다")));
    g.spawn_player(founder, 6, true);
    g.handle(1, ClientMsg::DonateToInstitute { artifact: aid, institute: inst });
    assert_eq!(g.store.data.artifacts[&aid].holder, Holder::Institute(inst));
    assert_eq!(g.store.data.institutes[&inst].exhibits, vec![aid]);
    // Visits: once per captain per day, the founder's own docking doesn't count.
    // (Undocking pushes the ship out; fly it back next to the station before docking again.)
    let dock_again = |g: &mut Game| {
        g.handle(2, ClientMsg::Undock);
        let (s, e) = ent_of(g, 2);
        let st = g.systems[&s].layout.station;
        g.ent_mut(s, e).unwrap().pos = st + Vec3::new(0.0, 0.0, 100.0);
        g.handle(2, ClientMsg::Dock);
    };
    let before = g.store.profile(founder).unwrap().wallet.credits;
    dock_again(&mut g);
    dock_again(&mut g);
    g.handle(1, ClientMsg::Undock);
    g.handle(1, ClientMsg::Dock);
    assert_eq!(g.store.data.institutes[&inst].visitors, 1);
    let fee = g.tuning.social.visit_base + g.tuning.social.visit_per_exhibit;
    assert_eq!(g.store.profile(founder).unwrap().wallet.credits, before + fee);
    g.unix += 86_400;
    dock_again(&mut g);
    assert_eq!(g.store.data.institutes[&inst].visitors, 2, "a new day, a new visit");
    // The story is in the chronicle and survives a save.
    assert!(g.history.events.iter().any(|e| e.kind == "INSTITUTE_FOUNDED"));
    let js = serde_json::to_string(&g.store.data).unwrap();
    let back: sim::store::StoreData = serde_json::from_str(&js).unwrap();
    assert_eq!(back.institutes, g.store.data.institutes);
}

#[test]
fn leaks_unseal_records_and_first_story_is_the_scoop() {
    let mut g = game();
    let _a = join(&mut g, 1, "Alpha", Species::Human, HullClass::Talon, Profession::CombatPilot);
    let _b = join(&mut g, 2, "Bravo", Species::Kharz, HullClass::Bastion, Profession::CombatPilot);
    let lois = join(&mut g, 3, "Lois", Species::Human, HullClass::Wren, Profession::Journalist);
    let clark = join(&mut g, 4, "Clark", Species::Human, HullClass::Wren, Profession::Journalist);
    let outsider = join(&mut g, 5, "Outsider", Species::Veyr, HullClass::Wren, Profession::Explorer);
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
    let log = g.history.evidence.iter().find(|e| e.event == destroyed && e.kind == shared::chronicle::EvidenceKind::ShipLog).unwrap().id;
    let events = g.history.chronicle(shared::chronicle::Importance::Trivial, 50);
    let sealed = |g: &Game| g.history.visible_evidence(&events, &[]).into_iter().find(|e| e.id == log).unwrap().text.starts_with("[봉인됨");
    assert!(sealed(&g), "nobody has opened the black box yet");
    // Lois holds it (as if from the wreck) and leaks it: now an outsider can read it.
    g.store.profile_mut(lois).unwrap().archive.push(log);
    g.out.clear();
    g.handle(5, ClientMsg::LeakEvidence { evidence: log });
    assert!(errors(&g, 5).iter().any(|e| e.contains("가지고 있는")), "only a holder can leak");
    g.handle(3, ClientMsg::LeakEvidence { evidence: log });
    assert!(!sealed(&g));
    assert!(g.history.events.iter().any(|e| e.kind == "LEAK" && e.causes.contains(&destroyed)));
    g.out.clear();
    g.handle(3, ClientMsg::LeakEvidence { evidence: log });
    assert!(errors(&g, 3).iter().any(|e| e.contains("이미 공개")));
    let _ = outsider;
    // Scoop: the first article on an event earns [단독] and a bonus; the second does not.
    let story = |head: &str| ClientMsg::FileStory { event: destroyed, headline: head.into(), body: "공개된 블랙박스에 따르면 마지막 교신은 이랬다.".into(), cite: vec![log] };
    let pay = |g: &mut Game, conn: ConnId, pid: PlayerId, head: &str| {
        let before = g.store.profile(pid).unwrap().wallet.credits;
        g.handle(conn, story(head));
        g.store.profile(pid).unwrap().wallet.credits - before
    };
    let first = pay(&mut g, 3, lois, "첫 보도");
    g.store.profile_mut(clark).unwrap().archive.push(log);
    let second = pay(&mut g, 4, clark, "후속 보도");
    assert_eq!(first, second + g.tuning.history.scoop_bonus, "{first} vs {second}");
    let herald = g.history.herald(10);
    assert!(herald.iter().any(|h| h.headline == "[단독] 첫 보도"));
    assert!(herald.iter().any(|h| h.headline == "후속 보도"));
}

#[test]
fn withdrawal_news_dividend_records_and_no_seizure_between_allies() {
    let mut g = game();
    g.dev_mode = true;
    let lord = join(&mut g, 1, "Lord", Species::Human, HullClass::Bastion, Profession::CombatPilot);
    let ally = join(&mut g, 2, "Ally", Species::Human, HullClass::Wren, Profession::CombatPilot);
    g.store.profile_mut(lord).unwrap().wallet.credits = 5000;
    g.handle(1, ClientMsg::AllianceCreate { name: "Grand Empire".into(), tag: "EMP".into() });
    let emp = g.alliance_of(lord).unwrap();
    g.handle(1, ClientMsg::AllianceInvite { name: "Ally".into() });
    g.handle(2, ClientMsg::AllianceAccept { alliance: emp });
    // Two systems, an empty treasury: the first hourly upkeep forces a withdrawal, reported as such.
    g.spawn_player(lord, 3, false);
    for sys in [3, 4] {
        g.handle(1, ClientMsg::Chat { text: format!("/influence 100 {sys}") });
    }
    let hour = 3600 * 20;
    g.tick = (g.tick / hour + 1) * hour - 1;
    g.tick(DT, T0);
    let w = g.history.events.iter().rev().find(|e| e.kind == "SYSTEM_CONTROL").unwrap();
    assert!(w.title.contains("철수") && w.title.contains("금고 부족"), "{}", w.title);
    // Dividends go into the members' biographies.
    g.store.data.alliances.iter_mut().find(|a| a.id == emp).unwrap().treasury = 1000;
    g.handle(1, ClientMsg::AllianceDividend);
    assert!(g.history.biography("Ally").iter().any(|b| b.text.contains("배당 500 CR")));
    // Allies cannot launder records by shooting each other down.
    g.store.profile_mut(ally).unwrap().archive.extend([1, 2]);
    for c in [1, 2] {
        move_to_system(&mut g, c, 7);
    }
    clear_npcs(&mut g, 7);
    let (_, el) = ent_of(&g, 1);
    let (_, ea) = ent_of(&g, 2);
    let before = g.store.profile(lord).unwrap().archive.len();
    // (Alliance members cannot damage each other; drop the ally's hull directly to make the kill.)
    g.ent_mut(7, ea).unwrap().hull = 0.0;
    let now = g.now;
    g.ent_mut(7, ea).unwrap().attackers.push((el, Some(lord), now));
    g.tick(DT, T0);
    assert!(g.sessions[&2].dead);
    assert_eq!(g.store.profile(lord).unwrap().archive.len(), before, "no seizure between allies");
    assert!(!g.history.events.iter().any(|e| e.kind == "INTEL_SEIZED"));
}

#[test]
fn escorts_do_not_count_as_pirates() {
    // A capital ship's escorts carry an AI too; they must not fill the pirate quota of a system.
    let mut g = game();
    let lord = join(&mut g, 1, "Lord", Species::Human, HullClass::Bastion, Profession::CombatPilot);
    for m in Module::ALL {
        g.store.profile_mut(lord).unwrap().ship.modules.insert(m, 9);
    }
    g.spawn_player(lord, 3, false);
    g.spawn_escorts(lord);
    clear_npcs(&mut g, 3);
    let escorts = g.systems[&3].ents.values().filter(|e| e.kind == EntityKind::Escort).count();
    assert!(escorts >= 3, "capital has escorts: {escorts}");
    run(&mut g, 60.0);
    let pirates = g.systems[&3].ents.values().filter(|e| matches!(e.kind, EntityKind::Npc(_))).count();
    assert!(pirates > 0, "pirates still show up around a big fleet");
}

/// 3D step 3: turret batteries have firing arcs, alternating dorsal and ventral (tuning
/// `combat.ron` turret_arc_overlap). A target level with a capital ship draws every battery; one
/// straight above or below draws only that half — so attackers dive, and captains roll.
#[test]
fn turret_arcs_reward_attacking_from_above_or_below() {
    let hits = |offset: Vec3| {
        let mut g = game();
        let pid = join(&mut g, 1, "Admiral", Species::Human, HullClass::Bastion, Profession::Explorer);
        for m in Module::ALL {
            g.store.profile_mut(pid).unwrap().ship.modules.insert(m, 9);
        }
        move_to_system(&mut g, 1, 5);
        clear_npcs(&mut g, 5);
        let me = Vec3::new(0.0, 0.0, 6000.0);
        place(&mut g, 1, me, me + Vec3::NEG_Z * 100.0);
        let (sys, eid) = ent_of(&g, 1);
        assert!(g.ent(sys, eid).unwrap().turret_cds.len() >= 8, "a capital ship with turret batteries");
        let target = g.spawn_npc(sys, NpcArchetype::Sentinel, 1, me + offset, None);
        g.ent_mut(sys, target).unwrap().ai = None;
        g.handle(1, ClientMsg::SetTarget(Some(target)));
        g.metrics = Some(vec![]);
        run(&mut g, 10.0);
        g.metrics.unwrap().iter().filter(|m| matches!(m, sim::metrics::Metric::Hit { from, to, .. } if *from == eid && *to == target)).count()
    };
    let (level, above, below) = (hits(Vec3::X * 500.0), hits(Vec3::Y * 500.0), hits(Vec3::NEG_Y * 500.0));
    assert!(level > 20, "level target draws fire: {level}");
    for (what, n) in [("above", above), ("below", below)] {
        let share = n as f32 / level as f32;
        assert!((0.35..0.65).contains(&share), "a target straight {what} draws only half the batteries: {n} vs {level} level");
    }
}

/// Round B: a battery that cannot reach the main target engages the nearest hostile on its own side,
/// so a capital ship fights above and below at once instead of idling half its guns.
#[test]
fn turret_halves_engage_their_own_side() {
    let mut g = game();
    let pid = join(&mut g, 1, "Admiral", Species::Human, HullClass::Bastion, Profession::Explorer);
    for m in Module::ALL {
        g.store.profile_mut(pid).unwrap().ship.modules.insert(m, 9);
    }
    move_to_system(&mut g, 1, 5);
    clear_npcs(&mut g, 5);
    let me = Vec3::new(0.0, 0.0, 6000.0);
    place(&mut g, 1, me, me + Vec3::NEG_Z * 100.0);
    let (sys, eid) = ent_of(&g, 1);
    let above = g.spawn_npc(sys, NpcArchetype::Sentinel, 1, me + Vec3::Y * 500.0, None);
    let below = g.spawn_npc(sys, NpcArchetype::Sentinel, 1, me + Vec3::NEG_Y * 500.0, None);
    for t in [above, below] {
        g.ent_mut(sys, t).unwrap().ai = None;
    }
    g.handle(1, ClientMsg::SetTarget(Some(above)));
    // Both hostiles are shooting at us, so both count as engaged.
    g.ent_mut(sys, eid).unwrap().attackers = vec![(above, None, 0.0), (below, None, 0.0)];
    g.metrics = Some(vec![]);
    run(&mut g, 10.0);
    let hits = |t: EntityId| g.metrics.as_ref().unwrap().iter().filter(|m| matches!(m, sim::metrics::Metric::Hit { from, to, .. } if *from == eid && *to == t)).count();
    let (a, b) = (hits(above), hits(below));
    assert!(a > 20 && b > 20, "dorsal half on the target above ({a}), ventral half on the one below ({b})");
    assert!((a as f32 / b as f32 - 1.0).abs() < 0.35, "the halves share the work: {a} vs {b}");
}

/// The client's target panel counts an enemy's turret batteries and how many reach me from its
/// public state alone (`shared::ships::turret_batteries` / `batteries_reaching`). Both must match
/// what the server actually fires with.
#[test]
fn target_panel_turret_count_matches_the_server() {
    let mut g = game();
    let mut conn = 1;
    for h in HullClass::ALL {
        for lv in [1u8, 5, 9] {
            let pid = join(&mut g, conn, &format!("C{conn}"), Species::Human, h, Profession::Explorer);
            for m in Module::ALL {
                g.store.profile_mut(pid).unwrap().ship.modules.insert(m, lv);
            }
            move_to_system(&mut g, conn, 5);
            let (sys, eid) = ent_of(&g, conn);
            let e = g.ent(sys, eid).unwrap();
            assert_eq!(shared::ships::turret_batteries(&e.state()) as usize, e.turret_cds.len(), "{h:?} Lv{lv}");
            conn += 1;
        }
    }
    for a in NpcArchetype::ALL {
        let id = g.spawn_npc(5, a, 3, Vec3::new(0.0, 0.0, 9000.0), None);
        let e = g.ent(5, id).unwrap();
        let st = e.state();
        assert_eq!(shared::ships::turret_batteries(&st) as usize, e.turret_cds.len(), "{a:?}");
        // Arcs: level with it every battery reaches; straight above only the dorsal half.
        let n = e.turret_cds.len() as u8;
        let p = Vec3::from(st.pos);
        let rot = Quat::from_array(st.rot);
        assert_eq!(shared::ships::batteries_reaching(&st, p + rot * Vec3::X * 500.0), n, "{a:?} level");
        assert_eq!(shared::ships::batteries_reaching(&st, p + rot * Vec3::Y * 500.0), n.div_ceil(2), "{a:?} above");
    }
}

#[test]
fn dev_teleport_poses_two_captains_a_height_apart() {
    let mut g = game();
    g.dev_mode = true;
    join(&mut g, 1, "Low", Species::Human, HullClass::Bastion, Profession::Explorer);
    join(&mut g, 2, "High", Species::Human, HullClass::Bastion, Profession::Explorer);
    g.handle(1, ClientMsg::Chat { text: "/tp station".into() });
    g.handle(2, ClientMsg::Chat { text: "/tp station 0 400 0".into() });
    let pos = |g: &Game, c| {
        let (s, e) = ent_of(g, c);
        let e = g.ent(s, e).unwrap();
        assert!(!e.docked);
        e.pos
    };
    let d = pos(&g, 2) - pos(&g, 1);
    assert!((d - Vec3::new(0.0, 400.0, 0.0)).length() < 1e-3, "{d}");
}

/// Holding Q/E for `secs` then letting go: how many of A's batteries reach B (directly above) once it settles.
fn reach_after_roll(secs: f32) -> (u8, u8) {
    let mut g = game();
    join(&mut g, 1, "Low", Species::Human, HullClass::Bastion, Profession::CombatPilot);
    join(&mut g, 2, "High", Species::Human, HullClass::Bastion, Profession::CombatPilot);
    for c in [1, 2] {
        grow(&mut g, c, 9, 3);
    }
    clear_npcs(&mut g, 3);
    let at = galaxy::system_layout(3).station + Vec3::new(0.0, 0.0, 3000.0);
    place(&mut g, 1, at, at + Vec3::NEG_Z);
    place(&mut g, 2, at + Vec3::Y * 1200.0, at + Vec3::NEG_Z);
    g.handle(1, ClientMsg::Controls(Controls { roll: 1.0, ..Default::default() }));
    run(&mut g, secs);
    g.handle(1, ClientMsg::Controls(Controls::default()));
    run(&mut g, 3.0);
    let (s, a) = ent_of(&g, 1);
    let (_, b) = ent_of(&g, 2);
    let me = g.ent(s, a).unwrap().state();
    (shared::ships::batteries_reaching(&me, g.ent(s, b).unwrap().pos), shared::ships::turret_batteries(&me))
}

#[test]
fn rolling_puts_an_overhead_ship_in_every_battery() {
    // The target panel's roll hint (docs/3D_전투_설계.md) promises 14/14 once the enemy is beside the
    // hull. A Lv9 Bastion rolls 0.51 rad/s: about 3 s of Q/E turns 90°; held longer it overshoots and
    // the enemy is under the hull — the hint comes back, so the captain lets go when it disappears.
    assert_eq!(reach_after_roll(0.0), (7, 14), "directly above: dorsal batteries only");
    assert_eq!(reach_after_roll(3.0), (14, 14), "rolled level with it: every battery");
    assert_eq!(reach_after_roll(3.5), (14, 14));
    assert_eq!(reach_after_roll(4.5), (7, 14), "overshot: ventral batteries only");
}

/// Whether my turrets shoot a captain beside me while I hold the trigger on a pirate overhead.
fn stray_turret_fire(same_fleet: bool) -> bool {
    let mut g = game();
    join(&mut g, 1, "Me", Species::Human, HullClass::Bastion, Profession::CombatPilot);
    join(&mut g, 2, "Mate", Species::Human, HullClass::Bastion, Profession::CombatPilot);
    for c in [1, 2] {
        grow(&mut g, c, 9, 5);
    }
    if same_fleet {
        g.handle(1, ClientMsg::FleetInvite { name: "Mate".into() });
        let fleet = g.fleet_of(g.sessions[&1].player.unwrap()).unwrap();
        g.handle(2, ClientMsg::FleetAccept { fleet });
    }
    clear_npcs(&mut g, 5);
    let at = galaxy::system_layout(5).station + Vec3::new(0.0, 0.0, 4000.0);
    place(&mut g, 1, at, at + Vec3::NEG_Z);
    place(&mut g, 2, at + Vec3::X * 700.0, at + Vec3::X * 700.0 + Vec3::NEG_Z);
    let pirate = g.spawn_npc(5, NpcArchetype::Raider, 1, at + Vec3::Y * 900.0, None);
    let (s, me) = ent_of(&g, 1);
    let (_, mate) = ent_of(&g, 2);
    g.ent_mut(s, me).unwrap().target = Some(pirate);
    g.out.clear();
    g.handle(1, ClientMsg::Controls(Controls { fire: true, ..Default::default() }));
    run(&mut g, 4.0);
    g.out.iter().any(|(_, m)| matches!(m, ServerMsg::Snapshot { fx, .. } if fx.iter().any(|f| matches!(f, Fx::Shot { from, to, .. } if *from == me && *to == mate))))
}

#[test]
fn turrets_shoot_captains_outside_the_fleet() {
    // The target panel's warning (view::hud::stray_fire_warning): with the pirate overhead, my ventral
    // half has nothing to shoot but the captain beside me — unless we fly as one fleet.
    assert!(stray_turret_fire(false), "outside a fleet the spare batteries take the other captain");
    assert!(!stray_turret_fire(true), "fleet mates are never picked");
}

/// Low selects Pir (another captain, directly in turret reach, open-PvP space) and then holds `fire`
/// or not; Pir shoots Low first when `pir_attacks`. Returns (Low's turrets hit Pir, Low's bounty).
fn select_captain(fire: bool, pir_attacks: bool) -> (bool, u64) {
    let mut g = game();
    let low = join(&mut g, 1, "Low", Species::Human, HullClass::Bastion, Profession::CombatPilot);
    join(&mut g, 2, "Pir", Species::Human, HullClass::Bastion, Profession::CombatPilot);
    for c in [1, 2] {
        grow(&mut g, c, 9, 5);
    }
    clear_npcs(&mut g, 5);
    let at = galaxy::system_layout(5).station + Vec3::new(0.0, 0.0, 4000.0);
    place(&mut g, 1, at, at + Vec3::NEG_Z);
    place(&mut g, 2, at + Vec3::X * 900.0, at + Vec3::X * 900.0 + Vec3::NEG_Z);
    let (s, me) = ent_of(&g, 1);
    let (_, pir) = ent_of(&g, 2);
    g.handle(1, ClientMsg::SetTarget(Some(pir)));
    if pir_attacks {
        g.handle(2, ClientMsg::SetTarget(Some(me)));
        g.handle(2, ClientMsg::Controls(Controls { fire: true, ..Default::default() }));
    }
    g.handle(1, ClientMsg::Controls(Controls { fire, ..Default::default() }));
    g.out.clear();
    run(&mut g, 5.0);
    let _ = s;
    let hit = g.out.iter().any(|(_, m)| matches!(m, ServerMsg::Snapshot { fx, .. } if fx.iter().any(|f| matches!(f, Fx::Shot { from, to, .. } if *from == me && *to == pir))));
    (hit, g.store.profile(low).unwrap().bounty)
}

#[test]
fn selecting_a_captain_is_not_an_attack() {
    // Picking a captain (T, or to read the target panel) must not open fire: turrets take a selected
    // captain only with the trigger held or in self-defence, like any other captain in reach.
    let (hit, bounty) = select_captain(false, false);
    assert!(!hit && bounty == 0, "selected only: no shots, no crime (hit {hit}, bounty {bounty})");
    assert!(select_captain(true, false).0, "trigger held: the turrets take the selected captain");
    let (hit, bounty) = select_captain(false, true);
    assert!(hit, "shot at first: the turrets answer the attacker without the trigger");
    assert_eq!(bounty, 0, "self-defence is no crime");
}

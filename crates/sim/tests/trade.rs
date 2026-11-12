//! The open galaxy's work and trade, against the authoritative `Game`: station job boards, the goods
//! market, customs, titles (`shared::jobs`, `shared::market`, `shared::titles`, sim/src/game/trade.rs).

use shared::galaxy::{self, Species};
use shared::glam::Vec3;
use shared::jobs::{self, Job, JobKind, JobTag};
use shared::market::{self, Good};
use shared::protocol::*;
use shared::protocol::EntityId;
use shared::ships::HullClass;
use shared::titles::Title;
use sim::game::{ConnId, Game};
use sim::store::Store;

const DT: f32 = 0.05;
const T0: u64 = 1_800_000_000;

fn game() -> Game {
    Game::new(Store::memory(), history::HistoryEngine::new(), T0)
}

fn join(g: &mut Game, conn: ConnId, name: &str, hull: HullClass) -> u64 {
    g.handle(conn, ClientMsg::Hello { version: PROTOCOL_VERSION, name: name.into(), token: None });
    g.handle(conn, ClientMsg::CreateCharacter { species: Species::Human, profession: Profession::CombatPilot, hull, ship_name: format!("{name}-ship"), livery: 1 });
    g.sessions[&conn].player.unwrap()
}

/// Arrive at `sys` and dock at its station the way a pilot would (the dock hooks run).
fn dock_at(g: &mut Game, conn: ConnId, sys: u16) {
    let pid = g.sessions[&conn].player.unwrap();
    g.spawn_player(pid, sys, false);
    g.systems.get_mut(&sys).unwrap().ents.retain(|_, e| !matches!(e.kind, EntityKind::Npc(_)));
    let (s, e) = (g.sessions[&conn].system, g.sessions[&conn].entity.unwrap());
    let station = g.systems[&sys].layout.station;
    let ent = g.ent_mut(s, e).unwrap();
    ent.pos = station + Vec3::Z * 150.0;
    ent.vel = Vec3::ZERO;
    ent.last_hit = -100.0;
    g.handle(conn, ClientMsg::Dock);
    assert!(g.ent(s, e).unwrap().docked, "docked at {}", galaxy::system(sys).name);
}

fn board(g: &Game, sys: u16) -> Vec<Job> {
    jobs::board(&shared::tuning::get(), sys, jobs::period(&shared::tuning::get(), g.unix))
}

/// The first job of this kind on any station's board, with its station.
fn find_job(g: &Game, kind: JobTag) -> Job {
    (0..galaxy::systems().len() as u16).flat_map(|s| board(g, s)).find(|j| j.kind.tag() == kind).unwrap_or_else(|| panic!("no {kind:?} anywhere"))
}

fn toasts(g: &Game, conn: ConnId) -> Vec<String> {
    g.out.iter().filter(|(c, _)| *c == conn).filter_map(|(_, m)| if let ServerMsg::Toast { text } = m { Some(text.clone()) } else { None }).collect()
}

fn credits(g: &Game, pid: u64) -> u64 {
    g.store.profile(pid).unwrap().wallet.credits
}

#[test]
fn docking_shows_the_station_board_and_market() {
    let mut g = game();
    join(&mut g, 1, "Arin", HullClass::Talon);
    g.out.clear();
    dock_at(&mut g, 1, 3);
    let got = g.out.iter().find_map(|(c, m)| if let (1, ServerMsg::StationBoard { system, jobs, market, next_board }) = (*c, m) { Some((*system, jobs.clone(), market.clone(), *next_board)) } else { None });
    let (system, jobs, quotes, next) = got.expect("a station board on docking");
    assert_eq!(system, 3);
    assert_eq!(jobs, board(&g, 3));
    assert_eq!(quotes.len(), Good::ALL.len());
    assert!(quotes.iter().any(|q| q.buy.is_some()), "the station sells something");
    assert!(next > g.unix);
}

#[test]
fn a_haul_rides_in_the_hold_and_pays_at_the_destination_or_goes_down_with_the_ship() {
    let mut g = game();
    let pid = join(&mut g, 1, "Hauler", HullClass::Mule);
    let job = find_job(&g, JobTag::Haul);
    let JobKind::Haul { to, units } = job.kind else { unreachable!() };
    dock_at(&mut g, 1, job.station);
    g.handle(1, ClientMsg::AcceptJob { id: job.id });
    let p = g.store.profile(pid).unwrap();
    assert_eq!(p.jobs.len(), 1);
    assert_eq!(p.cargo.get(jobs::SEALED), Some(&units), "sealed cargo loaded");
    // Taking the same job twice is refused.
    g.handle(1, ClientMsg::AcceptJob { id: job.id });
    assert_eq!(g.store.profile(pid).unwrap().jobs.len(), 1);
    let before = credits(&g, pid);
    dock_at(&mut g, 1, to);
    let p = g.store.profile(pid).unwrap();
    assert!(p.jobs.is_empty(), "delivered");
    assert_eq!(p.cargo.get(jobs::SEALED), None);
    assert_eq!(credits(&g, pid), before + job.reward.credits);
    assert_eq!(p.career.jobs_done(JobTag::Haul), 1);

    // The same haul again, but the ship is lost on the way: the cargo and the job go with it.
    dock_at(&mut g, 1, job.station);
    g.handle(1, ClientMsg::AcceptJob { id: job.id });
    assert_eq!(g.store.profile(pid).unwrap().jobs.len(), 1);
    g.spawn_player(pid, to, false);
    let (s, e) = (g.sessions[&1].system, g.sessions[&1].entity.unwrap());
    let rep = g.store.profile(pid).unwrap().reputation;
    let raider = g.spawn_npc(s, shared::missions::NpcArchetype::Raider, 0, g.ent(s, e).unwrap().pos + Vec3::Z * 200.0, None);
    let hp = g.ent(s, e).map(|x| x.hull + x.shield).unwrap();
    g.apply_damage(s, raider, e, hp + 1.0);
    g.tick(DT, T0);
    let p = g.store.profile(pid).unwrap();
    assert!(p.jobs.is_empty() && !p.cargo.contains_key(jobs::SEALED), "lost with the ship");
    assert_eq!(p.reputation, rep - 2);
}

#[test]
fn a_named_bounty_waits_in_its_system_and_pays_when_it_and_its_escort_are_sunk() {
    let mut g = game();
    let pid = join(&mut g, 1, "Hunter", HullClass::Bastion);
    let job = find_job(&g, JobTag::Bounty);
    let JobKind::Bounty { system, target, escorts, .. } = job.kind.clone() else { unreachable!() };
    dock_at(&mut g, 1, job.station);
    g.handle(1, ClientMsg::AcceptJob { id: job.id });
    g.out.clear();
    // Arrive in the target's system (somewhere else first, if the station is in it).
    if system == job.station {
        g.spawn_player(pid, (system + 1) % galaxy::systems().len() as u16, false);
    }
    g.spawn_player(pid, system, false);
    let ids: Vec<EntityId> = g.sessions[&1].job_waves.get(&job.id).expect("the target came out").1.clone();
    assert_eq!(ids.len() as u32, 1 + escorts);
    assert!(g.ent(system, ids[0]).unwrap().name.contains(&target), "the target carries its name");
    assert!(toasts(&g, 1).iter().any(|t| t.contains(&target)));
    // Leaving lets them go; they are back on return.
    g.spawn_player(pid, (system + 1) % galaxy::systems().len() as u16, false);
    assert!(g.ent(system, ids[0]).is_none());
    g.spawn_player(pid, system, false);
    let ids: Vec<EntityId> = g.sessions[&1].job_waves[&job.id].1.clone();
    let before = credits(&g, pid);
    let me = g.sessions[&1].entity.unwrap();
    for (k, w) in ids.iter().enumerate() {
        let hp = g.ent(system, *w).map(|e| e.hull + e.shield).unwrap();
        g.apply_damage(system, me, *w, hp + 1.0);
        g.tick(DT, T0);
        assert_eq!(g.store.profile(pid).unwrap().jobs.is_empty(), k + 1 == ids.len(), "done only when all are sunk");
    }
    let p = g.store.profile(pid).unwrap();
    assert!(credits(&g, pid) >= before + job.reward.credits, "reward on top of the bounties");
    assert_eq!(p.career.jobs_done(JobTag::Bounty), 1);
    assert!(g.sessions[&1].job_waves.is_empty());
}

#[test]
fn an_armada_is_a_fleet_battle_paid_when_the_last_ship_falls() {
    let mut g = game();
    let pid = join(&mut g, 1, "Shield", HullClass::Bastion);
    // A full-size one (armadas come in sizes, `armada_scale`).
    let job = (0..galaxy::systems().len() as u16).flat_map(|s| board(&g, s)).find(|j| matches!(&j.kind, JobKind::Armada { wave, .. } if wave.iter().map(|w| w.1).sum::<u32>() >= 6)).expect("a full armada");
    let JobKind::Armada { system, wave, .. } = job.kind.clone() else { unreachable!() };
    dock_at(&mut g, 1, job.station);
    g.handle(1, ClientMsg::AcceptJob { id: job.id });
    if system != job.station {
        g.spawn_player(pid, system, false);
    } else {
        // Accepting at a station inside the system starts it at once (on undock the fleet is waiting).
        g.spawn_player(pid, (system + 1) % galaxy::systems().len() as u16, false);
        g.spawn_player(pid, system, false);
    }
    let ids: Vec<EntityId> = g.sessions[&1].job_waves[&job.id].1.clone();
    assert_eq!(ids.len() as u32, wave.iter().map(|w| w.1).sum::<u32>());
    assert!(ids.len() >= 6, "a fleet, not a skirmish");
    // Spread wide, not stacked.
    let pos: Vec<Vec3> = ids.iter().map(|i| g.ent(system, *i).unwrap().pos).collect();
    let span = pos.iter().map(|p| p.distance(pos[0])).fold(0.0f32, f32::max);
    assert!(span > 300.0, "{span}");
    let me = g.sessions[&1].entity.unwrap();
    for w in &ids {
        let hp = g.ent(system, *w).map(|e| e.hull + e.shield).unwrap();
        g.apply_damage(system, me, *w, hp + 1.0);
    }
    g.tick(DT, T0);
    let p = g.store.profile(pid).unwrap();
    assert!(p.jobs.is_empty());
    assert_eq!(p.career.jobs_done(JobTag::Armada), 1);
}

#[test]
fn a_survey_completes_when_every_system_has_been_visited_in_any_order() {
    let mut g = game();
    let pid = join(&mut g, 1, "Scout", HullClass::Seer);
    let job = find_job(&g, JobTag::Survey);
    let JobKind::Survey { systems } = job.kind.clone() else { unreachable!() };
    dock_at(&mut g, 1, job.station);
    g.handle(1, ClientMsg::AcceptJob { id: job.id });
    for (k, s) in systems.iter().rev().enumerate() {
        g.spawn_player(pid, *s, false);
        let p = g.store.profile(pid).unwrap();
        assert_eq!(p.jobs.is_empty(), k + 1 == systems.len());
    }
    let p = g.store.profile(pid).unwrap();
    assert_eq!(p.career.jobs_done(JobTag::Survey), 1);
    assert!(systems.iter().all(|s| p.career.visited.contains(s)));
}

#[test]
fn trading_moves_prices_and_a_route_turns_a_profit() {
    let mut g = game();
    let pid = join(&mut g, 1, "Trader", HullClass::Mule);
    let t = shared::tuning::get();
    let (good, to, margin) = market::best_route(&t.trade.market, 0, 2.5).expect("a route from Sol Haven");
    assert!(margin > 0.0);
    dock_at(&mut g, 1, 0);
    g.store.profile_mut(pid).unwrap().wallet.credits = 50_000;
    let unit0 = market::buy_price(&t.trade.market, 0, good, g.market_offset(0, good)).unwrap();
    g.handle(1, ClientMsg::BuyGood { good, units: 60 });
    let held = g.store.profile(pid).unwrap().cargo.get(good.key()).copied().unwrap_or(0);
    assert_eq!(held, 60);
    let unit1 = market::buy_price(&t.trade.market, 0, good, g.market_offset(0, good)).unwrap();
    assert!(unit1 > unit0, "buying moved the price: {unit0} -> {unit1}");
    let after_buy = credits(&g, pid);
    dock_at(&mut g, 1, to);
    g.handle(1, ClientMsg::SellGood { good, units: held });
    let p = g.store.profile(pid).unwrap();
    assert_eq!(p.cargo.get(good.key()), None);
    assert!(credits(&g, pid) > 50_000, "the run paid: {} -> {}", after_buy, credits(&g, pid));
    assert!(p.career.trade_profit > 0);
    // Over time the station's price drifts back.
    let moved = g.market_offset(0, good);
    g.unix += t.trade.market.half_life as u64;
    assert!((g.market_offset(0, good) - moved / 2.0).abs() < 0.5);
}

#[test]
fn contraband_sells_at_the_edge_and_is_seized_in_the_core() {
    let mut g = game();
    let pid = join(&mut g, 1, "Shade", HullClass::Mule);
    let t = shared::tuning::get();
    let m = &t.trade.market;
    let stims = m.contraband[0];
    let black = galaxy::systems().iter().find(|s| m.black_market.contains(&s.tier)).unwrap().id;
    dock_at(&mut g, 1, black);
    g.store.profile_mut(pid).unwrap().wallet.credits = 20_000;
    g.handle(1, ClientMsg::BuyGood { good: stims, units: 20 });
    assert_eq!(g.store.profile(pid).unwrap().cargo.get(stims.key()), Some(&20));
    // Sell half where it is wanted …
    let fence = galaxy::systems().iter().find(|s| !m.seize_in.contains(&s.tier) && market::economy(m, s.id).demand.contains(&stims)).unwrap().id;
    dock_at(&mut g, 1, fence);
    g.handle(1, ClientMsg::SellGood { good: stims, units: 10 });
    assert_eq!(g.store.profile(pid).unwrap().career.smuggled, 10);
    // … and get caught with the rest in the core.
    let rep = g.store.profile(pid).unwrap().reputation;
    let before = credits(&g, pid);
    g.out.clear();
    dock_at(&mut g, 1, 0);
    let p = g.store.profile(pid).unwrap();
    assert_eq!(p.cargo.get(stims.key()), None, "seized");
    assert!(credits(&g, pid) < before, "fined");
    assert_eq!(p.reputation, rep - m.fine_reputation);
    assert!(toasts(&g, 1).iter().any(|t| t.contains("세관")));
    g.handle(1, ClientMsg::BuyGood { good: stims, units: 1 });
    assert_eq!(g.store.profile(pid).unwrap().cargo.get(stims.key()), None, "the core never sells it");
}

#[test]
fn supply_and_prospect_jobs_turn_in_at_the_station_that_posted_them() {
    let mut g = game();
    let pid = join(&mut g, 1, "Quarter", HullClass::Mule);
    let job = find_job(&g, JobTag::Supply);
    let JobKind::Supply { good, units } = job.kind.clone() else { unreachable!() };
    dock_at(&mut g, 1, job.station);
    g.handle(1, ClientMsg::AcceptJob { id: job.id });
    // Docking without the goods leaves the job open.
    dock_at(&mut g, 1, job.station);
    assert_eq!(g.store.profile(pid).unwrap().jobs.len(), 1);
    g.store.profile_mut(pid).unwrap().cargo.insert(good.key().into(), units + 3);
    let before = credits(&g, pid);
    dock_at(&mut g, 1, job.station);
    let p = g.store.profile(pid).unwrap();
    assert!(p.jobs.is_empty());
    assert_eq!(p.cargo.get(good.key()), Some(&3), "only what was asked for is taken");
    assert_eq!(credits(&g, pid), before + job.reward.credits);

    let job = find_job(&g, JobTag::Prospect);
    let JobKind::Prospect { rarity, count } = job.kind.clone() else { unreachable!() };
    dock_at(&mut g, 1, job.station);
    g.handle(1, ClientMsg::AcceptJob { id: job.id });
    let t = shared::tuning::get();
    let id = (0..shared::minerals::catalogue_size(&t.minerals)).find(|i| shared::minerals::mineral_in(&t.minerals, *i).is_some_and(|m| m.rarity >= rarity)).unwrap();
    g.store.profile_mut(pid).unwrap().specimens.insert(id, count);
    dock_at(&mut g, 1, job.station);
    let p = g.store.profile(pid).unwrap();
    assert!(p.jobs.is_empty());
    assert!(p.specimens.is_empty(), "the specimens were handed over");
    assert_eq!(p.career.jobs_done(JobTag::Prospect), 1);
}

#[test]
fn jobs_run_out_past_their_deadline_and_a_captain_carries_only_so_many() {
    let mut g = game();
    let pid = join(&mut g, 1, "Busy", HullClass::Mule);
    let t = shared::tuning::get();
    // Take as many as the boards allow, from a few stations.
    let mut taken = 0;
    for s in [0u16, 1, 2, 3, 4] {
        dock_at(&mut g, 1, s);
        for j in board(&g, s) {
            g.handle(1, ClientMsg::AcceptJob { id: j.id });
            taken = g.store.profile(pid).unwrap().jobs.len();
        }
    }
    assert_eq!(taken, t.trade.jobs.max_active);
    let longest = g.store.profile(pid).unwrap().jobs.iter().map(|a| a.deadline()).max().unwrap();
    // Game time passes the deadline; the server sweeps jobs every few seconds.
    for _ in 0..(11.0 / DT) as usize {
        g.tick(DT, longest + 1);
    }
    let p = g.store.profile(pid).unwrap();
    assert!(p.jobs.is_empty(), "all expired");
    assert!(!p.cargo.contains_key(jobs::SEALED), "sealed cargo of a failed haul is handed back");
}

#[test]
fn titles_come_with_the_record_and_one_is_worn() {
    let mut g = game();
    let pid = join(&mut g, 1, "Legend", HullClass::Talon);
    let th = &shared::tuning::get().trade.titles;
    g.out.clear();
    // Charting enough systems makes a pathfinder.
    let n = th[&Title::Pathfinder] as u16;
    for s in 0..n {
        g.spawn_player(pid, s, false);
    }
    let p = g.store.profile(pid).unwrap();
    assert!(p.titles.contains(&Title::Pathfinder), "{:?}", p.titles);
    assert_eq!(p.title, Some(Title::Pathfinder), "the first title is worn");
    assert!(toasts(&g, 1).iter().any(|t| t.contains("개척자")));
    // A title not earned cannot be worn; an earned one can, and none.
    g.handle(1, ClientMsg::SetTitle { title: Some(Title::Admiral) });
    assert_eq!(g.store.profile(pid).unwrap().title, Some(Title::Pathfinder));
    g.handle(1, ClientMsg::SetTitle { title: None });
    assert_eq!(g.store.profile(pid).unwrap().title, None);
    // Finishing chapter 1 is a title too (granted on the next check, like at login).
    g.store.profile_mut(pid).unwrap().scenarios_completed = 1;
    g.spawn_player(pid, 0, true);
    assert!(g.store.profile(pid).unwrap().titles.contains(&Title::Tracker));
}

#[test]
fn a_smuggling_run_pays_at_the_black_market_and_is_lost_to_core_customs() {
    let mut g = game();
    let pid = join(&mut g, 1, "Runner", HullClass::Mule);
    let job = find_job(&g, JobTag::Smuggle);
    let JobKind::Smuggle { to, units } = job.kind.clone() else { unreachable!() };
    assert!(shared::tuning::get().trade.market.black_market.contains(&galaxy::system(to).tier));
    dock_at(&mut g, 1, job.station);
    g.handle(1, ClientMsg::AcceptJob { id: job.id });
    assert_eq!(g.store.profile(pid).unwrap().cargo.get(jobs::CONTRABAND), Some(&units), "the crates are loaded");
    let before = credits(&g, pid);
    dock_at(&mut g, 1, to);
    let p = g.store.profile(pid).unwrap();
    assert!(p.jobs.is_empty() && !p.cargo.contains_key(jobs::CONTRABAND));
    assert_eq!(credits(&g, pid), before + job.reward.credits);
    assert_eq!(p.career.smuggled, units, "counts toward the smuggler's title");
    // The same run again, but docking in the core on the way: seized, fined, failed.
    let mut g = game();
    let pid = join(&mut g, 1, "Runner", HullClass::Mule);
    dock_at(&mut g, 1, job.station);
    g.handle(1, ClientMsg::AcceptJob { id: job.id });
    let rep = g.store.profile(pid).unwrap().reputation;
    g.out.clear();
    dock_at(&mut g, 1, 0);
    let p = g.store.profile(pid).unwrap();
    assert!(p.jobs.is_empty() && !p.cargo.contains_key(jobs::CONTRABAND), "seized and the run failed");
    assert!(p.reputation < rep);
    assert!(toasts(&g, 1).iter().any(|t| t.contains("세관")));
}

#[test]
fn a_patrol_clears_a_pack_in_each_system_and_pays_after_the_last() {
    let mut g = game();
    let pid = join(&mut g, 1, "Warden", HullClass::Bastion);
    let job = find_job(&g, JobTag::Patrol);
    let JobKind::Patrol { systems, .. } = job.kind.clone() else { unreachable!() };
    dock_at(&mut g, 1, job.station);
    g.handle(1, ClientMsg::AcceptJob { id: job.id });
    let before = credits(&g, pid);
    let away = (0..galaxy::systems().len() as u16).find(|s| !systems.contains(s) && *s != job.station).unwrap();
    for (k, sys) in systems.iter().enumerate() {
        g.spawn_player(pid, away, false);
        g.spawn_player(pid, *sys, false);
        let ids: Vec<EntityId> = g.sessions[&1].job_waves.get(&job.id).expect("a pack waits in each system").1.clone();
        assert_eq!(ids.len() as u32, shared::tuning::get().trade.jobs.patrol_wave.iter().map(|w| w.1).sum::<u32>());
        let me = g.sessions[&1].entity.unwrap();
        for w in &ids {
            let hp = g.ent(*sys, *w).map(|e| e.hull + e.shield).unwrap();
            g.apply_damage(*sys, me, *w, hp + 1.0);
            g.tick(DT, T0);
        }
        let p = g.store.profile(pid).unwrap();
        assert_eq!(p.jobs.is_empty(), k + 1 == systems.len(), "done only after the last system");
        if k + 1 < systems.len() {
            assert!(p.jobs[0].visited.contains(sys));
            // A cleared system stays clear.
            g.spawn_player(pid, away, false);
            g.spawn_player(pid, *sys, false);
            assert!(!g.sessions[&1].job_waves.contains_key(&job.id));
        }
    }
    assert!(credits(&g, pid) >= before + job.reward.credits);
    assert_eq!(g.store.profile(pid).unwrap().career.jobs_done(JobTag::Patrol), 1);
}

#[test]
fn salvage_marks_the_wreck_scans_out_its_recorder_and_pays_at_the_station() {
    let mut g = game();
    let pid = join(&mut g, 1, "Diver", HullClass::Seer);
    let job = (0..galaxy::systems().len() as u16).flat_map(|s| board(&g, s)).find(|j| matches!(&j.kind, JobKind::Salvage { system, .. } if *system != j.station)).expect("a salvage job");
    let JobKind::Salvage { system, poi, .. } = job.kind.clone() else { unreachable!() };
    dock_at(&mut g, 1, job.station);
    g.handle(1, ClientMsg::AcceptJob { id: job.id });
    assert!(g.store.profile(pid).unwrap().revealed.contains(&(system, poi)), "the wreck is on the map");
    // Docking at home without it does nothing.
    dock_at(&mut g, 1, job.station);
    assert_eq!(g.store.profile(pid).unwrap().jobs.len(), 1);
    // Fly to the wreck and scan it.
    g.spawn_player(pid, system, false);
    g.systems.get_mut(&system).unwrap().ents.retain(|_, e| !matches!(e.kind, EntityKind::Npc(_)));
    let at = g.systems[&system].layout.pois.iter().find(|p| p.id == poi).unwrap().pos;
    let me = g.sessions[&1].entity.unwrap();
    let e = g.ent_mut(system, me).unwrap();
    e.pos = at + Vec3::X * 100.0;
    e.vel = Vec3::ZERO;
    g.handle(1, ClientMsg::Scan { poi });
    for _ in 0..(60.0 / DT) as u32 {
        g.tick(DT, T0);
        if g.store.profile(pid).unwrap().cargo.contains_key(jobs::BLACKBOX) {
            break;
        }
    }
    assert_eq!(g.store.profile(pid).unwrap().cargo.get(jobs::BLACKBOX), Some(&1), "the recorder is out");
    let before = credits(&g, pid);
    dock_at(&mut g, 1, job.station);
    let p = g.store.profile(pid).unwrap();
    assert!(p.jobs.is_empty() && !p.cargo.contains_key(jobs::BLACKBOX));
    assert!(credits(&g, pid) >= before + job.reward.credits);
}

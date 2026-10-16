//! Interest management, headless: one viewer in a crowded system must receive bounded snapshots,
//! learn every entity within a few ticks, and drop the ones that leave — checked through the same
//! `SnapshotCache` the client uses, with no sockets or engine.

use server::game::Game;
use server::game::interest::{NEW_BUDGET, SNAPSHOT_BUDGET};
use server::store::Store;
use shared::galaxy::Species;
use shared::glam::Vec3;
use shared::missions::NpcArchetype;
use shared::protocol::*;
use shared::ships::HullClass;
use std::collections::HashMap;

#[test]
fn crowded_system_snapshots_stay_bounded_and_converge() {
    let mut g = Game::new(Store::memory(), history::HistoryEngine::new(), 1_800_000_000);
    g.handle(1, ClientMsg::Hello { version: PROTOCOL_VERSION, name: "Eye".into(), token: None });
    g.handle(1, ClientMsg::CreateCharacter { species: Species::Human, profession: Profession::Explorer, hull: HullClass::Seer, ship_name: "Lens".into(), livery: 1 });
    let pid = g.sessions[&1].player.unwrap();
    g.spawn_player(pid, 7, false);
    for s in g.systems.values_mut() {
        s.pirate_cd = f32::INFINITY;
    }
    // 400 idle pirates spread over a few kilometres (far from the viewer so they do not attack).
    let mut ids = vec![];
    for i in 0..400 {
        let a = i as f32 * 0.37;
        ids.push(g.spawn_npc(7, NpcArchetype::Raider, 1, Vec3::new(a.cos() * (6000.0 + i as f32 * 9.0), (i % 7) as f32 * 40.0, a.sin() * 6000.0 + 9000.0), None));
    }
    let server_count = |g: &Game| g.systems[&7].ents.values().filter(|e| !e.dead).count();
    let mut cache = SnapshotCache::default();
    let mut known: HashMap<EntityId, EntityState> = HashMap::new();
    let mut converged_at = None;
    for tick in 1..=40u64 {
        if tick == 20 {
            // 50 of them leave the system.
            for id in &ids[..50] {
                g.systems.get_mut(&7).unwrap().ents.remove(id);
            }
        }
        g.tick(0.05, 1_800_000_000);
        let snaps: Vec<ServerMsg> = g.out.drain(..).filter(|(c, _)| *c == 1).map(|(_, m)| m).filter(|m| matches!(m, ServerMsg::Snapshot { .. })).collect();
        assert_eq!(snaps.len(), 1, "one snapshot per tick");
        let ServerMsg::Snapshot { tick: t, full, infos, gone, dynamic, .. } = &snaps[0] else { unreachable!() };
        assert!(dynamic.len() <= SNAPSHOT_BUDGET + NEW_BUDGET, "tick {tick}: {} motion records", dynamic.len());
        assert!(infos.len() <= NEW_BUDGET + 8, "tick {tick}: {} infos", infos.len());
        // The wire format round-trips what the server built.
        let bin = encode_snapshot(&snaps[0]).unwrap();
        assert!(matches!(decode_snapshot(&bin), Some(ServerMsg::Snapshot { .. })));
        cache.apply_into(*t, *full, infos.clone(), gone, dynamic, &mut known);
        if converged_at.is_none() && known.len() == server_count(&g) {
            converged_at = Some(tick);
        }
    }
    let at = converged_at.expect("viewer never learned every entity");
    assert!(at <= 5, "400 newcomers at {NEW_BUDGET}/tick should be known within ~4 ticks, took {at}");
    assert_eq!(known.len(), server_count(&g), "departed entities were removed via `gone`");
    assert!(ids[..50].iter().all(|id| !known.contains_key(id)));
}

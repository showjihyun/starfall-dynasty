//! Server load benchmark: `cargo run -p server --release --example bench [players]`
//! N captains (a mix of fighters, cruisers and capitals with escorts) brawl in a war zone.
//! Reports simulation tick time and the bytes the network layer would have to send.

use server::game::Game;
use server::store::Store;
use shared::galaxy::Species;
use shared::protocol::*;
use shared::ships::{HullClass, Module};
use std::time::Instant;

fn main() {
    let n: u64 = std::env::args().nth(1).and_then(|v| v.parse().ok()).unwrap_or(60);
    let t0 = 1_800_000_000;
    let mut g = Game::new(Store::memory(), history::HistoryEngine::new(), t0);
    for c in 1..=n {
        g.handle(c, ClientMsg::Hello { version: PROTOCOL_VERSION, name: format!("Cap{c:03}"), token: None });
        let hull = HullClass::ALL[(c % 5) as usize];
        g.handle(c, ClientMsg::CreateCharacter { species: Species::ALL[(c % 3) as usize], profession: Profession::CombatPilot, hull, ship_name: format!("S{c}"), livery: (c % 7) as u8 });
        let pid = g.sessions[&c].player.unwrap();
        // A third fighters, a third cruisers, a third capitals.
        let lvl = [1u8, 5, 9][(c % 3) as usize];
        let p = g.store.profile_mut(pid).unwrap();
        for m in Module::ALL {
            p.ship.modules.insert(m, lvl);
        }
        g.spawn_player(pid, 7, false);
    }
    // Everyone turns and fires; capitals' turrets engage automatically.
    for c in 1..=n {
        g.handle(c, ClientMsg::Controls(Controls { thrust: 0.6, yaw: if c % 2 == 0 { 0.3 } else { -0.3 }, fire: true, ..Default::default() }));
    }
    let ents = g.systems[&7].ents.len();
    let ticks = 400;
    let mut tick_ns = vec![];
    let mut bytes = 0usize;
    let mut encode_ns = 0u128;
    let mut by_kind: std::collections::BTreeMap<String, usize> = Default::default();
    for _ in 0..ticks {
        let t = Instant::now();
        g.tick(0.05, t0);
        tick_ns.push(t.elapsed().as_nanos());
        let te = Instant::now();
        for (_, m) in g.out.drain(..) {
            let s = encode(&m);
            bytes += s.len();
            *by_kind.entry(kind_of(&s)).or_default() += s.len();
        }
        for (conns, m) in g.out_shared.drain(..) {
            let s = encode(&m);
            bytes += s.len() * conns.len();
            *by_kind.entry(kind_of(&s)).or_default() += s.len() * conns.len();
        }
        encode_ns += te.elapsed().as_nanos();
    }
    tick_ns.sort();
    let avg = tick_ns.iter().sum::<u128>() as f64 / ticks as f64 / 1e6;
    let p95 = tick_ns[ticks * 95 / 100] as f64 / 1e6;
    let secs = ticks as f64 * 0.05;
    println!("players {n}, entities in system {ents}");
    println!("tick avg {avg:.3} ms, p95 {p95:.3} ms (budget 50 ms)");
    println!("encode {:.3} ms/tick", encode_ns as f64 / ticks as f64 / 1e6);
    println!("outbound {:.1} MB/s total, {:.1} KB/s per client", bytes as f64 / secs / 1e6, bytes as f64 / secs / 1e3 / n as f64);
    let mut kinds: Vec<_> = by_kind.into_iter().collect();
    kinds.sort_by_key(|k| std::cmp::Reverse(k.1));
    for (k, b) in kinds.iter().take(8) {
        println!("  {k:<16} {:.1} KB/s per client", *b as f64 / secs / 1e3 / n as f64);
    }
    println!("battle state: {} players alive", g.sessions.values().filter(|s| s.entity.is_some()).count());
}

/// Message variant name from its JSON encoding (`{"Snapshot":{...}}` or `"Variant"`).
fn kind_of(s: &str) -> String {
    s.trim_start_matches(['{', '"']).split('"').next().unwrap_or("?").to_string()
}

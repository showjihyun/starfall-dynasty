//! `bot <ws-url> <name> [system]` — log in as a headless captain, optionally warp along a path,
//! and print every entity the server reports in that system. Handy for checking live servers.

use bot::Bot;
use shared::galaxy::Species;
use shared::protocol::{Profession, ServerMsg};
use shared::ships::HullClass;

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let url = args.get(1).cloned().unwrap_or_else(|| "ws://127.0.0.1:7777".into());
    let name = args.get(2).cloned().unwrap_or_else(|| "Observer".into());
    let mut b = Bot::connect(&url, &name).await.expect("connect");
    b.login_and_create(HullClass::Wren, Species::Human, Profession::Explorer).await.expect("login");
    for sys in args.iter().skip(3).filter_map(|s| s.parse::<u16>().ok()) {
        if let Err(e) = b.warp_to(sys).await {
            eprintln!("{e}");
        }
    }
    b.until(3.0, |m| matches!(m, ServerMsg::Snapshot { .. })).await;
    println!("system {} — {} entities", b.system, b.ents.len());
    let mut v: Vec<_> = b.ents.values().collect();
    v.sort_by_key(|e| e.id);
    for e in v {
        println!("{:>5} {:?} {} owner={:?} docked={} r={:.0}", e.id, e.kind, e.name, e.owner, e.docked, e.radius);
    }
}

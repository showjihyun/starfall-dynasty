//! `bot <ws-url> <name> [system…] [--hull H] [--say "/grow 9;/tp station 0 400 0"] [--stay SECS]` — log in
//! as a headless captain, optionally warp along a path, say each `;`-separated line (dev commands on a
//! `--dev` server), print every entity the server reports in that system, then stay logged in for SECS
//! (a second captain to pose next to a scripted client screenshot).

use bot::Bot;
use shared::galaxy::Species;
use shared::protocol::{ClientMsg, Profession, ServerMsg};
use shared::ships::HullClass;

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let url = args.get(1).cloned().unwrap_or_else(|| "ws://127.0.0.1:7777".into());
    let name = args.get(2).cloned().unwrap_or_else(|| "Observer".into());
    let get = |k: &str| args.iter().position(|x| x == k).and_then(|i| args.get(i + 1)).cloned();
    let hull = get("--hull").and_then(|h| HullClass::ALL.into_iter().find(|c| format!("{c:?}").eq_ignore_ascii_case(&h))).unwrap_or(HullClass::Wren);
    let mut b = Bot::connect(&url, &name).await.expect("connect");
    b.login_and_create(hull, Species::Human, Profession::Explorer).await.expect("login");
    let flags = ["--hull", "--say", "--stay"];
    let path = args.iter().enumerate().skip(3).filter(|(i, _)| !flags.contains(&args[i - 1].as_str())).filter_map(|(_, s)| s.parse::<u16>().ok()).collect::<Vec<_>>();
    for sys in path {
        if let Err(e) = b.warp_to(sys).await {
            eprintln!("{e}");
        }
    }
    for line in get("--say").iter().flat_map(|l| l.split(';')).map(str::trim).filter(|l| !l.is_empty()) {
        b.send(ClientMsg::Chat { text: line.to_string() }).await;
        b.pump(1.0).await;
    }
    b.until(3.0, |m| matches!(m, ServerMsg::Snapshot { .. })).await;
    println!("system {} — {} entities", b.system, b.ents.len());
    let mut v: Vec<_> = b.ents.values().collect();
    v.sort_by_key(|e| e.id);
    for e in v {
        println!("{:>5} {:?} {} owner={:?} docked={} r={:.0}", e.id, e.kind, e.name, e.owner, e.docked, e.radius);
    }
    if let Some(secs) = get("--stay").and_then(|v| v.parse::<f32>().ok()) {
        b.pump(secs).await;
    }
}

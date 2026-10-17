//! Automated play QA (docs/QA_PHASE2_3.md): starts a `--dev` server (recording to `--record DIR`
//! if given), plays the Phase 2–3 checklist with two headless captains, prints PASS/FAIL per item.
//!
//! `cargo build --release -p server && cargo run -p bot --release --example qa [-- --record DIR] [--keep]`
//!
//! `--keep` leaves the server running afterwards (port 7790) so `client --name QA-A …` can look at the
//! resulting world; `--record DIR` keeps a replayable recording of the whole run.

use std::process::Command;
use std::time::Duration;

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let arg = |k: &str| args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).cloned();
    let keep = args.iter().any(|a| a == "--keep");
    let port = 7790u16;
    // target/<profile>/examples/qa.exe → target/<profile>/server.exe
    let dir = std::env::current_exe().unwrap().parent().unwrap().parent().unwrap().to_path_buf();
    let exe = dir.join(if cfg!(windows) { "server.exe" } else { "server" });
    let data = arg("--data").map(std::path::PathBuf::from).unwrap_or_else(|| std::env::temp_dir().join(format!("sfd-qa-{}", std::process::id())));
    let _ = std::fs::remove_dir_all(&data);
    let mut cmd = Command::new(exe);
    cmd.args(["--dev", "--port", &port.to_string(), "--data", data.to_str().unwrap()]);
    if let Some(rec) = arg("--record") {
        cmd.args(["--record", &rec]);
    }
    let mut child = cmd.spawn().expect("server binary (cargo build --release -p server first)");
    for _ in 0..50 {
        if tokio::net::TcpStream::connect(("127.0.0.1", port)).await.is_ok() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let t0 = std::time::Instant::now();
    let result = bot::qa::run(&format!("ws://127.0.0.1:{port}"), "QA-A", "QA-B").await;
    match &result {
        Ok(r) => print!("{}", r.text()),
        Err(e) => println!("QA ABORTED: {e}"),
    }
    println!("({:.1} s)", t0.elapsed().as_secs_f64());
    if keep {
        println!("server left running on ws://127.0.0.1:{port} (data {})", data.display());
        if let Ok(r) = &result {
            // One line per captain: `client --server ws://127.0.0.1:7790 --name NAME --token TOKEN --auto`.
            let lines: Vec<String> = r.tokens.iter().map(|(n, t)| format!("{n} {t}")).collect();
            let _ = std::fs::write(data.join("qa_tokens.txt"), lines.join("
"));
            println!("tokens in {}", data.join("qa_tokens.txt").display());
        }
    } else {
        let _ = child.kill();
        let _ = std::fs::remove_dir_all(&data);
    }
    if !result.is_ok_and(|r| r.ok()) {
        std::process::exit(1);
    }
}

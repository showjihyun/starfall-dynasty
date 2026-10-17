//! The Phase 2–3 play checklist (docs/QA_PHASE2_3.md), played by two headless captains against a
//! real `--dev` server over WebSockets. Every item must pass.

use std::process::Command;
use std::time::Duration;

#[tokio::test(flavor = "multi_thread")]
async fn phase2_3_checklist_plays_through() {
    let port = 7792u16;
    let dir = std::env::current_exe().unwrap().parent().unwrap().parent().unwrap().to_path_buf();
    let exe = dir.join(if cfg!(windows) { "server.exe" } else { "server" });
    let data = std::env::temp_dir().join(format!("sfd-qa-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&data);
    let mut child = Command::new(exe).args(["--dev", "--port", &port.to_string(), "--data", data.to_str().unwrap()]).spawn().expect("server binary (cargo build -p server first)");
    for _ in 0..50 {
        if tokio::net::TcpStream::connect(("127.0.0.1", port)).await.is_ok() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let result = bot::qa::run(&format!("ws://127.0.0.1:{port}"), "QA-A", "QA-B").await;
    let _ = child.kill();
    let _ = std::fs::remove_dir_all(&data);
    let report = result.expect("QA run");
    assert!(report.ok(), "\n{}", report.text());
    assert!(report.items.len() >= 17);
}

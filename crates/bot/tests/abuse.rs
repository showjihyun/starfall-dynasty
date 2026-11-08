//! One bad client must not hurt the server or the other captains: a huge frame or a message flood
//! closes that connection only (server/src/main.rs MAX_FRAME, MSG_RATE/MSG_BURST, FLOOD_KICK).

use bot::Bot;
use futures_util::{SinkExt, StreamExt};
use shared::galaxy::Species;
use shared::protocol::*;
use shared::ships::HullClass;
use std::process::{Child, Command};
use std::time::Duration;
use tokio_tungstenite::tungstenite::Message;

struct Server(Child, std::path::PathBuf);
impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = std::fs::remove_dir_all(&self.1);
    }
}

async fn start_server(port: u16) -> Server {
    let dir = std::env::current_exe().unwrap().parent().unwrap().parent().unwrap().to_path_buf();
    let exe = dir.join(if cfg!(windows) { "server.exe" } else { "server" });
    let data = std::env::temp_dir().join(format!("sfd-abuse-{port}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&data);
    let child = Command::new(exe).args(["--port", &port.to_string(), "--data", data.to_str().unwrap()]).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).spawn().expect("server binary (cargo build -p server first)");
    for _ in 0..50 {
        if tokio::net::TcpStream::connect(("127.0.0.1", port)).await.is_ok() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    Server(child, data)
}

/// Reads until the server closes the socket (true) or `secs` pass (false).
async fn closed_within(ws: &mut tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>, secs: u64) -> bool {
    let until = tokio::time::Instant::now() + Duration::from_secs(secs);
    loop {
        match tokio::time::timeout_at(until, ws.next()).await {
            Err(_) => return false,
            Ok(None) | Ok(Some(Err(_))) | Ok(Some(Ok(Message::Close(_)))) => return true,
            Ok(Some(Ok(_))) => {}
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_huge_frame_or_a_flood_closes_only_that_connection() {
    let port = 7813;
    let _srv = start_server(port).await;
    let url = format!("ws://127.0.0.1:{port}");
    let mut good = Bot::connect(&url, "Honest").await.unwrap();
    good.login_and_create(HullClass::Talon, Species::Human, Profession::CombatPilot).await.unwrap();

    // A 1 MB frame.
    let (mut big, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
    let _ = big.send(Message::text(format!("{{\"Chat\":{{\"text\":\"{}\"}}}}", "x".repeat(1 << 20)))).await;
    assert!(closed_within(&mut big, 5).await, "a 1 MB frame closes the connection");

    // Thousands of messages as fast as the socket takes them (not pings: their pongs would fill the send
    // backlog and close the connection for another reason).
    let (mut flood, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
    let ping = encode(&ClientMsg::RequestAlliances);
    let mut sent = 0;
    for _ in 0..20_000 {
        if flood.send(Message::text(ping.clone())).await.is_err() {
            break;
        }
        sent += 1;
    }
    assert!(sent > 2_000, "the flood went out ({sent})");
    assert!(closed_within(&mut flood, 10).await, "a flood closes the connection");

    // The honest captain still plays — pinging as the real client does every second (a pong once wrapped
    // the send-backlog counter and disconnected every client that pinged): a round trip works.
    for i in 0..30 {
        good.send(ClientMsg::Ping { t: i as f64 }).await;
        good.pump(0.1).await;
    }
    good.send(ClientMsg::RequestAlliances).await;
    assert!(good.until(5.0, |m| matches!(m, ServerMsg::Alliances { .. })).await.is_some(), "the server still answers others");
}

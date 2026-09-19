//! `/ws` 통합 테스트 — **실제 WebSocket 클라이언트로** 프레이밍·인증·큐·수명 주기를 검증한다.
//!
//! # 왜 여기서 실제 소켓을 쓰는가
//!
//! 스프린트 계약 SC-14~SC-28 의 1차 증명은 서버 쪽이어야 한다. QA 봇이 재현하지 못하는
//! 항목(예: 자동 Pong 때문에 idle 을 만들기 어려운 SC-26)이 있어도 판정이 막히지 않도록,
//! 서버가 스스로 close code 와 순서를 확인한다.
//!
//! **DB 는 쓰지 않는다.** 영속화 채널을 테스트가 직접 받아 `PersistBatch` 를 들여다본다 —
//! 도메인 이벤트 검증은 DB 없이도 가능하고, DB 경로는 QA 의 e2e 가 따로 본다.

// 통합 테스트 파일이라 clippy.toml 의 allow-*-in-tests 가 헬퍼 함수까지 덮지 못한다.
// p0-01 의 contract_tests.rs 와 같은 처리다 — 운영 코드에는 영향이 없다.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use starfall_contracts::events::SessionCloseReason;
use starfall_contracts::primitives::{GameCalendar, GameTime, ServerVersion, UuidV7};
use starfall_gateway::{AppState, DevAuth, Probes, Stats, realtime_router};

use starfall_sim::{DomainEventBody, PersistBatch, Simulation, WorldConstants};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::{Message, Utf8Bytes};

const SECRET: &str = "dev_only_not_a_secret";
const WORLD_ID: &str = "01a0b1c2-3d4e-7f01-8a2b-9c0d1e2f3a4b";
const SUBJECT: &str = "01a0b1c2-2c01-7a45-8b67-89abcdef0123";

// ---------------------------------------------------------------------------
// 테스트 서버
// ---------------------------------------------------------------------------

struct TestServer {
    addr: SocketAddr,
    auth: DevAuth,
    shutdown: Arc<AtomicBool>,
    batches: Arc<Mutex<Vec<PersistBatch>>>,
    tick_thread: Option<std::thread::JoinHandle<()>>,
}

impl TestServer {
    async fn start(auth_enabled: bool) -> Self {
        let world = WorldConstants {
            world_id: UuidV7::parse(WORLD_ID).unwrap(),
            calendar: GameCalendar::new(20, GameTime::parse("3800-01-01T00:00:00Z").unwrap(), 60)
                .unwrap(),
            server_version: ServerVersion::parse("0.1.0-test").unwrap(),
        };
        let stats = Stats::new();
        stats.set_start_tick(0);

        let (persist_tx, mut persist_rx) = mpsc::channel(512);
        let batches = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&batches);
        tokio::spawn(async move {
            while let Some(batch) = persist_rx.recv().await {
                sink.lock().unwrap().push(batch);
            }
        });

        let shutdown = Arc::new(AtomicBool::new(false));
        let (submit, tick_thread) = starfall_gateway::runtime::build(
            Simulation::new(world, 0),
            persist_tx,
            stats.clone(),
            Duration::from_millis(50),
            Arc::clone(&shutdown),
        );

        let auth = DevAuth::from_secret(auth_enabled.then(|| SECRET.to_owned()));
        let probes = Probes::new(
            "postgres://starfall:pw@127.0.0.1:1/starfall",
            "redis://127.0.0.1:1/0",
        )
        .unwrap();
        let state = AppState::new("0.1.0-test", probes).with_realtime(auth.clone(), stats, submit);

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let _ = axum::serve(listener, realtime_router(state)).await;
        });

        Self {
            addr,
            auth,
            shutdown,
            batches,
            tick_thread: Some(tick_thread),
        }
    }

    fn token(&self, subject: &str) -> String {
        self.auth.mint(UuidV7::parse(subject).unwrap()).unwrap()
    }

    /// 종료 스윕을 돌리고 tick 스레드가 끝날 때까지 기다린다.
    async fn shutdown_and_join(&mut self) {
        self.shutdown.store(true, Ordering::Release);
        if let Some(handle) = self.tick_thread.take() {
            let _ = tokio::task::spawn_blocking(move || handle.join()).await;
        }
        // 영속화 수집 태스크가 마지막 배치를 담을 시간을 준다.
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    fn events(&self) -> Vec<(String, SessionCloseReason)> {
        self.batches
            .lock()
            .unwrap()
            .iter()
            .flat_map(|batch| batch.events.iter())
            .map(|event| match &event.body {
                DomainEventBody::SessionOpened(_) => (
                    "SESSION_OPENED".to_owned(),
                    SessionCloseReason::ClientClosed,
                ),
                DomainEventBody::SessionClosed(payload) => {
                    ("SESSION_CLOSED".to_owned(), payload.close_reason)
                }
            })
            .collect()
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Release);
    }
}

type Client =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

async fn connect(server: &TestServer, token: Option<&str>) -> Result<Client, u16> {
    let mut request = format!("ws://{}/ws", server.addr)
        .into_client_request()
        .unwrap();
    if let Some(token) = token {
        request
            .headers_mut()
            .insert("Authorization", format!("Bearer {token}").parse().unwrap());
    }
    match tokio_tungstenite::connect_async(request).await {
        Ok((stream, _)) => Ok(stream),
        Err(tokio_tungstenite::tungstenite::Error::Http(response)) => {
            Err(response.status().as_u16())
        }
        Err(error) => panic!("예상치 못한 연결 오류: {error}"),
    }
}

fn ping_command(command_id: &str, probe_seq: u32) -> Message {
    Message::Text(Utf8Bytes::from(format!(
        r#"{{"command_id":"{command_id}","command_type":"PING_SERVER","schema_version":1,"client_sent_at":null,"payload":{{"probe_seq":{probe_seq}}}}}"#
    )))
}

fn command_id(n: u64) -> String {
    format!(
        "01a0b1c2-0000-7{:03x}-8{:03x}-{:012x}",
        n & 0xfff,
        (n >> 12) & 0xfff,
        n
    )
}

/// 다음 **계약 메시지**를 받는다 (Ping/Pong 은 건너뛴다).
async fn next_json(client: &mut Client) -> serde_json::Value {
    loop {
        let message = tokio::time::timeout(Duration::from_secs(5), client.next())
            .await
            .expect("메시지 대기 타임아웃")
            .expect("스트림이 끝났다")
            .expect("소켓 오류");
        if let Message::Text(text) = message {
            return serde_json::from_str(text.as_str()).expect("계약 메시지는 JSON 이다");
        }
    }
}

// ---------------------------------------------------------------------------
// SC-14 / SC-16 — 인증
// ---------------------------------------------------------------------------

/// SC-14 — 유효 토큰 → 101, 첫 메시지가 `SESSION_READY`, `actor_id` 가 토큰 주체와 같다.
#[tokio::test]
async fn sc14_valid_token_first_message_is_session_ready() {
    let mut server = TestServer::start(true).await;
    let token = server.token(SUBJECT);
    let mut client = connect(&server, Some(&token)).await.expect("101 기대");

    let first = next_json(&mut client).await;
    assert_eq!(
        first["message_type"], "SESSION_READY",
        "첫 계약 메시지 (ADR-0005 §3)"
    );
    assert_eq!(
        first["payload"]["actor_id"], SUBJECT,
        "I-10: 행위자는 서버가 정한다"
    );
    assert_eq!(first["payload"]["world_id"], WORLD_ID);
    assert_eq!(first["payload"]["tick_hz"], 20);
    assert!(
        first["correlation_id"].is_string(),
        "세션 correlation 이 실린다"
    );
    println!("[SC-14] SESSION_READY 확인: {first}");

    server.shutdown_and_join().await;
}

/// SC-15 — 401 3종. 소켓이 열리지 않고 `SESSION_OPENED` 가 생기지 않는다 (I-24).
#[tokio::test]
async fn sc15_three_unauthenticated_cases_are_rejected() {
    let server = TestServer::start(true).await;
    let opened_before = server.events().len();

    // (a) 헤더 없음
    assert_eq!(connect(&server, None).await.unwrap_err(), 401);
    // (b) 서명 불일치
    assert_eq!(
        connect(&server, Some(&format!("{SUBJECT}.{}", "0".repeat(64))))
            .await
            .unwrap_err(),
        401
    );
    // (c) 주체가 UUIDv7 이 아니다
    assert_eq!(
        connect(&server, Some(&format!("not-a-uuid.{}", "0".repeat(64))))
            .await
            .unwrap_err(),
        401
    );

    assert_eq!(
        server.events().len(),
        opened_before,
        "I-24: 세션도 기록도 없다"
    );
    println!("[SC-15] 401 검사 3건 통과, SESSION_OPENED 증가 0");
}

/// SC-16 — 비밀 미설정이면 `/ws` 만 503 이고 `/healthz` 는 200 이다.
#[tokio::test]
async fn sc16_missing_secret_disables_only_ws() {
    let server = TestServer::start(false).await;
    assert_eq!(connect(&server, None).await.unwrap_err(), 503);

    let body = reqwest_get(&format!("http://{}/healthz", server.addr)).await;
    assert!(body.contains("\"status\":\"ok\""), "{body}");

    let stats = stats_json(&server).await;
    let rejected = stats["upgrade_rejected_total"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["label"] == "auth_not_configured")
        .unwrap()["count"]
        .as_u64()
        .unwrap();
    assert_eq!(rejected, 1, "거부의 증거는 서버 쪽 카운터다");
    println!("[SC-16] /ws 503 auth_not_configured, /healthz 200");
}

/// 의존성 없이 단순 GET 을 보낸다(테스트 전용 미니 HTTP 클라이언트).
async fn reqwest_get(url: &str) -> String {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let rest = url.trim_start_matches("http://");
    let (authority, path) = rest.split_once('/').unwrap();
    let mut stream = tokio::net::TcpStream::connect(authority).await.unwrap();
    stream
        .write_all(
            format!("GET /{path} HTTP/1.1\r\nHost: {authority}\r\nConnection: close\r\n\r\n")
                .as_bytes(),
        )
        .await
        .unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).await.unwrap();
    response
}

// ---------------------------------------------------------------------------
// SC-18 / SC-19 / SC-20 / SC-21 — tick·순서·큐
// ---------------------------------------------------------------------------

/// SC-19 — `COMMAND_RESULT` → `PING_REPLY` 순서이고 두 envelope 의 `tick` 이 같다 (I-15).
#[tokio::test]
async fn sc19_command_result_precedes_ping_reply_with_same_tick() {
    let mut server = TestServer::start(true).await;
    let token = server.token(SUBJECT);
    let mut client = connect(&server, Some(&token)).await.unwrap();
    assert_eq!(
        next_json(&mut client).await["message_type"],
        "SESSION_READY"
    );

    const CHECKED: u64 = 10;
    for n in 0..CHECKED {
        let id = command_id(1000 + n);
        client.send(ping_command(&id, n as u32)).await.unwrap();

        let result = next_json(&mut client).await;
        let reply = next_json(&mut client).await;
        assert_eq!(result["message_type"], "COMMAND_RESULT", "n={n}");
        assert_eq!(reply["message_type"], "PING_REPLY", "n={n}");
        assert_eq!(result["payload"]["status"], "ACCEPTED");
        assert!(result["payload"]["reason_code"].is_null(), "I-14");
        assert!(result["correlation_id"].is_null(), "스펙 §5.2: 언제나 null");
        assert_eq!(result["payload"]["command_id"], id);
        assert_eq!(reply["payload"]["command_id"], id);
        assert_eq!(reply["payload"]["probe_seq"], n);
        assert_eq!(result["tick"], reply["tick"], "같은 tick 에서 만들어진다");
    }
    println!("[SC-19] 순서·tick 검사 {CHECKED}건 통과");

    server.shutdown_and_join().await;
}

/// SC-18 — `tick_total == tick − start_tick + 1` (I-17).
#[tokio::test]
async fn sc18_tick_identity_holds() {
    let mut server = TestServer::start(true).await;
    tokio::time::sleep(Duration::from_millis(300)).await;

    let first = stats_json(&server).await;
    tokio::time::sleep(Duration::from_millis(500)).await;
    let second = stats_json(&server).await;

    for body in [&first, &second] {
        let (tick, start, total) = (
            body["tick"].as_u64().unwrap(),
            body["start_tick"].as_u64().unwrap(),
            body["tick_total"].as_u64().unwrap(),
        );
        assert_eq!(
            total,
            tick - start + 1,
            "tick_total == tick − start_tick + 1"
        );
    }
    let dt = second["tick"].as_u64().unwrap() - first["tick"].as_u64().unwrap();
    let dtotal = second["tick_total"].as_u64().unwrap() - first["tick_total"].as_u64().unwrap();
    assert_eq!(dt, dtotal, "두 호출 사이 증가량이 같다");
    assert!(dt > 0, "tick 이 실제로 돈다");
    println!("[SC-18] tick 증가 {dt}, tick_total 증가 {dtotal}");

    server.shutdown_and_join().await;
}

/// 세션이 실제로 닫힐 때까지 기다린다.
///
/// close 프레임을 받은 것과 tick 이 `SESSION_CLOSED` 를 발행한 것은 다른 사건이다
/// (제출은 비동기이고 판정은 다음 tick 이다). 기다리지 않고 종료 스윕을 돌리면
/// `close_reason` 이 `SERVER_SHUTDOWN` 으로 덮인다.
async fn wait_for_no_connections(server: &TestServer) {
    for _ in 0..300 {
        if stats_json(server).await["ws_connections"] == 0 {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("세션이 15초 안에 닫히지 않았다");
}

async fn stats_json(server: &TestServer) -> serde_json::Value {
    let raw = reqwest_get(&format!("http://{}/debug/stats", server.addr)).await;
    let body = raw.split("\r\n\r\n").nth(1).unwrap_or(&raw);
    serde_json::from_str(body.trim()).unwrap()
}

/// SC-20 — in-flight 상한 초과는 **거부**이고 연결은 유지된다. 손실 0.
#[tokio::test]
async fn sc20_in_flight_limit_rejects_without_closing() {
    let mut server = TestServer::start(true).await;
    let token = server.token(SUBJECT);
    let mut client = connect(&server, Some(&token)).await.unwrap();
    assert_eq!(
        next_json(&mut client).await["message_type"],
        "SESSION_READY"
    );

    // 한 tick(50 ms) 안에 상한(64)을 크게 넘겨 보낸다.
    const SENT: u64 = 120;
    for n in 0..SENT {
        client
            .send(ping_command(&command_id(2000 + n), n as u32))
            .await
            .unwrap();
    }

    let mut results = 0u64;
    let mut too_many = 0u64;
    let mut seen = std::collections::HashSet::new();
    while results < SENT {
        let message = next_json(&mut client).await;
        if message["message_type"] != "COMMAND_RESULT" {
            continue;
        }
        results += 1;
        assert!(
            seen.insert(
                message["payload"]["command_id"]
                    .as_str()
                    .unwrap()
                    .to_owned()
            ),
            "같은 command_id 에 COMMAND_RESULT 가 2건 왔다 (I-15 위반)"
        );
        if message["payload"]["reason_code"] == "TOO_MANY_IN_FLIGHT" {
            too_many += 1;
        }
    }
    assert_eq!(results, SENT, "보낸 수 == 받은 COMMAND_RESULT 수 (손실 0)");
    assert!(too_many > 0, "상한을 넘겼으므로 거부가 있어야 한다");

    // 연결이 살아 있다 — 새 명령이 여전히 처리된다.
    let id = command_id(9999);
    client.send(ping_command(&id, 1)).await.unwrap();
    loop {
        let message = next_json(&mut client).await;
        if message["message_type"] == "COMMAND_RESULT" && message["payload"]["command_id"] == id {
            assert_eq!(message["payload"]["status"], "ACCEPTED");
            break;
        }
    }
    println!(
        "[SC-20] 보낸 {SENT}건 == COMMAND_RESULT {results}건, TOO_MANY_IN_FLIGHT {too_many}건, 연결 유지"
    );

    server.shutdown_and_join().await;
}

/// SC-21 — 같은 `command_id` 2회 → `PING_REPLY` 는 1건뿐이다.
#[tokio::test]
async fn sc21_duplicate_command_id_yields_one_ping_reply() {
    let mut server = TestServer::start(true).await;
    let token = server.token(SUBJECT);
    let mut client = connect(&server, Some(&token)).await.unwrap();
    assert_eq!(
        next_json(&mut client).await["message_type"],
        "SESSION_READY"
    );

    let id = command_id(4242);
    client.send(ping_command(&id, 1)).await.unwrap();
    client.send(ping_command(&id, 1)).await.unwrap();

    let mut results = Vec::new();
    let mut replies = 0;
    while results.len() < 2 {
        let message = next_json(&mut client).await;
        match message["message_type"].as_str() {
            Some("COMMAND_RESULT") => results.push(message),
            Some("PING_REPLY") => replies += 1,
            _ => {}
        }
    }
    assert_eq!(results[0]["payload"]["status"], "ACCEPTED");
    assert_eq!(results[1]["payload"]["status"], "REJECTED");
    assert_eq!(results[1]["payload"]["reason_code"], "DUPLICATE_COMMAND_ID");
    assert_eq!(replies, 1, "PING_REPLY 는 2건이 아니다 (ADR-0006 §6)");
    println!("[SC-21] COMMAND_RESULT 2건(ACCEPTED+DUPLICATE), PING_REPLY 1건");

    server.shutdown_and_join().await;
}

// ---------------------------------------------------------------------------
// SC-24 / SC-25 — 프레이밍 위반과 예산
// ---------------------------------------------------------------------------

/// SC-24 — 16 KiB 초과 텍스트는 **계수되고**, 10초 창 8회를 넘기면 close 1002 다.
#[tokio::test]
async fn sc24_oversized_text_counts_then_closes_with_1002() {
    let mut server = TestServer::start(true).await;
    let token = server.token(SUBJECT);
    let mut client = connect(&server, Some(&token)).await.unwrap();
    assert_eq!(
        next_json(&mut client).await["message_type"],
        "SESSION_READY"
    );

    let oversized = Message::Text(Utf8Bytes::from("x".repeat(20 * 1024)));

    // 1회 — 계수되지만 끊기지 않는다 (라이브러리 한도 64 KiB > 앱 한도 16 KiB).
    client.send(oversized.clone()).await.unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await;
    let after_one = stats_json(&server).await["protocol_violations_total"]
        .as_u64()
        .unwrap();
    assert_eq!(after_one, 1, "위반 1건이 계수됐다");

    let id = command_id(5555);
    client.send(ping_command(&id, 1)).await.unwrap();
    loop {
        let message = next_json(&mut client).await;
        if message["payload"]["command_id"] == id {
            break; // 연결이 살아 있다
        }
    }

    // 나머지 8회 → 10초 창에서 9번째가 예산을 넘긴다.
    for _ in 0..8 {
        client.send(oversized.clone()).await.unwrap();
    }
    let code = read_until_close(&mut client).await;
    assert_eq!(
        code,
        Some(1002),
        "ADR-0005 §2: 프로토콜 위반 예산 초과 = 1002"
    );

    wait_for_no_connections(&server).await;
    server.shutdown_and_join().await;
    let closed: Vec<_> = server
        .events()
        .into_iter()
        .filter(|(kind, _)| kind == "SESSION_CLOSED")
        .collect();
    assert!(
        closed
            .iter()
            .any(|(_, reason)| *reason == SessionCloseReason::ProtocolViolation),
        "close_reason = PROTOCOL_VIOLATION 이 기록된다: {closed:?}"
    );
    println!("[SC-24] 위반 1회 계수 후 연결 유지, 9회째 close 1002 + PROTOCOL_VIOLATION");
}

/// SC-25 — 바이너리 프레임도 같은 예산으로 계수된다.
#[tokio::test]
async fn sc25_binary_frames_use_the_same_budget() {
    let mut server = TestServer::start(true).await;
    let token = server.token(SUBJECT);
    let mut client = connect(&server, Some(&token)).await.unwrap();
    assert_eq!(
        next_json(&mut client).await["message_type"],
        "SESSION_READY"
    );

    for _ in 0..9 {
        client
            .send(Message::Binary(vec![1, 2, 3].into()))
            .await
            .unwrap();
    }
    let code = read_until_close(&mut client).await;
    assert_eq!(code, Some(1002));

    let violations = stats_json(&server).await["protocol_violations_total"]
        .as_u64()
        .unwrap();
    assert!(
        violations >= 9,
        "바이너리 프레임 9건이 계수됐다: {violations}"
    );
    println!("[SC-25] 바이너리 프레임 9건 → close 1002, 위반 계수 {violations}");

    server.shutdown_and_join().await;
}

/// Close 프레임이 올 때까지 읽고 close code 를 돌려준다.
async fn read_until_close(client: &mut Client) -> Option<u16> {
    let deadline = Duration::from_secs(10);
    loop {
        let next = tokio::time::timeout(deadline, client.next()).await.ok()??;
        match next {
            Ok(Message::Close(frame)) => return frame.map(|frame| u16::from(frame.code)),
            Ok(_) => {}
            Err(tokio_tungstenite::tungstenite::Error::ConnectionClosed) => return None,
            Err(_) => return None,
        }
    }
}

/// SC-22 — 수신을 멈춘 클라이언트는 `SLOW_CONSUMER` 로 닫힌다 (close code 1011).
///
/// 이 경로는 **in-flight 해제가 "COMMAND_RESULT 생성 시점"일 때만 도달 가능하다**
/// (ADR-0006 §5). 전달 확인 시점으로 바꾸면 한 세션이 만들 수 있는 응답이 128건으로 묶여
/// 256 슬롯 송신 큐가 절대 차지 않는다.
///
/// # 판정의 정본은 close code 가 아니라 `close_reason` 이다
///
/// 서버는 close 1011 프레임을 보내지만, **응답을 읽지 않는 피어에게는 그 프레임이 닿지
/// 않을 수 있다** — 서버가 미처리 수신 데이터를 남긴 채 소켓을 닫으면 TCP 가 RST 를 보내고,
/// 피어의 수신 버퍼(쌓여 있던 Close 프레임 포함)가 버려진다. 이것은 구현 결함이 아니라
/// "읽지 않는 클라이언트"의 정의상 귀결이다. 그래서 ADR-0005 §2 가 정한 대로
/// **사실로 남는 것은 `close_reason`** 이고, 이 테스트도 그것을 판정한다.
/// close code 1011 자체는 `close_code(SlowConsumer)` 단위 테스트가 고정한다.
#[tokio::test]
async fn sc22_slow_consumer_is_closed_with_slow_consumer_reason() {
    let mut server = TestServer::start(true).await;
    let token = server.token(SUBJECT);
    let mut client = connect(&server, Some(&token)).await.unwrap();
    assert_eq!(
        next_json(&mut client).await["message_type"],
        "SESSION_READY"
    );

    // 여기서부터 **읽지 않는다.** 소켓 버퍼가 차고, 송신 태스크가 막히고, 송신 큐가 찬다.
    let mut sent = 0u64;
    let mut send_failed = false;
    while sent < 30_000 {
        let message = ping_command(&command_id(20_000 + sent), (sent % 4096) as u32);
        // 서버가 닫으면 send 는 오류가 나거나(RST) 영원히 막힌다(수신자가 사라져
        // 송신 버퍼가 차 있다). 둘 다 "서버가 닫았다"의 관측이다.
        match tokio::time::timeout(Duration::from_secs(2), client.send(message)).await {
            Ok(Ok(())) => {}
            Ok(Err(_)) | Err(_) => {
                send_failed = true;
                break;
            }
        }
        sent += 1;
    }

    // 서버가 실제로 닫을 때까지 기다린 뒤 close code 를 확인한다.
    wait_for_no_connections(&server).await;

    // close code 를 관측할 수 있으면 1011 이어야 한다(관측하지 못하는 것은 위 문서 참고).
    if let Some(code) = read_until_close(&mut client).await {
        assert_eq!(code, 1011, "ADR-0005 §2: SLOW_CONSUMER = 1011");
        println!("[SC-22] close code 1011 관측");
    } else {
        println!("[SC-22] close code 관측 불가(RST) — close_reason 으로 판정한다");
    }

    server.shutdown_and_join().await;
    let closed_events: Vec<_> = server
        .events()
        .into_iter()
        .filter(|(kind, _)| kind == "SESSION_CLOSED")
        .collect();
    assert!(
        closed_events
            .iter()
            .any(|(_, reason)| *reason == SessionCloseReason::SlowConsumer),
        "close_reason = SLOW_CONSUMER: {closed_events:?}"
    );
    println!("[SC-22] {sent}건 전송(send_failed={send_failed}) 후 SLOW_CONSUMER 로 닫힘");
}

// ---------------------------------------------------------------------------
// SC-26 / SC-27 — 유휴 종료와 정상 종료
// ---------------------------------------------------------------------------

/// SC-26 — 30초간 Pong·데이터 프레임이 없으면 close 1001 + `IDLE_TIMEOUT`.
///
/// 클라이언트가 **스트림을 폴링하지 않아야** 자동 Pong 이 나가지 않는다. 그래서 31초를
/// 그냥 기다렸다가 버퍼에 쌓인 프레임을 한꺼번에 읽는다.
#[tokio::test]
async fn sc26_idle_connection_is_closed_with_1001() {
    let mut server = TestServer::start(true).await;
    let token = server.token(SUBJECT);
    let mut client = connect(&server, Some(&token)).await.unwrap();
    assert_eq!(
        next_json(&mut client).await["message_type"],
        "SESSION_READY"
    );

    tokio::time::sleep(Duration::from_secs(32)).await;

    let code = read_until_close(&mut client).await;
    assert_eq!(code, Some(1001), "ADR-0005 §2: IDLE_TIMEOUT = 1001");

    wait_for_no_connections(&server).await;
    server.shutdown_and_join().await;
    let closed: Vec<_> = server
        .events()
        .into_iter()
        .filter(|(kind, _)| kind == "SESSION_CLOSED")
        .collect();
    assert!(
        closed
            .iter()
            .any(|(_, reason)| *reason == SessionCloseReason::IdleTimeout),
        "close_reason = IDLE_TIMEOUT: {closed:?}"
    );
    println!("[SC-26] 32초 무응답 → close 1001 + IDLE_TIMEOUT");
}

/// SC-27 / I-16 — 정상 종료는 살아 있던 **모든** 세션을 `SERVER_SHUTDOWN` 으로 닫는다.
#[tokio::test]
async fn sc27_shutdown_closes_every_open_session() {
    let mut server = TestServer::start(true).await;
    const SESSIONS: usize = 3;
    let mut clients = Vec::new();
    for index in 0..SESSIONS {
        let subject = format!("01a0b1c2-2c01-7a45-8b67-{:012x}", index);
        let token = server.token(&subject);
        let mut client = connect(&server, Some(&token)).await.unwrap();
        assert_eq!(
            next_json(&mut client).await["message_type"],
            "SESSION_READY"
        );
        clients.push(client);
    }

    tokio::time::sleep(Duration::from_millis(200)).await;
    let before = stats_json(&server).await;
    assert_eq!(
        before["ws_connections"], SESSIONS as u64,
        "레지스트리 실제 길이"
    );

    server.shutdown_and_join().await;

    let events = server.events();
    let opened = events
        .iter()
        .filter(|(kind, _)| kind == "SESSION_OPENED")
        .count();
    let shutdown_closed = events
        .iter()
        .filter(|(kind, reason)| {
            kind == "SESSION_CLOSED" && *reason == SessionCloseReason::ServerShutdown
        })
        .count();
    assert_eq!(opened, SESSIONS);
    assert_eq!(
        shutdown_closed, SESSIONS,
        "열려 있던 세션 수만큼 SERVER_SHUTDOWN (I-16)"
    );
    println!("[SC-27] 세션 {SESSIONS}건 전부 SERVER_SHUTDOWN 으로 닫힘");
}

/// 클라이언트의 **정상 Close** 는 즉시 `SESSION_CLOSED` 를 만들어야 한다.
///
/// 여기가 느리면 AC-17(b)("마지막 봇 종료 → 5초 내 전 행 가시")가 서버 탓으로 깨진다.
/// 실제로 첫 구현에서 정상 Close 경로만 약 15초(= ping 주기)가 걸렸다 — 소켓을 그냥
/// 끊는 경로는 즉시였기 때문에 통합 테스트로는 드러나지 않았다.
#[tokio::test]
async fn graceful_client_close_is_prompt() {
    let mut server = TestServer::start(true).await;
    let token = server.token(SUBJECT);
    let mut client = connect(&server, Some(&token)).await.unwrap();
    assert_eq!(
        next_json(&mut client).await["message_type"],
        "SESSION_READY"
    );

    let started = std::time::Instant::now();
    client.close(None).await.unwrap();
    while let Some(Ok(_)) = client.next().await {}
    wait_for_no_connections(&server).await;
    let elapsed = started.elapsed();
    println!(
        "[graceful-close] 정상 Close → 세션 종료까지 {:.3}초",
        elapsed.as_secs_f64()
    );
    assert!(
        elapsed < Duration::from_secs(2),
        "정상 Close 가 {elapsed:?} 걸렸다 — ping 주기까지 기다리고 있다"
    );

    server.shutdown_and_join().await;
    let closed: Vec<_> = server
        .events()
        .into_iter()
        .filter(|(kind, _)| kind == "SESSION_CLOSED")
        .collect();
    assert_eq!(
        closed,
        vec![(
            "SESSION_CLOSED".to_owned(),
            SessionCloseReason::ClientClosed
        )],
        "정상 Close 의 close_reason 은 CLIENT_CLOSED 다"
    );
}

/// SC-30 / SC-31 — 관측 항등식.
#[tokio::test]
async fn sc30_sc31_stats_identities_hold_at_rest() {
    let mut server = TestServer::start(true).await;
    let token = server.token(SUBJECT);
    let mut client = connect(&server, Some(&token)).await.unwrap();
    assert_eq!(
        next_json(&mut client).await["message_type"],
        "SESSION_READY"
    );

    const COMMANDS: u64 = 20;
    for n in 0..COMMANDS {
        client
            .send(ping_command(&command_id(7000 + n), n as u32))
            .await
            .unwrap();
    }
    let mut seen = 0;
    while seen < COMMANDS {
        if next_json(&mut client).await["message_type"] == "COMMAND_RESULT" {
            seen += 1;
        }
    }
    drop(client);
    wait_for_no_connections(&server).await;

    let body = stats_json(&server).await;
    let received = body["commands_received_total"].as_u64().unwrap();
    let enqueued_results = body["messages_enqueued_total"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["label"] == "COMMAND_RESULT")
        .unwrap()["count"]
        .as_u64()
        .unwrap();
    assert_eq!(
        received - enqueued_results,
        0,
        "AC-9(c) 서버 내부 1:1 불변식"
    );

    assert_eq!(body["ws_connections"], 0, "세션이 닫혔다");
    assert_eq!(
        body["ws_connections"].as_u64().unwrap(),
        body["sessions_opened_total"].as_u64().unwrap()
            - body["sessions_closed_total"].as_u64().unwrap(),
        "SC-30"
    );
    let (enqueued, written, dropped, depth) = (
        body["messages_enqueued_all"].as_u64().unwrap(),
        body["messages_written_all"].as_u64().unwrap(),
        body["messages_dropped_total"].as_u64().unwrap(),
        body["send_queue_depth"].as_u64().unwrap(),
    );
    assert_eq!(enqueued, written + dropped + depth, "SC-31 회계가 닫힌다");
    assert_eq!(depth, 0, "정지 시점 잔량 0");
    println!(
        "[SC-30/31] received={received} enqueued(COMMAND_RESULT)={enqueued_results} \
         enqueued_all={enqueued} written={written} dropped={dropped} depth={depth}"
    );

    server.shutdown_and_join().await;
}

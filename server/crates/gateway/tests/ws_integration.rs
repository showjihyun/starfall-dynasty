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
use std::time::{Duration, Instant};

use futures_util::{SinkExt, StreamExt};
use starfall_contracts::events::SessionCloseReason;
use starfall_contracts::primitives::{DataId, GameCalendar, GameTime, ServerVersion, UuidV7};
use starfall_gateway::{AppState, DevAuth, Probes, Stats, realtime_router};

use starfall_sim::world::{BoundaryConstants, ShipClassConstants};
use starfall_sim::{DomainEventBody, PersistBatch, ShipClassData, Simulation, WorldConstants};
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

/// p0-02 자산 테스트의 기본 `WorldConstants` — `max_entities_per_snapshot: 64`.
fn default_world() -> WorldConstants {
    WorldConstants {
        world_id: UuidV7::parse(WORLD_ID).unwrap(),
        calendar: GameCalendar::new(20, GameTime::parse("3800-01-01T00:00:00Z").unwrap(), 60)
            .unwrap(),
        server_version: ServerVersion::parse("0.1.0-test").unwrap(),
        star_system_id: DataId::parse("cradle").unwrap(),
        boundary: BoundaryConstants {
            soft_boundary_radius_m: 10_000.0,
            hard_boundary_radius_m: 12_000.0,
            boundary_pull_mps2: 25.0,
        },
        spawn_points_m: vec![[2500.0, 250.0, 0.0], [-2500.0, 250.0, 0.0]],
        spawn_clearance_m: 150.0,
        spawn_max_probe_attempts: 12,
        spawn_radial_offset_step_m: 150.0,
        spawn_world_seed: 42,
        linger_seconds: 30.0,
        reconnect_resume_window_seconds: 30.0,
        ship_class_id: DataId::parse("scout-s01").unwrap(),
        ship_class: ShipClassData {
            movement: ShipClassConstants {
                max_speed_mps: 140.0,
                main_thrust_mps2: 35.0,
                reverse_thrust_mps2: 18.0,
                lateral_thrust_mps2: 18.0,
                brake_decel_mps2: 50.0,
                assist_linear_decel_mps2: 7.0,
                assist_lateral_decel_mps2: 22.0,
                turn_rate_max_deg_s: 75.0,
                turn_accel_deg_s2: 220.0,
                turn_gain_deg_s_per_sin_half: 290.0,
                turn_deadzone_sin_half: 0.0009,
                roll_rate_max_deg_s: 90.0,
                roll_accel_deg_s2: 300.0,
                auto_level_rate_deg_s: 40.0,
                auto_level_deadzone_sin: 0.002,
            },
            hull_radius_m: 12.0,
        },
        // 이 파일의 테스트는 p0-02 자산이고 세션·명령 수명주기만 본다(스냅샷 자체는
        // `starfall-sim` 의 단위 테스트가 이미 덮는다). 실제 운영값(2, 100ms)을 쓰면
        // sc26 처럼 "일부러 읽지 않는" 테스트가 30초 동안 최대 300건의 스냅샷을 큐에
        // 쌓아 송신 큐(64, ADR-0011 §5)를 스냅샷만으로 넘겨 `SLOW_CONSUMER` 가
        // `IDLE_TIMEOUT` 보다 먼저 발동한다 — 이 항목이 보려는 것이 아니다. 200(10초)
        // 이면 이 파일의 가장 긴 테스트(32초)에서도 최대 3~4건뿐이라 간섭하지 않는다.
        snapshot_interval_ticks: 200,
        carry_forward_max_ticks: 10,
        rate_limit_per_tick_cap: 2,
        max_entities_per_snapshot: 64,
    }
}

impl TestServer {
    async fn start(auth_enabled: bool) -> Self {
        Self::start_with_world(auth_enabled, default_world()).await
    }

    /// [`start`] 와 같지만 `WorldConstants` 를 직접 넣는다 — `world_full`/재개 면제(I-44,
    /// S9)처럼 작은 `max_entities_per_snapshot` 이 필요한 테스트 전용.
    async fn start_with_world(auth_enabled: bool, world: WorldConstants) -> Self {
        Self::start_with_world_and_interval(auth_enabled, world, Duration::from_millis(50)).await
    }

    /// [`start_with_world`] 와 같지만 tick 간격을 직접 넣는다 — **판정을 지연시킨 tick을
    /// 주입**해 `TOO_MANY_IN_FLIGHT`가 구조적으로 도달 가능함을 보이는 테스트 전용(S10,
    /// ADR-0011 §5.2). 간격을 아주 길게 두면 그 창 안에서는 `sim.step()`이 돌지 않아
    /// `in_flight`가 절대 해제되지 않는다 — 실제 서버가 "밀린" 상황을 인위로 만든다.
    async fn start_with_world_and_interval(
        auth_enabled: bool,
        world: WorldConstants,
        tick_interval: Duration,
    ) -> Self {
        let stats = Stats::new();
        stats.set_start_tick(0);
        // `world_full` 게이트가 읽는 `world_capacity` 는 운영에서 `bins/game-server`의
        // 데이터 로딩 단계가 채운다(`set_data_loaded`) — 이 테스트 하네스는 그 단계를
        // 거치지 않으므로 직접 채워야 한다. 안 채우면 기본값(`u64::MAX`)이 남아
        // `world_full` 게이트가 이 파일의 어떤 테스트에서도 발동하지 않는다(i44 테스트
        // 작성 중 발견 — S9).
        stats.set_data_loaded(
            "test",
            1,
            u64::try_from(world.spawn_points_m.len()).unwrap_or(0),
            u64::from(world.snapshot_interval_ticks),
            u64::try_from(world.max_entities_per_snapshot).unwrap_or(u64::MAX),
        );

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
            tick_interval,
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

    /// `SESSION_OPENED`/`SESSION_CLOSED` 만 본다 — 이 테스트 스위트는 세션 수명주기를
    /// 검증하는 p0-02 자산이다. p1-01 부터 세션이 열리면 함선도 함께 스폰되므로(`SHIP_*`)
    /// 배치에 그 이벤트도 섞여 들어온다 — `filter_map` 으로 걸러낸다.
    fn events(&self) -> Vec<(String, SessionCloseReason)> {
        self.batches
            .lock()
            .unwrap()
            .iter()
            .flat_map(|batch| batch.events.iter())
            .filter_map(|event| match &event.body {
                DomainEventBody::SessionOpened(_) => Some((
                    "SESSION_OPENED".to_owned(),
                    SessionCloseReason::ClientClosed,
                )),
                DomainEventBody::SessionClosed(payload) => {
                    Some(("SESSION_CLOSED".to_owned(), payload.close_reason))
                }
                DomainEventBody::ShipSpawned(_) | DomainEventBody::ShipDespawned(_) => None,
            })
            .collect()
    }

    /// `SHIP_SPAWNED` 만 본다(`ship_id` 문자열) — 넘겨받기(takeover)가 함선을 복제하지
    /// 않는지 검증하는 전용 접근자. `events()`(p0-02 자산)는 이 바디를 버리므로 건드리지
    /// 않는다 — 새로 추가한다(팀 리더 요청, R4 S-6 §7a 짝 검증).
    fn ship_spawned_ids(&self) -> Vec<String> {
        self.batches
            .lock()
            .unwrap()
            .iter()
            .flat_map(|batch| batch.events.iter())
            .filter_map(|event| match &event.body {
                DomainEventBody::ShipSpawned(payload) => Some(payload.ship_id.to_string()),
                _ => None,
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
/// 다음 계약 메시지를 읽는다. **`WORLD_SNAPSHOT` 은 건너뛴다** — p1-01 부터 세션이 열리면
/// 함선이 함께 스폰되고 열려 있는 모든 세션에 주기적으로 스냅샷이 나간다(ADR-0011). 이
/// 파일의 테스트는 전부 p0-02 자산이라 세션·명령 수명주기만 본다 — 스냅샷을 보려면
/// (필요해지면) 별도 헬퍼를 둔다.
/// `next_json` 의 관용 버전 — 스트림이 끝나거나 소켓이 끊겨도 패닉하지 않고 `None` 을
/// 돌려준다. 연결이 **닫힐 수도 있는 것 자체가 이 테스트의 판정 대상**일 때만 쓴다
/// (SC-20 — 위 판단 보류 문서 참고). 다른 테스트는 전부 `next_json` 을 그대로 쓴다.
async fn try_next_json<S>(client: &mut S, timeout: Duration) -> Option<serde_json::Value>
where
    S: futures_util::Stream<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin,
{
    loop {
        let message = tokio::time::timeout(timeout, client.next())
            .await
            .ok()??
            .ok()?;
        if let Message::Text(text) = message {
            let value: serde_json::Value = serde_json::from_str(text.as_str()).ok()?;
            if value["message_type"] == "WORLD_SNAPSHOT" {
                continue;
            }
            return Some(value);
        }
    }
}

async fn next_json<S>(client: &mut S) -> serde_json::Value
where
    S: futures_util::Stream<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin,
{
    loop {
        let message = tokio::time::timeout(Duration::from_secs(5), client.next())
            .await
            .expect("메시지 대기 타임아웃")
            .expect("스트림이 끝났다")
            .expect("소켓 오류");
        if let Message::Text(text) = message {
            let value: serde_json::Value =
                serde_json::from_str(text.as_str()).expect("계약 메시지는 JSON 이다");
            if value["message_type"] == "WORLD_SNAPSHOT" {
                continue;
            }
            return value;
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

/// 세션 종료를 확인하는 폴링 간격.
const CLOSE_POLL_INTERVAL: Duration = Duration::from_millis(50);

/// 헬퍼의 폴 상한 — **`PING_INTERVAL` 에서 유도한다**(SV-1 d).
///
/// 이 상한이 지키는 진짜 경계는 "ping 주기까지 기다리고 있지 않은가"다
/// ([`starfall_gateway::ws::PING_INTERVAL`] = 15초). 예전에는 상한이 `300` 이라는
/// 맨숫자였고 `300 × 50 ms` 가 **우연히** 15초와 같았다 — 우연을 의도로 바꾼다.
/// `PING_INTERVAL` 을 바꾸면 이 상한이 따라 움직인다.
const CLOSE_POLL_LIMIT: usize =
    (starfall_gateway::ws::PING_INTERVAL.as_millis() / CLOSE_POLL_INTERVAL.as_millis()) as usize;

/// 세션이 실제로 닫힐 때까지 기다리고 **서버에 물어본 횟수**를 돌려준다.
///
/// close 프레임을 받은 것과 tick 이 `SESSION_CLOSED` 를 발행한 것은 다른 사건이다
/// (제출은 비동기이고 판정은 다음 tick 이다). 기다리지 않고 종료 스윕을 돌리면
/// `close_reason` 이 `SERVER_SHUTDOWN` 으로 덮인다.
///
/// # 왜 횟수를 돌려주는가 (SV-1, R4 사안 3 정정)
///
/// 이 헬퍼를 **벽시계로 감싸면 서버의 성질이 아니라 이 헬퍼의 폴링 비용을 잰다** —
/// 한 번의 폴이 `/debug/stats` HTTP 왕복 + 50 ms 수면이므로, 머신이 느려지면 서버가
/// 즉시 닫았어도 벽시계는 늘어난다.
///
/// ⚠ **정정 (R4, architect 1.9 사안 3)**: 폴이 세는 것은 "서버가 열려 있다고 답한
/// 횟수"가 **아니라** **"`ws_connections` 게이지가 아직 0이 아니었던 횟수"**다. 그
/// 게이지는 연결 태스크가 끝날 때(`submit.close` 뒤) 내려가므로, **소켓은 이미
/// 닫혔는데 게이지를 내릴 태스크가 굶어 늦게 도는 경우**에도 폴이 오른다 —
/// "실제로 늦게 닫힘"과 "게이지만 늦음"은 폴로는 구분되지 않는다. 그래도 폴 주기의
/// 고정 성분(50 ms 수면)은 부하로 늘지 않으므로, 부하는 이 값을 크게 올리지
/// 못한다(현실 부하에서 최대 관측 3 — ×80 극단 부하에서만 9~13까지 올랐다). 벽시계를
/// 단언에서 뺀 판단은 그대로 옳다 — 벽시계는 "우리가 묻는 데 걸린 시간"을 섞는다.
/// [`graceful_client_close_is_prompt`] 참고.
async fn wait_for_no_connections(server: &TestServer) -> usize {
    for polls in 1..=CLOSE_POLL_LIMIT {
        if stats_json(server).await["ws_connections"] == 0 {
            return polls;
        }
        tokio::time::sleep(CLOSE_POLL_INTERVAL).await;
    }
    panic!(
        "세션이 폴 {CLOSE_POLL_LIMIT}회(= PING_INTERVAL {}초) 안에 닫히지 않았다",
        starfall_gateway::ws::PING_INTERVAL.as_secs()
    );
}

async fn stats_json(server: &TestServer) -> serde_json::Value {
    let raw = reqwest_get(&format!("http://{}/debug/stats", server.addr)).await;
    let body = raw.split("\r\n\r\n").nth(1).unwrap_or(&raw);
    serde_json::from_str(body.trim()).unwrap()
}

/// SC-20 — tick당 명령 상한(8)을 넘긴 몰아보내기는 **연결을 끊지 않는다**.
///
/// **S10(ADR-0011 §5.2)으로 재설계됐다.** 이전 형태(in-flight 상한만으로 큐를 묶으려
/// 한 것)는 거부 응답도 큐 슬롯을 쓴다는 사실을 놓쳐, 워크스페이스 전체 병렬 실행에서
/// 5회 중 2회 간헐 실패했다(`03_server_impl.md` 기록). **tick당 명령 상한(8)이 진짜
/// 방어선이다** — 초과분은 큐에 들어가지도 않고 즉시(수신 태스크 안에서) 버려지므로,
/// 한 번에 몇 건을 보내든 그 세션이 큐에 넣을 수 있는 양은 tick당 `8 × 2(응답) = 16`건
/// 으로 **물리적으로 고정**된다 — burst 크기 `SENT` 와 무관하다(옛 문제의 근본 해결).
/// 그래서 이 테스트는 **정상 tick 간격(50 ms)에서** 몰아 보내고 손실 항등식
/// (`보낸 수 = COMMAND_RESULT 수 + commands_dropped_over_tick_cap_total`, 서버 쪽
/// `/debug/stats` 로 교차 검증)과 "연결이 끝까지 열려 있다"를 **엄격하게** 단언한다 —
/// 더는 완화할 필요가 없다.
#[tokio::test]
async fn sc20_tick_command_cap_drops_excess_without_closing() {
    let mut server = TestServer::start(true).await;
    let token = server.token(SUBJECT);
    let mut client = connect(&server, Some(&token)).await.unwrap();
    assert_eq!(
        next_json(&mut client).await["message_type"],
        "SESSION_READY"
    );

    let dropped_before = stats_json(&server).await["commands_dropped_over_tick_cap_total"]
        .as_u64()
        .unwrap();

    // 한 tick(50 ms) 안에 상한(8)을 넘겨 보낸다 — 옛 SC-20 과 같은 규모(120)로, "burst 크기와
    // 무관하게 안전하다"는 것을 같은 수치로 보인다.
    const SENT: u64 = 120;
    for n in 0..SENT {
        client
            .send(ping_command(&command_id(3000 + n), n as u32))
            .await
            .unwrap();
    }

    let mut results = 0u64;
    let mut seen = std::collections::HashSet::new();
    // 상한을 넘긴 만큼은 응답이 아예 오지 않으므로 "SENT 건 받을 때까지" 기다릴 수 없다 —
    // 대신 새 메시지가 1초 동안 없으면(전부 처리된 것으로 보고) 멈춘다.
    while let Some(message) = try_next_json(&mut client, Duration::from_secs(1)).await {
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
        assert_ne!(
            message["payload"]["reason_code"], "TOO_MANY_IN_FLIGHT",
            "tick당 상한이 먼저 걸러야 한다 — in-flight 상한까지 닿으면 안 된다"
        );
    }

    // 연결이 여전히 열려 있는지 새 명령으로 확인한다.
    let id = command_id(9999);
    client.send(ping_command(&id, 1)).await.unwrap();
    loop {
        let message = next_json(&mut client).await;
        if message["message_type"] == "COMMAND_RESULT" && message["payload"]["command_id"] == id {
            assert_eq!(message["payload"]["status"], "ACCEPTED");
            break;
        }
    }

    let dropped_after = stats_json(&server).await["commands_dropped_over_tick_cap_total"]
        .as_u64()
        .unwrap();
    let dropped = dropped_after - dropped_before;

    assert_eq!(
        SENT,
        results + dropped,
        "손실 항등식이 깨졌다: 보낸 수({SENT}) != COMMAND_RESULT({results}) + \
         commands_dropped_over_tick_cap_total({dropped})"
    );
    assert!(dropped > 0, "상한을 넘겼는데 드롭이 하나도 없었다");
    println!(
        "[SC-20] 보낸 {SENT}건 = COMMAND_RESULT {results}건 + tick-cap 드롭 {dropped}건, 연결 유지"
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
/// **S10(ADR-0011 §5.2)으로 트리거 경로가 바뀌었다.** 예전엔 명령을 읽지 않고 몰아
/// 보내 `COMMAND_RESULT`로 송신 큐를 채웠지만, 이제 tick당 명령 상한(8)이 초과분을
/// 큐에 넣기 **전에** 버려 그 경로로는 더 이상 큐를 채울 수 없다(SC-20 절 참고). 대신
/// ADR-0011 §5.2가 명시한 대로 **`WORLD_SNAPSHOT`은 클라이언트 행동과 무관하게
/// 밀려들어간다** — 이 테스트는 이제 **아무것도 보내지 않고 그냥 읽지 않는 것만으로**
/// SLOW_CONSUMER 를 유발한다. 실운영 스냅샷 주기(`snapshot_interval_ticks`)를 그대로
/// 쓰려고 이 테스트만 전용 `WorldConstants`(간격 1 tick = 20 Hz)로 서버를 띄운다 —
/// `default_world()`의 200(10초)은 다른 테스트가 스냅샷 간섭을 피하려고 일부러 늘린
/// 값이라 여기서는 맞지 않는다.
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
    let mut world = default_world();
    world.snapshot_interval_ticks = 1; // 20 Hz — 64슬롯 큐가 3.2초면 스냅샷만으로 찬다.
    let mut server = TestServer::start_with_world(true, world).await;
    let token = server.token(SUBJECT);
    let mut client = connect(&server, Some(&token)).await.unwrap();
    assert_eq!(
        next_json(&mut client).await["message_type"],
        "SESSION_READY"
    );

    // 여기서부터 **아무것도 보내지 않고, 아무것도 읽지 않는다.** WORLD_SNAPSHOT 이
    // 클라이언트 행동과 무관하게 20 Hz 로 밀려들어와 송신 큐(64)를 채운다.
    //
    // **6초로는 부족했다(실측 — 5회 중 4회 15초 대기에서도 닫히지 않아 타임아웃).**
    // 이론상 64 슬롯 ÷ 20 msg/s = 3.2 초지만, 실제로는 그보다 훨씬 오래 걸린다 — 클라이언트가
    // 안 읽어도 **OS TCP 송신 버퍼가 먼저 흡수**하기 때문이다: 게이트웨이의 쓰기 태스크는
    // mpsc 채널에서 메시지를 계속 꺼내 소켓에 쓰려 시도하고, mpsc 채널(64슬롯)은 그 시도
    // 자체가 TCP 송신 버퍼 포화로 막혀야 비로소 차기 시작한다. TCP 버퍼가 수십 KB면 그것만
    // 흡수하는 데도 수십 개의 스냅샷(수 초)이 더 필요하다. 30초(600개 분량)로 넉넉히 잡는다.
    tokio::time::sleep(Duration::from_secs(30)).await;

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
    println!("[SC-22] 6초간 미수신 후 SLOW_CONSUMER 로 닫힘 (스냅샷 20 Hz, 트리거 경로: S10)");
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

/// I-44 / S9 — `world_full` 게이트는 재개를 면제한다.
///
/// 정원(`max_entities_per_snapshot`)을 1로 좁혀 실제로 채운다. (1) 정원이 차면 **함선이
/// 없는** actor 는 503 `world_full` 로 막힌다. (2) 정원을 채운 actor 가 연결을 끊어도
/// (함선은 잔류로 남는다 — 세계 함선 수는 그대로 1) **같은 actor 가 재접속하면 막히지
/// 않는다.** (3) 그 사이 다른 actor 는 여전히 막힌다 — 면제는 "이 actor가 함선을
/// 가졌는가"로만 판정되지, 세계에 자리가 있는가로 판정되지 않는다.
#[tokio::test]
async fn i44_world_full_exempts_a_resuming_actor() {
    let mut world = default_world();
    world.max_entities_per_snapshot = 1;
    let mut server = TestServer::start_with_world(true, world).await;

    const ALPHA: &str = "01a0b1c2-2c01-7a45-8b67-0000000000a1";
    const BRAVO: &str = "01a0b1c2-2c01-7a45-8b67-0000000000b2";

    // alpha 가 접속해 정원(1)을 채운다.
    let alpha_token = server.token(ALPHA);
    let mut alpha = connect(&server, Some(&alpha_token)).await.unwrap();
    assert_eq!(next_json(&mut alpha).await["message_type"], "SESSION_READY");
    tokio::time::sleep(Duration::from_millis(100)).await; // tick 이 스폰을 반영할 시간

    // bravo(함선 없음) — 정원이 찼으므로 막힌다.
    let bravo_token = server.token(BRAVO);
    assert_eq!(
        connect(&server, Some(&bravo_token)).await.unwrap_err(),
        503,
        "함선 없는 actor 는 정원이 차면 503 world_full 이어야 한다"
    );

    // alpha 가 끊는다 — 함선은 잔류로 남는다(세계 함선 수 그대로 1).
    alpha.close(None).await.unwrap();
    while let Some(Ok(_)) = alpha.next().await {}
    wait_for_no_connections(&server).await;

    // bravo 는 여전히 막힌다 — 면제는 "이 actor가 함선을 가졌는가"로만 판정된다.
    assert_eq!(
        connect(&server, Some(&bravo_token)).await.unwrap_err(),
        503,
        "정원이 찬 동안 다른(함선 없는) actor 는 계속 막혀야 한다"
    );

    // alpha 가 같은 토큰으로 재접속 — 정원은 여전히 찼지만(잔류 함선 1) 재개는 면제된다.
    let mut alpha_again = connect(&server, Some(&alpha_token)).await.unwrap();
    assert_eq!(
        next_json(&mut alpha_again).await["message_type"],
        "SESSION_READY",
        "정원이 찬 상태에서도 자기 함선으로의 재개는 막히면 안 된다(I-44)"
    );

    server.shutdown_and_join().await;
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

/// `ws_connections` 게이지가 정상 Close 뒤에도 아직 0이 아니어도 되는 폴 횟수 (SV-1 b,
/// R4 사안 3 정정).
///
/// **이것은 시간 예산이 아니다.** 정상값은 0~1회(= 폴 1~2회)이고, 이 수가 늘어나는
/// 것은 게이지가 아직 0이 아니었을 때뿐이다 — 머신이 느려도 "물어본 횟수"는 늘지
/// 않는다(각 물음이 느려질 뿐이다). 그래서 10 은 "느린 머신을 봐주는 여유"가 아니라
/// **tick 1~2개 분량의 정상 편차에 대한 여유**이고, 진짜 회귀(ping 주기까지 기다리는
/// 15초 = 폴 300회)와는 30배 떨어져 있다.
///
/// ⚠ **정정 (R4)**: 이 값이 오르는 것이 곧 "서버가 실제로 늦게 닫았다"는 아니다 —
/// 게이지를 내리는 태스크가 굶어도 오른다. `wait_for_no_connections` 의 문서 참고.
const GRACEFUL_CLOSE_POLL_BUDGET: usize = 10;

/// 클라이언트의 **정상 Close** 는 즉시 `SESSION_CLOSED` 를 만들어야 한다.
///
/// 여기가 느리면 AC-17(b)("마지막 봇 종료 → 5초 내 전 행 가시")가 서버 탓으로 깨진다.
/// 실제로 첫 구현에서 정상 Close 경로만 약 15초(= ping 주기)가 걸렸다 — 소켓을 그냥
/// 끊는 경로는 즉시였기 때문에 통합 테스트로는 드러나지 않았다.
///
/// # 무엇을 재는가 — 벽시계가 아니라 폴 횟수다 (SV-1, architect 판정, R4 사안 3 정정)
///
/// 예전 형태는 `elapsed < 2초`였고 **그 `elapsed` 가 `wait_for_no_connections` 를
/// 포함했다.** 폴 1회 = `/debug/stats` HTTP 왕복 + 50 ms 수면이므로 정상 실측
/// 0.056~0.078초는 **폴 1~2회**, 즉 이 측정의 **해상도 바닥**이었다 — 서버의 실제
/// 종료 지연은 이 테스트가 볼 수 있는 값보다 작다. 그러면서 벽시계는 "우리가 묻는 데
/// 걸린 시간"을 함께 재므로, 머신이 붐비면 **서버가 즉시 닫았는데도** 2초를 넘길 수
/// 있다(실측: 워크스페이스 병렬 11회 중 1회, ×80 합성 부하 8회 중 7회).
///
/// ⚠ **정정 (R4, architect 1.9 사안 3)**: 폴이 세는 것은 "서버가 아직 열려 있다고
/// 답한 횟수"가 아니라 **"`ws_connections` 게이지가 아직 0이 아니었던 횟수"**다 —
/// "실제로 늦게 닫힘"과 "게이지를 내리는 태스크만 늦게 돎"은 폴로는 구분되지 않는다.
/// 그래도 벽시계를 단언에서 뺀 판단은 그대로 옳다: 폴 주기의 고정 성분(50 ms 수면)은
/// 부하로 늘지 않으므로, 부하는 벽시계처럼 이 값을 인플레하지 못한다. 벽시계는
/// 출력에만 남긴다.
#[tokio::test]
async fn graceful_client_close_is_prompt() {
    let mut server = TestServer::start(true).await;
    let token = server.token(SUBJECT);
    let mut client = connect(&server, Some(&token)).await.unwrap();
    assert_eq!(
        next_json(&mut client).await["message_type"],
        "SESSION_READY"
    );

    let started = Instant::now();
    client.close(None).await.unwrap();
    while let Some(Ok(_)) = client.next().await {}
    let polls = wait_for_no_connections(&server).await;
    let elapsed = started.elapsed();
    // R4 사안 3 정정: 이 값은 "서버가 아직 열려 있다고 답한 횟수"가 아니라 **게이지가
    // 아직 0이 아니었던 횟수**다 — 이름을 그에 맞춘다(architect 1.9 제안).
    let gauge_nonzero_polls = polls - 1;
    println!(
        "[graceful-close] `ws_connections` 게이지가 아직 0이 아니었던 폴 수 = \
         {gauge_nonzero_polls} (폴 {polls}회 / 상한 {CLOSE_POLL_LIMIT}). 벽시계 {:.3}초 — \
         **기록용이고 단언 대상이 아니다**",
        elapsed.as_secs_f64()
    );
    // SV-1 (b)(c): 단언은 **폴 횟수**로 한다. 벽시계는 위 출력에만 남기고 단언에서는
    // 뺀다(architect R3 판정 1, R4 사안 3이 전제를 정정했다 — 근거는 위 함수 문서 참고).
    assert!(
        polls <= GRACEFUL_CLOSE_POLL_BUDGET,
        "`ws_connections` 게이지가 폴 {polls}회 동안 0이 아니었다(예산 \
         {GRACEFUL_CLOSE_POLL_BUDGET}). 회귀(ping 주기 대기)라면 약 300회다. 극단 부하에서는 \
         게이지를 내리는 태스크의 기아로도 오르므로, 독립 시각(예: close 를 보낸 시점의 tick \
         대비 SESSION_CLOSED 의 tick) 없이 원인을 단정하지 말 것(architect 1.9 사안 3)."
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

    // S10(ADR-0011 §5.2): tick당 세션별 상한이 8이다. 한 tick 안에 다 들어가도록 그보다
    // 작게 보낸다 — 이 테스트의 목적은 회계 항등식이지 상한 자체가 아니다(그건 SC-20).
    const COMMANDS: u64 = 5;
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

// ---------------------------------------------------------------------------
// p1-01 R2 / S-A — 게이트웨이 인바운드 디스패치
//
// 라운드 2 QA 가 실서버에서 찾은 최대 결함: `SET_SHIP_CONTROL` 이 게이트웨이에서
// `UNKNOWN_COMMAND_TYPE` 으로 통째로 거부됐고, **이 파일이 그 명령을 한 번도 보내지
// 않았기 때문에** 154개 테스트가 전부 초록인 채로 살아남았다. 아래 두 테스트가 그
// 맹점을 닫는다:
//
// 1. `set_ship_control_over_a_real_socket_is_accepted_and_acked` — 실제 소켓으로
//    한 건 보내고 `ACCEPTED` + `commands_received_total` 증가 + 스냅샷의
//    `ack_input_seq` 까지 본다.
// 2. `every_registry_command_type_passes_through_the_gateway` — 레지스트리의
//    `kind == "command"` **전부**를 소켓으로 보낸다. 새 명령이 계약에 들어오는데
//    프레임 빌더가 없으면 여기서 실패한다.
// ---------------------------------------------------------------------------

/// 유효한 `SET_SHIP_CONTROL` 프레임. `aim_*` 는 단위 쿼터니언(0,0,0,1)의 micro 표현.
fn set_ship_control_command(command_id: &str, input_seq: u32, thrust_z_milli: i32) -> Message {
    Message::Text(Utf8Bytes::from(format!(
        r#"{{"command_id":"{command_id}","command_type":"SET_SHIP_CONTROL","schema_version":1,"client_sent_at":null,"payload":{{"input_seq":{input_seq},"thrust_x_milli":0,"thrust_y_milli":0,"thrust_z_milli":{thrust_z_milli},"roll_milli":0,"aim_x_micro":0,"aim_y_micro":0,"aim_z_micro":0,"aim_w_micro":1000000,"brake":false,"flight_assist":true}}}}"#
    )))
}

/// 레지스트리 이름 → 그 타입의 **유효한** 명령 프레임.
///
/// `None` 은 "이 스위트가 그 명령을 소켓으로 보낼 줄 모른다"는 뜻이고, 아래 순회
/// 테스트가 그것을 실패로 만든다. **새 명령을 계약에 넣으면 여기도 넣어야 한다.**
fn command_frame(registry_name: &str, command_id: &str, n: u32) -> Option<Message> {
    match registry_name {
        starfall_contracts::registry::PING_SERVER => Some(ping_command(command_id, n)),
        starfall_contracts::registry::SET_SHIP_CONTROL => {
            Some(set_ship_control_command(command_id, n.max(1), 500))
        }
        _ => None,
    }
}

/// 스냅샷을 자주 내보내는 월드 — `ack_input_seq` 를 테스트 시간 안에 보려면 필요하다
/// (`default_world` 의 200 tick = 10초는 너무 느리다).
fn world_with_fast_snapshots() -> WorldConstants {
    WorldConstants {
        snapshot_interval_ticks: 2,
        ..default_world()
    }
}

/// 다음 `WORLD_SNAPSHOT` 을 받는다 (다른 메시지는 건너뛴다).
async fn next_snapshot(client: &mut Client, timeout: Duration) -> Option<serde_json::Value> {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            return None;
        }
        let message = tokio::time::timeout(remaining, client.next())
            .await
            .ok()??
            .ok()?;
        if let Message::Text(text) = message {
            let value: serde_json::Value = serde_json::from_str(text.as_str()).ok()?;
            if value["message_type"] == "WORLD_SNAPSHOT" {
                return Some(value);
            }
        }
    }
}

/// S-A — `SET_SHIP_CONTROL` 이 **소켓을 통해** 판정에 도달한다.
///
/// QA 라운드 2 의 실측 반대편이다: 그때는 `rejected{UNKNOWN_COMMAND_TYPE}` 200건에
/// `commands_received_total` 델타가 **0** 이었다(큐에 넣으려 시도조차 하지 않았다).
/// 그래서 이 테스트는 `ACCEPTED` 만 보지 않고 **그 카운터의 증가**를 함께 단언한다.
#[tokio::test]
async fn set_ship_control_over_a_real_socket_is_accepted_and_acked() {
    let mut server = TestServer::start_with_world(true, world_with_fast_snapshots()).await;
    let token = server.token(SUBJECT);
    let mut client = connect(&server, Some(&token)).await.expect("101 기대");
    assert_eq!(
        next_json(&mut client).await["message_type"],
        "SESSION_READY"
    );

    let before = stats_json(&server).await["commands_received_total"]
        .as_u64()
        .unwrap();

    const INPUT_SEQ: u32 = 7;
    client
        .send(set_ship_control_command(&command_id(9101), INPUT_SEQ, 800))
        .await
        .unwrap();

    let result = next_json(&mut client).await;
    assert_eq!(result["message_type"], "COMMAND_RESULT");
    assert_eq!(
        result["payload"]["status"], "ACCEPTED",
        "게이트웨이가 SET_SHIP_CONTROL 을 판정에 넣어야 한다 (S-A). 받은 값: {result}"
    );
    assert_eq!(result["payload"]["command_id"], command_id(9101));
    assert!(
        result["payload"]["reason_code"].is_null(),
        "수락에는 reason_code 가 없다: {result}"
    );

    let after = stats_json(&server).await["commands_received_total"]
        .as_u64()
        .unwrap();
    assert_eq!(
        after - before,
        1,
        "commands_received_total 이 실제로 올라야 한다 — QA 라운드 2 의 델타는 0 이었다"
    );

    // `ack_input_seq` 는 "서버가 마지막으로 적용한 input_seq" 다 (SC-31).
    let snapshot = next_snapshot(&mut client, Duration::from_secs(5))
        .await
        .expect("스냅샷이 와야 한다");
    assert_eq!(
        snapshot["payload"]["ack_input_seq"], INPUT_SEQ,
        "적용된 input_seq 가 스냅샷으로 되돌아와야 한다: {snapshot}"
    );

    drop(client);
    wait_for_no_connections(&server).await;
    server.shutdown_and_join().await;
}

/// M-17 — **계약에 있는 명령 타입 전부가 이 소켓 경로를 최소 1회 지난다.**
///
/// 라운드 2 의 교훈은 "통과한 테스트 수는 경로가 실행됐다는 증거가 아니다" 였다.
/// 레지스트리를 순회해 각 명령을 실제로 보내고 **`commands_received_total` 이 그
/// 건수만큼 오르는 것**을 본다 — 거부는 그 카운터를 올리지 않으므로, 게이트웨이가
/// 어떤 명령을 모르면 여기서 반드시 실패한다.
#[tokio::test]
async fn every_registry_command_type_passes_through_the_gateway() {
    let commands: Vec<&'static str> = starfall_contracts::registry::CONTRACT_TYPES
        .iter()
        .filter(|entry| entry.kind == "command")
        .map(|entry| entry.name)
        .collect();
    assert!(!commands.is_empty(), "레지스트리에 명령 타입이 있어야 한다");

    let mut server = TestServer::start(true).await;
    let token = server.token(SUBJECT);

    for (index, name) in commands.iter().enumerate() {
        // 명령마다 새 세션을 연다 — `input_seq` 단조 증가·tick당 상한처럼 세션에
        // 붙은 상태가 타입끼리 간섭하지 않게 한다.
        let mut client = connect(&server, Some(&token)).await.expect("101 기대");
        assert_eq!(
            next_json(&mut client).await["message_type"],
            "SESSION_READY"
        );

        let id = command_id(9200 + index as u64);
        let frame = command_frame(name, &id, 1).unwrap_or_else(|| {
            panic!(
                "{name} 은 계약의 명령 타입인데 이 통합 스위트가 보낼 줄 모른다 — \
                 `command_frame` 에 추가하라. 게이트웨이를 한 번도 지나지 않은 명령이 \
                 남는 것이 라운드 2 의 S-A 결함이었다"
            )
        });

        let before = stats_json(&server).await["commands_received_total"]
            .as_u64()
            .unwrap();
        client.send(frame).await.unwrap();

        let result = next_json(&mut client).await;
        assert_eq!(result["message_type"], "COMMAND_RESULT", "{name}");
        assert_eq!(
            result["payload"]["status"], "ACCEPTED",
            "{name} 이 게이트웨이에서 거부됐다: {result}"
        );
        let after = stats_json(&server).await["commands_received_total"]
            .as_u64()
            .unwrap();
        assert_eq!(
            after - before,
            1,
            "{name} 이 큐에 들어가려 **시도**조차 하지 않았다 (commands_received_total 델타 0)"
        );

        drop(client);
        wait_for_no_connections(&server).await;
    }

    println!("[M-17] 소켓을 지난 명령 타입: {commands:?}");
    server.shutdown_and_join().await;
}

// ---------------------------------------------------------------------------
// p1-01 R3 §4.5 — tick 층 거부가 `commands_rejected_total` 을 세지 않는다
//
// QA 가 실서버에서 두 독립 출처(봇이 받은 `COMMAND_RESULT` vs `/debug/stats` 델타)의
// 어긋남으로 찾았다: `STALE_INPUT`·`RATE_LIMITED` 는 `simulation.rs` 에서
// `CommandResultPayload::rejected` 로 만들어져 `runtime.rs::route_outbound` 를 그냥
// 지나갔다 — 그 루프는 `record_message_enqueued` 만 부르고 거부 사유를 세지 않았다.
//
// **같은 맹점이 `DUPLICATE_COMMAND_ID` 에도 있다** — dedup 판정도 같은 tick 층 함수에서
// 일어나고(`sc21_duplicate_command_id_yields_one_ping_reply` 가 소켓으로는 이미
// 관측하고 있었다) 같은 이유로 스택 델타는 세지 않는다. 이 테스트가 셋을 함께 고정한다.
//
// architect R3 판정의 규율을 그대로 코드로 옮긴다: **라벨 델타만 보지 않는다.** 클라이언트가
// 실제로 받은 그 `reason_code` 의 `COMMAND_RESULT` 개수와 짝짓고, **두 수가 같아야 한다.**
// ---------------------------------------------------------------------------

/// `/debug/stats` 의 `commands_rejected_total` 에서 라벨 하나의 카운트를 뽑는다.
fn rejected_count(body: &serde_json::Value, label: &str) -> u64 {
    body["commands_rejected_total"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["label"] == label)
        .unwrap_or_else(|| panic!("commands_rejected_total 에 {label} 라벨이 없다"))["count"]
        .as_u64()
        .unwrap()
}

/// SC-33 — tick 층이 만드는 거부(`DUPLICATE_COMMAND_ID`·`RATE_LIMITED`·`STALE_INPUT`)도
/// `commands_rejected_total` 에 반영된다.
#[tokio::test]
async fn sc33_tick_layer_rejections_are_counted() {
    let mut server = TestServer::start(true).await;
    let token = server.token(SUBJECT);
    let mut client = connect(&server, Some(&token)).await.unwrap();
    assert_eq!(
        next_json(&mut client).await["message_type"],
        "SESSION_READY"
    );

    let before = stats_json(&server).await;

    // --- DUPLICATE_COMMAND_ID: 같은 command_id 2회. PING_SERVER 를 쓴다 —
    // `step_ship_control` 만 세션의 tick당 처리 카운터를 올리므로, 아래 RATE_LIMITED
    // 유도와 서로 간섭하지 않는다.
    let dup_id = command_id(90_000);
    client.send(ping_command(&dup_id, 1)).await.unwrap();
    client.send(ping_command(&dup_id, 1)).await.unwrap();

    // --- RATE_LIMITED: `default_world().rate_limit_per_tick_cap`(= 2) 을 넘겨 한 tick
    // 안에서 `SET_SHIP_CONTROL` 5건을 보낸다. 세션별 tick당 처리 한도를 넘긴 뒤 3건이
    // `RATE_LIMITED` 다(처리 순서상 앞의 2건은 상한 안이라 후보로 접수된다).
    const RATE_BURST: u32 = 5;
    const RATE_LIMIT_CAP: u32 = 2;
    for n in 1..=RATE_BURST {
        client
            .send(set_ship_control_command(
                &command_id(91_000 + u64::from(n)),
                n,
                500,
            ))
            .await
            .unwrap();
    }

    let mut duplicate_seen = 0u64;
    let mut rate_limited_seen = 0u64;
    let mut command_results = 0u32;
    // 이번 라운드의 COMMAND_RESULT 는 PING 2건 + SET_SHIP_CONTROL 5건 = 7건이다.
    while command_results < 2 + RATE_BURST {
        let message = next_json(&mut client).await;
        if message["message_type"] != "COMMAND_RESULT" {
            continue;
        }
        command_results += 1;
        match message["payload"]["reason_code"].as_str() {
            Some("DUPLICATE_COMMAND_ID") => duplicate_seen += 1,
            Some("RATE_LIMITED") => rate_limited_seen += 1,
            _ => {}
        }
    }
    assert_eq!(duplicate_seen, 1, "PING_SERVER 중복 1건이 거부돼야 한다");
    assert_eq!(
        rate_limited_seen,
        u64::from(RATE_BURST - RATE_LIMIT_CAP),
        "tick당 상한({RATE_LIMIT_CAP})을 넘긴 {}건이 RATE_LIMITED 여야 한다",
        RATE_BURST - RATE_LIMIT_CAP
    );

    // --- STALE_INPUT: 다음 tick 까지 기다린 뒤(세션의 tick당 처리 카운터가 리셋된다),
    // 방금 받아들여진 것보다 작은 input_seq 를 **혼자** 보낸다 — 한 tick에 1건뿐이므로
    // RATE_LIMITED 로 새지 않고, seq 가 마지막 적용값(RATE_LIMIT_CAP=2)보다 작아 Stale 이다.
    tokio::time::sleep(Duration::from_millis(150)).await;
    client
        .send(set_ship_control_command(&command_id(92_000), 1, 500))
        .await
        .unwrap();
    let stale_result = next_json(&mut client).await;
    assert_eq!(stale_result["message_type"], "COMMAND_RESULT");
    assert_eq!(stale_result["payload"]["status"], "REJECTED");
    assert_eq!(stale_result["payload"]["reason_code"], "STALE_INPUT");
    let stale_seen = 1u64;

    let after = stats_json(&server).await;
    let duplicate_delta = rejected_count(&after, "DUPLICATE_COMMAND_ID")
        - rejected_count(&before, "DUPLICATE_COMMAND_ID");
    let rate_limited_delta =
        rejected_count(&after, "RATE_LIMITED") - rejected_count(&before, "RATE_LIMITED");
    let stale_delta =
        rejected_count(&after, "STALE_INPUT") - rejected_count(&before, "STALE_INPUT");

    // 두 독립 출처가 같아야 한다 (QA 의 대조표를 테스트 안으로 옮긴 형태) — 존재만으로
    // PASS 를 주지 않는다.
    assert_eq!(
        duplicate_delta, duplicate_seen,
        "commands_rejected_total{{DUPLICATE_COMMAND_ID}} 델타가 실제 수신과 어긋난다"
    );
    assert_eq!(
        rate_limited_delta, rate_limited_seen,
        "commands_rejected_total{{RATE_LIMITED}} 델타가 실제 수신과 어긋난다"
    );
    assert_eq!(
        stale_delta, stale_seen,
        "commands_rejected_total{{STALE_INPUT}} 델타가 실제 수신과 어긋난다"
    );
    assert!(duplicate_delta >= 1 && rate_limited_delta >= 1 && stale_delta >= 1);

    println!(
        "[SC-33] DUPLICATE_COMMAND_ID delta={duplicate_delta}, RATE_LIMITED delta={rate_limited_delta}, \
         STALE_INPUT delta={stale_delta} — 전부 실제 수신과 일치"
    );

    server.shutdown_and_join().await;
}

// ---------------------------------------------------------------------------
// R4 S-6 — 동시 세션 넘겨받기 (사용자 결정 5, ADR-0011 §6.3, ADR-0005 §2)
// ---------------------------------------------------------------------------

/// 같은 actor 의 두 번째 접속이 들어오면 **먼저 연결한 쪽**이 close **4001** 을 받고,
/// 그 close_reason 은 `SUPERSEDED` 다. 둘째 연결은 함선을 넘겨받아 정상 명령을 계속
/// 받아들인다.
#[tokio::test]
async fn r4_s6_first_connection_is_closed_with_4001_when_superseded() {
    let mut server = TestServer::start(true).await;
    let token = server.token(SUBJECT);

    let mut first = connect(&server, Some(&token)).await.unwrap();
    assert_eq!(next_json(&mut first).await["message_type"], "SESSION_READY");

    // 같은 actor 로 두 번째 연결 — 넘겨받기를 유발한다.
    let mut second = connect(&server, Some(&token)).await.unwrap();
    assert_eq!(
        next_json(&mut second).await["message_type"],
        "SESSION_READY"
    );

    // 첫 연결이 close 4001 을 받아야 한다(ADR-0005 §2).
    let code = read_until_close(&mut first).await;
    assert_eq!(code, Some(4001), "ADR-0005 §2: SUPERSEDED = 4001");

    // 둘째 연결은 살아 있다 — 함선을 넘겨받았으니 정상 명령을 계속 받아들여야 한다.
    let id = command_id(9700);
    second.send(ping_command(&id, 1)).await.unwrap();
    let result = next_json(&mut second).await;
    assert_eq!(result["message_type"], "COMMAND_RESULT");
    assert_eq!(
        result["payload"]["status"], "ACCEPTED",
        "둘째 연결이 조종을 넘겨받았으면 정상 명령이 거부될 이유가 없다"
    );

    server.shutdown_and_join().await;

    let closed: Vec<_> = server
        .events()
        .into_iter()
        .filter(|(kind, _)| kind == "SESSION_CLOSED")
        .collect();
    assert!(
        closed
            .iter()
            .any(|(_, reason)| *reason == SessionCloseReason::Superseded),
        "첫 세션의 close_reason 이 SUPERSEDED 여야 한다 — 관측된 사유: {closed:?}"
    );
    println!("[R4 S-6] 첫 연결 close 4001, close_reason SUPERSEDED 확인");
}

/// R4 S-6 §7a 짝 — 위 테스트가 재는 것은 `ping_command` 가 `ACCEPTED` 되는 것뿐이었다.
/// ping 은 함선을 건드리지 않으므로 그 단언은 둘째 세션이 함선을 **새로** 스폰하는
/// 빌드에서도 똑같이 통과한다(팀 리더 지적). 이 테스트는 겨냥한 조건 — "함선이 둘이
/// 아니라 하나이고, 둘째 세션이 그 같은 함선을 넘겨받는다" — 을 직접 단언한다.
///
/// `TestServer::events()` 는 `SHIP_*` 를 버리므로(p0-02 자산, 주석 참고) 쓰지 않는다 —
/// [`TestServer::ship_spawned_ids`] 를 새로 쓴다.
///
/// **디버그 빌드에서 이 테스트가 빨개지는 걸 봤다고 이 단언이 하중을 받는다고 믿지 마라.**
/// `Simulation::debug_assert_i29`(`crates/sim/src/simulation.rs`)는
/// `#[cfg(debug_assertions)]` 라 `cargo test`(디버그)에서는 I-29 불변식 위반을 tick
/// 스레드가 먼저 panic 으로 잡는다 — 그러면 이 함수의 `assert_eq!(spawned.len(), 1, …)`
/// 는 평가되기도 전에 프로세스가 죽는다. **릴리스 빌드에는 그 그물이 없다** — 복제를
/// 잡을 것은 이 단언뿐이다. 실제로 `cargo test -p starfall-gateway --release`로 넘겨받기
/// 분기를 무력화해 확인했다: FAIL 이 panic 이 아니라 이 단언의 메시지였다
/// (`SHIP_SPAWNED` 2건, `left: 2 right: 1`) — 그 실행 결과가 이 단언이 릴리스에서
/// 실제로 하중을 받는다는 증거다(qa 요청, 2026-09-25).
#[tokio::test]
async fn r4_s6_second_connection_takes_over_the_ship_not_a_second_one() {
    let mut server = TestServer::start(true).await;
    let token = server.token(SUBJECT);

    let mut first = connect(&server, Some(&token)).await.unwrap();
    assert_eq!(next_json(&mut first).await["message_type"], "SESSION_READY");

    // 같은 actor 로 두 번째 연결 — 넘겨받기를 유발한다.
    let mut second = connect(&server, Some(&token)).await.unwrap();
    assert_eq!(
        next_json(&mut second).await["message_type"],
        "SESSION_READY"
    );

    let code = read_until_close(&mut first).await;
    assert_eq!(code, Some(4001), "ADR-0005 §2: SUPERSEDED = 4001");

    server.shutdown_and_join().await;

    let spawned = server.ship_spawned_ids();
    // 음성 대조(§7a): 스폰이 최소 1건은 관측돼야 한다 — 0건이면 이 단언은 아무것도
    // 재지 않는 `0 == 0` 통과다.
    assert!(
        !spawned.is_empty(),
        "SHIP_SPAWNED 가 한 건도 없다 — 이 테스트는 아무것도 재지 못한다"
    );
    assert_eq!(
        spawned.len(),
        1,
        "같은 actor 의 두 번째 접속이 함선을 하나 더 스폰했다(복제) — 관측된 SHIP_SPAWNED: {spawned:?}"
    );
    println!(
        "[R4 S-6 §7a] SHIP_SPAWNED 정확히 1건, ship_id={spawned:?} — 넘겨받기 확인(복제 아님)"
    );
}

/// S-3 (PR #1 리뷰 결함 3) — 동시 입장이 `world_full` 정원을 넘지 못한다.
///
/// `ships_total()`(`stats.rs`)은 tick 드라이버가 **tick 당 한 번**만 갱신한다(I-25) —
/// `/ws` 핸들러는 그 값을 읽기만 했고 입장 시 자리를 예약하지 않았다. 정원이 빈 상태에서
/// actor 여럿이 **같은 tick 경계 전에** 동시에 들어오면 전부 게이트를 통과하고, 다음
/// tick 에 정원을 넘는 함선이 한꺼번에 스폰된다.
///
/// # 경합을 결정적으로 만든다 (§7a)
///
/// tick 간격을 3초로 늘린다. `/ws` 핸들러는 게이트 판정을 **tick 을 기다리지 않고** 그
/// 자리에서 응답한다(HTTP 업그레이드 자체가 판정 직후 끝난다) — 그래서 5개 연결을
/// `join_all` 로 동시에 쏘면, 판정이 전부 같은 tick 간격 안(= `ships_active`/
/// `ships_lingering` 갱신 전)에서 끝난다는 것을 **타이밍 운에 기대지 않고** 시계로
/// 보장할 수 있다. 아래 `elapsed < tick_interval / 2` 단언이 바로 그 보장이 실제로
/// 성립했다는 증거다 — 이게 없으면 "정원 초과 0건"은 경합이 애초에 안 생긴 입력에서
/// 나온 `0 == 0` 일 수 있다.
///
/// **디버그 빌드에서 이 결함을 재현하면 `simulation.rs` 의 `debug_assert!` 가 먼저
/// panic 한다** — `r4_s6_second_connection_takes_over_the_ship_not_a_second_one` 와
/// 같은 이유다. 그 panic 자체가 "게이트가 뚫렸다"는 독립된 증거이지, 이 테스트가
/// 무력하다는 뜻이 아니다. 릴리스 빌드(`cargo test -p starfall-gateway --release`)에는
/// 그 그물이 없으므로 아래 단언들이 직접 하중을 받는다.
#[tokio::test]
async fn s3_concurrent_entrants_cannot_exceed_world_capacity() {
    let mut world = default_world();
    world.max_entities_per_snapshot = 2;
    let capacity = u64::try_from(world.max_entities_per_snapshot).unwrap();
    let tick_interval = Duration::from_secs(3);
    let mut server = TestServer::start_with_world_and_interval(true, world, tick_interval).await;

    const ACTORS: [&str; 5] = [
        "01a0b1c2-2c01-7a45-8b67-0000000000c1",
        "01a0b1c2-2c01-7a45-8b67-0000000000c2",
        "01a0b1c2-2c01-7a45-8b67-0000000000c3",
        "01a0b1c2-2c01-7a45-8b67-0000000000c4",
        "01a0b1c2-2c01-7a45-8b67-0000000000c5",
    ];
    let tokens: Vec<String> = ACTORS.iter().map(|actor| server.token(actor)).collect();

    let start = Instant::now();
    let attempts: Vec<Result<Client, u16>> =
        futures_util::future::join_all(tokens.iter().map(|token| connect(&server, Some(token))))
            .await;
    let elapsed = start.elapsed();
    let codes: Vec<Result<(), u16>> = attempts
        .iter()
        .map(|result| match result {
            Ok(_) => Ok(()),
            Err(code) => Err(*code),
        })
        .collect();
    drop(attempts); // 소켓은 더 필요 없다 — 이미 기록된 SHIP_SPAWNED 는 지워지지 않는다.

    // §7a — 경합이 실제로 일어났다는 증거: 5건의 게이트 판정이 tick 간격(3초)의 한참
    // 안쪽에서 끝났다 — 그 사이 어떤 tick 도 `ships_active`/`ships_lingering` 을 갱신할
    // 수 없었으므로, 5개 요청은 정말로 같은(갱신 전) `ships_total()` 을 놓고 겨뤘다.
    assert!(
        elapsed < tick_interval / 2,
        "경합 창이 만들어지지 않았다 — tick 이 그 사이에 끼어들었을 수 있다: {elapsed:?}"
    );

    let succeeded = codes.iter().filter(|result| result.is_ok()).count();
    let rejected_503 = codes
        .iter()
        .filter(|result| matches!(result, Err(503)))
        .count();
    assert_eq!(
        succeeded + rejected_503,
        codes.len(),
        "503 도 성공도 아닌 응답이 있었다: {codes:?}"
    );
    assert_eq!(
        u64::try_from(succeeded).unwrap(),
        capacity,
        "정원({capacity})을 넘겨 통과했다 — world_full 게이트가 동시 입장을 막지 못했다: {codes:?}"
    );

    // tick 이 실제로 들어온 Open 을 처리할 시간을 준다. 디버그 빌드라면 정원을 넘는
    // 함선이 스폰되는 순간 tick 스레드가 `debug_assert!` 로 먼저 죽는다(위 문서 참고).
    tokio::time::sleep(tick_interval + Duration::from_millis(200)).await;
    let spawned = server.ship_spawned_ids();
    assert_eq!(
        u64::try_from(spawned.len()).unwrap(),
        capacity,
        "정원을 넘는 함선이 실제로 스폰됐다 — SHIP_SPAWNED: {spawned:?}"
    );

    server.shutdown_and_join().await;
}

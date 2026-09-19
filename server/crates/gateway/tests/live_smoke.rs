//! **실서버 + 실 PostgreSQL** 대상 스모크. 기본 실행에서는 건너뛴다(`#[ignore]`).
//!
//! `cargo test --workspace` 는 인프라를 요구하면 안 되므로 여기 있는 테스트는 전부
//! `#[ignore]` 다. 실행하려면 서버와 인프라를 띄운 뒤:
//!
//! ```text
//! cd server && cargo test -p starfall-gateway --test live_smoke -- --ignored --nocapture
//! ```
//!
//! 환경 변수(전부 기본값 있음): `STARFALL_WS_URL`, `DATABASE_URL`, `STARFALL_DEV_AUTH_SECRET`.
//!
//! QA 도 그대로 쓸 수 있다 — 이 스모크는 `tools/bots` 와 **독립된 출처**라서
//! 3자 대조(봇 / 메트릭 / DB)의 DB 쪽을 서버가 스스로 확인하는 데 쓴다.

// 통합 테스트 파일이라 clippy.toml 의 allow-*-in-tests 가 헬퍼 함수까지 덮지 못한다.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::time::{Duration, Instant};

use futures_util::{SinkExt, StreamExt};
use sqlx::Row;
use sqlx::postgres::PgPoolOptions;
use starfall_contracts::primitives::{GameCalendar, GameTime, Tick, UuidV7};
use starfall_gateway::DevAuth;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::{Message, Utf8Bytes};

const DEFAULT_WS: &str = "ws://127.0.0.1:8080/ws";
const DEFAULT_DB: &str = "postgres://starfall:starfall_dev_only@127.0.0.1:15432/starfall";
const DEFAULT_SECRET: &str = "dev_only_not_a_secret";
/// 봇(`bot-000`~`bot-029`)과도 Unity(`...7e57...0001`)와도 겹치지 않는 주체.
const SMOKE_SUBJECT: &str = "01a0b1c2-5a11-7c01-8d01-000000000001";

fn env_or(name: &str, default: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| default.to_owned())
}

/// 실서버에 붙어 3 왕복을 하고, 그 세션의 도메인 이벤트가 DB 에 들어오는지 본다.
///
/// 덮는 것: AC-5(a) 인증, AC-6(c) 순서·tick, I-16 세션 쌍, AC-16(d) `occurred_at` 재계산,
/// AC-17(b) 종료 후 가시성(폴링으로 측정).
#[tokio::test]
#[ignore = "실서버와 PostgreSQL 이 필요하다"]
async fn live_round_trip_writes_session_rows() {
    let ws_url = env_or("STARFALL_WS_URL", DEFAULT_WS);
    let auth = DevAuth::from_secret(Some(env_or("STARFALL_DEV_AUTH_SECRET", DEFAULT_SECRET)));
    let subject = UuidV7::parse(SMOKE_SUBJECT).unwrap();
    let token = auth.mint(subject).unwrap();

    let mut request = ws_url.into_client_request().unwrap();
    request
        .headers_mut()
        .insert("Authorization", format!("Bearer {token}").parse().unwrap());
    let (mut client, response) = tokio_tungstenite::connect_async(request)
        .await
        .expect("서버가 떠 있어야 한다");
    assert_eq!(response.status().as_u16(), 101, "업그레이드");

    // 1) 첫 계약 메시지는 SESSION_READY 다.
    let ready = next_json(&mut client).await;
    assert_eq!(ready["message_type"], "SESSION_READY");
    assert_eq!(ready["payload"]["actor_id"], SMOKE_SUBJECT, "I-10");
    let correlation_id = ready["correlation_id"].as_str().unwrap().to_owned();
    let session_id = ready["payload"]["session_id"].as_str().unwrap().to_owned();
    let tick_hz = ready["payload"]["tick_hz"].as_u64().unwrap();
    let world_id = ready["payload"]["world_id"].as_str().unwrap().to_owned();
    println!(
        "[live] SESSION_READY session_id={session_id} correlation_id={correlation_id} \
         tick_hz={tick_hz} world_id={world_id} server_version={}",
        ready["payload"]["server_version"]
    );

    // 2) 3 왕복. COMMAND_RESULT 가 PING_REPLY 보다 먼저이고 tick 이 같다 (I-15).
    for n in 0..3u64 {
        let id = format!("01a0b1c2-0a00-7{:03x}-8a01-{:012x}", n, n);
        client
            .send(Message::Text(Utf8Bytes::from(format!(
                r#"{{"command_id":"{id}","command_type":"PING_SERVER","schema_version":1,"client_sent_at":null,"payload":{{"probe_seq":{n}}}}}"#
            ))))
            .await
            .unwrap();
        let result = next_json(&mut client).await;
        let reply = next_json(&mut client).await;
        assert_eq!(result["message_type"], "COMMAND_RESULT");
        assert_eq!(reply["message_type"], "PING_REPLY");
        assert_eq!(result["payload"]["status"], "ACCEPTED");
        assert_eq!(result["tick"], reply["tick"]);
        assert_eq!(reply["payload"]["probe_seq"], n);
    }
    println!("[live] 왕복 3건 확인 (COMMAND_RESULT → PING_REPLY, 같은 tick)");

    // 3) 정상 종료.
    client.close(None).await.unwrap();
    let closed_at = Instant::now();
    while let Some(Ok(_)) = client.next().await {}

    // 4) DB 폴링 — 마지막 프레임부터 몇 초 만에 보이는가 (AC-17b).
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&env_or("DATABASE_URL", DEFAULT_DB))
        .await
        .expect("PostgreSQL 접속");
    let correlation = UuidV7::parse(&correlation_id).unwrap().get();

    let mut visible_after = None;
    for _ in 0..100 {
        let row = sqlx::query(
            "SELECT
                 count(*) FILTER (WHERE event_type = 'SESSION_OPENED') AS opened,
                 count(*) FILTER (WHERE event_type = 'SESSION_CLOSED') AS closed
             FROM domain_events WHERE correlation_id = $1",
        )
        .bind(correlation)
        .fetch_one(&pool)
        .await
        .unwrap();
        let (opened, closed): (i64, i64) = (row.get("opened"), row.get("closed"));
        if opened == 1 && closed == 1 {
            visible_after = Some(closed_at.elapsed());
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let elapsed = visible_after.expect("10초 안에 세션 쌍이 보여야 한다");
    println!(
        "[live] 세션 쌍 가시까지 {:.3}초 (AC-17b 게이트 5초)",
        elapsed.as_secs_f64()
    );
    assert!(elapsed < Duration::from_secs(5), "AC-17(b) 게이트");

    // 5) 행 내용 검증 — 짝, close_reason, occurred_at 재계산.
    let rows = sqlx::query(
        "SELECT event_type, tick, sequence, occurred_at, actor_id, causation_id,
                payload->>'session_id' AS session_id, payload->>'close_reason' AS close_reason
         FROM domain_events WHERE correlation_id = $1 ORDER BY tick, sequence",
    )
    .bind(correlation)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(rows.len(), 2, "SESSION_OPENED 1 + SESSION_CLOSED 1 (I-16)");

    let calendar = GameCalendar::new(
        u32::try_from(tick_hz).unwrap(),
        GameTime::parse("3800-01-01T00:00:00Z").unwrap(),
        60,
    )
    .unwrap();
    for row in &rows {
        let event_type: String = row.get("event_type");
        let tick: i64 = row.get("tick");
        let occurred_at: String = row.get("occurred_at");
        let row_session: String = row.get("session_id");
        let actor: sqlx::types::Uuid = row.get("actor_id");
        let causation: Option<sqlx::types::Uuid> = row.get("causation_id");
        assert_eq!(row_session, session_id);
        assert_eq!(actor.to_string(), SMOKE_SUBJECT, "actor_id 는 토큰 주체다");
        assert!(
            causation.is_none(),
            "이 슬라이스의 causation_id 는 null 이다"
        );

        let recomputed = calendar
            .occurred_at(Tick::new(u64::try_from(tick).unwrap()).unwrap())
            .unwrap();
        assert_eq!(
            occurred_at,
            recomputed.as_str(),
            "occurred_at 이 tick 에서 재계산한 값과 문자열로 같아야 한다 (I-19)"
        );
        println!("[live]   {event_type} tick={tick} occurred_at={occurred_at}");
    }
    let closed_reason: String = rows[1].get("close_reason");
    assert_eq!(closed_reason, "CLIENT_CLOSED");
    println!("[live] close_reason=CLIENT_CLOSED, occurred_at 재계산 2건 일치");
}

async fn next_json<S>(client: &mut S) -> serde_json::Value
where
    S: StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin,
{
    loop {
        let message = tokio::time::timeout(Duration::from_secs(10), client.next())
            .await
            .expect("메시지 대기 타임아웃")
            .expect("스트림이 끝났다")
            .expect("소켓 오류");
        if let Message::Text(text) = message {
            return serde_json::from_str(text.as_str()).unwrap();
        }
    }
}

/// 서버가 **정상 종료할 때** 살아 있던 세션이 `SERVER_SHUTDOWN` 으로 닫히는지 실서버에서 본다
/// (AC-8d / SC-27). 실행 중에 누군가 서버를 종료시켜야 한다 — 스크립트가 stdin 으로
/// `shutdown` 을 보낸다.
#[tokio::test]
#[ignore = "실서버가 필요하고, 실행 중 외부에서 종료 신호를 보내야 한다"]
async fn live_open_session_is_closed_by_server_shutdown() {
    let ws_url = env_or("STARFALL_WS_URL", DEFAULT_WS);
    let auth = DevAuth::from_secret(Some(env_or("STARFALL_DEV_AUTH_SECRET", DEFAULT_SECRET)));
    let subject = UuidV7::parse("01a0b1c2-5a11-7c01-8d01-000000000002").unwrap();
    let token = auth.mint(subject).unwrap();

    let mut request = ws_url.into_client_request().unwrap();
    request
        .headers_mut()
        .insert("Authorization", format!("Bearer {token}").parse().unwrap());
    let (mut client, _) = tokio_tungstenite::connect_async(request).await.unwrap();

    let ready = next_json(&mut client).await;
    let correlation_id = ready["correlation_id"].as_str().unwrap().to_owned();
    println!("[live] 세션 유지 중 correlation_id={correlation_id} — 서버 종료를 기다린다");

    // 서버가 닫을 때까지 읽는다. close code 는 ADR-0005 §2 의 SERVER_SHUTDOWN = 1001.
    let mut close_code = None;
    while let Ok(Some(frame)) = tokio::time::timeout(Duration::from_secs(60), client.next()).await {
        match frame {
            Ok(Message::Close(frame)) => {
                close_code = frame.map(|frame| u16::from(frame.code));
                break;
            }
            Ok(_) => {}
            Err(_) => break,
        }
    }
    println!("[live] 서버가 닫았다. close code = {close_code:?}");
    assert_eq!(close_code, Some(1001), "SERVER_SHUTDOWN = 1001");

    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&env_or("DATABASE_URL", DEFAULT_DB))
        .await
        .unwrap();
    let correlation = UuidV7::parse(&correlation_id).unwrap().get();
    let reason: String = sqlx::query_scalar(
        "SELECT payload->>'close_reason' FROM domain_events
         WHERE correlation_id = $1 AND event_type = 'SESSION_CLOSED'",
    )
    .bind(correlation)
    .fetch_one(&pool)
    .await
    .expect("SESSION_CLOSED 행이 있어야 한다");
    assert_eq!(reason, "SERVER_SHUTDOWN", "I-16: 종료 스윕이 닫는다");
    println!("[live] close_reason=SERVER_SHUTDOWN 확인 (커밋까지 기다린 뒤 종료했다)");
}

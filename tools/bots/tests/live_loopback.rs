//! **실제 소켓 위에서 계측기를 검증한다.**
//!
//! `ledger_accounting.rs` 는 합성 호출로 계측 로직만 본다. 여기서는 진짜 WebSocket 핸드셰이크와
//! 프레임을 거쳐 같은 결론이 나오는지 본다 — 즉 봇의 **수신 경로**(파싱·디스패치·순서)까지
//! 포함한 검증이다. 서버 구현이 아직 없으므로 계약 모양대로 답하는 가짜 서버를 세운다.
//!
//! 핵심은 정상 경로가 아니라 **고장 경로**다:
//! - 가짜 서버가 `COMMAND_RESULT` 1건을 빠뜨리면 봇이 손실 1로 잡는가
//! - 가짜 서버가 `PING_REPLY` 를 `COMMAND_RESULT` 보다 먼저 보내면 I-15 위반으로 잡는가
//! - 가짜 서버가 `Authorization` 헤더를 실제로 받는가 (ADR-0008 §2 의 전달 경로)
//!
//! 이 가짜 서버는 **판정의 근거가 아니다**. 실제 판정은 언제나 진짜 서버를 상대로 한다.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::handshake::server::{ErrorResponse, Request, Response};

use starfall_bots::conn::{Behavior, BotSpec, Clock, run_connection};
use starfall_bots::token;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Fault {
    None,
    /// n 번째(0-based) 명령의 COMMAND_RESULT 를 보내지 않는다.
    DropResult(u32),
    /// PING_REPLY 를 COMMAND_RESULT 보다 먼저 보낸다 (I-15 위반).
    ReplyFirst,
    /// 같은 명령에 PING_REPLY 를 2건 보낸다 (중복 제거 실패).
    DoubleReply,
}

struct Fake {
    addr: std::net::SocketAddr,
    seen_auth: Arc<Mutex<Option<String>>>,
    handle: tokio::task::JoinHandle<()>,
}

async fn spawn_fake(fault: Fault) -> Fake {
    spawn_fake_n(fault, 1).await
}

/// `max_conns` 개의 연결을 받는 가짜 서버. 30봇 동시 접속을 하네스 쪽에서 미리 재 보려고
/// 여러 연결을 받게 해 두었다(U-6 자원 여유의 봇 쪽 절반).
async fn spawn_fake_n(fault: Fault, max_conns: usize) -> Fake {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    let seen_auth = Arc::new(Mutex::new(None));
    let auth_slot = Arc::clone(&seen_auth);

    let handle = tokio::spawn(async move {
        let mut accepted = 0usize;
        while accepted < max_conns {
            let Ok((stream, _)) = listener.accept().await else {
                return;
            };
            accepted += 1;
            let slot = Arc::clone(&auth_slot);
            tokio::spawn(serve_one(stream, fault, slot));
        }
        // 마지막 연결이 끝날 때까지 리스너 태스크는 살아 있어야 한다.
        std::future::pending::<()>().await;
    });

    Fake {
        addr,
        seen_auth,
        handle,
    }
}

async fn serve_one(stream: tokio::net::TcpStream, fault: Fault, slot: Arc<Mutex<Option<String>>>) {
    {
        // tungstenite 의 ErrorResponse 가 커서 clippy 가 경고한다. 가짜 서버는 거절하지 않으므로
        // Err 를 절대 만들지 않는다.
        #[allow(clippy::result_large_err)]
        let callback = move |req: &Request, resp: Response| -> Result<Response, ErrorResponse> {
            let value = req
                .headers()
                .get("Authorization")
                .and_then(|v| v.to_str().ok())
                .map(str::to_owned);
            if let Ok(mut g) = slot.lock() {
                *g = value;
            }
            Ok(resp)
        };
        let Ok(mut ws) = tokio_tungstenite::accept_hdr_async(stream, callback).await else {
            return;
        };

        let correlation = uuid::Uuid::now_v7();
        let ready = serde_json::json!({
            "message_id": uuid::Uuid::now_v7(),
            "message_type": "SESSION_READY",
            "schema_version": 1,
            "tick": 100,
            "correlation_id": correlation,
            "payload": {
                "session_id": uuid::Uuid::now_v7(),
                "world_id": "01a0b1c2-3d4e-7f01-8a2b-9c0d1e2f3a4b",
                "actor_id": token::subject_for("bot-000"),
                "tick_hz": 20,
                "server_version": "fake-0"
            }
        });
        if ws.send(Message::text(ready.to_string())).await.is_err() {
            return;
        }

        let mut seen = 0u32;
        let mut tick = 101u64;
        while let Some(Ok(msg)) = ws.next().await {
            match msg {
                Message::Text(text) => {
                    let Ok(v) = serde_json::from_str::<serde_json::Value>(text.as_str()) else {
                        continue;
                    };
                    let Some(cid) = v.get("command_id").cloned() else {
                        continue;
                    };
                    let seq = v
                        .get("payload")
                        .and_then(|p| p.get("probe_seq"))
                        .cloned()
                        .unwrap_or(serde_json::json!(0));
                    let result = serde_json::json!({
                        "message_id": uuid::Uuid::now_v7(),
                        "message_type": "COMMAND_RESULT",
                        "schema_version": 1,
                        "tick": tick,
                        "correlation_id": null,
                        "payload": { "command_id": cid, "status": "ACCEPTED", "reason_code": null }
                    });
                    let reply = serde_json::json!({
                        "message_id": uuid::Uuid::now_v7(),
                        "message_type": "PING_REPLY",
                        "schema_version": 1,
                        "tick": tick,
                        "correlation_id": null,
                        "payload": { "command_id": cid, "probe_seq": seq }
                    });
                    tick += 1;

                    let drop_result = matches!(fault, Fault::DropResult(n) if n == seen);
                    if fault == Fault::ReplyFirst {
                        let _ = ws.send(Message::text(reply.to_string())).await;
                        let _ = ws.send(Message::text(result.to_string())).await;
                    } else {
                        if !drop_result {
                            let _ = ws.send(Message::text(result.to_string())).await;
                        }
                        let _ = ws.send(Message::text(reply.to_string())).await;
                        if fault == Fault::DoubleReply {
                            let _ = ws.send(Message::text(reply.to_string())).await;
                        }
                    }
                    seen += 1;
                }
                Message::Close(_) => {
                    let _ = ws.send(Message::Close(None)).await;
                    return;
                }
                _ => {}
            }
        }
    }
}

async fn run_bot_against(fake: &Fake, pings: u32) -> starfall_bots::conn::ConnectionOutcome {
    let (_subject, tok) = token::identity("dev_only_not_a_secret", "bot-000");
    run_connection(BotSpec {
        label: "bot-000".to_owned(),
        url: format!("ws://{}/ws", fake.addr),
        token: tok,
        behavior: Behavior::Burst {
            pings,
            grace: Duration::from_millis(600),
        },
        clock: Clock::start(),
        live_corr: None,
    })
    .await
}

#[tokio::test]
async fn healthy_fake_server_yields_clean_gates_and_a_correlation_id() {
    let fake = spawn_fake(Fault::None).await;
    let outcome = run_bot_against(&fake, 5).await;
    let s = outcome.ledger.finish();

    // ADR-0008 §2: 토큰은 업그레이드 요청의 Authorization 헤더로 간다.
    let auth = fake.seen_auth.lock().expect("lock").clone();
    let (subject, tok) = token::identity("dev_only_not_a_secret", "bot-000");
    assert_eq!(
        auth,
        Some(format!("Bearer {tok}")),
        "Authorization 헤더가 전달되지 않았다"
    );

    let session = outcome
        .ledger
        .session
        .as_ref()
        .expect("SESSION_READY 를 받아야 한다");
    assert!(
        session.correlation_id.is_some(),
        "correlation 집합을 만들 수 없다"
    );
    assert_eq!(session.actor_id.to_string(), subject);
    assert_eq!(session.tick_hz, 20);
    assert_eq!(
        session.close_initiator, "client",
        "봇이 먼저 정상 Close 를 보낸다"
    );

    assert_eq!(s.sent_total, 5);
    assert_eq!(s.results_total, 5);
    assert_eq!(s.replies_total, 5);
    assert_eq!(s.missing_results, 0);
    assert_eq!(s.order_violations, 0);
    assert_eq!(s.probe_seq_mismatches, 0);
    assert!(s.one_to_one_holds() && s.accepted_reply_pairing_holds());
    assert_eq!(s.rtt.count, 5, "왕복 표본이 수집되어야 한다");
    assert!(s.wire_errors.is_empty(), "wire_errors={:?}", s.wire_errors);
    fake.handle.abort();
}

/// **의도적으로 응답 1건을 누락시키면 손실 1로 잡히는가** (리더 요구 항목, 실소켓 경로).
#[tokio::test]
async fn dropped_command_result_on_the_wire_is_detected_as_loss() {
    let fake = spawn_fake(Fault::DropResult(2)).await;
    let outcome = run_bot_against(&fake, 5).await;
    let s = outcome.ledger.finish();

    assert_eq!(s.sent_total, 5);
    assert_eq!(s.results_total, 4, "가짜 서버가 1건을 빠뜨렸다");
    assert_eq!(s.missing_results, 1, "손실 1건이 잡히지 않았다");
    assert!(
        !s.one_to_one_holds(),
        "손실이 있는데 게이트가 통과했다 — 부하 판정 전체가 거짓이 된다"
    );
    // PING_REPLY 는 5건 다 왔다. COMMAND_RESULT 없이 온 것은 순서 위반으로도 잡힌다(I-15).
    assert_eq!(s.replies_total, 5);
    assert_eq!(s.order_violations, 1);
    fake.handle.abort();
}

/// I-15 위반(PING_REPLY 가 먼저)을 실소켓 경로에서 잡는가.
#[tokio::test]
async fn reply_before_result_on_the_wire_is_detected() {
    let fake = spawn_fake(Fault::ReplyFirst).await;
    let outcome = run_bot_against(&fake, 4).await;
    let s = outcome.ledger.finish();

    assert_eq!(s.sent_total, 4);
    assert_eq!(s.results_total, 4);
    assert_eq!(s.replies_total, 4);
    assert_eq!(s.missing_results, 0);
    assert!(
        s.one_to_one_holds(),
        "1:1 은 성립한다 — 깨진 것은 순서뿐이다"
    );
    assert_eq!(s.order_violations, 4, "I-15 위반이 잡히지 않았다");
    fake.handle.abort();
}

/// 서버의 중복 제거가 깨져 PING_REPLY 가 2건 오는 경우(AC-7b 의 실패 모양).
#[tokio::test]
async fn duplicate_reply_on_the_wire_is_detected() {
    let fake = spawn_fake(Fault::DoubleReply).await;
    let outcome = run_bot_against(&fake, 3).await;
    let s = outcome.ledger.finish();

    assert_eq!(s.sent_total, 3);
    assert_eq!(s.replies_total, 6);
    assert_eq!(s.duplicate_replies, 3);
    assert!(!s.accepted_reply_pairing_holds());
    fake.handle.abort();
}

/// 서버에 닿지 못하면 **조용히 성공하지 않는다**. 클라이언트는 401·503·네트워크 오류를
/// 구분하지 않는다(ADR-0005 §6) — 원인 판정의 증거는 서버 쪽이다.
#[tokio::test]
async fn unreachable_server_is_reported_not_swallowed() {
    let (_subject, tok) = token::identity("dev_only_not_a_secret", "bot-000");
    let outcome = run_connection(BotSpec {
        label: "bot-000".to_owned(),
        // 아무도 듣고 있지 않은 포트
        url: "ws://127.0.0.1:1/ws".to_owned(),
        token: tok,
        behavior: Behavior::Burst {
            pings: 1,
            grace: Duration::from_millis(100),
        },
        clock: Clock::start(),
        live_corr: None,
    })
    .await;
    assert!(outcome.connect_error.is_some());
    assert!(outcome.ledger.session.is_none());
    let s = outcome.ledger.finish();
    assert_eq!(s.sent_total, 0);
    assert!(!s.errors.is_empty());
}

/// SC-61(AC-17a) 의 전제: correlation 이 **실행 중에** 파일에 나타나는가.
/// 실행이 끝난 뒤에만 쓰면 "A 단계가 도는 동안 조회"가 구조적으로 불가능해진다.
#[tokio::test]
async fn correlation_is_written_live_while_the_connection_is_still_open() {
    use starfall_bots::conn::LiveCorrelationSink;
    use std::sync::Arc;

    let fake = spawn_fake(Fault::None).await;
    let path =
        std::env::temp_dir().join(format!("starfall-live-corr-{}.txt", uuid::Uuid::now_v7()));
    let sink = Arc::new(LiveCorrelationSink::create(&path).expect("create sink"));
    let (_subject, tok) = token::identity("dev_only_not_a_secret", "bot-000");

    let url = format!("ws://{}/ws", fake.addr);
    let bot = tokio::spawn(run_connection(BotSpec {
        label: "bot-000".to_owned(),
        url,
        token: tok,
        behavior: Behavior::Steady {
            interval: Duration::from_millis(200),
            duration: Duration::from_secs(3),
            phase: Duration::ZERO,
        },
        clock: Clock::start(),
        live_corr: Some(Arc::clone(&sink)),
    }));

    // 봇이 아직 돌고 있는 동안 파일이 채워져야 한다.
    let mut lines: Vec<String> = Vec::new();
    for _ in 0..40 {
        tokio::time::sleep(Duration::from_millis(50)).await;
        if let Ok(text) = std::fs::read_to_string(&path) {
            lines = text
                .lines()
                .map(str::to_owned)
                .filter(|l| !l.is_empty())
                .collect();
            if !lines.is_empty() {
                break;
            }
        }
    }
    assert!(!bot.is_finished(), "봇이 끝나기 전에 확인해야 의미가 있다");
    assert_eq!(
        lines.len(),
        1,
        "실행 중에 correlation 이 파일에 없다 → SC-61 을 잴 수 없다"
    );

    let outcome = bot.await.expect("join");
    let session = outcome.ledger.session.as_ref().expect("session");
    assert_eq!(
        lines[0],
        session.correlation_id.expect("correlation").to_string()
    );
    let _ = std::fs::remove_file(&path);
    fake.handle.abort();
}

/// **U-6 의 봇 쪽 절반**: 하네스가 30개 동시 연결을 감당하는가.
///
/// 진짜 서버가 아직 없으므로 "서버가 버티는가"는 여기서 답하지 못한다. 답하는 것은
/// **"31 연결에서 문제가 생겼을 때 그것이 하네스 탓인지"**다. 하네스가 30개를 못 돌리면
/// 부하 결과 전체가 하네스의 한계를 측정한 것이 된다.
#[tokio::test]
async fn thirty_concurrent_bots_are_handled_by_the_harness() {
    let fake = spawn_fake_n(Fault::None, 30).await;
    let url = format!("ws://{}/ws", fake.addr);
    let clock = Clock::start();

    let mut handles = Vec::new();
    for i in 0..30usize {
        let label = token::bot_label(i);
        let (_subject, tok) = token::identity("dev_only_not_a_secret", &label);
        let url = url.clone();
        handles.push(tokio::spawn(run_connection(BotSpec {
            label,
            url,
            token: tok,
            behavior: Behavior::Steady {
                interval: Duration::from_millis(200),
                duration: Duration::from_secs(3),
                phase: Duration::ZERO,
            },
            clock,
            live_corr: None,
        })));
    }

    let mut ready = 0usize;
    let mut sent = 0u64;
    let mut results = 0u64;
    let mut missing = 0u64;
    let mut order_violations = 0u64;
    let mut server_closes = 0usize;
    for h in handles {
        let outcome = h.await.expect("join");
        if let Some(sess) = &outcome.ledger.session {
            ready += 1;
            if sess.close_initiator == "server" {
                server_closes += 1;
            }
        }
        let s = outcome.ledger.finish();
        sent += s.sent_total;
        results += s.results_total;
        missing += s.missing_results;
        order_violations += s.order_violations;
    }

    assert_eq!(ready, 30, "30개 연결이 모두 SESSION_READY 까지 가야 한다");
    assert_eq!(sent, results, "손실 0 (보낸 수 == 받은 수)");
    assert_eq!(missing, 0);
    assert_eq!(order_violations, 0);
    assert_eq!(server_closes, 0);
    assert!(sent >= 30 * 10, "봇당 최소 10건은 보냈어야 한다: {sent}");
    eprintln!("30 bots: sent={sent} results={results} ready={ready}");
    fake.handle.abort();
}

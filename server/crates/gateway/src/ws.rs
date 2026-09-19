//! `GET /ws` — 실시간 게이트웨이 (ADR-0005).
//!
//! # 수신 태스크는 상태를 만지지 않는다 (I-13)
//!
//! 프레임을 파싱해 [`crate::runtime::SubmitHandle`] 에 넣을 뿐이다. 시뮬레이션 상태 타입은
//! 이 크레이트에서 보이지 않는다.
//!
//! # 라이브러리 한도 > 앱 한도여야 계수가 가능하다 (AC-8a)
//!
//! tungstenite 의 `max_message_size` 를 16 KiB 로 두면 **라이브러리가 먼저 연결을 끊어**
//! 앱이 위반을 셀 기회가 없다 — "10초 창 8회"가 영원히 관측되지 않는다. 그래서 라이브러리
//! 한도는 64 KiB, 앱 한도는 16 KiB 다.
//!
//! # 큐에 넣지 못한 거부는 게이트웨이가 만든다 (ADR-0006 §5, I-13)
//!
//! `SERVER_BUSY`·`TOO_MANY_IN_FLIGHT`·`MALFORMED_COMMAND` 는 정의상 큐에 넣지 못했을 때
//! 나오므로 tick 이 만들 수 없다. 그래서 게이트웨이가 현재 tick 번호를 **읽어서** envelope 을
//! 채운다. 읽기는 상태 변경이 아니므로 I-13 위반이 아니다.

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use axum::body::Bytes;
use axum::extract::State;
use axum::extract::ws::{CloseFrame, Message, Utf8Bytes, WebSocket, WebSocketUpgrade};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use futures_util::SinkExt;
use futures_util::stream::{SplitSink, SplitStream, StreamExt};
use serde::Deserialize;
use starfall_contracts::events::SessionCloseReason;
use starfall_contracts::messages::{
    CommandResultMessage, CommandResultPayload, CommandResultType, RejectReasonCode,
};
use starfall_contracts::primitives::{ConstSchemaVersion, Tick, UuidV7};
use starfall_contracts::{PingServerCommand, registry};
use starfall_sim::{InboundCommand, ServerMessage};
use tokio::sync::{Notify, mpsc};

use crate::auth::extract_credential;
use crate::runtime::{
    CloseState, SEND_QUEUE_CAPACITY, SESSION_IN_FLIGHT_LIMIT, SessionRoute, SubmitHandle,
    close_code,
};
use crate::state::AppState;
use crate::stats::Stats;

/// 앱이 받아들이는 최대 메시지 크기. 초과하면 프로토콜 위반으로 **센다**.
pub const MAX_APP_MESSAGE_BYTES: usize = 16 * 1024;
/// 라이브러리(tungstenite) 한도. 앱 한도보다 **커야** 위반을 셀 수 있다.
pub const LIBRARY_MESSAGE_LIMIT: usize = 64 * 1024;
/// 10초 창에서 허용하는 프로토콜 위반 수 (ADR-0006 §6). 이 수를 **넘으면** 닫는다.
pub const VIOLATION_BUDGET: usize = 8;
/// 위반 예산의 슬라이딩 창 길이.
pub const VIOLATION_WINDOW: Duration = Duration::from_secs(10);
/// 서버가 Ping 을 보내는 주기.
pub const PING_INTERVAL: Duration = Duration::from_secs(15);
/// Pong·데이터 프레임이 이 시간 동안 없으면 닫는다.
pub const IDLE_TIMEOUT: Duration = Duration::from_secs(30);

/// 업그레이드 전에 거절한 이유. `/debug/stats` 라벨과 응답 본문의 `reason` 이 같은 값을 쓴다.
///
/// **클라이언트는 이 값을 볼 수 없다** — 브라우저·Mono 의 WebSocket 클라이언트는 업그레이드
/// 실패의 HTTP 상태 코드에 접근하지 못한다(client 실측: 에러 코드가 `Success` 로 온다).
/// 그래서 **거부의 증거는 언제나 서버 쪽**이고, 그 증거가 이 카운터다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpgradeRejection {
    /// `STARFALL_DEV_AUTH_SECRET` 미설정.
    AuthNotConfigured,
    /// 영속화 백로그 초과 (ADR-0007 §4).
    RecordingBacklog,
    /// 종료 중.
    ShuttingDown,
    /// `Authorization: Bearer` 헤더 없음.
    NoCredential,
    /// 서명 불일치 또는 주체가 정규 UUIDv7 이 아님.
    InvalidToken,
}

impl UpgradeRejection {
    /// 응답 본문·메트릭 라벨.
    #[must_use]
    pub const fn reason(self) -> &'static str {
        match self {
            Self::AuthNotConfigured => "auth_not_configured",
            Self::RecordingBacklog => "recording_backlog",
            Self::ShuttingDown => "shutting_down",
            Self::NoCredential => "no_credential",
            Self::InvalidToken => "invalid_token",
        }
    }

    const fn status(self) -> StatusCode {
        match self {
            Self::AuthNotConfigured | Self::RecordingBacklog | Self::ShuttingDown => {
                StatusCode::SERVICE_UNAVAILABLE
            }
            Self::NoCredential | Self::InvalidToken => StatusCode::UNAUTHORIZED,
        }
    }
}

fn reject(stats: &Stats, rejection: UpgradeRejection) -> Response {
    stats.record_upgrade_rejection(rejection);
    tracing::warn!(
        reason = rejection.reason(),
        status = rejection.status().as_u16(),
        "/ws 업그레이드 거절 — 세션도 기록도 만들지 않는다 (I-24)"
    );
    (
        rejection.status(),
        axum::Json(serde_json::json!({
            "status": "unavailable",
            "reason": rejection.reason(),
        })),
    )
        .into_response()
}

/// `GET /ws` 핸들러.
///
/// **인증은 업그레이드 전에** 한다 (ADR-0008 §2). 실패한 요청에는 WebSocket 연결이 존재하지
/// 않으므로 세션도 `SESSION_OPENED` 도 없다 (I-24).
pub async fn ws_handler(
    headers: HeaderMap,
    State(state): State<AppState>,
    upgrade: WebSocketUpgrade,
) -> Response {
    if !state.auth.is_enabled() {
        return reject(&state.stats, UpgradeRejection::AuthNotConfigured);
    }
    if !state.stats.is_accepting() {
        return reject(&state.stats, UpgradeRejection::ShuttingDown);
    }
    if state.stats.persist_backlog() > crate::runtime::PERSIST_BACKLOG_LIMIT {
        // 이벤트를 조용히 버리는 것보다 접속을 거절하는 편이 낫다 (ADR-0007 §4).
        return reject(&state.stats, UpgradeRejection::RecordingBacklog);
    }

    let Some(credential) = extract_credential(&headers) else {
        return reject(&state.stats, UpgradeRejection::NoCredential);
    };
    let Some(actor_id) = state.auth.verify(credential) else {
        return reject(&state.stats, UpgradeRejection::InvalidToken);
    };

    let Some(submit) = state.submit.clone() else {
        return reject(&state.stats, UpgradeRejection::ShuttingDown);
    };
    let stats = state.stats.clone();

    upgrade
        .max_message_size(LIBRARY_MESSAGE_LIMIT)
        .max_frame_size(LIBRARY_MESSAGE_LIMIT)
        .on_failed_upgrade(move |error| {
            tracing::warn!(%error, "WebSocket 업그레이드 실패");
        })
        .on_upgrade(move |socket| serve_session(socket, submit, stats, actor_id))
}

/// 한 연결의 생애.
///
/// # 읽기와 쓰기는 서로를 끝낸다
///
/// 송신 채널의 `Sender` 는 **tick 드라이버의 라우팅 표에도 복제되어 있다.** 그래서
/// 여기서 로컬 `Sender` 를 drop 해도 송신 태스크는 깨어나지 않는다(드라이버가 아직 들고
/// 있다). 반대로 송신 태스크가 유휴 종료를 결정해도 수신 태스크는 `stream.next()` 에서
/// 계속 기다린다.
///
/// 둘 중 하나라도 남아 있으면 `SESSION_CLOSED` 가 발행되지 않아 I-16 이 깨진다. 그래서
/// **먼저 끝난 쪽이 다른 쪽을 끝낸다**: 수신이 끝나면 `finish` 로 송신을 깨우고,
/// 송신이 먼저 끝나면(유휴·라우트 제거·소켓 오류) 수신을 abort 한다.
async fn serve_session(socket: WebSocket, submit: SubmitHandle, stats: Stats, actor_id: UuidV7) {
    // session_id 는 게이트웨이가 만든다 — 라우팅 표의 키가 되어야 하므로 tick 보다 먼저
    // 필요하다. correlation_id 는 tick 이 만든다(결정적 코어의 id 생성기).
    stats.connection_opened();
    let session_id = UuidV7::new_v7();
    let (out_tx, out_rx) = mpsc::channel::<ServerMessage>(SEND_QUEUE_CAPACITY);
    let in_flight = Arc::new(AtomicU32::new(0));
    let close_state = CloseState::new();
    let base = Instant::now();
    let last_inbound = Arc::new(AtomicU64::new(0));
    let finish = Arc::new(Notify::new());

    submit.open(
        session_id,
        actor_id,
        SessionRoute {
            outbound: out_tx.clone(),
            in_flight: Arc::clone(&in_flight),
            close_state: close_state.clone(),
            finish: Arc::clone(&finish),
        },
    );
    tracing::debug!(%session_id, %actor_id, "세션 수립 — 다음 tick 이 SESSION_READY 를 보낸다");

    let (sink, stream) = socket.split();
    let mut writer = tokio::spawn(writer_loop(
        sink,
        out_rx,
        close_state.clone(),
        Arc::clone(&last_inbound),
        stats.clone(),
        base,
        Arc::clone(&finish),
    ));
    let mut reader = tokio::spawn(reader_loop(ReaderContext {
        stream,
        submit: submit.clone(),
        stats: stats.clone(),
        session_id,
        out_tx,
        in_flight,
        close_state: close_state.clone(),
        last_inbound,
        base,
    }));

    let dropped = tokio::select! {
        _ = &mut reader => {
            tracing::debug!(%session_id, elapsed_ms = base.elapsed().as_millis(), "수신 태스크 종료");
            // 수신이 끝났다 → 송신을 깨워 Close 프레임을 내보내게 한다.
            finish.notify_one();
            writer.await.unwrap_or(0)
        }
        result = &mut writer => {
            tracing::debug!(%session_id, elapsed_ms = base.elapsed().as_millis(), "송신 태스크 종료");
            // 송신이 먼저 끝났다(유휴·라우트 제거·소켓 오류) → 수신을 끊는다.
            reader.abort();
            let _ = reader.await;
            result.unwrap_or(0)
        }
    };
    stats.record_messages_dropped(dropped);
    tracing::debug!(
        %session_id,
        reason = ?close_state.get(),
        elapsed_ms = base.elapsed().as_millis(),
        dropped,
        "세션 종료 제출"
    );

    // 세션 닫기 제출. 드라이버가 먼저 닫았으면(SLOW_CONSUMER·종료) sim 이 무시한다 —
    // 그래서 `SESSION_CLOSED` 는 세션당 정확히 1건이다 (I-16).
    submit.close(session_id, close_state.get());
    stats.connection_closed();
}

/// 송신 태스크. 반환값은 **버려진 메시지 수**(연결 종료로 보내지 못한 것).
async fn writer_loop(
    mut sink: SplitSink<WebSocket, Message>,
    mut out_rx: mpsc::Receiver<ServerMessage>,
    close_state: CloseState,
    last_inbound: Arc<AtomicU64>,
    stats: Stats,
    base: Instant,
    finish: Arc<Notify>,
) -> u64 {
    let mut ping = tokio::time::interval(PING_INTERVAL);
    ping.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    ping.tick().await; // 첫 tick 은 즉시 발화하므로 버린다.

    loop {
        tokio::select! {
            () = finish.notified() => break,
            message = out_rx.recv() => {
                let Some(message) = message else { break };
                let type_name = message.type_name();
                let Some(text) = encode(&message) else {
                    tracing::error!(type_name, "메시지 직렬화 실패 — 계약 타입이 깨졌다");
                    continue;
                };
                if sink.send(Message::Text(Utf8Bytes::from(text))).await.is_err() {
                    close_state.set_if_unset(SessionCloseReason::TransportError);
                    break;
                }
                stats.record_message_written(type_name);
            }
            _ = ping.tick() => {
                let idle = base.elapsed().saturating_sub(Duration::from_millis(
                    last_inbound.load(Ordering::Acquire),
                ));
                if idle >= IDLE_TIMEOUT {
                    // TCP 만으로는 죽은 연결이 몇 분간 살아 있는 것처럼 보이고,
                    // 그러면 "동시 접속 수"가 거짓말이 된다 (ADR-0005 §2).
                    close_state.set_if_unset(SessionCloseReason::IdleTimeout);
                    break;
                }
                if sink.send(Message::Ping(Bytes::new())).await.is_err() {
                    close_state.set_if_unset(SessionCloseReason::TransportError);
                    break;
                }
            }
        }
    }

    let reason = close_state.get();
    if let Some(code) = close_code(reason) {
        let _ = sink
            .send(Message::Close(Some(CloseFrame {
                code,
                reason: Utf8Bytes::from_static(""),
            })))
            .await;
    }
    let _ = sink.close().await;

    // 큐에 남은 것은 **버려진 것**이다. 정확히 세어야 회계가 닫힌다:
    //   enqueued == written + dropped + send_queue_depth
    let mut dropped = 0;
    out_rx.close();
    while out_rx.try_recv().is_ok() {
        dropped += 1;
    }
    dropped
}

fn encode(message: &ServerMessage) -> Option<String> {
    match message {
        ServerMessage::SessionReady(value) => serde_json::to_string(value.as_ref()).ok(),
        ServerMessage::CommandResult(value) => serde_json::to_string(value.as_ref()).ok(),
        ServerMessage::PingReply(value) => serde_json::to_string(value.as_ref()).ok(),
    }
}

/// 명령 envelope 에서 **디스패치에 필요한 최소한**만 읽는다 (ADR-0002 §3 의 2단계 디스패치).
///
/// `deny_unknown_fields` 를 달지 않는 것이 의도다 — peek 는 나머지를 무시해야 한다.
/// 구멍은 이 다음 단계(구체 타입 역직렬화)가 막는다.
#[derive(Debug, Deserialize)]
struct InboundPeek {
    command_id: Option<String>,
    command_type: Option<String>,
    schema_version: Option<u32>,
}

/// 위반 예산 — **슬라이딩 10초 창**.
///
/// 최근 [`VIOLATION_BUDGET`] 건의 시각을 들고 있다가, 새 위반이 났을 때 링이 가득 차 있고
/// 그중 가장 오래된 것이 창 안이면 예산 초과다. 창의 시작은 고정 epoch 이 아니라
/// **보존 중인 가장 오래된 위반의 시각**이다.
#[derive(Debug, Default)]
struct ViolationBudget {
    recent: std::collections::VecDeque<Instant>,
}

impl ViolationBudget {
    /// 위반 1건을 기록한다. 예산을 넘었으면 `true`.
    fn record(&mut self, now: Instant) -> bool {
        while let Some(front) = self.recent.front() {
            if now.duration_since(*front) > VIOLATION_WINDOW {
                self.recent.pop_front();
            } else {
                break;
            }
        }
        let over = self.recent.len() >= VIOLATION_BUDGET;
        self.recent.push_back(now);
        if self.recent.len() > VIOLATION_BUDGET + 1 {
            self.recent.pop_front();
        }
        over
    }
}

/// 수신 태스크의 입력. 인자가 많아 구조체로 묶는다(태스크로 spawn 하려면 `'static` 이어야 한다).
struct ReaderContext {
    stream: SplitStream<WebSocket>,
    submit: SubmitHandle,
    stats: Stats,
    session_id: UuidV7,
    out_tx: mpsc::Sender<ServerMessage>,
    in_flight: Arc<AtomicU32>,
    close_state: CloseState,
    last_inbound: Arc<AtomicU64>,
    base: Instant,
}

async fn reader_loop(context: ReaderContext) {
    let ReaderContext {
        mut stream,
        submit,
        stats,
        session_id,
        out_tx,
        in_flight,
        close_state,
        last_inbound,
        base,
    } = context;
    let mut budget = ViolationBudget::default();

    while let Some(frame) = stream.next().await {
        last_inbound.store(
            u64::try_from(base.elapsed().as_millis()).unwrap_or(u64::MAX),
            Ordering::Release,
        );

        let message = match frame {
            Ok(message) => message,
            Err(error) => {
                tracing::debug!(%session_id, %error, "소켓 오류");
                close_state.set_if_unset(SessionCloseReason::TransportError);
                return;
            }
        };

        let outcome = match message {
            Message::Text(text) => {
                if text.len() > MAX_APP_MESSAGE_BYTES {
                    tracing::debug!(%session_id, bytes = text.len(), "앱 프레임 상한 초과");
                    FrameOutcome::Violation
                } else {
                    handle_text(
                        text.as_str(),
                        &submit,
                        &stats,
                        session_id,
                        &out_tx,
                        &in_flight,
                    )
                }
            }
            Message::Binary(_) => {
                // 바이너리 포맷 전환은 측정 후 결정이다 (ADR-0002 §1, ADR-0005 §2).
                tracing::debug!(%session_id, "바이너리 프레임 — 프로토콜 위반");
                FrameOutcome::Violation
            }
            Message::Close(_) => {
                close_state.set_if_unset(SessionCloseReason::ClientClosed);
                return;
            }
            Message::Ping(_) | Message::Pong(_) => FrameOutcome::Handled,
        };

        if outcome == FrameOutcome::Overflow {
            tracing::warn!(%session_id, "거부 응답조차 송신 큐에 넣지 못했다 — SLOW_CONSUMER");
            close_state.set_if_unset(SessionCloseReason::SlowConsumer);
            return;
        }

        if outcome == FrameOutcome::Violation {
            stats.record_protocol_violation();
            if budget.record(Instant::now()) {
                tracing::warn!(
                    %session_id,
                    budget = VIOLATION_BUDGET,
                    window_s = VIOLATION_WINDOW.as_secs(),
                    "프로토콜 위반 예산 초과 — 연결을 닫는다"
                );
                close_state.set_if_unset(SessionCloseReason::ProtocolViolation);
                return;
            }
        }
    }

    // 스트림이 끝났다. 아무도 이유를 정하지 않았으면 피어의 정상 종료다.
    close_state.set_if_unset(SessionCloseReason::ClientClosed);
}

/// 텍스트 프레임 1건의 처리 결과.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FrameOutcome {
    /// 정상 처리(접수 또는 거부 응답 전송).
    Handled,
    /// 프로토콜 위반. 예산을 소모한다.
    Violation,
    /// 송신 큐가 가득 차 거부 응답조차 넣지 못했다 → 느린 소비자.
    Overflow,
}

/// 텍스트 프레임 1건을 처리한다.
fn handle_text(
    text: &str,
    submit: &SubmitHandle,
    stats: &Stats,
    session_id: UuidV7,
    out_tx: &mpsc::Sender<ServerMessage>,
    in_flight: &AtomicU32,
) -> FrameOutcome {
    let Ok(peek) = serde_json::from_str::<InboundPeek>(text) else {
        // 파싱 불가 — `command_id` 를 뽑을 수 없으므로 답할 수 없다.
        // 카운터가 유일한 신호다 (ADR-0006 §6).
        return FrameOutcome::Violation;
    };
    let Some(command_id) = peek.command_id.as_deref().and_then(UuidV7::parse) else {
        return FrameOutcome::Violation;
    };

    // 타입·버전 문제는 **거부**이지 위반이 아니다. 예산을 소모하지 않는다.
    if peek.command_type.as_deref() != Some(registry::PING_SERVER) {
        return send_rejection(
            out_tx,
            stats,
            command_id,
            RejectReasonCode::UnknownCommandType,
        );
    }
    if peek.schema_version != Some(1) {
        return send_rejection(
            out_tx,
            stats,
            command_id,
            RejectReasonCode::SchemaVersionUnsupported,
        );
    }

    // 2단계: `deny_unknown_fields` 가 살아 있는 구체 타입으로 다시 읽는다.
    let Ok(command) = serde_json::from_str::<PingServerCommand>(text) else {
        // 계약 타입으로 읽히지 않는 프레임은 위반이다 (ADR-0006 §6).
        // 거부 응답을 보내되 예산도 소모한다.
        return match send_rejection(
            out_tx,
            stats,
            command_id,
            RejectReasonCode::MalformedCommand,
        ) {
            FrameOutcome::Overflow => FrameOutcome::Overflow,
            _ => FrameOutcome::Violation,
        };
    };

    stats.record_command_received();

    // in-flight 상한. 넘으면 **거부하고 연결은 유지한다** (AC-7a).
    if in_flight.load(Ordering::Acquire) >= SESSION_IN_FLIGHT_LIMIT {
        return send_rejection(out_tx, stats, command_id, RejectReasonCode::TooManyInFlight);
    }
    in_flight.fetch_add(1, Ordering::AcqRel);

    if let Err(reason) = submit.try_command(session_id, InboundCommand::PingServer(command)) {
        let _ = in_flight.fetch_update(Ordering::AcqRel, Ordering::Acquire, |value| {
            Some(value.saturating_sub(1))
        });
        return send_rejection(out_tx, stats, command_id, reason);
    }
    FrameOutcome::Handled
}

/// 게이트웨이가 만드는 거부 응답.
///
/// tick 이 만들 수 없는 것만 여기서 만든다 — 정의상 큐에 넣지 못한 명령이기 때문이다.
///
/// **큐에 넣지 못하면 조용히 버리지 않는다** (I-22). 거부 응답조차 넣을 수 없다는 것은
/// 그 클라이언트가 응답을 읽지 않고 있다는 뜻이고, 그러면 느린 소비자로 연결을 닫는다.
/// 여기서 버리면 "보낸 명령 수 == 받은 COMMAND_RESULT 수"가 조용히 깨진다.
fn send_rejection(
    out_tx: &mpsc::Sender<ServerMessage>,
    stats: &Stats,
    command_id: UuidV7,
    reason: RejectReasonCode,
) -> FrameOutcome {
    stats.record_rejection(reason);
    let tick = Tick::new(stats.current_tick()).or_else(|| Tick::new(0));
    let Some(tick) = tick else {
        return FrameOutcome::Handled;
    };

    let message = ServerMessage::CommandResult(Box::new(CommandResultMessage {
        message_id: UuidV7::new_v7(),
        message_type: CommandResultType::CommandResult,
        schema_version: ConstSchemaVersion,
        tick,
        correlation_id: None,
        payload: CommandResultPayload::rejected(command_id, reason),
    }));
    let type_name = message.type_name();
    match out_tx.try_send(message) {
        Ok(()) => {
            stats.record_message_enqueued(type_name);
            FrameOutcome::Handled
        }
        Err(mpsc::error::TrySendError::Full(_)) => FrameOutcome::Overflow,
        // 송신 태스크가 이미 끝났다 — 연결이 정리되는 중이다.
        Err(mpsc::error::TrySendError::Closed(_)) => FrameOutcome::Handled,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn library_limit_must_exceed_app_limit() {
        // 같은 값이면 tungstenite 가 먼저 끊어 앱이 위반을 셀 수 없다 (AC-8a).
        const { assert!(LIBRARY_MESSAGE_LIMIT > MAX_APP_MESSAGE_BYTES) };
    }

    #[test]
    fn violation_budget_closes_on_the_ninth_within_the_window() {
        let mut budget = ViolationBudget::default();
        let base = Instant::now();
        for index in 0..VIOLATION_BUDGET {
            assert!(
                !budget.record(base + Duration::from_millis(index as u64 * 10)),
                "{index}번째 위반에서 닫히면 안 된다"
            );
        }
        assert!(
            budget.record(base + Duration::from_millis(100)),
            "10초 안의 9번째 위반에서 닫는다"
        );
    }

    #[test]
    fn violation_budget_forgets_outside_the_sliding_window() {
        let mut budget = ViolationBudget::default();
        let base = Instant::now();
        for index in 0..VIOLATION_BUDGET {
            assert!(!budget.record(base + Duration::from_millis(index as u64)));
        }
        // 창 밖에서 온 9번째는 예산을 넘기지 않는다.
        assert!(!budget.record(base + VIOLATION_WINDOW + Duration::from_secs(1)));
    }

    #[test]
    fn peek_reads_only_what_dispatch_needs() {
        let raw = r#"{"command_id":"01a0afaf-7e83-7f49-adfa-58ba04c2fd5a",
            "command_type":"PING_SERVER","schema_version":1,"client_sent_at":null,
            "payload":{"probe_seq":7},"extra":true}"#;
        let peek: InboundPeek = serde_json::from_str(raw).unwrap();
        assert_eq!(peek.command_type.as_deref(), Some("PING_SERVER"));
        assert_eq!(peek.schema_version, Some(1));
        // peek 는 통과시키지만 구체 타입은 막는다 (I-6).
        assert!(serde_json::from_str::<PingServerCommand>(raw).is_err());
    }

    #[test]
    fn rejection_reasons_have_stable_labels() {
        for (rejection, label) in [
            (UpgradeRejection::AuthNotConfigured, "auth_not_configured"),
            (UpgradeRejection::RecordingBacklog, "recording_backlog"),
            (UpgradeRejection::ShuttingDown, "shutting_down"),
            (UpgradeRejection::NoCredential, "no_credential"),
            (UpgradeRejection::InvalidToken, "invalid_token"),
        ] {
            assert_eq!(rejection.reason(), label);
        }
        assert_eq!(
            UpgradeRejection::NoCredential.status(),
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            UpgradeRejection::AuthNotConfigured.status(),
            StatusCode::SERVICE_UNAVAILABLE
        );
    }
}

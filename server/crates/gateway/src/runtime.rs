//! tick 드라이버와 제출 경로 (ADR-0006 §2·§4·§5).
//!
//! # 왜 tokio 태스크가 아니라 전용 OS 스레드인가 (ADR-0006 §2.1)
//!
//! 이 PC 실측(2026-09-18): `tokio::time::sleep(50ms)` 의 실제 간격은 **61 ms** 였다
//! (p50 61.2 ms / 60초 루프가 73.5초 / tick_lag +22 %). Windows 기본 시스템 타이머 해상도
//! 15.625 ms 때문에 50 ms 요청이 62.5 ms 로 올림된다. 같은 루프를 **전용 OS 스레드 +
//! `std::thread::sleep`** 으로 돌리면 p50 50.3 ms / lag +0.7 % 다.
//!
//! **"async 서버니까 tokio 타이머"로 되돌리지 말 것.** 게임 로직이 하나도 없는 상태에서
//! 게임 시간이 실제 시간보다 22 % 뒤처지는 서버를 p1 의 기준선으로 박게 된다.
//!
//! # 여기서 지키는 경계
//!
//! - 상태는 [`starfall_sim::Simulation::step`] 안에서만 바뀐다 (I-13).
//! - **제어 제출(세션 열기·닫기)은 명령 큐를 쓰지 않는다** (I-16). 큐가 가득 찼을 때 닫기가
//!   거부되면 `SESSION_OPENED` 만 있고 `SESSION_CLOSED` 가 없는 세션이 생긴다 — 부하
//!   상황에서만 깨지는, 가장 찾기 어려운 형태의 버그다.
//! - tick 안에서 시계를 읽지 않는다. 루프 계측(`Instant`)은 tick **밖**이다.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU32, Ordering};
use std::time::{Duration, Instant};

use starfall_contracts::UuidV7;
use starfall_contracts::events::SessionCloseReason;
use starfall_contracts::messages::RejectReasonCode;
use starfall_sim::{IdSource, InboundCommand, PersistBatch, ServerMessage, Simulation, Submission};
use tokio::sync::{Notify, mpsc};

use crate::stats::Stats;

/// 전역 명령 큐 용량 (ADR-0006 §5). 가득 차면 `SERVER_BUSY` 로 **거부**한다.
///
/// **30 연결에서는 구조적으로 도달하지 않는다**: 세션 in-flight 상한이 64 이므로
/// 30 × 64 = 1920 < 4096 이다(AC-7d). 그래서 `SERVER_BUSY` 경로는 단위 테스트로 덮고
/// 부하 리포트에는 "구조적 미도달"로 기록한다. 경로를 보려고 정상 부하에서 거부가 나는
/// 설정을 만드는 것은 목적과 수단이 바뀐 것이다.
pub const COMMAND_QUEUE_CAPACITY: usize = 4096;

/// 세션당 미응답 명령 상한 (ADR-0006 §5).
///
/// **해제 시점은 `COMMAND_RESULT` 를 만든 시점**이지 클라이언트가 받은 시점이 아니다.
/// 이 선택이 AC-7(c)(`SLOW_CONSUMER`)를 도달 가능하게 만든다 — 수신을 멈춘 클라이언트도
/// 계속 보낼 수 있어 송신 큐가 찬다. 전달 확인 시점으로 바꾸면 한 세션이 만들 수 있는
/// 응답이 128건으로 묶여 256 슬롯 송신 큐가 **절대** 차지 않는다.
pub const SESSION_IN_FLIGHT_LIMIT: u32 = 64;

/// 세션별 송신 큐 용량 (ADR-0006 §5). 가득 차면 **연결을 닫는다**.
pub const SEND_QUEUE_CAPACITY: usize = 256;

/// 영속화 채널 용량.
pub const PERSIST_QUEUE_CAPACITY: usize = 512;

/// 영속화 백로그 임계 (tick). 20 Hz 에서 600 tick = 30초 (ADR-0007 §4).
///
/// 넘으면 **새 연결 수락을 중단**한다. 기존 세션은 유지한다. 이벤트를 조용히 버리는 것보다
/// 접속을 거절하는 편이 낫다.
pub const PERSIST_BACKLOG_LIMIT: u64 = 600;

/// 이벤트가 없어도 이 주기마다 영속화 배치를 보낸다 (20 tick = 1초).
///
/// 두 가지를 한다. (1) `worlds.last_tick` 을 최신으로 유지해 재개 지점을 좁힌다.
/// (2) **DB 가 살아 있는지 계속 확인한다** — 이게 없으면 A 단계처럼 새 세션이 생기지 않는
/// 구간에서 PostgreSQL 을 내려도 백로그가 0 으로 유지되어 AC-19 가 재현되지 않는다.
pub const HEARTBEAT_TICKS: u64 = 20;

/// 세션이 닫힐 이유를 **처음 정한 쪽이 이긴다**.
///
/// 읽기·쓰기 태스크와 tick 드라이버 셋이 각자 종료를 결정할 수 있어 공유 셀이 필요하다.
#[derive(Debug, Clone)]
pub struct CloseState(Arc<AtomicU8>);

impl Default for CloseState {
    fn default() -> Self {
        Self::new()
    }
}

impl CloseState {
    /// 아직 정해지지 않은 상태로 만든다.
    #[must_use]
    pub fn new() -> Self {
        Self(Arc::new(AtomicU8::new(0)))
    }

    /// 아직 비어 있으면 이유를 적는다. 이미 정해졌으면 아무 일도 하지 않는다.
    pub fn set_if_unset(&self, reason: SessionCloseReason) {
        let _ = self
            .0
            .compare_exchange(0, encode(reason), Ordering::AcqRel, Ordering::Acquire);
    }

    /// 정해진 이유. 아무도 정하지 않았으면 `CLIENT_CLOSED`(피어가 정상 종료).
    #[must_use]
    pub fn get(&self) -> SessionCloseReason {
        decode(self.0.load(Ordering::Acquire))
    }
}

const fn encode(reason: SessionCloseReason) -> u8 {
    match reason {
        SessionCloseReason::ClientClosed => 1,
        SessionCloseReason::IdleTimeout => 2,
        SessionCloseReason::ProtocolViolation => 3,
        SessionCloseReason::SlowConsumer => 4,
        SessionCloseReason::ServerShutdown => 5,
        SessionCloseReason::TransportError => 6,
    }
}

const fn decode(value: u8) -> SessionCloseReason {
    match value {
        2 => SessionCloseReason::IdleTimeout,
        3 => SessionCloseReason::ProtocolViolation,
        4 => SessionCloseReason::SlowConsumer,
        5 => SessionCloseReason::ServerShutdown,
        6 => SessionCloseReason::TransportError,
        _ => SessionCloseReason::ClientClosed,
    }
}

/// `close_reason` → WebSocket close code (ADR-0005 §2 표).
///
/// 둘을 하나로 합치지 않는다: close code 는 전송 계층의 어휘이고 `close_reason` 은
/// **세계의 사실**이다. `TRANSPORT_ERROR` 에는 close code 가 없다(소켓이 이미 깨졌다).
#[must_use]
pub const fn close_code(reason: SessionCloseReason) -> Option<u16> {
    match reason {
        SessionCloseReason::ClientClosed => Some(1000),
        SessionCloseReason::IdleTimeout | SessionCloseReason::ServerShutdown => Some(1001),
        SessionCloseReason::ProtocolViolation => Some(1002),
        SessionCloseReason::SlowConsumer => Some(1011),
        SessionCloseReason::TransportError => None,
    }
}

/// 드라이버가 한 세션으로 가는 길을 잡아 두는 핸들.
#[derive(Debug)]
pub struct SessionRoute {
    /// 송신 큐.
    pub outbound: mpsc::Sender<ServerMessage>,
    /// 미응답 명령 수. 게이트웨이가 올리고 드라이버가 내린다.
    pub in_flight: Arc<AtomicU32>,
    /// 종료 사유 공유 셀.
    pub close_state: CloseState,
    /// 송신 태스크를 깨우는 신호.
    ///
    /// # 왜 `Sender` 를 drop 하는 것으로는 부족한가
    ///
    /// 송신 채널의 `Sender` 는 **수신 태스크도 들고 있다**(거부 응답을 직접 넣어야 하므로).
    /// 그래서 드라이버가 자기 클론을 버려도 채널은 닫히지 않고, 송신 태스크는 조용한
    /// 연결에서 다음 ping(15초)까지 깨어나지 않는다. 그러면 정상 종료가 Close 프레임을
    /// 내보내지 못한 채 프로세스가 죽는다 — 실측으로 확인한 경로다(2026-09-19).
    pub finish: Arc<Notify>,
}

/// 제어 제출. **명령 큐와 분리되어 있고 거부 대상이 아니다** (I-16).
#[derive(Debug)]
pub enum Control {
    /// 인증된 연결이 생겼다.
    Open {
        /// 제출 순번.
        seq: u64,
        /// 세션 id.
        session_id: UuidV7,
        /// 행위자.
        actor_id: UuidV7,
        /// 라우팅 핸들.
        route: SessionRoute,
    },
    /// 연결이 끝났다.
    Close {
        /// 제출 순번.
        seq: u64,
        /// 세션 id.
        session_id: UuidV7,
        /// 세계의 사실로 남을 이유.
        reason: SessionCloseReason,
    },
}

/// 게이트웨이가 tick 루프에 제출하는 유일한 통로.
///
/// **시뮬레이션 상태 타입에 접근할 수 없다** (I-13). 가진 것은 채널과 순번 발급기뿐이다.
#[derive(Debug, Clone)]
pub struct SubmitHandle {
    control: mpsc::UnboundedSender<Control>,
    commands: mpsc::Sender<Submission>,
    next_seq: Arc<std::sync::atomic::AtomicU64>,
    stats: Stats,
}

impl SubmitHandle {
    /// 전역 단조 증가 제출 순번. **한 tick 안의 처리 순서를 정하는 유일한 근거다**
    /// (수신 시각이나 `client_sent_at` 이 아니다 — ADR-0006 §4, I-11).
    #[must_use]
    pub fn next_seq(&self) -> u64 {
        self.next_seq.fetch_add(1, Ordering::Relaxed)
    }

    /// 세션 열기. 거부되지 않는다.
    pub fn open(&self, session_id: UuidV7, actor_id: UuidV7, route: SessionRoute) {
        let _ = self.control.send(Control::Open {
            seq: self.next_seq(),
            session_id,
            actor_id,
            route,
        });
    }

    /// 세션 닫기. 거부되지 않는다.
    pub fn close(&self, session_id: UuidV7, reason: SessionCloseReason) {
        let _ = self.control.send(Control::Close {
            seq: self.next_seq(),
            session_id,
            reason,
        });
    }

    /// 명령 제출. 전역 큐가 가득 차면 `SERVER_BUSY` 로 **거부**한다 (드롭도 블로킹도 아니다).
    ///
    /// # Errors
    ///
    /// 큐 포화 시 [`RejectReasonCode::ServerBusy`].
    pub fn try_command(
        &self,
        session_id: UuidV7,
        command: InboundCommand,
    ) -> Result<(), RejectReasonCode> {
        let submission = Submission::Command {
            seq: self.next_seq(),
            session_id,
            command,
        };
        self.commands.try_send(submission).map_err(|error| {
            match error {
                mpsc::error::TrySendError::Full(_) => RejectReasonCode::ServerBusy,
                // 드라이버가 죽었다 — 종료 중이다. 같은 사유로 답한다.
                mpsc::error::TrySendError::Closed(_) => RejectReasonCode::ServerBusy,
            }
        })
    }

    /// 관측 핸들.
    #[must_use]
    pub const fn stats(&self) -> &Stats {
        &self.stats
    }
}

/// UUIDv7 생성기 (결정적 코어 **밖**).
#[derive(Debug, Default)]
struct SystemIds;

impl IdSource for SystemIds {
    fn next_id(&mut self) -> UuidV7 {
        UuidV7::new_v7()
    }
}

/// tick 루프를 전용 OS 스레드에서 돌린다.
///
/// 반환된 핸들을 join 하면 **종료 스윕과 영속화 flush 가 끝난 뒤**에 돌아온다 (AC-8d).
pub fn spawn_tick_thread(
    mut sim: Simulation,
    mut control_rx: mpsc::UnboundedReceiver<Control>,
    mut command_rx: mpsc::Receiver<Submission>,
    persist_tx: mpsc::Sender<PersistBatch>,
    stats: Stats,
    tick_period: Duration,
    shutdown: Arc<AtomicBool>,
) -> std::thread::JoinHandle<()> {
    std::thread::Builder::new()
        .name("starfall-tick".to_owned())
        .spawn(move || {
            let mut ids = SystemIds;
            let mut routes: HashMap<UuidV7, SessionRoute> = HashMap::new();
            let mut deferred: VecDeque<PersistBatch> = VecDeque::new();
            let mut carry: Vec<Submission> = Vec::new();

            let period_nanos = u64::try_from(tick_period.as_nanos()).unwrap_or(50_000_000);
            let loop_start = Instant::now();
            let mut executed: u64 = 0;

            while !shutdown.load(Ordering::Acquire) {
                let body_start = Instant::now();

                // ── 1. 제출 수집 ──────────────────────────────────────────────
                let mut submissions = std::mem::take(&mut carry);
                while let Ok(control) = control_rx.try_recv() {
                    match control {
                        Control::Open {
                            seq,
                            session_id,
                            actor_id,
                            route,
                        } => {
                            routes.insert(session_id, route);
                            submissions.push(Submission::OpenSession {
                                seq,
                                session_id,
                                actor_id,
                            });
                        }
                        Control::Close {
                            seq,
                            session_id,
                            reason,
                        } => submissions.push(Submission::CloseSession {
                            seq,
                            session_id,
                            reason,
                        }),
                    }
                }
                while let Ok(submission) = command_rx.try_recv() {
                    submissions.push(submission);
                }
                stats.set_command_queue_depth(command_rx.len() as u64);

                // ── 2~4. 판정·상태 전이·이벤트 발행 (상태가 바뀌는 유일한 자리) ──
                let outcome = sim.step(submissions, &mut ids);

                // ── 7. 세션별 송신 큐로 ───────────────────────────────────────
                route_outbound(&mut routes, &outcome.outbound, &stats, &mut carry);

                for session_id in &outcome.closed_sessions {
                    routes.remove(session_id);
                }

                let opened = count_events(&outcome, starfall_contracts::registry::SESSION_OPENED);
                let closed = count_events(&outcome, starfall_contracts::registry::SESSION_CLOSED);
                stats.add_sessions(opened, closed);
                stats.set_ws_connections(routes.len() as u64);
                stats.set_send_queue_depth(send_queue_depth(&routes));

                // ── 6. 영속화 태스크로 (기다리지 않는다) ──────────────────────
                if !outcome.events.is_empty() || outcome.tick.is_multiple_of(HEARTBEAT_TICKS) {
                    deferred.push_back(PersistBatch {
                        tick: outcome.tick,
                        events: outcome.events,
                    });
                }
                drain_deferred(&mut deferred, &persist_tx);

                // ── 계측: tick **본문** 소요. 루프 주기가 아니다 (ADR-0006 §2.2). ──
                let body = body_start.elapsed();
                executed = executed.saturating_add(1);
                let target = Duration::from_nanos(period_nanos.saturating_mul(executed));
                let elapsed_total = loop_start.elapsed();
                let lag_micros = i64::try_from(elapsed_total.as_micros()).unwrap_or(i64::MAX)
                    - i64::try_from(target.as_micros()).unwrap_or(i64::MAX);
                stats.record_tick(
                    outcome.tick,
                    u64::try_from(body.as_micros()).unwrap_or(u64::MAX),
                    lag_micros,
                );

                // tick 을 건너뛰지 않고 따라잡지도 않는다 (ADR-0006 §2).
                if let Some(remaining) = target.checked_sub(elapsed_total) {
                    std::thread::sleep(remaining);
                }
            }

            // ── 종료 스윕 (I-16, AC-8d) ──────────────────────────────────────
            stats.set_accepting(false);
            let final_outcome = sim.shutdown(&mut ids);
            let closed = final_outcome.closed_sessions.len() as u64;
            for (_, route) in routes.drain() {
                route
                    .close_state
                    .set_if_unset(SessionCloseReason::ServerShutdown);
                // 신호를 보내야 송신 태스크가 깨어나 Close(1001) 를 내보낸다.
                route.finish.notify_one();
            }
            stats.add_sessions(0, closed);
            stats.set_ws_connections(0);
            if !final_outcome.events.is_empty() {
                deferred.push_back(PersistBatch {
                    tick: final_outcome.tick,
                    events: final_outcome.events,
                });
            }
            tracing::info!(
                tick = final_outcome.tick,
                sessions_closed = closed,
                pending_batches = deferred.len(),
                "tick 루프 종료 — SERVER_SHUTDOWN 스윕 후 영속화 flush"
            );
            // 남은 배치를 **기다려서** 전부 넘긴다. 여기서 버리면 AC-8(d) 가 거짓이 된다.
            while let Some(batch) = deferred.pop_front() {
                if persist_tx.blocking_send(batch).is_err() {
                    tracing::error!("영속화 채널이 닫혀 배치를 넘기지 못했다");
                    break;
                }
            }
            drop(persist_tx);
        })
        .unwrap_or_else(|error| {
            // 스레드 생성 실패는 기동 실패와 같다. 여기서 패닉하지 않고 빈 핸들을 만들 수는
            // 없으므로, 실패를 드러내고 프로세스를 정상 경로로 내보낸다.
            tracing::error!(%error, "tick 스레드를 만들지 못했다");
            std::thread::spawn(|| {})
        })
}

fn count_events(outcome: &starfall_sim::TickOutcome, event_type: &str) -> u64 {
    outcome
        .events
        .iter()
        .filter(|event| event.body.event_type() == event_type)
        .count() as u64
}

fn send_queue_depth(routes: &HashMap<UuidV7, SessionRoute>) -> u64 {
    routes
        .values()
        .map(|route| {
            (route
                .outbound
                .max_capacity()
                .saturating_sub(route.outbound.capacity())) as u64
        })
        .sum()
}

fn route_outbound(
    routes: &mut HashMap<UuidV7, SessionRoute>,
    outbound: &[starfall_sim::Outbound],
    stats: &Stats,
    carry: &mut Vec<Submission>,
) {
    let mut slow: Vec<UuidV7> = Vec::new();

    for out in outbound {
        let Some(route) = routes.get(&out.session_id) else {
            continue;
        };
        let type_name = out.message.type_name();

        // in-flight 는 **COMMAND_RESULT 를 만든 시점**에 해제한다 (ADR-0006 §5).
        if matches!(out.message, ServerMessage::CommandResult(_)) {
            let _ = route
                .in_flight
                .fetch_update(Ordering::AcqRel, Ordering::Acquire, |value| {
                    Some(value.saturating_sub(1))
                });
        }

        match route.outbound.try_send(out.message.clone()) {
            Ok(()) => stats.record_message_enqueued(type_name),
            Err(mpsc::error::TrySendError::Full(_)) => {
                // 느린 소비자는 시간이 지난다고 회복되지 않고, tick 루프가 그 연결을 기다리면
                // 세계 전체가 한 클라이언트에 인질이 된다 (ADR-0006 §5).
                route
                    .close_state
                    .set_if_unset(SessionCloseReason::SlowConsumer);
                route.finish.notify_one();
                slow.push(out.session_id);
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {
                // 송신 태스크가 이미 끝났다. 읽기 태스크가 곧 CloseSession 을 제출한다.
            }
        }
    }

    for session_id in slow {
        if routes.remove(&session_id).is_some() {
            tracing::warn!(%session_id, "송신 큐 포화 — SLOW_CONSUMER 로 연결을 닫는다");
            carry.push(Submission::CloseSession {
                // 이 제출은 다음 tick 의 맨 앞에서 처리된다. 순번은 0 이어도 안전하다 —
                // carry 는 다음 tick 의 제출 목록 앞에 붙고 sim 이 안정 정렬한다.
                seq: 0,
                session_id,
                reason: SessionCloseReason::SlowConsumer,
            });
        }
    }
}

fn drain_deferred(deferred: &mut VecDeque<PersistBatch>, persist_tx: &mpsc::Sender<PersistBatch>) {
    while let Some(batch) = deferred.pop_front() {
        match persist_tx.try_send(batch) {
            Ok(()) => {}
            Err(mpsc::error::TrySendError::Full(batch)) => {
                // **버리지 않는다** (I-22). 미뤄 두면 persist_backlog 가 자라고,
                // 임계를 넘으면 게이트웨이가 새 연결을 거절한다.
                deferred.push_front(batch);
                return;
            }
            Err(mpsc::error::TrySendError::Closed(_)) => return,
        }
    }
}

/// 제출 경로와 tick 스레드를 함께 만든다.
#[must_use]
pub fn build(
    sim: Simulation,
    persist_tx: mpsc::Sender<PersistBatch>,
    stats: Stats,
    tick_period: Duration,
    shutdown: Arc<AtomicBool>,
) -> (SubmitHandle, std::thread::JoinHandle<()>) {
    let (control_tx, control_rx) = mpsc::unbounded_channel();
    let (command_tx, command_rx) = mpsc::channel(COMMAND_QUEUE_CAPACITY);

    let handle = SubmitHandle {
        control: control_tx,
        commands: command_tx,
        next_seq: Arc::new(std::sync::atomic::AtomicU64::new(0)),
        stats: stats.clone(),
    };
    let thread = spawn_tick_thread(
        sim,
        control_rx,
        command_rx,
        persist_tx,
        stats,
        tick_period,
        shutdown,
    );
    (handle, thread)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn close_state_keeps_the_first_reason() {
        let state = CloseState::new();
        assert_eq!(state.get(), SessionCloseReason::ClientClosed);
        state.set_if_unset(SessionCloseReason::SlowConsumer);
        state.set_if_unset(SessionCloseReason::ServerShutdown);
        assert_eq!(state.get(), SessionCloseReason::SlowConsumer);
    }

    /// ADR-0005 §2 표와 글자 그대로 일치해야 한다 (AC-8e).
    #[test]
    fn close_codes_match_adr_0005_table() {
        assert_eq!(close_code(SessionCloseReason::ClientClosed), Some(1000));
        assert_eq!(close_code(SessionCloseReason::IdleTimeout), Some(1001));
        assert_eq!(
            close_code(SessionCloseReason::ProtocolViolation),
            Some(1002)
        );
        assert_eq!(close_code(SessionCloseReason::SlowConsumer), Some(1011));
        assert_eq!(close_code(SessionCloseReason::ServerShutdown), Some(1001));
        assert_eq!(close_code(SessionCloseReason::TransportError), None);
    }

    /// AC-7(d) — `SERVER_BUSY` 는 30 연결에서 구조적으로 도달하지 않는다.
    #[test]
    fn server_busy_is_structurally_unreachable_at_thirty_connections() {
        let max_queued = 30 * SESSION_IN_FLIGHT_LIMIT as usize;
        assert!(
            max_queued < COMMAND_QUEUE_CAPACITY,
            "30 × {SESSION_IN_FLIGHT_LIMIT} = {max_queued} 는 {COMMAND_QUEUE_CAPACITY} 보다 작다"
        );
    }
}

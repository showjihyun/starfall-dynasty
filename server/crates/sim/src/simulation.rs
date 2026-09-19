//! tick 1회 = [`Simulation::step`] 1회.
//!
//! ADR-0006 §4 의 7단계 중 이 슬라이스에 존재하는 것은 1·2·4·7 이다. 3(상태 전이)은
//! 바꿀 월드 상태가 없고, 5(역사 판정)는 p2, 6(영속화)은 반환값을 받은 쪽이 한다.
//! **없는 단계를 빈 함수로 만들어 두지 않는다** (스펙 §4).

use std::collections::BTreeMap;

use starfall_contracts::events::{
    SessionCloseReason, SessionClosedPayload, SessionOpenedPayload, SessionTransport,
};
use starfall_contracts::messages::{
    CommandResultMessage, CommandResultPayload, CommandResultType, PingReplyMessage,
    PingReplyPayload, PingReplyType, RejectReasonCode, SessionReadyMessage, SessionReadyPayload,
    SessionReadyType,
};
use starfall_contracts::primitives::{
    ConstSchemaVersion, GameCalendar, GameTime, Sequence, ServerVersion, Tick, TickHz, UuidV7,
};
use starfall_contracts::{PingServerCommand, registry};

use crate::session::{SessionSnapshot, SessionState};

/// UUIDv7 공급자.
///
/// UUIDv7 은 시계를 읽으므로 결정적 코어가 직접 만들면 안 된다 (ADR-0002 §5).
/// 운영은 실제 시계 기반 생성기를, 테스트는 결정적 생성기를 넣는다.
pub trait IdSource {
    /// 다음 id.
    fn next_id(&mut self) -> UuidV7;
}

impl<F: FnMut() -> UuidV7> IdSource for F {
    fn next_id(&mut self) -> UuidV7 {
        self()
    }
}

/// 한 월드의 불변 상수 (`worlds` 행에서 온다).
#[derive(Debug, Clone)]
pub struct WorldConstants {
    /// 월드(샤드) id.
    pub world_id: UuidV7,
    /// 게임 달력 상수. 월드 수명 동안 불변 (I-19).
    pub calendar: GameCalendar,
    /// 서버 빌드 버전. `SESSION_READY` 가 실어 보낸다.
    pub server_version: ServerVersion,
}

/// 게이트웨이가 tick 루프에 넣는 제출.
///
/// `seq` 는 **게이트웨이 수신 순번**(전역 단조 증가)이고 한 tick 안의 처리 순서를 정한다.
/// 수신 시각이나 `client_sent_at` 으로 정렬하지 않는다 (ADR-0006 §4, I-11).
#[derive(Debug)]
pub enum Submission {
    /// 인증된 연결이 생겼다. **거부 대상이 아니다** (I-16).
    OpenSession {
        /// 제출 순번.
        seq: u64,
        /// 게이트웨이가 만든 세션 id.
        session_id: UuidV7,
        /// 검증된 토큰의 주체 (I-10).
        actor_id: UuidV7,
    },
    /// 명령이 도착했다.
    Command {
        /// 제출 순번.
        seq: u64,
        /// 보낸 세션.
        session_id: UuidV7,
        /// 파싱된 명령.
        command: InboundCommand,
    },
    /// 연결이 끝났다. **거부 대상이 아니다** (I-16).
    CloseSession {
        /// 제출 순번.
        seq: u64,
        /// 닫히는 세션.
        session_id: UuidV7,
        /// 세계의 사실로 남을 이유.
        reason: SessionCloseReason,
    },
}

impl Submission {
    /// 제출 순번.
    #[must_use]
    pub const fn seq(&self) -> u64 {
        match self {
            Self::OpenSession { seq, .. }
            | Self::Command { seq, .. }
            | Self::CloseSession { seq, .. } => *seq,
        }
    }

    /// 어느 세션의 제출인가.
    #[must_use]
    pub const fn session_id(&self) -> UuidV7 {
        match self {
            Self::OpenSession { session_id, .. }
            | Self::Command { session_id, .. }
            | Self::CloseSession { session_id, .. } => *session_id,
        }
    }
}

/// 이 슬라이스가 받는 명령.
///
/// 열거형이지만 **내부 태그 serde 열거형이 아니다** — 역직렬화는 게이트웨이가 peek 후
/// 구체 타입으로 하고(ADR-0002 §3), 여기 오는 것은 이미 검증된 값이다.
#[derive(Debug, Clone)]
pub enum InboundCommand {
    /// `PING_SERVER`.
    PingServer(PingServerCommand),
}

impl InboundCommand {
    /// 멱등 키 (I-11).
    #[must_use]
    pub const fn command_id(&self) -> UuidV7 {
        match self {
            Self::PingServer(command) => command.command_id,
        }
    }

    /// 레지스트리 타입 이름.
    #[must_use]
    pub const fn type_name(&self) -> &'static str {
        match self {
            Self::PingServer(_) => registry::PING_SERVER,
        }
    }
}

/// 서버 → 클라이언트 메시지.
#[derive(Debug, Clone)]
pub enum ServerMessage {
    /// 세션의 첫 계약 메시지 (ADR-0005 §3).
    SessionReady(Box<SessionReadyMessage>),
    /// 명령 판정 결과.
    CommandResult(Box<CommandResultMessage>),
    /// `PING_SERVER` 의 타입별 결과.
    PingReply(Box<PingReplyMessage>),
}

impl ServerMessage {
    /// 레지스트리 타입 이름. 메트릭 라벨로 쓴다.
    #[must_use]
    pub const fn type_name(&self) -> &'static str {
        match self {
            Self::SessionReady(_) => registry::SESSION_READY,
            Self::CommandResult(_) => registry::COMMAND_RESULT,
            Self::PingReply(_) => registry::PING_REPLY,
        }
    }
}

/// 한 세션에 보낼 메시지.
#[derive(Debug, Clone)]
pub struct Outbound {
    /// 받을 세션.
    pub session_id: UuidV7,
    /// 보낼 메시지.
    pub message: ServerMessage,
}

/// 도메인 이벤트의 타입별 본문.
#[derive(Debug, Clone)]
pub enum DomainEventBody {
    /// `SESSION_OPENED`.
    SessionOpened(SessionOpenedPayload),
    /// `SESSION_CLOSED`.
    SessionClosed(SessionClosedPayload),
}

impl DomainEventBody {
    /// 레지스트리 타입 이름.
    #[must_use]
    pub const fn event_type(&self) -> &'static str {
        match self {
            Self::SessionOpened(_) => registry::SESSION_OPENED,
            Self::SessionClosed(_) => registry::SESSION_CLOSED,
        }
    }
}

/// **`recorded_at` 이 아직 없는** 도메인 이벤트.
///
/// `recorded_at` 은 실제 시각이고, tick 안에서는 시계를 읽지 않는다 (ADR-0006 §2).
/// 영속화 단계가 그 필드를 채워 계약 타입을 완성한다 (ADR-0007 §4).
#[derive(Debug, Clone)]
pub struct PendingEvent {
    /// 저장·전달 멱등 키.
    pub event_id: UuidV7,
    /// 월드 id.
    pub world_id: UuidV7,
    /// 발행 tick.
    pub tick: Tick,
    /// tick 안의 발행 순서. 0부터 빈틈없이 (I-18).
    pub sequence: Sequence,
    /// `tick` 에서 파생한 게임 시간 (I-19).
    pub occurred_at: GameTime,
    /// 세션 correlation.
    pub correlation_id: UuidV7,
    /// 이 슬라이스에서는 언제나 `None`.
    pub causation_id: Option<UuidV7>,
    /// 행위자 (`SESSION_*` 은 비-null).
    pub actor_id: UuidV7,
    /// 타입별 본문.
    pub body: DomainEventBody,
}

/// tick 1회의 결과.
///
/// **이 구조체가 이 크레이트의 유일한 출력이다.** 채널도 소켓도 DB 도 여기서 다루지 않는다.
#[derive(Debug, Default)]
pub struct TickOutcome {
    /// 실행한 tick 번호.
    pub tick: u64,
    /// 이 tick 이 발행한 도메인 이벤트 (`sequence` 순).
    pub events: Vec<PendingEvent>,
    /// 이 tick 이 보낼 메시지 (생성 순 — `COMMAND_RESULT` 가 `PING_REPLY` 보다 앞이다, I-15).
    pub outbound: Vec<Outbound>,
    /// 이 tick 에서 닫힌 세션. 드라이버가 라우팅 표에서 지운다.
    pub closed_sessions: Vec<UuidV7>,
}

/// 영속화 태스크로 넘기는 한 tick 의 기록 단위.
///
/// 이 타입이 `starfall-sim` 에 있는 이유: 게이트웨이(보내는 쪽)와 `starfall-persistence`
/// (받는 쪽)가 **서로를 의존하지 않고** 같은 타입을 쓰게 하기 위해서다. 채널도 DB 도
/// 여기서는 등장하지 않으므로 이 크레이트의 "IO 없음"은 그대로다.
///
/// `events` 가 비어 있을 수 있다 — 하트비트 배치다. `worlds.last_tick` 만 갱신해
/// (a) 재개 지점을 최신으로 유지하고, (b) **DB 가 살아 있는지 계속 확인한다.**
/// (b)가 없으면 이벤트가 없는 구간에서 DB 가 죽어도 백로그가 자라지 않아
/// "기록을 버리지 않는다"의 검증(AC-19)이 성립하지 않는다.
#[derive(Debug)]
pub struct PersistBatch {
    /// 이 배치가 대표하는 tick.
    pub tick: u64,
    /// 그 tick 이 발행한 이벤트 (`sequence` 순). 하트비트면 비어 있다.
    pub events: Vec<PendingEvent>,
}

/// 시뮬레이션 상태 전체.
#[derive(Debug)]
pub struct Simulation {
    world: WorldConstants,
    /// 다음에 실행할 tick.
    next_tick: u64,
    /// `session_id` 순 정렬 — 종료 스윕의 순서를 결정적으로 만든다.
    sessions: BTreeMap<UuidV7, SessionState>,
}

impl Simulation {
    /// `start_tick` 부터 시작하는 시뮬레이션을 만든다.
    ///
    /// `start_tick` 은 `worlds.last_tick` 과 `max(domain_events.tick)` 중 큰 값 + 1 이다
    /// (ADR-0006 §2.3). 0 부터 시작하는 것은 **그 월드에 아무 기록도 없을 때뿐**이다.
    #[must_use]
    pub fn new(world: WorldConstants, start_tick: u64) -> Self {
        Self {
            world,
            next_tick: start_tick,
            sessions: BTreeMap::new(),
        }
    }

    /// 다음에 실행할 tick.
    #[must_use]
    pub const fn next_tick(&self) -> u64 {
        self.next_tick
    }

    /// 지금 열려 있는 세션 수.
    #[must_use]
    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }

    /// 세션 상태 읽기 (테스트·진단 전용).
    #[must_use]
    pub fn session(&self, session_id: UuidV7) -> Option<SessionSnapshot> {
        self.sessions.get(&session_id).map(SessionState::snapshot)
    }

    /// 월드 상수.
    #[must_use]
    pub const fn world(&self) -> &WorldConstants {
        &self.world
    }

    /// tick 1회를 실행한다. **상태가 바뀌는 유일한 자리다** (I-13).
    ///
    /// `submissions` 는 제출 순번으로 정렬된 뒤 소비된다. 호출자가 정렬 상태를 보장할 필요는
    /// 없다 — 여러 채널에서 모았을 수 있으므로 여기서 한 번 더 정렬한다.
    pub fn step(
        &mut self,
        mut submissions: Vec<Submission>,
        ids: &mut dyn IdSource,
    ) -> TickOutcome {
        let tick_number = self.next_tick;
        self.next_tick = self.next_tick.saturating_add(1);

        let mut outcome = TickOutcome {
            tick: tick_number,
            ..TickOutcome::default()
        };

        // tick 번호가 계약 상한을 넘으면 이 tick 은 아무 일도 하지 않는다.
        // (2^53-1 tick = 20 Hz 로 약 1400만 년. 방어적 분기이지 운영 경로가 아니다.)
        let Some(tick) = Tick::new(tick_number) else {
            return outcome;
        };
        let Some(occurred_at) = self.world.calendar.occurred_at(tick) else {
            return outcome;
        };

        submissions.sort_by_key(Submission::seq);

        for submission in submissions {
            match submission {
                Submission::OpenSession {
                    session_id,
                    actor_id,
                    ..
                } => self.open_session(session_id, actor_id, tick, &occurred_at, ids, &mut outcome),
                Submission::Command {
                    session_id,
                    command,
                    ..
                } => self.handle_command(session_id, command, tick, ids, &mut outcome),
                Submission::CloseSession {
                    session_id, reason, ..
                } => self.close_session(session_id, reason, tick, &occurred_at, ids, &mut outcome),
            }
        }

        outcome
    }

    /// 살아 있는 모든 세션을 `SERVER_SHUTDOWN` 으로 닫는 마지막 tick (I-16, AC-8d).
    ///
    /// `BTreeMap` 순회라 세션 id 순으로 결정적이다.
    pub fn shutdown(&mut self, ids: &mut dyn IdSource) -> TickOutcome {
        let open: Vec<UuidV7> = self.sessions.keys().copied().collect();
        let submissions = open
            .into_iter()
            .enumerate()
            .map(|(index, session_id)| Submission::CloseSession {
                seq: index as u64,
                session_id,
                reason: SessionCloseReason::ServerShutdown,
            })
            .collect();
        self.step(submissions, ids)
    }

    fn open_session(
        &mut self,
        session_id: UuidV7,
        actor_id: UuidV7,
        tick: Tick,
        occurred_at: &GameTime,
        ids: &mut dyn IdSource,
        outcome: &mut TickOutcome,
    ) {
        if self.sessions.contains_key(&session_id) {
            return;
        }
        let correlation_id = ids.next_id();
        self.sessions
            .insert(session_id, SessionState::new(actor_id, correlation_id));

        outcome.events.push(PendingEvent {
            event_id: ids.next_id(),
            world_id: self.world.world_id,
            tick,
            sequence: next_sequence(outcome),
            occurred_at: occurred_at.clone(),
            correlation_id,
            causation_id: None,
            actor_id,
            body: DomainEventBody::SessionOpened(SessionOpenedPayload {
                session_id,
                transport: SessionTransport::Websocket,
            }),
        });

        // SESSION_READY 는 그 연결의 첫 계약 메시지다 (ADR-0005 §3).
        // 게이트웨이는 tick 이 이 메시지를 돌려주기 전까지 아무것도 보내지 않는다.
        let Some(tick_hz) = TickHz::new(self.world.calendar.tick_hz()) else {
            return;
        };
        outcome.outbound.push(Outbound {
            session_id,
            message: ServerMessage::SessionReady(Box::new(SessionReadyMessage {
                message_id: ids.next_id(),
                message_type: SessionReadyType::SessionReady,
                schema_version: ConstSchemaVersion,
                tick,
                correlation_id: Some(correlation_id),
                payload: SessionReadyPayload {
                    session_id,
                    world_id: self.world.world_id,
                    actor_id,
                    tick_hz,
                    server_version: self.world.server_version.clone(),
                },
            })),
        });
    }

    fn handle_command(
        &mut self,
        session_id: UuidV7,
        command: InboundCommand,
        tick: Tick,
        ids: &mut dyn IdSource,
        outcome: &mut TickOutcome,
    ) {
        // 세션이 이미 닫혔으면 답할 곳이 없다. 조용히 버리는 것이 아니라
        // **보낼 대상이 존재하지 않는 것**이고, 드라이버가 in-flight 를 정리한다.
        let Some(session) = self.sessions.get_mut(&session_id) else {
            return;
        };

        let command_id = command.command_id();
        let accepted = session.remember(command_id);

        let payload = if accepted {
            CommandResultPayload::accepted(command_id)
        } else {
            CommandResultPayload::rejected(command_id, RejectReasonCode::DuplicateCommandId)
        };

        // I-15: COMMAND_RESULT 가 타입별 결과보다 **먼저** 큐에 들어간다.
        outcome.outbound.push(Outbound {
            session_id,
            message: ServerMessage::CommandResult(Box::new(CommandResultMessage {
                message_id: ids.next_id(),
                message_type: CommandResultType::CommandResult,
                schema_version: ConstSchemaVersion,
                tick,
                // 스펙 §5.2: 명령은 게임플레이 트랜잭션을 시작하지 않았다. 언제나 null.
                correlation_id: None,
                payload,
            })),
        });

        if !accepted {
            // 중복은 시뮬레이션에 두 번 넣지 않는다 (ADR-0006 §6).
            // 그래서 PING_REPLY 도 두 번 나가지 않는다 (AC-7b).
            return;
        }

        match command {
            InboundCommand::PingServer(ping) => {
                outcome.outbound.push(Outbound {
                    session_id,
                    message: ServerMessage::PingReply(Box::new(PingReplyMessage {
                        message_id: ids.next_id(),
                        message_type: PingReplyType::PingReply,
                        schema_version: ConstSchemaVersion,
                        tick,
                        correlation_id: None,
                        payload: PingReplyPayload {
                            command_id,
                            probe_seq: ping.payload.probe_seq,
                        },
                    })),
                });
            }
        }
    }

    fn close_session(
        &mut self,
        session_id: UuidV7,
        reason: SessionCloseReason,
        tick: Tick,
        occurred_at: &GameTime,
        ids: &mut dyn IdSource,
        outcome: &mut TickOutcome,
    ) {
        let Some(session) = self.sessions.remove(&session_id) else {
            return;
        };

        outcome.events.push(PendingEvent {
            event_id: ids.next_id(),
            world_id: self.world.world_id,
            tick,
            sequence: next_sequence(outcome),
            occurred_at: occurred_at.clone(),
            correlation_id: session.correlation_id,
            causation_id: None,
            actor_id: session.actor_id,
            body: DomainEventBody::SessionClosed(SessionClosedPayload {
                session_id,
                close_reason: reason,
            }),
        });
        outcome.closed_sessions.push(session_id);
    }
}

/// 이 tick 안의 다음 `sequence`. 0부터 빈틈없이 (I-18).
fn next_sequence(outcome: &TickOutcome) -> Sequence {
    Sequence::new(outcome.events.len() as u64).unwrap_or_else(|| {
        // 한 tick 에 2^53건을 발행하는 일은 없다. 그래도 panic 하지 않는다.
        Sequence::new(0).unwrap_or_else(|| unreachable!("0 은 언제나 유효한 Sequence 다"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use starfall_contracts::{PingServerPayload, PingServerType, ProbeSeq};

    /// 결정적 id 생성기. 운영의 `Uuid::now_v7()` 을 대신한다.
    struct SeqIds(u64);

    impl IdSource for SeqIds {
        fn next_id(&mut self) -> UuidV7 {
            self.0 += 1;
            id(self.0)
        }
    }

    fn id(n: u64) -> UuidV7 {
        let text = format!(
            "01a0b1c2-0000-7{:03x}-8{:03x}-{:012x}",
            n & 0xfff,
            (n >> 12) & 0xfff,
            n
        );
        UuidV7::parse(&text).expect("정규 UUIDv7")
    }

    fn world() -> WorldConstants {
        WorldConstants {
            world_id: UuidV7::parse("01a0b1c2-3d4e-7f01-8a2b-9c0d1e2f3a4b").unwrap(),
            calendar: GameCalendar::new(20, GameTime::parse("3800-01-01T00:00:00Z").unwrap(), 60)
                .unwrap(),
            server_version: ServerVersion::parse("0.1.0").unwrap(),
        }
    }

    fn ping(command_id: UuidV7, probe_seq: u32) -> InboundCommand {
        InboundCommand::PingServer(PingServerCommand {
            command_id,
            command_type: PingServerType::PingServer,
            schema_version: ConstSchemaVersion,
            client_sent_at: None,
            payload: PingServerPayload {
                probe_seq: ProbeSeq(probe_seq),
            },
        })
    }

    /// AC-6(a) / SC-17 — **이 테스트가 이 슬라이스의 아키텍처 전제를 증명한다.**
    ///
    /// `#[tokio::test]` 가 아니다. 런타임도 소켓도 없이 돈다.
    #[test]
    fn commands_do_nothing_until_a_step_runs() {
        let mut sim = Simulation::new(world(), 100);
        let mut ids = SeqIds(0);

        // 세션을 연다.
        let session = id(9001);
        let outcome = sim.step(
            vec![Submission::OpenSession {
                seq: 0,
                session_id: session,
                actor_id: id(7001),
            }],
            &mut ids,
        );
        assert_eq!(outcome.tick, 100);
        assert_eq!(outcome.events.len(), 1);
        assert_eq!(outcome.outbound.len(), 1);

        // 명령 N건을 "제출"한다 — 아직 step 을 돌리지 않았다.
        const N: usize = 5;
        let submissions: Vec<Submission> = (0..N)
            .map(|i| Submission::Command {
                seq: 10 + i as u64,
                session_id: session,
                command: ping(id(2000 + i as u64), i as u32),
            })
            .collect();

        // 제출 목록을 들고만 있는 동안 상태는 전혀 변하지 않는다.
        let before = sim.session(session).expect("세션이 있어야 한다");
        assert_eq!(before.remembered_commands, 0);
        assert_eq!(sim.next_tick(), 101);

        // 1 step — 정확히 기대한 결과만 나온다.
        let outcome = sim.step(submissions, &mut ids);
        println!(
            "[SC-17] 제출한 명령 {N}건, 1 step 후 메시지 {}건",
            outcome.outbound.len()
        );
        assert_eq!(outcome.tick, 101);
        assert!(
            outcome.events.is_empty(),
            "명령은 도메인 이벤트를 만들지 않는다 (ADR-0007 §1)"
        );
        assert_eq!(
            outcome.outbound.len(),
            N * 2,
            "명령당 COMMAND_RESULT + PING_REPLY"
        );
        assert_eq!(sim.session(session).unwrap().remembered_commands, N);

        // I-15: 각 명령에 대해 COMMAND_RESULT 가 PING_REPLY 보다 먼저다.
        for pair in outcome.outbound.chunks(2) {
            assert!(matches!(pair[0].message, ServerMessage::CommandResult(_)));
            assert!(matches!(pair[1].message, ServerMessage::PingReply(_)));
        }
    }

    /// AC-7(b) / SC-21 — 중복 `command_id`.
    #[test]
    fn duplicate_command_id_is_rejected_and_not_replayed() {
        let mut sim = Simulation::new(world(), 0);
        let mut ids = SeqIds(0);
        let session = id(9001);
        sim.step(
            vec![Submission::OpenSession {
                seq: 0,
                session_id: session,
                actor_id: id(7001),
            }],
            &mut ids,
        );

        let command_id = id(4242);
        let outcome = sim.step(
            vec![
                Submission::Command {
                    seq: 1,
                    session_id: session,
                    command: ping(command_id, 1),
                },
                Submission::Command {
                    seq: 2,
                    session_id: session,
                    command: ping(command_id, 1),
                },
            ],
            &mut ids,
        );

        let replies = outcome
            .outbound
            .iter()
            .filter(|out| matches!(out.message, ServerMessage::PingReply(_)))
            .count();
        let results: Vec<&CommandResultMessage> = outcome
            .outbound
            .iter()
            .filter_map(|out| match &out.message {
                ServerMessage::CommandResult(message) => Some(message.as_ref()),
                _ => None,
            })
            .collect();

        assert_eq!(replies, 1, "PING_REPLY 는 2건이 아니다");
        assert_eq!(
            results.len(),
            2,
            "COMMAND_RESULT 는 명령당 정확히 1건 (I-15)"
        );
        assert!(results[0].payload.invariant_holds());
        assert_eq!(results[0].payload.status, CommandStatusRef::accepted());
        assert_eq!(
            results[1].payload.reason_code,
            Some(RejectReasonCode::DuplicateCommandId)
        );
    }

    /// I-16 / AC-8(d) — 종료 스윕.
    #[test]
    fn shutdown_closes_every_open_session_once() {
        let mut sim = Simulation::new(world(), 0);
        let mut ids = SeqIds(0);
        let sessions: Vec<UuidV7> = (0..3).map(|n| id(9000 + n)).collect();
        let opens = sessions
            .iter()
            .enumerate()
            .map(|(i, session_id)| Submission::OpenSession {
                seq: i as u64,
                session_id: *session_id,
                actor_id: id(7000 + i as u64),
            })
            .collect();
        let opened = sim.step(opens, &mut ids);
        assert_eq!(opened.events.len(), 3);

        let outcome = sim.shutdown(&mut ids);
        assert_eq!(outcome.events.len(), 3);
        assert_eq!(outcome.closed_sessions.len(), 3);
        assert_eq!(sim.session_count(), 0);

        // 같은 correlation 으로 짝이 맞는다 (I-16).
        for opened_event in &opened.events {
            let matched = outcome
                .events
                .iter()
                .filter(|closed| closed.correlation_id == opened_event.correlation_id)
                .count();
            assert_eq!(matched, 1);
        }
        // sequence 는 0..n-1 로 빈틈없다 (I-18).
        let sequences: Vec<u64> = outcome.events.iter().map(|e| e.sequence.get()).collect();
        assert_eq!(sequences, vec![0, 1, 2]);
    }

    /// I-17 — tick 은 정확히 1씩 증가하고 건너뛰지 않는다.
    #[test]
    fn tick_advances_by_exactly_one_from_start_tick() {
        let mut sim = Simulation::new(world(), 4_242);
        let mut ids = SeqIds(0);
        for expected in 4_242..4_252u64 {
            let outcome = sim.step(Vec::new(), &mut ids);
            assert_eq!(outcome.tick, expected);
        }
        assert_eq!(sim.next_tick(), 4_252);
    }

    /// 같은 입력 + 같은 id 생성기 = 같은 출력 (원칙 9).
    #[test]
    fn step_is_deterministic_for_the_same_inputs() {
        fn run() -> Vec<String> {
            let mut sim = Simulation::new(world(), 77);
            let mut ids = SeqIds(0);
            let session = id(9001);
            let mut lines = Vec::new();
            for outcome in [
                sim.step(
                    vec![Submission::OpenSession {
                        seq: 0,
                        session_id: session,
                        actor_id: id(7001),
                    }],
                    &mut ids,
                ),
                sim.step(
                    vec![
                        Submission::Command {
                            seq: 3,
                            session_id: session,
                            command: ping(id(31), 3),
                        },
                        Submission::Command {
                            seq: 1,
                            session_id: session,
                            command: ping(id(11), 1),
                        },
                    ],
                    &mut ids,
                ),
                sim.shutdown(&mut ids),
            ] {
                for event in &outcome.events {
                    lines.push(format!(
                        "E {} {} {} {}",
                        outcome.tick,
                        event.sequence.get(),
                        event.body.event_type(),
                        event.occurred_at
                    ));
                }
                for out in &outcome.outbound {
                    lines.push(format!("M {} {}", outcome.tick, out.message.type_name()));
                }
            }
            lines
        }
        assert_eq!(run(), run());
    }

    /// 제출 순번이 뒤섞여 들어와도 순번 순서로 처리된다 (ADR-0006 §4).
    #[test]
    fn submissions_are_ordered_by_submission_seq() {
        let mut sim = Simulation::new(world(), 0);
        let mut ids = SeqIds(0);
        let session = id(9001);
        sim.step(
            vec![Submission::OpenSession {
                seq: 0,
                session_id: session,
                actor_id: id(7001),
            }],
            &mut ids,
        );
        let outcome = sim.step(
            vec![
                Submission::Command {
                    seq: 9,
                    session_id: session,
                    command: ping(id(99), 9),
                },
                Submission::Command {
                    seq: 2,
                    session_id: session,
                    command: ping(id(22), 2),
                },
            ],
            &mut ids,
        );
        let seqs: Vec<u32> = outcome
            .outbound
            .iter()
            .filter_map(|out| match &out.message {
                ServerMessage::PingReply(message) => Some(message.payload.probe_seq.0),
                _ => None,
            })
            .collect();
        assert_eq!(seqs, vec![2, 9]);
    }

    /// 테스트에서 `CommandStatus` 를 직접 쓰기 위한 얇은 도우미.
    struct CommandStatusRef;
    impl CommandStatusRef {
        const fn accepted() -> starfall_contracts::CommandStatus {
            starfall_contracts::CommandStatus::Accepted
        }
    }
}

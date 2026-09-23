//! tick 1회 = [`Simulation::step`] 1회.
//!
//! ADR-0006 §4 의 7단계 중 이 슬라이스에 존재하는 것은 1·2·3·4·7 이다(p1-01 부터 3 "상태
//! 전이"가 생긴다 — 함선 물리·스폰·잔류·재개·디스폰). 5(역사 판정)는 p2, 6(영속화)은
//! 반환값을 받은 쪽이 한다. **없는 단계를 빈 함수로 만들어 두지 않는다**(스펙 §4).
//!
//! # 한 tick 안의 순서 (p1-01 이 더한 것)
//!
//! 1. 제출 처리(기존): 세션 열기(스폰/재개 판정 포함) → 명령(입력 후보 판정 포함) → 세션
//!    닫기(잔류 시작).
//! 2. **물리 적분**: 모든 함선(활성 + 잔류) 각각 정확히 1회, ADR-0010 §2 12단계.
//! 3. **잔류 만료 판정**: 창을 넘은 잔류 함선을 디스폰한다.
//! 4. **스냅샷 조립**: `snapshot_interval_ticks` 배수 tick에서만, 열려 있는 세션마다
//!    `WORLD_SNAPSHOT` 구조체 하나. **직렬화는 여기서 하지 않는다**(ADR-0011 §3) — 문자열로
//!    바꾸는 것은 게이트웨이 송신 태스크의 몫이다.

use std::collections::BTreeMap;

use starfall_contracts::events::{
    DespawnReason, SessionCloseReason, SessionOpenedPayload, SessionTransport,
    ShipDespawnedPayload, ShipSpawnedPayload,
};
use starfall_contracts::messages::{
    CommandResultMessage, CommandResultPayload, CommandResultType, PingReplyMessage,
    PingReplyPayload, PingReplyType, RejectReasonCode, SessionReadyMessage, SessionReadyPayload,
    ShipPresence, ShipState, WorldSnapshotMessage, WorldSnapshotPayload, WorldSnapshotType,
};
use starfall_contracts::primitives::{
    AngularVelocityMdegPerSecond, ConstSchemaVersion, DataId, GameCalendar, GameTime, PositionMm,
    QuaternionComponentMicro, Sequence, ServerVersion, Tick, TickHz, UuidV7, VelocityMmPerSecond,
};
use starfall_contracts::{
    PingServerCommand, SetShipControlCommand, SetShipControlPayload, registry,
};

use crate::entities::ShipEntity;
use crate::session::{SessionSnapshot, SessionState, ShipControlAdmission};
use crate::world::integrate::{self, BoundaryConstants, ControlInput, ShipClassConstants};
use crate::world::quantise::quantise;
use crate::world::{
    Quat, ShipPhysicsState, SpawnParams, Vec3, choose_spawn_point, facing_toward_origin,
};

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

/// 함선 클래스 하나의 적분 상수 + 스폰·지표에 쓰는 부가 값.
#[derive(Debug, Clone, Copy)]
pub struct ShipClassData {
    /// 적분기 입력 전부.
    pub movement: ShipClassConstants,
    /// 스폰 여유·최근접 함선 지표에 쓰는 선체 반경.
    pub hull_radius_m: f64,
}

/// 한 월드의 불변 상수 (`worlds` 행 + `data/` 3종에서 온다).
#[derive(Debug, Clone)]
pub struct WorldConstants {
    /// 월드(샤드) id.
    pub world_id: UuidV7,
    /// 게임 달력 상수. 월드 수명 동안 불변 (I-19).
    pub calendar: GameCalendar,
    /// 서버 빌드 버전. `SESSION_READY` 가 실어 보낸다.
    pub server_version: ServerVersion,

    // ---- p1-01: data/world/systems/*.json --------------------------------------------
    /// 이 좌표가 속한 성계.
    pub star_system_id: DataId,
    /// 경계.
    pub boundary: BoundaryConstants,
    /// 스폰 후보 지점.
    pub spawn_points_m: Vec<[f64; 3]>,
    /// 스폰 점유 판정 여유.
    pub spawn_clearance_m: f64,
    /// 스폰 최대 시도 수.
    pub spawn_max_probe_attempts: u32,
    /// 전부 점유일 때 미는 간격.
    pub spawn_radial_offset_step_m: f64,
    /// 이 월드의 스폰 해시 시드(ADR-0010 §4 의 `world_seed_le8`). `world_id` 의 하위
    /// 8바이트에서 유도한다 — designer 데이터에 별도의 시드 필드가 없고, `world_id` 가
    /// 월드마다 고정·고유하므로 이 값도 그렇다(judgement call, `03_server_impl.md` 기록).
    pub spawn_world_seed: u64,
    /// 조종사 없는 함선이 세계에 남는 시간(초).
    pub linger_seconds: f64,
    /// 재개 가능 창(초). `<= linger_seconds`(S2 검산 완료).
    pub reconnect_resume_window_seconds: f64,

    // ---- p1-01: data/ships/*.json (이 슬라이스는 한 종류뿐이다) -------------------------
    /// 새로 스폰하는 함선이 받는 클래스. **이 슬라이스는 함선 클래스가 하나뿐이라 선택
    /// 규칙이 없다** — 여러 클래스가 생기면 배정 규칙을 architect 와 정해야 한다
    /// (judgement call).
    pub ship_class_id: DataId,
    /// 그 클래스의 적분 상수.
    pub ship_class: ShipClassData,

    // ---- p1-01: data/movement/sync-tuning.json ----------------------------------------
    /// `tick_hz / snapshot_hz` — 기동 시 유도됨(S2).
    pub snapshot_interval_ticks: u32,
    /// 입력 0건일 때 직전 입력을 이월하는 최대 tick 수.
    pub carry_forward_max_ticks: u32,
    /// 세션당 tick당 `SET_SHIP_CONTROL` 처리 상한(`rate_limit_hz` 에서 유도, RATE_LIMITED
    /// 판정 기준). **유도 방식은 judgement call**: `rate_limit_hz.div_ceil(tick_hz)` —
    /// sim 에 시계가 없어 "초당" 을 직접 셀 수 없으므로 tick당 처리 개수로 근사한다
    /// (`03_server_impl.md` 기록).
    pub rate_limit_per_tick_cap: u32,
    /// 스냅샷 `ships` 배열의 계약 상한(`WORLD_SNAPSHOT.schema.json` 의 `maxItems: 64`,
    /// `sync-tuning.json` 의 `max_entities_per_snapshot` 과 같은 값이어야 한다 — S2 가
    /// 이미 `1..=64` 로 검산했다).
    pub max_entities_per_snapshot: usize,
}

impl WorldConstants {
    fn tick_hz_f64(&self) -> f64 {
        f64::from(self.calendar.tick_hz())
    }

    fn dt(&self) -> f64 {
        1.0 / self.tick_hz_f64()
    }
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
    /// `SET_SHIP_CONTROL`.
    SetShipControl(SetShipControlCommand),
}

impl InboundCommand {
    /// 멱등 키 (I-11).
    #[must_use]
    pub const fn command_id(&self) -> UuidV7 {
        match self {
            Self::PingServer(command) => command.command_id,
            Self::SetShipControl(command) => command.command_id,
        }
    }

    /// 레지스트리 타입 이름.
    #[must_use]
    pub const fn type_name(&self) -> &'static str {
        match self {
            Self::PingServer(_) => registry::PING_SERVER,
            Self::SetShipControl(_) => registry::SET_SHIP_CONTROL,
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
    /// 그 tick의 월드 상태 — **아직 문자열이 아니다**(ADR-0011 §3, S5 가 직렬화한다).
    WorldSnapshot(Box<WorldSnapshotMessage>),
}

impl ServerMessage {
    /// 레지스트리 타입 이름. 메트릭 라벨로 쓴다.
    #[must_use]
    pub const fn type_name(&self) -> &'static str {
        match self {
            Self::SessionReady(_) => registry::SESSION_READY,
            Self::CommandResult(_) => registry::COMMAND_RESULT,
            Self::PingReply(_) => registry::PING_REPLY,
            Self::WorldSnapshot(_) => registry::WORLD_SNAPSHOT,
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
    SessionClosed(starfall_contracts::events::SessionClosedPayload),
    /// `SHIP_SPAWNED`.
    ShipSpawned(ShipSpawnedPayload),
    /// `SHIP_DESPAWNED`.
    ShipDespawned(ShipDespawnedPayload),
}

impl DomainEventBody {
    /// 레지스트리 타입 이름.
    #[must_use]
    pub const fn event_type(&self) -> &'static str {
        match self {
            Self::SessionOpened(_) => registry::SESSION_OPENED,
            Self::SessionClosed(_) => registry::SESSION_CLOSED,
            Self::ShipSpawned(_) => registry::SHIP_SPAWNED,
            Self::ShipDespawned(_) => registry::SHIP_DESPAWNED,
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
    /// 이 슬라이스에서 `SESSION_*` 은 `None`, `SHIP_*` 는 항상 `Some`(I-30).
    pub causation_id: Option<UuidV7>,
    /// 행위자 (`SESSION_*`·`SHIP_*` 전부 비-null).
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
    /// 한 tick에 2건 이상 도착해 덮어써진 `SET_SHIP_CONTROL` 수(`input_superseded_total`).
    pub input_superseded: u64,
    /// 입력이 0건이라 직전 입력을 이월한 함선-tick 수(`input_carried_forward_total`).
    pub input_carried_forward: u64,
    /// 목표 자세가 퇴화해 현재 자세로 대체된 횟수(`aim_degenerate_total`).
    pub aim_degenerate: u64,
    /// 이 tick이 끝난 시점의 활성 함선 수(`ships_active` 게이지 — I-25, 카운터 뺄셈이
    /// 아니라 실제 길이다).
    pub ships_active: u64,
    /// 이 tick이 끝난 시점의 잔류 함선 수(`ships_lingering` 게이지).
    pub ships_lingering: u64,
    /// 스냅샷의 `ships` 배열이 `max_entities_per_snapshot` 을 넘은 tick 수(0이어야 정상 —
    /// 넘으면 서버 버그다). `starfall-sim` 에는 로깅이 없으므로(IO 금지) 게이트웨이가 이
    /// 값을 보고 `ERROR` 로그와 `snapshot_over_capacity_total` 을 남긴다(architect 결정).
    pub snapshot_over_capacity: u64,
    /// 잔류 원인 없이 디스폰 경로에 도달해 `SHIP_DESPAWNED` 를 **쓰지 않은** 함선의
    /// `ship_id` (R4 S-3, architect 1.6-1(c)/1.6-3) — 있어서는 안 된다(디버그 빌드는 이
    /// 경로에서 패닉한다). `starfall-sim` 에는 로깅이 없으므로 게이트웨이가 이 값을 보고
    /// `ERROR` 로그를 남긴다. **`/debug/stats` 키는 더하지 않는다**(사안 6과 같은 이유 —
    /// 다음 키 변경과 묶어 미룬다).
    pub causeless_despawns: Vec<UuidV7>,
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
    /// `ship_id` 순 정렬 — 스냅샷의 `ships` 배열이 이 순서를 그대로 물려받는다(I-37).
    ships: BTreeMap<UuidV7, ShipEntity>,
    /// `actor_id -> ship_id`. 한 actor 는 월드에 최대 1척(I-29).
    actor_ship: BTreeMap<UuidV7, UuidV7>,
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
            ships: BTreeMap::new(),
            actor_ship: BTreeMap::new(),
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

    /// 지금 세계에 있는 함선 수(활성 + 잔류). 게이트웨이가 `world_full` 503 판정에 쓴다
    /// (architect 결정 — 입장에서 막는다, 스폰을 거부하지 않는다).
    #[must_use]
    pub fn ship_count(&self) -> usize {
        self.ships.len()
    }

    /// 지금 함선을 가진(활성이든 잔류든) actor 전부, `actor_id` 오름차순.
    ///
    /// 게이트웨이의 `world_full` 게이트가 재개를 면제하는 데 쓴다(I-44, S9) — tick
    /// 드라이버가 매 tick 이 값을 읽어 게이트웨이의 읽기 전용 구조에 채워 넣는다.
    /// 여기서 읽기만 하고 아무것도 바꾸지 않으므로 I-13 위반이 아니다.
    pub fn actors_with_ships(&self) -> impl Iterator<Item = UuidV7> + '_ {
        self.actor_ship.keys().copied()
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
                } => self.handle_command(session_id, command, tick_number, tick, ids, &mut outcome),
                Submission::CloseSession {
                    session_id, reason, ..
                } => self.close_session(session_id, reason, tick, &occurred_at, ids, &mut outcome),
            }
        }

        self.integrate_ships_for_tick(tick_number, &mut outcome);
        self.expire_lingering_ships(tick_number, tick, &occurred_at, ids, &mut outcome);
        self.record_presence_gauges(&mut outcome);
        self.build_snapshots(tick, tick_number, ids, &mut outcome);

        #[cfg(debug_assertions)]
        self.debug_assert_i29(&outcome);

        outcome
    }

    /// 살아 있는 모든 세션을 `SERVER_SHUTDOWN` 으로 닫는 마지막 tick (I-16, AC-8d).
    ///
    /// 정상 종료는 **활성이든 잔류든 세계에 있는 함선을 전부** 디스폰한다(I-41) — 그래야
    /// "모든 `SHIP_SPAWNED` 가 짝을 갖는다"가 프로세스 경계에서도 성립한다. 활성 함선은
    /// 먼저 `SESSION_CLOSED` 를 발행해 그 `event_id` 를 자신의 `causation_id` 로 쓴다
    /// (순서: `SESSION_CLOSED` → `SHIP_DESPAWNED`, ADR-0011 §6).
    pub fn shutdown(&mut self, ids: &mut dyn IdSource) -> TickOutcome {
        let tick_number = self.next_tick;
        self.next_tick = self.next_tick.saturating_add(1);
        let mut outcome = TickOutcome {
            tick: tick_number,
            ..TickOutcome::default()
        };
        let Some(tick) = Tick::new(tick_number) else {
            return outcome;
        };
        let Some(occurred_at) = self.world.calendar.occurred_at(tick) else {
            return outcome;
        };

        let open_sessions: Vec<UuidV7> = self.sessions.keys().copied().collect();
        for session_id in open_sessions {
            let Some(session) = self.sessions.remove(&session_id) else {
                continue;
            };
            let event_id = ids.next_id();
            outcome.events.push(PendingEvent {
                event_id,
                world_id: self.world.world_id,
                tick,
                sequence: next_sequence(&outcome),
                occurred_at: occurred_at.clone(),
                correlation_id: session.correlation_id,
                causation_id: None,
                actor_id: session.actor_id,
                body: DomainEventBody::SessionClosed(
                    starfall_contracts::events::SessionClosedPayload {
                        session_id,
                        close_reason: SessionCloseReason::ServerShutdown,
                    },
                ),
            });
            outcome.closed_sessions.push(session_id);

            // R4 S-2 (architect 1.6-1(b)(d)): `actor_ship` 이 아니라 세션 자신의 기록으로
            // 찾는다 — 스윕이 세션에서 출발하므로, 모든 ACTIVE 함선이 이 루프에서 빠짐없이
            // 잔류로 전이된다(빠지는 함선이 없어야 한다는 요구를 이 방식으로 만족한다).
            if let Some(ship_id) = session.controlling_ship
                && let Some(ship) = self.ships.get_mut(&ship_id)
                && ship.controlling_session == Some(session_id)
            {
                ship.start_lingering(tick_number, event_id, session.correlation_id);
            }
        }

        // 종료 스윕 직전에 한 번 더 물리를 돌리지 않는다 — 종료는 시뮬레이션을 전진시키는
        // 것이 아니라 존재를 마치는 절차다. 남은 함선을 전부 디스폰한다.
        let ship_ids: Vec<UuidV7> = self.ships.keys().copied().collect();
        for ship_id in ship_ids {
            self.despawn_ship(
                ship_id,
                DespawnReason::ServerShutdown,
                tick,
                &occurred_at,
                ids,
                &mut outcome,
            );
        }

        self.record_presence_gauges(&mut outcome);

        #[cfg(debug_assertions)]
        self.debug_assert_i29(&outcome);

        outcome
    }

    /// I-29 1:1 불변식을 확인한다(R4 S-3, architect 1.6-1·1.9). `step`/`shutdown` 끝에서
    /// 호출한다 — 디버그 빌드(= `cargo test` 포함)에서만 컴파일되므로 sim 의 **모든**
    /// 테스트가 매 호출 뒤 이 검사를 공짜로 통과한다. 릴리스에는 이 함수 자체가 없다.
    #[cfg(debug_assertions)]
    fn debug_assert_i29(&self, outcome: &TickOutcome) {
        let mut violations = Vec::new();

        // actor 당 함선은 최대 1척이다.
        let mut seen_actors = std::collections::BTreeSet::new();
        for ship in self.ships.values() {
            if !seen_actors.insert(ship.actor_id) {
                violations.push(format!("actor {} 가 함선을 2척 이상 가졌다", ship.actor_id));
            }
        }

        // ACTIVE 함선은 고아가 아니다. LINGERING 함선은 조종 세션이 없고 원인이 있다.
        let mut claimed_sessions = std::collections::BTreeSet::new();
        for ship in self.ships.values() {
            match ship.presence {
                ShipPresence::Active => match ship.controlling_session {
                    None => violations.push(format!(
                        "함선 {} 이 ACTIVE 인데 조종 세션이 없다",
                        ship.ship_id
                    )),
                    Some(sid) if !self.sessions.contains_key(&sid) => violations.push(format!(
                        "함선 {} 의 조종 세션 {sid} 이 이미 닫혀 있다 — 고아 ACTIVE",
                        ship.ship_id
                    )),
                    Some(sid) => {
                        if !claimed_sessions.insert(sid) {
                            violations.push(format!("세션 {sid} 이 함선을 2척 이상 조종한다"));
                        }
                    }
                },
                ShipPresence::Lingering => {
                    if ship.controlling_session.is_some() {
                        violations.push(format!(
                            "함선 {} 이 LINGERING 인데 조종 세션이 남아 있다",
                            ship.ship_id
                        ));
                    }
                    if ship.linger_cause_event_id.is_none() {
                        violations.push(format!(
                            "함선 {} 이 LINGERING 인데 잔류 원인이 없다",
                            ship.ship_id
                        ));
                    }
                }
            }
        }

        // 세션 자신의 기록(`controlling_ship`)도 함선 쪽과 되짚어져야 한다.
        for (sid, session) in &self.sessions {
            if let Some(ship_id) = session.controlling_ship
                && !matches!(
                    self.ships.get(&ship_id),
                    Some(ship) if ship.controlling_session == Some(*sid)
                )
            {
                violations.push(format!(
                    "세션 {sid} 이 함선 {ship_id} 을 조종한다고 기록했지만 되짚어지지 않는다"
                ));
            }
        }

        // 이번 tick 이 발행한 이벤트 중 자기 참조가 없어야 한다(I-30).
        for event in &outcome.events {
            if event.causation_id == Some(event.event_id) {
                violations.push(format!(
                    "이벤트 {} 가 자기 자신을 원인으로 가리킨다",
                    event.event_id
                ));
            }
        }

        assert!(
            violations.is_empty(),
            "I-29/I-30 불변식 위반(tick {}): {violations:?}",
            outcome.tick
        );
    }

    // -----------------------------------------------------------------------------------
    // 세션 열기 — 스폰 또는 재개 (ADR-0011 §6)
    // -----------------------------------------------------------------------------------

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

        let session_opened_event_id = ids.next_id();
        outcome.events.push(PendingEvent {
            event_id: session_opened_event_id,
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
        let Some(tick_hz) = TickHz::new(self.world.calendar.tick_hz()) else {
            return;
        };
        outcome.outbound.push(Outbound {
            session_id,
            message: ServerMessage::SessionReady(Box::new(SessionReadyMessage {
                message_id: ids.next_id(),
                message_type: starfall_contracts::messages::SessionReadyType::SessionReady,
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

        // I-29 (R4 S-2, architect 1.6-1(a)): 이 actor 의 기존 함선 상태로 세 경우를 가른다.
        // **경우 2 — 잔류 중**: 재개한다. 새 SHIP_SPAWNED 없음.
        if let Some(&ship_id) = self.actor_ship.get(&actor_id)
            && let Some(ship) = self.ships.get_mut(&ship_id)
            && ship.presence == ShipPresence::Lingering
        {
            ship.activate(session_id);
            ship.discard_carried_input_for_resume();
            if let Some(session) = self.sessions.get_mut(&session_id) {
                session.controlling_ship = Some(ship_id);
            }
            return;
        }

        // **경우 3 — 이미 ACTIVE 인 함선이 있다**(동시 접속, QA R3 §6.7 재현). 사용자
        // 결정 5 (B) "나중 접속이 이어받는다" — ADR-0011 §6.3, 계약에 `SUPERSEDED` 가
        // 들어온 뒤 S-6 이 실제 인수인계를 넣는다. 옛 세션(S1)을 `SUPERSEDED` 로 닫고
        // **같은 tick에** 조종을 이 세션(S2)으로 옮긴다(재개와 같은 규칙 — 물리 유지,
        // `last_applied_input_seq = None`, 이월 입력 폐기). 잔류는 거치지 않고 함선
        // 이벤트도 없다. 기록 순서: `SESSION_OPENED(S2)`(이미 위에서 발행) →
        // `SESSION_CLOSED(S1, SUPERSEDED)`(여기서 발행, seq 가 더 크다).
        if let Some(&ship_id) = self.actor_ship.get(&actor_id) {
            // 잔류(경우 2)는 위에서 이미 반환했으므로 여기 도달했다는 것은 ACTIVE 라는
            // 뜻이다(I-29 — 세 번째 상태는 없다). `old_session_id` 는 세션 자신이 아니라
            // 함선의 기록으로 찾는다 — `close_session`(S-2)과 같은 규율이다.
            let old_session_id = self
                .ships
                .get(&ship_id)
                .and_then(|ship| ship.controlling_session);
            if let Some(old_session_id) = old_session_id
                && let Some(old_session) = self.sessions.remove(&old_session_id)
            {
                outcome.events.push(PendingEvent {
                    event_id: ids.next_id(),
                    world_id: self.world.world_id,
                    tick,
                    // correlation_id 는 S1(옛 세션)의 것이다 — 여는 이벤트와 짝짓는 키
                    // (p0-02 규칙 그대로, ADR-0011 §6.3).
                    sequence: next_sequence(outcome),
                    occurred_at: occurred_at.clone(),
                    correlation_id: old_session.correlation_id,
                    // SUPERSEDED 만 causation_id 가 비-null 이다 — 넘겨받은
                    // SESSION_OPENED(S2) 의 event_id(ADR-0011 §6.3).
                    causation_id: Some(session_opened_event_id),
                    actor_id: old_session.actor_id,
                    body: DomainEventBody::SessionClosed(
                        starfall_contracts::events::SessionClosedPayload {
                            session_id: old_session_id,
                            close_reason: SessionCloseReason::Superseded,
                        },
                    ),
                });
                outcome.closed_sessions.push(old_session_id);

                if let Some(ship) = self.ships.get_mut(&ship_id) {
                    ship.activate(session_id);
                    ship.discard_carried_input_for_resume();
                }
                if let Some(session) = self.sessions.get_mut(&session_id) {
                    session.controlling_ship = Some(ship_id);
                }
            }
            // 위 조건이 전부 성립하지 않아도(있을 수 없다 — I-29, S-3이 매 tick 검사한다)
            // 두 번째 함선만은 만들지 않는다: `actor_ship` 을 덮어쓰지 않고 반환한다
            // (그 덮어쓰기가 R3 §6.7 의 원인 1이었다).
            return;
        }

        // **경우 1 — 기존 함선 없음**: 스폰한다. 기존 활성·잔류 함선 위치를 점유 판정에
        // 쓴다(자기 자신은 아직 없다).
        let existing_positions: Vec<Vec3> = self.ships.values().map(|s| s.physics.p).collect();
        let params = SpawnParams {
            world_seed: self.world.spawn_world_seed,
            points_m: &self.world.spawn_points_m,
            clearance_m: self.world.spawn_clearance_m,
            hull_radius_m: self.world.ship_class.hull_radius_m,
            max_probe_attempts: self.world.spawn_max_probe_attempts,
            radial_offset_step_m: self.world.spawn_radial_offset_step_m,
        };
        let position =
            choose_spawn_point(actor_id.get().into_bytes(), &params, &existing_positions);
        let orientation = facing_toward_origin(position);
        let ship_id = ids.next_id();

        self.ships.insert(
            ship_id,
            ShipEntity {
                ship_id,
                actor_id,
                ship_class_id: self.world.ship_class_id.clone(),
                presence: ShipPresence::Active,
                physics: ShipPhysicsState::at_rest(position, orientation),
                controlling_session: Some(session_id),
                last_session_id: session_id,
                last_real_input: None,
                ticks_since_real_input: 0,
                linger_started_tick: None,
                linger_cause_event_id: None,
                linger_cause_correlation_id: None,
            },
        );
        self.actor_ship.insert(actor_id, ship_id);
        if let Some(session) = self.sessions.get_mut(&session_id) {
            session.controlling_ship = Some(ship_id);
        }

        outcome.events.push(PendingEvent {
            event_id: ids.next_id(),
            world_id: self.world.world_id,
            tick,
            sequence: next_sequence(outcome),
            occurred_at: occurred_at.clone(),
            correlation_id,
            causation_id: Some(session_opened_event_id),
            actor_id,
            body: DomainEventBody::ShipSpawned(ShipSpawnedPayload {
                ship_id,
                session_id,
                ship_class_id: self.world.ship_class_id.clone(),
                star_system_id: self.world.star_system_id.clone(),
                position_x_mm: quantised_position(position.x),
                position_y_mm: quantised_position(position.y),
                position_z_mm: quantised_position(position.z),
                orientation_x_micro: quantised_quat(orientation.x),
                orientation_y_micro: quantised_quat(orientation.y),
                orientation_z_micro: quantised_quat(orientation.z),
                orientation_w_micro: quantised_quat(orientation.w),
            }),
        });
    }

    // -----------------------------------------------------------------------------------
    // 명령 처리
    // -----------------------------------------------------------------------------------

    fn handle_command(
        &mut self,
        session_id: UuidV7,
        command: InboundCommand,
        tick_number: u64,
        tick: Tick,
        ids: &mut dyn IdSource,
        outcome: &mut TickOutcome,
    ) {
        // 세션이 이미 닫혔으면 답할 곳이 없다.
        let Some(session) = self.sessions.get_mut(&session_id) else {
            return;
        };

        let command_id = command.command_id();
        let accepted = session.remember(command_id);

        if !accepted {
            outcome.outbound.push(Outbound {
                session_id,
                message: ServerMessage::CommandResult(Box::new(CommandResultMessage {
                    message_id: ids.next_id(),
                    message_type: CommandResultType::CommandResult,
                    schema_version: ConstSchemaVersion,
                    tick,
                    correlation_id: None,
                    payload: CommandResultPayload::rejected(
                        command_id,
                        RejectReasonCode::DuplicateCommandId,
                    ),
                })),
            });
            return;
        }

        match command {
            InboundCommand::PingServer(ping) => {
                outcome.outbound.push(Outbound {
                    session_id,
                    message: ServerMessage::CommandResult(Box::new(CommandResultMessage {
                        message_id: ids.next_id(),
                        message_type: CommandResultType::CommandResult,
                        schema_version: ConstSchemaVersion,
                        tick,
                        correlation_id: None,
                        payload: CommandResultPayload::accepted(command_id),
                    })),
                });
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
            InboundCommand::SetShipControl(cmd) => {
                let per_tick_cap = self.world.rate_limit_per_tick_cap;
                let admission = session.step_ship_control(tick_number, cmd.payload, per_tick_cap);
                let payload = match admission {
                    ShipControlAdmission::Stale => {
                        CommandResultPayload::rejected(command_id, RejectReasonCode::StaleInput)
                    }
                    ShipControlAdmission::RateLimited => {
                        CommandResultPayload::rejected(command_id, RejectReasonCode::RateLimited)
                    }
                    ShipControlAdmission::Accepted {
                        superseded_previous,
                    } => {
                        if superseded_previous {
                            outcome.input_superseded += 1;
                        }
                        CommandResultPayload::accepted(command_id)
                    }
                };
                outcome.outbound.push(Outbound {
                    session_id,
                    message: ServerMessage::CommandResult(Box::new(CommandResultMessage {
                        message_id: ids.next_id(),
                        message_type: CommandResultType::CommandResult,
                        schema_version: ConstSchemaVersion,
                        tick,
                        correlation_id: None,
                        payload,
                    })),
                });
            }
        }
    }

    // -----------------------------------------------------------------------------------
    // 세션 닫기 — 잔류 시작 (ADR-0011 §6)
    // -----------------------------------------------------------------------------------

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

        let event_id = ids.next_id();
        outcome.events.push(PendingEvent {
            event_id,
            world_id: self.world.world_id,
            tick,
            sequence: next_sequence(outcome),
            occurred_at: occurred_at.clone(),
            correlation_id: session.correlation_id,
            causation_id: None,
            actor_id: session.actor_id,
            body: DomainEventBody::SessionClosed(
                starfall_contracts::events::SessionClosedPayload {
                    session_id,
                    close_reason: reason,
                },
            ),
        });
        outcome.closed_sessions.push(session_id);

        // R4 S-2 (architect 1.6-1(b)): 이 세션이 조종하던 함선은 세션 자신의 기록
        // (`controlling_ship`)으로 찾는다 — `actor_ship` 표를 거치지 않는다. 동시 접속이
        // 있었다면 그 표는 **다른** 세션이 조종하는 함선을 가리킬 수 있다.
        if let Some(ship_id) = session.controlling_ship
            && let Some(ship) = self.ships.get_mut(&ship_id)
            && ship.controlling_session == Some(session_id)
        {
            ship.start_lingering(tick.get(), event_id, session.correlation_id);
        }
    }

    // -----------------------------------------------------------------------------------
    // 물리 적분 — 모든 함선, 매 tick 정확히 1회 (ADR-0010 §2)
    // -----------------------------------------------------------------------------------

    fn integrate_ships_for_tick(&mut self, tick_number: u64, outcome: &mut TickOutcome) {
        let dt = self.world.dt();
        let boundary = self.world.boundary;
        let class = self.world.ship_class.movement;
        let carry_forward_max_ticks = self.world.carry_forward_max_ticks;
        let sessions = &self.sessions;

        for ship in self.ships.values_mut() {
            let winning = ship
                .controlling_session
                .and_then(|session_id| sessions.get(&session_id))
                .and_then(|session| session.winning_input_for_tick(tick_number));

            let control = if let Some(payload) = winning {
                let control = payload_to_control_input(payload);
                ship.last_real_input = Some(*payload);
                ship.ticks_since_real_input = 0;
                control
            } else if let Some(carried) = ship.last_real_input {
                if ship.ticks_since_real_input < carry_forward_max_ticks {
                    ship.ticks_since_real_input += 1;
                    outcome.input_carried_forward += 1;
                    payload_to_control_input(&carried)
                } else {
                    ControlInput::dormant(ship.physics.q)
                }
            } else {
                ControlInput::dormant(ship.physics.q)
            };

            let result = integrate::step(ship.physics, &control, &class, &boundary, dt);
            ship.physics = result.state;
            if result.aim_degenerate {
                outcome.aim_degenerate += 1;
            }
        }
    }

    // -----------------------------------------------------------------------------------
    // 잔류 만료 → 디스폰
    // -----------------------------------------------------------------------------------

    fn expire_lingering_ships(
        &mut self,
        tick_number: u64,
        tick: Tick,
        occurred_at: &GameTime,
        ids: &mut dyn IdSource,
        outcome: &mut TickOutcome,
    ) {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let linger_ticks = (self.world.linger_seconds * self.world.tick_hz_f64()).round() as u64;

        let expired: Vec<UuidV7> = self
            .ships
            .iter()
            .filter(|(_, ship)| ship.presence == ShipPresence::Lingering)
            .filter(|(_, ship)| {
                ship.linger_started_tick
                    .is_some_and(|start| tick_number.saturating_sub(start) >= linger_ticks)
            })
            .map(|(id, _)| *id)
            .collect();

        for ship_id in expired {
            self.despawn_ship(
                ship_id,
                DespawnReason::LingerExpired,
                tick,
                occurred_at,
                ids,
                outcome,
            );
        }
    }

    /// 함선 하나를 디스폰한다. `causation_id`/`correlation_id` 는 그 잔류를 시작시킨
    /// `SESSION_CLOSED` 에서 가져온다(`ShipEntity::start_lingering` 가 저장해 둔 값).
    ///
    /// # 원인이 없으면 (R4 S-3, architect 1.6-1(c)/1.6-3)
    ///
    /// S-2 이후 구조적으로 도달 불가능해야 한다 — 모든 `ACTIVE` 함선은 디스폰 전에 반드시
    /// `start_lingering` 을 거친다(조종 세션이 먼저 닫힌다, `close_session`/`shutdown`).
    /// 그래도 도달했다면 **그럴듯한 원인을 지어내지 않는다**(옛 `unwrap_or(event_id)`
    /// 자기 참조는 I-30 위반이었다 — QA R3 §6.7 이 실제로 이 경로를 탔다) — 이벤트를
    /// 쓰지 않고 `outcome.causeless_despawns` 에 신호만 남긴다(게이트웨이가 ERROR 로 남긴다,
    /// `/debug/stats` 키는 더하지 않는다 — 사안 6과 같은 이유). 디버그 빌드에서는 그
    /// 자리에서 패닉한다 — sim 의 모든 테스트가 매 `step`/`shutdown` 뒤 이 경로를 도는
    /// 셈이라 회귀를 테스트가 바로 잡는다.
    fn despawn_ship(
        &mut self,
        ship_id: UuidV7,
        reason: DespawnReason,
        tick: Tick,
        occurred_at: &GameTime,
        ids: &mut dyn IdSource,
        outcome: &mut TickOutcome,
    ) {
        let Some(ship) = self.ships.remove(&ship_id) else {
            return;
        };
        self.actor_ship.remove(&ship.actor_id);

        let Some(causation_id) = ship.linger_cause_event_id else {
            outcome.causeless_despawns.push(ship_id);
            debug_assert!(
                false,
                "I-29/I-30 위반: 함선 {ship_id} 이 잔류 원인 없이 디스폰 경로에 도달했다"
            );
            return;
        };
        let correlation_id = ship.linger_cause_correlation_id.unwrap_or(causation_id);
        let event_id = ids.next_id();

        outcome.events.push(PendingEvent {
            event_id,
            world_id: self.world.world_id,
            tick,
            sequence: next_sequence(outcome),
            occurred_at: occurred_at.clone(),
            correlation_id,
            causation_id: Some(causation_id),
            actor_id: ship.actor_id,
            body: DomainEventBody::ShipDespawned(ShipDespawnedPayload {
                ship_id,
                last_session_id: ship.last_session_id,
                despawn_reason: reason,
                position_x_mm: quantised_position(ship.physics.p.x),
                position_y_mm: quantised_position(ship.physics.p.y),
                position_z_mm: quantised_position(ship.physics.p.z),
            }),
        });
    }

    fn record_presence_gauges(&self, outcome: &mut TickOutcome) {
        let (active, lingering) =
            self.ships
                .values()
                .fold((0u64, 0u64), |(active, lingering), ship| {
                    match ship.presence {
                        ShipPresence::Active => (active + 1, lingering),
                        ShipPresence::Lingering => (active, lingering + 1),
                    }
                });
        outcome.ships_active = active;
        outcome.ships_lingering = lingering;
    }

    // -----------------------------------------------------------------------------------
    // 스냅샷 조립 (ADR-0011 §1·§3) — 직렬화는 하지 않는다
    // -----------------------------------------------------------------------------------

    fn build_snapshots(
        &self,
        tick: Tick,
        tick_number: u64,
        ids: &mut dyn IdSource,
        outcome: &mut TickOutcome,
    ) {
        if self.sessions.is_empty() {
            return;
        }
        if !tick_number.is_multiple_of(u64::from(self.world.snapshot_interval_ticks)) {
            return;
        }

        // 모든 수신자가 공유하는 `ships` 배열은 한 번만 만든다(전역 정렬 — SC-28·SC-63 근거).
        let mut ships: Vec<ShipState> = Vec::with_capacity(self.ships.len());
        for ship in self.ships.values() {
            ships.push(ShipState {
                ship_id: ship.ship_id,
                actor_id: ship.actor_id,
                ship_class_id: ship.ship_class_id.clone(),
                presence: ship.presence,
                position_x_mm: quantised_position(ship.physics.p.x),
                position_y_mm: quantised_position(ship.physics.p.y),
                position_z_mm: quantised_position(ship.physics.p.z),
                velocity_x_mm_s: quantised_velocity(ship.physics.v.x),
                velocity_y_mm_s: quantised_velocity(ship.physics.v.y),
                velocity_z_mm_s: quantised_velocity(ship.physics.v.z),
                orientation_x_micro: quantised_quat(ship.physics.q.x),
                orientation_y_micro: quantised_quat(ship.physics.q.y),
                orientation_z_micro: quantised_quat(ship.physics.q.z),
                orientation_w_micro: quantised_quat(ship.physics.q.w),
                angular_velocity_x_mdeg_s: quantised_angular(ship.physics.omega_aim.x),
                angular_velocity_y_mdeg_s: quantised_angular(ship.physics.omega_aim.y),
                angular_velocity_z_mdeg_s: quantised_angular(ship.physics.omega_aim.z),
                angular_velocity_roll_mdeg_s: quantised_angular(ship.physics.omega_roll),
            });
        }
        // 서버 버그 신호다(게이트웨이의 world_full 입장 제한이 뚫렸다는 뜻) — 그래도
        // **자르지 않는다.** 여기서 자르면 "월드에 이 함선이 있다"와 "이 스냅샷에 그
        // 함선이 있다"가 갈라져 더 나쁜 상태가 된다. debug 빌드는 즉시 죽여 잡고, release
        // 는 값을 그대로 보내되 게이트웨이가 로그·카운터로 드러낸다(sim 에는 IO가 없다).
        debug_assert!(
            ships.len() <= self.world.max_entities_per_snapshot,
            "함선 수가 max_entities_per_snapshot 을 넘었다 — 게이트웨이의 world_full 입장 제한이 뚫렸다"
        );
        if ships.len() > self.world.max_entities_per_snapshot {
            outcome.snapshot_over_capacity += 1;
        }

        let soft_boundary_radius_mm = quantise(
            self.world.boundary.soft_boundary_radius_m,
            1000.0,
            1,
            20_000_000,
        );
        let hard_boundary_radius_mm = quantise(
            self.world.boundary.hard_boundary_radius_m,
            1000.0,
            1,
            20_000_000,
        );
        // `snapshot_hz` 의 계약 범위(1..=60)와 `tick_hz`(1..=1000)가 만드는 몫이므로
        // u16 상한(255, WORLD_SNAPSHOT 계약)을 넘을 수 없다 — S2 가 이미 정수 나눗셈을
        // 검산했다.
        #[allow(clippy::cast_possible_truncation)]
        let snapshot_interval_ticks_u16 = self.world.snapshot_interval_ticks as u16;

        for (session_id, session) in &self.sessions {
            let controlled_ship_id = self.actor_ship.get(&session.actor_id).copied();
            let payload = WorldSnapshotPayload {
                star_system_id: self.world.star_system_id.clone(),
                soft_boundary_radius_mm,
                hard_boundary_radius_mm,
                snapshot_interval_ticks: snapshot_interval_ticks_u16,
                controlled_ship_id,
                ack_input_seq: session
                    .snapshot()
                    .last_applied_input_seq
                    .and_then(starfall_contracts::primitives::InputSeq::new),
                ships: ships.clone(),
            };
            outcome.outbound.push(Outbound {
                session_id: *session_id,
                message: ServerMessage::WorldSnapshot(Box::new(WorldSnapshotMessage {
                    message_id: ids.next_id(),
                    message_type: WorldSnapshotType::WorldSnapshot,
                    schema_version: ConstSchemaVersion,
                    tick,
                    correlation_id: None,
                    payload,
                })),
            });
        }
    }
}

/// tick 안의 다음 `sequence`. 0부터 빈틈없이 (I-18).
fn next_sequence(outcome: &TickOutcome) -> Sequence {
    Sequence::new(outcome.events.len() as u64).unwrap_or_else(|| {
        Sequence::new(0).unwrap_or_else(|| unreachable!("0 은 언제나 유효한 Sequence 다"))
    })
}

/// `quantise()`(i64, 이미 `lo..=hi` 로 클램프됨)를 `i32` 로 좁힌다. `lo`/`hi` 가 i32 범위
/// 안이라고 호출자가 보장하므로 이 캐스트는 값을 잃지 않는다 — 그래도 clippy 는 정적으로
/// 알 수 없으므로 여기 한 곳에서만 `allow` 한다.
fn quantise_i32(component: f64, scale: f64, lo: i64, hi: i64) -> i32 {
    let raw = quantise(component, scale, lo, hi);
    #[allow(clippy::cast_possible_truncation)]
    let narrowed = raw as i32;
    narrowed
}

fn quantised_position(component: f64) -> PositionMm {
    PositionMm::saturating(quantise(
        component,
        1000.0,
        -1_000_000_000_000,
        1_000_000_000_000,
    ))
}

fn quantised_velocity(component: f64) -> VelocityMmPerSecond {
    VelocityMmPerSecond::saturating(quantise_i32(component, 1000.0, -100_000_000, 100_000_000))
}

fn quantised_quat(component: f64) -> QuaternionComponentMicro {
    QuaternionComponentMicro::saturating(quantise_i32(
        component,
        1_000_000.0,
        -1_000_000,
        1_000_000,
    ))
}

fn quantised_angular(component_deg_s: f64) -> AngularVelocityMdegPerSecond {
    AngularVelocityMdegPerSecond::saturating(quantise_i32(
        component_deg_s,
        1000.0,
        -3_600_000,
        3_600_000,
    ))
}

fn payload_to_control_input(payload: &SetShipControlPayload) -> ControlInput {
    ControlInput {
        thrust: Vec3::new(
            f64::from(payload.thrust_x_milli.get()) / 1000.0,
            f64::from(payload.thrust_y_milli.get()) / 1000.0,
            f64::from(payload.thrust_z_milli.get()) / 1000.0,
        ),
        roll: f64::from(payload.roll_milli.get()) / 1000.0,
        aim_raw: Quat::new(
            f64::from(payload.aim_x_micro.get()) / 1_000_000.0,
            f64::from(payload.aim_y_micro.get()) / 1_000_000.0,
            f64::from(payload.aim_z_micro.get()) / 1_000_000.0,
            f64::from(payload.aim_w_micro.get()) / 1_000_000.0,
        ),
        brake: payload.brake,
        assist: payload.flight_assist,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use starfall_contracts::primitives::{
        ControlAxisMilli, InputSeq, QuaternionComponentMicro as QCM,
    };

    /// 결정적 UUIDv7 생성 (테스트 전용). 운영의 `Uuid::now_v7()` 을 대신한다.
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

    fn scout_class() -> ShipClassData {
        ShipClassData {
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
        }
    }

    fn world() -> WorldConstants {
        WorldConstants {
            world_id: UuidV7::parse("01a0b1c2-3d4e-7f01-8a2b-9c0d1e2f3a4b").unwrap(),
            calendar: GameCalendar::new(20, GameTime::parse("3800-01-01T00:00:00Z").unwrap(), 60)
                .unwrap(),
            server_version: ServerVersion::parse("0.1.0").unwrap(),
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
            ship_class: scout_class(),
            snapshot_interval_ticks: 2,
            carry_forward_max_ticks: 10,
            rate_limit_per_tick_cap: 2,
            max_entities_per_snapshot: 64,
        }
    }

    fn open(sim: &mut Simulation, ids: &mut SeqIds, session: UuidV7, actor: UuidV7) -> TickOutcome {
        sim.step(
            vec![Submission::OpenSession {
                seq: 0,
                session_id: session,
                actor_id: actor,
            }],
            ids,
        )
    }

    fn ship_control(command_id: UuidV7, input_seq: u32, thrust_z_milli: i32) -> InboundCommand {
        InboundCommand::SetShipControl(SetShipControlCommand {
            command_id,
            command_type: starfall_contracts::commands::SetShipControlType::SetShipControl,
            schema_version: ConstSchemaVersion,
            client_sent_at: None,
            payload: SetShipControlPayload {
                input_seq: InputSeq::new(input_seq).unwrap(),
                thrust_x_milli: ControlAxisMilli::new(0).unwrap(),
                thrust_y_milli: ControlAxisMilli::new(0).unwrap(),
                thrust_z_milli: ControlAxisMilli::new(thrust_z_milli).unwrap(),
                roll_milli: ControlAxisMilli::new(0).unwrap(),
                aim_x_micro: QCM::new(0).unwrap(),
                aim_y_micro: QCM::new(0).unwrap(),
                aim_z_micro: QCM::new(0).unwrap(),
                aim_w_micro: QCM::new(1_000_000).unwrap(),
                brake: false,
                flight_assist: true,
            },
        })
    }

    /// SC-08/09 — 세션을 열면 그 tick에 `SHIP_SPAWNED` 가 `SESSION_OPENED` 다음 순서로,
    /// `causation_id = SESSION_OPENED.event_id` 로 발행된다.
    #[test]
    fn opening_a_session_spawns_a_ship_caused_by_session_opened() {
        let mut sim = Simulation::new(world(), 0);
        let mut ids = SeqIds(0);
        let outcome = open(&mut sim, &mut ids, id(1), id(2));

        assert_eq!(outcome.events.len(), 2);
        let opened = &outcome.events[0];
        let spawned = &outcome.events[1];
        assert_eq!(opened.body.event_type(), "SESSION_OPENED");
        assert_eq!(spawned.body.event_type(), "SHIP_SPAWNED");
        assert_eq!(spawned.causation_id, Some(opened.event_id));
        assert!(opened.sequence.get() < spawned.sequence.get());
        assert_eq!(sim.ship_count(), 1);
    }

    /// SC-10 (일부) — 스폰 위치가 스폰 후보 지점 중 하나와 정확히 일치한다.
    #[test]
    fn spawn_position_matches_a_configured_spawn_point() {
        let mut sim = Simulation::new(world(), 0);
        let mut ids = SeqIds(0);
        let outcome = open(&mut sim, &mut ids, id(1), id(2));
        let DomainEventBody::ShipSpawned(payload) = &outcome.events[1].body else {
            panic!("SHIP_SPAWNED 가 아니다");
        };
        let matches_a_point = world().spawn_points_m.iter().any(|point| {
            quantised_position(point[0]) == payload.position_x_mm
                && quantised_position(point[1]) == payload.position_y_mm
                && quantised_position(point[2]) == payload.position_z_mm
        });
        assert!(matches_a_point, "{payload:?}");
    }

    /// SC-14/AC-3(g) — `ship_id` 별 `SHIP_SPAWNED` 1건 / `SHIP_DESPAWNED` 1건, 잔류 경로.
    #[test]
    fn session_close_lingers_then_expires_into_a_single_despawn() {
        let mut sim = Simulation::new(world(), 0);
        let mut ids = SeqIds(0);
        open(&mut sim, &mut ids, id(1), id(2));

        let close_outcome = sim.step(
            vec![Submission::CloseSession {
                seq: 0,
                session_id: id(1),
                reason: SessionCloseReason::ClientClosed,
            }],
            &mut ids,
        );
        assert_eq!(close_outcome.events.len(), 1);
        assert_eq!(close_outcome.events[0].body.event_type(), "SESSION_CLOSED");
        assert_eq!(sim.ship_count(), 1, "잔류 중이라 함선은 아직 있다");

        // linger_seconds=30, tick_hz=20 => 600 tick. 그 전까지는 디스폰되지 않는다.
        for _ in 0..599 {
            let out = sim.step(Vec::new(), &mut ids);
            assert!(
                !out.events
                    .iter()
                    .any(|e| e.body.event_type() == "SHIP_DESPAWNED"),
                "너무 일찍 디스폰됐다"
            );
        }
        let expiry_outcome = sim.step(Vec::new(), &mut ids);
        let despawn = expiry_outcome
            .events
            .iter()
            .find(|e| e.body.event_type() == "SHIP_DESPAWNED")
            .expect("600 tick 후 디스폰돼야 한다");
        let DomainEventBody::ShipDespawned(payload) = &despawn.body else {
            unreachable!()
        };
        assert_eq!(payload.despawn_reason, DespawnReason::LingerExpired);
        assert_eq!(sim.ship_count(), 0);
    }

    /// SC-11/AC-3(d) — 잔류 창 안에 재접속하면 같은 `ship_id`, 새 `SHIP_SPAWNED` 없음.
    #[test]
    fn reconnect_within_linger_window_resumes_the_same_ship() {
        let mut sim = Simulation::new(world(), 0);
        let mut ids = SeqIds(0);
        let first = open(&mut sim, &mut ids, id(1), id(2));
        let DomainEventBody::ShipSpawned(spawned) = &first.events[1].body else {
            panic!()
        };
        let ship_id = spawned.ship_id;

        sim.step(
            vec![Submission::CloseSession {
                seq: 0,
                session_id: id(1),
                reason: SessionCloseReason::ClientClosed,
            }],
            &mut ids,
        );

        // 재접속 — 같은 actor, 새 세션.
        let resume_outcome = open(&mut sim, &mut ids, id(99), id(2));
        assert!(
            !resume_outcome
                .events
                .iter()
                .any(|e| e.body.event_type() == "SHIP_SPAWNED"),
            "재개는 새 SHIP_SPAWNED 를 만들지 않는다"
        );
        assert_eq!(sim.ship_count(), 1);
        assert!(sim.actor_ship.get(&id(2)).copied() == Some(ship_id));
        let resumed_session = sim.session(id(99)).expect("재개 세션이 있어야 한다");
        assert_eq!(
            resumed_session.last_applied_input_seq, None,
            "재개 세션의 input_seq 상태는 새로 시작한다(ADR-0011 §6.2)"
        );
    }

    /// AC-3(e2) — 재개 후 `input_seq=1` 이 `ACCEPTED` 다(이월 입력을 버렸으므로 기준이
    /// 0 부터 다시 시작한다는 것의 직접 증거).
    #[test]
    fn after_resume_input_seq_one_is_accepted() {
        let mut sim = Simulation::new(world(), 0);
        let mut ids = SeqIds(0);
        open(&mut sim, &mut ids, id(1), id(2));
        // 첫 세션에서 input_seq 5 까지 적용해 둔다.
        sim.step(
            vec![Submission::Command {
                seq: 0,
                session_id: id(1),
                command: ship_control(id(500), 5, 1000),
            }],
            &mut ids,
        );
        sim.step(
            vec![Submission::CloseSession {
                seq: 0,
                session_id: id(1),
                reason: SessionCloseReason::ClientClosed,
            }],
            &mut ids,
        );
        open(&mut sim, &mut ids, id(99), id(2));

        let outcome = sim.step(
            vec![Submission::Command {
                seq: 0,
                session_id: id(99),
                command: ship_control(id(501), 1, 1000),
            }],
            &mut ids,
        );
        let result = outcome
            .outbound
            .iter()
            .find_map(|out| match &out.message {
                ServerMessage::CommandResult(message) => Some(message.as_ref()),
                _ => None,
            })
            .expect("COMMAND_RESULT 가 있어야 한다");
        assert!(result.payload.invariant_holds());
        assert_eq!(
            result.payload.status,
            starfall_contracts::CommandStatus::Accepted
        );
    }

    /// S4 첫 테스트 — 같은 세션에 입력 5건을 한 번에 제출하고 1 tick 돌리면 마지막
    /// 1건만 적용되고 `input_superseded_total` 이 4 오른다.
    #[test]
    fn only_the_last_of_five_inputs_in_one_tick_wins() {
        let mut sim = Simulation::new(world(), 0);
        let mut ids = SeqIds(0);
        open(&mut sim, &mut ids, id(1), id(2));

        let mut world_with_room = world();
        world_with_room.rate_limit_per_tick_cap = 100; // 이 항목은 supersede 를, 별도 항목이 RATE_LIMITED 를 본다
        let mut sim2 = Simulation::new(world_with_room, 0);
        open(&mut sim2, &mut ids, id(1), id(2));

        let submissions = (1..=5u32)
            .map(|n| Submission::Command {
                seq: u64::from(n),
                session_id: id(1),
                command: ship_control(id(600 + u64::from(n)), n, 1000),
            })
            .collect();
        let outcome = sim2.step(submissions, &mut ids);

        assert_eq!(outcome.input_superseded, 4);
        let session = sim2.session(id(1)).unwrap();
        assert_eq!(session.last_applied_input_seq, Some(5));
        let accepted = outcome
            .outbound
            .iter()
            .filter(|out| {
                matches!(&out.message, ServerMessage::CommandResult(m) if m.payload.status == starfall_contracts::CommandStatus::Accepted)
            })
            .count();
        assert_eq!(
            accepted, 5,
            "5건 전부 ACCEPTED — superseded 는 별도 카운터로만 센다"
        );
    }

    /// ADR-0011 §4 — 한 tick에 `(5, 3)` 순서로 도착하면 5가 적용되고 3은 `STALE_INPUT`.
    #[test]
    fn later_lower_seq_is_stale_after_a_higher_seq_already_won_this_tick() {
        let mut sim = Simulation::new(world(), 0);
        let mut ids = SeqIds(0);
        open(&mut sim, &mut ids, id(1), id(2));

        let outcome = sim.step(
            vec![
                Submission::Command {
                    seq: 1,
                    session_id: id(1),
                    command: ship_control(id(700), 5, 1000),
                },
                Submission::Command {
                    seq: 2,
                    session_id: id(1),
                    command: ship_control(id(701), 3, 1000),
                },
            ],
            &mut ids,
        );
        let results: Vec<&CommandResultMessage> = outcome
            .outbound
            .iter()
            .filter_map(|out| match &out.message {
                ServerMessage::CommandResult(m) => Some(m.as_ref()),
                _ => None,
            })
            .collect();
        assert_eq!(results.len(), 2);
        assert_eq!(
            results[0].payload.status,
            starfall_contracts::CommandStatus::Accepted
        );
        assert_eq!(
            results[1].payload.reason_code,
            Some(RejectReasonCode::StaleInput)
        );
    }

    /// AC-6(b) 축소판 — 세션당 tick 처리 한도를 넘는 명령은 `RATE_LIMITED` 다.
    #[test]
    fn commands_beyond_the_per_tick_cap_are_rate_limited() {
        let mut sim = Simulation::new(world(), 0); // rate_limit_per_tick_cap = 2
        let mut ids = SeqIds(0);
        open(&mut sim, &mut ids, id(1), id(2));

        let submissions = (1..=5u32)
            .map(|n| Submission::Command {
                seq: u64::from(n),
                session_id: id(1),
                command: ship_control(id(800 + u64::from(n)), n, 1000),
            })
            .collect();
        let outcome = sim.step(submissions, &mut ids);
        let rate_limited = outcome
            .outbound
            .iter()
            .filter(|out| {
                matches!(&out.message, ServerMessage::CommandResult(m) if m.payload.reason_code == Some(RejectReasonCode::RateLimited))
            })
            .count();
        assert_eq!(rate_limited, 3, "cap=2 이므로 5건 중 3건이 RATE_LIMITED");
    }

    /// AC-4(a)/SC-15 통합 버전 — tick 루프를 통해 함선이 실제로 전진한다.
    #[test]
    fn ship_moves_forward_through_the_tick_loop() {
        let mut sim = Simulation::new(world(), 0);
        let mut ids = SeqIds(0);
        open(&mut sim, &mut ids, id(1), id(2));
        // 스폰 직후 자세는 원점을 바라보므로, thrust_z(전방) 로 밀면 원점 쪽으로 이동한다.
        // "위치가 변한다"만 확인한다(정확한 궤적은 world::integrate 단위 테스트가 이미 본다).
        let ship_id = *sim.actor_ship.get(&id(2)).unwrap();
        let before = sim.ships.get(&ship_id).unwrap().physics.p;
        for n in 1..=20u32 {
            sim.step(
                vec![Submission::Command {
                    seq: 0,
                    session_id: id(1),
                    command: ship_control(id(900 + u64::from(n)), n, 1000),
                }],
                &mut ids,
            );
        }
        let after = sim.ships.get(&ship_id).unwrap().physics.p;
        let moved =
            (after.x - before.x).abs() + (after.y - before.y).abs() + (after.z - before.z).abs();
        assert!(
            moved > 1.0,
            "20 tick 전방 추력 후 거의 안 움직였다: {moved}"
        );
    }

    /// AC-4(e) / SC-19 / M-17 "이월"·"이월 만료" — `Simulation` 수준. 진짜 입력 1건을
    /// 넣은 뒤 입력 없이 `carry_forward_max_ticks + N` tick 을 돌려, 계약이 요구한 관찰
    /// 둘을 함께 닫는다: (1) `input_carried_forward` 가 정확히 `carry_forward_max_ticks`
    /// 만큼만 증가하고 그 뒤로는 증가하지 않는다, (2) 이월이 만료되면 가속이 0이 되지만
    /// 속도는 감쇠만 적용되어 즉시 0이 되지 않는다(검증 방법으로 지정된 "만료 후 속도
    /// 곡선"). 전에는 `integrate::step` 을 직접 불러 만료 이후 상태를 손으로 만들어
    /// 넣었을 뿐, 이월 메커니즘 자체(이 파일의 `integrate_ships_for_tick`)를 한 번도 타지
    /// 않았다(QA 04_qa_report_r1.md §6.4).
    #[test]
    fn carry_forward_expires_after_configured_ticks_then_decays_by_damping_only() {
        let mut sim = Simulation::new(world(), 0);
        let mut ids = SeqIds(0);
        open(&mut sim, &mut ids, id(1), id(2));
        let ship_id = *sim.actor_ship.get(&id(2)).unwrap();

        // 진짜 입력 1건 — 전방 추력. 이 tick은 winning input 이 직접 있으므로 이월이
        // 아니다.
        let real_input_outcome = sim.step(
            vec![Submission::Command {
                seq: 0,
                session_id: id(1),
                command: ship_control(id(900), 1, 1000),
            }],
            &mut ids,
        );
        assert_eq!(
            real_input_outcome.input_carried_forward, 0,
            "진짜 입력이 도착한 tick 을 이월로 세면 안 된다"
        );

        let carry_forward_max_ticks = world().carry_forward_max_ticks;
        let total_ticks = carry_forward_max_ticks + 5; // 만료 후 5 tick 더 관찰한다.

        let mut carried_flags: Vec<u64> = Vec::with_capacity(total_ticks as usize);
        let mut speeds = vec![sim.ships.get(&ship_id).unwrap().physics.v.length()];
        for _ in 0..total_ticks {
            // 입력 없음 — 이 tick 은 이월(carry-forward) 아니면 휴면(dormant)만 탄다.
            let outcome = sim.step(vec![], &mut ids);
            carried_flags.push(outcome.input_carried_forward);
            speeds.push(sim.ships.get(&ship_id).unwrap().physics.v.length());
        }

        println!(
            "[AC-4e/SC-19] carry_forward_max_ticks={carry_forward_max_ticks}, 이월 플래그(tick별)={carried_flags:?}"
        );
        println!("[AC-4e/SC-19] 속도 곡선(|v|, 실입력 직후부터 tick별)={speeds:?}");

        // (1) 정확히 carry_forward_max_ticks 번만 이월되고, 그 뒤로는 증가하지 않는다.
        let expected_flags: Vec<u64> = (0..total_ticks)
            .map(|i| u64::from(i < carry_forward_max_ticks))
            .collect();
        assert_eq!(
            carried_flags, expected_flags,
            "이월은 carry_forward_max_ticks tick 동안만 켜지고 그 뒤로는 꺼져야 한다"
        );
        assert_eq!(
            carried_flags.iter().sum::<u64>(),
            u64::from(carry_forward_max_ticks),
            "input_carried_forward 증가 총합이 carry_forward_max_ticks 와 달라야 할 이유가 없다"
        );

        // (2) 만료 후 속도 곡선 — 가속(추력)이 0이 됐지만 감쇠만 적용되어 점진적으로
        // 줄어야 한다(즉시 0이 되지 않는다). speeds[0] 은 실입력 직후, speeds[i+1] 은
        // i 번째 이월/휴면 tick 이후다. 마지막 이월 tick(인덱스 carry_forward_max_ticks)
        // 다음부터가 휴면 구간이다.
        let last_carried_speed = speeds[carry_forward_max_ticks as usize];
        let post_expiry = &speeds[(carry_forward_max_ticks as usize + 1)..];
        let class = world().ship_class.movement;
        let dt = world().dt();
        let mut prev = last_carried_speed;
        for (i, &s) in post_expiry.iter().enumerate() {
            assert!(
                s < prev,
                "만료 후 tick {i} 에서 속도가 줄지 않았다(즉시 정지 또는 계속 가속 의심): {prev} -> {s}"
            );
            assert!(
                prev - s <= class.assist_linear_decel_mps2 * dt + 1e-6,
                "만료 후 tick {i} 에서 감쇠 한도보다 많이 줄었다(즉시 0이 되는 것에 가깝다): {prev} -> {s}"
            );
            prev = s;
        }
        assert!(
            *post_expiry.last().unwrap() > 0.0,
            "만료 후 관찰 구간이 끝나기 전에 이미 0이 됐다 — 즉시 정지처럼 보인다"
        );
    }

    /// SC-05 축소판 — 열려 있는 세션 2개가 있으면 스냅샷 tick에 각각 1건씩 받는다.
    #[test]
    fn snapshot_ticks_send_one_snapshot_per_open_session() {
        let mut sim = Simulation::new(world(), 0); // snapshot_interval_ticks = 2
        let mut ids = SeqIds(0);
        // 두 세션을 **같은 tick**(0)에 연다 — 각각 `open()` 을 부르면 그때마다 `step()` 이
        // tick 을 1씩 전진시켜 버려서 이후 tick 계산이 어긋난다.
        sim.step(
            vec![
                Submission::OpenSession {
                    seq: 0,
                    session_id: id(1),
                    actor_id: id(2),
                },
                Submission::OpenSession {
                    seq: 1,
                    session_id: id(3),
                    actor_id: id(4),
                },
            ],
            &mut ids,
        );

        // tick 0 은 열기 tick이라 스냅샷 대상(0 % 2 == 0)이다.
        let outcome = sim.step(Vec::new(), &mut ids); // tick 1 — 스냅샷 없음
        let snapshots = outcome
            .outbound
            .iter()
            .filter(|out| matches!(out.message, ServerMessage::WorldSnapshot(_)))
            .count();
        assert_eq!(snapshots, 0, "tick 1 은 배수가 아니다");

        let outcome2 = sim.step(Vec::new(), &mut ids); // tick 2
        let snapshots2: Vec<&WorldSnapshotMessage> = outcome2
            .outbound
            .iter()
            .filter_map(|out| match &out.message {
                ServerMessage::WorldSnapshot(m) => Some(m.as_ref()),
                _ => None,
            })
            .collect();
        assert_eq!(snapshots2.len(), 2);
        let controlled: std::collections::BTreeSet<_> = snapshots2
            .iter()
            .map(|m| m.payload.controlled_ship_id)
            .collect();
        assert_eq!(controlled.len(), 2, "각자 자기 함선을 조종해야 한다");
        assert_eq!(snapshots2[0].payload.ships.len(), 2, "월드에 함선 2척");
    }

    /// I-16/AC-8(d) — 종료 스윕: 활성 함선도 잔류 함선도 전부 디스폰된다(I-41).
    #[test]
    fn shutdown_despawns_both_active_and_lingering_ships() {
        let mut sim = Simulation::new(world(), 0);
        let mut ids = SeqIds(0);
        open(&mut sim, &mut ids, id(1), id(2)); // 활성으로 남을 함선
        open(&mut sim, &mut ids, id(3), id(4));
        sim.step(
            vec![Submission::CloseSession {
                seq: 0,
                session_id: id(3),
                reason: SessionCloseReason::ClientClosed,
            }],
            &mut ids,
        ); // 이제 잔류 중인 함선

        let outcome = sim.shutdown(&mut ids);
        let despawns: Vec<&ShipDespawnedPayload> = outcome
            .events
            .iter()
            .filter_map(|e| match &e.body {
                DomainEventBody::ShipDespawned(payload) => Some(payload),
                _ => None,
            })
            .collect();
        assert_eq!(despawns.len(), 2);
        assert!(
            despawns
                .iter()
                .all(|d| d.despawn_reason == DespawnReason::ServerShutdown)
        );
        assert_eq!(sim.ship_count(), 0);

        // 두 디스폰 모두 causation_id 가 비-null 이다(I-30).
        for event in &outcome.events {
            if matches!(&event.body, DomainEventBody::ShipDespawned(_)) {
                assert!(
                    event.causation_id.is_some(),
                    "SHIP_DESPAWNED 는 causation_id 가 비-null 이어야 한다(I-30)"
                );
            }
        }
        // id(1)/id(2) 는 이 tick에 처음 닫힌 활성 함선이라, 그 원인(SESSION_CLOSED)이
        // **같은 배치 안에** 있고 결과보다 sequence 가 작다(AC-3b). id(3)/id(4) 는 이전
        // tick에 이미 잔류를 시작했으므로 원인이 이 배치 밖에 있는 것이 정상이다
        // (causation_id 가 여러 tick 전을 가리킬 수 있다 — ADR-0011 §6).
        let active_ship_despawn = outcome
            .events
            .iter()
            .find(|e| matches!(&e.body, DomainEventBody::ShipDespawned(p) if p.last_session_id == id(1)))
            .expect("활성 함선의 디스폰 이벤트가 있어야 한다");
        let cause_in_batch = outcome
            .events
            .iter()
            .find(|e| Some(e.event_id) == active_ship_despawn.causation_id)
            .expect("활성 함선 디스폰의 원인은 같은 배치 안에 있어야 한다");
        assert!(cause_in_batch.sequence.get() < active_ship_despawn.sequence.get());
    }

    /// R4 S-1 — 매 tick 이 지켜야 할 I-29/I-30 불변식 4가지(architect R4 결정 1.9).
    /// `all_events`는 시나리오 시작부터 지금까지 발행된 이벤트 전체(누적) — (iv)는
    /// 원인이 여러 tick 전일 수 있어 한 tick만으로는 검사할 수 없다.
    fn assert_i29_invariants(
        sim: &Simulation,
        outcome: &TickOutcome,
        all_events: &mut Vec<PendingEvent>,
    ) {
        // (i) actor 당 함선은 최대 1척이다.
        let mut seen_actors = std::collections::BTreeSet::new();
        for ship in sim.ships.values() {
            assert!(
                seen_actors.insert(ship.actor_id),
                "actor {} 가 함선을 2척 이상 가졌다(tick {})",
                ship.actor_id,
                outcome.tick
            );
        }

        // (ii) ACTIVE 함선은 고아가 아니다 — 조종 세션이 있고, 그 세션은 열려 있고,
        // 한 세션이 함선을 2척 이상 조종하지 않는다.
        let mut claimed_sessions = std::collections::BTreeSet::new();
        for ship in sim.ships.values() {
            if ship.presence != ShipPresence::Active {
                continue;
            }
            let Some(controlling) = ship.controlling_session else {
                panic!(
                    "함선 {} 이 ACTIVE 인데 조종 세션이 없다(tick {})",
                    ship.ship_id, outcome.tick
                );
            };
            assert!(
                sim.sessions.contains_key(&controlling),
                "함선 {} 의 조종 세션 {controlling} 이 이미 닫혀 있다 — 고아 ACTIVE(tick {})",
                ship.ship_id,
                outcome.tick
            );
            assert!(
                claimed_sessions.insert(controlling),
                "세션 {controlling} 이 함선을 2척 이상 조종한다(tick {})",
                outcome.tick
            );
        }

        all_events.extend(outcome.events.iter().cloned());

        // (iii) 이번 tick 이 발행한 이벤트 중 자기 자신을 원인으로 가리키는 것이 없다.
        for event in &outcome.events {
            assert_ne!(
                event.causation_id,
                Some(event.event_id),
                "이벤트 {} 가 자기 자신을 원인으로 가리킨다(tick {})",
                event.event_id,
                outcome.tick
            );
        }

        // (iv) 모든 SHIP_DESPAWNED 의 원인은 먼저 발행된 SESSION_CLOSED 다.
        for event in &outcome.events {
            let DomainEventBody::ShipDespawned(_) = &event.body else {
                continue;
            };
            let Some(cause_id) = event.causation_id else {
                panic!(
                    "SHIP_DESPAWNED {} 의 causation_id 가 null 이다(I-30)",
                    event.event_id
                );
            };
            let cause = all_events
                .iter()
                .find(|e| e.event_id == cause_id)
                .unwrap_or_else(|| {
                    panic!(
                        "SHIP_DESPAWNED {} 의 원인 {cause_id} 이 어떤 발행 이벤트에도 없다",
                        event.event_id
                    )
                });
            assert!(
                matches!(cause.body, DomainEventBody::SessionClosed(_)),
                "SHIP_DESPAWNED {} 의 원인이 SESSION_CLOSED 가 아니다 ({})",
                event.event_id,
                cause.body.event_type()
            );
            assert!(
                (cause.tick.get(), cause.sequence.get()) < (event.tick.get(), event.sequence.get()),
                "SHIP_DESPAWNED {} 의 원인이 결과보다 나중이거나 같다",
                event.event_id
            );
        }
    }

    /// R4 S-1 — QA R3 §6.7 in-process 재현. 같은 actor 의 동시 `OpenSession` 2건이 함선을
    /// 2척 만들고, 첫 함선이 고아 `ACTIVE` 로 남아 종료 스윕에서 자기 자신을 원인으로
    /// 디스폰되는 버그(자기 참조 causation, I-29/I-30 위반). **수정 전 코드에서 RED**
    /// (architect R4 결정 1.9 S-1). 시나리오: `OpenSession` 2건 → 둘 다 닫기 → 잔류 만료
    /// → `shutdown`. 매 tick 위 4개 불변식을 확인한다.
    #[test]
    fn r4_s1_concurrent_sessions_do_not_orphan_or_fabricate_causation() {
        let mut sim = Simulation::new(world(), 0);
        let mut ids = SeqIds(0);
        let actor = id(2);
        let mut all_events: Vec<PendingEvent> = Vec::new();

        let s1 = id(101);
        let s2 = id(102);

        let out = open(&mut sim, &mut ids, s1, actor);
        assert_i29_invariants(&sim, &out, &mut all_events);

        // 동시 접속 — QA R3 §6.7 재현. 첫 세션이 아직 열려 있는 채로 같은 actor 가
        // 두 번째 세션을 연다.
        let out = open(&mut sim, &mut ids, s2, actor);
        assert_i29_invariants(&sim, &out, &mut all_events);

        let out = sim.step(
            vec![Submission::CloseSession {
                seq: 0,
                session_id: s1,
                reason: SessionCloseReason::ClientClosed,
            }],
            &mut ids,
        );
        assert_i29_invariants(&sim, &out, &mut all_events);

        let out = sim.step(
            vec![Submission::CloseSession {
                seq: 0,
                session_id: s2,
                reason: SessionCloseReason::ClientClosed,
            }],
            &mut ids,
        );
        assert_i29_invariants(&sim, &out, &mut all_events);

        // 잔류 만료까지 진행한다 (linger_seconds=30, tick_hz=20 => 600 tick, world() 기준).
        for _ in 0..600 {
            let out = sim.step(Vec::new(), &mut ids);
            assert_i29_invariants(&sim, &out, &mut all_events);
        }

        let out = sim.shutdown(&mut ids);
        assert_i29_invariants(&sim, &out, &mut all_events);
    }

    /// R4 S-3 — `despawn_ship` 자신의 방어선, **디버그 팔**. 잔류 원인 없이 이 함수에
    /// 도달하면(구조적으로는 S-2 가 막아 정상 흐름으로는 만들 수 없는 상태다) 디버그
    /// 빌드는 그 자리에서 패닉한다 — 그럴듯한 원인을 지어내지 않는다(옛
    /// `unwrap_or(event_id)` 자기 참조 금지, I-30). `step`/`shutdown` 을 거치지 않고
    /// `despawn_ship` 을 직접 불러, 그 두 함수 끝의 `debug_assert_i29`(더 일찍 잡는
    /// 바깥 검사)가 아니라 **이 함수 자체의 방어선**만 확인한다.
    ///
    /// **릴리스 팔은 이 테스트로 확인되지 않는다** — `debug_assert!` 는 릴리스에서
    /// 아무것도 안 하므로 패닉이 나지 않고, `#[should_panic]` 은 그 빌드에서 거짓으로
    /// 실패한다. 그래서 `#[cfg(debug_assertions)]` 로 이 테스트를 디버그 빌드에만
    /// 존재하게 한다 — 짝은 아래
    /// [`r4_s3_release_arm_signals_without_fabricating_an_event`](team-lead 요청, QA
    /// 1.4절 공백)다.
    #[cfg(debug_assertions)]
    #[test]
    #[should_panic(expected = "I-29/I-30 위반")]
    fn r4_s3_despawn_ship_panics_in_debug_when_the_linger_cause_is_missing() {
        let mut sim = Simulation::new(world(), 0);
        let mut ids = SeqIds(0);
        let out = open(&mut sim, &mut ids, id(1), id(2));
        let DomainEventBody::ShipSpawned(spawned) = &out.events[1].body else {
            panic!()
        };
        let ship_id = spawned.ship_id;

        // 원인 없이 LINGERING 으로 손상시킨다 — 정상 흐름(S-2)으로는 만들 수 없는 상태를
        // 이 방어선 자체를 시험하려고 직접 만든다(같은 크레이트의 단위 테스트 권한).
        let ship = sim.ships.get_mut(&ship_id).unwrap();
        ship.presence = ShipPresence::Lingering;
        ship.controlling_session = None;
        ship.linger_cause_event_id = None;
        ship.linger_cause_correlation_id = None;

        let tick = Tick::new(1).unwrap();
        let occurred_at = sim.world().calendar.occurred_at(tick).unwrap();
        let mut outcome = TickOutcome {
            tick: 1,
            ..TickOutcome::default()
        };
        sim.despawn_ship(
            ship_id,
            DespawnReason::LingerExpired,
            tick,
            &occurred_at,
            &mut ids,
            &mut outcome,
        );
    }

    /// R4 S-3 후속 (team-lead 요청, QA 1.4절 — 릴리스 팔은 정적 확인뿐이었다) —
    /// **릴리스 팔**을 실행으로 확인한다. 디버그 빌드는 `debug_assert!` 가 먼저
    /// 패닉해 이 팔에 도달하지 않으므로(위 테스트가 그것을 확인한다), 이 테스트는
    /// `#[cfg(not(debug_assertions))]` 로 **릴리스 빌드에만** 존재한다 —
    /// `cargo test -p starfall-sim --release` 로 실행해야 한다.
    ///
    /// 원인 없는 디스폰에 도달했을 때 **이벤트를 쓰지 않고** `causeless_despawns` 에만
    /// 신호를 남기는지를 직접 실행해 확인한다 — "빠진 기록은 짝 검사가 잡지만 그럴듯한
    /// 가짜 원인은 아무도 못 잡는다"는 이 슬라이스의 원칙이 걸린 자리라서, 코드를 읽고
    /// "그렇게 생겼다"로 넘기지 않는다.
    #[cfg(not(debug_assertions))]
    #[test]
    fn r4_s3_release_arm_signals_without_fabricating_an_event() {
        let mut sim = Simulation::new(world(), 0);
        let mut ids = SeqIds(0);
        let out = open(&mut sim, &mut ids, id(1), id(2));
        let DomainEventBody::ShipSpawned(spawned) = &out.events[1].body else {
            panic!()
        };
        let ship_id = spawned.ship_id;

        // 원인 없이 LINGERING 으로 손상시킨다 — 정상 흐름(S-2)으로는 만들 수 없는 상태를
        // 이 방어선 자체를 시험하려고 직접 만든다(같은 크레이트의 단위 테스트 권한).
        let ship = sim.ships.get_mut(&ship_id).unwrap();
        ship.presence = ShipPresence::Lingering;
        ship.controlling_session = None;
        ship.linger_cause_event_id = None;
        ship.linger_cause_correlation_id = None;

        let tick = Tick::new(1).unwrap();
        let occurred_at = sim.world().calendar.occurred_at(tick).unwrap();
        let mut outcome = TickOutcome {
            tick: 1,
            ..TickOutcome::default()
        };
        sim.despawn_ship(
            ship_id,
            DespawnReason::LingerExpired,
            tick,
            &occurred_at,
            &mut ids,
            &mut outcome,
        );

        assert_eq!(
            outcome.causeless_despawns,
            vec![ship_id],
            "릴리스 팔이 신호를 남겨야 한다"
        );
        assert!(
            outcome.events.is_empty(),
            "릴리스 팔은 이벤트를 쓰지 않는다 — 그럴듯한 원인을 지어내지 않는다(I-30, \
             옛 unwrap_or(event_id) 금지)"
        );
        assert_eq!(
            sim.ship_count(),
            0,
            "함선 상태는 여전히 제거된다 — 신호만 남기고 상태를 누수하지 않는다"
        );
    }

    /// `ShipPhysicsState` 의 모든 `f64` 필드를 비트 패턴으로 뽑는다 — `==` 가 아니라
    /// **비트 동일**을 확인하기 위해서다(`-0.0`/`NaN` 도 `==` 로는 가려지지 않는다, S-4).
    fn physics_bits(state: &ShipPhysicsState) -> [u64; 14] {
        [
            state.p.x.to_bits(),
            state.p.y.to_bits(),
            state.p.z.to_bits(),
            state.v.x.to_bits(),
            state.v.y.to_bits(),
            state.v.z.to_bits(),
            state.q.x.to_bits(),
            state.q.y.to_bits(),
            state.q.z.to_bits(),
            state.q.w.to_bits(),
            state.omega_aim.x.to_bits(),
            state.omega_aim.y.to_bits(),
            state.omega_aim.z.to_bits(),
            state.omega_roll.to_bits(),
        ]
    }

    /// R4 S-4 (AC-3(d1)) — 재접속 시나리오와 재접속하지 않는 대조 시나리오를 나란히
    /// 돌려, 재개 이후 함선의 `f64` 물리 상태가 **비트 단위로 동일**한지 본다.
    ///
    /// 재개(`ShipEntity::activate`/`discard_carried_input_for_resume`)는 세션 부기만
    /// 만지고 `physics` 필드는 건드리지 않는다 — 코드로는 이미 자명하지만, 이 테스트는
    /// 그것을 **실행으로** 고정한다(회귀가 생기면 여기서 잡힌다). 대조 실행은 세션을
    /// 한 번도 닫지 않고 같은 구간 동안 입력만 보내지 않는다 — "잔류→재개 경로를
    /// 거치는 것 자체"가 물리를 바꾸는지가 유일한 변수가 되게 격리한다.
    ///
    /// 간격은 `carry_forward_max_ticks`(world() 기준 10)보다 훨씬 크게 잡는다(재접속은
    /// tick 60 에서, 종료 명령은 tick 3 이다) — 이월이 끝나고 휴면 입력으로 넘어간
    /// 뒤에 재개하는 경우까지 덮기 위해서다.
    fn run_ac3_d1_scenario(reconnect: bool) -> Vec<(u64, [u64; 14])> {
        let mut sim = Simulation::new(world(), 0);
        let mut ids = SeqIds(0);
        let actor = id(2);
        let session_a = id(9001);

        open(&mut sim, &mut ids, session_a, actor);
        sim.step(
            vec![Submission::Command {
                seq: 0,
                session_id: session_a,
                command: ship_control(id(9500), 1, 1000),
            }],
            &mut ids,
        );

        const RESUME_AT_TICK: u64 = 60;
        if reconnect {
            sim.step(
                vec![Submission::CloseSession {
                    seq: 0,
                    session_id: session_a,
                    reason: SessionCloseReason::ClientClosed,
                }],
                &mut ids,
            );
            while sim.next_tick() < RESUME_AT_TICK - 1 {
                sim.step(Vec::new(), &mut ids);
            }
            let session_b = id(9002);
            open(&mut sim, &mut ids, session_b, actor);
        } else {
            while sim.next_tick() < RESUME_AT_TICK {
                sim.step(Vec::new(), &mut ids);
            }
        }
        assert_eq!(
            sim.next_tick(),
            RESUME_AT_TICK,
            "두 시나리오가 같은 tick에서 비교 구간을 시작해야 한다"
        );

        const COMPARE_TICKS: u64 = 80;
        let mut samples = Vec::with_capacity(COMPARE_TICKS as usize);
        for _ in 0..COMPARE_TICKS {
            let out = sim.step(Vec::new(), &mut ids);
            let ship = sim
                .ships
                .values()
                .next()
                .expect("비교 구간 동안 함선이 있어야 한다(디스폰 전이다)");
            samples.push((out.tick, physics_bits(&ship.physics)));
        }
        samples
    }

    #[test]
    fn ac3_d1_reconnect_does_not_perturb_f64_physics_state() {
        let reconnected = run_ac3_d1_scenario(true);
        let control = run_ac3_d1_scenario(false);

        assert_eq!(
            reconnected.len(),
            control.len(),
            "비교 tick 수가 같아야 한다"
        );
        let compared_ticks = reconnected.len();
        for ((tick_a, bits_a), (tick_b, bits_b)) in reconnected.iter().zip(control.iter()) {
            assert_eq!(tick_a, tick_b, "비교 대상 tick 번호가 어긋났다");
            assert_eq!(
                bits_a, bits_b,
                "tick {tick_a} 에서 재접속 실행과 대조 실행의 f64 물리 상태가 비트 단위로 \
                 어긋났다 — 재개가 물리를 건드렸다는 뜻이다(AC-3(d1) 위반)"
            );
        }
        println!(
            "[AC-3(d1)] 재접속 vs 대조, tick {}~{} ({compared_ticks} tick) 비트 동일 확인",
            reconnected[0].0,
            reconnected[compared_ticks - 1].0
        );
    }

    /// R4 S-6 (architect 1.9 정책별 요구 4) — 같은 tick에 도착한 `OpenSession(S2)` 와
    /// `CloseSession(S1)` 의 처리 순서(`seq`)가 결과를 결정한다(ADR-0011 §6.3 "sim 은
    /// 제출 순번으로 결정적이다"). 두 방향을 in-process로 확인한다.
    #[test]
    fn r4_s6_open_before_close_supersedes_the_old_session() {
        let mut sim = Simulation::new(world(), 0);
        let mut ids = SeqIds(0);
        let actor = id(2);
        let s1 = id(101);
        let s2 = id(102);

        let opened_s1 = open(&mut sim, &mut ids, s1, actor);
        let DomainEventBody::ShipSpawned(spawned) = &opened_s1.events[1].body else {
            panic!()
        };
        let ship_id = spawned.ship_id;

        // S2 열기(seq 0)가 S1 닫기(seq 1)보다 먼저 처리된다 → 넘겨받기.
        let out = sim.step(
            vec![
                Submission::OpenSession {
                    seq: 0,
                    session_id: s2,
                    actor_id: actor,
                },
                Submission::CloseSession {
                    seq: 1,
                    session_id: s1,
                    reason: SessionCloseReason::ClientClosed,
                },
            ],
            &mut ids,
        );

        let closed: Vec<_> = out
            .events
            .iter()
            .filter(|e| matches!(&e.body, DomainEventBody::SessionClosed(_)))
            .collect();
        assert_eq!(
            closed.len(),
            1,
            "SESSION_CLOSED 는 세션당 정확히 1건이어야 한다(I-16) — S1 의 나중 \
             CloseSession 은 세션이 이미 없어 무시된다"
        );
        let DomainEventBody::SessionClosed(payload) = &closed[0].body else {
            unreachable!()
        };
        assert_eq!(payload.session_id, s1);
        assert_eq!(payload.close_reason, SessionCloseReason::Superseded);
        assert!(
            closed[0].causation_id.is_some(),
            "SUPERSEDED 만 causation_id 가 비-null 이다(ADR-0011 §6.3)"
        );
        let opened_s2_event_id = out
            .events
            .iter()
            .find(|e| matches!(&e.body, DomainEventBody::SessionOpened(_)))
            .expect("SESSION_OPENED(S2) 가 있어야 한다")
            .event_id;
        assert_eq!(
            closed[0].causation_id,
            Some(opened_s2_event_id),
            "원인은 넘겨받은 SESSION_OPENED(S2) 다"
        );

        assert!(
            !out.events.iter().any(|e| matches!(
                &e.body,
                DomainEventBody::ShipSpawned(_) | DomainEventBody::ShipDespawned(_)
            )),
            "넘겨받기는 잔류를 거치지 않는다 — 함선 이벤트가 없다"
        );
        assert_eq!(sim.ship_count(), 1, "새 함선을 만들지 않는다(I-29)");
        let ship = sim
            .ships
            .get(&ship_id)
            .expect("같은 함선이 그대로 있어야 한다");
        assert_eq!(ship.presence, ShipPresence::Active);
        assert_eq!(
            ship.controlling_session,
            Some(s2),
            "조종이 같은 tick에 S2 로 넘어갔다"
        );
        let session2 = sim.sessions.get(&s2).expect("S2 가 열려 있어야 한다");
        assert_eq!(session2.controlling_ship, Some(ship_id));
        assert!(!sim.sessions.contains_key(&s1), "S1 은 넘겨받기로 닫혔다");
    }

    #[test]
    fn r4_s6_close_before_open_resumes_without_superseding() {
        let mut sim = Simulation::new(world(), 0);
        let mut ids = SeqIds(0);
        let actor = id(2);
        let s1 = id(201);
        let s2 = id(202);

        let opened_s1 = open(&mut sim, &mut ids, s1, actor);
        let DomainEventBody::ShipSpawned(spawned) = &opened_s1.events[1].body else {
            panic!()
        };
        let ship_id = spawned.ship_id;

        // S1 닫기(seq 0)가 S2 열기(seq 1)보다 먼저 처리된다 → 잔류 → 같은 tick에 재개
        // (넘겨받기가 아니다).
        let out = sim.step(
            vec![
                Submission::CloseSession {
                    seq: 0,
                    session_id: s1,
                    reason: SessionCloseReason::ClientClosed,
                },
                Submission::OpenSession {
                    seq: 1,
                    session_id: s2,
                    actor_id: actor,
                },
            ],
            &mut ids,
        );

        let closed: Vec<_> = out
            .events
            .iter()
            .filter(|e| matches!(&e.body, DomainEventBody::SessionClosed(_)))
            .collect();
        assert_eq!(closed.len(), 1);
        let DomainEventBody::SessionClosed(payload) = &closed[0].body else {
            unreachable!()
        };
        assert_eq!(
            payload.close_reason,
            SessionCloseReason::ClientClosed,
            "S1 닫기가 먼저면 평범한 닫기다 — SUPERSEDED 가 아니다"
        );
        assert!(
            closed[0].causation_id.is_none(),
            "SUPERSEDED 가 아닌 사유는 causation_id 가 null 이다"
        );
        assert!(
            !out.events.iter().any(|e| matches!(
                &e.body,
                DomainEventBody::ShipSpawned(_) | DomainEventBody::ShipDespawned(_)
            )),
            "같은 tick 안의 재개라 함선 이벤트가 없다"
        );

        assert_eq!(sim.ship_count(), 1);
        let ship = sim
            .ships
            .get(&ship_id)
            .expect("같은 함선이 그대로 있어야 한다");
        assert_eq!(
            ship.presence,
            ShipPresence::Active,
            "S2 가 같은 tick에 재개했다"
        );
        assert_eq!(ship.controlling_session, Some(s2));
    }

    /// 같은 입력 + 같은 id 생성기 = 같은 출력 (원칙 9) — p1-01 로 넓힌 버전.
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
                    vec![Submission::Command {
                        seq: 1,
                        session_id: session,
                        command: ship_control(id(31), 1, 1000),
                    }],
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

    /// AC-7(b) — `ships` 는 언제나 `ship_id` 오름차순이다(`BTreeMap` 순회, I-37).
    #[test]
    fn snapshot_ships_are_sorted_by_ship_id() {
        let mut sim = Simulation::new(world(), 0);
        let mut ids = SeqIds(0);
        // 실측 UUIDv7 은 시간순이라 순서가 자명해지므로, 여러 actor 를 열어 오름차순
        // 검사만 한다(생성 순서와 정렬 순서가 항상 같은지는 보지 않는다).
        for n in 1..=5u64 {
            open(&mut sim, &mut ids, id(n), id(100 + n));
        }
        let outcome = sim.step(Vec::new(), &mut ids); // tick 1 은 배수가 아닐 수 있다
        let outcome = if outcome
            .outbound
            .iter()
            .any(|o| matches!(o.message, ServerMessage::WorldSnapshot(_)))
        {
            outcome
        } else {
            sim.step(Vec::new(), &mut ids)
        };
        let snapshot = outcome
            .outbound
            .iter()
            .find_map(|out| match &out.message {
                ServerMessage::WorldSnapshot(m) => Some(m.as_ref()),
                _ => None,
            })
            .expect("스냅샷이 있어야 한다");
        let ids_in_order: Vec<String> = snapshot
            .payload
            .ships
            .iter()
            .map(|s| s.ship_id.to_string())
            .collect();
        let mut sorted = ids_in_order.clone();
        sorted.sort();
        assert_eq!(ids_in_order, sorted);
        assert_eq!(ids_in_order.len(), 5);
    }
}

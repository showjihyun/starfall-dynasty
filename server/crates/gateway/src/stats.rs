//! `GET /debug/stats` — 운영 표면 (ADR-0007 §8). **계약이 아니다.**
//!
//! Unity 클라이언트가 소비하기 시작하면 `contracts/api/` 로 승격한다.
//!
//! # 의존성 없이 만든다
//!
//! `metrics` 크레이트는 그 자체로 저장소가 없어 `metrics-exporter-prometheus` 를 함께 들여야
//! 하고, 그러면 **스크레이퍼도 없는데** 익스포터 트리를 지불한다. `AtomicU64` 와 고정 배열이면
//! 지금 표면을 전부 덮는다.
//!
//! # 링 버퍼가 아니라 고정 버킷 히스토그램이다
//!
//! 링 버퍼로 분위수를 내면 **버퍼 길이 = 관측 창**이 된다. A 단계가 1200 tick 인데 버퍼가
//! 1024 면 리포트의 "A 단계 p99" 가 사실은 "마지막 51초의 p99" 다. p1 회귀 기준선으로 쓰려면
//! 창이 실행 전체여야 한다.
//!
//! # 항진명제를 메트릭으로 두지 않는다 (I-25)
//!
//! - `ws_connections` 는 카운터 뺄셈이 아니라 **tick 드라이버의 라우팅 표 실제 길이**다.
//!   그래야 `ws_connections == opened − closed` 가 "레지스트리에서 지워졌는데 카운터가 안
//!   올랐다" 같은 진짜 버그를 잡는다.
//! - `tick_skipped_total` 은 없다. 설계상 언제나 0이라 아무것도 검증하지 않는다. 대신
//!   `tick_total == tick − start_tick + 1` 항등식을 노출한다.

use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

use axum::Json;
use axum::extract::State;
use serde::Serialize;
use starfall_contracts::messages::RejectReasonCode;

use crate::state::AppState;

/// tick 본문 소요 히스토그램의 버킷 상한 (마이크로초).
const BUCKET_BOUNDS_US: [u64; 15] = [
    100, 200, 500, 1_000, 2_000, 5_000, 10_000, 20_000, 50_000, 100_000, 200_000, 500_000,
    1_000_000, 2_000_000, 5_000_000,
];

/// tick 초과 판정 기준 (마이크로초). ADR-0006 §7 — **`run_tick()` 본문 소요**다.
pub const TICK_OVERRUN_US: u64 = 50_000;

/// 메시지 타입 라벨. 고정 배열 색인과 순서가 같아야 한다.
const MESSAGE_TYPES: [&str; 4] = [
    "SESSION_READY",
    "COMMAND_RESULT",
    "PING_REPLY",
    "WORLD_SNAPSHOT",
];
/// 업그레이드 거절 사유 라벨. 고정 배열 색인과 순서가 같아야 한다. p1-01 이
/// `world_full`(architect 결정 — 스폰이 아니라 입장에서 막는다)을 더했다.
const UPGRADE_REJECTIONS: [&str; 6] = [
    "auth_not_configured",
    "recording_backlog",
    "shutting_down",
    "no_credential",
    "invalid_token",
    "world_full",
];
/// 거부 사유 라벨. 고정 배열 색인과 순서가 같아야 한다. p1-01 이 `RATE_LIMITED`·
/// `STALE_INPUT` 2개를 더해 8라벨이 됐다 — 순서는 계약 스키마의 `enum` 순서 그대로다
/// (스프린트 계약 §1-④).
const REJECT_REASONS: [&str; 8] = [
    "MALFORMED_COMMAND",
    "UNKNOWN_COMMAND_TYPE",
    "SCHEMA_VERSION_UNSUPPORTED",
    "DUPLICATE_COMMAND_ID",
    "SERVER_BUSY",
    "TOO_MANY_IN_FLIGHT",
    "RATE_LIMITED",
    "STALE_INPUT",
];

/// 메시지 타입 → 고정 배열 색인.
#[must_use]
pub fn message_index(type_name: &str) -> usize {
    MESSAGE_TYPES
        .iter()
        .position(|candidate| *candidate == type_name)
        .unwrap_or(0)
}

/// 거부 사유 → 고정 배열 색인.
#[must_use]
pub const fn reason_index(reason: RejectReasonCode) -> usize {
    match reason {
        RejectReasonCode::MalformedCommand => 0,
        RejectReasonCode::UnknownCommandType => 1,
        RejectReasonCode::SchemaVersionUnsupported => 2,
        RejectReasonCode::DuplicateCommandId => 3,
        RejectReasonCode::ServerBusy => 4,
        RejectReasonCode::TooManyInFlight => 5,
        RejectReasonCode::RateLimited => 6,
        RejectReasonCode::StaleInput => 7,
    }
}

#[derive(Debug, Default)]
struct Histogram {
    /// 버킷별 도수. 마지막 칸은 `+inf`.
    buckets: [AtomicU64; BUCKET_BOUNDS_US.len() + 1],
    count: AtomicU64,
    sum_us: AtomicU64,
    max_us: AtomicU64,
}

impl Histogram {
    fn record(&self, value_us: u64) {
        let index = BUCKET_BOUNDS_US
            .iter()
            .position(|bound| value_us <= *bound)
            .unwrap_or(BUCKET_BOUNDS_US.len());
        self.buckets[index].fetch_add(1, Ordering::Relaxed);
        self.count.fetch_add(1, Ordering::Relaxed);
        self.sum_us.fetch_add(value_us, Ordering::Relaxed);
        self.max_us.fetch_max(value_us, Ordering::Relaxed);
    }

    /// 분위수의 **버킷 상한**. 정확한 값이 아니라 보수적 상한이다(이름이 그것을 말한다).
    fn quantile_le_us(&self, quantile: f64) -> u64 {
        let total = self.count.load(Ordering::Relaxed);
        if total == 0 {
            return 0;
        }
        #[allow(
            clippy::cast_precision_loss,
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss
        )]
        let target = ((total as f64) * quantile).ceil() as u64;
        let mut cumulative = 0;
        for (index, bucket) in self.buckets.iter().enumerate() {
            cumulative += bucket.load(Ordering::Relaxed);
            if cumulative >= target.max(1) {
                return BUCKET_BOUNDS_US.get(index).copied().unwrap_or(u64::MAX);
            }
        }
        u64::MAX
    }

    fn snapshot(&self) -> HistogramBody {
        let count = self.count.load(Ordering::Relaxed);
        HistogramBody {
            count,
            sum_us: self.sum_us.load(Ordering::Relaxed),
            max_us: self.max_us.load(Ordering::Relaxed),
            p50_le_us: self.quantile_le_us(0.50),
            p90_le_us: self.quantile_le_us(0.90),
            p99_le_us: self.quantile_le_us(0.99),
            bucket_bounds_us: BUCKET_BOUNDS_US.to_vec(),
            buckets: self
                .buckets
                .iter()
                .map(|bucket| bucket.load(Ordering::Relaxed))
                .collect(),
        }
    }
}

#[derive(Debug)]
struct Inner {
    start_tick: AtomicU64,
    tick: AtomicU64,
    tick_started: AtomicBool,
    tick_total: AtomicU64,
    tick_overrun_total: AtomicU64,
    tick_body: Histogram,
    tick_lag_micros: AtomicI64,
    ws_connections: AtomicU64,
    live_connections: AtomicU64,
    sessions_opened_total: AtomicU64,
    sessions_closed_total: AtomicU64,
    commands_received_total: AtomicU64,
    commands_rejected_total: [AtomicU64; REJECT_REASONS.len()],
    messages_enqueued_total: [AtomicU64; MESSAGE_TYPES.len()],
    messages_written_total: [AtomicU64; MESSAGE_TYPES.len()],
    messages_dropped_total: AtomicU64,
    send_queue_depth: AtomicU64,
    send_queue_depth_max: AtomicU64,
    command_queue_depth: AtomicU64,
    command_queue_depth_max: AtomicU64,
    protocol_violations_total: AtomicU64,
    /// tick당 명령 상한(`MAX_COMMANDS_PER_SESSION_PER_TICK`)을 넘겨 판정에 넣지 못한
    /// 명령 누적(ADR-0011 §5.2, S10). `COMMAND_RESULT`가 없는 손실이라 I-15 범위 밖이고,
    /// QA의 손실 항등식은 `보낸 수 = COMMAND_RESULT 수 + commands_dropped_over_tick_cap_total`이 된다.
    commands_dropped_over_tick_cap_total: AtomicU64,
    upgrade_rejected_total: [AtomicU64; UPGRADE_REJECTIONS.len()],
    accepting_connections: AtomicBool,
    // 영속화 태스크가 같은 Arc 를 갱신한다. 크레이트 의존이 아니라 원자값만 공유한다.
    persisted_total: Arc<AtomicU64>,
    persist_failed_total: Arc<AtomicU64>,
    last_committed_tick: Arc<AtomicU64>,
    // p1-01 — data/ 로딩 결과(기동 시 한 번 쓰고 다시 바뀌지 않는다, SC-05).
    data_dir: RwLock<String>,
    ship_classes_loaded: AtomicU64,
    spawn_points_loaded: AtomicU64,
    snapshot_interval_ticks: AtomicU64,
    /// `WORLD_SNAPSHOT.ships` 의 계약 상한(`max_entities_per_snapshot`, `1..=64`) — `GET /ws`
    /// 가 이 값에 도달하면 새 연결을 `503 world_full` 로 막는다(architect 결정, 스폰을
    /// 거부하는 대신 입장에서 막는다 — 함선 없는 세션이 생기면 I-29 가 깨진다).
    world_capacity: AtomicU64,
    // p1-01 — 신규 메트릭 7종(SC-33) + 기록용 히스토그램(M-1).
    snapshots_sent_total: AtomicU64,
    snapshot_bytes_total: AtomicU64,
    send_queue_bytes: AtomicU64,
    send_queue_bytes_max: AtomicU64,
    input_superseded_total: AtomicU64,
    input_carried_forward_total: AtomicU64,
    aim_degenerate_total: AtomicU64,
    ships_active: AtomicU64,
    ships_lingering: AtomicU64,
    snapshot_build_us: Histogram,
    snapshot_over_capacity_total: AtomicU64,
    /// 지금 함선을 가진 actor 집합(활성 + 잔류) — `world_full` 게이트의 재개 면제(I-44,
    /// S9)에 쓴다. tick 드라이버가 매 tick `Simulation::actors_with_ships()` 로 통째로
    /// 덮어쓴다(`ws_connections` 와 같은 취급 — 카운터가 아니라 실제 집합의 스냅샷).
    actors_with_ships: RwLock<std::collections::HashSet<starfall_contracts::primitives::UuidV7>>,
}

/// 서버 전체의 관측 값. 싸게 clone 된다.
#[derive(Debug, Clone)]
pub struct Stats {
    inner: Arc<Inner>,
}

impl Default for Stats {
    fn default() -> Self {
        Self::new()
    }
}

impl Stats {
    /// 빈 통계를 만든다.
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Inner {
                start_tick: AtomicU64::new(0),
                tick: AtomicU64::new(0),
                tick_started: AtomicBool::new(false),
                tick_total: AtomicU64::new(0),
                tick_overrun_total: AtomicU64::new(0),
                tick_body: Histogram::default(),
                tick_lag_micros: AtomicI64::new(0),
                ws_connections: AtomicU64::new(0),
                live_connections: AtomicU64::new(0),
                sessions_opened_total: AtomicU64::new(0),
                sessions_closed_total: AtomicU64::new(0),
                commands_received_total: AtomicU64::new(0),
                commands_rejected_total: Default::default(),
                messages_enqueued_total: Default::default(),
                messages_written_total: Default::default(),
                messages_dropped_total: AtomicU64::new(0),
                send_queue_depth: AtomicU64::new(0),
                send_queue_depth_max: AtomicU64::new(0),
                command_queue_depth: AtomicU64::new(0),
                command_queue_depth_max: AtomicU64::new(0),
                protocol_violations_total: AtomicU64::new(0),
                commands_dropped_over_tick_cap_total: AtomicU64::new(0),
                upgrade_rejected_total: Default::default(),
                accepting_connections: AtomicBool::new(true),
                persisted_total: Arc::new(AtomicU64::new(0)),
                persist_failed_total: Arc::new(AtomicU64::new(0)),
                last_committed_tick: Arc::new(AtomicU64::new(0)),
                data_dir: RwLock::new(String::new()),
                ship_classes_loaded: AtomicU64::new(0),
                spawn_points_loaded: AtomicU64::new(0),
                snapshot_interval_ticks: AtomicU64::new(0),
                world_capacity: AtomicU64::new(u64::MAX),
                snapshots_sent_total: AtomicU64::new(0),
                snapshot_bytes_total: AtomicU64::new(0),
                send_queue_bytes: AtomicU64::new(0),
                send_queue_bytes_max: AtomicU64::new(0),
                input_superseded_total: AtomicU64::new(0),
                input_carried_forward_total: AtomicU64::new(0),
                aim_degenerate_total: AtomicU64::new(0),
                ships_active: AtomicU64::new(0),
                ships_lingering: AtomicU64::new(0),
                snapshot_build_us: Histogram::default(),
                snapshot_over_capacity_total: AtomicU64::new(0),
                actors_with_ships: RwLock::new(std::collections::HashSet::new()),
            }),
        }
    }

    /// 기동 시 `data/` 로딩 결과를 한 번 기록한다(SC-05). 그 뒤로는 바뀌지 않는다.
    pub fn set_data_loaded(
        &self,
        data_dir: &str,
        ship_classes: u64,
        spawn_points: u64,
        snapshot_interval_ticks: u64,
        world_capacity: u64,
    ) {
        if let Ok(mut guard) = self.inner.data_dir.write() {
            *guard = data_dir.to_owned();
        }
        self.inner
            .ship_classes_loaded
            .store(ship_classes, Ordering::Relaxed);
        self.inner
            .spawn_points_loaded
            .store(spawn_points, Ordering::Relaxed);
        self.inner
            .snapshot_interval_ticks
            .store(snapshot_interval_ticks, Ordering::Relaxed);
        self.inner
            .world_capacity
            .store(world_capacity, Ordering::Relaxed);
    }

    /// 지금 세계에 있는 함선 수(활성 + 잔류). `world_full` 판정에 쓴다.
    #[must_use]
    pub fn ships_total(&self) -> u64 {
        self.inner.ships_active.load(Ordering::Relaxed)
            + self.inner.ships_lingering.load(Ordering::Relaxed)
    }

    /// `WORLD_SNAPSHOT.ships` 의 계약 상한.
    #[must_use]
    pub fn world_capacity(&self) -> u64 {
        self.inner.world_capacity.load(Ordering::Relaxed)
    }

    /// tick 재개 지점을 알린다. `last_committed_tick` 도 여기서 맞춘다.
    pub fn set_start_tick(&self, start_tick: u64) {
        self.inner.start_tick.store(start_tick, Ordering::Relaxed);
        self.inner
            .last_committed_tick
            .store(start_tick.saturating_sub(1), Ordering::Relaxed);
    }

    /// 영속화 태스크에 넘길 원자값 묶음.
    #[must_use]
    pub fn persist_handles(&self) -> (Arc<AtomicU64>, Arc<AtomicU64>, Arc<AtomicU64>) {
        (
            Arc::clone(&self.inner.persisted_total),
            Arc::clone(&self.inner.persist_failed_total),
            Arc::clone(&self.inner.last_committed_tick),
        )
    }

    /// tick 1회의 결과를 기록한다.
    pub fn record_tick(&self, tick: u64, body_us: u64, lag_micros: i64) {
        self.inner.tick.store(tick, Ordering::Release);
        self.inner.tick_started.store(true, Ordering::Release);
        self.inner.tick_total.fetch_add(1, Ordering::Relaxed);
        self.inner.tick_body.record(body_us);
        if body_us > TICK_OVERRUN_US {
            self.inner
                .tick_overrun_total
                .fetch_add(1, Ordering::Relaxed);
        }
        self.inner
            .tick_lag_micros
            .store(lag_micros, Ordering::Relaxed);
    }

    /// 마지막으로 완료한 tick. 아직 한 번도 돌지 않았으면 `None`.
    #[must_use]
    pub fn last_tick(&self) -> Option<u64> {
        self.inner
            .tick_started
            .load(Ordering::Acquire)
            .then(|| self.inner.tick.load(Ordering::Acquire))
    }

    /// 지금 tick 번호 (아직 안 돌았으면 `start_tick`).
    #[must_use]
    pub fn current_tick(&self) -> u64 {
        self.last_tick()
            .unwrap_or_else(|| self.inner.start_tick.load(Ordering::Relaxed))
    }

    /// 영속화 백로그 = 현재 tick − 마지막 커밋 tick (ADR-0007 §4).
    #[must_use]
    pub fn persist_backlog(&self) -> u64 {
        self.current_tick()
            .saturating_sub(self.inner.last_committed_tick.load(Ordering::Acquire))
    }

    /// 세션 레지스트리의 **실제 길이**를 기록한다 (I-25).
    pub fn set_ws_connections(&self, count: u64) {
        self.inner.ws_connections.store(count, Ordering::Relaxed);
    }

    /// 업그레이드된 소켓 하나가 시작됐다.
    pub fn connection_opened(&self) {
        self.inner.live_connections.fetch_add(1, Ordering::AcqRel);
    }

    /// 업그레이드된 소켓 하나가 끝났다(Close 프레임 전송까지 마쳤다).
    pub fn connection_closed(&self) {
        let _ = self.inner.live_connections.fetch_update(
            Ordering::AcqRel,
            Ordering::Acquire,
            |value| Some(value.saturating_sub(1)),
        );
    }

    /// 아직 살아 있는 소켓 수.
    ///
    /// `ws_connections`(tick 드라이버의 라우팅 표 길이)와 **다른 것을 센다.** 종료 스윕은
    /// 라우팅 표를 먼저 비우므로 `ws_connections` 는 즉시 0이 되지만, 송신 태스크가 Close
    /// 프레임을 내보내는 동안 소켓은 아직 살아 있다. 정상 종료가 이 값을 기다리지 않으면
    /// 프로세스가 먼저 죽어 **클라이언트가 close code 를 보지 못한다**(실측 2026-09-19).
    #[must_use]
    pub fn live_connections(&self) -> u64 {
        self.inner.live_connections.load(Ordering::Acquire)
    }

    /// 세션 열림/닫힘을 센다.
    pub fn add_sessions(&self, opened: u64, closed: u64) {
        if opened > 0 {
            self.inner
                .sessions_opened_total
                .fetch_add(opened, Ordering::Relaxed);
        }
        if closed > 0 {
            self.inner
                .sessions_closed_total
                .fetch_add(closed, Ordering::Relaxed);
        }
    }

    /// 명령 수신을 센다 (큐에 넣기 전, 파싱 성공 시점).
    pub fn record_command_received(&self) {
        self.inner
            .commands_received_total
            .fetch_add(1, Ordering::Relaxed);
    }

    /// 거부를 사유별로 센다.
    pub fn record_rejection(&self, reason: RejectReasonCode) {
        self.inner.commands_rejected_total[reason_index(reason)].fetch_add(1, Ordering::Relaxed);
    }

    /// 송신 큐에 넣은 메시지를 센다.
    pub fn record_message_enqueued(&self, type_name: &str) {
        self.inner.messages_enqueued_total[message_index(type_name)]
            .fetch_add(1, Ordering::Relaxed);
    }

    /// 소켓에 실제로 쓴 메시지를 센다.
    pub fn record_message_written(&self, type_name: &str) {
        self.inner.messages_written_total[message_index(type_name)].fetch_add(1, Ordering::Relaxed);
    }

    /// 연결 종료로 버려진 메시지를 센다. **손실의 유일한 정당한 출처다.**
    pub fn record_messages_dropped(&self, count: u64) {
        if count > 0 {
            self.inner
                .messages_dropped_total
                .fetch_add(count, Ordering::Relaxed);
        }
    }

    /// `WORLD_SNAPSHOT` 을 소켓에 실제로 쓴 시점에 함께 센다(SC-33·SC-71) — **큐에 넣은
    /// 시점이 아니다.** `record_message_written("WORLD_SNAPSHOT")` 과 같은 자리
    /// (`ws.rs` 의 `writer_loop`)에서만 부른다. `bytes` 는 소켓에 넘긴 바로 그 UTF-8
    /// 바이트 길이라 봇이 수신 프레임에서 잰 길이와 같은 것을 센다(프레이밍 오버헤드
    /// 제외 — 20 % 판정에 영향 없는 수준임을 architect 지시 4가 확인했다).
    pub fn record_snapshot_written(&self, bytes: u64) {
        self.inner
            .snapshots_sent_total
            .fetch_add(1, Ordering::Relaxed);
        self.inner
            .snapshot_bytes_total
            .fetch_add(bytes, Ordering::Relaxed);
    }

    /// tick 드라이버가 매 tick `TickOutcome` 의 델타를 여기로 더한다.
    pub fn add_input_superseded(&self, count: u64) {
        if count > 0 {
            self.inner
                .input_superseded_total
                .fetch_add(count, Ordering::Relaxed);
        }
    }

    /// 〃 — `input_carried_forward_total`.
    pub fn add_input_carried_forward(&self, count: u64) {
        if count > 0 {
            self.inner
                .input_carried_forward_total
                .fetch_add(count, Ordering::Relaxed);
        }
    }

    /// 〃 — `aim_degenerate_total`.
    pub fn add_aim_degenerate(&self, count: u64) {
        if count > 0 {
            self.inner
                .aim_degenerate_total
                .fetch_add(count, Ordering::Relaxed);
        }
    }

    /// 함선 존재 게이지. **카운터 뺄셈이 아니라 tick 드라이버가 매 tick 월드에서 직접 읽어
    /// `store` 한다**(I-25 — `ws_connections` 와 같은 방식).
    pub fn set_ship_presence(&self, active: u64, lingering: u64) {
        self.inner.ships_active.store(active, Ordering::Relaxed);
        self.inner
            .ships_lingering
            .store(lingering, Ordering::Relaxed);
    }

    /// 함선을 가진 actor 집합을 통째로 덮어쓴다. **카운터가 아니라 tick 드라이버가 매
    /// tick `Simulation::actors_with_ships()` 로 직접 읽어 넣는 실제 집합의 스냅샷이다**
    /// (I-25 와 같은 취급, `ships_active`/`ships_lingering` 참고). `world_full` 게이트의
    /// 재개 면제(I-44, S9)에 쓴다.
    pub fn set_actors_with_ships(
        &self,
        actors: impl Iterator<Item = starfall_contracts::primitives::UuidV7>,
    ) {
        if let Ok(mut guard) = self.inner.actors_with_ships.write() {
            guard.clear();
            guard.extend(actors);
        }
    }

    /// 이 actor가 지금 함선을 가졌는가(활성이든 잔류든) — `world_full` 게이트가 재개를
    /// 면제할지 판단하는 데 쓴다(I-44, S9).
    #[must_use]
    pub fn actor_has_ship(&self, actor_id: starfall_contracts::primitives::UuidV7) -> bool {
        self.inner
            .actors_with_ships
            .read()
            .is_ok_and(|guard| guard.contains(&actor_id))
    }

    /// 스냅샷 구조체 조립에 걸린 시간(마이크로초) — `tick_body_us` 와 분리해 기록한다
    /// (M-1, server B-14). 판정 대상이 아니라 기록용이다.
    pub fn record_snapshot_build(&self, micros: u64) {
        self.inner.snapshot_build_us.record(micros);
    }

    /// 스냅샷의 `ships` 배열이 계약 상한을 넘은 tick 수. **0이어야 정상이다** — 0이 아니면
    /// `world_full` 입장 제한이 뚫린 서버 버그다.
    pub fn add_snapshot_over_capacity(&self, count: u64) {
        if count > 0 {
            tracing::error!(
                count,
                "SC-05/world_full 위반 — WORLD_SNAPSHOT.ships 가 계약 상한을 넘었다"
            );
            self.inner
                .snapshot_over_capacity_total
                .fetch_add(count, Ordering::Relaxed);
        }
    }

    /// 송신 큐 점유 슬롯 합 (채널 상태에서 읽은 값 — 카운터 뺄셈이 아니다).
    pub fn set_send_queue_depth(&self, depth: u64) {
        self.inner.send_queue_depth.store(depth, Ordering::Relaxed);
        self.inner
            .send_queue_depth_max
            .fetch_max(depth, Ordering::Relaxed);
    }

    /// 송신 큐에 들어 있는 메시지들의 바이트 합 (SC-33). `send_queue_depth` 와 같은 방식
    /// (채널 상태 스냅샷, 카운터 뺄셈 아님) — 정확한 바이트가 아니라 **추정치**다(큐에
    /// 든 `ServerMessage` 를 실제 직렬화하지 않고 타입별 평균 크기로 추정한다. 직렬화까지
    /// 하면 이 값을 재는 행위 자체가 tick 본문 바깥에서도 비용을 만든다).
    pub fn set_send_queue_bytes(&self, bytes: u64) {
        self.inner.send_queue_bytes.store(bytes, Ordering::Relaxed);
        self.inner
            .send_queue_bytes_max
            .fetch_max(bytes, Ordering::Relaxed);
    }

    /// 전역 명령 큐 깊이.
    pub fn set_command_queue_depth(&self, depth: u64) {
        self.inner
            .command_queue_depth
            .store(depth, Ordering::Relaxed);
        self.inner
            .command_queue_depth_max
            .fetch_max(depth, Ordering::Relaxed);
    }

    /// 프로토콜 위반 1건. **거부(rejection)는 위반이 아니다.**
    pub fn record_protocol_violation(&self) {
        self.inner
            .protocol_violations_total
            .fetch_add(1, Ordering::Relaxed);
    }

    /// tick당 명령 상한을 넘겨 판정에 넣지 못한 명령 1건(S10, ADR-0011 §5.2).
    pub fn record_command_dropped_over_tick_cap(&self) {
        self.inner
            .commands_dropped_over_tick_cap_total
            .fetch_add(1, Ordering::Relaxed);
    }

    /// 업그레이드 거절 1건을 사유별로 센다.
    ///
    /// **인증 거부의 증거는 전적으로 서버 쪽에서 나와야 한다** — 브라우저·Mono 의 WebSocket
    /// 클라이언트는 업그레이드 실패의 HTTP 상태 코드에 접근하지 못한다(client 실측:
    /// 에러 코드가 `Success` 로 온다). 이 카운터가 AC-5(b) 의 독립 출처다.
    pub fn record_upgrade_rejection(&self, rejection: crate::ws::UpgradeRejection) {
        let index = UPGRADE_REJECTIONS
            .iter()
            .position(|label| *label == rejection.reason())
            .unwrap_or(0);
        self.inner.upgrade_rejected_total[index].fetch_add(1, Ordering::Relaxed);
    }

    /// 새 연결을 받는 중인가.
    #[must_use]
    pub fn is_accepting(&self) -> bool {
        self.inner.accepting_connections.load(Ordering::Acquire)
    }

    /// 새 연결 수락 여부를 바꾼다 (종료 시작 시 false).
    pub fn set_accepting(&self, accepting: bool) {
        self.inner
            .accepting_connections
            .store(accepting, Ordering::Release);
    }

    fn snapshot(&self) -> StatsBody {
        let inner = &self.inner;
        let start_tick = inner.start_tick.load(Ordering::Relaxed);
        let tick = self.current_tick();
        StatsBody {
            start_tick,
            tick,
            tick_total: inner.tick_total.load(Ordering::Relaxed),
            tick_overrun_total: inner.tick_overrun_total.load(Ordering::Relaxed),
            tick_overrun_threshold_us: TICK_OVERRUN_US,
            tick_body_us: inner.tick_body.snapshot(),
            tick_lag_seconds: inner.tick_lag_micros.load(Ordering::Relaxed) as f64 / 1e6,
            ws_connections: inner.ws_connections.load(Ordering::Relaxed),
            live_connections: inner.live_connections.load(Ordering::Acquire),
            sessions_opened_total: inner.sessions_opened_total.load(Ordering::Relaxed),
            sessions_closed_total: inner.sessions_closed_total.load(Ordering::Relaxed),
            commands_received_total: inner.commands_received_total.load(Ordering::Relaxed),
            commands_rejected_total: labelled(&REJECT_REASONS, &inner.commands_rejected_total),
            messages_enqueued_total: labelled(&MESSAGE_TYPES, &inner.messages_enqueued_total),
            messages_written_total: labelled(&MESSAGE_TYPES, &inner.messages_written_total),
            messages_enqueued_all: sum(&inner.messages_enqueued_total),
            messages_written_all: sum(&inner.messages_written_total),
            messages_dropped_total: inner.messages_dropped_total.load(Ordering::Relaxed),
            send_queue_depth: inner.send_queue_depth.load(Ordering::Relaxed),
            send_queue_depth_max: inner.send_queue_depth_max.load(Ordering::Relaxed),
            command_queue_depth: inner.command_queue_depth.load(Ordering::Relaxed),
            command_queue_depth_max: inner.command_queue_depth_max.load(Ordering::Relaxed),
            protocol_violations_total: inner.protocol_violations_total.load(Ordering::Relaxed),
            commands_dropped_over_tick_cap_total: inner
                .commands_dropped_over_tick_cap_total
                .load(Ordering::Relaxed),
            upgrade_rejected_total: labelled(&UPGRADE_REJECTIONS, &inner.upgrade_rejected_total),
            upgrade_rejected_all: sum(&inner.upgrade_rejected_total),
            domain_events_persisted_total: inner.persisted_total.load(Ordering::Relaxed),
            domain_events_persist_failed_total: inner.persist_failed_total.load(Ordering::Relaxed),
            last_committed_tick: inner.last_committed_tick.load(Ordering::Acquire),
            persist_backlog: self.persist_backlog(),
            persist_backlog_limit: crate::runtime::PERSIST_BACKLOG_LIMIT,
            accepting_connections: self.is_accepting(),
            data_dir: inner
                .data_dir
                .read()
                .map(|guard| guard.clone())
                .unwrap_or_default(),
            ship_classes_loaded: inner.ship_classes_loaded.load(Ordering::Relaxed),
            spawn_points_loaded: inner.spawn_points_loaded.load(Ordering::Relaxed),
            snapshot_interval_ticks: inner.snapshot_interval_ticks.load(Ordering::Relaxed),
            snapshots_sent_total: inner.snapshots_sent_total.load(Ordering::Relaxed),
            snapshot_bytes_total: inner.snapshot_bytes_total.load(Ordering::Relaxed),
            send_queue_bytes: inner.send_queue_bytes.load(Ordering::Relaxed),
            send_queue_bytes_max: inner.send_queue_bytes_max.load(Ordering::Relaxed),
            input_superseded_total: inner.input_superseded_total.load(Ordering::Relaxed),
            input_carried_forward_total: inner.input_carried_forward_total.load(Ordering::Relaxed),
            aim_degenerate_total: inner.aim_degenerate_total.load(Ordering::Relaxed),
            ships_active: inner.ships_active.load(Ordering::Relaxed),
            ships_lingering: inner.ships_lingering.load(Ordering::Relaxed),
            snapshot_build_us: inner.snapshot_build_us.snapshot(),
            snapshot_over_capacity_total: inner
                .snapshot_over_capacity_total
                .load(Ordering::Relaxed),
        }
    }
}

fn labelled(labels: &[&'static str], values: &[AtomicU64]) -> Vec<LabelledCount> {
    labels
        .iter()
        .zip(values)
        .map(|(label, value)| LabelledCount {
            label,
            count: value.load(Ordering::Relaxed),
        })
        .collect()
}

fn sum(values: &[AtomicU64]) -> u64 {
    values
        .iter()
        .map(|value| value.load(Ordering::Relaxed))
        .sum()
}

/// 라벨이 붙은 카운터 1건.
#[derive(Debug, Clone, Serialize)]
pub struct LabelledCount {
    /// 라벨(타입 이름 또는 거부 사유).
    pub label: &'static str,
    /// 값.
    pub count: u64,
}

/// tick 본문 소요 분포.
#[derive(Debug, Clone, Serialize)]
pub struct HistogramBody {
    /// 표본 수.
    pub count: u64,
    /// 합 (마이크로초).
    pub sum_us: u64,
    /// 관측 최댓값 (마이크로초). **정확한 값이다.**
    pub max_us: u64,
    /// p50 이 속한 버킷의 상한.
    pub p50_le_us: u64,
    /// p90 이 속한 버킷의 상한.
    pub p90_le_us: u64,
    /// p99 이 속한 버킷의 상한.
    pub p99_le_us: u64,
    /// 버킷 상한 목록.
    pub bucket_bounds_us: Vec<u64>,
    /// 버킷별 도수 (마지막 칸은 `+inf`).
    pub buckets: Vec<u64>,
}

/// `/debug/stats` 응답 본문. **계약이 아니다.**
#[derive(Debug, Clone, Serialize)]
pub struct StatsBody {
    /// 이 프로세스가 시작한 tick (ADR-0006 §2.3 의 재개 지점).
    pub start_tick: u64,
    /// 마지막으로 완료한 tick.
    pub tick: u64,
    /// 이 프로세스가 실행한 tick 수. `tick_total == tick − start_tick + 1` 이어야 한다 (I-17).
    pub tick_total: u64,
    /// 본문 소요가 임계를 넘은 tick 수.
    pub tick_overrun_total: u64,
    /// 초과 판정 임계 (마이크로초).
    pub tick_overrun_threshold_us: u64,
    /// tick **본문** 소요 분포 (루프 주기가 아니다).
    pub tick_body_us: HistogramBody,
    /// 루프 주기 드리프트(초). 이 PC 의 바닥값은 전용 OS 스레드 기준 약 +0.7 %/분이다.
    pub tick_lag_seconds: f64,
    /// 지금 열려 있는 세션 수 — **라우팅 표의 실제 길이**.
    pub ws_connections: u64,
    /// 아직 살아 있는 업그레이드된 소켓 수. 종료 중에는 `ws_connections` 보다 잠깐 크다.
    pub live_connections: u64,
    /// `SESSION_OPENED` 발행 누적.
    pub sessions_opened_total: u64,
    /// `SESSION_CLOSED` 발행 누적.
    pub sessions_closed_total: u64,
    /// 파싱에 성공해 큐에 넣으려 시도한 명령 누적.
    pub commands_received_total: u64,
    /// 사유별 거부 누적.
    pub commands_rejected_total: Vec<LabelledCount>,
    /// 타입별 송신 큐 투입 누적.
    pub messages_enqueued_total: Vec<LabelledCount>,
    /// 타입별 소켓 기록 누적.
    pub messages_written_total: Vec<LabelledCount>,
    /// 송신 큐 투입 합계.
    pub messages_enqueued_all: u64,
    /// 소켓 기록 합계.
    pub messages_written_all: u64,
    /// 연결 종료로 버려진 메시지 누적.
    pub messages_dropped_total: u64,
    /// 송신 큐 점유 슬롯 합 (채널 상태).
    pub send_queue_depth: u64,
    /// 그 최고 수위.
    pub send_queue_depth_max: u64,
    /// 전역 명령 큐 깊이.
    pub command_queue_depth: u64,
    /// 그 최고 수위.
    pub command_queue_depth_max: u64,
    /// 프로토콜 위반 누적 (거부는 포함하지 않는다).
    pub protocol_violations_total: u64,
    /// tick당 명령 상한을 넘겨 판정에 넣지 못한 명령 누적(S10, ADR-0011 §5.2). `COMMAND_RESULT`가
    /// 없는 손실이다 — QA의 손실 항등식은 `보낸 수 = COMMAND_RESULT 수 + 이 값`이 된다.
    pub commands_dropped_over_tick_cap_total: u64,
    /// 사유별 업그레이드 거절 누적 (401·503). 인증 거부의 서버 쪽 증거다.
    pub upgrade_rejected_total: Vec<LabelledCount>,
    /// 업그레이드 거절 합계.
    pub upgrade_rejected_all: u64,
    /// 커밋된 도메인 이벤트 누적.
    pub domain_events_persisted_total: u64,
    /// 제약 위반으로 버린 이벤트 누적. **0이 아니면 버그다.**
    pub domain_events_persist_failed_total: u64,
    /// 마지막으로 커밋된 tick.
    pub last_committed_tick: u64,
    /// `tick − last_committed_tick`.
    pub persist_backlog: u64,
    /// 이 값을 넘으면 새 연결을 거절한다.
    pub persist_backlog_limit: u64,
    /// 새 연결을 받는 중인가.
    pub accepting_connections: bool,
    /// 해석된 `data/` 절대 경로(p1-01 SC-05·SC-06).
    pub data_dir: String,
    /// 로드된 함선 클래스 수.
    pub ship_classes_loaded: u64,
    /// 로드된 스폰 지점 수.
    pub spawn_points_loaded: u64,
    /// 유도된 `snapshot_interval_ticks`(`tick_hz / snapshot_hz`).
    pub snapshot_interval_ticks: u64,
    /// 소켓에 쓴 `WORLD_SNAPSHOT` 수 — `messages_written_total` 의 `WORLD_SNAPSHOT` 라벨과
    /// 같은 수여야 한다(교차 검증용 중복, SC-33).
    pub snapshots_sent_total: u64,
    /// 위와 같은 자리에서 센 페이로드 바이트 합.
    pub snapshot_bytes_total: u64,
    /// 지금 송신 큐에 들어 있는 메시지들의 바이트 합(추정치).
    pub send_queue_bytes: u64,
    /// 위의 최고 수위.
    pub send_queue_bytes_max: u64,
    /// 한 tick에 2건 이상 도착해 덮어써진 입력 수.
    pub input_superseded_total: u64,
    /// 도착 0건이라 직전 입력을 이월한 함선-tick 수.
    pub input_carried_forward_total: u64,
    /// 목표 쿼터니언 노름이 퇴화해 현재 자세로 대체한 횟수.
    pub aim_degenerate_total: u64,
    /// 지금 활성 함선 수(게이지 — I-25, 카운터 뺄셈이 아니다).
    pub ships_active: u64,
    /// 지금 잔류 함선 수.
    pub ships_lingering: u64,
    /// 스냅샷 구조체 조립 소요(마이크로초) — `tick_body_us` 와 분리된 기록(M-1).
    pub snapshot_build_us: HistogramBody,
    /// `WORLD_SNAPSHOT.ships` 가 계약 상한을 넘은 tick 누적. **0이어야 한다.**
    pub snapshot_over_capacity_total: u64,
}

/// `/debug/stats` 핸들러.
pub async fn debug_stats(State(state): State<AppState>) -> Json<StatsBody> {
    Json(state.stats.snapshot())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tick_identity_holds_after_steps() {
        let stats = Stats::new();
        stats.set_start_tick(4_242);
        assert_eq!(stats.last_tick(), None);
        assert_eq!(stats.current_tick(), 4_242);

        for tick in 4_242..4_252u64 {
            stats.record_tick(tick, 1_000, 0);
        }
        let body = stats.snapshot();
        assert_eq!(body.start_tick, 4_242);
        assert_eq!(body.tick, 4_251);
        assert_eq!(
            body.tick_total,
            body.tick - body.start_tick + 1,
            "I-17 항등식"
        );
    }

    #[test]
    fn overrun_counts_only_body_time_above_threshold() {
        let stats = Stats::new();
        stats.record_tick(0, TICK_OVERRUN_US, 0);
        stats.record_tick(1, TICK_OVERRUN_US + 1, 0);
        assert_eq!(stats.snapshot().tick_overrun_total, 1);
    }

    #[test]
    fn histogram_reports_exact_max_and_bucket_quantiles() {
        let stats = Stats::new();
        for value in [50u64, 150, 250, 3_000, 120_000] {
            stats.record_tick(0, value, 0);
        }
        let body = stats.snapshot().tick_body_us;
        assert_eq!(body.count, 5);
        assert_eq!(body.max_us, 120_000);
        assert_eq!(body.buckets.len(), BUCKET_BOUNDS_US.len() + 1);
        assert!(body.p99_le_us >= 120_000);
    }

    #[test]
    fn backlog_is_current_tick_minus_last_commit() {
        let stats = Stats::new();
        stats.set_start_tick(100);
        assert_eq!(stats.persist_backlog(), 1, "start 직후 = tick − (start−1)");
        stats.record_tick(700, 100, 0);
        assert_eq!(stats.persist_backlog(), 601);
        let (_, _, last) = stats.persist_handles();
        last.store(700, Ordering::Release);
        assert_eq!(stats.persist_backlog(), 0);
    }

    #[test]
    fn message_accounting_identity() {
        let stats = Stats::new();
        stats.record_message_enqueued("COMMAND_RESULT");
        stats.record_message_enqueued("PING_REPLY");
        stats.record_message_written("COMMAND_RESULT");
        stats.record_messages_dropped(1);
        let body = stats.snapshot();
        // enqueued == written + dropped + depth (정지 시점에는 depth 0)
        assert_eq!(
            body.messages_enqueued_all,
            body.messages_written_all + body.messages_dropped_total + body.send_queue_depth
        );
    }
}

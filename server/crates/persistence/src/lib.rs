//! PostgreSQL 영속화 (ADR-0007).
//!
//! # 여기서 지키는 네 가지
//!
//! 1. **tick 1회 = 트랜잭션 1회.** 부분 기록된 tick 이 생기지 않는다. 같은 트랜잭션에서
//!    `worlds.last_tick` 도 갱신하므로 tick 재개의 정본이 이벤트와 어긋날 수 없다.
//! 2. **이벤트를 버리지 않는다** (I-22). DB 가 느리거나 죽으면 tick 루프는 제출을 미루고
//!    `persist_backlog` 가 자란다. 임계를 넘으면 게이트웨이가 **새 연결을 거절**한다.
//! 3. **`recorded_at` 은 여기서 채운다.** tick 안에서는 시계를 읽지 않는다 (ADR-0006 §2).
//! 4. **`UNIQUE (world_id, tick, sequence)` 위반은 버그 신호다.** 삼키지 않는다.
//!
//! # 컴파일 타임 쿼리 검사를 쓰지 않는다 (ADR-0007 §5)
//!
//! `query!`/`query_as!` 는 빌드 시 DB 접속이나 `.sqlx/` 오프라인 메타데이터를 요구하고,
//! 그 메타데이터는 `sqlx-cli` 로 만드는데 이 PC 에 설치되어 있지 않다. 쿼리가 4종뿐인
//! 지금은 도구 설치·동기화 규율의 비용이 이득보다 크다. 대신 **명시적 바인딩 타입**을 쓰고
//! 실제 DB 를 쓰는 통합 테스트로 검증한다. 재평가 시점은 쿼리 10개 초과 또는 CI 도입이다.

pub mod history;

/// DB 통합 테스트 전용 주입점(`test-hooks` feature, Cargo.toml 참고).
///
/// # `cargo build -p starfall-game-server`(릴리스·단독 `-p`) 에는 안 들어간다 — 그런데
///
/// **`cargo test --workspace`/`--all-targets` 뒤에는 들어간다**(qa 실측, 2026-09-29).
/// 이 크레이트의 `[dev-dependencies]` 자기 참조(`features = ["test-hooks"]`)가
/// Cargo 의 feature 통합 규칙 때문에 **같은 프로파일(debug)의 다른 타깃**에도 번진다
/// — `bins/game-server` 를 워크스페이스 테스트와 같은 `cargo` 호출로 다시 빌드하면
/// `target/debug/starfall-game-server` 실행 파일에 이 feature 가 켜진 채로 들어간다.
/// **무장(arm)되지 않은 상태에서는 커밋 로직이 바이트 단위로 같다**(각 주입 지점의
/// 무해성 근거는 그 지점 주석 참고 — 전부 "카운터가 0이면 원자 읽기만 하고 아무 것도
/// 안 한다"는 모양이다) — 기능 결함은 아니다. 그래도 **증거용 실서버 판정은 이 것에
/// 기대지 않는다**: qa 의 `server_boot.py` 가 기동 전 실행 파일에서 이 모듈의 문자열을
/// 검색해 있으면 거부한다(exit 4). 증거를 낼 때는 `cargo build -p starfall-game-server`
/// (워크스페이스 전체 타깃을 같이 빌드하지 않는 단독 `-p`)로 다시 빌드한다.
///
/// # 왜 전역 플래그인가
///
/// `commit`·`now_real_time`·`extract_payload` 는 private 함수라 별도 크레이트인
/// `tests/*.rs` 통합 테스트에서 직접 주입할 수 없다. `run()` 이 유일한 공개 진입점이라,
/// "다음 호출 한 번만 실패시킨다"는 프로세스 전역 플래그로 표현한다.
///
/// # 병렬 테스트 경합
///
/// 이 플래그들은 전역이다. `INJECTION_LOCK` 을 먼저 잡지 않고 이 모듈을 쓰는 테스트를
/// 다른 테스트와 동시에 돌리면 서로의 주입을 밟는다 — 이 모듈을 쓰는 테스트는 반드시
/// `let _guard = test_hooks::INJECTION_LOCK.lock()...` 을 테스트 본문 맨 앞에서 잡는다.
#[cfg(feature = "test-hooks")]
pub mod test_hooks {
    use std::sync::atomic::{AtomicU32, Ordering};

    /// 이 모듈의 전역 플래그를 쓰는 테스트가 공유하는 락. 테스트 병렬 실행이 서로의
    /// 주입을 밟지 않게 한다(다른 테스트 파일의 DB 상태 격리는 `TestDb::create` 가
    /// 이미 보장하지만, 이 프로세스 전역 플래그는 DB 격리 밖이다). `tokio::sync::Mutex`
    /// 를 쓴다 — 테스트 본문이 이 가드를 쥔 채 여러 `.await` 를 건너므로
    /// `std::sync::Mutex` 는 `clippy::await_holding_lock`(워크스페이스 게이트, `-D
    /// warnings`)에 걸린다.
    pub static INJECTION_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

    pub(crate) static FAIL_CLOCK_REMAINING: AtomicU32 = AtomicU32::new(0);
    pub(crate) static FAIL_CLOCK_HIT_COUNT: AtomicU32 = AtomicU32::new(0);
    pub(crate) static FAIL_PAYLOAD_REMAINING: AtomicU32 = AtomicU32::new(0);
    pub(crate) static FAIL_PAYLOAD_HIT_COUNT: AtomicU32 = AtomicU32::new(0);
    pub(crate) static FORCE_TRANSIENT_REMAINING: AtomicU32 = AtomicU32::new(0);
    pub(crate) static FORCE_TRANSIENT_HIT_COUNT: AtomicU32 = AtomicU32::new(0);
    pub(crate) static FORCE_FATAL_REMAINING: AtomicU32 = AtomicU32::new(0);
    pub(crate) static FORCE_FATAL_HIT_COUNT: AtomicU32 = AtomicU32::new(0);
    pub(crate) static FORCE_APPARENT_FAILURE_REMAINING: AtomicU32 = AtomicU32::new(0);
    pub(crate) static FORCE_APPARENT_FAILURE_HIT_COUNT: AtomicU32 = AtomicU32::new(0);
    pub(crate) static TRANSIENT_HIT_COUNT: AtomicU32 = AtomicU32::new(0);
    /// `commit()` 이 트랜잭션을 여는 시점마다 +1(SC-112,
    /// `terminated_backend_is_retried`) — pg_stat_activity 를 폴링해 "지금 트랜잭션이
    /// 열려 있다" 순간을 노리는 것보다, 이 카운터가 바뀌는 순간을 노리는 편이 훨씬 더
    /// 자주 그 짧은 창을 맞힌다(문자열 매칭 폴링은 네트워크 왕복 지연 때문에 자주
    /// 놓쳤다 — 2026-09-29 실측).
    pub(crate) static TRANSACTION_ATTEMPT_COUNTER: AtomicU32 = AtomicU32::new(0);

    /// `commit_with_retry` 가 복구 불가로 정지를 결정할 때마다 그 `CommitError` 의
    /// `Display` 문자열을 담아 둔다(SC-25 — CAS 불일치 정지 로그의 `actual=` 필드를
    /// **로그 캡처 없이** 단언하기 위해서다, 팀 리더 지시 2026-09-30: "tracing 전역
    /// 캐시 문제"를 피한다). `tracing::error!` 의 `%error` 가 같은 `Display` 를 쓰므로
    /// 이 값이 실제 로그 줄과 항상 같다 — 별도로 문자열을 다시 조립하지 않는다.
    pub(crate) static LAST_FATAL_DETAIL: std::sync::Mutex<Option<String>> =
        std::sync::Mutex::new(None);

    /// 이 모듈의 모든 플래그를 0 으로 되돌린다. 테스트 시작마다(`INJECTION_LOCK` 을 잡은
    /// 뒤) 부른다 — 패닉으로 끝난 이전 테스트의 잔여 카운트를 물려받지 않기 위해서다.
    pub fn reset() {
        FAIL_CLOCK_REMAINING.store(0, Ordering::SeqCst);
        FAIL_CLOCK_HIT_COUNT.store(0, Ordering::SeqCst);
        FAIL_PAYLOAD_REMAINING.store(0, Ordering::SeqCst);
        FAIL_PAYLOAD_HIT_COUNT.store(0, Ordering::SeqCst);
        FORCE_TRANSIENT_REMAINING.store(0, Ordering::SeqCst);
        FORCE_TRANSIENT_HIT_COUNT.store(0, Ordering::SeqCst);
        FORCE_FATAL_REMAINING.store(0, Ordering::SeqCst);
        FORCE_FATAL_HIT_COUNT.store(0, Ordering::SeqCst);
        FORCE_APPARENT_FAILURE_REMAINING.store(0, Ordering::SeqCst);
        FORCE_APPARENT_FAILURE_HIT_COUNT.store(0, Ordering::SeqCst);
        TRANSIENT_HIT_COUNT.store(0, Ordering::SeqCst);
        TRANSACTION_ATTEMPT_COUNTER.store(0, Ordering::SeqCst);
        *LAST_FATAL_DETAIL
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
    }

    /// `commit_with_retry` 가 마지막으로 정지를 결정한 `CommitError` 의 `Display`
    /// 문자열. 이번 배치에서 아직 정지가 없었으면 `None`.
    #[must_use]
    pub fn last_fatal_detail() -> Option<String> {
        LAST_FATAL_DETAIL
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    pub(crate) fn record_fatal_detail(detail: String) {
        *LAST_FATAL_DETAIL
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(detail);
    }

    /// 다음 `now_real_time()` 호출을 `times` 번 실패(`None`)시킨다(SC-27,
    /// `recorded_at_failure_fails_batch`).
    pub fn fail_next_clock(times: u32) {
        FAIL_CLOCK_REMAINING.store(times, Ordering::SeqCst);
    }
    /// 실제로 [`fail_next_clock`] 주입이 걸린 횟수 — 0 이면 주입 지점이 실행되지 않은
    /// 것이라 그 테스트는 무효다(CLAUDE.md 검증 규율).
    #[must_use]
    pub fn clock_failure_hit_count() -> u32 {
        FAIL_CLOCK_HIT_COUNT.load(Ordering::SeqCst)
    }

    /// 다음 payload 직렬화를 `times` 번 실패시킨다(SC-101,
    /// `payload_serialize_failure_fails_batch`).
    pub fn fail_next_payload(times: u32) {
        FAIL_PAYLOAD_REMAINING.store(times, Ordering::SeqCst);
    }
    #[must_use]
    pub fn payload_failure_hit_count() -> u32 {
        FAIL_PAYLOAD_HIT_COUNT.load(Ordering::SeqCst)
    }

    /// 다음 커밋 **시도**를 `times` 번, 실제 DB 를 건드리지 않고 가짜 `57P01`
    /// (admin_shutdown, 일시 실패)로 실패시킨다(SC-103a, `admin_shutdown_is_transient`).
    pub fn force_transient_once(times: u32) {
        FORCE_TRANSIENT_REMAINING.store(times, Ordering::SeqCst);
    }
    #[must_use]
    pub fn transient_injection_hit_count() -> u32 {
        FORCE_TRANSIENT_HIT_COUNT.load(Ordering::SeqCst)
    }

    /// 다음 커밋 **시도**를 `times` 번, 실제 DB 를 건드리지 않고 가짜 `22P02`
    /// (허용 목록 밖 — 복구 불가)로 실패시킨다(SC-103 짝: "무엇이든 재시도"가 아님을
    /// 확인하는 반례, 같은 테스트가 [`force_transient_once`] 와 함께 쓴다).
    pub fn force_fatal_once(times: u32) {
        FORCE_FATAL_REMAINING.store(times, Ordering::SeqCst);
    }
    #[must_use]
    pub fn fatal_injection_hit_count() -> u32 {
        FORCE_FATAL_HIT_COUNT.load(Ordering::SeqCst)
    }

    /// 다음 **실제로 성공한** 커밋 `times` 번을, 호출자에게는 실패로 보이게 한다(모호한
    /// 커밋 재시도 재현, SC-20 짝 `ambiguous_commit_retry_counts_once`) — DB 에는 이미
    /// 커밋됐지만 응답이 유실된 것처럼 재시도를 강제한다.
    pub fn force_apparent_failure_after_next_success(times: u32) {
        FORCE_APPARENT_FAILURE_REMAINING.store(times, Ordering::SeqCst);
    }
    #[must_use]
    pub fn apparent_failure_hit_count() -> u32 {
        FORCE_APPARENT_FAILURE_HIT_COUNT.load(Ordering::SeqCst)
    }

    /// `commit_with_retry` 가 일시 오류로 분류해 재시도한 총 횟수 — 가짜 주입이든 진짜
    /// DB 오류(SC-112, `terminated_backend_is_retried`)든 전부 센다. "관찰한 일시
    /// 오류가 있다"는 증거가 필요한 두 테스트가 공유한다.
    #[must_use]
    pub fn transient_hit_count() -> u32 {
        TRANSIENT_HIT_COUNT.load(Ordering::SeqCst)
    }

    /// [`TRANSACTION_ATTEMPT_COUNTER`] 의 현재 값 — 이 값이 바뀌는 순간을 노려 끊으면
    /// (`terminated_backend_is_retried`) 문자열 매칭 폴링보다 훨씬 더 자주 트랜잭션
    /// 진행 중을 맞힌다.
    #[must_use]
    pub fn transaction_attempt_counter() -> u32 {
        TRANSACTION_ATTEMPT_COUNTER.load(Ordering::SeqCst)
    }

    /// [`force_transient_once`]/[`force_fatal_once`] 가 쓰는 가짜 오류. 실제 DB 를 전혀
    /// 건드리지 않는다 — `sqlx::error::DatabaseError` 를 손으로 구현해 SQLSTATE 만
    /// 흉내낸다(SC-103 요구: "`DatabaseError` 를 구현한 가짜 오류").
    #[derive(Debug)]
    pub(crate) struct FakeDbError(pub(crate) &'static str);

    impl std::fmt::Display for FakeDbError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "주입된 가짜 {} — test-hooks 전용", self.0)
        }
    }
    impl std::error::Error for FakeDbError {}
    impl sqlx::error::DatabaseError for FakeDbError {
        fn message(&self) -> &str {
            "test-hooks 주입"
        }
        fn code(&self) -> Option<std::borrow::Cow<'_, str>> {
            Some(std::borrow::Cow::Borrowed(self.0))
        }
        fn as_error(&self) -> &(dyn std::error::Error + Send + Sync + 'static) {
            self
        }
        fn as_error_mut(&mut self) -> &mut (dyn std::error::Error + Send + Sync + 'static) {
            self
        }
        fn into_error(self: Box<Self>) -> Box<dyn std::error::Error + Send + Sync + 'static> {
            self
        }
        fn kind(&self) -> sqlx::error::ErrorKind {
            sqlx::error::ErrorKind::Other
        }
    }

    /// `commit()` 이 실제 트랜잭션을 열기 전에 부른다 — [`FORCE_TRANSIENT_REMAINING`]
    /// (57P01) 을 먼저 보고, 없으면 [`FORCE_FATAL_REMAINING`](22P02) 을 본다. 어느
    /// 쪽이든 소비하면 가짜 `sqlx::Error` 를 돌려준다. 나머지 분류(`is_transient_db_error`)
    /// 는 평소와 똑같은 코드 경로를 탄다 — 주입이 별도 처리 분기를 만들지 않는다.
    pub(crate) fn take_forced_db_error() -> Option<sqlx::Error> {
        if let Some(err) = take_counter(
            &FORCE_TRANSIENT_REMAINING,
            &FORCE_TRANSIENT_HIT_COUNT,
            "57P01",
        ) {
            return Some(err);
        }
        take_counter(&FORCE_FATAL_REMAINING, &FORCE_FATAL_HIT_COUNT, "22P02")
    }

    fn take_counter(
        remaining: &AtomicU32,
        hit: &AtomicU32,
        code: &'static str,
    ) -> Option<sqlx::Error> {
        loop {
            let value = remaining.load(Ordering::SeqCst);
            if value == 0 {
                return None;
            }
            if remaining
                .compare_exchange(value, value - 1, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
            {
                hit.fetch_add(1, Ordering::SeqCst);
                return Some(sqlx::Error::Database(Box::new(FakeDbError(code))));
            }
        }
    }

    /// [`FORCE_APPARENT_FAILURE_REMAINING`] 이 남아 있으면 소비하고 `true` 를 돌려준다
    /// (실제로는 성공한 커밋을 실패로 보이게 할지).
    pub(crate) fn take_apparent_failure() -> bool {
        loop {
            let remaining = FORCE_APPARENT_FAILURE_REMAINING.load(Ordering::SeqCst);
            if remaining == 0 {
                return false;
            }
            if FORCE_APPARENT_FAILURE_REMAINING
                .compare_exchange(remaining, remaining - 1, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
            {
                FORCE_APPARENT_FAILURE_HIT_COUNT.fetch_add(1, Ordering::SeqCst);
                return true;
            }
        }
    }

    /// [`FAIL_CLOCK_REMAINING`] 이 남아 있으면 소비하고 `true`(이번 호출을 실패시킬지).
    pub(crate) fn take_clock_failure() -> bool {
        loop {
            let remaining = FAIL_CLOCK_REMAINING.load(Ordering::SeqCst);
            if remaining == 0 {
                return false;
            }
            if FAIL_CLOCK_REMAINING
                .compare_exchange(remaining, remaining - 1, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
            {
                FAIL_CLOCK_HIT_COUNT.fetch_add(1, Ordering::SeqCst);
                return true;
            }
        }
    }

    /// [`FAIL_PAYLOAD_REMAINING`] 이 남아 있으면 소비하고 `true`.
    pub(crate) fn take_payload_failure() -> bool {
        loop {
            let remaining = FAIL_PAYLOAD_REMAINING.load(Ordering::SeqCst);
            if remaining == 0 {
                return false;
            }
            if FAIL_PAYLOAD_REMAINING
                .compare_exchange(remaining, remaining - 1, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
            {
                FAIL_PAYLOAD_HIT_COUNT.fetch_add(1, Ordering::SeqCst);
                return true;
            }
        }
    }

    /// `commit_with_retry` 가 일시 오류를 만날 때마다 부른다(가짜·진짜 구분 없이).
    pub(crate) fn record_transient_hit() {
        TRANSIENT_HIT_COUNT.fetch_add(1, Ordering::SeqCst);
    }
}

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::Value;
use sqlx::postgres::{PgPool, PgPoolOptions};
use sqlx::{Row, Transaction};
use starfall_contracts::events::{
    MineralMinedEvent, MineralMinedType, SessionClosedEvent, SessionClosedType, SessionOpenedEvent,
    SessionOpenedType, ShipDespawnedEvent, ShipDespawnedType, ShipSpawnedEvent, ShipSpawnedType,
};
use starfall_contracts::primitives::{
    ConstSchemaVersion, DataId, GameCalendar, GameTime, RealTime, ServerVersion, UuidV7,
};
use starfall_sim::{DomainEventBody, PendingEvent, PersistBatch, StateWrite};
use tokio::sync::watch;

/// 영속화 실패.
#[derive(Debug, thiserror::Error)]
pub enum PersistenceError {
    /// 접속·쿼리 실패.
    #[error("PostgreSQL 오류")]
    Database(#[from] sqlx::Error),
    /// 마이그레이션 실패(체크섬 불일치 포함).
    #[error(
        "마이그레이션 적용 실패 — 마이그레이션 파일을 고쳤다면 `docker compose down -v && docker compose up -d` 가 필요하다"
    )]
    Migrate(#[from] sqlx::migrate::MigrateError),
    /// `worlds` 에 설정된 월드가 없다.
    #[error("기동 거부: worlds 에 world_id={world_id} 행이 없다 (마이그레이션 시드 확인)")]
    WorldMissing {
        /// 찾던 월드.
        world_id: UuidV7,
    },
    /// 설정과 `worlds` 행의 상수가 다르다 (I-19).
    #[error(
        "기동 거부: 월드 상수 불일치 — worlds.tick_hz={stored}, STARFALL_TICK_HZ={configured} (I-19, ADR-0006 §3)"
    )]
    TickHzMismatch {
        /// `worlds` 행의 값.
        stored: u32,
        /// 설정 값.
        configured: u32,
    },
    /// `worlds` 행의 값이 계약 범위를 벗어난다.
    #[error(
        "기동 거부: worlds 행의 달력 상수를 해석할 수 없다 (tick_hz={tick_hz}, epoch={epoch:?}, scale={scale})"
    )]
    WorldConstants {
        /// 저장된 tick 주기.
        tick_hz: i32,
        /// 저장된 달력 기준점.
        epoch: String,
        /// 저장된 축척.
        scale: i32,
    },
    /// 경제 상태 행이 계약 밖의 값을 담고 있다(있을 수 없는 상태 — 방어적. 쓰는 쪽은
    /// 항상 유효한 `DataId`/UUIDv7 만 쓴다).
    #[error("기동 거부: {table} 행이 깨졌다(world_id={world_id}): {detail}")]
    CorruptEconomicRow {
        /// 문제가 있던 테이블.
        table: &'static str,
        /// 그 월드.
        world_id: UuidV7,
        /// 무엇이 문제였는지.
        detail: String,
    },
}

/// 영속화 태스크가 갱신하는 관측 값.
///
/// 게이트웨이의 `/debug/stats` 가 **같은 `Arc` 를 읽는다.** 크레이트 의존이 생기지 않게
/// 원자값만 공유한다 (게이트웨이는 persistence 를 의존하지 않는다).
#[derive(Debug, Clone)]
pub struct PersistHandles {
    /// 커밋된 도메인 이벤트 누적 수.
    pub persisted_total: Arc<AtomicU64>,
    /// 복구 불가로 **버린** 배치의 이벤트 누적 수(`persist_fatal_total`). 0이 아니면
    /// 그 순간부터 `halted`도 true다.
    pub failed_total: Arc<AtomicU64>,
    /// 마지막으로 커밋된 tick. `persist_backlog = 현재 tick − 이 값`.
    pub last_committed_tick: Arc<AtomicU64>,
    /// 영속화가 정지했는가(K5, ADR-0013 §5). true가 되면 이후 어떤 배치도 커밋되지
    /// 않는다 — 종료 스윕의 `SESSION_CLOSED` 배치도 포함해서.
    pub halted: Arc<AtomicBool>,
    /// 배치 멱등(I-57) 재시도가 "이미 커밋됨"으로 건너뛴 누적 횟수(K6).
    pub ambiguous_commits_total: Arc<AtomicU64>,
}

/// 접속 풀을 만든다. **여기서 연결하지 않는다**(지연 연결 — ADR-0003 §3.1 제약 1).
///
/// # Errors
///
/// 접속 문자열을 해석할 수 없으면 실패한다.
pub fn pool(database_url: &str) -> Result<PgPool, PersistenceError> {
    Ok(PgPoolOptions::new()
        .max_connections(4)
        .acquire_timeout(Duration::from_secs(5))
        .connect_lazy(database_url)?)
}

/// 바이너리에 embed 된 마이그레이션을 적용한다.
///
/// # Errors
///
/// 접속 실패, 또는 이미 적용된 마이그레이션 파일이 수정되어 체크섬이 어긋나면 실패한다.
pub async fn run_migrations(pool: &PgPool) -> Result<(), PersistenceError> {
    sqlx::migrate!("../../migrations").run(pool).await?;
    Ok(())
}

/// `worlds` 테이블(DB)에서만 오는 월드 상수. `data/` 3종에서 오는 나머지
/// (`starfall_sim::WorldConstants` 의 게임 데이터 필드)는 이 크레이트가 모른다 —
/// `persistence` 는 DB만 알고 `data/` 로딩은 `bins/game-server` 의 몫이다. 호출자가 이
/// 값과 게임 데이터를 합쳐 `WorldConstants` 를 완성한다.
#[derive(Debug, Clone)]
pub struct WorldBasics {
    /// 월드(샤드) id.
    pub world_id: UuidV7,
    /// 게임 달력 상수. 월드 수명 동안 불변 (I-19).
    pub calendar: GameCalendar,
    /// 서버 빌드 버전.
    pub server_version: ServerVersion,
}

/// `worlds` 행을 읽고 설정과 대조한다. 다르면 **기동을 거부한다** (AC-2).
///
/// # Errors
///
/// 행이 없거나 `tick_hz` 가 다르거나 달력 상수를 해석할 수 없으면 실패한다.
pub async fn load_world(
    pool: &PgPool,
    world_id: UuidV7,
    configured_tick_hz: u32,
    server_version: ServerVersion,
) -> Result<(WorldBasics, Option<u64>), PersistenceError> {
    let row = sqlx::query(
        "SELECT tick_hz, calendar_epoch, calendar_scale, last_tick FROM worlds WHERE world_id = $1",
    )
    .bind(world_id.get())
    .fetch_optional(pool)
    .await?
    .ok_or(PersistenceError::WorldMissing { world_id })?;

    let tick_hz: i32 = row.try_get("tick_hz")?;
    let epoch: String = row.try_get("calendar_epoch")?;
    let scale: i32 = row.try_get("calendar_scale")?;
    let last_tick: Option<i64> = row.try_get("last_tick")?;

    let stored_hz = u32::try_from(tick_hz).unwrap_or(0);
    if stored_hz != configured_tick_hz {
        return Err(PersistenceError::TickHzMismatch {
            stored: stored_hz,
            configured: configured_tick_hz,
        });
    }

    let calendar = GameTime::parse(epoch.clone())
        .and_then(|epoch| GameCalendar::new(stored_hz, epoch, u32::try_from(scale).unwrap_or(0)))
        .ok_or(PersistenceError::WorldConstants {
            tick_hz,
            epoch,
            scale,
        })?;

    Ok((
        WorldBasics {
            world_id,
            calendar,
            server_version,
        },
        last_tick.and_then(|value| u64::try_from(value).ok()),
    ))
}

/// 이 월드의 다음 tick 번호 (ADR-0006 §2.3).
///
/// `worlds.last_tick` 과 `max(domain_events.tick)` 중 **큰 값 + 1**. 둘 다 없으면 0.
///
/// 두 출처를 모두 보는 이유: `last_tick` 은 같은 트랜잭션에서 갱신되므로 정상 경로에서는
/// 언제나 `max(tick)` 이상이지만, QA 의 append-only 탐침처럼 **서버 밖에서 행이 들어오면**
/// `max(tick)` 이 더 클 수 있다. 그때도 tick 은 뒤로 가지 않아야 한다 (I-17).
///
/// # Errors
///
/// 쿼리 실패 시.
pub async fn resume_tick(pool: &PgPool, world_id: UuidV7) -> Result<u64, PersistenceError> {
    let row = sqlx::query(
        "SELECT GREATEST(
             COALESCE((SELECT last_tick FROM worlds WHERE world_id = $1), -1),
             COALESCE((SELECT max(tick) FROM domain_events WHERE world_id = $1), -1)
         ) + 1 AS next_tick",
    )
    .bind(world_id.get())
    .fetch_one(pool)
    .await?;
    let next: i64 = row.try_get("next_tick")?;
    Ok(u64::try_from(next).unwrap_or(0))
}

/// 인벤토리 행 하나(`load_economic_state`) — `starfall_sim::Simulation::seed_inventory`
/// 의 인자 셋과 1:1.
#[derive(Debug, Clone)]
pub struct InventoryRow {
    /// 소유자.
    pub actor_id: UuidV7,
    /// 광물.
    pub mineral_id: DataId,
    /// 보유량(kg). 0 행은 없다(계약 불변식, 0002 §comment).
    pub quantity_kg: i64,
}

/// 매장지 상태 행 하나(`load_economic_state`) — **드러난 매장지만** 있다(I-68).
/// `starfall_sim::Simulation::seed_deposit_state` 의 인자 넷과(첫 번째가 `deposit_id`)
/// 1:1.
#[derive(Debug, Clone)]
pub struct DepositRow {
    /// 매장지.
    pub deposit_id: DataId,
    /// `as_of_tick` 시점 기준 잔량(회복 미적용 — sim 이 적용한다).
    pub remaining_kg: i64,
    /// `remaining_kg` 를 기록한 tick.
    pub as_of_tick: u64,
    /// 첫 채굴 tick(드러난 뒤 불변).
    pub first_extracted_tick: u64,
}

/// 기동 시 적재하는 한 월드의 경제 상태 전체(ADR-0013 §7 2단계, S8).
///
/// 세 필드는 각각 `starfall_sim::Simulation` 의 `seed_inventory`/`seed_deposit_state`/
/// `seed_processed_command_ids` 인자로 그대로 넘길 수 있는 모양이다 — server 는 반복문
/// 세 개만 쓰면 된다:
///
/// ```ignore
/// let economic = starfall_persistence::load_economic_state(&pool, world_id).await?;
/// for row in economic.inventory {
///     sim.seed_inventory(row.actor_id, row.mineral_id, row.quantity_kg);
/// }
/// for row in economic.deposits {
///     sim.seed_deposit_state(row.deposit_id, row.remaining_kg, row.as_of_tick, row.first_extracted_tick);
/// }
/// sim.seed_processed_command_ids(economic.processed_command_ids);
/// ```
#[derive(Debug, Clone)]
pub struct EconomicState {
    /// `inventory_items` 전 행(이 월드).
    pub inventory: Vec<InventoryRow>,
    /// `deposit_states` 전 행(이 월드) — 드러난 매장지만.
    pub deposits: Vec<DepositRow>,
    /// `processed_commands` 의 `command_id` 전체(이 월드) — 지속 중복 기억의 기동 시
    /// 적재분(ADR-0013 §3).
    pub processed_command_ids: Vec<UuidV7>,
}

/// 한 월드의 경제 상태(인벤토리·매장지·처리 장부) 전부를 적재한다(ADR-0013 §7 2단계).
///
/// 기동 순서: 마이그레이션 → 월드 대조 → **여기** → 역사 적재 → tick 드라이버 생성
/// (ADR-0013 §7). 상태 없이 tick 이 시작하면 첫 채굴이 "빈 인벤토리"를 전제로 판정돼
/// 비교 후 쓰기(K3)가 바로 월드를 멈춘다.
///
/// # Errors
///
/// 쿼리 실패, 또는 저장된 행이 계약 밖의 값(잘못된 `DataId`·비-v7 UUID)을 담고 있으면
/// 실패한다(있을 수 없는 상태 — 쓰는 쪽은 항상 유효한 값만 쓴다, 방어적).
pub async fn load_economic_state(
    pool: &PgPool,
    world_id: UuidV7,
) -> Result<EconomicState, PersistenceError> {
    let inventory_rows =
        sqlx::query("SELECT actor_id, mineral_id, quantity_kg FROM inventory_items WHERE world_id = $1 ORDER BY actor_id, mineral_id")
            .bind(world_id.get())
            .fetch_all(pool)
            .await?;
    let mut inventory = Vec::with_capacity(inventory_rows.len());
    for row in inventory_rows {
        let actor_id_raw: uuid::Uuid = row.try_get("actor_id")?;
        let actor_id = UuidV7::from_uuid(actor_id_raw).ok_or_else(|| {
            PersistenceError::CorruptEconomicRow {
                table: "inventory_items",
                world_id,
                detail: format!("actor_id={actor_id_raw} 는 UUIDv7 이 아니다"),
            }
        })?;
        let mineral_id_raw: String = row.try_get("mineral_id")?;
        let mineral_id = DataId::parse(mineral_id_raw.clone()).ok_or_else(|| {
            PersistenceError::CorruptEconomicRow {
                table: "inventory_items",
                world_id,
                detail: format!("mineral_id={mineral_id_raw:?} 는 유효한 DataId 가 아니다"),
            }
        })?;
        let quantity_kg: i64 = row.try_get("quantity_kg")?;
        inventory.push(InventoryRow {
            actor_id,
            mineral_id,
            quantity_kg,
        });
    }

    let deposit_rows = sqlx::query(
        "SELECT deposit_id, remaining_kg, as_of_tick, first_extracted_tick \
         FROM deposit_states WHERE world_id = $1 ORDER BY deposit_id",
    )
    .bind(world_id.get())
    .fetch_all(pool)
    .await?;
    let mut deposits = Vec::with_capacity(deposit_rows.len());
    for row in deposit_rows {
        let deposit_id_raw: String = row.try_get("deposit_id")?;
        let deposit_id = DataId::parse(deposit_id_raw.clone()).ok_or_else(|| {
            PersistenceError::CorruptEconomicRow {
                table: "deposit_states",
                world_id,
                detail: format!("deposit_id={deposit_id_raw:?} 는 유효한 DataId 가 아니다"),
            }
        })?;
        let remaining_kg: i64 = row.try_get("remaining_kg")?;
        let as_of_tick: i64 = row.try_get("as_of_tick")?;
        let first_extracted_tick: i64 = row.try_get("first_extracted_tick")?;
        deposits.push(DepositRow {
            deposit_id,
            remaining_kg,
            as_of_tick: u64::try_from(as_of_tick).unwrap_or(0),
            first_extracted_tick: u64::try_from(first_extracted_tick).unwrap_or(0),
        });
    }

    let command_rows = sqlx::query(
        "SELECT command_id FROM processed_commands WHERE world_id = $1 ORDER BY command_id",
    )
    .bind(world_id.get())
    .fetch_all(pool)
    .await?;
    let mut processed_command_ids = Vec::with_capacity(command_rows.len());
    for row in command_rows {
        let command_id_raw: uuid::Uuid = row.try_get("command_id")?;
        let command_id = UuidV7::from_uuid(command_id_raw).ok_or_else(|| {
            PersistenceError::CorruptEconomicRow {
                table: "processed_commands",
                world_id,
                detail: format!("command_id={command_id_raw} 는 UUIDv7 이 아니다"),
            }
        })?;
        processed_command_ids.push(command_id);
    }

    Ok(EconomicState {
        inventory,
        deposits,
        processed_command_ids,
    })
}

/// 지금 시각을 `RealTime` 으로. 호스트 시계를 쓴다(컨테이너 시계가 아니다).
#[must_use]
pub fn now_real_time() -> Option<RealTime> {
    #[cfg(feature = "test-hooks")]
    if test_hooks::take_clock_failure() {
        return None;
    }
    let since = SystemTime::now().duration_since(UNIX_EPOCH).ok()?;
    RealTime::from_unix(i64::try_from(since.as_secs()).ok()?, since.subsec_nanos())
}

/// 영속화 루프.
///
/// tick 루프와 **분리된** 태스크다. tick 루프는 DB 를 기다리지 않는다 — 기다리면 DB 지연이
/// 곧 시뮬레이션 지연이 된다 (ADR-0007 §4).
///
/// DB 가 일시적으로 죽어 있으면 같은 배치를 계속 재시도한다. **버리지 않는다.** 대신
/// `last_committed_tick` 이 멈춰 있어 `persist_backlog` 가 자라고, 임계를 넘으면
/// 게이트웨이가 새 연결을 거절한다. 하지만 **복구 불가** 오류(K4 — 제약 위반, 비교 후
/// 쓰기 불일치, 시계 불가, payload 직렬화 실패 등)를 만나면 재시도를 멈추고 이
/// 루프 자체를 끝낸다(K5) — 그 뒤로는 어떤 배치도 커밋하지 않는다. 종료 스윕의
/// `SESSION_CLOSED` 배치가 아직 채널에 남아 있어도 소비하지 않고 버린다(정지가 결정된
/// 순간부터 메모리 위의 기록은 갈라졌을 수 있어서다).
///
/// `commit_notify` 는 T0 합의(`02_server_ack.md` §4)의 커밋 알림 채널이다 — **깨우기일
/// 뿐**이다. 커밋(건너뜀 포함) 성공마다 `send_replace(Some(tick))` 한다. 워터마크는
/// 여전히 DB `worlds.last_tick` 이고, 러너는 `changed()` 를 본 뒤 그 워터마크를 직접
/// 읽는다(값 자체를 신뢰하지 않는다). 이 함수가 끝나면(정상 종료·정지 모두) `Sender` 가
/// drop 되어 러너가 마지막으로 한 번 따라잡고 종료한다.
pub async fn run(
    pool: PgPool,
    world_id: UuidV7,
    mut batches: tokio::sync::mpsc::Receiver<PersistBatch>,
    handles: PersistHandles,
    commit_notify: watch::Sender<Option<u64>>,
) {
    while let Some(batch) = batches.recv().await {
        let keep_going = commit_with_retry(&pool, world_id, &batch, &handles, &commit_notify).await;
        if !keep_going {
            tracing::error!(
                tick = batch.tick,
                "영속화 정지(K5) — 이후 배치를 커밋하지 않고 태스크를 끝낸다. 종료 스윕의 \
                 배치가 아직 채널에 남아 있어도 버린다"
            );
            break;
        }
    }
    tracing::info!("영속화 태스크 종료");
    // `commit_notify` 는 여기서 drop 된다(함수 인자 소유) — 러너가 이것으로 종료를 안다.
}

/// 한 배치를 커밋할 때까지(또는 정지가 결정될 때까지) 재시도한다.
///
/// `false` 를 돌려주면 [`run`] 의 루프가 끝난다(K5).
async fn commit_with_retry(
    pool: &PgPool,
    world_id: UuidV7,
    batch: &PersistBatch,
    handles: &PersistHandles,
    commit_notify: &watch::Sender<Option<u64>>,
) -> bool {
    let mut attempt: u32 = 0;
    loop {
        match commit(pool, world_id, batch).await {
            Ok(CommitOutcome::Committed) => {
                #[cfg(feature = "test-hooks")]
                if test_hooks::take_apparent_failure() {
                    // 실제로는 커밋됐지만(last_tick 이 이미 전진했다) 응답이 유실된
                    // 것처럼 재시도를 강제한다 — 다음 바퀴는 commit() 의 tick 비교가
                    // AlreadyCommitted 로 건너뛴다(K6, `ambiguous_commit_retry_counts_once`).
                    attempt = attempt.saturating_add(1);
                    tokio::time::sleep(Duration::from_millis(10)).await;
                    continue;
                }
                handles
                    .persisted_total
                    .fetch_add(batch.events.len() as u64, Ordering::Relaxed);
                handles
                    .last_committed_tick
                    .store(batch.tick, Ordering::Release);
                let _ = commit_notify.send_replace(Some(batch.tick));
                return true;
            }
            Ok(CommitOutcome::AlreadyCommitted) => {
                // 배치 멱등(I-57, K6) — 모호한 커밋 재시도에서만 일어난다. 관측 가능하게
                // 만든다: 이 카운터가 없으면 "건너뛰었다"와 "아무 것도 안 했다"가 로그로
                // 구분되지 않는다(CLAUDE.md 검증 규율).
                handles
                    .ambiguous_commits_total
                    .fetch_add(1, Ordering::Relaxed);
                // ADR-0013 §4: "이미 커밋된 배치이므로 persisted_total 도 올린다" — 이
                // 배치가 실제로 이벤트를 냈다는 사실은 재시도로 바뀌지 않는다.
                // `persisted_total` 은 "이 서버가 성공으로 처리한 배치의 이벤트 수"를
                // 재는 것이지 "실제로 쓴 행 수"가 아니다 — 후자는 `COUNT(domain_events)`
                // 가 잰다(`ambiguous_commit_retry_counts_once` 가 이 항등식을 확인한다).
                handles
                    .persisted_total
                    .fetch_add(batch.events.len() as u64, Ordering::Relaxed);
                handles
                    .last_committed_tick
                    .store(batch.tick, Ordering::Release);
                let _ = commit_notify.send_replace(Some(batch.tick));
                return true;
            }
            Err(error) if error.is_transient() => {
                // qa 실측(2026-09-29): `test-hooks` 는 릴리스·`-p` 빌드에는 안 들어가지만
                // `cargo test --workspace`/`--all-targets` 직후 재사용되는 debug
                // `starfall-game-server` 실행 파일에는 feature 통합으로 섞인다. 그
                // 상태에서도 이 줄은 **제어 흐름에 아무 영향이 없다** — 카운터 하나를
                // 올릴 뿐이고(감싸돌면 그냥 0으로 돌아간다, 진단용이라 문제없다), 분기
                // 선택·반환값·재시도 여부·DB 에 쓰는 값 전부 이 줄이 있든 없든 같다.
                // 증거용 실서버는 `cargo build -p starfall-game-server` 로 다시 빌드해
                // 이 feature 를 빼도록 qa 도구(`server_boot.py`)가 강제한다
                // (`03_server_impl.md` S6 절 참고).
                #[cfg(feature = "test-hooks")]
                test_hooks::record_transient_hit();
                attempt = attempt.saturating_add(1);
                if attempt == 1 || attempt.is_multiple_of(20) {
                    tracing::warn!(
                        tick = batch.tick,
                        attempt,
                        %error,
                        "영속화 재시도 중(일시 오류) — 이벤트를 버리지 않는다 (I-22)"
                    );
                }
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
            Err(error) => {
                // 복구 불가(K4) — 재시도해도 절대 성공하지 않는다. 로그 한 줄에 사유·
                // 카운터 값을 함께 찍는다(AC-5(b) 증거 — 프로세스가 곧 사라지므로
                // `/debug/stats` 를 나중에 못 읽는다).
                let fatal_total = handles
                    .failed_total
                    .fetch_add(batch.events.len() as u64, Ordering::Relaxed)
                    + batch.events.len() as u64;
                handles.halted.store(true, Ordering::Release);
                // SC-25 — 테스트가 로그를 캡처하지 않고도 이 줄과 같은 값을 볼 수
                // 있게 `%error` 와 같은 `Display` 문자열을 담아 둔다(무해성: 무장
                // 여부와 무관하게 항상 최신 정지 사유로 덮어쓸 뿐, 제어 흐름에는
                // 영향이 없다).
                #[cfg(feature = "test-hooks")]
                test_hooks::record_fatal_detail(error.to_string());
                tracing::error!(
                    tick = batch.tick,
                    events = batch.events.len(),
                    persist_fatal_total = fatal_total,
                    %error,
                    "복구 불가 영속화 실패 — 월드를 멈춘다(ADR-0013 §5)"
                );
                return false;
            }
        }
    }
}

/// 일시 실패 분류(K4). 이 목록 **밖**은 전부 복구 불가다 — 제약 위반(23xxx)·데이터
/// 예외(22xxx)·스키마 불일치(42xxx) 같은 절대 성공하지 않는 오류를 무한 재시도에
/// 남기지 않는다.
///
/// `57P01`(admin_shutdown)을 반드시 포함한다 — `docker compose stop postgres` 가 커밋
/// 도중이면 이 코드가 온다. 빠뜨리면 AC-6이 정지로 끝난다(server 검토 K4).
fn is_transient_db_error(error: &sqlx::Error) -> bool {
    match error {
        sqlx::Error::Io(_) | sqlx::Error::PoolTimedOut | sqlx::Error::PoolClosed => true,
        sqlx::Error::Database(db) => db.code().is_some_and(|code| {
            let code = code.as_ref();
            code.starts_with("08") // connection_exception
                || code == "40001" // serialization_failure
                || code == "40P01" // deadlock_detected
                || code.starts_with("53") // insufficient_resources
                || matches!(code, "57P01" | "57P02" | "57P03") // admin_shutdown·crash·cannot_connect_now
        }),
        _ => false,
    }
}

/// 커밋 시도의 결과. `AlreadyCommitted` 은 배치 멱등(I-57)이 건너뛴 경로다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CommitOutcome {
    /// 이번 시도가 실제로 썼다.
    Committed,
    /// 이미 커밋된 tick 이라 아무 것도 쓰지 않고 성공으로 끝냈다.
    AlreadyCommitted,
}

/// 커밋 실패. `is_transient()` 가 재시도 여부를 가른다(K4) — 그 결과 자체를 만드는
/// 판단은 [`is_transient_db_error`] 하나뿐이다.
#[derive(Debug, thiserror::Error)]
enum CommitError {
    /// DB 오류(전송·쿼리). 일시적일 수도 복구 불가일 수도 있다.
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    /// 비교 후 쓰기(K3)의 기대값이 저장된 값과 달랐다 — 항상 복구 불가(무결성 위반,
    /// SQL 이 영향 행 수로 알려준다).
    #[error("비교 후 쓰기 불일치: {detail}")]
    CompareAndSetMismatch {
        /// 어떤 비교였는지(진단용).
        detail: String,
    },
    /// 호스트 시계를 읽지 못했다(`recorded_at`) — 항상 복구 불가(2.1 결함 수정 — 예전엔
    /// 이벤트를 조용히 건너뛰었다).
    #[error("호스트 시계를 읽을 수 없다(recorded_at) — event_id={event_id}")]
    ClockUnavailable {
        /// 영향받은 이벤트.
        event_id: UuidV7,
    },
    /// payload 직렬화 실패 — 항상 복구 불가(2.2 결함 수정 — 예전엔 JSONB `null` 로
    /// 저장돼 `NOT NULL` 제약을 통과했다).
    #[error("payload 직렬화 실패 — event_id={event_id}")]
    PayloadSerialization {
        /// 영향받은 이벤트.
        event_id: UuidV7,
    },
    /// `worlds.last_tick` 갱신이 0행을 건드렸다 — 이미 위에서 `last_tick` 을 잠그고
    /// 비교했으므로 정상 경로에서는 일어날 수 없다(방어적).
    #[error("worlds.last_tick 갱신이 예상과 다른 행 수를 건드렸다")]
    WorldsUpdateAffectedUnexpectedRows,
}

impl CommitError {
    /// 재시도해도 되는가(K4). `Database` 만 조건부이고 나머지는 전부 복구 불가다.
    fn is_transient(&self) -> bool {
        matches!(self, Self::Database(error) if is_transient_db_error(error))
    }
}

async fn commit(
    pool: &PgPool,
    world_id: UuidV7,
    batch: &PersistBatch,
) -> Result<CommitOutcome, CommitError> {
    // 실제 트랜잭션을 열기 전에 주입을 소비한다 — DB 를 전혀 건드리지 않는다(SC-103a
    // `admin_shutdown_is_transient`/짝 `force_fatal_once`). 이 뒤로는 평소와 같은
    // `CommitError::from` → `is_transient()` 분류를 그대로 탄다.
    //
    // **무해성**(팀 리더 요청, qa 의 워크스페이스 빌드 섞임 실측에 대한 응답):
    // `take_forced_db_error()` 는 두 카운터가 모두 0(무장 안 됨)이면 원자 읽기 두 번만
    // 하고 즉시 `None` 을 돌려준다 — 이 `if let` 은 아무 것도 하지 않은 것과 같다.
    // `TRANSACTION_ATTEMPT_COUNTER` 증가(바로 아래)도 마찬가지로 제어 흐름을 바꾸지
    // 않는다 — 그냥 세는 것뿐이고, 아무도 읽지 않으면 감싸돈 채 남아 있을 뿐이다.
    // `feature = "test-hooks"` 가 워크스페이스 테스트 빌드에서 운영 실행 파일에 섞여도
    // (qa 실측, 2026-09-29) **커밋 로직 자체는 바이트 단위로 같다** — 증거용 실서버는
    // 그래도 `cargo build -p starfall-game-server` 로 다시 빌드해 이 feature 를 아예
    // 빼도록 qa 도구가 강제한다(`03_server_impl.md` S6 절).
    #[cfg(feature = "test-hooks")]
    if let Some(fake_error) = test_hooks::take_forced_db_error() {
        return Err(CommitError::Database(fake_error));
    }

    let mut tx: Transaction<'_, sqlx::Postgres> = pool.begin().await?;
    #[cfg(feature = "test-hooks")]
    test_hooks::TRANSACTION_ATTEMPT_COUNTER.fetch_add(1, Ordering::SeqCst);

    // 배치 멱등(I-57, ADR-0013 §4) — `worlds` 행을 잠그고 이미 이 tick 이상을 커밋했으면
    // 아무 것도 쓰지 않고 성공으로 끝낸다. 잠그는 이유: 같은 배치의 모호한 커밋 재시도가
    // (K6) 동시에 경합하지 않게.
    let row = sqlx::query("SELECT last_tick FROM worlds WHERE world_id = $1 FOR UPDATE")
        .bind(world_id.get())
        .fetch_one(&mut *tx)
        .await?;
    let last_tick: Option<i64> = row.try_get("last_tick")?;
    let batch_tick_i64 = i64::try_from(batch.tick).unwrap_or(i64::MAX);
    if last_tick.is_some_and(|last| last >= batch_tick_i64) {
        return Ok(CommitOutcome::AlreadyCommitted);
    }

    for event in &batch.events {
        let Some(recorded_at) = now_real_time() else {
            return Err(CommitError::ClockUnavailable {
                event_id: event.event_id,
            });
        };
        let (event_type, payload) = contract_payload(event, &recorded_at)?;

        sqlx::query(
            "INSERT INTO domain_events (
                 event_id, event_type, schema_version, world_id, tick, sequence,
                 occurred_at, recorded_at, correlation_id, causation_id, actor_id, payload
             ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8::timestamptz, $9, $10, $11, $12)
             ON CONFLICT (event_id) DO NOTHING",
        )
        .bind(event.event_id.get())
        .bind(event_type)
        .bind(1_i32)
        .bind(event.world_id.get())
        .bind(i64::try_from(event.tick.get()).unwrap_or(i64::MAX))
        .bind(i64::try_from(event.sequence.get()).unwrap_or(i64::MAX))
        .bind(event.occurred_at.as_str())
        .bind(recorded_at.as_str())
        .bind(event.correlation_id.get())
        .bind(event.causation_id.map(UuidV7::get))
        .bind(event.actor_id.get())
        .bind(sqlx::types::Json(payload))
        .execute(&mut *tx)
        .await?;

        // p1-02 — MINERAL_MINED 는 그 원인(MINE_RESOURCE.command_id)을 처리 장부에도
        // 남긴다(K1, 같은 트랜잭션 — ADR-0013 §2). 장부 쪽 PK 는 (world_id, command_id)
        // 월드 범위다.
        //
        // **`ON CONFLICT DO NOTHING` 을 쓰지 않는다** — 여기까지 같은 `command_id` 가
        // 두 번 도달하는 것은 §4 의 배치 멱등(위 tick 비교)이 이미 걸렀어야 하는 상태다.
        // 그 방어가 어떤 이유로든 뚫리면 이 PK 가 **마지막 방어선**이어야 한다(ADR-0013
        // §3, SC-21 `dup_reaching_db_halts`) — `DO NOTHING` 은 그 방어선을 조용히
        // 무력화해 중복 채굴이 인벤토리에 두 번 반영되는 것을 (장부만 비운 채) 허용한다.
        if let DomainEventBody::MineralMined(_) = &event.body {
            let command_id = event.causation_id.unwrap_or(event.event_id);
            sqlx::query(
                "INSERT INTO processed_commands (world_id, command_id, actor_id, command_type, tick)
                 VALUES ($1, $2, $3, $4, $5)",
            )
            .bind(world_id.get())
            .bind(command_id.get())
            .bind(event.actor_id.get())
            .bind(starfall_contracts::registry::MINE_RESOURCE)
            .bind(i64::try_from(event.tick.get()).unwrap_or(i64::MAX))
            .execute(&mut *tx)
            .await?;
        }
    }

    for write in &batch.state_writes {
        apply_state_write(&mut tx, world_id, write).await?;
    }

    // tick 재개의 정본을 **같은 트랜잭션에서** 갱신한다 (ADR-0007 §4).
    // 뒤로 가지 않도록 조건을 건다 (I-17). 위에서 이미 잠그고 비교했으므로 여기서
    // 영향 행이 1이 아니면 있을 수 없는 상태다 — 방어적으로 잡는다(K6 마지막 줄).
    let update_result = sqlx::query(
        "UPDATE worlds SET last_tick = $2
         WHERE world_id = $1 AND (last_tick IS NULL OR last_tick < $2)",
    )
    .bind(world_id.get())
    .bind(batch_tick_i64)
    .execute(&mut *tx)
    .await?;
    if update_result.rows_affected() != 1 {
        return Err(CommitError::WorldsUpdateAffectedUnexpectedRows);
    }

    tx.commit().await?;
    Ok(CommitOutcome::Committed)
}

/// [`StateWrite`] 하나를 비교 후 쓰기로 적용한다(K3). SQL 에는 게임 규칙이 없다 —
/// 저장된 표현과 `expected` 를 그대로 비교할 뿐이다.
async fn apply_state_write(
    tx: &mut Transaction<'_, sqlx::Postgres>,
    world_id: UuidV7,
    write: &StateWrite,
) -> Result<(), CommitError> {
    match write {
        StateWrite::Inventory {
            actor_id,
            mineral_id,
            expected,
            new,
        } => {
            let result =
                match expected {
                    None => sqlx::query(
                        "INSERT INTO inventory_items (world_id, actor_id, mineral_id, quantity_kg)
                         VALUES ($1, $2, $3, $4)
                         ON CONFLICT (world_id, actor_id, mineral_id) DO NOTHING",
                    )
                    .bind(world_id.get())
                    .bind(actor_id.get())
                    .bind(mineral_id.as_str())
                    .bind(new)
                    .execute(&mut **tx)
                    .await?,
                    Some(expected_quantity) => {
                        sqlx::query(
                            "UPDATE inventory_items SET quantity_kg = $4
                         WHERE world_id = $1 AND actor_id = $2 AND mineral_id = $3
                           AND quantity_kg = $5",
                        )
                        .bind(world_id.get())
                        .bind(actor_id.get())
                        .bind(mineral_id.as_str())
                        .bind(new)
                        .bind(expected_quantity)
                        .execute(&mut **tx)
                        .await?
                    }
                };
            if result.rows_affected() != 1 {
                // SC-25 — 정지 로그에 기대값뿐 아니라 **실제 DB 값**도 남긴다(팀 리더
                // 지시 2026-09-30). 같은 트랜잭션에서 읽는다 — 이 트랜잭션은 곧
                // 롤백되므로 읽기만 하고 아무것도 바꾸지 않는다.
                let actual: Option<i64> = sqlx::query_scalar(
                    "SELECT quantity_kg FROM inventory_items \
                     WHERE world_id = $1 AND actor_id = $2 AND mineral_id = $3",
                )
                .bind(world_id.get())
                .bind(actor_id.get())
                .bind(mineral_id.as_str())
                .fetch_optional(&mut **tx)
                .await?;
                return Err(CommitError::CompareAndSetMismatch {
                    detail: format!(
                        "inventory_items({actor_id}, {mineral_id}) expected={expected:?} actual={actual:?}"
                    ),
                });
            }
        }
        StateWrite::Deposit {
            deposit_id,
            expected,
            new,
            first_extracted_tick,
        } => {
            let (new_remaining, new_as_of_tick) = new;
            let new_as_of_tick_i64 = i64::try_from(*new_as_of_tick).unwrap_or(i64::MAX);
            let first_extracted_tick_i64 = i64::try_from(*first_extracted_tick).unwrap_or(i64::MAX);
            let result = match expected {
                None => {
                    sqlx::query(
                        "INSERT INTO deposit_states
                             (world_id, deposit_id, remaining_kg, as_of_tick, first_extracted_tick)
                         VALUES ($1, $2, $3, $4, $5)
                         ON CONFLICT (world_id, deposit_id) DO NOTHING",
                    )
                    .bind(world_id.get())
                    .bind(deposit_id.as_str())
                    .bind(new_remaining)
                    .bind(new_as_of_tick_i64)
                    .bind(first_extracted_tick_i64)
                    .execute(&mut **tx)
                    .await?
                }
                Some((expected_remaining, expected_as_of_tick)) => {
                    let expected_as_of_tick_i64 =
                        i64::try_from(*expected_as_of_tick).unwrap_or(i64::MAX);
                    sqlx::query(
                        "UPDATE deposit_states SET remaining_kg = $3, as_of_tick = $4
                         WHERE world_id = $1 AND deposit_id = $2
                           AND remaining_kg = $5 AND as_of_tick = $6",
                    )
                    .bind(world_id.get())
                    .bind(deposit_id.as_str())
                    .bind(new_remaining)
                    .bind(new_as_of_tick_i64)
                    .bind(expected_remaining)
                    .bind(expected_as_of_tick_i64)
                    .execute(&mut **tx)
                    .await?
                }
            };
            if result.rows_affected() != 1 {
                // SC-25 — 인벤토리와 같은 이유(위 주석 참고). `(remaining_kg,
                // as_of_tick)` 쌍으로 읽어 `expected`/`new` 와 같은 모양으로 맞춘다.
                let actual_row = sqlx::query(
                    "SELECT remaining_kg, as_of_tick FROM deposit_states \
                     WHERE world_id = $1 AND deposit_id = $2",
                )
                .bind(world_id.get())
                .bind(deposit_id.as_str())
                .fetch_optional(&mut **tx)
                .await?;
                let actual: Option<(i64, i64)> = match actual_row {
                    Some(row) => Some((row.try_get("remaining_kg")?, row.try_get("as_of_tick")?)),
                    None => None,
                };
                return Err(CommitError::CompareAndSetMismatch {
                    detail: format!(
                        "deposit_states({deposit_id}) expected={expected:?} actual={actual:?}"
                    ),
                });
            }
        }
    }
    Ok(())
}

/// 계약 타입을 **실제로 만들어** payload 를 뽑는다.
///
/// 열에 넣는 값과 payload 를 따로 만들면 둘이 조용히 갈라진다. 계약 구조체를 거치면
/// 저장된 JSONB 가 계약 스키마와 같은 모양임이 타입으로 보장된다.
fn contract_payload(
    event: &PendingEvent,
    recorded_at: &RealTime,
) -> Result<(&'static str, Value), CommitError> {
    match &event.body {
        DomainEventBody::SessionOpened(payload) => {
            let full = SessionOpenedEvent {
                event_id: event.event_id,
                event_type: SessionOpenedType::SessionOpened,
                schema_version: ConstSchemaVersion,
                world_id: event.world_id,
                tick: event.tick,
                sequence: event.sequence,
                occurred_at: event.occurred_at.clone(),
                recorded_at: recorded_at.clone(),
                correlation_id: event.correlation_id,
                causation_id: event.causation_id,
                actor_id: event.actor_id,
                payload: payload.clone(),
            };
            Ok((
                starfall_contracts::registry::SESSION_OPENED,
                extract_payload(&full, event.event_id)?,
            ))
        }
        DomainEventBody::SessionClosed(payload) => {
            let full = SessionClosedEvent {
                event_id: event.event_id,
                event_type: SessionClosedType::SessionClosed,
                schema_version: ConstSchemaVersion,
                world_id: event.world_id,
                tick: event.tick,
                sequence: event.sequence,
                occurred_at: event.occurred_at.clone(),
                recorded_at: recorded_at.clone(),
                correlation_id: event.correlation_id,
                causation_id: event.causation_id,
                actor_id: event.actor_id,
                payload: payload.clone(),
            };
            Ok((
                starfall_contracts::registry::SESSION_CLOSED,
                extract_payload(&full, event.event_id)?,
            ))
        }
        DomainEventBody::ShipSpawned(payload) => {
            let full = ShipSpawnedEvent {
                event_id: event.event_id,
                event_type: ShipSpawnedType::ShipSpawned,
                schema_version: ConstSchemaVersion,
                world_id: event.world_id,
                tick: event.tick,
                sequence: event.sequence,
                occurred_at: event.occurred_at.clone(),
                recorded_at: recorded_at.clone(),
                correlation_id: event.correlation_id,
                causation_id: non_null_causation(event),
                actor_id: event.actor_id,
                payload: payload.clone(),
            };
            Ok((
                starfall_contracts::registry::SHIP_SPAWNED,
                extract_payload(&full, event.event_id)?,
            ))
        }
        DomainEventBody::ShipDespawned(payload) => {
            let full = ShipDespawnedEvent {
                event_id: event.event_id,
                event_type: ShipDespawnedType::ShipDespawned,
                schema_version: ConstSchemaVersion,
                world_id: event.world_id,
                tick: event.tick,
                sequence: event.sequence,
                occurred_at: event.occurred_at.clone(),
                recorded_at: recorded_at.clone(),
                correlation_id: event.correlation_id,
                causation_id: non_null_causation(event),
                actor_id: event.actor_id,
                payload: *payload,
            };
            Ok((
                starfall_contracts::registry::SHIP_DESPAWNED,
                extract_payload(&full, event.event_id)?,
            ))
        }
        DomainEventBody::MineralMined(payload) => {
            let full = MineralMinedEvent {
                event_id: event.event_id,
                event_type: MineralMinedType::MineralMined,
                schema_version: ConstSchemaVersion,
                world_id: event.world_id,
                tick: event.tick,
                sequence: event.sequence,
                occurred_at: event.occurred_at.clone(),
                recorded_at: recorded_at.clone(),
                correlation_id: event.correlation_id,
                // MINERAL_MINED 는 계약이 causation_id 를 비-null 로 좁힌다(I-53) — sim 이
                // 언제나 MINE_RESOURCE 의 command_id 를 채워 보낸다는 것이 불변식이다.
                // SHIP_SPAWNED·SHIP_DESPAWNED 와 같은 방어(패닉 대신 자기 id 로 대체 +
                // 로그).
                causation_id: non_null_causation(event),
                actor_id: event.actor_id,
                payload: payload.clone(),
            };
            Ok((
                starfall_contracts::registry::MINERAL_MINED,
                extract_payload(&full, event.event_id)?,
            ))
        }
    }
}

/// `SHIP_SPAWNED`/`SHIP_DESPAWNED` 는 계약이 `causation_id` 를 비-null 로 좁힌다(I-30) —
/// `Simulation` 이 항상 `Some` 을 채워 보낸다는 것이 불변식이다. 그래도 `unwrap`/`expect`
/// 로 패닉하지 않는다 — 어겨졌다면 서버가 죽는 것보다 **눈에 띄게 틀린 값**(이벤트 자신의
/// id)을 저장하고 로그로 드러내는 편이 낫다(rust-authoritative-server §8).
fn non_null_causation(event: &PendingEvent) -> UuidV7 {
    event.causation_id.unwrap_or_else(|| {
        tracing::error!(
            event_id = %event.event_id,
            event_type = event.body.event_type(),
            "I-30 위반 — SHIP_* 이벤트의 causation_id 가 None 이다. event_id 로 대신한다"
        );
        event.event_id
    })
}

/// `payload` 필드를 뽑는다. **직렬화 실패를 `Value::Null` 로 감추지 않는다**(2.2 결함
/// 수정) — `Value::Null` 은 `sqlx::types::Json` 을 거쳐 SQL `NULL` 이 아니라 **JSONB
/// `null`** 로 바인딩되어 `payload JSONB NOT NULL` 을 통과해 버렸다. 이제 실패는 항상
/// `CommitError::PayloadSerialization`(복구 불가)이 된다.
///
/// # Errors
///
/// 직렬화 실패, 또는 결과에 `payload` 키가 없으면 실패한다(계약 타입이면 정상 경로에서
/// 일어나지 않는다 — 방어적).
fn extract_payload<T: serde::Serialize>(event: &T, event_id: UuidV7) -> Result<Value, CommitError> {
    #[cfg(feature = "test-hooks")]
    if test_hooks::take_payload_failure() {
        return Err(CommitError::PayloadSerialization { event_id });
    }
    serde_json::to_value(event)
        .ok()
        .and_then(|mut value| value.get_mut("payload").map(Value::take))
        .ok_or(CommitError::PayloadSerialization { event_id })
}

#[cfg(test)]
mod tests {
    use super::*;
    use starfall_contracts::events::{SessionOpenedPayload, SessionTransport};
    use starfall_contracts::primitives::{Sequence, Tick};

    fn uuid(text: &str) -> UuidV7 {
        UuidV7::parse(text).expect("정규 UUIDv7")
    }

    #[test]
    fn payload_matches_the_contract_shape() {
        let event = PendingEvent {
            event_id: uuid("01a0b1c2-4a13-7d09-b8e1-2f0a3b4c5d6e"),
            world_id: uuid("01a0b1c2-3d4e-7f01-8a2b-9c0d1e2f3a4b"),
            tick: Tick::new(1200).unwrap(),
            sequence: Sequence::new(0).unwrap(),
            occurred_at: GameTime::parse("3800-01-01T01:00:00Z").unwrap(),
            correlation_id: uuid("01a0b1c2-4a12-7c01-8d02-1e033f044a05"),
            causation_id: None,
            actor_id: uuid("01a0b1c2-2c01-7a45-8b67-89abcdef0123"),
            body: DomainEventBody::SessionOpened(SessionOpenedPayload {
                session_id: uuid("01a0b1c2-4a11-7b22-9c33-0d44e55f6a77"),
                transport: SessionTransport::Websocket,
            }),
        };
        let recorded = RealTime::parse("2026-09-18T13:04:22.417Z").unwrap();
        let (event_type, payload) = contract_payload(&event, &recorded).expect("직렬화 실패");

        assert_eq!(event_type, "SESSION_OPENED");
        // contracts/fixtures/SESSION_OPENED/basic.json 의 payload 와 같은 모양이어야 한다.
        assert_eq!(
            payload,
            serde_json::json!({
                "session_id": "01a0b1c2-4a11-7b22-9c33-0d44e55f6a77",
                "transport": "WEBSOCKET"
            })
        );
    }

    #[test]
    fn now_real_time_parses_as_contract_real_time() {
        let now = now_real_time().expect("시계를 읽을 수 있어야 한다");
        assert!(RealTime::parse(now.as_str()).is_some(), "{now}");
    }

    /// K4 분류(일시 실패 허용 목록) — 실제 DB에서 지정한 SQLSTATE로 예외를 일으켜
    /// [`is_transient_db_error`] 를 실측한다(가짜 `sqlx::Error` 를 손으로 만들 수
    /// 없으므로 `RAISE EXCEPTION USING ERRCODE` 로 진짜 오류를 얻는다).
    #[tokio::test]
    async fn error_classification() {
        // 계약 밖 테스트(qa 확인) — SC-103/SC-112 가 요구하는 두 이름
        // `admin_shutdown_is_transient`/`terminated_backend_is_retried` 는 이제
        // `tests/economic_state.rs` 에 따로 있다(server-db, S6). 이 테스트는 그 둘과
        // 겹치지 않는다: 저 둘은 **전체 커밋 경로**(재시도·정지)를 재는 통합 테스트고,
        // 이건 [`is_transient_db_error`] 하나만 놓고 **허용 목록 전체**(여기 10개
        // SQLSTATE)를 표로 훑는 단위 테스트다. 모듈 경로는 붙이지 않는다(census 대조).
        let Some(db) = starfall_testdb::TestDb::create("error_classification").await else {
            return;
        };

        // 일시 실패(K4 허용 목록): 08(연결)·40001(직렬화 실패)·40P01(교착)·53(자원)·
        // 57P01~57P03(관리자 종료·크래시).
        let transient_codes = [
            "08006", "40001", "40P01", "53300", "57P01", "57P02", "57P03",
        ];
        for code in transient_codes {
            let error = raise(&db.pool, code).await;
            assert!(
                is_transient_db_error(&error),
                "{code} 는 일시 실패여야 한다: {error}"
            );
        }

        // 복구 불가(그 밖 전부): 23(무결성)·22(데이터)·42(구문/접근 권한).
        let fatal_codes = ["23505", "22001", "42601"];
        for code in fatal_codes {
            let error = raise(&db.pool, code).await;
            assert!(
                !is_transient_db_error(&error),
                "{code} 는 복구 불가여야 한다: {error}"
            );
        }

        db.drop().await;
    }

    /// 지정한 SQLSTATE로 예외를 일으켜 진짜 `sqlx::Error::Database` 를 얻는다.
    #[cfg(test)]
    async fn raise(pool: &PgPool, sqlstate: &str) -> sqlx::Error {
        sqlx::query(&format!(
            "DO $$ BEGIN RAISE EXCEPTION USING ERRCODE = '{sqlstate}', MESSAGE = 'test'; END $$;"
        ))
        .execute(pool)
        .await
        .expect_err("의도적으로 일으킨 예외인데 성공했다")
    }
}

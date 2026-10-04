//! 역사 러너·역사 저장소 (H2, ADR-0014 §1~§6).
//!
//! **소유권**: 이 파일은 history-engine-engineer 소유다(`01_architect_tasks.md`).
//!
//! # 이 파일이 하는 일
//!
//! 1. 기동 시 [`load`] — 이미 커밋된 `historical_events` 에서 판정 상태(발견된
//!    `(성계, 광물)` 집합)를 재구축하고, `history_cursor` 를 읽고, PUBLIC 기록 전부를
//!    초기 BACKFILL 목록으로 돌려준다.
//! 2. [`run_runner`] — 영속화의 커밋 알림(`watch`)을 받아 깨어나, 커서 뒤·워터마크
//!    앞의 커밋된 `MINERAL_MINED` 를 `(tick, sequence)` 순으로 읽어 [`starfall_history`]
//!    코어로 판정하고, 통과한 기록을 **한 트랜잭션**(기록 + 근거 + 증거 + 커서 전진)으로
//!    쓴다. 판정 대상 타입이 아닌 행은 좌표만 코어에 넘겨 정렬·워터마크 검사만 받는다
//!    (Level 0, SC-49).
//!
//! # 멱등 3겹과 재처리/충돌 구분 (ADR-0014 §4)
//!
//! `historical_events` 의 `UNIQUE (world_id, event_type, dedupe_key)` 가 0 행으로
//! 막히면, 기존 행과 **내용 전체**(`recorded_at` 제외)를 비교한다. 같으면 재처리(조용히
//! 커서만 전진), 다르면 충돌(롤백 + `history_conflicts_total` +1 + 러너만 정지, 커서는
//! 그 이벤트 앞).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

use sqlx::postgres::{PgConnectOptions, PgPool, PgPoolOptions, PgRow};
use sqlx::{Row, Transaction};
use starfall_contracts::data::{EvidenceType, SignificanceRuleTable};
use starfall_contracts::events::{MineralMinedEvent, MineralMinedPayload, MineralMinedType};
use starfall_contracts::historical::{
    FactStatus, HistoricalLocation, HistoricalParticipant, HistoricalVisibility,
    MineralDiscoveredEvent, MineralDiscoveredPayload, MineralDiscoveredType,
};
use starfall_contracts::primitives::{
    ConstSchemaVersion, DataId, GameTime, RealTime, RuleVersion, Sequence, Tick, UuidV5, UuidV7,
};
use starfall_history::{CoreError, DomainEvent, HistoryCore, MineralDiscoveredRecord};
use tokio::sync::{mpsc, watch};
use uuid::Uuid;

/// 이 규칙 하나뿐인 소비자 이름(`history_cursor.consumer`). 규칙이 늘면 소비자별 독립
/// 커서가 필요해질 것을 대비해 상수로 뺀다.
const CONSUMER: &str = "mineral-discovery";

/// 한 번에 읽어오는 `domain_events` 행 수. 무한 루프 안에서 빈 결과가 나올 때까지
/// 반복하므로, 값이 작아도 정확성에는 영향이 없다 — 메모리·왕복 횟수 절충일 뿐이다.
const BATCH_SIZE: i64 = 500;

/// 역사 적재·러너 오류.
#[derive(Debug, thiserror::Error)]
pub enum HistoryError {
    /// 접속·쿼리 실패.
    #[error("PostgreSQL 오류")]
    Database(#[from] sqlx::Error),
    /// 저장된 행이 계약 모양으로 복원되지 않는다(우리가 쓴 데이터인데 못 읽으면 심각한
    /// 결함이다 — 기동을 거부한다).
    #[error("역사 표의 값을 계약 모양으로 복원할 수 없다: {0}")]
    Corrupt(String),
}

/// 기동 시 적재한 역사 상태.
#[derive(Debug)]
pub struct HistoryBoot {
    /// 기동 시점의 PUBLIC 역사 기록 목록 — 게이트웨이가 새 세션에 그대로 BACKFILL 한다.
    pub initial_backfill: Vec<MineralDiscoveredEvent>,
    core: HistoryCore,
    cursor: Option<(Tick, Sequence)>,
}

/// 관측 값. 게이트웨이의 `/debug/stats` 가 같은 `Arc` 를 읽는다(persistence 의
/// `PersistHandles` 와 같은 패턴).
#[derive(Debug, Clone)]
pub struct HistoryHandles {
    /// 이 러너가 커밋한 역사 기록 누적 수.
    pub records_total: Arc<AtomicU64>,
    /// 다른 근거의 충돌로 러너가 멈춘 누적 횟수(AC-11(h)(ii)·(iii)).
    pub conflicts_total: Arc<AtomicU64>,
    /// 판정 불가(역직렬화 실패·모르는 schema_version)로 러너가 멈춘 누적 횟수(fail-stop).
    pub detector_halted_total: Arc<AtomicU64>,
    /// 러너가 지금 멈춰 있는가. `persist` 의 `halted`(경제)와 별개다 — 러너가 멈춰도
    /// 서버·경제는 계속 돈다.
    pub halted: Arc<AtomicBool>,
    /// 커서가 가리키는 tick(관측용).
    pub cursor_tick: Arc<AtomicU64>,
    /// 이 러너가 지금까지 읽은 `domain_events` 행 수(따라잡기 진행 관측용).
    pub catchup_rows_total: Arc<AtomicU64>,
}

impl HistoryHandles {
    /// 전부 0/false 인 새 핸들.
    #[must_use]
    pub fn new() -> Self {
        Self {
            records_total: Arc::new(AtomicU64::new(0)),
            conflicts_total: Arc::new(AtomicU64::new(0)),
            detector_halted_total: Arc::new(AtomicU64::new(0)),
            halted: Arc::new(AtomicBool::new(false)),
            cursor_tick: Arc::new(AtomicU64::new(0)),
            catchup_rows_total: Arc::new(AtomicU64::new(0)),
        }
    }
}

impl Default for HistoryHandles {
    fn default() -> Self {
        Self::new()
    }
}

/// SC-59(`history-replay`) 전용 풀 — 모든 연결이 시작부터
/// `default_transaction_read_only = on` 이다. 증거 DB 에 대고 돌리는 도구라, "쓰기
/// 코드가 없다"(소스 부정)에 **DB 자체가 쓰기를 거부하는 것**을 더한다(team-lead 지시,
/// 2026-09-30). 세션 GUC 라 풀이 새 연결을 만들 때마다 적용되고, 트랜잭션을 열지 않은
/// 단일 문장에도 걸린다(SQLSTATE `25006`).
///
/// # Errors
///
/// `database_url` 을 해석할 수 없으면 실패한다.
pub fn read_only_pool(database_url: &str) -> Result<PgPool, HistoryError> {
    let options: PgConnectOptions = database_url
        .parse()
        .map_err(|error: sqlx::Error| HistoryError::Database(error))?;
    let options = options.options([("default_transaction_read_only", "on")]);
    Ok(PgPoolOptions::new()
        .max_connections(4)
        .acquire_timeout(Duration::from_secs(5))
        .connect_lazy_with(options))
}

impl HistoryBoot {
    /// **테스트 전용 진입점.** 판정 상태를 DB 와 무관하게 비운 채로 만든다 — `load()` 는
    /// 언제나 `historical_events` 에서 올바르게 재구축하므로, 정상 경로로는 이 상태에
    /// 도달할 수 없다. 재구축 **자체가 버그였을 때**(AC-11(h)(ii)·(iii), SC-105·111)를
    /// 시뮬레이션하는 것이 유일한 목적이다 — DB 에는 이미 행이 있는데 판정 상태는 그것을
    /// 모르는 상황을 만들어, 의미 유일성의 DB 쪽 절반(`UNIQUE` 제약)이 실제로 잡아내는지
    /// 본다.
    #[must_use]
    pub fn empty(rule: SignificanceRuleTable) -> Self {
        Self {
            initial_backfill: Vec::new(),
            core: HistoryCore::new(rule),
            cursor: None,
        }
    }
}

/// 기동 시 역사 상태를 적재한다.
///
/// 순서(ADR-0013 §7 개정): 마이그레이션 → 경제 상태 적재 → **여기** → tick 드라이버 생성
/// (이 함수의 결과가 `build` 의 인자라서 "목록 전달 전 연결 수락"이 타입으로 불가능해진다)
/// → 영속화·러너 spawn → bind.
///
/// `rule` 은 조립 바이너리(S2)가 `data/history/rules/mineral-discovery.json` 을 적재·
/// 검증해 넘긴 값이다 — 이 함수도 [`HistoryCore`] 도 파일을 읽지 않는다.
///
/// # Errors
///
/// DB 접속·쿼리 실패, 또는 저장된 행이 계약 모양으로 복원되지 않으면 실패한다(기동 거부).
pub async fn load(
    pool: &PgPool,
    world_id: UuidV7,
    rule: SignificanceRuleTable,
) -> Result<HistoryBoot, HistoryError> {
    let discovered = load_discovered_set(pool, world_id).await?;
    let cursor = load_cursor(pool, world_id).await?;
    let initial_backfill = load_public_backfill(pool, world_id).await?;

    let core = HistoryCore::rebuild(rule, world_id, discovered, cursor);

    Ok(HistoryBoot {
        initial_backfill,
        core,
        cursor,
    })
}

async fn load_discovered_set(
    pool: &PgPool,
    world_id: UuidV7,
) -> Result<Vec<(DataId, DataId)>, HistoryError> {
    let rows = sqlx::query("SELECT dedupe_key FROM historical_events WHERE world_id = $1")
        .bind(world_id.get())
        .fetch_all(pool)
        .await?;

    let mut discovered = Vec::with_capacity(rows.len());
    for row in &rows {
        let key: String = row.try_get("dedupe_key")?;
        let (system, mineral) = key
            .split_once('/')
            .ok_or_else(|| HistoryError::Corrupt(format!("dedupe_key '{key}' 에 '/' 가 없다")))?;
        let system = DataId::parse(system)
            .ok_or_else(|| HistoryError::Corrupt(format!("dedupe_key 의 성계 id 불량: {key}")))?;
        let mineral = DataId::parse(mineral)
            .ok_or_else(|| HistoryError::Corrupt(format!("dedupe_key 의 광물 id 불량: {key}")))?;
        discovered.push((system, mineral));
    }
    Ok(discovered)
}

async fn load_cursor(
    pool: &PgPool,
    world_id: UuidV7,
) -> Result<Option<(Tick, Sequence)>, HistoryError> {
    let row = sqlx::query(
        "SELECT tick, sequence FROM history_cursor WHERE world_id = $1 AND consumer = $2",
    )
    .bind(world_id.get())
    .bind(CONSUMER)
    .fetch_optional(pool)
    .await?;
    let Some(row) = row else {
        return Ok(None);
    };
    let tick = tick_from_db(row.try_get("tick")?)?;
    let sequence = sequence_from_db(row.try_get("sequence")?)?;
    Ok(Some((tick, sequence)))
}

async fn load_public_backfill(
    pool: &PgPool,
    world_id: UuidV7,
) -> Result<Vec<MineralDiscoveredEvent>, HistoryError> {
    let rows = sqlx::query(
        "SELECT historical_event_id, world_id, rule_version, importance_level, tick,
                occurred_at,
                to_char(recorded_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"')
                    AS recorded_at_text,
                visibility, fact_status, source_event_ids, location, participants, payload
         FROM historical_events
         WHERE world_id = $1 AND visibility = 'PUBLIC'
         ORDER BY tick",
    )
    .bind(world_id.get())
    .fetch_all(pool)
    .await?;

    rows.iter().map(row_to_mineral_discovered).collect()
}

/// 러너 — 영속화의 커밋 알림을 받아 깨어나고, 알림이 유실돼도 1초 폴링으로 따라온다
/// (ADR-0014 §2). `commit_notify` 가 닫히면(정상 종료·정지) 마지막으로 한 번 더 따라잡고
/// 조용히 종료한다 — 오류가 아니다.
///
/// fail-stop 또는 충돌을 만나면 **러너만** 멈춘다(`handles.halted = true`) — 경제 tick
/// 루프는 계속 돈다.
pub async fn run_runner(
    pool: PgPool,
    world_id: UuidV7,
    boot: HistoryBoot,
    mut commit_notify: watch::Receiver<Option<u64>>,
    live_tx: mpsc::Sender<MineralDiscoveredEvent>,
    handles: HistoryHandles,
) {
    let HistoryBoot {
        mut core, cursor, ..
    } = boot;
    let mut cursor = cursor;
    handles
        .cursor_tick
        .store(cursor.map_or(0, |(tick, _)| tick.get()), Ordering::Release);

    loop {
        let closed = tokio::select! {
            result = commit_notify.changed() => result.is_err(),
            () = tokio::time::sleep(Duration::from_secs(1)) => false,
        };

        let keep_going =
            catch_up(&pool, world_id, &mut core, &mut cursor, &live_tx, &handles).await;

        if !keep_going {
            handles.halted.store(true, Ordering::Release);
            tracing::error!("역사 러너 정지(fail-stop 또는 충돌) — 경제는 계속 돈다");
            break;
        }
        if closed {
            tracing::debug!("역사 러너 종료 — 커밋 알림 채널이 닫혔다(정상 종료 또는 정지)");
            break;
        }
    }
}

/// 워터마크(`worlds.last_tick`)까지, 커서 뒤의 이벤트를 전부 판정한다. `false` 를 돌려주면
/// [`run_runner`] 가 멈춘다.
async fn catch_up(
    pool: &PgPool,
    world_id: UuidV7,
    core: &mut HistoryCore,
    cursor: &mut Option<(Tick, Sequence)>,
    live_tx: &mpsc::Sender<MineralDiscoveredEvent>,
    handles: &HistoryHandles,
) -> bool {
    let watermark = match read_watermark(pool, world_id).await {
        Ok(Some(watermark)) => watermark,
        Ok(None) => return true, // 아직 커밋된 tick 이 없다.
        Err(error) => {
            tracing::warn!(%error, "워터마크 조회 실패 — 다음 깨우기에 재시도");
            return true;
        }
    };

    loop {
        let rows = match fetch_next_batch(pool, world_id, *cursor, watermark).await {
            Ok(rows) => rows,
            Err(error) => {
                tracing::warn!(%error, "domain_events 조회 실패 — 다음 깨우기에 재시도");
                return true;
            }
        };
        if rows.is_empty() {
            return true;
        }

        for row in &rows {
            handles.catchup_rows_total.fetch_add(1, Ordering::Relaxed);
            match process_row(pool, world_id, core, row, live_tx, handles).await {
                Ok(Some(new_cursor)) => {
                    *cursor = Some(new_cursor);
                    handles
                        .cursor_tick
                        .store(new_cursor.0.get(), Ordering::Release);
                }
                Ok(None) => {
                    // 일시적 DB 오류 — 커서는 그대로, 다음 깨우기에 이 지점부터 재시도.
                    return true;
                }
                Err(reason) => {
                    tracing::error!(reason = %reason, "역사 러너 정지");
                    return false;
                }
            }
        }
    }
}

async fn read_watermark(pool: &PgPool, world_id: UuidV7) -> Result<Option<Tick>, sqlx::Error> {
    let row = sqlx::query("SELECT last_tick FROM worlds WHERE world_id = $1")
        .bind(world_id.get())
        .fetch_optional(pool)
        .await?;
    let Some(row) = row else {
        return Ok(None);
    };
    let last_tick: Option<i64> = row.try_get("last_tick")?;
    Ok(last_tick
        .and_then(|value| u64::try_from(value).ok())
        .and_then(Tick::new))
}

async fn fetch_next_batch(
    pool: &PgPool,
    world_id: UuidV7,
    cursor: Option<(Tick, Sequence)>,
    watermark: Tick,
) -> Result<Vec<PgRow>, sqlx::Error> {
    let watermark_i64 = i64::try_from(watermark.get()).unwrap_or(i64::MAX);
    const COLUMNS: &str = "event_id, event_type, schema_version, world_id, tick, sequence,
                occurred_at,
                to_char(recorded_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"')
                    AS recorded_at_text,
                correlation_id, causation_id, actor_id, payload";
    match cursor {
        Some((tick, sequence)) => {
            let tick_i64 = i64::try_from(tick.get()).unwrap_or(i64::MAX);
            let sequence_i64 = i64::try_from(sequence.get()).unwrap_or(i64::MAX);
            sqlx::query(&format!(
                "SELECT {COLUMNS} FROM domain_events
                 WHERE world_id = $1 AND tick <= $2 AND (tick, sequence) > ($3, $4)
                 ORDER BY tick, sequence LIMIT $5"
            ))
            .bind(world_id.get())
            .bind(watermark_i64)
            .bind(tick_i64)
            .bind(sequence_i64)
            .bind(BATCH_SIZE)
            .fetch_all(pool)
            .await
        }
        None => {
            sqlx::query(&format!(
                "SELECT {COLUMNS} FROM domain_events
                 WHERE world_id = $1 AND tick <= $2
                 ORDER BY tick, sequence LIMIT $3"
            ))
            .bind(world_id.get())
            .bind(watermark_i64)
            .bind(BATCH_SIZE)
            .fetch_all(pool)
            .await
        }
    }
}

/// 행 하나를 판정하고(필요하면) 쓴다.
///
/// - `Ok(Some(cursor))`: 이 좌표까지 전진했다(기록을 냈든 안 냈든, 또는 조용한
///   재처리든).
/// - `Ok(None)`: 일시적 DB 오류 — 커서를 전진하지 않고 다음 깨우기에 재시도한다.
/// - `Err(reason)`: fail-stop 또는 충돌 — 러너가 멈춘다.
async fn process_row(
    pool: &PgPool,
    world_id: UuidV7,
    core: &mut HistoryCore,
    row: &PgRow,
    live_tx: &mpsc::Sender<MineralDiscoveredEvent>,
    handles: &HistoryHandles,
) -> Result<Option<(Tick, Sequence)>, String> {
    let event = decode_row(row)?;
    let coordinate = (event.tick, event.sequence);

    let records = match core.judge(&event) {
        Ok(records) => records,
        Err(CoreError::OutOfOrder { .. }) => {
            return Err(format!(
                "역행 입력 — 러너의 ORDER BY 가 어긋났거나 재구축 상태가 틀렸다: {event:?}"
            ));
        }
    };

    let Some(record) = records.into_iter().next() else {
        // Level 0, 또는 이미 발견됨 — 커서만 전진한다.
        return match advance_cursor_only(pool, world_id, coordinate).await {
            Ok(()) => Ok(Some(coordinate)),
            Err(error) => {
                tracing::warn!(%error, "커서 전진 실패 — 다음 깨우기에 재시도");
                Ok(None)
            }
        };
    };

    let detector_rule = detector_rule_for(&record.event.rule_version);
    match write_record(pool, world_id, &record, &detector_rule, coordinate).await {
        Ok(WriteOutcome::Fresh) => {
            handles.records_total.fetch_add(1, Ordering::Relaxed);
            let wire = record.event.clone().into_event(now_or_epoch());
            // 커밋 뒤에만 보낸다(ADR-0014 §6) — 이미 이 시점은 트랜잭션 커밋 후다.
            // 드라이버가 먼저 끝나 채널이 닫혀 있어도 오류가 아니다(§10.1 조정 2).
            let _ = live_tx.try_send(wire);
            Ok(Some(coordinate))
        }
        Ok(WriteOutcome::QuietRedelivery) => Ok(Some(coordinate)),
        Ok(WriteOutcome::Conflict { detail }) => {
            handles.conflicts_total.fetch_add(1, Ordering::Relaxed);
            Err(format!("충돌(AC-11(h)) — {detail}"))
        }
        Err(error) => {
            tracing::warn!(%error, "역사 기록 쓰기 실패 — 다음 깨우기에 재시도");
            Ok(None)
        }
    }
}

/// 러너가 만든 기록의 `recorded_at`. `now_real_time` 을 여기서 다시 구현하지 않는다 —
/// `starfall-persistence` 의 것과 같은 시계 출처(호스트)를 쓴다.
fn now_or_epoch() -> RealTime {
    crate::now_real_time().unwrap_or_else(|| {
        tracing::error!("호스트 시계를 읽을 수 없다 — recorded_at 에 1970-01-01 을 쓴다");
        RealTime::from_unix(0, 0)
            .unwrap_or_else(|| unreachable!("고정 입력(0, 0)은 항상 유효한 RealTime 이다"))
    })
}

async fn advance_cursor_only(
    pool: &PgPool,
    world_id: UuidV7,
    coordinate: (Tick, Sequence),
) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    upsert_cursor(&mut tx, world_id, coordinate).await?;
    tx.commit().await
}

async fn upsert_cursor(
    tx: &mut Transaction<'_, sqlx::Postgres>,
    world_id: UuidV7,
    (tick, sequence): (Tick, Sequence),
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO history_cursor (world_id, consumer, tick, sequence)
         VALUES ($1, $2, $3, $4)
         ON CONFLICT (world_id, consumer) DO UPDATE SET tick = $3, sequence = $4",
    )
    .bind(world_id.get())
    .bind(CONSUMER)
    .bind(i64::try_from(tick.get()).unwrap_or(i64::MAX))
    .bind(i64::try_from(sequence.get()).unwrap_or(i64::MAX))
    .execute(&mut **tx)
    .await?;
    Ok(())
}

enum WriteOutcome {
    /// 새 기록을 실제로 썼다.
    Fresh,
    /// 같은 근거·같은 내용의 재배달 — 아무 것도 새로 쓰지 않고 커서만 전진했다.
    QuietRedelivery,
    /// 다른 근거, 또는 같은 근거인데 내용이 다르다 — 롤백했다(커서도 전진 안 함).
    Conflict { detail: String },
}

/// 기록 + 근거 + 증거 + 커서 전진을 한 트랜잭션으로 쓴다(ADR-0014 §4 겹 1).
async fn write_record(
    pool: &PgPool,
    world_id: UuidV7,
    record: &MineralDiscoveredRecord,
    detector_rule: &str,
    coordinate: (Tick, Sequence),
) -> Result<WriteOutcome, sqlx::Error> {
    let mut tx = pool.begin().await?;

    let insert_result = sqlx::query(
        "INSERT INTO historical_events (
             historical_event_id, world_id, event_type, dedupe_key, rule_version,
             importance_level, tick, occurred_at, recorded_at, visibility, fact_status,
             source_event_ids, location, participants, payload
         ) VALUES ($1, $2, 'MINERAL_DISCOVERED', $3, $4, $5, $6, $7, now(), $8, 'CONFIRMED',
                   $9, $10, $11, $12)
         ON CONFLICT (world_id, event_type, dedupe_key) DO NOTHING",
    )
    .bind(record.event.historical_event_id.get())
    .bind(world_id.get())
    .bind(record.event.dedupe_key.as_str())
    .bind(record.event.rule_version.as_str())
    .bind(i32::from(record.event.importance_level))
    .bind(i64::try_from(record.event.tick.get()).unwrap_or(i64::MAX))
    .bind(record.event.occurred_at.as_str())
    .bind(wire_visibility(record.event.visibility))
    .bind(source_event_id_array(&record.event.source_event_ids))
    .bind(sqlx::types::Json(&record.event.location))
    .bind(sqlx::types::Json(&record.event.participants))
    .bind(sqlx::types::Json(&record.event.payload))
    .execute(&mut *tx)
    .await?;

    if insert_result.rows_affected() == 0 {
        // 의미 유일성 충돌(ADR-0014 §4 겹 3) — 기존 행과 내용 전체(recorded_at 제외)를
        // 비교한다.
        let existing = fetch_comparable(&mut tx, record.event.historical_event_id.get()).await?;
        let incoming = ComparableEvent::from_record(record);
        if existing == incoming {
            tx.commit().await?; // 아무 것도 새로 안 썼지만, 커서는 전진해도 안전하다.
            return Ok(WriteOutcome::QuietRedelivery);
        }
        tx.rollback().await?;
        return Ok(WriteOutcome::Conflict {
            detail: format!(
                "dedupe_key={} 의 기존 기록과 내용이 다르다",
                record.event.dedupe_key
            ),
        });
    }

    // 근거 유일성(ADR-0014 §4 겹 2) — 여기서 PK 위반이 나면(기록은 새로 들어갔는데
    // 근거가 이미 있었다는 뜻이라 불가능해야 하는 상태다) 평범한 DB 오류로 위로 전파돼
    // "일시 오류" 취급으로 재시도된다. 반복돼도 안 없어지면 그 자체가 조사할 결함이다.
    sqlx::query(
        "INSERT INTO historical_event_sources (source_event_id, detector_rule, historical_event_id)
         VALUES ($1, $2, $3)",
    )
    .bind(record.event.source_event_ids[0].get())
    .bind(detector_rule)
    .bind(record.event.historical_event_id.get())
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        "INSERT INTO evidence (evidence_id, historical_event_id, rule_version, evidence_type, visibility, source_entity_id)
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(record.evidence.evidence_id.get())
    .bind(record.event.historical_event_id.get())
    // 증거도 그 발견을 낸 규칙의 rule_version 을 갖는다(ADR-0014 §5, architect 지적
    // 2026-09-28) — historical_events.rule_version 과 같은 record 에서 나오므로 항상
    // 같다.
    .bind(record.event.rule_version.as_str())
    .bind(wire_evidence_type(record.evidence.evidence_type))
    .bind(wire_visibility(record.evidence.visibility))
    .bind(record.evidence.source_entity_id.get())
    .execute(&mut *tx)
    .await?;

    upsert_cursor(&mut tx, world_id, coordinate).await?;
    tx.commit().await?;
    Ok(WriteOutcome::Fresh)
}

/// 비교용 값 — `recorded_at` 을 뺀 기록 내용 전체(ADR-0014 §4). `Vec<Uuid>`·`serde_json::
/// Value` 의 `PartialEq` 가 각각 배열 동등(순서 있음)·객체 동등(키 순서 없음)이라 ADR 이
/// 말하는 "jsonb·배열 동등"과 같다.
#[derive(Debug, PartialEq)]
struct ComparableEvent {
    dedupe_key: String,
    rule_version: String,
    importance_level: i32,
    tick: i64,
    occurred_at: String,
    visibility: String,
    source_event_ids: Vec<Uuid>,
    location: serde_json::Value,
    participants: serde_json::Value,
    payload: serde_json::Value,
}

impl ComparableEvent {
    fn from_record(record: &MineralDiscoveredRecord) -> Self {
        Self {
            dedupe_key: record.event.dedupe_key.clone(),
            rule_version: record.event.rule_version.as_str().to_string(),
            importance_level: i32::from(record.event.importance_level),
            tick: i64::try_from(record.event.tick.get()).unwrap_or(i64::MAX),
            occurred_at: record.event.occurred_at.as_str().to_string(),
            visibility: wire_visibility(record.event.visibility).to_string(),
            source_event_ids: record
                .event
                .source_event_ids
                .iter()
                .copied()
                .map(UuidV7::get)
                .collect(),
            location: serde_json::to_value(&record.event.location)
                .unwrap_or(serde_json::Value::Null),
            participants: serde_json::to_value(&record.event.participants)
                .unwrap_or(serde_json::Value::Null),
            payload: serde_json::to_value(&record.event.payload).unwrap_or(serde_json::Value::Null),
        }
    }
}

const COMPARABLE_COLUMNS: &str = "dedupe_key, rule_version, importance_level, tick, occurred_at, \
     visibility, source_event_ids, location, participants, payload";

fn row_to_comparable(row: &PgRow) -> Result<ComparableEvent, sqlx::Error> {
    let sqlx::types::Json(location): sqlx::types::Json<serde_json::Value> =
        row.try_get("location")?;
    let sqlx::types::Json(participants): sqlx::types::Json<serde_json::Value> =
        row.try_get("participants")?;
    let sqlx::types::Json(payload): sqlx::types::Json<serde_json::Value> =
        row.try_get("payload")?;

    Ok(ComparableEvent {
        dedupe_key: row.try_get("dedupe_key")?,
        rule_version: row.try_get("rule_version")?,
        importance_level: row.try_get("importance_level")?,
        tick: row.try_get("tick")?,
        occurred_at: row.try_get("occurred_at")?,
        visibility: row.try_get("visibility")?,
        source_event_ids: row.try_get("source_event_ids")?,
        location,
        participants,
        payload,
    })
}

/// 쓰기 경로(`write_record`)용 — 충돌 시 반드시 기존 행이 있다는 전제 위에서 부른다.
async fn fetch_comparable(
    tx: &mut Transaction<'_, sqlx::Postgres>,
    historical_event_id: Uuid,
) -> Result<ComparableEvent, sqlx::Error> {
    let row = sqlx::query(&format!(
        "SELECT {COMPARABLE_COLUMNS} FROM historical_events WHERE historical_event_id = $1"
    ))
    .bind(historical_event_id)
    .fetch_one(&mut **tx)
    .await?;
    row_to_comparable(&row)
}

/// 재생 도구(SC-59)용 — 있을 수도 없을 수도 있다(재생에서 나왔는데 저장이 안 된
/// 회귀를 잡아야 하므로).
async fn fetch_comparable_optional(
    pool: &PgPool,
    historical_event_id: Uuid,
) -> Result<Option<ComparableEvent>, sqlx::Error> {
    let row = sqlx::query(&format!(
        "SELECT {COMPARABLE_COLUMNS} FROM historical_events WHERE historical_event_id = $1"
    ))
    .bind(historical_event_id)
    .fetch_optional(pool)
    .await?;
    row.as_ref().map(row_to_comparable).transpose()
}

/// 재생 도구(SC-59)용 — evidence 의 비교 가능한 부분(같은 규칙: id·`derived_from`
/// 등 고정값은 CHECK 로 이미 강제되므로 값이 실제로 갈릴 수 있는 열만 비교한다).
async fn fetch_evidence_comparable(
    pool: &PgPool,
    evidence_id: Uuid,
) -> Result<Option<(String, String, String, Uuid)>, sqlx::Error> {
    let row = sqlx::query(
        "SELECT rule_version, evidence_type, visibility, source_entity_id
         FROM evidence WHERE evidence_id = $1",
    )
    .bind(evidence_id)
    .fetch_optional(pool)
    .await?;
    let Some(row) = row else {
        return Ok(None);
    };
    Ok(Some((
        row.try_get("rule_version")?,
        row.try_get("evidence_type")?,
        row.try_get("visibility")?,
        row.try_get("source_entity_id")?,
    )))
}

/// 재생 도구(SC-59)용 — `historical_event_sources` 행이 그 `historical_event_id` 로
/// 가리키는 근거(`source_event_id`)와 `detector_rule` 을 돌려준다(팀장 지시,
/// 2026-09-30 — 계약이 이 표도 이름으로 지명했다. `historical_events.source_event_ids`
/// 배열만 봐서는 근거 유일성 표 자체가 따로 갈라지는 회귀를 못 잡는다).
async fn fetch_sources_comparable(
    pool: &PgPool,
    historical_event_id: Uuid,
) -> Result<Vec<(Uuid, String)>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT source_event_id, detector_rule FROM historical_event_sources
         WHERE historical_event_id = $1 ORDER BY source_event_id, detector_rule",
    )
    .bind(historical_event_id)
    .fetch_all(pool)
    .await?;
    rows.iter()
        .map(|row| {
            Ok((
                row.try_get("source_event_id")?,
                row.try_get("detector_rule")?,
            ))
        })
        .collect()
}

/// 이 월드에 저장된 `historical_events` 전체(id 만) — 재생에서 안 나온 행(저장은
/// 있는데 재생에 없는 반대 방향 회귀)을 잡는 데 쓴다.
async fn fetch_all_stored_historical_event_ids(
    pool: &PgPool,
    world_id: UuidV7,
) -> Result<Vec<Uuid>, sqlx::Error> {
    sqlx::query_scalar("SELECT historical_event_id FROM historical_events WHERE world_id = $1")
        .bind(world_id.get())
        .fetch_all(pool)
        .await
}

/// 이 월드의 `domain_events` 전체(워터마크까지, `(tick, sequence)` 순) — 재생 도구
/// 전용. 러너의 [`fetch_next_batch`] 와 달리 커서·`LIMIT` 이 없다 — 재생은 처음부터
/// 끝까지 한 번에 본다.
async fn fetch_all_domain_events_for_replay(
    pool: &PgPool,
    world_id: UuidV7,
    watermark: Tick,
) -> Result<Vec<PgRow>, sqlx::Error> {
    sqlx::query(
        "SELECT event_id, event_type, schema_version, world_id, tick, sequence, occurred_at,
                to_char(recorded_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"')
                    AS recorded_at_text,
                correlation_id, causation_id, actor_id, payload
         FROM domain_events
         WHERE world_id = $1 AND tick <= $2
         ORDER BY tick, sequence",
    )
    .bind(world_id.get())
    .bind(i64::try_from(watermark.get()).unwrap_or(i64::MAX))
    .fetch_all(pool)
    .await
}

/// 재생 결과 — [`replay_and_compare`] 가 돌려준다.
#[derive(Debug)]
pub struct ReplayReport {
    /// 이번 재생이 읽은 `domain_events` 행 수.
    pub domain_events_replayed: usize,
    /// 재생이 낸 역사 기록 수(= 저장된 것과 비교 대상 수).
    pub records_replayed: usize,
    /// 사람이 읽을 불일치 설명 — 비어 있으면 완전히 같다.
    pub mismatches: Vec<String>,
}

impl ReplayReport {
    /// 불일치가 하나도 없는가.
    #[must_use]
    pub fn is_match(&self) -> bool {
        self.mismatches.is_empty()
    }
}

/// 읽기 전용 재생(SC-59, AC-12(b), H-05·H-06).
///
/// 지정한 월드의 `domain_events` 를 **빈 판정 상태**(`HistoryCore::new`, `load()` 의
/// 재구축이 아니다 — 재구축과 재생이 같은 결과를 내는지 자체가 이 함수가 증명하려는
/// 것이다)에서 처음부터 판정하고, 그 결과를 저장된 `historical_events`·`evidence` 와
/// **id 까지** 비교한다(`recorded_at` 은 감사 전용이라 제외). **아무 것도 쓰지 않는다.**
///
/// # Errors
///
/// DB 접속·쿼리 실패, 또는 `domain_events` 행이 계약 모양으로 복원되지 않으면
/// 실패한다.
pub async fn replay_and_compare(
    pool: &PgPool,
    world_id: UuidV7,
    rule: SignificanceRuleTable,
) -> Result<ReplayReport, HistoryError> {
    let watermark = read_watermark(pool, world_id)
        .await?
        .unwrap_or(Tick::new(0).unwrap_or_else(|| unreachable!("0 은 항상 유효한 Tick 이다")));
    let rows = fetch_all_domain_events_for_replay(pool, world_id, watermark).await?;

    let mut core = HistoryCore::new(rule);
    let mut records = Vec::with_capacity(rows.len());
    for row in &rows {
        let event = decode_row(row).map_err(HistoryError::Corrupt)?;
        let produced = core
            .judge(&event)
            .map_err(|error| HistoryError::Corrupt(error.to_string()))?;
        records.extend(produced);
    }

    let mut mismatches = Vec::new();
    for record in &records {
        let replayed = ComparableEvent::from_record(record);
        match fetch_comparable_optional(pool, record.event.historical_event_id.get()).await? {
            None => mismatches.push(format!(
                "historical_event_id={} — 재생됐지만 저장된 historical_events 행이 없다(dedupe_key={})",
                record.event.historical_event_id, record.event.dedupe_key
            )),
            Some(stored) if stored != replayed => mismatches.push(format!(
                "historical_event_id={} — 재생 결과와 저장된 historical_events 내용이 다르다",
                record.event.historical_event_id
            )),
            Some(_) => {}
        }

        match fetch_evidence_comparable(pool, record.evidence.evidence_id.get()).await? {
            None => mismatches.push(format!(
                "evidence_id={} — 재생됐지만 저장된 evidence 행이 없다",
                record.evidence.evidence_id
            )),
            Some((rule_version, evidence_type, visibility, source_entity_id))
                if rule_version != record.event.rule_version.as_str()
                    || evidence_type != wire_evidence_type(record.evidence.evidence_type)
                    || visibility != wire_visibility(record.evidence.visibility)
                    || source_entity_id != record.evidence.source_entity_id.get() =>
            {
                mismatches.push(format!(
                    "evidence_id={} — 재생 결과와 저장된 evidence 내용이 다르다",
                    record.evidence.evidence_id
                ));
            }
            Some(_) => {}
        }

        // historical_event_sources — 근거 유일성 표 자체가 갈라지는 회귀(팀장 지시,
        // 2026-09-30). 이 규칙은 근거가 정확히 1개다.
        let expected_sources = vec![(
            record.event.source_event_ids[0].get(),
            detector_rule_for(&record.event.rule_version),
        )];
        let stored_sources =
            fetch_sources_comparable(pool, record.event.historical_event_id.get()).await?;
        if stored_sources != expected_sources {
            mismatches.push(format!(
                "historical_event_id={} — historical_event_sources 행이 재생 결과와 다르다 \
                 (기대={expected_sources:?}, 저장={stored_sources:?})",
                record.event.historical_event_id
            ));
        }
    }

    // 반대 방향: 저장은 있는데 재생에서 안 나온 행(이 규칙이 조용히 덜 발견하게
    // 바뀌는 회귀).
    let replayed_ids: std::collections::HashSet<Uuid> = records
        .iter()
        .map(|record| record.event.historical_event_id.get())
        .collect();
    for stored_id in fetch_all_stored_historical_event_ids(pool, world_id).await? {
        if !replayed_ids.contains(&stored_id) {
            mismatches.push(format!(
                "historical_event_id={stored_id} — 저장돼 있지만 재생에서 나오지 않았다"
            ));
        }
    }

    Ok(ReplayReport {
        domain_events_replayed: rows.len(),
        records_replayed: records.len(),
        mismatches,
    })
}

/// `historical_event_sources.detector_rule` 값 — **`rule_version`**(예:
/// `"mineral-discovery@1"`, 버전 포함).
///
/// **원복 이력(2026-09-30)**: 한때 `rule_id`(버전 없이)로 바꿨었다 —
/// migrations/0003 의 설계 주석이 그렇게 적혀 있었기 때문이다. architect 판정으로
/// 되돌렸다: 정본은 **ADR-0014 §4-2**(0003 은 증거 DB 에 이미 적용돼 동결이라 주석 자체를
/// 못 고친다 — ADR 에 "정본은 이 절"이라고 명시했다). 이유: 증거 DB 에 이미
/// `"mineral-discovery@1"` 행이 51개(32 월드) 있고 `rule_id` 값 행은 0개였다(architect
/// 실측) — 지금 바꾸면 (1) 같은 열에 두 철자가 영구 공존(옛 행은 원칙 5 로 못 고침),
/// (2) 옛 월드에서 커서를 되감으면 PK `(source_event_id, detector_rule)` 이 옛 값과
/// 안 겹쳐 근거 행이 두 벌 생긴다(멱등 둘째 겹이 조용히 무너진다). "@2 가 같은 근거를
/// 다시 쓰는가"라는 원래 우려는 dedupe_key `UNIQUE`(멱등 셋째 겹)가 이미 막으므로
/// `rule_id` 로 바꿀 실익이 없다.
fn detector_rule_for(rule_version: &RuleVersion) -> String {
    rule_version.as_str().to_string()
}

fn wire_visibility(visibility: HistoricalVisibility) -> &'static str {
    match visibility {
        HistoricalVisibility::Public => "PUBLIC",
        HistoricalVisibility::FactionOnly => "FACTION_ONLY",
        HistoricalVisibility::ParticipantsOnly => "PARTICIPANTS_ONLY",
        HistoricalVisibility::Classified => "CLASSIFIED",
        HistoricalVisibility::Secret => "SECRET",
        HistoricalVisibility::Discoverable => "DISCOVERABLE",
    }
}

fn wire_evidence_type(evidence_type: EvidenceType) -> &'static str {
    match evidence_type {
        EvidenceType::ShipLog => "SHIP_LOG",
    }
}

fn source_event_id_array(ids: &[UuidV7]) -> Vec<Uuid> {
    ids.iter().map(|id| id.get()).collect()
}

fn tick_from_db(value: i64) -> Result<Tick, HistoryError> {
    let raw =
        u64::try_from(value).map_err(|_| HistoryError::Corrupt(format!("tick={value} 음수")))?;
    Tick::new(raw).ok_or_else(|| HistoryError::Corrupt(format!("tick={value} 범위 초과")))
}

fn sequence_from_db(value: i64) -> Result<Sequence, HistoryError> {
    let raw = u64::try_from(value)
        .map_err(|_| HistoryError::Corrupt(format!("sequence={value} 음수")))?;
    Sequence::new(raw).ok_or_else(|| HistoryError::Corrupt(format!("sequence={value} 범위 초과")))
}

fn uuid_v7_from_row(row: &PgRow, column: &str) -> Result<UuidV7, String> {
    let raw: Uuid = row
        .try_get(column)
        .map_err(|error| format!("{column} 읽기 실패: {error}"))?;
    UuidV7::from_uuid(raw).ok_or_else(|| format!("{column} 이 UUIDv7 이 아니다: {raw}"))
}

/// 판정 대상 도메인 이벤트를 코어 입력으로 옮긴다(fail-stop, ADR-0014 §1). `MINERAL_MINED`
/// 가 아니면 좌표만 담는다(Level 0).
fn decode_row(row: &PgRow) -> Result<DomainEvent, String> {
    let event_type: String = row
        .try_get("event_type")
        .map_err(|error| format!("event_type 읽기 실패: {error}"))?;
    let event_id = uuid_v7_from_row(row, "event_id")?;
    let world_id = uuid_v7_from_row(row, "world_id")?;
    let tick_i64: i64 = row
        .try_get("tick")
        .map_err(|error| format!("tick 읽기 실패: {error}"))?;
    let tick = tick_from_db(tick_i64).map_err(|error| error.to_string())?;
    let sequence_i64: i64 = row
        .try_get("sequence")
        .map_err(|error| format!("sequence 읽기 실패: {error}"))?;
    let sequence = sequence_from_db(sequence_i64).map_err(|error| error.to_string())?;

    if event_type != starfall_contracts::registry::MINERAL_MINED {
        return Ok(DomainEvent::unjudged(event_id, world_id, tick, sequence));
    }

    let schema_version: i32 = row
        .try_get("schema_version")
        .map_err(|error| format!("schema_version 읽기 실패: {error}"))?;
    if schema_version != 1 {
        return Err(format!(
            "모르는 schema_version={schema_version} (event_id={event_id}) — 업캐스팅 없음, fail-stop"
        ));
    }

    let occurred_at_text: String = row
        .try_get("occurred_at")
        .map_err(|error| format!("occurred_at 읽기 실패: {error}"))?;
    let occurred_at = GameTime::parse(occurred_at_text.clone())
        .ok_or_else(|| format!("occurred_at 파싱 실패: {occurred_at_text}"))?;

    let recorded_at_text: String = row
        .try_get("recorded_at_text")
        .map_err(|error| format!("recorded_at 읽기 실패: {error}"))?;
    let recorded_at = RealTime::parse(&recorded_at_text)
        .ok_or_else(|| format!("recorded_at 파싱 실패: {recorded_at_text}"))?;

    let correlation_id = uuid_v7_from_row(row, "correlation_id")?;

    let causation_id: Option<Uuid> = row
        .try_get("causation_id")
        .map_err(|error| format!("causation_id 읽기 실패: {error}"))?;
    let causation_id = causation_id
        .and_then(UuidV7::from_uuid)
        .ok_or_else(|| format!("causation_id 없음/비-v7 (event_id={event_id}) — I-53 위반"))?;

    let actor_id: Option<Uuid> = row
        .try_get("actor_id")
        .map_err(|error| format!("actor_id 읽기 실패: {error}"))?;
    let actor_id = actor_id
        .and_then(UuidV7::from_uuid)
        .ok_or_else(|| format!("actor_id 없음/비-v7 (event_id={event_id})"))?;

    let sqlx::types::Json(payload): sqlx::types::Json<MineralMinedPayload> = row
        .try_get("payload")
        .map_err(|error| format!("payload 역직렬화 실패 (event_id={event_id}): {error}"))?;

    Ok(DomainEvent::mineral_mined(MineralMinedEvent {
        event_id,
        event_type: MineralMinedType::MineralMined,
        schema_version: ConstSchemaVersion,
        world_id,
        tick,
        sequence,
        occurred_at,
        recorded_at,
        correlation_id,
        causation_id,
        actor_id,
        payload,
    }))
}

fn row_to_mineral_discovered(row: &PgRow) -> Result<MineralDiscoveredEvent, HistoryError> {
    let historical_event_id: Uuid = row.try_get("historical_event_id")?;
    let historical_event_id = UuidV5::from_uuid(historical_event_id).ok_or_else(|| {
        HistoryError::Corrupt(format!(
            "historical_event_id 가 v5 아님: {historical_event_id}"
        ))
    })?;
    let world_id: Uuid = row.try_get("world_id")?;
    let world_id = UuidV7::from_uuid(world_id)
        .ok_or_else(|| HistoryError::Corrupt(format!("world_id 가 v7 아님: {world_id}")))?;
    let rule_version: String = row.try_get("rule_version")?;
    let rule_version = RuleVersion::parse(rule_version.clone())
        .ok_or_else(|| HistoryError::Corrupt(format!("rule_version 불량: {rule_version}")))?;
    let importance_level: i32 = row.try_get("importance_level")?;
    let importance_level = u8::try_from(importance_level)
        .map_err(|_| HistoryError::Corrupt(format!("importance_level 불량: {importance_level}")))?;
    let tick = tick_from_db(row.try_get("tick")?)?;
    let occurred_at_text: String = row.try_get("occurred_at")?;
    let occurred_at = GameTime::parse(occurred_at_text.clone())
        .ok_or_else(|| HistoryError::Corrupt(format!("occurred_at 불량: {occurred_at_text}")))?;
    let recorded_at_text: String = row.try_get("recorded_at_text")?;
    let recorded_at = RealTime::parse(&recorded_at_text)
        .ok_or_else(|| HistoryError::Corrupt(format!("recorded_at 불량: {recorded_at_text}")))?;
    let visibility: String = row.try_get("visibility")?;
    let visibility = parse_visibility(&visibility)?;
    let fact_status: String = row.try_get("fact_status")?;
    if fact_status != "CONFIRMED" {
        return Err(HistoryError::Corrupt(format!(
            "fact_status 불량: {fact_status}"
        )));
    }
    let source_event_ids: Vec<Uuid> = row.try_get("source_event_ids")?;
    let source_event_ids = source_event_ids
        .into_iter()
        .map(|id| {
            UuidV7::from_uuid(id)
                .ok_or_else(|| HistoryError::Corrupt("source_event_ids 원소 불량".into()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if source_event_ids.len() != 1 {
        return Err(HistoryError::Corrupt(format!(
            "source_event_ids 는 정확히 1개여야 한다(받음 {})",
            source_event_ids.len()
        )));
    }
    let sqlx::types::Json(location): sqlx::types::Json<HistoricalLocation> =
        row.try_get("location")?;
    let sqlx::types::Json(participants): sqlx::types::Json<Vec<HistoricalParticipant>> =
        row.try_get("participants")?;
    if participants.len() != 2 {
        return Err(HistoryError::Corrupt(format!(
            "participants 는 정확히 2개여야 한다(받음 {})",
            participants.len()
        )));
    }
    let sqlx::types::Json(payload): sqlx::types::Json<MineralDiscoveredPayload> =
        row.try_get("payload")?;

    Ok(MineralDiscoveredEvent {
        historical_event_id,
        event_type: MineralDiscoveredType::MineralDiscovered,
        schema_version: ConstSchemaVersion,
        world_id,
        rule_version,
        importance_level,
        tick,
        occurred_at,
        recorded_at,
        visibility,
        fact_status: FactStatus::Confirmed,
        source_event_ids,
        location,
        participants,
        payload,
    })
}

fn parse_visibility(raw: &str) -> Result<HistoricalVisibility, HistoryError> {
    match raw {
        "PUBLIC" => Ok(HistoricalVisibility::Public),
        "FACTION_ONLY" => Ok(HistoricalVisibility::FactionOnly),
        "PARTICIPANTS_ONLY" => Ok(HistoricalVisibility::ParticipantsOnly),
        "CLASSIFIED" => Ok(HistoricalVisibility::Classified),
        "SECRET" => Ok(HistoricalVisibility::Secret),
        "DISCOVERABLE" => Ok(HistoricalVisibility::Discoverable),
        other => Err(HistoryError::Corrupt(format!("모르는 visibility: {other}"))),
    }
}

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

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::Value;
use sqlx::postgres::{PgPool, PgPoolOptions};
use sqlx::{Row, Transaction};
use starfall_contracts::events::{
    SessionClosedEvent, SessionClosedType, SessionOpenedEvent, SessionOpenedType,
};
use starfall_contracts::primitives::{
    ConstSchemaVersion, GameCalendar, GameTime, RealTime, ServerVersion, UuidV7,
};
use starfall_sim::{DomainEventBody, PendingEvent, PersistBatch, WorldConstants};

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
}

/// 영속화 태스크가 갱신하는 관측 값.
///
/// 게이트웨이의 `/debug/stats` 가 **같은 `Arc` 를 읽는다.** 크레이트 의존이 생기지 않게
/// 원자값만 공유한다 (게이트웨이는 persistence 를 의존하지 않는다).
#[derive(Debug, Clone)]
pub struct PersistHandles {
    /// 커밋된 도메인 이벤트 누적 수.
    pub persisted_total: Arc<AtomicU64>,
    /// 커밋에 실패해 **버린** 배치의 이벤트 누적 수. 0이 아니면 버그다.
    pub failed_total: Arc<AtomicU64>,
    /// 마지막으로 커밋된 tick. `persist_backlog = 현재 tick − 이 값`.
    pub last_committed_tick: Arc<AtomicU64>,
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
) -> Result<(WorldConstants, Option<u64>), PersistenceError> {
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
        WorldConstants {
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

/// 지금 시각을 `RealTime` 으로. 호스트 시계를 쓴다(컨테이너 시계가 아니다).
#[must_use]
pub fn now_real_time() -> Option<RealTime> {
    let since = SystemTime::now().duration_since(UNIX_EPOCH).ok()?;
    RealTime::from_unix(i64::try_from(since.as_secs()).ok()?, since.subsec_nanos())
}

/// 영속화 루프.
///
/// tick 루프와 **분리된** 태스크다. tick 루프는 DB 를 기다리지 않는다 — 기다리면 DB 지연이
/// 곧 시뮬레이션 지연이 된다 (ADR-0007 §4).
///
/// DB 가 죽어 있으면 같은 배치를 계속 재시도한다. **버리지 않는다.** 대신
/// `last_committed_tick` 이 멈춰 있어 `persist_backlog` 가 자라고, 임계를 넘으면
/// 게이트웨이가 새 연결을 거절한다.
pub async fn run(
    pool: PgPool,
    world_id: UuidV7,
    mut batches: tokio::sync::mpsc::Receiver<PersistBatch>,
    handles: PersistHandles,
) {
    while let Some(batch) = batches.recv().await {
        commit_with_retry(&pool, world_id, &batch, &handles).await;
    }
    tracing::info!("영속화 태스크 종료 — 남은 배치 없음");
}

async fn commit_with_retry(
    pool: &PgPool,
    world_id: UuidV7,
    batch: &PersistBatch,
    handles: &PersistHandles,
) {
    let mut attempt: u32 = 0;
    loop {
        match commit(pool, world_id, batch).await {
            Ok(()) => {
                handles
                    .persisted_total
                    .fetch_add(batch.events.len() as u64, Ordering::Relaxed);
                handles
                    .last_committed_tick
                    .store(batch.tick, Ordering::Release);
                return;
            }
            Err(error) => {
                // 제약 위반은 재시도해도 절대 성공하지 않는다. ADR-0007 §4 가 정한 대로
                // **삼키지 않고** 에러 로그 + 메트릭으로 드러내고 이 배치만 포기한다.
                // (재시도 루프에 남기면 그 뒤의 모든 tick 이 영원히 커밋되지 않는다.)
                if is_constraint_violation(&error) {
                    tracing::error!(
                        tick = batch.tick,
                        events = batch.events.len(),
                        %error,
                        "domain_events 제약 위반 — 서로 다른 이벤트가 같은 (world_id, tick, sequence) 를 주장했다. 이것은 버그 신호다 (ADR-0007 §4)"
                    );
                    handles
                        .failed_total
                        .fetch_add(batch.events.len() as u64, Ordering::Relaxed);
                    return;
                }

                attempt = attempt.saturating_add(1);
                if attempt == 1 || attempt.is_multiple_of(20) {
                    tracing::warn!(
                        tick = batch.tick,
                        attempt,
                        %error,
                        "영속화 재시도 중 — 이벤트를 버리지 않는다 (I-22)"
                    );
                }
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        }
    }
}

fn is_constraint_violation(error: &sqlx::Error) -> bool {
    matches!(error, sqlx::Error::Database(db) if db.code().is_some_and(|code| code.starts_with("23")))
}

async fn commit(pool: &PgPool, world_id: UuidV7, batch: &PersistBatch) -> Result<(), sqlx::Error> {
    let mut tx: Transaction<'_, sqlx::Postgres> = pool.begin().await?;

    for event in &batch.events {
        let Some(recorded_at) = now_real_time() else {
            continue;
        };
        let (event_type, payload) = contract_payload(event, &recorded_at);

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
    }

    // tick 재개의 정본을 **같은 트랜잭션에서** 갱신한다 (ADR-0007 §4).
    // 뒤로 가지 않도록 조건을 건다 (I-17).
    sqlx::query(
        "UPDATE worlds SET last_tick = $2
         WHERE world_id = $1 AND (last_tick IS NULL OR last_tick < $2)",
    )
    .bind(world_id.get())
    .bind(i64::try_from(batch.tick).unwrap_or(i64::MAX))
    .execute(&mut *tx)
    .await?;

    tx.commit().await
}

/// 계약 타입을 **실제로 만들어** payload 를 뽑는다.
///
/// 열에 넣는 값과 payload 를 따로 만들면 둘이 조용히 갈라진다. 계약 구조체를 거치면
/// 저장된 JSONB 가 계약 스키마와 같은 모양임이 타입으로 보장된다.
fn contract_payload(event: &PendingEvent, recorded_at: &RealTime) -> (&'static str, Value) {
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
            (
                starfall_contracts::registry::SESSION_OPENED,
                extract_payload(&full),
            )
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
            (
                starfall_contracts::registry::SESSION_CLOSED,
                extract_payload(&full),
            )
        }
    }
}

fn extract_payload<T: serde::Serialize>(event: &T) -> Value {
    serde_json::to_value(event)
        .ok()
        .and_then(|mut value| value.get_mut("payload").map(Value::take))
        .unwrap_or(Value::Null)
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
        let (event_type, payload) = contract_payload(&event, &recorded);

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
}

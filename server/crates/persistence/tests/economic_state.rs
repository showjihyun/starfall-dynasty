//! S4 DB 통합 테스트 — 경제 상태(ADR-0013, `02_server_ack.md` K1~K6).
//!
//! `starfall_persistence::run()`(공개 API) 을 실제 채널로 구동한다 — 실제 바이너리가
//! 쓰는 것과 같은 경로다. 격리 DB는 `starfall-testdb` 가 만든다(증거 DB `starfall` 은
//! 건드리지 않는다).

// 테스트 코드에서는 unwrap/expect/panic 을 쓴다(`contract_tests.rs` 와 같은 관례) —
// 실패 지점을 흐리지 않기 위해서다. 이 파일의 모든 함수는 테스트이거나 테스트 전용
// 헬퍼라서 clippy.toml 의 `allow-*-in-tests`(그 자체는 `#[test]` 함수 본문에만 적용된다)
// 로는 부족하다.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use sqlx::Row;
use starfall_contracts::events::{MineralMinedPayload, SessionCloseReason, SessionClosedPayload};
use starfall_contracts::primitives::{DataId, GameTime, MassKg, Sequence, Tick, UuidV7};
use starfall_persistence::PersistHandles;
use starfall_sim::{DomainEventBody, PendingEvent, PersistBatch, StateWrite};
use tokio::sync::{mpsc, watch};

fn id(text: &str) -> UuidV7 {
    UuidV7::parse(text).unwrap_or_else(|| panic!("정규 UUIDv7 이어야 한다: {text}"))
}

/// **0001 이 시드하는 스파이크 월드(`01a0b1c2-3d4e-…`)와 다른 id 를 쓴다** — 같은 id 를
/// 쓰면 `seed_world` 의 INSERT 가 그 시드 행과 PK 충돌한다(실측: `worlds_pkey` 위반).
fn world_id() -> UuidV7 {
    id("01a0e7f3-7869-7105-9103-717c87929da8")
}

fn mass(value: i32) -> MassKg {
    MassKg::new(value).unwrap()
}

async fn seed_world(pool: &sqlx::PgPool, world: UuidV7) {
    sqlx::query(
        "INSERT INTO worlds (world_id, name, tick_hz, calendar_epoch, calendar_scale, sim_version, last_tick)
         VALUES ($1, 'test', 20, '3800-01-01T00:00:00Z', 60, 1, NULL)",
    )
    .bind(world.get())
    .execute(pool)
    .await
    .expect("worlds 시드 실패");
}

fn handles() -> PersistHandles {
    PersistHandles {
        persisted_total: Arc::new(AtomicU64::new(0)),
        failed_total: Arc::new(AtomicU64::new(0)),
        last_committed_tick: Arc::new(AtomicU64::new(0)),
        halted: Arc::new(AtomicBool::new(false)),
        ambiguous_commits_total: Arc::new(AtomicU64::new(0)),
    }
}

/// 채굴 1건의 `MINERAL_MINED` 이벤트 + 두 `StateWrite`(인벤토리·매장지, 둘 다 첫 행 —
/// `expected: None`). 기본 월드([`world_id`])로 만든다.
fn first_mine_batch(tick: u64, command_id: UuidV7, actor_id: UuidV7) -> PersistBatch {
    first_mine_batch_in_world(world_id(), tick, command_id, actor_id)
}

/// [`first_mine_batch`] 와 같지만 월드를 고른다 —
/// `cross_world_same_command_id_accepted` 처럼 월드가 둘 이상 필요한 테스트용.
fn first_mine_batch_in_world(
    world: UuidV7,
    tick: u64,
    command_id: UuidV7,
    actor_id: UuidV7,
) -> PersistBatch {
    let event = PendingEvent {
        event_id: id("01a0b1c2-a001-7e01-8f11-505162738495"),
        world_id: world,
        tick: Tick::new(tick).unwrap(),
        sequence: Sequence::new(0).unwrap(),
        occurred_at: GameTime::parse("3800-01-01T20:00:00Z").unwrap(),
        correlation_id: id("01a0b1c2-4a12-7c01-8d02-1e033f044a05"),
        causation_id: Some(command_id),
        actor_id,
        body: DomainEventBody::MineralMined(MineralMinedPayload {
            ship_id: id("01a0b1c2-8b01-7a11-8b22-9c33d44e55f6"),
            session_id: id("01a0b1c2-4a11-7b22-9c33-0d44e55f6a77"),
            star_system_id: DataId::parse("cradle").unwrap(),
            deposit_id: DataId::parse("far-reach").unwrap(),
            mineral_id: DataId::parse("starfall-glass").unwrap(),
            quantity_kg: mass(25),
            quantity_before_kg: mass(0),
            quantity_after_kg: mass(25),
            deposit_remaining_before_kg: mass(500),
            deposit_remaining_after_kg: mass(475),
        }),
    };
    PersistBatch {
        tick,
        events: vec![event],
        state_writes: vec![
            StateWrite::Inventory {
                actor_id,
                mineral_id: DataId::parse("starfall-glass").unwrap(),
                expected: None,
                new: 25,
            },
            StateWrite::Deposit {
                deposit_id: DataId::parse("far-reach").unwrap(),
                expected: None,
                new: (475, tick),
                first_extracted_tick: tick,
            },
        ],
    }
}

/// [`starfall_persistence::run`] 을 배경 태스크로 돌리고, 배치를 보낸 뒤 채널을 닫아
/// 끝날 때까지 기다린다. 종료 뒤 핸들을 그대로 돌려준다(검사용).
async fn run_batches(
    pool: sqlx::PgPool,
    world: UuidV7,
    batches: Vec<PersistBatch>,
) -> PersistHandles {
    let handles = handles();
    let (tx, rx) = mpsc::channel(16);
    let (commit_notify_tx, _commit_notify_rx) = watch::channel(None);
    let task = tokio::spawn(starfall_persistence::run(
        pool,
        world,
        rx,
        handles.clone(),
        commit_notify_tx,
    ));
    for batch in batches {
        tx.send(batch).await.expect("영속화 채널이 닫혔다");
    }
    drop(tx);
    task.await.expect("영속화 태스크가 패닉했다");
    handles
}

#[tokio::test]
async fn accepted_mine_commits_events_and_state() {
    // commit() 을 부르는 모든 테스트가 이 락을 잡는다 — test_hooks 의 전역 주입
    // 플래그는 이 파일 전체에서 공유되므로, 이 락 없이 병렬로 돌면 이 테스트가 다른
    // 테스트가 무장한 주입을 대신 소비하거나(또는 그 반대) 할 수 있다(실측:
    // `batch_recommit_is_noop`/`ambiguous_commit_retry_counts_once` 가 기본
    // `--test-threads` 에서 서로의 카운터를 밟았다).
    let _serialize = starfall_persistence::test_hooks::INJECTION_LOCK
        .lock()
        .await;
    // 계약 밖 테스트(qa 확인) — census 는 "계약 밖 이름"으로 출력만 하고 FAIL 을
    // 내지 않는다. 그래도 qa 요청대로 모듈 경로는 붙이지 않는다(census 가 글자 그대로
    // 대조하므로 계약에 들어갈 경우 이 이름 그대로 쓸 수 있게).
    let name = "accepted_mine_commits_events_and_state";
    let Some(db) = starfall_testdb::TestDb::create(name).await else {
        return;
    };
    seed_world(&db.pool, world_id()).await;

    let command_id = id("01a0b1c2-9c01-7d11-8e21-3f3140516171");
    let actor_id = id("01a0b1c2-2c01-7a45-8b67-89abcdef0123");
    let batch = first_mine_batch(24000, command_id, actor_id);

    let handles = run_batches(db.pool.clone(), world_id(), vec![batch]).await;

    assert_eq!(handles.persisted_total.load(Ordering::Relaxed), 1);
    assert_eq!(handles.failed_total.load(Ordering::Relaxed), 0);
    assert!(!handles.halted.load(Ordering::Relaxed));
    assert_eq!(handles.last_committed_tick.load(Ordering::Relaxed), 24000);

    let events: i64 = sqlx::query_scalar("SELECT count(*) FROM domain_events WHERE world_id = $1")
        .bind(world_id().get())
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(events, 1, "MINERAL_MINED 1건이 커밋돼야 한다");

    let inventory: i64 = sqlx::query_scalar(
        "SELECT quantity_kg FROM inventory_items WHERE world_id = $1 AND actor_id = $2 AND mineral_id = 'starfall-glass'",
    )
    .bind(world_id().get())
    .bind(actor_id.get())
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(inventory, 25);

    let row = sqlx::query(
        "SELECT remaining_kg, as_of_tick, first_extracted_tick FROM deposit_states WHERE world_id = $1 AND deposit_id = 'far-reach'",
    )
    .bind(world_id().get())
    .fetch_one(&db.pool)
    .await
    .unwrap();
    let remaining: i64 = row.try_get("remaining_kg").unwrap();
    let as_of_tick: i64 = row.try_get("as_of_tick").unwrap();
    let first_extracted_tick: i64 = row.try_get("first_extracted_tick").unwrap();
    assert_eq!(remaining, 475);
    assert_eq!(as_of_tick, 24000);
    assert_eq!(first_extracted_tick, 24000);

    let processed: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM processed_commands WHERE world_id = $1 AND command_id = $2",
    )
    .bind(world_id().get())
    .bind(command_id.get())
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(
        processed, 1,
        "MINE_RESOURCE의 command_id가 처리 장부에도 같은 트랜잭션에서 남아야 한다(K1)"
    );

    db.drop().await;
}

/// I-57/ADR-0013 §4 — 이미 커밋된 tick의 배치를 다시 커밋하면 행 변화 0, `AlreadyCommitted`
/// 로 건너뛴다(K6 — `ambiguous_commits_total` 로 관측 가능해야 한다).
#[tokio::test]
async fn batch_recommit_is_noop() {
    // commit() 을 부르는 모든 테스트가 이 락을 잡는다 — test_hooks 의 전역 주입
    // 플래그는 이 파일 전체에서 공유되므로, 이 락 없이 병렬로 돌면 이 테스트가 다른
    // 테스트가 무장한 주입을 대신 소비하거나(또는 그 반대) 할 수 있다(실측:
    // `batch_recommit_is_noop`/`ambiguous_commit_retry_counts_once` 가 기본
    // `--test-threads` 에서 서로의 카운터를 밟았다).
    let _serialize = starfall_persistence::test_hooks::INJECTION_LOCK
        .lock()
        .await;
    // SC-20 — 계약 이름 그대로(모듈 경로 없이, qa 요청).
    let name = "batch_recommit_is_noop";
    let Some(db) = starfall_testdb::TestDb::create(name).await else {
        return;
    };
    seed_world(&db.pool, world_id()).await;

    let command_id = id("01a0b1c2-9c01-7d11-8e21-3f3140516171");
    let actor_id = id("01a0b1c2-2c01-7a45-8b67-89abcdef0123");
    let batch = first_mine_batch(24000, command_id, actor_id);
    let batch_again = first_mine_batch(24000, command_id, actor_id);

    let handles = run_batches(db.pool.clone(), world_id(), vec![batch, batch_again]).await;

    // 첫 배치만 실제로 썼다(이벤트 1건, DB 행도 1건) — 하지만 `persisted_total` 은
    // **처리 성공한 배치의 이벤트 수**를 재지 실제로 쓴 행 수를 재지 않는다(ADR-0013
    // §4). 둘째 배치(재시도)는 AlreadyCommitted 로 건너뛰지만 그것도 "성공"이므로 다시
    // 더해져 1(첫 배치) + 1(둘째, 건너뜀) = 2 다. 이 테스트 모양(**같은 배치를 두 번
    // 그대로 보낸다**)에서는 `persisted_total == COUNT(domain_events)` 항등식이
    // 성립하지 않는다(2 != 1) — 그 항등식은 `ambiguous_commit_retry_counts_once` 가
    // 다른 모양(모호한 커밋 재시도 1회만)으로 확인한다(server 주석, SC-20 둘째 테스트).
    assert_eq!(handles.persisted_total.load(Ordering::Relaxed), 2);
    assert_eq!(handles.ambiguous_commits_total.load(Ordering::Relaxed), 1);
    assert!(!handles.halted.load(Ordering::Relaxed));

    let events: i64 = sqlx::query_scalar("SELECT count(*) FROM domain_events WHERE world_id = $1")
        .bind(world_id().get())
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(events, 1, "둘째 배치는 아무 것도 쓰지 않아야 한다");

    let inventory: i64 = sqlx::query_scalar(
        "SELECT quantity_kg FROM inventory_items WHERE world_id = $1 AND actor_id = $2 AND mineral_id = 'starfall-glass'",
    )
    .bind(world_id().get())
    .bind(actor_id.get())
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(
        inventory, 25,
        "인벤토리도 25에서 멈춰야 한다(두 번 더해지면 안 된다)"
    );

    db.drop().await;
}

/// K3 — 비교 후 쓰기 기대값이 저장된 값과 다르면(예: SQL로 직접 변조) 그 배치는
/// **복구 불가**로 정지한다(halted=true). 이후 배치는 아예 처리되지 않는다(K5).
#[tokio::test]
async fn cas_mismatch_halts_the_world() {
    // commit() 을 부르는 모든 테스트가 이 락을 잡는다 — test_hooks 의 전역 주입
    // 플래그는 이 파일 전체에서 공유되므로, 이 락 없이 병렬로 돌면 이 테스트가 다른
    // 테스트가 무장한 주입을 대신 소비하거나(또는 그 반대) 할 수 있다(실측:
    // `batch_recommit_is_noop`/`ambiguous_commit_retry_counts_once` 가 기본
    // `--test-threads` 에서 서로의 카운터를 밟았다).
    let _serialize = starfall_persistence::test_hooks::INJECTION_LOCK
        .lock()
        .await;
    starfall_persistence::test_hooks::reset();
    // SC-25 — 계약 이름은 `cas_mismatch_halts`(qa 대조, fn 이름은 더 설명적으로 둔다).
    let name = "cas_mismatch_halts";
    let Some(db) = starfall_testdb::TestDb::create(name).await else {
        return;
    };
    seed_world(&db.pool, world_id()).await;

    let actor_id = id("01a0b1c2-2c01-7a45-8b67-89abcdef0123");
    let first = first_mine_batch(24000, id("01a0b1c2-9c01-7d11-8e21-3f3140516171"), actor_id);
    let handles_first = run_batches(db.pool.clone(), world_id(), vec![first]).await;
    assert!(!handles_first.halted.load(Ordering::Relaxed));

    // SQL로 인벤토리를 직접 변조한다(AC-5(b)와 같은 시나리오) — 다음 채굴의 `expected`
    // (25)가 저장된 값과 어긋나게 만든다.
    sqlx::query(
        "UPDATE inventory_items SET quantity_kg = 999 WHERE world_id = $1 AND actor_id = $2 AND mineral_id = 'starfall-glass'",
    )
    .bind(world_id().get())
    .bind(actor_id.get())
    .execute(&db.pool)
    .await
    .unwrap();

    let second_command_id = id("01a0b1c2-9c02-7d12-9e22-4f4150617181");
    let second_event = PendingEvent {
        event_id: id("01a0b1c2-a002-7e02-8f12-505162738496"),
        world_id: world_id(),
        tick: Tick::new(24060).unwrap(),
        sequence: Sequence::new(0).unwrap(),
        occurred_at: GameTime::parse("3800-01-01T20:00:03Z").unwrap(),
        correlation_id: id("01a0b1c2-4a12-7c01-8d02-1e033f044a05"),
        causation_id: Some(second_command_id),
        actor_id,
        body: DomainEventBody::MineralMined(MineralMinedPayload {
            ship_id: id("01a0b1c2-8b01-7a11-8b22-9c33d44e55f6"),
            session_id: id("01a0b1c2-4a11-7b22-9c33-0d44e55f6a77"),
            star_system_id: DataId::parse("cradle").unwrap(),
            deposit_id: DataId::parse("far-reach").unwrap(),
            mineral_id: DataId::parse("starfall-glass").unwrap(),
            quantity_kg: mass(25),
            quantity_before_kg: mass(25),
            quantity_after_kg: mass(50),
            deposit_remaining_before_kg: mass(475),
            deposit_remaining_after_kg: mass(450),
        }),
    };
    let second = PersistBatch {
        tick: 24060,
        events: vec![second_event],
        state_writes: vec![StateWrite::Inventory {
            actor_id,
            mineral_id: DataId::parse("starfall-glass").unwrap(),
            expected: Some(25), // 저장된 값(999)과 다르다 — CAS 불일치.
            new: 50,
        }],
    };
    let third = first_mine_batch(24120, id("01a0b1c2-9c03-7d13-8e23-505162738497"), actor_id);

    let handles = run_batches(db.pool.clone(), world_id(), vec![second, third]).await;

    assert!(
        handles.halted.load(Ordering::Relaxed),
        "CAS 불일치는 월드를 멈춰야 한다(K3)"
    );
    assert!(handles.failed_total.load(Ordering::Relaxed) >= 1);

    // SC-25 — 정지 로그 한 줄에 사유·키·기대값·**DB 값**·persist_fatal_total 이 있어야
    // 한다(계약 요구). 로그를 캡처하지 않고 `CommitError` 의 `Display` 문자열(로그의
    // `%error` 와 같은 값)을 `test_hooks` 로 직접 잰다(tracing 전역 캐시 문제 회피,
    // 팀 리더 지시 2026-09-30). 변조로 만든 실제 DB 값(999)이 `actual=` 로 나와야
    // 한다 — `expected=Some(25)` 만으로는 "무엇과 달랐는지"를 알 수 없다.
    let detail = starfall_persistence::test_hooks::last_fatal_detail()
        .expect("정지가 결정됐으면 캡처된 detail 이 있어야 한다");
    assert!(
        detail.contains("actual=Some(999)") || detail.contains("actual=999"),
        "DB 에 실제로 있던 값(999)이 actual= 로 나와야 한다: {detail}"
    );
    assert!(
        detail.contains("expected=Some(25)"),
        "기대값도 그대로 있어야 한다: {detail}"
    );

    // 세 번째 배치는 정지 뒤라 아예 처리되지 않는다(K5) — 이 run_batches 호출은 새
    // handles 로 시작했으므로(0부터), "커밋 성공"은 한 건도 없어야 한다(second 는
    // 실패, third 는 아예 시도되지 않는다).
    assert_eq!(
        handles.persisted_total.load(Ordering::Relaxed),
        0,
        "정지 뒤 배치는 커밋되지 않아야 한다(K5) — third 배치는 시도조차 되지 않는다"
    );

    let events: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM domain_events WHERE world_id = $1 AND tick > 24000",
    )
    .bind(world_id().get())
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(
        events, 0,
        "실패한 트랜잭션은 롤백돼야 한다 — tick 24000 이후 이벤트가 하나도 늘지 않는다"
    );

    let inventory: i64 = sqlx::query_scalar(
        "SELECT quantity_kg FROM inventory_items WHERE world_id = $1 AND actor_id = $2 AND mineral_id = 'starfall-glass'",
    )
    .bind(world_id().get())
    .bind(actor_id.get())
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(
        inventory, 999,
        "변조된 값 그대로 남아야 한다 — 실패한 쓰기가 덮어쓰지 않는다"
    );

    db.drop().await;
}

/// migration 0002의 `domain_events_payload_is_object` CHECK가 실제로 걸리는지 —
/// JSONB `null` payload를 직접 INSERT하면 거부된다(2.2 결함 재현 방지 — persistence의
/// 방어가 뚫려도 DB가 마지막 방어선이다).
#[tokio::test]
async fn payload_null_rejected_by_db() {
    // commit() 을 부르는 모든 테스트가 이 락을 잡는다 — test_hooks 의 전역 주입
    // 플래그는 이 파일 전체에서 공유되므로, 이 락 없이 병렬로 돌면 이 테스트가 다른
    // 테스트가 무장한 주입을 대신 소비하거나(또는 그 반대) 할 수 있다(실측:
    // `batch_recommit_is_noop`/`ambiguous_commit_retry_counts_once` 가 기본
    // `--test-threads` 에서 서로의 카운터를 밟았다).
    let _serialize = starfall_persistence::test_hooks::INJECTION_LOCK
        .lock()
        .await;
    // SC-101 짝 — 계약 이름 그대로.
    let name = "payload_null_rejected_by_db";
    let Some(db) = starfall_testdb::TestDb::create(name).await else {
        return;
    };
    seed_world(&db.pool, world_id()).await;

    let result = sqlx::query(
        "INSERT INTO domain_events (
             event_id, event_type, schema_version, world_id, tick, sequence,
             occurred_at, recorded_at, correlation_id, causation_id, actor_id, payload
         ) VALUES ($1, 'SESSION_OPENED', 1, $2, 0, 0, '3800-01-01T00:00:00Z', now(), $3, NULL, $3, 'null'::jsonb)",
    )
    .bind(id("01a0b1c2-a001-7e01-8f11-505162738495").get())
    .bind(world_id().get())
    .bind(id("01a0b1c2-2c01-7a45-8b67-89abcdef0123").get())
    .execute(&db.pool)
    .await;

    let error = result.expect_err("payload가 JSONB null인 행이 통과했다 — CHECK 제약이 없다");
    let message = error.to_string();
    assert!(
        message.contains("domain_events_payload_is_object") || message.contains("jsonb_typeof"),
        "다른 이유로 실패한 것 같다: {message}"
    );

    // 양성 대조 — object는 통과한다.
    sqlx::query(
        "INSERT INTO domain_events (
             event_id, event_type, schema_version, world_id, tick, sequence,
             occurred_at, recorded_at, correlation_id, causation_id, actor_id, payload
         ) VALUES ($1, 'SESSION_OPENED', 1, $2, 0, 0, '3800-01-01T00:00:00Z', now(), $3, NULL, $3, '{}'::jsonb)",
    )
    .bind(id("01a0b1c2-a002-7e02-8f12-505162738496").get())
    .bind(world_id().get())
    .bind(id("01a0b1c2-2c01-7a45-8b67-89abcdef0123").get())
    .execute(&db.pool)
    .await
    .expect("object payload는 통과해야 한다(양성 대조)");

    db.drop().await;
}

/// worlds 시드 자체가 없으면(있을 수 없는 상태 — main.rs가 기동 시 이미 확인했어야
/// 한다) 첫 커밋 시도가 즉시 복구 불가로 정지한다. panic으로 죽는 대신 halted가 된다.
#[tokio::test]
async fn missing_world_row_halts_instead_of_panicking() {
    // commit() 을 부르는 모든 테스트가 이 락을 잡는다 — test_hooks 의 전역 주입
    // 플래그는 이 파일 전체에서 공유되므로, 이 락 없이 병렬로 돌면 이 테스트가 다른
    // 테스트가 무장한 주입을 대신 소비하거나(또는 그 반대) 할 수 있다(실측:
    // `batch_recommit_is_noop`/`ambiguous_commit_retry_counts_once` 가 기본
    // `--test-threads` 에서 서로의 카운터를 밟았다).
    let _serialize = starfall_persistence::test_hooks::INJECTION_LOCK
        .lock()
        .await;
    // 계약 밖 테스트(qa 확인, accepted_mine_commits_events_and_state 와 같은 사정).
    let name = "missing_world_row_halts_instead_of_panicking";
    let Some(db) = starfall_testdb::TestDb::create(name).await else {
        return;
    };
    // seed_world 를 부르지 않는다 — worlds 행이 없는 상태 그대로.

    let actor_id = id("01a0b1c2-2c01-7a45-8b67-89abcdef0123");
    let batch = first_mine_batch(24000, id("01a0b1c2-9c01-7d11-8e21-3f3140516171"), actor_id);
    let handles = run_batches(db.pool.clone(), world_id(), vec![batch]).await;

    assert!(handles.halted.load(Ordering::Relaxed));
    assert_eq!(handles.persisted_total.load(Ordering::Relaxed), 0);

    db.drop().await;
}

/// SC-100 짝 — 다른 월드에서는 **같은 command_id 가 수락**된다(PK 도 지속 기억도 월드
/// 범위, ADR-0013 §3 K1). 두 월드에서 완전히 독립된 `run()` 을 각각 돌려도 서로 막지
/// 않는다.
#[tokio::test]
async fn cross_world_same_command_id_accepted() {
    // commit() 을 부르는 모든 테스트가 이 락을 잡는다 — test_hooks 의 전역 주입
    // 플래그는 이 파일 전체에서 공유되므로, 이 락 없이 병렬로 돌면 이 테스트가 다른
    // 테스트가 무장한 주입을 대신 소비하거나(또는 그 반대) 할 수 있다(실측:
    // `batch_recommit_is_noop`/`ambiguous_commit_retry_counts_once` 가 기본
    // `--test-threads` 에서 서로의 카운터를 밟았다).
    let _serialize = starfall_persistence::test_hooks::INJECTION_LOCK
        .lock()
        .await;
    let name = "cross_world_same_command_id_accepted";
    let Some(db) = starfall_testdb::TestDb::create(name).await else {
        return;
    };
    let world_a = world_id();
    let world_b = id("01a0e7f3-7869-7105-9103-717c87929db9"); // world_id() 와 다른 월드.
    seed_world(&db.pool, world_a).await;
    seed_world(&db.pool, world_b).await;

    let command_id = id("01a0b1c2-9c01-7d11-8e21-3f3140516171");
    let actor_id = id("01a0b1c2-2c01-7a45-8b67-89abcdef0123");

    let batch_a = first_mine_batch_in_world(world_a, 24000, command_id, actor_id);
    let batch_b = first_mine_batch_in_world(world_b, 24000, command_id, actor_id);

    let handles_a = run_batches(db.pool.clone(), world_a, vec![batch_a]).await;
    let handles_b = run_batches(db.pool.clone(), world_b, vec![batch_b]).await;

    assert!(
        !handles_a.halted.load(Ordering::Relaxed),
        "월드 A 는 멈추지 않아야 한다"
    );
    assert!(
        !handles_b.halted.load(Ordering::Relaxed),
        "월드 B 도 같은 command_id 를 수락해야 한다(월드 범위 PK)"
    );
    assert_eq!(handles_a.persisted_total.load(Ordering::Relaxed), 1);
    assert_eq!(handles_b.persisted_total.load(Ordering::Relaxed), 1);

    for world in [world_a, world_b] {
        let processed: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM processed_commands WHERE world_id = $1 AND command_id = $2",
        )
        .bind(world.get())
        .bind(command_id.get())
        .fetch_one(&db.pool)
        .await
        .unwrap();
        assert_eq!(processed, 1, "월드 {world} 에 장부 행이 있어야 한다");
    }

    db.drop().await;
}

/// SC-21 — sim 의 월드 범위 기억을 **우회**해 같은 `command_id` 가 (다른 tick 으로) DB 에
/// 직접 도달하면, `processed_commands` 의 PK `(world_id, command_id)` 가 마지막
/// 방어선으로 걸려 정지한다. 인벤토리는 그 트랜잭션 롤백으로 불변이다.
#[tokio::test]
async fn dup_reaching_db_halts() {
    // commit() 을 부르는 모든 테스트가 이 락을 잡는다 — test_hooks 의 전역 주입
    // 플래그는 이 파일 전체에서 공유되므로, 이 락 없이 병렬로 돌면 이 테스트가 다른
    // 테스트가 무장한 주입을 대신 소비하거나(또는 그 반대) 할 수 있다(실측:
    // `batch_recommit_is_noop`/`ambiguous_commit_retry_counts_once` 가 기본
    // `--test-threads` 에서 서로의 카운터를 밟았다).
    let _serialize = starfall_persistence::test_hooks::INJECTION_LOCK
        .lock()
        .await;
    let name = "dup_reaching_db_halts";
    let Some(db) = starfall_testdb::TestDb::create(name).await else {
        return;
    };
    seed_world(&db.pool, world_id()).await;

    let command_id = id("01a0b1c2-9c01-7d11-8e21-3f3140516171");
    let actor_id = id("01a0b1c2-2c01-7a45-8b67-89abcdef0123");
    let first = first_mine_batch(24000, command_id, actor_id);
    let handles_first = run_batches(db.pool.clone(), world_id(), vec![first]).await;
    assert!(!handles_first.halted.load(Ordering::Relaxed));
    assert_eq!(handles_first.persisted_total.load(Ordering::Relaxed), 1);

    // sim 의 기억을 우회한다 — 같은 command_id 를 (새 이벤트 id·다른 tick 으로) 영속화에
    // 직접 밀어넣는다. tick 이 더 크므로 §4 의 배치 멱등(tick 비교)으로는 안 걸린다.
    let second_event = PendingEvent {
        event_id: id("01a0b1c2-a002-7e02-8f12-505162738496"),
        world_id: world_id(),
        tick: Tick::new(24060).unwrap(),
        sequence: Sequence::new(0).unwrap(),
        occurred_at: GameTime::parse("3800-01-01T20:00:03Z").unwrap(),
        correlation_id: id("01a0b1c2-4a12-7c01-8d02-1e033f044a05"),
        causation_id: Some(command_id), // 중복.
        actor_id,
        body: DomainEventBody::MineralMined(MineralMinedPayload {
            ship_id: id("01a0b1c2-8b01-7a11-8b22-9c33d44e55f6"),
            session_id: id("01a0b1c2-4a11-7b22-9c33-0d44e55f6a77"),
            star_system_id: DataId::parse("cradle").unwrap(),
            deposit_id: DataId::parse("far-reach").unwrap(),
            mineral_id: DataId::parse("starfall-glass").unwrap(),
            quantity_kg: mass(25),
            quantity_before_kg: mass(25),
            quantity_after_kg: mass(50),
            deposit_remaining_before_kg: mass(475),
            deposit_remaining_after_kg: mass(450),
        }),
    };
    let second = PersistBatch {
        tick: 24060,
        events: vec![second_event],
        state_writes: vec![
            StateWrite::Inventory {
                actor_id,
                mineral_id: DataId::parse("starfall-glass").unwrap(),
                expected: Some(25),
                new: 50,
            },
            StateWrite::Deposit {
                deposit_id: DataId::parse("far-reach").unwrap(),
                expected: Some((475, 24000)),
                new: (450, 24060),
                first_extracted_tick: 24000,
            },
        ],
    };

    let handles = run_batches(db.pool.clone(), world_id(), vec![second]).await;

    assert!(
        handles.halted.load(Ordering::Relaxed),
        "processed_commands PK 위반은 월드를 멈춰야 한다(SC-21)"
    );
    assert!(handles.failed_total.load(Ordering::Relaxed) >= 1);
    assert_eq!(
        handles.persisted_total.load(Ordering::Relaxed),
        0,
        "이 run 호출에서는 성공 커밋이 없어야 한다"
    );

    let events: i64 = sqlx::query_scalar("SELECT count(*) FROM domain_events WHERE world_id = $1")
        .bind(world_id().get())
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(events, 1, "첫 배치의 이벤트만 남아야 한다(둘째는 롤백)");

    let inventory: i64 = sqlx::query_scalar(
        "SELECT quantity_kg FROM inventory_items WHERE world_id = $1 AND actor_id = $2 AND mineral_id = 'starfall-glass'",
    )
    .bind(world_id().get())
    .bind(actor_id.get())
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(inventory, 25, "롤백됐으므로 인벤토리가 두 번 늘면 안 된다");

    let processed: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM processed_commands WHERE world_id = $1 AND command_id = $2",
    )
    .bind(world_id().get())
    .bind(command_id.get())
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(processed, 1, "장부 행도 첫 배치 것 하나뿐이어야 한다");

    db.drop().await;
}

/// SC-100 — 다른 actor 가 같은 `command_id` 를 보내면 sim 의 월드 범위 기억이
/// **영속화에 도달하기 전에** 하나만 거른다(그 경로는 `starfall-sim` 의
/// `duplicate_command_id_across_different_sessions_is_rejected` 가 잰다). 이 DB
/// 테스트는 그 나머지 절반 — 영속화까지 실제로 도달하는 것은 **수락된 쪽 한 건뿐**이고,
/// PK 가 `(world_id, command_id)` 뿐이라 actor 신원은 커밋에 전혀 영향을 주지 않는다는
/// 것을 확인한다: 서로 다른 actor 의 **서로 다른** command_id 가 같은 월드에서 둘 다
/// 문제없이 커밋된다.
#[tokio::test]
async fn dup_cross_actor_no_halt() {
    // commit() 을 부르는 모든 테스트가 이 락을 잡는다 — test_hooks 의 전역 주입
    // 플래그는 이 파일 전체에서 공유되므로, 이 락 없이 병렬로 돌면 이 테스트가 다른
    // 테스트가 무장한 주입을 대신 소비하거나(또는 그 반대) 할 수 있다(실측:
    // `batch_recommit_is_noop`/`ambiguous_commit_retry_counts_once` 가 기본
    // `--test-threads` 에서 서로의 카운터를 밟았다).
    let _serialize = starfall_persistence::test_hooks::INJECTION_LOCK
        .lock()
        .await;
    let name = "dup_cross_actor_no_halt";
    let Some(db) = starfall_testdb::TestDb::create(name).await else {
        return;
    };
    seed_world(&db.pool, world_id()).await;

    let actor_a = id("01a0b1c2-2c01-7a45-8b67-89abcdef0123");
    let actor_b = id("01a0b1c2-2c02-7a46-8b68-89abcdef0124");
    let command_a = id("01a0b1c2-9c01-7d11-8e21-3f3140516171");
    let command_b = id("01a0b1c2-9c02-7d12-9e22-4f4150617181");

    let batch_a = first_mine_batch(24000, command_a, actor_a);
    let handles_a = run_batches(db.pool.clone(), world_id(), vec![batch_a]).await;
    assert!(!handles_a.halted.load(Ordering::Relaxed));

    let second_event = PendingEvent {
        event_id: id("01a0b1c2-a002-7e02-8f12-505162738496"),
        world_id: world_id(),
        tick: Tick::new(24060).unwrap(),
        sequence: Sequence::new(0).unwrap(),
        occurred_at: GameTime::parse("3800-01-01T20:00:03Z").unwrap(),
        correlation_id: id("01a0b1c2-4a12-7c01-8d02-1e033f044a05"),
        causation_id: Some(command_b),
        actor_id: actor_b,
        body: DomainEventBody::MineralMined(MineralMinedPayload {
            ship_id: id("01a0b1c2-8b02-7a12-8b23-9c34d45e56f7"),
            session_id: id("01a0b1c2-4a13-7b23-9c34-0d45e56f6a78"),
            star_system_id: DataId::parse("cradle").unwrap(),
            deposit_id: DataId::parse("far-reach").unwrap(),
            mineral_id: DataId::parse("starfall-glass").unwrap(),
            quantity_kg: mass(25),
            quantity_before_kg: mass(0),
            quantity_after_kg: mass(25),
            deposit_remaining_before_kg: mass(475),
            deposit_remaining_after_kg: mass(450),
        }),
    };
    let batch_b = PersistBatch {
        tick: 24060,
        events: vec![second_event],
        state_writes: vec![
            StateWrite::Inventory {
                actor_id: actor_b,
                mineral_id: DataId::parse("starfall-glass").unwrap(),
                expected: None, // actor_b 는 이 광물이 처음이다.
                new: 25,
            },
            StateWrite::Deposit {
                deposit_id: DataId::parse("far-reach").unwrap(),
                expected: Some((475, 24000)),
                new: (450, 24060),
                first_extracted_tick: 24000,
            },
        ],
    };
    let handles_b = run_batches(db.pool.clone(), world_id(), vec![batch_b]).await;
    assert!(
        !handles_b.halted.load(Ordering::Relaxed),
        "다른 actor 의 별개 명령이 멈추면 안 된다"
    );
    assert_eq!(handles_b.persisted_total.load(Ordering::Relaxed), 1);

    let processed: i64 =
        sqlx::query_scalar("SELECT count(*) FROM processed_commands WHERE world_id = $1")
            .bind(world_id().get())
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert_eq!(processed, 2, "두 actor 의 명령이 각각 장부에 남아야 한다");

    db.drop().await;
}

/// AC-4(c) — 정상 종료 후 재기동을 시뮬레이션한다: 첫 배치가 커밋되고 **재기동 전에**
/// 장부 행이 이미 있음을 SQL 로 직접 확인한 뒤(⊘: "재기동 전 배치가 커밋되지 않아
/// 장부가 비어 있음"을 배제), 새 `run()` 태스크(=재기동 뒤의 새 영속화 루프)로 같은
/// `command_id` 재전송을 흉내낸다. persistence 층은 sim 의 기억 재적재를 재현하지
/// 않는다(그건 `bins/game-server` 의 기동 순서 몫, ADR-0013 §7) — 여기서는 **재기동
/// 뒤에도 DB 의 PK 가 같은 command_id 재적용을 막는다**는 지속성만 잰다.
#[tokio::test]
async fn dup_across_restart() {
    // commit() 을 부르는 모든 테스트가 이 락을 잡는다 — test_hooks 의 전역 주입
    // 플래그는 이 파일 전체에서 공유되므로, 이 락 없이 병렬로 돌면 이 테스트가 다른
    // 테스트가 무장한 주입을 대신 소비하거나(또는 그 반대) 할 수 있다(실측:
    // `batch_recommit_is_noop`/`ambiguous_commit_retry_counts_once` 가 기본
    // `--test-threads` 에서 서로의 카운터를 밟았다).
    let _serialize = starfall_persistence::test_hooks::INJECTION_LOCK
        .lock()
        .await;
    let name = "dup_across_restart";
    let Some(db) = starfall_testdb::TestDb::create(name).await else {
        return;
    };
    seed_world(&db.pool, world_id()).await;

    let command_id = id("01a0b1c2-9c01-7d11-8e21-3f3140516171");
    let actor_id = id("01a0b1c2-2c01-7a45-8b67-89abcdef0123");
    let first = first_mine_batch(24000, command_id, actor_id);
    let handles_first = run_batches(db.pool.clone(), world_id(), vec![first]).await;
    assert!(!handles_first.halted.load(Ordering::Relaxed));

    // ⊘ 배제: 재기동 전에 이미 장부 행이 있다.
    let processed_before: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM processed_commands WHERE world_id = $1 AND command_id = $2",
    )
    .bind(world_id().get())
    .bind(command_id.get())
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(
        processed_before, 1,
        "재기동 시뮬레이션 전에 장부 행이 이미 있어야 한다"
    );
    let inventory_before: i64 = sqlx::query_scalar(
        "SELECT quantity_kg FROM inventory_items WHERE world_id = $1 AND actor_id = $2 AND mineral_id = 'starfall-glass'",
    )
    .bind(world_id().get())
    .bind(actor_id.get())
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(inventory_before, 25);

    // "재기동" — 같은 DB 를 향한 새 `run()` 태스크(새 영속화 루프). 클라이언트가
    // 재접속 뒤 같은 command_id 를 재전송했다고 가정한다.
    let resend_event = PendingEvent {
        event_id: id("01a0b1c2-a003-7e03-8f13-505162738497"), // 재전송은 새 event_id.
        world_id: world_id(),
        tick: Tick::new(24060).unwrap(),
        sequence: Sequence::new(0).unwrap(),
        occurred_at: GameTime::parse("3800-01-01T20:00:03Z").unwrap(),
        correlation_id: id("01a0b1c2-4a12-7c01-8d02-1e033f044a05"),
        causation_id: Some(command_id), // 같은 command_id.
        actor_id,
        body: DomainEventBody::MineralMined(MineralMinedPayload {
            ship_id: id("01a0b1c2-8b01-7a11-8b22-9c33d44e55f6"),
            session_id: id("01a0b1c2-4a11-7b22-9c33-0d44e55f6a77"),
            star_system_id: DataId::parse("cradle").unwrap(),
            deposit_id: DataId::parse("far-reach").unwrap(),
            mineral_id: DataId::parse("starfall-glass").unwrap(),
            quantity_kg: mass(25),
            quantity_before_kg: mass(25),
            quantity_after_kg: mass(50),
            deposit_remaining_before_kg: mass(475),
            deposit_remaining_after_kg: mass(450),
        }),
    };
    let resend = PersistBatch {
        tick: 24060,
        events: vec![resend_event],
        state_writes: vec![
            StateWrite::Inventory {
                actor_id,
                mineral_id: DataId::parse("starfall-glass").unwrap(),
                expected: Some(25),
                new: 50,
            },
            StateWrite::Deposit {
                deposit_id: DataId::parse("far-reach").unwrap(),
                expected: Some((475, 24000)),
                new: (450, 24060),
                first_extracted_tick: 24000,
            },
        ],
    };
    let handles_after_restart = run_batches(db.pool.clone(), world_id(), vec![resend]).await;

    assert!(
        handles_after_restart.halted.load(Ordering::Relaxed),
        "재기동 뒤에도 같은 command_id 재적용은 PK 가 막아야 한다(AC-4(c))"
    );
    assert_eq!(
        handles_after_restart
            .persisted_total
            .load(Ordering::Relaxed),
        0
    );

    let processed_after: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM processed_commands WHERE world_id = $1 AND command_id = $2",
    )
    .bind(world_id().get())
    .bind(command_id.get())
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(
        processed_after, processed_before,
        "장부 행 수가 그대로여야 한다"
    );

    let inventory_after: i64 = sqlx::query_scalar(
        "SELECT quantity_kg FROM inventory_items WHERE world_id = $1 AND actor_id = $2 AND mineral_id = 'starfall-glass'",
    )
    .bind(world_id().get())
    .bind(actor_id.get())
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(
        inventory_after, inventory_before,
        "인벤토리도 그대로여야 한다"
    );

    db.drop().await;
}

/// ADR-0013 §5 K5 — 정지가 결정된 뒤에는 **그 뒤에 큐에 남은 어떤 배치도** 커밋하지
/// 않는다. 일반 배치 1개 + `SESSION_CLOSED` 종료 스윕 모양 1개를 fatal 배치 뒤에
/// 넣고, 행 변화가 전혀 없는지 확인한다(SC-102 단위 짝 — 실서버 증거는 SC-25 의 로그
/// `sessions_closed` 가 맡는다).
#[tokio::test]
async fn no_commit_after_fatal() {
    // commit() 을 부르는 모든 테스트가 이 락을 잡는다 — test_hooks 의 전역 주입
    // 플래그는 이 파일 전체에서 공유되므로, 이 락 없이 병렬로 돌면 이 테스트가 다른
    // 테스트가 무장한 주입을 대신 소비하거나(또는 그 반대) 할 수 있다(실측:
    // `batch_recommit_is_noop`/`ambiguous_commit_retry_counts_once` 가 기본
    // `--test-threads` 에서 서로의 카운터를 밟았다).
    let _serialize = starfall_persistence::test_hooks::INJECTION_LOCK
        .lock()
        .await;
    let name = "no_commit_after_fatal";
    let Some(db) = starfall_testdb::TestDb::create(name).await else {
        return;
    };
    seed_world(&db.pool, world_id()).await;

    let actor_id = id("01a0b1c2-2c01-7a45-8b67-89abcdef0123");
    let session_id = id("01a0b1c2-4a11-7b22-9c33-0d44e55f6a77");
    let first = first_mine_batch(24000, id("01a0b1c2-9c01-7d11-8e21-3f3140516171"), actor_id);
    let handles_first = run_batches(db.pool.clone(), world_id(), vec![first]).await;
    assert!(!handles_first.halted.load(Ordering::Relaxed));

    // 변조로 CAS 불일치를 유발한다(AC-5(b) 시나리오, `cas_mismatch_halts_the_world` 와
    // 같은 방법).
    sqlx::query(
        "UPDATE inventory_items SET quantity_kg = 999 WHERE world_id = $1 AND actor_id = $2 AND mineral_id = 'starfall-glass'",
    )
    .bind(world_id().get())
    .bind(actor_id.get())
    .execute(&db.pool)
    .await
    .unwrap();

    let fatal_event = PendingEvent {
        event_id: id("01a0b1c2-a002-7e02-8f12-505162738496"),
        world_id: world_id(),
        tick: Tick::new(24060).unwrap(),
        sequence: Sequence::new(0).unwrap(),
        occurred_at: GameTime::parse("3800-01-01T20:00:03Z").unwrap(),
        correlation_id: id("01a0b1c2-4a12-7c01-8d02-1e033f044a05"),
        causation_id: Some(id("01a0b1c2-9c02-7d12-9e22-4f4150617181")),
        actor_id,
        body: DomainEventBody::MineralMined(MineralMinedPayload {
            ship_id: id("01a0b1c2-8b01-7a11-8b22-9c33d44e55f6"),
            session_id,
            star_system_id: DataId::parse("cradle").unwrap(),
            deposit_id: DataId::parse("far-reach").unwrap(),
            mineral_id: DataId::parse("starfall-glass").unwrap(),
            quantity_kg: mass(25),
            quantity_before_kg: mass(25),
            quantity_after_kg: mass(50),
            deposit_remaining_before_kg: mass(475),
            deposit_remaining_after_kg: mass(450),
        }),
    };
    let fatal_batch = PersistBatch {
        tick: 24060,
        events: vec![fatal_event],
        state_writes: vec![StateWrite::Inventory {
            actor_id,
            mineral_id: DataId::parse("starfall-glass").unwrap(),
            expected: Some(25), // 저장된 값(999)과 다르다 — CAS 불일치 → fatal.
            new: 50,
        }],
    };

    // fatal 뒤에도 채널에 남아 시도조차 되지 않아야 하는 두 배치 — 일반 1 + SESSION_CLOSED
    // 스윕 모양 1.
    let normal_after =
        first_mine_batch(24120, id("01a0b1c2-9c03-7d13-8e23-505162738497"), actor_id);
    let sweep_after = PersistBatch {
        tick: 24180,
        events: vec![PendingEvent {
            event_id: id("01a0b1c2-a004-7e04-8f14-505162738498"),
            world_id: world_id(),
            tick: Tick::new(24180).unwrap(),
            sequence: Sequence::new(0).unwrap(),
            occurred_at: GameTime::parse("3800-01-01T20:00:09Z").unwrap(),
            correlation_id: id("01a0b1c2-4a12-7c01-8d02-1e033f044a05"),
            causation_id: None,
            actor_id,
            body: DomainEventBody::SessionClosed(SessionClosedPayload {
                session_id,
                close_reason: SessionCloseReason::ServerShutdown,
            }),
        }],
        state_writes: vec![],
    };

    let handles = run_batches(
        db.pool.clone(),
        world_id(),
        vec![fatal_batch, normal_after, sweep_after],
    )
    .await;

    assert!(handles.halted.load(Ordering::Relaxed));
    assert!(handles.failed_total.load(Ordering::Relaxed) >= 1);
    assert_eq!(
        handles.persisted_total.load(Ordering::Relaxed),
        0,
        "이 run 호출에서는 성공 커밋이 전혀 없어야 한다"
    );

    let events_after_first: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM domain_events WHERE world_id = $1 AND tick > 24000",
    )
    .bind(world_id().get())
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(
        events_after_first, 0,
        "fatal 배치도 롤백되고, 나머지 둘은 시도조차 되지 않아야 한다(K5)"
    );

    let session_closed: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM domain_events WHERE world_id = $1 AND event_type = 'SESSION_CLOSED'",
    )
    .bind(world_id().get())
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(
        session_closed, 0,
        "정지 뒤 종료 스윕의 SESSION_CLOSED 는 커밋되면 안 된다(K5)"
    );

    db.drop().await;
}

/// SC-20 둘째 테스트 — 실제로는 성공한 커밋을, 응답이 유실된 것처럼 한 번 더 재시도하게
/// 만든다(`force_apparent_failure_after_next_success`). 재시도는 tick 비교로
/// `AlreadyCommitted` 를 만나 K6 경로(건너뜀)를 탄다. `persist_ambiguous_commits_total
/// == 1` 이고, `persisted_total` 이 실제로 커밋된 행 수(`COUNT(domain_events)`)와
/// 같다는 항등식을 이 테스트 모양에서 확인한다(`batch_recommit_is_noop` 은 배치를
/// 통째로 두 번 보내는 다른 모양이라 이 항등식이 성립하지 않는다 — 그 테스트의 주석
/// 참고).
#[tokio::test]
async fn ambiguous_commit_retry_counts_once() {
    use starfall_persistence::test_hooks;
    let _guard = test_hooks::INJECTION_LOCK.lock().await;
    test_hooks::reset();

    let name = "ambiguous_commit_retry_counts_once";
    let Some(db) = starfall_testdb::TestDb::create(name).await else {
        test_hooks::reset();
        return;
    };
    seed_world(&db.pool, world_id()).await;

    let command_id = id("01a0b1c2-9c01-7d11-8e21-3f3140516171");
    let actor_id = id("01a0b1c2-2c01-7a45-8b67-89abcdef0123");
    let batch = first_mine_batch(24000, command_id, actor_id);

    test_hooks::force_apparent_failure_after_next_success(1);
    let handles = run_batches(db.pool.clone(), world_id(), vec![batch]).await;

    assert_eq!(
        test_hooks::apparent_failure_hit_count(),
        1,
        "주입이 실제로 걸렸어야 한다 — 0 이면 이 테스트는 무효다"
    );
    assert_eq!(handles.ambiguous_commits_total.load(Ordering::Relaxed), 1);
    assert!(!handles.halted.load(Ordering::Relaxed));

    let events: i64 = sqlx::query_scalar("SELECT count(*) FROM domain_events WHERE world_id = $1")
        .bind(world_id().get())
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(
        events, 1,
        "실제로 쓴 행은 1건뿐이어야 한다(재시도는 건너뜀)"
    );
    assert_eq!(
        handles.persisted_total.load(Ordering::Relaxed),
        u64::try_from(events).unwrap(),
        "persisted_total == COUNT(domain_events) — 이 테스트 모양(모호한 커밋 재시도 \
         1회)에서는 이 항등식이 성립한다"
    );

    test_hooks::reset();
    db.drop().await;
}

/// SC-27 — `recorded_at` 생성(호스트 시계 읽기) 실패는 이벤트를 건너뛰지 않고 배치
/// 전체를 복구 불가로 정지시킨다(ADR-0013 §5 가 고치라고 한 예전 `continue` 버그의
/// 회귀 방지).
#[tokio::test]
async fn recorded_at_failure_fails_batch() {
    use starfall_persistence::test_hooks;
    let _guard = test_hooks::INJECTION_LOCK.lock().await;
    test_hooks::reset();

    let name = "recorded_at_failure_fails_batch";
    let Some(db) = starfall_testdb::TestDb::create(name).await else {
        test_hooks::reset();
        return;
    };
    seed_world(&db.pool, world_id()).await;

    let actor_id = id("01a0b1c2-2c01-7a45-8b67-89abcdef0123");
    let batch = first_mine_batch(24000, id("01a0b1c2-9c01-7d11-8e21-3f3140516171"), actor_id);

    test_hooks::fail_next_clock(1);
    let handles = run_batches(db.pool.clone(), world_id(), vec![batch]).await;

    assert_eq!(
        test_hooks::clock_failure_hit_count(),
        1,
        "주입 지점이 실제로 실행됐어야 한다 — 0 이면 이 테스트는 무효다"
    );
    assert!(
        handles.halted.load(Ordering::Relaxed),
        "recorded_at 실패는 복구 불가여야 한다(§5)"
    );
    assert!(handles.failed_total.load(Ordering::Relaxed) >= 1);
    assert_eq!(handles.persisted_total.load(Ordering::Relaxed), 0);

    let events: i64 = sqlx::query_scalar("SELECT count(*) FROM domain_events WHERE world_id = $1")
        .bind(world_id().get())
        .fetch_one(&db.pool)
        .await
        .unwrap();
    let states: i64 =
        sqlx::query_scalar("SELECT count(*) FROM inventory_items WHERE world_id = $1")
            .bind(world_id().get())
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert_eq!(events, 0, "실패한 트랜잭션은 롤백돼야 한다 — 이벤트도 0");
    assert_eq!(states, 0, "한쪽만 커밋되면 안 된다 — 상태도 0");

    test_hooks::reset();
    db.drop().await;
}

/// SC-101 — payload 직렬화 실패는 JSONB `null` 로 감춰지지 않고 배치 전체를 복구
/// 불가로 정지시킨다(2.2 결함 재현 방지). `payload_null_rejected_by_db` 가 DB 층의
/// 마지막 방어선(CHECK 제약)을 재는 것과 짝이다 — 이 테스트는 persistence 층 자체가
/// 그 방어선에 기대지 않고 먼저 막는지를 잰다.
#[tokio::test]
async fn payload_serialize_failure_fails_batch() {
    use starfall_persistence::test_hooks;
    let _guard = test_hooks::INJECTION_LOCK.lock().await;
    test_hooks::reset();

    let name = "payload_serialize_failure_fails_batch";
    let Some(db) = starfall_testdb::TestDb::create(name).await else {
        test_hooks::reset();
        return;
    };
    seed_world(&db.pool, world_id()).await;

    let actor_id = id("01a0b1c2-2c01-7a45-8b67-89abcdef0123");
    let batch = first_mine_batch(24000, id("01a0b1c2-9c01-7d11-8e21-3f3140516171"), actor_id);

    test_hooks::fail_next_payload(1);
    let handles = run_batches(db.pool.clone(), world_id(), vec![batch]).await;

    assert_eq!(
        test_hooks::payload_failure_hit_count(),
        1,
        "주입 지점이 실제로 실행됐어야 한다 — 0 이면 이 테스트는 무효다"
    );
    assert!(
        handles.halted.load(Ordering::Relaxed),
        "payload 직렬화 실패는 복구 불가여야 한다(§5)"
    );
    assert!(handles.failed_total.load(Ordering::Relaxed) >= 1);
    assert_eq!(handles.persisted_total.load(Ordering::Relaxed), 0);

    let events: i64 = sqlx::query_scalar("SELECT count(*) FROM domain_events WHERE world_id = $1")
        .bind(world_id().get())
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(
        events, 0,
        "직렬화 실패 이벤트가 null payload 로 저장되면 안 된다"
    );

    // 부정 짝(규칙 9: 소스는 긍정을 증명하지 못한다 — 이 grep 은 "없다"만 증명한다.
    // 위 두 DB 테스트(이 테스트·`payload_null_rejected_by_db`)와 짝으로만 의미가
    // 있다).
    let source = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/lib.rs"))
        .expect("소스를 읽지 못했다");
    assert!(
        !source.contains("unwrap_or(Value::Null)")
            && !source.contains("unwrap_or(serde_json::Value::Null)"),
        "payload 실패를 JSONB null 로 감추는 코드가 다시 들어왔다"
    );

    test_hooks::reset();
    db.drop().await;
}

/// SC-103(a) — 허용 목록 안 SQLSTATE(`57P01`, admin_shutdown)는 **재시도**고 정지가
/// 아니다. 짝: 허용 목록 밖(`22P02`)은 같은 테스트 안에서 정지로 판정된다("무엇이든
/// 재시도" 가 아님을 확인한다).
#[tokio::test]
async fn admin_shutdown_is_transient() {
    use starfall_persistence::test_hooks;
    let _guard = test_hooks::INJECTION_LOCK.lock().await;
    test_hooks::reset();

    // (a) 57P01 — 재시도 뒤 같은 배치가 커밋된다, 정지하지 않는다.
    {
        let name = "admin_shutdown_is_transient";
        let Some(db) = starfall_testdb::TestDb::create(name).await else {
            test_hooks::reset();
            return;
        };
        seed_world(&db.pool, world_id()).await;

        let actor_id = id("01a0b1c2-2c01-7a45-8b67-89abcdef0123");
        let batch = first_mine_batch(24000, id("01a0b1c2-9c01-7d11-8e21-3f3140516171"), actor_id);

        test_hooks::force_transient_once(1);
        let handles = run_batches(db.pool.clone(), world_id(), vec![batch]).await;

        assert_eq!(
            test_hooks::transient_injection_hit_count(),
            1,
            "57P01 주입이 실제로 걸렸어야 한다"
        );
        assert!(
            !handles.halted.load(Ordering::Relaxed),
            "57P01 은 일시 실패여야 한다(K4) — 정지하면 안 된다"
        );
        assert_eq!(handles.failed_total.load(Ordering::Relaxed), 0);
        assert_eq!(
            handles.persisted_total.load(Ordering::Relaxed),
            1,
            "재시도 뒤 같은 배치가 커밋돼야 한다"
        );
        assert!(
            test_hooks::transient_hit_count() >= 1,
            "관찰한 일시 오류가 있어야 한다(0 이면 무효)"
        );

        db.drop().await;
    }

    // 짝 — 22P02(허용 목록 밖)는 정지한다.
    {
        test_hooks::reset();
        let name = "admin_shutdown_is_transient_fatal_pair";
        let Some(db) = starfall_testdb::TestDb::create(name).await else {
            test_hooks::reset();
            return;
        };
        seed_world(&db.pool, world_id()).await;

        let actor_id = id("01a0b1c2-2c01-7a45-8b67-89abcdef0123");
        let batch = first_mine_batch(24000, id("01a0b1c2-9c04-7d14-8e24-505162738499"), actor_id);

        test_hooks::force_fatal_once(1);
        let handles = run_batches(db.pool.clone(), world_id(), vec![batch]).await;

        assert_eq!(
            test_hooks::fatal_injection_hit_count(),
            1,
            "22P02 주입이 실제로 걸렸어야 한다"
        );
        assert!(
            handles.halted.load(Ordering::Relaxed),
            "허용 목록 밖 SQLSTATE 는 정지해야 한다 — \"무엇이든 재시도\"가 아니다"
        );
        assert!(handles.failed_total.load(Ordering::Relaxed) >= 1);

        db.drop().await;
    }

    test_hooks::reset();
}

/// SC-112 — 실제 DB 접속을 강제로 끊는다(`pg_terminate_backend`). sqlx 가 받는 오류가
/// `57P01`·`Io` 중 무엇일지는 타이밍이 정하지만(server 실측), 둘 다 K4 허용 목록
/// 안이므로 재시도로 이어지고 같은 배치가 결국 커밋돼야 한다 — 정지 없음.
///
/// 끊기가 커밋과 겹치지 않으면 오류가 0건 관측될 수 있다 — 그러면 **이 실행은
/// 무효다**(⊘, 계약 문구 그대로). 그래서 커밋이 끝날 때까지 반복해서 끊는 백그라운드
/// 태스크를 두고, 관찰한 일시 오류 수를 `assert!(>= 1)`로 단언한다.
#[tokio::test]
async fn terminated_backend_is_retried() {
    use starfall_persistence::test_hooks;
    let _guard = test_hooks::INJECTION_LOCK.lock().await;
    test_hooks::reset();

    let name = "terminated_backend_is_retried";
    let Some(db) = starfall_testdb::TestDb::create(name).await else {
        test_hooks::reset();
        return;
    };
    seed_world(&db.pool, world_id()).await;

    let actor_id = id("01a0b1c2-2c01-7a45-8b67-89abcdef0123");
    let batch = first_mine_batch(24000, id("01a0b1c2-9c01-7d11-8e21-3f3140516171"), actor_id);

    let handles = handles();
    let (tx, rx) = mpsc::channel(16);
    let (commit_notify_tx, _commit_notify_rx) = watch::channel(None);
    let run_task = tokio::spawn(starfall_persistence::run(
        db.pool.clone(),
        world_id(),
        rx,
        handles.clone(),
        commit_notify_tx,
    ));

    // 백그라운드 킬러 — `commit()` 이 트랜잭션을 여는 순간(`TRANSACTION_ATTEMPT_COUNTER`
    // 가 바뀌는 순간)을 노려 이 DB 의 활성 백엔드를 끊는다. `pg_stat_activity` 의 쿼리
    // 문자열을 폴링하는 것보다 훨씬 더 자주 "트랜잭션 진행 중" 창을 맞힌다 — 문자열
    // 매칭은 네트워크 왕복 지연 때문에 그 짧은 창을 자주 놓쳤다(실측). 자기 자신(이
    // 킬러 커넥션)은 `pg_backend_pid()` 로 제외한다.
    let killer_pool = db.pool.clone();
    let last_committed = handles.last_committed_tick.clone();
    let killer = tokio::spawn(async move {
        let mut last_seen_attempt = test_hooks::transaction_attempt_counter();
        for _ in 0..2000 {
            if last_committed.load(Ordering::Relaxed) >= 24000 {
                break;
            }
            let current_attempt = test_hooks::transaction_attempt_counter();
            if current_attempt != last_seen_attempt {
                last_seen_attempt = current_attempt;
                let _ = sqlx::query(
                    "SELECT pg_terminate_backend(pid) FROM pg_stat_activity \
                     WHERE pid <> pg_backend_pid() AND datname = current_database() \
                       AND state <> 'idle'",
                )
                .execute(&killer_pool)
                .await;
            }
            tokio::time::sleep(std::time::Duration::from_millis(1)).await;
        }
    });

    tx.send(batch).await.expect("영속화 채널이 닫혔다");
    drop(tx);
    run_task.await.expect("영속화 태스크가 패닉했다");
    let _ = killer.await;

    assert!(
        !handles.halted.load(Ordering::Relaxed),
        "일시적 접속 끊김은 정지로 이어지면 안 된다(K4)"
    );
    assert_eq!(handles.persisted_total.load(Ordering::Relaxed), 1);
    let observed = test_hooks::transient_hit_count();
    assert!(
        observed >= 1,
        "0 이면 이 실행은 무효다(끊기가 커밋과 겹치지 않았다) — 재실행 필요"
    );

    let events: i64 = sqlx::query_scalar("SELECT count(*) FROM domain_events WHERE world_id = $1")
        .bind(world_id().get())
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(events, 1, "끊기 뒤 재시도로 같은 배치가 결국 커밋돼야 한다");

    test_hooks::reset();
    db.drop().await;
}

/// S8(TaskList #27, ADR-0013 §7 2단계) — `load_economic_state` 가 돌려주는 값이 SQL 로
/// 직접 읽은 값과 같다. 계약 밖 테스트(이 함수 자체가 아직 계약에 이름이 없다 — 정해지면
/// 그 이름으로 바꾼다, team-lead 확인 중).
///
/// 두 actor·두 매장지로 인벤토리·매장지·장부 세 표를 모두 채운 뒤, `load_economic_state`
/// 의 결과를 (a) 행 수 (b) 각 필드 값 둘 다로 SQL 원본과 대조한다 — 행 수만 맞고 내용이
/// 섞이면(예: actor_id·mineral_id 가 바뀌어 들어감) 들키지 않는 결함이라 필드 단위로
/// 비교한다.
#[tokio::test]
async fn load_economic_state_matches_the_database() {
    let _serialize = starfall_persistence::test_hooks::INJECTION_LOCK
        .lock()
        .await;

    let name = "load_economic_state_matches_the_database";
    let Some(db) = starfall_testdb::TestDb::create(name).await else {
        return;
    };
    seed_world(&db.pool, world_id()).await;

    let actor_a = id("01a0b1c2-2c01-7a45-8b67-89abcdef0123");
    let actor_b = id("01a0b1c2-2c02-7a46-8b68-89abcdef0124");

    // actor_a: far-reach 광맥의 starfall-glass 25kg(첫 채굴, `first_mine_batch` 그대로).
    let first = first_mine_batch(24000, id("01a0b1c2-9c01-7d11-8e21-3f3140516171"), actor_a);
    let handles_first = run_batches(db.pool.clone(), world_id(), vec![first]).await;
    assert!(!handles_first.halted.load(Ordering::Relaxed));

    // actor_b: 같은 매장지에서 다시 채굴(매장지 상태는 기존 값 위에 CAS) — 인벤토리
    // 표에 두 번째 actor 행을, 장부에 두 번째 command_id 를 만든다.
    let second_event = PendingEvent {
        event_id: id("01a0b1c2-a002-7e02-8f12-505162738496"),
        world_id: world_id(),
        tick: Tick::new(24060).unwrap(),
        sequence: Sequence::new(0).unwrap(),
        occurred_at: GameTime::parse("3800-01-01T20:00:03Z").unwrap(),
        correlation_id: id("01a0b1c2-4a12-7c01-8d02-1e033f044a05"),
        causation_id: Some(id("01a0b1c2-9c02-7d12-9e22-4f4150617181")),
        actor_id: actor_b,
        body: DomainEventBody::MineralMined(MineralMinedPayload {
            ship_id: id("01a0b1c2-8b02-7a12-8b23-9c34d45e56f7"),
            session_id: id("01a0b1c2-4a13-7b23-9c34-0d45e56f6a78"),
            star_system_id: DataId::parse("cradle").unwrap(),
            deposit_id: DataId::parse("far-reach").unwrap(),
            mineral_id: DataId::parse("starfall-glass").unwrap(),
            quantity_kg: mass(25),
            quantity_before_kg: mass(0),
            quantity_after_kg: mass(25),
            deposit_remaining_before_kg: mass(475),
            deposit_remaining_after_kg: mass(450),
        }),
    };
    let second = PersistBatch {
        tick: 24060,
        events: vec![second_event],
        state_writes: vec![
            StateWrite::Inventory {
                actor_id: actor_b,
                mineral_id: DataId::parse("starfall-glass").unwrap(),
                expected: None,
                new: 25,
            },
            StateWrite::Deposit {
                deposit_id: DataId::parse("far-reach").unwrap(),
                expected: Some((475, 24000)),
                new: (450, 24060),
                first_extracted_tick: 24000,
            },
        ],
    };
    let handles_second = run_batches(db.pool.clone(), world_id(), vec![second]).await;
    assert!(!handles_second.halted.load(Ordering::Relaxed));

    let economic = starfall_persistence::load_economic_state(&db.pool, world_id())
        .await
        .expect("경제 상태 적재 실패");

    // (a) 행 수 — SQL 로 직접 센 것과 같아야 한다.
    let inventory_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM inventory_items WHERE world_id = $1")
            .bind(world_id().get())
            .fetch_one(&db.pool)
            .await
            .unwrap();
    let deposit_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM deposit_states WHERE world_id = $1")
            .bind(world_id().get())
            .fetch_one(&db.pool)
            .await
            .unwrap();
    let processed_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM processed_commands WHERE world_id = $1")
            .bind(world_id().get())
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert_eq!(economic.inventory.len() as i64, inventory_count);
    assert_eq!(economic.deposits.len() as i64, deposit_count);
    assert_eq!(economic.processed_command_ids.len() as i64, processed_count);
    assert_eq!(inventory_count, 2, "actor 둘 — 전제 확인");
    assert_eq!(deposit_count, 1, "매장지 하나(far-reach) — 전제 확인");
    assert_eq!(processed_count, 2, "command_id 둘 — 전제 확인");

    // (b) 필드 값 — 행 수만 맞고 내용이 섞이는 결함을 잡는다.
    let mineral_id = DataId::parse("starfall-glass").unwrap();
    let inventory_a = economic
        .inventory
        .iter()
        .find(|row| row.actor_id == actor_a)
        .expect("actor_a 행이 없다");
    assert_eq!(inventory_a.mineral_id, mineral_id);
    assert_eq!(inventory_a.quantity_kg, 25);
    let inventory_b = economic
        .inventory
        .iter()
        .find(|row| row.actor_id == actor_b)
        .expect("actor_b 행이 없다");
    assert_eq!(inventory_b.mineral_id, mineral_id);
    assert_eq!(inventory_b.quantity_kg, 25);

    let deposit = &economic.deposits[0];
    assert_eq!(deposit.deposit_id, DataId::parse("far-reach").unwrap());
    assert_eq!(deposit.remaining_kg, 450);
    assert_eq!(deposit.as_of_tick, 24060);
    assert_eq!(deposit.first_extracted_tick, 24000);

    let expected_commands: std::collections::BTreeSet<UuidV7> = [
        id("01a0b1c2-9c01-7d11-8e21-3f3140516171"),
        id("01a0b1c2-9c02-7d12-9e22-4f4150617181"),
    ]
    .into_iter()
    .collect();
    let actual_commands: std::collections::BTreeSet<UuidV7> =
        economic.processed_command_ids.iter().copied().collect();
    assert_eq!(actual_commands, expected_commands);

    db.drop().await;
}

/// 빈 월드(아직 아무도 채굴하지 않음)는 세 목록이 전부 빈 채로 성공한다 — 실패가
/// 아니라 "빈 시작 상태"가 정상 경로다(ADR-0013 §1 — 매장지 행은 첫 채굴 때만 생긴다).
#[tokio::test]
async fn load_economic_state_on_empty_world_returns_empty_lists() {
    let _serialize = starfall_persistence::test_hooks::INJECTION_LOCK
        .lock()
        .await;

    let name = "load_economic_state_on_empty_world_returns_empty_lists";
    let Some(db) = starfall_testdb::TestDb::create(name).await else {
        return;
    };
    seed_world(&db.pool, world_id()).await;

    let economic = starfall_persistence::load_economic_state(&db.pool, world_id())
        .await
        .expect("빈 월드에서도 성공해야 한다");

    assert!(economic.inventory.is_empty());
    assert!(economic.deposits.is_empty());
    assert!(economic.processed_command_ids.is_empty());

    db.drop().await;
}

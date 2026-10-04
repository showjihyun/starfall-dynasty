//! K·M. 역사 DB 통합 (AC-11·AC-13(c)) — `02_sprint_contract.md` §K·§M.
//!
//! 격리 DB 는 `starfall-testdb` 가 만든다(증거 DB `starfall` 은 건드리지 않는다). 판정
//! 대상 도메인 이벤트는 이 파일이 `domain_events` 에 **직접 SQL 로** 넣는다 — 커밋 경로
//! 자체의 정확성(계약 모양·CAS)은 S4 의 `economic_state.rs` 가 이미 잰다. 이 파일은 그
//! 아래 층, "이미 커밋된 행이 있을 때 역사 러너가 무엇을 하는가" 만 잰다.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::time::Duration;

use sqlx::{PgPool, Row};
use starfall_contracts::data::SignificanceRuleTable;
use starfall_contracts::primitives::UuidV7;
use starfall_persistence::history::{HistoryBoot, HistoryHandles, load, run_runner};
use tokio::sync::{mpsc, watch};

fn id(text: &str) -> UuidV7 {
    UuidV7::parse(text).unwrap_or_else(|| panic!("정규 UUIDv7 이어야 한다: {text}"))
}

/// 매 테스트마다 다른 월드를 쓴다(§0.2 "발견을 재는 모든 실행의 첫 단계").
fn fresh_world_id(salt: &str) -> UuidV7 {
    // salt 를 UUIDv7 의 임의 비트 구간에 섞어 넣어 테스트마다 다른 값을 결정적으로 만든다.
    let hash = salt.bytes().fold(0u32, |acc, b| {
        acc.wrapping_mul(31).wrapping_add(u32::from(b))
    });
    let text = format!("01a0e7f3-7869-7105-9103-{hash:012x}");
    id(&text)
}

/// `db.pool` 이 붙은 것과 **같은 격리 DB**를 가리키는 접속 문자열 — `TestDb` 는 이미 연
/// 풀만 공개하므로(`admin_url`은 비공개), `DATABASE_URL`의 관리 DB 부분을 그 DB 이름으로
/// 바꿔 재구성한다(`starfall-testdb`의 `with_database`와 같은 방식).
fn test_db_url(db: &starfall_testdb::TestDb) -> String {
    let base = std::env::var("DATABASE_URL")
        .expect("이 시점까지 왔다는 것 자체가 DATABASE_URL 이 있다는 뜻이다");
    match base.rfind('/') {
        Some(index) => format!("{}/{}", &base[..index], db.database_name()),
        None => format!("{base}/{}", db.database_name()),
    }
}

async fn seed_world(pool: &PgPool, world_id: UuidV7) {
    sqlx::query(
        "INSERT INTO worlds (world_id, name, tick_hz, calendar_epoch, calendar_scale, sim_version, last_tick)
         VALUES ($1, 'test', 20, '3800-01-01T00:00:00Z', 60, 1, NULL)",
    )
    .bind(world_id.get())
    .execute(pool)
    .await
    .expect("worlds 시드 실패");
}

async fn advance_watermark(pool: &PgPool, world_id: UuidV7, tick: u64) {
    sqlx::query("UPDATE worlds SET last_tick = $2 WHERE world_id = $1")
        .bind(world_id.get())
        .bind(i64::try_from(tick).unwrap())
        .execute(pool)
        .await
        .expect("watermark 갱신 실패");
}

const CORRELATION_ID: &str = "01a0b1c2-4a12-7c01-8d02-1e033f044a05";
const CAUSATION_ID: &str = "01a0b1c2-9c01-7d11-8e21-3f3140516171";

#[derive(Clone)]
struct MinedRow<'a> {
    event_id: UuidV7,
    tick: u64,
    sequence: u64,
    actor_id: UuidV7,
    ship_id: UuidV7,
    star_system_id: &'a str,
    deposit_id: &'a str,
    mineral_id: &'a str,
    quantity_kg: i64,
}

/// `domain_events` 에 `MINERAL_MINED` 행 하나를 직접 넣는다(정상 모양).
async fn insert_mineral_mined(pool: &PgPool, world_id: UuidV7, row: &MinedRow<'_>) {
    let payload = serde_json::json!({
        "ship_id": row.ship_id.to_string(),
        "session_id": "01a0b1c2-4a11-7b22-9c33-0d44e55f6a77",
        "star_system_id": row.star_system_id,
        "deposit_id": row.deposit_id,
        "mineral_id": row.mineral_id,
        "quantity_kg": row.quantity_kg,
        "quantity_before_kg": 0,
        "quantity_after_kg": row.quantity_kg,
        "deposit_remaining_before_kg": 500,
        "deposit_remaining_after_kg": 500 - row.quantity_kg,
    });
    sqlx::query(
        "INSERT INTO domain_events (
             event_id, event_type, schema_version, world_id, tick, sequence,
             occurred_at, recorded_at, correlation_id, causation_id, actor_id, payload
         ) VALUES ($1, 'MINERAL_MINED', 1, $2, $3, $4, '3800-01-01T20:00:00Z', now(), $5, $6, $7, $8)",
    )
    .bind(row.event_id.get())
    .bind(world_id.get())
    .bind(i64::try_from(row.tick).unwrap())
    .bind(i64::try_from(row.sequence).unwrap())
    .bind(id(CORRELATION_ID).get())
    .bind(id(CAUSATION_ID).get())
    .bind(row.actor_id.get())
    .bind(sqlx::types::Json(payload))
    .execute(pool)
    .await
    .expect("MINERAL_MINED 행 삽입 실패");
}

/// 판정 불가 `MINERAL_MINED` — payload 에서 `mineral_id` 를 뺀다(H1 이 요구하는 필드가
/// 없다). fail-stop(SC-55) 시연용.
async fn insert_undecodable_mineral_mined(
    pool: &PgPool,
    world_id: UuidV7,
    event_id: UuidV7,
    tick: u64,
) {
    let payload = serde_json::json!({
        "ship_id": id("01a0b1c2-8b01-7a11-8b22-9c33d44e55f6").to_string(),
        "session_id": "01a0b1c2-4a11-7b22-9c33-0d44e55f6a77",
        "star_system_id": "cradle",
        "deposit_id": "far-reach",
        // mineral_id 없음 — MineralMinedPayload 역직렬화가 실패해야 한다.
        "quantity_kg": 10,
        "quantity_before_kg": 0,
        "quantity_after_kg": 10,
        "deposit_remaining_before_kg": 500,
        "deposit_remaining_after_kg": 490,
    });
    sqlx::query(
        "INSERT INTO domain_events (
             event_id, event_type, schema_version, world_id, tick, sequence,
             occurred_at, recorded_at, correlation_id, causation_id, actor_id, payload
         ) VALUES ($1, 'MINERAL_MINED', 1, $2, $3, 0, '3800-01-01T20:00:00Z', now(), $4, $5,
                   $6, $7)",
    )
    .bind(event_id.get())
    .bind(world_id.get())
    .bind(i64::try_from(tick).unwrap())
    .bind(id(CORRELATION_ID).get())
    .bind(id(CAUSATION_ID).get())
    .bind(id("01a0b1c2-2c01-7a45-8b67-89abcdef0123").get())
    .bind(sqlx::types::Json(payload))
    .execute(pool)
    .await
    .expect("판정 불가 행 삽입 실패");
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

fn discovery_rule() -> SignificanceRuleTable {
    let path = repo_root().join("data/history/rules/mineral-discovery.json");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{} 읽기 실패: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{} 파싱 실패: {e}", path.display()))
}

/// `load()` 로 정상 재구축한 상태에서 [`drive_boot`] 를 돌린다 — 대부분의 테스트가 쓰는
/// 경로.
async fn drive_until(
    pool: PgPool,
    world_id: UuidV7,
    rule: SignificanceRuleTable,
    condition: impl Fn(&HistoryHandles) -> bool,
) -> (
    Vec<starfall_contracts::historical::MineralDiscoveredEvent>,
    HistoryHandles,
    Vec<starfall_contracts::historical::MineralDiscoveredEvent>,
) {
    let boot = load(&pool, world_id, rule).await.expect("load 실패");
    drive_boot(pool, world_id, boot, condition).await
}

/// [`run_runner`] 를 배경 태스크로 돌리고, 커밋 알림을 한 번 보낸 뒤 `handles` 의 어떤
/// 값이 조건을 만족할 때까지 짧게 폴링한다(고정 sleep 이 아니다 — 타이밍에 기대지 않는다).
/// 타임아웃 지나면 그 시점 handles 를 그대로 돌려준다(호출자가 실패를 판단한다).
///
/// `boot` 을 직접 받는다 — SC-105·111 은 [`HistoryBoot::empty`] 로 판정 상태를 일부러
/// 비운 채 넘긴다(정상 경로는 [`drive_until`] 이 쓰는 `load()` 뿐이다).
async fn drive_boot(
    pool: PgPool,
    world_id: UuidV7,
    boot: HistoryBoot,
    condition: impl Fn(&HistoryHandles) -> bool,
) -> (
    Vec<starfall_contracts::historical::MineralDiscoveredEvent>,
    HistoryHandles,
    Vec<starfall_contracts::historical::MineralDiscoveredEvent>,
) {
    let backfill = boot.initial_backfill.clone();
    let handles = HistoryHandles::new();
    let (commit_tx, commit_rx) = watch::channel(None);
    let (live_tx, mut live_rx) = mpsc::channel(16);
    let handles_for_runner = handles.clone();
    let runner = tokio::spawn(run_runner(
        pool,
        world_id,
        boot,
        commit_rx,
        live_tx,
        handles_for_runner,
    ));

    commit_tx.send_replace(Some(0)); // 깨우기 — 실제 값은 안 쓰인다(워터마크는 DB 에서).

    let mut waited = Duration::ZERO;
    let step = Duration::from_millis(20);
    let timeout = Duration::from_secs(5);
    while waited < timeout && !condition(&handles) {
        tokio::time::sleep(step).await;
        waited += step;
    }

    drop(commit_tx); // 채널을 닫아 러너가 마지막 한 번 더 따라잡고 끝나게 한다.
    let _ = tokio::time::timeout(Duration::from_secs(5), runner).await;

    let mut live = Vec::new();
    while let Ok(event) = live_rx.try_recv() {
        live.push(event);
    }

    (backfill, handles, live)
}

// ---------------------------------------------------------------------------
// SC-51 — 첫 채굴 커밋 뒤 역사 3표 각 1행 (AC-11(a), H-02)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn history_first_discovery_rows() {
    let name = "history_first_discovery_rows";
    let Some(db) = starfall_testdb::TestDb::create(name).await else {
        return;
    };
    let world_id = fresh_world_id(name);
    seed_world(&db.pool, world_id).await;

    // ⊘ 배제: 시작 시 그 월드의 역사 3표 행 수 = 0.
    for table in ["historical_events", "historical_event_sources", "evidence"] {
        let count: i64 = sqlx::query_scalar(&format!("SELECT count(*) FROM {table}"))
            .fetch_one(&db.pool)
            .await
            .unwrap();
        assert_eq!(count, 0, "{table} 은 시작 시 비어 있어야 한다");
    }

    let actor_id = id("01a0b1c2-2c01-7a45-8b67-89abcdef0123");
    let ship_id = id("01a0b1c2-8b01-7a11-8b22-9c33d44e55f6");
    let event_id = id("01a0b1c2-a001-7e01-8f11-505162738495");
    insert_mineral_mined(
        &db.pool,
        world_id,
        &MinedRow {
            event_id,
            tick: 24000,
            sequence: 0,
            actor_id,
            ship_id,
            star_system_id: "cradle",
            deposit_id: "far-reach",
            mineral_id: "starfall-glass",
            quantity_kg: 25,
        },
    )
    .await;
    advance_watermark(&db.pool, world_id, 24000).await;

    let (_, handles, live) = drive_until(db.pool.clone(), world_id, discovery_rule(), |h| {
        h.records_total.load(Ordering::Relaxed) >= 1
    })
    .await;

    assert_eq!(handles.records_total.load(Ordering::Relaxed), 1);
    assert!(!handles.halted.load(Ordering::Relaxed));
    assert_eq!(live.len(), 1, "LIVE 채널로 1건 나가야 한다");

    for table in ["historical_events", "historical_event_sources", "evidence"] {
        let count: i64 = sqlx::query_scalar(&format!("SELECT count(*) FROM {table}"))
            .fetch_one(&db.pool)
            .await
            .unwrap();
        assert_eq!(count, 1, "{table} 은 정확히 1행이어야 한다");
    }

    let source_event_ids: Vec<uuid::Uuid> =
        sqlx::query_scalar("SELECT source_event_ids FROM historical_events WHERE world_id = $1")
            .bind(world_id.get())
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert_eq!(
        source_event_ids,
        vec![event_id.get()],
        "source_event_ids 는 domain_events 에 존재하는 그 이벤트여야 한다"
    );

    let detector_rule: String =
        sqlx::query_scalar("SELECT detector_rule FROM historical_event_sources")
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert_eq!(
        detector_rule, "mineral-discovery@1",
        "detector_rule 은 rule_version 이다(architect 판정, 정본 ADR-0014 §4-2 — \
         2026-09-30. 0003 주석은 증거 DB 적용으로 동결돼 못 고친다)"
    );

    let evidence_row = sqlx::query(
        "SELECT rule_version, evidence_type, visibility, source_entity_id, authenticity_status,
                creation_method, derived_from_evidence_ids
         FROM evidence",
    )
    .fetch_one(&db.pool)
    .await
    .unwrap();
    let evidence_rule_version: String = evidence_row.try_get("rule_version").unwrap();
    let evidence_type: String = evidence_row.try_get("evidence_type").unwrap();
    let visibility: String = evidence_row.try_get("visibility").unwrap();
    let source_entity_id: uuid::Uuid = evidence_row.try_get("source_entity_id").unwrap();
    let authenticity_status: String = evidence_row.try_get("authenticity_status").unwrap();
    let creation_method: String = evidence_row.try_get("creation_method").unwrap();
    let derived_from: Vec<uuid::Uuid> = evidence_row.try_get("derived_from_evidence_ids").unwrap();
    assert_eq!(
        evidence_rule_version, "mineral-discovery@1",
        "증거도 규칙의 rule_version 을 가져야 한다(ADR-0014 §5, architect 지적)"
    );
    assert_eq!(evidence_type, "SHIP_LOG");
    assert_eq!(visibility, "PARTICIPANTS_ONLY");
    assert_eq!(source_entity_id, ship_id.get());
    assert_eq!(authenticity_status, "VERIFIED");
    assert_eq!(creation_method, "automatic");
    assert!(
        derived_from.is_empty(),
        "원본 증거는 derived_from 이 비어 있어야 한다"
    );

    db.drop().await;
}

// ---------------------------------------------------------------------------
// SC-52 — 역사 3표에 UPDATE/DELETE 전부 거부 (AC-11(b), H-07, I-63)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn history_tables_append_only() {
    let name = "history_tables_append_only";
    let Some(db) = starfall_testdb::TestDb::create(name).await else {
        return;
    };
    let world_id = fresh_world_id(name);
    seed_world(&db.pool, world_id).await;

    insert_mineral_mined(
        &db.pool,
        world_id,
        &MinedRow {
            event_id: id("01a0b1c2-a001-7e01-8f11-505162738495"),
            tick: 24000,
            sequence: 0,
            actor_id: id("01a0b1c2-2c01-7a45-8b67-89abcdef0123"),
            ship_id: id("01a0b1c2-8b01-7a11-8b22-9c33d44e55f6"),
            star_system_id: "cradle",
            deposit_id: "far-reach",
            mineral_id: "starfall-glass",
            quantity_kg: 25,
        },
    )
    .await;
    advance_watermark(&db.pool, world_id, 24000).await;
    let (_, handles, _) = drive_until(db.pool.clone(), world_id, discovery_rule(), |h| {
        h.records_total.load(Ordering::Relaxed) >= 1
    })
    .await;
    assert_eq!(handles.records_total.load(Ordering::Relaxed), 1);

    // ⊘ 배제: 대상 행이 실제로 있는지 먼저 확인한다(0행 매치로 트리거가 발화 안 하는 경우
    // 배제).
    for table in ["historical_events", "historical_event_sources", "evidence"] {
        let count: i64 = sqlx::query_scalar(&format!("SELECT count(*) FROM {table}"))
            .fetch_one(&db.pool)
            .await
            .unwrap();
        assert_eq!(count, 1, "{table} 에 시도 대상 행이 있어야 한다");
    }

    let attempts: [(&str, &str); 6] = [
        (
            "historical_events",
            "UPDATE historical_events SET importance_level = 5",
        ),
        ("historical_events", "DELETE FROM historical_events"),
        (
            "historical_event_sources",
            "UPDATE historical_event_sources SET detector_rule = 'x'",
        ),
        (
            "historical_event_sources",
            "DELETE FROM historical_event_sources",
        ),
        ("evidence", "UPDATE evidence SET evidence_type = 'x'"),
        ("evidence", "DELETE FROM evidence"),
    ];
    for (table, sql) in attempts {
        let error = sqlx::query(sql)
            .execute(&db.pool)
            .await
            .expect_err(&format!("{table} 변형이 거부되지 않았다: {sql}"));
        let db_error = error.as_database_error().expect("DB 오류여야 한다");
        assert_eq!(
            db_error.code().as_deref(),
            Some("P0001"),
            "{table}: SQLSTATE 가 P0001 이어야 한다 — {error}"
        );
        assert!(
            db_error.message().contains(table),
            "{table}: 오류 메시지에 표 이름이 있어야 한다 — {}",
            db_error.message()
        );
    }

    // 양성 대조: 가변 표 history_cursor 의 UPDATE 는 성공해야 한다(트리거가 "모든
    // UPDATE 를 막는 버그" 가 아님을 확인).
    let cursor_update = sqlx::query("UPDATE history_cursor SET tick = tick WHERE world_id = $1")
        .bind(world_id.get())
        .execute(&db.pool)
        .await
        .expect("history_cursor 의 UPDATE 는 성공해야 한다");
    assert_eq!(cursor_update.rows_affected(), 1);

    // 시도 뒤 행 수 불변.
    for table in ["historical_events", "historical_event_sources", "evidence"] {
        let count: i64 = sqlx::query_scalar(&format!("SELECT count(*) FROM {table}"))
            .fetch_one(&db.pool)
            .await
            .unwrap();
        assert_eq!(count, 1, "{table} 은 시도 뒤에도 1행이어야 한다");
    }

    db.drop().await;
}

// ---------------------------------------------------------------------------
// SC-53 — 같은 dedupe_key 두 번째 행을 SQL 로 직접 INSERT → UNIQUE 위반 (AC-11(c), I-61)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn history_dedupe_unique() {
    let name = "history_dedupe_unique";
    let Some(db) = starfall_testdb::TestDb::create(name).await else {
        return;
    };
    let world_id = fresh_world_id(name);
    seed_world(&db.pool, world_id).await;

    insert_mineral_mined(
        &db.pool,
        world_id,
        &MinedRow {
            event_id: id("01a0b1c2-a001-7e01-8f11-505162738495"),
            tick: 24000,
            sequence: 0,
            actor_id: id("01a0b1c2-2c01-7a45-8b67-89abcdef0123"),
            ship_id: id("01a0b1c2-8b01-7a11-8b22-9c33d44e55f6"),
            star_system_id: "cradle",
            deposit_id: "far-reach",
            mineral_id: "starfall-glass",
            quantity_kg: 25,
        },
    )
    .await;
    advance_watermark(&db.pool, world_id, 24000).await;
    let (_, handles, _) = drive_until(db.pool.clone(), world_id, discovery_rule(), |h| {
        h.records_total.load(Ordering::Relaxed) >= 1
    })
    .await;
    assert_eq!(handles.records_total.load(Ordering::Relaxed), 1);

    let existing: i64 = sqlx::query_scalar("SELECT count(*) FROM historical_events")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(existing, 1, "첫 행이 있어야 이 테스트가 의미 있다");

    let error = sqlx::query(
        "INSERT INTO historical_events (
             historical_event_id, world_id, event_type, dedupe_key, rule_version,
             importance_level, tick, occurred_at, recorded_at, visibility, fact_status,
             source_event_ids, location, participants, payload
         ) SELECT gen_random_uuid(), world_id, event_type, dedupe_key, rule_version,
                  importance_level, tick, occurred_at, now(), visibility, fact_status,
                  source_event_ids, location, participants, payload
           FROM historical_events LIMIT 1",
    )
    .execute(&db.pool)
    .await
    .expect_err("같은 dedupe_key 의 두 번째 행이 거부되지 않았다");
    let db_error = error.as_database_error().expect("DB 오류여야 한다");
    assert_eq!(db_error.code().as_deref(), Some("23505"));

    db.drop().await;
}

// ---------------------------------------------------------------------------
// SC-55 — fail-stop (AC-11(e), H-10)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn history_fail_stop() {
    let name = "history_fail_stop";
    let Some(db) = starfall_testdb::TestDb::create(name).await else {
        return;
    };
    let world_id = fresh_world_id(name);
    seed_world(&db.pool, world_id).await;

    let broken_id = id("01a0b1c2-a002-7e02-9f12-6061728394a5");
    insert_undecodable_mineral_mined(&db.pool, world_id, broken_id, 24000).await;

    // 정지 지점 뒤의 발견 후보(있어야 이 테스트가 무효가 아니다).
    let later_id = id("01a0b1c2-a003-7e03-9f13-707384950617");
    insert_mineral_mined(
        &db.pool,
        world_id,
        &MinedRow {
            event_id: later_id,
            tick: 24001,
            sequence: 0,
            actor_id: id("01a0b1c2-2c01-7a45-8b67-89abcdef0123"),
            ship_id: id("01a0b1c2-8b01-7a11-8b22-9c33d44e55f6"),
            star_system_id: "cradle",
            deposit_id: "far-reach",
            mineral_id: "starfall-glass",
            quantity_kg: 25,
        },
    )
    .await;
    advance_watermark(&db.pool, world_id, 24001).await;

    let (_, handles, live) = drive_until(db.pool.clone(), world_id, discovery_rule(), |h| {
        h.halted.load(Ordering::Relaxed)
    })
    .await;

    assert!(handles.halted.load(Ordering::Relaxed), "러너가 멈춰야 한다");
    assert_eq!(
        handles.records_total.load(Ordering::Relaxed),
        0,
        "뒤 후보가 기록되면 안 된다"
    );
    assert!(live.is_empty());

    let cursor: Option<i64> = sqlx::query_scalar(
        "SELECT tick FROM history_cursor WHERE world_id = $1 AND consumer = 'mineral-discovery'",
    )
    .bind(world_id.get())
    .fetch_optional(&db.pool)
    .await
    .unwrap();
    assert!(
        cursor.is_none() || cursor.unwrap() < 24001,
        "커서가 정지 지점을 넘어가면 안 된다"
    );

    // 같은 시간 채굴(=domain_events 커밋)은 계속된다 — history 가 안 막았다는 것을
    // 경제 쪽 행이 그대로 있는 것으로 확인한다.
    let mined_rows: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM domain_events WHERE world_id = $1 AND event_type = 'MINERAL_MINED'",
    )
    .bind(world_id.get())
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(
        mined_rows, 2,
        "정지 이후에도 커밋된 MINERAL_MINED 행은 그대로 있다"
    );

    db.drop().await;
}

// ---------------------------------------------------------------------------
// SC-56 — 워터마크 (AC-11(f), H-11)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn history_watermark() {
    let name = "history_watermark";
    let Some(db) = starfall_testdb::TestDb::create(name).await else {
        return;
    };
    let world_id = fresh_world_id(name);
    seed_world(&db.pool, world_id).await;

    // last_tick 을 0 에 둔 채, 그보다 큰 tick 의 후보를 넣는다.
    advance_watermark(&db.pool, world_id, 0).await;
    insert_mineral_mined(
        &db.pool,
        world_id,
        &MinedRow {
            event_id: id("01a0b1c2-a001-7e01-8f11-505162738495"),
            tick: 500,
            sequence: 0,
            actor_id: id("01a0b1c2-2c01-7a45-8b67-89abcdef0123"),
            ship_id: id("01a0b1c2-8b01-7a11-8b22-9c33d44e55f6"),
            star_system_id: "cradle",
            deposit_id: "far-reach",
            mineral_id: "starfall-glass",
            quantity_kg: 25,
        },
    )
    .await;

    // ⊘ 배제: tick > last_tick 인 후보가 실제로 있음을 먼저 단언한다.
    let beyond_watermark: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM domain_events de JOIN worlds w ON w.world_id = de.world_id
         WHERE de.world_id = $1 AND de.tick > COALESCE(w.last_tick, -1)",
    )
    .bind(world_id.get())
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert!(beyond_watermark >= 1);

    let (_, handles, _) = drive_until(db.pool.clone(), world_id, discovery_rule(), |h| {
        h.catchup_rows_total.load(Ordering::Relaxed) >= 1
    })
    .await;

    // 워터마크 밖이라 아무것도 처리되지 않았어야 한다.
    assert_eq!(handles.records_total.load(Ordering::Relaxed), 0);
    assert_eq!(handles.catchup_rows_total.load(Ordering::Relaxed), 0);
    assert!(!handles.halted.load(Ordering::Relaxed));

    db.drop().await;
}

// ---------------------------------------------------------------------------
// SC-57 — 재기동: 발견 수 불변 + 재기동 뒤 신규 발견 +1 (AC-11(g), S-7, H-15)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn history_restart_rebuild() {
    let name = "history_restart_rebuild";
    let Some(db) = starfall_testdb::TestDb::create(name).await else {
        return;
    };
    let world_id = fresh_world_id(name);
    seed_world(&db.pool, world_id).await;

    insert_mineral_mined(
        &db.pool,
        world_id,
        &MinedRow {
            event_id: id("01a0b1c2-a001-7e01-8f11-505162738495"),
            tick: 24000,
            sequence: 0,
            actor_id: id("01a0b1c2-2c01-7a45-8b67-89abcdef0123"),
            ship_id: id("01a0b1c2-8b01-7a11-8b22-9c33d44e55f6"),
            star_system_id: "cradle",
            deposit_id: "far-reach",
            mineral_id: "starfall-glass",
            quantity_kg: 25,
        },
    )
    .await;
    insert_mineral_mined(
        &db.pool,
        world_id,
        &MinedRow {
            event_id: id("01a0b1c2-a002-7e02-9f12-6061728394a5"),
            tick: 24001,
            sequence: 0,
            actor_id: id("01a0b1c2-2c02-7b46-9c68-9abcdef01234"),
            ship_id: id("01a0b1c2-8b02-7a12-9b23-ac34d55e66f7"),
            star_system_id: "cradle",
            deposit_id: "vela-orbit-1",
            mineral_id: "glacine",
            quantity_kg: 100,
        },
    )
    .await;
    advance_watermark(&db.pool, world_id, 24001).await;

    let (_, handles1, _) = drive_until(db.pool.clone(), world_id, discovery_rule(), |h| {
        h.records_total.load(Ordering::Relaxed) >= 2
    })
    .await;
    assert_eq!(
        handles1.records_total.load(Ordering::Relaxed),
        2,
        "재기동 전 기록 2건"
    );

    let count_before_restart: i64 = sqlx::query_scalar("SELECT count(*) FROM historical_events")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(count_before_restart, 2);

    // "재기동" = 새 load() + 새 run_runner() (러너가 죽어 "2 유지"가 공짜가 아니게, 이
    // 두 번째 구동에서도 실제로 진행해야 한다 — 아래에서 3번째 발견으로 증명한다).
    insert_mineral_mined(
        &db.pool,
        world_id,
        &MinedRow {
            event_id: id("01a0b1c2-a003-7e03-9f13-707384950617"),
            tick: 91234,
            sequence: 0,
            actor_id: id("01a0b1c2-2c03-7c47-9d69-abcdef012345"),
            ship_id: id("01a0b1c2-8b03-7b13-9c34-bd45e66f7708"),
            star_system_id: "cradle",
            deposit_id: "vela-orbit-2",
            mineral_id: "cobaltine",
            quantity_kg: 40,
        },
    )
    .await;
    advance_watermark(&db.pool, world_id, 91234).await;

    let (backfill2, handles2, _) = drive_until(db.pool.clone(), world_id, discovery_rule(), |h| {
        h.records_total.load(Ordering::Relaxed) >= 1
    })
    .await;
    assert_eq!(
        backfill2.len(),
        2,
        "재기동 시점 BACKFILL 목록은 이전 발견 2건이어야 한다"
    );
    assert_eq!(
        handles2.records_total.load(Ordering::Relaxed),
        1,
        "재기동 뒤 이 구동에서 신규 1건"
    );

    let count_after_restart: i64 = sqlx::query_scalar("SELECT count(*) FROM historical_events")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(
        count_after_restart, 3,
        "총합은 3이 돼야 한다(재기동 뒤 신규 발견 양성 대조)"
    );

    db.drop().await;
}

// ---------------------------------------------------------------------------
// SC-105 · SC-106 · SC-111 — AC-11(h) 세 주입, 같은 실행
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// SC-105 · SC-111 — AC-11(h)(ii)·(iii) 다른/같은 근거 충돌 (같은 코드 경로)
// ---------------------------------------------------------------------------
//
// 정상적으로 도는 러너는 이 상태에 절대 도달하지 않는다 — `load()` 가 `historical_events`
// 를 언제나 올바르게 재구축하므로, 이미 아는 `(성계, 광물)` 은 DB 쓰기를 시도하지도 않고
// 코어 안에서 조용히 걸러진다(SC-42). ADR-0014 §4 의 DB `UNIQUE` 제약은 **그 재구축
// 자체가 실패했을 때**의 최후 방어선이다 — 그래서 이 재현은 `HistoryBoot::empty()` 로
// 판정 상태를 일부러 비워, "DB 에는 이미 행이 있는데 코어는 처음 보는 척" 하는 상황을
// 만든다(history 검토 §11.4 — qa 에게 통보한 주입 방식).

/// 두 테스트(SC-105·SC-111)가 공유하는 주입 헬퍼 — 다른 것은 `injected_source_event_id`
/// 뿐이다(SC-105 = 실제 이벤트와 다른 근거, SC-111 = 같은 근거).
async fn seed_conflicting_row(
    pool: &PgPool,
    world_id: UuidV7,
    dedupe_key: &str,
    historical_event_id: uuid::Uuid,
    injected_source_event_id: UuidV7,
    injected_quantity_kg: i64,
) {
    sqlx::query(
        "INSERT INTO historical_events (
             historical_event_id, world_id, event_type, dedupe_key, rule_version,
             importance_level, tick, occurred_at, recorded_at, visibility, fact_status,
             source_event_ids, location, participants, payload
         ) VALUES ($1, $2, 'MINERAL_DISCOVERED', $3, 'mineral-discovery@1', 2, 24000,
                   '3800-01-01T20:00:00Z', now(), 'PUBLIC', 'CONFIRMED', $4, $5, $6, $7)",
    )
    .bind(historical_event_id)
    .bind(world_id.get())
    .bind(dedupe_key)
    .bind(vec![injected_source_event_id.get()])
    .bind(sqlx::types::Json(serde_json::json!({"star_system_id": "cradle"})))
    .bind(sqlx::types::Json(serde_json::json!([
        {"entity_id": id("01a0b1c2-2c01-7a45-8b67-89abcdef0123").to_string(), "entity_kind": "PLAYER", "role": "DISCOVERER"},
        {"entity_id": id("01a0b1c2-8b01-7a11-8b22-9c33d44e55f6").to_string(), "entity_kind": "SHIP", "role": "VESSEL"},
    ])))
    .bind(sqlx::types::Json(serde_json::json!({
        "mineral_id": "starfall-glass", "deposit_id": "far-reach", "quantity_kg": injected_quantity_kg
    })))
    .execute(pool)
    .await
    .expect("주입 행 삽입 실패");
}

/// SC-105(AC-11(h)(ii)) — **다른 근거**의 충돌: 주입 행의 `source_event_ids` 가 실제로
/// 이긴 이벤트와 **다르다**. 정상 러너는 이 상태에 도달하지 않는다(재구축이 언제나
/// 옳다) — `HistoryBoot::empty()` 로 판정 상태를 비워 시뮬레이션한다(history 검토
/// §11.4).
#[tokio::test]
async fn history_conflict_halts_runner_only() {
    let name = "history_conflict_halts_runner_only";
    let Some(db) = starfall_testdb::TestDb::create(name).await else {
        return;
    };
    let world_id = fresh_world_id(name);
    seed_world(&db.pool, world_id).await;

    let dedupe_key = "cradle/starfall-glass";
    let winning_event_id = id("01a0b1c2-a001-7e01-8f11-505162738495");
    // 주입 행의 근거는 이 이벤트가 아니다 — 다른(가짜) event_id.
    let injected_source_event_id = id("01a0b1c2-fa01-7a01-8b11-505162738497");
    let historical_event_id = starfall_history::id::historical_event_id(
        &world_id.to_string(),
        "MINERAL_DISCOVERED",
        dedupe_key,
    );
    seed_conflicting_row(
        &db.pool,
        world_id,
        dedupe_key,
        historical_event_id.get(),
        injected_source_event_id,
        25, // payload 는 실제 채굴과 같아도 된다 — 근거 자체가 다르다는 것이 이 케이스의 핵심.
    )
    .await;

    // ⊘ 배제: 주입 행이 실제로 있고, 그 근거가 실제 이벤트와 다름을 먼저 단언한다.
    let injected_source: Vec<uuid::Uuid> = sqlx::query_scalar(
        "SELECT source_event_ids FROM historical_events WHERE historical_event_id = $1",
    )
    .bind(historical_event_id.get())
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(injected_source, vec![injected_source_event_id.get()]);
    assert_ne!(
        injected_source_event_id, winning_event_id,
        "SC-105 는 근거 자체가 달라야 한다(SC-111 과 구분되는 지점)"
    );

    insert_mineral_mined(
        &db.pool,
        world_id,
        &MinedRow {
            event_id: winning_event_id,
            tick: 24000,
            sequence: 0,
            actor_id: id("01a0b1c2-2c01-7a45-8b67-89abcdef0123"),
            ship_id: id("01a0b1c2-8b01-7a11-8b22-9c33d44e55f6"),
            star_system_id: "cradle",
            deposit_id: "far-reach",
            mineral_id: "starfall-glass",
            quantity_kg: 25,
        },
    )
    .await;
    advance_watermark(&db.pool, world_id, 24000).await;

    let boot = HistoryBoot::empty(discovery_rule());
    let (_, handles, live) = drive_boot(db.pool.clone(), world_id, boot, |h| {
        h.conflicts_total.load(Ordering::Relaxed) >= 1 || h.halted.load(Ordering::Relaxed)
    })
    .await;

    assert_eq!(
        handles.conflicts_total.load(Ordering::Relaxed),
        1,
        "다른 근거 충돌이 1건 잡혀야 한다(SC-105)"
    );
    assert!(handles.halted.load(Ordering::Relaxed), "러너만 멈춰야 한다");
    assert!(live.is_empty(), "충돌한 기록은 LIVE 로 나가면 안 된다");

    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM historical_events")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(count, 1, "충돌한 쓰기는 롤백돼 행 수가 늘면 안 된다");

    let mined_rows: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM domain_events WHERE world_id = $1 AND event_type = 'MINERAL_MINED'",
    )
    .bind(world_id.get())
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(
        mined_rows, 1,
        "같은 시간 채굴은 계속 커밋됐다 — history 가 경제를 막지 않는다"
    );

    db.drop().await;
}

/// SC-111(AC-11(h)(iii)) — **같은 근거인데 내용이 다른** 충돌: 주입 행의
/// `source_event_ids` 가 실제로 이긴 이벤트와 **같다**(같은 근거) — payload 만 다르다.
/// golden 이 놓친 규칙 변경을 잡는 대조(architect). SC-105 와 근거가 다르다는 점에서만
/// 갈린다(verdict 를 가르는 성질 — 규칙 5).
#[tokio::test]
async fn history_same_source_different_content_halts() {
    let name = "history_same_source_different_content_halts";
    let Some(db) = starfall_testdb::TestDb::create(name).await else {
        return;
    };
    let world_id = fresh_world_id(name);
    seed_world(&db.pool, world_id).await;

    let dedupe_key = "cradle/starfall-glass";
    let winning_event_id = id("01a0b1c2-a001-7e01-8f11-505162738495");
    let historical_event_id = starfall_history::id::historical_event_id(
        &world_id.to_string(),
        "MINERAL_DISCOVERED",
        dedupe_key,
    );
    // 근거는 실제 이벤트와 **같다** — payload(quantity_kg)만 999 로 다르다.
    seed_conflicting_row(
        &db.pool,
        world_id,
        dedupe_key,
        historical_event_id.get(),
        winning_event_id,
        999,
    )
    .await;

    // ⊘ 배제: 주입 행이 있고, 러너 출력과 다른 곳이 payload 뿐임을 먼저 단언한다 —
    // 근거(source_event_ids)는 같아야 한다(다르면 SC-105 를 재는 것이다).
    let row = sqlx::query(
        "SELECT source_event_ids, payload FROM historical_events WHERE historical_event_id = $1",
    )
    .bind(historical_event_id.get())
    .fetch_one(&db.pool)
    .await
    .unwrap();
    let injected_source: Vec<uuid::Uuid> = row.try_get("source_event_ids").unwrap();
    let injected_payload: serde_json::Value = row.try_get("payload").unwrap();
    assert_eq!(
        injected_source,
        vec![winning_event_id.get()],
        "근거는 같아야 한다"
    );
    assert_eq!(injected_payload["quantity_kg"], 999);

    insert_mineral_mined(
        &db.pool,
        world_id,
        &MinedRow {
            event_id: winning_event_id,
            tick: 24000,
            sequence: 0,
            actor_id: id("01a0b1c2-2c01-7a45-8b67-89abcdef0123"),
            ship_id: id("01a0b1c2-8b01-7a11-8b22-9c33d44e55f6"),
            star_system_id: "cradle",
            deposit_id: "far-reach",
            mineral_id: "starfall-glass",
            quantity_kg: 25, // 실제 채굴은 25 — 주입 행의 999 와 다르다.
        },
    )
    .await;
    advance_watermark(&db.pool, world_id, 24000).await;

    let boot = HistoryBoot::empty(discovery_rule());
    let (_, handles, live) = drive_boot(db.pool.clone(), world_id, boot, |h| {
        h.conflicts_total.load(Ordering::Relaxed) >= 1 || h.halted.load(Ordering::Relaxed)
    })
    .await;

    assert_eq!(
        handles.conflicts_total.load(Ordering::Relaxed),
        1,
        "같은 근거·다른 내용 충돌이 1건 잡혀야 한다(SC-111)"
    );
    assert!(handles.halted.load(Ordering::Relaxed), "러너만 멈춰야 한다");
    assert!(live.is_empty(), "충돌한 기록은 LIVE 로 나가면 안 된다");

    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM historical_events")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(count, 1, "충돌한 쓰기는 롤백돼 행 수가 늘면 안 된다");
    let still_injected: serde_json::Value =
        sqlx::query_scalar("SELECT payload FROM historical_events WHERE historical_event_id = $1")
            .bind(historical_event_id.get())
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert_eq!(
        still_injected["quantity_kg"], 999,
        "롤백됐으니 주입 행 값 그대로다"
    );

    let mined_rows: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM domain_events WHERE world_id = $1 AND event_type = 'MINERAL_MINED'",
    )
    .bind(world_id.get())
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(
        mined_rows, 1,
        "같은 시간 채굴은 계속 커밋됐다 — history 가 경제를 막지 않는다"
    );

    db.drop().await;
}

// ---------------------------------------------------------------------------
// SC-106 — 재배달은 조용함(SC-105·111 의 양성 대조) (AC-11(h)(i))
// ---------------------------------------------------------------------------

#[tokio::test]
async fn history_redelivery_is_silent() {
    let name = "history_redelivery_is_silent";
    let Some(db) = starfall_testdb::TestDb::create(name).await else {
        return;
    };
    let world_id = fresh_world_id(name);
    seed_world(&db.pool, world_id).await;

    let event_id = id("01a0b1c2-a001-7e01-8f11-505162738495");
    insert_mineral_mined(
        &db.pool,
        world_id,
        &MinedRow {
            event_id,
            tick: 24000,
            sequence: 0,
            actor_id: id("01a0b1c2-2c01-7a45-8b67-89abcdef0123"),
            ship_id: id("01a0b1c2-8b01-7a11-8b22-9c33d44e55f6"),
            star_system_id: "cradle",
            deposit_id: "far-reach",
            mineral_id: "starfall-glass",
            quantity_kg: 25,
        },
    )
    .await;
    advance_watermark(&db.pool, world_id, 24000).await;
    let (_, handles_first, _) = drive_until(db.pool.clone(), world_id, discovery_rule(), |h| {
        h.records_total.load(Ordering::Relaxed) >= 1
    })
    .await;
    assert_eq!(handles_first.records_total.load(Ordering::Relaxed), 1);

    // 커서를 되감는다 — 재처리 실행이 실제로 그 이벤트를 다시 본다.
    sqlx::query(
        "UPDATE history_cursor SET tick = 0, sequence = 0
         WHERE world_id = $1 AND consumer = 'mineral-discovery'",
    )
    .bind(world_id.get())
    .execute(&db.pool)
    .await
    .expect("커서 되감기 실패");
    let rewound: i64 = sqlx::query_scalar(
        "SELECT tick FROM history_cursor WHERE world_id = $1 AND consumer = 'mineral-discovery'",
    )
    .bind(world_id.get())
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(rewound, 0, "커서가 실제로 되감겨야 한다");

    let (_, handles_second, _) = drive_until(db.pool.clone(), world_id, discovery_rule(), |h| {
        h.catchup_rows_total.load(Ordering::Relaxed) >= 1
    })
    .await;
    assert!(
        handles_second.catchup_rows_total.load(Ordering::Relaxed) >= 1,
        "되감은 뒤 재처리가 실제로 일어나야 한다"
    );
    assert_eq!(
        handles_second.conflicts_total.load(Ordering::Relaxed),
        0,
        "재배달은 충돌 카운터를 올리면 안 된다"
    );
    assert!(
        !handles_second.halted.load(Ordering::Relaxed),
        "재배달은 정지하면 안 된다"
    );
    let count_after_redelivery: i64 = sqlx::query_scalar("SELECT count(*) FROM historical_events")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(count_after_redelivery, 1, "재배달로 행이 늘면 안 된다");

    db.drop().await;
}

// ---------------------------------------------------------------------------
// SC-47 — 러너 정렬 경로: 삽입 순서를 섞어도 발견 집합이 같다 (AC-10(g), H-03)
// ---------------------------------------------------------------------------
//
// history r0 검토: 코어에 정렬한 입력을 넣는 형태는 (tick, sequence) 가 유일해 정렬
// 후 바이트가 같아지고, 정렬하는 쪽이 테스트 자신이라 자명 통과다. 실제 정렬 경로는
// 러너의 SQL ORDER BY 다 — 그래서 domain_events 에 삽입 순서만 바꿔 넣고, 매번 새
// 테스트 DB 에서 러너를 처음부터 돌려 결과가 같은지 본다.

/// 6개 고정 이벤트: 광물 2 곱하기 채굴자 3. 각 광물 안에 같은 tick 충돌 1쌍을 포함한다.
/// 반환 순서가 정답 순서( (tick, sequence) 오름차순 )다 — 인덱스 0 이 starfall-glass
/// 의 승자, 3이 glacine 의 승자.
fn ordering_fixture() -> Vec<MinedRow<'static>> {
    vec![
        MinedRow {
            event_id: id("01a0b1c2-f001-7a01-8b11-505162738491"),
            tick: 100,
            sequence: 5,
            actor_id: id("01a0b1c2-2c01-7a45-8b67-89abcdef0123"),
            ship_id: id("01a0b1c2-8b01-7a11-8b22-9c33d44e55f6"),
            star_system_id: "cradle",
            deposit_id: "far-reach",
            mineral_id: "starfall-glass",
            quantity_kg: 25,
        },
        MinedRow {
            event_id: id("01a0b1c2-f002-7a02-8b12-505162738492"),
            tick: 100,
            sequence: 9,
            actor_id: id("01a0b1c2-2c02-7b46-9c68-9abcdef01234"),
            ship_id: id("01a0b1c2-8b02-7a12-9b23-ac34d55e66f7"),
            star_system_id: "cradle",
            deposit_id: "far-reach",
            mineral_id: "starfall-glass",
            quantity_kg: 30,
        },
        MinedRow {
            event_id: id("01a0b1c2-f003-7a03-8b13-505162738493"),
            tick: 200,
            sequence: 0,
            actor_id: id("01a0b1c2-2c03-7c47-9d69-abcdef012345"),
            ship_id: id("01a0b1c2-8b03-7b13-9c34-bd45e66f7708"),
            star_system_id: "cradle",
            deposit_id: "far-reach",
            mineral_id: "starfall-glass",
            quantity_kg: 12,
        },
        MinedRow {
            // glacine 승자 — event_id 가 아래 패자보다 사전순으로 뒤다(⊘ iii).
            event_id: id("01a0b1c2-f994-7a04-8b14-505162738494"),
            tick: 150,
            sequence: 1,
            actor_id: id("01a0b1c2-2c04-7c48-9d70-bcdef0123456"),
            ship_id: id("01a0b1c2-8b04-7a14-9c35-ce56f77081ab"),
            star_system_id: "cradle",
            deposit_id: "vela-orbit-1",
            mineral_id: "glacine",
            quantity_kg: 100,
        },
        MinedRow {
            event_id: id("01a0b1c2-f005-7a05-8b15-505162738495"),
            tick: 150,
            sequence: 7,
            actor_id: id("01a0b1c2-2c05-7c49-9d71-cdef01234567"),
            ship_id: id("01a0b1c2-8b05-7a15-9c36-df67f7708abc"),
            star_system_id: "cradle",
            deposit_id: "vela-orbit-1",
            mineral_id: "glacine",
            quantity_kg: 80,
        },
        MinedRow {
            event_id: id("01a0b1c2-f006-7a06-8b16-505162738496"),
            tick: 300,
            sequence: 0,
            actor_id: id("01a0b1c2-2c06-7c50-9d72-def012345678"),
            ship_id: id("01a0b1c2-8b06-7a16-9c37-ef78f7709bcd"),
            star_system_id: "cradle",
            deposit_id: "vela-orbit-1",
            mineral_id: "glacine",
            quantity_kg: 50,
        },
    ]
}

/// `index` 번째 순열(0 기반, 팩토리얼 진법/Lehmer 코드)을 만든다. `0 <= index <
/// elems.len()!` 이면 서로 다른 순열을 낸다.
fn nth_permutation<T: Clone>(elems: &[T], mut index: usize) -> Vec<T> {
    let mut pool: Vec<T> = elems.to_vec();
    let mut out = Vec::with_capacity(elems.len());
    let mut radix = pool.len();
    while radix > 0 {
        let factorial = (1..radix).product::<usize>().max(1);
        let choice = index / factorial;
        index %= factorial;
        out.push(pool.remove(choice));
        radix -= 1;
    }
    out
}

#[tokio::test]
async fn history_runner_ordering_permutations() {
    let fixture = ordering_fixture();
    // ⊘(iii): event_id 순서가 (tick, sequence) 순서와 다른 쌍이 실제로 있다.
    assert!(
        fixture[3].event_id.to_string() > fixture[4].event_id.to_string(),
        "glacine 승자의 event_id 가 패자보다 사전순으로 뒤여야 한다(⊘ iii)"
    );
    assert!(
        (fixture[3].tick, fixture[3].sequence) < (fixture[4].tick, fixture[4].sequence),
        "glacine 승자가 더 작은 (tick, sequence) 를 가져야 한다"
    );

    const PERMUTATIONS: usize = 24; // 6! = 720 중 24개(요구 20 이상)를 실제로 돈다.
    let expected_winners = [fixture[0].event_id, fixture[3].event_id];

    let mut argmin_inserted_last_count = 0usize;
    let mut physical_order_differs_count = 0usize;

    for perm_index in 0..PERMUTATIONS {
        let order = nth_permutation(&fixture, perm_index);
        // 계약 이름 그대로 — 24번 다 같은 이름으로 RAN 줄을 찍는다(census 는 이름
        // 집합을 보지, 중복 등장을 문제 삼지 않는다). 월드 id 는 perm_index 로 따로
        // 갈라 각 순열이 독립 DB·독립 월드에서 돈다.
        let name = "history_runner_ordering_permutations";
        let Some(db) = starfall_testdb::TestDb::create(name).await else {
            return;
        };
        let world_id = fresh_world_id(&format!("{name}::{perm_index}"));
        seed_world(&db.pool, world_id).await;

        for row in &order {
            insert_mineral_mined(&db.pool, world_id, row).await;
        }
        advance_watermark(&db.pool, world_id, 300).await;

        // ⊘(i): 이번 순열에서 두 승자 중 하나가 물리적으로 마지막에 삽입됐는가.
        let last_inserted = order.last().expect("6개 원소").event_id;
        if expected_winners.contains(&last_inserted) {
            argmin_inserted_last_count += 1;
        }

        // ⊘(ii): ORDER BY 없는 SELECT(ctid = 물리 순서)가 (tick, sequence) 정렬과
        // 실제로 다른가.
        let physical: Vec<uuid::Uuid> =
            sqlx::query_scalar("SELECT event_id FROM domain_events ORDER BY ctid")
                .fetch_all(&db.pool)
                .await
                .unwrap();
        let sorted: Vec<uuid::Uuid> =
            sqlx::query_scalar("SELECT event_id FROM domain_events ORDER BY tick, sequence")
                .fetch_all(&db.pool)
                .await
                .unwrap();
        if physical != sorted {
            physical_order_differs_count += 1;
        }

        let (_, handles, _) = drive_until(db.pool.clone(), world_id, discovery_rule(), |h| {
            h.records_total.load(Ordering::Relaxed) >= 2
        })
        .await;

        assert_eq!(
            handles.records_total.load(Ordering::Relaxed),
            2,
            "순열 {perm_index}: 발견은 언제나 2건이어야 한다(삽입 순서와 무관)"
        );
        let mut winners: Vec<uuid::Uuid> = sqlx::query_scalar(
            "SELECT unnest(source_event_ids) FROM historical_events WHERE world_id = $1 ORDER BY tick",
        )
        .bind(world_id.get())
        .fetch_all(&db.pool)
        .await
        .unwrap();
        winners.sort();
        let mut expected: Vec<uuid::Uuid> = expected_winners.iter().map(|w| w.get()).collect();
        expected.sort();
        assert_eq!(
            winners, expected,
            "순열 {perm_index}: 발견자 집합이 오라클(삽입 순서 무관 정답)과 달라졌다"
        );

        db.drop().await;
    }

    println!(
        "SC-47: 순열 {PERMUTATIONS}개 실행. 승자 이벤트가 물리적으로 마지막에 삽입된 순열 \
         {argmin_inserted_last_count}건, ORDER BY 없는 물리 순서가 정렬 순서와 다른 순열 \
         {physical_order_differs_count}건"
    );
    assert!(
        argmin_inserted_last_count >= 1,
        "⊘(i): 승자가 물리적으로 마지막에 삽입된 순열이 최소 1건 있어야 한다"
    );
    assert!(
        physical_order_differs_count >= 1,
        "⊘(ii): 물리 순서가 정렬 순서와 다른 순열이 최소 1건 있어야 한다"
    );
}

// ---------------------------------------------------------------------------
// SC-54 — 커밋 직전 장애 → 재시작 → 기록 1, 커서가 그 이벤트 뒤 (AC-11(d), H-02)
// ---------------------------------------------------------------------------
//
// **방법론 메모**: 진짜 프로세스 크래시는 이 테스트 하네스에서 못 만든다. 대신 러너가
// 쓰는 것과 같은 INSERT 를 별도 트랜잭션에서 실행해 두고 COMMIT 대신 ROLLBACK 한다 —
// Postgres 의 트랜잭션 원자성 자체가 "커밋 직전에 죽은 프로세스" 와 같은 DB 상태(그
// 쓰기가 없던 일이 된다)를 보장하므로, 이것이 크래시 주입의 정확한 시뮬레이션이다.
// S4 의 유사 테스트(`cas_mismatch_halts`)와 달리 여기서는 커밋 자체를 주입하지, 값
// 불일치를 주입하지 않는다.

#[tokio::test]
async fn history_crash_before_commit() {
    let name = "history_crash_before_commit";
    let Some(db) = starfall_testdb::TestDb::create(name).await else {
        return;
    };
    let world_id = fresh_world_id(name);
    seed_world(&db.pool, world_id).await;

    let event_id = id("01a0b1c2-a001-7e01-8f11-505162738495");
    insert_mineral_mined(
        &db.pool,
        world_id,
        &MinedRow {
            event_id,
            tick: 24000,
            sequence: 0,
            actor_id: id("01a0b1c2-2c01-7a45-8b67-89abcdef0123"),
            ship_id: id("01a0b1c2-8b01-7a11-8b22-9c33d44e55f6"),
            star_system_id: "cradle",
            deposit_id: "far-reach",
            mineral_id: "starfall-glass",
            quantity_kg: 25,
        },
    )
    .await;
    advance_watermark(&db.pool, world_id, 24000).await;

    let dedupe_key = "cradle/starfall-glass";
    let historical_event_id = starfall_history::id::historical_event_id(
        &world_id.to_string(),
        "MINERAL_DISCOVERED",
        dedupe_key,
    );

    // 주입: 러너가 커밋했을 것과 같은 모양의 쓰기를 트랜잭션 안에서 해 보고, 그 트랜잭션
    // 안에서는 실제로 보임을 확인한 뒤(⊘: 주입 지점이 실행됐다) ROLLBACK 한다.
    let mut tx = db.pool.begin().await.expect("트랜잭션 시작 실패");
    let inject_result = sqlx::query(
        "INSERT INTO historical_events (
             historical_event_id, world_id, event_type, dedupe_key, rule_version,
             importance_level, tick, occurred_at, recorded_at, visibility, fact_status,
             source_event_ids, location, participants, payload
         ) VALUES ($1, $2, 'MINERAL_DISCOVERED', $3, 'mineral-discovery@1', 2, 24000,
                   '3800-01-01T20:00:00Z', now(), 'PUBLIC', 'CONFIRMED', $4, $5, $6, $7)",
    )
    .bind(historical_event_id.get())
    .bind(world_id.get())
    .bind(dedupe_key)
    .bind(vec![event_id.get()])
    .bind(sqlx::types::Json(serde_json::json!({"star_system_id": "cradle"})))
    .bind(sqlx::types::Json(serde_json::json!([
        {"entity_id": id("01a0b1c2-2c01-7a45-8b67-89abcdef0123").to_string(), "entity_kind": "PLAYER", "role": "DISCOVERER"},
        {"entity_id": id("01a0b1c2-8b01-7a11-8b22-9c33d44e55f6").to_string(), "entity_kind": "SHIP", "role": "VESSEL"},
    ])))
    .bind(sqlx::types::Json(
        serde_json::json!({"mineral_id": "starfall-glass", "deposit_id": "far-reach", "quantity_kg": 25}),
    ))
    .execute(&mut *tx)
    .await
    .expect("주입 INSERT 실패");
    assert_eq!(
        inject_result.rows_affected(),
        1,
        "주입 지점이 실행됐어야 한다(⊘)"
    );

    let visible_inside_tx: i64 =
        sqlx::query_scalar("SELECT count(*) FROM historical_events WHERE historical_event_id = $1")
            .bind(historical_event_id.get())
            .fetch_one(&mut *tx)
            .await
            .unwrap();
    assert_eq!(visible_inside_tx, 1, "트랜잭션 안에서는 보여야 한다");

    tx.rollback().await.expect("롤백 실패"); // "크래시" — 커밋되지 않는다.

    // 장애 직후: 주입이 커밋 전이었으므로 기록 = 0.
    let after_crash: i64 = sqlx::query_scalar("SELECT count(*) FROM historical_events")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(after_crash, 0, "커밋 전 롤백이므로 기록이 없어야 한다");
    let cursor_after_crash: Option<i64> = sqlx::query_scalar(
        "SELECT tick FROM history_cursor WHERE world_id = $1 AND consumer = 'mineral-discovery'",
    )
    .bind(world_id.get())
    .fetch_optional(&db.pool)
    .await
    .unwrap();
    assert!(
        cursor_after_crash.is_none(),
        "커서도 전진하지 않았어야 한다"
    );

    // "재시작" — 실제 러너가 처음부터(커서 없음) 같은 이벤트를 정상 처리한다.
    let (_, handles, _) = drive_until(db.pool.clone(), world_id, discovery_rule(), |h| {
        h.records_total.load(Ordering::Relaxed) >= 1
    })
    .await;
    assert_eq!(
        handles.records_total.load(Ordering::Relaxed),
        1,
        "재시작 뒤 정상 처리돼 기록이 1건이어야 한다"
    );
    let cursor_after: (i64, i64) = sqlx::query_as(
        "SELECT tick, sequence FROM history_cursor WHERE world_id = $1 AND consumer = 'mineral-discovery'",
    )
    .bind(world_id.get())
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(
        cursor_after,
        (24000, 0),
        "커서가 그 이벤트 좌표까지 전진해야 한다"
    );

    db.drop().await;
}

// ---------------------------------------------------------------------------
// SC-62 — 커밋 뒤에만 송신한다 (AC-13(c), I-65)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn history_notice_after_commit_only() {
    // 양성 대조 — 주입 없는 정상 처리에서는 1건이 나간다(⊘ 배제: "주입 없는 경우에도
    // 0건" — 송신 경로 자체가 죽은 게 아님을 먼저 보인다).
    let ok_name = "history_notice_after_commit_only";
    let Some(ok_db) = starfall_testdb::TestDb::create(ok_name).await else {
        return;
    };
    let ok_world = fresh_world_id(&format!("{ok_name}::baseline"));
    seed_world(&ok_db.pool, ok_world).await;
    insert_mineral_mined(
        &ok_db.pool,
        ok_world,
        &MinedRow {
            event_id: id("01a0b1c2-a001-7e01-8f11-505162738495"),
            tick: 24000,
            sequence: 0,
            actor_id: id("01a0b1c2-2c01-7a45-8b67-89abcdef0123"),
            ship_id: id("01a0b1c2-8b01-7a11-8b22-9c33d44e55f6"),
            star_system_id: "cradle",
            deposit_id: "far-reach",
            mineral_id: "starfall-glass",
            quantity_kg: 25,
        },
    )
    .await;
    advance_watermark(&ok_db.pool, ok_world, 24000).await;
    let (_, ok_handles, ok_live) =
        drive_until(ok_db.pool.clone(), ok_world, discovery_rule(), |h| {
            h.records_total.load(Ordering::Relaxed) >= 1
        })
        .await;
    assert_eq!(ok_handles.records_total.load(Ordering::Relaxed), 1);
    assert_eq!(
        ok_live.len(),
        1,
        "주입 없는 정상 처리는 LIVE 1건을 내야 한다"
    );
    ok_db.drop().await;

    // 주입 — 커밋을 실패시킨다: 러너가 만들 evidence_id 를 미리 SQL 로 심어 둬서, 러너의
    // evidence INSERT 가 PK 충돌(23505)로 실패하고 그 트랜잭션 전체가 커밋되지 못하게
    // 한다.
    let Some(db) = starfall_testdb::TestDb::create(ok_name).await else {
        return;
    };
    let world_id = fresh_world_id(&format!("{ok_name}::injected"));
    seed_world(&db.pool, world_id).await;
    let event_id = id("01a0b1c2-a001-7e01-8f11-505162738495");
    insert_mineral_mined(
        &db.pool,
        world_id,
        &MinedRow {
            event_id,
            tick: 24000,
            sequence: 0,
            actor_id: id("01a0b1c2-2c01-7a45-8b67-89abcdef0123"),
            ship_id: id("01a0b1c2-8b01-7a11-8b22-9c33d44e55f6"),
            star_system_id: "cradle",
            deposit_id: "far-reach",
            mineral_id: "starfall-glass",
            quantity_kg: 25,
        },
    )
    .await;
    advance_watermark(&db.pool, world_id, 24000).await;

    let dedupe_key = "cradle/starfall-glass";
    let historical_event_id = starfall_history::id::historical_event_id(
        &world_id.to_string(),
        "MINERAL_DISCOVERED",
        dedupe_key,
    );
    let evidence_id = starfall_history::id::evidence_id(&historical_event_id, "SHIP_LOG");
    // 아무 다른 historical_event 를 가리키는 자리표시자 행 — evidence.historical_event_id
    // FK 만 만족하면 되므로, 먼저 그 행 자체를 만든다(사실 내용은 안 쓰인다).
    sqlx::query(
        "INSERT INTO historical_events (
             historical_event_id, world_id, event_type, dedupe_key, rule_version,
             importance_level, tick, occurred_at, recorded_at, visibility, fact_status,
             source_event_ids, location, participants, payload
         ) VALUES ($1, $2, 'MINERAL_DISCOVERED', 'cradle/glacine', 'mineral-discovery@1', 2,
                   1, '3800-01-01T20:00:00Z', now(), 'PUBLIC', 'CONFIRMED', $3, $4, $5, $6)",
    )
    .bind(
        starfall_history::id::historical_event_id(
            &world_id.to_string(),
            "MINERAL_DISCOVERED",
            "cradle/glacine",
        )
        .get(),
    )
    .bind(world_id.get())
    .bind(vec![id("01a0b1c2-a999-7e99-9f99-999999999999").get()])
    .bind(sqlx::types::Json(serde_json::json!({"star_system_id": "cradle"})))
    .bind(sqlx::types::Json(serde_json::json!([
        {"entity_id": id("01a0b1c2-2c01-7a45-8b67-89abcdef0123").to_string(), "entity_kind": "PLAYER", "role": "DISCOVERER"},
        {"entity_id": id("01a0b1c2-8b01-7a11-8b22-9c33d44e55f6").to_string(), "entity_kind": "SHIP", "role": "VESSEL"},
    ])))
    .bind(sqlx::types::Json(
        serde_json::json!({"mineral_id": "glacine", "deposit_id": "vela-orbit-1", "quantity_kg": 1}),
    ))
    .execute(&db.pool)
    .await
    .expect("자리표시자 historical_events 행 삽입 실패");
    let placeholder_id: uuid::Uuid =
        sqlx::query_scalar("SELECT historical_event_id FROM historical_events LIMIT 1")
            .fetch_one(&db.pool)
            .await
            .unwrap();
    sqlx::query(
        "INSERT INTO evidence (evidence_id, historical_event_id, rule_version, evidence_type, visibility, source_entity_id)
         VALUES ($1, $2, 'mineral-discovery@1', 'SHIP_LOG', 'PARTICIPANTS_ONLY', $3)",
    )
    .bind(evidence_id.get())
    .bind(placeholder_id)
    .bind(id("01a0b1c2-8b01-7a11-8b22-9c33d44e55f6").get())
    .execute(&db.pool)
    .await
    .expect("주입 evidence 행 삽입 실패(⊘ — 이 행이 실제로 들어가야 아래에서 PK 충돌이 난다)");

    let (_, handles, live) = drive_until(db.pool.clone(), world_id, discovery_rule(), |h| {
        h.catchup_rows_total.load(Ordering::Relaxed) >= 1
    })
    .await;

    assert_eq!(
        handles.records_total.load(Ordering::Relaxed),
        0,
        "커밋이 실패했으니 기록도 0건이어야 한다"
    );
    assert!(
        live.is_empty(),
        "커밋되지 않은 처리는 LIVE 로 나가면 안 된다(커밋 전 송신 금지, ADR-0014 §6)"
    );

    db.drop().await;
}

// ---------------------------------------------------------------------------
// SC-59 — 읽기 전용 재생 도구 (AC-12(b), H-05·H-06)
// ---------------------------------------------------------------------------
//
// `history-replay` CLI(`bin/history-replay.rs`)는 이 파일이 이미 짠
// `starfall_persistence::history::replay_and_compare` 를 그대로 부른다 — 여기서는 그
// 함수 자체를 DB 로 검증한다. CLI 껍질(인자 파싱·데이터 파일 경로 해석)은 수동으로
// `cargo run --bin history-replay -- --world <실제 월드> --read-only` 를 돌려 실측했다
// (7987행 domain_events 를 가진 실제 개발 DB 의 월드 하나로 636개 이벤트를 재생해
// 저장된 발견 4건과 id 까지 정확히 일치함을 확인 — history 구현 요약 참고).

#[tokio::test]
async fn history_replay_matches_stored_records() {
    let name = "history_replay_matches_stored_records";
    let Some(db) = starfall_testdb::TestDb::create(name).await else {
        return;
    };
    let world_id = fresh_world_id(name);
    seed_world(&db.pool, world_id).await;

    insert_mineral_mined(
        &db.pool,
        world_id,
        &MinedRow {
            event_id: id("01a0b1c2-a001-7e01-8f11-505162738495"),
            tick: 24000,
            sequence: 0,
            actor_id: id("01a0b1c2-2c01-7a45-8b67-89abcdef0123"),
            ship_id: id("01a0b1c2-8b01-7a11-8b22-9c33d44e55f6"),
            star_system_id: "cradle",
            deposit_id: "far-reach",
            mineral_id: "starfall-glass",
            quantity_kg: 25,
        },
    )
    .await;
    insert_mineral_mined(
        &db.pool,
        world_id,
        &MinedRow {
            event_id: id("01a0b1c2-a002-7e02-9f12-6061728394a5"),
            tick: 24001,
            sequence: 0,
            actor_id: id("01a0b1c2-2c02-7b46-9c68-9abcdef01234"),
            ship_id: id("01a0b1c2-8b02-7a12-9b23-ac34d55e66f7"),
            star_system_id: "cradle",
            deposit_id: "vela-orbit-1",
            mineral_id: "glacine",
            quantity_kg: 100,
        },
    )
    .await;
    advance_watermark(&db.pool, world_id, 24001).await;

    // 정상 경로로 실제 역사를 만든다(재생이 비교할 대상).
    let (_, handles, _) = drive_until(db.pool.clone(), world_id, discovery_rule(), |h| {
        h.records_total.load(Ordering::Relaxed) >= 2
    })
    .await;
    assert_eq!(handles.records_total.load(Ordering::Relaxed), 2);

    // ⊘ 배제: 재생 대상이 실제로 있다.
    let report =
        starfall_persistence::history::replay_and_compare(&db.pool, world_id, discovery_rule())
            .await
            .expect("재생 실패");
    assert_eq!(report.domain_events_replayed, 2);
    assert_eq!(
        report.records_replayed, 2,
        "재생한 기록 수 ≥ 2 여야 이 항목이 의미 있다"
    );
    assert!(
        report.is_match(),
        "정상 상태에서는 재생과 저장이 완전히 같아야 한다: {:?}",
        report.mismatches
    );

    db.drop().await;
}

/// 음성 대조 — 재생 도구가 실제로 불일치를 잡는지, 저장된 행의 내용을 **주입으로**
/// 바꿔서 본다(append-only 트리거가 UPDATE 를 막으므로, "실수로 바뀐 것"이 아니라
/// SC-105/111 과 같은 방식으로 처음부터 다른 내용을 심어 넣는다).
#[tokio::test]
async fn history_replay_detects_content_mismatch() {
    let name = "history_replay_detects_content_mismatch";
    let Some(db) = starfall_testdb::TestDb::create(name).await else {
        return;
    };
    let world_id = fresh_world_id(name);
    seed_world(&db.pool, world_id).await;

    let event_id = id("01a0b1c2-a001-7e01-8f11-505162738495");
    insert_mineral_mined(
        &db.pool,
        world_id,
        &MinedRow {
            event_id,
            tick: 24000,
            sequence: 0,
            actor_id: id("01a0b1c2-2c01-7a45-8b67-89abcdef0123"),
            ship_id: id("01a0b1c2-8b01-7a11-8b22-9c33d44e55f6"),
            star_system_id: "cradle",
            deposit_id: "far-reach",
            mineral_id: "starfall-glass",
            quantity_kg: 25,
        },
    )
    .await;
    advance_watermark(&db.pool, world_id, 24000).await;

    let dedupe_key = "cradle/starfall-glass";
    let historical_event_id = starfall_history::id::historical_event_id(
        &world_id.to_string(),
        "MINERAL_DISCOVERED",
        dedupe_key,
    );
    // 러너를 거치지 않고 "저장된 것"을 직접 심는다 — payload.quantity_kg 가 실제 채굴
    // (25)과 다르다. 재생은 실제 domain_events(25)로 판정하므로 이 불일치를 잡아야 한다.
    seed_conflicting_row(
        &db.pool,
        world_id,
        dedupe_key,
        historical_event_id.get(),
        event_id, // 근거는 같다 — 그래도 payload 내용이 다르면 재생이 잡아야 한다.
        999,
    )
    .await;

    let report =
        starfall_persistence::history::replay_and_compare(&db.pool, world_id, discovery_rule())
            .await
            .expect("재생 실패");
    assert_eq!(report.domain_events_replayed, 1);
    assert_eq!(report.records_replayed, 1);
    assert!(
        !report.is_match(),
        "저장된 내용이 재생과 다른데 재생 도구가 그것을 못 잡았다"
    );
    assert!(
        report
            .mismatches
            .iter()
            .any(|m| m.contains(&historical_event_id.to_string())),
        "불일치 목록에 그 historical_event_id 가 있어야 한다: {:?}",
        report.mismatches
    );

    db.drop().await;
}

/// `read_only_pool`이 구조적으로 쓰기를 막는지 — "쓰기 코드가 없다"(소스 부정)에 더해
/// DB 자체가 거부하는지 직접 시도해 본다(team-lead 지시, 2026-09-30).
#[tokio::test]
async fn history_replay_pool_rejects_writes() {
    let name = "history_replay_pool_rejects_writes";
    let Some(db) = starfall_testdb::TestDb::create(name).await else {
        return;
    };
    let world_id = fresh_world_id(name);
    seed_world(&db.pool, world_id).await;

    let url = test_db_url(&db);
    let read_only =
        starfall_persistence::history::read_only_pool(&url).expect("읽기 전용 풀 생성 실패");

    // 양성 대조: 읽기는 그대로 된다(풀 자체가 죽은 게 아니다).
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM worlds WHERE world_id = $1")
        .bind(world_id.get())
        .fetch_one(&read_only)
        .await
        .expect("읽기 전용 풀에서 SELECT 는 성공해야 한다");
    assert_eq!(count, 1);

    // ⊘ 배제: 시도 자체가 실행됐다 — 아래 UPDATE 가 대상 행을 실제로 겨냥한다.
    let error = sqlx::query("UPDATE worlds SET name = 'mutated' WHERE world_id = $1")
        .bind(world_id.get())
        .execute(&read_only)
        .await
        .expect_err("읽기 전용 풀에서 UPDATE 가 거부되지 않았다");
    let db_error = error.as_database_error().expect("DB 오류여야 한다");
    assert_eq!(
        db_error.code().as_deref(),
        Some("25006"),
        "SQLSTATE 가 read_only_sql_transaction(25006) 이어야 한다: {error}"
    );

    // 행이 실제로 안 바뀌었음을 일반 풀로 재확인한다.
    let name_after: String = sqlx::query_scalar("SELECT name FROM worlds WHERE world_id = $1")
        .bind(world_id.get())
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(
        name_after, "test",
        "거부된 UPDATE 가 실제로 행을 안 바꿨어야 한다"
    );

    read_only.close().await;
    db.drop().await;
}

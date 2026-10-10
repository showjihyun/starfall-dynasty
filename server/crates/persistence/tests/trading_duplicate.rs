//! S4 DB 통합 테스트 — 판매의 중복 방지(K1, ADR-0016 §1: `MINE_RESOURCE`와 같은 월드
//! 범위 지속 기억·`processed_commands` 장부).
//!
//! `trading_sale.rs`와 같은 월드·시장 상수 패턴을 쓴다(이 파일 전용 id로 분리).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::too_many_arguments
)]

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use starfall_contracts::messages::{CommandStatus, RejectReasonCode};
use starfall_contracts::primitives::{
    ConstSchemaVersion, DataId, GameCalendar, GameTime, MassKg, RuleVersion, ServerVersion, UuidV7,
};
use starfall_contracts::primitives::{Sequence, Tick};
use starfall_contracts::{
    CommandResultMessage, MineralSoldPayload, SellMineralCommand, SellMineralPayload,
    SellMineralType,
};
use starfall_persistence::PersistHandles;
use starfall_sim::world::{BoundaryConstants, ShipClassConstants, Vec3};
use starfall_sim::{
    DepositConstants, DomainEventBody, IdSource, InboundCommand, ListingConstants, MarketConstants,
    MiningRuleConstants, PendingEvent, PersistBatch, PriceRule, ServerMessage, ShipClassData,
    Simulation, StateWrite, Submission, TickOutcome, WorldConstants,
};
use tokio::sync::{mpsc, watch};

fn id(text: &str) -> UuidV7 {
    UuidV7::parse(text).unwrap_or_else(|| panic!("정규 UuidV7 이어야 한다: {text}"))
}

fn world_id() -> UuidV7 {
    id("01a0e7f8-2c3d-7e4f-8a5b-6c7d8e9f0a12")
}

fn mass(value: i32) -> MassKg {
    MassKg::new(value).unwrap()
}

const MARKET_ID: &str = "cradle-exchange";
const MINERAL_ID: &str = "ferrosite";
const SPAWN_POINT_M: [f64; 3] = [500.0, 500.0, 500.0];

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

async fn seed_inventory_row(pool: &sqlx::PgPool, world: UuidV7, actor: UuidV7, qty: i64) {
    sqlx::query(
        "INSERT INTO inventory_items (world_id, actor_id, mineral_id, quantity_kg)
         VALUES ($1, $2, $3, $4)",
    )
    .bind(world.get())
    .bind(actor.get())
    .bind(MINERAL_ID)
    .bind(qty)
    .execute(pool)
    .await
    .expect("inventory 시드 실패");
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

fn price_rule() -> PriceRule {
    PriceRule {
        base_price_cr_per_kg: 10,
        band_kg: 2000,
        price_floor_pct: 30,
        price_drop_pct_per_band: 10,
    }
}

fn world() -> WorldConstants {
    let mut listings = BTreeMap::new();
    listings.insert(
        DataId::parse(MINERAL_ID).unwrap(),
        ListingConstants {
            price: price_rule(),
            recovery_kg: 1000,
        },
    );
    let mut markets = BTreeMap::new();
    markets.insert(
        DataId::parse(MARKET_ID).unwrap(),
        MarketConstants {
            position_m: Vec3::new(SPAWN_POINT_M[0], SPAWN_POINT_M[1], SPAWN_POINT_M[2]),
            radius_m: 10.0,
            dock_range_from_surface_m: 50.0,
            max_ship_speed_mps: 5.0,
            sell_cooldown_ticks: 0,
            recovery_interval_ticks: 10,
            pricing_version: RuleVersion::parse("cradle-exchange@1").unwrap(),
            listings,
        },
    );

    WorldConstants {
        world_id: world_id(),
        calendar: GameCalendar::new(20, GameTime::parse("3800-01-01T00:00:00Z").unwrap(), 60)
            .unwrap(),
        server_version: ServerVersion::parse("0.1.0").unwrap(),
        star_system_id: DataId::parse("cradle").unwrap(),
        boundary: BoundaryConstants {
            soft_boundary_radius_m: 10_000.0,
            hard_boundary_radius_m: 12_000.0,
            boundary_pull_mps2: 25.0,
        },
        spawn_points_m: vec![SPAWN_POINT_M],
        spawn_clearance_m: 1.0,
        spawn_max_probe_attempts: 12,
        spawn_radial_offset_step_m: 5.0,
        spawn_world_seed: 7,
        linger_seconds: 30.0,
        reconnect_resume_window_seconds: 30.0,
        ship_class_id: DataId::parse("scout-s01").unwrap(),
        ship_class: ShipClassData {
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
        },
        snapshot_interval_ticks: 200,
        carry_forward_max_ticks: 10_000,
        rate_limit_per_tick_cap: 5,
        max_entities_per_snapshot: 64,
        minerals: BTreeMap::new(),
        deposits: BTreeMap::from([(
            DataId::parse("test-deposit").unwrap(),
            DepositConstants {
                mineral_id: DataId::parse(MINERAL_ID).unwrap(),
                position_m: Vec3::new(0.0, 0.0, 0.0),
                radius_m: 1.0,
                initial_reserve_kg: 1,
            },
        )]),
        mining_rules: MiningRuleConstants {
            mining_range_from_surface_m: 150.0,
            max_ship_speed_mps: 10.0,
            cooldown_ticks: 0,
        },
        markets,
    }
}

struct SeqIds(u64);
impl IdSource for SeqIds {
    fn next_id(&mut self) -> UuidV7 {
        self.0 += 1;
        id(&format!(
            "01a0b1c2-0000-7{:03x}-8{:03x}-{:012x}",
            self.0 & 0xfff,
            (self.0 >> 12) & 0xfff,
            self.0
        ))
    }
}

fn sell_mineral(command_id: UuidV7, qty: i32) -> InboundCommand {
    InboundCommand::SellMineral(SellMineralCommand {
        command_id,
        command_type: SellMineralType::SellMineral,
        schema_version: ConstSchemaVersion,
        client_sent_at: None,
        payload: SellMineralPayload {
            market_id: DataId::parse(MARKET_ID).unwrap(),
            mineral_id: DataId::parse(MINERAL_ID).unwrap(),
            quantity_kg: mass(qty),
        },
    })
}

fn open_session(sim: &mut Simulation, ids: &mut SeqIds, session_id: UuidV7, actor_id: UuidV7) {
    sim.step(
        vec![Submission::OpenSession {
            seq: 0,
            session_id,
            actor_id,
        }],
        ids,
    );
}

fn sell(
    sim: &mut Simulation,
    ids: &mut SeqIds,
    session_id: UuidV7,
    command_id: UuidV7,
    qty: i32,
) -> TickOutcome {
    sim.step(
        vec![Submission::Command {
            seq: 1,
            session_id,
            command: sell_mineral(command_id, qty),
        }],
        ids,
    )
}

fn command_result(outcome: &TickOutcome) -> &CommandResultMessage {
    outcome
        .outbound
        .iter()
        .find_map(|out| match &out.message {
            ServerMessage::CommandResult(message) => Some(message.as_ref()),
            _ => None,
        })
        .expect("COMMAND_RESULT 가 있어야 한다")
}

fn batch_from(outcome: TickOutcome) -> PersistBatch {
    PersistBatch {
        tick: outcome.tick,
        events: outcome.events,
        state_writes: outcome.state_writes,
    }
}

/// SC-24(AC-5(c), ADR-0013 §7) — 정상 종료 → 재기동 → 같은 actor → 같은 명령 →
/// 중복, 지갑·인벤토리·`processed_commands`·`market_states` 행 불변. "적재 →
/// sim 기억 주입 경로" — `restart_economic_state.rs`의 S8 패턴 그대로, DB를
/// 바이패스하지 않는다: 재적재된 sim 이 **스스로** 중복을 거른다(DB PK 가 아니라
/// K1 이 먼저 막는다는 것을 확인한다).
#[tokio::test]
async fn sale_dup_across_restart_is_rejected() {
    let name = "sale_dup_across_restart_is_rejected";
    let Some(db) = starfall_testdb::TestDb::create(name).await else {
        return;
    };
    seed_world(&db.pool, world_id()).await;

    let actor = id("01a0b1c2-7a01-7a01-8a01-1a2b3c4d5f01");
    let mineral_id = DataId::parse(MINERAL_ID).unwrap();
    seed_inventory_row(&db.pool, world_id(), actor, 4000).await;

    let mut sim = Simulation::new(world(), 100);
    sim.seed_inventory(actor, mineral_id.clone(), 4000);
    let mut ids = SeqIds(0);
    let session_id = id("01a0b1c2-7b01-7b01-8b01-1a2b3c4d5f02");
    open_session(&mut sim, &mut ids, session_id, actor);

    let old_command_id = id("01a0b1c2-7c01-7c01-8c01-1a2b3c4d5f03");
    let first_outcome = sell(&mut sim, &mut ids, session_id, old_command_id, 2000);
    assert_eq!(
        command_result(&first_outcome).payload.status,
        CommandStatus::Accepted
    );
    let first_handles =
        run_batches(db.pool.clone(), world_id(), vec![batch_from(first_outcome)]).await;
    assert!(
        !first_handles.halted.load(Ordering::Relaxed),
        "전제: 첫 판매는 멈추면 안 된다"
    );

    // ⊘ 배제 — 재기동 흉내 전에 네 표의 행 수를 먼저 잰다.
    let wallets_before = row_count(&db.pool, "wallets", world_id()).await;
    let inventory_before = row_count(&db.pool, "inventory_items", world_id()).await;
    let processed_before = row_count(&db.pool, "processed_commands", world_id()).await;
    let markets_before = row_count(&db.pool, "market_states", world_id()).await;
    assert_eq!(
        wallets_before, 1,
        "⊘ 전제: 첫 판매로 지갑 행이 생겼어야 한다"
    );

    // ── "재기동" ──────────────────────────────────────────────────────────
    let economic = starfall_persistence::load_economic_state(&db.pool, world_id())
        .await
        .expect("경제 상태 적재 실패");
    assert!(
        economic.processed_command_ids.contains(&old_command_id),
        "처리 장부에 옛 command_id 가 있어야 한다"
    );

    let mut restarted_sim = Simulation::new(world(), 102);
    for row in economic.inventory {
        restarted_sim.seed_inventory(row.actor_id, row.mineral_id, row.quantity_kg);
    }
    for row in economic.wallets {
        restarted_sim.seed_wallet(row.actor_id, row.balance_cr);
    }
    for row in economic.market_states {
        restarted_sim.seed_market_state(
            row.market_id,
            row.mineral_id,
            row.saturation_kg,
            row.as_of_tick,
            row.last_seller_actor_id,
            row.last_sale_quantity_kg,
        );
    }
    restarted_sim.seed_processed_command_ids(economic.processed_command_ids);

    let mut restarted_ids = SeqIds(1000);
    let restarted_session = id("01a0b1c2-7b02-7b02-8b02-1a2b3c4d5f04");
    open_session(
        &mut restarted_sim,
        &mut restarted_ids,
        restarted_session,
        actor,
    );

    let resend_outcome = restarted_sim.step(
        vec![Submission::Command {
            seq: 1,
            session_id: restarted_session,
            command: sell_mineral(old_command_id, 2000), // 같은 command_id 재전송.
        }],
        &mut restarted_ids,
    );
    let result = command_result(&resend_outcome);
    assert_eq!(result.payload.status, CommandStatus::Rejected);
    assert_eq!(
        result.payload.reason_code,
        Some(RejectReasonCode::DuplicateCommandId),
        "재기동 전에 sim 이 이 command_id 를 본 적 없어도 적재된 K1 이 막아야 한다"
    );
    assert!(
        resend_outcome.state_writes.is_empty() && resend_outcome.events.is_empty(),
        "거절된 명령은 어떤 상태 쓰기·이벤트도 만들지 않는다 — DB 까지 갈 배치가 없다"
    );

    // 행 수 불변 — 거절이 sim 층에서 끝났으므로 persistence 를 호출조차 안 했다.
    assert_eq!(
        row_count(&db.pool, "wallets", world_id()).await,
        wallets_before
    );
    assert_eq!(
        row_count(&db.pool, "inventory_items", world_id()).await,
        inventory_before
    );
    assert_eq!(
        row_count(&db.pool, "processed_commands", world_id()).await,
        processed_before
    );
    assert_eq!(
        row_count(&db.pool, "market_states", world_id()).await,
        markets_before
    );

    db.drop().await;
}

/// SC-26(DB)(AC-5(d2), ADR-0013 §5a) — 다른 actor 가 같은 `command_id` 를 쓴 경우,
/// sim 기억이 먼저 하나를 막아도(별도로 `starfall-sim` 단위 테스트가 잰다) **영속화에
/// 도달하는 것은 수락된 쪽뿐**이고 서버는 멈추지 않는다. 이 DB 테스트는 그 나머지
/// 절반: 서로 다른 actor 의 **서로 다른** command_id 가 같은 월드에서 둘 다 문제없이
/// 커밋된다(PK 가 `(world_id, command_id)` 뿐이라 actor 신원은 커밋에 영향이 없다).
#[tokio::test]
async fn sale_dup_cross_actor_no_halt() {
    let name = "sale_dup_cross_actor_no_halt";
    let Some(db) = starfall_testdb::TestDb::create(name).await else {
        return;
    };
    seed_world(&db.pool, world_id()).await;

    let actor_a = id("01a0b1c2-7a03-7a03-8a03-1a2b3c4d5f05");
    let actor_b = id("01a0b1c2-7a04-7a04-8a04-1a2b3c4d5f06");
    let mineral_id = DataId::parse(MINERAL_ID).unwrap();
    seed_inventory_row(&db.pool, world_id(), actor_a, 4000).await;
    seed_inventory_row(&db.pool, world_id(), actor_b, 4000).await;

    let mut sim = Simulation::new(world(), 100);
    sim.seed_inventory(actor_a, mineral_id.clone(), 4000);
    sim.seed_inventory(actor_b, mineral_id, 4000);
    let mut ids = SeqIds(0);
    let session_a = id("01a0b1c2-7b05-7b05-8b05-1a2b3c4d5f07");
    let session_b = id("01a0b1c2-7b06-7b06-8b06-1a2b3c4d5f08");
    open_session(&mut sim, &mut ids, session_a, actor_a);
    open_session(&mut sim, &mut ids, session_b, actor_b);

    let command_a = id("01a0b1c2-7c05-7c05-8c05-1a2b3c4d5f09");
    let outcome_a = sell(&mut sim, &mut ids, session_a, command_a, 2000);
    assert_eq!(
        command_result(&outcome_a).payload.status,
        CommandStatus::Accepted
    );
    let handles_a = run_batches(db.pool.clone(), world_id(), vec![batch_from(outcome_a)]).await;
    assert!(!handles_a.halted.load(Ordering::Relaxed));

    let command_b = id("01a0b1c2-7c06-7c06-8c06-1a2b3c4d5f0a");
    let outcome_b = sell(&mut sim, &mut ids, session_b, command_b, 2000);
    assert_eq!(
        command_result(&outcome_b).payload.status,
        CommandStatus::Accepted
    );
    let handles_b = run_batches(db.pool.clone(), world_id(), vec![batch_from(outcome_b)]).await;
    assert!(
        !handles_b.halted.load(Ordering::Relaxed),
        "다른 actor 의 별개 명령이 멈추면 안 된다"
    );
    assert_eq!(handles_b.persisted_total.load(Ordering::Relaxed), 1);

    let processed = row_count(&db.pool, "processed_commands", world_id()).await;
    assert_eq!(processed, 2, "두 actor 의 명령이 각각 장부에 남아야 한다");
    let wallets = row_count(&db.pool, "wallets", world_id()).await;
    assert_eq!(wallets, 2, "두 actor 모두 지갑 행이 생겨야 한다");

    db.drop().await;
}

/// SC-28(AC-5(f)) — sim 기억을 우회해 중복 판매가 DB 에 직접 도달하면 **PK 위반 →
/// 정지**하고 지갑은 불변이다. p1-02 `dup_reaching_db_halts`와 같은 모양 — 배치를
/// 손으로 짜서 `processed_commands` 의 마지막 방어선(PK)만 겨냥한다.
#[tokio::test]
async fn sale_dup_reaching_db_halts() {
    let name = "sale_dup_reaching_db_halts";
    let Some(db) = starfall_testdb::TestDb::create(name).await else {
        return;
    };
    seed_world(&db.pool, world_id()).await;

    let actor = id("01a0b1c2-7a07-7a07-8a07-1a2b3c4d5f0b");
    let mineral_id = DataId::parse(MINERAL_ID).unwrap();
    seed_inventory_row(&db.pool, world_id(), actor, 4000).await;

    let mut sim = Simulation::new(world(), 100);
    sim.seed_inventory(actor, mineral_id.clone(), 4000);
    let mut ids = SeqIds(0);
    let session_id = id("01a0b1c2-7b08-7b08-8b08-1a2b3c4d5f0c");
    open_session(&mut sim, &mut ids, session_id, actor);

    let command_id = id("01a0b1c2-7c08-7c08-8c08-1a2b3c4d5f0d");
    let first_outcome = sell(&mut sim, &mut ids, session_id, command_id, 2000);
    assert_eq!(
        command_result(&first_outcome).payload.status,
        CommandStatus::Accepted
    );
    let first_handles =
        run_batches(db.pool.clone(), world_id(), vec![batch_from(first_outcome)]).await;
    assert!(!first_handles.halted.load(Ordering::Relaxed));

    let wallet_after_first: i64 =
        sqlx::query_scalar("SELECT balance_cr FROM wallets WHERE world_id = $1 AND actor_id = $2")
            .bind(world_id().get())
            .bind(actor.get())
            .fetch_one(&db.pool)
            .await
            .unwrap();

    // sim 의 기억을 우회한다 — 같은 command_id 를(새 event_id·다른 tick) 영속화에
    // 직접 밀어넣는다. tick 이 더 크므로 배치 멱등(tick 비교)으로는 안 걸린다.
    let second_event = PendingEvent {
        event_id: id("01a0b1c2-7d01-7d01-8d01-1a2b3c4d5f0e"),
        world_id: world_id(),
        tick: Tick::new(200).unwrap(),
        sequence: Sequence::new(0).unwrap(),
        occurred_at: GameTime::parse("3800-01-01T20:00:03Z").unwrap(),
        correlation_id: id("01a0b1c2-7e01-7e01-8e01-1a2b3c4d5f0f"),
        causation_id: Some(command_id), // 중복.
        actor_id: actor,
        body: DomainEventBody::MineralSold(MineralSoldPayload {
            ship_id: id("01a0b1c2-7f01-7f01-8f01-1a2b3c4d5f10"),
            session_id,
            star_system_id: DataId::parse("cradle").unwrap(),
            market_id: DataId::parse(MARKET_ID).unwrap(),
            mineral_id: mineral_id.clone(),
            pricing_version: RuleVersion::parse("cradle-exchange@1").unwrap(),
            quantity_kg: mass(1000),
            proceeds_cr: starfall_contracts::primitives::CreditAmount::new(10_000).unwrap(),
            base_price_cr_per_kg: starfall_contracts::primitives::CreditAmount::new(10).unwrap(),
            floor_price_cr_per_kg: starfall_contracts::primitives::CreditAmount::new(3).unwrap(),
            price_before_cr_per_kg: starfall_contracts::primitives::CreditAmount::new(10).unwrap(),
            price_after_cr_per_kg: starfall_contracts::primitives::CreditAmount::new(10).unwrap(),
            saturation_before_kg: mass(2000),
            saturation_after_kg: mass(3000),
            quantity_before_kg: mass(2000),
            quantity_after_kg: mass(1000),
            wallet_before_cr: starfall_contracts::primitives::CreditAmount::new(wallet_after_first)
                .unwrap(),
            wallet_after_cr: starfall_contracts::primitives::CreditAmount::new(
                wallet_after_first + 10_000,
            )
            .unwrap(),
        }),
    };
    let second = PersistBatch {
        tick: 200,
        events: vec![second_event],
        state_writes: vec![
            StateWrite::Inventory {
                actor_id: actor,
                mineral_id: mineral_id.clone(),
                expected: Some(2000),
                new: 1000,
            },
            StateWrite::Wallet {
                actor_id: actor,
                expected: Some(wallet_after_first),
                new: wallet_after_first + 10_000,
            },
        ],
    };

    let handles = run_batches(db.pool.clone(), world_id(), vec![second]).await;
    assert!(
        handles.halted.load(Ordering::Relaxed),
        "processed_commands PK 위반은 월드를 멈춰야 한다(SC-28)"
    );
    assert!(handles.failed_total.load(Ordering::Relaxed) >= 1);
    assert_eq!(handles.persisted_total.load(Ordering::Relaxed), 0);

    let events: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM domain_events WHERE world_id = $1 AND event_type = 'MINERAL_SOLD'",
    )
    .bind(world_id().get())
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(events, 1, "첫 배치의 이벤트만 남아야 한다(둘째는 롤백)");

    let wallet_after: i64 =
        sqlx::query_scalar("SELECT balance_cr FROM wallets WHERE world_id = $1 AND actor_id = $2")
            .bind(world_id().get())
            .bind(actor.get())
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert_eq!(
        wallet_after, wallet_after_first,
        "지갑 = 첫 판매 뒤 값(둘째는 롤백)"
    );

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

async fn row_count(pool: &sqlx::PgPool, table: &str, world: UuidV7) -> i64 {
    let sql = format!("SELECT count(*) FROM {table} WHERE world_id = $1");
    sqlx::query_scalar(&sql)
        .bind(world.get())
        .fetch_one(pool)
        .await
        .unwrap()
}

//! S4 DB 통합 테스트 — 판매 1건의 실제 영속화(ADR-0016, T0 `02_interface.md`).
//!
//! `economic_state.rs`/`restart_economic_state.rs`(채굴)와 같은 전략이지만, 배치를
//! 손으로 짜지 않고 **실제 `starfall_sim::Simulation`을 통해** `SellMineralCommand`를
//! 판정시켜 나온 `TickOutcome`을 그대로 `PersistBatch`로 옮긴다 — S3(server3)가 이미
//! `StateWrite::Wallet`/`StateWrite::MarketState`·`DomainEventBody::MineralSold`를
//! 합의된 모양으로 완성해 뒀으므로, 영속화 쪽(`apply_state_write`/`commit()`/
//! `contract_payload`)만 이 파일이 검증한다. 인벤토리 전제(판매 전 보유량)는 채굴을
//! 거치지 않고 `inventory_items`에 SQL로 직접 심는다(이 파일의 관심사는 판매뿐이다 —
//! 채굴 경로는 `economic_state.rs`가 이미 잰다) + `Simulation::seed_inventory`로 sim
//! 쪽 기억도 맞춰 둔다.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::too_many_arguments
)]

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use starfall_contracts::messages::CommandStatus;
use starfall_contracts::primitives::{
    ConstSchemaVersion, DataId, GameCalendar, GameTime, MassKg, RuleVersion, ServerVersion, UuidV7,
};
use starfall_contracts::{
    CommandResultMessage, SellMineralCommand, SellMineralPayload, SellMineralType,
};
use starfall_persistence::PersistHandles;
use starfall_sim::world::{BoundaryConstants, ShipClassConstants, Vec3};
use starfall_sim::{
    DepositConstants, IdSource, InboundCommand, ListingConstants, MarketConstants,
    MiningRuleConstants, PersistBatch, PriceRule, ServerMessage, ShipClassData, Simulation,
    Submission, TickOutcome, WorldConstants,
};
use tokio::sync::{mpsc, watch};

fn id(text: &str) -> UuidV7 {
    UuidV7::parse(text).unwrap_or_else(|| panic!("정규 UuidV7 이어야 한다: {text}"))
}

/// 이 파일 전용 월드 — 다른 `trading_*.rs`/`economic_state.rs`/`restart_economic_state.rs`
/// 와 다른 id.
fn world_id() -> UuidV7 {
    id("01a0e7f7-1b2c-7d3e-8f4a-5b6c7d8e9f01")
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

/// 판매 전제 — 이 actor가 이미 그 광물을 `qty`kg 갖고 있다(채굴을 거치지 않고 직접
/// 심는다, 이 파일의 관심사는 판매뿐이다).
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

/// `base_price_cr_per_kg=10, band_kg=2000, floor_pct=30, drop_pct=10` — S3b 의 Ferrosite
/// 상수와 같다(`simulation.rs::trading_tests::price_rule` 와 같은 값, 독립 선택).
fn price_rule() -> PriceRule {
    PriceRule {
        base_price_cr_per_kg: 10,
        band_kg: 2000,
        price_floor_pct: 30,
        price_drop_pct_per_band: 10,
    }
}

/// 시장 하나(`MARKET_ID`)·광물 하나(`MINERAL_ID`) — 스폰 지점과 같은 좌표(도킹 구역
/// 안), 쿨다운 0(이 파일은 쿨다운을 재지 않는다 — 그건 server3 의 sim 단위 테스트
/// 몫). `recovery_interval_ticks=10`·`recovery_kg=1000` 은 SC-50/51 이 배수 tick 계산을
/// 손으로 맞추기 쉬운 값으로 고른 것이다(디자인 §3.6 표의 실제 수치가 아니다 — 이
/// 파일은 가격 golden 이 아니라 영속화 경로만 잰다).
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
        // `MARKET_STATE`의 상장 노출은 "드러난 광물만"(I-86) — 매장지가 있고
        // `deposit_states`에 그 매장지 id가 있어야 드러난 것으로 친다
        // (`mineral_is_revealed`). 이 매장지는 채굴 판정에 쓰이지 않는다(이 파일은
        // 판매만 잰다) — SC-50/51 이 `sim.seed_deposit_state`로 드러난 것으로
        // 만든다.
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

/// SC-37(AC-6(d), I-93) — 전량 판매 뒤에도 `inventory_items` 행이 **지워지지 않고**
/// `quantity_kg = 0` 으로 남는다(ADR-0016 §2 — 상태 표에 DELETE 경로가 없다).
#[tokio::test]
async fn full_sale_keeps_zero_inventory_row() {
    let name = "full_sale_keeps_zero_inventory_row";
    let Some(db) = starfall_testdb::TestDb::create(name).await else {
        return;
    };
    seed_world(&db.pool, world_id()).await;

    let actor = id("01a0b1c2-6a01-7a01-8a01-1a2b3c4d5e01");
    let mineral_id = DataId::parse(MINERAL_ID).unwrap();
    seed_inventory_row(&db.pool, world_id(), actor, 2000).await;

    let mut sim = Simulation::new(world(), 100);
    sim.seed_inventory(actor, mineral_id, 2000);
    let mut ids = SeqIds(0);
    let session_id = id("01a0b1c2-6b01-7b01-8b01-1a2b3c4d5e02");
    open_session(&mut sim, &mut ids, session_id, actor);

    let command_id = id("01a0b1c2-6c01-7c01-8c01-1a2b3c4d5e03");
    let outcome = sell(&mut sim, &mut ids, session_id, command_id, 2000);
    let result = command_result(&outcome);
    assert_eq!(
        result.payload.status,
        CommandStatus::Accepted,
        "전제: 전량 판매가 수락돼야 한다(reason={:?})",
        result.payload.reason_code
    );

    let handles = run_batches(db.pool.clone(), world_id(), vec![batch_from(outcome)]).await;
    assert!(!handles.halted.load(Ordering::Relaxed));
    assert_eq!(handles.persisted_total.load(Ordering::Relaxed), 1);

    let row_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM inventory_items WHERE world_id = $1 AND actor_id = $2 AND mineral_id = $3",
    )
    .bind(world_id().get())
    .bind(actor.get())
    .bind(MINERAL_ID)
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(
        row_count, 1,
        "행이 지워지면 안 된다(DELETE 경로 없음, ADR-0016 §2)"
    );

    let qty_kg: i64 = sqlx::query_scalar(
        "SELECT quantity_kg FROM inventory_items WHERE world_id = $1 AND actor_id = $2 AND mineral_id = $3",
    )
    .bind(world_id().get())
    .bind(actor.get())
    .bind(MINERAL_ID)
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(qty_kg, 0);

    // ⊘ 전제 배제 — 이 판매가 실제로 "전량" 이었다(quantity_after_kg = 0)는 것을
    // 이벤트 payload 로도 확인한다. 이게 아니면 위 단언이 우연히 통과할 수 있다
    // (예: 처음부터 0kg 행이었던 경우).
    let quantity_after: i64 = sqlx::query_scalar(
        "SELECT (payload->>'quantity_after_kg')::bigint FROM domain_events \
         WHERE world_id = $1 AND event_type = 'MINERAL_SOLD'",
    )
    .bind(world_id().get())
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(
        quantity_after, 0,
        "⊘: 전량 판매가 아니면 이 단언이 실패해야 한다"
    );

    db.drop().await;
}

/// SC-34(AC-6(b), I-93) — 서버가 떠 있는 동안 지갑 행을 SQL 로 바꾼 뒤 그 actor 가
/// 다시 판매하면 **정지**한다(비교 후 쓰기 실패 — K5, ADR-0013 §5).
#[tokio::test]
async fn wallet_cas_mismatch_halts() {
    let name = "wallet_cas_mismatch_halts";
    let Some(db) = starfall_testdb::TestDb::create(name).await else {
        return;
    };
    seed_world(&db.pool, world_id()).await;

    let actor = id("01a0b1c2-6a02-7a02-8a02-1a2b3c4d5e04");
    let mineral_id = DataId::parse(MINERAL_ID).unwrap();
    seed_inventory_row(&db.pool, world_id(), actor, 4000).await;

    let mut sim = Simulation::new(world(), 100);
    sim.seed_inventory(actor, mineral_id, 4000);
    let mut ids = SeqIds(0);
    let session_id = id("01a0b1c2-6b02-7b02-8b02-1a2b3c4d5e05");
    open_session(&mut sim, &mut ids, session_id, actor);

    // 첫 판매(2000kg) — 지갑 행이 생긴다.
    let first_command = id("01a0b1c2-6c02-7c02-8c02-1a2b3c4d5e06");
    let first_outcome = sell(&mut sim, &mut ids, session_id, first_command, 2000);
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

    let balance_before_tamper: i64 =
        sqlx::query_scalar("SELECT balance_cr FROM wallets WHERE world_id = $1 AND actor_id = $2")
            .bind(world_id().get())
            .bind(actor.get())
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert!(
        balance_before_tamper > 0,
        "전제: 첫 판매로 지갑이 생겼어야 한다"
    );

    // 변조 — 서버가 떠 있는 동안 SQL 로 지갑 값을 바꾼다(실서버에서는 다른 프로세스가
    // 저지르는 것을 흉내낸다). sim 은 이 변조를 모른다.
    let tampered_balance = balance_before_tamper + 999;
    sqlx::query("UPDATE wallets SET balance_cr = $3 WHERE world_id = $1 AND actor_id = $2")
        .bind(world_id().get())
        .bind(actor.get())
        .bind(tampered_balance)
        .execute(&db.pool)
        .await
        .unwrap();

    // 둘째 판매 — sim 의 `wallet_expected` 는 변조 전 값이라 DB(변조된 값)와 어긋난다.
    let second_command = id("01a0b1c2-6c03-7c03-8c03-1a2b3c4d5e07");
    let second_outcome = sell(&mut sim, &mut ids, session_id, second_command, 1000);
    assert_eq!(
        command_result(&second_outcome).payload.status,
        CommandStatus::Accepted,
        "sim 판정 자체는 수락한다(변조를 모른다) — 정지는 영속화 층에서 일어난다"
    );
    let second_handles = run_batches(
        db.pool.clone(),
        world_id(),
        vec![batch_from(second_outcome)],
    )
    .await;

    assert!(
        second_handles.halted.load(Ordering::Relaxed),
        "지갑 행 비교 후 쓰기 실패는 월드를 멈춰야 한다(K5)"
    );
    assert!(second_handles.failed_total.load(Ordering::Relaxed) >= 1);
    assert_eq!(
        second_handles.persisted_total.load(Ordering::Relaxed),
        0,
        "이 run 호출에서는 성공 커밋이 없어야 한다"
    );

    // 그 tick 의 MINERAL_SOLD 가 DB 에 없다(롤백) — 첫 판매 이벤트 1건만 남는다.
    let events: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM domain_events WHERE world_id = $1 AND event_type = 'MINERAL_SOLD'",
    )
    .bind(world_id().get())
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(events, 1, "둘째 판매의 이벤트는 롤백돼야 한다");

    let balance_after: i64 =
        sqlx::query_scalar("SELECT balance_cr FROM wallets WHERE world_id = $1 AND actor_id = $2")
            .bind(world_id().get())
            .bind(actor.get())
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert_eq!(
        balance_after, tampered_balance,
        "변조된 값 그대로 남아야 한다(둘째 판매가 롤백됐다)"
    );

    db.drop().await;
}

/// SC-35(AC-6(b), I-93) — SC-34 와 같은 모양, 변조 대상이 `market_states` 행(그
/// 판매의 (market, mineral) 키).
#[tokio::test]
async fn market_cas_mismatch_halts() {
    let name = "market_cas_mismatch_halts";
    let Some(db) = starfall_testdb::TestDb::create(name).await else {
        return;
    };
    seed_world(&db.pool, world_id()).await;

    let actor = id("01a0b1c2-6a04-7a04-8a04-1a2b3c4d5e08");
    let mineral_id = DataId::parse(MINERAL_ID).unwrap();
    seed_inventory_row(&db.pool, world_id(), actor, 4000).await;

    let mut sim = Simulation::new(world(), 100);
    sim.seed_inventory(actor, mineral_id, 4000);
    let mut ids = SeqIds(0);
    let session_id = id("01a0b1c2-6b04-7b04-8b04-1a2b3c4d5e09");
    open_session(&mut sim, &mut ids, session_id, actor);

    let first_command = id("01a0b1c2-6c04-7c04-8c04-1a2b3c4d5e0a");
    let first_outcome = sell(&mut sim, &mut ids, session_id, first_command, 2000);
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

    let saturation_before_tamper: i64 = sqlx::query_scalar(
        "SELECT saturation_kg FROM market_states WHERE world_id = $1 AND market_id = $2 AND mineral_id = $3",
    )
    .bind(world_id().get())
    .bind(MARKET_ID)
    .bind(MINERAL_ID)
    .fetch_one(&db.pool)
    .await
    .unwrap();

    // 변조 — `as_of_tick` 은 그대로 두고 포화만 바꾼다(CAS 비교는 두 열 모두를 본다 —
    // 하나만 바뀌어도 불일치).
    let tampered_saturation = saturation_before_tamper + 500;
    sqlx::query(
        "UPDATE market_states SET saturation_kg = $4 \
         WHERE world_id = $1 AND market_id = $2 AND mineral_id = $3",
    )
    .bind(world_id().get())
    .bind(MARKET_ID)
    .bind(MINERAL_ID)
    .bind(tampered_saturation)
    .execute(&db.pool)
    .await
    .unwrap();

    let second_command = id("01a0b1c2-6c05-7c05-8c05-1a2b3c4d5e0b");
    let second_outcome = sell(&mut sim, &mut ids, session_id, second_command, 1000);
    assert_eq!(
        command_result(&second_outcome).payload.status,
        CommandStatus::Accepted
    );
    let second_handles = run_batches(
        db.pool.clone(),
        world_id(),
        vec![batch_from(second_outcome)],
    )
    .await;

    assert!(
        second_handles.halted.load(Ordering::Relaxed),
        "시장 행 비교 후 쓰기 실패는 월드를 멈춰야 한다(K5)"
    );
    assert!(second_handles.failed_total.load(Ordering::Relaxed) >= 1);
    assert_eq!(second_handles.persisted_total.load(Ordering::Relaxed), 0);

    let events: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM domain_events WHERE world_id = $1 AND event_type = 'MINERAL_SOLD'",
    )
    .bind(world_id().get())
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(events, 1, "둘째 판매의 이벤트는 롤백돼야 한다");

    let saturation_after: i64 = sqlx::query_scalar(
        "SELECT saturation_kg FROM market_states WHERE world_id = $1 AND market_id = $2 AND mineral_id = $3",
    )
    .bind(world_id().get())
    .bind(MARKET_ID)
    .bind(MINERAL_ID)
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(
        saturation_after, tampered_saturation,
        "변조된 값 그대로 남아야 한다(둘째 판매가 롤백됐다)"
    );

    db.drop().await;
}

/// SC-36(AC-6(c)) — 재기동하면 **변조된 값으로 적재**되고 다음 판매가 그 위에서
/// 수락된다 — DB 가 정본이다(ADR-0013 §7). SC-34 와 같은 변조를 만들고, 그다음
/// "재기동"(실제 `load_economic_state()` → 새 `Simulation` 주입)으로 그 값을 적재한
/// 뒤 판매하면 더 이상 CAS 불일치가 아니라 성공한다.
#[tokio::test]
async fn restart_loads_tampered_wallet_and_sells_on_top() {
    let name = "restart_loads_tampered_wallet_and_sells_on_top";
    let Some(db) = starfall_testdb::TestDb::create(name).await else {
        return;
    };
    seed_world(&db.pool, world_id()).await;

    let actor = id("01a0b1c2-6a06-7a06-8a06-1a2b3c4d5e0c");
    let mineral_id = DataId::parse(MINERAL_ID).unwrap();
    seed_inventory_row(&db.pool, world_id(), actor, 4000).await;

    let mut sim = Simulation::new(world(), 100);
    sim.seed_inventory(actor, mineral_id.clone(), 4000);
    let mut ids = SeqIds(0);
    let session_id = id("01a0b1c2-6b06-7b06-8b06-1a2b3c4d5e0d");
    open_session(&mut sim, &mut ids, session_id, actor);

    let first_command = id("01a0b1c2-6c06-7c06-8c06-1a2b3c4d5e0e");
    let first_outcome = sell(&mut sim, &mut ids, session_id, first_command, 2000);
    assert_eq!(
        command_result(&first_outcome).payload.status,
        CommandStatus::Accepted
    );
    let first_handles =
        run_batches(db.pool.clone(), world_id(), vec![batch_from(first_outcome)]).await;
    assert!(!first_handles.halted.load(Ordering::Relaxed));

    let balance_before_tamper: i64 =
        sqlx::query_scalar("SELECT balance_cr FROM wallets WHERE world_id = $1 AND actor_id = $2")
            .bind(world_id().get())
            .bind(actor.get())
            .fetch_one(&db.pool)
            .await
            .unwrap();
    let tampered_balance = balance_before_tamper + 777;
    assert_ne!(
        tampered_balance, balance_before_tamper,
        "⊘: 변조 전·후 값이 달라야 한다"
    );
    sqlx::query("UPDATE wallets SET balance_cr = $3 WHERE world_id = $1 AND actor_id = $2")
        .bind(world_id().get())
        .bind(actor.get())
        .bind(tampered_balance)
        .execute(&db.pool)
        .await
        .unwrap();

    // ── "재기동" ──────────────────────────────────────────────────────────
    let economic = starfall_persistence::load_economic_state(&db.pool, world_id())
        .await
        .expect("경제 상태 적재 실패");
    let reloaded_wallet = economic
        .wallets
        .iter()
        .find(|row| row.actor_id == actor)
        .expect("지갑 행이 있어야 한다");
    assert_eq!(
        reloaded_wallet.balance_cr, tampered_balance,
        "적재는 DB 의 실제(변조된) 값을 그대로 읽어야 한다 — DB 가 정본"
    );

    let mut restarted_sim = Simulation::new(world(), first_outcome_tick(&db, world_id()).await + 1);
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
    let restarted_session = id("01a0b1c2-6b07-7b07-8b07-1a2b3c4d5e0f");
    open_session(
        &mut restarted_sim,
        &mut restarted_ids,
        restarted_session,
        actor,
    );

    let second_command = id("01a0b1c2-6c07-7c07-8c07-1a2b3c4d5e10");
    let second_outcome = sell(
        &mut restarted_sim,
        &mut restarted_ids,
        restarted_session,
        second_command,
        1000,
    );
    let result = command_result(&second_outcome);
    assert_eq!(
        result.payload.status,
        CommandStatus::Accepted,
        "재기동 뒤 판매는 변조값 위에서 수락돼야 한다(reason={:?})",
        result.payload.reason_code
    );

    let second_handles = run_batches(
        db.pool.clone(),
        world_id(),
        vec![batch_from(second_outcome)],
    )
    .await;
    assert!(
        !second_handles.halted.load(Ordering::Relaxed),
        "재기동 뒤 적재값으로 CAS 가 성공해야 한다 — 변조값이 새 기대값이다"
    );
    assert_eq!(second_handles.persisted_total.load(Ordering::Relaxed), 1);

    let final_balance: i64 =
        sqlx::query_scalar("SELECT balance_cr FROM wallets WHERE world_id = $1 AND actor_id = $2")
            .bind(world_id().get())
            .bind(actor.get())
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert!(
        final_balance > tampered_balance,
        "변조값 위에 둘째 판매 대금이 더해져야 한다(변조값={tampered_balance}, 최종={final_balance})"
    );

    db.drop().await;
}

/// `restart_loads_tampered_wallet_and_sells_on_top` 전용 — 첫 배치가 실제로 커밋된
/// tick 을 DB 에서 직접 읽는다(재기동 뒤 `Simulation::new` 의 `start_tick` 을 그
/// 다음 tick 으로 맞추기 위해서 — `first_outcome.tick` 은 이미 소비됐다).
async fn first_outcome_tick(db: &starfall_testdb::TestDb, world: UuidV7) -> u64 {
    let tick: i64 = sqlx::query_scalar("SELECT last_tick FROM worlds WHERE world_id = $1")
        .bind(world.get())
        .fetch_one(&db.pool)
        .await
        .unwrap();
    u64::try_from(tick).unwrap_or(0)
}

/// SC-50(AC-11(c), I-87) — 회복 배수 tick 에 `market_states`·`domain_events` 행 수가
/// 변하지 않는다(회복은 DB 쓰기도 이벤트도 만들지 않는다 — sim 메모리에서만 흐른다).
/// ⊘: 그 tick 에 `MARKET_STATE` 가 실제로 나갔고(세션이 열려 있으므로) 포화가 실제로
/// 줄었다는 것을 같은 테스트에서 단언한다 — 아니면 "행 수가 안 바뀜" 이 회복이 전혀
/// 없었다는 뜻일 수도 있어 증거가 안 된다.
#[tokio::test]
async fn recovery_tick_writes_no_market_row_or_event() {
    let name = "recovery_tick_writes_no_market_row_or_event";
    let Some(db) = starfall_testdb::TestDb::create(name).await else {
        return;
    };
    seed_world(&db.pool, world_id()).await;

    let actor = id("01a0b1c2-6a08-7a08-8a08-1a2b3c4d5e11");
    let mineral_id = DataId::parse(MINERAL_ID).unwrap();
    seed_inventory_row(&db.pool, world_id(), actor, 2000).await;

    let mut sim = Simulation::new(world(), 100);
    sim.seed_inventory(actor, mineral_id, 2000);
    sim.seed_deposit_state(DataId::parse("test-deposit").unwrap(), 1, 0, 0); // 드러남(I-86) — MARKET_STATE 노출용.
    let mut ids = SeqIds(0);
    let session_id = id("01a0b1c2-6b08-7b08-8b08-1a2b3c4d5e12");
    open_session(&mut sim, &mut ids, session_id, actor);

    // tick 100 에 2000kg 판매 — band_kg=2000 이라 saturation_after = 2000(한 구간 가득).
    let command_id = id("01a0b1c2-6c08-7c08-8c08-1a2b3c4d5e13");
    let sale_outcome = sell(&mut sim, &mut ids, session_id, command_id, 2000);
    assert_eq!(
        command_result(&sale_outcome).payload.status,
        CommandStatus::Accepted
    );
    let sale_handles =
        run_batches(db.pool.clone(), world_id(), vec![batch_from(sale_outcome)]).await;
    assert!(!sale_handles.halted.load(Ordering::Relaxed));

    let market_rows_before: i64 =
        sqlx::query_scalar("SELECT count(*) FROM market_states WHERE world_id = $1")
            .bind(world_id().get())
            .fetch_one(&db.pool)
            .await
            .unwrap();
    let events_before: i64 =
        sqlx::query_scalar("SELECT count(*) FROM domain_events WHERE world_id = $1")
            .bind(world_id().get())
            .fetch_one(&db.pool)
            .await
            .unwrap();
    let saturation_before: i64 = sqlx::query_scalar(
        "SELECT saturation_kg FROM market_states WHERE world_id = $1 AND market_id = $2 AND mineral_id = $3",
    )
    .bind(world_id().get())
    .bind(MARKET_ID)
    .bind(MINERAL_ID)
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(saturation_before, 2000);

    // 명령 없이 tick 110 까지 진행(세션은 열려 있다). `recovery_interval_ticks=10` 이므로
    // tick 110 이 "회복이 실제로 바뀌는" 첫 배수 tick 이다(⌊110/10⌋=11 > ⌊as_of/10⌋=10
    // — `simulation.rs::check_market_recovery_broadcasts`의 판정 그대로). `step()`은
    // 호출마다 정확히 tick 하나를 소비하므로(판매 자체가 이미 tick 하나를 썼다) 고정
    // 범위 대신 `next_tick()`을 직접 보고 멈춘다 — 판매가 어느 tick을 먹었는지 손으로
    // 다시 세지 않게.
    let mut recovery_outcome = sim.step(vec![], &mut ids);
    while recovery_outcome.tick < 110 {
        recovery_outcome = sim.step(vec![], &mut ids);
    }
    assert_eq!(recovery_outcome.tick, 110);
    assert!(
        recovery_outcome.state_writes.is_empty(),
        "회복 tick 은 state_writes 를 만들지 않는다(sim 메모리에서만 흐른다)"
    );
    assert!(
        recovery_outcome.events.is_empty(),
        "회복 tick 은 이벤트를 만들지 않는다 — Level 0 조차 아니다"
    );

    // ⊘ — 이 tick 에 실제로 MARKET_STATE 가 나갔고 포화가 줄었다.
    let market_state_message = recovery_outcome
        .outbound
        .iter()
        .find_map(|out| match &out.message {
            ServerMessage::MarketState(message) => Some(message.as_ref()),
            _ => None,
        })
        .expect("⊘: 회복 배수 tick 에 MARKET_STATE 가 나가야 한다(세션이 열려 있다)");
    let listing = market_state_message
        .payload
        .listings
        .iter()
        .find(|listing| listing.mineral_id == DataId::parse(MINERAL_ID).unwrap())
        .expect("상장이 있어야 한다");
    assert!(
        i64::from(listing.saturation_kg.get()) < saturation_before,
        "⊘: 회복으로 포화가 실제로 줄어야 한다(이전={saturation_before}, 지금={})",
        listing.saturation_kg.get()
    );

    // 회복 tick 의 배치를 실제로 영속화해도(하트비트와 같은 모양) 행 수·값이 그대로다.
    let recovery_batch = batch_from(recovery_outcome);
    let recovery_handles = run_batches(db.pool.clone(), world_id(), vec![recovery_batch]).await;
    assert!(!recovery_handles.halted.load(Ordering::Relaxed));

    let market_rows_after: i64 =
        sqlx::query_scalar("SELECT count(*) FROM market_states WHERE world_id = $1")
            .bind(world_id().get())
            .fetch_one(&db.pool)
            .await
            .unwrap();
    let events_after: i64 =
        sqlx::query_scalar("SELECT count(*) FROM domain_events WHERE world_id = $1")
            .bind(world_id().get())
            .fetch_one(&db.pool)
            .await
            .unwrap();
    let saturation_after: i64 = sqlx::query_scalar(
        "SELECT saturation_kg FROM market_states WHERE world_id = $1 AND market_id = $2 AND mineral_id = $3",
    )
    .bind(world_id().get())
    .bind(MARKET_ID)
    .bind(MINERAL_ID)
    .fetch_one(&db.pool)
    .await
    .unwrap();

    assert_eq!(
        market_rows_after, market_rows_before,
        "행 수가 바뀌면 안 된다"
    );
    assert_eq!(events_after, events_before, "이벤트 수가 바뀌면 안 된다");
    assert_eq!(
        saturation_after, saturation_before,
        "저장된 포화는 회복을 적용하지 않은 그대로여야 한다(K3 — 영속화는 data/ 를 모른다)"
    );

    db.drop().await;
}

/// SC-51(AC-11(d), I-88, T-8) — 정상 재기동 → 포화·마지막 거래 보존, 재기동 뒤 첫
/// 배수 tick 에 식대로 회복한다.
#[tokio::test]
async fn restart_preserves_saturation_and_last_sale() {
    let name = "restart_preserves_saturation_and_last_sale";
    let Some(db) = starfall_testdb::TestDb::create(name).await else {
        return;
    };
    seed_world(&db.pool, world_id()).await;

    let actor = id("01a0b1c2-6a09-7a09-8a09-1a2b3c4d5e14");
    let mineral_id = DataId::parse(MINERAL_ID).unwrap();
    seed_inventory_row(&db.pool, world_id(), actor, 2000).await;

    let mut sim = Simulation::new(world(), 100);
    sim.seed_inventory(actor, mineral_id.clone(), 2000);
    sim.seed_deposit_state(DataId::parse("test-deposit").unwrap(), 1, 0, 0); // 드러남(I-86).
    let mut ids = SeqIds(0);
    let session_id = id("01a0b1c2-6b09-7b09-8b09-1a2b3c4d5e15");
    open_session(&mut sim, &mut ids, session_id, actor);

    let command_id = id("01a0b1c2-6c09-7c09-8c09-1a2b3c4d5e16");
    let sale_outcome = sell(&mut sim, &mut ids, session_id, command_id, 2000);
    assert_eq!(
        command_result(&sale_outcome).payload.status,
        CommandStatus::Accepted
    );
    let sale_handles =
        run_batches(db.pool.clone(), world_id(), vec![batch_from(sale_outcome)]).await;
    assert!(!sale_handles.halted.load(Ordering::Relaxed));

    // ⊘ 전제: 재기동 전 포화가 0이 아니다(회복이 끝났으면 이 검사가 무의미해진다).
    let saturation_before_restart: i64 = sqlx::query_scalar(
        "SELECT saturation_kg FROM market_states WHERE world_id = $1 AND market_id = $2 AND mineral_id = $3",
    )
    .bind(world_id().get())
    .bind(MARKET_ID)
    .bind(MINERAL_ID)
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(
        saturation_before_restart, 2000,
        "⊘ 전제: 재기동 전 포화 > 0"
    );

    // ── "재기동" ──────────────────────────────────────────────────────────
    let economic = starfall_persistence::load_economic_state(&db.pool, world_id())
        .await
        .expect("경제 상태 적재 실패");
    let reloaded_market = economic
        .market_states
        .iter()
        .find(|row| row.market_id == DataId::parse(MARKET_ID).unwrap())
        .expect("시장 행이 있어야 한다");

    // 보존 — 적재값이 재기동 전 값과 필드 단위로 같다(포화·마지막 거래 전부).
    assert_eq!(reloaded_market.saturation_kg, 2000);
    assert_eq!(
        reloaded_market.as_of_tick, 101,
        "open_session 이 이미 tick 100 을 썼으므로 판매 자체는 tick 101 이다(step() 은 \
         호출마다 정확히 tick 하나를 소비한다)"
    );
    assert_eq!(reloaded_market.last_seller_actor_id, actor);
    assert_eq!(reloaded_market.last_sale_quantity_kg, 2000);

    let mut restarted_sim = Simulation::new(world(), 102); // 판매가 tick 101 을 썼다 — 재개는 102.
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
    restarted_sim.seed_deposit_state(DataId::parse("test-deposit").unwrap(), 1, 0, 0); // 드러남(I-86) — 재기동한 sim 은 이것도 다시 적재해야 한다(이 파일은 광맥 영속화를 안 재므로 손으로 흉내).

    let mut restarted_ids = SeqIds(1000);
    let restarted_session = id("01a0b1c2-6b0a-7b0a-8b0a-1a2b3c4d5e17");
    open_session(
        &mut restarted_sim,
        &mut restarted_ids,
        restarted_session,
        actor,
    );

    // 재기동 뒤 첫 배수 tick(110) 까지 진행 — 식대로 회복돼야 한다:
    // effective(110) = max(0, 2000 - 1000*(⌊110/10⌋-⌊as_of/10⌋)) = 2000 - 1000 = 1000
    // (`next_tick()`을 보고 멈춘다 — 판매·세션 열기가 이미 쓴 tick 수를 손으로 다시
    // 세지 않게, `recovery_tick_writes_no_market_row_or_event`와 같은 이유).
    let mut recovery_outcome = restarted_sim.step(vec![], &mut restarted_ids);
    while recovery_outcome.tick < 110 {
        recovery_outcome = restarted_sim.step(vec![], &mut restarted_ids);
    }
    assert_eq!(recovery_outcome.tick, 110);
    assert!(recovery_outcome.state_writes.is_empty());
    assert!(recovery_outcome.events.is_empty());

    let market_state_message = recovery_outcome
        .outbound
        .iter()
        .find_map(|out| match &out.message {
            ServerMessage::MarketState(message) => Some(message.as_ref()),
            _ => None,
        })
        .expect("재기동 뒤 첫 배수 tick 에 MARKET_STATE 가 나가야 한다");
    let listing = market_state_message
        .payload
        .listings
        .iter()
        .find(|listing| listing.mineral_id == DataId::parse(MINERAL_ID).unwrap())
        .expect("상장이 있어야 한다");
    assert_eq!(
        i64::from(listing.saturation_kg.get()),
        1000,
        "재기동 뒤에도 회복식이 적재된 (saturation, as_of_tick) 위에서 그대로 적용돼야 한다"
    );
    let last_sale = listing
        .last_sale
        .as_ref()
        .expect("마지막 거래 게시판이 재기동을 넘어 보존돼야 한다");
    assert_eq!(last_sale.actor_id, actor);
    assert_eq!(i64::from(last_sale.quantity_kg.get()), 2000);

    db.drop().await;
}

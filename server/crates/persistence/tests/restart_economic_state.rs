//! S8 DB 통합 테스트 — 기동 시 경제 상태 적재 → sim 주입(ADR-0013 §7 2단계, qa 발견).
//!
//! S4가 놓쳤던 자리: `load_economic_state()`(server-db, persistence)가 DB에서 읽은
//! 인벤토리·매장지·처리 장부를 **재기동한 `Simulation`에 실제로 주입**하지 않으면,
//! 재기동 뒤 sim이 지속 기억을 전혀 모른 채 시작해 (a) 옛 `command_id` 재전송이
//! sim에서는 "처음 보는 것"으로 통과해 DB PK(23505)에서야 걸려 월드가 정지하고
//! (b) 첫 채굴이 DB의 실제 값과 어긋난 `expected`로 CAS(K3) 불일치를 내 정지한다.
//!
//! 이 파일은 **재기동 자체를 흉내 낸다**: 실제 `starfall_persistence::run()`으로
//! 한 번 커밋한 뒤, `load_economic_state()`로 다시 읽어 **새** `Simulation`에
//! `main.rs`가 하는 그대로 주입하고, 그 새 sim으로 (a)(b)(c)를 확인한다.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use starfall_contracts::events::MineralMinedPayload;
use starfall_contracts::messages::RejectReasonCode;
use starfall_contracts::primitives::{
    ConstSchemaVersion, DataId, GameCalendar, GameTime, MassKg, Sequence, ServerVersion, Tick,
    UuidV7,
};
use starfall_contracts::{CommandStatus, MineResourceCommand, MineResourcePayload};
use starfall_persistence::PersistHandles;
use starfall_sim::world::{BoundaryConstants, ShipClassConstants, Vec3};
use starfall_sim::{
    DepositConstants, DomainEventBody, IdSource, InboundCommand, MineralConstants,
    MiningRuleConstants, PendingEvent, PersistBatch, ServerMessage, ShipClassData, Simulation,
    StateWrite, Submission, WorldConstants,
};
use tokio::sync::{mpsc, watch};

fn id(text: &str) -> UuidV7 {
    UuidV7::parse(text).unwrap_or_else(|| panic!("정규 UuidV7 이어야 한다: {text}"))
}

/// 이 파일 전용 월드 — `economic_state.rs`의 것과 다른 id(같은 testdb라도 파일이
/// 다르면 각자 격리 DB를 새로 만들지만, 감사 관점에서 값을 겹치지 않게 둔다).
fn world_id() -> UuidV7 {
    id("01a0e7f5-9a8b-7c2d-8e3f-405162738495")
}

fn mass(value: i32) -> MassKg {
    MassKg::new(value).unwrap()
}

const MINERAL_ID: &str = "starfall-glass";
const DEPOSIT_ID: &str = "far-reach";
const SPAWN_POINT_M: [f64; 3] = [1100.0, -2600.0, -8200.0];

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

/// "서버가 예전에 한 번 커밋했던" 채굴 1건 — `first_mine_batch`(economic_state.rs)와
/// 같은 모양이지만 이 파일의 월드·상수를 쓴다. 매장지 초기 매장량 500kg, yield 25kg
/// — 커밋 뒤 잔량 475kg.
fn first_mine_batch(tick: u64, command_id: UuidV7, actor_id: UuidV7) -> PersistBatch {
    let event = PendingEvent {
        event_id: id("01a0b1c2-a0f1-7e01-8f11-505162738495"),
        world_id: world_id(),
        tick: Tick::new(tick).unwrap(),
        sequence: Sequence::new(0).unwrap(),
        occurred_at: GameTime::parse("3800-01-01T20:00:00Z").unwrap(),
        correlation_id: id("01a0b1c2-4af2-7c01-8d02-1e033f044a05"),
        causation_id: Some(command_id),
        actor_id,
        body: DomainEventBody::MineralMined(MineralMinedPayload {
            ship_id: id("01a0b1c2-8bf1-7a11-8b22-9c33d44e55f6"),
            session_id: id("01a0b1c2-4af1-7b22-9c33-0d44e55f6a77"),
            star_system_id: DataId::parse("cradle").unwrap(),
            deposit_id: DataId::parse(DEPOSIT_ID).unwrap(),
            mineral_id: DataId::parse(MINERAL_ID).unwrap(),
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
                mineral_id: DataId::parse(MINERAL_ID).unwrap(),
                expected: None,
                new: 25,
            },
            StateWrite::Deposit {
                deposit_id: DataId::parse(DEPOSIT_ID).unwrap(),
                expected: None,
                new: (475, tick),
                first_extracted_tick: tick,
            },
        ],
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

/// `main.rs`가 기동 시 만드는 것과 같은 모양의 `WorldConstants` — 매장지가 스폰
/// 지점과 같은 좌표라 사거리 판정 없이 채굴 판정(쿨다운·CAS)에만 집중한다.
fn world() -> WorldConstants {
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
        spawn_clearance_m: 150.0,
        spawn_max_probe_attempts: 12,
        spawn_radial_offset_step_m: 100.0,
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
        minerals: std::collections::BTreeMap::from([(
            DataId::parse(MINERAL_ID).unwrap(),
            MineralConstants {
                yield_per_extraction_kg: 25,
                regen_kg: 0, // 회복을 없애 두 번째 채굴의 CAS `expected`가 그대로 475임을 단순하게 만든다.
                regen_interval_ticks: 0,
            },
        )]),
        deposits: std::collections::BTreeMap::from([(
            DataId::parse(DEPOSIT_ID).unwrap(),
            DepositConstants {
                mineral_id: DataId::parse(MINERAL_ID).unwrap(),
                position_m: Vec3::new(SPAWN_POINT_M[0], SPAWN_POINT_M[1], SPAWN_POINT_M[2]),
                radius_m: 10.0,
                initial_reserve_kg: 500,
            },
        )]),
        mining_rules: MiningRuleConstants {
            mining_range_from_surface_m: 150.0,
            max_ship_speed_mps: 10.0,
            cooldown_ticks: 0, // 재기동 뒤 쿨다운은 지속되지 않는다(설계 — 아래 알려진 한계).
        },
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

fn mine_resource(command_id: UuidV7) -> InboundCommand {
    InboundCommand::MineResource(MineResourceCommand {
        command_id,
        command_type: starfall_contracts::commands::MineResourceType::MineResource,
        schema_version: ConstSchemaVersion,
        client_sent_at: None,
        payload: MineResourcePayload {
            deposit_id: DataId::parse(DEPOSIT_ID).unwrap(),
        },
    })
}

fn command_result_reason(
    outcome: &starfall_sim::TickOutcome,
) -> (CommandStatus, Option<RejectReasonCode>) {
    outcome
        .outbound
        .iter()
        .find_map(|out| match &out.message {
            ServerMessage::CommandResult(message) => {
                Some((message.payload.status, message.payload.reason_code))
            }
            _ => None,
        })
        .expect("COMMAND_RESULT 가 있어야 한다")
}

/// S8 — 재기동을 흉내 낸다: 실제 커밋 → `load_economic_state()` → **새** `Simulation`에
/// 주입 → (a) 옛 `command_id` 재전송은 정지가 아니라 `DUPLICATE_COMMAND_ID`,
/// (b) 적재값이 DB에 실제로 있던 값과 일치, (c) 재기동 뒤 첫(새) 채굴이 CAS 성공.
#[tokio::test]
async fn restart_reloads_economic_state_so_cas_succeeds_and_duplicates_are_rejected() {
    let name = "restart_reloads_economic_state_so_cas_succeeds_and_duplicates_are_rejected";
    let Some(db) = starfall_testdb::TestDb::create(name).await else {
        return;
    };
    seed_world(&db.pool, world_id()).await;

    let actor_id = id("01a0b1c2-2cf1-7a45-8b67-89abcdef0123");
    let old_command_id = id("01a0b1c2-9cf1-7d11-8e21-3f3140516171");

    // "이전 실행"이 커밋해 둔 상태.
    let first_batch = first_mine_batch(24000, old_command_id, actor_id);
    let first_run = run_batches(db.pool.clone(), world_id(), vec![first_batch]).await;
    assert!(
        !first_run.halted.load(Ordering::Relaxed),
        "전제: 첫 커밋 자체가 정지하면 안 된다"
    );

    // ── "재기동" ──────────────────────────────────────────────────────────
    let economic = starfall_persistence::load_economic_state(&db.pool, world_id())
        .await
        .expect("경제 상태 적재 실패");

    // (b) AC-5(c)/SC-26 — 적재값 == DB값(필드 단위로).
    assert_eq!(economic.inventory.len(), 1, "인벤토리 행 1개여야 한다");
    assert_eq!(economic.inventory[0].actor_id, actor_id);
    assert_eq!(
        economic.inventory[0].mineral_id,
        DataId::parse(MINERAL_ID).unwrap()
    );
    assert_eq!(economic.inventory[0].quantity_kg, 25);
    assert_eq!(economic.deposits.len(), 1, "매장지 행 1개여야 한다");
    assert_eq!(
        economic.deposits[0].deposit_id,
        DataId::parse(DEPOSIT_ID).unwrap()
    );
    assert_eq!(economic.deposits[0].remaining_kg, 475);
    assert_eq!(economic.deposits[0].as_of_tick, 24000);
    assert_eq!(economic.deposits[0].first_extracted_tick, 24000);
    assert!(
        economic.processed_command_ids.contains(&old_command_id),
        "처리 장부에 옛 command_id 가 있어야 한다"
    );

    // main.rs 가 하는 그대로 새 Simulation 에 주입한다.
    let mut sim = Simulation::new(world(), 24001);
    for row in economic.inventory {
        sim.seed_inventory(row.actor_id, row.mineral_id, row.quantity_kg);
    }
    for row in economic.deposits {
        sim.seed_deposit_state(
            row.deposit_id,
            row.remaining_kg,
            row.as_of_tick,
            row.first_extracted_tick,
        );
    }
    sim.seed_processed_command_ids(economic.processed_command_ids);

    let mut ids = SeqIds(0);
    let session_id = id("01a0b1c2-5ef1-7b33-9c44-d55e66f77a88");
    sim.step(
        vec![Submission::OpenSession {
            seq: 0,
            session_id,
            actor_id,
        }],
        &mut ids,
    );

    // (a) AC-4(c)/SC-78 — 옛 command_id 재전송 → DUPLICATE_COMMAND_ID, 정지가 아니다.
    let resend_outcome = sim.step(
        vec![Submission::Command {
            seq: 1,
            session_id,
            command: mine_resource(old_command_id),
        }],
        &mut ids,
    );
    let (resend_status, resend_reason) = command_result_reason(&resend_outcome);
    assert_eq!(resend_status, CommandStatus::Rejected);
    assert_eq!(
        resend_reason,
        Some(RejectReasonCode::DuplicateCommandId),
        "재기동 전에 sim 이 이 command_id 를 본 적이 없어도, 적재된 지속 기억(K1)이 \
         막아야 한다 — 안 그러면 DB PK(23505)까지 가서야 정지한다(qa 가 찾은 결함)"
    );

    // (c) AC-5(c)/SC-26 — 재기동 뒤 첫 채굴(새 command_id)이 CAS 성공해야 한다: sim이
    // 적재한 잔량(475)을 `expected`로 써서 DB의 실제 값(475)과 맞아떨어진다.
    let new_command_id = id("01a0b1c2-9cf2-7d12-8e22-3f3140516172");
    let mine_outcome = sim.step(
        vec![Submission::Command {
            seq: 2,
            session_id,
            command: mine_resource(new_command_id),
        }],
        &mut ids,
    );
    let (mine_status, mine_reason) = command_result_reason(&mine_outcome);
    assert_eq!(
        mine_status,
        CommandStatus::Accepted,
        "재기동 뒤 첫 채굴은 수락돼야 한다(sim 판정 층, reason={mine_reason:?})"
    );

    let second_batch = PersistBatch {
        tick: mine_outcome.tick,
        events: mine_outcome.events,
        state_writes: mine_outcome.state_writes,
    };
    let second_run = run_batches(db.pool.clone(), world_id(), vec![second_batch]).await;
    assert!(
        !second_run.halted.load(Ordering::Relaxed),
        "재기동 뒤 첫 채굴의 CAS 가 실패해 정지했다 — 적재값이 DB와 어긋났다는 뜻이다\
         (qa 가 찾은 바로 그 결함)"
    );
    assert_eq!(
        second_run.persisted_total.load(Ordering::Relaxed),
        1,
        "재기동 뒤 첫 채굴 배치가 실제로 커밋돼야 한다"
    );

    db.drop().await;
}

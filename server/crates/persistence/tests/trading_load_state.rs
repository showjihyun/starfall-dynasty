//! S4 DB 통합 테스트 — 지갑·시장 상태 기동 적재(ADR-0016 §2·§4, T0 합의).
//!
//! 이 파일은 `load_economic_state` 의 **지갑·시장 적재**만 다룬다. `StateWrite::Wallet`/
//! `StateWrite::Market`(sim 쪽, T0 합의 대기)이 아직 없어 `starfall_persistence::run()`을
//! 거쳐 판매 배치로 행을 만들 수 없다 — 그래서 여기서는 0004 가 만드는 테이블에 **직접
//! SQL 로** 행을 심고, `load_economic_state`가 그 값을 필드 단위로 그대로 돌려주는지만
//! 잰다(ADR-0013 §7 2단계가 요구하는 "DB → sim 초기값" 경로의 절반 — 나머지 절반
//! `Simulation::seed_wallet`/`seed_market_state` 호출은 S3/S2 가 나온 뒤 `restart_economic_state.rs`
//! 에 합류한다).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use starfall_contracts::primitives::{DataId, UuidV7};

fn id(text: &str) -> UuidV7 {
    UuidV7::parse(text).unwrap_or_else(|| panic!("정규 UUIDv7 이어야 한다: {text}"))
}

/// 0001 스파이크 월드·`economic_state.rs`의 월드와 다른 id(PK 충돌 방지).
fn world_id() -> UuidV7 {
    id("01a0e7f3-7869-7105-9103-71ad3b301e04")
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

/// 빈 월드(아직 아무도 판매하지 않음) — 지갑·시장 목록이 빈 채로 성공한다(ADR-0016
/// §2·§4의 "행 없음 = 0" 규약과 짝인 적재 쪽 단언).
#[tokio::test]
async fn load_economic_state_on_a_world_with_no_sales_returns_empty_wallets_and_markets() {
    let name = "load_economic_state_on_a_world_with_no_sales_returns_empty_wallets_and_markets";
    let Some(db) = starfall_testdb::TestDb::create(name).await else {
        return;
    };
    seed_world(&db.pool, world_id()).await;

    let economic = starfall_persistence::load_economic_state(&db.pool, world_id())
        .await
        .expect("빈 월드에서도 성공해야 한다");

    assert!(economic.wallets.is_empty());
    assert!(economic.market_states.is_empty());

    db.drop().await;
}

/// 지갑 둘·시장 상태 둘(시장 둘, 같은 시장의 광물 둘 포함)을 **직접 SQL로** 심은 뒤
/// `load_economic_state`가 (a) 행 수 (b) 필드 값 둘 다를 SQL 원본과 맞게 돌려주는지
/// 잰다. actor·시장·광물을 섞어 "행 수만 맞고 내용이 바뀌는" 결함을 잡는다
/// (`load_economic_state_matches_the_database`와 같은 관례, S8).
#[tokio::test]
async fn load_economic_state_matches_wallets_and_market_states_in_the_database() {
    let name = "load_economic_state_matches_wallets_and_market_states_in_the_database";
    let Some(db) = starfall_testdb::TestDb::create(name).await else {
        return;
    };
    seed_world(&db.pool, world_id()).await;

    let actor_a = id("01a0b1c2-3c01-7a45-8b67-89abcdef1123");
    let actor_b = id("01a0b1c2-3c02-7a46-8b68-89abcdef1124");

    sqlx::query(
        "INSERT INTO wallets (world_id, actor_id, balance_cr) VALUES ($1, $2, $3), ($1, $4, $5)",
    )
    .bind(world_id().get())
    .bind(actor_a.get())
    .bind(20_000_i64)
    .bind(actor_b.get())
    .bind(98_000_i64)
    .execute(&db.pool)
    .await
    .expect("wallets 시드 실패");

    sqlx::query(
        "INSERT INTO market_states
             (world_id, market_id, mineral_id, saturation_kg, as_of_tick,
              last_seller_actor_id, last_sale_quantity_kg)
         VALUES
             ($1, 'cradle-exchange', 'ferrosite', 2000, 24060, $2, 2000),
             ($1, 'cradle-exchange', 'glacine', 10000, 24120, $3, 10000)",
    )
    .bind(world_id().get())
    .bind(actor_a.get())
    .bind(actor_b.get())
    .execute(&db.pool)
    .await
    .expect("market_states 시드 실패");

    let economic = starfall_persistence::load_economic_state(&db.pool, world_id())
        .await
        .expect("경제 상태 적재 실패");

    let wallet_count: i64 = sqlx::query_scalar("SELECT count(*) FROM wallets WHERE world_id = $1")
        .bind(world_id().get())
        .fetch_one(&db.pool)
        .await
        .unwrap();
    let market_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM market_states WHERE world_id = $1")
            .bind(world_id().get())
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert_eq!(economic.wallets.len() as i64, wallet_count);
    assert_eq!(economic.market_states.len() as i64, market_count);
    assert_eq!(wallet_count, 2, "actor 둘 — 전제 확인");
    assert_eq!(market_count, 2, "같은 시장의 광물 둘 — 전제 확인");

    let wallet_a = economic
        .wallets
        .iter()
        .find(|row| row.actor_id == actor_a)
        .expect("actor_a 지갑 행이 없다");
    assert_eq!(wallet_a.balance_cr, 20_000);
    let wallet_b = economic
        .wallets
        .iter()
        .find(|row| row.actor_id == actor_b)
        .expect("actor_b 지갑 행이 없다");
    assert_eq!(wallet_b.balance_cr, 98_000);

    let market_id = DataId::parse("cradle-exchange").unwrap();
    let ferrosite = economic
        .market_states
        .iter()
        .find(|row| row.mineral_id == DataId::parse("ferrosite").unwrap())
        .expect("ferrosite 시장 행이 없다");
    assert_eq!(ferrosite.market_id, market_id);
    assert_eq!(ferrosite.saturation_kg, 2000);
    assert_eq!(ferrosite.as_of_tick, 24060);
    assert_eq!(ferrosite.last_seller_actor_id, actor_a);
    assert_eq!(ferrosite.last_sale_quantity_kg, 2000);

    let glacine = economic
        .market_states
        .iter()
        .find(|row| row.mineral_id == DataId::parse("glacine").unwrap())
        .expect("glacine 시장 행이 없다");
    assert_eq!(glacine.market_id, market_id);
    assert_eq!(glacine.saturation_kg, 10000);
    assert_eq!(glacine.as_of_tick, 24120);
    assert_eq!(glacine.last_seller_actor_id, actor_b);
    assert_eq!(glacine.last_sale_quantity_kg, 10000);

    db.drop().await;
}

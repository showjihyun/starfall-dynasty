# T0 — 경계 인터페이스 정본 (server3 + serverdb3)

구조체 모양만 모은 참조 문서. 결정 근거·ADR 검토는 `02_server_ack.md`.

## `StateWrite` (sim, `server/crates/sim/src/mining.rs`) — server 소유

```rust
pub enum StateWrite {
    Inventory {
        actor_id: UuidV7,
        mineral_id: DataId,
        expected: Option<i64>,
        new: i64,
    },
    Deposit {
        deposit_id: DataId,
        expected: Option<(i64, u64)>,
        new: (i64, u64),
        first_extracted_tick: u64,
    },
    // --- p1-03 신규 ---
    Wallet {
        actor_id: UuidV7,
        expected: Option<i64>,  // None = 행 없음 → INSERT
        new: i64,               // balance_cr
    },
    MarketState {
        market_id: DataId,
        mineral_id: DataId,
        expected: Option<(i64, u64)>,  // (saturation_kg, as_of_tick). None = 행 없음(그
                                         // 시장·광물 첫 판매) → INSERT
        new: (i64, u64),
        // `Deposit.first_extracted_tick`과 같은 자리 — CAS 비교 대상이 아니다. 이 둘은
        // "이전 값에서 유도"가 아니라 이번 판매의 actor_id·quantity_kg를 그대로 덮어쓰는
        // 값이라(마지막 거래 게시판), 비교할 "이전 값"이라는 개념이 없다(serverdb3 제안,
        // 2026-10-05 — Deposit과 달리 매번 바뀌지만 그래도 CAS 비교 불필요: 바뀐 값이
        // DB의 이전 값에 의존하지 않기 때문).
        last_seller_actor_id: UuidV7,
        last_sale_quantity_kg: i64,
    },
}
```

인벤토리 **감소**는 기존 `Inventory` variant 그대로(새 모양 없음).

## `apply_state_write` 확장 (persistence, `server/crates/persistence/src/lib.rs`) — serverdb3 소유

새 match 암 2개, 기존 `Inventory`/`Deposit` 암과 같은 구조(비교 후 쓰기, mismatch 시 같은 트랜잭션에서 실제 값 읽어 `CompareAndSetMismatch` 에 싣고 K5 정지):

```sql
-- Wallet, expected = None
INSERT INTO wallets (world_id, actor_id, balance_cr) VALUES ($1, $2, $3)
ON CONFLICT (world_id, actor_id) DO NOTHING;

-- Wallet, expected = Some(b)
UPDATE wallets SET balance_cr = $new
WHERE world_id = $1 AND actor_id = $2 AND balance_cr = $expected;

-- MarketState, expected = None
INSERT INTO market_states
  (world_id, market_id, mineral_id, saturation_kg, as_of_tick, last_seller_actor_id, last_sale_quantity_kg)
VALUES ($1, $2, $3, $4, $5, $6, $7)
ON CONFLICT (world_id, market_id, mineral_id) DO NOTHING;

-- MarketState, expected = Some((s, t))
UPDATE market_states SET saturation_kg = $new.0, as_of_tick = $new.1,
    last_seller_actor_id = $last_seller_actor_id, last_sale_quantity_kg = $last_sale_quantity_kg
WHERE world_id = $1 AND market_id = $2 AND mineral_id = $3
  AND saturation_kg = $expected.0 AND as_of_tick = $expected.1;
-- last_seller_actor_id·last_sale_quantity_kg는 WHERE에 들어가지 않는다(위 enum 주석 —
-- Deposit.first_extracted_tick과 같은 이유로 CAS 비교 대상이 아니다).
```

## `EconomicState` 확장 (persistence) — serverdb3 소유

```rust
pub struct EconomicState {
    pub inventory: Vec<InventoryRow>,              // 기존
    pub deposits: Vec<DepositRow>,                 // 기존
    pub processed_command_ids: Vec<UuidV7>,        // 기존
    // --- p1-03 신규 ---
    pub wallets: Vec<WalletRow>,
    pub market_states: Vec<MarketStateRow>,
}

pub struct WalletRow {
    pub actor_id: UuidV7,
    pub balance_cr: i64,
}

pub struct MarketStateRow {
    pub market_id: DataId,
    pub mineral_id: DataId,
    pub saturation_kg: i64,
    pub as_of_tick: u64,
    pub last_seller_actor_id: UuidV7,
    pub last_sale_quantity_kg: i64,
}
```

`load_economic_state` 가 `wallets`·`market_states` 테이블에서 각각 `WHERE world_id = $1` 로 읽어 채운다(기존 세 SELECT 와 같은 모양).

## `Simulation` 초깃값 주입 (sim) — server 소유

```rust
pub fn seed_wallet(&mut self, actor_id: UuidV7, balance_cr: i64);
pub fn seed_market_state(
    &mut self,
    market_id: DataId,
    mineral_id: DataId,
    saturation_kg: i64,
    as_of_tick: u64,
    last_seller_actor_id: UuidV7,
    last_sale_quantity_kg: i64,
);
```

`seed_wallet`은 `seed_inventory`와 같은 규약(행 없음=0 이므로 `balance_cr<=0`이면 스킵해도 안전 — 판매는 항상 `proceeds_cr>=1`이라 실제로 0 저장은 없다). **`seed_market_state`는 스킵하지 않는다** — `deposit_state`와 같은 이유: 포화가 회복으로 0까지 내려가도 행은 "마지막 거래 게시판"으로 남아 있어야 한다(ADR-0016 §4, serverdb3 지적).

호출 예(S2 가 main.rs 에 끼운다):

```rust
let economic = starfall_persistence::load_economic_state(&pool, world_id).await?;
// ... 기존 inventory/deposits/processed_command_ids 루프 ...
for row in economic.wallets {
    sim.seed_wallet(row.actor_id, row.balance_cr);
}
for row in economic.market_states {
    sim.seed_market_state(row.market_id, row.mineral_id, row.saturation_kg,
        row.as_of_tick, row.last_seller_actor_id, row.last_sale_quantity_kg);
}
```

## `DomainEventBody` 확장 (sim, `simulation.rs`) — server 소유

```rust
pub enum DomainEventBody {
    SessionOpened(SessionOpenedPayload),
    SessionClosed(starfall_contracts::events::SessionClosedPayload),
    ShipSpawned(ShipSpawnedPayload),
    ShipDespawned(ShipDespawnedPayload),
    MineralMined(MineralMinedPayload),
    // --- p1-03 신규 ---
    MineralSold(MineralSoldPayload),  // Level 0 — 역사적 사건 아님(ADR-0016 §3)
}
```

`event_type()` match 에 `Self::MineralSold(_) => registry::MINERAL_SOLD` 추가. `contract_payload()`(persistence) match 에 대응 암 추가 — S1 완료(계약 타입 `MineralSoldEvent` 생성) 후 S3에서 배선.

`commit()`(persistence, `lib.rs:976` 부근)의 `processed_commands` 분기 — 지금 `if let DomainEventBody::MineralMined(_) = &event.body`로 `MINE_RESOURCE`를 장부에 쓰는 것과 **같은 모양으로** `MineralSold`도 추가(serverdb3 제안):

```rust
if let DomainEventBody::MineralSold(_) = &event.body {
    // command_type = registry::SELL_MINERAL, 나머지는 MineralMined 분기와 동일
}
```

두 분기를 하나로 합칠지(`matches!`) 따로 둘지는 serverdb3가 구현 시 정한다 — 모양(같은 테이블·같은 PK·`command_type`만 다름)은 고정.

## 게이트웨이 상태 변경 명령 목록 (gateway, `ws.rs:678`) — server 소유(S5)

```rust
// 변경 전
if matches!(command, InboundCommand::MineResource(_))
// 변경 후
if matches!(command, InboundCommand::MineResource(_) | InboundCommand::SellMineral(_))
    && (stats.persist_halted() || stats.recording_lag() > RECORDING_BACKLOG_LIMIT)
```

## 0004 제약 이름 — ADR-0016 §5 그대로(실측 대조용, 변경 없음)

`wallets`(6): `wallets_pkey`, `wallets_world_id_fkey`, NOT NULL×3, `wallets_balance_cr_check`

`market_states`(12): `market_states_pkey`, `market_states_world_id_fkey`, NOT NULL×7, `market_states_saturation_kg_check`, `market_states_as_of_tick_check`, `market_states_last_sale_quantity_kg_check`

## 질문 답변 (serverdb3 → server3, 2026-10-05)

1. **`StateWrite::MarketState`(이름 통일 — serverdb3 제안은 `Market`이었으나 테이블명 `market_states`·메시지명 `MARKET_STATE`와 맞춰 `MarketState`로 확정)의 위치**: `mining.rs`에 그대로 둔다(옮기지 않는다). 이유: `StateWrite`는 크레이트 루트에서 `pub use mining::{..., StateWrite}`로 재노출되므로(`sim/src/lib.rs:41`) persistence 쪽 import(`use starfall_sim::{..., StateWrite}`)는 영향받지 않는다 — 옮겨도 임포트 경로는 안 바뀌지만, 옮기는 이유도 없다(파일 분리 이득 < 불필요한 diff). `mining.rs` 파일 상단 doc 주석만 "채굴 + 거래 공용 경제 상태 쓰기"로 갱신한다.
2. **같은 PersistBatch에 Inventory+Wallet+MarketState 세 개가 같이 들어가는가 — 그렇다.** 판매 한 건의 `TickOutcome.state_writes`에 세 항목을 모두 push하고, `gateway/src/runtime.rs:549,610`이 그대로 `PersistBatch.state_writes`로 옮긴다(이미 있는 배선, 변경 없음) → `commit()`이 한 트랜잭션에서 `apply_state_write`를 세 번 호출(ADR-0016 §1 "같은 커밋").
3. **CAS 비교 범위(Wallet 전부 비교, MarketState는 `(saturation_kg, as_of_tick)`만, `last_seller_actor_id`/`last_sale_quantity_kg`는 비교 없이 덮어쓰기)** — serverdb3 제안 그대로 채택. 근거는 위 enum 주석: 이 두 필드는 "이전 값에서 유도"가 아니라 이번 판매의 사실을 그대로 쓰는 값이라 비교할 이전 값 개념이 없다(`Deposit.first_extracted_tick`과 같은 성격 — 다만 매번 갱신된다는 점은 다르다). CAS를 더 엄격하게 걸어도 안전성이 늘지 않고(값이 이전 상태에 의존하지 않으므로) 불필요한 정지 경로만 늘린다.

이견 없음 — **합의 완료.** serverdb3는 0004 + `load_economic_state`/`apply_state_write` 확장을 진행, server3는 S1(계약 타입) → S3b(가격) → S3(sim 판매, `StateWrite` 생성 지점) 순으로 진행한다.

---

**상태**: **합의 완료**(server3 ↔ serverdb3, 2026-10-05). architect3에게 ADR-0016 accepted 요청 발송.

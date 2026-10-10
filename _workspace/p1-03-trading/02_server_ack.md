# T0 — server3 의 ADR-0016 검토 + 경계 합의 (초안, serverdb3 확인 대기)

- 작성: server3, 2026-10-05
- 상태: **합의 완료**(server3 ↔ serverdb3, 2026-10-05 — serverdb3의 역제안으로 `MarketState` CAS 비교 범위를 `(saturation_kg, as_of_tick)`로 좁혔다. 상세는 `02_interface.md` "질문 답변" 절). architect3에게 ADR-0016 accepted 요청 발송.
- 짝 문서: `02_interface.md`(구조체 모양 정본)

## 1. ADR-0016 검토 결과

읽었다: §1(장부 재사용)·§2(지갑)·§3(이벤트 하나)·§4(시장 상태, 게으른 회복)·§5(제약 표)·§6(거절 사유 확장)·§7(결정성).

**동의한다. 수정 요청 없음.** 근거:

- §1 "새 장부를 만들지 않는다" — `processed_commands` PK가 이미 `(world_id, command_id)` 월드 범위라 명령 종류를 가정하지 않는다(코드 확인: `server/crates/persistence/src/lib.rs:978` INSERT 문에 `command_type` 열이 이미 있다). `SELL_MINERAL` 을 같은 표에 넣는 데 스키마 변경이 필요 없다.
- §2 "행은 첫 판매에 생긴다, 지우지 않는다" — 기존 `inventory_items`/`deposit_states` 의 compare-and-set 패턴(`StateWrite::Inventory`/`Deposit`, `apply_state_write`)과 모양이 완전히 같다. `Wallet`/`MarketState` variant를 같은 함수에 같은 방식(`expected: None` → INSERT ON CONFLICT DO NOTHING, `Some` → UPDATE ... WHERE 기존값 일치)으로 추가하면 된다 — 새 패턴이 아니라 기존 패턴의 반복.
- §3 "이벤트 하나" — `DomainEventBody` enum에 `MineralSold(MineralSoldPayload)` variant 하나만 추가. `contract_payload()`(lib.rs:1150)의 match 암에 한 가지 추가.
- §4 "가격 저장 안 함" — 동의. `market_states` 행에는 `saturation_kg`·`as_of_tick`·`last_seller_actor_id`·`last_sale_quantity_kg` 만 있고 가격 열이 없다. 회복식은 sim(`starfall-sim`)에만 — persistence는 `data/`를 모르므로 이미 구조적으로 강제된다.
- §5 제약 표 — **아래 §3에서 이름을 그대로 확정**(바꾼 것 없음, 실측 대조만).
- §6 `CAPACITY_EXCEEDED` 의미 확장 — 동의. 새 거절 사유 코드를 만들지 않는다. `INSUFFICIENT_QUANTITY` 는 신규(계약 S1에서 추가).
- §7 결정성 — 동의. 새 메시지는 메시지 id 흐름(기존 `message_ids: Box<dyn IdSource>`)에서 꺼낸다 — 이미 `Simulation` 이 그 필드를 갖고 있어 별도 배선이 필요 없다.

**→ architect3 에게 요청: ADR-0016 을 accepted 로.**

## 2. 경계 합의 — 서버 쪽 결정 사항

### 2.1 배치에 실릴 상태 쓰기 모양

`server/crates/sim/src/mining.rs` 의 `StateWrite` enum에 variant 2개 추가(기존 `Inventory`·`Deposit` 와 같은 자리, 같은 소유자=server):

```rust
Wallet {
    actor_id: UuidV7,
    expected: Option<i64>,  // None = 행 없음(첫 판매)
    new: i64,                // balance_cr
},
MarketState {
    market_id: DataId,
    mineral_id: DataId,
    expected: Option<(i64, u64)>,  // (saturation_kg, as_of_tick) 만. None = 첫 판매
    new: (i64, u64),
    last_seller_actor_id: UuidV7,   // CAS 비교 대상 아님 — 매번 이번 판매 값으로 덮어쓴다
    last_sale_quantity_kg: i64,     // (Deposit.first_extracted_tick과 같은 자리)
},
```

인벤토리 감소는 **기존 `Inventory` variant를 그대로 재사용**(새 모양 없음) — `new`가 요청 kg만큼 줄어든 값이고 0이 되면 그대로 0(지우지 않는다, p1-02와 동일 — `INVENTORY_STATE`가 0kg 항목을 안 싣는 건 메시지 생성 쪽 규칙이지 저장 규칙이 아니다).

`apply_state_write`(persistence, serverdb3 소유)에 `Wallet`/`MarketState` 매치 암 추가:
- `Wallet`: `expected=None` → `INSERT INTO wallets (world_id, actor_id, balance_cr) VALUES ... ON CONFLICT (world_id, actor_id) DO NOTHING`. `Some(b)` → `UPDATE wallets SET balance_cr=$new WHERE world_id=$1 AND actor_id=$2 AND balance_cr=$expected`. `rows_affected != 1` → `CompareAndSetMismatch`(기존 패턴 그대로, 실제 DB 값도 같은 트랜잭션에서 읽어 로그에 싣는다).
- `MarketState`: `expected=None` → INSERT ON CONFLICT (world_id, market_id, mineral_id) DO NOTHING(`last_seller_actor_id`·`last_sale_quantity_kg` 도 "새 값"으로 함께 넣는다). `Some((s,t))` → `UPDATE ... SET saturation_kg=$new.0, as_of_tick=$new.1, last_seller_actor_id=$last_seller_actor_id, last_sale_quantity_kg=$last_sale_quantity_kg WHERE ... AND saturation_kg=$expected.0 AND as_of_tick=$expected.1`(WHERE 에 마지막 거래 두 열은 안 들어간다 — 이전 DB 값에서 유도되는 값이 아니라 이번 판매의 사실을 그대로 쓰는 값이라 비교할 "기대하는 이전 값"이 없다, serverdb3 제안). mismatch 시 `(saturation_kg, as_of_tick)` 만 로그.

### 2.2 기동 적재 → sim 초기값 주입 API

`persistence::EconomicState`(serverdb3 소유, `lib.rs:562`)에 필드 2개 추가:

```rust
pub wallets: Vec<WalletRow>,             // { actor_id, balance_cr }
pub market_states: Vec<MarketStateRow>,  // { market_id, mineral_id, saturation_kg, as_of_tick, last_seller_actor_id, last_sale_quantity_kg }
```

`load_economic_state`가 SELECT 두 벌을 더 돈다(기존 inventory/deposit/processed_commands 패턴 그대로 — 전부 `WHERE world_id = $1`).

`starfall_sim::Simulation`(server 소유, `simulation.rs:507` 부근 "기동 시 초깃값 주입" 절)에 `seed_inventory`/`seed_deposit_state` 와 같은 자리에 추가:

```rust
pub fn seed_wallet(&mut self, actor_id: UuidV7, balance_cr: i64);
pub fn seed_market_state(&mut self, market_id: DataId, mineral_id: DataId,
    saturation_kg: i64, as_of_tick: u64,
    last_seller_actor_id: UuidV7, last_sale_quantity_kg: i64);
```

적재 호출(`for row in economic.wallets { sim.seed_wallet(...) }` 등)은 S2(server)가 main.rs 기동 순서에 끼운다 — S4(serverdb3)는 `load_economic_state`/`seed_*` 함수만 내놓는다(**`main.rs` 를 고치지 않는다** — 01_architect_tasks.md S4 행 그대로).

기동 순서(ADR-0013 §7 확장, 스펙 §2-2): 마이그레이션 → 월드 대조 → **경제 상태 적재(인벤토리·매장지·처리장부·지갑·시장)** → 역사 적재 → tick 드라이버 생성. 다섯 종류를 한 함수(`load_economic_state`)가 한 번에 돌려주는 기존 모양 유지 — 호출부가 두 개로 늘지 않는다.

### 2.3 ADR-0016 §5 제약 표 — 이름 확정(ADR 원문 그대로, 실측 대조용)

**`wallets`(0004, 6)** — 키 `(world_id, actor_id)`:
1. `wallets_pkey`
2. `wallets_world_id_fkey`
3. NOT NULL `world_id`
4. NOT NULL `actor_id`
5. NOT NULL `balance_cr`
6. `wallets_balance_cr_check` — `balance_cr BETWEEN 0 AND 9007199254740991`

**`market_states`(0004, 12)** — 키 `(world_id, market_id, mineral_id)`:
1. `market_states_pkey`
2. `market_states_world_id_fkey`
3. NOT NULL `world_id`
4. NOT NULL `market_id`
5. NOT NULL `mineral_id`
6. NOT NULL `saturation_kg`
7. NOT NULL `as_of_tick`
8. NOT NULL `last_seller_actor_id`
9. NOT NULL `last_sale_quantity_kg`
10. `market_states_saturation_kg_check` — `BETWEEN 0 AND 2147483647`
11. `market_states_as_of_tick_check` — 범위(음수 금지, ADR-0007 §5a-1 과 같은 형식)
12. `market_states_last_sale_quantity_kg_check` — `BETWEEN 1 AND 2147483647`

합계 18개 제약. **표 밖 제약을 추가하지 않는다**(예: `last_sale_quantity_kg <= saturation_kg` 같은 파생 제약 — 필요해 보이면 구현 중 architect3 에게 먼저 요청, 01_architect_tasks.md "구현자가 architect 에게 가져올 것" 절 그대로).

기존 표 재검토(①이 바뀌는 것, ADR-0016 §5 표 그대로 — 변경 없음, 실측 확인만):
- `inventory_items_quantity_kg_check` 하한 0 → ① **예**(클라이언트가 `quantity_kg` 를 고른다). S3이 checked 뺄셈 + `INSUFFICIENT_QUANTITY` 거절을 넣는다.
- `processed_commands_pkey` → ① 예, 변경 없음(명령 종류 무관하게 같은 키 공간).

### 2.4 게이트웨이 상태 변경 명령 목록

`server/crates/gateway/src/ws.rs:678`:

```rust
// 기존
if matches!(command, InboundCommand::MineResource(_))
// S5가 바꿀 모양
if matches!(command, InboundCommand::MineResource(_) | InboundCommand::SellMineral(_))
```

`RECORDING_BACKLOG_LIMIT`(20, `gateway/src/stats.rs:47`)·판정 로직은 변경 없음 — 대상 명령 집합만 늘어난다.

## 2.5 commit() 의 처리 장부 분기 (serverdb3 제안, 합의)

`commit()`(persistence)의 `if let DomainEventBody::MineralMined(_) = &event.body { ... processed_commands INSERT ... }`(`lib.rs:976` 부근)과 **같은 모양으로** `MineralSold`도 `processed_commands`에 쓴다(`command_type = registry::SELL_MINERAL`). 두 분기를 `matches!`로 합칠지 따로 둘지는 serverdb3가 구현 시 정한다 — 테이블·PK·쓰는 시점은 고정.

## 2.6 질문 답변 (serverdb3 → server3)

1. **`StateWrite::MarketState` 위치** — `mining.rs`에 그대로 둔다. `StateWrite`가 크레이트 루트에서 `pub use mining::{..., StateWrite}`로 재노출되므로(`sim/src/lib.rs:41`) persistence 쪽 import는 영향받지 않는다. 파일 옮길 이득이 없다. `mining.rs` 상단 doc 주석만 "채굴 + 거래 공용 경제 상태 쓰기"로 갱신.
2. **같은 PersistBatch에 Inventory+Wallet+MarketState 세 StateWrite가 같이 들어가는가 — 그렇다.** 판매 한 건의 `TickOutcome.state_writes`에 세 항목을 모두 push, 기존 배선(`gateway/src/runtime.rs:549,610`)이 그대로 `PersistBatch.state_writes`로 옮긴다 → `commit()`이 한 트랜잭션에서 `apply_state_write`를 세 번 호출(ADR-0016 §1 "같은 커밋").

## 3. 알려진 한계 / 다음 단계

- **합의 완료**(2026-10-05). 이 문서는 **S1(계약 Rust 타입)이 끝나기 전**에 쓴 합의다. `DataId`·`UuidV7` 같은 primitive는 이미 있지만 `MineralSoldEvent`·`WalletStateMessage` 등 신규 타입은 S1에서 생긴다 — `DomainEventBody::MineralSold` variant와 `contract_payload` match 암 추가는 S1 완료 후 S3에서 실제로 쓴다.
- `market_states_as_of_tick_check` 의 정확한 범위식(상한을 `u32::MAX`로 할지 등)은 0004 마이그레이션 작성 시 serverdb3가 ADR-0007 §5a-1 기존 `as_of_tick` CHECK(있다면)와 맞춰 확정한다 — 여기서는 "존재한다"만 합의.
- architect3 에게 ADR-0016 accepted 요청 발송(이 턴).

# p1-03-trading — server-db(serverdb3) 구현 요약 (T0, S4)

작성: serverdb3, 2026-10-05(T0, S4 1부) · 2026-10-06(S4 완료 — server3의 S3 완료 뒤).
태스크: T0(경계 합의, server3와 공동, TaskList #44 — **완료**), S4(영속화 0004·지갑·시장, TaskList #49 — **완료**).

## T0 — 경계 합의

제안을 server3에게 SendMessage로 보냈고, server3가 `_workspace/p1-03-trading/02_server_ack.md`·`02_interface.md`에 합의를 기록했다(ADR-0016 **accepted**로 전환). 최종 합의 요지(정본은 두 문서):

- `StateWrite` 신규 변형 `Wallet`·`MarketState`(sim, server3 소유, `mining.rs`) — 기존 `Inventory`/`Deposit`과 같은 비교 후 쓰기 모양.
- 인벤토리 감소는 **새 변형 없이** 기존 `Inventory`를 재사용.
- `MarketState`의 CAS 비교 범위는 `(saturation_kg, as_of_tick)`만 — `last_seller_actor_id`·`last_sale_quantity_kg`는 비교 없이 매번 덮어쓰는 "새 값"(내 역제안, `Deposit.first_extracted_tick`과 같은 자리지만 매번 갱신된다는 점은 다름 — 이전 DB 값에 의존하지 않는 값이라 CAS 비교 대상일 필요가 없다).
- `EconomicState`에 `wallets: Vec<WalletRow>`·`market_states: Vec<MarketStateRow>` 추가, `load_economic_state`가 두 SELECT를 더 돈다.
- `Simulation::seed_wallet`/`seed_market_state`(server3가 S3에서 구현) — 시그니처 고정.
- `commit()`의 `processed_commands` 분기를 `DomainEventBody::MineralSold`에도(`MineralMined`와 같은 모양, `command_type = SELL_MINERAL`).
- 0004 제약 이름 18개(wallets 6 + market_states 12) — ADR-0016 §5 표 그대로, 변경 없음.

## S4 — 지금까지 한 것(StateWrite/DomainEventBody 의존 없는 절반)

**막힌 지점**: `apply_state_write`의 `Wallet`/`MarketState` 매치 암과 `commit()`의 `MineralSold` 분기는 `starfall_sim::StateWrite`·`DomainEventBody`에 그 변형이 있어야 컴파일된다 — S3(server3, sim 판매)가 아직 착수 전이라 **코드로 못 쓴다**(T0 critical path: S1 → S3 → S4). 그래서 S3와 무관한 절반(마이그레이션, Row 적재)을 먼저 TDD로 끝냈다.

### 1. 마이그레이션 `0004_wallets_and_markets.sql`

ADR-0016 §5 표 그대로(`wallets` 6, `market_states` 12 — 0002 파일과 같은 스타일의 주석, 0001~0003 동결 확인). `cargo test -p starfall-testdb`(스모크 테스트가 실제 PostgreSQL에 전체 마이그레이션 적용)로 적용 확인 — 에러 없음, `assert_migration_count_matches_directory` 가드 통과(디스크 파일 수 = `_sqlx_migrations` 행 수, 동적 비교라 하드코딩된 "3" 없음을 코드로 확인).

### 1b. 0004 제약 18개 — `pg_constraint` 실측 (architect3 요구사항 확인)

architect3가 "0004 의 제약은 `02_interface.md` 이름 18개와 정확히 같아야 한다"고 못박았으므로, qa의 `constraint_census.py`를 기다리지 않고 직접 실측했다. **증거 DB(`starfall`)는 건드리지 않았다** — 별도 스크래치 DB(`starfall_test_verify0004`, `starfall` 관리자 연결로 생성)에 0001~0004를 순서대로 psql로 적용한 뒤 `pg_constraint`를 조회하고 **즉시 DROP**(CLAUDE.md의 `docker compose down -v` 금지와는 다른 범위 — 컨테이너·볼륨은 그대로, 개별 스크래치 DB만 생성·삭제):

```sql
SELECT conrelid::regclass, conname, contype FROM pg_constraint
WHERE conrelid::regclass::text IN ('wallets','market_states') ORDER BY 1, 2;
```

결과 18행, 이름·개수 정확히 일치:
- `wallets`(6): `wallets_pkey`(p), `wallets_world_id_fkey`(f), `wallets_world_id_not_null`/`wallets_actor_id_not_null`/`wallets_balance_cr_not_null`(n×3), `wallets_balance_cr_check`(c)
- `market_states`(12): `market_states_pkey`(p), `market_states_world_id_fkey`(f), NOT NULL×7(`world_id`·`market_id`·`mineral_id`·`saturation_kg`·`as_of_tick`·`last_seller_actor_id`·`last_sale_quantity_kg`), `market_states_saturation_kg_check`·`market_states_as_of_tick_check`·`market_states_last_sale_quantity_kg_check`(c×3)

PostgreSQL 18(이미지 `postgres:18.6-trixie`)이 NOT NULL을 `pg_constraint`에 `contype='n'`으로 올리는 버전이라 0001/0002와 같은 카운팅 관례가 그대로 적용된다 — `qa`의 census 도구가 나중에 같은 질의로 다시 확인할 것이다.

### 2. `EconomicState` 확장 — RED → GREEN

**RED** 먼저 실측: `server/crates/persistence/tests/trading_load_state.rs`(신규) 2건을 `EconomicState.wallets`/`.market_states` 참조로 작성 → `cargo test -p starfall-persistence --test trading_load_state` 8개 컴파일 에러("unknown field `wallets`"/"`market_states`", `available fields are: inventory, deposits, processed_command_ids`) — 로그 그대로 확인.

**GREEN**: `persistence/src/lib.rs`에 `WalletRow`·`MarketStateRow` 구조체, `EconomicState` 필드 2개, `load_economic_state`에 SELECT 두 벌(기존 inventory/deposit/processed_commands와 같은 모양 — `WHERE world_id = $1`, 각 UUID/DataId 필드를 방어적으로 검증). 테스트는 `StateWrite::Wallet`/`MarketState`가 아직 없어 **직접 SQL로** `wallets`/`market_states` 행을 심어(sim 배치를 거치지 않고) 적재 결과를 필드 단위로 대조한다 — S3가 나오면 `restart_economic_state.rs`에 "실제 판매 배치 커밋 → 재기동 → 적재"로 합류할 예정(S8이 채굴에서 한 것과 같은 패턴).

```
STARFALL_DB_TEST RAN load_economic_state_on_a_world_with_no_sales_returns_empty_wallets_and_markets
STARFALL_DB_TEST RAN load_economic_state_matches_wallets_and_market_states_in_the_database
test result: ok. 2 passed; 0 failed
```

### 게이트(이 시점)

`server/`, `export PATH="$HOME/.cargo/bin:$PATH"`.

- `cargo fmt --all --check` — 통과.
- `cargo clippy --workspace --all-targets -- -D warnings` — 통과.
- `STARFALL_DB_TESTS=required cargo test --workspace --locked --no-fail-fast` — `starfall-contracts`(`contract_tests`, 8 실패)와 `starfall-game-server`(`data::tests::*`, 14 실패)만 실패. **이것은 내 변경 때문이 아니다** — `01_architect_tasks.md` "계약이 코드보다 앞서 있다" 절이 명시한 기존 베이스라인(`DEPOSIT_FIELD` v2, S1/S2 완료 전 정상 RED, `레지스트리 23→30`, `cradle.json`의 `mineral_id` 필드 등)과 정확히 같은 실패 메시지로 확인했다(S1/S2는 server3 소유, 이 작업 범위 밖). 내가 건드린 크레이트(`starfall-persistence`, `starfall-testdb`)와 나머지 전부(`starfall-sim`·`starfall-gateway`)는 **전부 통과**.

## 자기 점검(R3, 완료분만 — Wallet/MarketState CAS는 S3 이후 재점검)

| 항목 | ① 이름 실행 | ② 구현 변이 | ③ sha256 원복 |
|---|---|---|---|
| `load_economic_state_on_a_world_with_no_sales_returns_empty_wallets_and_markets` | `cargo test -p starfall-persistence --test trading_load_state` — 2건 모두 RAN 로그와 함께 단독 실행(이 파일에 이 둘뿐, 다른 SC 안 섞임) | `load_economic_state`의 `wallets` 적재 루프 바로 뒤에 `wallets.push(WalletRow{ actor_id: UuidV7::new_v7(), balance_cr: 999 })` 주입(기대값이 아니라 구현 자체를 깨뜸) → 두 테스트 모두 FAILED: 이 테스트는 `economic.wallets.is_empty()` 단언 실패 | 아래 실행 로그 |
| `load_economic_state_matches_wallets_and_market_states_in_the_database` | 위와 같은 실행(같은 바이너리, 같은 필터) | 같은 변이 — 이 테스트는 `wallet_count`(SQL 직접 센 값, 2) 와 `economic.wallets.len()`(변이로 3) 불일치로 `left: 3 / right: 2` 로 실패 | 아래 실행 로그 |

**실행 로그(실제 값, 2026-10-05)**:
```
$ sha256sum crates/persistence/src/lib.rs            # 변이 전(GREEN)
4e97b7fb7fc512ddd06cba05c30a8987235cf195884769bd0bf24c55043dffd1 *crates/persistence/src/lib.rs

# 변이 주입 후 cargo test -p starfall-persistence --test trading_load_state:
test load_economic_state_on_a_world_with_no_sales_returns_empty_wallets_and_markets ... FAILED
  assertion failed: economic.wallets.is_empty()
test load_economic_state_matches_wallets_and_market_states_in_the_database ... FAILED
  assertion `left == right` failed
    left: 3
   right: 2
test result: FAILED. 0 passed; 2 failed

$ sha256sum crates/persistence/src/lib.rs            # 원복 후
4e97b7fb7fc512ddd06cba05c30a8987235cf195884769bd0bf24c55043dffd1 *crates/persistence/src/lib.rs   # 변이 전과 동일
$ diff crates/persistence/src/lib.rs <변이 전 백업>   # 빈 diff

# 원복 후 재실행:
test result: ok. 2 passed; 0 failed
```

## S4 2부 — server3의 S3 완료 뒤 (2026-10-06)

server3가 `StateWrite::Wallet`/`StateWrite::MarketState`(`mining.rs`)·`DomainEventBody::MineralSold`(`simulation.rs`)·`handle_sell_mineral`(판정 8단계 전체, 세 `StateWrite` push)를 `02_interface.md` 합의 그대로 끝냈다(알림 수신, `cargo test -p starfall-sim` 그린). 이어서 persistence 쪽 나머지 절반을 TDD로 끝냈다.

### 1. production 코드 — `persistence/src/lib.rs`

- `apply_state_write`에 `StateWrite::Wallet`/`StateWrite::MarketState` 매치 암 추가. `Wallet`은 `Inventory`와 완전히 같은 모양(None→INSERT ON CONFLICT DO NOTHING, Some→UPDATE...WHERE balance_cr=expected). `MarketState`는 `Deposit`과 같은 모양이지만 CAS WHERE 절은 `(saturation_kg, as_of_tick)` 두 열만 — `last_seller_actor_id`·`last_sale_quantity_kg`는 INSERT/UPDATE의 SET/VALUES에는 들어가도 WHERE 비교에는 안 들어간다(T0 합의 그대로). mismatch 시 둘 다 같은 트랜잭션에서 실제 DB 값을 읽어 `CompareAndSetMismatch` 로그에 싣는다(기존 Inventory/Deposit과 같은 SC-25 패턴).
- `contract_payload`에 `DomainEventBody::MineralSold` 분기 — `MineralSoldEvent` 계약 타입으로 조립(causation_id는 `non_null_causation`으로 비-null 보장, `MineralMined`와 같은 방어).
- `commit()`의 `processed_commands` 분기에 `MineralSold`도 추가(`command_type = registry::SELL_MINERAL`, `MineralMined`와 완전히 같은 모양 — `ON CONFLICT DO NOTHING` 안 씀, PK가 마지막 방어선).
- `restart_economic_state.rs`(채굴 전용, S8)의 `world()`에 `markets: BTreeMap::new()` 추가(S3가 `WorldConstants`에 `markets` 필드를 더해서 생긴 컴파일 에러 — 이 파일은 내 소유라 직접 고쳤다, `main.rs`는 안 건드렸다).

### 2. 신규 DB 테스트 — qa3 계약의 [DB] 9개, TDD로 (실제 `Simulation`을 통해 생성한 `TickOutcome`을 그대로 `PersistBatch`로 커밋 — 배치를 손으로 짜지 않는다. 단, SC-28은 sim 기억을 **의도적으로 우회**해야 하므로 예외적으로 손으로 짠다)

**`crates/persistence/tests/trading_sale.rs`**(신규, 6건):
- `full_sale_keeps_zero_inventory_row`(SC-37) — 전량 판매 뒤 `inventory_items` 행이 지워지지 않고 `quantity_kg=0`으로 남는다. ⊘: 이벤트 `quantity_after_kg=0`도 확인(우연한 통과 배제).
- `wallet_cas_mismatch_halts`(SC-34) — 첫 판매 → SQL로 지갑 변조 → 둘째 판매(sim은 변조를 모른다) → CAS 불일치 → 정지, 둘째 이벤트 롤백, 지갑=변조값 그대로.
- `market_cas_mismatch_halts`(SC-35) — 같은 모양, 변조 대상 `market_states`.
- `restart_loads_tampered_wallet_and_sells_on_top`(SC-36) — SC-34와 같은 변조 뒤 **재기동**(`load_economic_state()` → 새 `Simulation` 주입) → 변조값이 새 기대값이 되어 그 위에서 판매가 성공.
- `recovery_tick_writes_no_market_row_or_event`(SC-50) — 회복 배수 tick에 `market_states`·`domain_events` 행 수·값이 그대로. ⊘: 그 tick에 실제 `MARKET_STATE` 브로드캐스트가 나가고 포화가 줄었음을 같은 테스트에서 확인(`check_market_recovery_broadcasts`가 실제로 돈다 — `Simulation::step()`은 빈 제출로도 tick마다 회복을 흐른다).
- `restart_preserves_saturation_and_last_sale`(SC-51) — 재기동 뒤 포화·마지막 거래 보존, 재기동 뒤 첫 배수 tick에 식대로 회복(2000→재기동→1000, `recovery_kg=1000`/`interval=10`로 손 계산 쉬운 값 — 디자인 §3.6 수치가 아니다, 영속화 경로만 잰다).

**`crates/persistence/tests/trading_duplicate.rs`**(신규, 3건):
- `sale_dup_across_restart_is_rejected`(SC-24) — "적재 → sim 기억 주입 경로"(S8 패턴): 재기동한 새 `Simulation`에 `seed_processed_command_ids` 등을 주입하면 같은 command_id 재전송을 **sim이 스스로** 거절(DUPLICATE_COMMAND_ID, DB 도달 전) — 네 표(wallets·inventory_items·processed_commands·market_states) 행 수 불변.
- `sale_dup_cross_actor_no_halt`(SC-26 DB) — 다른 actor의 다른 command_id 둘 다 정상 커밋, 서버 생존, 장부 2행.
- `sale_dup_reaching_db_halts`(SC-28) — sim 기억을 **손으로 짠 배치**로 우회해 같은 command_id를 직접 영속화에 밀어넣음 → `processed_commands_pkey` 위반 → 정지, 지갑은 첫 판매 뒤 값 그대로.

실행 로그(2026-10-06):
```
STARFALL_DB_TEST RAN full_sale_keeps_zero_inventory_row
STARFALL_DB_TEST RAN wallet_cas_mismatch_halts
STARFALL_DB_TEST RAN market_cas_mismatch_halts
STARFALL_DB_TEST RAN restart_loads_tampered_wallet_and_sells_on_top
STARFALL_DB_TEST RAN recovery_tick_writes_no_market_row_or_event
STARFALL_DB_TEST RAN restart_preserves_saturation_and_last_sale
STARFALL_DB_TEST RAN sale_dup_across_restart_is_rejected
STARFALL_DB_TEST RAN sale_dup_cross_actor_no_halt
STARFALL_DB_TEST RAN sale_dup_reaching_db_halts
test result: ok. 6 passed (trading_sale.rs)
test result: ok. 3 passed (trading_duplicate.rs)
```
합쳐서 qa3 계약의 [DB] 11개(이 9개 + 기존 `trading_load_state.rs` 2개, SC-51 적재 함수 층) 전부 RAN·통과.

### 자기 점검(R3, S4 2부 — production 코드 4곳 각각 변이·sha256 원복)

| 변이 대상 | 잡는 테스트 | ③ sha256 원복 |
|---|---|---|
| `apply_state_write` Wallet의 UPDATE에서 `AND balance_cr = $4` 제거(CAS 무력화) | `wallet_cas_mismatch_halts` — "지갑 행 비교 후 쓰기 실패는 월드를 멈춰야 한다" 실패 | 변이 전/후 `sha256sum crates/persistence/src/lib.rs` = `fc18fa62ea6767684bd2093bab7d8a106458a555fab7d630e25cd55cf874884f`, 동일 |
| `apply_state_write` MarketState의 UPDATE에서 `AND saturation_kg=$8 AND as_of_tick=$9` 제거 | `market_cas_mismatch_halts` — 같은 실패 문구 | 위와 동일 해시로 원복 확인 |
| `commit()`의 `MineralSold` → `processed_commands` 분기를 `if false`로 비활성 | `sale_dup_reaching_db_halts` — "processed_commands PK 위반은 월드를 멈춰야 한다" 실패(장부가 안 쌓여 PK 위반 자체가 안 일어남) | 위와 동일 해시로 원복 확인 |
| `contract_payload`의 `MineralSold` 분기가 레지스트리 이름을 `MINERAL_MINED`로 잘못 반환 | `full_sale_keeps_zero_inventory_row` — `event_type='MINERAL_SOLD'` 질의가 `RowNotFound`로 패닉 | 위와 동일 해시로 원복 확인 |

네 변이 모두 각자 지명된 테스트가 정확히 실패했고(다른 테스트가 우연히 잡은 것이 아님 — 필터로 단독 실행해 확인), 원복 후 `sha256sum`이 매 번 변이 전 베이스라인과 바이트 단위로 일치했다.

### 게이트(최종, 2026-10-06)

`server/`, `export PATH="$HOME/.cargo/bin:$PATH"`.

- `cargo fmt --all --check` — 통과(이 크레이트).
- `cargo clippy -p starfall-persistence --all-targets -- -D warnings` — 통과.
- `STARFALL_DB_TESTS=required cargo test -p starfall-persistence --locked --no-fail-fast` — 전부 통과(history 15, restart_economic_state 1, trading_load_state 2, trading_sale 6, trading_duplicate 3).
- `cargo clippy --workspace --all-targets -- -D warnings`(워크스페이스 전체) — `starfall-game-server`만 실패(`main.rs`가 아직 `WorldConstants.markets`를 안 채움 — server3의 S2/S5 마무리 몫, `main.rs`는 내 소유가 아니다). `STARFALL_DB_TESTS=required cargo test --workspace --exclude starfall-game-server` 기준으로는 `starfall-sim`에 server3가 작업 중인 단위 테스트 1건(`sale_and_mine_same_tick_interleave_by_submission`)과 `starfall-gateway`에 S5 미완(`SELL_MINERAL`이 `parse_command`에 아직 없음) 실패가 있다 — **둘 다 server3 진행 중인 자기 범위**이고 내 변경과 무관함을 테스트 이름·에러 메시지로 확인했다.

### 0004 동결 선언

S4의 [DB] 전부가 초록이 됐으므로 `_workspace/p1-03-trading/migration_freeze.md`에 선언 줄을 추가했다(Q3 첫 실서버 봇 실행 전 마감 — 지켰다). qa3에게 알린다.

### 알려진 한계

- `trading_sale.rs`/`trading_duplicate.rs`의 월드·시장 상수(`recovery_kg=1000`·`interval=10` 등)는 영속화 경로만 재려고 고른 값이고 디자인 §3.6 가격표의 실제 수치가 아니다 — 가격 golden은 S3b의 `pricing_golden.rs`가 정본이다.
- SC-26의 "sim 기억이 먼저 하나를 막는다" 절반은 `starfall-sim`의 단위 테스트(server3 소유)가 잰다 — 이 파일의 `sale_dup_cross_actor_no_halt`는 "영속화까지 도달하는 것은 둘 다 커밋된다"는 나머지 절반만 잰다(계약 문구 그대로).

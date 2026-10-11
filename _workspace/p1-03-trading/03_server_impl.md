# p1-03-trading — server3 구현 기록

- 작성: server3, 2026-10-05~07
- 범위: T0·S1·S2·S3b·S3·S5 전부 완료. S3 는 qa3 지적(사유 6 전용 테스트·SC-40·SC-45·46) 보완까지 끝났다.

## T0 — 경계 합의·ADR-0016 검토

`02_server_ack.md`·`02_interface.md` 로 완료, serverdb3 와 합의(2026-10-05). ADR-0016 accepted(architect3 확인). 요지:

- `StateWrite::Wallet`/`StateWrite::MarketState` variant 2개를 `server/crates/sim/src/mining.rs` 의 기존 enum에 추가(위치 유지 — 크레이트 루트 재노출이라 옮길 이유가 없다).
- `MarketState` 의 CAS 비교 대상은 `(saturation_kg, as_of_tick)` 만 — `last_seller_actor_id`·`last_sale_quantity_kg` 는 이전 값에서 유도되지 않는 "이번 판매 그대로" 값이라 비교 없이 덮어쓴다(serverdb3 역제안, 합의).
- `persistence::EconomicState` 에 `wallets`·`market_states` 필드, `Simulation::seed_wallet`/`seed_market_state` 신설.
- 게이트웨이 `RECORDING_BACKLOG` 판정 대상에 `SELL_MINERAL` 추가(S5 때 배선).
- 0004 제약 18개(wallets 6 + market_states 12) 이름 확정 — ADR-0016 §5 원문 그대로, 표 밖 제약 추가 없음.

## S1 — 계약 Rust 타입

### 구현 요약

7개 신규 타입 + 1개 신규 primitive + 기존 2개 타입 확장, 전부 기존 p1-02 코드 패턴을 그대로 따랐다(새 패턴 없음):

| 파일 | 변경 |
|---|---|
| `server/crates/contracts/src/primitives.rs` | `CreditAmount`(i64, 0..=2^53-1, `bounded_int_newtype!`) + `de_positive_credit_amount`(형제 `minimum:1` 필드용, `de_positive_mass_kg` 와 같은 패턴) |
| `server/crates/contracts/src/commands.rs` | `SellMineralCommand`/`SellMineralPayload`/`SellMineralType` |
| `server/crates/contracts/src/events.rs` | `MineralSoldEvent`/`MineralSoldPayload`/`MineralSoldType`(필드 18개, `causation_id`·`actor_id` 좁힘) |
| `server/crates/contracts/src/messages.rs` | `WalletStateMessage`·`SaleReceiptMessage`·`MarketStateMessage`(+ 중첩 `MarketListingState`·`LastSale`), `RejectReasonCode::InsufficientQuantity` 추가 |
| `server/crates/contracts/src/data.rs` | `MarketTable`(+`MarketRules`·`MarketListing`), `DepositReserveTable`(+`DepositReserve`), `DepositFieldTable`/`Deposit` 를 v2로 개정(`mineral_id`·`initial_reserve_kg` 제거, `schema_version` 을 `ConstSchemaVersion<2>` 로) |
| `server/crates/contracts/src/registry.rs` | `CONTRACT_TYPES` 23→30, `DEPOSIT_FIELD` 의 `schema_version` 필드 1→2, 레지스트리 이름 상수 7개 |
| `server/crates/contracts/tests/contract_tests.rs` | `EXPECTED_SCHEMA_COUNT` 29→36, `EXPECTED_VALID_FIXTURES` 46→57, `EXPECTED_INVALID_FIXTURES` 74→113, `SERDE_REJECTION_TABLE` +30행(신규 고유 파일명), `registry_responses` 의 명령 kind 수 3→4, **신설** `registry_data_files` 테스트(SC-44) |
| `server/crates/gateway/src/stats.rs` | `REJECT_REASONS` 15→16(`INSUFFICIENT_QUANTITY`), `reason_index` match 암 추가 — `RejectReasonCode` 가 비열거(non-exhaustive match)라 워크스페이스 컴파일이 깨지는 것을 막는 **컴파일 필수** 최소 수정(S5 의 실제 배선은 아직 없음, 라벨·색인만) |

fixture 파일은 전부 architect3 가 이미 만들어 뒀다(`contracts/fixtures/{SELL_MINERAL,MINERAL_SOLD,WALLET_STATE,SALE_RECEIPT,MARKET_STATE,MARKET,DEPOSIT_RESERVE}/**`, `DEPOSIT_FIELD` 반례 갱신) — S1 은 그 fixture 가 가리키는 Rust 모양을 만드는 일이었다.

### RED 확인

`cargo test -p starfall-contracts registry::tests::find_returns_registered_types` 가 구현 전 `assert_eq!(CONTRACT_TYPES.len(), 23)` 그대로였고(architect3 가 01_architect_tasks.md·스펙 §5.2 에 실측해 둔 기준선: 타입 23, fixture 46/74), 그 기준선과 이번 구현 뒤 수(30/57/113)를 대조한 것이 RED→GREEN 증거다(아래 §2 의 R3 표가 그 GREEN 을 실제로 실행해 확인한다).

### 게이트 (전부 통과)

```
cargo fmt --all --check                                        # 통과
cargo clippy -p starfall-contracts -p starfall-gateway \
  -p starfall-sim -p starfall-persistence -p starfall-testdb \
  --all-targets -- -D warnings                                  # 통과, 경고 0
cargo test -p starfall-contracts --locked                       # 50 passed (unit 37 + contract_tests 13)
```

**알려진 한계 — `cargo test --workspace`·`cargo build -p starfall-game-server` 는 아직 RED다.** `server/bins/game-server/src/{data.rs,main.rs}` 가 `Deposit.mineral_id`/`Deposit.initial_reserve_kg`(v1 필드, v2 에서 제거)를 참조해 컴파일 실패한다(E0609, 4건) — **S2 가 고치기 전까지 의도된 RED**다(01_architect_tasks.md: "서버도 지금 기동하지 못한다 ... S2 가 고칠 때까지 기동 거부가 정상이다"). `starfall-contracts`·`starfall-gateway`·`starfall-sim`·`starfall-persistence`·`starfall-testdb` 는 전부 컴파일·테스트 통과(위 게이트).

## R3 자기 점검 (qa-throughput)

| SC | 이름 실행(필터) | 구현 변이 | sha256 원복 | 지명 도구 |
|---|---|---|---|---|
| SC-42(AC-9) 타입 30·유효 57·반례 113·스키마 36·레지스트리 30 | `cargo test -p starfall-contracts fixtures_roundtrip` · `invalid_rejected_by_schema` · `schemas_valid_offline` · `registry_consistency` — 4건 전부 단독 실행 통과(위 게이트 로그) | `registry.rs` 의 `WALLET_STATE` 항목 이름을 `MUTATION_TEST_DISABLED_WALLET_STATE` 로 바꿈 → `fixtures_roundtrip` 이 `contracts/fixtures/MUTATION_TEST_DISABLED_WALLET_STATE` 디렉터리를 못 찾아 FAILED(panic, 경로 없음) | `registry.rs` 변이 전·후 sha256 `f1a99431e...` 동일(원복 확인, 아래 로그) | 해당 없음(컴파일 타임 상수 대조) |
| SC-43(AC-9, 스펙 §5.3) serde 반례 결과가 §5.3 표와 일치 | `cargo test -p starfall-contracts invalid_fixtures_layered -- --nocapture` — 113건 전부 "기대=거부 실제=거부", 신규 30행 전부 포함(아래 전체 출력 발췌) | `primitives.rs` 의 `de_positive_credit_amount` 를 `>= 0` 으로 완화 → `proceeds-zero.json` 이 "기대=거부 실제=통과" 로 FAILED | `primitives.rs` 변이 전·후 sha256 `51870ddf7...` 동일 | 해당 없음 |
| SC-44(AC-9) `data_files` data 9종 전용, `SELL_MINERAL.responses` accepted 순서 | `cargo test -p starfall-contracts registry_data_files` · `registry_responses` 둘 다 단독 실행 통과 | `registry.rs` 의 `SELL_MINERAL.responses` 의 accepted 세 항목(`SALE_RECEIPT`·`INVENTORY_STATE`·`WALLET_STATE`) 순서를 뒤섞음 → 두 테스트 모두 FAILED(기대 순서와 다름, 아래 로그) | 같은 `registry.rs` 변이 — 위 SC-42 행과 같은 원복 확인(파일 하나, 변이 두 번, 최종 sha256 동일) | 해당 없음 |

**원복 증명 로그**(같은 턴에서 실행, 파일 커밋 전·untracked 상태):

```
# 변이 전
f1a99431e402f3c8e18dd7fbf66bb61c0c284ef439f0fe6dc6bd42d4bd7bd83c *src/registry.rs
51870ddf7bb8c096a3228e7b14cb88d78cd0991fd0f4d8206ce242ae31255a96 *src/primitives.rs
# 변이 A(WALLET_STATE 이름) 적용 → fixtures_roundtrip FAILED(경로 없음) → 원복
# 변이 C(SELL_MINERAL responses 순서) 적용 → registry_data_files, registry_responses 둘 다 FAILED → 원복
# 변이 B(de_positive_credit_amount >= 0) 적용 → invalid_fixtures_layered FAILED(proceeds-zero.json 거부=false) → 원복
# 변이 뒤 재측정
f1a99431e402f3c8e18dd7fbf66bb61c0c284ef439f0fe6dc6bd42d4bd7bd83c *src/registry.rs   # 동일
51870ddf7bb8c096a3228e7b14cb88d78cd0991fd0f4d8206ce242ae31255a96 *src/primitives.rs # 동일
```

`invalid_fixtures_layered -- --nocapture` 전체 출력(113건)은 이 턴의 실행 로그에 있다 — 신규 30행 전부(`price-field-injected`·`balance-field-injected`·`missing-market-id`·`quantity-fractional`·`quantity-above-mass-max`·`quantity-zero`·`missing-pricing-version`·`missing-floor-price`·`proceeds-fractional`·`proceeds-zero`·`quantity-after-negative`·`wallet-after-zero`·`fractional-credits`·`negative-credits`·`above-safe-integer`·`missing-command-id`·`unlisted-hint-injected`·`missing-last-sale-key`·`price-zero`·`saturation-negative`·`last-sale-quantity-zero`·`server-only-mineral-field`·`schema-version-1`·`reserve-zero`·`layout-field-injected`·`floor-pct-zero`·`no-listings`·`state-field-injected`·`price-fractional`)가 "기대=거부 실제=거부"로 일치했다. 예측(스펙 §5.3 Rust serde 열 "거부" 전부)과 실측이 정확히 같다 — 예측을 실측으로 바꿀 필요 없음.

## qa3 에게

- SC-42·43·44 는 위 필터로 증명된다. 이견 없으면 §0.5 표의 신규 ② 열(Rust serde)은 전부 "거부"로 확정 — 예측과 실측이 일치해 수정 없음.
- `registry_data_files`(SC-44)는 내가 새로 만든 테스트다(이전엔 없었다) — 이름 그대로 계약에 반영했다.
- H1(러너 통과)·S2(데이터 적재)·S3(sim 판매)·S3b(가격)·S4(영속화)·S5(게이트웨이)는 이 문서에 이어서 기록한다.

## qa3 02_sprint_contract.md(r0 초안) 리뷰 — server3 담당 SC-01~49

§11 행 A~K 전부 "이 방법으로 증명할 수 있다"로 회신(2026-10-06, SendMessage). 테스트 함수 이름은 계약 그대로 쓴다(필터 목록은 메시지 참고). 답변한 질문:

- **Q-2**(사유별 `commands_rejected_total` 관측 지점): 사유 2~5(판정 필요)는 `starfall-sim`(SC-07 ①) + 실서버 `stats_delta.py` 관찰만, 사유 6·7(`INSUFFICIENT_QUANTITY`·`CAPACITY_EXCEEDED`, 이번에 새로 매핑)은 `starfall-gateway` 단위 테스트가 플럼빙만 확인 — qa3 안 그대로 확정.
- **Q-7**(`/debug/stats` 신규 필드 이름, SC-04): `markets_loaded`·`sole_market_id`·`sole_market_pricing_version`·`sole_market_listings_loaded`·`deposit_reserves_loaded` — 기존 `minerals_loaded`/`deposits_loaded`/`rule_version`(p1-02)과 같은 패턴. S2 구현 시 `StatsBody`(gateway)와 `GameData`(game-server data.rs)에 반영한다.

architect3 가 스펙 §4.8 에서 시장 기동 검산 3개(`sell_cooldown_s`/`recovery_interval_s` × `tick_hz` 정수, `(2^31−1)×max(base)≤2^53−1`)를 뺐다(스키마가 이미 보장 — 소수 초는 스키마층 반례로 막힌다) — **S2 의 `data.rs` 에 이 세 검산을 만들지 않는다.** `market_check_m13_proceeds_overflow_bound` 는 스키마를 거치지 않는 단위 테스트로만 유지(qa3 확인).

## qa3 R3 실행 확인 후 보완(2026-10-06)

qa3 가 SC-42·43·44 필터 8개를 직접 실행해 단독 1건씩 통과를 확인했고(§0.5 ② 열 42/42 일치), `invalid_fixtures_layered`·`fixtures_roundtrip`·`invalid_rejected_by_schema`·`schemas_valid_offline`·`registry_consistency` 의 `println!` 라벨이 `[p1-02 SC-35/36]` 만 있어 읽기 어렵다는 지적(선택)에 맞춰 `[p1-02 SC-35 / p1-03 SC-42]`·`[p1-02 SC-36 / p1-03 SC-43]` 로 병기했다(5개 지점, `tests/contract_tests.rs`). 게이트 재확인: `cargo fmt --all --check`·`cargo test -p starfall-contracts --locked`(50 passed)·`cargo clippy -p starfall-contracts --all-targets -- -D warnings` 전부 통과.

## S2 — 데이터 적재·검산

### 구현 요약

`server/bins/game-server/src/data.rs`:

- `Deposit`(v2, 공개부) 전용 `validate_deposit_fields` — `mineral_id`/`initial_reserve_kg` 를 더는 다루지 않는다(ADR-0015 §2로 `DepositReserve` 로 옮겼다). star_system_id 일치·소프트 경계·id 중복·채굴 구역 겹침만 남는다.
- `load_deposit_reserves`/`validate_deposit_reserves` 신설 — `DEPOSIT_RESERVE`(서버 전용) 로딩 + 광물 존재·regen_kg 검산 + **공개부와의 양방향 deposit_id 짝 검증**(§4.8 d01·d02, 광물 커버리지 검산보다 먼저 — 짝 없음이 "그 광물에 매장지가 없다"로 가려지지 않게 순서를 정했다).
- `load_markets`/`validate_markets` 신설 — `MARKET` 로딩 + §4.8 m01~m12(아래 "architect3 변경" 참고, m08·m09·m13 제외) + `check_listing_floor_and_cap`(바닥값·포화 상한), `floor_band` 순수 함수.
- `GameData` 에 `deposit_reserves`·`markets` 필드, `sole_market()` accessor(`sole_rule_version` 과 같은 패턴) 추가.
- `loaded_files` 10 → 12(시장·매장지 예비 파일 추가).

`server/bins/game-server/src/main.rs`:

- `build_world_constants` 의 `DepositConstants` 조립이 `game_data.deposit_reserves` 에서 `mineral_id`/`initial_reserve_kg` 를 조회하도록 수정(`.ok_or_else(...)?` — S2 검산이 짝을 보장하므로 실패하지 않는 방어적 경로).
- `/debug/stats` 호출에 markets·deposit_reserves·sole_market 필드 5개 추가(qa3 Q-7 답변 그대로).

`server/crates/gateway/src/stats.rs` — `/debug/stats` 신규 필드 5개(`markets_loaded`·`deposit_reserves_loaded`·`sole_market_id`·`sole_market_pricing_version`·`sole_market_listings_loaded`), `set_data_loaded` 시그니처 확장. `crates/gateway/tests/ws_integration.rs` 호출부 2곳도 같이 고쳤다(인자 5개 추가, 전부 기본값).

### architect3 결정 반영(2026-10-06)

스펙 §4.8 의 시장 검산 3개(`sell_cooldown_s`·`recovery_interval_s` × `tick_hz` 정수, `(2^31-1) × max(base) <= 2^53-1`)는 스키마가 이미 보장해 도달 불가능한 코드이므로 **만들지 않았다**. m13(오버플로 경계)은 스키마를 거치지 않는 순수 산술 단위 테스트(`market_check_m13_proceeds_overflow_bound`)로만 남겼다 — qa3 에게도 이 결정을 확인했다(§ "qa3 02_sprint_contract.md 리뷰" 절 참고).

### 테스트 — SC-05(스프린트 계약, p1-03) m01~m13·d01~d04 전부 신설

`server/bins/game-server/src/data.rs` 의 `#[cfg(test)] mod tests`:

| 필터 | 경우 |
|---|---|
| `data::tests::market_reject_m01_wrong_star_system` | m01 |
| `data::tests::market_reject_m02_pricing_version_wrong_prefix` | m02 |
| `data::tests::market_reject_m03_mineral_without_listing` | m03 |
| `data::tests::market_reject_m04_duplicate_listing_for_one_mineral` | m04 |
| `data::tests::market_reject_m05_listing_unknown_mineral` | m05 |
| `data::tests::market_reject_m06_outside_soft_boundary` | m06 |
| `data::tests::market_reject_m07_docking_zone_overlaps_mining_zone` | m07 |
| `data::tests::market_reject_m08_sell_cooldown_fractional` | m08(schema 층, 도달 가능한 반례로) |
| `data::tests::market_reject_m09_recovery_interval_fractional` | m09(같음) |
| `data::tests::market_reject_m10_floor_price_below_one` | m10 |
| `data::tests::market_reject_m11_saturation_cap_exceeds_mass_kg_max` | m11 |
| `data::tests::market_reject_m12_duplicate_market_id` | m12 |
| `data::tests::market_check_m13_proceeds_overflow_bound` | m13(순수 산술) |
| `data::tests::deposit_split_reject_d01_only_in_public` | d01 |
| `data::tests::deposit_split_reject_d02_only_in_reserve` | d02 |
| `data::tests::deposit_split_reject_d03_schema_version_1` | d03 |
| `data::tests::deposit_split_reject_d04_reserve_file_missing` | d04 |

양성 대조: `data::tests::accepts_unmodified_copy` 를 확장(시장 1·상장 4·매장지 예비 8 = `deposits.len()`). p1-02 의 `reject_c01`/`reject_c02`(mineral_id 반례)는 v2 분할로 `world/deposit-reserves/cradle.json` 을 변조하도록 고쳤다(이전엔 공개부를 변조했는데 그 필드가 이제 없다). `returns_ten_loaded_files...` 는 `returns_twelve_loaded_files...` 로 개명(10→12).

### R3 자기 점검

| SC | 이름 실행(필터) | 구현 변이 | sha256 원복 | 지명 도구 |
|---|---|---|---|---|
| SC-04(AC-2a) `/debug/stats` 시장·예비 필드 | `cargo test -p starfall-game-server accepts_unmodified_copy` 단독 통과(시장 1·상장 4·예비 8 단언) | (아래 세 변이 공통) | (아래) | 해당 없음 |
| SC-05(AC-2b) m01~m12 전부 거부 | `cargo test -p starfall-game-server data::tests::market_reject_` — 12건 전부 단독 통과 | `check_listing_floor_and_cap` 의 `floor_price < 1` → `< 0` 으로 완화 → `market_reject_m10_floor_price_below_one` FAILED(실제로 `Ok` 반환) | `data.rs` 변이 전·후 sha256 `53a49adc5...` 동일 | 해당 없음 |
| SC-05(AC-2b) d01~d04 전부 거부 | `cargo test -p starfall-game-server data::tests::deposit_split_reject_` — 4건 전부 단독 통과 | `validate_deposit_reserves` 의 양방향 짝 검사 `if !only_in_reserve.is_empty()` → `if false && ...` 로 무력화 → `deposit_split_reject_d02_only_in_reserve` FAILED | 같은 파일, 위와 같은 원복 확인(파일 하나, 변이 세 번, 최종 sha256 동일) | 해당 없음 |
| SC-05 m13 | `cargo test -p starfall-game-server market_check_m13_proceeds_overflow_bound` 단독 통과 | `validate_markets` 의 포화 상한 비교를 `> i64::from(i32::MAX) * 2` 로 완화 → `market_reject_m11_saturation_cap_exceeds_mass_kg_max` FAILED(2.8e9 가 완화된 상한보다 작아 통과) | 같은 파일, 위와 같은 원복 확인 | 해당 없음 |

**원복 증명 로그**(같은 턴, 커밋 전 untracked 상태):

```
# 변이 전(= 최종 상태와 동일, 아래와 비교)
53a49adc50dcb5e5f9f47a4a1981812ea8a184c3273460388175bdeb4d9daf92 *bins/game-server/src/data.rs
0d03806d8e8a99866c14d4323476c9d9462450c8a5f13fd94db612c0736c8999 *crates/gateway/src/stats.rs
50c3a69349add81c1ec36586a2555d4c4a6925556c0e99e61fadf900f3be6df5 *bins/game-server/src/main.rs
# 변이 A(floor_price < 0) → market_reject_m10 FAILED → 원복
# 변이 B(deposit 짝 검사 무력화) → deposit_split_reject_d02 FAILED → 원복
# 변이 C(saturation cap × 2) → market_reject_m11 FAILED → 원복
# 변이 뒤 재측정 — 전부 동일
53a49adc50dcb5e5f9f47a4a1981812ea8a184c3273460388175bdeb4d9daf92 *bins/game-server/src/data.rs
0d03806d8e8a99866c14d4323476c9d9462450c8a5f13fd94db612c0736c8999 *crates/gateway/src/stats.rs
50c3a69349add81c1ec36586a2555d4c4a6925556c0e99e61fadf900f3be6df5 *bins/game-server/src/main.rs
```

### 게이트

```
cargo fmt --all --check                                        # 통과
cargo clippy --workspace --all-targets -- -D warnings           # 통과, 경고 0(이제 전체 워크스페이스!)
cargo test -p starfall-game-server --locked                     # 57 passed
cargo test -p starfall-contracts --locked                       # 50 passed
cargo test -p starfall-sim --locked                              # 78 passed
cargo test -p starfall-history --locked                          # 통과(DB 테스트 ignored)
cargo test -p starfall-persistence --locked                      # 통과(DB 테스트 ignored)
cargo test -p starfall-gateway --locked --test ws_integration     # 25 passed, 1 FAILED(아래)
cargo test --workspace --locked                                  # 가게이트 lib 테스트 1건 제외 전부 통과
```

**유일한 RED — 의도됨, S5 전까지**: `starfall-gateway` 의 `ws::tests::every_registry_command_type_is_dispatchable`(lib)와 `tests/ws_integration.rs` 의 `every_registry_command_type_passes_through_the_gateway`(통합) 둘 다 "`SELL_MINERAL` 은 계약의 명령 타입인데 게이트웨이가 모른다 — `parse_command` 에 추가하라"로 실패한다. `server/crates/gateway/src/ws.rs` 의 `parse_command` 에 `SELL_MINERAL` 분기가 아직 없다 — **S5 몫**(01_architect_tasks.md 가 이미 그렇게 나눴다, S-A 패턴 — p1-01 라운드2 에서 실제로 있었던 결함 재발 방지 테스트). 그 둘을 뺀 워크스페이스 전체는 fmt·clippy·test 전부 깨끗하다 — `cargo build -p starfall-game-server`(S1 보고의 "알려진 한계")는 **이제 해결됐다.**

## S3b — 가격 규칙 순수 함수 + golden

### 구현 요약

`server/crates/sim/src/trading/mod.rs`(신설) + `price.rs`(신설, `pub mod trading;` 은 아니고 `mod trading;` + `lib.rs` 가 `price` 의 공개 타입만 재노출 — `mining.rs`/`StateWrite` 와 같은 패턴):

- `PriceRule`(순수 상수 구조체: `base_price_cr_per_kg`·`band_kg`·`price_floor_pct`·`price_drop_pct_per_band`) + `floor_band`·`saturation_cap`·`band`·`price_at_band`·`floor_price_cr_per_kg`(전부 `const fn`).
- `sell(rule, saturation_kg, quantity_kg) -> SaleResult` — 구간 걷기(스펙 §4.2a), kg 루프 아님(반복 ≤ `floor_band+1`).
- `effective_saturation(saturation_kg, as_of_tick, now_tick, recovery_kg, interval_ticks) -> i64` — 게으른 회복(I-87), `mining::effective_remaining` 과 같은 꼴(반대 방향·클램프 없음 — 상한은 `sell`이 이미 강제).
- `lib.rs` 가 `PriceRule`·`SaleResult`·`sell`·`effective_saturation` 재노출(S3 가 그대로 가져다 쓴다).

`server/crates/sim/tests/pricing_golden.rs`(신설) — `starfall-history` 의 `mineral_discovery.rs::golden` 모듈을 그대로 본뜸: `data/market/cradle-exchange.json` 원본 JSON을 정규화(`designer_note`·`rules.note` 제외) + SHA-256, 고정 입력(광물별 `floor_band`·band0~7 가격·`sell(0,band_kg)`·`sell(0,2×band_kg)`) 출력을 `pricing_version`(`cradle-exchange@1`) 이름의 golden 파일 2개에 묶는다. `STARFALL_REPLAY_BLESS=1` 로 **최초 생성**(이 golden 은 이번 슬라이스에 새로 생긴 것이다 — CLAUDE.md 가 금지하는 "기존 이동·채굴 golden 재생성"이 아니다).

`server/crates/sim/Cargo.toml` — `sha2`를 `[dev-dependencies]`에만 추가(운영 `[dependencies]`는 여전히 `starfall-contracts` 하나, SC-02 그대로).

### 테스트 — 02_sprint_contract.md SC-14~21 필터 그대로

| 필터 | SC |
|---|---|
| `trading::price::tests::design_table_*`(10개, P1~P10) | SC-14 |
| `trading::price::tests::split_invariance_*` | SC-15 |
| `trading::price::tests::monotonic_*`(2개) | SC-16 |
| `trading::price::tests::floor_and_cap_*`(2개) | SC-17 |
| `trading::price::tests::recovery_formula_equals_per_tick_simulation` | SC-18 |
| `trading::price::tests::proceeds_at_least_quantity_*` | SC-19(36 조합) |
| `trading::price::tests::band_walk_iterations_*`(3개) | SC-20 |
| `pricing_golden_binds_version_to_hash_and_table`·`pricing_golden_rejects_changed_price_without_version_bump`(`--test pricing_golden`) | SC-21 |

`design_table_*` 10건 전부 §0.6 P1~P10 숫자와 바이트 일치(디자인 문서에서 옮긴 상수, `data/` 안 읽음 — 코드 리뷰로 확인 가능). golden 출력(`tests/data/pricing_golden/cradle-exchange@1.output.txt`)의 band 0~7 가격이 디자인 §3.6 표(Ferrosite 10 9 8 7 6 5 4 3 등)와 정확히 일치.

### R3 자기 점검

| SC | 이름 실행(필터) | 구현 변이 | sha256 원복 | 지명 도구 |
|---|---|---|---|---|
| SC-14~20 | `cargo test -p starfall-sim --locked trading::` — 21건 전부 단독 통과(릴리스 빌드도 `cargo test -p starfall-sim --release --locked` 99 passed 에 포함 확인) | ① `price_at_band` 의 `/100`→`/1000` → `design_table_p1`·`p10` FAILED. ② `sell`의 `take = remaining.min(...)` → `take = remaining`(구간 무시) → 13건 FAILED(설계표 전부 + floor/cap + split invariance + band walk) | `price.rs` 변이 전·후 sha256 `405fcdbc...` 동일(두 변이 각각 걸고 원복, 최종 1회 비교) | 해당 없음(순수 함수 단위 테스트) |
| SC-21 | `cargo test -p starfall-sim --locked --test pricing_golden` — 3건 단독 통과 | `pricing_golden.rs`의 `obj.remove("designer_note")` 를 주석 처리 → `pricing_golden_binds_version_to_hash_and_table` FAILED(해시 불일치 실제로 관측) | `pricing_golden.rs` 변이 전·후 sha256 `367846af...` 동일 | golden 파일 2개 존재 확인: `tests/data/pricing_golden/cradle-exchange@1.{hash,output}.txt`(이번에 `STARFALL_REPLAY_BLESS=1` 로 신설 — 기존 이동·채굴 golden 은 건드리지 않음, `cargo test -p starfall-sim --release --locked` 의 `ac8_...`·`sc33_...` 가 여전히 통과해 확인) |

**원복 증명 로그**:

```
405fcdbcb2f436b2e6b788f83d7c446eb3225b0f78a3dab4e72dcde2d06f8a83 *crates/sim/src/trading/price.rs
367846afc0a29161fb5b28dc47967f17f3dd6efff3c6401b2bd514c8e598f6c0 *crates/sim/tests/pricing_golden.rs
# 변이 세 번(price_at_band 분모, sell 의 min 제거, golden 정규화의 designer_note 제거 생략) → 각각 FAILED 확인 → 원복
405fcdbcb2f436b2e6b788f83d7c446eb3225b0f78a3dab4e72dcde2d06f8a83 *crates/sim/src/trading/price.rs   # 동일
367846afc0a29161fb5b28dc47967f17f3dd6efff3c6401b2bd514c8e598f6c0 *crates/sim/tests/pricing_golden.rs # 동일
```

### 게이트

```
cargo fmt --all --check                                  # 통과
cargo clippy --workspace --all-targets -- -D warnings     # 통과, 경고 0
cargo test -p starfall-sim --locked                       # 99 passed(price 21건 포함)
cargo test -p starfall-sim --release --locked              # 99 passed, golden·determinism 불변
```

## S3 — sim 판매 판정

### 구현 요약

`server/crates/sim/src/mining.rs` — T0 합의대로 기존 `StateWrite` enum에 `Wallet{actor_id, expected: Option<i64>, new}`·`MarketState{market_id, mineral_id, expected: Option<(i64,u64)>, new, last_seller_actor_id, last_sale_quantity_kg}` 두 variant 추가(CAS 비교 대상은 `(saturation_kg, as_of_tick)` 만 — 마지막 거래 필드는 항상 덮어쓴다, serverdb3 역제안 그대로).

`server/crates/sim/src/simulation.rs`:

- `WorldConstants.markets: BTreeMap<DataId, MarketConstants>`, `Simulation`에 `wallets`·`actor_last_sale_tick`·`market_states` 세 맵, `seed_wallet`·`seed_market_state`(S3b 가 만든 `trading::{MarketConstants, MarketRuntimeState}` 그대로 재사용).
- `handle_sell_mineral` 신설 — `handle_mine_resource`와 같은 8단계 모양(판정 1~7 표 §4.2 그대로 — 1 중복, 2 시장·상장 존재, 3 판매 쿨다운, 4 도킹 구역, 5 속도, 6 보유량, 7 지갑 상한) + 적용(장부·쿨다운·인벤토리·지갑·시장 상태 갱신, `StateWrite` 3개, `MINERAL_SOLD` 이벤트, `SALE_RECEIPT`→`INVENTORY_STATE`→`WALLET_STATE` 순 응답).
- `open_session` — 세션 시작 송신에 `WALLET_STATE`를 `INVENTORY_STATE` 뒤·`DEPOSIT_FIELD_STATE` 앞에 삽입, 그 뒤 월드의 모든 시장에 대해 `MARKET_STATE`를 추가 송신(스펙 §5.1a, 다섯 메시지 순서).
- `handle_mine_resource` — 채굴로 처음 드러난 광물(`mineral_is_revealed` 를 매장지 상태 갱신 **전**에 캡처)이면 그 즉시 그 광물을 상장한 모든 시장에 `MARKET_STATE`를 다시 보낸다(I-86).
- `step` — 매 tick `check_market_recovery_broadcasts` 호출: 회복 배수 tick(`tick_number % recovery_interval_ticks == 0`)에서 실효 포화가 바로 전 tick과 달라진 시장에만 `MARKET_STATE`를 전원에게 보낸다(I-87, 판매·채굴이 없어도 회복만으로 트리거됨).
- 헬퍼: `build_wallet_state_message`·`build_market_state_message`(드러난 광물만 필터)·`broadcast_market_state`·`mineral_is_revealed`·`credit_amount_from_i64`.

### 테스트 — `mod trading_tests`(25건, `02_sprint_contract.md`의 필터 이름 그대로)

첫 구현(이전 턴)은 필터 이름이 계약과 어긋나 있었다 — 이번 턴에 전부 계약 그대로 개명·분리하고, 계약이 요구하는데 없던 경우(사유 3·5, SC-09, SC-11 역순, SC-12, SC-13 경계, SC-22·23·25·26·29, SC-48·49)를 전부 새로 썼다:

| 필터 | SC |
|---|---|
| `sale_judgement_order_cooldown_before_dock` | SC-08 |
| `sale_rejection_reason_2_no_such_market`·`_no_such_listing` | SC-07(사유 2, 두 경우) |
| `sale_rejection_reason_3_cooldown_active` | SC-07(사유 3) |
| `sale_rejection_reason_4_target_out_of_range` | SC-07(사유 4) |
| `sale_rejection_reason_5_ship_too_fast` | SC-07(사유 5) |
| `sale_holding_boundary_exact_accepts_and_plus_one_rejects` | SC-07(사유 6)·SC-10 |
| `sale_rejection_reason_7_capacity_exceeded` | SC-07(사유 7) |
| `sale_rejection_does_not_start_cooldown` | SC-09 |
| `sale_accepted_produces_event_and_responses_in_order` | SC-06 |
| `sale_same_tick_submission_order_determines_price`·`_reversing_swaps_proceeds` | SC-11(정순·역순 둘 다) |
| `sale_and_mine_same_tick_interleave_by_submission` | SC-12 |
| `sale_cooldown_t19_rejects_t20_accepts`·`sale_cooldown_is_independent_of_mining_cooldown` | SC-13 |
| `sale_dup_same_session` | SC-22 |
| `sale_dup_after_reconnect_in_linger` | SC-23 |
| `sale_dup_two_connections_same_tick` | SC-25 |
| `sale_dup_cross_actor` | SC-26 |
| `sale_dup_cross_kind_mine_then_sell_and_sell_then_mine` | SC-27 |
| `sale_dup_rejected_resend_in_session` | SC-29 |
| `session_start_sends_five_messages_in_order` | SC-47(c) |
| `market_state_lists_only_revealed_minerals` | (I-86, 계약 번호 없음·S3 자체 보강) |
| `market_state_sale_tick_sent_to_every_open_session` | SC-48(a) |
| `market_state_recovery_tick_both_changed_and_unchanged_are_observed` | SC-49(b) |

SC-24(`sale_dup_across_restart_is_rejected`)·SC-28(`sale_dup_reaching_db_halts`)는 `[DB]` 표식 — `starfall-persistence`, serverdb3 담당(위 "협업" 참고). SC-38(`sell_mineral_recording_backlog_`)·SC-44 관련 `commands_rejected_total` 카운터 쪽은 S5(게이트웨이) 몫.

### R3 자기 점검

| SC | 이름 실행(필터) | 구현 변이 | sha256 원복 | 지명 도구 |
|---|---|---|---|---|
| SC-06~13(사유·쿨다운·순서·경계 전부) | `cargo test -p starfall-sim --locked --lib sale_accepted_`·`sale_rejection_reason_`·`sale_judgement_order_cooldown_before_dock`·`sale_rejection_does_not_start_cooldown`·`sale_holding_boundary_`·`sale_same_tick_submission_order_`·`sale_and_mine_same_tick_interleave_by_submission`·`sale_cooldown_` — 각각 단독 실행, 다른 SC의 테스트가 섞이지 않음(아래 실행 발췌) | `handle_sell_mineral`의 판정 3(쿨다운) 조건 `tick_number < last_accepted_tick.saturating_add(...)` 을 `>` 로 반전 | `simulation.rs` 변이 전·후 sha256 `22332d1d...` 동일(아래 로그) | 해당 없음 |
| SC-22·23·25·26·27·29(중복 6종) | `cargo test -p starfall-sim --locked --lib sale_dup_` — 6건 단독 통과 | 같은 변이(위 행과 파일 하나 공유 — 쿨다운 반전은 중복 테스트에는 영향 없음을 실측으로 확인: 변이 적용 후에도 `sale_dup_` 6건은 전부 `ok`, 즉 이 변이로는 중복 계열이 안 죽는다는 것도 같은 실행에서 확인했다 — 별도 변이는 안 걸었다(이미 SC-01의 `processed_command_ids` 로직은 p1-02 S3에서 변이 검증됨, 재검증 불필요)) | — | 해당 없음 |
| SC-47(세션 시작 5 메시지) | `cargo test -p starfall-sim --locked --lib session_start_sends_five_messages_in_order` 단독 통과 | 위 쿨다운 변이와 별개 — `open_session`의 `WALLET_STATE` 삽입 순서를 `DEPOSIT_FIELD_STATE` 뒤로 옮기면(수동 확인, 코드 리뷰) 이 테스트가 바로 깨진다는 것은 테스트의 `assert_eq!(&kinds[..5], [...])`가 **순서 그대로** 비교하는 구조로 보장된다(부분집합 비교 아님) | 같은 `simulation.rs` 파일, 위 sha256과 동일 | 해당 없음 |
| SC-48·49(MARKET_STATE 전파) | `cargo test -p starfall-sim --locked --lib market_state_sale_tick_sent_to_every_open_session`·`market_state_recovery_tick_` 각각 단독 통과(아래 발췌) | (위 쿨다운 변이로 전체 `trading_tests::` 재실행해 교차 확인 — 이 두 테스트는 쿨다운과 무관해 영향 없음을 실측, 기대대로) | 같은 파일, 같은 sha256 | 해당 없음 |

**R3 요구 ①(이름 그대로 1건 이상 실행, 다른 SC 안 섞임)** — 21개 필터를 전부 단독 실행해 기대한 테스트 집합과 정확히 일치함을 확인(예: `sale_rejection_reason_` → 정확히 6건 `reason_2_no_such_market`·`reason_2_no_such_listing`·`reason_3`·`reason_4`·`reason_5`·`reason_7`만 걸림, `sale_dup_` → 정확히 6건).

**R3 요구 ②③(구현 변이 → 타겟 FAILED → 원복 → sha256 동일)**:

```
# 변이 전
22332d1d60af7183aa6bdf43e73ae9b327c69aaa7119a17a5eaba8a16c8e2de0 *crates/sim/src/simulation.rs

# 변이: handle_sell_mineral 판정 3(쿨다운)의 비교 연산자 `<` → `>` 로 반전
cargo test -p starfall-sim --locked --lib trading_tests:: 결과: 21 passed; 4 failed
  FAILED: sale_cooldown_is_independent_of_mining_cooldown
  FAILED: sale_cooldown_t19_rejects_t20_accepts
  FAILED: sale_judgement_order_cooldown_before_dock
  FAILED: sale_rejection_reason_3_cooldown_active
  (나머지 21건은 영향 없음 — 쿨다운과 무관한 경로라서 그대로 ok, 변이가 과대적용되지 않았다는 증거)

# 원복 후 재측정
22332d1d60af7183aa6bdf43e73ae9b327c69aaa7119a17a5eaba8a16c8e2de0 *crates/sim/src/simulation.rs   # 동일
cargo test -p starfall-sim --locked --lib trading_tests:: 결과: 25 passed; 0 failed
```

또한 `server/bins/game-server/src/main.rs`의 `build_world_constants`에 `WorldConstants.markets` 필드 조립을 추가했다(S2 까지는 빠져 있었다 — `cargo test --workspace`를 처음 돌렸을 때 `E0063 missing field markets`로 드러났다). `MarketTable`/`MarketRules`/`MarketListing`(공개부, 기동 검산이 이미 끝낸 값)을 `starfall_sim::MarketConstants`/`ListingConstants`/`PriceRule`로 그대로 옮긴다(판정 로직 없음 — `minerals`·`deposits` 조립과 같은 자리) — `sell_cooldown_s`/`recovery_interval_s`(초)를 `tick_hz`로 곱해 tick 단위로 바꾸는 부분만 `minerals`의 `regen_interval_s` 변환과 같은 패턴(`u64::try_from(...).unwrap_or(0).saturating_mul(u64::from(tick_hz))`)을 재사용했다. `data::tests::loads_the_real_data_directory`(실제 `data/` 디렉터리로 기동)가 이 조립을 통과시켜 실측 확인됐다.

### 게이트

```
cargo fmt --all --check                                        # 통과
cargo clippy --workspace --all-targets -- -D warnings           # 통과, 경고 0(전체 워크스페이스)
cargo test -p starfall-sim --locked                             # 전부 통과(trading_tests:: 25건 포함, lib 124 passed)
cargo test -p starfall-sim --release                             # 전부 통과(golden·determinism 불변)
cargo test -p starfall-game-server --locked                      # 57 passed(markets 필드 추가로 되살아남)
cargo test --workspace --locked --no-fail-fast                   # 아래 두 건만 FAILED, 나머지 전부 통과
cargo test -p starfall-gateway --locked --test ws_integration    # 25 passed, 1 FAILED(S5 몫, 아래)
```

`server/crates/gateway/tests/ws_integration.rs`의 `expect_session_start_state_messages` 헬퍼를 고쳤다(기존 4-메시지 가정이 `WALLET_STATE` 삽입으로 깨짐 — `INVENTORY_STATE`→`WALLET_STATE`→`DEPOSIT_FIELD_STATE` 순으로 세 메시지를 전부 소비·단언하도록). `default_world()`는 market 이 없어 `MARKET_STATE`는 이 헬퍼 뒤에 오지 않는다. 이 수정으로 FAILED 6건(`historical_event_notice_backfill_...`·`mine_resource_capacity_exceeded_...`·`notice_no_gap_same_tick`·`r4_s6_first_connection_is_closed_...`·`sc19_command_result_precedes_ping_reply_...`·`set_ship_control_over_a_real_socket_...`)이 전부 해소됐다 — 이 여섯은 S3 자체의 버그가 아니라 세션 시작 순서가 스펙대로 바뀐 데 따른 테스트 쪽 고착 단언이었다.

**유일한 잔존 RED(이 시점) — S5 몫(의도됨)**: `every_registry_command_type_passes_through_the_gateway`(통합)·`ws::tests::every_registry_command_type_is_dispatchable`(lib) — `parse_command`에 `SELL_MINERAL` 분기가 아직 없다. `cargo test --workspace --locked --no-fail-fast` 로 전체를 돌리면 이 두 건을 뺀 모든 크레이트가 통과한다(serverdb3의 S4 가 이미 끝나 있어 `starfall-persistence`도 포함해서 통과 — DB 의존 6건은 `DATABASE_URL` 미설정으로 SKIPPED, 환경 미검증). 아래 S5 절에서 이 두 건도 해소됐다.

## S3 보완 (qa3 지적, 2026-10-07)

qa3가 S3 완료 알림 전에 실행으로 잡은 3건을 전부 고쳤다.

1. **SC-07 사유 6 전용 테스트 없음** — `sale_rejection_reason_` 필터에 사유 6(`INSUFFICIENT_QUANTITY`)이 안 걸렸다(그 경로는 SC-10 의 `sale_holding_boundary_exact_accepts_and_plus_one_rejects` 가 덮었지만 이름이 안 맞았다). **`sale_rejection_reason_6_insufficient_quantity`** 를 새로 썼다 — 계약 ⊘ 그대로: 수락 1건을 먼저 만들어 지갑·인벤토리·시장 상태·판매 쿨다운이 전부 0 이 아닌 상태를 만든 뒤, 쿨다운이 끝나길 기다려 과대 판매를 거부시키고 그 네 상태가 **전혀 안 바뀜**을 단언한다. `sale_holding_boundary_*`는 그대로 SC-10(경계 양쪽) 전용으로 남겼다.
2. **SC-45·46 없음** — `market_listing_empty_before_first_extraction`(세션 2개, 둘 다 세션 시작 `MARKET_STATE.listings`가 빔을 확인) · `market_listing_appears_on_first_extraction_then_again_for_a_second_mineral`(광물 둘·시장 하나, 첫 채굴로 하나만 드러나고 둘째 채굴로 둘 다 드러나는 것을 **열린 세션 2개 전부**에서 확인, T-10 "다른 미드러난 광물 비누출"까지 같은 테스트에서 본다)를 새로 썼다.
3. **SC-40 `trading_replay_` 없음** — `server/crates/sim/tests/trading_replay.rs`(신설) — `mining_replay.rs`와 같은 2-프로세스 재생 패턴에 `SELL_MINERAL`을 섞었다. 비교 파일 3개: `snapshots.jsonl`(물리)·`events.jsonl`(`MINERAL_SOLD` 포함 도메인 이벤트, 계약 직렬화만 — `mining_replay.rs`와 같은 규율)·`results.jsonl`(세션 A·B 의 `COMMAND_RESULT` 전체, 수락·거부 사유 포함). 계획: 채굴(500kg)→판매(300kg, 수락, 쿨다운 20tick 시작)→**COOLDOWN_ACTIVE**(tick 3, 쿨다운 중 재시도)→쿨다운이 끝난 뒤(tick 25) **INSUFFICIENT_QUANTITY**(보유 200kg 인데 300kg 요청)→거부는 쿨다운을 다시 안 거므로 바로 다음 tick(26)에 남은 200kg 전부(수락, 구간 걸침)→그 뒤 TOTAL_TICKS(130)까지 흘려보내 회복 배수 tick(60·120)을 두 번 지난다. **둘째 함선(B)**: tick 2 부터 전진 100% 추력 지속 → tick 9 **TARGET_OUT_OF_RANGE**(짧은 가속만으로 이미 도킹 구역을 벗어난다 — 판정 순서 4가 5보다 먼저라 그 시점 속도와 무관하게 이 사유가 난다) → 같은 추력으로 더 가속한 뒤 tick 70 **SHIP_TOO_FAST**(실측 — 설계 당시엔 "가까운데 빠름을 먼저, 먼데 빠름을 나중"으로 tick 을 배치했으나 실제 물리 적분 결과는 반대로 나왔다, qa3 2026-10-08 지적 — 판정 로직엔 영향 없다, 두 사유 다 ≥1건이면 되는 ⊘라 tick 배치를 바꾸지 않고 주석만 고쳤다). qa3(2026-10-07 재지적)의 ⊘ 네 사유(쿨다운·도킹 밖·속도·보유 부족) + 둘째 함선을 전부 같은 실행에서 단언·출력한다(실측: `MINERAL_SOLD` 2건, `INSUFFICIENT_QUANTITY`·`COOLDOWN_ACTIVE`·`TARGET_OUT_OF_RANGE`·`SHIP_TOO_FAST` 각 1건, 회복 배수 통과 2회). 골든은 **최초 생성**(`STARFALL_REPLAY_BLESS=1`, `server/crates/sim/tests/data/replay_trading/` — 채굴 재생·이동 재생과 다른 디렉터리, 새 슬라이스의 새 골든이라 ADR-0010 §3.1 이 금지하는 "조용한 재생성"이 아니다) — qa3 지적으로 입력열을 넓힌 뒤 **두 번째 bless**를 했다(아래 "SC-40 재생성 로그"), 기존 이동(`tests/data/replay/`)·채굴(`tests/data/replay_mining/`) golden 은 손대지 않았다(sha256 변화 없음, `sc33_...`·`ac8_...` 테스트 통과로 확인).
4. **architect3 최종 판정(Q-1, 2026-10-07) 반영**: `market_check_m13_proceeds_overflow_bound`(순수 산술 단위 테스트)를 **삭제**했다(이전 보고의 "유지" 결정이 뒤집혔다 — §4.8 이 "계약이 보장하는 것"으로 명시적으로 옮겨져 고아 테스트를 남기지 않는다). `validate_markets` 위 주석도 그에 맞춰 고쳤다.

### SC-40 재생성 로그(qa3 지적 반영, 2026-10-07)

입력열에 둘째 함선(B)과 COOLDOWN_ACTIVE·TARGET_OUT_OF_RANGE·SHIP_TOO_FAST 거부를 추가해 **두 번째 bless**를 했다. 변경 전 golden 은 session A 단독·사유 1개(INSUFFICIENT_QUANTITY)뿐이었다 — 입력(`inputs.jsonl`)·결과(`results.jsonl`)·이벤트(`events.jsonl`)·스냅샷(`snapshots.jsonl`) 모두 세션 B의 추력·시도가 더해져 바이트 수가 늘어난다(스냅샷은 세션 A 기준만 쓰지만 세션 B가 같은 tick 에 존재해 world 전체 tick 진행에는 영향 없음 — 다만 시뮬레이션 틱 수 자체는 그대로 130이라 A의 snapshot 내용은 안 바뀐다, 실측: 바이트 수 증가는 추가된 `results.jsonl`·`inputs.jsonl` 항목 때문이고 `snapshots.jsonl`은 ship 2대 상태를 싣게 돼 커졌다):

```
# 변경 전(bless 전)
inputs.jsonl    2cf475917393c6aab1c6096b9505f44898f88033dc393341fa50062873785e74
events.jsonl    168ff359fec1aaeb9f20989046a974c9a5e994b0ae381e1d353ca88b73da4b97
results.jsonl   88da46cbe6aed96cdb5c870397459643668fc6828674782b73f67f513858dd25
snapshots.jsonl 0dc76873eb0ac1f30e0698e00377f842fb2e7fe58766728f2e1bbba632d004ae
initial.json    ad287026a142d142a3c5dbc4ce88e1732125125b109003303f076bceaaa5fc6a

# 변경 뒤(bless 뒤) — 넷 모두 바뀜(새 입력·새 거부·둘째 함선), 전부 1회
# 실행으로 재확인(사유 4건·MINERAL_SOLD 2건·회복 배수 2회 전부 출력됐다)
inputs.jsonl    46c2347c57657430e84a2f9f932c20849885bce540390ef0e93015625be91eb6
events.jsonl    2bccce31fe64334019a6a3939c700d9400bdcc8b32a01b648ac8b049a737bf88
results.jsonl   f5b260e2c66c8a7b6f2a72ddb7488a5d90748564d0fd2868fade8943a0687008
snapshots.jsonl 3a04b8dba960209e678bf9b369f59f9b7b33b6353ee733ffa7a548974958dc04
initial.json    b6b92d0ae39cbbe6d2c72608b1bc1516cb26dc01db237d739831f5e777785643

# 이후 일반 실행(BLESS 없이)으로 재확인 — 바로 위 값과 바이트 단위 일치(ok)
```

### R3 자기 점검 (S3 보완)

| SC | 이름 실행(필터) | 구현 변이 | sha256 원복 | 지명 도구 |
|---|---|---|---|---|
| SC-07(사유 6) | `cargo test -p starfall-sim --locked --lib sale_rejection_reason_` — 7건(사유 2 둘·3·4·5·6·7) 전부 단독 통과 | `handle_sell_mineral` 판정 6 의 `inventory_before < quantity` 를 `<=` 로 완화 → `sale_rejection_reason_6_insufficient_quantity`·`sale_holding_boundary_*`·그 외 수락 경로 의존 테스트 6건 FAILED(경계를 한 칸 당겨서 수락이 거부로 바뀌는 연쇄, 과대적용 아님 — 쿨다운과 무관한 경로임을 실측) | `simulation.rs` 변이 전·후 sha256 `37dd96fa1c05cb71b72efcb15f95b55e0bc79405d7a1c2a2eddee6e575951ee2` 동일 | 해당 없음 |
| SC-45·46 | `cargo test -p starfall-sim --locked --lib market_listing_` — 2건 단독 통과, `market_state_lists_only_revealed_minerals`(기존 I-86 테스트)도 같은 변이로 교차 확인 | `handle_mine_resource`의 I-86 재노출 브로드캐스트 조건 `if !mineral_was_revealed`를 `if mineral_was_revealed`로 반전 → `market_listing_appears_on_first_extraction_then_again_for_a_second_mineral`·`market_state_lists_only_revealed_minerals` 둘 다 FAILED(반대 조건에서 브로드캐스트가 전혀 안 나감) | 같은 파일, 위와 같은 sha256 동일(변이 두 개를 각각 걸고 원복, 최종 1회 비교) | 해당 없음 |
| SC-40 | `cargo test -p starfall-sim --locked --test trading_replay` 단독 통과(골든 5개 바이트 일치, qa3 지적 반영한 넓은 입력열 — 사유 4종·함선 2대 포함) | `world()`의 `base_price_cr_per_kg: 10` 을 `11`로 바꿈(넓은 입력열에 재적용) → `events.jsonl`이 골든과 바이트 단위로 달라 FAILED(오프셋 1597, `snapshots.jsonl`·`inputs.jsonl`은 그대로 일치 — 가격만 바뀌었다는 것의 직접 증거) | `trading_replay.rs` 변이 전·후 sha256 `cb814435cdc7cff2a6849eefe01b3e2287c6f2ada7eba53fee2b0a2400e8cac0` 동일(qa3 지적 반영 뒤의 현재 영구 상태) | 골든 파일 5개 존재 확인: `tests/data/replay_trading/{inputs.jsonl,initial.json,snapshots.jsonl,events.jsonl,results.jsonl}`(위 "SC-40 재생성 로그" — 두 번째 `STARFALL_REPLAY_BLESS=1`) |

**원복 증명 로그**(같은 턴, 커밋 전 untracked 상태):

```
# 변이 전(= 최종 상태와 동일)
37dd96fa1c05cb71b72efcb15f95b55e0bc79405d7a1c2a2eddee6e575951ee2 *crates/sim/src/simulation.rs
c66769fcbaede76ac7cfe18a23999d284ea077263d1d8daa48a335cc68a8d8a1 *crates/sim/tests/trading_replay.rs
# 변이 A(inventory_before <= quantity) → sale_rejection_reason_6·sale_holding_boundary_* 등 6건 FAILED → 원복
# 변이 B(!mineral_was_revealed → mineral_was_revealed) → market_listing_appears_*·market_state_lists_only_revealed_minerals FAILED → 원복
# 변이 C(base_price_cr_per_kg 10→11) → trading_replay 의 events.jsonl 골든 불일치 FAILED → 원복
# 변이 뒤 재측정 — 전부 동일
37dd96fa1c05cb71b72efcb15f95b55e0bc79405d7a1c2a2eddee6e575951ee2 *crates/sim/src/simulation.rs
c66769fcbaede76ac7cfe18a23999d284ea077263d1d8daa48a335cc68a8d8a1 *crates/sim/tests/trading_replay.rs
```

## S5 — 게이트웨이 SELL_MINERAL 배선

### 구현 요약

`server/crates/gateway/src/ws.rs`:

- `parse_command`에 `Some(registry::SELL_MINERAL) => parsed::<SellMineralCommand, _>(text, InboundCommand::SellMineral)` 분기 추가(`MINE_RESOURCE` 와 같은 자리) — S-A 결함 재발 방지 테스트(`every_registry_command_type_is_dispatchable`·통합의 `every_registry_command_type_passes_through_the_gateway`)가 요구한 그 한 줄.
- `RECORDING_BACKLOG` 판정의 `matches!(command, InboundCommand::MineResource(_))`를 `InboundCommand::MineResource(_) | InboundCommand::SellMineral(_)`로 확장(T0 합의 그대로 — 판매도 지갑·인벤토리·시장 상태를 바꾸므로 DB 에 의존한다, ADR-0016 §1).
- `handle_text`(lib)의 `every_registry_command_type_is_dispatchable` 테스트는 이미 있던 레지스트리 순회 테스트라 손대지 않았다 — 위 분기 추가만으로 그 자리에서 통과하게 됐다.

`server/crates/gateway/src/stats.rs` — SC-07(Q-2 답변) 플럼빙 전용 단위 테스트 `sell_mineral_rejection_counter_insufficient_quantity_and_capacity_exceeded` 신설: `record_rejection(InsufficientQuantity)`/`record_rejection(CapacityExceeded)`를 **각각 새 `Stats` 인스턴스에 따로** 기록해 그 라벨 칸만 오르고 다른 라벨 칸은 안 오르는지 확인한다(처음 썼던 버전은 둘을 한 인스턴스에 같이 기록해서 인덱스가 뒤바뀌어도 "둘 다 +1"이라 통과하는 함정이 있었다 — R3 변이로 실제로 걸렸고, 격리해서 고쳤다. 아래 R3 표).

`server/crates/gateway/tests/ws_integration.rs` — `world_with_one_mineable_deposit()`(M-17 전용 월드)에 시장 1개(`test-market`, 상장 `test-mineral`, 도킹 구역이 스폰 지점을 덮는다)를 추가하고, `sell_mineral_command`(10kg, `MINE_RESOURCE`가 먼저 25kg 캐 둔 보유량 안) 헬퍼와 `command_frame`의 `SELL_MINERAL` 분기를 추가했다. `every_registry_command_type_passes_through_the_gateway` 루프는 이제 이 월드의 세션 시작마다 `MARKET_STATE`(다섯째 메시지, 상장 포함)도 오므로 그 소비·단언을 한 줄 추가했다.

### 테스트 — SC-38·SC-07(사유 6·7 플럼빙)

| 필터 | SC |
|---|---|
| `sell_mineral_recording_backlog_rejects_over_limit_accepts_at_limit`(gateway lib) | SC-38 |
| `sell_mineral_rejection_counter_insufficient_quantity_and_capacity_exceeded`(gateway lib) | SC-07(Q-2, 사유 6·7 플럼빙) |
| `every_registry_command_type_is_dispatchable`(lib)·`every_registry_command_type_passes_through_the_gateway`(통합) | S-A 재발 방지(이번에 SELL_MINERAL 로 그린 전환) |

`sell_mineral_recording_backlog_*`는 `recording_lag`을 임계+1(21)로 만들어 거부(+대조로 같은 상태의 `SET_SHIP_CONTROL`은 수락)를 확인한 뒤, 앞선 배치를 커밋된 것으로 쳐서(`persist_handles().2.store(1, ...)`) 대기 큐 맨 앞을 가지치기시키고 새 배치(tick 2)를 넣어 `recording_lag`을 정확히 임계(20)로 만들어 이 사유로 더는 거부되지 않음을 확인한다(`set_last_enqueued_tick`이 큐에 **추가**만 하는 함수라는 것을 처음 버전에서 실측으로 알아내 고쳤다 — 아래 "오류와 수정" 참고).

### R3 자기 점검

| SC | 이름 실행(필터) | 구현 변이 | sha256 원복 | 지명 도구 |
|---|---|---|---|---|
| S-A 재발(dispatch) | `cargo test -p starfall-gateway --locked --lib every_registry_command_type_is_dispatchable` · `cargo test -p starfall-gateway --locked --test ws_integration every_registry_command_type_passes_through_the_gateway` 둘 다 단독 통과 | `parse_command`의 `SELL_MINERAL` 분기를 지움 → 위 두 테스트 **및** `sell_mineral_recording_backlog_*`까지 FAILED(그 테스트도 `parse_command`를 통과해야 `RECORDING_BACKLOG` 판정에 닿는다는 것의 증거) | `ws.rs` 변이 전·후 sha256 `9e11db0c7d3bc3c1e720543543727f38c271dcac0eb1830e39ea2c6323bb2da9` 동일 | 해당 없음 |
| SC-38 | `cargo test -p starfall-gateway --locked --lib sell_mineral_recording_backlog_` 단독 통과 | `RECORDING_BACKLOG` 판정의 `matches!`에서 `InboundCommand::SellMineral(_)`을 지움(`MineResource`만 남김) → `sell_mineral_recording_backlog_rejects_over_limit_accepts_at_limit` FAILED(판매가 거부 안 되고 `in_flight`에 들어가 버림) | 같은 파일, 위와 같은 sha256 동일 | 해당 없음 |
| SC-07(사유 6·7 플럼빙) | `cargo test -p starfall-gateway --locked --lib sell_mineral_rejection_counter_` 단독 통과 | `reason_index`의 `CapacityExceeded`·`InsufficientQuantity` 인덱스(14·15)를 서로 바꿈 → **처음 쓴 테스트는 안 걸렸다**(둘을 같이 기록해서 "둘 다 +1"이면 통과였다 — 위 "구현 요약" 참고) → 테스트를 "각자 새 `Stats`로 따로 기록, 다른 라벨은 안 오름까지 확인"으로 다시 써서 그 변이로 FAILED(`INSUFFICIENT_QUANTITY` 칸이 0) 확인 → 원복 | `stats.rs` 최종 상태 sha256 `f377188f9697af536a863eddfa0aea40b6a8c3cb8000b5d9176694997197be3d`(이 값은 테스트를 다시 쓴 뒤의 영구 상태 — 그 **안**에서 인덱스 변이 전/후가 동일함을 확인했다) | 해당 없음 |

**원복 증명 로그**:

```
9e11db0c7d3bc3c1e720543543727f38c271dcac0eb1830e39ea2c6323bb2da9 *crates/gateway/src/ws.rs
# 변이 A(SELL_MINERAL 분기 삭제) → dispatch 재발 테스트 2건 + backlog 테스트 FAILED → 원복
# 변이 B(RECORDING_BACKLOG matches! 에서 SellMineral 제거) → sell_mineral_recording_backlog_* FAILED → 원복
# 변이 뒤 재측정
9e11db0c7d3bc3c1e720543543727f38c271dcac0eb1830e39ea2c6323bb2da9 *crates/gateway/src/ws.rs   # 동일

# stats.rs — 테스트를 격리형으로 다시 쓴 뒤의 영구 상태에서 인덱스 변이 확인
f377188f9697af536a863eddfa0aea40b6a8c3cb8000b5d9176694997197be3d *crates/gateway/src/stats.rs
# 변이(reason_index 14<->15 교환) → sell_mineral_rejection_counter_* FAILED(INSUFFICIENT_QUANTITY 칸 0) → 원복
f377188f9697af536a863eddfa0aea40b6a8c3cb8000b5d9176694997197be3d *crates/gateway/src/stats.rs   # 동일
```

### 게이트

```
cargo fmt --all --check                                  # 통과
cargo clippy --workspace --all-targets -- -D warnings     # 통과, 경고 0
cargo test -p starfall-gateway --locked                    # lib 46 passed, 통합 26 passed — 전부 통과
cargo test --workspace --locked --no-fail-fast              # 전부 통과, FAILED 0건(S3·S4·S5 다 끝나 처음으로 워크스페이스 전체가 그린)
cargo test -p starfall-sim --release                        # 127 passed(golden·determinism 불변)
```

## 긴급 수정 — 기동 경로가 지갑·시장 상태를 seed 하지 않음(qa3 실서버 발견, 2026-10-07)

### 결함

`server/bins/game-server/src/main.rs`의 기동 경로가 `economic_state.inventory`·`economic_state.deposits` 는 `seed_inventory`·`seed_deposit_state` 로 sim 에 주입하면서, **`economic_state.wallets`·`economic_state.market_states` 는 `seed_wallet`·`seed_market_state` 로 주입하는 루프가 없었다**. `02_interface.md` "호출 예"가 지정한 다섯 호출 중 둘이 S2→S3 로 코드를 옮기며 빠졌다.

qa3 의 실서버 재기동 봇 두 재현:
1. `sell-dup` — 판매 수락(지갑 2,000) → `shutdown` → 재기동 → 같은 actor 접속 → 세션 시작 `WALLET_STATE` 가 **0**(DB `wallets` 행은 2,000).
2. `sell-cas` — 지갑 행을 2,777 로 변조 → 서버 자가 정지(의도대로) → 재기동 → 판매 시도 → sim 이 지갑을 몰라 `expected=None` 으로 INSERT 를 보냄 → 비교 후 쓰기(K3) 불일치 → **재기동한 서버가 그 판매에서 다시 정지**. 그 판매는 `COMMAND_RESULT` `ACCEPTED` 를 받았는데 DB 에 없다.

serverdb3 의 `restart_loads_tampered_wallet_and_sells_on_top`·`restart_preserves_saturation_and_last_sale`(persistence 크레이트)가 초록이었던 것은 그 테스트들이 `Simulation::seed_wallet`/`seed_market_state` 를 **직접** 부르기 때문이다 — `main()` 의 실제 기동 경로를 타는 테스트가 없어서 qa3 가 실서버로 먼저 잡았다.

### 수정

`main.rs`의 다섯 seed 호출(인벤토리·매장지·지갑·시장 상태·처리 장부)을 `seed_economic_state(simulation: &mut Simulation, economic_state: EconomicState)` 함수로 뽑아내고, 거기에 빠졌던 두 루프(`seed_wallet`·`seed_market_state`)를 추가했다. 함수로 뽑은 것 자체가 재발 방지 조치다 — `main()`·`serve()` 안에 흩어진 코드는 단위 테스트가 닿지 못한다(qa3 의 지적 그대로).

### 테스트 — `seed_economic_state_tests::seeded_wallet_and_market_state_appear_at_session_start`

`server/bins/game-server/src/main.rs`에 신설. 최소 월드(함선 하나·매장지 하나·시장 하나, 상장 `test-mineral`)를 만들고, `EconomicState`에 지갑 행(2,000cr)·시장 상태 행(포화 500kg, 마지막 판매 300kg)·드러난 매장지 행을 채워 `seed_economic_state`에 넘긴 뒤 세션을 열어 그 즉시 받는 `WALLET_STATE.balance_cr`·`MARKET_STATE.listings[0].saturation_kg`·`.last_sale`이 그 값 그대로인지 단언한다(qa3 재현 1·2를 둘 다 직접 재현 — seed 가 안 되면 지갑이 0, 상장의 포화가 0 이 된다).

### R3 자기 점검

| SC | 이름 실행(필터) | 구현 변이 | sha256 원복 | 지명 도구 |
|---|---|---|---|---|
| (계약 번호 없음 — 실서버 긴급 결함, SC-24·36·51 경로) | `cargo test -p starfall-game-server --locked seed_economic_state` 단독 통과 | `seed_economic_state`에서 `seed_wallet`·`seed_market_state` 두 루프를 통째로 지움(원래 결함 그대로 재현) → 테스트 FAILED(`left: 0, right: 2000` — qa3 재현 1 과 같은 증상을 그대로 잡아냄) | `main.rs` 변이 전·후 sha256 `e4514fdc7bf0281a9bf6ec235af9271fd8ad92ea8ef5f596c5046c1053114ff7` 동일 | 해당 없음 |

**원복 증명 로그**:

```
e4514fdc7bf0281a9bf6ec235af9271fd8ad92ea8ef5f596c5046c1053114ff7 *bins/game-server/src/main.rs
# 변이(seed_wallet·seed_market_state 루프 삭제, 원 결함 재현) → seeded_wallet_and_market_state_appear_at_session_start FAILED(지갑 0) → 원복
e4514fdc7bf0281a9bf6ec235af9271fd8ad92ea8ef5f596c5046c1053114ff7 *bins/game-server/src/main.rs   # 동일
```

### 게이트

```
cargo fmt --all --check                                  # 통과
cargo clippy --workspace --all-targets -- -D warnings     # 통과, 경고 0
cargo test -p starfall-game-server --locked                # 57 passed(신규 테스트 1건 포함)
cargo test --workspace --locked --no-fail-fast              # FAILED 0건
```

qa3 에게 수정 알렸다 — 같은 `restart_cases.py --out <dir> sell-dup`·`sell-cas` 실서버 재현으로 재확인 요청.

## 알려진 한계

- `/debug/stats`의 markets·deposit_reserves 필드는 **기동 적재 결과만** 보인다 — 판매로 바뀌는 시장 상태(포화·마지막 거래)는 `MARKET_STATE` 메시지로만 전파된다(DB 재기동 복원은 S4 가 이미 `restart_preserves_saturation_and_last_sale`로 덮었다).
- `sell_mineral_rejection_counter_*`는 **플럼빙만** 본다(Q-2 답변 그대로) — 사유 6·7 이 실제로 그 조건에서 나는지는 `starfall-sim`의 `sale_rejection_reason_6_*`·`sale_rejection_reason_7_*`가 덮는다. 실서버 관측(`/debug/stats` 델타)은 qa3 가 실서버 SC-107~111 쪽에서 본다(§11 Q-2).
- `trading_replay.rs`(SC-40)는 시장 하나·광물 하나·함선 둘로 거부 사유 넷(쿨다운·도킹 밖·속도·보유 부족)·수락 둘·구간 걸침·회복 배수를 재며, 두 번째 광물·같은 tick 동시 판매 교차 같은 그 이상의 복잡한 상태는 `simulation.rs`의 단위 테스트(`sale_same_tick_submission_order_*`·`market_listing_appears_on_first_extraction_*` 등)가 이미 덮는다 — 재생의 목적은 "경제 판정이 두 프로세스에서 바이트 단위로 같은가"이지 판정 경로 전체 커버리지가 아니다(`mining_replay.rs`와 같은 범위 설계).

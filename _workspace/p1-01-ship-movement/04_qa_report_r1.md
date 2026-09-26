# p1-01-ship-movement — QA 평가 리포트 라운드 1

- 작성: qa, 2026-09-20
- 계약: `_workspace/p1-01-ship-movement/02_sprint_contract.md` (SC-01~86, M-1~M-17) — **확정본, 이 리포트는 이 표의 항목으로만 판정한다**
- 증거 원문: `_workspace/p1-01-ship-movement/evidence/SC-*.log` (요약본이 아니라 명령 출력 그대로)
- **이번 라운드의 범위: 블록 0·1·2 + 실서버가 필요 없는 추가 항목(SC-82·83·84·86).** 블록 3 이후(실서버·Unity PlayMode·부하)는 다음 턴이다.

> **인수인계 사실.** 이 슬라이스의 계약과 QA 도구(`tools/bots/` 확장, `tests/e2e/` 확장)는 Phase 3~4 에서
> **다른 qa 에이전트**가 만들었고 그 에이전트는 맥락 한도로 재개 불가가 됐다. 평가는 **이 리포트가 처음**이다.
> `evidence/` 디렉토리도 이 리포트를 쓰면서 처음 만들었다 — 그래서 `03_client_impl.md` 가 참조하는
> `evidence/SC-41-step{1,2,3}-*.log` 같은 경로는 **존재하지 않는다**(§0.6 참조).

---

## 0. 실행 전제

### 0.1 환경 (M-15)

| 항목 | 값 |
|------|-----|
| 시각 | 2026-09-20 21:36 ~ (KST) |
| cargo | 1.98.1 (797e8a9bc 2026-08-05) |
| dotnet | 10.0.401 |
| python | 3.12.0 (`jsonschema` 4.23.0) |
| unity CLI | 1.0.0-beta.8 |
| Unity Editor | 대화형 인스턴스 **없음**(이번 턴은 `unity test` CLI 만) |
| 컨테이너 | `starfall-postgres-1` Up 26h healthy, `starfall-redis-1` Up 31h healthy + **타 프로젝트 10개**(`livingfeed-*` 5, `aether-*` 3 관측, 총 10) |
| 서버 빌드 프로필 | dev (`cargo test`/`cargo clippy` 기본) |
| 봇 시드 | 이번 턴은 실서버 봇 실행 없음 (블록 3 이후) |

**성능 측정은 이번 턴에 하지 않았다.** 이번 범위는 대부분이 빌드이고, 게이트 G-c(측정 중 빌드 금지)가
빌드와 측정을 같은 시간에 돌리지 못하게 한다. M-1~M-6 은 블록 8 몫이다.

### 0.2 G-a 결정 — **`docker compose down -v` 를 쓰지 않는다**

**리더 결정(2026-09-20)을 그대로 적용한다. SC-80 은 이번 실행의 tick 구간 한정으로 판정한다.** 근거 세 가지:

1. p0-02 의 `QA_APPEND_ONLY_PROBE` 행이 슬라이스를 넘어 살아 있다는 것 자체가 **절대 원칙 5(과거 기록은 수정하지 않는다)의 살아있는 증거**다. 지우면 그 연속성이 사라진다.
2. SC-80 이 묻는 것은 "**이번 슬라이스의 서버**가 위치 시계열을 쓰는가"다. tick 구간 한정은 범위를 좁히는 타협이 아니라 **질문에 맞는 범위**다.
3. 되돌릴 수 없는 삭제를, 한 항목의 판정 범위를 넓히자고 할 값이 아니다.

인프라는 이미 떠 있는 것을 그대로 쓴다. **이번 턴에 `docker compose down` 을 어떤 형태로도 실행하지 않았다.**
SC-80 자체는 실서버가 필요하므로 이번 턴 범위 밖이고, 다음 턴에 tick 구간 한정으로 판정한다.

### 0.3 §0.10 — **이 슬라이스가 증명하지 못하는 것** (요약에 싣도록 계약이 요구한 문장)

> **손 방향 부호가 통째로 뒤집혀도 자동 검증은 전부 통과한다.** 서버와 클라이언트가 같은 공식을 쓰므로
> 예측 오차 0, fixture 왕복 통과, 결정성 바이트 동일까지 전부 초록이다. 특히 오토레벨의 `sin_err` 부호
> 한 줄이 그 위험을 진다(ADR-0010 §2.1). **검출기는 SC-59(AC-14 a·d)의 육안 관찰 하나뿐이고,
> 그것을 사람이 실제로 볼 때까지 이 슬라이스는 부호에 대해 아무것도 증명하지 못한다.**

**이번 라운드에서 SC-59 는 실행되지 않았다 → M-14: 부호 미검증.** 이번 턴에 초록불이 난 68건은
전부 이 한계 위에 있다.

### 0.4 도구 자체 검증의 숫자는 구현 판정에 넣지 않는다

SC-85·SC-86 은 **QA 도구가 고장을 잡는가**를 보는 항목이다. 거기서 나온 수치(예: `bandwidth --selftest`
의 161.78 KiB/s, `two_client_view --selftest` 의 1 mm 어긋남 20건)는 **SC-85·SC-86 의 증거**이지
구현 판정이 아니다. 평가 집계에 넣지 않았다.

### 0.5 검사 건수 원칙

계약 §0.3 대로, 순회·집합 항목은 **실제로 순회한 개수**를 함께 적었다. 개수 없는 PASS 는 없다.

### 0.6 구현자 증거 경로 중 존재하지 않는 것

`03_client_impl.md` §7 이 SC-41 의 증거로 `evidence/SC-41-step{1,2,3}-*.log` 를 지목하는데,
`_workspace/p1-01-ship-movement/evidence/` 디렉토리는 **qa 가 2026-09-20 21:36 에 처음 만들었다**.
세 파일은 없다. 임팩트는 SC-41 항에 적었다(1단은 qa 가 독립 재현했다).

---

## 1. 블록 0 — 계약·빌드 (서버 불필요)

### 1.1 집계

| | 항목 수 | ID |
|---|---|---|
| PASS | 20 | SC-01·02·03·04, SC-35·36·37·38·39·40, SC-41·42·43·44·45, SC-47·48·49·50, SC-85 |
| **FAIL** | **1** | **SC-46**(EditMode 종료 코드 8) |
| 미검증(환경) | 0 | — |

*(SC-40 은 (e)/(f) 로 갈라 보았으나 둘 다 PASS 라 한 항목으로 세었다. SC-49 는 c·d·e 를 한 항목으로 세었다.)*

### 1.2 A절 — 서버 게이트와 결정성 위생

| ID | 판정 | 증거 |
|----|------|------|
| SC-01 | **PASS** | `cargo fmt --all --check` → **exit 0** (`evidence/SC-01-fmt.log`). `cargo clippy --workspace --all-targets -- -D warnings` → **exit 0**, 경고 0 (`SC-01-clippy.log`). `cargo test --workspace --locked` **3회 연속** → 매회 **바이너리 13개 / 152 passed / 0 failed / 4 ignored / exit 0** (43s·39s·38s, `SC-01-test-run{1,2,3}.log`) |
| SC-02 | **PASS** | `grep -nE "axum\|sqlx\|redis\|rand\|chrono\|tokio\|hash\|glam\|nalgebra" server/crates/sim/Cargo.toml` → **매칭 0 (exit 1)**. `[dependencies]` 전문 = `starfall-contracts.workspace = true` **한 줄뿐**. `[dev-dependencies]` 에 `serde`·`serde_json` 이 있으나 S6 결정성 재생 전용이고 수학·해시 크레이트가 아니다 (`SC-02-sim-deps.log`) |
| SC-03 | **PASS** | 순진한 grep **93건**(전부 `.expect(`·`still_settled_since`·`expected` 류 오탐) → ADR-0010 §3 원문의 정밀 grep(메서드 호출 형태) **0건, exit 1**. 금지 목록에 `mul_add`·`hypot`·`to_radians`/`to_degrees`·`signum` 포함 확인 (`SC-03-forbidden-fns.log`) |
| SC-04 | **PASS** | `SystemTime`/`Instant` **실사용 0건**(`crates/sim/src/lib.rs:33` 주석 1건뿐). `HashMap` **0건**. `HashSet` 1건은 `session.rs:54` 의 `seen` 이고 `insert`(85행)·`remove`(92행)뿐 — **순회 지점이 없어 순서 의존이 없다**. 함선 순회는 전부 `BTreeMap<UuidV7,_>` 경유: `simulation.rs:367`·`535`·`620`·`834`·`978` (`SC-04-determinism-hygiene.log`) |

**간헐 실패 재확인 결과 (리더 지시).** 과거 워크스페이스 전체 병렬 실행에서 5회 중 2회 실패하던 `sc20` 은
**3회 연속 전부 통과**했다(`sc20_tick_command_cap_drops_excess_without_closing`). 같이 회귀했던
`sc22_slow_consumer_is_closed_with_slow_consumer_reason`(30초 대기 포함, `ws_integration` 바이너리가 매회 32.2초)와
`sc30_sc31_stats_identities_hold_at_rest`, 대체 단위 테스트
`ws::tests::too_many_in_flight_is_reachable_when_ticks_are_injected_without_draining_it` 도 **3/3 통과**.
**이번 라운드에서 간헐 실패는 한 건도 관측되지 않았다.**

### 1.3 F절 — 계약과 Rust (`cargo test -p starfall-contracts --locked -- --nocapture`, exit 0)

| ID | 판정 | 검사 건수와 증거 |
|----|------|----------------|
| SC-35 | **PASS** | 테스트 **10건 전부 통과**. 출력에 찍힌 건수: **스키마 18 / 유효 fixture 26 / 반례 34 / 레지스트리 타입 13**. `registry_consistency`·`schema_ids_match_paths`·`registry_file_validates_against_schema`·`integer_bounds_rejected`(정수 경계 5건) 포함 (`SC-35-40-contracts.log`) |
| SC-36 | **PASS** | `fixtures_roundtrip` → `[SC-13] 왕복 검증한 유효 fixture: 26건` + 26개 파일명. **kind 별 분해가 서버 출력에 없어 qa 가 레지스트리로 산출**: 와이어(command 4 / server_message 8 / domain_event 8) = **20건**, data = **6건**, 합 26 (`SC-36-kind-split.log`). 비교 규칙은 계약이 요구한 "data 만 as_f64 정규화" 가 아니라 **전 kind 엄격 `Value` 비교**다(`contract_tests.rs:330`) — 계약보다 **강한** 규칙이라 PASS 로 둔다 |
| SC-37 | **PASS** | `invalid_rejected_by_schema` → `[SC-14] 스키마가 거부한 반례: **34건**` + 34개 파일명, 전부 거부 |
| SC-38 | **PASS** | `invalid_serde_matrix` → `[SC-15] serde 매트릭스 검사한 반례: **34건**`. 출력에 **34행** 전부 `기대=거부 / 실제=거부`(`기대=통과` 행 0). 스펙 §5.4 의 "Rust serde" 열(표에 오른 18행 전부 거부)과 **일치**. 보조 grep `serde(flatten)|serde(tag *=` → **실제 속성 0건**(매칭 4건은 전부 `commands.rs`·`dispatch.rs` 의 "쓰면 안 된다" 주석) |
| SC-39 | **PASS** | `required_field_mutations` → `[SC-18] required 변이 **264건**이 모두 역직렬화에 실패했다` (p0-02 는 124건 → 신규 7타입으로 140건 증가) |
| SC-40(e) | **PASS** | `registry_server_types_mapped` → `[SC-16] server 태그 타입 **13건**이 대응표에 있다` + 13행 이름에서 Rust 타입 대응 |
| SC-40(f) | **PASS** (다른 경로로 관측) | **계약이 겨냥한 구멍은 실제로 막혀 있다.** ① `fixtures_roundtrip` 은 `data` kind 를 `as_f64` 로 정규화하지 **않고** 전 kind 를 **엄격 `Value` 비교**한다(`contract_tests.rs:330~338`) — `serde_json::Value` 는 `Number(10)` 과 `Number(10.0)` 을 다른 값으로 보므로, 스키마가 `integer` 인 필드를 Rust 가 `f64` 로 받으면 **26건 왕복이 그 자리에서 깨진다**. ② SC-83 의 정수 행 **51개 전부**가 Rust 정수 newtype 으로 매핑되고(`PositionMm`=`i64`, `ControlAxisMilli`=`i32`, `InputSeq`=`u32`, `Tick`=`u64` …) **`f64` 로 받는 정수 필드가 0개**다(`SC-83-interface-matrix.json`, exit 0). **단, `contract_tests.rs` 에 `10.0` 거부를 직접 단언하는 테스트는 없다** — 지금은 성질이 qa 의 외부 스크립트로만 자기 방어된다. 권고는 §6.1 |

### 1.4 G절 — 생성기 확장과 EditMode (client)

| ID | 판정 | 증거 |
|----|------|------|
| SC-41 | **PASS** (1단 qa 독립 재현, 2·3단은 구현자 기록) | qa 가 `git show HEAD:tools/codegen/ContractsCodegen.cs` 로 확장 전 생성기를 현재 `contracts/` 에 돌려 **첫 실패가 `array` 가 아니라 `maxItems` 임을 재현**: `codegen: contracts/messages/WORLD_SNAPSHOT.schema.json/properties/payload/properties/ships/maxItems: unsupported JSON Schema keyword`, **exit 2** (`SC-41-step1-reproduced.log`). 2·3단(`array`, `kind 'data'`)은 `03_client_impl.md` 의 표 기록으로만 있다 — 중간 버전 생성기가 남아 있지 않아 재현 불가 |
| SC-42 | **PASS** | qa 가 **임시 디렉토리에 독립 재생성**(client 파일을 건드리지 않았다). 1회차 11파일 / 2회차 11파일 → `diff -r gen1 gen2` **차이 0**. **gen1 의 11개 `.cs` 가 client 제출본과 SHA-256 전부 일치**(`SAME` 11/11). 실제 디렉토리에 `--check` → `--check: up to date (11 file(s))`, **exit 0** (`SC-42-qa-regen-run{1,2}.log`, `SC-42-qa-check.log`, `SC-42-44-byte-identity.log`) |
| SC-43 | **PASS** | `WorldSnapshotMessage.cs:56` → `public ShipState[] Ships { get; set; }` (**배열**). 클래스 중첩: `WorldSnapshotMessage`(11행) > `WorldSnapshotPayload`(40행) > `ShipState`(71행). **`ShipState` JsonProperty = 18개**(`angular_velocity_roll_mdeg_s` 포함, 전체 목록은 증거 파일). 왕복 실증: `WorldSnapshot_TwoShipsOneLingering_RoundTrips` Passed (`SC-43-shipstate.log`) |
| SC-44 | **PASS** | stdout 3줄: `skipped SHIP_CLASS (kind 'data': no DTO is generated)` 외 `STAR_SYSTEM`·`SYNC_TUNING`. 생성 디렉토리에 `ShipClass(Table).cs`·`StarSystem(Table).cs`·`SyncTuning(Table).cs` **6개 후보 전부 부재** 확인 |
| SC-45 | **PASS** | `git diff --stat HEAD -- <Generated>` → **`ContractTypes.cs` 1 file changed, 8 insertions(+), 0 deletions**. 기존 6타입(`PingServerCommand`·`PingReplyMessage`·`CommandResultMessage`·`SessionReadyMessage`·`SessionOpenedEvent`·`SessionClosedEvent`) **전부 0줄 변경**. +8줄은 타입 맵 4행 + schema_version 맵 4행 (`SC-45-codegen-diff.log`) |
| **SC-46** | **FAIL** | **종료 코드 0이 아니다 — `unity test` exit 8 (Unity 프로세스 코드 2).** 상세와 담당은 §5.2 |
| SC-47 | **PASS** | `ContractFixtureTests.Fixtures_RoundTrip_Found26_RoundTripped20` Passed. **발견 26 / 왕복 20**, qa 산출 kind 분해(§1.3 SC-36)와 일치 |
| SC-48 | **PASS** | `CSharpLayerSplit_Totals34` 출력 **`reject 18 / accept 10 / no layer 6 = 34`** — **계약 §0.5 표와 정확히 일치**. 뒷받침: `Invalid_Rejected_VisitedEveryCSharpCase`(18), `NotDetectable_ListCoversExactlyTen`(10), `Invalid_NoCSharpLayer_HasNoEnvelopeDiscriminator` **TestCaseSource 6건**(SHIP_CLASS 2·STAR_SYSTEM 2·SYNC_TUNING 2, 각각 `has envelope discriminator: False`) |
| SC-49 | **PASS** | (c) `FixtureLoader_FailsWhenTooFewValidFixtures` — `guard sees 25 of 26 expected fixtures` (d) `WorldSnapshot_EmptyShipsArray_RoundTrips`(`ships.Length == 0`) + `WorldSnapshot_TwoShipsOneLingering_RoundTrips`(`ACTIVE`/`LINGERING`) (e) `Runtime_ToleratesUnknownFieldInsideShipStateArrayElement_AndStrictThrows` — `ignored member at payload.ships[0].warp_charge_percent` / `Strict rejected: Could not find member 'warp_charge_percent' on ShipState` |
| SC-50 | **PASS** | client 테스트 `ClientDataCopy_MatchesRepositoryOriginal` Passed **+ qa 독립 SHA-256 대조**(`SC-50-data-copy.log`): 3파일 전부 동일 해시(`sync-tuning` 2583B, `scout-s01` 1938B, `cradle` 4179B), 레포 `data/` 3 = 사본 3, **누락 0** |

### 1.5 SC-85 — QA 봇 하네스 자체 검증

**PASS.** `cd tools/bots && cargo test` → **바이너리 8개 / 49 passed / 0 failed / exit 0**,
`cargo fmt --all --check` exit 0 (`evidence/SC-85-bots-selftest.log`).

계약 §3.3 의 고장 주입 9종이 **전부 테스트 이름으로 존재하고 통과**한다:

| # | 주입 | 테스트 | 결과 |
|---|------|--------|------|
| 1 | 스냅샷 1건 누락 | `dropped_snapshot_shows_up_as_an_interval_violation` | ok |
| 2 | `ships` 내림차순 | `descending_ship_order_is_detected` | ok |
| 3 | `ack_input_seq` 되돌림 | `ack_rewind_is_detected` | ok |
| 4 | 위치 1 mm 어긋남 | (e2e 쪽) `two_client_view.py --selftest` → 불일치 **20건** 검출 | ok |
| 5 | 프레임 바이트 부풀림 | `byte_accounting_follows_the_wire_not_a_metric` | ok |
| 6 | `presence` 고정 | `presence_values_are_recorded_so_a_frozen_value_is_visible` | ok |
| 7 | 간격 3 tick | `interval_mismatch_between_declared_and_actual_is_detected` | ok |
| 8 | 짝짓기 키를 `correlation_id` 로 회귀 | (e2e 쪽, SC-86) `ship_events.py selftest` | ok |
| 9 | CSV 왕복 | `csv_preserves_the_exact_quantised_integers` | ok |
| — | 건강한 입력의 대조군 | `healthy_snapshots_pass_every_gate`, `missing_controlled_ship_is_detected` | ok |

**이 수치는 SC-85 의 증거이지 구현 판정이 아니다**(§0.4).

---

## 2. 블록 2 — 메모리 적분과 결정성 (SC-15~22, SC-34)

전부 `Simulation::new(world, 0)` / `step()` 수준의 메모리 테스트다(게이트 G-g). 실서버 `last_tick = 350280` 과 무관하다.

명령: `cargo test -p starfall-sim --locked -- --nocapture --test-threads=1` → **42 passed / 0 failed / exit 0** (`evidence/SC-15-22-sim.log`)
+ `cargo test -p starfall-sim --test determinism -- --nocapture` → **1 passed / 1 ignored / exit 0** (`evidence/SC-34-determinism.log`)

### 2.1 집계

| | 항목 수 | ID |
|---|---|---|
| PASS | 7 | SC-15·16·17·20·21·22, SC-34 |
| **FAIL** | **2** | **SC-18**(soft 경계 절반 미단언), **SC-19**(이월 만료 미단언) |

### 2.2 항목별

| ID | 판정 | 관측값과 증거 |
|----|------|-------------|
| SC-15 | **PASS** | `world::integrate::tests::forward_thrust_only_moves_z_and_matches_hand_calculation` → `[SC-15] 20 tick 후 p.z = 18.375000000000004 m (양자화 **18375 mm**), 손계산 기대 18375 mm`. **z 만 증가**도 단언한다(`integrate.rs:353·358`: `p.x.abs() < 1e-9`, `p.y.abs() < 1e-9`). **기대 숫자 출처는 `contracts/fixtures/SHIP_CLASS/example-scout.json`**(`integrate.rs:283` 의 `scout()` 헬퍼) — 게이트 G-b 준수. C3(클라이언트)와 같은 정수 18375 를 쓴다 |
| SC-16 | **PASS** | `speed_never_exceeds_max_speed` → `[AC-4b] **2000 tick** 후 도달 최대 속도 = **140 m/s** (상한 140)` — 상한에 붙되 넘지 않는다 |
| SC-17 | **PASS** | `diagonal_thrust_is_clamped_to_main_thrust_magnitude` → `[AC-4c] 대각선 가속 크기 = **35 m/s²** (상한 35)`. 클램프가 없으면 `√3 × 35 ≈ 60.6` 이 나온다 |
| **SC-18** | **FAIL** | **hard 절반만 단언됐다.** `hard_boundary_removes_only_radial_velocity`(`integrate.rs:440`)는 `r ≤ 12000 + 1e-6`, 반경 속도 `≤ 1e-6`, 접선 `v.z = 49.82` 보존을 단언한다 — 여기까지는 통과다. **soft 절반("soft 경계를 넘으면 원점 방향 가속이 더해지고 **조작은 계속 먹으며**")을 단언하는 테스트가 없다.** 상세와 담당은 §5.3 |
| **SC-19** | **FAIL** | **이월 만료가 단언되지 않았다.** `coasting_decays_gradually_not_instantly`(`integrate.rs:468`)는 `step(state, &no_input(), …)` 를 **직접** 부른다 — 즉 만료 *이후* 상태를 손으로 만들어 넣고 감쇠만 본다(`[AC-4e] 1 tick 후 속도 = 99.65 (시작 100)`). **`carry_forward_max_ticks` 경과가 실제로 휴면 입력으로 전환되는지**(`simulation.rs:846~848`)를 보는 테스트가 없고, 계약이 요구한 "속도 곡선"도 없다. 상세와 담당은 §5.4 |
| SC-20 | **PASS** | `brake_ignores_thrust_and_applies_single_damping` → `[AC-4f] 브레이크 1 tick 후 속도 = **97.5** (기대 97.5)`, 오차 `< 1e-9`. 추력 `(0,0,1)` 을 넣은 채 `brake=true` 로 눌러도 결과가 `100 − brake_decel × dt` 정확히 하나 — **감쇠가 이중으로 걸리지 않는다** |
| SC-21 | **PASS** | `attitude_settles_without_overshoot` → **안착 tick 수 = 84**(M-12). 루프 안에서 **매 tick** `\|ω_aim\| ≤ turn_rate_max_deg_s` 를 단언하고, 안착 후 **40 tick** 재상승 없음을 단언한다. (g2) `auto_level_does_not_oscillate_when_holding_attitude` → `[AC-4g2] **200 tick** 동안 최대 \|ω_roll\| = **0**` |
| SC-22 | **PASS** | `degenerate_aim_keeps_current_attitude_and_is_flagged`(`integrate.rs:601`) — 전 성분 0 쿼터니언이 **거부되지 않고** `outcome.aim_degenerate == true`, `\|ω_aim\| < 1e-9`, 자세 표류 `< 1e-6`. **카운터 배선 확인**: `simulation.rs:859~860` → `gateway/runtime.rs:426 stats.add_aim_degenerate(...)` → `stats.rs:210 aim_degenerate_total`. **단, `aim_degenerate_total` 의 실제 델타는 이번 라운드에 관측하지 않았다**(실서버 필요) — 블록 3 SC-33 에서 측정한다. M-17 에 "카운터 델타 미관측"으로 적었다 |
| SC-34 | **PASS** | `ac8_two_process_replay_produces_byte_identical_snapshots` → `[AC-8/SC-34] **비교한 스냅샷 줄 수 = 300**, snapshots.jsonl **총 바이트 = 525272** (두 프로세스 동일)`. 근사 비교 아님. 산출물 3파일은 `server/crates/sim/tests/data/replay/{initial.json, inputs.jsonl, snapshots.jsonl}` 에 실재한다(2041 B / 2708 B / 525272 B) → **M-13** |

### 2.3 테스트 이름의 번호 충돌 — 리포트를 읽을 때 반드시 알아야 한다

`crates/gateway` 의 테스트 이름 `sc20_tick_command_cap_drops_excess_without_closing`,
`sc22_slow_consumer_is_closed_with_slow_consumer_reason`, `sc30_sc31_stats_identities_hold_at_rest` 는
**p0-02 의 SC 번호**다. p1-01 계약의 SC-20(브레이크)·SC-22(퇴화 쿼터니언)·SC-30/31(스냅샷 규약)과 **번호가 겹치지만 다른 항목**이다.

| 테스트 이름 | p0-02 번호 | **p1-01 계약에서 대응하는 항목** |
|---|---|---|
| `sc20_tick_command_cap_drops_excess_without_closing` | p0-02 SC-20 | **p1-01 SC-26**(손실 항등식, ADR-0011 §5.2) — 블록 8 |
| `sc22_slow_consumer_is_closed_with_slow_consumer_reason` | p0-02 SC-22 | p1-01 범위 밖(p0-02 회귀 방지) |
| `sc30_sc31_stats_identities_hold_at_rest` | p0-02 SC-30/31 | p1-01 SC-33 의 교차 검증에 인접 — 블록 3 |

**p1-01 SC-20 은 브레이크(AC-4 f)이고 위 표대로 PASS 다.** 두 번호 체계를 섞으면 "SC-20 이 통과했다"가
서로 다른 두 사실을 가리킨다 — 다음 라운드와 리더 보고에서 이 표를 근거로 읽어야 한다.

---

## 3. 블록 1 — 데이터 테이블과 기동 거부 (SC-05·06·07)

**`docker compose down -v` 는 쓰지 않았다**(§0.2). 이미 떠 있던 `starfall-postgres-1`·`starfall-redis-1` 을 그대로 썼고,
이 블록은 DB 를 읽지 않는다(서버가 마이그레이션 확인과 tick 이어받기만 한다).

도구: `tests/e2e/server_boot.py`(qa 가 이번 라운드에 새로 씀 — `stats` / `reject` 두 하위 명령).
서버는 **stdin `shutdown`** 으로만 내린다(하드 킬 금지, 계약 §0.1).

### 3.1 집계 — **3/3 PASS**

### 3.2 SC-05 — 로드된 데이터가 `/debug/stats` 에 드러나는가

**PASS.** `python tests/e2e/server_boot.py stats` → `/debug/stats` 수신, stdin `shutdown` 후 **종료 코드 0**
(`evidence/SC-05-debug-stats.log`).

| `/debug/stats` 키 | 관측값 | **그 시점 `data/` 실제 값** (계약 §0.7) | 일치 |
|---|---|---|---|
| `data_dir` | `\\?\C:\WorkSpace\SpaceHistoric\data` | 레포 `data/` | ✔ |
| `ship_classes_loaded` | **1** | `data/ships/*.json` = 1개 (`scout-s01.json`) | ✔ |
| `spawn_points_loaded` | **12** | `cradle.json` 의 `point_count = 12`, `len(points_m) = 12` | ✔ |
| `snapshot_interval_ticks` | **2** | `worlds.tick_hz = 20` ÷ `sync-tuning.snapshot_hz = 10` = 2 | ✔ |

기동 로그도 같은 값을 찍는다:
`게임 데이터 로딩 완료 data_dir=… ship_classes=1 spawn_points=12 snapshot_interval_ticks=2`.
참고로 `hard_boundary_radius_m = 12000.0`, `soft = 10000.0` 이다(실서버 관측 해석에 쓸 값이라 함께 적는다).

**부수 관측(블록 3 의 SC-33 예고).** 접속 0건 상태에서 신규 7키가 **전부 존재**하고 0 으로 초기화돼 있다:
`snapshots_sent_total`·`snapshot_bytes_total`·`send_queue_bytes`·`send_queue_bytes_max`·`input_superseded_total`·`input_carried_forward_total`·`aim_degenerate_total`.
`ships_active`·`ships_lingering` 게이지 2종, `commands_rejected_total` **8라벨**
(`MALFORMED_COMMAND`·`UNKNOWN_COMMAND_TYPE`·`SCHEMA_VERSION_UNSUPPORTED`·`DUPLICATE_COMMAND_ID`·`SERVER_BUSY`·`TOO_MANY_IN_FLIGHT`·**`RATE_LIMITED`**·**`STALE_INPUT`**),
`messages_enqueued_total`/`messages_written_total` **각 4라벨**, 최상위 키 **47개**.
**존재만으로는 SC-33 에 PASS 를 주지 않는다** — 델타는 블록 3 에서 잰다.

**기동 시각의 tick**: `last_tick=Some(350420) → start_tick=350421`, 종료 시 `tick=350429`.
게이트 G-g 대로 **tick 0 기준 손계산 기대값을 실서버에 쓰지 않았다.**

### 3.3 SC-06 — 기동 거부 8종

**PASS. 8종 전부 종료 코드 1 + `기동 거부: 데이터 검증 실패` 로그.** `STARFALL_DATA_DIR` 로 임시 사본을 가리켰고
**레포 `data/` 원본은 건드리지 않았다**(`evidence/SC-06-boot-reject.log`).

| # | 어긴 것 | exit | 로그의 `field= / expected= / actual=` | `rule=` |
|---|---------|------|--------------------------------------|---------|
| ① | `hard_boundary_radius_m = 20000.1` | 1 | `…de_boundary_radius_m 는 (0 .. 20000 범위를 벗어난다 (받음: 20000.1) at line 24 column 37` | **schema** ✔ |
| ② | 스폰 `points_m` 빈 배열 | 1 | `points_m 은 1 ..= 64 개여야 한다 (받음: 0) at line 46 column 18` | **schema** ✔ |
| ③ | `point_count ≠ len(points_m)` | 1 | `field=spawn.point_count expected=points_m.len() (12) 과 같다 actual=13` | **derived** ✔ |
| ④ | `tick_hz / snapshot_hz` 정수 아님 | 1 | `field=snapshot.snapshot_hz expected=tick_hz(20) 을 나누어떨어뜨리는 값 actual=7` | **derived** ✔ |
| ⑤ | `reconnect_resume_window_seconds > linger_seconds` | 1 | `field=presence.reconnect_resume_window_seconds expected=<= linger_seconds (30) actual=35` | **derived** ✔ |
| ⑥ | 스폰 지점이 하드 경계 밖 | 1 | `field=spawn.points_m[0] expected=원점 거리 <= hard_boundary_radius_m (12000) actual=999999.03125… ([999999.0, 250.0, 0.0])` | **derived** ✔ |
| ⑦ | `main_thrust_mps2 < lateral` | 1 | `field=movement.main_thrust_mps2 expected=>= lateral_thrust_mps2 (18) actual=0.1` | **derived** ✔ |
| ⑧ | 함선 클래스 `id` 중복 | 1 | `field=id expected=고유 id (이미 …/ships/scout-s01.json 에서 사용됨) actual=scout-s01` | **derived** ✔ |

**①②③~⑧ 의 `rule=` 배분이 계약과 정확히 일치한다**(①② = `schema`, ③~⑧ = `derived`).

**보조 요구(디렉토리 부재)도 확인했다.** `STARFALL_DATA_DIR=<없는 경로>` →
`ERROR 기동 거부: data/ 디렉토리를 찾지 못했다 — 시도한 경로: C:/WorkSpace/SpaceHistoric/__no_such_data_dir__`, **종료 코드 1**.
**찾은 경로를 로그에 찍고 거부한다**는 요구가 충족된다.

> **형식에 대한 기록(판정 아님).** ①② 는 `field=` / `expected=` / `actual=` 세 토큰 대신
> `error=<serde 메시지>` 한 덩어리로 나온다. **필드 이름·제약·실제 값 세 정보는 그 안에 전부 있고**
> 계약의 검증 항목("로그에 파일·필드·기대값이 나온다")은 충족된다. 토큰 형식을 기계 파싱할 계획이
> 생기면 그때 맞추면 된다 — 지금 FAIL 로 잡을 일이 아니다.

### 3.4 SC-07 — 실제 `data/` 3파일의 스키마 검증 (회귀 확인)

**PASS.** `python tests/e2e/validate_data_files.py` → **검사한 파일 3 / 오류 0 / exit 0**
(`evidence/SC-07-data-schema.log`).

| 파일 | 스키마 | 오류 |
|---|---|---|
| `data/ships/scout-s01.json` | `contracts/data/ship-class.schema.json` | 0 |
| `data/world/systems/cradle.json` | `contracts/data/star-system.schema.json` | 0 |
| `data/movement/sync-tuning.json` | `contracts/data/sync-tuning.schema.json` | 0 |

architect 실측(2026-09-19)의 `sync-tuning.json` **6건 실패는 회귀하지 않았다**(D-1 유지).

**검증기가 켜져 있다는 증거를 함께 남겼다.** 같은 스크립트를 **일부러 어긴 사본**
(`hard_boundary_radius_m = 20000.1`)에 돌리면
`/play_area/hard_boundary_radius_m: 20000.1 is greater than the maximum of 20000` 으로 **오류 1건 / exit 1** 이 난다.
"0건 통과"가 "검증기가 꺼져 있다"와 구분된다(계약 §0.3).

---

## 4. 추가 항목 — 커버리지·경계면·fixture·도구 (SC-82·83·84·86)

실서버가 필요 없고 선행 게이트가 이미 충족돼 이번 턴에 함께 돌렸다.

### 4.1 집계 — **4/4 PASS**

### 4.2 SC-82 — 계약 커버리지 `--strict`

**PASS.** `python .claude/skills/integration-qa/scripts/check_contract_coverage.py --strict` →
**types 13 / errors 0 / warnings 0 / `RESULT: PASS` / exit 0** (`evidence/SC-82-coverage.log`).

**기준선 errors 14 → 0.** 게이트 G-l(S1·C1·Q2 완료 후)이 충족된 뒤 실행했다.
13타입 전부가 `schema ok` + `fixtures 2` + 생산자/소비자 코드 참조를 갖는다. 신규 7타입의 참조 위치:

| 타입 | 코드 참조 |
|---|---|
| `SET_SHIP_CONTROL` | producer:client `ContractTypes.cs` · producer:bots `conn.rs` · consumer:server `commands.rs` |
| `WORLD_SNAPSHOT` | producer:server `commands.rs` · consumer:client `ContractDispatch.cs` · consumer:bots `conn.rs` |
| `SHIP_SPAWNED` | producer/consumer:server `data.rs` |
| `SHIP_DESPAWNED` | producer/consumer:server `events.rs` |
| `SHIP_CLASS`·`STAR_SYSTEM` | consumer:server `bins/game-server/src/data.rs` |
| `SYNC_TUNING` | consumer:server `data.rs` · **consumer:client `Flight/PredictedShipController.cs`** |

### 4.3 SC-83 — 신규 7타입 경계면 3자 **필드별** 비교

**PASS. 대조한 행 201개 / 불일치 0건.** (`tests/e2e/interface_matrix.py` 를 qa 가 이번 라운드에 7타입으로 확장했다.
증거: `evidence/SC-83-interface-matrix.json`(전 행) + `evidence/SC-83-interface-matrix.md`(사람이 읽는 표). exit 0)

**총계가 아니라 표가 근거다**(AC-21 b) — 아래는 그 표의 요약이고, 행 자체는 위 두 파일에 있다.

| 구분 | 행 수 | 불일치 |
|------|------|-------|
| 와이어 4타입 envelope + payload (`SET_SHIP_CONTROL` 16 · `WORLD_SNAPSHOT` 13 · `SHIP_SPAWNED` 23 · `SHIP_DESPAWNED` 18) | **70** | 0 |
| **배열 원소 별도 표** — `ShipState` **18** + `ReferenceMarker` 5 | **23** | 0 |
| 데이터 3타입 (**C# 열 없음이 정상**, AC-10 d) — `SHIP_CLASS` 44 · `STAR_SYSTEM` 38 · `SYNC_TUNING` 26 | **108** | 0 |
| **신규 7타입 합계** | **201** | **0** |
| (회귀) p0-02 4타입 | 48 | 0 |

**`ShipState` 는 18행이다** — `angular_velocity_roll_mdeg_s` 포함. 계약 §2 의 표는 이 칸을 아직 "17필드"로 적고 있다(§6.2 통지 대상).

**정수 행 51개** — 각 언어 타입이 스키마 범위를 손실 없이 담는가:

| 스키마 범위 | Rust | C# | 무손실 |
|---|---|---|---|
| `position_*_mm` ±1e12 | `PositionMm` = `i64` | `long` | ✔ |
| `velocity_*_mm_s` ±1e8 | `VelocityMmPerSecond` = `i32` | `int` | ✔ |
| `angular_velocity_*_mdeg_s` ±3.6e6 | `AngularVelocityMdegPerSecond` = `i32` | `int` | ✔ |
| `orientation_*_micro` ±1e6 | `QuaternionComponentMicro` = `i32` | `int` | ✔ |
| `thrust_*_milli` · `roll_milli` ±1000 | `ControlAxisMilli` = `i32` | `int` | ✔ |
| `input_seq` 1..2³²−1 | `InputSeq` = `u32` | `uint` | ✔ |
| `tick` 0..2⁵³−1 | `Tick` = `u64` | `long` | ✔ |

**Rust 51/51 무손실. C# 37/37 무손실**(나머지 14개는 데이터 3종의 정수 행이라 C# 열이 없다 — 정상).
**정수 필드를 `f64` 로 받는 곳은 0개다** — SC-40(f) 가 겨냥한 구멍의 직접 증거다.

**좁힘 행 4개 전부 통과**:

| 행 | 스키마 | Rust | C# |
|---|---|---|---|
| `SHIP_SPAWNED.actor_id` | required, null 불가 | `UuidV7` (비-`Option`) | `System.Guid` + `Required.Always` |
| `SHIP_SPAWNED.causation_id` | required, null 불가 | `UuidV7` | `System.Guid` + `Required.Always` |
| `SHIP_DESPAWNED.actor_id` | required, null 불가 | `UuidV7` | `System.Guid` + `Required.Always` |
| `SHIP_DESPAWNED.causation_id` | required, null 불가 | `UuidV7` | `System.Guid` + `Required.Always` |

**`causation_id` 좁힘은 이 프로젝트의 첫 사례이고, 생성기 수정 없이 동작한다**(계약 §2 의 예측이 실측으로 확인됐다).
대조군으로 envelope 기본값(`correlation_id`)은 세 곳 모두 널 가능이다:
스키마 `null=True` / Rust `Option<UuidV7>` / C# `System.Guid?` + `Required.AllowNull`.

> **도구가 처음엔 틀렸다 — 총계만 봤으면 server 에게 헛수고 39건을 보낼 뻔했다.**
> 확장 직후 첫 실행은 **불일치 39건 / FAIL** 이었다. 전부 같은 원인이었다:
> 비교 규칙이 "널 가능(null 허용)"과 "선택(키가 없어도 됨)"을 구분하지 않아,
> 데이터 스키마의 **선택 필드를 Rust 가 `Option<T>` 로 받는 정상 표현**을 불일치로 셌다
> (`SHIP_CLASS.designer_note`, `STAR_SYSTEM.spawn.note` 등 39건). 규칙을
> `Option 기대 = nullable **또는** not required`, 그리고 `#[serde(default)]` 를 그 대안으로 인정하도록
> 고친 뒤 0건이 됐다(`STAR_SYSTEM.reference_markers` = `Vec<ReferenceMarker> + #[serde(default)]` 가 마지막 1건이었다).
> **§0.3 의 "검사 건수를 적는다"가 요구하는 것이 이것이다** — 39 라는 숫자가 아니라 39행의 내용을 봐야 진짜 불일치인지 안다.

### 4.4 SC-84 — fixture 의 `world_id` ↔ `tick_hz` (I-19)

**PASS.** `python tests/e2e/fixture_world_scan.py` → **스캔한 유효 fixture 26건 / 위반 0 / exit 0**
(`evidence/SC-84-world-tickhz.log`. 스크립트는 qa 가 이번 라운드에 새로 썼다 — p0-02 는 인라인 명령이었고 재사용 가능한 스크립트가 없었다).

| `world_id` | `tick_hz` | fixture 수 |
|---|---|---|
| `01a0b1c2-3d4e-7f01-8a2b-9c0d1e2f3a4b` (스파이크 월드) | **20** | 1 |
| `01a0b1c2-3d4f-7a02-9b13-0e1f2a3b4c5d` (`tick-zero` 전용) | **1** | 1 |

`world_id` 를 가진 fixture 10건 / `tick_hz` 를 가진 fixture 2건 / 관측된 (world, hz) 쌍 2개.
**한 `world_id` 가 두 `tick_hz` 를 갖는 경우 0건**, 한 파일 안에서 값이 갈라지는 경우 0건.
p0-02 의 B-14 수정(스파이크 월드와 `tick-zero` 월드 분리)이 **그대로 유지되고 있다**.

### 4.5 SC-86 — e2e 스크립트 자체 검증

**PASS.** qa 가 이번 라운드에 **직접 다시 실행**했다(앞 에이전트의 실행 기록이 `evidence/` 에 없었다).
전부 exit 0 (`evidence/SC-86-e2e-selftest.log`).

| 스크립트 | 결과 |
|---|---|
| `ship_events.py selftest` | **§0.6 의 규칙이 실제로 무언가를 막는다는 증거.** 재개 함선(세션 2개 / 스폰 1건) 합성 데이터에서 `ship_id` 짝짓기 = 깨진 그룹 0, **`correlation_id` 짝짓기 = `corr-A`{SPAWNED 1, DESPAWNED 0} + `corr-B`{SPAWNED 0, DESPAWNED 1} 로 깨진다**(기대대로) |
| `two_client_view.py selftest` | 동일 파일 불일치 0 / **1 mm 어긋뜨리면 20건 전부 검출** / 뒤쪽 +28 m, 앞쪽 −28 m 로 **SC-65 의 부호 판정이 살아 있다** |
| `bandwidth.py --selftest` | 합성 31세션 × 10 Hz × 10초 × 16,566 B → **161.78 KiB/s** (ADR-0011 §2 의 161.8 과 일치), 합계 41.08 Mbit/s (ADR 42.5) |
| `check_occurred_at.py --selftest` | 8케이스 / 불일치 0 (`tick_hz` 로 먼저 나누는 순서 포함) |

**이 수치는 SC-86 의 증거이지 구현 판정이 아니다**(§0.4).

---

## 5. M-17 — **신규 경로를 실제로 탔는가** (계약 §7, architect 결정 2026-09-20)

**통과한 테스트 수는 경로가 실행됐다는 증거가 아니다.** server 가 `world_full` 게이트를 만들고도
한 번도 실행한 적이 없었던 것이 이 표의 이유다. 이번 턴 범위(메모리 + 기동 경로)에서 채울 수 있는 만큼 채웠다.

| 신규 경로 | 이번 턴 | 근거 | 남은 것 |
|---|---|---|---|
| **스폰** | **실행됨(메모리)** | `simulation::tests::opening_a_session_spawns_a_ship_caused_by_session_opened`, `spawn_position_matches_a_configured_spawn_point`, `world::spawn::tests::*` 7건 | 실서버 `SHIP_SPAWNED` DB 행 — 블록 3 (SC-08) |
| **재개** | **실행됨(메모리)** | `simulation::tests::reconnect_within_linger_window_resumes_the_same_ship`, `after_resume_input_seq_one_is_accepted` | 실서버 30초 창 — 블록 5 (SC-11) |
| **잔류 만료 디스폰** | **실행됨(메모리)** | `simulation::tests::session_close_lingers_then_expires_into_a_single_despawn` | 실서버 `SHIP_DESPAWNED{LINGER_EXPIRED}` 행 + `causation_id` — 블록 5 (SC-12) |
| **종료 디스폰** | **실행됨(메모리)** + **실서버 스윕은 돌았으나 대상 0** | `simulation::tests::shutdown_despawns_both_active_and_lingering_ships`. 실서버: SC-05 의 stdin `shutdown` 로그 `tick 루프 종료 — SERVER_SHUTDOWN 스윕 후 영속화 flush tick=350430 sessions_closed=0` — **스윕 코드는 탔고 디스폰 대상이 0척이었다** | 함선이 있는 상태의 종료 — 블록 5 (SC-13) |
| **`world_full` 거부** | **실행됨** | `crates/gateway/tests/ws_integration.rs:843 i44_world_full_exempts_a_resuming_actor` — 정원을 채운 뒤 **함선 없는 actor 가 503 `world_full` 을 받는 것**과 **재개 actor 가 면제되는 것**을 둘 다 태운다. 워크스페이스 3회 실행 전부 통과. 순수 판정 함수 단위 테스트 `world_full_rejects_a_new_spawn`(`ws.rs:702`)도 함께 | — (architect 가 지적한 "만들고 한 번도 안 탄" 상태는 **해소됐다**) |
| **경계 hard** | **실행 + 단언됨** | `hard_boundary_removes_only_radial_velocity` — `r ≤ 12000+1e-6`, 반경속도 ≤ 1e-6, 접선 `v.z=49.82` 보존 | 실서버 접촉 — 블록 8 (M-8) |
| **경계 soft** | **실행됨, 단언 없음** | 위 테스트가 `p=11999 > soft 10000` 에서 시작하므로 `integrate.rs:217` 의 `a_bound` 가지가 **돈다**. 그러나 **원점 방향 가속도, "조작이 계속 먹는다"도 단언되지 않는다** | **SC-18 FAIL 의 내용** (§6.3) |
| **브레이크** | **실행 + 단언됨** | `brake_ignores_thrust_and_applies_single_damping` — 97.5 = 100 − brake_decel×dt, 오차 < 1e-9 | 실서버 사용률 — M-7 (블록 8) |
| **퇴화 쿼터니언** | **실행 + 단언됨(플래그)** / **카운터 델타 미관측** | `degenerate_aim_keeps_current_attitude_and_is_flagged` 가 `outcome.aim_degenerate == true` 를 단언. 배선 `simulation.rs:859` → `runtime.rs:426` → `stats.rs:210`. SC-05 기동 직후 `aim_degenerate_total = 0` | `aim_degenerate_total` 의 **델타** — 블록 3/4 (SC-33, SC-24) |
| **이월(carry forward)** | **미확인** | `simulation.rs:846~848` 이 그 가지다. `input_carried_forward` 를 **단언하거나 출력하는 테스트가 없다**. SC-05 기동 직후 값은 0(함선 0척이라 당연) | **SC-19 FAIL 의 내용** (§6.4). 실서버 델타는 블록 3 |
| **이월 만료** | **미확인** | 같은 가지의 `else` 쪽(휴면 입력으로 전환). 잔류 테스트가 cap(10)을 훨씬 넘겨 돌리므로 **탔을 가능성이 높지만, 카운터도 단언도 없어 확인할 수단이 없다** | 같음 |
| **tick 상한 초과** (`commands_dropped_over_tick_cap_total`) | **실행됨** | `crates/gateway/tests/ws_integration.rs sc20_tick_command_cap_drops_excess_without_closing`(p0-02 번호, **p1-01 계약에서는 SC-26**) — 손실 항등식을 서버 `/debug/stats` 로 교차 검증하며 3회 실행 전부 통과. sim 쪽 `simulation::tests::commands_beyond_the_per_tick_cap_are_rate_limited` | 부하 중 실측 — 블록 8 (SC-26) |
| **`TOO_MANY_IN_FLIGHT`** | **실행됨(주입 단위 테스트)** | `ws::tests::too_many_in_flight_is_reachable_when_ticks_are_injected_without_draining_it` 3/3 통과. 정상 부하에서는 **구조적 미도달**이 설계다(계약 D절 머리말) | 부하에서 `== 0` 기록 — 블록 8 |

---

## 6. FAIL 상세 — 담당자별 수정 요청

### 6.1 server — SC-40(f) 는 PASS 지만 **권고 1건**

**판정은 PASS 다**(§1.3). 다만 성질이 **qa 의 외부 스크립트로만 자기 방어된다**:
지금 `10.0` 을 막는 것은 (a) `fixtures_roundtrip` 의 엄격 `Value` 비교가 **우연히** 덮는 범위와
(b) `tests/e2e/interface_matrix.py` 의 정적 타입 대조뿐이다.
`contract_tests.rs` 에 **직접 단언 한 건**을 넣으면 계약 스위트가 스스로 지킨다:

```
유효 fixture 의 integer 선언 필드에 같은 값을 소수 형태(10 → 10.0)로 넣고
(contract.round_trip)(&mutated).is_err() 를 단언 — 검사한 필드 수를 출력에 찍는다
```

- 파일: `server/crates/contracts/tests/contract_tests.rs` (테스트 9 `integer_bounds_rejected`, 796행 옆)
- 지금 있는 것: 상·하한 5건(`tick` 2 + `probe_seq` 3). **정수/소수 구분은 없다.**

### 6.2 architect — **계약·스펙 문서 통지 2건** (FAIL 아님, §7 규칙)

계약 §7 의 "§0.5 표와 결과가 다르면 FAIL 이 아니라 architect 통지"에 해당한다. **표를 고치지 않았다.**

| # | 문서 | 지금 적힌 값 | 실측 | 비고 |
|---|------|------------|------|------|
| 1 | **계약 §2** 의 `WORLD_SNAPSHOT` 행 | "`ShipState` **17필드** 별도 표" | **18필드** | B-1 의 `angular_velocity_roll_mdeg_s` 가 반영되지 않은 칸이다. 같은 계약의 **SC-43 은 18 을 요구**하고("17로 두면 반드시 실패한다") **§0.5 도 18/10/6 으로 갱신돼 있다** — §2 의 이 칸만 갱신에서 빠졌다 |
| 2 | **스펙 §5.4** 머리말 | "C# 거부 **17** / 통과 10 / 계층 없음 **7** = 34" | **18 / 10 / 6** | 계약 §0.5 가 이미 정정 사유(34번째 반례 `angular-velocity-roll-missing.json` 는 데이터가 아니라 메시지 타입이라 C# 이 거부한다)와 함께 18/10/6 으로 적고 있고, **client 의 `CSharpLayerSplit_Totals34` 가 `reject 18 / accept 10 / no layer 6` 으로 통과한다**. 스펙 본문의 18행짜리 표 자체는 옳다 — 머리말 숫자만 옛값이다 |

**둘 다 구현은 새 값이 맞고 문서만 옛값이다.** 어느 쪽을 정본으로 둘지는 architect 가 정한다.

### 6.3 server — **SC-18 FAIL**: soft 경계의 절반이 단언되지 않았다

- **계약이 요구한 관찰**: "soft 경계를 넘으면 원점 방향 가속이 **더해지고 조작은 계속 먹으며**, hard 경계에서 반경 속도 성분이 0 이 되고 접선 성분은 남는다" (AC-4 d)
- **지금 있는 것**: `server/crates/sim/src/world/integrate.rs:440 hard_boundary_removes_only_radial_velocity` — **hard 쪽만** 단언한다. 이 테스트의 doc 주석(437행)은 "soft 경계를 넘으면 원점 방향 가속이 더해지고…"라고 적혀 있지만 **본문에 그 단언이 없다.**
- **실제로는 그 가지가 돈다**: 시작 위치 `p=(11999,0,0)` 은 `soft = 10000` 밖이라 `integrate.rs:217` 의 `a_bound` 가 계산된다. **돌기만 하고 아무도 보지 않는다.**
- **재현**: `cd server && cargo test -p starfall-sim --lib world::integrate::tests::hard_boundary_removes_only_radial_velocity -- --nocapture` → 출력은 `[AC-4d] 경계 접촉 후 r=… v=…` 한 줄뿐이고 soft 에 대한 수치가 없다.
- **요청(작다)**: soft 와 hard 를 **두 테스트로 나누고** soft 쪽에
  (a) soft 밖에서 가속 벡터가 **원점 방향 성분을 갖는다**,
  (b) **같은 tick 에 추력 입력을 주면 그 추력이 여전히 반영된다**("조작은 계속 먹는다")
  를 수치로 단언한다. **(b) 가 이 항목의 핵심이다** — 경계가 조작을 빼앗지 않는다는 것이 게임 규칙이다.

### 6.4 server — **SC-19 FAIL**: 이월 만료가 단언되지 않았다

- **계약이 요구한 관찰**: "조작을 멈추고 **이월이 만료되면** 가속이 0 이 되지만 속도는 감쇠만 적용되어 즉시 0 이 되지 않는다". **검증 방법: `carry_forward_max_ticks` 경과 후 속도 곡선** (AC-4 e)
- **지금 있는 것**: `server/crates/sim/src/world/integrate.rs:468 coasting_decays_gradually_not_instantly` 는
  `step(state, &no_input(), …)` 를 **직접** 부른다. 즉 **만료 이후의 상태를 손으로 만들어 넣고** 1 tick 감쇠만 본다
  (`[AC-4e] 1 tick 후 속도 = 99.65 (시작 100)`). **이월 메커니즘 자체(`crates/sim/src/simulation.rs:846~848`)를 건너뛴다.**
- **그래서 M-17 의 "이월"·"이월 만료" 두 줄이 이번 라운드에 채워지지 않았다** — `input_carried_forward` 를
  단언하거나 출력하는 테스트가 하나도 없어서 **탔는지 확인할 수단이 없다.**
- **재현**: `grep -rn "input_carried_forward" server/crates/sim server/crates/gateway --include=*.rs | grep -i assert` → **0건**.
- **요청(작다)**: `Simulation` 수준 테스트 하나.
  (1) 진짜 입력 1건을 넣고 (2) 이후 입력 없이 `carry_forward_max_ticks + N` tick 을 돌린다.
  (3) `input_carried_forward` 가 **정확히 `carry_forward_max_ticks` 만큼 증가하고 그 뒤로는 증가하지 않는다**를 단언.
  (4) 만료 전후의 **속도를 tick 별로 출력**한다(계약이 요구한 "속도 곡선").
  이 테스트 하나가 SC-19 와 M-17 의 두 줄을 동시에 닫는다.

### 6.5 client — **SC-46 FAIL**: EditMode 종료 코드가 0 이 아니다

- **계약이 요구한 관찰**: "**종료 코드 0**, 실패 0, 리포트 2개 존재, `tests` 수가 0 이 아니고 그 수를 적는다" (AC-11)
- **실제**: `unity test … --mode EditMode` → **exit 8** (`Error: 테스트 실패: Unity 프로세스가 코드 2(으)로 종료되었습니다.`)
  (`evidence/SC-46-editmode-qa.log`). **테스트는 하나도 실패하지 않았다**: `total=141 passed=138 failed=0 skipped=2 **inconclusive=1**`.
- **원인을 격리했다** (`evidence/SC-46-root-cause.log`):

  | 실행 | 결과 | exit |
  |---|---|---|
  | `--filter "Reconcile_MissingS6ReplayAsset_IsRecordedPending"` | total=1 **inconclusive=1** | **8** |
  | `--filter "Starfall.Tests.EditMode.ContractFixtureTests"` | total=67 passed=67 | **0** |

  → **Inconclusive 한 건이 CLI 종료 코드를 비-0 으로 만든다.** 테스트 러너가 깨진 것이 아니다.
- **왜 지금 터졌나 — 이것이 이 FAIL 의 진짜 내용이다.** client 의 실행(**05:27**)에서 이 테스트는
  `Skipped`("S6 replay asset not found … server has not produced it yet")였다.
  server 의 S6/S8 이 **06:12** 에 `server/crates/sim/tests/data/replay/` 를 만들었다
  (`initial.json` 2041 B / `inputs.jsonl` 2708 B / `snapshots.jsonl` 525272 B).
  그 뒤로 이 테스트는 `Inconclusive` 로 바뀐다:
  `"S6 replay directory exists but this test has not been wired to consume it yet - update this test to replace the synthetic replay once the directory appears."`
  **client 가 스스로 심어 둔 신호가 정확히 의도대로 울렸다.** 두 구현자의 실행 시각이 달라 아무도 못 봤을 뿐이다.
- **재현**: `unity test client --mode EditMode --report-format nunit --output <out>.xml` → exit 8.
- **요청**: `Reconcile_MissingS6ReplayAsset_IsRecordedPending`(`client/Assets/_Project/Tests/EditMode/ReconciliationTests.cs`)을
  **실제 S6 산출물을 읽는 테스트로 교체**한다 — client 가 `03_client_impl.md` §7 에서 SC-51/52 에 대해 예고한 바로 그 작업이다.
  **게이트 G-k 대로 파일을 복사하지 말고 상대 경로로 참조한다**: `server/crates/sim/tests/data/replay/`.
  이것이 SC-46 · SC-51 · SC-52 를 **한꺼번에** 닫는다.

---

## 7. 라운드 1 집계 (블록 0·1·2 + 추가 4항목)

### 7.1 전체

| 판정 | 항목 수 | ID |
|------|--------|-----|
| **PASS** | **34** | SC-01·02·03·04·05·06·07, SC-15·16·17·20·21·22, SC-34, SC-35·36·37·38·39·40, SC-41·42·43·44·45·47·48·49·50, SC-82·83·84·85·86 |
| **FAIL** | **3** | **SC-18**(server) · **SC-19**(server) · **SC-46**(client) |
| 미검증(환경) | **0** | — |
| **이번 턴 판정 총계** | **37 / 86** | 나머지 49항목은 블록 3~9(실서버·Unity·부하)로, 다음 턴이다 |

**미검증(환경)이 0 이라는 사실을 강조한다.** 이번 범위에서 환경 때문에 못 잰 것은 없다.
**FAIL 3건은 전부 "구현 부재"가 아니라 "검증 부재"이거나(SC-18·19) "다른 구현의 산출물이 도착해 상태가 바뀐 것"(SC-46)이다.**
세 건 모두 고칠 코드가 작다.

### 7.2 담당자별 수정 요청

| 담당 | 항목 | 한 줄 요약 | 상세 |
|---|---|---|---|
| **server** | SC-18 | soft 경계 테스트를 분리하고 "**조작은 계속 먹는다**"를 단언 | §6.3 |
| **server** | SC-19 | `carry_forward_max_ticks` 만료를 실제로 태우고 `input_carried_forward` 를 단언 + 속도 곡선 출력 | §6.4 |
| **client** | SC-46 | `Reconcile_MissingS6ReplayAsset_IsRecordedPending` 를 **실제 S6 산출물**로 교체 → exit 0 회복 (SC-51·52 도 같이 닫힌다) | §6.5 |
| server (권고) | SC-40(f) | `10.0` 거부 단언 1건 추가 — 판정은 PASS | §6.1 |
| **architect (통지)** | — | 계약 §2 의 "`ShipState` 17필드" · 스펙 §5.4 머리말 "17/10/7" 이 옛값이다 | §6.2 |

### 7.3 `cargo test --workspace --locked` 반복 실행 — **간헐 실패 0**

| 회차 | 결과 | 소요 |
|---|---|---|
| 1 | 바이너리 13 / **152 passed / 0 failed** / 4 ignored / exit 0 | 43 s |
| 2 | 동일 | 39 s |
| 3 | 동일 | 38 s |

과거 5회 중 2회 실패하던 `sc20_tick_command_cap_drops_excess_without_closing` 을 포함해
`sc22_slow_consumer_…`·`sc30_sc31_stats_…`·`too_many_in_flight_is_reachable_…`·`i44_world_full_exempts_a_resuming_actor`
가 **3/3 통과**했다. **재시도로 덮은 실행은 한 건도 없다.**

---

## 8. 성능 기록 (M-1 ~ M-6) — **이번 라운드 없음**

이번 범위는 대부분이 빌드이고, 게이트 G-c(측정 중 어떤 빌드도 돌리지 않는다)가 빌드와 측정을 같은 시간에
돌리지 못하게 한다. **그래서 리더가 이 턴을 블록 3 앞에서 끊었다.** M-1~M-6 은 블록 8 에서 잰다.

유일하게 기록할 값: SC-05 의 기동 직후(접속 0건) `/debug/stats` — `tick_total = 9`, `tick_overrun_total = 0`,
`tick_body_us.max = 88 µs`, `tick_lag_seconds = -0.049825`. **부하가 없는 상태라 기준선으로 쓰지 않는다.**

## 9. designer 지표 (M-7 ~ M-8, M-16) — **이번 라운드 없음**

전부 실서버·Unity 가 필요하다. 블록 6~8.

---

## 10. 계약 외 발견 (판정에 쓰지 않는다)

1. **`sc*` 테스트 이름이 p0-02 번호를 쓴다.** `sc20_*`·`sc22_*`·`sc30_sc31_*` 는 p1-01 계약의 SC-20/22/30/31 과
   **번호만 같고 다른 항목**이다. 대응표를 §2.3 에 넣었다. 다음 슬라이스에서 테스트 이름에 슬라이스 접두사를
   붙이는 편이 안전하다(`p0_02_sc20_…`).
2. **`03_client_impl.md` §7 이 없는 증거 파일을 가리킨다.** `evidence/SC-41-step{1,2,3}-*.log` 는 존재하지 않는다
   (디렉토리 자체를 qa 가 이번에 처음 만들었다). 1단은 qa 가 재현했고, 2·3단은 중간 버전 생성기가 없어 재현 불가다.
   **구현자가 "증거 파일"을 적을 때 실제로 그 경로에 쓰는지 확인하는 절차가 필요하다.**
3. **`crates/sim` 의 `[dev-dependencies]` 에 `serde`·`serde_json` 이 들어왔다**(S6 결정성 재생용).
   SC-02 는 `[dependencies]` 를 보므로 PASS 다. 프로덕션 링크에는 포함되지 않는다는 주석도 파일에 있다.
   **결정성 코어의 의존성 0 원칙이 dev 쪽으로 한 칸 열린 것은 사실이므로 적어 둔다.**
4. **SC-06 ①② 의 로그 형식.** 스키마 층 거부는 `field=/expected=/actual=` 세 토큰 대신 `error=<serde 메시지>`
   한 덩어리다. 세 정보는 전부 들어 있어 판정에는 영향이 없다(§3.3 참고).
5. **`server/crates/sim/tests/data/replay/` 가 git 에 추적되지 않는다**(`?? server/crates/sim/tests/data/`).
   C4 가 상대 경로로 이 디렉토리를 읽게 되면(G-k), **커밋하지 않을 경우 다른 머신·CI 에서 그 테스트가
   또 Inconclusive 가 된다.** 커밋할지 `.gitignore` 에 넣고 테스트를 조건부로 둘지 architect·리더 판단이 필요하다.
   (결정성 테스트가 매 실행마다 바이트 동일하게 재생성하므로 커밋해도 `git diff` 가 생기지 않는다 — server 실측.)

---

## 11. 다음 턴 — 진입 전제와 차단 목록

### 11.1 블록 3~5 (실서버, Unity 불필요) — **차단 없음**

지금 바로 갈 수 있다. 확인된 전제:

- 인프라 가동 중(`starfall-postgres-1` healthy `127.0.0.1:15432`, `starfall-redis-1` healthy `127.0.0.1:16379`).
- 서버가 실제 `data/` 로 뜨고 stdin `shutdown` 으로 **종료 코드 0** 으로 내려간다(§3.2).
- 봇 하네스 빌드·테스트 정상(49 passed) — E6 없음.
- `tests/e2e/server_boot.py stats` 로 서버를 띄우고 내리는 절차가 스크립트화돼 있다.
- **G-a 결정 적용**: `down -v` 안 쓴다 → **SC-80 은 이번 실행의 tick 구간 한정**으로 판정한다.
  구간의 시작 tick 은 서버 기동 로그의 `start_tick` 으로 잡는다(이번 관측: `last_tick=350420 → start_tick=350421`).
- **게이트 G-i/G-j**: SC-10·11·12 는 30초 잔류 창 + 여유가 필요하다. 블록 5 에 시간 예산을 따로 잡아야 한다.
- **게이트 G-f**: 치트·프레이밍(블록 4)은 부하와 분리 실행.

### 11.2 블록 6 (Unity) — **client 가 먼저 해야 할 일 3건**

리더가 client 스폰 프롬프트에 그대로 넣을 수 있는 형태로 적는다.

1. **`Reconcile_MissingS6ReplayAsset_IsRecordedPending` 를 실제 S6 산출물로 교체한다.**
   산출물은 이미 있다: `server/crates/sim/tests/data/replay/{initial.json, inputs.jsonl, snapshots.jsonl}`
   (2041 B / 2708 B / 525,272 B, 스냅샷 300줄). **복사하지 말고 상대 경로로 참조한다**(게이트 G-k).
   지금 이 테스트가 `Inconclusive` 라 **EditMode 전체 종료 코드가 8 이다**(SC-46 FAIL). 교체하면
   **SC-46 · SC-51 · SC-52 가 한꺼번에 닫힌다**. SC-55 는 이미 통과했다(`Reconcile_TurningSnapshot_RestoresBothAngularVelocitiesSeparately_NotFromASum`).
2. **`ProfilerMarker "Starfall.Snapshot.Handle"` 훅을 심는다** (SC-58).
   client 가 `03_client_impl.md` §7 에서 **스스로 "알려진 누락"으로 보고한 항목**이고,
   **환경 문제가 아니라 구현 부재라 계약 §6 상 FAIL 이 될 자리**다. 실서버 측정 전에 들어가 있어야 한다.
3. **SC-59 의 육안 관찰(mp4 4개 + 스크린샷 4장)을 준비한다.**
   계약 §0.10 이 말하는 대로 **이 슬라이스에서 손 방향 부호를 검출할 수 있는 장치는 SC-59 하나뿐이다.**
   HUD 표시 코드(`GreyboxSession.OnGUI` 의 `thrust/roll/brake/assist`)는 이미 있다 — 남은 것은 Play 모드 실행과 녹화다.

추가로 블록 7(SC-64·65)은 **§0.11 의 "한 Unity 프로세스 안에 세션 2개 + 관측자 파이프라인 2개"** 경로가
동작하는지 먼저 확인해야 한다. 막히면 SC-64 만 봇으로 살리고 **SC-65 는 미검증(환경, E8)** 이다 —
**봇을 B 로 쓰면 부호가 뒤집혀 정상 동작을 버그로 읽는다.**

### 11.3 블록 8 (부하) — 게이트 확인 사항

- **G-d**: 31번째 연결(Unity PlayMode)이 **먼저** 붙은 뒤 봇 시작.
- **G-e**: A 단계 중 `client/` 아래 파일 저장 금지(도메인 리로드가 31번째 연결을 끊는다).
- **G-c**: 측정 중 빌드 금지. 이번 턴이 빌드를 전부 끝내 두었으므로, 다음 측정 턴은 **빌드 없이** 들어갈 수 있다.
- **G-h**: 판정 전에 `close_reason` 을 먼저 본다.

---

## 12. 요약 — **이 슬라이스가 아직 증명하지 못한 것** (계약이 요약에 싣도록 요구한 문장)

> **손 방향 부호가 통째로 뒤집혀도 자동 검증은 전부 통과한다.** 서버와 클라이언트가 같은 공식을 쓰므로
> 예측 오차 0, fixture 왕복 통과, 결정성 바이트 동일까지 전부 초록이다. 특히 오토레벨의 `sin_err` 부호
> 한 줄이 그 위험을 진다(ADR-0010 §2.1). **검출기는 SC-59(AC-14 a·d)의 육안 관찰 하나뿐이고,
> 그것을 사람이 실제로 볼 때까지 이 슬라이스는 부호에 대해 아무것도 증명하지 못한다.**

**이번 라운드에 SC-59 는 실행되지 않았다 → M-14: 부호 미검증.**
위에서 PASS 로 적은 34건 — 152개 Rust 테스트, 141개 Unity 테스트, 201행 경계면 대조, 두 프로세스 525,272바이트
동일까지 — **전부 이 한계 위에 있다.** 숫자가 많다는 것이 부호가 맞다는 뜻이 아니다.

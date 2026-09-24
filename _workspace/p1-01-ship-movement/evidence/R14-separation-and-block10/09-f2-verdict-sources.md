# F-2 — 미지명 36항목이 **실제로 어떤 출력으로 판정됐는가** (리포트 전수 추출)

architect R20 F-2. **추측으로 채우지 않기 위해** 리포트의 판정표 행을 그대로 뽑았다.
판정 행이 없는 항목은 이 슬라이스에서 **한 번도 판정된 적이 없다** — 그 칸은 블록 절차서의 명령을 적어야 한다.

## 요약 — 수단 후보와 확신도

**"확신 높음"만 그대로 쓸 수 있다.** 나머지는 증거를 읽고 architect 가 판단한다 — 내가 짐작으로 채우면 그것이 R18 이 잡은 병(라벨과 실제 출처의 불일치)을 **문서 쪽에서 재현**하는 것이다.

**신호가 둘 이상이면 고르지 않는다.** 첫 생성에서 SC-03 을 `cargo test` 로 분류할 뻔했는데, 그 근거로 잡은 `still_settled_since` 는 **테스트 이름이 아니라 grep 오탐 목록의 토큰**이었다. 자동 분류가 틀리는 자리가 정확히 거기다.

| SC | 상태 | 수단 후보 | 근거 |
|---|---|---|---|
| SC-03 | 판정됨 | **architect 판단 필요 (신호 충돌)** | `cargo test -p starfall-sim` 계열 / grep (증거 로그에 명령이 있다) · 확신 낮음 |
| SC-04 | 판정됨 | grep (증거 로그에 명령이 있다) | 본문에 grep · 확신 높음 |
| SC-10 | 미판정 | 블록 절차서의 명령 (목록 B) | — |
| SC-16 | 판정됨 | `cargo test -p starfall-sim` 계열 | 테스트 이름: speed_never_exceeds_max_speed · 확신 높음 |
| SC-17 | 판정됨 | `cargo test -p starfall-sim` 계열 | 테스트 이름: diagonal_thrust_is_clamped_to_main_thrust_magnitude · 확신 높음 |
| SC-18 | 판정됨 | `cargo test -p starfall-sim` 계열 | 테스트 이름: hard_boundary_removes_only_radial_velocity · 확신 높음 |
| SC-19 | 판정됨 | `cargo test -p starfall-sim` 계열 | 테스트 이름: coasting_decays_gradually_not_instantly · 확신 높음 |
| SC-21 | 판정됨 | `cargo test -p starfall-sim` 계열 | 테스트 이름: attitude_settles_without_overshoot · 확신 높음 |
| SC-22 | 판정됨 | `cargo test -p starfall-sim` 계열 | 테스트 이름: degenerate_aim_keeps_current_attitude_and_is_flagged · 확신 높음 |
| SC-23 | 판정됨 | **architect 판단 필요** | 리포트 인용만 있다 · 확신 낮음 |
| SC-24 | 판정됨 | **architect 판단 필요** | 리포트 인용만 있다 · 확신 낮음 |
| SC-25 | 판정됨 | **architect 판단 필요** | 리포트 인용만 있다 · 확신 낮음 |
| SC-28 | 미판정 | 블록 절차서의 명령 (목록 B) | — |
| SC-30 | 미판정 | 블록 절차서의 명령 (목록 B) | — |
| SC-31 | 판정됨 | `cargo test -p starfall-sim` 계열 | 테스트 이름: ack_input_seq · 확신 높음 |
| SC-32 | 미판정 | 블록 절차서의 명령 (목록 B) | — |
| SC-35 | 판정됨 | `cargo test -p starfall-sim` 계열 | 테스트 이름: registry_consistency, schema_ids_match_paths · 확신 높음 |
| SC-37 | 판정됨 | `cargo test -p starfall-sim` 계열 | 테스트 이름: invalid_rejected_by_schema · 확신 높음 |
| SC-38 | 판정됨 | **architect 판단 필요 (신호 충돌)** | `cargo test -p starfall-sim` 계열 / grep (증거 로그에 명령이 있다) · 확신 낮음 |
| SC-39 | 판정됨 | `cargo test -p starfall-sim` 계열 | 테스트 이름: required_field_mutations · 확신 높음 |
| SC-40 | 판정됨 | `cargo test -p starfall-sim` 계열 | 테스트 이름: registry_server_types_mapped, fixtures_roundtrip · 확신 높음 |
| SC-41 | 판정됨 | `dotnet run ContractsCodegen.cs` | codegen 인용 · 확신 높음 |
| SC-42 | 판정됨 | `dotnet run ContractsCodegen.cs` | codegen 인용 · 확신 높음 |
| SC-43 | 판정됨 | **architect 판단 필요** | 리포트 인용만 있다 · 확신 낮음 |
| SC-44 | 판정됨 | **architect 판단 필요** | 리포트 인용만 있다 · 확신 낮음 |
| SC-45 | 판정됨 | `cargo test -p starfall-sim` 계열 | 테스트 이름: close_reason · 확신 높음 |
| SC-50 | 판정됨 | **architect 판단 필요** | 리포트 인용만 있다 · 확신 낮음 |
| SC-51 | 판정됨 | **architect 판단 필요** | 리포트 인용만 있다 · 확신 낮음 |
| SC-52 | 판정됨 | **architect 판단 필요** | 리포트 인용만 있다 · 확신 낮음 |
| SC-53 | 판정됨 | `unity test client --mode EditMode` | EditMode 인용 · 확신 높음 |
| SC-54 | 판정됨 | **architect 판단 필요** | 리포트 인용만 있다 · 확신 낮음 |
| SC-60 | 판정됨 | **architect 판단 필요** | 리포트 인용만 있다 · 확신 낮음 |
| SC-66 | 판정됨 | **architect 판단 필요** | 리포트 인용만 있다 · 확신 낮음 |
| SC-67 | 판정됨 | `cargo test -p starfall-sim` 계열 | 테스트 이름: ack_input_seq · 확신 높음 |
| SC-68 | 판정됨 | **architect 판단 필요** | 리포트 인용만 있다 · 확신 낮음 |
| SC-74 | 미판정 | 블록 절차서의 명령 (목록 B) | — |

## SC-03

- `04_r1` :: SC-03 ¦ **PASS** ¦ 순진한 grep **93건**(전부 `.expect(`·`still_settled_since`·`expected` 류 오탐) → ADR-0010 §3 원문의 정밀 grep(메서드 호출 형태) **0건, exit 1**. 금지 목록에 `mul_add`·`hypot`·`to_radians`/`to_degrees`·`signum` 포함 확인 (`SC-03-forbidden-fns.log`)
- `04_r4` :: SC-03 ¦ 순진한 grep / ADR-0010 §3 정밀 grep (`evidence/R4-B6/B0/SC-03-04-87.log`) ¦ 순진 **118줄**(오탐 — p0-02 때 13 에서 코드가 늘었다) / 정밀 **0건**(exit 1). **양성 대조**: 같은 정밀 정규식이 심은 `x.sin()`·`f64::hypot`·`mul_add` 줄을 잡고 `.expect(` 줄은 안 잡는다(1/1) ¦ **PASS**



## SC-04

- `04_r1` :: SC-04 ¦ **PASS** ¦ `SystemTime`/`Instant` **실사용 0건**(`crates/sim/src/lib.rs:33` 주석 1건뿐). `HashMap` **0건**. `HashSet` 1건은 `session.rs:54` 의 `seen` 이고 `insert`(85행)·`remove`(92행)뿐 — **순회 지점이 없어 순서 의존이 없다**. 함선 순회는 전부 `BTreeMap<UuidV7,_>` 경유:
- `04_r4` :: SC-04 ¦ grep 4종 ¦ `SystemTime`/`Instant` 코드 0(주석 1) · `HashSet` 은 `session.rs` 의 `seen`(insert/remove 만, 순회 0) · `ships: BTreeMap<UuidV7, …>`(`:373`), 순회 6곳 전부 `self.ships.{keys,values}` ¦ **PASS**



## SC-10

**판정 행 없음** — 미판정. 블록 절차서의 명령이 들어가야 한다.



## SC-16

- `04_r1` :: SC-16 ¦ **PASS** ¦ `speed_never_exceeds_max_speed` → `[AC-4b] **2000 tick** 후 도달 최대 속도 = **140 m/s** (상한 140)` — 상한에 붙되 넘지 않는다



## SC-17

- `04_r1` :: SC-17 ¦ **PASS** ¦ `diagonal_thrust_is_clamped_to_main_thrust_magnitude` → `[AC-4c] 대각선 가속 크기 = **35 m/s²** (상한 35)`. 클램프가 없으면 `√3 × 35 ≈ 60.6` 이 나온다



## SC-18

- `04_r1` :: **SC-18** ¦ **FAIL** ¦ **hard 절반만 단언됐다.** `hard_boundary_removes_only_radial_velocity`(`integrate.rs:440`)는 `r ≤ 12000 + 1e-6`, 반경 속도 `≤ 1e-6`, 접선 `v.z = 49.82` 보존을 단언한다 — 여기까지는 통과다. **soft 절반("soft 경계를 넘으면 원점 방향 가속이 더해지고 **조작은 계속 먹으며**")을 단언하



## SC-19

- `04_r1` :: **SC-19** ¦ **FAIL** ¦ **이월 만료가 단언되지 않았다.** `coasting_decays_gradually_not_instantly`(`integrate.rs:468`)는 `step(state, &no_input(), …)` 를 **직접** 부른다 — 즉 만료 *이후* 상태를 손으로 만들어 넣고 감쇠만 본다(`[AC-4e] 1 tick 후 속도 = 99.65 (시작 100)`). **`carry_forward_m



## SC-21

- `04_r1` :: SC-21 ¦ **PASS** ¦ `attitude_settles_without_overshoot` → **안착 tick 수 = 84**(M-12). 루프 안에서 **매 tick** `\ ¦ ω_aim\



## SC-22

- `04_r1` :: SC-22 ¦ **PASS** ¦ `degenerate_aim_keeps_current_attitude_and_is_flagged`(`integrate.rs:601`) — 전 성분 0 쿼터니언이 **거부되지 않고** `outcome.aim_degenerate == true`, `\ ¦ ω_aim\



## SC-23

- `04_r2` :: **SC-23** ¦ 위치·자세 주입 반례가 `MALFORMED_COMMAND` 가 아니라 `UNKNOWN_COMMAND_TYPE` 으로 거부된다 — **차단은 되지만 계약이 지정한 층에서 되지 않는다**
- `04_r2` :: SC-23 ¦ **FAIL(차단)** ¦ probe `cheat-position`·`cheat-attitude` 가 `SESSION_READY` 타임아웃. 설령 붙었어도 §2.1 때문에 `MALFORMED_COMMAND` 가 아니라 `UNKNOWN_COMMAND_TYPE` 이 된다



## SC-24

- `04_r2` :: **SC-24(c)(d)** ¦ 범위 초과 거부·이월 지속·회전량 상한 — 명령이 도달하지 않아 관측 불가
- `04_r2` :: SC-24 ¦ **FAIL(차단)** ¦ (b) 필드 목록은 §3.3 에서 정적으로 확인됨. (c)(d) 는 probe `cheat-range` 타임아웃



## SC-25

- `04_r2` :: **SC-25** ¦ 속도 핵: 두 봇의 이동 거리 차이를 재려면 함선이 움직여야 한다. **모든 함선이 스폰 지점에 고정돼 있다**



## SC-28

**판정 행 없음** — 미판정. 블록 절차서의 명령이 들어가야 한다.



## SC-30

**판정 행 없음** — 미판정. 블록 절차서의 명령이 들어가야 한다.



## SC-31

- `04_r2` :: **SC-31** ¦ `ack_input_seq` 가 **영원히 `null`** 이다. "적용 전 `null`" 절반은 관측됐지만(1066건 전수 `null`), **"서버가 마지막으로 적용한 `input_seq`" 와 "세션 내 단조 비감소"는 적용이 0건이라 관측 자체가 불가능**하다



## SC-32

**판정 행 없음** — 미판정. 블록 절차서의 명령이 들어가야 한다.



## SC-35

- `04_r1` :: SC-35 ¦ **PASS** ¦ 테스트 **10건 전부 통과**. 출력에 찍힌 건수: **스키마 18 / 유효 fixture 26 / 반례 34 / 레지스트리 타입 13**. `registry_consistency`·`schema_ids_match_paths`·`registry_file_validates_against_schema`·`integer_bounds_rejected`(정수 경계 5건) 포함 (`SC-35-40-c
- `04_r4` :: **SC-35~40** ¦ `cargo test -p starfall-contracts --locked -- --nocapture` ¦ **33 + 10 passed / 0 failed**(bless 1 ignored), exit 0. 출력: 스키마 **18** 오프라인 검증 · 유효 fixture **27** 왕복 · 반례 **34** 스키마 거부 · serde 매트릭스 **34/34 기대=실제**(전부 "거부") · required 변이 **278**건 전부 실패 · 레지스트리 **13** / server 태그 13 대응 ¦ **PASS**



## SC-37

- `04_r1` :: SC-37 ¦ **PASS** ¦ `invalid_rejected_by_schema` → `[SC-14] 스키마가 거부한 반례: **34건**` + 34개 파일명, 전부 거부



## SC-38

- `04_r1` :: SC-38 ¦ **PASS** ¦ `invalid_serde_matrix` → `[SC-15] serde 매트릭스 검사한 반례: **34건**`. 출력에 **34행** 전부 `기대=거부 / 실제=거부`(`기대=통과` 행 0). 스펙 §5.4 의 "Rust serde" 열(표에 오른 18행 전부 거부)과 **일치**. 보조 grep `serde(flatten) ¦ serde(tag *=` → **실제 속성 0건**(매칭 4건은 전부 `commands.rs`·`dispatch.rs` 의 "쓰면 안 된다" 주석)



## SC-39

- `04_r1` :: SC-39 ¦ **PASS** ¦ `required_field_mutations` → `[SC-18] required 변이 **264건**이 모두 역직렬화에 실패했다` (p0-02 는 124건 → 신규 7타입으로 140건 증가)



## SC-40

- `04_r1` :: SC-40(e) ¦ **PASS** ¦ `registry_server_types_mapped` → `[SC-16] server 태그 타입 **13건**이 대응표에 있다` + 13행 이름에서 Rust 타입 대응
- `04_r1` :: SC-40(f) ¦ **PASS** (다른 경로로 관측) ¦ **계약이 겨냥한 구멍은 실제로 막혀 있다.** ① `fixtures_roundtrip` 은 `data` kind 를 `as_f64` 로 정규화하지 **않고** 전 kind 를 **엄격 `Value` 비교**한다(`contract_tests.rs:330~338`) — `serde_json::Value` 는 `Number(10)` 과 `Number(10.0)` 을 다른 값으로 보므로, 스키마가



## SC-41

- `04_r1` :: SC-41 ¦ **PASS** (1단 qa 독립 재현, 2·3단은 구현자 기록) ¦ qa 가 `git show HEAD:tools/codegen/ContractsCodegen.cs` 로 확장 전 생성기를 현재 `contracts/` 에 돌려 **첫 실패가 `array` 가 아니라 `maxItems` 임을 재현**: `codegen: contracts/messages/WORLD_SNAPSHOT.schema.json/properties/payload/properties/ship



## SC-42

- `04_r1` :: SC-42 ¦ **PASS** ¦ qa 가 **임시 디렉토리에 독립 재생성**(client 파일을 건드리지 않았다). 1회차 11파일 / 2회차 11파일 → `diff -r gen1 gen2` **차이 0**. **gen1 의 11개 `.cs` 가 client 제출본과 SHA-256 전부 일치**(`SAME` 11/11). 실제 디렉토리에 `--check` → `--check: up to date (11 file(s))`, 
- `04_r4` :: **SC-42** ¦ `dotnet run ContractsCodegen.cs … --out <tmp>/gen1`, 같은 명령 gen2, `--check` 양쪽 ¦ run1·run2 exit 0, **gen1 == gen2**(diff exit 0), `--check` **up to date (11)** exit 0, client `Generated/` 도 `--check` exit 0. **qa 재생성 11 파일 == client 제출본 11 파일 바이트 동일** ¦ **PASS**



## SC-43

- `04_r1` :: SC-43 ¦ **PASS** ¦ `WorldSnapshotMessage.cs:56` → `public ShipState[] Ships { get; set; }` (**배열**). 클래스 중첩: `WorldSnapshotMessage`(11행) > `WorldSnapshotPayload`(40행) > `ShipState`(71행). **`ShipState` JsonProperty = 18개**(`angular_velocity
- `04_r4` :: **SC-43** ¦ 생성물 실물 ¦ `ShipState[] Ships`, `ShipState` 중첩, JsonProperty **18** ¦ **PASS**



## SC-44

- `04_r1` :: SC-44 ¦ **PASS** ¦ stdout 3줄: `skipped SHIP_CLASS (kind 'data': no DTO is generated)` 외 `STAR_SYSTEM`·`SYNC_TUNING`. 생성 디렉토리에 `ShipClass(Table).cs`·`StarSystem(Table).cs`·`SyncTuning(Table).cs` **6개 후보 전부 부재** 확인
- `04_r4` :: **SC-44** ¦ 위 실행 stdout ¦ `skipped SHIP_CLASS/STAR_SYSTEM/SYNC_TUNING (kind 'data': …)` **3줄**, 세 `.cs` 부재 ¦ **PASS**



## SC-45

- `04_r1` :: SC-45 ¦ **PASS** ¦ `git diff --stat HEAD -- <Generated>` → **`ContractTypes.cs` 1 file changed, 8 insertions(+), 0 deletions**. 기존 6타입(`PingServerCommand`·`PingReplyMessage`·`CommandResultMessage`·`SessionReadyMessage`·`SessionOpenedEvent`
- `04_r4` :: **SC-45** ¦ `git diff HEAD(42a7965) -- Generated/` ¦ 기존 6타입 중 **5개 바이트 동일**, `SessionClosedEvent.cs` **1줄 변경**(= `close_reason` XML 주석에 `SUPERSEDED` 설명, 스키마 description 그대로) · `ContractTypes.cs` **+8/−0** ¦ **PASS (7차 개정 해석)** — 1줄 변경은 **생성기 변경이 아니라 7차 계약 데이터 변경**(스키마 description)의 결과다. 계약 문구 "6개 바이트 동일" 은 7차 이전 기준이다 → §계약 자체 문제에 적는다



## SC-50

- `04_r1` :: SC-50 ¦ **PASS** ¦ client 테스트 `ClientDataCopy_MatchesRepositoryOriginal` Passed **+ qa 독립 SHA-256 대조**(`SC-50-data-copy.log`): 3파일 전부 동일 해시(`sync-tuning` 2583B, `scout-s01` 1938B, `cradle` 4179B), 레포 `data/` 3 = 사본 3, **누락 0**
- `04_r4` :: **SC-50** ¦ `ClientDataCopy_MatchesRepositoryOriginal` + **qa 독립 `sha256sum`**(`evidence/R4-B6/B0/SC-50-data-copy.log`) ¦ 레포 `data/` json 3 = client `Assets/_Project/Data` json 3, **3/3 바이트 동일** ¦ **PASS**



## SC-51

- `04_r2` :: SC-51 위치 오차 ¦ 0.005 m ¦ **0.000656 m** ¦ 약 7.6배



## SC-52

- `04_r2` :: SC-52 자세 오차 ¦ 0.02 deg ¦ **8.658e-5 deg** ¦ 약 231배



## SC-53

- `04_r4` :: **SC-53** ¦ `Reconcile_IsPureFunction_SameInputsTwice_SameOutput`, `Reconcile_DoesNotDependOnClockOrCallOrder` ¦ 2건 Passed. (2) 의 테스트는 **호출 사이에 벽시계를 바꾼다**(5 ms 수면 + 할당 10만). 프레임 시간·도착 시각은 EditMode 에서 바꿀 수 없어 **정적 보조**: `Reconcile(history, confirmed, ackInputSeq, ship, boundary, dt)` 에 시각 매개변수가 없고, `Starfall.Flight/*.cs` 에 `DateTime ¦ Stopwatch



## SC-54

- `04_r4` :: **SC-54** ¦ `Reconcile_HistoryWithASkippedInputSeq_StillConvergesToServerState` ¦ Passed (입력열 1,2,4,5) ¦ **PASS**



## SC-60

- `04_r4` :: SC-60 (b)(c)(d) 로직 ¦ `RemoteInterpolationTests` 5 · `RemoteShipBufferTests` 6 · `RemoteShipRegistryTests` 4 = **15건** ¦ 전부 Passed. (b) `Slerp_AtQuarterPoint_DiffersFromNormalizedLerp_ForLargeAngle`(t = 0.25) (c) `GetDisplay_PastExtrapolationCap_FreezesAndZeroesVelocity` (d) `OnSnapshot_LingeringShip_IsKept_NotRemoved`·`…Absent…_IsRemovedI ¦ 로직 확인. **SC-60 판정은 블록 6 실화면(a)(e) 뒤**



## SC-66

- `04_r2` :: SC-66 ¦ **FAIL(차단)** ¦ 같음. **시도 0 / 차단 0** — 세션이 서지 않아 시도조차 못 했다



## SC-67

- `04_r2` :: SC-67 ¦ **FAIL(차단)** ¦ probe `cheat-seq` 타임아웃. `ack_input_seq` 는 §2.7 대로 구조적으로 `null`



## SC-68

- `04_r2` :: SC-68 ¦ **부분** ¦ (e) 는 §3.3 에서 **PASS**(계약이 증거다). (f) 잔류 가로채기·(g) `SESSION_READY` 이전 송신은 **미실행**



## SC-74

**판정 행 없음** — 미판정. 블록 절차서의 명령이 들어가야 한다.


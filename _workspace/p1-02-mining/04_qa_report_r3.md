# p1-02-mining QA 리포트 — 라운드 3 (마지막, 3/3)

- 일시: 2026-10-04 00:19 ~ 00:55 KST. 동결 `FREEZE` 는 00:19 부터.
- 기준: `02_sprint_contract.md` (114 항목). r3 중 계약 변경 3건(§12 변경 이력). 셋 다 방법 칸의 이름 정합이고 문서만 고쳤다(코드·판정 기준·⊘ 불변): SC-09 필터 교체 + gateway 줄 분리(리더 지시) · SC-10 필수 쌍 3+4 테스트 추가 · SC-28 유령 이름 → 실제 이름 셋(리더 결정 A)
- 빌드: HEAD `e0037a4` + 작업 트리(커밋 전). 소스 433 파일 + 마이그레이션 3 · Cargo.lock 2 의 sha256 은 시작과 끝이 같다(`src_sha_start.txt` = `src_sha_end.txt`, `src_sha_start_extra.txt`)
- 증거: `evidence/r3_20261004/`
- **요약: PASS 112 / FAIL 0 / 미검증 2 / 전체 114**
  - SC-28 은 한때 FAIL 이었다(이름만). 계약 방법 칸이 지명한 `recording_halted_rejects_state_change` 가 존재하지 않아 실행이 0 건이었다. 리더 결정 A 로 실제 이름 셋으로 정정했고, 계약 명령 그대로 각 1 건 실행·ok 를 확인해 PASS 로 판정했다(§4).
  - 미검증: SC-68(사람 Unity 세션) · SC-98(CI 첫 실행 — 커밋·푸시 전)
  - r2 대비: FAIL → PASS 5 (SC-09 · 10 · 17 · 19 · 22), FAIL → PASS 1 (SC-28 — r2 사유인 21 경계·이동 수락이 해소됐고, r3 에서 드러난 유령 이름은 계약 정정으로 해소), 회귀 0

## 1. 이번 라운드에서 본 것

| 단계 | 결과 | 증거 |
|---|---|---|
| 0. 동결 기록 | HEAD `e0037a4`, porcelain 시작·끝 차이 = r3 증거 파일뿐 | `head.txt` · `porcelain.txt` · `porcelain_end.txt` |
| r2 대비 바뀐 소스 | 3 파일: `sim/src/simulation.rs` · `gateway/src/ws.rs` · `gateway/tests/ws_integration.rs`(r2 `src_sha_start.txt` 와 diff). `runtime.rs` 는 r2 와 같다 | `src_sha_start.txt` |
| 테스트 외 코드 변경 여부 | **없음.** server 트랜스크립트에서 15:00Z 이후 Edit 를 전부 대조했다. 비테스트 영역(simulation.rs < 1850행, ws.rs < 786행) 편집은 server 자기 점검 변이 2쌍(삽입 → 되돌림)뿐이다. 다른 에이전트의 편집은 0건이다. 그래서 r2 범위 밖 결과(봇·DB 정지·Unity·파이썬 판정)를 회귀 기준으로 그대로 인용한다 | `nontest_change_audit.txt` |
| server 자기 점검 sha256 | 03_server_impl.md R2-S 표의 4 파일 해시 = `src_sha_start.txt` 4/4 일치 | 위 |
| 서버 게이트 | fmt rc 0 · clippy rc 0 · workspace test **297 passed / 0 failed / 6 ignored**(r2 296 + SC-09 카운터 테스트 1) · release sim **80 passed** | `gates_summary.txt` · `test.log` · `release.log` |
| 반복 10 회 | 10/10 rc=0, 매회 297 passed, 실행 중 소스 편집 0 | `repeat/summary.txt` |
| DB 테스트 census (SC-113) | 계약 27 / RAN 27 / 누락 0 / SKIPPED 0 → PASS | `census.json` |
| 계약 필터별 실행 (정정 전) | 59 필터(옛 추출기 57 + 수동 추가 2: SC-09 게이트웨이, SC-28 맨 이름) + 후보 3. 실행 0 = 1(SC-28 유령 이름) | `filters/filters_summary.tsv` · `filters_input.tsv` |
| 계약 필터별 실행 (**정정 뒤, 고친 스캔 전수**) | 고친 스캔(§3a)이 뽑은 Rust 69 필터(cargo 명령 61 + 맨 테스트 이름 8)를 계약 명령 그대로 실행. **실행 0 = 0, 실패 0.** 각 필터는 그 SC 의 테스트만 잡는다(예외는 SC-18·26 의 같은 테스트 1건 — 계약이 두 SC 에 같은 이름을 지명했다). 맨 Unity 이름 7 은 r2 xml 에서 확인(전부 passed). **유령 0** | `filters_final/filters_summary.tsv` · `scan_final/` |
| qa 구현 변이 4 | 4/4 잡힘, 원복 sha256 일치, 원복 뒤 초록 | `mutations/summary.txt` · `mutations/*.log` |
| 증거 DB | spike 월드 4805 · md5 `d1904d8f…` · migrations 3 · 테스트 DB 0. 시작 = 끝 = r2 | `fp_before.txt` · `fp_after.txt` |
| 서버 바이너리 | `cargo build -p starfall-game-server` sha `c732899b…`. r2 `79cc25f7…` 와 다르지만 PE 타임스탬프·디버그 정보의 소스 해시 때문에 비교 근거가 못 된다. 테스트 외 변경 판단은 위 트랜스크립트 감사로 했다 | `server_sha.txt` |

## 2. 재판정 항목 (r2 FAIL 6)

| ID | r2 | r3 | 증거 | r2 §4 빈 칸 → 지금 |
|----|----|----|------|------|
| SC-09 | FAIL | **PASS** | 새 필터 `mining::each_rejection_reason` → **2 건**(`…_untouched`, `…_untouched_reasons_3_6_7`), 다른 SC 의 테스트는 잡지 않는다. 게이트웨이 `mine_resource_capacity_exceeded_is_counted_in_commands_rejected_total` 1 건. 디버그 297 · 릴리스 80 에서 두 sim 테스트 ok | 사유 4 장부 ✓(simulation.rs:3748) · 5 쿨다운 ✓(:3772)·장부 ✓(:3777) · 6 쿨다운 ✓(:3903) · 7 광맥 ✓(:4001, 60 tick 대기 뒤 스냅샷)·쿨다운 ✓(:4005) · 7 주입 = **상한 − 1** ✓(:3922, ⊘ 단언 :3948) · `commands_rejected_total{CAPACITY_EXCEEDED}` +1 ✓(ws_integration.rs:1692~1770). **실제 거부 경로를 지난다(소스로 확인):** `TestServer` 는 실제 `runtime::build` 와 `Simulation` 으로 돈다(ws_integration.rs:197). 경로는 실제 소켓 → 게이트웨이 → sim 판정 7(`checked_add`, simulation.rs:1268) 거부 → `runtime.rs:773` 송신 성공 시 `stats.record_rejection(reason)` → `/debug/stats` 다. 테스트는 선로 위 사유가 `CAPACITY_EXCEEDED` 인지 ⊘로 먼저 단언하고(:1750), 카운터가 이전 값 + 1 과 **정확히** 같아질 때만 통과한다. server 변이(`record_rejection` 호출 제거)로 실패도 확인됐다. 계약 방법 칸은 ① sim ② gateway 두 줄이고, 둘 다 계약 명령 그대로 실행 ≥ 1 이다(sim 2 · gateway 1) 변이: qa MD(용량 거부가 광맥을 씀) → `:4002 CAPACITY_EXCEEDED: 광맥 상태가 바뀌면 안 된다` 실패. qa MC(소진 판정 죽임) → `:3888` 실패. server 변이 2(속도 거부의 쿨다운 누출, 카운터 호출 제거)는 R2-S 표에 있고 원복 sha256 일치 |
| SC-10 | FAIL | **PASS** | 필터 `mining::reject_order_*` → 1 건 `reject_order_adjacent_pairs_pick_the_earlier_one`. 3+4 쌍은 계약에 새로 지명한 `mining::cooldown_before_range_in_judgement_order` → 1 건 ok(계약 명령 그대로) | 다섯 쌍 모두 **같은 테스트 안에서** 단독 → 자기 사유 2 단언 + 둘 다 → 앞 사유 1 단언: 4+5(:4028~4081) · 6+7(:4084~4185) · 2+3(:4187~4237) · 5+6(:4239~4313) · 3+4(:3186~3259). 변이 qa MC(소진 판정 죽임) → `:4107 소진 단독(상한 안 건드림) → RESOURCE_DEPLETED` 실패. r2 가 말한 모양("소진 판정이 죽어도 SHIP_TOO_FAST 로 통과")이 이제 잡힌다 |
| SC-17 | FAIL(이름) | **PASS** | 필터 `mining::dup_across_reconnect` → 1 건 `dup_across_reconnect_within_linger_window_is_rejected`, ok | 이름만 바뀌었다(트랜스크립트: fn 줄 1 편집). 내용 판정은 r2 그대로 |
| SC-19 | FAIL(이름) | **PASS** | 필터 `mining::dup_two_sessions_same_tick` → 1 건 `dup_two_sessions_same_tick_accept_one_duplicate_one`, ok | 위와 같음 |
| SC-22 | FAIL | **PASS** | 필터 `mining::rejected_resend_*` → 1 건 `rejected_resend_branches`, ok(디버그·릴리스) | 갈래 2 보존 단언 ✓(:4667 인벤토리 = `MINERAL_MINED.quantity_kg`, 이전 수락 없는 actor). 수락 ≤ 1 은 902 의 `COOLDOWN_ACTIVE` 대조(:4680)로 보인다. 변이: server(산출 두 번 반영 → `left 50, right 25`), 원복 sha256 일치 |
| SC-28 | FAIL | **PASS** (계약 이름 정정 뒤) | 정정 전: 필터 2 `recording_halted_rejects_state_change` → 0 건, 레포 어디에도 없는 이름. 정정 뒤 계약 명령 셋 그대로: `recording_lag_boundary_*` 1 · `mine_resource_is_not_rejected_with_recording_backlog_when_lag_is_within_the_limit` 1 · `mine_resource_passes_when_persist_backlog_is_high_but_recording_lag_is_zero` 1, 전부 ok | 내용은 충족한다. lag **정확히 21** ⊘ 단언(ws.rs:1002, `RECORDING_BACKLOG_LIMIT + 1`) → `RECORDING_BACKLOG` ✓. **같은 상태(halted 아님)** `SET_SHIP_CONTROL` 큐 진입 ✓(:1038~1065). halted → 거부·이동 큐 진입 ✓(:1067~1144, 첫 필터 테스트 안). 20 통과 ✓·persist 20·lag 0 통과 ✓ — 계약에 새로 지명한 두 테스트. 변이: qa MA(임계 +1) → `:1018` 실패. qa MB(게이트를 모든 명령에 적용) → `:1056 lag 21(halted 아님)에서도 SET_SHIP_CONTROL …` 실패. 원복 sha256 일치 |

### 판정 메모 (FAIL 로 삼지 않은 것)
- **SC-09 ⊘ 문구 "산출 ≥ 2"**: 테스트는 `effective_remaining > 0`(:3988) 로 단언한다. 실제 입력은 결정적으로 30 kg(회복 5 kg/10 tick × 60 tick)이다. 산출이 1 이어도 사유 단언(:3993 `CapacityExceeded`)이 실패하므로 공짜 통과 경로는 없다. 기록만 한다.
- **SC-09 사유 2(TARGET_UNKNOWN)** 는 쿨다운이 걸린 상태에서 시험한다. 계약 괄호는 "쿨다운 사유 외는 쿨다운이 끝난 시점에서" 를 요구한다. 판정 순서상 2 가 3 보다 앞이라 도달하고, 쿨다운 기준 불변 단언도 그대로 작동한다. r2 도 ✓ 로 판정했다. 계약 외 신규 요구로 올리지 않는다.
- **SC-10 6+7 의 "둘 다"** 는 구조적으로 만들 수 없다. 산출 = min(yield, 잔량)(simulation.rs:1257)이라 잔량 ≤ 0 이면 산출이 0 이고, 판정 7 에 닿지 않는다. 테스트는 소진 + 상한급 yield 로 근사한다(:4157). 스펙 §4.2 의 "이웃 쌍 다섯" 중 6+7 은 순서 단언만 의미가 있다 — 다음 스펙 정리 후보로만 적는다.
- SC-09 "사유별 관찰 수 6개를 찍는다": 출력은 없다. 대신 사유별 라벨 단언 6 묶음이 있다. r2 판정 기준을 그대로 따른다.

## 3. 계약 필터 실행 결과

`cargo test -p <crate> --locked -- <필터의 * 앞부분>` 를 필터마다 따로 돌렸다(`run_filters.sh`).

| 결과 | 필터 |
|---|---|
| 실행 0 건 (정정 전) | **SC-28 `recording_halted_rejects_state_change`**(유령 — 아래). 정정 뒤에는 0 건 |
| 실행 ≥ 1, 실패 0, 그 SC 의 테스트만 | 나머지 58(SC-09 새 필터 2 건 · SC-09 게이트웨이 1 · SC-05 16 · SC-50 2 · 그 외 1) |
| 후보(정정 전 참고 실행 → 정정 뒤 계약 명령이 됨) | SC-10 `mining::cooldown_before_range_in_judgement_order` 1 · SC-28 `mine_resource_is_not_rejected_with_recording_backlog_when_lag_is_within_the_limit` 1 · `mine_resource_passes_when_persist_backlog_is_high_but_recording_lag_is_zero` 1 |

- **유령 이름이 r1·r2 에서 안 보인 이유(도구 결함 — §3a):** `cargo_sc_map.py` 의 정규식은 `` `cargo test -p X 필터` `` 꼴만 줍는다. SC-28 방법 칸의 `+ \`recording_halted_rejects_state_change\`` 같은 맨 이름은 줍지 않는다. r3 에서 §1 방법 칸의 맨 백틱 이름을 전부 코드의 fn 이름과 대조했다. 테스트 이름 유령은 이것 하나다(나머지 후보는 필드·지표·메시지 이름).
- **qa 도구 결함 재발:** 필터 목록을 Python 으로 만들 때 Windows 표준 출력이 CRLF 를 붙였다. 1차 실행은 `*` 없는 필터가 전부 0 건이었다. `filters.bad_crlf/` 에 남겨 두고 판정에는 쓰지 않았다. CR 을 제거하고 다시 돌렸다. r2 와 같은 결함이므로 `run_filters.sh` 가 입력의 `\r` 을 스스로 지우도록 고쳐야 한다(qa 소유, 다음 슬라이스).
- 정정 전에는 계약 명령이 SC 의 테스트를 다 실행하지 않는 경우가 있었다(유령은 아님): SC-10 의 스펙 필수 쌍 3+4 와 SC-28 의 20·persist 20 쪽. 정정 뒤에는 계약 명령으로 실행된다.

### 3a. 도구 결함 — `cargo_sc_map.py` 의 필터 추출 (리더 조건 (2))

- **결함 1 — 맨 이름 누락:** 정규식 `` `cargo test -p ([\w-]+)(?: --[\w-]+)* ([^`\s]+)` `` 은 `cargo test -p` 로 시작하는 백틱만 줍는다. 방법 칸에 `` + `name` `` 처럼 맨 이름으로 지명한 테스트는 실행 대상에서 빠진다. 그래서 SC-28 의 유령 이름이 r1·r2 에서 0 건으로 드러나지 않았다.
- **결함 2 — 값을 받는 플래그 뒤 필터 누락:** `` `cargo test -p X --test ws_integration name` `` 꼴이면 `--test` 의 값을 필터로 읽는다. 그러면 닫는 백틱이 맞지 않아 행 전체를 버린다. 계약의 SC-09 gateway 줄은 `--test` 없이 적어 이 결함을 피했다.
- **결함 3(qa 실행기) — CR · IFS:** 필터 목록에 CR 이 붙거나(Windows Python 표준 출력), 빈 열이 IFS 탭에 접혀 열이 밀리면 실행이 0 건이 된다. 무효 실행은 판정에 쓰지 않고 `filters.bad_crlf/` 와 `filters_final.bad_ifs/` 에 남겼다.
- **고친 스캔:** `scripts/contract_scan_fixed.py` 는 ① cargo 명령(값을 받는 플래그 포함)과 ② 맨 백틱 이름을 모두 줍는다. 맨 이름은 코드 전체와 대조해 넷으로 나눈다.
  - `TEST_FN`: 테스트 fn 과 정확히 일치(`*` 는 접두로 비교)
  - `NON_TEST_IDENT`: 필드·지표·메시지 이름
  - `NON_CODE`: 명시 목록. 지금은 `check_violation` 하나(PostgreSQL 오류 조건 이름, SQLSTATE 23514)
  - `GHOST`: 코드 어디에도 없는 이름
  - `TEST_FN` 은 정의 위치로 크레이트를 정해 실행 목록에 넣는다. 실행기는 `scripts/run_filters_final.sh` 다(CR 제거, 빈 열 자리 채움).
- **전수 결과 (리더 조건 (3)):**

| 계약본 | cargo 필터 | 맨 이름 | 그중 테스트 | 유령 | 증거 |
|---|---|---|---|---|---|
| 정정 전(`contract_before_sc09_edit.md`) | 57 | 41 | 14 | **1** (SC-28 `recording_halted_rejects_state_change`) | `scan_before.txt` · `scan_before/` |
| 정정 뒤(현재) | 61 | 43 | 15 | **0** | `scan_final.txt` · `scan_final/` |

  - 맨 이름으로만 지명돼 옛 실행 목록에서 빠졌던 Rust 테스트 이름은 8 개다: SC-05 ×3, SC-09, SC-20 `ambiguous_commit_retry_counts_once`, SC-100 `cross_world_same_command_id_accepted`, SC-101 `payload_null_rejected_by_db`, SC-50 `rule_golden_detects_value_change_without_version_bump`. 전부 존재하고, 계약 명령 그대로 각 ≥ 1 건 실행·ok 다. 같은 누락이 다른 SC 에서 내용 결함을 숨긴 경우는 없다.
  - Unity 맨 이름 7 개(SC-67·69·70·71 ×2·110 ×2): client 소스는 r2 이후 sha 가 바뀌지 않았으므로 r2 실행 xml 로 확인했다. EditMode 5(`UnknownClosedValue_*` 는 5 케이스)와 PlayMode 2, 모두 passed 다(`scan_final/unity_bare_in_r2_xml.txt`).
- **남은 일 (qa, 동결 해제 뒤):** `tests/e2e/cargo_sc_map.py` 에 결함 1·2 수정을 반영하고, selftest 에 맨 이름·`--test X` 케이스를 추가한다. r3 에서는 동결 범위라 증거 디렉터리의 스크립트로만 돌렸다. `run_filters.sh` 의 CR·IFS 처리도 함께 고친다.

## 4. SC-28 — 유령 이름과 결정 (리더 결정 A)

- **원인:** 계약 방법 칸이 `recording_halted_rejects_state_change` 를 지명했는데, 그런 테스트는 없다. halted 단언은 S10 (a) 때 `recording_lag_boundary_…` 안에 들어갔다(ws.rs:1067). 계약 이름을 만든 쪽도, 이름을 맞춘 쪽도 이 이름을 대조하지 않았고, qa 도구는 맨 이름을 줍지 않았다(§3a).
- **행동 결함은 0 이다.** 거부·통과·halted·이동 수락 다섯 조건이 모두 실행되고, 임계 변이와 게이트 범위 변이 둘 다 잡힌다.
- **결정(리더):** A. 계약 방법 칸만 정정하고 코드는 그대로 둔다. SC-28 은 실제 이름 셋으로 바꾸고, SC-10 에는 `mining::cooldown_before_range_in_judgement_order` 를 추가했다. 변경 이력에는 SC-09 와 같은 사유로 한 줄씩 남겼다.
- **조건 (1):** 새로 지명한 이름을 모두 계약 명령 그대로 실행했다. SC-28 셋, SC-10 하나, SC-09 gateway 하나가 각 ≥ 1 건 실행·ok 다(`filters_final/filters_summary.tsv`). **조건 (2)·(3):** §3a.
- 판정: **PASS.**

## 5. 동결 중 사건

| 시각 (KST) | 사건 | 처리 |
|---|---|---|
| 00:20 | qa 판정용 서버 바이너리 빌드(`cargo build -p starfall-game-server`) | 소스 편집이 아니다. 테스트 외 변경을 판단하는 수단으로는 쓰지 못했다(§1) |
| 00:22~00:25 | qa 판정용 구현 변이 4(ws.rs 2, simulation.rs 2) | 매번 원본 바이트로 되돌렸다. sha256 이 변이 전과 같다. 전체 소스 433 sha 도 재대조 일치(`mutations/post_mutation_src_check.txt`). 게이트 실행(00:25:51~) **전에** 끝냈다 |
| 00:23 | 계약 SC-09 1 행 + 변경 이력 1 행 편집(리더가 허용한 유일한 편집) | 편집 전 사본 `contract_before_sc09_edit.md` |
| 00:38~00:45 | 필터 실행 1차 CRLF 무효 → 재실행 | §3 |
| 00:46~00:51 | 리더 결정 A·SC-09 두 줄 지시 반영: 계약 SC-09·10·28 방법 칸 + 변경 이력 3 행 편집(문서만). 고친 스캔 전수, 필터 69 재실행(1차는 IFS 열 밀림으로 무효 → `filters_final.bad_ifs/`) | 소스 433 sha 재대조 일치(`src_sha_end2.txt`), 증거 DB 지문 불변(`fp_after2.txt`) |

## 6. 경계면·게임 특화 위험 (회귀)

r2 §6 과 같다. 바뀐 파일은 테스트 영역뿐이고, 게이트·반복 10·census·증거 DB 지문 모두 r2 와 같거나 +1(새 테스트)이다.

| 점검 | 결과 | 메모 |
|---|---|---|
| 서버 판정 우회 · 복사·멱등 · 결정성 · 기록 불변 · 역사 쓰기 경로 | PASS(r2 인용 + r3 게이트) | 복사·멱등 단위층(SC-17·19·22)이 이제 계약 이름으로 실행된다 |
| 경제 상한(ADR-0013 §5a) | PASS | 사유 7 이 sim(상한 − 1 주입)과 실제 소켓 경로(카운터 +1) 양쪽에서 보인다 |

## 7. 계약 외 발견

1. **`mine_resource_capacity_exceeded_is_counted_in_commands_rejected_total` 의 시간 여유:** `TestServer` 는 배치를 커밋하지 않는다. 그래서 `recording_lag` 이 벽시계와 함께 자라고, 2회차 명령은 300 ms 대기 뒤 lag ≤ 20 tick(20 ms 간격 → 약 400 ms) 안에 도착해야 `CAPACITY_EXCEEDED` 가 된다. 로컬에서는 11/11 통과했다(게이트 1 + 반복 10). 느린 CI 러너에서는 `RECORDING_BACKLOG` 로 실패할 수 있다. 공짜 통과가 아니라 거짓 실패 쪽 위험이다. SC-98 CI 첫 실행 때 지켜볼 항목이다.
2. **필터 추출기 결함**: §3a(도구 결함)로 옮겼다.
3. r2 §7 의 1~3(DB 테스트 표식 누락, notice-gap 우연 의존, 원본 데이터 부하)은 그대로다.

## 8. 최종 집계 (114)

| 구분 | 수 | 항목 |
|---|---|---|
| PASS | 112 | r2 PASS 106 + r3 SC-09 · 10 · 17 · 19 · 22 · 28 |
| FAIL | 0 | — |
| 미검증 | 2 | SC-68: 사람 Unity 세션(`sc68_human_session_procedure.md`, 사용자 일정) · SC-98: CI 첫 실행(커밋·푸시 뒤) |

- 태스크 #21(Q1b CI 출처 게이트 `--slice`·gates.yml): **pending.** 커밋·푸시 전이라 진행할 수 없다. SC-98 과 함께 Phase 6 커밋 뒤 첫 CI 실행에서 5 체크를 확인한다.
- 라운드 수: r3 / 최대 3. FAIL 0. r3 의 계약 정정 3건은 모두 방법 칸의 이름 정합이다. 판정 기준·⊘·코드는 바뀌지 않았다.

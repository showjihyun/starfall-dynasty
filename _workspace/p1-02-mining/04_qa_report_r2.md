# p1-02-mining QA 리포트 — 라운드 2

- 일시: 2026-10-03 19:03 ~ 23:59 KST (동결 시작 10:03:25Z)
- 기준: `02_sprint_contract.md` (114 항목). r2 중 계약 변경은 SC-15 경로 표기 1건(문서만, §12)
- 빌드: HEAD `e0037a4` + 작업 트리(커밋 전). 소스 436 파일 sha256 — 동결 시작 = r2 끝(`src_sha_start.txt` = `src_sha_end.txt`). 서버 바이너리 `cargo build -p starfall-game-server` 19:04, sha `79cc25f7…`
- 증거: `evidence/r2_20261003/`
- **요약: PASS 106 / FAIL 6 / 미검증 2 / 전체 114**
  - FAIL (전부 server, 단위 테스트층): SC-09 · SC-10 · SC-17 · SC-19 · SC-22 · SC-28
  - 미검증: SC-68(사람 세션) · SC-98(CI 첫 실행 — 커밋·푸시 전)
  - r1 대비: FAIL → PASS 1 (SC-59), 미검증 → PASS 3 (SC-36 · SC-87 · SC-89), 회귀 0

## 1. 이번 라운드에서 본 것

| 단계 | 결과 | 증거 |
|---|---|---|
| 부하 12 분 (×100 사본, 리더 결정 (d)) | 유효 1회. 1차는 동결 위반과 겹쳐 무효(§5) | `load/` · `load1_ABORTED/` |
| 서버 게이트 | fmt 0 · clippy 0 · workspace test **296 passed / 0 failed / 6 ignored** · release sim **80 passed** | `gates_summary.txt` · `test.log` · `release.log` |
| 반복 10 회 | 10/10 rc=0, 매회 296 passed, 실행 중 편집 0 | `repeat/summary.txt` |
| DB 테스트 이름 대조 (SC-113) | 계약 27 / RAN 27 / 누락 0 / SKIPPED 0 | `census.json` |
| 계약 필터별 실행 | 57 필터 각각 `cargo test -p <crate> <필터>` — **0 건 실행 6**, 다른 SC 의 테스트를 잡는 필터 1 (§3) | `filters/filters_summary.tsv` |
| 봇 전 케이스 + 파이썬 판정 | 전부 PASS (notice-gap 1차 겹침 0 → 재시도 1회, r1 과 같은 처리) | `bots/` · `bots/py/summary.txt` |
| 정적·오프라인 | 출처 게이트 p1-02·p1-01 · selftest 16 · data/ 스키마 · 경계면 표 · 커버리지 `--strict` · golden sha r1 동일 · SC-94 장부 7 = r1 · 제약 census · SC-109 탐침 · 코드 생성 2회 | `offline/summary.txt` · `offline/static.txt` |
| Unity (qa 직접, Editor 닫힘 확인) | EditMode **410 / 0 / skipped 2**(LiveServerTests 게이트) · PlayMode **2 / 0** | `unity/editmode-r2.xml` · `unity/playmode-r2.xml` |
| 증거 DB | spike 4805 · md5 `d1904d8f…` · migrations 3 · 테스트 DB 0 — 시작 = 끝, DB 정지 전 = 후 | `fp_before.txt` · `fp_after.txt` · `bots/backlog_fp_*.txt` |

## 2. 재판정 항목

| ID | r1 | r2 | 증거 | 비고 |
|----|----|----|------|------|
| SC-09 | FAIL | **FAIL** | `each_rejection_reason_leaves_prior_accepted_state_untouched` + `_reasons_3_6_7` ok(디버그·릴리스). 계약 필터 `mining::reject_*` 는 **SC-11 의 테스트 1건만** 잡는다 | 사유별 4 칸(인벤토리·광맥·쿨다운 기준 tick·장부) 중 빈 칸 남음 — §4 |
| SC-10 | FAIL | **FAIL** | 다섯 쌍 모두 존재(`adjacent_reject_reason_pairs_pick_the_earlier_one` 4쌍 + `cooldown_before_range_in_judgement_order` 3+4). 필터 `mining::reject_order_*` **0 건** | ⊘ 의 "쌍마다 3 단언(각 조건 단독 → 자기 사유)" 이 같은 테스트 안에 없다 — §4 |
| SC-17 | FAIL | **FAIL (이름)** | 내용 충족: `duplicate_command_id_across_reconnect_within_linger_window_is_rejected` — 수락 → CloseSession → 재개(SHIP_SPAWNED 없음 단언) → 같은 id → `DUPLICATE_COMMAND_ID`, 인벤토리 불변. 세션 id 1 ≠ 99. 구현 변이로 잡힘(03_server_impl.md). 필터 `mining::dup_across_reconnect` **0 건** | 계약 명령이 아무것도 실행하지 않는다 — 이름만 맞추면 PASS |
| SC-19 | FAIL | **FAIL (이름)** | 내용 충족: `same_actor_two_sessions_same_tick_duplicate` — s1 `SUPERSEDED` 를 먼저 단언(⊘), 수락 1 · 중복 1, 두 결과 tick 동일. 구현 변이로 잡힘. 필터 `mining::dup_two_sessions_same_tick` **0 건** | 위와 같음 |
| SC-22 | FAIL | **FAIL** | 갈래 1 충족. 갈래 2: 재접속 뒤 **원래 거절된 id(900)** 재전송 → 수락, 대조로 902 는 쿨다운. 구현 변이(거절 id 과잉 기억)로 잡힘. 필터 `mining::rejected_resend_*` **0 건** | 계약 "재접속 갈래는 수락 1, **그 뒤 보존 법칙 성립**" 의 보존 단언 없음 — §4 |
| SC-28 | FAIL | **FAIL** | persist 20·lag 0 → 통과, persist 25·lag 0 → 통과(보강), halted → `RECORDING_BACKLOG`·이동 통과. 필터 `recording_lag_boundary_*` **0 건** | **21 tick 경계 거부가 없다**(거부 테스트는 lag 25). "lag 21 상태의 `SET_SHIP_CONTROL` 수락" 도 halted 가 아닌 lag 초과 상태로는 없다 — §4 |
| SC-36 | 미검증 | **PASS** | `invalid_fixtures_layered` 1 건 실행. 신규 40 반례: 기대(스펙 §5.3, S1 전 작성 — architect 확인) = 실측 **40/40 거부**, 사유가 의도한 위반(유효 fixture 대비 변형)을 가리킴 40/40. 순서 대조 74/74 불일치 0 | `sc36_compare.txt` · `evidence/r2_sc36/fxdiff.txt` |
| SC-59 | FAIL | **PASS** | `history-replay --read-only`: r2 새 월드 `01a1013a…` 재생 6123 · 기록 4 PASS, 옛 월드 `01a0ef96…` 636 · 4 PASS, r1 trace `01a0ef85…` 14 · 1 PASS. `--read-only` 없이 → rc 1. 새 월드 `detector_rule = mineral-discovery@1` × 4. DB 테스트 `history_replay_matches_stored_records`·`_detects_content_mismatch`(음성 대조)·`_pool_rejects_writes` RAN | `sc59/` |
| SC-87 | 미검증 | **PASS** | ⊘ 발생: 31 연결 유지 · 수락 **5999** ≥ 3000(사전 등록 기대 ≈ 6000). tick 초과 **1 / 12010 = 0.0083 %** ≤ 0.5 % | `load/README.json` — 사본 sha·바꾼 필드·적재 줄·도달 가능성 |
| SC-89 | 미검증 | **PASS** | 같은 실행 `RECORDING_BACKLOG` 0, `recording_lag` 최대 8. 검출기 생존: 같은 빌드 backlog 케이스 PASS(SC-29 — lag 22 에서 첫 거부, K2b 직접 기준 성립) | `bots/backlog/backlog.json` |
| SC-15 | PASS | PASS | 계약 경로를 `mining::` 로 정정한 뒤 실행 1 건(`filters/SC-15_recheck.log`) | 원인: server 의 모듈 이름 변경(R1-S-2) |
| SC-69 | PASS(이름 불일치) | PASS | 병합 테스트 `InventoryPanel_ChangesOnlyOnInventoryState` 한 테스트 안에 구조(공개 변경자 = `ApplyInventoryState` 하나) → 동작(INVENTORY_STATE 뒤 Items 1·ferrosite·200) 순서. 출력 `public method: ApplyInventoryState` | 옛 이름 → 새 이름: `InventoryPanelState_StartsEmpty_AndHasNoOtherMutator` + `InventoryPanelState_ApplyInventoryState_ReplacesItemsFromMessage` → `InventoryPanel_ChangesOnlyOnInventoryState` |
| SC-110 | PASS(이름 불일치) | PASS | 새 이름 두 테스트 passed. 구현 변이 증거(동결 **전** 19:00~19:01): PilotTag 첫 4글자 변이 → `PilotLabel_LastFourChars` 만 실패, 표지 기준 병합 변이 → `DiscoveryList_KeysByActorIdNotLabel` 만 실패, 원복 sha256 = 변이 전(`evidence/r2_sc110/restore_sha.txt`) | `PilotTag_UsesLastFourCharacters_NotFirst` → `PilotLabel_LastFourChars`, `DiscoveryFeedState_CollidingPilotTags_DiscoverersStayDistinct_ByActorId` → `DiscoveryList_KeysByActorIdNotLabel` |

이름 정합 9건(server R1-S-2: SC-08·11·12·13·16·33·36·37·39)은 전부 계약 필터로 **실행 1 건 이상**이고 그 SC 의 테스트만 잡는다(SC-33 은 `--test mining_replay` 로 `sc33_two_process_replay_with_mining_is_byte_identical` 1 건 + 자식 전용 worker ignored). `mining_tests` → `mining` 모듈 이름 변경이 넓게 잡는 경우는 SC-09 하나다(§3).

## 3. 계약 필터 실행 결과 (57 필터)

`cargo test -p <crate> <필터>` 를 필터마다 따로 돌려 실행 수를 셌다(`*` 앞까지 부분 문자열 — cargo 는 글롭이 없다).

| 결과 | SC |
|---|---|
| 실행 0 건 | SC-10 `mining::reject_order_*` · SC-17 `mining::dup_across_reconnect` · SC-19 `mining::dup_two_sessions_same_tick` · SC-22 `mining::rejected_resend_*` · SC-28 `recording_lag_boundary_*` · (SC-15 — 계약 정정 뒤 1 건) |
| 다른 SC 의 테스트를 잡음 | SC-09 `mining::reject_*` → `mining::reject_does_not_start_cooldown`(SC-11) 1 건만. SC-09 의 두 테스트(`each_rejection_reason_…`)는 잡지 않는다 |
| 나머지 50 필터 | 실행 ≥ 1, 실패 0, 그 SC 의 테스트만 |

이 여섯은 r1 의 "이름 정합 14건" 목록에 없었다 — r1 에서 내용 FAIL 이라 이름 대조표에 올리지 않았고, R1-S 는 "계약이 지명한 테스트 이름은 그대로" 라는 r1 지시를 r1 의 **매핑된 이름**으로 읽었다. 내 r1 지시가 두 가지를 구분하지 않은 탓이다.

## 4. 수정 요청 (r3) → server

공통: 계약 방법 칸의 필터로 **실행 1 건 이상, 그 SC 의 테스트만**. 테스트 이름을 바꾸거나, 리더 결정으로 계약 필터를 바꾼다(코드는 그대로 두고 계약만 고치는 길도 있다 — 어느 쪽인지 리더가 정한다).

### SC-09
- 위치: `server/crates/sim/src/simulation.rs:3599`(`each_rejection_reason_leaves_prior_accepted_state_untouched`), `:3723`(`_reasons_3_6_7`)
- 기대(계약): 사유 2~7 **각각** 거부 뒤 인벤토리·광맥·쿨다운 기준 tick·처리 장부 불변. 7 은 적재량을 `MassKg` 상한 − 1 로 주입하고 `commands_rejected_total{CAPACITY_EXCEEDED}` +1 을 단언.
- 실제(빈 칸):

| 사유 | 인벤토리 | 광맥 | 쿨다운 | 장부 |
|---|:-:|:-:|:-:|:-:|
| 2 TARGET_UNKNOWN | ✓ | ✓ | ✓ | ✓ |
| 3 COOLDOWN_ACTIVE | ✓ | ✓ | ✓ | ✓ |
| 4 TARGET_OUT_OF_RANGE | ✓ | ✓ | ✓ | **✗** |
| 5 SHIP_TOO_FAST | ✓ | ✓ | **✗** | **✗** |
| 6 RESOURCE_DEPLETED | ✓ | ✓ | **✗** | ✓ |
| 7 CAPACITY_EXCEEDED | ✓ | **✗**(회복 때문에 뺐다는 주석 — 대기 뒤 스냅샷으로 가능) | **✗** | ✓ |

  - 7 의 주입은 인벤토리 = 상한(i32::MAX) 그 자체이고 상한 − 1 이 아니다. `commands_rejected_total{CAPACITY_EXCEEDED}` +1 단언은 워크스페이스 어디에도 없다(`grep CapacityExceeded` 는 매핑 한 줄뿐).
- **내 r1 지시의 결함**: r1 수정 요청에 "실제: 2·4·5 만" 이라고 써서 4·5 를 완성으로 잘못 알렸다. 이번 빈 칸 중 4·5 는 그 탓이다.

### SC-10
- 위치: `simulation.rs:3908` · `:3186`
- 기대(계약 ⊘): 쌍마다 **같은 테스트에서** 각 조건 단독 → 자기 사유를 먼저 단언하고(쌍마다 3 단언) 그다음 둘 다 → 앞 사유.
- 실제: 다섯 쌍 모두 "둘 다 → 앞 사유" 만 단언하고, 단독 조건은 "다른 테스트가 덮는다" 주석으로 갈음했다. 예: 5+6 쌍은 잔량 0 이 실제로 `RESOURCE_DEPLETED` 를 만드는지 같은 테스트에서 보이지 않으므로, 소진 판정이 죽어 있어도 `SHIP_TOO_FAST` 로 통과한다. 그것이 이 ⊘ 가 막으려던 모양이다.
- 내 r1 지시는 "인접 쌍 5 전부" 만 적고 3 단언을 적지 않았다 — 지시 누락.

### SC-17 · SC-19
- 내용은 충족. 필터 0 건만 고치면 된다.

### SC-22
- 위치: `simulation.rs:4277` 갈래 2
- 기대: 재접속 갈래는 "재판정이 실제로 **수락**됐음을 찍는다(수락 1, **그 뒤 보존 법칙 성립**)".
- 실제: 수락과 뒤이은 쿨다운은 단언하지만, 인벤토리 = 산출 1 회분(보존)을 단언하지 않는다. 한 줄이면 된다.

### SC-28
- 위치: `server/crates/gateway/src/ws.rs:984`(`mine_resource_is_rejected_with_recording_backlog_when_lag_exceeds_the_limit`)
- 기대(계약): `recording_lag` **21** 에서 `MINE_RESOURCE` → `RECORDING_BACKLOG`, **같은 상태** `SET_SHIP_CONTROL` → 수락. 20 에서 통과. persist 20 · lag 0 에서 통과. halted 에서 거부·이동 수락.
- 실제: 거부 쪽 입력이 lag **25** — 임계가 21~25 사이 어디여도 통과한다(20 통과 테스트와 합쳐도 경계를 못 박는다). `SET_SHIP_CONTROL` 수락은 halted + lag ≤ 20 상태에서만 본다 — "lag 21, halted 아님" 상태의 이동 수락이 없다.
- r1 지시에 "recording_lag 21 경계 거부" 를 적었는데 반영되지 않았다. `SET_SHIP_CONTROL` 절은 r1 지시에 없었다(내 누락, 계약에는 있다).

## 5. 동결 중 사건

| 시각 (KST) | 사건 | 처리 |
|---|---|---|
| 19:02~19:04 | **client-2 동결 위반** — 동결 공지 뒤 SC-110 변이 확인을 sha256 으로 다시 돌렸다. 클라이언트 2 파일 변이·원복(PilotTag.cs 19:03:22, DiscoveryFeedState.cs 19:04:07), Unity 3 회(xml 19:02:57·19:04:01·19:04:30) | 첫 부하 실행(19:04 시작)과 ~30 초 겹침 → **무효**, bots.exe 만 종료하고 서버는 stop 파일로 정상 종료(exit 0), 재실행. 두 파일 해시는 변이 전과 같고 동결 시작 스냅샷도 같은 값. 리더 확인: 공지와 작업이 엇갈려 생긴 일. SC-110 증거는 동결 전 실행이 정본 |
| 19:05~19:17 | 재실행 부하 | 15 초 감시 — Unity·cargo·rustc 프로세스 0, 실행 시작 뒤 수정된 소스 0 |
| 19:17~22:45 | **qa 대기 공백**(3.5 시간) | 부하 완료 알림을 놓치고 턴을 끝냈다. 리더 지적 뒤 같은 턴 안 확인 루프로 바꿨다 |
| 23:03~23:04 | 리더의 하네스 문서 수정(`CLAUDE.md`·`.claude/agents/*`·`.claude/skills/*`) | 판정 대상 코드가 아니다. 소스 436 파일 해시 불변 |
| — | qa 도구 결함: 필터 실행 스크립트가 Windows Python 출력의 CRLF 를 필터 끝에 붙여 1차 실행이 거의 전부 0 건 | `filters.bad_crlf/` 로 남기고(판정에 안 씀) 고쳐 재실행 |

## 6. 경계면·게임 특화 위험 (회귀)

| 점검 | 결과 | 메모 |
|---|---|---|
| 계약 ↔ Rust (반례 74) | PASS | SC-35·36 — 74/74 거부, 신규 40 사유 ↔ 의도 일치 |
| 계약 ↔ C# | PASS | 코드 생성 2회 17 파일 sha 불변(SC-65), EditMode 410 |
| 서버 판정 우회 | PASS | `MINE_RESOURCE` payload 속성 = `{deposit_id}`, `additionalProperties: false`(SC-79), 봇 cheat 5 케이스 PASS |
| 복사·멱등 | PASS(실서버) / FAIL(단위층 이름) | 봇 mine-dup·재접속·재기동·cross-actor 5 PASS. 단위층 SC-17·19 는 내용 충족, 이름만 |
| 결정성 | PASS | SC-33 두 프로세스 재생, golden sha r1 동일, SC-59 재생 3 월드 |
| 기록 불변 | PASS | 제약 census PASS, SC-109 `domain_events_payload_is_object`(23514) — 대조 `{"x":1}` 은 INSERT 0 1, 둘 다 롤백, 남은 행 0. SC-94 자기 참조 장부 7 = r1 |
| 역사 쓰기 경로 | PASS | SC-95: INSERT 3 곳 전부 `persistence/src/history.rs` 의 러너 쓰기 함수(514·559·569) |

## 7. 계약 외 발견

1. **DB 테스트 표식 누락**: `ran_but_not_in_contract` 에 9 이름 — `history_replay_*` 3 개(SC-59 의 짝, 계약 방법 칸은 CLI 라 표식 대상이 아님), `admin_shutdown_is_transient`(SC-103 이 이름을 적었지만 ` [DB]` 표식이 없어 census 가 추적하지 않음), 그 밖 5. census 판정에는 영향 없음(누락 0). SC-103 의 표식 추가는 계약 쪽 정리 대상.
2. **notice-gap 의 ⊘ 발생이 우연에 기댄다**: 1차 겹침 델타 0(r1 도 같았다) → 재시도 1회에 델타 2. 판정은 같은 규칙(r1 선례)으로 했지만, 봇이 겹침을 결정적으로 만들지 못한다. 다음 슬라이스 전 봇 개선 후보.
3. **원본 데이터 부하(r1 과 같은 설정)**: 수락 512 · RESOURCE_DEPLETED 5488 — 판정에는 SC-86·88·90(회복·장부·발견) 용으로만 썼다. SC-87·89 는 ×100 사본 실행이 판정.

## 8. 이전 라운드 대비

- FAIL → PASS: SC-59
- 미검증 → PASS: SC-36 · SC-87 · SC-89
- FAIL 유지: SC-09 · SC-10 · SC-22 · SC-28(내용 빈 칸) · SC-17 · SC-19(이름만)
- 회귀(PASS → FAIL): 0
- 미검증 유지: SC-68(사람 세션 — 사용자 일정) · SC-98(CI 첫 실행 — 커밋 뒤)
- 라운드 수: r2 / 최대 3. r3 에 남은 FAIL 6 은 전부 단위 테스트의 단언·이름이고, 실행 동작 결함은 0 이다(실서버 판정은 전부 PASS).

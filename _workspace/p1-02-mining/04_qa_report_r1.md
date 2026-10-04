# p1-02-mining QA 리포트 — 라운드 1 (Phase 5)

- 일시: 2026-09-30 (UTC 2026-09-29T23:18:38Z 시작 ~ 2026-09-30 00:30Z 경)
- 기준: `02_sprint_contract.md` (r2, 114 항목, §7b 규칙 1~9 승계)
- 트리: HEAD `e0037a43084ced7a44353f4c48b44beef6a292d8`, `git status --porcelain` **158 줄**(이 브랜치의 미커밋 변경 — 규칙 9: 더러운 트리를 그대로 적는다). client-2 의 Unity 실행도 같은 HEAD·158 줄.
- 편집 동결: 리더 공지·전원 확인 뒤 시작. 판정 중 구현 소스(server·client/Assets·contracts·data·tools/bots/src) mtime 변화 **0**(시작 시각 이후 새 파일 없음 — `find -newer start.txt`). 예외 기록: qa 가 r1 도중 `tests/e2e/cargo_sc_map.py`(집계 도구, 판정 입력 아님)를 만들었고, SC-90 계약 칸에 도구 지명 한 문장을 넣었다(리더 결정 (a), §12 이력).
- 서버 바이너리: `cargo build -p starfall-game-server` → sha256 앞 16 `15e76690585febcd`, 기동 전 test-hooks 문자열 검사 통과, 마이그레이션 동결 대조(파일 1~3 = 적용 1~3) 통과.
- 증거 루트: `evidence/r1_20260930/` (이하 상대 경로). client 결과: `evidence/editmode-20260930.xml`, `evidence/playmode-20260930.xml`.

## 요약

**PASS 102 / FAIL 7 / 미검증 5 / 전체 114**

| 구분 | 항목 |
|---|---|
| FAIL (server) | SC-09 · SC-10 · SC-17 · SC-19 · SC-22 · SC-28 |
| FAIL (history) | SC-59 |
| 미검증 — 계약 전제 결정 필요 | SC-87 · SC-89 (⊘ "수락 ≥ 3000" 이 데이터 매장량으로 도달 불가 — qa 계약 결함, 리더 결정 대기) |
| 미검증 — 대기 | SC-36 (§0.5 ② 열 미작성 — 계약이 "표가 채워지기 전에는 대기" 로 정함) |
| 미검증 — 사람 | SC-68 (사람 세션 — 절차서 `sc68_human_session_procedure.md`, 사용자 시각 대기) |
| 미검증 — CI | SC-98 (PR 의 `judgment gates` job 로그 필요 — 아직 PR/CI 실행 전, 로컬 게이트는 p1-02·p1-01 모두 exit 0) |

## 항목별 결과

표기: `log:N` = `test.log` N 번째 줄(워크스페이스 `STARFALL_DB_TESTS=required cargo test --workspace --locked --no-fail-fast`, 290 passed / 0 failed / 6 ignored). **이름 불일치 PASS** 는 리더 조건대로 계약 이름과 다른 테스트가 **이번 r1 로그에 실행·통과 줄로 있을 때만** 인정했다(대응표는 아래 절).

| ID | 결과 | 증거 | 비고 |
|----|------|------|------|
| SC-01 | PASS | `gates_summary.txt`: fmt rc0 · clippy rc0 · test 290/0/6 · release sim 78/0. 반복 10/10 초록(`repeat/summary.txt`, 회차별 편집 창 0), ws_integration 매회 25/25 | history 가 본 ws_integration 1회 실패는 r1 반복 10회에서 재현 안 됨 |
| SC-02 | PASS | sim `[dependencies]` = starfall-contracts 만, history = contracts·serde·serde_json·uuid(v5)·thiserror | grep 2 적중은 history/Cargo.toml:20-21 주석 |
| SC-03 | PASS | `SystemTime`·`Instant` 호출 0. `HashMap` core.rs:60·71·88-89·114 — insert·entry 만, 순회 없음 | |
| SC-04 | PASS | `sc04_boot.json` 광물 4·광맥 8·`mineral-discovery@1` = 파일 수, stdin shutdown exit 0 | 새 월드 |
| SC-05 | PASS | cargo `data::tests::reject_c01`~`c16` 16 건 ok(log:70~97) + `accepts_unmodified_copy` ok(log:78). 실바이너리 ①⑦⑪ `sc05_reject.json` 3/3 + 무변경 사본 음성 대조 | 계약 필터 `data::reject_c` 는 경로 표기 오류(모듈 `tests::` 누락, qa 계약 몫) |
| SC-06 | PASS | `validate_data_files.py` rc 0 | |
| SC-07 | PASS | `sc07_pairs.json` 10/10 짝(기동 로그 `데이터 파일 적재` × 스키마 × 유도 게이트) | |
| SC-08 | PASS (이름 불일치) | `simulation::tests::mining_tests::accepted_mine_produces_event_and_both_messages` log:353 — causation_id·before/after·인벤토리·광맥 드러남 단언(simulation.rs:3269~3315 qa 확인) | |
| SC-09 | **FAIL** | ⊘ "거부 전 상태가 비어 있지 않음 + 거부 뒤 불변" 을 하는 테스트(`each_rejection_reason_leaves_prior_accepted_state_untouched`)가 사유 2·4·5 만 덮는다. 3(COOLDOWN_ACTIVE)·6(RESOURCE_DEPLETED)·7(CAPACITY_EXCEEDED)은 사유 코드만 확인 | 수정 요청 참고 |
| SC-10 | **FAIL** | 인접 쌍 5 중 3 만 존재(쿨다운+사거리, 사거리+과속, 소진+상한). (TARGET_UNKNOWN+COOLDOWN)·(SHIP_TOO_FAST+RESOURCE_DEPLETED) 없음 | |
| SC-11 | PASS (이름 불일치) | `rejection_does_not_start_cooldown` log:368 | server 감사 확인 |
| SC-12 | PASS (이름 불일치) | `partial_yield_when_remaining_is_less_than_extraction_yield` log:365 | fixture `partial-last-kg-depletes.json` 은 계약 테스트가 읽음 |
| SC-13 | PASS (이름 불일치) | `same_tick_mining_is_ordered_by_submission_sequence_not_ship_id` log:373 + `reversing_submission_order_reverses_the_winner` log:369 | 둘을 합쳐 계약 문장 충족(server 감사) |
| SC-14 | PASS (경로 표기) | `mining::tests::regen_closed_form_matches_stepwise_simulation` log:348 | |
| SC-15 | PASS (경로 표기) | `simulation::tests::mining_tests::range_uses_pre_integration_state` log:362 | |
| SC-16 | PASS (이름 불일치) | `same_session_resend_is_rejected_as_duplicate` log:372 | |
| SC-17 | **FAIL** | 잔류 창 안 재접속 뒤 같은 command_id → DUPLICATE 를 보는 **sim 테스트가 없다**(`reconnect_within_linger_window_resumes_the_same_ship` 는 이동 테스트) | 실서버판(봇 SC-78 b)은 PASS — 계약이 지명한 단위층이 비었다 |
| SC-18 | PASS | `restart_reloads_economic_state_so_cas_succeeds_and_duplicates_are_rejected` ok (S8, r2 이름 정정) | |
| SC-19 | **FAIL** | `same_actor_two_sessions_same_tick_duplicate` 는 주석과 달리 세션 **하나**(id(1))만 연다 — 계약의 "같은 actor 의 **두 연결**(넘겨받기)" 을 만들지 않는다 | server 감사로 발견 |
| SC-20 | PASS | `batch_recommit_is_noop` ok, `ambiguous_commit_retry_counts_once` ok | Phase 5 관찰 대상 둘(반환값 직접 단언·전 테이블 불변 단언 없음)은 `qa_phase5_watchlist.md` — **판정 문구 대비 약한 단언**으로 기록, 결과는 PASS 유지(카운터 +1·행 불변 핵심 단언 존재) |
| SC-21 | PASS | `dup_reaching_db_halts` ok + `dup_across_restart` ok | |
| SC-100 | PASS | `duplicate_command_id_across_different_sessions_is_rejected` ok + `dup_cross_actor_no_halt` ok | |
| SC-22 | **FAIL** | `rejected_command_resend_branches` 갈래 2 가 재접속 뒤 **새** id(902)로 시험 — 원래 거절된 id(900) 재전송 경우가 없다 | |
| SC-23 | PASS | `py/sc23_ledger_cases.json` 키 3·이벤트 22·연쇄 19 | 치트 케이스 월드 |
| SC-24 | PASS | `py/sc24_conservation_cases.json` 광물 2·광맥 3·실제 회복 1 | |
| SC-25 | PASS | `cas/cas-halt.judge.json`: 변조 150→927 뒤 채굴 → **스스로 종료 exit 1**, 정지 로그 한 줄에 사유·키·`expected=Some(150) actual=Some(927)`·`persist_fatal_total`, 그 채굴 MINERAL_MINED 0 행, 대조 실행 정지 없음·exit 0. ⊘: 변조 전 채굴 수락·커밋 1, 둘째 채굴 영속화 도달, 손잡기 시간 초과 0 | **(a) 두 층**: 정지 tick(1205) **뒤** 판정된 채굴 **0/0**(분모 0 — 서버가 정지 tick 직후 종료해 관측 창이 없었다; 이 층은 단위 테스트 `mine_resource_is_rejected_with_recording_backlog_when_lag_exceeds_the_limit` 의 persist_halted 블록이 맡는다). **설계상 손실 창**(정지 tick 에 ACCEPTED 로 답하고 기록 안 됨) **1 건** — FAIL 로 세지 않음(ADR-0013) |
| SC-26 | PASS | `cas/cas-reload.json`: 선행 정지 exit 1 → 재기동(`last_tick=Some(…)`) → 세션 시작 인벤토리 927 = 변조 값(정지 전 메모리 300 과 다름 — ⊘) → 채굴 수락, `quantity_before_kg` 927. DB 층 `restart_reloads_…`·`load_economic_state_*` 2 ok | |
| SC-27 | PASS | `recorded_at_failure_fails_batch` ok | |
| SC-101 | PASS | `payload_serialize_failure_fails_batch` ok | |
| SC-102 | PASS | `no_commit_after_fatal` ok | |
| SC-103 | PASS | `admin_shutdown_is_transient` ok(+ `_fatal_pair`) | |
| SC-112 | PASS | `terminated_backend_is_retried` ok | |
| SC-28 | **FAIL** | 계약 ⊘ 의 핵심 대조 "두 지표가 갈리는 입력(persist_backlog=20 · recording_lag=0)에서 MINE_RESOURCE 통과" 를 만드는 테스트가 없다. `ws::tests::mine_resource_is_(not_)rejected_with_recording_backlog_*` 는 recording_lag 만 조작 | 실서버 SC-104 는 PASS(톱니 20·lag 0) — 계약이 지명한 게이트웨이 단위층이 비었다 |
| SC-29 | PASS | `backlog/backlog.json` 정지 전 수락 3 → RECORDING_BACKLOG 8(거부 시점 lag 65~535 > 20) | 정지 전후 증거 DB 지문 동일 `4805\|d1904d8f…\|3` |
| SC-30 | PASS | 정지 구간 26.8 s: SET_SHIP_CONTROL 수락 245, 스냅샷 269 | |
| SC-31 | PASS | 재개 뒤 lag ≤ 20 이후 수락 13 | |
| SC-32 | PASS | `py/sc32_accepted.json` 봇 수락 17 ⊆ processed_commands·MINERAL_MINED(누락 0,0). ⊘ 정지 중 수락 **1**(봇 JSON) | |
| SC-104 | PASS | `idle/backlog-idle.json` 채굴 20/20 수락, 거부 0, persist_backlog 최대 20(톱니 있음), recording_lag 최대 0 | architect S11 기준(≤5 + 톱니) 충족 — 별도 필드 |
| SC-33 | PASS (이름 불일치) | `sc33_two_process_replay_with_mining_is_byte_identical` log:433 | 계약은 `--test mining_replay`, 실제 파일 determinism_mining |
| SC-34 | PASS | golden sha 824a…/c924…/3e00… = §0.7 기준선, git 변경 없음 | |
| SC-35 | PASS | `sc35_contracts_nocapture.log`: 레지스트리 23, 유효 fixture 46 왕복, 스키마 29, 반례 74 | |
| SC-36 | 미검증(대기) | `invalid_serde_matrix` ok(log:51) — 반례 74 기대/실제 표 출력. 그러나 **§0.5 ② 열이 비어 있다** → 계약상 대기 | server 가 테스트 출력으로 ② 열을 채우면 즉시 판정 가능 |
| SC-37 | PASS (이름 불일치) | `required_field_mutations` log:52 — "[p1-02 SC-37] required 변이 445건 모두 실패" | |
| SC-38 | PASS | `registry_responses` ok — "명령 kind 타입 3건, responses 채워진 타입 3건" | |
| SC-39 | PASS (이름 불일치) | `deposit_field_state_mixes_unconfirmed_and_revealed_and_stays_revealed_after_full_regen` log:359 | |
| SC-40 | PASS | `leak/leak-scan.json` 첫 채굴 전 프레임 전부(INVENTORY_STATE 1·DEPOSIT_FIELD_STATE 8항목 넷 null) 누출 적중 0, 양성 대조 ≥ 1 | |
| SC-41~50 | PASS | history 코어 테스트 11 개 ok(`first_extraction_discovers`…`rule_golden` + 음성 대조 `rule_golden_detects_value_change_without_version_bump`) — `cargo_map.json` | 각 1 매칭·1 ok |
| SC-51~57 | PASS | 러너 DB 테스트 7 개 ok(RAN 줄 census 포함) | |
| SC-105·106·111 | PASS | `history_conflict_halts_runner_only`·`history_redelivery_is_silent`·`history_same_source_different_content_halts` ok | 한 파일 같은 실행 |
| SC-58 | PASS | `py/sc58_oracle_load.json` 오라클 4 = 역사 4, NULL 행 0 | 부하 월드 |
| SC-59 | **FAIL** | 계약 지명 도구 `cargo run -p starfall-persistence --bin history-replay` 가 **없다**(워크스페이스 `[[bin]]` 은 game-server 하나) | history |
| SC-60 | PASS | `trace/trace-abc.json` sc_60: 열린 세션 2 = LIVE 받은 세션 2 | |
| SC-61 | PASS | `trace/trace-c.json` sc_61: READY 뒤 BACKFILL | |
| SC-62 | PASS | `history_notice_after_commit_only` ok | |
| SC-63 | PASS | server `notice_no_gap_same_tick` ok + 봇 `gap1/notice-gap.json` 세션 100, 둘 다 못 받은 세션 0, 겹침 델타 1 | 1차 시도 `gap/` 은 델타 0 → **무효**(재시도 1회) |
| SC-64 | PASS | `load/mine-load.json` sc_64 표본 124·기록 4, p50 44 ms, max 53 ms ≤ 5 s | 새 월드 — 러너 따라잡기 표본 없음 |
| SC-65 | PASS | 생성기 2회, 17 파일 sha 전·1회·2회 동일(`unchanged`), 신규 DTO 6 | |
| SC-66 | PASS | EditMode `Fixtures_RoundTrip_Found46_RoundTripped36` passed, `CSharpLayerSplit_Totals74` passed | §0.5 ③ 열 전사는 안 됨(client 후속) |
| SC-67 | PASS | EditMode `UnknownClosedValue_*` 5 passed | |
| SC-68 | 미검증(사람) | — | 절차서 준비 완료, 사용자 시각 대기 |
| SC-69 | PASS (이름 불일치) | EditMode `InventoryPanelState_StartsEmpty_AndHasNoOtherMutator`·`…_ApplyInventoryState_ReplacesItemsFromMessage` passed + 소스 grep 0 | |
| SC-70 | PASS | PlayMode `DiscoveryFeed_DedupesByHistoricalEventId` passed | |
| SC-110 | PASS (이름 불일치) | EditMode `PilotTag_UsesLastFourCharacters_NotFirst`·`DiscoveryFeedState_CollidingPilotTags_DiscoverersStayDistinct_ByActorId` passed | |
| SC-71 | PASS | (A) `DepositTableEntry_HasNoServerOnlyFields` (B) PlayMode `DepositMarker_FollowsMessageNotDataCopy` passed | |
| SC-72 | PASS | `ClientDataCopy_MatchesRepositoryOriginal` passed — 원본 독립 순회 10 · 사본 10 (+ 음성 대조 `ClientDataCopy_CopyOnlyFile_IsDetected_NegativeControl`) | |
| SC-73 | PASS | `cases/cheat-mine-inject.json` 원문 4/4 에 주입 필드, 4/4 MALFORMED_COMMAND, 인벤토리 사전 채굴 뒤 불변 | |
| SC-74 | PASS | `cases/cheat-mine-range.json` 5/5 TARGET_OUT_OF_RANGE, 보낸 시점 거리 > 한계 | |
| SC-75 | PASS | `cases/cheat-mine-fast.json` 사거리 안 > 10 m/s → SHIP_TOO_FAST | |
| SC-76 | PASS | `cases/cheat-mine-cooldown.json` 수락 ≤ 21·≥ 2, 나머지 COOLDOWN_ACTIVE, RATE_LIMITED 0, 연결 유지 | |
| SC-77 | PASS | `cases/cheat-mine-unknown.json` TARGET_UNKNOWN(케밥 형식 id) | |
| SC-78 | PASS | (a) `cases/mine-dup.json` (b) `dup/mine-dup-reconnect.json` 같은 함선 이어받기 (c) `dup/mine-dup-restart.resume.json` 재기동 로그 줄·processed_commands 불변 | |
| SC-108 | PASS | `dup/mine-dup-cross-actor.run.json`·`.resume.json` — 둘째 DUPLICATE, 10 s 뒤 stats 응답·정지 없음·커밋 전진(재기동 뒤 변형 포함) | |
| SC-79 | PASS | 스키마 payload properties = {deposit_id}, additionalProperties false + SC-73 주입 거부 | |
| SC-80 | PASS | trace-abc sc_80: 기록 0 행에서 시작, A·B 같은 LIVE, 발견자 A | |
| SC-81 | PASS | sc_81: B 수락·인벤토리 증가·추가 LIVE 0·SQL 기록 1 | |
| SC-82 | PASS | trace-c sc_82: A LINGER_EXPIRED 1 행, C 이전 세션 0, BACKFILL 로 A 의 발견·광맥 드러남 | |
| SC-83 | PASS | `py/sc83_oracle_*` 부하 월드 4 광물 + trace 월드 glacine: 각 기록 1·근거 = 첫 채굴·증거 1 | |
| SC-84 | PASS | `race/race-same-tick.json` 같은 tick, 발견자 = sequence 작은 쪽, 기록 1 | 무효 시도 수 기록 |
| SC-85 | PASS | `lastkg/last-kg.json` 같은 tick, 식(I-69) 잔량 100, 100/DEPLETED/DEPLETED, 인벤토리 +100 | |
| SC-86 | PASS | `py/sc86_regen_load.json` 광물 4 모두 보존, 실제 회복 광맥 8 | |
| SC-87 | 미검증(계약 전제) | `load/mine-load.json` 31 연결 유지, tick 초과 **0/12004**. ⊘ "수락 ≥ 3000" 미달(512) | 매장량 총 ~59,100 kg 로 **구조적으로 도달 불가** — qa 계약 결함. 리더 결정 대기 |
| SC-88 | PASS | `py/sc88_load.json` 원장·보존(키 31) | |
| SC-89 | 미검증(계약 전제) | RECORDING_BACKLOG 0, recording_lag 최대 0 — 판정은 SC-87 과 같은 부하 ⊘ 에 묶임 | 검출기 생존 증거: 같은 빌드의 backlog PASS |
| SC-90 | PASS | `py/sc90_discoveries_load.json` 채굴 광물 4 = 발견 행 4, 같은 집합, 고갈 120 | 도구 지명 r2 보강(§12) |
| SC-91 | PASS | `py/sc91_*.txt` 치트·trace·부하 월드 모두 5 종 전부 ≥ 1 | |
| SC-92 | PASS | `py/sc92_causation_*` 부하 512 = 512, 치트 22 = 22, 중복 0 | |
| SC-93 | PASS | `py/sc93_gaps_*` 위반 그룹 0(부하 494 tick, 치트 39 tick) | |
| SC-94 | PASS | 자기 참조 7 행 = p1-01 장부 | |
| SC-95 | PASS | 운영 코드 INSERT 는 persistence/src/history.rs 하나(나머지는 테스트) | |
| SC-96 | PASS | `check_contract_coverage.py --strict` RESULT PASS | |
| SC-97 | PASS | `sc97.json` 10 타입 141 행 불일치 0 | |
| SC-98 | 미검증(CI) | 로컬 출처 게이트 p1-02 exit 0(114/114 지명, 유령 0, Rust 항등식 ①② OK), p1-01 회귀 exit 0 | PR 의 `judgment gates` job 실행 뒤 |
| SC-99 | PASS | trace-abc A 1/1·B 1/1, mine-load 짝 불일치 봇 0(수락 512) | |
| SC-107 | PASS | `sc107.json` 104 제약 + 트리거 4, 표별 일치 | |
| SC-109 | PASS | 모든 NOT NULL 을 채운 행에 payload `'null'` → `domain_events_payload_is_object` 위반. 대조: 같은 행 `{"x":1}` 은 INSERT 성공(롤백). 남은 행 0 | 1차 시도는 schema_version 누락으로 다른 이유에 막혀 무효 |
| SC-113 | PASS | `census.json` 27/27 RAN, 누락 0, SKIPPED 0 | |
| SC-114 | PASS | spike 4805·md5 d1904d8f…·migrations 3 불변, 테스트 DB 잔여 0, `tests::refuses_non_test_database` ok | |

(SC-41~50·51~57 은 행을 묶었다 — 항목별 매칭·통과는 `cargo_map.json`.)

## 수정 요청

리더 방침: r1 뒤 server 에게 한 번에 배정. 파일:라인은 server 의 읽기 전용 감사(2026-09-30)와 qa 확인.

### SC-09 → server
- 위치: `server/crates/sim/src/simulation.rs` `mining_tests::each_rejection_reason_leaves_prior_accepted_state_untouched`
- 기대: §4.2 사유 2~7 **각각**에서 "거부 전 상태가 비어 있지 않음" 을 먼저 단언하고 거부 뒤 인벤토리·광맥·쿨다운·장부 불변.
- 실제: 2·4·5 만. 3·6·7 은 사유 코드만.

### SC-10 → server
- 기대: 인접 쌍 5 전부(표 순서 앞 사유).
- 실제: (TARGET_UNKNOWN+COOLDOWN_ACTIVE)·(SHIP_TOO_FAST+RESOURCE_DEPLETED) 없음.

### SC-17 → server
- 기대: `mining::dup_across_reconnect` — 잔류 창 안 재접속(새 세션, 같은 actor) 뒤 같은 명령 → DUPLICATE (sim 수준).
- 실제: 해당 테스트 없음.

### SC-19 → server
- 위치: `same_actor_two_sessions_same_tick_duplicate` — `open(&mut sim, &mut ids, id(1), id(2))` 하나만, 두 제출 모두 `session_id: id(1)`.
- 기대: 같은 actor 의 **두 세션**(넘겨받기) 이 같은 tick 에 같은 command_id → 수락 1·중복 1, 두 결과 tick 동일.

### SC-22 → server
- 위치: `rejected_command_resend_branches` 갈래 2 — 재접속 뒤 id(902)(새 id).
- 기대: 원래 **거절된** id(900) 를 재접속 뒤 재전송 → 새로 판정(수락 ≤ 1).

### SC-28 → server
- 기대: 두 지표가 갈리는 입력(persist_backlog = 20, recording_lag = 0 — 하트비트 톱니 꼭대기)에서 MINE_RESOURCE 통과, 그리고 recording_lag 21 경계 거부.
- 실제: recording_lag 만 조작하는 두 테스트.

### SC-59 → history
- 기대: `cargo run -p starfall-persistence --bin history-replay -- --world <id> --read-only` — 읽기 전용 재생, 기록·evidence id 비교, 재생 이벤트 수·비교 기록 수(≥ 2) 출력.
- 실제: 바이너리 없음.

## 결정 필요

1. **SC-87·SC-89 ⊘** (qa 계약 결함): "수락 채굴 ≥ 30 × (600/3) × 0.5" 는 광맥 총 매장량(약 59,100 kg, 산출 25~200 kg)으로 도달 불가 — 10 분에 약 390 회 + 회복분. r1 실측 수락 512 · RESOURCE_DEPLETED 5,489 · 모든 명령이 판정 경로를 탐. 추천: ⊘ 를 "**판정된** 채굴 명령 ≥ 3000 + 봇마다 수락 ≥ 1" 로(판정 기준 tick 초과 ≤ 0.5 %·거부 0 은 불변). 결과를 본 뒤의 변경이라 사후성을 리포트에 명시해야 한다.
2. **SC-36**: §0.5 ② 열을 server 가 채울지(테스트 출력 74 행이 있다).

## 이름 불일치 대응표 (계약 → 실제 → r1 로그 줄)

| SC | 계약 방법 칸 | 실제 테스트 | log |
|---|---|---|---|
| SC-05 | `data::reject_c…` | `data::tests::reject_c01`~`c16`, `data::tests::accepts_unmodified_copy` | 70~97, 78 |
| SC-08 | `mining::accept_*` | `simulation::tests::mining_tests::accepted_mine_produces_event_and_both_messages` | 353 |
| SC-11 | `mining::reject_does_not_start_cooldown` | `…::rejection_does_not_start_cooldown` | 368 |
| SC-12 | `mining::partial_then_depleted` | `…::partial_yield_when_remaining_is_less_than_extraction_yield` | 365 |
| SC-13 | `mining::same_tick_submission_order` | `…::same_tick_mining_is_ordered_by_submission_sequence_not_ship_id` + `…::reversing_submission_order_reverses_the_winner` | 373, 369 |
| SC-14 | `mining::regen_closed_form_matches_stepwise` | `mining::tests::regen_closed_form_matches_stepwise_simulation` | 348 |
| SC-15 | `mining::range_uses_pre_integration_state` | `simulation::tests::mining_tests::range_uses_pre_integration_state` | 362 |
| SC-16 | `mining::dup_same_session` | `…::same_session_resend_is_rejected_as_duplicate` | 372 |
| SC-33 | `--test mining_replay` | `sc33_two_process_replay_with_mining_is_byte_identical`(determinism_mining) | 433 |
| SC-36 | `invalid_fixtures_layered` | `invalid_serde_matrix` | 51 (판정은 대기) |
| SC-37 | `required_removal_mutations` | `required_field_mutations` | 52 |
| SC-39 | `mining::deposit_field_state_reveal` | `…::deposit_field_state_mixes_unconfirmed_and_revealed_and_stays_revealed_after_full_regen` | 359 |
| SC-69 | `InventoryPanel_ChangesOnlyOnInventoryState` | `InventoryPanelState_StartsEmpty_AndHasNoOtherMutator` + `…_ApplyInventoryState_ReplacesItemsFromMessage` | editmode-20260930.xml |
| SC-110 | `PilotLabel_LastFourChars` · `DiscoveryList_KeysByActorIdNotLabel` | `PilotTag_UsesLastFourCharacters_NotFirst` · `DiscoveryFeedState_CollidingPilotTags_DiscoverersStayDistinct_ByActorId` | editmode-20260930.xml |

이름 정합(계약을 이름에, 또는 이름을 계약에)은 r1 뒤 정리 항목. 계약 쪽 경로 표기 오류(SC-05·14·15)는 qa 몫.

## 경계면 점검

| 경계면 | 결과 | 메모 |
|---|---|---|
| 계약 ↔ Rust ↔ C# | PASS | SC-97 141 행 불일치 0, SC-35 46 fixture 왕복, EditMode 411/0 |
| 명령 ↔ 서버 판정 | PASS(실서버) / 단위층 FAIL 6 | 치트 6·중복 3 경로 실서버 PASS. 단위 테스트 틈은 위 수정 요청 |
| 도메인 → 역사 | PASS | 오라클 = 역사(부하 4), 발견 1/광물, 근거 = 첫 채굴 |
| 역사 → 게이트웨이 → 클라이언트 | PASS | LIVE·BACKFILL 누락 0(세션 100, 겹침 1), 재배달 중복 표시 1회(PlayMode) |
| DB ↔ 코드 | PASS | 제약 104 + 트리거 4, NULL payload CHECK 실제로 막음, 키 존재 단언 14 곳 |
| 즉시 응답 ↔ 최종 결과 | PASS / 설계상 손실 창 1 | 정지 tick 의 ACCEPTED 1 건은 ADR-0013 손실 창 |

## 게임 특화 위험

| 위험 | 결과 | 메모 |
|---|---|---|
| 서버 판정 우회 | 막힘 | 수량·광물·위치·actor 주입 4/4 MALFORMED, 사거리·속도·쿨다운·없는 광맥 각 사유 |
| 아이템 복사 | 막힘 | 같은 세션·재접속·재기동·다른 actor 중복 모두 DUPLICATE, 원장·보존 항등식(치트 월드·부하 월드), processed_commands ⊇ 수락 |
| 멱등성 | PASS | 배치 재커밋·모호한 커밋·역사 재처리·재배달 |
| 결정성 | PASS | 채굴 포함 2 프로세스 재생 동일, 이동 golden 불변 |
| 기록 불변 | PASS | 역사 3 표 UPDATE/DELETE 거부, 추가 전용 트리거 |
| 정지 경로 DoS | 막힘 | 다른 actor 같은 id(재기동 전후) → DUPLICATE, 서버 생존 |

## 계약 외 발견 (판정에 넣지 않음)

- **설계상 손실 창 실측**: 영속화 정지 tick 에 ACCEPTED 로 답했지만 기록되지 않은 채굴 1 건(cas-halt). ADR-0013 의 "응답이 영속보다 먼저" 창. 크기는 recording_lag ≤ 20 이 묶는다.
- **SC-25 (a) 실서버 분모 0**: 서버가 정지 tick 직후(50 ms 내) 종료해 "정지 뒤 도착한 명령" 이 판정될 창이 실서버에는 없다 — 단위 테스트가 유일한 관측.
- **ADR-0013 K2b 직접 기준**(architect 관측 전 등록): 첫 대기 배치 tick 1297 → lag 21 표본 tick 1318(delta 21 ≤ 21 + 표본 간격 2) **성립**. 정의 항등식 위반 0/262, 이른 교차 0.
- **test-hooks 가 워크스페이스 빌드 exe 에 섞임**(Phase 4 발견, server-db 문서화 중): 실서버 판정 exe 는 `-p` 빌드로 다시 만들고 기동 전 문자열 검사로 거른다.
- r1 도중 qa 도구 결함 발견·수정 없음. (Phase 4 의 qa 도구 결함 — trace-c 의 `reason` 키, cas-halt 쿨다운, backlog 분류기 2회 — 은 모두 r1 전에 고쳤고 r1 에서 정상 동작 확인.)

## 이전 라운드 대비

r1 이 첫 라운드다. Phase 4 잠정 PASS(치트 6·leak-scan·trace·재기동 5·race·last-kg·backlog-idle·backlog·notice-gap)는 전부 이 트리에서 다시 돌려 같은 결과다. 새로 깨진 것(회귀) 없음.

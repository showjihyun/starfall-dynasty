# p1-02-mining — H1·H2 구현 (역사 엔진, `mineral-discovery@1`)

작성: history · 2026-09-27~28

**주의**: §1 아래의 표·§1.1·§1.2·§1.4 는 실수로 파일을 덮어써서(2026-09-28) 그 시점 최종 상태로 다시 쓴 것이다. §1.3~§1.3d 는 team-lead 지시로 **대화 기록에서 재구성**했다(각 절 머리에 표시) — `_workspace` 는 추적되는 감사 기록이고 이 슬라이스 폴더는 아직 커밋 전이라 git으로 복구할 수 없었다. 재구성한 절의 인용 로그(패닉 출력 등)는 대화에 남은 도구 출력을 그대로 옮긴 것이지 기억으로 채운 숫자가 아니다. §2 부터는 원본이 유실된 적 없다(H2 는 처음부터 이 파일에 썼다). **앞으로 이 파일은 Write 로 통째로 쓰지 않고 Edit 로 덧붙인다.**

범위: 태스크 #16(H1, 완료) + #17(H2, 완료 — main.rs 배선은 server 몫으로 남음). `02_sprint_contract.md` §J·§K·§M(AC-10·AC-11·AC-13(c)).

## 1. H1 — 역사 판정 코어 (`server/crates/history`)

| 파일 | 내용 |
|---|---|
| `src/id.rs` | 결정적 id 파생. `NS_HISTORY`·`NS_EVIDENCE`(ADR-0014 §1 고정값), `historical_event_id()`, `evidence_id()`. 단위 테스트가 Python `uuid.uuid5` 독립 계산값과 일치를 건다 |
| `src/input.rs` | `DomainEvent`/`DomainEventKind` — S1 의 계약 타입 `starfall_contracts::events::MineralMinedEvent` 를 `Box` 로 감싼다(`Box` 는 clippy `large_enum_variant` 때문). `Unjudged` 변형은 좌표만 가진 Level 0 통과용 |
| `src/output.rs` | `MineralDiscoveredDraft`(계약 `historical::MineralDiscoveredEvent` 에서 `recorded_at` 만 뺀 모양 + `dedupe_key` 필드, `into_event(recorded_at)` 로 완성), `EvidenceDraft`(계약에 와이어 타입이 없는 부분), `MineralDiscoveredRecord { event, evidence }` |
| `src/core.rs` | `HistoryCore` — `SignificanceRuleTable` 을 직접 받는다(자체 하드코딩 규칙 상수 없음 — qa 지적으로 `DiscoveryRuleConfig`/`v1()` 삭제). `new()`(빈 상태) / `rebuild()`(H2 기동 재구축용, 월드 하나의 발견 집합 + 마지막 좌표를 받는다) / `judge()` / `judge_all()`. `CoreError::OutOfOrder` |
| `tests/mineral_discovery.rs` | SC-41~46·48~50(§J). **규칙 값의 원천은 파일 하나**(`discovery_rule()` 헬퍼가 매 테스트마다 `data/history/rules/mineral-discovery.json` 을 파싱) — 하드코딩 사본이 없어 "코드에만 있는 값이 파일과 갈라지는" 종류의 결함이 구조적으로 불가능하다 |
| `tests/data/first-extraction-evidence-id.txt` | SC-41 기대 evidence_id — Python 독립 계산 |
| `tests/data/golden/mineral-discovery@1.{hash,output}.txt` | SC-50 golden |

### 1.1 판정 설계

`HistoryCore` 는 "같은 tick 안 sequence 비교"를 따로 하지 않는다. 호출자(러너)가 `(tick, sequence)` 오름차순으로 이미 정렬해 넘긴다는 전제(ADR-0014 §2) 위에서 **역행만 거부**하고 "먼저 온 것이 이긴다"로 발견자를 정한다 — SC-44 는 event_id 순서와 sequence 순서가 반대인 두 이벤트를 (tick, sequence) 로 정렬해 먹여서 이 사실을 검증한다.

### 1.2 golden(SC-50) — 해시는 원본 JSON 에 건다, Rust 타입 Debug 포맷이 아니다

초기 구현은 `SignificanceRuleTable` 을 `{:?}`(Debug) 로 찍어 해시했다. qa·팀장 지적: Debug 출력은 안정된 직렬화 계약이 아니다 — enum variant 이름이 바뀌거나 파생을 손으로 바꾸면 **규칙 값이 그대로여도** 해시가 흔들린다. 수정: `normalise_and_hash` 가 이제 **규칙 파일의 원본 JSON 텍스트**를 `serde_json::Value` 로 파싱해 `designer_note`(설명문, 값 아님)만 뺀 뒤 재직렬화한 바이트를 해시한다. `serde_json` 이 `preserve_order` feature 를 안 켜서 객체가 내부적으로 `BTreeMap` 이므로 재직렬화가 자동으로 키 정렬된다. 새 테스트 `hash_is_insensitive_to_json_key_order_and_whitespace` 가 "값에만 반응하고 모양엔 안 반응한다"를 직접 건다(키 순서를 통째로 뒤집어도 같은 해시).

재생성 근거(매번): (1) 규칙 파일 sha256 `9f04593b6cbfcfee1da56688240b3aa398c17813a55abeafe25b4af3d6470276` 불변(client 사본과 동일) (2) `.output.txt` 한 줄 불변(`2775a80a-2a8a-5615-a86f-859ea777901d|mineral-discovery@1|2|e9b611d1-637e-5287-862e-74eabffb0252`) (3) `.hash.txt` 만 해시 함수 변경으로 바뀜.

### 1.3 검증 — 변이로 RED 증명 (재구성, 2026-09-28 — 원본 §3·§4 가 파일 덮어쓰기로 유실돼 대화 기록에서 되살렸다. 정확한 숫자·경로는 대화에 남은 도구 출력에서 그대로 옮겼다 — 기억으로 채운 수치는 없다)

구현과 테스트를 같이 썼기 때문에(컴파일이 S1 대기로 늦게 열려 자연 RED 를 못 봤다), 빌드가 열린 뒤 **의도적으로 판정 로직을 부러뜨려 테스트가 실제로 그 실패를 재는지** 확인했다. 각각 부러뜨림 → 실패 관측 → 원복 → 초록.

**(a) id.rs — `NS_HISTORY` 마지막 니블 `d` → `e`**

```
thread 'first_extraction_discovers' panicked at crates\history\tests\mineral_discovery.rs:127:5:
assertion `left == right` failed
  left: "dc222552-85b7-569c-aec8-ee5eb4786fc2"
 right: "2775a80a-2a8a-5615-a86f-859ea777901d"
test first_extraction_discovers ... FAILED
```
(`id::tests::*` 단위 테스트 2개도 같이 실패했다 — 생략) 원복 후 `cargo test -p starfall-history` 전부 초록.

**(b) core.rs — "이미 발견됨" 집합 검사를 무력화**(`if !world.discovered.insert(..) { return Ok(vec![]) }` → 결과를 안 보고 항상 통과)

```
SC-42: 처리한 MINERAL_MINED 수 = 2 (분모), 두 번째 결과 = 1 건

thread 'second_extraction_is_silent' panicked at crates\history\tests\mineral_discovery.rs:194:5:
assertion `left == right` failed: 같은 광물의 재추출은 역사가 아니다
  left: 1
 right: 0
test second_extraction_is_silent ... FAILED
```
원복 후 초록.

**(c) core.rs — 역행 검사를 `if false { .. }` 로 무력화**

```
thread 'out_of_order_input_rejected' panicked at crates\history\tests\mineral_discovery.rs:401:5:
같은 좌표의 재입력은 Err 이어야 한다
test out_of_order_input_rejected ... FAILED
```
원복 후 초록. 세 변이 모두 원복 뒤 `cargo test -p starfall-history` 전부 초록으로 재확인했다.

### 1.3a qa 지적 — `v1()` 이 golden 에도 파일에도 묶여 있지 않았다 (재구성)

qa 가 H1 경계면을 검증하며 지적: `rule.rs`(이후 삭제됨 — §1.3b 참고)의 `DiscoveryRuleConfig::v1()` doc 주석이 "golden(SC-50)이 이 값과 파일의 일치를 지킨다"고 썼지만, 실제로 `rule_golden` 은 파일에서 만든 설정으로만 돌고 `v1()` 을 **한 번도 부르지 않았다**. 그런데 SC-41~49 전부가 `v1()` 로 돌았다 — golden 이 잡는 것은 "파일이 golden 과 같다"뿐이지 "SC-41~49 가 쓰는 값이 파일과 같다"가 아니었다(CLAUDE.md 검증 규율의 사례와 같은 모양의 결함).

1차 수정: `v1_matches_rule_file` 테스트를 신설해 `v1()`과 파일에서 만든 설정의 전체 필드 동등을 직접 단언했다. 변이 확인(`v1()`의 `visibility`를 Public→ParticipantsOnly로 바꿈):

```
thread 'golden::v1_matches_rule_file' panicked at crates\history\tests\mineral_discovery.rs:701:9:
assertion `left == right` failed: v1() 이 규칙 파일과 갈라졌다 — 파일이 바뀌면 v1() 도 같이 고쳐야 한다(SC-41~49 가 여전히 v1() 로 돌기 때문)
  left: DiscoveryRuleConfig { ..., visibility: ParticipantsOnly, ... }
 right: DiscoveryRuleConfig { ..., visibility: Public, ... }
test golden::v1_matches_rule_file ... FAILED
```
같은 변이에서 `golden::rule_golden` 은 **그대로 초록**이었다 — qa 가 지적한 틈을 정확히 재현했다.

### 1.3b 팀장 지시 — 탐지가 아니라 제거 (재구성)

team-lead 가 "`v1()`을 고치는 대신 아예 없애라"는 방향을 줬다. `HistoryCore::new()`가 `starfall_contracts::data::SignificanceRuleTable`을 직접 받도록 바꾸고 `DiscoveryRuleConfig`/`v1()`/`rule.rs` 자체를 삭제했다. `input.rs`/`output.rs`도 S1 계약 타입(`MineralMinedEvent`, `historical::MineralDiscoveredEvent`)으로 전면 교체했다(이 부분은 지금의 §1 표와 같다 — 그때 바뀐 결과가 지금도 최종 상태다). `v1_matches_rule_file` 테스트는 비교 대상 두 번째 사본 자체가 사라져 삭제했다.

이번엔 **실제 데이터 파일**(`data/history/rules/mineral-discovery.json`)의 `evidence.visibility` 를 `CLASSIFIED` 로 바꿔 재확인했다:

```
thread 'first_extraction_discovers' panicked at crates\history\tests\mineral_discovery.rs:134:5:
assertion `left == right` failed
  left: Classified
 right: ParticipantsOnly
test first_extraction_discovers ... FAILED

thread 'golden::rule_golden' panicked at crates\history\tests\mineral_discovery.rs:543:9:
assertion `left == right` failed: 규칙 파일 내용이 golden 해시와 달라졌다 — rule_version 을 올렸는가?
test golden::rule_golden ... FAILED
```
SC-41 **과** golden 둘 다 이 변화를 독립적으로 잡았다 — 구조가 하나로 합쳐졌으니 당연하지만, 실제로 관측했다. 원복 후 전부 초록.

### 1.3c 팀장 후속 지시 — 공유 추적 파일을 변이 시연에 직접 쓰지 않는다 (재구성)

§1.3b 의 시연은 추적 파일을 직접 고쳤다가 원복했다. team-lead 지적: 그 사이 client 의 SC-72 나 qa 의 `validate_data_files` 가 돌면 거짓 빨간불이 나고, 원복을 잊으면 증거가 오염된다.

수정 — `discovery_rule_path()`(실제 경로) / `discovery_rule_from_path(path)`(임의 경로에서 읽기) / `write_temp_rule_copy(rule)`(메모리 값을 OS 임시 디렉터리에 써서 경로를 돌려줌, `std::env::temp_dir()`) 를 추가했다. 새 테스트 2개(둘 다 추적 파일 무변경):
- `temp_rule_copy_round_trips` — 헬퍼 자체가 맞는지(쓰고 다시 읽으면 원본과 같다) 확인.
- `sc41_fails_when_evidence_visibility_mutates_in_a_temp_copy` — `evidence.visibility` 를 `CLASSIFIED` 로 바꾼 임시 사본으로 `HistoryCore` 를 만들어 판정 결과에 실제로 반영되는지 확인.

`cargo test -p starfall-history` 15건 전부 초록. `data/history/rules/mineral-discovery.json` 은 실행 전후 내용 불변(`visibility`·`evidence.visibility` 값 확인).

### 1.3d golden 해시 입력을 Debug 포맷에서 원본 JSON 으로 (재구성 — §1.2 의 배경)

§1.3c 정정 뒤 qa·team-lead 가 더 깊이 짚었다: `normalise_and_hash` 가 `SignificanceRuleTable` 을 `{:?}`(Debug) 로 찍어 해시했는데, Debug 출력은 안정된 직렬화 계약이 아니다 — enum variant 이름을 바꾸거나 파생을 수동 구현으로 바꾸면 규칙 값이 그대로여도 해시가 흔들리고, 와이어 문자열(`PUBLIC` 등)과 Debug 이름이 갈라져도 이 함수는 모른다.

수정은 §1.2 에 적은 그대로다(원본 JSON 파싱 + `designer_note` 제외 + 재직렬화 해시). 재생성 근거 세 가지(규칙 파일 sha 불변 `9f04593b…`, `.output.txt` 불변, `.hash.txt` 만 변경 — 원인은 해시 함수 변경)를 처음부터 기록했다. 새 테스트 `hash_is_insensitive_to_json_key_order_and_whitespace` 로 "값에만 반응하고 모양엔 안 반응한다"를 직접 걸었다. `cargo test -p starfall-history` 16건(단위 3 + 통합 13) 전부 초록 — 이것이 §1.4 에 적은 최종 수다.

### 1.4 게이트

`cargo test -p starfall-history` — 단위 3 + 통합 13 = **16건 전부 통과**. `cargo fmt -p starfall-history --check`·`cargo clippy -p starfall-history --all-targets -- -D warnings` 통과.

---

## 2. H2 — 러너·역사 저장소 (`server/migrations/0003_history.sql`, `server/crates/persistence/src/history.rs`)

### 2.1 마이그레이션 0003

4개 표(ADR-0014 §3 그대로 — participants/objects 정규화, claims, relations, chronicle 은 미룸):

| 표 | 요지 |
|---|---|
| `historical_events` | PK `historical_event_id`(결정적 UUIDv5), `UNIQUE(world_id, event_type, dedupe_key)`, `source_event_ids UUID[]`(≥1), `location`/`participants`/`payload` JSONB, `fact_status` CHECK `= 'CONFIRMED'` |
| `historical_event_sources` | PK `(source_event_id, detector_rule)` — 근거 유일성. `detector_rule` = `rule_id`(버전 없이). `source_event_id` → `domain_events(event_id)` FK(architect 권고, 2026-09-28 수락 — I-62 를 DB 구조로 강제) |
| `evidence` | `authenticity_status`·`creation_method`·`derived_from_evidence_ids` 를 CHECK 로 고정값(`VERIFIED`/`automatic`/`'{}'`)에 묶는다. `rule_version TEXT NOT NULL`(architect 지적 2026-09-28, ADR-0014 §5 "증거도 rule_version 을 가진다" — 첫 버전에 빠져 있었다. 증거 DB `starfall` 에는 아직 0003 이 안 걸려 있어 **파일을 직접 고쳤다**, 새 파일 불필요) |
| `history_cursor` | PK `(world_id, consumer)` — 유일하게 트리거 없는 가변 표 |

추가 전용 트리거 3개(세 표에), `forbid_history_mutation()` 함수가 `TG_TABLE_NAME` 으로 **실제 표 이름**을 에러 메시지에 넣는다(0001 의 `forbid_mutation()` 은 'domain_events' 를 문자 그대로 박아 놨었다 — SC-52 가 표 이름까지 확인하므로 재사용하면 거짓으로 통과한다).

**실측 제약 census**(테스트DB, 0001~0003 적용 후, `pg_constraint`/`pg_trigger`, evidence.rule_version·sources FK 추가 반영):

| | 1차(48) | 2차(현재, 50) |
|---|---|---|
| historical_events | 22(c4·f1·n15·p1·u1) | 22(변화 없음) |
| historical_event_sources | 5(f1·n3·p1) | **6**(f**2**·n3·p1 — FK +1) |
| evidence | 13(c3·f1·n8·p1) | **14**(c3·f1·n**9**·p1 — rule_version NOT NULL +1) |
| history_cursor | 8(c2·f1·n4·p1) | 8(변화 없음) |
| **합** | 48 | **50** |

트리거는 여전히 3개. architect 에게 50으로 갱신 요청을 다시 보냈다(ADR-0013 §5a-1 표가 이미 한 번 48로 갱신됐었다).

### 2.2 러너 설계 (`persistence/src/history.rs`)

- `load(pool, world_id, rule: SignificanceRuleTable) -> HistoryBoot` — `historical_events.dedupe_key` 전체에서 발견 집합을 재구축하고, `history_cursor` 를 읽고, `visibility='PUBLIC'` 전체를 tick 오름차순 BACKFILL 목록으로 돌려준다.
- `run_runner(pool, world_id, boot, commit_notify, live_tx, handles)` — `commit_notify.changed()` 또는 1초 타이머로 깨어나(ADR-0014 §2), 워터마크(`worlds.last_tick`)까지 커서 뒤의 `domain_events` 를 `(tick, sequence)` 순으로 읽어 판정한다. `commit_notify` 가 닫히면(정상 종료·정지) 마지막으로 한 번 더 따라잡고 조용히 종료한다.
- 판정 대상이 아닌 행(`event_type != MINERAL_MINED`)은 좌표만 코어에 넘긴다(Level 0). `MINERAL_MINED` 행은 `schema_version`·payload 역직렬화가 실패하면 **fail-stop**(정지, 커서 유지) — 업캐스팅 없음.
- 기록이 나오면 **한 트랜잭션**(기록 + 근거 + 증거 + 커서 전진)으로 쓴다. `ON CONFLICT (world_id, event_type, dedupe_key) DO NOTHING` 이 0행이면 기존 행과 **내용 전체**(`recorded_at` 제외)를 비교한다 — 같으면 조용히 커서만 전진(재배달), 다르면 롤백 + `conflicts_total` +1 + **러너만** 정지(ADR-0014 §4).
- 비교는 `ComparableEvent`(dedupe_key·rule_version·importance_level·tick·occurred_at·visibility·`Vec<Uuid>`·`serde_json::Value`×3) 의 `PartialEq` 로 한다 — `Vec` 동등은 순서 있음(배열), `Value::Object` 동등은 키 순서 없음(jsonb) — ADR 이 말하는 "jsonb·배열 동등"과 같다.
- `HistoryBoot::empty(rule)` — **테스트 전용**. 판정 상태를 DB 와 무관하게 비운다. `load()` 는 언제나 올바르게 재구축하므로 정상 경로로는 도달 불가능한 상태이고, "재구축 자체가 버그였을 때"(AC-11(h)(ii)(iii))를 시뮬레이션하는 것이 유일한 목적이다.
- `HistoryHandles` — `/debug/stats` 관측용(`records_total`·`conflicts_total`·`detector_halted_total`·`halted`·`cursor_tick`·`catchup_rows_total`).

### 2.3 DB 통합 테스트 (`persistence/tests/history_runner.rs`, `starfall-testdb`)

**12개**, 전부 `#[tokio::test]` + `starfall_testdb::TestDb::create("<계약 테스트 이름 그대로>")` — **접두사 없이**(qa 지적, 2026-09-28: 첫 버전은 `TestDb::create`에 `"persistence::history_..."`를 넘겨 RAN 줄이 `persistence::`로 시작했다. "계약 이름 그대로"라고 보고했지만 실행 출력은 달랐다 — 8건 전부 census 대조에서 누락으로 잡혔다. `let name = "history_..."`로 접두사를 뺐다):

| 테스트 함수 | SC | 요지 |
|---|---|---|
| `history_first_discovery_rows` | SC-51 | 첫 채굴 커밋 뒤 역사 3표 각 1행, `source_event_ids` 존재, evidence 플래그(SHIP_LOG·PARTICIPANTS_ONLY·VERIFIED·automatic·빈 derived_from·**rule_version**) |
| `history_tables_append_only` | SC-52 | 6개 UPDATE/DELETE 시도 전부 SQLSTATE `P0001` + 표 이름 포함 메시지로 거부, `history_cursor` UPDATE 는 성공(양성 대조) |
| `history_dedupe_unique` | SC-53 | 같은 dedupe_key 두 번째 행 SQL 직접 INSERT → `23505` |
| `history_fail_stop` | SC-55 | payload 에 `mineral_id` 없는 행 → 정지, 커서가 그 앞, 뒤 후보 미기록, 이후 `MINERAL_MINED` 커밋은 계속됨(경제 행 수로 확인) |
| `history_watermark` | SC-56 | `tick > last_tick` 인 행은 처리되지 않음(catchup_rows_total = 0) |
| `history_restart_rebuild` | SC-57, S-7 | 발견 2건 → "재기동"(새 `load()`+`run_runner()`) → BACKFILL 목록 2건 재현 + 신규 발견 1건 추가돼 합 3 |
| `history_conflict_halts_runner_only` | **SC-105만** | `HistoryBoot::empty()` 로 비운 러너가, **다른 근거**(injected 행의 source_event_ids ≠ 실제 이벤트)로 이미 DB 에 있는 dedupe_key 를 "최초"로 판정 시도 → 충돌 1건, 러너만 정지, 롤백, 경제는 계속 |
| `history_same_source_different_content_halts` | **SC-111만** | `HistoryBoot::empty()`, **같은 근거**(injected 행의 source_event_ids = 실제 이벤트)인데 payload 만 다름 → 충돌 1건, 나머지는 위와 같다 |
| `history_redelivery_is_silent` | SC-106 | 커서를 되감아 같은 근거·같은 내용을 재처리 → 정지 없음, 충돌 카운터 0, 행 수 불변 |
| `history_runner_ordering_permutations` | SC-47 | 광물 2×채굴자 3(같은 tick 충돌 1쌍 포함)을 순열 24개(요구 ≥20)로 매번 새 테스트 DB 에 삽입 순서만 바꿔 넣고 발견자 집합이 오라클과 항상 같은지 확인. ⊘ 셋 다 실측: 승자가 물리적으로 마지막 삽입된 순열 6/24, `ORDER BY` 없는 물리 순서가 정렬과 다른 순열 23/24, event_id≠(tick,seq) 순서 쌍을 입력 단계에서 단언 |
| `history_crash_before_commit` | SC-54 | 커밋 직전 장애 시뮬레이션(별도 트랜잭션에서 같은 INSERT 후 ROLLBACK — Postgres 원자성이 "커밋 직전 죽은 프로세스"와 같은 DB 상태를 보장) → 직후 기록 0·커서 없음 → "재시작"(새 load+run_runner) → 기록 1, 커서가 그 좌표 |
| `history_notice_after_commit_only` | SC-62 | 먼저 정상 처리로 LIVE 1건(⊘ 양성 대조), 그다음 evidence PK 를 미리 충돌시켜 커밋을 실패시킨 시나리오에서 LIVE 0건 |

**qa 지적 반영(SC-105/SC-111 분리)**: 첫 버전은 두 SC 를 한 테스트(`history_conflict_halts_runner_only`)에 합쳤다 — "재구축이 언제나 옳으므로 판정 상태를 비워야 DB 충돌에 닿는다"는 메커니즘이 같아서였다. qa 지적: 계약 규칙 5(SC 하나 = verdict 하나) 위반이고, 한 주입만 실제로 정지를 일으켜도 초록이 될 수 있다. `seed_conflicting_row` 헬퍼(injected `source_event_id` 하나만 매개변수로 받는다)를 공유하는 두 테스트로 나눴다 — SC-105 는 다른 근거, SC-111 은 같은 근거·다른 payload 를 주입하고, 각자 "주입 행이 있다 + 근거가 (다르다/같다)"를 먼저 단언한다(⊘).

**모두 매 테스트마다 다른 월드**(`fresh_world_id(테스트이름)`)를 쓴다(§0.2). 도메인 이벤트는 이 테스트 파일이 `domain_events` 에 **직접 SQL 로** 넣는다(커밋 경로 자체의 정확성은 S4 의 `economic_state.rs` 가 이미 잰다 — 이 파일은 "이미 커밋된 행이 있을 때 러너가 무엇을 하는가"만 잰다).

`history_crash_before_commit`(SC-54)의 방법론 메모: 진짜 프로세스 크래시는 이 테스트 하네스에서 못 만든다. 러너가 쓸 것과 같은 INSERT 를 별도 트랜잭션에서 실행해 트랜잭션 안에서 보임을 확인(주입 카운터 ⊘)한 뒤 COMMIT 대신 ROLLBACK 한다 — Postgres 트랜잭션 원자성 자체가 "커밋 직전에 죽은 프로세스"와 같은 DB 상태를 보장하므로 이것이 정확한 시뮬레이션이다.

### 2.4 검증 — 변이로 RED 증명

- `write_record` 의 내용 비교를 `if true`(항상 조용한 재처리로 취급)로 무력화 → `history_conflict_halts_runner_only` 가 "충돌 1건" 대신 0을 관측해 실패 → 원복.
- `catch_up` 의 fail-stop 처리(`return false`)를 `return true`(무시하고 계속)로 무력화 → `history_fail_stop` 이 "러너가 멈춰야 한다"에서 실패 → 원복.

원복 뒤 `cargo test -p starfall-persistence` 전부 초록(단위 3 + economic_state 5 + history_runner **12** = 20 — SC-47·54·62·111 분리 추가 후 수), `cargo fmt --check`·`cargo clippy -p starfall-history -p starfall-persistence --all-targets -- -D warnings` 통과. `-p starfall-history`(16) 를 더하면 **합 36건**.

### 2.3a RAN 줄 발췌 (qa·team-lead 요청, `STARFALL_DB_TESTS=required` 로 실행)

```
STARFALL_DB_TEST RAN history_first_discovery_rows
STARFALL_DB_TEST RAN history_crash_before_commit
STARFALL_DB_TEST RAN history_tables_append_only
STARFALL_DB_TEST RAN history_runner_ordering_permutations   (×24, 순열마다 같은 이름)
STARFALL_DB_TEST RAN history_redelivery_is_silent
STARFALL_DB_TEST RAN history_same_source_different_content_halts
STARFALL_DB_TEST RAN history_notice_after_commit_only   (×2, 양성 대조 + 주입)
STARFALL_DB_TEST RAN history_conflict_halts_runner_only
STARFALL_DB_TEST RAN history_watermark
STARFALL_DB_TEST RAN history_restart_rebuild
STARFALL_DB_TEST RAN history_fail_stop
STARFALL_DB_TEST RAN history_dedupe_unique

test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

증거 DB `starfall` 확인(위 실행 직후): `SELECT count(*) FROM domain_events` = **4805**(qa 가 보고한 기준선과 같다, 불변) — `starfall_test_*` 만 쓰는 코드 경로는 §2.5 에 그대로다.

### 2.5 증거 DB 보호

모든 DB 테스트가 `starfall-testdb`(`TestDb::create`/`drop`)만 쓴다. 로컬 실행 전후 `starfall` DB(`domain_events` 등)의 행 수 변화 없음을 확인했다(테스트가 붙는 DB는 항상 `starfall_test_<uuid>`).

---

## 3. 아직 남은 것 — server 배선 필요

`load()`/`run_runner()` 의 시그니처가 T0 골격(빈 `Ok`)에서 실제로 바뀌었다 — `load()` 는 이제 `rule: SignificanceRuleTable` 을 세 번째 인자로 받고, `run_runner()` 는 `boot: HistoryBoot`(러너가 컴파일러로 강제되는, load() 의 산출물)와 `handles: HistoryHandles` 를 받는다. 이 때문에 **`bins/game-server/src/main.rs` 가 지금 컴파일되지 않는다**(server 소유 파일이라 내가 고치지 않았다) — S2(완료)가 이미 `data/history/rules/mineral-discovery.json` 을 적재하고 있을 것이므로, main.rs 에서 그 값을 `history::load`/`history::run_runner` 호출에 그대로 넘기면 될 것이다. server 에게 정확한 diff 요청을 보냈다(§4).

`-p starfall-history`·`-p starfall-persistence` 범위는 독립적으로 전부 초록이다. `cargo test --workspace`/`cargo clippy --workspace` 는 main.rs 가 고쳐질 때까지 실패한다 — 이건 H2 가 낸 회귀가 아니라 시그니처 계약 변경의 자연스러운 결과이고, server 가 다음 스텝이다.

## 4. SC-59 — 읽기 전용 재생 도구 `history-replay` (R1-H, 2026-09-30)

Phase 5 r1 FAIL 로 재배정된 항목. `server/crates/persistence/src/bin/history-replay.rs`(Cargo 가 `src/bin/` 를 자동으로 바이너리 타깃으로 인식 — `Cargo.toml` 변경 없이 새 파일만으로 등록된다) + `starfall_persistence::history::replay_and_compare`(`history.rs`, 로직 대부분은 여기).

```text
cd server && cargo run -p starfall-persistence --bin history-replay -- --world <world-id> --read-only
```

### 5.1 설계

- `replay_and_compare(pool, world_id, rule) -> ReplayReport` — 그 월드의 `domain_events` 를 워터마크까지 `(tick, sequence)` 순으로 전부 읽어 **빈** `HistoryCore::new(rule)`(재구축이 아니다 — `load()` 의 `rebuild()` 와 다른 경로를 타야 "재구축과 재생이 같은 결과를 낸다"는 것 자체를 증명할 수 있다)에 처음부터 먹인다.
- 나온 기록마다 저장된 `historical_events`·`evidence` 행을 **id 로 직접** 찾아 내용 전체(`recorded_at` 제외)를 비교한다. 반대 방향(저장은 있는데 재생에서 안 나옴)도 잡는다 — "조용히 덜 발견하는" 회귀까지 잡으려면 필요하다.
- `ComparableEvent`/`fetch_comparable`(쓰기 경로의 충돌 비교, §2.2)와 로직을 공유한다 — 행→비교값 변환(`row_to_comparable`)을 뽑아 쓰기 전용 경로(`fetch_comparable`, 행이 반드시 있다는 전제)와 재생 전용 경로(`fetch_comparable_optional`, 없을 수 있다)로 나눴다.
- **아무 것도 쓰지 않는다** — `replay_and_compare`에는 `INSERT`/`UPDATE`가 없다. 도구는 CLAUDE.md 금지 사항(주 DB 역사 테이블을 비우지 않는다)을 지킨다.
- CLI 껍질(`bin/history-replay.rs`)은 얇다: `--world`·`--read-only`(필수 플래그, 쓰기 모드가 없어도 이름을 잘못 쓰는 사고를 막는다) 파싱, `DATABASE_URL`로 풀 생성, 규칙 파일 로드(`STARFALL_DATA_DIR` → `data` → `../data` — `bins/game-server/src/data.rs`의 해석 규칙과 같지만 그 모듈은 다른 크레이트(bin 전용)라 재사용이 안 돼 최소한만 옮겨 적었다), 결과 출력 + 종료 코드(`PASS`=0, `FAIL`=1, 불일치 목록을 stderr에 줄마다).
- ⊘(계약): "재생 대상 0건" 배제 — `domain_events_replayed`·`records_replayed` 를 항상 stdout에 찍고, 0건이면 stderr에 경고를 낸다.

### 5.2 Cargo 의존 변경

`crates/persistence/Cargo.toml`의 **주 `[dependencies]`**에 `tokio` `rt-multi-thread` feature를 추가했다(`[dev-dependencies]`에는 이미 있었지만 `src/bin/`은 주 의존성만 본다 — 바이너리가 자기 tokio 런타임을 만들어야 해서 필요했다). 트리에 새 크레이트가 늘지는 않는다(`dev-dependencies`와 합집합). 이 파일은 team-lead 지시로 명시적으로 배정된 새 파일(`history 소유`)이라 직접 고쳤다 — server에게 통보는 아래 보고에 포함.

### 5.2b `detector_rule` 원복 — architect 판정 (2026-09-30, §5.2a 뒤집음)

**§5.2a 의 수정은 되돌렸다.** team-lead 가 "쓰기 경로 의미를 바꾼 변경이니 architect 판정이 필요하다"고 지적해 요청했고, architect 판정: **`rule_version`("mineral-discovery@1", 버전 포함)을 계속 쓴다.** 정본은 **ADR-0014 §4-2**(0003 은 증거 DB 에 이미 적용돼 동결이라 설계 주석 자체를 못 고친다 — ADR 에 "정본은 이 절"이라고 명시).

이유(architect 실측): 증거 DB 에 이미 `"mineral-discovery@1"` 행이 **51개(32 월드)** 있고 `rule_id`(버전 없이) 값 행은 **0개**였다. 지금 바꾸면 (1) 같은 열에 두 철자가 영구 공존(옛 행은 원칙 5 로 못 고침) (2) 옛 월드에서 커서를 되감아 재배달하면 PK `(source_event_id, detector_rule)` 이 옛 값과 안 겹쳐 **근거 행이 두 벌** 생긴다 — 멱등 둘째 겹이 조용히 무너진다. "`@2` 가 같은 근거를 다시 쓰는가"라는 §5.2a 의 원래 우려는 `dedupe_key` `UNIQUE`(멱등 셋째 겹)가 이미 막으므로 `rule_id` 로 바꿀 실익이 없었다.

원복: `detector_rule_for()`가 다시 `rule_version.as_str().to_string()`(변환 없이 그대로)을 돌려준다. SC-51 단언도 `"mineral-discovery@1"`로 되돌렸다. **되돌리기 전 실측 확인**: `SELECT detector_rule, count(*) FROM historical_event_sources GROUP BY detector_rule` → `mineral-discovery@1|51` **한 줄뿐**(§5.2a 가 살아 있던 동안 실제 서버·봇 실행으로 새 값이 쓰인 적은 없었다 — `history-replay` 자체는 쓰기가 없고, 그 사이 쓰기를 하는 실서버/봇 실행이 없었기 때문). 원복 뒤 실제 월드로 재확인: `재생한 domain_events = 636, 재생한 역사 기록 = 4` / **PASS**(§5.2a 실측 당시의 FAIL 이 사라졌다 — 원복이 맞다).

`cargo test -p starfall-persistence --test history_runner` 15건 전부 초록, `cargo fmt --all --check`·`cargo clippy --workspace --all-targets -- -D warnings` 재확인 통과.

**쓰기 경로 증거(architect 요청, 2026-09-30)** — `01a0ef96…` 재생은 **읽기 경로**(옛 값과 일치)만 보여준다. **새로 쓰는 값이 `rule_version`인지**는 `history_first_discovery_rows`(§2.3, SC-51)가 **새 테스트 DB**에서 실제 러너로 발견을 만들고 `SELECT detector_rule FROM historical_event_sources`가 `"mineral-discovery@1"`인지 단언하는 것으로 확인된다(`history_runner.rs:304~311`). 그 실행의 RAN 줄:

```
$ STARFALL_DB_TESTS=required cargo test -p starfall-persistence --test history_runner history_first_discovery_rows -- --nocapture

running 1 test
STARFALL_DB_TEST RAN history_first_discovery_rows
test history_first_discovery_rows ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 14 filtered out; finished in 0.42s
```

### 5.2a `detector_rule` 결함 수정 + `historical_event_sources` 비교 추가 (팀장 지시, 2026-09-30 — **되돌려짐, §5.2b 참고**)

팀장이 계약 SC-59 방법·⊘ 칸을 다시 읽고 지적: 원본은 `historical_events`·`evidence` 만 비교했는데, 계약은 `historical_event_sources`(행·결정적 id·rule_version)도 이름으로 지명했다. 추가하다가 실제 결함을 하나 발견했다 — `process_row` 가 `historical_event_sources.detector_rule` 에 `rule_version`(`"mineral-discovery@1"`, 버전 포함)을 그대로 썼는데, migrations/0003 의 설계 주석은 "detector_rule = rule_id(버전 없이) — 규칙이 @2 로 올라도 같은 근거를 새 근거로 다시 안 쓰기 위해서"라고 적어 뒀다. 코드가 그 의도를 안 지키고 있었다.

수정: `detector_rule_for(&RuleVersion) -> String`(`RuleVersion` 패턴 `{rule-id}@{n}` 에서 `@` 앞을 뺀다) 공용 함수를 만들어 쓰기 경로(`process_row`)와 재생 비교(`replay_and_compare`) 양쪽에 쓴다. SC-51 테스트(`history_first_discovery_rows`)에 `detector_rule = "mineral-discovery"` 단언을 추가했다.

**실측(로컬 개발 DB, 실제 월드)**: 재생 도구를 다시 돌리니 이번엔 **FAIL** — 4건 전부 `historical_event_sources` 불일치(저장된 값이 옛 코드가 쓴 `"mineral-discovery@1"`, 재생은 고친 코드대로 `"mineral-discovery"`를 기대). 이것은 재생 도구의 버그가 아니라 **진짜 발견**이다: 그 월드는 고친 코드 이전에 만들어진 행이고, 역사 표는 append-only 라 그 행을 고칠 수 없다(원칙 5 그대로 — 정정은 새 레코드로). 이 월드는 이미 광물 4종을 다 발견해서 앞으로 새 발견도 안 생긴다. 새 월드부터는 고친 코드로 쓰이므로 문제없다. **이 FAIL 을 그대로 로그에 남긴다** — 재생 도구가 "그냥 항상 PASS" 가 아니라는 증거이자, 옛 데이터의 실제 상태를 정직하게 보고한다.

### 5.3 검증

- **실서버 데이터로 수동 실측(최초 버전 — historical_events·evidence 만 비교)**: 로컬 개발 DB(`starfall`, `domain_events` 7987행)의 실제 월드 하나(`01a0ef96-c9ad-7c5e-8e55-1b2afe94f0a9`, 봇/실서버가 만든 진짜 역사)에 대해 실행 — `재생한 domain_events = 636, 재생한 역사 기록 = 4` / `PASS`. **읽기만 했다** — 이 DB에 아무 것도 안 썼다(`replay_and_compare`에 쓰기 코드가 없다는 사실 자체가 구조적 보장이다).
- **같은 월드, `historical_event_sources` 비교 추가 + `detector_rule` 결함 수정 후 재실측**: 이번엔 **FAIL**(4건, §5.2a) — 옛 코드가 쓴 `detector_rule` 값과 고친 코드의 기대값이 다르다. 이 월드는 이미 다 발견돼 새로 검증할 방법이 없어(append-only) 그대로 로그에 남긴다 — 고친 코드가 실제로 다른 결과를 낸다는 증거이자, 재생 도구가 항상 PASS 만 내는 죽은 검사기가 아니라는 증거이기도 하다.
- **DB 통합 테스트 3개**(`starfall-testdb`, `history_runner.rs`, 전부 고친 코드로 새로 만든 월드 — 깨끗한 PASS 를 이것으로 증명한다):
  - `history_replay_matches_stored_records` — 정상 경로(실제 러너, 고친 `detector_rule` 포함)로 발견 2건을 만든 뒤 재생 → `domain_events_replayed=2`, `records_replayed=2`, `is_match()==true`(`historical_event_sources` 비교까지 포함해서).
  - `history_replay_detects_content_mismatch`(음성 대조) — `seed_conflicting_row`(§2.3의 SC-105/111 헬퍼 재사용)로 저장된 행의 payload만 실제 채굴과 다르게 심어 놓고 재생 → `is_match()==false`, 불일치 목록에 그 `historical_event_id`가 있음을 확인.
  - `history_first_discovery_rows`(SC-51, §2.3) — `detector_rule = "mineral-discovery"`(버전 없이) 단언 추가.
- `cargo test -p starfall-persistence -p starfall-history` — persistence 3+5+14+1(restart_economic_state, server 소유 파일)+history 16 전부 초록. `cargo fmt --all --check`·`cargo clippy -p starfall-persistence --bins -- -D warnings`·`cargo check --workspace --all-targets` 전부 통과.

### 5.4 마무리 세 가지 (팀장 지시, 2026-09-30)

1. **`cargo clippy --workspace --all-targets -- -D warnings`(CI 그대로) 재확인** — `--bins`만으로는 부족하다는 지적. 실행 결과 워크스페이스 전체(contracts·sim·history·persistence·gateway·game-server·testdb) 깨끗하게 통과. `cargo test --workspace --no-fail-fast`(STARFALL_DB_TESTS=required)도 전부 0 failed(history_runner.rs 15건 포함) 재확인했다.
2. **`--read-only`를 DB 세션 수준에서 구조적으로 강제** — `starfall_persistence::history::read_only_pool(database_url)`(신규, `history.rs`)이 `PgConnectOptions`에 `default_transaction_read_only = on`을 세션 시작 파라미터로 건 풀을 만든다. `bin/history-replay.rs`가 기존 `starfall_persistence::pool()` 대신 이 함수를 쓴다. "소스에 쓰기 코드가 없다"에 더해 **DB 자체가 쓰기 문장을 거부**한다(SQLSTATE `25006`). 새 DB 테스트 `history_replay_pool_rejects_writes` — 이 풀로 SELECT는 성공, UPDATE는 `25006`으로 거부되고 실제로 행이 안 바뀌었음을 일반 풀로 재확인한다.
3. **server-db 통보** — `crates/persistence/Cargo.toml`(소유: server-db) 변경(§5.2의 `rt-multi-thread`)을 알렸다. 되돌릴 필요 없음을 팀장이 이미 확인.

`cargo test -p starfall-persistence` — history_runner.rs **15건**(§5.3의 3개 + 이번 1개) 전부 초록. qa에게 SC-59 재판정 요청했다.

## 5. 다음

- ~~server: main.rs 배선~~ — **완료**(server, §3 참고했던 문제는 해소됨. `cargo check --workspace --all-targets` 초록).
- ~~`record.rs` 채널 자리표시자 교체~~ — S5(server)가 게이트웨이 쪽 배선을 마치며 해소된 것으로 보인다(§3 작성 시점 메모, 재확인 필요하면 `record.rs` 를 다시 본다).
- SC-47·54·62·111(분리) — 완료(§2.3).
- ~~SC-59~~ — **완료**(§4, R1-H).
- SC-58(오라클 교차, qa) 는 여전히 qa 몫.
- 제약 census **50**(§2.1) — architect 가 ADR-0013 §5a-1 을 50/104(+트리거 4)로 갱신 완료, qa 도 실측 PASS 확인함.


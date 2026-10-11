# 03 — History 구현 (H1, historyimpl3)

## 1. 작업 범위와 결론

**결론: 코드 변경 없음.** `MINERAL_SOLD`(p1-03 Level 0 도메인 이벤트)가 생겨도
`mineral-discovery@1` 의 결정성·재생은 깨지지 않는다 — `server/crates/persistence/src/history.rs`
의 `decode_row`(L965)가 `event_type != MINERAL_MINED` 인 행은 payload 를 전혀 역직렬화
하지 않고 좌표(`event_id`·`world_id`·`tick`·`sequence`)만 읽어 `DomainEvent::unjudged`
로 돌려준다. 이것이 H-30~H-33 네 항목의 실제 동작이었고, **DB 통합 테스트로 처음
증명**한 것이 이 태스크의 전부다(team-lead 지시, `_workspace/p1-03-trading/02h_history_review.md`
§2.3·§4).

- 변경 파일: `server/crates/persistence/tests/history_runner.rs` (추가만, 기존 테스트
  무변경)
- 변경 없음(확인함): `server/crates/persistence/src/history.rs`,
  `server/crates/history/**`, `data/history/rules/mineral-discovery.json`,
  역사 마이그레이션

## 2. 이벤트 생성 방법

`MINERAL_SOLD` 행은 이 파일의 기존 관례(`insert_mineral_mined` — 직접 SQL)를 그대로
따라 **손으로 삽입**했다(`insert_mineral_sold`, `history_runner.rs`). `Simulation::step()`
을 거치는 실제 커밋 경로(계약 모양·CAS 정확성)는 이미 `crates/persistence/tests/trading_sale.rs`
(serverdb3, S4)가 재고 있고, 이 파일의 관심사는 그 아래 층 — "이미 커밋된 행이 있을
때 역사 러너가 무엇을 하는가" — 뿐이다(파일 머리말과 동일한 분리 원칙). `decode_row`
가 `MINERAL_MINED` 가 아닌 타입에 대해 payload 를 읽지 않으므로(위 §1), payload
내용이 판정 로직에 영향을 주지 않는다 — 다만 모양은
`contracts/fixtures/MINERAL_SOLD/first-sale-sells-all.json` 을 그대로 흉내 냈다.

## 3. 판정 규칙 표 — 변경 없음

| 규칙 | rule_version | 대상 타입 | 이번 슬라이스 영향 |
|---|---|---|---|
| mineral-discovery | `mineral-discovery@1` | `MINERAL_MINED` | 없음 — `MINERAL_SOLD` 는 Level 0, 판정 대상이 아니다(새 Historical Event 0종) |

## 4. 프로젝션 — 변경 없음

Chronicle·Biography 프로젝션 대상 타입 불변(`MINERAL_DISCOVERED`만). `MINERAL_SOLD` 는
이 슬라이스에서 역사 → 플레이 연결이 없다(UI-13 은 client4 가 기존 `MINERAL_DISCOVERED`
NOTICE 를 그대로 쓴다. 디자인 §5·§9.3).

## 5. 테스트 결과 — H-30~H-33 (SC-52~55)

| H- | SC | 계약 이름 | 결과 | 비고 |
|---|---|---|---|---|
| H-30 | SC-52 | `h30_runner_passes_sales_and_judges_after` | PASS | 채굴A(t10)→판매3(t11-13)→채굴B(t14). 커서(14,0) ≥ 마지막 판매 좌표(13,0). `MINERAL_DISCOVERED` 2행. `historical_event_sources` 중 판매 event_id 0/3 |
| H-31 | SC-53 | `h31_runner_advances_over_sales_only` | PASS | 판매만 2행. 커서가 (21,0) 이상으로 전진(역사 0행이어도) |
| H-32 | SC-54 | `h32_sold_v2_does_not_halt_runner` + 짝 `history_fail_stop`(기존) | PASS | `MINERAL_SOLD` schema_version=2 는 안 멈춤(1건 발견 판정) / 짝 테스트(기존, 미변경)는 `MINERAL_MINED` 쪽 fail-stop 을 같은 파일에서 계속 확인 |
| H-33 | SC-55 | `h33_replay_matches_with_sales` | PASS | `replay_and_compare` 일치. `domain_events_replayed`=2(채굴1+판매1), 판매 행 수 ⊘ 선단언(≥1) |

실행:
```
cd server
export DATABASE_URL=postgres://starfall:starfall_dev_only@localhost:15432/starfall
export STARFALL_DB_TESTS=required
cargo test -p starfall-persistence --test history_runner h3 -- --test-threads=4
```
```
STARFALL_DB_TEST RAN h31_runner_advances_over_sales_only
STARFALL_DB_TEST RAN h32_sold_v2_does_not_halt_runner
STARFALL_DB_TEST RAN h33_replay_matches_with_sales
STARFALL_DB_TEST RAN h30_runner_passes_sales_and_judges_after
test h31_runner_advances_over_sales_only ... ok
test h33_replay_matches_with_sales ... ok
test h30_runner_passes_sales_and_judges_after ... ok
test h32_sold_v2_does_not_halt_runner ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 15 filtered out; finished in 11.86s
```

회귀 전체(같은 파일, `history_runner.rs` 19 개 전부 — 기존 15 + 신규 4): **19 passed**
(295.82s, 영향 없음 확인).

## 6. R3 자기 점검표 (qa-throughput)

| 항목 | 증거 |
|---|---|
| 1. 계약 방법 이름 그대로 실행, 1건 이상 | 위 §5 실행 로그 — `STARFALL_DB_TEST RAN h30_…`~`h33_…` 각 1회, 4 passed, 다른 테스트 안 섞임(`-- h3` 필터, 15 filtered out) |
| 2. 구현 변이 | `server/crates/persistence/src/history.rs` L965 `decode_row` 의 `if event_type != MINERAL_MINED { Ok(unjudged) }` 분기를 `Err("MUTATION: …")` 로 바꿔 fail-stop 하게 함 → 재실행 결과: h30·h31·h32·h33 **4건 전부 FAILED**(아래 발췌) |
| 3. 원복 증명 | 변이 전 sha256 = 변이 적용 전 측정값, 원복 후 재측정값이 **동일**: `45cc002a90358cad77f1b8585883aeb79bf033f0459557d0360b45d358c5593a *crates/persistence/src/history.rs`(변이 전/원복 후 동일값, 커밋 전 작업 트리에서 직접 비교). `history_runner.rs` 는 이번 작업에서 추가만 했고 변이 대상이 아니다 |
| 4. 바이너리·스크립트·카운터 실재 | DB 테스트이므로 `TestDb::create("h30_runner_passes_sales_and_judges_after")` 등 계약 이름 그대로 호출(파일 내 `let name = "h30_…"; starfall_testdb::TestDb::create(name)`) |
| 5. 오래 걸리는 실행 대기 | 전체 19개 스위트(295.82s)를 백그라운드로 돌리고 같은 턴 안에서 폴링 루프로 완료를 확인함(task bietn856q → `test result: ok. 19 passed`) — "알림 오면 이어서"로 턴을 끝내지 않음 |

변이 적용 시 실패 로그(발췌):
```
test h32_sold_v2_does_not_halt_runner ... FAILED
test h33_replay_matches_with_sales ... FAILED
test h31_runner_advances_over_sales_only ... FAILED
test h30_runner_passes_sales_and_judges_after ... FAILED

---- h30_runner_passes_sales_and_judges_after ----
assertion `left == right` failed: 발견 2 가 아니라 1 이면 B 가 판정되지 않은 것(커서 정지)이다
  left: 1
 right: 2
---- h31_runner_advances_over_sales_only ----
assertion failed: !handles.halted.load(Ordering::Relaxed)
---- h32_sold_v2_does_not_halt_runner ----
assertion `left == right` failed: 뒤의 채굴이 발견으로 판정돼야 한다 — 러너가 멈추지 않았다
  left: 0
 right: 1
---- h33_replay_matches_with_sales ----
재생 실패: Corrupt("MUTATION: 판정 대상 아닌 타입 MINERAL_SOLD 에서 fail-stop")
```
원복 후 재실행: 4 passed (위 §5 로그 그대로 재확인).

`cargo fmt --all -- --check`: 변경 없음(diff 0). `cargo clippy -p starfall-persistence --all-targets -- -D warnings`: 경고 0.

## 7. 관찰(수정하지 않음, 참고용)

`HistoryHandles::detector_halted_total` 카운터는 현재 코드 전체에서 `new()` 초기화 뒤
한 번도 증가하지 않는다(`fetch_add` 호출 없음, `history.rs` 전수 grep 로 확인). H-30·
H-32 의 `assert_eq!(handles.detector_halted_total.load(Ordering::Relaxed), 0)` 는 이
사실 때문에 항상 참이다 — 계약 텍스트(§2.3 "history_detector_halted … 0")를 그대로
단언했을 뿐, 이 카운터 자체의 증가 로직을 재는 테스트는 아니다. 이번 슬라이스 범위
밖(역사 코드 변경 없음 전제)이라 고치지 않았다 — fail-stop 관측은 `handles.halted`
(bool)가 실제로 잡고 있다(H-32·`history_fail_stop` 둘 다 이것으로 확인).

## 8. 다음 단계

- qa3 에게 모듈 완료 통보(§5·§6 참조).
- architect3·server3 통보 불필요 — 코드 변경 없음, 테스트 전부 PASS(예상대로 "코드
  변경 없음"이 확인됐다. 발견이 있었다면 알렸을 것이다).

# p1-02-mining — 마감 요약 (Phase 6)

- 작성: 리더, 2026-10-05
- PR: #5 (`p1-02-mining` → `main`)
- 상태: **done** — SC-68 은 사용자가 사람 진술로 수용(2026-10-05, 스크린샷 없음)

## 1. 구현된 것

| 영역 | 내용 |
|---|---|
| 서버 판정 채굴 | `MINE_RESOURCE` → 거부 사유 2~7(`TARGET_UNKNOWN`·`COOLDOWN_ACTIVE`·`TARGET_OUT_OF_RANGE`·`SHIP_TOO_FAST`·`RESOURCE_DEPLETED`·`CAPACITY_EXCEEDED`) 판정 순서, `MINERAL_MINED`, 처리 장부 멱등 |
| 상태 메시지 | `INVENTORY_STATE`·`DEPOSIT_FIELD_STATE` — 별도 message-id 스트림(ADR-0006 §4a) |
| 영속화 | 마이그레이션 0002(경제)·0003(역사) — 제약 104 + 트리거 4(ADR-0013 §5a). `recording_lag` > 20 이면 `RECORDING_BACKLOG`, 이동 명령은 계속 수락(ADR-0013 §6 K2b) |
| 역사 | `mineral-discovery@1` → `MINERAL_DISCOVERED`, `detector_rule` = `rule_version`(ADR-0014 §4-2). 읽기 전용 재생 도구 `history-replay` |
| 테스트 기반 | `starfall-testdb`(테스트마다 `starfall_test_<uuidv7>` DB, 증거 DB 불변), 채굴 golden `sim/tests/data/replay_mining/` |
| 클라이언트 | 계약 DTO, data 사본·로더, 그레이박스 채굴 UI. **C4(사람 세션 1차 FAIL 뒤)**: 3D 광맥 표식·거리 라벨, 채굴 가능 3줄, 쿨다운, 산출 알림/내 발견(금색)/성계 발견(하늘색) 분리, 채굴 키 E→G |
| QA 도구 | 봇 채굴 시나리오, e2e 판정 도구, CI 출처 게이트 `--slice` |

## 2. 증거

| 항목 | 결과 | 경로 |
|---|---|---|
| 스프린트 계약 | 114 항목 | `02_sprint_contract.md` |
| r1 | PASS 102 / FAIL 7 / 미검증 5 | `04_qa_report_r1.md` |
| r2 | PASS 106 / FAIL 6 / 미검증 2 | `04_qa_report_r2.md` |
| r3 (마지막) | **PASS 112 / FAIL 0 / 미검증 2** | `04_qa_report_r3.md`, `evidence/r3_20261004/` |
| SC-98 CI 첫 실행 | **PASS** — 출처 게이트 분모 114, 이 슬라이스 라벨 17, 다른 슬라이스 제외 py 26 + rs 6 찍힘. server job 에서 `STARFALL_DB_TEST RAN` 60줄(Postgres 서비스 위) | PR #5 run 37169479580 |
| SC-68 사람 세션 | 1차 **FAIL**(광맥 3D 표식·채굴 가능·쿨다운 표시 없음, E 키 충돌) → C4 보강 → 2차 **사람 진술로 확인, 스크린샷 없음** | `evidence/sc68/notes.md`, `evidence/c4/` |
| 부하 | 31 연결 10분, 채굴 수락 5,999, tick 지연 1/12,010, 기록 지연 거부 0 | `evidence/r2_20261003/load/` |
| 증거 DB | 4805 events, 지문 불변(r3 종료 시) | `evidence/r3_20261004/fp_after2.txt` |

### SC-68 — 사용자 수용(2026-10-05: "진술로 수용하고 병합해줘")
절차서의 판정 입력은 스크린샷 6장 + 소감 두 문항이다. 2차 세션에서 사람은 "잘 되는데?", "하늘색 배너도 떴어"라고 진술했고, DB 에 같은 월드의 `MINERAL_DISCOVERED` 2건(사람 glacine, 봇 starfall-glass)이 있다. 스크린샷과 소감 두 문항의 명시 답은 없다. 그래서 PASS 로 적지 않고 **"사람 확인(진술)"** 으로 남긴다.

## 3. 기술 부채

1. **판정 스크립트 결함** — `tests/e2e/cargo_sc_map.py` 가 백틱 안 맨 테스트 이름과 `--test X` 뒤 필터를 줍지 않는다. 이 때문에 SC-28 의 존재하지 않는 테스트 이름이 r1·r2 에서 안 보였다. `run_filters.sh` 는 CRLF 로 열이 밀려 0건이 된다(r2·r3 두 번). 참고 구현: `evidence/r3_20261004/scripts/`. **다음 슬라이스 Phase 3 전에 고친다.**
2. **`run_round.py` 미작성** — qa-throughput R5 의 라운드 자동 실행기를 이번 슬라이스는 만들지 않았다(규칙이 r2 중간에 생겼다). 다음 슬라이스 Phase 4 산출물.
3. **SC-09 gateway 카운터 테스트의 시간 여유** — `mine_resource_capacity_exceeded_is_counted_in_commands_rejected_total`. 로컬 11/11, CI 1/1 통과. 느린 러너에서 거짓 실패 가능성을 지켜본다.
4. **화면 요소는 테스트 밖이었다** — 디자인 §8 의 2·3번 요소가 r1~r3 동안 구현되지 않았는데 계약 114 항목 중 이를 잡는 자동 항목이 없었다(SC-68 하나뿐). 구현자의 "사람이 확인할 항목"이 C3 를 사람 세션에 넘기지 않았다. → 다음 슬라이스: 디자인의 화면 요소 목록을 계약에서 **요소마다 순수 로직 EditMode 테스트 + 사람 세션 체크리스트**로 쪼갠다. 사람 세션을 r1 전에 한 번 돌린다.
5. **재조정 오차 로그** — 정지 상태에서 `reconcile_error_threshold_event`(방향 오차 1~2°, 기준 1°)가 계속 찍힌다. 동작 영향은 관찰되지 않았다.
6. **HUD 겹침** — 작은 창에서 두 HUD 박스가 겹칠 수 있다(그레이박스 한계).
7. **Editor 기동 절차** — 그레이박스는 `STARFALL_GREYBOX_AUTOBUILD`·`STARFALL_NET_AUTOCONNECT`·`STARFALL_DEV_AUTH_SECRET` 을 Editor 프로세스 환경에 넣어야 한다. 넣지 않으면 빈 씬이다(p1-01 R21 과 같은 실수 반복). 기동 스크립트를 만든다.
8. 이월(p1-01): 이슈 #3, GreyboxSession 시계 주입, world_full 예약 누수, Unity EditMode 가 CI 밖.

## 4. 진행 시간 (qa-throughput 목표 대비)

| 구간 | 목표 | 실측 |
|---|---|---|
| r1 판정 | 3시간 이하 | 약 13시간 (규칙 도입 전) |
| FAIL 수정 → r2 시작 | 같은 날 | 약 2.9일 (규칙 도입 전) |
| r2 FAIL 수정 → r3 판정 끝 | 같은 날 | 같은 날 — #36 수정 → r3 끝 약 2시간 (10-03 23:5x → 10-04 01:5x) |
| 멈춤 | 0회 | r2 에서 2회(규칙 도입 전), r3 0회 |
| 사람 세션 | — | 1차: PC 절전으로 서버가 실행 한도(벽시계 2시간)에 닿아 종료, 2차 시도에서 기능 부재 발견 |

r3 는 규칙(R1·R2·R6·R7)을 적용한 첫 라운드였고 같은 날 끝났다. 다만 r3 에서 결정 요청 메시지가 엇갈려(qa-r3 의 결정 요청과 리더의 결정이 교차) 한 번 더 왕복했다.

## 5. 다음 슬라이스 추천

1. 먼저 위 부채 1·2·7(판정 스크립트, `run_round.py`, Editor 기동 스크립트)을 작은 SOLO 작업으로 처리한다.
2. 로드맵 Phase 1 의 다음 기능(거래·시장 또는 Chronicle 열람)을 `starfall-spec` 범위 게이트로 정한다. 이번 슬라이스의 인벤토리·역사 기록이 그대로 입력이 된다.

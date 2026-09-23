# p1-01-ship-movement — QA 평가 리포트 라운드 4

- 작성: qa(라운드 3 을 끝낸 세션), 2026-09-22
- 계약: `02_sprint_contract.md` **7차 개정**
- 승인: `00_request.md` 사용자 결정 **5**(동시 접속 = **(B) 나중 접속이 이어받는다**) · **6**(라운드 4, 블록 6~9 전부)
- 분담: **이 세션 = 0·1·2단계**(계약 개정 / I-29 재판정 + SC-11 / 블록 0 회귀). 블록 6~9(3~7단계)는 **새 qa 세션** — 끝에 인계 절을 둔다
- 라운드 3 결과: `04_qa_report_r3.md` §8 (PASS 12 / FAIL 1 기록 후 닫음 / 보류 1 / 계약 외 결함 1)

---

## 0. 0단계 — 계약 7차 개정 (빌드·측정 없음)

정본: `01_architect_decisions.md` `## R3 추가 판정`(550행~) 사안 1~4 + `### 보충`. 변경은 `02_sprint_contract.md` §9 **7차** 행에 모았다.

| # | 변경 | 자리 |
|---|---|---|
| 1 | **SC-11 (3)** 을 architect 대체 문구 그대로 교체 — 와이어 층 = **양자화 봉투(≥ 64 시작점) + 음성 대조 필수**, 봉투 밖 1 양자 이내 FAIL 은 표본을 늘려 재판정, **(3′) 비트 일치는 `f64` 층**(server AC-3(d1) in-process 차분 테스트, 비교 tick 수 포함). 방법 칸 "결정적이므로 허용 오차 없음" → "허용 오차를 고르지 않는다 — 봉투는 양자화에서 유도한다" | 계약 §1 C절 SC-11 |
| 2 | **SC-88 신설**(스펙 AC-3(h), I-29) — (B) 단언 (a) 함선 1척·겹침 0·S2 가 스폰 없이 이어받음 (b) 같은 tick `SESSION_OPENED(S2)` k → `SESSION_CLOSED(S1, SUPERSEDED)` k+1, 원인 = S2 열기 (c) close **4001** (d) **밀려난 클라이언트 재접속 없음** (e) 자기 참조 0·유령 `ACTIVE` 0·종료 디스폰 원인 실재. 판정 도구 `overlap` 의 **양방향 자기 검증**을 문구에 넣었다. **(d) 는 S1 이 실제 클라이언트(Unity)여야 한다** — 봇은 원래 재접속하지 않아 봇으로 재면 항진명제다(§7a) → 블록 6 몫 | 계약 §1 C절, SC-14 뒤 |
| 3 | **SC-81** — "tick 구간 한정" → **동결된 결함 장부 등식**. 검사는 **타입 + ≠ self + 순서**(존재만 보는 조인은 자기 참조를 통과시킨다 — 실측). (b) 표 전체 결함 집합 == 장부 7건 (c) 인스턴스 구간 결함 0 (d) 도구 양방향 자기 검증 | 계약 §1 L절 SC-81 |
| 4 | **AC-2(h-log) → AC-2(i)** — 계약 §7a·§9 6차 행에 대응 표기, 도구 3곳 본문 교체(`log_pipe_backpressure.py` 머리·조건 ③ 사유·`--help`, `make_red_logsink_binary.py` 4곳, `tests/e2e/README.md` 2곳). `04_qa_report_r3.md` 는 과거 리포트라 **머리에 대응 한 줄만** 두었다. 증거 경로 `evidence/R3-B/AC2hlog/` 는 그대로 | 사안 4 표 |
| 5 | **SV-1** — §7a 표의 해당 행에 "폴 횟수 = **`ws_connections` 게이지가 아직 0 이 아니었던 횟수**" 정정. 판정 기록은 아래 §0.2 | 사안 3 |

### 0.1 SC-11 — **보류 → PASS** (architect R3 추가 판정 "이 판정들이 R3 판정을 바꾸는가" 표)

새 문구의 (3) 은 라운드 3 에서 **이미 잰 것**이다(`evidence/R3-B/B5/resume-check/resume_check.json`):
T1 14필드 **전부 봉투 안**(정수 일치 10/14, 편차 최대 위치 2 mm < 봉투 폭 8 mm), 잔류 중 관측자 **61/61 봉투 안**, 봉투 시작점 **64개**,
**음성 대조**(휴면 `flight_assist=false`) T1 **11필드 봉투 밖** + 관측 **61/61 밖** — 대조가 틀린 모델을 가른다. (1)(2)(e2) 는 라운드 3 PASS 그대로.
→ **SC-11 PASS.** (3′)(`f64` 차분, server S-4)은 **라운드 4 신규 확인 항목**으로 1단계에서 server 출력을 받아 본다 — SC-11 PASS 를 막지 않는다(architect).

### 0.2 SV-1 판정 기록 갱신 (판정은 그대로: FAIL(관측 설계) → 수정 확인으로 닫음)

- **전제 교체**: 폴 횟수는 "서버가 열려 있다고 답한 횟수"가 아니라 **"`ws_connections` 게이지가 아직 0 이 아니었던 횟수"** — 게이지를 내리는 연결 태스크가 굶으면 소켓이 닫혔어도 오른다. "실제로 늦게 닫힘"과 "게이지만 늦음"은 폴로 구분되지 않는다.
- **server 의 "참 양성" 라벨 철회(미증명).** ×80 의 FAIL 3회는 서버 FAIL 도 K 조정 근거도 아니고 **판정 불능(작동 범위 밖)**.
- **원래의 flaky 조건은 아무도 재현하지 못했다** — 수정의 근거는 측정 설계를 바로잡은 것과 ×80 에서 보인 대조(옛 7/8 FAIL, 새 형태 폴 1~2 PASS 4회)다.
- K = 10 유지(현실 구간 최대 3 에 3.3배, 회귀 ≈ 300 에 30배). **다시 여는 조건**: 현실 조건에서 새 형태가 한 번이라도 실패 — 그때는 원인을 붙이기 전에 서버 쪽 독립 시각(close 송신 tick 대비 `SESSION_CLOSED` tick)을 먼저 잰다.

### 0.3 도구 준비 (빌드 없음 — Python 만)

| 도구 | 항목 | 자체 검증 |
|---|---|---|
| **`ship_events.py ledger`** (신규) | SC-81 | 결함 = null·**self**·dangling·타입·순서. **표 전체 → 162행 중 결함 정확히 7건(전부 self) = 장부, PASS** / **블록 5 구간(403435~427104) → 26행 결함 0, PASS** / **둘째 인스턴스(427105~430030) → 결함 1(self), FAIL**. 고장 주입: 장부에서 1건 빼면 "장부에 없는 새 결함 1" 로 FAIL, 가짜 1건 더하면 "장부 행 사라짐" 으로 FAIL — **등식이 양쪽으로 깨진다**. 증거 `evidence/R4-0/SC81-ledger-*.json` |
| **`concurrent_session.py run/check`** (신규) | SC-88 | 라운드 A(잔류 창 뒤 유령 0 확인) + 라운드 B(**S2 를 살려 둔 채 종료** — 종료 디스폰 원인 판정이 0건 위에서 "전부"가 되지 않게). (d) 는 판정하지 않고 Unity 몫으로 남긴다 |
| `ship_events.py overlap` | SC-88 (a) | 라운드 3 에서 양방향 확인(actor 044 구간 FAIL, 블록 5 PASS) — 계약 문구가 이 증거를 요구한다 |

**계약 변경 신호 뒤 할 일(대기 중)**: architect 가 `contracts/` 에 `SUPERSEDED` 와 유효 fixture `SESSION_CLOSED/superseded.json` 을 넣으면 —
유효 fixture **26 → 27** 반영(계약 머리 "계약 데이터" 줄, §0.9 기준선은 과거 기록이라 그대로), `check_sessions.py` 는 **종료 사유를 하드코딩하지 않아 변경 불필요**(DB 에서 분포를 읽는다) — 주석에 `SUPERSEDED` 가 정상값임만 적는다,
봇은 종료 사유 문자열을 열거하지 않고 서버 Close code 를 그대로 싣는다(`SessionRecord.peer_close_code`) — **변경 불필요**, `tools/bots/tests/wire_fixtures.rs` 는 서버 메시지 타입만 읽어 도메인 이벤트 fixture 증가와 무관하다(1단계 전 재실행으로 확인).

### 0.4 architect 의 `contracts/` 변경 반영 (2026-09-22, 통지 수신 뒤)

architect 실측: 스키마 18 / **유효 27** 전부 통과 / 반례 34 전부 거부 / 레지스트리 13. 새 fixture 는 옛 스키마로 검증하면 거부된다(새 값을 실제로 검사한다).

| 반영 | 내용 | 확인 |
|---|---|---|
| 계약 | 머리에 7차 계약 데이터 줄(유효 **27**), SC-36·47·49 의 26 → 27(SC-47 의 C# 왕복 수 20 → 21 은 **client 실측으로 확정**), SC-81 에 **세션 간선**, SC-88 에 넘겨받기 단언 ①~⑤ 대응 + (f) "다른 사유의 원인 null", §9 7차 행에 (6) | — |
| `check_sessions.py` | 계약 enum(7값, `SUPERSEDED` 포함) 밖의 `close_reason` 을 **FAIL 로 드러낸다**(`close_reasons_unknown_to_contract`) | `py_compile` |
| 봇 | `ledger::close_code_meaning` — ADR-0005 표(1000/1001/1002/1011/**4001 = SUPERSEDED**), **표 밖 code 는 `UNKNOWN`**. probe 출력에 `meaning=` 과 **`controlled_ship_id=`**(SC-88 ③ 대조용) | 새 테스트 `close_codes_map_to_close_reasons_and_unknown_codes_surface`. `tools/bots` **70 passed / 0 failed**, fmt·clippy 0. `wire_fixtures` 는 유효 27 이 된 뒤에도 통과(서버 메시지 타입만 읽는다) |
| `ship_events.py ledger` | 세션 간선 결함 종류 9개(`opened_cause_not_null`·`closed_cause_not_null`·`superseded_cause_null`·`self`·`dangling`·`wrong_type`·`actor_mismatch`·`same_session`·`not_same_tick_earlier`) 추가 | **표 전체: 함선 162 + 세션 4352행, 결함 7(전부 장부의 self) — PASS** / 블록 5 구간 0 — PASS / 둘째 인스턴스 1 — FAIL. **실데이터에 `SUPERSEDED` 가 아직 0건**이라 세션 간선이 실데이터로는 아무것도 가르지 않았다 → 아래 합성 검증 |
| **`ship_events.py causation-selftest`**(신규) | **DB 에 쓰지 않고** 합성 17행을 CTE `domain_events` 로 덮어 **같은 SQL** 을 돌린다 | **PASS: 정상 7행(올바른 넘겨받기·일반 닫힘 포함) 결함 0, 틀린 모양 10행이 각각 기대한 결함 종류로 잡힘, 불일치 0.** 실행 뒤 DB 의 합성 id 행 **0건** 확인. `evidence/R4-0/SC81-causation-selftest.json` |
| `concurrent_session.py check` | ③ `controlled_ship_id` S1 = S2 = 유일한 스폰 함선 | `py_compile` |

**남은 것 — 측정 착수 신호 대기**: server(I-29 구조 수정 + (B) 넘겨받기, S-4 `f64` 차분)와 client(4001 처리 + C# 재생성)가 둘 다 끝나면 리더가 신호를 준다. 그 전에는 실서버를 띄우지 않는다.

---

## 1. 1단계 — I-29 재판정(SC-88) · SC-11 (3′) · SC-81 실데이터 · 릴리스 안전망

**빌드(측정 전에 전부, G-c)** — `evidence/R4-1/B0-builds.log`: GREEN `starfall-game-server.exe` sha256[:16] **`9c5ce49fc2b0f110`**, `cargo test --workspace --locked --no-run` 선빌드,
레포 `server/` 트리 해시 **`d2adedf8f2bc3760`**, 봇 최신. Unity 0, docker 7(starfall 2 + livingfeed 5). 리더 신호: 전원 유휴.

### 1.1 SC-11 (3′) — **PASS** (qa 가 직접 실행)

`cargo test -p starfall-sim --lib --locked -- ac3_d1 r4_s1 r4_s3 r4_s6 --nocapture` → **5 passed / 0 failed**(`evidence/R4-1/sim-r4-tests.log`).
출력 `[AC-3(d1)] 재접속 vs 대조, tick 60~139 (80 tick) 비트 동일 확인` — 재접속 실행(tick 2 닫기 → 잔류 → tick 59 재개)과 한 번도 닫지 않은 대조 실행의 함선 `f64` 14필드를 매 tick `to_bits()` 로 비교, **80 tick 전부 비트 동일**.
**판별력 확인(코드 읽기)**: 재개가 물리를 건드렸다면(속도 초기화·이월 입력 재적용·새 스폰) 위치 차이가 이후 모든 tick 에 남으므로, 대조 구간 후반에 함선이 멈춰도 검사가 자명해지지 않는다.
→ **SC-11 은 (1)(2)(3)(3′)(e2) 전부 PASS.**

### 1.2 SC-88 — **(a)(b)(c)(e)(f) PASS / (d) 블록 6(Unity S1)**

서버 기동 `start_tick = 430357`(드레인 경로), `tests/e2e/concurrent_session.py run` → 종료(stdin `shutdown`, **exit 0**) → `check`. 증거 `evidence/R4-1/SC88/`(`run.json`·`check.json`·봇 로그 4개).

| 관찰 | 라운드 A (bot-050) | 라운드 B (bot-051) |
|---|---|---|
| **(a)** 스폰 / 세션 열기 | **1 / 2** | **1 / 2** |
| **(a)③** S1·S2 `controlled_ship_id` | 같다(`…6c1c` = 유일한 스폰) | 같다(`…1316`) |
| 게이지 S2 접속 뒤 | `ws 1 / active 1`(S1 이 밀려남, 함선 1척) | `ws 1 / active 1` |
| **(b)①②** 같은 tick 순서·원인 | tick 430423: seq 0 `SESSION_OPENED(S2)` → seq 1 `SESSION_CLOSED(S1, SUPERSEDED)`, **원인 = 그 `SESSION_OPENED`** | tick 431394: 같은 모양 |
| **(c)④** S1 이 받은 close | **4001, initiator=server** | **4001, initiator=server** |
| **(f)⑤** 다른 사유의 원인 | `CLIENT_CLOSED` 원인 **null** | `SERVER_SHUTDOWN` 닫힘 원인 **null** |
| **(e)** 잔류 창 뒤 유령 | 시작 `active 0` → 모두 닫힘 → **35 s 뒤 `ws 0 / active 0 / lingering 0`**, `SHIP_DESPAWNED{LINGER_EXPIRED}` 원인 = S2 의 `CLIENT_CLOSED` | — |
| **(e)** 종료 디스폰 원인 | — | S2 를 살려 둔 채 종료(직전 `ws 1 / active 1`) → 같은 tick 431613: `SESSION_CLOSED(S2, SERVER_SHUTDOWN)` seq 0 → **`SHIP_DESPAWNED{SERVER_SHUTDOWN}` seq 1, 원인 = 그 닫힘** — **종료 디스폰 1건 위에서** 판정(0건 위의 "전부" 아님) |

같은 구간 도구: **`overlap` PASS**(함선 2, 겹침 0) · **`pairs` PASS**(2/2/2) · **`ledger --from-tick 430357` PASS**(함선 이벤트 4, 세션 8, **`SUPERSEDED` 2**, 결함 0).
**R3 §6.7 대비**: 같은 재현(같은 라벨 겹침 접속)이 R3 에서는 **함선 2척·유령 1·자기 참조 1** 이었고 이번에는 **1척·유령 0·자기 참조 0**.
**(d) "밀려난 클라이언트 재접속 없음" 은 판정하지 않았다** — 봇은 원래 재접속하지 않아 봇 S1 로는 항진명제다(계약 SC-88, 스펙 AC-3(h)). 봇 S1 이 4001 뒤 재접속하지 않은 것은 **증거가 아니다.** → 인계(§3).

### 1.3 SC-81 세션 간선 — **실데이터에서 처음 탔다**

- 인스턴스 구간: `SUPERSEDED` **2건**, 세션 간선 결함 0 (위).
- **표 전체**: 함선 이벤트 166 · 세션 이벤트 4360 · `SUPERSEDED` 2, 결함 **정확히 7 = 동결 장부**(전부 `self`), 새 결함 0, 사라진 장부 행 0 → **PASS**(`evidence/R4-1/SC81-ledger-whole.json`).
- 합성 검증(§0.4 `causation-selftest`)이 가른 10종 결함이 실데이터에서도 **0건**이다 — architect 요청("실데이터에서 결함 0 을 한 번") 충족.
- SC-81 자체의 판정(블록 9)은 후임 몫이다. 도구와 장부는 준비됐다.

### 1.4 ⚠ 릴리스 안전망 — "원인을 지어내지 않는다" — **디버그 방어선 PASS(단위 테스트) / 릴리스 무기록 팔은 정적 확인뿐 → 미검증(테스트 부재)**

**근거 종류를 나눠 적는다(§0.3: 정적 읽기만으로는 PASS 가 아니다).**

| 팔 | 근거 종류 | 내용 |
|---|---|---|
| 원인 없는 디스폰에서 **이벤트를 쓰지 않는다** | **정적 확인** | `server/crates/sim/src/simulation.rs:1093~1100` — `let Some(causation_id) = ship.linger_cause_event_id else { outcome.causeless_despawns.push(ship_id); debug_assert!(false, …); return; };` — **`ids.next_id()`(`:1102`)와 `outcome.events.push`(`:1104`) 이전에 반환**한다. 옛 `unwrap_or(event_id)` 는 사라졌다(`grep` 0) |
| **ERROR 로그** | **정적 확인** | `server/crates/gateway/src/runtime.rs:551~559` `log_causeless_despawns` 가 함선마다 `tracing::error!("… SHIP_DESPAWNED 를 쓰지 않았다")`, 호출은 매 step(`:453`)과 종료(`:497`) |
| **디버그 방어선** | **단위 테스트(실행)** | `r4_s3_despawn_ship_panics_in_debug_when_the_linger_cause_is_missing` **should_panic ok** — `despawn_ship` 을 직접 불러 원인 없는 `LINGERING` 을 만든다 |
| **릴리스 팔**(`debug_assert` 가 꺼졌을 때 이벤트 0 + 신호 1) | **테스트 없음** | 위 테스트는 디버그 패닉만 본다. 패닉 **뒤의 상태**(`outcome.events` 비었음, `causeless_despawns == [ship_id]`)를 단언하는 테스트가 없다 |

**판정**: 디버그 방어선은 **PASS**, 릴리스의 "이벤트를 쓰지 않는다"는 **코드상 맞지만 실행 증거가 없다 → 미검증(테스트 부재)**. 정상 경로로는 도달하지 않아 실서버로 태울 수 없다(S-2 가 막는다).
**server 요청(작다)**: 같은 테스트를 `std::panic::catch_unwind(AssertUnwindSafe(|| sim.despawn_ship(…)))` 로 감싸 패닉을 받은 뒤 **`outcome.events.is_empty()` 와 `outcome.causeless_despawns == [ship_id]`** 를 단언하거나, `#[cfg(not(debug_assertions))]` 변형을 두고 `cargo test --release -p starfall-sim` 로 한 번 보인다. 블록 9 진입을 막지는 않는다(자기 참조 결함은 SC-81 장부가 잡는다).

---

## 2. 2단계 — 블록 0 회귀 (계약 7차 + server·client R4 코드 뒤)

증거 `evidence/R4-2/`. 실서버는 1단계 뒤 정상 종료(exit 0)했고 측정과 겹치는 빌드는 없었다.

| 항목 | 명령 | 결과 | 판정 |
|---|---|---|---|
| **SC-01** | `cargo fmt --all --check` / `cargo clippy --workspace --all-targets -- -D warnings` / `cargo test --workspace --locked` **3회** | fmt exit 0 · clippy 경고 0 · **167 passed / 0 failed × 3**(간헐 실패 없음) | **PASS** |
| **SC-87** | `cargo test -p starfall-sim --test determinism --locked` → `git diff --exit-code -- server/crates/sim/tests/data/replay/` | 1 passed(bless 1 ignored) · **diff exit 0** · golden sha256[:16] `824a6489…`/`c924e59f…`/`3e00e527…` | **PASS** — S-2 의 sim 구조 변경이 결정적 재생 바이트를 바꾸지 않았다 |
| **SC-82** | `check_contract_coverage.py --strict` | **types 13 / errors 0 / warnings 0, exit 0** | **PASS** — *주의(architect): 이 스크립트는 열거 값을 보지 않는다. `SUPERSEDED` 의 Rust 쪽 일치는 SC-01 의 계약 테스트(유효 27 왕복), C# 쪽은 client 실측(27 발견 / 21 왕복)이 덮는다 — C# 수는 **qa 가 재현하지 않았다**(블록 6 세션에서 EditMode 를 돌릴 때 확인)* |
| **SC-83** | `interface_matrix.py` | 신규 7타입 대조 행 **201**, 불일치 **0**, exit 0 | **PASS** (라운드 1 과 같은 행 수 — 7타입의 필드는 바뀌지 않았다. `SESSION_CLOSED` 는 p0-02 타입이라 이 표 밖) |
| **SC-84** | `fixture_world_scan.py` | 스캔 **27**(라운드 1: 26 — `superseded.json` 추가), world_id 11, tick_hz 2, 쌍 2, exit 0 | **PASS** |
| **SC-86** | e2e 도구 자체 검사 11건 + 봇 | **11/11 기대대로**(`evidence/R4-2/SC-86-selftests.log`): `ship_events selftest`(correlation 키로 되돌리면 재개에서 깨짐) · `causation-selftest`(합성 17행) · **`overlap` 알려진 위반 FAIL / 블록 5 PASS** · **`ledger` 전체 PASS / 블록 5 PASS / 알려진 위반 FAIL** · `log_pipe_backpressure selftest` · **`stats_delta` 옛 증거(R3 §4.5) RED / 새 증거 PASS** · `check_occurred_at --selftest`. 봇 `cargo test` **70 / 0** | **PASS** — 새 도구마다 **양방향**(알려진 위반을 잡고, 깨끗한 구간을 통과시킨다) |

**재실행하지 않은 블록 0 항목(기록)**: SC-02~04(sim 의존성·금지 함수·결정성 위생), SC-35~50(계약·생성기·EditMode). 리더 지정 범위 밖이다. **SC-47·49 는 유효 27 로 기준이 바뀌었으므로** 블록 6 세션이 Unity EditMode 를 돌릴 때 **27 발견 / 21 왕복**을 qa 손으로 확인해야 한다.

---

## 3. 인계 — 블록 6~9 (새 qa 세션용)

**이 절과 `evidence/` 만으로 시작할 수 있게 쓴다.** 앞 라운드 리포트는 필요할 때 그 절만 읽는다(r3 §5~§11, 이 리포트 §0~§2).

### 3.1 지금의 상태 (2026-09-22)

| 무엇 | 값 |
|---|---|
| 판정 완료 | SC-01·08~14·15~24·28~34·35~50(r1/r2)·51·52·55·66~68·76·79·80·82~87 등 — 최신 집계는 r3 §8 + 이 리포트 §0~§2. **이번 라운드 PASS**: SC-11(전 절)·SC-88 (a)(b)(c)(e)(f)·SC-01·82·83·84·86·87 |
| **남은 판정** | **블록 6**: SC-53·54·56·57·58·**59**·60 (+ SC-88 **(d)**) / **블록 7**: SC-61~65 / **블록 8**: SC-25·26·27·69~75 / **블록 9**: SC-77·78·**81** (+ SC-76·80 재확인) |
| 서버 바이너리 | `server/target/debug/starfall-game-server.exe` sha256[:16] **`9c5ce49fc2b0f110`**, 소스 트리 해시 **`d2adedf8f2bc3760`**(`python tests/e2e/make_red_logsink_binary.py --out x --hash-only`). **바뀌었으면 SC-01 부터 다시** |
| DB | 마지막 인스턴스 `start_tick 430357` ~ 종료 tick 431613. 다음 인스턴스는 그 뒤에서 시작한다. **`docker compose down` 금지(G-a)** |
| 영구 기록 | **자기 참조 7행**(동결 장부, 스펙 §11-8 / `ship_events.py` 의 `FROZEN_DEFECT_LEDGER`) · 라운드 2 의 짝 없는 함선 3척. **지우지도 고치지도 않는다**(원칙 5) |
| 열린 요청 | server: 릴리스 안전망의 "이벤트를 쓰지 않는다" 실행 증거(§1.4). 블록 진입을 막지 않는다 |

### 3.2 블록별 진입 조건·도구·함정

**블록 6 — Unity 실서버·예측·육안 (SC-53·54·56·57·58·59·60, SC-88(d))**
- 선행: client 가 Unity Editor 를 실서버에 붙인다(현재 Unity 미실행). **G-e: 측정 중 `client/` 저장 금지.** SC-58 은 Deep Profile 끔.
- **SC-59(부호)는 사람이 본다** — 화면 입력 상태가 함께 찍힌 클립, (a)(d). **이것 없이는 슬라이스가 부호에 대해 아무것도 증명하지 못한다(계약 §0.10 — 리포트 요약에 반드시 싣는다).**
- 먼저 **Unity EditMode 를 qa 가 직접 한 번**: SC-47·49 의 새 기준 **27 발견 / 21 왕복**(client 보고값의 재현, I-25).
- ⚠ **운영 주의 (client 발견)**: 블록 6 의 SC-59 녹화(단일 세션, `StarfallNetHost`)와 블록 7 의 2세션 하네스(`TwoSessionHarness.cs:61~68`)가 **subject A 에 같은 identity 해석 함수**(`DevAuthToken.ResolveSubject()`)를 쓴다. 둘을 동시에 띄우면 나중 것이 먼저 것을 **이번에 구현한 넘겨받기로 밀어낸다**(4001, 재접속 안 함). **버그가 아니라 설계대로의 동작**이지만, 모르면 SC-59 녹화가 이유 없이 끊긴 것처럼 보인다. **블록 6·7 은 순차 실행**하거나 한쪽에 `STARFALL_DEV_ACTOR_SUBJECT` 를 다르게 줘라.
- **SC-88 (d) 를 재는 설계(바로 위 주의를 거꾸로 쓴다)**: Unity 를 **S1** 으로 붙인 뒤 **같은 subject** 로 봇 S2 를 붙인다 — 봇 라벨이 아니라 Unity 의 subject 로 토큰을 만들어야 한다(`bots token` 은 라벨 → subject 규칙이 다르다: **Unity 쪽 `STARFALL_DEV_ACTOR_SUBJECT` 를 봇 subject(`bots subjects --count N` 출력)와 같게 두는 쪽이 쉽다**). 판정: ① Unity 가 **close 4001** 을 받고 client 의 **grep 가능한 로그 한 줄**을 남긴다 ② **관측 창 ≥ 10 s(백오프 상한)** 동안 그 actor 의 `SESSION_OPENED` 가 **S2 말고 없다**(DB) ③ 양성 대조 — 4001 이 **아닌** 끊김(예: 서버 재기동 없이 소켓만 끊기 어렵다면 client EditMode 의 `ReconnectPolicy` 양성 대조 6종)을 증거로 함께 둔다. **봇 S1 로 재면 항진명제다.**

**블록 7 — 2 클라이언트 가시성 (SC-61~65)**
- **§0.11: 관측자 B 를 봇으로 대체하지 않는다**(부호가 뒤집힌다). 한 Unity 프로세스 안 세션 2개(`TwoSessionHarness`). 막히면 SC-64 만 봇, **SC-65 는 미검증(E8)**.
- 블록 6 과 **순차**(위 운영 주의).

**블록 8 — 부하 A·B·C·D + 성능 (SC-25·26·27·69~75)**
- **31번째 연결 = 사람이 띄운 Unity PlayMode 가 먼저**(G-d). **빌드 금지**(G-c) — 부하 전에 `cargo build`·`cargo test --no-run`·봇 빌드를 끝낸다.
- 봇 게이트는 이제 **3단언 + `accepted > 0`**(입력이 0 이면 빨간불, r3 §5.1). 봇 결과의 `GATES` 줄에 `(accepted N)`·`(ping pairs N)` 이 찍힌다 — 초록불 옆의 N 을 함께 적는다(§7a).
- 동시 컨테이너 수를 적는다(이 PC 에는 livingfeed 5 개가 떠 있다).
- 서버는 **`tests/e2e/server_boot.py serve`**(stdout 드레인 경로)로 띄운다. 드레인 없는 `subprocess.PIPE` 는 라운드 2 에서 서버를 세웠다(r3 §1).
- 로그가 4 KiB 에서 끊겨 보이면: `/healthz`·`/debug/stats` 가 `000` 이면 정지, 정상이면 드롭(로그 싱크가 `[로그 싱크] … 버렸다` 를 stdout 에 찍는다).

**블록 9 — 기록 무결성 (SC-77·78·81 + SC-76·80 재확인)**
- **SC-81 은 I-29 수정을 기다리지 않고 돈다.** 도구: `ship_events.py ledger`(구간 없이 = 표 전체 결함 == 동결 장부 7건, 구간 주면 = 그 구간 0) + `causation-selftest`. 현재 표 전체 실측: 함선 166 · 세션 4360 · `SUPERSEDED` 2, 결함 7(= 장부).
- SC-78: `ship_events.py resume`. SC-76: `pairs`(I-41 전용 — **I-29 판정에 쓰지 않는다**, 동시 겹침은 `overlap`).
- SC-80(`types`) 은 구간 한정(G-a) — 라운드 2 방식.

### 3.3 이 슬라이스에서 배운 것 — 판정에 그대로 적용할 것

1. **초록불 옆에 "그 조건이 실제로 일어났다"를 함께 적는다**(계약 §7a). 항등식은 입력 0 에서 반드시 빨간불이어야 하고, 도구는 **양방향**(알려진 위반 FAIL / 깨끗한 구간 PASS)을 보여야 한다.
2. **서버를 띄우는 드라이버도 qa 도구다** — 드레인 경로로 띄운다. 하네스를 용의선상에서 빨리 빼지 않는다(r3 §1.5).
3. **집계는 순차와 동시를 가르지 못할 수 있다** — `pairs` 의 actor 목록이 I-29 동시 위반을 정보로 흘려보냈다(r3 §6.7).
4. **"존재" 만 보는 인과 검사는 자기 참조를 통과시킨다** — 타입 + ≠ self + 순서(SC-81).
5. 원자료를 남긴다(`--series-out` 등) — 판정 방식을 바꿔 다시 볼 수 있어야 한다(r3 §6.6).
6. **맥락 관리**: 블록마다 리포트에 즉시 쓰고, 긴 출력은 파일로 받아 `grep`/`tail` 만 본다. 블록 6~8 은 산출물이 크다 — 블록 경계에서 멈추고 보고해도 된다.

---

## 4. 빌드 단계 — 블록 0 재실행 (qa2, 2026-09-22, 측정 전)

작성: qa2(블록 6~9 세션). 증거 `evidence/R4-B6/B0/`. 시작 시 서버 바이너리 sha256[:16] `9c5ce49fc2b0f110`(§3.1과 같음), **트리 해시는 이미 `45970d728885630f`로 바뀌어 있었다**(server 가 릴리스 안전망 테스트를 쓰는 중 — `crates/sim/src/simulation.rs` 만 바이너리보다 새것). Unity Editor 미실행, docker 7(starfall 2 + livingfeed 5).

| 항목 | 명령 | 결과 | 판정 |
|---|---|---|---|
| **SC-46** | `unity test client --mode EditMode --report-format nunit,junit --output …/unity-tests-qa-r4/EditMode.nunit.xml --junit-output …/EditMode.xml` | **exit 0**, 리포트 2개, **tests 159 / passed 157 / failed 0 / skipped 2**(`LiveServerTests` env-gate 2건, 기존) / inconclusive 0 | **PASS** |
| **SC-47** | 위 리포트 `Fixtures_RoundTrip_Found27_RoundTripped21` 출력 | **qa 가 출력에서 직접 셈: `found` 줄 27(서로 다른 경로 27) / "round-trip candidates: 21"**, Passed. client 보고값(27/21)을 qa 손으로 재현(I-25) | **PASS** |
| **SC-48** | `CSharpLayerSplit_Totals34` 외 8건 | 출력 `reject 18 / accept 10 / no layer 6 = 34`, 계층 없음 6건 각각 판별자 없음 | **PASS** |
| **SC-49** | (c)`FixtureLoader_FailsWhenTooFewValidFixtures` (d)`WorldSnapshot_EmptyShipsArray_RoundTrips`·`WorldSnapshot_TwoShipsOneLingering_RoundTrips` (e)`Runtime_ToleratesUnknownFieldInsideShipStateArrayElement_AndStrictThrows` | 4건 Passed. (c) 출력 **"guard sees 26 of 27 expected fixtures"** — 새 기준 27 로 가드가 켜진다 | **PASS** |
| **SC-50** | `ClientDataCopy_MatchesRepositoryOriginal` + **qa 독립 `sha256sum`**(`evidence/R4-B6/B0/SC-50-data-copy.log`) | 레포 `data/` json 3 = client `Assets/_Project/Data` json 3, **3/3 바이트 동일** | **PASS** |
| **SC-53** | `Reconcile_IsPureFunction_SameInputsTwice_SameOutput`, `Reconcile_DoesNotDependOnClockOrCallOrder` | 2건 Passed. (2) 의 테스트는 **호출 사이에 벽시계를 바꾼다**(5 ms 수면 + 할당 10만). 프레임 시간·도착 시각은 EditMode 에서 바꿀 수 없어 **정적 보조**: `Reconcile(history, confirmed, ackInputSeq, ship, boundary, dt)` 에 시각 매개변수가 없고, `Starfall.Flight/*.cs` 에 `DateTime|Stopwatch|Time\.|Environment.TickCount|UnityEngine|Random` **0건**(grep exit 1) | **PASS** — 근거 종류: 실행 2건 + 정적 1 |
| **SC-54** | `Reconcile_HistoryWithASkippedInputSeq_StillConvergesToServerState` | Passed (입력열 1,2,4,5) | **PASS** |
| SC-60 (b)(c)(d) 로직 | `RemoteInterpolationTests` 5 · `RemoteShipBufferTests` 6 · `RemoteShipRegistryTests` 4 = **15건** | 전부 Passed. (b) `Slerp_AtQuarterPoint_DiffersFromNormalizedLerp_ForLargeAngle`(t = 0.25) (c) `GetDisplay_PastExtrapolationCap_FreezesAndZeroesVelocity` (d) `OnSnapshot_LingeringShip_IsKept_NotRemoved`·`…Absent…_IsRemovedImmediately` | 로직 확인. **SC-60 판정은 블록 6 실화면(a)(e) 뒤** |
| SC-88 (d) 양성 대조 | `RealtimeClientTests` 12 + `TransportHandshakeTests.RemoteClose_WithApplicationRangeCode4001_IsReadableAsCloseCode` = **13건** | 전부 Passed. 4001 이외 6종(무코드·1000·1001·1006·4000·4002)은 **재접속을 스케줄한다**, 4001 만 안 한다, 실소켓으로 4001 이 `CloseCode` 에 읽힌다 | (d) 판정은 블록 6 실서버 뒤 — 이것은 대조군 |
| **SC-42** | `dotnet run ContractsCodegen.cs … --out <tmp>/gen1`, 같은 명령 gen2, `--check` 양쪽 | run1·run2 exit 0, **gen1 == gen2**(diff exit 0), `--check` **up to date (11)** exit 0, client `Generated/` 도 `--check` exit 0. **qa 재생성 11 파일 == client 제출본 11 파일 바이트 동일** | **PASS** |
| **SC-43** | 생성물 실물 | `ShipState[] Ships`, `ShipState` 중첩, JsonProperty **18** | **PASS** |
| **SC-44** | 위 실행 stdout | `skipped SHIP_CLASS/STAR_SYSTEM/SYNC_TUNING (kind 'data': …)` **3줄**, 세 `.cs` 부재 | **PASS** |
| **SC-45** | `git diff HEAD(42a7965) -- Generated/` | 기존 6타입 중 **5개 바이트 동일**, `SessionClosedEvent.cs` **1줄 변경**(= `close_reason` XML 주석에 `SUPERSEDED` 설명, 스키마 description 그대로) · `ContractTypes.cs` **+8/−0** | **PASS (7차 개정 해석)** — 1줄 변경은 **생성기 변경이 아니라 7차 계약 데이터 변경**(스키마 description)의 결과다. 계약 문구 "6개 바이트 동일" 은 7차 이전 기준이다 → §계약 자체 문제에 적는다 |
| **SC-35~40** | `cargo test -p starfall-contracts --locked -- --nocapture` | **33 + 10 passed / 0 failed**(bless 1 ignored), exit 0. 출력: 스키마 **18** 오프라인 검증 · 유효 fixture **27** 왕복 · 반례 **34** 스키마 거부 · serde 매트릭스 **34/34 기대=실제**(전부 "거부") · required 변이 **278**건 전부 실패 · 레지스트리 **13** / server 태그 13 대응표 · `integer_bounds_rejected` ok. kind 별: server_message 8 / command 4 / domain_event 9 / data 6 = 27 | **PASS** |
| SC-02 | `grep -nE "axum\|sqlx\|redis\|rand\|chrono\|tokio\|hash\|glam\|nalgebra" crates/sim/Cargo.toml` | exit 1(0건). `[dependencies]` = `starfall-contracts` 하나, dev = serde·serde_json | **PASS** |
| SC-03 | 순진한 grep / ADR-0010 §3 정밀 grep (`evidence/R4-B6/B0/SC-03-04-87.log`) | 순진 **118줄**(오탐 — p0-02 때 13 에서 코드가 늘었다) / 정밀 **0건**(exit 1). **양성 대조**: 같은 정밀 정규식이 심은 `x.sin()`·`f64::hypot`·`mul_add` 줄을 잡고 `.expect(` 줄은 안 잡는다(1/1) | **PASS** |
| SC-04 | grep 4종 | `SystemTime`/`Instant` 코드 0(주석 1) · `HashSet` 은 `session.rs` 의 `seen`(insert/remove 만, 순회 0) · `ships: BTreeMap<UuidV7, …>`(`:373`), 순회 6곳 전부 `self.ships.{keys,values}` | **PASS** |
| SC-87 | `cargo test -p starfall-sim --test determinism --locked` + `git diff --exit-code -- …/replay/` | 1 passed(1 ignored) · diff exit 0 · golden `824a6489…`/`c924e59f…`/`3e00e527…`(§2 와 같음) | **PASS** |


### 4.1 server 트리 변경 → SC-01 재실행 · 릴리스 안전망 재판정 (r4 §1.4)

server 의 "이제 빌드하지 않는다" 신호(07:4x) 뒤. 변경 파일은 `crates/sim/src/simulation.rs` 하나(테스트 1건 신설 + 디버그 테스트에 `#[cfg(debug_assertions)]`).
**측정 바이너리를 다시 만들었다**: `cargo build -p starfall-game-server --locked` → sha256[:16] **`9c5ce49fc2b0f110` → `d5e47ed7296b87bc`**(gateway·game-server 재컴파일). 트리 해시 **`45970d728885630f`**. 블록 6~9 의 모든 실서버 측정은 이 바이너리다. 이후 빌드 없음(G-c).

| 항목 | 명령 | 결과 | 판정 |
|---|---|---|---|
| **SC-01** | `cargo fmt --all --check` / `cargo clippy --workspace --all-targets -- -D warnings` / `cargo test --workspace --locked` ×3 | fmt 0 · clippy 0 · **167 passed / 0 failed / 4 ignored ×3**(간헐 실패 없음) | **PASS** |
| SC-01 (8차 넷째 명령) | `cargo test -p starfall-sim --release --locked` · `cargo clippy --release -p starfall-sim --all-targets -- -D warnings` | **50 passed / 0 failed / 1 ignored**, `r4_s3_release_arm_signals_without_fabricating_an_event ... ok` · release clippy 0 | **PASS** |
| **r4 §1.4 릴리스 팔** "원인 없는 디스폰은 이벤트를 쓰지 않고 신호만 남긴다" | 위 릴리스 실행 + **qa 독립 고장 주입**(레포 밖 복사본, `CARGO_TARGET_DIR` scratch — `server/` 무변경) | **변이 A**(신호 없이 `unwrap_or(ship_id)` 로 원인을 지어냄) → **FAILED** `:2141 "릴리스 팔이 신호를 남겨야 한다" left [] right [UuidV7(…0004)]` / **변이 B**(신호는 남기되 이벤트도 씀) → **FAILED** `:2152 "릴리스 팔은 이벤트를 쓰지 않는다"` / 원본 → ok. **두 단언이 각자 빨간불을 켠다**(server 의 RED 는 A 계열 하나였다) | **미검증(테스트 부재) → PASS** |

**⚠ 표준 게이트의 공백(리더 지적, qa 실측으로 확인)**: 디버그 3회 로그(`SC-01-test-run{1,2,3}.log`)에 `r4_s3_release_arm` 이 **0회**, 릴리스 로그에 1회. 디버그 `should_panic` 팔은 반대로 디버그에 1 / 릴리스에 0 — 총 수 167 이 그대로라 **숫자로는 보이지 않는다**(§7a 의 `world_full` 과 같은 형태).
→ **계약 8차 개정으로 SC-01 에 넷째 명령을 넣었다**(`02_sprint_contract.md` SC-01 행 · §9 8차 행). 이유: I-30("원인을 지어내지 않는다")의 유일한 실행 증거이고, 넣지 않으면 누가 `return` 을 지워도 표준 게이트는 초록이다. 비용은 sim 릴리스 빌드 1회. **다음 슬라이스의 표준 게이트(`integration-qa` 체크리스트·CI)에도 같은 명령을 올릴 것** — "다음 슬라이스로 넘길 것" 에 적는다.

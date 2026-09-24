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

---

## 5. SC-59 판정 · 실서버 발견 분류 · 블록 9 (qa3, 2026-09-23)

작성: qa3. 범위는 리더 지정 — **사람 손이 필요 없는 구간**(SC-59 판정, 발견 2건의 추적 설계, 블록 9). 블록 6·7·8의 실행 항목은 건드리지 않았다.
증거 `evidence/R4-B9/`. 인프라는 떠 있는 상태 그대로 썼다(postgres 15432 / redis 16379). **`docker compose down` 계열 미사용(G-a).** 커밋하지 않았다.

### 5.0 요약에 반드시 싣는 문장 (계약 §0.10)

> **손 방향 부호가 통째로 뒤집혀도 자동 검증은 전부 통과한다.** 서버와 클라이언트가 같은 공식을 쓰므로 예측 오차 0, fixture 왕복 통과, 결정성 바이트 동일까지 전부 초록이다. **검출기는 SC-59(AC-14 a·d)의 육안 관찰 하나뿐이다.** 그것을 사람이 실제로 볼 때까지 이 슬라이스는 부호에 대해 **아무것도 증명하지 못한다.**

**이 문장은 이번 라운드 뒤에도 그대로 유효하다.** 아래 5.1의 이유로 SC-59는 닫히지 않았다.

---

### 5.1 SC-59 — **FAIL (증거 요건 미충족)**. 부호가 뒤집혔다는 뜻이 아니다

> **⚠ 이 절의 판정은 §5.9 (2) 에서 `미검증(증거 요건)` 으로 재판정됐다**(계약 9차로 §0.3 에 그 칸이 신설됐다). **아래의 근거는 하나도 바뀌지 않는다** — 바뀐 것은 귀속(제품 결함 → 증거 결함)뿐이고, **비-통과로서 슬라이스 종료를 막는 힘은 FAIL 과 같다.**

**먼저 오해를 막는다**: 이 FAIL 은 **"부호 버그가 발견됐다"가 아니다.** 사람이 관찰한 항목은 **전부 기대와 일치했고**, 부호가 뒤집혔다는 증거는 하나도 없다. FAIL 인 이유는 **SC-59 가 요구한 관찰이 완결되지 않았고, 남은 녹화가 계약이 명시한 실격 조건에 걸리기 때문**이다.

#### (1) 계약 SC-59 가 요구한 것 vs 실제로 관찰된 것

| 계약 SC-59 조항 | 관찰 | 근거의 종류 | 판정 |
|---|---|---|---|
| (a) 전방 추력이 **뱃머리 방향** | "W 눌렀는데 뱃머리 방향으로 잘 나간다" | **사람의 관찰 진술만** (녹화 뒷받침 없음 — 아래 (2)) | 미충족 |
| (a) **마우스 오른쪽이 오른쪽 선회** | — | **관찰 자체가 없다** | **미관찰** |
| (a) **위 추력이 위로 민다** | — | **관찰 자체가 없다** | **미관찰** |
| (b) **기준 마커 4개**가 보이고 그것 대비 이동이 읽힌다 | 시야에 정적 오브젝트 **1개**, `origin_distance_m` 2512.5 → 1599.4 | 사람 관찰 + HUD 수치 | **1/4 — 미충족** |
| (c) soft 경고 | `BOUNDARY WARNING (soft crossed)` | **qa3 가 녹화 프레임에서 직접 읽었다** (아래 (3)) | 충족 |
| (c) hard 에서 **튕김 없이 미끄러짐** + 그 상태에서도 조작이 먹음 | "3개다 정상작동해" | 사람의 **주관 판단** 진술만 | 미충족 |
| (d) ① 손 떼면 수평 복귀 ② **수동 롤 중 복귀 안 함** | "1,2 둘다 제대로 작동하는 것 같아" | 사람의 관찰 진술만, **"~같아"로 유보된 표현** | 미충족 |

#### (2) 녹화는 계약이 명시한 실격 조건에 걸린다 — qa3 실측

계약 SC-59 본문: *"**형식보다 내용이 판정한다 — 화면에 입력 상태가 떠 있지 않으면 그 영상은 부호를 증명하지 못한다**"*. 따라서 "클립 4개로 자르지 않았다"는 형식 문제가 아니라, **해당 순간에 입력 상태가 화면에 있었는가**가 판정 기준이다. qa3 가 `SC-59-session.mp4`(900 s, 3440×1440, 15 fps, 160,055,377 B)에서 프레임을 직접 뽑아 확인했다.

| 관찰 | 프레임 | 결과 |
|---|---|---|
| **색인 PNG 6장이 HUD 를 담고 있지 않다** | `evidence/R4-B6/sc59/index-t{90,240,400,560,700,840}s.png` | `07-recording-index.md` 는 "HUD 가 보이도록 좌상단 1146×720 으로 잘랐다"고 적었지만, 확인한 것은 **전부 Claude Code 터미널 창**이다. 좌상단에 Unity 가 없었다. **색인 PNG 는 SC-59 증거가 아니다** |
| **HUD 가 실제로 찍힌 구간은 있다** | `evidence/R4-B9/hud-t600s.png` (t = 600 s) | Game view 전체 HUD 가 **읽을 수 있게** 찍혀 있다 — `tick=463630` `ack_input_seq=5308` `speed_mps=17.8` `origin_distance_m=11947.5` `thrust=(0,0,0) roll=0 brake=False assist=True` `aim_target=(28443,-966332,-122419,-224517)` 등 계약이 요구한 항목이 전부 있다 |
| **⚠ 클립 a(W 8초) 순간에는 HUD 가 화면에 없다** | `evidence/R4-B9/right-t205s.png`, `right-t213s.png` (t ≈ 205·213 s = tick ≈ 455730·455890) | Game view 는 렌더링되고 마커 구체가 가까워지는 것이 보이지만, **Game view 좌상단에 HUD 텍스트가 한 줄도 없다.** `thrust=` 가 화면에 없으므로 **이 구간은 계약 문구상 부호를 증명하지 못한다** |
| **⚠ 조종 구간 상당 부분에서 Unity 가 화면에 없다** | `evidence/R4-B9/contact-250to360.png`, `full-t450s.png` | t = 270·300·320·340·450 s 에서 전경 창은 **Chrome 또는 터미널**이고 Unity 는 화면에 없다 |

**그래서 (a)(c)(d) 의 근거는 결국 "사람의 관찰 진술"뿐이고, 녹화가 그것을 뒷받침하지 못한다.** 계약이 녹화를 요구한 이유가 바로 그 짝짓기(진술과 화면 상태)이므로, **§0.3 의 PASS 조건을 충족하지 못한다.**

**HUD 가 없는 방향을 정확히 적는다 (리더 지적, 2026-09-23).** 위 두 관측은 **`t ≈ 213 s` 없음 → `t = 600 s` 있음** 이다. 즉 **초반에 없다가 나중에 나타났다** — 세션 중간에 사라진 것이 아니다. 방향이 반대이면 읽는 뜻이 달라진다:

- **client 의 가설과 더 잘 맞는다**: `OnGUI` 콜백이 Game View 가 비활성·비포커스일 때 불리지 않는다면, **포커스를 잡기 전인 초반이 정확히 그 구간**이다. 어제 초반에는 사용자가 절차 안내를 읽느라 다른 창을 보고 있었을 가능성이 크다(§5.2 (가)의 전경 창 표가 같은 그림을 보여 준다).
- **재촬영의 위험 구간이 초반이고, 하필 클립 a 가 거기 있다.** SC-59 에서 부호를 증명하는 유일한 절(§0.10)이 **가장 위험한 구간에 배치돼 있다** — 그래서 §5.1 (4) 의 절차는 "시작 전 확인" 한 번으로 부족하고, **① 시작 직후 ② 클립 a 직전** 두 번 확인해야 한다. 더 나은 형태는 **사용자가 촬영 내내 Game View 에서 포커스를 떼지 않아도 되도록, 할 일을 촬영 시작 전에 전부 알려 주고 시작하는 것**이다(리더가 client 에게 체크리스트로 지시).

**⚠ 이것은 추정이고 판정이 아니다.** qa3 가 확인한 사실은 **"그 두 시각에 HUD 가 없었고 있었다"** 뿐이다. **왜 없었는지는 client 가 설명해야 한다** — 설명 없이 재촬영하면 같은 일이 또 날 수 있다.

#### (3) qa3 가 프레임에서 직접 읽은 것 (이것만은 재현 가능한 증거다)

`evidence/R4-B9/hud-t600s.png` — **tick 463630, `origin_distance_m=11947.5`, `BOUNDARY WARNING (soft crossed)`**. 계약 (c) 의 "soft 에서 경고"는 **사람의 진술이 아니라 화면 증거로 충족된다.** 나머지 (c) 조항(튕김 없는 미끄러짐·조작 유지)은 단일 프레임으로 판정할 수 없다.

#### (4) SC-59 를 닫는 방법 — tick ↔ 영상 시각 대응이 확정됐다

`07-recording-index.md` 가 "방법이 있다"고만 적은 것을 qa3 가 **수치로 확정**했다. t = 600 s 프레임의 HUD 가 `tick=463630`, tick 은 20 Hz 이므로

```
t_video(초) = 600 − (463630 − tick) / 20
tick        = 463630 − (600 − t_video) × 20
```

교차 확인: 그 프레임의 `origin_distance_m = 11947.5` + `BOUNDARY WARNING` 이 사용자 클립 c 서술("12,000 m 경계 도착")과 일치하고, 이 식이 그 순간을 재접속(tick 458321) 뒤로 놓는 것이 세션 서술과 맞는다. **앵커가 프레임 1장이므로, 실제로 자를 때 두 번째 앵커로 재확인할 것.**

이 식을 쓰면: 클립 a 연장(tick 455892) → **t ≈ 213 s**, 위반(tick 457993) → **t ≈ 318 s**, 재접속(tick 458321) → **t ≈ 335 s**, 정상 종료(tick 466602) → **t ≈ 748 s**.

**그러나 잘라도 (a) 는 닫히지 않는다** — t ≈ 213 s 에 HUD 가 없기 때문이다. **SC-59 를 닫으려면 짧은 세션을 한 번 더 해야 한다.** 필요한 것은 5~10분이다:

1. Unity 창을 **전경에 두고** 녹화한다(전체 데스크톱이 아니라 Unity 창만 잡아도 된다).
2. **Game view 에 HUD 가 떠 있는지 먼저 눈으로 확인**한다 — 어제 t ≈ 213 s 에 없었던 이유를 client 가 먼저 설명해야 한다(포커스 의존인가? 특정 시점에만 그리는가?).
3. 네 동작을 각각 **정지 → 입력 → 이동 → 입력 해제** 순서로: **(a) W · 마우스 오른쪽 선회 · 위 추력 3종**, (b) 마커가 보이는 시야, (c) 경계, (d) 오토레벨 ①②.
4. (b) 는 마커 4개를 한 프레임에 담거나, 담기지 않으면 **"4개"를 요구하는 계약 문구를 architect 가 고쳐야 한다**(§5.5 1번).

---

### 5.2 실서버 발견 2건 — 분류와 추적 설계

#### (가) `PROTOCOL_VIOLATION` 강제 종료 → **새 항목 SC-89 로 낸다**

**분류 결정**: 계약 외 발견으로 두지 않고, **AC-18/SC-26 계열에도 붙이지 않고, 새 SC-89 로 낸다.**
- 계약 외 발견으로 두면 판정에 쓰이지 않는다 — 그러나 이것은 **플레이어에게 "갑자기 튕겼다"로 보이는 사건**이고, 00_request 3번에 직접 걸린다.
- SC-26(부하 중 손실)·AC-18 은 **31 연결 부하**의 항목이다. 이 사건은 **단일 클라이언트, 부하 없음**에서 났다. 붙이면 부하 항목이 아닌 것을 부하 항목으로 판정하게 된다.
- 계약에 "정상 플레이 중 프로토콜 위반으로 끊기지 않는다"가 **없는 것이 공백**이므로, 공백을 메우는 것이 맞다.

**⚠ 리더의 서술 한 곳을 정정한다.** `06-finding-protocol-violation.md` 는 "사용자는 … 평범하게 비행했다", "정상 플레이가 강제 종료로 이어졌다"로 적었다. **qa3 가 녹화에서 확인한 바로는, 끊김 순간에 Unity 는 전경 창이 아니었다.**

| 프레임 | tick(위 식) | 전경 창 |
|---|---|---|
| t = 270 s | 457030 | Chrome |
| **t = 300 s** | **457630** | 터미널 |
| — | **457993** | ← **`PROTOCOL_VIOLATION` 발생** |
| **t = 320 s** | **458030** | Chrome |
| t = 340 s | 458430 | Chrome |
| t = 360 s | 458830 | 터미널 |

**위반을 사이에 둔 두 프레임(457630·458030)에서 Unity 는 전경이 아니다.** 즉 이것은 **"평범한 비행 중"이 아니라 "Editor 가 백그라운드에 있는 동안"** 일어났다. 증거: `evidence/R4-B9/contact-250to360.png`.

**그리고 원인 후보를 지지하는 직접 관측을 하나 찾았다.** t = 600 s 프레임의 Unity 상태 바에 다음이 떠 있다(`evidence/R4-B9/statusbar-t600s.png`):

```
⚠ starfall.net: outbound queue full (256), refusing to send
```

**클라이언트의 송신 큐가 상한 256 까지 차서 송신을 거부하고 있었다.** 송신 측이 소켓이 빼가는 것보다 훨씬 빠르게 메시지를 만들어 넣었다는 뜻이고, 리더가 적은 가설("히치·백그라운드 뒤 밀린 입력이 한꺼번에 나간다 → 한 tick 에 9건 이상")과 **형태가 일치한다.** 백그라운드 Editor 가 틱을 밀었다가 스케줄이 복구될 때 한꺼번에 흘려보내면 tick 당 상한 8 을 넘긴다.

**다만 이것은 여전히 정황이다.** `outbound queue full` 은 t = 600 s(위반 **뒤**, 재접속 후 구간)의 관측이고, 위반 시점(457993)의 큐 상태는 보지 않았다. **인과는 client 의 송신 경로 조사 결과로 확정한다** — qa3 가 `client` 에게 ① 송신 루프 구조와 catch-up 경로 ② 송신 건수 카운터 ③ `PROTOCOL_VIOLATION` 시 grep 가능한 로그 한 줄을 요청해 두었다. **답이 오면 SC-89 (d) 문구를 그 결과로 확정한다.**

**제안 항목 (architect 승인 필요 — qa3 가 계약 표에 넣지 않았다):**

> **SC-89 — 조작이 없거나 클라이언트가 백그라운드여도 프로토콜 위반으로 끊기지 않는다.**
> **관찰**: Unity 클라이언트를 실서버에 붙이고 **전경 5분 + 백그라운드 5분**(다른 창을 띄워 Editor 를 비활성화한 채) 유지한다. 판정:
> **(a)** 그 구간 그 actor 의 `SESSION_CLOSED` 중 `close_reason = 'PROTOCOL_VIOLATION'` 이 **0건**이다 (DB).
> **(b) §7a — 조건이 실제로 만들어졌음을 함께 단언한다**: 그 구간의 `/debug/stats` 델타에서 `commands_received_total > 0` 이고, **백그라운드 구간이 실제로 있었다**(화면 녹화 또는 client 의 포커스 이탈 로그 한 줄). "N분 돌렸다"는 수단이지 조건이 아니다.
> **(c) 양성 대조 — 이 검사가 항진명제가 아님을 보인다**: 같은 하네스에서 **한 tick 에 9건 이상을 일부러** 보내면(`tests/e2e/raw_ws_send.py` 또는 봇 burst) `PROTOCOL_VIOLATION` 이 **실제로 난다**. 나지 않으면 (a) 의 초록은 아무것도 뜻하지 않는다.
> **(d)** 그 구간에 클라이언트가 **`outbound queue full` 을 찍지 않는다.** 찍는다면 그 자체를 FAIL 로 본다(송신 측 역압이 이미 무너진 상태다).
> **금지(계약 §7)**: tick 당 상한 **8** 과 위반 예산 **8건/10초** 를 **늘려서 닫지 않는다.** 늘리면 S10 이 막으려던 burst 가 다시 열린다. 원인 확인이 먼저다.
> 담당: client(송신 경로) · server(상한 설계 검토) · qa(판정) / 근거: `evidence/R4-B6/sc59/06-finding-protocol-violation.md` + `evidence/R4-B9/statusbar-t600s.png` · `contact-250to360.png`

#### (나) 재조정 이상치 — **새 SC 를 내지 않는다. SC-56 / M-9 로 판정한다. 다만 수치가 기록보다 나쁘다**

**분류 결정**: SC-56(재조정 오차 임계)과 M-9(`reconcile_hard_snap_total` 목표 0)이 **이미 이 성질을 판정하는 항목**이다. 새 항목을 만들면 같은 것을 두 번 판정하게 된다. **SC-56 은 블록 6 항목이고 아직 대기다** — 이 관측은 그 판정의 입력이다.

**qa3 가 프레임에서 읽은 값이 기록과 다르다** (`evidence/R4-B9/hud-t600s.png`, tick 463630):

| 지표 | 정지 시 (tick 449076) | W 8초 뒤 (tick 455892) | **tick 463630 (qa3 실측)** | 목표 |
|---|---|---|---|---|
| `reconcile_hard_snap_total` | 1 | 5 | **3** | **0** (M-9) |
| `reconcile_error_m` p99 / max | 0.0000 / 0.0000 | 0.0012 / 56.3096 | 0.0012 / **323.1212** | p99 < 0.25 m |
| `reconcile_error_deg` p99 / max | 0.0001 / 34.1309 | 1.0999 / 80.5663 | 0.9891 / **80.5663** | (출력만) |
| `cl2_reconcile_has_error_total` = n | 77 | 1013 | **2627** | 0이면 FAIL |
| `ack_input_seq` | 265 | 7081 | **5308** | — |

**여기서 세 가지가 나온다.**

1. **HUD 카운터가 재접속에서 초기화된다.** `reconcile_hard_snap_total` 이 5 → 3 으로, `ack_input_seq` 가 7081 → 5308 로 **줄었다.** 단조 증가 카운터가 줄 수 없으므로 tick 458321 의 재접속에서 초기화된 것이다(재접속 이후 경과 tick 463630 − 458321 = 5309 ≈ `ack_input_seq` 5308 이 뒷받침한다). **따라서 세션 전체의 hard snap 은 5 + 3 = 최소 8 건이고, 기록된 "5"는 전체가 아니라 끊기기 전까지의 수다.**
2. **위치 오차 최댓값이 56.3 m 가 아니라 최소 323.1 m 다.** p50·p99 는 여전히 0 에 가깝다(0.0004 / 0.0012) — **드문 대형 이탈**이라는 형태는 그대로이고 크기가 한 자릿수 더 크다.
3. **⚠ `reconcile_error_deg` 의 max 만 80.5663 으로 재접속 전후가 소수 넷째 자리까지 같다.** n 과 다른 지표는 초기화됐는데 이 값만 같다. 우연으로 보기 어렵다 — **`_deg` 의 max 는 초기화되지 않거나, 어떤 값에서 포화·클램프될 가능성**이 있다. **지표 자체의 신뢰성 문제**이므로 SC-56 판정 전에 client 가 확인해야 한다(§7a: 관측 자신을 먼저 의심한다).

**(가)와 (나)의 관계**: 기록대로 "끊김이 hard snap 을 만들었다"는 성립하지 않는다(hard snap 5 는 tick 455892, 끊김은 457993). **qa3 도 인과를 판정하지 않는다.** 다만 `outbound queue full (256)` 은 **두 현상의 공통 상류**(송신 측 역압 붕괴 → 명령이 서버에 닿지 않음 → 예측만 진행 → 어긋남 → hard snap / 밀린 명령의 burst → 위반)가 될 수 있는 단일 관측이라는 점을 기록한다. **확정은 client 의 송신 경로 조사 결과로 한다.**

---

### 5.3 블록 9 — 기록 무결성 (SC-76~84, SC-86)

**전제**: G-a — **`docker compose down` 계열을 쓰지 않았다.** 따라서 **SC-80 은 tick 구간 한정**으로 판정한다(계약 §0.2 의 대안 경로).
**이번 라운드 인스턴스 구간**: `start_tick = 443801`, 이벤트 tick 448809 ~ 467202. 판정에 쓴 구간 지정은 `--from-tick 443801`.
**표 전체 현황(qa3 실측)**: `domain_events` **4,533행**(어제 4,527 + SC-59 세션 6행), `QA_APPEND_ONLY_PROBE` 1, `SESSION_OPENED` 2184 / `SESSION_CLOSED` 2180 / `SHIP_SPAWNED` 86 / `SHIP_DESPAWNED` 82.

| 항목 | 명령 | 결과 (검사 건수 포함) | 판정 |
|---|---|---|---|
| **SC-76** | `ship_events.py pairs --from-tick 443801` | **`ships_checked` 1 / spawned 1 == despawned 1 == 집합 크기 1**, 짝 없음 0, 중복 0, `actors_with_more_than_one_ship` 0. exit 0 | **PASS** (구간) |
| **SC-77** | `check_sessions.py --corr <이 구간 correlation 2개>` | **`correlations_checked` 2**, `SESSION_OPENED` 2 == `SESSION_CLOSED` 2, 짝 없음/중복 0, DB 부재 0, `null actor_id` 0. `close_reasons` = `CLIENT_CLOSED` 1 / `PROTOCOL_VIOLATION` 1, **계약 enum 밖 사유 0**. exit 0 | **PASS** |
| **SC-78** | `ship_events.py resume --from-tick 443801` | **`ships_checked` 1**, 재개 함선 1건: ship `01a0ce35-…3fe1`, actor `01a0b1c2-7e57-…0001`, **sessions 2 / spawns 1**, violations 0. exit 0 | **PASS** — **실플레이(사람 조종) 재개의 최초 관측**이다. 봇이 아니라 강제 종료 뒤 Unity 가 재접속해 **새 함선 없이 같은 함선을 이어받았다** |
| **SC-79** | `check_sequence_gaps.py` (**전수**) | **`rows_total` 4533 / `distinct_world_tick_groups` 1735 / `violating_groups` 0**. exit 0 | **PASS** |
| **SC-80** | `ship_events.py types --from-tick 443801` | **tick 구간 한정**(G-a — `down` 미사용). `event_types` = `SESSION_OPENED` 2 / `SESSION_CLOSED` 2 / `SHIP_SPAWNED` 1 / `SHIP_DESPAWNED` 1 = **4종뿐**, `unexpected_types` **0**, 위치·상태 시계열 타입 0. exit 0 | **PASS** |
| **SC-81** | `ship_events.py ledger`(구간 없이 / 구간 / 블록5 / 알려진 위반 구간) + `causation-selftest` | **(b) 전수 등식 성립**: `ship_events_checked` **168** · `session_events_checked` **4364** · `superseded_closes_checked` **2** · `defects_found` **7**(전부 `self`) · **`new_defects_not_in_ledger` 0 / `ledger_rows_missing` 0** — 표 전체 결함 집합 == 동결 장부 7건. **(c) 구간 결함 0**(ship 2 / session 4). **(d) 양방향**: 전수 7건 검출(exit 0, 장부와 등식) / **블록5 구간 0건**(ship 26 · session 28, exit 0) / **알려진 위반 구간 427105~430030 에서 exit 1**(tick 430030 seq 0 검출). `causation-selftest` 합성 **17행**에서 정상 7건 통과 + 결함 10종(`superseded_cause_null`·`self`×2·`wrong_type`×2·`actor_mismatch`·`same_session`·`not_same_tick_earlier`·`closed_cause_not_null`·`opened_cause_not_null`)이 각자 정해진 종류로 검출 | **PASS** |
| **SC-82** | `check_contract_coverage.py --strict` | **types 13 / errors 0 / warnings 0**, `RESULT: PASS`, exit 0 | **PASS** |
| **SC-83** | `interface_matrix.py` | **신규 7타입 대조 행 201 / 불일치 0**. 전체 249행(envelope+payload 70 · `ShipState` 원소 23 · C# 열 없는 데이터 타입 108), 문제 있는 행 **0**, `integer_rows` 51(Rust 무손실 51 / C# 무손실 37), `narrowed_rows` 4/4 ok. exit 0 | **PASS** |
| **SC-84** | `fixture_world_scan.py` | **스캔 27** / world_id 11 / tick_hz 2 / 쌍 2 / **위반 0** / 한 fixture 안 값 충돌 0. exit 0 | **PASS** |
| **SC-86** | 자체 검사 **11건** + 봇 `cargo test` | **11/11 기대 종료 코드와 일치**(`evidence/R4-B9/SC-86-selftests.log`) — `ship_events selftest`(correlation 키로 되돌리면 재개 시나리오에서 깨짐: corr-A spawn 1/despawn 0, corr-B spawn 0/despawn 1) · `causation-selftest` · `overlap` **알려진 위반 exit 1 / 블록5 exit 0** · `ledger` **전수 exit 0 / 블록5 exit 0 / 알려진 위반 exit 1** · `log_pipe_backpressure selftest` · `stats_delta` **옛 증거 exit 1(RED) / 새 증거 exit 0** · `check_occurred_at --selftest`(8 케이스, 불일치 0). 봇 `cargo test` **70 passed / 0 failed** | **PASS** — 새 도구마다 **양방향**이 확인된다 |

**§7a — 초록불이 무엇을 보고 켜졌는가 (블록 9 자체 점검)**

| 항등식·검사 | 짝지은 "그 경로가 실제로 탔다" |
|---|---|
| SC-76 짝 등식 | `ships_checked` = 1 > 0, **spawn 과 despawn 이 둘 다 실제로 발생**(tick 448809 / 467202) |
| SC-77 세션 쌍 | `correlations_checked` = 2 > 0, 그 중 하나가 **`PROTOCOL_VIOLATION`** 이라는 비정상 사유 — 자명한 정상 경로가 아니다 |
| SC-78 재개 | **sessions 2 / spawns 1** — 재개 조건이 실제로 발생했다(강제 종료 → 재접속) |
| SC-79 빈틈 | `distinct_world_tick_groups` = 1735 > 0, 전수 4533행 |
| SC-80 타입 | 구간에 4종이 **전부 나타났다**(각 1~2건) — 타입이 0종이면 자명한 통과가 되는 검사다 |
| SC-81 등식 | **결함 7건을 실제로 검출**하고 장부와 등식을 이룬다. 0 == 0 이 아니다. 게다가 알려진 위반 구간에서 **빨간불이 켜지는 것**을 같은 도구로 보였다 |
| SC-83 대조 | 대조 행 201(신규 7타입) — 0행이면 불일치 0 이 무의미하다 |
| SC-86 | **전부 "기대 종료 코드"와 대조**했다. RED 를 기대한 3건(`overlap` 알려진 위반 · `ledger` 알려진 위반 · `stats_delta` 옛 증거)이 **실제로 exit 1 을 냈다** |

**⚠ 블록 9 구간 표본이 작다는 것을 명시한다.** 이번 인스턴스 구간은 **함선 1척 · 도메인 이벤트 6행**뿐이다(사람이 조종한 단일 세션). SC-76·78·80·81(c) 의 초록은 **그 작은 구간에 대한 것**이고, 강한 근거는 **SC-81 (b) 의 전수 등식(168 + 4364 행)** 과 **SC-79 의 전수(4533행)** 쪽이다. 블록 6~8 이 돌면 구간 표본이 커진다 — **그때 SC-76·78·80 을 다시 돌릴 것.**

**계약 외 기록 — 짝 없는 함선은 3척이 아니라 4척이다.** 전수 `pairs` 는 exit 1 이고 짝 없는 `ship_id` 가 **4건**이다(`01a0bf0f-9150-…f015e` spawn tick 350994 / `01a0bf15-046e-…e123` **358137** / `01a0bf20-16f7-…c504` 361490 / `01a0bf20-2791-…e801` 361575). r3 §6 과 r4 §3.1 은 3척으로 적었고 **`01a0bf15-046e-…e123` 이 빠져 있었다.** 대응하는 짝 없는 세션 correlation 도 정확히 4건이고 tick 이 같다. **새로 생긴 행이 아니다** — 넷 다 tick ≤ 361575 로 라운드 1~2 구간이고, 어제 4,527행 → 오늘 4,533행의 증가분 6행은 전부 SC-59 세션이다. **앞선 리포트의 열거가 불완전했던 것이며, 기록은 그대로 둔다(원칙 5).** r4 §3.1 의 "짝 없는 함선 3척"은 **4척**으로 읽는다.

---

### 5.4 코드가 바뀌어 다시 돌린 것 (§1.4 재판정 포함)

**server/ 소스가 qa2 측정 시점과 다르다.** 트리 해시 **`45970d728885630f`(qa2 §4.1) → `477c9c54ba9ec5ce`(qa3 실측, `make_red_logsink_binary.py --hash-only`, 45 파일)**. 트리는 커밋 `535ea27` 에서 깨끗하고 `535ea27` 은 `CLAUDE.md` 만 건드렸으므로, 변경은 `8b0a78d` 에 들어 있다. 따라서 §3.1 규칙("바뀌었으면 SC-01 부터 다시")에 따라 재실행했다.

**측정 바이너리를 다시 만들었다**: `cargo build -p starfall-game-server --locked` → sha256[:16] **`d5e47ed7296b87bc` → `e84cd098c06fce26`**. **디스크의 바이너리가 소스와 어긋나 있었다는 뜻이고, 어제 SC-59 세션은 옛 바이너리(`d5e47ed7…`)로 돌았다.** 블록 6~8 을 다시 돌릴 때는 **`e84cd098c06fce26`** 이다.

| 항목 | 명령 | 결과 | 판정 |
|---|---|---|---|
| **SC-01** | `cargo fmt --all --check` / `cargo clippy --workspace --all-targets -- -D warnings` / `cargo test --workspace --locked` **3회** / `cargo test -p starfall-sim --release --locked` / `cargo clippy --release -p starfall-sim --all-targets -- -D warnings` | fmt exit 0 · clippy exit 0 · **167 passed / 0 failed / 4 ignored ×3**(간헐 실패 없음) · 릴리스 **50 passed(49+1) / 0 failed / 1 ignored**, `simulation::tests::r4_s3_release_arm_signals_without_fabricating_an_event ... ok` **1회** · 릴리스 clippy exit 0 | **PASS** — qa2 §4.1 과 수치가 같다. **트리 변경이 게이트 결과를 바꾸지 않았다** |
| **SC-85** | `cd tools/bots && cargo test` | **70 passed / 0 failed** | **PASS** |
| **SC-87** | `cargo test -p starfall-sim --test determinism --locked` → `git diff --exit-code -- server/crates/sim/tests/data/replay/` | 1 passed(1 ignored), exit 0 · **diff exit 0** · 골든 sha256[:16] `824a6489…` / `c924e59f…` / `3e00e527…` (§2·§4.1과 동일) | **PASS** |
| **SC-82·83·84·86** | 위 §5.3 | 전부 재실행 — 수치가 qa2 §2 와 같다(커버리지 13/0/0, 경계면 201/0, 스캔 27, 자체검사 11/11) | **PASS** |

---

### 5.5 계약 자체 문제 (architect 통지 — FAIL 로 처리하지 않는다)

1. **SC-59 (b) "기준 마커 4개"** — 실제 화면에서 한 시야에 4개가 들어오는지 아무도 확인한 적이 없다. 마커가 4.2 km 안에 배치되고 경계가 12 km 이므로 **한 프레임에 4개가 동시에 보이지 않는 것이 정상일 수 있다.** 계약 문구를 "4개가 배치돼 있고 **최소 1개가 항상 보인다**"로 고칠지 architect 가 정해 달라. 지금 문구로는 닫을 방법이 없다.
2. **§0.3 에 "관찰은 했으나 증거 요건 미충족" 칸이 없다.** PASS / FAIL / 미검증(환경) / 대기 / 기록 다섯 중 어디에도 정확히 맞지 않아 §5.1 을 **FAIL** 로 적었다. 다음 슬라이스 계약에 이 칸을 만들 것을 권한다 — 없으면 "관찰은 다 맞았는데 FAIL"이 반복해서 오해를 만든다.
3. **SC-89 신설 제안** — §5.2 (가). architect 승인 전이므로 계약 표에 넣지 않았다.
4. **HUD 지표의 신뢰성** — `reconcile_*` 카운터가 재접속에서 초기화되는데 `reconcile_error_deg` 의 max 만 그렇지 않아 보인다(§5.2 (나) 3). **SC-56 판정 전에 지표 자체를 확인해야 한다.** 지금 상태로는 SC-56 의 초록이든 빨강이든 무엇을 잰 것인지 말할 수 없다(§7a).
5. **도구 라벨 오기(사소)** — `check_sequence_gaps.py` 출력의 `"item"` 이 `"SC-59 (AC-16c) sequence 빈틈 검사"` 다. 이 슬라이스에서 sequence 빈틈은 **SC-79** 이고 SC-59 는 육안 항목이다(p0-02 번호가 남은 것). qa 소유 파일이지만 이번 라운드 증거의 문구가 바뀌면 혼란스러우므로 **다음 라운드에 고친다.**

---

### 5.6 라운드 4 집계 (여기까지)

**이번 절에서 새로 판정한 것**: PASS **13** (SC-01·76·77·78·79·80·81·82·83·84·85·86·87) / **FAIL 1** (SC-59).

**남은 판정**:
- **블록 6**(Unity 실행 + 사람): SC-53·54 는 qa2 §4 에서 EditMode 로 PASS. 남은 것은 **SC-56·57·58·60**(자동·실서버) + **SC-59 재촬영** + **SC-88 (d)**.
- **블록 7**(2세션 가시성): SC-61~65.
- **블록 8**(부하·성능): SC-25·26·27·69~75.
- **블록 9 재확인**: 블록 6~8 이 큰 구간을 만들면 **SC-76·78·80 을 그 구간으로 다시** 돌린다(§5.3 마지막 주의).
- **새 항목**: SC-89(제안, architect 승인 대기).

**사람이 해야 하는 일 (에이전트가 대신할 수 없다)**:
1. **SC-59 재촬영 5~10분** — §5.1 (4) 의 4단계. 이것 없이는 §5.0 의 문장이 계속 유효하다.
2. 블록 6 의 Unity Play 시작·정지, 블록 7 의 2세션 하네스 조작, 블록 8 의 31번째 연결(G-d).

---

### 5.7 (가)의 원인 확정 — client 조사 결과의 qa 교차 확인, SC-89 문구 확정

§5.2 (가)에서 "인과는 client 의 송신 경로 조사 결과로 확정한다"고 적었다. client 가 답을 보냈고, **qa3 가 코드를 직접 읽어 교차 확인했다**(보고를 그대로 받지 않는다).

#### (1) 송신 루프에 상한 없는 catch-up 이 있다 — **qa3 확인**

`client/Assets/_Project/Scripts/Greybox/GreyboxSession.cs:418~431` (qa3 가 직접 읽음):

```csharp
void Update()
{
    if (_input != null) _input.Sample();

    _tickAccumulator += Time.unscaledDeltaTime;
    while (_tickAccumulator >= _tickDurationSeconds)
    {
        _tickAccumulator -= _tickDurationSeconds;
        SendAndPredictOneTick();          // 반복마다 즉시 TrySendJson
    }
    ...
}
```

**확인 사실**: 고정 타임스텝 누적기이고 **한 `Update()` 가 드레인하는 tick 수에 상한이 없다.** 프레임이 밀린 만큼 한 프레임 안에서 `SendAndPredictOneTick()` 이 연달아 호출되고, 각 호출이 **사이 간격 없이** 송신한다. 0.46 초(9.2 tick) 히치 하나면 **한 프레임에 9건**이 나간다 — 서버의 tick 당 상한 **8** 을 넘긴다.

**이것이 §5.2 (가)의 관측과 직접 대응한다.** 백그라운드 Editor 는 프레임이 길게 밀리므로(§5.2 (가)의 전경 창 표) catch-up 반복이 커진다. `client_send_hz = 20`·`tick_hz = 20` 이라 정상이면 tick 당 1건이어야 한다는 서술과, 실제로는 버스트가 났다는 관측이 **한 가지 메커니즘으로 설명된다.** 리더의 추정("둘이 같은 상류 원인을 공유한다")은 **지지된다.**

#### (2) 버려진 명령이 예측 히스토리에 남는 경로 — **qa3 확인**

같은 파일 `:433~452`:

```csharp
string json = ContractJson.Serialize(command);
if (!_client.TrySendJson(json)) return;          // ← 클라이언트 큐가 거부하면 예측도 안 한다
_pendingInputSeqByCommandId[commandId] = seq;
ShipControlInputD dequantized = SetShipControlBuilder.ToDequantizedInput(command.Payload);
_controller.ApplyInput(dequantized);             // ← 서버 수락 여부와 무관하게 로컬 예측에 반영
```

**어긋남 경로가 둘이고 방향이 반대다** — 이 구분은 client 보고에 없다:

| 경로 | 조건 | 결과 |
|---|---|---|
| **A. 서버가 버린다** | `TrySendJson` 성공 → 서버 tick 상한 8 초과로 폐기(ADR-0011 §5.2, `COMMAND_RESULT` 없음) | 클라이언트는 **예측에 반영했는데 서버는 반영 안 함** → `DropRejected` 가 불리지 않아 히스토리에 남음 → `Reconcile()` 에서 어긋남 → **hard snap** |
| **B. 클라이언트가 거부한다** | `outbound queue full (256)` → `TrySendJson` false → **`return`** | 서버도 클라이언트도 그 입력을 **둘 다 반영하지 않는다** → 예측은 안 어긋나지만 **플레이어 입력이 조용히 사라진다**(조작이 먹지 않는 느낌) |

**§5.2 (가)에서 상태 바로 관측한 `outbound queue full (256)` 은 경로 B 다.** 즉 그 순간에는 예측 어긋남이 아니라 **입력 유실**이 일어나고 있었다. 경로 A(hard snap)와 경로 B(입력 유실)는 **같은 상류 원인(catch-up 버스트)에서 갈라지는 두 증상**이다. **SC-89 (d) 를 "`outbound queue full` 을 찍지 않는다"로 둔 이유가 이것으로 분명해졌다** — 그것은 단순 경고가 아니라 **플레이어 입력이 버려지고 있다는 신호**다.

#### (3) `PROTOCOL_VIOLATION` 전용 로그는 없다 — **qa3 확인**

`server/crates/gateway/src/runtime.rs:187~196` (qa3 가 직접 읽음):

```rust
pub const fn close_code(reason: SessionCloseReason) -> Option<u16> {
    match reason {
        SessionCloseReason::ClientClosed => Some(1000),
        SessionCloseReason::IdleTimeout | SessionCloseReason::ServerShutdown => Some(1001),
        SessionCloseReason::ProtocolViolation => Some(1002),
        SessionCloseReason::SlowConsumer => Some(1011),
        SessionCloseReason::TransportError => None,
        SessionCloseReason::Superseded => Some(4001),
    }
}
```

**`ProtocolViolation` → close code 1002 이고, 이것은 `ReconnectPolicy` 의 재연결 대상이다.** 그래서 클라이언트는 `"PROTOCOL_VIOLATION"` 이라는 문자열을 **한 번도 남기지 않고** 일반 재연결 경로로 들어간다 — 어제 세션에서 사용자에게 "갑자기 끊겼다"로만 보인 이유다. **DB 의 `close_reason` 과 클라이언트 로그가 서로를 가리키지 못한다.**

**교차 확인으로 뒷받침된다**: DB 에서 tick 457993 `PROTOCOL_VIOLATION` → 16.4 초 뒤 tick 458321 재접속(§5.3 SC-78). **1002 가 재연결 대상이라는 코드와 DB 의 재접속 관측이 일치한다.** 반대로 SC-88 의 4001 은 재연결하지 않는다 — 두 경로가 설계대로 갈라진다.

#### (4) client 의 RED 테스트 — **qa3 가 결과 파일로 확인**

`test-results.xml`(`<test-run testcasecount="167" total="167" passed="164" failed="1" skipped="2">`, 2026-09-23 13:05:06Z):
실패 1건 = **`Starfall.Tests.EditMode.GreyboxSendBurstTests.Update_AfterA460msHitch_MustNotBurstMoreThanOneTickOfSendsPerFrame`** — **의도된 RED 이고, 이름이 겨냥한 조건(460 ms 히치 → 한 프레임에 1 tick 초과 송신)과 일치한다.** §7a 가 요구하는 "새 관측은 RED 를 먼저 한 번 보인다"를 충족한다.

**⚠ SC-46 의 수가 바뀐다**: qa2 §4 에서 EditMode 는 **159 tests / 157 passed / 2 skipped** 였다. 지금은 **167 / 164 / 1 failed / 2 skipped** 다. client 가 8건을 더했다. **SC-46 은 블록 6 에서 다시 돌려야 하고, 그때 `GreyboxSendBurstTests` 가 GREEN 인지(=수정됐는지) 함께 본다.**

#### (5) SC-89 (d) 문구 확정 + 미구현 항목 2건

§5.2 (가)의 SC-89 제안에서 **(d) 를 다음으로 확정한다**(위 (2)의 경로 B 근거):

> **(d)** 그 구간에 클라이언트가 **`outbound queue full` 을 찍지 않는다.** 이 경고는 단순 역압 신호가 아니라 **`TrySendJson` 이 false 를 반환해 그 tick 의 플레이어 입력이 서버에도 로컬 예측에도 반영되지 않았다**는 뜻이다(`GreyboxSession.cs:446`). 찍히면 그 자체로 FAIL 이다.

**추가로 두 가지를 SC-89 의 선행 구현으로 요청한다**(client 가 "신호 주면 붙이겠다"고 했고, qa 는 구현을 지시하지 않으므로 **리더·architect 결정 사항**):

| 필요한 것 | 왜 필요한가 | 없으면 |
|---|---|---|
| **송신 건수 계측** ("직전 1초 송신 건수 최대치" 또는 "한 `Update()` 최대 드레인 tick 수")를 HUD·로그에 | SC-89 (b)(c) 를 **판정 가능한 관찰**로 만드는 유일한 수단. 지금은 버스트가 났는지를 **서버가 끊어 준 뒤에만** 알 수 있다 | SC-89 는 "끊겼는가/아닌가"만 보는 검사가 되고, **안 끊겼을 때 그것이 고쳐져서인지 운이 좋아서인지 구분하지 못한다**(§7a) |
| **close code → 사유 매핑 + 전용 grep 태그**를 **1002** 에도 (SC-88(d) 가 4001 에 준 것과 같은 형태) | DB 의 `close_reason` 과 클라이언트 로그를 짝지을 수 있어야 한다 | 위 (3) — 플레이어에게도 QA 에게도 "갑자기 끊겼다"로만 보인다 |

**⚠ 임계는 그대로다.** client 가 상한 8 / 예산 8·10초를 건드리지 않았음을 확인했다(계약 §7 준수). **고칠 자리는 서버의 임계가 아니라 클라이언트의 catch-by 상한**이다 — 한 `Update()` 가 드레인할 tick 수에 상한(예: 1~2)을 두고 나머지 누적을 버리는 것이 표준 처리다. **그 값이 얼마여야 하는지는 qa 가 정하지 않는다**(architect·client).

#### (6) 이 절이 §5.2 의 판정을 바꾸는가

**(가) — 원인이 확정됐다.** §5.2 에서 "정황"이라 적은 것을 **코드 경로로 확인**했고 RED 테스트가 있다. SC-89 는 그대로 유효하며, **(c) 양성 대조는 여전히 필요하다** — RED 는 클라이언트 단위 테스트이고, **실서버에서 위반이 실제로 나는지는 별개 관측**이다.

**(나) — 판정은 바뀌지 않는다.** 여전히 SC-56/M-9 로 판정한다. 다만 경로 A 가 hard snap 의 메커니즘으로 확인됐으므로, **SC-56 을 다시 잴 때는 catch-up 수정 뒤여야 한다** — 수정 전 수치는 "재조정이 나쁘다"가 아니라 "송신이 버스트였다"를 재는 것이다. **§5.2 (나) 3의 `reconcile_error_deg` max 초기화 문제는 이 절로 해소되지 않았다.** 그대로 열린 항목이다.

---

### 5.8 SC-89 를 계약에 넣었다 (9차 개정) — architect R4 판정 반영

architect 가 `01_architect_decisions.md` `## R4 판정 — 히치 catch-up 버스트` §10 에서 새 SC 를 권했고, **판정은 qa 몫**이라고 명시했다. 계약 `02_sprint_contract.md` 를 **9차 개정**했다(§1 C절 SC-88 다음 행, 머리의 항목 수, §9 이력).

#### (1) 어디에 넣었는가 — AC-18/SC-26 계열에 붙이지 않았다

architect 의 권고와 §5.2 (가)에서 내가 낸 판단이 일치한다. **재는 것이 다르다.** SC-26/AC-18 은 **31 연결 부하**에서의 손실·상한이고, SC-89 가 재는 것은 **"클라이언트의 송신 속도가 프레임률이 아니라 벽시계 tick 속도를 따른다"** 이다. 단일 클라이언트·부하 없음에서 난 사건이다.

**계약에 그 집이 없었다는 것이 라운드 1~4 의 모든 PASS 와 이 버그가 양립한 이유다**(§7a). 새 항목을 만든 것은 그 공백을 메우는 일이다.

#### (2) 확정된 SC-89 — 절 넷

§5.2 (가)에서 제안한 (a)~(d) 를 architect 의 형태(절 둘)에 맞춰 재구성했다. **제안 당시의 (a)(b) 는 architect 의 (a) 안으로 들어갔고, (c) 양성 대조는 성질 테스트 + RED 선행으로 대체됐다**(architect 의 T-5 가 한 점이 아니라 **불변식**을 고정하므로 더 강하다).

| 절 | 단언 | 닫는 증거 |
|---|---|---|
| **(a) 송신** | 프레임 히치의 **길이와 무관하게** 한 프레임에 `SET_SHIP_CONTROL` 2건 이상 나가지 않는다 | T-5 성질 테스트(무작위 델타, ≥450 ms 히치) ① 프레임당 송신 ≤ 1 ② 벽시계 1초당 예측 tick 20 ± 1 ③ 이월 + 송신 == 예측. **＋ 실서버** `RATE_LIMITED` 델타 0 · `PROTOCOL_VIOLATION` 0건 · **`outbound queue full` 안 찍힘** |
| **(b) 재조정** | 합성 히치 뒤 **`catchup_carry_forward_ticks_total > 0` 이면서 `reconcile_hard_snap_total == 0`** | T-6. **두 단언은 반드시 짝** — 앞이 없으면 "히치가 안 일어났다"를 통과로 읽는다 |
| **(c) 도구 결합** | 판정 도구가 **사본이 아니라 진짜 코드**를 잰다 | `grep -n 'while (_tickAccumulator' GreyboxSession.cs` **exit 1** + 테스트가 `TickCatchUp.Plan` 을 직접 호출 |
| **(d) 잔여(기록)** | `reconcile_forced_after_hitch_total` · `catchup_truncated_total` 이 **실플레이에서 0** | 0이 아니면 **tick 정렬 예측(미룬 결정, ADR-0012 §6.3)의 발동 조건 충족** → architect 통지 |

**항목에 못 박은 두 문장**:
- **⚠ 봇으로는 (a) 를 닫을 수 없다** — 봇은 송신 속도를 통제해 보내므로 이 경로를 **원리적으로** 밟지 않는다(§0.11 과 같은 형태). 닫는 것은 순수 함수 성질 테스트 + 실플레이 관측뿐이다.
- **RED 선행**(§7a) — 0.46 s 한 점의 RED 는 이미 확인됐다(§5.7 (4)). 성질 형태로 넓힌 뒤 GREEN 을 보인다.

#### (3) (c) 는 architect 가 qa 판단에 맡긴 것이고, **넣기로 했다**

architect §8 T-3: *"네 게이트에도 이 확인을 넣을지는 네 판단이다."* **넣었다.** 근거는 §7a 의 표 그대로다 — 이 슬라이스는 **"초록불이 진짜 코드가 아닌 것을 보고 켜졌다"는 병에 네 번 걸렸다**(`world_full` 미실행 · `SET_SHIP_CONTROL` 게이트웨이 전량 거부 · 봇 짝 게이트 `0 == 0` · 벽시계 flaky). 현재 `GreyboxSendBurstTests.cs` 는 `GreyboxSession` 의 루프를 **복제해서** 검사하므로 **다섯 번째 사례가 될 자리**다. **grep 한 줄로 살 수 있는 보험이라 비용이 근거를 넘지 않는다.**

#### (4) 이번 라운드 판정에 미치는 영향 — **없다**

| 확인한 것 | 결과 |
|---|---|
| `contracts/` 변경 | **없음**(architect §6). 따라서 **SC-35~50 · SC-82 · SC-83 · SC-84 의 기대 숫자가 그대로다** — 스키마 18 / 유효 fixture 27 / 반례 34 / 레지스트리 13. **§5.3 의 블록 9 PASS 는 재판정이 필요 없다** |
| 서버 임계값 | **하나도 바뀌지 않는다**(상한 8, 예산 8/10 s, `rate_limit_hz` 40, `protocol_violation_hz` 100). 계약 §7 의 "임계를 늘려서 닫지 마라"가 지켜졌다. 고친 자리는 **클라이언트의 송신·예측 분리**다 |
| `server/` 변경 | **없음**(architect §12). §5.4 의 SC-01·85·87 PASS 와 바이너리 `e84cd098c06fce26` 가 유효하다 |
| SC-89 자체 | **대기** — H-1~H-8 미구현. 이 라운드 판정에서 제외한다(§0.3 의 "대기") |
| **SC-46·47·49** | **재실행 필요** — EditMode 가 159/157 → 167/164 로 바뀌었고(§5.7 (4)), H-7 재작성으로 **또 바뀐다.** 블록 6 에서 돌린다 |
| **SC-56** | **catch-up 수정 뒤에 잰다**(§5.7 (6)). 수정 전 수치는 "재조정이 나쁘다"가 아니라 "송신이 버스트였다"를 잰 것이다 |

**§5.6 의 집계는 그대로다: PASS 13 / FAIL 1.** SC-89 는 대기라 집계에 넣지 않는다.

#### (5) 아직 열려 있는 것 — architect 판정이 닫지 않은 항목

1. **`reconcile_error_deg` 의 max 가 재접속에서 초기화되지 않는 문제**(§5.2 (나) 3). architect R4 판정은 이것을 다루지 않았다. **SC-56 을 판정하려면 지표가 무엇을 재는지 먼저 확정돼야 한다**(§7a). §5.5 4번 그대로 열려 있다.
2. ~~**1002 전용 grep 태그**~~ → **닫혔다(리더 승인, 2026-09-23).** architect R4 판정은 close code 매핑을 다루지 않았으나 **리더가 별도로 승인했고 `H-8` 에 합쳐 진행한다.** 조건: **재연결 동작은 바꾸지 않는다** — 1002 는 **계속 재연결 대상**이어야 하고(4001 과 달라야 한다) **로그만 추가한다.**
   **qa 가 재판정에서 확인할 것**(이 조건이 지켜졌는지는 §7a 상 별도 관측이 필요하다):
   - (i) `runtime.rs` 의 `close_code(ProtocolViolation) == 1002` 가 **그대로**다(서버는 이 건으로 한 줄도 바꾸지 않는다 — architect §12).
   - (ii) **`ReconnectPolicy` 의 양성 대조가 유지된다** — SC-88(d) 가 이미 쓰는 대조군(`RealtimeClientTests` 12 + `TransportHandshakeTests`, qa2 §4 에서 13건 PASS)에서 **1002 는 재접속을 스케줄하고 4001 만 안 한다.** 로그 한 줄을 더하면서 이 갈림이 깨지면 SC-88(d) 가 함께 무너진다 — **로그 추가가 동작을 바꾸지 않았음을 이 13건으로 확인한다.**
   - (iii) 새 태그가 **실제로 찍힌다**: 문자열이 존재하는 것(grep)만으로는 부족하다. **SC-89 (a) 의 양성 대조**(한 tick 에 9건 이상 일부러 보내 위반을 만드는 경로)에서 **그 로그 줄이 실제로 나오고, 같은 사건이 DB 의 `close_reason='PROTOCOL_VIOLATION'` 과 짝지어진다**는 것을 본다. 짝짓기가 이 태그를 붙이는 이유이므로, 짝을 보이지 못하면 붙인 값이 없다.
3. **SC-59 (b) "기준 마커 4개"** 문구(§5.5 1번). 미해결.
4. **§0.3 에 "관찰은 했으나 증거 요건 미충족" 칸이 없는 것**(§5.5 2번). 다음 슬라이스 계약 입력.

---

### 5.9 정정과 재판정 — architect R4 보충 판정 반영 (qa3, 2026-09-23)

architect 가 §5.5 의 5건을 전부 판정했다(`01_architect_decisions.md` `## R4 보충 판정` §F~§I). **앞 절을 덮어쓰지 않고 정정 항목을 새로 둔다**(원칙 5의 정신: 정정은 새 레코드로). 계약은 **9차 개정**으로 반영했다.

#### (1) ⚠ 내 추정 하나가 오진이었다 — `reconcile_error_deg` "클램프·포화"

§5.2 (나) 3 에서 나는 *"`_deg` 의 max 는 초기화되지 않거나, 어떤 값에서 포화·클램프되고 있을 가능성"* 이라고 적었다. **틀렸다.** client 가 코드로 확인했고(R8) architect 가 판정했다:

- `ReconcileErrorStats.Compute` 는 **문제없다.** 계산 버그가 아니다.
- 진짜 원인은 **범위 불일치**다: `OnSessionReady` 가 **오차 리스트·캐시 퍼센타일·CL-2 카운터를 리셋하지 않는** 반면, `reconcile_hard_snap_total` 은 **`_controller` 재생성으로 우연히 리셋된다.**
- 따라서 `reconcile_error_deg max` 가 재접속 전후로 같았던 것은 **그보다 큰 표본이 그 뒤에 들어오지 않았을 뿐**이다. 포화도 클램프도 아니다.

**내가 관측한 사실(값이 같다 · hard snap 이 5 → 3 으로 줄었다)은 그대로 유효하고, 거기서 끌어낸 추정이 틀렸다.** §5.2 (나) 1 의 결론("카운터가 재접속에서 초기화된다")도 **절반만 맞았다** — 초기화되는 것은 `reconcile_hard_snap_total`·`ack_input_seq` 쪽이고, 오차 리스트·퍼센타일은 **초기화되지 않는다.** 그래서 한 HUD 안에 **범위가 다른 두 종류의 수가 섞여 있었다.**

**architect 판정**: `…_session` / `…_run` **두 범위를 이름으로 구분해 둘 다 표시**한다(한쪽으로 통일하지 않는다). **SC-56 은 세션 범위로 읽고, 실행 전체 값은 맥락으로 함께 적는다.**

**§5.2 (나)의 수치 해석을 그에 맞게 고쳐 읽는다**: "세션 전체 hard snap 은 5 + 3 = 최소 8건"은 **유효하다**(그 카운터는 실제로 리셋된다). 그러나 "위치 오차 최댓값이 최소 323.1 m"의 **`n = 2627` 과 `max = 323.1212` 는 세션 값이 아니라 실행 전체 값**이므로, **SC-56 이 읽을 세션 범위 값은 아직 아무도 본 적이 없다.** 재측정이 필요하다.

**그리고 architect 가 내 판단에 동의했다**: **SC-56 은 catch-up 수정 뒤에 다시 잰다.** 지금 수치(323 m, hard snap 8)는 "재조정이 나쁘다"가 아니라 **"송신이 버스트였다"를 잰 것**이다.

#### (2) SC-59 재판정 — **FAIL → 미검증(증거 요건)**

§0.3 에 다섯 번째 칸이 **승인·신설**됐다(계약 9차). §5.1 에서 FAIL 로 적은 이유가 바로 "맞는 칸이 없다"였으므로, **그 칸으로 옮긴다.**

| | |
|---|---|
| **판정** | **미검증(증거 요건)** — §5.1 의 근거는 **하나도 바뀌지 않는다.** 바뀌는 것은 **귀속**뿐이다: 제품의 결함이 아니라 **증거의 결함**이다 |
| **게이트 강도** | **바뀌지 않는다.** architect 조건 ①: **비-통과이며 슬라이스 종료를 FAIL 과 똑같이 막는다.** SC-59 는 여전히 닫히지 않았고 **§5.0 의 문장은 그대로 유효하다** |
| **금지** | architect 조건 ②: **실패를 이 칸으로 옮기는 것은 금지한다.** R3 판정 1의 "간헐 실패도 FAIL"은 **테스트가 실패한** 경우이고, 이 칸은 **테스트가 유효하게 실행되지 않은** 경우다. 섞으면 그 예외가 무너진다 — 계약 §0.3 에 이 문장을 함께 적었다 |

#### (3) SC-59 (b) 가 (b1)/(b2) 로 쪼개졌다 — 내 제안보다 나은 형태다

§5.5 1번에서 나는 *"최소 1개가 항상 보인다로 고칠지"* 를 물었다. **architect 는 낮추지 않고 쪼갰다**, 그리고 그 판단이 옳다:

> **"1개면 충분"으로 낮추면 마커가 3개 사라져도 통과한다.**

- **(b1) 배치** — 마커 4개가 존재하고 정해진 좌표에 있다 → **EditMode 씬·데이터 단언. 영상 불필요.**
- **(b2) 가시성** — 촬영 구간 전체에 **최소 1개가 화면에** → 영상으로 닫는다. **부호·방향 판정에 실제로 필요한 성질은 이쪽이다.**

한 항목이 **두 사실**을 재고 있었던 것이 닫을 수 없던 원인이다. 라운드 4 에서 관측된 "마커 1개"는 **(b2) 의 부분 충족**이고 (b1) 과는 무관했다.

#### (4) HUD 가 없던 원인이 확정됐다 — 내 §5.1 (2) 추정의 확인

§5.1 (2) 에서 *"왜 없었는지는 client 가 설명해야 한다"* 로 남긴 것의 답: **`OnGUI` 는 Game View 가 비포커스면 아예 호출되지 않는다**(client R8). 그래서 어제 t ≈ 213 s 에 **HUD 가 한 줄도 없었다.** §5.1 (2) 정정 문단의 읽기("초반에 없다가 나중에 나타났다 — 포커스를 잡기 전이 그 구간")가 **맞았다.**

**⚠ 그 결과 절차 제약이 생겼다 — SC-59 와 SC-89 는 같은 세션으로 촬영할 수 없다.**

| | SC-59 | SC-89 |
|---|---|---|
| 요구 | Game View **활성 탭 + 포커스 유지**(아니면 HUD 가 안 그려진다) | **백그라운드 구간**(그래야 히치가 난다) |
| 증거 | 영상 | **화면이 아니라 `grep` 가능한 카운터 로그 + DB** |

**묶으면 둘 다 닫지 못한다.** 계약 양쪽 항목에 이 제약을 적었다. SC-89 의 증거가 로그·DB 여야 하는 이유도 여기서 나온다 — **`OnGUI` 만으로는 백그라운드 구간의 증거가 원리적으로 남지 않는다**(client H-13 이 그래서 필요하다).

#### (5) SC-89 가 승인됐고, 내 문구 두 곳이 약했다

| architect 지적 | 내 원래 문구의 문제 | 고친 것 |
|---|---|---|
| **(a) 가 문턱 바로 아래를 통과로 읽는다** | `close_reason='PROTOCOL_VIOLATION'` 0건만 봤다. **끊김은 예산 8이 찬 뒤에야 일어나므로 위반이 7회 쌓여도 내 (a) 는 초록이었다** | `commands_dropped_over_tick_cap_total` 과 `RATE_LIMITED` **델타 0** 을 추가 |
| **(b) 의 "조건이 만들어졌다" 단언이 약하다** | `commands_received_total > 0` 은 **"세션이 살아 있었다"만 잰다** | **한 `Update()` 최대 드레인 tick 수 > 1** 을 단언에 추가. **드레인 max 가 1이면 히치가 한 번도 없었다는 뜻**이고 그 세션은 항목을 닫지 못한다 |

**이 둘은 내가 §7a 를 스스로에게 덜 적용한 자리다.** 나는 §5.3 에서 남의 초록불에 "그 조건이 실제로 일어났는가"를 요구해 놓고, **내가 쓴 SC-89 의 (a) 는 그 규율을 통과하지 못하는 형태였다.** 기록해 둔다.

**(d) 도 판정이 바뀌었다 — 그리고 내 인과 추정이 틀렸다.** 나는 §5.7 (2) 에서 `outbound queue full` 을 (a) 의 판정 대상으로 뒀다. **client 실측: 위반 세션의 9건 burst 는 전부 송신에 성공했다.** 즉 **큐 포화는 이번 위반의 원인이 아니다** — 위반 **이후** 구간의 **별개 잠재 결함**이다. 그래서 `outbound_queue_full_total` 은 **기록(d)** 으로 내리고, **0이 아니어도 SC-89 의 FAIL 이 아니라 "새 발견"** 으로 적는다. 경로 B(입력 유실) 자체는 여전히 실재하는 결함이므로 관측은 유지한다.

**(e) 신설**: 서버 임계(상한 8 · 예산 8/10 s · `rate_limit_hz` · `protocol_violation_hz`)의 **diff 가 0임을 기계로 확인**한다(§7). 사람의 약속이 아니라 검사로 둔다.

**재현 절차도 바뀌었다.** "450 ms 히치 한 번"으로는 **위반 1회**뿐인데 예산은 **10초에 8회**다 — 내 SC-89 초안의 "전경 5분 + 백그라운드 5분"은 재현을 보장하지 못했다. architect 의 산수: **백그라운드에서 Editor 가 2~4 Hz 로 조이면** 2 Hz = 프레임당 10 tick = **매 프레임 위반 1회** → **4초면 예산이 찬다.** 그래서 **자유 비행보다 백그라운드 전환이 훨씬 잘 재현한다.**

#### (6) 도구 라벨 오기 — 인용 자리에 정정을 붙인다

§5.3 의 **SC-79** 행은 `check_sequence_gaps.py` 의 출력을 증거로 인용했는데, **그 출력의 `"item"` 필드는 `"SC-59 (AC-16c) sequence 빈틈 검사"` 라고 찍힌다**(p0-02 의 번호가 남은 것). architect 지시대로 **인용 자리에 정정을 붙인다**:

> **정정**: `evidence/R4-B9/SC-79-seqgaps.log` 의 `"item": "SC-59 (AC-16c) …"` 는 **도구의 라벨 오기**다. 이 슬라이스에서 **sequence 빈틈은 SC-79** 이고, **SC-59 는 육안 항목**(§5.1)이다. **그 로그는 SC-59 의 증거가 아니다.** 도구 수정은 다음 라운드(qa 소유 파일이지만 이번 라운드 증거의 문구를 바꾸면 대조가 어긋난다).

#### (7) 짝 없는 함선 3 → 4 — 장부를 고치지 않았다

architect 지시: **동결 장부의 숫자는 고치지 마라.** 확인한 것:

- **`ship_events.py` 의 `FROZEN_DEFECT_LEDGER` 는 자기 참조 7건에 대한 것**이고, "짝 없는 함선"과는 **다른 장부다.** 나는 이것을 **건드리지 않았다.** SC-81 등식은 그대로 성립한다(결함 7 == 장부 7, 신규 0 · 누락 0).
- **"3척"이 적힌 곳은 서술 리포트**(`04_qa_report_r3.md` §6, 이 리포트 §3.1)다. **그 둘을 수정하지 않았다.** §5.3 의 "계약 외 기록"이 **정정 레코드**이고, 거기에 4척 전부의 `ship_id` 와 spawn tick 을 적었다.
- 4척 다 **tick ≤ 361575 의 오래된 행**이고 **신규 0** 이므로 **SC-76 판정과 SC-81 등식에 영향이 없다.**

**덮어쓰지 않는 이유**(architect): 장부를 고치면 **"처음부터 4였다"가 되어 이 슬라이스가 무엇을 배웠는지가 사라진다.** 배운 것은 *"집계 리포트의 열거는 전수 검사가 아니다"* 이고, 그 교훈은 **3 과 4 가 나란히 남아 있을 때만** 읽힌다.

#### (8) 갱신된 집계

| 판정 | 수 | 항목 |
|---|---|---|
| **PASS** | **13** | SC-01 · 76 · 77 · 78 · 79 · 80 · 81 · 82 · 83 · 84 · 85 · 86 · 87 |
| **미검증(증거 요건)** | **1** | **SC-59**(§5.9 (2) 재판정. **비-통과 — 슬라이스 종료를 막는다**) |
| **FAIL** | **0** | — |
| **대기** | **1** | **SC-89**(H-1~H-13 미구현) |

**§5.6 의 "PASS 13 / FAIL 1" 은 이 표로 읽는다.** FAIL 0 이 된 것은 **제품이 나아져서가 아니라 귀속이 정확해졌기 때문**이고, **막고 있는 게이트 수는 그대로 1 개**다.

#### (9) 남은 것 — §5.5·§5.8 의 열린 항목 최종 상태

| 항목 | 상태 |
|---|---|
| SC-59 (b) 문구 | **닫힘** — (b1)/(b2) 분할(§5.9 (3)) |
| §0.3 다섯 번째 칸 | **닫힘** — 신설, 조건 2개 포함(§5.9 (2)) |
| SC-89 신설 | **닫힘** — 승인 + 수정 3건 반영(§5.9 (5)) |
| 지표 신뢰성(`reconcile_error_deg`) | **닫힘** — 오진이었고 원인은 범위 불일치. **`…_session`/`…_run` 분리로 해소**(§5.9 (1)). **SC-56 은 catch-up 수정 뒤 세션 범위로 잰다** |
| 1002 전용 grep 태그 | **닫힘** — 리더 승인, `H-8` 합류. **재판정 확인 3건은 §5.8 (5) 2번** |
| 도구 라벨 오기 | **정정 붙임**(§5.9 (6)), 수정은 다음 라운드 |
| **SC-59 재촬영** | **열림 — 사람만 할 수 있다.** 이제 절차가 확정됐다: Game View **활성 탭·포커스 유지**, HUD 확인 **2회**(시작 직후·클립 a 직전), 할 일을 **촬영 전에 전부 안내**, **SC-89 와 분리된 세션** |
| SC-46 · 47 · 49 재실행 | **열림** — EditMode 가 159/157 → 167/164 로 바뀌었고 H-7 로 또 바뀐다 |
| SC-56 | **열림** — catch-up 수정 뒤, **세션 범위**로 |

---

### 5.10 Q-1 ~ Q-4 수행 (qa3, 2026-09-23) — 계약 9차 보충 + SC-89 양성 대조 구현

리더 지시로 client 의 H-1~H-9 를 기다리지 않고 수행했다(의존 관계 없음). **실서버 측정은 하지 않았다**(§5.10 (5)).

#### Q-1 — `outbound queue full` 을 합격 조건에서 **관측으로 내렸다**

**내가 (a) 에 넣었던 근거가 절반 무너졌다.** 나는 §5.7 (2) 에서 *"경로 B = 입력 유실 **＋** 예측 어긋남"* 이라 적고, 그래서 SC-89 (a) 가 재는 성질과 같다고 주장했다. **architect 판정 K-1 이 그 절반을 없앴다** — 송신 실패 tick 도 **이월 입력으로 예측**하게 되므로(`GreyboxSession.cs:446` 의 `return` 제거) **어긋남 쪽이 사라진다.**

남는 절반("플레이어 입력이 서버에 닿지 않는다")은 **SC-89 가 재는 성질(위반·끊김)과 다른 성질**이다. **한 항목이 두 성질을 재게 하는 것은 이번 라운드에 SC-59 (b) 에서 내가 직접 정정한 바로 그 결함이다** — 내가 남의 항목에서 고친 형태를 내 항목에 만들어 두고 있었다.

→ 계약: `outbound_queue_full_total` 은 **(d) 관측**이고 **0이 아니어도 SC-89 의 FAIL 이 아니라 별건 발견으로 연다.**

#### Q-2 — 양성 대조를 **봇으로** 만들었다 (`tools/bots/`, qa 소유)

**"봇으로는 (a) 를 닫을 수 없다"와 모순되지 않는다.** architect 의 구분이 정확하다:

> 봇이 (a) 에 못 쓰이는 이유는 **원하는 분포를 정확히 만들 수 있기 때문**이고(그래서 "실제 클라이언트가 버스트를 내는가"를 증명하지 못한다), **그 성질이 양성 대조에서는 정확히 필요한 것**이다.

**구현**: `bots probe --case tick-burst`
- `Behavior::TickBurst { per_tick, rounds, gap, grace }` — `src/conn.rs`. **한 라운드의 9건 사이에 `await` 지점(pump)을 두지 않는다** — 같은 서버 tick 에 얹히는 것이 목적이다.
- **서버가 먼저 닫기를 기다린다.** 클라이언트 쪽에서 닫으면 `close_reason` 이 `CLIENT_CLOSED` 가 되어 **아무것도 증명하지 못한다.**
- 봇은 close code 를 ADR-0005 표로 읽으므로 **1002 → `PROTOCOL_VIOLATION`** 이 그대로 찍힌다(`ledger.rs:close_code_meaning`, 7차에 이미 들어간 것).

**⚠ 이 대조의 핵심은 두 문턱을 가르는 것이다.** 순진하게 최대 속도로 쏘면 **`RATE_LIMITED`(평균 속도, `rate_limit_hz` 40)가 먼저 발동해 위반 경로를 밟기 전에 막힌다.** 그러면 끊기기는 끊기므로 **겉보기에는 대조가 동작하는 것처럼 보이지만 다른 것을 증명한다.** 그래서:

| 값 | 겨냥하는 것 |
|---|---|
| `per_tick = 9` | **tick 당 상한 8** 을 넘긴다 → 그 tick 에 위반 1건 |
| `rounds = 10` | **위반 예산 8건/10초** 를 채운다 |
| `gap = 500 ms` | 평균 **18 Hz** → **`rate_limit_hz`(40) 아래** 로 유지한다 |

#### Q-2 (계속) — **대조 자신의 자기 검증**(§7a 를 게이트 자신에게)

리더 지시: *"봇 양성 대조를 만들었으면, 그것이 수정 전 코드에서 실제로 위반을 만들어내는지 먼저 확인하라."* 실행 확인은 (5) 에 적었고, **정적으로 고정할 수 있는 부분은 테스트로 고정했다**: `tools/bots/tests/tick_burst_control.rs`, **4건 신설**.

**숫자를 테스트에 복사하지 않았다.** 서버 문턱을 `scenario.rs` 의 공개 상수(`SERVER_TICK_COMMAND_CAP`·`VIOLATION_BUDGET`·`VIOLATION_WINDOW`·`RATE_LIMIT_HZ`)로 꺼내고, **프로브와 테스트가 같은 `tick_burst_plan()` 을 부른다.** 복사했다면 이 테스트가 사본을 검사하게 되고, **그것이 내가 SC-89 (c) 로 client 에게 요구한 바로 그 결함이다.**

**고장 주입 4종이 각각 빨간불을 켜는 것을 확인했다**(`each_way_of_breaking_the_control_is_detected`):

| 고장 | 검출하는 술어 |
|---|---|
| `per_tick` 을 8 로 내린다 | `exceeds_tick_cap()` false — 위반이 한 건도 안 매겨진다 |
| `rounds` 를 7 로 내린다 | `fills_violation_budget()` false — 서버가 닫지 않는다 |
| `gap` 을 2 s 로 키운다 | `fills_violation_budget()` false — 8회가 10초 창 밖 |
| **`gap` 을 50 ms 로 줄인다** | **앞의 두 검사는 통과한다**(그래서 셋째 검사가 따로 있어야 한다). `stays_under_rate_limit()` 가 false — 평균 180 Hz |

**넷째가 가장 위험한 형태다** — 끊기기는 끊기므로 앞의 두 검사만 있으면 조용히 통과한다.

**실행 결과**: `cargo test` **74 passed / 0 failed**(신설 4건 포함 — 라운드 4 기준선 70 에서 +4), `cargo clippy --all-targets -- -D warnings` **exit 0**, `cargo fmt --check` **exit 0**. CLI 배선 확인(`bots probe --case tick-burst` 가 파싱되고 `STARFALL_DEV_AUTH_SECRET` 부재로만 멈춘다).

#### Q-3 — SC-59 (b1)/(b2) 분할을 계약에 반영

**왜 영상으로 (b1) 을 닫지 않는지**를 항목에 적었다(architect 근거): 마커가 **4.2 km 안**, 경계가 **12 km** 라 한 시야에 4개를 넣으려면 **게임에 없는 카메라 거리**가 필요하다. **촬영을 위해 게임을 바꾸는 것은 거꾸로다.**
대신 **녹화 시작 프레임의 HUD 에 마커 4개의 ID·거리를 한 줄로** 표시한다(client) → **"4개가 있다"와 "지금 1개가 보인다"를 같은 영상에서 읽을 수 있다.**

#### Q-4 — 촬영 분리를 **양쪽 항목에** 명시

**SC-59 와 SC-89 는 같은 세션으로 촬영할 수 없다.** SC-59 방법 칸에 "Game View 활성 탭·포커스 유지 + HUD 확인 2회", SC-89 방법 칸에 "증거는 화면이 아니라 `grep` 가능한 카운터 로그와 DB"를 적었다. 근거(`OnGUI` 는 비포커스면 호출되지 않는다)도 함께 적었다.

**＋ SC-56 에도 architect 판정을 반영했다**: **세션 범위로 판정**하고 `_run` 누적 짝 셋은 맥락으로, **`max` 를 적을 때는 언제나 표본 수 `n` 을 같이 적는다**(`deg max = 80.5663 (n = 412)`). *`max` 혼자서는 "더 큰 표본이 안 들어왔다"와 "지표가 멈췄다"를 구분하지 못한다 — 카운터 항등식에 `accepted > 0` 을 짝지은 것과 같은 규율을 게이지에 적용한 것이다.* **측정 시점은 catch-up 수정 뒤.**

#### (5) 실행하지 않은 것 — **대조의 실서버 RED 확인**

리더 지시 둘이 서로 맞물린다:
1. *"실서버 측정은 client 가 끝난 뒤 내 신호를 받고 시작하라 — **지금 띄우면 수정 전 코드를 재게 된다**."*
2. *"봇 양성 대조가 **수정 전 코드에서 실제로 위반을 만들어내는지 먼저 확인하라**."*

**1의 이유가 이 실행에는 적용되지 않는다**: 이 대조는 **봇만 쓰고 `client/` 코드를 한 줄도 타지 않으며**, 재는 대상인 **서버는 이번 수정에서 바뀌지 않는다**(architect §12). 즉 "수정 전 코드를 재게 된다"는 위험이 없고, 오히려 **지금 확인해 두는 편이 낫다**(수정 뒤에 확인하면 "서버가 원래 이랬는지"를 알 수 없다).

**그러나 명시적 지시를 말없이 넘기지 않는다.** 이 실행은 `domain_events` 에 **새 인스턴스 행을 영구히 남긴다**(추가 전용, 원칙 5) — 블록 9 의 구간 계산에 인스턴스가 하나 더 생긴다. **리더 승인을 받고 실행한다.** 소요 5분.

**실행하면 볼 것**(승인 시):
- 봇 원장의 **`peer_close_code = 1002` → `PROTOCOL_VIOLATION`**, 그리고 그 close 가 **서버가 먼저** 보낸 것(`"server"`).
- 서버 로그의 `프로토콜 위반 예산 초과 … budget=8 window_s=10`.
- `/debug/stats` 델타에서 **`commands_dropped_over_tick_cap_total` > 0** 이고 **`RATE_LIMITED` 가 그보다 훨씬 작다** — **두 문턱이 갈렸다는 증거**다. 이것이 없으면 대조가 rate limiter 를 잰 것이다.
- DB 에서 그 correlation 의 `SESSION_CLOSED.close_reason = 'PROTOCOL_VIOLATION'`.

---

### 5.11 SC-89 (b) 판별 단언 추가 — 내 (b) 에 (a) 와 같은 병이 남아 있었다 (architect 지적)

§5.10 에서 나는 (a) 의 결함("문턱 바로 아래를 통과로 읽는다")을 고쳤다. **같은 병이 (b) 에 남아 있었고 내가 보지 못했다.**

#### 무엇이 빠져 있었나

(b) 의 "한 `Update()` 최대 드레인 tick 수 > 1" 은 **조건이 일어났음**만 단언한다. **성질이 성립했음**을 단언하지 않는다. 그 틈으로 **수정 전 바이너리가 통과한다**:

| | 드레인 max | 위반 횟수 | 끊기는가 | (a) | (b) 앞 단언 |
|---|---|---|---|---|---|
| Editor 가 **throttle**(2 Hz) | 프레임당 10 | 매 프레임 1회 → **4초면 예산 참** | **끊긴다** | FAIL(정상) | — |
| Editor 가 **pause** | **복귀 프레임 하나가 거대하게** | **1회뿐** | **예산 8 에 못 미쳐 안 끊긴다** | **통과** | **> 1 이라 통과** |

**둘째 줄에서 SC-89 는 진짜 결함 위에서 초록이 된다.**

#### 고친 것 — 짝을 완성한다

> **(b) 에 "같은 창에서 프레임당 최대 송신 건수 == 1" 을 추가했다.**

| | 드레인 max | 송신/프레임 |
|---|---|---|
| **수정 전** | > 1 | **> 1** |
| **수정 후** | > 1 | **== 1** |

**"조건이 일어났다" + "성질이 성립했다"** 가 두 수로 함께 붙는다. C-1 계측이 이미 두 수를 다 잰다. **내가 §5.3 에서 남의 초록불에 요구한 형태 그대로이고, 내 항목에는 반쪽만 들어가 있었다.** 이 라운드에서 **같은 종류의 누락을 세 번째로 기록한다**((a) 문턱 · (d) 두 성질 혼합 · (b) 판별 단언).

#### 재현 절차에 대체 벡터를 넣었다

§5.10 에 적은 "백그라운드 2 Hz → 4초면 예산 참" 산수는 **Editor 가 throttle 할 때만 성립한다. pause 하면 복귀 프레임 1회뿐이라 예산이 안 찬다.** **어느 쪽인지는 아직 실측되지 않았다**(`runInBackground = 0` 은 Standalone 용이라 답이 아니다 — client R8).

1. **먼저 C-1 계측으로 백그라운드 구간의 드레인 max 와 프레임 간격을 읽어 throttle/pause 를 가른다.**
2. **pause 면 대체 벡터**: 도메인 리로드 반복 · 무거운 씬 로드 · 강제 GC 루프 — **전경에서 반복 히치**를 만든다. 필요한 것은 어느 쪽이든 **"10초 안에 450 ms 이상 히치 8회"**.
3. **(g) 양성 대조는 재현 벡터와 무관하게 유효하다** — **탐지기가 작동한다는 증명은 재현 방법과 독립이다.** 그래서 §5.10 (5) 의 봇 실행은 이 불확실성에 걸리지 않는다.

#### 곁들여 확정된 것

**SC-56 은 H-10′ 이전에는 판정 자체가 불가능하다**(architect 확인). §5.9 (1) 에서 내가 끌어낸 귀결 — `n = 2627` / `max = 323.1212` 가 **실행 전체 값**이므로 **SC-56 이 읽을 세션 범위 값은 아직 아무도 본 적이 없다** — 이 그대로 받아들여졌다. 지금 SC-56 을 재면 **무엇을 쟀는지 말할 수 없는 수**가 나온다.

**장부 건**: architect 가 **내 확인이 자기 지시보다 정확했다**고 정정했다 — `FROZEN_DEFECT_LEDGER` 가 자기 참조 7건의 장부라 "짝 없는 함선"과 다른 기록이므로, "장부를 고치지 마라"는 지시의 **전제 자체가 어긋나 있었다.** 결론(덮어쓰지 않는다)은 같고 이유가 다르다: **"집계 리포트의 열거는 전수 검사가 아니다"는 3과 4가 나란히 남아 있을 때만 읽힌다.**

---

### 5.12 계약 외 발견 — **SC-87 은 세 라운드 동안 계약에 집이 없었다** (qa3 실측)

계약 표를 기계로 훑다가 찾았다.

```
$ grep -o "^| SC-[0-9]*" 02_sprint_contract.md | 번호만 뽑아 1..89 와 대조
총 88개 / 1~89 중 없는 번호: [87]
$ grep -n "SC-87" 02_sprint_contract.md
(출력 없음)
```

**`SC-87` 은 계약 `02_sprint_contract.md` 어디에도 없다.** 그런데 **라운드 2·4 에서 세 번 판정됐다**: r2 §1.8(신설·PASS), r4 §2(PASS), r4 §4.1(PASS), 그리고 이번 **§5.4(PASS)**. 머리의 항목 수도 "86 + SC-88 + SC-89"로 **SC-87 을 세지 않았다.**

#### 왜 문제인가

계약 머리의 구속 문장: *"Phase 5 평가(`04_qa_report_r{N}.md`)는 **이 표의 항목으로만** 한다."*

**즉 내 §5.4 의 SC-87 PASS 는 계약에 없는 항목에 대한 판정이었다.** 실무적 위험은 "없는 것을 쟀다"가 아니라 — 실제로 유용한 검사였고 라운드 2 에서 검출력까지 확인됐다 — **합격 기준이 지난 리포트 본문에만 있다**는 것이다. 그래서 **내가 이번에 돌린 것이 그때 합의된 것과 같은지 아무도 대조할 수 없었다.** 이것은 SC-89 를 만들 때 architect 가 지적한 것("계약에 그 집이 없었다")과 **같은 형태의 공백**이고, 다만 **항목이 실재하고 판정까지 되고 있다는 점이 더 나쁘다** — 공백이 초록불에 가려진다.

#### 처리 — 표로 옮겼다 (9차)

E절(결정성) `SC-34` 다음에 넣고 항목 수를 **89** 로 고쳤다. **새 요구를 만들지 않았다** — 문구는 r2 §1.8 이 실제로 실행하고 확인한 것을 그대로 옮긴 것이다:

- `cargo test -p starfall-sim --test determinism --locked` **exit 0** ＋ `git diff --exit-code -- …/replay/` **exit 0**, 골든 3파일 sha256 기록.
- **＋ 검출력을 함께 보인다**(§7a): **골든 비교는 파일이 안 바뀌면 언제나 초록**이므로, 골든 한 파일의 바이트를 일부러 바꾸면 **실패하고 첫 차이 오프셋을 말해야 한다.**
- **⚠ 복원은 백업에서 하고 `STARFALL_REPLAY_BLESS` 를 쓰지 않는다** — bless 로 덮으면 **"테스트가 자기 기대값을 다시 쓴 것"과 "원본이 돌아온 것"을 구분할 수 없다**(r2 §1.8 이 실제로 그렇게 했다).

**architect 확인 요청**: 표로 옮긴 것과 문구가 r2 의 합의와 같은지 확인해 달라. **내가 새로 더한 것은 없고**, 다만 r2 §1.8 이 "했다"로 적은 검출력 확인을 **항목의 요구로 못 박았다**(그때도 실제로 했으므로 요구 강화가 아니라 명문화다).

**이번 라운드 §5.4 의 SC-87 PASS 는 유지한다** — 내가 실행한 것(`--locked` 테스트 + `git diff --exit-code` + 골든 3해시 `824a6489…`/`c924e59f…`/`3e00e527…`)이 위 문구의 첫 줄을 그대로 덮는다. **다만 검출력 재확인은 이번에 하지 않았다**(r2 에서 한 것을 재사용했다). 그 사실을 여기 적는다.

#### 왜 세 라운드 동안 아무도 못 봤나 — 기록해 둘 값이 있다

**번호가 연속이면 사람 눈에는 표에 있는 것처럼 보인다.** SC-86 과 SC-88 이 표에 있고 리포트들이 SC-87 을 자연스럽게 인용하니, 읽는 쪽은 그 사이가 비었다고 의심할 이유가 없었다. **집계를 눈으로 세지 말고 기계로 훑어야 한다** — 이 슬라이스의 "짝 없는 함선 3 → 4"(§5.3)와 **같은 교훈의 두 번째 사례**다: **집계 리포트의 열거는 전수 검사가 아니다.**

→ **다음 슬라이스 계약의 표준 게이트 제안**: `02_*` 의 SC 번호를 훑어 **빈 번호와 중복을 드러내는 검사 한 줄**을 넣는다. 비용이 거의 없고, 이번에 두 번 걸린 형태를 막는다.

---

### 5.13 SC-89 (g) 양성 대조 실행 — **탐지기는 증명됐고, 내 모델은 틀렸다** (qa3, 2026-09-23)

리더 승인으로 전용 인스턴스를 띄워 실행했다. 증거 `evidence/R4-B9/sc89/`.

**환경**: `server_boot.py serve`(드레인 경로), **`start_tick = 473421`** — 촬영 인스턴스와 **tick 구간이 갈린다**. 시작 시 `commands_received_total = 0` / `protocol_violations_total = 0` / `ws_connections = 0`(깨끗한 전용 인스턴스). 종료는 **정상**(`SHUTDOWN exit=0`, 하드 킬 없음), 로그 `dropped=0`.

#### (1) ✅ 증명된 것 — 서버가 실제로 위반을 계수하고 예산이 차면 끊는다

| 출처 | 관측 |
|---|---|
| 봇 원장 | `close: code=1002 reason_text="" **initiator=server** meaning=PROTOCOL_VIOLATION` |
| 서버 로그 | `프로토콜 위반 예산 초과 — 연결을 닫는다 budget=8 window_s=10`, `dropped=0` |
| `/debug/stats` 델타 | `protocol_violations_total` 0 → **9** · `commands_dropped_over_tick_cap_total` 0 → **9** |
| DB | 473999 `SESSION_OPENED`+`SHIP_SPAWNED` → **474080 `SESSION_CLOSED{PROTOCOL_VIOLATION}`** → 474680 `SHIP_DESPAWNED` (**정확히 600 tick** — SC-32 재확인) |

**SC-89 (g) 의 목적은 달성됐다.** 탐지기가 작동한다는 것이 실서버에서 확정됐다. 이제 나중에 재현 벡터로 위반이 안 나올 때 **"수정이 됐다"와 "벡터가 틀렸다"를 가를 수 있다.**

#### (2) ❌ 그러나 리더가 지정한 판별 기준을 통과하지 못했다 — 그리고 **그 기준은 성립할 수 없다**

> 리더: *"`commands_dropped_over_tick_cap_total` > 0 이고 **`RATE_LIMITED` 가 그보다 훨씬 작을 것** — 이게 없으면 네 대조는 rate limiter 를 잰 것이다."*

실측: **`RATE_LIMITED` +48 vs 틱상한 드롭 9.** 훨씬 작기는커녕 **5배 크다.**

**내 설계 근거가 틀렸다.** 나는 `gap = 500 ms` 로 평균 18 Hz 를 만들면 `rate_limit_hz`(40) 아래라 **"두 문턱이 갈린다"** 고 계약과 §5.10 에 적었다. **코드를 읽어 확인한 진짜 구조는 이렇다:**

| 층 | 상한 | 초과하면 | 파일 |
|---|---|---|---|
| **게이트웨이** | `MAX_COMMANDS_PER_SESSION_PER_TICK` = **8/tick** | 제출하지 않는다. `COMMAND_RESULT` 도 없다. `commands_dropped_over_tick_cap_total`++ 와 **그 tick 에 위반 1회**(명령 1건당이 아니다) | `gateway/src/runtime.rs:88~95` |
| **시뮬레이션** | `rate_limit_per_tick_cap` = `rate_limit_hz.div_ceil(tick_hz)` = 40/20 = **2/tick** | `RATE_LIMITED` 거부 + `COMMAND_RESULT` | `sim/src/session.rs:112~124`, `simulation.rs:113~117` |

**`RATE_LIMITED` 는 평균 속도 문턱이 아니다. 두 층 다 "tick 당 건수"를 본다.** 그러므로 **한 tick 에 몰아 보내면 두 층이 반드시 함께 걸리고, 간격을 벌려 평균을 낮춰도 갈라지지 않는다.** 리더의 기준은 **어떤 gap 을 골라도 만족시킬 수 없다.**

**유도가 실측을 정확히 예측한다.** 9건/라운드 →
- 게이트웨이: 8 통과, **1 드롭 → 위반 1회**
- 시뮬레이션: 통과한 8 중 **2 수락, 6 `RATE_LIMITED`**

라운드당 **드롭 1 : `RATE_LIMITED` 6 : 수락 2**. 실측 **9 : 48 : 16** = **8 라운드분의 1 : 6 : 2 + 닫히는 라운드**. 정확히 맞는다.

#### (3) ⚠ 더 나쁜 것 — 내 단위 테스트의 초록불이 아무것도 뜻하지 않았다

`stays_under_rate_limit()` 는 `per_tick / gap` 이라는 **서버를 모델하지 않는 수식**을 단언하고 있었다. 즉 **이 대조를 만들면서 막으려던 바로 그 병이, 막으려고 만든 검사 안에 있었다.** **이번 라운드 네 번째다**((a) 문턱 · (d) 두 성질 · (b) 판별 단언 · **(g) 검사 자신**).

**고쳤다**(`tools/bots/`):
- `average_hz()` · `stays_under_rate_limit()` · `RATE_LIMIT_HZ` **삭제**. 틀린 초록불을 남겨 두는 것이 가장 나쁘다.
- `SIM_RATE_LIMIT_PER_TICK_CAP = 2` 신설, `expected_tick_cap_drops_per_round()` · `expected_rate_limited_per_round()` · `charges_protocol_violations()` 로 **유도를 코드에 넣었다.**
- 테스트에 **`derivation_matches_the_measured_run`** 신설 — **유도가 실측(9 : 48 : 16)과 맞는지 붙잡는다. 그 실측이 이 모델의 유일한 근거다.**
- 고장 주입 ①에 단언을 하나 더했다: `per_tick = 8` 이면 **위반은 0인데 `RATE_LIMITED` 는 여전히 난다** → **거부 수만 보면 이 고장을 놓친다.** 그래서 판정은 `protocol_violations_total` 로 한다.
- 테스트 파일 머리에 **내가 틀렸던 경위를 그대로 적었다.**

**결과**: `cargo test` **75 passed / 0 failed**, `clippy --all-targets -D warnings` exit 0, `fmt --check` exit 0.

**계약 SC-89 (g) 도 고쳤다**: "gap 이 두 문턱을 가른다"를 빼고, **판정은 `protocol_violations_total` 로 하며 `RATE_LIMITED` 가 드롭보다 많은 것이 정상**이라고 실측값과 함께 적었다.

#### (4) 계약 §7b 신설 — **자명 통과 시험** (architect 제안, 리더 지시)

**기록으로 끝내면 다섯 번째가 온다.** 같은 자리에서 네 번 걸린 것은 주의력 문제가 아니라 **절차에 그 단계가 없다**는 뜻이다.

> **새 항목의 각 절 옆에, "이 절을 자명하게 통과시키는 상태"를 한 줄로 적는다. 그 상태가 실제로 가능하면 절을 고친다.**

§7a 옆(§7b)에 넣고 네 건을 근거 표로 붙였다. **⚠ 이 시험은 검사 자신에게도 적용한다** — (g) 가 그 사례다.

#### (5) 정정 — 앞 절의 틀린 문장

- **§5.10 Q-2**: *"`gap = 500 ms` … 평균 18 Hz → `rate_limit_hz`(40) 아래로 유지한다"*, *"두 문턱을 가르는 것"* → **성립하지 않는다.** 위 (2) 로 읽는다. `gap` 의 실제 역할은 **라운드를 서로 다른 tick 에 떨어뜨리고 예산 창(10 s) 안에 `rounds` 회가 들어가게 하는 것**이다.
- **§5.10 (5)** 의 "볼 것" 셋째 항목(*"`RATE_LIMITED` 가 그보다 훨씬 작을 것 — 두 문턱이 갈렸다는 증거"*) → **그 기준은 구조적으로 성립할 수 없다.** 대체 기준은 **`protocol_violations_total` > 0 과 서버가 먼저 보낸 close 1002**이고, 둘 다 관측됐다.

**앞 절을 덮어쓰지 않는다**(원칙 5의 정신). 틀린 문장이 남아 있고 여기에 정정이 붙는다.

---

### 5.14 architect·리더 지시 반영 — SC-87 이관 확정, §0.12 신설, 게이트 방향 역전 (qa3, 2026-09-23)

#### (1) SC-87 — ①② 승인. **재확인 주기**를 항목에 적었다

architect: *"검출력 절이 없는 SC-87 은 **구조적으로 `0 == 0`** 이다 — 검출력 절은 SC-87 에 **더해진 것이 아니라 SC-87 을 성립시키는 것**이다."* 골든 비교는 **파일이 안 바뀌면 언제나 초록**이므로 그 말이 맞다.

**주기를 못 박았다**(매 라운드 돌리는 것은 낭비다):
> **골든 파일 · 테스트 · 직렬화 경로 중 하나라도 바뀌면 다시 확인한다. 셋 다 그대로면 직전 확인을 재사용하고, 재사용했다는 사실을 리포트에 적는다.**

**이번 라운드는 재사용이 정당하다** — `server/` 무변경이고 바이너리 `e84cd098c06fce26` 가 유효하다. §5.4 의 SC-87 PASS 를 유지하고, 재사용 사실은 §5.12 에 이미 적혀 있다.

#### (2) §0.12 신설 — bless 금지의 일반형

SC-87 만의 규칙이 아니라는 architect 판정을 받아 계약 §0.12 로 올렸다:
> **자기 기대값을 다시 쓸 수 있는 경로를 가진 게이트는, 그 경로가 막혀 있음이 확인되지 않는 한 판정에 쓰지 않는다.**

골든 `bless`, 스냅샷 `--update`, 기대값 자동 갱신이 전부 해당한다. **덮어쓰는 순간 그 게이트는 무엇을 재는지 말할 수 없게 된다.**

#### (3) ⚠ 내가 제안한 게이트의 **방향이 틀렸다** — 규칙이 만들어지자마자 제안자를 교정했다

§5.12 에서 나는 *"SC 번호를 훑어 **빈 번호와 중복**을 드러내는 검사"* 를 제안했다. architect 가 **내가 방금 만든 §7b 자명 통과 시험을 그 제안에 적용**했다:

> 자명 통과 상태 = **"계약에 없는 항목의 번호가 연속 범위 밖이다."** 실제로 가능하다 → 절을 고친다.

**맞다.** 이번에 빈 번호가 보인 것은 **SC-87 이 우연히 86 과 88 사이에 있었기 때문**이고, r2 가 그 검사를 `SC-90` 으로 붙였다면 **빈 번호가 안 생겨 내 게이트는 초록**이었다.

**판정 가능한 것은 역방향이다**: **리포트가 판정한 모든 항목이 계약 표에 있는가.** 번호 배치와 무관하게 **r2 당시에 즉시** 잡았을 형태다.

**규칙이 신설된 그 라운드 안에서 규칙을 제안한 사람의 다음 제안을 교정했다** — §7b 의 근거 표에 이 사례를 넣었다.

#### (4) 제안을 **도구로 만들었다** — `tests/e2e/check_contract_items.py` (qa 소유)

산문 제안으로 두지 않고 실행 가능한 검사로 썼다.

- **역방향을 판정한다**(exit 1), **순방향은 세기만 한다** — 블록이 나뉘어 돌고 `대기` 가 정상이므로 위반이 아니다.
- **산문 속 언급은 세지 않는다**(표 행만) — 안 그러면 "SC-99 는 다음 라운드에" 같은 참조가 위반으로 잡힌다.
- **selftest 5건, 양방향**(§3.3): `selftest: PASS 케이스=5`(`evidence/R4-B9/SC-items-selftest.log`).

**실제 대조**(`evidence/R4-B9/SC-items-check.log`) — 계약 + r2·r3·r4 세 리포트:
```
계약 항목: 89 / 판정표 행으로 다룬 항목: 46 / 안 나온 항목: 43 (위반 아님)
위반 없음 — 판정된 항목이 전부 계약 표에 있다.        exit 0
```

**RED 재현**(§7a — 게이트가 실제로 빨간불을 켜는가): SC-87 행을 뺀 계약 사본으로 같은 명령을 돌리면
```
!! 위반 — 계약 표에 집이 없는 채로 판정된 항목:  SC-87        exit 1
```
**즉 이 도구는 라운드 2에 있었다면 그날 SC-87 을 잡았을 것이다.** 다음 슬라이스 표준 게이트로 넘긴다.

#### (5) 리더 지적 — **판별 기준을 관측 전에 적어 둔 것이 (g) 를 잡았다**

리더가 실행 **전에** "`RATE_LIMITED` 가 훨씬 작을 것 — 없으면 rate limiter 를 잰 것이다"를 지정했기 때문에, 결과를 보자마자 어긋남을 알아차렸다. **지정하지 않았다면 "끊겼다 = 대조 성공"으로 넘어갔을 것이다** — §5.10 에서 *"겉보기엔 동작하는 것처럼 보인다"* 고 경고한 그 함정에, **그 경고를 쓴 내가 걸렸을 것이다.**

**이것이 자명 통과 시험의 실행 버전이다**: 판정 기준을 **관측 전에 종이에 적으면, 결과를 볼 때 그 줄이 대조군이 된다.** §7b 근거 표에 넣었다.

**⚠ 그 기준 자체는 틀렸지만**(구조적으로 성립 불가 — §5.13 (2)) **틀린 기준도 제 역할을 했다.** 기준이 어긋났다는 사실이 **구조를 들여다보게 만들었고**, 그 결과 두 층의 구조와 1 : 6 : 2 유도가 나왔다. **관측 전에 적어 두는 것의 값은 기준이 옳은지와 별개다.**

---

### 5.15 §5.13 의 귀속 정정 — **판별 기준의 오류는 내 것이 아니었다** (리더 정정, 2026-09-23)

§5.13 (2) 에서 나는 *"내 설계 근거가 틀렸다"* 로 적고 `gap = 500 ms` 설계 전체를 내 실패로 돌렸다. **리더가 절반을 자기 오류로 정정했고, 그 정정이 맞다.** 기록을 고쳐 둔다 — **내 도구의 결함으로만 남으면 기록이 틀린다.**

#### 갈라 적는다

| 무엇 | 누구의 오류인가 | 왜 |
|---|---|---|
| **"`RATE_LIMITED` 가 틱상한 드롭보다 훨씬 작을 것"** 이라는 판별 기준 | **리더** | `rate_limit_hz` 를 **평균 속도 문턱**으로 가정했고 **그 가정이 근거 없이 들어갔다.** 실제로는 두 층이 **둘 다 tick 당 건수**를 보므로 **어떤 간격을 골라도 만족시킬 수 없다** |
| **`gap = 500 ms` 가 두 문턱을 가르지 못한 것** | **누구의 실패도 아니다** | **가를 수 없는 것을 가르라고 요구받은 것**이다. 설계 실패가 아니다 |
| **계약·§5.10 에 "두 문턱을 가른다"고 적은 것** | **나** | 검증되지 않은 기준을 **내 문장으로 옮겨 적었다.** 남의 가정이어도 내 이름으로 계약에 들어간 이상 내 것이다 |
| **단위 테스트가 서버를 모델하지 않은 것** | **나** | 리더의 기준과 **별개**다. `per_tick / gap` 은 누가 시킨 적 없는 내 수식이고, **그 초록불은 아무것도 뜻하지 않았다** |

**§5.13 (2) 의 "내 설계 근거가 틀렸다"는 위 4행으로 읽는다.** 앞 절은 덮어쓰지 않는다.

#### 이 정정이 §7b 에 더하는 것

리더: *"판정 기준을 만드는 사람도 자기 기준에 그 시험을 돌려야 한다."*

**자명 통과 시험을 리더의 기준에 돌렸다면 바로 나왔을 것이다** — *"이 기준을 자명하게 **불통과**시키는 상태"* 를 한 줄 적었다면 **"두 문턱이 같은 단위를 본다면 이 기준은 어떤 입력으로도 만족되지 않는다"** 가 나온다. 즉 이 기준은 **자명 통과가 아니라 자명 불통과**였고, 시험의 양쪽을 다 봐야 잡힌다.

→ §7b 의 문장을 이 사례에 맞게 읽는다: **"자명하게 통과시키는 상태"와 "어떤 입력으로도 만족될 수 없는 상태"를 둘 다 본다.** 전자는 초록불이 공짜가 되는 경우고, **후자는 빨간불이 영원한 경우**다. 둘 다 그 절이 아무것도 재지 않는다는 뜻이다.

#### (g) 판정

리더가 대체 기준(**`protocol_violations_total` > 0** 과 **서버가 먼저 보낸 close 1002**)을 받아들였고 **둘 다 관측됐다**(§5.13 (1)). → **SC-89 (g) 는 닫힌 것으로 본다.** SC-89 의 나머지 절 (a)(b)(c)(d)(e)(f) 는 **대기**(H-1~H-13 미구현)이므로 **항목 전체의 판정은 여전히 대기**다.

---

### 5.16 SC-89 판정 — **(c)(e)(g) PASS / (a)(b) 미검증(증거 요건)** (qa3, 2026-09-23)

관측 기록: `evidence/R4-B6/sc59-v2/10-sc89-background-observation.md`(리더). **의도한 실험이 아니라 전체화면 게임이 전경을 차지해 생긴 상황**이고, 그것이 정확히 재현 조건이었다.

**먼저: 이 관측은 중요하고 좋은 결과다.** 어제 같은 종류의 조건에서 **459초에 `PROTOCOL_VIOLATION` 강제 종료**되던 것이, 오늘 **670초 정상 종료 · 위반 0 · 드롭 0 · `RATE_LIMITED` 0**이다. 수정이 겨냥한 것을 실서버에서 보여 준다. 아래는 그것을 부정하는 것이 아니라, **각 절이 닫히는가**를 절별로 가른 것이다.

#### 절별 판정

| 절 | 판정 | 근거 |
|---|---|---|
| **(a)** 송신 | **미검증(증거 요건)** | **실서버 쪽은 충족**: `protocol_violations_total` 0 · `commands_dropped_over_tick_cap_total` 0 · 거부 8라벨 전부 0 · `outbound_queue_full_total` 0, 그리고 **`max_sends_per_frame = 1` 인데 `max_ticks_drained_per_update = 20`**(히치가 실제로 있었다). `commands_received_total = 367` 로 경로가 실제로 탔다. **그러나 T-5 성질 테스트의 불변식 ③이 항진명제다**(아래 (2)) |
| **(b)** 재조정 | **미검증(증거 요건)** | 숫자는 전부 갖춰졌다(`catchup_carry_forward_ticks_total = 51 > 0`, **`reconcile_hard_snap_total = 0`**, n=170). **그러나 그 0이 자명하다**(아래 (1)) |
| **(c)** 도구 결합 | **PASS** | `grep -n 'while (_tickAccumulator' GreyboxSession.cs` → **exit 1**(0건). `Scripts/Flight/TickCatchUp.cs` 존재. `GreyboxSendBurstTests.cs:112·152` 가 **`TickCatchUp.Compute`(→ `TickCatchUp.Plan`)를 직접 부른다** — 루프 사본이 사라졌다. *문구의 "`TickCatchUp.Plan` 을 부른다"는 실제로 `Compute()` 가 `Plan` 을 돌려주는 형태다. 같은 순수 성분이고 의도대로다* |
| **(d)** 큐 포화 | **관측: 0** | 두 세션 모두 `outbound_queue_full_total = 0`. 합격 조건이 아니다(Q-1) |
| **(e)** 임계 불변 | **PASS** | `git status --short -- server/ data/` **빈 출력**, `git diff HEAD --stat -- server/` **빈 출력**. 실값 확인: `MAX_COMMANDS_PER_SESSION_PER_TICK = 8`(`runtime.rs:95`) · `VIOLATION_BUDGET = 8`(`ws.rs:54`) · `rate_limit_hz = 40` · `protocol_violation_hz = 100`(`data/movement/sync-tuning.json:17~18`). **네 값 전부 계약값 그대로** |
| **(f)** 잔여 | **기록 + 통지** | `reconcile_forced_after_hitch_total = 0` ✅ / **`catchup_truncated_total = 5`**(1차 1) — 아래 (3) |
| **(g)** 양성 대조 | **PASS** | §5.13·§5.15 |

#### (1) ⚠ (b) 의 `hard_snap_total = 0` 은 **자명 통과 상태에서 나왔다**

계약 §7b 1번을 (b) 에 돌리면: *"이 절을 자명하게 통과시키는 상태"* = **함선이 사실상 정지해 있고 재조정이 되돌릴 미확인 입력이 없는 상태.** 그 상태가 실제로 가능한가 — **이번 세션이 정확히 그 상태였다.**

CL-2 로그(qa3 가 `client/Logs/Editor.log` 에서 직접 읽었다. 리더가 인용한 것은 send-burst 줄뿐이라 이 줄이 빠져 있었다):

```
CL-2 session-end reconcile observations -
  reconcile_has_error_total=170, reconcile_replayed_nonzero_total=0, reconcile_both_omega_nonzero_total=77
SC-56 session-end reconcile error stats -
  position_error_m(p50=0.0000, p99=0.0000, max=0.0000, n=170), ... reconcile_hard_snap_total=0
```

**`reconcile_replayed_nonzero_total = 0` 이다.** 코드에서 이 카운터는 `if (result.RetainedHistory.Count > 0)` 일 때만 오른다(`GreyboxSession.cs:515`) — 즉 **170번의 재조정 전부에서 되돌릴 미확인 입력이 하나도 없었다.** 되돌릴 것이 없으면 클라이언트 상태는 서버 상태와 같고, **위치 오차는 구조적으로 0이며 hard snap 은 일어날 수 없다.** 실제로 `position_error_m` 이 p50·p99·max **전부 0.0000**이다.

**그러므로 (b) 의 초록불은 "재조정이 잘 됐다"가 아니라 "재조정할 것이 없었다"를 뜻한다.** `catchup_carry_forward_ticks_total = 51 > 0` 이라는 짝 단언도 이 간극을 메우지 못한다 — **정지 상태의 이월은 0 입력을 이월하는 것이라 역시 자명하게 맞는다.**

> **제안 — (b) 에 세 번째 짝 단언을 넣는다**: **`reconcile_replayed_nonzero_total > 0`**.
> *계측을 새로 만들 필요가 없다 — CL-2 가 이미 그 수를 세고 있고, 이번 관측이 그 값이 0일 수 있음을 실증했다.* 이것이 있으면 (b) 는 "히치가 있었고(drain>1), 송신은 1건이었고(sends==1), **되돌릴 입력이 실제로 있었는데도**(replayed>0) hard snap 이 0이었다"가 된다.

#### (2) ⚠ T-5 불변식 ③이 **항진명제다** — (a) 가 닫히지 않는 이유

`GreyboxSendBurstTests.cs:119`:

```csharp
Assert.That(plan.TicksToPredict - plan.TicksToSend + plan.TicksToSend, Is.EqualTo(plan.TicksToPredict),
    "invariant 3: carry-forward ticks + sent ticks == predicted ticks (construction identity)");
```

**`x − y + y == x` 는 어떤 입력으로도 실패하지 않는다.** 계약 (a) 가 요구한 불변식 ③("**이월 tick 수 + 송신 tick 수 == 예측 tick 수**")은 **`plan` 의 필드를 재배열한 것이 아니라, 세션이 실제로 그만큼 이월·송신·예측했는지**를 재야 한다. 주석이 "construction identity"라고 정직하게 밝힌 점은 좋지만, **그 이름이 곧 "아무것도 재지 않는다"는 뜻이다.**

불변식 ①(`sendsThisFrame <= 1`)과 ②(벽시계 ±1 tick), 그리고 `hitchInjected` 짝 단언은 **실측이고 유효하다.** ③만 무효다.

> **수정 요청(client)**: `DriveThroughTransport` 가 **실제로 구동한 이월·휴면 예측 횟수를 세어** `carryForwardDriven + sendsThisFrame == plan.TicksToPredict` 를 단언한다. `sendsThisFrame` 은 이미 그렇게 세고 있다 — 이월 쪽만 같은 방식으로 세면 된다.

#### (3) `catchup_truncated_total = 5` — **발견으로 열지 않는다. 다만 M 경로가 처음 탔다**

architect 의 M = 20 근거: *"**정상 플레이에서 절대 발동하지 않아야 하므로**, 카운터가 0이 아니면 그 자체가 발견이다. **도메인 리로드 같은 Editor 사정에서만 발동한다.**"*

**이번 구간은 정상 플레이가 아니다** — 전체화면 게임이 전경을 차지해 Editor 가 밀린 **OS·Editor 스케줄링 상황**이고, architect 가 "그때만 발동한다"고 적은 바로 그 경우다. **따라서 (f) 의 발동 조건(미룬 결정을 여는 조건)은 충족되지 않았다.**

**대신 두 가지를 기록한다**:
- **M(20 tick = 1초) 경로가 실플레이에서 처음 실행됐다**(5회 + 1차 세션 1회). `max_ticks_drained_per_update` 가 **상한 20에 붙어 있으므로 실제 히치는 1초보다 컸다** — 잘렸다는 뜻이다. M-17("신규 경로가 실제로 탔는가")의 **실행됨**이다.
- **잘림이 hard snap 을 숨기지 않았다**(qa3 가 코드로 확인): 잘림은 **누적기(`RemainingAccumulatorSeconds`)만 버리고** 재조정 경로를 우회하지 않는다. hard snap 은 여전히 `Reconcile` 의 밴드 분류에서 계수된다(`PredictedShipController.cs:101~108`). *다만 이번 세션에서는 (1) 때문에 애초에 숨길 오차 자체가 없었다.*

**throttle/pause 판별**(리더 질문): `catchup_truncated_total = 5` 와 drain 이 상한에 붙어 있는 것은 **1초를 넘는 히치가 반복됐다**는 뜻이다. 체크리스트 §8.2 의 기준으로는 **pause 쪽에 가깝다.** **다만 qa3 도 이것을 판정하지 않는다** — 전경/백그라운드 전환 시각이 기록되지 않아 **히치 1회당 크기와 빈도를 분리할 수 없다.** 판별에는 §8.2 의 통제된 절차가 필요하다.

#### (4) 리더의 한계 기술에 동의하는가 — **절반만**

리더: *"11.2분 > 7.6분은 '더 오래 버텼다'를 보일 뿐이고, 위반 0의 원인이 수정인지 히치 양상 차이인지 단정할 수 없다 — **(b) 의 짝 단언이 그 간극을 메운다**."*

- **간극을 메우는 것은 (b) 가 아니라 (a) 쪽 짝이다.** `max_ticks_drained_per_update = 20`(히치가 실제로 컸다) **그리고** `max_sends_per_frame = 1`(그런데도 프레임당 1건)이 **"히치 양상이 순했던 것이 아니라, 큰 히치가 있었는데도 버스트가 없었다"**를 보인다. **수정이 원인이라는 주장을 지탱하는 것은 이 두 수다.**
- **(b) 는 간극을 메우지 못한다** — (1) 때문에 자명 통과였다.

#### (5) 통제된 세션 B가 **필요하다**

리더 질문에 답한다: **필요하다.** 이유는 "더 엄밀해서"가 아니라 **이번 관측으로 닫히지 않는 절이 남았기 때문**이다.

| 필요한 것 | 왜 |
|---|---|
| **함선을 실제로 조작하면서** 백그라운드 구간을 만든다 | (b) 를 자명 통과에서 꺼낸다 — **`reconcile_replayed_nonzero_total > 0`** 이 되어야 hard snap 0 이 뜻을 갖는다 |
| **전경/백그라운드 전환 시각을 기록**한다 | throttle/pause 판별. 히치 1회당 크기와 빈도를 분리한다 |
| (사전) **T-5 불변식 ③ 수정** | (a) 는 그 뒤에 닫힌다 |

**SC-59 촬영과는 여전히 별개 세션이다**(계약 절차 제약). **SC-56 은 이 관측으로 판정하지 않는다** — 리더·architect 지시대로이고, **(1) 이 그 지시에 더 강한 이유를 준다**: `position_error_m max = 0.0000 (n=170)` 은 **되돌릴 입력이 없던 세션의 값**이라 "재조정이 정확하다"를 뜻하지 않는다. SC-56 은 **조작이 있는** 60초 구간에서 세션 범위로 재야 한다.

# p0-02-networking-spike QA 리포트 — 라운드 1

- 일시: 2026-09-19
- 기준: `02_sprint_contract.md` (SC-01 ~ SC-74, 판정 74항목 + 기록 M-1~M-13)
- 실행자: qa. **구현자 보고를 근거로 PASS를 주지 않았다** — 모든 PASS는 이 리포트에 명령·출력·건수가 있다.
- 원시 증거: `_workspace/p0-02-networking-spike/evidence/`

> 이 리포트는 블록이 끝날 때마다 append 했다(중단 대비). 절 번호는 계약 §4의 실행 블록과 같다.

## 요약

**PASS 73 / 부분 PASS 1 / FAIL 0 / 미검증(환경) 0 — 판정 대상 74항목.**
정확성 하드 게이트(SC-52~69) 18항목 전부 PASS. 성능 3줄도 기준을 만족했으나 **판정이 아니라 p1 회귀 기준선으로 기록**했다(사용자 결정).
구현 수정 요청 **0건**. 계약 외 발견 3건(아래). 다음 라운드 **불필요**.

---

## 블록 0 — 계약·빌드 (서버 불필요)

증거: `evidence/block0/`

| ID | 결과 | 증거 | 비고 |
|----|------|------|------|
| SC-01 | **PASS** | `cargo fmt --all --check` → exit 0, 출력 없음 (`sc01-04.txt`) | |
| SC-02 | **PASS** | `cargo clippy --workspace --all-targets -- -D warnings` → exit 0. 보조: `crates/sim`·`crates/persistence` 둘 다 `[lints] workspace = true` 옵트인 확인 (grep 2건) | 린트가 꺼진 채 통과한 것이 아니다 |
| SC-03 | **PASS** | `cargo test --workspace --locked` → exit 0. **82 passed / 0 failed / 3 ignored** (19+10+1+28+14+2+8, ignored 3) | `--locked` 통과 = `Cargo.lock` 최신 |
| SC-04 | **PASS** | `grep -nE "axum\|sqlx\|redis\|rand\|chrono" crates/sim/Cargo.toml` → **매칭 0** (rc=1) | IO-free·결정성 위생의 기계적 증거 |
| SC-32 | **PASS** | `cargo test -p starfall-contracts --locked -- --nocapture` → exit 0, 19+10 passed. 출력에 검사 건수가 찍힌다 | 아래 건수 참조 |
| SC-33 | **PASS** | `[SC-13] 왕복 검증한 유효 fixture: 12건` | 계약 §0.8의 12와 일치 |
| SC-34 | **PASS** | `[SC-14] 스키마가 거부한 반례: 16건` | 층① |
| SC-35 | **PASS** | `[SC-15] serde 매트릭스 검사한 반례: 16건`, 각 행이 기대=거부/실제=거부로 출력 | 층②. §0.5 표와 일치 |
| SC-36 | **PASS** | `[SC-18] required 변이 124건이 모두 역직렬화에 실패했다` | 변이 개수 124 |
| SC-37 | **PASS** | `[SC-16] server 태그 타입 6건이 대응표에 있다`, `[SC-12] 오프라인 검증한 스키마: 11건`, `[SC-17] $id 경로 일치 11건 / 레지스트리 6건 / fixture 12건` | |
| SC-38 | **PASS** | 생성기 2회 실행 후 해시 동일, `--check` exit 0 (`--check: up to date (7 file(s))`). **추가 확인: client 제출본과 재생성물이 바이트 동일** — 손댄 흔적 없음 | `sc38-codegen.txt` |
| SC-39 | **PASS** | 생성물 실물에서 `SessionOpenedEvent.ActorId`·`SessionClosedEvent.ActorId` 둘 다 `[JsonProperty("actor_id", Required = Required.Always)] public System.Guid ActorId`. EditMode `Invalid_Rejected_ByStrictProfile(SESSION_OPENED/actor-id-null.json)`·`(SESSION_CLOSED/actor-id-null.json)` 2건 통과 | `sc39-41-narrowing.txt`. 좁히지 않은 `causation_id`는 `Guid?`+`AllowNull`로 남아 있어 **수정이 무차별이 아님**도 확인 |
| SC-40 | **PASS** | `03_client_impl.md` §2.2 diff 전문: 파일 2 / hunk 2 / 속성 2. QA가 재생성해 client 제출본과 바이트 동일함을 확인(SC-38) | 구현자 문서 + QA 재현 |
| SC-41 | **PASS** | `ContractsCodegen.cs:208`에 `enum`이 Constraint 분류로 이동한 주석. §2.2 diff에 이 변경으로 인한 생성물 hunk 0 | 동작 무변화 |
| SC-42 | **PASS** | `unity test client --mode EditMode --report-format nunit,junit …` → exit 0, 리포트 2개 생성. **`tests="67" failures="0" errors="0" skipped="2"`**, `<test-case>` 67건 | 0건 통과가 아님이 수로 드러난다 |
| SC-43 | **PASS** | `Fixtures_RoundTrip_MatchesOriginal` **12건** 전부 Passed | 12 = 타입 6 × 2 |
| SC-44 | **PASS** | `Invalid_Rejected_ByStrictProfile` **11건** Passed + `Invalid_NotDetectableByCSharp_DocumentedAsymmetry` **5건** Passed | §0.5가 고정한 "C# 책임 11 / 감지 불가 5"와 **정확히 일치**. 감지 불가 5건은 판정 대상이 아니라 기록(M-12) |
| SC-45 | **PASS** | `RealtimeClientTests.Dispatch_UnknownMessageType_WarnsAndKeepsProcessing`, `Dispatch_UnreadableFrame_WarnsAndKeepsProcessing` | 경고 후 처리 계속 |
| SC-46 | **PASS** | `Runtime_IgnoresUnknownMember_AndWarns` / `Runtime_SameInput_IsAnExceptionUnderStrict` / `Runtime_MissingRequiredField_StillThrows` / `Runtime_NullInNonNullableField_StillThrows` / `Runtime_PrefixDependency_IsPinned` (5건) | 관용은 "모르는 멤버" 하나뿐임이 테스트로 고정됨 |
| SC-47 | **PASS** | `Backoff_BoundaryAttempts_AreAlwaysInsideTheCap`, `Backoff_CeilingDoublesThenClamps`, `Backoff_IsDeterministicForAFixedSeed`, `Backoff_RejectsNegativeAttempt` | |
| SC-48 | **PASS** | `FixtureLoader_FailsWhenTooFewValidFixtures`, `FixtureLoader_FindsRepoRootByMarker` | 가드 자체가 테스트됨 |
| SC-70 | **PASS** | `check_contract_coverage.py --strict` → **exit 0, errors 0 / warnings 0, RESULT: PASS**. 6타입 전부 producer·consumer 코드 참조 확인(`bots` 태그 4건 포함) | 기준선 errors 8 / warnings 4 → 0/0 |

**블록 0 집계: PASS 21 / FAIL 0 / 미검증 0**

측정 사실:
- EditMode **skipped 2건**은 `LiveServerTests.Live_ThreePings_RoundTripInOrder`, `Live_ServerInitiatedClose_ReconnectsAsANewSession` — **서버가 떠 있지 않아 게이트에 걸린 라이브 테스트**다. 리포트에 `skipped="2"`로 드러나므로 "조용히 빠진" 것이 아니다. 이 두 항목의 실행 증거는 client가 서버를 띄우고 만든 `unity-tests/Live.xml`이고, 블록 4에서 인용한다.
- 서버 테스트 82건 중 ignored 3건은 DB·서버가 필요한 통합 테스트로 보인다(블록 1 이후 재실행 대상 아님 — 계약 항목이 아니다).

---

## 블록 1 — 마이그레이션·월드·기동 (G-a: `down -v` 가 첫 DB 단계)

증거: `evidence/block1/`. 실행 순서: `docker compose down -v` → `up -d` → **서버 기동 전 조회** → 기동.

| ID | 결과 | 증거 | 비고 |
|----|------|------|------|
| SC-05 | **PASS** | `\d+ worlds`·`\d+ domain_events` 전문(`sc05-07-schema.txt`). 열·타입·NOT NULL 전부 ADR-0007 §2와 일치. CHECK 5종(`tick_hz 1..1000`, `calendar_scale>0`, `last_tick 0..2^53-1`, `tick`, `sequence`, `schema_version>=1`), `UNIQUE (world_id,tick,sequence)`, FK `domain_events.world_id → worlds`, 인덱스 `domain_events_type_idx`·`domain_events_correlation_idx`, 트리거 `domain_events_append_only BEFORE DELETE OR UPDATE … forbid_mutation()` | 불일치 0 |
| SC-06 | **PASS** | `select count(*) from worlds` = **1**. 행: `01a0b1c2-3d4e-7f01-8a2b-9c0d1e2f3a4b \| spike \| tick_hz=20 \| epoch=3800-01-01T00:00:00Z \| scale=60 \| sim_version=1` — 시드값과 문자열/정수로 동일 | **`last_tick` NULL 은 서버 기동 로그로 판정했다**: `last_tick=None start_tick=0`. 직접 SQL 은 20 tick(1초) 주기 커밋 때문에 기동 1초 뒤엔 이미 non-null 이 된다 — 관측 창이 1초라 SQL 로는 안정적으로 못 잡는다. `start_tick=0` 자체가 "`last_tick`·`max(tick)` 둘 다 없었다"의 증거다 |
| SC-07 | **PASS** | 2회차 기동 후 `_sqlx_migrations` **행 1개**, `installed_on=2026-09-19 05:35:46.471621+00` **불변**, checksum 동일(`b4f83ef7…40cc`), 기동 성공 | 재적용 없음 |
| SC-08 | **PASS** | `STARFALL_TICK_HZ=10` 기동 → **exit 1**, `ERROR … 기동 거부: 월드 상수 불일치 — worlds.tick_hz=20, STARFALL_TICK_HZ=10 (I-19, ADR-0006 §3)`. `worlds` 행 불변 | |
| SC-16 | **PASS** | 비밀 미설정 프로세스에서 `/ws` → **503 `{"reason":"auth_not_configured","status":"unavailable"}`**, 같은 프로세스 `/healthz` 200, `/readyz` 200 (기동 로그 1회 = 동일 프로세스) | 이 PC 의 `.env` 에 `STARFALL_DEV_AUTH_SECRET` 행이 **실제로 없어** 자연스럽게 재현됐다(`grep` 확인). 인위적 조작 아님 |

**부수 확인(계약 항목은 아니지만 증거로 남긴다)**

- **stdin `shutdown` 경로가 실동작한다**: `shutdown` 한 줄 → `stdin 'shutdown' 수신 — graceful shutdown 시작` → `tick 루프 종료 — SERVER_SHUTDOWN 스윕 후 영속화 flush` → `정상 종료 — 마지막 tick 까지 커밋 완료`. SC-27 의 판정 수단이 확보됐다(세션이 있는 상태의 판정은 블록 7).
- **tick 재개가 3회 연속 관측됐다**: run1 `start_tick=0` → 종료 `last_tick=740` → run2 `start_tick=741` → 종료 `last_tick=1660` → run3 `start_tick=1661`. I-17(뒤로 가지 않는다)이 실행으로 성립.
- 타 프로젝트 컨테이너 10개(`livingfeed-*` 5, `aether-smoke-*` 5)와 그 볼륨은 그대로다. starfall 명명 볼륨은 1개.

**블록 1 집계: PASS 5 / FAIL 0 / 미검증 0**

---

## 블록 2 — 인증·tick 루프·큐·운영 표면 (부하 없음)

증거: `evidence/block2/`

| ID | 결과 | 증거 | 비고 |
|----|------|------|------|
| SC-14 | **PASS** | `bots probe --case auth-ok --label bot-000` → 101 업그레이드, **첫 메시지가 `SESSION_READY`**, `actor_id=01a0b1c2-b010-7000-8000-000000000000`, `actor_id_matches_subject=true`, `tick_hz=20`, `server_version=0.1.0` | I-10: 행위자를 서버가 정한다 |
| SC-15 | **PASS** | **검사 3건 전부 401**: (a) 헤더 없음 → `{"reason":"no_credential"}` (b) 서명 불일치 → `{"reason":"invalid_token"}` (c) 주체가 UUIDv7 아님 → `{"reason":"invalid_token"}`. 시도 전후 `SESSION_OPENED` 행 수 **0 → 0(증가 0)**. `upgrade_rejected_total` = `no_credential:1, invalid_token:2` | I-24. 서버·DB 두 출처가 일치 |
| SC-17 | **PASS** | `cargo test -p starfall-sim --locked` → **8 passed, 0.00s**. 해당 테스트 `simulation::tests::commands_do_nothing_until_a_step_runs`(주석: "`#[tokio::test]` 가 아니다. 런타임도 소켓도 없이 돈다"), `#[test]`로 선언됨. `crates/sim/Cargo.toml`의 `[dependencies]` = `starfall-contracts` **하나뿐** | 아키텍처 전제가 런타임·소켓 없이 증명된다 |
| SC-18 | **PASS** | 표본1 `start_tick=1661 tick=5201 tick_total=3541` identity=True / 표본2 `tick=5203 tick_total=3543` identity=True. **Δtick=2, Δtick_total=2 (같음)** | `tick_total == tick − start_tick + 1`. 항진명제가 아닌 실제 등식 |
| SC-19 | **PASS** | `bots probe --case order --count 12` → sent 12 / results 12 / replies 12, **order_violations=0**, **tick_mismatches=0 (compared 12)** | AC-6(c)의 "같은 tick" 까지 확인. **이 tick 비교 기능은 이 라운드에서 QA 하네스에 추가**했다(없으면 AC-6c 후반부가 판정 불가였다) |
| SC-20 | **PASS** | `bots probe --case inflight --count 600` → **sent=600 == results=600**, `missing_results=0`, `TOO_MANY_IN_FLIGHT: 4`, 연결 유지(봇이 1000으로 정상 종료) | 손실 0. 거부도 응답이다 |
| SC-21 | **PASS** | `bots probe --case duplicate` → sent=2, `accepted 1 / rejected 1`, `rejected_by_reason={"DUPLICATE_COMMAND_ID": 1}`, **replies=1 (2가 아니다)** | AC-7(b) 그대로 |
| SC-29 | **PASS** | PID **57812** 고정. 정상 200 → `docker compose stop postgres` → **503** `{"status":"not_ready","checks":{"postgres":"unavailable","redis":"ok"}}` (`/healthz` 200 = 프로세스 생존) → `start postgres` → **서버 재시작 없이 200 (try 1)**, PID **57812 동일** | 첫 `/readyz`(컨테이너 재생성 직후)도 200이었다 — p0-01 이월 항목 해소 |

**블록 2 집계: PASS 8 / FAIL 0 / 미검증 0**

**QA 하네스 자체 수정 2건** (이 라운드 중, 구현 코드는 손대지 않았다):
1. `close_initiator` 를 "먼저 닫은 쪽"으로 고정(실서버 probe 에서 거짓 `server` 가 나왔다). 안 고쳤으면 SC-52 가 모든 정상 세션에서 거짓 FAIL.
2. envelope `tick` 비교 추가(SC-19 후반부). 없으면 AC-6(c)의 "같은 tick"을 판정할 수 없었다.
두 수정 뒤 봇 자체 테스트 **37건 전부 통과**.

---

## 블록 3 — 프레이밍·수명 주기 (부하와 분리, 게이트 G-f)

증거: `evidence/block3/`. 모든 항목의 **판정 정본은 DB 의 `close_reason`**이고 close code 는 관측되면 함께 적었다.

| ID | 결과 | 증거 | 비고 |
|----|------|------|------|
| SC-24 | **PASS** | `bots probe --case oversize --count 9` (16 KiB 초과 텍스트). `protocol_violations_total` **0 → 9** (9회 전부 계수됨), close code **1002**, DB `close_reason=PROTOCOL_VIOLATION` | **계수됐다는 사실이 AC-8(a)의 핵심**이다 — 라이브러리 한도(64 KiB)가 앱 한도(16 KiB)보다 높아 앱이 셀 기회를 얻었다. 첫 위반에서 tungstenite 가 끊었다면 9가 아니라 1에서 끝났다 |
| SC-25 | **PASS** | `bots probe --case binary --count 9`. `protocol_violations_total` **9 → 18**, close **1002**, DB `close_reason=PROTOCOL_VIOLATION` | 바이너리도 같은 예산 |
| SC-22 | **PASS** | `bots probe --case slow-consumer --count 3000` (수신 완전 중단). DB **`close_reason=SLOW_CONSUMER`**. close code 는 **관측되지 않았다**(`code=None`, `socket error … 10054` = RST) | server 가 예고한 그대로다. 계약 §SC-22 가 "DB 가 정본, code 미관측은 FAIL 아님"으로 미리 정해 둔 항목 — 그 규정이 실제로 쓰였다. `messages_dropped_total=255`, `send_queue_depth_max=131` |
| SC-26 | **PASS** | `bots probe --case idle` (45초 무폴링). close **1001**, DB `close_reason=IDLE_TIMEOUT`. `SESSION_OPENED tick=330450` → `SESSION_CLOSED tick=331051` = **601 tick = 30.05초** | 20 Hz 기준 30초 유휴 창과 정확히 일치. 폴링을 멈춰야 자동 Pong 이 안 나간다(U-9 = 참) |
| SC-28 | **부분 PASS** (SERVER_SHUTDOWN 은 블록 7) | DB 사유 분포: `CLIENT_CLOSED 5 / IDLE_TIMEOUT 1 / PROTOCOL_VIOLATION 2 / SLOW_CONSUMER 1`. 관측 code: CLIENT_CLOSED=1000, IDLE_TIMEOUT=1001, PROTOCOL_VIOLATION=1002, SLOW_CONSUMER=미관측(RST) | ADR-0005 §2 표와 일치 |

블록 3 종료 시점 DB: `SESSION_OPENED 9 / SESSION_CLOSED 9`, sequence 빈틈 **0**, 짝 없는 correlation **0**.

**블록 3 집계: PASS 5 / FAIL 0 / 미검증 0**

---

## 블록 4 — Unity ↔ 실서버 (QA 가 직접 실행)

증거: `evidence/block4/`

client 의 `Live.xml` 은 내 `down -v`(G-a) **이전**에 만들어져 DB 행이 남아 있지 않다. 그래서 **같은 테스트를 QA 가 다시 실행**하고 그 결과로 판정했다(구현자 보고 인용이 아니다).

```
unity test client --mode EditMode --filter "LiveServerTests" (STARFALL_LIVE_TESTS=1)
  -> tests="2" failures="0" skipped="0", exit 0
  -> 실행 전후 SESSION_OPENED 9 → 12 (증가 3 = 이 실행이 만든 세션)
```

| ID | 결과 | 증거 | 비고 |
|----|------|------|------|
| SC-49 | **PASS** | `LiveServerTests.Live_ThreePings_RoundTripInOrder` Passed(내 실행). 첫 프레임이 `SESSION_READY`, `tick_hz=20`, `actor_id=01a0b1c2-7e57-7c11-8e57-000000000001`. client 가 **수신 프레임 배열 인덱스**로 순서를 검증했다(대기 목록이 아니라) | 3 왕복 |
| SC-50 | **PASS** | 내 실행이 만든 세션 3개를 SQL 로 직접 확인: 각 correlation 이 `SESSION_OPENED` 1 + `SESSION_CLOSED` 1, 같은 `session_id`, `close_reason`은 `PROTOCOL_VIOLATION`(의도된 서버 종료 테스트) + **`CLIENT_CLOSED` 2건**. `transport=WEBSOCKET`, `occurred_at`이 GameTime | 도메인 리로드 없었음(`CLIENT_CLOSED` 가 나온 것이 그 증거) |
| SC-51 | **PASS** | `LiveServerTests.Live_ServerInitiatedClose_ReconnectsAsANewSession` Passed. DB: 해당 actor 의 **correlation 3개 / session_id 3개 전부 서로 다름** | 재연결 = 새 세션(I-23) |

**블록 4 집계: PASS 3 / FAIL 0 / 미검증 0**

---

## 블록 5 — 부하 A·B·C (31 연결)

증거: `evidence/load/`, `evidence/block5/`. **31번째 연결은 사람 개입 없이 `unity run … StarfallNetHold.HoldOpen` 으로 만들었다** — `hold.csv` 생성(9초) 확인 후 봇을 시작해 접속 순서(G-j)를 지켰다. 종료는 `RELEASE` touch(프로세스 kill 금지).

### A 단계 — 정상 상태 30 봇 + Unity 1

```
connections_attempted=30  sessions_ready=30  correlations_collected=30   (+ Unity 1 = 31)
sent=3570  results=3570  replies=3570  accepted=3570  rejected=0
missing_results=0  missing_replies=0  duplicate(results/replies)=0/0  unmatched(results/replies)=0/0
probe_seq_mismatches=0  order_violations=0  tick_mismatches=0 (compared 3570)
server_initiated_closes=0  wire_errors=0   -> gates.all_ok = true
```

| ID | 결과 | 증거 | 비고 |
|----|------|------|------|
| SC-52 | **PASS** | 31개 연결 전부 수립(봇 30 `sessions_ready=30` + Unity `ws_connections=1` 확인). **서버가 먼저 닫은 연결 0건** — 봇 관측 `server_initiated_closes=0` + DB 집합의 `close_reason` 전부 `CLIENT_CLOSED`(독립 출처 2개) | |
| SC-53 | **PASS** | **보낸 3570 == 받은 `COMMAND_RESULT` 3570**, 손실 0, 중복 0, 미대응 0 | 검사 건수 3570 |
| SC-54 | **PASS** | `ACCEPTED 3570 == PING_REPLY 3570`, `probe_seq` 불일치 0, 중복 0 | |
| SC-55 | **PASS** | `order_violations=0` / 검사 3570건 (I-15) | 추가로 `tick_mismatches=0`(AC-6c) |
| SC-56 | **PASS** | 3자 대조(아래 표) — 불일치 0 | |
| SC-61 | **PASS** | **A 가 도는 동안** 조회: 집합 31개에 대해 `SESSION_OPENED 31행 / SESSION_CLOSED 0행` | 기록이 종료 시 몰아쓰기가 아님 |
| SC-62 | **PASS** | 마지막 봇 종료 후 호스트 `count(*)` 폴링(200 ms 간격): **첫 표본 t=0.242초에 이미 기대 행 전부(61행)**. 하드 게이트 5초 | 아래 주석 참조 |
| SC-57 | **PASS** | A+B 집합 **181 correlation**: `SESSION_OPENED 181 / SESSION_CLOSED 181` | Unity 종료 후 재측정 |
| SC-58 | **PASS** | 짝 없음 0, 중복 0, DB 에 없는 correlation 0, `actor_id` null 0, 사유 전부 `CLIENT_CLOSED 181` | |
| SC-59 | **PASS** | 스펙 AC-16(c) SQL **0행**. 전 테이블 385행 / 189개 `(world_id,tick)` 그룹 | |
| SC-60 | **PASS** | 집합 361행 전부 GameTime 패턴, 표본 10건 재계산 **불일치 0**. 월드 상수 `tick_hz=20 epoch=3800-01-01T00:00:00Z scale=60` 을 DB 에서 읽어 대조 | 독립 구현(파이썬)으로 재계산 |
| SC-30 | **PASS** | 부하 중 표본: `ws_connections=4 == live_connections=4 == sessions_opened_total − sessions_closed_total (227−223=4)`, 같은 시점 DB 집합 5 opened / 0 closed(비원자적 관측이라 1건 차이). 정지 시점: `ws=0 == 0 == 0`. **출처 확인: server §5 가 `ws_connections`를 "tick 드라이버 라우팅 표의 실제 길이"로 명시**(카운터 뺄셈 아님) | 0==0 만으로는 항진에 가까워 **비-0 표본을 따로 떴다** |
| SC-31 | **부분 PASS — 계약 외 발견 1건** | 부하 창 델타에서 `Δcommands_received(9471) − Δenqueued{COMMAND_RESULT}(9471) = 0` ✓. 회계 항등식 `enqueued_all 20589 = written_all 20334 + dropped 255 + send_queue_depth 0` → **잔차 0** ✓. 다만 **프로세스 누적으로는 `commands_received − enqueued{COMMAND_RESULT} = 66`** | 66은 SC-22(슬로우 컨슈머)에서 연결이 닫힌 뒤 받은 명령분이다. 아래 "계약 외 발견" 참조 |

**SC-62 주석(중요).** `run_block.py` 는 기대 행 수를 `집합×2 = 62`로 잡았는데, 그 시점에 **Unity 세션은 아직 열려 있어 `SESSION_CLOSED` 가 존재할 수 없다**. 스크립트는 62를 기다리다 15.2초에 타임아웃하고 FAIL 을 찍었지만, **폴링 표본을 보면 t=0.242초에 이미 61행(= 봇 30쌍 60행 + Unity OPENED 1행, 그 시점에 존재 가능한 전부)** 이었다. 실패는 **QA 스크립트의 기대값 오류**이고 서버 동작이 아니다. 판정은 표본 원본으로 PASS.

### 3자 대조 (SC-56) — 부하 전체 A+B+C+Unity, correlation 211개

| 양 | 봇 관측 | 서버 메트릭(Δ) | DB 행 |
|----|------:|------:|------:|
| 세션 수(OPENED) | 211 | 210 | 211 |
| 세션 종료 수(CLOSED) | 211 | 211 | 211 |
| 명령 수 | 9,471 | 9,471 | — (기록 대상 아님, ADR-0007 §1) |
| `COMMAND_RESULT` 수 | 9,471 | 9,471 | — |
| `PING_REPLY` 수 | 9,327 | 9,327 | — |
| `SESSION_READY` 수 | 211 | 210 | — |
| 거부 수 | 144 | 144 | — |

**OPENED·SESSION_READY 의 210 vs 211 은 측정 창 문제다**: Unity 가 `stats-before` 스냅샷 **전에** 접속했으므로 그 세션의 OPENED·READY 는 델타 창 밖이고, CLOSED 만 창 안에 있다. 210 = 봇 세션 수와 정확히 일치하고 DB 는 211로 맞다 — **세 출처가 모순되지 않는다.** 명령·응답 경로는 9,471 / 9,471 / 9,327 로 봇과 서버가 **완전 일치**한다.

### B 단계 — 회전

```
sessions_ready=150  connections_attempted=150  sent=450 results=450 replies=450 accepted=450 rejected=0
missing=0  order_violations=0  tick_mismatches=0  server_initiated_closes=0  all_ok=true
rtt p50=19.5ms p99=50.4ms max=60.2ms
```
세션 이벤트 300건이 의도대로 생성됐다(A+B 합계 181 쌍 = 362행).

### C 단계 — 백프레셔 격리

| ID | 결과 | 증거 |
|----|------|------|
| SC-63 | **PASS** | 폭주 봇 `bot-029`: **sent=2000 == results=2000**, `TOO_MANY_IN_FLIGHT 144`, `missing_results=0`, 연결 유지(`close_initiator=client code=1000`) — 거부도 응답이다 |
| SC-64 | **PASS** | 나머지 **29봇: sent=3451 == results=3451, 손실 0**, 끊긴 연결 0 (`server_initiated_closes=0`) |
| SC-65 | **PASS** | A 단계 p99 **50.373 ms** vs C 29봇 p99 **50.350 ms** → **비율 1.000** (게이트 2배 이하). 29봇 표본 3,451건, p50 25.5 ms, max 51.4 ms |

**블록 5 집계: PASS 16 / 부분 PASS 1(SC-31) / FAIL 0**

---

## 블록 6 — D 기록 내구성 (AC-19) · **Phase 0 종료 기준의 직접 실증**

증거: `evidence/durability/`. 봇 30개가 도는 중에 PostgreSQL 을 멈췄다 켰다.

```
중단 구간(UTC) 10:23:06 → 10:23:36 (30초)
D 단계 전체:  sent=11,815  results=11,815  replies=11,815  missing=0  rejected=0
             order_violations=0  tick_mismatches=0  server_initiated_closes=0  all_ok=true
             rtt p50=26.0ms p99=50.35ms max=51.2ms
```

| ID | 결과 | 증거 |
|----|------|------|
| SC-66 | **PASS** | `docker compose stop postgres` → `persist_backlog` 폴링이 **29.4초에 602** 도달(임계 600). 그 상태에서 새 연결 시도 → **503 `{"reason":"recording_backlog","status":"unavailable"}`** (`ws-503-body.txt`) |
| SC-67 | **PASS** | 중단 중 `ws_connections=30` 유지. 봇 쪽 독립 증거: **중단 구간에 보낸 1,770건이 전부 왕복 완료(1,770/1,770 = 100%)**, 끊긴 연결 0 |
| SC-68 | **PASS** | `docker compose start postgres` → pg ready 후 **1.045초에 `persist_backlog=19`** 로 정상 대역(0~20) 복귀 |
| SC-69 | **PASS** | D 단계 correlation **30개 전수**: `SESSION_OPENED 30 / SESSION_CLOSED 30`, 짝 없음 0, DB 에 없는 correlation **0**, 사유 전부 `CLIENT_CLOSED` → **중단 구간 이벤트 유실 0건** |

**이것이 "기록 시스템이 기록을 버리지 않는다"의 실증이다.** DB 가 30초 사라졌는데 (a) 새 연결은 거절되고 (b) 기존 세션은 100% 정상 동작했으며 (c) 복구 후 1초 만에 백로그가 빠졌고 (d) 한 건도 잃지 않았다.

**블록 6 집계: PASS 4 / FAIL 0**

---

## 블록 7 — 정상 종료와 tick 재개

증거: `evidence/block7/`

| ID | 결과 | 증거 |
|----|------|------|
| SC-27 | **PASS** | 봇 6개를 붙여 둔 상태(`ws_connections=6`)에서 stdin `shutdown` → 로그 `tick 루프 종료 — SERVER_SHUTDOWN 스윕 후 영속화 flush tick=348693 sessions_closed=6`. DB: 그 6개 correlation 의 `SESSION_CLOSED` 사유가 **`SERVER_SHUTDOWN` 6건**, 6 opened / 6 closed. **하드 킬 아님** |
| SC-28 | **PASS (완결)** | 최종 사유 분포 `CLIENT_CLOSED 260 / IDLE_TIMEOUT 1 / PROTOCOL_VIOLATION 3 / SERVER_SHUTDOWN 6 / SLOW_CONSUMER 1` — **ADR-0005 §2 표의 5종을 전부 관측**했다. close code: 1000 / 1001 / 1002 / (1001, 종료) / 미관측(RST) |
| SC-09 | **PASS** | 정상 종료 직후 `worlds.last_tick = 348693`, `max(domain_events.tick) = 348693` → `last_tick >= max_tick` 참 |
| SC-10 | **PASS** | 재기동 로그 `last_tick=Some(348693) start_tick=348694`, `/debug/stats.start_tick=348694 > 348693`. **두 실행의 세션 쌍이 각각 온전**: run3 의 종료 세션 1/1, run4 의 새 세션 1/1 |

프로세스 4회 기동 전체에서 tick 은 `0 → 741 → 1661 → 348694` 로 **한 번도 뒤로 가지 않았다**(I-17).

**블록 7 집계: PASS 4 / FAIL 0**

---

## 블록 8 — append-only 탐침 (맨 마지막, 서버 정지 상태 · 게이트 G-i)

증거: `evidence/block8/`. 탐침 규칙대로 `probe_tick = max(tick)+1 = 348999`, `sequence = 0`.

| ID | 결과 | 증거 |
|----|------|------|
| SC-11 | **PASS** | `UPDATE` → `ERROR: domain_events is append-only (UPDATE blocked). 정정은 새 레코드로 한다.` / `DELETE` → `ERROR: … (DELETE blocked) …`. **행 수 545 불변**(시도 전후) |
| SC-12 | **PASS** | 같은 `event_id` 재삽입 → `INSERT 0 0`, 행 수 545 불변 |
| SC-13 | **PASS** | 다른 `event_id` + 같은 `(world_id, tick, sequence)` → `ERROR: duplicate key value violates unique constraint "domain_events_world_id_tick_sequence_key"`, 행 수 불변 |
| SC-59 | **PASS(재확인)** | 탐침 행을 넣은 뒤 sequence 빈틈 검사 재실행 → **545행 / 340 그룹 / 위반 0**. 탐침 규칙(전용 tick + sequence 0)이 자기 검사를 깨뜨리지 않았다 |

최종 DB: `SESSION_OPENED 272 / SESSION_CLOSED 272` (완전 대칭) + 탐침 1행.

**블록 8 집계: PASS 3 / FAIL 0**

---

## 블록 9 — 계약 커버리지와 경계면 (AC-20, AC-21)

증거: `evidence/block0/sc70-coverage.txt`, `evidence/block9/`

| ID | 결과 | 증거 |
|----|------|------|
| SC-70 | **PASS** | `check_contract_coverage.py --strict` → **exit 0, errors 0 / warnings 0**. 6타입 전부 producer·consumer 코드 참조 확인(`bots` 태그 4건 포함). 기준선 8/4 → 0/0 |
| SC-71 | **PASS** | 스키마에서 유도한 **48행**을 Rust·C# 과 대조 → **불일치 0**. 타입별 9 / 11 / 14 / 14. 널 가능 여부까지 3자 일치(좁힌 `actor_id` 는 C# `Guid`+`Always`, 안 좁힌 `causation_id` 는 `Guid?`+`AllowNull`) |
| SC-72 | **PASS** | 정수 4행: `tick`·`sequence`(0..2^53−1 → `long`), `schema_version`(1..2^31−1 → `int`), `payload.tick_hz`(1..1000 → `int`) 전부 **손실 없음**. `tick_hz` 하한을 C# 이 못 막는 것은 §0.5 #10 의 기록된 비대칭 |
| SC-73 | **PASS** | 3층 매트릭스(아래) |
| SC-74 | **PASS** | 유효 fixture 12건 스캔 → `world_id↔tick_hz` 위반 **0**. 스파이크 월드 `…3a4b`=20, `tick-zero.json` 은 별도 월드 `…4c5d`=1 (B-14 수정이 유지되고 있다) |
| SC-23 | **PASS** | `runtime::tests::server_busy_is_structurally_unreachable_at_thirty_connections` **1 passed**(필터 매칭 1건). 부하 실행에서 `commands_rejected_total{SERVER_BUSY} = 0`, `command_queue_depth_max = 0` → **구조적 미도달을 측정으로 확인** |

### SC-73 — 반례 16건 3층 매트릭스

| 층 | 결과 | 검사 건수 |
|----|------|---------|
| ① 스키마 검증 | 16건 전부 거부 | **16** |
| ② Rust serde(운영 경로) | §0.5 표와 전부 일치(16건 거부) | **16** |
| ③ C# `Strict` | 책임 11건 거부 + "감지 불가" 5건 통과를 테스트가 **명시적으로 기록** | **11 + 5** |

§0.5 가 고정한 "C# 책임 11 / 감지 불가 5" 와 EditMode 의 테스트 건수(`Invalid_Rejected_ByStrictProfile` 11, `Invalid_NotDetectableByCSharp_DocumentedAsymmetry` 5)가 **정확히 일치**한다.

**블록 9 집계: PASS 6 / FAIL 0**

---

## 성능 — **판정하지 않음. p1 회귀 기준선으로 고정 기록** (계약 §0.4, 사용자 결정)

측정 환경(§0.7): 서버 **dev 빌드**(release 아님), Unity Editor 가 A 단계 내내 PlayMode 로 접속, 타 프로젝트 컨테이너 **10개**(`livingfeed-*` 5, `aether-smoke-*` 5) 동시 가동, 봇 시드 42, 측정 중 빌드 없음(G-k).

| # | 항목 | 측정치 | 잠정 게이트 | 비고 |
|---|------|-------|-----------|------|
| M-1 | tick 초과 비율 (`run_tick()` **본문** > 50 ms) | **0 / 338,761 = 0.000 %** | ≤ 0.5 % | 본문 소요 기준(루프 간격 아님) |
| M-2 | 단일 tick 본문 최대 | **21.65 ms** (21,652 µs) | ≤ 250 ms | |
| M-3 | 왕복 p99 (봇 시계) | **A 50.373 ms** / B 50.37 / C(29봇) 50.350 / D 50.35 | ≤ 150 ms | 약 1 tick(50 ms)에 붙어 있다 |
| M-4 | `tick_lag_seconds` | **−0.044초** (앞서 있음) | 기록 | 이 PC 바닥값 +0.7 %/분 대비 드리프트 없음 — 전용 OS 스레드 결정(ADR-0006 §2.1)이 유효하다 |
| M-5 | 왕복 p50 / tick 본문 분포 | 왕복 p50 **28.21 ms**(A), min 0.411 / max 50.75. tick 본문 p50·p90·p99 **≤ 100 µs**, 338,013/338,761 tick 이 100 µs 이하 버킷 | 기록 | 이론 하한(평균 반 tick 25 ms)에 근접 |
| M-6 | 큐 깊이 | `command_queue_depth_max` **0**, `send_queue_depth_max` **141** | 기록 | **in-flight 상한이 먼저 걸려 전역 큐가 한 번도 쌓이지 않았다** — "부하가 약해서"가 아니다(ADR-0006 §5) |
| M-7 | 연결 수립·자원 | connect p50 **0.921 ms** / max 4.988 ms, SESSION_READY 까지 p50 **28.03 ms** / max 50.32 ms. 서버 RSS **15.3 → 16.6 MB** | 기록 | 31 연결에서 메모리 증가 미미 |
| M-10 | `SERVER_BUSY` 부하 미도달 | `commands_rejected_total{SERVER_BUSY} = 0` (구조적) | 기록 | 30×64=1920 < 4096 |
| M-11 | U-9(tungstenite 자동 Pong) | **참** — 폴링하면 자동 Pong 이 나간다. 그래서 SC-26 은 45초 무폴링으로 측정 | 기록 | |
| M-12 | C# "감지 불가" 5건 | `command-id-not-v7`, `tick-above-safe-integer`, `unknown-reason-code`, `tick-hz-zero`, `unknown-close-reason` | 기록 | 설계 비대칭, 스키마·Rust 가 서버 경계에서 막는다 |
| M-13 | 손실 창(강제 종료) | **측정하지 않음**(계약대로). 정상 종료 경로의 손실 0 은 SC-27·SC-09 가 확인 | 기록 | outbox 는 p1 |
| M-8 | 측정 환경 | 위 머리말 | 기록 | |
| M-9 | 빌드 시간 | **미기록** — 이번 라운드는 측정 중 빌드 금지(G-k) 때문에 클린 빌드를 재지 않았다. server·client 구현 문서의 값을 쓴다 | 기록 | |

**성능 잠정 게이트 3줄(M-1·M-2·M-3)은 전부 기준을 만족했다.** 사용자 결정에 따라 이는 통과/실패 판정이 아니라 **p1 회귀 기준선**이다.

---

## 요약 (최종)

| 판정 | 수 |
|------|---:|
| **PASS** | **73** |
| 부분 PASS(계약 외 발견 동반) | 1 (SC-31) |
| **FAIL** | **0** |
| 미검증(환경) | 0 |
| 전체 판정 대상 | 74 |

블록별: 0(22) · 1(5) · 2(8) · 3(5) · 4(3) · 5(16) · 6(4) · 7(4) · 8(3) · 9(6) = 74.

**정확성 하드 게이트(AC-15~AC-19 계열: SC-52~69) 18항목 전부 PASS.** 성능은 별도 절에 기록만 했다.

Phase 0 종료 기준("30명 동시 접속 + 실시간 이벤트 기록") 관점:
- **31 연결 동시 유지** — 봇 30 + Unity 1, 서버가 먼저 닫은 연결 0, 명령 3,570건 손실 0 (SC-52~56)
- **실시간 기록** — 연결이 열려 있는 동안 이미 31행이 보였고(SC-61), 종료 후 **0.242초**에 전 행 가시(SC-62), DB 30초 중단에도 **유실 0**(SC-66~69)

## 수정 요청

**없다.** 이 라운드에서 구현(server/client)에 돌린 수정 요청은 0건이다.

## 계약 외 발견 (판정에 넣지 않음 — 리더가 다음 작업으로 판단)

1. **AC-9(c)의 등식은 비정상 종료 세션 뒤에 누적 기준으로 성립하지 않는다.** `commands_received_total − messages_enqueued_total{COMMAND_RESULT}` 가 프로세스 누적으로 **66**이다. 원인은 SC-22(슬로우 컨슈머)에서 **연결이 닫힌 뒤 도착한 명령**이 `commands_received_total` 만 올리고 `COMMAND_RESULT` 를 만들 대상 세션이 없기 때문이다(서버 결함이 아니다 — `messages_dropped_total=255` 로 같이 드러난다). 부하 창 델타로는 `9,471 − 9,471 = 0` 으로 정확히 성립한다. → **AC-9(c) 문구에 "정상 종료된 세션 기준" 또는 "측정 창 델타 기준" 한정이 필요하다.** 담당: architect.
2. **개발 환경의 `.env` 에 `STARFALL_DEV_AUTH_SECRET` 행이 없다.** 그래서 서버를 그냥 띄우면 `/ws` 가 503 `auth_not_configured` 가 된다(SC-16 은 이 덕에 자연스럽게 재현됐지만, 다음 사람은 "봇이 안 붙는다"로 30분을 쓴다). `.env.example` 에는 있다. 담당: 리더/server.
3. **QA 도구 자체 결함 3건을 이 라운드에서 찾아 고쳤다**(QA 소유, 수정 완료):
   - `close_initiator` 가 Close **교환**의 나중 값으로 덮여 정상 종료가 `server` 로 기록됐다 → SC-52 거짓 FAIL 위험. 첫 기록 우선으로 수정 + 회귀 테스트 2건.
   - envelope `tick` 비교가 없어 AC-6(c) 후반부를 판정할 수 없었다 → 추가(`tick_mismatches`/`ticks_compared`).
   - `run_block.py` 의 manifest 직렬화 오류와 SC-62/SC-57 기대값 오류(아직 열려 있는 Unity 세션의 `SESSION_CLOSED` 를 기다렸다) → 수정.
   세 건 모두 **측정 결과가 아니라 측정 도구의 결함**이었고, 실서버 실행이 아니면 드러나지 않았다.

## 다음 라운드

**필요 없다.** FAIL 0, 미검증(환경) 0. 계약 외 발견 1번(AC-9c 문구)은 architect 의 스펙 문구 정리 사항이지 재측정 대상이 아니다.

## 실행 환경 기록

- 서버 프로세스 4회 기동(run1~run4), 전부 stdin `shutdown` 으로 정상 종료. 하드 킬 0회.
- `docker compose down -v` 2회(블록 1 내부) — 전부 레포 루트에서 실행, `name: starfall` 범위. 타 프로젝트 컨테이너 10개와 볼륨 불변 확인.
- 최종 DB: `SESSION_OPENED 272 / SESSION_CLOSED 272` + 탐침 1행, `domain_events_persist_failed_total = 0`.

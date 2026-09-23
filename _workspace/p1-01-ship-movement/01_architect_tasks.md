# p1-01-ship-movement 태스크 분해

- 작성: architect, 2026-09-19 (2026-09-20 server·client 검토 반영해 개정)
- 스펙: `docs/specs/p1-01-ship-movement.md` (**agreed**)
- 결정 기록: `_workspace/p1-01-ship-movement/01_architect_decisions.md` — **먼저 읽을 것**
- ADR: `docs/adr/0009`·`0010`·`0011`·`0012` (proposed) + `docs/adr/0007` §1 개정
- **Phase 2 완료 — 계약·ADR 확정.** 다음은 qa 스프린트 계약 → 구현(Sonnet + TDD).
- 검토: `01_server_spec_review.md`(B-1~B-14), `01_client_spec_review.md`(R1~R8) — **차단 4건 전부 반영됐다.**

## 이번 슬라이스의 작업 방식

**구현자는 TDD로 작업한다.** 각 태스크에 **"먼저 쓸 실패 테스트"**를 한 줄로 지정했다. 규칙:

1. 그 테스트를 먼저 쓰고 **실패하는 것을 확인**한다(빨간불 로그를 구현 요약에 남긴다).
2. 통과시키는 최소 구현을 한다.
3. 정리한다.

빨간불을 건너뛰면 "코드가 틀려도 통과하는 테스트"를 쓰게 된다 — p0-02에서 항진명제 2건이 그렇게 들어왔다(I-25).

**계약이 코드보다 앞서 있다.** 지금 `cargo test`와 Unity EditMode는 **빨간불이 정상**이다. architect 실측 기준선: `check_contract_coverage.py --strict` **errors 14 / warnings 0**. 이 14가 0이 되는 것이 AC-21(a)의 통과 조건이다.

## ⚠ 시작 전에 알아야 할 두 가지

1. **`tools/codegen`이 이 계약에서 멈춘다.** 배열(`'array' is not supported`)과 `data` kind(`kind 'data' is not supported yet`) 둘 다 **예외를 던진다** — 건너뛰기가 아니다. **C1이 끝나기 전에는 어떤 DTO도 생성되지 않는다.** 그래서 C1이 클라이언트 작업의 맨 앞이고, C2~C6 전부의 선행이다.
2. **`data/` 3파일은 이제 전부 스키마를 통과한다**(D-1·D-2·D-3 완료, 3자 독립 확인). 유도값 5종 검산도 통과한다. **S2가 실제 `data/`로 바로 갈 수 있다.**
3. **테스트의 기대 숫자는 `contracts/fixtures/`에서만 온다.** `data/`는 designer 소유이고 살아 있으므로, 거기서 기대값을 읽으면 **조작감을 튜닝하는 순간 테스트가 빨간불**이 된다. `data/`는 **기동 경로 검증(AC-2)에만** 쓴다. (스펙 §11-2의 규칙)
4. **실서버의 `worlds.last_tick = 350280`이다.** 적분·결정성 테스트는 `Simulation::new(world, 0)`으로 메모리에서 돌린다 — tick 0 기준 손계산 기대값은 실서버에서 맞지 않는다.(스펙 §11-3)

## 파일 소유권

한 태스크 = 한 담당자 = 겹치지 않는 경로.

| 경로 | 소유 |
|------|------|
| `contracts/**`, `docs/specs/**`, `docs/adr/**`, `_workspace/p1-01-ship-movement/01_architect_*` | **architect** |
| `server/**` | **server** |
| `client/**`, `tools/codegen/**` | **client** |
| `tools/bots/**`, `tests/e2e/**`, `_workspace/p1-01-ship-movement/02_*`·`04_*`·`evidence/**` | **qa** |
| `docs/design/**`, `data/**` | **designer** |

**경계 주의 2건**

- `tools/`는 `codegen`(client)과 `bots`(qa)로 나뉜다. 서로의 하위 디렉토리를 고치지 않는다.
- `data/**`는 designer 소유다. server와 client는 **읽기만** 한다. 값이 스키마를 어기면 서버가 기동을 거부하고, 고치는 것은 designer다.
  - 클라이언트가 가지는 `data/` **사본**은 `client/` 아래이므로 client 소유다. 원본과의 동일성은 AC-11(f)가 지킨다.

## 선행 관계

```
architect: 계약·스펙·ADR·결정 기록 (완료)
   │
   ├── C1 생성기 확장 ──┬── C2 계약 테스트
   │   (클라이언트 전부의 선행)  ├── C3 예측 코어 ── C4 입력·재조정 ──┐
   │                            └── C5 타 함선 보간 ────────────────┴── C6 그레이박스
   │
   ├── S1 Rust 계약 타입 ── S2 데이터 로딩 ── S3 월드·적분 ── S4 tick 통합 ── S5 브로드캐스트 ── S6 결정성
   │
   └── Q1 스프린트 계약 ── Q2 봇 확장 ── Q3 e2e ── Q4 실행·리포트
                                 (Q2 는 S1·S5 의 실서버가 있어야 끝난다)

designer D-1 (sync-tuning 6건 수정) ──► S2 의 실제 data/ 연결
S6 산출물(입력열 + tick별 스냅샷) ──► C4 의 첫 테스트 자산
```

**S1과 C1은 병렬로 시작할 수 있고, 각자 자기 쪽 전부의 앞에 있다.**

## 클라이언트 (C1이 맨 앞)

| ID | 태스크 | 담당 | 수정 경로 | 선행 | 관련 AC |
|----|-------|------|---------|------|--------|
| **C1** | **생성기 확장 4건** — (1) `maxItems`를 제약 allowlist에 (2) `type: array` + `items` → C# 배열, 원소가 객체면 중첩 클래스 (3) **`BuildProperties`가 `Nested != null`일 때 `CsType`을 우선**하도록 한 줄(놓치면 `ships`가 배열이 아니게 나온다) (4) `kind: data`·`rest`를 **예외 대신 건너뛰고 stdout에 한 줄**. 이어서 DTO 재생성 | client | `tools/codegen/**`, `client/Assets/_Project/Scripts/Contracts/Generated/**` | — | AC-10 |
| **C2** | 계약 EditMode 테스트 확장(유효 26), **스펙 §5.4의 `C# (Strict)` 열을 실측으로 채우기**, 빈 배열/2척(하나는 LINGERING) 스냅샷 왕복, `Runtime` 관용 범위, **`data/` 사본 ↔ 원본 동일성** | client | `client/Assets/_Project/Tests/**` | C1 | AC-11 |
| **C3** | 예측 코어: `double` 벡터·쿼터니언, ADR-0010 §2 12단계, 양자화·역양자화, `data/` 로더. **MonoBehaviour 없음, Unity 타입 없음** | client | `client/Assets/_Project/Scripts/Sim/**`, `client/.../Data/**`(사본) | C1 | AC-12, I-36 |
| **C4** | 입력 수집·양자화·송신(`client_send_hz` 고정), 재조정(위치·속도·자세·**각속도 2종**), 렌더 오프셋 3구간. **`WORLD_SNAPSHOT`은 `ContractDispatch` 트리 경로를 타지 않는다** — 판별자만 확인하고 타입으로 직접 역직렬화한다(Mono 실측: 트리 경로가 가비지 8배). 나머지 타입은 기존 경로 유지 | client | `client/Assets/_Project/Scripts/Flight/**` | C3 | AC-12, AC-13 |
| **C5** | 스냅샷 버퍼, 타 함선 고정 지연 보간(**위치 선형 + 자세 slerp**), 외삽 상한 후 정지, 부재 = 디스폰, `LINGERING` 표시. **R4의 직접 역직렬화 경로를 여기서 만든다** | client | `client/Assets/_Project/Scripts/Remote/**` | C3 | AC-15 |
| **C6** | 그레이박스 씬·함선 프리팹·추적 카메라·**기준 마커 4개**·soft/hard 경계 표시·진단 HUD(속도, 원점 거리, tick, `ack_input_seq`, 예측 오차, 보이는 함선 수, 가장 가까운 함선 거리, 경계 경고) | client | `client/Assets/_Project/Scenes/**`, `Scripts/Greybox/**` | C4, C5 | AC-13, AC-14 |

**먼저 쓸 실패 테스트**

| ID | 첫 테스트 (빨간불부터) |
|----|----------------------|
| C1 | 현재 `contracts/`로 생성기를 돌려 **3단 캐스케이드를 전부 기록**한다 — 첫 실패는 `maxItems`이지 `array`가 아니다(client 실측). 목표는 같은 명령이 종료 코드 0이 되고 `--check`도 0이 되는 것 |
| C2 | fixture 로더가 유효 fixture를 **26건 미만** 발견하면 실패하는 테스트 (현재 12건이므로 즉시 빨간불). 왕복은 **계약 메시지 20건**에 대해 센다 — 데이터 6건은 DTO가 없다 |
| C3 | `contracts/fixtures/SHIP_CLASS/example-scout.json` 값으로 **항등 자세 + `thrust_z_milli=1000` + 20 tick** → 위치의 `z`만 증가하고 그 값이 **S3의 첫 테스트와 같은 기대 정수**임을 단언한다. **두 테스트가 같은 숫자를 쓰는 것이 ADR-0010 규약의 실체다** |
| C4 | S6이 만든 입력열 + tick별 스냅샷을 재생해 **각 스냅샷마다 재조정 직전 위치 오차 ≤ `reconcile_ignore_threshold_m`**를 단언 — 예측 코어 연결이 없어 실패한다. **선회 중 스냅샷을 반드시 포함**: `ω_aim`과 `ω_roll`을 **스냅샷의 두 필드에서 각각** 받아야 통과하고 합에서 분해하면 실패한다 |
| C5 | **`t = 0.25`(비대칭 지점)** 에서, **자세 차가 60° 이상인 합성 쌍**으로 보간을 단언한다. **`t = 0.5`를 쓰면 안 된다** — slerp와 nlerp가 정확히 같은 값을 내는 대칭점이라 선형 보간 + 정규화 구현이 통과한다(client R6). 60°는 실제 스냅샷 쌍(최대 7.5°)에서 나오지 않으므로 **C5가 자산을 합성한다.** 이어서 외삽 상한을 넘기면 **속도가 0이 되는**(정지) 테스트 |
| C6 | 자동 테스트가 아니다. **먼저 만들 관찰**: 전방 입력이 뱃머리 방향으로 가고 마우스 오른쪽이 오른쪽 선회가 되는 것을 녹화로 남긴다(AC-14a). **부호 버그는 테스트가 잡지 못한다** — 규약이 통째로 뒤집혀도 일관되면 전부 통과한다 |

**client가 먼저 답해야 할 것**

- C1의 배열 지원이 중첩 클래스를 올바로 내는가?(U-14) 실패하면 **계약을 바꾸지 말고 architect에게 알린다.**
- **Unity의 `double`이 Rust와 비트 동일한가?(U-15 — 이 슬라이스의 가장 중요한 미확인 사실.)** AC-12가 반증하면 ADR-0010 §3과 ADR-0012 §3을 고쳐야 하므로 **결과가 나오는 즉시 architect에게 알린다. 스스로 임계값을 늘려 통과시키지 않는다.**
- 10 Hz × 15 KiB 스냅샷의 파싱 비용과 프레임당 할당량은?(U-16, p0-02 U-10의 만기)

## 서버

| ID | 태스크 | 담당 | 수정 경로 | 선행 | 관련 AC |
|----|-------|------|---------|------|--------|
| **S1** | 신규 계약 7종의 Rust serde 타입 + 범위 newtype 7종 + 레지스트리 이름 대응표 + 계약 테스트 확장(26/33) | server | `server/crates/contracts/**` | — | AC-9 |
| **S2** | `data/` 3종 로딩과 스키마 준수 검증, **유도값 검산**(`tick_hz/snapshot_hz` 정수, `point_count` ↔ `points_m`, 스폰 ⊂ 경계, 재개 창 ≤ 잔류, soft ≤ hard), 위반 시 **기동 거부** | server | `server/bins/game-server/src/{data.rs,config.rs,main.rs}` | S1 | AC-2 |
| **S3** | 월드 상태 모델(함선 엔티티, `actor_id → ship_id` 표)과 ADR-0010 §2 적분 12단계, 스폰 해시, 양자화·역양자화 | server | `server/crates/sim/src/world/**`, `sim/src/lib.rs` | S1 | AC-4, AC-1 |
| **S4** | tick 통합: 입력 확정(마지막 것 승·이월·거부), 스폰·**잔류·재개**·디스폰과 도메인 이벤트(인과·sequence), 스냅샷 **구조체** 조립 | server | `server/crates/sim/src/{simulation.rs,session.rs}` | S3 | AC-3, AC-5, AC-6, AC-7 |
| **S5** | 게이트웨이: 세션별 스냅샷 브로드캐스트와 **직렬화를 tick 본문 밖으로**, 송신 큐 256→64, 신규 메트릭 6종 | server | `server/crates/gateway/src/{runtime.rs,ws.rs,stats.rs}` | S4 | AC-7(f), AC-18, AC-19 |
| **S6** | 결정성 재생: 파일에 담긴 입력열(함선 3척·600 tick, 추력·선회·브레이크·경계 접촉·잔류 포함)을 **서로 다른 프로세스에서 2회** 돌려 스냅샷 바이트 비교. **산출물 형식은 아래에 확정돼 있다 — C4가 그것을 읽는다** | server | `server/crates/sim/tests/**` | S4 | AC-8 |

**먼저 쓸 실패 테스트**

| ID | 첫 테스트 (빨간불부터) |
|----|----------------------|
| S1 | `contracts::tests`의 fixture 개수 단언을 **유효 26 / 반례 34**로 올린다 — 타입이 없어 컴파일부터 실패하는 것이 시작점이다. 왕복 비교는 **kind별로 나눈다**(와이어는 엄격 `Value`, `data`는 `as_f64()` 정규화 후 정확 비교 — AC-9(a)). `maxItems`는 serde가 모르므로 **역직렬화 후 서버가 검사**한다 |
| S2 | `hard_boundary_radius_m = 20000.1`인 임시 테이블을 로더에 넣으면 `Err`가 나온다는 단위 테스트. 이어서 유도값 5종 각각에 대해 같은 형태로 |
| S3 | 항등 자세 + `thrust_z_milli=1000` + 20 tick → **위치의 `z`만 증가**하고 값이 손계산한 양자화 정수와 같다 (값은 `contracts/fixtures/SHIP_CLASS/example-scout.json`로 고정, **C3와 같은 숫자**) |
| S4 | 같은 세션에 입력 5건을 한 번에 제출하고 1 tick 돌리면 **마지막 1건만 적용**되고 `ack_input_seq`가 그 값이며 `input_superseded_total`이 4 오른다 |
| S5 | 세션 2개가 열린 상태에서 `snapshot_interval_ticks` 만큼 돌리면 `TickOutcome.outbound`에 `WORLD_SNAPSHOT`이 **정확히 2건**이고 각 `controlled_ship_id`가 다르다 |
| S6 | 같은 입력열을 2회 돌린 산출물의 **바이트 동일**을 단언 (처음엔 산출물 생성 경로가 없어 실패한다) |

**server가 먼저 답해야 할 것**

- ADR-0010 §2의 12단계에서 **순서를 바꾸고 싶은 곳**이 있으면 지금 말할 것. 확정 후 바꾸면 클라이언트까지 두 곳이다.
- 스냅샷 31벌을 tick 본문에서 만들면 tick 본문 소요가 얼마나 늘어나는가? 구조체 복제를 공유로 줄일 여지가 있는가? (스펙은 방법을 정하지 않는다 — 경계만 정한다: **직렬화는 tick 밖**)
- `starfall-sim`에 `Vec3`/`Quat`를 손으로 쓸 것인가? **수학 크레이트도 해시 크레이트도 ADR 없이는 들이지 않는다**(결정성 보장을 그 크레이트에 위임하게 된다 — ADR-0010 §3·§4).
- AC-8의 "서로 다른 프로세스 2회"를 이 PC에서 어떻게 돌리는가? 같은 프로세스 2회 실행은 프로세스 의존 순서를 잡지 못한다.
- **U-20**: 데이터 fixture의 실수 왕복이 깨지면 **데이터를 정수로 바꾸지 말고 비교 방식을 고친다**(ADR-0009 §2의 예외는 의도된 것이다).

## QA

| ID | 태스크 | 담당 | 수정 경로 | 선행 | 관련 AC |
|----|-------|------|---------|------|--------|
| **Q1** | 스프린트 계약: AC-1~AC-21을 검사 항목으로 펼치고 **각 항목의 증거 형태**를 미리 정한다. 테스트 불가능한 AC가 있으면 architect에게 알린다 | qa | `_workspace/p1-01-ship-movement/02_qa_sprint_contract.md` | — | 전체 |
| **Q2** | 봇 하네스: `SET_SHIP_CONTROL` 송신(주기 인자화), `WORLD_SNAPSHOT` 수신·검증, **수신 바이트 독립 계측**, 치트 시나리오 7종(필드 주입 2·범위 초과·폭주·`input_seq` 역행·잔류 가로채기·`SESSION_READY` 이전 전송) | qa | `tools/bots/**` | S1, Q1 | AC-6, AC-16, AC-17, AC-18 |
| **Q3** | e2e 스크립트: 인과 조인 검사, **`ship_id` 기준 짝짓기**(correlation 아님), 재개 시 세션 2 : 스폰 1 검사, `event_type` distinct 6종 검사, "A가 움직이면 B가 본다" 대조, 대역폭 집계 | qa | `tests/e2e/**` | Q2 | AC-3, AC-16, AC-20 |
| **Q4** | 31 연결 부하 실행(**B 단계에 잔류·재개 포함**), 대역폭 실측과 ADR-0011 §2 대조, p0-02 성능 기준선 회귀 비교, 평가 리포트 | qa | `_workspace/p1-01-ship-movement/04_*`, `evidence/**` | S5, C4, Q3 | AC-18, AC-19 |
| **Q5** | 계약 커버리지 `--strict` 0/0, 신규 7타입 **필드별** 3자 대조표, **`data/` 실제 3파일 재검증** | qa | `_workspace/p1-01-ship-movement/04_*` | S1, C1 | AC-21 |

**먼저 쓸 실패 테스트**

| ID | 첫 테스트 (빨간불부터) |
|----|----------------------|
| Q2 | 봇이 `WORLD_SNAPSHOT`을 하나도 못 받으면 실패하는 시나리오 — 서버가 아직 안 보내므로 즉시 빨간불 |
| Q3 | `SHIP_SPAWNED.causation_id = SESSION_OPENED.event_id` 조인이 0행이면 실패하는 SQL (현재 두 타입이 없어 실패) |
| Q5 | `check_contract_coverage.py --strict`가 0을 반환하는지 — 현재 errors 14로 빨간불 |

**qa가 먼저 답해야 할 것**

- AC-6(속도 핵)의 판정이 **이 PC에서 무효가 되는 조건**은? 폭주 봇이 자기 CPU나 소켓에 먼저 막히면 "서버가 막았다"와 구분되지 않는다 — **봇이 실제로 보낸 명령 수**를 세야 한다.
- AC-16(c)("A가 본 자기 위치 == B가 본 A의 위치")는 같은 `ships` 배열에서 나오므로 **항진명제에 가깝다.** 진짜 검증은 (a)와 (b)이고, (c)가 잡는 것은 세션별 직렬화 버그 하나다 — **리포트에 그 구분을 적는다**(I-25).
- AC-18(c)의 대역폭을 어디서 재는가? 서버 메트릭(`snapshot_bytes_total`)과 봇 수신 바이트는 **독립 출처**여야 한다.
- p0-02 대조 SQL을 그대로 쓰면 안 된다: **`SHIP_*`는 `ship_id`로 짝짓고**, 재개된 함선은 세션 2개에 스폰 1건이다(I-41).

## S6 산출물 형식 (확정 — C4가 이것을 읽는다)

client·server가 둘 다 먼저 정해 달라고 요청한 항목이다. **JSON Lines, 한 줄 = 한 tick.** 파서를 새로 쓰지 않고 계약 DTO를 그대로 쓴다.

| 파일 | 내용 |
|------|------|
| `initial.json` | 초기 월드 상태: 함선 3척의 `{ship_id, actor_id, ship_class_id, p, v, q, ω_aim, ω_roll}` + `star_system_id` + 시작 tick(0) |
| `inputs.jsonl` | 한 줄 = `{"tick": n, "session": "<uuid>", "payload": { SET_SHIP_CONTROL payload 그대로 }}`. 입력이 없는 tick은 줄이 없다(이월 경로를 타게 한다) |
| `snapshots.jsonl` | 한 줄 = **그 tick의 `WORLD_SNAPSHOT` payload를 직렬화한 것 그대로**. 수신자는 첫 함선 기준 하나만 낸다 |

- **자식 프로세스가 쓰는 것은 운영과 같은 직렬화 경로여야 한다**(server). 테스트 전용 덤프 형식을 쓰면 "스냅샷 바이트가 같다"가 아니라 "덤프가 같다"를 증명하게 된다.
- **입력열은 파일에서 읽는다.** 자식이 입력을 생성하면 입력 생성기까지 결정적이어야 하고, C4가 쓸 자산이 되지 못한다.
- 경로: `server/crates/sim/tests/data/replay/`. **C2~C5가 읽을 때는 복사하지 않고 상대 경로로 참조한다**(사본이 갈라지면 두 팀이 다른 것을 본다).

## `ship_class_id` → `data/ships/*.json` 매핑 (확정)

client가 물은 것이다. **파일 이름이 아니라 파일 안의 `id` 필드가 키다.**

- 서버·클라이언트는 기동 시 `data/ships/*.json`을 전부 읽어 `id → 행`의 색인을 만든다.
- 와이어의 `ship_class_id`는 **그 `id`와 글자 그대로 같다**(lower-kebab, 변환 없음 — ADR-0009 §2).
- 파일 이름을 키로 삼지 않는 이유: designer가 파일을 옮기거나 이름을 바꾸는 순간 **과거 도메인 이벤트의 참조가 끊긴다.** `id`는 역사에 박히는 값이고 파일 이름은 아니다.
- 같은 `id`가 둘 이상 나오면 **기동을 거부**한다(I-38에 추가).

## 후속 태스크 (구현 라운드에서 발생, QA 평가 전에 처리)

| ID | 태스크 | 담당 | 수정 경로 | 관련 |
|----|-------|------|---------|------|
| **S7** | **`SESSION_IN_FLIGHT_LIMIT` 64 → 16** 적용 + 세 상수의 산술 불변식을 **단위 테스트로 고정**(`in_flight × MAX_RESPONSES_PER_COMMAND + SNAPSHOT_HEADROOM ≤ SEND_QUEUE_CAPACITY`, 16×2+16=48 ≤ 64). p0-02 테스트의 **테스트 수준 우회를 제거**하고 원래 형태로 통과하는지 확인 | server | `server/crates/gateway/src/runtime.rs`, 기존 테스트 | ADR-0011 §5.1 |
| **S8** | **S6 재생 fixture에 롤 구간 보강** — 수동 롤 입력 구간 + 오토레벨 작동 구간을 넣어 `ω_aim`과 `ω_roll`이 **동시에 0이 아닌** tick을 만든다. **이것 없이는 AC-12(e)가 미검증이다**(잘못된 분해도 통과한다) | server | `server/crates/sim/tests/data/replay/**` | AC-12(e) |
| **S9** | `world_full` 게이트에 **재개 면제** 적용(`함선 수 ≥ 정원` **그리고** `이 actor가 함선 없음` → 503). 게이트웨이가 읽는 actor 집합은 tick 루프가 갱신하는 읽기 전용 구조 | server | `server/crates/gateway/**` | I-44, ADR-0011 §1 |
| **T-C7** | **계약이 요구하는 소비자 처리를 테스트로 강제**(I-46): 정규화·역양자화 배율·모르는 값 관용 각각에 대해, **그 처리를 빼면 실패하는** EditMode 테스트를 하나씩 둔다. client가 정규화 누락 버그를 실행으로 잡은 사례가 근거다 | client | `client/Assets/_Project/Tests/**` | I-46 |
| **T-A4** | **스폰 밀어내기 재구현은 하지 않는다** — ADR-0010 §4를 구현 쪽 규칙으로 **고쳤다**(내 "바퀴 수" 규칙이 오버플로 스폰을 한 점에 모으는 결함이 있었다). `spawn.rs`의 `choose_spawn_point` 문서 주석에 남은 "architect 확인이 필요하다"를 **확인 완료로 갱신**하고 ADR 문단을 가리키게 한다. 코드 동작은 바꾸지 않으므로 S6 fixture도 그대로다 | server | `server/crates/sim/src/world/spawn.rs`(주석만) | ADR-0010 §4 |
| **T-A3** | **비단위 쿼터니언 유효 fixture 1건 추가** — 지금 fixture의 양자화 자세는 단위에서 2.5e-7밖에 안 벗어나 **정규화를 빼먹어도 어떤 테스트도 깨지지 않는다.** 성분 범위 안에서 노름이 뚜렷하게 1이 아닌 유효 fixture를 넣어 T-C7이 물릴 자리를 만든다. **fixture 수가 26 → 27로 바뀌므로 Rust·Unity의 개수 단언을 함께 고쳐야 한다 — QA 평가가 끝난 뒤에 한다** | architect | `contracts/fixtures/WORLD_SNAPSHOT/**`, 스펙 §5.3 | I-46 |

**진행 상황 (2026-09-20)**: **S7·S8·S9 완료**(테스트 147건 통과, 게이트 3종 OK). S8로 **AC-12(e)의 "미검증" 고정이 해제**됐다. S7이 재확인한 문제로 **S10이 추가됐고, 이것이 평가 전 마지막 차단 항목이다.**

| ID | 태스크 | 담당 | 수정 경로 | 관련 |
|----|-------|------|---------|------|
| **S10** | **tick당 명령 상한 도입** — 게이트웨이가 세션별로 한 tick에 **최대 8건**만 제출한다. 9번째부터는 **제출하지 않고 `COMMAND_RESULT`도 만들지 않으며** `commands_dropped_over_tick_cap_total` 증가 + **그 tick에 대해 프로토콜 위반 1회**(명령당이 아니다). 네 상수의 부등식(`8 × 2 + 16 = 32 ≤ 64`)을 단위 테스트로 고정. **`TOO_MANY_IN_FLIGHT`는 판정을 지연시킨 tick을 주입하는 단위 테스트로 덮고** 리포트에 "부하 실행으로는 미도달(구조적)"으로 적는다 | server | `server/crates/gateway/src/{ws.rs,runtime.rs,stats.rs}` | ADR-0011 §5.2, I-47 |
| **Q6** | **SC-20 재정의** — "in-flight 상한이 연결을 끊지 않는다"를 부하 항목에서 **내린다**(구조적 미도달). 대신 **"큰 burst가 `SLOW_CONSUMER`로 끊기지 않는다"**를 항목으로 두고, 끊겼다면 `close_reason`이 `PROTOCOL_VIOLATION`인지 확인한다. 손실 항등식을 `보낸 수 = COMMAND_RESULT + commands_dropped_over_tick_cap_total`로 갱신 | qa | `_workspace/p1-01-ship-movement/02_qa_sprint_contract.md` | AC-6, I-47 |

**S10 없이 평가에 들어가면 SC-20이 FAIL이다** — server가 완화 형태로 유지한 테스트가 워크스페이스 병렬 실행에서 5회 중 2회 간헐 실패하고, 스프린트 계약 §0이 간헐 실패도 FAIL로 본다. **S10은 그 테스트를 "완화"가 아니라 "불필요"로 만든다**(그 경로가 구조적으로 도달 불가능해진다).

**S7·S8·S9 완료 / S10·Q6은 평가 전에, T-C7·T-A3은 평가 후에.** T-A3은 fixture 개수 단언을 건드려 **돌고 있는 테스트를 깨뜨리므로** 평가 중에 하지 않는다.

## designer와의 접점 (architect가 조정, 리더가 중계)

`docs/specs/p1-01-ship-movement.md` §9.2의 **D-1 ~ D-10**이 정본이다. 구현을 막는 것은 둘뿐이다.

| # | 조치 | 막는 것 |
|---|------|--------|
| **D-1** | `data/movement/sync-tuning.json` 6건 수정 | **서버가 기동하지 않는다**(I-38). S2의 실제 `data/` 연결 |
| **D-2** | `snapshot_hz` 20 → 10, `remote_interp_delay_ms` 100 → 200 | 대역폭 예산 초과(ADR-0011 §2) |



designer가 값을 정하는 범위와 계약이 강제하는 한계:

| designer가 정할 값 | 계약 한계 | 넘으면 |
|---|---|---|
| `hard_boundary_radius_m` | **≤ 20,000** | 서버가 기동하지 않는다. ADR-0009 §3을 고치는 대화가 먼저 |
| `soft_boundary_radius_m` | ≤ 20,000, 그리고 ≤ hard | 스키마 / 기동 검산 |
| `max_speed_mps` | 0 초과 ~ 100,000 | 스키마 거부 |
| `main/reverse/lateral_thrust_mps2`, `brake_decel_mps2`, `assist_*` | 0 ~ 1,000,000 | 스키마 거부 |
| `turn_rate_max_deg_s`, `roll_rate_max_deg_s`, `auto_level_rate_deg_s` | ≤ 3,600 | 스키마 거부 |
| `turn_accel_deg_s2`, `roll_accel_deg_s2`, `turn_gain_deg_s_per_sin_half` | 0 초과 ~ 36,000 | 스키마 거부 |
| 스폰 지점 | 1 ~ 64개, 전부 하드 경계 안, `point_count`와 개수 일치 | 스키마 / 기동 검산 |
| `main_thrust_mps2` | `≥ lateral_thrust_mps2`, `≥ reverse_thrust_mps2` | **기동 검산** (대각선 클램프 상수가 최대값이어야 한다) |
| 함선 클래스 `id` | 파일 간 중복 없음 | **기동 검산** |
| `reconnect_resume_window_seconds` | ≤ `linger_seconds` | 기동 검산 |
| `snapshot_hz` | `tick_hz`를 나누어떨어뜨릴 것 | 기동 검산 |

`contracts/fixtures/SHIP_CLASS/*`·`STAR_SYSTEM/*`·`SYNC_TUNING/*`의 숫자는 **스키마 검증용 예시이며 설계 값이 아니다**(스펙 §5.5).

## 사용자 결정 대기

**없다.** Q1~Q6이 2026-09-19에 전부 확정됐다(결정 기록 §1·§2). 구현 중 새 결정이 필요해지면 architect가 스펙 §9에 추가하고 리더에게 알린다.

# p1-01-ship-movement 스프린트 계약

- 작성: qa, 2026-09-20 (구현 착수 전 / Phase 3)
- 스펙: `docs/specs/p1-01-ship-movement.md` (**agreed — 최종**, AC-1~21, 불변식 I-26~I-41)
- ADR: `docs/adr/0009`(좌표·단위·양자화) · `0010`(결정적 적분) · `0011`(상태 동기화·입력) · `0012`(예측·보정) — 전부 accepted. `0007` §1 기록 범위 표 개정
- 설계 정본: `docs/design/p1-01-ship-movement-design.md` (S-1~S-8, §9 지표) · `data/{ships,world,movement}/**`
- 태스크: `01_architect_tasks.md` (S1~S6 / C1~C6 / Q1~Q5, 태스크마다 "먼저 쓸 실패 테스트")
- 계약 데이터 (qa 실측, 2026-09-20): **스키마 18 / 유효 fixture 26 / 반례 34 / 레지스트리 타입 13(`registry_version: 3`)**
- **7차 개정 후 계약 데이터 (architect 실측, 2026-09-22)**: 스키마 18 / **유효 fixture 27**(`SESSION_CLOSED/superseded.json` 추가) / 반례 34 / 레지스트리 13(`registry_version: 3` 유지). 아래 항목의 "26"은 **27**로 읽는다. C# 쪽 수(27 발견 / 21 왕복 예상)는 client 실측으로 확정한다
- 합의: server ☐ · client ☐ · qa ☑ · architect(참조) ☐ · designer(참조) ☐ — §9 확인란
- 항목 수: **86 + SC-87(라운드 2 신설 — 9차에 표로 편입) + SC-88(7차 신설) + SC-89(9차 신설) = 89** — server 38 / client 19 / qa 25 / 공동 4(SC-26 server·qa, SC-57 client·qa, SC-64·65 qa·client) + 기록 항목 **M-1~M-16**(판정 제외)

**이 문서의 구속력.** Phase 5 평가(`04_qa_report_r{N}.md`)는 이 표의 항목으로만 한다. 스펙 §7의 AC와 designer 설계 문서(스펙 §10이 "설계 수치의 정본"으로 지정)를 실행 가능한 관찰로 옮긴 것이고, **그 둘에 없는 요구는 넣지 않았다.** 평가 중 발견한 그 밖의 문제는 리포트의 "계약 외 발견"에 적고 판정에 쓰지 않는다.

---

## 0. 공통 실행 전제

### 0.1 환경

```bash
export PATH="$HOME/.cargo/bin:$HOME/.dotnet/tools:/c/Users/CHOISOOYEON/AppData/Local/Unity/bin:$PATH"
export STARFALL_DEV_AUTH_SECRET=dev_only_not_a_secret    # .env 에 이 행이 없다 (p0-02 계약 외 발견 2번)
```

- cargo 명령은 `C:\WorkSpace\SpaceHistoric\server`에서, 나머지는 레포 루트에서.
- SQL: `docker compose exec -T postgres psql -U starfall -d starfall -At -F' | ' -c "..."`. HTTP: `curl.exe`.
- **`psql`에서 `select f(x), count(*) ... group by 1`은 되지만 `select a||count(*)`는 "aggregate in GROUP BY"로 실패한다** — p0-02에서 두 번 겪었다. 열을 `-F' | '`로 나눠 받는다.
- 대기는 Bash 전경 `sleep` 대신 `docker compose exec -T postgres pg_isready -t N`, PowerShell `Start-Sleep`, 또는 폴링 스크립트를 쓴다.
- 서버는 stdin에 `shutdown` 한 줄로 정상 종료한다. **하드 킬 금지**(p0-02 SC-27 경로).

### 0.2 Docker와 DB 초기화 — **이번 슬라이스의 첫 게이트** (스펙 §11-1)

이 PC의 `domain_events`에는 **p0-02가 정당하게 남긴 `QA_APPEND_ONLY_PROBE` 1행**이 있다(qa 실측 2026-09-20: `QA_APPEND_ONLY_PROBE 1 / SESSION_CLOSED 272 / SESSION_OPENED 272`). 그대로 두면 AC-20(e)의 전수 `distinct event_type`이 **이번 실행과 무관한 이유로** 5종을 반환한다.

- **기본 경로: 측정 세션 첫 단계에서 레포 루트 `docker compose down -v`**(프로젝트 범위, `name: starfall`). 그러면 AC-20(e)를 전수로 판정할 수 있다.
- **대안 경로**: `down -v` 없이 간다면 SC-80의 쿼리를 **이번 실행의 tick 구간으로 한정**한다. 둘 중 무엇을 썼는지 리포트에 적는다.
- 전역 `system prune`·`volume prune`·프로젝트 밖 `down -v` 금지. 타 프로젝트 컨테이너 10개(`livingfeed-*` 5, `aether-smoke-*` 5)는 건드리지 않는다.
- **`worlds.last_tick = 350280`**(qa 실측). 실서버 tick은 35만대에서 시작한다 — `down -v` 후에는 0부터다. **어느 쪽이든 tick 0 기준 손계산 기대값을 실서버에 쓰지 않는다**(§11-3, 게이트 G-g).

### 0.12 자기 기대값을 다시 쓸 수 있는 게이트 (9차 신설, architect)

> **자기 기대값을 다시 쓸 수 있는 경로를 가진 게이트는, 그 경로가 막혀 있음이 확인되지 않는 한 판정에 쓰지 않는다.**

골든 파일의 `bless`, 스냅샷 테스트의 `--update`, 기대값 자동 갱신이 전부 여기 해당한다. **덮어쓰면 "테스트가 자기 기대값을 다시 쓴 것"과 "원본이 돌아온 것"이 구분되지 않는다** — 그 순간 그 게이트는 무엇을 재는지 말할 수 없게 된다(SC-87 의 `STARFALL_REPLAY_BLESS` 가 이 규칙의 첫 사례다).

검출력을 확인할 때는 **백업에서 복원**하고, 갱신 경로를 쓰지 않는다.

### 0.3 증거 기준

| 판정 | 조건 |
|------|------|
| **PASS** | 명령과 출력 요약(종료 코드 포함), 테스트 이름, 또는 파일:라인이 리포트에 있다. 정적 읽기만으로는 PASS가 아니다 |
| **FAIL** | 기대 관찰이 나오지 않음. **간헐 실패도 FAIL**(비결정성 이슈로 기록, 재시도로 덮지 않는다) |
| **미검증(환경)** | §6의 E-조건. PASS로 올리지 않는다 |
| **미검증(증거 요건)**<br>*(9차 신설)* | **관찰은 수행됐고 기대와 어긋나지 않았으나, 그 항목이 요구한 증거가 성립하지 않는다**(예: 관찰이 항목의 일부 조항만 덮었다, 영상이 항목이 명시한 실격 조건에 걸린다). **① 이것은 비-통과다 — 슬라이스 종료를 FAIL 과 똑같이 막는다.** 바꾸는 것은 **귀속**(제품 결함이 아니라 증거의 결함)이지 게이트 강도가 아니다. **② 실패를 이 칸으로 옮기는 것은 금지한다** — R3 판정 1의 "간헐 실패도 FAIL"은 **테스트가 실패한** 경우이고 이 칸은 **테스트가 유효하게 실행되지 않은** 경우다. 둘을 섞으면 그 예외가 무너진다 |
| **대기** | 선행 태스크 미완(E9). 그 라운드 판정에서 제외 |
| **기록** | 판정하지 않고 사실만 남긴다 (§7의 M-1~M-16) |

**`미검증(증거 요건)` 을 오용으로부터 지키는 문장 (9차, 리포트 요약에 함께 싣는다).** 이 칸을 쓴 라운드의 요약에는 다음을 적는다:

> **FAIL 수가 0 이 된 것이 제품이 나아져서가 아니라 귀속이 정확해졌기 때문이라면 그렇게 적는다 — 막고 있는 게이트 수는 줄지 않았다.**

이 문장이 없으면 집계를 읽는 사람이 "FAIL 0 = 진행 가능"으로 읽는다. 칸을 나눈 목적은 **원인을 정확히 귀속하는 것**이지 게이트를 무르는 것이 아니다.

**검사 건수 원칙.** 순회·집합 항목은 **실제로 순회한 개수**가 증거에 드러나야 한다. Unity는 리포트의 `tests` 수, cargo는 `N passed`, 부하는 correlation/ship_id 집합 크기, 스냅샷 대조는 **조인된 행 수**. 개수가 없으면 "검증기가 꺼진 채 0건 통과"와 구분되지 않아 PASS로 인정하지 않는다.

**구현이 없어서 실행 못 한 것은 FAIL이다.** 환경 문제(§6)와 섞지 않는다.

### 0.4 하드 게이트와 기록의 분리

| 구분 | 대상 | 판정 |
|------|------|------|
| **정확성 하드 게이트** | SC-01 ~ SC-86 전부 | PASS / FAIL / 미검증(환경) |
| **성능** | **판정하는 것은 하나뿐: SC-75 `tick 초과 비율 ≤ 0.5 %`** | 나머지는 M-1~M-6 |
| **기록만** | M-1 ~ M-16 | 값이 안 나와도 FAIL 아님 |

**성능의 관심사가 p0-02와 다르다.** p0-02는 "첫 측정치를 기준선으로 고정"이었고, 이번은 **회귀 여부**다. 게임 로직과 31벌 브로드캐스트가 들어갔으므로 **tick 본문 소요가 늘어나는 것은 회귀가 아니라 예상**이다(AC-19). p0-02 기준선: **초과 0.000 % / 본문 최대 21.65 ms / 왕복 p99 50.4 ms / RSS 16.6 MB / `command_queue_depth_max` 0 / `send_queue_depth_max` 141**.

**`tick_body_us`를 p0-02와 직접 비교하지 않는다**(architect 지시 6). 스냅샷이 2 tick마다라 본문 소요가 **이봉분포**가 되어 p50은 낮은 쪽만, p99는 높은 쪽만 본다. **`snapshot_build_us`를 분리해 "스냅샷 조립 X µs / 나머지 Y µs"로 적는다.**

### 0.5 반례 34건의 층별 거부 책임 (SC-37 / SC-38 / SC-48의 채점 기준)

**스펙 §5.4 표가 정본이다.** 구현 결과가 표와 다르면 표를 고치지 말고 architect에게 알린다(FAIL이 아니라 **계약 설계 변경 통지** — p0-02 SC-44와 같은 처리).

| 층 | 기대 | 검사 건수 |
|----|------|---------|
| ① 스키마 검증 | **34건 전부 거부** | 34 |
| ② Rust serde(운영 경로) | §5.4 + 선행 스펙 표의 "Rust serde" 열과 **전부 일치** | 34 |
| ③ C# `Strict` | **거부 18 / 통과(감지 불가) 10 / 계층 없음(데이터 3종의 6건) 6 = 34** | 18 + 10 (+6 대상 외) |

- ③의 숫자는 **client가 현재 트리로 34건 전수 재측정한 값**이다(2026-09-20 ack §0). **B-1이 `angular_velocity_roll_mdeg_s`를 추가하면서 34번째 반례 `WORLD_SNAPSHOT/invalid/angular-velocity-roll-missing.json`이 생겼고, 그것은 데이터가 아니라 메시지 타입이라 C#이 거부한다** — 그래서 거부가 17 → **18**이다. 18 + 10 + 6 = 34.
- "감지 불가" 10건은 **판정 대상이 아니라 기록**(M-11)이다. 통과하는 것이 기록된 설계 결과다.
- **계층 없음 6건은 데이터 3종 × 2건**이다: `SHIP_CLASS/invalid/{movement-unknown-field, turn-gain-zero}`, `STAR_SYSTEM/invalid/{hard-radius-above-ceiling, no-spawn-points}`, `SYNC_TUNING/invalid/{integrator-is-not-a-tunable, snapshot-hz-zero}`. C# DTO가 없는 것이 정상이고(AC-10(d)) 누락이 아니다.

### 0.6 짝짓기 기준이 p0-02와 다르다 — **SQL을 그대로 재사용하면 조용히 틀린다**

| 대상 | 짝짓기 키 | 근거 |
|------|---------|------|
| `SESSION_OPENED` ↔ `SESSION_CLOSED` | **`correlation_id`** (p0-02 그대로) | AC-20(b) |
| `SHIP_SPAWNED` ↔ `SHIP_DESPAWNED` | **`ship_id`** | I-41, AC-20(a) |

**`SHIP_*`를 `correlation_id`로 짝지으면 맞지 않는다.** 한 함선이 여러 세션을 거칠 수 있고, 두 이벤트의 `correlation_id`는 각각 **그 이벤트를 일으킨 세션**을 가리킨다(I-41). **재개된 함선은 세션 2개에 스폰 1건**이다 — 그 비대칭 자체가 SC-78의 판정 대상이다.

### 0.7 기대 숫자의 출처 — **`data/`에서 읽지 않는다** (스펙 §11-2)

- 단위·통합 테스트의 기대 숫자는 **`contracts/fixtures/`에서만** 온다.
- `data/`는 **기동 경로 검증(AC-2)과 실서버 관측 해석에만** 쓴다.
- 이유: `data/`는 designer 소유이고 살아 있다. 거기서 기대값을 읽으면 **조작감을 튜닝하는 순간 테스트가 빨간불**이 된다. "재미를 고치면 테스트가 깨진다"는 최악의 결합이다.
- 실서버 관측에서 `data/` 값을 참조해야 할 때(예: `max_speed_mps`로 이동 거리 상한 설명)는 **그 시점의 `data/` 값을 증거에 함께 적는다** — 값이 바뀌면 리포트가 스스로 설명하게 한다.

### 0.8 측정 환경과 빌드 금지 (스펙 §11-5·6)

- **측정 중 어떤 빌드도 돌리지 않는다**(`cargo`·Unity 임포트·`dotnet run`). p0-02에서 같은 측정이 14.9초 → 0.079초로 왜곡됐다. **이번엔 AC-8이 자식 프로세스를 띄워 CPU 경합이 더 크다.**
- 리포트에 반드시 적는다: Unity Editor 실행 여부와 상태, 동시 컨테이너 수, 봇 시드, 서버 빌드 프로필(dev/release), 단계별 시각.
- `tick_lag_seconds` 옆에 **Windows 타이머 바닥값 +0.7 %/분**을 나란히 적는다.

### 0.9 구현 전 기준선 — **판정에 쓰지 않는다**

빨간불이 정상이다(계약이 코드보다 앞서 있다). qa가 직접 확인한 값:

| 항목 | 기준선 | 확인 |
|------|--------|------|
| 계약 커버리지 | **types 13 / errors 14 / warnings 0**, `RESULT: FAIL` | qa 실행 2026-09-20 (architect·server·client 3자와 일치) |
| 계약 파일 수 | 스키마 **18** / 유효 **26** / 반례 **34** | qa `find` 실행 |
| DB | `QA_APPEND_ONLY_PROBE 1 / SESSION_* 272쌍`, `worlds.last_tick = 350280` | qa 실행 |
| `cargo test` | 빨간불(신규 타입 없음) | server 검토 |
| Unity EditMode | 빨간불(fixture 26 기대 vs 현재) | client 검토 |
| `tools/codegen` | **3단 캐스케이드로 종료 코드 2** (`maxItems` → `array` → `kind 'data'`) | client 실측, AC-10(a)가 기록 대상으로 삼는다 |

### 0.10 이 슬라이스가 **증명하지 못하는 것** (스펙 §11-7 — 리포트에 반드시 적는다)

**손 방향 부호가 통째로 뒤집혀도 자동 검증은 전부 통과한다.** 서버와 클라이언트가 같은 공식을 쓰므로 예측 오차 0, fixture 왕복 통과, 결정성 바이트 동일까지 전부 초록이다. 특히 오토레벨의 `sin_err` 부호 한 줄이 그 위험을 진다(ADR-0010 §2.1).

**검출기는 SC-59(AC-14 a·d)의 육안 관찰 하나뿐이다.** 그것을 사람이 실제로 볼 때까지 이 슬라이스는 부호에 대해 **아무것도 증명하지 못한다.** 리포트의 요약에 이 문장을 그대로 싣는다.

### 0.11 두 번째 관측자(B)는 **봇으로 대체하지 않는다**

SC-64·65는 "A의 화면과 B의 화면이 같은 것을 보는가"를 재고, SC-65의 기대값은 **B가 보간 지연 200 ms만큼 과거를 그린다**는 데서 나온다(뒤쪽 +28 m).

**봇을 B로 쓰면 부호가 뒤집힌다.** 봇에는 보간이 없어 스냅샷의 서버 진실을 갖고, A는 예측으로 앞서 있다 → 차이가 **앞쪽 0~7 m**가 된다. SC-65는 "앞쪽이면 외삽 과다"로 채점하므로 **정상 동작을 버그로 읽는다**(client ack §⑥).

**채택하는 형태 (client 제안):** **한 Unity 프로세스 안에 세션 2개 + 관측자 파이프라인 2개.** 각자 다른 `actor_id`(`STARFALL_DEV_ACTOR_SUBJECT`로 분리), 자기 소켓·버퍼·CSV. 보간 코드는 Unity 타입이 없는 순수 C#이므로(C3·C5 지시) **Editor 인스턴스 2개가 필요 없다.** A를 사람이 조종할 때는 A가 대화형 Editor, B가 같은 프로세스의 두 번째 세션이다.

- **재현되지 않는 것은 프로세스 격리(GC·렌더 타이밍)뿐이고 SC-64·65는 그것을 재지 않는다.**
- 이 경로가 막히면: **SC-64만 봇으로 살리고 SC-65는 미검증(환경, E8)** 으로 둔다. **뒤집힌 부호로 PASS를 찍는 것보다 정직하다.**
- 봇 주체 30개와의 disjoint 확인은 p0-02 방식(`bots identities --disjoint-from`) 그대로.

---

## 1. 검증 항목

### A. 서버 게이트와 결정성 위생 (AC-1) — server

| ID | 검증 항목 | 검증 방법 | 담당 | 근거 AC | 불가 |
|----|----------|----------|------|--------|------|
| SC-01 | `cargo fmt --all --check` / `cargo clippy --workspace --all-targets -- -D warnings` / `cargo test --workspace --locked` 세 명령 전부 종료 코드 0, 실패 0. **실행된 테스트 수를 적는다**. **(8차) 넷째 명령 `cargo test -p starfall-sim --release --locked` 도 종료 코드 0, 실패 0 이고 그 출력에 `#[cfg(not(debug_assertions))]` 테스트(현재 `r4_s3_release_arm_signals_without_fabricating_an_event`)가 `ok` 로 1건 이상 찍힌다** — 디버그 게이트에서는 릴리스 전용 테스트가 **컴파일조차 되지 않고**, 디버그/릴리스 팔이 1:1 로 자리를 바꿔 총 수(167)로는 안 보인다(§7a) | 세 명령을 그대로 + 넷째 명령. p0-02 기준 82 passed → 이번 수를 나란히 | server | AC-1 | E1 |
| SC-02 | `starfall-sim`의 `Cargo.toml`에 `axum`·`sqlx`·`redis`·`rand`·`chrono`·`tokio`가 **여전히 없고**, 수학·해시 크레이트도 들어오지 않았다 | `grep -nE "axum\|sqlx\|redis\|rand\|chrono\|tokio\|hash\|glam\|nalgebra" server/crates/sim/Cargo.toml` → 매칭 0. `[dependencies]` 전문을 증거에 | server | AC-1, ADR-0010 §4 | E1 |
| SC-03 | `starfall-sim` 소스에 ADR-0010 §3의 금지 함수 호출이 **0건**. **두 줄을 나란히 보인다**: 순진한 grep이 **오탐 13건**(`.expect(`가 `exp`에 걸린다), 메서드 호출 형태로 좁히면 **0건** | server가 `03_server_impl.md`에 두 명령과 두 결과를 적는다. 금지 목록에 **`mul_add`(FMA)·`hypot`·`to_radians`/`to_degrees`·`signum`** 포함 확인 | server | AC-1 | E1 |
| SC-04 | 결정성 위생 보조: `sim` 안에 `SystemTime`/`Instant` 사용 0건, `HashMap` 순회 의존 0건(함선 순회가 `ship_id` 순) | grep 2종 + 순회 지점의 파일:라인 | server | AC-1, I-37 | E1 |

### B. 데이터 테이블과 기동 거부 (AC-2) — server

| ID | 검증 항목 | 검증 방법 | 담당 | 근거 AC | 불가 |
|----|----------|----------|------|--------|------|
| SC-05 | 정상 `data/` 3파일로 기동하면 `/debug/stats`에 **로드된 함선 클래스 수·스폰 지점 수·유도된 `snapshot_interval_ticks`**가 있고 파일과 일치한다 | 기동 후 `curl.exe -s .../debug/stats`. 현재 `data/` 기준 기대: 클래스 1, 스폰 지점 12, `snapshot_interval_ticks = 20/10 = 2`. **그 시점 `data/` 값을 증거에 함께 적는다**(§0.7) | server | AC-2(a) | E2, E5 |
| SC-06 | **기동 거부 8종이 전부 실패하고 로그에 파일·필드·기대값이 나온다** — 검사 8건: ① `hard_boundary_radius_m = 20000.1` ② 스폰 `points_m` 빈 배열 ③ `point_count ≠ len(points_m)` ④ `tick_hz / snapshot_hz`가 정수 아님 ⑤ `reconnect_resume_window_seconds > linger_seconds` ⑥ 스폰 지점이 하드 경계 밖 ⑦ `main_thrust_mps2 < lateral` 또는 `< reverse` ⑧ 함선 클래스 `id` 중복 | **`STARFALL_DATA_DIR`**(server가 S2에서 추가 확정)로 임시 사본을 가리키고 한 파일만 어긴다 → **종료 코드 1** + 로그 `ERROR 기동 거부: 데이터 검증 실패 file=<절대경로> field=<JSON Pointer> expected=<제약> actual=<값> rule=<schema\|derived>`. **①②는 `rule=schema`, ③~⑧은 `rule=derived`**. **`data/` 원본은 건드리지 않는다**(designer 소유). 디렉토리를 못 찾으면 **찾은 경로를 로그에 찍고 기동 거부**한다 | server | AC-2(b~g), 태스크 §D-12, I-38 | E5 |
| SC-07 | **실제 `data/` 3파일이 계약 스키마를 통과한다**(재확인) | `python` 오프라인 검증기로 `data/ships/*.json`·`data/world/systems/*.json`·`data/movement/sync-tuning.json`을 각 스키마에 대해 검증 → 오류 0. architect 실측(2026-09-19)은 `sync-tuning.json` 6건 실패였고 D-1로 해소됐다 — **회귀 여부를 QA가 독립으로 본다** | qa | AC-21(d), §5.6 | — |

### C. 스폰·잔류·재개·디스폰과 인과 (AC-3) — server

**짝짓기는 `ship_id`다**(§0.6).

| ID | 검증 항목 | 검증 방법 | 담당 | 근거 AC | 불가 |
|----|----------|----------|------|--------|------|
| SC-08 | 세션 1개를 열면 그 `correlation_id`에 `SESSION_OPENED` 1건 + `SHIP_SPAWNED` 1건이고, **`SHIP_SPAWNED.causation_id = SESSION_OPENED.event_id`** | SQL 자기 조인(`tests/e2e/check_causation.py`). **조인 결과 행 수를 적는다** | server | AC-3(a), I-30 | E2, E5 |
| SC-09 | 원인의 `(tick, sequence)`가 결과보다 **작다** | 같은 조인에서 두 쌍의 `(tick, sequence)` 비교. 위반 0행 | server | AC-3(b) | E2 |
| SC-10 | `SHIP_SPAWNED`의 위치가 `data/`의 어느 스폰 지점과 **정확히 일치**하고, **잔류 만료 후**(> `linger_seconds`) 같은 `actor_id`로 재접속하면 새 `SHIP_SPAWNED`의 위치가 **첫 번째와 같다**(난수 아님) | 스폰 위치(mm 정수)를 `data/`의 `points_m` × 1000과 대조. 두 번째 스폰까지 **30초 + 여유 대기**(게이트 G-j). *30초 안의 재접속은 스폰이 아니라 재개이므로 이 항목으로 재지 않는다 — 그렇게 재면 항진명제가 된다(AC-3 주석)* | server | AC-3(c), ADR-0010 §4 | E2, E5 |
| SC-11 | 잔류 창 **안**에 재접속하면 (1) `ship_id`가 같고 (2) `SHIP_SPAWNED`가 **추가로 발행되지 않으며** (3) 재개 직후 스냅샷 T1의 14필드와 **잔류 중 관측자 행 전부**가, 끊기기 직전 스냅샷 T0의 각 정수를 ±0.5 양자 범위에서 흔든 시작점 집합(≥ 64)에서 [남은 이월 → 휴면 입력]으로 N tick을 독립 적분한 결과의 [min, max] **봉투 안**에 든다(qa 독립 계산, 하네스 = client C# 적분기). **음성 대조**(휴면 모델을 틀리게 한 계산)가 봉투를 벗어나야 대조가 유효하다. 봉투는 표본으로 만든 과소 추정이므로, 봉투 밖 1 양자 이내의 FAIL은 표본을 늘려 재판정한다. (3′) **비트 일치는 `f64` 층에서** 본다: server의 AC-3(d1) in-process 차분 테스트 출력(비교 tick 수 포함). *(7차 개정 — 옛 문구 "끊기기 직전 스냅샷 상태에 추력 0으로 N tick 적분한 값과 양자화 정수로 같다"는 출발점이 스냅샷이라 성립할 수 없었다. qa R3 §5.6·§6.5, architect R3 추가 판정 사안 2)* | 봇이 끊고 재접속. 두 스냅샷을 CSV로 남기고 **적분 예측은 qa가 독립 계산**(서버가 계산한 값을 서버가 확인하면 I-25 위반) — `bots resume` + `tests/e2e/resume_check.py`(하네스 `tests/e2e/resume_predict/`). **허용 오차를 고르지 않는다 — 봉투는 양자화에서 유도한다**(ADR-0010 §3 '보장의 입력'). **독립 계산의 전제(server ack §1-③ 확정)**: 잔류 중 입력은 **휴면 입력**(`flight_assist=true`, `brake=false`, 추력·롤 0, 목표 자세 = 현재 자세)이고 — **잔류 중에도 ADR-0010 §2의 5단계 오토레벨이 계속 돈다.** 따라서 `q`·`ω_roll`은 "그대로 두면 되는 값"이 아니며 **독립 계산에 5단계를 반드시 포함**한다. 살아 있는 경로는 1·2(불감대)·3(슬루)·**5(오토레벨)**·7(경계)·9(감쇠)·10~12뿐이다 | server | AC-3(d), I-29 | E2, E6 |
| SC-12 | 재접속하지 않으면 `linger_seconds` 뒤 `SHIP_DESPAWNED{LINGER_EXPIRED}` 1건이 나오고 **`causation_id`가 그 잔류를 시작시킨 `SESSION_CLOSED.event_id`** | 30초 + 여유 대기 후 SQL 조인. **원인 이벤트의 tick이 결과보다 수백 tick 앞선다는 것을 증거에 적는다** — 상관과 인과가 서로 다른 시각을 가리키는 첫 데이터다 | server | AC-3(e), I-30 | E2, E5 |
| SC-13 | 정상 종료 시 **(a) 잔류 함선과 (b) 활성 함선이 둘 다** `SHIP_DESPAWNED{SERVER_SHUTDOWN}`으로 디스폰된다(I-41은 프로세스 경계에서도 성립해야 한다). **(b)의 `causation_id`는 같은 tick에 발행된 그 세션의 `SESSION_CLOSED.event_id`이고 `sequence`가 그보다 크다** | 잔류 1척 + 활성 1척을 둔 상태에서 stdin `shutdown` → DB 확인. (b)의 순서는 **SC-09와 같은 조인**으로 검사한다. 두 경우의 건수를 각각 적는다 | server | AC-3(f), I-41, server ack §2 | E2, E5 |
| SC-14 | 전 구간에서 **`ship_id`별 `SHIP_SPAWNED` 1건 / `SHIP_DESPAWNED` 1건** | `tests/e2e/check_ship_pairs.py`(ship_id 기준). 짝 없음 0, 중복 0, 검사한 `ship_id` 수를 적는다 | server | AC-3(g), I-41 | E2 |
| SC-88 | **같은 actor 의 동시 세션(I-29, 사용자 결정 5 = (B) 나중 접속이 이어받는다).** 같은 토큰으로 세션 S1 을 열고, S1 이 열려 있는 동안 S2 를 연다. (a) **어느 시점에도 그 actor 의 함선은 1척** — 함선 수명 구간 [스폰, 디스폰) 의 겹침 0, S2 는 **새 `SHIP_SPAWNED` 없이 S1 의 함선을 이어받는다**(`controlled_ship_id` 동일) (b) 같은 tick 에 `SESSION_OPENED(S2)` 가 sequence k, **`SESSION_CLOSED(S1, close_reason = SUPERSEDED)`** 가 k+1 이고 **그 `causation_id` = `SESSION_OPENED(S2).event_id`** (c) S1 의 연결은 **close code 4001** 로 닫힌다(서버가 보낸 Close 프레임의 code) (d) **밀려난 S1 쪽 클라이언트가 재접속하지 않는다** — S1 닫힘 뒤 관측 창(≥ 10 s, 백오프 상한) 동안 그 actor 의 `SESSION_OPENED` 가 S2 말고는 없다. **(d) 는 S1 이 실제 클라이언트(Unity `RealtimeClient`)여야 한다** — 봇은 원래 재접속하지 않으므로 봇을 S1 로 두면 항진명제다(§7a). 블록 6 에서 Unity S1 + 봇 S2 로 재고, client 의 grep 가능한 로그 한 줄을 함께 증거로 둔다 (f) **다른 사유의 `SESSION_CLOSED` 원인은 null**(SC-81 세션 간선과 같은 검사). architect 의 넘겨받기 단언 ①~⑤ 와의 대응: ① = (b) 순서, ② = (b) 원인·상관 조인, ③ = (a) 스폰 없음 + `controlled_ship_id` 동일, ④ = (c), ⑤ = (f) (e) 검사 구간 전체에서 **`causation_id = event_id` 인 행 0건**, 모든 세션을 닫고 잔류 창이 지나면 **`ships_active = 0`**(조종 세션 없는 `ACTIVE` 0), 그 상태로 종료하면 `SHIP_DESPAWNED{SERVER_SHUTDOWN}` 전부의 원인이 **실제 `SESSION_CLOSED`** | `tests/e2e/concurrent_session.py run`(떠 있는 서버에 라운드 A·B) → 서버 정상 종료 → `concurrent_session.py check` + `ship_events.py overlap` + `ledger --from-tick <run_from_tick>`. 절차: 봇 2개(같은 라벨) 겹침 접속 → `/debug/stats` 게이지(`ws_connections`·`ships_active`)를 S2 전·후·양쪽 닫힘 뒤·잔류 창 뒤에 스냅 → 종료 → `tests/e2e/ship_events.py overlap`(수명 구간 겹침) + `causation`(타입 + ≠ self + 순서 — SC-81 과 같은 검사) + (b) 는 그 tick 의 행을 SQL 로 나란히 출력. (c)(d) 는 **S1 쪽 봇이 받은 Close code** 와 그 actor 의 `SESSION_OPENED` 수. **판정 도구의 양방향 자기 검증을 증거에 싣는다**: `overlap` 은 알려진 위반 구간(qa R3 §6.7 재현, tick 427105~430030 의 actor `…0044`)에서 **FAIL**, 블록 5 구간(403435~427104)에서 **PASS** 를 내야 한다. **`pairs` 의 `actors_with_more_than_one_ship` 은 순차와 동시를 가르지 못하므로 이 판정에 쓰지 않는다.** server 의 in-process RED(수정 전 코드에서 실패)는 스펙 AC-3(h) 의 server 몫이고 이 항목은 **실서버 관측**이다. RED 증거는 qa R3 §6.7 의 GREEN 재현(수정 전 바이너리)이 그대로 쓰인다 | server·qa | AC-3(h), I-29, I-30, I-40 | E2, E5 |

| SC-89 | **클라이언트의 송신 속도가 프레임률이 아니라 벽시계 tick 속도를 따른다** (architect R4 판정 §10 + R4 보충 §F). **(a) 송신 쪽** — 프레임 히치의 **길이와 무관하게** 한 프레임에 `SET_SHIP_CONTROL` 이 **2건 이상 나가지 않는다.** **＋ 문턱 바로 아래를 통과로 읽지 않기 위해**(9차): 그 구간의 `commands_dropped_over_tick_cap_total` 과 `RATE_LIMITED` **델타가 0이다.** *`close_reason` 0건만 보면 끊김은 예산 8이 찬 뒤에야 일어나므로 **위반이 7회 쌓여도 초록**이다.* **(b) 재조정 쪽** — 합성 히치 뒤 **`catchup_carry_forward_ticks_total > 0` 이면서 `reconcile_hard_snap_total == 0`.** **두 단언은 반드시 짝으로 본다**(§7a). **＋ 히치가 실제로 있었음의 단언**(9차): **한 `Update()` 최대 드레인 tick 수 > 1.** *`commands_received_total > 0` 은 "세션이 살아 있었다"만 잰다. **드레인 max 가 1이면 히치가 한 번도 없었다는 뜻이고 그 세션은 이 항목을 닫지 못한다.*** **＋ 판별 단언(9차 보충, architect)**: **같은 창에서 프레임당 최대 송신 건수 == 1.** **⚠ 이것이 없으면 수정 전 바이너리도 통과한다**: Editor 가 백그라운드에서 **throttle 이 아니라 pause** 하면 복귀 프레임 **하나**가 거대하게 드레인해 **위반 1회**만 나고, 예산 8에 못 미쳐 **끊기지 않는다** → (a) 통과, 드레인 max > 1 이므로 (b) 의 앞 단언도 통과. **즉 진짜 결함 위에서 초록이 된다.** 짝은 **"조건이 일어났다(드레인 max > 1)" + "성질이 성립했다(송신/프레임 == 1)"** 두 수가 함께 있을 때 완성된다 — 수정 전은 `> 1 이고 > 1`, 수정 후는 `> 1 이고 == 1`. C-1 계측이 두 수를 다 잰다 **(c) 판정 도구가 사본이 아니라 진짜 코드를 잰다** — `GreyboxSession.Update()` 에 `while (_tickAccumulator >= …)` 산술이 **남아 있지 않고**(T-3), `GreyboxSendBurstTests` 가 `TickCatchUp.Plan` 을 **직접 부른다**(T-4). **(d) `outbound_queue_full_total`(관측 — 합격 조건 아님)** — **0이 아니어도 SC-89 의 FAIL 이 아니라 별건 발견으로 연다.** *근거 둘: ① client 실측 — 위반 세션의 9건 burst 는 **전부 송신에 성공했다**. **큐 포화는 이번 위반의 원인이 아니다.** ② qa3 가 (a) 에 넣었던 근거("경로 B = 입력 유실 **＋ 예측 어긋남**")의 **절반이 architect 판정 K-1 로 사라졌다** — 송신 실패 tick 도 **이월 입력으로 예측**하므로(`GreyboxSession.cs:446` 의 `return` 제거) 어긋남 쪽이 없어진다. 남는 절반("플레이어 입력이 서버에 닿지 않는다")은 **SC-89 가 재는 성질(위반·끊김)과 다른 성질**이고, **한 항목이 두 성질을 재게 하는 것은 이번 라운드에 SC-59 (b) 에서 정정한 바로 그 결함이다.** **(e) 임계값이 그대로다**(9차, §7 기계 확인) — 상한 8 · 예산 8/10 s · `rate_limit_hz` · `protocol_violation_hz` 의 **diff 가 0**이다. **(f) 잔여 관측(기록)** — `reconcile_forced_after_hitch_total` 과 `catchup_truncated_total` 은 **실플레이에서 0** 이어야 한다. **0이 아니면 tick 정렬 예측(미룬 결정, ADR-0012 §6.3)의 발동 조건이 충족된 것**이므로 architect 에게 알린다 | (a) **T-5 성질 테스트**(무작위 프레임 델타 열, **≥ 450 ms 히치 포함**): ① 프레임당 송신 ≤ 1 ② 벽시계 1초당 예측 tick 수 == 20 ± 1 ③ 이월 tick 수 + 송신 tick 수 == 예측 tick 수. **＋ 실서버 세션**의 `/debug/stats` 델타(`stats_delta.py`)와 DB. **⚠ `outbound queue full` 은 (a) 의 합격 조건이 아니다**(9차 보충, architect Q-1) — 아래 (d) 의 **관측**이다. **⚠ 절차 제약(9차): SC-59 와 같은 세션으로 촬영하지 않는다** — SC-59 는 Game View 포커스를 요구하고 이 항목은 **백그라운드 구간**을 요구한다. **그래서 이 항목의 증거는 화면이 아니라 `grep` 가능한 카운터 로그와 DB 다**(client H-13: 관측을 `OnGUI` 뿐 아니라 로그로도 낸다 — **`OnGUI` 는 비포커스면 호출되지 않으므로 HUD 만으로는 백그라운드 구간의 증거가 원리적으로 안 남는다**). **재현 절차**: 자유 비행보다 **백그라운드 전환**이 훨씬 잘 재현한다 — 450 ms 히치 한 번은 위반 1회뿐이지만(예산은 10초에 8회), **백그라운드에서 Editor 가 2~4 Hz 로 조이면** 2 Hz = 프레임당 10 tick = **매 프레임 위반 1회** → 4초면 예산이 찬다(architect R4 보충 §F). **⚠ 그러나 그 산수는 Editor 가 throttle 할 때만 성립한다 — pause 하면 복귀 프레임 1회뿐이라 예산이 안 찬다. 어느 쪽인지는 아직 실측되지 않았다**(`runInBackground = 0` 은 Standalone 용이라 답이 아니다 — client R8). **순서**: ① **먼저 C-1 계측으로 백그라운드 구간의 드레인 max 와 프레임 간격을 읽어 throttle/pause 를 가른다.** ② **pause 로 판명되면 대체 벡터를 쓴다** — 도메인 리로드 반복 · 무거운 씬 로드 · 강제 GC 루프처럼 **전경에서 반복 히치를 만드는 것**. 어느 쪽이든 필요한 것은 **"10초 안에 450 ms 이상 히치 8회"** 다. ③ **(g) 양성 대조는 재현 벡터와 무관하게 그대로 유효하다** — *탐지기가 작동한다는 증명은 재현 방법과 독립이다.* (c) `grep -n 'while (_tickAccumulator' client/Assets/_Project/Scripts/Greybox/GreyboxSession.cs` 가 **exit 1**, 그리고 테스트 소스에 `TickCatchUp.Plan` 호출이 있다. (e) `git diff` 로 해당 상수·설정의 변경 0. **(g) 양성 대조는 봇으로 한다**(9차 보충, architect Q-2) — `tools/bots/` 의 **한 tick 에 9건을 보내는 시나리오** 하나로 **"서버가 실제로 위반을 계수하고 예산이 차면 끊는다"** 를 보인다. *모순이 아니다: **봇이 (a) 를 닫지 못하는 이유는 원하는 분포를 정확히 만들 수 있기 때문**이고(그래서 "실제 클라이언트가 버스트를 내는가"를 증명하지 못한다), **그 성질이 양성 대조에서는 정확히 필요한 것**이다.* 수정 전 바이너리 보관(AC-2(i) 방식)보다 낫다 — **재현이 결정적이고, 수정이 진화해도 대조가 낡지 않으며, 하네스가 이미 있다.** **그 대조 자신도 RED 를 먼저 보인다**: 시나리오를 만들었으면 **그것이 실제로 위반을 만들어내는지 먼저 확인한다.** 만들지 못하면 그 대조는 아무것도 증명하지 못한다(§7a 를 게이트 자신에게). **⚠ 판정은 `protocol_violations_total` 로 한다 — 거부 수로 하지 않는다**(qa3 실측 2026-09-23, `evidence/R4-B9/sc89/`). *`RATE_LIMITED` 는 평균 속도 문턱이 아니다.* **두 층이 있고 둘 다 tick 당 건수를 본다**: 게이트웨이 **8/tick**(초과 시 제출 안 함 + 그 tick 에 위반 1회)과 시뮬레이션 **`rate_limit_hz.div_ceil(tick_hz)` = 2/tick**(초과 시 `RATE_LIMITED`). **그래서 몰아 보내기는 두 층을 반드시 함께 건드리고, 간격을 벌려 평균을 낮춰도 갈라지지 않는다.** 9건/라운드면 **드롭 1 : `RATE_LIMITED` 6 : 수락 2** 가 유도되고, 실측이 정확히 그 비였다(위반 9 · 드롭 9 · `RATE_LIMITED` 48 · 수락 16 = 8라운드분 + 닫히는 라운드). **`RATE_LIMITED` 가 드롭보다 많은 것이 정상이며, 그것을 대조의 결함으로 읽지 않는다.** **⚠ 봇으로는 (a) 를 닫을 수 없다** — 봇은 송신 속도를 통제해 보내므로 이 경로를 **원리적으로** 밟지 않는다(§0.11 과 같은 형태). 닫는 것은 **순수 함수 성질 테스트 + 실플레이 관측**뿐이다. **RED 선행**(§7a): 수정 전 코드에서 성질 테스트가 실제로 빨간불을 켜는 것을 먼저 보인다 — 0.46 s 한 점의 RED 는 이미 확인됐다(qa3 r4 §5.7 (4), `test-results.xml` 167/164/**1 failed**) | client, qa | **신설**(AC 밖 — architect R4 판정 §10, 보충 §F) | E3 |

### D. 적분과 서버 판정 (AC-4, AC-5) — server

> **`TOO_MANY_IN_FLIGHT` 는 이제 구조적 미도달이다**(Q6). tick 당 상한 8(`MAX_COMMANDS_PER_SESSION_PER_TICK`)이 먼저 걸려 in-flight 64 에 도달할 수 없다. server 가 **지연 tick 주입 단위 테스트**로 대체했고, 부하 실행에서는 `commands_rejected_total{TOO_MANY_IN_FLIGHT} == 0` 을 기록한다. **간헐 실패를 걱정하던 항목이 튜닝이 아니라 설계로 없어졌다.**

**전부 메모리에서 돈다**(`Simulation::new(world, 0)`, tokio·소켓 없음). 실서버 `last_tick = 350280`과 무관하다(게이트 G-g).

| ID | 검증 항목 | 검증 방법 | 담당 | 근거 AC | 불가 |
|----|----------|----------|------|--------|------|
| SC-15 | 항등 자세에서 `thrust_z_milli = 1000`을 T tick 적용하면 **위치의 `z`만 증가**하고 값이 ADR-0010 §2 손계산과 **양자화 후 정수로 일치** | `cargo test -p starfall-sim` 해당 테스트. **기대 숫자는 `contracts/fixtures/SHIP_CLASS/example-scout.json`에서 온다**(§0.7). **C3(클라이언트)의 첫 테스트와 같은 숫자여야 한다** — 그 일치가 ADR-0010 규약의 실체다 | server | AC-4(a), ADR-0009 §1 | E1 |
| SC-16 | 최대 추력을 오래 넣어도 `|v|`가 `max_speed_mps`를 **넘지 않는다** | 같은 테스트 스위트. 도달 최대 속도를 출력에 | server | AC-4(b) | E1 |
| SC-17 | 세 축 전부 1000인 입력의 로컬 가속 **크기**가 `main_thrust_mps2`를 넘지 않는다(대각선 클램프) | 합성 가속 크기를 출력. 클램프가 없으면 √3배가 나온다 | server | AC-4(c), S-4-4 | E1 |
| SC-18 | soft 경계를 넘으면 원점 방향 가속이 **더해지고 조작은 계속 먹으며**, hard 경계에서 **반경 속도 성분이 0**이 되고 **접선 성분은 남는다** | 경계 접촉 시나리오 테스트. 접선 속도가 보존됨을 수치로 | server | AC-4(d), I-34, S-6 | E1 |
| SC-19 | 조작을 멈추고 이월이 만료되면 가속이 0이 되지만 **속도는 감쇠만 적용되어 즉시 0이 되지 않는다** | `carry_forward_max_ticks` 경과 후 속도 곡선 | server | AC-4(e), S-1-4 | E1 |
| SC-20 | 브레이크 중에는 추력이 무시되고 감쇠가 **하나만** 적용된다 | 브레이크 tick의 가속 성분 분해 | server | AC-4(f) | E1 |
| SC-21 | 목표 자세를 정반대로 주면 **오버슈트 없이 안착**: (1) 매 tick `|ω_aim| ≤ turn_rate_max_deg_s` (2) 자세 오차 `s`가 `turn_deadzone_sin_half` 아래로 내려간 뒤 **40 tick 동안 다시 올라가지 않는다** (3) **안착 tick 수를 출력에 찍는다**. (g2) 오토레벨 켠 채 목표 자세 유지 시 **진동하지 않는다** | 같은 스위트. 안착 tick 수는 M-12로도 기록(U-21) | server | AC-4(g), ADR-0010 §2 | E1 |
| SC-22 | 퇴화 쿼터니언(전 성분 0)은 **거부되지 않고** 현재 자세 유지 + `aim_degenerate_total` 증가 | 단위 테스트 + 메트릭 증가 확인 | server | AC-4(h), I-39 | E1 |
| SC-23 | `position-field-injected.json`·`attitude-field-injected.json`을 **그대로 소켓으로** 보내면 `COMMAND_RESULT{REJECTED, MALFORMED_COMMAND}`가 오고 **그 tick에 함선 상태가 바뀌지 않는다**(스냅샷으로 확인) | 봇 치트 케이스 2종. **시도 횟수와 차단 횟수를 적는다** | server | AC-5(a), I-26, S-4-1 | E5, E6 |
| SC-24 | (b) `SetShipControlPayload`의 필드 목록에 위치·속도·현재 자세가 **없다**(타입이 곧 증거) (c) 범위 밖 값은 **클램프되지 않고 거부**되며 **그 tick에 직전 입력이 이월되어 함선이 계속 정상 이동한다** (d) `aim_*`를 극단값으로 계속 보내도 한 tick 회전량이 `turn_rate_max_deg_s × dt`를 넘지 않는다 | (b) 파일:라인 + 필드 목록 (c)(d) 봇 치트 케이스. (c)는 거부와 **동시에 이동이 끊기지 않음**을 스냅샷으로 | server | AC-5(b~d), I-33, I-26, S-4-2·3 | E5, E6 |

### E. 속도 핵·스냅샷·결정성 (AC-6, AC-7, AC-8) — server

| ID | 검증 항목 | 검증 방법 | 담당 | 근거 AC | 불가 |
|----|----------|----------|------|--------|------|
| SC-25 | 같은 조작을 두 봇이 보내되 하나는 `client_send_hz`(20), 하나는 **10배 속도**로 30초 유지 → **두 함선의 원점 기준 이동 거리 차이가 `max_speed_mps × dt = 7.0 m` 이내**(최대 속도 기준 고정값) | 봇 2대 시나리오. **판정 근거는 "코드에 방어가 있다"가 아니라 두 거리의 차이다.** 거리는 **봇이 받은 스냅샷의 위치 정수**로 계산. 7.0 m는 "두 봇의 시작 tick이 최대 1 tick 어긋날 수 있다"는 **측정 불확실성의 상한**이지 속도 핵의 허용치가 아니다. **함께 적는다**: 두 봇의 최종 속도와 **거리차 / 7.0 비율** — **비율이 0.2를 넘으면 7 m 안이어도 기록한다**(경계 어긋남이 아닌 다른 원인의 신호) | server | AC-6(a), I-28, S-4-5, server ack §1-⑩ | E5, E6 |
| SC-26 | **큰 burst 를 보내도 연결이 `SLOW_CONSUMER` 로 끊기지 않는다.** 묶는 양이 in-flight 가 아니라 **`MAX_COMMANDS_PER_SESSION_PER_TICK = 8`** 로 바뀌었고(ADR-0011 §5.2), 초과 프레임은 **제출도 응답도 없이** 카운터 + 그 tick 에 프로토콜 위반 1회다. 손실 항등식이 갱신된다: **보낸 수 = 받은 `COMMAND_RESULT` + `commands_dropped_over_tick_cap_total`**. `RATE_LIMITED` 거부를 받아도 **연결은 유지**된다 | **두 출처로 판정한다**(p0-02 의 1:1 보다 강하다 — I-25): ① 봇 `summary.json` 의 `sent_total`·`results_total`·`rejected_by_reason` ② `/debug/stats` 의 `commands_dropped_over_tick_cap_total` 델타. 세 수가 항등식을 만족해야 한다. **봇이 실제로 보낸 명령 수**도 함께 적는다(폭주 봇이 자기 CPU·소켓에 먼저 막히면 "서버가 막았다"와 구분되지 않는다). 종료 시 `close_reason` 이 **`SLOW_CONSUMER` 가 아님**을 DB 로 확인 | server, qa | AC-6(b), ADR-0011 §5.2, architect 지시 5 | E6, E7 |
| SC-27 | `input_superseded_total`이 증가한다 | `/debug/stats` 델타 | server | AC-6(c) | E5 |
| SC-28 | 열려 있는 모든 세션이 `snapshot_interval_ticks`마다 **정확히 1건**을 받고, 연속 두 스냅샷의 envelope `tick` 차이가 그 값(=2)과 같다. **단 각 세션의 첫 스냅샷은 검사에서 제외한다** | 봇 스냅샷 CSV에서 tick 차이 분포. **차이가 2가 아닌 쌍의 수 = 0**(첫 스냅샷 제외), **검사한 쌍 수와 제외한 건수(세션당 1건)를 적는다**. **제외하는 이유**: 스냅샷은 **전역 tick 기준**(`tick % interval == 0`)으로 발사되므로 중간에 들어온 세션의 첫 간격은 **0~1 tick**이다. 세션별 위상을 두면 31세션이 서로 다른 tick에 직렬화를 요구해 **모든 tick이 스냅샷 tick**이 되고 부하가 2배가 된다(server ack §1-⑤) | server | AC-7(a) | E6 |
| SC-29 | `ships`에 **월드의 모든 함선**(잔류 포함)이 **`ship_id` 오름차순**으로 있다 | 봇이 수신 즉시 순서를 검사(정렬 위반 수를 센다). **정렬 기준은 `ship_id`의 정규 소문자 문자열 사전순이며 서버 내부 `BTreeMap<UuidV7,_>`의 바이트순과 같다**(하이픈 위치가 고정이라 두 순서가 일치한다 — server ack §2). 함선 수는 `/debug/stats`의 `ships_active + ships_lingering`과 대조하며, **그 두 값은 월드에서 직접 읽어 `store`하는 게이지여야 한다**(증감 카운터면 이 대조가 항진명제가 된다 — I-25) | server | AC-7(b), I-37 | E6 |
| SC-30 | 각 수신자의 `controlled_ship_id`가 자기 함선이고 `ships` 안에 있다 | 봇마다 `controlled_ship_id`가 자기 `SHIP_SPAWNED`의 `ship_id`와 같은지 + 배열에 존재 | server | AC-7(c) | E6 |
| SC-31 | `ack_input_seq`가 **서버가 마지막으로 적용한 `input_seq`**이고, 적용 전에는 `null`이며 **한 세션 안에서 단조 비감소**다. 한 tick에 `(5, 3)` 순서로 도착하면 5가 적용되고 3은 `STALE_INPUT` | 봇이 `ack_input_seq` 시계열을 **세션별로** 기록 → 세션 내 단조 비감소 위반 수 0. **재개를 포함한 새 세션은 `null`에서 다시 시작하므로 전 구간 단조로 읽지 않는다**(server ack §1-③: 재개 시 `last_applied_input_seq`를 `None`으로 초기화한다 — 안 하면 새 세션의 `input_seq=1`이 `STALE_INPUT`으로 거부된다). `(5,3)` 케이스는 전용 치트 시나리오 | server | AC-7(d), ADR-0011 §4 | E6 |
| SC-32 | 잔류 함선의 `presence`가 `LINGERING`이고, **디스폰된 다음 스냅샷부터 `ships`에서 사라진다** | 봇 CSV의 `presence` 열 + 디스폰 tick 전후 스냅샷 비교 | server | AC-7(e), I-40 | E6 |
| SC-33 | `/debug/stats`에 **신규 7키**가 있고 **동작한다**(부하 전후로 값이 변한다): `snapshots_sent_total` · `snapshot_bytes_total` · `send_queue_bytes` · `send_queue_bytes_max` · `input_superseded_total` · `input_carried_forward_total` · `aim_degenerate_total` (전부 **스칼라**). 더해 **기존 라벨 배열이 늘어난다**: `commands_rejected_total` 6 → **8라벨**(`RATE_LIMITED`·`STALE_INPUT` 추가), `messages_enqueued_total`/`messages_written_total` 3 → **4라벨**(`WORLD_SNAPSHOT` 추가). `ships_active`·`ships_lingering` 게이지 2종도 함께 | 키 존재 + 델타 확인. 존재만으로 PASS 주지 않는다. **교차 검증**: `snapshots_sent_total`은 `messages_written_total{WORLD_SNAPSHOT}`과 **같은 수여야 한다**(server가 의도한 중복) | server | AC-7(f), server ack §1-④ | E5 |
| SC-34 | 고정 초기 상태 + **파일에 기록된 입력열**(함선 3척, 600 tick, 추력·선회·브레이크·경계 접촉·잔류 포함)을 **서로 다른 프로세스에서 2회** 돌린 스냅샷 산출물이 **바이트 단위로 동일**. **비교한 tick 수와 총 바이트를 출력에 찍는다** | `cargo test -p starfall-sim` 결정성 테스트. **근사 비교 금지.** 산출물 형식은 태스크 문서의 3파일(`initial.json`/`inputs.jsonl`/`snapshots.jsonl`)이고 **운영과 같은 직렬화 경로**여야 한다 | server | AC-8, I-37 | E1 |
| SC-87 | **결정적 재생 골든**(라운드 2 신설 — **9차에 계약 표로 옮겼다**, 아래 주석). 같은 입력열을 재생하면 골든 3파일(`initial.json`·`inputs.jsonl`·`snapshots.jsonl`)이 **바이트 단위로 일치**하고, 재생이 골든을 **다시 쓰지 않는다** | `cargo test -p starfall-sim --test determinism --locked` → **exit 0** 이고 이어서 `git diff --exit-code -- server/crates/sim/tests/data/replay/` → **exit 0**. 골든 3파일의 sha256[:16] 을 적는다. **＋ 검출력을 함께 보인다**(§7a — 골든 비교는 파일이 안 바뀌면 언제나 초록이다): 골든 한 파일의 바이트를 일부러 바꾸면 **테스트가 실패하고 첫 차이 오프셋을 말해야 한다.** **⚠ 복원은 백업에서 하고 `STARFALL_REPLAY_BLESS` 를 쓰지 않는다** — bless 로 덮으면 "테스트가 자기 기대값을 다시 쓴 것"과 "원본이 돌아온 것"을 구분할 수 없다(qa r2 §1.8 실측. 일반 규칙은 §0.12). **검출력의 재확인 주기(9차)**: **골든 파일 · 테스트 · 직렬화 경로 중 하나라도 바뀌면 다시 확인한다. 셋 다 그대로면 직전 확인을 재사용하고, 재사용했다는 사실을 리포트에 적는다.** *매 라운드 돌리는 것은 낭비다.* | server, qa | AC-8, I-25 | E1 |

### F. 계약 ↔ Rust (AC-9) — server

공통: `cd server && cargo test -p starfall-contracts --locked -- --nocapture`

| ID | 검증 항목 | 검증 방법 | 담당 | 근거 AC | 불가 |
|----|----------|----------|------|--------|------|
| SC-35 | ADR-0002 §3의 테스트 9종이 **타입 13종 전부**를 덮고 통과. **검사 건수(26 / 34 / 18)를 출력에 찍는다** | 위 명령 | server | AC-9 | E1 |
| SC-36 | 유효 fixture **27건**(7차; 이전 26) 왕복 동일. **비교 규칙을 `kind`로 나눈다**: 와이어 3종은 `Value` 엄격 비교, `data`는 양쪽 `Number`를 `as_f64()`로 정규화 후 **정확히 비교**(근사 아님) | 출력의 27건 파일명 + kind별 건수 | server | AC-9(a), U-20 | E1 |
| SC-37 | **[층①]** 반례 **34건 전부** 스키마 검증에서 거부 | 34건 파일명·개수 | server | AC-9(b) | E1 |
| SC-38 | **[층②]** 반례 34건의 Rust 역직렬화 결과가 §0.5 표와 **일치** | 34행 결과표. 보조: `grep -rn "serde(flatten)\|serde(tag *=" server/crates/contracts/src`가 peek 구조체 외 0건 | server | AC-9(c), I-6 | E1 |
| SC-39 | 유효 fixture에서 `required` 필드를 하나씩 제거한 변이가 **전부** 실패. **변이 개수**를 적는다 | 위 명령 (p0-02는 124건이었다) | server | AC-9(d) | E1 |
| SC-40 | (e) `server` 태그 타입 **13종**이 이름→Rust 타입 대응표에 있다. (f) **`"type": "integer"`로 선언된 필드를 Rust가 정수 타입으로 받는다**(그래야 `10.0`이 역직렬화에서 거부된다 — `data` 정규화가 놓치는 구멍을 막는 단언) | 위 명령. (f)는 정규화 비교가 통과시키는 `10 ↔ 10.0` 드리프트를 막는 항목이다 | server | AC-9(e)(f) | E1 |

### G. 생성기 확장과 EditMode (AC-10, AC-11) — client

| ID | 검증 항목 | 검증 방법 | 담당 | 근거 AC | 불가 |
|----|----------|----------|------|--------|------|
| SC-41 | 확장 **전에** 현재 `contracts/`로 생성기를 돌려 **3단 캐스케이드를 전부 기록**한다 — 첫 실패는 **`maxItems`**이지 `array`가 아니다. 셋 다 종료 코드 2 | 세 번의 실행 로그를 `03_client_impl.md`에. **빨간불을 건너뛰면 "코드가 틀려도 통과하는 테스트"가 된다** | client | AC-10(a), §5.7 | E4 |
| SC-42 | 확장 후 생성 명령을 **두 번** 실행하면 두 번째 후 해시 불변, `--check` 종료 코드 0 | p0-02 SC-38과 같은 형식. **QA가 재생성해 client 제출본과 바이트 동일함도 확인** | client | AC-10(b) | E4 |
| SC-43 | `WorldSnapshotMessage`의 `ships`가 **배열**이고 `ShipState`가 **중첩 클래스**로 생성되며 **18개 속성이 전부** 있다(B-1의 `angular_velocity_roll_mdeg_s` 포함 — client가 현재 트리로 재측정한 값이다. **17로 두면 반드시 실패한다**) | 생성물 실물 확인 + 리플렉션 테스트. **`BuildProperties`의 `CsType` 우선 한 줄을 놓치면 `ships`가 배열이 아닌 `ShipState`로 나온다** — 그 회귀를 잡는 항목 | client | AC-10(c), U-14 | E4 |
| SC-44 | `SHIP_CLASS`·`STAR_SYSTEM`·`SYNC_TUNING`의 `.cs`가 **생성되지 않는다**(건너뛰기이지 실패가 아니다) **그리고 건너뛴 것이 stdout에 한 줄씩 남는다** | 생성 디렉토리에 세 파일 부재 + stdout 3줄. **조용한 건너뛰기는 p0-01이 경계한 형태다** | client | AC-10(d) | E4 |
| SC-45 | 확장 전후 diff: **기존 6타입의 타입별 파일 6개가 바이트 동일**하고 **`ContractTypes.cs`의 변경이 신규 항목 추가 8줄뿐** | `03_client_impl.md`의 diff 전문. *`ContractTypes.cs`는 생성물이고 신규 타입이 레지스트리 맵에 들어가므로 반드시 늘어난다* | client | AC-10(e) | E4 |
| SC-46 | `mkdir -p _workspace/p1-01-ship-movement/unity-tests` 후 EditMode 실행 → 종료 코드 0, 실패 0, **리포트 2개 존재**, **`tests` 수가 0이 아니고 그 수를 적는다** | `unity test client --mode EditMode --report-format nunit,junit --output …/EditMode.nunit.xml --junit-output …/EditMode.xml`. **`--report-format both`는 이 CLI가 거부한다**(exit 2). `--filter`는 정규식 | client | AC-11 | E3 |
| SC-47 | **유효 fixture 27건(7차; 이전 26)을 로더가 발견하고, 그중 계약 메시지 21건(이전 20 — 도메인 이벤트 fixture 가 1건 늘었다, client 실측으로 확정)이 `Strict` 왕복을 통과한다** | 두 수(26 / 20)가 각각 "fixture가 사라지지 않았다"와 "왕복을 빠뜨리지 않았다"를 지킨다. **20 = 26 − 데이터 3종의 6건**(`SHIP_CLASS`·`STAR_SYSTEM`·`SYNC_TUNING` 각 2건). 데이터는 DTO가 없고 envelope 판별자조차 없어 왕복할 대상이 아니다 | client | AC-11(a) | E3 |
| SC-48 | §0.5의 C# 열이 실측으로 채워졌고 그 결과를 **상수로 박는다**: 거부 **18** / 통과 **10** / 계층 없음 **6** (client가 현재 트리로 재측정. **17/10/7로 두면 반드시 실패한다**) | EditMode 테스트의 건수 3개. 표와 다르면 **FAIL이 아니라 architect 통지** | client | AC-11(b), §5.4 | E3 |
| SC-49 | (c) fixture 로더가 유효 fixture를 **27건 미만**(7차; 이전 26) 발견하면 실패 (d) **`ships`가 빈 배열인 스냅샷**과 **2척(하나는 `LINGERING`)인 스냅샷**이 모두 왕복 (e) `Runtime` 프로필이 `ShipState` 안의 모르는 필드를 무시하고 경고를 남기며 같은 입력이 `Strict`에서는 예외 | 테스트 3종 이름과 건수 | client | AC-11(c~e) | E3 |
| SC-50 | **클라이언트가 가진 `data/` 사본이 레포 원본과 내용이 같다** | **바이트 해시 비교**(줄바꿈 정규화·JSON 파싱 후 비교를 하지 않는다 — CRLF/LF 차이와 키 순서 차이를 놓친다). **파일 복사 방식의 유일한 방어다**(ADR-0012 §7) | client | AC-11(f) | E3 |

### H. 예측·재조정·실서버·육안·타 함선 (AC-12~15) — client

| ID | 검증 항목 | 검증 방법 | 담당 | 근거 AC | 불가 |
|----|----------|----------|------|--------|------|
| SC-51 | S6 산출물(입력열 + tick별 스냅샷)을 재생해 **각 스냅샷마다 열린 고리 적분 오차 ≤ `reconcile_ignore_threshold_m`**(0.005 m). **p99·최대·비교 지점 수를 출력에 찍는다** | **"재조정 직전" 이 아니라 "열린 고리" 다**(architect 2026-09-21). 이 fixture 로 계약 문구대로 `Reconcile()` 을 돌리면 **잴 것이 거의 남지 않는다**: (1) 첫 스냅샷은 1단계가 `history[S=1].StateAfter`(1 tick 분)를 확인 상태(2 tick 분)와 비교해 **미터급 장부 오차**를 내고, (2) 이후 ~293지점은 4단계가 `seq ≤ S` 를 버렸는데 `ack_input_seq` 는 그대로 `S` 라 매칭이 실패해 **`HasError = false`** → 히스토리가 비어 재생도 없고 오차가 **자명하게 0** 이 된다. **client 의 `ShipIntegrator.Step` 직접 대조가 이 fixture 에서 의도한 양을 재는 유일한 비-자명 방법이다.** **그리고 더 엄격하다** — 되감지 않고 600 tick 을 끝까지 적분하므로 재는 것이 **누적 표류**다(재조정을 끼우면 매 스냅샷 리셋돼 **2 tick 창**만 잰다). 같은 임계값을 그대로 쓰는 것은 **보수적**이다: 더 약한 기준을 더 어려운 조건에 적용한다. 비교 가능한 지점은 **스냅샷이 도착한 tick뿐**이다. **초과하면 그것이 결과다 — 임계값을 늘려 통과시키지 말고 architect에게 알린다** | client | AC-12(a), I-36 | E3 |
| SC-52 | 자세 오차 ≤ `reconcile_orientation_ignore_threshold_deg`(0.02°) | 같은 재생. 분포와 지점 수. **SC-51과 같은 경로(열린 고리)다** | client | AC-12(b) | E3 |
| SC-53 | 재조정이 **순수 함수**다: (1) 같은 입력 2회 실행이 같은 결과 **그리고** (2) **호출 사이에 시계·프레임 시간·스냅샷 도착 시각을 바꿔도 같은 결과** | (1)만으로는 부족하다 — **2회 동일은 순수성의 필요조건이지 충분조건이 아니다**(전역 상태를 읽어도 두 번 다 같으면 통과한다, client ack). (2)가 ADR-0012 §3의 "시계가 입력이 아니다"를 실제로 재는 유일한 케이스다 | client | AC-12(c), ADR-0012 §3 | E3 |
| SC-54 | `input_seq`를 하나 건너뛴 입력열에서도 재조정 후 상태가 서버 상태와 같다 | 합성 입력열 | client | AC-12(d) | E3 |
| SC-55 | **각속도까지 되돌리지 않으면 실패하는 케이스**(선회 중 스냅샷)가 포함되어 있고 통과한다 — `ω_aim` 과 `ω_roll` 을 **스냅샷의 두 필드에서 각각 되돌리지 않으면 실패한다** | **판정 근거가 둘로 나뉜다**(architect 2026-09-21): **① wire→sim 의 두 필드 되감기**는 **합성 `Reconcile` 테스트**(`Reconcile_TurningSnapshot_RestoresBothAngularVelocitiesSeparately_NotFromASum`)가 덮는다 — 이것이 SC-55 본문이 말하는 그 케이스다. **② 물리(선회 중 두 각속도가 자세에 반영되는가)**는 **S6 재생**이 덮고, 거기에 **AC-12(f)** 가 추가된다(확인 상태의 `ω_aim`·`ω_roll` 이 롤 구간에서 **둘 다 0이 아님**을 단언. 각속도 오차 분포는 **출력만** — 해당 튜너블이 없으므로 **이번 슬라이스에서 임계값을 새로 만들지 않는다**). **S6 재생만으로 SC-55 를 판정하지 말 것** — 그 경로는 열린 고리라 C# 이 자기 ω 를 스스로 적분하므로, **각속도를 대조하지 않으면 wire→sim 이 `angular_velocity_roll_mdeg_s` 를 떨어뜨려도 통과한다**(tick 0 의 ω 는 스폰 직후라 0 이어서 초기 상태 경로로도 안 걸린다). **그것이 정확히 B-1 과 이 항목이 존재하는 이유다.** AC-12(f)(CL-1)가 붙기 전에는 ② 쪽 근거가 불완전하다. 단언은 **같은 스냅샷 안에서 두 각속도가 둘 다 0이 아님**을 봐야 한다 — 한쪽만 0이 아닌 구간을 고르면 두 필드를 구분하지 못한다 | client | AC-12(e)(f), ADR-0010 §1.1 | E3 |
| SC-56 | **⚠ 범위(9차 보충, architect Q-1 문단 1)**: 모든 재조정 지표는 **세션 범위(`OnSessionReady` 에서 리셋)로 판정**하고, `_run` 누적 짝 셋(`hard_snap_total_run`·`error_m_max_run`·`error_deg_max_run`)은 **맥락으로 함께 적는다.** *라운드 4 에서 한 HUD 안에 범위가 다른 두 수가 섞여 있었다 — `hard_snap_total` 만 우연히 리셋되고 오차 리스트·퍼센타일은 리셋되지 않았다(qa3 r4 §5.9 (1)).* **그리고 `max` 를 적을 때는 언제나 표본 수 `n` 을 같이 적는다**(예: `deg max = 80.5663 (n = 412)`) — **`max` 혼자서는 "더 큰 표본이 안 들어왔다"와 "지표가 멈췄다"를 구분하지 못한다.** 카운터 항등식에 `accepted > 0` 을 짝지은 것과 같은 규율을 게이지에 적용한 것이다. **측정 시점**: `SC-89` 의 catch-up 수정 **뒤**에 잰다 — 수정 전 수치는 "재조정이 나쁘다"가 아니라 **"송신이 버스트였다"를 잰 것**이다. ｜ 실서버 60초 조작(직진·선회·브레이크·정지·경계 접근) → (a) `SESSION_READY` 다음 첫 `WORLD_SNAPSHOT` 에 자기 함선이 있다 (b) 재조정 직전 위치 오차 p50/p99/최대를 HUD·로그에 (c) **`reconcile_hard_snap_total` 이 0** **(d) 아래 관찰 3건을 세션 로그에 남긴다**(architect 2026-09-21, CL-2): **① `HasError == true` 인 `Reconcile()` 호출 수 — 0이면 FAIL** **② 3단계에서 재생한 입력 수가 0이 아닌 호출 수**(되감기만 한 것과 구분) **③ `ω_aim`·`ω_roll` 이 둘 다 0이 아닌 스냅샷에서 재조정한 횟수 — ≥ 1** | **(d) 가 없으면 (b) 의 수치는 자명하게 통과한다** — `HasError = false` 면 오차가 0으로 찍히므로 "재조정 경로가 진짜 서버 산출물을 만난 적이 있는가" 에 아무도 답하지 않는다. **fixture 로 메우지 않는다 — 실서버가 메운다**: `Reconcile()` 이 전제하는 tick 당 1건 분포는 정의상 실서버에서만 나온다(`data/movement/sync-tuning.json` 의 `client_send_hz = 20` = `tick_hz`, designer 노트 "One input per tick", `carry_forward_max_ticks = 10` 은 딸꾹질 흡수용이지 정상 경로가 아니다. S6 fixture 는 `10_000` 에 600 tick/명령 6건 — **약 100배 성기다**). **판정 전에 `close_reason` 을 먼저 본다**: 송신 큐 64 = **2.13초**이고 Editor 도메인 리로드·GC 히치가 그것을 넘으면 서버가 **정당하게** `SLOW_CONSUMER` 로 끊는다. 그 경우 서버 버그가 아니라 **재측정**이다. **상한: 연속 3회 `SLOW_CONSUMER` 면 재측정이 아니라 조사 대상**이고 그 사실을 리포트에 적는다(무한 재시도 금지 — client ack) | client | AC-13(a~d), §8 | E3, E5 |
| SC-57 | 정상 종료 후 잔류 창이 지나면 DB에 그 함선의 `SHIP_DESPAWNED{LINGER_EXPIRED}`가 있다 | SQL(ship_id 기준) | client, qa | AC-13(d) | E2, E3 |
| SC-58 | **스냅샷 1건당 할당량과 초당 할당량**을 **2단으로** 남긴다: **1단(정본)** 격리 벤치(Unity 동봉 mono, 200회 반복) — client가 이미 측정했다(직접 역직렬화 **0.265 ms/건, 200건당 gen0 1회** vs 트리 경로 0.656 ms/8회). **2단** Editor Profiler 확인 | **Deep Profile을 끈다**(켜면 할당량이 부풀어 숫자가 못 쓰게 된다). 마커 이름은 **`Starfall.Snapshot.Handle`**로 고정 — Profiler Hierarchy 검색으로 찾고, **마커가 있는 프레임의 `GC Alloc`이 곧 건당 할당**이다(10 Hz라 6~14프레임 중 1프레임에만 있다). 초당 = 건당 × 10. **1단·2단 절대값이 다를 것이므로 둘 다 적고 어느 쪽이 정본인지 명시한다** | client | AC-13(e), U-16 | E3 |
| SC-59 | **육안**: (a) 전방 추력이 **뱃머리 방향**으로 가고, 마우스 오른쪽이 **오른쪽 선회**이며, 위 추력이 **위로** 민다 **(b1) 배치** — 기준 마커가 **4개 존재하고 정해진 좌표에 있다.** **EditMode 씬·데이터 단언으로 닫는다 — 영상 불필요** · **(b2) 가시성** — **촬영 구간 전체에 걸쳐 최소 1개가 화면에 있다.** 영상으로 닫는다. **부호·방향 판정에 실제로 필요한 성질은 이쪽이다** (c) soft에서 경고, hard에서 미끄러짐 (d) 오토레벨이 손을 떼면 수평을 되찾고 **수동 롤 중에는 동작하지 않는다** | **mp4(H.264) 4개**(`SC-59{a,b,c,d}-*.mp4`, 10~25초, 1280×720 이상) + 같은 이름의 `.png` + 한 줄 설명. ffmpeg 7.1.1 `gdigrab` 사용 가능(client 확인). **형식보다 내용이 판정한다 — 화면에 입력 상태가 떠 있지 않으면 그 영상은 부호를 증명하지 못한다**: `thrust=(x,y,z)`·`roll`·`brake`·`assist`를 **양자화 정수 그대로**, 마우스 위치 또는 목표 자세 표시자, 속도·원점 거리·tick·`ack_input_seq`·예측 오차, 기준 마커 최소 1개. **한 클립에 정지 → 입력 → 이동 → 입력 해제**를 담는다(스틸은 회전 방향을 증명하지 못하므로 보조). **(d)는 ① 손 뗐을 때 수평 복귀와 ② 수동 롤 중 복귀가 일어나지 않음을 한 클립에** — 후자가 없으면 "오토레벨이 항상 돈다"는 버그를 통과시킨다. **(a)(d)가 이 슬라이스에서 부호 버그를 잡는 유일한 장치다**(§0.10). **(b1) 은 씬·데이터 단언(EditMode)으로 닫는다 — 영상이 아니다.** "1개면 충분"으로 낮추면 마커가 3개 사라져도 통과하므로 배치와 가시성을 분리했다(9차, architect R4 보충 §G·Q-3). **왜 영상으로 (b1) 을 닫지 않는가**: 마커가 **4.2 km 안**에 있고 경계가 **12 km** 라, 한 시야에 4개를 넣으려면 **게임에 없는 카메라 거리**가 필요하다. **촬영을 위해 게임을 바꾸는 것은 거꾸로다.** **대신 녹화 시작 프레임의 HUD 에 마커 4개의 ID·거리를 한 줄로 표시한다**(client 작업) — 그러면 **"4개가 있다"와 "지금 1개가 보인다"를 같은 영상에서 읽을 수 있다.** **⚠ 절차 제약(9차): SC-59 와 SC-89 를 같은 세션으로 촬영할 수 없다.** SC-59 는 **Game View 가 활성 탭이고 포커스를 가져야** HUD 가 그려지고(`OnGUI` 는 **비포커스면 아예 호출되지 않는다** — client R8, 라운드 4 에서 HUD 가 한 줄도 없던 이유), SC-89 는 **백그라운드 구간을 요구한다.** 묶으면 둘 다 닫지 못한다. **촬영 절차에 "Game View 활성 탭·포커스 유지"를 넣고, 할 일을 촬영 시작 전에 전부 알려 준 뒤 시작한다.** HUD 확인은 **① 시작 직후 ② 클립 a 직전** 두 번 | client | AC-14 | E3 |
| SC-60 | 타 함선: (a) 두 번째 접속 함선이 보이고 움직인다 (b) **예측되지 않고 보간된다 — 자세도 slerp**. 검증은 **`t = 0.25`(비대칭 지점)에서 자세 차 60° 이상인 합성 쌍**으로 한다 (c) 스냅샷이 끊기면 `remote_extrapolate_max_ms`(250 ms)까지만 외삽하고 **그 뒤 정지**(날아가지 않는다) (d) 세션이 끊기면 `LINGERING`으로 계속 보이고 디스폰되면 다음 스냅샷에서 제거 (e) 보간 지연이 `snapshot_interval_ticks`와 `sync-tuning`에서 계산되고 그 값을 로그에 찍는다 | *`t = 0.5`는 slerp와 nlerp가 같은 값을 내는 대칭점이라 선형 보간+정규화 구현이 통과한다 — 쓰면 안 된다.* 60°는 실제 스냅샷 쌍(최대 7.5°)에 없으므로 자산을 합성한다 | client | AC-15 | E3 |

### I. QA — 같은 월드를 보는가, 치트 (AC-16, AC-17)

| ID | 검증 항목 | 검증 방법 | 담당 | 근거 AC | 불가 |
|----|----------|----------|------|--------|------|
| SC-61 | A만 30초 전방 추력, B는 무입력 → **B가 받은 스냅샷 안의 A의 위치가 단조적으로 변하고**, 총 이동 거리가 `max_speed_mps`와 시간으로 설명되는 범위 안이다 | B의 CSV에서 A의 `ship_id` 행을 tick 순으로. **그 시점 `max_speed_mps` 값을 증거에 함께 적는다**(§0.7) | qa | AC-16(a) | E6, E7 |
| SC-62 | B의 함선은 **스폰 위치 근처에 머문다** | B의 CSV에서 자기 `ship_id` 행. 총 변위를 적는다 | qa | AC-16(b) | E6 |
| SC-63 | 같은 tick의 스냅샷에서 **A가 본 자기 위치와 B가 본 A의 위치가 양자화 정수로 완전히 같다**. **검사한 스냅샷 쌍의 수를 적는다** | 두 CSV를 envelope `tick`으로 조인, `(tick, ship_id)`의 정수 6개 비교. **리포트에 "이 항목은 항진명제에 가깝다"를 적는다** — 두 값은 같은 `ships` 배열에서 나오므로 정상 구현에서는 다를 수 없고, **잡는 것은 세션별 직렬화가 배열을 다르게 만드는 버그 하나뿐**이다. 진짜 검증은 SC-61·SC-62다(I-25) | qa | AC-16(c)(d) | E6 |
| SC-64 | **S-3 ① 정지 비교**: A가 정지한 뒤 **0.5초 이상** 지난 시점에 두 화면의 A 좌표 차이가 **≤ 2 m** | **한 Unity 프로세스 안의 세션 2개 + 관측자 파이프라인 2개**(§0.11)의 같은 tick 관측. *정지 상태에서는 예측도 보간도 같은 값을 내므로 지연이 오차를 만들지 않는다 — 2 m를 넘으면 네트워크가 아니라 **상태가 실제로 다른 것**이다.* **이 항목만은 B를 봇으로 대체해도 의미가 산다**(정지 상태에서 두 경로의 차이가 0으로 수렴한다) | qa, client | 설계 §8 S-3-6 | E3, E6 |
| SC-65 | **S-3 ② 이동 비교**: A가 140 m/s로 순항 중인 순간의 차이가 **진행 방향 뒤쪽으로 28 ± 12 m**. **부호가 판정의 핵심** — **앞쪽으로 어긋나면 외삽 과다이거나 보간 버퍼가 비어 있는 것** | 차이 벡터를 A의 속도 방향에 **투영한 부호 있는 값**으로 판정한다. 28 m의 근거는 보간 지연 200 ms × 140 m/s. **관측자 B를 봇으로 대체하면 안 된다(§0.11)** — 봇은 보간이 없어 서버 진실을 갖고 A는 예측으로 앞서 있으므로 차이가 **앞쪽 0~7 m**로 나온다. 부호가 반대이고 크기가 한 자릿수 다르며, 이 항목은 "앞쪽이면 외삽 과다"로 채점하므로 **정상 동작을 버그로 읽는다**(client ack). 대체가 불가피하면 **이 항목은 미검증(환경)**으로 둔다 | qa, client | 설계 §8 S-3-6b | E3, E8 |
| SC-66 | 치트 (a): 위치·현재 자세 필드 주입 → **전부 `MALFORMED_COMMAND`, 상태 변화 0**. **시도 횟수와 차단 횟수를 적는다** | 봇 치트 케이스 2종 × N회 | qa | AC-17(a) | E6 |
| SC-67 | 치트 (b)(d): 조작 값 범위 초과 → 전부 거부, **클램프 흔적 없음**, 그 tick에 **이월이 동작해 조작이 끊기지 않음**. `input_seq` 역행·반복 → `STALE_INPUT` 거부, **`ack_input_seq`가 되돌아가지 않음** | 각각 시도/차단 수 + `ack_input_seq` 시계열의 단조 비감소 위반 0 | qa | AC-17(b)(d) | E6 |
| SC-68 | 치트 (e)(f)(g): 다른 `ship_id` 조작 → **명령에 `ship_id` 필드가 없어 시도할 방법 자체가 없다**("어휘에 없어 시도 불가"로 적고 그것이 코드 검사보다 강한 보장임을 명시) / 다른 actor의 잔류 함선 가로채기 재접속 → **자기 `actor_id`의 함선만 돌아온다** / `SESSION_READY` **이전**에 보낸 `SET_SHIP_CONTROL`이 함선을 만들거나 움직이지 않는다 | (e)는 계약 필드 목록이 증거. (f)(g)는 봇 시나리오 + 스냅샷·DB 확인 | qa | AC-17(e~g) | E6 |

### J. QA — 31 연결 부하와 대역폭 (AC-18)

단계: **A** 31 연결 60초(각 봇이 `client_send_hz`=20으로 조작 전송) · **B** 세션 회전 5회(**잔류·재개 경로 포함**) · **C** 1개 폭주 · **D** DB 30초 중단

| ID | 검증 항목 | 검증 방법 | 담당 | 근거 AC | 불가 |
|----|----------|----------|------|--------|------|
| SC-69 | 명령 손실 0, `COMMAND_RESULT` 1:1 (**측정 창 델타 기준**) | 봇 `summary.json` + `/debug/stats` 델타. p0-02 계약 외 발견 1번대로 **누적이 아니라 델타**로 판정 | qa | AC-18(a) | E6, E7 |
| SC-70 | **스냅샷 손실 0**: 봇이 센 수신 수 == `snapshots_sent_total` 델타 | 두 출처를 나란히. 차이가 있으면 그 차이가 곧 발견이다 | qa | AC-18(b) | E6, E7 |
| SC-71 | **실제 송신 바이트를 봇 쪽에서 독립으로 측정**해 ADR-0011 §2 산출(세션당 **161.8 KiB/s**, 합계 약 **42.5 Mbit/s**)과 대조. **차이가 20 %를 넘으면 ADR의 표를 고친다.** 전제 확인: **`snapshot_bytes_total`이 소켓에 실제로 쓴 바이트여야 한다** — 큐에 넣은 시점에 세면 드롭·잘림이 양쪽에서 같이 사라져 **독립 출처가 아니게 된다** | 봇이 수신 프레임의 바이트 길이를 직접 합산(서버 메트릭과 무관). **전제는 확인됐다**: 서버는 `ws.rs:278`의 `record_message_written` **바로 옆**, 즉 `sink.send`가 `Ok`를 돌려준 뒤에만 센다(server ack §1-②). **무엇을 쟀는지 적는다** — 서버가 세는 것은 **WebSocket 페이로드 바이트**이고 봇이 TCP 바이트를 재면 프레임 헤더(15 KiB에서 4 B, **0.03 % 미만**)가 더해진다. **권고는 페이로드 길이**(그래야 차이가 "ADR 산출이 틀렸다"만 뜻한다) | qa | AC-18(c), architect 지시 4 | E6, E7 |
| SC-72 | 세션당 송신이 `egress_budget_kib_s_per_session`(192 KiB/s) **아래**임을 확인 | 봇 실측 KiB/s. 초과하면 설계 §S-7-4의 대응(`snapshot_hz` 10 → 5)은 **designer 결정 사항**이고 QA는 수치만 보고한다 | qa | AC-18(d) | E6 |
| SC-73 | `send_queue_bytes` 최대와 용량 대비 비율, **`close_reason = SLOW_CONSUMER` 발생 수**(정상 부하에서 **0**이어야 한다. 0이 아니면 용량이 아니라 소비자가 문제다) | `/debug/stats` + DB의 `close_reason` 분포 | qa | AC-18(e), U-18 | E5, E6 |
| SC-74 | DB 중단 구간에도 **스냅샷은 계속 흐른다**(이동은 DB에 의존하지 않는다) | D 단계에서 중단 구간에 봇이 받은 스냅샷 수 > 0, 끊긴 연결 0 | qa | AC-18(f), I-31 | E2, E6 |

### K. QA — 성능 회귀 (AC-19)

| ID | 검증 항목 | 검증 방법 | 담당 | 근거 AC | 불가 |
|----|----------|----------|------|--------|------|
| SC-75 | **tick 초과 비율(`run_tick` 본문 > 50 ms) ≤ 0.5 %** — 이것이 성능에서 **유일한 판정 항목**이다 | `tick_overrun_total / tick_total`. 나머지(본문 분포·왕복·큐·RSS)는 M-1~M-6으로 **기록**하고, 눈에 띄게 나빠진 항목에 **원인 가설**을 적는다 | qa | AC-19 | E5, E7 |

### L. QA — 기록 무결성·커버리지·경계면 (AC-20, AC-21)

| ID | 검증 항목 | 검증 방법 | 담당 | 근거 AC | 불가 |
|----|----------|----------|------|--------|------|
| SC-76 | 수집한 **`ship_id` 집합**에 대해 `SHIP_SPAWNED` 수 == `SHIP_DESPAWNED` 수 == 집합 크기, 짝 없음 0, 중복 0 | `tests/e2e/check_ship_pairs.py`. **`ship_id` 기준**(correlation 아님 — §0.6). 집합 크기를 적는다 | qa | AC-20(a), I-41 | E2, E6 |
| SC-77 | `SESSION_OPENED`/`SESSION_CLOSED`는 **p0-02 그대로 correlation 기준**으로 짝이 맞는다 | p0-02의 `check_sessions.py` 재사용 | qa | AC-20(b) | E2, E6 |
| SC-78 | 재개가 일어난 함선에 대해 **세션 쌍은 2개인데 스폰은 1건**임을 보인다 | 그 `ship_id`의 이벤트 전체와 관련 correlation 2개를 나란히 출력 | qa | AC-20(c), I-29 | E2, E6 |
| SC-79 | `(world_id, tick)`별 `sequence`가 0..n−1로 빈틈없다 | p0-02 AC-16(c)의 SQL 그대로. **전 테이블 대상** | qa | AC-20(d), I-18 | E2 |
| SC-80 | **`domain_events`에 위치 시계열이 없다**: `event_type` distinct가 이 슬라이스의 **4종**(`SESSION_OPENED`·`SESSION_CLOSED`·`SHIP_SPAWNED`·`SHIP_DESPAWNED`)뿐이고 위치·상태 시계열 타입이 **하나도 없다** | **`down -v` 게이트(G-a)를 썼으면 전수로**, 아니면 **이번 실행의 tick 구간으로 한정**해서. **어느 쪽을 썼는지 리포트에 적는다**(§0.2). 이 PC에는 p0-02의 `QA_APPEND_ONLY_PROBE` 1건이 정당하게 남아 있다 | qa | AC-20(e), I-31·I-21 | E2 |
| SC-81 | 모든 `SHIP_*`의 `causation_id`가 **비-null이고, 자기 자신이 아닌, 정해진 타입의, 먼저 발행된 실제 이벤트를 가리킨다** — `SHIP_SPAWNED` ← `SESSION_OPENED`, `SHIP_DESPAWNED` ← `SESSION_CLOSED`, `≠ self`, 원인의 `(tick, sequence)` < 결과. **세션 간선(7차)**: `SESSION_CLOSED{SUPERSEDED}` ← **새 세션의 `SESSION_OPENED`**(같은 actor, 다른 correlation, **같은 tick, 더 작은 sequence**), **그 밖의 사유의 `SESSION_CLOSED` 와 모든 `SESSION_OPENED` 의 `causation_id` 는 null** | 도구 `tests/e2e/ship_events.py ledger`(구간 없이 = (b), 구간 주면 = (c)) + `causation-selftest`(합성 17행을 CTE 로 덮어 같은 SQL 을 돌린다 — DB 쓰기 없음. 정상 넘겨받기 결함 0, 틀린 모양 10종이 각자 정해진 결함 종류로 잡혀야 한다). **동결된 결함 장부로 판정한다(7차 개정 — tick 구간 한정이 아니다)**: (a) 검사는 **존재 여부만 보지 않는다** — 타입 + ≠ self + 순서를 함께 본다. *존재만 보는 조인은 자기 참조를 통과시킨다(실측: 0건을 냈다)* (b) **표 전체의 결함 집합 == 장부 7건**(스펙 §11-8: tick 395902 seq 8~13 의 6건 + tick 430030 seq 0 의 1건, 전부 `SHIP_DESPAWNED{SERVER_SHUTDOWN}` 자기 참조). 하나라도 더 있으면 새 결함 → FAIL, 하나라도 빠지면 추가 전용(I-20) 위반 또는 도구 결함 → FAIL. 등식이라 장부 행의 불변성까지 함께 검사된다 (c) **그 라운드 인스턴스 구간에서는 결함 0건** (d) **도구 자기 검증은 양방향** — 현재 DB 에서 정확히 7건(알려진 위반 검출), 블록 5 구간(403435~427104)에서 0건. 검사한 행 수를 적는다. 장부 행은 지우지도 고치지도 않는다(원칙 5). 소비자 규칙: 자기 루프는 순환이 아니라 **"원인 미상(결함 기록)"** 으로 읽는다(스펙 §11-8) | qa | AC-20(f), I-30 | E2 |
| SC-82 | `check_contract_coverage.py --strict` → **종료 코드 0(errors 0, warnings 0)** | 기준선 **errors 14 / warnings 0**(qa 실측)이 0이 되는 것이 통과 조건. **S1·C1·Q2 완료 후에만 실행**(게이트 G-l) | qa | AC-21(a) | E9 |
| SC-83 | 신규 **7타입**의 스키마·Rust·C# DTO를 **필드별 표**로 비교 → **불일치 0**. 열거 규약: envelope 필드 전부(`payload` 컨테이너 포함) + payload 자체 필드, **배열 원소 타입(`ShipState`)은 별도 표**. 데이터 3종은 C# 열이 없다. **대조한 행 수와 불일치 수를 적는다** | `tests/e2e/interface_matrix.py`(p0-02 자산) 확장. **총계가 아니라 표가 근거다** | qa | AC-21(b) | E9 |
| SC-84 | fixture의 `world_id`↔`tick_hz` 조합이 I-19와 모순되지 않는다 | p0-02의 스캔 스크립트 재사용. 위반 0, 스캔한 fixture 수를 적는다 | qa | AC-21(c), I-19 | — |
| SC-85 | **QA 도구 자체 검증**: 봇 계측이 틀렸을 때 빨간불이 켜지는가 — §3.3의 고장 주입 8종이 전부 검출된다 | `cd tools/bots && cargo test`. **p0-02에서 이 항목이 라운드 1을 구했다**(거짓 FAIL 2건·미검출 위험 1건) | qa | §3.3, I-25 | E1 |
| SC-86 | **e2e 스크립트 자체 검증**: 짝짓기 키를 `correlation_id`로 되돌리면 **재개 시나리오에서 실패한다**는 것을 합성 데이터로 보인다 | `tests/e2e`의 자체 테스트. **§0.6의 규칙이 실제로 무언가를 막는다는 증거** | qa | §0.6, I-41 | — |

---

## 2. 경계면 비교표의 고정 대상 (SC-83의 채점 기준)

신규 **7타입**. 행 목록은 스키마에서 기계적으로 뽑고, 총계가 아니라 **표**가 근거다(AC-21(b)).

| 타입 | envelope | payload | 배열 원소 | C# 열 |
|------|---------|---------|---------|-------|
| `SET_SHIP_CONTROL` | command envelope 5 + `payload` | `input_seq`, `thrust_x/y/z_milli`, `roll_milli`, `aim_x/y/z/w_micro`, `brake`, `flight_assist` | — | 있음 |
| `WORLD_SNAPSHOT` | message envelope 5 + `payload` | 월드부 `star_system_id`·`soft/hard_boundary_radius_mm`·`snapshot_interval_ticks`·`ships` + 세션부 `controlled_ship_id`·`ack_input_seq` | **`ShipState` 18필드 별도 표** | 있음 |
| `SHIP_SPAWNED` | event envelope 11 + `payload` | 스폰 payload | — | 있음 (`actor_id`·`causation_id` **좁힘**) |
| `SHIP_DESPAWNED` | event envelope 11 + `payload` | `ship_id`·`last_session_id`·`despawn_reason`·위치 3 | — | 있음 (같은 좁힘) |
| `SHIP_CLASS` · `STAR_SYSTEM` · `SYNC_TUNING` | (데이터, envelope 없음) | 스키마 전체 | 스폰 `points_m` 등 배열은 별도 행 | **없음**(생성 안 함 — AC-10(d)) |

**정수 행**: 위치 `mm`(±1e12, `long`/`i64`), 속도 `mm/s`(±1e8, `int`/`i32`), 각속도 `mdeg/s`(±3.6e6, `int`/`i32`), 쿼터니언 성분 `micro`, 조작축 `milli`(±1000), `input_seq`(≥1). **각 언어 타입이 스키마 범위를 손실 없이 담는가**를 본다.

**좁힘 행**: `SHIP_SPAWNED`·`SHIP_DESPAWNED`의 `actor_id`·`causation_id`가 **비-null**(Rust 비-`Option` / C# `Guid` + `Required.Always`). **`causation_id` 좁힘은 이 프로젝트의 첫 사례**이고 client 실측으로 생성기 수정 없이 동작한다.

---

## 3. QA 소유 도구의 확장 설계 (구현은 Phase 4 — Q2·Q3)

p0-02의 `tools/bots/`(Rust, 독립 와이어 타입)와 `tests/e2e/`(Python)를 확장한다. **계약 타입은 이번에도 독립으로 쓴다** — 봇은 3자 대조의 독립 축이고, 서버와 같은 타입을 쓰면 서버가 틀렸을 때 봇도 같이 틀린다(I-25, T12 지시 철회 확인됨).

### 3.1 봇 하네스 확장 (`tools/bots/`)

| 기능 | 내용 | 쓰이는 항목 |
|------|------|-----------|
| `SET_SHIP_CONTROL` 송신 | 주기 인자화(`--send-hz`, 기본 20). **양자화 정수를 봇이 직접 만든다**(서버 헬퍼를 쓰지 않는다) | SC-25~27, SC-61~62, J 전체 |
| 스냅샷 수신·검증 | `ship_id` 오름차순 위반 수, `tick` 간격 분포, `controlled_ship_id` 존재, `presence` 전이, `ack_input_seq` 단조성 | SC-28~32 |
| **스냅샷 CSV** | **client와 표기까지 같다**(client ack §⑨ 확정): 헤더 행 **있음**, 컬럼 순서 `tick,observer_actor_id,ship_id,presence,px_mm,py_mm,pz_mm,vx_mm_s,vy_mm_s,vz_mm_s` 고정, UUID는 **정규 소문자 하이픈 36자**, `tick`은 **envelope의 서버 tick**(프레임 카운터 아님), 좌표·속도는 **양자화 정수 그대로**, 1행 = 스냅샷 1건 × 함선 1척(자기 함선 포함), UTF-8 BOM 없음 `\n`. **`observer_actor_id` = 그 관측자가 `SESSION_READY`에서 통보받은 자기 `actor_id`**. **조인 키는 `(tick, ship_id)`**. 경로는 `--snapshot-csv` | SC-61~65, SC-11 |
| 수신 바이트 독립 계측 | 각 WS 텍스트 프레임의 **바이트 길이**를 합산. 서버 메트릭과 무관한 출처 | SC-70~72 |
| 치트 케이스 7종 | `position-injection` · `attitude-injection` · `out-of-range` · `flood`(10×) · `seq-rewind` · `hijack-linger` · `pre-ready` | SC-23~24, SC-66~68 |
| designer 지표(봇 관점) | `path_length_m`/`displacement_m`, `time_to_first_contact_s`, `nearest_ship_distance_m`, `brake_uses_per_minute`, `full_thrust_time_ratio` — **서버가 계산하고 서버가 검사하면 I-25 위반**이므로 봇이 독립 계산한다 | M-7, M-8 |
| 이동 거리 계산 | 스냅샷 위치 정수에서 계산(서버 메트릭 아님) | SC-25, SC-61 |

기존 산출물(`summary.json`·`commands.csv`·`sessions.json`·`correlations.txt`·`correlations.live.txt`)은 유지하고 `snapshots.csv`·`bandwidth.json`을 더한다.

**client 요청 2건을 받는다**: ① 봇도 `presence` 컬럼을 낸다(위 형식에 이미 포함). ② **`ships`가 빈 배열인 스냅샷은 CSV 행이 하나도 생기지 않으므로** "관측 실패"와 "월드가 비었음"을 구분할 수 있게 봇도 요약 한 줄(`WORLD_SNAPSHOT tick=<n> ships=0 …`)을 남긴다.

### 3.2 e2e 스크립트 확장 (`tests/e2e/`)

| 스크립트 | 항목 | 비고 |
|---------|------|------|
| **`ship_events.py`** (신규 — 하위 명령 `pairs`·`causation`·`resume`·`types`·`selftest`) | SC-14·76 / SC-08·09·12·81 / SC-78 / SC-80 / **SC-86** | **`ship_id` 기준 짝짓기**(p0-02 의 `check_sessions.py` 와 **다른 키**). 네 검사가 같은 DB 접근과 tick 구간 인자를 공유해 한 파일로 묶었다. `selftest` 가 "키를 correlation 으로 되돌리면 재개에서 깨진다"를 합성 데이터로 보인다 |
| `two_client_view.py` (신규) | SC-61~65 | 두 CSV를 envelope `tick`으로 조인. **S-3 부호 판정 포함** |
| `bandwidth.py` (신규) | SC-70~72 | 봇 실측 vs `snapshot_bytes_total` 델타 vs ADR-0011 §2 |
| `perf_compare.py` (신규) | SC-75, M-1~M-6 | p0-02 기준선과 **같은 표**로. `snapshot_build_us` 분리 |
| `check_sessions.py` · `check_sequence_gaps.py` · `poll_until.py` · `three_way_load.py` · `interface_matrix.py` · `append_only_probe.py` · `db.py` · `run_block.py` | SC-77, SC-79, SC-82~84, 블록 실행 | p0-02 자산 재사용(스크립트 자체 결함 3건은 p0-02 라운드 1에서 수정됨) |

### 3.3 **도구가 틀렸을 때 빨간불이 켜지는가** (SC-85·SC-86)

p0-02에서 이 항목이 라운드 1을 구했다(`close_initiator` 거짓 FAIL, tick 비교 부재, 기대값 오류 3건). 이번에 주입할 고장:

| # | 주입 | 기대 검출 |
|---|------|---------|
| 1 | 가짜 서버가 스냅샷 1건을 빠뜨린다 | 스냅샷 손실 1로 계수(SC-70의 검출력) |
| 2 | `ships`를 `ship_id` 내림차순으로 보낸다 | 정렬 위반 수 > 0 (SC-29) |
| 3 | `ack_input_seq`를 한 번 되돌린다 | 단조 위반 1 (SC-31) |
| 4 | 위치 정수를 1 mm 어긋나게 보낸다 | 두 관측자 조인에서 불일치 1 (SC-63) |
| 5 | 프레임 바이트를 부풀린다 | 대역폭 계측이 그만큼 커진다(SC-71의 독립성) |
| 6 | `presence`를 `LINGERING`으로 고정 | 전이 검사가 잡는다 (SC-32) |
| 7 | 스냅샷 간격을 3 tick으로 | 간격 위반 수 > 0 (SC-28) |
| 8 | **짝짓기 키를 `correlation_id`로 되돌린다** | **재개 시나리오 합성 데이터에서 실패한다** (SC-86, §0.6) |
| 9 | CSV 왕복 | 기록한 CSV를 다시 읽어 조인하면 같은 값(형식 자체의 무결성) |

### 3.4 designer 시나리오 S-1~S-8 매핑

| 시나리오 | 대응 항목 | 비고 |
|---------|---------|------|
| S-1 혼자서도 움직임이 읽힌다 | SC-19(관성), SC-59(b 마커) + **M-16 기록**(0→140 4.0±0.3초, 미끄러짐 1,400±150 m, 브레이크 2.8±0.3초/196±25 m) | 수치는 designer 정본. **사람 조작이라 판정이 아니라 기록**이고, 벗어나면 designer 재조정 신호 |
| S-2 브레이크가 결단이 된다 | **M-16 기록**(500 m에서 제동 → 앞 ~300 m 정지 / 150 m → 지나침) | 같은 이유로 기록 |
| S-3 A가 움직이면 B가 본다 | **SC-61·62·63(판정) + SC-64(정지 ≤2 m) + SC-65(이동 28±12 m 뒤쪽, 부호 핵심)** | **이 슬라이스의 최우선 판정.** designer가 단일 "≤30 m" 기준을 둘로 쪼갠 이유를 §0.4 각주에 적는다 |
| S-4 클라이언트가 위치를 주장할 수 없다 | SC-23, SC-24, SC-17(대각선), SC-25~27(폭주) | 치트 5종이 전부 항목화됨 |
| S-5 끊겨도 세상이 찢어지지 않는다 | SC-11(재개), SC-12(만료 디스폰), SC-32(LINGERING), SC-60(d) | |
| S-6 벽이 있지만 벽처럼 보이지 않는다 | SC-18(적분), SC-59(c 육안) | **텔레포트·튕김·하드 스냅 0회**는 SC-56(c)의 `reconcile_hard_snap_total = 0`과 함께 본다 |
| S-7 30척이 있어도 같은 세상이다 | SC-69~75, M-1~M-8 | 회귀 게이트는 SC-75 하나 |
| S-8 두 언어가 같은 물리를 계산한다 | SC-34(Rust 결정성) + SC-51~55(C# 예측 일치) | **독립 출처 검증**(I-25). 둘이 같은 입력열을 쓴다 |

---

## 4. 실행 순서와 게이트

| 게이트 | 조건 | 어기면 |
|-------|------|-------|
| **G-a** | **`docker compose down -v`를 쓸지 먼저 정한다.** 쓰면 측정 세션의 첫 DB 단계여야 하고, 안 쓰면 SC-80을 tick 구간 한정으로 판정한다 | 그 이전 DB 증거 무효. 어느 쪽인지 안 적으면 SC-80이 해석 불가 |
| **G-b** | 기대 숫자는 **`contracts/fixtures/`에서만**. `data/`는 SC-05~07과 실서버 해석에만 | designer가 튜닝하는 순간 테스트가 빨간불 |
| **G-c** | **측정 중 어떤 빌드도 돌리지 않는다.** AC-8이 자식 프로세스를 띄워 p0-02보다 경합이 크다 | 성능 기록 전체가 무효 |
| **G-d** | 31번째 연결(Unity)이 **먼저** 붙은 뒤 봇 시작 | 31이 30이 되어 AC-18이 조용히 약해진다 |
| **G-e** | **A 단계가 도는 동안 `client/` 아래 파일을 저장하지 않는다** | 도메인 리로드가 31번째 연결을 끊는다 |
| **G-f** | 치트·프레이밍 시나리오는 부하와 **분리** 실행 | 강제 close가 "서버가 먼저 닫은 연결 0"을 깨뜨린다 |
| **G-g** | 적분·결정성(D절, SC-34)은 **메모리**(`Simulation::new(world, 0)`)에서 | 실서버 `last_tick = 350280` 때문에 tick 0 기준 기대값이 맞지 않는다 |
| **G-h** | **판정 전에 `close_reason`을 먼저 본다** | 송신 큐 2.13초 < Editor 히치면 `SLOW_CONSUMER`가 정당하다 — 서버 버그로 오독 금지 |
| **G-i** | 잔류·재개(SC-11)와 만료 디스폰(SC-10·SC-12)은 **30초 창 + 여유**가 필요하다. B 단계에 넣고 시간 예산을 잡는다 | 30초를 기다리지 않으면 두 항목이 구조적으로 측정 불가 |
| **G-j** | 만료 후 재스폰(SC-10 후반)은 `linger_seconds` **경과 후**에만 | 30초 안에 재면 재개를 스폰으로 오판한다 |
| **G-k** | AC-8 산출물(입력열·스냅샷)은 **C4가 읽는다**. 복사하지 않고 상대 경로로 참조 | 사본이 갈라지면 server와 client가 다른 것을 본다 |
| **G-l** | SC-82(커버리지 `--strict`)는 **S1·C1·Q2 완료 후** | 그 전 결과는 판정에 쓰지 않는다(기준선 14/0) |
| **G-m** | SC-83(경계면)은 S1 + C1 완료 후 | 한쪽만이면 **대기** |

**권장 블록 순서**

| 블록 | 내용 | 항목 |
|------|------|------|
| 0 | 계약·빌드(서버 불필요) | SC-01~04, SC-35~40, SC-41~50, SC-85 |
| 1 | (선택) `down -v` → 인프라 → 데이터·기동 거부 | SC-05~07 |
| 2 | 메모리 적분·결정성 | SC-15~22, SC-34 |
| 3 | 서버 단독 실서버(부하 없음): 스폰·인과·스냅샷 규약 | SC-08~09, SC-28~33 |
| 4 | 치트·프레이밍 | SC-23~24, SC-66~68 |
| 5 | 잔류·재개·디스폰 (30초 창) | SC-10~14, SC-32 |
| 6 | Unity 실서버·예측·육안 | SC-51~60 |
| 7 | 2 클라이언트 가시성 (S-3) | SC-61~65 |
| 8 | 부하 A·B·C·D + 성능 | SC-25~27, SC-69~75 |
| 9 | 기록 무결성·커버리지·경계면 | SC-76~84, SC-86 |

---

## 5. 기록 항목 (판정하지 않음)

| # | 기록할 것 | 담당 | 근거 |
|---|----------|------|------|
| M-1 | tick 본문 소요 p50·p99·max + **`snapshot_build_us` 분리**("스냅샷 조립 X µs / 나머지 Y µs") | qa | AC-19, B-14 |
| M-2 | 왕복 p50·p99 (p0-02: **50.4 ms**) | qa | AC-19 |
| M-3 | 큐: `send_queue_bytes` max·비율, `command_queue_depth` max (p0-02: 0 / 141) | qa | AC-18(e) |
| M-4 | 서버 RSS (p0-02: **16.6 MB**) | qa | AC-19 |
| M-5 | `tick_lag_seconds` + **Windows 바닥값 +0.7 %/분** 병기 | qa | §11-6 |
| M-6 | 대역폭 실측: 세션당 KiB/s, 합계 Mbit/s (ADR 산출 161.8 / 42.5) | qa | AC-18(c) |
| M-7 | **designer 재조정 1순위**: `brake_uses_per_minute`(목표 ≥ 2), `full_thrust_time_ratio`(목표 0.5~0.8) — **보조 감쇠 7 m/s²가 맞는지는 이 두 수치가 말한다** | qa | 설계 §9.1, designer 요청 |
| M-8 | 나머지 designer 지표: `nearest_ship_distance_m` p50(<3,000), `ships_within_2000m` p50(≥2), `ship_speed_mps` p95(≥130), `time_to_first_contact_s` p50(<90), `path_length/displacement`(1.5~4.0), 경계 접촉 수 | qa | 설계 §9.1 |
| M-9 | `reconcile_correction_m` p99(<0.25 m), `reconcile_hard_snap_total`(0), `input_carry_forward_ticks_total`, `input_superseded_total` | qa, client | 설계 §9.2 |
| M-10 | 클라이언트 할당량: **스냅샷 1건당 + 초당**(U-16, p0-02 U-10의 만기) | client | AC-13(e) |
| M-11 | C# "감지 불가" **10건** 목록 — 설계 비대칭으로 그대로 기록 | qa | §5.4 |
| M-12 | 자세 안착 tick 수 (U-21) | server | AC-4(g) |
| M-13 | AC-8 결정성: 비교한 tick 수와 총 바이트 | server | AC-8 |
| M-14 | **부호를 증명하지 못한다**는 사실 (§0.10). SC-59가 실행되지 않았으면 "부호 미검증"을 요약에 명시 | qa | §11-7 |
| M-15 | 측정 환경: Unity Editor 상태, 컨테이너 수, 시드, 빌드 프로필, 단계 시각 | qa | §11-5 |
| M-16 | designer 시나리오 S-1·S-2의 관측치(가속 시간, 미끄러짐 거리, 제동 거리) | qa, client | 설계 §8 |
| M-17 | **신규 경로 실행 여부 표**(§7). `world_full` 거부와 `commands_dropped_over_tick_cap_total` 은 **정상 부하에서 0**이라 별도 시나리오로 태워야 실행된다 | qa | architect 결정 2026-09-20 |

---

## 6. "미검증(환경)" 처리 기준

| 코드 | 조건 | 영향 | 표기 |
|------|------|------|------|
| E1 | cargo 툴체인 사용 불가 | A·D·E·F절 | 미검증(환경) |
| E2 | Docker/PostgreSQL 미가동, 포트 점유 | B·C·L절 | 미검증(환경) |
| E3 | Unity Editor 라이선스·CLI 실패 | G·H절 | 미검증(환경) — CLI 로그 첨부 |
| E4 | .NET SDK 미가용 | SC-41~45 | 미검증(환경) |
| E5 | 서버가 **환경 문제로** 기동 못 함 | 실서버 항목 | 미검증(환경) — `netstat` 첨부 |
| E6 | 봇 하네스 빌드 불가 | E·I·J절 | 미검증(환경) |
| E7 | **31 연결 + Unity + Docker 동시 실행 자원 부족** | J·K절 | 미검증(환경) — 관측 수치 기록 |
| E8 | **한 프로세스 2세션 경로(§0.11)가 동작하지 않음** | SC-64·65 | SC-64는 봇으로 대체 가능(사실을 적는다). **SC-65는 미검증(환경)** — 봇 대체 시 부호가 뒤집혀 채점이 거꾸로 된다 |
| E9 | 선행 태스크 미완 | SC-82·83 등 | **대기**(판정 제외) |

**환경 문제와 구현 부재를 섞지 않는다. 구현이 없으면 FAIL이다.**

---

## 7. 판정·라운드 규칙

- 라운드 **최대 3회**. 결과는 `04_qa_report_r{N}.md`.
- FAIL은 **파일:라인 + 재현 명령 + 기대/실제**를 담아 담당자에게. QA는 구현 코드를 고치지 않는다(`tools/bots/`·`tests/e2e/`만 QA 소유).
- **§0.5 표와 결과가 다르면 FAIL이 아니라 architect 통지**(계약 설계가 바뀐 것).
- **SC-51·52가 임계를 넘으면 임계값을 늘리지 않는다** — architect에게 알린다(ADR-0010 §3의 "같은 비트" 가정이 반증된 것이다).
- 리포트는 **정확성 판정 / 성능 기록 / designer 지표**를 별도 절로 쓴다.
- **요약에 §0.10(부호 미증명)을 반드시 싣는다.**
- **신규 경로마다 "실제로 탔는가"를 표로 적는다**(architect 결정 2026-09-20). server 가 `world_full` 게이트를 만들고도 **한 번도 실행된 적이 없었다**는 것을 뒤늦게 발견했다(테스트 하네스가 `world_capacity` 를 설정하지 않았다). **통과한 테스트 수는 경로가 실행됐다는 증거가 아니다.** 최소 다음을 **실행됨 / 미실행**과 근거(카운터 델타 또는 이벤트 행)로 적는다: 스폰 · 재개 · 잔류 만료 디스폰 · 종료 디스폰 · `world_full` 거부 · 경계 soft/hard · 브레이크 · 퇴화 쿼터니언 · 이월 만료 · **tick 상한 초과**.

### 7a. 관측 자신에 대한 규율 — **초록불이 무엇을 보고 켜졌는가** (6차 개정, architect R3)

> **관찰이 겨냥한 조건이 실제로 발생했는지를 함께 단언하지 않으면 초록불은 아무것도 뜻하지 않는다.
> 카운터 항등식은 입력이 전부 0일 때 반드시 실패해야 한다.**

위 M-17 표("신규 경로가 실제로 탔는가")를 **게이트·스크립트·테스트 자신에게** 적용한 것이다. 적용 방법:

- 항등식(`A == B`)을 판정에 쓸 때마다 **"그 경로가 탔다"는 단언을 짝짓는다**(`A > 0`, 비교한 건수 > 0, 조건이 실제로 발생했다는 관측). 짝이 없는 항등식은 PASS 근거로 쓰지 않는다.
- 부하·고장 조건을 **만들어서** 재는 관측은 **그 조건이 만들어졌음**을 판정 조건에 넣는다. "N회 연결했다"·"N초 돌렸다"는 수단이지 조건이 아니다.
- 벽시계로 인과적 성질을 대신 재지 않는다. 재야 한다면 **측정 구간에 우리 쪽 비용(폴링·HTTP 왕복)이 섞이지 않는지**를 먼저 따진다.
- 새 관측은 **RED를 먼저 한 번 보인다** — 수정 전 동작이나 고장 주입에서 실제로 빨간불이 켜지는가.

**이 슬라이스에서 이 규율이 없어서 생긴 네 사례** (전부 실제로 일어났다):

| 사례 | 무엇이 초록이었나 | 실제로 일어나지 않은 것 | 고친 짝 |
|---|---|---|---|
| `world_full` (라운드 1 전) | 게이트 구현·테스트 통과 | **한 번도 실행되지 않았다** — 테스트 하네스가 `world_capacity` 를 설정하지 않았다 | M-17 표 (§7) |
| `SET_SHIP_CONTROL` (라운드 2) | 관련 테스트 **154개 초록** | 실서버에서 **시뮬레이션 도달 0** — `commands_received_total` 델타 0, `UNKNOWN_COMMAND_TYPE` +200 | 실서버 카운터 델타로 판정 (qa r3 §3.1) |
| 봇 짝 게이트 (라운드 2) | `accepted == replies_total` | **`0 == 0`** — 수락이 0건일 때만 통과하는 검사였다 | 3단언 + **`accepted > 0`** (스펙 §5.1a, qa r3 §5.1) |
| 벽시계 flaky (`graceful_client_close_is_prompt`) | `elapsed < 2s` 가 대체로 통과 | 서버 종료 지연이 아니라 **우리 폴링 비용**(50 ms 수면 + HTTP 왕복)을 쟀다. *7차 정정: 고친 폴 횟수도 "서버가 열려 있다고 답한 횟수"가 아니라 **"`ws_connections` 게이지가 아직 0 이 아니었던 횟수"** 다 — 게이지를 내리는 태스크가 굶으면 소켓이 닫혔어도 오른다. "실제로 늦게 닫힘"과 "게이지만 늦음"은 폴로 구분되지 않는다(architect R3 추가 판정 사안 3)* | **폴 횟수 ≤ K** 로 단언 (architect R3 판정 1, SV-1) |

같은 규율을 AC-2(i) e2e(라운드 3 리포트의 이름은 AC-2(h-log)) 에도 적용한다 — **파이프가 실제로 찼음**(싱크가 버린 줄 > 0 또는 미드레인 누적 ≥ 파이프 용량)을 판정 조건에 넣고, 연결 횟수는 수단으로만 쓴다.

### 7b. 자명 통과 시험 — **새 SC 를 쓸 때마다 거치는 단계** (9차 신설, architect 제안)

§7a 가 **"왜"** 라면 이것은 **"어떻게"** 다. 지금까지는 그때그때 *발견*했고, 그래서 **한 라운드에 같은 누락을 네 번** 놓쳤다.

> **1. 새 항목의 각 절 옆에, "이 절을 자명하게 통과시키는 상태"를 한 줄로 적는다. 그 상태가 실제로 가능하면 절을 고친다.**
> **2. 판정 기준은 관측 전에 적는다 — 옳은지 확신하지 못해도 적는다.**
> **3. 판정 기준을 만드는 사람도 자기 기준에 1번을 돌린다.**

**2번이 이 규칙을 쓸 수 있게 만든다.** "옳은 기준을 미리 적어라"를 요구하면 **기준을 쓰는 단계에서 막힌다**(옳은지 어떻게 아는가?). **"미리 적어라"는 누구나 지금 할 수 있고, 틀렸을 때조차 어긋남이 신호가 된다** — 관측 뒤에 기준을 정하면 결과에 맞춰 기준이 휘고, 그러면 아무것도 대조하지 못한다.

**실제로 그렇게 작동했다**: r4 §5.13 의 판별 기준은 **성립 불가능한 조건**이었지만(아래 "자명 불통과"), **관측 전에 적혀 있었기 때문에 어긋남이 즉시 드러났고** 그것이 서버의 두 층 구조와 1 : 6 : 2 유도를 끌어냈다. **관측 전에 적어 두는 것의 값은 기준이 옳은지와 별개다.**

**이 시험이 실제로 잡았을 네 건** (전부 SC-89 를 쓰면서 뒤늦게 발견한 것이다):

| 절 | 자명 통과 상태 | 고친 결과 |
|---|---|---|
| (a) | **위반 7회가 쌓였지만 예산이 안 참** — 끊김은 예산 8이 찬 뒤에야 나므로 `close_reason` 0건이다 | `commands_dropped_over_tick_cap_total`·`RATE_LIMITED` 델타 0 을 추가 |
| (b) | **히치가 한 번도 없었음** / **pause 라 위반 1회뿐** | 드레인 max > 1 **그리고** 송신/프레임 == 1 (판별 단언) |
| (d) | **두 성질 중 하나만 성립** | 성질을 쪼개 관측으로 내림 |
| **(g)** | **대조가 `RATE_LIMITED` 만 건드리고 위반은 한 번도 안 만듦** — 그래도 끊기므로 겉보기엔 동작한다 | 판정을 `protocol_violations_total` 로 고정 |

**R3 판정의 `accepted > 0` 도, AC-2(i) 의 "버려진 줄 > 0" 도 같은 시험의 결과물이다.** 한 줄짜리 단계로 만들면 **발견이 아니라 절차가 된다.**

**이 시험은 SC 절 말고도 적용된다.** 같은 라운드에서 두 번 더 쓰였다:

| 대상 | 자명 통과 상태 | 고친 결과 |
|---|---|---|
| **qa 가 제안한 "빈 번호·중복 검사"** (SC-87 이 세 라운드 동안 표에 없던 것을 막으려는 게이트) | **"계약에 없는 항목의 번호가 연속 범위 밖이다."** r2 가 그 검사를 SC-90 으로 붙였다면 빈 번호가 안 생겨 **검사는 초록**이었다 — 이번에 통한 것은 SC-87 이 **우연히 86 과 88 사이에 있었기 때문**이다 | **방향을 뒤집었다**: **리포트가 판정한 모든 `SC-\d+` 가 계약 표에 존재하는가**(＋ 순방향으로 계약의 모든 항목이 판정·대기로 분류됐는가). 번호 배치와 무관하게 **r2 당시에 즉시** 잡았을 형태다 |
| **판정 전에 기준을 지정하는 것** (SC-89 (g) 실행) | 기준을 **실행 뒤에** 정하면 **"끊겼다 = 대조 성공"** 으로 읽는다 | **실행 전에** "이 결과를 자명하게 통과시키는 상태"를 적어 두면 **결과를 볼 때 그 줄이 대조군이 된다.** 리더가 실행 전에 지정한 판별 기준이 없었다면, **그 함정을 경고한 qa 자신이 그 함정에 걸렸을 것이다**(r4 §5.13) |

**둘째 줄이 규칙 2번의 사례다** — 기준이 관측 전에 적혀 있었기에 결과를 볼 때 그 줄이 대조군이 됐다.

**⚠ 시험은 양쪽을 본다 (9차 보충, r4 §5.15).** "자명하게 통과시키는 상태"만이 아니라 **"어떤 입력으로도 만족될 수 없는 상태"** 도 함께 본다.

| 보는 것 | 뜻 | 사례 |
|---|---|---|
| **자명 통과** | 초록불이 공짜다 | SC-89 (a)(b)(d)(g) — 위 표 |
| **자명 불통과** | **빨간불이 영원하다** | "`RATE_LIMITED` 가 틱상한 드롭보다 훨씬 작을 것" — **두 층이 같은 단위(tick 당 건수)를 보므로 어떤 입력으로도 만족되지 않는다.** 이 기준은 **관측 전에 지정됐고 실제로 (g) 의 오류를 드러냈지만, 기준 자신은 성립 불가였다**(r4 §5.13·§5.15) |

**둘 다 "그 절이 아무것도 재지 않는다"는 뜻이다.** 그리고 **판정 기준을 만드는 사람도 자기 기준에 이 시험을 돌린다** — 위 둘째 사례는 리더가 지정한 기준이었고, 시험을 돌렸다면 지정 시점에 잡혔다.

**⚠ 이 시험은 검사 자신에게도 적용한다.** (g) 는 **그 병을 잡으려고 만든 도구 안에서** 나왔다 — qa 가 쓴 양성 대조의 단위 테스트가 **서버를 모델하지 않는 수식**을 단언하고 있었고, 그 초록불은 아무것도 뜻하지 않았다(실측으로 뒤집혔다, r4 §5.13).

---

## 8. 구현자 확인란

| 영역 | 항목 | 담당 | 확인 | 서명/날짜 |
|------|------|------|------|----------|
| A 게이트·결정성 위생 | SC-01~04 | server | ☐ | |
| B 데이터·기동 거부 | SC-05~07 | server | ☐ | |
| C 스폰·잔류·재개·인과 | SC-08~14 | server | ☐ | |
| D 적분·서버 판정 | SC-15~24 | server | ☐ | |
| E 속도핵·스냅샷·결정성 | SC-25~34 | server | ☐ | |
| F 계약 ↔ Rust | SC-35~40 | server | ☐ | |
| G 생성기·EditMode | SC-41~50 | client | ☐ | |
| H 예측·실서버·육안·타함선 | SC-51~60 | client | ☐ | |
| I~L QA 전체 | SC-61~86 | qa | ☑ | qa / 2026-09-20 |
| 부록 | §0.5 층별 표, §0.6 짝짓기 키, §2 비교표, §4 게이트 | server·client 공통 | ☐ ☐ | |

**이의 제기**

| # | SC ID | 제기자 | 이의 내용 | 처리 |
|---|-------|-------|----------|------|
| 1 | | | | |
| 2 | | | | |
| 3 | | | | |

### 확인 쟁점 — **10건 전부 답변 반영 완료 (2026-09-20)**

`02_server_ack.md`(①②③④⑤⑩)와 `02_client_ack.md`(⑥⑦⑧⑨ + 숫자 정정 2건)의 답을 위 항목에 반영했다. 아래는 기록이다.

1. **SC-06의 기동 거부 8종을 어떻게 주입하는가.** `data/` 원본은 designer 소유라 건드릴 수 없다. **서버가 데이터 디렉토리를 인자·환경 변수로 받는가?**(`STARFALL_DATA_DIR` 같은) 없으면 QA는 임시 사본으로 시연할 방법이 없다. (server)
2. **SC-71의 전제**: `snapshot_bytes_total`이 **소켓에 실제로 쓴 바이트**인가, 큐에 넣은 시점인가? 파일:라인으로 답해 달라. 큐 시점이면 봇 실측과 **독립 출처가 아니다**(architect 지시 4). (server)
3. **SC-11의 재개 상태 검증**: 끊기기 직전 스냅샷 → N tick 적분을 QA가 독립 계산하려면 **추력 0·이월 만료 상태의 감쇠 규칙**만 알면 되는가? 그 외 상태(`flight_assist` 토글 등)가 재개 시 어떻게 초기화되는지 확정해 달라. (server)
4. **SC-33의 메트릭 이름 6종**을 확정해 달라(`/debug/stats`의 정확한 키). p0-02처럼 **라벨 배열**인지 스칼라인지도. (server)
5. **SC-28의 스냅샷 주기**: 세션이 **중간에 들어오면** 첫 스냅샷까지의 간격이 2 tick이 아닐 수 있다. 첫 스냅샷을 검사 대상에서 빼는가, 아니면 접속 tick 기준으로 정렬되는가? (server)
6. **SC-64·65(S-3)의 두 번째 관측자**: Editor 인스턴스를 2개 띄울 수 없으면 **봇을 B로 쓰는 것을 허용하는가?** 봇은 보간을 하지 않으므로 "두 화면 비교"가 "서버 값 vs A의 화면"이 된다 — 그 경우 SC-65의 28 m는 **A 쪽 보간 지연만** 반영한다. designer·client 확인 필요. (client, designer)
7. **SC-58의 프로파일러 측정 방법**: Editor 프로파일러로 "스냅샷 1건당"을 어떻게 분리하는가(마커를 넣는가)? (client)
8. **SC-59 육안 녹화의 보관 형식**(스크린샷 N장 / 짧은 mp4 / gif). 증거로 남길 형태를 정해 달라. (client)
9. **봇 스냅샷 CSV의 `observer_actor_id`**: 봇은 actor가 곧 자신이므로 자기 `actor_id`를 쓴다. **client도 같은 의미로 쓰는지** 확인해 달라(조인 키가 어긋나면 SC-63이 0행이 된다). (client)
10. **SC-25의 "한 tick 이동분"**: 140 m/s × 0.05 s = **7 m**를 기준으로 삼으면 되는가? 실제 속도가 낮으면 기준도 낮아져야 하는데, **그 시점 속도로 계산**하는 것이 맞는가 아니면 최대 속도 기준 고정인가? (server, designer)

---

## 9. 계약 변경 이력

| 날짜 | 변경 | 사유 |
|------|------|------|
| 2026-09-23 (9차, qa3) | **SC-89 신설 + architect 보충 판정 반영 3건.** **(1) SC-89** — "클라이언트의 송신 속도가 프레임률이 아니라 벽시계 tick 속도를 따른다". 절 여섯: (a) 프레임당 송신 ≤ 1 **＋ `commands_dropped_over_tick_cap_total`·`RATE_LIMITED` 델타 0**(문턱 바로 아래를 통과로 읽지 않기 위해 — 끊김은 예산 8이 찬 뒤에야 나므로 위반 7회가 쌓여도 `close_reason` 0건이다) · (b) `catchup_carry_forward_ticks_total > 0` **짝 단언**과 함께 `reconcile_hard_snap_total == 0` **＋ 한 `Update()` 최대 드레인 tick 수 > 1**(드레인 max 가 1이면 히치가 없었던 것이라 그 세션은 항목을 닫지 못한다) · (c) 판정 도구가 **사본이 아니라 진짜 코드**를 잰다 · (d) `outbound_queue_full_total` 은 **기록**(0이 아니어도 FAIL 아님 — 위반 세션의 9건 burst 는 전부 송신 성공이었으므로 **큐 포화는 이번 위반의 원인이 아니다**) · (e) **임계값 diff 0 기계 확인** · (f) 잔여 관측 둘이 0이 아니면 **미룬 결정의 발동 조건**. **"봇으로는 (a) 를 닫을 수 없다"와 "SC-59 와 같은 세션으로 촬영하지 않는다"를 항목에 넣었다.** **임계값은 하나도 바꾸지 않았고**(§7) **`contracts/` 변경 없음** → 라운드 1 실측 기반(스키마 18 / 유효 26→27 / 반례 34 / 레지스트리 13)과 SC-35~50·82~84 의 기대 숫자는 **그대로다**(재판정 불필요). **(2) SC-59 (b) 를 (b1) 배치 / (b2) 가시성으로 분할** — 한 항목이 두 사실을 재고 있었다. (b1)은 **EditMode 씬·데이터 단언(영상 불필요)**, (b2)는 **촬영 구간 전체에 최소 1개가 화면에**. "1개면 충분"으로 낮추면 마커가 3개 사라져도 통과한다. 방법 칸에 **`OnGUI` 는 비포커스면 호출되지 않는다**(client R8 — 라운드 4 에서 HUD 가 한 줄도 없던 이유)와 HUD 확인 2회(시작 직후·클립 a 직전)를 넣었다. **(3) §0.3 에 다섯 번째 칸 `미검증(증거 요건)` 신설** — **비-통과이며 슬라이스 종료를 FAIL 과 똑같이 막는다**(바꾸는 것은 귀속이지 게이트 강도가 아니다). **실패를 이 칸으로 옮기는 것은 금지**한다(R3 판정 1의 "간헐 실패도 FAIL"은 *테스트가 실패한* 경우, 이 칸은 *테스트가 유효하게 실행되지 않은* 경우) | architect `01_architect_decisions.md` `## R4 판정 — 히치 catch-up 버스트` §2·§5·§8·§10 + `## R4 보충 판정` §F·§G·§H·§I. 실서버 근거: qa3 r4 §5.2 (가)·§5.7·§5.8. **계약에 SC-89 의 집이 없었다는 것이 라운드 1~4 의 모든 PASS 와 이 버그가 양립한 이유다**(§7a) — 재는 것이 AC-18/SC-26(손실·상한)과 달라 그 계열에 붙이지 않았다. **(c) 는 architect 가 qa 판단에 맡긴 것을 qa3 가 넣었다**: 이 슬라이스가 "테스트가 사본을 검사한다"는 병에 네 번 걸렸고(§7a 표), grep 한 줄로 살 수 있는 보험이다. **qa3 가 R3 에서 의심했던 "`reconcile_error_deg` max 클램프·포화"는 오진이었다** — client 코드 확인 결과 `ReconcileErrorStats.Compute` 는 정상이고 **`OnSessionReady` 가 오차 리스트·퍼센타일·CL-2 카운터를 리셋하지 않는 반면 `reconcile_hard_snap_total` 만 `_controller` 재생성으로 우연히 리셋되는 범위 불일치**였다. architect 판정: **`…_session` / `…_run` 두 범위를 이름으로 구분해 둘 다 표시**하고, **SC-56 은 세션 범위로 읽되 실행 전체 값을 맥락으로 함께 적는다** |
| 2026-09-22 (8차, qa2) | **SC-01 에 넷째 명령 `cargo test -p starfall-sim --release --locked` 추가**(릴리스 전용 테스트가 `ok` 로 1건 이상 찍힐 것). **SC-45 해석 주석**: 7차의 `SESSION_CLOSED` 스키마 description 변경(`SUPERSEDED`)은 `SessionClosedEvent.cs` 의 XML 주석 1줄을 바꾼다 — 이것은 생성기 변경이 아니라 계약 데이터 변경의 결과이므로 "기존 6타입 바이트 동일" 에서 **스키마 description 에서 온 주석 줄**은 제외해 읽는다 | 리더 지적(§7a 계열): server 의 릴리스 팔 테스트(`#[cfg(not(debug_assertions))]`)는 표준 게이트 `cargo test --workspace` 에서 **한 번도 돌지 않는다**(qa2 실측: 디버그 3회 로그에 그 이름 0회, 릴리스 로그 1회). 넣지 않으면 누가 `return` 을 지워도 표준 게이트는 초록이다. qa2 가 레포 밖 복사본에 고장 2종을 주입해 두 단언이 각각 빨간불을 켜는 것을 확인했다(r4 §4.1). 이 슬라이스의 가장 중요한 원칙(원인을 지어내지 않는다, I-30)을 지키는 유일한 실행 증거라 **표준 게이트에 넣는다** — 비용은 릴리스 빌드 1회(sim 만, 이 PC 에서 수십 초) |
| 2026-09-22 (7차) | **architect R3 추가 판정 + 사용자 결정 5·6 반영.** (1) **SC-11 (3)** 문구 교체 — 와이어 층은 **양자화 봉투 + 음성 대조 필수**(qa 독립 계산, client C# 적분기), 비트 일치는 **(3′) `f64` 층**(server AC-3(d1) in-process 차분 테스트). 방법 칸의 "결정적이므로 허용 오차 없음" → "허용 오차를 고르지 않는다 — 봉투는 양자화에서 유도한다". (2) **SC-88 신설**(스펙 AC-3(h), I-29 동시 세션) — 사용자 결정 5 **(B) 나중 접속이 이어받는다**의 단언: 동시 함선 ≤ 1(수명 구간 겹침 0), 옛 세션 `SUPERSEDED` 이고 원인 = 새 `SESSION_OPENED`(같은 tick, sequence k < k+1), close **4001**, **밀려난 클라이언트 재접속 없음**, 자기 참조 0, 유령 `ACTIVE` 0. 판정 도구 `ship_events.py overlap` 의 양방향 자기 검증을 문구에 넣었다. `pairs` 의 actor 목록은 I-29 판정에 쓰지 않는다. (3) **SC-81** — tick 구간 한정 → **동결된 결함 장부**(7건) 등식 판정, 검사는 **타입 + ≠ self + 순서**(존재만 보는 조인은 자기 참조를 통과시킨다 — 실측). (4) **AC-2(h-log) → AC-2(i)** 참조 정정(스펙에서 번호가 바뀌었다). 도구 3곳(`log_pipe_backpressure.py`·`make_red_logsink_binary.py`·`tests/e2e/README.md`)도 고쳤다. `04_qa_report_r3.md` 는 과거 리포트라 본문을 고치지 않고 머리에 대응표 한 줄만 두었다. 증거 경로 `evidence/R3-B/AC2hlog/` 는 그대로 둔다. (5) **SV-1** — 폴 횟수의 뜻을 "게이지가 아직 0 이 아니었던 횟수"로 정정(§7a 표), server 의 "참 양성" 라벨 **철회(미증명)**. K=10 유지, in-process ×64/×80 은 **작동 범위 밖(판정 불능)**. 다시 여는 조건: 현실 조건에서 새 형태가 한 번이라도 실패 — 그때는 원인을 붙이기 전에 서버 쪽 독립 시각을 먼저 잰다. **판정 기준이 바뀐 항목은 SC-11 (3) · SC-81 둘이고, 새 항목은 SC-88 하나다.** **(6) architect `contracts/` 변경 반영(같은 날)**: `SESSION_CLOSED.close_reason` 에 `SUPERSEDED`, 유효 fixture **26 → 27**(SC-36·47·49 와 머리 줄), SC-81 에 **세션 간선**(`SUPERSEDED` ← 새 `SESSION_OPENED`, 나머지 원인 null), SC-88 에 넘겨받기 단언 ①~⑤ 대응과 (f). qa 도구: `check_sessions.py` 가 계약 enum 밖 `close_reason` 을 FAIL 로 드러내고, 봇이 close code 를 ADR-0005 표로 읽어 모르는 code 는 `UNKNOWN` 으로 찍는다 | architect `01_architect_decisions.md` `## R3 추가 판정` 사안 1~4 + `### 보충`, `00_request.md` 사용자 결정 5·6, qa R3 §5.5·§5.6·§6.4·§6.5·§6.7·§9 |
| 2026-09-21 (6차) | **§7a 신설 — 관측 자신에 대한 규율.** *(이 행의 "AC-2(h-log)" 는 지금의 스펙 AC-2(i) 다 — 7차 개정)* "관찰이 겨냥한 조건이 실제로 발생했는지를 함께 단언하지 않으면 초록불은 아무것도 뜻하지 않는다. 카운터 항등식은 입력이 전부 0일 때 반드시 실패해야 한다." 근거 사례 4건(`world_full` 미실행 / `SET_SHIP_CONTROL` 154개 초록인데 도달 0 / 봇 짝 게이트 `0 == 0` / 벽시계 flaky 가 폴링 비용을 잼)을 표로 붙였다. **판정 기준의 변경은 없다** — SC 항목 문구를 바꾸지 않았고, 기존 §0.3 검사 건수 원칙·§7 M-17 을 게이트 자신에게 확장한 것이다. 이 규율에 따라 봇 게이트를 3단언(`results == accepted + rejected` / `ping_replies == 수락된 PING_SERVER` / **`accepted > 0`**)으로 고쳤다(스펙 §5.1a). **AC-2(h-log)** 는 스펙에 이미 들어갔고(architect R3), 이 계약의 기존 SC 번호 체계에 새 번호를 만들지 않고 **AC-2 계열 관찰**로 리포트에 적는다 | architect R3 판정 2 + "세 건을 관통하는 것" 절, 리더 지시 A-2 |
| 2026-09-21 (5차) | **architect 의 SC-51/52 수용 판정과 그에 딸린 문구 정정 4건.** **SC-51** 을 "재조정 직전 위치 오차" → **"열린 고리 적분 오차"** 로 바꾸고 근거를 적었다(이 fixture 로 `Reconcile()` 을 돌리면 첫 지점은 장부 불일치로 미터급, 나머지 ~293지점은 `HasError=false` 로 자명하게 0 — **유효 측정이 거의 없다**. client 경로는 편의 대체가 아니라 **의도한 양을 재는 유일한 비-자명 방법**이고, 누적 표류를 재므로 **더 엄격하다**). **SC-52** 에 "SC-51과 같은 경로" 한 줄. **SC-55** 의 판정 근거를 **①wire→sim 되감기(합성) / ②물리(S6 재생 + 신설 AC-12(f))** 로 **분리**(열린 고리라 각속도를 대조하지 않으면 `ω_roll` 누락이 빠져나간다 — architect 가 client 대조 루프에서 찾은 구멍). **SC-56** 에 관찰 ①②③ 추가(**①이 0이면 FAIL**) — 지금 문구로는 `HasError=false` 여도 수치가 나와 **자명하게 통과**하기 때문이다. **fixture 와 golden 은 바꾸지 않는다**: 조밀하게 하면 SC-51/52 가 오히려 약해지고(누적 표류 → 2 tick 창), golden 재생성이 SC-34 PASS·server 의 bless 게이트 검증·스테이징된 3파일·client 실측치를 전부 무효화한다. `contracts/` 변경 없음 | architect 판정 2026-09-21, 리더 전달. qa 라운드 2 §1.6·§1.7 |
| 2026-09-20 (4차) | **§2 의 `WORLD_SNAPSHOT` 행 비고를 `ShipState` 17필드 → **18필드** 로 정정.** 같은 문서의 §0.5·SC-43·SC-48 과 스펙 §5.4 본문 표는 이미 18 을 전제하고 있었고 **이 칸 하나만 2차 개정에서 빠졌다** (B-1 이 `angular_velocity_roll_mdeg_s` 를 추가하면서 17 → 18 이 됐다). qa 라운드 1 의 SC-83 실측이 `ShipState` **18행**(`SC-83-interface-matrix.json`)으로 확인했고, client 의 생성물도 `[JsonProperty]` 18개다. **판정 기준은 바뀌지 않는다** — SC-43·SC-48 은 이미 18 을 요구했으므로 이 정정은 문서 일관성 회복이다 | architect 결정(정본을 구현·계약 §0.5 쪽으로 확정), qa 라운드 1 §6.2 통지 |
| 2026-09-20 (3차) | **Q6 계약 재정의 + 구현 완료 반영.** architect 가 burst 를 **구조로** 해결했다(`MAX_COMMANDS_PER_SESSION_PER_TICK = 8`, ADR-0011 §5.2) → **SC-26 을 "큰 burst 가 `SLOW_CONSUMER` 로 끊기지 않는다"로 교체**하고 손실 항등식을 **`보낸 수 = COMMAND_RESULT + commands_dropped_over_tick_cap_total`** 로 갱신(출처가 둘이라 p0-02 의 1:1 보다 강하다 — I-25). *architect 지시의 "SC-20" 은 p0-02 번호이고 이 계약에서는 **SC-26** 이다.* `TOO_MANY_IN_FLIGHT` 는 **구조적 미도달**이 됐다(tick 상한이 먼저 걸린다. server 가 지연 tick 주입 단위 테스트로 대체) — D절 머리말에 명시. **SC-55 의 "미검증" 고정 해제**(server S8 이 롤 구간 tick 460~500 추가. 단 **같은 스냅샷에서 두 각속도가 모두 0이 아님**을 봐야 한다). §7 에 **"신규 경로마다 실제로 탔는가" 표**를 의무화하고 M-17 신설 — `world_full` 게이트가 만들어지고도 한 번도 실행되지 않았던 사례가 근거다(**통과한 테스트 수는 경로 실행의 증거가 아니다**). §3.2 를 실제 구현에 맞춤(`ship_events.py` 5개 하위 명령으로 통합) | architect Q6 결정, server·client 구현 완료 보고 |
| 2026-09-20 (2차) | **server·client ack 반영 — 확정.** 반드시 실패할 숫자 3건 정정(SC-43 17→**18** 속성, SC-48·§0.5 **18/10/6**, "데이터 7종"→**3종 6건**). SC-28에 **첫 스냅샷 제외**(전역 tick 기준 발사), SC-31을 **세션 내 단조**로 한정, SC-13을 **(a) 잔류 / (b) 활성**으로 분리, SC-29에 정렬 기준·게이지 출처 명시, SC-71에 **페이로드 vs TCP 바이트** 주의, SC-53에 **시계를 바꿔도 같은 결과** 케이스 추가, SC-55를 "두 필드를 각각 되돌리지 않으면 실패"로, SC-50을 **바이트 해시**로, SC-56에 재측정 상한(연속 3회), SC-58에 **Deep Profile 끔**·마커 이름·1단/2단, SC-59에 **화면 입력 상태 필수**·클립 구성, SC-47에 20의 근거, SC-06에 `STARFALL_DATA_DIR`·로그 형식·종료 코드 1, SC-11에 **휴면 입력 표와 잔류 중 오토레벨(5단계)**, SC-33을 **7키 + 라벨 확장**으로, SC-25를 **7.0 m 고정 + 비율 0.2 기록**으로. **§0.11 신설**(관측자 B를 봇으로 대체 금지 — 부호가 뒤집힌다), §3.1 CSV 표기 확정, E8 재정의 | `02_server_ack.md`(쟁점 ①②③④⑤⑩ + 모호 4건), `02_client_ack.md`(쟁점 ⑥⑦⑧⑨ + B-1 이후 재측정 2건) |
| 2026-09-20 | 최초 작성 (SC-01 ~ SC-86, M-1 ~ M-16) | 스펙 §7 AC-1~21 + designer 설계 §8·§9를 실행 항목으로 변환. 짝짓기 키를 `ship_id`로 분리(§0.6), 기대값 출처를 `contracts/fixtures/`로 제한(§0.7), 성능 판정을 tick 초과 비율 하나로 축소(§0.4), 부호 미증명을 §0.10에 명시, S-3을 정지/이동 두 항목으로 분리(SC-64·65), 도구 자체 검증을 SC-85·86으로 항목화 |

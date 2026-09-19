# p0-02-networking-spike 스프린트 계약

- 작성: qa, 2026-09-18 (구현 착수 전 / Phase 3)
- 스펙: `docs/specs/p0-02-networking-spike.md` (상태 **agreed**, AC-1~21 재배치판)
- ADR: `docs/adr/0005`(전송·프레이밍) · `0006`(tick·게임 시간·큐) · `0007`(영속화·기록 범위) · `0008`(개발용 인증) — 전부 accepted
- 태스크: `_workspace/p0-02-networking-spike/01_architect_tasks.md` (T1~T14) · 반영 내역: `01_architect_decisions.md`
- 계약 데이터 (qa 실측, 2026-09-18): **스키마 11 / 유효 fixture 12 / 반례 fixture 16** — `find`로 직접 셈(§0.8)
- 합의: server ☐ · client ☐ · qa ☑ · architect(참조) ☐ — §8 확인란에 표시한다
- 항목 수: **74** (server 37 / client 14 / qa 23). 이 중 판정 대상 74, **성능은 판정하지 않는 기록 항목 M-1~M-13으로 분리**(§0.4, §6)

**이 문서의 구속력.** Phase 5 평가(`04_qa_report_r{N}.md`)는 이 표의 항목으로만 한다. 스펙 §7의 AC-1~21을 실행 가능한 관찰로 옮긴 것이고, **스펙에 없는 새 요구는 넣지 않았다.** 평가 중 발견한 스펙 밖 문제는 리포트의 "계약 외 발견"에 적고 판정에 쓰지 않는다. 항목을 바꾸려면 §9 변경 이력에 기록하고 server·client의 재확인을 받는다.

**AC 번호 주의.** `01_server_spec_review.md`·`01_client_spec_review.md`는 **옛 AC 번호**를 쓴다(옛 AC-15 = 새 AC-16, 옛 AC-17 = 새 AC-15). 이 계약의 "근거 AC" 열은 전부 **개정판 번호(AC-1~21)**다.

---

## 0. 공통 실행 전제

### 0.1 환경 준비

```bash
# Bash에서 새 도구를 쓰기 전에 반드시 한 번
export PATH="$HOME/.cargo/bin:$HOME/.dotnet/tools:/c/Users/CHOISOOYEON/AppData/Local/Unity/bin:$PATH"
```

- **cargo 명령은 `C:\WorkSpace\SpaceHistoric\server`에서.** 그 외(codegen, unity, docker, python, curl)는 레포 루트 `C:\WorkSpace\SpaceHistoric`에서.
- **HTTP 점검은 `curl.exe`로 적는다.** PowerShell의 `curl`은 `Invoke-WebRequest` 별칭이고, 503을 기대하는 점검에는 `-SkipHttpErrorCheck`가 필요하다(없으면 예외로 죽어 증거가 남지 않는다). `curl.exe` 경로 확인: `/mingw64/bin/curl.exe` (qa 실측 2026-09-18).
- **SQL은 다음 형태로 고정한다** (qa 실측 2026-09-18 — PostgreSQL 18.6 응답 확인):
  ```bash
  cd /c/WorkSpace/SpaceHistoric
  PSQL() { docker compose exec -T postgres psql -U starfall -d starfall -At -c "$1"; }
  PSQL "select version();"
  ```
- **대기가 필요한 곳에서 Bash 전경 `sleep`을 쓰지 않는다**(이 세션의 도구가 막는다). 대신:
  - DB 준비 대기 → `docker compose exec -T postgres pg_isready -U starfall -d starfall -t 60` (컨테이너 안에서 대기)
  - 고정 시간 대기 → PowerShell `Start-Sleep -Seconds N`
  - 조건 대기 → `/debug/stats` 또는 `count(*)` 폴링 루프(§3.2의 스크립트)
- 확인된 도구(2026-09-18): Docker 27.3.1 / Compose v2.29.7, PostgreSQL 18.6, `curl.exe` 8.x, Python 3.12.0. cargo·dotnet·unity는 p0-01에서 확인(1.98.1 / 10.0.401 / 1.0.0-beta.8).
- **포트 8080은 현재 비어 있다**(qa 실측: `netstat -ano | grep ":8080"` 출력 없음). 인프라는 postgres/redis 둘 다 `healthy`(qa 실측).

### 0.2 Docker와 마이그레이션 절차 (위반 시 해당 항목 무효)

이 PC에는 **다른 프로젝트 컨테이너 5개와 볼륨 90여 개**가 있다.

- `docker system prune` · `docker volume prune` · `docker builder prune` · **프로젝트 밖 `docker compose down -v` 금지**
- **레포 루트의 `docker compose down -v`는 허용된다** — `docker-compose.yml`의 `name: starfall` 덕분에 starfall 프로젝트에만 작용한다(스펙 §7 서두, ADR-0007 §2). AC-2가 이것을 요구한다.
- **마이그레이션 `.sql`을 한 글자라도 고친 뒤에는 `docker compose down -v && docker compose up -d`가 필수다.** 안 하면 `_sqlx_migrations` 체크섬 불일치로 **기동 자체가 실패**한다("previously applied but has been modified"). 이 실패는 FAIL이 아니라 **절차 오류**이며, 절차를 지키고 다시 측정한다.
- **`down -v`는 그 시점까지의 모든 DB 증거를 지운다.** 따라서 `down -v`는 한 측정 세션의 **첫 DB 단계**여야 하고(§4 게이트 G-a), 그 이전에 수집한 DB 증거(세션 쌍·탐침 행·correlation 집합)는 전부 무효다. p0-01이 남긴 `_boot_marker`·`_boot_marker_qa_r1` 테이블도 함께 사라지는데, p0-01은 종료된 슬라이스이므로 무해하다.

### 0.3 증거 기준 (PASS의 조건)

| 판정 | 조건 |
|------|------|
| **PASS** | 명령과 출력 요약(종료 코드 포함), 또는 테스트 이름, 또는 파일:라인이 리포트에 있다. 실행하지 않은 정적 읽기만으로는 PASS가 아니다 |
| **FAIL** | 기대 관찰이 나오지 않음. **간헐적으로 실패하는 항목도 FAIL**(비결정성 이슈로 기록, 재시도로 덮지 않는다) |
| **미검증(환경)** | §5의 E-조건에 걸려 실행 자체가 불가능했다. PASS로 올리지 않는다 |
| **대기** | 선행 태스크 미완(E8). 그 라운드 판정에서 제외하고 다음 라운드에 판정 |
| **기록** | 판정하지 않고 사실만 남긴다 (§6의 M-1~M-13) |

**검사 건수 원칙(모든 순회·집합 항목 공통).** fixture·반례·변이·세션·명령을 순회하는 항목은 **실제로 순회한 개수가 증거에 드러나야 한다**. 개수가 없으면 "검증기가 꺼진 채 0건 통과"와 구분할 수 없으므로 PASS로 인정하지 않는다.

- Unity: 리포트의 `tests` 수를 적는다. **매칭 0건 필터는 종료 코드 0 + `tests="0"`을 만든다**(client 실측 C-10).
- cargo: 실행된 테스트 수(`N passed`)를 적는다. 필터를 쓴 경우 필터 문자열도 함께.
- 부하: 봇이 수집한 correlation 집합의 **배열 길이**가 곧 "몇 건을 검사했는지"다(§0.6).

**구현이 없어서 실행 못 한 것은 FAIL이다.** 환경 문제(§5)와 섞지 않는다.

### 0.4 정확성 게이트와 성능 기록의 분리 (사용자 결정 2026-09-18)

| 구분 | 대상 | 판정 |
|------|------|------|
| **정확성 하드 게이트** | SC-01 ~ SC-74 전부 | PASS / FAIL / 미검증(환경) |
| **성능 잠정 게이트(비차단)** | M-1 tick 초과 비율 ≤ 0.5 % · M-2 단일 tick 본문 최대 ≤ 250 ms · M-3 왕복 p99 ≤ 150 ms | **미달해도 FAIL이 아니다.** 측정치를 **p1 회귀 기준선으로 고정 기록**하는 것이 이 항목의 산출물이다 |
| **기록만** | M-4 ~ M-13 | 값이 안 나와도 FAIL 아님. 다음 측정과 비교할 수 있게 숫자를 남긴다 |

**성능표에 있지만 하드 게이트인 것 둘** — 혼동을 막기 위해 여기 못박는다.

- **AC-17(b) "마지막 봇 종료 → 모든 행 가시까지 ≤ 5초"는 하드 게이트다**(SC-62). 스펙 §7 성능표가 직접 "하드 게이트"로 표기했다. 이것은 처리 속도가 아니라 "종료 시 몰아 쓰기가 아님"의 증명이다.
- **AC-18(c) "29봇 p99가 A 단계 대비 2배 이하"는 하드 게이트다**(SC-65). 절대 지연이 아니라 **격리 여부**를 재는 상대값이고, 사용자 결정이 비차단으로 지정한 3줄(tick 초과 비율 / 단일 tick 상한 / 왕복 p99 절대값)에 들어 있지 않다.

### 0.5 반례 16건의 층별 거부 책임 (SC-34 / SC-35 / SC-44 / SC-73의 채점 기준)

**스펙 §5.4 + p0-01 §5 표가 정본이다.** 구현 결과가 이 표와 다르면 표를 고치지 말고 architect에게 알린다(FAIL이 아니라 **계약 설계 변경 통지**다 — p0-01 SC-25와 같은 처리).

| # | fixture | ① 스키마 (SC-34) | ② Rust serde = 운영 경로 (SC-35) | ③ C# `Strict` (SC-44) |
|---|---------|:---:|---|---|
| 1 | `PING_SERVER/invalid/actor-field-injected.json` | 거부 | 거부 (`deny_unknown_fields`) | 거부 |
| 2 | `PING_SERVER/invalid/command-id-not-v7.json` | 거부 | 거부 (`UuidV7` newtype) | **감지 불가**(기록) |
| 3 | `PING_SERVER/invalid/probe-seq-negative.json` | 거부 | 거부 (`u32`) | 거부 |
| 4 | `PING_SERVER/invalid/probe-seq-above-u32.json` | 거부 | 거부 (`u32`) | 거부 |
| 5 | `PING_REPLY/invalid/missing-tick.json` | 거부 | 거부 (필수 필드) | 거부 |
| 6 | `PING_REPLY/invalid/payload-unknown-field.json` | 거부 | 거부 (`deny_unknown_fields`) | 거부 |
| 7 | `PING_REPLY/invalid/tick-above-safe-integer.json` | 거부 | 거부 (`Tick` 범위) | **감지 불가**(기록) |
| 8 | `COMMAND_RESULT/invalid/unknown-reason-code.json` | 거부 | 거부 (닫힌 열거형) | **감지 불가**(기록) |
| 9 | `COMMAND_RESULT/invalid/payload-unknown-field.json` | 거부 | 거부 (`deny_unknown_fields`) | 거부 |
| 10 | `SESSION_READY/invalid/tick-hz-zero.json` | 거부 (`minimum: 1`) | 거부 (범위 newtype) | **감지 불가**(기록) |
| 11 | `SESSION_READY/invalid/missing-session-id.json` | 거부 | 거부 (필수 필드) | 거부 |
| 12 | `SESSION_OPENED/invalid/actor-id-null.json` | 거부 (좁힘) | 거부 (비-`Option`) | **거부 — T7 생성기 수정 후**(현재는 통과, U-2) |
| 13 | `SESSION_OPENED/invalid/missing-world-id.json` | 거부 | 거부 | 거부 |
| 14 | `SESSION_CLOSED/invalid/actor-id-null.json` | 거부 (좁힘) | 거부 (비-`Option`) | **거부 — T7 생성기 수정 후**(아직 C#으로 측정된 적 없다) |
| 15 | `SESSION_CLOSED/invalid/unknown-close-reason.json` | 거부 | 거부 | **감지 불가**(기록) |
| 16 | `SESSION_CLOSED/invalid/correlation-id-null.json` | 거부 (envelope 필수·비-null) | 거부 (비-`Option`) | 거부 |

- **C# 책임 = 거부 기대 9건**(#1,3,4,5,6,9,11,13,16) **+ 좁힘 수정 후 거부 2건**(#12,14) = **11건**. **"감지 불가" 5건**(#2,7,8,10,15)은 **판정하지 않고 기록**한다 — 통과하는 것이 §5.4가 기록한 설계 결과다.
- `Runtime` 프로필은 위에 더해 `payload-unknown-field` 계열(#6,#9)도 통과시킨다(의도된 비대칭). **다만 필수 필드 누락(#5,#11)과 널 불가 필드의 널(#16)은 `Runtime`에서도 예외다**(ADR-0005 §5). → SC-46.
- 커버리지 스크립트(SC-70)는 `invalid/`를 세지 않는다 — **의도된 동작**. 그래서 반례 거부는 커버리지로 증명되지 않고 SC-34/35/44가 따로 있어야 한다.

### 0.6 세션 집합의 정의 (부하 항목 전체의 조회 기준)

전수 `select count(*)`로 판정하지 **않는다**. DB는 실행마다 누적되고, AC-4의 탐침 행과 AC-13/14의 단독 왕복이 섞인다(스펙 §7 "세션 집합의 정의", ADR-0006 §7 / I-25).

1. 봇과 Unity 클라이언트가 **`SESSION_READY`에서 받은 `correlation_id`와 `session_id`를 수집**한다. 봇은 `tools/bots`가 파일로 쓰고(§3.1), Unity는 로그 한 줄에서 QA가 뽑는다.
2. QA는 그 집합을 파일(`correlations.txt`, 한 줄에 하나)로 모아 배열로 조회한다.
   ```bash
   CORR=$(paste -sd, /path/correlations.txt)
   PSQL "select count(*) from domain_events
         where event_type='SESSION_OPENED' and correlation_id = any('{$CORR}'::uuid[]);"
   ```
3. **배열 길이가 곧 '몇 건을 검사했는지'다.** 다른 실행이 섞여도 무해하다.

### 0.7 측정 환경 기록 의무 (스펙 §7 말미)

부하 리포트에 **반드시** 적는다. 없으면 다음 측정과 비교할 수 없고, M-1~M-5가 해석 불가능해진다.

- **Unity Editor 실행 여부와 상태**(미실행 / Editor만 / PlayMode 접속 중). Windows 전역 타이머 해상도에 영향을 줄 수 있다(ADR-0006 §2.1, server 실측 §6-2).
- 동시에 도는 다른 프로젝트 컨테이너 수(현재 5개).
- 봇 하네스 시드, 봇 수, 단계별 시작·종료 시각(호스트 시계).
- 서버 빌드 프로필(dev/release)과 실행 방식(`cargo run` / 빌드된 exe 직접 실행).

### 0.8 구현 전 기준선 — **판정에 쓰지 않는다**

착수 시점에 빨간불인 것이 **정상**이다(계약이 코드보다 앞서 있다). 아래는 qa가 직접 실행해 확인한 값이고, 리포트에는 "기준선"으로만 인용한다.

| 항목 | 기준선 | 확인 방법 |
|------|--------|----------|
| 계약 커버리지 | **errors 8 / warnings 4**, `RESULT: FAIL`, exit 1 | qa 실행 2026-09-18. errors 8 = 신규 4타입의 코드 참조 없음, warnings 4 = `tools/bots` 루트 없음 |
| 계약 파일 수 | 스키마 **11** / 유효 **12** / 반례 **16** | qa `find` 실행 2026-09-18 (server·client 실측과 일치) |
| `cargo test --workspace` | 빨간불 (스키마 개수 단언 7≠11, 레지스트리 매핑 2≠6) | server 검토 B-2 |
| Unity EditMode | **26건 중 12건 실패**, 종료 코드 8 | client 실측 C-13 |
| `tools/bots/`, `tests/e2e/` | **존재하지 않음** | qa `ls` 실행 2026-09-18 |

**이 빨간불은 p0-01의 테스트 설계가 의도대로 동작한다는 증거이지 결함이 아니다.**

---

## 1. 검증 항목

### A. 서버 빌드·품질 게이트 (AC-1)

공통 실행 위치: `cd /c/WorkSpace/SpaceHistoric/server`

| ID | 검증 항목 (관찰 가능한 결과) | 검증 방법 | 담당 | 근거 AC | 불가 |
|----|---------------------------|----------|------|--------|------|
| SC-01 | `cargo fmt --all --check` 종료 코드 0, 출력 없음 | `cargo fmt --all --check; echo "exit=$?"` | server | AC-1 | E1 |
| SC-02 | `cargo clippy --workspace --all-targets -- -D warnings` 종료 코드 0, warning 0건. 보조: 신규 2크레이트(`sim`, `persistence`)의 `Cargo.toml`에 `[lints] workspace = true`가 있다 | `cargo clippy --workspace --all-targets -- -D warnings; echo "exit=$?"` + `grep -A1 '^\[lints\]' crates/sim/Cargo.toml crates/persistence/Cargo.toml` | server | AC-1 | E1 |
| SC-03 | `cargo test --workspace --locked` 종료 코드 0, 실패 0건. **실행된 테스트 수를 리포트에 적는다** | `cargo test --workspace --locked; echo "exit=$?"` → 각 `test result:` 줄의 passed 합계 | server | AC-1 | E1 |
| SC-04 | `crates/sim/Cargo.toml`에 `axum`·`sqlx`·`redis`·`rand`·`chrono`가 **하나도 없다**(IO-free·결정성 위생의 기계적 증거) | `grep -nE "axum\|sqlx\|redis\|rand\|chrono" crates/sim/Cargo.toml; echo "rc=$? (1이어야 한다)"` — 보조 기록: `cargo tree -p starfall-sim -e normal` 출력 | server | AC-1, §8 | E1 |

### B. 마이그레이션 · 월드 행 · tick 재개 · append-only (AC-2, AC-3, AC-4)

| ID | 검증 항목 | 검증 방법 | 담당 | 근거 AC | 불가 |
|----|----------|----------|------|--------|------|
| SC-05 | `down -v` 후 기동하면 `domain_events`·`worlds`에 **ADR-0007 §2의 열·제약이 전부** 있다 (열 이름·타입·NOT NULL·CHECK·`UNIQUE (world_id,tick,sequence)`·FK·인덱스 2개) | 명령 B-1의 `\d+` 출력 2개를 ADR-0007 §2 DDL과 행 단위로 대조. 리포트에 대조표(열 수 / 불일치 0) | server | AC-2 | E2, E5 |
| SC-06 | `select count(*) from worlds` = **1**이고 그 행의 `world_id`=`01a0b1c2-3d4e-7f01-8a2b-9c0d1e2f3a4b`, `tick_hz`=20, `calendar_epoch`=`3800-01-01T00:00:00Z`, `calendar_scale`=60, `sim_version`=1, `last_tick` **IS NULL** | 명령 B-2 (문자열/정수 그대로 비교) | server | AC-2 | E2, E5 |
| SC-07 | **두 번째 기동**에서 마이그레이션이 다시 적용되지 않고 기동에 성공한다 | 기동 전후 `select version, checksum from _sqlx_migrations order by version;` 출력이 **동일**하고 행 수 증가 0. 서버 로그에 재적용 기록 없음 | server | AC-2 | E2, E5 |
| SC-08 | `STARFALL_TICK_HZ=10`으로 기동하면 **기동을 거부**한다(비0 종료). 로그에 `worlds.tick_hz`(20) 불일치 사유가 있고, `worlds` 행은 변하지 않는다 | 명령 B-3 | server | AC-2 | E2, E5 |
| SC-09 | 세션 1개를 열고 닫은 뒤 **정상 종료**(stdin `shutdown`)한 시점에 `select last_tick from worlds` **>=** `select max(tick) from domain_events` | 명령 B-4 (두 값을 리포트에 같이 적는다). **정상 종료 경로에서만 측정한다** — 하드 킬 직후에는 `last_tick < max(tick)`일 수 있고 그것은 FAIL이 아니라 **M-13의 손실 창**이다(server 주의, outbox 부재) | server | AC-3 | E2, E5 |
| SC-10 | **재기동 후** `/debug/stats`의 `start_tick`이 재기동 직전 `max(tick)`보다 **크고**, 첫 실행·두 번째 실행의 세션 쌍이 **각각 온전히 존재**한다(두 correlation 집합으로 확인, 유실 0건) | 명령 B-5. 증거에 `max(tick)`, `start_tick`, 집합 크기 2개와 각 집합의 OPENED/CLOSED 행 수 | server | AC-3 | E2, E5 |
| SC-11 | `update domain_events set event_type='X'`와 `delete from domain_events`가 **둘 다 예외**로 실패하고 메시지에 `append-only`가 있으며, 실행 후 행 수가 변하지 않는다 | 명령 B-6 (before/after `count(*)` 함께) | server | AC-4, 원칙 5 | E2 |
| SC-12 | 같은 `event_id`를 다시 삽입해도 행 수가 늘지 않는다(`ON CONFLICT (event_id) DO NOTHING`) | 명령 B-6 | server | AC-4 | E2 |
| SC-13 | 다른 `event_id`로 같은 `(world_id, tick, sequence)`를 삽입하면 **UNIQUE 위반으로 실패**한다 | 명령 B-6. **탐침 행 규칙 준수 필수**(아래) | server | AC-4, I-18 | E2 |

**탐침 행 규칙 (SC-11~13, 어기면 SC-59를 QA가 스스로 깨뜨린다).**

1. 탐침은 **서버가 떠 있지 않을 때** 넣는다. 서버가 도는 중에 넣으면 그 tick을 서버가 같이 쓰려다 `UNIQUE` 위반을 일으켜 **가짜 버그 신호**가 된다.
2. 탐침 `tick`은 **이 실행이 쓰지 않는 전용 값**: `probe_tick = (select coalesce(max(tick),-1)+1 from domain_events)`. `sequence`는 **0부터 연속**으로 넣는다(1행이면 `sequence=0`).
3. 탐침을 넣은 뒤 서버를 다시 기동하면 `start_tick = probe_tick + 1`이 된다(ADR-0006 §2.3). **정상이며 tick은 뒤로 가지 않는다.** 이 사실을 리포트에 적는다 — 적지 않으면 다음 사람이 `start_tick` 점프를 버그로 본다.
4. 그래서 탐침 블록은 **모든 부하·왕복 측정이 끝난 뒤**에 실행한다(§4 블록 7).

```bash
# B-1 ~ B-6 (레포 루트, PSQL 함수는 §0.1)
cd /c/WorkSpace/SpaceHistoric
# B-1
docker compose down -v && docker compose up -d
docker compose exec -T postgres pg_isready -U starfall -d starfall -t 60
#   (서버 기동은 §4 블록 0. 기동 후)
PSQL "\d+ worlds"; PSQL "\d+ domain_events"
PSQL "select indexname, indexdef from pg_indexes where tablename='domain_events' order by 1;"
# B-2
PSQL "select count(*) from worlds;"
PSQL "select world_id, tick_hz, calendar_epoch, calendar_scale, sim_version, last_tick is null from worlds;"
# B-3 (PowerShell: 환경 변수만 바꿔 기동, 종료 코드를 본다)
#   $env:STARFALL_TICK_HZ=10; .\target\debug\starfall-game-server.exe; echo "exit=$LASTEXITCODE"  (비0 기대)
#   Remove-Item Env:\STARFALL_TICK_HZ
# B-4
PSQL "select last_tick from worlds;"; PSQL "select max(tick) from domain_events;"
# B-5
curl.exe -s http://127.0.0.1:8080/debug/stats | python -c "import json,sys; d=json.load(sys.stdin); print('start_tick',d['start_tick'],'tick',d['tick'],'tick_total',d['tick_total'])"
# B-6 (서버 정지 상태에서)
PSQL "select count(*) from domain_events;"
PSQL "update domain_events set event_type='X';"            # 예외 + append-only 기대
PSQL "delete from domain_events;"                          # 예외 + append-only 기대
PSQL "select count(*) from domain_events;"                 # 불변 기대
#   재삽입·UNIQUE 위반 탐침은 python tests/e2e/append_only_probe.py (§3.2, 탐침 규칙 내장)
```

### C. 개발용 인증 (AC-5)

| ID | 검증 항목 | 검증 방법 | 담당 | 근거 AC | 불가 |
|----|----------|----------|------|--------|------|
| SC-14 | 유효 토큰 → **101 업그레이드**, 그 연결의 **첫 메시지가 `SESSION_READY`**이고 `payload.actor_id`가 토큰 주체와 문자열로 같다 | `tools/bots probe --case auth-ok --subject <s>` 출력(첫 프레임 원문 + actor_id 비교). 또는 client의 PlayMode 로그(SC-49와 공유) | server | AC-5(a), I-10 | E5, E6 |
| SC-15 | **3가지 모두** 401이고 소켓이 열리지 않으며, 3회 시도 전후 `SESSION_OPENED` 행 수 **증가 0**: (a) 헤더 없음 (b) 서명 불일치 (c) 주체가 UUIDv7이 아님 | 명령 C-1 (3회 각각의 HTTP 코드 + before/after `count(*)`). **검사 3건을 증거에 명시** | server | AC-5(b), I-24 | E5 |
| SC-16 | `STARFALL_DEV_AUTH_SECRET` 미설정으로 기동 → `GET /ws`가 **503**이고 본문 `reason == "auth_not_configured"`이며, **같은 프로세스에서 `/healthz` 200, `/readyz` 200** | 명령 C-2 | server | AC-5(c), ADR-0008 §3 | E5 |

```bash
# C-1  (업그레이드 요청 형태를 그대로 만들고 Authorization만 바꾼다)
WSH=(-H "Connection: Upgrade" -H "Upgrade: websocket" -H "Sec-WebSocket-Version: 13" -H "Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==")
PSQL "select count(*) from domain_events where event_type='SESSION_OPENED';"      # before
curl.exe -s -o NUL -w "no-header=%{http_code}\n"  "${WSH[@]}" http://127.0.0.1:8080/ws
curl.exe -s -o NUL -w "bad-sig=%{http_code}\n"    "${WSH[@]}" -H "Authorization: Bearer 01a0b1c2-3d4e-7f01-8a2b-9c0d1e2f3a4b.deadbeef" http://127.0.0.1:8080/ws
curl.exe -s -o NUL -w "not-v7=%{http_code}\n"     "${WSH[@]}" -H "Authorization: Bearer not-a-uuid.$(printf %064d 0)" http://127.0.0.1:8080/ws
PSQL "select count(*) from domain_events where event_type='SESSION_OPENED';"      # after == before

# C-2  (비밀을 지우고 기동한 프로세스에 대해)
curl.exe -s -w "\nHTTP %{http_code}\n" "${WSH[@]}" http://127.0.0.1:8080/ws       # 503 + reason
curl.exe -s -o NUL -w "healthz=%{http_code}\n" http://127.0.0.1:8080/healthz      # 200
curl.exe -s -o NUL -w "readyz=%{http_code}\n"  http://127.0.0.1:8080/readyz       # 200
```

### D. tick 루프 · 명령 큐 · 수명 주기 · 프레이밍 (AC-6, AC-7, AC-8)

| ID | 검증 항목 | 검증 방법 | 담당 | 근거 AC | 불가 |
|----|----------|----------|------|--------|------|
| SC-17 | 수동 step 모드 테스트에서 명령 N건 제출 직후 **이벤트 0건·세션 상태 불변**이고, 1 step 후 **정확히 기대한 결과만** 나온다. **이 테스트가 tokio 런타임과 소켓 없이 돈다**(`TickOutcome` 반환값 기반) | `cargo test -p starfall-sim --locked -- --nocapture` 해당 테스트. 보조 증거: 그 테스트에 `#[tokio::test]`가 없고 `crates/sim/Cargo.toml`에 tokio가 없다(SC-04와 같은 grep). 제출 건수 N을 출력에 찍는다 | server | AC-6(a), I-13, ADR-0006 §4 | E1 |
| SC-18 | `/debug/stats`에서 **`tick_total == tick − start_tick + 1`**이고, 2회 호출 사이에 `tick`과 `tick_total`이 **같은 양**만큼 증가한다 | 명령 D-1 (두 호출 사이 간격과 두 증가량을 적는다) | server | AC-6(b), I-17 | E5 |
| SC-19 | 한 `command_id`에 대해 받은 프레임 순서가 **`COMMAND_RESULT` → `PING_REPLY`**이고 두 메시지 envelope의 `tick`이 **같다** | `tools/bots probe --case order --count 10` — 프레임 수신 순서와 두 `tick` 값을 원문으로 출력. 검사 10건 | server | AC-6(c), I-15 | E5, E6 |
| SC-20 | 한 세션에서 in-flight 상한(64)을 넘겨 폭주시키면 `COMMAND_RESULT{REJECTED, TOO_MANY_IN_FLIGHT}`가 오고 **연결은 유지**되며, **보낸 명령 수 == 받은 `COMMAND_RESULT` 수**(손실 0) | `tools/bots probe --case inflight --burst 500` — 보낸/받은/거부 사유별 수, 종료 시 연결 상태 | server | AC-7(a), I-22 | E5, E6 |
| SC-21 | 같은 `command_id` 2회 → `COMMAND_RESULT{ACCEPTED}` 1 + `PING_REPLY` **1** + `COMMAND_RESULT{REJECTED, DUPLICATE_COMMAND_ID}` 1. **`PING_REPLY`가 2건이 아니다** | `tools/bots probe --case duplicate` — 받은 메시지 3건 원문 | server | AC-7(b), ADR-0006 §6 | E5, E6 |
| SC-22 | 수신을 멈춘 클라이언트에 송신 큐 상한을 넘겨 보내면 `close_reason = SLOW_CONSUMER`인 `SESSION_CLOSED` 행이 생긴다. **판정 정본은 DB의 `close_reason`이다** — close code 1011은 RST로 유실될 수 있어 관측되지 않아도 FAIL이 아니다(server 주의) | `bots probe --case slow-consumer`(기본 3000건, **수신을 완전히 멈춘 채** 보낸다. server 실측 2261건에서 닫혔다) + 그 세션 correlation으로 DB 조회. close code는 관측되면 기록 | server | AC-7(c), ADR-0006 §5 | E5, E6 |
| SC-23 | `SERVER_BUSY` 경로가 **단위 테스트로 덮여 통과**한다. **봇으로는 재현할 수 없다**(구조적 — server 확인). 부하 실행에서 도달하지 않았음을 `commands_rejected_total{SERVER_BUSY} == 0`으로 기록한다(30×64=1920 < 4096) | `cargo test -p starfall-sim --locked -- server_busy` (테스트 이름·건수) + 부하 리포트의 `commands_rejected_total{SERVER_BUSY} == 0` 기록 | server | AC-7(d), ADR-0006 §5 | E1 |
| SC-24 | 16 KiB 초과 **텍스트** 메시지가 프로토콜 위반으로 **계수되고**(`protocol_violations_total` 증가 관측), 10초 창 8회를 넘기면 close **1002** + `close_reason = PROTOCOL_VIOLATION`. 즉 **라이브러리 한도(64 KiB) > 앱 한도(16 KiB)라 앱이 셀 수 있다**는 것이 실행으로 보인다 | `tools/bots probe --case oversize --violations 9` — 위반 1회 후 `/debug/stats.protocol_violations_total` 증가 확인(끊기지 않음), 9회째에 close code 관측 + DB의 `close_reason` | server | AC-8(a), ADR-0005 §2 | E5, E6 |
| SC-25 | **바이너리 프레임**이 같은 예산으로 계수되고 초과 시 같은 처리(1002 + `PROTOCOL_VIOLATION`) | `tools/bots probe --case binary --violations 9` | server | AC-8(b) | E5, E6 |
| SC-26 | Pong·데이터 프레임을 30초간 보내지 않으면 close **1001** + `IDLE_TIMEOUT` | `bots probe --case idle`. **tokio-tungstenite는 폴링하면 자동 Pong을 보낸다(U-9 = 참, server 확인)** — 그래서 봇은 30초 이상 **폴링 자체를 멈춘 뒤**(기본 45초) 버퍼를 비워 Close를 읽는다. 관측한 대기 시간·close code + DB의 `close_reason` | server | AC-8(c), ADR-0005 §2 | E5, E6 |
| SC-27 | **stdin `shutdown` 한 줄**(또는 터미널 Ctrl-C)로 정상 종료하면, **열려 있던 세션 수만큼** `close_reason = SERVER_SHUTDOWN`인 `SESSION_CLOSED` 행이 추가된다. **하드 킬로 대체하지 않는다** | 명령 D-2 (열린 세션 수 = `/debug/stats.ws_connections` 사전 관측, 종료 후 해당 correlation 집합의 `SERVER_SHUTDOWN` 행 수와 비교) | server | AC-8(d), I-16 | E5 |
| SC-28 | 각 경우의 close code가 **ADR-0005 §2 표와 일치**: 클라이언트 정상 종료 1000 / `IDLE_TIMEOUT` 1001 / `PROTOCOL_VIOLATION` 1002 / `SLOW_CONSUMER` 1011 / `SERVER_SHUTDOWN` 1001 | SC-22·24·25·26·27과 SC-49에서 관측한 close code를 **5행 매트릭스**로 합쳐 리포트에 싣는다(불일치 0) | server | AC-8(e) | E5, E6 |

```powershell
# D-1
curl.exe -s http://127.0.0.1:8080/debug/stats > s1.json
Start-Sleep -Seconds 5
curl.exe -s http://127.0.0.1:8080/debug/stats > s2.json
# tick_total == tick - start_tick + 1 과 두 증가량을 비교 (python 한 줄)

# D-2  stdin 종료 (가장 안정적인 방법 — 서버 exe에 파이프 stdin을 붙여 띄운다)
$exe = "C:\WorkSpace\SpaceHistoric\server\target\debug\starfall-game-server.exe"
$psi = [Diagnostics.ProcessStartInfo]::new($exe)
$psi.RedirectStandardInput = $true; $psi.UseShellExecute = $false
$psi.WorkingDirectory = "C:\WorkSpace\SpaceHistoric\server"
$p = [Diagnostics.Process]::Start($psi)
# ... 세션을 열어 둔 상태에서 ws_connections 확인 후 ...
$p.StandardInput.WriteLine("shutdown")
$p.WaitForExit(); "exit=$($p.ExitCode)"
```

### E. 운영 표면 (AC-9)

| ID | 검증 항목 | 검증 방법 | 담당 | 근거 AC | 불가 |
|----|----------|----------|------|--------|------|
| SC-29 | 인프라가 뜬 **직후 첫 `/readyz`도 200**이다. `docker compose stop postgres` 후 503, `start` 후 **서버 재시작 없이** 200 복귀(프로세스 PID 동일) | 명령 E-1 (첫 호출 결과를 반드시 따로 적는다 — p0-01 이월 항목) | server | AC-9(a), ADR-0007 §9 | E2, E5 |
| SC-30 | `/debug/stats.ws_connections`가 **세션 레지스트리의 실제 길이에서 계산**되고(코드 파일:라인), 정지 시점에 `ws_connections == sessions_opened_total − sessions_closed_total`이며, 그 값이 같은 시점 DB의 (집합 내) `SESSION_OPENED − SESSION_CLOSED` 행 수와 **같다** | ① 파일:라인으로 계산 출처 확인(카운터 뺄셈이면 **FAIL** — 항등식이 되어 아무것도 증명하지 않는다, I-25). server 정본: `ws_connections`는 **tick 드라이버 라우팅 표의 실제 길이**다 ② 부하 중 한 시점과 부하 종료 후 한 시점, 두 번 관측 ③ 같은 시점 SQL. **`live_connections`(살아 있는 소켓 수)도 함께 적는다** — 종료 중에는 `live > ws`가 정상이다(스윕이 라우팅 표를 먼저 비운다) | server | AC-9(b), I-25 | E5 |
| SC-31 | `commands_received_total − messages_enqueued_total{COMMAND_RESULT} == 0`(서버 내부 1:1 불변식)이고, 정지 시점에 회계 항등식 **`messages_enqueued_all == messages_written_all + messages_dropped_total + send_queue_depth`**가 성립한다(정지 시점에는 뒤 두 항이 0이라 `enqueued == written`이 된다) | 부하 종료 후 `/debug/stats` 1회. **네 값을 함께 적는다** — `dropped > 0`은 회계 오류가 아니라 연결이 비정상 종료됐다는 뜻이다(server 답). 부하 중 샘플은 원자적으로 읽히지 않으므로 **기록용** | server | AC-9(c), I-15 | E5 |

```powershell
# E-1
Get-Process starfall-game-server | Select-Object Id,StartTime
curl.exe -s -w "\nfirst-readyz=%{http_code}\n" http://127.0.0.1:8080/readyz
docker compose stop postgres
curl.exe -s -w "\nstopped=%{http_code}\n" http://127.0.0.1:8080/readyz      # 503
docker compose start postgres
docker compose exec -T postgres pg_isready -U starfall -d starfall -t 60
curl.exe -s -w "\nrecovered=%{http_code}\n" http://127.0.0.1:8080/readyz    # 200, 재시작 없이
Get-Process starfall-game-server | Select-Object Id,StartTime               # PID 동일
```

### F. 계약 ↔ Rust (AC-10)

공통 명령: `cd /c/WorkSpace/SpaceHistoric/server && cargo test -p starfall-contracts --locked -- --nocapture`
(테스트 이름은 server가 정한다. **SC ID → 테스트 이름 대응표를 `03_server_impl.md`에 남긴다.**)

| ID | 검증 항목 | 검증 방법 | 담당 | 근거 AC | 불가 |
|----|----------|----------|------|--------|------|
| SC-32 | ADR-0002 §3의 테스트 **9종**이 타입 **6종 전부**를 덮고 통과한다. 테스트 출력에 **검사 건수 12 / 16 / 11**이 찍힌다 | 위 명령. 출력에서 세 숫자와 테스트 9종 이름을 인용 | server | AC-10 | E1 |
| SC-33 | 유효 fixture **12건**이 역직렬화 → 재직렬화 후 원본과 `serde_json::Value` 비교로 동일 | 위 명령 (`fixtures_roundtrip`). 증거에 12건 파일명 | server | AC-10(a) | E1 |
| SC-34 | **[층①]** 반례 **16건 전부**가 스키마 검증에서 거부된다 | 위 명령 (`invalid_rejected_by_schema`). 증거에 16건 파일명과 개수 | server | AC-10(b), I-4 | E1 |
| SC-35 | **[층②]** 반례 16건의 **Rust 역직렬화 결과가 §0.5 표의 ② 열과 일치**(현재 16건 전부 거부) | 위 명령 (`invalid_serde_matrix`). 증거는 16행 결과표. 보조: `grep -rn "serde(flatten)\|serde(tag *=" crates/contracts/src`가 peek 구조체 외 0건 | server | AC-10(c), I-6 | E1 |
| SC-36 | 각 유효 fixture에서 `required` 필드를 하나씩 제거한 변이가 **전부** 실패한다 | 위 명령 (`required_field_mutations`). **변이 개수**를 증거에 | server | AC-10(d) | E1 |
| SC-37 | `producers`·`consumers`에 `server`가 있는 타입 **6종 전부**가 이름→Rust 타입 대응표에 있다 | 위 명령 (`registry_server_types_mapped`) | server | AC-10(e) | E1 |

### G. 클라이언트 — 생성기와 EditMode (AC-11, AC-12)

EditMode 공통 명령 G-1 (레포 루트):

```bash
unity test client --mode EditMode --report-format nunit,junit \
  --output _workspace/p0-02-networking-spike/unity-tests/EditMode.nunit.xml \
  --junit-output _workspace/p0-02-networking-spike/unity-tests/EditMode.xml
echo "exit=$?"
ls -l _workspace/p0-02-networking-spike/unity-tests/EditMode.nunit.xml _workspace/p0-02-networking-spike/unity-tests/EditMode.xml
grep -o 'tests="[0-9]*"'    _workspace/p0-02-networking-spike/unity-tests/EditMode.xml | head -1
grep -o 'failures="[0-9]*"' _workspace/p0-02-networking-spike/unity-tests/EditMode.xml | head -1
grep -c "<test-case"        _workspace/p0-02-networking-spike/unity-tests/EditMode.nunit.xml
```

- **`--report-format both`는 이 CLI가 거부한다**(종료 코드 2). `nunit,junit` 쉼표 목록이 유효한 형태다(client 실측 C-11 — `--help`가 틀렸다. 고치려 들지 말 것).
- `--filter`는 **정규식**(부분 일치). glob은 `ArgumentException` → CLI 종료 코드 6, 리포트 없음(C-9).
- `unity` CLI 종료 코드: 성공 0 / 테스트 실패 8 / 런 에러 6 / 인자 오류 2.

| ID | 검증 항목 | 검증 방법 | 담당 | 근거 AC | 불가 |
|----|----------|----------|------|--------|------|
| SC-38 | 생성기를 **두 번** 실행해도 생성 파일 해시가 변하지 않고 `--check`가 종료 코드 0 | 명령 G-2 | client | AC-11(a) | E4 |
| SC-39 | **좁힘 수정 후** `SessionOpenedEvent.ActorId`·`SessionClosedEvent.ActorId`가 `System.Guid` + `Required.Always`이고, `SESSION_OPENED/invalid/actor-id-null.json`·`SESSION_CLOSED/invalid/actor-id-null.json` **2건이 C#에서 거부**된다 | G-1의 좁힘 회귀 테스트(리플렉션으로 타입·Required 확인 + 반례 2건 거부). 검사 2건 명시 | client | AC-11(b), ADR-0005 §4-3 | E3 |
| SC-40 | 생성기 수정 **전후 생성물 diff에서 변한 것이 그 두 속성뿐**임이 증명된다 | client가 `03_client_impl.md`에 diff 전문을 붙이고, QA는 diff의 변경 hunk 수와 대상 파일·속성명을 확인 | client | AC-11(c) | E4 |
| SC-41 | `enum`이 allowlist의 "제약" 분류로 이동했고 **생성물이 변하지 않는다**(동작 무변화, 주석 정정) | `tools/codegen/ContractsCodegen.cs`의 해당 파일:라인 + SC-40 diff에 그 변경으로 인한 생성물 차이가 **없음** | client | AC-11(d) | E4 |
| SC-42 | G-1이 **종료 코드 0**, 실패 **0**, **리포트 2개 존재**, **`tests` 수가 0이 아니다**(그 수를 리포트와 `03_client_impl.md`에 적는다) | 명령 G-1 | client | AC-12 | E3 |
| SC-43 | 유효 fixture **12건** 왕복(`Strict`)이 전부 통과한다 | G-1의 `Fixtures_RoundTrip_*`. 증거에 12건 파일명 | client | AC-12(a) | E3 |
| SC-44 | §0.5 표에서 **C# 책임인 11건이 거부**되고, **"감지 불가" 5건이 실제로 통과함을 테스트가 명시적으로 기록**한다 | G-1의 `Invalid_Rejected_*` + "감지 불가" 기록 테스트. 증거: 거부 11건 파일명 + 예외 메시지, 통과 5건 파일명. **표와 다르면 FAIL이 아니라 architect 통지 사유**(계약 설계가 바뀐 것) | client | AC-12(b), 스펙 §5.4 | E3 |
| SC-45 | 알 수 없는 `message_type`을 디스패치하면 **예외 없이 경고만** 남고 **이후 메시지 처리가 계속**된다 | G-1의 해당 테스트(알 수 없는 타입 → 유효 타입 순서로 2건 처리, 두 번째가 정상 처리됨을 Assert) | client | AC-12(c), ADR-0005 §5 | E3 |
| SC-46 | `Runtime` 프로필이 (a) payload의 모르는 필드를 **무시하고 경고를 남기고**, (b) 같은 입력이 `Strict`에서는 **예외**이며, (c) **`Runtime`에서도** `missing-session-id.json`(필수 필드 누락)과 `correlation-id-null.json`(널 불가 널)은 **예외**다 | G-1의 해당 테스트 4케이스. 경고 건수와 예외 타입/메시지를 Assert. 보조: `"Could not find member"` 접두사 의존을 고정하는 테스트가 존재 | client | AC-12(d), ADR-0005 §5 | E3 |
| SC-47 | 백오프 지연이 **순수 함수**이고 `base=500ms, factor=2, cap=10s, full jitter, 지수 클램프 5`를 만족하며, **`n = 0,1,5,62,63,64,100` 전부에서 `0 < delay <= 10s`**다. 고정 시드로 결정적이다 | G-1의 해당 테스트. 7개 n 값과 각 delay를 출력에 남긴다(클램프가 없으면 n=62,63에서 0 ms — client 실측 C-16) | client | AC-12(e), ADR-0005 §5 | E3 |
| SC-48 | fixture 로더가 유효 fixture를 **12건 미만** 발견하면 테스트가 **실패**한다(Skip 아님). 가드 증명을 위해 로더가 센 수(12)가 증거에 있다 | G-1의 `FixtureLoader_*` | client | AC-12(f), I-4 | E3 |

```bash
# G-2 (레포 루트) — QA 재현은 실물을 건드리지 않고 스크래치 사본에서
GEN=client/Assets/_Project/Scripts/Contracts/Generated
SCRATCH=/c/Users/CHOISO~1/AppData/Local/Temp/claude/C--WorkSpace-SpaceHistoric/p002; mkdir -p "$SCRATCH"
dotnet run tools/codegen/ContractsCodegen.cs -- --contracts contracts --out "$GEN"
find "$GEN" -name '*.cs' | sort | xargs sha256sum > "$SCRATCH/h1.txt"
dotnet run tools/codegen/ContractsCodegen.cs -- --contracts contracts --out "$GEN"
find "$GEN" -name '*.cs' | sort | xargs sha256sum > "$SCRATCH/h2.txt"
diff "$SCRATCH/h1.txt" "$SCRATCH/h2.txt" && echo "deterministic OK"
dotnet run tools/codegen/ContractsCodegen.cs -- --contracts contracts --out "$GEN" --check; echo "check exit=$?"
```

### H. 클라이언트 ↔ 실서버 (AC-13, AC-14)

| ID | 검증 항목 | 검증 방법 | 담당 | 근거 AC | 불가 |
|----|----------|----------|------|--------|------|
| SC-49 | Unity가 `Starfall.Net`으로 `/ws`에 붙어 `PING_SERVER` **3건**을 보내면 (a) 첫 메시지가 `SESSION_READY`이고 `tick_hz == 20`, (b) `COMMAND_RESULT` 3건 + `PING_REPLY` 3건을 받고 `command_id`·`probe_seq`가 전부 맞으며 순서가 I-15를 만족한다 | 사람이 띄운 Editor의 PlayMode에서 `Starfall/Net/Send PING_SERVER x3`(`03_client_impl.md` §1.4). 증거: `client/Logs/starfall-net.log`의 `starfall.net: SESSION_READY …` 한 줄(`tick_hz=20` 포함) + 수신 프레임 6건 원문. `python tests/e2e/unity_corr.py --evidence <p>.json`이 그 줄을 파싱한다. 검사 3 왕복 | client | AC-13(a)(b) | E3, E5 |
| SC-50 | DB에 그 세션의 `SESSION_OPENED`/`SESSION_CLOSED` **1쌍**이 **같은 `correlation_id`**로 있고 `close_reason = CLIENT_CLOSED`다 | `python tests/e2e/unity_corr.py --out <f>` → 그 `correlation_id`로 §0.6 조회. **전제: 측정 중 도메인 리로드 없음.** 리로드 여부는 로그의 `starfall.net: closing … reason=` 값으로 구분한다 — `CLIENT_CLOSED`면 정상, `EDITOR_RELOAD`·`PLAYMODE_EXIT`이면 **FAIL이 아니라 재측정**(client가 사유를 구분해 찍는다) | client | AC-13(c) | E3, E5 |
| SC-51 | 서버 재시작(또는 연결 강제 종료) 후 클라이언트가 **백오프 뒤 재연결**해 새 `SESSION_READY`를 받고, DB에 **서로 다른 `session_id`와 서로 다른 `correlation_id`**를 가진 세션 쌍이 **2개** 생긴다. 로그에 `starfall.net: dropping N in-flight command(s) on disconnect (no resend, I-23)` 한 줄이 있고, **시도 카운터가 `SESSION_READY` 수신 시에만 리셋**됨이 로그로 확인된다 | `unity_corr.py`가 뽑는 세 문구로 판정한다: `reconnect attempt=<n> delay_ms=<ms>`(백오프), `dropping N in-flight command(s) … (no resend, I-23)`(**N=0이어도 남는다**), `SESSION_READY … attempt=<int>`(**같은 줄에서 카운터 리셋 확인**). 두 correlation으로 DB 조회(각각 OPENED/CLOSED 1쌍) | client | AC-14, I-23 | E3, E5 |

### I. QA — 정상 상태 30+1과 기록 무결성 (AC-15, AC-16, AC-17)

부하 단계는 스펙 §7의 형태를 그대로 쓴다. **A 단계는 Unity 클라이언트가 먼저 붙은 뒤에 시작한다**(Editor 프로젝트 로드만 21초 — client 실측).

| 단계 | 내용 | 기대 산출 |
|------|------|----------|
| **A 정상 상태** | Unity 1대 접속 유지 → 봇 30개가 5초 안에 접속 → 60초 유지, 각 봇 500 ms마다 `PING_SERVER` | 명령 약 3,600건, 세션 31쌍 |
| **B 회전** | 봇 30개가 각각 접속 → ping 3회 → 종료를 5회 반복 | 세션 150쌍 = 도메인 이벤트 300건 |
| **C 백프레셔** | 봇 29개가 A와 같은 부하, 1개가 2,000건 폭주 | 거부 계수, 격리 |
| **D 기록 내구성** | A 재실행 도중 `stop postgres` → 백로그 → `start postgres` | 무손실 |

| ID | 검증 항목 | 검증 방법 | 담당 | 근거 AC | 불가 |
|----|----------|----------|------|--------|------|
| SC-52 | A 단계에서 **31개 연결 전부 수립**되고, **서버가 먼저 닫은 연결이 0건**이다 | 봇 `summary.json`의 수립 수(30) + Unity 1 = 31, 각 세션의 close 관측값 + 집합 내 `close_reason`이 전부 `CLIENT_CLOSED`(독립 출처 2개) | qa | AC-15(a) | E7, E6 |
| SC-53 | **보낸 명령 수 == 받은 `COMMAND_RESULT` 수**이고 `command_id` 1:1, 중복 0 (**손실 0건**) | `tools/bots` A 단계 `summary.json`(sent / results / dup / unmatched). 세 숫자를 리포트에 | qa | AC-15(b), I-15 | E7, E6 |
| SC-54 | **`ACCEPTED` 수 == `PING_REPLY` 수**이고 `probe_seq`·`command_id`가 전부 일치, 중복 0 | 같은 `summary.json` | qa | AC-15(c) | E7, E6 |
| SC-55 | **모든 `PING_REPLY`가 그 명령의 `COMMAND_RESULT` 뒤에** 도착했다(위반 0건 / 검사 N건) | 봇이 수신 순서를 기록(`commands.csv`의 `order_ok`). 위반 수와 검사 수를 함께 | qa | AC-15(d), I-15 | E7, E6 |
| SC-56 | **3자 대조**: 봇 관측 == `/debug/stats` == DB 행 수. 하나라도 다르면 그 차이가 발견이다 | 리포트에 3열 표(명령 수·COMMAND_RESULT 수·세션 OPENED/CLOSED 수). 출처: 봇 파일 / `/debug/stats` / correlation 집합 SQL | qa | AC-15(e), AC-9(c) | E7, E6 |
| SC-57 | A+B 종료 후, 수집한 correlation 집합에 대해 **`SESSION_OPENED` 행 수 == 집합 크기**이고 `SESSION_CLOSED`도 **같은 수**다 | 명령 I-1. 집합 크기(배열 길이)를 증거에 명시 | qa | AC-16(a) | E7 |
| SC-58 | `correlation_id`로 묶었을 때 **짝 없는 이벤트 0건, 중복 0건** | 명령 I-2 (0행 기대). 검사한 correlation 수를 함께 | qa | AC-16(b), I-16 | E7 |
| SC-59 | 각 `(world_id, tick)`의 `sequence`가 **0..n−1로 빈틈없다** — 스펙 AC-16(c) SQL이 **0행** 반환 | 명령 I-3 (전 테이블 대상. 탐침 행 규칙 §B 준수 전제). A+B 직후 1회(판정) + 탐침 블록 뒤 1회(보조) | qa | AC-16(c), I-18 | E7 |
| SC-60 | `occurred_at`이 전부 `GameTime` 패턴을 만족하고, **표본 10건**을 ADR-0006 §3 공식으로 `tick`에서 재계산한 값과 **문자열로 같다** | 명령 I-4 (`tick_hz=20, calendar_scale=60, epoch=3800-01-01T00:00:00Z`, `tick_hz`로 **먼저** 나눈다). 표본 10건 전부를 표로 | qa | AC-16(d), I-19 | E7 |
| SC-61 | **A 단계 진행 중**(연결 31개가 열려 있는 동안) 조회하면 집합의 `SESSION_OPENED`가 이미 **31행**이고 그 세션들의 `SESSION_CLOSED`는 **0행**이다 — 종료 시 몰아 쓰기가 아님의 증명 | `run_block.py load`가 A 시작 후 기본 T+25초에 `check_in_flight.py`를 부른다(집합 = 봇의 `correlations.live.txt` + Unity). **A가 도는 동안에만 측정 가능하다** — 끝나면 증명할 수 없다. 증거에 조회 시각·집합 크기·A 실행 여부가 남는다 | qa | AC-17(a) | E7 |
| SC-62 | 마지막 봇이 끊긴 시점부터 **5초 이내**에 기대한 모든 행이 존재한다. **호스트에서 `count(*)`를 폴링해 "기대 행 수 도달까지 몇 초"로 잰다** (`recorded_at` vs 호스트 시계 비교 금지 — 컨테이너 시계 차이) | 명령 I-6. 증거: 마지막 봇 종료 시각(호스트), 폴링 간격(≤200 ms), 도달 시각, 경과 초 | qa | AC-17(b) | E7 |

```bash
# I-1 ~ I-6 (레포 루트. CORR 는 §0.6)
# I-1
PSQL "select event_type, count(*) from domain_events
      where correlation_id = any('{$CORR}'::uuid[]) group by 1 order by 1;"
# I-2  짝 없음/중복  (0행 기대)
PSQL "select correlation_id,
             count(*) filter (where event_type='SESSION_OPENED') o,
             count(*) filter (where event_type='SESSION_CLOSED') c
      from domain_events where correlation_id = any('{$CORR}'::uuid[])
      group by 1 having count(*) filter (where event_type='SESSION_OPENED') <> 1
                   or count(*) filter (where event_type='SESSION_CLOSED') <> 1;"
# I-3  sequence 빈틈 (스펙 AC-16(c) 원문, 0행 기대)
PSQL "select world_id, tick from domain_events group by world_id, tick
      having count(*) <> max(sequence) + 1 or min(sequence) <> 0
          or count(distinct sequence) <> count(*);"
# I-4  occurred_at 표본 10건 재계산 대조
PSQL "select tick, occurred_at from domain_events
      where correlation_id = any('{$CORR}'::uuid[]) order by tick limit 10;" \
  | python tests/e2e/check_occurred_at.py     # (§3.2)
# I-5  A 단계 진행 중
PSQL "select count(*) filter (where event_type='SESSION_OPENED') opened,
             count(*) filter (where event_type='SESSION_CLOSED') closed
      from domain_events where correlation_id = any('{$CORR}'::uuid[]);"   # 31 / 0 기대
# I-6  종료 후 가시성 폴링
python tests/e2e/poll_until.py --expect <N> --corr correlations.txt --timeout 15
```

### J. QA — 백프레셔 격리 (AC-18, C 단계)

| ID | 검증 항목 | 검증 방법 | 담당 | 근거 AC | 불가 |
|----|----------|----------|------|--------|------|
| SC-63 | 폭주 봇이 `TOO_MANY_IN_FLIGHT` 거부를 받지만 **보낸 명령 수 == 받은 `COMMAND_RESULT` 수**(거부도 응답이다)이고 **연결이 유지**된다 | C 단계 `summary.json`의 폭주 봇 행(sent / results / rejected{reason} / 연결 종료 사유) | qa | AC-18(a), I-22 | E7, E6 |
| SC-64 | 나머지 **29개 봇의 손실이 0**이고 **연결이 끊기지 않는다** | 같은 `summary.json`의 29봇 집계 + 집합 내 `close_reason` 전부 `CLIENT_CLOSED` | qa | AC-18(b) | E7, E6 |
| SC-65 | 29개 봇의 왕복 **p99가 A 단계 대비 2배를 넘지 않는다** (**하드 게이트** — 절대 지연이 아니라 격리 증명이다, §0.4) | A 단계 p99와 C 단계 29봇 p99를 나란히. 두 값과 비율을 리포트에 | qa | AC-18(c) | E7, E6 |

### K. QA — 기록을 버리지 않는다 (AC-19, D 단계) · **Phase 0 종료 기준의 직접 실증**

| ID | 검증 항목 | 검증 방법 | 담당 | 근거 AC | 불가 |
|----|----------|----------|------|--------|------|
| SC-66 | PostgreSQL 중단 후 백로그가 임계를 넘으면 **새 연결이 503 `reason=recording_backlog`**를 받는다 | 명령 K-1. **`/debug/stats.persist_backlog > 600`을 폴링으로 확인한 뒤** 새 연결을 시도한다(600 tick = 20 Hz × 30초 = 스펙의 "30초"). 관측한 backlog 값과 경과 시간을 증거에 | qa | AC-19(a), ADR-0007 §4 | E2, E7 |
| SC-67 | **기존 세션은 끊기지 않으며** 중단 구간 내내 계속 ping 왕복을 한다 | 봇 `commands.csv`에서 중단 구간(시작~복구 시각)에 성공한 왕복 건수 > 0이고 그 구간에 세션 종료가 0건 | qa | AC-19(b) | E2, E7 |
| SC-68 | PostgreSQL 복구 후 `persist_backlog`가 **정상 대역(0~20)으로 돌아간다** | 명령 K-1의 복구 후 폴링(`poll_until.py backlog --target 20`, 도달 시각 기록). **정상 동작에서도 0~20을 오간다**(20 tick = 1초 주기 커밋, server §5) — 0이 아닌 것은 이상이 아니다. **스펙 AC-19(c)의 문구는 "0으로"이므로 이 완화는 architect 통지 사항이다**(FAIL 아님) | qa | AC-19(c) | E2, E7 |
| SC-69 | 중단 구간에 발생한 이벤트가 **한 건도 유실되지 않고** DB에 들어온다 | 그 실행의 correlation 집합으로 OPENED/CLOSED 대조(집합 크기 == 행 수, 짝 없음 0). **손실 0건 / 검사 N건**을 명시 | qa | AC-19(d), I-22 | E2, E7 |

```powershell
# K-1 (D 단계) — A 재실행 도중에
docker compose stop postgres
# persist_backlog > 600 까지 폴링
do { $b = (curl.exe -s http://127.0.0.1:8080/debug/stats | ConvertFrom-Json).persist_backlog; $b; Start-Sleep -Seconds 2 } while ($b -le 600)
curl.exe -s -w "`nnew-conn=%{http_code}`n" -H "Connection: Upgrade" -H "Upgrade: websocket" `
  -H "Sec-WebSocket-Version: 13" -H "Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==" `
  -H "Authorization: Bearer <valid>" http://127.0.0.1:8080/ws       # 503 + recording_backlog
docker compose start postgres
# persist_backlog == 0 까지 폴링 (도달 시각 기록)
```

### L. QA — 계약 커버리지와 경계면 교차 검증 (AC-20, AC-21)

| ID | 검증 항목 | 검증 방법 | 담당 | 근거 AC | 불가 |
|----|----------|----------|------|--------|------|
| SC-70 | 계약 커버리지 스크립트가 `--strict`에서 **종료 코드 0**(errors 0, **warnings 0**) | `cd /c/WorkSpace/SpaceHistoric && python .claude/skills/integration-qa/scripts/check_contract_coverage.py --strict; echo "exit=$?"` — **T1·T7·T8·T12 완료 후에만 실행**(게이트 G-c). 기준선 8/4는 판정에 쓰지 않는다 | qa | AC-20 | E8 |
| SC-71 | 신규 4타입 **46필드**의 스키마 · Rust 타입 · C# DTO 3자 비교에서 **필드 이름·필수 여부·널 가능 여부가 동일**하고 **불일치 0건** | §2의 행 목록대로 표를 만들어 리포트에 싣는다. **비교한 필드 수(46)와 불일치 수(0)**를 적는다 | qa | AC-21 | E8 |
| SC-72 | 정수 필드의 언어 타입이 스키마의 `minimum`~`maximum`을 **손실 없이** 담는다(이름 동일 요구 없음) | §2의 정수 4행(`tick`, `sequence`, `schema_version`, `payload.tick_hz`). 범위 밖 거부는 SC-34 결과를 인용 | qa | AC-21 | E8 |
| SC-73 | 층별 거부 범위가 **§0.5 표와 일치**하고, 3층 매트릭스(16행 × 3층)가 리포트에 기록된다 | SC-34 / SC-35 / SC-44 결과를 한 표로 합친다 | qa | AC-21, 스펙 §5.4 | E8 |
| SC-74 | fixture의 `world_id` ↔ `tick_hz` 조합이 **I-19와 모순되지 않는다**(같은 `world_id`를 쓰는 fixture는 같은 `tick_hz`) | 명령 L-1 (fixture 전수 스캔, 위반 0건. `SESSION_READY/tick-zero.json`의 `world_id`가 스파이크 월드와 다름을 확인) | qa | AC-21, I-19 | — |

```bash
# L-1
cd /c/WorkSpace/SpaceHistoric && python - <<'PY'
import json,glob,collections
m=collections.defaultdict(set)
for p in glob.glob("contracts/fixtures/**/*.json",recursive=True):
    if "/invalid/" in p.replace("\\","/"): continue
    d=json.load(open(p)); pl=d.get("payload",{})
    w=d.get("world_id") or pl.get("world_id")
    hz=pl.get("tick_hz")
    if w and hz is not None: m[w].add((hz,p))
bad=0
for w,s in m.items():
    if len({hz for hz,_ in s})>1: bad+=1; print("VIOLATION",w,sorted(s))
print("worlds checked:",len(m),"violations:",bad)
PY
```

---

## 2. 경계면 비교표의 고정 대상 (SC-71 / SC-72의 채점 기준)

리포트에 실을 표의 **행 목록을 지금 고정한다**(구현 후 임의 확대 금지). 합계 **46행** — 스펙 AC-21이 고정한 값이다.

**채점 근거는 총계가 아니라 행이다 (2026-09-18 client 답변 반영).** client가 `03_client_impl.md` §9에 **필드별 46행 표**를 실었다. SC-71/72의 판정은 그 표의 행과 이 절의 행 목록을 대조해서 한다.

- 비대칭(메시지 타입만 `payload` 자체를 한 행으로 셈)은 **설계 의도가 아니라 client의 계산 착오**였다. 대칭으로 세면 **48**이다.
- 스펙 AC-21이 고정한 값이 46이므로 **총계는 46으로 유지**하고, "46 vs 48"은 **architect 통지 사항**으로 리포트에 적는다(FAIL이 아니다 — 스펙 수정은 architect 권한이다).
- 리포트에는 **비교한 행 수와 불일치 수**를 적는다. 총계가 아니라 행이 증거다.

**COMMAND_RESULT (6+3=9)** — `message_id`(UuidV7, required) · `message_type`(const `COMMAND_RESULT`, required) · `schema_version`(const 1, required) · `tick`(0..9007199254740991, required) · `correlation_id`(UuidV7 **또는 null**, required 키 — 서버는 **항상 null**을 넣는다) · `payload`(required) · `payload.command_id`(UuidV7, required) · `payload.status`(enum `ACCEPTED|REJECTED`, required) · `payload.reason_code`(enum 6종 **또는 null**, required 키)

**SESSION_READY (6+5=11)** — `message_id` · `message_type`(const) · `schema_version`(const 1) · `tick` · `correlation_id`(**세션 correlation**, 널 가능 키) · `payload` · `payload.session_id`(UuidV7) · `payload.world_id`(UuidV7) · `payload.actor_id`(UuidV7) · `payload.tick_hz`(integer **1..1000**) · `payload.server_version`(string)

**SESSION_OPENED (11+2=13)** — `event_id` · `event_type`(const) · `schema_version`(const 1) · `world_id` · `tick` · `sequence`(0..9007199254740991) · `occurred_at`(GameTime) · `recorded_at`(RealTime) · `correlation_id`(**비-null**) · `causation_id`(UuidV7 **또는 null**) · `actor_id`(**좁힘: 비-null**) · `payload.session_id` · `payload.transport`(enum `WEBSOCKET`)

**SESSION_CLOSED (11+2=13)** — 위 envelope 11행 동일 + `payload.session_id` · `payload.close_reason`(enum 6종)

**정수 4행 (SC-72)**

| 필드 | 스키마 범위 | Rust(기대) | C#(기대) | 판정 기준 |
|------|------------|-----------|---------|----------|
| `tick` | 0 … 9007199254740991 | 범위 검증 newtype(u64 기반) | `long` | 범위를 손실 없이 담으면 PASS. 이름 동일 요구 없음 |
| `sequence` | 0 … 9007199254740991 | 범위 검증 newtype | `long` | 위와 같음 |
| `schema_version` | 1 … 2147483647 | `u32`/`i32` + const 검증 | `int` | 위와 같음 |
| `payload.tick_hz` | **1 … 1000** | 범위 newtype | `int` | C#은 0을 담는다 → §0.5 #10이 "감지 불가"인 이유. 스키마·Rust가 막는 것을 SC-34/35가 인용 |

**널 가능 3행** — `correlation_id`(메시지 envelope), `causation_id`, `reason_code`는 **키가 항상 존재하고 값이 null일 수 있다**. Rust는 `Option<T>` + `skip_serializing_if` **금지**, C#은 `Required.AllowNull`. 한쪽이라도 "키 없음"으로 처리하면 SC-71 불일치다.
**좁힘 2행** — `SESSION_OPENED.actor_id`, `SESSION_CLOSED.actor_id`는 **비-null**(Rust 비-`Option` / C# `Guid` + `Required.Always`). 이벤트 envelope의 `correlation_id`도 비-null이며 **메시지 envelope의 것과 같은 타입으로 만들지 않는다**.

---

## 3. QA 소유 도구 — **구현 완료** (2026-09-18/19, T12·T13)

`tools/bots/**`와 `tests/e2e/**`는 QA 소유다. 계약 단계의 설계를 그대로 구현했고, **실행 명령과 실측 결과를 여기 고정한다.** 서버가 아직 없으므로 도구는 "서버 없음"을 정확히 구분해 보고한다(종료 코드 3 = FAIL(구현 없음), 2 = 미검증(환경)).

### 3.0 실행 명령 (그대로 복사해 쓴다)

```bash
export PATH="$HOME/.cargo/bin:$HOME/.dotnet/tools:/c/Users/CHOISOOYEON/AppData/Local/Unity/bin:$PATH"
export STARFALL_DEV_AUTH_SECRET=dev_only_not_a_secret     # .env.example 의 공개 개발값

# 1) 봇 하네스 빌드 + 자체 검증 (서버 불필요)
cd /c/WorkSpace/SpaceHistoric/tools/bots
cargo build --offline
cargo test  --offline                 # 37건 (계측기·와이어·토큰·루프백)
cargo clippy --offline --all-targets  # 0 warning
cargo fmt --check

# 2) 신원 확인 — Unity 주체와 겹치지 않는가 (client 요청)
./target/debug/bots identities --count 30 --disjoint-from 01a0b1c2-7e57-7c11-8e57-000000000001
./target/debug/bots token --label bot-000

# 3) 환경 점검 — 어떤 E-코드가 걸리는지 먼저 본다
cd /c/WorkSpace/SpaceHistoric
python tests/e2e/run_block.py preflight

# 4) 부하 (Unity PlayMode 접속을 **먼저** 붙인 뒤)
python tests/e2e/unity_corr.py --out _workspace/p0-02-networking-spike/evidence/unity-corr.txt
python tests/e2e/run_block.py load --unity-corr _workspace/p0-02-networking-spike/evidence/unity-corr.txt

# 5) 기록 내구성 (AC-19)
TOKEN=$(tools/bots/target/debug/bots token --label bot-000 | sed -n 's/^token   : //p')
python tests/e2e/run_block.py durability --token "$TOKEN"

# 6) 서버 단독 항목 probe
python tests/e2e/run_block.py probes

# 7) 탐침 — **모든 측정이 끝난 뒤, 서버 정지 상태에서** (§B 규칙)
python tests/e2e/append_only_probe.py --evidence _workspace/p0-02-networking-spike/evidence/append-only.json
```

봇 단독 실행(블록 러너 없이):

```bash
EV=_workspace/p0-02-networking-spike/evidence/load
./target/debug/bots run --scenario a --bots 30 --seed 42 --duration 60 --ramp 5 --interval 500 --out $EV/a --live-corr $EV/a/correlations.live.txt
./target/debug/bots run --scenario b --bots 30 --cycles 5 --pings 3 --out $EV/b
./target/debug/bots run --scenario c --bots 30 --burst 2000 --duration 60 --out $EV/c
./target/debug/bots probe --case duplicate
```

종료 코드: `0` 통과 / `1` 게이트 위반(증거는 파일에) / `2` 사용법·설정 오류 / `3` 실행 실패(세션 0건).

### 3.1 봇 하네스 `tools/bots/` — 구현된 모습

- **별도 Cargo 워크스페이스.** 루트 `rust-toolchain.toml`(1.98.1)을 상속하고 고치지 않는다. 오프라인 빌드 17초.
- **계약 타입을 독립으로 썼다 — `starfall-contracts`를 path 의존하지 않는다.** `01_architect_tasks.md` T12의 지시와 다르므로 **architect 통지 사항**이다. 이유 둘:
  1. **관측자의 독립성(I-25).** 봇은 3자 대조의 한 축이다. 서버가 쓰는 바로 그 serde 타입으로 서버의 출력을 읽으면, 타입이 틀렸을 때 봇도 같이 틀려 차이가 나지 않는다.
  2. 계약이 코드보다 앞서 있어 `server/crates/contracts`가 지금 바뀌는 중이다. 묶이면 서버가 안 서는 동안 QA 도구도 서지 않는다.
  - 대가(봇이 잘못 읽을 위험)는 **계약 fixture로 막는다**: `tests/wire_fixtures.rs`가 `contracts/fixtures/**`를 봇 타입으로 역직렬화 → 재직렬화해 원본과 `Value` 비교한다. 즉 봇은 서버가 아니라 **계약**에 맞춰져 있다.
- 커버리지 스크립트가 찾는 **리터럴 상수** 4종을 `src/wire.rs`에 둔다(SC-70의 warnings 4건 해소 대상).
- **토큰**(ADR-0008 §1): `sha256("starfall-dev-subject:" + label)`의 앞 16바이트에 버전 7·변이 비트를 박아 `bot-000`~`bot-029`의 주체를 만들고, `hex(HMAC_SHA256(secret, subject_ascii))`를 붙인다. 파일로 저장하지 않는다. **ADR이 정하지 않은 두 가지(HMAC 메시지가 UUID 문자열인지, 파생 규칙)는 잠정 결정이며 server가 정본이다**(§3.3).
- **시드**는 초기 위상 지터에만 쓰고(30봇이 같은 순간에 몰려 보내는 인공 부하 제거) `summary.json`에 남긴다.
- **`--live-corr`**: `SESSION_READY`를 받는 **즉시** correlation을 파일에 덧붙인다. 이것이 없으면 **SC-61을 구조적으로 측정할 수 없다**(실행이 끝난 뒤 쓰는 `correlations.txt`로는 "도는 동안" 조회할 대상이 없다).
- **측정 규칙**: 왕복은 봇의 단조 시계(`Instant`)로만 잰다. `client_sent_at`은 `null`로 보낸다 — 판정에 쓰지 않는다는 것을 구조로 만든다(I-11). 벽시계(`clock_base_unix_ms`)는 `docker stop` 시각과 맞추는 용도로만 쓴다(AC-19 구간 슬라이싱).
- **분위수는 nearest-rank**다. 보간하면 p99가 실제 관측 표본이 아니게 되어 `commands.csv`로 되짚을 수 없다.

| 파일 | 내용 | 쓰이는 항목 |
|------|------|------------|
| `sessions.json` | `bot, session_id, correlation_id, actor_id, tick_hz, server_version, ready_tick, *_us, *_unix_ms, close_code, close_reason_text, close_initiator` | SC-52, SC-57~59, SC-61, SC-69, §0.6 |
| `commands.csv` | `bot,command_id,probe_seq,sent_us,result_us,status,reason_code,reply_us,order,rtt_ms` | SC-53~55, SC-63~65, SC-67, M-3/M-5 |
| `summary.json` | `gates`(one_to_one / accepted_reply_pairing / order_violations / server_initiated_closes / sessions_ready / correlations_collected / wire_errors / all_ok) + `aggregate`(손실·중복·미대응·거부 사유별) + `rtt`·`ack` 분포 + `connect_ms`·`ready_ms` + 시드·시각 | SC-53~56, SC-63~65, M-3, M-5, M-7 |
| `correlations.txt` | 종료 시점의 집합 | §0.6 |
| `correlations.live.txt` | **실행 중** 갱신되는 집합 | SC-61 |

- **A 단계는 Unity가 붙은 뒤 시작**한다. 31번째 연결은 **사람이 띄운 Editor의 PlayMode**여야 한다(§4 G-j).

### 3.2 검증 스크립트 `tests/e2e/` — 구현된 모습

셸이 아니라 **파이썬**으로 썼다. Git Bash와 PowerShell의 인용 규칙 차이로 같은 SQL 문자열이 셸마다 다르게 깨지면 "재현 가능한 명령"이 아니기 때문이다. 종료 코드 규약은 §0.3·§5 그대로(0/1/2/3).

| 파일 | 항목 | 비고 |
|------|------|------|
| `db.py` | 공통 | psql 호출 한 곳, 집합 읽기, **"구현 없음(3)"과 "환경 문제(2)"를 분리**, UTF-8 출력 |
| `run_block.py` | §4 블록 | `preflight` / `load`(A→SC-61→SC-62→SC-56→B→SC-57~60→C) / `durability`(AC-19 전 구간) / `probes` |
| `check_in_flight.py` | SC-61 | **A가 도는 동안에만** 가능. `correlations.live.txt`를 읽는다 |
| `poll_until.py rows` | SC-62 | 호스트 `count(*)` 폴링(기본 200 ms). `recorded_at`과 호스트 시계를 비교하지 않는다 |
| `poll_until.py backlog` | SC-66 / SC-68 | `--above 600`(=20 Hz×30초)으로 **시간이 아니라 상태**를 기다린다 |
| `check_sessions.py` | SC-57 / SC-58 (+SC-69) | 집합 기반 쌍·중복·누락·`actor_id` null·`close_reason` 분포 |
| `check_sequence_gaps.py` | SC-59 | 스펙 AC-16(c) SQL **원문**, 전 테이블 |
| `check_occurred_at.py` | SC-60 | `--selftest`로 DB 없이 공식만 검증 가능 |
| `append_only_probe.py` | SC-11~13 | 서버가 떠 있으면 거부(§B 규칙 1). 탐침 tick = `max(tick)+1`, `sequence` 0 |
| `three_way.py` | SC-56 | 봇 / `/debug/stats` / DB 3열 표. 메트릭 키가 없으면 판정하지 않고 기록 |
| `unity_corr.py` | SC-49~51, §0.6 | client가 고정한 로그 문구를 읽어 31번째 correlation을 뽑는다 |
| `README.md` | 절차 | 실행 순서에서 틀리기 쉬운 것 6가지 |

### 3.3 server 정본 정렬 — **완료 (2026-09-19)**

`03_server_impl.md`가 나와 확인 10건이 전부 답을 받았다. **서버가 정본이고 QA 도구를 고쳤다.**

| # | 항목 | server 정본 | QA 도구 상태 |
|---|------|------------|-------------|
| 1 | HMAC 서명 대상 | 주체 UUID의 **정규 소문자 하이픈 36자 ASCII 문자열**. 키 = 비밀의 UTF-8, 서명 = 소문자 hex 64자 | **가정이 맞았다 — 변경 없음** |
| 2 | `bot-NNN` 주체 | **`01a0b1c2-b010-7000-8000-000000000NNN`** (NNN = 000..029) | **교체함**(qa의 sha256 파생 폐기). `src/token.rs`의 `subject_for`, 기준값 2건을 `tests/token_vectors.rs`에 고정 |
| 3 | 엔드포인트·헤더 | `ws://127.0.0.1:8080/ws`, `Authorization: Bearer <subject>.<hmac_hex>`. `curl.exe`로 401/503을 볼 때 **WS 헤더 4종 필수**(없으면 400) | 그대로. §C의 `WSH` 배열이 이미 4종을 붙인다 |
| 4 | 환경 변수 | `STARFALL_DEV_AUTH_SECRET` `STARFALL_TICK_HZ` `STARFALL_WORLD_ID` `STARFALL_HTTP_ADDR` `STARFALL_LOG_FORMAT` `DATABASE_URL` `REDIS_URL`. **큐 용량·백로그 임계·프레임 한도·위반 예산은 상수라 설정으로 못 바꾼다** | 그대로 |
| 5 | `/debug/stats` | 최상위 키 **31개**. 라벨이 붙은 것은 **`[{label, count}]` 배열**이다(dict 아님) | **`three_way.py`의 `metric()`을 배열 대응으로 고쳤다.** 서버 모양 합성 payload로 7케이스 검증(스칼라·라벨 배열·없는 라벨=0·없는 키=None) |
| 6 | 401/503 `reason` | 503 `auth_not_configured`·`recording_backlog`·`shutting_down` / 401 `no_credential`·`invalid_token`. 본문 `{"status":"unavailable","reason":"…"}` | `run_block.py`가 503 본문을 파일로 남긴다. `upgrade_rejected_total`(5라벨)을 증거에 함께 기록 |
| 7 | 백로그 임계 | 600 맞다. **정상 동작에서도 `persist_backlog`는 0~20을 오간다**(20 tick 주기 커밋) | **판정 문구를 고쳤다**: SC-68의 목표를 `0` → **정상 대역 0~20**(`poll_until.py backlog --target 20`). 스펙 AC-19(c)는 "0으로"라 **architect 통지 사항** |
| 8 | 서버 메시지 envelope | `sequence`·`world_id`·`occurred_at`·`recorded_at`·`actor_id` **없다** | 가정이 맞았다. `deny_unknown_fields`라 달라지면 `wire_errors`로 즉시 드러난다 |
| 9 | `correlation_id` | `COMMAND_RESULT`·`PING_REPLY`는 **언제나 null**(키는 존재). 세션 correlation은 `SESSION_READY`에 있고 그 값이 `SESSION_OPENED`/`SESSION_CLOSED`와 같다 | 가정이 맞았다. §0.6의 집합이 바로 이 값이다 |
| 10 | 시드 월드 | `01a0b1c2-3d4e-7f01-8a2b-9c0d1e2f3a4b` / `spike` / tick_hz 20 / epoch `3800-01-01T00:00:00Z` / scale 60 / `sim_version=1` | 가정이 맞았다 |

**토큰 정렬 증거** — 세 출처가 문자열까지 같다(server 기준값 / Python 독립 계산 / Rust 구현):

```
bot-000  01a0b1c2-b010-7000-8000-000000000000.f0561d67b8c71be38992925b16251a890c508770f48f47a63eb51ebf5a880fc6
bot-029  01a0b1c2-b010-7000-8000-000000000029.c93a0e52ad2926a417dff00599b0dfc818bef26c343e24b34e4daad597dc3d71
bots identities --count 30 --disjoint-from 01a0b1c2-7e57-7c11-8e57-000000000001  →  disjoint=true checked=30
```

**server가 준 실행 주의 → 반영한 곳**

| server 주의 | 반영 |
|------------|------|
| SC-09는 정상 종료 경로에서만. 하드 킬 뒤 `last_tick < max(tick)`는 M-13 손실 창 | SC-09 문구 |
| SC-22 판정 정본은 **DB의 `close_reason`**(1011은 RST로 유실 가능). 수신을 완전히 멈추고 3000건 이상 | SC-22 문구 + `probe --case slow-consumer` 기본 3000 |
| SC-26은 30초 이상 **폴링 자체를 멈춰야** 한다(자동 Pong, U-9 = 참) | SC-26 문구 + `Idle` 45초 무폴링 |
| C 단계 폭주 봇은 **반드시 동시에 읽어야** 한다 | 게이트 **G-l** + `Flood`가 16건마다 읽도록 수정 |
| SC-23·SC-17은 봇으로 재현 불가 | SC-23 문구(단위 테스트 + 구조적 미도달 기록) |
| **측정 중 빌드 금지**(첫 측정 14.9초 → 재측정 0.079s) | 게이트 **G-k** |
| client의 실서버 왕복 중에는 `down -v` 금지 | 게이트 **G-m** |

**server가 고친 회귀 3건**은 QA가 이 항목들에서 다시 본다: 정상 Close 지연(2초 게이트) → SC-50·SC-52, 종료 시 close code 관측 → SC-27·SC-28, 폭주 클라이언트의 거부 응답 유실 → SC-20·SC-63.

### 3.4 봇 계측의 자체 검증 (실행 결과, 2026-09-19)

**"손실 0"이 신호인지 침묵인지 구분하려면 계측기가 틀렸을 때 빨간불이 켜져야 한다.** 그래서 정상 경로가 아니라 **고장 경로**를 테스트한다.

```
cd tools/bots && cargo test --offline     → 37 passed; 0 failed
  ledger_accounting  15건  합성 프레임으로 계측 로직
  live_loopback       7건  **진짜 WebSocket 소켓** 위에서 (가짜 서버)
  wire_fixtures       6건  contracts/fixtures 와 모양 대조
  token_vectors       9건  server 기준값 · Python 독립 계산 · Rust 구현 3자 대조
```

| 주입한 고장 | 방법 | 기대 | 결과 |
|------|----------|------|------|
| **응답 1건 누락** | 가짜 서버가 5건 중 3번째 `COMMAND_RESULT`를 안 보냄 | `missing_results=1`, `one_to_one` **false** | PASS (`dropped_command_result_on_the_wire_is_detected_as_loss`) |
| 순서 역전(I-15) | `PING_REPLY`를 `COMMAND_RESULT`보다 먼저 | `order_violations=4`, 1:1은 **true**(두 성질은 독립) | PASS |
| 중복 응답 | 같은 명령에 `PING_REPLY` 2건 | `duplicate_replies=3`, pairing **false** | PASS |
| 미대응 프레임 | 보낸 적 없는 `command_id`로 응답 | `unmatched_results=1` | PASS |
| `probe_seq` 불일치 | 다른 `probe_seq`로 응답 | `probe_seq_mismatches=1` | PASS |
| 서버 없음 | 닫힌 포트 | `connect_error` 기록, 세션 0, 종료 코드 **3** | PASS |
| 거부 폭주 | 100건 중 36건 `TOO_MANY_IN_FLIGHT` | 손실 0, 1:1 **true**(거부도 응답이다) | PASS |
| 깨진 서버 메시지 | 필수 필드 누락·널 불가 널 | 봇이 **거부**하고 `wire_errors`로 드러냄 | PASS |
| 실행 중 집합 수집 | 3초 연결 중 폴링 | 연결이 살아 있는 동안 correlation 1건이 파일에 | PASS (SC-61 전제) |
| `Authorization` 전달 | 가짜 서버가 헤더 수신 | `Bearer <token>` 그대로 | PASS |
| **30 동시 연결** | 가짜 서버에 봇 30개 동시 접속, 3초 유지 | 30개 전부 READY, 손실 0 | PASS — `sent=450 results=450 ready=30`, 4.5초 (U-6의 봇 쪽 절반, §3.5) |

### 3.5 U-6(자원 여유)에 대해 지금 말할 수 있는 것

U-6은 "30 봇 + Docker + Unity Editor 동시 실행의 자원 여유"다. **아직 해소되지 않았다** — 진짜 서버가 없으므로 절반만 답할 수 있다.

| 답할 수 있는 것 | 근거 |
|----------------|------|
| 하네스는 30 동시 연결을 감당한다 | 가짜 서버 상대 30봇 동시 실행: `ready=30`, `sent=450 == results=450`, 손실 0, 4.5초 (`thirty_concurrent_bots_are_handled_by_the_harness`) |
| 하네스는 **프로세스 1개**다 | 30봇 = tokio 태스크 30개. 30개 프로세스가 아니므로 메모리·핸들 압박이 작다 |
| 봇의 메모리는 명령 수에 비례하고 작다 | A 단계 전체가 명령 약 3,600건 = 레코드 3,600개. 종료 시 한 번에 직렬화한다 |
| 빌드·테스트 비용이 측정에 끼어들지 않는다 | 오프라인 빌드 17초, 전체 테스트 5초. 부하 실행 전에 끝난다 |

| 아직 못 답하는 것 | 어떻게 답할 것인가 |
|------------------|------------------|
| 서버가 31 연결에서 버티는가 | 서버 완성 후 A 단계 실행. `/debug/stats`의 tick 본문 분포와 RSS·CPU를 M-1·M-2·M-7로 기록 |
| Unity Editor를 **PlayMode로 띄운 채** 측정할 때 타이머 해상도가 바뀌는가 | 실행 시 §0.7의 측정 환경을 반드시 적는다. ADR-0006 §2.1의 바닥값(+0.7 %/분)과 나란히 비교 |
| Docker(타 프로젝트 컨테이너 5개 포함) + 서버 + Editor + 봇이 동시에 여유가 있는가 | `run_block.py preflight`가 컨테이너 상태를 먼저 기록한다. 부족하면 **FAIL이 아니라 E7(미검증(환경))**이고, 관측한 자원 수치를 함께 남긴다 |

**판단**: 애초 예상보다 위험이 낮다. 봇 쪽 한계로 부하가 깨질 가능성은 실행으로 배제했고, 남은 위험은 전부 서버·Editor 쪽이다. 첫 A 단계에서 문제가 나면 "하네스 탓인지"를 다시 의심할 필요가 없다는 것이 이 측정의 값이다.

**실서버 단발 확인(2026-09-19, 읽기 전용 1세션)** — 평가가 아니라 정렬 확인이다:

```
bots probe --case auth-ok --label bot-000
  actor_id=01a0b1c2-b010-7000-8000-000000000000  actor_id_matches_subject=true
  tick_hz=20  server_version=0.1.0  correlation_id=Some(…)  close: code=1000 initiator=client
```

토큰이 실서버에서 **수락**되고(101), 첫 메시지가 `SESSION_READY`이며, `actor_id`가 토큰 주체와 같다(I-10). 정렬이 맞다는 실행 증거다.

**그 확인이 하네스 버그 1건을 드러냈다 — 고쳤다.** 첫 실행에서 `close_initiator=server`가 나왔다. 정상 종료는 Close 프레임 **교환**이라 봇이 먼저 닫아도 서버의 Close가 뒤따라 오는데, 나중 값으로 덮어쓰고 있었다. 그대로 두면 **SC-52("서버가 먼저 닫은 연결 0건")가 모든 정상 세션에서 거짓 FAIL**이 된다. 가짜 서버 테스트는 Close 응답이 스트림 종료로 surface 되어 이 경로를 지나치지 못했다 — **실서버가 아니면 못 찾는 종류**였다.

- 고친 규칙: **먼저 닫은 쪽이 기록된다**(첫 기록 우선). 상대의 close code는 `peer_close_code`로 따로 남긴다(SC-28용).
- 회귀 테스트 2건 추가: `close_initiator_is_decided_by_whoever_closed_first`, `server_initiated_close_is_recorded_as_server`.
- 재확인: 같은 probe에서 `initiator=client`.

추가로 **`occurred_at` 공식의 독립 구현**(`check_occurred_at.py --selftest`)이 server가 PostgreSQL로 계산한 fixture 3건(tick 1200 / 24000 / 86400)과 **문자열까지 일치**함을 확인했다(8/8, mismatches=0).

---

## 4. 실행 순서와 게이트

| 게이트 | 조건 | 해제 전 결과의 취급 |
|-------|------|--------------------|
| **G-a** | `docker compose down -v`(AC-2)는 한 측정 세션의 **첫 DB 단계**다 | 그 이전의 모든 DB 증거는 **무효**. 다시 수집한다 |
| **G-b** | 마이그레이션 `.sql`이 바뀌면 `down -v && up -d` 후 전 DB 항목 재측정 | 체크섬 불일치로 인한 기동 실패는 FAIL이 아니라 **절차 오류** |
| **G-c** | SC-70(커버리지 `--strict`)은 **T1·T7·T8·T12 완료 후** | 그 전 결과는 판정에 쓰지 않는다(기준선 8/4) |
| **G-d** | SC-71~74(경계면)는 server T1 + client T7 완료 후 | 한쪽만이면 **대기**(판정 제외, 다음 라운드) |
| **G-e** | 부하(SC-52~69)는 서버 T5 + client T8 + qa T12 완료 후 | 미완이면 대기. **구현이 없으면 FAIL** |
| **G-f** | 위반·종료 계열(SC-22·24·25·26·27)은 **부하와 동시에 돌리지 않는다** | 위반·강제 close가 SC-52의 "서버가 먼저 닫은 연결 0"을 깨뜨린다 |
| **G-g** | 별도 기동이 필요한 항목(SC-08 tick_hz 불일치, SC-16 비밀 미설정)은 부하 블록 **밖**에서 | 부하 중 서버를 바꾸면 그 부하 측정이 무효 |
| **G-h** | Unity 콜드 임포트·EditMode(G 항목)는 부하와 **동시에 돌리지 않는다.** 그리고 **A 단계가 도는 동안 `client/` 아래 어떤 파일도 저장하지 않는다** — 도메인 리로드가 31번째 연결을 끊는다(client 요청, 2026-09-18) | 자원·타이머 해상도 영향(§0.7). 단 A 단계에는 Editor가 PlayMode로 붙어 있어야 한다. 저장으로 리로드가 일어나면 그 A 실행은 무효이고 **재측정**이다(FAIL 아님) |
| **G-j** | 31번째 연결은 **사람이 띄워 둔 Editor의 PlayMode**로 만든다. `unity test --mode PlayMode`로는 만들 수 없다 — 테스트가 끝나면 Editor가 내려가 연결이 같이 죽는다(client 실측, `03_client_impl.md` §1.4) | 자동화 시도로 31이 30이 되면 AC-15가 조용히 약해진다 |
| **G-i** | 탐침 블록(SC-11~13)은 **모든 부하·왕복 측정이 끝난 뒤**, **서버 정지 상태**에서 | 순서를 어기면 가짜 `UNIQUE` 위반과 `start_tick` 점프가 생긴다 |
| **G-k** | **측정 중에는 어떤 빌드도 돌리지 않는다**(`cargo build`·`unity test`·`dotnet run`). server 실측: 첫 측정 14.9초가 같은 PC의 컴파일 때문이었고 재측정은 0.079s·0.201s였다 | 빌드가 섞인 측정치는 **무효**다. 성능 기록(M-1~M-7)이 통째로 의미를 잃는다 |
| **G-l** | C 단계 폭주 봇은 **보내면서 계속 읽는다** | 읽지 않으면 설계대로 느린 소비자로 닫혀 AC-18(a)의 "연결이 유지된다"를 QA가 스스로 깨뜨린다(server 주의). 하네스는 16건마다 읽는다 |
| **G-m** | client가 실서버 왕복(SC-49~51)을 측정하는 동안 **`docker compose down -v`를 하지 않는다** | G-a의 `down -v`가 client의 증거를 지운다. 평가 라운드 시작 신호를 받은 뒤에 연다 |

**권장 실행 블록 순서**

| 블록 | 내용 | 항목 |
|------|------|------|
| 0 | 계약·빌드 (서버 기동 불필요) | SC-01~04, SC-32~37, SC-38~48 |
| 1 | `down -v` → `up -d` → 서버 기동 | SC-05~08 |
| 2 | 서버 단독 표면 (부하 없음) | SC-14~16, SC-17~21, SC-29 |
| 3 | 프레이밍·강제 종료 계열 | SC-22, SC-24~26, SC-28(부분) |
| 4 | Unity 실서버 왕복·재연결 | SC-49~51 |
| 5 | 부하 A1 → B → C (Editor PlayMode 유지) | SC-18, SC-30, SC-31, SC-52~61, SC-63~65, SC-62 |
| 6 | 부하 A2 + D (기록 내구성) | SC-66~69 |
| 7 | 정상 종료 → 재기동(tick 재개) | SC-27, SC-09, SC-10, SC-59(보조 재실행) |
| 8 | 탐침(서버 정지) | SC-11~13, SC-59(최종 확인) |
| 9 | 커버리지·경계면·리포트 | SC-70~74 |

**모듈 완료 알림 시 QA가 즉시 보는 경계면**(Phase 4, 라운드 판정과 별개):
T1 완료 → SC-32~37 + `contracts/` ↔ Rust 필드 비교 / T7 완료 → SC-38~41 + `contracts/` ↔ C# 필드 비교 / T2 완료 → SC-05~06, SC-11~13 / T4·T5 완료 → SC-14~31 / T8·T9 완료 → SC-42~51.

---

## 5. "미검증(환경)" 처리 기준 (평가 전에 합의)

아래 조건이면 **FAIL이 아니라 미검증(환경)**으로 기록하고 필요한 조치를 리더에게 보고한다.

| 코드 | 조건 | 영향 항목 | 리포트 표기 |
|------|------|----------|------------|
| E1 | cargo 툴체인 사용 불가(다운로드·오프라인) | SC-01~04, SC-17, SC-23, SC-32~37 | 미검증(환경) — 툴체인 |
| E2 | Docker Desktop 미가동 / 컨테이너 unhealthy / 15432·16379 점유 | SC-05~13, SC-29, SC-57~62, SC-66~69 | 미검증(환경) — 인프라 |
| E3 | Unity Editor 라이선스 실패 / `unity test`가 에디터를 띄우지 못함 | SC-39, SC-42~51 | 미검증(환경) — Unity CLI 로그 첨부 |
| E4 | .NET SDK 미가용 | SC-38, SC-40, SC-41 | 미검증(환경) |
| E5 | 서버가 **환경 문제로** 기동하지 못함(포트 8080 점유 등) | SC-05~31 | 미검증(환경) — `netstat -ano` 첨부 |
| E6 | 봇 하네스를 빌드할 수 없다(툴체인·의존성) | SC-14, SC-19~26, SC-52~56, SC-63~65 | 미검증(환경) |
| E7 | 31 연결 + Docker + Unity Editor 동시 실행 자원 부족(U-6) | SC-52~69 | 미검증(환경) — 관측한 자원 수치 기록 |
| E8 | 선행 태스크 미완 | SC-70~74 등 | **대기**(판정 제외, 다음 라운드) |

**"환경이 없어서"와 "구현이 없어서"를 섞지 않는다. 구현이 없으면 FAIL이다.**
**간헐 실패는 재시도로 덮지 않는다** — 같은 명령을 두 번 돌려 결과가 다르면 비결정성 이슈로 FAIL 기록한다.

---

## 6. 기록 항목 (판정하지 않음)

값이 안 나와도 FAIL이 아니지만, **리포트와 `03_*_impl.md`에 숫자가 있어야** 다음 슬라이스가 비교 기준을 갖는다.

| # | 기록할 것 | 잠정 기준(비차단) | 담당 | 근거 |
|---|----------|------------------|------|------|
| M-1 | **tick 초과 비율** = `tick_overrun_total / tick_total`, 기준은 **`run_tick()` 본문 소요 > 50 ms**. 측정 지점을 파일:라인으로 함께 적는다(루프 간격으로 재면 완벽한 타이머도 100 %가 된다) | ≤ 0.5 % | server, qa | 스펙 §7, ADR-0006 §2.2 |
| M-2 | 단일 tick **본문** 최대 소요 | ≤ 250 ms | server, qa | 스펙 §7 |
| M-3 | 왕복 p99(`PING_SERVER`→`PING_REPLY`, **봇 시계**) | ≤ 150 ms | qa | 스펙 §7 |
| M-4 | `tick_lag_seconds`와 **이 PC 바닥값 +0.7 %/분**을 나란히 | — | server, qa | ADR-0006 §2.1 |
| M-5 | 왕복 p50 / tick 본문 소요 p50·p99·max (고정 버킷 히스토그램) | — | qa | 스펙 §7 |
| M-6 | `command_queue_depth` p99/max. **in-flight 상한이 먼저 걸려 낮게 나온다**(30×64=1920 < 4096)는 설명을 함께 | — | qa | ADR-0006 §5 |
| M-7 | 31 연결 수립 소요, 서버 RSS·CPU 피크 | — | qa | 스펙 §7 |
| M-8 | **측정 환경**: Unity Editor 실행 여부·상태, 동시 컨테이너 수, 봇 시드, 단계 시각 | — | qa | §0.7 |
| M-9 | 서버 워크스페이스 클린 빌드 시간(p0-01의 26초 대비) / Unity EditMode 콜드·웜(63초·11초 대비) | — | server, client | 스펙 §8 |
| M-10 | `SERVER_BUSY`가 부하 실행에서 **미도달(구조적)**이라는 기록 | — | qa | AC-7(d) |
| M-11 | U-5a(`SetRequestHeader` 실동작) · U-5b(도메인 리로드 거동) · U-9(tungstenite 자동 Pong) 실측 결과 | — | client, server | 스펙 §10 |
| M-12 | C# "감지 불가" 5건(§0.5 #2,7,8,10,15)의 관찰 결과 — 설계 비대칭으로 그대로 기록 | — | qa | 스펙 §5.4 |
| M-13 | **손실 창**: outbox가 없어 강제 종료(kill) 시 미커밋 tick이 사라질 수 있다. 이번 슬라이스는 **측정하지 않고 사실만 기록**한다(정상 종료 경로의 손실 0은 SC-27이 본다) | — | qa | 스펙 §8, ADR-0007 §6 |

---

## 7. 판정·라운드 규칙

- 라운드는 **최대 3회**. 각 라운드 결과는 `_workspace/p0-02-networking-spike/04_qa_report_r{N}.md`.
- FAIL 항목은 **파일:라인 + 재현 명령 + 기대/실제**를 담아 담당자에게 보낸다. QA는 구현 코드를 고치지 않는다(`tests/e2e/`, `tools/bots/`만 QA 소유).
- 경계면 불일치는 **생산자와 소비자 양쪽**에 알리고, 계약 자체가 모호하면 architect에게도 알린다.
- **§0.5 표와 결과가 다르면 FAIL이 아니라 architect 통지**(계약 설계가 바뀐 것이다).
- 3라운드 후에도 FAIL이 남으면 남은 항목·원인 추정·선택지(범위 축소 / 스펙 수정 / 추가 라운드)를 리더에게 보고한다.
- **리포트는 정확성 판정과 성능 기록을 별도 절로 쓴다.** 성능 미달이 "FAIL"로 보이는 표를 만들지 않는다(§0.4).

---

## 8. 구현자 확인란

각자 "이 방법으로 완료를 증명할 수 있다"에 표시한다. 이의가 있으면 아래 표에 적고, QA가 §9에 반영한 뒤 다시 확인을 받는다.

| 영역 | 항목 | 담당 | 확인 | 서명/날짜 |
|------|------|------|------|----------|
| A 빌드·크레이트 위생 | SC-01 ~ SC-04 | server | ☐ | |
| B 마이그레이션·tick 재개·append-only | SC-05 ~ SC-13 | server | ☐ | |
| C 인증 | SC-14 ~ SC-16 | server | ☐ | |
| D tick·큐·수명 주기·프레이밍 | SC-17 ~ SC-28 | server | ☐ | |
| E 운영 표면 | SC-29 ~ SC-31 | server | ☐ | |
| F 계약 ↔ Rust | SC-32 ~ SC-37 | server | ☐ | |
| G 생성기·EditMode | SC-38 ~ SC-48 | client | ☐ | |
| H 실서버 왕복·재연결 | SC-49 ~ SC-51 | client | ☐ | |
| I~L QA 부하·커버리지·경계면 | SC-52 ~ SC-74 | qa | ☑ | qa / 2026-09-18 |
| 부록 | §0.5 층별 표, §2 비교표 대상, §4 게이트, §5 미검증 기준 | server·client 공통 | ☐ ☐ | |

**이의 제기**

| # | SC ID | 제기자 | 이의 내용 (무엇이 증명 불가능한가 / 어떤 문구로 바꾸면 되는가) | 처리 |
|---|-------|-------|--------------------------------------------------|------|
| 1 | | | | |
| 2 | | | | |
| 3 | | | | |

### 확인 쟁점 — **client 답변 반영 완료(6~9), server 답변 대기(1~5, 10)**

1. **SC-31 — 송신 큐 잔량을 독립 관측할 수단이 없다.** `/debug/stats` 목록에 송신 큐 게이지가 없어 "`enqueued − written`이 큐 잔량과 일치"를 직접 확인할 수 없다. 그래서 **모든 세션이 닫힌 정지 시점에 `enqueued == written`**으로 판정하도록 적었다. 이 판정으로 충분한가, 아니면 세션별 송신 큐 잔량 합계를 노출할 것인가? (server)
2. **SC-27 — stdin 종료 절차.** D-2의 PowerShell `RedirectStandardInput` 방식이 실제 바이너리에서 동작하는가? 바이너리 이름·경로(`target/debug/starfall-game-server.exe`)가 맞는가? `cargo run`으로 띄웠을 때도 stdin이 서버까지 전달되는가? (server)
3. **SC-08 / SC-16 — 설정 주입 방법.** `config.rs`가 `.env`를 읽지 않으므로 QA는 셸 환경 변수로 주입한다. `STARFALL_TICK_HZ`·`STARFALL_DEV_AUTH_SECRET`·`STARFALL_WORLD_ID` 이름이 확정인가? 기동 거부 시 종료 코드와 로그 문구를 어떻게 잡을 수 있는가? (server)
4. **SC-14~SC-26 — 봇 probe가 필요한 서버 단독 항목.** 이 항목들의 1차 증명은 server의 통합·단위 테스트이고 QA는 봇 probe로 재현한다. **서버 테스트 이름 ↔ SC ID 대응표**를 `03_server_impl.md`에 남겨 달라. 재현이 불가능한 항목이 있으면 지금 알려 달라. (server)
5. **SC-24 — 위반 계수의 관측 지점.** `protocol_violations_total`이 위반 1회마다 증가하는가(세션별 예산과 별개로)? 8회 예산의 10초 창 시작 시점은 무엇인가(첫 위반 시각 기준인가)? (server)
6. ~~Unity 세션의 correlation 수집 경로~~ → **해결(client, 2026-09-18).** 문구와 **필드 순서**가 고정됐다: `starfall.net: SESSION_READY session_id=… correlation_id=… actor_id=… world_id=… tick_hz=… server_version=… attempt=…`. 위치는 `client/Logs/Editor.log`(스택 트레이스 붙음)와 **`client/Logs/starfall-net.log`**(한 줄만, QA 자동 수집용). `dropping`·`reconnect`·`closing` 3개도 같이 고정됐다. → `tests/e2e/unity_corr.py`가 이 4개만 읽는다.
7. ~~Unity의 개발용 토큰 주체~~ → **해결(client).** `01a0b1c2-7e57-7c11-8e57-000000000001`. 봇 30개와 겹치지 않음을 도구로 확인: `bots identities --count 30 --disjoint-from 01a0b1c2-7e57-7c11-8e57-000000000001` → `disjoint=true checked=30` (+ `tests/token_vectors.rs`가 회귀로 고정).
8. ~~SC-61 측정 창과 접속 순서~~ → **해결(client).** 31번째 연결은 **사람이 띄운 Editor의 PlayMode**여야 한다 — `unity test --mode PlayMode`는 테스트가 끝나면 Editor가 내려가 연결이 같이 죽는다. `STARFALL_NET_AUTOCONNECT=1` 부트스트랩과 `Starfall/Net/*` 메뉴가 있다(`03_client_impl.md` §1.4). → §4 게이트 **G-j** 신설, **G-h**에 "A 중 `client/` 파일 저장 금지" 추가.
9. ~~46필드 열거 규약의 비대칭~~ → **해결(client).** 비대칭은 설계 의도가 아니라 **계산 착오**였고 대칭이면 48이다. 총계 46(스펙 AC-21 고정값)은 유지하되 **판정 근거는 `03_client_impl.md` §9의 필드별 46행 표**로 한다. "46 vs 48"은 **architect 통지 사항**(FAIL 아님). → §2 개정.
10. **탐침 순서(§B 규칙 3~4).** 탐침 행이 `start_tick`을 밀어 올리는 것이 정상 동작임에 동의하는가? 탐침을 마지막 블록으로 미루는 것에 이의가 없는가? (server)

---

## 9. 계약 변경 이력

| 날짜 | 변경 | 사유 |
|------|------|------|
| 2026-09-19 (2차) | **server 정본 정렬(§3.3 확인 10건 전부 해소).** 봇 주체 파생을 server 공식(`01a0b1c2-b010-7000-8000-000000000NNN`)으로 교체, `/debug/stats` 라벨 배열 대응, SC-31을 4값 회계 항등식으로, SC-68 목표를 정상 대역 0~20으로(스펙 문구 "0으로"는 **architect 통지**), SC-09를 정상 종료 경로 한정으로, SC-22 판정 정본을 DB `close_reason`으로, SC-26을 무폴링 45초로, SC-23을 "봇 재현 불가"로, SC-30에 `live_connections` 병기. 게이트 **G-k**(측정 중 빌드 금지)·**G-l**(폭주 봇은 읽는다)·**G-m**(client 측정 중 `down -v` 금지) 신설 | server 구현 완료(게이트 3종 통과, 테스트 82건) + server의 실행 주의 7건 |
| 2026-09-19 | **client 답변 반영 + QA 도구 구현 결과 반영.** §2를 "총계가 아니라 `03_client_impl.md` §9의 46행 표가 판정 근거"로 개정(46 vs 48은 architect 통지). §4에 **G-j**(31번째 연결은 사람이 띄운 Editor의 PlayMode) 신설, **G-h**에 "A 단계 중 `client/` 파일 저장 금지" 추가. SC-49·50·51·61의 검증 방법을 client가 고정한 로그 4문구와 `unity_corr.py`로 교체. §3을 "설계"에서 **"구현 완료 + 실행 명령 + 자체 검증 결과 + server 대조 항목 10건"**으로 교체 | client 구현 완료(EditMode 65건 통과, 생성기 좁힘 수정), qa T12·T13 구현 완료 |
| 2026-09-18 | 최초 작성 (SC-01 ~ SC-74, M-1 ~ M-13) | 스펙 §7 AC-1~21을 실행 항목으로 변환. 반례 거부를 스키마층(SC-34)·serde층(SC-35)·C#층(SC-44)으로 분리하고 "감지 불가" 5건을 기록(M-12)으로 고정. 세션 대조를 correlation 집합(§0.6)으로. 정확성/성능 분리(§0.4)와 하드 게이트 예외 2건(AC-17b·AC-18c) 명시. 탐침 행의 tick 재개 부작용을 절차로 고정(§B 규칙). `down -v`를 첫 DB 단계로 하는 게이트 G-a 신설 |

# p0-01-bootstrap QA 리포트 — 라운드 1

- 일시: 2026-09-18 21:38 ~ 21:46 (KST)
- 평가자: qa. 기준: `_workspace/p0-01-bootstrap/02_sprint_contract.md` (2차 정정판)
- 입력: `03_server_impl.md`, `03_client_impl.md`, `02_server_ack.md`, `02_client_ack.md`, 갱신된 `docs/specs/p0-01-bootstrap.md`, `docs/adr/0001~0004`, 코드 전체
- **요약: PASS 37 / FAIL 0 / 미검증(환경) 0 / 기록 1 / 전체 38**

모든 항목을 QA가 **직접 실행**해 판정했다. 구현자 보고는 대조용으로만 썼고, 보고와 어긋난 칸은 없었다. 순회 항목은 전부 순회 개수가 출력에 드러났다(§0.3 빈 순회 방지).

추가로 계약이 요구하지 않은 **독립 교차 검증 1건**을 수행했다: 제3자 검증기(Python `jsonschema` 4.23.0, 오프라인 레지스트리)로 유효 fixture 4건 통과·반례 7건 거부·**C# DTO가 재직렬화한 출력의 스키마 적합**을 Rust·C# 구현과 무관하게 확인했다(§4 A·B·C).

## 0. 이번 라운드 전에 고친 스프린트 계약 (QA 소유 파일)

구현 중 실행으로 드러난 오류 4건 + 보강 1건을 `02_sprint_contract.md`에 반영한 뒤 평가했다. **스펙 AC를 늘리거나 줄이지 않았고, 증거 수집 방법만 고쳤다.**

| 항목 | 고친 내용 | 근거 |
|------|----------|------|
| SC-22 | `--report-format both` → **`nunit,junit`** | `both`는 이 CLI에서 exit 2 (client 실측). 스펙 AC-7도 같은 내용으로 갱신됨 |
| SC-29 | 로그 경로 → **`client/Logs/Editor.log`** | `%LOCALAPPDATA%\Unity\Editor\Editor.log`는 테스트 실행으로 갱신되지 않는다. 스펙 AC-8도 갱신됨 |
| SC-03 | `git status --porcelain server/Cargo.lock` 보조 증거 **폐기** | 커밋 1건뿐이라 항상 `?? server/Cargo.lock`이다. "빈 출력 = lock 불변"이 성립하지 않는다. `--locked` exit 0이 이미 증거 |
| SC-11 | 판정을 **before/after diff**로 교체 | 이 PC에 타 프로젝트 익명 볼륨이 **43개** 이미 있어(QA 실측) "없음" 분기가 도달 불가였다 |
| SC-10 | `pg_isready` 대기 루프 추가 | `up -d` 직후 `psql`은 초기화 중이라 실패한다. 대기 없는 실패는 비결정성이 아니라 절차 오류 |

## 1. 항목별 결과

### A. 서버 빌드·품질 게이트 (AC-1)

| ID | 결과 | 증거 | 비고 |
|----|------|------|------|
| SC-01 | PASS | `cd server && cargo fmt --all --check` → `exit=0`, 출력 없음 | |
| SC-02 | PASS | `cargo clean -p starfall-contracts -p starfall-gateway -p starfall-game-server`(1778 파일 제거) 후 `cargo clippy --workspace --all-targets -- -D warnings` → 세 크레이트 `Checking` 재실행, **exit=0, warning 0** | 캐시 결과가 아님을 보이려고 일부러 clean 후 재실행 |
| SC-03 | PASS | `cargo test --workspace --locked` → `exit=0`. 스위트 4개 합계 **32 passed / 0 failed**(doctest 1 ignored) | `--locked` 통과 = `Cargo.lock` 최신 |

보조 증거(판정 아님): `[lints] workspace = true` 옵트인이 **세 멤버 전부**에 있다 — `crates/contracts/Cargo.toml:10-11`, `crates/gateway/Cargo.toml:10-13`(주석 2줄 뒤 `workspace = true`), `bins/game-server/Cargo.toml:10-11`. clippy 조정은 `server/clippy.toml`의 `allow-unwrap-in-tests`/`allow-expect-in-tests`/`allow-panic-in-tests`로 테스트에만 한정되며 운영 코드에는 적용되지 않는다(T1 지시 준수).

### B. 서버 기동과 운영 엔드포인트 (AC-2, AC-3)

| ID | 결과 | 증거 | 비고 |
|----|------|------|------|
| SC-04 | PASS | `curl.exe -s -w "\nHTTP %{http_code}\n" .../healthz` → `{"status":"ok","version":"0.1.0"}` + `HTTP 200`. 기동 로그: `INFO starfall_game_server: … addr=127.0.0.1:8080 healthz=http://127.0.0.1:8080/healthz` | `.env` 없이 기동(§5 계약 외 발견 3) |
| SC-05 | PASS | `/readyz` → `{"status":"ready","checks":{"postgres":"ok","redis":"ok"}}` + `HTTP 200`. 닫힌 집합 검사 스크립트 `closed-set OK`. 정상 상태에서 **10/10, 12/12, 7/8 연속 200** | 컨테이너 **재생성 직후 첫 1회만** 503 — §5 계약 외 발견 1, 판정 근거는 아래 |
| SC-06 | PASS | `docker compose stop postgres` → `/readyz` `HTTP 503`, `{"status":"not_ready","checks":{"postgres":"unavailable","redis":"ok"}}`, 같은 시점 `/healthz` **200**, PID **28824 불변** | |
| SC-07 | PASS | `docker compose start postgres` → `pg_isready` 통과 후 `/readyz` **200**, PID **28824 동일**(서버 재시작 없음) | |
| SC-08 | PASS | 200·503 두 상태 본문 모두에서 `postgres://\|redis://\|starfall_dev_only\|password\|sqlx\|io error\|connection refused\|15432\|16379` **0건**. 두 상태 모두 `checks` 값이 `{"ok","unavailable"}` 안 | 원인은 로그에만: `WARN readiness: PostgreSQL 점검 실패 error=pool timed out …` |

**SC-05 판정 근거(중요).** `docker compose down` + `up -d`로 컨테이너를 **재생성한 직후 첫 호출**이 `{"postgres":"ok","redis":"unavailable"}` + 503을 내고 **두 번째 호출부터 200**이 된다. 2회 재현했다(서버 로그 `12:43:06`, `12:44:26` — 둘 다 `WARN readiness: Redis 점검 실패 error=broken pipe`). 이것을 §0.3의 "간헐 실패 → FAIL"로 보지 않은 이유는:

1. **재현 가능한 상태 전이이지 무작위가 아니다.** 입력이 다르다 — 첫 호출은 파괴된 컨테이너를 가리키는 죽은 소켓 위에서 일어난다. 정상 상태에서는 29회 연속 200이었다.
2. **그 순간 "not ready"는 올바른 답이다.** 캐시된 연결이 실제로 끊겨 있었다. 확인 없이 `ok`를 반환하는 쪽이 오히려 결함이다.
3. 값이 **닫힌 집합 안**에 있었고(`unavailable`), **서버 재시작 없이 자동 복구**됐다 — AC-3이 요구한 복구 성질을 오히려 한 번 더 증명한다.

다만 운영상 의미가 있어 §5에 계약 외 발견으로 남긴다(단발 `/readyz` 호출로 게이트를 만들면 CI가 흔들린다).

### C. 로컬 인프라 (AC-4)

| ID | 결과 | 증거 | 비고 |
|----|------|------|------|
| SC-09 | PASS | `docker compose up -d` → `docker compose ps`: `postgres \| Up 10 seconds (healthy)`, `redis \| Up 10 seconds (healthy)` | |
| SC-10 | PASS | QA 자체 마커 `_boot_marker_qa_r1`(1행 삽입) → `docker compose down`(**-v 없이**) → `up -d` → `select to_regclass(...)` → `server_marker=_boot_marker`, `qa_marker=_boot_marker_qa_r1`, `rows_survived=1` | server가 남긴 `_boot_marker`도 함께 잔존 |
| SC-11 | PASS | before/after 볼륨 diff **무출력**(95개 그대로), 새로 생긴 64자 해시 볼륨 **0건**, 컨테이너 목록 diff 무출력, `livingfeed-*` 5개 `Up 26 hours` 유지. 마운트: `volume starfall_postgres-data -> /var/lib/postgresql` | 전역 prune·`down -v` 미사용 |

**M-4 재확인:** 마운트 대상이 `/var/lib/postgresql`(PG18 `VOLUME`)이고, 이 경로 덕분에 SC-10의 데이터 잔존이 성립한다.

### D. 계약 ↔ Rust (AC-5)

명령: `cd server && cargo test -p starfall-contracts --locked --test contract_tests -- --nocapture --test-threads=1` → **10 passed / 0 failed**, `exit=0`.

| ID | 결과 | 증거 (테스트 이름 + 출력에 찍힌 개수) | 비고 |
|----|------|--------------------------------------|------|
| SC-12 | PASS | `schemas_valid_offline` → `[SC-12] 오프라인 검증한 스키마: 7건` (PING_SERVER, command/event/message-envelope, primitives, PING_REPLY, types.schema) | 네트워크 접근 없음(`offline()`) |
| SC-13 | PASS | `fixtures_roundtrip` → `[SC-13] 왕복 검증한 유효 fixture: 4건` + 파일명 4개 | |
| SC-14 | PASS | `invalid_rejected_by_schema` → `[SC-14] 스키마가 거부한 반례: 7건` + 파일명 7개 | **층①**. QA 독립 검증기로도 7건 거부 재확인(§4 B) |
| SC-15 | PASS | `invalid_serde_matrix` → 7행 전부 `기대=거부 실제=거부`, 각 행에 serde 실제 오류 문자열. `[SC-15] serde 매트릭스 검사한 반례: 7건` | **층②**. §0.4 표와 **모든 칸 일치** |
| SC-16 | PASS | `registry_server_types_mapped` → `server 태그 타입 2건`, `PING_SERVER -> …PingServerCommand`, `PING_REPLY -> …PingReplyMessage` | |
| SC-17 | PASS | `registry_consistency`(항목 2 / fixture 4) + `schema_ids_match_paths`(`$id` 7건) + `registry_file_validates_against_schema` | 세 번째는 server가 분리 신설 |
| SC-18 | PASS | `required_field_mutations` → `[SC-18] required 변이 28건이 모두 역직렬화에 실패했다` | `client_sent_at`·`correlation_id` 키 제거 변이 포함 |

SC-15 보조 증거(운영 경로 구멍 점검): `grep -rn --include=*.rs -E '^\s*#\[serde\((flatten|tag)' crates bins` → **0건**. 계약서 초안의 느슨한 grep으로 잡히는 4~5줄은 전부 "쓰면 안 된다"고 설명하는 `//!` 문서 주석과 테스트 실패 메시지였다(`commands.rs:3,6`, `dispatch.rs:9,14`, `contract_tests.rs:447`) — 실제 속성 사용 아님.

### E. 계약 → C# 생성기 (AC-6)

| ID | 결과 | 증거 | 비고 |
|----|------|------|------|
| SC-19 | PASS | 생성기 2회 실행 후 `sha256sum` 비교 `diff` 무출력(`IDENTICAL`). 3파일 해시 기록 | 출력 결정적 |
| SC-20 | PASS | 스크래치 사본에 `// qa mutation r1` 한 줄 추가 → `--check` `exit=1` + `differs   PingServerCommand.cs   (<경로>)` | 복구 후 재실행 `up to date (3 file(s))`, `exit=0` |
| SC-21 | PASS | `QaOrphan.cs` 추가 → `--check` `exit=1` + `orphan    QaOrphan.cs   (<경로>)` | **파일 집합 비교가 살아 있다** |

`client/` 실물을 건드리지 않고 스크래치 사본(`--out <스크래치>`)에서 재현했다. 사본에는 `.meta` 3개가 함께 복사됐는데 `--check`가 이를 고아로 신고하지 않았다 — `.cs`만 비교한다는 설계대로다.

### F. Unity 클라이언트 (AC-7, AC-8)

명령: `unity test client --mode EditMode --no-banner --report-format nunit,junit --output …/EditMode.nunit.xml --junit-output …/EditMode.xml`

| ID | 결과 | 증거 | 비고 |
|----|------|------|------|
| SC-22 | PASS | `exit=0`, 리포트 **2개 파일 모두 생성**(EditMode.nunit.xml 21,529B / EditMode.xml 7,306B, 21:40 갱신). `tests="18" failures="0" errors="0" skipped="0"`, nunit `result="Passed" passed="18"` | 웜 실행 8.9초 |
| SC-23 | PASS | `Fixtures_RoundTrip_MatchesOriginal(...)` 4 케이스 + `Fixtures_RoundTrip_VisitedAllFourValidFixtures`(visited 4건). 각 케이스 `system-out`에 재직렬화 JSON 전문 | 소스 확인: `JToken.DeepEquals(before, after)` (ContractFixtureTests.cs:70) |
| SC-24 | PASS | `Invalid_Rejected_ByStrictProfile(...)` **5건** 전부 예외 + `Invalid_Rejected_VisitedAllFiveCSharpCases`(visited 5건). 테스트가 `InvalidFixtures().Count == 7`도 단언(:116) | 예외 메시지 5건 모두 기록됨 |
| SC-25 | **기록** | `Invalid_NotDetectableByCSharp_DocumentedAsymmetry(...)` 2건이 **수락(ACCEPTED)**됨 — `command-id-not-v7`, `tick-above-safe-integer`. 테스트 출력이 사유를 명시 | **스펙 §5 표와 일치 → architect 통지 사유 아님.** 아래 별도 절 |
| SC-26 | PASS | `Dispatch_PreservesRealTime` → `via ContractDispatch.TryRead : 2026-09-17T14:05:09.123Z` / `via default JObject.Parse : 09/17/2026 14:05:09` | 회귀 가드 양쪽 값 모두 출력 |
| SC-27 | PASS | `UuidV7_MatchesSchemaPattern`(계약에서 읽은 패턴으로 **1,000건** 검사) + `UuidV7_NoDuplicatesWithinSameMillisecond`(같은 ms `1789735252536` 안에서 **10,000건, 중복 0**, 공유 접두사 `01a0b488-b238`) | 패턴을 복사하지 않고 `primitives.schema.json`에서 직접 읽는다 |
| SC-28 | PASS | `FixtureLoader_FindsRepoRootByMarker` → `repo root: C:\WorkSpace\SpaceHistoric`, `marker: contracts/registry/types.json`, 유효 fixture 4건 열거. `FixtureLoader_FailsWhenFewerThanFourValidFixtures` → `Assert.Throws<InvalidOperationException>` 2건 + 정상 목록 `DoesNotThrow` | 소스 확인: `ValidFixtures()`가 `TopDirectoryOnly`라 `invalid/`를 빨아들이지 않고, 테스트가 경로에 `/invalid/` 없음을 단언(:242) |
| SC-29 | PASS | `rm -rf client/Library` 후 콜드 실행 → `exit=0`, **1m04.9s**, 18/0/0/0. `client/Logs/Editor.log`(1.59MB, 21:45): `error CS` **0건**, `Assets/_Project` 경고 **0건**, 전체 `warning CS`도 **0건**. `Library/ScriptAssemblies/Starfall.Contracts.dll`·`Starfall.Tests.EditMode.dll` 재생성 확인 | 게이트 G-e대로 마지막에 실행 |
| SC-30 | PASS | `m_EditorVersion: 6000.6.1f1` → 추출값 `[6000.6.1f1]`, 문자열 동일 | `m_EditorVersionWithRevision: 6000.6.1f1 (7efac9f6c10e)` |

**SC-25 기록(판정 제외) — C#이 잡지 못하는 반례 2건**

| fixture | 스키마 검증 | Rust serde | C# Strict | 스펙 §5 표 | 일치 |
|---------|------------|-----------|----------|-----------|------|
| `PING_SERVER/invalid/command-id-not-v7.json` | 거부 | 거부 (`UuidV7 는 버전 7이어야 한다 (받은 값의 버전: 4)`) | **수락** (`Guid`가 버전을 보지 않음) | 감지 불가 | ✅ |
| `PING_REPLY/invalid/tick-above-safe-integer.json` | 거부 | 거부 (`Tick 는 0 ..= 9007199254740991 범위여야 한다`) | **수락** (`long`이 2^53 이상을 담음) | 감지 불가 | ✅ |

**기록된 설계 비대칭이며 결함이 아니다.** 두 건 모두 스키마 검증(QA 독립 검증기로도 거부 확인)과 Rust serde가 **서버 경계에서** 막는다. 클라이언트는 서버 메시지의 소비자일 뿐이고, 신뢰 경계는 서버에 있다(원칙 1). 결과가 표와 달라졌다면 통지 사유였겠지만, 이번 라운드에서는 표와 정확히 일치했다.

### G. QA 통합·경계면·저장소 상태 (AC-9, AC-10, AC-11)

| ID | 결과 | 증거 | 비고 |
|----|------|------|------|
| SC-31 | PASS | `python .claude/skills/integration-qa/scripts/check_contract_coverage.py --strict` → `types: 2, errors: 0, warnings: 0`, `RESULT: PASS`, `exit=0`. 두 타입 모두 producer·consumer 코드 위치가 실제 파일로 해석됨 | **게이트 G-a 준수**: T2·T7 완료 후 실행. (계약 작성 시점의 사전 실행은 warnings 4로 실패했고 판정에 쓰지 않았다) |
| SC-32 | PASS | 아래 3자 비교표 — 필드 12개 전부 이름·필수·널 가능 **일치, 불일치 0건** | |
| SC-33 | PASS | 정수 3종 전부 스키마 범위를 손실 없이 표현. 범위 밖은 스키마 검증이 거부(독립 검증기로 확인) | |
| SC-34 | PASS | 층별 매트릭스가 스펙 §5 표와 **모든 칸 일치**(아래 표) | |
| SC-35 | PASS | `git log --oneline` → `2d9cf08 Initial commit` 1건. `rev-list --count HEAD=1`, `HEAD=2d9cf085…`, `origin/main=2d9cf085…` **동일** | 새 커밋 0건 |
| SC-36 | PASS | `git status --porcelain`에 `contracts/`·`docs/`·`_workspace/`·`server/`·`tools/`·`client/` untracked. `Generated/`의 `.cs` 3건과 `.meta` 3건 + `Generated.meta` 모두 추적 후보로 보임 | |
| SC-37 | PASS | `git check-ignore -v` 8경로 전부 `rc=0`과 매칭 규칙 출력(`client/[Ll]ibrary/`, `client/[Tt]emp/`, `client/[Ll]ogs/`, `client/[Oo]bj/`, `client/[Uu]ser[Ss]ettings/`, `target/`, `tools/**/obj/`, `.env`) | |
| SC-38 | PASS | `server/.sqlx/query-example.json` → **`rc=1`(무시되지 않음)**. `Generated/*.cs`와 `*.cs.meta`도 `rc=1` | 다음 슬라이스 오프라인 빌드 근거 확보 |

**SC-32 — 3자 필드 비교표 (스키마 ↔ Rust ↔ C#)**

`PING_SERVER`

| 계약 필드 | 스키마 | Rust (`commands.rs`) | C# (`PingServerCommand.cs`) | 이름 | 필수 | 널 가능 |
|----------|--------|---------------------|---------------------------|------|------|--------|
| `command_id` | UuidV7, required | `UuidV7` | `Guid` / `Required.Always` | ✅ | ✅ | ✅ |
| `command_type` | const `PING_SERVER` | `PingServerType` (`rename = "PING_SERVER"`) | `string` + `CommandTypeConst` / Always | ✅ | ✅ | ✅ |
| `schema_version` | const 1 (1‥2147483647) | `ConstSchemaVersion<1>` | `int` + `SchemaVersionConst` / Always | ✅ | ✅ | ✅ |
| `client_sent_at` | RealTime \| null, **required 키** | `Option<RealTime>` + `deserialize_with = "required_nullable"` | `string` / **`Required.AllowNull`** | ✅ | ✅ | ✅ |
| `payload.probe_seq` | int 0‥4294967295 | `ProbeSeq(u32)` | `uint` / Always | ✅ | ✅ | ✅ |

`PING_REPLY`

| 계약 필드 | 스키마 | Rust (`messages.rs`) | C# (`PingReplyMessage.cs`) | 이름 | 필수 | 널 가능 |
|----------|--------|---------------------|---------------------------|------|------|--------|
| `message_id` | UuidV7, required | `UuidV7` | `Guid` / Always | ✅ | ✅ | ✅ |
| `message_type` | const `PING_REPLY` | `PingReplyType` | `string` + `MessageTypeConst` / Always | ✅ | ✅ | ✅ |
| `schema_version` | const 1 | `ConstSchemaVersion<1>` | `int` / Always | ✅ | ✅ | ✅ |
| `tick` | int 0‥9007199254740991 | `Tick(u64)` + 상한 검증 | `long` / Always | ✅ | ✅ | ✅ |
| `correlation_id` | UuidV7 \| null, **required 키** | `Option<UuidV7>` + `required_nullable` | `Guid?` / **`Required.AllowNull`** | ✅ | ✅ | ✅ |
| `payload.command_id` | UuidV7, required | `UuidV7` | `Guid` / Always | ✅ | ✅ | ✅ |
| `payload.probe_seq` | int 0‥4294967295 | `ProbeSeq(u32)` | `uint` / Always | ✅ | ✅ | ✅ |

**불일치 0건.** 두 널 가능 필드의 "키는 항상 있고 값만 null"(I-5)은 문서가 아니라 **실제 와이어 출력**으로 확인했다.
- Rust: `deny_unknown_fields` + `required_nullable`로 키 누락을 거부(SC-18의 28건 변이에 포함) — `skip_serializing_if` 미사용.
- C#: 재직렬화 출력이 `{"client_sent_at":null,…}` / `{"correlation_id":null,…}`(테스트 `system-out` 원문).

**SC-33 — 정수 범위 손실 없음**

| 필드 | 스키마 범위 | Rust | C# | 손실 여부 | 범위 밖 거부 |
|------|------------|------|-----|----------|-------------|
| `probe_seq` | 0 ‥ 4294967295 | `ProbeSeq(u32)` (정확히 일치) | `uint` (정확히 일치) | 없음 | 스키마 ✅ / Rust ✅ / **C# ✅** |
| `tick` | 0 ‥ 9007199254740991 | `Tick(u64)` + 상한 검증 newtype | `long` (최대 9223372036854775807 ⊇ 범위) | 없음 | 스키마 ✅ / Rust ✅ / C# ❌(기록된 비대칭) |
| `schema_version` | 1 ‥ 2147483647 | `ConstSchemaVersion<1>`(`u32` 기반) | `int` (상한 정확히 일치) | 없음 | 스키마 ✅ / Rust ✅(const 검증) / C# — |

AC-10이 요구하는 "범위 밖 값에 대해 **최소한 스키마 검증이 거부**"는 세 필드 모두 충족한다(§4 B의 독립 검증에서 `probe-seq-negative`, `probe-seq-above-u32`, `tick-above-safe-integer` 모두 거부).

**SC-34 — 층별 거부 매트릭스 (스펙 §5 정본과 대조)**

| # | fixture | ① 스키마 (SC-14) | ① QA 독립 검증기 | ② Rust serde (SC-15) | ③ C# Strict (SC-24/25) | 스펙 §5와 |
|---|---------|-----------------|-----------------|---------------------|----------------------|----------|
| 1 | `actor-field-injected` | 거부 | 거부 (`Unevaluated properties … 'player_id'`) | 거부 (`unknown field 'player_id'`) | 거부 | 일치 |
| 2 | `command-id-not-v7` | 거부 | 거부 (패턴 불일치) | 거부 (`버전: 4`) | **수락**(기록) | 일치 |
| 3 | `probe-seq-negative` | 거부 | 거부 (`-1 is less than the minimum of 0`) | 거부 (`expected u32`) | 거부 | 일치 |
| 4 | `probe-seq-above-u32` | 거부 | 거부 (`greater than the maximum of 4294967295`) | 거부 (`expected u32`) | 거부 | 일치 |
| 5 | `missing-tick` | 거부 | 거부 (`'tick' is a required property`) | 거부 (`missing field 'tick'`) | 거부 | 일치 |
| 6 | `payload-unknown-field` | 거부 | 거부 (`Additional properties … 'client_sent_at'`) | 거부 (`unknown field 'client_sent_at'`) | 거부 | 일치 |
| 7 | `tick-above-safe-integer` | 거부 | 거부 (`greater than the maximum of 9007199254740991`) | 거부 (`Tick 는 0 ..= …`) | **수락**(기록) | 일치 |

`#1`이 `unevaluatedProperties`로만 막힌다는 M-5의 주장을 **제3자 검증기가 독립 재현**했다(오류 메시지가 `Unevaluated properties are not allowed`). 계약 작성 스타일의 전제가 서 있다.

## 2. 수정 요청

**없다.** FAIL 0건.

## 3. 경계면 점검 결과 (integration-qa §3)

| 경계면 | 결과 | 메모 |
|-------|------|------|
| 계약 ↔ Rust | **일치** | 필드 12개, 필수·널 가능·정수 범위 전부 대조(SC-32/33). `deny_unknown_fields` 적용, `flatten`·내부 태그 0건 |
| 계약 ↔ C# | **일치** | 같은 12개 필드 + `[JsonProperty("snake_case", Required=…)]` 명시. 이름 변환 규칙 의존 없음. QA가 C# 재직렬화 출력을 fixture 원본과 직접 비교(4/4 동일)하고 스키마 검증도 통과시킴 |
| Rust ↔ C# (양쪽 소비자) | **일치** | 같은 fixture 4건이 양쪽에서 왕복 동일. 널 키 유지·`Guid` 소문자 하이픈 표기 양쪽 동일 |
| REST | **해당 있음(부분)** | 서버 표면은 `/healthz`·`/readyz` 2개(`lib.rs:33-34`)가 전부. **클라이언트 소비자 없음** — `client/Assets/_Project`에 `UnityWebRequest`·`HttpClient`·`WebSocket` 참조 0건(스펙 §3 제외 범위대로). 두 경로는 계약이 아니며 `contracts/api/` 승격 대상으로 남아 있다 |
| WebSocket | 해당 없음 | p0-02 범위 |
| 명령 (클라 생성 ↔ 서버 역직렬화) | **부분** | 전송 경로가 없다. 다만 `command_id` 생성기(C# UUIDv7)와 서버의 `UuidV7` 수용 규칙이 같은 패턴(계약 `primitives.schema.json`)을 참조함을 확인. 클라이언트 명령 envelope에 행위자 필드가 없고 주입 시 **스키마·serde·C# 3층 모두** 거부(#1) |
| 도메인 → 역사 | 해당 없음 | 도메인 이벤트·역사 엔진 없음(스펙 §6) |
| 역사 → 프로젝션 → UI | 해당 없음 | 동일 |
| DB ↔ 코드 | 해당 없음 | 마이그레이션·테이블·쿼리 없음. `/readyz`는 `SELECT 1` 수준의 연결성만 확인 |
| 즉시 응답 ↔ 최종 결과 | 해당 없음 | `COMMAND_RESULT` 없음 |
| 상태 머신 | 해당 없음 | 게임 상태 없음 |

## 4. 게임 특화 위험 (integration-qa §4)

| 위험 | 결과 | 메모 |
|------|------|------|
| 서버 판정 우회 | **점검함 — 문제 없음** | 명령 envelope에 `player_id`/`actor_id`/`user_id` 없음(스키마·Rust·C# 전부 grep 0건). 주입 반례가 3층 중 3층에서 거부. 서버가 클라이언트 값을 규칙에 쓰는 코드 없음(`client_sent_at`은 참고용이며 사용처 없음) |
| 화폐·아이템 복사 | 해당 없음 | 경제·인벤토리·DB 쓰기 경로 없음. `MoneyMinor`는 계약 정의만 있고 Rust 타입도 아직 없다 |
| 멱등성 | 해당 없음(설계 확보) | 명령 처리 경로 없음. `command_id`가 UUIDv7 멱등 키로 계약에 고정되어 있고 서버가 "타임스탬프를 해석하지 않는다"고 문서화(`commands.rs`) |
| 결정성 | **점검함 — 위험 없음** | `sim`·`history` 크레이트 미생성. 현재 코드의 `SystemTime`/`Instant::now`/`thread_rng` 사용은 `readiness.rs:285` 1건뿐이며 `#[cfg(test)]`(:211) 안이다. 시간 타입을 계약에 넣지 않고 문자열 newtype으로 둔 것도 확인 |
| 기록 불변 | 해당 없음 | 역사·이벤트 테이블 없음 |
| Fact/Claim 혼합 | 해당 없음 | Claim·Interpretation 경로 없음 |
| 가시성 누출 | **점검함 — 문제 없음** | 비공개 사건 개념이 없다. 대신 **운영 정보 누출**을 점검: `/readyz` 본문에 DSN·비밀번호·드라이버 에러 0건(200·503 양쪽), `CheckStatus`가 열거형이라 문자열이 들어갈 자리가 없음. `Probes`의 `Debug`도 손으로 구현해 접속 문자열 유출을 막음 |
| LLM 사실 생성 | 해당 없음 | LLM 호출 경로 없음 |
| 클라이언트 확정 계산 | **점검함 — 문제 없음** | 클라이언트에 상태·잔액 개념 없음. 수신 경로(`Runtime` 프로필)는 의도적으로 `NotImplementedException`으로 비워 둠 |

## 5. 계약 외 발견

스프린트 계약에 없어 **판정에 넣지 않았다.** 리더가 다음 작업으로 판단할 항목이다.

1. **`/readyz`의 첫 호출이 컨테이너 재생성 직후 1회 503을 낸다 (redis 쪽).** 2회 재현(서버 로그 `12:43:06`, `12:44:26`: `WARN readiness: Redis 점검 실패 error=broken pipe`). 같은 상황에서 **postgres 쪽은 503을 내지 않았다** — sqlx 풀이 `ping on idle connection returned error`(INFO)를 내부에서 흡수하고 새 연결로 재시도하기 때문이다. 즉 두 점검의 "끊어진 연결" 처리 정책이 다르다. 영향: 다음 슬라이스의 스모크 스크립트·CI가 `up -d` 직후 `/readyz`를 **한 번만** 호출하면 흔들린다. 조치 제안: (a) QA 소유 `tests/e2e/` 스모크는 폴링으로 짜고(내가 처리), (b) server가 redis 점검에 1회 재시도를 넣을지는 p0-02에서 결정. 지금 고칠 사안은 아니다 — 담당: server(판단), qa(스크립트).
2. **`dev/null/` 디렉토리에 Git LFS 훅 4개가 잘못 설치돼 있다.** `dev/null/{post-checkout,post-commit,post-merge,pre-push}`(21:15 생성). `core.hooksPath`는 설정돼 있지 않고 `.git/hooks`에는 샘플뿐이라, **LFS `pre-push` 훅이 실제로는 설치되지 않은 상태**다(`filter.lfs.clean` 필터는 정상 설정됨). 영향: (a) 이 디렉토리는 무시 대상이 아니라 그대로 커밋 후보로 보인다(`git check-ignore` rc=1), (b) 바이너리 에셋이 들어온 뒤 push하면 LFS 객체가 원격에 올라가지 않을 수 있다. 담당: architect(`.gitignore`·LFS 정책 소유). Windows 셸에서 `> /dev/null`이 `dev/null` 파일로 해석된 결과로 보인다.
3. **`.env`가 존재하지 않는다.** AC-2의 Given은 "`.env`가 있고"이지만, `docker-compose.yml`의 `${VAR:-기본값}`과 `config.rs`의 기본값이 `.env.example`과 같은 값이라 **없이도 전 항목이 통과**했다. 판정에는 영향이 없으나(관찰 결과가 기준), 스펙의 Given과 실제 실행 조건이 다르다는 점은 기록해 둔다. 담당: 없음(정보).
4. **`LICENSE`가 워킹트리에서 수정 상태(` M LICENSE`)다.** AC-11(a)는 "새 커밋 0건"이므로 위반이 아니다(계약 SC-35 해석 노트대로). 다만 커밋 시점에 독점 라이선스 교체가 함께 들어가야 한다는 점을 다음 슬라이스가 알아야 한다.
5. **server가 ADR-0003 §3.1 문구 보완을 요청했다**(`redis` feature에 `connection-manager` 추가). QA도 `Cargo.toml`에서 두 feature 사용을 확인했다. 담당: architect(ADR 문구).
6. **`client/.vscode/`가 무시 대상이 아니다**(client가 §8-3에서 보고). QA 재확인 결과 `git status`에는 `client/` 전체가 untracked로 뭉쳐 보이므로 개별 확인이 필요하다. AC-11의 열거 목록에 없어 판정 대상이 아니다. 담당: architect.

## 6. 기록 항목 (M-1 ~ M-10, 판정 아님)

QA가 이번 라운드에서 재측정한 값은 ★ 표시.

| # | 항목 | 값 |
|---|------|-----|
| M-1 | 서버 클린 빌드 | T1 18초 / T2 18초 / T3 26초(201 크레이트). `/readyz`(sqlx+redis)의 실제 비용 **+8초** |
| M-2 | 툴체인 1.98.1 첫 다운로드 | 60초 |
| M-3 | 첫 이미지 pull | 12초 |
| M-4 | PG18 | `PGDATA=/var/lib/postgresql/18/docker`, `VOLUME=/var/lib/postgresql` ★ 마운트 실측 재확인 |
| M-5 | `unevaluatedProperties` | 재현됨 ★ QA 독립 검증기로도 재현(`Unevaluated properties are not allowed ('player_id')`) |
| M-6 | 라이브러리 핀 | `sqlx 0.8.6`(0.9.0 아님), `redis 1.7.0` + `connection-manager` |
| M-7 | Unity EditMode | 콜드 63초 / 웜 11초 → ★ QA 재측정 **콜드 1m04.9s / 웜 8.9s** |
| M-8 | Unity 식별자·Hub | `DefaultCompany`/`client` → `Starfall`/`Starfall Dynasty`/`com.starfall.dynasty`. Hub 등록됨 |
| M-9 | 패키지 제거 영향 | 없음. URP 17.6.0 + Input System 1.20.0이 정본 |
| M-10 | C# 미감지 2건 | ★ 이번 라운드에서 관찰·기록 완료(§1 F의 SC-25 표) |

## 7. 이전 라운드 대비

**첫 라운드다.** 회귀 비교 대상 없음.

## 8. 환경 상태 (평가 후)

- `docker compose`: postgres·redis **healthy 유지**(평가 전과 동일). 전역 prune·`down -v` 미사용. 타 프로젝트 컨테이너 5개 `Up 26 hours` 불변.
- QA가 DB에 남긴 것: `_boot_marker_qa_r1` 테이블 1개(SC-10 증거). 지우지 않았다 — 재검증 시 그대로 쓸 수 있다.
- `cargo run -p starfall-game-server`(PID 28824)는 QA가 평가용으로 띄웠다. 평가 종료와 함께 정리한다.
- `client/Library/`는 SC-29 때문에 삭제 후 재생성됐다(콜드 임포트 상태에서 테스트 통과 확인 완료).
- `client/Assets/_Project/Scripts/Contracts/Generated/`는 SC-19에서 생성기를 2회 돌렸으나 **해시가 변하지 않았다**(`unchanged` 3건). 변조 실험은 전부 스크래치 사본에서 했고 `client/` 실물은 그대로다.

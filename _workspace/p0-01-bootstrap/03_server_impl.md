# server 구현 요약 (p0-01-bootstrap)

- 구현: server (rust-server-engineer), 2026-09-18
- 태스크: T1(워크스페이스·툴체인·`/healthz`), T2(계약 크레이트·테스트 9종), T3(docker compose·`.env.example`·`/readyz`), T4(이 문서)
- 스프린트 계약 확인서: `02_server_ack.md` (차단급 이의 없음, 증거 방법 이의 2건)
- **상태: SC-01 ~ SC-18 전부 실행으로 통과.** 미검증(환경)으로 남긴 항목 없음.

## 1. 만든 것

```
rust-toolchain.toml                          # 레포 루트 (ADR-0001 §4 — server/ 와 tools/bots/ 가 상속)
docker-compose.yml                           # PostgreSQL 18 + Redis, 127.0.0.1 바인딩
.env.example                                 # dev 전용 값 (실제 .env 는 커밋 안 함)
server/
├── Cargo.toml                               # [workspace] + [workspace.dependencies] + [workspace.lints]
├── Cargo.lock                               # 추적 대상 (2227줄)
├── clippy.toml                              # 테스트에서만 unwrap/expect/panic 허용
├── crates/
│   ├── contracts/                           # starfall-contracts
│   │   ├── src/{lib,primitives,commands,messages,registry,dispatch}.rs
│   │   └── tests/contract_tests.rs          # 계약 테스트 9종
│   └── gateway/                             # starfall-gateway
│       └── src/{lib,health,readiness,state}.rs
└── bins/game-server/                        # starfall-game-server
    └── src/{main,config}.rs
```

**만들지 않은 것:** `domain`, `sim`, `persistence`, `history` 크레이트 (ADR-0001 §2 변경 2). 이름·위치는 ADR에 고정돼 있고 처음 쓰는 슬라이스에서 만든다.

**`starfall-gateway` 에 게임 규칙·월드 상태 없음** (ADR-0001 §2). 현재 표면은 운영 엔드포인트 2개뿐이고, 그 이유를 `crates/gateway/src/lib.rs` 모듈 문서에 남겨 다음 슬라이스가 여기에 게임 로직을 넣지 않게 했다.

## 2. 실행 방법 (명령 그대로)

```bash
# 모든 세션에서 한 번
export PATH="$HOME/.cargo/bin:$HOME/.dotnet/tools:/c/Users/CHOISOOYEON/AppData/Local/Unity/bin:$PATH"

# --- 인프라 (레포 루트에서) ---
cd /c/WorkSpace/SpaceHistoric
cp .env.example .env          # 선택: compose 에 같은 기본값이 들어 있어 없어도 뜬다
docker compose up -d
docker compose ps             # postgres, redis 둘 다 (healthy)

# --- 서버 (server/ 에서 — 레포 루트에 Cargo.toml 이 없다) ---
cd /c/WorkSpace/SpaceHistoric/server
set -a; source ../.env; set +a        # 또는 직접 export
cargo run -p starfall-game-server     # 로그에 바인딩 주소가 남는다

# --- 검증 게이트 (server/ 에서) ---
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --locked
cargo test -p starfall-contracts --locked -- --nocapture   # 개수 증거가 보인다

# --- 엔드포인트 (PowerShell 의 curl 은 Invoke-WebRequest 별칭이므로 curl.exe) ---
curl.exe -s -w "\nHTTP %{http_code}\n" http://127.0.0.1:8080/healthz
curl.exe -s -w "\nHTTP %{http_code}\n" http://127.0.0.1:8080/readyz
```

## 3. SC ID → 테스트 이름 / 명령 대응표

모든 cargo 명령의 작업 디렉토리는 `C:\WorkSpace\SpaceHistoric\server` 다.

| SC | 검증 방법 | 결과 |
|----|----------|------|
| SC-01 | `cargo fmt --all --check` | exit 0, 출력 없음 |
| SC-02 | `cargo clippy --workspace --all-targets -- -D warnings` | exit 0, warning 0 |
| SC-03 | `cargo test --workspace --locked` | exit 0, **32 passed / 0 failed** (1 doctest ignored) |
| SC-04 | `curl.exe …/healthz` + 기동 로그 | 200, `{"status":"ok","version":"0.1.0"}`, 로그에 `addr=127.0.0.1:8080` |
| SC-05 | `curl.exe …/readyz` (인프라 정상) | 200, `{"status":"ready","checks":{"postgres":"ok","redis":"ok"}}` |
| SC-06 | `docker compose stop postgres` → `/readyz` | 503, `postgres:"unavailable"`, `/healthz` 200, PID 동일 |
| SC-07 | `docker compose start postgres` → `/readyz` | 1초 만에 200 복귀, PID 동일(재시작 없음) |
| SC-08 | 200·503 두 상태의 본문 검사 | 닫힌 집합 만족, 누출 0건 |
| SC-09 | `docker compose up -d; docker compose ps` | 7초 만에 둘 다 `healthy` |
| SC-10 | 마커 테이블 → `down`(`-v` 없이) → `up -d` → 조회 | `_boot_marker` 잔존 |
| SC-11 | 볼륨·컨테이너 before/after diff | 추가된 볼륨은 `starfall_postgres-data` 하나뿐, 타 프로젝트 5개 불변 |
| **SC-12** | `schemas_valid_offline` | 스키마 **7건** 메타스키마 유효 + `$ref` 전부 해석 (오프라인) |
| **SC-13** | `fixtures_roundtrip` | 유효 fixture **4건** 왕복 일치 + 재검증 통과 |
| **SC-14** | `invalid_rejected_by_schema` | 반례 **7건** 전부 스키마 거부 [층①] |
| **SC-15** | `invalid_serde_matrix` | 반례 **7건** serde 결과가 §0.4 표와 일치 [층②] |
| **SC-16** | `registry_server_types_mapped` | `server` 태그 **2건** 전부 대응표에 존재 |
| **SC-17** | `registry_consistency` + `schema_ids_match_paths` + `registry_file_validates_against_schema` | 레지스트리 2항목·fixture 4건 대조, `$id` 7건 일치, `types.json` 자체 검증 통과 |
| **SC-18** | `required_field_mutations` | `required` 변이 **28건** 전부 역직렬화 실패 |
| (보강) | `integer_bounds_rejected` | ADR-0002 §3 테스트 9. tick 2건 + probe_seq 3건 경계 확인 |

`cargo test -p starfall-contracts --locked -- --nocapture` 를 돌리면 각 테스트가 순회 개수와 파일명을 출력한다(§0.3 "빈 순회 방지" 요구).

### SC-02 보조 증거 — `[lints] workspace = true` 옵트인

세 멤버 전부 옵트인했다. 빠뜨리면 린트가 꺼진 채 clippy 가 통과하므로 별도 확인했다.

```
OK  crates/contracts/Cargo.toml
OK  crates/gateway/Cargo.toml
OK  bins/game-server/Cargo.toml
```

**clippy 조정 사실 (T1 지시).** `[workspace.lints.clippy]` 에 `unwrap_used`·`expect_used`·`panic` 을 켰다. 이대로면 테스트 코드도 걸리므로 두 가지로 예외를 뒀고, 운영 코드에는 영향이 없다.

1. `server/clippy.toml` 에 `allow-unwrap-in-tests` / `allow-expect-in-tests` / `allow-panic-in-tests` = true — `#[test]` 함수와 `#[cfg(test)]` 안에서만 적용된다.
2. `crates/contracts/tests/contract_tests.rs` 상단에 `#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]` — 통합 테스트의 헬퍼 함수(`#[test]` 가 아닌 것)까지 덮기 위해서다.

### SC-15 보조 증거 — `flatten` / 내부 태그 미사용

```
$ grep -rn --include=*.rs -E '^\s*#\[serde\((flatten|tag)' crates bins
  0건 — 속성으로서의 flatten/내부태그 사용 없음
```

**QA 참고:** 계약서의 `grep -rn "serde(flatten)\|serde(tag *="` 를 그대로 쓰면 **4줄이 잡힌다.** 전부 `//!` 문서 주석(왜 쓰면 안 되는지 설명하는 문장)이고 실제 속성이 아니다. 위처럼 행 시작 `#[serde(` 에 앵커를 걸면 0건으로 나온다.

## 4. 계약 대응표

| 레지스트리 이름 | kind | 스키마 | Rust 타입 |
|----------------|------|--------|----------|
| `PING_SERVER` | command | `commands/PING_SERVER.schema.json` | `starfall_contracts::commands::PingServerCommand` |
| `PING_REPLY` | server_message | `messages/PING_REPLY.schema.json` | `starfall_contracts::messages::PingReplyMessage` |

대응표의 정본은 `server/crates/contracts/src/registry.rs` 의 `CONTRACT_TYPES` 다. 새 타입은 여기 한 줄만 추가하면 테스트 9종이 자동으로 그 타입을 덮는다(왕복·반례 거부·`required` 변이·정수 상한).

타입 이름은 **리터럴 상수**로 뒀다(`pub const PING_SERVER: &str = "PING_SERVER";`). QA 커버리지 스크립트가 코드에서 문자열을 찾기 때문이다.

### 직렬화 규칙 준수 (ADR-0002 §3의 4가지 조건)

| 조건 | 구현 |
|------|------|
| `serde_json::Value` 비교 | `contract_tests.rs` 의 `fixtures_roundtrip` 이 `assert_eq!(Value, Value)` |
| `skip_serializing_if` 금지 | 널 가능 필드 2개(`client_sent_at`, `correlation_id`)에 **아무 직렬화 속성도 없다** |
| 시간은 문자열 newtype | `RealTime`·`GameTime` — `chrono`/`time` 의존 없음. `.1Z` 가 `.100Z` 로 정규화되지 않는지 단위 테스트가 고정 |
| 정수는 정확한 폭 + 범위 newtype | `ProbeSeq(u32)`, `Tick(u64)`+상한 검증, `ConstSchemaVersion<1>` |
| `flatten`/내부 태그 금지 | 타입마다 envelope 5~6개 필드를 펼쳐 쓰고 `deny_unknown_fields`. 디스패치는 `dispatch.rs` 의 peek 구조체 2개로 2단계 처리 |

**"키는 있고 값만 null" 을 강제한 방법.** serde 는 `Option<T>` 필드가 입력에 없으면 조용히 `None` 을 채운다. 그대로 두면 `client_sent_at` 키를 통째로 뺀 문서가 통과해 I-5 가 깨진다. `#[serde(deserialize_with = "required_nullable")]` 을 달면 serde 가 그 지름길을 쓰지 못하고 `missing field` 로 실패한다. **이 설계가 맞는지는 추측하지 않고 SC-18 이 증명했다** — 28건 변이 중 `client_sent_at`·`correlation_id` 제거 4건이 실제로 실패했다.

## 5. 실측값 (M-1 ~ M-6)

| # | 항목 | 값 |
|---|------|-----|
| **M-1** | 서버 클린 빌드 시간 | 아래 3회 |
| M-1a | T1 시점 (axum만, gateway+game-server) — `cargo build --workspace` | **18초** |
| M-1b | T2 시점 (+ contracts/jsonschema) — `cargo build --workspace --all-targets` | **18초** |
| M-1c | T3 시점 (+ sqlx·redis) — `cargo build --workspace --all-targets` | **26초**, 201 크레이트 컴파일 |
| **M-2** | 레포 안 첫 `cargo` 의 툴체인 다운로드 | **60초** (`1.98.1-x86_64-pc-windows-msvc`, 5 components) |
| **M-3** | 첫 `docker compose` 이미지 pull | **12초** (postgres + redis, 21:10:34 → 21:10:46) |
| **M-4** | PG18 `PGDATA` / `Config.Volumes` | `PGDATA=/var/lib/postgresql/18/docker`, `VOLUMES={"/var/lib/postgresql":{}}` |
| **M-5** | `unevaluatedProperties` 재현 | **재현됨** — 아래 |
| **M-6** | sqlx / redis 핀 결정 | `sqlx 0.8.6`, `redis 1.7.0` — 아래 |

### M-1 해석 — `/readyz` 의 실제 비용

**M-1b(18초) ↔ M-1c(26초)가 직접 비교 가능한 쌍이다** (둘 다 `--all-targets`, 차이는 sqlx+redis 뿐). 즉 `/readyz` 때문에 늘어난 클린 빌드 시간은 **약 +8초**다. M-1a 는 `--all-targets` 없이 쟀으므로 M-1b 와 명령이 다르다 — 둘을 직접 빼지 말 것.

내가 ADR 검토에서 "`/readyz` 는 과하지 않다"고 주장한 근거를 숫자로 남기면: 클린 빌드 +8초를 내고, 그 대가로 **호스트 포트 15432·PG18 볼륨 경로·`.env` 배선이 서버 프로세스 관점에서 실제로 동작함**을 SC-05 ~ SC-08 로 증명했다. TLS feature 를 켜지 않아 rustls/ring 트리가 통째로 빠진 것이 비용을 낮췄다.

### M-4 — ADR-0003 §3 의 전제가 정확했다

```
$ docker image inspect postgres:18.6-trixie --format '{{range .Config.Env}}{{println .}}{{end}}' | grep -i pgdata
PGDATA=/var/lib/postgresql/18/docker
$ docker image inspect postgres:18.6-trixie --format 'VOLUMES={{json .Config.Volumes}}'
VOLUMES={"/var/lib/postgresql":{}}
```

그래서 명명 볼륨을 `/var/lib/postgresql` 에 마운트했다. 실제 마운트도 확인했다.

```
$ docker inspect starfall-postgres-1 --format '{{range .Mounts}}{{.Type}} {{.Name}} -> {{.Destination}}{{end}}'
volume starfall_postgres-data -> /var/lib/postgresql
```

ADR 갱신 필요 없음 — 기재된 내용과 실측이 일치한다.

### M-5 — `unevaluatedProperties` 는 `allOf` 애너테이션을 제대로 수집한다

T2 의 첫 작업으로 확인했다. 이게 안 됐다면 계약 작성 스타일 전체가 흔들렸을 것이다.

- `actor-field-injected.json`(`player_id` 주입)이 **스키마 검증에서 거부**됐다. `player_id` 는 envelope 에도 타입 스키마에도 없고, envelope 에는 `additionalProperties:false` 가 없다. 따라서 이 반례를 막는 메커니즘은 **`unevaluatedProperties: false` 하나뿐**이며, 그것이 `allOf` 안쪽 `properties` 애너테이션을 수집해야만 성립한다. 거부됐다는 것이 곧 수집이 동작한다는 증거다.
- `jsonschema` 0.56.0, `default-features = false`, 모든 스키마를 `$id` 로 `Registry` 에 사전 등록 + `offline()`.
- ADR-0002 §3 의 설명(`resolve-http`·`resolve-file` 은 opt-in)도 실물 `Cargo.toml` 과 일치했다.

실제 API 는 ADR 에 적힌 형태 그대로였다.

```rust
let registry = jsonschema::Registry::new().add(&id, value)? /* … */ .prepare()?;
let validator = jsonschema::options().with_registry(&registry).offline().build(&json!({"$ref": id}))?;
```

### M-6 — sqlx 0.8.6 으로 핀 (0.9.0 아님), redis 1.7.0

- **sqlx `0.8.6`.** `0.9.0` 이 나와 있지만 갓 나온 메이저 변경이다. ADR-0003 §3.1 이 "파괴적 변경이 크면 0.8.x 로 핀"을 재량으로 위임했고, 부트스트랩 슬라이스가 라이브러리 마이그레이션을 떠안을 이유가 없다고 판단했다. `/readyz` 가 쓰는 표면은 `PgPoolOptions` + `sqlx::query("SELECT 1")` 뿐이라 어느 쪽이든 동작한다. **재평가 시점: 영속화 슬라이스** — 거기서 `query!` 매크로·`#[sqlx::test]`·마이그레이션을 쓰기 시작할 때 0.9 로 갈지 결정하는 것이 맞다.
  - feature: `default-features = false, features = ["runtime-tokio", "postgres"]`. **TLS 없음** — 로컬은 평문이고, rustls/ring 트리를 들이지 않아 M-1c 비용을 낮췄다. 원격 DB 가 생기면 `tls-rustls-ring` 추가 + ADR 갱신.
- **redis `1.7.0`.** feature 는 `["tokio-comp", "connection-manager"]`.
  - **ADR-0003 §3.1 보완 필요(사소):** ADR 은 `tokio-comp` 만 적었는데, `redis::aio::ConnectionManager` 는 **별도 `connection-manager` feature** 뒤에 있다. `tokio-comp` 만으로는 `unresolved import` 로 컴파일이 실패한다(내가 그렇게 실패했다). ADR 문구에 feature 하나를 더해 주면 다음 사람이 같은 데서 막히지 않는다.
  - `ConnectionManager` 를 **기동 시점에 만들지 않는다.** Redis 가 꺼져 있으면 생성 자체가 실패해 지연 연결 제약을 어기기 때문이다. `tokio::sync::OnceCell` 로 첫 성공한 점검까지 미루고, 한 번 만들어진 뒤에는 매니저가 자동 재연결을 맡는다.

## 6. `/readyz` 구현 제약 준수 (ADR-0003 §3.1)

| 제약 | 구현 | 증거 |
|------|------|------|
| 1. 지연 연결 | `PgPoolOptions::connect_lazy`, `redis::Client::open` (둘 다 I/O 없음) | `probes_construct_without_connecting` 테스트, 인프라 없이 기동 성공 |
| 2. 2초 타임아웃·동시 실행 | `tokio::join!` + `tokio::time::timeout(2s)` 각각 | `readyz_reports_unavailable_when_nothing_is_listening` 이 5초 미만을 단언 |
| 3. 본문에 에러·DSN·비밀번호 없음 | `CheckStatus` 가 `Ok`/`Unavailable` **열거형** — 문자열이 들어갈 자리 자체가 없다 | `failure_body_never_carries_driver_detail` + SC-08 실행 검사 |
| 4. `/healthz` 와 용도 분리 | `/healthz` 는 의존성을 보지 않는다 | SC-06 에서 `/readyz` 503 인 동안 `/healthz` 200 |

`Probes` 의 `Debug` 는 손으로 구현했다 — 파생하면 `PgPool`/`redis::Client` 의 접속 문자열이 로그·패닉 메시지로 샐 수 있다.

## 7. 알려진 한계

1. **`Probes::new` 는 Tokio 런타임 컨텍스트 안에서 불러야 한다.** `sqlx` 의 지연 풀이 유휴 연결 정리 태스크를 즉시 spawn 하기 때문이다. 런타임 밖에서 부르면 `this functionality requires a Tokio context` 로 패닉한다. "연결을 열지 않는 것"과 "런타임이 필요 없는 것"은 다르다 — 함수 문서의 `# Panics` 절에 적어 뒀다. 실제 기동 경로(`runtime.block_on(serve(config))`)는 런타임 안이라 문제없다.
2. **`.env` 를 서버가 직접 읽지 않는다.** dotenv 크레이트를 들이지 않았다(설정 출처가 두 곳이 되면 "왜 이 값이 쓰였나"를 추적하기 어렵다). 셸에서 `set -a; source ../.env; set +a` 하거나 직접 export 한다. `.env` 없이도 `config.rs` 의 기본값이 `.env.example` 과 같은 값이라 로컬에서는 그대로 동작한다.
3. **`/readyz` 는 연결성만 본다.** 스키마·마이그레이션 상태를 보지 않는다. 이번 슬라이스에 테이블이 없기 때문이다. 영속화 슬라이스에서 "마이그레이션이 최신인가"를 추가할지 정해야 한다.
4. **`Sequence`·`GameTime`·`MoneyMinor` 는 타입만 있고 쓰이는 곳이 없다.** 현재 두 계약 타입이 쓰지 않는다. 이벤트 envelope(`event-envelope.schema.json`)이 스키마로만 존재하고 이번 슬라이스에 해당 타입이 없기 때문이다 — 스펙 §5가 의도한 상태다. `MoneyMinor` 는 아직 Rust 타입을 만들지 않았다(쓰는 계약이 생길 때 추가).
5. **`docker compose up -d` 직후 바로 `psql` 을 부르면 실패할 수 있다.** 초기화 중이기 때문이다. 스모크 스크립트에는 `pg_isready` 대기 루프를 넣어야 간헐 실패가 없다(`02_server_ack.md` 쟁점 4 참고).
6. **HTTP 요청 로깅·메트릭이 없다.** `tower-http`·OpenTelemetry 를 이번에 넣지 않았다(스펙 범위 밖). tick 루프·명령 경로가 생기는 슬라이스에서 `correlation_id`·`command_id`·`tick` span 과 함께 넣는 것이 맞다.
7. **에디션 2024 를 썼다.** ADR에 명시된 사항은 아니다. rustc 1.98.1 에서 안정이고 현재 코드에 문제가 없었다. 문제가 되면 `edition = "2021"` 로 내리면 된다.

## 8. architect 에게 보낼 것 (계약 변경 요청 아님)

- **계약(`contracts/**`) 변경 요청 없음.** 확정된 7개 스키마·레지스트리·fixture 11건 그대로 구현했고, 스펙 §5 의 "Rust serde" 열과 구현 결과가 **모든 칸에서 일치**했다(`02_server_ack.md` §1).
- **ADR-0003 §3.1 문구 보완 1건(사소):** `redis` feature 에 `connection-manager` 를 추가해 주기 바란다. 현재 문구(`tokio-comp` 만)로는 `redis::aio::ConnectionManager` 가 컴파일되지 않는다. 결정 자체(redis 1.7.0 + ConnectionManager)는 그대로 유효하다.

## 9. client 가 알아야 할 것

이번 슬라이스에 WebSocket·명령 왕복은 없다(p0-02). 지금 쓸 수 있는 것은 운영 엔드포인트 2개다.

| 메서드 | 경로 | 성공 | 실패 |
|--------|------|------|------|
| GET | `http://127.0.0.1:8080/healthz` | 200 `{"status":"ok","version":"0.1.0"}` | — (의존성을 보지 않는다) |
| GET | `http://127.0.0.1:8080/readyz` | 200 `{"status":"ready","checks":{"postgres":"ok","redis":"ok"}}` | 503, 같은 형태에 값이 `"unavailable"` |

- 기본 바인딩은 `127.0.0.1:8080` (`STARFALL_HTTP_ADDR` 로 변경). PostgreSQL 15432, Redis 16379.
- `checks.*` 의 값은 **`"ok"` 또는 `"unavailable"` 뿐이다.** 다른 문자열이 오면 서버 버그다.
- 이 두 경로는 계약(`contracts/`)이 아니다. Unity 가 소비하게 되면 `contracts/api/` 로 승격한다(스펙 §5).

Rust 쪽 계약 타입의 필드·필수·널 가능·정수 매핑은 `02_server_ack.md` §3 표에 정리돼 있다. C# DTO 와 나란히 볼 때 그 표를 쓰면 된다. 특히 두 가지가 C# 과 대칭이다.

- `client_sent_at`·`correlation_id` 는 **키가 항상 있고 값만 null** 이다. Rust 는 `skip_serializing_if` 를 쓰지 않았고, C# 은 `Required.AllowNull` 이어야 같은 모양이 된다.
- `tick` 상한(2^53−1) 위반은 **Rust 는 거부하지만 C# `long` 은 거부하지 못한다.** 의도된 비대칭이고 서버 경계에서 막힌다.

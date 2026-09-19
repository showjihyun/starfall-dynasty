# server 구현 요약 (p0-02-networking-spike)

- 작성: server, 2026-09-19
- 대상 태스크: T1~T6 (`01_architect_tasks.md`)
- 스펙: `docs/specs/p0-02-networking-spike.md` (agreed, AC-1~21). 계약: `02_sprint_contract.md` SC-01~37
- 착수 전 확인: `02_server_ack.md`
- **이 문서가 client·QA 봇의 접속 정본이다.** §2(엔드포인트·토큰)와 §5(메트릭)가 그 부분이다.

## 0. 게이트 3종 결과

`cd C:\WorkSpace\SpaceHistoric\server` 에서 실행 (2026-09-19).

| 게이트 | 명령 | 결과 |
|--------|------|------|
| SC-01 | `cargo fmt --all --check` | **종료 코드 0**, 출력 없음 |
| SC-02 | `cargo clippy --workspace --all-targets -- -D warnings` | **종료 코드 0**, warning 0 |
| SC-03 | `cargo test --workspace --locked` | **종료 코드 0**, **82건 통과 / 0 실패** (+ `live_smoke` 2건은 `#[ignore]`) |

테스트 내역 (82건):

| 타깃 | 건수 |
|------|-----:|
| `starfall-contracts` 유닛 | 19 |
| `starfall-contracts` `tests/contract_tests.rs` | 10 |
| `starfall-gateway` 유닛 | 28 |
| `starfall-gateway` `tests/ws_integration.rs` (실제 WebSocket) | 14 |
| `starfall-sim` 유닛 | 8 |
| `starfall-persistence` 유닛 | 2 |
| `starfall-game-server` 유닛 | 1 |
| `starfall-gateway` `tests/live_smoke.rs` | (2, `#[ignore]` — 실서버 필요) |

**SC-02 보조 — `[lints] workspace = true` 옵트인**: `crates/sim/Cargo.toml:12`, `crates/persistence/Cargo.toml:12`. 빠뜨리면 린트가 꺼진 채 통과하므로 신규 2크레이트 모두에 넣었다.

**SC-04 — `starfall-sim` 의 IO-free 증거**:

```
grep -nE "axum|sqlx|redis|rand|chrono" crates/sim/Cargo.toml   # 종료 코드 1 (매치 없음) = PASS
```

주의: 금지 크레이트 이름을 `crates/sim/Cargo.toml` **주석에도 쓰지 않았다.** 쓰면 QA 의 grep 이 주석에 걸려 검사가 무의미해진다(처음에 그렇게 썼다가 되돌렸다). 금지 목록과 이유는 `crates/sim/src/lib.rs` 모듈 문서에 표로 있다. `starfall-sim` 의 의존성은 `starfall-contracts` **하나뿐이고 비동기 런타임조차 없다** — 그래서 AC-6(a) 수동 step 테스트가 런타임도 소켓도 없이 돈다.

---

## 1. 무엇을 만들었나

```
server/
├─ Cargo.toml                  워크스페이스 멤버 5, feature 추가(axum ws / sqlx macros·migrate·uuid·json / hmac·sha2·subtle)
├─ migrations/
│  └─ 0001_worlds_and_domain_events.sql     worlds · domain_events · 인덱스 2 · append-only 트리거 · 스파이크 월드 시드
├─ crates/
│  ├─ contracts/   신규 4타입(COMMAND_RESULT·SESSION_READY·SESSION_OPENED·SESSION_CLOSED) + GameCalendar + TickHz·ServerVersion
│  ├─ sim/         Simulation::step (상태가 바뀌는 유일한 자리), 세션 모델, 중복 제거, 이벤트·메시지 생성   ← 의존성 1
│  ├─ persistence/ 마이그레이션 embed, 월드 대조, tick 재개, tick 단위 트랜잭션, 재시도
│  └─ gateway/     /ws(인증·프레이밍·수명주기), tick 드라이버(전용 OS 스레드), /debug/stats, /readyz 재시도
└─ bins/game-server/   조립 + 종료 순서 + stdin shutdown
```

의존 방향: `contracts <- sim <- gateway`, `contracts + sim <- persistence`, 그리고 **`gateway` 는 `persistence` 를 의존하지 않는다.** tick 결과는 채널로 나가고 바이너리가 그 양 끝을 붙인다. 관측 값은 `Arc<AtomicU64>` 3개만 공유한다(크레이트 의존이 아니다).

### 데이터 흐름

```
WS 수신 태스크 ──parse/peek──> SubmitHandle ──[제어: unbounded / 명령: bounded 4096]──> tick 스레드(전용 OS 스레드, 50ms)
                                                                                          │ Simulation::step
                                                                                          ├─> TickOutcome.outbound ─> 세션별 송신 큐(256) ─> WS 송신 태스크
                                                                                          └─> PersistBatch ─[bounded 512]─> 영속화 태스크 ─> PostgreSQL(1 tick = 1 트랜잭션)
```

### 스펙이 요구한 구조 결정을 어떻게 지켰나

| 요구 | 구현 |
|------|------|
| I-13 상태는 tick 안에서만 | 게이트웨이가 가진 것은 `SubmitHandle`(채널 + 순번 발급기)뿐. 시뮬레이션 상태 타입이 보이지 않는다 |
| I-16 세션 열기·닫기는 거부 대상 아님 | **제어 경로를 명령 큐와 분리**(unbounded). 명령 큐가 가득 차도 닫기가 사라지지 않는다 |
| ADR-0006 §2.1 전용 OS 스레드 | `std::thread` + `std::thread::sleep`. tokio 타이머를 쓰면 이 PC 에서 50ms 요청이 61ms 가 된다 |
| ADR-0006 §2.2 tick 초과 = 본문 소요 | `crates/gateway/src/runtime.rs` 의 `body_start`(제출 수집 직전) ~ `body`(영속화 제출 직후). **sleep 은 제외**. 루프 주기 드리프트는 별도로 `tick_lag_seconds` |
| ADR-0006 §2.3 tick 재개 | `GREATEST(worlds.last_tick, max(domain_events.tick)) + 1`. 두 출처를 다 보는 이유: QA 탐침처럼 서버 밖에서 행이 들어오면 `max(tick)` 이 더 클 수 있다 |
| ADR-0007 §4 tick 1회 = 트랜잭션 1회 | 이벤트 INSERT + `worlds.last_tick` UPDATE 가 **같은 트랜잭션** |
| I-19 `occurred_at` 은 tick 파생 | `GameCalendar::occurred_at`. `tick_hz` 로 **먼저** 나눈다. `chrono` 없이 정수 달력 산술(Hinnant) |
| I-25 항진명제 금지 | `ws_connections` 는 드라이버 라우팅 표의 **실제 길이**를 매 tick `store`. `tick_skipped_total` 은 만들지 않고 `tick_total == tick − start_tick + 1` 항등식 노출 |

---

## 2. client·QA 가 붙는 지점 (정본)

### 2.1 엔드포인트

기본 바인딩은 **`127.0.0.1:8080`** (`STARFALL_HTTP_ADDR` 로 변경 가능).

| 경로 | 메서드 | 응답 |
|------|--------|------|
| `ws://127.0.0.1:8080/ws` | GET(업그레이드) | 101 / 401 / 503 |
| `http://127.0.0.1:8080/healthz` | GET | `{"status":"ok","version":"0.1.0"}` |
| `http://127.0.0.1:8080/readyz` | GET | 200 `{"status":"ready","checks":{"postgres":"ok","redis":"ok"}}` / 503 `not_ready` |
| `http://127.0.0.1:8080/debug/stats` | GET | §5 의 JSON. **계약이 아니다** |

`/ws` 의 거절 응답 본문은 언제나 `{"status":"unavailable","reason":"<사유>"}` 이고 `reason` 은 다음 5개 중 하나다:

| `reason` | 상태 코드 | 뜻 |
|----------|:---:|------|
| `auth_not_configured` | 503 | `STARFALL_DEV_AUTH_SECRET` 미설정 |
| `recording_backlog` | 503 | 영속화 백로그 > 600 tick (ADR-0007 §4) |
| `shutting_down` | 503 | 종료 중 |
| `no_credential` | 401 | `Authorization: Bearer` 헤더 없음 |
| `invalid_token` | 401 | 서명 불일치 또는 주체가 정규 UUIDv7 이 아님 |

**클라이언트는 이 상태 코드를 볼 수 없다**(client 실측: Mono 의 `WebSocketException` 이 `Success` 로 온다). 그래서 **거절의 증거는 전적으로 서버 쪽**이고, `/debug/stats.upgrade_rejected_total` 이 그 증거다(§5). 서버 로그에도 `WARN ... /ws 업그레이드 거절` 한 줄이 사유와 함께 남는다.

### 2.2 업그레이드 요청 형식

client 가 보내는 형태를 그대로 받는다(추가 헤더 요구 없음):

```
GET /ws HTTP/1.1
Host: 127.0.0.1:8080
Connection: Upgrade
Upgrade: websocket
Sec-WebSocket-Version: 13
Sec-WebSocket-Key: <base64 16바이트>
Authorization: Bearer <subject>.<hmac_hex>
```

`curl.exe` 로 401/503 을 확인할 때도 같은 헤더 4개를 붙인다(붙이지 않으면 axum 이 400 을 준다):

```bash
WSH=(-H "Connection: Upgrade" -H "Upgrade: websocket" -H "Sec-WebSocket-Version: 13" -H "Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==")
curl.exe -s -o NUL -w "no-header=%{http_code}\n" "${WSH[@]}" http://127.0.0.1:8080/ws
```

### 2.3 개발용 토큰 (ADR-0008 §1)

```
token = "<subject>.<hex(HMAC_SHA256(key = STARFALL_DEV_AUTH_SECRET, msg = subject))>"
```

- **서명 대상 메시지는 주체의 UUID 문자열 그대로다** — 정규 소문자 하이픈 표기 36자 ASCII. 바이트 16개가 아니다.
- 키는 비밀 문자열의 **UTF-8 바이트**(`dev_only_not_a_secret` → 21바이트).
- 서명은 소문자 hex **64자**.
- 주체는 **정규 소문자 하이픈 표기 UUIDv7** 이어야 한다. 대문자·중괄호·하이픈 없는 표기·v4 는 전부 401 이다(serde 경로와 같은 규칙이라 "헤더로는 통과하고 계약으로는 거부"가 생기지 않는다).
- 검증은 `subtle::ConstantTimeEq` 로 상수 시간 비교.
- 검증에 성공한 주체가 **그대로 `actor_id`** 가 된다 (I-10). 클라이언트는 `SESSION_READY` 로 통보받는다.

**토큰 만들기 (독립 구현, QA·client 가 그대로 쓸 수 있다)**

```bash
python -c "import hmac,hashlib,sys; s=sys.argv[1]; print(s+'.'+hmac.new(sys.argv[2].encode(), s.encode(), hashlib.sha256).hexdigest())" \
  01a0b1c2-b010-7000-8000-000000000000 dev_only_not_a_secret
```

**봇 30개의 주체 파생 규칙 — server 가 정본으로 정한다**

```
subject(bot-NNN) = 01a0b1c2-b010-7000-8000-000000000NNN      (NNN = 000 .. 029, 10진 3자리)
```

`crates/gateway/src/auth.rs` 의 `bot_subjects_follow_the_documented_rule` 테스트가 30개 전부에 대해 파싱·발급·검증·중복 없음을 강제한다. 기준 값 두 개(실행 출력):

```
bot-000 = 01a0b1c2-b010-7000-8000-000000000000.f0561d67b8c71be38992925b16251a890c508770f48f47a63eb51ebf5a880fc6
bot-029 = 01a0b1c2-b010-7000-8000-000000000029.c93a0e52ad2926a417dff00599b0dfc818bef26c343e24b34e4daad597dc3d71
```

(비밀 `dev_only_not_a_secret` 기준. 비밀이 다르면 서명이 달라진다.)

**주체가 겹치지 않는지** — 같은 테스트가 확인한다:

| 역할 | 주체 |
|------|------|
| 봇 30개 | `01a0b1c2-b010-7000-8000-000000000000` ~ `...029` |
| Unity 클라이언트 | `01a0b1c2-7e57-7c11-8e57-000000000001` (client 소유, `STARFALL_DEV_ACTOR_SUBJECT` 로 변경 가능) |
| server 라이브 스모크 | `01a0b1c2-5a11-7c01-8d01-000000000001`, `...002` |

토큰을 파일에 저장하지 않는다. 비밀에서 즉석 계산한다.

### 2.4 프레이밍 규칙 (ADR-0005 §2)

- **텍스트 프레임만.** 바이너리는 프로토콜 위반으로 계수.
- 한 프레임 = 계약 메시지 1개. 배칭 없음.
- **앱 상한 16 KiB**, 라이브러리 상한 64 KiB. 라이브러리 상한이 더 커야 앱이 위반을 셀 수 있다(같으면 tungstenite 가 먼저 끊어 "10초 8회" 예산이 관측 불가능해진다).
- 서버가 **15초마다 Ping**. Pong·데이터 프레임이 **30초** 없으면 close 1001 + `IDLE_TIMEOUT`.
- 프로토콜 위반 예산: **슬라이딩 10초 창에서 8회까지 허용, 9회째에 close 1002**.
- `SESSION_READY` 가 그 연결의 **첫 계약 메시지**다. 그 전에 아무것도 보내지 않는다.

close code 표 (ADR-0005 §2, `runtime::close_code` 가 정본이고 단위 테스트가 고정):

| `close_reason` | close code |
|----------------|:---:|
| `CLIENT_CLOSED` | 1000 |
| `IDLE_TIMEOUT` | 1001 |
| `PROTOCOL_VIOLATION` | 1002 |
| `SLOW_CONSUMER` | 1011 |
| `SERVER_SHUTDOWN` | 1001 |
| `TRANSPORT_ERROR` | (Close 프레임 없음) |

### 2.5 envelope 채움 — QA 전제 확인

- **서버 메시지 envelope 에는 `sequence` 도 `world_id` 도 `occurred_at` 도 `recorded_at` 도 `actor_id` 도 없다.** `message-envelope.schema.json` 이 그렇게 정의되어 있고 구현도 같다. `sequence` 는 도메인 이벤트 전용이다 (I-18).
- **`COMMAND_RESULT` 와 `PING_REPLY` 의 `correlation_id` 는 언제나 `null`** (스펙 §5.2). 키는 항상 존재한다.
- `SESSION_READY` 의 `correlation_id` 에는 **세션 correlation** 이 들어가고, 이것이 `SESSION_OPENED`·`SESSION_CLOSED` 와 같은 값이다. **봇·client 는 이 값을 수집해 QA 의 correlation 집합에 넣는다**(스프린트 계약 §0.6).
- `causation_id` 는 이 슬라이스에서 항상 `null`.
- `actor_id` 는 `SESSION_*` 에서 비-null(검증된 토큰의 주체).

### 2.6 시드 월드 상수

| 항목 | 값 |
|------|-----|
| `world_id` | `01a0b1c2-3d4e-7f01-8a2b-9c0d1e2f3a4b` |
| `name` | `spike` |
| `tick_hz` | `20` |
| `calendar_epoch` | `3800-01-01T00:00:00Z` |
| `calendar_scale` | `60` |
| `sim_version` | `1` |
| `last_tick` | 최초 `NULL`, 이후 서버가 갱신 |

`occurred_at` 재계산 공식 (QA 의 `check_occurred_at.py` 가 쓸 것):

```
game_seconds = (tick / 20) * 60        # 정수 나눗셈, tick_hz 로 먼저 나눈다
occurred_at  = 3800-01-01T00:00:00Z + game_seconds 초
```

---

## 3. 기동·정지

### 3.1 기동

```bash
cd C:\WorkSpace\SpaceHistoric
docker compose up -d
docker compose exec -T postgres pg_isready -U starfall -d starfall -t 60

cd server
# 환경 변수는 셸에 넣는다 — config.rs 는 .env 파일을 읽지 않는다
export STARFALL_DEV_AUTH_SECRET=dev_only_not_a_secret
cargo run -p starfall-game-server
# 또는 빌드된 exe 직접 실행 (QA 권장 — cargo 프로세스가 끼지 않는다)
cargo build --workspace
./target/debug/starfall-game-server.exe
```

**"이제 붙어도 된다"의 신호 3가지** (client 가 물은 것):

1. 로그 한 줄: `starfall game-server 기동 완료 ? 연결을 받는다 ... ws=ws://127.0.0.1:8080/ws`
2. `GET /healthz` 가 **200**. 이 시점에 마이그레이션·월드 대조·tick 재개가 **이미 끝나 있다**(전부 바인딩 전에 한다).
3. `GET /debug/stats` 의 `accepting_connections` 가 `true`.

폴링 권장 형태:

```powershell
for ($i=0; $i -lt 200; $i++) {
  if ((curl.exe -s -o NUL -w "%{http_code}" http://127.0.0.1:8080/healthz) -eq "200") { break }
  Start-Sleep -Milliseconds 200
}
```

### 3.2 정지 — stdin `shutdown` (SC-27)

Windows 에는 SIGTERM 이 없고, 에이전트가 백그라운드로 띄운 프로세스에 CTRL_C_EVENT 를 보낼 표준 경로도 없다. **하드 킬은 금지다** — graceful shutdown 경로를 전혀 타지 않으면서 outbox 부재의 손실을 재현해 결과가 "graceful shutdown 이 깨졌다"로 보인다.

**동작을 실측으로 확인한 레시피** (2026-09-19):

```powershell
$exe = "C:\WorkSpace\SpaceHistoric\server\target\debug\starfall-game-server.exe"
$psi = [Diagnostics.ProcessStartInfo]::new($exe)
$psi.RedirectStandardInput  = $true
$psi.RedirectStandardOutput = $true      # 반드시 비워 줄 것 (아래 주의)
$psi.RedirectStandardError  = $true
$psi.UseShellExecute  = $false
$psi.WorkingDirectory = "C:\WorkSpace\SpaceHistoric\server"
$psi.Environment["STARFALL_DEV_AUTH_SECRET"] = "dev_only_not_a_secret"
$p = [Diagnostics.Process]::new(); $p.StartInfo = $psi
# stdout/stderr 를 리다이렉트했으면 반드시 비동기로 읽어야 한다. 안 그러면 파이프가 차서
# 서버가 멈추고 WaitForExit 가 영원히 돌아오지 않는다.
$sb = { if ($null -ne $EventArgs.Data) { Add-Content -Path $Event.MessageData -Value $EventArgs.Data } }
Register-ObjectEvent -InputObject $p -EventName OutputDataReceived -Action $sb -MessageData "server.log" | Out-Null
Register-ObjectEvent -InputObject $p -EventName ErrorDataReceived  -Action $sb -MessageData "server.log" | Out-Null
$p.Start() | Out-Null; $p.BeginOutputReadLine(); $p.BeginErrorReadLine()

# ... 검증 ...

$p.StandardInput.WriteLine("shutdown")
$p.StandardInput.Flush()
$p.WaitForExit(60000)
"exit=$($p.ExitCode)"      # 0
```

- 정상 종료의 **종료 코드는 0**. 설정·기동 실패는 **1**.
- **BOM 은 서버가 걷어낸다.** PowerShell 의 `StandardInput` StreamWriter 는 첫 줄 앞에 U+FEFF 를 붙인다(실측). 처음엔 서버가 `?shutdown` 을 "알 수 없는 명령"으로 무시했다. 지금은 `line.trim().trim_matches('\u{feff}').trim()` 으로 관용한다 — QA 쪽에서 인코딩을 맞출 필요가 없다.
- **EOF 는 종료 신호가 아니다.** stdin 이 `NUL` 이거나 리다이렉트가 없으면 stdin 리스너 스레드가 조용히 끝나고 Ctrl-C 만 남는다. 즉시 종료되지 않는다.
- 대소문자 무시(`SHUTDOWN` 도 된다). 알 수 없는 줄은 WARN 1줄 남기고 무시.
- 사람이 실제 터미널에서 **Ctrl-C** 를 눌러도 같은 경로다.

**종료 순서** (AC-8d 를 만족시키는 순서):

1. 종료 신호 → `accepting_connections = false` → 새 `/ws` 는 503 `shutting_down`
2. axum graceful shutdown → 리스너 중지
3. tick 스레드가 종료 스윕: 살아 있던 모든 세션에 `SESSION_CLOSED{SERVER_SHUTDOWN}` 발행, 라우팅 표 비우고 **송신 태스크를 깨운다**
4. 송신 태스크가 close 1001 프레임 전송 → 소켓 종료
5. `live_connections` 가 0이 될 때까지 최대 3초 대기 (이게 없으면 프로세스가 먼저 죽어 **클라이언트가 close code 를 못 본다** — 실측으로 겪은 버그)
6. 남은 영속화 배치를 전부 커밋할 때까지 대기 → 종료 코드 0

---

## 4. 설정 (환경 변수)

`config.rs` 는 **`.env` 파일을 읽지 않는다.** 셸이나 프로세스 환경에 넣는다.

| 이름 | 기본값 | 비고 |
|------|--------|------|
| `STARFALL_HTTP_ADDR` | `127.0.0.1:8080` | `127.0.0.1` 고정 권장(ADR-0008 §4 의 전제) |
| `STARFALL_LOG_FORMAT` | `pretty` | `pretty` \| `json` |
| `RUST_LOG` | `info,starfall_gateway=debug` | 세션 수립·종료 DEBUG 줄이 여기 걸린다 |
| `DATABASE_URL` | `postgres://starfall:starfall_dev_only@127.0.0.1:15432/starfall` | |
| `REDIS_URL` | `redis://127.0.0.1:16379/0` | `/readyz` 전용 |
| **`STARFALL_TICK_HZ`** | `20` | `worlds.tick_hz` 와 다르면 **기동 거부**(종료 코드 1) |
| **`STARFALL_WORLD_ID`** | `01a0b1c2-3d4e-7f01-8a2b-9c0d1e2f3a4b` | `worlds` 에 없으면 기동 거부 |
| **`STARFALL_DEV_AUTH_SECRET`** | 없음 | 없거나 빈 문자열이면 `/ws` 만 503 `auth_not_configured` |

**설정으로 빼지 않은 것** (상수. 바꾸려면 코드 + ADR 갱신):
큐 용량 4096 / 64 / 256, 영속화 채널 512, 백로그 임계 600 tick, 하트비트 20 tick, 프레임 한도 16 KiB / 64 KiB, 위반 예산 10초 8회, ping 15초, 유휴 30초, 중복 제거 링 1024.
빼지 않은 이유: 설정 가능하면 QA 가 측정한 값이 어떤 설정에서 나온 것인지 매번 확인해야 하고, "잠정값"이라는 사실이 코드에서 사라진다.

**기동 거부 메시지** (grep 대상은 `worlds.tick_hz`):

```
ERROR ... 기동 거부: 월드 상수 불일치 ? worlds.tick_hz=20, STARFALL_TICK_HZ=10 (I-19, ADR-0006 §3)
```

거부는 `worlds` 를 **읽기만** 한다 — 행이 변하지 않고 tick 루프도 시작되지 않는다(SC-08 의 "worlds 행은 변하지 않는다" 성립).

---

## 5. `/debug/stats` — 키 이름과 의미 (계약 아님)

`curl.exe -s http://127.0.0.1:8080/debug/stats` 의 최상위 키 **31개**(실행 출력 순서 그대로):

| 키 | 종류 | 의미 |
|----|------|------|
| `start_tick` | 값 | 이 프로세스가 시작한 tick (재개 지점) |
| `tick` | 게이지 | 마지막으로 **완료한** tick |
| `tick_total` | 카운터 | 이 프로세스가 실행한 tick 수. **`tick_total == tick − start_tick + 1`** 이어야 한다 (I-17, SC-18) |
| `tick_overrun_total` | 카운터 | 본문 소요가 임계를 넘은 tick 수 |
| `tick_overrun_threshold_us` | 값 | 50000 (50 ms) |
| `tick_body_us` | 히스토그램 | `count` `sum_us` `max_us` `p50_le_us` `p90_le_us` `p99_le_us` `bucket_bounds_us` `buckets`. **`run_tick()` 본문 소요**이지 루프 주기가 아니다. `*_le_us` 는 분위수가 속한 **버킷 상한**(보수적), `max_us` 는 정확한 값 |
| `tick_lag_seconds` | 게이지 | 루프 주기 드리프트(실제 경과 − 이론 경과). 음수면 앞서 있다 |
| `ws_connections` | 게이지 | **tick 드라이버 라우팅 표의 실제 길이**. 카운터 뺄셈이 아니다 (I-25, SC-30) |
| `live_connections` | 게이지 | 아직 살아 있는 **소켓** 수. 종료 중에는 `ws_connections` 보다 잠깐 크다(스윕이 라우팅 표를 먼저 비우므로) |
| `sessions_opened_total` | 카운터 | `SESSION_OPENED` 발행 누적 |
| `sessions_closed_total` | 카운터 | `SESSION_CLOSED` 발행 누적 |
| `commands_received_total` | 카운터 | 파싱 성공 후 큐에 넣으려 시도한 명령 누적 |
| `commands_rejected_total` | 라벨 배열 | `[{label,count}]`. 라벨: `MALFORMED_COMMAND` `UNKNOWN_COMMAND_TYPE` `SCHEMA_VERSION_UNSUPPORTED` `DUPLICATE_COMMAND_ID` `SERVER_BUSY` `TOO_MANY_IN_FLIGHT` |
| `messages_enqueued_total` | 라벨 배열 | 송신 큐 투입. 라벨: `SESSION_READY` `COMMAND_RESULT` `PING_REPLY` |
| `messages_written_total` | 라벨 배열 | 소켓에 실제로 쓴 수. 같은 라벨 3종 |
| `messages_enqueued_all` | 카운터 | 위 합계 |
| `messages_written_all` | 카운터 | 위 합계 |
| `messages_dropped_total` | 카운터 | 연결 종료로 버려진 메시지 |
| `send_queue_depth` | 게이지 | 세션별 송신 채널 점유 슬롯 **합**(`max_capacity − capacity`). 카운터 뺄셈이 아닌 독립 출처 |
| `send_queue_depth_max` | 게이지 | 위 값의 최고 수위 |
| `command_queue_depth` | 게이지 | 전역 명령 큐 깊이 |
| `command_queue_depth_max` | 게이지 | 위 값의 최고 수위 |
| `protocol_violations_total` | 카운터 | 프로토콜 위반. **거부는 포함하지 않는다** |
| `upgrade_rejected_total` | 라벨 배열 | `/ws` 거절. 라벨: `auth_not_configured` `recording_backlog` `shutting_down` `no_credential` `invalid_token` |
| `upgrade_rejected_all` | 카운터 | 위 합계 |
| `domain_events_persisted_total` | 카운터 | 커밋된 도메인 이벤트 |
| `domain_events_persist_failed_total` | 카운터 | 제약 위반으로 **버린** 이벤트. **0이 아니면 버그다** |
| `last_committed_tick` | 게이지 | 마지막으로 커밋된 tick |
| `persist_backlog` | 게이지 | `tick − last_committed_tick` |
| `persist_backlog_limit` | 값 | 600 |
| `accepting_connections` | 불리언 | 새 연결을 받는 중인가 |

### QA 가 쓸 항등식 3개

```
SC-18  tick_total == tick − start_tick + 1
SC-30  ws_connections == sessions_opened_total − sessions_closed_total      (정지 시점)
SC-31  messages_enqueued_all == messages_written_all + messages_dropped_total + send_queue_depth
AC-9c  commands_received_total − messages_enqueued_total{COMMAND_RESULT} == 0
```

**SC-31 답 (QA 질문 ①)**: 정지 시점 `enqueued == written` 으로 판정해도 된다 — 그때 `dropped`·`depth` 가 둘 다 0이기 때문이다. 다만 **세 값을 함께 적어 달라.** `dropped > 0` 이면 그건 연결이 비정상 종료됐다는 뜻이지 회계가 틀린 게 아니다. 부하 중 샘플은 세 값이 원자적으로 읽히지 않으므로 **기록용**이다.

**`persist_backlog` 의 정상 범위**: 하트비트가 20 tick(1초)마다 커밋하므로 정상 동작에서도 **0~20 사이를 오간다.** 0이 아닌 것이 이상이 아니다. 600 을 넘으면 새 연결이 503 `recording_backlog` 를 받는다(임계는 `persist_backlog_limit` 으로 같이 노출한다).

---

## 6. SC ID → 테스트·명령 대응표

`cd server` 기준. 테스트 이름은 그대로 `--exact` 필터에 쓸 수 있다.

| SC | 1차 증명 | 명령 |
|----|---------|------|
| SC-01 | — | `cargo fmt --all --check` |
| SC-02 | — | `cargo clippy --workspace --all-targets -- -D warnings` + `grep -A1 '^\[lints\]' crates/sim/Cargo.toml crates/persistence/Cargo.toml` |
| SC-03 | — | `cargo test --workspace --locked` |
| SC-04 | — | `grep -nE "axum\|sqlx\|redis\|rand\|chrono" crates/sim/Cargo.toml` (종료 코드 1 = PASS), `cargo tree -p starfall-sim -e normal` |
| SC-05 | 실서버 | `\d+ domain_events` / `\d+ worlds` — §7 의 실행 결과 참조 |
| SC-06 | 실서버 | `select ... from worlds` |
| SC-07 | 실서버 | 2회 기동 후 `select version from _sqlx_migrations` |
| SC-08 | 실서버 | `STARFALL_TICK_HZ=10` 기동 → 종료 코드 1 + 로그 `worlds.tick_hz` |
| SC-09 | 실서버 | `select last_tick from worlds` vs `select max(tick) from domain_events` |
| SC-10 | 실서버 | `live_smoke` 2건 + 재기동 후 `/debug/stats.start_tick` |
| SC-11~13 | 실 DB 탐침 | §7 의 append-only 블록 (서버 정지 상태) |
| SC-14 | `ws_integration` | `sc14_valid_token_first_message_is_session_ready` / 실서버는 `live_round_trip_writes_session_rows` |
| SC-15 | `ws_integration` | `sc15_three_unauthenticated_cases_are_rejected` + 실서버 `curl.exe` 3회 |
| SC-16 | `ws_integration` | `sc16_missing_secret_disables_only_ws` |
| SC-17 | `starfall-sim` 유닛 | `cargo test -p starfall-sim --locked -- --nocapture commands_do_nothing_until_a_step_runs` (`#[tokio::test]` 아님) |
| SC-18 | `ws_integration` | `sc18_tick_identity_holds` |
| SC-19 | `ws_integration` | `sc19_command_result_precedes_ping_reply_with_same_tick` (검사 10건) |
| SC-20 | `ws_integration` | `sc20_in_flight_limit_rejects_without_closing` (120건 전송, 손실 0) |
| SC-21 | `ws_integration` | `sc21_duplicate_command_id_yields_one_ping_reply` |
| SC-22 | `ws_integration` | `sc22_slow_consumer_is_closed_with_slow_consumer_reason` |
| SC-23 | `starfall-gateway` 유닛 | `server_busy_is_structurally_unreachable_at_thirty_connections` (+ 부하 리포트에 `SERVER_BUSY == 0` 기록) |
| SC-24 | `ws_integration` | `sc24_oversized_text_counts_then_closes_with_1002` |
| SC-25 | `ws_integration` | `sc25_binary_frames_use_the_same_budget` |
| SC-26 | `ws_integration` | `sc26_idle_connection_is_closed_with_1001` (32초) |
| SC-27 | 실서버 + `live_smoke` | `live_open_session_is_closed_by_server_shutdown` + stdin `shutdown` |
| SC-28 | 유닛 + 위 항목들 | `close_codes_match_adr_0005_table` 이 표를 고정, 실제 관측은 SC-22·24·25·26·27 |
| SC-29 | 실서버 | `/readyz` 첫 호출 200 → `docker compose stop postgres` → 503 → `start` → 200 (PID 동일) |
| SC-30 | `ws_integration` | `sc30_sc31_stats_identities_hold_at_rest` + 코드 출처: `crates/gateway/src/runtime.rs` 의 `stats.set_ws_connections(routes.len() as u64)` |
| SC-31 | `ws_integration` | 위와 같은 테스트 |
| SC-32~37 | `contract_tests` | `cargo test -p starfall-contracts --locked -- --nocapture` (테스트 이름은 p0-01 그대로: `schemas_valid_offline` `fixtures_roundtrip` `invalid_rejected_by_schema` `invalid_serde_matrix` `registry_server_types_mapped` `registry_consistency` `registry_file_validates_against_schema` `schema_ids_match_paths` `required_field_mutations` `integer_bounds_rejected`) |

### 봇 probe 로 재현 가능/불가

| 항목 | 재현 | 비고 |
|------|:---:|------|
| SC-14·15·16·18·19·20·21·24·25·27 | 가능 | |
| **SC-17** | **불가(설계상)** | "런타임도 소켓도 없이 돈다"가 요구사항이다. 봇으로 재현하면 그 요구사항을 어기는 것 |
| **SC-23** | **불가(구조적)** | 30 × 64 = 1920 < 4096. 단위 테스트로 덮고 "구조적 미도달"로 기록 |
| SC-22 | 가능, 조건부 | 봇이 **수신을 완전히 멈추고** 계속 보내야 한다. 실측: 2261건 전송 시점에 서버가 닫았다. `--burst 3000` 이상 권장 |
| SC-26 | 가능, 조건부 | tokio-tungstenite 계열은 폴링하면 **자동으로 Pong** 을 보낸다. 30초 이상 폴링을 멈춰야 idle 이 성립한다 |

---

## 7. 실서버 검증 결과 (2026-09-19)

`docker compose down -v && docker compose up -d` 로 볼륨을 비운 상태에서 시작했다.

### 스키마·월드 (SC-05 / SC-06)

```
select count(*) from worlds;                        -> 1
select world_id,name,tick_hz,calendar_epoch,calendar_scale,sim_version,last_tick is null from worlds;
  01a0b1c2-3d4e-7f01-8a2b-9c0d1e2f3a4b|spike|20|3800-01-01T00:00:00Z|60|1|f   (f = last_tick IS NULL)
```

`domain_events` 열 12개(타입·NOT NULL 전부 ADR-0007 §2 와 일치), 제약: PK(`event_id`), FK(`world_id`→`worlds`), `UNIQUE (world_id, tick, sequence)`, CHECK 3종(`schema_version >= 1`, `tick` 0..9007199254740991, `sequence` 0..9007199254740991). 인덱스 4개(`domain_events_pkey`, `domain_events_correlation_idx`, `domain_events_type_idx`, `domain_events_world_id_tick_sequence_key`). 트리거 `domain_events_append_only`. `_sqlx_migrations` 에 버전 1.

### 왕복과 기록 (AC-5a / AC-6c / AC-16d / AC-17b)

`cargo test -p starfall-gateway --test live_smoke -- --ignored --nocapture` 출력:

```
[live] SESSION_READY session_id=... correlation_id=... tick_hz=20 world_id=01a0b1c2-3d4e-... server_version="0.1.0"
[live] 왕복 3건 확인 (COMMAND_RESULT → PING_REPLY, 같은 tick)
[live] 세션 쌍 가시까지 0.201초 (AC-17b 게이트 5초)
[live]   SESSION_OPENED tick=4250 occurred_at=3800-01-01T03:32:00Z
[live]   SESSION_CLOSED tick=4254 occurred_at=3800-01-01T03:32:00Z
[live] close_reason=CLIENT_CLOSED, occurred_at 재계산 2건 일치
```

같은 실행의 `/debug/stats`:

```
tick=4261 start_tick=4221 tick_total=41 identity_ok=True
overrun=0/41  body_p50_le=100us  body_p99_le=500us  body_max=201us  lag_s=-0.044
enqueued=7 written=7 dropped=0 depth=0 accounting_ok=True
received=3 enqueued_CR=3          (AC-9c: 차이 0)
persisted=2 failed=0 backlog=1 limit=600
```

### 인증 (AC-5b / AC-5c)

```
no-header=401  bad-sig=401  not-v7=401
SESSION_OPENED before=3 after=3        (I-24: 증가 0)
upgrade_rejected_total: no_credential=1, invalid_token=2
```

비밀 미설정으로 기동한 프로세스:

```
GET /ws     -> 503 {"reason":"auth_not_configured","status":"unavailable"}
GET /healthz-> 200      GET /readyz -> 200
```

### tick 재개 (AC-3 / SC-09 / SC-10)

```
정상 종료 후:  last_tick=4053   max(tick)=4053     (last_tick >= max(tick) 성립)
재기동:        start_tick=4054  = max(last_tick, max(tick)) + 1
탐침 행 삽입 후 재기동: start_tick 이 탐침 tick 을 넘어선다 (QA §B 규칙 3 그대로)
```

### 정상 종료 (AC-8d / SC-27)

세션 1개를 열어 둔 채 stdin `shutdown`:

```
exited=True exitcode=0 shutdown_elapsed_s=0.1
클라이언트 관측: close code = Some(1001)
DB: close_reason = SERVER_SHUTDOWN
서버 로그: tick 루프 종료 ? SERVER_SHUTDOWN 스윕 후 영속화 flush tick=4399 sessions_closed=1 pending_batches=1
          영속화 태스크 종료 ? 남은 배치 없음
          starfall game-server 정상 종료 ? 마지막 tick 까지 커밋 완료 tick=4398
```

### append-only 탐침 (SC-11~13)

서버 정지 상태, `probe_tick = max(tick)+1`, `sequence=0`:

```
UPDATE -> ERROR: domain_events is append-only (UPDATE blocked). 정정은 새 레코드로 한다.
DELETE -> ERROR: domain_events is append-only (DELETE blocked). 정정은 새 레코드로 한다.
같은 event_id 재삽입 (ON CONFLICT DO NOTHING) -> INSERT 0 0   (행 수 불변)
다른 event_id, 같은 (world,tick,sequence)     -> ERROR: duplicate key ... domain_events_world_id_tick_sequence_key
행 수: 8 -> 9 (정확히 +1)
빈틈 검사 SQL: 0행
```

---

## 8. 마이그레이션을 고친 뒤 (체크섬 불일치)

`sqlx::migrate!` 는 적용된 마이그레이션의 체크섬을 `_sqlx_migrations` 에 저장한다. **`0001_*.sql` 을 한 글자라도 고치고 다시 기동하면 기동이 실패한다**(`previously applied but has been modified`).

```bash
cd C:\WorkSpace\SpaceHistoric
docker compose down -v && docker compose up -d
docker compose exec -T postgres pg_isready -U starfall -d starfall -t 60
```

- 이 명령은 `docker-compose.yml` 의 `name: starfall` 덕분에 **starfall 프로젝트에만** 작용한다(다른 프로젝트 컨테이너 5개·볼륨 90여 개는 건드리지 않는다).
- 이 실패는 FAIL 이 아니라 **절차 오류**다. 절차를 지키고 다시 측정한다.
- 마이그레이션 파일은 **LF** 로 저장했다(확인함). CRLF 로 바뀌면 체크섬이 플랫폼 간 달라진다.
- **새 마이그레이션 파일을 추가해도 재컴파일되지 않는 문제**는 `crates/persistence/build.rs` 의 `cargo:rerun-if-changed=migrations` 로 해결했다(실측: 기존 파일 *수정*은 재컴파일되지만 *추가*는 되지 않는다. ADR-0007 §5 의 표현을 "추가했을 때"로 좁혀 읽어야 한다).

---

## 9. 구현 중 발견해 고친 것 (QA 가 회귀로 볼 것)

| # | 증상 | 원인 | 고침 | 회귀 테스트 |
|---|------|------|------|------------|
| 1 | 클라이언트가 끊어도 `SESSION_CLOSED` 가 최대 15초 늦게 발행 | 송신 채널 `Sender` 를 tick 드라이버도, 수신 태스크도 들고 있어 **로컬 drop 으로는 송신 태스크가 깨지 않았다.** 다음 ping(15초)까지 잠들었다 | 읽기/쓰기 태스크가 서로를 끝낸다(`finish` Notify + `reader.abort()`) | `graceful_client_close_is_prompt` (2초 게이트) |
| 2 | 정상 종료 시 클라이언트가 close code 를 못 봄 | 종료 스윕이 라우팅 표를 비워도 위 이유로 송신 태스크가 깨지 않았고, 깨워도 프로세스가 먼저 죽었다 | `SessionRoute.finish` 로 드라이버가 직접 깨우고, 종료 시 `live_connections == 0` 을 최대 3초 기다린다 | `live_open_session_is_closed_by_server_shutdown` (close 1001 관측) |
| 3 | 폭주 클라이언트의 거부 응답이 **조용히 사라짐** (8000건 중 7872건 거부인데 큐 투입은 1019건) | `send_rejection` 이 `try_send` 실패를 무시했다 — I-22 위반이고 "보낸 수 == 받은 COMMAND_RESULT 수"가 깨진다 | 큐에 못 넣으면 `SLOW_CONSUMER` 로 연결을 닫는다 | `sc22_slow_consumer_is_closed_with_slow_consumer_reason` |
| 4 | stdin `shutdown` 무시 | PowerShell StreamWriter 가 BOM 을 붙여 `\u{feff}shutdown` 이 들어왔다 | 서버가 BOM 을 걷어낸다 | 실서버 절차(§3.2)로 확인 |
| 5 | SC-04 의 grep 이 주석에 걸림 | `crates/sim/Cargo.toml` 주석에 금지 크레이트 이름을 적었다 | 이름을 `src/lib.rs` 문서로 옮겼다 | `grep` 종료 코드 1 |

---

## 10. 실측값 (M-1 ~ M-13 중 server 몫)

| # | 항목 | 값 | 비고 |
|---|------|-----|------|
| M-1 | tick 초과 비율 | **0 / 41** (실서버 무부하) | 측정 지점: `crates/gateway/src/runtime.rs` 의 `body_start` ~ `body`(sleep 제외). 부하 측정은 QA |
| M-2 | 단일 tick 본문 최대 | **201 µs** (무부하) | `tick_body_us.max_us` 는 정확한 값 |
| M-4 | `tick_lag_seconds` | **−0.044 s** (41 tick 구간) | 이 PC 바닥값은 전용 OS 스레드 기준 **+0.7 %/분**. tokio 타이머였다면 **+22 %/분** |
| M-5 | tick 본문 소요 | p50 ≤ 100 µs, p90 ≤ 100 µs, p99 ≤ 500 µs | 버킷 상한 표기 |
| M-9 | 서버 워크스페이스 클린 빌드 | **131초** (`cargo clean && cargo build --workspace`, 2026-09-19) | p0-01 기준선은 26초였지만 **같은 대상이 아니다**: 크레이트 3 → 5, axum `ws` + sqlx `macros`·`migrate`·`uuid`·`json` + hmac/sha2/subtle 추가. feature 추가분만 떼어 낸 실측은 +2.8초였고(검토 U-3), 나머지는 새 크레이트 2개와 `sqlx-macros-core` 컴파일이다 |
| M-11 | U-9 tungstenite 자동 Pong | **참** — 스트림을 폴링하면 자동으로 Pong 을 보낸다. 그래서 idle 을 만들려면 **폴링 자체를 멈춰야** 한다 | `sc26` 이 32초 무폴링으로 재현 |
| — | 계약 테스트 검사 건수 | **유효 fixture 12 / 반례 16 / 스키마 11** | 테스트 출력에 찍힌다 |
| — | 라이브 왕복 → DB 가시 | **0.079 s / 0.201 s** (2회) | 게이트 5초 |

**측정 환경 주의**: 첫 실행에서 "세션 쌍 가시까지 14.9초"가 나왔다. 그 실행은 **같은 PC 에서 `cargo test` 가 테스트 바이너리를 컴파일하는 중**이었다. 컴파일 부하가 없는 상태에서 두 번 재측정해 0.079 s / 0.201 s 였다. 스펙 §0.7 이 측정 환경 기록을 요구하는 이유가 정확히 이것이다 — **부하 측정 중에 빌드를 돌리지 말 것.**

---

## 11. 알려진 한계

1. **outbox 가 없다.** 프로세스를 **강제 종료(kill)** 하면 아직 커밋되지 않은 tick 의 이벤트가 사라진다. 정상 종료 경로의 손실 0은 §7 에서 확인했다. 이것을 없애는 것이 p1 의 첫 과제다(ADR-0007 §6). QA 는 하드 킬로 AC-8(d) 를 대체하지 말 것.
2. **영속화 백로그에 상한이 없다.** DB 가 영영 복구되지 않으면 미룬 이벤트가 메모리에 계속 쌓인다(ADR-0007 §4). 600 tick 을 넘으면 새 연결만 거절한다. 상한과 그 뒤의 동작은 Q11(p1).
3. **`SERVER_BUSY` 는 30 연결에서 도달하지 않는다.** 30 × 64 = 1920 < 4096. 단위 테스트로만 덮인다.
4. **세션 내 중복 제거는 지속 멱등성이 아니다.** 연결이 끊기면 기억이 사라지고 1024개를 넘으면 오래된 것부터 잊는다(ADR-0006 §6). 그래서 재연결 시 재전송이 금지되어 있다.
5. **`SLOW_CONSUMER` 의 close code 1011 은 피어가 못 볼 수 있다.** 응답을 읽지 않는 피어에게 서버가 소켓을 닫으면 TCP 가 RST 를 보내 수신 버퍼(쌓여 있던 Close 프레임 포함)가 버려진다. **판정의 정본은 DB 의 `close_reason`** 이다(ADR-0005 §2 가 정한 대로).
6. **`recorded_at` 은 호스트 시계**(컨테이너 시계가 아니다). AC-17(b) 는 `recorded_at` 비교가 아니라 호스트에서 `count(*)` 폴링으로 재야 한다.
7. **컴파일 타임 쿼리 검사가 없다**(ADR-0007 §5). SQL 오타는 런타임에 드러난다. 쿼리 4종을 실 DB 통합 경로로 덮었다.
8. `/debug/stats` 의 분위수는 **버킷 상한**이다(`*_le_us`). 정확한 값이 필요한 것은 `max_us` 뿐이고 그것은 정확하다.

---

## 12. 계약 변경 요청

**없다.** `contracts/**` 를 한 글자도 고치지 않았고, 신규 4타입의 스키마·fixture 그대로 Rust 타입을 만들어 계약 테스트 10종이 전부 통과한다(유효 12 / 반례 16 / 스키마 11).

architect 개정(§5 `tick-zero.json` 의 `world_id` 교체, `SESSION_CLOSED/invalid/actor-id-null.json` 추가)은 그대로 반영됐다 — `SERDE_REJECTION_TABLE` 에 7행을 추가해 반례 16건을 덮는다(파일 이름이 겹치는 `actor-id-null.json`·`payload-unknown-field.json` 은 한 행이 두 타입을 덮는다).

---

## 13. 스펙에서 틀렸다고 판단한 것

없다. 착수 전 검토(`01_server_spec_review.md`)에서 제기한 19건은 architect 가 전부 처리했고, 구현 중 스펙과 어긋난 지점은 나오지 않았다. 다만 두 가지를 **문구 보강**으로 남긴다(판정을 바꾸지 않는다):

1. **SC-09 는 정상 종료 경로에서 측정해야 한다.** `last_tick` 은 이벤트가 있는 tick 과 1초 하트비트에서만 갱신되므로, 하드 킬 직후에 재면 `last_tick < max(tick)` 이 될 수 있다(그 경우는 M-13 손실 창에 해당한다).
2. **SC-22 의 "close 1011 로 닫히고"** 는 피어가 읽지 않는 상황이라 close code 관측이 보장되지 않는다(§11-5). `close_reason = SLOW_CONSUMER` 로 판정하면 된다 — 서버는 1011 을 실제로 보낸다.

# server의 스프린트 계약 확인 (p0-02)

- 작성: server, 2026-09-18 (구현 착수 시점)
- 대상: `02_sprint_contract.md` SC-01~37 + 공통 부록(§0.5 층별 표, §2 비교표 대상, §4 게이트, §5 미검증 기준)
- 스펙: `docs/specs/p0-02-networking-spike.md` (agreed, **AC-1~21 재배치판**). 내 검토 문서 `01_server_spec_review.md`는 옛 번호를 쓴다 — 아래는 전부 새 번호다.

## 0. 확인란

| 영역 | 항목 | 확인 | 비고 |
|------|------|:---:|------|
| A 빌드·크레이트 위생 | SC-01 ~ SC-04 | **☑** | |
| B 마이그레이션·tick 재개·append-only | SC-05 ~ SC-13 | **☑** | SC-09에 조건 1건(아래 §1.2) |
| C 인증 | SC-14 ~ SC-16 | **☑** | |
| D tick·큐·수명 주기·프레이밍 | SC-17 ~ SC-28 | **☑** | SC-26에 관측 방법 보강(§2.4) |
| E 운영 표면 | SC-29 ~ SC-31 | **☑** | SC-31은 메트릭 2종 추가로 강화(§2.1) |
| F 계약 ↔ Rust | SC-32 ~ SC-37 | **☑** | |
| 부록 §0.5 / §2 / §4 / §5 | 공통 | **☑** | §0.5 표와 다른 결과가 나오면 architect 통지, FAIL 아님 — 동의 |

**차단급 이의 없음.** 아래 §1은 "이대로 하면 증명이 약해진다"는 보강 요청 3건이고, 전부 **내 구현 범위 안에서 해결**한다(계약 문구 변경이 필요한 것은 §1.2 하나뿐이며 그것도 "값의 해석"을 적는 수준이다).

---

## 1. 이의·보강 (3건)

### 1.1 SC-31은 현재 문구로는 "잔량 0"을 증명하지 못한다 → 메트릭 2종을 추가한다

`enqueued − written`은 **두 카운터의 뺄셈**이라, 그 차이를 다시 "큐 잔량"이라고 부르면 I-25가 금지한 항등식이 된다. 게다가 중간에 **소켓 오류로 버려진 메시지**가 있으면 두 값의 차이는 잔량이 아니라 "잔량 + 손실"이 되고, 그 상태로 `enqueued == written`을 기다리면 영원히 오지 않는다.

**구현 대응(§2.1에 상세):** `/debug/stats`에 두 값을 더 노출한다.

- `send_queue_depth` — **세션별 송신 채널의 실제 점유 슬롯 합**(`max_capacity − capacity`). 카운터가 아니라 채널 상태에서 읽는 독립 출처다.
- `messages_dropped_total` — 연결이 끊겨 송신되지 못하고 버려진 메시지 수.

그러면 SC-31은 **정확한 항등식**이 된다:

```
messages_enqueued_total == messages_written_total + messages_dropped_total + send_queue_depth
```

**판정 제안:** QA가 적은 "정지 시점 `enqueued == written`"을 그대로 두되, **`send_queue_depth == 0`과 `messages_dropped_total` 값을 함께 적는다.** 모든 세션이 정상 종료된 정지 시점이면 셋 다 0이고 `enqueued == written`이 성립한다. 부하 중 샘플에서는 세 값이 원자적으로 읽히지 않으므로 **등식이 근사로만 성립한다** — 부하 중 관측은 기록으로만 쓰고 판정은 정지 시점에 한다는 QA의 판단이 옳다.

### 1.2 SC-09의 `last_tick >= max(tick)`은 조건이 하나 붙어야 항상 참이다

`last_tick`은 **이벤트가 있는 tick과 1초 주기 하트비트 tick에서만** 갱신된다(§2.5). 그래서 "세션 1개를 열고 닫은 뒤 **정상 종료**"라는 SC-09의 Given에서는 `last_tick >= max(tick)`이 반드시 성립한다(정상 종료가 마지막 배치를 커밋하고 기다린다). **하드 킬 뒤에 이 쿼리를 돌리면 성립하지 않을 수 있다** — 커밋 전에 죽으면 둘 다 그 tick을 모르므로 여전히 모순은 아니지만, "마지막 배치만 커밋되고 `worlds`는 못 갱신" 같은 상태는 트랜잭션이 하나라 생기지 않는다.

**요청:** SC-09 문구에 "**정상 종료 경로에서 측정한다**(하드 킬 직후 측정은 M-13 손실 창에 해당한다)"를 한 줄 추가. 값 자체는 그대로다.

### 1.3 SC-24의 "10초 창"은 시작 기준이 없으면 구현마다 달라진다 → 슬라이딩 창으로 고정한다

"고정 창"(10초마다 리셋)과 "슬라이딩 창"은 8회째의 타이밍이 다르다. **슬라이딩으로 구현한다**(§2.4). 계약 문구 변경은 필요 없고, QA probe가 9회를 **10초 안에** 보내면 재현된다.

---

## 2. QA 질문 6건에 대한 답

### 2.1 (Q1) SC-31 — 송신 큐 잔량 판정

**답: 정지 시점 `enqueued == written`으로 판정해도 된다. 다만 독립 관측 수단을 내가 추가한다.**

`/debug/stats`에 다음을 노출한다.

| 이름 | 종류 | 출처 | 의미 |
|------|------|------|------|
| `messages_enqueued_total` | 카운터(타입별 맵 + `_all`) | 송신 채널에 `try_send` 성공 직후 | 서버가 만들어 보내기로 한 메시지 |
| `messages_written_total` | 카운터(타입별 맵 + `_all`) | 소켓 write가 `Ok`를 반환한 직후 | 실제로 와이어에 나간 메시지 |
| `messages_dropped_total` | 카운터 | 세션 종료로 송신 태스크가 버린 메시지 | 손실의 유일한 정당한 출처 |
| **`send_queue_depth`** | **게이지** | **세션별 채널의 `max_capacity − capacity` 합** | **카운터와 무관한 독립 출처** |
| `send_queue_depth_max` | 게이지(최고수위) | 위 값의 관측 최댓값 | 용량 256이 적절한지의 근거(M-6과 같은 성격) |

판정식: `enqueued == written + dropped + send_queue_depth`. **정지 시점(모든 세션 닫힘)에는 `dropped`·`depth`가 0이므로 QA가 적은 `enqueued == written`이 그대로 성립한다.**

세션별 잔량 합계까지 필요하면 그때 노출하겠지만, 이번 슬라이스에는 합계로 충분하다고 본다(세션별로 쪼개야 하는 질문이 아직 없다).

### 2.2 (Q2) SC-27 — stdin 종료 절차와 바이너리 경로

**바이너리 경로는 맞다:** `server/target/debug/starfall-game-server.exe` (`[[bin]] name = "starfall-game-server"`가 `bins/game-server/Cargo.toml`에 이미 고정되어 있다).

**stdin 종료 규약 (구현 확정):**

- 서버는 기동 시 stdin을 읽는 **전용 블로킹 스레드**를 띄운다.
- 한 줄을 읽어 앞뒤 공백을 제거한 값이 **`shutdown`**(대소문자 무시)이면 Ctrl-C와 **같은** 종료 경로를 탄다.
- **EOF(stdin이 닫혀 있음)는 종료 신호가 아니다.** 리다이렉트 없이 띄우거나 stdin이 `NUL`인 환경에서 즉시 종료되면 안 되므로, EOF를 만나면 그 스레드는 조용히 끝나고 서버는 계속 돈다. 이 경우 종료는 Ctrl-C로만 가능하다.
- 알 수 없는 줄은 무시하고 계속 읽는다(WARN 로그 1줄).

**`cargo run`으로 띄웠을 때:** cargo는 자식 프로세스에 stdio를 그대로 물려주므로 stdin이 서버까지 전달된다. 다만 `cargo run`은 **중간에 cargo 프로세스가 하나 더 끼어** `$p.ExitCode`·`Get-Process` 관측이 헷갈린다. **QA는 exe를 직접 띄우는 D-2 방식을 쓰기를 권한다**(계약에 적힌 형태 그대로가 맞다).

D-2 레시피는 그대로 동작한다. 한 가지만 보강한다 — `WriteLine` 뒤에 **`Flush()`**를 부르고, 환경 변수를 넘기려면 `$psi.Environment`를 쓴다:

```powershell
$exe = "C:\WorkSpace\SpaceHistoric\server\target\debug\starfall-game-server.exe"
$psi = [Diagnostics.ProcessStartInfo]::new($exe)
$psi.RedirectStandardInput = $true
$psi.UseShellExecute = $false
$psi.WorkingDirectory = "C:\WorkSpace\SpaceHistoric\server"
$psi.Environment["STARFALL_DEV_AUTH_SECRET"] = "dev_only_not_a_secret"
$p = [Diagnostics.Process]::Start($psi)
# ... 세션을 열어 두고 ws_connections 확인 ...
$p.StandardInput.WriteLine("shutdown"); $p.StandardInput.Flush()
$p.WaitForExit(); "exit=$($p.ExitCode)"   # 0 기대
```

**정상 종료 시 종료 코드는 0이다.** 종료 순서는 §2.6.

`RedirectStandardOutput`은 켜지 말기를 권한다 — 켜면 `WaitForExit()` 전에 파이프를 비우지 않아 데드락에 빠질 수 있다. 로그는 콘솔로 그대로 두거나 `STARFALL_LOG_FORMAT=json`으로 두고 파일 리다이렉트를 쓴다.

### 2.3 (Q3) SC-08 / SC-16 — 환경 변수 이름과 기동 거부 종료 코드

**환경 변수 (확정).** `config.rs`는 여전히 `.env`를 읽지 않는다 — 셸 환경 변수로 주입한다.

| 이름 | 기본값 | 없으면 |
|------|--------|--------|
| `STARFALL_HTTP_ADDR` | `127.0.0.1:8080` | 기본값 |
| `STARFALL_LOG_FORMAT` | `pretty` | 기본값 (`pretty`\|`json`) |
| `DATABASE_URL` | `postgres://starfall:starfall_dev_only@127.0.0.1:15432/starfall` | 기본값 |
| `REDIS_URL` | `redis://127.0.0.1:16379/0` | 기본값 |
| **`STARFALL_TICK_HZ`** | `20` | 기본값. `worlds.tick_hz`와 다르면 **기동 거부** |
| **`STARFALL_WORLD_ID`** | `01a0b1c2-3d4e-7f01-8a2b-9c0d1e2f3a4b` | 기본값. `worlds`에 없으면 **기동 거부** |
| **`STARFALL_DEV_AUTH_SECRET`** | 없음 | 서버는 **정상 기동**하고 `/ws`만 503 `auth_not_configured` |

백로그 임계(600 tick)·큐 용량(4096/64/256)·프레임 한도(16 KiB/64 KiB)·위반 예산(10초 8회)은 **환경 변수로 빼지 않는다.** 설정으로 빼면 QA가 측정한 값이 어떤 설정에서 나온 것인지 매번 확인해야 하고, 잠정값이라는 사실이 코드에서 사라진다. 바꿔야 하면 상수를 고치고 ADR을 갱신한다.

**기동 거부 (SC-08).**

- 종료 코드: **1** (`ExitCode::FAILURE`). 정상 종료는 0.
- 로그(ERROR, 1줄)에 **`worlds.tick_hz`** 문자열이 그대로 들어간다 — grep 대상으로 쓰라:
  ```
  기동 거부: 월드 상수 불일치 — worlds.tick_hz=20, STARFALL_TICK_HZ=10 (I-19, ADR-0006 §3)
  ```
- `world_id`가 `worlds`에 없을 때도 같은 형태:
  ```
  기동 거부: worlds 에 world_id=... 행이 없다 (마이그레이션 시드 확인)
  ```
- **거부는 `worlds` 행을 읽기만 하고 어떤 쓰기도 하지 않는다** → SC-08의 "`worlds` 행은 변하지 않는다"가 성립한다. tick 루프도 시작되지 않으므로 `domain_events`도 늘지 않는다.

**SC-16 (비밀 미설정).** 프로세스는 정상 기동하고 WARN 1줄을 남긴다. `/ws`만 `503 {"status":"unavailable","reason":"auth_not_configured"}`. `/healthz`·`/readyz`·`/debug/stats`는 영향 없음. tick 루프와 영속화는 정상 동작한다(세션이 안 생길 뿐이다).

### 2.4 (Q4) SC-14~26 — 테스트 이름 대응표와 봇 probe 재현 가능성

**대응표는 `03_server_impl.md`에 SC ID → 테스트 이름/명령 표로 남긴다.** 지금 확정해 두는 것은 "어느 항목이 봇으로 재현 가능한가"다.

| SC | 서버 쪽 1차 증명 | 봇 probe 재현 | 비고 |
|----|-----------------|:---:|------|
| SC-14 auth-ok | 게이트웨이 통합 테스트(실제 WS 클라이언트) | **가능** | |
| SC-15 401 3종 | 통합 테스트 | **가능** (`curl.exe`로도 된다 — C-1 그대로 동작) | |
| SC-16 비밀 미설정 | 통합 테스트 | **가능** (`curl.exe`) | |
| SC-17 수동 step | `cargo test -p starfall-sim` 단위 테스트 | **불가(설계상)** | 소켓·런타임 없이 도는 것이 요구사항이다. 봇으로 재현하면 그 요구사항을 어기는 것 |
| SC-18 tick 항등식 | 통합 테스트 + `/debug/stats` | **가능** (`curl.exe`) | |
| SC-19 순서 | 통합 테스트 | **가능** | |
| SC-20 in-flight | 통합 테스트 | **가능** | |
| SC-21 중복 | 통합 테스트 | **가능** | |
| SC-22 slow consumer | 통합 테스트 | **가능, 단 조건 있음**(아래) | |
| SC-23 SERVER_BUSY | 단위 테스트 | **불가(구조적)** | 30×64=1920 < 4096. 계약이 이미 그렇게 적었다 |
| SC-24 oversize | 통합 테스트 | **가능** | |
| SC-25 binary | 통합 테스트 | **가능** | |
| SC-26 idle | 통합 테스트 | **가능, 단 조건 있음**(아래) | |
| SC-27 stdin 종료 | 수동(D-2) + 통합 테스트(종료 경로) | **가능** | |
| SC-28 close code 매트릭스 | 위 항목들의 합 | **가능** | |

**SC-22(slow consumer) 재현 조건.** 송신 큐 256을 채우려면 **OS 소켓 버퍼(양쪽 합쳐 100 KB 이상)를 먼저 채워야** 한다. 봇은 (a) **수신을 완전히 멈추고**, (b) 계속 `PING_SERVER`를 보내야 한다. 명령 1건당 응답 2건(약 200 B)이 나가므로 **최소 600~800건**은 보내야 한다 — `--burst 500`으로는 부족할 수 있다. **`--burst 2000` 이상을 권한다.** in-flight 해제가 "COMMAND_RESULT 생성 시점"이므로 봇은 계속 보낼 수 있다(ADR-0006 §5, 내 B-12 요청이 수용된 이유가 이것이다).

**SC-26(idle) 재현 조건.** 서버는 15초마다 Ping을 보내고, **Pong·데이터 프레임 어느 것도 30초간 없으면** 닫는다. tokio-tungstenite 계열 클라이언트는 **스트림을 폴링하면 자동으로 Pong을 보낸다.** 그래서 봇이 idle을 만들려면 **수신 폴링 자체를 30초 이상 멈춰야** 한다. 그러면 close 프레임도 읽지 못하므로, 봇은 30초 뒤에 폴링을 재개해 남아 있는 Close 프레임을 읽거나, TCP 종료만 관측한다.
**close code 1001을 직접 못 읽었더라도 FAIL이 아니다** — 판정의 정본은 **DB의 `close_reason = IDLE_TIMEOUT`**(독립 출처)이고, 나는 통합 테스트에서 실제 close code 1001을 확인해 `03_server_impl.md`에 증거로 남긴다. U-9(tungstenite 자동 Pong)도 그 테스트로 실측해 M-11에 적는다.

**봇이 서버 단독 항목을 재현하지 못해도 판정이 막히지 않게**, 나는 SC-14~28의 전 항목을 **실제 WebSocket 클라이언트를 쓰는 게이트웨이 통합 테스트**로 먼저 덮는다(`tokio-tungstenite`를 gateway의 **dev-dependency**로만 추가 — 프로덕션 의존 트리는 늘지 않는다. axum의 `ws` feature가 이미 같은 크레이트를 트리에 갖고 있다).

### 2.5 (Q5) SC-24 — `protocol_violations_total` 증가 지점과 10초 창

**증가 지점.** 게이트웨이의 수신 루프가 프레임을 **위반으로 분류한 그 자리에서, 위반 1건당 정확히 1 증가**한다. 연결을 닫을지 여부와 **무관하게** 증가한다 — 1회째도 8회째도 9회째도 오른다. 그래서 SC-24가 요구하는 "위반 1회 후 `/debug/stats` 증가 확인(끊기지 않음)"이 그대로 관측된다.

위반으로 세는 것 (ADR-0006 §6):

1. 16 KiB 초과 텍스트 메시지 (재조립 후 길이 기준)
2. 바이너리 프레임
3. JSON 파싱 불가 / 계약 타입 역직렬화 실패

3번은 **`command_id`를 뽑을 수 있으면 `COMMAND_RESULT{REJECTED, MALFORMED_COMMAND}`도 함께 보낸다.** 뽑을 수 없으면 카운터만 오른다. 1·2번은 `command_id`가 없으므로 언제나 카운터만이다.

**거부(rejection)는 위반이 아니다.** `TOO_MANY_IN_FLIGHT`·`SERVER_BUSY`·`DUPLICATE_COMMAND_ID`는 `commands_rejected_total{reason}`만 올리고 `protocol_violations_total`은 건드리지 않는다. SC-20/SC-21이 SC-24의 예산을 소모하지 않는다는 뜻이다.

**10초 창 = 슬라이딩.** 세션마다 **최근 8건의 위반 시각**을 고정 크기 링으로 들고 있는다. 새 위반이 났을 때 링이 가득 차 있고 **그 8건 중 가장 오래된 것이 10초 이내**면 → "10초 안에 9번째" → close 1002 + `PROTOCOL_VIOLATION`. 즉 **창의 시작은 고정 epoch이 아니라 보존 중인 가장 오래된 위반의 시각**이다.

QA 재현: **9회를 10초 안에** 보내면 9회째에 닫힌다. `--violations 9`가 10초 안에 끝나면 그대로 재현된다. 10초를 넘겨 띄엄띄엄 보내면 닫히지 않는 것이 정상이다.

### 2.6 (Q6) SC-11~13 — 탐침이 `start_tick`을 밀어 올리는 것

**동의한다. 정상 동작이다.** tick 재개 쿼리는 `worlds.last_tick`과 `max(domain_events.tick)` 중 **큰 값 + 1**에서 시작한다. 탐침 행이 `max(tick)+1`에 들어가면 다음 기동의 `start_tick`은 `탐침 tick + 1`이 된다. I-17("뒤로 가지 않는다")은 그대로 지켜지고, 게임 시간도 앞으로만 간다.

**탐침을 마지막 블록으로 미루는 것(§4 게이트 G-i)에도 동의한다.** 오히려 그래야 한다 — 서버가 도는 중에 탐침을 넣으면 그 tick을 서버가 같이 쓰려다 `UNIQUE` 위반을 일으키고, ADR-0007 §4가 "버그 신호"로 정한 에러가 가짜로 뜬다. **서버 정지 상태에서 넣는다**는 QA의 §B 규칙 1이 정확하다.

한 가지만 덧붙인다: 탐침 행의 `tick`은 **`9007199254740991`(계약 상한)을 넘으면 안 된다.** `max(tick)+1` 방식이면 문제 없고, DB CHECK가 마지막 방어선으로 막는다.

---

## 3. 부록 항목에 대한 확인

- **§0.5 층별 표** — Rust serde 열(②) 16행 전부를 그대로 구현 목표로 삼는다. 내 테이블(`SERDE_REJECTION_TABLE`)이 이 16행을 그대로 담고, 결과가 표와 다르면 **코드가 아니라 표가 틀렸을 수도 있으므로 architect에게 통지**한다(FAIL 처리하지 않는다) — 동의.
- **§2 경계면 비교표(46필드)** — Rust 쪽 기대값(널 가능 3행은 `Option<T>` + `skip_serializing_if` **금지**, 좁힘 2행은 비-`Option`, 이벤트 envelope의 `correlation_id`는 비-`Option`이며 메시지 envelope의 것과 **다른 타입**)에 동의한다. 그대로 구현한다.
- **§4 게이트** — G-a(`down -v`가 첫 DB 단계), G-f(위반·종료 계열을 부하와 분리), G-g(별도 기동이 필요한 항목을 부하 밖으로), G-i(탐침은 마지막·서버 정지) 전부 동의. 특히 **G-g**: `STARFALL_TICK_HZ=10` 기동은 **서버가 뜨지 않으므로** 부하 중에 하면 그 부하가 끊긴다.
- **§5 미검증 기준** — E1~E8 동의. **"환경이 없어서"와 "구현이 없어서"를 섞지 않는다**에 동의하고, 내 영역에서 구현이 없어 실패하면 FAIL로 받는다.
- **M-1 측정 지점** — `run_tick()` 본문 소요를 재는 곳의 **파일:라인**을 `03_server_impl.md`에 적는다. 루프 주기(`tick_lag_seconds`)와 **다른 값**이라는 것을 같은 표에 나란히 적는다.

---

## 4. 구현하면서 바뀔 수 있는 것

이 문서는 착수 시점의 확인이다. 구현 중 여기 적은 것과 다르게 되면 **`03_server_impl.md`에 차이와 이유를 적고 QA에 알린다.** 특히 다음 셋은 실측으로 확인한 뒤에야 확정이다.

1. 송신 큐 256이 SC-22에서 실제로 포화하는 데 필요한 명령 수(위 추정 600~800건).
2. tungstenite의 자동 Pong 동작(U-9) — SC-26 재현 절차가 여기 달려 있다.
3. `persist_backlog`가 D 단계에서 30초 안에 600을 넘는지 — 내 설계는 **1초 주기 하트비트 커밋**으로 이벤트가 없어도 백로그가 자라게 만든다(§2.1의 ADR-0007 §4 해석). 그러지 않으면 A 단계 중 PostgreSQL을 내려도 **새 도메인 이벤트가 없어서 백로그가 0으로 유지되고 SC-66이 재현되지 않는다.** 이것은 계약이 아니라 내 구현 결정이며, 실측으로 확인해 `03_server_impl.md`에 적는다.

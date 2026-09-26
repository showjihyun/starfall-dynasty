# p1-01-ship-movement — QA 평가 리포트 라운드 3

- 작성: qa, 2026-09-21
- 계약: `02_sprint_contract.md` (**5차 개정** 반영)
- 라운드 1: PASS 34 / FAIL 3 / 미검증 0 — **유효**
- 라운드 2: PASS 16 / FAIL 12 / 보류 3 — **§1 에서 그중 2건의 귀속을 정정한다**
- 범위: **근본 원인 독립 검증 → qa 도구 수정 2건 → S-A·S-B·S-C 재판정 → 블록 4 → 블록 5**
- **이름 대응(2026-09-22 추가, 본문은 과거 기록이라 고치지 않는다)**: 이 리포트의 **AC-2(h-log) = 스펙 AC-2(i)**, **AC-2(h-dir) = 스펙 AC-2(h)**. SC-11 은 계약 7차 개정으로 **보류 → PASS** 로 바뀌었다(`04_qa_report_r4.md` §0).

---

## 1. 🔴 먼저 — **라운드 2 의 S-B·S-C 귀속을 정정한다. 절반은 내 도구였다**

server 가 보고한 근본 원인을 **내가 이미 가진 라운드 2 증거로 독립 검증했다**(I-25 — 남의 진단을 그대로 받아 적는 것은 검증이 아니다).

### 1.1 결정적 증거 — 내 로그 파일 크기

라운드 2 에서 두 서버 인스턴스가 멈췄을 때 내가 남긴 **원본 stdout 파일 크기**:

| 인스턴스 | 파일 | **바이트** |
|---|---|---|
| 첫 번째 (블록 3·4) | `scratchpad/live/server.log` | **4033** |
| 두 번째 (블록 5) | `scratchpad/live2/server.log` | **4100** |

**Python `subprocess` 익명 파이프 용량은 4096 B 다.**
4033 과 4100 — **둘 다 4 KiB 경계에 정확히 붙어 있다.**
(`evidence/B4/server-stdout.log` 2803 B · `evidence/B5/server-stdout.log` 2870 B 는 ANSI 이스케이프를
제거한 뒤라 더 작다. **파이프에 실제로 흘러간 바이트는 4033·4100 이다.**)

두 인스턴스가 **서로 다른 시나리오**(4봇 25초 / 관측자 1 + probe 1)였는데도 **같은 4 KiB 지점**에서
로그가 끊겼다. 시나리오와 무관하고 **누적 바이트와만 상관있다** — 파이프 포화 가설의 정확한 예측이다.

### 1.2 내가 라운드 2 에 적은 관측들이 전부 이 하나로 설명된다

| 라운드 2 관측 | 파이프 포화로 설명 |
|---|---|
| "멈춤 지점이 실행마다 **한 칸 달랐다**"(①은 `수신 태스크 종료` **뒤**, ③은 **앞**) | 파이프가 차는 지점은 **누적 바이트**가 정하므로 줄 단위로 달라진다. 내가 "두 멈춤의 위치가 한 칸 다르다"고 적은 것이 오히려 이 가설의 증거였다 |
| "CPU 누적 **0.25초** / RSS **16.6 MB** — 스핀이 아니라 정지" | server 실측 0.28초 / 15.7 MB 와 일치. **워커 12개 전부 `Wait`** |
| "`IDLE_TIMEOUT` 30초가 발동하지 않았다" | ping 타이머도 tokio 태스크다. 워커가 없으면 폴링되지 않는다 |
| "stdin `shutdown` 2/2 무효" | `with_graceful_shutdown` 퓨처도 폴링되지 않는다 |
| "**봇 2대 × 10초만 무사했다**" | 그 실행만 4 KiB 에 닿지 않았다 |
| "`수신 태스크 종료` 뒤 `세션 종료 제출` 누락" | `serve_session` 이 **그 로그 줄을 쓰다가** 멈춰 `submit.close()` 에 도달하지 못했다 |

**내가 "간헐적"이라고 적은 것(3회 중 2회)은 틀렸다.** 간헐이 아니라 **누적 로그량 4 KiB 초과라는 결정적 조건**이었다.
봇 2대 실행이 무사했던 것은 운이 아니라 **그 실행이 임계에 닿지 않았기 때문**이다.

### 1.3 그래서 귀속은 어떻게 되는가 — **양쪽 다 결함이었다**

**내 도구의 결함**: `tests/e2e/server_boot.py` 가 `stdout=subprocess.PIPE` 로 띄우고 **종료 후에만** `read()` 한다.
실행 중 아무도 드레인하지 않는다. **내가 서버를 멈추게 한 조건을 만들었다.**

**서버의 결함**: 그렇다고 해서 **로그 소비자가 느리다는 이유로 게임 서버 전체가 정지하는 것은 정당하지 않다.**
`init_tracing` 이 기본 writer(`std::io::stdout()`)로 **이벤트를 낸 스레드에서 동기 쓰기**를 하고 있었고,
그 스레드가 tokio 워커라 하나씩 잠겼다. 진단을 쓰는 행위가 진단 대상을 죽인 것이다.

**두 결함이 겹쳐야 멈춘다.** server 는 서버 쪽을 고쳤고(`logsink.rs` — 비블로킹 유계 큐, 드롭 시 계수),
**나는 §2 에서 내 쪽을 고친다.**

### 1.4 라운드 2 리포트의 판정은 어떻게 되는가

- **SC-14 · SC-76 의 FAIL 은 유지한다.** 원인이 무엇이든 **`SHIP_SPAWNED` 만 있고 `SHIP_DESPAWNED` 가 없는 함선 3척이
  DB 에 실재한다**(I-41 위반). 그 행들은 지워지지 않는다(절대 원칙 5). 라운드 3 에서 **새 tick 구간**으로 재판정한다.
- **S-B·S-C 를 "server 단독 결함"으로 적은 §5.1 의 귀속은 틀렸다.** 위 내용으로 정정한다.
- **S-A(`SET_SHIP_CONTROL` 미디스패치)는 그대로 server 단독 결함이다.** 파이프와 무관하며,
  `commands_received_total` 델타 0 / `UNKNOWN_COMMAND_TYPE` +200 은 로그와 아무 관계가 없다.

### 1.5 내가 놓친 것 — 기록

라운드 2 에서 나는 **하네스를 용의선상에서 너무 빨리 뺐다.** `tasklist` 로 봇 프로세스가 없고
`netstat` 로 ESTABLISHED 가 없다는 것을 확인하고 "qa 도구 쪽 가능성은 배제했다"고 적었는데,
그 두 검사는 **봇**만 봤다. **서버를 띄운 것도 내 도구**라는 사실은 검사하지 않았다.
`server_boot.py` 가 파이프를 붙잡고 있다는 것은 내 코드에 있었고 내가 읽을 수 있었다.

**§3.3 의 "도구가 틀렸을 때 빨간불이 켜지는가" 가 봇과 e2e 스크립트만 덮고 있고,
서버를 띄우는 드라이버 자체는 덮지 않는다.** 그것이 이번에 드러난 공백이다.

---

## 2. qa 도구 수정 2건 — **SC-85·SC-86 의 증거로 기록한다**

### 2.1 (a) `tests/e2e/server_boot.py` — stdout 드레인 스레드

`spawn()` 이 `Popen` 직후 **데몬 드레인 스레드**를 걸고 살아 있는 버퍼를 함께 반환하도록 고쳤다.
종료 시 `proc.stdout.read()` 대신 그 버퍼를 쓴다. 드레인이 마지막 줄까지 읽을 시간은
`_drain_settle(proc, wait_s=2.0)` 로 **유계**로 준다 — 무한 대기하면 **고치려던 결함
(소비자가 생산자를 세운다)으로 그대로 되돌아간다.**

함수 docstring 에 **왜** 필요한지를 남겼다(4033 B / 4100 B 실측 포함). 다음에 이 파일을 읽는 사람이
"스레드 하나 줄이자"고 생각하지 않도록.

`python -m py_compile` 통과. 동작 확인은 §3 의 실서버 실행에서 한다 — **긴 로그가 4 KiB 를 넘어
온전히 남는가**가 그 증거다.

> **리더 지시대로 구분 기준을 적어 둔다**: 다음 라운드에서 로그가 4 KiB 에서 끊기면
> **서버 정지가 아니라 로그 드롭**이다. `/healthz` 와 `/debug/stats` 로 가른다 —
> **정지면 둘 다 `000`, 드롭이면 정상 응답.**

### 2.2 (b) `tools/bots/src/ledger.rs` — 짝 게이트가 **틀렸는데 초록불이었다**

계약 §3.3 은 "도구가 틀렸을 때 **빨간불이 켜지는가**"를 묻는다. 이 항목은 **그 반대**였다.

**옛 판정**: `accepted_reply_pairing_holds()` = `accepted == replies_total`.
`replies_total` 은 `PING_REPLY` 수이고, `SET_SHIP_CONTROL` 은 계약상 `PING_REPLY` 를 내지 않는다.
따라서 이 게이트는 실제로는 **"수락된 명령이 하나도 없을 때만 통과"** 하는 검사였다.

| 시점 | `accepted` | `replies_total` | 게이트 |
|---|---|---|---|
| 라운드 2 (S-A 미수정, 조작 명령 전부 거부) | **0** | 0 | `0 == 0` → **조용히 초록** |
| 라운드 3 (S-A 수정 후) | 199 | 0 | **거짓 빨간불**, `all_ok=false`, exit 1 |

**라운드 1 에서 내가 경계면 비교 도구의 거짓 경보 39건을 잡은 것과 같은 계열이고, 방향이 반대다.**
그때는 총계를 안 믿고 행을 읽어서 잡았다. **이번 것은 내가 못 잡았다** — 서버가 고장나 있던 덕에
가려져 있었고, 서버가 고쳐지자 드러났다.

**수정**: `CommandRecord.expects_ping_reply` 를 송신 시점에 기록하고
(`on_sent` = `PING_SERVER`, 신설 `on_sent_no_reply` = `SET_SHIP_CONTROL`),
`accepted_expecting_reply` 카운터를 새로 둬 게이트를 `accepted_expecting_reply == replies_total` 로 바꿨다.

**회귀 테스트 3건을 넣었고, 그중 2건이 즉시 같은 버그의 두 번째 서식지를 잡았다**:
`finish()` 의 `missing_replies` 계산도 **응답을 내지 않는 명령을 "응답 누락"으로 세고 있었다**
(`ledger.rs` 의 같은 조건). 게이트만 고치고 이 줄을 두었더라면 **수치는 맞는데 게이트는 여전히
빨간불**이었을 것이다.

| 테스트 | 무엇을 고정하는가 |
|---|---|
| `accepted_commands_without_a_ping_reply_do_not_break_the_pairing_gate` | `SET_SHIP_CONTROL` 3건 수락 + `PING_REPLY` 0 → **통과해야 한다**(거짓 실패 방지) |
| `a_missing_ping_reply_for_an_accepted_ping_still_fails_the_gate` | `PING_SERVER` 2건 수락인데 응답 1건 → **여전히 실패해야 한다**. *이것이 없으면 "거짓 실패를 없앴다"와 "검사를 없앴다"가 구분되지 않는다* |
| `mixed_command_types_are_paired_by_what_each_type_actually_answers` | 실제 시나리오 e 모양(PING 2 + 조작 20) 혼재 시 `accepted=22 / accepted_expecting_reply=2 / replies=2` |

**결과**: `cd tools/bots && cargo test` → **52 passed / 0 failed / exit 0**(라운드 2 의 49 + 3),
`cargo fmt --all --check` exit 0, `cargo clippy --all-targets -- -D warnings` 경고 0.
(`evidence/R3-SC-85-bots.log`)

### 2.3 §3.3 의 공백 — 기록

계약 §3.3 의 고장 주입 9종은 **봇 계측과 e2e 스크립트**를 덮는다.
이번 두 건은 그 바깥이었다:

| 결함 | 어디에 있었나 | §3.3 이 덮는가 |
|---|---|---|
| (a) 드레인 없음 | **서버를 띄우는 드라이버** `server_boot.py` | ✗ — 주입 목록에 드라이버가 없다 |
| (b) 짝 게이트 | 봇 계측 `ledger.rs` | ✗ — 주입 9종은 **스냅샷** 계측만 덮는다. 명령/응답 회계는 `ledger_accounting.rs` 가 따로 덮고 있었는데, **그 파일에도 명령 타입별 기대 응답을 보는 케이스가 없었다** |

**다음 슬라이스의 계약에 반영할 것**(architect 판단 사항, 이번 판정에는 쓰지 않는다):
"QA 도구 자체 검증"의 범위에 **측정 대상을 기동·종료시키는 드라이버**와
**명령 타입별 응답 규약**을 넣는다.

---

## 3. S-A · S-B · S-C 재판정 — **셋 다 고쳐졌다** (qa 독립 재현)

서버 기동 `start_tick = 365692`. 봇 `run --scenario e --bots 4 --duration 25 --seed 4242`
(server 가 쓴 것과 **같은 시드·같은 구성**). 증거: `evidence/R3/`.

### 3.1 S-A — `SET_SHIP_CONTROL` 이 시뮬레이션에 도달한다

| 관측 | 라운드 2 | **라운드 3** |
|---|---|---|
| 봇 `COMMAND_RESULT` | 200 (accepted **0** / rejected **200**) | 200 (**accepted 200** / rejected **0**) |
| `rejected by reason` | `{"UNKNOWN_COMMAND_TYPE": 200}` | **없음 — 전 라벨 델타 0** |
| **`commands_received_total` 델타** | **0** | **+200** |
| `input_carried_forward_total` 델타 | 0 | **+1832** |
| **`ack_input_seq`** | 전부 `null` | **봇 4대 전부 `ack_last = 50`** (= 봇당 송신 50건) |
| 함선 이동 | 스폰 지점 고정 | **`path = 13,554.31 m` / `displacement = 13,423.81 m`** |
| 도달 최대 속도 | 0 | **`140.00019 m/s`** (= `max_speed_mps` 140, SC-16 의 상한) |
| 최대 원점 거리 | 2512 m(스폰 반경) | **5,696.44 m** |

**`commands_received_total` 델타 +200 이 결정적이다** — 라운드 2 에서 이 값이 0 이었던 것이
"큐에 넣으려 시도조차 되지 않았다"의 증거였고, 지금은 200건 전부가 시도되고 수락됐다.

server 의 실측(received +199, `probe fly` 로 `ack_last=Some(328)`, `path=2615.69 m`)과
**구성이 달라 수치는 다르지만 방향과 성질이 같다.** 내 실행은 4봇 25초라 봇당 50건 × 4 = 200건이다.

### 3.2 S-B — 세션이 닫힌다, 유령이 없다

| 관측 | 라운드 2 | **라운드 3** |
|---|---|---|
| `sessions_opened / closed` | 4 / **3** | **4 / 4** |
| `ws_connections`(봇 종료 후) | **1** (3분 이상 영구) | **0** |
| `ships_active / lingering` | 1 / 0 (유령 `ACTIVE`) | **0 / 4** (전부 정상 잔류) |
| 소켓 | **CLOSE_WAIT 4개 누적** | **CLOSE_WAIT 0개** (`TIME_WAIT` 만 — OS 정상 절차) |

### 3.3 S-C — 서버가 멈추지 않는다 + (a) 드레인이 동작한다

블록 3 재현(봇 4대 25초) + 블록 4 probe 5건을 **같은 인스턴스에서 연속** 수행한 뒤:

```
/healthz     → 200
/debug/stats → 200
```

라운드 2 라면 이 시점에 **둘 다 `000`** 이었다(4 KiB 초과 시점을 한참 지났다).
**드레인 스레드가 파이프를 비우고 있고 서버가 로그를 계속 쓰고 있다.**

> **리더가 요구한 구분 기준대로 확인했다**: 정지면 `/healthz`·`/debug/stats` 둘 다 `000`,
> 드롭이면 정상 응답. **둘 다 200 이므로 정지가 아니다.**

`shutdown` 판정(S-C 의 나머지 절반)은 §5 의 종료 단계에서 한다.

---

## 4. 블록 4 — 치트·프레이밍 (G-f: 부하와 분리 실행)

부하(§3.1)를 먼저 끝내고 그 뒤에 probe 를 돌렸다. probe 가 유발하는 강제 close 가
§3 의 관측을 오염시키지 않는다. 증거: `evidence/B4r3/`.

### 4.1 SC-23 · SC-66 — **PASS**

| probe | 시도 | 차단 | `rejected_by_reason` | 상태 변화 |
|---|---|---|---|---|
| `cheat-position` (위치 필드 주입) | 9 | **9** | **`{"MALFORMED_COMMAND": 9}`** | **`path = 0.00 m`** |
| `cheat-attitude` (현재 자세 주입) | 9 | **9** | **`{"MALFORMED_COMMAND": 9}`** | **`path = 0.00 m`** |

- **라운드 2 에서는 `UNKNOWN_COMMAND_TYPE` 이었다** — 차단은 됐지만 **계약이 지정한 층이 아니었다**.
  이제 `deny_unknown_fields` 가 실제로 일하는 자리에서 막힌다.
- **상태 변화 0 이 수치로 보인다**: `path = 0.00 m / displacement = 0.00 m / speed_max = 0.00 m/s`,
  `radius_max = 2512.47 m` = 스폰 반경 그대로. **주입된 위치가 반영됐다면 이 값들이 움직였을 것이다.**
- `close: code=1002 initiator=server` — 반복 프로토콜 위반 뒤 서버가 닫았다(정상 동작).
- **교차 검증**: `/debug/stats` 의 `commands_rejected_total{MALFORMED_COMMAND}` 델타 **+27**
  = 3 probe × 9 (position 9 + attitude 9 + range 9). 봇 계측과 서버 계측이 **일치**한다.

### 4.2 SC-67 — **PASS**

`probe cheat-seq` (`input_seq` 역행·반복):

| 관측 | 값 |
|---|---|
| 송신 / `COMMAND_RESULT` | 60 / 60 (**accepted 20 / rejected 40**) |
| `rejected_by_reason` | **`{"STALE_INPUT": 40}`** |
| **`ack_regressions`** | **0** — `ack_input_seq` 가 되돌아가지 않았다 |
| `ack_last` | `Some(97)` |
| **거부 중에도 이동이 끊기지 않았다** | `path = 144.14 m`, `speed_max = 44.79 m/s` |

**40건이 `STALE_INPUT` 으로 거부되는 동안 함선은 계속 움직였다** — 계약이 요구한
"거부와 동시에 이동이 끊기지 않음"이 한 실행 안에서 동시에 보인다.

### 4.3 SC-68 — **(e) PASS / (g) PASS / (f) 미실행**

- **(e)**: 라운드 2 §3.3 에서 확정. 스키마·Rust·C# 세 곳 모두 `SetShipControlPayload` 에 `ship_id` 필드가 **없다**.
  **어휘에 없어 시도 불가** — 코드 검사보다 강한 보장이다.
- **(g)** `probe pre-ready`: `SESSION_READY` **이전** 송신 10건 중 **결과 8건**
  (accepted 2 / **`{"RATE_LIMITED": 6}`**), **`missing_results = 2`**.
  세션이 서기 전에 보낸 것은 **판정에 들어가지 않았고**, 함선은 ready 이후에만 움직였다
  (`ready_tick` 이후 `path = 95.34 m`). **세션 이전 전송이 함선을 만들지도 움직이지도 않았다.**
- **(f)** 다른 actor 의 잔류 함선 가로채기 — **이번에도 실행하지 않았다**.
  `probe hijack-linger` 케이스가 CLI 목록에 없다(§3.1 의 치트 7종 중 미구현분).
  **qa 도구 미비이지 server 결함이 아니다.** 라운드 4 몫.

### 4.4 SC-24 — **미관측(qa 도구 미비)**. server 결함이 아니다

| 절 | 상태 |
|---|---|
| **(b)** 위치·속도·현재 자세가 필드 목록에 없다 | **관측됨** (SC-83 경계면 표 16행, 라운드 1) |
| **(c)** 범위 밖 값이 **클램프되지 않고** 거부된다 | **관측됨** — `cheat-range` 9건 전부 `MALFORMED_COMMAND`, `path = 0.00 m`. **클램프했다면 ±1000 으로 잘린 값이 적용돼 함선이 움직였을 것이다** |
| (c) 그 tick 에 **이월로 조작이 끊기지 않는다** | **다른 실행에서 관측됨**: `cheat-seq` 가 거부 40건 중 `path = 144 m`, 라이브 `input_carried_forward_total` +1832. **단 한 실행 안에서 "범위 초과 거부 + 이월 지속"을 같이 보이지는 못했다** |
| **(d)** `aim_*` 극단값을 계속 보내도 한 tick 회전량이 `turn_rate_max_deg_s × dt` 이하 | **관측 안 됨** — 현재 `probe cheat-range` 는 **유효 명령을 한 건도 보내지 않고**(`sent=0`) 주입 프레임만 보낸다 |

**판정을 PASS 로 올리지 않는다** — (d)의 관찰이 없다. **그러나 server FAIL 로도 적지 않는다**:
관찰이 없는 이유가 **내 probe 설계**이기 때문이다. 라운드 4 에서 `tools/bots` 를 확장한다
(① 유효 명령 → ② 범위 초과 → ③ `aim_*` 극단값 N tick, 스냅샷의 자세 차분으로 tick 당 회전량 계산).

### 4.5 🟠 **블록 4 발견 — `commands_rejected_total` 이 tick 층 거부를 세지 않는다**

두 독립 출처가 어긋난다(I-25 가 잡으라는 바로 그 어긋남):

| 거부 사유 | 봇이 받은 `COMMAND_RESULT` | **`/debug/stats` 델타** |
|---|---|---|
| `MALFORMED_COMMAND` (게이트웨이 층) | 27 | **+27** ✔ |
| **`STALE_INPUT`** (tick 층) | **40** | **0** ✗ |
| **`RATE_LIMITED`** (tick 층) | **6** | **0** ✗ |

**원인 (파일:라인)**: `stats.record_rejection(reason)` 호출은 레포 전체에서 **`ws.rs:709` 한 곳뿐**이고,
그것은 게이트웨이의 `send_rejection` 안이다. tick 이 만든 거부는
`simulation.rs:750`(`StaleInput`)·`:753`(`RateLimited`) 에서 `CommandResultPayload::rejected` 로 생성돼
`runtime.rs:551` 의 outbound 루프를 그냥 지나간다 — **그 루프는 `record_message_enqueued` 만 부르고
거부 사유를 세지 않는다.**

**영향 — SC-33 에 직접 걸린다.** 계약 SC-33 은 `commands_rejected_total` 이 6 → **8라벨**로 늘고
(`RATE_LIMITED`·`STALE_INPUT` 추가) **"존재만으로 PASS 주지 않는다 — 델타를 확인한다"** 고 못박았다.
**늘어난 두 라벨이 구조적으로 영원히 0 이다.**

**담당: server.** 요청: tick 층 거부도 `record_rejection` 을 타게 한다
(`runtime.rs` 의 outbound 루프에서 `ServerMessage::CommandResult` 의 `reason_code` 를 보고 세면 된다).
**그 전까지 SC-33 은 PASS 를 줄 수 없다.**

---

---

## 5. 이어받기 — 세 번째 qa (2026-09-21)

직전 qa 가 §4 까지 쓰고 세션이 끝났다. 나는 §1~§4 를 파일로만 이어받았고 **그 판정을 재실행하지 않았다**
(SC-10·12 는 라운드 2 PASS, SC-23·66·67 은 §4 PASS 그대로). 이 절부터가 내 작업이다.

리포트 안에서 스펙 AC-2 의 두 (h) 를 **AC-2(h-log)**(259행, 로그 소비자가 느려도…)와
**AC-2(h-dir)**(260행, 데이터 디렉토리 해석)로 구분해 부른다(§5.5 계약 외 발견 1).

### 5.1 단계 A-1 — 봇 게이트 3단언 (스펙 §5.1a, architect R3 판정 2)

**확인 결과(수정 전)**: architect 가 지정한 3단언 중 **둘째(`ping_replies == 수락된 PING_SERVER`)만 있었다.**

| 단언 | 수정 전 | 수정 후 |
|---|---|---|
| `results_total == accepted + rejected` | **없음** (`one_to_one` 은 `sent == results` 이고 분류를 보지 않는다) | `LedgerSummary::results_partition_holds()` |
| `replies_total == accepted_expecting_reply` | 있음 (`ledger.rs` `accepted_reply_pairing_holds`) | 그대로 |
| **`accepted > 0`** | **없음** — 입력이 전부 0 이면 all_ok 가 그대로 초록 | `accepted_path_exercised()`, 셋을 묶은 `command_reply_gates_hold()` |

`report.rs` 의 `Gates` 에 `results_partition`·`accepted_exercised` 를 추가하고 **`all_ok` 에 둘 다 넣었다.**
`main.rs` 의 `GATES` 줄이 `(ping pairs N)`·`(accepted N)` 을 함께 찍는다 — 초록불 옆에 **그 초록불이 몇 건을 보고 켜졌는지**가 보이게.
`CommandRecord.expects_ping_reply` 주석에 **스펙 §5.1a 가 정본**이라는 참조를 남겼다(architect 판정 2 의 "코드에 주석" 지시).

**고장 주입 테스트 5건 추가** (`tools/bots/tests/ledger_accounting.rs`):

| 테스트 | 고정하는 것 |
|---|---|
| `an_all_zero_ledger_fails_the_command_reply_gates` | **입력 0**: 세 항등식(1:1·분류·짝)이 자명하게 true 임을 먼저 고정하고, 그래도 3단언 게이트는 false |
| `round2_shape_every_control_rejected_fails_the_command_reply_gates` | **라운드 2 의 실제 모양**(조작 200 건 전부 `UNKNOWN_COMMAND_TYPE`): 1:1 은 정당하게 성립, 게이트는 실패 |
| `an_unknown_status_breaks_the_results_partition` | 분류 항등식이 **무언가를 검사하는가** — 닫힌 집합 밖 `status` 1건이면 false |
| `a_run_with_accepted_commands_passes_all_three_assertions` | 거짓 실패 방지 — PING 1 + 조작 5 수락 + `STALE_INPUT` 1 이면 통과 |
| `a_report_with_ready_sessions_but_nothing_accepted_is_not_all_ok` | **리포트 층까지 전달되는가** — 세션 2개가 섰는데 명령 0 이면 `all_ok=false` |

**RED 증거**(`evidence/R3-A/A1-fault-injection.log`) — 가드를 지우면 실제로 빨간불이 켜지는가:

| 주입 | 결과 |
|---|---|
| ① `accepted_path_exercised()` 를 `true` 로 | **3 failed** / 20 passed (위 표 1·2·5행) |
| ② `report.rs` 의 `all_ok` 에서 `accepted_exercised` 제거 | **1 failed** / 22 passed (5행 — 게이트 함수만 고치고 리포트가 안 부르는 경우를 잡는다) |

두 주입 모두 원본 복원을 `grep -c FAULT-INJECTED` = 0 으로 확인했다. 통과: `cargo test --test ledger_accounting` → **23 passed / 0 failed**.
전체 회귀(`cargo test`·fmt·clippy)는 A-4 수정과 함께 §5.4 에 적는다.

### 5.2 단계 A-2 — 카운터 항등식 규율을 계약에 문장으로

`02_sprint_contract.md` **§7a 신설**(§7 바로 뒤) + **§9 6차 개정** 행. architect 권고 문장을 그대로 싣고,
근거 사례 4건(`world_full` / `SET_SHIP_CONTROL` 154개 초록·도달 0 / 봇 짝 게이트 `0 == 0` / 벽시계 flaky)을
"무엇이 초록이었나 / 실제로 일어나지 않은 것 / 고친 짝" 표로 붙였다. **SC 항목 문구와 판정 기준은 바꾸지 않았다** —
§0.3 검사 건수 원칙과 §7 M-17 을 게이트 자신에게 확장한 것이다.

### 5.3 단계 A-3 — AC-2(h-log) 로그 파이프 e2e (구현만. 실행은 단계 B)

**새 파일 2개** (qa 소유 `tests/e2e/`):

| 파일 | 역할 |
|---|---|
| `tests/e2e/log_pipe_backpressure.py` | `run`: 서버를 **아무도 읽지 않는 stdout 파이프**로 띄우고 채운 뒤 응답성·종료를 본다 / `selftest`: 서버 없이 판정 함수와 ① 검출기를 검사 |
| `tests/e2e/make_red_logsink_binary.py` | ② RED 용 **싱크를 끈 바이너리**를 레포 밖(scratchpad)에서 빌드. `server/` 는 건드리지 않는다 |

**드레인 경로와 미드레인 경로의 분리**: `server_boot.py`(평소 기동, 드레인 스레드 있음)는 그대로 두고,
미드레인은 이 스크립트에만 있다. 이 스크립트의 stdout 은 **`LateReader` 가 `resume()` 되기 전까지 한 바이트도 읽히지 않는다.**

**조건 셋의 구현**:

| 조건 | 구현 |
|---|---|
| ① 파이프가 실제로 찼다 | **`PeekNamedPipe` 로 읽지 않고** 누적 바이트를 100 ms 마다 표본 → `max_avail ≥ 용량`. 용량은 `GetNamedPipeInfo` 실측(**in/out 4096 B** — selftest 에서 확인). 또는 `--drop-probe` 로 **싱크의 "N줄을 버렸다 (누적 M)" 통지의 M > 0**. 둘 다 아니면 판정하지 않고 **종료 코드 4(관측 조건 미발생)** — 연결 횟수는 수단이다 |
| ② RED 선행 | `--exe` 로 바이너리를 받고 `--expect fail` 로 돈다. RED 바이너리는 `make_red_logsink_binary.py` 가 **현재 소스를 복사해 `init_tracing` 의 한 줄만 바꿔**(`non_blocking_stdout()` → `None`, 즉 서버 자신의 **동기 stdout 폴백** = 수정 전 동작) 빌드한다. 패치 지점이 정확히 1곳이 아니면 exit 1. 복사 전후 `server/` 트리 해시를 찍어 **RED 와 GREEN 이 그 한 줄 말고는 같은 소스**임을 보인다 |
| ③ stderr | **파일로** 돌린다(`server-stderr.log`). 드레인하지 않는 것은 stdout 하나뿐이다 |

**단계**: 기동 → `/healthz` 200 대기 → `bots run --scenario b` 로 채우기(파이프가 찰 때까지 최대 3라운드, 봇은 시간을 묶는다 — 서버가 멎으면 봇도 멎는다)
→ **찬 상태에서** `/healthz`·`/debug/stats` 10회 + **새 WebSocket 이 `SESSION_READY` 까지 가는가**(라운드 2 의 첫 증상)
→ **여전히 아무도 읽지 않는 채로** stdin `shutdown` → 20초 안에 exit 0 이어야 한다.
안 내려가면 그때 파이프를 읽기 시작해 **드레인이 서버를 풀어 주는지** 본다 — 풀리면 원인이 파이프였다는 인과 증거이고 **하드 킬 없이** 끝난다.
그래도 안 내려가면 하드 킬하고 그 사실을 `HARD_KILL` 로 기록한다(리더 지시: 그 자체가 판정 대상).

**`--drop-probe` 가 따로 있는 이유**: 싱크는 버린 줄 통지를 **소비자가 다시 읽기 시작한 뒤에야** 스트림에 쓴다
(`logsink.rs` `drain_loop` — 막힌 `write_all` 다음에 통지). 그래서 "멈춘 소비자 그대로 종료"(기본 모드)와
"버린 줄 수 수집"(drop-probe)은 **한 실행에서 동시에 볼 수 없다.** 기본 모드가 종료 성질을, drop-probe 가 ① 의 두 번째 근거를 맡는다.
또 싱크 큐가 **4096 줄**이라 기본 로그 필터(연결당 ~3줄)로는 버림이 거의 안 생긴다 — drop-probe 는 `--rust-log` 로 로그량을 키워 돈다.

**selftest 결과** (`evidence/R3-A/A3-selftest.log`) — **13/13 ok, exit 0**:
- 판정 함수 11건: 입력 전부 0 → PASS 아님 / 파이프 안 참 → `CONDITION_NOT_MET` / 버린 줄 0 → 조건 아님 / health 표본 2건(최소 3) → FAIL / `000` 1회 → FAIL / `SESSION_READY` 미도달 → FAIL / 하드 킬 → FAIL / rc=1 → FAIL / **기본 모드에서 드레인 뒤에야 내려감 → FAIL** / 버린 줄 17 → 조건 성립 / 정상 → PASS
- ① 검출기 2건(실제 파이프): **512 B 쓴 파이프 → peek 512 (< 4096, 참 아님)** / **64 KiB 쓰다 막힌 파이프 → peek 65536 (≥ 4096)**.
  막힌 쓰기의 데이터도 `TotalBytesAvail` 에 잡힌다 — 그래서 막힌 서버는 peek ≥ 용량으로 보인다.

**RED 바이너리 사전 빌드(예열)**: 의존성 컴파일 시간을 단계 B 에서 빼기 위해 현재 소스로 한 번 빌드했다
(scratchpad `red-logsink/`, 트리 해시 `918a28e200be8aa0`, exit 0). **이 바이너리는 판정에 쓰지 않는다** —
server 수정이 끝난 뒤 소스를 다시 복사해 증분 빌드한 것을 쓴다.

**사실 확인 한 건 (계약 외 발견 2 로 적는다)**: architect 조건 ③ 과 리더 지시가 "`logsink`의 버린 줄 통지가
**stderr** 로 나간다"고 적었는데, 현재 코드는 **stdout** 에 쓴다(`server/bins/game-server/src/logsink.rs` `drain_loop` 의 `writeln!(out, …)`, `out = io::stdout().lock()`).
③ 의 결론(stderr 도 파일로)은 여전히 옳다 — 동기 폴백·패닉 메시지가 stderr 로 간다. 그러나 ① 의 두 번째 근거를 **어디서 읽는가**는 stdout 이다.

### 5.4 단계 A-4 — SC-24 probe 확장: `probe cheat-range-turn` (구현만. 실행은 단계 B)

§4.4 의 설계 미비(유효 명령 0건이라 (d) 관측 불가, (c) 의 거부와 이월이 두 실행에 나뉨)를 **한 연결 안의 4단계**로 고쳤다:

| 단계 | 보내는 것 | 겨냥한 관찰 |
|---|---|---|
| ① 1.0 s | 유효 `SET_SHIP_CONTROL` 전방 추력 1000, 20 Hz | 아직 최고 속도 전(0→최고 4 s) — 이월이 끊기면 **속도가 줄어드는 것이 보이는** 구간 |
| ② N건 | `thrust-above-range.json` 원문(추력 1001, 목표 자세 Y90)을 **유효 명령 대신 같은 20 Hz 로**. `input_seq` 를 **다음 번호로 바꿔** 넣는다 | 거부 사유 = `MALFORMED_COMMAND` / **클램프 흔적 없음 = 주입한 번호가 `ack_input_seq` 로 한 번도 안 나온다** / 거부 구간 스냅샷 쌍에서 **속도 감소 0 + 이동 > 0** (이월) |
| ②' 10건 | 유효 재개 (이월 창 10 tick 안) | — |
| ③ 3 × 3.0 s | 추력 0, 목표 자세를 성분 최댓값의 **180° 반대편**(Y 축, `(0,1e6,0,0)` ↔ `(0,0,0,1e6)`)으로 번갈아 | 스냅샷 쌍의 **쿼터니언 차분 / Δtick ≤ `turn_rate_max_deg_s / tick_hz`**, 매 스냅샷 **`|ω_aim|` ≤ 한도** |

**조건이 실제로 생겼는지를 함께 단언한다**(§7a): (c) 는 주입이 **실제로 전부 `MALFORMED_COMMAND` 로 거부됐고** 그 구간을 덮는 쌍이 **1개 이상**이어야,
(d) 는 **리미터가 실제로 걸린 쌍(한도의 90 % 이상)이 1개 이상**이어야 판정한다. 목표가 가까워 한도에 닿지 않았다면 "넘지 않았다"는 아무것도 뜻하지 않는다.
주입 구간이 이월 창(`carry_forward_max_ticks`) 이상이면 **probe 설계 오류**로 실패시킨다(그때는 이월 만료가 정당해진다).
Y 축 선회를 고른 이유: 수평 선회는 오토레벨(롤)이 끼어들 이유가 없어 쿼터니언 차분이 **조준 회전만** 잰다. `|ω_roll|` 최댓값도 함께 찍어 이 가정을 실측으로 확인한다.

**해상도 한계(판정에 함께 적는다)**: 스냅샷이 2 tick 마다라 쌍 하나는 2 tick 평균이다. 한 tick 에 한도의 2배, 다음 tick 에 0 인 경우는 쌍으로 안 보인다 —
그래서 서버가 그 tick 에 쓴 **`|ω_aim|` 필드**도 매 스냅샷 대조한다(둘 다 통과해야 한다).

**한도 값은 `data/` 에서 읽지 않고 인자로 받는다**(`--turn-rate-max-deg-s`, `--carry-forward-max-ticks` 필수) — 계약 §0.7 대로 측정 시점 값을 증거에 찍는다.
현재 `data/`: `scout-s01.json` `turn_rate_max_deg_s = 75.0`, `sync-tuning.json` `input.carry_forward_max_ticks = 10`, 위반 예산 `ws.rs:54` `VIOLATION_BUDGET = 8` / 10 s → 주입 N 은 1~6 으로 묶었다.

**구현** (qa 소유 `tools/bots/`): `src/range_turn.rs`(순수 분석, IO 없음) 신설 · `snapshot.rs` 에 자기 함선 시계열 `OwnSample`(tick·위치·속도·쿼터니언·ω·ack) ·
`ledger.rs` 에 표지(`mark`)와 `result_of` · `wire.rs` `with_aim` · `conn.rs` `run_range_turn` · `scenario.rs` `CheatRangeTurn` · `main.rs` 판정 출력(성립 시 exit 0, 아니면 1).

**고장 주입 테스트 11건** (`tests/range_turn_faults.rs`) — 정상 1 / 쿼터니언 각 정확도·부호 불변 1 / **입력 전부 0 → 둘 다 불성립** / **리미터에 안 닿은 선회 → 불성립(넘지 않았어도)** /
한 쌍 한도 초과 / `|ω_aim|` 75.002 / 거부 중 속도 감소 / 주입 번호가 ack 됨(클램프 흔적) / 주입이 수락됨 / 주입 결과 없음 / 주입 구간 > 이월 창.

**분석기 자체의 RED**(`evidence/R3-A/A4-fault-injection.log`): 포화 단언 제거 → 1 failed(리미터 미도달 테스트) / 속도 감소 계수 제거 → 1 failed / 클램프 흔적 계수 제거 → 1 failed. 셋 다 복원 확인.

**`tools/bots` 전체 회귀**(`evidence/R3-A/A-bots-regression.log`): `cargo test` **68 passed / 0 failed**
(ledger 23 · live_loopback 7 · range_turn 11 · snapshot 9 · token 9 · wire 9), `cargo fmt --all --check` exit 0, `cargo clippy --all-targets -- -D warnings` 경고 0.
라운드 3 앞부분의 52 → 68 (+5 게이트, +11 range_turn).

### 5.5 계약 외 발견

1. **스펙 AC-2 에 (h) 가 둘이다** — `docs/specs/p1-01-ship-movement.md` 259행 "(h) 로그 소비자가 느려도…"(architect R3 신설)와 260행 기존 "(h) 데이터 디렉토리 해석".
   새 항목이 이미 쓰인 글자를 재사용했다. **스펙은 architect 소유라 고치지 않았다.** 이 리포트 안에서는 **AC-2(h-log) / AC-2(h-dir)** 로 구분한다. 권고: 새 항목을 (i) 로.
2. **버린 줄 통지의 경로** — architect 조건 ③ 과 리더 지시는 통지가 stderr 로 나간다고 적었으나, 현재 `logsink.rs` `drain_loop` 는 **stdout** 에 쓴다(§5.3 끝). ③ 의 결론(stderr 도 파일로)은 유지된다. 판정 영향 없음 — 기록.
3. **싱크의 버린 줄 수가 `/debug/stats` 에 없다** — 유일한 관측 경로가 로그 스트림 안의 통지이고, 그 통지는 소비자가 다시 읽어야만 나온다. 그래서 "멈춘 소비자 그대로 종료"와 "버린 줄 수"를 한 실행에서 동시에 볼 수 없다(§5.3). 카운터가 있으면 한 실행으로 합칠 수 있다. **server 판단 사항 — 요구가 아니라 기록.**

### 5.6 대기 중 추가 — SC-11 (3) 독립 계산 도구 (단계 B 블록 5 용)

**발견: qa 도구에 적분기가 없었다.** SC-11 (3) 은 "재개 직후 스냅샷 = 끊기기 직전 스냅샷에서 N tick 적분한 값(양자화 정수로 같다, **qa 독립 계산**)"인데,
라운드 2 의 SC-11 FAIL(차단)은 재개 자체가 서버 정지로 막혀 이 계산까지 가지 못했고, 계산 도구도 없었다.

**만든 것** (전부 qa 소유):

| 파일 | 역할 |
|---|---|
| `tests/e2e/resume_predict/` (`ResumePredict.csproj` + `Program.cs`) | **client 의 C# `Starfall.Sim`**(ADR-0010 §2 를 줄 단위로 옮긴 순수 함수)을 레포 소스 그대로 컴파일해, T0 와이어 상태에서 [남은 이월 → 휴면 입력(ADR-0011 §6.1)] 으로 N tick 적분. 서버 코드가 아니므로 I-25 를 지킨다. client 소스를 복사·수정하지 않는다 |
| `tests/e2e/resume_check.py` | `bots resume` 결과 × 하네스: T1 14필드 대조 + 관측자 행(잔류 중 p·v) 대조 + **음성 대조** |
| `bots resume` (신규 하위 명령) | 관측자를 붙인 채 ① 추력+롤+**보조 끔** 1 s → **입력 정지** 1.5 s(이월 창 0.5 s 를 넘겨 T0 가 휴면 구간에 있게) → 닫기 → 6 s 뒤 같은 `actor_id` 로 재접속(잔류 창 30 s 안) → 1 s 뒤 `input_seq=1` 1건(AC-3(e2)). 보조를 끈 이유: 휴면으로 넘어가는 순간 오토레벨(5단계)이 돌기 시작해 **잔류 구간이 5단계를 반드시 탄다** — 독립 계산이 갈리는 자리(ADR-0011 §6.1) |

**🔴 계약 자체의 문제 (architect 통지 대상, "계약이 틀렸다고 본 항목")** — SC-11 (3) 의 "**양자화 정수로 같다, 허용 오차 없음**"은
**스냅샷에서 출발하는 한 성립할 수 없다.** 서버 상태는 `f64` 이고 양자화는 스냅샷을 만들 때 한 번만 일어난다
(`server/crates/sim/src/world/ship.rs:6` 주석, ADR-0009 §2). T0 스냅샷은 서버 상태를 반올림한 것이므로 거기서 출발한 계산은
서버의 `f64` 계속 적분과 비트가 같을 수 없다. **실측(서버 없이, 하네스 자체)**: T0 정수를 ±0.5 양자 흔든 시작점 32개로 같은 적분을 돌리면
140 tick 뒤 **위치가 4~5 mm 벌어진다**(`evidence/R3-A/SC11-harness-selftest.json` 의 `envelope_width_at_t1`: x 4 / y 4 / z 5 mm, 방향 x·y 2 micro).
즉 **같은 T0 스냅샷과 양립하는 서버 상태들이 서로 다른 T1 정수를 낸다** — 정직한 서버도 "정확히 같다"를 보장할 수 없다.

**그래서 두 판정을 나란히 적는다**(허용 오차를 **고르지 않고** 계약의 전제에서 **유도한다**):
1. **계약 문구 그대로**: 14필드 완전 일치 여부 — 일치하지 않아도 **server FAIL 로 적지 않는다**(§0.5 와 같은 처리: 계약 설계 문제 → architect 통지).
2. **양자화 봉투**: T0 를 ±0.5 양자 흔든 64개 시작점의 [min, max] 안에 T1 과 **잔류 중 관측자 행 전부**가 드는가.

**도구 자체의 RED**: 휴면 모델을 일부러 틀리게(`flight_assist=false`) 하면 봉투를 벗어나는가 — 합성 자체 검사에서 **관측 70점 전부 + T1 5필드가 봉투 밖**(정상 모델은 70/70 안, 14/14 일치).
틀린 모델이 봉투 밖으로 나가지 않으면 이 대조는 아무것도 검사하지 않는 것이다(§7a).

회귀: `tools/bots` `cargo test` 68/0, fmt·clippy 0 (resume 추가 후 재실행, `evidence/R3-A/A-bots-regression.log`). 하네스 `dotnet build -c Release` 경고 0 / 오류 0.

### 5.7 대기 중 추가 — 단계 B 판정 도구 2건

| 도구 | 항목 | 자체 검증 |
|---|---|---|
| `tests/e2e/stats_delta.py` | SC-33 — 8라벨 전부의 델타 + 봇 관측 거부 수와 **라벨별 일치** + `snapshots_sent_total` ≡ `messages_written_total{WORLD_SNAPSHOT}` | **라운드 3 §4.5 의 증거(`evidence/B4r3/stats-*.json`)에 돌리면 빨간불**: `MALFORMED_COMMAND` 27 = 27 ✔, `STALE_INPUT` 40 vs **0** ✗, `RATE_LIMITED` 6 vs **0** ✗, exit 1 (`evidence/R3-A/SC33-tool-red-on-B4r3.json`). 즉 이 도구는 §4.5 결함을 **잡는다** — 고쳐진 서버에서 초록이면 그 초록은 의미가 있다 |
| `tests/e2e/ship_events.py shutdown` | SC-13 — 종료 디스폰을 (a) 잔류(원인이 여러 tick 전) / (b) 활성(원인이 **같은 tick**, `sequence` 작음)으로 분리. **둘 다 ≥ 1 이어야 PASS** | 현재 DB 전 구간에 돌려 배관 확인(읽기만): `SERVER_SHUTDOWN` 6건 전부 (a), **(b) 0 → FAIL** — 과거 어느 종료에도 활성 함선이 없었다는 뜻이고, 그래서 블록 5 는 **활성 1척 + 잔류 1척을 둔 채** 종료해야 한다 |

---

## 6. 단계 B — 실서버 측정 (server 신호 2026-09-21 23:40 경 수신 뒤)

### 6.0 빌드와 환경 (측정 전에 전부 끝냄 — G-c)

`evidence/R3-B/B0-builds.log`:

| 항목 | 값 |
|---|---|
| GREEN `starfall-game-server.exe` | `cargo build -p starfall-game-server` (dev) — **재컴파일됨**(server 의 마지막 `cargo test` 가 bin 을 갱신하지 않았다), mtime 23:41:31, sha256[:16] `16555876b5b5b00e` |
| RED (싱크 끔) | 레포 밖 복사본 + `init_tracing` 1줄 패치, sha256[:16] `fd61fadd88bc2fbc` |
| 소스 동일성 | 레포 `server/` 트리 해시 **`1d46f7aca9140b8d`** = RED 복사본(패치 전) 해시 = 모든 빌드 뒤 재확인 해시. **RED 와 GREEN 은 그 1줄 말고 같은 소스다** |
| SV-1 테스트 바이너리 | `cargo test -p starfall-gateway --test ws_integration --no-run` 선빌드 |
| 환경 | Unity.exe **0개**, cargo/rustc 프로세스 0, docker 실행 컨테이너 7(starfall 2 + livingfeed 5), 빌드 프로필 dev |

### 6.1 AC-2(h-log) — **PASS** (RED → GREEN, 조건 ①②③ 전부)

`evidence/R3-B/AC2hlog/{red,green,green-drop,green-drop2,green-drop3}/verdict.json` + `*-run.log`. 같은 스크립트·같은 채우기(`bots run --scenario b`), 다른 것은 바이너리뿐.

| 실행 | 바이너리 | ① 파이프 참 (peek 최대 / 용량 4096) | 찬 상태 `/healthz`·`/debug/stats` | 찬 상태 새 WS `SESSION_READY` | `shutdown` | 판정 |
|---|---|---|---|---|---|---|
| **red** | 싱크 끔 | **4256 B** (4.1 s 에 참) | **0/10 · 0/10** (전부 `000`) | **미도달**(20 s 시간 초과) | 20 s 안에 **안 내려감** → 드레인 재개 **0.06 s 뒤 exit 0** | **FAIL (기대대로)** |
| **green** | 현재 | **4130 B** (5.4 s 에 참, 끝까지 유지 — 아무도 안 읽음) | **10/10 · 10/10** | **도달 0.58 s** | **아무도 안 읽는 채로 2.06 s 뒤 exit 0** | **PASS** |
| green-drop3 | 현재, `RUST_LOG=trace`, 1300 연결 | 4486 B **+ 싱크가 버린 줄 누적 4538** | 10/10 · 10/10 | 도달 0.56 s | (드레인 재개 뒤) exit 0 | **PASS** |

**RED 가 인과까지 보였다**: RED 는 `shutdown` 이 20초 동안 무효였다가 **파이프를 읽기 시작한 지 0.06초 만에** 정상 종료(exit 0)했다 — 멈춤의 원인이 파이프였다는 직접 증거이고, **하드 킬 없이** 끝났다.
라운드 2 의 증상(HTTP 무응답, TCP 는 붙는데 `SESSION_READY` 없음, `shutdown` 무효) **셋 다** RED 에서 재현됐고 GREEN 에서 **셋 다** 사라졌다.
RED 의 stdout 에는 ANSI 이스케이프가 섞여 있다 — 수정 전 기본 writer 의 동작 그대로다(수정본은 `with_ansi(false)`).

**조건 ①의 두 번째 근거(버린 줄 > 0)를 얻기까지 — 기록**: 싱크 큐가 **4096 줄**이라 기본 필터·20 연결(green)로는 버림이 안 생긴다.
`trace` + 20 연결(green-drop) → 250줄, `trace` + 500 연결(green-drop2) → **3340줄: 큐가 전부 흡수, 버림 0**(파이프가 90초 막혀 있는 동안 ~850 KB 를 들고 버텼다).
`trace` + **1300 연결**(green-drop3)에서야 **4538줄 버림** — 통지는 **stdout** 에 `[로그 싱크] 소비자가 느려 4538줄을 버렸다 (누적 4538) — 서버는 계속 돈다` 로 찍혔다(§5.5-2 의 확인).
**이것으로 `logsink` 의 "버리고 센다" 경로가 처음으로 실제로 탔다**(M-17, §9 표). 채우기 봇 1300 연결은 **새 3단언 게이트 전부 초록**(`accepted 1300`, `ping pairs 1300`).

**③ stderr**: 다섯 실행 모두 `server-stderr.log` **0 B** — 교착이 옮겨 갈 자리는 없었고 버린 줄 통지도 stderr 로 가지 않았다.

**기록 — strict GREEN 의 마지막 로그 줄은 없다**: 소비자가 멈춘 채 종료하면 큐에 남은 줄(`정상 종료 — 마지막 tick 까지 커밋 완료`)은 `FlushGuard` 의 2초 기한 뒤 버려진다(`green/server-stdout.log` 21줄 3980 B). **설계대로다** — 로그를 잃고 서버는 선다. 그래서 종료 성공의 증거는 로그 줄이 아니라 **exit code 0 과 2.06 s** 다.
DB 구간 기록: 이 다섯 실행의 마지막 tick 은 396920 / 399040 / 403433 등(로그). 블록 5 의 tick 구간은 이후 인스턴스의 `start_tick` 부터다.

### 6.2 실서버 인스턴스 (블록 4 잔여·SC-33·블록 5 공용)

`tests/e2e/server_boot.py serve`(드레인 경로) — **`start_tick = 403435`**, 23:5x 기동. 이 인스턴스의 DB 구간은 `tick ≥ 403435` 이다(G-a 방식).
stdout 은 종료 시 `evidence/R3-B/server-stdout.log` 로 남는다.

### 6.3 §4.5 재판정 + SC-33 — **PASS**

치트 probe 5종을 전후 스냅 사이에 돌렸다(부하 없음, G-f). `evidence/R3-B/B4/{probe-*.log, stats-before/after.json, SC-33-delta.json}`.
봇이 받은 `COMMAND_RESULT` 거부 수(독립 출처)와 서버 `commands_rejected_total` 델타를 **8라벨 전부** 대조했다(`tests/e2e/stats_delta.py diff … --expect`, exit 0, fails 0):

| 라벨 | 층 | 봇이 받은 거부 | **서버 델타** | 유발 probe |
|---|---|---|---|---|
| `MALFORMED_COMMAND` | 게이트웨이 | 13 | **+13** ✔ | cheat-position 9 + cheat-range-turn 4 |
| **`STALE_INPUT`** | **tick** | 40 | **+40** ✔ (라운드 3 §4.5: **0**) | cheat-seq |
| **`RATE_LIMITED`** | **tick** | 6 | **+6** ✔ (§4.5: **0**) | pre-ready |
| **`DUPLICATE_COMMAND_ID`** | **tick** | 1 | **+1** ✔ | duplicate — **server 가 추가로 찾은 같은 맹점**, 실서버에서도 확인 |
| `UNKNOWN_COMMAND_TYPE` | 게이트웨이 | 0 | 0 | **이 실행에서 안 탐**(대조 아님, §7a) |
| `SCHEMA_VERSION_UNSUPPORTED` | 게이트웨이 | 0 | 0 | 안 탐 |
| `SERVER_BUSY` | — | 0 | 0 | 안 탐 |
| `TOO_MANY_IN_FLIGHT` | — | 0 | 0 | 안 탐 — 계약상 **구조적 미도달**(tick 상한이 먼저, 3차 개정) |

**라벨 수 8 = 계약.** 움직인 4라벨 전부 봇과 일치하고, **기대에 없는 라벨이 움직인 것은 없다**(봇이 못 본 거부 0).
tick 층 3라벨(`STALE_INPUT`·`RATE_LIMITED`·`DUPLICATE_COMMAND_ID`)이 **실제로 0 에서 움직였다** — §4.5 결함이 실서버에서 닫혔다.

**신규 7키** (같은 구간 델타):

| 키 | 델타 | |
|---|---|---|
| `snapshots_sent_total` | +230 | = `messages_written_total{WORLD_SNAPSHOT}` **+230** ✔ (절대값도 같다) |
| `snapshot_bytes_total` | +507,423 | |
| `send_queue_bytes` / `_max` | 게이지 0(휴지) / max **+16,384** | 게이지는 쉬는 순간 0 이 정상. max 가 움직였다 |
| `input_superseded_total` | **+1** | cheat-seq 의 같은 tick 두 후보 |
| `input_carried_forward_total` | **+129** | |
| `aim_degenerate_total` | **0 → 11** (별도 한 건) | 아래 |

**`aim_degenerate_total` 을 움직였다 — 라운드 2 에서 델타 0 이던 세 키의 마지막**: 봇에 퇴화 쿼터니언 행동이 없어, 측정 중 빌드 금지(G-c)를 지키려고
**표준 라이브러리 WebSocket 송신기 `tests/e2e/raw_ws_send.py`**(qa 소유, 신규)로 `aim_* = 0` 인 `SET_SHIP_CONTROL` 1건을 보냈다(`evidence/R3-B/B4/degenerate-aim.log`).
결과: **`ACCEPTED`**(거부가 아니다 — I-39), `aim_degenerate_total` **+11**, `input_carried_forward_total` **+10**.
**11 = 적용 1 tick + 이월 10 tick** — 퇴화 입력이 이월되는 동안 매 tick 다시 세진다. 이월 창(`carry_forward_max_ticks = 10`)과 정확히 맞는다.
**`messages_enqueued/written_total` 4라벨** 모두 존재·델타(SESSION_READY +5, COMMAND_RESULT +262, PING_REPLY +1, WORLD_SNAPSHOT +230). `ships_active`/`ships_lingering` 게이지 0 / 5.

→ **SC-33 PASS.** 라운드 2 의 "델타 미관측 3키"(superseded·carried·degenerate)와 라운드 3 의 "tick 층 라벨 영구 0" 이 둘 다 닫혔다.
**§4.5 → 해소(server 수정 확인).**

### 6.4 SV-1 `graceful_client_close_is_prompt` — **FAIL(귀속: 테스트의 관측 설계) → 수정 확인. (e) 해석 1건은 architect 통지**

**판정 기록(architect R3 판정 1 지시대로)**: 라운드 3 의 간헐 실패는 **FAIL** 로 적는다. 귀속은 server 경로가 아니라 **테스트의 관측 설계**(벽시계가 우리 폴링 비용을 쟀다). 예산을 늘려 닫지 않았다.

**닫는 조건 (a)~(e) — 코드와 실행으로 확인**:

| # | 조건 | 확인 |
|---|---|---|
| a | 헬퍼가 폴 횟수 반환 | `ws_integration.rs:500` `async fn wait_for_no_connections(..) -> usize` |
| b | 단언을 폴 횟수로 | `:1007~1011` `polls <= GRACEFUL_CLOSE_POLL_BUDGET`(=10, `:963`) |
| c | 벽시계는 출력만 | `:999~1003` `println!` — "기록용이고 단언 대상이 아니다" |
| d | 상한을 `PING_INTERVAL` 에서 유도 | `:484~485` `CLOSE_POLL_LIMIT = PING_INTERVAL / CLOSE_POLL_INTERVAL` (= 300, 출력 "상한 300" 으로 실측 확인) |
| e | 부하 아래 RED→GREEN | server 실험(×80 in-process, 아래) + **qa 재현(아래)** |

**qa 재현 (I-25 — server 보고를 내 손으로)** — 이미 빌드된 테스트 바이너리를 직접 실행(빌드 없음, G-c), 부하는 코어 12 × 배수 개의 바쁜 프로세스.
`tests/e2e/sv1_load_repro.py`(qa 소유, 신규), `evidence/R3-B/SV1/{sv1-load,sv1-load-heavy,sv1-fullsuite}.json`:

| 조건 | 실행 | 폴 횟수 분포 | 벽시계 최대 | 옛 형태(<2 s) FAIL | 새 형태(≤10) FAIL |
|---|---|---|---|---|---|
| 단일 테스트 ×0 | 10 | 전부 2 | 0.069 s | 0 | 0 |
| ×1 / ×2 / ×4 / ×8 | 각 10 | 2~3 | 0.071~0.136 s | 0 | 0 |
| ×16 / ×32 (384 프로세스) | 각 10 | 2~3 | 0.137 s | 0 | 0 |
| **스위트 전체 병렬**(원래 flaky 가 난 형태) ×0 | 10 (18 테스트 × 10, 전부 통과) | 전부 2 | 0.068 s | 0 | 0 |
| 스위트 전체 병렬 + ×4 (Unity 빌드 대용) | 10 | 2~3 | 0.151 s | 0 | 0 |

**리더 확인 지점 셋에 대한 답**:

1. **8회 중 빠진 1회**: server 원자료 표에 있다 — **elapsed 0.514 s / polls 9**. 옛 형태 PASS, 새 형태 PASS 이지만 **폴이 정상값(1~2)의 4.5배로 올라 예산 10 턱밑**이다. 그래서 정확한 집계는 "4/8 + 3/8" 이 아니라
   **옛 형태 7/8 FAIL, 새 형태 5/8 PASS(그중 1회는 폴 9) · 3/8 FAIL** 이다. server 본문 "폴은 1,2,2,2,9,11,12,13 으로 이분됨" 에서 9 는 **위쪽 무리**에 속한다.
2. **"참 양성"의 독립 증거 — 없다. 순환이다.** `03_server_impl.md` R3 절이 드는 근거는 (i) 폴이 오른 회차와 안 오른 회차의 elapsed 분포가 겹친다, (ii) "48회+ 에서 폴이 실제 지연 증거 없이 오른 사례는 없었다" 둘이다.
   (i) 은 "폴 상승이 벽시계 인플레와 독립"을 보일 뿐 **서버가 실제로 늦게 닫았다는 것을 보이지 않는다.** (ii) 의 "실제 지연 증거"가 무엇인지 절 안에 **폴 횟수 말고는 없다** — 서버 쪽 세션 종료 시각, `SESSION_CLOSED` tick, 소켓 FIN 시각 같은 **다른 출처가 한 번도 측정되지 않았다.** 즉 "폴이 올랐으니 실제로 늦었다"를 폴로 정당화한다.
   **더 근본적인 것**: 폴이 세는 것은 `ws_connections` **게이지가 0 이 되기까지**이고, 그 감소는 서버 자신의 tokio 태스크가 한다. 극단적 기아에서는 그 태스크도 늦게 돈다 — 그러니 **폴은 부하에 민감하다.** 테스트 주석(`:955~962`, `:992`)의 전제 "**부하는 폴 횟수를 늘리지 못한다**"는
   server 자신의 ×64(폴 25)·×80(폴 9~13) 자료와 **내 ×2~×32 자료(폴 2 → 3)** 에서 **문자 그대로는 틀렸다.** architect 는 "부하 아래 폴 횟수가 늘어나면 가설이 틀렸다는 신호 — 멈추고 보고하라"고 조건을 걸었다 → **architect 통지.**
   다만 그 증가가 가리키는 것은 "서버가 ping 주기(15 s = 폴 300)까지 기다린다"는 **이 테스트가 잡으려는 회귀가 아니다**(×80 의 폴 11~13 은 벽시계 2.6~5 s ≪ 15 s). 그러므로 ×80 의 3회는 **이 성질에 대해서는 거짓 양성**이고, "서버가 기아로 느려졌다"는 참일 수는 있으나 **증명되지 않았다.**
3. **현실적 경합에서의 안정성 — 안정적이다.** ×0~×32 외부 부하, 그리고 **원래 flaky 가 난 형태(스위트 전체 병렬) ×0·×4 에서 90회 실행 중 새 형태 FAIL 0**, 폴 최대 3(예산 10 까지 3배 여유, 진짜 회귀 300 과는 100배).
   **그러나 옛 형태도 이 90회에서 한 번도 안 터졌다** — 즉 **원래의 flaky 조건은 qa 도 server 도 재현하지 못했다.** 옛/새 형태를 **가르는** 증거는 server 의 in-process ×80 실험뿐이다(외부 프로세스 부하로는 ×32 까지도 벽시계가 0.14 s 를 넘지 않았다 — Windows 가 I/O 대기에서 깨는 스레드에 우선순위를 올려 주는 효과로 보이나 **확인하지 않았다**).

**종합**: (a)~(d) 는 코드로, (e) 의 "대조를 한 번 보여라"는 server 의 ×80 자료로 **충족**(옛 7/8 FAIL, 같은 실행에서 새 형태 PASS 4회가 폴 1~2). qa 재현은 현실 구간에서 **새 형태가 흔들리지 않음**을 보였다. → **SV-1 은 수정 확인으로 닫는다.**
**architect 통지 1건**: 테스트 주석의 전제("부하는 폴 횟수를 늘리지 못한다")는 극단 부하에서 반증됐다 — 폴은 게이지를 내리는 서버 태스크의 기아에 민감하다. K=10 은 현실 구간에서 충분하지만,
×80 의 FAIL 3회를 "참 양성"이라 부른 근거는 **폴로 폴을 정당화하는 순환**이다. 서버 측 독립 시각(예: 세션 종료 제출 시각 로그) 없이 그 라벨을 붙이지 말 것을 권고한다(판정 영향 없음).

### 6.5 블록 5 — 잔류·재개·디스폰 (같은 인스턴스, `start_tick = 403435`, G-i·G-j 준수)

증거: `evidence/R3-B/B5/`. 재개(`bots resume --gap 6`)와 만료(`bots resume --gap 35` — **30초 창 + 5초 여유 뒤에만** 재접속, G-j)를 **다른 actor 로 동시에** 돌렸다.
그 시점 `data/`: `linger_seconds = 30`, `reconnect_resume_window_seconds = 30`(R2 확인값), `carry_forward_max_ticks = 10`, `tick_hz = 20`.

#### SC-11 — **(1)(2)(e2) PASS / (3) 계약 문구로는 불성립(계약 결함, architect 통지) · 양자화 봉투 대조 PASS**

`resume.json` + `resume-check/resume_check.json`(`tests/e2e/resume_check.py`, 하네스 = client C# 적분기):

| 관찰 | 값 |
|---|---|
| (1) `ship_id` 동일 | leg1 = leg2 = `01a0c484-dcf2-77e2-9ebe-62d13268675c` ✔ |
| (2) `SHIP_SPAWNED` 추가 발행 없음 | DB(`tick ≥ 424776`): 이 함선의 `SHIP_SPAWNED` **1건**(424794), `SHIP_DESPAWNED` 1건(425626, 두 번째 세션 뒤 `LINGER_EXPIRED`). `ship_events.py resume` PASS(세션 2 : 스폰 1) ✔ |
| T0 조건 | 마지막 실제 입력 tick 424814, **T0 = 424844 (+30)** → 이월 10 tick 이 T0 전에 만료, **T0 이후는 전부 휴면 입력**. `carry_remaining_at_t0 = 0` |
| T0 상태 | 속도 (39.2, −3.7, 7.8) m/s, **`ω_roll` −39.5 °/s**(보조 끔 + 롤 입력의 결과) — 잔류 구간이 **감쇠(9단계)와 오토레벨(5단계)을 둘 다** 탄다 |
| T1 (N = 122 tick) | 속도 **0**, `ω_roll` −1.25 °/s — 오토레벨이 거의 끝남 |
| (3a) **계약 문구: 14필드 양자화 정수 완전 일치** | **10/14** — 위치 (+2, −1, +1) mm, `orientation_w` −1 micro 차이 |
| (3b) **양자화 봉투**(T0 ±0.5 양자 64개 시작점) | **T1 14필드 전부 봉투 안**. 봉투 폭: 위치 8/4/5 mm, 방향 2/1/0/1 micro |
| 잔류 중 관측자 행 | **61점 전부 봉투 안**(최대 편차 위치 2 mm, 속도 1 mm/s). presence **LINGERING 60 → ACTIVE 1**(T1) |
| **음성 대조**(휴면 모델을 `flight_assist=false` 로 틀리게) | T1 **11필드 봉투 밖**, 관측 **61/61 봉투 밖** — 대조가 틀린 모델을 잡는다 |
| (e2) 재개 후 첫 `input_seq = 1` | **`ACCEPTED`**(tick 424986), 그 뒤 `ack_input_seq = 1` ✔ (I-43) |

**(3) 의 판정**: 서버의 T1 은 **T0 스냅샷과 양립하는 모든 서버 상태에서 나올 수 있는 값의 범위 안**이고, 독립 계산(C#)과의 차이 2 mm 는 **봉투 폭(8 mm)의 절반 이하**다.
"정확히 같다"가 10/14 인 것은 서버 결함이 아니라 **계약이 요구할 수 없는 것을 요구했기 때문**이다(§5.6). → **server FAIL 로 적지 않는다. architect 통지**: SC-11 (3) 을
"양자화 봉투 안" 또는 "메모리 테스트에서 `f64` 상태로 비트 일치"로 바꾸는 것을 권고한다.

#### SC-68 (f) — **PASS** (블록 4 잔여)

다른 actor 의 잔류 함선 가로채기: 재개 시나리오의 **관측자(bot-008, 다른 actor)** 가 bot-007 함선이 잔류하는 동안 붙어 있었는데, 관측자의 `controlled_ship_id` 는 **자기 함선**(`01a0c484-d9d2-…881d`)이었고
잔류 함선은 **bot-007 이 재접속했을 때 bot-007 에게만** 돌아왔다(leg2 `controlled_ship_id` = 잔류 함선). 만료 시나리오의 관측자(bot-018)도 같다(`…13e8` ≠ 잔류 함선).
**`hijack-linger` 전용 probe 없이** 관측 구조로 닫았다 — 가로챌 수단(명령의 `ship_id`)이 어휘에 없고(e), 세션은 토큰 주체의 함선만 받는다.

#### SC-32 — **PASS** (라운드 2 의 "관측 미완" 해소)

만료 시나리오(`expire.json`, `SC-32-check.txt`): 함선 `…ea68`, DB `SHIP_DESPAWNED{LINGER_EXPIRED}` tick **425445**.

| 관찰 | 값 |
|---|---|
| 관측자가 본 그 함선의 행 | 326 = ACTIVE 26 + **LINGERING 300** (424846 ~ 425444, 2 tick 간격 = **정확히 600 tick = 30 s**) |
| **디스폰 tick 이후의 행** | **0** — 마지막 행 425444(디스폰 직전 스냅샷) |
| 관측자가 디스폰 뒤에도 보고 있었나 | 관측 스냅샷 424778 ~ **425648** — 디스폰 뒤 **~200 tick** 더 봤다 (조건이 실제로 발생했다) |
| LINGERING → ACTIVE 역전 | 0 |

leg2(35 s 뒤)는 **새 함선**(`…dea4`)을 받았다 — 잔류가 만료된 뒤의 재접속은 재개가 아니라 스폰이다(G-j; SC-10 은 라운드 2 PASS 라 재판정하지 않았다).

#### SC-13 — **PASS** ((a) 1 / (b) 1)

종료 직전 `/debug/stats`: `ws_connections 1 / ships_active 1 / ships_lingering 1`(bot-030 비행 중 + bot-031 이 3초 전에 끊김). stdin `shutdown` → **`SHUTDOWN exit=0`**. 활성 봇은 `close 1001 initiator=server` 를 받았다.
`ship_events.py shutdown --from-tick 403435`(`SC-shutdown.json`):

| | 함선 | 원인 | 원인 tick | 원인 sequence | 결과 tick / sequence |
|---|---|---|---|---|---|
| **(a) 잔류** | `…c04a` | `SESSION_CLOSED` | **427029**(75 tick 전) | 0 | 427104 / 2 |
| **(b) 활성** | `…5b6b` | `SESSION_CLOSED` | **427104(같은 tick)** | **0 < 1** | 427104 / 1 |

**기록**: 서버 로그의 마지막 줄이 `정상 종료 — 마지막 tick 까지 커밋 완료 tick=427103` 인데 스윕은 tick **427104** 에 있다. 스윕 이벤트는 DB 에 **실제로 있다**(위 표) — 로그 문구의 tick 이 하나 작게 찍히는 것으로 보인다. 판정 영향 없음, server 참고.

#### SC-14 · SC-76 — **PASS (이 인스턴스 구간)** / 라운드 2 의 짝 없는 3척은 기록으로만

`ship_events.py pairs --from-tick 403435`: **`ship_id` 13개, SPAWNED 13 = DESPAWNED 13, 짝 없음·중복 0.** `causation` PASS(함선 이벤트 26, null 0, 스폰 원인 13/13 `SESSION_OPENED`·앞섬, 디스폰 원인 13/13 `SESSION_CLOSED`, lag 0~600).
`actors_with_more_than_one_ship` 에 bot-017 이 2척으로 나온다 — **순차**다(첫 함선 만료 425445 뒤 425546 에 새 스폰). I-29(동시에 1척) 위반이 아니다.
라운드 2 의 짝 없는 3척(`01a0bf0f-9150-…015e`, `01a0bf20-16f7-…c504`, `01a0bf20-2791-…e801`)은 **DB 에 그대로 있고 지우지 않았다**(절대 원칙 5). 이번 판정 구간 밖이다.

### 6.6 SC-24 — **PASS** ((b) 라운드 1, (c)(d) 한 실행 안에서)

**첫 인스턴스**(`evidence/R3-B/B4/probe-cheat-range-turn.log`): (c) 성립, (d) 는 **전체 자세 회전** 기준으로 한도 초과 5쌍(최대 3.7544 °/tick > 3.75) — 동시에 `|ω_roll|` 최대 **15.9 °/s**, `|ω_aim|` 최대 **75.000 °/s(초과 0)**.
**원자료를 남기지 않은 실행이라 다시 볼 수 없었다.** 롤(오토레벨)이 조준 선회 위에 얹혀 전체 회전이 조준 한도를 넘은 것으로 추정했지만 **추정으로 판정하지 않았다.**
서버를 내린 뒤(빌드 금지 G-c 준수) `tools/bots` 를 고쳤다: (d) 를 **뱃머리 방향(조준 채널)** 회전으로 판정하고 전체 자세 회전은 참고로 따로 찍으며, `--series-out` 으로 원자료를 남긴다.
새 고장 주입 테스트 `roll_on_top_of_a_saturated_yaw_is_not_an_aim_violation`(전체 회전은 초과, 뱃머리는 아님 → 성립) + RED(판정을 전체 회전으로 되돌리면 그 테스트 FAIL) — `evidence/R3-A/A4-fault-injection.log` injection 4. `tools/bots` 69 passed / 0 failed.

**둘째 인스턴스**(`start_tick = 427105`, `evidence/R3-B/B4b/`) — 4회, 원자료 `range-turn-series*.json`:

| 봇 | (c) 거부 MALFORMED | 클램프 흔적(주입 seq ack) | 거부 구간 쌍 / 속도 감소 / 이동 | (d) 뱃머리 최대 °/tick (한도 3.75) | 포화 쌍 / 초과 | 전체 자세 최대 (참고) | `\|ω_aim\|` 최대 | `\|ω_roll\|` 최대 |
|---|---|---|---|---|---|---|---|---|
| bot-011 | 4/4 | 0 | 2 / 0 / 7.19 m | 3.7487 | 55 / **0** | 3.7491 | 75.000 | 1.88 |
| bot-041 | 4/4 | 0 | 2 / 0 / 7.48 m | 3.7487 | 57 / **0** | 3.7488 | 75.000 | 0.91 |
| bot-042 | 4/4 | 0 | 3 / 0 / 11.25 m | 3.7487 | 57 / **0** | 3.7487 | 75.000 | 0.45 |
| **bot-043** | 4/4 | 0 | 2 / 0 / 7.48 m | **3.7487** | 57 / **0** | **3.7549 (초과 5쌍)** | 75.000 | **15.90** |

**bot-043 이 첫 인스턴스의 모양을 그대로 재현했다** — 롤 15.9 °/s 가 얹힌 실행에서 전체 자세 회전은 한도를 넘었지만 **뱃머리 회전은 3.7487 로 한도 안**이고 서버가 쓴 `|ω_aim|` 도 75.000 을 넘지 않았다.
즉 초과분은 **롤 권한**(`roll_rate_max_deg_s`, ADR-0010 §2 의 권한 분리)이지 조준 한도 위반이 아니다. 추정이 원자료로 확인됐다.

- **(c)** 범위 초과 4건이 **유효 입력 대신 같은 20 Hz 로** 들어간 동안 **거부 4/4 `MALFORMED_COMMAND`**, 주입한 `input_seq` 가 `ack_input_seq` 로 **한 번도 나오지 않았고**(클램프 흔적 0), 거부 구간을 덮는 스냅샷 쌍에서 **속도가 계속 올랐다**(예: 32.2 → 38.2 m/s, 감소 0) — **거부와 이월 지속이 한 실행 안에서 동시에** 보인다(§4.4 의 공백 해소).
- **(d)** 목표를 180° 반대편으로 계속 줘서 **리미터가 실제로 걸린 쌍이 55~57개**(조건 발생), 그중 **한도 초과 0**. 스냅샷 해상도(2 tick)의 한계는 매 스냅샷 `|ω_aim|` ≤ 75.000 으로 보완했다.

### 6.7 🔴 **계약 외 발견 — 같은 actor 의 두 번째 세션이 두 번째 함선을 만든다(I-29 위반) → 첫 함선이 영구 유령이 되고 종료 때 자기 자신을 원인으로 디스폰(I-30 위반)**

**어떻게 찾았나**: 단계 B 전 구간(`tick ≥ 395701`)에 `ship_events.py causation` 을 돌리자 **RED 인스턴스의 종료 스윕(tick 395902)** 에서 `SHIP_DESPAWNED` 6건의
`causation_id` 가 **자기 자신의 `event_id`** 였다(`evidence/R3-B/stageB-all-causation.json`, 위 SQL 결과). RED 는 패치된 바이너리이지만 **종료 스윕 코드는 GREEN 과 같다** — 그래서 GREEN 으로 재현했다.

**GREEN 재현** (둘째 인스턴스, `evidence/R3-B/I29/`): 같은 라벨(`bot-044`)로 `probe fly` 두 개를 2초 차이로 겹쳐 띄웠다.

| 시점 | `ws_connections` | `ships_active` | DB |
|---|---|---|---|
| 첫 세션만 | 1 | 1 | `SHIP_SPAWNED …ff9d0` @428680 |
| **둘째 세션 열림** | 2 | **2** | **`SHIP_SPAWNED …ab1f2` @428730 — 같은 actor 에 두 번째 함선** |
| 두 세션 모두 닫힘 | 0 | **1** | `SESSION_CLOSED` @428911, @428961 |
| 30 s 잔류 창 지남 (tick 429838) | 0 | **1** | 둘째 함선만 `LINGER_EXPIRED` @429561. **첫 함선은 세션 없이 `ACTIVE` 로 남음(유령)** |
| stdin `shutdown` | — | — | 첫 함선 `SHIP_DESPAWNED{SERVER_SHUTDOWN}` @430030 — **`causation_id = 자기 자신`, `correlation_id` 는 어떤 `SESSION_OPENED` 에도 없음** |

`ship_events.py overlap`(신규 — 아래) 결과: 둘째 인스턴스 **FAIL 1쌍(actor 044)**, 블록 5 인스턴스 **PASS(13척, 겹침 0)**, AC-2(h-log) 실행들 **FAIL 8쌍 — 전부 RED 인스턴스**(멈춘 세션이 안 닫힌 채 같은 봇이 재접속).

**원인(파일:라인)**:
1. 세션 열기(`simulation.rs:609~617`)가 `actor_id → ship_id` 표에서 **잔류(`Lingering`) 함선만** 되돌려주고, **그 actor 의 함선이 `ACTIVE`(다른 세션이 조종 중)** 인 경우를 막지 않아 새로 스폰한다 → I-29 위반. 표가 새 함선으로 **덮어써져**(`:651` `actor_ship.insert`) 첫 함선은 표에서 사라진다.
2. 그 뒤 첫 세션이 닫힐 때 잔류 시작 조건 `actor_ship[actor] == ship && ship.controlling_session == Some(session)` (`:815~820`)이 거짓이 되어 **잔류로 가지 않는다** → 만료도 없다 → 유령 `ACTIVE`(I-40 위반: 세션 없는 `ACTIVE`).
   같은 조건이 종료 스윕에도 있다(`server/crates/sim/src/simulation.rs:525~530`).
3. 종료 스윕의 `despawn_ship` 이 `linger_cause_event_id` 가 없으면 **`event_id` 자신을 원인·상관으로 쓴다**(`simulation.rs:921~922`, 주석 "없으면(있을 수 없다, I-30) … 로그를 남긴다") — "있을 수 없다"던 경로가 **실제로 탔다.** 결과는 **자기 참조 인과**와 **세션 없는 correlation**.

**왜 중요한가(현실 경로)**: 연결이 소리 없이 끊긴(FIN 없음) 클라이언트가 `IDLE_TIMEOUT` 30 s 안에 재접속하거나, 한 계정으로 클라이언트 두 개를 켜면 바로 이 경로다. **역사 기록에 원인이 자기 자신인 사건이 남는다**(절대 원칙 9 의 감사 가능성).
`pairs`(I-41: 함선당 스폰 1/디스폰 1)는 이 경우에도 **PASS** 다 — 짝은 맞기 때문이다. 그래서 SC-14 는 이 결함을 못 잡고, SC-81(블록 9, causation 이 실제 이벤트를 가리키는가)이 잡는다.

**담당: server + architect.** 스펙 I-29 는 "잔류 함선이 있으면 되돌려주고, 없으면 스폰"만 정해 **같은 actor 의 활성 세션이 이미 있을 때**를 정하지 않았다 — 새 세션을 거부할지(예: 409/`ALREADY_CONNECTED`), 옛 세션을 닫고 함선을 넘겨받을지는 **architect 결정**이다. 결정 전까지 server 는 적어도 (3)의 자기 참조를 로그가 아니라 **실패로 드러내는 테스트**가 필요하다.
**재현 명령**: 서버 기동 후 `bots probe --case fly --count 8 --label bot-044 &` → 2 s 뒤 같은 명령 → 종료 → `python tests/e2e/ship_events.py overlap --from-tick <start_tick>` / `causation --from-tick <start_tick>`. 기대: 둘째 세션이 거부되거나 첫 함선을 넘겨받아 **동시 함선 1척**, 자기 참조 인과 0. 실제: 동시 2척, 유령 1척, 자기 참조 1건.

**qa 도구 공백 — 기록(§1.5 와 같은 계열)**: 나는 블록 5 에서 `pairs` 의 `actors_with_more_than_one_ship` 을 보고 "bot-017 은 순차라 I-29 위반이 아니다"라고 적었는데, **그 목록은 순차와 동시를 가르지 못한다.**
RED 구간의 동시 8쌍도 같은 목록에 섞여 있었다. 그래서 `ship_events.py overlap`(수명 구간 겹침)을 추가했고, 그 도구로 위 세 구간을 다시 판정했다. **알려진 위반(actor 044)을 FAIL 로, 깨끗한 구간(블록 5)을 PASS 로 가른다** — 양방향 확인.

### 6.8 SC-31 — **PASS** (라운드 2 FAIL 재판정, 블록 3 항목이지만 라운드 2 FAIL 목록이라 닫았다)

라운드 2 는 `ack_input_seq` 가 영원히 `null` 이라(S-A) 관측 불가였다. 이번 라운드 증거:

| 절 | 증거 |
|---|---|
| 적용 전 `null` | 재개 세션 첫 스냅샷(T1) `ack_input_seq = null`(`B5/resume.json` leg2 t1) |
| 서버가 마지막으로 적용한 `input_seq` | 봇 e 4대 각 `ack_last = 50` = 봇당 송신 50(§3.1) / cheat-seq `ack_last = 97` = 패턴 최대 적용값 |
| 세션 내 단조 비감소 | 이번 라운드 모든 봇 실행에서 `ack_regressions = 0`(cheat-seq 스냅샷 56건 포함) |
| 재개 = 새 세션은 `null` 에서 다시 | T1 `null` → `input_seq = 1` **ACCEPTED** → `ack = 1`(I-43) |
| **`(5, 3)` 이 한 tick 에 도착하면 5 적용, 3 `STALE_INPUT`** | 셋째 인스턴스(`start_tick 430031`), `tests/e2e/raw_ws_send.py --burst`(두 프레임을 기다림 없이 연속 송신) **3회**: 두 `COMMAND_RESULT` 의 envelope tick 이 **같다**(430052 / 430076 / 430100 — 같은 tick 도착을 실제로 만들었다), **5 = ACCEPTED, 3 = `STALE_INPUT`, 그 tick 스냅샷 `ack = 5`**, 3/3. 서버 `commands_rejected_total{STALE_INPUT}` 델타 **+3** = 봇 관측 3. `input_superseded_total` 델타 0 — 3 은 후보가 아니라 거부다(ADR-0011 §4 의 구분 그대로) |

증거: `evidence/R3-B/SC31/`. 이 인스턴스 DB 구간(`tick ≥ 430031`): 함선 1척 짝 맞음, 겹침 0, 인과 PASS.

**부수 관찰(SC-68 (g) 의 빈칸 해소)**: pre-ready probe 가 10건 중 결과 8건(`missing_results = 2`)이었던 것 — 같은 구간 `commands_dropped_over_tick_cap_total` 이 **0 → 2**.
손실 항등식 **보낸 10 = `COMMAND_RESULT` 8 + tick 상한 드롭 2** 가 성립한다(3차 개정의 SC-26 항등식). 결과 누락이 아니라 설계된 드롭이다.

---

## 7. M-17 — 신규 경로가 **실제로 탔는가** (라운드 3 누적)

| 경로 | 라운드 3 | 근거 |
|---|---|---|
| 스폰 | **실행됨** | DB `SHIP_SPAWNED` 13(블록 5 인스턴스) |
| **재개** | **실행됨(실서버, 처음)** | bot-007 함선: 세션 2 : 스폰 1, T1 이 양자화 봉투 안(§6.5) |
| 잔류 만료 디스폰 | 실행됨 | `LINGER_EXPIRED` 425445 등, 원인 lag 600 |
| 종료 디스폰 (a) 잔류 / (b) 활성 | **둘 다 실행됨(처음)** | §6.5 SC-13 — 과거 DB 전체에서 (b) 는 0 이었다 |
| `world_full` 거부 | **미실행** | `upgrade_rejected_total{world_full}` 전 인스턴스 0. 블록 8(31 연결)에서도 정원 64 에 닿지 않는다 — 별도 시나리오 필요 |
| 경계 soft/hard | **미실행(실서버)** | 최대 원점 거리 5,696 m(§3.1) — soft 경계 밖으로 나간 실행 없음 |
| 브레이크 | **미실행(실서버)** | 이번 라운드 봇이 브레이크를 쓰지 않았다(메모리 테스트만) |
| 퇴화 쿼터니언 | **실행됨(실서버, 처음)** | `aim_degenerate_total` 0 → 11 = 적용 1 + 이월 10 |
| 이월 | 실행됨 | `input_carried_forward_total` +129, +10 / range-turn 거부 구간 속도 계속 증가 |
| **이월 만료 → 휴면** | **실행됨(실서버, 처음)** | resume leg1: 마지막 입력 424814 → T0 424844, 휴면 모델로 계산한 봉투가 관측 61점 + T1 을 전부 포함, 틀린 휴면 모델은 전부 밖 |
| tick 상한 초과 드롭 | 실행됨 | `commands_dropped_over_tick_cap_total` 0 → 2 (pre-ready) |
| **tick 층 거부 계수**(`STALE_INPUT`·`RATE_LIMITED`·`DUPLICATE_COMMAND_ID`) | **실행됨(수정 후 처음)** | +40 / +6 / +1, +3 — 봇과 일치 |
| **로그 싱크 버리고 세기** | **실행됨(처음)** | green-drop3 4538줄, stdout 통지 |
| 로그 싱크 종료 기한(`FlushGuard` 2 s) | 실행됨 | 소비자 멈춤 상태 종료 2.06 s, exit 0 |
| **같은 actor 동시 세션** | **실행됨 — 결함 발견** | §6.7 (I-29/I-30) |
| 재개 후 `input_seq = 1` 수락(I-43) | 실행됨 | 4회(resume leg2, SC-31 burst 2·3회차) |

---

## 8. 라운드 3 최종 집계

**라운드 3 에서 판정한 항목** (직전 qa §3~§4 + 이번 §5~§6):

| 판정 | 수 | ID |
|---|---|---|
| **PASS** | **12** | SC-13 · SC-14 · SC-23 · SC-24 · SC-31 · SC-32 · SC-33 · SC-66 · SC-67 · SC-68((e)(f)(g)) · SC-76 · SC-85(도구 자체 검증 확장) |
| PASS (AC 관찰, SC 번호 없음) | 1 | **AC-2(h-log)** — RED→GREEN, 조건 ①②③ |
| **FAIL (기록 후 수정 확인으로 닫음)** | 1 | **SV-1** `graceful_client_close_is_prompt`(SC-01 소속 테스트) — 귀속: 테스트의 관측 설계 |
| **보류(계약 결함 — architect)** | 1 | **SC-11** — (1)(2)(e2) PASS, (3) "양자화 정수로 같다"는 성립 불가, 양자화 봉투 대조는 PASS |
| 미관측 | 0 | — |
| **계약 외 발견 (열린 결함)** | 1 | **§6.7 I-29/I-30** — 같은 actor 동시 세션 → 두 번째 함선 · 유령 · 자기 참조 인과 |

**라운드 2 FAIL 12건의 행방**: 11건 PASS(SC-13·14·23·24·31·32·33·66·67·68(f)(g)·76), 1건 보류(SC-11 — 계약). **열린 FAIL 은 SC 항목 기준 0건.**
라운드 2 보류 3건(SC-58 블록 6, SC-64·65 블록 7)은 이번 턴 범위 밖이다.

**담당자별**:
- **server**: §6.7 (I-29 동시 세션 정책이 정해지면 구현, 그 전이라도 `despawn_ship` 의 자기 참조 폴백을 조용한 기록이 아니라 드러나는 실패로 — server 판단) / 참고: 종료 로그의 tick 이 스윕 tick 보다 하나 작게 찍힘(§6.5, 판정 영향 없음)
- **architect**: SC-11 (3) 문구 / I-29 동시 세션 정책 / SV-1 테스트 주석의 전제 반증과 "참 양성" 라벨 / 스펙 AC-2 (h) 중복 / ③의 stderr 사실 오류
- **qa(나)**: 없음 — 도구 공백 2건(서버 드라이버 §1.5, `pairs` 의 I-29 판별 §6.7)은 이번 라운드에 고쳤다

---

## 9. 계약 자체가 틀렸다고 본 항목

1. **SC-11 (3) "양자화 정수로 같다, 허용 오차 없음"** — 서버 상태는 `f64`, 스냅샷만 양자화된다(`ship.rs:6`). 같은 T0 스냅샷과 양립하는 시작 상태들이 122 tick 뒤 **위치 최대 8 mm 까지 갈린다**(실측 봉투). 정직한 서버도 보장할 수 없다.
   권고: "T0 를 ±0.5 양자 흔든 봉투 안" 또는 "메모리 테스트에서 `f64` 상태 비트 일치"로.
2. **SV-1 테스트 주석(`ws_integration.rs:955~962`, `:992`)의 전제 "부하는 폴 횟수를 늘리지 못한다"** — 폴은 게이지를 내리는 서버 태스크의 기아에 민감하다(server ×64 폴 25, ×80 폴 9~13, qa ×2~×32 폴 2→3).
   K=10 은 현실 구간에서 충분하지만 전제 문장은 틀렸고, ×80 의 FAIL 3회를 "참 양성"이라 부른 근거는 순환이다(§6.4).
3. **스펙 AC-2 의 (h) 중복**(259·260행) — §5.5-1.
4. **architect 조건 ③ 의 "버린 줄 통지가 stderr 로"** — 실제는 stdout(§5.3, §6.1 green-drop3 에서 확인). 결론(stderr 도 파일로)은 유지.
5. **스펙 I-29 가 "같은 actor 의 활성 세션이 이미 있을 때"를 정하지 않았다** — §6.7. 거부할지 넘겨받을지가 없어 서버가 두 번째 함선을 만든다.
6. (직전 qa 권고 재확인) **§3.3 도구 자체 검증 범위**에 "측정 대상을 기동·종료시키는 드라이버"와 "명령 타입별 응답 규약", 그리고 이번 라운드의 **"순차와 동시를 가르지 못하는 집계"**(I-29)를 넣을 것.

---

## 10. 블록 6~9 진입 조건

| 블록 | 준비된 것 | 남은 것 |
|---|---|---|
| **6 Unity 실서버·예측·육안** (SC-51~60) | 서버 GREEN 바이너리(sha `16555876…`), 드레인 기동 경로(`server_boot.py serve`)가 1시간 이상 멈춤 없이 동작, 종료 exit 0 반복 확인. SC-51·52 는 라운드 2 PASS | **client 의 Unity Editor 실서버 실행**(현재 Unity 미실행). **SC-59(부호 검증)는 사람이 직접 봐야 한다** — 화면 입력 상태가 함께 찍힌 클립(계약 SC-59). §0.10: 그 전까지 이 슬라이스는 부호에 대해 아무것도 증명하지 못한다. G-e(측정 중 `client/` 저장 금지) |
| **7 2 클라이언트 가시성** (SC-61~65) | 봇 쪽 관측자·CSV 준비됨 | **§0.11 — 관측자 B 를 봇으로 대체 금지.** client 의 "한 Unity 프로세스 안 세션 2개" 경로가 실서버에서 도는지 client 확인 필요. SC-64·65 라운드 2 보류 |
| **8 부하 A·B·C·D + 성능** (SC-25~27, SC-69~75) | 봇 게이트 3단언(입력 0 이면 빨간불) 적용, 1300 연결 churn 에서 게이트 초록 확인. SV-1 재현 도구(`sv1_load_repro.py`) | **31번째 연결 = 사람이 띄운 Unity PlayMode 가 먼저**(G-d). 측정 중 빌드 금지(G-c). 이 PC 는 livingfeed 컨테이너 5개가 떠 있다 — 리포트에 적을 것 |
| **9 기록 무결성·커버리지·경계면** (SC-76~84, SC-86) | `ship_events.py` 에 `shutdown`·`overlap` 추가, `stats_delta.py`, `resume_check.py` | **SC-81(인과가 실제 이벤트를 가리키는가)은 §6.7 결함이 있는 구간을 포함하면 FAIL 한다** — 둘째 인스턴스(`tick 427105~430030`)와 RED 구간(395701~395902) 에 자기 참조 7건이 영구히 남았다(절대 원칙 5 — 지우지 않는다). 판정 구간을 정하거나 결함 수정 후 새 구간으로. **SC-86 범위에 이번 라운드 새 도구들의 자체 검사(selftest·음성 대조·RED)를 넣을 것** |

**요약에 반드시 싣는 문장(계약 §0.10)**: **손 방향 부호가 통째로 뒤집혀도 자동 검증은 전부 통과한다.** 검출기는 SC-59(AC-14 a·d)의 육안 관찰 하나뿐이다. 그것을 사람이 실제로 볼 때까지 이 슬라이스는 부호에 대해 **아무것도 증명하지 못한다.**

---

## 11. 라운드 4가 승인된다면 — 계획 (실행하지 않았다)

계약 §7 의 라운드 3회를 다 썼다. 아래는 **사용자가 라운드 4를 승인할 때**의 순서다. 소요는 이 PC·이번 라운드 실측에 기댄 대략값이다(측정 시간 기준, 대기·보고 제외).

**아직 한 번도 판정하지 않은 항목**(라운드 1~3 합산): SC-25·26·27 · SC-53·54·56·57·58·59·60 · SC-61~65 · SC-69~75 · SC-77·78·81.
**다시 돌려야 하는 항목**: SC-01(서버 코드가 바뀌었다 — 게이트 3종 재실행), SC-82·83·84·86(라운드 1 PASS, 이후 계약·코드·qa 도구가 바뀌었다).
**보류**: SC-11 (3) — 문구가 바뀌어야 판정할 수 있다.

| 순서 | 무엇 | 선행 조건 | 소요 |
|---|---|---|---|
| **0** | 계약 7차 개정 반영 — SC-11 (3) 문구(봉투 기준 또는 메모리 `f64` 비트 일치), I-29 동시 세션 정책을 스펙에 넣은 뒤 **관찰 항목을 추가**(같은 actor 겹침 접속 → `ship_events.py overlap` 겹침 0 + `causation` 자기 참조 0), §3.3 에 도구 공백 3종 추가 | **architect 판정 2건**(SC-11 문구, I-29 정책) | 문서 30분 |
| **1** | **I-29 재판정** — server 의 RED→GREEN 보고를 내 손으로 재현. 겹침 접속 재현 + 종료 → `overlap`·`causation`·`pairs`, 함께 SC-11 (3) 새 문구로 재판정(`resume_check.py` 그대로) | server 수정 완료 + **리더 착수 신호** + 측정 전 빌드 완료(G-c) | 30분 |
| **2** | **블록 0 회귀** — SC-01(fmt·clippy·test 3회), SC-82 커버리지 `--strict`, SC-83 경계면 표, SC-84, **SC-86 에 새 도구 자체 검사 포함**(`log_pipe_backpressure selftest`, `resume_check` 음성 대조, `stats_delta` 의 옛 증거 RED, `ship_events overlap` 양방향, `tools/bots` 69 tests) | 1 이후(코드가 더 안 바뀌는 시점) | 1시간 |
| **3** | **블록 6 — Unity 실서버·예측·육안** SC-53·54·56·57·58·60 (+ SC-51·52 는 라운드 2 PASS 유지) | **client 가 Unity Editor 로 실서버에 붙인다**(현재 Unity 미실행). G-e(측정 중 `client/` 저장 금지). SC-58 은 Deep Profile 끔 | 1~1.5시간 |
| **3a** | **SC-59 부호 육안 검증** | **사람이 필요하다** — 화면 입력 상태가 함께 찍힌 클립을 사람이 보고 (a)(d)를 판정. **이것 없이는 슬라이스가 부호에 대해 아무것도 증명하지 못한다(§0.10)** | 사람 30분 |
| **4** | **블록 7 — 두 클라이언트 가시성** SC-61~65 | §0.11: 한 Unity 프로세스 안 세션 2개(client 경로). 막히면 **SC-64 만 봇으로, SC-65 는 미검증(E8)** — 봇으로 대체하면 부호가 뒤집힌다 | 1시간 |
| **5** | **블록 8 — 부하 A·B·C·D + 성능** SC-25·26·27 · SC-69~75 | **31번째 연결 = 사람이 띄운 Unity PlayMode 가 먼저**(G-d). **빌드 금지**(G-c). 동시 컨테이너 수 기록(지금 livingfeed 5개). D 단계는 PostgreSQL 을 멈췄다 다시 켠다 — **`docker compose down` 은 어떤 형태로도 쓰지 않는다**(G-a) | 1.5~2시간 |
| **6** | **블록 9 — 기록 무결성** SC-77·78·81 (+ SC-76·80 재확인) | **판정 구간 결정**: RED 구간(tick 395701~395902)과 둘째 인스턴스(427105~430030)에 자기 참조 인과 7건이 **영구히 남아 있다**(절대 원칙 5). 그 구간을 뺀 tick 구간으로 판정하고, 뺀 이유를 리포트에 적는다 | 30분 |
| **7** | 리포트 `04_qa_report_r4.md` + 리더 보고 | — | 30분 |

**합계**: qa 측정 약 **6~7.5시간** + **사람 참여 2회**(SC-59 육안 30분, 블록 8 의 Unity PlayMode 31번째 연결 유지). 1·2 는 server 수정 직후 바로 할 수 있고, 3~5 는 client·사람의 일정에 묶인다.

**맥락 관리(세 번째 qa 의 제안)**: 블록 1~2 와 3~7 을 **서로 다른 qa 세션**으로 나누는 것을 권한다. 이번 라운드는 한 세션이 단계 A·B 를 모두 맡아 끝까지 갔지만, 블록 6~8 은 Unity 로그·부하 산출물이 커서 한 세션에 다 담기 어렵다. 인계는 이 리포트와 `evidence/` 로 충분하다(이번에 그렇게 이어받았다).

**판정 규율 재확인**: 라운드 4 에서도 새 관찰마다 **조건이 실제로 발생했는지**를 함께 단언한다(§7a). 특히 블록 8 은 "N 연결이 붙었다"가 아니라 **31번째가 Unity 인지**, "손실 0" 이 아니라 **보낸 수 > 0 인 창에서의 손실 0** 이다.

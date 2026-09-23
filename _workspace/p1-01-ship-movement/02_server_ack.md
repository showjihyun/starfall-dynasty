# server의 스프린트 계약 확인 (p1-01)

- 작성: server, 2026-09-20
- 대상: `02_sprint_contract.md` SC-01~40(server 몫) + 공통 부록(§0.5 층별 표, §0.6 짝짓기 키, §2 비교표, §4 게이트)
- 선행: `01_server_spec_review.md`(B-1~B-14). **B-1·B-2·B-3이 채택된 것을 확인했다.**
- **이 턴은 답변과 인수인계다. 코드는 한 줄도 쓰지 않았다.**
- 구현은 별도 담당이 맡는다 → §3이 그 인수인계 문서다.

## 0. 확인란

| 영역 | 항목 | 확인 | 비고 |
|------|------|:---:|------|
| A 게이트·결정성 위생 | SC-01 ~ SC-04 | **☑** | |
| B 데이터·기동 거부 | SC-05 ~ SC-07 | **☑** | SC-06은 §1-① 의 `STARFALL_DATA_DIR` 추가가 전제 |
| C 스폰·잔류·재개·인과 | SC-08 ~ SC-14 | **☑** | SC-11에 §1-③ 의 초기화 표가 붙어야 한다 |
| D 적분·서버 판정 | SC-15 ~ SC-24 | **☑** | |
| E 속도핵·스냅샷·결정성 | SC-25 ~ SC-34 | **☑** | SC-25는 §1-⑩ 의 기준 확정 필요, SC-28은 §1-⑤ |
| F 계약 ↔ Rust | SC-35 ~ SC-40 | **☑** | SC-40(f)가 좋은 추가다 — B-2 정규화가 뚫는 구멍을 정확히 막는다 |
| 부록 §0.5 / §0.6 / §2 / §4 | 공통 | **☑** | |

**차단급 이의 없음.** §2에 "증명 불가능하거나 모호한 것" 4건을 적었고, 전부 문구 수준이다.

송신 큐 64 유지 결정에 **동의한다.** "히치를 서버 정책이 흡수하면 '얼마나 느린 소비자까지 참을 것인가'가 측정 없는 판단이 된다"가 맞다. 내 B-10의 요지는 용량이 아니라 **문서의 숫자가 3배 틀렸다는 것**이었고(6.4초 → 실제 2.13초), 그 정정과 "Editor 히치가 원인일 수 있다"는 해석 지침만 남으면 된다.

---

## 1. QA 확인 쟁점 6건에 대한 답

### ① SC-06 — 데이터 디렉토리 인자 (`STARFALL_DATA_DIR`)

**지금은 없다. 추가한다.**

현재 `bins/game-server/src/config.rs:120-157`이 읽는 환경 변수는 7개뿐이다: `STARFALL_HTTP_ADDR`, `STARFALL_LOG_FORMAT`, `STARFALL_TICK_HZ`, `STARFALL_WORLD_ID`, `DATABASE_URL`, `REDIS_URL`, `STARFALL_DEV_AUTH_SECRET`. 데이터 로딩 자체가 이번 슬라이스에 처음 생기므로 **경로도 이번에 정한다.**

**확정 (S2에서 구현):**

| 이름 | 기본값 | 의미 |
|------|--------|------|
| **`STARFALL_DATA_DIR`** | `data` (레포 루트 기준 상대 경로) | 이 디렉토리 **아래에서** 세 테이블을 찾는다 |

찾는 경로는 기본값 기준으로 지금과 같다:

```
$STARFALL_DATA_DIR/ships/*.json            → SHIP_CLASS   (디렉토리 안의 모든 .json)
$STARFALL_DATA_DIR/world/systems/*.json    → STAR_SYSTEM  (디렉토리 안의 모든 .json)
$STARFALL_DATA_DIR/movement/sync-tuning.json → SYNC_TUNING (파일 1개 고정)
```

QA가 쓸 형태:

```powershell
# 임시 사본을 만들고 한 파일만 어긴다. data/ 원본은 건드리지 않는다.
$tmp = "$env:TEMP\starfall-data-reject-01"
Copy-Item -Recurse -Force C:\WorkSpace\SpaceHistoric\data $tmp
# ① hard_boundary_radius_m = 20000.1 로 고쳐 넣는다
$psi.Environment["STARFALL_DATA_DIR"] = $tmp
# → 비0 종료 + 로그에 파일·필드·기대값
```

**추가로 확정하는 것 셋.**

1. **상대 경로는 프로세스의 작업 디렉토리 기준**이다. `cargo run -p starfall-game-server`는 작업 디렉토리가 `server/`이므로 기본값이 맞지 않는다 → **기본값을 `../data`로 두거나 QA가 절대 경로를 주는 편이 안전하다.** 내 권고: **기본값을 절대 경로로 해석하지 않고, 찾지 못하면 "찾은 경로"를 로그에 찍고 기동 거부**한다. "왜 빈 테이블로 떴는가"를 나중에 추적하는 것보다 낫다.
2. **로그 형식을 고정한다** — SC-06이 "파일·필드·기대값"을 요구하므로:
   ```
   ERROR 기동 거부: 데이터 검증 실패 file=<절대경로> field=<JSON Pointer> expected=<제약> actual=<값> rule=<schema|derived>
   ```
   `rule=schema`(계약 스키마 위반)와 `rule=derived`(I-38 유도값 검산 위반)를 구분한다. SC-06의 8종 중 ①②는 `schema`, ③~⑧은 `derived`다.
3. **종료 코드는 1**(p0-02와 같다. 정상 종료만 0).

### ② SC-71 — `snapshot_bytes_total`은 **소켓에 실제로 쓴 바이트**다

**답: 소켓 기록 시점에 센다.** 파일:라인으로 답한다.

p0-02에 이미 그 자리가 있다. `crates/gateway/src/ws.rs`의 송신 루프:

```
ws.rs:274   if sink.send(Message::Text(Utf8Bytes::from(text))).await.is_err() { … break }
ws.rs:278   stats.record_message_written(type_name);      ← 여기가 "실제로 썼다"
```

`record_message_written`은 `crates/gateway/src/stats.rs:347`이고, **`sink.send`가 `Ok`를 돌려준 뒤에만** 불린다. 반면 큐 투입은 두 곳(`runtime.rs`의 tick 드라이버, `ws.rs:571`의 게이트웨이 거부 응답)에서 `record_message_enqueued`(`stats.rs:341`)로 따로 센다.

**따라서 `snapshot_bytes_total`은 `record_message_written`과 같은 자리에서, 같은 조건으로 센다.** 구현은 `ws.rs:278` 바로 옆에 한 줄:

```
stats.record_message_written(type_name);
stats.record_message_bytes_written(type_name, text.len() as u64);   // ← 추가
```

`text`는 **소켓에 넘긴 바로 그 UTF-8 바이트열**이므로 봇이 수신 프레임에서 잰 길이와 같은 것을 센다(프레이밍 오버헤드 제외 — 아래 주의 참고).

**독립 출처가 되는 이유**: 큐에 넣었지만 못 쓴 메시지는 p0-02의 `messages_dropped_total`이 따로 센다(연결 종료 시 `out_rx`를 비우며 계수). 그래서 회계가 닫힌다:

```
enqueued == written + dropped + send_queue_depth
```

이 항등식은 p0-02에서 이미 테스트로 고정돼 있다(`ws_integration.rs`의 `sc30_sc31_stats_identities_hold_at_rest`). **바이트 쪽도 같은 자리에서 세므로 같은 성질을 물려받는다.**

**QA에게 주의 하나 (20 % 판정에 영향):** 서버가 세는 것은 **WebSocket 페이로드 바이트**이고, 봇이 소켓에서 재면 **프레임 헤더(2~14 B) + TCP/IP**가 더해진다. 15 KiB 메시지에서 헤더는 4 B(126~65535 길이 → 2+2)이므로 **0.03 % 미만**이라 20 % 판정에는 영향이 없다. 다만 **봇이 tungstenite의 메시지 페이로드 길이를 재면 서버와 정확히 같고, TCP 바이트를 재면 다르다** — 어느 쪽을 쟀는지 리포트에 적어 달라. 권고는 **페이로드 길이**다(그래야 차이가 "ADR 산출이 틀렸다"만 뜻한다).

### ③ SC-11 — 재개 시 무엇을 이어받는가

**답: 월드 상태는 전부 이어받고, 세션·입력 상태는 전부 새로 시작한다.** 표로 확정한다.

| 항목 | 재개 시 | 근거 |
|------|--------|------|
| `ship_id` | **그대로** | I-29(actor당 1척), ADR-0011 §6 |
| 위치 `p`, 속도 `v` | **그대로** (잔류 동안 적분된 값) | ADR-0011 §6 |
| 자세 `q` | **그대로** | 〃 |
| `ω_aim`, `ω_roll` | **그대로** | 〃. 잔류 중에도 슬루·오토레벨이 계속 돌았다 |
| `ship_class_id` | **그대로** | 함선이 같다 |
| `presence` | `LINGERING` → **`ACTIVE`** | I-40 |
| `session_id`, `correlation_id` | **새로 생성** | ADR-0005 §3(세션 재개는 없다) |
| `input_seq` | **1부터.** 서버의 `last_applied_input_seq`를 **`None`으로 초기화** | ADR-0011 §6. 초기화하지 않으면 새 세션의 `input_seq=1`이 `STALE_INPUT`으로 거부된다 |
| `ack_input_seq` | **`null`** (그 세션이 아직 아무것도 적용하지 않았다) | AC-7(d) |
| 중복 제거 링(`command_id` 1024) | **비운다** (세션에 묶인다) | ADR-0006 §6 |
| 이월 입력(마지막 `SET_SHIP_CONTROL`) | **버린다** | 아래 |
| `flight_assist`, `brake`, 추력, 롤 | **전부 0/기본값으로 초기화** | 아래 |
| `SHIP_SPAWNED` 이벤트 | **발행하지 않는다** | I-41 |
| 잔류 타이머, 보관된 `SESSION_CLOSED.event_id` | **해제·폐기** | 다시 끊기면 그때의 `SESSION_CLOSED`가 새 원인이 된다 |

**"입력 관련 상태를 전부 초기화한다"의 근거**: 잔류가 시작되면 이월은 `carry_forward_max_ticks`(10 tick = 500 ms) 안에 이미 만료됐고, 그 시점부터 함선은 **추력 0, 롤 0, 목표 자세 = 현재 자세**로 굴러왔다. 30초 뒤에 그 마지막 입력을 되살리면 **재접속하는 순간 함선이 갑자기 가속한다.** 그래서 재개 시점의 입력 상태는 "이월 만료 상태"와 같다.

**`flight_assist`만 따로 본다.** 잔류 중 감쇠가 계속 작용했다는 것은 `flight_assist = true`로 굴렀다는 뜻이다. 재개 후 첫 입력이 오기 전까지도 같아야 관성이 튀지 않는다. **→ 잔류·재개 모두 `flight_assist = true`, `brake = false`, 추력·롤 = 0, 목표 자세 = 현재 자세로 고정한다.** 이것을 **"휴면 입력(dormant input)"**이라 부르고, 이월 만료 시에도 같은 값으로 간다.

**따라서 SC-11의 QA 독립 계산에 필요한 것은 감쇠 규칙 하나가 맞다.** 구체적으로 ADR-0010 §2에서 휴면 입력일 때 살아 있는 경로는:

- 1단계: `q_aim = q`(목표 = 현재) → 2단계 `s = 0` → 불감대 → `ω_t = 0`
- 3단계: `ω_aim`이 `turn_accel_deg_s2 × dt`씩 0으로 슬루
- 5단계: `roll = 0`, `assist = true` → **오토레벨이 돈다** ← 주의
- 6단계: `a_local = 0` → `a_thrust = 0`
- 7단계: 경계 밖이면 당김
- 9단계: `assist && |a_thrust| == 0` → **`assist_linear_decel_mps2 × dt`로 전 속도 감쇠**
- 10~12단계: 상한·위치·하드 경계

**QA에게 중요한 것 하나**: 잔류 중에도 **오토레벨이 계속 돈다**(5단계). 자세가 시간에 따라 변하므로 `q`와 `ω_roll`은 "그대로 두면 되는" 값이 아니다. SC-11이 `(p, v, q, ω_aim, ω_roll)` 전부를 검사하겠다고 한 것은 옳고, **독립 계산도 5단계를 포함해야 한다.**

> **이것을 ADR-0011 §6에 "휴면 입력" 표로 넣어 달라고 architect에게 요청한다** — 지금은 "추력 0, aim = 현재 자세"까지만 있고 `flight_assist`·`brake`가 정해지지 않았다. 이 두 값이 다르면 잔류 궤적이 통째로 달라진다.

### ④ SC-33 — 신규 메트릭 6종의 정확한 키 이름과 형태

**확정.** p0-02의 명명 규칙(`*_total` = 카운터, `*_depth`/`*_bytes` = 게이지, 라벨은 `[{label, count}]` 배열)을 그대로 따른다.

| 키 | 형태 | 의미 |
|----|------|------|
| `snapshots_sent_total` | **스칼라** 카운터 | 소켓에 쓴 `WORLD_SNAPSHOT` 수. `messages_written_total`의 `WORLD_SNAPSHOT` 라벨과 **같은 수여야 한다**(교차 검증용 중복이다) |
| `snapshot_bytes_total` | **스칼라** 카운터 | 위와 같은 자리에서 센 **페이로드 바이트 합** (§1-②) |
| `send_queue_bytes` | **스칼라** 게이지 | 지금 송신 큐에 들어 있는 메시지들의 바이트 합 |
| `send_queue_bytes_max` | **스칼라** 게이지 | 위의 최고 수위 |
| `input_superseded_total` | **스칼라** 카운터 | 한 tick에 2건 이상 도착해 덮어써진 입력 수 |
| `input_carried_forward_total` | **스칼라** 카운터 | 도착 0건이라 직전 입력을 이월한 tick 수(세션 합계) |
| `aim_degenerate_total` | **스칼라** 카운터 | 목표 쿼터니언 노름 < 1e-3 으로 현재 자세로 대체한 횟수 |

**SC-33이 6종이라 했으나 `send_queue_bytes`를 현재/최대로 나누면 7키다.** p0-02가 `send_queue_depth` / `send_queue_depth_max`를 두 키로 냈으므로 같은 형태로 간다 — QA는 **7키**로 세어 달라.

**라벨 배열이 되는 것은 없다.** 스냅샷은 타입이 하나이고, 입력 관련 카운터는 세션별로 나누면 31개 라벨이 생겨 `/debug/stats`가 불필요하게 커진다. **단 기존 라벨 배열 2종은 이번에 항목이 는다:**

- `commands_received_total` — p0-02는 스칼라였다. **`SET_SHIP_CONTROL`이 생기므로 라벨 배열로 바꿀지**가 판단 지점인데, **바꾸지 않는다**(p0-02의 항등식 `commands_received_total − messages_enqueued_total{COMMAND_RESULT} == 0`이 스칼라 전제로 쓰이고 있다). 타입별 분해가 필요하면 별도 키를 추가한다.
- `commands_rejected_total` — 라벨 배열. **`RATE_LIMITED`·`STALE_INPUT` 2개가 추가되어 8라벨**이 된다. 순서는 스키마 `enum` 순서 그대로: `MALFORMED_COMMAND, UNKNOWN_COMMAND_TYPE, SCHEMA_VERSION_UNSUPPORTED, DUPLICATE_COMMAND_ID, SERVER_BUSY, TOO_MANY_IN_FLIGHT, RATE_LIMITED, STALE_INPUT`.
- `messages_enqueued_total` / `messages_written_total` — 라벨 배열. **`WORLD_SNAPSHOT`이 추가되어 4라벨**: `SESSION_READY, COMMAND_RESULT, PING_REPLY, WORLD_SNAPSHOT`.

**SC-29가 요구한 `ships_active` / `ships_lingering`도 함께 낸다**(스칼라 게이지 2종). 그래야 "함선 수를 `/debug/stats`와 대조"가 가능하다. 두 값은 **tick 드라이버가 월드에서 직접 읽어 `store`** 한다 — p0-02의 `ws_connections`와 같은 방식이고, 카운터 뺄셈이 아니다(I-25).

그리고 내 검토 B-14의 `snapshot_build_us`(히스토그램)도 낸다. SC-33의 판정 대상은 아니고 **M 기록용**이다.

### ⑤ SC-28 — 중간 접속 세션의 첫 스냅샷

**답: 스냅샷은 전역 tick 기준으로 발사된다. 접속 tick 기준이 아니다.** 그래서 **첫 간격은 1~2 tick으로 달라진다.**

규칙을 확정한다:

```
tick % snapshot_interval_ticks == 0 인 tick에서, 그 시점 살아 있는 모든 세션에 1건씩 보낸다.
```

- **전역 정렬을 택하는 이유**: 세션별 위상을 따로 두면 31개 세션이 서로 다른 tick에 직렬화를 요구해 **모든 tick이 스냅샷 tick**이 된다(부하가 2배가 되고 `tick_body_us`의 이봉 구조도 사라진다). 전역 정렬이면 2 tick마다 한 번에 31벌을 만들고 나머지 tick은 비어 있다.
- **세션이 tick T에 수락되면**(= `SESSION_OPENED`가 발행된 tick), 첫 스냅샷은 `T`(T가 배수면 같은 tick) 또는 `T+1`이다. 즉 **첫 간격은 0 또는 1 tick**이고 그 다음부터 항상 2다.

**SC-28 문구 제안:**

> 연속 두 스냅샷의 envelope `tick` 차이가 `snapshot_interval_ticks`와 같다. **각 세션의 첫 스냅샷은 검사에서 제외한다** — 스냅샷은 전역 tick 기준(`tick % interval == 0`)으로 발사되므로 중간에 들어온 세션의 첫 간격은 0~1 tick이다. 제외한 건수를 증거에 적는다(세션당 1건 = 31건).

**부수 성질 하나 (SC-63·AC-16c에 좋다)**: 전역 정렬이므로 **같은 tick의 스냅샷은 모든 수신자에게 같은 `ships` 배열**이다. "A가 본 자기 위치 == B가 본 A의 위치"가 같은 배열에서 나온다는 QA의 관찰(항진명제에 가깝다)이 **설계상 보장된다**는 뜻이고, 그것이 바로 그 항목이 잡는 것이 "세션별 직렬화 버그 하나"뿐인 이유다.

### ⑩ SC-25 — "한 tick 이동분"의 기준

**답: 최대 속도 기준 고정값 7 m를 쓰는 것이 맞다. 다만 그 이유가 "느슨하게 잡자"가 아니다.**

이 판정이 답해야 하는 질문은 "두 봇의 거리 차가 작은가"가 아니라 **"빨리 보낸 쪽이 더 갔는가"**다. 그래서 기준은 **측정 불확실성의 상한**이어야 한다.

거리 차가 생기는 정당한 원인은 둘뿐이다.

1. **경계 tick의 어긋남** — 두 봇의 첫 입력이 서로 다른 tick에 적용되면 궤적이 최대 1 tick 어긋난다. 그 1 tick의 이동량은 **그 시점 속도 × dt**이고, 상한이 `max_speed_mps × dt = 140 × 0.05 = 7 m`다.
2. **양자화** — 스냅샷 위치가 1 mm 단위. 무시할 수 있다.

**그 시점 속도로 계산하면 판정이 더 엄격해지지만 계산이 불안정해진다**: 두 봇의 속도가 매 tick 다르고, "어느 tick의 속도인가"를 정해야 하며, 스냅샷은 2 tick마다만 오므로 그 사이 속도를 모른다. **불확실성을 줄이려다 불확실성의 정의가 논쟁거리가 된다.**

**권고하는 형태 (SC-25 문구 제안):**

> 두 함선의 **원점 기준 이동 거리** 차이가 `max_speed_mps × dt = 7.0 m` 이내다. 이 값은 "두 봇의 시작 tick이 최대 1 tick 어긋날 수 있다"는 측정 불확실성의 상한이며, 속도 핵의 허용치가 아니다.
> **함께 적는다**: 두 봇의 최종 속도(스냅샷 정수에서) 와, 거리 차 / 7.0 의 비율. **비율이 0.2를 넘으면 그 자체를 기록**한다 — 7 m 안이어도 계속 한쪽이 앞선다면 경계 어긋남이 아니라 다른 원인이 있다는 신호다.

마지막 문장이 이 항목의 진짜 가치다. 7 m는 통과선이고, **추세**가 증거다. designer 확인이 필요하면 "140 m/s는 `data/ships/scout-s01.json`의 `max_speed_mps`이고 dt는 월드 상수 20 Hz에서 온다"는 점만 확인하면 된다 — 둘 다 이미 고정된 값이다.

---

## 2. 증명 불가능하거나 모호한 항목 (4건, 전부 문구)

| SC | 문제 | 제안 |
|----|------|------|
| **SC-28** | 중간 접속 세션의 첫 간격이 2가 아니다 → "차이가 2가 아닌 쌍의 수 = 0"이 **반드시 실패**한다 | §1-⑤ 의 문구. 첫 스냅샷 제외 + 제외 건수 기록 |
| **SC-29** | "`ship_id` 오름차순"은 맞지만, 봇이 검사하는 것은 **수신한 JSON의 배열 순서**다. `ship_id`는 UUIDv7 문자열이므로 **문자열 정렬과 바이트 정렬이 같은지**가 전제다. UUIDv7 정규 소문자 표기는 하이픈 위치가 고정이라 **문자열 사전순 = 바이트순**이 성립한다(하이픈 `0x2D`가 모든 16진 숫자보다 작지만 위치가 같아 비교에 영향 없음). 성립하지만 **적혀 있지 않다** | SC-29에 "정렬 기준은 `ship_id`의 **정규 소문자 문자열 사전순**이며, 서버 내부의 `UuidV7` 바이트 순서와 같다"를 한 줄. 서버는 `BTreeMap<UuidV7, _>` 순회로 내고, 봇은 문자열로 검사해도 같은 결과가 나온다 |
| **SC-31** | "`ack_input_seq`가 **서버가 마지막으로 적용한** `input_seq`"인데, **재개된 세션에서 `null`로 돌아간다.** "되돌아가지 않는다"를 전 구간으로 읽으면 재개에서 실패한다 | "**한 세션 안에서** 단조 비감소. 새 세션(재개 포함)은 `null`에서 다시 시작한다"로 한정. §1-③ 의 표와 짝이다 |
| **SC-13** | "잔류 중 정상 종료하면 `SHIP_DESPAWNED{SERVER_SHUTDOWN}`" — **활성 함선도** 종료 시 디스폰된다(I-41). 활성 쪽의 `causation_id`가 무엇인지가 계약에 없다(내 검토 B-9) | SC-13을 둘로: (a) 잔류 함선, (b) **활성 함선**. (b)의 `causation_id`는 **같은 tick에 발행된 그 세션의 `SESSION_CLOSED.event_id`**이고 `sequence`가 그보다 크다. 이 순서를 SC-09와 같은 조인으로 검사할 수 있다 |

그 밖에 §0.5 층별 표(반례 34건), §0.6 짝짓기 키, §2 비교표, §4 게이트는 그대로 받는다. **SC-40(f)("integer 선언 필드를 Rust가 정수 타입으로 받는다")는 QA가 스스로 추가한 항목인데, B-2의 정규화가 뚫는 구멍을 정확히 막는다.** 내가 제안하려던 것과 같고, 더 명확하게 쓰여 있다.

---

## 3. 인수인계 — `server/` 현재 구조와 이번 슬라이스가 손댈 지점

구현 담당이 이어받을 것을 전제로 적는다. **p0-02는 내가 구현했고, 아래의 "함정"은 전부 그때 실제로 겪은 것이다.**

### 3.1 현재 구조 (파일 수는 실측)

```
server/
├─ Cargo.toml                      워크스페이스 멤버 5
├─ migrations/0001_*.sql           worlds · domain_events · append-only 트리거 · 시드
├─ crates/
│  ├─ contracts/  (src 7파일)      계약 serde 타입, 범위 newtype, GameCalendar, 레지스트리 대응표
│  ├─ sim/        (src 3파일)      lib.rs · session.rs · simulation.rs   ← 의존성 1개
│  ├─ persistence/(src 1파일)      마이그레이션 embed, 월드 대조, tick 재개, tick 단위 트랜잭션
│  └─ gateway/    (src 8파일)      auth · health · readiness · runtime · state · stats · ws · lib
└─ bins/game-server/ (src 2파일)   main.rs(조립·종료) · config.rs(환경 변수)
```

의존 방향: `contracts <- sim <- gateway`, `contracts + sim <- persistence`, **`gateway`는 `persistence`를 의존하지 않는다**(채널 양 끝을 바이너리가 붙인다). 관측은 `Arc<AtomicU64>` 3개만 공유한다.

### 3.2 태스크별 — 어디에 무엇이 들어가는가

| 태스크 | 새로 만들 것 | 기존 파일 수정 |
|--------|------------|--------------|
| **S1** | `contracts/src/` 에 신규 7타입. `WORLD_SNAPSHOT`은 배열을 쓰는 첫 계약이라 `ShipState`를 별도 타입으로 | `contracts/src/lib.rs`(re-export), `registry.rs`(**리터럴 상수 7개 + `CONTRACT_TYPES` 7행** — 커버리지 스크립트가 문자열을 grep 한다), `tests/contract_tests.rs`(`EXPECTED_SCHEMA_COUNT` 11→18, `EXPECTED_VALID_FIXTURES` 12→26, `EXPECTED_INVALID_FIXTURES` 16→34, `SERDE_REJECTION_TABLE` 행 추가) |
| **S2** | `bins/game-server/src/data.rs` (로더 + 유도값 검산) | `config.rs`(`STARFALL_DATA_DIR`), `main.rs`(마이그레이션 직후·바인딩 **전**에 로드. 실패 = 기동 거부) |
| **S3** | `crates/sim/src/world/` (`vec3.rs`·`quat.rs`·`ship.rs`·`integrate.rs`·`spawn.rs`·`quantise.rs`) | `sim/src/lib.rs`(모듈 공개) |
| **S4** | — | `sim/src/simulation.rs`(`Simulation`에 `WorldState` 추가, `step()`에 적분·스폰·잔류·디스폰·스냅샷 조립), `sim/src/session.rs`(세션 상태에 `last_applied_input_seq`·이월 입력 추가) |
| **S5** | — | `gateway/src/runtime.rs`(스냅샷 라우팅), `ws.rs`(바이트 계수 1줄), `stats.rs`(메트릭 7+2종), `runtime.rs:52`(`SEND_QUEUE_CAPACITY` 256→64) |
| **S6** | `crates/sim/tests/determinism.rs` | — |

### 3.3 건드리면 위험한 곳 (p0-02에서 실제로 물린 것)

| # | 자리 | 무엇이 터지는가 |
|---|------|----------------|
| **1** | `gateway/src/ws.rs` 의 **읽기/쓰기 태스크 종료 규약** | 송신 채널 `Sender`를 **tick 드라이버와 수신 태스크가 둘 다** 들고 있다. 어느 한쪽이 drop 해도 송신 태스크는 깨지 않는다. 그래서 `SessionRoute.finish`(`Notify`)로 **드라이버가 직접 깨우고**, 수신이 먼저 끝나면 `finish.notify_one()` → 송신 await, 송신이 먼저 끝나면 `reader.abort()`를 한다. **이 대칭을 깨면 정상 Close 가 최대 15초(ping 주기) 지연된다** — p0-02에서 겪었고 `graceful_client_close_is_prompt` 테스트가 그것을 막고 있다. **스냅샷 경로를 추가할 때 이 구조를 건드리지 말 것.** |
| **2** | `gateway/src/runtime.rs:288~360` **tick 본문의 경계** | `body_start`(제출 수집 직전) ~ `body`(영속화 제출 직후)가 `tick_body_us`의 측정 구간이고 `sleep`은 제외돼 있다. **직렬화를 이 안에 넣으면 스펙 §8 위반이자 AC-19 기준선이 무의미해진다.** 스냅샷은 여기서 **구조체만** 만들고 문자열로 바꾸는 것은 `ws.rs`의 송신 태스크다 |
| **3** | tick 루프가 **전용 OS 스레드 + `std::thread::sleep`** 인 것 | 이 PC 실측: `tokio::time::sleep(50ms)`는 실제 **61 ms**(60초 루프가 73.5초, lag +22 %). "async 서버니까 tokio 타이머"로 되돌리면 게임 시간이 22 % 뒤처진다. `runtime.rs` 모듈 문서에 근거가 있다 |
| **4** | `runtime.rs`의 **제어 경로 / 명령 큐 분리** | 세션 열기·닫기는 unbounded 제어 채널, 명령만 bounded 4096. 하나로 합치면 큐 포화 시 `SESSION_CLOSED`가 사라져 **I-16이 부하에서만 깨진다** |
| **5** | `ws.rs` 의 **거부 응답 큐 포화 처리** | `send_rejection`이 `try_send` 실패를 무시하면 "보낸 명령 수 == 받은 `COMMAND_RESULT` 수"가 조용히 깨진다(p0-02에서 8000건 중 7872건이 사라진 적 있다). 지금은 `FrameOutcome::Overflow` → `SLOW_CONSUMER`로 연결을 닫는다. **`SET_SHIP_CONTROL` 거부 경로도 같은 함수를 쓸 것** |
| **6** | `stats.rs` 의 `ws_connections` | **카운터 뺄셈이 아니라** 드라이버가 매 tick `routes.len()`을 `store` 한다(I-25). `ships_active`/`ships_lingering`도 **같은 방식**으로 낼 것 — 증감 카운터로 만들면 SC-29의 대조가 항진명제가 된다 |
| **7** | `persistence/src/lib.rs` 의 **제약 위반 처리** | `UNIQUE (world_id, tick, sequence)` 위반은 재시도해도 성공하지 않으므로 **그 배치만 버리고 ERROR + `domain_events_persist_failed_total`**로 드러낸다. 재시도 루프에 남기면 그 뒤 모든 tick이 영원히 커밋되지 않는다. **`SHIP_*` 이벤트가 늘어도 이 규칙은 그대로** |
| **8** | `persistence` 의 **하트비트** | 이벤트가 없어도 20 tick(1초)마다 배치를 보내 `worlds.last_tick`을 갱신한다. **이게 없으면 DB가 죽어도 백로그가 자라지 않아 AC-19의 D 단계(기록 내구성)가 재현되지 않는다** |
| **9** | `crates/sim/Cargo.toml` **주석** | 금지 크레이트 이름을 주석에도 쓰면 SC-04의 grep이 주석에 걸려 검사가 무의미해진다. 금지 목록은 `sim/src/lib.rs` 문서에 있다. **이 파일에 이름을 다시 넣지 말 것** |
| **10** | `bins/game-server/src/main.rs` **종료 순서** | 수락 중단 → axum graceful → tick 스레드 종료 스윕 → **`live_connections == 0`을 최대 3초 대기** → 영속화 flush → exit 0. 대기를 빼면 프로세스가 먼저 죽어 **클라이언트가 close code 를 못 본다**. 이번 슬라이스는 여기에 **잔류·활성 함선의 `SHIP_DESPAWNED`**가 끼어든다 → §2의 SC-13 분리 참고 |

### 3.4 p0-02에서 알고 있는 함정 (문서에 없는 것)

1. **마이그레이션 체크섬** — `0001_*.sql`을 한 글자라도 고치면 기존 볼륨에서 **기동이 실패**한다. 고쳤으면 레포 루트에서 `docker compose down -v && docker compose up -d`. 이번 슬라이스는 마이그레이션을 고치지 않으므로 해당 없지만, `SHIP_*`가 `domain_events`에 들어가는 것뿐이라 **새 마이그레이션이 필요 없다**는 점을 확인해 둔다.
2. **새 마이그레이션 파일을 추가해도 재컴파일되지 않는다** — `crates/persistence/build.rs`의 `cargo:rerun-if-changed=migrations`가 그것을 막고 있다. 기존 파일 *수정*은 재컴파일되지만 *추가*는 안 된다(실측).
3. **stdin `shutdown`의 BOM** — PowerShell `StandardInput`은 첫 줄 앞에 U+FEFF를 붙인다. 서버가 `trim_matches('\u{feff}')`로 걷어낸다. 이 관용을 제거하지 말 것.
4. **통합 테스트의 clippy** — `tests/*.rs`는 `clippy.toml`의 `allow-*-in-tests`가 헬퍼 함수까지 덮지 못한다. 파일 맨 위에 `#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]`를 둔다(p0-02의 두 테스트 파일이 그렇게 돼 있다).
5. **`const { assert!(...) }`** — 상수끼리 비교하는 `assert!`는 clippy가 `assertions_on_constants`로 거부한다. `const` 블록으로 감싼다.
6. **측정 중 빌드 금지** — p0-02에서 같은 측정이 14.9초 → 0.079초로 바뀌었다. 원인은 같은 PC에서 `cargo test`가 컴파일 중이었던 것. AC-19/SC-34는 빌드가 끝난 뒤에 돌린다.
7. **`cargo test --workspace`가 32초 걸린다** — p0-02의 유휴 타임아웃 테스트(`sc26_idle_connection_is_closed_with_1001`)가 32초를 쓴다. 정상이다. SC-34가 자식 프로세스를 띄우면 더 늘어난다.
8. **현재 DB 상태** — `SESSION_OPENED` 272 / `SESSION_CLOSED` 272 / **`QA_APPEND_ONLY_PROBE` 1**, `worlds.last_tick = 350280`. 적분·결정성 테스트는 **메모리에서**(`Simulation::new(world, 0)`) 돌려야 손계산 기대값이 맞는다.

### 3.5 먼저 답이 필요한 것 (구현 담당 → architect)

내 검토(B-1·B-2·B-3)는 반영됐다. 구현 중 새로 드러날 것으로 보는 지점 둘만 남긴다.

- **S6 산출물 형식을 client(C4)와 먼저 맞출 것.** 계약서가 `initial.json` / `inputs.jsonl` / `snapshots.jsonl` 3파일로 이미 고정했으니, `snapshots.jsonl`의 한 줄이 **직렬화된 `WORLD_SNAPSHOT` payload 그대로**인지(그래야 C4가 계약 DTO를 재사용한다) 확인만 하면 된다.
- **`WORLD_SNAPSHOT`의 `maxItems: 64`는 serde가 강제하지 못한다.** 역직렬화 후 서버가 검사하는 자리를 정해야 한다(권고: `ShipState` 배열을 받는 지점에서 길이 검사 + `MALFORMED_COMMAND`가 아니라 **서버 내부 불변식 위반**으로 다룬다 — 이 메시지는 서버가 만드는 것이라 클라이언트가 보낼 일이 없다).

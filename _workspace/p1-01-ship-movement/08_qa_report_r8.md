# p1-01 함선 이동 — 평가 리포트 라운드 8 (qa)

- 평가자: qa (독립 판정)
- 일시: 2026-09-24 KST
- 대상: **R5 실서버 세션** + **R6 부호 세션** + **R16 계측 추가** — 이 슬라이스에서 처음으로 실서버 증거가 존재하는 라운드
- 도구: python 3.12(증거 재계산), git, grep/md5sum. **Unity CLI 실행 불가 — §6.1**
- 규율: 계약 §0.3 · §0.10 · §0.12 · §7a · §7b
- 전제: 증거 수집·1차 판독을 리더가 단독으로 했다. **리더의 수는 하나도 인용하지 않았다 — 전부 로그에서 직접 파싱해 재계산했다.**

> **§0.3 요구 문장:** 이 라운드에 **FAIL이 처음으로 1건 생겼다**(SC-56). FAIL 수가 늘어난 것은 제품이 나빠져서가 아니라 **처음으로 그 항목을 잴 수 있는 세션이 생겼기 때문이다** — 네 라운드 동안 `미검증(증거 요건)`이었던 자리에 실제 수치가 들어왔고, 그 수치가 기준을 어겼다. **막고 있는 게이트 수는 줄지 않았다.**
>
> **§0.10 (요약에 반드시 싣는다):** 손 방향 부호가 통째로 뒤집혀도 자동 검증은 전부 통과한다. 서버와 클라이언트가 같은 공식을 쓰므로 예측 오차 0, fixture 왕복 통과, 결정성 바이트 동일까지 전부 초록이다. **검출기는 SC-59의 육안 관찰 하나뿐이다.** 다만 이번 라운드에 **R6 로그가 그 검출기의 일부를 사람 눈 없이 대체했다**(§3.4) — 전체를 대체하지는 못한다(§3.5).
>
> **SC-59는 제품 책임자 판정으로 통과다. 뒤집지 않는다.** 아래 §3은 그 판정의 근거가 로그로 어디까지 재현되는지만 따진다.

---

## 0. 판정 요약

| SC | 라운드 7 | **라운드 8** | 한 줄 근거 |
|---|---|---|---|
| SC-46 | PASS | **미검증(환경)** | **Unity Editor(PID 20260)가 열려 있어 `unity test`가 실행되지 않는다.** 리더의 "Editor는 닫혀 있다"는 **사실이 아니다**(§6.1) |
| **SC-56** | 미검증(증거 요건) | **FAIL** | (c) `reconcile_hard_snap_total == 0`을 요구하는데 **R5 = 18, R6 = 39**. 세션 범위 값이고, 재측정 면제 조건(`SLOW_CONSUMER`)에 해당하지 않는다(§2) |
| SC-59 | 통과(제품 책임자) | **통과(제품 책임자) — 유지.** qa 판정 아님 | 기준 1·3·5-b는 **로그로 재현된다**(§3.2·§3.6). 기준 2는 **좌선회만 닫히고 우선회는 열려 있다**(§3.4) |
| **SC-89** | 미검증(증거 요건) | **PASS** — 단 **(f)가 발동해 architect 통지가 필요하다** | 자명 통과가 **실제로 해소됐다**(§1) |

### 0.1 리더가 옳았던 것

1. **`truncated=10`이 히치의 증거라는 주장은 성립한다.** 의심하라는 지시대로 `TickCatchUp.Compute`를 읽고 따졌고, **일반적 프레임 변동으로는 이 값이 절대 오르지 않는다**(§1.1). 리더가 맞다.
2. **부호 계산 3건이 전부 재현된다.** 기준 1 `d·fwd = 1.0000`(2구간), 기준 3 `+0.9991 / -0.9999`. 소수점까지 일치(§3.2).
3. **오토레벨 5-b가 §7b(1) 짝을 포함해 실제로 닫힌다.** 복귀 `-0.0523 → -0.0020`, 억제 `roll=-1000` 구간 ±0.9 진동 — 둘 다 재현됐고, **같은 세션의 인접 구간**이다(§3.6). **이 슬라이스에서 §7b(1) 짝이 자명 통과 없이 실제로 성립한 첫 사례다.**
4. **R5가 R16 이전 빌드라는 구분은 정확하다.** R5 로그에 `position_*`·`mouse_*`가 없음을 확인했다(§5.1).

### 0.2 리더가 틀렸거나 과한 것 — 4건

| # | 리더의 진술 | 실제 |
|---|---|---|
| **L-15** | "기준 2: `yaw_deg / mouse_dx_total = 0.12000` 16줄 전부 부호 보존" | **이 비율은 측정이 아니라 항등식이다.** `ShipInputSampler.cs:82-84`에서 `MouseDeltaXTotal += delta.x`와 `_yawDeg += delta.x * 0.12`가 **같은 `delta.x`를 연속 두 줄에서** 더한다. 비율은 **어떤 입력에도 0.12**이고 **부호가 어긋날 수 있는 실행 경로가 없다.** §7b(1) 자명 통과다. 게다가 16줄 중 **1줄은 `0.0/0.0`**이라 세면 안 된다(§3.4(1)) |
| **L-16** | "뱃머리가 직전의 우현 쪽으로 갔는가 — 일치 8건 / 불일치 0건" | **재현되지 않는다.** 같은 로그를 직접 파싱해 12개 비교 가능 구간을 얻었고 **일치 11 / 불일치 1**이다(§3.4(2)) |
| **L-17** | "Editor는 닫혀 있다" | **열려 있다.** Unity PID 20260, 기동 2026-09-24 10:59:27(= R6 촬영 세션). `unity test`가 거부됐다(§6.1) |
| **L-18** | 오토레벨 증거를 **"R6 세션에서"**로 귀속 | **오토레벨 증거는 R5에 있다.** R6의 16줄은 **전부 `roll=0`**이라 §7b(1)의 ② 구간이 없다(§3.6) |

### 0.3 결론 한 줄

**증거가 처음 생겼고, 그 증거가 SC-89를 실제로 닫았고, 같은 증거가 SC-56을 FAIL로 만들었다. 그리고 기준 2를 닫았다는 두 근거 중 하나(비율 0.12)는 대수적 항등식이었다 — 이 슬라이스가 여섯 번째로 같은 형태에 걸렸고, 이번엔 판정자 쪽에서 걸렸다.**

---

## 1. A — SC-89

### 1.1 (b)의 자명 통과가 실제로 해소됐는가 — **해소됐다. 리더가 옳다**

**지시대로 의심했다.** `truncated`가 일반적 프레임 변동으로도 오르는 값이면 조건 발생의 증거가 아니다. `client/Assets/_Project/Scripts/Flight/TickCatchUp.cs:137-146`을 직독했다:

```csharp
int backlogTicks = (int)Math.Floor(accumulatorSecondsBeforeThisFrame / tickDurationSeconds);
if (backlogTicks > maxPredictedTicksPerFrame)   // M = 20
    return new Plan(20, Math.Min(20, K), 0.0, truncated: true);
```

`Truncated == true`의 필요충분조건은 **`backlogTicks > 20`**, 즉 `accumulator > 1.0초`다. 호출부(`GreyboxSession.cs:622-629`)는 `_tickAccumulator += Time.unscaledDeltaTime` 직후 한 번만 계산하고, 잔여는 항상 `< 0.05초`로 리셋된다(truncate 시 `0.0`). **따라서 `truncated`가 1 오르려면 한 프레임의 `unscaledDeltaTime`이 0.95초를 넘어야 한다.**

**일반적 프레임 변동으로는 불가능하다.** 60 fps의 프레임 델타는 ~0.017초이고 0.95초는 그 **57배**다. 계약이 재현 벡터로 지정한 "450 ms 이상 히치"보다도 **두 배 큰** 사건이다.

**§7a 단언 — 겨냥한 조건이 실제로 발생했는가:** 발생했다. **R5에서 10회, R6에서 6회.** 그리고 카운터 하나가 아니라 **두 수의 동반 이동**으로 교차 확인된다 — 내가 54줄을 파싱해 뽑은 것:

```
truncated   0  1  2  3  4  5  6  7  8  9  10
carry_fwd   0 10 20 30 40 50 60 70 80 90 100     (= 10 × truncated, 정확히)
dormant     0 ................................. 93   (truncate 1회당 +9)
```

**`catchup_carry_forward_ticks_total`은 truncate 프레임에서만 움직였다** — 각 truncate 프레임이 20 tick을 예측하고 1건만 송신하므로 나머지 19 중 10이 carry-forward(상한 `carry_forward_max_ticks = 10`), 9가 dormant다. **이 정확한 비율이 `TickCatchUp` 계획이 실제로 실행됐다는 독립 증거다.**

**부수 발견:** carry_forward가 **정확히 10의 배수로만** 움직였다는 것은 **backlog 2~20짜리 중간 프레임이 한 번도 없었다**는 뜻이다. 프레임은 "1 tick 이하" 아니면 "1초 초과" 둘 중 하나였다. 이 세션의 히치는 짧은 딸꾹질이 아니라 **거대한 정지**다(§1.5a).

### 1.2 `max_sends_per_trailing_1s = 21`이 20 Hz에서 정상인가 — **정상이다. 그러나 코드 주석이 틀렸고 그것이 결함이다**

`client/Assets/_Project/Scripts/Greybox/SendBurstStats.cs:70-78`:

```csharp
double cutoff = nowSeconds - 1.0;
_sendTimestampsSeconds.RemoveAll(t => t < cutoff);   // t == cutoff 는 남는다
```

창이 **닫힌 구간 `[now-1.0, now]`**다. 송신이 tick마다(0.05초 간격) 일어나므로 이 창에는 **21개**가 들어간다(양 끝 포함). **20이 아니라 21이 이 구현의 정상값이다. 버스트가 아니다.**

**그러나 같은 파일의 XML 주석은 이렇게 말한다:**

> `/// At client_send_hz=20 with no hitch this should stay at 1 (one send per tick, ticks spread across the second) - anything higher within a burst is exactly what SC-89 watches for.`

**"should stay at 1"은 틀렸다.** 초당 20회 송신하면 1초 창에는 20~21개가 있다. **이 주석을 읽은 제3자는 `21`을 보고 "기대 1인데 21 — 21배 버스트"로 읽는다.** 리더가 이 수를 의심한 것 자체가 그 주석이 이미 한 명을 오도했다는 증거다.

**수정 요청 (F-16, client):** `SendBurstStats.cs:36-39`의 `should stay at 1`을 *"~20-21 (the window is closed on both ends, so a steady 20 Hz send rate yields 21) — a burst shows as a value well above that"*로 고친다. **프레임당 값(`MaxSendsPerFrame`)과 1초 창 값의 기대치가 다르다**를 한 줄로 분리한다.

### 1.3 `reconcile_forced_after_hitch_total = 0`인데 히치가 있었다는 주장은 양립하는가 — **양립한다**

두 수는 **서로 다른 것을 센다.**

| 카운터 | 세는 것 | 발동 조건 |
|---|---|---|
| `catchup_truncated_total` | **송신·예측 쪽** — 한 프레임 backlog > 20 tick | `unscaledDeltaTime > 0.95 s` |
| `reconcile_forced_after_hitch_total` | **수신·재조정 쪽** — `RebaseHold`가 500 ms 천장을 넘겨 미송신 이력을 강제로 버린 횟수 | 스냅샷이 **끊긴 채** 홀드가 500 ms 초과 |

R5·R6의 히치는 **클라이언트 프레임이 멈춘 것**이지 **스냅샷 스트림이 끊긴 것**이 아니다. WebSocket 수신은 별도 태스크이고 서버는 계속 20 Hz로 돌았다(`tick_overrun_total = 0`, `tick_lag_seconds = -0.0498`). 복귀 프레임에서 밀린 스냅샷이 한꺼번에 처리되므로 홀드가 500 ms를 넘길 일이 없다. **0이 정상이고, 두 수의 0/비0 조합이 오히려 "클라이언트만 멈췄다"를 특정한다.**

### 1.4 (a)~(f) 절별 판정

| 절 | 판정 | 증거 (전부 내가 직접 뜬 것) |
|---|---|---|
| **(a)** 프레임당 송신 ≤ 1 ＋ 드롭·`RATE_LIMITED` 델타 0 | **PASS** | `max_sends_per_frame = 1`(R5·R6). 서버 `commands_dropped_over_tick_cap_total = 0`, `RATE_LIMITED = 0`, `protocol_violations_total = 0`, 거부 8종 전부 0 — **두 세션 모두**. `commands_received_total = 4780`(R5) / `1223`(R6)이므로 **경로가 실제로 탔다**(0건 통과 아님) |
| **(b)** `carry_forward > 0` ∧ `hard_snap == 0` ＋ 드레인 max > 1 ＋ 판별 단언 송신/프레임 == 1 | **PASS — 단 `hard_snap == 0` 조항은 SC-56에서 FAIL이다** | 드레인 max = **20 > 1** ＋ 송신/프레임 = **1** → 계약이 §7b에서 지정한 판별 짝(수정 전 `>1 이고 >1`, 수정 후 `>1 이고 ==1`)이 **완성됐다**. `carry_forward = 100 / 61 > 0` ✓. **⚠ `hard_snap == 0`은 성립하지 않는다(18 / 39)** — 계약이 이 조항을 SC-89와 SC-56 양쪽에 걸었고, **SC-56 쪽에서 FAIL로 처리한다**(§2.2). SC-89가 재는 성질("송신이 프레임률이 아니라 벽시계를 따른다")은 판별 짝으로 닫혔다 |
| **(c)** 판정 도구가 사본이 아니라 진짜 코드를 잰다 | **닫힘(유지)** | `grep -n 'while (_tickAccumulator' GreyboxSession.cs` → **exit 1**(실행함). `GreyboxSendBurstTests.cs:123,185`가 `TickCatchUp.Compute`를 직접 호출 |
| **(d)** `outbound_queue_full_total`(관측) | **관측: 0** | R5·R6 모두 `outbound_queue_full_total = 0`, `prediction_history_overflow_total = 0`. 서버 `messages_dropped_total = 0`, `send_queue_depth_max = 3` |
| **(e)** 임계값 diff 0 | **닫힘(유지)** | `git status --porcelain` **빈 출력**(작업 트리 청결), `git diff --stat HEAD -- contracts/ data/movement/sync-tuning.json` **빈 출력** |
| **(f)** 잔여 관측 둘이 실플레이에서 0 | **⚠ 발동 — architect 통지 필요** | `catchup_truncated_total = 10`(R5) / **6**(R6) — **0이 아니다.** 계약: *"0이 아니면 tick 정렬 예측(미룬 결정, ADR-0012 §6.3)의 발동 조건이 충족된 것이므로 architect에게 알린다."* **리더는 이 조항을 보고하지 않았다.** `reconcile_forced_after_hitch_total`은 0 |

**§7a 단언 — 이 초록이 무엇을 보고 켜졌는가.** (a)의 `max_sends_per_frame == 1`은 **프로덕션 K=1 때문에 구조적으로 1을 넘을 수 없다** — 그 자체로는 거의 항등식이고, 실제로 재는 것은 *"계획을 우회하는 송신 경로가 없다"*뿐이다. **이 항목을 비-자명하게 만드는 것은 서버 쪽 수다**: 수정 전 바이너리라면 같은 10회 히치에서 `protocol_violations_total > 0`과 `commands_dropped_over_tick_cap_total > 0`이 나온다. 그 두 수가 **명령 4780건을 받은 세션에서 0**인 것이 판정의 무게를 지는 관측이다. **클라이언트 카운터 단독으로는 PASS 근거로 쓰지 않았다.**

### 1.5 1002 태그 (i)(ii)(iii)

| | 판정 | 근거 |
|---|---|---|
| (i) `close_code(ProtocolViolation) == 1002` | **닫힘(유지)** | `git status` 청결 — `server/crates/gateway/src/runtime.rs` 무변경 |
| (ii) `ReconnectPolicy` 양성 대조 | **미검증(환경)** — r7의 닫힘을 **재확인하지 못했다** | 이번 라운드에 EditMode를 실행하지 못했다(§6.1). r7이 실행한 증거는 유효하고 코드는 무변경이므로 **되돌리지는 않되, 이 라운드의 실행 증거는 없다** |
| (iii) 태그가 실제로 찍히고 DB `close_reason`과 짝지어진다 | **안 닫힘 (r6·r7에 이어 3라운드 연속)** | **R5·R6 둘 다 `protocol_violations_total = 0`**이고 종료 사유는 `ClientClosed`(R5 stdout)·`PLAYMODE_EXIT code=1000`이다. **실서버 세션이 생겼지만 위반을 내는 세션은 아니었다.** 지시대로 그대로 유지한다 |

### 1.5a 계약 외 발견 — **이 세션의 히치는 "딸꾹질"이 아니다**

§1.1의 배수 관계에서 나온다. R5는 tick 594988~600870(**5882 tick = 294초**) 구간에서 `commands_received_total = 4780`이다. **1100 tick 가까이가 서버에 도달하지 않았다.** truncate 1회가 버리는 tick은 `backlog - 20`이므로, 10회로 1100 tick을 버리려면 **평균 backlog ≈ 130 tick = 6.5초**다.

즉 **평균 6초대의 완전 정지가 5분 세션에 10번 있었다.** 서버 쪽 `input_carried_forward_total = 221` / `input_superseded_total = 112`이므로, 서버는 그 구간 대부분을 **휴면 입력으로** 함선을 돌렸다.

이것은 SC-89의 FAIL이 아니다(계약이 재는 성질은 닫혔다). **그러나 이 세션을 SC-56·SC-59의 증거로 읽을 때의 전제다** — §2·§3에서 쓴다. 그리고 **에디터가 6초씩 멈추는 원인 자체가 조사 대상이다**(기록).

---

## 2. B — SC-56

### 2.1 (d)①②③의 자명 통과 지적이 해소됐는가 — **해소됐다**

| 관찰 | 계약 요구 | R5 | R6 | |
|---|---|---|---|---|
| ① `HasError == true`인 `Reconcile()` 호출 수 | **0이면 FAIL** | **2388** | 609 | **닫힘** |
| ② 재생한 입력 수가 0이 아닌 호출 수 | 되감기만 한 것과 구분 | **84** | 20 | **닫힘** |
| ③ `ω_aim`·`ω_roll`이 둘 다 0이 아닌 스냅샷에서 재조정한 횟수 ≥ 1 | ≥ 1 | **438** | 218 | **닫힘** |

**84·438이 0이 아닌 것으로 충분한가 — ③은 충분하고, ②는 형식적으로 충분하되 그 값 자체가 별건 신호다.**

- ③은 계약이 `≥ 1`을 요구했고 438이다. **자명 통과 상태(0)와 명확히 갈린다.** 충분하다.
- ②는 계약이 "0이 아닐 것"만 요구했고 84다. 형식적으로 충분하다. **그러나 `84 / 2388 = 3.5 %`다** — 재조정의 96.5 %가 **재생할 입력이 하나도 없는 상태**였다. 20 Hz로 보내고 10 Hz로 스냅샷을 받으면 매 재조정에 미확인 입력이 1~2건 있는 것이 정상이다. **3.5 %는 클라이언트 예측이 서버보다 앞서 있지 않았다는 신호**이고, §2.3의 7.0 m와 같은 원인을 가리킨다. (계약 외 발견 — 판정에 쓰지 않는다)

### 2.2 `max = 1450.3 m`와 `hard_snap_total = 18` — **판정한다: SC-56은 FAIL이다**

리더는 "별도로 봐야 할 수치"라고만 적고 판정하지 않았다. **판정한다.**

**계약 SC-56 (c)는 `reconcile_hard_snap_total`이 0일 것을 요구한다.** 조건부가 아니고 허용 범위가 없다.

| | R5 (5.2분) | R6 (약 2분) | 기준 |
|---|---|---|---|
| `reconcile_hard_snap_total` (세션) | **18** | **39** | **== 0** |
| `position_error_m` p50 / p99 / max (n) | 0.0006 / 4.6529 / **1450.3056** (2388) | 0.0006 / **7.0005** / 316.1938 (609) | `reconcile_hard_snap_threshold_m = 5.0` |
| `orientation_error_deg` p50 / p99 / max (n) | 0.0000 / 0.0303 / **18.2943** (2388) | 0.0001 / 2.2019 / **20.1435** (609) | `reconcile_orientation_hard_snap_deg = 15.0` |

**재측정 면제 조건에 해당하지 않는다.** 계약: *"판정 전에 `close_reason`을 먼저 본다 … 서버가 정당하게 `SLOW_CONSUMER`로 끊으면 재측정이다."* R5 서버 stdout은 `reason=ClientClosed elapsed_ms=311362 dropped=0`, R6은 `PLAYMODE_EXIT code=1000`. **`SLOW_CONSUMER`가 아니다.** 계약이 지정한 측정 시점("SC-89의 catch-up 수정 뒤") 조건은 충족한다.

**M-9/S-6 목표와 대조한다.** 계약 §3.4: *"S-6 벽이 있지만 벽처럼 보이지 않는다 — **텔레포트·튕김·하드 스냅 0회**는 SC-56(c)의 `reconcile_hard_snap_total = 0`과 함께 본다."* **18회와 39회는 플레이어가 보는 텔레포트다.** 1450 m는 greybox 선체 길이의 50배가 넘고 140 m/s로 10초 이동하는 거리다. 이것은 "수치가 크다"가 아니라 **디자이너가 없다고 못 박은 현상이 실제로 일어났다**는 것이다.

**FAIL이다.** `미검증(증거 요건)`이 아니다 — 계약 §0.3의 그 칸은 *"테스트가 유효하게 실행되지 않은"* 경우이고, 여기서는 **관찰이 유효하게 수행됐고 기대와 어긋났다.**

**⚠ 계약 §7 규율을 그대로 적용한다: 임계값을 늘리지 않는다.** `reconcile_hard_snap_threshold_m = 5.0`을 올려 통과시키는 것은 SC-51/52에 대해 계약이 금지한 것과 같은 형태다. architect에게 알린다(A-2).

### 2.3 원인 추정 — **히치 탓만이 아니다. R6에 깨끗한 반례가 있다** (계약 외 발견, 원인 규명용)

"에디터가 6초씩 멈췄으니 하드 스냅은 당연하다"로 닫으면 틀린다. **R6 로그를 truncate 델타와 함께 읽으면 갈린다**(16줄 전수 파싱):

```
row3 tick=607624  trunc=2  snap= 1  speed=140.0  err_m=0.0004
row4 tick=607724  trunc=2  snap=19  speed=140.0  err_m=7.0005   <- dTrunc=0, dSnap=+18
row5 tick=607824  trunc=2  snap=32  speed=140.0  err_m=7.0003   <- dTrunc=0, dSnap=+13
row6 tick=607926  trunc=3  snap=36  speed=109.9  err_m=0.0010
```

**row3→row5 구간에 truncate가 한 번도 없다.** 히치가 없는 10초 동안 **하드 스냅이 31회** 났고, 그동안 위치 오차가 **7.000 m에 고정**됐다.

`7.0005 m`는 우연한 수가 아니다. `140.0 m/s × 0.05 s (= 1 tick) = 7.000 m`. **클라이언트 예측이 서버보다 정확히 1 tick 뒤처진 상태가 10초간 지속됐고**, 그 오차가 임계 5.0 m를 넘으므로 **스냅샷마다 하드 스냅**이 났다. 속도가 109.9로 떨어지자(row6) 1 tick 오차는 5.5 m → 곧 0.001로 회복됐다.

**대조 증거 — 같은 속도에서 정상인 구간이 R5에 있다.** R5 row32~49는 `speed = 140.0`, `thrust_z = 1000`이 18줄 연속인데 `err_m`이 **0.0006~0.0008**이다. **같은 속도에서 오차가 0.0007일 수도, 7.0005일 수도 있다** — 속도 때문이 아니라 **tick 정렬이 한 칸 어긋난 채 고착되기 때문**이다.

이것은 계약 SC-89 (f)가 가리키는 **ADR-0012 §6.3의 미룬 결정(tick 정렬 예측)**과 정확히 같은 자리다. §1.4 (f)의 통지와 묶어 architect에게 보낸다.

**재현 방법(client·architect용):** `evidence/R6-signs/Editor-session.log`의 row3~row6(tick 607624~607926). 기대: `err_m`이 임계 아래. 실제: 7.0005 m 고정, `reconcile_hard_snap_total` +31. 관련 코드: `Flight/PredictedShipController.cs:89-111` → `Reconciliation.Reconcile`의 `ackInputSeq` 기준 이력 절단. §2.1의 "재생 입력 0인 재조정 96.5 %"가 같은 원인의 다른 얼굴로 보인다.

### 2.4 세션 스코프와 run 스코프 분리가 요건대로인가 — **형식은 맞다. 그러나 이 세션은 분리를 검증하지 못한다**

두 줄이 요건대로 따로 찍힌다:

```
SC-56 session-end ... position_error_m(... n=2388), orientation_error_deg(... n=2388), reconcile_hard_snap_total=18
SC-56 run-scope ... (NOT the SC-56 judging value, context only) - reconcile_hard_snap_total_run=18,
                     reconcile_error_m_max_run=1450.3056 (n=2388), reconcile_error_deg_max_run=18.2943 (n=2388)
```

- `max` 옆에 `n`이 **항상 붙어 있다** — 계약이 요구한 규율(*"`max` 혼자서는 '더 큰 표본이 안 들어왔다'와 '지표가 멈췄다'를 구분하지 못한다"*) 충족.
- 판정값이 세션 범위임을 줄 자체가 명시한다 — 충족.

**§7a 단언 — 그러나 이 관찰이 겨냥한 조건은 발생하지 않았다.** 범위 분리가 존재하는 이유는 **재접속을 넘어 누적되는 값과 세션 값이 갈리는 것**인데, R5·R6 모두 `sessions_opened_total = 1` / `sessions_closed_total = 1`이고 **재접속이 0회**다. 그래서 두 값이 `18 == 18`, `39 == 39`로 **같다.** 라운드 4의 범위 불일치가 실제로 고쳐졌는지는 **이 증거로 확인되지 않는다** — 재접속이 있는 세션이 필요하다. **형식 충족은 인정하되 닫힘으로 적지 않는다.**

---

## 3. C — SC-59 (a)(b) — 제품 책임자 통과는 뒤집지 않는다

### 3.1 재현 방법

`evidence/R6-signs/Editor-session.log`의 `periodic_status` 16줄과 `evidence/R5-realserver/`의 54줄을 정규식으로 전부 파싱하고, 쿼터니언 회전을 Hamilton 곱(`v' = q·(v,0)·conj(q)`, Unity `(x,y,z,w)` 순서)으로 직접 구현해 재계산했다. **리더의 값을 인용하지 않았다.**

### 3.2 기준 1·3 — **리더의 계산이 재현된다**

```
기준 1  추력 (0,0,1000), 함선 로컬 +Z 축에 변위 투영
  row3->4  |d|=700.1 m  d.fwd(시작자세)=+1.0000  d.fwd(끝자세)=+1.0000
  row4->5  |d|=706.9 m  d.fwd=+1.0000            d.fwd=+1.0000
기준 3  추력 (0,+1000,0), 함선 로컬 +Y 축에 투영
  row8->9   |d|=227.3 m  d.up=+0.9381  (+0.9970)   <- 속도 23.9->80.0, 관성 잔재
  row9->10  |d|=570.4 m  d.up=+0.9991  (+0.9973)
기준 3  추력 (0,-1000,0)
  row12->13 |d|= 92.6 m  d.up=-0.9956  (-0.9936)
  row13->14 |d|=234.2 m  d.up=-0.9999  (-1.0000)
  row14->15 |d|=331.7 m  d.up=-0.9893  (-0.2652)   <- 이 구간에 큰 자세 변화
```

**리더가 보고한 `1.000`(2구간)·`+0.999`·`-1.000`이 전부 나온다.** 리더는 각 계열에서 **가장 깨끗한 구간 하나씩**을 보고했고, 내가 전 구간을 돌린 결과 **어느 구간도 반대 부호를 주지 않았다.** 선별 보고였지만 선별이 결론을 바꾸지 않았다.

### 3.3 "규약 무관"이라는 리더의 주장이 참인가 — **손 방향에 대해서는 참이다. 그러나 숨은 가정이 둘 있다**

| | 판정 |
|---|---|
| **손 방향(좌·우수 좌표계) 가정을 피하는가** | **피한다.** `d · rot(q, (0,0,1))`은 두 양을 같은 기저에서 내적할 뿐이고, 기저가 좌수든 우수든 값이 같다. **리더가 맞다** |
| **숨은 가정 ① — "함선 로컬 +Z가 렌더링된 뱃머리다"** | **피하지 못한다.** 이 계산이 닫는 명제는 *"`thrust_z`가 함선 로컬 +Z 방향의 운동을 만든다"*이고, 그것은 **같은 적분기가 자세와 위치를 둘 다 만들기 때문에 거의 자동으로 성립한다**(`Sim/ShipIntegrator.cs:152-155`가 로컬 축에 추력을 얹고 `attitude_*`가 그 로컬 축의 월드 표현이다). SC-59 (a)가 묻는 것은 *"W를 누르면 **화면에 보이는 뱃머리** 쪽으로 간다"*이고, **로컬 +Z와 메시의 코가 같은 방향인지는 로그가 말하지 않는다.** 씬·프리팹 사실이고 **사람 눈이나 EditMode 씬 단언이 필요하다** |
| **숨은 가정 ② — 기준 3의 "위"가 함선 상방인가 월드 상방인가** | 계산은 **함선 +Y**로 했고 그것이 R/F 키의 정의(`ShipInputSampler.cs:104-105`)와 맞는다. 계약 문구 *"위 추력이 위로 민다"*가 월드 up을 뜻한다면 이 계산은 그것을 닫지 않는다. **architect에게 문구 확인 요청** |

### 3.4 기준 2 — **리더의 근거 하나는 항등식이고 다른 하나는 재현되지 않는다. 그러나 별도 방법으로 닫힌다**

#### (1) `yaw_deg / mouse_dx_total = 0.12000`은 **측정이 아니다** (L-15)

`client/Assets/_Project/Scripts/Greybox/ShipInputSampler.cs:82-84`:

```csharp
MouseDeltaXTotal += delta.x;
MouseDeltaYTotal += delta.y;
_yawDeg += delta.x * MouseSensitivityDegPerPixel;   // = 0.12
```

**두 누적기가 같은 `delta.x`를 연속 두 줄에서 더한다.** `yaw_deg == 0.12 × mouse_dx_total`은 **모든 입력에 대해 대수적으로 참**이고, 부호가 어긋날 수 있는 실행 경로가 **존재하지 않는다.** 16줄 전부에서 `0.12000`이 나오는 것은 관찰이 아니라 **산술의 재확인**이다(내가 16줄 전부 돌려 확인했다 — 그중 row0은 `0.0/0.0`이라 비율이 정의되지 않는다. **"16줄 전부"는 15줄이다**).

같은 파일 `:55-57`의 주석 *"if the two ever disagree in sign, that disagreement is the SC-59 bug"*도 **참이 될 수 없는 문장**이다. 계약 §7b의 "자명 불통과"의 거울상 — **어떤 입력으로도 빨간불이 켜지지 않는다.**

**단, R16이 이 필드들을 넣은 판단 자체는 옳았다.** 쓸모는 비율이 아니라 **`mouse_dx_total`이 매핑 *이전*의 원시 델타라는 점**에 있고, 그것이 (3)의 판독을 가능하게 한다.

#### (2) "뱃머리가 직전의 우현 쪽으로 — 일치 8 / 불일치 0"은 **재현되지 않는다** (L-16)

연속 두 줄에 대해 `dmx = Δmouse_dx_total`, `right_a = rot(q_a,(1,0,0))`, `fwd_b = rot(q_b,(0,0,1))`를 잡고 `sign(dmx)`와 `sign(fwd_b · right_a)`를 대조했다(`dmx == 0`인 2구간 제외):

```
 1->2  dmx= -108.0  proj=-0.0365  turn= 2.1deg  AGREE
 2->3  dmx= -106.0  proj=-0.2800  turn=53.7deg  AGREE
 5->6  dmx=  +57.0  proj=+0.0176  turn=56.9deg  AGREE
 6->7  dmx= -351.0  proj=-0.6481  turn=45.4deg  AGREE
 7->8  dmx= -322.0  proj=-0.3924  turn=23.1deg  AGREE
 8->9  dmx=  +17.0  proj=-0.0119  turn=17.3deg  DISAGREE   <-
 9->10 dmx= -222.0  proj=-0.3610  turn=21.2deg  AGREE
10->11 dmx=  -76.0  proj=-0.0203  turn=58.2deg  AGREE
11->12 dmx= -138.0  proj=-0.3415  turn=88.5deg  AGREE
12->13 dmx=  +49.0  proj=+0.1018  turn=13.2deg  AGREE
13->14 dmx=   -2.0  proj=-0.0043  turn= 1.0deg  AGREE
14->15 dmx=  -24.0  proj=-0.0056  turn=83.0deg  AGREE
                                    일치 11 / 불일치 1
```

**리더의 "8 / 0"이 나오지 않는다.** 구간 수도(12 vs 8) 불일치 수도(1 vs 0) 다르다. 어떤 필터를 썼는지 문서에 없으므로 재현할 수 없다.

불일치 구간(8→9)은 **설명 가능하다**: 그 구간의 yaw 변화는 `+2.04°`인데 pitch 변화가 `-9.36°`로 4배 크고, 함선은 선회율 상한(75 deg/s) 때문에 **직전 구간의 조준 목표를 아직 쫓는 중**이었다(자세 회전 17.3°). 5초 샘플링에서 자세는 그 구간의 마우스가 아니라 **누적 조준 목표**를 따라간다. **즉 이 판독법 자체가 5초 창에서 신뢰도가 낮다** — 리더가 이 방법을 근거로 삼은 것이 방법론적 약점이다(F-14와 직결).

#### (3) **그래서 기준 2는 닫히는가 — 방위각으로 닫힌다. 좌선회는 확실히, 우선회는 약하게**

누적량끼리 대조하면 5초 샘플링 문제가 사라진다. 각 줄에서 `azimuth = atan2(fwd.x, fwd.z)`(ADR-0009 §1의 **+X = 오른쪽, +Z = 전방** 정의만 쓰고 손 법칙은 쓰지 않는다 — azimuth 증가 = 뱃머리가 +X 쪽 = 우선회)를 조준 목표와 자세 각각에 대해 계산해 `yaw_deg`와 비교했다:

```
 row   yaw_deg   azim(aim)   차이      azim(attitude)  차이
   1      0.72       0.72    -0.00          3.99      +3.27
   2    -12.24     -12.24    +0.00        -11.55      +0.69
   3    -24.96     -24.96    +0.00        -24.96      +0.00
   6    -18.12     -18.12    -0.00        -11.05      +7.07
   7    -60.24     -60.24    -0.00        -72.59     -12.35
   8    -98.88     -98.88    +0.00        -99.34      -0.46
  10   -123.48    -123.48    -0.00       -122.83      +0.65
  12   -149.16    -149.16    +0.00       -149.15      +0.01
  15   -146.40    -146.40    +0.00       -146.12      +0.28
```

- **`azim(aim) == yaw_deg`가 15줄 전부에서 소수점 둘째 자리까지 일치한다** → `YawPitchToQuaternion`이 yaw를 월드 +Y 둘레로 **부호 보존해** 적용한다.
- **`azim(attitude)`가 `yaw_deg`를 추종한다** — 정지 구간에서 오차 < 1°, 급기동 중 최대 12° 지연(선회율 상한) → **서버가 조준 목표를 부호 보존해 자세로 옮긴다.**
- **`mouse_dx_total` → `yaw_deg`는 항등식이지만, 그 항등식의 계수가 `+0.12`(양수)라는 것이 코드에 박혀 있다.** 즉 **마우스 오른쪽(+dx) → yaw 증가 → azimuth 증가 → 뱃머리가 +X(오른쪽)로.** 세 고리가 전부 연결된다.

**§7a 단언 — 겨냥한 조건이 실제로 발생했는가:** 좌선회는 발생했다. `yaw_deg`가 `0 → -149.16`까지 갔고 **azimuth가 같은 부호·같은 크기로 따라갔다.** **우선회는 세 구간뿐이고 크기가 `+0.72° / +2.04° / +5.88°`다.** 그중 `+2.04°` 구간은 자세가 따라오지 않았다((2)의 DISAGREE와 같은 구간).

**판정:** **전역 부호 반전 버그는 배제된다**(반전 빌드였다면 좌선회 149°가 반대 부호로 찍혔다). **좌우 비대칭 버그는 배제되지 않는다** — `+dx`에만 걸리는 클램프·데드존·부호 처리 결함은 최대 5.9°짜리 표본 2건으로 걸러지지 않는다. **기준 2는 "전역 반전 없음"까지만 닫힌 것으로 본다.**

**수정 요청 (L-19, 리더):** 다음 촬영에 **우선회를 좌선회와 같은 크기(누적 90° 이상)로** 한 번 넣는다. `yaw_deg`가 누적값이라 5초 주기와 무관하게 읽힌다 — **절차 한 줄이면 닫힌다.**

### 3.5 로그가 대체하지 못하는 것 (§0.10과 함께 읽는다)

| 기준 | 로그로 닫히는 것 | 로그가 닫지 못하는 것 |
|---|---|---|
| (a) 전방 추력 | `thrust_z`가 로컬 +Z 운동을 만든다 | **로컬 +Z가 화면의 뱃머리인가** — 메시·프리팹 사실 |
| (a) 마우스 우선회 | 전역 부호 반전 없음 | **좌우 비대칭**, 그리고 "화면의 오른쪽"이 로컬 +X인가 |
| (a) 위 추력 | `thrust_y`가 로컬 +Y 운동을 만든다 | 로컬 +Y가 화면의 위인가 |
| (b2) 마커 가시성 | `reference_markers` 1줄로 4개의 ID·거리 | **촬영 구간 전체에 최소 1개가 화면에** — 영상 필요 |
| (c) 경계 | `boundary_soft_crossed=true` (R5 row51, `origin_distance_m=11961.3`) | **soft 경고·hard 미끄러짐의 화면 표현** |
| (d) 오토레벨 | **전부 닫힌다** (§3.6) | — |

### 3.6 오토레벨(5-b) — **§7b(1) 짝이 성립한다. 이 슬라이스에서 처음이다**

**먼저 귀속 정정(L-18):** 리더는 이것을 R6 세션 성과로 적었지만 **R6의 16줄은 전부 `roll=0`이다.** 억제 구간(②)이 없다. **오토레벨 증거는 R5에 있다.**

적분기(`Sim/ShipIntegrator.cs:128-139`)의 `sinErr`를 그대로 옮겨 R5 54줄에서 재계산했다:

```
u      = (0,1,0) - fwd * dot((0,1,0), fwd),   fwd = rot(q, (0,0,1))
sinErr = dot( cross( rot(q,(0,1,0)), u/|u| ), fwd )
```

```
row  tick     roll    assist  sinErr    nu(게이트2)
 21  597438      0     true   -0.0523   0.8356     <- 손 뗌, 뱅크 상태
 22  597538      0     true   -0.0020   0.8356     <- 수평 복귀  (1)
 23  597638      0     true   -0.0020   0.8356
 24  597716      0     true   -0.0020   0.8356
---- Q 유지 시작 ----
 25  597938  -1000     true   +0.4201   0.8277     <- 억제  (2)
 26  598038  -1000     true   +0.9092   0.8255
 27  598138  -1000     true   -0.4126   0.8255
 28  598238  -1000     true   -0.9126   0.8255
 29  598338  -1000     true   +0.4052   0.8255
 30  598438  -1000     true   +0.9158   0.8255
---- 손 뗌 ----
 31  598490      0     true   -0.3981   0.8255
 32  598652      0     true   -0.0026   0.8255     <- 다시 수평 복귀  (1)
 33~50          0     true   -0.0019   0.8255     <- 18줄 유지
```

**리더의 두 수(`-0.0523 → -0.0020`, `roll=-1000` 구간 ±0.9)가 정확히 재현된다.**

**§7b(1) 짝이 성립하는가 — 성립한다:**

- **① 복귀:** `roll=0` ∧ `flight_assist=true` ∧ `nu=0.8356`(게이트 2 열림 — 선수가 월드 up과 평행하지 않다)에서 `|sinErr|`가 `0.0523 → 0.0020`으로 수렴하고 유지된다. **두 번 일어난다**(row21→22, row31→32).
- **② 억제:** `roll=-1000` 6줄에서 `|sinErr|`가 `0.42~0.92` 사이를 진동하며 **한 번도 0으로 수렴하지 않는다.** 같은 세션, 복귀 구간 **바로 다음 인접 구간**이고, `flight_assist`는 그동안 `true`로 유지된다 — **즉 "어시스트를 껐더니 안 돈다"가 아니라 "어시스트가 켜진 채 수동 롤이 이기는가"를 정확히 잰다.** 계약 (d)②가 겨냥한 것이 바로 이것이다.
- **§7a 단언:** 겨냥한 조건이 실제로 발생했다. 게이트 2(`nu ≥ AutoLevelDeadzoneSin`)가 **0.83으로 넉넉히 열려 있어** "수직 상승이라 오토레벨이 정당하게 쉰 것"이 아니고, 억제 구간의 `|sinErr|` 최대가 0.92로 **데드존보다 두 자릿수 크다.**

**R6 단독으로는 이것을 못 한다.** `roll≠0` 줄이 0개이므로 R6만 있으면 ②가 없고 §7b(1) 자명 통과였다. **R5·R6 둘이 있어야 SC-59 전체가 로그로 읽힌다.**

**부수 관찰:** row33~50(18줄, 약 80초) 동안 `sinErr = -0.0019`와 `nu = 0.8255`가 **한 자리도 변하지 않는다.** 조작자가 자리를 비운 구간이고, 오토레벨이 데드존 안에서 정지한 정상 거동이다(`speed_mps=140.0`·`thrust_z=1000`이 유지되므로 함선은 계속 움직였다).

---

## 4. D — R14 미대응 7건의 현재 상태

| # | r7의 요구 | 현재 | 판정 |
|---|---|---|---|
| **F-13** | 축 규약 문장의 `right-hand-rule about +Z` 제거 | `PeriodicStatusLog.cs:105-116` — *"Do NOT reach for the right-hand rule … ADR-0009 §1 fixes a LEFT-handed frame … the named rule that matches it is the left-hand one"* ＋ **두 번 틀렸던 이력까지 기록** | **닫힘** |
| **L-12** | `foreground` → `background` ＋ ①을 건너뛴 사실 명시 | `PeriodicStatusLog.cs:24-30` — *"the contract's 2-4 Hz throttle is a BACKGROUND vector; its foreground vectors are domain reload, heavy scene load and a forced GC loop"* ＋ *"that measurement has not been run, and 'runInBackground: 0 therefore it pauses' is an inference"* | **닫힘** |
| **F-15** | `Sample()`의 두 bool을 엇갈리게 | `PeriodicStatusLogTests.cs:50-51` — `boundarySoftCrossed: true, flightAssist: false`. `:231-234`에 *"deliberately opposite (qa r7 F-15)"* ＋ 두 값 각각 단언 | **코드상 닫힘 — 실행 재확인 불가**(§6.1). 주입 시험을 하지 못했다 |
| **F-12′** | *"can never disagree"* 축소 | `PeriodicStatusLog.cs:92-96` — *"with one divergence worth knowing … R14's comment claimed the two 'can never disagree'; that was too strong"* | **닫힘** |
| **F-11** (brake 절반) | `brake`를 로그에 | `grep brake PeriodicStatusLog.cs` → **0건.** HUD(`GreyboxSession.cs:923`)에만 있다 | **미대응 (유지)** |
| **F-10** | `v'=q·(v,0)·conj(q)` ＋ `attitude_*`가 **월드 프레임**임 ＋ 판독 예제 | R16 블록(`:150-166`)이 *"projects the displacement onto the ship's forward axis"*라는 **판독 방법**은 적었다. **그러나 (1) 회전 적용 형태가 없고 (2) `attitude_*`·`position_*`이 월드 프레임이라는 말이 한 글자도 없다** | **부분 대응.** 나는 §3을 쓰려고 `Quatd.cs`·`GreyboxSession.cs`를 열어야 했다 — F-8의 취지("소스를 열지 않고 판정") 미충족 |
| **F-14** | 로그 주기 5.0 → 1.0 | `GreyboxSession.cs:653` — 여전히 `5.0` | **미대응. 다만 심각도가 낮아졌다** — R16의 `yaw_deg`·`mouse_*_total`이 **누적값**이라 §3.4(3)의 판독은 `q_rel` 없이 되고 180° 뒤집힘 문제를 우회한다. **§3.4(2)의 자세 변화 판독에는 여전히 치명적이다**(리더의 8/0이 재현되지 않은 직접 원인) |
| **L-10** | `sc59-v2/13-correction-leader-2.md` 생성 | `ls evidence/R4-B6/sc59-v2/` → `12-correction-leader.md`까지. **13번 파일 없음.** R13 "남은 것"(`03_client_impl.md:1560-1568`)에도 **5-b 문장이 없다** | **미대응 (유지) — 지시대로 확인했다** |
| **L-8** | `HudTextWrap.cs` 헤더 과장 축소 | `:12` — *"A field growing tomorrow makes MORE rows, never a shorter, silently-clipped one."* 그대로 | **미대응 (유지)** |
| **L-7** | R13 md5 표에 유효 시점 주석 | R14 표(`:1649-1650`)에는 있다. **R13 표에는 없고, R16 표에도 같은 단서가 빠졌다** | **미대응 (유지, 경미)** |
| **L-11** | R13 F-1 "RED 1건" → 2건 | `03_client_impl.md:1485` — *"1건 실패"* 그대로 | **미대응 (유지)** |
| **F-2′** | `ObserverSession` 배선 테스트 | 코드 무변경(md5 `d2f5…` 일치). 테스트 추가 흔적 없음 | **미대응 (유지)** — 실행 확인은 §6.1로 불가 |

**대응 4건 / 부분 2건 / 미대응 7건.** R16은 **R5 세션이 드러낸 구멍(부호 판독 필드)에 집중했고**, r7 항목 중 우선순위가 높은 넷(F-13·L-12·F-15·F-12′)을 닫았다. **이번에도 "남은 것" 절에 미대응 항목이 이름으로 나열되지 않았다** — R16 "남은 것"은 세션 재촬영 이야기뿐이다. **L-13이 두 라운드 연속 미대응이다.**

---

## 5. 증거의 빌드 귀속

### 5.1 R5 = R16 **이전** 빌드 — 확인됐고, **리더의 구분은 정확하다**

```
$ grep -c "position_x"     evidence/R5-realserver/Editor-session.log   -> 0
$ grep -c "mouse_dx_total" evidence/R5-realserver/Editor-session.log   -> 0
$ grep -c "position_x"     evidence/R6-signs/Editor-session.log        -> 16
```

R5의 첫 줄 `attitude_w=0`(R16이 고친 결함), R6는 `attitude_w=1000000`(항등). **두 세션의 빌드가 다르다는 것이 로그 자체로 증명된다.**

| | R5 | R6 |
|---|---|---|
| 빌드 | R16 **이전** | R16 **이후** |
| 길이 | 311.4초 (서버 stdout `elapsed_ms=311362`) | 약 110초 (tick 607124~608828 = 1704 tick) |
| 닫는 것 | **SC-89 전 절**, **SC-56 (b)(c)(d)**, **SC-59 (d) 오토레벨 ①②** | **SC-59 기준 1·2·3** (위치 벡터·마우스 원시 델타) |
| 닫지 못하는 것 | 기준 1·2·3 (필드 없음) | 기준 5-b (`roll≠0` 구간 0개) |

**리더가 귀속을 틀린 곳은 오토레벨 하나다**(L-18). 그 외는 정확하다.

### 5.2 두 세션 모두 SC-59/SC-89 절차 제약을 지켰는가 — **지켰다**

계약: *"SC-59와 SC-89를 같은 세션으로 촬영할 수 없다."* 두 세션 모두 `application_focused=true`가 **전 줄에서 유지**된다(R5 54줄, R6 16줄 전수 확인). 즉 **백그라운드 구간이 없다.** SC-89의 증거는 계약이 지정한 대로 **화면이 아니라 카운터**에서 나왔고, 히치 벡터는 백그라운드가 아니라 **전경에서 자연 발생한 6초대 정지**였다(§1.5a) — 계약이 ②로 허용한 전경 벡터(도메인 리로드·무거운 씬 로드·GC 루프)와 같은 계열이다. **판정에는 문제가 없다: (b)의 판별 짝은 재현 벡터와 무관하다**(계약이 (g)에서 명시한 것과 같은 논리).

---

## 6. E — 전체 스위트와 기준선

### 6.1 **`unity test`를 실행하지 못했다 — 리더의 전제가 틀렸다 (L-17)**

```
$ unity test client --mode EditMode --report-format nunit,junit \
    --output .../unity-tests-qa-r8/EditMode.nunit.xml --junit-output .../EditMode.xml
Error: "C:\WorkSpace\SpaceHistoric\client"의 프로젝트가 실행 중인 에디터(PID 20260)에서 이미 열려 있습니다.
        에디터를 닫고 명령을 다시 실행하세요.

$ Get-Process -Id 20260
  Id : 20260   ProcessName : Unity   StartTime : 2026-09-24 오전 10:59:27
```

**10:59:27은 R6 촬영 세션의 Editor 기동 시각이다**(`evidence/R6-signs/unity-open.log`의 타임스탬프와 일치). **그 Editor가 지금도 떠 있다.**

**Editor를 닫지 않았다.** CLAUDE.md는 에이전트가 Play 버튼을 누를 수 없다고 못 박고, 이 인스턴스는 사람이 연 촬영 세션이다. 되돌리기 어려운 외부 상태를 qa가 임의로 종료하지 않는다. **사람이 닫아야 하는 항목으로 §9에 올린다.**

**결과:**

- **SC-46 → 미검증(환경).** r7의 `243/241/0/2`는 그 라운드의 유효 증거이고 되돌리지 않지만, **리더가 보고한 `246/244/0/2`는 이 라운드에 확인되지 않았다.**
- **§0.12 결함 주입 시험을 한 건도 하지 못했다.** F-15·F-2′·R16 신규 테스트 3건의 검출력은 **정적 읽기까지만** 확인됐다. 계약 §0.3: *"정적 읽기만으로는 PASS가 아니다."*
- 1002 (ii) 양성 대조 재확인 불가(§1.5).

**주입 결함 잔존 0건** — 주입을 하지 않았으므로 표식도 없다:

```
$ grep -rn "QA-R8 TEMP DEFECT" client/ tools/ tests/   -> 0건
$ git status --porcelain                               -> 빈 출력 (작업 트리 청결)
```

### 6.2 R16 md5 표 7건 — **7건 전부 현재 트리와 일치한다**

```
2a5c79e03ce1f44632ddf3cd97207948  GreyboxSession.cs        (표와 일치)
0731aa15fedeef1c18175d0c8e973927  PeriodicStatusLog.cs     (일치)
ee0256ff89f0c212fe04a95b2f96b8ae  ShipInputSampler.cs      (일치)
21fb3b1fa6d551255a72c01e978610bb  MarkerHudLine.cs         (일치)
adaddf93b2a16354bcee1d14bd7af4db  PendingRebaseSlot.cs     (일치)
47e66ef800479426bcae4de45984305d  SnapshotRebaseBatch.cs   (일치)
d2f54c230665116a0f37e96bfabddca3  ObserverSession.cs       (일치)
```

`git status --porcelain`이 빈 출력이므로 **이 md5는 커밋 `927fe07`의 상태**다 — 표가 유효하다. **다만 R16 표에는 R14 표에 붙었던 "이 표는 R16 종료 시점 기준" 단서가 없다**(L-7과 같은 형태).

---

## 7. F — 역방향 게이트

```
$ python tests/e2e/check_contract_items.py --selftest
OK   위반 — 리포트가 SC-87을 판정했는데 계약에 없다 (p1-01 실제 사례) :: 위반=[87] 기대=[87]
OK   깨끗 — 판정된 것이 전부 계약에 있다 :: 위반=[] 기대=[]
OK   순방향은 위반이 아니다 — 계약에 있으나 이번 라운드에 안 나온 항목(대기) :: 위반=[] 기대=[]
OK   산문 속 언급은 세지 않는다 — 'SC-99는 다음 라운드에' 같은 참조 :: 위반=[] 기대=[]
OK   리포트 여러 개를 합쳐 본다 :: 위반=[87] 기대=[87]
selftest: PASS  케이스=5
```

**본 게이트를 이 리포트에 대해 실행했다:**

```
$ python tests/e2e/check_contract_items.py --contract .../02_sprint_contract.md --report .../08_qa_report_r8.md
계약 항목: 89
리포트가 판정표 행으로 다룬 항목: 4
계약에 있으나 이 리포트들에 안 나온 항목: 85 (위반 아님 — 대기·블록 분할)
위반 없음 — 판정된 항목이 전부 계약 표에 있다.
exit 0
```

**판정 행이 실제로 4건 파싱됐다**(SC-46·SC-56·SC-59·SC-89 — §0의 네 행). `F-*`·`L-*`·`A-*`는 SC 번호가 없는 계약 외 항목이므로 게이트 대상이 아니다.

**§7a 단언:** selftest 케이스 1이 SC-87을 **실제로 검출**하고 케이스 2가 깨끗한 입력에서 **검출하지 않는다** — 두 방향이 갈린다. 파서가 죽어 있으면 케이스 1이 `위반=[]`로 떨어져 FAIL이 난다. **"0건 파싱 후 위반 0"이 아니다.**

---

## 8. 결함·수정 요청

### 8.1 architect — **통지 2건 (계약이 명시적으로 요구)**

| # | 내용 |
|---|---|
| **A-1** | **SC-89 (f) 발동.** `catchup_truncated_total = 10`(R5) / `6`(R6)로 **0이 아니다.** 계약 SC-89 (f): *"0이 아니면 tick 정렬 예측(미룬 결정, ADR-0012 §6.3)의 발동 조건이 충족된 것이므로 architect에게 알린다."* **리더 보고에 이 조항이 없었다** |
| **A-2** | **SC-56 FAIL에 임계값 조정으로 대응하지 않는다.** `reconcile_hard_snap_threshold_m = 5.0` / `reconcile_orientation_hard_snap_deg = 15.0`은 그대로 둔다(§7 규율). **원인은 §2.3의 1 tick 정렬 어긋남으로 보이고, 그것은 A-1과 같은 자리다** — 두 통지를 묶어 판단이 필요하다 |
| (이월) | **SC-59 (a)에 롤 방향이 없다.** §3.6에서 롤 부호의 *효과*는 읽혔지만(억제 거동), *방향*(E → 우현 상승)은 계약 문구에 없다 |
| (신규) | **SC-59 기준 3의 "위"가 함선 상방인지 월드 상방인지** 계약 문구가 가르지 않는다(§3.3) |
| (이월) | §0.3에 "제품 책임자 판정" 칸이 없다 |

### 8.2 client

| # | 위치 | 문제 | 기대 |
|---|---|---|---|
| **F-18** (최우선) | `Flight/PredictedShipController.cs:89-111` ＋ `Reconciliation.Reconcile` | **SC-56 FAIL의 원인.** R6 tick 607624~607926에서 **히치 없이**(`dTrunc=0`) 위치 오차가 `7.0005 m = 140 m/s × 1 tick`에 **10초간 고정**되고 하드 스냅 31회(§2.3). 같은 속도의 R5 row32~49는 `0.0007 m`다 — **재현되지만 항상은 아닌 1 tick 정렬 어긋남.** 동반 신호: 재조정 2388건 중 **재생 입력이 있는 것이 84건(3.5 %)** | 재현: R6 로그 row3~6. **ack tick과 클라이언트 최신 예측 tick의 관계를 한 줄 로그로 내서** 어긋남이 언제 생기는지 특정한다 |
| **F-17** | `Greybox/ShipInputSampler.cs:55-57`, `Greybox/PeriodicStatusLog.cs` R16 블록 | *"Sign disagreement between the pair IS the bug"*가 **참이 될 수 없다.** `MouseDeltaXTotal`과 `_yawDeg`는 `:82-84`에서 **같은 `delta.x`를 연속 두 줄에서** 더하므로 비율이 항상 `+0.12`다(§3.4(1)). **주석이 제3자에게 자명 통과를 판정 근거로 쓰라고 지시한다** | *"두 값은 같은 `delta.x`에서 나오므로 서로 어긋날 수 없다. 이 둘이 여는 것은 매핑 **이전**의 원시 부호를 로그에 남기는 것이고, 대조 상대는 `attitude_*`의 방위각이다"* ＋ §3.4(3)의 판독법을 예제로 |
| **F-16** | `Greybox/SendBurstStats.cs:36-39` | *"At client_send_hz=20 with no hitch this should stay at 1"* — **틀렸다.** 창이 `[now-1.0, now]` 닫힌 구간이라 20 Hz 정상 송신에서 **21**이 나온다(§1.2). 이 주석이 이미 한 명(리더)을 "21이 이상하다"로 오도했다 | `~20-21`이 정상이고 그 이유(양 끝 포함)를 한 줄. **프레임당 값과 1초 창 값의 기대치가 다르다**를 분리해 적는다 |
| **F-10** (이월, 부분) | `Greybox/PeriodicStatusLog.cs` 필드 블록 | 여전히 **(1) 회전 적용 형태**(`v'=q·(v,0)·conj(q)`, Hamilton, `(x,y,z,w)` 순서)와 **(2) `attitude_*`·`position_*`이 월드 프레임임**이 없다 | 두 줄 ＋ §3.4(3)의 방위각 판독 예제. **`atan2(fwd.x, fwd.z)` 증가 = 우선회**는 ADR-0009의 축 정의만 쓰고 손 법칙을 쓰지 않는다 |
| **F-11** (이월, 절반) | `Greybox/PeriodicStatusLog.cs` | `brake`가 **여전히 로그에 없다**(HUD에만). SC-56이 요구하는 "브레이크" 조작을 로그로 확인할 수 없었다 | 필드로 추가. **`flight_assist`와 엇갈리는** 테스트 조합을 함께(F-15 규율) |
| **F-14** (이월) | `Greybox/GreyboxSession.cs:653` | 주기가 여전히 `5.0`. R16 누적 필드가 일부를 구제했으나 **자세 변화 기반 판독(§3.4(2))은 5초 창에서 신뢰할 수 없다** — 리더의 8/0이 재현되지 않은 직접 원인 | `1.0`으로. 60초 세션에 60줄 |
| **F-2′** (이월) | `Greybox/ObserverSession.cs:270`, `:124` | 배선 테스트 0건 그대로 | r7의 요구 그대로 |
| **L-8** (이월) | `Greybox/HudTextWrap.cs:12` | 미대응 | 문장을 *"모든 필드가 공백으로 구분되는 한"*으로 좁힌다 |

### 8.3 리더 (문서·절차)

| # | 내용 |
|---|---|
| **L-15** | `yaw_deg / mouse_dx_total = 0.12`을 기준 2의 근거에서 **삭제**한다(§3.4(1)) — 대수적 항등식이다. 대체 근거는 §3.4(3)의 방위각 대조 |
| **L-16** | "뱃머리가 직전 우현으로 — 일치 8 / 불일치 0"이 **재현되지 않는다**(내 계산: 12구간 11/1). 필터 기준을 문서에 적거나 §3.4(3)으로 교체한다 |
| **L-17** | "Editor는 닫혀 있다"가 사실이 아니었다(PID 20260 가동 중). **다음 라운드 지시 전에 `Get-Process Unity`로 확인**한다. 촬영 세션의 Editor를 닫는 것은 사람이 한다 |
| **L-18** | 오토레벨 5-b 증거의 귀속을 **R6 → R5**로 정정한다. R6에는 `roll≠0` 줄이 없다 |
| **L-19** | 다음 촬영에 **우선회 누적 90° 이상**을 한 구간 넣는다. 현재 우선회 최대 표본이 `+5.88°`다(§3.4(3)) |
| **L-20** | **SC-89 (f) 발동을 보고에서 누락했다**(§1.4). 계약이 architect 통지를 명시한 조항이다 |
| **L-13** (이월) | R16 "남은 것"에 **미대응 항목이 이름으로 나열되지 않았다.** 두 라운드 연속. 현재 미대응 7건(§4) |
| **L-10** (이월) | `sc59-v2/13-correction-leader-2.md` **여전히 없고**, R13 "남은 것"(`:1560-1568`)에 5-b 문장도 없다 |
| **L-11** (이월) | `03_client_impl.md:1485` "1건 실패" → 2건 |
| **L-7** (이월) | R13 md5 표의 유효 시점 주석. **R16 표에도 같은 단서가 빠졌다** |
| **L-14** (이월, 부분 대응) | 증거 폴더 복사는 **실제로 이루어졌다**(R5·R6 둘 다) — 이번 라운드에 내가 `unity test`로 `Editor.log`를 덮어쓰지 못한 것도 그 덕은 아니지만, 사본이 있어 판독이 가능했다. 체크리스트 문서에 단계로 들어갔는지는 확인 대상 |

---

## 9. 남은 게이트와 그것을 닫으려면 무엇이 필요한가

### 9.1 사람 손이 필요한 것

| 게이트 | 필요한 것 | 비고 |
|---|---|---|
| **SC-46 · §0.12 주입 시험 · 1002 (ii)** | **Unity Editor(PID 20260)를 닫는 것.** 그것 하나면 qa가 CLI로 전부 실행한다 | **에이전트는 이 프로세스를 종료하지 않는다.** 1분 |
| **SC-59 (b2) 마커 가시성** | 촬영 구간 전체에 마커 최소 1개가 화면에. **영상으로만 닫힌다**(계약이 그렇게 정했다) | 촬영 |
| **SC-59 (a) 나머지 — "로컬 +Z가 화면의 뱃머리인가"** | 로그가 원리적으로 닫지 못한다(§3.5). **사람 눈** 또는 **메시 전방축을 단언하는 EditMode 씬 테스트** | 사람 판단 → 후자면 client 작업 |
| **SC-59 (c) 경계 미끄러짐의 화면 표현** | 영상 | 촬영 |
| **SC-59 기준 2 우선회** | 다음 촬영에 **우선회 누적 90° 이상** 한 구간(L-19). 로그만으로 닫힌다 | 촬영 절차 한 줄 |
| **1002 (iii)** | **프로토콜 위반을 내는 실클라이언트로 실서버 세션.** R5·R6은 `protocol_violations_total = 0`이라 이 경로를 밟지 않았다 | 사람 또는 봇 세션 |
| **SC-56 범위 분리 검증** | **재접속이 있는 세션.** 현재 두 세션 모두 `sessions_opened_total = 1`이라 세션값과 run값이 같다(§2.4) | 촬영 중 한 번 끊었다 잇기 |

### 9.2 사람 손이 필요 없는 것

| 게이트 | 필요한 것 | 담당 |
|---|---|---|
| **SC-56 FAIL 해소** | **F-18** — 1 tick 정렬 어긋남 수정. **재현 데이터가 이미 있다**(R6 row3~6). 그 뒤 세션 재촬영 필요 | client → 재촬영만 사람 |
| **SC-89 (f) 처리** | A-1 통지에 대한 판단(ADR-0012 §6.3의 미룬 결정을 지금 내릴지) | architect |
| **F-16 · F-17 · F-10 · F-11 · F-14** | 주석·필드 수정. 전부 코드 편집 | client |
| **F-2′ · L-8** | 이월 | client |
| **문서 정정 L-7·L-10·L-11·L-13·L-15·L-16·L-18·L-20** | 이월 ＋ 신규 | 리더 |

### 9.3 한 줄 요약

**사람이 지금 해야 하는 것은 하나다: Unity Editor를 닫는 것.** 그것이 SC-46과 §0.12 주입 시험, 1002 (ii)를 한 번에 연다. **그 다음 병목은 F-18이다** — **SC-56은 촬영을 더 한다고 닫히지 않고, 코드를 고쳐야 닫힌다.** 촬영으로만 닫히는 것은 SC-59의 영상 절(b2·c)과 기준 2 우선회, 1002 (iii), SC-56 재측정과 범위 분리뿐이다.

---

## 10. §0.12 준수 · 금지 사항

- **결함 주입 0회** — Unity CLI를 쓸 수 없어 검출력 시험을 하지 못했다. 표식 `QA-R8 TEMP DEFECT` 잔존 **0건**(`grep -rn` 실행, 빈 출력).
- `git status --porcelain` **빈 출력** — 이 리포트 외에 파일을 하나도 건드리지 않았다.
- **`docker compose down -v` 미사용**(어떤 형태로도). **서버 하드 킬 없음**(서버는 이미 종료돼 있었고 R5 stdout이 `stdin 'shutdown' 수신 — graceful shutdown` 정상 종료를 보인다). golden 재생 파일 미변경. 커밋 없음. `_workspace/` 삭제 없음.
- **증거 로그 미변경** — `unity test`가 실행되지 않았으므로 `client/Logs/Editor.log`도 덮어쓰이지 않았다. `evidence/R5-realserver/`·`R6-signs/`는 읽기만 했다.

---

## 11. 실행 증거 색인

| 산출물 | 경로 / 명령 | 요약 |
|---|---|---|
| R5 실서버 세션 | `evidence/R5-realserver/Editor-session.log` (periodic 54줄) ＋ `server-stats-after.json` ＋ `server-stdout.log` | SC-89 전 절, SC-56 (b)(c)(d), SC-59 (d) |
| R6 부호 세션 | `evidence/R6-signs/Editor-session.log` (periodic 16줄) ＋ `server-stats-after.json` | SC-59 기준 1·2·3, SC-56 FAIL의 깨끗한 반례 |
| 부호 재계산 (기준 1·3) | §3.2 인라인 — 로그 직접 파싱 ＋ Hamilton 회전 재구현 | `d·fwd=+1.0000` ×2, `d·up=+0.9991 / -0.9999` |
| 체인 대조 (기준 2) | §3.4(2) 인라인 | **일치 11 / 불일치 1** — 리더의 8/0 미재현 |
| 방위각 대조 (기준 2) | §3.4(3) 인라인 | `azim(aim) == yaw_deg` 15줄 전부 일치, `azim(attitude)` 추종 |
| 오토레벨 `sinErr` 재계산 | §3.6 인라인 — `ShipIntegrator.cs:128-139` 식 이식 | 복귀 2회 ＋ 억제 6줄, **§7b(1) 짝 성립** |
| `truncated`의 발동 조건 유도 | §1.1 — `TickCatchUp.cs:137-146` 직독 | `unscaledDeltaTime > 0.95 s`. `carry_fwd = 10 × truncated` 교차 확인 |
| SC-89 (c) grep | `grep -n 'while (_tickAccumulator' GreyboxSession.cs` | **exit 1** |
| SC-89 (e) diff | `git diff --stat HEAD -- contracts/ data/movement/sync-tuning.json`, `git status --porcelain` | 둘 다 빈 출력 |
| md5 7건 | `md5sum` 실행 (§6.2) | **7/7 일치** |
| Unity 실행 차단 | §6.1 — `unity test` 출력 ＋ `Get-Process -Id 20260` | Editor PID 20260 가동 중 (기동 10:59:27) |
| 역방향 게이트 selftest | `python tests/e2e/check_contract_items.py --selftest` | **PASS 케이스=5**, 양성 대조가 실제로 빨간불 |

---

## 12. 라운드 8 결론

- **PASS 1건**(SC-89 — (a)(b) 실서버 증거로 자명 통과 해소, (c)(e) 닫힘 유지, **(f)는 architect 통지 발동**)
- **FAIL 1건**(SC-56 — `reconcile_hard_snap_total` 18 / 39 ≠ 0). **네 라운드 만에 처음으로 이 항목을 잴 수 있게 됐고, 재자마자 어긋났다.**
- **통과 1건**(SC-59 — 제품 책임자 판정, qa 판정 아님). 로그 재현 결과 기준 1·3·5-b는 닫히고, **기준 2는 "전역 반전 없음"까지만** 닫힌다.
- **미검증(환경) 1건**(SC-46 — Editor 점유).
- **신규 결함 3건**(F-18·F-17·F-16, 전부 client) ＋ **신규 리더 항목 6건**(L-15~L-20) ＋ **이월 미대응 7건**.

**§0.10을 다시 적는다(M-14):** 손 방향 부호가 통째로 뒤집혀도 자동 검증은 전부 통과한다. 이번 라운드에 **R6·R5 로그가 그 검출기의 일부를 사람 눈 없이 대체했다** — 그러나 **"로컬 +Z가 화면의 뱃머리인가"는 로그가 원리적으로 닫지 못한다. 사람이 볼 때까지 이 슬라이스는 부호에 대해 전부를 증명하지 못한다.**

**리더가 옳았던 가장 중요한 것:** `truncated`가 히치의 증거라는 주장. 의심하라는 지시대로 `TickCatchUp.Compute`를 유도했고 **0.95초 문턱은 일반 프레임 변동이 절대 닿지 못하는 값**이다. 네 라운드 동안 자명 통과였던 SC-89 (b)가 **이 한 수로 닫혔다.** 그리고 오토레벨 5-b는 **이 슬라이스에서 §7b(1) 짝이 자명 통과 없이 성립한 첫 사례**다.

**리더가 틀렸던 가장 중요한 것:** 기준 2의 근거 둘 중 하나가 **대수적 항등식**이었다 — `yaw_deg`와 `mouse_dx_total`이 코드에서 **같은 `delta.x`를 연속 두 줄에서** 더한다. **§7a를 슬라이스의 규율로 세운 그 자리에서, 판정자가 자기 판정 기준에 §7b(1)을 돌리지 않았다.** 계약 §7b 규칙 3번이 정확히 이것을 막으려던 것이다: *"판정 기준을 만드는 사람도 자기 기준에 1번을 돌린다."*

**하나만 고른다면 F-18이다.** SC-56은 이 슬라이스의 유일한 FAIL이고 **촬영을 몇 번 더 해도 닫히지 않는다.** 그리고 그 원인으로 보이는 1 tick 정렬 어긋남은 SC-89 (f)가 가리키는 **ADR-0012 §6.3의 미룬 결정**과 같은 자리다 — **미룬 결정의 발동 조건이 실제로 충족됐고, 그것이 플레이어에게는 초당 세 번의 텔레포트로 보인다.**

---

# 부록 — 라운드 8 갱신 절 (Editor 해제 후 재판정)

> **절대 원칙 5: 위 §0~§12는 그대로 둔다.** 이 절은 정정·추가 레코드다.
> 배경: 리더가 Unity Editor(PID 20260)를 닫았고, §6.1에서 막혔던 것이 풀렸다.
> 리더는 자기 지시문의 "Editor는 닫혀 있다"가 확인 없이 쓴 문장이었음을 **스스로 정정했다.**
> 리더가 먼저 실행한 `246/244/0/2`는 **판정 근거로 쓰지 않았다** — 전부 직접 실행했다.

## A1. SC-46 재판정 — **미검증(환경) 해제 → PASS**

```
$ unity test client --mode EditMode --report-format nunit,junit \
    --output unity-tests-qa-r8/EditMode.nunit.xml --junit-output unity-tests-qa-r8/EditMode.xml
exit 0
<test-run testcasecount="246" total="246" passed="244" failed="0" inconclusive="0" skipped="2"
          start-time="2026-09-24 09:44:06Z" duration="1.0836252">
```

산출물 2개 생성 확인: `EditMode.nunit.xml`(235,002 B) · `EditMode.xml`(82,440 B).
**리더 보고와 일치한다.**

**§7a 단언 — 이 초록이 무엇을 보고 켜졌는가:**

- **243 → 246의 +3이 전부 설명된다.** 픽스처별 케이스 수를 nunit에서 직접 셌다:
  `PeriodicStatusLogTests` **9 → 12**(+3 = R16 신규 3건), 나머지 전부 불변
  (`PendingRebaseSlot` 5, `HudTextWrap` 6, `MarkerHudLine` 5, `SnapshotRebaseBatch` 6,
  `GreyboxSendBurst` 2 — qa r7 수치 그대로). **미설명 증감 0건.**
- **건너뛴 2건은 숨은 실패가 아니다**(nunit에서 이름 확인):
  `LiveServerTests.Live_ThreePings_RoundTripInOrder`,
  `LiveServerTests.Live_ServerInitiatedClose_ReconnectsAsANewSession` — 실서버 게이트.
- **그리고 이 246은 아래 §A2의 두 결함에 대해 여전히 0건짜리 검출기다.**
  건수는 경로가 실행됐다는 증거가 아니다.

**판정: SC-46 PASS.** §0 표의 `미검증(환경)`을 **해제한다.**

## A2. §0.12 결함 주입 시험 — **7회 실행. RED 5건, 영검출 2건**

전부 스크래치패드 백업 → 주입 → 전체 스위트 → **백업에서 복원**(갱신 경로 미사용) →
md5 재확인. 표식 `QA-R8 TEMP DEFECT`.

| # | 주입 | 결과 | 잡은 테스트 | 산출물 |
|---|---|---|---|---|
| **1** | `Format()`의 `mouse_dx_total`·`position_z`를 **절댓값**으로 (리더 지정 = 부호를 잃은 빌드) | **RED — failed=1** | `Format_NegatedMouseDelta_IsVisibleInTheLog` | `EditMode.RED-SignLost.nunit.xml` |
| **1b** | 같은 것에서 **`position_z`만** 절댓값 (둘 중 어느 쪽이 잡히는지 가른다) | **RED — failed=1** | 같은 테스트 | `EditMode.PROBE-PositionZAbs.nunit.xml` |
| **2** | `BuildHudRows`의 `HudTextWrap.Wrap` 제거 (리더 지정 = F-1 재발) | **RED — failed=2** | `BuildHudRows_Sc56AndSc89Lines_AllFieldsAppearSomewhereInOutput`<br>`Format_FourMarkerLine_SurvivesTheHudWrapWithEveryFieldIntact` | `EditMode.RED-NoWrap.nunit.xml` |
| **3** | `flight_assist=`를 `BoundarySoftCrossed`에서 뽑기 (**qa 추가 — r7 F-15 회귀 확인**) | **RED — failed=1** | `Format_FlightAssist_PrintsBothStates` | `EditMode.RED-AssistFromBoundary.nunit.xml` |
| **4** | `position_x` ↔ `position_y` 교환 (**qa 추가 — 축 교환**) | **RED — failed=2** | `Format_PositionIsAVector_NotJustTheMagnitudeItAlreadyHad`<br>`Format_NegatedMouseDelta_IsVisibleInTheLog` | `EditMode.RED-PositionXYSwap.nunit.xml` |
| **5** | `YawPitchToQuaternion`의 `halfYaw` **부호 반전** (**qa 추가 — SC-59가 잡으려는 결함의 실제 형태**) | **⚠ 영검출 — failed=0, 246/244/0/2** | 없음 | `EditMode.PROBE-YawQuatSignFlip.nunit.xml` |
| **6** | `Sample()`에서 `delta.x = -delta.x` (**qa 추가 — R16이 스스로 동기로 든 결함**) | **⚠ 영검출 — failed=0, 246/244/0/2** | 없음 | `EditMode.PROBE-DeltaXNegatedAtRead.nunit.xml` |

### A2.1 리더 지정 두 건 — 둘 다 RED. **그리고 L-11이 실행으로 확정됐다**

주입 2는 **failed=2**다. `03_client_impl.md:1485`의 R13 F-1 기록은 *"1건 실패"*라고 적혀 있다.
**qa r6가 L-11로 올리고 r7이 미대응으로 이월한 그 수치가 이번에 실행으로 확정됐다 — 2건이다.**
L-11은 이제 추정이 아니라 측정이다.

주입 1b는 **`position_z` 단독으로도 잡힌다**를 보인다. 주입 1의 RED 1건이 `mouse_dx_total`
하나로 켜진 것이 아니라 **두 필드가 각각 독립적으로 잡힌다.**

### A2.2 F-15는 **진짜로 닫혔다** (qa r7의 자명 통과 지적 해소)

qa r7 §2.4는 **정확히 같은 주입**(`flight_assist=` ← `BoundarySoftCrossed`)에서
`243/241/failed=0` — **영검출**을 실측했다. 같은 주입이 이번엔 **failed=1**이고,
잡는 테스트 이름이 `Format_FlightAssist_PrintsBothStates`로 **R14가 그 목적으로 만든 바로 그 테스트**다.

**§7a 단언:** 이 RED는 "테스트가 추가됐다"가 아니라 **"그 테스트가 겨냥한 결함을 실제로 켠다"**를
보인다. r7의 영검출과 r8의 RED가 **같은 주입·같은 명령·다른 결과**이므로 대조가 성립한다.
**이 슬라이스에서 §7b(1) 위반이 수정되고 그 수정이 독립 검증된 첫 사례다.**

### A2.3 **영검출 2건 — 이것이 이 부록의 가장 중요한 결과다**

**주입 5(`halfYaw` 부호 반전)는 SC-59 기준 2가 잡으려는 결함의 정확한 형태다** —
마우스를 오른쪽으로 밀면 함선이 왼쪽으로 돈다. **246건 전원 초록이다.**

**주입 6(`delta.x`를 읽는 자리에서 반전)도 246건 전원 초록이다.** 그리고 이쪽은 더 나쁘다:

> **R16이 `mouse_dx_total`을 추가한 동기로 스스로 든 결함이 바로 이것이다.**
> `03_client_impl.md` R16 §2: *"`ShipInputSampler.cs`의 `delta.x`를 음수로 뒤집은 빌드는
> R16 이전 로그와 완전히 동일한 줄을 낸다."*
> **R16 이후 로그도 완전히 동일한 줄을 낸다.** 반전이 **두 누적기보다 위쪽**에서 일어나므로
> `MouseDeltaXTotal`과 `_yawDeg`가 **둘 다 같이 뒤집히고**, 비율은 그대로 `+0.12`이며,
> `attitude_*`와의 방위각 대조(§3.4(3))도 **일관되게 뒤집힌 채** 맞아떨어진다.

**§3.4(1)의 L-15 판정을 이 실측으로 정밀화한다.** 마우스 → 선회 사슬에 부호 반전이 살 수 있는
자리는 넷이고, R16이 추가한 쌍이 덮는 것은 그중 하나뿐이다:

| 부호 반전이 사는 자리 | `yaw/dx` 비율 쌍이 잡는가 | §3.4(3) 방위각 대조가 잡는가 | 246건 스위트가 잡는가 |
|---|---|---|---|
| ① `_yawDeg += delta.x * sens`의 부호 | **잡는다**(비율이 `-0.12`가 된다) | 잡는다 | 미측정 |
| ② `YawPitchToQuaternion`의 `halfYaw` 부호 | **못 잡는다**(`yaw_deg` 불변) | **잡는다**(`azim(aim) == -yaw_deg`가 된다) | **못 잡는다 — 주입 5 실측** |
| ③ 서버의 조준 → 자세 매핑 | 못 잡는다 | **잡는다**(`azim(attitude)`만 어긋난다) | 못 잡는다(EditMode에 서버 없음) |
| ④ `delta`를 **읽는 자리** 또는 그 위(디바이스·Input System) | **못 잡는다** | **못 잡는다** | **못 잡는다 — 주입 6 실측** |

**그러므로 §0.10을 이 부록에서 다시, 더 강하게 적는다.**
계약 §0.10은 *"손 방향 부호가 통째로 뒤집혀도 자동 검증은 전부 통과한다"*고 적었다.
**이번 라운드에 그것을 추정이 아니라 실행으로 보였다 — 두 가지 반전 각각에 대해 246건 전원 초록으로.**
그리고 **④는 로그로도 닫히지 않는다.** `mouse_dx_total`은 *코드가 본* 델타이지 *손이 움직인* 방향이 아니다.
**SC-59 기준 2의 첫 고리(사람 손 → `delta.x`)를 닫는 것은 사람 눈뿐이고, 그것이 §0.10이 말한
"사람이 볼 때까지 증명되지 않는다"의 가장 좁고 가장 단단한 잔여다.**

§3.5 표의 "(a) 마우스 우선회" 행을 다음으로 **대체한다**:

| 기준 | 로그로 닫히는 것 | 로그가 닫지 못하는 것 |
|---|---|---|
| (a) 마우스 우선회 | `mouse_dx_total` **이후**의 전 구간(②③ 포함) — 전역 반전 배제. 단 좌선회만 크게 표본됨 | **좌우 비대칭** ＋ **`delta`를 읽는 자리와 그 위의 반전(주입 6)** — **사람 눈이 아니면 어떤 로그·테스트로도 안 닫힌다** |

**수정 요청 (F-19, client — 신규):** 주입 5가 246건을 통과하는 이유는 **`YawPitchToQuaternion`이
순수 함수인데 단위 테스트가 하나도 없기 때문**이다. `ShipInputSamplerTests` 픽스처가 **존재하지 않는다**
(nunit 전수 확인). 최소 한 건이면 된다: *"yaw = +90°이면 `AimTargetWorld`가 월드 +Z를 월드 +X로 보낸다"*
— `atan2(fwd.x, fwd.z)`로 단언하면 손 법칙을 쓰지 않고 ADR-0009 §1의 축 정의만 쓴다.
**이 한 건이 ②를 영구히 닫는다.** ④는 여전히 사람 몫이다.

### A2.4 원복 확인

```
복원 후 md5:
0731aa15fedeef1c18175d0c8e973927  PeriodicStatusLog.cs     (R16 표와 일치)
2a5c79e03ce1f44632ddf3cd97207948  GreyboxSession.cs        (일치)
ee0256ff89f0c212fe04a95b2f96b8ae  ShipInputSampler.cs      (일치)
21fb3b1fa6d551255a72c01e978610bb  MarkerHudLine.cs         (일치)
adaddf93b2a16354bcee1d14bd7af4db  PendingRebaseSlot.cs     (일치)
47e66ef800479426bcae4de45984305d  SnapshotRebaseBatch.cs   (일치)
d2f54c230665116a0f37e96bfabddca3  ObserverSession.cs       (일치)

$ grep -rn "QA-R8 TEMP DEFECT" client/Assets tools tests | wc -l   -> 0
$ git status --porcelain  (client/ 경로)                            -> 변경 0건
$ unity test client --mode EditMode        (복원 후 재실행)
  exit 0, 246/244/failed=0/skipped=2       -> EditMode.GREEN-after-restore.nunit.xml
```

**복원은 전부 백업 파일 복사다 — 어떤 갱신·bless 경로도 쓰지 않았다**(§0.12).
`git status`에 보이는 `01_architect_decisions.md`·`docs/adr/0012-*.md` 변경은 **architect-r8의 작업이고
내 주입과 무관하다** — 내가 건드린 파일 3개는 전부 md5가 커밋 상태로 돌아왔다.

## A3. 1002 (ii) `ReconnectPolicy` 양성 대조 — **닫힘 (실행 증거 복원)**

`EditMode.nunit.xml`에서 직접 뽑았다 — 전부 `result="Passed"`:

| 테스트 | 케이스 수 | 재는 것 |
|---|---|---|
| `Disconnect_WithProtocolViolationCloseCode_StillReconnects` | 1 | **1002 → 재접속한다** |
| `Disconnect_WithProtocolViolationCloseCode_LogsAGreppableLine` | 1 | **1002 → 전용 로그 줄이 나온다** |
| `Disconnect_WithSomeOtherCloseCode_DoesNotLogTheProtocolViolationLine` | 1 | **1002가 아니면 그 줄이 안 나온다** (음성 대조) |
| `Disconnect_WithSupersededCloseCode_DoesNotReconnect` | 1 | **4001 → 재접속하지 않는다** |
| `ShouldReconnect_SupersededCloseCodeIs4001` | 1 | 4001이라는 수 자체 |
| `ShouldReconnect_IsFalseOnlyForTheSupersededCloseCode` | 1 | **false가 4001 하나뿐** |
| `Disconnect_WithAnyOtherCloseCode_StillReconnects` | **6** | 코드 없음·1000·1001·1006·인접 앱 범위 2종 → 전부 재접속 |

**§7a 단언 — 갈림이 실제로 갈린다.** 1002는 *재접속 ∧ 로그 O*, 4001은 *재접속 X*,
기타 6종은 *재접속 ∧ 로그 X*. **세 갈래가 서로 다른 결과를 내고, 각 갈래에 음성 대조가 짝지어 있다.**
전부 통과시키는 단일 상수(`always reconnect`, `never log`)가 존재하지 않는다.

**판정: (ii) 닫힘.** §1.5 표의 `미검증(환경)`을 **해제한다.**
**(iii)은 그대로 안 닫힘** — 실서버에서 위반을 내는 세션이 여전히 없다(R5·R6 `protocol_violations_total = 0`).

## A4. 갱신된 판정 요약

| SC | §0 (부록 전) | **부록 후 (최종)** |
|---|---|---|
| SC-46 | 미검증(환경) | **PASS** — 246/244/0/2, exit 0, 산출물 2개, +3 전수 설명(§A1) |
| SC-56 | **FAIL** | **FAIL — 유지.** 부록은 이 판정을 건드리지 않는다 |
| SC-59 | 통과(제품 책임자) | **통과(제품 책임자) — 유지.** 다만 §A2.3이 **잔여 미증명 범위를 넓혔다** |
| SC-89 | PASS ＋ (f) 발동 | **PASS ＋ (f) 발동 — 유지.** 1002 (ii) 닫힘 복원, **(iii) 안 닫힘 유지** |

**막고 있는 게이트:** SC-56 FAIL 1건 ＋ SC-59의 영상 절(b2·c) ＋ 1002 (iii) ＋ SC-56 범위 분리.
**미검증(환경)은 0건이 됐다.**

## A5. §9 갱신 — 남은 게이트

§9.1의 첫 줄(**"Unity Editor를 닫는 것"**)은 **해소됐다.** 갱신된 목록:

**사람 손이 필요한 것**

1. **SC-59 (b2)·(c) 영상** — 계약이 영상으로만 닫도록 정했다.
2. **SC-59 기준 2 우선회 누적 90° 이상** (L-19) — 촬영 절차 한 줄.
3. **SC-59 (a)의 "로컬 +Z가 화면의 뱃머리인가"** — 사람 눈 또는 씬 단언.
4. **SC-59 기준 2의 첫 고리(손 → `delta.x`)** — **§A2.3 주입 6으로 확정됐다. 어떤 로그·테스트로도 안 닫힌다.**
5. **1002 (iii)** — 위반을 내는 실클라이언트 세션.
6. **SC-56 범위 분리** — 재접속이 있는 세션.
7. **SC-56 재측정** — F-18 수정 **후**에.

**사람 손이 필요 없는 것**

1. **F-18** (SC-56 FAIL의 원인, 1 tick 정렬) — client. architect-r8에 설계 요청 진행 중.
2. **F-19** (신규) — `YawPitchToQuaternion` 단위 테스트 1건. **주입 5의 영검출을 영구히 닫는다.**
3. F-16 · F-17 · F-10 · F-11 · F-14 · F-2′ · L-8 — client.
4. A-1 (SC-89 (f) 미룬 결정) — architect.
5. 문서 정정 L-7 · L-10 · L-11 · L-13 · L-15~L-20 — 리더. **L-11은 §A2.1에서 실행으로 확정됐다(2건).**

**한 줄:** **사람이 다음에 해야 하는 것은 촬영 하나이고, 그 전에 F-18과 F-19가 코드에서 닫혀야
그 촬영이 헛되지 않는다.** F-18 없이 촬영하면 SC-56은 또 FAIL이고, F-19 없이 촬영하면
기준 2의 ②번 자리가 다음 라운드에도 사람 눈에만 걸려 있다.

## A6. 부록의 증거 색인

| 산출물 | 경로 | 요약 |
|---|---|---|
| GREEN (SC-46 판정용) | `unity-tests-qa-r8/EditMode.nunit.xml`, `EditMode.xml` | 246/244/0/2, exit 0 |
| RED 부호 상실 (리더 지정 1) | `unity-tests-qa-r8/EditMode.RED-SignLost.nunit.xml` | failed=1 |
| PROBE `position_z`만 절댓값 | `unity-tests-qa-r8/EditMode.PROBE-PositionZAbs.nunit.xml` | failed=1 — 단독으로도 잡힌다 |
| RED 랩 제거 (리더 지정 2) | `unity-tests-qa-r8/EditMode.RED-NoWrap.nunit.xml` | **failed=2 — L-11 확정** |
| RED `flight_assist` ← boundary | `unity-tests-qa-r8/EditMode.RED-AssistFromBoundary.nunit.xml` | **failed=1 — r7 영검출과 대조** |
| RED `position_x` ↔ `y` 교환 | `unity-tests-qa-r8/EditMode.RED-PositionXYSwap.nunit.xml` | failed=2 |
| **PROBE yaw 쿼터니언 부호 반전** | `unity-tests-qa-r8/EditMode.PROBE-YawQuatSignFlip.nunit.xml` | **failed=0 — §0.10 실증** |
| **PROBE `delta.x` 읽는 자리 반전** | `unity-tests-qa-r8/EditMode.PROBE-DeltaXNegatedAtRead.nunit.xml` | **failed=0 — R16의 동기가 달성되지 않았다** |
| 복원 후 GREEN | `unity-tests-qa-r8/EditMode.GREEN-after-restore.nunit.xml` | 246/244/0/2, md5 7/7 일치, 표식 0건 |

## A7. 부록 결론

**리더가 지정한 두 주입은 둘 다 RED다.** 그리고 그중 하나가 **L-11을 추정에서 측정으로 바꿨다**(2건).
**F-15는 진짜로 닫혔다** — qa r7이 영검출을 실측한 바로 그 주입이 이번엔 빨간불을 켠다.
이 슬라이스에서 자명 통과가 수정되고 그 수정이 독립 검증된 첫 사례다.

**그러나 이 부록의 결론은 초록이 아니다.**
**SC-59가 잡으려는 결함의 두 가지 실제 형태를 주입했더니 246건이 전원 초록이었다.**
그중 하나는 **R16이 스스로 동기로 든 결함**이고, R16이 추가한 필드는 그것을 덮지 못한다 —
반전이 두 누적기보다 위쪽에서 일어나기 때문이다.

**계약 §0.10은 이제 인용문이 아니라 이 라운드의 실측 결과다.**

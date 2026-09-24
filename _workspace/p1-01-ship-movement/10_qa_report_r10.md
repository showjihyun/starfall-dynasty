# QA 평가 리포트 — p1-01-ship-movement 라운드 10

- 평가자: qa (독립), 2026-09-24
- 대상: R8 히치 주입 세션(`evidence/R8-hitch-drift/`)의 SC-56 재판정, r9 §B.5 모델과 관측의 충돌 해소, F-27·F-28
- 입력: `09_qa_report_r9.md`(§A.2 §B.4 §B.5 §H.2 §H.2a §G), `evidence/R8-hitch-drift/{00-session.md,Editor-session.log,server-stats-after.json}`, `02_sprint_contract.md`(2026-09-24 10차 개정), 작업 트리 코드, `git show 8b0a78d`
- **이 라운드에서 레포의 제품 코드를 수정하지 않았다. 커밋하지 않았다. 골든 파일을 건드리지 않았다. 서버를 띄우거나 끄지 않았다. `docker compose`를 어떤 형태로도 실행하지 않았다. 증거 로그를 덮어쓰지 않았다.**
- **r9 리포트는 그대로 둔다(절대 원칙 5).** 아래의 반박·갱신은 전부 새 레코드다.
- 주입 표식 `QA-R10 TEMP DEFECT`는 **사용하지 않았다** — 이번 라운드의 RED 대조는 레포 파일을 건드리지 않고 `git show 8b0a78d`의 사본을 별도 하네스에서 돌리는 방식으로 했다(§A.3). 레포 잔존 0건은 `git status`로 확인했고, 제품 코드 파일의 수정은 한 건도 없다.

---

## 0. 판정 요약

| 항목 | 판정 | 한 줄 |
|---|---|---|
| **A.1 리더 판독 재현** | **부분 정정** | 수치는 재현되나 **집계 모집단이 틀렸다** — "99건 중 n/a 5건"은 **두 세션을 합친 값**이고, 주입은 세션 2에서만 일어났다. 세션 2만 보면 **16건 중 n/a 4건** |
| **A.2 r9 §B.5가 어디서 어긋났나** | **§B.5의 기전은 옳다. 일반화가 틀렸다** | §B.5는 **backlog > M(=20)인 3초 블랙아웃**의 모델이고, 이 세션은 **8 tick(400 ms) < M**이라 truncate를 타지 않는다. **두 문서가 다른 절차를 말하고 있었다.** 틀린 것은 §B.5 (4)의 *"사람이 만들 수 있는 드리프트 경로는 truncate뿐"* 이다 |
| **A.2b `HasError` 4/8 갈림의 규칙** | **로그에서 8/8 확정** | `HasError == (스냅샷 tick ≤ 히치 직전 `CurrentTickIndex`)`. 8회 주입 전부 이 규칙과 일치한다 (§A.2b 표) |
| **A.4 세션 1 — 주입 0건의 자연 드리프트** | **신규 관측: 자연 발생이 흔하다** | 주입이 한 건도 없는 세션 1에서 드리프트 **97건**, 그중 `speed ≥ 100`이 **83건**, 측정 82건(최대 0.0037 m). **수정 전 코드라면 그 83건이 각각 7.0 m → `HardSnap`이다**(실행 확인) |
| **A.4b 세션 1 `drift=-59`의 정체** | **주입 아님. 스크립트 컴파일 스톨** | 직전 줄이 `[ScriptCompilation] Requested script compilation`이고 직후가 bee_backend·도메인 리로드·`EDITOR_RELOAD` 종료다. **Editor 고유 히치이지 플레이어 빌드의 자연 히치가 아니다** |
| **A.3 r9 §H.2 한계 2 vs 리더 산술** | **리더가 맞다. r9가 틀렸다 — 실행으로 확정** | 같은 히치를 수정 전 코드(`8b0a78d`)로 돌리면 `position_error_m = **49.0000**`(= 7×140×0.05)이고 `hard_snap = **2**`다. 수정 후 코드는 `0.0000`, `hard_snap = 0`. **주입 히치는 RED/GREEN을 가른다** |
| **B. SC-56** | **통과** | (a)만 간접. (c1)(c2)(c3) 전부 성립하고, **§7a 단언: 겨냥한 조건이 실제로 발생했다** — 세션 2에서 `speed ≥ 100` 드리프트 사건 16건, 그중 12건이 오차를 측정했다. **(c1)은 이 세션에서 자명하지 않다**(§A.3의 RED가 `hard_snap = 2`) |
| **B.2 `position_error_m max = 0.3151`** | **FAIL 아님 / 출처 미확인(계측)** | (c3)의 사정권 **밖**이다 — (c3)이 보는 스냅샷(고속 드리프트) 최대는 **0.0183 m**. 0.3151은 `drift == 0` 스냅샷에서 났고 **그 스냅샷에는 로그 줄이 없다.** architect 잔차 상한 0.53 m 이내, 밴드는 `SmoothTracked`(< 5.0) → (c1) 무관. **신규 F-29** |
| **B.3 `orientation_error_deg max = 3.2981`** | **SC-56을 깨지 않음 — r9 §A-3 논거 유효** | 1 tick 최대 회전 **3.75°** 이내이고 자세 하드 스냅 임계는 **15.0°**. **단 p99 = 3.1694로 표본이 여러 개다**(r9 세션은 p99 = 0.0001) — 역시 `drift == 0` 스냅샷이고 **로그 줄이 없다**(F-29 같은 구멍) |
| **C. F-28** | **리더의 원인 규명이 틀렸다. 결함은 실재한다** | 세션 경계가 **아니다** — 문제의 사건은 세션 2 **안**에서 났다(`SESSION_READY` line 2130, `closing` line 2585, 사건 line 2573). `GreyboxSession.cs:295-296`은 **돌 이유가 없었다.** 진짜 원인: **약 350초 클라 정지**. 허수 판정: 18건 중 **0건이 세션 경계 허수**, 2건이 "정지 길이를 잰 값" |
| **C.3 지표 사용 가능성** | **`_total`은 사용 가능 / `_max`는 판정에 쓸 수 없다** | 현재 계약은 `_max`를 판정에 쓰지 않으므로 **계약 차단 아님.** F-28을 client 수정 항목으로 확정 |
| **D. F-27** | **r9의 진단은 옳고, 범위는 r9가 말한 것보다 넓다** | truncate 전용이 아니다 — **truncate가 0인 400 ms 히치에서도** `HasError=false` + `hard_snap=0`인 채 **49.00 m 점프**가 난다(실행 측정). 3초 stall이면 **391.56 m**(r9의 "약 390 m" 확인) |
| **D.1 truncate 2건의 시점** | **미검증(계측)** | 로그에 truncate 사건 줄이 **없다.** 5초 주기 카운터로 두 창까지만 좁혀진다 |
| **E. 전체 스위트** | **PASS — 내가 직접 실행** | `272 / passed 270 / failed 0 / skipped 2` — 기준선과 동일. `DefaultStallMs` 250→400은 **어떤 테스트도 건드리지 않는다**(§E) |
| **F. 역방향 게이트** | **PASS** | selftest 5/5, 이 리포트 판정 행 **2행 파싱**, 위반 0 (§F) |
| **계약 외 ① `DefaultStallMs` 홀짝 법칙** | **내 독립 하네스에서 재현되지 않는다** | r9 §H.2a의 "짝수 10/10 · 홀수 5/10"을 내 스윕은 **재현하지 못했다**(150~500 ms 전부 10/10, 100 ms 5/10, 50 ms 2/10). **그 법칙이 지금 `HitchInjection.cs`에 사실로 적혀 있다** |
| **계약 외 ② 재조정 평활화가 존재하지 않는다** | **신규 발견** | `RenderOffset`은 `HardSnapTotal` 계수에만 쓰이고, `RenderLocalShip()`은 `CurrentState`를 **오프셋 없이 그대로** 그린다 — `Smooth`/`SmoothTracked` 밴드는 **아무것도 평활화하지 않는다** |

### 0.1 계약 항목 판정표 (역방향 게이트가 파싱하는 표)

| 항목 | 판정 | 증거 |
|---|---|---|
| **SC-56** | **통과** | (a) **간접** — 세션 2에 "첫 스냅샷에 자기 함선이 있다"를 직접 적는 줄이 없다. 컨트롤러가 세워졌고 tick이 붙은 것으로 간접 확인(`Editor-session.log:2184` `tick=1050125`). (b) `:2651` `position_error_m(p50=0.0006, p99=0.3006, max=0.3151, n=307)`. **(c1)** `reconcile_hard_snap_total = 0`(`:2651`) — **자명하지 않다**: 같은 주입을 `8b0a78d` 코드로 돌리면 `hard_snap = 2`(§A.3 실행). **(c2)** 세션 2 드리프트 사건 18건 중 **`speed ≥ 100 m/s`가 16건**, 주입 8회 전부 140.0 m/s 부근(§A.1 표) — **조건이 실제로 발생했다**. **(c3)** 그 16건 중 오차가 측정된 12건의 **최대 0.0183 m ≤ 0.25 m**(`:2437`). **⚠ 나머지 4건은 `n/a`라 (c3)이 정의되지 않는다** — 그 4건이 F-27의 사각이고 계산상 49 m 점프다(§D). (d) CL-2 `:2635` 307 / 209 / 69 — 전부 비-0 |
| **SC-89** | **(a)(b)(c)(e) 통과 / (f) 기록** | (a) `max_sends_per_frame = 1`(`:2651`), 서버 `protocol_violations_total = 0`·`commands_dropped_over_tick_cap_total = 0`·`RATE_LIMITED = 0`(`server-stats-after.json`). (b) `catchup_carry_forward_ticks_total = 77 > 0` ∧ `reconcile_hard_snap_total = 0` ∧ **`max_ticks_drained = 20 > 1`** — **히치가 실제로 있었다는 단언이 이번엔 8회 의도 주입 + 350초 정지로 확실히 충족된다**(r9가 닫지 못한 자리다). (c) `grep 'while (_tickAccumulator' GreyboxSession.cs` → 0건, 테스트가 `TickCatchUp.Plan`을 직접 호출(`TickCatchUpTests`, 내 실행에서 Passed). (e) `git diff HEAD -- data/movement/sync-tuning.json server/crates/` **빈 출력**. **(f) `catchup_truncated_total = 2 ≠ 0` → architect 통지 대상**(미룬 결정 ADR-0012 §6.3의 발동 조건). `reconcile_forced_after_hitch_total = 0` |

**한 줄 결론:** **SC-56은 이 세션으로 닫힌다.** r9가 "실서버 세션으로는 닫을 수 없다"고 판정한 근거(§B.5 (4))가 **틀렸고**, r9 자신의 §H.2a가 이미 그 반증을 들고 있었다. **다만 닫히는 것과 동시에, 계약이 보지 않는 자리에서 49 m 순간이동이 8번 일어났다**(F-27) — 그것은 SC-56의 FAIL이 아니라 **계약이 아직 묻지 않은 성질**이다.

---

## A. 모델과 관측이 갈린 이유

### A.1 리더 판독 재현 — 수치는 맞고, **모집단이 틀렸다**

`Editor-session.log`(사본, 2,728줄)를 직접 파싱했다. 리더의 수치는 재현된다. **그러나 이 로그에는 `SESSION_READY`가 두 번 있다**(`:580` EDITOR_RELOAD로 끝남 `:1902`, `:2130` PLAYMODE_EXIT로 끝남 `:2585`), 그리고 **히치 주입은 두 번째 세션에서만 일어났다**(첫 `hitch_injected`가 `:2007`, 세션 2 진입 직후).

| | 전체 로그 | 세션 1 | **세션 2 (주입 세션)** |
|---|---|---|---|
| `reconcile_tick_drift_event` | 115 | 97 | **18** |
| 그중 `speed ≥ 100` | 99 | 83 | **16** |
| 그중 `position_error_m = n/a` | 5 | 1 | **4** |
| 드리프트 사건 오차 최대 | 0.0267 | 0.0037(고속) | **0.0183** |
| drift 값 분포 | — | ±1 × 96, −59 × 1 | **±7 × 12, ±8 × 4, −76, −6983** |

**세션 1은 주입이 없고**(`hitch_injected` 1줄이 있으나 `tick=-1`, 즉 접속 전이라 아무 효과가 없다 — `:2008`) **드리프트 96건이 전부 `|drift| = 1`, `delta_tick = 2`다** — 서버 이월/덮어쓰기가 만드는 평시 드리프트다(서버 `input_carried_forward_total = 227`, `input_superseded_total = 110`). 리더가 인용한 "99건 중 5건"은 이 잡음 83건을 주입 결과와 같은 통에 넣은 값이고, **세션 종료 덤프(`n = 307`, `hard_snap = 0`)는 세션 2만의 값**이므로 같은 모집단이 아니다. 세션 범위로 판정하라는 계약 SC-56 §범위 규정(9차 architect Q-1)을 적용하면 **세션 2만 본다**.

**리더는 이 오류를 스스로 정정했고, 판정문에 적으라고 요청했다. 그대로 적는다.**

> 리더가 지적한 자기 오류의 형태: **R7 세션에서는 누계 카운터를 구간 사건처럼 읽었고(시점을 안 봄), 이번에는 세션 경계를 안 봤다.** 더 나쁜 것은 **F-28(세션 경계에서 드리프트가 허수로 잡힌다)을 같은 문서 §3에 직접 써 놓고도 자기 집계에 그 경계를 적용하지 않았다**는 점이다.
> **덧붙일 것이 하나 있다: F-28은 세션 경계 문제가 아니었다**(§C.1). 즉 리더는 **잘못 진단한 경계를 적용하지 않은 것**이고, 만약 적용했다면 이번에는 다른 방향으로 틀렸을 것이다. 두 오류가 서로를 가렸다.
> `00-session.md` §1 표는 지우지 않는다(절대 원칙 5). 리더가 새 레코드로 정정하겠다고 했다.

**이 정정은 판정을 바꾸지 않는다** — 세션 2만 봐도 `speed ≥ 100` 드리프트 16건, 측정 12건이므로 (c2)(c3)은 그대로 성립한다. 그러나 "99건 중 5건뿐"이라는 인상(=대부분 측정됐다)은 **세션 2에서는 16건 중 4건, 25%가 측정되지 않았다**로 바뀐다.

### A.2 r9 §B.5의 추론이 어긋난 자리 — **코드로 특정한다**

**r9 §B.5는 틀리지 않았다. 다른 절차를 말하고 있었다.**

§B.5가 분석한 것은 리더가 r9에게 물었던 *"Game View 포커스를 3~5초 뺐다가 복귀"* 다. 그 절차는 `Time.unscaledDeltaTime ≈ 3 s` → `backlogTicks = 60`이고, `TickCatchUp.Compute`(`TickCatchUp.cs:137-144`)가

```
if (backlogTicks > maxPredictedTicksPerFrame)   // 60 > 20
    return new Plan(20, 1, 0.0, truncated: true);
```

로 **40 tick을 버린다.** 버린 만큼 클라 히스토리 최상단이 스냅샷 tick보다 뒤처지고, `Reconciliation.Reconcile` 1단계(`Reconciliation.cs:96-103`)가 `history[i].ServerTick != snapshotTick`을 전부 건너뛰어 `HasError = false`가 된다. **여기까지 §B.5는 정확하다.**

**R8 세션은 그 절차를 쓰지 않았다.** `HitchInjection.DefaultStallMs = 400`이고 400 ms = **8 tick**이다. `8 > 20`이 아니므로 `Truncated = false`, `TicksToPredict = 8` — **클라이언트가 밀린 8 tick을 전부 예측한다.** 히스토리는 `ServerTick = T+1 … T+8`을 갖게 되고, 스냅샷 tick이 그 범위 안이면 1단계가 **맞는다.**

> **어긋난 것은 §B.5 (4)의 일반화다:** *"사람이 만들 수 있는 드리프트 경로는 truncate뿐이고, truncate는 구조적으로 `HasError`를 false로 만든다."*
> **앞 절은 거짓이다.** sub-M 히치는 truncate 없이 드리프트를 만든다. `K = ProductionMaxSendsPerFrame = 1`이므로 8 tick을 예측해도 **송신은 1건뿐**이고, 서버 `ack`은 그 1건만 반영한다 → `Δack = 1`, `Δtick = 8` → **`drift = −7`**. 로그의 `drift=-7 delta_tick=8`이 정확히 이 수다.
> **그리고 §B.5 (4)가 든 근거("§6.2가 이월 tick에도 seq를 발급하므로 `Δack == Δtick`이 유지된다")도 per-snapshot으로는 거짓이다.** `GreyboxSession.cs:858`의 `PredictCarryForwardOrDormantTick()`은 `_localInputSeq++`로 seq를 **태우지만 보내지 않는다.** 서버 `ack`은 *적용된* seq만 따라가므로, 태운 seq는 **다음 송신 명령이 나갈 때 한꺼번에** `ack`에 반영된다. 로그가 이것을 그대로 보여 준다 — `drift = −7` 다음 스냅샷이 언제나 **`drift = +7 delta_tick = 2`** 다. **합은 0이지만 스냅샷 단위로는 0이 아니고, 드리프트는 스냅샷 단위로 잰다.**

**r9는 자기 리포트 안에서 이미 자신을 반증하고 있었다.** §H.2a의 위상 스윕이 400 ms에서 "10/10 적중"을 냈고, 그 적중 정의가 바로 `drift ≠ 0 ∧ HasError ∧ speed ≥ 100`이다. 그럼에도 §B.5 (4)와 §G.1은 *"SC-56은 이제 사람 손을 필요로 하지 않는다"*로 갔다. **두 절이 정면으로 모순이고, 관측은 §H.2a 쪽 손을 들어 줬다.**

### A.2b `HasError`가 4/8로 갈린 규칙 — 로그에서 8/8 확정

`hitch_injected`의 `tick`은 **stall 직전** `_controller.CurrentTickIndex`다(`GreyboxSession.cs:692`, 주석이 그렇게 명시). 이를 직후 드리프트 사건의 스냅샷 tick과 대조했다.

| 주입 tick | 직후 드리프트 사건 tick | Δ | `position_error_m` |
|---|---|---|---|
| 1050125 | 1050132 | **+7** | n/a |
| 1050184 | 1050184 | **0** | 0.0009 |
| 1050244 | 1050244 | **0** | 0.0004 |
| 1050276 | 1050276 | **0** | 0.0004 |
| 1050313 | 1050314 | **+1** | n/a |
| 1050533 | 1050540 | **+7** | n/a |
| 1050637 | 1050638 | **+1** | n/a |
| 1050755 | 1050754 | **−1** | 0.0004 |

> **규칙: `HasError == (스냅샷 tick ≤ 히치 직전 CurrentTickIndex)`. 8/8 일치.**

이것이 `Reconciliation.cs:96-103`의 1단계 규칙 그 자체다 — 클라이언트가 그 tick을 **이미 예측해 뒀으면** 맞고, 스냅샷이 앞서 있으면 못 맞춘다. 히치가 스냅샷 경계에 대해 어디서 시작했는지(`ApplyPendingRebase`가 stall 프레임에 무엇을 들고 있었는지)가 Δ의 부호를 정한다. **`StarfallNetHost.Update()`의 `Pump()`와 `GreyboxSession.Update()`의 실행 순서가 코드에 고정돼 있지 않다**(`GreyboxSession.cs:711` 주석이 "execution order vs. this Update() unconfirmed"라고 자백한다) — 그래서 이 Δ는 프레임 위상에 따라 흔들린다.

**주의 (§7a):** 위 표는 규칙의 **일치**를 보인 것이지, 4건이 `n/a`인 것이 "정상"임을 보인 것이 아니다. `n/a`인 4건은 **각각 49 m 점프였다**(§D).

### A.3 r9 §H.2 한계 2 vs 리더의 49 m — **실행으로 갈랐다. 리더가 맞다**

r9 §H.2 한계 2: *"적중한 스냅샷에서 수정 전 코드도 `errM = 0.0000`을 낸다. 즉 주입 히치는 (c3′)의 조건을 만들지만 RED/GREEN을 가르지는 못한다."*
리더 `00-session.md` §2: *"`drift = -7`은 `7 × 140 × 0.05 = 49 m`다."*

**하네스를 직접 지었다.** `client/Assets/_Project/Scripts/{Sim,Flight}`의 **실제 소스 파일을 그대로** .NET 8 콘솔 프로젝트에 넣고(`Reconciliation.cs`·`TickCatchUp.cs`·`PredictionHistory.cs`·`ShipIntegrator.cs` 등 사본이 아니라 복사본), `git show 8b0a78d:…/Reconciliation.cs`의 ack 키 판본을 `PreFixReconciliation`으로 나란히 두고, **같은 서버·같은 대본 입력·같은 프레임 일정**에서 `Reconcile`만 바꿔 두 번 돌렸다. 클라 루프 순서는 `GreyboxSession.Update()`를 따랐다(`ApplyPendingRebase` → 누산 → `TickCatchUp.Compute` → 예측/송신, `K=1`, `M=20`, `snapshot_interval_ticks=2`, `carry_forward_max_ticks=10`).

**하네스가 실서버 세션을 재현하는지 먼저 확인했다** — 400 ms 주입에서 하네스가 내는 사건 쌍이

```
POST drift=   -7 delta_tick=    8 tick=1000208 speed_mps= 140.0 position_error_m=n/a
POST drift=    7 delta_tick=    2 tick=1000210 speed_mps= 140.0 position_error_m=0.0000
```

로, **실로그의 `drift=-7 delta_tick=8` → `drift=7 delta_tick=2` 쌍과 형태가 같다.** 3,000 ms에서는 `drift=-59 delta_tick=60 speed=122.9`가 나오는데, 이것도 실로그 세션 1의 `drift=-59 delta_tick=80 speed=105.0`과 같은 계열이고 **r9 §B.5 (1)의 산술(3초 블랙아웃 → 122.5 m/s)을 독립적으로 맞춘다.**

**결과 (10개 위상 전부):**

```
stall_ms=400 (8 ticks)
phase 0: POST worstFastDriftErr=0.0000 hard_snap=0 | PRE worstFastDriftErr=49.0000 hard_snap=2
phase 1: POST 0.0000 hard_snap=0 | PRE 49.0000 hard_snap=2
phase 2: POST 0.0000 hard_snap=0 | PRE 42.0000 hard_snap=2
phase 4: POST 0.0000 hard_snap=0 | PRE 56.0000 hard_snap=2
...
worst position_error_m at a fast measured drift: POST 0.0000, PRE 56.0000
```

`PRE`의 값은 **정확히 `|drift| × 140 × 0.05`** 다 (7→49.0000, 6→42.0000, 8→56.0000). **리더의 산술이 소수점까지 맞다.**

> **판정: r9 §H.2 한계 2는 틀렸다.**
> **(1)** 수정 전 코드는 적중 스냅샷에서 `0.0000`이 아니라 **49 m**를 낸다. 이유는 ack 키 판본이 `history[i].InputSeq == ack`로 **히치 직전 송신 항목을 찾아내기 때문**이다 — 그 항목의 예측 상태는 tick `T`의 것이고 확정 상태는 `T+8`의 것이라 8 tick 분량의 차가 그대로 오차가 된다. **tick 키 판본이 `n/a`를 내는 바로 그 스냅샷에서 ack 키 판본은 큰 값을 낸다.**
> **(2)** 따라서 `reconcile_hard_snap_total`은 이 세션에서 **자명한 0이 아니다** — 수정 전 코드라면 주입 1회당 **2씩** 올랐다(위치 밴드 `49 ≥ 5.0` → `HardSnap`).
> **(3)** 그러므로 r9 §B.4 (c4)의 *"귀속을 세션에서 하네스로 옮긴다"* 는 **필요 없다.** 세션이 스스로 RED/GREEN을 가른다.

**정직하게 적는 한계 둘.**
1. **RED은 하네스에서 확인한 것이고, 수정 전 바이너리를 실서버에 붙여 본 것이 아니다.** 하네스는 지터·패킷 재배열·`RebaseHold`·`Pump()` 실행 순서를 모형화하지 않는다. 다만 하네스의 사건 쌍 형태가 실로그와 일치하고, 오차값이 닫힌 산술(`|drift|·v·dt`)과 정확히 같으므로 **기전은 확인됐다고 본다.**
2. 하네스의 `POST`는 모든 위상에서 `0.0000`을 내는데 **실세션은 0.0003~0.0009**였다. 하네스가 양자화 왕복과 네트워크 순서를 안 태우기 때문이다. **결론의 방향(0 vs 49)에는 영향이 없다.**

### A.3b 세션 2의 **실제** `drift`/`delta_tick` 값을 입력으로 넣은 결과 (리더 요청 2)

하네스의 위상 10개가 관측된 조합을 그대로 재현한다. 각 조합에 대해 수정 전 코드가 내는 값:

| 세션 2 관측 조합 | 관측 건수 | 하네스 재현 | **수정 전 코드 출력** | 밴드 |
|---|---|---|---|---|
| `drift=-7 delta_tick=8` | 6 | phase 0·1·5·6·7 | **49.0000 m** | `HardSnap` (≥ 5.0) |
| `drift=+7 delta_tick=2` | 6 | 같은 위상 | **49.0000 m** | `HardSnap` |
| `drift=-8 delta_tick=10` | 1 | phase 4 | **56.0000 m** | `HardSnap` |
| `drift=+8 delta_tick=2` | 2 | phase 4 | **56.0000 m** | `HardSnap` |
| `drift=-8 delta_tick=8` | 1 | 미재현 (Δack=0, 하네스는 항상 1건 송신) | 산술상 56 m | — |
| `drift=±1 delta_tick=2` (세션 1, 96건) | 96 | stall=150 ms phase 0 | **7.0000 m** | `HardSnap` |

**수정 전 코드가 내는 값은 전부 정확히 `|drift| × 140 × 0.05`다.** 그리고 `7.0000`은 `Reconciliation.cs:19`의 헤더가 **직접 예고한 수**다 — *"an error of exactly v*dt (140 m/s -> 7.0 m, well past reconcile_hard_snap_threshold_m=5.0)"*. **코드 주석의 예고를 실행이 소수점까지 확인했다.**

**리더 요청 2에 대한 답: 리더의 산술은 모델이 아니라 실행으로 확인됐다.** 그리고 관측된 5개 조합 중 4개를 하네스가 그대로 만들어 냈으므로, 이 대조는 관측 밖의 이상화가 아니다.

### A.4 세션 1 — 주입 0건의 자연 드리프트 (리더 요청 3)

**세션 1에는 유효한 주입이 한 건도 없다.** `hitch_injected` 줄은 `:2008` 하나뿐이고 `tick=-1 speed_mps=0.0` — 접속 전 키 입력이라 `_controller`가 없어 아무 일도 하지 않았다. 그럼에도:

| | 세션 1 (주입 0건) |
|---|---|
| 드리프트 사건 | **97건** |
| `delta_tick = 2`(정상 스냅샷 간격) | **96건** — 전부 `|drift| = 1` |
| `speed ≥ 100 m/s` | **83건** |
| 그중 오차가 측정된 것 | **82건**, 최대 **0.0037 m** |
| 나머지 1건 | `drift=-59 delta_tick=80 speed=105.0` (§A.4b) |

> **이것이 "실제 운영에서 드리프트가 나는가"에 대한 첫 직접 관측이다. 난다. 흔하다. 최고속에서 난다.**
> 기전은 서버 쪽 분기다 — `input_carried_forward_total = 227`, `input_superseded_total = 110`. 이월이 난 tick은 `Δack < Δtick`, 덮어쓰기가 난 tick은 `Δack > Δtick`이고, 스냅샷 간격이 2 tick이라 그중 상쇄되지 않은 것이 `±1`로 남는다.
> **수정 전 코드였다면 그 83건이 각각 7.0 m를 내고 `reconcile_hard_snap_total`을 83 이상 올렸을 것이다**(실행 확인, §A.3b). **주입 없이도 이 세션은 RED/GREEN을 갈랐다.**

**이것이 r9 §A.2를 한 번 더 정정한다.** r9는 R7 세션을 근거로 *"드리프트가 100 m/s 이상에서 한 번도 일어나지 않았다"*고 판정했는데, **같은 코드·같은 서버에서 세션 1은 83번 일어났다.** R7 세션이 조용했던 것은 구조적 이유가 아니라 **그 세션의 사정**이었다. r9의 *"고속 주행 중에 의도적으로 히치를 넣어야 한다"*는 결론도 필요조건이 아니었다.

### A.4b `drift=-59`는 무엇이었나

주입이 아니고, 게임플레이 자연 히치도 아니다. 로그가 직접 말한다:

```
:1836  [ScriptCompilation] Requested script compilation because: AssetDatabase observed changes...
:1838  periodic_status tick=1049710 ... catchup_truncated_total=1 reconcile_tick_drift_total=96
:1862  reconcile_tick_drift_event drift=-59 delta_tick=80 tick=1049790 speed_mps=105.0 position_error_m=n/a
:1864  Starting: ...bee_backend.exe ... ScriptAssemblies
       ... Csc / ILPostProcess / "Reloading assemblies after finishing script compilation"
:1902  starfall.net: closing session_id=... reason=EDITOR_RELOAD code=1000
```

**스크립트 컴파일 + 도메인 리로드가 4초(80 tick) 동안 메인 스레드를 잡은 것이다.** 이것은 **Editor 고유의 히치**이고 플레이어 빌드에는 없다. `delta_tick = 80 > M = 20`이므로 truncate 경로이고, 그래서 `n/a`다 — **r9 §B.5의 모델이 자연 발생 사례에서도 맞았다.**

> **판정: `-59`는 "실제 운영에서 드리프트가 나는가"의 답이 아니다**(Editor 전용 사건). **그 답은 같은 세션의 96건 `|drift|=1`이고, 그쪽이 훨씬 강한 답이다.**

---

## B. SC-56 판정

계약은 **2026-09-24 10차 개정**으로 SC-56 (c)를 **(c1)(c2)(c3) 세 절**로 확정했다(`02_sprint_contract.md:259`, 변경 이력 `:593`). **r9 §B.4의 (c3′)/(c4) 4절안(F-22)은 채택되지 않았다.** 아래는 **효력 있는 계약 문구**로 판정하고, 리더가 물은 4절안은 §B.4에서 따로 답한다.

### B.1 절별 판정 (세션 2 범위)

| 절 | 요구 | 관측 | 판정 |
|---|---|---|---|
| **(c1)** | `reconcile_hard_snap_total == 0` | `:2651` **0** | **통과 — 자명하지 않다.** §7a 단언 둘: ① 같은 주입에 대해 수정 전 코드는 `49~56 m`·`hard_snap = 2`를 낸다(§A.3b). ② **주입이 0건인 세션 1조차** 자연 드리프트 83건을 냈고 수정 전 코드라면 각각 `7.0 m` → `HardSnap`이다(§A.4). **이 0은 "조용했다"가 아니라 "고쳐졌다"를 뜻한다** |
| **(c2)** | 드리프트가 오르는 순간 `speed ≥ 100 m/s`인 적용 스냅샷 ≥ 1건 | **16건** (18건 중). 주입 8회가 전부 `speed_mps = 140.0` 부근(`:2232` `hitch_injected … speed_mps=140.0` ↔ `:2244` `drift=-7 … speed_mps=140.0`, **같은 tick 1050184**) | **통과 — 조건이 실제로 발생했다.** r9가 "발생하지 않았다"로 막았던 바로 그 자리다 |
| **(c3)** | 그 스냅샷의 `position_error ≤ 0.25 m` | 측정된 12건 최대 **0.0183 m**(`:2437`), 0.25 초과 **0건** | **통과.** ⚠ **나머지 4건은 `n/a`라 (c3)이 정의되지 않는다** — 계약이 이 경우를 말하지 않는다(§B.4) |
| **(a)** | 첫 `WORLD_SNAPSHOT`에 자기 함선 | 직접 단언 줄 **없음**. 컨트롤러 생성은 간접 확인 | **통과(간접)** — 로그에 이 절을 직접 닫는 줄이 없다. 한 줄 추가를 권고(F-30) |
| **(b)** | p50/p99/max를 HUD·로그에 (`n` 동반) | `:2651` `n=307` 동반 | **통과** |
| **(d)** | CL-2 관찰 3건 전부 비-0 | `:2635` **307 / 209 / 69** | **통과** |

> **§7a 단언 — 이번에 무엇을 보고 초록이 켜졌는가.**
> **(c1)의 0은 "아무 일도 없었다"가 아니다.** 같은 세션 안에서 (ⅰ) 의도된 히치가 8회 실제로 일어났고(`hitch_injected` 8줄, 전부 140 m/s), (ⅱ) 그 8회가 전부 드리프트 사건 쌍을 만들었으며(16줄), (ⅲ) 그중 12회는 재조정이 **오차를 실제로 쟀고**(0.0003~0.0183 m), (ⅳ) 동일 조건을 수정 전 코드로 돌리면 **`hard_snap = 2`, `err = 49 m`** 가 나온다. **조건 발생 · 측정 발생 · 대조군 적색 — 셋이 다 있다.** 이 세 가지를 못 대던 것이 R5·R6·R8·R9였다.

### B.2 `position_error_m max = 0.3151` — 출처 특정 시도와 판정

**리더 말이 맞다: 이 값은 드리프트 사건이 아니다.** 내 파싱으로 세션 2 드리프트 사건의 측정 오차 최대는 **0.0183 m**이고, 전체 로그로 넓혀도 **0.0267 m**다. 0.3006(p99)·0.3151(max)은 **`drift == 0`인 재조정에서 났다.**

**특정하려 했고, 특정할 수 없었다. 이유가 계측에 있다.**
`ReconcileTickDriftEvent.TryCreate`는 **`drift != 0`일 때만** 줄을 남긴다(`GreyboxSession.cs:613` 호출부, `driftEvent.HasValue` 가드). `drift == 0` 스냅샷은 `ReconcileErrorStats`의 표본 통에만 들어가고 **tick·속도·추력 어느 것도 기록되지 않는다.** 307개 표본 중 어느 것이 0.3151이었는지 로그가 답하지 않는다.

**대조한 것:**
- **architect 잔차 모델** (계약 `:259`): *"추력 전면 반전이 드리프트 tick과 겹치면 `abs(Δa)·dt²` 누적으로 **0.53 m**까지 난다"*. **0.3151 < 0.53 — 모델 안이다.** 세션에 추력 변화가 실제로 있었다(`:2437` `thrust_z=0`, 직전 `:2425` `thrust_z=1000`; `:2570` periodic은 `thrust_*=0`에 `speed=119.0`으로 감속 중).
- **밴드**: `RenderOffset.ClassifyPosition`(`RenderOffset.cs:41-45`) — `0.005 < 0.3151 < 5.0` → **`SmoothTracked`**. `HardSnap`이 아니므로 **(c1)과 모순 없다.**
- **내 하네스**: 3초 stall(truncate)을 넣으면 `POST`가 측정하는 오차가 **0.2100 m**까지 오른다(위상 1·2·3·7·8·9). 세션의 `truncated = 2`와 자릿수가 맞는다 — **truncate 복구 스냅샷이 유력한 출처**지만, 하네스에서는 그 0.21이 **드리프트 사건에서** 났고 실세션의 0.3151은 **`drift == 0`에서** 났다. **일치하지 않으므로 확정하지 않는다.**

> **판정: (c3)의 사정권 밖이며 FAIL이 아니다. 출처는 `미검증(계측)`.** architect 잔차 상한 안이고 밴드가 `SmoothTracked`이므로 SC-56의 어느 절도 깨지 않는다. **그러나 "왜 0.3 m가 났는지 아무도 모르는 상태"가 계약에 의해 허용되고 있다는 사실 자체가 구멍이다 — F-29.**

### B.3 `orientation_error_deg max = 3.2981`

- `turn_rate_max_deg_s = 75.0`(`data/ships/scout-s01.json`) → **1 tick 최대 3.75°**. `3.2981 < 3.75` — **1 tick 분량 안이다.**
- 자세 하드 스냅 임계 `reconcile_orientation_hard_snap_deg = 15.0`. `ClassifyOrientation`은 `≥ 15.0`에서만 `HardSnap`이므로 3.2981은 **`SmoothTracked`**, 카운터를 올리지 않는다. **`hard_snap = 0`과 모순 없다.**
- **r9 §A-3 논거는 그대로 적용된다.**

**그러나 r9 세션과 달라진 점을 그대로 적는다.** r9 세션은 `p99 = 0.0001, max = 2.1944 (n=346)` — 단일 이상치였다. 이번은 **`p99 = 3.1694, max = 3.2981 (n=307)`** 로, **3° 대 표본이 최소 3~4개**다. 그리고 **세션 2 드리프트 사건 18줄 중 `orientation_error_deg > 0.5`인 것은 0건**이다(직접 파싱). 즉 이 3° 표본들도 **`drift == 0` 스냅샷**이고 **로그 줄이 없다.** §B.2와 같은 구멍이다.

**판정: SC-56을 깨지 않는다. 다만 "1 tick 안이므로 설명된다"는 것은 크기의 상한만 말할 뿐, 왜 r9 세션보다 표본이 늘었는지는 답하지 않는다 — F-29가 함께 닫는다.**

### B.4 리더가 물은 r9 §B.4의 4절안 (c1)/(c2)/(c3′)/(c4)

효력 있는 계약이 아니지만 물었으므로 답한다.

| 절 | 판정 |
|---|---|
| **(c1)** | **유효하고 이 세션에서 비-자명하게 통과.** 4절안과 현 계약이 같은 문구다 |
| **(c2)** (모든 비-0 드리프트 전수 기록) | **유효하고 실행 가능하다.** F-21이 착지해 18건 전수가 로그에 있다. §A.1 표가 그 전수 기록이다. **현 계약 (c2)보다 강하다 — 채택을 권고한다** |
| **(c3′)** (고속·`has_error` 사건에만 0.25 적용, 0건이면 비-적용) | **필요하다. 현 계약 (c3)은 이 구멍을 안 막는다.** 이번 세션이 정확히 그 경우다 — 고속 드리프트 16건 중 **4건이 `has_error == false`**이고, 현 계약 (c3)은 "그 스냅샷"이 무엇인지 말하지 않아 **측정된 12건만 보고 통과**시킬 수 있다. 그 4건이 49 m 점프다 |
| **(c4)** (귀속을 하네스로) | **불필요해졌다. 철회를 권고한다.** 전제였던 §B.5 (4)가 반증됐다(§A.2·§A.3) — 세션이 스스로 RED/GREEN을 가른다 |

> **내가 제안하는 문구(architect 판정 대상):** 현 계약 (c3)에 **한 줄만 더한다.**
> *"(c3) 그 스냅샷의 `position_error ≤ 0.25 m`. **`has_error == false`인 고속 드리프트 사건이 있으면 그 건수와 각각의 `delta_tick`을 리포트에 싣고, `reconcile_rebase_jump_m`(F-27)이 `reconcile_hard_snap_threshold_m`(5.0 m) 이하임을 별도로 보인다.** 그 게이지가 없으면 그 건들은 `미검증(계측)`이다."*
> **자명 통과 시험(§7b(1)):** *"조용한 세션"* → (c2)가 잡는다. *"빠를 때 어긋났는데 전부 `n/a`인 세션"* → **현 문구는 통과시키고, 이 한 줄은 막는다.** *"코드가 고장난 채 빠를 때 어긋난 세션"* → (c1)이 잡는다(§A.3으로 확인됨).

---

## C. F-28 — 드리프트 지표 오염

### C.1 원인 — **리더의 규명이 틀렸다**

리더: *"직전 세션 종료 tick과 새 세션 첫 스냅샷 tick의 차이를 드리프트로 계산했다. `GreyboxSession.cs:295-296`에 리셋이 있는데 이 사건이 그것을 거치지 않았다."*

**로그가 그렇게 말하지 않는다.**

```
:2130  starfall.net: SESSION_READY session_id=01a0d30a-d496-7562-baa6-a808909ede04   ← 세션 2 시작
:2570  starfall.greybox: periodic_status tick=1050790 … reconcile_tick_drift_total=17 reconcile_tick_drift_max=76
:2573  starfall.greybox: reconcile_tick_drift_event drift=-6983 delta_tick=7004 tick=1057794 …   ← 문제의 사건
:2585  starfall.net: closing session_id=01a0d30a-d496-7562-baa6-a808909ede04 reason=PLAYMODE_EXIT
```

**문제의 사건은 세션 2 시작과 종료 사이, 같은 `session_id` 안에서 났다.** `delta_tick = 7004`이고 `tick = 1057794`이므로 `_driftPrevTick = 1050790` — **같은 세션의 직전 적용 스냅샷**이다(`:2570` periodic의 tick과 일치). **`OnSessionReady`는 그 사이에 돌 이유가 없었고, 안 돈 것이 옳다.** `GreyboxSession.cs:295-296`의 리셋에는 결함이 없다.

**진짜 원인은 클라이언트가 약 350초(7004 tick ÷ 20 Hz) 동안 프레임을 돌리지 않은 것이다.** 사건 직전 로그에 `Scanning for USB devices` 3줄과 `Asset Pipeline Refresh`가 있다 — Unity Editor가 **포커스를 되찾을 때** 내는 줄이다. 즉 사용자가 Play 상태로 Editor를 떠나 있었고, `runInBackground`가 꺼진 Editor는 `Update()`를 멈췄으며, 돌아온 프레임에서 `ApplyPendingRebase()`가 7,004 tick 앞선 스냅샷 하나를 적용했다.

> **`ReconcileTickDrift.Compute`는 정의대로 계산했다.** `deltaAck − deltaTick`은 **정의상** 정지 길이를 그대로 삼킨다. 결함은 산술이 아니라 **지표가 두 가지 다른 것을 한 수에 섞는 것**이다 — (ⅰ) 서버 이월/덮어쓰기로 ack과 tick이 벌어진 양(재조정이 신경 쓰는 것)과 (ⅱ) 클라이언트가 몇 초 자고 있었는지(재조정과 무관).

### C.2 18건 중 허수는 몇 건인가

| 기준 | 건수 |
|---|---|
| 세션 경계 때문에 생긴 허수 | **0건** — 그런 사건은 없다 |
| "클라 정지 길이"를 잰 사건(`delta_tick > M = 20` ∧ `has_error == false`) | **2건** — `drift=-76 delta_tick=98`(`:2401`), `drift=-6983 delta_tick=7004`(`:2573`) |
| 재조정이 실제로 겨냥하는 드리프트 | **16건** (전부 주입 히치, `delta_tick ≤ 10`) |

**`reconcile_tick_drift_max = 6983`은 100% 오염이다** — 실제 재조정 관련 최대는 **8**이다(873배가 아니라 **873배**, 리더 계산 맞음).
**`reconcile_tick_drift_total = 18`은 2건이 오염이다** — 리더의 "1건"은 과소 계수다.

### C.3 이 지표를 계약 판정에 쓸 수 있는가

| 지표 | 판정 |
|---|---|
| `reconcile_tick_drift_total` | **쓸 수 있다.** 계약 (c2)는 이것을 **조건 트리거**로만 쓰고 **`speed ≥ 100`을 짝으로 요구**한다. 오염 2건은 `speed_mps = 96.3`·`0.0`이라 **(c2)에 들어오지 못한다.** 우연이 아니라 (c2)의 속도 짝이 정확히 이런 것을 거르라고 있는 것이다 |
| `reconcile_tick_drift_max` | **쓸 수 없다.** 정지 길이에 대해 무한대다. **현재 계약이 판정에 쓰지 않으므로 계약 차단은 아니다** |

> **F-28 확정 — client 수정 항목.** 조건:
> 1. **원인 기술을 고칠 것.** 세션 경계 문제가 아니다. `OnSessionReady`의 리셋을 건드리면 안 된다 — 거기엔 결함이 없다.
> 2. `reconcile_tick_drift_max`를 **`has_error == true`인 사건에 대해서만** 갱신한다. `has_error == false`인 사건은 별도 카운터(`reconcile_client_behind_total`)와 별도 최대(`reconcile_client_behind_max_ticks`)로 가른다 — **두 수를 한 통에 넣지 않는다**(계약 §7a가 반복해 잡아 온 형태다).
> 3. **F-27과 같은 자리를 고친다** — 아래 §D.2의 게이지가 들어가면 `has_error == false` 사건이 침묵하지 않게 된다.

---

## D. F-27 — 하드 스냅 카운터의 truncate 사각

### D.1 truncate 2건의 시점 — **미검증(계측)**

**로그에 truncate 사건 줄이 없다.** `catchup_truncated_total`은 5초 주기 `periodic_status`의 누계로만 나온다. 세션 2의 계열:

```
tick=1050154 trunc=0 | tick=1050254 trunc=0 | tick=1050346 trunc=1
tick=1050542 trunc=1 | tick=1050642 trunc=1 | tick=1050742 trunc=1 | tick=1050790 trunc=2
```

→ **T1 ∈ (1050254, 1050346], T2 ∈ (1050742, 1050790]** 까지만 좁혀진다. 그 창 안의 드리프트 사건은 T1이 `drift=-8 delta_tick=10`(n/a), T2가 `drift=-7 delta_tick=8`(측정됨)인데, **`delta_tick` 8·10은 `M = 20`을 넘지 않으므로 그 사건들이 truncate 프레임이라는 근거가 되지 못한다.** 400 ms 주입은 truncate를 낼 수 없다(§A.2).

> **판정: 미검증(계측). "그때 카운터가 정말 못 봤는지"는 이 로그로 답할 수 없다.** 그리고 그 사실 자체가 F-27의 요지를 강화한다 — **truncate는 일어난 흔적을 5초 누계 말고는 남기지 않는다.**

### D.2 수정 형태 — **실행으로 측정한 근거와 함께**

r9 §B.4는 사각의 원인을 truncate로 특정했다. **범위가 그보다 넓다.** 내 하네스에서 `F-27 게이지`(= 리베이스 직전 예측 위치와 직후 상태의 거리, `HasError` 무관)를 재면:

```
stall=400 ms : maxRebaseJump= 49.00 m  (hasError=False, drift=-7)  trunc=0  hard_snap=0
stall=3000 ms: maxRebaseJump=391.56 m  (hasError=False, drift=-59) trunc=1  hard_snap=0
```

> **truncate가 0인 400 ms 히치에서도 49 m 점프가 나고, `hard_snap`도 오차 표본도 0이다.** 사각의 진짜 조건은 "truncate가 났다"가 아니라 **"적용 스냅샷의 tick이 히스토리 최상단보다 앞선다"** 이고, sub-M 히치도 그것을 만든다. **r9 §B.4를 이 방향으로 갱신한다.** (391.56 m는 r9가 산술로 낸 "약 390 m"를 확인해 준다.)
>
> **그리고 이 세션은 그 점프를 실제로 4번 냈다** — §A.2b 표의 `n/a` 4건이 각각 `|drift| × 140 × 0.05` = **49 m / 42 m** 급이다. `reconcile_hard_snap_total = 0`은 그것을 한 번도 보지 못했다.

**권고하는 수정(구현은 client, 계수 여부는 architect):**

1. `PredictedShipController.Reconcile`에서 **`if (result.HasError)` 밖에** 게이지를 하나 둔다:
   `reconcile_rebase_jump_m = |CurrentState.Position(리베이스 직전) − result.State.Position|`, 그리고 `reconcile_rebase_jump_max_m`.
2. `HasError == false`인 리베이스 건수를 센다: `reconcile_rebase_without_error_total`.
3. `ReconcileTickDriftEvent`의 `position_error_m=n/a` 줄에 **`rebase_jump_m=…`을 함께 싣는다.** `n/a`는 "재지 않았다"를 옳게 말하지만, **잰 것이 아무것도 없다는 뜻이어서는 안 된다.**
4. **`reconcile_hard_snap_total`에 합칠지는 architect 판단이다.** 합치면 `hard_snap == 0`의 의미가 바뀌고 SC-56 (c1)이 이 세션에서 **FAIL이 된다**(49 m ≥ 5.0 m × 8회). 합치지 않으면 지금처럼 두 성질이 갈라진 채로 남는다. **어느 쪽이든 계약이 그 선택을 명시해야 한다** — 지금은 명시가 없어서 "0"이 무엇을 뜻하는지 리포트마다 달라진다.

---

## E. 전체 스위트 — **PASS (내가 직접 실행)**

- **실행 전 `tasklist`로 Editor 확인** → `Unity.exe` PID 16040·4224가 떠 있었다. **닫지 않고 리더에게 알렸다.** 리더가 닫은 뒤 실행했다.
- 명령: `unity test client --mode EditMode` (레포 루트, `PATH`에 Unity CLI 추가)
- 결과 파일: `C:\WorkSpace\SpaceHistoric\test-results.xml`, `start-time="2026-09-24 11:17:36Z"`

```
<test-run id="2" testcasecount="272" total="272" passed="270" failed="0" inconclusive="0" skipped="2" …>
```

**`272 / 270 / failed=0 / skipped=2` — 기준선과 동일.**

**`DefaultStallMs` 250 → 400의 영향 — 없다. 그리고 그것이 문제다.**

```csharp
// HitchInjectionTests.cs:17-23
public void DefaultStallMs_IsInsideTheVisibleDriftWindow()
{
    Assert.That(HitchInjection.DefaultStallMs, Is.GreaterThan(100));
    Assert.That(HitchInjection.DefaultStallMs, Is.LessThan(500));
}
```

결과 XML에서 이 테스트가 **실제로 돌았음을 확인**했다(`result="Passed"`, `runstate="Runnable"`, seed 2106512017). **250도 400도 이 단언을 통과한다.** r9가 "보초이지 증거가 아니다"라고 적은 그대로이며, **F-25가 근거로 든 성질(짝수 tick)은 어떤 테스트도 고정하지 않는다.** 누가 350으로 바꿔도 스위트는 초록이다.

> **§7a 단언:** 이 272건 초록은 **`DefaultStallMs` 변경에 대해 아무것도 말하지 않는다.** 변경의 유효성을 말하는 것은 스위트가 아니라 **R8 세션이 8/8 짝을 만들었다는 관측**이다(§A.1).

skipped 2건은 기준선과 같은 2건이다(신규 아님).

---

## F. 역방향 게이트 — **PASS**

```
$ python tests/e2e/check_contract_items.py --selftest
... 5 cases ...
selftest: PASS  케이스=5

$ python tests/e2e/check_contract_items.py \
    --contract _workspace/p1-01-ship-movement/02_sprint_contract.md \
    --report   _workspace/p1-01-ship-movement/10_qa_report_r10.md
계약 항목: 89
리포트가 판정표 행으로 다룬 항목: 2          ← 0이 아니다
계약에 있으나 이 리포트들에 안 나온 항목: 87 (위반 아님 — 대기·블록 분할)
위반 없음 — 판정된 항목이 전부 계약 표에 있다.
```

**판정 행 파싱 수 = 2 (SC-56, SC-89).** r9가 첫 실행에서 0행을 받았던 함정을 확인했다 — 파서는 `^\|\s*\*{0,2}SC-\d+`만 센다(`check_contract_items.py:48`), 즉 **표의 첫 칸에 SC 번호가 와야 한다.** §0.1의 표가 그 형식이다. **대조로 r9 리포트를 넣어도 2행이 나온다**(내가 실행해 확인).

---

## G. 계약 외 발견

### ① r9 §H.2a의 "홀짝 법칙"이 내 하네스에서 재현되지 않는다

r9 §H.2a: *"짝수 tick이면 위상과 무관하게 10/10, 홀수면 5/10."* **F-25가 이 근거로 250→400을 바꿨고, 그 법칙이 지금 `HitchInjection.cs`에 사실로 적혀 있다.**

내 하네스의 같은 스윕(위상 10개, 적중 = `drift ≠ 0 ∧ HasError ∧ speed ≥ 100`):

```
 50 ms (1 tick): 2/10      300 ms (6 ticks): 10/10
100 ms (2 ticks): 5/10      350 ms (7 ticks): 10/10   ← 홀수인데 10/10
150 ms (3 ticks): 10/10     400 ms (8 ticks): 10/10
200 ms (4 ticks): 10/10     450 ms (9 ticks): 10/10   ← 홀수인데 10/10
250 ms (5 ticks): 10/10     500 ms (10 ticks): 10/10
```

**홀짝 효과가 없다.** 내 모델에서 지배하는 것은 **길이**다 — 3 tick 이상이면 위상 무관 10/10, 2 tick에서 5/10, 1 tick에서 2/10.

**두 모델이 왜 갈렸는지 하나는 짚을 수 있다.** 히치는 사건을 **쌍**으로 만든다(`drift=-7 dt=8` → `drift=+7 dt=2`). **뒤쪽 회복 사건은 언제나 `HasError = true`** 이고(히스토리가 이미 그 tick을 예측해 뒀다), 내 적중 판정은 둘 중 아무 쪽이나 인정한다. r9의 적중 판정이 앞쪽(히치 프레임) 사건만 보았다면 5/10이 나올 수 있다. **실세션은 앞쪽이 4/8, 뒤쪽이 8/8이었다** — 어느 이상화 모델도 이 4/8을 예측하지 못한다.

> **판정: 두 이상화 모델이 충돌하고, 실세션은 400 ms에 대해서만 답했다(8/8).** 250 ms가 실제로 절반만 맞는지는 **아무도 실행으로 보이지 않았다.**
> **수정 요청 F-31 (client·architect):** `HitchInjection.cs`의 홀짝 법칙 단락을 **관측으로 뒷받침된 문장으로 낮추거나, 그 법칙을 고정하는 테스트를 붙인다.** 지금은 **근거 없는 단정이 소스 주석에 사실로 박혀 있고**, 값을 지키는 테스트는 `(100, 500)` 범위만 본다. 상한 500 ms(감속 시작)는 §B.5의 산술과 내 `3000 ms → 122.9 m/s` 실행이 함께 뒷받침하므로 **그대로 둔다.**

### ② 재조정 평활화가 코드에 존재하지 않는다

`RenderOffset`의 유일한 호출처는 `PredictedShipController.cs:121-123`이고, 거기서 결과는 **`HardSnapTotal`을 올릴지 결정하는 데만** 쓰인다. `GreyboxSession.RenderLocalShip()`(`:893-898`)은

```csharp
ShipSimState state = _controller.CurrentState;
_localShipView.transform.SetPositionAndRotation(ToUnity(state.Position), ToUnity(state.Orientation));
```

로 **오프셋 없이 그대로 그린다.** `grep -rn 'RenderOffset\.' --include=*.cs`가 테스트를 빼면 그 한 곳뿐이다.

> **즉 `Smooth`·`SmoothTracked` 밴드는 아무것도 평활화하지 않으며, 모든 재조정이 시각적으로는 하드 스냅이다.** 0.3151 m도, 49 m도 한 프레임에 그대로 옮겨진다. **이것이 의도된 greybox 범위 축소인지, 누락인지 계약이 말하지 않는다.** SC-56의 어느 절도 이것을 묻지 않으므로 FAIL이 아니라 **architect 판정 요청**으로 올린다. `reconcile_smooth_duration_ms = 200`이 데이터에 있는데 그것을 읽는 코드가 없다는 점을 함께 적는다.

### ③ 서버 명령 수 정정

리더 `00-session.md`는 "명령 1,148건"이라 적었다. `server-stats-after.json`의 `commands_received_total`은 **1187**이다(`start_tick=1045931`, `tick_total=12092`). 판정에 영향은 없다 — **위반 0 · 드롭 0 · 거부 전 항목 0 · `tick_overrun_total = 0`은 그대로 확인**했다. `input_superseded_total = 110`, `input_carried_forward_total = 227`.

---

## H. 수정 요청

| # | 대상 | 내용 | 조건 / 왜 |
|---|---|---|---|
| **F-27** (갱신) | client · architect | 리베이스 점프 게이지를 **`if (result.HasError)` 밖**에 둔다(§D.2 4항목). **범위 정정: truncate 전용이 아니다** — `trunc=0`인 400 ms 히치에서도 49 m가 난다(실행) | 이 세션이 그 점프를 **4번** 냈고 계측이 한 번도 못 봤다. `hard_snap`에 합칠지는 architect 결정이며, **계약이 그 선택을 명시해야 한다** |
| **F-28** (확정) | client | `reconcile_tick_drift_max`를 `has_error == true` 사건으로 한정하고, `has_error == false`는 `reconcile_client_behind_*`로 분리 | **원인 기술을 고칠 것 — 세션 경계가 아니다**(§C.1). `OnSessionReady` 리셋은 건드리지 않는다 |
| **F-29** (신규) | client | `drift == 0` 스냅샷이라도 **`position_error_m`이 `reconcile_smooth_threshold_m`(0.25) 또는 `orientation_error_deg`가 `reconcile_orientation_smooth_threshold_deg`(1.0)를 넘으면** F-21과 같은 한 줄을 남긴다 | **0.3151 m와 3.2981°의 출처를 아무도 특정할 수 없다.** p99/max만으로는 "어느 순간의 무엇인지"에 답이 없다 |
| **F-30** (신규) | client | `SESSION_READY` 뒤 첫 `WORLD_SNAPSHOT`에 자기 함선이 있었음을 한 줄로 남긴다 | SC-56 (a)를 **직접 닫는 줄이 로그에 없다.** 지금은 간접 추론이다 |
| **F-31** (신규) | client · architect | `HitchInjection.cs`의 홀짝 법칙 단락을 근거 수준에 맞게 낮추거나 테스트로 고정한다 | **독립 하네스에서 재현되지 않았다**(§G①). 근거 없는 단정이 소스에 사실로 있다 |
| **F-32** (신규) | architect | SC-56 (c3)에 `has_error == false`인 고속 드리프트 사건의 처리를 명시한다(§B.4 제안 문구) | 현 문구는 **`n/a` 4건을 통과시킨다** |
| **F-33** (신규) | architect | 평활화 부재(§G②)가 의도인지 판정한다 | `reconcile_smooth_duration_ms`를 읽는 코드가 없다 |
| **r9 §B.4 (c4) 철회 권고** | architect | 귀속을 하네스로 옮기는 절은 불필요하다 | 전제(§B.5 (4))가 반증됐다 |

---

## I. 남은 게이트 — 사람 손이 필요한 것과 아닌 것

### I.1 사람 손이 필요한 것

| 게이트 | 필요한 것 | 왜 사람인가 |
|---|---|---|
| **G-3. SC-59 촬영** | 마우스 우측 이동 ↔ 함선 우선회의 육안 대조 | r9 §C.3에서 확정 — `delta.x` 읽는 자리의 부호는 **어떤 테스트·로그로도 닫히지 않는다.** 계약 §0.10이 요구하는 "이 슬라이스가 증명하지 못하는 것"에 그대로 남는다 |
| **G-4. SC-89 (f) 플레이어 빌드 재측정** | Editor Play가 아닌 실제 빌드 | 그대로 유효 |
| **G-2b. 포커스 이탈 관찰 (목적: F-27 체감)** | 최고속 유지 → 포커스 3초(최대 4초) 이탈 → 복귀, 3회. **화면이 어떻게 보이는가만 본다** | **SC-56을 닫는 데는 더 이상 필요 없다.** 391 m 점프가 플레이어에게 어떻게 보이는지는 화면 관찰이 유일한 수단이고, §G②(평활화 부재) 판정의 입력이 된다 |

**사람 손 목록에서 빠진 것:** **"SC-56을 닫는 재측정 세션"**. 이번 세션이 닫았다.

### I.2 사람 손 없이 닫을 수 있는 것

| 게이트 | 담당 | 남은 일 | 선후 |
|---|---|---|---|
| **F-27 게이지** | client | §D.2 4항목. 내 하네스가 게이지 형태와 기대값(49.00 / 391.56 m)을 이미 준다 | 독립 |
| **F-28 분리** | client | 카운터 2개 분리 | 독립 |
| **F-29 오차 사건 로그** | client | 임계 초과 표본에 한 줄 | 독립 |
| **F-30 (a) 한 줄** | client | 로그 한 줄 | 독립 |
| **F-31 주석/테스트** | client | 주석 한 단락 또는 테스트 1건 | 독립 |
| **F-32 계약 (c3) 보강** | architect | 문구 한 줄. `contracts/` 변경 없음, 임계값 변경 없음 | **F-27 게이지가 있어야 가리킬 곳이 생긴다** |
| **F-33 평활화 판정** | architect | 판정문 | 독립 |
| **F-24 V-3·V-4 하네스 벡터** | client 또는 qa | truncate 벡터. **내 `r10probe`가 출발점이다** — 실제 `Flight`/`Sim` 소스를 그대로 쓰고 pre/post 양판본을 나란히 돌린다 | SC-56에는 더 이상 **필수가 아니다**(귀속이 세션으로 돌아왔다). 회귀 보호로는 여전히 가치 있다 |

**임계 경로는 F-27 → F-32다.** 나머지는 병렬이다.

### I.3 STATUS.md에 적을 수 있는 것

- **SC-56이 닫혔다.** R5·R6·R8·R9에 이어 다섯 번째 시도에서, **세션이 조건을 만들고(8회 주입, 전부 140 m/s), 측정이 일어나고(12/16), 대조군이 적색(수정 전 코드 49~56 m·`hard_snap = 2`)** 인 상태로 닫혔다.
- **드리프트는 주입 없이도 난다 — 흔하게, 최고속에서.** 주입이 0건인 세션 1에서 자연 드리프트 **97건**, 그중 `speed ≥ 100`이 **83건**(측정 82건, 최대 0.0037 m). 수정 전 코드라면 각각 **7.0 m → `HardSnap`** 이다. **r9 §A.2의 "이 조건은 의도적 히치 없이는 만들어지지 않는다"도 이로써 정정된다.**
- **r9의 "실서버 세션으로는 닫을 수 없다"는 판정은 틀렸다.** 원인은 §B.5가 분석한 절차(3초 truncate)와 실제로 쓴 절차(400 ms sub-M 히치)가 달랐고, **§B.5 (4)가 그 차이를 일반화로 덮었기 때문**이다. r9 §H.2a는 같은 문서 안에서 이미 반대 결과를 갖고 있었다.
- **SC-89 (a)(b)(c)(e)도 이 세션이 닫는다** — 히치가 실제로 있었다는 단언(`max_ticks_drained = 20 > 1`)이 처음으로 의도 주입으로 충족됐다. **(f)는 `catchup_truncated_total = 2 ≠ 0`이므로 architect 통지 대상**이다.
- **전체 스위트 272/270/0/2 — qa 직접 실행 확인.** 단 이 초록은 `DefaultStallMs` 변경에 대해 아무것도 말하지 않는다.
- **계측 구멍이 세 개 남았다.** (ⅰ) 가장 큰 위치 불연속(49~391 m)이 나는 경로를 `reconcile_hard_snap_total`이 못 본다(F-27). (ⅱ) `drift == 0` 스냅샷의 오차 이상치는 출처를 특정할 수 없다(F-29). (ⅲ) truncate는 5초 누계 말고 흔적을 남기지 않는다(D.1).
- **평활화가 코드에 없다**(§G②) — 밴드 이름이 약속하는 동작이 존재하지 않는다. architect 판정 대기.
- **부호는 여전히 미검증이다**(계약 §0.10, M-14) — SC-59 육안 관찰이 실행되지 않았다.
- **라운드 카운트:** 수정→재평가 3라운드 상한을 넘은 지 오래다. **다만 이번 라운드는 재평가가 아니라 판정 정정이었고, SC-56은 더 이상 열려 있지 않다.** 남은 것은 계측 보강(F-27·F-29)과 계약 문구(F-32)이며, **전부 사람 일정에 의존하지 않는다.**

---

## 부록 A. 내가 실행한 것

| 무엇 | 명령 / 위치 | 결과 |
|---|---|---|
| 원본 로그 전수 파싱 | `Editor-session.log` (2,728줄), 세션 경계 분리 | §A.1 표 |
| EditMode 전체 스위트 | `unity test client --mode EditMode` | `test-results.xml` 272/270/0/2 |
| 역방향 게이트 selftest | `python tests/e2e/check_contract_items.py --selftest` | PASS 5/5 |
| 역방향 게이트 본 실행 | `--contract 02_… --report 10_…` | 판정 행 2, 위반 0 |
| pre/post 재조정 하네스 | `%TEMP%\claude\…\84d80a95-…\scratchpad\r10probe` — `client/Assets/_Project/Scripts/{Sim,Flight}` 실소스 + `git show 8b0a78d`의 `Reconciliation` | §A.3, §B.2, §D.2, §G① |
| 임계값 diff | `git diff HEAD -- data/movement/sync-tuning.json server/crates/` | 빈 출력 |

**하네스는 레포 밖 스크래치패드에 있다.** 회귀 테스트로 승격할 가치가 있으면(F-24) `client/Assets/_Project/Tests/EditMode/`로 옮기는 것은 client 소유 작업이다 — 나는 `tests/e2e/`·`tools/bots/`만 소유한다.

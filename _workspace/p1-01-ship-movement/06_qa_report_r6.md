# p1-01 함선 이동 — 평가 리포트 라운드 6 (qa)

- 평가자: qa (독립 판정)
- 일시: 2026-09-24 KST
- 대상: 리더 R13 수정 5건(F-1·F-2·F-5·F-7·F-8) + `runInBackground` 판정 + 문서 정합성
- 도구: Unity 6 CLI(EditMode, 실행 6회), ffmpeg 7.1.1, PIL 10.4.0, python 3.12
- 규율: 계약 §0.3 · §0.12 · §7a · §7b
- 전제: **이번 라운드의 수정은 전부 리더가 직접 했고 교차 검토가 없다.** 보고된 값은 하나도 신뢰 기반으로 받지 않았다.

> **§0.3 요구 문장:** 이 라운드에도 FAIL은 0이고 **막고 있는 게이트 수는 줄지 않았다.** `미검증(증거 요건)` 2건(SC-56·SC-89)은 비-통과이며 슬라이스 종료를 FAIL과 똑같이 막는다. R13은 **증거를 만든 것이 아니라 증거를 만들 수 있게 만든 것**이고, 그 구분은 리더 자신이 R13 "남은 것"에 정확히 적었다.
>
> **SC-59는 제품 책임자 판정으로 통과다. 뒤집지 않는다.** 이 리포트가 SC-59에 대해 하는 일은 그 판정이 기대는 증거 구성의 **기록이 정직한지**를 계속 보는 것뿐이다.

---

## 0. 판정 요약

### 0.1 R13 수정 5건

| 항목 | 리더 보고 | 내 재현 | 판정 |
|---|---|---|---|
| **F-1** HUD 랩 | RED 1건 | **RED 2건** (아래 참조) | **PASS** — 수정 실재, 구조 주장도 성립(단 한 문장 과대, §2.1) |
| **F-2** ObserverSession | "컴파일·스위트 초록으로 확인" | **그 확인은 아무것도 재지 않았다** (§2.2) | **미검증** — 코드는 옳아 보이나 검출기가 0이다 |
| **F-5** 퇴화 테스트 | RED 4건 | **RED 4건 일치** ＋ 제3 퇴화(`>=`) 2건도 잡힘 | **PASS** |
| **F-7** 마커 HUD | RED 1건 | **RED 1건 일치** | **PASS** |
| **F-8** 주기 로그 13필드 | RED 2건 | **RED 2건 일치** | **부분 PASS — 축 규약 주석이 틀렸고 기준 5-b는 여전히 수집 불가** (§2.5, 이번 라운드 최대 발견) |

### 0.2 계약 항목

| SC | 판정 | 한 줄 근거 |
|---|---|---|
| SC-46 | **PASS** | `unity test client --mode EditMode --report-format nunit,junit` exit 0, `total=237 passed=235 failed=0 skipped=2`, 리포트 2개 생성 |
| SC-56 | **미검증(증거 요건)** | (d)②③이 **이제 화면에 뜬다**(F-1 수정 실측 확인, §2.1) — 그러나 이번 라운드에 실서버 60초 세션 신규 촬영 없음. 코드 읽기는 PASS가 아니다 |
| SC-59 | **통과 — 제품 책임자 판정** (qa 판정 아님) | 기록 검증만 수행(§4). 정정 6건 중 **L-2 미대응**, §6.4는 F-8 이후에도 **절반만** 실현 가능 |
| SC-89 | **미검증(증거 요건)** | (b)의 자명 통과 지적 **여전히 미해소** — 증거원을 바꾼 것이지 증거를 만든 것이 아니다. 1002 태그 (i)(ii) **닫힘**, **(iii) 안 닫힘**(§3.2) |

### 0.3 결론 한 줄

**리더의 다섯 건 중 셋은 보고 그대로였고, 하나는 보고보다 강했고, 하나는 검증이 없었다. 그리고 F-8의 핵심 주석이 틀렸다.**

---

## 1. 먼저 — md5 4건 중 2건이 트리의 어떤 파일과도 일치하지 않는다

R13은 네 건 모두 "원복 후 md5 일치"로 닫았다. 실제 트리는 이렇다.

| 파일 | 리더가 적은 md5 | 현재 트리 | |
|---|---|---|---|
| `GreyboxSession.cs` | `1f5ccf81fc92958ec3f9032cfe37fdd1` | `02d0a577e4be549bad09f9126d4cda70` | **불일치** |
| `SnapshotRebaseBatch.cs` | `47e66ef800479426bcae4de45984305d` | `47e66ef800479426bcae4de45984305d` | 일치 |
| `MarkerHudLine.cs` | `21fb3b1fa6d551255a72c01e978610bb` | `21fb3b1fa6d551255a72c01e978610bb` | 일치 |
| `PeriodicStatusLog.cs` | `7f51ec1b203e6feb01213a9eeab2efa6` | `b958e5d757ce10aeb65c10e15df63776` | **불일치** |

```
$ find client/Assets/_Project -name "*.cs" -exec md5sum {} + \
    | grep -iE "1f5ccf81fc92958ec3f9032cfe37fdd1|7f51ec1b203e6feb01213a9eeab2efa6"
(출력 없음, exit=1)
```

**트리의 어떤 `.cs` 파일도 그 두 해시를 갖고 있지 않다.** 가장 그럴듯한 설명은 선의의 것이다 — F-1의 md5를 찍은 뒤 F-7이 `GreyboxSession.OnGUI`/`BuildMarkers`에 마커 줄을 넣었고(`GreyboxSession.cs:232`, `:891`), F-8의 md5를 찍은 뒤 `runInBackground` 판정이 `PeriodicStatusLog.cs` 헤더를 다시 썼다. 순서상 자연스럽다.

**그래도 이것은 결함이다.** R13은 그 해시를 "결함 주입을 원복했다는 증명"으로 제시했고, **제3자는 그 증명을 재현할 수 없다.** 해시는 찍은 시점을 함께 적지 않으면 뒤따른 편집과 구분되지 않는다. 이번 라운드의 원복 증명은 **리더의 해시가 아니라 내가 직접 뜬 기준선**에 기댄다(§2.0).

**수정 요청 (L-7, 리더):** R13의 md5 4건 옆에 "이 해시는 그 항목 작업 시점의 것이며, 이후 F-7/판정 작업이 같은 파일을 다시 건드렸다"를 명시한다. 해시 자체는 고치지 않는다(절대 원칙 5 — 당시 기록으로 옳다).

---

## 2. A·B — RED 재현과 수정의 §7a/§7b 적합성

### 2.0 절차 (§0.12 준수 — 갱신 경로 아님, 백업/복원)

네 파일을 스크래치패드에 백업하고 기준 md5를 뜬 뒤, 한 건씩 주입 → 전체 스위트 → 백업에서 복원 → md5 재확인을 반복했다. 표식은 `QA-R6 TEMP DEFECT`.

```
기준선(내가 뜬 것):
  GreyboxSession.cs      02d0a577e4be549bad09f9126d4cda70
  SnapshotRebaseBatch.cs 47e66ef800479426bcae4de45984305d
  MarkerHudLine.cs       21fb3b1fa6d551255a72c01e978610bb
  PeriodicStatusLog.cs   b958e5d757ce10aeb65c10e15df63776
  ObserverSession.cs     3b3867c2f7821a7cabd9c97357eb2dca

종료 후: 5개 전부 위 값과 일치
$ grep -rn "QA-R6 TEMP DEFECT" client/ tools/ tests/ | wc -l   →  0
$ unity test client --mode EditMode … EditMode.GREEN-after-restore.nunit.xml
  → exit 0, total=237 passed=235 failed=0 skipped=2 (start 2026-09-23 23:15:55Z)
```

### 2.1 F-1 — **PASS**, 그리고 리더가 보고보다 강하게 고쳤다

**RED:** `BuildHudRows`의 `rows.AddRange(HudTextWrap.Wrap(line, HudMaxCharsPerLine))` → `rows.Add(line)`.

```
exit=8, total=237 passed=233 failed=2
FAILED HudTextWrapTests.BuildHudRows_Sc56AndSc89Lines_AllFieldsAppearSomewhereInOutput
FAILED MarkerHudLineTests.Format_FourMarkerLine_SurvivesTheHudWrapWithEveryFieldIntact
```

**리더는 "1건 실패"라고 적었다. 실제로는 2건이다** — F-7의 마커 테스트가 F-1의 랩에 묶여 있어 함께 붉어진다. 과대 보고가 아니라 **과소** 보고이므로 결함으로 올리지 않고 기록한다. §7a 단언: 겨냥한 조건이 실제로 발생했다 — 두 실패 모두 `row.Length <= HudMaxCharsPerLine` 단언이 켠 불이고, 길이 단언이 없는 `Does.Contain` 쪽은 양쪽에서 초록이다(랩이 없어도 내용은 보존되므로 — 옳은 거동이다).

**(B) "폭을 늘린 게 아니라 구조를 바꿨다"는 참인가 — 대체로 참이다. 경계 케이스를 직접 따졌다.**

| 입력 | `Wrap(text, 110)` | 손실 |
|---|---|---|
| `""` | `[""]` | 없음 |
| `null` | `[""]` | 없음(NRE 아님) |
| `"   "`(공백만) | `[""]` | 공백 런만 정규화. **필드 손실 아님** |
| `"a  b"`(연속 공백) | `["a  b"]` | 없음 — 내부 연속 공백 보존 |
| `" abc"`(선행 공백) | `["abc"]` | 선행 공백 1개. 필드 손실 아님 |
| 110자 초과 단일 토큰 | `[토큰]` (1행, 250자 그대로) | 없음 — **다만 행 길이 ≤110 보장이 깨진다** |
| `maxCharsPerLine <= 0` | `[text]` | 없음 |

**내용 보존은 진짜 불변식이다.** 반면 **"행 길이 ≤ 110"은 불변식이 아니다** — 110자보다 긴 단일 토큰은 자기 행에 통째로 남고 `GUI.Label`에서 **예전과 똑같이 잘린다.** 테스트 파일이 두 성질을 동시에 단언하는데(`…SingleTokenLongerThanLimit…`는 250자 1행을, `BuildHudRows_…`는 전 행 ≤110을) 모순이 아닌 이유는 **생산 라인이 전부 공백 구분이기 때문**이다. 현재 최장 토큰은 `cl2_reconcile_both_omega_nonzero_total=99999999`(46자)이므로 실위험은 없다. 그러나 R13의 *"필드가 늘어나면 행이 늘지 조용히 잘리지 않는다"*는 **공백 없는 긴 필드 하나면 깨진다.** 문장을 좁힐 것을 권한다(L-8).

**(B) 110자가 900 px에 정말 들어가는가 — 들어간다. 재서 확인했다.**

리더 말대로 코드에 픽셀 검증이 없어서(테스트 주석이 그 회피를 명시한다) 이 짝은 **테스트로 닫히지 않는 가정**이다. 그래서 프레임에서 직접 쟀다.

`evidence/R4-B6/sc59-v2/scan/grid3.png`(1240×720, 1:1 네이티브) 좌상단 HUD를 3배 확대:

```
reconcile_error_m p50=0.0000 p99=0.0009 max=216.8781 n=2216      ← 59자
```

이 줄이 x ≈ 7 px에서 시작해 x ≈ 397 px에서 끝난다 → **≈ 390 px / 59자 = 6.6 px/자.**

- **역검산(§7a):** 같은 6.6 px/자로 옛 잘림 지점을 예측하면 — `cl2_reconcile_has_error_total=2216`(34자 ≈ 225 px) + 공백 + 다음 토큰 37자(≈ 245 px) = **477 px > 400 px** → 그 토큰은 400 px 라벨에 못 들어간다. **qa4가 관측한 잘림 위치와 정확히 일치한다.** 즉 이 6.6 px/자는 추정이 아니라 두 방향에서 확인된 값이다.
- **판정:** 110자 × 6.6 = **726 px ≤ 900 px.** 여유 174 px(≈ 26자). **F-1의 두 상수는 정합한다.**
- 화면 밖으로 나가지도 않는다: 박스는 `x=8, w=916`이고 녹화 해상도는 **3440×1440**(`ffprobe SC-59-v2.mp4`)이다.

**§7a 단언:** "SC-56 (d)②③과 SC-89 판별 단언이 이제 화면에 뜬다"는 **구조적으로도(테스트) 픽셀로도(실측) 성립한다.** 다만 **그 값이 0이 아닌지는 여전히 미관측**이다 — 그래서 SC-56·SC-89는 통과가 아니다.

### 2.2 F-2 — **미검증. 리더의 확인은 아무것도 재지 않았다**

R13: *"컴파일·스위트 초록으로 확인."* 이것이 §7a가 말하는 그 결함이다. 확인해 보았다.

```
$ grep -rn "ObserverSession" client/Assets/_Project/Tests/ | wc -l
1                 ← ObserverCsvTests.cs:8 의 주석 한 줄. 실행 코드는 0건
```

그래서 **수정을 통째로 무력화하고 스위트를 돌렸다.** `ObserverSession.Update()`에서 `ApplyPendingRebase();` 호출을 제거(= F-2 수정이 없던 상태와 동치: 스냅샷은 pending에 들어가기만 하고 `Reconcile`도 `WriteRow`도 영영 일어나지 않는다):

```
exit=0, total=237 passed=235 failed=0 skipped=2
```

**"검증했다"는 초록과 바이트 단위로 같다.** 수정이 옳든, 틀렸든, 아예 없든 스위트는 같은 값을 낸다. 리더의 확인은 **0을 0과 비교한 것**이고, 이 슬라이스가 이미 네 번 데인 형태다(CLAUDE.md 검증의 규율).

**코드 자체는 읽어서 옳아 보인다** — `ObserverSession.cs:182,196-200`이 `SnapshotRebaseBatch.ShouldReplacePending`으로 최고 tick만 남기고, `:232-247`의 `ApplyPendingRebase()`가 `Update()`의 `Pump()` **직후**(`:277,284`)에 한 번만 `Reconcile` + `WriteRow`한다. `GreyboxSession`과 같은 형태다. 그러나 **읽기는 PASS가 아니다**(§0.3).

**수정 요청 (F-2′, client):** 순서 의존이 이 수정의 전부다(`ApplyPendingRebase()`가 `Pump()` **뒤**, `_input.Sample()` **앞**). 그 순서를 깨면 지금은 아무 불도 안 켜진다. 최소 한 건 — 한 프레임에 tick 100·102·101 세 스냅샷을 넣고 **`WriteRow`가 한 번만, tick 102로** 일어남을 단언하는 테스트가 필요하다. `SnapshotRebaseBatchTests.Batch_OfThreeArrivingOutOfPumpOrder…`는 순수 결정만 재고 배선은 재지 않는다. **(같은 지적이 `GreyboxSession`의 H-9 배선에도 해당한다 — 기록.)**

### 2.3 F-5 — **PASS. 제3 퇴화 형태도 잡는다**

**RED(리더가 보고한 형태):** `ShouldReplacePending` → `return true`.

```
exit=8, total=237 passed=231 failed=4      ← 리더 보고와 일치
FAILED SnapshotRebaseBatchTests.Batch_OfThreeArrivingOutOfPumpOrder_OnlyHighestTickEverWins
FAILED …ShouldReplacePending_CandidateEqualsPending_False
FAILED …ShouldReplacePending_CandidateOlderThanPending_False
FAILED …ShouldReplacePending_DegenerateAlwaysTrueOrAlwaysFalse_WouldFailAtLeastOneCase
```

**리더가 물은 제3 퇴화(부등호 방향/`>=` vs `>`)를 내가 추가로 돌렸다:** `candidateTick > pendingTick.Value` → `>=`.

```
exit=8, total=237 passed=233 failed=2
FAILED …ShouldReplacePending_CandidateEqualsPending_False
FAILED …ShouldReplacePending_DegenerateAlwaysTrueOrAlwaysFalse_WouldFailAtLeastOneCase
```

**잡는다.** 표 4케이스의 판별력을 따로 따지면:

| 퇴화 | 깨지는 케이스 |
|---|---|
| always-true | `(100,100,false)`, `(101,102,false)` |
| always-false | `(100,null,true)`, `(102,100,true)` |
| `>=` | `(100,100,false)` |
| 부등호 반전(`<`) | `(102,100,true)` |
| `!pendingTick.HasValue` 제거 | `(100,null,true)` |

**표 자체가 충분하다.** 그리고 새 테스트는 `SnapshotRebaseBatch.ShouldReplacePending`을 4회 실제 호출한다(상수 접힘 없음) — qa4 F-5가 지적한 병이 실제로 제거됐다.

### 2.4 F-7 — **PASS. `markers=none`도 §7a 논거대로 동작한다**

**RED:** `MarkerHudLine.Format`에서 `shipPositionM` 무시(`marker.PositionM.X - shipPositionM.X` → `marker.PositionM.X`).

```
exit=8, total=237 passed=234 failed=1      ← 리더 보고와 일치
FAILED MarkerHudLineTests.Format_DistanceIsMeasuredFromTheShip_NotFromTheOrigin
```

**`markers=none`:** `Format()`은 `markers == null || Count == 0`에서 `"markers=none"`을 낸다(빈 문자열 아님). `Format_NoMarkers_SaysNone_NotAnEmptyString`이 null과 빈 리스트 양쪽을 단언한다. §7a 논거 — "빈 행은 안 그려진 행과 구분되지 않는다" — 를 그대로 만족한다.

**가려지거나 잘릴 여지 — 없다.** 실제 데이터로 계산했다:

```
markers=star-cradle=0.0m planet-vela=3138.5m beacon-meridian=2981.6m derelict-unnamed=4202.4m
→ 93자 (함선이 원점일 때). 거리는 하드 경계 12 km 까지이므로 최대 ~97자
→ 110자 한계 미만 → 절대 랩되지 않고, 6.6 px/자 × 93 ≈ 614 px ≤ 900 px
```

`_starSystem?.ReferenceMarkers`가 null일 때(데이터 로드 전)도 `markers=none`이 그려지므로 "행 자체가 없는" 상태는 발생하지 않는다.

### 2.5 F-8 — **부분 PASS. 이번 라운드 최대 발견이 여기 있다**

**RED:** `Format()`에서 `thrust_y`↔`thrust_z` 교환.

```
exit=8, total=237 passed=233 failed=2      ← 리더 보고와 일치
FAILED PeriodicStatusLogTests.Format_AttitudeThrustRollAndBoundary_EachFieldAppearsWithItsOwnValue
FAILED PeriodicStatusLogTests.Format_NegativeThrustAndRoll_KeepTheirSign
```

여기까지는 보고대로다. **문제는 그 13개 필드가 실제로 SC-59 기준 2와 5-b를 제3자에게 열어 주는가이고, 리더가 이것이 F-8의 존재 이유라고 옳게 적었다. 직접 유도해 봤다.**

#### (1) 축 규약 주석이 **틀렸다** — 부호가 정확히 반대다

`PeriodicStatusLog.cs` 필드 블록:

> ```
> // Roll is the scalar rate about ship-local +Z (Sim/ShipSimState.cs:19); positive roll is
> // right-hand-rule about +Z, i.e. the ship's right wing drops.
> ```

**"right-hand-rule about +Z"는 맞고, "the ship's right wing drops"는 틀렸다. 오른손 법칙으로 +Z 축을 돌리면 오른쪽 날개는 올라간다.**

실제 적분기를 그대로 옮겨 수치로 확인했다 — `ShipIntegrator.cs:147-149`(`w2 = fwd * omegaRoll`, `q = q + (Quatd.Pure(w2) * q) * (0.5*dt)`, `Normalized()`)와 `Quatd.cs`(Hamilton 곱, `Rotate(v) = q·(v,0)·conj(q)`):

```
+90 deg/s 롤을 1초 (dt=0.05, 20 step) 적용:
  함선 오른쪽(+X)  →  월드 (0.0008, 1.0000, 0.0000)   = 월드 +Y = 위
  함선 위(+Y)      →  월드 (-1.0000, 0.0008, 0.0000)  = 월드 -X = 왼쪽
```

**양의 롤은 우현 날개를 들어올린다.** 주석이 유도를 도우려고 붙인 단 하나의 물리 문장이 거꾸로다. **F-8의 존재 이유가 "제3자가 눈 없이 방향을 판정하게 한다"인데, 그 제3자가 이 주석을 읽으면 뱅크 방향을 반대로 읽는다.** 이것은 계약 §0.10이 "부호 버그 검출기"라고 부른 바로 그 축의 문서화 결함이다.

**수정 요청 (F-9, client):** `PeriodicStatusLog.cs`의 해당 문장을 *"positive roll rotates ship-+X toward ship-+Y, i.e. the ship's right wing RISES (verified against ShipIntegrator step 5)"*로 고친다. 같은 문장이 다른 곳에 복제돼 있지 않은지 함께 확인한다.

#### (2) 기준 2(선회 방향) — **유도 가능하다. 유도해 봤다.** 단 주석이 두 가지를 빠뜨렸다

절차:

```
q_rel = conj(q_t1) * q_t2                     (t1 자세 기준의 상대 회전)
vec(q_rel) 을 함선 로컬 +Y(위) 에 투영 > 0     →  뱃머리가 +X(우현) 쪽으로 갔다 = 오른쪽 선회
```

수평 상태에서 30° 우선회한 자세로 검산:

```
+Y 양의 회전: 뱃머리 → (+0.500, 0.000, +0.866)  = +X 쪽 = RIGHT,  attitude_y_micro = +258819
-Y 양의 회전: 뱃머리 → (-0.500, 0.000, +0.866)  = -X 쪽 = LEFT,   attitude_y_micro = -258819
```

**유도는 성립한다.** 그리고 `aim_target_*`에 같은 절차를 돌리면 "플레이어가 오른쪽을 겨눴다"와 "함선이 오른쪽으로 따라갔다"를 **한 줄 안에서 짝지을 수 있다** — 리더가 `Attitude*`와 `AimTarget*`을 둘 다 남긴 판단은 옳다.

**그러나 ADR-0009 §1과 이 주석만으로는 부족하다.** 두 가지가 없다:

1. **회전을 어떻게 적용하는가** — `v' = q·(v,0)·conj(q)`, Hamilton 곱. 이 사실은 `Quatd.cs`에만 있다. ADR-0009 §1의 표는 *"쿼터니언 (x,y,z,w), Hamilton 곱. 양의 각속도 = Unity의 양의 회전 방향"*까지만 적고, **Unity의 양의 회전 방향이 무엇인지는 적지 않는다.** 제3자는 Unity 문서를 찾아가거나 소스를 읽어야 한다.
2. **`attitude_*`가 월드 프레임이라는 사실** — `GreyboxSession.cs:694-697`은 `_controller.CurrentState.Orientation`(월드 자세)을 찍는다. 그래서 **`attitude_y > 0`을 그대로 "오른쪽"으로 읽으면 틀린다** — 수평·+Z 정면에서 출발했을 때만 맞고, 일반적으로는 위의 상대 회전 계산이 필요하다. 주석은 프레임을 한 글자도 말하지 않는다.

**수정 요청 (F-10, client):** 필드 블록에 두 줄을 더한다 — *"rotations apply as v' = q·(v,0)·conj(q) (Hamilton, see Quatd.cs)"*, *"attitude_\* and aim_target_\* are WORLD-frame orientations; turn direction is read as q_rel = conj(q_prev)·q_now and the sign of vec(q_rel) on the ship-local up axis, never from a raw component sign."* 그리고 **판독 예제 한 줄**(우선회 30° → `attitude_y_micro ≈ +258819`)을 붙이면 제3자가 소스를 열지 않고 닫을 수 있다.

#### (3) 기준 5-b(오토레벨) — **여전히 수집 불가. F-8은 이 절을 열지 못했다**

`ShipIntegrator.cs:122`:

```csharp
if (input.Roll != 0.0 || !input.FlightAssist)   // 수동 롤
{ … }
else                                            // 오토레벨
{ … }
```

**오토레벨은 `FlightAssist`가 켜져 있을 때만 돈다. 그런데 `flight_assist`는 13개 필드에 없다.** `brake`도 없다.

그래서 로그에서 "손을 뗐는데 수평으로 안 돌아왔다"를 보아도 **"어시스트가 꺼져 있었다"와 "오토레벨이 고장났다"를 구분할 수 없다.** 계약 SC-59 (d)는 *"① 손 뗐을 때 수평 복귀와 ② 수동 롤 중 복귀가 일어나지 않음을 한 클립에"*를 요구하고, **②는 `roll` 필드로 닫히지만 ①은 닫히지 않는다.** HUD에는 `assist=`가 있다(`GreyboxSession.cs` `lines` 목록) — **로그에만 없다.** F-1의 교훈("증거원은 화면이 아니라 로그여야 한다")을 그대로 적용하면 이것은 빠지면 안 되는 필드다.

자세 쪽은 문제없다: `rot(q,(1,0,0)).y → 0` 수렴으로 "수평을 되찾았다"를 자세 4필드만으로 계산할 수 있다.

**수정 요청 (F-11, client, 우선):** `PeriodicStatusLog`에 `flight_assist`·`brake` 두 bool을 추가한다. HUD가 이미 찍는 값이고 같은 `_input`에서 온다. **이것이 없으면 §6.4의 "기준 5-b를 로그로 수집한다"는 F-8 이후에도 여전히 거짓이다.**

#### (4) 부수 — §7a를 자기 자신에게 적용하지 않은 곳 하나

`GreyboxSession.cs:681-697`은 `_input != null ? … : 0`이다. 샘플러가 없으면 `thrust_x=0 roll=0 attitude_w=0`이 찍히는데, 이것은 **"재봤더니 0"과 구분되지 않는다.** 같은 파일이 `send_burst_*`에 대해서는 `n/a`로 정확히 그 구분을 한다. 특히 `attitude_w=0`은 정규화된 쿼터니언에서 나올 수 없는 값이라 침묵하는 센티널이 되어 있다. 낮은 우선순위지만 기록한다(F-12).

#### (5) 배선에 테스트가 없다

8건은 전부 `Format()`만 잰다. `MaybeLogPeriodicStatus()`의 **생성자 호출**(`attitudeY: … Orientation.Y`)에서 축을 바꿔 넣어도 8건은 전부 초록이다. 읽어서 옳음은 확인했다(`GreyboxSession.cs:681-698`, 13필드 모두 올바른 멤버). F-2와 같은 종류의 공백이므로 같이 기록한다.

### 2.6 계약 외 발견 — 롤 축이 이 슬라이스에서 가장 덜 검증된 축이다

`ShipInputSampler.cs:85-86`: `E → roll += 1.0`, `Q → roll -= 1.0`.
§2.5(1)의 실측: **양의 롤 = 우현 날개 상승 = 좌측 뱅크.** 즉 코드상 **E를 누르면 왼쪽으로 기운다.**

이것을 SC-59 판정에 쓰지 않는다 — **SC-59는 제품 책임자 판정으로 통과이고 나는 그것을 뒤집지 않으며, 게임을 관측하지도 않았다.** 다만 두 가지를 기록으로 남긴다.

1. qa4 §2.1 표 5-a가 이미 적었다: 롤을 담은 유일한 프레임(`h270.png`)은 `roll=-1000`이면서 **`speed_mps=0.0`, 정지 상태**였고, 같은 프레임의 `ack_input_seq`는 더 이전 입력의 ack다. **증언 3건 + 프레임 6건 중 롤 방향을 닫은 것은 하나도 없다.**
2. 그래서 **다음 촬영에서 (d) 클립은 롤 방향까지 명시적으로 담아야 한다.** `E` → 화면에서 어느 쪽으로 기우는지. 계약 SC-59 (a)는 추력·마우스 선회만 열거하고 롤 방향을 열거하지 않으므로, 이것은 **계약 문구의 공백**이기도 하다 → architect 사안.

---

## 3. C — `runInBackground` 판정

### 3.1 논거는 성립하지 않는다 (결론은 유지할 만하다)

R13: *"catch-up 버스트는 `Update()`가 멈췄다 재개하기 때문에 존재한다. `runInBackground = true`로 바꾸면 SC-89가 재려던 현상이 사라진다."*

코드로 따졌다. 버스트를 만드는 것은 `GreyboxSession.Update()`의

```
_tickAccumulator += Time.unscaledDeltaTime;                     (히치가 여기로 들어온다)
TickCatchUp.Compute(… ProductionMaxSendsPerFrame, ProductionMaxPredictedTicksPerFrame)
for (i < plan.TicksToPredict) { isSendSlot → TrySend… }         (여기서 버스트가 난다)
```

이고, 입력은 **`Time.unscaledDeltaTime`이 크다는 사실 하나**다. `runInBackground = true`가 없애는 것은 **정지(pause)이지 큰 dt가 아니다.** 비포커스 Editor는 프레임률을 2~4 Hz로 조이므로 dt는 250~500 ms가 되고, **매 프레임 5~10 tick을 드레인한다.** 버스트가 사라지기는커녕 **더 자주, 더 작게** 난다.

그리고 계약이 바로 그 프로파일을 **좋은 재현 벡터**라고 적어 두었다(SC-89 방법 칸, architect R4 보충 §F):

> **백그라운드에서 Editor 가 2~4 Hz 로 조이면** 2 Hz = 프레임당 10 tick = **매 프레임 위반 1회** → 4초면 예산이 찬다.

즉 `runInBackground = true`는 **SC-89가 재려던 현상을 지우는 것이 아니라, 계약이 선호한다고 적은 재현 조건을 보장하는 쪽**이다. 리더의 인과가 거꾸로다.

**그렇다고 바꾸라는 뜻은 아니다. 결론은 유지할 만하고, 더 나은 근거가 이미 계약에 있다.**

- 프로젝트 설정을 바꾸는 것은 **피시험 시스템을 바꾸는 것**이고, 증거를 만들려고 그것을 하는 것이 §7b(1)이 경계하는 형태다. 이 문장만으로 충분하다.
- 계약 SC-89 방법 칸이 이미 절차를 준다: ① **먼저 계측으로 throttle/pause를 가른다** ② **pause로 판명되면 대체 벡터**(도메인 리로드 반복·강제 GC 루프로 **전경에서** 반복 히치). 어느 쪽이든 필요한 것은 *"10초 안에 450 ms 이상 히치 8회"*뿐이다. **설정을 건드릴 필요가 애초에 없다.**

**수정 요청 (L-9, 리더):** R13 판정 문단의 *"SC-89가 재려던 현상이 사라진다"* 문장을 정정 레코드로 뒤집는다(절대 원칙 5 — 본문은 고치지 않는다). 대체 문장: *"설정을 바꾸면 버스트가 사라지는 것이 아니라 히치 프로파일이 바뀐다. 바꾸지 않는 이유는 증거를 얻으려고 피시험 시스템을 바꾸지 않기 위해서이고, 계약 SC-89 방법 칸이 전경 대체 벡터를 이미 지정하고 있어 바꿀 필요가 없다."*

**H-13 역할 축소에 대해:** "OnGUI·화면 캡처 비의존 로거"라는 재정의 자체는 옳고 가치가 있다. 다만 계약 SC-89 방법 칸이 H-13에 부여한 역할은 *"`OnGUI` 는 비포커스면 호출되지 않으므로 HUD 만으로는 백그라운드 구간의 증거가 원리적으로 안 남는다"*에 대한 답이었다. **그 역할은 충족되지 않았고, 충족하지 않기로 한 것**이다. 기록으로 명확히 남겨야 한다(L-9에 포함).

### 3.2 대체 증거가 SC-89를 닫는가 — **(a)~(f)는 원리상 닫을 수 있다. (iii)은 못 닫는다. 그리고 이번 라운드엔 아무것도 닫히지 않았다**

**(a)~(f) — 리더가 옳다.** 결정적인 이유는 리더가 적지 않았으므로 여기 적는다: **백그라운드 간극이 만드는 버스트는 복귀 프레임에서 일어나고, 그 프레임은 포커스가 있다.** `SendBurstStats.RecordUpdate(plan.TicksToPredict)`와 `RecordFrameSendCount(sendsThisFrame)`가 `Update()` 안에 있으므로(`GreyboxSession.cs:639-640`), **간극이 만든 최악값은 세션 카운터에 정확히 들어간다.** 따라서 `OnSessionEnded` 덤프는 (a) 프레임당 송신 최대, (b) `catchup_carry_forward_ticks_total`·`reconcile_hard_snap_total`·드레인 max, (f) `reconcile_forced_after_hitch_total`·`catchup_truncated_total`을 전부 담는다. (c)는 grep+테스트, (d)는 `/debug/stats`, (e)는 `git diff`로 세션과 무관하다.

**1002 태그 (i)(ii)(iii):**

| | 판정 | 증거 |
|---|---|---|
| (i) `close_code(ProtocolViolation) == 1002` 그대로 | **닫힘** | `git diff --stat server/crates/gateway/src/runtime.rs` → **빈 출력**. `runtime.rs:191 SessionCloseReason::ProtocolViolation => Some(1002)` |
| (ii) `ReconnectPolicy` 양성 대조 유지 | **닫힘** | 오늘 리포트에서 4건이 **양방향으로** Passed: `Disconnect_WithProtocolViolationCloseCode_StillReconnects` · `…_LogsAGreppableLine` · `Disconnect_WithSupersededCloseCode_DoesNotReconnect` · `Disconnect_WithSomeOtherCloseCode_DoesNotLogTheProtocolViolationLine`. **§7a: 갈림이 실제로 갈린다 — 1002는 재접속+로그, 4001은 로그만+재접속 없음, 그 외는 재접속만+로그 없음** |
| (iii) 새 태그가 **실제로 찍히고 DB `close_reason`과 짝지어진다** | **안 닫힘** | 이것은 **실서버 + 위반을 실제로 내는 클라이언트**를 요구한다. `OnSessionEnded` 덤프도 `application_focused` 전이도 이 짝을 만들지 못한다. **리더의 "전부 닫힌다"는 이 절에 대해 틀렸다** |

**그리고 가장 중요한 것:** 증거원을 어디로 옮기든 **qa4 §6.3의 (b) 자명 통과 지적은 그대로다** — 초록이 나온 상태가 `reconcile_replayed_nonzero_total = 0` + 함선 정지였다는 사실은 *"어디서 읽는가"*가 아니라 *"히치가 실제로 있는 세션을 돌렸는가"*의 문제다. **이번 라운드에 그 세션은 없다.** SC-89는 **미검증(증거 요건)** 그대로다.

### 3.3 순환 논증 제거 — **확인됨**

`PeriodicStatusLog.cs` 헤더에 `CORRECTION (R13, leader, after qa r5 §E)` 문단이 들어갔고, TickCatchUp/ADR-0012를 근거로 들던 문장이 **삭제**됐다. 새 문장은 *"the honest statement is the opposite: Update() very likely does NOT run while the Game View is unfocused"*이고, 주장하는 범위를 *"wherever Update() runs, a line is written"*으로 정확히 좁혔다. **§7a 상 옳은 형태다.**

---

## 4. D — 계약·문서 정합성

### 4.1 정정 6건 대 qa4의 L-1′~L-6 — **L-2가 빠졌다**

| qa4 지적 | `12-correction-leader.md` | |
|---|---|---|
| L-1′ §6.2 Fact 칸 3행 | 정정 1 | **대응** |
| L-2 다섯 기준이 계약 절과 대응하지 않는다 | — | **미대응**(정정 6이 "원인"으로만 언급) |
| L-3 `cmds 5804` 출처 없음 | 정정 3 | **대응** |
| L-4 §5.2 각주 짝값 미관측 | 정정 2 | **대응** |
| L-5 §6.1 절 구분 | 정정 6 | **대응** |
| L-6 §6.4 실현 불가 | 정정 5 | **대응** |
| (F-6 빌드 신선도, 기록 항목) | 정정 4 | **추가 대응 — 요구하지 않은 것까지 적었다** |

**L-2의 실행 항목이 아무 데도 없다.** L-2가 요구한 것은 *"다음 촬영의 기준표는 계약 절 번호((a)(b1)(b2)(c)(d))를 그대로 쓴다"*이고, 그 기준표가 들어갈 자리는 재촬영 체크리스트다. 그런데 `evidence/R4-B6/sc59/09-reshoot-checklist.md`는 **R13 이전 상태 그대로다**:

- HUD를 *"대략 8,8 기준 **420**×20+18×n px 박스"*로 설명 — **F-1 이후 916 px다.**
- `GreyboxSession.cs:512` 부근을 가리킴 — **현재 `OnGUI`는 :838이다.**
- 확인할 줄 목록에 **마커 줄(F-7)이 없다.**
- **주기 로그(`periodic_status`) 수집 단계가 없다** — F-8 필드를 추가해 놓고 그것을 모으는 절차가 절차서에 없다.
- 기준표가 여전히 계약 절 번호가 아니다.

**수정 요청 (L-2′, client 또는 리더):** 재촬영 체크리스트를 R13 트리 기준으로 갱신하고, 기준표를 계약 SC-59의 (a)(b1)(b2)(c)(d)로 바꾼다. **이 문서는 다음 촬영의 작업 지시서이고, 낡은 채로 두면 R13의 세 수정이 촬영에 반영되지 않는다.**

### 4.2 `11-sc59-observation.md` §6.4는 이제 실현 가능한가 — **절반만**

| 기준 | F-8 이전 | F-8 이후 | 근거 |
|---|---|---|---|
| 2. 선회 방향 | 불가 | **가능** | `attitude_*` + `aim_target_*`, §2.5(2)에서 유도 검산 완료. 단 주석 보완(F-10) 필요 |
| 4. 경계 | 부분 가능 | **가능** | `boundary_soft_crossed` bool이 생겼다 — 반경을 몰라도 읽힌다 |
| 5-b. 오토레벨 | 불가 | **여전히 불가** | `flight_assist` 없음(§2.5(3)). 자세 쪽은 되지만 어시스트 on/off를 못 가린다 |

**기록할 자리:** 절대 원칙 5에 따라 `11-sc59-observation.md`를 고치지 않고 **`evidence/R4-B6/sc59-v2/13-correction-leader-2.md`(새 레코드)**에 위 표를 남긴다. 그리고 같은 사실이 **`03_client_impl.md` R13 "남은 것"** 에도 한 줄 필요하다 — 현재 그 절은 F-8을 완료로만 적고 5-b가 아직 막혀 있다는 말을 하지 않는다. (L-10)

### 4.3 역방향 게이트

```
$ python tests/e2e/check_contract_items.py --selftest
OK   위반 — 리포트가 SC-87을 판정했는데 계약에 없다 (p1-01 실제 사례) :: 위반=[87] 기대=[87]
OK   깨끗 / OK 순방향 / OK 산문 / OK 여러 리포트 합산
selftest: PASS  케이스=5
```

**§7a 단언:** 양성 대조(SC-87)가 실제로 빨간불을 켠다 — `0건 파싱 후 위반 0`이 아니다. 이 리포트가 판정한 계약 항목은 **SC-46·56·59·89 네 건**이고 전부 계약 §1 표에 있다. F-1·F-2·F-5·F-7~F-12·L-7~L-10은 **SC 번호가 없는 계약 외 항목**이므로 게이트 대상이 아니다.

---

## 5. E — 전체 스위트

```
$ unity test client --mode EditMode --report-format nunit,junit \
    --output <qa-r6>/EditMode.nunit.xml --junit-output <qa-r6>/EditMode.xml
exit 0, testcasecount=237 total=237 passed=235 failed=0 inconclusive=0 skipped=2
start-time 2026-09-23 23:04:39Z
```

**리더 보고(237/235/failed=0/skipped=2)와 일치한다.**

**§7a 단언 — 이 초록이 무엇을 보고 켜졌는가:**

- **건너뛴 2건의 정체를 확인했다**: `LiveServerTests.Live_ThreePings_RoundTripInOrder`, `LiveServerTests.Live_ServerInitiatedClose_ReconnectsAsANewSession` — 실서버 게이트. 숨은 실패가 아니다.
- **223 → 237의 +14가 R13 주장과 정확히 맞는다**: `HudTextWrapTests` **6**(신규) + `MarkerHudLineTests` **5**(신규, R13 "5건") + `PeriodicStatusLogTests` 5 → **8**(+3, R13 "3건") = **14**. 전부 `Passed`.
- **`SnapshotRebaseBatchTests`는 6건 그대로**(F-5는 테스트를 늘린 게 아니라 1건을 고친 것) — R13 기술과 일치.
- 그리고 **이 237이 F-2에 대해서는 0건짜리 검출기**라는 것을 §2.2에서 실측으로 보였다. 건수는 경로가 실행됐다는 증거가 아니다.

---

## 6. 결함·수정 요청 목록

### 6.1 client

| # | 위치 | 문제 | 기대 |
|---|---|---|---|
| **F-9** | `PeriodicStatusLog.cs` 필드 블록 축 규약 주석 | **"positive roll … the ship's right wing drops"가 틀렸다 — 실제로는 올라간다.** `ShipIntegrator.cs:147-149` + `Quatd` Hamilton으로 수치 확인(§2.5(1)). F-8이 제3자에게 방향을 열어 주려고 붙인 유일한 물리 문장이 거꾸로다 | *"rotates ship-+X toward ship-+Y — the right wing RISES"*로 정정. 복제본 여부 확인 |
| **F-11** | `PeriodicStatusLog.cs` / `GreyboxSession.cs:681-698` | **`flight_assist`·`brake`가 없다.** 오토레벨은 `FlightAssist`일 때만 돈다(`ShipIntegrator.cs:122`) → **SC-59 기준 5-b가 F-8 이후에도 로그로 수집 불가**(§2.5(3)) | 두 bool 추가. HUD는 이미 찍고 있다 |
| **F-10** | `PeriodicStatusLog.cs` 필드 블록 | 회전 적용 형태(`v'=q·v·conj(q)`, Hamilton)와 **`attitude_*`가 월드 프레임**이라는 사실이 없어, 원소 부호를 그대로 좌/우로 읽으면 틀린다(§2.5(2)) | 두 줄 추가 ＋ 판독 예제(우선회 30° → `attitude_y_micro ≈ +258819`) |
| **F-2′** | `ObserverSession.cs:277-284` | F-2 수정에 **테스트가 0건**이다. `ApplyPendingRebase()`를 통째로 지워도 스위트가 237/235/failed=0으로 초록(§2.2 실측). 순서 의존이 이 수정의 전부인데 아무것도 그것을 지키지 않는다 | tick 100·102·101 한 배치 → **`WriteRow` 1회, tick 102** 단언. `GreyboxSession` H-9 배선도 같은 공백 |
| F-12 | `GreyboxSession.cs:681-697` | `_input != null ? … : 0` — 미측정 0이 측정된 0으로 찍힌다. 같은 파일이 `send_burst_*`에는 `n/a`로 정확히 그 구분을 한다. `attitude_w=0`은 정규 쿼터니언에 없는 값이라 침묵 센티널이 된다 | 낮은 우선순위. `n/a` 또는 별도 `input_sampler=absent` |
| L-8 | `HudTextWrap.cs` 헤더 / `GreyboxSession.cs:35-40` 주석 | *"필드가 늘어나면 행이 늘지 조용히 잘리지 않는다"*는 **공백 없는 110자 초과 토큰 하나면 깨진다**(그 토큰은 자기 행에 통째로 남아 900 px에서 잘린다). 현재 최장 토큰 46자라 실위험은 없다 | 문장을 *"모든 필드가 공백으로 구분되는 한"*으로 좁힌다 |

### 6.2 리더 (문서)

| # | 위치 | 문제 | 기대 |
|---|---|---|---|
| **L-9** | `03_client_impl.md` R13 `판정 — runInBackground` | *"`runInBackground = true`로 바꾸면 SC-89가 재려던 현상이 사라진다"*가 **틀렸다.** 바꿔도 버스트는 사라지지 않고 히치 프로파일만 바뀌며, 계약 자신이 그 프로파일을 좋은 재현 벡터로 적고 있다(§3.1). **결론은 옳고 근거가 틀렸다** | 정정 레코드로 근거를 교체(본문은 절대 원칙 5로 보존). ＋ H-13이 **계약이 준 역할(백그라운드 구간 증거)을 충족하지 않기로 했다**는 사실을 명시 |
| **L-2′** | `evidence/R4-B6/sc59/09-reshoot-checklist.md` | R13 이전 상태 — HUD 420 px, `GreyboxSession.cs:512`, 마커 줄 없음, `periodic_status` 수집 단계 없음, 기준표가 계약 절 번호가 아님. **qa4 L-2가 요구한 실행 항목이 여기 있어야 하는데 없다**(§4.1) | R13 트리 기준으로 갱신, 기준표를 (a)(b1)(b2)(c)(d)로 |
| **L-10** | 새 레코드 `sc59-v2/13-…` ＋ `03_client_impl.md` R13 "남은 것" | §6.4는 F-8 이후에도 **절반만** 실현 가능 — 기준 2·4 가능, **5-b 불가**(F-11)(§4.2) | 새 정정 레코드에 표를 남기고, R13 "남은 것"에 한 줄 |
| L-7 | `03_client_impl.md` R13 md5 4건 | 그중 2건이 **트리의 어떤 파일과도 일치하지 않는다**(§1). 선의의 시차로 보이나 제3자가 원복을 검증할 수 없다 | 해시 옆에 "작업 시점의 값이며 이후 F-7/판정 작업이 같은 파일을 다시 수정" 명시. 해시는 고치지 않는다 |
| L-11 | `03_client_impl.md` R13 F-1 | "RED 1건"은 **실제 2건**이다(§2.1). 과소 보고라 위험하지 않으나 수가 틀렸다 | 2건으로 정정 |

### 6.3 architect

- **계약 SC-59 (a)에 롤 방향이 없다.** (a)는 전방 추력·마우스 선회·위 추력만 열거하고 **Q·E의 뱅크 방향을 요구하지 않는다.** 그런데 §2.5(1)의 실측상 `E → 양의 롤 → 우현 날개 상승 = 좌측 뱅크`이고, qa4 §2.1이 이미 *"롤 축은 증언·프레임 어느 쪽으로도 닫히지 않았다"*를 기록했다. **이 슬라이스에서 가장 덜 검증된 축이다**(§2.6). (d) 클립에 롤 방향 관찰을 명시할지 판단이 필요하다.
- **(이월) §0.3에 "제품 책임자 판정" 칸이 없다** — qa4 §2.5. 이번 라운드에 변화 없음.

---

## 7. 실행 증거 색인

| 산출물 | 경로 |
|---|---|
| EditMode GREEN (판정용) | `_workspace/p1-01-ship-movement/unity-tests-qa-r6/EditMode.nunit.xml`, `EditMode.xml` |
| RED F-1 (랩 제거) | `unity-tests-qa-r6/EditMode.RED-F1.nunit.xml` — failed=2 |
| RED F-5 (always-true) | `unity-tests-qa-r6/EditMode.RED-F5.nunit.xml` — failed=4 |
| RED F-5 (제3 퇴화 `>=`) | `unity-tests-qa-r6/EditMode.RED-F5-gte.nunit.xml` — failed=2 |
| RED F-7 (`shipPositionM` 무시) | `unity-tests-qa-r6/EditMode.RED-F7.nunit.xml` — failed=1 |
| RED F-8 (`thrust_y`↔`thrust_z`) | `unity-tests-qa-r6/EditMode.RED-F8.nunit.xml` — failed=2 |
| **F-2 영검출 증명** (수정 무력화) | `unity-tests-qa-r6/EditMode.RED-F2-null.nunit.xml` — **failed=0, 초록과 동일** |
| GREEN 복원 확인 | `unity-tests-qa-r6/EditMode.GREEN-after-restore.nunit.xml` — 237/235/0/2 |
| 역방향 게이트 selftest | §4.3 인라인 |

**§0.12 준수:** 주입 5회 전부 백업/복원, 표식 `QA-R6 TEMP DEFECT` 잔존 **0건**, 5개 파일 md5 기준선 일치. `docker compose down -v` 미사용, 서버 하드 킬 없음, golden 파일 미변경, 커밋 없음.

---

## 8. 라운드 6 결론

- **PASS 1건**(SC-46) — 실행 증거 있음.
- **통과 1건**(SC-59) — 제품 책임자 판정, qa 판정 아님. 기록 검증만 수행.
- **미검증(증거 요건) 2건**(SC-56·SC-89) — 비-통과. **슬라이스 종료를 막는다.**
- **R13 5건 중: PASS 3(F-1·F-5·F-7) / 부분 PASS 1(F-8) / 미검증 1(F-2).**
- **신규 결함 6건**(F-9·F-10·F-11·F-2′·F-12·L-8) — 전부 client.
- **리더 수정 요청 5건**(L-2′·L-7·L-9·L-10·L-11), **architect 사안 1건**(SC-59(a)에 롤 방향 없음).

**리더가 틀린 곳 셋을 명시한다.**

1. **F-8의 축 규약 주석이 틀렸다** — 양의 롤은 오른쪽 날개를 **올린다**. 제3자 판독을 가능하게 하려고 붙인 단 하나의 물리 문장이 거꾸로다. F-8의 목적에 직접 반한다.
2. **F-2를 검증했다는 진술이 성립하지 않는다** — 수정을 통째로 지워도 스위트가 같은 값을 낸다. "컴파일·스위트 초록"은 이 항목에 대해 관측이 아니다.
3. **`runInBackground` 논거가 인과가 거꾸로다** — 설정을 바꿔도 버스트는 사라지지 않는다. 결론은 유지할 만하지만 그 이유로는 아니다. 그리고 **1002 (iii)은 대체 증거로 닫히지 않는다.**

**리더가 옳았던 곳도 명시한다.** RED 네 건의 실패 건수는 전부 사실이었고(F-1은 오히려 과소 보고), **F-1의 110/900 짝은 내가 프레임에서 직접 재 보니 실제로 정합했다**(6.6 px/자 × 110 = 726 px ≤ 900 px, 옛 잘림 위치로 역검산까지 일치). `SnapshotRebaseBatch` 4케이스 표는 리더가 묻지 않은 제3 퇴화까지 잡는다. `PeriodicStatusLog.cs` 헤더의 순환 논증은 실제로 제거됐다. **그리고 `OnSessionEnded` 덤프가 백그라운드 간극의 버스트를 담는다는 판단은 옳다 — 복귀 프레임이 포커스 상태이기 때문이며, 그 이유를 리더는 적지 않았다.**

**하나만 고른다면 F-11이다.** F-8은 "다음 촬영을 사람 증언 없이 닫는다"를 위해 만들어졌는데, `flight_assist` 한 필드가 없어서 **SC-59 (d)의 절반이 여전히 로그로 닫히지 않는다.** F-9(주석 정정)와 F-11(필드 둘)은 합쳐 20줄이 안 되고, 그 둘을 고치면 §6.4의 후속 계획이 **처음으로 전부 참**이 된다.

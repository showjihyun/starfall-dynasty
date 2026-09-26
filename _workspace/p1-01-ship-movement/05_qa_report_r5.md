# p1-01 함선 이동 — 평가 리포트 라운드 5 (qa)

- 평가자: qa (독립 판정)
- 일시: 2026-09-23 23:44 ~ 00:05 KST
- 대상: 리더 재판정 요청 A~G
- 도구: Unity 6 CLI(EditMode), ffmpeg 7.1.1(프레임 확대), python 3.12
- 규율: 계약 §0.3(증거 기준) · §0.12 · §7a(초록불이 무엇을 보고 켜졌는가) · §7b(자명 통과 시험)

> **§0.3 요구 문장:** 이 라운드에서 FAIL 수는 0이다. **그것은 제품이 나아져서가 아니라 귀속이 정확해졌기 때문이다 — 막고 있는 게이트 수는 줄지 않았다.** `미검증(증거 요건)` 2건(SC-56·SC-89)은 비-통과이며 슬라이스 종료를 FAIL과 똑같이 막는다.
>
> **SC-59는 qa 판정이 아니다.** 제품 책임자(사용자)가 통과로 판정했고, qa는 그것을 뒤집지 않는다. qa가 한 일은 **그 판정이 기록에 정직하게 남았는지**를 검증한 것이다(§2). **그리고 SC-59의 통과는 SC-89의 증거 기준을 낮추지 않는다 — 두 항목은 독립이고, §6.3에서 SC-89로 내려가는 근거는 이 리포트에 하나도 쓰지 않았다.**

---

## 0. 판정 요약

| SC | 판정 | 한 줄 근거 |
|---|---|---|
| SC-46 | **PASS** | `unity test … --report-format nunit,junit` exit 0, `total=223 passed=221 failed=0 skipped=2`, 리포트 2개 생성 |
| SC-47 | **PASS** | `ContractFixtureTests.Fixtures_RoundTrip_Found27_RoundTripped21` Passed — 27/21이 테스트 이름에 박혀 있다 |
| SC-49 | **PASS** | (c) `FixtureLoader_FailsWhenTooFewValidFixtures` (d) `WORLD_SNAPSHOT/empty-nulls-and-bounds` + `two-ships-one-lingering` 왕복 (e) `Runtime_ToleratesUnknownFieldInsideShipStateArrayElement_AndStrictThrows` |
| SC-56 | **미검증(증거 요건)** | H-10′ 코드는 정확하나 (d)②③이 **HUD에서 구조적으로 읽히지 않는다**(F-1). 이번 라운드에 60초 실서버 세션 신규 촬영 없음 |
| SC-59 | **통과 — 제품 책임자 판정(증언 3건 + 프레임 판독 3건)** | qa 판정 아님. 근거의 성격과 §6 기록 검증은 §2 |
| SC-89 | **미검증(증거 요건)** | (b)의 자명 통과 지적 **미해소**. 게다가 판별 단언 값이 HUD에 뜨지 않는다(F-1) |

계약 외 태스크(SC 번호 없음):

| 항목 | 판정 | 근거 |
|---|---|---|
| T-5 불변식 ③ 동어반복 제거 | **PASS (RED 재현 완료)** | §3 |
| H-9 수신측 스냅샷 배치 | **PASS (조건부)** | §4 — 단 F-2 신규 결함 |
| H-13 주기 로그 | **불충분** | §5 — 핵심 전제가 미검증 |
| G 역방향 게이트 | **PASS** | §7 |

---

## 1. 리더의 전사는 정확했다 — 먼저 그것부터

지난 라운드(`sc59/08-correction.md`)의 허위 기술이 반복됐는지가 이 판정의 출발점이다. **반복되지 않았다.** `§5.2` 표의 프레임 파일을 직접 열어 대조한 결과 **전부 일치**했다. 확대해서 읽었다(원본 430px 폭은 육안 판독에 부족해 2.5~4배 최근접/란초스 확대).

| 대조 항목 | 리더의 기술 | 내가 연 것 | 결과 |
|---|---|---|---|
| `thrust=(0,0,1000)` 3프레임 **연속** | 행 19·20·21 | `frames/thrust-150-210.png` crop(0,0,430,530)×2.5 | **일치** — (0,0,0),(0,0,0),**(0,0,1000)×3**,(0,0,0) |
| `origin_distance_m` 2512.5→2342.5→1970.7→1680.0 | t=165~180s | `frames/origin-150-290.png` ×2.5 | **일치** (이어서 1461.9·1461.9·1411.8·1266.5·1394.4·2400.6) |
| 최대 3697.1 | 전 구간 | 같은 파일 하단 4행 + `origin-250-700.png` | **일치** |
| `thrust=(0,1000,0)`, `speed_mps=17.1`→`140.0` | `scan/grid3.png` 우상·좌하 | `scan/grid3.png` 원본 | **일치** (우상 tick=511524 speed=17.1 / 좌하 tick=511724 speed=140.0, 둘 다 thrust=(0,1000,0)) |
| `roll=-1000` | `frames/h270.png` | 원본 | **일치** |
| `aim_target` 고정 34프레임 | `frames/aim-160-200.png` | 원본 | **일치** — 전 행 `(110091,-810004,161267,552962)` |

**§5.4의 네 가지 "확정되지 않은 것"도 전부 옳다.** (이후 SC-59는 제품 책임자 판정으로 통과가 됐다 — §2. 그 판정은 이 표의 대조 결과를 바꾸지 않고, 이 표도 그 판정을 바꾸지 않는다.)

---

## 2. SC-59 — 제품 책임자 판정의 **기록 검증** (요청 A, 개정)

**판정 자체는 다시 하지 않는다.** 사용자가 통과로 판정했고 그것은 qa가 뒤집을 사안이 아니다. 이 절은 리더가 요청한 세 가지 — ① §6.2 Fact/Claim 분류가 증거 상태와 맞는가 ② §5.4가 무력화됐는가 ③ 처음 읽는 사람이 오해 없이 읽는가 — 에 답한다.

### 2.0 먼저, 이 통과는 계약의 설계와 어긋나지 않는다

이것부터 적는 이유는 공정하기 위해서다. 계약 §0.10은 SC-59에 대해 이렇게 적었다:

> **검출기는 SC-59(AC-14 a·d)의 육안 관찰 하나뿐이다.** 그것을 **사람이 실제로 볼 때까지** 이 슬라이스는 부호에 대해 아무것도 증명하지 못한다.

**SC-59는 처음부터 사람 눈으로 닫히도록 설계된 항목이다.** mp4는 검출기가 아니라 그 관측의 **보존 수단**이다. 제품 책임자가 조종해서 보고 "잘 되더라"고 판정한 것은 **계약이 지정한 검출기가 실제로 돌았다**는 뜻이다. 그러니 이 통과는 규율의 예외가 아니라 그 항목의 원래 닫는 방식이고, 부족한 것은 **보존**이다. §6.2가 그 구분을 표로 남긴 것은 옳은 형태다.

그리고 **(b1)은 증언이 아니라 실행 증거로 닫혀 있다.** 오늘 내가 돌린 EditMode 리포트에 있다:

```
Passed ReferenceMarkerPlacementTests.Cradle_HasExactlyFourReferenceMarkers
Passed ReferenceMarkerPlacementTests.Cradle_MarkerIsAtItsDesignatedCoordinate("star-cradle",0,0,0)
Passed …("planet-vela",3000,-200,-900) / …("beacon-meridian",-1800,900,2200) / …("derelict-unnamed",700,-1400,-3900)
Passed ReferenceMarkerPlacementTests.Cradle_NoTwoMarkersShareAnId
```

계약이 *"(b1)은 EditMode 씬·데이터 단언으로 닫는다 — 영상이 아니다"*라고 지정한 그대로다. **§6이 (b1)을 언급하지 않았으나 (b1)은 실제로 닫혀 있다** — 이것은 §6에 **유리한** 누락이므로 §2.3에서 보완을 권한다.

### 2.1 §6.2 분류표 — **Fact 칸 3행이 전부 Claim이다** (요청 A-①의 답)

리더의 질문은 "내가 Claim을 Fact 칸에 잘못 넣은 게 있는가"였다. **있다. 세 행 전부다.** 다만 **기준이 틀렸다는 뜻이 아니라, 프레임이 뒷받침하는 범위보다 행의 문장이 넓다**는 뜻이다.

근거는 HUD 필드의 출처다. `GreyboxSession.cs:864-871`:

```csharp
// Input state, verbatim quantised integers - the sign-bug detector (AC-14) needs
// to see exactly what was sent, not an interpretation of it.
_input != null
    ? "thrust=(" + Quantization.QuantizeControlAxis(_input.Thrust.X) + "," + …
      … + ") roll=" + Quantization.QuantizeControlAxis(_input.Roll) + …
```

`thrust=`와 `roll=`은 **클라이언트가 자기 입력 구조체에 넣은 값**이다. 그리고 짝지은 두 수는 **방향이 없는 스칼라**다 — `speed_mps` = `Velocity.**Length()**`(:812), `origin_distance_m` = `Position.**Length()**`(:813), 둘 다 **서버 상태가 아니라 `_controller`의 클라이언트 예측 상태**에서 읽는다.

| §6.2 행 | 현재 칸 | 프레임이 실제로 뒷받침하는 것 | 프레임이 뒷받침하지 **않는** 것 | 옳은 칸 |
|---|---|---|---|---|
| 1. W = 뱃머리(+Z) | Fact | W 입력이 `thrust=(0,0,1000)`으로 샘플링됐고 그 구간에 함선이 **움직였다**(2512.5→1680.0) | **그 이동 방향이 뱃머리였다** | **Fact(축소) + Claim(방향)** |
| 3. R = 위(+Y) | Fact | R 입력이 `thrust=(0,1000,0)`으로 샘플링됐고 **가속했다**(17.1→140.0) | **그 가속 방향이 위였다.** 그리고 §1 표가 스스로 건 조건 *"수평 상태에서 관측"*이 충족됐다는 관측도 없다(HUD에 자세 필드가 없다) | **Fact(축소) + Claim(방향)** |
| 5-a. Q·E 롤 입력 **서버 반영** | Fact | Q·E가 `roll=-1000`으로 **샘플링됐다** | **서버에 반영됐다.** `h270.png`는 `_input.Roll`을 찍을 뿐이고, 같은 프레임의 `ack_input_seq=5358`은 **더 이전** 입력의 ack다. 게다가 그 프레임의 `speed_mps=0.0` — **함선은 정지 상태였다** | **Fact(축소) + Claim(반영)** |

**왜 이것이 중요한가:** SC-59는 이 슬라이스의 **유일한 부호 버그 검출기**다(§0.10). 그런데 **부호가 뒤집힌 빌드가 이 여섯 프레임과 글자 하나까지 같은 화면을 낸다** — W로 뱃머리 반대로 날아가도 `thrust=(0,0,1000)`이고 속도는 0→140이며 원점 거리는 (원점이 뒤에 있으면 더 잘) 준다. **즉 §6.2의 Fact 칸은 "부호가 옳다"를 한 건도 담고 있지 않다.** 사용자의 눈이 담고 있고, 그래서 여섯 행이 **전부** 사용자 증언에 기대고 있다.

**수정 요청 (L-1′):** §6.2 표의 세 Fact 행을 두 줄로 쪼갠다 — "입력 샘플링/이동 발생 = Fact(프레임)" 과 "방향·서버 반영 = Claim(증언)". 같은 오류가 §5.3("확정된 것")에도 있으므로, §5.3은 **절대 원칙 5에 따라 고치지 말고** §6에 정정 레코드를 얹는다.

### 2.2 §5.4는 무력화되지 않았다 — 그러나 §5.3이 §6.2로 전파됐다 (요청 A-②의 답)

§5.4는 **본문 그대로 살아 있다**(라인 89~102 확인). §6.2 하단이 *"§5.4에 적은 목록은 취소되지 않는다"*고 명시했고 그 문장은 정확하다. **절대 원칙 5 준수 — 문제 없음.**

다만 전파가 하나 있다. §5.3("판독으로 확정된 것 — 기준 1·3 확정")의 과대 진술이 §6.2의 Fact 칸으로 **그대로 옮겨졌고**, 이제 같은 오류가 두 곳에 서로를 뒷받침하는 모양으로 있다. §5.4를 읽지 않고 §5.3 → §6.2만 따라간 독자는 "프레임이 부호를 확정했다"고 읽는다. §2.1의 수정 요청이 이 고리를 끊는다.

부수 하나: §6.2는 기준 2를 통째로 Claim으로 옮겼는데, §5.4(2)는 *"'변했다'까지는 확정"*이라고 더 정확하게 적어 두었다. **§6.2가 §5.4보다 거칠다** — 승격이 아니라 강등 방향이라 위험하지는 않으나, 표가 문서의 최종 요약으로 읽히므로 맞춰 두는 편이 낫다.

### 2.3 처음 읽는 사람이 오해하는가 — **두 곳에서 오해한다** (요청 A-③의 답)

구조는 좋다. §6.1/§6.2/§6.3/§6.4 분리, 판정자 명시("리더 판정도, qa 판정도 아니다"), 사용자 원문 인용, §6.3이 계측 결함임을 자백한 것 — **정직한 기록의 형태를 갖췄다.** 오해는 두 군데다.

1. **§6.1 "SC-59 통과"에 절 구분이 없다.** SC-59는 (a)(b1)(b2)(c)(d) 다섯 절이고, **(b1)은 플레이 세션과 무관한 EditMode 단언**이다. 지금 문장은 "다섯 절 전부"로 읽히는데, §6의 다섯 기준에는 (b1)(b2)가 아예 없다. **역설적으로 (b1)은 실제로 닫혀 있으므로**(§2.0의 7건 테스트) 절을 명시하면 기록이 **더 강해진다.** 권고: §6.1을 *"(a)(c)(d)는 제품 책임자 육안 판정으로, (b1)은 `ReferenceMarkerPlacementTests` 7건으로 닫힌다. (b2)는 보존 수단이 아직 없다"*로.

2. **§6.4의 후속 계획이 실현 불가능하다.** *"다음 세션 B에서 H-13 로그로 기준 2·4·5-b를 추가 촬영 없이 수집한다"* — **H-13 로그는 그 세 가지를 담을 수 없다.** `PeriodicStatusLog.cs:39-55`의 필드 전부는 `Tick, AckInputSeq, SpeedMps, OriginDistanceM, PredictErrorM, PredictErrorDeg, ReconcileHardSnapTotal, SendBurstMax…, Catchup…, ReconcileForcedAfterHitchTotal, VisibleShips, ApplicationFocused`다. **자세(orientation)도, 진행 방향도, `thrust`도, `roll`도, 경계 플래그도 없다.**

   | 기준 | H-13 로그로 가능한가 |
   |---|---|
   | 2. 선회 방향 | **불가** — 자세 필드가 없다 |
   | 4. 경계 | **부분 가능** — `origin_distance_m`가 soft 반경을 넘는 줄이 남으면 "도달"은 보인다. "미끄러짐·복귀"는 거리 수열로 간접 추정일 뿐 |
   | 5-b. 오토레벨 복귀 / 억제 | **불가** — 자세 필드가 없다 |

   **§6.4를 그대로 두면 "이미 해결된 후속"으로 읽히고 실제로는 아무것도 수집되지 않는다.** 권고: §6.4를 *"기준 4는 H-13 로그로 수집 가능. 기준 2·5-b는 `PeriodicStatusLog`에 자세 필드(쿼터니언 또는 오일러 3축)와 `thrust`·`roll`을 추가해야 수집 가능하며, 그 전까지는 Claim으로 남는다"*로 고친다. 이것은 client 작업 항목이다(F-8).

### 2.4 §5.2 프레임 판독값 대조 — 요청 A-3, **전부 일치** (6건 직접 열람)

통과 판정과 무관하게 수행했다. 원본 430 px 폭은 육안 판독에 부족해 ffmpeg로 2.5~4배 확대해 읽었다(최근접/란초스). **틀린 값은 없다.**

| 대조 항목 | 리더의 기술 | 내가 연 것 | 결과 |
|---|---|---|---|
| `thrust=(0,0,1000)` 3프레임 **연속** | 행 19·20·21 | `frames/thrust-150-210.png` crop(0,0,430,530)×2.5 | **일치** — (0,0,0),(0,0,0),**(0,0,1000)×3**,(0,0,0) |
| `origin_distance_m` 2512.5→2342.5→1970.7→1680.0 | t=165~180s | `frames/origin-150-290.png` ×2.5 | **일치** (이어서 1461.9·1461.9·1411.8·1266.5·1394.4·2400.6) |
| 최대 3697.1 | 전 구간 | 같은 파일 하단 4행 + `frames/origin-250-700.png` | **일치** |
| `thrust=(0,1000,0)`, `speed_mps=17.1`→`140.0` | `scan/grid3.png` 우상·좌하 | `scan/grid3.png` 원본 | **일치** (우상 tick=511524 speed=17.1 / 좌하 tick=511724 speed=140.0, 둘 다 `thrust=(0,1000,0)`) |
| `roll=-1000` | `frames/h270.png` | 원본 | **일치** (＋미기재 사실: 같은 프레임 `speed_mps=0.0`, 정지 상태) |
| `aim_target` 34프레임 고정 | `frames/aim-160-200.png` | 원본 | **일치** — 전 행 `(110091,-810004,161267,552962)` |

**지난 라운드의 허위 기술은 반복되지 않았다.** 처음 축소 이미지로 읽었을 때 `(0,0,1000)`이 2행 비연속으로 보였으나, 원본 해상도로 확대하니 **3행 연속이 맞았다** — 리더의 기술이 옳고 내 첫 판독이 틀렸다.

### 2.5 계약에 이 판정의 집이 없다 (architect 사안)

계약 §0.3의 판정 칸은 PASS / FAIL / 미검증(환경) / 미검증(증거 요건) / 대기 / 기록 **여섯 개**뿐이다. **"제품 책임자 판정"은 그중 어디에도 해당하지 않는다** — PASS의 조건(*"명령과 출력 요약, 테스트 이름, 또는 파일:라인이 리포트에 있다. 정적 읽기만으로는 PASS가 아니다"*)을 증언은 만족하지 못하고, 그렇다고 비-통과도 아니다.

그리고 계약 §1의 항목 표에는 **판정/상태 칸 자체가 없다**(열: ID / 검증 항목 / 검증 방법 / 담당 / 근거 AC / 불가). 그래서 리더가 지시한 "계약서 SC-59 행 상태 갱신"은 **열을 신설하지 않으면 수행할 수 없고**, 그것은 qa가 단독으로 할 구조 변경이 아니다. **따라서 판정은 이 리포트에 기록했고**(§0 표, 근거란 문구 그대로), **계약 변경은 architect에게 올린다** — 다음 둘 중 하나가 필요하다:

- §0.3에 **일곱째 칸 신설**: *"제품 책임자 판정 — 계약이 지정한 검출기가 사람 눈인 항목에 한해, 제품 책임자가 직접 관측하고 판정한 것. 보존 증거의 부재를 별도 레코드로 남긴다."* (SC-59처럼 §0.10이 "사람이 볼 때까지"라고 적은 항목에만 적용되도록 범위를 묶어야 한다 — 묶지 않으면 **모든 항목의 탈출구**가 된다)
- 또는 SC-59를 **하드 게이트에서 빼고 §0.4의 "기록"으로 옮긴다.**

**⚠ §7b(1) 자명 통과 시험을 이 새 칸 자신에 돌린 결과:** 범위를 묶지 않으면 자명 통과 상태가 *"어떤 항목이든 제품 책임자가 '되던데'라고 말하면 통과"*이고, **그 상태는 실제로 가능하다.** 그러므로 칸을 만든다면 **§0.10이 검출기를 사람 눈으로 지정한 항목**으로 적용 대상을 명시적으로 제한해야 한다. 현재 그 조건을 만족하는 항목은 SC-59 하나뿐이다.
## 3. T-5 불변식 ③ — **PASS**, RED 재현 완료 (요청 C)

### 3.1 재현 절차 (§0.12 준수 — 갱신 경로 아님, 백업/복원)

```
cp TickCatchUp.cs → scratchpad/TickCatchUp.orig.cs      (md5 e1fb231c21a4b8f70d0f698d4461ce47)
sed: int ticksToSend = Math.Min(backlogTicks, maxSendsPerFrame);
  →  int ticksToSend = Math.Min(backlogTicks, maxSendsPerFrame) + 1;   (TickCatchUp.cs:144)
unity test client --mode EditMode --report-format nunit --output …/EditMode.RED.nunit.xml
cp scratchpad/TickCatchUp.orig.cs → TickCatchUp.cs      (md5 재확인 e1fb231c… 일치, "QA-R5 TEMP DEFECT" grep 0건)
```

### 3.2 RED 결과 — `unity-tests-qa-r5/EditMode.RED.nunit.xml`

`total=223 passed=210 failed=11 skipped=2`, unity exit=8. 요청한 2/2가 실패했다:

```
FAILED GreyboxSendBurstTests.PropertyTest_RandomFrameDeltasIncludingA460msHitch_HoldsAllThreeInvariants
  invariant 3: carry-forward ticks + ACTUALLY SENT ticks == predicted ticks
  Expected: 1   But was: 0
FAILED GreyboxSendBurstTests.Update_AfterA460msHitch_MustNotBurstMoreThanOneTickOfSendsPerFrame
  … K=1 … Expected: less than or equal to 1   But was: 2
```

**겨냥한 단언이 실제로 불을 켰다** — 성질 테스트 쪽은 다른 줄이 아니라 **불변식 ③ 자신**이 실패했다. 동반 실패 9건은 `TickCatchUpTests` 쪽이며 같은 결함을 본 것이다.

### 3.3 GREEN 복원 확인

복원 후 재실행 `EditMode.GREEN-after-restore.nunit.xml`: `total=223 passed=221 failed=0 skipped=2`, start-time `2026-09-23 14:50:15Z`. **트리는 초록으로 되돌려 두었다.**

### 3.4 §7b(1) 자명 통과 시험 — 직접 따짐

새 단언은 `carryForwardTicksThisFrame + sendsThisFrame == plan.TicksToPredict`이고, `carryForwardTicksThisFrame = plan.TicksToPredict - plan.TicksToSend`이므로 **대수적으로 `sendsThisFrame == plan.TicksToSend`로 환원된다.** 즉 이것은 "와이어 실측 == 계획값"이고, 이전의 `TicksToPredict == TicksToPredict`와 달리 **한쪽이 Plan 밖(전송 관측)에서 온다.** 자명 통과 상태는 "아무것도 보내지 않음(둘 다 0)"인데, 같은 루프의 `totalSends > 0`(성질 테스트)과 `sendsThisFrame == 1`(단일 프레임 테스트)이 그것을 막는다. **동어반복은 제거됐다.**

남는 약점 하나(기록): `DriveThroughTransport`가 `plan`을 읽어 송신 슬롯을 정하므로 **"Plan을 어떻게 집행하는가"의 버그는 이 파일이 잡지 못한다** — 다만 그 집행 루프는 `GreyboxSession.Update():615-621`와 문자 그대로 같은 형태이고, 파일 헤더가 그 한계를 명시하고 있다. 계약 외 사항이므로 판정하지 않는다.

---

## 4. H-9 수신측 스냅샷 배치 (요청 D)

### (a) 오래된 스냅샷이 보간 버퍼에 전부 들어가는가 — **예, 확인**

`GreyboxSession.cs:443`의 `_remoteRegistry.OnSnapshot(message.Tick, others)`는 `OnWorldSnapshotCore` 진입부터 그 줄까지 **조기 반환이 하나도 없는** 무조건 경로다(첫 조기 반환은 :455 `if (controlledWire == null) return;`으로 **443보다 뒤**다). 배치 억제는 :486 이후 자기 함선 rebase 경로에만 걸린다. **설계 주석대로 동작한다.**

### (b) 6건 테스트가 "항상 true" 퇴화를 잡는가 — **잡는다**

`AlwaysTrue` 구현은 `ShouldReplacePending_CandidateOlderThanPending_False`와 `…_CandidateEqualsPending_False` 두 건을 깬다. `AlwaysFalse`는 `…_NoPendingYet_AlwaysTrue`와 `…_CandidateNewerThanPending_True`를 깬다. **어느 퇴화도 통과하지 못한다.**

**단, 6건 중 1건은 아무것도 재지 않는다(F-5).** `ShouldReplacePending_DegenerateAlwaysTrueOrAlwaysFalse_WouldFailAtLeastOneCase`는

```csharp
Assert.That(AlwaysTrue(99, 100) && AlwaysFalse(101, 100), Is.False, …);
```

인데 이는 `true && false == false`로 **상수 접힘이다. 피시험 대상(`SnapshotRebaseBatch`)을 한 번도 호출하지 않는다.** 구현을 어떻게 망가뜨려도 이 테스트는 초록이다. 주석이 *"RED-check reasoning… executable, not just asserted in prose"*라고 주장하지만 **실행되는 것은 추론이 아니라 두 리터럴이다.** §7b(1) 기준으로 자명 통과 상태이고, 남겨 두면 "퇴화 방어가 테스트로 덮여 있다"는 잘못된 안도를 준다. 삭제하거나, 두 퇴화 대리 구현을 **실제 네 케이스에 돌려 각각 한 건 이상 어긋남**을 단언하도록 고칠 것을 권한다.

### (c) ObserverSession이 같은 결함에 노출되는가 — **예, 실재한다 (F-2, 신규 결함)**

`client/Assets/_Project/Scripts/Greybox/ObserverSession.cs:174`:

```
_controller.Reconcile(confirmed, payload.AckInputSeq, _data.Tuning ?? DefaultTuning());
```

이 줄은 `OnWorldSnapshot` **안에서 인라인으로** 호출된다. `GreyboxSession`이 H-9로 고친 바로 그 형태다 — 한 `Pump()` 드레인에 자기 함선 스냅샷이 둘 이상 들어오면 **오래된 쪽의 `Reconcile`이 새 쪽 뒤에 실행되어 `CurrentState`를 되감는다.** ObserverSession은 `:279`에서 `_controller.ApplyInput`도 하므로 예측 상태를 실제로 들고 있고, `:181`에서 그 `CurrentState`를 **CSV 증거 행으로 기록한다**(`WriteRow`). 즉 되감김이 **QA 증거 파일에 그대로 새겨진다.**

**신규 결함 항목으로 올린다.** 담당: client. 재현: 관측자 세션에서 프레임률이 `snapshot_interval_ticks`(=2, 10 Hz)보다 느려지는 구간. 기대 동작: `GreyboxSession`과 동일하게 `SnapshotRebaseBatch.ShouldReplacePending`으로 지연 적용. 계약 §0.11이 관측자 B를 봇으로 대체하지 못하게 한 이상, 이 경로의 CSV는 SC-64/65 계열의 증거원이므로 방치할 수 없다.

---

## 5. H-13 주기 로그 (요청 E) — **코드 경로 증명이 불충분하다**

### 5.1 client가 주장한 두 가지는 사실이다

- `PeriodicStatusLog.cs`에 `UnityEngine.GUI`/`EditorWindow` 참조 **0건**, `Application.isFocused`는 **필드에 기록만** 하고 분기에 쓰이지 않는다(`Format()`은 값을 문자열로 찍기만 한다).
- 호출은 `GreyboxSession.Update():630`의 `MaybeLogPeriodicStatus()` — `Update()` 말미, **조건 분기 없이** 무조건. `OnGUI`와 무관하다. 게이트는 `Time.unscaledTime` 기반 5초 간격(`:639-646`).

### 5.2 그러나 결론이 따라 나오지 않는다

"비포커스에서도 로그가 남는다"의 **적재 전제는 세 번째 명제**다:

> **③ Editor가 비포커스일 때 `Update()`가 계속 호출된다.**

①(OnGUI 비의존)과 ②(isFocused 비분기)는 ③이 참일 때만 결론을 낸다. **③은 이 프로젝트에서 미검증이다.** 그리고 계약이 그 사실을 이미 적어 두었다 (02_sprint_contract.md:185, SC-89 방법 칸):

> **⚠ 그러나 그 산수는 Editor 가 throttle 할 때만 성립한다 — pause 하면 복귀 프레임 1회뿐이라 예산이 안 찬다. 어느 쪽인지는 아직 실측되지 않았다**(`runInBackground = 0` 은 Standalone 용이라 답이 아니다 — client R8).

`client/ProjectSettings/ProjectSettings.asset:90`에 `runInBackground: 0`이 실제로 들어 있다. 계약은 그것이 Standalone용이라 Editor 답이 아니라고 옳게 적었지만, **"답이 아니다"는 "③이 참이다"가 아니라 "모른다"**다. 그리고 `PeriodicStatusLog.cs` 헤더의 논증은

> Update() is driven by the Editor's PlayerLoop, which SC-89's own accepted evidence (TickCatchUp, ADR-0012 …) already establishes keeps ticking in the background

인데, **ADR-0012/TickCatchUp이 확립한 것은 "누산기가 벌어진다"는 설계 전제이지 관측이 아니다.** 계약이 같은 질문을 "아직 실측되지 않았다"로 열어 둔 상태에서 그것을 근거로 쓰면 **순환이다.**

### 5.3 무엇을 더 봐야 하는가

③을 닫는 데 **사람이 Play를 누를 필요는 없다.** 계약 SC-89 방법 칸의 ①번 순서가 이미 그 절차다 — *"먼저 C-1 계측으로 백그라운드 구간의 드레인 max 와 프레임 간격을 읽어 throttle/pause 를 가른다."* 구체적으로:

1. 백그라운드 구간을 포함한 Play 세션의 `Editor.log`에서 `periodic_status` 줄을 `grep`한다.
2. 연속 줄의 `tick` 간격을 본다. **`application_focused=false`인 줄이 2줄 이상 연속으로 존재하면 ③이 참(throttle)** — 이것이 §7a가 요구하는 "겨냥한 조건이 실제로 발생했다"의 단언이다.
3. `false` 줄이 **0줄이거나 1줄뿐**이면 pause이고, H-13은 목적을 달성하지 못한 것이다. 그 경우 계약이 지정한 대체 벡터(도메인 리로드 반복·강제 GC 루프로 **전경에서** 반복 히치)로 간다.

`ApplicationFocused`를 필드로 남긴 설계 판단은 **정확히 이 확인을 가능하게 한다** — 좋은 설계다. 다만 **그 필드를 읽은 로그가 아직 한 줄도 없으므로**, 지금 상태는 "증명"이 아니라 "증명할 준비"다. 판정: **불충분**. 이 한 번의 `grep`이 나오면 그 자리에서 닫힌다.

---

## 6. SC-56 / SC-89 / 블록 9 (요청 F)

### 6.1 F-1 — **이번 라운드 최대 발견: HUD가 여러 줄의 뒷부분을 소리 없이 버린다**

`GreyboxSession.cs:883-885`:

```csharp
GUI.Box(new Rect(8, 8, 420, 20 + lines.Count * 18), "");
for (int i = 0; i < lines.Count; i++)
    GUI.Label(new Rect(16, 12 + i * 18, 400, 18), lines[i]);
```

**라벨 폭 400 px에 높이 18 px.** 기본 label 스타일이 줄을 넘기면 한 줄만 그려지고 나머지는 **표시되지 않는다 — 말줄임표도 없다.** 관측으로 확인했다. `scan/grid3.png` 좌상 패널 crop(0,180,420,70)을 4배 확대한 결과:

```
cl2_reconcile_has_error_total=2216
send_burst_max_ticks_per_update=20
catchup_carry_forward_ticks_total=90
visible_ships=1
```

세 줄 모두 **첫 필드에서 끝나고 라벨 폭의 절반이 비어 있다.** 같은 패널의 `reconcile_error_m p50=0.0000 p99=0.0009 max=216.8781 n=2216`(59자)은 **온전히** 그려진다 — 즉 잘림이 아니라 **다음 토큰이 안 들어가서 줄바꿈된 뒤 버려진 것**이고, 화면만 보면 그 줄이 원래 그 길이인 것처럼 보인다.

**소리 없이 사라진 필드와 그것이 막는 계약 절:**

| 사라진 필드 | 소스 | 막히는 절 |
|---|---|---|
| `cl2_reconcile_replayed_nonzero_total` | GreyboxSession.cs:840 | **SC-56 (d)②** |
| `cl2_reconcile_both_omega_nonzero_total` | GreyboxSession.cs:840 | **SC-56 (d)③** |
| `send_burst_max_sends_per_frame` | GreyboxSession.cs:847 | **SC-89 판별 단언 (K-7)** — "프레임당 최대 송신 건수 == 1" |
| `send_burst_max_sends_per_trailing_1s` | GreyboxSession.cs:848 | SC-89 보조 |
| `catchup_dormant_ticks_total` · `catchup_truncated_total` · `reconcile_forced_after_hitch_total` | GreyboxSession.cs:851-853 | **SC-89 (f)** |

이것이 §7a가 말하는 결함의 교과서적 형태다: **HUD는 초록으로 보이는데, 판정에 필요한 절반은 애초에 화면에 온 적이 없다.** 그리고 리더의 §5.2 각주 *"`send_burst_max_ticks_per_update=20`(예측 캡, 전송 캡 아님)이 전 구간 유지"*가 정확히 이 함정에 걸려 있다 — **짝이 되는 K-7 값은 한 프레임도 화면에 없었다.**

**완화 요인(공정하게 기록):** `OnSessionEnded`의 `Debug.Log`(GreyboxSession.cs:313-345 부근)는 cl2 ①②③, SC-56 p50/p99/max/n, `_run` 3쌍, SC-89 세션 최악값을 **전부 한 줄씩 온전히 찍는다.** 즉 **데이터는 얻을 수 있다 — HUD 스크린샷으로만 얻을 수 없다.** 따라서 이것은 계측 결함이 아니라 **증거원 선택의 결함**이며, 동시에 HUD 자체의 수정 대상이다(라벨 폭 확대 또는 긴 줄 분할). 담당: client.

### 6.2 SC-56 — **미검증(증거 요건)**

- **세션 스코프 분리: 코드상 정확하다.** `OnSessionReady`(:275-283)가 오차 리스트·퍼센타일 캐시·cl2 3종을 전부 리셋하고, `_hardSnapTotalRun`·`_reconcileError{M,Deg}MaxRun`은 **의도적으로 건드리지 않는다**(주석이 그 의도를 명시). `_run` 3쌍은 세션 종료 로그에서 *"NOT the SC-56 judging value, context only"*라는 라벨과 함께 나온다. **H-10′ 요건 충족.**
- **`max`·`n` 동반 표기: 충족.** HUD 두 줄이 `max=216.8781 n=2216` 형태로 함께 찍고(`scan/grid3.png`에서 실제로 읽힘), 세션 종료 로그는 `_run` 쪽도 `(n=…)`을 각각 붙인다.
- **그러나 판정은 못 한다.** SC-56은 코드 리뷰 항목이 아니라 **"실서버 60초 조작 세션의 (a)~(d) 관찰"** 항목이고, 계약 §0.3은 *"정적 읽기만으로는 PASS가 아니다"*라고 못박는다. 이번 라운드에 그 세션은 새로 촬영되지 않았고, 기존 프레임으로는 **(d)②③이 F-1 때문에 원리적으로 읽히지 않는다.**

### 6.3 SC-89 — **미검증(증거 요건). (b)의 지적은 해소되지 않았다**

리더가 제시한 `cmds 5804 | violations 0 | drops 0`은 **판정 근거로 쓰지 않는다** — 리더 자신이 증거 문서 §3에 그렇게 적었고, 그 판단은 옳다(이 촬영은 SC-89를 겨냥해 설계되지 않았고, 계약 §7b(2)의 "관측 전에 적은 기준"이 이 수에 대해 존재하지 않는다).

**(b)가 여전히 닫히지 않는 이유는 이전과 같고, 하나 늘었다:**

1. (이전) 초록이 나온 상태가 `reconcile_replayed_nonzero_total = 0` + 함선 정지 — 자명 통과 상태였다. **이를 뒤집는 새 관측이 이번 라운드에 없다.**
2. (신규) 계약이 (b)에 **필수로 넣은 판별 단언** — *"같은 창에서 프레임당 최대 송신 건수 == 1"* — 의 값 `send_burst_max_sends_per_frame`이 **HUD에 구조적으로 표시되지 않는다**(F-1). 계약이 *"⚠ 이것이 없으면 수정 전 바이너리도 통과한다"*고 경고한 바로 그 수다. HUD 증거로는 **수정 전과 수정 후를 구분할 수 없다.**

(a)~(f)와 1002 태그 (i)(ii)(iii)에 대한 개별 판정은, 위 (b)가 닫히지 않는 한 항목 전체가 닫히지 않으므로 이번 라운드에 새로 올리지 않는다. **SC-89의 증거원은 HUD가 아니라 `OnSessionEnded` 로그 + `periodic_status` 로그여야 한다** — 계약 방법 칸이 이미 그렇게 적고 있고(*"이 항목의 증거는 화면이 아니라 `grep` 가능한 카운터 로그와 DB 다"*), F-1이 그 지시가 옳았음을 사후적으로 증명한다.

### 6.4 블록 9 — 인스턴스 분리 유지, 그리고 섞임 1건 발견 (F-3)

`evidence/R4-B9/`는 그대로 유지한다(인스턴스 `473421` / `475001` 분리 요구 준수 — 내가 이 디렉터리를 수정하지 않았다).

**그러나 `evidence/R4-B6/sc59-v2/server-stdout.log`가 SC-59 세션의 서버 로그가 아니다.** 그 파일은

```
start_tick=468221  (2026-09-23T13:33:26Z = 22:33 KST)
… stdin 'shutdown' 수신 … tick=473426  (13:37:47Z = 22:37 KST)
```

즉 **블록 9의 `473421` 인스턴스**다. SC-59 녹화는 23:13:49 시작이고 HUD 프레임의 tick은 **511324~512624**다. **SC-59를 서빙한 서버의 로그는 이 증거 폴더에 없다.** 부수 효과로 §5.1의 교차 확인(*"촬영 후 `cmds 5804`. 20 Hz × 290초 ≈ 5,800 — 전경 구간 길이와 일치한다"*)은 **출처가 남지 않은 수에 기대고 있다**(F-4). 그 교차 확인은 "전경 조종 구간이 약 290초였다"는 §5.1의 결론을 떠받치는 유일한 독립 근거였으므로, **그 결론도 프레임 판독만으로 되돌려야 한다.**

---

## 7. 역방향 게이트 (요청 G) — **PASS**

게이트 자신의 검출력부터 확인했다(§7a를 게이트에 적용):

```
$ python tests/e2e/check_contract_items.py --selftest
OK   위반 — 리포트가 SC-87을 판정했는데 계약에 없다 (p1-01 실제 사례) :: 위반=[87] 기대=[87]
OK   깨끗 — 판정된 것이 전부 계약에 있다 :: 위반=[] 기대=[]
OK   순방향은 위반이 아니다 …
OK   산문 속 언급은 세지 않는다 …
OK   리포트 여러 개를 합쳐 본다 :: 위반=[87] 기대=[87]
selftest: PASS  케이스=5
```

**양성 대조(SC-87)가 실제로 빨간불을 켠다.** 이어서 이 리포트 자신에 대해 실행했다:

```
$ python tests/e2e/check_contract_items.py \
    --contract _workspace/p1-01-ship-movement/02_sprint_contract.md \
    --report   _workspace/p1-01-ship-movement/05_qa_report_r5.md
계약 항목: 89
리포트가 판정표 행으로 다룬 항목: 6
계약에 있으나 이 리포트들에 안 나온 항목: 83 (위반 아님 — 대기·블록 분할)
위반 없음 — 판정된 항목이 전부 계약 표에 있다.
exit=0
```

**§7a 단언:** 겨냥한 조건이 발생했다 — 판정표 행 **6건**(SC-46·47·49·56·59·89)이 실제로 파싱됐고, `0건 파싱 후 위반 0`이 아니다. SC-87 사고 형태(계약에 집이 없는 항목의 판정)는 이 라운드에 없다.

---

## 8. 결함·발견 목록

### 8.1 리더에게 (증거 문서 수정 요청)

| # | 위치 | 문제 | 기대 |
|---|---|---|---|
| **L-1′** | 같은 문서 **§6.2 Fact 칸 3행** (§5.3에서 전파) | **Fact 칸 3행이 전부 Claim이다.** HUD `thrust=`·`roll=`은 `_input`(입력 샘플)이고 `speed_mps`·`origin_distance_m`는 **방향 없는 스칼라**이며 **서버가 아니라 클라이언트 예측 상태**에서 읽는다 — **부호가 뒤집힌 빌드가 같은 프레임을 낸다**(§2.1) | 세 행을 "입력 샘플링/이동 발생 = Fact" 와 "방향·서버 반영 = Claim" 두 줄로 쪼갠다. **§5.3은 고치지 말고**(절대 원칙 5) §6에 정정 레코드를 얹는다 |
| **L-5** | 같은 문서 **§6.1** | "SC-59 통과"에 절 구분이 없어 (a)~(d) 전부로 읽힌다. **(b1)은 실제로 EditMode 7건으로 닫혀 있는데** 그 사실이 안 적혀 있어 **기록이 실제보다 약하다** | *"(a)(c)(d)는 제품 책임자 육안 판정, (b1)은 `ReferenceMarkerPlacementTests` 7건, (b2)는 보존 수단 없음"*으로 절을 명시 |
| **L-6** | 같은 문서 **§6.4** | 후속 계획이 실현 불가능하다 — **H-13 로그에는 자세·`thrust`·`roll`·경계 플래그가 없어** 기준 2·5-b를 수집할 수 없다(§2.3-2). 그대로 두면 "해결된 후속"으로 읽힌다 | 기준 4만 수집 가능으로 낮추고, 2·5-b는 `PeriodicStatusLog` 필드 추가(F-8) 이후로 명시 |
| L-2 | 같은 문서 §1 | 다섯 기준이 계약 SC-59의 절((a)(b1)(b2)(c)(d))과 대응하지 않는다. (b1)(b2)가 목록에 없다 | 다음 촬영의 기준표는 계약 절 번호를 그대로 쓴다 |
| L-3 | 같은 문서 §5.1·§3 | `cmds 5804`의 출처 아티팩트가 폴더에 없다. 폴더의 `server-stdout.log`는 **블록 9 `473421` 인스턴스**다(F-3) | 해당 줄에 "출처 미보존" 표기, 또는 §5.1의 290초 결론을 프레임만으로 재유도 |
| L-4 | 같은 문서 §5.2 각주 | `send_burst_max_ticks_per_update=20` "전 구간 유지" — 짝이 되는 K-7 값은 **화면에 뜬 적이 없다**(F-1) | 각주에서 "짝값 미관측" 명시 |

### 8.2 client에게 (계약 외 발견 — 신규 결함)

| # | 위치 | 문제 | 기대 |
|---|---|---|---|
| **F-1** | `GreyboxSession.cs:883-885` | `GUI.Label` 폭 400/높이 18 → cl2·send_burst·catchup 세 줄의 **2~3번째 필드가 화면에 전혀 그려지지 않는다**(말줄임 없음). SC-56(d)②③·SC-89 판별 단언·SC-89(f)의 증거가 HUD에서 원리적으로 안 나온다 | 라벨 폭 확대 또는 긴 줄을 여러 `lines` 항목으로 분할. **수정 후 한 프레임을 찍어 세 줄이 전부 보이는지 확인** |
| **F-2** | `ObserverSession.cs:174` | `Reconcile()`이 `OnWorldSnapshot` 안에서 인라인 호출 — H-9가 `GreyboxSession`에서 고친 것과 **같은 되감기 결함**. `:181` `WriteRow`로 CSV 증거에 새겨진다 | `SnapshotRebaseBatch.ShouldReplacePending` + 지연 적용을 동일하게 적용 |
| **F-5** | `SnapshotRebaseBatchTests.cs` `…DegenerateAlwaysTrueOrAlwaysFalse…` | `true && false`의 상수 접힘. **피시험 대상을 호출하지 않는다** — 구현을 어떻게 망가뜨려도 초록 | 삭제하거나, 두 퇴화 대리를 실제 네 케이스에 돌려 각각 어긋남을 단언 |
| **F-7** | `GreyboxSession.cs` HUD | 계약 SC-59가 요구한 **"녹화 시작 프레임의 HUD에 마커 4개의 ID·거리를 한 줄로"**가 구현되어 있지 않다(`BuildMarkers()`는 있으나 HUD 줄이 없다). (b2)를 영상으로 닫을 수 없다 | 해당 HUD 줄 추가 후 재촬영 |
| **F-8** | `PeriodicStatusLog.cs:39-55` | 필드에 **자세·진행 방향·`thrust`·`roll`·경계 플래그가 없다.** SC-59 기준 2(선회 방향)·5-b(오토레벨)는 이 로그로 **수집 불가**이고, 이것이 §6.4의 후속 계획을 무효로 만든다(L-6) | 자세(쿼터니언 또는 오일러 3축)와 `thrust`·`roll` 양자화 정수를 필드에 추가. **HUD가 아니라 로그여야 하는 이유는 F-1과 같다** |

### 8.3 기록 (판정하지 않음)

- **F-6 빌드 신선도.** `GreyboxSession.cs` mtime `2026-09-23 23:29:48`, `PeriodicStatusLog.cs` `23:29:34`, `SnapshotRebaseBatch.cs` `23:27:03`. SC-59 녹화의 **전경 구간은 23:13:49~23:18:29**이다. 즉 **촬영된 빌드는 H-9·H-13 이전**이다. 부호 판정에 H-9가 영향을 준다고 보지는 않으나, 다음 촬영은 현재 트리로 다시 해야 한다.
- **M-14 (계약 §5 기록 항목) — 발동 조건 해소, 그러나 절반만.** M-14는 *"SC-59가 실행되지 않았으면 '부호 미검증'을 요약에 명시"*다. **SC-59는 실행됐다** — 계약 §0.10이 지정한 검출기(사람 눈)가 제품 책임자의 조종으로 실제로 돌았고 통과 판정이 나왔다. 따라서 "부호 미검증"은 더 이상 적지 않는다. **다만 §0.10의 나머지 절반은 유효하다: 그 관측의 제3자 대조 가능한 보존 기록이 없다.** 요약에 적는 문장은 이것이다 — **"부호는 제품 책임자 육안으로 판정됐고, 그 관측을 되짚을 수 있는 기록은 없다."**
- **계약 §0.3에 이 판정의 칸이 없다(architect 사안).** §2.5 참조 — 칸을 신설하려면 **§0.10이 검출기를 사람 눈으로 지정한 항목**으로 적용 범위를 묶어야 한다. 묶지 않으면 모든 항목의 탈출구가 된다(§7b(1)을 그 칸 자신에 돌린 결과).

---

## 9. 실행 증거 색인

| 산출물 | 경로 |
|---|---|
| EditMode GREEN (판정용) | `_workspace/p1-01-ship-movement/unity-tests-qa-r5/EditMode.nunit.xml`, `EditMode.xml` |
| EditMode RED (+1 결함 주입) | `_workspace/p1-01-ship-movement/unity-tests-qa-r5/EditMode.RED.nunit.xml` |
| EditMode GREEN (복원 확인) | `_workspace/p1-01-ship-movement/unity-tests-qa-r5/EditMode.GREEN-after-restore.nunit.xml` |
| 역방향 게이트 selftest | §7 인라인 |

명령:

```
unity test client --mode EditMode --report-format nunit,junit \
  --output <qa-r5>/EditMode.nunit.xml --junit-output <qa-r5>/EditMode.xml
  → exit 0, total=223 passed=221 failed=0 skipped=2 (start 2026-09-23 14:48:41Z)

(TickCatchUp.cs:144 에 +1 주입)
unity test client --mode EditMode --report-format nunit --output <qa-r5>/EditMode.RED.nunit.xml
  → exit 8, total=223 passed=210 failed=11 skipped=2

(백업에서 복원, md5 e1fb231c21a4b8f70d0f698d4461ce47 일치)
unity test client --mode EditMode --report-format nunit --output <qa-r5>/EditMode.GREEN-after-restore.nunit.xml
  → exit 0, total=223 passed=221 failed=0 skipped=2 (start 2026-09-23 14:50:15Z)

python tests/e2e/check_contract_items.py --selftest
  → selftest: PASS 케이스=5
```

**건수 원칙(§0.3):** Unity 순회 건수는 리포트의 `testcasecount=223`이고, 그중 이번 판정이 겨냥한 경로가 실제로 실행됐음을 개별 확인했다 — `GreyboxSendBurstTests` 2건, `SnapshotRebaseBatchTests` 6건, `TickCatchUpTests` 13건, `ContractFixtureTests.Fixtures_RoundTrip_Found27_RoundTripped21` 1건이 모두 `Passed`로 리포트에 존재한다. **"검증기가 꺼진 채 0건 통과"가 아니다.**

---

## 10. 라운드 5 결론

- **PASS 3건**(SC-46·47·49) — 전부 실행 증거 있음.
- **통과 1건**(SC-59) — **제품 책임자 판정(증언 3건 + 프레임 판독 3건)**. qa 판정 아님. §6 기록은 형태가 정직하나 **Fact 칸 3행이 Claim이고**(L-1′) **후속 계획이 실현 불가능하다**(L-6).
- **미검증(증거 요건) 2건**(SC-56·89) — 비-통과. **슬라이스 종료를 막는다.**
- **FAIL 0건이지만 막고 있는 게이트 수는 줄지 않았다.**
- **신규 결함 5건**(F-1·F-2·F-5·F-7·F-8) — 전부 client.
- **리더 수정 요청 6건**(L-1′·L-2·L-3·L-4·L-5·L-6).
- **architect 사안 1건**: §0.3에 "제품 책임자 판정" 칸이 없다(§2.5).

**SC-59의 통과는 SC-89를 건드리지 않는다.** 두 항목은 재는 성질이 다르고(부호 vs 송신 속도), SC-89가 막혀 있는 이유는 §6.3의 두 가지 — (b)의 자명 통과 지적 미해소, 판별 단언 값의 HUD 부재 — 이며 **둘 다 SC-59와 무관하다.** 이 리포트에서 SC-59를 근거로 SC-89 쪽으로 내려간 문장은 하나도 없다.

**하나만 고른다면 F-1이다.** SC-56·SC-59·SC-89 세 항목이 전부 HUD 프레임을 증거원으로 삼고 있었는데, 그 HUD는 판정에 필요한 필드의 상당수를 **한 번도 그린 적이 없다.** 계약이 SC-89에 대해 *"이 항목의 증거는 화면이 아니라 `grep` 가능한 카운터 로그와 DB 다"*라고 적어 둔 것이 옳았고, **같은 원칙을 SC-56과 SC-59에도 적용해야 한다.** H-13의 `periodic_status` 줄은 `Format()`에서 그 세 줄의 모든 필드를 온전히 찍으므로 **이미 그 대체 증거원이다** — §5.3의 `grep` 한 번이 H-13과 SC-89 양쪽을 동시에 여는 가장 짧은 경로다.

**그리고 이 라운드가 남기는 교훈 하나.** SC-59에서 오늘 네 번 촬영이 실패한 원인은 함선이 아니라 **관측 장치**였다 — `OnGUI`가 비포커스에서 안 돌고(§6.3), HUD 라벨이 필드를 버리고(F-1), 주기 로그에는 필요한 필드가 없다(F-8). **제품은 되는데 기록이 안 됐다.** 제품 책임자가 직접 조종해서 판정해야 했던 것 자체가 그 사실의 증거다. F-1·F-7·F-8 셋을 고치면 다음 촬영은 **사람 증언 없이도 닫힌다.**

# p1-01 함선 이동 — 평가 리포트 라운드 7 (qa)

- 평가자: qa (독립 판정)
- 일시: 2026-09-24 KST
- 대상: **리더 R14** — qa r6 지적에 대해 리더가 혼자 작성하고 혼자 보고한 수정 5건 + 갱신된 재촬영 체크리스트
- 도구: Unity 6000.6.1f1 CLI(EditMode, 실행 5회), python 3.12
- 규율: 계약 §0.3 · §0.12 · §7a · §7b
- 전제: **R14도 작성자와 보고자가 동일인이고 교차 검토가 없다.** 보고된 값은 하나도 신뢰 기반으로 받지 않았다.

> **§0.3 요구 문장:** 이 라운드에도 FAIL은 0이고 **막고 있는 게이트 수는 줄지 않았다.** `미검증(증거 요건)` 2건(SC-56·SC-89)은 비-통과이며 슬라이스 종료를 FAIL과 똑같이 막는다. **R14는 이번에도 증거를 만들지 않았다** — 실서버 세션도, 녹화도 없다. 만든 것은 다음 촬영이 닫을 수 있는 범위를 넓힌 것뿐이고, 그 범위조차 §E에서 보듯 아직 전부 열리지 않았다.
>
> **SC-59는 제품 책임자 판정으로 통과다. 뒤집지 않는다.**

---

## 0. 판정 요약

### 0.1 R14 수정 5건

| 항목 | 리더 보고 | 내 독립 검증 | 판정 |
|---|---|---|---|
| **1. 축 규약 주석 정정** | "고쳤다" | 결론(`right wing RISES`)은 **맞다**(§1.1 수치 유도). **그러나 같은 문장의 `right-hand-rule about +Z`가 틀렸다** — 이 좌표계는 ADR-0009가 **왼손 좌표계**라고 못 박았고, 기하학적 오른손 법칙을 +Z에 적용하면 **정확히 반대 결론**이 나온다 | **부분 PASS — 문장이 여전히 자기모순이다** |
| **2. `flight_assist` 추가** | "기준 5-b를 닫는다" | 필드는 실재하고 HUD와 같은 원천이다. **그러나 (a) 오토레벨 조건은 `FlightAssist`만이 아니다** (§2.1) **(b) 신규 테스트가 `FlightAssist`와 `BoundarySoftCrossed`를 구분하지 못한다 — 실측으로 초록 확인**(§2.4) | **부분 PASS — 새 테스트가 §7b(1) 자명 통과다** |
| **3. `runInBackground` 논거 교체** | "근거 둘로 교체" | 근거 (1)은 타당. **근거 (2)의 사실 진술이 틀렸다** — 계약의 2~4 Hz는 **백그라운드** 벡터이지 "전경 throttle"이 아니다(§4.1). 이 오기가 **출하 코드 주석에 들어가 있다** | **부분 PASS — 결론 유지, 새 사실 오류 1건** |
| **4. `PendingRebaseSlot` 추출** | "상태 기계는 닫혔고 배선은 미검증" | **리더의 자기 평가가 정확하다.** 호출 삭제 실험 재현 → 여전히 초록(§3.1). RED 재현 1건 일치(§3.3). 추가 퇴화(stale payload)도 잡는다(§3.4) | **PASS — 이 라운드에서 가장 정직한 항목** |
| **5. 재촬영 체크리스트 갱신** | "사람 눈 없이 닫는 판독 경로" | 로그 경로·grep 명령은 **실제로 동작한다**(§5.1, 실측). **그러나 세 단계가 빠졌고, 기준 2의 판독 절차는 이 함선의 선회 속도에서 원리적으로 성립하지 않는다**(§5.2) | **부분 PASS — 지시서로 아직 실행 불가** |

### 0.2 계약 항목

| SC | 판정 | 한 줄 근거 |
|---|---|---|
| SC-46 | **PASS** | `unity test client --mode EditMode` exit 0, `total=243 passed=241 failed=0 skipped=2`, nunit·junit 리포트 2개 생성(§6) |
| SC-56 | **미검증(증거 요건)** | 이번 라운드에 실서버 60초 세션 신규 촬영 **없음**. R14는 SC-56에 아무것도 더하지 않았다 |
| SC-59 | **통과 — 제품 책임자 판정** (qa 판정 아님) | 기록 검증만 수행. §6.4의 기준 2·5-b는 **아직 로그로 닫히지 않는다**(§2.3·§5.2) |
| SC-89 | **미검증(증거 요건)** | (b) 자명 통과 지적 **여전히 미해소** — 히치가 실제로 있는 세션이 이번에도 없다. 1002 (i)(ii) 닫힘 재확인, **(iii) 안 닫힘**(§4.2) |

### 0.3 결론 한 줄

**리더는 qa r6가 지적한 세 오류 중 둘은 실제로 고쳤고, 하나(축 규약)는 결론만 고치고 근거는 반대로 남겼다. 그리고 5-b를 닫으려고 추가한 테스트가 §7b(1) 자명 통과다 — 이 슬라이스가 다섯 번째로 같은 형태에 걸렸다.**

---

## 1. A — 축 규약 정정이 이번엔 맞는가 (최우선)

**전임자의 결론을 재인용하지 않았다.** `ShipIntegrator.cs`와 `Quatd.cs`를 직접 읽고, 적분기 5단계 후반부를 그대로 옮겨 수치로 돌렸다.

### 1.1 결론("right wing RISES")은 **맞다**

`Quatd.cs:37-41`(Hamilton 곱), `:63-68`(`Rotate(v) = q·(v,0)·conj(q)`), `ShipIntegrator.cs:147-149`(`w2 = fwd * (omegaRoll * π/180)`, `q = q + (Quatd.Pure(w2) * q) * (0.5*dt)`, `Normalized()`)를 그대로 재현:

```
+90 deg/s 롤, dt=0.05, 20 step (= 1초), q0 = Identity:
  q      = (0, 0, 0.706822, 0.707392)
  ship +X (우현) → 월드 (0.0008,  1.0000, 0.0000)  = 월드 +Y = 위
  ship +Y (상방) → 월드 (-1.0000, 0.0008, 0.0000)  = 월드 -X = 왼쪽
  ship +Z (선수) → 월드 (0.0000,  0.0000, 1.0000)  = 불변 (롤 축이 맞다)
-90 deg/s:
  ship +X        → 월드 (0.0008, -1.0000, 0.0000)  = 아래
```

**양의 롤은 우현 날개를 올린다.** R14의 정정 결론은 옳다.

**§7a 단언 — 이 유도가 겨냥한 조건이 실제로 발생했는가:** 겨냥한 것은 "롤 부호가 자세에 미치는 방향"이고, ship +Z가 불변으로 남은 것이 **롤 축이 실제로 선수축이었다**는 동반 증거다. 축이 엉뚱했다면 +Z도 움직였을 것이다. 그리고 음의 롤에서 부호가 **실제로 뒤집혔다** — 한쪽만 재고 "부호를 확인했다"고 적지 않았다.

### 1.2 그러나 **같은 문장의 근거 절이 틀렸다** — 이번에도 물리 문장이 거꾸로다

현재 `PeriodicStatusLog.cs:85-87`:

> ```
> // Roll is the scalar rate about ship-local +Z (Sim/ShipSimState.cs:19); positive roll is
> // right-hand-rule about +Z, which maps ship +X onto ship +Y - i.e. the ship's right wing
> // RISES.
> ```

**`right-hand-rule about +Z`가 이 좌표계에서 틀렸다.**

ADR-0009 §1 표(`docs/adr/0009-*.md:28`)는 이렇게 못 박는다:

> | 축 | **왼손 좌표계, Y-up.** +X = 오른쪽, +Y = 위, +Z = 전방 |
> | 회전 | 쿼터니언 `(x, y, z, w)`, Hamilton 곱. **양의 각속도 = Unity의 양의 회전 방향** |

그리고 같은 ADR `:33`은 **"오른손 좌표계(수학 관례)를 쓰지 않는다"**를 굵게 적고, 그 이유로 *"변환 계층은 부호 버그가 사는 곳"*이라고 쓴다.

기하학적으로 따지면 이렇다. 관측자가 함선 뒤에서 같은 방향을 보고 선다 → 망막에 **+Y 위, +X 오른쪽, +Z 화면 안쪽**. 오른손 엄지를 +Z(화면 안쪽)로 두면 손가락은 **관측자 기준 시계 방향**으로 감긴다. 시계 방향은 `+Y → +X`, 즉 **`+X → -Y`(우현 날개 하강)**이다.

**즉 이 좌표계에서 기하학적 오른손 법칙을 +Z에 적용하면 "drops"가 나온다** — R13이 원래 썼던 그 결론이. 대수(Hamilton 성분 계산)는 `+X → +Y`를 주고, 그것이 **왼손 법칙 = Unity의 양의 회전 방향**이며 ADR-0009가 실제로 지정한 규약이다.

**그래서 현재 문장은 자기모순이다.** 한 문장 안에 "right-hand-rule about +Z"와 "maps +X onto +Y"가 같이 있는데, 이 basis에서 둘은 같은 것을 가리키지 않는다. **F-8과 이 주석 블록의 존재 이유가 "제3자가 소스를 열지 않고 방향을 판정하게 하는 것"인데, 그 제3자가 명명된 법칙을 적용하면 결론과 반대 답을 얻고 어느 쪽을 믿을지 알 수 없다.** qa r6가 잡은 것과 **같은 자리, 같은 종류**의 결함이 한 칸 옮겨 남아 있다.

R14 본문(`03_client_impl.md:1578`)도 같은 오기다: *"+Z축 오른손 회전은 +X를 +Y로 보낸다."*

**수정 요청 (F-13, client — 우선):** 정확한 단어는 ADR-0009가 이미 준다.
*"positive roll = Unity's positive rotation direction about ship-local +Z (ADR-0009 §1 — this is a LEFT-handed basis, so it is the left-hand rule, NOT the right-hand rule; applying the right-hand rule here gives the opposite answer). It maps ship +X onto ship +Y — the right wing RISES. Verified numerically against ShipIntegrator.cs:147-149 + Quatd's Hamilton product."*
R14 본문 문장도 같이 정정한다(절대 원칙 5 — 정정 레코드로).

### 1.3 주석 나머지는 **한 글자도 어긋나지 않는다** (대조 완료)

| 주석의 주장 | 실제 코드 | |
|---|---|---|
| `+X = ship-right, +Y = ship-up, +Z = ship-forward (bow)` | ADR-0009 §1 표 `:28`·`:30`과 일치 | **일치** |
| `mirrored by Sim/ShipControlInputD.cs:17` | `:17` = `/// +X ship-right, +Y ship-up, +Z ship-forward (ADR-0009 section 1).` | **일치 — 줄 번호까지 정확** |
| `thrust=(0,0,1000)` 전방 / `(0,1000,0)` 위 / `(1000,0,0)` 우현 | `ShipIntegrator.cs:152-155` — X·Y는 `LateralThrustMps2`, Z는 전/후진 분기. 로컬 축 그대로 | **일치** |
| `Roll is the scalar rate about ship-local +Z (Sim/ShipSimState.cs:19)` | `:19` = `/// <summary>omega_roll - scalar rate about the ship-local +Z axis.</summary>` | **일치 — 줄 번호까지 정확** |
| `Attitude*`는 현재 자세, `AimTarget*`은 조준 목표 | `GreyboxSession.cs:687-696` — 전자는 `_controller.CurrentState.Orientation`, 후자는 `_input.AimTargetWorld` | **일치** |
| `ShipIntegrator.cs:122`(`flight_assist` 주석) | `:122` = `if (input.Roll != 0.0 \|\| !input.FlightAssist)` | 줄 번호 정확, **의미 해석은 불완전** → §2.1 |

**복제본 없음:** `grep -rn "right wing\|right-hand\|left-hand" client/` → `PeriodicStatusLog.cs:86` **1건뿐**. R14의 "복제본 확인" 요구는 충족된다.

---

## 2. B — `flight_assist` 추가가 기준 5-b를 실제로 닫아주는가

### 2.1 (1) 오토레벨 조건은 **`FlightAssist`만이 아니다**

`ShipIntegrator.cs:119-140`:

```csharp
Vec3d fwd = q.Rotate(LocalForward);
if (input.Roll != 0.0 || !input.FlightAssist)   // 수동 롤
{ omegaRollT = input.Roll * ship.RollRateMaxDegS; }
else                                            // 오토레벨
{
    Vec3d u = WorldUp - fwd * Vec3d.Dot(WorldUp, fwd);
    double nu = u.Length();
    if (nu < ship.AutoLevelDeadzoneSin) { omegaRollT = 0.0; }   // ← 게이트 ②
    else {
        Vec3d upS = q.Rotate(new Vec3d(0.0, 1.0, 0.0));
        double sinErr = Vec3d.Dot(Vec3d.Cross(upS, u / nu), fwd);
        omegaRollT = Math.Abs(sinErr) < ship.AutoLevelDeadzoneSin ? 0.0 : ship.AutoLevelRateDegS * sinErr;  // ← 게이트 ③
    }
}
```

오토레벨이 실제로 각속도를 내려면 **세 조건이 전부** 필요하다:

1. `input.FlightAssist == true` **그리고 `input.Roll == 0.0`** — 리더는 앞의 하나만 적었다.
2. `nu >= AutoLevelDeadzoneSin` — **선수가 월드 up과 거의 평행하면 롤 기준이 없어 오토레벨이 정당하게 아무것도 안 한다.**
3. `|sinErr| >= AutoLevelDeadzoneSin` — 이미 수평이면 0.

**R14의 "오토레벨은 `input.FlightAssist`일 때만 돈다"는 필요조건을 충분조건처럼 적은 것이다.** 판정에 미치는 영향은 실제로 있다: `flight_assist=true`인데 자세가 수평으로 수렴하지 않는 구간을 본 제3자는 "오토레벨 고장"이라고 읽지만, **게이트 2가 걸린 수직 상승 구간이면 정상 거동**이다.

### 2.2 (2) 그래도 **제3자가 기준 5-b를 판정할 수 있는가 — 직접 유도했다. 할 수 있다, 단 조건부다**

기준 5-b는 `11-sc59-observation.md:134` = **"오토레벨 복귀 / Q·E 유지 시 억제"** (계약 SC-59 (d) ①②).

로그 14필드만으로 유도 가능한 것:

| 필요한 사실 | 유도 경로 | 가능? |
|---|---|---|
| 어시스트가 켜져 있었다 | `flight_assist=true` | **가능** (R14가 연 것) |
| 손을 뗐다 (수동 롤 없음) | `roll=0` — **이미 있던 필드** | **가능** |
| 선수가 월드 up과 평행하지 않았다 (게이트 2) | `fwd = rot(q, (0,0,1))`를 `attitude_*`에서 계산 → `\|fwd·(0,1,0)\| ≪ 1` | **가능** |
| 수평을 되찾았다 (①) | `upS = rot(q,(0,1,0))`, `u = (0,1,0) - fwd·dot((0,1,0),fwd)`, `sinErr = dot(cross(upS, u/\|u\|), fwd)` → `\|sinErr\| → 0` | **가능** — 적분기 `:136-137`과 같은 식 |
| 수동 롤 중에는 억제된다 (②) | `roll≠0` 구간에서 위 `sinErr`가 0으로 수렴하지 않음 | **가능** |

**그래서 (2)의 답은 "유도할 수 있다"다.** `flight_assist` + `roll` + `attitude_*` 넷이 있으면 닫힌다. 리더가 고른 필드는 **옳았다.**

**그러나 주석은 그 유도를 가능하게 만들지 않는다. 세 가지가 빠졌다.**

1. **`attitude_*`가 월드 프레임이라는 말이 한 글자도 없다.** `GreyboxSession.cs:693-696`은 `_controller.CurrentState.Orientation`(월드 자세)을 찍는다. 위 유도가 성립하는 이유가 바로 그것인데, 주석은 프레임을 말하지 않는다.
2. **"수평"의 정의가 없다.** 월드 up = `(0,1,0)`이라는 사실과 `sinErr` 식은 `ShipIntegrator.cs:128-137`에만 있다. 제3자는 적분기를 열어야 한다 — F-8의 취지에 반한다.
3. **회전 적용 형태(`v' = q·(v,0)·conj(q)`, Hamilton)가 없다.** `Quatd.cs:57-68`에만 있다.

**1·3은 qa r6가 F-10으로 이미 요청한 것이고, R14는 F-10에 손대지 않았다.** R14 문서에 "보류"라고도 적혀 있지 않다 — 그냥 빠졌다(§7 미대응 목록).

**§7a 단언:** 위 "가능" 판정은 내가 **실제로 식을 적분기에서 옮겨 와 맞춰 본 결과**이고, 가능한 것은 *유도 절차*이지 *현재 주석을 읽은 제3자가 그 절차에 도달하는 것*이 아니다. 둘은 다르다.

### 2.3 (2) 계속 — **5초 주기가 5-b와 기준 2를 다르게 제약한다** (계약 외 발견, 중요)

`GreyboxSession.cs:653`: `PeriodicStatusLogIntervalSeconds = 5.0`. `data/ships/scout-s01.json`: `turn_rate_max_deg_s = 75.0`, `roll_rate_max_deg_s = 90.0`, `auto_level_rate_deg_s = 40.0`.

- **기준 2(선회 방향)는 이 주기에서 원리적으로 안 읽힌다.** 체크리스트가 지시하는 `q_rel = conj(q1)·q2`는 **연속 두 줄** 사이 회전인데, 그 간격이 5초이고 최대 선회율이 75 deg/s면 **최대 375°**다. 쿼터니언 최단경로 해석은 **180°를 넘으면 부호가 뒤집힌다** — 즉 **2.4초 이상 최대 선회하면 "오른쪽으로 돌았다"가 "왼쪽으로 돌았다"로 읽힌다.** 이것은 정확히 이 슬라이스가 잡으려는 종류의 부호 오독이고, **판독 절차 자신이 그것을 제조한다.**
- **기준 5-b(오토레벨)는 읽히되 표본이 두 점뿐이다.** 40 deg/s면 45° 뱅크가 ~1.1초에 회복되므로 **한 5초 창 안에서 시작하고 끝난다.** 운 나쁘면 `banked` 줄이 하나도 안 남는다. 조작자가 **각 구간을 5초 이상 유지**해야 하는데, 체크리스트는 그 말을 하지 않는다(§5.2).

**수정 요청 (F-14, client):** 둘 중 하나. (a) `PeriodicStatusLogIntervalSeconds`를 1.0으로 낮춘다 — 60초 세션에 60줄, grep에 아무 부담이 없다. 또는 (b) 체크리스트에 **"기준 2는 각 선회를 2초 이내로 짧게 끊고, 5-b는 각 구간을 10초 이상 유지한다"**를 넣고 그 근거(75 deg/s × 2.4 s = 180°)를 적는다. **(a)를 권한다** — 절차서가 아니라 계측이 성질을 보장하는 쪽이다.

### 2.4 (2) 계속 — **신규 테스트가 §7b(1) 자명 통과다. 실측으로 확인했다**

`PeriodicStatusLogTests.Format_FlightAssist_PrintsBothStates`(`:208-230`)는 주석에 *"Both states asserted (§7b(1)): a hard-coded 'true' would satisfy either one alone"*라고 적혀 있다. 상수 접힘은 막는다. **그러나 잘못된 필드 배선은 못 막는다.**

이유: `Sample()`은 `boundarySoftCrossed: true, flightAssist: true`이고, assistOff 변형은 **둘 다 false**다. 두 bool이 **두 케이스에서 완전히 상관**이므로, `Format()`이 `flight_assist=`를 `BoundarySoftCrossed`에서 뽑아도 테스트는 통과한다.

**주입해서 확인했다** — `Format()`의 마지막 줄을 `" flight_assist=" + (BoundarySoftCrossed ? "true" : "false")`로:

```
exit=0, total=243 passed=241 failed=0 skipped=2
```

**초록이다. 243건 어느 것도 `FlightAssist`와 `BoundarySoftCrossed`를 구분하지 못한다.**

증거: `unity-tests-qa-r7/EditMode.PROBE-AssistFromBoundary.nunit.xml`. 원복 후 md5 `8d908903b5f9e71d93ac07af00ba0534` 일치, 표식 잔존 0건.

이것이 왜 중요한가: **`flight_assist`는 5-b 판정의 유일한 어시스트 신호이고, `boundary_soft_crossed`는 자유 비행 대부분에서 `false`다.** 잘못 배선되면 로그 전체가 `flight_assist=false`로 찍히고, 제3자는 **"어시스트가 꺼져 있었다"**고 읽어 5-b를 조용히 닫지 못한 채 끝낸다. 겨냥한 조건을 재지 못하는 관찰이다.

**수정 요청 (F-15, client — 우선):** `Sample()`의 두 bool을 **엇갈리게** 만들거나(`boundarySoftCrossed: true, flightAssist: false` 한 케이스 추가), `Format_AttitudeThrustRollAndBoundary_EachFieldAppearsWithItsOwnValue`에 네 조합 중 최소 두 개의 **불일치** 조합을 넣는다. 다른 12개 정수 필드는 `Sample()`이 전부 다른 값(111/222/333/444/…)이라 이 병이 없다 — **bool 둘만 같은 값을 공유한다.** 그 주석(`:30-32`)이 *"Every value distinct and non-zero so a Format() that drops one, or swaps two axes, cannot still produce a passing line"*라고 적고 있는데, **그 규율이 bool 두 개에는 적용되지 않았다.**

### 2.5 (3) 호출부가 HUD와 **같은 값**을 넣는가 — 같은 원천이되, **한 경우에 어긋난다**

| | HUD (`GreyboxSession.cs:904-910`) | 로그 (`:683-698`) |
|---|---|---|
| 원천 | `_input.FlightAssist` | `_input.FlightAssist` |
| 재유도 | 없음 | 없음 |
| **`_input == null`일 때** | **`"no input sampler"`** (행 전체 대체) | **`flight_assist=false`** |

**재유도는 없다 — 리더의 설계는 옳다.** 그러나 `GreyboxSession.cs:681-682`의 주석은 이렇게 주장한다:

> `// F-8: the same quantised integers OnGUI prints and the wire carries - never a re-derivation, so the log and the HUD can never disagree about what was sent.`

**`can never disagree`는 참이 아니다.** 샘플러가 없으면 HUD는 정직하게 `"no input sampler"`라고 말하고, 로그는 **측정된 것처럼 보이는 `flight_assist=false`·`roll=0`·`thrust_x=0`**을 찍는다. qa r6가 F-12로 올린 침묵 센티널이고, **R14는 그 패턴을 그대로 복사해 14번째 필드를 추가했다** — 하필 5-b의 유일한 어시스트 신호에.

실무 위험은 낮다(씬에 샘플러가 배선돼 있으면 `_input`은 null이 아니다). **기록 항목으로 남기되, 주석의 `can never disagree` 문장은 좁혀야 한다** — §7a는 "관찰이 겨냥한 조건이 실제로 발생했는지를 함께 단언하라"이고, 이 문장은 그 반대 방향의 과장이다. (F-12′)

---

## 3. C — `PendingRebaseSlot` 추출이 F-2를 얼마나 닫았는가

### 3.1 (1) 리더의 자기 평가는 **정확하다** — 후하지도 박하지도 않다

리더: *"닫힌 것은 상태 기계의 거동이다. `ObserverSession`이 그 객체를 실제로 호출하는 배선은 여전히 어떤 테스트도 실행하지 않으므로, qa가 했던 '호출 삭제' 실험은 지금도 초록일 것이다."*

**재현했다.** `ObserverSession.cs:270`의 `ApplyPendingRebase();`를 주석 처리:

```
exit=0, total=243 passed=241 failed=0 skipped=2
```

**기준선과 바이트 단위로 같다.** 증거: `unity-tests-qa-r7/EditMode.RED-F2-null.nunit.xml`. 원복 후 md5 `d2f54c230665116a0f37e96bfabddca3` 일치.

**리더가 자기 성과를 과소평가한 것이 아니다 — 정확히 맞게 적었다.** 이 슬라이스에서 리더가 "닫혔다"와 "닫히지 않았다"의 경계를 스스로 정확히 그은 첫 사례이므로 명시한다. F-2′는 **열린 채로 남는다.**

### 3.2 (2) `ObserverSession` 마이그레이션이 거동을 바꾸지 않았는가 — **동치다**

R13은 `else if (SnapshotRebaseBatch.ShouldReplacePending(tick, _pendingRebaseTick)) { 5필드 대입 }`이었고(qa r6 §2.2가 `:182,196-200`으로 인용), R14는 `else { _pendingRebase.TryQueue(...); }`이며 `TryQueue`(`PendingRebaseSlot.cs:41-51`)가 **같은 `SnapshotRebaseBatch.ShouldReplacePending`을 첫 줄에서 호출**하고 거짓이면 `return false`한다.

**동치다.** 차이는 반환값(버려짐) 하나뿐이고, 판정 함수는 **같은 순수 함수의 같은 호출**이지 사본이 아니다(`:43`).

**첫 스냅샷 분기(`_controller == null`)와의 상호작용 — 따졌다. 문제없고, 오히려 R14가 안전판 하나를 더 걸었다.**

- 첫 스냅샷은 컨트롤러를 만들고 **즉시 `WriteRow`** 한 뒤 큐잉하지 **않는다**(`:167-179`). 같은 프레임에 뒤이어 온 스냅샷만 큐에 들어간다. 그래서 첫 프레임에 100·102·101이 한꺼번에 드레인되면 행이 둘 — tick 100(생성) → tick 102(재조정) — 나오고 **tick은 단조**다. 되감김 없음.
- `ApplyPendingRebase()`는 `_controller`를 null 검사 없이 역참조한다(`:230`). **안전한 이유는 `OnSessionReady`가 `_controller = null`과 함께 `_pendingRebase.Clear()`를 하기 때문**이고(`:124`), 그 `Clear()`는 이번 마이그레이션에서 들어왔다. **적재 하중이 큰 한 줄인데, 이것 역시 배선이므로 테스트가 0건이다** — `PendingRebaseSlotTests.Clear_DropsAQueuedSnapshot…`이 재는 것은 슬롯이지 `OnSessionReady`가 아니다. F-2′에 포함해 기록한다.

### 3.3 (3) 신규 테스트 5건이 §7b(1)을 만족하는가 — **만족한다. 자명 통과 상태 없음**

`PendingRebaseSlotTests.cs` 5건의 판별력을 퇴화별로 따졌다:

| 퇴화 | 깨지는 테스트 |
|---|---|
| `TryQueue` 항상 true (판정 제거) | `TryQueue_ReportsWhetherItReplaced` (`102==102`, `101<102` 두 케이스) |
| `TryQueue` 항상 false | `OutOfOrderBatch…`, `TryTake_EmptiesTheSlot…`, `Clear_Drops…` |
| `TryTake`가 `Clear()` 안 함 | `TryTake_EmptiesTheSlot…` — **주입 확인, §3.4** |
| `TryTake` 항상 false | `OutOfOrderBatch…`, `TryTake_EmptiesTheSlot…` |
| `HasPending` 항상 false/true | `TryTake_EmptiesTheSlot…` / `EmptySlot_TakesNothing` |
| `Clear()` no-op | `Clear_DropsAQueuedSnapshot…` |
| **tick만 맞고 payload가 오래됨** | `OutOfOrderBatch…` — **주입 확인, §3.4** |

**§7b(1) 짝이 실제로 걸려 있다:** `TryTake_EmptiesTheSlot…`(채운 뒤 비워지는가)과 `EmptySlot_TakesNothing`(안 채운 슬롯이 기본 tick 0을 쥐고 있지 않은가)이 짝이고, 후자가 없으면 전자는 "0을 0과 비교"가 된다. 테스트 주석(`:71-72`)이 그 짝임을 명시한다. **이 부분은 옳게 적용됐다.**

### 3.4 (4) RED 재현 — **리더 보고와 일치. 그리고 묻지 않은 퇴화도 잡는다**

**리더가 보고한 형태** (`PendingRebaseSlot.cs:71`의 `Clear();` 제거):

```
exit=8, total=243 passed=240 failed=1        ← 리더 보고 "1건 실패"와 일치
FAILED Starfall.Tests.EditMode.PendingRebaseSlotTests
         .TryTake_EmptiesTheSlot_SoAFrameWithNoSnapshotsReconcilesNothing
```

**내가 추가로 돌린 퇴화** (`TryQueue:46`의 `_confirmed = confirmed;` 제거 — tick은 갱신하되 payload는 옛것을 쥔다. §3.3 표의 마지막 행):

```
exit=8, total=244 passed=241 failed=1
FAILED PendingRebaseSlotTests.OutOfOrderBatch_OnlyHighestTickIsTaken_AndItsPayloadTravelsWithIt
```

**잡는다.** 테스트 주석이 주장하는 *"Asserting the payload (not just the tick) is what catches a slot that picks the right tick but keeps an older snapshot's state"*는 **참이다** — 주장이 아니라 실측이다. (총계가 244인 것은 같은 실행에 §5.1의 임시 로그 프로브 1건이 함께 있었기 때문이다.)

증거: `unity-tests-qa-r7/EditMode.RED-TryTakeClear.nunit.xml`, `EditMode.RED-StalePayload.nunit.xml`.

---

## 4. D — `runInBackground` 논거 교체가 타당한가

### 4.1 근거 (2)의 **사실 진술이 틀렸다. 계약에서 직접 확인했다**

R14 §3(`03_client_impl.md:1598-1600`):

> *"계약 자신이 2~4 Hz 전경 throttle을 재현 벡터로 적어 두었다."*

그리고 같은 문장이 **출하 코드**에 들어가 있다 — `PeriodicStatusLog.cs:18`:

> `// burst - the contract itself names a 2-4 Hz foreground throttle as a good reproduction vector.`

**계약 SC-89 방법 칸(`02_sprint_contract.md:185`)의 실제 문장은 이렇다:**

> **재현 절차**: 자유 비행보다 **백그라운드 전환**이 훨씬 잘 재현한다 — … **백그라운드에서 Editor 가 2~4 Hz 로 조이면** 2 Hz = 프레임당 10 tick = **매 프레임 위반 1회** → 4초면 예산이 찬다.
> **⚠ 그러나 그 산수는 Editor 가 throttle 할 때만 성립한다 — pause 하면 복귀 프레임 1회뿐이라 예산이 안 찬다. 어느 쪽인지는 아직 실측되지 않았다.**
> **순서**: ① **먼저 C-1 계측으로 백그라운드 구간의 드레인 max 와 프레임 간격을 읽어 throttle/pause 를 가른다.** ② **pause 로 판명되면 대체 벡터를 쓴다** — 도메인 리로드 반복 · 무거운 씬 로드 · 강제 GC 루프처럼 **전경에서 반복 히치를 만드는 것**.

**두 가지가 갈린다.**

1. **2~4 Hz는 백그라운드 벡터다. "전경 throttle"이라는 것은 계약에 없다.** 전경 벡터는 **도메인 리로드 반복 · 무거운 씬 로드 · 강제 GC 루프**다. 리더가 `background`를 `foreground`로 바꿔 적었고, **그 오기가 코드 주석으로 출하됐다.** 이 주석을 읽고 재현을 시도하는 사람은 계약에 없는 절차를 찾게 된다.
2. **"계약이 이미 전경 대체 벡터를 지정했다"는 명제 자체는 참이다** — ②가 그것이다. 그러므로 **리더의 결론(설정을 바꿀 필요 없음)은 여전히 성립한다.** 틀린 것은 어느 벡터가 그것인지다.

**추가로, 계약이 ②의 전제로 건 ①이 실행되지 않았다.** 계약은 "먼저 계측으로 throttle/pause를 가른다"고 순서를 지정했다. 리더는 `ProjectSettings.asset:90`의 `runInBackground: 0`에서 *"Update()는 비포커스에서 아마 돌지 않는다"*를 **추론**해 ②로 건너뛰었다. 추론은 합리적이지만 **계약이 요구한 계측이 아니다.** "①을 건너뛰고 ②를 택했다"고 적혀 있어야 한다.

**근거 (1)은 타당하다** — *"`runInBackground`는 피시험 시스템의 일부이고, 계측 편의를 위해 피시험 시스템을 바꾸면 측정이 측정이기를 그만둔다."* 이 한 문장만으로 결론이 선다. 버스트 인과와 무관하므로 qa r6가 뒤집은 논거를 되살리지 않는다. **이 근거는 옳다.**

**수정 요청 (L-12, 리더):** `03_client_impl.md` R14 §3의 *"2~4 Hz 전경 throttle"* → *"2~4 Hz **백그라운드** throttle"*, 그리고 `PeriodicStatusLog.cs:18`의 `foreground` → `background`. 이어서 *"계약이 지정한 전경 대체 벡터는 도메인 리로드 반복·무거운 씬 로드·강제 GC 루프다(SC-89 방법 칸 ②). 계약이 요구한 ①(계측으로 throttle/pause 판별)은 실행하지 않았고, `runInBackground: 0`에서 추론해 ②를 택했다"*를 덧붙인다.

### 4.2 이 근거로 무엇이 닫히고 무엇이 안 닫히는가 — **다시 판정했다. 하나도 닫히지 않는다**

**논거 교체는 증거를 만들지 않는다.** 근거의 타당성은 "왜 설정을 바꾸지 않는가"에 답할 뿐, SC-89가 요구하는 어떤 관측도 대체하지 않는다.

| SC-89 절 | 이번 라운드 판정 | 근거 |
|---|---|---|
| **(a)** 프레임당 송신 ≤ 1 ＋ 드롭·`RATE_LIMITED` 델타 0 | **미검증(증거 요건)** | 성질 테스트는 있으나 **실서버 세션 델타가 없다.** 이번 라운드에 세션 0회 |
| **(b)** `catchup_carry_forward > 0` ∧ `hard_snap == 0` ＋ 드레인 max > 1 | **미검증(증거 요건)** | qa4 §6.3의 **자명 통과 지적 그대로** — 초록이 나온 상태가 `reconcile_replayed_nonzero_total = 0` + 함선 정지였다. 증거원을 로그로 옮겨도 **히치가 실제로 있는 세션을 돌렸는가**는 그대로다. 이번 라운드에 그 세션 없음 |
| **(c)** 판정 도구가 진짜 코드를 잰다 | **닫힘 (유지)** | grep + `TickCatchUp.Plan` 직접 호출. 이번 라운드에 변화 없음 |
| **(d)** `outbound_queue_full_total` (관측) | 해당 없음 | 합격 조건 아님 |
| **(e)** 임계값 diff 0 | **닫힘 (유지)** | 이번 라운드에 `contracts/`·상수 변경 없음 |
| **(f)** 잔여 관측 2종 | **미검증(증거 요건)** | 세션 필요 |

**1002 태그 (i)(ii)(iii) — 재확인했다(재인용 아님):**

| | 판정 | 내가 이번에 뜬 증거 |
|---|---|---|
| (i) `close_code(ProtocolViolation) == 1002` | **닫힘** | `git diff --stat server/crates/gateway/src/runtime.rs` → **빈 출력**. `runtime.rs:191 SessionCloseReason::ProtocolViolation => Some(1002)` |
| (ii) `ReconnectPolicy` 양성 대조 | **닫힘** | 내 r7 실행의 nunit에서 `RealtimeClientTests.Disconnect_*` 전부 `result="Passed"`, `Disconnect_WithAnyOtherCloseCode_StillReconnects`는 `testcasecount="6"` 파라미터화로 6케이스 모두 통과. **§7a: 갈림이 실제로 갈린다** — 1002/4001/기타가 서로 다른 분기로 들어간다 |
| (iii) 태그가 실제로 찍히고 DB `close_reason`과 짝지어진다 | **안 닫힘** | 실서버 + 위반을 내는 실클라이언트가 필요하다. **리더도 R14에서 "미결로 남긴다"고 수용했다 — 이 부분은 리더가 옳다** |

---

## 5. E — 갱신된 재촬영 체크리스트가 실행 가능한가

### 5.1 grep 명령과 로그 경로 — **둘 다 동작한다. 실행해서 확인했다**

체크리스트가 지시하는 것:
```
grep 'reference_markers' client/Logs/Editor.log        # (b2)
grep 'periodic_status'   client/Logs/Editor.log > psl.txt
```

**(1) 로그 경로가 맞는가 — 맞다. Unity 자신이 그렇게 말한다.**

`%LOCALAPPDATA%\Unity\Editor\Editor.log`를 열면 48행에 이렇게 찍혀 있다:

```
*********************************
Logs moved to project-relative Editor.log file, and subsequent logs will only be
written to this file. See:
C:/WorkSpace/SpaceHistoric/client/Logs/Editor.log
```

**Unity 6이 프로젝트 상대 경로로 리디렉트하고, 그 이후 로그는 오직 그 파일에만 쓴다.** 체크리스트가 적은 경로가 정본이다.

**(2) `Debug.Log`가 그 파일에 쓰이는가 — 쓰인다. 센티널로 실측했다.**

임시 EditMode 테스트(`QaR7TempLogProbe`, `QA-R7 TEMP DEFECT` 표식)가 `Debug.Log("starfall.greybox: periodic_status tick=999999 QA-R7-LOG-PROBE-SENTINEL")`를 한 줄 내도록 하고 스위트를 돌린 뒤:

```
$ grep -c "QA-R7-LOG-PROBE-SENTINEL" client/Logs/Editor.log
1
$ grep 'periodic_status' client/Logs/Editor.log
starfall.greybox: periodic_status tick=999999 QA-R7-LOG-PROBE-SENTINEL
```

**체크리스트의 grep 명령이 글자 그대로 동작한다.** 프로브는 실행 후 삭제했다(§8).

**§7a 단언:** 이 관찰이 겨냥한 조건은 "`Debug.Log` 문자열이 그 파일에 도달한다"이고, 센티널이 **그 파일에서, 그 grep으로** 나왔으므로 조건이 실제로 발생했다. **미검증으로 남는 것:** 이 확인은 **batchmode 실행**이다. 재촬영은 **GUI Editor Play 모드**이고, 나는 Editor를 띄울 수 없다(CLAUDE.md — 에이전트는 Play 버튼을 누를 수 없다). 리디렉트 문장은 batchmode 조건부가 아니므로 GUI에도 적용될 것으로 읽히지만, **그것은 추론이지 실측이 아니다.**

**(3) `reference_markers` 토큰이 실재하는가 — 실재한다.** `GreyboxSession.cs:231` `Debug.Log("starfall.greybox: reference_markers " + …)`. 세션 시작 1회.

### 5.2 "사람 눈 없이 닫는 판독 경로"가 정말 사람 눈 없이 닫히는가 — **아직 아니다. 네 단계가 빠졌다**

| # | 빠진 것 | 왜 치명적인가 |
|---|---|---|
| **E-1** | **촬영 직후 `Editor.log`를 증거 폴더로 복사하는 단계가 없다** | Editor를 다시 열면 `Editor.log`가 `Editor-prev.log`로 밀리고, **한 번 더 열면 사라진다.** 그리고 체크리스트 1단계가 지시하는 `unity test`가 **바로 그 파일을 덮어쓴다** — 나 자신이 이 라운드에 그렇게 덮어썼다. 촬영 후 아무도 Editor를 안 열어야만 증거가 산다는 전제는 4라운드짜리 슬라이스에서 성립한 적이 없다. **`cp client/Logs/Editor.log _workspace/.../evidence/<세션>/Editor.log`를 촬영 직후 단계로 넣어야 한다.** |
| **E-2** | **기준 2의 판독 절차가 이 함선의 선회율에서 성립하지 않는다** | §2.3 — 5초 간격 × 최대 75 deg/s = 375° > 180°. `q_rel`이 부호를 뒤집는다. **판독 절차 자신이 부호 오독을 만든다.** 유지 시간 제약이나 주기 단축이 필요하다 |
| **E-3** | **판독 예제가 `attitude_y`(월드 원소)와 `vec(q_rel).y`를 뒤섞는다** | 체크리스트: *"`q_rel = conj(q1)·q2`. … 우선회 30°면 `attitude_y ≈ +258819`."* 두 문장의 주어가 다르다. **`attitude_y`는 월드 자세의 원소이고, 수평·+Z 정면에서 출발했을 때만 그 값이 된다.** qa r6 F-10이 경고한 바로 그 오독이다. 그리고 `vec(q_rel)`을 **함선 로컬 up에 투영**하는 단계가 빠져 있어, 뱅크·피치가 있는 함선에서는 월드 Y 성분이 "오른쪽으로 돌았다"를 뜻하지 않는다 |
| **E-4** | **기준 2·5-b의 유지 시간 지시가 없다** | §2.3. 5-b는 40 deg/s라 한 5초 창 안에서 시작·완료되어 **표본이 안 남을 수 있다** |

**(b2)는 닫힌다.** `grep 'reference_markers'` 한 줄이 마커 4개의 ID·거리를 주고, 그것과 `markers=` HUD 줄이 같은 세션에 있다. 여기에 사람 눈은 필요 없다.
**기준 4(경계)도 닫힌다.** `boundary_soft_crossed=true` bool은 반경을 몰라도 읽히고, `origin_distance_m` 추이가 동반 증거다.

### 5.3 기준 5-b 판독 지시가 §7b(1)을 옳게 적용했는가 — **옳게 적용했다**

체크리스트: *"`flight_assist=true` 구간에서 `attitude_*`가 수평으로 수렴하는지, 그리고 `roll≠0`(Q·E 유지) 구간에서는 수렴하지 않는지. **두 구간을 모두 만들어야 한다** — 한쪽만 있으면 §7b(1) 자명 통과다."*

**옳다.** 이것이 정확히 계약 SC-59 (d)의 *"②가 없으면 '오토레벨이 항상 돈다'는 버그를 통과시킨다"*이고, §7b(1)의 짝 규율 그대로다. **§7a 단언:** 이 지시는 "수렴했다"라는 관찰에 "수렴하지 않는 조건도 만들었다"를 짝으로 요구하므로, 초록이 무엇을 보고 켜졌는지가 정해진다.

**다만 §2.1의 누락이 여기에도 온다:** `roll≠0` 구간은 `flight_assist`의 값과 **무관하게** 수동 분기로 간다(`Roll != 0.0 || !FlightAssist`). 그래서 ② 구간에서는 `flight_assist`를 볼 필요가 없고, ① 구간에서는 `flight_assist=true` **와 `roll=0`이 동시에** 필요하다. 체크리스트는 ①에 `roll=0` 조건을 적지 않았다. (F-13에 함께)

---

## 6. F — 전체 스위트와 기준선

```
$ unity test client --mode EditMode --report-format nunit,junit \
    --output <qa-r7>/EditMode.nunit.xml --junit-output <qa-r7>/EditMode.xml
exit 0
testcasecount=243 total=243 passed=241 failed=0 inconclusive=0 skipped=2
start-time 2026-09-23 23:30:14Z
```

**리더 보고(243/241/failed=0/skipped=2)와 정확히 일치한다.**

**§7a 단언 — 이 초록이 무엇을 보고 켜졌는가:**

- **237 → 243의 +6이 전부 설명된다**(픽스처 노드 제외한 실제 케이스 수를 nunit에서 셌다): `PendingRebaseSlotTests` **5건 신규** + `PeriodicStatusLogTests` 8 → **9**(+1, `Format_FlightAssist_PrintsBothStates`) = **+6**. 나머지 픽스처는 불변(`HudTextWrap` 6, `MarkerHudLine` 5, `SnapshotRebaseBatch` 6 — qa r6 수치 그대로).
  **R14 본문은 "테스트 5건 신규"라고만 적고 6번째(`flight_assist` 양쪽 상태)를 세지 않았다.** 과소 보고이므로 결함으로 올리지 않고 기록한다.
- **건너뛴 2건은 숨은 실패가 아니다**: `LiveServerTests.Live_ThreePings_RoundTripInOrder`, `LiveServerTests.Live_ServerInitiatedClose_ReconnectsAsANewSession` — 실서버 게이트.
- **그리고 이 243이 세 가지에 대해서는 0건짜리 검출기다**: `ObserverSession` 배선(§3.1), `flight_assist` 필드 배선(§2.4), `GreyboxSession.MaybeLogPeriodicStatus` 배선(qa r6 §2.5(5), 이번에도 변화 없음). **건수는 경로가 실행됐다는 증거가 아니다.**

### md5 표 6건 대조 — **6건 전부 현재 트리와 일치한다**

R13에서 md5 2건이 무효였던 전력이 있어 전부 다시 떴다.

| 파일 | R14가 적은 md5 | 현재 트리 | |
|---|---|---|---|
| `GreyboxSession.cs` | `5b110456e76ac6e3c8ec327687a14f72` | 같음 | **일치** |
| `PeriodicStatusLog.cs` | `8d908903b5f9e71d93ac07af00ba0534` | 같음 | **일치** |
| `MarkerHudLine.cs` | `21fb3b1fa6d551255a72c01e978610bb` | 같음 | **일치** |
| `SnapshotRebaseBatch.cs` | `47e66ef800479426bcae4de45984305d` | 같음 | **일치** |
| `PendingRebaseSlot.cs` | `adaddf93b2a16354bcee1d14bd7af4db` | 같음 | **일치** |
| `ObserverSession.cs` | `d2f54c230665116a0f37e96bfabddca3` | 같음 | **일치** |

**R14가 "위 표는 R14 종료 시점 기준이다"라고 유효 범위를 명시한 것도 옳다** — qa r6 L-7이 요구한 규율을 새 표에 적용했다. (R13 표 자체의 주석은 여전히 없다 — §7 L-7 미대응.)

---

## 7. R14가 손대지 않은 qa r6 항목 — **6건, 보류 표시도 없다**

R14 본문은 5개 절로 끝나고 "남은 것"에 해당하는 절이 없다. qa r6가 올린 항목 대비:

| qa r6 항목 | R14 | |
|---|---|---|
| F-9 축 규약 주석 | R14 §1 | **대응** (단 §1.2의 새 결함) |
| F-11 `flight_assist`·`brake` | R14 §2 | **부분 대응 — `flight_assist`만. `brake`는 추가되지 않았고 언급도 없다** |
| F-2′ ObserverSession 배선 | R14 §4 | **부분 대응, 한계를 정직하게 명시**(§3.1) |
| L-9 `runInBackground` 근거 | R14 §3 | **대응** (단 §4.1의 새 오기) |
| L-2′ 재촬영 체크리스트 | R14 §5 | **부분 대응**(§5.2) |
| **F-10** `v'=q·v·conj(q)` ＋ 월드 프레임 명시 ＋ 판독 예제 | — | **미대응, 언급 없음** — §2.2가 보인 대로 **5-b·기준 2 판독의 실제 병목이 여기다** |
| **F-12** `_input != null ? … : 0` 침묵 센티널 | — | **미대응.** R14가 같은 패턴으로 14번째 필드를 추가했다(§2.5) |
| **L-8** "필드가 늘어나면 조용히 잘리지 않는다" 과장 | — | **미대응, 언급 없음** |
| **L-7** R13 md5 4건의 유효 시점 주석 | — | **미대응**(새 표에는 적용, 옛 표에는 미적용) |
| **L-10** `sc59-v2/13-correction-leader-2.md` ＋ R13 "남은 것"에 5-b 한 줄 | — | **미대응.** `sc59-v2/`에 13번 파일이 **없고**, R13 "남은 것"(`:1560-1568`)에 5-b 문장이 **없다** |
| **L-11** R13 F-1 "RED 1건" → 실제 2건 | — | **미대응.** `:1483`이 여전히 "1건 실패" |

**`brake` 누락은 판정에 영향이 없다** — 5-b는 `flight_assist`·`roll`·`attitude_*`로 닫히고 브레이크는 감쇠(`ShipIntegrator.cs:170`)에만 걸린다. 그러나 **요청을 절반만 이행하고 나머지 절반을 언급하지 않는 것**은 다음 라운드가 그것을 잊게 만든다. (L-13)

---

## 8. §0.12 준수 — 결함 주입과 원복

주입 4회, 전부 스크래치패드 백업 → 주입 → 전체 스위트 → 백업에서 복원 → md5 재확인. 표식 `QA-R7 TEMP DEFECT`.

```
기준선(내가 뜬 것, 주입 전):
  GreyboxSession.cs      5b110456e76ac6e3c8ec327687a14f72
  PeriodicStatusLog.cs   8d908903b5f9e71d93ac07af00ba0534
  MarkerHudLine.cs       21fb3b1fa6d551255a72c01e978610bb
  SnapshotRebaseBatch.cs 47e66ef800479426bcae4de45984305d
  PendingRebaseSlot.cs   adaddf93b2a16354bcee1d14bd7af4db
  ObserverSession.cs     d2f54c230665116a0f37e96bfabddca3

종료 후: 6개 전부 위 값과 일치 (§6 표)
$ grep -rn "QA-R7 TEMP DEFECT" client/ tools/ tests/ | wc -l   →  0
$ git status --porcelain client/Assets/_Project/Tests/EditMode/  → QaR7TempLogProbe.cs 없음
```

`docker compose down -v` **미사용**(어떤 형태로도), 서버 하드 킬 없음, golden 재생 파일 미변경, 커밋 없음, `_workspace/` 삭제 없음.

---

## 9. 역방향 게이트 (G)

```
$ python tests/e2e/check_contract_items.py --selftest
OK   위반 — 리포트가 SC-87을 판정했는데 계약에 없다 (p1-01 실제 사례) :: 위반=[87] 기대=[87]
OK   깨끗 — 판정된 것이 전부 계약에 있다 :: 위반=[] 기대=[]
OK   순방향은 위반이 아니다 :: 위반=[] 기대=[]
OK   산문 속 언급은 세지 않는다 :: 위반=[] 기대=[]
OK   리포트 여러 개를 합쳐 본다 :: 위반=[87] 기대=[87]
selftest: PASS  케이스=5

$ python tests/e2e/check_contract_items.py \
    --contract .../02_sprint_contract.md --report .../07_qa_report_r7.md
계약 항목: 89
리포트가 판정표 행으로 다룬 항목: 4
계약에 있으나 이 리포트들에 안 나온 항목: 85 (위반 아님 — 대기·블록 분할)
위반 없음
```

**§7a 단언 — "0건 파싱 후 위반 0"이 아니다.**
**판정 행이 실제로 4건 파싱됐다**(SC-46·SC-56·SC-59·SC-89 — §0.2 표의 네 행). 그리고 **양성 대조가 실제로 빨간불을 켠다**: selftest 케이스 1이 SC-87을 위반으로 **검출**하고, 케이스 2가 깨끗한 입력에서 **검출하지 않는다** — 두 방향이 갈린다. 파서가 죽어 있으면 케이스 1이 `위반=[]`로 떨어져 FAIL이 난다.
F-13·F-14·F-15·F-2′·F-12′·L-12·L-13은 **SC 번호가 없는 계약 외 항목**이므로 게이트 대상이 아니다.

---

## 10. 결함·수정 요청 목록

### 10.1 client

| # | 위치 | 문제 | 기대 |
|---|---|---|---|
| **F-13** | `PeriodicStatusLog.cs:85-87` ＋ `03_client_impl.md:1578` | **정정된 문장이 여전히 자기모순이다.** `right-hand-rule about +Z`는 이 **왼손 좌표계**(ADR-0009 §1 `:28`·`:33`)에서 틀렸고, 그 법칙을 그대로 적용하면 `+X → -Y`(drops)가 나와 같은 문장의 결론과 반대다(§1.2). **결론은 맞다** | ADR-0009의 단어를 쓴다: *"Unity's positive rotation direction about ship-local +Z — a LEFT-handed basis, so the left-hand rule, NOT the right-hand rule"* ＋ 수치 검증 출처. 체크리스트 5-b ① 조건에 `roll=0`도 명시 |
| **F-15** | `PeriodicStatusLogTests.cs:14-46, 208-230` | **R14가 5-b를 닫으려고 추가한 테스트가 §7b(1) 자명 통과다.** `Sample()`이 `boundarySoftCrossed`·`flightAssist`를 **둘 다 true**, 반대 케이스는 **둘 다 false**로 두어 두 bool이 완전 상관이다. `flight_assist=`를 `BoundarySoftCrossed`에서 뽑아도 **243건 전원 초록**(§2.4 실측) | 두 bool이 **엇갈리는** 조합을 최소 하나 넣는다. 정수 12필드는 이미 전부 다른 값이라 이 병이 없다 — bool 둘만 예외 |
| **F-14** | `GreyboxSession.cs:653` (`PeriodicStatusLogIntervalSeconds = 5.0`) | **5초 주기가 기준 2의 판독을 원리적으로 깬다.** 최대 선회율 75 deg/s × 5 s = 375° > 180° → `q_rel`이 부호를 뒤집는다. 5-b(40 deg/s)는 한 창 안에서 시작·완료되어 표본이 안 남을 수 있다(§2.3) | 주기를 **1.0초**로 낮춘다(60초 세션에 60줄). 또는 체크리스트에 유지 시간 제약과 그 근거를 적는다. 전자를 권한다 |
| **F-10** (이월) | `PeriodicStatusLog.cs` 필드 블록 | **미대응.** 회전 적용 형태(`v'=q·(v,0)·conj(q)`, Hamilton)와 **`attitude_*`가 월드 프레임**이라는 사실이 없어, 5-b·기준 2 유도의 실제 병목이 여기다(§2.2) | 두 줄 ＋ 판독 예제. 예제는 `vec(q_rel)`을 주어로 쓴다(`attitude_y` 원소가 아니라) |
| **F-2′** (이월) | `ObserverSession.cs:270`, `:124` | **배선 테스트 0건 그대로.** `ApplyPendingRebase()` 삭제 → 243/241/failed=0(§3.1 실측). `OnSessionReady`의 `_pendingRebase.Clear()`도 같은 공백이고, 그것이 `:230`의 무검사 `_controller` 역참조를 막는 유일한 장치다 | 가짜 전송으로 `ObserverSession`을 구동해 tick 100·102·101 한 배치 → **`WriteRow` 1회, tick 102** 단언 ＋ 세션 재시작에서 옛 스냅샷이 새 컨트롤러에 닿지 않음 |
| **F-12′** | `GreyboxSession.cs:681-682` 주석 | *"the log and the HUD **can never disagree**"*가 참이 아니다 — `_input == null`이면 HUD는 `"no input sampler"`, 로그는 `flight_assist=false`·`roll=0`이다(§2.5). F-12 패턴을 5-b의 유일한 어시스트 신호에 복사했다 | 문장을 *"whenever the sampler exists"*로 좁히거나, 14필드에도 `n/a` 규율을 적용 |
| L-8 (이월) | `HudTextWrap.cs` 헤더 | **미대응.** 공백 없는 110자 초과 토큰 하나면 "조용히 잘리지 않는다"가 깨진다 | 문장을 *"모든 필드가 공백으로 구분되는 한"*으로 좁힌다 |

### 10.2 리더 (문서)

| # | 위치 | 문제 | 기대 |
|---|---|---|---|
| **L-12** | `03_client_impl.md:1598-1600` ＋ **`PeriodicStatusLog.cs:18`(출하 코드)** | *"2~4 Hz **전경** throttle"* / `"a 2-4 Hz **foreground** throttle"`이 **틀렸다.** 계약 SC-89 방법 칸의 2~4 Hz는 **백그라운드** 벡터이고, 전경 대체 벡터는 **도메인 리로드·무거운 씬 로드·강제 GC 루프**다(§4.1). **결론은 옳고 사실 진술이 틀렸다** — qa r6의 L-9와 같은 형태가 한 칸 옮겨 재발했다 | `foreground` → `background`. ＋ 전경 벡터가 무엇인지 명시. ＋ **계약이 ②의 전제로 건 ①(계측으로 throttle/pause 판별)을 실행하지 않고 `runInBackground: 0`에서 추론해 건너뛰었다**를 명시 |
| **L-13** | `03_client_impl.md` R14 | **qa r6 항목 6건(F-10·F-12·L-7·L-8·L-10·L-11)과 F-11의 `brake` 절반에 손대지 않았고, 보류 표시도 없다**(§7). 특히 **L-10의 `sc59-v2/13-correction-leader-2.md`가 존재하지 않고**, R13 "남은 것"(`:1560-1568`)에 5-b가 막혀 있다는 문장이 없다 | R14 끝에 "남은 것" 절을 만들어 미대응 항목을 **이름으로** 나열한다. 묵시적 누락과 의식적 보류를 구분할 수 있어야 다음 라운드가 잊지 않는다 |
| **L-14** | `evidence/R4-B6/sc59/09-reshoot-checklist.md` 갱신 절 | **촬영 직후 `Editor.log`를 증거 폴더로 복사하는 단계가 없다.** Editor 재실행 1회로 `Editor-prev.log`로 밀리고 2회로 사라지며, **체크리스트 1단계의 `unity test` 자신이 그 파일을 덮어쓴다**(§5.2 E-1, 이번 라운드에 내가 실제로 덮어썼다) | 촬영 직후 단계로 `cp client/Logs/Editor.log <evidence>/Editor.log` ＋ 그 파일의 md5를 증거 색인에 |
| L-11 (이월) | `03_client_impl.md:1483` | **미대응.** R13 F-1 "RED 1건"은 실제 2건 | 2건으로 정정 |

### 10.3 architect

- **(이월) 계약 SC-59 (a)에 롤 방향이 없다.** §1.1의 실측상 `E → 양의 롤 → 우현 날개 상승 = 좌측 뱅크`이고, 이 슬라이스에서 **가장 덜 검증된 축**이다. 이번 라운드에 변화 없음.
- **(이월) §0.3에 "제품 책임자 판정" 칸이 없다.** 이번 라운드에 변화 없음.
- **(신규) 계약 SC-59 판독을 로그로 닫으려면 로그 주기가 계약 관심사다.** §2.3 — 5초 주기와 75 deg/s 선회율의 곱이 `q_rel` 판독을 깬다. F-14를 client 재량으로 둘지, 계약에 "판독 주기는 최대 선회율 × 주기 < 180°를 만족해야 한다"를 명시할지 판단이 필요하다.

---

## 11. 실행 증거 색인

| 산출물 | 경로 | 요약 |
|---|---|---|
| EditMode GREEN (판정용) | `unity-tests-qa-r7/EditMode.nunit.xml`, `EditMode.xml` | 243/241/0/2, exit 0 |
| **F-2 영검출 재현** (`ApplyPendingRebase()` 삭제) | `unity-tests-qa-r7/EditMode.RED-F2-null.nunit.xml` | **failed=0 — 초록과 동일** |
| RED `TryTake`의 `Clear()` 제거 | `unity-tests-qa-r7/EditMode.RED-TryTakeClear.nunit.xml` | failed=1, `TryTake_EmptiesTheSlot…` |
| RED stale payload (`_confirmed` 미갱신) | `unity-tests-qa-r7/EditMode.RED-StalePayload.nunit.xml` | failed=1, `OutOfOrderBatch…` |
| **PROBE `flight_assist` ← `BoundarySoftCrossed`** | `unity-tests-qa-r7/EditMode.PROBE-AssistFromBoundary.nunit.xml` | **failed=0 — 자명 통과 증명** |
| 롤 부호 수치 유도 | §1.1 인라인 (적분기 5단계 재현) | +X → 월드 +Y |
| `Editor.log` 경로·grep 실측 | §5.1 인라인 | Unity 리디렉트 문장 ＋ 센티널 grep 1건 |
| 역방향 게이트 | §9 인라인 | selftest PASS 5케이스, 판정 행 **4건 파싱** |

---

## 12. 라운드 7 결론

- **PASS 1건**(SC-46) — 실행 증거 있음.
- **통과 1건**(SC-59) — 제품 책임자 판정, qa 판정 아님.
- **미검증(증거 요건) 2건**(SC-56·SC-89) — 비-통과. **슬라이스 종료를 막는다. 이번 라운드에도 실서버 세션이 0회다.**
- **R14 5건 중: PASS 1(§4 `PendingRebaseSlot`) / 부분 PASS 4.**
- **신규 결함 5건**(F-13·F-14·F-15·F-12′ client, L-12·L-14 리더) ＋ **이월 미대응 7건**.

**리더가 틀린 곳 셋을 명시한다.**

1. **축 규약 문장이 여전히 자기모순이다.** 결론(`right wing RISES`)은 내가 독립적으로 유도해 맞음을 확인했지만, **같은 문장의 `right-hand-rule about +Z`는 이 왼손 좌표계에서 틀렸고 반대 결론을 준다.** ADR-0009가 *"오른손 좌표계(수학 관례)를 쓰지 않는다"*를 굵게 적어 둔 바로 그 함정이다. **§7a를 설교하면서 §7a를 어긴 자리를 고치는 수정문이, 같은 자리에서 다시 어겼다.**
2. **`flight_assist`를 닫으려고 추가한 테스트가 자명 통과다.** `flight_assist=`를 `boundary_soft_crossed`에서 뽑아도 243건 전원 초록이다(실측). **필드를 추가하면서 그 필드가 올바른 원천에서 오는지를 재는 관찰은 만들지 않았다** — 이 슬라이스가 다섯 번째로 걸린 같은 형태다.
3. **`runInBackground` 논거 (2)의 사실 진술이 틀렸고, 그 오기가 출하 코드 주석에 들어갔다.** 계약의 2~4 Hz는 백그라운드 벡터다. **결론은 옳고 근거의 사실이 틀렸다 — qa r6의 L-9와 정확히 같은 형태가 한 칸 옮겨 재발했다.**

**리더가 옳았던 곳도 명시한다.**

- **`PendingRebaseSlot`의 자기 평가가 정확하다.** *"상태 기계는 닫혔고 배선은 미검증"*은 내가 호출 삭제를 재현해 확인했다 — **후하지도 박하지도 않다.** 이 슬라이스에서 리더가 "닫혔다"의 경계를 스스로 정확히 그은 첫 사례다.
- **RED 1건 보고가 정확했고**, 내가 추가로 돌린 stale-payload 퇴화도 의도한 테스트가 잡는다.
- **md5 6건이 전부 현재 트리와 일치하고**, "이 표는 R14 종료 시점 기준"이라는 유효 범위 단서까지 붙였다 — qa r6 L-7의 규율을 새 표에 적용했다.
- **`Editor.log` 경로가 맞다.** Unity 자신이 프로젝트 상대 경로로 리디렉트한다고 찍고 있고, grep 명령이 글자 그대로 동작한다(센티널로 실측).
- **5-b 판독 지시의 "두 구간을 모두 만들어야 한다"가 §7b(1)을 옳게 적용했다.**
- **1002 (iii)이 대체 증거로 닫히지 않는다는 qa 판정을 수용하고 미결로 남겼다.**

**하나만 고른다면 F-15다.** R14의 존재 이유가 *"`flight_assist` 한 필드로 5-b를 연다"*인데, **그 필드가 올바른 값에서 오는지를 재는 테스트가 하나도 없다.** 243건 전원 초록인 상태에서 그 필드는 `boundary_soft_crossed`의 사본일 수 있고, 그러면 자유 비행 로그는 전부 `flight_assist=false`가 되어 **5-b는 F-11 이전과 똑같이 닫히지 않는다 — 다만 이번엔 아무도 그것을 모른다.** 세 줄짜리 수정이다.

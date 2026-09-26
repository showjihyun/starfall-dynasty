# QA 평가 리포트 r20 — SC-60 (a)(b)(c)(d) 판정 (EditMode)

- 라운드: R20. 실행은 **리더**가 했고(`unity test client --mode EditMode`), **판정은 qa 가 한다.**
- 증거: `evidence/R20-unity/test-results.xml` (복사본) · `evidence/R20-unity/git-state.txt`
- **나는 Unity 를 열지 않았고 테스트를 돌리지 않았다.** XML 을 직접 파싱해 판정했다.

---

## §7a — 실행해 확인한 것과 남이 준 것을 그대로 받은 것

| 사실 | 어떻게 |
|---|---|
| 세 스위트가 **5 / 6 / 4 로 전부 Passed** | **내가 XML 을 파싱해 확인**(리더의 요약을 그대로 받지 않았다) |
| 루트 `total=323 passed=321 failed=0 skipped=2`, `start 13:24:31Z → end 13:24:32Z` | 같은 파싱 |
| **건너뛴 2건이 무엇인가** — `Live_ServerInitiatedClose_ReconnectsAsANewSession` · `Live_ThreePings_RoundTripInOrder` | 같은 파싱. **둘 다 내 세 스위트 밖이고 `STARFALL_LIVE_TESTS` 게이트다** — SC-60 판정에 영향 없다 |
| `HEAD = 443ab3c` · **워킹트리 더럽다(18줄)** | **실행**: `git rev-parse HEAD` · `git status --porcelain` |
| `runInBackground` **1 → 0** 이 이 실행에 포함돼 있다 | **실행**: `git diff` |
| **`unity test` 가 exit 0 과 함께 콘솔에 결과를 한 줄도 안 찍는다** | **리더가 실행으로 확인한 것을 받았다** — 내가 재현하지 않았다(Unity 를 열 수 없다) |
| 계약이 지명한 개수(5·6·4)와 실제가 일치 | 내가 **읽어서** 센 것(R19)을 **실행이** 확인했다 |

---

## 1. 판정 — **절마다 verdict 하나** (§7b 규칙 5)

| 절 | verdict | 분모(§0.4) — **이것이 만족됐는가** | 증거 |
|---|---|---|---|
| **SC-60 (a)** | **PASS** (유보 1건, §2) | `OnSnapshot_NewShipId_CreatesABuffer` **Passed** ＋ `GetDisplay_BetweenTwoSamples_Interpolates` **Passed** — **두 이름이 실행 목록에 있다** | XML |
| **SC-60 (b)** | **PASS** | `Slerp_AtQuarterPoint_DiffersFromNormalizedLerp_ForLargeAngle` **Passed** — 계약이 요구한 **비대칭 지점(`t=0.25`)** 그대로다. 계약이 금지한 `t=0.5` 는 **nlerp 구현도 통과시키는 대칭점**이고, 이 테스트는 그 함정을 피해 있다 | XML |
| **SC-60 (c)** | **PASS** | **둘 다 Passed** — `GetDisplay_PastLatestSample_WithinCap_Extrapolates`(**조건 발생**: 외삽 구간이 생겼다) ＋ `GetDisplay_PastExtrapolationCap_FreezesAndZeroesVelocity`(**상한**: 그 뒤 정지) | XML |
| **SC-60 (d)** | **PASS** | **둘 다 Passed** — `OnSnapshot_LingeringShip_IsKept_NotRemoved` ＋ `OnSnapshot_ShipAbsentFromLatestSnapshot_IsRemovedImmediately` | XML |
| **SC-60 (e)** | **대기** | Play 세션의 로그 한 줄(`remote interpolation delay = 200 ms = 4 ticks`) | 아직 |

### 1.1 (c) 를 "둘 다" 로 판정한 이유 — **상한 하나로는 자명 통과다**

`FreezesAndZeroesVelocity` **하나만** 보면 **외삽을 아예 하지 않는 구현도 초록이다** —
그 구현은 상한을 영원히 넘지 않는다. **상한은 *"넘지 않았다"* 를 말하고 *"거기까지 갔다"* 를
말하지 않는다.** 짝인 `WithinCap_Extrapolates` 가 **조건 발생**을 맡는다.

**§7b 규칙 1 의 두 짝(조건 발생 + 검출기 생존)이 테스트 이름 두 개로 나뉘어 있는 형태**이고,
(b)(d) 도 같은 구조다. **그래서 판정 근거를 "스위트가 초록" 이 아니라 "이 두 이름이 실행
목록에 있다" 로 적었다.**

### 1.2 ⚠ 이 실행에서 만난 규칙 9 형태 — **exit 0 은 판정 근거가 아니다**

리더 실행에서 `unity test` 가 **exit 0 을 내고 stdout 에 prewarm 두 줄만** 찍었다. 결과는
`test-results.xml` 에만 있다.

> **종료 코드는 *"실패가 없었다"* 를 말하고 *"무엇이 돌았다"* 를 말하지 않는다.**
> **루트의 `failed=0` 도 같다** — 필터가 잘못 걸려 **0건이 돌아도 참이다.**

**§0.4 의 *"빈 집합에 대한 전칭명제는 참이다"* 와 같은 뿌리이고, 부정문 · 상한 · 종료 코드가
전부 그 가족이다.** 절차서 §1 에 경고 상자로 박았다. **그래서 이 리포트의 판정 근거는
exit 코드가 아니라 스위트별 `total`·`result` 와 테스트 이름 15개다.**

---

## 2. (a) 의 유보 — **유지한다. 실행이 이 유보를 바꾸지 않는다**

EditMode 는 *"버퍼가 생기고 보간된다"* 를 단언하지만 **"화면에 그려진다"** 는 단언하지 못한다.
**계약 SC-60 의 방법 칸이 행 전체를 EditMode 로 지명했으므로 요구를 더하지 않는다** —
리더가 EditMode 를 돌렸다는 사실도 이 유보를 바꾸지 않는다(층이 달라진 게 아니다).

**(a) 를 PASS 로 적되 그 PASS 가 무엇을 덮는지 적는다**: **원격 함선의 등록·보간 로직**이다.
**화면 표시는 이 판정의 범위가 아니다.** architect 에게 넘긴 관찰 그대로다(절차서 §6-1).

---

## 3. 증거와 규칙 9 이행

| 항목 | 값 |
|---|---|
| `git rev-parse HEAD` | **`443ab3c129aaa395691c32029fc981f8eaa53107`** |
| `git status --porcelain` | **더럽다 — 18줄.** 블록 1·S-R25·계약 개정·내 도구 변경이 미커밋이다. **그대로 적는다** |
| **`runInBackground`** | **`1 → 0`** 이 이 실행에 **포함돼 있다**(`git diff` 확인). **EditMode 에 영향이 없을 것으로 보지만 없다는 것을 아무도 재지 않았다** — 그래서 **판정 근거로 쓰지 않고 실행 조건으로 적는다** |
| 건너뛴 2건 | `Live_*` 둘, `STARFALL_LIVE_TESTS` 게이트. **세 스위트 밖** |

**더러운 트리에서 내린 판정의 의미**: 이 verdict 는 **`443ab3c` + 그 18줄의 워킹트리**에 대한
것이다. 커밋 해시 하나로는 재현되지 않는다 — **규칙 9 의 이행 조항이 요구하는 것이 정확히
이 문장이다.**

---

## 4. 요약

| 항목 | verdict |
|---|---|
| SC-60 (a) | **PASS** (범위: 등록·보간 로직. 화면 표시는 아니다) |
| SC-60 (b) | **PASS** |
| SC-60 (c) | **PASS** (조건 발생 + 상한 **둘 다**) |
| SC-60 (d) | **PASS** |
| SC-60 (e) | **대기** — Play 세션 |
| SC-56 (e) | **대기** — Play 세션 |

**막고 있는 게이트가 둘 줄었다.** 남은 것은 **Play 한 번**이고, 그 한 번의 실질 목표는
**SC-56 (e)** 다(SC-60 (e) 는 접속 시 자동으로 찍힌다).

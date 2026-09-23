# client 확인 (p1-01 스프린트 계약)

- 날짜: 2026-09-20
- 입력: `02_sprint_contract.md`(SC-41~60, §0.5, §2, §4, §8 쟁점 6·7·8·9), 확정 스펙·ADR-0009~0012
- **이번 턴은 답변만이다.** 코드를 쓰지 않았다. 재측정은 스크래치 사본(`scratchpad/p1/Gen2.cs`, `p1/probe`)으로만 했다.
- 반영 확인 감사: R1(26/20), R2(캐스케이드 3단), R3(`ContractTypes.cs` 예외), R4(디스패치 우회), R5·R7(단위·비교 지점), **R6(slerp t=0.25 + 60°)** 전부 계약에 들어와 있다.

---

## 0. 먼저 — 계약이 쓴 숫자 2개가 지금 틀렸다 (B-1 반영 이후 재측정)

내 검토 이후 **계약이 바뀌었다**(B-1: `ShipState`에 `angular_velocity_roll_mdeg_s` 추가, 반례 33 → 34). 그래서 확장 생성기로 **현재 `contracts/` 트리**를 다시 생성하고 34건을 전수 재측정했다.

### ① SC-43·AC-10(c)의 "17개 속성" → **18개**

```
ShipState property count: 18
ships property type: ...WorldSnapshotMessage+WorldSnapshotPayload+ShipState[]  isArray=True
required 18: ship_id, actor_id, ship_class_id, presence,
             position_{x,y,z}_mm, velocity_{x,y,z}_mm_s,
             orientation_{x,y,z,w}_micro,
             angular_velocity_{x,y,z}_mdeg_s, angular_velocity_roll_mdeg_s
```

B-1이 `angular_velocity_roll_mdeg_s`를 추가하면서 17 → 18이 됐다. **SC-43과 AC-10(c)의 17을 18로 고쳐 달라.** 그대로 두면 SC-43이 반드시 실패한다.

이 필드가 바로 **SC-55가 "합에서 분해하면 실패한다"고 적은 그 필드**다 — `ω_roll`을 월드 각속도 3성분에서 분해하지 않고 별도 필드로 받는다. 내 검토 §계획에서 "조용한 함정"으로 적었던 분해 문제를 계약이 없애 준 것이라 환영한다.

### ② SC-48·§0.5의 "거부 17 / 통과 10 / 계층 없음 7" → **거부 18 / 통과 10 / 계층 없음 6**

현재 34건 전수 실측:

```
counter-examples: 34 | C# rejects: 18 | C# accepts: 10 | no C# layer: 6
  REJECTS WORLD_SNAPSHOT/invalid/angular-velocity-roll-missing.json
          :: Required property 'angular_velocity_roll_mdeg_s' not found in JSON.
```

- 34번째 반례는 **`WORLD_SNAPSHOT/invalid/angular-velocity-roll-missing.json`** — 데이터 타입이 아니라 **메시지 타입**이고, C#이 **거부**한다. 그래서 거부가 17 → **18**로 간다.
- "계층 없음"은 여전히 **6**이다. C# DTO가 없는 반례는 데이터 3종의 것뿐이고 각 2건씩이다:
  `SHIP_CLASS/invalid/{movement-unknown-field, turn-gain-zero}`, `STAR_SYSTEM/invalid/{hard-radius-above-ceiling, no-spawn-points}`, `SYNC_TUNING/invalid/{integrator-is-not-a-tunable, snapshot-hz-zero}`.
- §0.5의 "**계층 없음(데이터 7종) 7**"은 두 군데가 어긋난다: 데이터 타입은 **3종**(7종이 아니다)이고 그 반례는 **6건**(7건이 아니다). 7을 쓴 것은 34 − 17 − 10 = 7로 역산했기 때문으로 보이는데, 틀린 것은 17 쪽이다.
- **18 + 10 + 6 = 34** ✓

유효 쪽은 변동 없음: `valid 26 | DTO 있는 것 20 | round-tripped 20, problems 0`. SC-47은 그대로 맞다.

**요청**: §0.5 ③행과 SC-48의 상수를 **18 / 10 / 6**으로, SC-43·AC-10(c)를 **18**로. 둘 다 내가 지금 실측한 값이고, 구현자가 상수로 박을 숫자다.

---

## 쟁점 ⑥ — 두 번째 관측자: **봇으로 대체하지 마라. 그러면 SC-65의 부호가 뒤집힌다**

### 왜 봇 대체가 위험한가 (숫자로)

SC-65는 "차이 벡터를 A의 속도 방향에 투영한 **부호 있는 값**이 뒤쪽으로 28 ± 12 m"다. 그 28 m는 **B가 보간 지연 200 ms만큼 과거를 그리기 때문에** 생긴다.

봇을 B로 쓰면 **보간이 사라진다.** 봇이 가진 것은 스냅샷의 원시 값, 즉 **그 tick의 서버 진실**이다. 반대편 A는 **예측**으로 마지막 스냅샷보다 앞서 있다. 그래서 차이는

- 보간 B: `A의 화면 − B의 화면` = **뒤쪽 +28 m** (B가 과거를 본다)
- 봇 B: `A의 화면 − 서버 진실` = **앞쪽으로 0 ~ 한 스냅샷 주기분** (A가 미래를 본다)

**부호가 반대이고 크기도 한 자릿수 다르다.** SC-65는 부호를 판정의 핵심으로 삼았으므로, 봇으로 대체하면 "앞쪽으로 어긋나면 외삽 과다"라는 판정 규칙이 **정상 동작을 버그로 읽는다.** 항목이 약해지는 정도가 아니라 **거꾸로 채점한다.**

### 대안 — Editor 인스턴스 2개가 **필요 없다**

SC-64·65가 B에게서 필요로 하는 것은 **화면 픽셀이 아니라 보간 파이프라인**이다. 그리고 C5 설계상 보간 코드는 **Unity 타입이 없는 순수 C#**이다(C3·C5 지시: "MonoBehaviour 없음, Unity 타입 없음"). 그러면 **한 Unity 프로세스 안에서 독립 세션 2개 + 독립 관측자 파이프라인 2개**를 돌릴 수 있다.

| | A | B |
|---|---|---|
| WebSocket 세션 | 별개 | 별개 (다른 `actor_id`, 다른 토큰 주체) |
| `controlled_ship_id` | 자기 함선 | 자기 함선(다른 함선) |
| A 함선을 보는 경로 | **예측** | **보간**(200 ms 지연, slerp 포함) |
| 스냅샷 도착 타이밍 | 자기 소켓 | 자기 소켓 |
| CSV | 자기 것 | 자기 것 |

이것은 SC-64·65가 재는 성질(예측 대 보간의 지연차)에 대해 **두 Editor와 완전히 동등하다.** 재현되지 않는 것은 프로세스 격리(GC·렌더 타이밍)뿐인데, SC-64·65는 그것을 재지 않는다.

- 토큰 주체는 p0-02에서 쓴 `STARFALL_DEV_ACTOR_SUBJECT` 환경 변수로 갈라진다. B용 주체를 하나 더 정하면 된다(예: `...-000000000002`). **봇 30개 주체와의 disjoint는 그대로 확인 가능하다.**
- 구동 형태는 p0-02에서 **실행 확인한** 헤드리스 경로를 그대로 쓴다: `unity run client -- -executeMethod ...`(`-quit`가 붙으므로 메인 스레드를 잡고 있는 블로킹 루프여야 한다 — 인수인계 §2에 적었다).
- A를 사람이 조종해야 하면(SC-59와 같이 돌릴 때) **A는 대화형 Editor, B는 같은 프로세스 안의 두 번째 세션**으로 둔다. 인스턴스는 여전히 1개다.

### 그래도 프로세스를 갈라야 한다면 (차선)

Unity의 프로젝트 잠금은 **경로 단위**다(p0-02 실측: `"…\client"의 프로젝트가 실행 중인 에디터(PID 35992)에서 이미 열려 있습니다`). 두 번째 경로를 만들면 두 인스턴스가 뜬다 — `Assets`·`Packages`·`ProjectSettings`를 디렉토리 정션으로 건 사본을 레포 **밖**에 두는 방식이다.
**단, 나는 이 경로를 실행해 보지 않았다.** 추측으로 절차를 적지 않겠다 — 필요해지면 내가 먼저 실행해 확인한 뒤 적겠다. 위 "한 프로세스 두 세션"이 더 싸고 확실하므로 그쪽을 먼저 쓰기를 권한다.

### 정리 (⑥에 대한 답)

1. **권장**: 한 Unity 프로세스, 세션 2개, 관측자 파이프라인 2개. SC-64·65의 의미가 온전히 보존된다. 추가 패키지 없음(MPPM·Recorder 둘 다 미설치 확인).
2. **봇 대체는 허용하지 말라.** 굳이 해야 한다면 SC-65를 **다른 항목으로 바꿔야 한다**: 기대값을 "뒤쪽 28 ± 12 m"가 아니라 **"앞쪽 0 ~ +7 m(예측 선행)"**으로, 근거도 "보간 지연"이 아니라 "예측 선행"으로. 같은 이름으로 두면 채점이 거꾸로 된다.
3. SC-64(정지 비교 ≤ 2 m)는 **봇으로도 의미가 산다** — 정지 상태에서는 예측도 보간도 같은 값을 내므로 두 경로의 차이가 0에 수렴한다. ⑥이 막히면 **SC-64만 봇으로 살리고 SC-65는 미검증(환경)으로** 두는 편이, 뒤집힌 부호로 PASS를 찍는 것보다 정직하다.

---

## 쟁점 ⑦ — 프로파일러 "스냅샷 1건당" 분리 방법

**2단으로 낸다. 1단이 정본 숫자, 2단이 Editor 안에서의 확인이다.**

### 1단 (정본, 이미 실행됨)

p0-02에서 미뤄진 U-16을 **검토 단계에서 이미 쟀다.** Unity 동봉 컴파일러로 빌드해 **Unity 동봉 `mono.exe`**에서, Unity 동봉 Newtonsoft 13.0.2로, 31척 16,347 B 스냅샷을 200회 반복:

| 경로 | 1건당 | 10 Hz 환산 | 200건당 gen0 GC |
|------|---:|---:|---:|
| `JObject` 트리만 | 0.401 ms | 4.01 ms/s | 7 |
| **트리 + `ToObject`(p0-02 경로)** | **0.656 ms** | 6.56 ms/s | **8** |
| **문자열에서 직접 타입 역직렬화(R4 채택안)** | **0.265 ms** | 2.65 ms/s | **1** |
| UTF-8 디코드만(바닥값) | 0.025 ms | 0.25 ms/s | 0 |

이 방식의 장점: **반복 가능하고 격리돼 있다.** 렌더·에디터 오버헤드가 섞이지 않고, N=200이라 단발 스파이크에 흔들리지 않는다. SC-58의 "건당·초당" 숫자는 이 표에서 바로 나온다.

### 2단 (Editor 안 확인, SC-58의 실행 항목)

1. 스냅샷 처리 전 구간을 **`ProfilerMarker`** 하나로 감싼다 — 이름을 `Starfall.Snapshot.Handle`로 고정한다(QA가 Profiler에서 찾을 문자열이다). `ProfilerMarker`·`ProfilerRecorder`·`Profiler.GetTotalAllocatedMemoryLong`이 Unity 6 참조 어셈블리에 **존재함을 확인했다.**
2. **Deep Profile을 끈다.** 켜면 할당량이 부풀어 숫자가 무의미해진다.
3. Profiler 창 → Hierarchy → 검색창에 `Starfall.Snapshot.Handle`. 스냅샷은 10 Hz, 렌더는 60~144 fps이므로 **6~14프레임 중 1프레임에만 이 마커가 있다.** 마커가 **있는 프레임**의 `GC Alloc` 열을 읽으면 그것이 곧 **스냅샷 1건당 할당**이다. 프레임을 고르는 것이 곧 분리다 — 별도 계산이 필요 없다.
4. 초당 = 건당 × `snapshot_hz`(10). 계산식을 리포트에 같이 적는다.
5. 보조로 `ProfilerRecorder`를 그 마커에 붙여 `Count`(프레임당 호출 수)를 HUD에 띄우면, **"이 프레임에 스냅샷이 있었다"가 화면에서 보인다** — 프레임을 고르는 작업이 눈으로 확인된다.

**경계**: 1단과 2단의 절대값은 다를 것이다(Editor 오버헤드, 다른 할당원 혼입). **둘 다 적고, 어느 쪽이 정본인지 명시한다.** 상대 비교(R4의 경로 2 대 3)는 1단이 더 믿을 만하고, "실제 클라이언트에서 얼마나 나오는가"는 2단이 답한다.

---

## 쟁점 ⑧ — 육안 녹화 보관 형식

**형식: mp4 4개 + 결정적 순간 스크린샷 4장 + 한 줄 설명 텍스트.**

| 항목 | 내용 |
|---|---|
| 개수 | SC-59의 (a)(b)(c)(d) **각각 1클립**. (a)는 전방·우선회·상방을 한 클립에 이어 담는다 |
| 길이·해상도 | 10~25초, 1280×720 이상, 30 fps |
| 형식 | **mp4(H.264)**. gif는 색·프레임이 깨져 부호 판정에 부적합하고 용량도 더 크다 |
| 도구 | **ffmpeg 7.1.1이 이 PC에 있고 `gdigrab` 지원을 확인했다.** Windows Game Bar(`Win+Alt+R` → `Videos/Captures`)도 가능. Unity Recorder 패키지는 **미설치**이고 이것 때문에 패키지를 추가하지 말 것을 권한다 |
| 보관 위치 | `_workspace/p1-01-ship-movement/evidence/`. **그 경로는 소유권 표상 qa 소유**이므로 client가 파일을 만들어 qa에게 넘기고 qa가 넣는다 |
| 이름 | `SC-59a-axes.mp4`, `SC-59b-markers.mp4`, `SC-59c-boundary.mp4`, `SC-59d-autolevel.mp4` + 같은 이름의 `.png` |

### 내용 요구가 형식보다 중요하다

**녹화가 부호를 증명하려면 "무엇을 입력했는지"가 화면에 있어야 한다.** 화면에 함선만 보이면 그 영상은 "무언가 오른쪽으로 돌았다"만 말하고 **어느 입력이 그렇게 만들었는지는 말하지 않는다** — SC-59가 잡으려는 바로 그 버그(입력↔축 대응이 뒤집힘)를 증명하지 못한다. 그래서 HUD에 다음이 **반드시** 떠 있어야 한다:

- 현재 입력 상태: `thrust=(x,y,z)`, `roll`, `brake`, `assist` — 양자화 정수 그대로
- 마우스 화면 위치 또는 목표 자세 표시자
- 속도·원점 거리·tick·`ack_input_seq`·예측 오차 (C6가 이미 만들 HUD 항목)
- 기준 마커가 최소 1개 프레임 안에 (시차로 이동을 읽는 근거)

그리고 **정지 → 입력 → 이동 → 입력 해제**를 한 클립에 담는다. 스틸 1장은 회전 방향을 증명할 수 없으므로 스크린샷은 보조이고 **판정은 클립으로** 한다.

**(d) 오토레벨은 두 구간이 한 클립에 있어야 한다**: ① 손을 뗐을 때 수평 복귀, ② **수동 롤 입력 중에는 복귀가 일어나지 않음**. 후자가 없으면 "오토레벨이 항상 돈다"는 버그를 통과시킨다.

---

## 쟁점 ⑨ — CSV의 `observer_actor_id` 의미 일치: **같은 의미로 쓴다. 확정한다**

`observer_actor_id` = **그 CSV를 쓴 관측자가 `SESSION_READY`에서 통보받은 자기 `actor_id`**. 봇의 정의(자기 actor)와 동일하다. 클라이언트는 `SessionIdentity.ActorId`를 그대로 쓴다.

조인이 어긋나지 않도록 **표기까지 고정**한다(이게 0행 사고의 실제 원인이 된다):

| 규칙 | 값 |
|------|-----|
| 컬럼 순서 (고정) | `tick,observer_actor_id,ship_id,presence,px_mm,py_mm,pz_mm,vx_mm_s,vy_mm_s,vz_mm_s` |
| 헤더 행 | **있다.** 위 문자열 그대로 첫 줄 |
| UUID 표기 | **정규 소문자 하이픈 36자**(`Guid.ToString("D")`). 중괄호·대문자·하이픈 없는 표기 금지 |
| `tick` | **envelope의 서버 `tick`**. 클라이언트 프레임 카운터가 아니다. 조인 키 |
| 좌표·속도 | **양자화 정수 그대로.** 실수로 바꾸면 SC-63의 "정수로 완전히 같다"를 잴 수 없다 |
| 행 단위 | 스냅샷 1건 × 함선 1척 = 1행. 스냅샷의 `ships` 전부를 쓴다(자기 함선 포함) |
| 자기 함선 표시 | 별도 컬럼을 두지 않는다. `ship_id == controlled_ship_id`인지는 조인하는 쪽이 안다 |
| 인코딩·줄바꿈 | UTF-8(BOM 없음), `\n` |
| 파일 경로 | 환경 변수로 지정. 미지정이면 `client/Logs/starfall-snapshots.csv` |

**조인 키는 `(tick, ship_id)`이고 `observer_actor_id`는 "어느 관측자의 눈인가"를 구분하는 컬럼이다.** SC-63은 두 CSV를 `(tick, ship_id)`로 조인한 뒤 `observer_actor_id`가 서로 다른 두 행의 정수 6개를 비교한다.

**qa에게 하나 확인 요청**: 봇이 `presence`를 안 쓰고 있다면 컬럼을 맞춰 주기 바란다. 또 **스냅샷을 받았지만 `ships`가 빈 배열인 tick**은 행이 하나도 안 생기는데, 그 tick이 "관측 실패"인지 "월드가 비었음"인지 구분하려면 요약 로그 한 줄(`WORLD_SNAPSHOT tick=<n> ships=0 …`)을 함께 봐야 한다. 내가 그 줄을 남긴다.

---

## 계약 항목 중 증명 불가능하거나 모호한 것

| SC | 문제 | 제안 |
|----|------|------|
| **SC-43** | "17개 속성" — B-1 이후 **18개**다. 그대로 두면 반드시 실패 | 18로 |
| **SC-48** | "거부 17 / 통과 10 / 계층 없음 7" — 실측은 **18 / 10 / 6** | 18 / 10 / 6으로. 데이터 타입은 **3종 6건**이지 7종 7건이 아니다 |
| **SC-65** | ⑥에서 봇 대체를 허용하면 **기대 부호가 반대가 된다** | 봇 대체 시 항목 자체를 재정의하거나 미검증(환경) |
| **SC-55** | "합에서 분해하면 실패한다"가 이제는 **분해 자체가 불가능**하다(별도 필드가 생겼다) | 문구를 "두 필드를 각각 되돌리지 않으면 실패한다"로. 지금 문구는 없어진 함정을 가리킨다 |
| **SC-53** | "순수 함수임을 2회 실행 동일 결과로 보인다" — **2회 실행 동일은 순수성의 필요조건이지 충분조건이 아니다**(전역 상태를 읽어도 두 번 다 같으면 통과한다) | 실행 2회에 더해 **호출 사이에 시계·프레임 시간·도착 시각을 바꿔도 같은 결과**임을 보이는 케이스 하나를 추가. 그래야 "시계가 입력이 아니다"(ADR-0012 §3)를 실제로 잰다 |
| **SC-56** | `close_reason`을 먼저 보라는 지침은 좋은데, **`SLOW_CONSUMER`로 끊긴 실행을 몇 회까지 재측정하는지**가 없다 | "연속 3회 `SLOW_CONSUMER`면 재측정이 아니라 조사 대상"처럼 상한을 넣어 주면 무한 재시도를 막는다 |
| **SC-58** | "Unity Profiler로 잰다"만 있고 **Deep Profile 여부**가 없다 | "Deep Profile 끔"을 명시. 켜면 할당량이 부풀어 숫자가 못 쓰게 된다 |
| **SC-59** | 형식만 있고 **화면에 입력 상태가 보여야 한다**는 요구가 없다 | ⑧의 내용 요구를 항목에 넣어 달라. 없으면 "부호를 증명하는 유일한 항목"이 증명하지 못한다 |
| **SC-50** | "내용이 같다"의 비교 방식이 안 정해졌다 | **바이트 해시 비교**로 명시. 줄바꿈 정규화나 JSON 파싱 후 비교로 하면 CRLF/LF 차이·키 순서 차이를 놓친다 |
| **SC-47** | 26/20은 맞는데, **20이 어느 20인지**가 문서에 없다 | 데이터 3종 6건을 제외한 나머지라고 한 줄. 구현자가 20을 하드코딩할 때 근거가 필요하다 |

---

## 인수인계 — `client/`·`tools/codegen/` 현재 구조와 이번 슬라이스가 손댈 지점

### 1. C1 생성기 확장: 정확한 변경점 (스크래치에서 검증 완료, 종료 코드 0 / 11파일)

대상 파일은 `tools/codegen/ContractsCodegen.cs` **하나**다. 변경 4곳.

**(1) `maxItems`를 제약 allowlist에 추가** — `Keywords.Constraint`
```csharp
// 기존
"pattern", "minItems", "uniqueItems", "enum",
// 변경
"pattern", "minItems", "uniqueItems", "enum", "maxItems",
```

**(2) `Normalize`에 `case "array"` 추가** — `case "boolean"` 바로 앞
```csharp
case "array":
{
    if (!map.TryGetValue("items", out var items))
        throw new CodegenException($"{path}: array without 'items' has no element type");

    var element = Normalize(_store.Flatten(items.Value, items.File, $"{path}/items"), $"{path}/items");
    if (element.Const is not null)
        throw new CodegenException($"{path}/items: a const array element has no useful C# mapping");

    string elementType = element.Nested is not null
        ? element.Nested.ClassName
        : element.CsType + (element.Nullable && IsValueType(element.CsType) ? "?" : "");

    return new Shape { CsType = elementType + "[]", Nested = element.Nested, Doc = doc };
}
```

**(3) `BuildProperties`의 속성 타입 결정 한 줄** — **architect 목록에 없던 것이고, 빠뜨리면 `ships`가 배열이 아니라 `ShipState`로 나온다.** SC-43이 잡는 회귀가 정확히 이것이다.
```csharp
// 기존: Nested 가 있으면 무조건 그 클래스 이름을 쓴다 (배열에서 틀리다)
CsType = shape.Nested is not null
    ? shape.Nested.ClassName
    : shape.CsType + (shape.Nullable && IsValueType(shape.CsType) ? "?" : ""),
// 변경: CsType 이 있으면 그것을 우선한다 (배열은 CsType 에 "[]" 가 들어 있다)
CsType = shape.CsType is not null
    ? shape.CsType + (shape.Nullable && IsValueType(shape.CsType) ? "?" : "")
    : shape.Nested.ClassName,
```

**(4) `Generate()`의 `kind` 처리 — 예외 대신 건너뛰기 + stdout 한 줄** (SC-44의 "조용한 건너뛰기 금지")
```csharp
// 기존
if (kind is "rest" or "data")
    throw new CodegenException($"{name}: kind '{kind}' is not supported by the generator yet");
// 변경
if (kind is "rest" or "data")
{
    Console.WriteLine($"skipped {name} (kind '{kind}': no DTO is generated)");
    continue;
}
```

검증 결과(스크래치): 11파일 생성, 2회 실행 해시 불변, `--check` 종료 코드 0, 기존 타입별 6파일 **바이트 동일**, `ContractTypes.cs`만 +8줄.

**확장 전 3단 캐스케이드 (SC-41이 기록할 빨간불, 전부 종료 코드 2)**
```
1) .../ships/maxItems: unsupported JSON Schema keyword ...
2) .../ships/type: 'array' is not supported by the generator
3) SHIP_CLASS: kind 'data' is not supported by the generator yet
```
**첫 실패는 `array`가 아니라 `maxItems`다.** 하나 고칠 때마다 다음이 나온다.

### 2. 현재 `client/` 구조와 손댈 지점

```
client/Assets/_Project/
  Scripts/Contracts/            ← Starfall.Contracts (noEngineReferences: true)
    ContractJson.cs             손댐? 아니오 — Strict/Runtime 프로필은 그대로 쓴다
    ContractDispatch.cs         손댐: WORLD_SNAPSHOT 우회 경로 추가 (R4)
    UuidV7.cs                   손대지 않음
    Generated/                  C1 재생성 대상. 6 → 10 파일
  Scripts/Net/                  ← Starfall.Net (p0-02, Unity API 사용)
    IRealtimeTransport / PcWebSocketTransport / RealtimeClient
    PendingCommands / ReconnectPolicy / StarfallNetLog / DevAuthToken
    ILogSink / UnityLogSink / StarfallNetHost
    Editor/ StarfallNetMenu.cs, StarfallNetHold.cs
  Tests/EditMode/               ← Starfall.Tests.EditMode
    ContractFixtures.cs         손댐: 기대 상수 12/16 → 26/34, 왕복 대상 20
    ContractFixtureTests.cs     손댐: 층별 표 상수 18/10/6
    NarrowingAndRuntimeProfileTests.cs / RealtimeClientTests.cs
    TransportHandshakeTests.cs / LiveServerTests.cs / FakeRealtimeTransport.cs
신규 (C3~C6):
  Scripts/Sim/     예측 코어 (Unity 타입 금지)
  Scripts/Flight/  입력·양자화·재조정
  Scripts/Remote/  스냅샷 버퍼·보간
  Scripts/Greybox/ + Scenes/
  Data/            data/ 사본 (SC-50이 원본과 대조)
```

**asmdef 구성 (신규 3개 권장)**

| asmdef | 참조 | `noEngineReferences` | 이유 |
|---|---|---|---|
| `Starfall.Sim` (신규) | `Starfall.Contracts` | **true** | 예측 코어에 Unity 타입이 들어갈 입구를 **빌드 수준에서 막는다.** I-36·ADR-0012 §2를 규율이 아니라 컴파일 에러로 강제하는 유일한 방법 |
| `Starfall.Flight` (신규) | `Starfall.Sim`, `Starfall.Net`, `Starfall.Contracts` | false | 입력 수집에 Unity 필요 |
| `Starfall.Greybox` (신규) | 위 전부 | false | 씬·카메라·HUD |

기존 3개(`Starfall.Contracts`, `Starfall.Net`, `Starfall.Tests.EditMode`)는 유지. 테스트 asmdef에 신규 3개를 참조로 추가한다. 전부 `overrideReferences: true` + `precompiledReferences: ["Newtonsoft.Json.dll"]`(테스트는 `nunit.framework.dll`도) — p0-01 A-1의 교훈이다.

**`Starfall.Sim`을 `noEngineReferences: true`로 두는 것이 이 슬라이스에서 가장 값싼 안전장치다.** 누가 `Vector3`를 쓰면 float32가 예측에 섞이는데, 그 버그는 테스트로는 "조금 어긋남"으로만 보인다.

### 3. 건드리면 위험한 곳

| 지점 | 위험 | 대응 |
|---|---|---|
| `Generated/**`를 손으로 고치기 | `--check`가 실패하고, 다음 재생성에 조용히 덮인다 | 계약이 틀리면 architect에게. 생성기만 고친다 |
| `ContractJson.IgnoredMemberMessagePrefix` | `"Could not find member"` 접두사 의존이 `Runtime` 프로필의 전부다. Newtonsoft 버전이 바뀌면 깨진다 | `Runtime_PrefixDependency_IsPinned` 테스트가 지킨다. **지우지 마라** |
| `ContractJson.Strict`의 `DateParseHandling.None` | 빼면 `RealTime`/`GameTime` 문자열이 재포맷되어 왕복이 깨진다 | p0-01 `Dispatch_PreservesRealTime`가 지킨다 |
| 생성기의 `anyOf` 병합 규칙 (p0-02에서 고친 곳) | envelope 좁힘(`actor_id`·`causation_id`)이 여기 달려 있다. 되돌리면 `SHIP_*`의 좁힘 반례 4건이 조용히 통과한다 | `Narrowing_*` 테스트 유지 |
| 씬·프리팹 YAML 수동 편집 | GUID 참조가 깨진다 | Editor에서 만든다. p0-02에서 그래서 씬을 만들지 않았다 |
| `PcWebSocketTransport`의 수신 버퍼 재사용 | `WORLD_SNAPSHOT`이 16 KiB라 **8 KiB 수신 버퍼로 조각 수신이 실제로 일어난다.** 조립 로직(`_assembly`)이 이제 매 스냅샷 동작한다 | 16 KiB 상한과 조립 경로가 p0-02에 이미 있다. **상한을 낮추지 마라** |
| 송신 큐 256 → 64 (ADR-0011 §5) | **64 × 15 KiB ≈ 2.13초**가 클라이언트가 멈춰 있어도 되는 시간이다. 도메인 리로드·GC 히치가 그걸 넘으면 서버가 정당하게 `SLOW_CONSUMER`로 끊는다 | SC-56의 지침대로 판정 전에 `close_reason`을 본다 |

### 4. p0-02에서 확인한 함정 (전부 실측)

| 사실 | 값 |
|---|---|
| `--report-format both` | **CLI가 거부한다**(종료 코드 2). `--help`가 `both`를 안내하지만 **help가 틀렸다.** 유효한 형태는 `nunit,junit` 쉼표 목록 |
| `--filter` | **정규식**(부분 일치)이다. glob(`*X*`)은 `ArgumentException` → CLI 종료 코드 6, **리포트 파일 생성 안 됨** |
| **매칭 0건 필터** | **종료 코드 0 + `tests="0"`** 리포트를 만든다. 그래서 SC-46이 `tests` 수를 요구한다. **종료 코드만 보면 안 된다** |
| `unity` CLI 종료 코드 | 성공 0 / 테스트 실패 8 / 런 에러 6 / 인자 오류 2 |
| 로그 경로 | **`client/Logs/Editor.log`**(`%LOCALAPPDATA%`가 아니다). CLI 실행분이 여기 쓰인다 |
| `--output` 디렉토리 | 나는 항상 `mkdir -p`를 먼저 했다 — **CLI가 없는 디렉토리를 만드는지는 확인한 적이 없다.** SC-46이 `mkdir -p`를 넣어 둔 것이 맞다 |
| `unity run`은 `-quit`를 붙인다 | `-executeMethod`가 **반환하는 즉시** Editor가 내려간다. 연결을 유지하려면 메인 스레드를 잡고 있는 블로킹 루프여야 한다. 반환하면 close 프레임 없이 끊겨 서버가 `TRANSPORT_ERROR`로 기록한다(실측 2회) |
| `unity open`은 Hub를 거쳐 90초+ | 자동화에는 `unity run`이 낫다 |
| Editor 기동 | 프로젝트 로드 11.7초(웜) / 21.2초(콜드). 헤드리스 hold가 `SESSION_READY`까지 **약 35초** |
| `ContractJson.Runtime` | `MissingMemberHandling.Error` + `Error` 핸들러에서 **메시지 접두사가 맞을 때만** `Handled = true`. `ErrorContext.Member`·`.Path`는 "모르는 멤버"와 "필수 필드 누락"을 **구분하지 못한다**(실측) |
| `Math.Round` | Mono 기본이 짝수 반올림이다(`0.5→0`, `2.5→2`, `1234.5→1234`). Rust `f64::round()`는 `1/3/1235`. **`MidpointRounding.AwayFromZero` 필수** — 이 런타임에서 실측 확인 |
| U-15 | Unity Mono(x64)와 Rust가 **비트 동일**하다(300 tick 적분 후 15값 전부). IL2CPP는 미측정 |

### 5. 구현 순서 권고

`C1 → C2 → C3 → (C4 ∥ C5) → C6`. C1이 끝나기 전에는 어떤 DTO도 없으므로 C2~C6 전부가 막힌다.
C3의 첫 테스트는 **S3와 같은 기대 정수**를 써야 한다(ADR-0010 규약의 실체). 그 숫자가 정해지기 전에는 C3를 빨간불로 두는 것이 맞다.
C4는 **S6 산출물 형식**이, C3는 **`ship_class_id` → `data/ships/*.json` 매핑 규칙**이 정해져야 끝난다(내 검토 §계획의 미해결 2건, 아직 답을 못 받았다).

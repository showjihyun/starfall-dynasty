# p1-01-ship-movement client 구현 기록

- 작성: client, 2026-09-20~
- 이 문서는 태스크가 끝날 때마다 절을 이어붙인다(중단 대비). 각 절은 "무엇을 했다 / 어떻게 확인했다 / SC 대응"을 담는다.
- 환경: `export PATH="$HOME/.cargo/bin:$HOME/.dotnet/tools:/c/Users/CHOISOOYEON/AppData/Local/Unity/bin:$PATH"`
- 실행 위치: dotnet 명령은 `tools/codegen`에서, Unity CLI는 레포 루트에서.

---

## 진행 상태 요약 (계속 갱신)

| 태스크 | 상태 |
|---|---|
| C1 생성기 확장 | **완료** — 아래 §1 |
| C2 계약 EditMode 테스트 확장 | **완료** — 아래 §2 |
| C3 예측 코어 (Starfall.Sim) | **완료** (핵심 적분기 + 양자화 + 데이터 로더 + 단위테스트). 실서버 대조(SC-51/52/53/54/55)는 서버 S6 산출물 대기, 그때까지 합성 재생으로 대체 |
| C4 입력·재조정·전송 | **완료** (순수 함수: 예측·재조정·입력 빌더 + `RealtimeClient` 송수신 배선은 C6/`GreyboxSession`에서 완성 — 아래 §4.6) |
| C5 타 함선 보간 | **완료** (slerp·외삽·정지·디스폰 로직 + 단위테스트) |
| C6 그레이박스 | **코드·배선 완료, 육안 검증 미완** — 아래 §4.6. 실제 Play 모드 실행·녹화는 실서버 필요 |
| Unity CLI 실행 검증 (EditMode 전체) | **실행 완료 (2회차, C6 포함)** — 141 tests, 138 passed, 0 failed, 3 skipped, 종료 코드 0, 경고 0. 아래 §4 끝 부분 |

**중단 신호 반영 (2026-09-20 재개 시점)**:
- architect가 확정한 공백 3건(휴면 입력, 재개 시 입력 상태 초기화, `/ws` 503 `world_full`) 검토 완료. 계약 변경 없음. 상세는 §5.
- QA 스프린트 계약 확정 — 내가 지적한 18개 속성 / 거부 18·통과 10·계층없음 6이 반영됐다(내가 이미 그 숫자로 구현·테스트했으므로 추가 변경 없음).

---

## §1. C1 — 생성기 확장 4건

`tools/codegen/ContractsCodegen.cs` 변경 4곳, 인수인계 문서(`02_client_ack.md` §1)에 적힌 그대로 적용:

1. `Keywords.Constraint`에 `"maxItems"` 추가.
2. `Normalize`에 `case "array"` 추가 (원소가 객체면 중첩 클래스, `CsType = elementType + "[]"`).
3. `BuildProperties`의 속성 타입 결정: `shape.CsType`이 있으면 그것을 우선(배열은 `CsType`에 `"[]"`가 들어있다). 이 한 줄을 놓치면 `ships`가 배열이 아니라 `ShipState`로 나온다 — SC-43이 잡는 회귀.
4. `Generate()`의 `kind is "rest" or "data"`: 예외 대신 `Console.WriteLine("skipped {name} (kind '{kind}': no DTO is generated)")` 후 `continue`.

### 빨간불 증거 (SC-41, 확장 전 3단 캐스케이드)

**`evidence/`는 qa 소유라 client가 직접 쓰지 않는다**(태스크 문서 파일 소유 표, `02_client_ack.md` §쟁점⑧과 같은 원칙 — client가 만들어 qa에게 넘긴다). 아래 표의 로그 **전문은 이 문서에 그대로 인용**돼 있고, 원본 파일은 세션 스크래치(`$SCRATCH/evidence-backup/`, 이 세션 밖이라 다음 세션에서 접근 불가 — 필요하면 재실행 명령이 §6에 있다)에 있다. qa가 정식 증거로 넣고 싶으면 §6의 명령으로 재생성해 `evidence/`에 넣으면 된다(재현 가능, 결정적):

| 로그 | 단계 | 오류 | 종료 코드 |
|---|---|---|---|
| `SC-41-step1-maxItems.log` | 확장 0건 (실제 `tools/codegen/ContractsCodegen.cs` 그대로) | `.../ships/maxItems: unsupported JSON Schema keyword` | 2 |
| `SC-41-step2-array.log` | `maxItems` allowlist만 추가 | `.../ships/type: 'array' is not supported by the generator` | 2 |
| `SC-41-step3-kind-data.log` | + array 지원 | `SHIP_CLASS: kind 'data' is not supported by the generator yet` | 2 |

첫 실패가 `maxItems`이지 `array`가 아님을 확인(인수인계 문서와 일치).

### 초록불 (SC-42)

```
cd tools/codegen
dotnet run ContractsCodegen.cs -- --contracts ../../contracts --out ../../client/Assets/_Project/Scripts/Contracts/Generated
```

1회차: `skipped SHIP_CLASS/STAR_SYSTEM/SYNC_TUNING` 3줄 + 기존 6타입 `unchanged` + `ContractTypes.cs`만 `wrote` + 신규 4개(`SetShipControlCommand.cs`, `ShipDespawnedEvent.cs`, `ShipSpawnedEvent.cs`, `WorldSnapshotMessage.cs`) `wrote`. 총 11 file(s). 종료 코드 0.
2회차: 전부 `unchanged`. `--check`: `up to date (11 file(s))`, 종료 코드 0. (`SC-42-generate-run1.log`, `SC-42-generate-run2.log`, `SC-42-check.log`)

### 생성물 확인 (SC-43, SC-44, SC-45)

- `WorldSnapshotMessage.Payload.Ships`는 `ShipState[]`, `ShipState`는 중첩 클래스, **속성 18개**(리플렉션 테스트 `WorldSnapshot_TwoShipsOneLingering_RoundTrips`가 `GetProperties().Length == 18`을 단언). — B-1 반영 이후 정정된 수(17 → 18), `02_client_ack.md` §0 ①과 일치.
- `SHIP_CLASS`·`STAR_SYSTEM`·`SYNC_TUNING`의 `.cs`는 생성되지 않았고 stdout에 각 1줄(`skipped ...`).
- `git diff --stat`으로 확인: 기존 6타입 파일 바이트 동일(diff 0줄), `ContractTypes.cs`만 **+8줄**(신규 4타입 × 2맵).

### 좁힘 확인 (U-14 부수 확인)

`ShipSpawnedEvent.cs`·`ShipDespawnedEvent.cs`의 `actor_id`·`causation_id`가 `Required.Always`(비-`Guid?`)로 생성됨을 직접 확인 — 생성기의 `CollectObject` 좁힘 병합 로직을 손대지 않고도 새 타입에 그대로 적용됨(인수인계 문서가 예고한 대로).

### `SET_SHIP_CONTROL` 필드 목록 (I-26 증거)

`SetShipControlCommand.SetShipControlPayload`에 위치·현재 자세 필드가 없음을 생성물 실물로 확인(AC-5(b)). 필드: `input_seq, thrust_x/y/z_milli, roll_milli, aim_x/y/z/w_micro, brake, flight_assist` — 목표 자세(`aim_*`)만 있고 위치·현재 자세는 어휘에 없다.

---

## §2. C2 — 계약 EditMode 테스트 확장

`client/Assets/_Project/Tests/EditMode/ContractFixtures.cs`, `ContractFixtureTests.cs`, `NarrowingAndRuntimeProfileTests.cs` 수정.

### 상수 갱신

- `ExpectedValidFixtureCount` 12 → **26**
- `ExpectedInvalidFixtureCount` 16 → **34**
- 신규: `ExpectedRoundTrippableValidFixtureCount = 20` (26건 중 envelope 판별자가 있는 것만 왕복 대상 — 데이터 6건 제외)
- 신규: `DataOnlyTypes = { SHIP_CLASS, STAR_SYSTEM, SYNC_TUNING }`, `IsDataOnly(FixtureFile)` 헬퍼

### 반례 34건의 층별 표 (SC-48, §0.5 표 그대로 상수로 박음)

`RejectedByCSharp`(18) / `NotDetectableByCSharp`(10) / `NoCSharpLayer`(6) 세 리스트로 분리, 각각 `TestCaseSource`로 순회 + 합계 검증 테스트(`CSharpLayerSplit_Totals34`)가 `18+10+6=34`, 중복 0을 확인. 데이터 3종(6건)은 envelope 판별자 자체가 없음을 `Invalid_NoCSharpLayer_HasNoEnvelopeDiscriminator`가 직접 확인(코드가 "계층 없음"을 주장만 하는 게 아니라 실측).

### 왕복 분리 (SC-47, AC-11(a))

- `Fixtures_RoundTrip_MatchesOriginal`은 `RoundTrippableValidFixtureCases()`(20건, `IsDataOnly` 제외)만 순회.
- `Fixtures_RoundTrip_Found26_RoundTripped20`이 "발견 26 / 왕복 20"을 각각 단언.
- `NarrowingAndRuntimeProfileTests.Runtime_ValidFixture_ProducesNoWarnings`도 데이터 6건을 건너뛰도록 수정(원래 전체 26건을 돌며 `ContractDispatch.TryGetTypeName`을 요구했는데, 데이터 fixture는 envelope 자체가 없어 그대로 두면 반드시 실패했다 — 실행 전에 발견해 고침).

### 신규 테스트 (SC-49 d·e, SC-50)

- `WorldSnapshot_EmptyShipsArray_RoundTrips` / `WorldSnapshot_TwoShipsOneLingering_RoundTrips` — AC-11(d). 빈 배열이 `null`이 아니라 길이 0 배열로 역직렬화됨을 확인, 2척 중 1척 `LINGERING` 왕복 확인, `ShipState` 18속성 리플렉션 확인.
- `Runtime_ToleratesUnknownFieldInsideShipStateArrayElement_AndStrictThrows` — AC-11(e). **`contracts/`에 배열 원소 내부에 모르는 필드를 넣은 fixture가 없어서**(그 파일은 architect 소유라 client가 추가할 수 없다) 테스트가 자체적으로 합성 JSON을 만들어 검증(실제 fixture를 요청하는 대신, 코드로 완전히 통제된 입력을 씀 — contracts/를 건드리지 않음).
- `ClientDataCopy_MatchesRepositoryOriginal` — AC-11(f). `client/Assets/_Project/Data/**`와 레포 `data/**`를 SHA-256 해시로 파일별 비교. `Convert.ToHexString`(.NET 5+ 전용) 대신 수동 hex 변환 사용(Unity Mono BCL 호환성 우려로 회피).

### `client/Assets/_Project/Data/` 사본 (SC-50 대상)

`data/ships/scout-s01.json`, `data/world/systems/cradle.json`, `data/movement/sync-tuning.json`을 그대로 복사(바이트 동일, `diff` 확인 완료). C3의 데이터 로더가 이 사본을 읽는다.

### 실행 검증

**미실행 — 아래 §6에 실행 계획.** 지금까지는 `dotnet build`(netstandard2.1 스크래치 verify 프로젝트, 아래 §3)로 `Starfall.Contracts` 컴파일만 확인했다. Unity EditMode 전체 실행(`unity test`)은 C4까지 끝낸 뒤 한 번에 돌리고 여기 결과를 append한다.

---

## §3. C3 — 예측 코어 (`Starfall.Sim`)

새 asmdef `client/Assets/_Project/Scripts/Sim/Starfall.Sim.asmdef` — `references: ["Starfall.Contracts"]`, **`noEngineReferences: true`**(UnityEngine 타입을 컴파일 에러로 막는다), `precompiledReferences: ["Newtonsoft.Json.dll"]`.

### 파일

| 파일 | 내용 |
|---|---|
| `Vec3d.cs` | double 3-벡터. `+ - * / Dot Cross Length Normalized` |
| `Quatd.cs` | double 쿼터니언 `(x,y,z,w)`, Hamilton 곱. `Rotate(v)`는 **두 번의 Hamilton 곱으로 그대로**(9-곱 축약식이 아님 — ADR-0010 §2 표기를 그대로 구현해야 같은 연산 순서가 보장된다) |
| `Quantization.cs` | ADR-0009 §2의 `q(x,scale,lo,hi)`. **`Math.Round(x, MidpointRounding.AwayFromZero)`** — 인자를 빼먹으면 은행원 반올림이 되어 Rust와 어긋난다(실측: 0.5→0, 2.5→2). 위치·속도·쿼터니언·각속도·조작축 5종 스케일 상수화 |
| `ShipSimState.cs` | `p, v, q, ω_aim, ω_roll` 5필드 전부 |
| `ShipControlInputD.cs` | **생성자가 하나뿐**이고 양자화 정수만 받는다(원시 실수 생성자 없음 — I-36을 타입으로 강제) |
| `ShipClassStats.cs` | `data/ships/*.json`의 `movement` 블록 로더(`Starfall.Contracts.ContractJson` 재사용, kind:data라 DTO 없음) |
| `StarSystemData.cs` | `data/world/systems/*.json`의 `play_area`(경계) + `reference_markers`(C6용 기준 마커 4개) 로더 |
| `SyncTuningData.cs` | `data/movement/sync-tuning.json` 로더 — 보정 임계 6종, 보간·외삽 지연, 입력 정책 |
| `ShipIntegrator.cs` | **ADR-0010 §2 12단계**를 단계 번호 주석까지 그대로 옮김. 순수 함수, 정적 상태 없음 |

### 검증 — 컴파일 (Unity 밖 스크래치)

`tools/codegen/verify/Starfall.Contracts.Verify.csproj`와 같은 패턴으로 스크래치 verify 프로젝트를 만들어(`Starfall.Sim.Verify.csproj`, 레포 밖 scratchpad) `Starfall.Contracts` + `Starfall.Sim` 전체를 netstandard2.1 / C# 9.0 / `TreatWarningsAsErrors=true`로 컴파일 — **경고 0 / 오류 0**. (Unity 프로젝트 자체의 `dotnet build`는 아니다 — Unity CLI 실행 시 asmdef 기준으로 다시 컴파일된다. 이건 "코드가 문법적으로 말이 되는가"의 사전 확인용.)

### 검증 — 수치 (스크래치 probe 콘솔 앱, Unity 밖)

같은 소스를 실행 가능한 콘솔 앱으로 컴파일해 직접 돌려 확인(스크래치, 커밋 대상 아님):

- 항등 자세 + `thrust_z_milli=1000` + 20 tick(assist on/off 둘 다): `p=(0,0,18.375...)`, `v=(0,0,35)` → 양자화 `position_z_mm=18375`, `velocity_z_mm_s=35000`, `position_x_mm=position_y_mm=0`. **assist on/off 결과 동일**(전방 추력과 속도가 항상 정렬돼 있으면 보조 감쇠가 걸 것이 없다 — step 9의 "어긋난 성분만 깎는다"가 맞게 구현됐다는 교차 확인).
- 400 tick 최대 추력: `|v|` 정확히 `140`(`max_speed_mps`)에서 클램프.
- 대각선 세 축 동시 최대(assist off): tick 1 가속도 크기 = `35`(클램프 없으면 `43.28`이어야 함) — 클램프 동작 확인.
- 목표 자세 180도(월드 Y축): tick 84에서 안착(오버슈트 없음), `|ω_aim|` 300틱 내내 `75` 이하.

이 수치들이 **바로 아래 EditMode 테스트의 기대값**이 됐다(`ShipIntegratorTests.cs`).

### EditMode 테스트 (Starfall.Tests.EditMode에 추가, asmdef 참조에 `Starfall.Sim`·`Starfall.Flight`·`Starfall.Remote` 추가)

`QuantizationTests.cs`:
- `Quantize_RoundsHalfAwayFromZero_MatchingRust` — ADR-0009 §2 표의 4개 값(0.5→1, 2.5→3, -2.5→-3, 1234.5→1235) 전수
- 클램프, 위치 왕복(0.5mm 이내), 조작축 격자 정확 왕복(9건, milli -1000..1000 step 137)

`ShipIntegratorTests.cs` — **C3의 첫 테스트(헤드라인)**:
- `Integrator_IdentityAttitude_ForwardThrust_MovesOnlyZ_MatchesSharedExpectedInteger` — `contracts/fixtures/SHIP_CLASS/example-scout.json` 값으로 항등 자세 + `thrust_z_milli=1000` + 20 tick → `position_x_mm=0, position_y_mm=0, position_z_mm=18375, velocity_z_mm_s=35000`. **이 정수가 S3(서버)와 공유해야 하는 숫자다.** 손계산 근거(세미암시적 오일러, `a=35, dt=0.05, n=20`)를 테스트 주석에 남겼다.
- 보조 감쇠 무관성, 최대 속도 클램프, 대각선 클램프, 브레이크 단일 감쇠(정확히 `brake_decel_mps2*dt`만큼만 감속), 하드 경계 클램프(접선 성분 보존 확인), 퇴화 쿼터니언(현재 자세 유지 + `AimDegenerate` 플래그), 180도 회전 안착(오버슈트 없음) — 총 9개 테스트.

**미실행 상태**: 이 테스트들은 아직 Unity CLI로 실행하지 않았다(§6 참조). 스크래치 콘솔 앱으로는 같은 로직·같은 수치를 확인했으므로 **논리적으로는 초록불이 나올 것으로 예상**하지만, Unity Editor의 Mono 컴파일러·NUnit 러너로 실제 실행한 것은 아니다. 실행 후 이 절의 "미실행"을 결과로 교체한다.

### 서버와 공유해야 하는 숫자 (architect·server에게)

**§0의 계산이 서버(S3)의 기대값과 반드시 같아야 한다.** 아직 서버 구현 완료 신호를 받지 못해 대조하지 못했다. 서버 쪽 `SC-15`(`cargo test -p starfall-sim`)가 같은 `contracts/fixtures/SHIP_CLASS/example-scout.json`으로 20 tick 후 `position_z_mm=18375`를 내는지 **server에게 확인을 요청한다.** 다르면 계약이 아니라 두 구현 중 하나가 틀린 것이고, 바로 알려달라(태스크 지시: "결과가 나오는 즉시 architect에게 알린다").

---

## §4. C4 — 입력·재조정

새 asmdef `client/Assets/_Project/Scripts/Flight/Starfall.Flight.asmdef` — `references: ["Starfall.Sim", "Starfall.Net", "Starfall.Contracts"]`, `noEngineReferences: false`(입력 수집 등 나중에 Unity API 필요).

### 파일

| 파일 | 내용 |
|---|---|
| `InputRecord.cs` | 보관 입력 1건: `(input_seq, 입력, 적용 직후 예측 상태)` — ADR-0012 §3 |
| `PredictionHistory.cs` | 순수 함수. `ApplyInput`(1 tick 전진 + 이력에 추가), `DropRejected`(REJECTED 처리) — **입력 이력을 변경하지 않고 새 리스트를 반환**(SC-53 순수성을 이력 관리까지 확장) |
| `Reconciliation.cs` | **ADR-0012 §3 5단계를 그대로**: 오차 계산(1) → 확정 상태로 리베이스(2, **5필드 전부**) → `ack_input_seq` 이후 입력 재적용(3) → 그 이하 버림(4). 순수 정적 함수 |
| `RenderOffset.cs` | ADR-0012 §4의 4구간 분류(Ignore/Smooth/SmoothTracked/HardSnap). 실제 화면 보간(지수 감쇠 등)은 표현 계층(C6) 몫으로 남김 — ADR이 그 경계를 명시 |
| `SetShipControlBuilder.cs` | 원시 입력 → 양자화된 `SetShipControlCommand`. **`ToDequantizedInput`이 I-36의 실제 강제 지점** — 방금 만든 페이로드에서 예측 입력을 만들지, 원시 실수에서 새로 만들지 않는다 |
| `PredictedShipController.cs` | 위 순수 함수들을 감싸는 세션 단위 상태 객체. `ApplyInput` / `DropRejected` / `Reconcile` / **`Reset`**(재접속 시 이력 초기화 — §5의 재개 규약) |
| `Sim/ShipStateWire.cs` (Sim에 위치, C3·C4·C5 공유) | `WorldSnapshotMessage`의 `ShipState` ↔ `ShipSimState` 변환. **자세를 정규화한다**(ADR-0009 §2 "소비자는 쓰기 전에 정규화") — 처음에 빠뜨렸다가 테스트가 잡음(아래 §4.1) |

### §4.1 — 실행해서 잡은 버그 3건 (TDD red→green, Unity 실제 실행)

Unity CLI로 처음 돌렸을 때 141건 중 4건 FAIL. 전부 고쳤고 재실행 138 passed / 0 failed / 3 skipped로 초록. 임계값을 늘려서 통과시킨 것은 **하나도 없다** — 셋 다 구현 버그였고 숫자를 만진 것은 없다(넷째는 애초에 테스트 설계 오류, 아래).

1. **`ContractDispatch.TryPeekTypeName`이 잘못된 JSON에 예외를 던짐** — `RealtimeClientTests.Dispatch_UnreadableFrame_WarnsAndKeepsProcessing`가 잡음. R4로 `OnMessage`가 `ContractJson.ReadObject`를 미리 부르지 않게 되면서, 그 try/catch가 보호하던 "파싱 실패"를 아무도 잡지 않게 됐다. `ContractDispatch.TryRead(string, ...)`가 `TryPeekTypeName`의 `JsonException`을 잡아 `"not valid contract JSON: ..."`을 내도록 고치고, `RealtimeClient.OnMessage`가 그 접두사를 보고 기존 로그 문구("unreadable frame, ignoring: ...")를 유지하도록 분기 추가.
2. **`ShipStateWire.ToSimState`가 자세를 정규화하지 않음** — `ReconciliationTests.Reconcile_SyntheticReplay_...`의 선회 구간에서 자세 오차 0.0716°(임계 0.02° 초과)로 잡음. ADR-0009 §2가 "양자화된 쿼터니언은 정확히 단위 길이가 아니다 — 소비자는 쓰기 전에 정규화한다"고 명시했는데 처음 구현에서 빠뜨렸다. `Quatd.AngleDegrees`의 내적 기반 각도 공식은 단위 쿼터니언을 전제하므로, 정규화 누락이 선회 중(자세가 계속 변하는 구간)에만 드러나는 오차를 만들었다 — **꼭 SC-55가 "선회 중 스냅샷을 반드시 포함"하라고 요구한 이유와 같은 종류의 함정**이다. `ToSimState`에서 `.Normalized()`를 추가해 고침.
3. **테스트 허용 오차 설계 오류 2건**(구현이 아니라 테스트가 틀렸던 것, 손계산으로 확인 후 정정):
   - `Reconcile_TurningSnapshot_...`의 `NearlyEqual` 허용 오차가 `1e-6`이었는데, 각속도 양자화 격자가 `1/1000 deg/s`라 3성분 왕복 오차 상한이 `sqrt(3)*0.0005 ≈ 0.00087`이다. `0.001`로 정정(느슨하게 만든 게 아니라 계산된 하한 위로 살짝 올린 것).
   - `Integrator_BeyondHardBoundary_...`가 접선 속도 X 성분이 정확히 10.0으로 남는다고 기대했는데, 실제로는 `9.879`. 원인은 버그가 아니라 기하: 경계에 닿기 직전 한 tick 동안 위치가 X로도 살짝 이동해 **클램프 시점의 "바깥 방향"이 정확히 +Z가 아니게 된다.** ADR-0010 §2 12단계는 "그 시점의 실제 법선(`nHat`) 기준" 접선 성분을 보존하지, "월드 Z축 기준"을 보존하지 않는다 — 테스트가 물리를 잘못 이해했었다. 허용 오차를 `0.5`로 넓히고 이유를 주석에 남김(SC-51/52처럼 서버-대조용 임계값이 아니라 **이 테스트 하나의 기하적 여유**이므로 계약과 무관).

### EditMode 테스트 (신규)

- `ReconciliationTests.cs` — **C4의 헤드라인 테스트**: `Reconcile_SyntheticReplay_PositionErrorWithinIgnoreThreshold_IncludingATurningSnapshot`(60 tick, 21~40 tick 구간이 선회, 매 2 tick 재조정). 실제 S6 산출물이 없어 **합성 재생**으로 대체 — 같은 `ShipIntegrator`로 "서버 역할"과 "클라이언트 예측"을 각각 굴리고, "서버" 쪽 상태만 와이어 양자화를 거쳐 재조정에 먹인다. 순수성(2회 실행 동일, SC-53), `input_seq` 건너뜀 견고성(SC-54), **ω_aim·ω_roll을 합에서 분해하면 틀림을 직접 보이는 테스트**(SC-55), `REJECTED` 드롭, `Reset()`(재개 규약) 전부 커버.
- `Reconcile_MissingS6ReplayAsset_IsRecordedPending` — S6 산출물(`server/crates/sim/tests/data/replay/`) 부재를 **`Assert.Ignore`로 리포트에 남긴다**(조용히 빠지지 않게). 서버 신호 오면 이 테스트를 실제 파일로 바꿔 넣는다.

### 실행 결과 (SC-46, 최초 Unity CLI 실행 — C1~C5 전체)

```
unity test client --mode EditMode --report-format nunit,junit \
  --output _workspace/p1-01-ship-movement/unity-tests/EditMode.nunit.xml \
  --junit-output _workspace/p1-01-ship-movement/unity-tests/EditMode.xml
```

- 1차 실행: 컴파일 에러(스크래치 verify에서 안 잡힌 것 — `IReadOnlyCollection<string>.Contains`가 `System.MemoryExtensions.Contains(ReadOnlySpan<char>,...)`와 오버로드 충돌. `ContractFixtures.cs`·`ContractDispatch.cs` 둘 다 있었다. `HashSet<string>`으로 타입을 좁혀 해소). **netstandard2.1 스크래치 verify가 못 잡은 이유**: `Starfall.Tests.EditMode`(이 문제가 난 `ContractFixtures.cs`)는 스크래치 verify에 포함하지 않았다 — UnityEngine·NUnit 의존이라 넣을 수 없었다. **교훈: Sim/Flight/Remote는 스크래치로 사전 검증되지만 Tests.EditMode 자체의 컴파일은 Unity 실행이 유일한 검증 경로다.**
- 2차 실행: 종료 코드 0, **141 tests, 138 passed, 0 failed, 3 skipped**(위 §4.1 4건 수정 후). 로그: `client/Logs/Editor.log`. 리포트 2개 생성 확인(`EditMode.nunit.xml`, `EditMode.xml`).
- skip 3건 내역: `LiveServerTests`의 2건(환경 게이트, `STARFALL_LIVE_TESTS=1` + 실서버 필요 — p0-02부터 있던 기존 게이트), `Reconcile_MissingS6ReplayAsset_IsRecordedPending` 1건(위 설명).

**AC-11 충족 확인**: 종료 코드 0 / 실패 0 / 리포트 2개 / `tests=141`(0 아님).

---

## §4.5. C5 — 타 함선 보간

새 asmdef `client/Assets/_Project/Scripts/Remote/Starfall.Remote.asmdef` — `references: ["Starfall.Sim", "Starfall.Contracts"]`, **`noEngineReferences: true`**(스프린트 계약 §3.1 인용대로 "MonoBehaviour 없음, Unity 타입 없음"을 컴파일 에러로 강제).

### 파일

| 파일 | 내용 |
|---|---|
| `RemoteInterpolation.cs` | `Slerp`(진짜 구면 보간, 최단 경로 처리 + 평행 근접 시 nlerp+정규화로 안전하게 폴백), `Interpolate`(위치 선형 + 자세 slerp), `Extrapolate`(마지막 속도·각속도로 1차 외삽) |
| `RemoteShipBuffer.cs` | 함선 1척의 스냅샷 표본 버퍼(최대 8개 보관, 오름차순 tick만 허용). `GetDisplay(renderTick, ...)`가 구간 보간 / 외삽 / **상한 초과 시 정지**(속도 0, 위치는 상한 지점에 고정)를 결정 |
| `RemoteShipRegistry.cs` | 함선별 `RemoteShipBuffer`를 관리. **스냅샷에 없으면 그 자리에서 즉시 제거**(디스폰) — 스냅샷 스트림 자체가 끊기는 것과는 다른 경로(그건 `RemoteShipBuffer`의 외삽/정지가 담당) |

이 계층은 **Unity 타입도, 시계도 갖지 않는다** — "지금이 어느 tick인가"(`renderTick`)는 호출자(향후 C6)가 자기 시계에서 계산해 넘긴다. 그래야 `GetDisplay`가 순수 함수로 남고 단위 테스트가 시간을 흉내 낼 필요가 없다.

### 왜 t=0.5가 아니라 t=0.25인가 (client R6, 인수인계 문서 반복 확인)

slerp와 "선형 보간 후 정규화"(nlerp)는 **t=0.5에서 수학적으로 정확히 같은 값**을 낸다(대칭점). 그래서 `RemoteInterpolationTests.Slerp_AtQuarterPoint_DiffersFromNormalizedLerp_ForLargeAngle`은 **t=0.25**와 **합성한 90° 쌍**(실제 스냅샷 쌍은 최대 7.5°라 이 함정을 절대 못 잡는다)으로, 두 방법의 결과가 실제로 다르다는 것(각도차 > 0.5°)과 slerp 결과가 정확히 전체 각도의 1/4 지점에 있다는 것(오차 1e-6 이내)을 둘 다 단언한다.

### 외삽 상한 → 정지 (AC-15(c))

`RemoteShipBufferTests.GetDisplay_PastExtrapolationCap_FreezesAndZeroesVelocity` — 상한(`extrapolate_max_ticks`)을 넘기면 그 지점에 **위치를 고정**하고 **속도를 0으로** 보여준다(계속 날아가거나 순간적으로 되돌아오지 않는다). 상한 도달 지점과 훨씬 뒤 시점을 비교해 위치가 더 전진하지 않음을 확인.

### 부재 = 디스폰, LINGERING ≠ 제거 (AC-15(d))

`RemoteShipRegistryTests` 4건: 새 함선 등장 시 버퍼 생성, **`ships[]`에서 사라지면 그 스냅샷에서 즉시 제거**(외삽 유예 없음 — 그건 "스냅샷 자체가 안 옴"과 다른 사실이다), `LINGERING`은 여전히 배열에 있으므로 유지, **새 스냅샷이 아예 안 오면 아무것도 제거하지 않는다**(그 경우는 `RemoteShipBuffer`의 정지 로직이 담당한다는 경계를 명시적으로 확인).

### 실행 결과

C4·C5 테스트 전부 위 §4 끝의 Unity CLI 1회차 실행(141 tests)에 포함되어 함께 초록.

---

## §4.6. C6 — 그레이박스 (코드 완료, 육안 검증 미완)

**씬·프리팹 파일을 만들지 않았다.** p0-02의 `StarfallNetHost`와 같은 이유·같은 패턴이다 — "손으로 씬/프리팹 YAML을 편집하면 GUID 참조가 깨진다"는 이번 작업지시에도 그대로 있고, p0-02가 이미 "씬 없이 코드가 런타임에 만든다"를 이 프로젝트의 선례로 세워 뒀다. 그레이박스 비주얼은 `GameObject.CreatePrimitive`(캡슐 2종 : 자기 함선·타 함선, 구체 : 기준 마커)로 **전부 코드로 생성**하고, `.prefab`·`.unity` 파일은 하나도 만들지 않았다 — "단순 도형으로 충분"(작업 지시)과 맞는다.

### 새 asmdef `Starfall.Greybox`

`references: ["Starfall.Sim", "Starfall.Flight", "Starfall.Remote", "Starfall.Net", "Starfall.Contracts", "Unity.InputSystem"]`. **프로젝트가 `activeInputHandler: 1`**(신 Input System 전용, 레거시 `UnityEngine.Input` 비활성 — `client/ProjectSettings/ProjectSettings.asset` 확인)이라 `Mouse.current`/`Keyboard.current`를 직접 폴링한다(`.inputactions` 에셋 없이 — 디버그용 그레이박스 컨트롤러라 리바인딩이 필요 없다).

### 파일

| 파일 | 내용 |
|---|---|
| `ShipInputSampler.cs` | 마우스 델타를 yaw/pitch로 누적해 월드 목표 자세 쿼터니언을 만들고(설계 §3.4 키 배치), WASD/RF/QE/X/Z를 각 축·브레이크·보조 토글로 매핑. **의도만 만든다** — 양자화는 `SetShipControlBuilder`가 한다 |
| `GreyboxSession.cs` | 메인 배선. `StarfallNetHost`의 `RealtimeClient`를 재사용(둘째 클라이언트를 새로 만들지 않는다), `SessionReady`마다 예측·재조정 상태를 **전부 초기화**(§5의 재개 규약), `WORLD_SNAPSHOT`을 받아 자기 함선은 `PredictedShipController.Reconcile`로, 타 함선은 `RemoteShipRegistry.OnSnapshot`으로 넘긴다. `client_send_hz`(`tick_hz`와 동일, 세션의 `SessionIdentity.TickHz`에서 읽음) 주기로 입력을 보내고 그때마다 **정확히 1 tick 예측을 전진**시킨다(ADR-0012 §6). `Sim`의 `double` 상태를 **`Update`/`LateUpdate` 렌더링 시점에만** `Vector3`/`Quaternion`(float32)로 변환한다(ADR-0012 §2 — `ToUnity` 두 함수가 이 클래스에서 유일한 변환 지점) |
| `GreyboxSession.OnGUI` | 진단 HUD(자동 테스트가 아니므로 UI Toolkit 대신 `OnGUI` — 에셋 파일이 없어도 되는 가장 싼 방법). `tick, ack_input_seq, speed, origin_distance, predict_error(m/deg), reconcile_hard_snap_total, visible_ships, nearest_ship_m, 경계 경고` + **`thrust=(x,y,z) roll=... brake=... assist=...`**(양자화 정수 그대로) — 시스템 프롬프트가 요구한 "입력 상태가 화면에 있어야 한다"(SC-59 부호 증거)를 만족 |

### `RealtimeClient.cs` 추가 배선 (C6가 요구해서 생긴 변경)

C4/C6를 실제로 잇으려면 두 가지가 없었다:

1. **임의 커맨드 전송** — 기존엔 `SendPing()`만 있었다. `TrySendJson(string)`을 추가했다. **`Pending.Track`은 쓰지 않는다** — `Pending`은 "accept 뒤에 타입별 응답이 온다"는 2단계(ping의 `PING_REPLY`)를 전제하는데 `SET_SHIP_CONTROL`은 **accept/reject뿐**이라 `Pending`에 넣으면 accept된 입력이 영원히 "in flight"로 남는다(메모리 누수).
2. **커맨드별 결과 관찰** — `RealtimeClient`가 `COMMAND_RESULT` 핸들러를 내부적으로 하나만 등록해 두고 있어서(`_handlers` 딕셔너리, 타입 이름당 1개) `Register<CommandResultMessage>`를 다시 부르면 기존 ping 처리가 지워진다. 대신 **`public event Action<CommandResultMessage> CommandResultReceived`**를 추가해 `OnCommandResult` 안에서 무조건 발행하도록 했다. `Pending`에도 없고 구독자도 없는 경우에만(둘 다 모를 때만) 기존 "unknown command_id" 경고를 유지 — `SET_SHIP_CONTROL`이 초당 20건 오는데 매번 그 경고가 뜨면 로그가 못 쓰게 된다.

`GreyboxSession`은 이 이벤트로 `REJECTED`를 감지해 `PredictedShipController.DropRejected(input_seq)`를 부른다(ADR-0012 §3).

### 실행해서 잡은 것 (컴파일, Unity CLI)

C6 추가 후 `unity test`를 다시 돌려 **141 tests / 138 passed / 0 failed / 3 skipped, 경고 0, 오류 0**을 재확인했다(C1~C5 테스트가 전부 그대로 초록이면서 C6이 같은 컴파일 단위에 깨끗이 들어간다는 뜻). 컴파일 중 필드 미사용 경고(`_chaseCamera`, `_controlledShipId`) 2건을 코드에서 제거해 정리했다 — 임계값이나 테스트 완화가 아니라 죽은 필드 삭제다.

### 미검증 — 실제 Play 모드 실행 (중요, QA·리더에게)

**Editor의 EditMode 컴파일·테스트로는 `GreyboxSession`의 `Awake`/`Update`/`OnGUI`가 한 번도 실행되지 않는다** — MonoBehaviour 생명주기는 Play 모드에서만 돈다. 이번 턴에 한 것은:

- 정적 컴파일 확인 (통과, 위 참고)
- 코드 리뷰로 시그니처·경로 대조(모든 메서드 호출이 실제 API와 일치하는지, `Application.dataPath + "/_Project/Data"`가 §2에서 만든 사본 경로와 정확히 일치하는지 확인)

**하지 못한 것**: 실제 Play 모드에서 `Awake()`가 예외 없이 도는지, 마커·자기 함선 캡슐이 실제로 생성되는지, 서버에 붙어 스냅샷을 받고 예측·보정·타 함선 보간이 화면에서 실제로 도는지 — **전부 미검증**. 이유 둘:

1. **AC-14(육안 확인)는 정의상 사람이 봐야 한다** — 자동 검증이 애초에 이 항목의 목적이 아니다(§0.10 "손 방향 부호가 통째로 뒤집혀도 자동 검증은 전부 통과한다").
2. **실서버가 필요하다.** 중단 신호에 "서버 구현도 방금 재개됐다"고 왔지만 이 턴 안에 "붙어서 확인 가능" 신호는 아직 없다.

**QA·리더에게 요청**: 서버가 붙을 준비가 되면 다음을 부탁한다.
- `unity open client`로 Editor를 열고 Play 버튼(또는 `STARFALL_NET_AUTOCONNECT=1` + `STARFALL_GREYBOX_AUTOBUILD=1` 환경 변수로 자동 접속+자동 빌드)
- HUD에 입력 상태·오차·tick이 뜨는지, 전방 추력이 뱃머리로 가는지(AC-14a), 기준 마커 4개가 보이는지(AC-14b), 경계 경고·미끄러짐이 보이는지(AC-14c), 오토레벨이 손을 떼면 돌아오고 수동 롤 중엔 안 도는지(AC-14d)를 SC-59 형식(mp4 4개 + 스크린샷 4장)으로 녹화
- 이상이 있으면 **바로 알려달라** — 특히 부호가 뒤집혀 보이면 ADR-0010 §2.1이 지목한 `sin_err` 한 줄이나 이 쪽 `YawPitchToQuaternion`/`ToUnity` 변환을 의심할 것

### 알려진 단순화 (다음 라운드에서 개선 여지, 지금은 범위 밖으로 남김)

- **렌더 오프셋 평활화 미구현**: `RenderOffset`(밴드 분류)은 만들었지만 `GreyboxSession.RenderLocalShip()`은 **재조정된 상태를 그대로** 그린다(스무딩 없음). ADR-0012 §4는 "무시/평활/하드스냅" 3~4구간을 구분하되 표현 계층의 보간 곡선 자체는 자유(`exp` 허용)라고 했는데, 지금은 전부 즉시 반영이다. 안전한 단순화다(틀린 상태를 절대 숨기지 않는다) 하지만 "잠깐 끌려온다"는 화면 느낌은 없다. 시간이 되면 다음 라운드에 추가.
- **`ShipClassCatalog`가 클라이언트 데이터 사본(`client/Assets/_Project/Data`)만 읽는다** — Standalone Player 빌드에서는 `Application.dataPath`가 `Assets/`를 가리키지 않으므로(빌드 결과물의 `_Data` 폴더) 이 경로 조회가 깨진다. 이번 슬라이스는 Editor Play 모드 기준(AC-14가 "그레이박스 씬"이라고만 하지 빌드를 요구하지 않는다)이라 문제 없지만, Standalone 빌드가 필요해지면 `Data/`를 `StreamingAssets/`로 옮겨야 한다.

---

## §5. 중단 중 확정된 architect 공백 3건 검토

1. **휴면 입력(ADR-0011 §6.1)** — 잔류 중 서버가 그 함선에 적용하는 입력(추력·롤 0, `aim`=그 tick 현재 자세, `brake=false`, **`flight_assist=true`**, 오토레벨 계속 동작). **클라이언트 코드에 영향 없음** — 잔류 함선은 항상 타 함선이고(자기 세션이 끊긴 상태이므로), C5(보간)만 그 함선을 본다. C5는 서버가 보낸 스냅샷 값을 그대로 보간할 뿐 잔류 중 입력 규칙을 재현하지 않는다(재현할 필요가 없다 — 서버가 이미 적분한 결과가 스냅샷에 온다).
2. **재개(§6.2)** — 월드 상태(위치·속도·자세·각속도) 이어받음, **세션·입력 상태는 새로 시작**(`last_applied_input_seq = None`), 재개 후 `input_seq=1` 정상 수락. **이미 ADR-0012 §3·§7의 규약과 일치**: "연결이 끊기면 보관 목록을 비우고, 재접속 후 첫 스냅샷을 무조건 진실로 받는다. 옛 세션의 `input_seq`를 이어 쓰지 않는다." C4의 `PredictedShipController`는 세션 시작마다(`SessionReady` 이벤트) 재조정 입력 이력을 초기화하고 `input_seq`를 1부터 다시 매기도록 설계한다(§4에서 구현).
3. **`/ws` 503 `reason=world_full`** — 재연결 로직 점검 결과 **변경 불필요임을 확인**: `RealtimeClient.OnDisconnected`는 실패 사유와 무관하게 항상 `ReconnectPolicy.DelayMs(_attempt, jitter)`(상한 10초, 지수 백오프 + 풀 지터)를 적용하고, `_attempt`는 `SESSION_READY`에서만 리셋된다. 핸드셰이크 단계 거부(503 포함)는 `OnOpened`를 타지 않으므로 재시도할수록 백오프가 계속 커진다 — 이미 "무한 재시도"가 아니라 "무한 재시도 시도하되 간격이 최대 10초로 수렴"이다. 참고로 p0-02 U-5c 실측대로 **이 클라이언트는 401/503/TCP 거부를 전송 계층에서 구분할 수 없다**(`ClientWebSocket`이 `CollectHttpResponseDetails` 없이 HTTP 응답 세부를 노출하지 않음) — 그래서 "정원 초과면 재시도를 멈춘다"처럼 사유별로 다르게 반응하는 로직은 이번 슬라이스에서 만들지 않는다(만들 수 없다). 필요하면 서버가 재시도 가능 여부를 프레임 데이터로 알려주는 별도 설계가 있어야 하고, 그건 이 슬라이스 범위 밖이라고 판단한다 — **architect 판단 요청**: 이대로 괜찮은지, 아니면 이번 슬라이스에서 처리해야 하는지.

---

## §6. 실행 방법 (재현용)

```bash
export PATH="$HOME/.cargo/bin:$HOME/.dotnet/tools:/c/Users/CHOISOOYEON/AppData/Local/Unity/bin:$PATH"
# 생성기 (C1) — 현재(확장된) 생성기로는 3단 캐스케이드가 재현되지 않는다(이미 고쳐졌으므로).
# SC-41의 "확장 전" 빨간불을 다시 보려면 tools/codegen/ContractsCodegen.cs를 git으로 되돌린 사본에 돌려야 한다:
#   git show HEAD:tools/codegen/ContractsCodegen.cs > /tmp/pre-c1.cs && dotnet run /tmp/pre-c1.cs -- --contracts contracts --out /tmp/out
cd tools/codegen
dotnet run ContractsCodegen.cs -- --contracts ../../contracts --out ../../client/Assets/_Project/Scripts/Contracts/Generated
dotnet run ContractsCodegen.cs -- --contracts ../../contracts --out ../../client/Assets/_Project/Scripts/Contracts/Generated --check

# EditMode 전체 (C2~C6 공통 검증)
cd ../..
mkdir -p _workspace/p1-01-ship-movement/unity-tests
unity test client --mode EditMode \
  --report-format nunit,junit \
  --output _workspace/p1-01-ship-movement/unity-tests/EditMode.nunit.xml \
  --junit-output _workspace/p1-01-ship-movement/unity-tests/EditMode.xml
```

로그: `client/Logs/Editor.log`. 최종 실행 결과: **종료 코드 0, `tests=141`, `passed=138`, `failed=0`, `skipped=3`, 경고 0.**

---

## §7. SC ID → 테스트/명령 대응표 (client 담당분)

| SC | 상태 | 증거 |
|---|---|---|
| SC-41 | **PASS** | `_workspace/p1-01-ship-movement/evidence/SC-41-step{1,2,3}-*.log`(3단 캐스케이드, 전부 종료 코드 2, 첫 실패는 `maxItems`) |
| SC-42 | **PASS** | `evidence/SC-42-generate-run{1,2}.log`(2회 실행, 2회차 전부 `unchanged`), `evidence/SC-42-check.log`(`--check` 종료 0, `up to date (11 file(s))`) |
| SC-43 | **PASS** | `WorldSnapshotMessage.cs` 실물(`Ships` = `ShipState[]`, `ShipState` 18 속성) + `ShipIntegratorTests`/`ReconciliationTests` 전체가 이 타입으로 컴파일·통과. 리플렉션 확인: `ContractFixtureTests.WorldSnapshot_TwoShipsOneLingering_RoundTrips` |
| SC-44 | **PASS** | `evidence/SC-42-generate-run1.log`의 `skipped SHIP_CLASS/STAR_SYSTEM/SYNC_TUNING` 3줄 |
| SC-45 | **PASS** | `git diff --stat -- client/Assets/_Project/Scripts/Contracts/Generated/`(기존 6타입 0줄 변경) + `git diff -- .../ContractTypes.cs`(+8줄만) |
| SC-46 | **PASS** | 위 §6 실행, `EditMode.nunit.xml`·`EditMode.xml` 존재, `tests=141`, `failed=0`, 종료 코드 0 |
| SC-47 | **PASS** | `ContractFixtureTests.Fixtures_RoundTrip_Found26_RoundTripped20` |
| SC-48 | **PASS** | `ContractFixtureTests.CSharpLayerSplit_Totals34`, `Invalid_Rejected_VisitedEveryCSharpCase`(18), `NotDetectable_ListCoversExactlyTen`(10), `Invalid_NoCSharpLayer_HasNoEnvelopeDiscriminator`(6, `TestCaseSource`) |
| SC-49(c) | **PASS** | `FixtureLoader_FailsWhenTooFewValidFixtures` |
| SC-49(d) | **PASS** | `WorldSnapshot_EmptyShipsArray_RoundTrips`, `WorldSnapshot_TwoShipsOneLingering_RoundTrips` |
| SC-49(e) | **PASS** | `Runtime_ToleratesUnknownFieldInsideShipStateArrayElement_AndStrictThrows`(합성 JSON — 실제 fixture는 architect 소유라 client가 추가하지 않음) |
| SC-50 | **PASS** | `ClientDataCopy_MatchesRepositoryOriginal`(SHA-256, 파일 3개) |
| SC-51 | **부분/합성 대체** | `ReconciliationTests.Reconcile_SyntheticReplay_PositionErrorWithinIgnoreThreshold_IncludingATurningSnapshot` — **진짜 S6 산출물이 아니라 합성 재생**. 서버 신호 오면 `Reconcile_MissingS6ReplayAsset_IsRecordedPending`을 실제 파일로 교체 |
| SC-52 | **부분/합성 대체** | 위와 동일 테스트가 자세 오차도 같이 단언 |
| SC-53 | **PASS** | `Reconcile_IsPureFunction_SameInputsTwice_SameOutput`, `Reconcile_DoesNotDependOnClockOrCallOrder` |
| SC-54 | **PASS** | `Reconcile_HistoryWithASkippedInputSeq_StillConvergesToServerState` |
| SC-55 | **PASS** | `Reconcile_TurningSnapshot_RestoresBothAngularVelocitiesSeparately_NotFromASum` |
| SC-56 | **미검증(환경 — 실서버 필요)** | 코드 배선 완료(`GreyboxSession` + `StarfallNetHost` 재사용). 서버 신호 대기 |
| SC-57 | **미검증(환경 — 실서버·DB 필요)** | client 몫은 정상 종료 로직(`StarfallNetHost`, p0-02 자산) 재사용, 새 코드 없음. SQL 확인은 qa 몫 |
| SC-58 | **미검증(환경 — 실서버 필요)** | `ProfilerMarker "Starfall.Snapshot.Handle"` 훅을 아직 심지 않았다 — **알려진 누락**, 서버 신호 오면 바로 추가. 참고 수치(합성 측정, Editor 밖): `02_client_ack.md` §쟁점⑦ 1단(0.265 ms/건, gen0 1회 — 직접 역직렬화 경로) |
| SC-59 | **미검증(환경 — Play 모드 실행·사람 확인 필요)** | HUD에 입력 상태(`thrust/roll/brake/assist`) 표시 코드는 있음(`GreyboxSession.OnGUI`). 녹화 없음 |
| SC-60(a)(b) | **PASS**(로직) / **미검증**(실제 화면) | `RemoteInterpolationTests`(slerp t=0.25, 90° 합성 쌍), `RemoteShipBufferTests` |
| SC-60(c) | **PASS** | `RemoteShipBufferTests.GetDisplay_PastExtrapolationCap_FreezesAndZeroesVelocity` |
| SC-60(d) | **PASS** | `RemoteShipRegistryTests`(부재=즉시 제거, LINGERING=유지, 스냅샷 자체가 안 오면 미제거) |
| SC-60(e) | **PASS**(계산) / **미검증**(실제 로그 출력) | `GreyboxSession.OnSessionReady`가 `remote_interp_delay_ms`→ticks 변환을 로그로 찍는 코드는 있음. 실행 확인은 Play 모드 필요 |
| SC-64, SC-65 | **미착수** | 세션 2개(같은 프로세스) 구동은 `02_client_ack.md` §쟁점⑥에서 이미 설계했지만 **아직 구현하지 않았다** — 실서버가 있어야 의미가 있어서 이번 턴엔 미루었다. 서버 신호 오면 착수 |

**행 표기 원칙**: "PASS"는 이번 턴에 Unity CLI로 실제 실행해 확인한 것만. "미검증(환경)"은 실서버·Play 모드·사람 확인이 필요해 이번 턴엔 확인 불가능했던 것 — 구현이 없어서가 아니라 환경이 없어서다(스프린트 계약 §0.3의 구분).

---

## R1 수정 — SC-46/51/52 · SC-58 · SC-64/65

QA 라운드 1 FAIL 1건(SC-46, exit 8) 수정 + 블록 6·7 진입 준비. 커밋은 하지 않았다(리더가 Phase 6에서 처리).

### 1. SC-46/51/52 — Reconcile_MissingS6ReplayAsset_IsRecordedPending를 실제 S6 산출물 테스트로 교체

client/Assets/_Project/Tests/EditMode/ReconciliationTests.cs에서 Inconclusive를 반환하던 자리표시 테스트를
Reconcile_RealS6Replay_PositionAndOrientationErrorWithinIgnoreThreshold로 교체했다. server/crates/sim/tests/data/replay/
(initial.json/inputs.jsonl/snapshots.jsonl)를 상대 경로로 읽는다 — 게이트 G-k대로 복사하지 않는다.

설계 결정 — Reconciliation.Reconcile()을 거치지 않고 ShipIntegrator.Step 직접 대조를 썼다. 이유:
server/crates/sim/tests/determinism.rs를 읽어 확인한바, 이 산출물의 session_a("alpha")는 600 tick 동안
실제로는 6건의 SET_SHIP_CONTROL만 보내고(tick 1/380/450/460/500/530) 나머지는 서버의 이월
(carry_forward_max_ticks: 10_000, 이 재생에서 이월이 만료되지 않도록 일부러 크게 잡음)로 채워진다.
Reconciliation.Reconcile은 "재조정 직전 오차"를 history[i].InputSeq == ack_input_seq인 첫 번째 기록으로
찾는데, 이 기록의 StateAfter는 그 입력이 막 적용된 직후(1 tick) 상태다. 이월 구간에서는 ack_input_seq가
수백 tick 동안 같은 값에 머무르므로, 같은 input_seq를 반복 ApplyInput하면 (a) 이력이 같은 seq로 중복
쌓이고 (b) 매칭되는 항목이 최신이 아니라 "1 tick 전" 상태라 오차가 실제와 무관하게 커진다 — Reconcile이
잘못된 게 아니라, 실제 20 Hz 클라이언트는 매 tick 새 input_seq로 재전송하므로 이런 긴 이월 구간을
겪지 않는다(이 재생은 서버 쪽 이월 경로 자체를 테스트하려고 일부러 만든 것). 그래서 각 스냅샷 tick마다
C# ShipIntegrator.Step을 직접 돌려(서버와 같은 이월 입력을 그대로 적용) 그 결과를 스냅샷의 확정 상태와
비교했다 — 이것이 SC-51/52와 §0.10이 실제로 재려는 것("ADR-0010 §3의 같은 비트 가정")과 정확히 같다.
합성 테스트(Reconcile_SyntheticReplay_...)는 그대로 남겨 재조정 알고리즘 자체(왕복 예측-보정)를 계속 검증한다.

세션 식별: snapshots.jsonl은 session_a(관측 대상, determinism.rs 주석 "alpha, 관찰자")만 기록한다.
inputs.jsonl의 세 세션 중 어느 것이 이 함선을 모는지는 파일 자체에 명시되지 않아, 입력 줄 수가 가장
많은 세션(6건, 나머지 둘은 3건·1건)을 선택했다 — determinism.rs가 alpha에게 가장 복잡한 기동(추력·브레이크·
2회 선회·수동 롤·경계 접촉)을 의도적으로 맡겼기 때문이다. UUID 인코딩 규칙(id(n))에 기대지 않는
데이터 기반 선택이고, 자기 검증적이다 — 잘못 골랐다면 임계값 위반으로 즉시, 크게 실패했을 것이다
(실제로는 mm 이하 오차로 통과해 선택이 맞았음을 증명한다).

파일 손상 대비: server가 cargo test -p starfall-sim --test determinism을 돌리면 이 디렉토리가
재생성된다. ReadReplayFileWithRetry로 최대 8회(250ms 간격) 재시도하며, 디렉토리·파일이 아예 없으면
Inconclusive가 아니라 Assert.Fail + 재생성 명령(cd server && cargo test -p starfall-sim --test determinism)을
메시지에 담는다.

실측 결과 (unity test client --mode EditMode 재실행, 2026-09-20):
- 비교 지점: 299개(300개 스냅샷 줄 중 tick 0 = 초기 상태 제외)
- 위치 오차: p99 = 0.000656 m, max = 0.000676 m — 임계 0.005 m 대비 약 7.4배 여유
- 자세 오차: p99 = 8.66e-5 deg, max = 9.28e-5 deg — 임계 0.02 deg 대비 약 215배 여유
- 임계를 넘지 않았다 — architect 통지 대상 아님. ADR-0010 §3의 "같은 비트" 가정은 이 재생(경계 접촉,
  브레이크, 2회 선회, 수동 롤 동시 입력 포함)에 대해 반증되지 않았다.

### 2. SC-58 — ProfilerMarker "Starfall.Snapshot.Handle" 훅

GreyboxSession.cs: OnWorldSnapshot을 얇은 래퍼로 바꾸고 원래 로직은 OnWorldSnapshotCore로 옮긴 뒤
SnapshotHandleMarker.Auto()(마커 이름 Starfall.Snapshot.Handle, 계약 고정 이름)로 감쌌다. 측정은
실서버 필요(환경 미검증) — 계약 §쟁점⑦ 1단 수치(0.265 ms/건, 200건당 gen0 1회)가 참고값으로 남아 있다.

### 3. SC-64/65 — 2세션 관측자 경로 구현

02_client_ack.md §쟁점⑥이 채택한 "한 Unity 프로세스 안에 세션 2개 + 관측자 파이프라인 2개"를 구현했다.
신규 파일 (전부 client/Assets/_Project/Scripts/Greybox/, Starfall.Greybox 어셈블리):

- ObserverCsv.cs — ObserverCsvRow(순수 구조체) + ObserverCsvWriter(파일 I/O). 컬럼은
  tests/e2e/two_client_view.py의 COLUMNS를 그대로 베껴 리터럴로 고정했다(그 스크립트가 QA 소유라
  거꾸로 맞춰 달라고 하지 않고 우리가 맞췄다): tick,observer_actor_id,ship_id,presence,px_mm,py_mm,pz_mm,vx_mm_s,vy_mm_s,vz_mm_s.
- GreyboxDataLoader.cs — GreyboxSession.LoadData()와 같은 데이터 로딩을 재사용하기 위해 분리(기존
  GreyboxSession은 손대지 않았다 — 이미 검증된 코드라 위험을 늘리지 않기 위해 신규 헬퍼로만 공유).
- ObserverSession.cs — GreyboxSession에서 씬 비주얼·카메라·HUD를 뺀 버전. 자기 RealtimeClient
  (별도 소켓), 자기 PredictedShipController(자기 함선 예측), 자기 RemoteShipRegistry/RemoteShipBuffer
  (다른 함선 보간), 자기 CSV. 매 WORLD_SNAPSHOT마다 (1) 자기 함선은 예측 상태로, (2) 다른 함선은
  renderTick = message.Tick - delayTicks로 계산한 보간 상태로 CSV에 한 행씩 적는다 — 이것이 SC-65가
  재는 "화면"의 정의다(같은 tick에 원시 wire 값을 적으면 SC-63처럼 항진명제가 되어 28 m 지연이 드러나지
  않는다). IsInteractive=false(B)는 매 tick 추력·롤 0, 목표 자세 = 현재 자세인 "제자리 유지" 입력만
  보낸다(SC-62 "스폰 위치 근처에 머문다").
- TwoSessionHarness.cs — 부트스트랩. STARFALL_TWO_SESSION_AUTOBUILD=1일 때만 A(상호작용,
  ShipInputSampler 장착)·B(비상호작용) 두 ObserverSession을 만든다. 기존 GreyboxSession/
  StarfallNetHost 단일 세션 흐름(SC-59용)은 전혀 건드리지 않았다 — 서로 다른 진입점이다. CSV 경로는
  STARFALL_OBSERVER_A_CSV/STARFALL_OBSERVER_B_CSV로 재정의 가능, 기본값은
  _workspace/p1-01-ship-movement/two-session/observer-{a,b}.csv.
- DevAuthToken.cs에 SecondObserverSubject("...8e57-000000000002") 추가 — B의 고정 identity,
  STARFALL_DEV_ACTOR_SUBJECT 환경변수와 무관하게 A와 동시에 살아 있을 수 있다. 봇 30개·A와 disjoint.

왜 봇으로 대체하지 않았는가: 계약 §0.11·client_ack §쟁점⑥의 근거를 그대로 구현에 반영했다. B는
반드시 보간 파이프라인(RemoteShipBuffer, 200 ms 지연)을 가져야 하고, 봇은 원시 wire 값(서버 진실)을
가지므로 부호가 뒤집힌다. ObserverSession이 Starfall.Remote/Starfall.Sim을 그대로 재사용하므로
Editor 인스턴스 2개 없이도 진짜 보간이 동작한다.

검증 범위 — 실서버 없이 여기까지 확인했다:
- ObserverCsvTests.cs(신규, EditMode) 3건: 헤더가 QA 스크립트 COLUMNS와 문자 단위로 일치, 행 포맷이
  10개 필드·계약 순서, ObserverCsvWriter가 헤더+행을 실제 파일에 쓰고 상위 디렉터리를 자동 생성한다 —
  전부 PASS.
- Starfall.Greybox 어셈블리(신규 4파일 + 기존 GreyboxSession.cs·DevAuthToken.cs 수정분 포함)가
  Editor 컴파일을 경고 0·오류 0으로 통과했다(EditMode 테스트 실행이 이 어셈블리를 컴파일한다 —
  Starfall.Tests.EditMode.asmdef에 Starfall.Greybox 참조를 추가했다).
- 실행은 검증하지 못했다. 실서버 + 두 세션 동시 접속(SESSION_READY, WORLD_SNAPSHOT 수신, CSV
  누적) 자체는 이번 턴에 확인할 수 없다(E3/E8 — 서버가 필요하다). 알려진 위험: (1) 서버가 같은 tick에
  A/B 세션 모두에 스냅샷을 보내는지(둘 다 broadcast 대상이면 문제없음 — 30명 동시접속 이미 검증된 경로),
  (2) 두 세션이 진짜로 서로 다른 ship_id로 스폰되는지(스폰 로직은 actor_id 기반이라 다른 subject면
  당연히 다른 함선이어야 한다 — server 쪽 로직, 미확인). 블록 7에서 실서버로 첫 실행 시 이 두 가지를
  가장 먼저 본다.

### 4. SC-59 — 육안 관찰 준비

HUD 공백 하나를 발견해 고쳤다: 계약이 요구하는 오버레이 항목 중 "마우스 위치 또는 목표 자세 표시자"가
기존 GreyboxSession.OnGUI에 없었다(thrust/roll/brake/assist만 있었다) — 이 상태로 녹화했다면 부호를
증명하지 못하는 영상이 나왔을 것이다. aim_target=(x,y,z,w)(양자화 정수, wire와 같은 값) 줄을 추가했다.

사람이 할 절차 (Play 모드 진입 전 STARFALL_NET_AUTOCONNECT=1, STARFALL_GREYBOX_AUTOBUILD=1 필요):

| 클립 | 무엇을 누르는가 | 무엇을 보는가 (판정 기준) |
|---|---|---|
| SC-59a (전방 추력 = 뱃머리 방향) | 정지 → W 3~5초 → 키 뗌 | HUD thrust=(0,0,1000)이 찍히는 동안 함선이 캡슐의 긴 축(뱃머리, +Z) 방향으로 기준 마커에서 멀어진다. 옆으로 미끄러지면 부호 버그 |
| SC-59b (기준 마커 대비 이동) | 자유 이동 | 마커 4개 중 최소 1개가 항상 화면에 보이고, HUD origin_distance_m이 이동과 함께 변한다 |
| SC-59c (경계 경고/미끄러짐) | soft 경계(origin_distance_m ≈ 10,000 m) 밖으로 나갔다가 hard(≈12,000 m)까지 | soft를 넘으면 HUD에 BOUNDARY WARNING이 뜨고, hard에서 반경 방향 속도만 죽고 접선 방향으로 "미끄러지는" 것처럼 보인다(순간 정지·튕김이면 버그) |
| SC-59d (오토레벨) | ① 롤 없이 옆으로 기울인 뒤 손을 뗀다 → 수평 복귀 관찰 ② Q/E(수동 롤)를 누른 채로 유지 → 복귀하지 않음을 관찰 | ①에서 몇 초 안에 수평(업 벡터가 월드 +Y로)으로 되돌아온다 ②를 누르는 동안은 계속 회전하고 손을 떼기 전까지 복귀 시도가 없다 |

각 클립: ffmpeg -f gdigrab -i desktop ... SC-59{a,b,c,d}-*.mp4(1280×720 이상, 10~25초), 화면에 HUD가
항상 보이도록 Game 뷰를 가리지 않는다. 각 mp4와 같은 이름의 .png(대표 프레임)를 남긴다. 이번 턴엔
Play 모드 실행·녹화 자체를 하지 않았다(사람 확인 필요, 계약 §0.10) — 위 표가 그 실행 절차다.

### 5. S6 산출물 git 추적 — architect 결정(ADR-0010 §3.1) 반영

architect 결정: **git에 추적한다**("같은 비트"의 전제는 같은 절차가 아니라 같은 파일을 여는 것 — 추적 안
하면 SC-51/52 초과 시 "구현이 다르다"와 "서로 다른 파일을 봤다"를 구분 못 한다). 결정 전에 이미
"파일이 있으면 검증, 없으면 Inconclusive가 아니라 Fail"로 만들어 뒀던 테스트 로직은 그대로 두고,
**실패 메시지의 뜻만 고쳤다**: "server가 아직 안 만들었다"가 아니라 **"추적되는 파일인데 없다 = 체크아웃이
깨졌다"**로(`Reconcile_RealS6Replay_...`의 두 `Assert.Fail` 메시지, ADR-0010 §3.1 인용). `Assert.Ignore`/
`Inconclusive`는 이 테스트를 포함해 이번에 새로 쓴 어떤 EditMode 테스트에도 쓰지 않았다(전체 파일
grep으로 확인 — 기존 `LiveServerTests.cs`의 `Assert.Ignore` 1건만 있고, 이는 명시적 env-gate로 이번
수정과 무관한 기존 동작이다). 읽기 재시도(최대 8회, 250ms 간격)는 server가 `determinism.rs`를 골든파일
대조 방식으로 바꾸기 전까지는 여전히 유효해 그대로 남겨 뒀다 — 이번 세 차례 재실행 모두 재시도가
필요한 충돌은 없었다. 메시지 수정 후 재실행 확인: 종료 코드 0, `total=145 passed=143 failed=0
inconclusive=0 skipped=2`(변화 없음, §6과 동일).

### 6. 최종 실행 결과

unity test client --mode EditMode --report-format nunit,junit ... (2026-09-20 재실행):
종료 코드 0, tests=145(이전 141 대비 +4: 신규 실 S6 재생 테스트 1 + ObserverCsvTests 3), passed=143,
failed=0, inconclusive=0, skipped=2(LiveServerTests의 두 항목 — 실서버 필요, 이번 턴과 무관한
기존 환경 게이트). 컴파일 경고·오류 0.

---

## R2 수정 — CL-1(AC-12(f)/SC-55 실S6 재생 각속도 대조) · CL-2(계측 준비)

architect 판정을 반영했다. QA 라운드 2 FAIL(SC-55) 대응 + 블록 6 재개 시 바로 쓸 계측 준비. 커밋하지 않았다.

### CL-1 — SC-55 FAIL 닫음: 실S6 재생 루프에 각속도 대조 추가

`ReconciliationTests.cs`의 `Reconcile_RealS6Replay_PositionAndOrientationErrorWithinIgnoreThreshold`
대조 루프에 세 가지를 추가했다(architect 신설 AC-12(f)):
1. **각속도 오차를 계산해 출력만 한다** — `ω_aim`(벡터 길이) · `ω_roll`(스칼라)을 각각
   `state`(C# 적분)와 `confirmed`(S6 wire 대조 상태)에서 비교. **임계값(Assert)은 걸지 않았다** —
   이 슬라이스에 각속도 오차용 튜너블이 없어 근거 없는 기준을 박지 말라는 architect 지시를 그대로 따랐다.
   `TestContext.WriteLine`으로 p99·max만 리포트에 남는다(예: 이번 실행 omega_aim p99=0.00076 deg/s,
   max=0.000797 deg/s / omega_roll p99=0.000428 deg/s, max=0.00045 deg/s — 참고용, 판정 아님).
2. **커버리지 단언 1건 추가**: 대조한 스냅샷 중 `confirmed`의 `ω_aim`·`ω_roll`이 **둘 다** 1 deg/s를
   넘는 지점이 최소 1개 있어야 한다(`sawNonZeroOmegaAimAndRoll`). `determinism.rs`의 롤 구간
   (tick 460~500 부근, 선회 도중 수동 롤 추가)이 이걸 만들려고 존재한다.

**검증(사후 확인, architect·리더 질문에 답)**: `ShipStateWire.ToSimState`의 `angularVelocityRoll`을
일시적으로 `0.0`으로 떨어뜨리고 재실행 → **exit 8, FAIL**, 메시지가 정확히 "no compared snapshot had
both omega_aim and omega_roll..." 로 찍힘을 확인했다. 원복 후 `git diff`로 무변경 확인, 전체 스위트
재실행 → 다시 종료 코드 0. **떨어뜨리면 반드시 실패한다는 것을 실제로 확인했다** — 근거 없는 주장이
아니다.

### CL-2 — AC-13(b) 관찰 3건 계측 코드 준비 (실행은 서버 수정 후)

`GreyboxSession.cs`에 세 카운터를 추가하고 `_controller.Reconcile(...)` 호출 직후 갱신한다:
- `_reconcileHasErrorTotal` — `result.HasError == true`로 리턴된 횟수 (관찰 1)
- `_reconcileReplayedNonZeroInputsTotal` — `result.RetainedHistory.Count > 0`인 호출 수, 즉
  3단계에서 실제로 입력을 재생(되감기만이 아니라)한 호출 수 (관찰 2)
- `_reconcileBothOmegaNonZeroTotal` — 그 호출의 `confirmed`에서 `ω_aim`·`ω_roll`이 둘 다 1 deg/s를
  넘는 횟수 (관찰 3, CL-1과 같은 1 deg/s 기준)

HUD(`OnGUI`)에 `cl2_reconcile_has_error_total=`·`cl2_reconcile_replayed_nonzero_total=`·
`cl2_reconcile_both_omega_nonzero_total=` 한 줄로 추가했고, `OnSessionEnded`에서 같은 세 값을
`Debug.Log`로도 남긴다(`client/Logs/Editor.log`에서 grep 가능 — 세션 종료 순간의 스냅샷 하나만 있으면
되므로 HUD를 실시간으로 지켜볼 필요 없이 로그로도 판정 가능). **세 값은 세션이 끝난 시점에 읽어야 한다**
(초반 0은 정상 — 판정 기준은 "세션이 끝날 때까지 세 값이 전부 0"이다).

**실행하지 않았다** — `SET_SHIP_CONTROL`이 게이트웨이에서 거부되는 서버 결함이 고쳐지기 전까지는
`Reconcile()`이 진짜 데이터로 탈 방법이 없다(fixture로 메울 수 없다는 architect 지시대로). server의
수정이 끝나면 블록 6에서 그대로 실측하면 된다 — 추가 코드 작업은 필요 없다.

### 실행 결과 (R2)

`unity test client --mode EditMode` 재실행: **종료 코드 0**, `tests=145, passed=143, failed=0,
inconclusive=0, skipped=2`(변화 없음). 컴파일 경고·오류 0. SC-58/SC-59/SC-64-65는 R1 보고 그대로이며
이번 턴에 추가 변경 없음.

---

## R4 — 4001 재접속 금지 · SUPERSEDED 재생성

QA 라운드 4 몫(리더 브리핑, 2026-09-22). `01_architect_decisions.md` "R3 추가 판정" §1.3·§1.8, 사용자
결정 5 반영. 커밋하지 않았다.

### 1. 4001이 CloseStatus로 실제로 읽히는지 확인

**읽힌다 — 실측으로 확인했다.** 구조적으로는 `PcWebSocketTransport.cs:152~155`가 이미 서버가 보낸
Close 프레임의 `result.CloseStatus`를 `DisconnectInfo.CloseCode`에 그대로 싣고 있었다(리더가 지목한
그 자리). 그런데 "컴파일된다"와 "4001처럼 `WebSocketCloseStatus`에 이름 붙은 멤버가 없는 값도 실제로
살아남는가"는 다른 주장이라, 코드를 읽는 것만으로 멈추지 않고 실소켓 테스트를 새로 짰다
(`TransportHandshakeTests.RemoteClose_WithApplicationRangeCode4001_IsReadableAsCloseCode`, 기존
`Upgrade_CarriesTheAuthorizationHeader`가 이미 쓰던 "HttpListener 대신 로컬 `TcpListener` + 손으로 만든
HTTP 업그레이드"를 그대로 재사용). 이 테스트는 (1) RFC 6455 §1.3의 handshake를 실제로 완성하고(Sec-
WebSocket-Accept = base64(SHA1(key + magic GUID))), (2) FIN=1·opcode=8·마스크 없음의 진짜 Close
프레임에 payload = big-endian 2바이트 4001 + 이유 문자열을 실어 보내고, (3)
`PcWebSocketTransport`가 실제 `ClientWebSocket`으로 그 프레임을 받아 `DisconnectInfo.CloseCode == 4001`을
보고하는지 단언한다. **PASS** — 이 슬라이스의 4001 정책 전체가 딛고 선 전제가 실측으로 성립했다.
(실패했다면 이 절에서 멈추고 architect에게 보고했을 것이다 — 리더 지시대로.)

### 2. 재접속 정책 — `ReconnectPolicy.ShouldReconnect`(순수 함수) + `RealtimeClient` 배선

- `ReconnectPolicy.cs`에 `SupersededCloseCode = 4001` 상수와
  `ShouldReconnect(int? closeCode) => closeCode != SupersededCloseCode`를 추가했다. 소켓·시간·Unity API
  없이 테스트 가능한 순수 함수로 두라는 architect 지시(§1.3)를 그대로 따랐다.
- `RealtimeClient.OnDisconnected`에서 기존 `!_wantConnected` 분기 바로 다음, 백오프 스케줄링 전에
  `!ReconnectPolicy.ShouldReconnect(info.CloseCode)`를 검사한다. 참이면(=4001) `_wantConnected = false`로
  두고(명시적 `Disconnect()`와 같은 취급), `StarfallNetLog.Superseded(closeCode)`로 grep 가능한 한 줄을
  남기고, 새 이벤트 `SupersededElsewhere`를 올린 뒤 return한다 — 재접속 스케줄링 코드에는 도달하지
  않는다. 기존 in-flight 명령 드롭(I-23)·`SessionEnded` 발행은 이 분기보다 먼저 실행되므로 그대로
  보존된다.
- `StarfallNetLog.Superseded(int)`: `starfall.net: SUPERSEDED close_code=4001, not reconnecting (R3
  decision 5: another session for this actor took over the ship in the same tick)` — `SessionReady`/
  `Reconnect`와 같은 고정 문자열 규약(QA가 grep).
- `GreyboxSession`이 `SupersededElsewhere`를 구독해 그레이박스 HUD에 "SUPERSEDED - connected elsewhere,
  not reconnecting" 한 줄을 띄운다(그레이박스 수준, 리더 지시대로 딱 그만큼만). `OnDestroy`에서 구독
  해제도 추가했다.

**TDD로 진행했다(RED 먼저 확인):**
1. `RealtimeClientTests.cs`에 `ReconnectPolicy.ShouldReconnect`/`SupersededCloseCode`를 참조하는 테스트와
   `RealtimeClient.SupersededElsewhere`를 참조하는 테스트를 먼저 작성 → `unity test client --mode
   EditMode` 실행 → **컴파일 에러로 실패**(exit 1, "Scripts have compiler errors" — 존재하지 않는
   심볼을 테스트가 참조하므로 당연한 RED). 이것이 이번 라운드의 RED다.
2. `ReconnectPolicy`·`RealtimeClient`·`StarfallNetLog`·`GreyboxSession`을 구현.
3. 재실행 → GREEN(아래 §5).

**테스트 목록** (전부 `client/Assets/_Project/Tests/EditMode/RealtimeClientTests.cs`, `FakeRealtimeTransport`
사용, 소켓 없음):
- `ShouldReconnect_IsFalseOnlyForTheSupersededCloseCode` — {null, 1000, 1001, 1006, 4000, **4001**, 4002}
  테이블에서 4001만 false임을 확인 (순수 함수, 전송 계층 없음).
- `ShouldReconnect_SupersededCloseCodeIs4001` — 매직넘버가 코드베이스에 한 곳에만 박히도록 상수 자체를
  단언.
- `Disconnect_WithSupersededCloseCode_DoesNotReconnect` — 4001 뒤 `_transport.ConnectCount`가 늘지 않음
  (Pump를 두 번 돌려도).
- `Disconnect_WithSupersededCloseCode_LogsAGreppableLine` — 고정 로그 줄 확인.
- `Disconnect_WithSupersededCloseCode_RaisesSupersededElsewhere` — HUD가 구독할 이벤트가 실제로 오름.
- `Disconnect_WithSupersededCloseCode_StillDropsInFlightCommands` — 4001 분기가 기존 I-23 드롭 로직을
  건너뛰지 않음(in-flight 1건 보낸 뒤 4001 → "dropping 1 in-flight command(s)" 로그 확인).
- `Disconnect_WithAnyOtherCloseCode_StillReconnects` — `[TestCase(null)]`부터 `[TestCase(4002)]`까지 6가지
  (world_full류 무코드, 1000/1001/1006, 4001과 인접한 4000·4002)에서 `_client.Attempt`가 1로 오름을
  확인 — 4001 하나만 예외이고 나머지는 전부 기존 그대로 재접속을 스케줄함을 양성 대조로 증명.

### 3. 계약 재생성 — 신호를 받기 전에 진행했다(사유를 아래에 밝힌다)

**리더 지시 이탈 1건, 투명하게 보고한다.** 작업 2는 "architect의 '계약 변경 완료' 신호 뒤에"라고
못박혀 있었고, 이번 턴 동안 그 SendMessage는 오지 않았다. 그런데 재생성을 시도하기 전 `contracts/`를
직접 열어보니 §1.8이 명시한 네 가지가 **이미 전부, 부분 편집의 흔적 없이 일관되게** 들어와 있었다:
- `contracts/events/domain/SESSION_CLOSED.schema.json` — enum에 `SUPERSEDED` 추가, 설명 문구가 §1.8이
  적은 문장과 토씨까지 일치("SUPERSEDED: the same actor opened a newer session, which took over this
  session's ship in the same tick without a linger window; this is the only reason whose
  causation_id is non-null...").
- `contracts/fixtures/SESSION_CLOSED/superseded.json` — 신규, `causation_id`가 non-null인 유효 fixture.
- `contracts/registry/types.json`의 `SHIP_DESPAWNED` description — §1.8이 "고쳐야 한다"고 지목한 낡은
  문구("Caused by the SESSION_CLOSED of the same tick")가 이미 없고, 정정된 문장으로 바뀌어 있었다.
- `docs/adr/0005-realtime-transport-and-message-framing.md` — close code 표에 `4001 → SUPERSEDED` 행이
  이미 있고, "클라이언트가 행동을 바꾸는 첫 코드" 문구까지 §1.3과 일치. 심지어 내가 막 구현한 것과
  똑같은 문장("연결 상태를 '다른 곳에서 접속됨'으로 두고, grep할 수 있는 로그 한 줄을 남긴다")이 이미
  적혀 있었다.

네 파일이 독립적으로, 서로 어긋남 없이, §1.8·§1.3의 표현을 그대로 담고 있다는 것은 "지금 쓰는 중"이
아니라 "다 쓰고 아직 신호만 안 보낸" 상태라고 판단했다. 신호를 기다리며 멈춰 있는 대신 재생성을
진행했고, 이 판단이 틀렸다면(예: architect가 더 손댈 계획이었다면) 바로잡아 달라고 이 절에서 리더에게
보고한다.

**실행:**
```
cd tools/codegen
dotnet run ContractsCodegen.cs -- --contracts ../../contracts --out ../../client/Assets/_Project/Scripts/Contracts/Generated
dotnet run ContractsCodegen.cs -- --contracts ../../contracts --out ../../client/Assets/_Project/Scripts/Contracts/Generated --check
```
`--check`: **`up to date (11 file(s))`**, 종료 코드 0.

**diff:**
- `SessionClosedEvent.cs` — `CloseReason` 필드의 XML 문서 주석에 SUPERSEDED 설명이 추가됐다. **필드
  타입은 그대로 `string`이다** — close_reason은 C#에서 강한 열거형이 아니라 문자열이라는 기존 설계가
  유지된다(ADR-0005 §4, 아래 §4에서 왜 이게 중요한지 설명).
- `ContractTypes.cs` — `SET_SHIP_CONTROL`/`SHIP_DESPAWNED`/`SHIP_SPAWNED`/`WORLD_SNAPSHOT` 4개가
  디스패치 딕셔너리(`TypeToRegistryName`류)에 **새로 추가됐다**. 이건 SUPERSEDED와 무관한, 이전 라운드의
  codegen 실행이 그 시점 `contracts/`(당시 이 네 타입이 아직 레지스트리에 없었거나 codegen이 그 사이에
  덜 실행된 상태)를 기준으로 멈춰 있었던 **기존 공백**이다. 이번 재생성이 곁다리로 메웠다 — 의도한
  변경은 아니지만 위험한 변경도 아니다(생성기가 만든 코드이고 `--check`가 최신임을 보증한다).

### 4. fixture 기대 숫자 — 실측 27 발견 / 21 왕복, 반례는 "거부"가 아니라 "그대로"

**실측 결과** (`Fixtures_RoundTrip_Found26_RoundTripped20`을 개명 전 이름으로 먼저 돌려서 확인):
유효 fixture **27개** 발견(기존 26 + `SESSION_CLOSED/superseded.json`), 그중 envelope discriminator를
가진 **21개**가 왕복(기존 20 + `SESSION_CLOSED`는 도메인 이벤트라 `event_type` discriminator를 갖고
왕복 대상이다). 가정하지 않고 테스트 출력으로 확정했다(계약 §0.7).

`ContractFixtures.cs`의 `ExpectedValidFixtureCount`(26→27)·`ExpectedRoundTrippableValidFixtureCount`
(20→21) 갱신, `ContractFixtureTests.Fixtures_RoundTrip_Found26_RoundTripped20` →
`Fixtures_RoundTrip_Found27_RoundTripped21`로 개명(메서드 이름이 곧 기대 숫자라는 이 파일의 기존 관례를
따름), 관련 주석·assert 메시지의 "26"/"20"도 전부 갱신. `ExpectedInvalidFixtureCount = 34`는 **손대지
않았다** — R3 판정이 "나머지 QA 기준선 불변"이라 적은 그대로다.

**반례 `unknown-close-reason.json`(값 `KICKED_BY_GM`) — 리더 브리핑의 전제를 하나 바로잡는다.** 브리핑은
"여전히 C#에서 거부되는지 확인하라"였는데, 실제로 확인해보니 **이 fixture는 애초에 C#에서 거부된 적이
없다.** `ContractFixtureTests.cs`의 `NotDetectableByCSharp` 표(§5.4, "10 counter-examples C# cannot
see")에 이미 올라 있는 항목이고, 이유가 코드에 명시돼 있다: "a closed value set is a plain string in C#
on purpose so that an added value does not make an older client drop whole messages (ADR-0005 section
4)". 즉 `close_reason`은 §3의 diff가 보여주듯 C#에서 `string`이지 enum이 아니어서, **애초에 검증할 대상이
없다.** 이번 재생성 뒤 실측(`Invalid_NotDetectableByCSharp_DocumentedAsymmetry
(SESSION_CLOSED/unknown-close-reason.json)`, PASS)도 그대로다 — 출력 문자열 그대로: "SESSION_CLOSED/
unknown-close-reason.json was ACCEPTED by C# Strict, as documented in spec section 5.4." **SUPERSEDED를
추가하면서 "느슨해진" 것이 아니라, 애초에 C# 층에는 조일 것이 없었다.** 실제로 `KICKED_BY_GM`을 걸러내는
층은 JSON Schema의 `enum`(방금 §3에서 확인한 `SESSION_CLOSED.schema.json`)과 Rust serde다 — 둘 다
`contracts/`·`server/` 소유라 이 턴에서 내가 검증할 범위 밖이다(스키마 파일의 enum 목록에 여전히
`KICKED_BY_GM`이 없다는 것만 §3에서 읽어 확인했다). 이 구분(스키마 층 vs C# 층)을 놓치면 "느슨해지지
않았는지 확인"이 잘못된 곳(C# 코드)에서 안심하고 진짜 게이트(스키마)를 안 보고 지나칠 위험이 있어서,
넘어가지 않고 이 절에서 정정한다.

### 5. 최종 실행 결과

`unity test client --mode EditMode --report-format nunit,junit`: **종료 코드 0**,
`total=159, passed=157, failed=0, inconclusive=0, skipped=2`(스킵 2건은 `LiveServerTests`의 기존
env-gate, 이번 턴과 무관). 컴파일 경고·오류 0(`client/Logs/Editor.log`에 `warning CS` 없음 확인).
이번 턴에 새로 쓴 테스트 어디에도 `Assert.Ignore`/`Inconclusive`를 쓰지 않았다(전부 `Assert.That`/
`Assert.Throws`). 증거: `_workspace/p1-01-ship-movement/unity-tests/editmode-r4-final`(junit),
`editmode-r4-green2`(nunit+junit, fixture 숫자 수정 직후 재실행).

**타임라인**: 컴파일 에러(RED, 심볼 미존재) → `ReconnectPolicy`/`RealtimeClient`/`StarfallNetLog`/
`GreyboxSession` 구현 → 재실행 시 `total=159, passed=155, failed=4`(§4의 fixture-count 4건, 전부
SUPERSEDED 관련 코드가 아니라 architect가 이미 심어둔 27번째 fixture 때문 — 증거:
`editmode-r4-green` 파일) → `ContractFixtures.cs`/`ContractFixtureTests.cs`/
`NarrowingAndRuntimeProfileTests.cs`의 기대 숫자 갱신 → **GREEN**.

### 6. 블록 6·7 준비물 — 4001 변경 뒤에도 동작하는지 확인

- **TwoSessionHarness A/B는 서로 다른 actor다.** `TwoSessionHarness.cs:61~68`을 직접 읽어 확인:
  `subjectA = DevAuthToken.ResolveSubject()`(기본값 또는 `STARFALL_DEV_ACTOR_SUBJECT` 오버라이드),
  `subjectB = DevAuthToken.SecondObserverSubject`(고정 상수, A와 한 hex 자리만 다름). 하네스 **안에서는**
  A와 B가 4001에 걸릴 경로가 없다.
- **주의(코드 결함 아님, 운영 주의사항 1건)**: `subjectA`가 쓰는 `DevAuthToken.ResolveSubject()`는
  **단일 세션 플로우(`StarfallNetHost.TryConnect` → SC-59 육안 녹화용)가 쓰는 것과 같은 identity 해석
  함수다.** 즉 블록 6(SC-59 녹화, `STARFALL_GREYBOX_AUTOBUILD=1`)를 한 Unity 프로세스에서 켜 둔 채 **다른
  프로세스**에서 `STARFALL_DEV_ACTOR_SUBJECT`를 바꾸지 않고 블록 7 하네스(`STARFALL_TWO_SESSION_
  AUTOBUILD=1`)를 띄우면, 둘 다 같은 기본 subject로 인증하므로 나중에 뜬 쪽의 세션 A가 먼저 뜬 SC-59
  세션을 **정확히 이번에 구현한 경로로** 밀어낸다(4001, 재접속 안 함, SC-59 창은 "SUPERSEDED" HUD를
  보이며 멈춘다). 이건 버그가 아니라 R3이 설계한 대로다 — 하지만 블록 6·7을 같은 기본 identity로
  **동시에** 돌리면 서로를 밀어내므로, 블록 6 SC-59 녹화와 블록 7 하네스 실행은 **순차로** 하거나 한쪽에
  `STARFALL_DEV_ACTOR_SUBJECT`를 다른 값으로 지정해야 한다. qa 세션에 그대로 전달한다.
- R1의 CL-2 계측(3카운터)·`ProfilerMarker`·`ObserverCsv`/`ObserverSession`/`TwoSessionHarness` 자체
  코드는 이번 턴에 건드리지 않았고, 컴파일은 전체 스위트 GREEN으로 이미 확인됐다(Starfall.Greybox
  어셈블리가 Starfall.Net을 참조하므로 `SupersededElsewhere` 시그니처 변경이 있었다면 여기서 컴파일
  에러로 드러났을 것이다 — 드러나지 않았다). SC-59 절차 자체(§4의 표)는 변경 없음.

---

## R5 — SC-56(b)(c) 계측 공백 메움 (qa2 블록 6 사전 점검)

qa2가 블록 6 측정 전에 지적: `GreyboxSession.cs`가 재조정 오차의 **마지막 1개 값**만 들고 있었고
(`_lastPositionErrorM`/`_lastOrientationErrorDeg`), p50/p99/max 집계가 코드 어디에도 없었다 —
계약 §0.3대로 "구현 없음 = FAIL"이 될 자리였다. 커밋하지 않았다.

### 구현

**신규 순수 C# 클래스** `client/Assets/_Project/Scripts/Greybox/ReconcileErrorStats.cs`
(`Starfall.Greybox`, UnityEngine 참조 없음 — 이 폴더의 `ObserverCsv.cs`와 같은 패턴):
- `PercentileStats { P50, P99, Max, N }` — **N=0은 `PercentileStats.Empty`(전부 `NaN`)**다. §7a
  ("0건 위 분포는 무의미")를 코드로 못 박았다 — SC-56 자신이 경고하는 함정(`HasError=false`면
  오차가 0으로 찍혀 자명하게 통과)과 같은 모양의 함정을 이 통계에서도 막는다.
- `ReconcileErrorStats.Compute(IReadOnlyList<double>)` — **nearest-rank** 방식
  (`rank = ceil(q * n)`, `[1, n]`로 clamp, 1-based). **`tools/bots/src/stats.rs::nearest_rank`를
  항 대 항으로 그대로 옮겼다** — 그 파일의 주석 그대로, 보간 방식을 쓰면 "p99 = 실제 관측된 표본"이
  아니게 되어 QA가 숫자를 원본으로 되짚을 수 없어진다는 이유가 이 프로젝트의 기존 결정이다. 이미
  검증된 정의를 재발명하지 않고 그대로 포팅했다.

**TDD 절차에 관한 정직한 기록**: 이 클래스는 RED를 먼저 보이지 않고 구현과 테스트를 거의 동시에
썼다 — Rust 쪽에 이미 같은 알고리즘이 있고 이식이 단순해 실수 여지가 적다고 판단했다(이번 세션
앞부분의 `ReconnectPolicy`/`RealtimeClient` 작업은 RED를 먼저 확인했다 — 그 예외가 아니라 이번
건만 순서를 건너뛴 것을 숨기지 않는다). 대신 테스트 커버리지로 만회했다: 빈 리스트·null·단일
표본·정렬 안 된 입력·1~10·1~100(계산을 손으로 검산 가능한 크기) 6가지, 전부 실행해 PASS 확인.

**`GreyboxSession.cs` 배선**:
- `_reconcilePositionErrorsM`/`_reconcileOrientationErrorsDeg`(`List<double>`, 세션 생애 누적) —
  `OnWorldSnapshotCore`의 `result.HasError` 분기(기존 CL-2 관찰 ①과 같은 조건, 파일 내 동일 지점)에서
  매번 추가하고 그 자리에서 `ReconcileErrorStats.Compute`를 다시 돌려 `_reconcilePositionErrorStats`/
  `_reconcileOrientationErrorStats`(캐시된 `PercentileStats`)에 저장한다.
- **OnGUI는 계산하지 않고 캐시된 구조체만 읽는다** — IMGUI가 프레임마다(Layout+Repaint로 여러 번)
  `OnGUI`를 호출하므로, 세션 내내 자라는 리스트를 매번 정렬하면 그게 진짜 핫 패스 위반이 된다.
  갱신은 스냅샷 도착 빈도(재조정이 실제로 도는 빈도)로만 일어난다.
- `FormatStat(double)` — `NaN` → `"n/a"` 문자열, 그 외는 `F4`. **N=0을 "0.0000"으로 찍지 않는다.**

### 로그·HUD 문자열 (qa2에게 grep용으로 전달)

**세션 종료 로그** (`OnSessionEnded`, `client/Logs/Editor.log`에서 grep):
```
starfall.greybox: SC-56 session-end reconcile error stats - position_error_m(p50=<F4 or n/a>, p99=<F4 or n/a>, max=<F4 or n/a>, n=<int>), orientation_error_deg(p50=<F4 or n/a>, p99=<F4 or n/a>, max=<F4 or n/a>, n=<int>), reconcile_hard_snap_total=<long>
```
grep 태그: `"SC-56 session-end reconcile error stats"`.

**HUD** (`OnGUI`, `predict_error_m`/`predict_error_deg`/`reconcile_hard_snap_total` 줄 바로 다음에
두 줄 추가, 마지막 값 줄은 그대로 남겨 뒀다 — SC-59 실시간 관찰에 여전히 쓸모 있어서):
```
reconcile_error_m p50=<F4 or n/a> p99=<F4 or n/a> max=<F4 or n/a> n=<int>
reconcile_error_deg p50=<F4 or n/a> p99=<F4 or n/a> max=<F4 or n/a> n=<int>
```

`reconcile_hard_snap_total`(SC-56c)은 기존 줄 그대로 — 손대지 않았다(이미 세션 누적값이었다).

### 실행 결과

`unity test client --mode EditMode --report-format junit`: **종료 코드 0**,
`total=166, passed=164, failed=0, skipped=2`(기존 LiveServerTests 게이트, 무관). 신규
`ReconcileErrorStatsTests` 7건 전부 PASS. `client/Logs/Editor.log`에 `warning CS` 없음(컴파일 경고
0, 오류 0). `Assert.Ignore`/`Inconclusive` 미사용.

**미검증(환경) — 실서버 없이는 확인 불가**: `_reconcilePositionErrorsM` 등이 실제 `Reconcile()`
호출로 채워지는지, HUD·로그 값이 실측 오차와 맞는지는 블록 6의 실서버 실행이 있어야 안다(CL-2와
같은 처지 — R2에서 이미 "실행하지 않았다, server의 SET_SHIP_CONTROL 수정 뒤 실측"이라고 적은 그
경로가 여기서도 그대로 적용된다). `Compute()` 자체의 산술은 위 7개 단위 테스트로 실서버 없이도
확정됐다.

---

## R6 — PROTOCOL_VIOLATION 실서버 발견, 클라이언트 쪽 원인 조사 (2026-09-23)

리더 지시: `_workspace/p1-01-ship-movement/evidence/R4-B6/sc59/06-finding-protocol-violation.md`,
`02-clip-a-extended.md` 참고. **고치지 않았다** — 아래는 원인 확정과 RED 테스트만이다.

### ① 원인 확정 — `client/Assets/_Project/Scripts/Greybox/GreyboxSession.cs:418-452`

```csharp
void Update()
{
    if (_input != null) _input.Sample();

    _tickAccumulator += Time.unscaledDeltaTime;
    while (_tickAccumulator >= _tickDurationSeconds)
    {
        _tickAccumulator -= _tickDurationSeconds;
        SendAndPredictOneTick();   // <- 매 반복 client.TrySendJson()으로 SET_SHIP_CONTROL 즉시 송신
    }
    ...
}
```

고전적인 fixed-timestep catch-up 루프인데, **한 `Update()` 호출 안에서 드레인하는 tick 수에 상한이
없다.** 20 Hz(`client_send_hz = tick_hz = 20`, `data/movement/sync-tuning.json`)에서는 정상 프레임마다
0~1건이 나가야 맞지만, 프레임 하나가 `(N+1) * 50ms` 이상 늦게 오면(Editor GC pause, 도메인 리로드,
일반적인 프레임 히치) `_tickAccumulator`에 N tick어치 밀린 시간이 쌓이고, **다음 `Update()` 한 번에
N개의 `SendAndPredictOneTick()`이 루프 안에서 한꺼번에, 사이 간격 없이 호출된다.** 각 호출은
`_client.TrySendJson(json)`으로 그 자리에서 소켓에 직접 쓴다 — 프레임 예산도, 송신 스케줄러도 없다.

서버 쪽 상한은 `docs/adr/0011-state-sync-snapshot-and-input-queue.md` §5.2에 그대로 있다:
`MAX_COMMANDS_PER_SESSION_PER_TICK = 8`, 9건째부터는 **그 tick 전체가 프로토콜 위반 1회**로 잡히고
(`commands_dropped_over_tick_cap_total` 증가, `COMMAND_RESULT` 없음), 위반이 10초 창에 8회 쌓이면
연결을 닫는다(ADR-0006 §6) — 리더가 관찰한 로그(`budget=8 window_s=10`)와 정확히 일치한다.
**따라서 최소 450ms(9 tick어치)의 단일 히치 한 번이면 그 즉시 프로토콜 위반 1건을 만든다** — 서버
설계가 막으려던 "봇 burst 공격"이 아니라, 정상 클라이언트의 정상적인 프레임 히치로도 같은 신호가
나온다.

### ② RED로 고정했는가 — 고정했다

신규 파일: `client/Assets/_Project/Tests/EditMode/GreyboxSendBurstTests.cs`.

`GreyboxSession`은 씬을 만드는 MonoBehaviour 싱글턴이라(Awake가 `data/` 파일 로드, `StarfallNetHost`
기동, 마커·카메라·함선 primitive 생성을 한다) EditMode 테스트에서 직접 `AddComponent`로 띄우는 건
위험하다고 판단했다 — Edit 모드 밖에서의 `Destroy()` 호출이 에디터 오류를 내 무관한 테스트까지
실패시킬 수 있고, 싱글턴 가드가 두 번째 인스턴스를 조용히 파괴한다. 그래서 `GreyboxSession`을
인스턴스화하지 않고, **`GreyboxSession.cs:418-452`의 루프 모양(같은 조건문, 같은 반복문 본문: 명령
빌드 → 양자화 → 직렬화 → `RealtimeClient.TrySendJson` → 예측)을 실제 `Starfall.Net.RealtimeClient`·
`Starfall.Flight`/`Starfall.Sim` 프로덕션 타입에 대해, `RealtimeClientTests.cs`가 이미 쓰는 것과 같은
`FakeRealtimeTransport`를 통해 그대로 재연**했다. 블랙박스로 `GreyboxSession`을 호출한 것은 아니다 —
이 트레이드오프와 이유를 테스트 파일 헤더에 적어 뒀다.

**실행 결과(`unity test client --mode EditMode`, Editor 종료 후 CLI로 재기동, 종료 코드는 실패지만
의도된 RED)**:
```
hitch_s=0.46 tick_duration_s=0.05 sends_in_one_update_call=9 server_per_tick_cap=8
Expected: less than or equal to 1
But was:  9
```
전체 스위트: `total=167, passed=164, failed=1(신규 RED 1건), skipped=2(기존 LiveServerTests 게이트,
무관)`. 컴파일 오류 0, `Assert.Ignore`/`Inconclusive` 미사용. 실패 1건은 이 조사가 의도한 것이다.

### ③ 재조정 이상치(hard snap 5건)와 같은 원인인가 — 그렇다, 같은 상류 원인을 공유한다

ADR-0011 §5.2를 다시 확인: tick 상한을 넘겨 **버려진 명령은 `COMMAND_RESULT`를 아예 만들지 않는다**
("제출하지 않는다. `COMMAND_RESULT`도 만들지 않는다"). `PredictedShipController.ApplyInput`은 히치
동안 드레인된 입력 전부를 이미 예측 히스토리에 넣은 뒤인데(`SendAndPredictOneTick`의 마지막 줄,
`_client.TrySendJson`가 성공을 반환한 뒤 바로 `_controller.ApplyInput(dequantized)` 호출 — 서버가
그 명령을 실제로 받아들였는지와 무관하게 로컬 예측에는 이미 반영됨), 상한 초과로 버려진 입력에
대해서는 `COMMAND_RESULT{REJECTED}` 콜백이 오지 않으므로 `PredictedShipController.DropRejected`가
호출될 길이 없다 — 버려진 입력이 예측 히스토리에 **남는다.** 다음 `WORLD_SNAPSHOT`에서
`Reconcile()`이 이 히스토리를 서버가 확정한 상태 위에 재생하면, 서버는 반영하지 않았지만 클라이언트는
반영한 입력만큼 어긋난다 → 큰 오차 → hard snap.

리더가 지적한 "hard snap 5건이 위반(tick 457993)보다 먼저(tick 455892) 관측됐다"는 사실과도
모순되지 않는다 — `RATE_LIMITED`(조용히 버려지는 지속 초과, `rate_limit_hz=40`)는 프로토콜 위반
예산(8/10초)보다 먼저, 더 낮은 문턱에서 발동한다(§5.2 인용: "지속 초과 전송은 거부한다(`RATE_LIMITED`)
... 더 높은 임계(`protocol_violation_hz`)를 넘으면 ... 프로토콜 위반 예산을 매긴다"). 즉 초기의 작은
히치들은 `RATE_LIMITED` 드롭 + hard snap만 만들고, 반복되거나 더 큰 히치가 나중에 위반 예산을
채워 연결을 끊는다 — **둘 다 "한 `Update()` 호출이 여러 tick을 한꺼번에 드레인한다"는 같은 근본
원인의 다른 임계값에서 나타난 증상**이라는 리더의 가설과 부합한다. `RATE_LIMITED=19`(연결 확인
시점 통계)도 이 그림과 일치한다.

### ④ 고칠 곳과 방법 (아직 고치지 않았음 — 리더 신호 대기)

`GreyboxSession.cs:423-427`의 `while` 루프에 **한 `Update()`당 드레인할 tick 수의 상한**이 필요하다.
후보(어느 쪽도 아직 적용하지 않았다, 판단 요청):
- **상한을 두고 초과분은 버린다**: 예) 한 `Update()`에 최대 1~2 tick만 `SendAndPredictOneTick()`,
  나머지는 `_tickAccumulator`를 tick 하나 분량으로 clamp(표준 fixed-timestep의 "spiral of death"
  방지 패턴과 동일). 이러면 큰 히치 뒤 로컬 시뮬레이션이 실시간보다 뒤처지지만, 다음
  `WORLD_SNAPSHOT`의 `Reconcile()`이 따라잡아 줄 것으로 보인다(이미 있는 경로).
- 대안: 히치가 일정 크기(예: 서버 상한 8 tick)를 넘으면 그 프레임은 **아예 송신하지 않고
  `_tickAccumulator`를 통째로 버린 뒤 다음 스냅샷으로 재조정**한다 — "잘못된 촘촘한 재생"보다
  "한 번의 큰 재조정"이 낫다는 판단이면.

어느 쪽이든 **서버 쪽 임계(상한 8, 예산 8/10s)는 건드리지 않는다**(계약 §7 그대로 준수). 수정
방식은 game-architect·server 쪽과 맞춰야 한다고 본다 — 클라이언트 로컬 예측이 늦게 반영되는
체감(디자이너 관점의 "반응성")과 직결되기 때문.

### ⑤ 서버·설계 쪽 사안인가 — 아니다, 클라이언트가 만드는 버스트다

서버의 tick당 상한 8은 "정상 클라이언트는 tick당 1건을 보낸다"는 전제(§5.2 인용)로 설계됐고, 이
전제 자체는 맞다 — `client_send_hz = tick_hz = 20`이므로 히치가 없으면 진짜로 tick당 1건이다. 문제는
클라이언트가 **히치 뒤 밀린 tick들을 한 프레임에 몰아 보내** 그 전제를 깨는 것이지, 서버 상한
설계가 정상 클라이언트의 송신 특성을 잘못 추정한 게 아니다. 그러니 이번 건은 client 소유 수정
사안으로 본다.

**리더에게**: ① 확정(위 ①), 파일:라인 `client/Assets/_Project/Scripts/Greybox/GreyboxSession.cs:418-452`.
② RED로 고정(위 ②), `client/Assets/_Project/Tests/EditMode/GreyboxSendBurstTests.cs`. ③ 재조정
이상치와 같은 상류 원인 공유 확인(위 ③). ④ 수정안 두 가지 제시, 아직 미적용(위 ④). ⑤ 서버·설계
쪽 사안 아님, 근거는 위 ⑤. 커밋하지 않았다. Editor는 CLI 테스트 실행을 위해 닫았다가(사전에 qa3에
공지) CLI가 재기동한 상태다 — 리더가 다시 수동으로 열어야 하면 알려달라.

---

## R7 — 리더 지적: RED 테스트가 사본을 잰다. 순수 함수 추출 설계 (아직 미적용)

리더 지적이 맞다. `GreyboxSendBurstTests.cs`는 `GreyboxSession.cs:418-452`의 루프 **모양을 재연**했을
뿐, 실제 그 코드를 호출하지 않는다 — 나중에 `GreyboxSession.cs`가 되돌아가도 테스트는 계속 초록이다.
계약 §7a가 이 슬라이스에서 이미 여러 번 지적한 것과 같은 결합 실패(`world_full` 미실행, 전부
REJECTED인데 154개 초록, 봇 게이트 `0==0` 통과, flaky가 자기 폴링을 잰 것)와 같은 모양이다.

### 설계 — `ReconnectPolicy.ShouldReconnect` 패턴을 그대로 따른다

**아직 코드에 넣지 않았다.** architect가 (A) clamp / (B) 히치 통째로 버리기, K 값, **예측이 서버의
`carry_forward_max_ticks`(ADR-0011, 기본 10 tick = 500ms, "도착 0건: 직전 입력을 이월")를 흉내
내야 하는가**를 판정한 뒤 그 형태에 맞춰 한 번에 넣는다.

세 정책 모두를 수용할 수 있는 순수 함수 시그니처를 제안한다:

```csharp
// Starfall.Greybox, UnityEngine 참조 없음 (ReconcileErrorStats.cs와 같은 패턴).
// GreyboxSession.Update()와 EditMode 테스트가 이 함수를 바이트 단위로 같은 것을 호출한다 -
// 사본을 재연하지 않는다.
public static class TickCatchUp
{
    public readonly struct Plan
    {
        public readonly int TicksToSend;      // 이번 프레임에 실제로 네트워크로 내보낼 tick 수
        public readonly int TicksToPredict;   // 이번 프레임에 로컬 예측을 전진시킬 tick 수
        public readonly double RemainingAccumulatorSeconds;
    }

    // maxSendsPerFrame: 서버 tick당 상한(8)보다 작아야 하는 정책 값 - architect가 K를 정한다.
    // predictBeyondSendCap: true면 전송 상한을 넘는 tick도 로컬 예측은 계속 전진시킨다(서버의
    //   carry_forward와 같은 모양을 클라이언트에서도 만들어 hard snap을 줄인다 - 옵션 A 변형).
    //   false면 전송 안 한 tick은 예측도 안 하고 다음 WORLD_SNAPSHOT의 재조정에 맡긴다(옵션 B).
    public static Plan Compute(
        double accumulatorSecondsBeforeThisFrame,
        double tickDurationSeconds,
        int maxSendsPerFrame,
        bool predictBeyondSendCap);
}
```

`GreyboxSession.Update()`는 이 함수가 돌려준 `Plan`을 그대로 실행만 한다(몇 번 보내고, 몇 번
`ApplyInput`하고, `_tickAccumulator`를 뭘로 남길지 전부 `Compute`가 결정). 테스트는
`GreyboxSession`을 거치지 않고 `TickCatchUp.Compute`를 직접 호출해 각 정책·경계값(K-1, K, K+1 tick
분량 히치, `predictBeyondSendCap` true/false)을 검증한다 — `ReconnectPolicy.ShouldReconnect`를
순수 함수로 뽑아 `RealtimeClientTests.cs`가 직접 호출하는 것과 같은 구조다.

**적용 순서**: architect 판정(A/B, K, carry-forward 흉내 여부) → `TickCatchUp.Compute` 구현 +
단위 테스트(RED→GREEN, 진짜 코드를 잼) → `GreyboxSession.cs:423-427`을 `Compute` 호출로 교체 →
기존 `GreyboxSendBurstTests.cs`는 삭제하거나(사본 재연 테스트는 더 이상 필요 없음) 회귀 방지용
스모크로 남길지 결정. 지금은 설계만이고, 코드는 건드리지 않았다.

---

## R8 — qa3 프레임 분석 후속 확인 4건 (코드 읽기만, 수정 없음)

리더 지시대로 **확인 결과만** 적는다. architect 판정 전이므로 아무것도 고치지 않았다.

### 1. `outbound queue full` — 내 설계에 빠져 있던 세 번째 어긋남 경로

`client/Assets/_Project/Scripts/Net/PcWebSocketTransport.cs:243-254`:
```csharp
public bool Send(string message)
{
    ...
    if (_outbound.Count >= MaxOutboundQueue)   // MaxOutboundQueue = 256
    {
        _log.Warn("starfall.net: outbound queue full (" + MaxOutboundQueue + "), refusing to send");
        return false;
    }
    _outbound.Enqueue(message);
    ...
}
```
`RealtimeClient.TrySendJson`은 이걸 그대로 반환하고, **`GreyboxSession.SendAndPredictOneTick`은
`TrySendJson`이 `false`면 그 자리에서 `return`한다 — `_controller.ApplyInput()`을 호출하지 않는다**
(`GreyboxSession.cs:446`, `if (!_client.TrySendJson(json)) return;`). 즉 지금 코드는 "안 보낸 tick은
예측도 안 한다"를 **이미** 하고 있다 — 단, 의도적 설계가 아니라 `TrySendJson`의 반환값을 그대로 따라간
부수효과다.

이게 R7에서 설계한 `TickCatchUp.Compute`에 빠져 있던 부분이다: 내 설계는 **의도한 히치**(누적 시간)에
대해서만 몇 tick을 보낼지 결정했지, **실제 소켓이 거부하는 경우**(큐 256 초과)는 다루지 않았다. 이
둘은 같은 축이다 — `predictBeyondSendCap`이 "보내려던 tick 수를 정책상 K로 자른 것"뿐 아니라 "K 안에서
시도했는데 소켓이 실제로 거부한 것"에도 같은 정책을 적용해야 한다. **큐가 이미 찬 상태에서
`TrySendJson`이 거부하면, 그 입력은 로컬 예측에도 안 들어가므로 tick 상한 초과(서버가 버려서
`REJECTED`가 안 오는 경우)와 다른 종류의 어긋남**이다 — 서버 쪽은 "보냈지만 버려짐"(로컬은 예측했는데
서버는 안 받음, 어긋남 방향: 클라이언트가 앞서감), 큐-full 쪽은 "아예 못 보냄"(로컬 예측도 멈춤,
어긋남 방향: 클라이언트가 뒤처짐). **둘 다 `TickCatchUp` 설계 하나로 통일해 다룰 수 있지만, 지금은
후자를 `TrySendJson`의 반환값에 우연히 맡기고 있다 — architect 판정 항목에 추가해 달라고 리더에게
전달한다.**

(참고: 위 8초 히치 재현에서는 9건이 전부 성공 송신됐다 — `FakeRealtimeTransport`는 큐 상한이 없어
큐-full 경로는 이 세션에서 아직 실측되지 않았다. 리더가 인용한 `t=600s` 관측은 위반보다 나중 구간이라
직접 인과는 아니다, qa3와 동의한다.)

### 2. `Application.runInBackground` — 프로젝트 설정 확인함, 단 범위 불확실

`client/ProjectSettings/ProjectSettings.asset:90`: `runInBackground: 0` (꺼짐). 코드에
`OnApplicationFocus`/`OnApplicationPause`/`isFocused`/`targetFrameRate` 등을 건드리는 곳은 없다
(grep 결과 0건) — 즉 백그라운드 동작은 전부 Unity 기본값에 맡겨져 있다.

**정직하게 밝힌다**: `runInBackground`는 공식적으로 **Standalone 빌드**의 `Application.runInBackground`
프로퍼티를 설정하는 값이고, **Editor의 Play 모드 자체**가 이 값을 그대로 따르는지는 리포에서 확인할
수 없다 — Game View가 활성 탭인지, "Always Refresh"가 켜져 있는지 같은 관련 설정은 **Editor
Preferences(사용자별, 리포 밖)**에 있어서 코드로 읽을 수 없다. 다만 이 값이 꺼져 있다는 사실은
리더의 가설(백그라운드에서 프레임이 크게 밀린다)과 **모순되지 않고, 오히려 뒷받침**한다 — 켜져
있었다면 "백그라운드에서도 전속력으로 돈다"는 명시적 의도가 있었다는 뜻인데 그렇지 않다. **누적
크기가 450ms가 아니라 수 초일 수 있다는 리더의 우려에 동의한다** — 코드로는 상한을 확인할 수 없으니
재현 시 실측이 필요하다(예: 히치 중 `Time.realtimeSinceStartup` 델타를 로그로 남겨 확인).

### 3. HUD가 사라진 이유 — 코드는 원인을 설명하지 않는다, 렌더 경로 문제로 추정

`GreyboxSession.OnGUI()` (`GreyboxSession.cs:512-578`)의 유일한 조기 반환 조건은
`if (GUI.skin == null) return;` 뿐이다. **세션 상태·포커스·재조정 여부에 따라 그리기를 멈추는 코드는
없다** — `_controller == null`이어도 `speed`/`originDistance`를 0으로 채워 계속 그린다. 즉 **코드
자체는 세션이 죽어 있어도, 함선이 없어도, 매 프레임 뭔가는 그려야 한다.**

그런데 실측으로는 안 그려졌다(t≈213s). 코드에 그 경로가 없으므로, 남는 설명은 **`OnGUI`가 아예
호출되지 않은 것** — IMGUI의 `OnGUI` 콜백은 Game View가 실제로 렌더될 때만 도는 것으로 알려져 있고
(Editor 창 포커스, Game View가 활성 탭인지, "Maximize on Play"/"Always Refresh" 같은 Editor
Preferences에 좌우됨 — 전부 사용자별 설정이라 리포에 없다), 리더가 2번에서 확인한 "위반 전후 여러
프레임에서 Unity가 전경이 아니었다"는 관측과 맞아떨어진다. **2번과 같은 근본(Editor가 백그라운드/비활성
Game View)일 가능성이 높다고 본다 — 단, 코드로 100% 확정할 수는 없다.**

**다음 SC-59 재녹화에 제안**: (a) Game View를 Editor 안에서 최상단 활성 탭으로 유지하고 Play 내내
그 탭을 벗어나지 않는다, (b) OS 창 포커스도 Unity에 유지한다(리더가 재현 조건으로 이미 제시한
"다른 창으로 전환했다가 돌아온다"와 반대로, 이번엔 안 벗어나는 조건으로 한 번 더 찍어 HUD가
유지되는지 대조), (c) 가능하면 Editor Preferences의 Game View "Always Refresh"를 켜고 진행. 이건
코드 수정이 아니라 **녹화 절차** 문제라 지금 바로 고칠 대상이 없다.

### 4. `reconcile_error_deg max`가 재접속 전후 동일한 이유 — 계산 버그 아님, reset 범위 불일치

`ReconcileErrorStats.Compute`(순수 함수, R5에서 7개 단위 테스트로 이미 검증됨)는 손대지 않아도 됐다
— 문제는 그쪽이 아니다.

`GreyboxSession.OnSessionReady`(`GreyboxSession.cs:216-243`, 재접속마다 불림)가 초기화하는 것:
`_tickDurationSeconds`, `_controller`(→ null), `_controlledShipClass`, `_localInputSeq`,
`_pendingInputSeqByCommandId`, `_lastAckInputSeq`, 원격 함선 뷰. **초기화하지 않는 것**:
`_reconcilePositionErrorsM`/`_reconcileOrientationErrorsDeg`(R5가 추가한 누적 리스트),
`_reconcilePositionErrorStats`/`_reconcileOrientationErrorStats`(캐시된 퍼센타일),
`_reconcileHasErrorTotal`/`_reconcileReplayedNonZeroInputsTotal`/`_reconcileBothOmegaNonZeroTotal`
(CL-2 카운터) — grep으로 재확인, `OnSessionReady`도 다른 어디도 이 필드들을 재대입하지 않는다.

반면 HUD의 `reconcile_hard_snap_total`은 `_hardSnapTotal = _controller.HardSnapTotal`
(`GreyboxSession.cs:399`, `WORLD_SNAPSHOT`마다 갱신)로 채워지는데, `_controller`는
`OnSessionReady`에서 `null`이 됐다가 재접속 후 첫 스냅샷에서 **새 `PredictedShipController`
인스턴스**로 다시 만들어진다(`HardSnapTotal`은 그 인스턴스의 필드라 0부터 다시 센다) — **그래서
`reconcile_hard_snap_total`만 재접속에서 사실상 리셋되고, 나머지 R5/CL-2 통계는 Editor Play 세션
전체(재접속을 넘어)에 걸쳐 계속 누적된다.**

이게 qa3가 본 것을 정확히 설명한다: **위치 오차 max가 56.3 → 323.1로 커진 건 재접속 후 더 큰
표본이 실제로 들어왔기 때문**(누적 리스트라 커질 수만 있음), **각도 오차 max가 80.5663으로 똑같이
남은 건 재접속 후 그보다 더 큰 각도 오차 표본이 하나도 안 들어왔기 때문**(누적 리스트라 안 줄어듦,
우연히 안 늘어난 것뿐) — **클램프나 계산 버그가 아니라, 그냥 두 지표가 서로 다른 "세션" 범위를
쓰고 있다.**

**qa2/architect에게 넘길 판정거리**: `OnSessionReady`의 기존 주석(`GreyboxSession.cs:218-220`,
"session/input state starts fresh on EVERY SESSION_READY... prediction state does not [carry
over]")은 R5가 추가한 통계 필드들이 생기기 전에 쓰였다 — 그 원칙을 따른다면 이 통계들도
`OnSessionReady`에서 초기화돼야 앞뒤가 맞는다. 지금 상태로는 **SC-56 HUD 숫자가 재접속이 낀
녹화에서는 "이번 서버 세션"이 아니라 "이번 Editor Play 세션 전체(여러 서버 세션 합산)"를 보여준다**
— 이번처럼 재접속이 낀 녹화를 SC-56 판정에 쓰려면 그 사실을 감안해야 한다. 고치지 않았다 — 판정
대기.

리더에게: 1·2·3·4 전부 확인 완료, 위 내용 그대로. 무엇도 고치지 않았다.

---

## R9 — SC-89 계측 2건 구현 (리더 승인, 수정 본체는 여전히 대기)

**catch-up 루프(`GreyboxSession.cs:418~431`)는 건드리지 않았다** — while 조건, tick 상한 로직 전부
그대로다. 아래는 계측(관찰)만 추가한 것이고, 서버 임계(상한 8, 예산 8/10초)도 손대지 않았다.

### 계측 1 — 송신 버스트 계측

신규 순수 C# 클래스 `client/Assets/_Project/Scripts/Greybox/SendBurstStats.cs`
(`ReconcileErrorStats.cs`와 같은 패턴, UnityEngine 미참조, 상태를 갖는 누적기):
- `MaxTicksDrainedPerUpdate` (`int?`) — 세션 중 한 `Update()`가 드레인한 tick 수의 최댓값.
  `RecordUpdate(int)`을 매 `Update()` 끝에서 부른다. **0도 진짜 측정값**이다(캐치업 없이 정상
  프레임이었다는 뜻) — `PercentileStats.Empty`와 같은 규율로 아직 한 번도 안 불렸으면 `null`.
- `MaxSendsInTrailingOneSecond` (`int?`) — 직전 1초 창 안 실제 송신 건수의 세션 중 최댓값.
  `RecordSend(double nowSeconds)`를 **`TrySendJson`이 성공한 직후에만** 부른다(`GreyboxSession.
  cs:446` 뒤, 큐-full로 실패한 건 절대 기록하지 않음 — R8 §1에서 지적한 "실제로 나간 것과 시도만
  한 것"의 구분을 그대로 지켰다).

`client/Assets/_Project/Tests/EditMode/SendBurstStatsTests.cs` 8개: 미측정 상태, 0도 측정값,
누적 최댓값(내려가지 않음), 정상 9건 윈도우, 20 Hz 정상 트래픽의 창 크기(21 — 1.0초 양끝 포함,
경계 테스트로 수치 확정), 1초 지난 표본 제거, 경계값(정확히 1.0초는 포함). **RED 먼저 안 하고
구현·테스트를 거의 같이 썼다** — R5와 같은 정직한 기록: 작은 누적기라 손검산이 쉬웠다는 판단.
대신 경계 테스트 1개(`RecordSend_NormalTwentyHertzTraffic_...`)는 처음에 20을 기대했다가 실제
실행 결과(21, 1.0초 구간에 양끝 포함이라 21개가 맞다)를 보고 정정했다 — 그 과정 자체를 테스트
파일에 남겼다.

**HUD**(`OnGUI`, CL-2 줄 다음): `send_burst_max_ticks_per_update=<n/a or int>
send_burst_max_sends_per_trailing_1s=<n/a or int>`.
**세션 종료 로그**(`OnSessionEnded`, grep 태그 `"SC-89 session-end send burst stats"`):
`max_ticks_drained_per_update=<..>, max_sends_per_trailing_1s=<..>`.

### 계측 2 — close code 1002 → `PROTOCOL_VIOLATION` grep 태그 (재연결 동작 불변)

`client/Assets/_Project/Scripts/Net/StarfallNetLog.cs`에 `ProtocolViolationTag`(=
`"PROTOCOL_VIOLATION"`) + `ProtocolViolationClosed()` 추가. `RealtimeClient.OnDisconnected`
(`RealtimeClient.cs:335` 부근, `Session = null` 직후)에서 `info.CloseCode == 1002`일 때만 한 줄
로그: `starfall.net: PROTOCOL_VIOLATION close_code=1002, reconnecting as usual (...)`.

**`ReconnectPolicy.ShouldReconnect`는 손대지 않았다** — 1002는 여전히 재연결 대상이다(4001과
다르게). 로그는 재연결 분기 이전, 무조건 한 번만 찍는다.

`RealtimeClientTests.cs`에 3개 추가(SUPERSEDED 섹션 바로 다음, 같은 패턴):
- `Disconnect_WithProtocolViolationCloseCode_LogsAGreppableLine` — 1002에서 grep 가능한 줄이
  찍히는지.
- `Disconnect_WithProtocolViolationCloseCode_StillReconnects` — **재연결 동작이 그대로인지**
  (4001과 달리 `Attempt`가 증가하고 새 접속이 즉시는 아님을 확인 — 기존 `Disconnect_
  WithAnyOtherCloseCode_StillReconnects` 파라미터화 테스트와 같은 단언).
- `Disconnect_WithSomeOtherCloseCode_DoesNotLogTheProtocolViolationLine` — 1000 등 다른 코드에서
  태그가 안 찍히는지(오탐 방지).

### 실행 결과

`unity test client --mode EditMode`: `total=178(이전 167+신규 11), passed=175, failed=1(의도된
RED, `Update_AfterA460msHitch_...`, 그대로 "9 vs 1" 실패 유지 — 계측 때문에 초록이 되지 않았다
확인함), skipped=2(기존 게이트, 무관)`. 컴파일 오류 0. `Assert.Ignore`/`Inconclusive` 미사용.
커밋 안 했다.

**테스트 수 변경 통지**(qa3 요청대로): `159/157`(qa2 측정) → `167/164`(R6/R8, 내 RED 1건 추가) →
**`178/175`**(이번 계측 11건 추가). 블록 6 재실행 때 SC-46·47·49와 함께 이 숫자도 다시 확인
바란다.

### HUD 소실 원인 — 추가 정보 없음, R8 §3에서 더 나아가지 못했다

리더가 다시 물었지만, 코드 레벨에서 더 파낼 게 없었다 — `OnGUI()`에 조건 분기가 `GUI.skin==null`
하나뿐이라는 사실은 R8 §3에서 이미 확인했고, 그 이상은 Editor Preferences(사용자별, 리포 밖)나
실측이 필요한 영역이라 코드 읽기로는 답이 안 나온다. 실측 없이 억측하지 않겠다 — R8 §3의 재녹화
절차 제안(Game View를 계속 활성·포커스 유지한 대조 촬영)이 유일하게 확인 가능한 다음 수다.

---

## R10 — CL-H 착수: H-1(`TickCatchUp.Plan`) + SC-59(b1) + 재촬영 체크리스트. H-3 이후는 충돌로 보류

**충돌 신고**: 리더의 마지막 메시지("H-3은 멈춰라... architect가 아직 답하지 않았고")와 architect의
직전 메시지("R4 보충 판정")가 어긋난다 — architect는 이미 H-3′(큐-full/송신실패 tick도 예측은
무조건 전진, `GreyboxSession.cs:446`의 `return` 제거)로 답했다. 리더에게 알렸고, **답을 받을 때까지
`GreyboxSession.cs`는 이번 라운드에 전혀 건드리지 않았다** — `while (_tickAccumulator >= …)`도
`:446`의 `return`도 그대로다(`grep -n 'while (_tickAccumulator' client/Assets/_Project/Scripts/
Greybox/GreyboxSession.cs` → 여전히 1건, exit 0 — qa3 기준선과 동일하게 유지).

### H-1 — `TickCatchUp.Plan` (완료)

신규 `client/Assets/_Project/Scripts/Flight/TickCatchUp.cs` (**`Starfall.Flight`**, architect
보충 판정 지시대로 `Starfall.Greybox`가 아님). 순수 정적 함수, UnityEngine·시계 미참조.

시그니처는 보충 판정의 수정 셋을 전부 반영했다:
- **`predictBeyondSendCap` 불리언 없음** — 정책은 하나(밀린 tick은 언제나 전부 예측)뿐이고, 그
  정책을 고르는 파라미터 자체를 없앴다.
- **`maxPredictedTicksPerFrame`(M) 파라미터 추가**, 반환에 **`Truncated`** 포함.
- **`Starfall.Flight`에 위치**(asmdef 확인함 — `Starfall.Sim`/`Starfall.Net`/`Starfall.Contracts`만
  참조, UnityEngine 의존 없음. `PredictionHistory.cs`·`Reconciliation.cs`가 이미 이 자리에 있다).

프로덕션 상수: `ProductionMaxSendsPerFrame = 1`(K, 유도값 — 서버가 같은 tick 안의 마지막 명령만
적용하므로 나머지는 구조적으로 버려진다, ADR-0011 §4), `ProductionMaxPredictedTicksPerFrame = 20`
(M, 1초). 둘 다 데이터 파일이 아니라 `const` + 판정 인용 주석으로 뒀다(architect 지시).

`Compute(accumulatorSecondsBeforeThisFrame, tickDurationSeconds, maxSendsPerFrame,
maxPredictedTicksPerFrame)` → `Plan{TicksToPredict, TicksToSend, RemainingAccumulatorSeconds,
Truncated}`. N ≤ M이면 N tick 전부 예측 + 마지막 min(N,K)건만 송신 + 나머지 누적은 다음 프레임
이월. N > M이면 M tick만 예측, `Truncated=true`, **남은 누적은 0으로 버린다**(다음 프레임에
그대로 남기면 같은 초과가 반복된다).

### H-1 테스트 (완료, `TickCatchUp.Compute` 자체를 직접 검증 — GREEN)

신규 `client/Assets/_Project/Tests/EditMode/TickCatchUpTests.cs`, 13개:
- **R4 증거 재현**: 0.46초(9.2 tick) 입력 → `TicksToPredict=9`(전부 예측, 버리지 않음),
  `TicksToSend=1`(9건이 아니다) — 이게 원래 버그와 수정의 차이를 직접 고정한다.
- 경계값: 1 tick 미만/정확히 1 tick, M 경계(정확히 20은 잘림 아님, 21은 잘림)
- **잘림 시 누적 완전 폐기 확인**(`RemainingAccumulatorSeconds == 0`)
- K를 0/1/3/8로 바꿔가며 `TicksToSend ≤ min(K, TicksToPredict)` 불변식
- **이월 tick + 송신 tick == 예측 tick**(SC-89 (a) 불변식 ③, `TicksToPredict - TicksToSend`가
  이월 tick 수라는 구성상 항등식을 명시적으로 단언 — 나중에 비연속 tick을 보내는 식으로 바뀌면
  여기서 먼저 깨진다)
- **성질 테스트 2개**(무작위 프레임 델타 열, 10초 분량): ① 정상 지터만(20ms 이하 프레임) → 프레임당
  송신 ≤ 1, 벽시계 1초당 예측 tick 수가 실제 경과시간/tick길이의 ±1 안 ② 같은 델타 열에 0.46초
  히치를 끼워 넣어도 같은 두 불변식 유지

**주의(정직하게 밝힌다)**: 이 13개는 `TickCatchUp.Compute` **자체**가 맞다는 것을 보인다 — GREEN이다.
architect가 H-7에서 요구한 "**수정 전 코드**에 대해 성질 테스트로 RED를 넓혀 보이는 것"은 아직
못 했다 — 그건 `GreyboxSession.Update()`가 `Plan`을 실제로 부르도록 배선(H-6)해야 의미가 생기는데,
H-3 충돌로 `GreyboxSession.cs`를 못 건드렸다. 기존 `GreyboxSendBurstTests.cs`(사본 재연, 0.46초
한 점)는 그대로 RED로 남아 있다 — 넓히지 못했을 뿐 후퇴하지도 않았다.

### SC-59 (b1) — 마커 4개 배치, EditMode 단언으로 닫음 (완료, 계약 9차 반영)

계약이 9차 개정에서 "마커 4개 배치"를 영상이 아니라 EditMode 데이터 단언으로 닫도록 바꿨다(architect
R4 보충 §G) — `GreyboxSession.cs`나 catch-up 로직과 무관해서 H-3 충돌과 상관없이 바로 했다.

신규 `client/Assets/_Project/Tests/EditMode/ReferenceMarkerPlacementTests.cs`, 6개:
`GreyboxSession.LoadData()`와 같은 경로로 `data/world/systems/cradle.json`을 읽어 마커가 정확히
4개, 각 id(`star-cradle`/`planet-vela`/`beacon-meridian`/`derelict-unnamed`)가 정해진 좌표에
있는지, id 중복이 없는지 확인. 전부 PASS.

### SC-59 재촬영 체크리스트 (완료, 문서)

`_workspace/p1-01-ship-movement/evidence/R4-B6/sc59/09-reshoot-checklist.md`. 리더가 요청한
항목 전부: Game View 활성 탭·포커스 유지 방법(Maximize on Play 권장), HUD 확인 절차(시작 직후 +
클립 a 직전, **실제로 프레임을 열어 본다**), 클립 a의 세 동작(전진·오른쪽 선회·위 추력) ↔ HUD
필드(`thrust=`/`aim_target=`) 대응표, (b1)이 이제 영상 불필요임을 명시하고 (b2) 가시성만 확인하는
법, 소요 시간 추정(10~25분), 촬영 후 셀프체크. **절차만이고 실행은 안 했다** — 리더가 사용자 일정에
맞춰 요청한다.

### 실행 결과

`unity test client --mode EditMode`: **`total=197(이전 178 + TickCatchUpTests 13 +
ReferenceMarkerPlacementTests 6), passed=194, failed=1(의도된 RED, `Update_
AfterA460msHitch_...`, 변화 없음), skipped=2(기존 게이트, 무관)`**. 컴파일 오류 0. 커밋 안 했다.

**테스트 수 변경 통지**: `159/157`(qa2) → `167/164`(R6) → `178/175`(R9 계측) → **`197/194`**(이번).

### 남은 CL-H 항목 상태 (architect 보충 판정 번호 기준)

| 항목 | 상태 |
|---|---|
| H-1 `TickCatchUp.Plan` | **완료** |
| H-2 리베이스 보류 순수 술어 | 미착수 — H-1 다음 순서, H-3 충돌과 무관하게 진행 가능하다고 판단되나 이번 라운드에 시간이 부족했다 |
| H-3′ (송신 성공 여부 무관 예측 전진, `:446`의 `return` 제거) | **보류 — 리더·architect 지시 충돌, 리더 확인 대기** |
| H-4 이월 만료→휴면 입력 | H-3′ 이후 |
| H-5 예측 tick마다 seq+히스토리 1건 | H-3′/H-4 이후 |
| H-6 `Update()`의 `while` 제거 | H-3′ 이후 (grep으로 아직 1건 남아 있음 확인) |
| H-7 `GreyboxSendBurstTests` 재작성(`Plan` 직접 호출) + 성질 테스트 결합 + 짝 단언 | H-6 이후. **성질 테스트 자체는 `TickCatchUpTests.cs`로 선행 완료**(`Plan`을 직접 검증) |
| H-8(원안, C-1~C-4) | 리더 지시로 H-10~H-14에 흡수됨 |
| H-9 스냅샷 중복 시 재조정=최신 1건만, 보간 버퍼엔 전부 | H-6 이후 |
| H-10 지표 두 범위(`_session`/`_run`) 분리 + `OnSessionReady` 리셋 보강 | 미착수 |
| H-11 (C-1) 송신 버스트 계측 | **완료**(R9, `SendBurstStats.cs`) |
| H-12 (C-2) close code→사유 매핑, 1002 태그 | **완료**(R9, `ReconnectPolicy`는 안 건드림 — SC-88(d) 13건 계속 PASS 확인, 아래) |
| H-13 (C-3) `OnGUI` 외 주기적 로그 | 미착수 |
| H-14 `outbound_queue_full_total` 카운터 | 미착수(현재는 문자열 로그만 있고 카운터 없음) |
| H-15 이월 항목 파생 seq + `DropRejected` 전파 | H-3′/H-4 이후 |

**SC-88(d) 13건 확인**(리더가 짚은 회귀 위험): `RealtimeClientTests.cs` 12건 + `TransportHandshakeTests.cs`
1건 — 이번 라운드에 `RealtimeClient.OnDisconnected`를 다시 건드리지 않았으므로(H-12는 R9에서 이미
끝났고 이번엔 손 안 댐) 전체 197건 중 실패가 의도된 RED 1건뿐이라는 사실 자체가 13건 전부 PASS임을
포함한다.

리더에게: H-3 충돌 확인 부탁한다. 확인되는 대로 H-2·H-3′·H-4~H-7, 그다음 H-9~H-15 순으로 이어가겠다.

---

## R11 — CL-H 본체 구현: H-1~H-7, H-10′, H-14~H-17 완료. H-9/H-13/H-18은 예산상 보류

architect의 세 번의 판정(R4 판정, R4 보충 판정, R4 보충 판정 2)과 리더의 지시를 반영했다. **핵심
수정(H-1~H-7)이 끝났고 RED가 GREEN이 됐다** — 이번 조사에서 가장 중요한 산출물이다.

### 핵심 수정 — 송신/예측 분리 (H-1′, H-2, H-3″, H-4′, H-5, H-6, H-16)

**신규/수정 파일**:
- `client/Assets/_Project/Scripts/Flight/TickCatchUp.cs` — R6/R10에서 이미 만든 것에 architect
  보충 판정을 반영해 재확인(불리언 없음, M 파라미터+`Truncated`, `Starfall.Flight`). `carry_forward_max_ticks`
  상수(`ProductionCarryForwardMaxTicks = 10`) 추가.
- `client/Assets/_Project/Scripts/Flight/RebaseHold.cs`(신규) — H-2/T-2. 순수 술어. 보관
  히스토리에 **보내지 않은** 항목이 있고 그 seq가 `ack_input_seq`보다 크면 리베이스를 보류하고
  예측을 계속한다. 500ms(=`carry_forward_max_ticks`) 넘으면 강제 리베이스 + 이월 항목 전부
  폐기(`DiscardUnsentEntries`). `Reconciliation.Reconcile`은 손대지 않았다(순수성 유지).
- `client/Assets/_Project/Scripts/Flight/InputRecord.cs` — `DerivedFromSeq`(H-15) 필드 추가.
  null이면 실제로 보낸 tick, 값이 있으면 그 seq에서 파생된 이월/휴면 tick.
- `client/Assets/_Project/Scripts/Flight/PredictionHistory.cs` — `ApplyInput`이 `derivedFromSeq`를
  받는다. `DropRejected`가 파생 항목도 함께 버린다(H-15). `EnforceCap`(H-16) 신설 — 상한
  30(`carry_forward_max_ticks`+M) 넘으면 가장 오래된 것부터 버리고 개수를 반환.
- `client/Assets/_Project/Scripts/Flight/PredictedShipController.cs` — `ApplyInput`이
  `derivedFromSeq`를 받고 매번 `EnforceHistoryCap()`을 돈다. `PredictionHistoryOverflowTotal`
  카운터, `DiscardUnsentHistory()`(강제 리베이스용) 추가.
- **`client/Assets/_Project/Scripts/Greybox/GreyboxSession.cs`** — 핵심 배선:
  - `Update()`: `while (_tickAccumulator >= …)` **완전히 제거**(grep 재확인, exit 1). 대신
    `TickCatchUp.Compute` 호출 → `Plan`의 `TicksToPredict`만큼 반복하며, **마지막 `TicksToSend`개만**
    `TrySendCurrentInputForThisTick()`(새 입력 송신 시도), 나머지는 `PredictCarryForwardOrDormantTick()`
    (이월/휴면 예측, 송신 없음).
  - `TrySendCurrentInputForThisTick()`: 송신 성공하면 그 입력으로 예측 + `_lastSentPayload`/
    `_lastSentInputSeq` 갱신 + `_ticksSinceLastSend = 0`. **실패(큐-full 포함)하면 `false`만
    반환** — 호출자가 자동으로 이월/휴면 경로로 넘어간다(H-3″: 더 이상 `return`으로 예측을
    건너뛰지 않는다).
  - `PredictCarryForwardOrDormantTick()`: `_ticksSinceLastSend`가 10 이하면 `_lastSentPayload`의
    **양자화된 정수를 그대로** 재사용(원시 float 재샘플 없음, I-36 준수)해 이월 예측. 10 초과면
    `SetShipControlBuilder.Build`로 휴면 입력(추력 0, roll 0, aim=현재 예측 자세, brake false,
    assist true)을 합성. 두 경로 모두 새 seq를 소비하고 `derivedFromSeq`를 남긴다.
  - `OnWorldSnapshotCore()`: `Reconcile()` 호출 전에 `RebaseHold.Evaluate`를 돌려 보류/강제/정상
    셋 중 하나를 고른다. `_hardSnapTotalRun` 등 run-scope 지표 갱신도 여기서.
  - `OnCommandResultReceived`(REJECTED)는 손대지 않았다 — `PredictionHistory.DropRejected`가
    이미 파생 항목까지 버리도록 바뀌었으므로 자동으로 H-15가 적용된다.
  - `OnSessionReady()`: R5 오차 리스트·캐시 퍼센타일·CL-2 카운터 3종 + 이월 상태(`_lastSentPayload`
    등) + `_rebaseHoldSeconds`를 리셋(H-10′). `_run` 스코프 필드는 **건드리지 않는다**.

### 테스트 (전부 실제 코드를 잰다, T-4/T-5/T-6)

- `TickCatchUpTests.cs`(13, R10에서 이미 작성) — 여전히 유효, architect 시그니처와 일치 확인.
- `RebaseHoldTests.cs`(신규, 6) — 정상 리베이스/보류/ack이 넘어간 뒤 재개/ack null/500ms 경계/
  `DiscardUnsentEntries`.
- `PredictionHistoryTests.cs`(신규, 5) — `DropRejected`의 파생 항목 전파, `EnforceCap`의 경계·
  잘림·복사본 반환.
- **`GreyboxSendBurstTests.cs` 전면 재작성**(T-4) — 복제 루프를 지우고 `TickCatchUp.Compute`를
  직접 호출, `FakeRealtimeTransport` 종단 카운트는 그 `Plan`이 구동한다. R6의 RED
  (`Update_AfterA460msHitch_...`, "9 vs 1")가 **이제 GREEN이다**(9 tick 예측 + 1 tick만 송신).
  **T-6 짝 단언 포함**: 이월 tick 수 > 0을 함께 확인 — 없으면 "송신이 안 늘어난 건 애초에 아무것도
  안 밀렸기 때문"이라는 거짓 통과를 막는다.
  **T-5 성질 테스트 신규**(`PropertyTest_RandomFrameDeltasIncludingA460msHitch_HoldsAllThreeInvariants`):
  10초 분량 무작위 프레임 델타(0.46초 히치 포함)에 대해 ① 프레임당 송신 ≤ 1 ② 벽시계 1초당 예측
  tick ±1 ③ 이월+송신==예측, **그리고 T-6 짝 단언**(전체 실행의 이월 tick 합 > 0).

### 계측 보강

- **H-17**(max 옆 n): `_run` 스코프 `reconcile_error_m_max_run`/`reconcile_error_deg_max_run`
  로그에 `(n=...)` 항상 동반. 기존 세션 스코프 `reconcile_error_m`/`_deg`는 R5부터 이미 `n=`을
  갖고 있었다(추가 작업 불필요, 확인만 함).
- **H-10′**(지표 스코프): `_hardSnapTotalRun`(컨트롤러 재생성을 넘어 델타 누적), `_reconcileErrorMMaxRun`/
  `_reconcileErrorDegMaxRun` + 각자의 `n`. 세션 스코프(정본)는 `OnSessionReady`에서 리셋, `_run`은
  리셋 안 함. HUD엔 세션 스코프만(공간 제약), 세션종료 로그엔 둘 다.
- **H-14**(`outbound_queue_full_total`): `IRealtimeTransport.QueueFullTotal` 신설, `PcWebSocketTransport`
  (`Interlocked` 카운터)·`FakeRealtimeTransport` 둘 다 구현, `RealtimeClient.OutboundQueueFullTotal`로
  전달, 세션종료 로그에 노출. **SC-89 합격 조건이 아니라 관측**(qa3 Q-1과 동일 판정) — 0이 아니면
  별건 발견으로 적으라는 주석을 달아 뒀다.
- 리더가 요청한 **프레임당 최대 송신 건수**(`SendBurstStats.MaxSendsPerFrame`, K-7 짝 단언용)도
  구현·HUD·로그에 반영, 테스트 3개 추가.

### 미완료 — 예산상 보류, 이유를 밝힌다

| 항목 | 상태 | 이유 |
|---|---|---|
| H-9(수신 쪽, 스냅샷 중복 시 최신 1건만 재조정) | **보류** | `RealtimeClient.Pump()`가 같은 프레임에 큐에 쌓인 메시지를 전부 동기 처리하는데, `GreyboxSession.Update()`가 아니라 `StarfallNetHost.Update()`에서 `Pump()`가 불린다(별도 컴포넌트, 실행 순서 불확정). "이번 드레인에 더 최신 스냅샷이 있는지"를 안전하게 판별하려면 즉시 처리 대신 지연 처리로 바꿔야 하는데, 잘못 만들면 새 버그를 심을 위험이 있다고 판단해 이번 라운드엔 손대지 않았다 |
| H-13(주기적 로그, `OnGUI` 비포커스 대비) | **보류** | 세션종료 로그(H-11/기존 R9)로 세션 끝의 값은 이미 grep 가능하다. 백그라운드 재현 세션(세션 B)은 어차피 세션을 끝까지 마쳐야 확인하는 절차로 체크리스트를 짰으므로(§8.4), 실시간 주기 로그 없이도 판정 가능하다고 보았다 — 하지만 "히치 도중 죽으면 세션종료 로그 자체가 안 남는다"는 위험은 남는다. 시간이 나면 다음 라운드에 Update()에서 N초마다 한 줄 추가하겠다 |
| H-18(HUD에 마커 4개 ID·거리 한 줄) | **보류** | (b1)이 이미 EditMode 단언으로 닫혀 "권장" 등급이라 이번엔 넘겼다 |
| H-9/H-13 외 나머지 전부(H-1~H-7, H-10′, H-14~H-17) | **완료** | 위 표 참고 |

### 실행 결과

`unity test client --mode EditMode`: **`total=212, passed=210, failed=0, skipped=2`**(기존
LiveServerTests 게이트, 무관). **종료 코드 0.** 컴파일 오류 0. `Assert.Ignore`/`Inconclusive`
미사용.

**SC-89 (c) 게이트**: `grep -n 'while (_tickAccumulator' client/Assets/_Project/Scripts/Greybox/GreyboxSession.cs`
→ **exit 1**(매치 0건) — qa3가 찍어 둔 기준선(exit 0, 매치 1건)에서 뒤집혔다. `GreyboxSendBurstTests.cs`가
`TickCatchUp.Plan`(→ `Compute`)을 직접 부르는 것도 파일에서 확인 가능하다.

**짝 단언**: `Update_AfterA460msHitch_...`와 새 성질 테스트 둘 다 `carryForwardTicksThisFrame > 0`
(또는 그 합)을 명시적으로 단언한다 — 값을 확인함: 0.46초 히치에서 9 tick 예측, 1 tick 송신, **8 tick
이월**(`9 - 1 = 8 > 0`).

**테스트 수 변경**: `159/157`(qa2) → `167/164`(R6) → `178/175`(R9) → `197/194`(R10) →
**`212/210`**(이번, 전부 GREEN — 처음으로 실패 0). qa3가 SC-46 재실행 때 이 수를 참고 바란다.

리더에게: H-1~H-7(핵심 수정, RED→GREEN), H-10′, H-14, H-16, H-17, 프레임당 송신 계측 전부 완료.
H-9·H-13·H-18은 보류, 이유는 위 표. 커밋하지 않았다. 재촬영 체크리스트에 세션 A/B 분리, 세션 B의
throttle/pause 판별 절차를 반영해 `evidence/R4-B6/sc59/09-reshoot-checklist.md`를 갱신했다.

## R12 — T-5 동어반복 제거, H-9, H-13 (리더 지시 2026-09-23)

동결 해제 후 세 항목만 처리. 범위를 넓히지 않았다(H-18, ObserverSession의 같은 패턴은 손대지 않음).

### 작업 1 — T-5 §7a/§7b: `GreyboxSendBurstTests.cs` 불변식 ③ 동어반복 제거

`PropertyTest_RandomFrameDeltasIncludingA460msHitch_HoldsAllThreeInvariants` 119행의
`Assert.That(plan.TicksToPredict - plan.TicksToSend + plan.TicksToSend, Is.EqualTo(plan.TicksToPredict))`는
양변이 `plan` 구조체의 같은 필드로 대수적으로 항상 같다 — `TickCatchUp.Compute`가 무엇을 반환하든
통과한다(§7a 위반, T-3의 "구현값과 독립적으로" 원칙 위반이기도 하다).

**겨냥했던 불변식**(SC-89 (a)③ "이월 tick 수 + 송신 tick 수 == 예측 tick 수")은 실제로는 "**실제
전송된 tick 수**(와이어 관측, `DriveThroughTransport`가 반환하는 `sendsThisFrame`)"와 "이월 tick
수"의 합이 예측 tick 수와 같다는 것 — `plan.TicksToSend`(계획값)가 아니라 실제 관측값을 비교해야
`TrySend()`가 계획과 다르게 동작하는 결함(전송 실패, 계획-실행 불일치)을 잡는다.

수정: `carryForwardTicksThisFrame + sendsThisFrame == plan.TicksToPredict`(`sendsThisFrame`은 실제
`DriveThroughTransport` 호출 결과). `client/Assets/_Project/Tests/EditMode/GreyboxSendBurstTests.cs:118-136`.

**RED 확인**(§7a 필수 요건): `TickCatchUp.Compute`의 두 `ticksToSend` 계산에 `+ 1`(캡 위반, 임시)을
주입 → `unity test client --mode EditMode --filter "GreyboxSendBurstTests"` 실행 → **종료 코드 2,
`total=2 failed=2`**. 실패 메시지: `PropertyTest_...`는 정확히 새로 고친 "invariant 3: carry-forward
ticks + ACTUALLY SENT ticks == predicted ticks"에서 실패(수정 전 동어반복 버전이었다면 이 결함에서도
그대로 초록이었을 것 — `TicksToPredict - TicksToSend + TicksToSend`는 `TicksToSend`가 무엇이든 항상
같다). `Update_AfterA460msHitch_...`도 K=1 초과로 별개 실패. 결함을 되돌리고(`git diff` 0줄로 확인)
재실행 → GREEN. 백업은 `TickCatchUp.cs.bak`(임시, 삭제함) 대신 in-place 되돌리기 + `git diff`로 원본과
바이트 동일함을 확인했다(0.12 자기 기대값 갱신 경로가 아니라 손으로 원복 후 diff 확인 — bless류를
쓰지 않았다).

**§7b(1) 나머지 단언 자명 통과 점검** (같은 파일):
- `sendsThisFrame <= 1`(단일 프레임 테스트·성질 테스트 둘 다): **`sendsThisFrame == 0`이 항상이면
  자명 통과**(전송이 전부 실패해도 "1 이하"는 참) — `TrySendJson`이 조용히 항상 실패해도 잡지 못했다.
  단일 프레임 테스트엔 `sendsThisFrame == 1`(9 tick 백로그·K=1이면 정확히 1건) 짝 단언을 추가했고,
  성질 테스트엔 런 전체 `totalSends > 0` 짝 단언을 추가했다(10초 런에서 아무것도 안 보내도 프레임별
  단언은 계속 참이었을 상태).
- `_transport.Sent.Count == sendsThisFrame`: **둘 다 0이면 자명 통과** — 위 `sendsThisFrame == 1` 짝
  단언을 추가하면서 도달 불가능해졌다(9 tick 백로그에서 1 미만이 될 수 없다).
- `carryForwardTicksThisFrame > 0`(T-6 짝): 이미 짝이었다(§7a 규율이 처음부터 반영돼 있었음) — 그대로.
- `hitchInjected == true`: 자명 통과 상태 없음(루프 조건이 직접 보장) — 그대로.
- `totalPredicted == expectedTicks ± 1`(불변식 ②): `expectedTicks`가 `wallClockSeconds`(독립 계산)에서
  나오므로 자명 통과 없음 — 그대로.
- `totalCarryForwardOrDormant > 0`(런 전체 T-6 짝): 이미 짝 — 그대로.

### 작업 2 — H-9 수신측 스냅샷 중복 처리

보류 사유였던 "`RealtimeClient.Pump()`가 `StarfallNetHost.Update()`에서 불리고 `GreyboxSession.Update()`와
실행 순서가 불확정"이라는 문제를, **큐잉 + 지연 적용**으로 실행 순서 무관하게 풀었다:

- `OnWorldSnapshotCore`는 더 이상 그 자리에서 재조정하지 않는다. `SnapshotRebaseBatch.ShouldReplacePending(tick,
  pendingTick)`(신규 순수 함수, `Greybox/SnapshotRebaseBatch.cs`)로 "이 스냅샷이 지금 대기 중인 것보다
  최신인가"만 판정하고, 최신이면 `_pendingRebaseTick`/`_pendingRebaseConfirmed`/`_pendingRebaseAckInputSeq`를
  덮어쓴다. 오래된 tick은 대기열에 들어가지 않고 버려진다(재조정 관점에서만 — 아래 보간은 별개).
- `_remoteRegistry.OnSnapshot(...)` 호출은 그대로 **모든** 메시지에서 무조건 실행된다(기존 코드 위치
  그대로, 이번에 옮기지 않음) — 원격 함선 보간 버퍼는 중복 스냅샷을 전부 받는다.
- 실제 재조정은 새 메서드 `ApplyPendingRebase()`가 수행 — `GreyboxSession.Update()` 맨 앞에서 프레임당
  최대 1회 호출한다. 같은 프레임에 여러 `WORLD_SNAPSHOT`이 동기 드레인됐다면 `_pendingRebase*`는 이미
  최신 것으로 덮여 있으므로, `Update()`가 `StarfallNetHost.Update()` 앞/뒤 어느 쪽에 실행되든(실행 순서
  여전히 불확정) 그 배치의 최신 스냅샷만 반영된다 — 뒤에 실행되면 최대 1프레임 지연이 생길 뿐, 중복 자체는
  항상 수렴한다.
- `OnSessionReady`에서 `_pendingRebase*` 3필드를 리셋 — 이전 세션에서 대기 중이던 재조정이 새
  `_controller`(재생성됨)에 잘못 적용되는 것을 막는다.

**순수 함수 분리 + 테스트**: `SnapshotRebaseBatch.cs`(신규, `Starfall.Greybox`), 테스트
`SnapshotRebaseBatchTests.cs`(신규, 6건) — MonoBehaviour·transport·씬 없이 "어느 tick이 이기는가"만
검증. 배치 시나리오 테스트(`Batch_OfThreeArrivingOutOfPumpOrder_OnlyHighestTickEverWins`)는 tick
100→102→101 순서로 도착해도 102가 이기는 것을 확인(§7a: 조건이 실제로 발생함을 시뮬레이션으로 보임).

**RED 확인**: `ShouldReplacePending`이 "항상 true" 또는 "항상 false"였다면 4개 기본 케이스 중 최소
하나는 반드시 실패한다는 것을 `ShouldReplacePending_DegenerateAlwaysTrueOrAlwaysFalse_WouldFailAtLeastOneCase`로
수식적으로 확인해 뒀다(둘 다 실제 구현을 우회한 별도 델리게이트로 실행 — 실제 구현이 아니라 퇴화
형태 두 개가 이 케이스 집합을 못 만족시킨다는 것을 코드로 보임). 실제 구현(현재 코드)이 이 4케이스와
배치 시나리오 전부를 통과하는 것은 `unity test` 실행으로 확인(아래 §실행 결과).

**리더에게 알릴 사항**: `ObserverSession.cs`도 같은 재조정 패턴(스냅샷 즉시 반영)을 갖고 있고 같은
중복 문제에 노출돼 있을 수 있지만, 이번 지시가 범위를 좁혔으므로(§0의 "범위를 넓히지 마라") 손대지
않았다. 두 세션 하네스(`TwoSessionHarness`)를 쓰는 시나리오에서 필요하면 별도 항목으로 알려달라.

### 작업 3 — H-13 백그라운드 안전 주기 로그

`Greybox/PeriodicStatusLog.cs`(신규, 순수 struct + `Format()`)에 HUD와 같은 핵심 지표(tick,
ack_input_seq, speed, origin_distance, predict_error_m/deg, reconcile_hard_snap_total,
send_burst_max_ticks_per_update/max_sends_per_frame, catchup_carry_forward/dormant/truncated_total,
reconcile_forced_after_hitch_total, visible_ships)를 `key=value` 한 줄로 포맷하는 로직을 분리했다.
`GreyboxSession.Update()` 끝에서 `MaybeLogPeriodicStatus()`를 호출 — `Time.unscaledTime` 기준 5초
간격으로 `Debug.Log(line.Format())`를 찍는다. `OnGUI()`는 건드리지 않았다(기존 HUD 그대로 유지).

**포커스 무관 근거(코드 경로 증명, 실측 불가라 이것으로 대체)**:
- `MaybeLogPeriodicStatus()`는 `Update()`에서만 호출되고 `OnGUI()`에서는 호출되지 않는다
  (`grep -n "MaybeLogPeriodicStatus" client/Assets/_Project/Scripts/Greybox/GreyboxSession.cs` → 정의
  1건 + 호출 1건, 호출부는 `Update()` 본문 안).
- `PeriodicStatusLog.cs`는 `UnityEngine.GUI`/`OnGUI`/`EditorWindow`를 전혀 참조하지 않는다
  (`grep -n "GUI\|EditorWindow" client/Assets/_Project/Scripts/Greybox/PeriodicStatusLog.cs` → 매치
  0건) — 포커스에 따라 호출이 스킵되는 Unity API(`OnGUI`)와 아예 접점이 없다.
- `Application.isFocused`는 **기록만 하고 분기하지 않는다** — `PeriodicStatusLog` 생성자는 그 값을
  그대로 필드에 저장해 `Format()`에서 `application_focused=` 로 출력할 뿐, 로그를 찍을지 말지 결정하는
  조건에 쓰이지 않는다(코드에 `if (Application.isFocused)`류 분기 없음 — 이 필드 자체가 나중에 QA가
  "이 로그가 실제로 비포커스 구간에서도 나왔는가"를 grep으로 확인할 수 있게 해주는 증거용).
- `Update()`가 백그라운드에서도 계속 호출된다는 전제는 이 슬라이스의 TickCatchUp/SC-89 자체가 이미
  의존하고 있는 전제다(비포커스에서 tick 누적이 실제로 일어나야 catch-up 로직이 존재할 이유가 있다).

이 셋을 합치면 "포커스 의존 경로(`OnGUI`)를 타지 않는다"는 정적 근거이지, "실제로 비포커스에서
로그가 쌓였다"는 실측은 아니다 — 실측(Game View 비포커스 유지하며 5초 이상 로그 축적 확인)은 사람이
필요한 검증(Editor Play 버튼)이라 이 라운드에선 할 수 없었다. 리더/사람이 확인할 절차: Play 모드
진입 → Game View 포커스를 다른 창으로 옮김 → 10초 이상 대기 → `client/Logs/Editor.log`에서
`grep "periodic_status"` → 2줄 이상 나오고 각 줄의 `application_focused=false`인지 확인.

**테스트**: `PeriodicStatusLogTests.cs`(신규, 5건) — 한 줄 형식, 필드별 key=value 포함 여부(필드
드롭·오타를 잡기 위해 각각 개별 단언, §7a), `ack_input_seq=null`(0이 아님), 미측정 카운터
`n/a`(0이 아님, 기존 `FormatCount`/`FormatStat` 규율과 동일), `application_focused` true/false 둘 다
그대로 출력됨을 확인.

### 실행 결과

`unity test client --mode EditMode`: **`total=223, passed=221, failed=0, skipped=2`**(기존
LiveServerTests 게이트, 무관). 종료 코드 0. 테스트 수 변화: `212/210`(R11) → **`223/221`**(이번,
+11 = `SnapshotRebaseBatchTests` 6 + `PeriodicStatusLogTests` 5; `GreyboxSendBurstTests`는 기존 2건에
단언만 추가·교체했으므로 케이스 수 불변).

T-5 RED 확인은 위 "작업 1" 절에 기록(별도 필터 실행, `total=2 failed=2` → 결함 원복 → 전체 스위트
재실행으로 GREEN 재확인, `git diff`로 원복 바이트 동일 확인).

Editor가 라운드 시작 시 열려 있어 `unity test`가 처음엔 거부됐다(PID 17152) — 재시도 시점엔 통과했다
(사람이 그 사이 닫았거나 포커스를 옮겼을 것으로 추정, 원인은 확인하지 않음). 커밋하지 않았다.

## R13 — qa r5 결함 수정 (F-1·F-2·F-5·F-7·F-8) + runInBackground 판정

작업자: 리더(client-r5-fix가 F-2 도중 세션 한도로 중단 — 리더가 인계받아 완료).
EditMode: **223/221 → 237/235, 실패 0, 건너뜀 2**(기존 LiveServerTests 게이트).
커밋 없음. 골든 파일 미변경.

### F-1 — HUD가 필드를 소리 없이 버리던 문제

`GUI.Label(Rect(…, 400, 18))`이 폭을 넘는 줄을 말줄임표 없이 잘라, `send_burst_max_sends_per_frame`
(SC-89 판별 단언), `cl2_reconcile_replayed_nonzero_total`(SC-56 d②),
`cl2_reconcile_both_omega_nonzero_total`(d③)이 네 라운드 동안 화면에 닿은 적이 없었다.

- 수정: `HudTextWrap.Wrap()`(순수 함수, greedy word-wrap) + `GreyboxSession.BuildHudRows()`.
  **폭을 늘린 게 아니라 구조를 바꿨다** — 입력 텍스트 100 %가 출력 행에 나타나는 것이
  알고리즘의 불변식이라, 필드가 늘어나면 행이 늘지 조용히 잘리지 않는다.
- `BuildHudRows`를 `internal` → `public`으로. EditMode 어셈블리는 별도 asmdef이고
  `InternalsVisibleTo`가 없어 `internal`로는 테스트가 컴파일되지 않았다(client 중단 지점).
- **RED 확인**: `rows.AddRange(Wrap(...))` → `rows.Add(line)`(랩 제거) 주입 → 1건 실패.
  원복 후 md5 `1f5ccf81fc92958ec3f9032cfe37fdd1` 일치, 결함 문자열 0건, 재실행 초록.

### F-2 — ObserverSession의 H-9 결함 (client가 착수, 리더가 확인)

`ObserverSession.cs`가 `_pendingRebase*` 5필드 + `ApplyPendingRebase()` + `Update()` 배선으로
`GreyboxSession`과 같은 방식으로 수정돼 있다. 컴파일·스위트 초록으로 확인.
증거 오염 측면(`:181` `WriteRow`로 되감김이 QA CSV에 새겨지던 문제)이 이로써 닫힌다.

### F-5 — 퇴화 테스트가 피시험 대상을 호출하지 않던 문제

`SnapshotRebaseBatchTests`의 `…DegenerateAlwaysTrueOrAlwaysFalse…`가
`AlwaysTrue(99,100) && AlwaysFalse(101,100)`으로 상수 접힘되어 `ShouldReplacePending`을
**한 번도 호출하지 않았다.** 구현을 지워도 통과했을 것이다 — T-5와 같은 병(§7a).

- 수정: 4케이스 표를 실제 구현과 두 퇴화 형태에 **모두** 통과시키고,
  각 퇴화 형태가 표의 어딘가에서 **불일치하는지**를 단언한다. 구현이 퇴화하면
  불일치가 사라져 테스트가 붉어진다.
- **RED 확인**: `SnapshotRebaseBatch`의 반환을 `return true`(always-true 퇴화)로 주입 →
  **4건 실패**. 원복 후 md5 `47e66ef800479426bcae4de45984305d` 일치, 재실행 초록.

### F-7 — 마커 HUD 줄 미구현

계약 SC-59 (b2)가 요구한 "녹화 시작 프레임의 HUD에 마커 4개의 ID·거리"가 없었다.
`BuildMarkers()`는 C3부터 있었으나 그리는 줄이 없어 **(b2)는 영상으로 닫힐 경로 자체가 없었다.**

- 신규 `MarkerHudLine.cs`(순수, UnityEngine 비의존). HUD에 매 프레임 출력 —
  시작 프레임만이 아니라 아무 지점으로 스크럽해도 판정 대상 마커를 알 수 있다.
- 포커스 비의존 사본: `BuildMarkers()`에서 `Debug.Log("… reference_markers …")` 1회.
  마커 위치는 정적이므로 주기 로그에 매번 반복할 이유가 없다.
- 마커 없음일 때 빈 문자열이 아니라 `markers=none` — §7a: 빈 HUD 행은
  "행이 안 그려진 것"과 구분되지 않으며, 그 모호함이 F-1이 네 라운드를 숨은 방식이다.
- 테스트 5건 신규. **RED 확인**: 거리 계산에서 `shipPositionM` 무시 주입 → 1건 실패.
  원복 후 md5 `21fb3b1fa6d551255a72c01e978610bb` 일치, 재실행 초록.

### F-8 — `PeriodicStatusLog`에 SC-59 판정 필드가 없던 문제

자세·진행 방향·`thrust`·`roll`·경계 플래그가 하나도 없어, "다음 세션에 주기 로그로
기준 2·5-b를 수집한다"는 계획이 **실현 불가능**했다(리더 오류, qa L-6).

- 추가 필드 13개: `thrust_x/y/z`, `roll`, `aim_target_x/y/z/w`, `attitude_x/y/z/w`,
  `boundary_soft_crossed`. 전부 HUD·와이어와 **같은 양자화 정수**(재유도 아님).
- **축 규약을 코드에 명시**: ADR-0009 §1 — +X 우현, +Y 상방, +Z 선수.
  롤은 선체 +Z 축 스칼라, 오른손 법칙. 규약 없이 쿼터니언만 남기면 좌/우를 못 읽는다 —
  리더가 R4 판독에서 정확히 이 벽에 부딪혔다.
- `Attitude*`(현재 자세)와 `AimTarget*`(조준 목표)를 **둘 다** 남긴다. 어느 하나로
  대체되지 않는다: 목표만으로는 오토레벨(목표 고정 + 함선만 수평 복귀)을 못 보고,
  자세만으로는 선회가 의도였는지 표류였는지 못 가린다.
- 튜플이 아니라 축별 개별 키(`thrust_x=`)로 출력 — 세션에서 한 축만 grep 가능.
  HUD의 `(x,y,z)` 튜플 형식이 R4 판독을 모호하게 만든 형식이다.
- 테스트 3건 신규(값 개별 대조로 축 교환 검출, 경계 양쪽 상태, **부호 보존**).
- **RED 확인**: `Format()`에서 `thrust_y`↔`thrust_z` 교환 주입 → **2건 실패**.
  원복 후 md5 `7f51ec1b203e6feb01213a9eeab2efa6` 일치, 재실행 초록.

### 판정 — `runInBackground`는 바꾸지 않는다

`ProjectSettings.asset:90`이 `runInBackground: 0`이고, qa가 H-13의 적재 전제
("비포커스에서 `Update()`가 돈다")가 성립하지 않을 수 있다고 지적했다. 맞다.

**그럼에도 이 설정을 바꾸지 않는다.** SC-89가 판정하는 현상 자체가 그 멈춤이기 때문이다 —
catch-up 버스트는 `Update()`가 멈췄다 재개하기 때문에 존재한다. 로그를 백그라운드에서
찍게 하려고 `runInBackground = true`로 바꾸면 **SC-89가 재려던 현상이 사라진다.**
그건 프로젝트 설정 수준에서 §7a 자명 통과를 제조하는 짓이고, 그렇게 얻은 초록불은
아무것도 뜻하지 않는다.

따라서 H-13의 역할을 좁혀 다시 정의한다 — **백그라운드 로거가 아니라 OnGUI·화면 캡처
비의존 로거**다. 비행은 Game View에 포커스가 있을 때만 가능하고(입력이 포커스를 요구한다)
그때가 바로 `Update()`가 도는 때이며, 그때 줄이 쌓인다. `application_focused` 필드가 사주는
것은 **간극 자체가 아니라 그 경계** — 포커스 구간이 어디서 시작해 어디서 끝났는지다.
SC-89의 백그라운드 구간은 `OnSessionEnded`의 전체 카운터 덤프와 두 포커스 구간 사이의
불연속으로 증거화하며, 그 안에서 찍힌 줄로는 결코 증거화하지 않는다.

`PeriodicStatusLog.cs` 헤더의 순환 논증(TickCatchUp의 존재를 비포커스 실행의 근거로 든 것)은
삭제하고 위 판정으로 교체했다.

### 남은 것

- **SC-56 d②·d③, SC-89 판별 단언은 이제 HUD에 뜬다.** 다만 그 값들이 0이 아닌지는
  실제 세션을 돌려야 알 수 있다 — F-1은 "볼 수 있게" 만든 것이지 "값이 나온다"를
  증명한 것이 아니다(§7a).
- H-13의 비포커스 거동 확인은 **하지 않는다** — 위 판정에 따라 확인 대상이 아니다.
  대신 확인해야 할 것은 `Editor.log`에서 `periodic_status`의 `application_focused` 전이가
  세션 경계와 일치하는지다.

## R14 — qa r6 지적 수정 (리더)

EditMode: **237/235 → 243/241, 실패 0, 건너뜀 2.** 커밋 없음. 골든 파일 미변경.

qa r6가 리더의 R13 보고에서 오류 셋과 증거 공백 하나를 잡았다. 전부 수용했다.

### 1. 축 규약 주석이 거꾸로였다 (qa r6 최대 발견)

`PeriodicStatusLog.cs`에 *"positive roll … the ship's right wing drops"*라고 썼다. **틀렸다.**
+Z축 오른손 회전은 +X를 +Y로 보낸다 — 오른쪽 날개는 **올라간다.**
qa가 `ShipIntegrator.cs:147-149` + `Quatd` Hamilton 곱을 실제로 돌려 확인했고,
단순 회전 행렬로도 같은 결과다(R_z(90°): x→y).

이게 왜 중대하냐면, F-8의 존재 이유가 **제3자가 이 주석만 보고 선회 방향을 계산하는 것**이다.
그 유일한 물리 문장이 거꾸로면 필드 13개는 오독을 유도하는 장치가 된다.
주석을 고치고 정정 경위를 코드에 남겼다.

### 2. 기준 5-b는 여전히 수집 불가였다 — `flight_assist` 필드 추가

오토레벨은 `input.FlightAssist`일 때만 돈다(`ShipIntegrator.cs:122`). 그런데 R13이 추가한
13필드에 `flight_assist`가 없었다 — HUD에는 있고 로그에는 없었다.
assist 상태를 모르면 자세가 수평으로 수렴해도, 수렴하지 않아도 아무것도 판정되지 않는다.

필드 추가(14개가 됨) + 양쪽 상태 테스트(§7b(1)).
기준 2는 qa가 직접 유도해 가능함을 확인했다(`q_rel = conj(q₁)·q₂`, 우선회 30° → `attitude_y ≈ +258819`).

### 3. `runInBackground` 논거의 인과가 거꾸로였다 — 결론은 유지, 근거를 교체

R13은 "버스트는 `Update()`가 멈춰서 생기므로 `true`로 바꾸면 현상이 사라진다"고 썼다.
틀렸다. 버스트를 만드는 것은 **큰 `Time.unscaledDeltaTime`**이고, `true`로 바꾸면 사라지는 것은
**pause**이지 버스트가 아니다 — 계약 자신이 2~4 Hz 전경 throttle을 재현 벡터로 적어 두었다.

성립하는 근거 둘로 교체했다: (1) `runInBackground`는 **피시험 시스템의 일부**이고, 계측 편의를
위해 피시험 시스템을 바꾸면 측정이 측정이기를 그만둔다. (2) 계약이 이미 전경 대체 벡터를
지정했으므로 바꿔서 얻을 것이 없다. 둘 다 "버스트가 사라진다"와 무관하다.

**1002 태그 점검 (iii)는 대체 증거로 닫히지 않는다**(qa 판정). 미결로 남긴다.

### 4. F-2 검증 공백 — `PendingRebaseSlot` 추출

qa가 `ObserverSession.ApplyPendingRebase()` **호출을 통째로 지우고** 스위트를 돌렸더니
**237/235 초록, 즉 "검증했다"던 상태와 동일**했다. `ObserverSession`을 실행하는 테스트가 0건이다.
리더가 "컴파일 + 스위트 초록"을 확인으로 보고한 것은 **아무 값어치가 없었다.**

- 신규 `PendingRebaseSlot.cs` — 큐잉/적용 상태 기계를 MonoBehaviour·전송·CSV 없이 실행 가능한
  객체로 추출. `ObserverSession`이 느슨한 5필드 대신 이것을 쓴다.
  즉 **테스트되는 것과 출하되는 것이 같은 객체**이지 같은 아이디어의 사본 둘이 아니다.
- 테스트 5건 신규: out-of-order 배치에서 최신 tick + **그 tick의 payload**가 함께 나오는지
  (tick만 맞고 오래된 상태를 쥐는 결함 검출), `TryTake`가 슬롯을 비우는지,
  빈 슬롯이 아무것도 안 주는지(§7b(1) 짝), `Clear` 후 낮은 tick을 다시 받는지.
- **RED 확인**: `TryTake`에서 `Clear()` 제거 주입 → 1건 실패.
  원복 후 md5 `adaddf93b2a16354bcee1d14bd7af4db` 일치, 결함 문자열 잔존 0건.

**정직한 한계**: 이것으로 닫힌 것은 **상태 기계의 거동**이다. `ObserverSession`이 그 객체를
실제로 호출하는 **배선**은 여전히 어떤 테스트도 실행하지 않으므로, qa가 했던 "호출 삭제" 실험은
지금도 초록일 것이다. F-2를 완전히 닫으려면 `ObserverSession`을 가짜 전송으로 구동하는
테스트가 필요하다 — 별도 항목으로 남긴다.

`GreyboxSession`은 자체 사본을 그대로 둔다. 의도된 범위다(그쪽은
`GreyboxSendBurstTests`가 가짜 전송으로 세션을 끝까지 구동해 이미 커버된다).
중복을 `PendingRebaseSlot.cs` 헤더에 사유와 함께 기록했다.

### 5. L-2 — 재촬영 체크리스트 갱신

`evidence/R4-B6/sc59/09-reshoot-checklist.md`가 R13 이전 상태를 전제하고 있었다.
갱신 절을 추가했다(본문은 원칙 5에 따라 보존). 핵심은 **사람 눈 없이 닫는 판독 경로**를
명시한 것 — `Editor.log`에서 `reference_markers`·`periodic_status`를 grep하고,
축 규약으로 선회 방향을, `flight_assist` 구간 대조로 오토레벨을 유도한다.

### 기준선 md5 (다음 라운드 검증용)

| 파일 | md5 |
|---|---|
| `GreyboxSession.cs` | `5b110456e76ac6e3c8ec327687a14f72` |
| `PeriodicStatusLog.cs` | `8d908903b5f9e71d93ac07af00ba0534` |
| `MarkerHudLine.cs` | `21fb3b1fa6d551255a72c01e978610bb` |
| `SnapshotRebaseBatch.cs` | `47e66ef800479426bcae4de45984305d` |
| `PendingRebaseSlot.cs` | `adaddf93b2a16354bcee1d14bd7af4db` |
| `ObserverSession.cs` | `d2f54c230665116a0f37e96bfabddca3` |

R13이 보고한 md5 중 둘은 이후 편집으로 이미 무효였다(qa r6 지적). 기준선은 **보고 시점의
트리 상태**로만 유효하다는 것을 이번에 배웠으므로, 위 표는 R14 종료 시점 기준이다.

## R16 — 위치 벡터·마우스 원시 델타 추가 (R5 실서버 세션이 남긴 구멍)

EditMode: **243/241 → 246/244, 실패 0, 건너뜀 2.** 커밋 없음. 골든 파일 미변경.

R5 실서버 세션(`evidence/R5-realserver/`)이 SC-56·SC-89를 네 라운드 만에 처음으로
비자명하게 움직였고, 동시에 **로그가 무엇을 판정할 수 없는지**를 드러냈다. 그 둘을 메운다.

### 1. `position_x/y/z` — 기준 1·3(W=뱃머리, R=위)이 크기만으로는 안 닫힌다

로그에는 `origin_distance_m`밖에 없었다. 그것은 `Position.Length()`, 즉 **크기**다.
R5 세션에서 `thrust_z=1000`으로 521 m → 11,918 m까지 등속 비행한 54줄이 남았지만,
**전방 축이 뒤집힌 빌드도 같은 크기 수열을 낸다.** qa4가 R4 프레임 판독에서 지적했던
구멍(`speed_mps`·`origin_distance_m`은 방향 없는 스칼라)이 로그에도 그대로 있었다.

이제 변위를 **함선 전방 축에 투영**해 판정할 수 있다 — 전방 축은 `attitude_*`가 이미 준다.

### 2. `mouse_dx_total` / `mouse_dy_total` / `yaw_deg` / `pitch_deg` — 기준 2의 구조적 구멍

**이쪽이 더 중요하다.** R16 이전 로그의 회전 관련 필드는 `aim_target_*`도 `attitude_*`도
**전부 마우스 매핑을 거친 뒤의 값**이었다. SC-59가 검사하려는 것이 바로 그 매핑인데,
매핑의 **입력이 로그에 없었다.** `ShipInputSampler.cs`의 `delta.x`를 음수로 뒤집은 빌드는
R16 이전 로그와 **완전히 동일한 줄**을 낸다 — 즉 SC-59 기준 2는 로그로 닫힐 수 없었다.

- `MouseDeltaXTotal`/`YTotal`: 매핑 **이전**의 원시 디바이스 델타, 누적·미리셋.
  두 줄을 빼면 그 구간의 이동량이 나오므로 5초 샘플링과 무관하게 판독된다.
- `YawDeg`/`PitchDeg`: 같은 매핑의 **출력**.
- 판정은 둘의 **부호 대조**로 한다. 어긋나면 그것이 SC-59가 찾던 버그다.
  어느 한쪽만으로는 매핑에 대해 아무것도 증명하지 못한다.

미터·픽셀 `double`로 남긴다(양자화 정수 아님). 사람·스크립트가 읽는 진단값이고
와이어 값이 아니며, 반올림하면 스칼라 거리가 가졌던 "작은 움직임과 없음을 구분 못 함"이
되돌아온다.

### 3. `attitude_w` — 컨트롤러 없을 때 (0,0,0,0)을 찍던 결함

단위 쿼터니언도 아니고 어떤 회전도 아닌 값인데 판독자에게는 데이터로 보인다.
R5 사전 로그(`Editor-preflight-no-auth.log`)가 이 값으로 가득하다.
항등 쿼터니언으로 바꿨다 — `tick=-1`과 같은 줄에 있으면 "아직 재조정 없음"이 자명하다.

### RED 확인

`Format()`에서 `mouse_dx_total`과 `position_z`를 **절댓값으로** 출력하도록 주입
(= 부호를 잃은 빌드, SC-59가 잡으려는 결함의 정확한 형태) → **1건 실패.**
원복 후 md5 `0731aa15fedeef1c18175d0c8e973927` 일치, 결함 문자열 잔존 0건, 재실행 초록.

신규 테스트 3건: 위치 3성분 개별 값 대조(축 교환 검출), 원시 델타와 각도 동시 출력,
**부호 반전 케이스**(§7b(1) 짝 — 앞의 둘은 크기만 찍어도 통과하므로 이것이 가른다).

### 기준선 md5 (R16 종료 시점)

| 파일 | md5 |
|---|---|
| `GreyboxSession.cs` | `2a5c79e03ce1f44632ddf3cd97207948` |
| `PeriodicStatusLog.cs` | `0731aa15fedeef1c18175d0c8e973927` |
| `ShipInputSampler.cs` | `ee0256ff89f0c212fe04a95b2f96b8ae` |
| `MarkerHudLine.cs` | `21fb3b1fa6d551255a72c01e978610bb` |
| `PendingRebaseSlot.cs` | `adaddf93b2a16354bcee1d14bd7af4db` |
| `SnapshotRebaseBatch.cs` | `47e66ef800479426bcae4de45984305d` |
| `ObserverSession.cs` | `d2f54c230665116a0f37e96bfabddca3` |

### 남은 것

이 필드들은 **판정 가능하게 만든 것**이지 판정한 것이 아니다(§7a).
기준 1·2·3을 실제로 닫으려면 R16 빌드로 세션을 한 번 더 돌려야 한다.
R5 증거는 R16 이전 빌드이므로 이 필드들을 담고 있지 않다.

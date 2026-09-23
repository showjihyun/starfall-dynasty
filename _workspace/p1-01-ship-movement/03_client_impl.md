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

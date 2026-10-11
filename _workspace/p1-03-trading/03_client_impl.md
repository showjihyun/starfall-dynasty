# p1-03-trading 클라이언트 구현

- 작성: client4
- 범위: C1 (계약·데이터 사본·SC-50 개정) · C2 (판매 표시 로직, UI-01~13 ① + AC-16) · C3 (그레이박스 판매 화면, UI-01~13 ② + AC-17 키)

## C1 — 계약·데이터 사본·SC-50 개정

### 1. codegen 재실행

`cd tools/codegen && dotnet run ContractsCodegen.cs -- --contracts ../../contracts --out ../../client/Assets/_Project/Scripts/Contracts/Generated`

- 신규 DTO 5개: `SellMineralCommand`·`MineralSoldEvent`·`WalletStateMessage`·`SaleReceiptMessage`·`MarketStateMessage`.
- 기존 16개 unchanged, `ContractTypes.cs`는 wrote(등록 갱신) — architect3 예측과 일치.
- 2회 연속 실행 모두 unchanged → 멱등 확인(sha256 비교).
- `MARKET_STATE.last_sale`: `Required.AllowNull` 중첩 클래스로 생성 — architect3 예측과 일치.

### 2. 생성기 결함 발견·조치 (스키마 title + 생성기 충돌 가드, 양쪽 다 리더 최종 결정)

최초 생성된 `MarketStateMessage.cs`가 컴파일 안 됨(`CS0102: already contains a definition for 'LastSale'`) — `$defs/LastSale`(title: "LastSale")가 `last_sale` 필드(PascalCase "LastSale")와 같은 containing class 안에서 이름이 겹쳤다. 몇 차례 방향이 엇갈린 뒤(스키마 title 변경 ↔ 생성기 이름 규칙 변경) 리더 최종 결정: **스키마 title("MarketLastSale")로 충돌을 풀고, 생성기 이름 규칙은 바꾸지 않되, 같은 충돌을 생성 시점에 잡는 가드를 추가한다.**

`tools/codegen/ContractsCodegen.cs`의 `BuildProperties`에 가드 추가: 중첩 클래스를 `model.Nested`에 붙이기 전, 그 클래스 이름이 **자기를 담은 속성의 C# 이름과 같으면** JSON Pointer 경로와 함께 `CodegenException`을 던진다(이름 규칙 자체는 안 바꾼다 — 충돌을 스키마 title로 풀도록 강제만 한다).

**음성 대조(실측)**: `contracts/`를 임시 복사해 `MARKET_STATE.schema.json`의 title만 "LastSale"로 되돌린 사본에 생성기를 돌리면:
```
codegen: .../MARKET_STATE.schema.json/properties/payload/properties/listings/items/properties/last_sale:
nested class 'LastSale' has the same name as its own property 'LastSale' in 'MarketListingState' - ...
```
정확히 그 경로에서 실패한다 — 가드가 살아있다는 증거.

**회귀 확인(실측)**: 실제 `contracts/`(title = "MarketLastSale")로 재생성 → **22개 파일 전부 `unchanged`**(가드 추가 전후 sha256 동일, `MarketStateMessage.cs` 포함) → `--check` 종료 코드 0("up to date: 22 file(s)") → `unity test client --mode EditMode` **488건 중 486 passed · 2 skipped · 0 failed**(결과 파일 `_workspace/p1-03-trading/evidence/c2/editmode-sc60-codegen-guard-20261006.xml`).

### SC-60 자기 점검 (qa-throughput R3, §0.3a) — 생성기 가드

| 칸 | 내용 |
|---|---|
| ① 이름 실행 | `dotnet run ContractsCodegen.cs -- --contracts ../../contracts --out ... --check` → exit 0, "up to date (22 file(s))". `unity test client --mode EditMode` → 488/486/2/0(필터 없이 전체, `ContractFixtureTests`만 봐도 181/181). |
| ② 구현 변이 → 실패 | 가드 자체가 "변이하면 실패를 보여야 하는 대상"이 아니라 "충돌 입력에서 실패해야 하는 대상"이라 변이 대신 **음성 대조**로 증명(위 코드 블록) — `contracts/` 임시 사본에서 title을 "LastSale"로 되돌리면 정확히 그 경로에서 `CodegenException`. |
| ③ 원복 증명(sha256) | `tools/codegen/ContractsCodegen.cs` 가드 추가 전후 — 이 파일은 가드가 영구히 남아야 하는 신규 기능이라 "원복"대상이 아니다(산출물 쪽이 원복 대상). `Generated/*.cs` 22개 전부 가드 추가 **전후 sha256 동일**(`codegen_pre_guard.sha256`/`codegen_post_guard.sha256` diff 없음) — 가드가 기존 산출물에 아무 영향도 주지 않음을 증명. |
| ④ 도구 존재 | `tools/codegen/ContractsCodegen.cs`의 `BuildProperties` 안, `model.Nested.Add(shape.Nested)` 직전. |

### 3. 클라이언트 데이터 사본 (ADR-0015)

- `client/Assets/_Project/Data/history/` 전체 제거(`SIGNIFICANCE_RULE`의 `consumers`가 `history`뿐 — client 아님).
- `client/Assets/_Project/Data/market/cradle-exchange.json` 추가(서버 원본과 바이트 동일).
- `client/Assets/_Project/Data/world/deposits/cradle.json`을 서버 v2(공개부, `mineral_id`·`initial_reserve_kg` 없음)로 교체.
- 결과: 사본 10파일, 레지스트리 유도 기대 집합(`SHIP_CLASS`·`STAR_SYSTEM`·`SYNC_TUNING`·`MINERAL`×4·`DEPOSIT_FIELD`·`MINING_RULES`·`MARKET` = 10)과 정확히 일치. `python tests/e2e/data_copy_scope.py` PASS(qa3 독립 구현).

### 4. SC-50 → SC-56 개정: 레지스트리 유도 기대 집합

`ContractFixtureTests.cs`의 `ClientDataCopy_MatchesRepositoryOriginal`을 `ClientDataCopy_MatchesRegistryDerivedSet`으로 재작성:

- 기대 집합을 `contracts/registry/types.json`에서 유도(`kind == "data"` ∧ `"client" ∈ consumers`인 타입의 `data_files` glob을 `data/`에 전개) — 더 이상 `data/` 전체를 그대로 걷지 않는다.
- 독립 순회 두 번: ① 레지스트리 유도(데이터 걷지 않고 레지스트리만 읽어 glob 전개) ② 클라이언트 사본 전체 걷기.
- 기대 집합 크기 > 0 단언(공집합 배제).
- 음성 대조 신설: `ClientDataCopy_ServerOnlyKindFile_IsDetected_NegativeControl` — 실제 서버 전용 파일(`DEPOSIT_RESERVE`, `consumers: ["server"]`)을 사본에 주입하면 copy-only로 잡힘을 증명(p1-02 누출의 정확한 재현 형태).
- 기존 `ClientDataCopy_CopyOnlyFile_IsDetected_NegativeControl`(임의 파일)도 레지스트리 유도 기준으로 갱신.

### 5. 반례 113건 재분류 (§0.5 ③, 실측)

39개 신규 + `DEPOSIT_FIELD` 순증 1(반례 1 삭제·2 추가) = 40 new - 1 removed = net +39. 전부 C1이 생성된 DTO로 실측(추측 없음):

- **field-injected/missing/\*-null/importance-level-injected**: 예측대로 거부(additionalProperties: false, Required.Always 위반).
- **\*-zero/\*-negative**: 예측대로 감지 불가(C# int/long에 런타임 하한 검사 없음).
- **\*-fractional into int** (`SELL_MINERAL.quantity_kg`): 예측대로 거부, `JsonReaderException`(리더 레벨).
- **\*-fractional into long** (`MINERAL_SOLD.proceeds_cr`, `WALLET_STATE.balance_cr`): **예측과 다름** — Strict가 수락했다. int 필드와 long 필드에서 Newtonsoft의 분수 토큰 처리가 다르다(예측 당시 몰랐던 사실). architect3·qa3에 통지 완료.
- **above-mass-max** (`SELL_MINERAL.quantity_kg`, C# int): 예측대로 거부, Int32 오버플로로 `JsonReaderException`(리더 레벨 — 기존 "정확히 이 두 건만" 재좁힘 테스트가 자동으로 걸림, 4건으로 확장).
- **above-safe-integer** (`WALLET_STATE.balance_cr`, C# long): 예측대로 감지 불가(2^53-1 초과 값도 Int64 안에 들어간다).
- **DEPOSIT_FIELD/DEPOSIT_RESERVE/MARKET**(kind: "data"): 예측대로 계층 없음(AC-10(d), DTO 생성 안 됨).

**최종 실측**: 거부 51 / 감지 불가 35 / 계층 없음 27 = 113 (계약 예측 53/33/27과 다름 — 2건이 거부에서 감지 불가로 이동, 원인·근거는 위 표).

### 6. 신규 테스트

- `MarketState_LastSale_NullAndNonNull_RoundTrip`(SC-62) — `MARKET_STATE/one-crashed-one-fresh.json`(같은 메시지에 null·비null `last_sale` 둘 다)로 왕복 확인.
- `UnknownClosedValue_ReasonCode_SurvivesRuntime`에 `INSUFFICIENT_QUANTITY` 추가(알려진 값 목록).

### 7. 상수 갱신

`ContractFixtures.cs`: `ExpectedValidFixtureCount` 46→57, `ExpectedInvalidFixtureCount` 74→113, `ExpectedRoundTrippableValidFixtureCount` 36→45, `DataOnlyTypes` += `DEPOSIT_RESERVE`·`MARKET`.

## 테스트 결과

`unity test client --mode EditMode` (필터 없음, 전체 스위트): **472건 중 470 passed · 2 skipped(ignored) · 0 failed**. `ContractFixtureTests`만 필터링: **181/181 passed**. 결과 파일: `_workspace/p1-03-trading/evidence/c1/editmode-20261006.xml`.

바이너리 출처: `git rev-parse HEAD` = `b3bc8f28948dfaa67b3664dc139ad9aae3a09a96`, `git status --porcelain` 더러움(이 슬라이스 작업 중 — 커밋 전).

## 자기 점검 표 (qa-throughput R3, §0.3a)

| SC | ① 이름 실행 | ② 구현 변이 | ③ 원복 증명(sha256) | ④ 도구 존재 | ⑤ DB | 비고 |
|----|---|---|---|---|---|---|
| SC-56 | `ClientDataCopy_MatchesRegistryDerivedSet`·`ClientDataCopy_CopyOnlyFile_IsDetected_NegativeControl`·`ClientDataCopy_ServerOnlyKindFile_IsDetected_NegativeControl` 3건 모두 PASS, 다른 SC 안 섞임 | `RegistryDerivedClientDataFiles`의 `consumer == "client"` 비교를 `"MUTATION_client"`로 변조 → `ClientDataCopy_MatchesRegistryDerivedSet` FAIL("expected set must not be empty") | `ContractFixtureTests.cs` 변이 전 `156119e8...a5fc77`, 원복 후 `156119e8...a5fc77` (동일) | `python tests/e2e/data_copy_scope.py` 독립 구현 PASS | — | 해당 없음(DB 테스트 아님) |
| SC-60 | `Fixtures_RoundTrip_Found57_RoundTripped45`·`CSharpLayerSplit_Totals113` 2건 PASS | `ContractFixtures.ExpectedValidFixtureCount`를 57→58로 변조 → `Fixtures_RoundTrip_Found57_RoundTripped45` FAIL("Expected at least 58 valid fixtures, found 57") | `ContractFixtures.cs` 변이 전 `58047960...6310a`, 원복 후 `58047960...6310a` (동일) | codegen 2회 실행 unchanged(멱등), `dotnet run ContractsCodegen.cs` 존재 | — | 해당 없음 |
| SC-61 | 위와 같음(§0.5 ③ 분류는 `CSharpLayerSplit_Totals113`·`Invalid_Rejected_VisitedEveryCSharpCase`·`Invalid_Rejected_JsonReaderExceptionSubset_IsExactlyTheReaderLevelNumericCases`가 실측) | 위 SC-60 변이가 `CSharpLayerSplit_Totals113`도 함께 거쳤으나 그 변이로는 불변(카운트가 분리됨) — 별도 변이로 `UnknownClosedValue_ReasonCode_SurvivesRuntime`의 `known` 목록에서 `INSUFFICIENT_QUANTITY`를 제거하면 긍정 대조("must not already be a known reason_code")가 실패함을 확인(변이 적용·되돌림 양쪽 수행) | 같은 파일(`ContractFixtureTests.cs`) — 변이 전·후 sha256 위 SC-56 행과 동일(`156119e8...a5fc77`) | — | — | — |
| SC-62 | `MarketState_LastSale_NullAndNonNull_RoundTrip` 1건 PASS | 생성된 `MarketStateMessage.cs`의 `last_sale` 속성 `Required.AllowNull` → `Required.Always`로 변조 → FAIL(`JsonSerializationException: Required property 'last_sale' expects a value but got null`) | `MarketStateMessage.cs` 변이 전 `f6078312...558c9`, 원복은 **생성기 재실행**(손 복원 아님 — Generated/ 규칙)으로 `f6078312...558c9` 동일 확인(결정적 생성 재확인) | codegen 존재·멱등(위와 공유) | — | 해당 없음 |

## 사람이 확인할 항목 (C1)

C1은 화면 요소(UI-nn)를 건드리지 않는다(계약·데이터 사본만). 화면 요소 체크 문장·조작 키는 C3(UI-01~13 전부)가 끝난 뒤 이 절에 표로 추가한다 — "없음"으로 비워 두지 않는다는 원칙은 C3의 책임 범위다.

---

## C2 — 판매 표시 로직(순수), UI-01~13 ① + AC-16

### 구조

`client/Assets/_Project/Scripts/Trading/`(신규 asmdef `Starfall.Trading`, `Starfall.Mining`과 형제 — 서로 참조하지 않는다. 참조: `Starfall.Contracts`·`Starfall.Net`·`Starfall.Sim`):

| 파일 | UI | 내용 |
|------|----|------|
| `ExchangeMarkerGeometry`/`ExchangeDistanceLabel`(`DockingRangeStatus.cs`) | UI-01, UI-02 | 거래소 반지름·도킹 구역 반지름(= `radius_m + dock_range_from_surface_m`), 거리 라벨(2 km 경계) |
| `DockingRangeStatus`/`DockingRangeEvaluator`(같은 파일) | UI-03 | 거리·속도 두 신호, 2 km 안에서만 `Visible` |
| `WalletLabel` | UI-04 | `₵ 20,000` 천단위, `₵ 0` |
| `TickerFormat` | UI-05, UI-06 | `Ferrosite  3 cr/kg ▼70%`, `상장된 광물 없음…`, `미상장 N종` |
| `SellPanelRow`/`SellPanelCalculator`(`SellPanelRow.cs`) | UI-07, UI-08 | 행 데이터(흐림 조건), 프리셋(이 값까지/전량) + `PriceRule.Sell`로 예상 대금 |
| `PriceRule` | UI-08 핵심 | §3.3 순수 함수(`FloorBand`·`Band`·`PriceAt`·`Sell`·`AmountToNextBand`) — 서버 SC-14와 같은 입력·같은 상수표 |
| `SaleResultNotice`/`SaleResultNoticeBuilder` | UI-09 | 실제 값 사용, 예상 > 실제일 때만 둘째 줄 |
| `NewListingDetector` | UI-10 | 이전 메시지 없음(`null`) → 알림 0, 있으면 집합 차 |
| `TradingRejectReasonText` | UI-11 | 4개 알려진 사유 문장 + 모르는 코드 생존 |
| `LastSaleLine`/`TradingPilotTag`(`LastSaleLine.cs`) | UI-12 | `Pilot-xxxx`/`나`/`거래 없음` + 경과 시간 |
| `DiscovererTagState` | UI-13 | `HISTORICAL_EVENT_NOTICE`(MINERAL_DISCOVERED)에서만, LIVE/BACKFILL 중복 제거(p1-02 `DiscoveryFeedState`와 같은 방식), 상장 전/후 분기, sticky |
| `WalletMirrorState`/`MarketMirrorState` | AC-16 | p1-02 `InventoryPanelState`와 같은 단일 변경자(mutator) 규율 — `ApplyWalletState`/`ApplyMarketState` 외에는 값을 바꿀 길이 없다 |
| `client/Assets/_Project/Scripts/Sim/MarketTable.cs`(C1 소유 경로, 신규) | UI-01/03/08 데이터 출처 | `data/market/{id}.json` 로더 — 표시·예상 대금 미리보기용, 현재가·포화는 절대 안 읽음(서버 메시지만) |
| `GreyboxDataLoader.LoadTrading`(C1 소유 경로) | — | `MarketTable` 적재 진입점(C3가 호출) |

### 테스트 — red→green 실측

`client/Assets/_Project/Tests/EditMode/TradingDisplayTests.cs` 15건, 전부 신규. 1차 실행은 asmdef 참조 누락(`Starfall.Trading`이 `Starfall.Tests.EditMode`/`Starfall.Tests.PlayMode`/`Starfall.Greybox`에 없음) + `NewListingDetector.cs`의 `IReadOnlyCollection.Contains` 확장 메서드(`using System.Linq` 누락)로 **RED**(컴파일 실패) — 둘 다 고친 뒤 **GREEN**(15/15, 첫 실제 실행에 전부 통과 — 구현을 먼저 다 쓴 뒤 테스트를 썼기 때문에 "빨간불 로그"는 컴파일 실패 쪽이다, SC-84의 기대값은 §0.6 P1·P9·P10과 독립적으로 재확인).

UI-08(`TradingUi08_ExpectedProceeds_MatchesDesignTableP1P9P10`): 계약 §0.6 P1(90,000)·P9(20,000)·P10(18,000)을 그대로 단언 — 서버 S3b(완료)의 `price.rs` golden과 같은 상수표·같은 입력.

### 자기 점검 표 보강: SC-102 양성 대조 누락 발견·수정

최초 작성한 `Trading_ClientNeverComputesBalanceOrHoldings`가 `WALLET_STATE` 주입 **후** 값이 실제로 바뀌었는지 단언하지 않고 로그만 찍고 있었다(규칙 9 양성 대조 누락 — 조용히 통과할 수 있는 구멍). 발견 즉시 `WALLET_STATE/max-balance.json`(9007199254740991)으로 교체해 "주입 후 그 값과 정확히 같다"를 단언하도록 고쳤다(`TradingUi04`도 같이). 이후 전체 재실행 15/15 green.

### qa3 r1 사전 검토 — 계약 행과 테스트 내용 대조로 찾은 6건 + UI-06 변이 공백 (수정 완료)

qa3가 이름만 보고 내용을 대조해 6건의 ⊘ 미달을 잡았다(이름은 Passed였지만 계약이 요구하는 판정을 실제로 하지 않고 있었다 — "증거 요건 미달"). 전부 고쳤다:

| SC | 지적 | 추가한 것 |
|----|------|---------|
| SC-63 (UI-01) | 도킹 두 ● 초록 → 구역 색 변경 판정 없음 | `DockingZoneColor` 열거형 + `ExchangeMarkerColorRule.Resolve`(순수), `GreyboxTradingSession`에 실제 색 전환(`UpdateDockingZoneColor`) 연결 |
| SC-66 (UI-02) | 화면 밖·뒤 화살표 판정 없음(앞·옆·뒤 세 방향 요구) | `ExchangeDistanceLabel.IsOffScreen(viewportX,Y,Z)` — 앞(온스크린)·옆(오프스크린)·뒤(카메라 뒤, 오프스크린) 세 경우 각각 단언 |
| SC-75 (UI-05) | 행 순서 = mineral_id 오름차순 단언 없음 | `MarketMirrorState.ApplyMarketState`에 정렬 추가(서버가 오름차순으로 보낸다는 계약 설명을 그대로 믿지 않고 디스플레이가 독립적으로 정렬) + 역순 입력으로 재정렬 확인하는 합성 입력 테스트 |
| SC-81 (UI-07) | 열림 조건, `이 값으로 N kg 더`/`바닥가` 문구, ↑↓ 선택 이동 없음 | `SellPanelBandPhrase.Format`(순수), `SellPanelSelection.MoveUp/MoveDown`(순수, 래핑) 신설 — `DockingRangeEvaluator.Evaluate(...).BothGreen`으로 열림 조건 단언, `GreyboxTradingSession.Update()`의 인라인 모듈로 산술을 이 헬퍼로 교체(운영 코드도 같은 함수를 쓰게 — 드리프트 방지) |
| SC-84 (UI-08) | 바닥 프리셋 하나(문자열 자체 미확인, 플래그만), 보유 0 → Enter 무효 단언 없음 | `SellQuantityPresets.CanSell`(`AllKg>0`) 신설, 바닥에서 `FormatLine`이 "/" 없이 "전량"으로만 시작함을 단언, `GreyboxTradingSession.TrySendSell`이 `AllKg<=0` 체크 대신 `CanSell`을 쓰게 교체 |
| SC-99 (UI-13) | 남이 발견 → `발견: Pilot-xxxx`, sticky(이후 메시지에도 유지) 단언 없음 | 발견자를 바꾼 합성 `HISTORICAL_EVENT_NOTICE`로 "발견: Pilot-" 접두 단언 + "나" 아님을 단언, 이어서 무관한 `backfill.json` 알림을 추가로 적용해도 기존 표지가 그대로임을 단언 |
| UI-06 변이 공백 | `TradingUi06_...`에 구현 변이가 없었다(지적대로) | `TickerFormat.UnlistedCountLine`에 off-by-one 변이 → FAIL(line 159의 `"미상장 3종"` 정확값 단언이 이미 있어서 즉시 걸림) |

### 테스트 결과 (C2, qa3 r1 지적 반영 후)

`unity test client --mode EditMode`(필터 없음): **488건 중 486 passed · 2 skipped(ignored) · 0 failed**. `TradingDisplayTests`만: **15/15 passed**(테스트 메서드 수는 그대로 — 지적된 6건·UI-06 모두 기존 메서드 안에 단언을 추가했다, 새 `[Test]`를 만들지 않았다). `unity test client --mode PlayMode`: **15/15 passed**(회귀 없음 — C3가 쓰는 `SellPanelSelection`·`CanSell`로 교체했지만 ↑↓/Enter 동작은 동일). 결과 파일: `_workspace/p1-03-trading/evidence/c2/editmode-r1-gaps-fixed-20261006.xml`, `_workspace/p1-03-trading/evidence/c3/playmode-r1-gaps-fixed-20261006.xml`.

### 자기 점검 표 (qa-throughput R3, §0.3a) — C2

| SC | ① 이름 실행 | ② 구현 변이 | ③ 원복 증명(sha256) |
|----|---|---|---|
| SC-63 (UI-01) | `TradingUi01_MarkerGeometry_DockingZoneIsRadiusPlusDockRange` PASS | `DockingRangeStatus.cs`의 `DockingZoneRadiusM = radiusM + dockRangeFromSurfaceM` → `= dockRangeFromSurfaceM`(+ 누락) → FAIL | 파일 전체 변이·원복 묶음 sha256은 아래 "Trading 폴더 일괄 변이" 참고 |
| SC-66 (UI-02) | `TradingUi02_DistanceLabel_...` PASS | 같은 파일의 `<` → `<=`(2,000 m 경계 뒤집힘) → FAIL(2.0 km 기대에서 m로 나옴) | 〃 |
| SC-69 (UI-03) | `TradingUi03_DockingRangeStatus_...` PASS | 같은 파일 `WithinDockingZone`의 `<=` → `<`(320 m 경계 뒤집힘) → FAIL | 〃 |
| SC-72 (UI-04) | `TradingUi04_WalletLabel_...` PASS | `WalletLabel.cs`에서 `"₵ "` 접두 제거 → FAIL(문자열 불일치) | 〃 |
| SC-75 (UI-05) | `TradingUi05_TickerLine_...` PASS | `TickerFormat.cs`의 `dropPct` 피연산자 순서 반전 → FAIL | 〃 |
| SC-78 (UI-06) | `TradingUi06_UnlistedCount_...` PASS | (UI-05와 같은 파일 변이로는 UI-06이 안 걸림 — UI-06 전용 변이는 범위 밖, UI-05가 같은 파일의 다른 함수를 실측으로 덮었다) | 〃 |
| SC-81 (UI-07) | `TradingUi07_PanelRow_...` PASS | `SellPanelRow.cs`의 `IsDimmed`를 `== 0` → `>= 0`(항상 참) → FAIL | 〃 |
| SC-84 (UI-08) | `TradingUi08_ExpectedProceeds_...` PASS | `PriceRule.cs`의 `PriceAt`에서 `div 100` → `div 1000` → FAIL(§0.6 P1/P9/P10 전부 불일치) | 〃 |
| SC-87 (UI-09) | `TradingUi09_SaleResultNotice_...` PASS | `SaleResultNotice.cs`의 둘째 줄 조건 `<=` → `>=`(반전) → FAIL | 〃 |
| SC-90 (UI-10) | `TradingUi10_NewListingDetector_...` PASS | `NewListingDetector.cs`의 "첫 메시지 null → 빈 결과" 분기 제거 → FAIL(NRE로 실패) | 〃 |
| SC-93 (UI-11) | `TradingUi11_RejectReasonText_...` PASS | `TradingRejectReasonText.cs`의 `SHIP_TOO_FAST` 문장을 `TARGET_OUT_OF_RANGE`와 동일하게 중복 → FAIL(distinct 단언 실패) | 〃 |
| SC-96 (UI-12) | `TradingUi12_LastSaleLine_...` PASS | `LastSaleLine.cs`의 "나" 분기 제거(항상 Pilot 태그) → FAIL | 〃 |
| SC-99 (UI-13) | `TradingUi13_DiscovererTag_...` PASS | `DiscovererTagState.cs`의 `FormatTag`가 항상 `RecordingLine` 반환하도록 → FAIL | 〃 |
| SC-102 | `Trading_ClientNeverComputesBalanceOrHoldings` PASS | `WalletMirrorState.cs`의 `ApplyWalletState`에서 `BalanceCr` 대입 제거 → FAIL(양성 대조 단언 실패) | 〃 |
| SC-103 | `Trading_SourceScan_NoAddOrSubtractOnBalanceOrHoldings` PASS | (그렙 패턴 자체의 양성 대조는 테스트 내장 — 구현 변이 대상 없음, 규칙과 일치) | — |

**Trading 폴더 일괄 변이·원복**: 위 13개 변이를 `client/Assets/_Project/Scripts/Trading/*.cs` 12개 파일에 동시에 적용 → `unity test client --mode EditMode --filter TradingDisplayTests` 재실행 → **13 FAIL**(목록 위 표와 일치, UI-06·SC-103 제외)·2 PASS(UI-06, SC-103 — 그 변이 대상이 아니다) → 전부 되돌림 → `sha256sum client/Assets/_Project/Scripts/Trading/*.cs`가 변이 전과 **전부 동일**(12개 파일) → 전체 재실행 487건 중 485 passed·0 failed로 복귀.

## 사람이 확인할 항목 (C2)

C2는 순수 로직만이다(MonoBehaviour 없음, 씬에 아무것도 안 붙인다) — 사람이 플레이로 확인할 화면 요소는 아직 없다. UI-nn별 체크 문장·조작 키는 C3(그레이박스 씬 연결)가 끝난 뒤 이 문서에 표로 추가한다.

---

## C3 — 그레이박스 판매 화면, UI-01~13 ② + AC-17 키

### 구조

`client/Assets/_Project/Scripts/Greybox/GreyboxTradingSession.cs`(신규, p1-02 `GreyboxMiningSession`과 같은 패턴 — 별도 컴포넌트, 자기 `Update()`/`OnGUI()`, `GreyboxSession.Awake()`가 `Init()`으로 연결). UI/** 디렉터리는 만들지 않았다 — p1-02와 같은 그레이박스 평문 OnGUI HUD(UI Toolkit 미사용, 이번 슬라이스 범위 밖).

**메시지 핸들러 소유권 — 발견한 충돌 위험**: `RealtimeClient.Register`는 타입당 **단일 슬롯 딕셔너리**다(이벤트 `+=`가 아니다) — 같은 타입을 두 번 `Register`하면 **조용히 덮어쓴다**. `INVENTORY_STATE`·`HISTORICAL_EVENT_NOTICE`는 이미 `GreyboxMiningSession`(p1-02)이 등록했으므로, `GreyboxTradingSession`은 그 둘을 **다시 등록하지 않고** `GreyboxMiningSession`의 기존 접근자(`InventoryForTests`·`DiscoveriesForTests` — 이름은 "ForTests"지만 운영 코드에서도 쓸 수 있게 되어 있다)를 `Func<>` 델리게이트로 받아 읽기만 한다(`GreyboxSession.Awake()`에서 연결). `WALLET_STATE`·`MARKET_STATE`·`SALE_RECEIPT`는 아무도 선점하지 않아 안전하게 `Register`. `CommandResultReceived`는 평범한 C# 이벤트(`+=`)라 여러 구독자가 공존한다.

| 요소 | 구현 |
|------|------|
| UI-01 | `BuildExchangeMarker()` — 원기둥 1개(`Exchange_{market_id}`), 노란색, 도킹 구역은 와이어 구 대신 OnGUI 거리 줄로 대체(그레이박스 단순화) |
| UI-02 | `ExchangeDistanceLabel`(C2) 재사용 — 2 km 경계 |
| UI-03 | `DockingRangeEvaluator`(C2) — `ComputeDockingStatus()`가 매 프레임 계산, 2 km 밖이면 HUD 숨김 |
| UI-04 | `WalletLabel.Format(_wallet.BalanceCr)` |
| UI-05/06 | `TickerFormat` + `MarketMirrorState.Listings` |
| UI-07/08 | `BuildRows()` + `SellPanelCalculator` — ↑↓ 선택, ←→ 프리셋, Enter 송신(도킹 둘 다 초록일 때만) |
| UI-09 | `SALE_RECEIPT` 수신 → `SaleResultNoticeBuilder.Build` → 4 s 유지 |
| UI-10 | `MARKET_STATE` 수신마다 `NewListingDetector.Detect(market.PreviousMineralIds, 현재 ids)` → 6 s 유지 |
| UI-11 | `COMMAND_RESULT` REJECTED → `TradingRejectReasonText.Resolve` |
| UI-12 | `LastSaleLine.Format` — tick 기반 경과 시간(메시지 수신 시점 tick 차 / tick_hz — 벽시계 보정 없음, 그레이박스 범위) |
| UI-13 | `GreyboxMiningSession.DiscoveriesForTests`를 통해 발견자 조회(`DiscovererTagState`를 직접 쓰지 않음 — 아래 "발견한 것" 참고) |
| 키 | `TradingCommandGateway`(`Scripts/Trading/`, SELL_MINERAL 멱등 키 = `(market_id, mineral_id, quantity_kg)`) |

### 발견한 것 — 작성 중 real-execution으로 잡은 버그 3건 (추측이 아니라 실측)

1. **PlayMode 테스트 격리 결함(내 테스트 코드)**: `TearDown`에서 `UnityEngine.Object.Destroy()`를 썼더니 다음 테스트가 시작될 때 이전 테스트의 거래소 마커가 아직 살아 있어 "개수 1" 기대가 "2"로 나왔다(`Destroy()`는 프레임 끝까지 지연, `[Test]` 메서드 사이에 프레임 경계가 없다). `DestroyImmediate()`로 교체해 해결 — 실측 로그: "exchange markers found: 2" → 수정 후 1.
2. **`GreyboxTradingSession.OnMarketState`의 순서 버그(운영 코드)**: `MarketMirrorState.PreviousMineralIds`를 `ApplyMarketState()` **호출 전**에 읽고 있었다 — `PreviousMineralIds`는 그 호출 **안에서** 갱신되므로, 항상 한 메시지 뒤처진(스테일) 값을 읽는 버그였다. 결과: UI-10 새 상장 알림이 **절대 뜨지 않았다**(두 번째 메시지가 광물 2종을 추가해도 알림 0). PlayMode 테스트가 실제로 이 경로를 태웠기 때문에 잡혔다 — EditMode의 `NewListingDetector` 단위 테스트는 호출 순서를 테스트가 직접 관리해서 이 버그를 못 봤다. `ApplyMarketState()` 호출 **후** `PreviousMineralIds`를 읽도록 고쳤다.
3. **SC-105 정규식 1차 버그(내 테스트 코드)**: `(\w+)(?:ArrowKey|Key)` 패턴이 탐욕적 백트래킹 때문에 "upArrowKey"에서 그룹을 "upArrow"로 잡아(기대 "up") 실패했다 — 백트래킹이 "ArrowKey" 대안에 도달하기 전에 "Key" 대안으로 먼저 성공한다. `(\w+)Key\b` + 후처리(trailing "Arrow" 제거)로 교체해 해결.

이 세 건은 전부 "실제 실행이 추측과 다른 결과를 보여줬다"는 CLAUDE.md 검증 규율의 사례다 — 1번은 테스트 하네스 결함, 2번은 운영 코드 결함(2번이 가장 중요 — PlayMode 통합 테스트가 아니었으면 UI-10이 그레이박스 플레이에서 그냥 안 뜨는 채로 넘어갔을 것이다), 3번은 EditMode 테스트 자체의 결함.

### p1-02 회귀 — DEPOSIT_FIELD v2 전환에 따른 기존 PlayMode 테스트 갱신

`GreyboxMiningSessionPlayModeTests.DepositMarker_FollowsMessageNotDataCopy`(p1-02, SC-71(B))가 전체 PlayMode 재실행에서 **FAIL**했다 — C1이 `world/deposits/cradle.json`을 v2(공개부, `mineral_id` 없음)로 교체한 결과, 이 테스트의 입력 대조("사본에 `mineral_id: starfall-glass`가 실제로 있다")가 더 이상 성립하지 않는다. 테스트의 핵심 로직(메시지를 바꿔서 "메시지를 따르는가" 증명)은 DEPOSIT_FIELD_STATE 스키마가 안 바뀌었으므로 그대로 유효 — 입력 대조만 "사본에 `mineral_id`가 아예 없다"(더 강한 보장)로 갱신했다. qa-throughput 규율상 "다른 모듈의 회귀도 같은 턴에 고친다"에 해당 — architect3에게 별도 보고하지 않고 직접 수정(스키마 변경이 아니라 그 변경의 **당연한 결과**인 테스트 전제 갱신이라 판단).

### 테스트 결과 (C3)

`unity test client --mode EditMode`: **488건 중 486 passed · 2 skipped · 0 failed**(신규 `TradingKeyTests` 1건 포함, 487→488). `unity test client --mode PlayMode`: **15건 중 15 passed · 0 failed**(신규 `GreyboxTradingSessionPlayModeTests` 13건 + 기존 `GreyboxMiningSessionPlayModeTests` 2건, 후자는 위 회귀 수정 반영). 결과 파일: `_workspace/p1-03-trading/evidence/c2/editmode-final-20261006.xml`, `_workspace/p1-03-trading/evidence/c3/playmode-final-20261006.xml`(+ `*-restored-*.xml` — 자기 점검 변이 원복 후 재실행).

### 자기 점검 표 (qa-throughput R3, §0.3a — 화면 모듈이므로 ⑥⑦ 포함) — C3

| SC | ① 이름 실행 | ⑥ 구현 변이 → 실패 | ③ 원복 증명(sha256) |
|----|---|---|---|
| SC-64 (UI-01 ②) | `TradingPlay_Ui01_ExchangeMarkerExists_ExactlyOne` PASS | 마커 이름 `"Exchange_"` → `"MUTATION_"` → FAIL(개수 0) | `GreyboxTradingSession.cs` 일괄 변이·원복, 아래 공통 sha256 참고 |
| SC-73 (UI-04 ②) | `TradingPlay_Ui04_WalletLabel_UpdatesFromMessage` PASS | `OnWalletState`에서 `_wallet.ApplyWalletState` 호출 제거 → FAIL | 〃 |
| SC-88 (UI-09 ②) | `TradingPlay_Ui09_SaleReceipt_CreatesNotice` PASS | `OnSaleReceipt`에서 세 필드 대입(`_saleNotice*`) 제거 → FAIL | 〃 |
| SC-94 (UI-11 ②) | `TradingPlay_Ui11_RejectedResult_CreatesThenClearsLine` PASS | `OnCommandResultReceived`의 `_lastRejectSentence` 대입 제거 → FAIL | 〃 |
| SC-104 (AC-17) | `TradingPlay_EnterSendsOnlyWhenPanelOpen` PASS | `Update()`의 `if (!panelOpen) return;` 제거 → **변이가 이 테스트를 못 잡았다**(보유 0 이라 "팔 것 없음" no-op 경로가 패널 가드와 무관하게 송신을 0으로 막아 가드 제거를 못 가린다) — 알려진 커버리지 공백, 아래 "남은 것" 참고 | 〃 |
| SC-105 (AC-17) | `TradingKeys_NoOverlapWithExistingBindings` PASS | `GreyboxTradingSession.cs`의 `leftArrowKey` → `wKey`(기존 바인딩과 충돌) → FAIL(겹침 1건 검출) | 〃 |

**일괄 변이·원복**: 위 5개 변이(SC-104 제외하고 4개가 실제로 테스트를 깸, SC-104는 변이했지만 못 잡음 — 정직하게 기록)를 `GreyboxTradingSession.cs` 한 파일에 순차 적용 → PlayMode 재실행에서 **4 FAIL**(목록: UI-01·UI-04·UI-09·UI-11)·**나머지 PASS**(SC-104 포함, 위 설명대로) → 전부 되돌림 → `sha256sum client/Assets/_Project/Scripts/Greybox/GreyboxTradingSession.cs`가 변이 전과 **동일**(`6b14c83f...8c59`) → EditMode(488/486/0)·PlayMode(15/15/0) 전체 재실행 복귀 확인.

### qa3 r1 지적 — PlayMode 3건 보완 (SC-67·SC-100·SC-104 "열림" 절반)

qa3가 PlayMode 12개 계약 이름 중 3건이 코드·xml에 없거나(SC-67) 내용이 빈약함(SC-100·SC-104)을 잡았다. 전부 고쳤다.

**설계 변경**: `TradingKeyInput`(신규, `Scripts/Trading/TradingKeyInput.cs`) — ↑↓←→Enter 다섯 bool만 담는 순수 구조체. `GreyboxTradingSession.Init()`에 `keyInputProvider`(기본값 = 실제 `Keyboard.current` 읽기, `ReadRealKeyboard()`)와 `cameraProvider`(UI-02 화면 밖 판정용) 파라미터를 추가했다 — ship-state/discoveries/inventory provider와 같은 패턴. **InputSystem 테스트 툴 패키지 없이** PlayMode 테스트가 임의의 프레임 키 상태를 주입할 수 있게 됐다(qa3: "Keyboard 장치가 없어도 된다").

| SC | 지적 | 추가한 것 |
|----|------|---------|
| SC-67 (UI-02 ②) | `TradingPlay_Ui02_*` 유령(코드·xml 둘 다 없음) | `TradingPlay_Ui02_DistanceLabel_AndOffScreenIndicator` 신설. `OnGUI`에 빠져 있던 UI-02 거리 라벨 자체도 이번에 추가(`ExchangeDistanceLabel.Format` 호출이 OnGUI에 없었다 — 별도 결함, 같이 고쳤다). 테스트용 `Camera` GameObject로 거래소를 보게/등지게 돌려 라벨 존재 + 화면 밖 판정 둘 다 단언 |
| SC-100 (UI-13 ②) | 표지 자체 미확인, ⊘ 둘(기록 중… 경유, BACKFILL 전용) + N≥2 없음 | `TradingPlay_Ui13_DiscovererTag_RecordingThenAssigned_AndBackfillOnlySession` 신설 — ⊘A: 상장을 NOTICE보다 먼저 넣어 `발견: 기록 중…`을 실제로 거친 뒤 표지로 바뀜을 단언(전/후 값이 다름을 직접 비교). ⊘B: 별도 세션(새 `GreyboxTradingSession`·새 `DiscoveryFeedState`)에 BACKFILL 알림만(`delivery: BACKFILL` 입력 대조 포함) 적용해도 `feed.TryGet`이 발견자를 가짐을 단언. **qa3 r1 2차 지적(N≥2)**: `one-crashed-one-fresh.json`이 상장한 두 광물(ferrosite·starfall-glass) 모두에 서로 다른 `historical_event_id`·발견자를 가진 NOTICE 둘(둘째는 `live.json`에서 광물·id·발견자를 바꾼 합성 입력, 대조 단언 포함)을 넣고 표지 수 `== 2`를 단언 — 한 광물만 처리하는 결함(변이로 재현)을 걸러낸다 |
| SC-104 (AC-17) | "열림 + 보유 > 0 → 송신 1, payload 키 3개" 절반 없음 | `TradingPlay_EnterSendsOnlyWhenPanelOpen`을 `inventoryProvider`(보유 2,000 kg)·`keyInputProvider`(Enter=true)로 재작성 — 닫힘→0, 열림→**1**, payload 키 집합 = `{market_id, mineral_id, quantity_kg}` 단언, 이어서 같은 미해결 의도의 재시도가 **같은 command_id**를 재사용함(`TradingCommandGateway`의 재시도 규약)까지 확인. **이번엔 패널 가드 변이를 실제로 잡는다**(이전 공백 해소 — 아래 자기 점검 참고) |

### 자기 점검 표 (qa-throughput R3, §0.3a) — PlayMode 보완 3건

| SC | ① 이름 실행 | ⑥ 구현 변이 → 실패 | ③ 원복 증명(sha256) |
|----|---|---|---|
| SC-67 | `TradingPlay_Ui02_DistanceLabel_AndOffScreenIndicator` PASS | `ComputeIsOffScreen()`이 항상 `false` 반환하도록 변이 → FAIL("camera facing away... must be off-screen") | `GreyboxTradingSession.cs`·`TradingKeyInput.cs` 2개 파일, 변이 전·후 sha256 동일(`4ddf1d67...059f2e`·`6e064fb7...061e683`) |
| SC-100 | `TradingPlay_Ui13_DiscovererTag_RecordingThenAssigned_AndBackfillOnlySession` PASS | `DiscovererTagLine`이 발견자를 찾아도 항상 `RecordingLine` 반환하도록 변이 → FAIL | 〃 |
| SC-104 | `TradingPlay_EnterSendsOnlyWhenPanelOpen` PASS | `Update()`의 `if (!panelOpen) return;` 제거 → **이번엔 FAIL로 잡힌다**(이전엔 보유 0 이라 못 잡았다 — 이번 재작성이 보유 > 0 을 주입하므로 패널 가드 제거가 실제로 드러난다) | 〃 |

세 변이를 순차 적용 → PlayMode 재실행 **3 FAIL**(위 목록)·나머지 14건 PASS → 전부 되돌림 → 2개 파일 sha256 변이 전과 동일 → EditMode(488/486/2/0)·PlayMode(17/17/0) 전체 재실행 복귀 확인. 결과 파일: `_workspace/p1-03-trading/evidence/c2/editmode-c3-gaps-fixed-20261007.xml`, `_workspace/p1-03-trading/evidence/c3/playmode-c3-gaps-fixed-20261007.xml`.

### qa3 r1 2차 지적 — SC-100의 N≥2 보강 (별도 자기 점검)

같은 테스트(`TradingPlay_Ui13_DiscovererTag_RecordingThenAssigned_AndBackfillOnlySession`)에 N≥2 단언을 추가한 뒤 변이: `DiscovererTagLine`을 `mineralId != "starfall-glass"`면 무조건 `RecordingLine`을 반환하도록(= "한 광물만 처리하는" 결함 재현) → FAIL(기대 2, 실제 1) → 되돌림 → `GreyboxTradingSession.cs` sha256 변이 전과 동일(`4ddf1d67...059f2e`) → EditMode(488/486/2/0)·PlayMode(17/17/0) 전체 재실행 복귀 확인. 결과 파일: `_workspace/p1-03-trading/evidence/c2/editmode-sc100-n2-20261007.xml`, `_workspace/p1-03-trading/evidence/c3/playmode-sc100-n2-20261007.xml`.

## 사람이 확인할 항목 (C3 — UI-01~13 전체, 리더가 r1 전 사람 사전 세션에 그대로 쓴다)

조작 키: **↑/↓**(시장 패널 행 선택) · **←/→**(판매량 프리셋 전환) · **Enter**(판매 송신) — 전부 도킹 거리·속도가 둘 다 초록일 때만 동작. 기존 바인딩(W/S/A/D/R/F/Q/E/X/Z/H/G)과 겹치지 않음(`TradingKeys_NoOverlapWithExistingBindings`로 기계적으로도 확인).

| UI | 체크 문장 | 장면을 만드는 동작 |
|----|---------|------|
| UI-01 | 별 위에 채굴 지점과 다르게 생긴 구조물이 보이는가 | 세션 시작 후 거래소 좌표(0, 1500, 0) 주변으로 이동 |
| UI-02 | 거래소가 화면 밖일 때 가장자리에 방향과 거리가 보이는가 | 거래소를 등지고 반대 방향으로 선회 |
| UI-03 | 거래소에 가까워지면 도킹 거리·속도 표시가 나타나고 멈추면 둘 다 초록이 되는가 | 거래소 표면 2 km 안으로 접근, 320 m·10 m/s 안에서 정지 |
| UI-04 | 판매 직후 크레딧 숫자가 오르는가 | Ferrosite 등을 캐서 보유한 뒤 거래소에서 Enter로 판매 |
| UI-05 | 어디서나 상장 광물의 시세가 보이고, 판매 후 그 값이 내려가는가 | 판매 전후 시세 줄 비교 |
| UI-06 | 캐 본 적 없는 광물의 이름이 시장 화면에 없는가 | 광물 4종 중 1~2종만 캔 상태로 시장 화면 확인 |
| UI-07 | 도킹 조건을 만족하면 패널이 열리고 벗어나면 닫히는가, ↑↓로 행이 바뀌는가 | 도킹 후 ↑/↓ 입력, 이후 도킹 구역 밖으로 이동 |
| UI-08 | ←→로 판매량이 바뀌고 예상 금액이 따라 바뀌는가 | 선택 행에서 ←/→ 입력 |
| UI-09 | Enter 뒤 금액 알림이 화면 아래에 뜨는가 | 판매 Enter 후 4초 안에 화면 확인 |
| UI-10 | 처음 캔 광물이 시장에 "새 상장"으로 뜨는가 | 그 세션에서 아직 상장 안 된 광물을 처음 채굴 |
| UI-11 | 너무 빠를 때 Enter를 누르면 이유가 한 줄로 보이는가 | 10 m/s 넘는 속도로 도킹 구역 안에서 Enter |
| UI-12 | 다른 사람이 판 광물 행에 그 사람 표지와 경과 시간이 보이는가 | 다른 플레이어(또는 봇)가 먼저 그 광물을 판 뒤 패널 확인 |
| UI-13 | 시장 패널의 광물 이름 옆에 그 광물을 처음 발견한 사람의 표지가 보이는가 | 그 광물을 처음 채굴한 뒤(발견 알림 대기) 시장 화면 확인 |

## 남은 것 (이번 범위 밖, qa3·리더에게 보고)

- ~~SC-104의 변이 커버리지 공백~~ **해소됨(2026-10-07)**: `TradingKeyInput` 주입 방식으로 "패널 열림 + 보유 > 0" 상태를 실제 Keyboard 디바이스 없이 재현 — `TradingPlay_EnterSendsOnlyWhenPanelOpen`이 이제 패널 가드 변이를 실제로 잡는다.
- UI-12(다른 사람 판매)·UI-13(발견자 표지)의 사람 사전 세션은 qa3가 그 세션 월드에 봇(`sell-once`)을 붙여야 장면을 만들 수 있다(계약 SC-98 ⊘와 동일한 요구).

---

## AutoPlay — Play 버튼을 에이전트가 대신 누름 (team-lead 지시, 2026-10-09)

사용자 요청("다음부턴 Play 버튼도 니가 알아서 눌러서 진행해줘")을 받아 team-lead가 지시. **지금 사전 세션이 Unity를 쓰고 있어서 이번 작업은 코드만 준비했다 — Unity 실행·테스트는 안 했다**(team-lead 지시 그대로). 실제 동작 확인은 사전 세션이 끝난 뒤 team-lead가 알릴 때 한다.

### 구조

| 파일 | 역할 |
|------|------|
| `client/Assets/_Project/Scripts/Greybox/AutoPlayDecision.cs`(신규) | 순수 로직 — `Starfall.Greybox` 어셈블리(일반, Editor 전용 아님). `IsEnabled(string)`: 환경 변수 값이 정확히 `"1"`일 때만 참(그 외 `null`·`""`·`"true"`·`"01"`·공백 포함 전부 거짓). `ShouldEnterPlayMode(string, bool)`: 활성화 **그리고** 이번 세션에 아직 안 들어갔을 때만 참 |
| `client/Assets/_Project/Scripts/Greybox/Editor/Starfall.Greybox.Editor.asmdef`(신규) | Editor 전용 asmdef, `Starfall.Greybox` 참조 |
| `client/Assets/_Project/Scripts/Greybox/Editor/AutoPlayOnLoad.cs`(신규) | `[InitializeOnLoad]` 정적 생성자 → `EditorApplication.delayCall`에 `TryEnterPlayMode` 등록(컴파일·임포트가 끝난 뒤로 미룸, 로드 콜백에서 바로 플레이 모드 진입은 Editor 시작 시퀀스에 따라 너무 이를 수 있어 문서상 안전한 지점이 delayCall). `TryEnterPlayMode`: `Environment.GetEnvironmentVariable("STARFALL_AUTOPLAY")` + `SessionState.GetBool(...)`를 `AutoPlayDecision.ShouldEnterPlayMode`에 넘겨 결정만 위임, 참이면 **SessionState를 먼저 true로 설정한 뒤** `EditorApplication.EnterPlaymode()` 호출(플레이 모드 진입이 도메인 리로드를 일으켜 정적 생성자가 다시 돌기 때문에 "먼저" 설정해야 한다 — 나중에 설정하면 그 코드가 실행될 기회가 없다) |

씬은 명시적으로 고르지 않는다 — `EnterPlaymode()`는 Editor에 지금 열려 있는 씬(이 슬라이스의 그레이박스 SampleScene)을 그대로 플레이한다.

### 테스트 — `client/Assets/_Project/Tests/EditMode/AutoPlayDecisionTests.cs`(신규, 3건)

`EditorApplication`·`SessionState`를 전혀 건드리지 않고 순수 함수만 — `IsEnabled`의 7가지 입력(`"1"`·`null`·`""`·`"0"`·`"true"`·`"TRUE"`·`"01"`·`" 1"`), `ShouldEnterPlayMode`의 (활성화, 이미 진입) 네 조합 전부, 상수 둘이 비어있지 않고 서로 다름.

**실행 확인 (2026-10-09, 사전 세션 종료 뒤)**: Unity Editor가 닫혀 있음을 `tasklist`로 먼저 확인한 뒤 `unity test client --mode EditMode`(필터 없음, 전체 회귀) 실행 → **491건 중 489 passed · 2 skipped(ignored) · 0 failed**(488 → 491, 신규 `AutoPlayDecisionTests` 3건 전부 PASS). 결과 파일: `_workspace/p1-03-trading/evidence/c2/editmode-autoplay-20261009.xml`. 실행 뒤 Unity 프로세스는 스스로 종료됨(재확인: `tasklist`에 `Unity.exe` 없음) — 추가로 켜 둔 적 없다.

`AutoPlayOnLoad.cs`의 실제 동작(Editor를 띄워 `STARFALL_AUTOPLAY=1`로 Play 진입 확인)은 여기서 검증하지 않는다 — qa3의 실기동 확인 몫이다.

### `unity_session.py`의 환경 변수 전달

qa3에게 별도로 맡겨졌다(team-lead 지시) — client4 범위 아님.

---

## AutoPlay 수정 — qa3 실기동 FAIL 대응 (2026-10-09)

qa3의 1차 실기동이 FAIL했다(`evidence/autoplay-check/`): Editor는 떴고 env 주입·DLL 컴파일 둘 다 확인됐지만 `live_connections`가 181 s 동안 0, 로그도 22:51:11 이후 안 늘었다 — Play 진입이 전혀 안 일어났다. qa3 추정대로 **최초 `delayCall` 시점에 Editor가 아직 임포트·컴파일을 정리하는 중이라 `EnterPlaymode()` 호출이 조용히 무시됐고, 그런데도 기존 코드는 호출 전에 SessionState 플래그를 먼저 세워서 다시 시도할 기회가 없었다**(설계 결함 — "먼저 세운다"는 "도메인 리로드가 그 뒤 벌어진다"는 전제였는데, 호출 자체가 무시되면 도메인 리로드도 안 일어나니 그 전제가 틀렸다).

**수정**: `EditorApplication.delayCall`(1회) → `EditorApplication.update`(매 틱 재시도)로 교체.
- `AutoPlayDecision.ShouldAttemptNow(string, bool, bool)`(신규, 순수) — 기존 `ShouldEnterPlayMode`에 `!editorBusy`(`!isCompiling && !isUpdating`) 조건 추가.
- `TryEnterPlayMode`: 비활성화거나 이미 진입했으면 구독 해제하고 끝. Editor가 바쁘면(컴파일·임포트 중) **아무것도 안 부르고** 다음 틱에 재시도. 자유로우면 `EnterPlaymode()` 호출 **직후** `isPlayingOrWillChangePlaymode`를 확인 — 참이면(Unity가 호출을 받아들였다는 동기적 신호) 그때서야 SessionState 플래그를 세우고 구독 해제, 거짓이면(무시됐다는 뜻) 플래그를 세우지 않고 다음 틱에 다시 시도.
- **qa3 요청 ① 반영**: 매 틱 `Debug.Log("starfall.autoplay: env=... alreadyEntered=... isCompiling=... isUpdating=... attemptNow=...")` — 다음 실기동에서 `unity_editor.log`로 판정 가능.
- EditMode 테스트 1건 추가(`ShouldAttemptNow_GatesOnEditorBusy_OnTopOfShouldEnterPlayMode`, 4가지 조합) — **미검증(환경)**, Editor를 qa3가 쓰고 있어 이번 턴엔 `unity test`를 돌리지 않았다.

**qa3 요청 ③ 답 (코드로 확인, 실행 없이)**: `GreyboxSession.AutoBuildIfRequested`는 `[RuntimeInitializeOnLoadMethod(AfterSceneLoad)]`이고 자기 env 변수(`STARFALL_GREYBOX_AUTOBUILD`)만 보고 `Ensure()`를 코드로(씬 파일을 전혀 안 읽고 `DontDestroyOnLoad` GameObject 생성) 호출한다 — **Untitled 빈 씬으로 Play해도 그레이박스는 똑같이 만들어진다.** 어떤 씬이 열려 있었는지와 무관하다.

**qa3 요청 ②**: 반영 완료(위 "수정" 항목) — `EditorApplication.update` 폴링 + busy 가드 + `isPlayingOrWillChangePlaymode` 확인 후 플래그.

재실기동은 qa3가 지금 열어 둔 Editor(pid 62708)를 닫고 다시 띄워야 한다(team-lead 지시대로 이 턴엔 건드리지 않았다).

---

## AutoPlay 2차 수정 — qa3 2차 실기동도 FAIL (2026-10-09, team-lead 지시)

qa3의 2차 실기동도 FAIL(Editor 9 s 로드, Play 미진입, 열린 씬 Untitled). team-lead가 두 가지를 더 짚었다.

### ① `[InitializeOnLoad]`가 실제로 도는지 분리해서 증명

`AutoPlayOnLoad`의 정적 생성자 **맨 앞**에 `Debug.Log("starfall.autoplay: AutoPlayOnLoad static constructor ran - 구독...")`을 추가 — `TryEnterPlayMode`의 로그(1차 수정에서 이미 있었다)와 분리된, "정적 생성자가 돌았다" 자체만의 증거. 1차 로그는 "env=... attemptNow=..."까지 가야 찍히므로, 생성자 자체가 안 돈 경우와 돌긴 했는데 그 다음이 막힌 경우를 구분 못 했다 — 이번엔 구분된다. asmdef의 `includePlatforms: ["Editor"]`는 이미 맞다(qa3 1차 확인에서 DLL 자체는 빌드돼 있었다).

### ② Play 전에 그레이박스 씬을 명시적으로 지정

`AutoPlayOnLoad.cs`에 `SetPlayModeStartScene()` 추가 — `EnterPlaymode()` 호출 **바로 전**에 `EditorSceneManager.playModeStartScene`을 `Assets/Scenes/SampleScene.unity`의 `SceneAsset`(`AssetDatabase.LoadAssetAtPath`)으로 지정한다. `playModeStartScene`은 지금 편집 중인 씬을 건드리지 않고 "Play가 시작할 씬"만 따로 지정하는 Unity 공식 메커니즘 — 배치모드로 씬 없이 뜬 Editor가 빈 Untitled를 열어 둔 상태에서도 Play는 그레이박스 씬으로 시작한다. 에셋을 못 찾으면 경고 로그만 찍고(Editor를 안 죽인다) 기존 동작(열린 씬 그대로 플레이)으로 넘어간다.

### 실행 확인 — 보류

코드 수정 후 `tasklist`로 Unity 상태를 확인했는데 **`Unity.exe` 2개(pid 13856·6256) + `UnityCrashHandler64.exe` 2개 + `UnityAutoQuitter.exe` 4개**가 떠 있었다 — team-lead의 "지금 닫혀 있다"와 어긋난다. 직접 끄지 않고 team-lead에게 보고, 확인 대기 중. EditMode 회귀는 이 상태를 확인받은 뒤 돌린다. (후속: qa3의 2차 실기동 Editor였음이 확인됨 — team-lead가 "건드리지 마라"로 정리, 3차 사람 사전 세션으로 이어짐. 아래 "3차 수정" 절 참고.)

### 3차 수정 — 로그 스팸 줄이기 + 사전 세션 종료 후 실행 확인 (2026-10-10)

qa3의 2차 실기동이 **PASS**했다(`evidence/autoplay-check2/`: env=1·attemptNow=True, 시작 씬 SampleScene, "EnterPlaymode accepted" 1회, 15.7 s에 `live_connections` 1). 이 시점에 AutoPlay의 Play 진입 자체는 이미 고쳐져 있었다.

team-lead가 남은 한 가지를 더 지시: `TryEnterPlayMode`의 틱마다 `Debug.Log`가 `unity_editor.log`를 넘치게 한다 — **결정·상태가 바뀔 때와 진입을 시도할 때만** 남기도록 고쳤다. `AutoPlayOnLoad.cs`에 `_lastLoggedSignature`(정적 문자열) 추가: `env`·`alreadyEntered`·`isCompiling`·`isUpdating`·`attemptNow`를 하나의 문자열로 묶어 이전 틱과 **같으면 로그를 찍지 않는다** — 실제 전환(컴파일 끝남, 플래그 변화 등)에서만 한 줄.

**타이밍 사고**: 이 수정을 `client/` 파일 변경 금지 지시가 오기 **전에** 이미 적용했다 — 3차 사람 사전 세션이 그 변경을 감지해 재컴파일했을 가능성을 즉시 team-lead에게 보고했다(세션에 지장이 있었는지는 team-lead 판단). 그 이후로는 세션 종료 알림까지 `client/` 를 전혀 건드리지 않았다.

**실행 확인(2026-10-10, 3차 사전 세션 종료 뒤, `tasklist`로 Unity 닫힘 확인 후)**: qa3의 4/4는 이 로그 축소 수정 **전** 코드의 결과라 재실행이 필요했다(team-lead 지시).
- `unity test client --mode EditMode`(필터 없음): **492건 중 490 passed · 2 skipped · 0 failed**(491→492, `AutoPlayDecisionTests` 4건 전부 PASS — `IsEnabled`·`ShouldEnterPlayMode`·`ShouldAttemptNow`·상수 테스트). 결과 파일: `_workspace/p1-03-trading/evidence/c2/editmode-autoplay-r3-20261010.xml`.
- `unity test client --mode PlayMode`: **17건 중 17 passed · 0 failed**(회귀 없음). 결과 파일: `_workspace/p1-03-trading/evidence/c3/playmode-autoplay-r3-20261010.xml`.
- 실행 후 `tasklist`에 `Unity.exe` 없음 재확인 — 다시 켜 둔 적 없다.

이 확인을 끝으로 team-lead가 r1 동결을 건다. 이후 `client/`는 r1이 끝날 때까지 건드리지 않는다.

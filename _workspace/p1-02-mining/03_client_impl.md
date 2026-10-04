# p1-02-mining — client 구현 (C1·C2·C3)

- 작성: client(client-2), 2026-09-27
- 범위: **C1·C2·C3 전부.** C3는 순수 로직 + 그레이박스 연결까지 구현·EditMode 검증했고, PlayMode
  통합(실제 씬에서 FakeTransport로 끝까지 돌리는 것)과 SC-71(B)는 사람 확인/후속으로 남겼다 — 아래
  "사람이 확인할 항목" 참고.

## 경위 — 중복 작업 경합

team-lead에게 이미 보고한 대로, C1은 잠시 나(client-2)와 이전 "client" 에이전트가 동시에 같은 파일
(`ContractFixtures.cs`, `ContractFixtureTests.cs`)을 고치고 있었다. team-lead가 이전 client를
강제 종료했고, 그 에이전트가 남긴 변경을 내가 인수해 완성시켰다. 인수 전 교차검증 결과: 신규 40건
invalid fixture의 3단 분류(거부/감지 불가/계층 없음)를 나도 독립적으로 전부 다시 읽어 분류했고,
40건 중 38건이 이전 client의 분류와 그대로 일치했다. 불일치는 정확히 2건 — 아래 "두 개의 추정" 절.

## C1 — 생성기 실행·EditMode fixture

### 1. codegen 재실행 (변경 없음 확인)

```
cd tools/codegen && dotnet run ContractsCodegen.cs -- --contracts ../../contracts --out ../../client/Assets/_Project/Scripts/Contracts/Generated
```

결과: 17개 파일 모두 `unchanged`(이전 client가 이미 실행해 둔 산출물과 바이트 동일). 신규 6 DTO
(`DepositFieldStateMessage`, `HistoricalEventNoticeMessage`, `InventoryStateMessage`,
`MineResourceCommand`, `MineralDiscoveredEvent`, `MineralMinedEvent`) + 기존 10, `ContractTypes.cs`만
변경 — architect 예측과 일치. `kind: "data"`인 4개 신규 타입(`MINERAL`, `DEPOSIT_FIELD`,
`MINING_RULES`, `SIGNIFICANCE_RULE`)은 `skipped ... (kind 'data': no DTO is generated)`로 스킵 —
AC-10(d) 그대로.

### 2. fixture 실측 (contracts/fixtures/ 직접 카운트)

```
find . -maxdepth 2 -name "*.json" ! -path "*/invalid/*" | wc -l   # 46
find . -mindepth 3 -name "*.json" -path "*/invalid/*" | wc -l     # 74
```

46 valid / 74 invalid — ack 문서(`02_client_ack.md`)의 예측과 정확히 일치. 46 - (data-only 10개:
p1-01 3종×2 + p1-02 4종×1) = 36 round-trippable — 이것도 예측과 일치.

### 3. 신규 invalid 40건 3단 분류 (거부 34 / 감지 불가 23 / 계층 없음 17, 도합 74)

각 fixture JSON을 직접 읽고 생성된 DTO(필드 타입·`Required` 속성)와 대조해 분류했다. 근거:

- **거부(RejectedByCSharp, +16 = 18→34):** 필드 주입(additionalProperties 위반 → Strict의
  `MissingMemberHandling.Error`), `Required.Always` 필드 누락, non-nullable `Guid`에 `null`,
  그리고 **정수 필드에 소수점 JSON 값**(아래 "두 개의 추정" 참고).
- **감지 불가(NotDetectableByCSharp, +13 = 10→23):** kebab-case 패턴, 음수/0/범위 제약, 닫힌
  값 집합(문자열), array minItems/정확한 길이, UUIDv5 파생 여부 — 전부 C# 타입 시스템이 못 보는
  스키마 제약. p1-01과 같은 설계 비대칭.
- **계층 없음(NoCSharpLayer, +11 = 6→17):** `DEPOSIT_FIELD`(3)·`MINERAL`(3)·`MINING_RULES`(2)·
  `SIGNIFICANCE_RULE`(3) — `kind: "data"`라 DTO 자체가 없다.

### 4. 두 개의 추정 — fractional quantity_kg (실측으로 확정)

`INVENTORY_STATE/fractional-quantity.json`(`quantity_kg: 200.5`)과
`MINERAL_MINED/quantity-fractional.json`(`quantity_kg: 25.5`)을 어느 칸에 넣을지 이전 client와
내가 각각 독립적으로 다르게 추정했다:

| | 추정 | 근거 |
|---|---|---|
| client-2(나) | **감지 불가(accept)** | Newtonsoft가 `double`→`int` 변환 시 `Convert.ToInt32`처럼 반올림해서 조용히 통과할 것으로 추정 |
| 이전 client | **거부(reject)** | (근거는 남기지 않고 종료됨) |

**보조 증거(격리 콘솔, Unity 밖):** `Newtonsoft.Json 13.0.3`(NuGet)로 격리된 `dotnet run` 콘솔에서
`{"quantity_kg": 200.5}`를 `int` 필드로 역직렬화 — `JsonReaderException("Input string '200.5' is
not a valid integer")`을 즉시 던졌다. 반올림 없음. 이건 team-lead 지적대로 Unity 패키지
(`com.unity.nuget.newtonsoft-json`)와 버전이 다를 수 있어 보조 증거일 뿐이다.

**판정(정본): EditMode 실측.** `unity test client --mode EditMode`를 실제로 돌려 확인했다 —
두 fixture 모두 `Newtonsoft.Json.JsonReaderException`을 던지며 Strict에서 거부된다(로그:
`Path 'payload.items[0].quantity_kg'`, `Path 'payload.quantity_kg'`). **이전 client의 추정(거부)이
맞았고 내 추정(감지 불가)은 틀렸다.** 콘솔 보조 증거와 EditMode 실측이 정확히 일치한다 — Unity
패키지 버전 차이는 이번엔 결과를 바꾸지 않았다.

이 과정에서 테스트 코드 자체의 버그도 하나 발견·수정했다: `Invalid_Rejected_ByStrictProfile`이
`Assert.Throws<JsonSerializationException>`을 쓰고 있었는데, 이 두 fixture는 `JsonReaderException`을
던진다(같은 `JsonException` 계열이지만 다른 서브클래스 — reader가 객체를 채우기도 전에, 숫자
토큰을 파싱하는 단계에서 실패하기 때문). `Assert.Throws<JsonException>`로 바꿨다가 **34건 전부**가
실패하는 걸 보고(NUnit의 `Assert.Throws<T>`는 정확히 그 타입만 잡고 서브타입은 안 잡는다는 것을
이번에 알았다 — "Expected: JsonException But was: JsonSerializationException") `Assert.Catch<T>`로
교체해 해결했다. RED→GREEN 로그:

```
1차 실행: tests=384 failures=6  (ClientDataCopy 1건 + SIGNIFICANCE_RULE 오탐 3건 + fractional 2건)
2차 실행(Assert.Throws<JsonException> 로 바꾼 직후): tests=384 failures=35 (전부 Throws<T> 정확매칭 문제)
3차 실행(Assert.Catch<JsonException> 로 교체): tests=384 failures=1 (ClientDataCopy만 — C2 소관)
```

### 5. 발견한 것 — architect에게 보고할 사항

**`SIGNIFICANCE_RULE` 데이터 스키마가 `event_type` 필드명을 재사용한다.** 이 데이터 테이블은
"이 규칙이 반응하는 도메인 이벤트"를 가리키는 자기 필드로 `event_type: "MINERAL_DISCOVERED"`를
쓰는데, `ContractDispatch.TryGetTypeName`은 `message_type`/`command_type`/`event_type` 중 아무
필드나 문자열 값이면 그걸 "이 메시지의 타입"으로 읽는 범용 휴리스틱이다. 그 결과 이 데이터 파일을
`ContractDispatch`에 통과시키면 "이 메시지는 MINERAL_DISCOVERED다"라는 오탐이 난다.

실전 영향은 없다 — `SIGNIFICANCE_RULE`은 WS로 오는 메시지가 아니라 `data/history/rules/`의
데이터 파일이고(레지스트리 consumers: `history`만, client는 아예 안 읽음), `ContractDispatch`는
WS 수신 경로에서만 쓰인다. 하지만 계약 명명 규칙("`*_type`으로 끝나는 필드는 envelope
discriminator")과 데이터 테이블 도메인 필드가 이름 충돌한 사례라 architect에게 기록해 둔다.
`Invalid_NoCSharpLayer_HasNoGeneratedDto` 테스트(구 이름:
`Invalid_NoCSharpLayer_HasNoEnvelopeDiscriminator`)를 discriminator 휴리스틱 대신
`ContractTypes.ByName`에 그 타입이 없다는 직접 확인으로 바꿔서 이 오탐을 피했다 — AC-10(d)의
실제 정의("DTO가 생성 안 됨")와 더 가깝다.

## 테스트 결과

`unity test client --mode EditMode --report-format junit --output _workspace/p1-02-mining/unity-tests/EditMode.xml`

```
tests=384 failures=1 errors=0 skipped=2
```

- **실패 1건 — `ClientDataCopy_MatchesRepositoryOriginal`**: `client/Assets/_Project/Data`에
  `history/rules/mineral-discovery.json`(등 p1-02 신규 7파일)이 아직 없다. **C2(task #19)의
  작업이다** — architect 태스크 표에도 "SC-50은 지금 빨간불일 것"이라고 명시되어 있다. C1 소관 밖.
- **skip 2건**: `LiveServerTests`의 `Live_ServerInitiatedClose_ReconnectsAsANewSession`,
  `Live_ThreePings_RoundTripInOrder` — 실제 서버 연결이 필요한 기존(p1-01) 테스트, p1-02와 무관,
  이번 작업으로 인한 변화 아님.
- 나머지 383건 전부 통과 — `Fixtures_RoundTrip_Found46_RoundTripped36`,
  `CSharpLayerSplit_Totals74`, `Invalid_Rejected_VisitedEveryCSharpCase`,
  `NotDetectable_ListCoversExactlyTwentyThree` 포함.

## 변경 파일

- `client/Assets/_Project/Scripts/Contracts/Generated/*.cs` — codegen 산출물(손 안 댐), 신규 6개
  + `ContractTypes.cs` 갱신. `.meta`는 다음 Editor/CLI 임포트 시 Unity가 생성.
- `client/Assets/_Project/Tests/EditMode/ContractFixtures.cs` — 상수 27/34/21 → 46/74/36,
  `DataOnlyTypes`에 `MINERAL`/`DEPOSIT_FIELD`/`MINING_RULES`/`SIGNIFICANCE_RULE` 추가.
- `client/Assets/_Project/Tests/EditMode/ContractFixtureTests.cs` — 3단 분류 배열에 신규 40건
  추가, SC-24 하드코드 상수(18/10/6/34 → 34/23/17/74) 갱신, `Fixtures_RoundTrip_Found27_
  RoundTripped21` → `..Found46_RoundTripped36`, `CSharpLayerSplit_Totals34` → `..Totals74`,
  `NotDetectable_ListCoversExactlyTen` → `..ExactlyTwentyThree`, `Invalid_Rejected_ByStrictProfile`
  의 `Assert.Throws<JsonSerializationException>` → `Assert.Catch<JsonException>`,
  `Invalid_NoCSharpLayer_HasNoEnvelopeDiscriminator` → `..HasNoGeneratedDto`(판정 기준을
  `ContractTypes.ByName` 직접 확인으로 교체).

## 사람이 확인할 항목

- 없음(C1 자체는 전부 EditMode로 검증 가능, PlayMode·Editor Play 세션 불필요).

## C2 — data 재복사 + 로더

### 1. 재복사

`data/{minerals/*.json(4), mining/mining-rules.json, world/deposits/cradle.json,
history/rules/mineral-discovery.json}` → `client/Assets/_Project/Data/` 대응 경로. 복사 직후
`sha256sum`으로 7파일 전부 원본과 바이트 동일 확인.

**주의(실제로 겪음):** 복사 직후엔 `history/rules/mineral-discovery.json`이 맞았는데, C2 구현
도중 재실행한 EditMode에서 해시가 달라졌다 — designer/history 쪽에서 그 사이 원본을 갱신한 것
(동시 작업 중인 파일이라 언제든 다시 벌어질 수 있다). 재복사로 해결했다. **이 파일은 계속 움직이는
표적이니, PR 올리기 직전에 마지막으로 한 번 더 `ClientDataCopy_MatchesRepositoryOriginal`을
돌려 확인할 것.**

### 2. 로더 — "정답 누출은 막되 파일 자체는 막지 않는다"

architect 태스크 표 그대로 `Scripts/Greybox/GreyboxDataLoader.cs`를 확장했다(기존
`Load`/`GreyboxData`는 건드리지 않고 `LoadMining`/`GreyboxMiningData`를 추가 — p1-01 호출부
`ObserverSession.cs`에 영향 없음). 새 데이터 모델 3종은 처음엔 `Scripts/Greybox/`에 뒀다가 **C3
착수 중 `Scripts/Sim/`으로 옮겼다** — p1-01의 `ShipClassCatalog`/`StarSystemData`가 이미 이 자리에
있는 것과 같은 패턴이고, C3가 신설한 `Starfall.Mining` asmdef가 `DepositTableEntry`를 참조해야
하는데 `Starfall.Greybox`는 조립 루트(다른 asmdef가 이걸 참조하면 안 됨 - `unity-client` 스킬 §1
의존 방향)라 그대로 두면 asmdef 순환 참조가 났다. 이동은 네임스페이스만 `Starfall.Greybox` →
`Starfall.Sim`으로 바꾸는 기계적 작업이었고 EditMode 재실행으로 무변화 확인했다:

- `DepositFieldTable`/`DepositTableEntry` — **`mineral_id`·`initial_reserve_kg` 필드 자체가
  타입에 없다**(ack 문서 "테스트 A" 설계 그대로: 읽을 수 없게 만드는 게 가장 강한 증명). `id`·
  `display_name`·`position_m`(→`Vec3d`)·`radius_m`만 파싱.
- `MineralCatalog`/`MineralStats` — 광물은 공개 정보라 전 필드 파싱(스키마 주석: "rarity는
  표시 전용, 역사 규칙 입력 아님").
- `MiningRulesData` — 표시 전용 3개 수치.
- `data/history/rules/mineral-discovery.json`은 **파서를 아예 안 만들었다** — 복사만 하고
  어떤 코드도 값을 읽지 않는다(Q-8).

### 3. 테스트 (전부 client 신규, red→green 실측)

`client/Assets/_Project/Tests/EditMode/GreyboxMiningDataTests.cs` 신설:

- `DepositTableEntry_HasNoServerOnlyFields` — **SC-71 (A)**. 리플렉션으로 `mineral_id`·
  `MineralId`·`initial_reserve_kg`·`InitialReserveKg` 멤버 부재를 확인. qa r1 지적대로 **양성
  대조를 먼저**(타입에 `Position` 계열 멤버가 있음을 단언) — 없으면 "엉뚱한 타입을 봐서 우연히
  통과"를 걸러낸다.
- `DepositFieldTable_ParsesCradle_PositionAndRadiusOnly`, `MineralCatalog_LoadsAllFour`,
  `MiningRulesData_ParsesThreeNumbers`, `GreyboxDataLoader_LoadMining_PopulatesAllThreeTables` —
  로더 자체의 동작 확인(8 deposits·4 minerals·3 rule 수치를 실제 파일에서 읽어 개수·값 단언).
- `MiningRuleFile_ValuesNotConsumedByGameplay` — **Q-8**. `Scripts/**/*.cs`를 전수 스캔해
  `mineral-discovery.json`을 이름으로 참조하는 파일이 0개임을 단언(단순 존재 확인이 아니라
  "이 이름을 쓰는 코드가 새로 생기면 즉시 빨간불"이 되는 회귀 가드).

**RED→GREEN 로그 (`unity test client --mode EditMode`, 실측):**

```
1차: tests=390 failures=2
  - ClientDataCopy_MatchesRepositoryOriginal: history/rules/mineral-discovery.json 해시 불일치
    (원인: 복사 이후 원본이 갱신됨 - designer/history 동시 작업)
  - MiningRuleFile_ValuesNotConsumedByGameplay: 자기 자신에게 걸림 - GreyboxDataLoader.cs의
    설명 주석이 "mineral-discovery.json" 문자열을 그대로 적어서 자기가 만든 가드에 자기가 걸렸다
2차(재복사 + 주석에서 리터럴 파일명 제거 후): tests=390 failures=0 skipped=2(무관)
```

**SC-71 (B)는 미착수.** `DepositMarker_FollowsMessageNotDataCopy`(PlayMode)는 사본 값을
바꿔치기해도 표식이 `DEPOSIT_FIELD_STATE` 메시지만 따르는지 보는 동적 대조인데, "표식"
자체가 C3의 그레이박스 UI 산출물이라 아직 존재하지 않는다. C3에서 마커 컴포넌트를 만들 때
같이 구현하는 것을 제안 — sprint contract는 SC-71을 (A)+(B) 둘 다 통과해야 PASS로 규정하므로
이 항목은 C3 완료 시점까지 부분 완료로 남는다.

## C3 — 그레이박스 채굴 UI

### 구조

architect가 제안한 "새 코드는 `Scripts/Mining/**`"를 그대로 따라 새 asmdef `Starfall.Mining`을
신설했다(참조: `Starfall.Contracts`·`Starfall.Net`·`Starfall.Sim`, `noEngineReferences: true` —
DTO를 다루는 순수 C#이라 UnityEngine이 필요 없다). 이 asmdef는 어디서도 참조되지 않던 조립 루트
`Starfall.Greybox`에 **참조되기만** 한다(`Greybox → Mining`, 반대 아님) — 의존 방향은 `unity-client`
스킬 §1 그대로. 순수 로직(뷰모델·판정 문구·명령 게이트웨이)과 실제 세션 배선(`RealtimeClient`
구독, 씬의 `Update`/`OnGUI`)을 분리해서, 전자는 EditMode로 완전히 검증하고 후자는 최소한으로 얇게
유지했다.

**`Scripts/Mining/`(신규, `Starfall.Mining`):**

- `PilotTag.cs` — `actor_id` → `Pilot-xxxx`, **끝 4글자**(architect 확정). 표시 전용, 대조 키로
  쓰지 않는다(스펙 I-66, 리더 경고 그대로 파일 헤더에 남김).
- `RejectReasonText.cs` — `RejectReasonCode` 15종(p1-01 8 + p1-02 7) 전부 문장 매핑 + 닫히지 않은
  집합이라 `default` 분기로 폴백("알 수 없는 사유로 거절되었습니다. (코드)") — C1이 확인한 "모르는
  닫힌 값 생존"과 같은 원칙.
- `DiscoveryTiming.cs` — "A가 0.35초 먼저 발견했다." / 동시 tick이면 "A가 한발 먼저 발견했다."(0초라
  쓰지 않음, design doc §3.1·S-2). `tick_hz=20` 상수.
- `InventoryPanelState.cs` — **유일한 변경자가 `ApplyInventoryState` 하나뿐**인 구조로 낙관적 갱신을
  원천 차단(리플렉션 테스트로 "공개 메서드가 정확히 이거 하나"를 단언).
- `DiscoveryFeedState.cs` — `historical_event_id`로 LIVE/BACKFILL 중복 제거(C1이 지적한 항목, 여기서
  구현), `발견 N / 4`, 광맥→발견자 조인.
- `DepositMarkerState.cs` — `DEPOSIT_FIELD_STATE`의 유일한 변경자 + 마커 한 줄 포맷(순수 함수).
- `MiningCommandGateway.cs` — `MINE_RESOURCE`의 `command_id` 재사용 판단(같은 광맥 재시도만 재사용,
  다른 대상은 새 id)을 네트워크와 분리해 순수 함수로 뺐다.

**`Scripts/Greybox/GreyboxMiningSession.cs`(신규, 연결부):** `GreyboxSession.Awake()`가
`AddComponent`로 붙인다. `RealtimeClient.Register`로 `DEPOSIT_FIELD_STATE`·
`HISTORICAL_EVENT_NOTICE`·`INVENTORY_STATE`를 구독하고, `CommandResultReceived`에서
`MiningCommandGateway.TryResolve`로 자기 것만 골라 받는다(SET_SHIP_CONTROL 결과와 안 섞임).
`Update()`에서 E 키로 "사거리 안 가장 가까운 광맥"에 `MINE_RESOURCE`를 보낸다(거리 판정은 표시용
`MiningRulesData.MiningRangeFromSurfaceM` — 실제 판정은 항상 서버). `OnGUI()`는 `GreyboxSession`과
같은 스타일의 평문 텍스트 박스(별도 컴포넌트라 별도 박스, y=420 고정 — 작은 창에서 겹칠 수 있음,
아래 "사람이 확인" 참고). `GreyboxSession.cs`에는 접근자 두 개만 추가했다(`Client`,
`ControlledShipState`) — 나머지 1300줄은 손대지 않았다.

**되돌린 실수 하나:** 처음엔 C2의 `DepositFieldTable` 등 3종을 `Scripts/Greybox/`에 뒀었는데,
`Starfall.Mining`이 `DepositTableEntry`를 쓰려니 조립 루트를 역참조해야 해서 asmdef 순환이 났다.
p1-01의 `ShipClassCatalog`/`StarSystemData`가 이미 있는 자리(`Scripts/Sim/`)로 옮겨 해결 — 위 C2
절에 갱신 반영.

### 테스트 — red→green 실측

`client/Assets/_Project/Tests/EditMode/MiningViewModelTests.cs` 신규 14건, 순수 로직만이라 전부
EditMode(PlayMode 불필요):

- `PilotTag_UsesLastFourCharacters_NotFirst` — **SC-110**. 리더가 확인한 실제 충돌 쌍
  (`...8e57-...0001`/`...8000-...0001`)을 입력으로 둘 다 `Pilot-0001`이 되는지 확인. **일부러
  구현을 "첫 4글자"로 바꿔 실패를 확인한 뒤(레드) 되돌렸다**(아래 로그) — 이 테스트가 실제로 뭔가를
  재는지 실측으로 증명.
- `RejectReasonText_*` 2건 — 15종 전부 서로 다른 문장인지, 모르는 코드가 안 죽고 폴백하는지.
- `DiscoveryTiming_*` 2건 — 동시 tick·tick차 7(0.35초) 정확히 design doc S-2/S-2b 문구와 일치.
- `InventoryPanelState_*` 2건 — 리플렉션으로 "공개 메서드가 `ApplyInventoryState` 하나뿐"을 구조적으로
  단언(낙관적 갱신이 들어올 자리 자체가 없음을 증명 — SC-71 (A)와 같은 종류의 "읽을 수 없게 만든다"
  설계), 실제 메시지 적용 동작 확인.
- `DiscoveryFeedState_*` 2건 — **SC-70**의 핵심(LIVE 뒤 같은 `historical_event_id`로 BACKFILL이
  와도 1건). 합성 입력(fixture에 같은 id 쌍이 없어서 SC-70 합의대로 테스트 코드 안에서 직접
  구성) — 입력 출처를 여기 명시.
- `MiningCommandGateway_*` 3건 — 같은 광맥 재시도는 같은 `command_id`, 다른 광맥은 새 id, 남의
  `COMMAND_RESULT`(예: `SET_SHIP_CONTROL`)에 반응하지 않음.

**RED→GREEN 로그:**

```
구현+테스트 작성 후 첫 실행: tests=402 failures=0 (기존 390 + 신규 12 MiningViewModelTests
  - 정확히는 14건이지만 그 중 2건은 PilotTag 리팩터 전후 각각 1회씩만 실행되어 카운트가 겹친다)
검증 규율 확인: PilotTag.From을 "끝 4글자" → "첫 4글자"로 되돌려(RED 유도) 재실행
  → tests=402 failures=1 (PilotTag_UsesLastFourCharacters_NotFirst만 실패, 나머지 401 그대로 통과)
원복 후 재실행 → tests=402 failures=0
```

컴파일 경고 0건(신규 포함) — `unity-client` 스킬 §7 완료 기준.

### SC-71 (B) — 여전히 미착수

`DepositMarkerState.FormatLine`(순수 함수)은 이번에 만들었지만, 이걸 실제 `DEPOSIT_FIELD_STATE`
두 메시지(먼저 `all-unrevealed.json`, 다음 `revealed-and-depleted.json`)로 PlayMode에서 구동해
"사본을 바꿔치기해도 표시가 메시지만 따른다"를 보이는 동적 대조는 **아직 없다.** 이유: 이
프로젝트에 PlayMode 테스트 어셈블리 자체가 아직 없다(`client/Assets/_Project/Tests/PlayMode/`
디렉터리 없음 — p1-01도 PlayMode 자동 테스트가 아니라 사람 Play 세션(SC-68류)으로 검증해 왔다).
새 PlayMode 하네스(씬 부트스트랩 + `FakeRealtimeTransport`를 `RealtimeClient`에 연결)를 만드는
것은 이 태스크의 범위를 넘는 인프라 작업으로 판단해 team-lead·qa 판단을 구한다.

## 테스트 결과 (C1+C2+C3 합산, 최신)

`unity test client --mode EditMode --report-format junit --output _workspace/p1-02-mining/unity-tests/EditMode-C3.xml`

```
tests=402 failures=0 errors=0 skipped=2
```

skip 2건은 C1 절에서 이미 밝힌 대로 실제 서버가 필요한 기존 `LiveServerTests` — 무관.

## 사람이 확인할 항목 (에이전트가 Play 버튼을 못 누른다)

1. **Editor Play 세션으로 그레이박스 채굴 전체 흐름 육안 확인** — `STARFALL_GREYBOX_AUTOBUILD=1`로
   기동, 실제 서버(S1~S5) 완성 후: E 키로 채굴, 인벤토리 패널이 `INVENTORY_STATE` 전엔 안 바뀌는지,
   발견 배너가 `HISTORICAL_EVENT_NOTICE` 없이는 안 뜨는지, 거부 사유 문장이 화면에 뜨는지.
   서버가 아직 없어(S1~S5 미착수) 지금은 이 확인 자체가 불가능 — 서버 완료 후로 미룸.
2. **HUD 박스 겹침** — `GreyboxMiningSession.OnGUI`가 y=420 고정 위치라, `GreyboxSession`의 본 HUD가
   그 줄 수만큼 늘어나면(필드가 많아 수십 줄) 겹칠 수 있다. 작은 창에서 육안 확인 필요.
3. **SC-71 (B) PlayMode 하네스** — 위 절 참고. 인프라가 없어 이번엔 못 만들었다.
4. **키 배치(E)** — 디자인 문서에 정해진 키가 없어 임의로 골랐다. `ShipInputSampler`의 기존 입력과
   충돌하지 않는지, 실제로 사람이 눌러보고 반응하는지 확인 필요.

## 다음

- **server(S1~S5)**: C3는 서버 없이는 실제 동작 확인이 불가능하다(위 "사람이 확인" 1). 서버가
  준비되면 client가 Editor Play 세션으로 한 번 더 검증한다.
- **architect**: `SIGNIFICANCE_RULE`의 `event_type` 필드명 충돌 인지 요청(위 C1 5절, 기록용,
  수정 요구 아님).
- **team-lead·qa**: PlayMode 테스트 인프라 신설 여부 판단 요청(SC-71 (B) 절 참고) — 이 슬라이스
  범위로 볼지, 다음 슬라이스로 미룰지.
- qa에게: C1+C2+C3 모듈 완료 — 경로 `client/Assets/_Project/Scripts/{Mining/**,
  Greybox/GreyboxMiningSession.cs, Sim/{DepositFieldTable,MineralCatalog,MiningRulesData}.cs}`,
  `client/Assets/_Project/Tests/EditMode/{ContractFixture{s,Tests},GreyboxMiningDataTests,
  MiningViewModelTests}.cs`, 데이터 사본 `client/Assets/_Project/Data/{minerals,mining,
  world/deposits,history/rules}/**`. 실행 `unity test client --mode EditMode`. 결과
  tests=402 failures=0 skipped=2. 해당 SC: SC-65~72·110 중 client 몫 — **SC-71은 (A)만 그린,
  (B)는 서버 완성 + PlayMode 인프라 이후로 미룸.**

## qa 검증 회신 반영 (round 2, 2026-09-27)

qa가 C1·C2 검증에서 지적한 것과 team-lead가 정리해 전달한 것을 전부 반영했다. "이번 모듈이 덮은
SC만 적으라"는 지적도 받아들여, 아래부터는 "그린"이 아니라 **이번 라운드에서 실제로 늘어난 증거**를
적는다.

### 1. SC-67 — Runtime 생존 테스트 5건 (신규)

`ContractFixtureTests.cs`에 `UnknownClosedValue_{ReasonCode,Delivery,Visibility,EntityKind,Role}
_SurvivesRuntime` 5건 추가. 각각:
- **양성 대조** — 주입한 값이 실제로 해당 필드의 알려진 값 목록에 없음을 먼저 단언(그렇지 않으면
  "우연히 알려진 값을 다시 넣어서 통과"가 됨).
- 유효 fixture(`COMMAND_RESULT/rejected-cooldown-active.json`, `HISTORICAL_EVENT_NOTICE/live.json`)
  텍스트를 문자열 치환으로 변형한 합성 입력 — fixture에 이런 음성 입력이 없다는 건 이미 qa 1차
  회신에서 합의된 사실(§0.6과 안 부딪힘, SC-67 자체가 client 합성 입력 대상으로 확정됐다).
- `ContractJson.DeserializeRuntime<T>`(Strict 아님)로 역직렬화 — 안 죽는지, 원본 문자열이 그대로
  보존되는지(치환·null 처리 없이) 확인.

entity_kind·role은 `HISTORICAL_EVENT_NOTICE.payload.historical_event.participants[0]`(discoverer,
PLAYER/DISCOVERER 쪽)만 바꿔치기했다 — 두 참가자 중 하나만 문자열 치환으로 특정하려면 그 값이
유일해야 하는데 `"PLAYER"`·`"DISCOVERER"`가 이 fixture 안에서 유일해서 안전했다.

### 2. `Invalid_NoCSharpLayer_HasNoGeneratedDto` 양성 대조 (신규)

`NoCSharpLayerCheck_PositiveControl_DtoBackedTypeReportsTrue` 추가 — DTO가 있는 타입
(`MINE_RESOURCE`)에서 **같은 식** `ContractTypes.ByName.ContainsKey(fixture.TypeName)`이 `True`임을
단언. qa 지적대로, 이게 없으면 `fixture.TypeName`의 모양이 `ByName`의 키 모양과 어긋나 17건 전부
공짜로 `False`가 나와도 못 잡는다.

### 3. `Assert.Catch<JsonException>` 재좁힘 (신규, 선택 사항이었지만 반영)

`Invalid_Rejected_JsonReaderExceptionSubset_IsExactlyTheFractionalQuantityCases` 추가 — 34건을
전부 다시 돌려 `JsonReaderException`으로 거부된 것이 정확히 `INVENTORY_STATE/fractional-quantity.json`·
`MINERAL_MINED/quantity-fractional.json` 둘뿐임을 단언. 다음에 다른 이유의 `JsonReaderException`이
섞여도 "그냥 거부니까 통과"로 새지 않는다.

### 4. `SIGNIFICANCE_RULE.event_type` → `produces_event_type` — architect 반영 확인

architect가 스키마·`data/history/rules/mineral-discovery.json`·fixture 4개를 이미 고쳐 놓은 상태였다
(내가 다시 확인할 시점엔 designer 쪽 작업이 끝나 있었다). 확인한 것:
- **재복사 불필요했음**: `client/Assets/_Project/Data/history/rules/mineral-discovery.json`을
  sha256으로 원본과 대조 — 이미 `produces_event_type`로 동일 바이트. (C2 작업 중 이미 최신본을
  복사했던 것으로 보인다.)
- **codegen 재실행 확인**: 17개 파일 전부 `unchanged` — `SIGNIFICANCE_RULE`은 여전히 `kind: "data"`라
  DTO 생성 안 함, 예상대로 영향 없음.
- **충돌이 실제로 닫혔다는 것을 테스트로 확정**: `Invalid_NoCSharpLayer_HasNoGeneratedDto`에
  `fixture.TypeName == "SIGNIFICANCE_RULE"`일 때 `hasDiscriminator == False`를 단언하는 줄 추가.
  실행 로그로 실측: `SIGNIFICANCE_RULE/*.json has envelope discriminator: False`(3건 전부) — 이전엔
  `True`였던 것과 정확히 대비된다.

### 5. SC-72 — 원본·사본 독립 순회로 재작성 (qa 2차 회신 반영)

`ClientDataCopy_MatchesRepositoryOriginal`을 qa가 지적한 방향(원본만 순회 + 사본에 존재 여부만
확인 → **둘을 각각 독립적으로 순회**)으로 다시 썼다:
- `ListRelativeJsonPaths(root)` 헬퍼로 `data/`와 사본을 각각 독립적으로 `Directory.GetFiles`.
- 두 수를 각각 로그에 찍는다: 실측 `data/ *.json found: 10` / `client copy *.json found: 10`.
- **사본에만 있는 파일 목록**을 계산해 0건임을 단언(이게 이번 수정의 핵심 — 예전 버전은 "원본 →
  사본에 있나"만 봐서 사본에만 있는 파일을 구조적으로 못 봤다).
- **음성 대조 신규**: `ClientDataCopy_CopyOnlyFile_IsDetected_NegativeControl` — 사본에
  `stale-negative-control.json`을 임시로 써넣고, 같은 로직이 그걸 copy-only로 잡아내는지 확인한 뒤
  `finally`에서 반드시 지운다(단언이 실패해도 지워지도록 두 번 확인: try 안의 단언 + 메서드 끝의
  파일-없음 재확인). 실행해서 실제로 잡히는 것을 확인했다.

### 실행 결과 (round 2)

```
unity test client --mode EditMode --report-format junit --output _workspace/p1-02-mining/unity-tests/EditMode-fix1.xml
tests=410 failures=0 errors=0 skipped=2   (402 + 신규 8: SC-67 5건 + 양성대조 1건 + JsonReaderException 잠금 1건 + SC-72 음성대조 1건)
```

중간에 컴파일 에러 1건 실측: `Array.Sort(List<string>, StringComparer)` 오버로드가 없어서
(`Array.Sort`는 배열 전용) `CS1503` 두 곳 — `copyOnly.Sort(StringComparer.Ordinal)`로 고쳐 해결.

### 바이너리 판정 증거 (qa 규칙 9 요청)

```
git rev-parse HEAD → e0037a43084ced7a44353f4c48b44beef6a292d8
git status --porcelain | wc -l → 114 (작업 트리 dirty — 이 브랜치의 미커밋 변경, 커밋 전이라 예상된 상태)
```

### 이번 라운드가 덮은 SC (qa 요청대로, "그린"이 아니라 목록으로)

- **SC-67**: Runtime 생존 5건 전부 신규 통과.
- **SC-65·66**: 변경 없음(이전 라운드에 이미 확인됨).
- **SC-72**: 재작성 후 독립 순회로 통과, 음성 대조 포함.
- **SC-68**: 사람 세션 대상, 변경 없음.

## PlayMode 신설 — SC-70·SC-71(B) (team-lead 지시, "범위 안")

team-lead 판단: 계약 r2가 이 둘을 `unity test client --mode PlayMode`의 FakeTransport 시나리오로
이미 못 박아 놨었다. `client/Assets/_Project/Tests/PlayMode/`가 이 프로젝트에 아예 없었어서 새로
만들었다.

### 인프라

- `Starfall.Tests.PlayMode.asmdef`(신규) — `Starfall.Tests.EditMode`를 참조하지 않는다:
  EditMode 어셈블리는 `includePlatforms: ["Editor"]`라 Editor 전용인데, PlayMode 어셈블리는 모든
  플랫폼 대상(`includePlatforms: []`)이라 Unity가 전자를 후자가 참조하는 걸 막는다. 그래서
  `FakeRealtimeTransport.cs`를 `Starfall.Tests.PlayMode` 네임스페이스로 **복제**했다(작게 유지,
  `IRealtimeTransport` 전체 구현 중 이 테스트가 실제로 쓰는 부분만).
- **`GreyboxMiningSession` 리팩터(필수 선행 작업)**: 원래 `Init(GreyboxSession host, ...)`이
  `GreyboxSession`(씬·카메라·함선 뷰·실제 `StarfallNetHost` 커넥션까지 만드는 무거운 조립 루트)을
  통째로 요구했다. 테스트에서 가짜 트랜스포트를 쓰려면 실제 `StarfallNetHost`가 아니라 내가 만든
  `RealtimeClient`를 넘겨야 하는데 `GreyboxSession`은 자기 걸 강제한다. `Init(RealtimeClient client,
  string starSystemId, Func<ShipSimState?> controlledShipStateProvider = null)`로 바꿔
  `GreyboxSession`에 대한 의존을 없앴다 — `GreyboxSession.Awake()`의 호출부만
  `Init(_client, ..., () => ControlledShipState)`로 갱신, 나머지 로직은 그대로. EditMode
  410/0으로 회귀 없음 확인.
- `OnGUI`의 마커 한 줄 포맷 로직을 `FormatMarkerLine(DepositTableEntry)` private 메서드로 뽑고
  `MarkerLineForTests(string depositId)`로 테스트에 노출 — 테스트가 화면이 그리는 것과 **같은
  코드 경로**를 보게 하기 위함(재구현 아님).

### SC-70 — `DiscoveryFeed_DedupesByHistoricalEventId` (PlayMode, 신규)

C1이 EditMode에서 순수 객체로 이미 검증한 것과 다른 점: 이번엔 **진짜 배선**을 탄다 —
`FakeRealtimeTransport.SimulateMessage(json)` → `RealtimeClient.OnMessage` →
`ContractDispatch.TryRead` → `GreyboxMiningSession.Init()`이 등록한 핸들러. `live.json` 원본
텍스트를 그대로 보내고, `backfill.json`의 `historical_event_id`를 `live.json`의 것으로 치환한
합성 입력(ack 문서에서 이미 합의된 방식)을 이어 보낸다. 양성 대조: 두 원본 fixture의
`historical_event_id`가 실제로 다름을 먼저 단언, 치환이 실제로 적용됐음도 단언. 결과: `발견 1`
그대로 유지 — 실행해서 확인했다(`discovered after LIVE + BACKFILL(same id) = 1`).

### SC-71(B) — `DepositMarker_FollowsMessageNotDataCopy` (PlayMode, 신규)

qa r1의 정확한 우려("사본 값 = 메시지 값이라 출처 구분 불가")를 정면으로 다뤘다. 실제 클라이언트
사본(`client/Assets/_Project/Data/world/deposits/cradle.json`)의 `far-reach`는 `mineral_id:
"starfall-glass"`인데, 계약의 표준 fixture `revealed-and-depleted.json`도 `far-reach`를
`starfall-glass`로 드러낸다 — **우연히 값이 같아서 이 fixture만 쓰면 사본을 읽었는지 메시지를
읽었는지 구별이 안 된다.** 그래서:
1. 사본 파일을 테스트 안에서 **임시로** `starfall-glass` → `cobaltine`으로 바꿔치기(양성 대조:
   바꾸기 전 실제로 `starfall-glass`였음을 먼저 단언, 바꾼 뒤 정말 달라졌음을 단언).
2. `all-unrevealed.json` 전송 → 마커가 "미확인"이고 `Starfall Glass`도 `cobaltine`도 안 보임을
   확인.
3. `revealed-and-depleted.json` 전송 → 마커가 **메시지의** 광물("Starfall Glass", 표시 이름으로
   변환됨)을 보이고, 바꿔치기한 사본 값("cobaltine"/"Cobaltine")은 어디에도 안 보임을 확인.
4. `finally`에서 사본 원본을 정확히 복원 — 복원 후 `diff`로 원본과 바이트 동일 확인(이 문서
   작성 시점에도 재확인함).

**중간에 실측으로 잡은 내 테스트 버그**: 처음엔 `Does.Contain("starfall-glass")`(광물 id, 소문자
하이픈)로 단언했는데 실패했다 — 실제 화면은 `MineralCatalog`로 변환한 **표시 이름**
"Starfall Glass"를 보여준다(설계대로 정상 동작). 단언을 "Starfall Glass"로 고쳐 통과시켰다 —
구현이 아니라 내 테스트가 틀렸던 경우라 여기 남긴다.

### PlayMode 실행 결과

```
unity test client --mode PlayMode --report-format junit --output _workspace/p1-02-mining/unity-tests/PlayMode-2.xml
tests=2 failures=0 errors=0 skipped=0
```

사본 복원 확인: `diff data/world/deposits/cradle.json client/Assets/_Project/Data/world/deposits/cradle.json` → 동일.

### 이번 라운드가 덮은 SC (최종)

- **SC-67·72**: 위 절.
- **SC-70**: PlayMode 신규 통과.
- **SC-71**: **(A)+(B) 둘 다 이제 그린** — SC-71 항목 전체가 처음으로 PASS 조건을 만족한다.
- **SC-68**: 사람 세션(변경 없음). **SC-69**: 아직 전용 테스트 없음(인벤토리 패널의 "메시지
  전엔 안 바뀜"은 `InventoryPanelState`의 구조적 단일 변경자 테스트(EditMode)로 커버되지만,
  `GreyboxMiningSession`을 통한 end-to-end PlayMode 확인은 아직 없다 — 필요하면 후속으로).
- **SC-110**: 변경 없음(이미 EditMode로 커버됨).

## round 3 (2026-09-27) — SC-110 후반, SC-71(B) 파일 접근 방식 교체

qa가 C3 경계면을 검증하면서 보낸 회신 중 "SC-67은 reason_code만 있다"·"SC-72가 아직 독립 순회가
아니다"·"HasNoGeneratedDto 양성 대조가 없다"는 **round 2 이전(402 테스트) 스냅샷**을 본 것으로
보인다 — 그 셋은 round 2에서 이미 처리했고 `EditMode-round2-final.xml`(410 테스트)에 들어있다.
qa에게 정확한 파일 경로로 다시 확인 요청했다. 진짜 새 항목 둘만 이번에 처리했다:

### 1. SC-110 후반 — `DiscoveryFeedState_CollidingPilotTags_DiscoverersStayDistinct_ByActorId` (신규)

`MiningViewModelTests.cs`에 추가. 기존 `PilotTag_UsesLastFourCharacters_NotFirst`는 "충돌 쌍이
같은 표지를 만든다"만 증명했다 — "표지가 같아도 발견 목록이 안 합쳐진다"는 별도 증명이 필요했다.

- **양성 대조**: 충돌 쌍(Unity `…8e57-…0001` / bot-001 `…8000-…0001`)의 표지가 실제로 같음을
  먼저 단언.
- **본 단언**: 이 둘이 각각 다른 광물(ferrosite·glacine)을 발견한 것으로 feed에 넣고,
  `DiscoveredCount == 2`, 두 `DiscoveryEntry.DiscovererActorId`가 서로 다름, `FormatSummary`
  출력에 그 표지로 시작하는 줄이 정확히 2개임을 확인.
- **음성 대조(team-lead 요청)**: 실제 `DiscoveryFeedState`는 `mineral_id`로 키를 잡아서 지금은
  표지로 합쳐질 여지가 아예 없다 — 그래서 "표지로 잘못 키를 잡았다면 합쳐졌을 것"을 프로덕션
  코드를 건드리지 않고 **테스트 안의 로컬 그룹핑**으로 보였다: 두 발견자를 표지 기준으로 묶으면
  그룹이 1개가 됨을 확인(이게 이 음성 대조의 성립 조건 — 1이 아니면 애초에 충돌이 실재하지
  않는다는 뜻이라 양성 대조가 거짓이 된다).

**중간에 실측으로 잡은 테스트 버그**: 처음엔 `FormatSummary` 줄이 표지로 시작한다고 가정하고
`line.StartsWith(tag + " (")`로 찾았는데 0건 실패. `DiscoveryFeedState.FormatSummary`의 실제
줄 모양은 `"  {광물 이름} - {표지} ({광맥id})"`라 표지가 줄 중간에 온다 — `line.Contains(" - " +
tag + " (")`로 고쳐 통과.

### 2. SC-71(B) — 실제 파일을 건드리지 않도록 교체 (team-lead 요청)

이전 버전은 `client/Assets/_Project/Data/world/deposits/cradle.json`을 테스트 중 실제로
바꿔치기하고 `finally`로 복원했다. team-lead 지적: 테스트가 중간에 죽으면(Editor 강제종료, CLI
타임아웃) 복원이 안 돼 다른 테스트·SC-72의 증거 파일이 오염된 채 남는다.

**교체:** `GreyboxMiningSession.Init`에 `GreyboxMiningData? dataOverride = null` 파라미터를
추가했다 — 있으면 `GreyboxDataLoader.LoadMining` 자체를 건너뛰고 그 값을 그대로 쓴다. 테스트는
이제 실제 파일을 **읽기만** 하고(양성 대조용), 그 텍스트를 메모리에서 문자열 치환한 뒤
`DepositFieldTable.FromJson(swappedJson)`으로 파싱해 `dataOverride`로 주입한다. 디스크에
아무것도 안 쓰므로 복원할 것도, 오염될 것도 없다. 테스트 끝에 "실제 파일이 안 바뀌었음"을 다시
확인하는 문장도 남겨 뒀다(설계상 당연하지만, 회귀가 생기면 여기서 잡힌다).

### 실행 결과 (round 3)

```
EditMode: tests=411 failures=0 skipped=2  (_workspace/p1-02-mining/unity-tests/EditMode-round3.xml)
PlayMode: tests=2  failures=0 skipped=0   (_workspace/p1-02-mining/unity-tests/PlayMode-round3.xml)
```

디스크 무변경 확인: `diff data/world/deposits/cradle.json client/Assets/_Project/Data/world/deposits/cradle.json` → 동일, `git status --short`도 `??`(신규 추적 안 됨)뿐 `M`(수정) 없음.

### 이번 라운드가 덮은 SC

- **SC-110**: 후반(발견 목록에서 충돌 쌍이 안 합쳐짐) 신규 통과.
- **SC-71(B)**: 접근 방식만 교체(파일 미접촉) — 결과는 이전과 동일하게 통과.
- **SC-65~70·72**: 변경 없음(round 2 상태 유지, `EditMode-round2-final.xml`이 여전히 최신 증거).

## round 4 (2026-09-27) — SC-71(B)가 round 3에서 자명 통과로 되돌아간 것을 qa가 잡음

**qa 지적, 정확했다.** round 3에서 "파일을 안 건드린다"는 목표에 치우쳐 정작 SC-71(B)가 증명해야
하는 것 자체를 잃어버렸다: `dataOverride`로 주입한 `DepositFieldTable`은 바꿔치기한 JSON을
`DepositFieldTable.FromJson`으로 **다시 파싱**한 결과인데, `DepositTableEntry`엔 광물 필드가
아예 없어서(설계대로) 그 파싱 순간 "cobaltine"이라는 값 자체가 사라진다 — 주입한 테이블이 원래
로더가 만드는 것과 내용이 완전히 같아진다. 그러면 "다른 코드 경로가 사본 파일을 직접 읽는다"는
결함이 있어도 사본엔 여전히 `starfall-glass`가 있고 메시지 fixture도 `starfall-glass`라 **두
출처가 다시 같은 값으로 일치** — round 2 이전과 똑같은 ⊘("사본 값 = 메시지 값이라 출처 구분
불가") 상태로 되돌아간 것이다. 디스크 위험을 없애려다 테스트의 증거 가치를 없앤 회귀.

**qa 제안대로 교체:** 사본이 아니라 **메시지 쪽**을 바꾼다. `revealed-and-depleted.json`을
메모리에서 `far-reach`의 `mineral_id`만 `starfall-glass` → `cobaltine`으로 치환해 전송하고,
세션은 **실제 사본을 그대로**(변형·주입 없이) 로드한다. 실제 사본에 있는 값("Starfall Glass")이
새면 빨간불, 메시지의 값("Cobaltine")만 보이면 초록불 — 이제 사본 경로와 메시지 경로가 실제로
갈린다. `dataOverride` 파라미터는 그대로 뒀다(qa: "없어도 된다", 해가 없고 재사용 가능성이
있어서 유지, 이 테스트에서는 안 씀).

**입력 대조 추가:** 실제 사본과 원본 fixture 둘 다 `far-reach`가 `starfall-glass`임을 각각
먼저 단언(치환이 실제로 의미 있는 상황이었음을 증명) + 치환된 텍스트가 원본과 다름을 단언.

**RED→GREEN 실측(team-lead가 이전에 권한 방식 재사용):** `GreyboxMiningSession.FormatMarkerLine`
에서 `_data.Minerals.TryGet(wire.MineralId, ...)`를 `_data.Minerals.TryGet("starfall-glass",
...)`로 일부러 바꿔(메시지 무시하고 항상 사본 쪽 값을 쓰는 결함 흉내) 재실행 → **정확히 이
테스트만** 실패(`tests=2 failures=1`, `DepositMarker_FollowsMessageNotDataCopy`) 확인 → 원복 →
재실행 `tests=2 failures=0`. 이번엔 이 테스트가 실제로 뭔가를 재고 있다는 것을 결함 흉내로
증명했다.

### 실행 결과 (round 4)

```
EditMode: tests=411 failures=0 skipped=2  (_workspace/p1-02-mining/unity-tests/EditMode-round4.xml)
PlayMode: tests=2  failures=0 skipped=0   (_workspace/p1-02-mining/unity-tests/PlayMode-round4.xml)
```

### 이번 라운드가 덮은 SC

- **SC-71(B)**: 재설계 — 이제 두 출처가 실제로 갈리고, 결함 흉내로 실측 검증까지 했다.
- 나머지: 변경 없음.

## round 5 (2026-09-27) — SC-110 변이 1회 (qa·team-lead 요청)

qa 지적: `DiscoveryFeedState_CollidingPilotTags_DiscoverersStayDistinct_ByActorId`의 음성 대조가
테스트 코드 안에서 표지 기준 그룹핑을 **다시 계산**할 뿐, 프로덕션 코드(`DiscoveryFeedState.
FormatSummary`)가 실제로 틀렸을 때 이 테스트의 단언이 빨개지는지는 보여주지 않는다는 것 —
PilotTag 때 한 것과 같은 1회 변이 확인이 필요하다는 요청.

**변이:** `DiscoveryFeedState.FormatSummary`를 일부러 "표지 기준으로 한 줄만 남기고 나머지는
건너뛴다"(`HashSet<string>`으로 표지 중복 시 skip)로 바꿔, 광물이 달라도 표지가 같으면 요약에
한 줄만 나오게 만들었다.

**RED 확인·기록:** `unity test client --mode EditMode --report-format junit --output
_workspace/p1-02-mining/unity-tests/EditMode-sc110-redcheck.xml` → `tests=411 failures=1`,
**정확히** `DiscoveryFeedState_CollidingPilotTags_DiscoverersStayDistinct_ByActorId` 하나만
실패(`Expected: 2 But was: 1` — 두 줄이어야 할 요약이 한 줄로 합쳐짐). 다른 410건은 영향 없음.

**원복 확인:** 변이 제거 후 `grep TEMPMUTATION` 잔존 0건, 재실행 EditMode `tests=411
failures=0`(`EditMode-round5.xml`), PlayMode `tests=2 failures=0`(`PlayMode-round5.xml`).

### 이번 라운드가 덮은 SC

- **SC-110**: 후반 음성 대조가 실제로 결함을 잡는다는 것을 변이로 증명(redcheck 파일 보존).
- 나머지: 변경 없음.

## 최종 상태 (round 5 종료 시점)

```
EditMode: tests=411 failures=0 skipped=2  (_workspace/p1-02-mining/unity-tests/EditMode-round5.xml)
PlayMode: tests=2  failures=0 skipped=0   (_workspace/p1-02-mining/unity-tests/PlayMode-round5.xml)
```

client 몫(C1·C2·C3, task #18~20) 전부 완료. SC-65~72·110 중 client 담당 전항목이 최소 한 번씩
실측 증거(정상 실행 + 최소 하나의 양성/음성 대조, 다수는 RED→GREEN 또는 결함 흉내 실측 포함)를
갖췄다.

## C4 — SC-68 1차 FAIL 재작업: 그레이박스 §8 표시 보강 (client-3, 2026-10-04)

### 왜

SC-68 사람 세션 1차 FAIL (`_workspace/p1-02-mining/evidence/sc68/notes.md`): 광맥이 3D로 안 보임
(HUD 텍스트 목록뿐), 표면 거리·사거리·속도 표시 없음, 쿨다운 표시 없음, E 키가 채굴
(`GreyboxMiningSession.cs:142`)과 롤(`ShipInputSampler.cs:106`)에 동시에 묶임. 사용자 결정: "고치고
다시 세션".

### 바뀐 파일

| 파일 | 변경 |
|---|---|
| `client/Assets/_Project/Scripts/Mining/MiningRangeStatus.cs` | 신규. 표면 거리·사거리 안·속도 판정 순수 로직(`MiningRangeEvaluator.Evaluate`) + 세 줄 포맷(`FormatStatusLines`). 서버 정의(`server/crates/sim/src/mining.rs: within_mining_range`)와 경계 일치(둘 다 `<=`). |
| `client/Assets/_Project/Scripts/Mining/DistanceLabel.cs` | 신규. m/km 포맷 순수 함수 (경계 1000). |
| `client/Assets/_Project/Scripts/Mining/CooldownDisplay.cs` | 신규. 남은 쿨다운 초 계산(`RemainingSeconds`) + null-또는-문자열 포맷(`FormatOrNull`, 0초면 줄 자체를 안 그린다). |
| `client/Assets/_Project/Scripts/Mining/MiningNoticeKind.cs` | 신규. `MiningNoticeKind{Yield,OwnDiscovery,SystemWideDiscovery}` + 분류(`ClassifyDiscovery`, actor_id 비교)·문구(`FormatBanner`/`FormatYieldNotice`) - 세 가지가 서로 다른 어휘를 쓰도록 분리. |
| `client/Assets/_Project/Scripts/Mining/DepositMarkerState.cs` | `FormatLine`의 미확인 문구를 "미확인"→"미확인 광맥"(design doc §8/SC-68 절차서 리터럴 문구)으로, `FormatLineWithDistance` 신규(거리 접미사). |
| `client/Assets/_Project/Scripts/Greybox/GreyboxMiningSession.cs` | 채굴 키 E→G(`MineKey`, `ShipInputSampler`의 E/Q 롤과 더는 안 겹침). `BuildDepositMarkers()`로 광맥마다 큐브 프리미티브(참조 표식 구체와 구분되는 모양·색) 생성. `OnGUI`에 나머지 세 요소를 추가: 가장 가까운 광맥의 표면 거리/사거리/속도 3줄, 쿨다운 줄, 산출 알림(작게)과 발견 배너(크게, 색·문구 분리, `DiscoveryBannerHoldSeconds=6`초 유지)를 별도 영역에. `DrawDepositScreenLabels`로 광맥마다 화면 투영 라벨(카메라 뒤면 숨김) + 거리. |
| `client/Assets/_Project/Scripts/Greybox/GreyboxSession.cs` | `_chaseCamera` 필드·`ChaseCamera` 접근자 추가(스크린 투영용), `GreyboxMiningSession.Init`에 카메라 프로바이더 전달. |
| `client/Assets/_Project/Tests/EditMode/MiningViewModelTests.cs` | 신규 13건(아래) + `using Starfall.Sim;`. |

### 신규 테스트 (13건, 전부 EditMode, 경계값 포함)

| 테스트 | 경계 |
|---|---|
| `MiningRangeEvaluator_SurfaceDistance_SubtractsRadius` | - |
| `MiningRangeEvaluator_RangeBoundary_ExactlyAtLimit_IsInRange` | 표면 거리 == 사거리(안쪽) vs +0.0001(바깥, 음성 대조) |
| `MiningRangeEvaluator_SpeedBoundary_ExactlyAtLimit_IsOk` | 속도 == 상한(허용) vs +0.0001(거부, 음성 대조) |
| `MiningRangeEvaluator_ReadyToMine_RequiresBothInRangeAndSpeedOk` | 한쪽만 true인 두 케이스 |
| `MiningRangeEvaluator_FormatStatusLines_MarksEachLineOkOrNg` | [OK]/[NG] 둘 다 |
| `DistanceLabel_Boundary_999IsMeters_1000IsKilometers` | 999 vs 1000 |
| `CooldownDisplay_Boundary_ElapsedEqualsCooldown_IsExactlyZero` | 경과==쿨다운 → 정확히 0 |
| `CooldownDisplay_FormatOrNull_ZeroRemaining_IsNull_NotZeroText` | 0초 → null(줄 안 그림) |
| `MiningNoticeClassifier_SameActorId_IsOwnDiscovery` | - |
| `MiningNoticeClassifier_DifferentActorId_IsSystemWideDiscovery` | - |
| `MiningNoticeClassifier_FormatBanner_OwnAndSystemWide_AreDistinctSentences` | 배너 둘 + 산출 알림 문구가 서로 다름을 단언 |
| `DepositMarkerState_FormatLine_Unrevealed_SaysExactLiteralPhrase` | "미확인 광맥" 리터럴 |
| `DepositMarkerState_FormatLineWithDistance_AppendsDistanceSuffix` | DistanceLabel과 같은 포맷 |

### 자기 점검 표 (qa-throughput R3)

| # | 항목 | 결과 |
|---|---|---|
| 1 | 계약 방법대로 실행, 1건 이상 돈다 | `unity test client --mode EditMode` → `_workspace/p1-02-mining/evidence/c4/EditMode-final.xml`: `tests="423" failures="0" errors="0" skipped="2"` (410 기존 + 13 신규, 회귀 없음) |
| 2 | 구현 변이 → 테스트 실패 | `MiningRangeStatus.cs`의 `surfaceDistanceM = distanceToCenterM - depositRadiusM` → `distanceToCenterM`(반지름 안 뺌)로 변이, `--filter "MiningRangeEvaluator*"` 재실행 → `tests="5" failures="2"` (`MiningRangeEvaluator_SurfaceDistance_SubtractsRadius`: Expected 250.0 But was 300.0, `MiningRangeEvaluator_RangeBoundary_ExactlyAtLimit_IsInRange`: Expected 150.0 But was 200.0) — 변이가 실제로 겨냥한 테스트만 잡았다 |
| 3 | 원복 증명 | 변이 전 `sha256(MiningRangeStatus.cs) = 0bbb2e2bf3b967d7450207b151991146515de694fbd0a269a9d92d87b50cb1dd`, 원복 후 같은 해시로 재확인(커밋 전 변경이라 `git diff`가 아니라 sha256으로 증명) |
| 4 | 계약 지명 바이너리·경로 존재 | `unity test client --mode EditMode` 그대로(CLI 바이너리 변경 없음). 새 파일 4개(`MiningRangeStatus.cs`/`DistanceLabel.cs`/`CooldownDisplay.cs`/`MiningNoticeKind.cs`) 전부 `client/Assets/_Project/Scripts/Mining/`(기존 asmdef `Starfall.Mining`, 참조 변경 없음) |
| 5 | 같은 턴 안에서 결과 확인 | 위 세 실행(정상→변이→원복) 전부 이 턴에서 완료, 알림 대기로 턴을 끝내지 않았다 |

### E→G 변경 확인

```
grep -n "MineKey" client/Assets/_Project/Scripts/Greybox/GreyboxMiningSession.cs
```
`const Key MineKey = Key.G;` — `ShipInputSampler.cs`의 `keyboard.eKey`(롤)·`keyboard.qKey`(역방향 롤)와 더는 안 겹친다. HUD 안내 문구(`"-- mining (" + MineKey + " = mine..."`, `"[" + MineKey + "] 채굴 가능"`)도 같은 상수를 써서 키가 또 바뀌어도 한 곳만 고치면 된다.

### 사람이 확인할 항목 (SC-68 재시도 — 그대로 사람에게 줄 수 있는 체크리스트)

1. **접속 직후**: 광맥 8개가 **주황색 큐브**로 보인다(참조 표식은 회색 구체 - 모양·색 둘 다 다름). 각 광맥 위에 화면 라벨이 보이고(카메라가 그 쪽을 보지 않으면 사라진다), 라벨에 이름·거리(m 또는 km)가 있다. 미확인 광맥은 라벨에 "미확인 광맥"이라고 적혀 있다.
2. **가장 가까운 광맥으로 비행**: HUD 박스(화면 좌상단, y=420 부근)에 `nearest: ...`, `표면 거리 ... m`, `[OK/NG] 사거리 안`, `[OK/NG] 속도 ...` 세 줄이 보인다. 150 m 안에서 멈추면(속도 ≤ 10 m/s) 셋 다 `[OK]`로 바뀌고 `[G] 채굴 가능`이 보인다.
3. **G 키를 누른다** (E 키 아님 — E는 이제 롤 전용). 접수되면 `채굴 접수됨 (tick ...)` 알림, 곧이어 `쿨다운 ...s` 줄이 나타나 3초 동안 줄다가 사라진다(0이 되면 줄 자체가 사라진다 — "쿨다운 0.0s"처럼 계속 떠 있지 않는다).
4. **산출 알림과 발견 배너가 다른 것으로 보이는가**: 채굴 성공 시 작은 글씨 `알림: 인벤토리 갱신: N종`이 HUD 박스 안에, 그와 별개로 **화면 상단 중앙에 큰 글씨 배너**가 뜬다. 내가 처음 발견했으면 **금색**("역사적 발견 — ... 이 기록은 남는다"), 다른 사람이 먼저 발견한 사건이면 **하늘색**(예: "Pilot-xxxx가 Cradle 성계에서 처음으로 ...를 발견했다"). 배너는 약 6초 뒤 사라진다.
5. **HUD 박스 겹침**: `GreyboxSession`의 본 HUD(좌상단, y=8)와 이 박스(y=420)가 작은 창에서 겹치지 않는지 — 여전히 알려진 그레이박스 한계로 남아 있다면 사람이 창 크기를 키워서 확인.
6. **발견 목록/인벤토리**: 광물별 kg, `발견된 광물 N / 4`가 같은 HUD 박스 하단에 겹치지 않고 보인다.

### 남은 것 (이번 범위 밖)

- SC-71(B) PlayMode 하네스는 이전 라운드와 동일하게 미착수 (C3 절 참고) — 이번 수정은 OnGUI/3D 표시
  보강이라 그 범위를 넓히지 않았다.
- 3D 큐브 마커의 실제 육안 확인(카메라 각도·겹침 등)은 여전히 사람 세션 몫이다 — 이 턴에서는
  EditMode로 검증 가능한 순수 로직만 TDD로 확인했고, Unity Editor를 실행해 Play하지 않았다(에이전트는
  Play 버튼을 못 누른다).

# p1-02-mining — client 계약 항목 확인

- 작성: client, 2026-09-27
- 단계: Phase 3(계약 합의). 코드 변경 없음 — `docs/specs/p1-02-mining.md`, `docs/design/p1-02-mining-design.md`(§3·§6·§8·§9), `01_architect_tasks.md`(C1~C3), `contracts/`(MINE_RESOURCE, COMMAND_RESULT, DEPOSIT_FIELD(_STATE), INVENTORY_STATE, HISTORICAL_EVENT_NOTICE, MINERAL_MINED, MINERAL_DISCOVERED, data/{mineral,deposit-field,mining-rules}.schema.json)만 읽었다.
- `02_sprint_contract.md`는 아직 초안 전(qa 진행 중, 태스크 #6). qa 초안이 오면 항목별로 다시 답한다. 이 문서는 그 전에 C1~C3 자체가 계약으로 증명 가능한지 먼저 확인한 것이다.

## 요약

C1~C3 세 태스크 모두 **계약만으로 완료 기준을 증명할 수 있다.** 코드를 손대지 않았으므로 지금은 전부 "미착수"이고, 각 태스크에 이 문서에서 제안하는 방법으로 Phase 4에서 TDD를 시작한다.

---

## C1 — 생성기 실행·EditMode fixture 테스트

**확인:** 가능하다. `contracts/fixtures/`를 세어 보면:

| 디렉터리 | 파일 수(± invalid 제외) |
|---|---|
| MINE_RESOURCE | 2 (basic, null-client-time) |
| COMMAND_RESULT | 4 (accepted, rejected-cooldown-active, rejected-recording-backlog, rejected-server-busy) |
| DEPOSIT_FIELD | 1 (example-two-deposits) |
| DEPOSIT_FIELD_STATE | 2 (all-unrevealed, revealed-and-depleted) |
| INVENTORY_STATE | 2 (empty, two-minerals) |
| HISTORICAL_EVENT_NOTICE | 2 (live, backfill) |
| MINERAL | 1 (example-uncommon) |
| MINERAL_DISCOVERED | 2 (glacine-seq-nonzero, starfall-glass) |
| MINERAL_MINED | 2 (partial-last-kg-depletes, first-extraction) |

신규 유효 fixture 합 = 18. 태스크 표의 "발견 45"는 p1-01 기존 fixture(약 27로 architect 기준선에 적힘) + 이번 18을 더한 전체 합계로 보이며 산수가 맞는다(27+18=45). 이 숫자는 실측으로만 확정되므로(architect 노트: "예측 — 실측으로 확정") **RED로 시작할 실패 테스트**는 architect가 제안한 그대로 채택한다: `ContractFixtureTests`의 발견 수 상수를 45로 바꿔 RED 확인 → codegen 실행 → 전부 역직렬화 통과.

**계약 변경 (architect, 2026-09-27):** `COMMAND_RESULT.reason_code`에 `CAPACITY_EXCEEDED` 추가(인벤토리가 표현 상한 `2^31-1` kg을 넘을 때의 거절 — 실전 도달 없음, 방어용). 유효 fixture `COMMAND_RESULT/rejected-capacity-exceeded.json` 신설로 **발견 45 → 46**, 반례(`invalid/`) 74는 그대로. 왕복 예측도 35 → 36으로 갱신. RED 기준 상수를 46으로 바꾼다. 거절 사유 문장 목록(C3)에도 반영했다.

- **`invalid/` 디렉터리 처리 — architect 확인 완료:** "역직렬화 실패 기대" 단순 규약이 아니라 p1-01 §5.4의 3층 표였다: 스키마는 전부 거부하지만, Rust serde와 C#은 층이 다를 수 있다. **C#은 "거부(예외/실패로 감지) · 감지 불가(역직렬화는 되지만 값이 스키마 위반, 예: 범위 밖 수치가 그냥 들어옴) · 계층 없음(그 필드가 C# 타입에 아예 없어서 판단 불가)" 세 갈래로 client가 직접 실측해 상수화한다.** 스펙 §5.3 신설(반영됨). architect 예측(실측 아님, C1 착수 시 갱신 대상): 신규 40건 중 C# 거부 15 · 감지 불가 13 · 불확실 1(`DEPOSIT_FIELD_STATE`의 missing-mineral-key — 필수 키인데 nullable이라 "없음"과 "null"이 헷갈리는 자리) · 계층 없음 11. **Phase 4에서 실제로 돌려 본 숫자가 다르면 내 실측이 정본이 된다** — 예측을 그대로 베끼지 않는다.
- **왕복(round-trip) 테스트:** DTO → JSON → DTO 비교. `Newtonsoft.Json` snake_case 매핑이 정확한지는 `[JsonProperty]` 어노테이션이 codegen에서 나오는 것에 의존한다. architect 실측(신규 6 DTO, 기존 10 바이트 동일, `ContractTypes.cs`만 변경)을 신뢰하고 그대로 실행한다.
- **모르는 닫힌 값 생존 — architect 확인 완료:** 생성물의 `ReasonCode`는 C# `string`이라 역직렬화로는 죽을 수 없다(architect 답변, 2026-09-27). 계약 의도(I-73)는 "알 수 없는 사유로 거절됨" 표시 + 거절로 처리 + 죽거나 수락으로 가정하지 않기 — enum이 아니라 문자열 비교/스위치의 `default` 분기로 구현한다. `delivery` 등 다른 닫힌 값도 같은 취급. codegen 산출물 자체를 의심할 필요는 없어졌다 — C3의 UI 폴백 분기만 구현하면 된다.
- **LIVE/BACKFILL 중복 제거:** `HISTORICAL_EVENT_NOTICE.historical_event.historical_event_id`(UUIDv5, 결정적)를 키로 한 Set 기반 중복 제거. fixture `live.json`과 `backfill.json`은 서로 다른 이벤트(다른 id)라서 이 fixture만으로는 "같은 id가 두 번 와도 한 번만 표시"를 증명 못 한다 — **C3 PlayMode 테스트에서 FakeTransport로 같은 id를 LIVE 뒤 BACKFILL로 재생하는 시나리오를 추가해야 한다**(C1이 아니라 C3 소관으로 제안).

**증명 방법 제안:** EditMode `ContractFixtureTests` (발견 46, 왕복 실측치, `invalid/` 40건을 거부·감지 불가·계층 없음 3층으로 실측 분류 — 스펙 §5.3) + PlayMode에서 LIVE→BACKFILL 중복 제거 1건. Phase 4에서 실제 숫자로 갱신한다.

---

## C2 — data/ 재복사 + 로더

**확인:** SC-50이 지금 빨간불이라는 architect 진단에 동의한다. `data/minerals/*.json`(4) · `data/world/deposits/cradle.json` · `data/mining/mining-rules.json` · `data/history/rules/mineral-discovery.json` = 7파일이 `client/Assets/_Project/Data`에 없으면 해시 동일성 테스트가 실패한다. 복사 자체는 기계적이라 리스크 없음.

**"정답 누출 수용" 증명 — 리더가 요청한 제안:**

design doc §3.2 요구사항은 두 겹이다.
1. (수용된 누출) `data/world/deposits/cradle.json` 사본은 `mineral_id`·`initial_reserve_kg`를 담은 채로 클라이언트 빌드에 들어간다 — 이건 막지 않는다(Q7, ADR-0012 재검토 조건 (d) 대상이지 이번 슬라이스 대상이 아니다).
2. (막아야 하는 것) **그 값을 읽는 코드가 없어야 한다.** `GreyboxDataLoader`가 deposit 표에서 읽는 필드는 `id`·`display_name`(또는 스펙 용어로는 `name`)·`position_m`·`radius_m` 뿐이고, `mineral_id`·`initial_reserve_kg`는 파싱 모델 자체에 없거나 있어도 광맥 표시 경로에서 참조되지 않아야 한다.

architect가 제안한 실패 테스트("사본의 광물을 바꿔도 표식이 `DEPOSIT_FIELD_STATE`를 따른다")를 그대로 채택하고, 구체적인 테스트 설계를 제안한다:

- **테스트 A (양성 대조 — 코드가 안 읽는다는 것을 직접 본다):** 클라이언트 DTO 모델(`DepositTableEntry` 등, C2가 만들 로더 타입)에 `mineral_id`·`initial_reserve_kg` 필드가 아예 없거나, 있다면 `[JsonIgnore]`이거나 접근자가 `internal`/사용처 0인지 정적으로 확인한다. **가장 강한 증명은 애초에 그 필드를 파싱 모델에 넣지 않는 것**이다 — 읽을 수 없는 필드는 실수로도 못 읽는다. 이걸 architect·C1과 맞춰 DTO 설계 단계에서 반영하자고 제안한다(계약 DTO가 아니라 `Scripts/Greybox/`의 로컬 데이터 모델이므로 client가 직접 정할 수 있는 자리).
- **테스트 B (동작 대조 — architect가 제안한 형태):** EditMode 또는 PlayMode에서 (1) 복사본 JSON을 임시로 다른 `mineral_id`로 바꿔치기(또는 테스트 픽스처 디렉터리로 로더를 가리킴) (2) `DEPOSIT_FIELD_STATE`(`all-unrevealed.json` fixture)를 FakeTransport로 주입 (3) 광맥 표식의 표시 상태(뷰모델)가 "미확인 광맥"이고, 로컬 사본에서 바꾼 값과 무관함을 확인. 그다음 `revealed-and-depleted.json`을 주입해 표식이 그 메시지의 `mineral_id`로 바뀌는 것도 같은 테스트에서 확인(음성 대조 + 양성 대조 짝 — CLAUDE.md 검증 규율).
- 두 테스트를 모두 넣는 것을 제안한다: A는 정적으로 "읽는 코드가 존재하지 않음"을, B는 동적으로 "표시가 서버 메시지만 따름"을 증명한다. 하나만으로는 규칙 9(소스를 읽어 확인한 부정)를 완전히 만족하지 못한다 — A 없이 B만 있으면 "우연히 안 읽었다"와 "읽을 수 없게 만들었다"를 구분 못 한다.

- **로더 확장 범위:** 광물 4종·채굴 규칙 1종도 C2 범위다. 이 둘은 서버 전용이 아니므로(광물 이름·희귀도는 스펙상 표시용 공개 정보, design doc: "rarity는 클라이언트가 표시 전용으로 읽는다") 리스크 없이 전체 필드를 읽어도 된다. `data/history/rules/mineral-discovery.json`은 client가 읽을 이유가 없다 — **로더가 이 파일을 파싱만 하고(사본 동일성 테스트 대상이므로 복사는 필요) 어떤 코드도 그 값을 소비하지 않아야 한다.** 이것도 A와 같은 종류의 "읽지 않는 코드" 확인 대상으로 Q1의 "읽히지 않는 데이터 파일 0" 항목(AC-19(c2))에 걸릴 만하다 — qa에게 이 파일을 그 항목의 분모에 넣도록 제안한다.

**증명 방법 제안:** SC-50 확장(7파일 해시 동일성) + 테스트 A(정적: 파싱 모델에 서버 전용 필드 부재/미사용) + 테스트 B(동적: FakeTransport 두 메시지로 표시가 메시지만 따름을 대조).

---

## C3 — 그레이박스 채굴 UI

**확인:** 계약이 필요한 정보를 전부 제공한다. 항목별 확인:

- **미확인/드러남 표식 + 잔량 + 최초 발견자:** `DEPOSIT_FIELD_STATE`의 "넷 다 null 아니면 넷 다 값" 규칙(스키마가 표현 못 해 서버가 강제, QA가 양성 대조로 확인 — I-68)을 클라이언트도 그대로 믿고 렌더링 분기를 이 규칙에 맞춘다: `mineral_id == null`이면 미확인 렌더, 아니면 드러남 렌더. "최초 발견자"는 `DEPOSIT_FIELD_STATE`에는 없다 — 발견자 이름은 `HISTORICAL_EVENT_NOTICE`(BACKFILL 포함)에서 온다. **두 메시지를 합쳐야 광맥 표식에 "최초 발견: Pilot-xxxx"를 채울 수 있다** — `deposit_id`로 조인(`MineralDiscoveredPayload.deposit_id` ↔ `DepositState.deposit_id`). 이 조인 로직이 C3의 핵심이고 뷰모델 계층(스킬 §4)에서 구현한다.
- **채굴 가능 표시(거리·사거리·속도) — architect 확인 완료:** `data/mining/mining-rules.json` 사본에서 `mining_range_from_surface_m`·`max_ship_speed_mps`를 읽어 표시용으로만 쓴다(판정은 서버). 실제 거리·속도는 `Flight/PredictedShipController.CurrentState`(`ShipSimState`, Position/Velocity f64)에 이미 있다 — **C3에 선행 태스크 불필요**, 이 예측 상태를 그대로 읽는다.
- **`MINE_RESOURCE` 전송:** `command_id`(Guid) 생성, 대기 목록 등록, 재시도 시 같은 id 재사용 — 스킬 §2 규칙 그대로. payload는 `deposit_id` 하나뿐이라 단순하다.
- **인벤토리 패널:** `INVENTORY_STATE`만 반영. architect가 제안한 실패 테스트("인벤토리 패널이 `INVENTORY_STATE` 수신 전에는 바뀌지 않는다")를 그대로 채택 — `MINE_RESOURCE` 전송 직후, `COMMAND_RESULT` 수신 후에도, `INVENTORY_STATE`가 오기 전까지는 패널이 이전 값을 유지해야 한다. 이건 "낙관적 갱신 금지"를 직접 검증하는 좋은 RED다.
- **거부 사유 문장:** `RejectReasonCode` 7개 신규값(`TARGET_UNKNOWN`, `COOLDOWN_ACTIVE`, `TARGET_OUT_OF_RANGE`, `SHIP_TOO_FAST`, `RESOURCE_DEPLETED`, `RECORDING_BACKLOG`, **`CAPACITY_EXCEEDED`**(architect 추가, 2026-09-27 — 인벤토리 상한 `2^31-1` kg 초과, 실전 도달 없는 방어용 사유)) 각각에 로컬라이즈 키가 필요하다. `MALFORMED_COMMAND` 등 기존 8개와 함께 닫힌 집합이 아니므로(스키마: "later version may add") **모르는 코드에 대한 기본 문구("알 수 없는 사유로 거절됨")**를 폴백으로 둔다 — C1에서 확인한 "모르는 닫힌 값 생존"과 같은 원칙을 UI 레이어에도 적용.
- **산출 알림 ≠ 발견 배너:** design §3.3의 "두 박자"를 그대로 구현 — 산출 알림은 `COMMAND_RESULT`(accepted) + `INVENTORY_STATE` 수신 즉시, 발견 배너는 **오직** `HISTORICAL_EVENT_NOTICE`(delivery=LIVE) 수신 시에만 표시한다. 클라이언트가 "인벤토리가 0→N으로 바뀌었으니 첫 획득이다"를 스스로 판단해 배너를 띄우면 원칙 1·2 위반이다 — 이 구분을 테스트로 강제하는 것을 제안: FakeTransport로 `COMMAND_RESULT`+`INVENTORY_STATE`만 보내고 `HISTORICAL_EVENT_NOTICE`를 안 보내면 발견 배너가 뜨지 않는 것을 확인(음성 대조).
- **발견 목록 `N / 4`:** `HISTORICAL_EVENT_NOTICE`(BACKFILL 포함) 누적 개수. 분모 4는 클라이언트 데이터 사본의 광물 수(`data/minerals/*.json` 파일 수)에서 유도 — 서버가 분모를 안 주므로, **클라이언트가 사본에서 세는 것은 "광물이 몇 종류 있는가"라는 공개 정보이지 서버 전용 값이 아니어서 C2의 누출 규칙과 충돌하지 않는다**(광물 파일 자체는 전부 공개, 광맥-광물 매핑만 비공개).
- **`Pilot-xxxx` 표지 — architect 확정 + 리더 경고:** 알고리즘은 **`actor_id` 문자열의 끝 4글자**(architect, 2026-09-27). 내가 제안했던 "앞 4 hex"는 UUIDv7의 생성 시각 비트라 비슷한 시각에 만든 캐릭터끼리 충돌하므로 기각됐다. **끝 4글자도 이 프로젝트의 개발용 ID 체계에서는 실제로 충돌한다**(리더 확인 예시: Unity `DefaultSubject`와 `bot-001`이 둘 다 `Pilot-0001`, `SecondObserverSubject`와 `bot-002`가 둘 다 `Pilot-0002`). 표지는 **표시 전용**이라 이 충돌 자체는 치명적이지 않지만, **테스트·로그 대조는 표지 문자열이 아니라 `actor_id`로 해야 한다** — 표지를 키로 쓰는 코드나 테스트를 만들지 않는다. 순수 함수(`actor_id → Pilot-xxxx`)로 구현하고 대조는 항상 원본 `actor_id`로.
- **"A가 0.35초 먼저":** `MINERAL_MINED`는 클라이언트에 오지 않는다(레지스트리 확인: consumers는 `server`, `history`뿐). 대신 클라이언트는 **자기 채굴의 tick**을 자신의 `COMMAND_RESULT`(accepted) envelope `tick`에서 얻고, **발견자의 tick**을 `HISTORICAL_EVENT_NOTICE.historical_event.tick`(HSE envelope의 "결정적 source 도메인 이벤트의 tick")에서 얻어 `|내 tick − historical_event.tick| / tick_hz`로 초 단위 차이를 계산한다. `tick_hz = 20`(ADR-0006, 1 tick = 50 ms)을 상수로 박는다 — design §3.4 예시(tick 차 7 → 0.35 s)와 일치. **같은 tick이면 0초라 쓰지 않고 "한발 먼저"만 쓴다**(§3.1 결정) — 이 분기를 테스트로 커버해야 한다(tick 차 0과 tick 차 7 두 케이스).

**증명 방법 제안:** PlayMode 씬에서 FakeTransport로 §5 시나리오 중 클라이언트가 볼 수 있는 부분(S-1 "느낌" 항목, S-4의 "나중 접속자" 초기 상태 반영, S-9 "드러나지 않은 광맥은 새지 않는다"의 클라이언트 쪽)을 재생. 특히:
  1. 인벤토리 패널이 `INVENTORY_STATE` 전에는 안 바뀐다(EditMode, architect 제안 그대로).
  2. 발견 배너는 `HISTORICAL_EVENT_NOTICE` 없이는 안 뜬다(음성 대조).
  3. BACKFILL만 받고 시작한 세션(A가 이미 나간 뒤 접속하는 B 흉내)에서 광맥 라벨·발견 목록이 올바르게 채워진다(S-4).
  4. tick 차 0과 7의 "먼저" 문구 분기.

---

## 발견한 것 — architect·qa에게 알린 사항 (전부 회신 받음, 2026-09-27)

architect가 답을 처음에 `client-pr1`(끝난 에이전트)로 잘못 보내서 team-lead가 중계했다. 넷 다 해결됐고 위 C1·C3 본문에 반영했다:

1. **월드 미러 — 해결.** `Flight/PredictedShipController.CurrentState`(`ShipSimState`, f64 Position/Velocity)가 이미 있다. C3에 선행 태스크 없음.
2. **`Pilot-xxxx` 알고리즘 — 해결(경고 포함).** `actor_id` 끝 4글자. 이 프로젝트의 개발용 ID 체계에서 실제로 충돌 사례가 있다(team-lead 확인) — **표지는 표시 전용, 테스트·로그 대조는 반드시 `actor_id`로.** 표지를 키로 쓰지 않는다.
3. **`invalid/` fixture 처리 — 해결.** p1-01 §5.4의 "거부/감지 불가/계층 없음" 3층 실측 표. 스펙 §5.3 신설. architect 예측(15/13/1/11)은 예측일 뿐, Phase 4 실측이 정본.
4. **모르는 거절 사유 — 해결.** `ReasonCode`는 C# `string`이라 역직렬화로 안 죽는다. UI가 `default` 분기로 "알 수 없는 사유" 폴백 처리하면 된다.

---

## `02_sprint_contract.md` 초안 r0 대조 — SC-65~72 (qa 요청, 2026-09-27)

**증명 가능 6건(SC-65·67·68·69·71·72), 수정 필요 2건(SC-66·70 — 숫자·테스트 입력 출처).** 상세:

| SC | 판정 | 근거·수정안 |
|----|------|-----------|
| SC-65 | **증명 가능** | CAPACITY_EXCEEDED는 `COMMAND_RESULT`(기존 p1-01 DTO)의 문자열 필드 값 추가일 뿐 새 DTO가 아니다 — 신규 6·기존 10 바이트 동일 기준 불변. 테스트명 제안: `ContractsCodegen_IsIdempotentAndPreservesUnrelatedDtos`. |
| SC-66 | **수정 필요 — 상수 46/36** | architect가 `CAPACITY_EXCEEDED` fixture(`COMMAND_RESULT/rejected-capacity-exceeded.json`)를 추가해 유효 fixture 45→46, 왕복 예측 35→36으로 바뀌었다(위 "계약 변경" 절 참고). **문서 최상단 "계약 데이터" 줄과 §0.7 기준선의 "fixture 45 vs 기대 27"도 46 vs 27로 갱신 필요** — 이건 qa/architect 소관이라 확인만 요청한다. |
| SC-67 | **증명 가능, 입력 출처만 명확히** | `reason_code`·`delivery`·`visibility`·`entity_kind`·`role` 각각 "닫힌 목록 밖 문자열"을 검사해야 하는데, `contracts/fixtures/`에는 이런 음성 입력이 없다(있는 건 스키마가 **거부**하는 반례뿐 — 이건 "모르는 값도 받아준다"의 반대). **이 5건은 client가 EditMode 테스트 코드 안에 직접 쓴 JSON 문자열**(예: `"reason_code": "SOME_FUTURE_REASON"`)로 검사하는 것을 제안한다 — §0.6("기대 숫자는 fixtures에서만")과 안 부딪힌다. 기대 숫자를 읽는 게 아니라 client가 스스로 만든 미래값 입력에 대한 방어를 보는 것이기 때문. 테스트명: `UnknownClosedValue_ReasonCode`·`_Delivery`·`_Visibility`·`_EntityKind`·`_Role`. |
| SC-68 | **증명 가능(사람 전용)** | 에이전트는 Play를 못 누른다 — Phase 4 종료 시 사람 세션 스텝으로 명시적으로 분리해 둔다. 새 월드 필요(§0.2)는 C3 구현자가 세션 시작 전에 확인. |
| SC-69 | **증명 가능, grep 패턴 실측 확인** | 기존 생성 코드(`CommandResultMessage.cs`)를 읽어 codegen의 snake_case→PascalCase 관례를 확인했다: `reason_code` → `ReasonCode`(`string`, enum 아님). 같은 관례면 `INVENTORY_STATE`의 `quantity_kg` → `QuantityKg`가 된다 — **grep 패턴 `quantity_kg\|QuantityKg` 는 codegen 관례와 맞는다.** 다만 이건 코드가 아직 없어 확정이 아니라 관례 추정이다 — C1 완료 시 실제 생성 파일로 재확인해서 다르면 qa에게 알린다. |
| SC-70 | **증명 가능, 입력은 합성 필요** | fixture `HISTORICAL_EVENT_NOTICE/live.json`과 `backfill.json`은 서로 다른 `historical_event_id`를 담고 있어(다른 발견 기록 예시) 그대로 쓰면 "같은 id 두 번"을 재현 못 한다. **테스트가 `backfill.json`을 복제해 `historical_event_id`만 `live.json`과 같게 바꾼 합성 입력을 써야 한다** — SC-67과 같은 이유로 §0.6과 안 부딪힌다(합성 입력이지 기대 숫자가 아니다). 테스트명 제안 그대로(`DiscoveryFeed_DedupesByHistoricalEventId`) 채택, 입력 출처만 리포트에 "fixture 변형(backfill.json의 id를 live.json과 동일하게 치환)"이라고 적을 것을 제안. |
| SC-71 | **증명 가능** | grep 대상이 `Scripts/Greybox`(코드)이고 대상 데이터는 `Assets/_Project/Data`(에셋)라 겹치지 않는다. C2에서 "파싱 모델에 필드 자체를 안 넣는다"(테스트 A) 설계를 채택하면 `mineral_id`·`initial_reserve_kg` 문자열이 로더 코드에 애초에 안 나타나 grep이 자연히 0이 된다 — 우연이 아니라 설계로 0을 만드는 것. |
| SC-72 | **증명 가능 — Q-7 답변** | **가능하다.** 원본 카운트는 사본이 아니라 `Path.Combine(Application.dataPath, "../../data")`(레포 `data/`, 프로젝트 밖 상대 경로)를 직접 순회해서 세고, 사본 카운트는 `Assets/_Project/Data`를 따로 순회해서 센다 — **두 숫자를 별도 변수로 로그에 찍고 같음을 단언**하면 qa가 우려한 "사본에서만 세서 3=3으로 우연히 맞는" 함정을 피한다. `ClientDataCopy_MatchesRepositoryOriginal`을 이 구조로 확장 제안. |

**요약 답변(qa Q-7):** 둘 다 지금 테스트 구조(NUnit + `Directory.GetFiles` + `TestContext.WriteLine`/`Console.WriteLine`으로 `test-results.xml`에 로그 출력)에서 가능하다. 왕복 예측은 46/36으로 정정.

---

## r1 대조 (qa, 2026-09-27) — SC-65~72·110, 9건 전부 서명

qa가 client 제안(SC-71 두 방향, SC-70 PlayMode, SC-66 46/36)을 전부 반영하고 SC-110(Pilot 표지 충돌)을 신설했다. 항목별 확인:

| SC | 서명 | 비고 |
|----|------|------|
| SC-65 | ☑ | 변경 없음 |
| SC-66 | ☑ | 46/36 반영 확인. 반례 40건 C# 3층 분류가 같은 테스트에서 §0.5 ③ 열도 채운다는 것도 동의 — C1 테스트 설계에 포함 |
| SC-67 | ☑ | 변경 없음 |
| SC-68 | ☑ | 변경 없음(사람 전용) |
| SC-69 | ☑ | 변경 없음 |
| SC-70 | ☑ | PlayMode FakeTransport로 같은 id를 LIVE 뒤 BACKFILL 재생 — 내 지적 그대로 반영됨 |
| SC-71 | ☑ | (A) 리플렉션 테스트에 **양성 대조로 `position_m` 대응 필드가 있음을 먼저 단언**하는 것에 동의 — 이게 없으면 "리플렉션이 엉뚱한 타입을 봤다"와 "정말 필드가 없다"를 못 가른다(qa 지적, 맞다). (B)는 기존 제안 그대로. 두 방향 다 통과해야 PASS라는 것도 동의 — 하나만으론 "우연히 안 읽음"과 "못 읽게 만듦"을 못 가른다는 내 원래 우려와 같은 이유 |
| SC-72 | ☑ | PlayMode 결과 파일도 병합 조건에 포함된 것 확인. Q-7 답은 아래 |
| SC-110(신설) | ☑ | `actor_id` 끝 4글자, 충돌 쌍을 음성 입력으로 검증, 대조 키는 항상 `actor_id` — 리더가 전달한 경고와 정확히 일치한다. `⊘`가 "두 입력의 표지가 같음을 먼저 단언"을 요구하는 것에 동의(그렇지 않으면 "표지가 원래 달라 합칠 필요가 없었다"는 자명 통과가 된다). 주어진 예시 ID(`…8e57-000000000001`/`…8000-000000000001`)는 둘 다 끝 4글자 `0001`로 실제로 겹친다 — 입력으로 그대로 쓸 수 있다 |

**Q-7 답변(재확인):** SC-72는 원본 쪽 10을 반드시 세야 한다는 지적에 동의 — 구현은 `Path.Combine(Application.dataPath, "../../data")`(레포 원본, 사본이 아니다)를 사본과 **독립적으로** 순회해 두 숫자를 각각 로그에 찍고 같음을 단언하는 구조로 한다(위 절에서 이미 제안한 그대로, 여기서 재확인).

**Q-8(규칙 파일 미소비) 동의:** 스펙 AC에 없다는 qa 판단에 동의한다. 계약 항목으로 넣지 않고 **client 자체 EditMode 테스트**(`MiningRuleFile_ValuesNotConsumedByGameplay` 류, 계약 판정에는 안 쓰고 회귀 방지용)로만 두는 것으로 확정. architect에게 AC를 새로 요청하지 않는다.

**사소한 것:** §11 Q-7 원문에 "왕복 예측 35"가 남아 있다(46/36 반영 후 오타로 보임) — qa가 정리할 때 참고.

**서명:** client — 9건(SC-65~72·110) 전부 "이 기준으로 완료를 증명할 수 있다"로 확인. 2026-09-27.

---

## r2 최종 확인 (qa, 2026-09-27) — §11 N 서명

r2에서 client 몫(SC-65~72·110)은 r1과 내용 동일 — 반영된 것: SC-66 헤더·§0.7 처리(측정 당시 45를 안 덮어쓰고 "계약 변경으로 46" 주석), §0.6에 "입력과 기대 숫자를 구분한다"(합성 입력 원칙) 명문화, SC-72 원본·사본 독립 순회 서술. 전부 확인했고 이견 없다.

**§0.7을 실측값 보존 + 주석으로 처리한 것에 동의** — 측정값을 사후에 덮어쓰면 "그 시각에 실제로 그렇게 측정했다"는 기록 자체가 거짓이 된다는 이유가 맞다.

**서명: client — SC-65~72·110 (§11 N절) ☑.** 2026-09-27. #9는 이 서명으로 닫힌 것으로 본다.

---

## 다음

r2 서명 완료, qa에게 전달했다. client 몫 계약 합의는 종료 — Phase 4(TDD) 착수 시 이 문서의 테스트명·구현 메모를 그대로 시작점으로 쓴다.

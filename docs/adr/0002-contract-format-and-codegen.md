# 0002. 계약 형식과 코드 생성 방식

- 상태: accepted (server·client 검토 완료, 2026-09-18)
- 날짜: 2026-09-17 (검토 반영 2026-09-18)
- 슬라이스: p0-01-bootstrap
- 검토: `_workspace/p0-01-bootstrap/01_server_adr_review.md`(A-6~A-13, R-5~R-10), `_workspace/p0-01-bootstrap/01_client_adr_review.md`(B-1, C-1~C-5, D-1~D-2). 반영 내역은 `01_architect_decisions.md`

## 맥락

서버(Rust)와 클라이언트(C#)가 같은 데이터 모양을 각자 정의하면 반드시 어긋난다(기획안 HSE §100). 기획안은 초기엔 JSON Schema, 나중에 바이너리로 가라고 했다. 정해야 할 것은 (1) 스키마 방언과 작성 스타일, (2) Rust·C# 타입을 **손으로 쓰는가 생성하는가**, (3) ID·시간·화폐 표기, (4) 부트스트랩에서 실제로 실행할 명령이다. 결정은 이 PC에서 실제로 돌려 본 결과에 근거한다(아래 "검증한 사실").

## 결정

### 1. 형식: JSON Schema 2020-12, 파일 하나에 타입 하나

- 와이어 포맷은 UTF-8 JSON 텍스트. 바이너리 전환(Protobuf 등)은 실시간 전투 규모 측정 후 별도 ADR.
- 모든 스키마에 절대 `$id`를 붙인다: `https://schemas.starfall.invalid/contracts/{레포 상대 경로}`. `.invalid`는 예약 TLD라 어떤 검증기도 네트워크로 가져올 수 없다. 파일 간 참조는 **상대 `$ref`**(`../common/primitives.schema.json#/$defs/UuidV7`)로 쓰고, 검증기는 모든 스키마를 미리 등록한 뒤 **오프라인**으로 돌린다. `$id`와 실제 경로가 어긋나면 등록은 성공하고 `$ref`가 조용히 엉뚱한 문서를 가리키므로, 이 일치는 테스트가 지킨다(§3 테스트 8).
- 공통 값은 `common/primitives.schema.json`의 `$defs`에만 정의한다(`Uuid`, `UuidV7`, `TypeName`, `SchemaVersion`, `SafeInteger`, `NonNegativeSafeInteger`, `Tick`, `Sequence`, `MoneyMinor`, `GameTime`, `RealTime`). 패턴을 타입 스키마에 복사하지 않는다.
- 타입 스키마 작성 스타일(고정):
  ```
  allOf: [ { $ref: "../common/{command|event|message}-envelope.schema.json" } ]
  properties: { {command|message|event}_type: {const: "NAME"}, schema_version: {const: N}, payload: {$ref: "#/$defs/NamePayload"} }
  unevaluatedProperties: false      # envelope 밖의 필드를 막는다
  $defs: { NamePayload: { type: object, ..., additionalProperties: false } }
  ```
  `properties.{type}.const`를 **최상위에** 두는 것은 QA 계약 커버리지 스크립트가 그 자리에서 타입 상수를 읽기 때문이다.
- envelope 필드는 **항상 존재**하고, 값이 없으면 `null`(`anyOf: [X, {type: "null"}]`). payload 필드만 선택(optional)을 쓴다. 이 규칙은 스타일이 아니라 **왕복 테스트(§3 테스트 2)를 성립시키는 전제**다. "필드 없음"과 "null"이 Rust `Option`과 C# nullable에서 다르게 해석되면 왕복이 불가능해진다.
- **정수 필드는 `minimum`·`maximum`을 반드시 선언한다.** 범위가 언어 타입 매핑의 유일한 근거다(§4). 정수에는 `format`을 쓰지 않는다 — 범위와 `format`이 둘 다 있으면 진실이 둘이 되고, 실제로 `probe_seq`(`0..4294967295`인데 `format: int64`)에서 두 검토자가 같은 모순을 지적했다. 문자열의 `format: uuid`는 유지한다(생성기가 `System.Guid` 매핑에 쓴다).
- 명령 envelope에는 `player_id`/`actor_id`를 두지 않는다. 행위자는 세션에서 서버가 정한다(CLAUDE.md 원칙 1).
- 서버 메시지 envelope(`message-envelope`)을 새로 정의한다. 스킬에 명시된 명령·이벤트 envelope만으로는 WebSocket 서버 메시지의 `message_type`·`tick` 자리가 없다.

### 2. fixture: 정상 + 거부되어야 할 예시

- `contracts/fixtures/{NAME}/*.json` = 유효 예시. 타입마다 최소 1개, 경계값(널 가능 필드 null, 정수 상한)을 포함한다.
- `contracts/fixtures/{NAME}/invalid/*.json` = **반드시 거부되어야 하는** 예시. 파일명이 거부 이유다. 현재 7건: `actor-field-injected`, `command-id-not-v7`, `probe-seq-negative`, `probe-seq-above-u32`, `missing-tick`, `payload-unknown-field`, `tick-above-safe-integer`.
  - 구조 위반(누락·미지 필드·패턴)뿐 아니라 **상한 위반**을 반드시 포함한다. `MoneyMinor`·`Tick`·`SafeInteger`는 "범위가 곧 의미"인 타입이고, 상한을 넘긴 정수가 조용히 통과하는 것은 나중에 그대로 화폐 버그의 입구가 된다.
- QA 커버리지 스크립트는 `fixtures/{NAME}/*.json`만 세고 하위 디렉토리를 보지 않는다. 이는 **의도한 것**이다: `invalid/`는 "이 타입의 예시"가 아니라 "검증기가 살아 있는지 증명하는 반례"이므로 커버리지 수에 들어가면 안 된다. 대신 Rust 계약 테스트가 `invalid/`를 전부 거부하는지 확인하고, QA 스프린트 계약에 별도 항목으로 넣는다. 통과만 확인하는 테스트는 검증기가 꺼져 있어도 통과한다.
- 거부 책임은 층마다 다르다(스키마 / Rust serde / C# 역직렬화). 어느 파일을 어느 층이 잡는지는 스펙 `docs/specs/p0-01-bootstrap.md` §5의 표가 정본이다.

### 3. Rust: 손으로 쓴 serde 타입 + 레지스트리 주도 계약 테스트 9종

`server/crates/contracts`에 serde 타입을 **손으로 쓴다**. 생성하지 않는다. 대신 다음 테스트로 계약과의 일치를 증명한다.

| # | 테스트 | 무엇을 막는가 |
|---|--------|--------------|
| 1 | 모든 `*.schema.json`이 2020-12 메타스키마로 유효하고 `$ref`가 전부 해석된다(네트워크 접근 없이) | 깨진 스키마, 미등록 참조 |
| 2 | 유효 fixture → 역직렬화 → 재직렬화 → **원본과 의미적으로 동일**(`serde_json::Value` 비교) → 재직렬화 결과를 스키마로 검증 | 필드 누락·이름 오타·직렬화 형식 변형 |
| 3 | `invalid/` 전부가 **스키마 검증에서** 거부된다 | 검증기가 꺼진 채 통과하는 상태 |
| 4 | 레지스트리 `schema_version` == 스키마 `const` == fixture 값, 레지스트리 이름 == 스키마 타입 상수 | 레지스트리와 스키마의 드리프트 |
| 5 | `producers` **또는** `consumers`에 `server`가 포함된 타입은 이름 → Rust 타입 대응표에 반드시 존재. fixture 디렉토리인데 레지스트리에 없으면 실패 | 계약에만 있고 코드에 없는 타입 |
| 6 | `invalid/` 전부를 해당 Rust 타입으로 **역직렬화 시도**하고 기대 결과표와 대조한다 | **운영 경로 구멍**: 스키마는 거부하는데 serde는 통과시키는 조합. 운영 중 명령을 막는 것은 스키마 검증기가 아니라 serde다 |
| 7 | 유효 fixture에서 스키마의 `required` 필드를 하나씩 제거해 역직렬화가 전부 실패하는지 | 스키마는 required인데 Rust가 `Option<T>`인 드리프트. fixture는 항상 그 필드를 갖고 있어 테스트 2로는 영원히 드러나지 않는다 |
| 8 | `registry/types.json`을 `registry/types.schema.json`으로 검증 + 엔트리의 `schema` 경로 존재 + 모든 `$id`가 경로 규칙과 일치 | 아무도 검사하지 않던 레지스트리 데이터 파일, `$ref`가 엉뚱한 문서를 가리키는 사고 |
| 9 | 정수 상한 위반(`tick = 2^53`, `probe_seq = 2^32`)이 역직렬화에서 실패 | 범위를 타입으로 좁히지 않은 구현 |

테스트 5의 태그 검사는 **해당 크레이트가 존재하는 태그에 대해서만** 한다. p0-01에서는 `server`만이다(`starfall-history`는 이번에 만들지 않는다 — ADR-0001 §2).

**검증기 구성.** `jsonschema` 크레이트(0.56.x)를 `default-features = false`로 쓰고, 모든 스키마를 `$id`로 `Registry`에 등록한 뒤 `offline()` 리트리버와 함께 빌드한다. 공식 문서상 `resolve-http`·`resolve-file`은 opt-in feature다. 따라서 `default-features = false`의 의미는 "기본에 들어 있는 리졸버를 끈다"가 아니라 **"리졸버 feature를 명시적으로 켜지 않아 미등록 `$ref`가 조용한 네트워크 접근이 아니라 실패로 드러나게 한다"**이며, `offline()`은 그 보장을 feature 플래그가 아니라 검증기 자체로 만든다.

**"재직렬화 = 원본"의 달성 조건(Rust 타입 설계 제약).** 이 4가지가 없으면 구현자가 자연스럽게 고르는 방식이 테스트 2를 반드시 깨뜨린다.

1. 비교는 문자열이 아니라 `serde_json::Value`로 한다(필드 순서 문제 소멸). 스펙의 "의미적으로 동일"은 이 뜻이다.
2. **널 가능 envelope 필드에 `skip_serializing_if`를 쓰지 않는다.** `Option<T>`에 반사적으로 붙이는 이 속성이 붙으면 `client_sent_at: null`이 재직렬화에서 사라지고, envelope 규칙(§1)이 깨지며, 클라이언트는 "필드 없음"을 받는다. 선택 필드(payload 한정)가 생기면 그때만 허용한다.
3. **`RealTime`·`GameTime`을 시간 타입으로 매핑하지 않는다.** `chrono::DateTime<Utc>`는 소수 자리를 0/3/6/9로 정규화해(`.1` → `.100`) 1~9자리를 허용하는 계약 패턴과 왕복이 깨진다. 검증하는 **문자열 newtype**으로 간다(C#의 `DateParseHandling.None` 요구와 같은 문제의 Rust판).
4. 정수는 폭이 정확한 타입 + 범위 검증 newtype으로. 계약에 실수(float)를 넣지 않는 것이 이 테스트의 전제다.

**envelope을 `#[serde(flatten)]`으로 공유하지 않는다.** serde의 `deny_unknown_fields`는 `flatten`과 함께 동작하지 않고, 내부 태그 열거형(`#[serde(tag = "...")]`)도 같은 버퍼링 경로라 마찬가지다. 둘 중 하나라도 쓰면 `actor-field-injected`를 **스키마는 거부하는데 서버는 받아들인다** — 원칙 1의 계약 수준 방어(§1의 "명령 envelope에 행위자 없음")가 운영 경로에서 무너진다. 타입마다 envelope 필드를 펼쳐 쓰고 `deny_unknown_fields`를 건다. 반복이 부담이면 선언 매크로로 줄이되 `deny_unknown_fields`는 유지한다. `command_type` peek(디스패치)용 구조체만 예외로 둔다.

**커버리지 스크립트 전제.** 스크립트는 코드에서 타입 이름 문자열을 찾는다. 타입 태그를 `concat!`이나 매크로 조합으로 만들면 검색에 걸리지 않는다. 리터럴 상수로 둔다.

### 4. C#: 제한된 부분집합 전용 자체 생성기 (`tools/codegen`)

DTO는 `client/Assets/_Project/Scripts/Contracts/Generated/`에 **생성**하고 손으로 고치지 않는다(생성물은 커밋한다 — 클라이언트 빌드가 .NET SDK 없이도 되어야 한다). 생성기는 **.NET 10 SDK의 단일 파일 앱**(`dotnet run tools/codegen/ContractsCodegen.cs`)이며 NuGet 의존 없이 `System.Text.Json`만 쓴다.

**키워드 allowlist(3분류).** "모르는 키워드에서 실패"가 실제 방어가 되려면 아는 키워드가 명시적 목록이어야 한다.

| 분류 | 키워드 | 처리 |
|------|--------|------|
| 구조(출력에 영향) | `type`, `properties`, `required`, `$ref`, `$defs`, `allOf`, `anyOf`, `const`, `format`, `additionalProperties`, `unevaluatedProperties`, `items`, `enum` | 해석한다 |
| 주석(안전하게 무시) | `$schema`, `$id`, `title`, `description`, `examples`, `$comment`, `deprecated` | 무시한다(코드에 상수 목록으로 둔다) |
| 제약(C# 출력에는 영향 없음) | `pattern`, `minItems`, `uniqueItems` | 무시한다. Rust 검증기가 강제한다 |
| 범위(정수 매핑 근거) | `minimum`, `maximum` | 정수 타입 선택에 쓴다 |
| 그 외 | — | **JSON Pointer 경로와 함께 실패** |

**타입 매핑.**

| 스키마 | C# |
|--------|-----|
| `string` + `format: uuid` | `System.Guid` |
| `string`(그 외), 특히 `$ref`가 `RealTime`·`GameTime` | `string`. **`DateTime`/`DateTimeOffset`으로 매핑하지 않는다** — `GameTime`(3827년)을 시간 타입에 넣는 순간 게임 달력이 실제 시간 축에 올라탄다 |
| `integer` | 선언된 `[minimum, maximum]`을 **손실 없이 담는 가장 좁은 타입**을 `int` → `uint` → `long` 순으로. 범위 선언이 없으면 실패. `ulong`은 쓰지 않는다(2^53−1 전제가 C#에서만 느슨해진다) |
| `boolean` | `bool` |
| `const` | 기본값이 박힌 속성 + `public const` 상수 |
| `object` | 중첩 클래스 |

범위 기반 매핑의 실익: `probe_seq`(`0..4294967295`)가 `uint`가 되어 C#이 `probe-seq-negative`·`probe-seq-above-u32`를 **잡는다**(`long`이면 둘 다 통과). `SchemaVersion`(1..2147483647)→`int`, `Tick`/`NonNegativeSafeInteger`/`SafeInteger`/`MoneyMinor`→`long`.

**`Required` 매핑(널 가능 처리).**

| 스키마 | `[JsonProperty]` |
|--------|------------------|
| `required`에 있고 널 가능 아님 | `Required = Required.Always` |
| `required`에 있고 `anyOf[..., null]` | `Required = Required.AllowNull` |
| `required`에 없음(payload 선택 필드) | `Required = Required.Default`, `NullValueHandling = Ignore` |

`Required.AllowNull`이 envelope 규칙(§1)을 코드로 표현하는 유일한 지점이다. 널 가능 필드에 `Required.Always`를 주면 **유효 fixture**(`null-client-time-max-seq.json`)가 거부된다(client 검토에서 실행 확인).

**그 외 생성 규칙.**
- 모든 속성에 `[JsonProperty("snake_case", Required = ...)]`를 명시한다. 이름 변환 규칙에 의존하지 않는다.
- 생성 파일은 `#nullable disable`로 시작한다. 참조 타입의 널 가능성은 `Required.AllowNull`로만 표현하고 `string?`을 쓰지 않는다. 값 타입은 `Guid?`/`long?`. (`#nullable enable`이면 초기화되지 않은 참조 속성마다 `CS8618`이 뜨고 — client 프로브에서 9건 — "신규 경고 0" 기준이 바로 깨진다.)
- 레지스트리 이름 → DTO 타입 대응표(`ContractTypes.ByName`)를 함께 생성한다.
- 출력은 결정적(정렬 고정, LF, 타임스탬프·도구 버전 미출력)이다.
- `--check`는 **내용과 파일 집합을 모두** 비교한다. 파일 집합을 안 보면 타입 이름이 바뀐 뒤 옛 `.cs`가 남아도 통과하고, 그 고아 파일은 컴파일되므로 아무도 모른다.
- 생성기는 `.meta` 파일을 만들거나 지우지 않는다(Unity가 관리). 파일을 지울 때는 `.cs`만 지운다.

**C# 직렬화 프로필 2종.** 하나로 두면 규칙이 충돌한다. `event-contracts` 규약은 "선택 필드 추가는 같은 `schema_version`"(호환 변경)인데, `MissingMemberHandling.Error`가 런타임 기본이면 서버가 선택 필드를 추가하는 순간 구버전 클라이언트가 그 메시지를 통째로 예외로 떨군다 — 호환 변경이 비호환이 된다.

| 프로필 | 설정 | 용도 |
|--------|------|------|
| `Strict` | `DateParseHandling.None` + `MissingMemberHandling.Error` | 테스트·개발 빌드. 계약 위반을 **드러내는** 용도 |
| `Runtime` | `DateParseHandling.None` + `MissingMemberHandling.Ignore` + 무시한 멤버를 경고 로그로 | 실제 수신 경로(p0-02 전송 계층부터). 서버가 먼저 배포될 수 있다 |

p0-01에서는 수신 경로가 없으므로 **코드로는 `Strict`만 만든다**. 이 표는 p0-02가 기본값을 다시 논쟁하지 않게 지금 박아 두는 것이다. 비대칭은 의도적이다: **서버는 명령에 엄격**(`deny_unknown_fields`, 클라이언트를 믿지 않는다), **클라이언트는 서버 메시지에 관대**(모르는 필드·타입은 경고 후 계속).

**`DateParseHandling.None`은 디스패치 경로에서도 필수다.** 서버 메시지를 `JObject`로 먼저 열어 `message_type`을 보고 디스패치할 때, 기본 설정 리더는 ISO 문자열을 `DateTime`으로 바꾼 뒤 문자열 속성에 되돌려 넣으며 형식을 바꾼다(근거: 검증한 사실 4).

### 5. ID·시간·화폐 표기

| 값 | 규칙 |
|----|------|
| 식별자 | UUIDv7(RFC 9562), 소문자 하이픈 표기. `event_id`, `message_id`, `world_id`, 엔티티 ID는 서버 생성. `command_id`는 클라이언트 생성 |
| `command_id` | 클라이언트가 v7로 만든다. 재시도는 **같은 `command_id`**. 서버는 멱등 키로만 쓰고 **내장 타임스탬프를 신뢰·해석하지 않는다**. v7을 요구하는 이유는 멱등성 테이블 인덱스 지역성 |
| C#에서 UUIDv7 생성 | 48비트 밀리초 + 버전 니블 7 + variant `10xx` + CSPRNG 나머지를 **정규 문자열로 조립해 `new Guid(string)`**. `new Guid(byte[])`는 앞 3개 그룹을 리틀엔디언으로 읽어 **버전 니블이 `7`→`b`로 깨지는데 variant는 살아남는다**(client가 재현). 난수는 `RandomNumberGenerator`. .NET 9+의 `Guid.CreateVersion7()`은 생성기(.NET 10)에는 있고 Unity(netstandard2.1)에는 없다 |
| 결정성 | 결정적 코어(sim·history) 안에서 ID를 직접 만들지 않는다. ID는 주입된 생성기로만 얻고, 재생·결정성 비교는 ID가 아니라 `(world_id, tick, sequence)` + 내용으로 한다. 역사 이벤트의 멱등 키는 역사 파이프라인 슬라이스의 ADR에서 정한다 |
| 게임 시간 | 정본은 `tick`(정수). `occurred_at`은 tick에서 결정적으로 파생한 **게임 달력 문자열**(`3827-04-13T18:32:11Z`). 표시·역사 문장용이며 실제 시간으로 파싱하거나 정렬에 쓰지 않는다. tick↔달력 변환 규칙은 tick ADR에서 |
| 실제 시간 | `recorded_at`, `client_sent_at`은 RFC 3339 UTC(`Z` 필수, 소수점 1–9자리 선택). 감사·진단 전용 |
| 화폐 | 정수 최소 단위(`MoneyMinor`). 실수 화폐 필드는 계약 리뷰에서 거부한다 |
| 정수 범위 | 계약의 모든 정수는 ±(2^53−1) 안이고 `minimum`·`maximum`을 선언한다. 어떤 JSON 소비자(JS·Python 등)도 정확히 표현할 수 있어야 로그·대시보드에서 값이 조용히 바뀌지 않는다 |

### 6. 부트스트랩에서 실행할 명령

```bash
# C# DTO 생성 (client, 레포 루트에서)
dotnet run tools/codegen/ContractsCodegen.cs -- --contracts contracts --out client/Assets/_Project/Scripts/Contracts/Generated
dotnet run tools/codegen/ContractsCodegen.cs -- --contracts contracts --out client/Assets/_Project/Scripts/Contracts/Generated --check

# Rust 계약 테스트 (server) — 워크스페이스 루트는 server/ 다
cd server && cargo test -p starfall-contracts --locked

# 레지스트리 커버리지 (qa, 레포 루트에서)
python .claude/skills/integration-qa/scripts/check_contract_coverage.py --strict
```

## 검증한 사실

**architect (2026-09-17, 이 PC)**

1. **Rust `jsonschema` 0.56.0**(`default-features = false`) + 미리 등록한 레지스트리로 다중 파일 2020-12 스키마를 네트워크 없이 검증. 유효 fixture 통과, `invalid/` 전부 거부, 메타스키마 검증 통과. (상한 위반 fixture 2건 추가 후 재실행에서도 7건 전부 거부 — 독립 검증기(Python `jsonschema`)로 확인.)
2. **quicktype 26.0.0**: `allOf`로 합쳐지는 envelope 필드를 **조용히 전부 버렸다**(`command_id`, `client_sent_at`, `message_id`, `tick`, `correlation_id` 없음). 에러도 경고도 없다 → 탈락.
3. **NJsonSchema 11.6.1**: 외부 파일 안의 `#/$defs/...`와 `$defs` 안에서의 파일 간 `$ref`를 해석하지 못했다 → 탈락.
4. **Newtonsoft 13.0.2 날짜 함정**: `JObject.Parse(json).ToObject<T>()` 기본 설정에서 `"2026-09-17T14:05:09.123Z"` → `"09/17/2026 14:05:09"`(형식 변경 + 밀리초 손실). `DateParseHandling.None` 리더면 보존. `JsonConvert.DeserializeObject<T>` 직접 경로에서는 재현되지 않는 **디스패치 전용 함정**.
5. **자체 생성기 프로토타입**(약 150줄, .NET 10 단일 파일 앱): 클래스 4개 + 대응표 생성 → `netstandard2.1`/`LangVersion 9.0` 경고 0·오류 0 → 유효 fixture 왕복 `JToken.DeepEquals` 일치 → `MissingMemberHandling.Error`로 미지 필드 거부.
6. Unity 6000.6 매뉴얼: C# 언어 버전 **9.0**(Roslyn). `record`/`init` 제한 → 생성 코드는 일반 속성만.
7. `com.unity.nuget.newtonsoft-json` 최신 **3.2.2**(Newtonsoft 13.0.2 동기화).
8. 참고 버전: `serde` 1.0.229, `serde_json` 1.0.151, `uuid` 1.26.1(`v7`·`serde`), `typify` 0.8.0, `schemars` 1.2.2.

**client 검토 (2026-09-18, 독립 재현)**

9. 유효 fixture 4건 왕복 `JToken.DeepEquals` 일치, `invalid/` 중 3건 거부·2건 통과(스펙 §5 표와 일치)를 **실제 `contracts/fixtures/**`로** 재현.
10. 널 가능 필드에 `Required.Always`를 주면 **유효 fixture가 거부**된다(`Required property 'client_sent_at' expects a value but got null`) → §4 `Required` 표의 근거.
11. `probe_seq`를 `uint`로 매핑하면 `probe-seq-negative`가 거부된다(`Error converting value -1 to type 'System.UInt32'`) → §4 범위 기반 정수 매핑의 근거.
12. `#nullable enable` 상태에서 생성 스타일 코드는 `CS8618` 9건 → §4 `#nullable disable` 고정의 근거.
13. Newtonsoft 패키지의 DLL 2종 모두 `UnityEngine` 참조 없음(=`noEngineReferences`와 양립), 둘 다 `isExplicitlyReferenced: 0`(=`overrideReferences` 없이는 자동 참조로 우연히 동작).
14. `Guid` 엔디언 함정 재현: 빅엔디언 바이트를 `new Guid(byte[])`에 넘기면 `019957fe-5283-7abc-…` → `fe579901-8352-bc7a-…`(버전 니블 깨짐, variant는 유지).

**server 검토 (2026-09-18)**

15. `jsonschema`의 `Registry::new().add(...).prepare()` + `options().with_registry(&r).offline()` 형태가 문서상 정확히 지원됨을 확인. `offline()`은 0.52.0 추가, MSRV 1.85(우리는 1.98.1).
16. 현재 스키마 7개의 `$id`가 전부 경로 규칙과 일치하고 상대 `$ref`가 올바른 대상을 가리킴을 손으로 확인.
17. **(구현 중 재현, 2026-09-18)** `jsonschema` 0.56의 `unevaluatedProperties`가 `allOf` 애너테이션을 수집해 `actor-field-injected`를 거부함을 Rust에서 직접 확인했다. 이것이 **명령 envelope에 행위자 필드를 넣지 못하게 막는 유일한 스키마 메커니즘**이고(§1), 여기에 계약 작성 스타일 전체가 걸려 있었다. 이제 미확인 항목이 아니다.

## 검토한 대안과 버린 이유

- **typify로 Rust 타입 생성**: 공식 README가 "work in progress"이며 정수 `minimum`/`maximum`을 타입으로 강제하지 못한다고 명시한다. 우리 계약은 범위가 곧 의미인 정수가 핵심이다.
- **schemars로 Rust 타입에서 스키마 생성**: 계약의 진실이 Rust 코드가 되어 클라이언트가 서버 구현에 종속된다.
- **스키마를 단일 파일로 펴서 NJsonSchema 사용**: 도구 한계가 계약 작성 스타일을 지배한다.
- **C# DTO도 손으로 작성**: Unity 쪽에는 2020-12 검증기가 없어 드리프트를 fixture 테스트로만 잡게 된다. 생성이 더 싸다.
- **정수 매핑을 `format`으로 유지**(현행 유지안): 공짜로 얻을 수 있는 방어(C#이 범위 위반을 거부)를 버린다. 두 검토자가 같은 모순(`probe_seq`)을 지적했고, 범위 기반으로 바꾸면 `format`은 쓰이지 않으므로 계약에서 정수 `format`을 제거해 진실을 하나로 만든다.
- **`MissingMemberHandling.Error`를 런타임 기본으로**: "선택 필드 추가 = 호환 변경" 규칙과 충돌한다. 대안으로 "선택 필드 추가도 `schema_version`을 올린다"가 있으나 업캐스터 비용이 커서 버린다.
- **Protobuf/FlatBuffers 즉시 도입**: 측정 없는 바이너리 전환은 원칙 10 위반.

## 결과

- 좋은 점: 계약이 표준 2020-12로 남고, 드리프트가 세 층(스키마·serde·DTO)에서 각각 테스트로 잡힌다. 생성기가 모르는 키워드에서 멈추므로 "조용히 사라진 필드"가 구조적으로 불가능하다. 범위 기반 정수 매핑으로 C#도 범위 위반을 거부한다.
- 감수할 점: 생성기를 우리가 유지한다(프로토타입 기준 약 150줄, 규칙 추가로 더 커진다). 스키마에 새 키워드를 쓰려면 생성기를 먼저 확장해야 한다 — 계약 어휘를 좁게 유지하는 효과도 있다. Rust는 타입마다 envelope 필드를 펼쳐 써야 한다(`flatten` 금지의 대가).
- 다시 검토할 조건: (a) 계약 타입 50개 초과 또는 생성기 300줄 초과 시 성숙한 도구 재평가, (b) 실시간 메시지 대역폭이 측정된 병목이면 바이너리 포맷 ADR, (c) 첫 비호환 스키마 변경 때 구버전 스키마·fixture 보존 규칙 ADR, (d) 첫 payload 선택 필드가 들어올 때 Rust `skip_serializing_if` 허용 범위 확정.

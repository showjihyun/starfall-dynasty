# client의 ADR 검토 (p0-01-bootstrap)

- 검토자: client (unity-client-engineer), 서브 에이전트 모드
- 날짜: 2026-09-18
- 검토 대상: ADR-0001, ADR-0002, ADR-0003, `docs/specs/p0-01-bootstrap.md`, `_workspace/p0-01-bootstrap/01_architect_tasks.md`, `contracts/**`
- 이번 턴에 코드·프로젝트는 만들지 않았다. 아래 "검증한 사실"은 스크래치패드 프로브와 CLI 조회로 **실제 실행**해 얻은 것이다.

## architect 질의 5건에 대한 답 (요약)

| # | 질문 | 답 |
|---|------|----|
| 1 | 자체 생성기 방침에 동의하는가 | **동의**. 다만 ADR-0002 §4에 빠진 규칙 4개(Required 매핑, RealTime/GameTime→string 고정, 키워드 allowlist 3분류, nullable 컨텍스트)를 명문화해야 유지 비용이 예측 가능해진다. 상세: 수정 C-1~C-4 |
| 2 | `noEngineReferences: true`가 Newtonsoft와 맞물리는가 | **맞물린다(검증함)**. Newtonsoft DLL 2종 모두 `UnityEngine` 참조가 없다. 단 `precompiledReferences`는 **`overrideReferences: true`가 있어야 효력이 있다** — ADR-0001 §3과 T5 지시에 이 필드가 빠져 있다. 그리고 `noEngineReferences`의 부작용(IL2CPP 스트리핑 시 `[Preserve]` 불가)이 언급되지 않았다. 상세: 수정 A-1, A-2, A-3 |
| 3 | `unity projects new`가 git/Unity Cloud를 건드리는가 | **건드리지 않는다(확인함)**. `unity projects new --help`에 git·cloud 옵션이 **하나도 없다**. 그 옵션들은 `unity projects create`에만 있다(`--vcs`, `--git-*`, `--cloud`, `--cloud-org`). URP 템플릿 tarball에도 `.git*`·cloud 파일이 없다. 다만 T5 지시문의 "**Hub 등록 포함**"은 근거가 없다(도움말상 등록은 `create`의 기능). 상세: 수정 E-1 |
| 4 | UUIDv7 `new Guid(byte[])` 엔디언 함정 | **실재한다(재현함)**. 빅엔디언 바이트를 그대로 넘기면 버전 니블이 `7`→`b`로 깨지고 타임스탬프 순서가 뒤집힌다. 대응은 "정규 문자열을 만들어 `new Guid(string)`"을 1순위로 고정하자. 상세: 수정 D-1 |
| 5 | 6000.6.1f1로 시작해도 되는가 | **T6은 지금 시작, T5·T7은 결정 대기**. 이번 슬라이스의 클라이언트 산출물에는 버전 의존 코드가 없어 나중에 갈아타는 비용이 거의 0이다. 다만 **AC-8이 버전을 고정 증거로 요구**하므로 "사용자가 Q1에 답한 뒤 T5 실행"이 맞다. 상세: 수정 E-3 |

---

## 동의하는 결정

**ADR-0001**

- 모노레포 + `contracts/`가 서버·클라이언트와 같은 커밋에서 바뀌는 구조. 클라이언트 입장에서 계약 드리프트를 커밋 단위로 잡는 것이 fixture 테스트보다 먼저 오는 방어선이다.
- `client/`를 레포 루트로 두지 않는 결정. `server/`·`contracts/`가 Unity 에셋 스캔에 들어가면 `.meta` 오염과 임포트 지연이 즉시 생긴다. 실무적으로 옳다.
- 부트스트랩에서 asmdef를 2개만 만드는 결정. 빈 asmdef는 의존 방향을 증명하지 못하고 임포트 시간만 늘린다. `Core/Net/World/Gameplay/History/UI`는 이름·경로만 예약하는 것으로 충분하다.
- `tools/codegen/**`의 소유자를 client로 둔 것. 출력물이 컴파일되는 곳이 client이므로 깨졌을 때 고칠 사람이 소유해야 한다.
- `client/Assets/_Project/{Art,Shaders,VFX,Rendering}`과 `client/Assets/Settings/**`를 techart 소유로 분리한 것. 템플릿이 만드는 URP 에셋이 `Assets/Settings/`에 들어가는 것을 확인했다(템플릿 tarball에 `ProjectData~/Assets/Settings/` 존재). 경계가 실제 파일 배치와 일치한다.

**ADR-0002**

- JSON Schema 2020-12, 파일 하나에 타입 하나, `$id`에 `.invalid` TLD, 상대 `$ref` + 오프라인 사전 등록. 생성기를 직접 만드는 입장에서 파일 간 `$ref`가 표준대로 유지되는 편이 훨씬 다루기 쉽다.
- **자체 생성기 방침에 동의한다.** quicktype이 `allOf` envelope 필드를 조용히 버린 사례는 클라이언트에게 최악의 실패 모드다 — 컴파일도 되고 테스트도 통과하는데 `command_id`가 없는 DTO가 나온다. "모르는 키워드에서 실패"는 이 실패 모드를 구조적으로 없앤다. 장기 유지 비용도 우려하지 않는다: 계약 어휘가 좁게 유지되는 한 생성기는 스키마 키워드 수에 비례해 커지지, 타입 수에 비례해 커지지 않는다. 재검토 조건(타입 50개 / 생성기 300줄)도 적절하다.
- envelope 필드는 항상 존재하고 값이 없으면 `null`(I-5). C#에서 이 규칙이 `Required.AllowNull` 하나로 그대로 표현된다 — 검증했다.
- 명령 envelope에 `player_id`/`actor_id`를 두지 않는 것. 클라이언트가 행위자를 주장할 자리를 아예 없애는 게 맞다.
- 생성물을 커밋하고 `--check`로 무결성을 지키는 것(I-2). 클라이언트 빌드가 .NET SDK 없이도 되어야 한다는 전제에 동의한다.
- 정수를 ±(2^53−1)로 제한. C# `long`으로 안전하고, `tick: 9007199254740991` fixture가 정확히 왕복하는 것을 확인했다.
- `DateParseHandling.None` + 계약 전용 시리얼라이저 고정. ADR의 "검증한 사실 4"를 그대로 재현했다(아래 증거).

**ADR-0003**

- Rust 툴체인 패치 고정, docker compose 인프라만, 포트 15432/16379, Redis 영속화 끄기, `.env` 미커밋. 클라이언트 작업과 충돌 지점이 없고 원칙 3·7과 일치한다.
- CI를 이번 슬라이스에서 하지 않는 것. Unity 라이선스를 러너에 올리는 일은 그 자체로 한 슬라이스다.
- `.gitattributes`에 `* text=auto eol=lf`. 이게 없으면 `codegen --check`(I-2)가 클론 직후 모든 사람에게서 실패한다 — 아래 C-5에서 범위를 구체화해 달라고 요청한다.

**스펙**

- I-1~I-8 불변식, 특히 I-4(검증기가 살아 있어야 한다)와 §5의 "invalid fixture별 거부 책임" 표. 표가 "C#은 이건 못 잡는다"를 미리 못박아서, QA가 나중에 "왜 C#은 통과했나"로 시간을 쓰지 않게 한다. 이 표의 내용이 실제로 맞다는 것을 5건 전부 실행으로 확인했다.
- `contracts/api/` 승격 규칙(`/healthz`·`/readyz`는 클라이언트가 소비하지 않으므로 계약 아님). 동의한다. 클라이언트는 이번 슬라이스에서 HTTP를 전혀 호출하지 않는다.

---

## 수정 요청 (항목별: 무엇을 / 왜 / 대안)

### A. asmdef — `noEngineReferences` 관련 (ADR-0001 §3, T5)

**A-1 (차단급). `overrideReferences: true`를 함께 적어야 한다.**

- **무엇을**: ADR-0001 §3 표와 T5 지시의 `Starfall.Contracts` 항목을 "`noEngineReferences: true`, `overrideReferences: true`, `precompiledReferences: ["Newtonsoft.Json.dll"]`"로 고친다. `Starfall.Tests.EditMode`도 같은 규칙이 필요하다.
- **왜**: `precompiledReferences`는 `overrideReferences`(Editor UI의 "Override References")가 켜져 있을 때만 읽힌다. 꺼져 있으면 Unity는 **Auto Referenced DLL 전부**를 자동 참조하고 `precompiledReferences`는 무시된다. 즉 현재 문구대로 쓰면 "Newtonsoft를 명시적으로 참조했다"고 생각하지만 실제로는 자동 참조로 우연히 동작하고, 나중에 누군가 Override References를 켜는 순간 조용히 깨진다.
- 확인한 사실: `com.unity.nuget.newtonsoft-json@3.2.2`의 `Runtime/Newtonsoft.Json.dll.meta`와 `Runtime/AOT/Newtonsoft.Json.dll.meta` 둘 다 `isExplicitlyReferenced: 0` → **Auto Referenced 상태**다. 그래서 `overrideReferences`를 빼도 "일단 컴파일은 된다". 이게 함정이다.
- **대안**: 없음(고쳐야 한다). 테스트 어셈블리의 `precompiledReferences`에는 `nunit.framework.dll`도 반드시 함께 넣는다 — 이것이 빠지면 `overrideReferences: true` 순간 NUnit이 사라져 테스트 어셈블리가 컴파일되지 않는다.
- 참고로 제안하는 asmdef 2종의 확정 형태(실행은 T5에서):

  ```jsonc
  // Assets/_Project/Scripts/Contracts/Starfall.Contracts.asmdef
  { "name": "Starfall.Contracts", "rootNamespace": "Starfall.Contracts",
    "references": [], "includePlatforms": [], "excludePlatforms": [],
    "allowUnsafeCode": false, "overrideReferences": true,
    "precompiledReferences": ["Newtonsoft.Json.dll"],
    "autoReferenced": true, "defineConstraints": [], "versionDefines": [],
    "noEngineReferences": true }

  // Assets/_Project/Tests/EditMode/Starfall.Tests.EditMode.asmdef
  { "name": "Starfall.Tests.EditMode", "rootNamespace": "Starfall.Tests.EditMode",
    "references": ["Starfall.Contracts", "UnityEngine.TestRunner", "UnityEditor.TestRunner"],
    "includePlatforms": ["Editor"], "excludePlatforms": [],
    "allowUnsafeCode": false, "overrideReferences": true,
    "precompiledReferences": ["nunit.framework.dll", "Newtonsoft.Json.dll"],
    "autoReferenced": false, "defineConstraints": ["UNITY_INCLUDE_TESTS"],
    "versionDefines": [], "noEngineReferences": false }
  ```

**A-2 (수정 요청). `noEngineReferences`의 근거 문구를 바꾸자.**

- **무엇을**: ADR-0001 §3의 "엔진 없이 컴파일되게 두어 나중에 CI에서 Unity 없이도 계약 테스트를 돌릴 여지를 남긴다"를 "생성된 DTO에 엔진 타입(`Vector3`, `Debug`, `Application` 등)이 섞여 들어가는 것을 컴파일러가 막게 한다"로 바꾼다.
- **왜**: `noEngineReferences: true`만으로는 Unity 없이 컴파일되지 않는다. asmdef는 Unity 컴파일 파이프라인 전용 파일이고, 밖에서 돌리려면 별도 csproj/MSBuild와 Newtonsoft NuGet 참조가 따로 필요하다. ADR이 얻지 못하는 이득을 근거로 적으면, 나중에 "CI에서 왜 안 되지"로 시간을 쓴다. **실제 이득(엔진 오염 차단)은 충분히 크므로 결정 자체는 유지**하자는 뜻이다.
- **대안(권장)**: 그 "Unity 없는 계약 테스트"를 진짜로 원하면 T6에 `tools/codegen/verify/`(client 소유)로 `netstandard2.1` 타깃 csproj를 하나 추가해 생성물 + Newtonsoft 13.0.2를 컴파일한다. 그러면 (a) Unity에 임포트하기 **전에** 컴파일 에러를 잡아 Safe Mode를 예방하고, (b) 나중에 CI에서 Unity 없이 돌릴 대상이 실제로 생긴다. 원한다면 이번 슬라이스에서 넣겠다(내 소유 경로).

**A-3 (기록만, 이번 슬라이스 작업 아님). IL2CPP 스트리핑 대비를 ADR "결과"에 한 줄 남기자.**

- **무엇을**: ADR-0001 §3 또는 ADR-0002 §4에 "`noEngineReferences: true`인 `Starfall.Contracts`는 `UnityEngine.Scripting.PreserveAttribute`를 쓸 수 없으므로, 플레이어 빌드 스트리핑 대비는 `link.xml` 또는 자체 `PreserveAttribute`로 한다"를 남긴다.
- **왜**: 생성 DTO의 속성은 Newtonsoft가 리플렉션으로만 접근한다. IL2CPP Managed Stripping이 Medium 이상이면 스트립 대상이 될 수 있고, 증상은 "에디터에서는 되는데 빌드에서만 필드가 기본값" — 추적이 가장 비싼 부류다. 패키지가 넣어 주는 `link.xml`은 `System.ComponentModel` 컨버터만 보존하지 우리 DTO는 보존하지 않는다(패키지 tarball에서 확인).
- **대안**: (a) `Assets/link.xml`에 `<assembly fullname="Starfall.Contracts" preserve="all"/>`, (b) `Starfall.Contracts` 안에 `PreserveAttribute`를 직접 선언(Unity 링커는 네임스페이스와 무관하게 이름이 `PreserveAttribute`인 어트리뷰트를 인식한다)하고 생성기가 DTO에 붙인다. (a)가 이번엔 더 싸다. **실제 빌드가 생기는 슬라이스에서 하면 된다 — 지금은 ADR에 한 줄만.**

### B. 런타임 관용성 (ADR-0002 §4, 스펙 §5)

**B-1 (수정 요청). `MissingMemberHandling.Error`를 런타임 전역 기본값으로 두면 안 된다.**

- **무엇을**: ADR-0002 §4의 "계약 전용 시리얼라이저" 항목을 **두 프로필**로 나눈다.
  - `ContractJson.Strict` — `DateParseHandling.None` + `MissingMemberHandling.Error`. **테스트와 개발 빌드 전용.** AC-7(b)가 요구하는 거부는 이걸로 증명한다.
  - `ContractJson.Runtime` — `DateParseHandling.None` + `MissingMemberHandling.Ignore` + 무시한 멤버 수를 경고 로그로 남김. **실제 수신 경로 기본값.**
- **왜**: ADR-0002 §2/`event-contracts` 스킬이 "선택 필드 추가는 같은 `schema_version`"으로 정했고, `unity-client` 스킬은 "서버가 먼저 배포될 수 있으므로 모르는 것은 경고 후 계속"으로 정했다. 그런데 `MissingMemberHandling.Error`가 런타임 기본이면, 서버가 payload에 선택 필드 하나를 **호환 변경으로** 추가하는 순간 구버전 클라이언트가 그 메시지를 통째로 예외로 떨군다. 즉 "호환 변경"이 클라이언트에서 비호환이 된다. 두 규칙이 현재 문구에서 정면으로 충돌한다.
- **대안**: (i) 위의 2프로필(권장), (ii) "payload 선택 필드 추가도 `schema_version`을 올린다"로 ADR-0002 §2를 바꾸기 — 이건 업캐스터 비용이 커서 반대한다. (iii) 현 상태 유지 — 반대한다.
- 이번 슬라이스 영향: 수신 경로가 아직 없으므로 **코드상으로는 `Strict` 하나만 만들면 된다.** 요청은 "ADR 문구에 런타임 프로필이 다르다는 것을 지금 박아 두자"는 것이다. p0-02에서 전송 계층을 만들 때 기본값이 정해져 있어야 한다.

### C. 생성기 규칙의 빈칸 (ADR-0002 §4, T6)

현재 §4로는 아래 4가지가 구현자 재량으로 남는데, 넷 다 "틀린 선택을 하면 유효 fixture가 깨지거나 AC-8이 깨지는" 항목이다. ADR에 명문화해 달라.

**C-1 (차단급). `required` × 널 가능 → `Required` 매핑 표.**

- **무엇을**: 아래 3행을 ADR-0002 §4에 추가.

  | 스키마 | `[JsonProperty]` |
  |--------|------------------|
  | `required`에 있고 `anyOf[..., null]` 아님 | `Required = Required.Always` |
  | `required`에 있고 `anyOf[..., null]` 있음 | `Required = Required.AllowNull` |
  | `required`에 없음 (payload 선택 필드) | `Required = Required.Default` |

- **왜**: 실행으로 확인했다 — `client_sent_at`에 `Required.Always`를 주면 **유효 fixture인** `PING_SERVER/null-client-time-max-seq.json`이 거부된다(`Required property 'client_sent_at' expects a value but got null`). §4의 현재 문구("널 가능 → `T?`/nullable 참조")는 C# 타입만 말하고 `Required`를 말하지 않아서, 구현자가 "required니까 Always"로 가면 바로 빨간 테스트가 된다. I-5를 코드로 표현하는 유일한 지점이 `Required.AllowNull`이다.
- **대안**: 없음.

**C-2 (차단급). `RealTime`/`GameTime`은 항상 C# `string`으로 매핑한다고 못박기.**

- **무엇을**: §4 매핑 목록에 "`$ref`가 `RealTime`·`GameTime`인 필드는 `string`. `DateTime`/`DateTimeOffset`으로 매핑하지 않는다"를 추가.
- **왜**: 두 primitive에는 `format: date-time`이 없고 `pattern`만 있어서, 생성기가 "문자열"로 가면 **우연히** 맞다. 하지만 ADR-0002 §5는 `occurred_at`을 "실제 시간으로 파싱하지 말 것"이라 못박았는데 이 요구가 생성기 규칙에는 적혀 있지 않다. 누군가 "가독성"을 이유로 `DateTimeOffset`을 넣으면 `GameTime`(3827년)이 파싱되어 게임 달력이 실제 시간 축에 올라탄다 — 원칙 위반이 타입 하나로 들어온다.
- **대안**: 없음. (이번 슬라이스 계약에는 `GameTime`이 없지만, `event-envelope`에 이미 정의되어 있으므로 도메인 이벤트가 들어오는 다음 슬라이스에 바로 걸린다.)

**C-3. 키워드 allowlist를 3분류로 명시.**

- **무엇을**: §4의 "모르는 키워드를 만나면 실패"를 다음처럼 구체화.
  - *구조 키워드(출력에 영향)*: `type`, `properties`, `required`, `$ref`, `$defs`, `allOf`, `anyOf`, `const`, `format`, `additionalProperties`, `unevaluatedProperties`, `items`, `enum`
  - *주석 키워드(안전하게 무시)*: `$schema`, `$id`, `title`, `description`, `examples`, `$comment`, `deprecated`
  - *제약 키워드(C# 출력에는 영향 없음, Rust 검증기가 강제)*: `minimum`, `maximum`, `pattern`, `minItems`, `uniqueItems`
  - 위 세 목록에 없는 키워드 → **JSON Pointer 경로와 함께 실패**
- **왜**: "모르는 키워드에서 실패"가 실제로 방어가 되려면 "아는 키워드"가 명시적 목록이어야 한다. 목록이 없으면 구현자가 "이건 무시해도 되겠지"를 즉흥 판단하게 되고, quicktype과 같은 실패 모드가 우리 생성기에서 재현된다. 특히 `minimum`/`maximum`을 "무시해도 되는 것"으로 두는 판단은 C-4와 맞물려야 한다.
- **대안**: 최소한 "무시 목록을 코드에 상수로 둔다"만이라도.

**C-4 (제안, 반대하면 그대로 가도 됨). 정수 매핑을 `format` 대신 선언된 `[minimum, maximum]` 범위로 결정.**

- **무엇을**: §4의 "`integer/format:int64` → `long`, `int32` → `int`"를 "선언된 `minimum`/`maximum`을 담는 가장 좁은 타입을 `int` → `uint` → `long` 순으로 고른다. 범위 선언이 없으면 `long`"으로 바꾼다.
- **왜**: `probe_seq`는 `format: int64`지만 실제 범위가 `0..4294967295`다. 현 규칙이면 `long`이 되고, 스펙 §5 표대로 C#은 `probe-seq-negative.json`을 못 잡는다. 범위 기반으로 고르면 `uint`가 되고 **잡는다**. 실행으로 확인했다: `long`이면 통과, `uint`면 `Error converting value -1 to type 'System.UInt32'`로 거부. 즉 C# 책임 invalid fixture가 3건 → 4건으로 늘고, 스펙 §5 표가 그만큼 정확해진다. 다른 primitive에도 안전하다: `SchemaVersion`(1..2147483647)→`int`, `Tick`/`NonNegativeSafeInteger`(0..2^53−1)→`long`, `SafeInteger`→`long`, `MoneyMinor`→`long`.
- **대안**: 현 규칙(전부 `long`) 유지 + 스펙 §5 표 그대로. 이것도 **틀리지 않다**(스키마 검증은 Rust가 한다). 다만 공짜로 얻을 수 있는 방어를 버리는 것이다. architect가 "생성기 규칙을 단순하게"를 우선하면 현 규칙으로 가도 이의 없다. 결정만 내려 주면 된다.
- 주의: `ulong`은 목록에 넣지 말자. 2^53−1 초과 값을 표현할 수 있게 되면 ADR-0002 §5의 "모든 정수는 ±2^53−1 안" 전제가 C# 쪽에서만 느슨해진다.

**C-5 (차단급, architect 소유 파일에 대한 요청). nullable 컨텍스트와 `.gitattributes` 범위.**

- **무엇을 (a)**: §4에 "생성 파일은 `#nullable disable`로 시작한다. 참조 타입의 널 가능성은 `Required.AllowNull`로만 표현하고 `string?`을 쓰지 않는다. 값 타입은 `Guid?`/`long?`을 쓴다"를 추가.
- **왜 (a)**: Unity 6 프로젝트는 nullable 컨텍스트가 꺼져 있다. 생성기가 `string?`을 내면 `CS8632`가, `#nullable enable`을 켜면 초기화되지 않은 참조 속성마다 `CS8618`이 뜬다. **내 프로브 컴파일에서 `CS8618`이 9건 실제로 발생했다**(출력 아래 증거). AC-8은 "신규 경고 0"을 요구하므로, 이 선택 하나로 AC-8이 바로 깨진다. `#nullable disable` 고정이 프로젝트 설정과 무관하게 결정적이다.
- **무엇을 (b)**: `.gitattributes`(architect 소유)에 `*.cs text eol=lf`와 `*.json text eol=lf`를 **반드시** 포함해 달라.
- **왜 (b)**: 이 PC의 git은 `core.autocrlf=true`다. 생성기가 LF로 쓰고 git이 체크아웃에서 CRLF로 바꾸면, 클론한 사람의 `--check`가 전원 실패한다(I-2가 거짓 양성으로 무력화). 이번 슬라이스는 커밋하지 않으니 당장 터지지 않지만, 첫 커밋 순간 터진다. fixture JSON도 같은 이유로 포함해야 Rust의 원본 비교가 안전하다.

### D. UUIDv7 헬퍼 (T7)

**D-1 (수정 요청). 1순위 구현 방식을 "정규 문자열 → `new Guid(string)`"으로 고정.**

- **무엇을**: T7 지시의 "`new Guid(byte[])`는 앞 3개 그룹을 리틀엔디언으로 읽는다. 바이트를 그대로 넘기면 깨진다"에 이어 **권장 구현을 명시**한다: 48비트 밀리초 + 버전 니블 7 + variant `10xx` + CSPRNG 나머지를 **16진 문자열로 조립해 `new Guid(string)`으로 파싱한다.** `new Guid(byte[])`를 쓰려면 앞 4·2·2바이트를 `Array.Reverse`한 뒤에만 쓴다.
- **왜**: 실행으로 재현했다.

  ```
  canonical string built from bytes : 019957fe-5283-7abc-8def-1a1b1c1d1e1f   UuidV7 pattern = MATCH
  new Guid(bigEndianBytes)          : fe579901-8352-bc7a-8def-1a1b1c1d1e1f   UuidV7 pattern = FAIL
  new Guid(swapped first 3 groups)  : 019957fe-5283-7abc-8def-1a1b1c1d1e1f   UuidV7 pattern = MATCH
  new Guid(canonicalString)         : 019957fe-5283-7abc-8def-1a1b1c1d1e1f   UuidV7 pattern = MATCH
  ```
  함정이 특히 고약한 이유: **variant 니블은 살아남는다**(byte 8은 스왑 대상이 아니라 `8def`가 그대로 남는다). 그래서 "variant는 맞는데 version만 틀린" ID가 나오고, 패턴 검사를 대충 짜면 통과한다. 위 예에서 14번째 문자가 `7`→`b`로 바뀌었다.
- **대안**: `Array.Reverse` 방식도 정답이다. 다만 문자열 방식이 (i) 엔디언 가정을 아예 없애고, (ii) `Convert.ToHexString`이 netstandard2.1에 없다는 점만 주의하면 되며(직접 hex 변환 필요), (iii) `Guid.ToByteArray()`도 같은 리틀엔디언 특성을 가지므로 바이트 왕복 테스트를 짤 필요가 없어진다. **참고**: .NET 9+에는 `Guid.CreateVersion7()`이 있지만(이 PC의 .NET 10.0.12에서 존재 확인) Unity의 netstandard2.1/Mono에는 **없다**. 생성기(tools/codegen, .NET 10)와 클라이언트(Unity)의 런타임이 다르다는 점을 헷갈리지 말자.
- **추가 요청**: T7 테스트 항목에 "같은 밀리초 안에서 N개를 연속 생성해도 패턴을 만족하고 중복이 없다"를 넣자. ADR-0002 §5가 v7을 요구하는 이유가 "멱등성 테이블 인덱스 지역성"이므로 엄격한 단조 증가까지는 요구하지 않아도 되지만, 중복은 막아야 한다. 난수는 `System.Random`이 아니라 `System.Security.Cryptography.RandomNumberGenerator`를 쓴다(netstandard2.1에서 사용 가능).

**D-2 (사소). 헬퍼의 소속 어셈블리를 정해 달라.**

- **무엇을**: `Starfall.Contracts`(손으로 쓴 파일, `Generated/` 밖) 안에 두는 것으로 확정. ADR-0001 §3 표에 "`Contracts` 어셈블리에는 `Generated/`(생성물)와 계약 전용 시리얼라이저·ID 헬퍼(손으로 쓴 파일)가 함께 산다"를 한 줄 덧붙여 달라.
- **왜**: 표가 지금은 "`Generated/`는 생성물"만 말해서, 손으로 쓴 `ContractJson.cs`/`UuidV7.cs`가 어디 소속인지 읽는 사람마다 달라진다. `Core` 어셈블리는 아직 없고, EditMode 테스트가 `Net`을 끌어오지 않고 ID 패턴을 검증하려면 `Contracts`에 있는 편이 맞다. `Core`가 생기는 슬라이스에서 옮길지 그때 정하면 된다.
- 주의: `noEngineReferences: true`이므로 이 헬퍼들은 `UnityEngine.Random`·`Debug`·`Application`을 쓸 수 없다. 의도된 제약이고 문제없다.

### E. 절차·도구 (T5, T7, ADR-0003)

**E-1. T5 지시문의 "Hub 등록 포함"은 근거가 없다.**

- **무엇을**: "생성(비대화형, Hub 등록 포함)"에서 "Hub 등록 포함"을 빼거나, 생성 후 `unity projects add C:/WorkSpace/SpaceHistoric/client`를 별도 단계로 적는다.
- **왜**: 도움말상 Hub 레지스트리 등록을 명시하는 것은 `unity projects create`("Create a new Unity project **and register it in the Hub**")이고, `new`는 "Create a new Unity project (non-interactive, CI-friendly)"로 등록을 말하지 않는다. 실제 등록 여부는 프로젝트를 만들어 봐야 알 수 있어 **미확인**이다. 등록이 안 되어도 `unity test client`·`unity open client`는 경로로 동작하므로 이번 슬라이스에 지장은 없다.
- **왜 `new`가 맞는지(확인함)**: `unity projects new --help`의 옵션은 `--path`, `--editor-version`, `--template`, `-a/--architecture`, `--open`이 전부다. git·cloud 옵션은 **하나도 없다**. 반면 `unity projects create`에는 `--cloud`, `--no-cloud`, `--cloud-org`, `--cloud-project`, `--vcs`, `--git-namespace`, `--git-repo`, `--git-visibility`, `--git-default-branch`, `--git-remote-protocol`, `--git-description`, `--git-token`, `--git-token-stdin`이 있다. 그리고 URP 템플릿 tarball 안에 `.git*`·cloud 관련 파일이 없음을 확인했다. **`projects new` 선택은 옳다.**

**E-2 (차단급). AC-7/T7의 `unity test` 명령이 지정한 경로에 리포트를 남기지 못한다.**

- **무엇을**: `unity test client --mode EditMode --report-format junit --junit-output <path>`를 **`--report-format both --output _workspace/p0-01-bootstrap/unity-tests/EditMode.nunit.xml --junit-output _workspace/p0-01-bootstrap/unity-tests/EditMode.xml`** 로 바꾼다. (JUnit 하나만 원하면 `--report-format junit --output _workspace/p0-01-bootstrap/unity-tests/EditMode.xml`.)
- **왜**: `unity test --help`(1.0.0-beta.8)의 `--junit-output` 설명이 "Path to write the JUnit report to **when both report formats are produced**"다. 즉 `--report-format junit`만 주면 `--junit-output`은 무시되고, 결과는 `--output` 기본값인 `./test-results.xml`로 간다. 현재 명령대로 돌리면 테스트는 통과하는데 **QA가 볼 증거 파일이 지정 경로에 없다.** `unity-client` 스킬 §7도 `--report-format junit --output ...`으로 되어 있어 스펙 쪽 표기와 어긋난다.
- **대안**: 없음(명령을 고쳐야 한다). 어느 쪽이든 T7 실행 후 실제 파일 존재를 확인해 `03_client_impl.md`에 적겠다.

**E-3. Q1(에디터 버전) — 내 권고.**

- **무엇을**: "T6은 Q1과 무관하게 지금 시작한다. T5·T7은 Q1 답변 후 시작한다"를 태스크 표의 선행 조건에 명시.
- **왜**: 이번 슬라이스의 클라이언트 코드는 순수 C#(DTO·시리얼라이저·UUID·파일 읽기)이라 6000.6과 6000.3 사이에 동작 차이가 없다. 씬·프리팹·아트가 하나도 없으므로 **나중에 버전을 바꿔도 비용은 "`ProjectVersion.txt` 교체 + 재임포트 + URP 패키지/에셋 버전 변경"뿐**이고, URP 에셋은 techart 소유라 내 작업물이 다시 깨질 일이 없다. 그런데 **AC-8이 "`ProjectVersion.txt`가 ADR-0003에서 확정한 버전과 같다"를 증거로 요구**하므로, 결정 전에 만들면 AC-8은 정의상 "미검증"이 된다. 결정이 하루 이상 걸린다면 6000.6.1f1로 진행하고 AC-8을 "잠정 버전 기준 PASS"로 표기하는 쪽을 추천한다 — 되돌리는 비용이 실제로 낮다.
- **(b) 선택 시 비용 정보**: `6000.3.24f1`을 고르면 에디터 본체 + **Web Build Support 모듈 재설치**가 필요하고(현재 6000.6.1f1에만 설치됨 — `unity editors --installed` 확인), URP 템플릿 버전도 달라진다(6000.6.1f1에 설치된 것은 `com.unity.template.urp-blank` 17.2.1 / URP 17.6.0). 다운로드 시간이 이 슬라이스의 최장 단일 작업이 될 가능성이 높다. ADR-0003 §2에 이 비용을 한 줄 적어 두면 사용자가 판단하기 쉽다.

**E-4. T5 패키지 지시가 템플릿 실상과 어긋난다.**

- **무엇을**: T5의 "Test Framework는 템플릿에 포함되어 있는지 확인하고 없으면 추가한다. Input System·Addressables·Cinemachine은 쓰는 슬라이스에서 추가한다"를 실측치로 대체한다.
- **왜(확인함)**: 설치된 `com.unity.template.urp-blank-17.2.1`의 `ProjectData~/Packages/manifest.json`은 다음과 같다.

  ```json
  { "dependencies": {
      "com.unity.ai.navigation": "2.0.12",
      "com.unity.collab-proxy": "2.12.4",
      "com.unity.ide.rider": "3.0.38",
      "com.unity.ide.visualstudio": "2.0.26",
      "com.unity.inputsystem": "1.19.0",
      "com.unity.render-pipelines.universal": "17.6.0",
      "com.unity.test-framework": "1.8.0",
      "com.unity.timeline": "6.6.0",
      "com.unity.ugui": "2.6.0",
      "com.unity.visualscripting": "1.9.11" } }
  ```
  - `com.unity.test-framework` **1.8.0 포함** → 추가 작업 없음.
  - `com.unity.inputsystem` **1.19.0 이미 포함** → "쓰는 슬라이스에서 추가"는 무의미하다(이미 있다).
  - Newtonsoft는 없다 → `com.unity.nuget.newtonsoft-json@3.2.2` 추가만 하면 된다. 3.2.2가 레지스트리 `latest`이고 Newtonsoft **13.0.2**에 동기화되어 있음을 확인했다(ADR-0002 "검증한 사실 7" 맞음).
- **제안(결정 요청)**: `com.unity.visualscripting`, `com.unity.timeline`, `com.unity.ai.navigation`, `com.unity.collab-proxy`는 이번 슬라이스는 물론 MVP 범위에서도 쓰지 않는다. 임포트 시간과 패키지 표면을 줄이려면 지금 빼는 게 가장 싸다. 특히 `com.unity.collab-proxy`(Unity Version Control)는 git을 쓰기로 한 결정과 중복된다. **다만 템플릿 기본 구성을 건드리는 것은 ADR 범위를 넘어 보여, architect 판단을 요청한다.** 뺄지 말지만 알려 주면 T5에서 처리하겠다. (보수적으로 가면 `collab-proxy`만 제거를 권한다.)
- **추가(확인함)**: 템플릿은 `ProjectData~/Assets/TutorialInfo/`(튜토리얼 Readme 에셋)와 `ProjectData~/Library/` 시드를 함께 넣는다. `TutorialInfo`는 삭제 대상으로 T5에 명시하자. 그리고 템플릿의 `ProjectSettings.asset`은 `companyName: Unity Technologies`, `productName: com.unity.template.urp-blank`, `applicationIdentifier:` 빈 값이다. **`unity projects new`에는 회사명·제품명 옵션이 없으므로**, Q5 값(`Starfall` / `Starfall Dynasty` / `com.starfall.dynasty`)은 생성 후 별도로 설정해야 한다. 생성 직후 실제 값이 무엇인지(폴더명으로 치환되는지)는 **미확인**이며, T5에서 확인해 보고하겠다.

**E-5. fixture 경로 해석을 마커 기반으로.**

- **무엇을**: T7의 `Path.Combine(Application.dataPath, "../../contracts/fixtures")`를 "`Application.dataPath`에서 위로 올라가며 `contracts/registry/types.json`이 있는 첫 디렉토리를 레포 루트로 삼는다. 못 찾으면 테스트를 실패시킨다(Skip/Ignore가 아니라 Fail)"로 바꾸자.
- **왜**: 경로 자체는 맞다(`client/Assets/../../` = 레포 루트, 확인함). 하지만 (i) 클라이언트 폴더가 한 단계 옮겨지면 테스트가 **조용히 0건 실행**으로 통과할 수 있고, (ii) `Application.dataPath`는 `UnityEngine` 참조가 필요해 `Starfall.Contracts`(`noEngineReferences: true`)에 둘 수 없으므로 헬퍼 위치가 테스트 어셈블리로 고정된다는 점이 문서에 없다.
- **대안**: 현 경로 유지 + "fixture 개수가 4개 미만이면 실패" 가드만 추가해도 (i)은 막힌다. 어느 쪽이든 "0건 통과"를 막는 가드는 반드시 넣겠다 — I-4와 같은 성격의 방어다.

### F. 계약 자체에 대한 의견 (변경 요청 아님, 기록)

- **F-1.** `PING_REPLY`에는 `RealTime`/`GameTime` 필드가 없다. 즉 AC-7(c)가 보려는 "JObject 디스패치 경로의 날짜 변환 함정"은 **현재 계약의 실제 디스패치 경로에서는 발생할 수 없다**(클라이언트가 디스패치하는 것은 서버 메시지뿐이고, `client_sent_at`은 클라이언트가 보내는 명령에만 있다). 그래도 테스트는 넣는 게 맞다 — `event-envelope`에 이미 `recorded_at`(RealTime)과 `occurred_at`(GameTime)이 있어 도메인 이벤트가 들어오는 순간 실제로 터진다. AC-7(c) 문구 제안은 아래 수용 기준 절에 적었다.
- **F-2.** `correlation_id` 설명에 "Reply matching uses explicit payload fields (e.g. command_id), not this field"라고 못박아 준 것이 클라이언트 구현에 매우 유용하다. 대기 중 명령 매칭을 `payload.command_id`로만 하겠다. 다만 fixture `PING_REPLY/basic.json`의 `correlation_id`가 `null`이고 `with-correlation.json`은 값이 있는데, **서버가 어느 쪽으로 보낼지는 p0-02에서 정해질 사항**이다. 클라이언트는 둘 다 처리한다.
- **F-3.** `probe_seq`가 `u32`인데 `format: int64`인 것은 Rust(`u32`)와 C#(`long` 또는 `uint`) 매핑이 갈리는 유일한 지점이다. AC-10의 "정수 타입이 3자 간 동일"이라는 문구가 여기서 반드시 걸린다 — 아래 수용 기준 절에서 문구를 제안한다.

---

## 수용 기준 검토 (증명 가능한가, 문구 제안)

client 담당 AC와, client가 증거를 대야 하는 AC만 본다.

| AC | 증명 가능한가 | 판단 |
|----|--------------|------|
| AC-6 (생성 결정성 + `--check`) | 가능 | 보강 필요 (아래 1) |
| AC-7 (EditMode 계약 테스트) | **현재 명령으로는 증거 파일이 안 남는다** | 수정 필요 (E-2, 아래 2) |
| AC-8 (프로젝트 열기, 경고 0, 버전 일치) | **현재 문구로는 불가능** | 수정 필요 (아래 3) |
| AC-9 (커버리지 `--strict`) | 가능 (client 선행 필요) | 문구 보강 (아래 4) |
| AC-10 (3자 경계면 비교) | 부분적으로 불가능 | 수정 필요 (아래 5) |
| AC-11 (저장소 상태) | 가능 | 보강 (아래 6) |

**1) AC-6 — `--check`가 "고아 파일"을 못 본다.**

현 문구는 "생성 파일 한 줄을 고치고 `--check` → 실패"만 요구한다. 이러면 스키마에서 타입 이름이 바뀌었을 때 옛 이름의 `.cs`가 `Generated/`에 남아도 `--check`가 통과한다. 남은 파일은 컴파일되므로 아무도 모른다.

> **제안 문구 (AC-6 뒤에 붙임)**: "When `Generated/`에 생성기가 만들지 않는 `.cs` 파일을 하나 추가하고 `--check`를 실행, Then 0이 아닌 종료 코드와 그 파일 경로가 출력된다. `--check`는 내용뿐 아니라 **파일 집합**을 비교한다."

또한 생성기는 `.meta`를 만들거나 지우지 않는다(Unity가 관리). 파일을 지울 때 `.cs`만 지우고 `.meta`는 Unity가 정리하도록 두겠다 — 이 점을 T6 지시에 한 줄 적어 주면 좋겠다.

**2) AC-7 — 명령 수정 + (c) 문구 조정.**

> **제안 문구 (명령)**: `unity test client --mode EditMode --report-format both --output _workspace/p0-01-bootstrap/unity-tests/EditMode.nunit.xml --junit-output _workspace/p0-01-bootstrap/unity-tests/EditMode.xml`, Then 종료 코드 0, 실패 0건, **두 리포트 파일이 실제로 존재한다**.

> **제안 문구 (c)**: "`RealTime` 값을 가진 계약 JSON(`PING_SERVER/basic.json`의 `client_sent_at`)을 `JObject` 디스패치 경로로 열었을 때 값이 `2026-09-17T14:05:09.123Z` 문자열 그대로 유지된다. 기본 설정 `JObject.Parse`가 같은 값을 `09/17/2026 14:05:09`로 바꾸는 것도 같은 테스트에서 함께 보여 회귀 가드로 남긴다."

이렇게 쓰는 이유는 F-1이다 — 현재 계약의 서버 메시지에는 날짜 필드가 없으므로, 테스트 대상은 "디스패치 헬퍼가 안전한 리더를 쓰는가"이지 "PING_REPLY가 깨지는가"가 아니다. 그리고 디스패치 헬퍼는 p0-02에서 실제 수신 경로가 쓸 **같은 함수**로 만들겠다. 그래야 가드가 진짜 코드 경로 위에 있다.

추가로 AC-7(b)의 "3건"은 C-4(범위 기반 정수 매핑)를 채택하면 **4건**이 된다(`probe-seq-negative.json` 포함). 채택 여부에 따라 스펙 §5 표의 "C#: 감지 불가 → 제외"도 함께 바뀐다.

**3) AC-8 — "클린 클론"은 I-8과 모순되고, "신규 경고 0"은 기준이 없다.**

- I-8이 "커밋하지 않는다"이므로 **클론할 대상이 없다.** 현 문구대로면 AC-8은 실행 불가다.
- "신규"의 기준선이 없다. 템플릿이 만든 에셋에서 나오는 경고까지 우리 책임이 되면 판정이 흔들린다.
- "지정된 에디터 버전으로 `client/`를 열기"는 대화형 동작이라 증거로 남기기 어렵다.

> **제안 문구 (AC-8 대체)**: "Given `client/Library/`를 삭제한 상태(콜드 임포트), When `unity test client --mode EditMode --report-format both --output ... --junit-output ...`, Then 종료 코드 0이고, Editor 로그에 컴파일 에러 0건이며 `Assets/_Project/**` 경로에서 발생한 컴파일 경고가 0건이다. 그리고 `client/ProjectSettings/ProjectVersion.txt`의 `m_EditorVersion`이 ADR-0003 §2에서 확정한 값과 문자열로 같다."

이러면 (i) 커밋 없이 실행 가능하고, (ii) 경고 범위가 우리 코드로 한정되며, (iii) 한 명령의 종료 코드로 증거가 남는다. 콜드 임포트 시간은 AC와 별개로 §8 요구대로 측정해 기록하겠다.

**4) AC-9 — 실행 순서를 못박아야 한다.**

커버리지 스크립트를 읽어 확인한 사실: `client` 태그의 소스 루트는 **`client/Assets`**, 확장자 `.cs`이고, `Library`·`PackageCache`·`Temp`·`obj` 등은 제외된다. 루트가 없으면 경고가 나고 `--strict`에서 실패한다. 매칭은 `PING_SERVER` 리터럴 또는 `PingServer`(+PascalCase 접미사)다. 생성 DTO가 `PingServerCommand` 클래스와 `"PING_SERVER"` 상수를 모두 내므로 통과한다.

> **제안 문구 (AC-9에 추가)**: "이 검사는 T5·T7이 끝나 `client/Assets/_Project/Scripts/Contracts/Generated/`에 생성물이 존재한 뒤에 실행한다. 그 전 실행 결과는 PASS/FAIL 판정에 쓰지 않는다."

**5) AC-10 — "정수 타입이 3자 간 동일"은 성립할 수 없다.**

`probe_seq`는 스키마 `integer/format:int64, 0..4294967295`, Rust는 T2 지시상 `u32`, C#은 `long`(또는 C-4 채택 시 `uint`)이다. 세 값이 "동일"한 경우가 없다. 이대로 두면 QA가 반드시 불일치 1건을 기록하거나, 반대로 기준을 느슨하게 해석해 검사가 무의미해진다.

> **제안 문구 (AC-10 대체)**: "Then 필드 이름·필수 여부·널 가능 여부가 3자 간 동일하고, 정수 필드는 **스키마가 선언한 `[minimum, maximum]`을 손실 없이 담는 타입**으로 매핑되어 있으며(타입 이름이 같을 필요는 없다), 범위 밖 값에 대해 최소한 한쪽(Rust 스키마 검증)이 거부한다. 불일치 0건이 리포트에 기록된다."

**6) AC-11 — 클라이언트가 만드는 무시 대상 보강.**

`.gitignore`는 architect 소유지만, client가 만드는 산출물 기준으로 다음이 필요하다: `client/Library/`, `client/Temp/`, `client/Logs/`, `client/obj/`, `client/UserSettings/`, `client/Builds/`, `client/*.csproj`, `client/*.sln`, `client/Assets/**/*.meta`는 **무시하면 안 된다**(반드시 추적 대상). 그리고 `tools/codegen/`의 `bin/`·`obj/`, .NET 단일 파일 앱이 만드는 아티팩트도 무시 대상이다.

> **제안 문구 (AC-11에 추가)**: "`client/Assets/_Project/Scripts/Contracts/Generated/**`의 `.cs`와 `.meta`가 추적 대상에 보이고, `client/Library/`·`client/Temp/`·`client/Logs/`·`client/obj/`·`tools/codegen/**/obj/`가 보이지 않는다."

**7) 스펙 §8(비기능) — 측정 항목 확정.**

"Unity EditMode 테스트 1회 시간"을 기록하라고 되어 있다. 콜드(Library 삭제 후)와 웜을 구분하지 않으면 다음 슬라이스와 비교가 안 된다.

> **제안 문구**: "콜드 임포트 포함 1회, 웜 상태 1회를 각각 기록한다."

---

## 내 태스크 실행 계획 요약 (순서와 예상 위험)

Q1(에디터 버전)·Q5(식별자) 답변 전에도 진행 가능한 것부터 배치했다.

| 순서 | 작업 | 선행 | 산출 경로 | 예상 시간 |
|------|------|------|----------|----------|
| 1 | **T6-a** 생성기 골격: 스키마 로더(오프라인 `$ref` 해석), 키워드 allowlist(C-3), 매핑 규칙(C-1·C-2·C-4·C-5a) | 없음 | `tools/codegen/ContractsCodegen.cs` | 중 |
| 2 | **T6-b** `--check` 모드(내용 + 파일 집합 비교), 결정적 출력(정렬·LF) | 1 | 위와 같음 | 소 |
| 3 | **T6-c** `netstandard2.1` 컴파일 검증 하네스(선택, A-2 대안) | 1 | `tools/codegen/verify/` | 소 |
| 4 | **T5** Unity 프로젝트 생성 → 패키지 고정(Newtonsoft 3.2.2 추가, E-4 결정 반영) → asmdef 2종(A-1 형태) → `TutorialInfo` 제거 → 식별자 설정 | **Q1, Q5** | `client/**` | 대 (임포트 대기) |
| 5 | **T7-a** DTO 생성 실행 → `Generated/` 커밋(파일 배치만, git 커밋 아님) | 2, 4 | `client/Assets/_Project/Scripts/Contracts/Generated/**` | 소 |
| 6 | **T7-b** `ContractJson`(Strict/Runtime 2프로필, B-1) + `UuidV7` 헬퍼(D-1) | 4 | `client/Assets/_Project/Scripts/Contracts/**` | 중 |
| 7 | **T7-c** EditMode 테스트: 유효 fixture 4건 왕복, C# 책임 invalid 3(또는 4)건 거부, 디스패치 날짜 가드, UUIDv7 패턴·중복, fixture 개수 가드 | 5, 6 | `client/Assets/_Project/Tests/EditMode/**` | 중 |
| 8 | **T7-d** `unity test`(E-2의 수정된 명령) 실행, 리포트 확인 | 7 | `_workspace/p0-01-bootstrap/unity-tests/` | 소 |
| 9 | **T8** 구현 요약 + 호출 API 목록(이번 슬라이스는 **없음**: 클라이언트가 호출하는 REST/WS 엔드포인트 0개) + 측정치 | 1–8 | `_workspace/p0-01-bootstrap/03_client_impl.md` | 소 |

순서 근거: 1–3을 먼저 하면 **Unity에 임포트하기 전에 생성물이 `netstandard2.1`에서 경고 0으로 컴파일되는지** 알 수 있다. 계약 DTO가 컴파일 에러를 내면 Editor가 Safe Mode로 뜨고 `unity test`·`unity command`가 전부 막히므로(스킬 §6), 이 순서가 가장 값싼 보험이다.

**예상 위험**

| 위험 | 영향 | 대응 |
|------|------|------|
| Q1 미결정으로 T5가 막힘 | 4–9 전체 지연 | 1–3을 먼저 완료. 하루 이상이면 6000.6.1f1로 진행하고 AC-8을 "잠정 버전 기준"으로 표기(E-3) |
| 생성물 컴파일 에러 → Editor Safe Mode | CLI 연결 차단, 복구에 시간 소모 | 3번 하네스로 선검증. Safe Mode에 들어가면 YAML 수동 편집 금지, 컴파일 에러부터 수정 |
| URP 템플릿 첫 임포트가 길다 | 4번이 최장 단일 작업 | 시간을 측정해 §8 기준선으로 기록. Windows Defender에서 `client/Library` 예외(ADR-0003 §5)는 사용자 동의가 필요하므로 요청만 남긴다 |
| `unity test`가 제 리포트를 안 남김 | AC-7 증거 부재 | E-2 명령으로 실행하고 파일 존재를 직접 확인 |
| `--check` 바이트 비교가 CRLF로 깨짐 | I-2 무력화 | C-5(b) `.gitattributes` 요청. 커밋 전까지는 디스크 기준이라 영향 없음 |
| Editor가 켜진 채 codegen이 파일을 쓰면 임포트 경합 | 간헐적 `.meta` 경고 | codegen 실행 시 Editor를 닫거나(`unity close client`), 실행 후 `unity command`로 refresh |
| C-4 미결정 상태로 T6을 짜면 나중에 매핑이 바뀜 | 생성물 전체 재생성 + 테스트 표 수정 | **architect 결정이 필요한 유일한 생성기 항목.** 결정 전까지는 `long`(현 ADR)으로 구현하고, 채택 시 상수 하나로 전환 가능하게 짜겠다 |

**계약 변경 요청은 없다.** `contracts/**`는 현 상태로 클라이언트 구현에 충분하다. F-1~F-3은 문서·매핑 규칙 문제이지 스키마 문제가 아니다.

---

## 미확인 사실

1. **asmdef `overrideReferences` 동작(6000.6.1f1 실측 아님).** Unity의 문서화된 동작과 패키지 메타(`isExplicitlyReferenced: 0`)에 근거한 판단이다. A-1의 asmdef 형태는 T5에서 실제 컴파일로 확인한 뒤 `03_client_impl.md`에 결과를 적겠다.
2. **`unity projects new`의 Hub 레지스트리 등록 여부.** 도움말에 언급이 없다. 프로젝트를 만들지 않았으므로 미확인(E-1).
3. **생성 직후 `ProjectSettings.asset`의 `companyName`/`productName` 실제 값.** 템플릿 원본은 `Unity Technologies` / `com.unity.template.urp-blank`이고 `unity projects new`에 해당 옵션이 없다. Hub가 폴더명으로 치환하는지 미확인(E-4).
4. **`com.unity.nuget.newtonsoft-json@3.2.2`가 6000.6.1f1에서 실제로 해석·임포트되는지.** 패키지 `unity` 최소 버전이 `2018.4`이고 상한 선언은 없음을 tarball의 `package.json`에서 확인했지만, 실제 임포트는 T5에서 확인한다.
5. **Unity 6000.6.1f1의 C# 언어 버전 9.0.** ADR-0002 "검증한 사실 6"의 매뉴얼 근거를 그대로 따랐다. 이번 턴에 직접 재확인하지 않았다. (내 프로브는 .NET 10 / C# 최신으로 돌았으므로 언어 버전 증거가 아니다. `#nullable` 관련 결론 C-5(a)는 언어 버전이 아니라 **nullable 컨텍스트 기본값**에 달린 것이므로 영향받지 않는다.)
6. **템플릿 목록의 불일치.** ADR-0003 표는 `com.unity.template.universal-2d` 7.0.0 / `get-started` 4.0.1을 적었으나, 6000.6.1f1의 `ProjectTemplates/` 디렉토리에 실제로 있는 tgz는 `com.unity.template.urp-blank-17.2.1`과 `com.unity.template.2d-cross-platform-2d-7.0.0` 둘뿐이다. `get-started`는 Hub 캐시 등 다른 경로에 있을 수 있다. 우리가 쓰는 `urp-blank`는 존재를 확인했으므로 **이번 작업에는 영향 없다.**
7. **다음 Unity LTS 시점.** ADR-0003이 이미 미확인으로 적었고, 나도 확인하지 못했다.

---

## 부록 — 이번 턴에 실행해 얻은 증거

환경: Windows 11, .NET SDK 10.0.401(런타임 10.0.12), Unity CLI 1.0.0-beta.8, 설치 에디터 6000.6.1f1(모듈 Web). 프로브 파일은 스크래치패드에만 작성했고 레포에는 아무것도 쓰지 않았다.

**(1) `Guid` 엔디언 프로브** — D-1 본문에 출력 인용. 추가로 `Guid.ToByteArray()`가 같은 리틀엔디언 특성을 보인다(`01 99 57 FE 52 83 7A BC` → `FE 57 99 01 83 52 BC 7A`). `Guid.CreateVersion7()`은 .NET 10에 존재하나 Unity(netstandard2.1)에는 없다.

**(2) Newtonsoft 13.0.2 계약 동작 프로브** — 실제 `contracts/fixtures/**`를 대상으로 실행.

```
== 1. 유효 fixture 왕복 (deserialize -> serialize -> JToken.DeepEquals) ==
  PING_SERVER/basic.json                    SAME
  PING_SERVER/null-client-time-max-seq.json SAME
  PING_REPLY/basic.json                     SAME
  PING_REPLY/with-correlation.json          SAME
== 2. invalid fixture (strict: DateParseHandling.None + MissingMemberHandling.Error) ==
  actor-field-injected.json    rejected  Could not find member 'player_id' on object of type 'PingServerCommand'
  missing-tick.json            rejected  Required property 'tick' not found in JSON
  payload-unknown-field.json   rejected  Could not find member 'client_sent_at' on object of type 'PingReplyPayload'
  command-id-not-v7.json       accepted  (스펙 §5 표대로 C#은 감지 불가)
  probe-seq-negative.json      accepted  (스펙 §5 표대로 — 단, long일 때만. 아래 4 참조)
== 3. 널 가능 envelope 필드에 Required.Always를 주면 ==
  null-client-time-max-seq.json (유효 fixture!) rejected
      Required property 'client_sent_at' expects a value but got null      <-- C-1의 근거
== 4. probe_seq를 uint로 매핑하면 ==
  probe-seq-negative.json        rejected  Error converting value -1 to type 'System.UInt32'   <-- C-4의 근거
  null-client-time-max-seq.json  accepted  (probe_seq = 4294967295 정상)
== 5. JObject 디스패치 경로와 client_sent_at ==
  JObject.Parse (기본)                    -> 09/17/2026 14:05:09      <-- 함정 재현
  JObject.Load(DateParseHandling.None)    -> 2026-09-17T14:05:09.123Z
  기본 JObject .ToObject<T>()             -> 09/17/2026 14:05:09      <-- 함정 재현
  안전 JObject .ToObject<T>(strict)       -> 2026-09-17T14:05:09.123Z
  JsonConvert.DeserializeObject (기본)    -> 2026-09-17T14:05:09.123Z  <-- 직접 경로에선 재현 안 됨
== 6. 경계값 ==
  tick = 9007199254740991 (정확)   probe_seq = 4294967295 (정확)
  Guid 왕복 소문자 하이픈 표기 유지: 01a0afaf-7e84-729e-9f17-464483f27a6d
```

→ **ADR-0002 "검증한 사실 4"를 그대로 재현했다.** 디스패치 경로에서만 터지고 직접 역직렬화에서는 안 터진다는 서술까지 정확하다.

→ 위 프로브 컴파일 시 `#nullable enable` 상태의 참조 타입 속성에서 **`CS8618` 경고 9건**이 발생했다. 이것이 C-5(a)(생성 파일 `#nullable disable` 고정)의 근거다.

**(3) Unity CLI 조회**

- `unity projects new --help`: 옵션은 `--path`, `--editor-version`, `--template`, `-a/--architecture`, `--open`만. git·cloud 옵션 **없음**.
- `unity projects create --help`: `--cloud`, `--no-cloud`, `--cloud-org`, `--cloud-project`, `--vcs`, `--git-namespace`, `--git-repo`, `--git-visibility`, `--git-default-branch`, `--git-remote-protocol`, `--git-description`, `--git-token`, `--git-token-stdin` 존재.
- `unity test --help`: `--junit-output` 설명이 "Path to write the JUnit report to **when both report formats are produced**" (E-2의 근거).
- `unity editors --installed --json`: `6000.6.1f1` 하나, `modules: "Web"`.

**(4) 로컬 파일 조사**

- URP 템플릿 tarball(`.../ProjectTemplates/com.unity.template.urp-blank-17.2.1.tgz`)의 `ProjectData~/Packages/manifest.json` — E-4에 전문 인용. `.git*`·cloud 파일 없음. `ProjectData~/Assets/TutorialInfo/` 존재. `ProjectSettings.asset`의 `companyName: Unity Technologies`, `productName: com.unity.template.urp-blank`.
- `com.unity.nuget.newtonsoft-json@3.2.2` 레지스트리 조회 및 tarball 검사 — `dist-tags.latest = 3.2.2`, `unity: "2018.4"`, 설명에 "Currently synced to version 13.0.2". `Runtime/Newtonsoft.Json.dll`(Editor용)과 `Runtime/AOT/Newtonsoft.Json.dll`(플레이어용) 두 개, **둘 다 `isExplicitlyReferenced: 0`(Auto Referenced)**, **둘 다 `UnityEngine` 문자열 없음**(엔진 비의존 확인 → `noEngineReferences: true`와 양립). 패키지의 `link.xml`은 `System.ComponentModel` 컨버터만 보존(A-3의 근거).
- `.claude/skills/integration-qa/scripts/check_contract_coverage.py` 정독 — `client` 태그 소스 루트 `client/Assets`, `.cs`, `Library`/`PackageCache` 등 제외, `PING_SERVER` 리터럴 또는 `PingServer*` 매칭(AC-9 판단 근거).

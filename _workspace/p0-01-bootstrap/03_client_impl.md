# client 구현 요약 (p0-01-bootstrap)

- 작성: client, 2026-09-18
- 태스크: T5(Unity 프로젝트) · T6(계약 코드 생성기) · T7(DTO·헬퍼·EditMode 테스트) · T8(이 문서)
- 스프린트 계약: SC-19 ~ SC-30 (전부 구현·실행 완료), 기록 항목 M-7·M-8·M-9
- 이번 슬라이스에서 클라이언트가 호출하는 REST/WebSocket 엔드포인트: **없음**(게임플레이·전송 계층은 p0-02)

## 0. 결과 한 줄

`cargo` 쪽과 무관하게 클라이언트 경로는 전부 실행으로 증명됐다. EditMode **18건 전부 통과**(실패 0, 에러 0, 스킵 0), 콜드 임포트에서도 컴파일 에러 0·`Assets/_Project/**` 경고 0, 생성기는 결정적이고 `--check`가 변조와 고아 파일을 모두 잡는다.

**단, 스펙 AC-7 / SC-22에 적힌 `unity test` 명령은 그대로 실행하면 실패한다.** 1번 항목을 먼저 읽어 달라.

## 1. 반드시 고쳐야 할 것 — `--report-format both`는 이 CLI에서 유효하지 않다

스펙 AC-7, 태스크 T7, 스프린트 계약 SC-22의 명령 F-1이 모두 `--report-format both`로 적혀 있다. 실행 결과:

```
$ unity test client --mode EditMode --report-format both --output ... --junit-output ...
error: option '--report-format <formats>' argument 'both' is invalid. Invalid report format. Allowed values: nunit, junit.
exit=2
```

도움말 문구("Report formats to produce, comma-separated: nunit, junit, or both")가 **오해를 부른다**. `both`는 리터럴 값이 아니라 "둘 다 만들려면 콤마로 나열하라"는 뜻이다. 동작하는 형태는 다음이며, 이것으로 두 리포트가 모두 생성된다.

```bash
unity test client --mode EditMode --no-banner --report-format nunit,junit \
  --output _workspace/p0-01-bootstrap/unity-tests/EditMode.nunit.xml \
  --junit-output _workspace/p0-01-bootstrap/unity-tests/EditMode.xml
```

이 문서의 모든 증거는 위 명령으로 얻었다. **스펙 AC-7·T7·SC-22의 명령 문자열을 `both` → `nunit,junit`으로 고쳐 달라**(architect·qa). 내가 고칠 수 있는 파일이 아니다. 리뷰 단계에서 내가 `both`를 제안했던 것이므로 근거를 정정하는 책임도 내게 있다 — 당시 근거는 도움말의 `--junit-output` 설명("when both report formats are produced")이었고, 그 부분은 **여전히 맞다**(형식을 하나만 주면 `--junit-output`은 무시된다). 틀린 것은 "둘 다"를 적는 방법뿐이다.

## 2. Editor 로그 경로 — QA가 적은 경로가 아니다 (SC-29)

SC-29의 `$LOG`가 `%LOCALAPPDATA%\Unity\Editor\Editor.log`로 되어 있는데, **`unity test`가 띄우는 에디터는 프로젝트 안에 로그를 쓴다.**

| 경로 | 실측 |
|------|------|
| `C:\Users\CHOISOOYEON\AppData\Local\Unity\Editor\Editor.log` | 4.9 KB, 21:22 — `projects new`가 띄운 Hub 쪽 에디터의 것. 테스트 실행 뒤에도 갱신되지 않는다 |
| **`client/Logs/Editor.log`** | **159 KB, 21:23 — 테스트 실행마다 갱신된다. 컴파일 로그가 여기에 있다** |

SC-29의 점검은 이 경로로 해야 한다.

```bash
LOG=client/Logs/Editor.log
grep -c "error CS" "$LOG"                          # 0
grep "warning CS" "$LOG" | grep -c "Assets/_Project"  # 0
```

보험으로 CLI stdout도 `_workspace/p0-01-bootstrap/unity-tests/unity-test-stdout.txt`에 남긴다(배치 실행에서는 거의 비어 있다 — 컴파일 로그는 위 Editor.log로 간다).

**추가 확인: `unity test`의 종료 코드는 실패에 실제로 반응한다.** 일부러 실패하는 테스트를 임시로 넣고 돌린 결과 **exit=8**이었고, 제거 후 다시 0으로 돌아왔다. 즉 SC-22의 "종료 코드 0"은 빈 신호가 아니다.

## 3. 만든 것

### T6 — 계약 코드 생성기

| 파일 | 내용 |
|------|------|
| `tools/codegen/ContractsCodegen.cs` | .NET 10 단일 파일 앱. NuGet 의존 없음(`System.Text.Json`만) |
| `tools/codegen/verify/Starfall.Contracts.Verify.csproj` | 선택 항목이었던 netstandard2.1 선컴파일 하네스. **넣었다** |

구현한 규칙(ADR-0002 §4):

- 키워드 allowlist 4분류. 목록 밖 키워드는 **JSON Pointer 경로와 함께 실패**한다. 실측:
  ```
  codegen: .../PING_SERVER.schema.json/properties/payload/properties/probe_seq/multipleOf:
    unsupported JSON Schema keyword. Extend the generator before using it in a contract.
  exit=2
  ```
- 정수는 선언된 `[minimum, maximum]`을 담는 가장 좁은 타입(`int` → `uint` → `long`). `ulong` 없음. **범위 선언이 없는 정수는 실패**한다. 실측:
  ```
  codegen: .../probe_seq: integer without both 'minimum' and 'maximum'. The C# type is chosen
    from the declared range (ADR-0002 section 4), so an unbounded integer has no mapping.
  exit=2
  ```
- `format: uuid` → `System.Guid`. 그 외 문자열(`RealTime`·`GameTime` 포함) → `string` 고정.
- `Required` 3행 표: required+비널 → `Required.Always`, required+널가능 → `Required.AllowNull`, 선택 → `Required.Default` + `NullValueHandling.Ignore`.
- 생성 파일은 `#nullable disable`로 시작. 필요한 `using`만 출력(불필요한 using 경고 여지 제거).
- 결정적 출력: 속성·파일 모두 `StringComparer.Ordinal` 정렬, LF, BOM 없음, 타임스탬프·도구 버전 미출력.
- `--check`는 **내용 + 파일 집합**을 본다. **`*.cs`만 대상이고 `*.meta`는 세지 않는다**(Unity 소유). `--check` 없이 실행하면 자기가 만들지 않은 `.cs`는 지우고 `.meta`는 남긴다.
- 추가 가드(요구사항은 아니지만 넣었다): 레지스트리 이름 ≠ 스키마 타입 상수이거나, 레지스트리 `schema_version` ≠ 스키마 `const`이면 **생성이 실패**한다. Rust 계약 테스트 4와 같은 성질을 생성 시점에 한 번 더 건다.

### T5 — Unity 프로젝트

- 생성: `unity --no-banner --non-interactive projects new client --path C:/WorkSpace/SpaceHistoric --editor-version 6000.6.1f1 --template com.unity.template.urp-blank` (89초, exit 0)
- `client/.git` 없음, 새 커밋 0건(레포 커밋 수는 architect의 초기 커밋 1건 그대로).
- 패키지(`client/Packages/manifest.json`): `com.unity.collab-proxy`·`com.unity.visualscripting` 제거, `com.unity.nuget.newtonsoft-json` **3.2.2** 추가. `test-framework` 1.8.0은 템플릿에 이미 있었다.
- `Assets/TutorialInfo/` 삭제. **`Assets/Readme.asset`도 함께 삭제했다** — 이 에셋은 `TutorialInfo/Editor/Readme.cs`를 참조하므로 폴더만 지우면 스크립트 잃은 에셋이 남는다. 폴더·에셋의 `.meta`는 대상과 함께만 지웠고, 남아 있는 파일의 `.meta`는 손대지 않았다.
- 식별자: `companyName: Starfall`, `productName: Starfall Dynasty`, `applicationIdentifier`(Android/Standalone/iPhone) `com.starfall.dynasty`. Editor가 꺼진 상태에서 `ProjectSettings.asset`의 스칼라만 수정했다(GUID 참조가 없는 필드라 YAML 편집 금지 규칙의 위험이 없다. 씬·프리팹은 이번에 건드리지 않았다).
- asmdef 2종 — **`overrideReferences: true`를 넣었고 실제로 컴파일된다**:

| asmdef | 핵심 설정 |
|--------|----------|
| `Starfall.Contracts` | `noEngineReferences: true`, `overrideReferences: true`, `precompiledReferences: ["Newtonsoft.Json.dll"]` |
| `Starfall.Tests.EditMode` | `includePlatforms: ["Editor"]`, `overrideReferences: true`, `precompiledReferences: ["nunit.framework.dll", "Newtonsoft.Json.dll"]`, `autoReferenced: false`, `defineConstraints: ["UNITY_INCLUDE_TESTS"]`, 참조 `Starfall.Contracts` + TestRunner 2종 |

두 어셈블리 모두 빌드됨(`Library/ScriptAssemblies/Starfall.Contracts.dll`, `Starfall.Tests.EditMode.dll`).

### T7 — DTO·헬퍼·테스트

**생성 DTO 3파일** (`client/Assets/_Project/Scripts/Contracts/Generated/`):

| 파일 | 클래스 | 필드 → C# 타입 |
|------|--------|----------------|
| `PingServerCommand.cs` | `PingServerCommand` (+ 중첩 `PingServerPayload`) | `client_sent_at`→`string`(AllowNull) · `command_id`→`System.Guid`(Always) · `command_type`→`string`(Always, `CommandTypeConst="PING_SERVER"`) · `schema_version`→`int`(Always, `SchemaVersionConst=1`) · `payload.probe_seq`→**`uint`**(Always) |
| `PingReplyMessage.cs` | `PingReplyMessage` (+ 중첩 `PingReplyPayload`) | `correlation_id`→`System.Guid?`(AllowNull) · `message_id`→`System.Guid`(Always) · `message_type`→`string`(Always, `MessageTypeConst="PING_REPLY"`) · `schema_version`→`int`(Always) · `tick`→**`long`**(Always) · `payload.command_id`→`System.Guid`(Always) · `payload.probe_seq`→**`uint`**(Always) |
| `ContractTypes.cs` | `ContractTypes` | `ByName`(이름→`Type`), `SchemaVersionByName`(이름→`int`) |

**손으로 쓴 파일** (`Generated/` 밖, 같은 어셈블리):

| 파일 | 역할 |
|------|------|
| `ContractJson.cs` | `Strict` 프로필(`DateParseHandling.None` + `MissingMemberHandling.Error`), `ReadToken`/`ReadObject`(안전 리더), `DeserializeStrict`/`Serialize`. `Runtime` 프로필은 **`NotImplementedException`을 던지는 프로퍼티**로 자리만 잡아 뒀다 — p0-02가 어디에 무엇을 만들어야 하는지 코드로 보인다 |
| `ContractDispatch.cs` | `TryGetTypeName`(`message_type`→`command_type`→`event_type` 순), `TryRead(string/JObject)`. **p0-02 수신 경로가 그대로 쓸 함수**다. 모르는 타입·깨진 프레임은 예외가 아니라 `false` + `error`로 돌려준다(연결을 죽이지 않는다) |
| `UuidV7.cs` | `NewGuid()`, `FromUnixTimeMilliseconds(long)`, `CurrentUnixTimeMilliseconds()`. 48비트 ms + 버전 7 + variant `10xx` + `RandomNumberGenerator` 난수를 **정규 문자열로 조립해 `new Guid(string)`**. `Convert.ToHexString`이 netstandard2.1에 없어 hex 변환은 직접 짰다 |

**테스트** (`client/Assets/_Project/Tests/EditMode/`): `ContractFixtures.cs`(로더·가드), `ContractFixtureTests.cs`(18 케이스).

## 4. SC ID → 테스트 이름 / 명령 대응표

클래스 접두사는 전부 `Starfall.Tests.EditMode.ContractFixtureTests`.

| SC | 테스트 이름 / 명령 | 결과 | 증거 |
|----|-------------------|------|------|
| SC-19 | 명령 E-1 (아래 §6) | PASS | 2회 실행 해시 동일 (`diff` 무출력) |
| SC-20 | 명령 E-2 | PASS | `exit=1`, `differs   PingReplyMessage.cs   (<경로>)` |
| SC-21 | 명령 E-3 | PASS | `exit=1`, `orphan    QaOrphan.cs   (<경로>)` |
| SC-22 | 명령 F-1 (`--report-format nunit,junit`) | PASS | `exit=0`, `tests="18" failures="0" errors="0" skipped="0"`, 두 리포트 파일 존재 |
| SC-23 | `Fixtures_RoundTrip_MatchesOriginal(PING_REPLY/basic.json)` 외 3건 + `Fixtures_RoundTrip_VisitedAllFourValidFixtures` | PASS | 각 케이스 `system-out`에 재직렬화 JSON 전문. 순회 4건 |
| SC-24 | `Invalid_Rejected_ByStrictProfile(...)` 5건 + `Invalid_Rejected_VisitedAllFiveCSharpCases` | PASS | 5건 파일명 + 예외 메시지(§5) |
| SC-25 | `Invalid_NotDetectableByCSharp_DocumentedAsymmetry(...)` 2건 | 기록 | 두 건 모두 **통과(=수락)**, 사유를 `system-out`에 명시 |
| SC-26 | `Dispatch_PreservesRealTime` | PASS | `via ContractDispatch.TryRead : 2026-09-17T14:05:09.123Z` / `via default JObject.Parse : 09/17/2026 14:05:09` |
| SC-27 | `UuidV7_MatchesSchemaPattern`, `UuidV7_NoDuplicatesWithinSameMillisecond` | PASS | 1,000건 패턴 검사 + 같은 ms에 **10,000건, 중복 0**, 공유 타임스탬프 접두사 `01a0b478-5ffd` |
| SC-28 | `FixtureLoader_FindsRepoRootByMarker`, `FixtureLoader_FailsWhenFewerThanFourValidFixtures` | PASS | 루트 `C:\WorkSpace\SpaceHistoric`, 센 개수 4, 가드가 빈/짧은 목록에 실제로 throw |
| SC-29 | 명령 F-2 (콜드) | PASS | Library 삭제 후 `exit=0`, 63초, `error CS`=0, `Assets/_Project` 경고=0 |
| SC-30 | 명령 F-3 | PASS | `m_EditorVersion: 6000.6.1f1` |

### 반례 5건의 실제 거부 메시지 (SC-24 증거)

```
PING_SERVER/actor-field-injected.json   Could not find member 'player_id' on object of type 'PingServerCommand'. Path 'player_id', line 6, position 14.
PING_SERVER/probe-seq-negative.json     Error converting value -1 to type 'System.UInt32'. Path 'payload.probe_seq', line 7, position 19.
PING_SERVER/probe-seq-above-u32.json    Error converting value 4294967296 to type 'System.UInt32'. Path 'payload.probe_seq', line 7, position 27.
PING_REPLY/missing-tick.json            Required property 'tick' not found in JSON. Path '', line 10, position 1.
PING_REPLY/payload-unknown-field.json   Could not find member 'client_sent_at' on object of type 'PingReplyPayload'. Path 'payload.client_sent_at', line 10, position 21.
```

`probe-seq-negative`와 `probe-seq-above-u32`가 **범위 기반 정수 매핑(architect 결정 ①)의 직접적 결실**이다. `long`이었다면 둘 다 조용히 통과했다.

## 5. 실행 방법 (명령 그대로)

```bash
export PATH="$HOME/.cargo/bin:$HOME/.dotnet/tools:/c/Users/CHOISOOYEON/AppData/Local/Unity/bin:$PATH"
cd /c/WorkSpace/SpaceHistoric

# --- DTO 생성 (T6) ---
dotnet run tools/codegen/ContractsCodegen.cs -- \
  --contracts contracts --out client/Assets/_Project/Scripts/Contracts/Generated

# --- 생성물 무결성 (I-2) ---
dotnet run tools/codegen/ContractsCodegen.cs -- \
  --contracts contracts --out client/Assets/_Project/Scripts/Contracts/Generated --check

# --- Unity 임포트 전 선컴파일 (선택, Safe Mode 예방) ---
dotnet build tools/codegen/verify/Starfall.Contracts.Verify.csproj -v:minimal --nologo
#   -> "경고 0개 / 오류 0개" (TreatWarningsAsErrors, netstandard2.1, LangVersion 9.0)

# --- EditMode 테스트 (T7) --- 주의: `both`가 아니라 `nunit,junit`
mkdir -p _workspace/p0-01-bootstrap/unity-tests
unity test client --mode EditMode --no-banner --report-format nunit,junit \
  --output _workspace/p0-01-bootstrap/unity-tests/EditMode.nunit.xml \
  --junit-output _workspace/p0-01-bootstrap/unity-tests/EditMode.xml

# --- 콜드 임포트 점검 (SC-29) --- 다른 Unity 항목을 통과시킨 뒤 마지막에
rm -rf client/Library && <위 unity test 명령>
grep -c "error CS" client/Logs/Editor.log                        # 0
grep "warning CS" client/Logs/Editor.log | grep -c "Assets/_Project"  # 0
sed -n 's/^m_EditorVersion: //p' client/ProjectSettings/ProjectVersion.txt | tr -d '\r'   # 6000.6.1f1
```

## 6. QA 재현용 메모 (쟁점 답변의 실측 결과)

- **`--out`에 임의 경로 가능**: 확인했다. QA는 `client/`를 건드리지 않고 스크래치 사본에서 SC-20/21을 재현하면 된다.
- **`--check`는 `.meta`를 고아로 신고하지 않는다**: 실물 `Generated/`(`.cs` 3 + `.meta` 3)에서 `--check` → `up to date (3 file(s))`, `exit=0`. QA의 `cp -r` 사본에서도 동일하게 동작한다.
- SC-19/20/21은 실물 경로와 스크래치 사본 양쪽에서 모두 통과를 확인했다.

## 7. 기록 항목

| # | 값 |
|---|-----|
| **M-7** | EditMode 테스트 1회: **콜드(Library 삭제 포함) 63초** / **웜 11초**. 참고로 스크립트가 새로 들어간 첫 실행은 41초 |
| **M-8** | `projects new` 직후: `companyName: DefaultCompany`, `productName: client`(폴더명), `applicationIdentifier.Standalone: com.Unity-Technologies.com.unity.template.urp-blank`. 설정 후: `Starfall` / `Starfall Dynasty` / `com.starfall.dynasty`. **Hub 레지스트리 등록 여부 = 등록된다** — `unity projects list`에 `C:\WorkSpace\SpaceHistoric\client`가 나타난다. `unity projects add`는 필요 없었다 |
| **M-9** | `collab-proxy`·`visualscripting` 제거의 영향 = **없다**. `packages-lock.json`이 오류 없이 재해석됐고, Editor 로그에 패키지 해석 오류·경고 0건, 콜드 임포트도 정상. 두 패키지와 그 전이 의존성이 lock에서 사라졌다 |

**패키지 실측(ADR-0003 표와 차이 있음).** Hub가 템플릿의 고정 버전이 아니라 레지스트리 최신을 해석했다. 템플릿 tarball 안의 값 → 실제 프로젝트: `ai.navigation` 2.0.12→**2.0.14**, `collab-proxy` 2.12.4→2.13.6(제거함), `inputsystem` 1.19.0→**1.20.0**, `visualscripting` 1.9.11→1.9.12(제거함). `render-pipelines.universal` 17.6.0, `test-framework` 1.8.0, `timeline` 6.6.0, `ugui` 2.6.0은 동일. **URP 17.6.0 + Input System 1.20.0이 이 프로젝트의 정본 버전**이다(techart·차기 슬라이스 참고).

## 8. 알려진 한계 / 하지 않은 것

1. **`--report-format both`가 동작하지 않는다** → §1. 스펙·태스크·스프린트 계약의 명령 문자열 정정이 필요하다.
2. **SC-29의 로그 경로가 틀렸다** → §2. `client/Logs/Editor.log`가 맞다.
3. **`client/.vscode/`가 `.gitignore`에 없다**(`git check-ignore` rc=1). 템플릿이 만든 에디터 설정이라 추적 후보로 보인다. `.gitignore`는 architect 소유라 건드리지 않았다. `client/client.slnx`·`client/Assembly-CSharp*.csproj`는 정상적으로 무시된다.
4. **`Runtime` 시리얼라이저 프로필은 코드로 존재하지 않는다**(architect 결정 ②대로). 접근하면 `NotImplementedException`이 뜬다. p0-02 전송 계층에서 구현한다.
5. **IL2CPP 스트리핑 대비(`link.xml` 또는 자체 `PreserveAttribute`)는 하지 않았다**(ADR-0001 §3 기록, 스펙 §3 제외 목록). 플레이어 빌드가 없어서 이번에 증명할 것이 없다. `Starfall.Contracts`가 `noEngineReferences: true`라 `[Preserve]`를 쓸 수 없으므로, 첫 플레이어 빌드 슬라이스에서 `Assets/link.xml`이 필요하다.
6. **PlayMode 테스트 없음, 씬·프리팹 작업 없음, UI 없음.** 이번 슬라이스 범위 밖이다. `Assets/Scenes/SampleScene.unity`와 `Assets/Settings/**`(URP 에셋)는 템플릿 그대로 두었다 — **`Assets/Settings/**`는 techart 소유**다.
7. **`verify/` 하네스는 `Starfall.Contracts`만 컴파일한다.** 테스트 어셈블리는 UnityEngine·NUnit이 필요해 Unity 밖에서 컴파일할 수 없다.
8. `Assets/InputSystem_Actions.inputactions`(템플릿 기본 입력 액션)를 남겨 두었다. Input System을 유지하기로 했으므로 지울 이유가 없고, 지우면 템플릿 씬이 경고를 낸다.

## 9. server·qa·architect가 알아야 할 것

- **server에게**: C# 쪽 타입은 `probe_seq`=`uint`, `tick`=`long`, `schema_version`=`int`, `command_id`/`message_id`/`payload.command_id`=`Guid`, `correlation_id`=`Guid?`, `client_sent_at`=`string`(널 허용)이다. 직렬화 시 Guid는 소문자 하이픈 표기로 나가고(왕복 확인), 널 가능 필드는 **키를 항상 쓰고 값만 `null`로** 보낸다(`{"correlation_id":null,...}` 실측). Rust에서 `skip_serializing_if`를 쓰면 이 대칭이 깨진다.
- **qa에게**: §1·§2·§6이 SC-22·SC-29·SC-20/21 재현에 직접 영향을 준다. `unity test`의 실패 시 종료 코드는 **8**이다(0이 유효한 신호임을 확인함).
- **architect에게**: 계약 변경 요청은 **없다**. 요청은 문서 3건의 명령 문자열 정정(§1)과 SC-29 로그 경로(§2)뿐이다. ADR-0003 §2의 템플릿 패키지 버전 표는 §7의 실측값으로 갱신하면 좋겠다.

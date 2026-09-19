# client의 스펙·ADR 검토 (p0-02)

- 대상: `docs/specs/p0-02-networking-spike.md` (draft), ADR-0005·0006·0008, `01_architect_tasks.md` T7~T10, `contracts/**` 신규 4종
- 작성: 2026-09-18, client (unity-client-engineer)
- **구현은 하지 않았다.** 생성기 확인은 전부 스크래치 출력 경로(`%TEMP%\claude\...\scratchpad\gen-out`)로 했고, `client/`·`tools/`·`docs/`·`contracts/`는 이번 턴에 한 글자도 고치지 않았다.
- 실행 환경: Unity 6000.6.1f1, `unity` CLI 1.0.0-beta.8, .NET SDK 10.0.401, Newtonsoft.Json 13.0.2(Unity `com.unity.nuget.newtonsoft-json` 3.2.2와 같은 버전)

---

## 동의하는 결정

| 결정 | 근거 | 클라이언트 측 영향 |
|------|------|------------------|
| WebSocket 1개, 텍스트 프레임, **1 프레임 = 1 메시지**, 16 KiB 상한 (ADR-0005 §2) | 손실 계측이 프레임 경계와 섞이지 않는다 | `ClientWebSocket` 수신을 고정 버퍼 재사용으로 짤 수 있다. 수신 측에도 같은 16 KiB 상한을 대칭으로 걸겠다 |
| `SESSION_READY`가 그 연결의 **첫 계약 메시지** (ADR-0005 §3) | 클라이언트가 자기 신원을 주장하지 않는다 | 연결 상태 기계가 `Connecting → Ready` 2단계로 끝난다. "내가 누구인지 물어보는" 왕복이 없다 |
| `COMMAND_RESULT`가 타입별 결과보다 **먼저**, `command_id`당 정확히 1건 (I-15) | 거부 경로가 반드시 테스트된다 | 대기 목록이 `Sent → Accepted → Completed` / `Sent → Rejected` 로 명확히 갈린다. `ACCEPTED`를 완료로 치지 않는 규칙에 동의 |
| **재개 없음. 재연결 = 새 세션** (ADR-0005 §3·§5, I-23) | 절반짜리 재개가 재현 불가 버그를 만든다 | 클라이언트에서 가장 비싼 코드(미전달 큐·재전송 창)가 통째로 사라진다. 강하게 동의 |
| 문자열 `enum` → Rust 닫힌 열거형 / **C# `string`** (ADR-0005 §4) | 값 추가가 구버전 클라이언트를 죽이지 않는다 | **생성기가 이미 그렇게 동작한다(U-1 실측 PASS, 아래 증거)**. 생성기 수정 불필요 |
| `if`/`oneOf`를 계약에 쓰지 않고 I-14를 서버 타입으로 강제 (ADR-0005 §4) | 생성기 allowlist 유지 | 동의. 클라이언트는 I-14를 **믿지 않고** `status`를 보고 분기한다 |
| `occurred_at`은 `GameTime` 문자열, 파싱·정렬 금지 (ADR-0006 §3) | 게임 시간이 실제 시간 축에 올라타지 않는다 | 생성기가 이미 `string`으로 고정하고 `DateParseHandling.None`이 이를 지킨다. 이번 슬라이스에 UI가 없어 추가 작업 없음 |
| 개발용 토큰을 `Authorization: Bearer`로, 추출은 한 함수에 (ADR-0008 §2) | 진짜 인증·WebGL 교체 지점이 좁다 | `ClientWebSocketOptions.SetRequestHeader`가 netstandard2.1·Mono 양쪽에 **존재함을 확인**했다(아래) |
| `IRealtimeTransport` 뒤 PC 구현만, WebGL은 자리만 (ADR-0005 §5·§6) | 측정 없는 이식 비용을 안 낸다 | 동의. `ClientWebSocket`·`Task`·`CancellationToken`이 인터페이스 밖으로 새지 않게 하겠다 |
| 태스크 순서 T7 → T8 → T9 (생성물을 Unity에 넣기 전 컴파일 검증) | p0-01 Safe Mode 교훈 | `tools/codegen/verify/`가 이미 그 역할을 한다. 그대로 쓴다 |

레지스트리 주도 생성 덕분에 **`contracts/events/domain/`이 새 디렉토리라는 것이 문제가 되지 않는다** — 생성기는 디렉토리를 순회하지 않고 `registry/types.json`의 `schema` 경로를 연다(`ContractsCodegen.cs` `Generator.Generate()`). T7 지시의 걱정(“`commands`/`messages`만 순회하면 조용히 건너뛴다”)은 실측 결과 해당 없음.

---

## 수정 요청 (무엇을 / 왜 / 대안)

### R1. **[차단] 유효 fixture는 16건이 아니라 12건이다** — 스펙 §5.3, AC-9(a), AC-11(a), AC-11(f)

- **무엇이 틀렸나**: 스펙이 네 곳에서 "유효 fixture 16건"이라고 쓴다(§5.3 본문, AC-9(a), AC-11(a), AC-11(f)). 실제 트리에는 **12건**이다. 반례 15건은 맞다.
- **증거**:
  ```
  $ find contracts/fixtures -name "*.json" -not -path "*/invalid/*" | wc -l   →  12
  $ find contracts/fixtures -path "*/invalid/*" -name "*.json" | wc -l       →  15
  타입별: COMMAND_RESULT 2/2, PING_REPLY 2/3, PING_SERVER 2/4,
          SESSION_CLOSED 2/2, SESSION_OPENED 2/2, SESSION_READY 2/2
  ```
  Unity EditMode 스위트가 독립적으로 같은 수를 센다(아래 §"현재 client 스위트 상태"): `Expected: 4 But was: 12`, `Expected: 7 But was: 15`.
  6 타입 × 2 = 12. 16은 p0-01 기준선(4)을 8로 잘못 잡고 신규 8을 더한 값으로 보인다.
- **왜 차단인가**: AC-11(f)가 "**16건 미만이면 테스트 실패**"를 요구한다. 그대로 구현하면 T9의 fixture 로더가 12건을 보고 **항상 실패**한다. 반대로 구현자가 임의로 12로 고치면 "스펙에 없는 판단"이 된다.
- **요청**: 네 곳의 16을 **12**로 정정. 타입당 2건 규칙을 유지한다면 "유효 fixture = 타입 수 × 2"로 쓰는 편이 다음 슬라이스에서 다시 틀리지 않는다.

### R2. **[차단] U-2 실패 — `actor_id` 좁힘이 C# DTO에 반영되지 않는다** (스펙 §5.4, AC-10(b))

- 실측 결과는 아래 "U-1·U-2 생성기 확인 결과"에 전부 있다. 요약: `SessionOpenedEvent.ActorId`가 `System.Guid?` + `Required.AllowNull`로 나오고, `SESSION_OPENED/invalid/actor-id-null.json`이 **C# Strict에서 통과**한다. §5.4 표의 "거부 예상 (`Required.Always`)"은 사실과 다르다.
- **원인은 계약이 아니라 생성기다**(따라서 스키마를 고칠 이유가 없다). `ContractsCodegen.cs:460`에서 타입 고유 속성을 envelope 속성 위에 **키 단위로 병합**하기 때문에, envelope의 `anyOf`(널 허용)가 살아남은 채 좁힘의 `type`/`format`이 덧붙는다. 그 다음 `Normalize`가 **`anyOf`를 가장 먼저 검사해서**(`ContractsCodegen.cs:531`) 무조건 `Nullable = true`로 만든다.
- **요청 — architect가 둘 중 하나를 골라 주기 바란다**:
  - **(A) 권장**: 생성기를 고친다(생성기는 client 소유, T7). 속성 병합에서 override가 구체 `type`을 들고 오면 상속된 `anyOf`를 버린다. 그러면 §5.4 행은 "거부 (`Required.Always`)"가 되고 **표를 고칠 필요가 없다**. `SESSION_CLOSED`도 같이 고쳐진다. 회귀 테스트(생성 결과의 `ActorId` 타입·Required 검사)를 T9에 넣겠다. 기존 유효 fixture 4건은 모두 `actor_id`가 비-null이라 깨지지 않는 것을 확인했다.
  - **(B)**: §5.4 표를 "**감지 불가** — 생성기가 envelope의 널 허용을 좁힘보다 우선한다"로 정정하고, 그 한계를 기록으로 남긴다.
  - (A)를 권장하는 이유: 이것이 **첫 번째 좁힘**이라 지금 고치지 않으면 앞으로 모든 좁힘이 같은 방식으로 조용히 무시된다. 스키마가 좁혔는데 DTO가 널을 받는 것은 "계약이 거짓말하는" 상태다.
- **T7 진행 조건**: (A)/(B) 지시가 오기 전에는 생성기를 건드리지 않고 기다린다. (A)면 T7에서 같이 처리한다.
- **부수 요청**: §5.4 표에 `SESSION_CLOSED.actor_id` 좁힘을 덮는 반례가 없다. `SESSION_CLOSED/invalid/actor-id-null.json`을 추가하면 두 타입이 대칭으로 검증된다(계약 변경이라 architect 소관).

### R3. `Runtime` 프로필 AC가 위험한 구현을 허용한다 — AC-11(d)

- 현재 문구: "`Runtime` 프로필이 payload의 모르는 필드를 **무시하고 경고를 남기며**, 같은 입력이 `Strict`에서는 예외가 된다."
- **문제**: 이 문구를 만족하는 가장 자연스러운 구현(`MissingMemberHandling.Error` + `Error` 핸들러에서 무조건 `Handled = true`)이 **필수 필드 누락까지 삼킨다.** 실측: `SESSION_READY/invalid/missing-session-id.json`이 예외 없이 통과하고 `session_id`가 `00000000-0000-0000-0000-000000000000`이 된다. 즉 서버가 깨진 메시지를 보내도 클라이언트가 "빈 Guid를 가진 정상 세션"으로 진행한다.
- **요청**: AC-11(d)에 한 줄 추가 — "**`Runtime`에서도 필수 필드 누락과 널 불가 필드의 널은 예외다.** `Runtime`이 관용하는 것은 *모르는 멤버* 하나뿐이다."
- 구현 가능성은 확인했다(아래 §"Runtime 프로필"). 이 문장이 없으면 "관용적 프로필"이라는 말이 "아무거나 받는 프로필"로 구현될 여지가 있다.

### R4. 백오프 파라미터는 동의하지만 ADR-0005 §5에 세 가지가 빠져 있다

파라미터 자체(`base=500ms, factor=2, cap=10s, full jitter`)는 Unity에서 자연스럽고 동의한다. 빠진 것:

1. **시도 카운터 `n`의 리셋 조건이 없다.** "소켓이 붙었을 때"로 리셋하면, 서버가 붙자마자 닫는 상태(재기동 루프, 업그레이드 후 즉시 종료)에서 30개 클라이언트가 영원히 500 ms 간격으로 재접속한다. **요청: `SESSION_READY`를 받았을 때만 `n = 0`으로 리셋한다**고 명시. TCP 연결 성공은 리셋 조건이 아니다.
2. **지수 클램프가 없어 공식을 그대로 쓰면 0 ms가 나온다.** C#의 `<<`는 시프트 수를 64로 마스킹한다. 실측:
   ```
   n=5   500L<<n =           16000  → min(cap, ·) = 10000   (정상)
   n=62  500L<<n =               0  → min(cap, ·) =     0   ← 지연 0
   n=63  500L<<n =               0  → min(cap, ·) =     0   ← 지연 0
   n=64  500L<<n =             500  → min(cap, ·) =   500   ← cap 아래로 되돌아감
   ```
   cap=10 s에서 62회 시도는 약 10분이다. 밤새 켜 둔 클라이언트가 도달한다. **요청: ADR에 "지수는 cap에 도달하는 최소값에서 클램프한다(여기서는 5)"를 명시.** AC-11(e)에 `n = 0,1,5,62,63,64,100`을 포함한 경계 케이스를 요구해 주면 테스트가 이 버그를 잡는다.
3. **"무제한 재시도"의 예외가 정의되지 않았다.** 401(토큰 무효)은 백오프로 해결되지 않는다. **요청: 영구 실패는 재시도하지 않고 호출자에게 알린다**를 ADR-0005 §5에 한 줄로. 다만 R5 때문에 클라이언트가 401을 알아낼 방법이 제한된다 — 아래 참조.

"재연결 시 재전송하지 않는다"는 Unity에서 **자연스럽다**. 오히려 클라이언트가 단순해진다(미전달 큐·재전송 창·중복 제거 없음). AC-13의 "재전송되지 않았음을 로그로 확인"을 위해, 대기 중이던 명령을 끊길 때 `ConnectionLost`라는 고정 사유로 실패 처리하고 **grep 가능한 한 줄**(`starfall.net: dropping N in-flight command(s) on disconnect (no resend, I-23)`)을 남기겠다. 이 문구를 AC-13 증거로 써도 되는지 확인 바란다.

### R5. 클라이언트는 업그레이드 실패의 HTTP 상태 코드를 읽을 수 없다 (U-5 확장)

- **실측**: Unity 6000.6.1f1 Editor는 Mono로 돌고(`Mono config path = .../MonoBleedingEdge/etc`), 프로젝트의 `apiCompatibilityLevel: 6`(= .NET Standard 2.1)이다. 그 컴파일 표면과 Mono 런타임 `System.dll` 양쪽에 **`ClientWebSocketOptions.CollectHttpResponseDetails`가 없다**(있으면 `ClientWebSocket.HttpStatusCode`로 401/503을 읽을 수 있다. .NET 7+ API). `SetRequestHeader`·`KeepAliveInterval`·`AddSubProtocol`은 **있다**.
- **결과**: 401(영구 실패)·503(비밀 미설정)·네트워크 오류가 클라이언트에게는 전부 `WebSocketException` 한 종류로 보인다. R4-(3)의 "401은 재시도하지 않는다"를 **지금 API로는 안전하게 구현할 수 없다**(예외 메시지 문자열 파싱은 하고 싶지 않다).
- **요청**: 둘 중 하나.
  - **(A) 권장**: ADR-0005 §5에 "클라이언트는 업그레이드 실패 사유를 구분하지 않고 전부 백오프 재시도하며, 원인 판정 증거는 **서버 로그·메트릭**이다"라고 못박고, ADR-0005 §6(WebGL이 나중에 부딪힐 것) 옆에 "헤더·HTTP 상세에 닿을 수 없다"는 같은 계열의 제약으로 기록한다. AC-4(b)(c)는 이미 서버 측 증거(401 응답, `SESSION_OPENED` 증가 0)로 판정하므로 **AC가 약해지지 않는다.**
  - (B) REST로 `/readyz`를 한 번 찔러 503/네트워크를 구분한다 → 이번 슬라이스에 REST 경로를 새로 만들게 되고(스펙 §3 제외 항목), 401은 여전히 구분되지 않는다. 비용 대비 소득이 없다고 본다.
- §10에 **U-5를 두 항목으로 쪼개 주기 바란다**: (U-5a) `SetRequestHeader`로 `Authorization`을 실제로 **보내는가** — API 존재는 확인했으나 Mono의 제한 헤더 처리 때문에 런타임 확인이 남아 있다. T8의 첫 작업. (U-5b) 도메인 리로드 시 연결 거동. (U-5c, 신규) 업그레이드 실패 상태 코드 접근 불가 — **이미 확인됨(위)**.

### R6. AC-17(부하 중 Unity 1대)은 "누가 언제 띄우는가"가 없으면 구조적으로 미검증이 된다

- A 단계는 60초다. Unity Editor 콜드 기동 + 프로젝트 로드가 이 PC에서 **21초**였고(오늘 실측: `projectLoad 21.15s`), PlayMode 진입이 더 붙는다. 부하 시작 후에 Editor를 띄우면 남는 시간이 거의 없다.
- **요청**: AC-17에 순서를 박아 주기 바란다 — "**Editor를 미리 띄워 PlayMode 접속을 성립시킨 뒤** qa가 봇 30개를 시작한다. 31번째 세션은 봇보다 먼저 열려 있어도 된다(스펙이 요구하는 것은 동시성이지 접속 순서가 아니다)." 또는 A 단계를 120초로 늘린다.
- 이 문장이 없으면 실행자가 "부하 중에 Unity를 띄운다"를 문자 그대로 하다가 시간 안에 못 붙고 미검증(환경) 처리하게 된다. 지금 정하면 실제로 통과할 수 있는 기준이다.

### R7. AC-11은 "종료 코드 0 + 리포트 2개"만으로는 0건 통과를 못 막는다

- **실측**: `--filter ZZZ_NoSuchTest_ZZZ` → **종료 코드 0**, 두 리포트 파일 정상 생성, 내용은 `<testsuites tests="0" failures="0" .../>`. p0-01이 경계한 바로 그 모양이다.
- **요청**: AC-11에 "생성된 리포트의 `tests` 수를 확인하고 구현 요약에 적는다"를 추가. 리포트 안의 건수가 증거다. (T9의 fixture 로더 가드는 fixture 수만 보고 테스트 수는 못 본다.)

### R8. 문서에 적힌 명령 검증 — `--report-format both`는 여전히 무효이고, `--help`가 틀렸다

- `unity test --help`는 `--report-format <formats> ... comma-separated: nunit, junit, **or both** (default: nunit)`이라고 **안내한다**. 실제로는:
  ```
  $ unity test client --mode EditMode --report-format both ...
  error: option '--report-format <formats>' argument 'both' is invalid. Invalid report format. Allowed values: nunit, junit.
  종료 코드 2
  ```
- 즉 `01_architect_tasks.md` T9의 경고(“`both`는 유효하지 않다”)가 **맞고**, 틀린 것은 Unity CLI의 `--help`다. 다음 사람이 help를 보고 "고치려" 들지 않도록, 스펙 AC-11 옆이나 T9 지시에 **"`--help`는 `both`를 안내하지만 CLI가 거부한다(1.0.0-beta.8 실측)"** 한 줄을 남겨 주기 바란다.
- 확정된 유효 형태(오늘 실행): `--report-format nunit,junit --output <a.xml> --junit-output <b.xml>` → 두 파일 모두 생성.

### R9. §5.4 표에 "어느 프로필로 잰 값인가"를 적어 주기 바란다 (경미)

표 머리가 `C# Strict`인데, **운영 수신 경로는 `Runtime`**이고 `Runtime`은 설계상 `Strict`보다 **덜** 잡는다(모르는 멤버를 통과시킨다). T9가 "감지 불가 항목이 실제로 통과함을 테스트가 명시적으로 기록"하려면 어느 프로필 기준인지가 명확해야 한다. 제안: 열 이름을 `C# (Strict)`로 두고, 표 아래에 "`Runtime`은 여기에 더해 `payload-unknown-field` 계열도 통과시킨다 — 의도된 비대칭" 한 줄.

---

## U-1·U-2 생성기 확인 결과 (실행 증거)

**모든 출력은 스크래치 경로로 갔다. `client/Assets/_Project/Scripts/Contracts/Generated/`는 이번 턴에 변경되지 않았다.**

```
$ dotnet run tools/codegen/ContractsCodegen.cs -- --contracts contracts --out <scratch>/gen-out
wrote CommandResultMessage.cs
wrote ContractTypes.cs
wrote PingReplyMessage.cs
wrote PingServerCommand.cs
wrote SessionClosedEvent.cs
wrote SessionOpenedEvent.cs
wrote SessionReadyMessage.cs
7 file(s)                                       ← 신규 4타입 전부 생성됨. 종료 코드 0

$ (같은 명령 2회차)  → 7건 모두 "unchanged", 파일 집합 해시 동일 (88296fb0…)
$ (--check)          → "--check: up to date (7 file(s))"  종료 코드 0
```

즉 **AC-10 전반부(2회 실행 후 해시 불변, `--check` 종료 코드 0)는 신규 4타입 포함해도 이미 성립한다.** 생성기는 레지스트리 주도라 `contracts/events/domain/`을 문제없이 찾는다.

### U-1 — 문자열 `enum` → C# `string`: **PASS. 생성기 수정 불필요**

생성된 DTO의 실제 타입(리플렉션으로 확인):

```
CommandResultPayload.Status     : System.String , Required = Always
CommandResultPayload.ReasonCode : System.String , Required = AllowNull
SessionClosedPayload.CloseReason: System.String , Required = Always
SessionOpenedPayload.Transport  : System.String , Required = Always
SessionReadyPayload.TickHz      : System.Int32  , Required = Always
```

ADR-0005 §4가 요구한 매핑 그대로다. 반례 `COMMAND_RESULT/invalid/unknown-reason-code.json`(`reason_code: "NOT_IN_THE_CLOSED_SET"`)과 `SESSION_CLOSED/invalid/unknown-close-reason.json`이 **C#에서 통과**하는 것도 확인했다 — §5.4가 예측한 "감지 불가" 그대로이고, 이것이 의도한 관용이다.

**동작 원리(다음 사람을 위해)**: `enum`은 `Keywords.Structural`("Interpreted: these change the generated code")에 들어 있지만 `Normalize`의 어디에서도 읽히지 않는다. 즉 **허용되고 무시된다**. 결과는 맞지만 주석은 틀렸다 — `enum`이 있어야 할 자리는 `Keywords.Constraint`("Ignored here on purpose; the Rust schema validator enforces them")다. 동작 변화 없는 1줄 정리이고, T7에서 (R2의 (A) 지시가 오면 그 수정과 함께) 같이 옮기겠다. 지시가 없으면 건드리지 않는다.

### U-2 — envelope 필드 널 가능성 좁힘: **FAIL**

```
SessionOpenedEvent.ActorId : System.Nullable`1[System.Guid] , Required = AllowNull   ← 기대: System.Guid / Required.Always
SessionClosedEvent.ActorId : System.Nullable`1[System.Guid] , Required = AllowNull   ← 같음
SessionOpenedEvent.CorrelationId : System.Guid , Required = Always                   ← (참고) 좁히지 않은 비-널 필드는 정상
```

생성된 코드 그대로:

```csharp
/// <summary>Narrowed from the envelope: never null for this type.</summary>
[JsonProperty("actor_id", Required = Required.AllowNull)]
public System.Guid? ActorId { get; set; }
```

**주석은 "never null"이라고 말하는데 타입은 널을 허용한다.** 그리고 반례가 실제로 통과한다:

```
SESSION_OPENED/invalid/actor-id-null.json  →  ACCEPTS   (C#이 감지하지 못함)
```

**원인** (`tools/codegen/ContractsCodegen.cs`):
1. `CollectObject`가 allOf(envelope)를 먼저 펼쳐 `props["actor_id"] = { description, anyOf:[UuidV7, null] }`를 만든다.
2. 타입 고유 `properties.actor_id`(`{description, $ref: UuidV7}`)를 **키 단위로 덮어쓴다**(`:460` `foreach (var kv in one) existing[kv.Key] = kv.Value;`). `type`·`format`이 추가될 뿐 **`anyOf`는 남는다.**
3. `Normalize`가 `anyOf`를 가장 먼저 검사하고(`:531`) 그 가지에서 `innerShape.Nullable = true`를 **무조건** 세운다. `type`이 나중에 병합돼도 이 결정을 못 뒤집는다.

**계약(스키마)은 맞다.** architect가 오프라인 검증기로 확인한 "스키마 층에서 좁힘이 동작한다"와 모순되지 않는다 — 깨진 것은 C# 생성기뿐이다. → **R2의 (A)/(B) 지시를 기다린다.**

### 15개 반례 전수 — C# `Strict` 실측 (검사 15건 / 거부 9 / 통과 6)

| fixture | C# Strict 실측 | 스펙 §5.4 / p0-01 §5 예측 | 일치 |
|---------|---------------|--------------------------|:---:|
| `COMMAND_RESULT/invalid/payload-unknown-field.json` | 거부 (`Could not find member 'queued_ticks'`) | 거부 | O |
| `COMMAND_RESULT/invalid/unknown-reason-code.json` | **통과** | 감지 불가 | O |
| `PING_REPLY/invalid/missing-tick.json` | 거부 (`Required property 'tick' not found`) | 거부 | O |
| `PING_REPLY/invalid/payload-unknown-field.json` | 거부 | 거부 | O |
| `PING_REPLY/invalid/tick-above-safe-integer.json` | **통과** | 감지 불가 | O |
| `PING_SERVER/invalid/actor-field-injected.json` | 거부 (`Could not find member 'player_id'`) | 거부 | O |
| `PING_SERVER/invalid/command-id-not-v7.json` | **통과** | 감지 불가 | O |
| `PING_SERVER/invalid/probe-seq-above-u32.json` | 거부 (`4294967296 → System.UInt32`) | 거부 | O |
| `PING_SERVER/invalid/probe-seq-negative.json` | 거부 (`-1 → System.UInt32`) | 거부 | O |
| `SESSION_CLOSED/invalid/correlation-id-null.json` | 거부 (`{null} → System.Guid`) | 거부 | O |
| `SESSION_CLOSED/invalid/unknown-close-reason.json` | **통과** | 감지 불가 | O |
| `SESSION_OPENED/invalid/actor-id-null.json` | **통과** | **거부 예상** | **X** ← R2 |
| `SESSION_OPENED/invalid/missing-world-id.json` | 거부 (`Required property 'world_id' not found`) | 거부 | O |
| `SESSION_READY/invalid/missing-session-id.json` | 거부 (`Required property 'session_id' not found`) | 거부 | O |
| `SESSION_READY/invalid/tick-hz-zero.json` | **통과** | 감지 불가 | O |

**유효 fixture 12건 전수**: `Strict`로 역직렬화 → 재직렬화 → `JToken.DeepEquals` 비교, **12/12 동일, 문제 0건**. 신규 4타입도 왕복이 성립한다.

### `Runtime` 프로필 — "무시한 멤버를 경고 로그" (architect 질문 1)

**`Error` 핸들러 없이는 불가능하다.** 실측 3가지:

| 방식 | 결과 |
|------|------|
| `MissingMemberHandling.Ignore` 단독 | 역직렬화는 되지만 **어떤 콜백도 없다.** 필드가 버려진 사실을 알 방법이 없다 |
| `ITraceWriter`(`MemoryTraceWriter`, Verbose) | `Could not find member 'queued_ticks' on …` 한 줄을 **얻을 수 있다**. 단 메시지 1건에 트레이스 6줄이 나온다(모든 단계를 문자열로 기록) → **핫 수신 경로에 쓸 수 없다**(스펙 §8 "프레임마다 할당하지 않는다"). 개발 빌드 전용 옵션으로는 쓸 만함 |
| `MissingMemberHandling.Error` + `settings.Error` 핸들러 | **유일하게 쓸 만한 방법.** 멤버 이름과 JSON 경로를 준다 |

세 번째 방식의 함정과 해결(실측):

- `ErrorContext.Member`는 **구분자가 못 된다**: 모르는 멤버(`member='queued_ticks'`)와 필수 필드 누락(`member='session_id'`) 둘 다 non-null이다.
- `ErrorContext.Path`도 못 된다: 널 불가 필드에 널이 오면 `path == member`가 되어 모르는 멤버와 같은 모양이 된다.
- **동작하는 구분자는 메시지 접두사뿐이다**: `Error.Message.StartsWith("Could not find member")`일 때만 `Handled = true`. Newtonsoft은 메시지를 현지화하지 않고 Unity가 13.0.2에 고정되어 있어 실용적으로 안정적이다. **단 이 접두사에 의존한다는 사실을 테스트로 고정**하겠다(Newtonsoft 업그레이드가 런타임이 아니라 테스트를 깨도록).

그 프로필을 **실제 수신 경로**(`ContractDispatch`가 쓰는 `JObject.ToObject(type, JsonSerializer.Create(settings))`)로 통과시킨 결과:

```
H1 payload에 모르는 필드      -> 통과, 경고 1건 [payload.queued_ticks]
H2 필수 필드 누락             -> 예외 (Required property 'session_id' not found)
H3 널 불가 필드에 널          -> 예외 (Error converting value {null} to type 'System.Guid')
H4 닫힌 집합 밖의 값          -> 통과, 경고 0건         ← 의도된 관용(ADR-0005 §4)
H5 유효 fixture               -> 통과, 경고 0건         ← 오탐 없음
I  Error 핸들러가 JsonSerializer.Create를 통과하는가 -> 예 (경고 1건, JsonConvert와 동일)
```

→ **AC-11(d)는 구현 가능하다.** 다만 R3의 한 줄(필수 필드 누락은 `Runtime`에서도 예외)을 AC에 넣어야 "관용"이 "무검증"으로 구현되는 것을 막는다.

### U-7 — `unity test --filter`의 문법: **정규식이다. glob이 아니다**

- CLI가 `--filter`를 Unity의 `-testFilter`로 넘기고, Unity는 이를 `groupNames`에 넣어 **`new Regex(pattern)`으로 테스트 전체 이름에 부분 일치**시킨다(앵커 없음).
- `--filter "*UuidV7*"` (glob처럼 씀) →
  ```
  ArgumentException: parsing "*UuidV7*" - Quantifier {x,y} following nothing.
  Test run completed. Exiting with code 3 (RunError).
  unity CLI 종료 코드 6, 리포트 파일 생성 안 됨
  ```
- `--filter "UuidV7"` → 26건 중 **2건 실행**, 종료 코드 0, 두 리포트 파일 정상 생성(20초).
- `--filter "ZZZ_NoSuchTest_ZZZ"` → **종료 코드 0**, 리포트 `tests="0"` (→ R7).

**관측한 `unity` CLI 종료 코드** (QA에 유용): 성공 `0` / 테스트 실패 `8`(Unity 2) / 런 에러(필터 정규식 오류 등) `6`(Unity 3) / 잘못된 CLI 인자 `2`(Unity 미기동).

### U-5 부분 선답 — `ClientWebSocket` API 표면

Unity 6000.6.1f1 Editor = Mono(`MonoBleedingEdge`), 프로젝트 `apiCompatibilityLevel: 6`(.NET Standard 2.1). netstandard2.1 참조 어셈블리와 Mono `System.dll` 양쪽 확인:

| 멤버 | netstandard2.1 ref | Mono `System.dll` | 의미 |
|------|:---:|:---:|------|
| `ClientWebSocketOptions.SetRequestHeader` | 있음 | 있음 | `Authorization` 헤더 **설정 자체는 API로 가능**. 실제 전송 여부는 T8에서 런타임 확인(U-5a) |
| `ClientWebSocketOptions.KeepAliveInterval` | 있음 | 있음 | ADR-0005 §2의 ping/idle과 맞물릴 수 있다 |
| `ClientWebSocketOptions.AddSubProtocol` | 있음 | 있음 | ADR-0005 §6(a) WebGL 티켓 경로가 나중에 쓸 자리 |
| `ClientWebSocketOptions.CollectHttpResponseDetails` | **없음** | **없음** | 401/503/네트워크 오류를 **구분할 수 없다** → R5 |

---

## 수용 기준 검토 (증명 가능한가, 문구 제안)

| AC | 증명 가능? | 판단 근거 / 문구 제안 |
|----|-----------|---------------------|
| **AC-10** 생성기 | **부분** | 전반부(2회 실행 해시 불변, `--check` 0)는 **오늘 이미 통과**. (a) U-1 PASS. (b) U-2 **FAIL** → R2 결론이 나와야 AC 문구가 확정된다. (b)의 "아니면 architect에게 보고" 경로가 발동됐다 |
| **AC-11** EditMode | **조건부** | `16` → `12` 정정 필요(R1). (d)에 "필수 필드 누락은 Runtime에서도 예외" 추가 필요(R3). 리포트의 `tests` 수 확인 추가 필요(R7). (a)~(f) 나머지는 전부 구현·검증 가능하고, (e)는 R4-(2)의 경계 케이스를 넣으면 실제 버그를 잡는다 |
| **AC-12** 실서버 왕복 | 가능 | 서버(T4·T5)가 서야 판정 가능. PlayMode에서 `ClientWebSocket` 비동기를 `[UnityTest]` 코루틴으로 돌리면 된다. (c)의 DB 확인은 qa의 `tests/e2e/` SQL을 쓰겠다(스펙 §7의 `docker compose exec -T postgres psql …` 형태 그대로) |
| **AC-13** 재연결 | 가능 | 단, "재전송되지 않았음"의 증거 문구를 고정해 달라(R4 끝). 또 **도메인 리로드가 이 AC를 망칠 수 있다**: Editor가 스크립트를 리로드하면 백그라운드 소켓이 비정상 종료되어 `close_reason`이 `CLIENT_CLOSED`가 아니라 `TRANSPORT_ERROR`가 된다. AC-12(c)가 `CLIENT_CLOSED`를 요구하므로, 측정 중 리로드가 없어야 한다는 전제를 T10에 기록하겠다 |
| **AC-17** 봇 30 + Unity 1 | **현 문구로는 사실상 미검증** | R6 참조. 순서를 박으면 통과 가능한 기준이 된다 |
| **AC-9 / AC-20** | 가능 | AC-9(a)의 16 → 12(R1). AC-20의 "비교한 필드 수"는 좋은 요구다 — 신규 4타입 DTO의 필드 수는 생성물 기준으로 `COMMAND_RESULT` 6(envelope)+3(payload), `SESSION_READY` 6+5, `SESSION_OPENED` 11+2, `SESSION_CLOSED` 11+2 = **46 필드**다. 이 수를 리포트 기준값으로 써도 된다 |
| §7 성능표 | 클라이언트 무관 | 잠정 게이트 3줄은 서버·봇 측정이다. 클라이언트는 §8의 "핫 경로 할당 금지"만 책임진다 — 수신 버퍼 재사용·`LINQ`/문자열 결합 금지로 지키고, 증거는 코드 리뷰 수준으로만 남긴다(프로파일러 측정은 이번 범위 밖) |

### 현재 client 스위트 상태 (기준선, 오늘 실측)

```
$ unity test client --mode EditMode --report-format nunit,junit --output … --junit-output …
tests="26" failures="12" errors="0" skipped="0"      종료 코드 8, 13초(웜)
```

**계약 4종이 들어온 시점에서 p0-01 EditMode 스위트는 이미 빨갛다.** 12건 전부 T7·T9가 고칠 예정된 실패다:

- `Fixtures_RoundTrip_MatchesOriginal(...)` 8건 — `registry type 'COMMAND_RESULT'/'SESSION_*' has no generated DTO` (T7이 생성하면 해소)
- `Fixtures_RoundTrip_VisitedAllFourValidFixtures` — `Expected: 4 But was: 12`
- `Invalid_Rejected_VisitedAllFiveCSharpCases` — `Expected: 7 But was: 15`
- `Invalid_Rejected_ByStrictProfile(COMMAND_RESULT/payload-unknown-field.json)` — DTO 없음
- `FixtureLoader_FindsRepoRootByMarker` — `Expected: 4 But was: 12`

이 스위트가 계약이 자라면 **조용히 넘어가지 않고 실패한다**는 것이 p0-01 설계 의도대로 동작했다는 증거이기도 하다. 그리고 이 세 숫자(12, 12, 15)가 R1의 독립 증거다.

---

## 내 태스크 실행 계획 (순서·예상 위험)

**착수 조건**: R1(fixture 12건)과 R2(U-2 처리 방향) 두 가지 지시. 나머지는 병행 가능.

| # | 단계 | 내용 | 위험 / 대응 |
|---|------|------|------------|
| 1 | T7-a | R2 지시가 (A)면 생성기 수정: 속성 병합에서 override가 구체 `type`을 주면 상속 `anyOf`를 버린다. `enum`을 `Keywords.Constraint`로 이동(동작 무변화). `--check`로 결정성 재확인 | 병합 규칙 변경이 기존 7파일을 바꿀 수 있다 → 수정 전후 생성물 diff를 떠서 **변한 것이 `SessionOpened/ClosedEvent.ActorId` 2곳뿐임을 증명**하고 T10에 붙인다 |
| 2 | T7-b | `client/.../Generated/`에 실제 생성. `tools/codegen/verify/`로 netstandard2.1 + C#9 + warnings-as-errors 컴파일 | Unity 임포트 전에 컴파일이 깨지면 Safe Mode → CLI 연결 불가(p0-01 교훈). verify가 그 앞을 막는다 |
| 3 | T9-a | EditMode 테스트의 기대 상수를 12/15로 올리고, "감지 불가" 6건을 **통과한다는 사실을 기록하는** 테스트로 만든다. U-2 회귀 테스트(ActorId 타입·Required) 추가 | fixture 수가 또 바뀌면 상수가 또 틀린다 → 하드코딩 대신 `registry/types.json`의 타입 수 × 2로 유도할 수 있는지 T9에서 판단 |
| 4 | T8-a | **U-5a 먼저**: 빈 PlayMode 씬에서 `SetRequestHeader("Authorization", …)` + `/ws` 접속 1회. 서버가 아직 없으면 로컬 더미 WebSocket 리스너로 헤더 수신만 확인 | Mono의 제한 헤더 처리로 `ArgumentException`이 날 가능성 → 나면 즉시 architect에 보고(ADR-0005 §6(a) 티켓 경로를 앞당겨야 한다) |
| 5 | T8-b | `IRealtimeTransport`(연결·송신·수신 콜백·종료·상태) + `PcWebSocketTransport`. 수신은 전용 Task → `ConcurrentQueue` → 메인 스레드 `Update`에서 배치 디스패치 | Unity API를 수신 스레드에서 부르면 조용히 죽는다 → 전송 계층은 `UnityEngine` 호출을 하지 않고 로그도 인터페이스(`ILogSink`) 뒤로 뺀다 |
| 6 | T8-c | `ContractJson.Runtime` 구현(위 실측 프로필), `message_type` 디스패치, 명령 대기 목록, 백오프 순수 함수(+ R4의 클램프) | 도메인 리로드(U-5b): `AssemblyReloadEvents.beforeAssemblyReload`·`EditorApplication.playModeStateChanged`·`Application.quitting`에서 **정상 Close**를 보내지 않으면 서버가 `TRANSPORT_ERROR`로 기록한다 → AC-12(c)의 `CLIENT_CLOSED`가 깨진다. 이 훅이 T8의 필수 항목이다 |
| 7 | T9-b | PlayMode 스모크(AC-12·AC-13). `--filter`는 U-7대로 **정규식**으로 쓴다 | 서버가 없으면 **미검증(환경)**으로 적는다. 통과로 적지 않는다 |
| 8 | T10 | 요약 + 실측치(U-1·U-2·U-5a/b/c·U-7, EditMode 콜드·웜, 생성기 변경분) | — |

**범위 밖으로 새지 않도록 미리 못박는 것**: 씬·프리팹·UI·카메라·함선을 만들지 않는다. `Starfall.Net`은 `noEngineReferences`를 켜지 **않고**(Unity API 필요), `overrideReferences: true` + `precompiledReferences`를 명시한다(p0-01 A-1). REST(`IApiClient`)는 이번에 만들지 않는다.

---

## 확인한 사실 / 미확인

### 확인한 사실 (전부 오늘 이 PC에서 실행)

| # | 사실 | 확인 방법 |
|---|------|----------|
| C-1 | 신규 4타입이 기존 생성기로 **수정 없이** 생성된다(7파일, 종료 코드 0). 레지스트리 주도라 `events/domain/`도 찾는다 | `dotnet run tools/codegen/ContractsCodegen.cs -- --contracts contracts --out <scratch>` |
| C-2 | 생성기는 **결정적**이다. 2회차 7건 unchanged, 파일 집합 해시 동일, `--check` 종료 코드 0 | 위 명령 2회 + `--check` |
| C-3 | **U-1 PASS**: `status`·`reason_code`·`transport`·`close_reason` 전부 C# `string` | 생성물 리플렉션 |
| C-4 | **U-2 FAIL**: `SessionOpened/ClosedEvent.ActorId`가 `Guid?` + `Required.AllowNull`. `actor-id-null.json`이 통과한다 | 생성물 리플렉션 + 반례 15건 전수 역직렬화 |
| C-5 | 유효 fixture **12건**(스펙은 16이라고 씀), 반례 **15건**. 유효 12건은 `Strict` 왕복 **12/12 동일** | `find` + 12건 전수 왕복 `JToken.DeepEquals` |
| C-6 | 반례 15건 C# `Strict`: **거부 9 / 통과 6**. 예측과 어긋나는 것은 `actor-id-null.json` **1건뿐** | 반례 15건 전수 역직렬화 |
| C-7 | `MissingMemberHandling.Ignore`에는 콜백이 없다. `Error` 핸들러 + 메시지 접두사 필터가 유일하게 실용적이고, **실제 수신 경로(`JObject.ToObject` + `JsonSerializer.Create`)에서도 동작한다** | 시나리오 D·E·F·H·I |
| C-8 | `ErrorContext.Member`와 `.Path`는 "모르는 멤버"와 "필수 필드 누락"을 구분하지 못한다 | 시나리오 E |
| C-9 | `unity test --filter`는 **정규식**(부분 일치). glob(`*X*`)은 `ArgumentException` → CLI 종료 코드 6, 리포트 없음 | 두 번의 실제 실행 + `client/Logs/Editor.log` |
| C-10 | 매칭 0건 필터는 **종료 코드 0 + `tests="0"` 리포트**를 만든다 | `--filter ZZZ_NoSuchTest_ZZZ` |
| C-11 | `--report-format both`는 CLI가 **거부**한다(종료 코드 2). `unity test --help`의 안내가 틀렸다. `nunit,junit`은 정상 동작하고 두 파일을 만든다 | 실제 실행 |
| C-12 | `unity` CLI 종료 코드: 성공 0 / 테스트 실패 8 / 런 에러 6 / 인자 오류 2 | 4회 실행 |
| C-13 | 현재 EditMode 스위트 **26건 중 12건 실패**(계약이 자란 만큼). 스위트가 독립적으로 valid 12·invalid 15를 센다 | 전체 실행, 13초(웜) |
| C-14 | `ClientWebSocketOptions.SetRequestHeader`·`KeepAliveInterval`·`AddSubProtocol`은 netstandard2.1·Mono 양쪽에 **있고**, `CollectHttpResponseDetails`는 **없다** | 참조 어셈블리·Mono `System.dll` 심볼 확인 |
| C-15 | Unity 6000.6.1f1 Editor는 Mono로 돈다. 프로젝트 `apiCompatibilityLevel: 6`(.NET Standard 2.1) | `client/Logs/Editor.log`, `ProjectSettings.asset` |
| C-16 | `500L << n`이 n=62,63에서 **0**, n=64에서 500이 된다(C# 시프트 마스킹) | 실행 |
| C-17 | Editor 프로젝트 로드 21.15초(`##utp ProjectInfo`). EditMode 웜 실행 13~20초 | Editor.log, 4회 실행 |

### 미확인 (실측이 남은 것)

| # | 미확인 사실 | 누가 언제 | 안 되면 |
|---|------------|----------|--------|
| U-5a | `SetRequestHeader("Authorization", …)`가 Mono 런타임에서 **실제로 헤더를 보내는가**(API 존재는 확인, 제한 헤더 처리는 미확인) | client, T8 첫 작업 | architect 보고 → ADR-0005 §6(a) 티켓 경로를 앞당긴다 |
| U-5b | 도메인 리로드·PlayMode 진입/종료 시 `ClientWebSocket`이 어떻게 끊기는가, 정상 Close를 보낼 수 있는가 | client, T8 | 서버가 `TRANSPORT_ERROR`로 기록 → AC-12(c)의 `CLIENT_CLOSED` 재검토 필요 |
| U-5c | **해결됨**: 업그레이드 실패의 HTTP 상태 코드는 읽을 수 없다(C-14) | — | R5로 ADR에 기록 요청 |
| U-N1 | 수신 경로의 프레임당 할당량(스펙 §8) — 프로파일러로 재지 않았다 | client, T8 이후(선택) | 이번 슬라이스는 코드 규약 준수만 주장하고 수치는 p1로 |
| U-N2 | PlayMode 테스트가 이 PC에서 실서버를 상대로 안정적으로 도는가 | client, T9 | 미검증(환경)으로 보고 |
| U-N3 | 부하 실행 중 Editor를 띄운 채 30 봇이 도는 자원 여유(U-6과 겹침) | qa + client, T11/T14 | AC-17 미검증(환경) |

### 이번 턴에 남긴 변경

없다. `client/`, `tools/`, `docs/`, `contracts/`는 읽기만 했다. 생성물·프로브·Unity 리포트는 전부 세션 스크래치 디렉토리에 있고 저장소 밖이다. `client/Library/`·`client/Logs/`는 `unity test` 실행으로 갱신됐으나 `.gitignore` 대상이다(`git status --porcelain client tools` → 빈 출력).

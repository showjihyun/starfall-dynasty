# client 구현 요약 (p0-02-networking-spike)

- 날짜: 2026-09-18
- 태스크: T7(생성기·DTO), T8(`Starfall.Net`), T9(테스트), T10(이 문서)
- 대상 AC: AC-11, AC-12 / 대상 SC: SC-38 ~ SC-51
- 상태: **SC-38 ~ SC-51 전부 완료.** 실서버 왕복·재연결은 2026-09-19에 실행했다.
- 커밋하지 않았다. `git add`도 하지 않았다.

## 0. 한눈에

| 항목 | 결과 |
|------|------|
| EditMode (오프라인) | **67 tests / 0 failures / 0 errors / 2 skipped**, 종료 코드 0. skip 2건은 서버 게이트가 걸린 `LiveServerTests` |
| EditMode (실서버, `STARFALL_LIVE_TESTS=1`) | **2 tests / 0 failures**, 종료 코드 0 |
| 컴파일 경고 | **0건** (`client/Logs/Editor.log`에 `warning CS` 없음) |
| 생성기 결정성 | 2회 실행 해시 동일, `--check` 종료 코드 0 |
| 생성기 수정 영향 | **2파일 2속성뿐** (diff 전문 §2.2) |
| U-5a (Authorization 헤더) | **PASS — Mono가 실제로 보낸다.** 와이어 덤프 §4.1 |
| U-5c (업그레이드 실패 상태 코드) | **읽을 수 없음 확인.** 401 응답이 `WebSocketException(Success)`로 온다 §4.2 |
| U-5b (도메인 리로드) | 훅은 구현했으나 **실서버 없이는 미검증** |
| 서버 왕복 SC-49/50/51 | **PASS** — Live 2 tests / 0 failures, DB 3세션 전부 1:1 |
| U-5b (도메인 리로드) | **부분 측정.** 훅이 없으면 `TRANSPORT_ERROR`(3회 재현), 블로킹 close면 `CLIENT_CLOSED`(3회 재현). 실제 리로드 중 훅 완주는 **미측정** |
| QA 자동화(31번째 연결) | **가능. Pipeline 패키지 불필요** — `unity run client -- -executeMethod …HoldOpen` (§13) |

---

## 1. 실행 방법

### 1.1 EditMode (SC-42 ~ SC-48). 서버 불필요

```bash
unity test client --mode EditMode --report-format nunit,junit \
  --output _workspace/p0-02-networking-spike/unity-tests/EditMode.nunit.xml \
  --junit-output _workspace/p0-02-networking-spike/unity-tests/EditMode.xml
```

실행 결과(2026-09-19): 종료 코드 **0**, `tests="67" failures="0" errors="0" skipped="2"`, 두 리포트 파일 존재.

| 테스트 클래스 | 건수 | 비고 |
|--------------|---:|------|
| `ContractFixtureTests` | 36 | |
| `RealtimeClientTests` | 18 | |
| `NarrowingAndRuntimeProfileTests` | 9 | |
| `TransportHandshakeTests` | 2 | 루프백 소켓 |
| `LiveServerTests` | 2 | **서버 없으면 skip** — `STARFALL_LIVE_TESTS=1`로 켠다 |
| | **67** | 실패 0, skip 2 |

skip 2건은 "통과"가 아니라 "서버 게이트가 걸렸다"는 뜻이고 리포트에 `skipped="2"`로 드러난다.

`--filter`는 정규식(부분 일치)이고 **매칭 0건이면 종료 코드 0 + `tests="0"`**이다. 그래서 위 건수가 판정의 일부다.

### 1.2 생성기 (SC-38, SC-40, SC-41)

```bash
GEN=client/Assets/_Project/Scripts/Contracts/Generated
dotnet run tools/codegen/ContractsCodegen.cs -- --contracts contracts --out "$GEN"
dotnet run tools/codegen/ContractsCodegen.cs -- --contracts contracts --out "$GEN" --check   # exit 0
cd tools/codegen/verify && dotnet build                                                      # 경고 0 오류 0
```

`verify`는 생성물을 **Unity에 넣기 전에** netstandard2.1 + C# 9 + warnings-as-errors로 컴파일한다(p0-01 Safe Mode 예방).

### 1.3 실서버 왕복 (SC-49, SC-50, SC-51)

```bash
export STARFALL_DEV_AUTH_SECRET=dev_only_not_a_secret
export STARFALL_NET_AUTOCONNECT=1          # 선택: PlayMode 진입 시 자동 접속
# (같은 셸에서) Unity Editor로 client 프로젝트를 연다 — 환경 변수 상속이 필요하다
# Play → Starfall/Net/Send PING_SERVER x3 → Starfall/Net/Disconnect
grep -F 'starfall.net: ' client/Logs/starfall-net.log
```

기본 엔드포인트는 `ws://127.0.0.1:8080/ws`이고 `STARFALL_WS_URL`로 바꾼다.

### 1.4 A 단계의 31번째 연결 (SC-61) — qa용 절차 (§13.2가 정본)

```
1. 서버·인프라 기동 확인
2. export STARFALL_DEV_AUTH_SECRET=... ; export STARFALL_NET_AUTOCONNECT=1
3. 같은 셸에서 Editor로 client 프로젝트를 열고 Play
4. client/Logs/starfall-net.log 에 SESSION_READY 한 줄이 뜨면 correlation_id를 correlations.txt에 넣는다
5. qa가 봇 30개 시작
6. A가 끝날 때까지 client/ 아래 파일을 저장하지 않는다 (도메인 리로드가 연결을 끊는다)
```

**`unity test --mode PlayMode`로는 이걸 할 수 없다** — 테스트가 끝나면 Editor가 종료되고 연결도 같이 죽는다. SC-61의 31번째는 사람이 띄워 둔 Editor여야 한다. (02_client_ack.md §3에서 합의)

---

## 2. 생성기 수정 (T7 / SC-39 ~ SC-41)

### 2.1 무엇을 고쳤나

**`tools/codegen/ContractsCodegen.cs`, `CollectObject`의 속성 병합 한 곳.**

타입 스키마가 envelope 필드의 널 가능성을 좁히면(`SESSION_OPENED`/`SESSION_CLOSED`의 `actor_id`), 기존 코드는 envelope의 `anyOf`를 남긴 채 좁힘의 `type`/`format`을 덧붙였다. 그 다음 `Normalize`가 **`anyOf`를 가장 먼저** 읽어 무조건 `Nullable = true`로 만들었다. 결과는 `Guid?` + `Required.AllowNull`이고, 주석만 "never null"이라고 말하는 상태였다.

수정: **override가 구체 `type`을 들고 오면 상속된 `anyOf`를 버린다.**

```csharp
if (one.ContainsKey("type") && !one.ContainsKey("anyOf"))
    existing.Remove("anyOf");
foreach (var kv in one) existing[kv.Key] = kv.Value;
```

넓히는 방향(비-널 envelope 필드를 타입이 널 가능으로)은 그대로 동작한다. override가 자기 `anyOf`를 들고 오면 조건이 거짓이라 아무것도 지우지 않고, `Normalize`가 그 `anyOf`를 읽는다.

**SC-41**: `enum`을 `Keywords.Structural` → `Keywords.Constraint`로 옮겼다. `enum`은 `Normalize` 어디에서도 읽히지 않으므로 "구조(출력에 영향)" 분류가 사실과 달랐다. **생성물 차이 없음**(§2.2 diff에 이 변경으로 인한 hunk가 하나도 없다).

### 2.2 수정 전후 생성물 diff 전문 (SC-40)

같은 `contracts/` 트리에 대해 수정 전 생성기와 수정 후 생성기를 각각 스크래치 디렉토리에 돌려 `diff -ur`한 결과 **전부**:

```diff
--- gen-before/SessionClosedEvent.cs
+++ gen-after/SessionClosedEvent.cs
@@ -15,8 +15,8 @@
         public const int SchemaVersionConst = 1;
 
         /// <summary>Narrowed from the envelope: never null for this type.</summary>
-        [JsonProperty("actor_id", Required = Required.AllowNull)]
-        public System.Guid? ActorId { get; set; }
+        [JsonProperty("actor_id", Required = Required.Always)]
+        public System.Guid ActorId { get; set; }
 
         /// <summary>Id of the event or command that directly caused this event, or null.</summary>
         [JsonProperty("causation_id", Required = Required.AllowNull)]

--- gen-before/SessionOpenedEvent.cs
+++ gen-after/SessionOpenedEvent.cs
@@ -15,8 +15,8 @@
         public const int SchemaVersionConst = 1;
 
         /// <summary>Narrowed from the envelope: never null for this type.</summary>
-        [JsonProperty("actor_id", Required = Required.AllowNull)]
-        public System.Guid? ActorId { get; set; }
+        [JsonProperty("actor_id", Required = Required.Always)]
+        public System.Guid ActorId { get; set; }
 
         /// <summary>Id of the event or command that directly caused this event, or null.</summary>
         [JsonProperty("causation_id", Required = Required.AllowNull)]
```

**변경 파일 2개, hunk 2개, 속성 2개.** 나머지 5개 생성 파일(`CommandResultMessage`, `ContractTypes`, `PingReplyMessage`, `PingServerCommand`, `SessionReadyMessage`)은 바이트 단위로 동일하다.

### 2.3 결정성 (SC-38)

```
2회 연속 생성 → 7파일 sha256 목록 동일 ("deterministic OK")
--check → "--check: up to date (7 file(s))"  종료 코드 0
```

### 2.4 좁힘이 과하게 먹지 않았다는 증거

`Narrowing_DoesNotLeakToFieldsTheEnvelopeLeavesNullable`이 6개 속성을 검사한다. 좁힘 수정이 "모든 곳의 `anyOf`를 지우는" 형태였다면 `CausationId`가 같이 뒤집혔을 것이다.

```
SessionOpenedEvent.CausationId   : System.Nullable`1[System.Guid] , Required = AllowNull
SessionOpenedEvent.CorrelationId : System.Guid                    , Required = Always
SessionClosedEvent.CausationId   : System.Nullable`1[System.Guid] , Required = AllowNull
SessionClosedEvent.CorrelationId : System.Guid                    , Required = Always
CommandResultMessage.CorrelationId : System.Nullable`1[System.Guid] , Required = AllowNull
PingServerCommand.ClientSentAt     : System.String                 , Required = AllowNull
nullable-envelope properties checked: 6
```

---

## 3. SC ID → 테스트·명령 대응표

| SC | 검증 방법 | 결과 | 증거 |
|----|----------|:---:|------|
| SC-38 | 명령 G-2 | **PASS** | `deterministic OK` + `--check: up to date (7 file(s))` exit 0 |
| SC-39 | `NarrowingAndRuntimeProfileTests.Narrowing_ActorId_IsNonNullableAndAlwaysRequired` (검사 2건) + `ContractFixtureTests.Invalid_Rejected_ByStrictProfile(SESSION_OPENED/actor-id-null.json)`, `(SESSION_CLOSED/actor-id-null.json)` | **PASS** | `narrowed properties checked: 2`, 반례 2건 모두 `JsonSerializationException` |
| SC-40 | 수정 전후 diff | **PASS** | §2.2 — 파일 2, hunk 2, 속성 2 |
| SC-41 | `enum` → `Keywords.Constraint`(`ContractsCodegen.cs`) | **PASS** | §2.2 diff에 이 변경으로 인한 생성물 차이 **없음** |
| SC-42 | 명령 G-1 | **PASS** | exit 0, `tests="65" failures="0" errors="0"`, 리포트 2개 |
| SC-43 | `Fixtures_RoundTrip_MatchesOriginal`(12 케이스) + `Fixtures_RoundTrip_VisitedEveryValidFixture` | **PASS** | `valid fixtures round-tripped: 12` |
| SC-44 | `Invalid_Rejected_ByStrictProfile`(11 케이스) + `Invalid_Rejected_VisitedEveryCSharpCase` + `Invalid_NotDetectableByCSharp_DocumentedAsymmetry`(5 케이스) + `NotDetectable_ListCoversExactlyFive` | **PASS** | `must reject: 11`, `unable to detect: 5`, 11+5=16 |
| SC-45 | `RealtimeClientTests.Dispatch_UnknownMessageType_WarnsAndKeepsProcessing`, `Dispatch_UnreadableFrame_WarnsAndKeepsProcessing` | **PASS** | 경고 후 다음 메시지가 정상 처리됨을 Assert |
| SC-46 | `Runtime_IgnoresUnknownMember_AndWarns`, `Runtime_SameInput_IsAnExceptionUnderStrict`, `Runtime_MissingRequiredField_StillThrows`, `Runtime_NullInNonNullableField_StillThrows`, `Runtime_ValidFixture_ProducesNoWarnings`, `Runtime_PrefixDependency_IsPinned` | **PASS** | §5 |
| SC-47 | `Backoff_BoundaryAttempts_AreAlwaysInsideTheCap`(n 7개), `Backoff_CeilingDoublesThenClamps`, `Backoff_IsDeterministicForAFixedSeed`, `Backoff_RejectsNegativeAttempt` | **PASS** | §6 |
| SC-48 | `FixtureLoader_FindsRepoRootByMarker`, `FixtureLoader_FailsWhenTooFewValidFixtures` | **PASS** | `valid fixtures counted: 12`, `guard sees 11 of 12 expected fixtures` → throw |
| SC-49 | `LiveServerTests.Live_ThreePings_RoundTripInOrder` | **PASS** | §11.1 — 첫 프레임이 `SESSION_READY`, `tick_hz=20`, 3왕복 전부 `COMMAND_RESULT` → `PING_REPLY` 순서, 검사 3쌍 |
| SC-50 | 위 테스트가 출력한 correlation으로 `psql` 조회 | **PASS** | §11.2 — 3 correlation 전부 OPENED 1 / CLOSED 1, `close_reason=CLIENT_CLOSED` |
| SC-51 | `LiveServerTests.Live_ServerInitiatedClose_ReconnectsAsANewSession` | **PASS** | §11.3 — 서버가 close 1002로 끊음 → 백오프 351 ms → 새 session_id·correlation_id, `dropping 1 in-flight`, attempt 0→1→0 |

**SC-44 의 층별 결과는 §0.5 표와 100% 일치한다. architect 통지 사유 없음.**

---

## 4. 실측 (U-5)

### 4.1 U-5a — `SetRequestHeader("Authorization", …)`는 실제로 전송된다. **PASS**

`TransportHandshakeTests.Upgrade_CarriesTheAuthorizationHeader`가 루프백 `TcpListener`를 띄우고 `PcWebSocketTransport`로 접속해 **와이어에 도착한 요청 헤드 원문**을 읽는다(`HttpListener`는 Windows에서 URL 예약이나 관리자 권한이 필요해 쓰지 않았다).

```
GET /ws HTTP/1.1
Host: 127.0.0.1:54592
Connection: Upgrade
Upgrade: websocket
Sec-WebSocket-Version: 13
Sec-WebSocket-Key: UXsWeGSnJEiZ6fKkk/g1Qg==
Authorization: Bearer 01a0b1c2-7e57-7c11-8e57-000000000001.12004f438e12f26610239fdb6b24cfec619acee05a9d9c008e85e030b8312274
```

Mono는 `Authorization`을 제한 헤더로 막지 않는다. **ADR-0005 §6(a)의 티켓 경로를 앞당길 필요가 없다.**

**server에게**: 위가 서버가 보게 될 업그레이드 요청의 실제 모양이다. 헤더 이름은 `Authorization`, 값은 `Bearer ` + `<subject>.<hex hmac>`이고 `Host`·`Connection`·`Upgrade`·`Sec-WebSocket-Version`·`Sec-WebSocket-Key` 외의 헤더는 붙지 않는다.

### 4.2 U-5c — 업그레이드 실패의 원인은 클라이언트가 알 수 없다. **재확인**

`TransportHandshakeTests.FailedUpgrade_ReportsDisconnectWithoutAStatusCode`가 401을 응답하는 리스너를 상대로 측정했다.

```
disconnect reported as: Failed WebSocketException(Success): Unable to connect to the remote server
```

HTTP 상태 코드가 없을 뿐 아니라 **`WebSocketErrorCode`가 `Success`로 온다.** 메시지 문자열도 401을 언급하지 않는다. ADR-0005 §5의 "원인을 구분하지 않고 전부 백오프 재시도"가 유일하게 구현 가능한 규칙이라는 것이 코드가 아니라 실행으로 확인됐다. **R4-3을 거부한 architect 판단이 옳았다.**

### 4.3 U-5b — 도메인 리로드 시 거동: **부분 측정 (§12에 결과)**

훅은 세 개 다 걸었다(`AssemblyReloadEvents.beforeAssemblyReload`, `EditorApplication.playModeStateChanged == ExitingPlayMode`, `Application.quitting`). 전부 **블로킹 Close**(예산 1500 ms)를 부른다 — Editor는 비동기를 기다려 주지 않기 때문이다. 예산 안에 못 끝내면 경고를 남긴다.

**실서버가 없으면 이 훅이 목표(서버가 `CLIENT_CLOSED`로 기록)를 달성했는지 확인할 수 없다.** SC-50과 같이 판정해야 한다.

### 4.4 U-7 — 재확인 없음(1차 리뷰에서 확정). `--report-format both`는 여전히 거부됨

---

## 5. `Runtime` 프로필 (SC-46)

`ContractJson.CreateRuntime(Action<string> onIgnoredMember)` / `ContractJson.Runtime`.

`MissingMemberHandling.Error` + `Error` 핸들러에서 **`Error.Message.StartsWith("Could not find member")`일 때만** `Handled = true`. 그 외에는 손대지 않아 그대로 던진다.

| 입력 | Runtime | 근거 |
|------|--------|------|
| `COMMAND_RESULT/invalid/payload-unknown-field.json` | **통과 + 경고 1건** (`payload.queued_ticks`), `status`·`command_id` 보존 | 서버가 먼저 배포된다 |
| 같은 입력, `Strict` | **예외** | 대조군 |
| `SESSION_READY/invalid/missing-session-id.json` | **예외**, 경고 0건 | R3 |
| `SESSION_CLOSED/invalid/correlation-id-null.json` | **예외**, 경고 0건 | R3 |
| 유효 fixture 12건 | **전부 통과, 경고 0건** | 오탐 없음 |

**접두사 의존을 테스트로 고정했다**(`Runtime_PrefixDependency_IsPinned`). Newtonsoft이 문구를 바꾸면 런타임이 아니라 이 테스트가 깨진다. 실제로 찍힌 문구:

```
Could not find member 'client_sent_at' on object of type 'PingReplyPayload'. Path 'payload.client_sent_at', line 10, position 21.
```

`ErrorContext.Member`와 `.Path`로는 구분할 수 없다는 1차 리뷰의 실측이 근거이고, 그 사실은 `ContractJson.cs`의 `IgnoredMemberMessagePrefix` 주석에 남겼다.

**`ContractJson.Runtime`은 정적 이벤트 `ContractJson.IgnoredMember`로 경고를 흘린다** — `Starfall.Contracts`는 `noEngineReferences: true`라 스스로 로그를 남길 수 없다. `RealtimeClient`는 정적 이벤트 대신 `CreateRuntime(sink)`로 자기 싱크를 쓴다(전역 상태를 만들지 않는다).

---

## 6. 백오프 (SC-47)

`ReconnectPolicy` — 순수 함수, 시계·Unity API 없음. `random(1, min(10s, 500ms * 2^min(n,5)))`.

하한을 0이 아니라 **1 ms**로 뒀다. "지연이 0이 되지 않는다"가 확률이 아니라 함수의 성질이어야 로그를 읽을 때 "백오프가 빠졌다"와 구분된다.

측정된 경계값(고정 시드):

```
n=  0  ceiling_ms=500    delay_ms=90
n=  1  ceiling_ms=1000   delay_ms=368
n=  5  ceiling_ms=10000  delay_ms=1261
n= 62  ceiling_ms=10000  delay_ms=2725
n= 63  ceiling_ms=10000  delay_ms=7576
n= 64  ceiling_ms=10000  delay_ms=3605
n=100  ceiling_ms=10000  delay_ms=9153
boundary attempts checked: 7
```

ceiling 진행: `500, 1000, 2000, 4000, 8000, 10000, 10000, 10000` (n=5에서 클램프).
결정성: seed 99 → `224, 866, 1936, 2757, 7225, 748, 2637, 1090, 2256, 7801, 2283, 7581` (두 번 돌려 동일).

`ReconnectPolicy.CeilingMs`는 시프트가 아니라 **반복 곱셈**으로 `2^e`를 만든다. 시프트가 바로 그 버그(`500L << 62 == 0`)의 원인이라, 클램프를 걸어도 시프트를 남겨 두면 다음 사람이 클램프를 지웠을 때 조용히 되돌아온다.

---

## 7. `Starfall.Net` 구조

| 파일 | 역할 |
|------|------|
| `IRealtimeTransport.cs` | 연결·송신·수신 콜백·종료·상태 + `Pump()`. **PC 전용 타입이 하나도 없다** — `ClientWebSocket`·`Task`·`CancellationToken`은 전부 구현 뒤에 있다 |
| `PcWebSocketTransport.cs` | `ClientWebSocket` 구현. **`UnityEngine`을 호출하지 않는다**(백그라운드 태스크에서 도니까). 수신 버퍼·조립 스트림 재사용, 16 KiB 상한, 송신 큐 256, 바이너리 프레임 거부 |
| `RealtimeClient.cs` | `message_type` 디스패치, 세션 신원, 명령 대기 목록, 재연결 타이머. 메인 스레드 전용 |
| `PendingCommands.cs` | `Sent → Accepted → Completed` / `Sent → Rejected` / `→ ConnectionLost` |
| `ReconnectPolicy.cs` | 순수 백오프 |
| `StarfallNetLog.cs` | **QA가 파싱하는 고정 문구** |
| `DevAuthToken.cs` | ADR-0008 §1 HMAC 토큰. 비밀은 환경 변수에서만 |
| `ILogSink.cs` | 스레드 안전 로그 seam + `RecordingLogSink`(테스트용) |
| `UnityLogSink.cs` | `Debug.*` + `client/Logs/starfall-net.log` |
| `StarfallNetHost.cs` | `MonoBehaviour`. 매 프레임 `Pump()`, 세 종료 훅에서 블로킹 Close |
| `Editor/StarfallNetMenu.cs` | `Starfall/Net/Connect`·`Disconnect`·`Send PING_SERVER x3`·`Print Dev Token Subject` |

asmdef: `Starfall.Net`(→ `Starfall.Contracts`), `Starfall.Net.Editor`(Editor 전용). 둘 다 `overrideReferences: true` + `precompiledReferences: ["Newtonsoft.Json.dll"]`, `noEngineReferences`는 **켜지 않았다**(Unity API 필요 — p0-01 A-1).

**씬·프리팹을 만들지 않았다.** `[RuntimeInitializeOnLoadMethod]` 부트스트랩이라 빈 기본 씬에서 Play만 눌러도 붙는다. 씬 YAML을 손으로 쓰면 GUID 참조가 깨진다.

### 7.1 I-15를 지키는 방식

`COMMAND_RESULT{ACCEPTED}`는 **완료가 아니다.** `PING_REPLY`가 와야 `Completed`가 된다. `PING_REPLY`가 `COMMAND_RESULT`보다 먼저 오면 **에러 로그**를 남긴다(`violates I-15`) — 삼키면 서버 순서 버그가 부하 테스트를 그냥 통과한다.

### 7.2 I-23을 지키는 방식

끊기면 `FailAllOnDisconnect()`가 전부 `ConnectionLost`로 settle하고 **재전송 큐가 아예 없다.** 로그 한 줄은 **0건일 때도** 남긴다.

---

## 8. QA가 파싱할 로그 문구 (확정)

`client/Logs/starfall-net.log`(타임스탬프 + 레벨 + 한 줄, 스택 트레이스 없음)와 `client/Logs/Editor.log`(Unity가 스택 트레이스를 덧붙임) 양쪽에 같은 줄이 나간다.

| 문구 | 언제 | SC |
|------|------|---|
| `starfall.net: SESSION_READY session_id=<uuid> correlation_id=<uuid> actor_id=<uuid> world_id=<uuid> tick_hz=<int> server_version=<s> attempt=<int>` | `SESSION_READY` 수신 | SC-49, SC-50, SC-51, SC-61 |
| `starfall.net: dropping N in-flight command(s) on disconnect (no resend, I-23)` | 연결 종료 시. **N=0에도 남긴다** | SC-51 |
| `starfall.net: reconnect attempt=<n> delay_ms=<ms> (counter resets only on SESSION_READY)` | 재연결 대기 직전 | SC-51 |
| `starfall.net: closing session_id=<uuid> reason=<CLIENT_CLOSED\|EDITOR_RELOAD\|PLAYMODE_EXIT\|APP_QUIT> code=1000` | 정상 Close 송신 | SC-50 |

필드 순서는 고정이고 UUID는 계약 표기 그대로(`Guid.ToString("D")`)다. 문구는 `StarfallNetLog.cs`에 리터럴로 모여 있고, `RealtimeClientTests.SessionReady_IsLoggedInTheFormatQaParses`가 접두사와 필드 순서를 Assert한다.

correlation 수집:

```bash
grep -oE 'starfall\.net: SESSION_READY session_id=[0-9a-f-]{36} correlation_id=[0-9a-f-]{36}' \
  client/Logs/starfall-net.log \
| grep -oE 'correlation_id=[0-9a-f-]{36}' | cut -d= -f2 >> /path/correlations.txt
```

`reason=EDITOR_RELOAD`가 찍혔는데 DB가 `TRANSPORT_ERROR`면 Close가 늦은 것이고, 줄이 아예 없이 `TRANSPORT_ERROR`면 훅이 안 걸린 것이다.

---

## 9. AC-21 필드 표 (46 규약의 근거)

QA 질문 4의 약속대로, 총계를 주장하지 않고 표에서 유도되게 싣는다. `counted` 열이 어느 규약으로 센 행인지 표시한다.

### `COMMAND_RESULT` — envelope 6 (payload 포함) + payload 3 = 9

| 필드 | C# 타입 | Required | 스키마 | counted |
|------|--------|---------|--------|---------|
| `message_id` | `Guid` | Always | 필수·비널 | envelope |
| `message_type` | `string` (const) | Always | 필수·비널 | envelope |
| `schema_version` | `int` (const 1) | Always | 필수, 1..2147483647 | envelope |
| `tick` | `long` | Always | 필수, 0..2^53-1 | envelope |
| `correlation_id` | `Guid?` | AllowNull | 필수·널 가능 | envelope |
| `payload` | `CommandResultPayload` | Always | 필수·비널 | **envelope-row** |
| `payload.command_id` | `Guid` | Always | 필수·비널 | payload |
| `payload.status` | `string` | Always | 필수, enum 2값 | payload |
| `payload.reason_code` | `string` | AllowNull | 필수·널 가능, enum 6값 | payload |

### `SESSION_READY` — envelope 6 (payload 포함) + payload 5 = 11

envelope는 위와 동일. payload:

| 필드 | C# 타입 | Required | 스키마 | counted |
|------|--------|---------|--------|---------|
| `payload.session_id` | `Guid` | Always | 필수·비널 | payload |
| `payload.world_id` | `Guid` | Always | 필수·비널 | payload |
| `payload.actor_id` | `Guid` | Always | 필수·비널 | payload |
| `payload.tick_hz` | `int` | Always | 필수, 1..1000 | payload |
| `payload.server_version` | `string` | Always | 필수, pattern | payload |

`tick_hz`의 `int`가 1..1000을 손실 없이 담는다(스키마 `minimum`이 1인데 `int`는 0도 담는다 — §0.5 #10의 "감지 불가"가 이것이다).

### `SESSION_OPENED` — envelope 11 (payload 제외, 실제 top-level 12) + payload 2 = 13

| 필드 | C# 타입 | Required | 스키마 | counted |
|------|--------|---------|--------|---------|
| `event_id` | `Guid` | Always | 필수·비널 | envelope |
| `event_type` | `string` (const) | Always | 필수·비널 | envelope |
| `schema_version` | `int` (const 1) | Always | 필수 | envelope |
| `world_id` | `Guid` | Always | 필수·비널 | envelope |
| `tick` | `long` | Always | 필수, 0..2^53-1 | envelope |
| `sequence` | `long` | Always | 필수, 0..2^53-1 | envelope |
| `occurred_at` | `string` | Always | 필수, GameTime | envelope |
| `recorded_at` | `string` | Always | 필수, RealTime | envelope |
| `correlation_id` | `Guid` | Always | 필수·비널 | envelope |
| `causation_id` | `Guid?` | AllowNull | 필수·널 가능 | envelope |
| `actor_id` | **`Guid`** | **Always** | 필수·**좁힘으로 비널** | envelope |
| `payload` | `SessionOpenedPayload` | Always | 필수·비널 | **세지 않음 (규약)** |
| `payload.session_id` | `Guid` | Always | 필수·비널 | payload |
| `payload.transport` | `string` | Always | 필수, enum 1값 | payload |

### `SESSION_CLOSED` — envelope 11 + payload 2 = 13

envelope는 `SESSION_OPENED`와 동일(`event_type` const만 다름). payload: `session_id` (`Guid`, Always), `close_reason` (`string`, Always, enum 6값).

**합계 9 + 11 + 13 + 13 = 46.** 대칭 규약(어디서나 `payload`를 1행으로)이면 48, 어디서도 안 세면 44다. 위 표가 있으면 어느 쪽으로 채점해도 같은 근거를 쓴다.

---

## 10. 측정치

| 항목 | 값 | p0-01 대비 |
|------|---|-----------|
| EditMode 전체 (어셈블리 재컴파일 포함) | **31초** | 콜드 63초 → 개선 |
| EditMode 전체 (웜) | **12초** | 웜 11초와 같은 수준(테스트 26→67건, 어셈블리 +2) |
| 실서버 Live 2건 (필터 실행) | **34초** | |
| 헤드리스 hold Editor 기동 → SESSION_READY | **약 35초** | QA가 A 단계 전에 확보해야 하는 시간 |
| Editor 프로젝트 로드 | 11.7초 (웜) / 21.2초 (콜드) | — |
| 생성기 1회 실행 | 0.6초 | — |
| `tools/codegen/verify` 빌드 | 1.2초, 경고 0 오류 0 | — |
| 테스트 건수 | **67** (p0-01: 26) | — |
| 컴파일 경고 | **0** | 유지 |

---

## 11. 실서버 검증 (SC-49 / SC-50 / SC-51) — 2026-09-19

서버: `server/target/debug/starfall-game-server.exe`, `start_tick=4461`, `tick_hz=20`, `world_id=01a0b1c2-3d4e-7f01-8a2b-9c0d1e2f3a4b`, `server_version=0.1.0`.
실행: `unity test client --mode EditMode --filter "LiveServerTests"` (`STARFALL_LIVE_TESTS=1`, `STARFALL_DEV_AUTH_SECRET=dev_only_not_a_secret`).
리포트: `_workspace/p0-02-networking-spike/unity-tests/Live.xml`(+`.nunit.xml`). **2 tests / 0 failures / 0 errors / 0 skipped**, 34초.

### 11.1 SC-49 — 3 왕복

연결의 **첫 프레임 원문**:

```json
{"message_id":"01a0b7ec-f356-7709-bec7-39c89565c366","message_type":"SESSION_READY","schema_version":1,
 "tick":7323,"correlation_id":"01a0b7ec-f356-7709-bec7-39c62f5ebdae",
 "payload":{"session_id":"01a0b7ec-f325-75f9-82d6-e42ca16e1337",
            "world_id":"01a0b1c2-3d4e-7f01-8a2b-9c0d1e2f3a4b",
            "actor_id":"01a0b1c2-7e57-7c11-8e57-000000000001",
            "tick_hz":20,"server_version":"0.1.0"}}
```

- `tick_hz == 20` ✓, `actor_id`가 인증된 주체와 문자열로 같다 ✓ (I-10 — 클라이언트는 주장하지 않고 통보받는다)
- 순서는 클라이언트의 대기 목록이 아니라 **수신 프레임 배열 인덱스**로 검증했다. 검증 대상이 곧 증거가 되면 안 되기 때문이다.

```
frame order ok: COMMAND_RESULT[1] -> PING_REPLY[2] for 01a0b7ec-f35a-7cc5-a862-25b930362e2a probe_seq=0
frame order ok: COMMAND_RESULT[3] -> PING_REPLY[4] for 01a0b7ec-f35a-7606-8d79-bbe7b7ebc0f5 probe_seq=1
frame order ok: COMMAND_RESULT[5] -> PING_REPLY[6] for 01a0b7ec-f35a-70f2-bbdb-b269f050358d probe_seq=2
command/reply pairs checked: 3
command … -> Completed in 57.0 / 57.1 / 57.1 ms
```

프레임 단위로 같이 확인한 것: `COMMAND_RESULT`가 `command_id`당 정확히 1건, `ACCEPTED`의 `reason_code`가 `null`(I-14), `COMMAND_RESULT`·`PING_REPLY`의 `correlation_id`가 `null`(스펙 §5.2).

**왕복 57 ms**는 이론 하한(평균 반 tick 25 ms + 전송)과 3 tick(150 ms) 사이다. 부하 없이 단독 연결에서 잰 값이고 **잠정 게이트 판정용이 아니다**(그것은 봇 시계로 재는 qa 소관, M-3).

### 11.2 SC-50 — DB 대조 (검사한 correlation 3개)

```sql
select correlation_id, event_type, tick, sequence, actor_id,
       payload->>'session_id', payload->>'transport', payload->>'close_reason', occurred_at
from domain_events where correlation_id = any('{…}'::uuid[]) order by correlation_id, tick, sequence;
```

| correlation_id | 이벤트 | tick | seq | session_id | close_reason |
|---|---|---:|---:|---|---|
| `01a0b7ec-f036-…c79c` | SESSION_OPENED | 7307 | 0 | `01a0b7ec-f02a-756e-aa88-33b4bec8eee3` | |
| | SESSION_CLOSED | 7312 | 0 | 같음 | `PROTOCOL_VIOLATION` |
| `01a0b7ec-f2c0-…a005` | SESSION_OPENED | 7320 | 0 | `01a0b7ec-f29f-72e8-8918-8c7137a65adf` | |
| | SESSION_CLOSED | 7322 | 0 | 같음 | `CLIENT_CLOSED` |
| `01a0b7ec-f356-…bdae` | SESSION_OPENED | 7323 | 0 | `01a0b7ec-f325-75f9-82d6-e42ca16e1337` | |
| | SESSION_CLOSED | 7325 | 0 | 같음 | `CLIENT_CLOSED` |

- 짝 검사 SQL(`having … <> 1`)이 **0행** — 3개 전부 1:1이고 고아·중복 없음
- `session_id`가 클라이언트 로그의 값과 전부 일치, `actor_id`가 셋 다 Unity 주체
- 정상 종료한 두 세션이 **`CLIENT_CLOSED`** — 내 종료 훅이 close 프레임을 실제로 보냈다는 서버 쪽 증거

### 11.3 SC-51 — 서버가 끊은 뒤 재연결

클라이언트가 스스로 끊으면 재연결 루프가 멈춰 아무것도 증명하지 못한다. 그래서 **서버가 끊게** 만들었다: 16 KiB를 넘는 프레임 12개를 보내 프로토콜 위반 예산을 소진시켰다(ADR-0005 §2). 서버는 `protocol_violations_total=9`를 세고 close 1002로 끊었다.

```
INFO starfall.net: SESSION_READY session_id=01a0b7ec-f02a-…eee3 correlation_id=01a0b7ec-f036-…c79c … attempt=0
WARN starfall.net: send failed - WebSocketException(ConnectionClosedPrematurely): …
INFO starfall.net: dropping 1 in-flight command(s) on disconnect (no resend, I-23)
INFO starfall.net: reconnect attempt=0 delay_ms=351 (counter resets only on SESSION_READY)
INFO starfall.net: disconnect detail: Remote code=1002
INFO starfall.net: socket open, waiting for SESSION_READY (attempt=1)
INFO starfall.net: SESSION_READY session_id=01a0b7ec-f29f-…5adf correlation_id=01a0b7ec-f2c0-…a005 … attempt=1
INFO starfall.net: closing session_id=01a0b7ec-f29f-…5adf reason=CLIENT_CLOSED code=1000
INFO starfall.net: dropping 0 in-flight command(s) on disconnect (no resend, I-23)
```

- **서로 다른 `session_id`·`correlation_id`** ✓ (재연결은 재개가 아니다 — I-23)
- 끊길 때 in-flight 1건이 `ConnectionLost`로 처리되고 **재전송되지 않았다.** 새 세션의 왕복은 새 `command_id`로 이뤄졌다(테스트가 `Is.Not.EqualTo(inFlight)`로 Assert)
- **시도 카운터**: 0 → (실패) → 1 → (SESSION_READY) → 0. TCP 연결 성공(`socket open … attempt=1`)에서는 리셋되지 않는다
- 백오프 실지연 **351 ms** — n=0의 상한 500 ms 안

## 12. U-5b — 도메인 리로드 시 `close_reason` (부분 측정)

**측정한 것 (각각 재현됨):**

| 상황 | 서버 기록 | 횟수 |
|------|----------|:---:|
| 클라이언트가 블로킹 close를 보냄 (`DisconnectBlocking`) | **`CLIENT_CLOSED`** | 3 |
| Editor 프로세스가 close 없이 사라짐 (batchmode `-quit` 종료) | **`TRANSPORT_ERROR`** | 2 |

두 번째가 중요하다: `unity run`은 `-quit`를 붙이므로 `-executeMethod`가 반환하는 즉시 Editor가 내려간다. 그때 연결은 close 프레임 없이 끊기고 서버는 `TRANSPORT_ERROR`로 기록한다(04:31:42, 04:37:04 두 번 관측). **즉 "훅이 안 돌면 `TRANSPORT_ERROR`"는 추측이 아니라 관측이다.**

**측정하지 못한 것:** Unity가 실제 도메인 리로드 중에 `AssemblyReloadEvents.beforeAssemblyReload`를 끝까지 돌려 close 프레임이 나가는지.

- batchmode에서 `EditorUtility.RequestScriptReload()`를 불러도 **리로드가 일어나기 전에 프로세스가 종료된다**(`ReloadProbe` 실행 결과: `beforeAssemblyReload` 로그 없음, `TRANSPORT_ERROR`).
- 대화형 Editor로 돌렸을 때 스크립트를 편집한 시각과 맞물려 `CLIENT_CLOSED` 세션 2건이 관측됐지만, 그 Editor의 `starfall-net.log` 기록이 남지 않아 **인과를 확정할 수 없다.** 추측을 측정으로 적지 않는다.

**운영상 결론은 어느 쪽이든 같다.** QA 절차에 이미 들어 있는 규칙이 이 불확실성을 덮는다: **A 단계가 도는 동안 `client/` 아래 파일을 저장하지 않는다.** 리로드가 아예 일어나지 않으면 이 질문은 부하 측정에 영향을 주지 않는다. 그리고 리로드가 일어나 31번째가 사라지면, `reason=EDITOR_RELOAD` 로그 줄의 유무로 "훅이 늦었다"와 "훅이 안 돌았다"를 사후에 구분할 수 있다.

**남은 확인 방법(다음 슬라이스, 1분짜리):** 사람이 Editor를 PlayMode로 띄워 접속한 상태에서 스크립트를 1바이트 고치고 저장 → DB의 그 세션 `close_reason`을 본다.

## 13. QA 자동화 — 31번째 연결 (SC-61 / AC-15)

### 13.1 `unity command editor_play`는 **쓸 수 없다**

```
$ unity command --project-path client
Error: No Pipeline instance found for project: C:\WorkSpace\SpaceHistoric\client.
       Make sure Unity Editor is running with the Pipeline package installed.
```

`client/Packages/manifest.json`에 `com.unity.pipeline`이 없다. `unity list`와 `unity run --command`도 같은 패키지에 의존한다. **패키지를 지금 추가하지 않았다** — Editor 안에서 RPC 서버를 돌리는 패키지를 지연 측정 직전에 넣는 것은 연결 하나를 얻자고 치를 비용이 아니다.

### 13.2 대신 쓸 것 — 패키지 없이 동작하는 헤드리스 hold (**실행 확인함**)

PlayMode도 필요 없다. 31번째 연결에 필요한 것은 *오래 사는 Editor 프로세스 안의 진짜 `Starfall.Net` 클라이언트*뿐이고, EditMode에서 메인 스레드로 펌프하면 `MonoBehaviour.Update`와 같은 일을 한다.

```bash
export STARFALL_DEV_AUTH_SECRET=dev_only_not_a_secret
export STARFALL_NET_HOLD_SENTINEL='C:\...\RELEASE'    # 이 파일이 생기면 정상 종료
export STARFALL_NET_HOLD_OUTPUT='C:\...\hold.csv'     # correlation_id,session_id,actor_id
export STARFALL_NET_HOLD_SECONDS=240                   # 안전 예산
unity run client --timeout 400 -- -executeMethod Starfall.Net.EditorTools.StarfallNetHold.HoldOpen
```

QA 절차:

```
1. (백그라운드로) 위 명령 실행
2. hold.csv 가 생길 때까지 폴링 — 생기면 SESSION_READY 완료. 첫 필드가 correlation_id다
   → 그대로 correlations.txt 에 넣으면 31번째가 된다
3. 봇 30개 시작 (A 단계)
4. A 종료 후 RELEASE 파일을 touch → 정상 close → Editor 종료 (exit 0)
```

**실행 증거 (2026-09-19):**

```
hold.csv: 01a0b7f3-94fe-76d3-80e3-6aff39de5ac0,01a0b7f3-94d2-7633-8ec0-e51d4e7c7753,01a0b1c2-7e57-7c11-8e57-000000000001
연결 유지 중 /debug/stats: ws_connections=1, live_connections=1
연결 유지 중 SQL:  opened=1  closed=0        ← SC-61 이 요구하는 바로 그 모양
RELEASE touch 후 2초 내 종료, DB: SESSION_CLOSED / CLIENT_CLOSED
```

- Editor 기동 ~35초. **A 단계 시작 전에 hold.csv 를 기다리는 것으로 순서가 자동으로 지켜진다.**
- `unity open`도 되긴 하지만(대화형 Editor가 `-executeMethod`를 받는다) 기동에 90초 넘게 걸리고 종료가 사람 손에 달려 있어 자동화에는 `unity run` 쪽이 낫다.
- **주의: 프로세스를 죽이지 말 것.** 죽이면 close 프레임이 안 나가 `TRANSPORT_ERROR`가 되고, 그 세션은 "서버가 먼저 닫은 연결"처럼 보인다(SC-52 판정을 오염시킨다). 반드시 sentinel로 끝낸다.

### 13.3 사람이 PlayMode로 해야 하는 경우 (대체 경로)

`STARFALL_NET_AUTOCONNECT=1`을 export한 셸에서 Editor를 열고 Play를 누른다. 그 경우 `StarfallNetHost`가 붙고, `Starfall/Net/*` 메뉴로 ping·종료를 제어한다. 자동화가 아니라 백업 절차다.

## 14. 알려진 한계

1. **U-5b 부분 측정.** §12. 실제 도메인 리로드 중 훅 완주는 사람 손이 필요한 1분짜리 확인으로 남았다.
2. **SC-51의 끊김 계기가 프로토콜 위반이다.** 서버 재시작으로 끊는 쪽이 AC-14 문구에 더 가깝지만, 실행 중인 테스트와 서버 프로세스를 동시에 조종해야 해서 오탐이 늘어난다. 서버가 주도해 끊고 클라이언트가 스스로 돌아온다는 성질은 동일하게 증명된다.
3. **프레임당 할당이 0은 아니다.** 수신 버퍼와 조립 스트림과 직렬화기는 재사용하지만, `Encoding.UTF8.GetString`의 문자열 1개와 `JObject` 트리는 프레임마다 생긴다. Newtonsoft + JSON에서 이걸 없애려면 `Utf8JsonReader`급 재작성이 필요하고, 이 슬라이스의 부하(초당 2 프레임)에서는 측정할 가치가 없다. **프로파일러로 재지 않았다** — 규약 준수만 주장한다.
4. **송신 큐 포화 시 연결을 닫지 않는다.** 서버는 포화 시 연결을 끊지만(ADR-0006 §5), 클라이언트는 `Send`가 `false`를 반환하고 호출자가 판단한다. 클라이언트 송신 큐는 서버 tick을 인질로 잡지 않으므로 같은 정책이 필요 없다고 판단했다. 스펙에 요구가 없어 임의로 늘리지 않았다.
5. **`Starfall.Net`은 `verify` 컴파일 게이트 밖이다.** `UnityEngine` 참조가 필요해 Unity 밖에서 컴파일할 수 없다. 대신 EditMode 실행이 컴파일 검증을 겸한다.
6. **`.meta` 파일은 Unity가 만들었다.** 새 스크립트·asmdef의 `.meta`가 생성되어 있고 추적 대상이다(커밋은 하지 않았다).

## 15. 스펙·계약에서 틀렸다고 판단한 부분

**없다.** §0.5의 16행 층별 표, AC-11, AC-12는 실행 결과와 전부 일치했다.

한 가지 **정정이 아니라 기록**: AC-21의 46은 대칭 규약이 아니다(§9). 총계를 바꾸자는 요청은 하지 않는다 — 표가 있으면 채점이 총계에 의존하지 않는다.

## 16. 다음에 필요한 것

| 대상 | 요청 |
|------|------|
| server | 없음. 접속·인증·종료 전부 `03_server_impl.md` 대로 동작했다. 이 라운드에서 서버 쪽 결함 없음 |
| qa | 봇 신원 30개를 출력하는 서브커맨드가 있으면 `01a0b1c2-7e57-7c11-8e57-000000000001`과의 disjoint를 한 줄로 확인할 수 있다. 없으면 `actor_id` 집합 대조로 대체 |
| qa | SC-61 절차는 §1.4. **A가 도는 동안 `client/` 아래 파일을 저장하지 않는다**를 실행 절차에 넣어 달라(도메인 리로드가 31번째 연결을 끊는다) |

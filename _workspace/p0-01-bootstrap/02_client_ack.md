# client 스프린트 계약 확인 (p0-01-bootstrap)

- 작성: client, 2026-09-18 (구현 착수 전)
- 대상: `02_sprint_contract.md` SC-19 ~ SC-30, 기록 항목 M-7·M-8·M-9
- 읽은 확정본: `docs/specs/p0-01-bootstrap.md`(AC 개정판), `01_architect_decisions.md`, `01_architect_tasks.md`(T5~T8), `docs/adr/0002` §4·§5, `contracts/**`(반례 7건 확인)

## 결론

**차단급 이의 없음. 12항목 전부 이 방법으로 완료를 증명할 수 있다.** 아래 확인란에 표시하고 바로 T6 → T5 → T7 순으로 착수한다.

architect 결정 3건은 모두 수용한다.
- ① 정수 범위 기반 매핑 채택 → `probe_seq`가 `uint`가 되어 C# 책임 반례가 5건이 된다. 계약에서 `format`이 빠진 것도 확인했다(`SchemaVersion`, `SafeInteger`, `NonNegativeSafeInteger`, 두 payload의 `probe_seq`). 생성기는 `minimum`/`maximum`만 보고 매핑하며, **정수인데 범위 선언이 없으면 실패**시킨다. 문자열의 `format: uuid`는 유지되었으므로 `Guid` 매핑 근거로 쓴다.
- ② 시리얼라이저는 문서상 2프로필, 코드는 `Strict`만. 동의한다. 다만 `ContractJson` 클래스에 `Runtime` 자리를 주석이 아니라 **`NotImplementedException`을 던지는 프로퍼티**로 남기겠다 — p0-02에서 "어디에 만들어야 하는지"가 코드로 보인다.
- ③ `collab-proxy` + `visualscripting` 제거, `timeline`·`ai.navigation`·`inputsystem` 유지. 동의한다. 제거 후 해석 오류 여부를 M-9로 기록한다.

`tick`은 `long`이라 상한 위반(`tick-above-safe-integer`)을 C#이 잡지 못한다는 비대칭도 확인했다. **의도된 설계로 받아들이고**, SC-25가 요구한 대로 테스트가 이 2건을 "감지 불가"로 명시적으로 드러내게 만든다(조용히 빠뜨리지 않는다).

## 항목별 확인

| ID | 확인 | 비고 |
|----|------|------|
| SC-19 결정적 출력 | 동의 | 속성·파일 모두 `StringComparer.Ordinal` 정렬, LF, BOM 없음, 타임스탬프·도구 버전 미출력 |
| SC-20 변조 감지 | 동의 | 내용 불일치 파일의 **레포 상대 경로**를 한 줄씩 출력, 종료 코드 1 |
| SC-21 고아 파일 감지 | 동의 (아래 쟁점 1 참조) | `--out` 아래 `*.cs`를 **재귀로** 열거해 생성 목록과 집합 비교 |
| SC-22 F-1 종료 0 · 리포트 2개 | 동의 | `--report-format both` 고정. 실행 후 두 파일 존재를 직접 확인해 T8에 적는다 |
| SC-23 유효 4건 왕복 | 동의 | `[TestCaseSource]`로 순회하고 **순회 개수(4)를 Assert**한다(빈 순회 방지, §0.3) |
| SC-24 C# 책임 반례 5건 거부 | 동의 | 반례 #1,3,4,5,6. 각 건의 파일명과 예외 메시지를 `TestContext.WriteLine`으로 남긴다 |
| SC-25 감지 불가 2건 기록 | 동의 | #2 `command-id-not-v7`, #7 `tick-above-safe-integer`. **"통과함"을 Assert하는 테스트**로 만들어, 나중에 이게 거부되기 시작하면 테스트가 빨개져 architect 통지 사유가 드러나게 한다 |
| SC-26 시간 문자열 보존 | 동의 | 한 테스트 안에서 안전 리더(`2026-09-17T14:05:09.123Z` 유지)와 기본 `JObject.Parse`(`09/17/2026 14:05:09`로 변형) 두 값을 모두 Assert·출력 |
| SC-27 UUIDv7 패턴·중복 없음 | 동의 | N은 **10,000**으로 잡고 증거에 개수를 남긴다. 같은 밀리초 보장을 위해 루프 안에서 시각을 1회만 읽는 경로를 테스트한다 |
| SC-28 fixture 로더 마커·4건 가드 | 동의 | `contracts/registry/types.json` 마커 상향 탐색. 못 찾거나 4건 미만이면 `Assert.Fail`(Skip/Ignore 아님). 센 개수를 출력 |
| SC-29 콜드 임포트 | 동의 (아래 쟁점 2 참조) | G-e대로 **맨 마지막에** 실행 |
| SC-30 `ProjectVersion.txt` | 동의 | `6000.6.1f1` |

기록 항목 M-7(콜드/웜 테스트 시간), M-8(`companyName`/`productName` 원래 값·설정 후 값·Hub 등록 여부), M-9(패키지 2종 제거 영향)는 `03_client_impl.md`에 숫자·결과로 남긴다.

## QA가 물은 쟁점 3건

### 쟁점 1 — `--out`에 임의 경로를 줄 수 있나 (SC-20/21)

**가능하다. QA는 스크래치 사본에서 재현하면 되고 `client/`를 건드릴 필요가 없다.** `--out`은 임의의 절대·상대 경로를 받고, 없으면 만든다. 계약 읽기(`--contracts`)와 출력(`--out`)은 완전히 독립이다.

다만 QA 스크립트가 그대로 돌아가려면 **한 가지 전제**가 필요해서 그렇게 구현한다.

> `--check`의 파일 집합 비교는 **`*.cs`만 본다. `*.meta`는 세지도, 고아로 신고하지도 않는다.**

이유: QA의 E-2/E-3은 `cp -r "$GEN" "$SCRATCH/gen"`으로 실물 `Generated/`를 통째로 복사한다. 실물에는 Unity가 만든 `.cs.meta`가 같이 있으므로, 만약 `--check`가 모든 파일을 세면 **아직 아무 변조도 하지 않은 깨끗한 사본에서 `.meta`들이 전부 고아로 잡혀 "clean exit=0" 기대가 깨진다.** `.cs`만 보면 QA 스크립트가 적힌 그대로 동작한다. ADR-0002 §4의 "생성기는 `.meta`를 만들거나 지우지 않는다"와도 일관된다.

또 하나: **평소 실행(`--check` 없이)은 자기가 만들지 않은 `.cs`를 삭제한다**(`.meta`는 남긴다 — Unity가 정리). `Generated/`는 생성기 소유이기 때문이다. 삭제한 파일 경로는 출력한다. QA의 E-3 흐름(`cp -r "$GEN"/. "$SCRATCH/gen"/` 후 `QaOrphan.cs` 추가 → `--check`)은 **`--check`만 쓰므로 영향 없다.**

### 쟁점 2 — `unity test`가 쓰는 Editor 로그 경로 (SC-29)

**QA가 적은 `C:\Users\CHOISOOYEON\AppData\Local\Unity\Editor\Editor.log`가 Windows Unity Editor의 기본 경로가 맞다.** 다만 `unity test`는 에디터를 배치 모드로 직접 띄우므로 CLI가 `-logFile`로 다른 경로를 지정할 가능성이 있고, `unity test --help`에는 로그 옵션이 없어 **도움말만으로는 확정할 수 없다.**

그래서 이렇게 하겠다.

1. T7에서 실제로 실행한 뒤 **Editor.log의 수정 시각이 갱신되었는지 확인**하고, 실제 경로를 `03_client_impl.md`에 적는다. CLI가 다른 경로를 출력하면 그 경로를 정본으로 적는다.
2. **보험으로 CLI stdout/stderr를 `_workspace/p0-01-bootstrap/unity-tests/unity-test-stdout.txt`로 함께 남긴다.** 배치 모드 컴파일 에러·경고는 대개 CLI 출력에도 올라오므로, 로그 경로가 어긋나도 SC-29의 증거가 사라지지 않는다.
3. `Editor.log`는 실행마다 덮어쓰이고 직전 것은 `Editor-prev.log`로 밀린다. **콜드 임포트 실행 직후에 바로 세야** 한다 — 그 뒤에 다른 Unity 실행이 끼면 증거가 밀려난다. G-e(콜드 임포트를 맨 마지막에)와 함께 지키면 문제없다.

경고 집계식 `grep "warning CS" "$LOG" | grep -c "Assets/_Project"`는 Unity가 로그에 정방향 슬래시(`Assets/_Project/...`)를 쓰므로 그대로 동작할 것으로 본다. 실측 후 다르면 T8에 적는다.

### 쟁점 3 — D 항목(클라이언트 테스트) 이름

**QA 권장안을 그대로 쓴다.** 새로 짓지 않겠다 — 계약 문서와 테스트 이름이 어긋나면 QA가 한 번 더 대조해야 한다. 접두사는 아래로 고정하고, `03_client_impl.md`에 `SC ID → 테스트 전체 이름` 대응표를 싣는다.

| SC | 테스트 이름 |
|----|------------|
| SC-23 | `Fixtures_RoundTrip_MatchesOriginal` (TestCaseSource 4건) + `Fixtures_RoundTrip_VisitedAllFourValidFixtures` |
| SC-24 | `Invalid_Rejected_ByStrictProfile` (TestCaseSource 5건) + `Invalid_Rejected_VisitedAllFiveCSharpCases` |
| SC-25 | `Invalid_NotDetectableByCSharp_DocumentedAsymmetry` (TestCaseSource 2건) |
| SC-26 | `Dispatch_PreservesRealTime` |
| SC-27 | `UuidV7_MatchesSchemaPattern`, `UuidV7_NoDuplicatesWithinSameMillisecond` |
| SC-28 | `FixtureLoader_FindsRepoRootByMarker`, `FixtureLoader_FailsWhenFewerThanFourValidFixtures` |

전체 이름은 `Starfall.Tests.EditMode.ContractFixtureTests.<위 이름>` 형태가 된다(정확한 값은 리포트 XML에서 확인해 T8에 싣는다).

## 수정 요청 / 메모 (차단급 아님)

1. **SC-21의 "그 파일 경로"** — 절대 경로가 아니라 `--out` 기준 상대 경로 + 레포 상대 경로를 함께 출력한다(스크래치 사본에서 돌릴 때 어느 디렉토리인지 헷갈리지 않게). QA 스크립트의 `grep`은 파일명(`QaOrphan.cs`)으로 하면 양쪽 모두에서 걸린다.
2. **SC-22/SC-29의 리포트 경로** — `_workspace/p0-01-bootstrap/unity-tests/`가 없으면 `unity test`가 리포트를 못 쓸 수 있어, T7에서 디렉토리를 먼저 만든다.
3. **SC-27의 N** — 계약에는 N이 정해져 있지 않다. 10,000으로 잡되, 이건 "같은 밀리초 안 중복 없음"을 보는 것이지 성능 측정이 아니다. QA가 다른 수를 원하면 알려 달라(테스트 상수 하나만 바꾸면 된다).
4. **SC-23의 "원본과 동일"** — `JToken.DeepEquals`로 비교하며, 원본과 재직렬화 결과를 **둘 다 `DateParseHandling.None` 리더로 파싱해서** 비교한다. 기본 리더로 원본을 열면 비교 전에 이미 날짜가 변형되어 비교 자체가 무의미해진다(리뷰 단계에서 실행 확인한 함정).

## 확인란

| 영역 | 항목 | 담당 | 확인 | 서명/날짜 |
|------|------|------|------|----------|
| E codegen | SC-19 ~ SC-21 | client | ☑ | client / 2026-09-18 |
| F Unity 클라이언트 | SC-22 ~ SC-30 | client | ☑ | client / 2026-09-18 |

계약 변경 요청 없음. `contracts/**`는 현 상태로 구현에 충분하다.

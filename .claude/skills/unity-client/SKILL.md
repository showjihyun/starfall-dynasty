---
name: unity-client
description: "STARFALL DYNASTY Unity 6(C#) 클라이언트 구현 규약. client/ 프로젝트 구조와 asmdef 의존 방향, 서버 권위 구조의 클라이언트 역할(명령 전송·상태 미러·자기 함선 예측과 보정·타 엔티티 보간), WebSocket/REST 전송 추상화(PC·WebGL), 메인 스레드 마샬링, 계약 DTO와 fixture 테스트, UI Toolkit 기반 인벤토리·시장·Chronicle·Biography, Unity CLI·MCP로 Editor 조작, EditMode/PlayMode 테스트, GC·WebGL 주의점. client/ 폴더의 C# 스크립트·씬·프리팹·UI·클라이언트 네트워킹 작업 시 반드시 사용. 렌더링·셰이더·VFX는 space-3d-rendering 스킬."
---

# Unity Client — 서버의 사실을 보여주고, 판정하지 않는 클라이언트

클라이언트는 **표현(presentation)과 예측(prediction)** 을 맡는다. 판정은 서버가 한다. 이 경계가 무너지면 해킹 가능한 경제, 서버와 다른 화면, 재현 불가능한 버그가 생긴다.

Unity 공식 플러그인 스킬을 함께 쓴다: `/unity:unity-cli`(Editor 제어·테스트·빌드), `/unity:new-unity-project`(최초 생성), `/unity:unity-package-management`, `/unity:ui-uitk`, `/unity:physics-3d-collision`, `/unity:localization`, `/unity:optimize-web`.

## 1. 프로젝트 구조

```
client/                                   # Unity 6 프로젝트 루트
├── Packages/manifest.json                # 패키지 버전 고정
└── Assets/_Project/
    ├── Scripts/
    │   ├── Core/                 Starfall.Core           부트스트랩, 로깅, 설정, 시간
    │   ├── Contracts/Generated/  Starfall.Contracts      계약 DTO (생성물, 수정 금지)
    │   ├── Net/                  Starfall.Net            전송 추상화, REST, WebSocket, 메시지 디스패치, 명령 전송
    │   ├── World/                Starfall.World          서버 상태 미러, 보간, 예측·보정
    │   ├── Gameplay/             Starfall.Gameplay       함선 조작 입력, 채굴·전투 표현 트리거
    │   └── History/              Starfall.History        Chronicle·Biography 뷰모델
    ├── UI/                       Starfall.UI             UXML, USS, 화면 컨트롤러
    ├── Scenes/  Prefabs/
    ├── Tests/EditMode/  Tests/PlayMode/
    └── Art/ Shaders/ VFX/ Rendering/                     (unity-tech-artist 소유)
```

의존 방향: `Contracts ← Net ← World ← Gameplay / History ← UI`, `Core`는 모두가 참조. 역방향 참조가 필요해지면 인터페이스나 이벤트로 끊는다. asmdef로 나누는 이유는 컴파일 시간, 의존 방향 강제, 테스트 어셈블리 분리다.

기본 패키지: URP, Input System, Newtonsoft Json(`com.unity.nuget.newtonsoft-json`), Addressables, Cinemachine, Test Framework. Entities(ECS)는 측정된 병목과 ADR이 있을 때만 추가한다.

## 2. 서버 권위 구조에서 클라이언트가 하는 일

| 대상 | 클라이언트 처리 |
|------|---------------|
| 인벤토리·잔액·거래 결과 | 서버 결과가 올 때까지 "처리 중" 표시. 로컬에서 먼저 더하거나 빼지 않는다 |
| 자기 함선 이동 | 입력 즉시 예측 이동 → 서버 스냅샷과 비교 → 오차가 작으면 부드럽게 보정, 크면 순간 이동 |
| 다른 함선·NPC | 스냅샷 버퍼(약 2 tick 지연)로 보간. 외삽은 짧게 제한 |
| 전투 | 발사 연출은 즉시(외형만), 명중·피해·파괴는 서버 이벤트로만 |
| 채굴 | 요청 → `COMMAND_RESULT` → 인벤토리 변경 이벤트 도착 시 반영 |

명령 전송 규칙:
- 명령마다 `command_id`(Guid)를 만들고 대기 목록에 넣는다. 타임아웃 재시도는 **같은 `command_id`** 로 한다. 서버는 이 ID로 중복을 거른다.
- `COMMAND_RESULT: accepted`는 "접수됨"이지 "완료됨"이 아니다. 실제 결과는 이어서 오는 이벤트·스냅샷으로 판단한다.
- 클라이언트가 보내는 위치·수량은 서버가 믿지 않는다는 전제로 설계한다. 의도(목표 지점, 대상 ID)만 보낸다.

## 3. 네트워크 계층

- 전송 추상화 `IRealtimeTransport`(연결, 송신, 수신 콜백, 종료)를 두고 플랫폼별 구현을 분리한다.
  - PC: `System.Net.WebSockets.ClientWebSocket`
  - WebGL: .NET 소켓 API를 쓸 수 없으므로 JavaScript 브라우저 WebSocket을 `.jslib` 인터롭으로 감싼 구현
- REST는 `UnityWebRequest` 기반 `IApiClient`로 감싼다 (PC·WebGL 공통으로 동작).
- 신뢰성이 필요한 요청(로그인, 인벤토리, 거래, 역사 조회)과 실시간 상태(위치, 전투)를 논리적으로 분리한다. 나중에 실시간 채널만 다른 전송으로 바꿀 수 있게 하기 위해서다 (기획안 TECH §7).
- 수신 콜백은 백그라운드 스레드에서 올 수 있다. Unity API는 메인 스레드에서만 호출되므로, 수신 메시지는 스레드 안전 큐에 넣고 메인 스레드(`Update` 또는 `await Awaitable.MainThreadAsync()`)에서 처리한다.
- 메시지 디스패치는 `message_type → 핸들러` 등록 방식. 모르는 타입은 경고 로그만 남기고 계속 동작한다 (서버가 먼저 배포될 수 있다).
- 비동기는 Unity 6 `Awaitable`을 쓰고 `destroyCancellationToken`으로 취소한다.

## 4. 계약과 DTO

- DTO는 `contracts/`에서 생성해 `Scripts/Contracts/Generated/`에 둔다. 손으로 고치면 다음 생성 때 사라지고 서버와 어긋난다.
- 직렬화 이름은 `[JsonProperty("snake_case")]`로 명시한다.
- UI와 게임플레이는 DTO를 직접 쓰지 않고 뷰모델로 변환해 쓴다. 계약이 바뀌어도 변환 한 곳만 고치면 된다.
- **fixture 테스트(필수):** `contracts/fixtures/**`의 모든 JSON을 대응 DTO로 역직렬화하는 EditMode 테스트. 경로는 `Path.Combine(Application.dataPath, "../../contracts/fixtures")`.
- 서버 준비 전에는 fixture를 재생하는 가짜 전송(`FakeTransport`)으로 UI와 게임플레이를 개발한다.

## 5. UI (UI Toolkit)

- HUD, 인벤토리, 시장, 스캐너, Galactic Chronicle, Biography는 UXML/USS로 만든다. 세부 방법은 `/unity:ui-uitk`.
- Chronicle·Biography는 서버 프로젝션 API의 페이지네이션을 그대로 따른다. 클라이언트에서 전체 역사를 받아 정렬하지 않는다.
- 역사 문장은 서버가 주는 `headline_key + params`를 로컬라이즈해서 만든다. 문장을 코드에 하드코딩하지 않는다.
- 각 역사 항목에는 "근거 보기"로 이어지는 evidence ID를 유지한다 (기획안 HSE §75).

## 6. Editor 조작과 파일 편집

- Editor가 켜져 있으면 `unity status`로 연결을 확인하고, 씬·프리팹·GameObject 변경은 `unity command` 또는 Unity MCP 도구로 한다. `.unity`/`.prefab` YAML을 손으로 고치면 GUID 참조가 쉽게 깨진다.
- Editor가 없으면 C# 스크립트·UXML·USS·asmdef만 파일로 작성하고, 씬 조립은 "Editor 필요" 작업으로 남긴다.
- `.meta` 파일을 지우거나 새로 만들지 않는다. Unity가 관리한다.
- 컴파일 에러가 있으면 Editor가 Safe Mode로 뜨고 CLI가 연결되지 않는다. 컴파일 에러부터 고친다.
- Unity MCP를 Claude Code에 연결하려면 Unity CLI 설치 후 `unity mcp configure claude-code --project-path client`를 실행한다 (프로젝트에 `com.unity.pipeline` 패키지 필요 — `unity pipeline install`). 세부는 `/unity:unity-cli`.

## 7. 테스트와 완료 기준

| 종류 | 대상 | 실행 |
|------|------|------|
| EditMode | fixture 역직렬화, 메시지 디스패치, 예측·보간 계산, 뷰모델 변환 | `unity test client --mode EditMode` |
| PlayMode | 씬 로드, FakeTransport로 채굴·거래 흐름, UI 바인딩 | `unity test client --mode PlayMode` |

- 결과는 `--report-format junit --output _workspace/{slice-id}/unity-tests/{mode}.xml`로 남겨 qa가 증거로 쓰게 한다.
- 완료 전: 컴파일 경고 신규 0건, EditMode 테스트 통과, 핫 경로(`Update`, 네트워크 수신)에서 매 프레임 할당 없음(LINQ·문자열 결합·박싱 금지, 버퍼 재사용).
- Unity CLI나 Editor가 없어 실행하지 못했다면 "미검증(환경)"으로 보고한다.

## 8. WebGL 주의 (보조 플랫폼)

PC Standalone이 기준이다. WebGL용 코드 경로는 추상화 뒤에 격리한다.
- 스레드·`System.Net.Sockets`·`ClientWebSocket` 사용 불가 → 전송 추상화의 WebGL 구현 사용
- 메모리·다운로드 크기 예산 → Addressables로 지연 로드
- 최적화 절차는 `/unity:optimize-web`

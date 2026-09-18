# 0001. 레포·모듈 구조

- 상태: accepted (server·client 검토 완료, 2026-09-18)
- 날짜: 2026-09-17 (검토 반영 2026-09-18)
- 슬라이스: p0-01-bootstrap
- 검토: `_workspace/p0-01-bootstrap/01_server_adr_review.md`(A-1~A-5, R-11), `_workspace/p0-01-bootstrap/01_client_adr_review.md`(A-1~A-3, D-2). 반영 내역은 `01_architect_decisions.md`

## 맥락

Unity(C#) 클라이언트와 Rust 서버가 한 레포에 산다. 여러 에이전트가 병렬로 작업하므로 **디렉토리 경계가 곧 파일 소유권 경계**여야 하고, 나중에 바꾸기 가장 비싼 것이 이 경계다. 기획안은 모듈형 모놀리스로 시작하라고 못박았다(TECH §28, CLAUDE.md 원칙 7). 역사 엔진은 나중에 분리될 수 있으므로(HSE §96–97) 지금부터 별도 모듈 경계를 가져야 한다.

## 결정

### 1. 최상위 구조 (모노레포)

```
/
├── CLAUDE.md  README.md  .gitignore  .gitattributes  .editorconfig
├── docker-compose.yml  .env.example  rust-toolchain.toml
├── 기획안/                    제품 의도 원천 (읽기 전용)
├── docs/{adr,specs,design}/   결정·스펙·디자인
├── contracts/                 Unity↔Rust 데이터 계약 (단일 진실)
├── data/                      게임 데이터 테이블 (designer)
├── server/                    Rust 워크스페이스
├── client/                    Unity 6 프로젝트 루트
├── tools/{codegen,bots}/      계약 코드 생성기, 부하·시나리오 봇
├── tests/e2e/                 서비스 경계를 넘는 시나리오 테스트 (qa)
└── _workspace/                슬라이스별 에이전트 산출물 (추적함)
```

폴리레포로 나누지 않는다. 계약(`contracts/`)이 서버·클라이언트와 **같은 커밋에서** 바뀌어야 경계면 드리프트를 커밋 단위로 잡을 수 있다.

### 2. Rust 워크스페이스 (`server/`)

`rust-authoritative-server` 스킬 §1의 기본안을 채택하되 두 가지를 바꾼다.

```
server/
├── Cargo.toml              [workspace] + [workspace.dependencies] + [workspace.lints]
├── Cargo.lock              추적
├── crates/
│   ├── contracts/          contracts/ 대응 serde 타입 + fixture·스키마 테스트
│   ├── domain/             엔티티·명령·도메인 이벤트·규칙 (IO 없음)
│   ├── sim/                tick 루프·시스템 (IO 없음, 결정적)
│   ├── history/            역사 엔진 (history-engine-engineer 소유)
│   ├── persistence/        sqlx 저장소·outbox·트랜잭션
│   └── gateway/            Axum REST/WebSocket·인증·세션
├── bins/game-server/       조립 바이너리 (설정, tracing, 런타임 기동)
└── migrations/             sqlx 마이그레이션
```

**변경 1 — 패키지 이름에 `starfall-` 접두사.** 디렉토리는 `crates/sim`, 패키지 이름은 `starfall-sim`, 코드에서는 `starfall_sim`. `domain`, `history`, `contracts`, `sim`은 모두 crates.io에 같은 이름이 존재해 `cargo add`·docs·IDE에서 혼동이 생긴다. 명령은 `cargo test -p starfall-contracts` 형태가 된다(스킬 예시의 `-p contracts`는 이 접두사를 붙여 읽는다).

**변경 2 — 부트스트랩에서는 실제로 쓰는 크레이트만 만든다.** p0-01은 `starfall-contracts`, `starfall-gateway`, `bins/game-server`만 만든다. `domain`·`sim`·`persistence`·`history`는 이름·위치를 여기서 고정하고, 처음 쓰는 슬라이스에서 만든다. 빈 크레이트는 `cargo test`·clippy 시간만 쓰고 아무 것도 증명하지 않는다.

의존 방향: `domain ← sim ← gateway/persistence`, `contracts`는 누구나, `history`는 `domain`에만 의존한다. 역방향 의존이 필요해지면 트레이트로 끊고 ADR로 남긴다.

**`starfall-gateway`에는 게임 규칙과 월드 상태를 두지 않는다.** 첫 게임 상태 코드가 `starfall-domain`을 만든다. 변경 2의 부작용으로 p0-01에 존재하는 유일한 로직 크레이트가 gateway가 되는데, 다음 슬라이스에서 규칙을 넣기 가장 쉬운 자리가 바로 거기다. 그 길을 한 번 열면 "단일 크레이트 서버"를 버린 이유(아래 대안 절)가 문서에만 남는다.

`[lib] name`을 따로 지정해 짧게 만들지 않는다. 패키지 이름과 crate 이름이 갈라지면 접두사의 이점(백트레이스·`cargo tree`·IDE에서의 식별)이 절반 사라진다. 호출부가 장황하면 파일 단위로 `use starfall_sim as sim;`을 쓴다.

### 3. Unity 어셈블리 (`client/`)

`unity-client` 스킬 §1의 구조와 의존 방향(`Contracts ← Net ← World ← Gameplay/History ← UI`, `Core`는 공통)을 그대로 채택한다. 부트스트랩에서 만드는 어셈블리는 두 개다.

| asmdef | 경로 | 비고 |
|--------|------|------|
| `Starfall.Contracts` | `Assets/_Project/Scripts/Contracts/` | `Generated/`(생성물, 수정 금지)와 손으로 쓴 계약 전용 시리얼라이저·UUIDv7 헬퍼가 함께 산다. `noEngineReferences: true` + `overrideReferences: true` + `precompiledReferences: ["Newtonsoft.Json.dll"]` |
| `Starfall.Tests.EditMode` | `Assets/_Project/Tests/EditMode/` | 테스트 어셈블리(Editor 플랫폼), Contracts 참조. `overrideReferences: true` + `precompiledReferences: ["nunit.framework.dll", "Newtonsoft.Json.dll"]` |

`overrideReferences: true`가 없으면 `precompiledReferences`는 **읽히지 않는다**. Unity는 Auto Referenced DLL을 전부 자동 참조하므로 "명시적으로 참조했다"고 착각한 채 우연히 컴파일되고, 나중에 누가 Override References를 켜는 순간 조용히 깨진다(client 검토 A-1: Newtonsoft 패키지의 두 DLL 모두 `isExplicitlyReferenced: 0`임을 확인). 테스트 어셈블리에서 `nunit.framework.dll`을 빠뜨리면 같은 순간 NUnit이 사라진다.

`noEngineReferences: true`의 이득은 "Unity 없이 컴파일"이 **아니다**(asmdef는 Unity 컴파일 파이프라인 전용이다). 실제 이득은 **생성된 DTO에 엔진 타입(`Vector3`, `Debug`, `Application` 등)이 섞여 들어가는 것을 컴파일러가 막는 것**이다. 그 부작용으로 이 어셈블리는 `UnityEngine.Scripting.PreserveAttribute`를 쓸 수 없으므로, IL2CPP 스트리핑 대비(DTO 속성은 Newtonsoft가 리플렉션으로만 접근한다)는 `Assets/link.xml` 또는 자체 `PreserveAttribute`로 한다 — **실제 플레이어 빌드가 생기는 슬라이스의 과제**이며 지금은 기록만 한다. Newtonsoft 패키지가 넣어 주는 `link.xml`은 `System.ComponentModel` 컨버터만 보존하고 우리 DTO는 보존하지 않는다.

나머지(`Starfall.Core/Net/World/Gameplay/History/UI`)는 이름·경로만 고정하고 처음 쓰는 슬라이스에서 만든다. 테스트 어셈블리는 당분간 EditMode/PlayMode 각각 하나씩 두고, 컴파일 시간이 문제가 될 때 영역별로 쪼갠다.

### 4. 파일 소유권 (starfall-dev 스킬 표에 대한 델타)

| 경로 | 소유자 | 비고 |
|------|-------|------|
| `README.md`, `.gitignore`, `.gitattributes`, `.editorconfig` | architect | 레포 정책 파일 |
| `docker-compose.yml`, `.env.example`, `rust-toolchain.toml` | server | 로컬 인프라·툴체인 |
| `server/**` (history 제외), `server/migrations/**` | server | |
| `server/crates/history/**`, 역사 테이블 마이그레이션 | history | |
| `client/ProjectSettings/**`, `client/Packages/**` | client | techart는 client에게 요청 |
| `client/Assets/_Project/{Scripts,UI,Tests}/**` | client | `Scripts/Contracts/Generated/**`는 생성물 |
| `client/Assets/_Project/{Art,Shaders,VFX,Rendering}/**`, `client/Assets/Settings/**` | techart | 부트스트랩에서 템플릿이 만든 URP 에셋 포함 |
| `tools/codegen/**` | client | 출력물이 C#이므로 소비자가 소유 |
| `tools/bots/**`, `tests/e2e/**` | qa | |
| `contracts/**`, `docs/{adr,specs}/**` | architect | |

`rust-toolchain.toml`을 **레포 루트**에 둔다(스킬 기본안은 `server/`). rustup은 상위 디렉토리를 따라 올라가며 찾으므로, 루트에 두면 `server/`와 qa 소유의 `tools/bots/`가 같은 버전을 쓰고 두 파일이 어긋날 일이 없다.

`tools/bots/`는 `server/`와 **별도 Cargo 워크스페이스**로 두고 `starfall-contracts`를 path 의존으로 쓴다. 한 워크스페이스로 합치면 루트 `Cargo.toml`을 server와 qa가 함께 고쳐야 한다.

## 검토한 대안과 버린 이유

- **폴리레포(서버/클라이언트 분리)**: 계약 변경이 두 커밋으로 쪼개져 "어느 쪽이 맞는가"를 잃는다. 팀이 커지면 재검토.
- **단일 크레이트 서버(HSE §98의 `src/simulation`, `src/history` 모듈 구조)**: 모듈 경계를 컴파일러가 강제하지 않아 `sim`에 IO가 새어 들어가도 막지 못한다. 크레이트 경계가 결정성 보호와 파일 소유권을 동시에 해결한다.
- **부트스트랩에서 전 크레이트·전 asmdef 생성**: 빈 껍데기는 검증할 것이 없고, 실제 의존 방향은 첫 코드가 들어갈 때 결정된다.
- **`client/`를 레포 루트로**(Unity 프로젝트가 루트): `server/`, `contracts/`가 Unity의 에셋 스캔 대상이 되어 Editor가 느려지고 `.meta` 오염이 생긴다.

## 결과

- 좋은 점: 계약·서버·클라이언트가 한 커밋에서 검증된다. 에이전트가 고칠 경로가 표 하나로 결정된다. 역사 엔진은 처음부터 분리된 크레이트라 나중에 서비스로 떼어내도 호출 경계만 바꾸면 된다.
- 감수할 점: `starfall-` 접두사로 명령이 길어진다. 크레이트를 나중에 추가하는 방식이라 "구조가 완성된 모습"은 문서(이 ADR)로만 보인다.
- 다시 검토할 조건: (a) 서버 빌드가 개발 반복을 방해할 만큼 느려질 때 크레이트 분할 재조정, (b) 역사 엔진을 별도 프로세스로 떼야 할 부하 근거가 나올 때(TECH §29), (c) 팀이 분리되어 레포 권한을 나눠야 할 때.

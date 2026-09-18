# p0-01-bootstrap 태스크 (개정 2026-09-18 — 검토 반영판)

- 스펙: `docs/specs/p0-01-bootstrap.md` (상태 agreed)
- ADR: `docs/adr/0001~0004` (전부 accepted)
- 검토 반영 내역: `_workspace/p0-01-bootstrap/01_architect_decisions.md` — **자기 리뷰 항목이 수용/부분 수용/거부 중 무엇인지 여기서 먼저 확인할 것**
- 계약: `contracts/**` 확정. 직접 고치지 말고 architect에게 요청 → architect가 스키마·레지스트리·fixture를 함께 고치고 전원에게 알린다
- 환경: `export PATH="$HOME/.cargo/bin:$HOME/.dotnet/tools:/c/Users/CHOISOOYEON/AppData/Local/Unity/bin:$PATH"`
- 사용자 결정(확정): Unity **6000.6.1f1**, 회사 `Starfall` / 제품 `Starfall Dynasty` / 번들 `com.starfall.dynasty`, 저장소 공개 + 독점 라이선스, LFS 사용, **이 슬라이스에서 커밋·푸시 없음**

## 태스크

| ID | 태스크 | 담당 | 수정 경로 | 선행 | 관련 AC |
|----|-------|------|----------|------|--------|
| T1 | Rust 워크스페이스 골격 + 툴체인 고정 + `/healthz` | server | `rust-toolchain.toml`, `server/Cargo.toml`, `server/Cargo.lock`, `server/crates/gateway/**`, `server/bins/game-server/**` | — | AC-1, AC-2 |
| T2 | 계약 크레이트 `starfall-contracts` (serde 타입 + 계약 테스트 **9종**) | server | `server/crates/contracts/**` | T1 | AC-1, AC-5 |
| T3 | docker compose + `.env.example` + `/readyz` | server | `docker-compose.yml`, `.env.example`, `server/crates/gateway/**`(readyz 핸들러) | T1 | AC-3, AC-4 |
| T4 | 서버 구현 요약 + 측정치 | server | `_workspace/p0-01-bootstrap/03_server_impl.md` | T1–T3 | AC-1~AC-5 증거 |
| T5 | Unity 프로젝트 생성·패키지 정리·asmdef 2종·식별자 설정 | client | `client/**` (단 `Assets/_Project/Scripts/Contracts/**`, `Assets/_Project/Tests/**` 제외) | — (Q1 해소됨) | AC-8 |
| T6 | 계약 → C# DTO 생성기 (+ 선택: netstandard2.1 컴파일 하네스) | client | `tools/codegen/**` | — | AC-6 |
| T7 | DTO 생성물 + 계약 헬퍼(시리얼라이저·UUIDv7) + EditMode 테스트 | client | `client/Assets/_Project/Scripts/Contracts/**`, `client/Assets/_Project/Tests/EditMode/**` | T5, T6 | AC-6, AC-7 |
| T8 | 클라이언트 구현 요약 + 측정치 | client | `_workspace/p0-01-bootstrap/03_client_impl.md` | T5–T7 | AC-6~AC-8 증거 |
| T9 | 스프린트 계약 작성·합의 | qa | `_workspace/p0-01-bootstrap/02_sprint_contract.md` | 스펙 확정(완료) | 전체 |
| T10 | 로컬 스모크 스크립트 | qa | `tests/e2e/**` | T1, T3 | AC-2, AC-3, AC-4 |
| T11 | 커버리지 `--strict` + 경계면 교차 검증 + 저장소 상태 점검 | qa | `_workspace/p0-01-bootstrap/04_qa_report_r{N}.md` | T2, T7 | AC-5, AC-9, AC-10, AC-11 |

권장 순서 — server: T1 → T2 → T3 → T4 (계약 크레이트는 외부 의존이 없어 환경 문제와 무관하게 끝난다). client: T6 → T5 → T7 → T8 (생성물을 Unity에 넣기 전에 컴파일 검증하면 Safe Mode를 예방한다).

## 태스크별 지시

### T1 — Rust 워크스페이스 (server)

- 루트 `rust-toolchain.toml`: `channel = "1.98.1"`, `components = ["rustfmt", "clippy"]`, `profile = "minimal"`. **레포 안 첫 cargo 실행이 툴체인을 새로 내려받는다**(설치된 것은 `stable`뿐). 시간을 재서 T4에 기록.
- **워크스페이스 루트는 `server/`다.** 모든 cargo 명령은 `cd server` 후. 검증 명령은 `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace --locked`.
- `server/Cargo.toml`: `[workspace]` + `[workspace.dependencies]` + `[workspace.lints]`. **`[workspace.lints]`는 각 멤버가 `[lints] workspace = true`로 옵트인해야 적용된다** — 빠뜨리면 린트가 꺼진 채 clippy가 통과한다. 크레이트 추가 시마다 확인하고 T4에 체크 항목으로 남긴다.
- 패키지는 `starfall-` 접두사. 이번에 만드는 것은 `starfall-contracts`, `starfall-gateway`, `starfall-game-server`뿐. `domain`/`sim`/`persistence`/`history`는 만들지 않는다. **`starfall-gateway`에 게임 규칙·월드 상태를 넣지 않는다**(ADR-0001 §2) — 다음 슬라이스에서 가장 쉬운 길이 여기이므로 지금 선을 그어 둔다.
- `[lib] name`을 따로 지정하지 않는다. 호출부가 길면 `use starfall_sim as sim;`.
- `/healthz`는 의존성을 보지 않는 liveness(스펙 §5 형태). 설정은 환경 변수, 로그는 `tracing`.
- clippy로 `unwrap_used`를 강제하면 테스트에서도 걸린다. `clippy.toml`/`#[allow]`로 테스트만 예외 처리하고 **조정 사실을 T4에 명시**.
- axum 0.8.x/tokio 1.5x는 API가 버전마다 바뀐다. 코드 작성 전 Context7로 고정 버전 문서 확인.

### T2 — 계약 크레이트 (server)

- **T2의 첫 작업**: `unevaluatedProperties`가 `allOf` 애너테이션을 제대로 수집해 `actor-field-injected`가 거부되는지 재현한다. 이게 안 되면 계약 작성 스타일 전체가 흔들리므로 즉시 architect에게 알릴 것.
- 테스트는 ADR-0002 §3의 **9종 전부**(왕복·반례 스키마 거부·**반례 serde 거부**·required 변이·레지스트리 자체 검증·`$id` 정합·정수 상한 거부 포함). 반례는 이제 **7건**이다(상한 위반 2건 추가: `probe-seq-above-u32`, `tick-above-safe-integer`).
- 타입 설계 제약(ADR-0002 §3, 지키지 않으면 왕복 테스트가 반드시 깨진다):
  - 비교는 `serde_json::Value`로. 문자열 비교 금지.
  - 널 가능 envelope 필드에 **`skip_serializing_if` 금지**(`client_sent_at: null`이 사라진다).
  - `RealTime`/`GameTime`은 **검증하는 문자열 newtype**. `chrono`/`time` 타입 금지(소수 자리 정규화로 왕복이 깨진다).
  - `#[serde(flatten)]`·내부 태그 열거형 금지 — `deny_unknown_fields`가 무력화되어 `actor-field-injected`가 **서버에서만** 통과한다. 타입마다 envelope 필드를 펼쳐 쓰고 `deny_unknown_fields`를 건다. 디스패치용 peek 구조체만 예외.
  - 정수는 폭이 정확한 타입 + 범위 검증 newtype(`probe_seq` = `u32`, `tick` = 0..2^53−1).
- 검증기: `jsonschema` 0.56.x, `default-features = false`, 모든 스키마를 `$id`로 `Registry`에 등록 후 `offline()`. (이유: 리졸버 feature를 켜지 않아 미등록 `$ref`가 조용한 네트워크 접근이 아니라 실패로 드러나게 한다.)
- 커버리지 스크립트 전제: 타입 태그를 `concat!`·매크로 조합으로 만들지 말고 **리터럴 상수**로 둔다.
- `contracts/` 경로 해석에 실패하면 "찾을 수 없다"로 **명확히 죽인다**. fixture 0건 순회 후 통과는 최악이다 — **개수 0이면 실패**로 못박는다.
- `uuid` 1.26.x 역직렬화는 대문자·중괄호도 받는다. 재직렬화가 소문자 하이픈인지 테스트로 확인.

### T3 — 로컬 인프라 + `/readyz` (server)

- `docker-compose.yml`: `name: starfall`, `version:` 키 없음, PostgreSQL·Redis만. 이미지 `postgres:18.6-trixie`, `redis:8.10.1-trixie`.
- **PG18 함정**: 명명 볼륨을 `/var/lib/postgresql`에 마운트(`/var/lib/postgresql/data` 아님). 착수 시 `docker image inspect postgres:18.6-trixie`로 `PGDATA`·`Config.Volumes`를 실측해 T4에 기록(현재 미확인 사실).
- 호스트 포트 기본값 `127.0.0.1:${STARFALL_PG_HOST_PORT:-15432}`, `127.0.0.1:${STARFALL_REDIS_HOST_PORT:-16379}` (5432는 네이티브 `postgresql-x64-16`이 점유 중).
- Redis 영속화 끄기(`--save "" --appendonly no`), healthcheck 2종.
- `/readyz` 구현 제약(ADR-0003 §3.1): **지연 연결**(DB 없이도 기동), **점검마다 2초 타임아웃·동시 실행**, **`checks` 값은 `"ok" | "unavailable"` 닫힌 집합**(드라이버 에러·DSN·비밀번호를 본문에 넣지 않는다), `/healthz`와 용도 분리.
- 크레이트: `sqlx`(TLS 없음, `runtime-tokio`+`postgres`), `redis` **1.7.0**(`tokio-comp`, `ConnectionManager`). `sqlx 0.9.0`의 파괴적 변경이 크면 **0.8.x로 핀**하고 이유를 T4에 남긴다.
- **Docker 주의**: 이 PC에 다른 프로젝트 컨테이너 5개가 돌고 있다. `docker system prune`·`docker volume prune`·프로젝트 밖 `down -v` **금지**. 항상 `docker compose -p starfall …` 범위로.
- 클린 빌드 시간을 T1 직후(axum만)와 T3 직후(sqlx·redis 추가) **두 번** 측정해 T4에 기록한다.

### T5 — Unity 프로젝트 (client)

- 버전은 **6000.6.1f1** 확정. 대기 없이 착수한다.
  ```bash
  unity --no-banner --non-interactive projects new client \
    --path C:/WorkSpace/SpaceHistoric \
    --editor-version 6000.6.1f1 \
    --template com.unity.template.urp-blank
  ```
  `projects new`에는 git·cloud 옵션이 없다(확인됨). 생성 후 `client/.git`이 없고 새 커밋이 생기지 않았는지 확인한다. **Hub 레지스트리에는 등록된다**(client 실측: `DefaultCompany`/`client`로 등록됨) — `unity projects add`는 필요 없다.
- **패키지 정리(architect 결정, T5 실행 완료)**: 템플릿 기본에서 **`com.unity.collab-proxy`와 `com.unity.visualscripting`을 제거**했다. 전자는 git 결정과 중복이고, 후자는 MVP 범위에서 쓰지 않는 큰 서브시스템이다. **제거로 인한 해석 오류는 없었다**(실측). `timeline`·`ai.navigation`·`inputsystem`은 남긴다. 실제 해석된 버전은 템플릿에 적힌 고정 버전이 아니라 **Hub가 레지스트리 최신으로 올린 값**이다(`inputsystem` 1.20.0, `ai.navigation` 2.0.14). 패키지 버전의 정본은 `client/Packages/manifest.json`이다.
- 추가할 패키지는 `com.unity.nuget.newtonsoft-json` **3.2.2** 하나. `test-framework` 1.8.0은 이미 포함되어 있다.
- `Assets/TutorialInfo/`(템플릿 튜토리얼 에셋) 삭제.
- 식별자 설정: 회사 `Starfall`, 제품 `Starfall Dynasty`, 번들 `com.starfall.dynasty`. `projects new`에는 해당 옵션이 없으므로 생성 후 설정하고, 생성 직후 값이 무엇이었는지도 T8에 기록.
- asmdef 2종(ADR-0001 §3). **`overrideReferences: true`가 없으면 `precompiledReferences`는 읽히지 않는다**:
  - `Starfall.Contracts`: `noEngineReferences: true`, `overrideReferences: true`, `precompiledReferences: ["Newtonsoft.Json.dll"]`
  - `Starfall.Tests.EditMode`: Editor 플랫폼, `overrideReferences: true`, `precompiledReferences: ["nunit.framework.dll", "Newtonsoft.Json.dll"]`, 참조에 `Starfall.Contracts`·TestRunner
- `.meta`는 지우거나 손으로 만들지 않는다. 콜드 임포트 시간을 측정해 T8에 기록.

### T6 — 계약 코드 생성기 (client)

- **NJsonSchema 11.6.1과 quicktype 26.0.0은 실패가 확인됐다. 다시 시도하지 말 것**(ADR-0002 §"검증한 사실" 2·3).
- .NET 10 단일 파일 앱, NuGet 의존 없이 `System.Text.Json`만.
- 규칙은 ADR-0002 §4 전부: 키워드 allowlist 3분류(그 외는 JSON Pointer 경로와 함께 실패), **정수는 선언된 `[minimum, maximum]`을 담는 가장 좁은 타입**(`int`→`uint`→`long`, `ulong` 금지, 범위 없으면 실패), `format: uuid` → `Guid`, `RealTime`/`GameTime` → **`string` 고정**, `Required` 3행 표(널 가능 + required → **`Required.AllowNull`**), 생성 파일 `#nullable disable`, 결정적 출력, `--check`는 **내용 + 파일 집합** 비교, `.meta`는 건드리지 않는다.
  - 정수 매핑이 범위 기반으로 확정되었으므로 `probe_seq`는 **`uint`**다. 이 결정으로 C# 책임 반례가 3건 → **5건**이 되었다(스펙 §5 표).
- 선택(권장): `tools/codegen/verify/`에 `netstandard2.1` 타깃 csproj를 두어 생성물 + Newtonsoft 13.0.2를 Unity 임포트 **전에** 컴파일한다. 계약 DTO가 컴파일 에러를 내면 Editor가 Safe Mode로 뜨고 CLI가 전부 막히므로 값싼 보험이다. AC는 아니다.

### T7 — DTO·헬퍼·EditMode 테스트 (client)

- `Generated/`는 손으로 고치지 않는다. 생성물은 추적 대상이다(커밋은 이번 슬라이스에 하지 않는다).
- 계약 전용 시리얼라이저는 **프로필 2종으로 설계**하되 이번 슬라이스는 `Strict`만 구현한다(`DateParseHandling.None` + `MissingMemberHandling.Error`). `Runtime`(Ignore + 경고 로그)은 p0-02 전송 계층에서 구현한다 — ADR-0002 §4에 근거가 있다.
- **디스패치 헬퍼는 p0-02의 실제 수신 경로가 쓸 같은 함수로 만든다.** 그래야 AC-7(c)의 날짜 가드가 진짜 코드 경로 위에 있다. `JObject`를 열 때 `DateParseHandling.None` 리더를 쓴다.
- UUIDv7 헬퍼: **정규 문자열 조립 → `new Guid(string)`**. `new Guid(byte[])`는 앞 3그룹을 리틀엔디언으로 읽어 **버전 니블이 `7`→`b`로 깨지는데 variant는 살아남아** 패턴 검사를 대충 짜면 통과한다. 난수는 `RandomNumberGenerator`. 위치는 `Starfall.Contracts`(손으로 쓴 파일, `Generated/` 밖) — `noEngineReferences`이므로 `UnityEngine.Random`·`Debug`는 쓸 수 없다.
- fixture 로더: `contracts/registry/types.json`을 마커로 위로 올라가며 레포 루트를 찾고, **못 찾거나 유효 fixture가 4건 미만이면 테스트를 실패**시킨다(Skip 아님). 유효 fixture는 `{TYPE}/*.json`만 순회하고 `invalid/`를 재귀로 빨아들이지 않는다.
- 테스트 결과: `unity test client --mode EditMode --report-format nunit,junit --output _workspace/p0-01-bootstrap/unity-tests/EditMode.nunit.xml --junit-output _workspace/p0-01-bootstrap/unity-tests/EditMode.xml` — 형식은 **쉼표 목록**으로 준다(`both`는 유효하지 않다: `Allowed values: nunit, junit`, 종료 코드 2). 형식을 하나만 주면 `--junit-output`이 무시되고 리포트가 `--output` 기본값으로 간다. 실행 후 **두 파일 존재를 직접 확인**해 T8에 적는다. 컴파일 에러·경고는 **`client/Logs/Editor.log`**에서 확인한다(프로젝트 안, 실행마다 갱신).

### T9~T11 — QA

- T9 스프린트 계약: 스펙 §7의 AC를 실행 항목으로. 스펙에 없는 요구를 새로 만들지 않는다. 특히 아래를 **독립 항목**으로 넣는다.
  - `invalid/` **7건 전부가 스키마 검증에서 거부**되는가(커버리지 스크립트는 `invalid/`를 세지 않는다 — 의도된 동작).
  - `invalid/` 7건의 **Rust serde 거부 결과가 스펙 §5 표와 일치**하는가(운영 경로 구멍 점검).
  - C# 책임 5건이 거부되고, 나머지 2건은 "감지 불가"로 기록되는가.
- T10 스모크 스크립트: `docker compose up -d` → healthy 대기 → `/healthz`·`/readyz` → 종료 코드. **`curl` 대신 `curl.exe`**(PowerShell에서 `curl`은 `Invoke-WebRequest` 별칭). PowerShell 네이티브로 쓰면 503을 기대하는 점검에 `-SkipHttpErrorCheck`가 필요하다. **전역 `docker system prune`·`volume prune`·프로젝트 밖 `down -v` 금지** — 이 PC에는 다른 프로젝트 컨테이너 5개가 돌고 있다. "깨끗한 상태"는 **starfall 프로젝트 볼륨이 없는 상태**를 뜻한다.
- T11 검증에 반드시 포함:
  - `python .claude/skills/integration-qa/scripts/check_contract_coverage.py --strict` 종료 코드 0. **T2·T7 완료 후에 실행**한다(그 전 결과는 판정에 쓰지 않는다).
  - 경계면 교차 비교: 필드 이름·필수·널 가능은 동일, 정수는 **스키마 범위를 손실 없이 담는지**로 본다(타입 이름 동일이 기준이 아니다 — `probe_seq`는 스키마 `0..4294967295`, Rust `u32`, C# `uint`).
  - 저장소 상태(AC-11): 새 커밋 0건, 무시/추적 경계, **`server/.sqlx/`가 무시되지 않는지** 확인.
  - 실행할 수 없었던 항목은 PASS가 아니라 "미검증(환경)".

## 계약 변경이 필요할 때

server·client 누구도 `contracts/**`를 직접 고치지 않는다. 근거(무엇이 안 되는가, 어떤 shape이 필요한가)와 함께 architect에게 요청한다. architect가 스키마·레지스트리·fixture를 함께 고치고 server·client·qa 전원에게 변경 내용과 호환성을 알린다. 한쪽에만 알리는 것이 경계면 버그의 주원인이다.

이번 검토에서 실제로 반영된 계약 변경: 상한 위반 반례 fixture 2건 추가, 정수 `format` 제거(매핑 근거를 `minimum`/`maximum`으로 일원화). 유효 fixture는 그대로이며 재검증을 마쳤다.

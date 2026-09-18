# 0003. 로컬 개발 환경과 툴체인 고정

- 상태: accepted (사용자 결정 + server·client 검토 완료, 2026-09-18)
- 날짜: 2026-09-17 (검토·결정 반영 2026-09-18)
- 슬라이스: p0-01-bootstrap
- 검토: `01_server_adr_review.md`(A-14~A-21, R-3·R-4), `01_client_adr_review.md`(E-3·E-4). 반영 내역은 `01_architect_decisions.md`

## 맥락

이 프로젝트는 Rust·Unity·PostgreSQL·Redis·Docker를 동시에 쓰고, 여러 에이전트와 사람이 같은 Windows PC에서 명령을 실행한다. 버전이 고정되지 않으면 "내 환경에서는 되는데"가 곧바로 QA 증거의 신뢰성 문제로 번진다. 결정은 이 PC에서 확인한 사실(아래)에 근거한다.

## 확인한 환경 (2026-09-17)

| 도구 | 버전 | 비고 |
|------|------|------|
| Rust | rustc/cargo 1.98.1, rustup 1.29.1 | `stable-x86_64-pc-windows-msvc` |
| .NET SDK | 10.0.401 | 단일 파일 앱(`dotnet run x.cs`) 동작 확인 |
| Node / npm | 24.19.0 / 11.17.0 | 계약 생성기에는 쓰지 않음 |
| Python / uv | 3.12.0 / 0.11.2 | QA 커버리지 스크립트는 표준 라이브러리만 사용 |
| Docker / Compose | 27.3.1 / v2.29.7-desktop.1 | 실행 중 |
| git / git-lfs | 2.47.1.windows.1 / 3.6.0 | |
| Unity CLI | 1.0.0-beta.8 | 로그인됨 |
| Unity Editor | 6000.6.1f1 (설치됨, 모듈: Web) | stream `SUPPORTED`, `lts: false` |
| 설치된 템플릿(6000.6.1f1) | `com.unity.template.urp-blank` 17.2.1, `universal-2d` 7.0.0, `get-started` 4.0.1 | |

## 결정

### 1. Rust 툴체인

레포 루트 `rust-toolchain.toml`로 고정한다(ADR-0001 §4: `server/`와 `tools/bots/`가 같은 파일을 상속).

```toml
[toolchain]
channel = "1.98.1"          # 패치까지 고정. 올릴 때는 이 ADR을 갱신한다
components = ["rustfmt", "clippy"]
profile = "minimal"
```

`stable`로 두지 않는다. stable은 6주마다 바뀌고, 새 clippy 린트가 `-D warnings` 게이트를 갑자기 실패시켜 기능과 무관한 수정을 강요한다. 툴체인 상승은 의도된 작업으로 만든다.

**알아 둘 것**: 이 PC에 설치된 툴체인은 `stable-x86_64-pc-windows-msvc` 하나뿐이다. 버전이 같아도 rustup에게 `1.98.1`은 **다른 툴체인 이름**이라, 레포 안에서 첫 `cargo` 실행 시 `1.98.1-x86_64-pc-windows-msvc`를 새로 내려받는다(1회, 수백 MB). 첫 명령이 오래 걸리는 것은 고장이 아니다. 오프라인 환경이면 여기서 막힌다.

**Rust 워크스페이스 루트는 `server/`다**(ADR-0001 §4에 따라 `tools/bots/`가 별도 워크스페이스이므로 레포 루트에는 `Cargo.toml`이 없다). 모든 cargo 명령은 `cd server` 후 실행한다. 검증 명령은 `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace --locked`로 쓴다. `--all`은 가상 매니페스트에서 rustfmt의 대상 범위 모호함을 없애고, `--locked`가 없으면 `Cargo.lock` 추적 결정이 아무것에 의해서도 검증되지 않는다.

### 2. Unity Editor 버전 — **6000.6.1f1 고정** (사용자 결정, 2026-09-18)

확인한 사실만 적는다.

- 설치된 `6000.6.1f1`은 **LTS가 아니다**(Unity CLI: `stream: SUPPORTED`, `lts: false`).
- 현재 LTS 스트림은 `6000.0`(최신 6000.0.84f1)과 `6000.3`(최신 6000.3.24f1). Unity 공식 지원 페이지 기준 **Unity 6.0 LTS는 2026년 10월까지**, **Unity 6.3 LTS는 2027년 12월까지** 지원된다.
- Unity 공식 정책: LTS는 2년 지원(연 1회 릴리스), Update 릴리스는 **다음 릴리스가 나올 때까지만** 지원.
- 6.3 다음 LTS가 어느 버전인지·언제인지는 **공식 문서에서 확인하지 못했다**(미확인). `6000.7.0a6` 알파는 존재한다.
- Unity 6000.6 매뉴얼 기준 C# 언어 버전은 9.0이다.

**결정: `6000.6.1f1`로 고정한다**(사용자 결정). 이미 설치되어 있고, 사전 제작 단계라 깨질 콘텐츠가 없으며, 다음 LTS로 올릴 때 건너뛸 버전 수가 적다. `client/ProjectSettings/ProjectVersion.txt`의 값이 정본이고, 에디터 버전 변경은 이 ADR 갱신 + 슬라이스 작업으로만 한다.

**이 결정이 안고 가는 위험과 재검토 조건**: Update 릴리스는 다음 릴리스가 나오는 순간 지원 범위를 벗어난다. 따라서 **콘텐츠(씬·프리팹·아트·시리얼라이즈된 에셋)가 쌓이기 전에 LTS로 이전한다**를 미결 과제로 남긴다. 트리거는 (a) 다음 LTS 스트림 공개, (b) `client/Assets/_Project/{Art,VFX,Rendering}`에 실제 에셋이 들어가기 시작하는 슬라이스 — 둘 중 먼저 오는 쪽이다. 지금 이전 비용은 사실상 `ProjectVersion.txt` 교체 + 재임포트지만, 씬·프리팹이 쌓인 뒤에는 에셋 업그레이드 비용이 따라붙는다.

참고(선택하지 않은 안의 비용): `6000.3.24f1` LTS로 갔다면 에디터 본체 + **Web Build Support 모듈 재설치**가 필요했고(현재 모듈은 6000.6.1f1에만 설치됨), URP 템플릿 버전도 달라진다. 다운로드가 이 슬라이스의 최장 단일 작업이 됐을 것이다.

**프로젝트 생성 실측 (2026-09-18, client)**. `unity projects new client --editor-version 6000.6.1f1 --template com.unity.template.urp-blank`로 생성했고 **Hub 레지스트리에 등록된다**(`DefaultCompany`/`client`). 해석된 패키지 버전은 템플릿에 적힌 고정 버전이 아니라 **Hub가 레지스트리 최신으로 올린 값**이므로, 패키지 버전의 정본은 템플릿이 아니라 `client/Packages/manifest.json`이다.

| 패키지 | 템플릿 기재 | 실제 해석 |
|--------|------------|----------|
| `com.unity.inputsystem` | 1.19.0 | **1.20.0** |
| `com.unity.ai.navigation` | 2.0.12 | **2.0.14** |
| `com.unity.render-pipelines.universal` | 17.6.0 | 17.6.0 |
| `com.unity.test-framework` | 1.8.0 | 1.8.0 |
| `com.unity.nuget.newtonsoft-json` | (없음, 추가함) | 3.2.2 |
| `com.unity.collab-proxy`, `com.unity.visualscripting` | 2.12.4 / 1.9.11 | **제거** (해석 오류 없음) |

콜드 임포트 63초, 웜 11초(EditMode 테스트 1회 기준). 다음 슬라이스의 비교 기준값이다.

### 2.1 실측 기준값 (2026-09-18, 이 PC)

| 항목 | 값 |
|------|-----|
| 서버 클린 빌드 — axum만 (T1) | 18초 |
| 서버 클린 빌드 — +`starfall-contracts` (T2) | 18초 |
| 서버 클린 빌드 — +sqlx·redis (T3, 201 crates) | 26초 |
| → `/readyz`의 클린 빌드 비용 | **+8초** |
| 레포 안 첫 cargo 실행의 툴체인 1.98.1 설치 | 60초 (1회) |
| 첫 `docker compose up -d` 이미지 pull | 12초 |
| Unity 콜드 임포트 / 웜 (EditMode 1회) | 63초 / 11초 |

`/readyz`를 부트스트랩에 넣은 비용이 클린 빌드 +8초로 측정됐다. 이 엔드포인트가 증명하는 것(호스트 포트·자격 증명·IPv4 경로로 서버 프로세스가 실제로 붙는다)에 비하면 싸다 — 재논의 대상이 아니다.

### 3. docker compose — 인프라만, 버전 고정, 포트 충돌 회피

- `docker-compose.yml`에는 **PostgreSQL과 Redis만** 둔다. 게임 서버는 `cargo run`으로 네이티브 실행한다(Windows에서 컨테이너 빌드 반복은 느리고, 디버깅·프로파일링이 어렵다). 서버 컨테이너화는 CI/배포 슬라이스에서.
- 이미지는 패치까지 고정: `postgres:18.6-trixie`, `redis:8.10.1-trixie`. 태그 `18`·`8`은 조용히 움직인다. 다이제스트(`@sha256:`) 고정은 CI 슬라이스에서 재검토.
- **PostgreSQL 18 주의**(실측 확인, 2026-09-18 server): `docker image inspect postgres:18.6-trixie` 결과 `PGDATA=/var/lib/postgresql/18/docker`, `Volumes={"/var/lib/postgresql":{}}`로 이 ADR의 전제가 정확했다. 실제 마운트도 `starfall_postgres-data -> /var/lib/postgresql`로 확인됐다. 이미지의 `PGDATA`가 `/var/lib/postgresql/18/docker`로 바뀌었고 `VOLUME`은 `/var/lib/postgresql`이다. 볼륨을 예전처럼 `/var/lib/postgresql/data`에 마운트하면 **에러 없이** 데이터가 볼륨 밖에 쓰이고 매 기동마다 `initdb`가 돈다. 명명 볼륨을 `/var/lib/postgresql`에 마운트한다.
- **호스트 포트**: 이 PC에는 네이티브 `postgresql-x64-16` 서비스가 `0.0.0.0:5432`에서 이미 듣고 있다. 컨테이너 포트를 5432로 노출하면 바인딩이 실패하거나(또는 도구가 엉뚱한 DB에 붙어) 디버깅 불가능한 혼란이 생긴다. 기본 호스트 포트를 **PostgreSQL 15432, Redis 16379**로 하고 `.env`로 바꿀 수 있게 한다. 바인딩은 `127.0.0.1:`로 한정한다(LAN 노출 금지).
- 두 서비스 모두 healthcheck를 둔다(PostgreSQL: `pg_isready`, Redis: `redis-cli ping`). QA는 `docker compose ps`의 `healthy`를 증거로 쓴다. 다만 `healthy`는 **컨테이너 안에서** 점검이 통과했다는 뜻일 뿐, 호스트 포트·자격 증명·IPv4 경로가 맞는지는 증명하지 않는다. 그것은 `/readyz`가 증명한다(§3.1).
- **이 PC의 Docker에는 다른 프로젝트 컨테이너가 5개 돌고 있다**(`livingfeed-*`). `docker system prune`, `docker volume prune`, 프로젝트 밖 `down -v`를 **절대 쓰지 않는다**. 정리는 항상 `docker compose -p starfall …` 범위로 한다. 문서에서 말하는 "깨끗한 상태"는 전역 초기화가 아니라 **starfall 프로젝트 볼륨이 없는 상태**를 뜻한다.

### 3.1 `/readyz`의 구현 제약

`/readyz`는 이 슬라이스에 남긴다. 위의 위험한 결정 세 가지(호스트 포트 15432, PG18 볼륨 경로, `.env` 배선)를 **서버 프로세스 관점에서** 검증하는 유일한 경로이기 때문이다. 대신 다음을 지킨다.

1. **지연 연결.** DB가 꺼져 있어도 프로세스는 기동한다. 기동 시 연결을 강제하면 "인프라 없이도 서버는 살아 있다"는 수용 기준을 만족할 수 없다.
2. **점검마다 바운드 타임아웃(2초), 두 점검은 동시에.** `/readyz` 자체가 매달리면 readiness 신호가 아니라 장애다. 클라이언트 레벨 타임아웃도 함께 건다.
3. **응답 본문에 드라이버 에러 문자열·DSN·비밀번호를 넣지 않는다.** `checks.*`의 값은 닫힌 집합 **`"ok" | "unavailable"`**이고, 원인은 `tracing` 로그에만 남긴다. 개발 편의로 에러를 본문에 실으면 그 습관이 배포까지 간다.
4. **`/healthz`와 용도를 섞지 않는다.** `/healthz`는 의존성을 보지 않는 liveness, `/readyz`는 트래픽 수용 가능 여부다. 배포 슬라이스에서 `/readyz`를 재시작 트리거로 쓰면 DB 장애가 서버 재시작 루프가 된다.

의존성 크레이트는 TLS를 켜지 않는다(로컬은 평문).

| 크레이트 | 버전 | features | 비고 |
|---------|------|---------|------|
| `sqlx` | **0.8.6** | `default-features = false`, `runtime-tokio`, `postgres` | 아래 핀 근거 참조 |
| `redis` | **1.7.0** | `default-features = false`, `tokio-comp`, **`connection-manager`** | 연결은 `redis::aio::ConnectionManager` |

**`connection-manager` feature는 필수다.** `tokio-comp`만으로는 `redis::aio::ConnectionManager`가 존재하지 않아 unresolved import로 컴파일이 깨진다(server 구현 중 실제로 막혔다). 결정(재연결이 되는 clone 가능한 멀티플렉스 연결) 자체는 그대로다.

`fred`·`deadpool-redis`·`bb8-redis`는 쓰지 않는다 — Redis는 "지워도 되는" 계층(원칙 3)인데 큰 의존성과 설정 표면을 살 이유가 없고, 멀티플렉스 연결이 이미 동시 명령을 파이프라인한다. 원격 DB·Redis가 생기면 TLS feature 추가 + 이 ADR 갱신.

**`sqlx`는 0.8.6으로 핀한다**(server 재량 결정). 0.9.0은 갓 나온 메이저 변경이고, 부트스트랩이 라이브러리 마이그레이션을 떠안을 이유가 없다. 이번 슬라이스가 쓰는 표면은 `PgPoolOptions`와 `SELECT 1`뿐이라 0.9의 새 기능이 필요 없다. **재평가 시점은 `query!`·`#[sqlx::test]`·마이그레이션을 쓰는 영속화 슬라이스**다 — 그때 0.9의 파괴적 변경을 한 번에 검토한다(그 슬라이스가 어차피 sqlx API를 넓게 쓴다).
- Redis는 **영속화를 끈다**(`--save "" --appendonly no`). Redis는 진실의 원천이 아니라는 원칙(CLAUDE.md 3)을 환경으로 강제한다. Redis를 날려도 PostgreSQL로 전부 복구되어야 한다.
- Compose 파일에 `version:` 키를 쓰지 않는다(Compose v2에서 폐기). 최상위 `name: starfall`로 프로젝트 이름을 고정한다.

### 4. `.env.example`

실제 `.env`는 커밋하지 않는다(.gitignore). `.env.example`은 dev 전용 값으로 채운다.

```
POSTGRES_USER=starfall
POSTGRES_PASSWORD=starfall_dev_only
POSTGRES_DB=starfall
STARFALL_PG_HOST_PORT=15432
STARFALL_REDIS_HOST_PORT=16379
DATABASE_URL=postgres://starfall:starfall_dev_only@127.0.0.1:15432/starfall
REDIS_URL=redis://127.0.0.1:16379/0
STARFALL_HTTP_ADDR=127.0.0.1:8080
STARFALL_LOG_FORMAT=pretty
```

호스트 이름은 `localhost`가 아니라 `127.0.0.1`로 쓴다. Windows에서 `localhost`가 `::1`로 먼저 풀리면 IPv4에만 바인딩된 Docker 포트에 붙지 못하거나 지연이 생긴다.

### 5. Windows 주의사항

- **PATH**: 설치 직후 기존 셸·세션에는 새 도구가 없다. 에이전트는 명령 앞에 다음을 붙인다.
  `export PATH="$HOME/.cargo/bin:$HOME/.dotnet/tools:/c/Users/CHOISOOYEON/AppData/Local/Unity/bin:$PATH"`
- **줄바꿈**: 시스템 git 설정이 `core.autocrlf=true`다. 생성물 바이트 비교(`codegen --check`)와 JSON fixture가 CRLF로 오염되지 않도록 `.gitattributes`에 `* text=auto eol=lf` + `*.cs`/`*.json` 명시 규칙을 두었다(작성 완료, ADR-0004 참조).
- **HTTP 확인 명령**: PowerShell에서 `curl`은 `Invoke-WebRequest`의 **별칭**이라 `-s -o - -w` 같은 인자가 통하지 않는다. 스크립트에는 `curl.exe`로 적는다(System32와 Git 양쪽에 존재). PowerShell 네이티브로 쓸 경우 503을 기대하는 점검에는 `Invoke-WebRequest -SkipHttpErrorCheck`가 필요하다(없으면 예외가 난다).
- **Unity 경로**: `client/Library/`는 깊고 파일이 많다. git에서 제외되지만, Windows Defender 실시간 검사 예외에 `client/Library`, `server/target`을 넣으면 빌드가 눈에 띄게 빨라진다(선택).
- **한글 경로**: `기획안/` 폴더 때문에 `git status`가 이스케이프 출력을 낸다. 필요하면 `git config core.quotepath false`.
- **uv**: 사용자 홈(`C:\Users\CHOISOOYEON\pyproject.toml`)에 깨진 `pyproject.toml`이 있어 홈 아래에서 `uv run`이 실패한다. 레포는 `C:\WorkSpace\`라 영향이 없지만, 홈 아래 임시 디렉토리에서 실행할 때는 `uv run --no-project`를 쓴다.

### 6. CI는 이 슬라이스에서 하지 않는다

GitHub Actions는 MVP 스택에 있지만(TECH §3), 원격 저장소·러너에서의 Unity 라이선스 처리가 별도 작업이다. 부트스트랩은 **로컬에서 재현 가능한 명령**을 확정하는 데까지만 한다. CI는 다음 슬라이스에서 이 명령들을 그대로 옮긴다.

## 검토한 대안과 버린 이유

- **`channel = "stable"`**: 툴체인이 조용히 올라가 clippy 게이트가 무관한 이유로 깨진다.
- **게임 서버도 compose에 넣기**: Windows에서 Rust 재빌드 반복이 느려지고, 에이전트가 로그·디버거에 접근하기 어려워진다. 배포 시점에 다시 본다.
- **호스트 포트 5432/6379 유지**: 이 PC에서 이미 충돌한다(확인함).
- **네이티브 PostgreSQL 16 사용**: 버전이 고정되지 않고(16 vs 18), PostgreSQL 18의 `uuidv7()`·`uuid_extract_timestamp()`를 못 쓴다. 팀원마다 다른 인스턴스를 보게 된다.
- **Redis 대신 Valkey**: MVP 스택이 Redis이고 로컬 개발에서 라이선스 이슈가 없다(Redis 8은 AGPLv3 포함 3중 라이선스). 호스팅·배포 시점에 재검토할 항목으로만 남긴다 — 법률 판단은 아니다.

## 결과

- 좋은 점: 모든 실행 증거가 같은 버전에서 나온다. 로컬 인프라는 `docker compose up -d` 한 줄로 재현되고, 포트 충돌·PG18 볼륨 함정을 미리 피한다. `/readyz`가 "인프라가 떠 있다"와 "서버가 그 인프라에 실제로 붙는다"를 구분해 증명한다.
- 감수할 점: Unity 6000.6.1f1은 LTS가 아니므로 다음 Update 릴리스가 나오면 지원 범위를 벗어난다. 사용자 결정에 따라 이 위험을 안고 가되, 콘텐츠가 쌓이기 전 LTS 이전을 미결 과제로 둔다. `/readyz` 때문에 부트스트랩에 sqlx·redis 의존성이 들어와 빌드 시간이 늘어난다 — **측정 결과 클린 빌드 +8초**(§2.1)로, 얻는 검증에 비해 싸다.
- 다시 검토할 조건: (a) 다음 Unity LTS 공개 또는 아트 에셋 유입 시작 시 LTS 이전 계획, (b) CI 도입 시 이미지 다이제스트 고정과 러너 환경 정리, (c) PostgreSQL/Redis 메이저 업그레이드는 마이그레이션 계획과 함께, (d) 원격 DB·Redis가 생기면 TLS feature와 자격 증명 관리.

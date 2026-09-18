# p0-01-bootstrap: 개발 기반 확립

- 상태: agreed (server·client 검토 반영 + 사용자 결정 확정, 2026-09-18)
- 로드맵 Phase: p0 (사전 제작)
- 근거 기획안 절: TECH §3·§42 (MVP 스택), TECH §43 (개발 원칙), HSE §100–103 (Event Contract·Envelope·이름 규칙), GDD §39 (Phase 0)
- 관련 ADR: 0001(레포·모듈 구조), 0002(계약 형식·코드 생성), 0003(로컬 개발 환경), 0004(원격·공개 범위·라이선스·LFS)
- 검토 기록: `_workspace/p0-01-bootstrap/01_server_adr_review.md`, `01_client_adr_review.md`, 반영 내역 `01_architect_decisions.md`

## 1. 목표

이 슬라이스가 끝나면 **게임 기능은 하나도 없지만**, 팀이 기능을 만들 수 있는 기반이 실행으로 증명된다: 레포 구조와 소유권이 정해졌고, 서버가 빌드·기동·헬스 응답을 하며, 서버 프로세스가 로컬 인프라에 실제로 붙고, `contracts/`의 한 명령·한 메시지가 Rust와 C# 양쪽에서 **같은 모양으로** 역직렬화된다.

플레이어가 할 수 있는 것은 없다. 이것이 의도다(원칙 8).

## 2. 작업 흐름 (플레이 흐름 대체)

게임플레이가 없으므로 "개발자가 빈 체크아웃에서 할 수 있어야 하는 일"을 흐름으로 정의한다. **Rust 워크스페이스 루트는 `server/`다** — 레포 루트에는 `Cargo.toml`이 없다(ADR-0001 §4).

1. 개발자가 레포를 열고 `.env.example`을 `.env`로 복사한다.
2. `docker compose up -d` → PostgreSQL·Redis가 healthy가 된다. *(첫 실행은 이미지 pull로 수 분 걸린다 — 타임아웃이 아니다.)*
3. `cd server && cargo run -p starfall-game-server` → 서버가 기동하고 `/healthz`가 200을 준다. 인프라가 떠 있으면 `/readyz`도 200을 준다. *(레포 안 첫 cargo 실행은 툴체인 1.98.1을 내려받는다 — 1회, 수 분.)*
4. `cd server && cargo test --workspace --locked` → 계약 fixture가 Rust 타입과 일치하고, 반례가 거부됨이 증명된다.
5. 클라이언트 개발자가 `dotnet run tools/codegen/ContractsCodegen.cs ...`로 DTO를 생성하고, Unity에서 EditMode 테스트를 돌려 **같은 fixture**가 C# DTO로도 역직렬화됨을 본다.
6. QA가 계약 커버리지 스크립트를 `--strict`로 돌려 레지스트리·스키마·fixture·코드 참조가 어긋나지 않았음을 확인한다.

## 3. 범위

**포함**
- ADR-0001/0002/0003/0004, 이 스펙, 작업 분해, 검토 반영 기록
- `contracts/` 골격: 레지스트리 + 레지스트리 스키마, envelope 3종(command/event/message), 공통 primitives, `PING_SERVER`(명령)·`PING_REPLY`(서버 메시지) 스키마, 유효 fixture 4 + 반례 fixture 7
- Rust 워크스페이스 골격(`starfall-contracts`, `starfall-gateway`, `bins/game-server`), 루트 `rust-toolchain.toml`
- HTTP 운영 엔드포인트 `/healthz`, `/readyz`
- `docker-compose.yml`(PostgreSQL·Redis), `.env.example`
- Unity 프로젝트 골격(6000.6.1f1, URP 템플릿, asmdef 2개, 패키지 고정), C# DTO 생성기(`tools/codegen`)와 생성물, EditMode 계약 테스트
- 저장소 정책 파일: `.gitignore`, `.gitattributes`(LFS 포함), `LICENSE`(독점)
- QA: 계약 커버리지 `--strict` 통과, 경계면 교차 검증, 로컬 스모크 스크립트

**제외 (다음 슬라이스)**
- WebSocket 연결·명령 전송·`PING_SERVER` 실제 왕복 (p0-02 네트워킹 스파이크)
- tick 루프, tick 주기·게임 달력(에폭·환산 규칙) 결정, `sim_version`
- DB 마이그레이션·테이블·sqlx 쿼리, outbox
- Historical Event 저장 파이프라인, 역사 envelope(`rule_version`·`source_event_ids`·`importance_level`)
- 30명 동시 접속 검증, 렌더링·시뮬레이션 벤치마크, WebGL 가능성 테스트
- CI(GitHub Actions), 커밋·푸시, 인증·세션, IL2CPP 스트리핑 대비(`link.xml`)

## 4. 규칙과 불변식

게임 상태가 없으므로 서버 판정 규칙은 없다. 이 슬라이스가 **지금부터 강제하는 불변식**은 다음이다.

- **I-1 계약 단일 진실**: 와이어 데이터의 모양은 `contracts/`에만 정의된다. Rust 타입과 C# DTO는 계약을 변경할 권한이 없다.
- **I-2 생성물 불가침**: `client/Assets/_Project/Scripts/Contracts/Generated/**`는 생성기 출력이다. 손으로 고친 흔적과 고아 파일을 `--check`가 드러낸다.
- **I-3 fixture 없는 타입은 `active`가 아니다**.
- **I-4 검증기는 살아 있어야 한다**: `invalid/` 7건 중 하나라도 스키마 검증을 통과하면 계약 테스트는 실패한다.
- **I-5 envelope 필드는 항상 존재한다**: 값이 없으면 `null`. 선택 필드는 payload에서만. 이 규칙이 왕복 테스트(AC-5b)의 전제다.
- **I-6 클라이언트는 행위자를 주장하지 않는다**: 명령 envelope에 행위자 필드가 없고, 넣으면 **스키마와 serde 양쪽이** 거부한다.
- **I-7 Redis는 진실이 아니다**: 로컬 Redis는 영속화가 꺼진 상태로 뜬다. 지워도 다른 검증이 깨지지 않는다.
- **I-8 이 슬라이스의 산출물은 커밋하지 않는다**: 원격에는 초기 커밋(LICENSE) 1건만 있고 그 위에 로컬 `main`이 있다. 커밋·푸시는 사용자가 요청할 때만 한다(ADR-0004).
- **I-9 비밀은 저장소에 들어가지 않는다**: 저장소는 공개다. `.env`는 추적하지 않고 `.env.example`에는 dev 전용 값만 둔다.

## 5. 데이터 계약

| 파일 | 내용 |
|------|------|
| `contracts/registry/types.json` / `types.schema.json` | 타입 레지스트리(타입 2종)와 그 스키마 |
| `contracts/common/primitives.schema.json` | `Uuid`, `UuidV7`, `TypeName`, `SchemaVersion`, `SafeInteger`, `NonNegativeSafeInteger`, `Tick`, `Sequence`, `MoneyMinor`, `GameTime`, `RealTime`. 정수는 `minimum`/`maximum`만 선언하고 `format`을 쓰지 않는다(ADR-0002 §1) |
| `contracts/common/command-envelope.schema.json` | `command_id`, `command_type`, `schema_version`, `client_sent_at`(널 가능), `payload` |
| `contracts/common/event-envelope.schema.json` | HSE §101 envelope. 이번 슬라이스에 쓰는 타입은 없고 스키마 유효성과 예시만 검증된다 |
| `contracts/common/message-envelope.schema.json` | `message_id`, `message_type`, `schema_version`, `tick`, `correlation_id`(널 가능), `payload` |
| `contracts/commands/PING_SERVER.schema.json` | payload `{ probe_seq: 0..4294967295 }`. producers: client / consumers: server |
| `contracts/messages/PING_REPLY.schema.json` | payload `{ command_id, probe_seq }`. producers: server / consumers: client |
| `contracts/fixtures/{TYPE}/*.json` | 유효 예시 (타입당 2개, 총 4개) |
| `contracts/fixtures/{TYPE}/invalid/*.json` | 거부되어야 하는 예시 (총 7개) |

운영 엔드포인트(계약 아님 — Unity가 소비하지 않는다. 클라이언트가 쓰게 되면 `contracts/api/`로 승격):

- `GET /healthz` → 200 `{"status":"ok","version":"<서버 버전>"}`. 의존성을 보지 않는 liveness.
- `GET /readyz` → 200 `{"status":"ready","checks":{"postgres":"ok","redis":"ok"}}`, 하나라도 실패하면 503 + 같은 형태. **`checks.*`의 값은 `"ok" | "unavailable"` 닫힌 집합**이고 드라이버 에러·DSN·비밀번호를 본문에 넣지 않는다(ADR-0003 §3.1).

**반례 fixture의 층별 거부 책임** — 같은 반례라도 스키마 검증(개발·테스트), Rust serde(운영 경로), C# 역직렬화(클라이언트)가 잡는 범위가 다르다. 운영 중 명령을 막는 것은 스키마 검증기가 아니라 serde이므로 Rust 열이 핵심이다.

| fixture | 스키마 검증 | Rust serde (운영 경로) | C# Strict 역직렬화 |
|---------|------------|----------------------|-------------------|
| `PING_SERVER/invalid/actor-field-injected.json` | 거부 | 거부 (`deny_unknown_fields`) | 거부 (알 수 없는 멤버) |
| `PING_SERVER/invalid/command-id-not-v7.json` | 거부 | 거부 (`UuidV7` newtype) | **감지 불가** (`Guid`가 파싱함) |
| `PING_SERVER/invalid/probe-seq-negative.json` | 거부 | 거부 (`u32`) | 거부 (`uint`) |
| `PING_SERVER/invalid/probe-seq-above-u32.json` | 거부 | 거부 (`u32`) | 거부 (`uint`) |
| `PING_REPLY/invalid/missing-tick.json` | 거부 | 거부 (필수 필드) | 거부 (`Required.Always`) |
| `PING_REPLY/invalid/payload-unknown-field.json` | 거부 | 거부 (`deny_unknown_fields`) | 거부 (알 수 없는 멤버) |
| `PING_REPLY/invalid/tick-above-safe-integer.json` | 거부 | 거부 (`Tick` 범위 검증) | **감지 불가** (`long`이 담음) |

C#이 못 잡는 2건은 버그가 아니라 기록된 설계 결과다. 스키마 검증과 Rust serde가 서버 경계에서 막는다.

## 6. 역사 연결

이 슬라이스는 Historical Event를 만들지 않는다(도메인 이벤트가 없다). 다만 역사 시스템이 나중에 요구하는 것을 **계약 수준에서 미리 확보**했다: 이벤트 envelope의 `tick`·`sequence`(결정적 순서), `correlation_id`/`causation_id` 분리(HSE §102), `occurred_at`(게임 시간)과 `recorded_at`(실제 시간) 분리, `schema_version`(업캐스팅 근거). 역사 전용 필드(`rule_version`, `source_event_ids`, `importance_level`)는 history-engine-engineer 검토가 필요하므로 이번에 정의하지 않는다.

## 7. 수용 기준

모든 Then은 실행 증거다. 실행하지 못한 항목은 PASS가 아니라 "미검증(환경)"이다. cargo 명령은 **`server/`에서**, 나머지는 레포 루트에서 실행한다. HTTP 확인은 `curl.exe`로 적는다(PowerShell의 `curl`은 `Invoke-WebRequest` 별칭이다).

- **AC-1 (server)** Given 레포, When `cd server` 후 `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace --locked`, Then 세 명령 모두 종료 코드 0. (`--locked`가 `Cargo.lock` 추적 결정을 실제로 검증한다.)
- **AC-2 (server)** Given `.env`가 있고 인프라가 떠 있음, When `cd server && cargo run -p starfall-game-server` 후 `curl.exe -s -o NUL -w "%{http_code}" http://127.0.0.1:8080/healthz`, Then `200`이고 본문이 `{"status":"ok",...}`. 기동 로그에 바인딩 주소가 남는다.
- **AC-3 (server)** Given 인프라가 떠 있음, When `GET /readyz`, Then 200이고 `checks.postgres == "ok"`, `checks.redis == "ok"`이며 두 값 모두 `"ok" | "unavailable"` 집합 안이다. When `docker compose stop postgres` 후 다시 호출, Then 503이고 `checks.postgres == "unavailable"`이며 **서버 프로세스는 살아 있다**. When `docker compose start postgres` 후 **서버를 재시작하지 않고** 다시 호출, Then 200으로 돌아온다(한 번 실패한 풀이 영구 고장으로 남지 않는다).
- **AC-4 (server)** Given starfall 프로젝트 볼륨이 없는 상태(전역 prune 금지 — 이 PC에는 다른 프로젝트 컨테이너가 돌고 있다), When `docker compose up -d` 후 `docker compose ps`, Then postgres·redis 모두 `healthy`. When `docker compose exec -T postgres psql -U <user> -d <db> -c "create table _boot_marker(id int)"` → `docker compose down`(**`-v` 없이**) → `docker compose up -d` → 같은 테이블 조회, Then 테이블이 남아 있고, `docker volume ls`에 이 프로젝트의 **익명 볼륨(64자 해시)이 새로 생기지 않는다**. (로그 문자열 매칭은 이미지 버전에 종속되므로 쓰지 않는다.)
- **AC-5 (server)** Given `contracts/`, When `cd server && cargo test -p starfall-contracts --locked`, Then ADR-0002 §3의 테스트 9종이 모두 통과한다. 특히:
  (a) 모든 스키마가 2020-12 메타스키마로 유효하고 `$ref`가 전부 해석된다(네트워크 접근 없이).
  (b) 유효 fixture 4건이 역직렬화 → 재직렬화 후 원본과 **`serde_json::Value` 비교로 동일**하고 스키마 검증을 통과한다.
  (c) `invalid/` **7건이 스키마 검증에서 전부 거부**된다.
  (d) `invalid/` 7건을 **Rust 타입으로 역직렬화**한 결과가 §5 표의 "Rust serde" 열과 일치한다(현재 7건 전부 거부).
  (e) `producers` **또는** `consumers`에 `server`가 포함된 타입이 전부 이름→Rust 타입 대응표에 있고, 레지스트리에 없는 fixture 디렉토리가 없다.
  (f) 레지스트리·스키마 상수·fixture의 타입 이름과 `schema_version`이 일치하고, `types.json`이 `types.schema.json`으로 검증되며, 모든 `$id`가 경로 규칙과 일치한다.
  (g) `required` 필드를 하나씩 제거한 변이가 전부 역직렬화 실패한다.
- **AC-6 (client)** Given `contracts/`, When `dotnet run tools/codegen/ContractsCodegen.cs -- --contracts contracts --out client/Assets/_Project/Scripts/Contracts/Generated`를 **두 번** 실행, Then 두 번째 실행 후 생성 파일의 해시가 변하지 않는다. When 생성 파일 한 줄을 고치고 `--check`, Then 0이 아닌 종료 코드와 다른 파일 경로가 출력된다. When `Generated/`에 생성기가 만들지 않는 `.cs`를 추가하고 `--check`, Then 0이 아닌 종료 코드와 그 파일 경로가 출력된다(**내용뿐 아니라 파일 집합을 비교한다**).
- **AC-7 (client)** When `unity test client --mode EditMode --report-format nunit,junit --output _workspace/p0-01-bootstrap/unity-tests/EditMode.nunit.xml --junit-output _workspace/p0-01-bootstrap/unity-tests/EditMode.xml`, Then 종료 코드 0, 실패 0건, **두 리포트 파일이 실제로 존재**한다. (형식을 둘 다 받으려면 `nunit,junit` 목록으로 준다. `both`는 이 CLI에서 유효하지 않고 종료 코드 2로 실패한다 — client 실측. 형식을 하나만 주면 `--junit-output`이 무시되고 리포트가 `--output` 기본값으로 간다.) 테스트에는 다음이 포함된다.
  (a) 유효 fixture 4건을 DTO로 역직렬화 → 재직렬화 → 원본과 동일(`Strict` 프로필: `DateParseHandling.None`, `MissingMemberHandling.Error`).
  (b) §5 표에서 C# 책임인 반례 **5건**이 예외로 거부된다.
  (c) `RealTime` 값을 가진 계약 JSON(`PING_SERVER/basic.json`의 `client_sent_at`)을 **디스패치 헬퍼**로 열었을 때 `2026-09-17T14:05:09.123Z` 문자열이 그대로 유지되고, 기본 설정 `JObject.Parse`가 같은 값을 `09/17/2026 14:05:09`로 바꾸는 것도 같은 테스트가 회귀 가드로 보여 준다.
  (d) UUIDv7 헬퍼가 만든 ID가 `UuidV7` 패턴을 만족하고, 같은 밀리초 안에서 N개를 연속 생성해도 중복이 없다.
  (e) fixture 로더가 레포 루트를 `contracts/registry/types.json` 마커로 찾고, 유효 fixture를 4건 미만 발견하면 **테스트가 실패한다**(0건 통과 방지 — I-4와 같은 성격).
- **AC-8 (client)** Given `client/Library/`를 삭제한 상태(콜드 임포트), When 위 `unity test` 명령, Then 종료 코드 0이고 **`client/Logs/Editor.log`**(프로젝트 안에 쓰이며 실행마다 갱신된다 — `%LOCALAPPDATA%\Unity\Editor\Editor.log`가 아니다)에 컴파일 에러 0건이며 `Assets/_Project/**`에서 발생한 컴파일 경고가 0건이다. 그리고 `client/ProjectSettings/ProjectVersion.txt`의 `m_EditorVersion`이 **`6000.6.1f1`과 문자열로 같다**.
- **AC-9 (qa)** When `python .claude/skills/integration-qa/scripts/check_contract_coverage.py --strict`, Then 종료 코드 0(errors 0, warnings 0). 이 검사는 **T2와 T7이 끝나 `server/**/*.rs`와 `client/Assets/**/*.cs`에 타입 이름이 존재한 뒤** 실행한다. 그 전 실행 결과는 판정에 쓰지 않는다.
- **AC-10 (qa)** Given 서버·클라이언트 구현 완료, When QA가 스키마·Rust 타입·C# DTO를 나란히 읽고 비교, Then 필드 이름·필수 여부·널 가능 여부가 3자 간 동일하고, **각 정수 필드의 언어 타입이 스키마의 `minimum`~`maximum`을 손실 없이 표현**하며(타입 이름이 같을 필요는 없다), 범위 밖 값에 대해 최소한 스키마 검증이 거부한다. 층별 거부 범위는 §5 표와 일치한다. 불일치 0건이 리포트에 기록된다.
- **AC-11 (qa)** When `git log --oneline`, `git status --porcelain`, `git check-ignore -v <경로들>`, Then (a) 이 슬라이스가 만든 **새 커밋이 0건**(HEAD가 원격 초기 커밋과 같다), (b) `contracts/`·`docs/`·`_workspace/`·`server/`·`client/Assets/_Project/Scripts/Contracts/Generated/**`의 `.cs`와 `.meta`·`tools/`가 추적 후보(untracked)로 보이고, (c) `client/Library/`·`client/Temp/`·`client/Logs/`·`client/obj/`·`client/UserSettings/`·`server/target/`·`tools/**/obj/`·`.env`가 무시되며, (d) **`server/.sqlx/`는 무시되지 않는다**(다음 슬라이스의 오프라인 빌드 근거).

## 8. 비기능 요구

- 성능 목표 없음(원칙 8). 대신 다음을 **측정해 기록**한다 — p0-02 이후의 비교 기준이다.
  - 서버 클린 빌드 시간 **2회**: T1 완료 시점(axum만)과 T3 완료 시점(sqlx·redis 추가). 두 숫자가 "`/readyz`를 부트스트랩에 넣은 비용"의 사후 증거다.
  - Unity: 콜드 임포트(Library 삭제 후) 포함 1회와 웜 상태 1회의 EditMode 테스트 시간.
  - 레포 안 첫 `cargo` 실행의 툴체인 다운로드 시간, 첫 `docker compose up -d`의 이미지 pull 시간(참고값).
- 재현성: 모든 검증 명령은 인자 없이 같은 결과를 내야 한다(랜덤·현재 시각 의존 금지).
- WebGL: 이번 슬라이스의 코드 경로(계약 DTO·테스트)는 플랫폼 의존이 없다. 전송 계층이 생기는 p0-02에서 WebGL 분기를 다룬다.
- 보안: `.env`는 커밋하지 않는다. 컨테이너 포트는 `127.0.0.1`에만 바인딩한다. 저장소가 공개이므로 어떤 실제 자격 증명도 넣지 않는다(I-9).

## 9. 열린 질문

이번 슬라이스를 차단하는 질문은 없다. 사용자 결정(Unity 버전, 원격·공개 범위·라이선스, LFS, 프로젝트 식별자, 커밋 시점)은 2026-09-18에 확정되어 ADR-0003·0004에 반영했다. 남은 것은 **이후 슬라이스에서 결정할 항목**이다.

| # | 항목 | 결정 시점 |
|---|------|----------|
| Q6 | LTS 이전 시점과 대상 버전 | 다음 Unity LTS 공개 또는 아트 에셋 유입 시작 — 둘 중 먼저 (ADR-0003 §2) |
| Q7 | 라이선스·기여 약정 법률 검토 | 상업 출시 준비 시작 시 (ADR-0004 §3) |
| Q8 | 공개가 불리한 데이터(반치팅 임계값 등) 분리 방식 | 그런 데이터가 처음 생길 때 (ADR-0004 §2) |
| Q9 | Windows Defender 실시간 검사 예외(`client/Library`, `server/target`) | 사용자 판단. 빌드·임포트 속도 개선용이며 기능에는 영향 없음 |
| Q10 | `unityyamlmerge` 설정 절차 | 씬·프리팹을 여러 명이 동시에 고치는 첫 슬라이스 |

## 변경 기록

| 날짜 | 변경 | 이유 |
|------|------|------|
| 2026-09-17 | 최초 작성 (draft) | p0-01-bootstrap 스펙·계약 초안 |
| 2026-09-18 | AC 전면 개정(실행 위치·`curl.exe`·`--locked`/`--all`, AC-3 복구 확인과 닫힌 집합, AC-4 증거를 데이터 잔존+익명 볼륨으로, AC-5 9종 테스트로 확장, AC-6 파일 집합 비교, AC-7 리포트 명령·5건 거부·로더 가드, AC-8 콜드 임포트 기준, AC-9 실행 순서, AC-10 범위 기반 비교, AC-11 커밋 기준 변경), §5에 Rust serde 열과 반례 7건, §4에 I-9, §9 재작성 | server·client 검토(R-1~R-11, A~F 항목)와 사용자 결정 반영 |

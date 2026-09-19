# p0-02-networking-spike 태스크 (개정 2026-09-18 — 검토 반영판)

- 스펙: `docs/specs/p0-02-networking-spike.md` (상태 **agreed**)
- ADR: `docs/adr/0005`(전송·프레이밍), `0006`(tick·게임 시간·큐), `0007`(영속화·기록 범위), `0008`(개발용 인증) — **전부 accepted**
- **검토 반영 내역: `_workspace/p0-02-networking-spike/01_architect_decisions.md` — 자기 리뷰 항목이 수용/부분수용/거부 중 무엇인지 여기서 먼저 확인할 것**
- **AC 번호가 바뀌었다.** 초안 AC-1~20 → 개정 AC-1~21(AC-3 tick 재개, AC-19 기록 내구성 신설). 검토 문서의 옛 번호로 작업하지 말 것
- 계약: `contracts/**` 확정 — **스키마 11, 유효 fixture 12, 반례 16**. 이번 개정에서 `SESSION_READY/tick-zero.json`의 `world_id`가 바뀌었고 `SESSION_CLOSED/invalid/actor-id-null.json`이 추가됐다(반례 15→16). 직접 고치지 말고 architect에게 요청
- 환경: Bash 명령 앞에 `export PATH="$HOME/.cargo/bin:$HOME/.dotnet/tools:/c/Users/CHOISOOYEON/AppData/Local/Unity/bin:$PATH"`
- **커밋·푸시하지 않는다**(ADR-0004 §1)
- Docker: 전역 `system prune`·`volume prune`·프로젝트 밖 `down -v` **금지**. 다만 레포 루트의 **`docker compose down -v`는 `name: starfall` 덕분에 이 프로젝트에만 작용하며, AC-2와 마이그레이션 수정 후에 필요하다**

## 사용자 결정 (확정)

1. 성능 잠정 게이트는 **Phase 0 종료를 막지 않는다.** 첫 측정치를 p1 회귀 기준선으로 고정 기록. 정확성 게이트(AC-15~AC-19)는 그대로 하드
2. "30명 동시 접속" = **봇 30 + Unity 1 = 31**
3. `calendar_scale = 60` (현실 1초 = 게임 1분)
4. CI는 다음 슬라이스 (`sqlx-cli`/`.sqlx/`와 함께)
5. `.env.example`의 `STARFALL_DEV_AUTH_SECRET=dev_only_not_a_secret` 그대로

## 착수 전 반드시 읽을 것

1. `01_architect_decisions.md` — 자기 요청의 처리 결과
2. `docs/specs/...` §4 불변식(I-10~**I-25**)과 §7 수용 기준
3. 자기 영역 ADR: server = 0005·0006(**§2.1·§2.2·§2.3 신설**)·0007·0008 / client = 0005 §4-3·§5·§6, 0002 §4 / qa = 0006 §7, 0007 §1·§4·§8

## 태스크

| ID | 태스크 | 담당 | 수정 경로 | 선행 | 관련 AC |
|----|-------|------|----------|------|--------|
| T1 | 계약 Rust 타입 4종 + 계약 테스트 상수·거부표 갱신 | server | `server/crates/contracts/**` | — | AC-1, AC-10 |
| T2 | `starfall-persistence` + 마이그레이션 0001 + append-only + tick 재개 | server | `server/crates/persistence/**`, `server/migrations/**`, `server/bins/game-server/build.rs` | T1 | AC-1, AC-2, AC-3, AC-4 |
| T3 | `starfall-sim` — 전용 스레드 tick 루프·명령 큐·제어 경로·세션 레지스트리 | server | `server/crates/sim/**`, `server/Cargo.toml`(멤버 추가) | T1 | AC-1, AC-6, AC-7 |
| T4 | WebSocket 게이트웨이 + 개발용 인증 + 프레이밍 | server | `server/crates/gateway/**`, `.env.example` | T1, T3 | AC-5, AC-7, AC-8 |
| T5 | 조립·설정·`/debug/stats`·`/readyz` 재시도·stdin 종료 | server | `server/bins/game-server/**`, `server/crates/gateway/src/{readiness,stats}.rs` | T2, T3, T4 | AC-2, AC-6b, AC-8d, AC-9 |
| T6 | 서버 구현 요약 + 실측치 | server | `_workspace/p0-02-networking-spike/03_server_impl.md` | T1–T5 | AC-1~AC-10 증거 |
| **T7** | **생성기 좁힘 수정** + DTO 생성 | client | `tools/codegen/**`, `client/Assets/_Project/Scripts/Contracts/Generated/**` | 계약 확정 | AC-11 |
| T8 | `Starfall.Net` — 전송 추상화·PC WebSocket·디스패치·재연결 | client | `client/Assets/_Project/Scripts/Net/**`, `client/Assets/_Project/Scripts/Contracts/ContractJson.cs`(Runtime 프로필) | T7 | AC-12d, AC-13, AC-14 |
| T9 | EditMode + PlayMode 테스트 | client | `client/Assets/_Project/Tests/**` | T7, T8 | AC-11b, AC-12, AC-13, AC-14 |
| T10 | 클라이언트 구현 요약 + 실측치 | client | `_workspace/p0-02-networking-spike/03_client_impl.md` | T7–T9 | AC-11~AC-14 증거 |
| T11 | 스프린트 계약 작성·합의 | qa | `_workspace/p0-02-networking-spike/02_sprint_contract.md` | 스펙 확정(완료) | 전체 |
| T12 | 봇 하네스 (별도 Cargo 워크스페이스) | qa | `tools/bots/**` | T1 | AC-15, AC-18, AC-19 |
| T13 | 검증 SQL·스모크 스크립트 | qa | `tests/e2e/**` | T2 | AC-4, AC-16, AC-17 |
| T14 | 부하 실행(A·B·C·D) + 3자 대조 + 리포트 | qa | `_workspace/p0-02-networking-spike/04_qa_report_r{N}.md` | 전부 | AC-15~AC-21 |

**권장 순서** — server: T1 → T2 → T3 → T4 → T5 → T6 (server 검토에서 합의된 순서. 마이그레이션이 먼저 서야 tick 루프 출력이 갈 곳이 생긴다). client: **T7 → T8 → T9 → T10**(T7의 생성기 수정이 다른 모든 클라이언트 작업의 전제다). qa: T11 → T13 → T12 → T14.

**경로 무충돌**: `server/**`(server) / `client/**`·`tools/codegen/**`(client) / `tools/bots/**`·`tests/e2e/**`(qa) / `contracts/**`·`docs/**`(architect). 루트 `rust-toolchain.toml`은 아무도 고치지 않는다.

**server 예상 13~17시간**(자체 산정). client는 T7이 선행이므로 그 결과에 따라 T8~T9가 열린다.

---

## 태스크별 지시

### T1 — 계약 Rust 타입 (server)

- 신규 4타입을 손으로 쓴다. p0-01의 타입 설계 제약 그대로: `serde_json::Value` 비교, 널 가능 envelope 필드에 `skip_serializing_if` 금지, `RealTime`/`GameTime`은 검증하는 문자열 newtype, **`#[serde(flatten)]`·내부 태그 열거형 금지**, 정수는 폭이 정확한 타입 + 범위 newtype.
- 닫힌 값 집합 4종은 **Rust 열거형**(`SCREAMING_SNAKE_CASE`, 모르는 값 거부). C#은 `string` — 의도된 비대칭.
- `SESSION_OPENED`/`SESSION_CLOSED`의 `actor_id`는 **`Option`이 아니다**. 다른 envelope 필드의 널 가능성은 그대로. `required_nullable`과 섞이지 않게 주의.
- 이벤트 envelope의 `correlation_id`는 **비-null**, 메시지 envelope에서는 널 가능. 같은 타입으로 만들지 않는다.
- **테스트 상수를 실제 파일 수로 맞춘다**: `EXPECTED_SCHEMA_COUNT = 7 → 11`, `EXPECTED_VALID_FIXTURES = 4 → 12`, `EXPECTED_INVALID_FIXTURES = 7 → 16`. `SERDE_REJECTION_TABLE`에 **신규 반례 9행** 추가(§5.4 표 전체). **검사 건수를 테스트 출력에 찍는다.**
- 커버리지 스크립트가 코드에서 타입 이름 문자열을 찾는다. **리터럴 상수**로 둔다.
- 착수 시점에 `cargo test`는 이미 빨간불이다(계약이 코드보다 앞서 있다). 정상이다.

### T2 — 영속화·마이그레이션·tick 재개 (server)

- 마이그레이션은 ADR-0007 §2 그대로. **`worlds.last_tick`(nullable) 포함.** 시드 값은 스펙이 기대값으로 쓴다 — 바꾸려면 architect에게.
- feature 조합은 실측 확정: `runtime-tokio, postgres, macros, migrate, uuid, json`. **`chrono`·`time`을 넣지 않는다**(`recorded_at`을 Rust에서 만든다).
- `build.rs`에 `cargo:rerun-if-changed=migrations`. **새 `.sql` 추가는 재컴파일되지 않는다**(수정은 된다).
- `.sql`은 **LF**. 저장 후 실제 바이트 확인.
- **마이그레이션을 고치면 `docker compose down -v && docker compose up -d`.** 체크섬 불일치로 기동이 실패한다 — T2 착수 직후에 바로 겪는다.
- append-only 트리거(ADR-0007 §3). `TRUNCATE`가 막히지 않는 것은 의도.
- **tick 재개**(ADR-0006 §2.3): `start_tick = (worlds.last_tick, MAX(domain_events.tick) 중 큰 값, 둘 다 없으면 -1) + 1`. `worlds.last_tick` 갱신은 **이벤트를 쓰는 트랜잭션 안에서** 한다.
- 쓰기: tick 단위 1 트랜잭션, `ON CONFLICT (event_id) DO NOTHING`. **`UNIQUE (world_id, tick, sequence)` 위반을 삼키지 말 것** — tick 재개가 들어왔으므로 이제 진짜 버그 신호다.
- `recorded_at`은 **Rust 호스트 시계**로. `occurred_at`용 civil-from-days 헬퍼를 재사용한다(`chrono` 불필요).
- 영속화 채널 포화 시 이벤트를 버리지 않는다. 백로그 600 tick 초과 시 `/ws` 503 `reason=recording_backlog`. **이 경로는 AC-19가 실제로 검증한다 — 죽은 코드로 두지 말 것.**

### T3 — `starfall-sim` (server)

- **의존성 0개를 목표로 한다.** `axum`·`sqlx`·`redis`·`rand`·`chrono`가 `Cargo.toml`에 없어야 한다(AC-1이 이걸로 판정한다).
- `starfall-domain`은 만들지 않는다. 세션 모델과 도메인 이벤트 생성자를 `sim` 안에. 분리는 p1.
- **tick 루프는 전용 OS 스레드 + `std::thread::sleep`**(ADR-0006 §2.1). `tokio::time::sleep`은 이 PC에서 50 ms가 61 ms가 된다. 통신은 `try_send`/`try_recv`.
- **"tick 초과"는 `run_tick()` 본문 소요다**(§2.2). 루프 간격으로 재면 빈 루프도 100 %다.
- tick은 1씩 증가, 따라잡기 없음, **재기동 시 `start_tick`부터**.
- **세션 모델에 송신 채널·소켓을 넣지 않는다.** `TickOutcome { tick, events, outbound: Vec<(SessionId, Message)> }`를 반환하고 게이트웨이가 라우팅한다. **이 형태여야 AC-6(a)의 수동 step 테스트가 런타임·소켓 없이 돈다** — 그 테스트가 이 슬라이스의 아키텍처 전제를 증명하는 단 하나의 테스트다.
- **제어 제출(세션 열기·닫기)은 명령 큐를 쓰지 않고 거부 대상이 아니다**(I-16). 명령과의 순서는 **한 곳에서 발급하는** 전역 단조 증가 제출 번호로.
- 명령 큐: 전역 4096 + 세션 in-flight 64. **in-flight는 `COMMAND_RESULT`를 만든 시점에 해제**(전달 확인 아님). `SERVER_BUSY`는 30 연결에서 구조적 미도달이므로 **단위 테스트로 덮는다**.
- 세션 내 중복 제거: 최근 `command_id` 1024개. **주석에 "지속 멱등성이 아님"과 ADR-0006 §6 참조를 남긴다.**
- `occurred_at` 파생은 `tick_hz`로 **먼저** 나눈다. 단위 테스트로 tick 0·1200·24000·86400을 fixture 값과 문자열 비교(server가 이 PC에서 공식 일치를 이미 확인했다).

### T4 — 게이트웨이·인증·프레이밍 (server)

- axum 0.8.9 WS API는 server가 스크래치에서 **컴파일로 확인**했다(업그레이드 전 `HeaderMap` 추출, `max_message_size`/`max_frame_size`/`on_failed_upgrade`, `routing::any`, `CloseFrame{code: u16, reason: Utf8Bytes}`). `ws` feature는 크레이트 8개 추가, 빌드 +2.8초.
- **라이브러리 한도 64 KiB / 앱 한도 16 KiB.** 같게 두면 tungstenite가 먼저 끊어 **앱이 위반을 셀 수 없고 AC-8(a)가 관측 불가능해진다.**
- 모든 프로토콜 위반(크기·바이너리·파싱 실패)을 **같은 예산**(10초 8회)으로 세고 초과 시 **1002**. 1009는 쓰지 않는다.
- 인증은 **업그레이드 전**. 자격 증명 추출을 **한 함수**에. 상수 시간 비교는 `subtle::ConstantTimeEq`(추가 크레이트 0개).
- 비밀 미설정 시 `/ws`만 503 `reason=auth_not_configured`. **기본 비밀값을 코드에 두지 않는다.**
- **큐에 넣지 못한 명령의 `COMMAND_RESULT`는 게이트웨이가 만든다**(ADR-0006 §5). envelope `tick`은 공유 원자값에서 읽은 현재 tick — 이 읽기는 I-13 위반이 아니다.
- 송신은 세션별 bounded 256. 가득 차면 연결을 닫는다(1011, `SLOW_CONSUMER`).
- `SESSION_READY`는 그 연결의 **첫 계약 메시지**다. 게이트웨이는 tick이 돌려주기 전까지 아무것도 보내지 않는 구조로 보장한다.
- `messages_enqueued_total{type}`·`messages_written_total{type}`를 여기서 센다(AC-9c).

### T5 — 조립·운영 표면 (server)

- 설정: `STARFALL_DEV_AUTH_SECRET`, `STARFALL_TICK_HZ`(기본 20), `STARFALL_WORLD_ID`. `config.rs`는 `.env`를 읽지 않는다 — `.env.example`에 추가하고 개발자가 셸에 로드.
- **기동 시 `worlds` 행과 설정 대조, 불일치면 기동 거부**(AC-2).
- `/debug/stats`: 새 의존성 없이 `AtomicU64` + **고정 버킷 히스토그램**(링 버퍼 아님 — 버퍼 길이가 관측 창이 되면 "A 단계 p99"가 거짓이 된다). 노출: `tick`, `start_tick`, `tick_total`, `tick_overrun_total`, tick 본문 소요 분포, `tick_lag_seconds`, `command_queue_depth`, `commands_received_total{type}`, `commands_rejected_total{reason}`, `messages_enqueued_total{type}`, `messages_written_total{type}`, `ws_connections`, `protocol_violations_total`, `persist_backlog`, `sessions_opened_total`, `sessions_closed_total`.
  - **`ws_connections`는 세션 레지스트리의 실제 길이로 계산한다.** 카운터 뺄셈으로 만들면 항등식이 되어 아무것도 증명하지 못한다(I-25).
  - **`tick_skipped_total`을 만들지 않는다.** 설계상 항상 0이라 항진명제다. 대신 `tick_total == tick − start_tick + 1`이 AC다.
- `/readyz`: **즉시 실패에만 1회 재시도, 타임아웃에는 재시도 없음.** 점검당 2초 유지. 본문은 닫힌 집합 그대로.
- **stdin `shutdown` 한 줄 → Ctrl-C와 같은 종료 경로**(스펙 §5.5, architect 승인된 범위 추가). Windows에 SIGTERM이 없어 이것이 없으면 AC-8(d)를 자동 검증할 수 없다.
- 종료 순서: 수락 중단 → 다음 tick에서 전 세션 `SERVER_SHUTDOWN` → **영속화 flush 대기** → 종료.

### T6 — 서버 구현 요약 (server)

필수 기록: 워크스페이스 클린 빌드 시간(p0-01의 26초 대비), `[lints] workspace = true` 옵트인 확인, build.rs 방식, 계약 테스트 검사 건수(12/16/11), tick 루프 실측(본문 소요 p50/p99/max, `tick_lag_seconds`와 바닥값 +0.7 %/분 대비), U-9(tungstenite 자동 Pong) 확인 결과, 스펙에서 틀렸다고 판단한 부분.

### T7 — 생성기 수정과 DTO (client) — **다른 클라이언트 작업의 선행**

- **첫 작업은 생성기의 좁힘 처리 수정이다**(ADR-0005 §4-3, A안 확정). `ContractsCodegen.cs:460`의 속성 병합에서 **override가 구체 `type`을 들고 오면 상속된 `anyOf`를 버린다**. 그래야 `:531`의 `Normalize`가 무조건 nullable로 만들지 않는다.
- **수정 전후 생성물 diff를 떠서 변한 것이 `SessionOpenedEvent.ActorId`·`SessionClosedEvent.ActorId` 두 곳뿐임을 증명**하고 T10에 붙인다(AC-11c). 유효 fixture 12건은 `actor_id`가 전부 비-null이라 안 깨진다.
- `enum`을 `Keywords.Structural` → `Keywords.Constraint`로 옮긴다(동작 무변화, 주석 정정 — 실제로 읽히지 않고 무시된다).
- 생성 후 `--check`로 결정성 재확인. `tools/codegen/verify/`(netstandard2.1 + C#9 + warnings-as-errors)로 **Unity 임포트 전에** 컴파일 검증 — Safe Mode 예방(p0-01 교훈).
- 생성기는 레지스트리 주도라 `contracts/events/domain/`을 문제없이 찾는다(client 실측 확인). 초안의 "디렉토리 순회" 우려는 해당 없음.

### T8 — `Starfall.Net` (client)

- **U-5a를 먼저 실측한다**: 빈 PlayMode 씬에서 `SetRequestHeader("Authorization", …)` + 접속 1회. 서버가 없으면 로컬 더미 리스너로 헤더 수신만 확인. Mono의 제한 헤더 처리로 실패하면 **즉시 architect에 보고**(ADR-0005 §6(a) 티켓 경로를 앞당겨야 한다).
- 새 asmdef `Starfall.Net`. `noEngineReferences`를 켜지 **않는다**(Unity API 필요). `overrideReferences: true` + `precompiledReferences` 명시(p0-01 A-1).
- 수신은 전용 Task → 스레드 안전 큐 → 메인 스레드 배치 디스패치. **전송 계층은 `UnityEngine`을 호출하지 않고 로그도 인터페이스 뒤로** 뺀다.
- 모르는 `message_type`은 경고 후 계속.
- **`Runtime` 프로필**: `DateParseHandling.None` + `MissingMemberHandling.Error` + `Error` 핸들러에서 **`Message.StartsWith("Could not find member")`일 때만 `Handled = true`**. `ErrorContext.Member`·`.Path`는 구분자가 못 된다(client 실측). **필수 필드 누락과 널 불가 필드의 널은 `Runtime`에서도 예외다.** 접두사 의존을 **테스트로 고정**한다.
- 명령 대기 목록: `Sent → Accepted → Completed` / `Sent → Rejected`. **`ACCEPTED`를 완료로 취급하지 않는다**(I-15).
- 백오프: `random(0, min(10s, 500ms * 2^min(n,5)))`. **클램프 5 필수**(없으면 n=62,63에서 0 ms). **카운터는 `SESSION_READY` 수신 시에만 리셋**(TCP 연결 성공은 아니다).
- **업그레이드 실패 원인을 구분하지 않는다**(U-5c: `CollectHttpResponseDetails` 없음). 전부 백오프 재시도, 원인 증거는 서버 쪽.
- 미완료 명령 재전송 금지. 끊길 때 `starfall.net: dropping N in-flight command(s) on disconnect (no resend, I-23)` 한 줄(AC-14 증거).
- **도메인 리로드·PlayMode 종료·앱 종료 훅에서 정상 Close를 보낸다.** 필수 항목이다 — 보내지 않으면 서버가 `TRANSPORT_ERROR`로 기록해 AC-13(c)가 깨진다. U-5b를 여기서 확인한다.
- 핫 경로 무할당(버퍼 재사용, LINQ·문자열 결합·박싱 금지).

### T9 — 테스트 (client)

- EditMode 명령(형식은 **쉼표 목록**):
  ```
  unity test client --mode EditMode --report-format nunit,junit --output _workspace/p0-02-networking-spike/unity-tests/EditMode.nunit.xml --junit-output _workspace/p0-02-networking-spike/unity-tests/EditMode.xml
  ```
  **`unity test --help`는 `--report-format both`를 안내하지만 CLI가 거부한다**(종료 코드 2, client 재확인). help가 틀렸다 — 고치려 들지 말 것.
- **리포트의 `tests` 수를 확인하고 T10에 적는다.** 매칭 0건 필터는 종료 코드 0 + `tests="0"`을 만든다(client 실측).
- `--filter`는 **정규식**(부분 일치). glob(`*X*`)은 `ArgumentException` → CLI 종료 코드 6, 리포트 없음.
- fixture 로더는 유효 fixture가 **12건 미만이면 실패**시킨다. 기대 상수는 12/16.
- **"감지 불가" 항목이 통과한다는 사실을 테스트가 명시적으로 기록**한다. 조용히 빼면 다음 사람이 커버리지를 착각한다.
- **좁힘 회귀 테스트**: `SessionOpened/ClosedEvent.ActorId`의 타입과 `Required`를 리플렉션으로 확인 + 두 `actor-id-null.json`이 거부되는지(AC-11b).
- 백오프 경계 케이스 `n = 0,1,5,62,63,64,100`(AC-12e).
- PlayMode는 실서버 필요. 못 돌리면 **미검증(환경)** — 통과로 적지 않는다. 측정 중 **도메인 리로드가 없어야** AC-13(c)가 성립한다.
- `unity` CLI 종료 코드: 성공 0 / 테스트 실패 8 / 런 에러 6 / 인자 오류 2.

### T10 — 클라이언트 구현 요약 (client)

필수 기록: 생성기 수정 전후 diff(두 속성뿐임), U-5a·U-5b 실측 결과, EditMode 리포트의 `tests` 수, 콜드·웜 시간(p0-01의 63초·11초 대비), 스펙에서 틀렸다고 판단한 부분.

### T11~T14 — QA

**T11 스프린트 계약**: 스펙 §7의 AC 21개를 실행 항목으로. 스펙에 없는 요구를 새로 만들지 않는다. 독립 항목으로 넣을 것:

- 반례 **16건 전부**가 스키마 검증에서 거부되는가(커버리지 스크립트는 `invalid/`를 세지 않는다 — 의도된 동작).
- 반례 16건의 **Rust serde 결과가 §5.4 + p0-01 §5 표와 일치**하는가.
- C# 책임 항목 거부 + "감지 불가" 항목 **기록**.
- **정확성 게이트와 성능 잠정 게이트를 리포트에서 분리 판정**한다. 사용자 결정에 따라 **성능 미달은 실패가 아니다** — 첫 측정치를 p1 회귀 기준선으로 **고정 기록**하는 것이 이 항목의 산출이다.
- **구현 전 기준선**(참고용, 판정에 쓰지 않음): 커버리지 errors 8 / warnings 4, `cargo test` 빨간불, Unity EditMode 26건 중 12건 실패. **이 빨간불은 계약이 코드보다 앞서 있다는 의미이고, p0-01의 테스트 설계가 의도대로 동작한다는 증거다.**

**T12 봇 하네스** (`tools/bots/**`):

- **별도 Cargo 워크스페이스.** 루트 `rust-toolchain.toml`을 고치지 않는다.
- ~~`starfall-contracts`를 path 의존~~ → **봇은 자기 타입을 독립으로 쓰고, 모양은 계약 fixture로 고정한다** (2026-09-19 정정, qa 판단 채택).
  - **내 원래 지시가 내가 쓴 I-25와 어긋났다.** "검증은 출처가 독립일 때만 검증이다"라고 해 놓고, 관측자에게 피관측자의 serde 타입을 그대로 쓰라고 했다. 그러면 서버 타입이 계약에서 벗어나도 봇이 같이 벗어나 **두 출처가 사이좋게 틀린다** — 3자 대조의 한 축이 사라진다.
  - **이 독립성이 실제로 값을 했다**: qa가 실서버 probe 1회로 **자기 하네스의 집계 버그**(Close 교환에서 나중 값이 먼저 값을 덮어써 정상 종료가 "서버가 먼저 닫음"으로 기록되던 것)를 찾았다. 그대로 뒀으면 SC-52가 **모든 정상 세션에서 거짓 FAIL**이었고, 원인을 서버에서 찾았을 것이다.
  - 대신 지켜야 할 것: 봇 타입은 `contracts/fixtures/**`로 모양을 고정하고(독립이되 표류하지 않는다), 커버리지 스크립트가 찾을 **리터럴 타입 이름 상수**를 유지한다.
- 커버리지 스크립트가 `tools/bots/**/*.rs`에서 타입 이름을 찾는다. **리터럴 상수**를 쓰면 `bots` 태그 경고 4건이 사라진다.
- 스펙 §7의 4단계(A 정상 / B 회전 / C 백프레셔 / **D 기록 내구성**)를 구현한다. **시드를 인자로 받고 로그에 남긴다.**
- **A 단계는 Unity 클라이언트가 먼저 붙은 뒤에 시작한다**(Editor 로드만 21초 — client 실측).
- **`SESSION_READY`에서 받은 `correlation_id`와 `session_id`를 수집해 파일로 남긴다.** QA의 모든 DB 대조가 이 집합으로 이뤄진다(전수 `count(*)`는 다른 실행·탐침 행이 섞여 쓸 수 없다).
- 봇이 직접 측정: 보낸 명령 수, `COMMAND_RESULT` 수와 `command_id` 대응, `PING_REPLY` 수와 `probe_seq` 대응, **`PING_REPLY`가 항상 `COMMAND_RESULT` 뒤였는지**, 왕복 지연 전량, 거부 사유별 계수, 연결 수립·종료 시각.
- 개발용 토큰 30개를 `STARFALL_DEV_AUTH_SECRET`으로 결정적으로 생성. 파일에 쓰지 않는다.
- `k6`는 이 PC에 없다. Rust 봇만 쓴다.

**T13 검증 SQL·스모크** (`tests/e2e/**`):

- SQL은 `docker compose exec -T postgres psql -U starfall -d starfall -At -c "..."`. HTTP는 **`curl.exe`**(503 기대 점검에는 `-SkipHttpErrorCheck`).
- 필수 쿼리: correlation 집합 기반 세션 쌍 대조, `sequence` 빈틈 검사(AC-16c SQL), `occurred_at` 재계산 대조(표본 10건), append-only 시도(UPDATE·DELETE **둘 다 실패**), `ON CONFLICT` 재삽입, `UNIQUE (world_id,tick,sequence)` 위반, `worlds.last_tick` vs `max(tick)`.
- **append-only 탐침 행은 전용 `tick` 값 + `sequence` 0부터 연속**으로 넣는다. 아니면 AC-16(c)를 자기가 깨뜨린다.
- AC-2용 `docker compose down -v`는 **레포 루트에서** 실행한다(starfall 프로젝트에만 작용).

**T14 부하 실행과 리포트**:

- **3자 대조가 핵심 증거**: 봇 관측 == `/debug/stats` == DB 행 수. 하나라도 다르면 그 차이가 곧 발견이다.
- 실행 중 가시성(AC-17a)은 **A 단계가 도는 동안** 쿼리해야 한다. 끝나면 증명할 수 없다.
- AC-17(b)는 **호스트에서 `count(*)` 폴링**으로 잰다. `recorded_at` vs 호스트 시계 비교는 컨테이너 시계 차이가 섞인다.
- **D 단계(AC-19)**: A 도중 `docker compose stop postgres` → 30초 후 새 연결이 503 `reason=recording_backlog` → 기존 세션 유지 확인 → `docker compose start postgres` → 백로그 소진 → **한 건도 유실 없음**. 5분이면 되고, Phase 0 종료 기준의 "실시간 이벤트 기록"에 가장 직접적인 실증이다.
- 커버리지 `--strict`는 **T1·T7·T8·T12가 끝난 뒤** 실행한다(0/0이 통과 조건).
- **측정 환경을 리포트에 적는다**: Unity Editor 실행 여부(전역 타이머 해상도에 영향), 동시에 도는 다른 프로젝트 컨테이너 수. 없으면 다음 측정과 비교할 수 없다.
- 실행하지 못한 항목은 PASS가 아니라 **"미검증(환경)"**. 검사 건수를 항목마다 적는다.

---

## 계약 변경이 필요할 때

server·client·qa 누구도 `contracts/**`를 직접 고치지 않는다. 근거와 함께 architect에게 요청하고, architect가 스키마·레지스트리·fixture를 함께 고쳐 **전원**에게 알린다.

이번 검토에서 실제로 반영된 계약 변경 2건: `SESSION_READY/tick-zero.json`의 `world_id` 교체(I-19 모순 해소), `SESSION_CLOSED/invalid/actor-id-null.json` 추가(반례 15→16, 좁힘 검증 대칭화). 둘 다 호환 변경이며 스키마·레지스트리는 바뀌지 않았다.

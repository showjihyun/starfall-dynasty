# p0-02-networking-spike: 30 연결에서 명령이 tick을 돌고 사실이 기록된다

- 상태: **agreed** (server·client 검토 + 사용자 결정 반영, 2026-09-18)
- 로드맵 Phase: p0 (사전 제작) — **이 슬라이스가 Phase 0 종료 기준을 증명한다**
- 근거 기획안 절: GDD §39 (Phase 0 종료 기준 "30명 동시 접속 + 실시간 이벤트 기록"), GDD §33 (서버 판정), TECH §7–8 (Reliable/Realtime 분리, WebSocket), TECH §12–13 (Simulation Tick, 결정성), TECH §26 (보안), TECH §33 (부하 테스트), HSE §7–8 (DomainEvent), HSE §30·§32 (순서·결정성), HSE §35–36 (게임 시간), HSE §101–102 (Envelope, Correlation vs Causation)
- 관련 ADR: 0005(실시간 전송·프레이밍), 0006(tick·게임 시간·명령 큐), 0007(도메인 이벤트 영속화), 0008(개발용 인증) — **전부 accepted**. 기반: 0001(모듈 구조), 0002(계약 형식), 0003(로컬 환경)
- 검토 기록: `_workspace/p0-02-networking-spike/01_server_spec_review.md`, `01_client_spec_review.md`, 반영 내역 `01_architect_decisions.md`
- 선행 슬라이스: `docs/specs/p0-01-bootstrap.md` (implemented), `_workspace/p0-01-bootstrap/05_summary.md`

## 1. 목표

이 슬라이스가 끝나면 **게임플레이는 여전히 하나도 없지만**, 게임을 만들 수 있는 구조가 실행으로 증명된다:

Unity 클라이언트가 Rust 서버에 WebSocket으로 붙어 자기가 누구인지 **서버에게 통보받고**, 계약으로 정의된 명령을 보내면 그 명령이 **핸들러가 아니라 고정 tick 루프에서** 판정되며, 그 과정에서 생긴 **세계의 사실이 PostgreSQL에 append-only로 기록**되고 SQL로 조회된다. 그리고 이 전부가 **30개 동시 연결에서 유지되며, 유지되었다는 사실이 숫자로 남는다**.

플레이어가 할 수 있는 것은 여전히 없다. 이것이 의도다(원칙 8). 이 슬라이스가 증명하는 것은 재미가 아니라 **p1의 모든 기능이 올라탈 경로가 실제로 존재한다**는 것이다.

## 2. 작업 흐름 (플레이 흐름 대체)

게임플레이가 없으므로 "한 연결의 생애"를 흐름으로 정의한다. 각 단계의 주체를 명시한다.

1. **개발자**가 `docker compose up -d`로 인프라를 띄우고 `.env`에 `STARFALL_DEV_AUTH_SECRET`이 있는지 확인한다.
2. **서버**가 기동한다. `sqlx::migrate!`가 `worlds`·`domain_events`를 만들고 스파이크 월드 1행을 시드한다. 서버는 자기 설정(`tick_hz`)과 `worlds` 행을 대조하고, 다르면 **기동을 거부**한다.
3. **서버**가 **그 월드의 마지막 tick 다음 번호**에서 20 Hz tick 루프를 시작한다(전용 OS 스레드). 이후 tick 번호는 1씩만 증가하고 **뒤로 가지 않는다**.
4. **클라이언트**가 `GET /ws`에 `Authorization: Bearer <dev token>`을 붙여 붙는다.
5. **서버**가 업그레이드 **전에** 토큰을 검증한다. 실패하면 401이고 소켓은 열리지 않는다 — 세션도, 기록도 없다.
6. **서버**가 다음 tick에서 세션을 수락하고, `SESSION_OPENED` 도메인 이벤트를 발행하며, 그 tick 번호를 실은 `SESSION_READY`를 보낸다. 클라이언트는 여기서 자기 `actor_id`·`world_id`·`session_id`·`tick_hz`를 **알게 된다**(주장하지 않는다).
7. **클라이언트**가 `PING_SERVER{probe_seq}`를 보낸다. 게이트웨이는 **큐에 넣기만 한다.**
8. **서버**의 다음 tick이 큐를 제출 순번대로 비우고 판정한다. 접수되면 `COMMAND_RESULT{ACCEPTED}`를, 이어서 `PING_REPLY`를 같은 tick에서 보낸다. 거부되면 `COMMAND_RESULT{REJECTED, reason_code}`만 보낸다(큐에 넣지 못한 거부는 게이트웨이가 만든다).
9. **클라이언트**가 `command_id`로 두 메시지를 자기 대기 목록과 맞추고 왕복 시간을 자기 시계로 잰다.
10. **클라이언트**가 연결을 닫거나, 서버가 닫는다(유휴·위반·느린 소비자·종료).
11. **서버**가 다음 tick에서 `SESSION_CLOSED{close_reason}`을 발행한다. `SESSION_OPENED`와 같은 `correlation_id`를 가진다. **세션 열기·닫기 제출은 거부 대상이 아니다** — 명령 큐가 가득 차도 닫기가 사라지지 않는다.
12. **영속화 태스크**가 각 tick의 이벤트와 `worlds.last_tick` 갱신을 **한 트랜잭션**으로 쓴다.
13. **QA**가 `psql`로 그 행들을 조회하고, 봇이 관측한 수와 `/debug/stats`의 수와 대조한다. 셋이 같아야 한다.
14. **연결이 끊기면** 클라이언트가 지수 백오프 + full jitter로 재연결한다. 재연결은 **새 세션**이다 — 새 `session_id`, 새 `correlation_id`, 새 이벤트 쌍. 끊길 때 미완료였던 명령은 **다시 보내지 않는다.**
15. **정상 종료**: 서버가 Ctrl-C 또는 stdin `shutdown` 한 줄을 받으면 수락을 멈추고, 다음 tick에서 살아 있던 모든 세션을 `SERVER_SHUTDOWN`으로 닫고, 그 이벤트가 커밋될 때까지 기다린 뒤 종료한다.

## 3. 범위

**포함**

- ADR-0005/0006/0007/0008(accepted), 이 스펙, 작업 분해, 검토 반영 기록
- 계약: `COMMAND_RESULT`·`SESSION_READY`(서버 메시지), `SESSION_OPENED`·`SESSION_CLOSED`(도메인 이벤트) 신설. `PING_SERVER`/`PING_REPLY`는 스키마 변경 없이 레지스트리에 `bots` 태그만 추가. **유효 fixture 타입당 2건, 반례는 타입별로 다름**(§5.3)
- 서버: `GET /ws` WebSocket 게이트웨이(업그레이드 전 인증, 프레이밍 규칙, 수명 주기, 정상 종료), 고정 20 Hz tick 루프(전용 OS 스레드), 명령 큐와 거부 정책, 제어 경로 분리, 세션 레지스트리, 도메인 이벤트 발행
- 서버: 첫 sqlx 마이그레이션(`worlds`, `domain_events`, append-only 트리거), tick 단위 트랜잭션 영속화, tick 재개
- 서버: 새 크레이트 `starfall-sim`(IO 없음·결정적), `starfall-persistence`. `/debug/stats` 운영 엔드포인트. `/readyz` 재시도(p0-01 이월). stdin `shutdown` 종료 경로
- 클라이언트: `Starfall.Net` 어셈블리, `IRealtimeTransport` + PC WebSocket 구현, 메인 스레드 마샬링, `message_type` 디스패치, `Runtime` 직렬화 프로필, 명령 대기 목록, 재연결 백오프, 리로드·종료 훅의 정상 Close
- 클라이언트: 생성기의 **좁힘 처리 수정**, 새 계약 DTO 생성, EditMode 테스트, 실서버 대상 PlayMode 스모크
- QA: `tools/bots/` Rust 봇 하네스(별도 Cargo 워크스페이스), 30 연결 부하 실행, DB·메트릭·봇 3자 대조, DB 중단·복구 무손실 검증, 계약 커버리지, 경계면 교차 검증

**제외 (다음 슬라이스로)**

- 게임플레이 일체(이동·채굴·전투·인벤토리·거래), 월드 상태 모델, `starfall-domain` 크레이트
- Historical Event 판정·중요도·증거·연대기 (p2)
- Transactional Outbox, 지속 멱등성(`processed_commands`), NATS, Redis 세션 저장 — **ADR-0007 §6과 ADR-0006 §6이 각각의 마감 기한을 명시한다**
- WebGL 전송 구현(추상화만), 브라우저용 자격 증명 전달 경로
- 실제 인증·계정·권한·TLS·레이트 리밋 (ADR-0008 §4)
- 함선·씬·비주얼·UI (techart 미투입)
- CI(GitHub Actions), 커밋·푸시, `sqlx-cli`/`.sqlx/` 오프라인 메타데이터 — **사용자 결정으로 다음 슬라이스에 함께 묶는다**
- 스냅샷·상태 동기화·예측·보간

## 4. 규칙과 불변식

게임 상태가 없으므로 서버 판정 8단계 중 **1단계(행위자 확인)만** 실제로 동작한다. 나머지 7단계는 검증할 상태가 없어 이번 슬라이스에 존재하지 않는다 — 빈 함수로 만들어 두지 않는다. 이 슬라이스가 지금부터 강제하는 불변식은 다음이다(p0-01의 I-1~I-9는 계속 유효하다).

- **I-10 행위자는 서버가 정한다.** `actor_id`는 검증된 개발용 토큰의 주체에서만 나온다. 명령 payload의 어떤 필드도 `actor_id`가 될 수 없고, 명령 envelope에는 행위자 필드 자체가 없다(I-6).
- **I-11 클라이언트가 보낸 값 중 서버가 그대로 쓰는 것은 `command_id`와 payload 필드뿐이다.** `command_id`는 **멱등 키로만** 쓰고, 내장 타임스탬프를 순서·시각으로 해석하지 않는다. `client_sent_at`은 로그·메트릭 전용이며 판정·정렬에 쓰지 않는다.
- **I-12 서버가 채우는 envelope 필드를 클라이언트 입력에서 복사하지 않는다.** `event_id`·`message_id`·`world_id`·`tick`·`sequence`·`occurred_at`·`recorded_at`·`correlation_id`·`causation_id`·`actor_id`는 전부 서버가 만든다. 특히 `correlation_id`를 `command_id`로 대신하지 않는다 — 클라이언트가 서로 무관한 트랜잭션을 같은 correlation으로 묶을 수 있게 된다. `causation_id`에 들어갈 수 있는 값은 **서버가 이미 접수·중복 제거를 마친 명령의 `command_id`이거나 서버가 발행한 이벤트의 `event_id`뿐이며**, payload에서 읽은 값은 절대 아니다(이번 슬라이스에서는 둘 다 `null`이다).
- **I-13 상태는 tick 안에서만 바뀐다.** WebSocket 수신 태스크는 세션·시뮬레이션 상태를 변경하지 않고 제출만 한다. 크레이트 경계로 강제한다: `starfall-gateway`는 `starfall-sim`의 제출 핸들만 가지며 상태 타입에 접근할 수 없다. **게이트웨이가 현재 tick 번호를 읽는 것은 상태 변경이 아니므로 위반이 아니다**(큐에 넣지 못한 명령의 거부 응답에 필요하다).
- **I-14 `COMMAND_RESULT.status == ACCEPTED`이면 `reason_code`는 `null`이고, `REJECTED`이면 `null`이 아니다.** 스키마는 이 상관을 표현하지 않는다(ADR-0005 §4). 서버 타입과 테스트가 강제한다. **클라이언트는 이 불변식을 믿지 않고 `status`를 보고 분기한다.**
- **I-15 한 `command_id`에 대해 `COMMAND_RESULT`는 정확히 1건이고, 타입별 결과(`PING_REPLY`)보다 먼저 전달된다.** 이 1:1 관계가 부하 측정의 손실 계측 기준이다.
- **I-16 `SESSION_OPENED` 1건에 `SESSION_CLOSED`가 정확히 1건 대응하며, 둘은 같은 `correlation_id`를 가진다.** 프로세스가 정상 종료하면 살아 있던 모든 세션이 `SERVER_SHUTDOWN`으로 닫힌다. **이를 지키기 위해 세션 열기·닫기 제출은 명령 큐를 쓰지 않으며 거부 대상이 아니다**(ADR-0006 §4).
- **I-17 한 월드의 tick은 뒤로 가지 않는다.** 프로세스 생애 동안 정확히 1씩 증가하고, 재기동 시 **그 월드에 기록된 마지막 tick 다음부터 재개한다**(ADR-0006 §2.3). 건너뛰지 않고, 초과해도 따라잡지 않는다. 유휴 중 재기동으로 게임 시간이 **정체**할 수는 있으나 되감기지는 않는다.
- **I-18 한 `(world_id, tick)` 안의 `sequence`는 0부터 빈틈없이 증가한다.** DB의 `UNIQUE (world_id, tick, sequence)`가 마지막 방어선이고, 그 위반은 **정상 경로가 아니라 버그 신호**다. `sequence`는 도메인 이벤트 전용이며 서버 메시지는 이 공간을 쓰지 않는다.
- **I-19 `occurred_at`은 `tick`에서만 파생된다.** 실제 시각은 입력이 아니다. 파생 상수(`tick_hz`·`calendar_epoch`·`calendar_scale`)는 `worlds` 행에 있고 **그 월드의 수명 동안 불변**이다. 바꾸려면 새 `world_id`를 만든다. **같은 `world_id`를 쓰는 계약 fixture는 같은 `tick_hz`를 써야 한다.**
- **I-20 `domain_events`는 append-only다.** DB 트리거가 UPDATE·DELETE를 막는다. 정정은 새 레코드로 한다.
- **I-21 명령·응답·거부·인증 실패는 도메인 이벤트가 아니다.** 기록 범위는 ADR-0007 §1의 표가 전부다. 새 이벤트 타입은 architect가 그 표를 고칠 때만 생긴다.
- **I-22 큐가 가득 차도 조용히 버리지 않는다.** 명령은 거부(응답 있음)하거나, 송신 큐라면 연결을 닫는다(사유가 `SESSION_CLOSED`에 남는다). **영속화 채널은 이벤트를 버리지 않고 미룬다.** 어느 쪽이든 관측 가능하다.
- **I-23 재연결은 재개가 아니다.** 새 세션·새 id·새 이벤트 쌍이고, 미완료 명령은 재전송하지 않는다.
- **I-24 인증되지 않으면 세션이 없다.** 401·503으로 끝난 요청은 `SESSION_OPENED` 행을 만들지 않는다. 따라서 "동시 연결 수"와 "인증된 행위자 수"는 같은 수다.
- **I-25 검증은 출처가 독립일 때만 검증이다.** 같은 증감 지점에서 나온 값들의 항등식이나, 설계상 항상 참인 명제를 수용 기준으로 삼지 않는다(ADR-0006 §7). 이 슬라이스의 3자 대조는 **봇 관측 / 서버 메트릭 / DB 행**이다.

## 5. 데이터 계약

### 5.1 타입 (`contracts/registry/types.json`, `registry_version: 2`)

| 타입 | kind | 생산자 | 소비자 | 요지 |
|------|------|--------|--------|------|
| `PING_SERVER` | command | client, bots | server | 변경 없음. `bots` 태그 추가 |
| `PING_REPLY` | server_message | server | client, bots | 변경 없음. `bots` 태그 추가 |
| `COMMAND_RESULT` | server_message | server | client, bots | **신규.** `{command_id, status, reason_code}` |
| `SESSION_READY` | server_message | server | client, bots | **신규.** `{session_id, world_id, actor_id, tick_hz, server_version}` |
| `SESSION_OPENED` | domain_event | server | server | **신규.** `{session_id, transport}`. `actor_id` 비-null로 좁힘 |
| `SESSION_CLOSED` | domain_event | server | server | **신규.** `{session_id, close_reason}`. `actor_id` 비-null로 좁힘 |

`SESSION_*`의 소비자에 `history`를 넣지 않는다 — `server/crates/history`가 아직 없고, 없는 소비자를 등록하면 커버리지 스크립트가 **오류**로 막는다. p2에서 추가한다.

닫힌 값 집합:

- `COMMAND_RESULT.status`: `ACCEPTED` | `REJECTED`
- `COMMAND_RESULT.reason_code`: `MALFORMED_COMMAND` | `UNKNOWN_COMMAND_TYPE` | `SCHEMA_VERSION_UNSUPPORTED` | `DUPLICATE_COMMAND_ID` | `SERVER_BUSY` | `TOO_MANY_IN_FLIGHT` | `null`
- `SESSION_OPENED.transport`: `WEBSOCKET`
- `SESSION_CLOSED.close_reason`: `CLIENT_CLOSED` | `IDLE_TIMEOUT` | `PROTOCOL_VIOLATION` | `SLOW_CONSUMER` | `SERVER_SHUTDOWN` | `TRANSPORT_ERROR`

**이 집합들은 값이 추가될 수 있다.** 그래서 Rust는 닫힌 열거형(모르는 값 거부), C#은 `string`(모르는 값 경고 후 계속)으로 매핑한다(ADR-0005 §4). 값 추가는 같은 `schema_version`의 호환 변경이다. **client 실측으로 생성기가 이미 이 매핑대로 동작함이 확인됐다**(U-1 PASS).

### 5.2 Envelope 채움 책임

| 필드 | 누가 | 이 슬라이스의 값 |
|------|------|-----------------|
| `command_id` | **클라이언트** | UUIDv7. 서버는 멱등 키로만 쓴다(I-11) |
| `client_sent_at` | **클라이언트** | 로그·메트릭 전용. 판정에 쓰지 않는다 |
| `event_id`, `message_id` | 서버 | UUIDv7. 결정적 코어 **밖**의 주입된 생성기가 만든다(ADR-0002 §5) |
| `world_id` | 서버 | `worlds` 행에서. 스파이크 월드 `01a0b1c2-3d4e-7f01-8a2b-9c0d1e2f3a4b` |
| `tick` | 서버 | 이벤트를 발행한 / 메시지를 만든 tick. 큐에 넣지 못해 게이트웨이가 만드는 거부 응답은 게이트웨이가 읽은 **현재 tick** |
| `sequence` | 서버 | 그 tick 안의 발행 순서, 0부터. **도메인 이벤트 전용** — 서버 메시지 envelope에는 이 필드가 **없다**(`message-envelope.schema.json` 확인). 메시지가 `sequence` 공간을 같이 쓰면 `domain_events`의 `sequence`에 구멍이 생겨 AC-16(c)가 항상 실패한다 |
| `occurred_at` | 서버 | `tick`에서 결정적으로 파생(ADR-0006 §3). 실제 시각 입력 없음. 도메인 이벤트 전용 |
| `recorded_at` | 서버 | 영속화 단계에서 **Rust 호스트 시계**로. 감사 전용. 도메인 이벤트 전용 |
| `correlation_id` | 서버 | `SESSION_OPENED`·`SESSION_CLOSED`·`SESSION_READY` = 그 세션의 correlation(셋이 공유). **`COMMAND_RESULT`·`PING_REPLY` = 항상 `null`** |
| `causation_id` | 서버 | 이번 슬라이스는 항상 `null`(명령이 만든 도메인 이벤트가 없다) |
| `actor_id` | 서버 | 검증된 토큰의 주체. `SESSION_*`에서는 비-null. 도메인 이벤트 전용 |

**`COMMAND_RESULT`·`PING_REPLY`의 `correlation_id`는 항상 `null`이다**(B-13). 명령은 게임플레이 트랜잭션을 시작하지 않았고, 세션 correlation을 여기 넣으면 I-12가 경고하는 "무관한 것을 같은 correlation으로 묶는" 여지가 생긴다. 진짜 트랜잭션 correlation이 들어올 자리는 명령이 상태를 바꾸는 p1에 비워 둔다.
`contracts/fixtures/COMMAND_RESULT/accepted.json`은 널이 아닌 `correlation_id`를 갖는데, 이는 **널 가능 필드의 왕복을 덮는 스키마 커버리지용**이며 이번 슬라이스의 서버가 만드는 값이 아니다(p0-01의 `PING_REPLY/with-correlation.json`과 같은 성격).

### 5.3 파일과 건수

| 파일 | 상태 |
|------|------|
| `contracts/registry/types.json` | `registry_version` 1 → 2, 타입 2 → 6 |
| `contracts/messages/COMMAND_RESULT.schema.json` | 신규 |
| `contracts/messages/SESSION_READY.schema.json` | 신규 |
| `contracts/events/domain/SESSION_OPENED.schema.json` | 신규 (`contracts/events/domain/` 첫 파일) |
| `contracts/events/domain/SESSION_CLOSED.schema.json` | 신규 |

**건수 (실측, 2026-09-18):**

| | 수 | 근거 |
|---|---:|------|
| 스키마 파일 (`*.schema.json`, `registry/types.schema.json` 포함) | **11** | |
| 유효 fixture | **12** | 타입 6종 × 2 |
| 반례 fixture | **16** | PING_SERVER 4, PING_REPLY 3, SESSION_CLOSED 3, 나머지 각 2 |

초안은 유효를 **16으로 잘못 적었다**(p0-01 기준선 4를 8로 계산해 신규 8을 더한 값). server·client 두 리뷰어가 독립적으로 12를 실측했고, Unity EditMode 스위트도 같은 수를 센다. 반례는 이 개정에서 `SESSION_CLOSED/invalid/actor-id-null.json`을 추가해 15 → **16**이 됐다(§5.4의 좁힘 검증을 두 타입에 대칭으로 걸기 위해서다).

**architect가 오프라인 2020-12 검증기로 실행한 결과: 유효 12건 전부 통과, 반례 16건 전부 거부.** 유효 건수는 "타입 수 × 2"로 유도할 수 있지만 반례는 타입마다 달라 공식이 없다 — **반례 수가 바뀌면 architect가 이 표와 AC를 함께 갱신한다.**

### 5.4 반례 fixture의 층별 거부 책임

p0-01 §5 표를 이어간다. 운영 중 메시지를 막는 것은 스키마 검증기가 아니라 serde이므로 Rust 열이 핵심이고, `C# (Strict)` 열의 "감지 불가"는 버그가 아니라 **기록된 설계 결과**다.

| fixture | 스키마 | Rust serde | C# (Strict) | 비고 |
|---------|--------|-----------|-------------|------|
| `COMMAND_RESULT/invalid/unknown-reason-code.json` | 거부 | 거부 (닫힌 열거형) | **감지 불가** (실측) | `enum` → C# `string` 매핑의 의도된 대가(ADR-0005 §4) |
| `COMMAND_RESULT/invalid/payload-unknown-field.json` | 거부 | 거부 (`deny_unknown_fields`) | 거부 (실측) | |
| `SESSION_READY/invalid/tick-hz-zero.json` | 거부 (`minimum: 1`) | 거부 (범위 newtype) | **감지 불가** (실측) | 1..1000 → `int`가 0을 담는다 |
| `SESSION_READY/invalid/missing-session-id.json` | 거부 | 거부 (필수 필드) | 거부 (실측) | |
| `SESSION_OPENED/invalid/actor-id-null.json` | 거부 (좁힘) | 거부 (비-`Option`) | **거부 — T7의 생성기 수정 후** | 현재 생성기는 **통과시킨다**(U-2 FAIL 실측). ADR-0005 §4-3의 A안으로 고친다 |
| `SESSION_OPENED/invalid/missing-world-id.json` | 거부 | 거부 | 거부 (실측) | |
| `SESSION_CLOSED/invalid/actor-id-null.json` | 거부 (좁힘) | 거부 (비-`Option`) | **거부 — T7의 생성기 수정 후** | 이 개정에서 추가. 좁힘 수정이 두 타입 모두에 적용됐는지 확인한다 |
| `SESSION_CLOSED/invalid/unknown-close-reason.json` | 거부 | 거부 | **감지 불가** (실측) | 위와 같은 이유 |
| `SESSION_CLOSED/invalid/correlation-id-null.json` | 거부 (envelope 필수·비-null) | 거부 (비-`Option`) | 거부 (실측) | |

**"실측"은 client가 당시 존재하던 반례 15건을 C# `Strict`로 전수 역직렬화해 확인한 결과다**(거부 9 / 통과 6). 예측과 어긋난 것은 `SESSION_OPENED/invalid/actor-id-null.json` 1건이며, 그것이 U-2 FAIL이다. **16번째인 `SESSION_CLOSED/invalid/actor-id-null.json`은 이 개정에서 추가되어 아직 C#으로 측정되지 않았다** — 좁힘 수정 후 두 타입이 같이 거부되는지를 AC-11(b)가 확인한다.

**`Runtime` 프로필은 `Strict`보다 덜 잡는다.** 위 표에 더해 `payload-unknown-field` 계열도 통과시킨다(경고 로그는 남는다) — 의도된 비대칭이다(ADR-0002 §4). 다만 **필수 필드 누락과 널 불가 필드의 널은 `Runtime`에서도 예외다**(ADR-0005 §5). `Runtime`이 관용하는 것은 "모르는 멤버" 하나뿐이다.

### 5.5 운영 엔드포인트 (계약 아님)

| 경로 | 응답 |
|------|------|
| `GET /healthz` | 변경 없음 |
| `GET /readyz` | 변경 없음. 각 점검은 **즉시 실패에만** 1회 재시도하고 타임아웃에는 재시도하지 않는다(ADR-0007 §9). 점검당 예산 2초 유지 |
| `GET /ws` | 101 (업그레이드) / 401 (토큰 없음·무효) / 503 `{"status":"unavailable","reason":"auth_not_configured"}` (비밀 미설정) / 503 `{"status":"unavailable","reason":"recording_backlog"}` (영속화 백로그 초과) |
| `GET /debug/stats` | JSON. 카운터·게이지 + tick **본문** 소요 분포(고정 버킷 히스토그램). `tick`·`start_tick`·`tick_total` 포함(ADR-0007 §8) |

**stdin 종료 경로 (승인된 범위 추가).** Windows에는 SIGTERM이 없고, 에이전트가 백그라운드로 띄운 서버 프로세스에 CTRL_C_EVENT를 보낼 표준 경로도 없다. 하드 킬은 graceful shutdown 경로를 **전혀 타지 않으면서** outbox 부재로 인한 손실을 재현해, 결과가 "graceful shutdown이 깨졌다"로 보인다. 그래서 **서버는 stdin에서 `shutdown` 한 줄을 읽으면 Ctrl-C와 같은 종료 경로를 탄다.** 새 네트워크 표면이 없고 구현이 몇 줄이며, 이것이 없으면 AC-7(d)는 이 PC에서 자동 검증이 불가능하다. 사람이 실제 터미널에서 Ctrl-C를 눌러 확인하는 경로도 함께 유지한다.

## 6. 역사 연결

이 슬라이스는 **Historical Event를 하나도 만들지 않는다.** 판정 규칙·`rule_version`·중요도·가시성은 p2 범위이고 history-engine-engineer 검토 대상이다. 대신 역사 시스템이 나중에 요구할 것을 **지금 실행 가능한 형태로** 확보한다.

| 도메인 이벤트 | 중요도 | 지금 기록하는 이유 | 나중에 무엇으로 이어지는가 |
|--------------|:---:|------|------------------|
| `SESSION_OPENED` | Level 0 (역사 아님) | 인증된 행위자가 tick T부터 세계에 존재했다는 사실. 존재 구간의 시작 | 전기(Biography)의 "활동 기간", 사건 재구성에서 "그때 거기 있었는가"의 1차 재료. 어떤 주장(Claim)이 특정 인물을 사건에 연결할 때, 그 인물의 존재 구간이 **반증 가능성**을 만든다 |
| `SESSION_CLOSED` | Level 0 (역사 아님) | 존재 구간의 끝과 그 **이유**. "떠났다"와 "잘렸다"는 다른 사실이다 | 위와 같음. `close_reason`은 나중에 "전투 중 접속이 끊긴 것"과 "도망친 것"을 구분해야 할 때 필요한 원자적 사실이 된다 |

**이 두 타입은 Historical Event로 승격되지 않는다.** 접속은 사건이 아니다. 여기 적는 이유는 반대 방향의 실수를 막기 위해서다 — 중요도 판정기를 만들 때 "모든 도메인 이벤트에 레벨을 매긴다"고 접근하면 접속 로그가 연대기에 올라간다. 판정기의 기본값은 **판정하지 않음**이어야 한다.

계약 수준에서 이미 확보된 것(p0-01에서 정의, 이번에 **처음으로 실제 값이 채워진다**): `tick`·`sequence`(결정적 순서), `correlation_id`/`causation_id` 분리(HSE §102), `occurred_at`(게임 시간) / `recorded_at`(실제 시간) 분리, `schema_version`(업캐스팅 근거), `world_id`(샤딩·다중 월드).

이번에 새로 확보한 것 둘:

1. **`occurred_at`의 파생 규칙과 그 상수가 월드에 묶여 불변**이라는 것(I-19). 이것이 없으면 역사 엔진이 읽는 게임 시간은 "그때 설정이 어땠는지"에 달린 값이 된다.
2. **게임 시간이 되감기지 않는다**는 것(I-17). tick 재개가 없으면 서버 재기동마다 게임 시간이 과거로 돌아가고, 같은 게임 시각에 서로 다른 두 사건이 존재하게 된다 — 역사 기록으로서 회복 불가능한 상태다. 대신 **다운타임만큼 게임 시간이 정체**하는데, 그것은 "그 동안 아무 일도 기록되지 않았다"는 참인 진술이다.

## 7. 수용 기준

모든 Then은 실행 증거다. 실행하지 못한 항목은 PASS가 아니라 **"미검증(환경)"**이다. 증거에는 **몇 건을 검사했는지**가 드러나야 한다.

**실행 위치와 도구.** cargo 명령은 `server/`에서, 나머지는 레포 루트에서. HTTP 확인은 `curl.exe`(PowerShell의 `curl`은 `Invoke-WebRequest` 별칭이고, 503을 기대하는 점검에는 `-SkipHttpErrorCheck`가 필요하다). SQL 확인은 `docker compose exec -T postgres psql -U starfall -d starfall -At -c "..."`(2026-09-18 실행 확인).

**DB 초기 상태.** 이 PC에는 p0-01이 만든 `starfall_postgres-data` 볼륨이 **이미 있다.** AC-2의 "볼륨 없는 상태"는 레포 루트에서 **`docker compose down -v`**로 만든다 — `docker-compose.yml`의 `name: starfall` 덕분에 이 명령은 **starfall 프로젝트에만** 작용한다(다른 프로젝트 컨테이너 5개와 볼륨 90여 개는 건드리지 않는다). 금지된 것은 전역 `docker system prune`·`docker volume prune`·프로젝트 밖 `down -v`다. 마이그레이션 `.sql`을 고친 뒤에도 `down -v`가 필요하다(체크섬 불일치로 기동이 실패한다).

### 서버

- **AC-1 (server) 게이트.** When `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace --locked`, Then 세 명령 모두 종료 코드 0. 새 크레이트 `starfall-sim`·`starfall-persistence`가 `[lints] workspace = true`를 옵트인했음을 구현 요약에 명시한다. **`starfall-sim`의 `Cargo.toml`에 `axum`·`sqlx`·`redis`·`rand`·`chrono`가 없다**(IO-free·결정성 위생의 기계적 증거).
- **AC-2 (server) 마이그레이션과 월드.** Given `docker compose down -v` 후 인프라 재기동, When 서버를 기동, Then `\d domain_events`·`\d worlds`에 ADR-0007 §2의 열·제약이 전부 있고, `select count(*) from worlds` = 1이며 그 행의 `world_id`·`tick_hz`·`calendar_epoch`·`calendar_scale`이 시드값과 **문자열/정수로 같고** `last_tick`은 `NULL`이다. When 서버를 **두 번째로** 기동, Then 마이그레이션이 다시 적용되지 않고 기동에 성공한다. When `STARFALL_TICK_HZ`를 10으로 바꿔 기동, Then 서버가 `worlds.tick_hz`(20) 불일치를 이유로 **기동을 거부**한다.
- **AC-3 (server) tick 재개.** Given 서버가 한 번 떠서 세션 1개를 열고 닫은 뒤 정상 종료, When `select last_tick from worlds`와 `select max(tick) from domain_events`, Then 두 값이 서로 모순되지 않는다(`last_tick >= max(tick)`). When 서버를 **다시 기동**, Then `/debug/stats`의 `start_tick`이 **그 `max(tick)`보다 크고**, 두 번째 실행이 만든 이벤트가 **하나도 유실되지 않는다**(첫 실행과 두 번째 실행의 세션 쌍이 각각 온전히 존재한다). 이 AC가 없으면 재기동이 섞인 모든 측정이 비결정적으로 일부 행을 잃는다.
- **AC-4 (server) append-only.** Given `domain_events`에 행이 최소 1건, When `update domain_events set event_type='X'`와 `delete from domain_events`를 각각 실행, Then **둘 다 예외**로 실패하고 메시지에 `append-only`가 들어 있으며, 실행 후 행 수가 변하지 않는다. When 같은 `event_id`를 다시 삽입, Then 행 수가 늘지 않는다. When 다른 `event_id`로 같은 `(world_id, tick, sequence)`를 삽입, Then **UNIQUE 위반으로 실패**한다. **탐침 행은 이 실행이 쓰지 않는 전용 `tick` 값을 쓰고 `sequence`를 0부터 연속으로 넣는다** — 그러지 않으면 AC-16(c)의 빈틈 검사가 탐침 행 때문에 실패한다.
- **AC-5 (server) 인증.** (a) 유효 토큰 → 101 업그레이드 후 첫 메시지가 `SESSION_READY`이고 그 `actor_id`가 토큰 주체와 같다. (b) 헤더 없음 / 서명 불일치 / 주체가 UUIDv7이 아님 **3가지 모두** → 401, 소켓 열리지 않음, 해당 시도 후 `SESSION_OPENED` 행 수 증가 0. (c) `STARFALL_DEV_AUTH_SECRET` 미설정으로 기동 → `GET /ws`가 503이고 본문의 `reason`이 `auth_not_configured`이며, **같은 프로세스에서 `/healthz` 200, `/readyz` 200**이다.
- **AC-6 (server) tick 루프가 실제로 판정한다.** (a) tick 루프를 수동 step 모드로 둔 통합 테스트에서, 명령 N건을 제출한 직후 어떤 이벤트도 발행되지 않고 세션 상태가 변하지 않는다. 1 step 실행 후 **정확히 기대한 결과만** 나온다. **이 테스트는 tokio 런타임과 소켓 없이 돈다**(`TickOutcome` 반환값 기반 — ADR-0006 §4). (b) `/debug/stats`에서 **`tick_total == tick − start_tick + 1`**이고, 2회 호출 사이에 `tick`과 `tick_total`이 같은 양만큼 증가한다. (c) 한 `command_id`에 대해 클라이언트가 받은 프레임 순서가 `COMMAND_RESULT` → `PING_REPLY`이고 두 메시지의 envelope `tick`이 같다.
- **AC-7 (server) 큐·거부·수명 주기.** (a) 한 세션에서 in-flight 상한(64)을 넘겨 폭주시키면 `COMMAND_RESULT{REJECTED, TOO_MANY_IN_FLIGHT}`가 오고 **연결은 유지**되며, 보낸 명령 수 = 받은 `COMMAND_RESULT` 수다(손실 0). (b) 같은 `command_id`를 2회 보내면 `COMMAND_RESULT{ACCEPTED}` 1건 + `PING_REPLY` 1건 + `COMMAND_RESULT{REJECTED, DUPLICATE_COMMAND_ID}` 1건이 오고 `PING_REPLY`는 **2건이 아니다**. (c) 수신을 멈춘 클라이언트에 송신 큐 상한을 넘겨 보내면 연결이 close 1011로 닫히고 `close_reason = SLOW_CONSUMER`인 `SESSION_CLOSED` 행이 생긴다(in-flight 해제가 "COMMAND_RESULT 생성 시점"이어야 도달 가능하다 — ADR-0006 §5). (d) `SERVER_BUSY` 경로는 **단위 테스트로** 덮는다. 30 연결 부하에서는 in-flight 상한(30×64=1920 < 4096) 때문에 **구조적으로 도달하지 않으며**, 리포트에 "부하 실행으로는 미도달(구조적)"으로 기록한다.
- **AC-8 (server) 프레이밍과 종료.** (a) 16 KiB를 넘는 텍스트 메시지 → 프로토콜 위반으로 계수, 10초 창 8회 초과 시 close 1002 + `close_reason = PROTOCOL_VIOLATION`. **라이브러리 한도(64 KiB)가 앱 한도(16 KiB)보다 높아 앱이 위반을 셀 수 있음**을 실행으로 보인다(같은 값이면 tungstenite가 먼저 끊어 계수 자체가 불가능하다). (b) 바이너리 프레임 → 같은 처리. (c) Pong·데이터 프레임을 30초간 보내지 않으면 close 1001 + `IDLE_TIMEOUT`. (d) **stdin `shutdown` 한 줄 또는 터미널 Ctrl-C**로 정상 종료 시 살아 있던 모든 세션이 `SERVER_SHUTDOWN`으로 닫히고, **열려 있던 세션 수만큼** `SESSION_CLOSED` 행이 추가된다(I-16). 하드 킬로 대체하지 않는다 — 하드 킬은 outbox 부재의 손실을 재현할 뿐 이 AC를 판정하지 못한다. (e) 각 경우의 close code가 ADR-0005 §2 표와 일치한다.
- **AC-9 (server) 운영 표면.** (a) 인프라가 뜬 직후 `/readyz`를 처음 호출해도 200이다(p0-01 이월). `docker compose stop postgres` 후 503, 재시작 후 **서버 재시작 없이** 200 복귀는 그대로 유지된다. (b) `/debug/stats`의 `ws_connections`가 **세션 레지스트리의 실제 길이에서 계산**되고, `ws_connections == sessions_opened_total − sessions_closed_total`이며, 이 값이 같은 시점 DB의 `SESSION_OPENED` − `SESSION_CLOSED` 행 수(해당 correlation 집합 안에서)와 같다. (c) **측정 창 델타 기준으로** `Δcommands_received_total − Δmessages_enqueued_total{COMMAND_RESULT} == 0`(서버 내부 1:1 불변식), `messages_enqueued_total − messages_written_total`이 큐 잔량 + `messages_dropped_total`과 일치한다.

> **누적 기준으로는 성립하지 않는다 — 문구가 조건을 빠뜨렸다** (2026-09-19 정정, QA 실측). 누적으로 재면 차이가 **66**이 나온다. 원인은 **연결이 이미 닫힌 뒤 도착한 명령**이다(프레임 상한 위반으로 서버가 끊은 세션 — SC-22). 서버는 그 명령을 받아 세긴 하지만 보낼 소켓이 없으므로 `COMMAND_RESULT`를 큐에 넣을 수 없고, 같은 현상이 `messages_dropped_total`(실측 255)로도 드러난다. **이것은 결함이 아니라 I-15가 애초에 "살아 있는 세션"에 대한 진술이라는 뜻이다.** 부하 창 델타로 재면 `9,471 − 9,471 = 0`으로 정확히 성립한다(QA 실측). 판정은 **측정 창 델타** 또는 **정상 종료 세션 집합**으로 하고, 창 밖의 차이는 `messages_dropped_total`로 설명되는지 확인한다.
- **AC-10 (server) 계약 테스트.** When `cargo test -p starfall-contracts --locked`, Then ADR-0002 §3의 테스트 9종이 **타입 6종 전부**를 덮고 통과한다. 특히 (a) 유효 fixture **12건**이 왕복 후 `serde_json::Value` 비교로 동일, (b) 반례 **16건이 스키마 검증에서 전부 거부**, (c) 반례 16건의 Rust 역직렬화 결과가 §5.4 + p0-01 §5 표의 "Rust serde" 열과 일치, (d) 각 fixture에서 `required` 필드를 하나씩 제거한 변이가 전부 실패, (e) `producers`·`consumers`에 `server`가 있는 타입 6종이 모두 이름→Rust 타입 대응표에 있다. **테스트가 검사한 건수(12/16/11)를 출력에 찍는다.**

### 클라이언트

- **AC-11 (client) 생성기.** (a) When `dotnet run tools/codegen/ContractsCodegen.cs -- --contracts contracts --out client/Assets/_Project/Scripts/Contracts/Generated`를 두 번 실행, Then 두 번째 실행 후 파일 해시가 변하지 않고 `--check`가 종료 코드 0. (b) **좁힘 수정(ADR-0005 §4-3 A안) 후** `SessionOpenedEvent.ActorId`와 `SessionClosedEvent.ActorId`가 `System.Guid` + `Required.Always`이고, `SESSION_OPENED/invalid/actor-id-null.json`·`SESSION_CLOSED/invalid/actor-id-null.json`이 C#에서 **거부**된다. (c) 수정 전후 생성물 diff를 떠서 **변한 것이 그 두 속성뿐임을 증명**하고 구현 요약에 붙인다. (d) `enum`을 allowlist의 "제약" 분류로 옮긴다(동작 무변화, 주석 정정).
- **AC-12 (client) EditMode.** When `unity test client --mode EditMode --report-format nunit,junit --output _workspace/p0-02-networking-spike/unity-tests/EditMode.nunit.xml --junit-output _workspace/p0-02-networking-spike/unity-tests/EditMode.xml`, Then 종료 코드 0, 실패 0, **두 리포트 파일 존재**, **리포트의 `tests` 수가 0이 아니고 그 수를 구현 요약에 적는다**(매칭 0건 필터는 종료 코드 0 + `tests="0"`을 만든다 — client 실측). 포함되는 것: (a) 유효 fixture **12건** 왕복(`Strict`). (b) §5.4 + p0-01 §5에서 C# 책임인 반례가 전부 거부되고, "감지 불가" 항목이 실제로 통과함을 테스트가 **명시적으로 기록**한다. (c) 알 수 없는 `message_type`을 디스패치하면 예외 없이 경고만 남고 이후 메시지 처리가 계속된다. (d) `Runtime` 프로필이 payload의 모르는 필드를 **무시하고 경고를 남기며**, 같은 입력이 `Strict`에서는 예외가 된다. **그리고 `Runtime`에서도 필수 필드 누락(`missing-session-id.json`)과 널 불가 필드의 널(`correlation-id-null.json`)은 예외다.** (e) 백오프 지연이 순수 함수이고 `base=500ms, factor=2, cap=10s, full jitter, 지수 클램프 5` 규칙을 만족하며, **`n = 0, 1, 5, 62, 63, 64, 100`의 경계 케이스에서 전부 `0 < delay <= 10s`**다(클램프가 없으면 n=62,63에서 0 ms가 된다 — client 실측). 고정 시드로 결정적이다. (f) fixture 로더가 유효 fixture를 **12건 미만** 발견하면 테스트가 실패한다.
- **AC-13 (client) 실서버 왕복.** Given 서버와 인프라가 떠 있음, When Unity가 `Starfall.Net`으로 `/ws`에 붙어 `PING_SERVER` 3건을 보내고 정상 종료, Then (a) `SESSION_READY`를 첫 메시지로 받고 `tick_hz == 20`, (b) `COMMAND_RESULT` 3건 + `PING_REPLY` 3건을 받아 `command_id`·`probe_seq`가 전부 맞고 순서가 I-15를 만족하며, (c) DB에 그 세션의 `SESSION_OPENED`/`SESSION_CLOSED` 1쌍이 같은 `correlation_id`로 있고 `close_reason = CLIENT_CLOSED`다. **(c)의 전제: 측정 중 도메인 리로드가 없어야 한다** — 리로드가 소켓을 비정상 종료시키면 `close_reason`이 `TRANSPORT_ERROR`가 된다. 리로드·PlayMode 종료·앱 종료 훅에서 정상 Close를 보내는 것이 T8의 필수 항목이다. 증거는 PlayMode 리포트 또는 Editor 로그 발췌 + SQL 출력. 실행할 수 없으면 **미검증(환경)**.
- **AC-14 (client) 재연결.** When 서버를 재시작(또는 연결을 강제 종료)하고 클라이언트를 그대로 둠, Then 클라이언트가 백오프 후 재연결해 새 `SESSION_READY`를 받고, DB에 **서로 다른 `session_id`와 서로 다른 `correlation_id`**를 가진 세션 쌍이 2개 생긴다. 끊길 때 미완료였던 명령이 재전송되지 않았음을 로그 한 줄(`starfall.net: dropping N in-flight command(s) on disconnect (no resend, I-23)`)로 확인한다. **시도 카운터가 `SESSION_READY` 수신 시에만 리셋됨**을 로그로 확인한다.

### QA — 30 연결

부하 실행의 형태를 고정한다. 숫자가 무엇을 뜻하는지가 형태에서 나온다.

| 단계 | 내용 | 산출 |
|------|------|------|
| **A 정상 상태** | Unity 클라이언트를 **먼저** 접속시킨 뒤, 봇 30개가 5초 안에 접속 → 60초 유지, 각 봇이 500 ms마다 `PING_SERVER` | 명령 3,600건, 세션 31쌍 |
| **B 회전** | 봇 30개가 각각 접속 → ping 3회 → 종료를 5회 반복 | 세션 150쌍 = 도메인 이벤트 300건 |
| **C 백프레셔** | 봇 29개가 A와 같은 부하를 유지하는 동안 1개가 2,000건을 최대 속도로 폭주 | 거부 계수, 격리 확인 |
| **D 기록 내구성** | A 단계 도중 `docker compose stop postgres` → 30초 대기 → `docker compose start postgres` | 백로그 소진, 무손실 |

**세션 집합의 정의.** 전수 `count(*)`로 판정하지 않는다 — DB는 실행마다 누적되고 AC-4의 탐침 행·AC-13/14의 단독 왕복이 섞인다. **봇과 Unity 클라이언트가 `SESSION_READY`에서 받은 `correlation_id`를 수집하고, QA는 그 집합으로 조회한다.**

```sql
select count(*) from domain_events
where event_type = 'SESSION_OPENED' and correlation_id = any($1::uuid[]);
```

배열 길이가 곧 "몇 건을 검사했는지"이고, 다른 실행이 섞여도 무해하다.

- **AC-15 (qa) 정상 상태 30+1 연결.** Given A 단계, Then (a) 31개 연결 전부 수립되고 서버가 먼저 닫은 연결이 **0건**, (b) 봇이 보낸 명령 수 == 받은 `COMMAND_RESULT` 수이고 `command_id` 1:1, 중복 0 (**손실 0건**), (c) `ACCEPTED` 수 == `PING_REPLY` 수이고 `probe_seq`·`command_id`가 전부 일치, 중복 0, (d) 모든 `PING_REPLY`가 그 명령의 `COMMAND_RESULT` **뒤에** 도착했다(I-15), (e) 서버 메트릭이 봇 관측과 일치한다(AC-9c).
- **AC-16 (qa) 기록 무결성.** Given A+B 종료 후, 수집한 correlation 집합에 대해, Then (a) `SESSION_OPENED` 행 수 == 집합 크기이고 `SESSION_CLOSED`도 같은 수, (b) `correlation_id`로 묶었을 때 **짝 없는 이벤트 0건, 중복 0건**, (c) 다음 SQL이 0행을 반환한다 — 각 `(world_id, tick)`의 `sequence`가 0..n−1로 빈틈없음:
  ```sql
  select world_id, tick from domain_events
  group by world_id, tick
  having count(*) <> max(sequence) + 1 or min(sequence) <> 0 or count(distinct sequence) <> count(*);
  ```
  (d) `occurred_at`이 전부 `GameTime` 패턴을 만족하고, 표본 10건에 대해 ADR-0006 §3 공식으로 `tick`에서 다시 계산한 값과 **문자열로 같다**.
- **AC-17 (qa) 실시간성.** (a) A 단계 **진행 중**(연결이 31개 열려 있는 동안) SQL을 실행하면 이미 `SESSION_OPENED` 31행이 있고 그 세션들의 `SESSION_CLOSED`는 0행이다 — 기록이 종료 시 몰아 쓰기가 아님을 증명한다. (b) 마지막 봇이 끊긴 시점부터 **5초 이내**에 기대한 모든 행이 존재한다. **측정은 호스트에서 `count(*)`를 폴링해 "기대 행 수에 도달하기까지 몇 초"로 한다** — `recorded_at`과 호스트 시계를 비교하면 컨테이너 시계 차이가 섞인다.
- **AC-18 (qa) 백프레셔 격리.** Given C 단계, Then (a) 폭주 봇은 `TOO_MANY_IN_FLIGHT` 거부를 받지만 **보낸 명령 수 == 받은 `COMMAND_RESULT` 수**이고(거부도 응답이다) 연결이 유지되며, (b) 나머지 29개 봇의 손실이 0이고 연결이 끊기지 않으며, (c) 29개 봇의 왕복 p99가 A 단계 대비 **2배를 넘지 않는다**.
- **AC-19 (qa) 기록을 버리지 않는다.** Given D 단계, Then (a) PostgreSQL 중단 30초 후 **새 연결이 503 `reason=recording_backlog`**를 받고, (b) **기존 세션은 끊기지 않으며** 계속 ping 왕복을 한다, (c) PostgreSQL 복구 후 `persist_backlog`가 **정상 대역(0~20)으로 복귀**하고, (d) 중단 구간에 발생한 이벤트가 **한 건도 유실되지 않고** DB에 들어온다(correlation 집합 대조). 이것이 "기록 시스템이 기록을 버리지 않는다"의 유일한 실증이며, Phase 0 종료 기준의 "실시간 이벤트 기록"에 정확히 해당한다.

> **(c)의 "0"은 틀린 기대였다** (2026-09-19 정정). 서버 구현은 20 tick마다 하트비트 커밋을 하므로 **정상 동작에서도 `persist_backlog`가 0~20을 오간다**(QA가 실서버 preflight에서 `persist_backlog=4` 관측). 원래 문구대로 `--target 0` 폴링으로 판정했으면 **정상 서버에서 폴링이 영영 끝나지 않는다.** 판정은 "0에 도달"이 아니라 "**소진되어 정상 대역으로 돌아왔다**"이며, 상한 20은 하트비트 주기에서 나온 값이다. 하트비트 주기가 바뀌면 이 대역도 함께 바뀐다 — 그때 server가 architect에게 알린다.
- **AC-20 (qa) 계약 커버리지.** When `python .claude/skills/integration-qa/scripts/check_contract_coverage.py --strict`, Then 종료 코드 0(errors 0, warnings 0). **구현 전 기준선**(architect 실행, 2026-09-18): errors 8, warnings 4 — 코드 참조 없음 8건, `tools/bots` 루트 없음 4건. 이 8+4가 0이 되는 것이 통과 조건이며, 그 전 실행 결과는 판정에 쓰지 않는다. **같은 시점의 `cargo test`와 Unity EditMode도 빨간불이 기준선이다**(계약이 코드보다 앞서 있다 — server·client 실측: Rust는 스키마 개수·레지스트리 매핑 단언 실패, Unity는 26건 중 12건 실패). 이 빨간불은 p0-01의 테스트 설계가 의도대로 동작한다는 증거이지 결함이 아니다.
- **AC-21 (qa) 경계면 교차 검증.** Given 구현 완료, When 신규 4타입의 스키마·Rust 타입·C# DTO를 나란히 비교, Then 필드 이름·필수 여부·널 가능 여부가 3자 간 동일하고, 정수 필드의 언어 타입이 스키마의 `minimum`~`maximum`을 손실 없이 담으며, 층별 거부 범위가 §5.4 표와 일치한다. **판정 근거는 총계가 아니라 `_workspace/p0-02-networking-spike/03_client_impl.md` §9의 필드별 대조표다.** 불일치 건수(0이어야 한다)와 대조한 행 수를 리포트에 적는다. fixture의 `world_id`↔`tick_hz` 조합이 I-19와 모순되지 않는지도 확인한다.

> **총계 46 → 48로 정정, 열거 규약 명시** (2026-09-19). 초안의 46은 **메시지 envelope은 `payload` 컨테이너를 세고 이벤트 envelope은 세지 않은** 비대칭 계산이었다(client 자기 정정). 대칭 규약 — **envelope 필드 전부(`payload` 컨테이너 포함) + payload 자체 필드** — 로 세면 `COMMAND_RESULT` 6+3, `SESSION_READY` 6+5, `SESSION_OPENED` 12+2, `SESSION_CLOSED` 12+2 = **48**이다.
>
> **총계는 게이트가 아니라 표의 재현성 점검용이다.** 두 사람이 같은 대상을 세어 다른 수가 나왔다는 것 자체가, 규약 없는 총계는 검증이 아니라는 증거다. 그래서 규약을 적고 총계는 참고값으로 내린다.
>
> 현재 대조표는 **46행**이고 빠진 2행은 `SESSION_OPENED.payload`·`SESSION_CLOSED.payload` **컨테이너 행**이다. **지금 채우지 않는다** — 그 두 필드의 필수·널 가능·타입은 payload 내부 행과 fixture 왕복이 이미 덮고, 실행 중인 검증을 멈출 값어치가 없다. 다음 계약 변경 때 표를 48행으로 맞춘다.

### 성능 — 판정 기준과 기록 전용의 구분

**사용자 결정(2026-09-18): 성능 잠정 게이트는 Phase 0 종료를 막지 않는다.** 측정 전에 정한 추정치이므로 미달이어도 실패로 보지 않는다. 대신 **첫 측정치를 p1 회귀 기준선으로 고정 기록**한다. 정확성 게이트(AC-15~AC-19)는 그대로 하드 게이트다.

| 항목 | 성격 | 값 | 근거 |
|------|------|-----|------|
| **tick 초과 비율** = `tick_overrun_total / tick_total`, **`run_tick()` 본문 소요 > 50 ms** (A 단계) | 잠정(비차단) | ≤ 0.5% | 아무 게임 로직도 없는 루프다. 이보다 높으면 지터가 아니라 설계 문제다 |
| 단일 tick **본문** 최대 소요 (A 단계) | 잠정(비차단) | ≤ 250 ms | 5 tick 주기 |
| 왕복 p99 (`PING_SERVER`→`PING_REPLY`, 봇 시계) | 잠정(비차단) | ≤ 150 ms | 3 tick 주기. 루프백에서 3 tick을 넘으면 원인은 물리가 아니라 큐잉이다 |
| `tick_lag_seconds` (루프 주기 드리프트) | **기록만** | — | **이 PC의 바닥값은 전용 OS 스레드 기준 +0.7 %/분**이고 그보다 좋은 값은 나올 수 없다. 측정치와 이 바닥값을 나란히 적어야 "0.7 %"가 문제인지 물리인지 구분된다 |
| 왕복 p50 / tick 본문 소요 p50·p99·max | 기록 | — | 이론 하한은 약 25 ms(평균 반 tick) + 전송 |
| `command_queue_depth` p99/max | 기록 | — | **in-flight 상한이 먼저 걸리므로 낮게 나온다**(30×64=1920 < 4096). "부하가 약해서"가 아니다 |
| 31 연결 수립 소요, 서버 RSS·CPU 피크 | 기록 | — | |
| 마지막 봇 종료 → 모든 행 가시까지 | **하드 게이트**(AC-17b) | ≤ 5초 | 20 Hz tick 단위 배치는 1초 안에 끝나야 한다. 5초는 여유를 주면서 "종료 시 flush"를 배제한다 |

**측정 환경을 리포트에 반드시 적는다**: Unity Editor 실행 여부(전역 타이머 해상도에 영향을 줄 수 있다), 동시에 도는 다른 프로젝트 컨테이너 수. 이것이 없으면 다음 측정과 비교할 수 없다.

### p1 회귀 기준선 (2026-09-19 QA 라운드 1 실측 — 사용자 결정 1에 따라 **고정 기록**)

| 항목 | 잠정 게이트 | **실측** | 여유 |
|------|-----------|---------|------|
| tick 초과 비율(`run_tick` 본문 > 50 ms) | ≤ 0.5 % | **0 / 338,761 = 0.000 %** | 완전 |
| 단일 tick 본문 최대 소요 | ≤ 250 ms | **21.65 ms** | 11.5배 |
| 왕복 p99 | ≤ 150 ms | **약 50.4 ms** | 3배 |
| 왕복 p50 | (기록) | **28.21 ms** | 이론 하한(평균 반 tick 25 ms) + 전송에 근접 |
| `send_queue_depth_max` | (기록) | **141** / 용량 256 | **55 % — 세 큐 중 유일하게 절반을 넘겼다** |
| 서버 RSS | (기록) | 15.3 → **16.6 MB** | |

**잠정 게이트 3줄을 전부 여유 있게 밑돌았다.** 이 수치가 p1의 회귀 비교 기준이며, **다음 슬라이스에서 이 값이 눈에 띄게 나빠지면 그것이 회귀 신호다.** 지금 게이트를 이 실측치 근처로 조이지는 않는다 — p1이 실제 게임 로직을 tick에 넣기 시작하면 tick 본문 소요는 당연히 늘어난다. 조이는 것은 p1의 부하가 무엇인지 안 뒤에 한다.

왕복 p50 28.21 ms가 이론 하한(평균 반 tick = 25 ms)에 근접한 것은 **큐 대기가 사실상 없었다**는 뜻이고, 이는 `SERVER_BUSY`가 구조적으로 도달 불가능하다는 ADR-0006 §5의 산수와 일치한다. 반대로 **`send_queue_depth_max` 141은 주목할 값이다** — 압력이 걸린 곳은 명령 큐가 아니라 **송신 큐**였다. p1에서 브로드캐스트(스냅샷·상태 변경)가 들어오면 여기가 먼저 찬다.

## 8. 비기능 요구

- **동시 연결 31**(봇 30 + Unity 1)이 이번 슬라이스의 상한이다. 그 이상은 측정하지 않는다(원칙 8).
- **결정성**: tick 안에서 실제 시계·난수·`HashMap` 순회 순서에 의존하지 않는다. 이번 슬라이스에 난수는 없다. `starfall-sim`은 IO 의존성이 없어야 하고 이것은 `Cargo.toml`로 검증된다(AC-1). **`rand`를 `[workspace.dependencies]`에 추가하지 않는 것을 규칙으로 유지한다** — axum의 `ws` feature가 tungstenite 마스킹 키용으로 `rand`를 트리에 끌어들이지만(server 실측), `sim`이 `axum`을 의존하지 않으므로 손이 닿지 않는다.
- **손실 창(기록만)**: outbox가 없으므로 프로세스를 **강제 종료(kill)**하면 아직 커밋되지 않은 tick의 이벤트가 사라질 수 있다. 정상 종료 경로의 손실 0은 AC-8(d)가 확인한다. 이것을 없애는 것이 p1의 첫 과제다(ADR-0007 §6).
- **백로그 상한 없음(기록만)**: DB가 영영 복구되지 않으면 미룬 이벤트가 메모리에 무한히 쌓인다(ADR-0007 §4). AC-19는 복구되는 경우만 검증한다.
- **재현성**: 모든 검증 명령은 인자 없이 같은 결과를 내야 한다. 봇 하네스는 **시드를 인자로 받고 로그에 남긴다**.
- **WebGL**: 이번 슬라이스의 클라이언트 코드는 `IRealtimeTransport` 뒤에서 PC 전용이다. WebGL이 막힐 지점과 **지금 이미 막혀 있는 것**(`CollectHttpResponseDetails` 부재)은 ADR-0005 §6에 기록되어 있다. WebGL 빌드를 시도하지 않는다.
- **보안**: 서버는 `127.0.0.1`에만 바인딩한다. 개발용 토큰의 안전성은 그 가정 위에 있다(ADR-0008 §4). `.env`는 커밋하지 않고 `.env.example`의 `STARFALL_DEV_AUTH_SECRET=dev_only_not_a_secret`은 **사용자 결정으로 그대로 둔다**(값 자체가 비밀이 아님을 이름이 선언한다). 어떤 실제 자격 증명도 저장소에 넣지 않는다(I-9).
- **핫 경로 할당**(클라이언트): 수신 처리와 `Update`에서 프레임마다 할당하지 않는다. 버퍼를 재사용하고 LINQ·문자열 결합·박싱을 쓰지 않는다. **이번 슬라이스는 코드 규약 준수만 주장하고 프로파일러 수치는 p1로 미룬다.**
- **측정 기록**: 서버 클린 빌드 시간 — p0-01의 26초와 비교한다(server 실측으로 feature 추가분은 **약 +2.8초**로 예상된다). Unity 콜드·웜 EditMode 시간 — p0-01의 63초·11초와 비교한다.

## 9. 열린 질문

**이번 슬라이스를 차단하는 질문은 없다.** 초안의 Q1~Q5는 2026-09-18 사용자 결정으로 확정됐다.

| # | 항목 | 결정 |
|---|------|------|
| Q1 | 게임 달력 축척 | **`calendar_scale = 60`** (현실 1초 = 게임 1분). 월드에 묶인 불변 상수 → ADR-0006 §3 |
| Q2 | 성능 잠정 게이트가 Phase 0 종료를 막는가 | **막지 않는다.** 첫 측정치를 p1 회귀 기준선으로 고정 기록. 정확성 게이트는 그대로 하드 → §7 |
| Q3 | "30명 동시 접속"의 정의 | **봇 30 + Unity 클라이언트 1 = 31** → AC-15, A 단계 |
| Q4 | `.env.example`의 dev 비밀 공개 | **그대로 둔다** → §8 |
| Q5 | CI 도입 시점 | **다음 슬라이스.** `sqlx-cli`/`.sqlx/` 오프라인 빌드와 함께 묶는다 → ADR-0007 §5 |

**이후 슬라이스에서 결정할 항목** (p0-01에서 이월, 변동 없음):

| # | 항목 | 결정 시점 |
|---|------|----------|
| Q6 | LTS 이전 시점과 대상 버전 | 다음 Unity LTS 공개 또는 아트 에셋 유입 시작 |
| Q7 | 라이선스·기여 약정 법률 검토 | 상업 출시 준비 시작 시 |
| Q8 | 공개가 불리한 데이터 분리 방식 | 그런 데이터가 처음 생길 때 |
| Q9 | Windows Defender 실시간 검사 예외 | 사용자 판단 |
| Q10 | `unityyamlmerge` 설정 절차 | 씬·프리팹을 여러 명이 동시에 고치는 첫 슬라이스 |
| Q11 | 영속화 백로그 상한과 그 뒤의 동작 | outbox가 들어오는 p1 (ADR-0007 §4) |

## 10. 확인하지 못한 사실

검토 과정에서 U-1·U-2·U-3·U-4·U-7이 실측으로 해소됐고, U-5가 셋으로 쪼개졌다.

| # | 사실 | 상태 |
|---|------|------|
| U-1 | `tools/codegen`의 문자열 `enum` 처리 | **해소 — PASS.** 네 필드 모두 C# `string`. 생성기 수정 불필요(분류 주석만 정정) |
| U-2 | 생성기의 envelope 필드 널 가능성 좁힘 반영 | **해소 — FAIL.** `Guid?` + `AllowNull`로 나오고 반례가 통과한다. **T7에서 생성기를 고친다**(ADR-0005 §4-3) |
| U-3 | axum `ws` feature의 의존 트리·빌드 시간 | **해소.** 크레이트 8개 추가(153→161), 클린 빌드 **+2.8초(+8 %)**. 부수 관찰: `rand`가 트리에 들어오나 `sim`은 닿지 않는다 |
| U-4 | sqlx 0.8.6의 `migrate!` feature 조합 | **해소.** `runtime-tokio, postgres, macros, migrate, uuid, json`. `macros` 없으면 컴파일 실패. **`chrono`·`time` 불필요** |
| U-5a | `SetRequestHeader("Authorization", …)`가 Mono 런타임에서 실제로 헤더를 보내는가 | **미확인.** API 존재는 확인. client T8 첫 작업. 안 되면 ADR-0005 §6(a) 티켓 경로를 앞당긴다 |
| U-5b | 도메인 리로드·PlayMode 전환 시 소켓 거동과 정상 Close 가능 여부 | **미확인.** client T8. AC-13(c)의 `CLIENT_CLOSED`가 여기 달려 있다 |
| U-5c | 업그레이드 실패의 HTTP 상태 코드 접근 | **해소 — 불가.** `CollectHttpResponseDetails`가 netstandard2.1·Mono 양쪽에 없다 → ADR-0005 §6 |
| U-6 | 30 봇 + Docker + Unity Editor 동시 실행의 자원 여유 | **미확인.** qa. **Unity Editor 실행 여부가 타이머 측정에 영향을 줄 수 있으므로 리포트에 반드시 기록** |
| U-7 | `unity test --filter` 문법 | **해소.** **정규식**(부분 일치, 앵커 없음). glob은 `ArgumentException` → CLI 종료 코드 6, 리포트 없음 |
| U-8 | `k6` 미설치 | 사실 기록. 이번 슬라이스는 Rust 봇만 쓴다 |
| U-9 | tungstenite가 받은 Ping에 자동 Pong을 보내는가 | **미확인.** 서버가 보내는 Ping과 받는 Pong만 우리 로직이라 판정에 영향 없음. server T4에서 실동작 확인 |
| U-10 | 수신 경로의 프레임당 할당량 | **미확인(의도).** 이번 슬라이스는 규약 준수만 주장, 수치는 p1 |

**실행으로 확인한 환경 사실** (리포트가 근거로 쓸 것): 포트 8080 비어 있음 / `starfall_postgres-data` 볼륨 존재 / append-only 트리거가 PostgreSQL 18.6에서 동작(UPDATE·DELETE 차단, TRUNCATE 통과) / `occurred_at` 공식이 fixture 3건과 문자열 일치 / `unity` CLI 종료 코드 = 성공 0, 테스트 실패 8, 런 에러 6, 인자 오류 2 / **`unity test --help`는 `--report-format both`를 안내하지만 CLI가 거부한다**(종료 코드 2). 유효한 형태는 `nunit,junit`이다.

## 변경 기록

| 날짜 | 변경 | 이유 |
|------|------|------|
| 2026-09-18 | 최초 작성 (draft). 계약 4타입 신설, ADR-0005~0008, 수용 기준 20개 | p0-02-networking-spike 스펙·계약 초안 |
| 2026-09-19 | **QA 라운드 1 반영.** AC-9(c)의 1:1 불변식에 **측정 창 델타 / 정상 종료 세션** 한정 추가(누적으로는 닫힌 뒤 도착한 명령 때문에 성립하지 않는다 — 결함이 아니라 문구가 조건을 빠뜨린 것), §7에 **p1 회귀 기준선 표** 고정 기록, ADR-0006 §5에 큐 실측 | QA 평가 PASS 73 / 부분 PASS 1 / FAIL 0. 잠정 게이트 3줄 전부 여유 있게 통과 |
| 2026-09-19 | **구현 실측 정정 3건.** AC-19(c)의 `persist_backlog` 판정을 "0 도달"→"정상 대역(0~20) 복귀"(하트비트 커밋 때문에 정상 동작에서도 0이 아니다 — 옛 문구면 폴링이 끝나지 않는다), AC-21의 총계 46→48 + 열거 규약 명시 + 판정 근거를 필드별 표로 이동, T12의 봇 path 의존 지시 철회(I-25 위반이었다) | server·client 구현과 QA 도구 정렬에서 드러난 스펙 오류. 상세는 `01_architect_decisions.md` §7 |
| 2026-09-18 | **개정 (agreed).** fixture 건수 16→12 정정(+반례 15→16), tick 재개 규칙과 AC-3 신설(I-17 월드 단위), tick 초과 정의를 `run_tick` 본문 소요로 못박음, 항진명제 2건 교체(I-25 신설), 좁힘 처리를 생성기 수정으로 결정, 제어 경로 분리(I-16), `correlation_id` 채움 규칙 확정, close code 1009 제거, `/ws` 503 `reason` 분기, stdin 종료 경로 승인, 세션 집합을 correlation 대조로, AC-19(기록 내구성) 신설, 성능 게이트 비차단화, 열린 질문 Q1~Q5 확정 | server·client 검토(B-1~B-19, R1~R9)와 사용자 결정 5건 반영. 상세는 `_workspace/p0-02-networking-spike/01_architect_decisions.md` |

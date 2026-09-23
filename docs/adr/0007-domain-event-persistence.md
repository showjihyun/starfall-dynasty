# 0007. 도메인 이벤트 영속화 — 첫 마이그레이션, 기록 범위, append-only

- 상태: **accepted** (server 검토 반영, 2026-09-18. history·qa 검토는 p2·QA 단계에서)
- 날짜: 2026-09-18
- 슬라이스: p0-02-networking-spike
- 검토: `_workspace/p0-02-networking-spike/01_server_spec_review.md`(B-1·B-7·B-9·B-18·B-19, U-4). 반영 내역은 `01_architect_decisions.md`
- 근거 기획안 절: TECH §14 (PostgreSQL), TECH §15–16 (이벤트 소싱 비판·무엇을 저장할 것인가), TECH §19 (Transactional Outbox), HSE §7–8 (DomainEvent), HSE §28–30 (Event Bus·멱등성·순서), HSE §69–70 (저장 전략·최소 스키마), HSE §101 (Envelope)
- 관련: ADR-0003(로컬 인프라·sqlx 핀), ADR-0006(tick·게임 시간), CLAUDE.md 원칙 3·4·5·9

## 맥락

Phase 0 종료 기준의 절반이 "**실시간 이벤트 기록**"이다. 이 슬라이스가 프로젝트의 첫 테이블을 만든다. 첫 테이블은 나중에 만드는 어떤 테이블보다 비싸다: 이후 모든 역사·증거·프로젝션이 여기에 붙고, 이미 들어간 행은 원칙 5에 따라 고칠 수 없다.

정할 것은 네 가지다. (1) 어떤 이벤트를 기록하고 어떤 것을 기록하지 않는가, (2) 테이블 모양, (3) 언제·어떻게 쓰는가, (4) 지금 안 하는 것(outbox)과 그것을 언제 해야 하는가.

## 결정

### 1. 기록 범위 — 도메인 이벤트만, 전송 로그는 아니다

원칙 4는 "모든 이벤트가 역사적 사건은 아니다"이다. 그 앞에 한 단계가 더 있다: **모든 일이 도메인 이벤트인 것도 아니다.** 이 슬라이스의 기록 규칙을 표로 고정한다.

| 발생한 일 | `domain_events`에 기록 | 이유 |
|----------|:---:|------|
| `SESSION_OPENED` | **기록** | 세계의 사실이다. 인증된 행위자가 tick T부터 존재했다. 존재 구간의 시작 |
| `SESSION_CLOSED` | **기록** | 존재 구간의 끝. `close_reason`이 "떠났다"와 "잘렸다"를 구분한다 |
| `SHIP_SPAWNED` | **기록** | (p1-01 추가) 함선 엔티티가 **어디에** 생겼다. 행위자의 존재(`SESSION_OPENED`)와 다른 사실이다 — 캐릭터 ≠ 함선(GDD §4). 스폰 위치는 다른 어떤 기록으로도 복원되지 않는다 |
| `SHIP_DESPAWNED` | **기록** | (p1-01 추가) 함선 엔티티가 사라졌다, 그 이유와 **마지막 위치**와 함께. "그 함선이 떠날 때 어디 있었나"는 사건 재구성이 가장 먼저 묻는 질문이고, 메모리 월드가 함선을 버리는 순간 다른 답이 없다 |
| **함선 이동(적분)** | 기록 안 함 | (p1-01 추가) **이 표에서 가장 중요한 한 줄이다.** 위치가 바뀌는 것은 초당 20번 일어나는 상태 전이이지 사건이 아니다. 기록하면 `domain_events`는 하루에 수천만 행의 좌표 로그가 되고, 역사 판정기가 읽을 것은 "무슨 일이 일어났는가"가 아니라 좌표 시계열이 된다 — GDD §42가 경고한 "재미없는 DB"의 가장 빠른 길이다. 실시간 상태의 권위는 메모리 `WorldState`이고(HSE §95), 관측 경로는 `WORLD_SNAPSHOT`이다(I-31) |
| `SET_SHIP_CONTROL` 수신 | 기록 안 함 | 명령이다. 아래 원칙 그대로 |
| `PING_SERVER` 수신 | 기록 안 함 | 진단용 프로브다. 세계에서 아무 일도 일어나지 않았다 |
| `PING_REPLY`·`COMMAND_RESULT` 송신 | 기록 안 함 | 전송 계층의 응답이다. 사실이 아니라 사실에 대한 통지 |
| 거부된 명령 | 기록 안 함 | 세계가 바뀌지 않았다. `commands_rejected_total{reason_code}` 메트릭과 로그로 남는다 |
| 인증 실패 | 기록 안 함 | 보안 이벤트이지 세계의 사실이 아니다. 계정이 생기면 별도 저장소로 간다 |
| tick 실행 | 기록 안 함 | 메트릭이다. tick은 시간 축이지 사건이 아니다 |

**명령을 기록하지 않는 것이 이 표의 핵심이다.** 명령을 기록하기 시작하면 `domain_events`는 요청 로그가 되고, 역사 판정기는 "무슨 일이 일어났는가"가 아니라 "누가 무엇을 요청했는가"를 읽게 된다. 기획안이 경고한 "재미없는 DB"(GDD §42)의 시작이 정확히 이 지점이다.

부작용: 이 슬라이스는 **초당 이벤트 수가 낮다**(세션당 2건). 그래서 "실시간 기록"의 부하는 ping을 늘려서가 아니라 **세션 회전(churn)**으로 만든다(스펙 §7). 기록할 것을 억지로 늘리지 않는다.

> **p1-01 개정 (2026-09-19).** 표에 `SHIP_SPAWNED`·`SHIP_DESPAWNED`·함선 이동·`SET_SHIP_CONTROL` 네 줄을 추가했다. **QA가 반드시 알아야 할 두 가지**: (1) 함선은 세션보다 오래 산다(30초 잔류·재개 — ADR-0011 §6). 따라서 `SHIP_*`는 **세션당이 아니라 함선당 1쌍**이고, 재접속으로 이어 탄 함선은 세션이 2개여도 스폰이 1건이다. **짝짓기는 `correlation_id`가 아니라 `ship_id`로 한다** — p0-02의 correlation 기반 대조 SQL을 그대로 쓰면 맞지 않는다. (2) 새 두 타입은 `causation_id`가 **비-null인 프로젝트 첫 사례**이며(I-30), `SHIP_DESPAWNED`의 원인은 여러 tick 전의 `SESSION_CLOSED`다 — 상관과 인과가 같은 행에서 서로 다른 시각을 가리키는 첫 데이터다(HSE §102). 상세는 `docs/specs/p1-01-ship-movement.md` §6. I-21에 따라 이 표를 고치는 것이 새 이벤트 타입을 만드는 유일한 경로이므로, 여기 적히지 않은 것은 기록되지 않는다.

### 2. 마이그레이션 0001 — `worlds`와 `domain_events`

`server/migrations/` 아래 파일 하나. `historical-engine` 스킬 `references/data-model.md`의 `domain_events`를 이 슬라이스 범위로 줄이되, **계약(`event-envelope.schema.json`)과 열 대 열로 대응**시킨다.

```sql
CREATE TABLE worlds (
    world_id       UUID PRIMARY KEY,
    name           TEXT NOT NULL,
    tick_hz        INT  NOT NULL CHECK (tick_hz BETWEEN 1 AND 1000),
    calendar_epoch TEXT NOT NULL,          -- GameTime 형식
    calendar_scale INT  NOT NULL CHECK (calendar_scale > 0),
    sim_version    INT  NOT NULL,
    -- 이 월드가 도달한 마지막 tick. 재기동 시 여기서 이어 간다 (ADR-0006 section 2.3).
    -- 이벤트를 쓰는 트랜잭션 안에서 함께 갱신되므로 domain_events 와 구조적으로 어긋나지 않는다.
    last_tick      BIGINT CHECK (last_tick BETWEEN 0 AND 9007199254740991),
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE domain_events (
    event_id       UUID PRIMARY KEY,
    event_type     TEXT   NOT NULL,
    schema_version INT    NOT NULL CHECK (schema_version >= 1),
    world_id       UUID   NOT NULL REFERENCES worlds (world_id),
    tick           BIGINT NOT NULL CHECK (tick BETWEEN 0 AND 9007199254740991),
    sequence       BIGINT NOT NULL CHECK (sequence BETWEEN 0 AND 9007199254740991),
    occurred_at    TEXT   NOT NULL,        -- 게임 시간 (GameTime)
    recorded_at    TIMESTAMPTZ NOT NULL,   -- 실제 시간, 감사용
    correlation_id UUID   NOT NULL,
    causation_id   UUID,
    actor_id       UUID,
    payload        JSONB  NOT NULL,
    UNIQUE (world_id, tick, sequence)
);
CREATE INDEX domain_events_type_idx        ON domain_events (event_type, world_id, tick);
CREATE INDEX domain_events_correlation_idx ON domain_events (correlation_id);
```

`UNIQUE (world_id, tick, sequence)`가 만드는 인덱스가 순서 조회를 이미 덮으므로 같은 열 조합의 인덱스를 또 만들지 않는다.

참조안과 다르게 한 것과 이유:

| 항목 | 참조안 | 여기 | 이유 |
|------|-------|------|------|
| `world_id` | `TEXT` | `UUID` + FK | 계약이 `UuidV7`이다. 타입이 갈라지면 경계면 교차 검증이 "이름은 다르지만 같은 것"을 매번 판단해야 한다 |
| `sequence` | `INT` | `BIGINT` + CHECK | 계약의 `Sequence`는 0..2^53−1이다. 조용한 축소를 만들지 않는다. 4바이트가 아깝지 않다 |
| `tick` | 제약 없음 | CHECK 상한 | 계약의 `SafeInteger` 상한을 DB에서도 강제한다. 여기가 마지막 방어선이다 |
| `worlds` | 없음 | 추가 | `world_id`가 어디선가 온 상수가 아니라 **행**이어야 tick 주기·달력 상수가 월드에 묶인다(ADR-0006 §3) |
| `last_tick` | 없음 | 추가 | tick 재개의 정본(ADR-0006 §2.3). 없으면 재기동마다 tick이 0으로 돌아가 이벤트 배치가 통째로 유실된다 |

마이그레이션은 **스파이크 월드 1행을 시드**한다: `world_id = 01a0b1c2-3d4e-7f01-8a2b-9c0d1e2f3a4b`, `tick_hz = 20`, `calendar_epoch = 3800-01-01T00:00:00Z`, `calendar_scale = 60`, `sim_version = 1`, `last_tick = NULL`. 값이 고정이라 QA가 기대값으로 쓸 수 있고, 계약 fixture의 `world_id`와 같다.

**서버는 기동 시 자기 설정과 `worlds` 행을 대조하고 다르면 기동을 거부한다.** 이 검사가 "환경 변수 하나로 저장된 이벤트의 시간 의미가 바뀌는" 경로를 막는다.

**기존 볼륨과 마이그레이션 체크섬** (server 실측, B-19): 이 PC에는 p0-01이 만든 `starfall_postgres-data` 볼륨이 **이미 있다**. AC-2의 "볼륨 없는 상태"를 만들려면 레포 루트에서 `docker compose down -v`가 필요하고, `docker-compose.yml`의 `name: starfall` 덕분에 **이 명령은 starfall 프로젝트에만 작용한다**(다른 프로젝트 컨테이너 5개와 볼륨 90여 개는 건드리지 않는다). 그리고 `sqlx::migrate!`는 적용된 마이그레이션의 체크섬을 `_sqlx_migrations`에 저장하므로, **개발 중 `0001_*.sql`을 한 글자라도 고치고 다시 기동하면 기동이 실패한다**("previously applied but has been modified"). 고친 뒤에는 반드시 `docker compose down -v && docker compose up -d`.

### 3. append-only를 지금 건다

`persistence-patterns.md` §6의 `forbid_mutation()` 트리거 함수를 그대로 쓰고, `domain_events`에 `BEFORE UPDATE OR DELETE ... FOR EACH ROW`로 건다.

나중이 아니라 지금 거는 이유: 비용이 여섯 줄이고, 막아야 할 것(원칙 5)이 절대 원칙이며, **코드가 가장 새로울 때가 실수로 UPDATE를 부를 위험이 가장 큰 때**다. 나중에 걸면 "그동안 아무도 안 고쳤겠지"를 증명할 수 없다.

알려진 구멍 두 가지를 기록한다.

- `TRUNCATE`는 이 트리거로 막히지 않는다(행 단위 트리거다). 개발 중 테이블 비우기는 가능하고, 그것은 의도다 — 스파이크 DB는 버릴 수 있어야 한다. 운영 DB의 TRUNCATE 차단은 권한 설계 문제이며 배포 슬라이스에서 다룬다.
- 슈퍼유저는 트리거를 DROP할 수 있다. 트리거는 실수를 막는 장치이지 악의를 막는 장치가 아니다.

운영자 정정은 새 레코드(`ADMIN_EVENT_CORRECTION`, HSE §68)로 남긴다 — 이 슬라이스에서는 만들지 않는다.

### 4. 쓰기 — tick 단위 1 트랜잭션, 멱등 삽입

- tick 루프는 `TickOutcome{tick, events}`를 **bounded 채널**로 영속화 태스크에 넘긴다. tick 루프는 DB를 기다리지 않는다(기다리면 DB 지연이 곧 시뮬레이션 지연이 된다).
- 영속화 태스크는 **한 tick의 이벤트 전부 + `worlds.last_tick` 갱신을 한 트랜잭션**으로 쓴다. 부분 기록된 tick이 생기지 않고, tick 위치와 이벤트가 구조적으로 어긋나지 않는다.
- 삽입은 `ON CONFLICT (event_id) DO NOTHING`. 같은 배치를 재시도해도 두 번 들어가지 않는다.
- **`ON CONFLICT (event_id)`는 `UNIQUE (world_id, tick, sequence)` 위반을 잡지 않는다**(server 실측, PostgreSQL 18.6). 그 위반은 에러를 내고 **트랜잭션 전체를 중단시켜 같은 tick의 멀쩡한 이벤트까지 없앤다.** 이것은 고쳐야 할 결함이 아니라 **그대로 두어야 하는 신호**다 — 서로 다른 이벤트가 같은 `(world_id, tick, sequence)`를 주장하는 것은 시뮬레이션 버그이고, 조용히 통과시키면 `sequence` 무결성(I-18)이 무너진다. ADR-0006 §2.3의 tick 재개가 **"재기동 때마다 나는 에러"를 제거해서** 이 신호를 다시 유효하게 만든다. 삼키지 말고 에러 로그 + `domain_events_persist_failed_total`로 드러낸다.
- `recorded_at`은 **영속화 단계에서** 채운다. tick 루프 안에서 시계를 읽지 않는다(ADR-0006 §2). **SQL의 `now()`가 아니라 Rust 호스트 시계로 만든다** — 컨테이너 시계와 호스트 시계가 어긋나면(WSL2 절전 복귀 등) AC-16(b)의 5초 측정에 그 차이가 섞인다. 포맷은 `occurred_at` 때문에 어차피 필요한 civil-from-days 헬퍼를 재사용하므로 `chrono` 의존이 생기지 않는다.
- `sequence`는 tick 안에서 **발행 순서대로 0부터** 부여한다. 발행 순서는 제출 순번(ADR-0006 §4)에서 결정적으로 나온다.
- 영속화 채널이 가득 차면(DB가 느리거나 죽었으면): tick 루프는 **버리지 않고** 다음 tick의 영속화 제출을 미루며 `persist_backlog` 메트릭을 올린다. 백로그가 임계(이번 슬라이스 잠정값 600 tick = 30초)를 넘으면 **새 연결 수락을 중단**하고(업그레이드 전 503) 기존 세션은 유지한다. 이벤트를 조용히 버리는 것보다 접속을 거절하는 편이 낫다.
  - **백로그에는 상한이 없다.** DB가 영영 돌아오지 않으면 미룬 이벤트가 메모리에 무한히 쌓인다. B 단계 회전 부하(초당 수십 건, 이벤트당 수백 바이트) 기준으로 몇 시간은 버티지만, **"버리지 않는다"는 곧 "언젠가 OOM"이다.** 이번 슬라이스는 그 한계를 감수하고 `persist_backlog`로 관측만 한다. 상한과 그 뒤의 동작(디스크 스풀? 종료?)은 outbox가 들어오는 p1에서 함께 정한다.
  - **이 정책에는 검증 AC가 있어야 한다**(B-18). 없으면 매직 넘버 600이 들어간 죽은 코드가 된다. 검증은 5분이면 된다: A 단계 도중 `docker compose stop postgres` → 30초 후 새 연결이 503, 기존 세션은 유지 → `docker compose start postgres` → 백로그가 빠지고 **한 건도 잃지 않고** 전부 DB에 들어온다. 이것이 "기록 시스템이 기록을 버리지 않는다"의 유일한 실증이고, Phase 0 종료 기준의 "실시간 이벤트 기록"에 정확히 해당한다(스펙 AC-19).
  - `/ws`의 503은 **원인을 구분한다**: `{"status":"unavailable","reason":"auth_not_configured"}`(ADR-0008 §3) / `{"status":"unavailable","reason":"recording_backlog"}`. 같은 본문을 쓰면 인증 미설정과 기록 백로그를 구분할 수 없다.

### 5. sqlx 사용 범위 — 이번 슬라이스는 런타임 검사 쿼리

- 마이그레이션은 `sqlx::migrate!`로 **바이너리에 embed**하고 기동 시 적용한다. 별도 CLI 없이 `cargo run`만으로 스키마가 선다.
- **feature 조합은 server가 컴파일로 확정했다**(U-4): `runtime-tokio, postgres, macros, migrate, uuid, json`. `macros` 없이 `migrate`만 켜면 ``cannot find `migrate` in `sqlx` ``로 **컴파일이 실패한다**. `uuid`가 `Uuid` 바인딩을, `json`이 JSONB 바인딩을 준다. **`chrono`·`time`은 필요 없다** — `recorded_at`을 Rust에서 만들기로 했으므로(§4) 시간 타입을 주고받을 일이 없고, 그만큼 크레이트가 트리에서 빠진다.
- **`query!`/`query_as!`(컴파일 타임 검사)를 이번 슬라이스에서는 쓰지 않는다.** 이유: 컴파일 타임 검사는 빌드 시 DB 접속이나 `.sqlx/` 오프라인 메타데이터를 요구하고, 그 메타데이터는 `sqlx-cli`로 만드는데 **이 PC에 `sqlx-cli`가 설치되어 있지 않다**(2026-09-18 실측: `sqlx: command not found`). 이번 슬라이스의 쿼리 표면은 INSERT 1종과 SELECT 2~3종이라 컴파일 타임 검사의 이득보다 도구 설치·`.sqlx/` 동기화 규율의 비용이 크다.
  - 대신 런타임 검사 `sqlx::query`/`query_as`에 **명시적 바인딩 타입**을 쓰고, 마이그레이션이 적용된 DB에서 도는 통합 테스트로 쿼리를 검증한다.
  - **재평가 시점은 쿼리가 10개를 넘거나 CI가 생기는 슬라이스**다. 그때 `cargo install sqlx-cli` + `cargo sqlx prepare` + `.sqlx/` 커밋을 한 번에 도입한다. `.gitignore`는 이미 `server/.sqlx/`를 추적하도록 준비돼 있다(ADR-0003, p0-01 AC-11).
- 마이그레이션 `.sql` 파일은 **줄바꿈이 LF여야** 해시가 플랫폼 간 같다(sqlx 공식 문서의 경고). `.gitattributes`에 `*.sql text eol=lf`가 이미 있다(확인함).
- **마이그레이션 파일을 `추가`하면 재컴파일되지 않는다**(server 실측으로 범위를 좁혔다): 기존 `.sql` **수정은** 재컴파일된다(`include_str!` 경로가 걸려 있다), 새 `.sql` **추가는** 되지 않는다. 그래서 "마이그레이션을 썼는데 테이블이 없다"가 된다. `build.rs`에 `cargo:rerun-if-changed=migrations`를 두면 둘 다 해결되고 불필요한 재빌드도 일으키지 않음을 확인했다.

### 6. Outbox를 지금 만들지 않는다 (그리고 언제 만들어야 하는가)

Transactional Outbox(TECH §19)는 이번 슬라이스 범위 밖이다(`00_request.md`). 그 결과 다음이 성립하지 **않는다**:

> 이벤트를 발행한 tick과 그 이벤트가 DB에 커밋된 시점 사이에 프로세스가 죽으면 **그 이벤트는 사라진다.**

지금 받아들이는 이유: 이 슬라이스에는 **이벤트와 짝이 맞아야 할 상태 변경이 없다.** 잃어버린 `SESSION_OPENED`는 "기록이 없다"일 뿐, "돈은 줄었는데 기록이 없다"가 아니다.

**이것이 더 이상 참이 아니게 되는 순간이 outbox의 마감 기한이다**: 상태 변경(잔액·인벤토리·소유권)과 도메인 이벤트가 **같은 트랜잭션**이어야 하는 첫 명령이 생기는 슬라이스(p1의 채굴·거래). 그 슬라이스는 outbox 없이 시작할 수 없다. 여기에 적어 두는 이유는, 스파이크가 "이벤트는 그냥 비동기로 쓰면 된다"는 습관을 남기고 끝나는 것이 가장 흔한 실패이기 때문이다.

부하 측정은 이 손실 창을 **관측한다**: 정상 종료 경로에서 손실 0건을 확인하고, 강제 종료(kill) 시 손실이 발생할 수 있음은 스펙 §8에 기록만 한다.

### 7. Redis는 쓰지 않는다

세션 레지스트리는 프로세스 메모리에 둔다. Redis를 세션 저장소로 쓰면 "서버 재시작 후에도 세션이 산다"는 착각이 생기는데, WebSocket 연결 자체가 프로세스에 묶여 있어 사실이 아니다. `/readyz`가 Redis를 점검하는 것은 유지한다(ADR-0003). 원칙 3.

### 8. 운영 엔드포인트 `/debug/stats` (계약 아님)

`GET /debug/stats` → JSON. 카운터·게이지와 tick 본문 소요 분포를 담는다. 새 의존성 없이 `AtomicU64`로 구현한다(Prometheus 익스포터를 지금 들이지 않는다 — 스크레이퍼가 없고, `metrics` 크레이트는 저장소가 없어 `metrics-exporter-prometheus`와 그 hyper 서버 한 벌을 함께 사야 한다). `/healthz`·`/readyz`와 같은 성격의 **운영 표면이고 계약 타입이 아니다.** Unity 클라이언트가 소비하기 시작하면 `contracts/api/`로 승격한다.

**분포는 링 버퍼가 아니라 고정 버킷 히스토그램으로 낸다**(server 지적). 링 버퍼로 분위수를 내면 **버퍼 길이 = 관측 창**이 되어, A 단계가 1200 tick인데 버퍼가 1024면 "A 단계 p99"라고 적은 값이 실제로는 마지막 51초의 p99가 된다. 마이크로초 로그 버킷 32개 + `max` + `count` + `sum`이면 실행 **전체**를 덮고 256바이트면 된다. p1 회귀 기준선으로 쓰려면 창이 고정이어야 한다.

QA가 쓰는 교차 검증은 **출처가 독립일 때만 의미가 있다**: `ws_connections`를 **세션 레지스트리의 실제 길이**로 계산해야, `ws_connections == sessions_opened_total − sessions_closed_total`이 "레지스트리에서 지워졌는데 closed 카운터가 안 올랐다" 같은 진짜 버그를 잡는다. 세 값이 같은 증감 지점에서 나오면 그 등식은 코드가 어떻게 틀려도 참이다. 그 위에 **DB의 행 수**가 세 번째 출처로 겹친다.

### 9. `/readyz` 첫 호출 503 (p0-01 이월 항목)

p0-01에서 컨테이너 재생성 직후 첫 `/readyz`가 Redis broken pipe로 1회 503을 냈다(자동 복구). 원인은 풀에 남은 끊긴 연결이다.

결정: **각 점검은 "즉시 실패"에만 1회 재시도한다. 타임아웃에는 재시도하지 않는다.**

- 재시도 대상은 연결 오류(broken pipe, connection reset)처럼 **마이크로초 단위로 끝나는 실패**다. p0-01이 겪은 실제 실패가 정확히 그것이고, 재시도 비용이 사실상 0이다.
- 첫 시도가 **타임아웃이면 재시도하지 않고** 즉시 `unavailable`을 낸다.
- 이 조건이 필요한 이유(B-7): 초안의 "총 예산 2초에 재시도 포함"은 ADR-0003 §3.1의 "점검마다 2초 타임아웃"(현재 코드 `CHECK_TIMEOUT = 2s`)과 충돌했다. 둘 다 만족하려면 점검당 1초가 되고, 그러면 **살아 있지만 느린 DB(1.5초)가 지금은 통과하다가 앞으로는 실패**하게 되어 동작이 조용히 바뀐다. "타임아웃에는 재시도 없음"이면 점검당 2초가 그대로 유지되고 총 소요도 사실상 2초를 넘지 않는다.
- 무한 재시도나 긴 백오프를 넣지 않는다 — readiness가 매달리면 그 자체가 장애다(ADR-0003 §3.1 제약 2).

## 검토한 대안과 버린 이유

- **전면 이벤트 소싱(상태를 이벤트에서 재구성)**: 원칙 7과 TECH §15가 명시적으로 반대한다. 상태는 상태 테이블에, 이벤트는 기록으로.
- **ping마다 도메인 이벤트 기록**: `domain_events`를 요청 로그로 만든다. 부하 수치는 예쁘게 나오지만 증명하는 것이 없다.
- **`historical_events` 테이블을 지금 만들기**: 중요도 판정·`rule_version`·가시성이 history-engine-engineer 검토 대상이고 p2 범위다. 빈 테이블은 아무것도 증명하지 않는다.
- **`sequence`를 `INT`로**: 계약과 타입이 갈라진다. 4바이트를 아끼려고 경계면 검증에 예외를 하나 만드는 거래는 나쁘다.
- **`world_id`를 설정 상수로(테이블 없이)**: tick 주기·달력 상수를 붙일 자리가 없어져, 그것들이 환경 변수로 흩어진다.
- **append-only 트리거를 나중에**: 코드가 가장 새로울 때가 가장 위험한 때다. 비용이 여섯 줄이다.
- **`query!` 컴파일 타임 검사를 지금**: `sqlx-cli` 미설치 + 쿼리 3종. 도입 비용이 이득보다 크다. 기한을 §5에 못박았다.
- **outbox를 지금**: 짝이 맞아야 할 상태 변경이 없어 검증할 대상이 없다. 대신 마감 기한(§6)을 명시했다.
- **이벤트를 tick 루프에서 동기 커밋**: DB 지연이 시뮬레이션 지연이 된다.
- **영속화 채널 포화 시 이벤트 드롭**: 기록 시스템이 기록을 버리면 그 시스템은 존재 이유가 없다. 접속 거절이 낫다.

## 결과

- 좋은 점: 첫 테이블이 계약과 열 단위로 대응해 경계면 검증이 기계적이다. 월드 상수가 행으로 존재해 게임 시간의 의미가 고정된다. append-only가 처음부터 켜져 있다. 기록·미기록 표가 있어 "일단 다 저장하자"는 표류를 막는다.
- 감수할 점: outbox가 없어 프로세스 강제 종료 시 최근 이벤트가 사라질 수 있다(§6에 기한 명시). 컴파일 타임 쿼리 검사가 없어 SQL 오타가 런타임에 드러난다(통합 테스트로 덮는다). 영속화 백로그 임계(600 tick)는 근거 없는 잠정값이며 첫 측정으로 조정한다.
- 다시 검토할 조건: (a) 상태 변경을 동반하는 첫 명령 → **outbox 필수**, (b) 쿼리 10개 초과 또는 CI 도입 → `sqlx-cli` + `.sqlx/`, (c) `domain_events`가 수천만 행이 되거나 쓰기가 병목으로 측정될 때 → 파티셔닝(HSE §72), (d) history 크레이트가 생길 때 → 레지스트리 소비자 태그 추가 + 읽기 경로 설계.

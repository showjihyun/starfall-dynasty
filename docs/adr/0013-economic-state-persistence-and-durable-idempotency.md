# 0013. 경제 상태의 영속화와 지속 멱등성 — tick 배치가 곧 outbox 다

- 상태: **accepted** (2026-09-27 — 이 ADR 이 기대던 가치 판단 둘을 사용자가 결정했다: §5 월드 정지(스펙 Q1), §6 ≤ 1 초 손실 감수(Q2). **server 검토 K1~K6 + 추가 1·2 반영 개정 2026-09-27** — `_workspace/p1-02-mining/02_server_ack.md` §1)
- 날짜: 2026-09-27
- 슬라이스: p1-02-mining
- 근거 기획안 절: TECH §14(PostgreSQL), TECH §19(Transactional Outbox), TECH §26–27(Security·Economy — 화폐 복사가 최악의 버그), HSE §28–29(Event Bus·멱등성), GDD §33(서버 판정 8단계)
- 관련: ADR-0006 §6(세션 내 중복 제거 — "지속 멱등성은 상태를 바꾸는 첫 명령의 필수 항목"), ADR-0007 §4·§6(tick 단위 트랜잭션, outbox 마감 기한), ADR-0011 §7(함선 상태 비영속), CLAUDE.md 원칙 1·3·5·9

## 맥락

p1-02 는 **상태를 바꾸는 첫 명령**(`MINE_RESOURCE`)을 만든다. 두 개의 ADR 이 이 순간을 마감 기한으로 적어 두었다.

- ADR-0007 §6: *"상태 변경(잔액·인벤토리·소유권)과 도메인 이벤트가 같은 트랜잭션이어야 하는 첫 명령이 생기는 슬라이스는 outbox 없이 시작할 수 없다."*
- ADR-0006 §6: *"지속 멱등성(`processed_commands` + 명령·상태 변경 동일 트랜잭션)은 상태를 바꾸는 첫 명령이 생기는 슬라이스의 필수 항목."*

지금 서버의 모양(p1-01 기준, 코드로 확인):

1. 월드 상태의 권위는 `starfall-sim` 의 **메모리**다. tick 루프는 DB 를 기다리지 않는다.
2. tick 마다 `TickOutcome.events` 가 bounded 채널로 영속화 태스크에 가고, 영속화 태스크가 **한 tick = 한 트랜잭션**으로 `domain_events` + `worlds.last_tick` 을 쓴다.
3. 영속화 실패 처리: 연결 오류는 같은 배치를 무한 재시도, **제약 위반(SQLSTATE 23xxx)은 그 배치만 포기하고 다음으로 간다**(`persistence/src/lib.rs` `commit_with_retry`).
4. `recorded_at` 을 못 만들면 그 이벤트를 **`continue` 로 건너뛴다**(`commit`).
5. 중복 명령 제거는 **세션 안에서만** 최근 1024개(`session.remember`).

3·4·5 는 이동만 있을 때는 "기록 한 줄이 빠진다"였다. 인벤토리가 들어오면 **"수량은 늘었는데 기록이 없다"** 또는 **"같은 명령이 두 번 수량을 늘렸다"**가 된다. 둘 다 원칙 1 과 경제의 최악의 버그(TECH §27)다.

## 결정

### 1. 인벤토리의 정본은 PostgreSQL 이다. 메모리는 앞서 달리는 사본이다

- 인벤토리와 매장지 상태(잔여량, 드러남)는 **PostgreSQL 상태 테이블**에 산다. 원칙 3 의 "영속 데이터는 PostgreSQL" 이다.
  - **매장지 회복은 게으르게 계산한다.** designer 규칙(월드 tick 이 `regen_interval_ticks` 의 배수일 때마다 `regen_kg`, 초기 매장량이 상한)은 저장된 `(remaining_kg, as_of_tick)` 과 현재 tick 만으로 닫힌 식이 된다: `effective(t) = min(initial, remaining + regen_kg × (⌊t / I⌋ − ⌊as_of / I⌋))`. 그래서 **회복은 DB 쓰기를 만들지 않고**, 도메인 이벤트도 만들지 않는다(이동처럼 결정적 규칙이지 사건이 아니다). 행은 채굴이 있을 때만 `(remaining_after, 그 tick)` 으로 쓰인다. 이벤트의 `deposit_remaining_before_kg` 는 **회복을 적용한 뒤의** 값이다. **회복식은 sim 에만 있다** — 영속화는 그 식을 모른다(§5, K3).
  - **드러남(`first_extracted_tick`)** 은 매장지 행의 한 열이다. 첫 채굴 tick 에 한 번 쓰이고 바뀌지 않는다. 잔여량에서 유도하지 않는다(회복이 잔여를 초기값으로 되돌려도 드러남은 유지된다).
- **쿨다운은 영속하지 않는다.** actor 별 마지막 채굴 tick 은 메모리 상태다. 서버 재기동이 쿨다운(3 초)을 초기화하지만 재기동은 플레이어가 일으킬 수 없다.
- sim 은 판정을 위해 메모리 사본을 가진다. **기동 시 DB 에서 적재**하고, 그 뒤로는 sim 이 판정·갱신하며 tick 배치가 DB 를 따라오게 한다(write-behind).
- **소유자는 actor 다, 함선이 아니다.** 함선은 영속화되지 않는다(ADR-0011 §7) — 재기동과 잔류 만료로 사라지는 엔티티에 영속 자산을 매달 수 없다. 함선 화물칸은 함선 영속화 슬라이스에서 이 결정을 다시 연다.

### 2. tick 배치 트랜잭션이 곧 outbox 다 — 별도 outbox 테이블을 만들지 않는다

한 tick 의 영속화 트랜잭션은 이제 네 가지를 **함께** 쓴다.

| 쓰는 것 | 이유 |
|---|---|
| 그 tick 의 `domain_events` 전부 | 기존 (ADR-0007 §4) |
| 그 tick 이 바꾼 **상태 행**(인벤토리, 매장지 잔여) | 상태와 기록이 같은 커밋에 산다 |
| 그 tick 이 수락·적용한 상태 변경 명령의 `processed_commands` 행 | 지속 멱등성(§3) |
| `worlds.last_tick` | 기존 — tick 재개의 정본이자 **배치 멱등의 열쇠**(§4) |

Transactional Outbox 의 본질은 *"상태 변경과 '발행할 이벤트'를 한 트랜잭션에 넣고, 발행은 커밋 뒤에 따로 한다"* 이다. 여기서는 **`domain_events` 테이블 자체가 outbox 다.** 이벤트를 별도 테이블에 한 번 더 쓰고 릴레이가 옮기는 구조는 옮겨 갈 외부 버스(NATS 등)가 있을 때 의미가 있고, 지금 유일한 소비자(역사 엔진, ADR-0014)는 같은 DB 에서 `domain_events` 를 `(tick, sequence)` 순으로 읽는다. 같은 행을 두 번 쓰는 테이블은 두 기록이 갈라질 자리만 만든다.

**다시 검토할 조건**: 프로세스 밖 소비자(외부 버스, 별도 서비스)가 생길 때 — 그때 릴레이와 발행 상태 열을 도입한다.

### 3. 지속 멱등성 — `processed_commands` 와 월드 범위 기억

- `processed_commands(world_id, command_id, actor_id, command_type, tick)`, **PK `(world_id, command_id)`** — **상태를 바꾸는 명령이 수락·적용된 tick 의 트랜잭션에서** 쓴다. 거부된 명령은 쓰지 않는다(세계가 바뀌지 않았다 — ADR-0007 §1).
- **이것은 역사 기록이 아니다.** ADR-0007 §1 의 "명령을 `domain_events` 에 기록하지 않는다"는 그대로다. 이 테이블은 멱등 장부이고 역사 판정기가 읽지 않는다.
- sim 은 상태 변경 명령의 중복 제거를 **세션이 아니라 월드 범위**로 한다 — 어느 actor 가 보냈든 이 월드에서 이미 적용된 `command_id` 면 중복이다. 기억은 기동 시 그 월드의 `processed_commands` 에서 적재한다.
  - **기억의 범위 = PK 의 범위여야 한다**(server K1). 초안은 PK 가 `command_id` 하나(월드 무관)이고 기억은 actor 범위였다. 범위가 다르면 기억이 통과시킨 것을 PK 가 막고, PK 위반은 §5 의 **월드 정지**다 — 한 사용자가 토큰 둘로 같은 `command_id` 를 보내거나 QA 가 새 월드에서 `command_id` 를 재사용하는 것만으로 서버가 멈춘다(DoS). actor 범위 PK 가 아니라 월드 범위인 이유: 한 월드에 같은 `causation_id` 가 둘 생기면 `MINERAL_MINED.causation_id ↔ processed_commands.command_id` 조인(스펙 AC-18(b))이 모호해진다.
  - 다른 월드의 같은 `command_id` 는 별개다(월드는 역사의 단위 — 스펙 I-70). 그래서 같은 `command_id` 는 **재접속 뒤에도, 서버 재기동 뒤에도** 두 번 적용되지 않는다. 재전송은 `COMMAND_RESULT{REJECTED, DUPLICATE_COMMAND_ID}` 다(기존 코드, 의미 확장).
- DB 의 PK 가 마지막 방어선이다. sim 의 기억이 어떤 이유로든 중복을 통과시키면 **트랜잭션이 PK 위반으로 실패하고 §5 의 정지 경로로 간다** — 조용히 두 번 늘지 않고 시끄럽게 멈춘다.
- 세션 내 1024 기억(ADR-0006 §6)은 **모든 명령**에 계속 쓴다. 상태 변경 명령은 **둘 다** 본다: 세션 기억(거절된 것 포함 — 세션 안의 재전송은 무엇이든 `DUPLICATE_COMMAND_ID`, 스펙 AC-4(g))과 월드 지속 기억(수락된 것만). *(개정: 초안의 "두 기억은 겹치지 않는다" 는 스펙 AC-4(g) 와 모순이었다 — server 추가 1.)*
- 전량 적재의 규모 한계: 채굴 명령은 쿨다운으로 actor 당 초당 1건 미만이다. **`processed_commands` 가 한 월드에 100만 행을 넘으면** 적재 창(최근 N tick) + UUIDv7 시간 하한 거부로 바꾸는 ADR 을 쓴다. 지금 창을 두면 "창 밖의 옛 명령 재전송"이라는 복사 경로를 새로 만든다.
- **클라이언트의 재접속 재전송 금지(ADR-0005 §5)는 풀지 않는다.** 서버가 이제 재전송을 안전하게 만들지만, 재전송을 허용하는 것은 클라이언트 동작 변경이고 이번 슬라이스의 증명 대상이 아니다.

### 4. 배치는 tick 단위로 멱등이다

커밋 응답을 받지 못한 채 연결이 끊기면(모호한 커밋) 재시도가 **이미 커밋된 배치**를 다시 쓸 수 있다. 이벤트는 `ON CONFLICT (event_id) DO NOTHING` 으로 안전하지만 상태 행과 `processed_commands` 는 그렇지 않다.

→ **트랜잭션의 첫 동작이 `worlds` 행을 잠그고 `last_tick` 을 읽는 것**이다. `last_tick ≥ batch.tick` 이면 그 배치는 이미 커밋된 것이므로 **아무것도 쓰지 않고 성공으로 끝낸다.** 영속화는 단일 작성자·tick 오름차순이므로(§6) 이 비교 하나가 배치 멱등을 만든다.
- **건너뜀은 모호한 커밋 재시도에서만 일어난다**(K6). 정상 경로에서 건너뛴다면 단일 작성자 전제가 깨진 것이다. 그래서 건너뛸 때마다 `persist_ambiguous_commits_total` 을 올려 **이 경로가 실제로 실행됐는지** 보이게 하고(CLAUDE.md 검증의 규율 — 한 번도 안 도는 게이트는 아무것도 뜻하지 않는다), 이미 커밋된 배치이므로 `persisted_total` 도 올린다.
- `worlds` 의 `last_tick` 갱신은 **영향 행 수 = 1 을 단언**한다. 0 이면 월드 행이 없거나 tick 이 역행한 것이고, 복구 불가 기록 실패다(§5).

### 5. 상태 행은 비교 후 쓰기(compare-and-set)로 쓴다. 그리고 경제 기록 실패는 월드를 멈춘다

- 상태 행 갱신은 **"DB 의 현재 값이 sim 이 판정 전에 본 값과 같을 때만"** 쓴다. 0 행이 갱신되면 **메모리와 DB 가 갈라진 것**이다.
- **기대값은 sim 이 저장 표현 그대로 만들어 배치에 싣는다**(K3). 배치는 행마다 `{기대값: 저장 표현 그대로 | 없음(새 행), 새 값}` 을 갖고, 같은 행이 한 tick 에 여러 번 바뀌면 **첫 기대값과 마지막 새 값으로 접는다.** SQL 은 **동등 비교만** 한다. *(개정: 초안은 영속화가 이벤트 payload 에 회복식을 적용해 기대값을 유도하라고 했다 — 그러면 게임 규칙이 sim 과 영속화에 두 번 구현되고, 두 구현이 어긋나는 순간 **정직한 서버가 거짓으로 멈춘다.** 비교 후 쓰기는 "메모리와 DB 가 같은가" 를 재는 장치이지 규칙을 다시 계산하는 장치가 아니다.)* 이벤트 payload 의 before/after 는 그대로 남는다 — 그것은 원장이고 QA 의 항등식이 읽는다.
- 다음은 전부 **복구 불가 기록 실패**다: 제약 위반(23xxx), 비교 후 쓰기 0 행, `recorded_at` 생성 실패, payload 직렬화 실패.
- **복구 불가 기록 실패가 나면 서버는 월드를 멈춘다(fail-stop).** 새 명령 수락을 멈추고, 에러 로그와 `persist_fatal_total` 을 남기고, 정상 종료 경로로 프로세스를 끝내며 0 이 아닌 종료 코드를 낸다. 재기동은 DB 에서 상태를 다시 적재하므로 **DB 가 말하는 세계**로 돌아온다.
  - 지금의 "그 배치만 포기하고 계속"(맥락 3)을 **대체한다.** 경제 상태가 없던 시절엔 기록 한 줄의 손실이었지만, 이제 포기한 배치 뒤로 메모리가 DB 와 다른 세계를 계속 판정한다. 그 위에서 수락된 모든 명령이 틀린 전제를 딛는다.
  - `recorded_at` 실패 시 이벤트를 **건너뛰는** 현재 코드(맥락 4)는 **버그로 취급하고 고친다** — 건너뛰면 상태는 커밋되고 기록은 빠진 배치가 된다. 정확히 막으려는 모양이다.
  - **일시 실패는 허용 목록으로 정한다**(K4): sqlx `Io`·`PoolTimedOut`, SQLSTATE `08xxx`(연결), `40001`(직렬화), `40P01`(교착), `53xxx`(자원 부족), `57P01`~`57P03`(관리자 종료·충돌·접속 불가). 이것들만 같은 배치를 무한 재시도하고(§4 가 재시도를 안전하게 만든다) **나머지는 전부 복구 불가**다. 목록을 "복구 불가 쪽" 으로 두지 않는 이유: 모르는 오류가 재시도로 분류되면 백로그만 조용히 자라고, 복구 불가로 분류되면 시끄럽게 멈춘다 — 틀릴 거면 시끄러운 쪽으로 틀린다. **`57P01` 을 빠뜨리면 `docker compose stop postgres`(스펙 AC-6)가 월드 정지로 끝난다.**
  - **payload 직렬화 실패도 같다**: 지금 `extract_payload`(`persistence/src/lib.rs`)는 실패 시 JSONB `null` 을 저장하고 그것이 `NOT NULL` 을 통과한다(server 가 찾은 두 번째 조용한 경로). 복구 불가로 바꾼다.
  - **정지가 결정된 뒤에는 어떤 배치도 커밋하지 않는다**(K5) — 종료 스윕이 만드는 `SESSION_CLOSED`·`SHIP_DESPAWNED{SERVER_SHUTDOWN}` 도. 정지 결정 이후의 배치는 갈라진 메모리 위에서 만들어진 것이다. 그 결과 정지한 월드의 `domain_events` 에는 세션·함선의 닫힘 기록이 없다 — **짝 검사(p1-01 I-41)는 정지로 끝난 실행을 예외로 읽어야 하고**, 그 사실을 로그의 정지 사유가 알려 준다.
- 이것은 CLAUDE.md 의 "서버를 하드 킬하지 않는다"와 충돌하지 않는다. 그 규칙은 운영자가 서버를 죽이는 것에 대한 것이고, 여기는 서버가 **자기 무결성 위반을 발견하고 정상 종료 경로로 스스로 멈추는 것**이다.

### 5a. 정지 경로 전수표 — 클라이언트가 월드를 멈출 수 있는가 (리더 요청, 2026-09-27)

§5 가 복구 불가 기록 실패를 **월드 정지**로 만들었으므로(사용자 결정 Q1), **클라이언트 입력으로 도달할 수 있는 모든 정지 경로는 DoS 다.** 사용자는 "가용성보다 무결성" 을 골랐지만 *무결성 위반을 플레이어가 일으킬 수 있다* 는 전제는 없었다. K1 은 사례였다. 이 절은 **분모**다 — 이번 슬라이스가 쓰는 테이블의 제약 **전부**를 세고, 제약마다 ① 클라이언트 입력으로 도달할 수 있는가 ② 도달 전에 무엇이 거르는가 ③ 거르는 것이 실제로 거른다는 테스트가 있는가를 적는다. **①이 예이고 ②가 없으면 K1 과 같은 결함이다.**

**분모의 정의와 출처.** PostgreSQL 18 은 `NOT NULL` 도 `pg_constraint` 에 `contype = 'n'` 행으로 기록한다(2026-09-27 실측: `worlds` 11, `domain_events` 16). 그래서 분모 = **`pg_constraint` 에서 그 테이블의 행 수**이고, 아래 표의 행 수가 그것과 같아야 한다. 0002·0003 은 아직 없으므로 표는 **요구 제약 집합**이고, 마이그레이션이 생기면 qa 의 게이트가 `pg_constraint` 실측 수와 이 표를 대조한다 — **표와 DB 가 어긋나면 그 자체가 FAIL**(새 제약이 검토 없이 들어왔거나, 요구한 제약이 빠졌다). 추가 전용 트리거는 제약이 아니지만 UPDATE/DELETE 를 막으므로 별도 행으로 센다.

**"제약이 있다" 와 "제약이 막는다" 는 다르다.** server 가 찾은 `extract_payload` 의 JSONB `null` 은 `payload NOT NULL` 을 **통과했다** — SQL `NULL` 과 JSON `null` 은 다른 값이다. 그래서 0002 가 `domain_events` 에 `CHECK (jsonb_typeof(payload) = 'object')` 를 **추가**한다(기존 4,805 행은 전부 `object` 임을 실측했다 — 검증이 기존 행에서 실패하지 않는다). 0001 파일은 고치지 않는다(체크섬).

#### 5a-1. 제약 표 (분모 = 104 제약 + 트리거 4. 0003 의 50 + 3 은 history 실측과 architect 독립 실측으로 확정, 2026-09-28)

> ⚠ **이 표는 기계가 읽는다** — qa 의 `constraint_census.py`(p1-02 계약 SC-107)가 테이블별 소제목의 괄호 숫자와 **집계** 줄을 파싱해 `pg_constraint` 와 대조한다. 형식(테이블별 굵은 소제목의 "(마이그레이션, N)" 괄호, 집계 줄)을 바꾸면 qa 에 먼저 알린다. 역사 수는 실측으로 갱신했다(50, 2026-09-28).

범례 — ① 도달: **예** = 클라이언트가 값을 고른다 / **누적** = 클라이언트 행동의 누적으로 원리상 도달 / 아니오 = 값의 출처가 서버·데이터. ③ 테스트: 거르는 장치가 **실제로 거른다**는 실행 증거.

**`worlds` (0001, 11 — 실측)**

| 제약 | ① | ② | ③ |
|---|---|---|---|
| `worlds_pkey`, NOT NULL × 7(`world_id`·`name`·`tick_hz`·`calendar_epoch`·`calendar_scale`·`sim_version`·`created_at`), `tick_hz_check`, `calendar_scale_check` (10) | 아니오 — 영속화는 `worlds` 에 INSERT 하지 않는다. 행은 마이그레이션·운영자 SQL 로만 생긴다 | — | 해당 없음 |
| `worlds_last_tick_check` (1) | 아니오 — `last_tick` 은 영속화가 배치 tick 으로 쓴다 | 단일 작성자·tick 오름차순(§6), 영향 행 = 1 단언(K6) | 스펙 AC-4(e) |

**`domain_events` (0001 16 — 실측 + 0002 추가 1 = 17)**

| 제약 | ① | ② | ③ |
|---|---|---|---|
| `pkey`(`event_id`) | 아니오 — 서버 UUIDv7 | — | 기존 |
| `world_id_tick_sequence_key` | 아니오 — sim 이 tick 안 발행 순으로 부여 | — | 기존(ADR-0007 §4) |
| `world_id_fkey` | 아니오 — 기동 시 `worlds` 대조 | 기동 거부 | 기존 |
| `schema_version_check`, `tick_check`, `sequence_check` (3) | 아니오 | — | 기존 |
| NOT NULL × 10 (`event_id`·`event_type`·`schema_version`·`world_id`·`tick`·`sequence`·`occurred_at`·`recorded_at`·`correlation_id`·`payload`) | 아니오. `recorded_at` 만 실패 경로가 있다(호스트 시계) | `continue` 로 건너뛰던 것을 복구 불가로(§5) | 스펙 AC-5(d) |
| **`payload_is_object`**(0002 추가) | 아니오 — payload 의 문자열은 `data/` id·서버 UUID 뿐. 클라이언트의 `deposit_id` 는 **판정 2단계에서 광맥 표에 있는 것만** 통과하므로 이벤트에 실리는 것은 표의 문자열이다 | `TARGET_UNKNOWN` + 직렬화 실패 = 복구 불가(§5) | AC-5(d), 스펙 AC-3(b) |
| 추가 전용 트리거(제약 아님, 별도 1) | 아니오 | — | 기존 |

**`inventory_items` (0002, 7) — 키 `(world_id, actor_id, mineral_id)`**

| 제약 | ① | ② | ③ |
|---|---|---|---|
| `pkey` | 아니오 — `actor_id` 는 인증에서, `mineral_id` 는 광맥 표에서 유도 | — | — |
| `world_id_fkey` | 아니오 | 기동 대조 | — |
| NOT NULL × 4 | 아니오 | — | — |
| **`quantity_kg_check`** `BETWEEN 0 AND 2147483647` | **누적** — 적재 상한이 없다(Q4) | **sim 의 checked 덧셈 → `CAPACITY_EXCEEDED` 거절**(§5a-2 #4) | **경계값 주입 단위 테스트**(적재량을 상한 − 1 로 두고 채굴 → 거절, 인벤토리·이벤트·장부 불변, `commands_rejected_total{CAPACITY_EXCEEDED}` +1) — 스펙 AC-3(b) |

**`deposit_states` (0002, 11) — 키 `(world_id, deposit_id)`, 행은 첫 채굴 때 생긴다**

| 제약 | ① | ② | ③ |
|---|---|---|---|
| `pkey`, `world_id_fkey`, NOT NULL × 5(`world_id`·`deposit_id`·`remaining_kg`·`as_of_tick`·`first_extracted_tick`) (7) | 아니오 — `deposit_id` 는 판정 2단계를 통과한 표의 id | `TARGET_UNKNOWN` | AC-3(b) |
| `remaining_kg_check` `BETWEEN 0 AND 2147483647` | 아니오 — 산출 = min(산출량, 잔여), 잔여 0 이면 `RESOURCE_DEPLETED` 가 먼저. 회복은 초기 매장량으로 상한 | sim | AC-3(e)(g) |
| `as_of_tick_check` 범위 | 아니오 | — | — |
| `first_extracted_tick_check` `BETWEEN 0 AND 9007199254740991` | 아니오 — 값은 첫 채굴 tick(서버). `first_extracted_le_as_of` 가 상한은 함의하지만 **하한 0 은 함의하지 않아** 독립 제약이다 | — | — *(2026-09-28 추가: 0002 가 이 제약을 넣었고 SC-107 이 "표에 없는 제약" 으로 잡았다 — 게이트가 설계대로 동작한 첫 사례. 검토 결과 무해·정당, 표를 실측에 맞춤)* |
| `first_extracted_le_as_of` `first_extracted_tick <= as_of_tick` | 아니오 — 드러남은 첫 채굴 tick 에 한 번 | sim | AC-3(a) |

**`processed_commands` (0002, 8) — 키 `(world_id, command_id)`**

| 제약 | ① | ② | ③ |
|---|---|---|---|
| **`pkey` `(world_id, command_id)`** | **예 — `command_id` 는 클라이언트가 고른다** | sim 의 **월드 범위** 지속 기억(기동 시 적재) + 세션 기억 → `DUPLICATE_COMMAND_ID`. 기억과 PK 의 범위가 같다(§3 K1). 같은 tick 두 세션은 제출 순번으로 순차 판정 | 스펙 **AC-4(a)(b)(c)(d)(d2)** + 기억을 우회시킨 **AC-4(f)**(PK 가 실제로 막는다 — 막으면 정지이므로 이 경로는 테스트에서만 연다) |
| `world_id_fkey`, NOT NULL × 5(`world_id`·`command_id`·`actor_id`·`command_type`·`tick`), `tick_check` (7) | 아니오 | — | — |

**역사 테이블 (0003, 50 — history 실측 + architect 독립 실측 일치 2026-09-28)** — `historical_events` 22(c4·f1·n15·p1·u1), `historical_event_sources` 6(f2·n3·p1), `evidence` 14(c3·f1·n9·p1), `history_cursor` 8(c2·f1·n4·p1). *(48 → 50: §5a-1b 의 대조가 요구한 `evidence.rule_version NOT NULL` +1 과 권고한 `historical_event_sources.source_event_id → domain_events` FK +1.)* *(초안 25 와의 차이 23 의 분해 — 2026-09-28 architect 판정.)* **NOT NULL 30**(historical_events 15 · sources 3 · evidence 8 · cursor 4)은 세는 기준의 차이다 — 초안은 NOT NULL 을 "다수" 로만 적고 세지 않았다(PG18 은 `contype = 'n'` 으로 센다, §5a 머리). **NOT NULL 이 아닌 제약 18 중 초안 요구 목록에 이름이 있던 것은 10**(historical_events PK·UNIQUE·FK·fact_status·importance·tick 범위 6, sources PK 1, evidence PK·FK 2, cursor PK 1)이고, **목록에 없던 8 은 검토 없이 들어온 제약**이다 — 0002 의 `first_extracted_tick_check` 와 같은 경우. 제약별 판정:

| 목록 밖 제약 | 판정 | 이유 |
|---|---|---|
| `historical_events` `CHECK (array_length(source_event_ids, 1) >= 1)` | **승인** | 계약 `source_event_ids minItems: 1`·스펙 I-62 의 DB 판 |
| `historical_event_sources` FK → `historical_events` | **승인** | 근거 역조회 행이 없는 역사를 가리키지 못하게 |
| `evidence` `CHECK authenticity_status = 'VERIFIED'` | **승인** | 이번 슬라이스의 유일한 모양으로 잠금 — `fact_status` 와 같은 논리(ADR-0014 §3). 풀 때는 마이그레이션 + ADR |
| `evidence` `CHECK creation_method = 'automatic'` | **승인** | 같음 |
| `evidence` `CHECK derived_from_evidence_ids = '{}'` | **승인** | 같음 — 원본 증거의 표지 |
| `history_cursor` FK → `worlds` | **승인** | 없는 월드의 커서 방지 |
| `history_cursor` `CHECK tick` 범위 | **승인** | 계약 `Tick` 범위의 DB 판 |
| `history_cursor` `CHECK sequence` 범위 | **승인** | 계약 `Sequence` 범위의 DB 판 |

8 건 모두 ① 도달 아니오(쓰는 것은 러너뿐)이고 위반은 러너만 멈춘다. **검토 없이 들어온 것은 사실이고, 그것을 잡은 것은 표와 실측의 대조였다** — 요구 목록을 이름으로 적어 둔 덕에 "수가 다르다" 가 "어느 것이 다른가" 로 좁혀졌다. 앞으로 요구 제약은 **이름으로** 적는다("NOT NULL 다수" 같은 어림 금지). 구성(초안 목록): `historical_events`(PK, `UNIQUE (world_id, event_type, dedupe_key)`, FK world, `CHECK fact_status = 'CONFIRMED'`, `CHECK importance_level BETWEEN 1 AND 5`, tick 범위, NOT NULL 다수), `historical_event_sources`(PK), `evidence`(PK, FK), `history_cursor`(PK), 추가 전용 트리거 3.

| 전 제약 | ① 아니오 — 쓰는 것은 러너뿐이고 입력은 커밋된 `domain_events` 다(ADR-0014 §3) | ② — | ③ **그리고 이 제약들의 위반은 월드를 멈추지 않는다. 러너만 멈춘다**(ADR-0014 §4, 스펙 I-67) — AC-11(c)(e) |

**집계**: `worlds` 11 + `domain_events` 17 + `inventory_items` 7 + `deposit_states` 11 + `processed_commands` 8 = **54 제약(월드 정지 경로)**, + `domain_events` 추가 전용 트리거 1(제약 아님) / 역사 0003 **50 + 트리거 3**(history_cursor 는 가변이라 트리거 없음). **분모 합계 = 54 + 50 = 104 제약 + 트리거 4.** 이 중 **① 예 = 1**(`processed_commands_pkey`), **① 누적 = 1**(`inventory_items_quantity_kg_check`). 둘 다 ② 와 ③ 이 있다. **① 예·누적이면서 ② 가 없는 제약 = 0.**

#### 5a-1b. 독립 대조 기록 (architect, 2026-09-28 — 리더 지적: "구현의 실측을 표에 옮기면 SC-107 은 자기 일치가 된다")

표의 수가 구현자의 실측에서만 오면 게이트는 "구현이 만든 수 == 구현이 만든 수" 가 된다. 그래서 **architect 가 따로** 두 가지를 했다.

1. **독립 실측**: 증거 DB 가 아닌 임시 DB `starfall_test_architect_census` 를 만들어 `0001`~`0003` 을 `psql` 로 직접 적용하고 `pg_constraint` 를 종류별로 셌다 → 적용 뒤 `DROP`. 결과: `worlds` 11 · `domain_events` 17 · `inventory_items` 7 · `deposit_states` 11 · `processed_commands` 8 (경제 **54**) / `historical_events` 22 · `historical_event_sources` 5 · `evidence` 13 · `history_cursor` 8 (역사 **48**) / 사용자 트리거 4. **server·history 의 보고와 일치.** (증거 DB `starfall` 에는 아직 0001 만 적용돼 있다 — `_sqlx_migrations` 실측.)
2. **제약별 의도 대조** — 0003 파일을 읽고 종류별로 스펙·ADR-0014 요구와 맞춘다.

| 표 | p | u | f | c | n | CHECK·UNIQUE 의 의도 → 요구 출처 | 판정 |
|---|---|---|---|---|---|---|---|
| `historical_events` | 1 (결정적 id) | 1 `(world_id, event_type, dedupe_key)` | 1 → worlds | 4: importance 1~5 · tick 범위 · `fact_status = 'CONFIRMED'` · `source_event_ids` 비어있지 않음 | 15 (전 열) | UNIQUE = ADR-0014 §4-3 의미 유일성. CHECK = 계약 envelope(1~5, Tick, FactStatus, minItems 1) 과 ADR-0014 §3 | 전부 승인 |
| `historical_event_sources` | 1 `(source_event_id, detector_rule)` | — | **2** → historical_events · **→ domain_events(event_id)** | — | 3 | PK = ADR-0014 §4-2 근거 유일성. 둘째 FK = 스펙 I-62(모든 근거는 커밋된 `domain_events` 에 있다)를 테스트에서 구조로 | 승인 (2026-09-28 재실측) |
| `evidence` | 1 (결정적 id) | — | 1 → historical_events | 3: VERIFIED · automatic · `derived_from = '{}'` | **9** (+`rule_version`) | 원본 증거 잠금(ADR-0014 §3 개정), `rule_version` = ADR-0014 §5 | 승인 — 요구 누락 해소(2026-09-28 재실측) |
| `history_cursor` | 1 `(world_id, consumer)` | — | 1 → worlds | 2: tick·sequence 범위 | 4 | 가변 진행 위치(ADR-0014 §2) | 승인 |

NOT NULL 30 은 전 열 NOT NULL 이고 NULL 이 뜻을 갖는 열이 없다 — 승인. **제거를 요청할 제약은 없다.**

**요구 누락 (제약이 아니라 열)**: ADR-0014 §5 는 "모든 역사 기록**(과 증거)**은 `rule_version` 을 가진다" 인데 `evidence` 에 `rule_version` 열이 없다. 증거가 어느 규칙 버전으로 만들어졌는지는 `historical_event_id` 를 거쳐 유도할 수 있지만, 증거는 나중에 편집·파생 증거의 뿌리가 되는 독립 기록이고 요구가 명시적이다 → **history 에 `rule_version TEXT NOT NULL` 추가를 요청**. 증거 DB 에 0003 이 아직 적용되지 않았으므로(위 1) **0003 을 직접 고쳐도 체크섬 문제가 없다** — 테스트 DB 는 매번 새로 만든다. 권고(요구 아님): `historical_event_sources.source_event_id` → `domain_events(event_id)` FK — 스펙 I-62 "모든 근거는 `domain_events` 에 존재" 를 테스트(AC-11(a))에서 구조로 옮긴다. 둘 다 들어오면 이 표를 다시 실측해 갱신한다(evidence n 9 · 합계 +1, sources f 2 · +1).

**→ 해소(2026-09-28)**: history 가 둘 다 0003 에 반영. architect 가 임시 DB `starfall_test_architect_census2` 로 다시 독립 실측 → `historical_event_sources` 6(f2 — 대상 `historical_events`·`domain_events` 확인), `evidence` 14(n9), 역사 합 **50**, 경제 54 그대로, 트리거 4. **그리고 0003 은 이제 증거 DB 에 적용됐다**: `starfall._sqlx_migrations` 에 version 2·3 이 2026-09-28 13:15 UTC 로 들어왔고, 저장된 체크섬이 현재 파일의 SHA-384 와 일치한다(수정 후 적용 — `evidence.rule_version` 열 존재 확인). **이 시점부터 0002·0003 은 동결이다 — 고치려면 새 마이그레이션 파일(0004…)을 추가한다.**

**`→ domain_events` FK 의 판단**: 참조 대상이 추가 전용이라 참조 무결성을 깰 경로가 없다 — UPDATE/DELETE 는 트리거가 막고, 행 단위 트리거가 못 막는 `TRUNCATE` 는 이 FK 가 오히려 막는다(참조되는 표는 `CASCADE` 없이 TRUNCATE 할 수 없다 — ADR-0007 §3 의 알려진 구멍 하나가 역사가 쌓인 뒤에는 닫힌다). `ON DELETE` 동작은 무관하다(기본 `NO ACTION`, 삭제가 없다). 쓰기 순서도 안전하다: 러너는 **커밋된** 행만 읽으므로(ADR-0014 §2) 근거가 없는 상태에서 sources 를 쓰는 일이 없다. 비용은 sources INSERT 마다 PK 조회 1 회 — 발견은 월드당 최대 4 건이다.

#### 5a-2. 제약 밖의 정지 원인

| # | 원인 | ① | ② | ③ |
|---|---|---|---|---|
| 1 | 인벤토리·광맥 CAS 0 행 | 아니오 — 괴리는 운영자 SQL·코드 결함뿐. 같은 행이 한 tick 에 여러 번 바뀌어도 K3 이 접는다 | — | AC-5(b)(변조로만 유발) |
| 2 | `recorded_at` 생성 실패 | 아니오 | — | AC-5(d) |
| 3 | payload 직렬화 실패 | 아니오 | — | AC-5(d) |
| 4 | 인벤토리 누적 넘침(= `quantity_kg_check` 에 닿기 전) | **누적** — 월드 공급률 합 ≈ 33 kg/s(ferrosite 20 + glacine 10 + cobaltine 2.7 + glass 0.14) + 초기 매장량 59,100 kg. 한 actor 가 한 광물의 전 공급을 독점해도 ferrosite 로 약 **3.4 년** | sim 의 checked 덧셈 → **`CAPACITY_EXCEEDED` 로 거절**. **디버그·릴리스 같은 경로**다(panic 을 두지 않는다 — 표준 게이트가 디버그로 돌므로 panic 을 두면 초록불이 운영 경로를 재지 않는다, server 제안·리더 지지. p1-01 의 `debug_assert_i29` 가 먼저 panic 해 새 단언이 평가되지 않았던 전례). 더 크게 알리려면 panic 이 아니라 **거절 + 카운터 + ERROR 로그** — 경로는 하나여야 테스트가 그 경로를 잰다. 거절 코드는 이 슬라이스에 추가한다(아래) | 5a-1 의 경계값 테스트 |
| 5 | 허용 목록 밖 DB 오류 | 아니오 — 운영 환경 | — | AC-5(f) 의 짝 |

**`CAPACITY_EXCEEDED` 를 지금 넣는 이유**(server 질문 1 에 대한 답): 기존 값은 전부 오도한다 — `RESOURCE_DEPLETED` 는 광맥이 비었다는 뜻이고 `MALFORMED_COMMAND` 는 형식 오류다. 오도하는 사유는 클라이언트 표시와 QA 판정을 둘 다 틀리게 만든다. 반대 논리("구조적으로 도달할 수 없는 사유 코드는 만들지 않는다" — 스펙 §4.2 의 `NO_ACTIVE_SHIP` 판단)와 구별되는 점: **이 경로는 도달 불가가 아니라 도달 비용이 클 뿐이고, 경계값 주입으로 실행 증거를 낼 수 있다.** 이름은 앞으로 설계 상한(적재량)이 들어와도 같은 뜻으로 쓰인다 — 그래서 p1-03 만기는 "전용 사유 추가" 에서 "설계 상한이 들어오면 같은 코드로 판정 위치만 앞당긴다" 로 바뀐다.

**규칙**: 새 테이블·새 제약·새 명령 필드가 생기는 슬라이스는 이 표에 행을 더하고 분모를 다시 센다. **표에 없는 제약은 검토되지 않은 정지 경로다.**

### 6. 손실 창과 그 상한

write-behind 의 대가는 **"커밋되지 않은 꼬리는 크래시에서 사라진다"**이다(ADR-0007 §6 의 창이 그대로 남는다). 이 ADR 이 바꾸는 것은 **사라지는 방식**이다.

- 사라질 때 **상태·이벤트·멱등 장부가 함께 사라진다**(같은 트랜잭션). 그래서 크래시의 결과는 *"플레이어가 마지막 몇 초의 채굴을 잃었다"*이지 *"수량만 남았다"* 나 *"같은 명령이 재기동 뒤 다시 적용됐다"* 가 아니다. **잃을 수는 있어도 복사는 없다.**
- 창의 크기는 **`recording_lag = last_enqueued_tick − last_committed_tick`**(tick) 이다 — 영속화 채널에 넣었지만 아직 커밋되지 않은 구간. **상태 변경 명령은 `recording_lag` 가 임계를 넘으면 게이트웨이에서 거부한다** — 새 거부 코드 `RECORDING_BACKLOG`. 임계 잠정값 **20 tick(1 초)**. 이동 명령은 이 거부를 받지 않는다(이동은 DB 에 의존하지 않는다 — p1-01 I-31).
  - *(개정 K2)* 초안은 기존 `persist_backlog`(현재 tick − 마지막 커밋 tick)로 판정하라고 했다. 이벤트 없는 tick 은 하트비트 배치가 20 tick 마다 나가므로 `persist_backlog` 는 평상시에도 0→20 톱니를 그리고, **그 꼭대기가 임계 20 과 같다** — 커밋이 50 ms 만 늦어도 한가한 서버가 채굴을 거부한다. `recording_lag` 는 "넣었는데 아직 안 된 것" 만 재므로 한가할 때 0 이다. 연결 거부(600 tick, ADR-0007 §4)는 기존 `persist_backlog` 를 그대로 쓴다.
  - *(개정 K2b, 2026-09-30 — qa 계약 외 발견)* **`last_enqueued − last_committed` 도 틀린 양이었다.** 한가한 서버·건강한 DB 에서 실측 최대가 **정확히 20 = 임계**였다(`evidence/q2_backlog_idle_20260930/backlog-idle.json`, 폴링 2,315 회). 기전: 마지막 커밋이 하트비트 H 이고 다음 배치가 H+20 에 들어오면, 그 사이 tick 에 **기다리는 일이 하나도 없었는데도** 차이가 20 이다 — 이 식은 "커밋 대기 중인 일의 나이" 가 아니라 **"마지막 커밋과 마지막 투입 사이의 tick 거리"** 를 재고, 그 거리는 하트비트 간격이 상한이다. K2 가 없애려던 "톱니 꼭대기 = 임계" 가 한 칸 옮겨 남은 것이다(커밋이 1 tick 만 늦어도 한가한 서버가 거부한다).
  - **정의(최종)**: `recording_lag = 현재 tick − (아직 커밋되지 않은 배치 중 가장 오래된 것의 tick)`, 커밋 대기 배치가 없으면 **0**. 영속화 채널에 넣은 배치 tick 을 순서대로 기억하고(채널 용량만큼의 FIFO) 커밋된 tick 이하를 앞에서 버린다. 이것이 크래시가 잃을 수 있는 가장 오래된 경제 행동의 나이이므로 **손실 창(§6 첫 문단)과 정확히 같은 양**이다. 하트비트 간격과 무관하다: 건강한 DB 에서 배치는 1~2 tick 안에 커밋되므로 한가할 때 0~2 에 머물고, DB 가 멈추면 그 순간부터 1 tick 에 1 씩 자란다.
  - 임계 20 은 그대로(손실 창 1 초 < 쿨다운 3 초). **판정 기준(관측 전 기록)**: 같은 `backlog-idle` 실행에서 새 정의의 최대 ≤ 5 이고, 같은 실행의 `persist_backlog` 톱니 최대 ≥ 20(하트비트가 실제로 있었다 — 대조가 겨냥한 조건), DB 정지(AC-6) 실행에서는 새 정의가 정지 뒤 21 tick 안에 20 을 넘어 거부가 시작된다.
  - **결과와 기준 정정(2026-09-30, qa 실측)**: backlog-idle — 새 정의 최대 **0**, 같은 실행 `persist_backlog` 최대 20 → 충족. DB 정지 — 정지부터 `> 20` 첫 표본까지 **42·32 tick** → **내가 적은 "정지 뒤 21 tick" 은 불충족이고, 틀린 것은 기준이다**(규칙 3 — 기준을 만드는 사람도 자기 기준에 규칙 1 을 돌린다). 정의상 lag 는 **커밋 대기 배치가 생긴 순간부터** 자란다. 이동은 도메인 이벤트를 만들지 않으므로 DB 가 내려간 뒤 첫 대기 배치는 다음 하트비트(≤ 20 tick) 또는 다음 채굴이고, 그 전에는 **잃을 것이 없으므로 0 이 옳다.** 정정한 기준: ① **첫 대기 배치의 tick 부터** 21 tick 안에 `> 20` ② 정지부터는 **하트비트 간격 20 + 21 = 41 tick + 표본 간격**(100 ms 폴링 = 2 tick) 안. **⚠ 이 둘은 사후 정정 기준이다 — 2026-09-30 실행 두 건(42·32)을 본 뒤에 만들었다. 그 두 실행은 이 기준의 근거가 아니라 계기이고, 이 기준으로 그 데이터를 "충족" 이라 판정하지 않는다**(특히 ②의 +2 는 42 를 통과시키는 딱 그만큼이다 — 리더 지적). 판정은 **새 실행**으로만 한다: server 가 `oldest_pending_tick` 을 노출한 뒤 ①(직접 기준)으로.
  - **직접 기준 판정 (Phase 5 r1, 2026-09-30 — qa 가 관측 전에 방법 등록: 표본 100 ms, ⊘ `oldest_pending_tick` 비-null 전이 선단언, `delta ≤ 21 + g`, g = 그 표본의 직전 표본 간격)**: 첫 대기 배치 tick P = 1297 → `recording_lag > 20` 첫 표본 tick 1318, **delta 21 ≤ 21 + g(2) — 성립.** 함께 등록한 단언: 정의 항등식 `recording_lag == tick − oldest_pending_tick` 위반 **0 / 262** 표본, 이른 교차(`tick − P ≤ 20` 에서 lag > 20) **0**. 근거: `_workspace/p1-02-mining/04_qa_report_r1.md`. **K2b 는 이 실행으로 닫는다** — 사후 기준(②)은 판정에 쓰지 않았다. ①을 직접 재려면 가장 오래된 대기 배치의 tick 이 관측돼야 한다 — `/debug/stats` 에 `oldest_pending_tick` 을 싣는 것은 server S5 에 요청(없으면 ②로만 판정).
  - 기존 연결 거부 임계(600 tick, ADR-0007 §4)와 다른 값인 이유: 연결 거부는 "기록이 30 초 밀렸다"를, 이것은 "잃어도 되는 경제 행동의 양"을 잰다. 1 초는 채굴 쿨다운보다 짧게 잡은 값이다(= 크래시가 잃는 채굴은 actor 당 최대 1 회).
- 영속화의 전제: **단일 작성자, tick 오름차순 커밋.** §4 의 배치 멱등과 ADR-0014 의 커서 읽기가 모두 이 전제에 선다. 영속화를 병렬화하는 변경은 이 ADR 을 다시 연다.

### 7. 기동 순서

1. 마이그레이션 → `worlds` 대조(기존)
2. **경제 상태 적재**: 그 월드의 인벤토리·매장지 상태·`processed_commands` → sim 초기 상태. 매장지 행이 없는 매장지는 `data/` 의 초기 매장량·미드러남으로 시작한다(행은 첫 채굴 때 생긴다)
3. **역사 적재**(`history::load`): 판정 상태 재구축 + PUBLIC 기록 목록(ADR-0014 §6)
4. **런타임 조립**(목록을 인자로 받는다) — **여기서 tick 이 시작한다**(ADR-0006 §2.3 재개)
5. 러너·영속화 태스크 기동
6. 소켓 바인드 = 연결 수락

*(개정 — server 추가 2, T0 합의)* 초안은 tick 재개 뒤에 러너가 목록을 넘기게 했다. 목록을 런타임 조립의 **인자**로 받으면 "목록 없이 게이트웨이가 선다" 가 타입으로 불가능해진다 — 누락 틈(ADR-0014 §6)을 순서가 아니라 구조로 막는다.

2 가 4 보다 앞이어야 한다. 상태 없이 tick 이 돌면 첫 채굴이 "빈 인벤토리"를 전제로 판정되고 비교 후 쓰기(§5)가 바로 월드를 멈춘다 — 그 경우 정지가 옳은 결과이지만 원인이 기동 순서라는 것을 로그가 말해 주지 않는다.

## 검토한 대안과 버린 이유

- **동기 커밋(명령 처리 중 DB 트랜잭션을 기다림)**: tick 루프가 DB 지연을 먹는다(ADR-0007 §4 가 버린 이유 그대로). 채굴 한 건을 위해 20 Hz 가 흔들린다.
- **상태를 DB 에서만 판정(sim 이 매번 조회)**: sim 에 IO 가 들어간다(ADR-0001 §2, AC-1 금지 목록). 결정성 재생이 DB 에 묶인다.
- **별도 `outbox` 테이블 + 릴레이**: §2. 소비자가 같은 DB 에 있는 동안은 같은 사실을 두 번 쓰는 것뿐이다.
- **이벤트 payload 의 `command_id` 에 부분 유니크 인덱스(`domain_events` 로 멱등)**: 테이블이 하나 줄지만, 명령 하나가 이벤트 0개·여러 개를 내는 순간(거래) 성립하지 않고, 멱등 장부와 역사 기록을 한 테이블에 섞는다.
- **상태를 델타(`quantity = quantity + n`)로 쓰기**: 모호한 커밋 재시도에 이중 적용된다(§4 가 막더라도 두 번째 방어가 없다). 비교 후 쓰기는 **메모리-DB 괴리를 검출하는 장치**를 공짜로 준다.
- **실패한 경제 배치를 포기하고 계속**: §5. 메모리가 틀린 세계를 계속 판정한다.
- **Redis 에 인벤토리**: 원칙 3.

## 결과

- 좋은 점: 크래시·재시도·재접속·재기동 어디서도 `command_id` 하나가 인벤토리를 두 번 바꾸지 못한다 — 메모리 기억, DB PK, 배치 멱등, 비교 후 쓰기가 네 겹이다. 기록과 상태가 한 커밋에 있어 **"인벤토리 = 이벤트 합"** 항등식을 QA 가 SQL 한 줄로 잴 수 있다. outbox 테이블이 하나 줄었다.
- 감수할 점: 크래시는 최대 `RECORDING_BACKLOG` 임계만큼의 경제 행동을 잃는다(플레이어가 본 수락이 사라질 수 있다). 기록 실패가 월드 전체를 멈춘다 — 가용성보다 무결성을 고른 것이다. `processed_commands` 전량 적재는 규모 한계가 있다.
- 다시 검토할 조건: (a) 프로세스 밖 이벤트 소비자 → outbox 릴레이, (b) `processed_commands` 100 만 행 → 적재 창 ADR, (c) 영속화 병렬화 → §4·§6 재설계, (d) 함선 영속화 → 소유자(actor vs 함선) 재결정, (e) 첫 부하 측정에서 `RECORDING_BACKLOG` 거부가 정상 부하에서 1 % 를 넘으면 임계 재조정.

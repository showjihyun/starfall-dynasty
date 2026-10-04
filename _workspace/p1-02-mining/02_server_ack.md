# p1-02-mining — server 검토 (Phase 3, 태스크 #7)

- 작성: server, 2026-09-27
- 대상: ADR-0013(accepted, 개정으로 반영), ADR-0014 §2·§6(server 경계), 현재 영속화 코드 결함, 스프린트 계약 server 항목, T0
- 이 문서의 코드 인용은 전부 브랜치 `p1-02-mining` 작업 트리에서 **직접 읽은 것**이다(행 번호 포함). 실행한 것과 읽은 것을 구분해 적는다.

## 1. ADR-0013 — **수락 (2026-09-27, architect 가 K1~K6·추가 1~3 전부 반영 개정)**

> 개정 확인 방법: `docs/adr/0013` 본문에서 PK `(world_id, command_id)`(54행), K6 건너뜀 관측(69행), K4 허용 목록(80행), `extract_payload` null 경로(81행), K5 종료 스윕 포함 커밋 금지(82행), `recording_lag`(90–91행)를 grep 으로 확인. ADR-0014 §6 트리거 32(89–90행) 확인. 스펙 신설 항목 AC-4(d2)·AC-5(e)(f)·AC-6(a2) 는 architect 통보 기준. 계약 파일 변경 없음.

(아래는 검토 당시 원문 — 기록으로 남긴다)

### 검토 당시: 조건부 수락. 수정안 6건(K1~K6), 그중 K1·K2·K3 은 수락 전제

골격(tick 배치 = outbox, 지속 멱등, 배치 멱등, 비교 후 쓰기, fail-stop, 게으른 회복, 기동 적재)은 **구현 가능하고 수락한다.** tick 루프는 DB 를 기다리지 않는 구조 그대로이므로 tick 본문 기준선(p0-02 M-2 최대 21.65 ms)에 거는 것은 sim 의 채굴 판정뿐이다(명령당 O(log n) 조회 몇 번 — 무시할 수준, §1.7). 비용은 전부 영속화 태스크 쪽에 있다.

아래 셋은 **ADR 원문대로 구현하면 결함이 되는 것**이다.

### K1. `processed_commands` 의 PK 와 sim 기억의 범위가 다르다 → 한 사용자가 월드를 멈출 수 있다 (수락 전제)

- 원문: 테이블 `command_id PK`(월드 무관 전역), sim 기억은 **actor 범위**.
- 결함 경로 A(월드 정지 DoS): 한 사용자가 개발 토큰 둘(actor A, B)로 **같은 `command_id`** 를 채굴 명령에 쓴다. sim 은 actor 범위라 B 의 것도 통과 → 같은 tick 배치(또는 뒤 배치)에서 PK 위반 → §5 에 따라 **월드 전체 정지**. 공격자 한 명이 서버를 끌 수 있다.
- 결함 경로 B(QA 새 월드): 발견 측정은 매번 새 월드에서 한다(태스크 문서 ⚠1). 봇이 결정적/재사용 `command_id` 를 쓰면 새 월드의 첫 채굴이 **옛 월드의 행**과 PK 충돌 → 정지. 새 월드의 sim 기억은 그 월드 행만 적재하므로 막지 못한다.
- 수정안: **PK = `(world_id, command_id)`, sim 기억도 월드 범위**(`BTreeSet`/`HashSet<command_id>` 하나). 두 겹의 범위가 같아야 "DB PK 는 sim 이 뚫렸을 때만 걸린다"가 참이 된다.
  - actor 범위 대신 월드 범위를 고르는 이유: `MINERAL_MINED.causation_id = command_id` 이고 AC-18(b)·history 오라클이 `causation_id ↔ processed_commands.command_id` 를 1:1 로 조인한다. actor 범위 PK `(world_id, actor_id, command_id)` 로 맞추면 DoS 는 막히지만 한 월드에 같은 causation_id 가 둘 생겨 조인이 모호해진다.
  - 스펙 I-52 문구("actor 범위")는 **월드 범위가 actor 범위를 포함**하므로 약속을 깨지 않는다(같은 actor 의 재전송은 여전히 거부). 다른 actor 가 남의 id 를 쓰면 `DUPLICATE_COMMAND_ID` 를 받는 것이 달라지는 유일한 동작이다. architect 판단 요청.

### K2. `RECORDING_BACKLOG` 가 재는 값이 하트비트 주기와 같은 20 이다 → 한가한 월드에서 오거부 (수락 전제)

- 코드 확인: `persist_backlog = current_tick − last_committed_tick` (`crates/gateway/src/stats.rs:465`). 배치는 이벤트가 있거나 `tick % HEARTBEAT_TICKS == 0` 일 때만 보낸다(`crates/gateway/src/runtime.rs:459`), `HEARTBEAT_TICKS = 20` (`runtime.rs:120`).
- 그래서 **이벤트가 없는 구간**(세션은 있고 `SET_SHIP_CONTROL` 만 오가는 평상시 — 이동은 도메인 이벤트를 만들지 않는다)에서 `persist_backlog` 는 매초 0→20 으로 톱니를 그리고, 하트비트 커밋이 1 tick(50 ms) 넘게 걸리는 순간 21 이 된다. 임계 20 과 톱니 꼭대기가 같다. 잃을 경제 행동이 **0 건**인데 채굴이 거부된다.
- 손실 창의 정의는 "커밋되지 않은 **보낸** 배치"다. 수정안: **`recording_lag = last_enqueued_tick − last_committed_tick`** (드라이버가 배치를 `deferred` 에 넣을 때 `last_enqueued_tick` 을 기록). 모든 배치가 커밋됐으면 0 이다. 이것으로 `RECORDING_BACKLOG` 를 판정하고, 기존 `persist_backlog`(연결 거부 600)는 그대로 둔다. DB 정지(AC-6) 중에는 채굴 이벤트 배치가 쌓이므로 lag 가 정상적으로 자란다 — AC-6(a) 는 그대로 성립한다.
- `/debug/stats` 에 `recording_lag` 를 새 키로 낸다(AC-6 증거용).

### K3. 비교 후 쓰기의 "기대값" 을 이벤트 payload 에서 유도하지 않는다 — sim 이 상태 쓰기 목록을 배치에 싣는다 (수락 전제)

- 원문 §5: DB 의 매장지 `(remaining, as_of_tick)` 에 **회복식을 적용한 값**이 payload 의 `deposit_remaining_before_kg` 와 같아야 한다. 이러면 영속화가 회복식(= 게임 규칙, designer 수치 `regen_kg`·`regen_interval_ticks`·`initial`)을 **두 번째로 구현**해야 한다. 두 구현이 한 번이라도 어긋나면(반올림, 상한 처리) **거짓 월드 정지**다. persistence 는 `data/` 를 모른다(`lib.rs:112-115` 가 그 경계를 적어 둔다).
- 수정안: `PersistBatch` 에 `state_writes: Vec<StateWrite>` 를 더한다. 행 단위·tick 단위로 접는다(같은 tick 에 같은 행을 두 번 바꾸면 한 항목 — 같은 tick 두 actor 동시 채굴, S-2).
  - `Inventory { actor_id, mineral_id, expected: Option<i64>, new: i64 }`
  - `Deposit { deposit_id, expected: Option<(remaining_kg, as_of_tick)>, new: (remaining_kg, as_of_tick), first_extracted_tick }`
  - `expected = None` 은 "행이 없어야 한다" → `INSERT … ON CONFLICT DO NOTHING` 후 영향 행 1 확인. `Some` 은 `UPDATE … WHERE <열> = expected` 후 영향 행 1 확인. **저장된 표현 그대로를 비교**하므로 SQL 에 게임 규칙이 없다.
  - 회복식의 정확성은 sim 단위 테스트(I-69)가, "상태 = 이벤트 합" 은 QA 의 원장 SQL(AC-5(a))이 잰다. "이벤트와 state_writes 가 서로 맞다"는 sim 속성 테스트로 건다(채굴 N 회 무작위 → 이벤트의 before/after 체인과 state_writes 가 일치).
  - AC-5(b)(SQL 로 인벤토리 변조 → 채굴 → 정지)는 이 방식에서도 그대로 걸린다: sim 의 `expected` ≠ DB 값 → 0 행 → 정지.

### K4. 복구 불가/일시 실패 분류를 "일시 실패 허용 목록" 으로 적는다

- 현재 `commit_with_retry` 는 23xxx 만 특별 취급하고 **나머지 전부를 무한 재시도**한다(`lib.rs:250-272`). 22xxx(데이터 예외)·42xxx(스키마 불일치) 같은 절대 성공하지 않는 오류가 "일시 실패" 로 영원히 돈다.
- 수정안: **일시 실패 = `sqlx::Error::{Io, PoolTimedOut, PoolClosed?}` + SQLSTATE `08xxx`(연결), `40001`·`40P01`(직렬화·교착), `53xxx`(자원), `57P01`~`57P03`(관리자 종료·크래시)**. 그 밖의 DB 오류와 §5 의 목록은 전부 복구 불가 → 정지.
  - `57P01 admin_shutdown` 을 반드시 일시 실패에 넣어야 한다: AC-6 의 `docker compose stop postgres` 가 커밋 도중이면 이 코드가 온다. 빠뜨리면 **AC-6 이 월드 정지로 끝난다.** AC-6 이 이 분류의 실행 증거가 된다.

### K5. 복구 불가 실패 뒤에는 아무것도 커밋하지 않는다 — 종료 스윕의 배치도

- §5 "정상 종료 경로로 끝낸다" 를 그대로 구현하면 지금의 종료 경로는 종료 스윕의 `SESSION_CLOSED` 배치를 **영속화에 넘겨 커밋을 기다린다**(`runtime.rs` 종료 스윕, `main.rs:250-252`). 정지 뒤의 배치는 갈라진 메모리 위의 기록이므로 커밋하면 안 된다.
- 수정안(ADR 에 한 줄): 정지가 결정되면 영속화 태스크는 **이후 어떤 배치도 커밋하지 않고** 수신단을 닫고 끝난다. 종료 스윕의 `SESSION_CLOSED` 는 잃는다(크래시와 같은 창). 구현: 영속화가 `fatal: Arc<Notify>`(또는 `watch`)를 울리고, `main` 의 graceful shutdown `select!` 가 그것도 기다린다 → 기존 종료 경로 → `serve` 가 `Err` 를 돌려 종료 코드 1. 게이트웨이는 `halted` 원자값을 보고 상태 변경 명령을 즉시 `RECORDING_BACKLOG` 로 거부한다(새 사유 코드를 만들지 않는다 — lag 가 곧 무한대가 되므로 의미도 같다).

### K6. 배치 멱등의 "건너뜀" 은 관측 가능해야 한다

- §4 의 `last_tick ≥ batch.tick → 아무것도 쓰지 않고 성공` 은 **모호한 커밋 재시도에서만** 일어난다(재기동은 `last_tick + 1` 부터 시작 — `lib.rs:187-199`). 그때 앞선 시도는 오류로 끝나 `persisted_total` 을 올리지 않았으므로, 건너뜀에서 올려야 `persisted_total == COUNT(*)` 항등식이 유지된다.
- `persist_ambiguous_commits_total` 을 둔다. 이 경로가 실행됐는지 알 수단이 없으면 AC-4(e) 의 초록불은 "건너뛰었다" 가 아니라 "아무것도 안 했다" 일 수도 있다(CLAUDE.md 검증 규율). AC-4(e) 의 DB 통합 테스트는 **두 번째 커밋의 반환값이 `AlreadyCommitted` 임**을 단언한다.
- `worlds` 갱신도 잠금 비교 뒤에는 **영향 행 1 을 단언**한다(현재 `lib.rs:316-323` 은 조건부 UPDATE 의 결과를 보지 않는다).

### 1.7 성능 — 무엇을 거는가

| 자리 | 추가 비용 | 판단 |
|---|---|---|
| tick 본문 | 채굴 명령당 매장지·인벤토리·중복 기억 조회(BTreeMap/HashSet) | 21.65 ms 기준선에 측정 불가 수준. 목표 판정은 기존 tick p99 게이트로 |
| 메모리 | 중복 기억 = 월드의 `processed_commands` 전량. 항목당 ~40 B → 100만 행 ≈ 40 MB | ADR 의 100 만 행 트리거와 맞는다 |
| 기동 | 전량 적재 1 쿼리 | 100 만 행에서 수 초 — 트리거 이내 |
| 영속화 트랜잭션 | `SELECT … FOR UPDATE`(worlds) 1 + 이벤트 N + state_writes M + 장부 K + worlds 1 왕복. 지금 이벤트를 **한 행씩** INSERT 한다(`lib.rs:285-312`) | 100 명·쿨다운 3 초 → tick 당 채굴 ≈ 1.7 → 트랜잭션당 ≈ 10 왕복. 로컬 PG 에서 수 ms. 50 ms 예산 안. UNNEST 배치화는 **측정이 요구할 때**(원칙 8) |
| 위험 | 커밋 지연 꼬리가 1 초를 넘으면 `RECORDING_BACKLOG` 가 정상 부하에서 뜬다 | `persist_commit_us` p50/p99 를 `/debug/stats` 에 낸다 → ADR 재검토 조건 (e) 의 1 % 를 **잴 수단**. 없으면 (e) 는 판정 불가 |

`worlds` 행 잠금은 러너의 워터마크 읽기(일반 SELECT, MVCC)를 막지 않는다.

### 1.8 ADR-0013 §3 끝줄 ↔ 스펙 AC-4(g) 모순

- ADR: *"세션 내 1024 기억은 상태를 바꾸지 않는 명령에 계속 쓴다. 두 기억은 겹치지 않는다."*
- 스펙 §4.2 표 1행·AC-4(g): *세션 안에서는 **거절된** 채굴 명령의 재전송도 `DUPLICATE_COMMAND_ID`.* 이것은 세션 기억이 `MINE_RESOURCE` 에도 걸린다는 뜻이다(지속 기억은 수락분만 적는다).
- 구현은 스펙대로 한다: **`MINE_RESOURCE` 는 세션 기억(모든 id) → 지속 기억(수락 id) 순으로 둘 다 본다.** 지속 기억 삽입은 수락 **즉시**(커밋 전) — 그래야 AC-4(d)(같은 tick, 같은 id, 두 연결)가 수락 1·중복 1 이 된다. ADR 문장을 고쳐 달라.

### 1.9 기동 순서 — ADR-0013 §7 의 3·4 를 바꾼다

T0 합의(§4)에 따라 **러너 적재(목록 확보)가 tick 드라이버 생성보다 앞**이다. 목록이 `runtime::build` 의 인자라서 "목록 전 연결 수락" 이 타입으로 불가능해진다. 러너의 따라잡기(커서 뒤 처리)는 드라이버가 뜬 뒤 비동기로 하고 그 결과는 LIVE 로 흐른다. 개정 순서: 마이그레이션·월드 대조 → **경제 상태 적재** → **역사 적재(판정 상태·PUBLIC 목록·커서)** → 드라이버 생성(tick 시작, 목록 주입) → 영속화·러너 spawn → bind.

## 2. 현재 코드 결함 — 확인함, 그리고 결함은 하나가 아니라 셋이다

### 2.1 `recorded_at` 실패 시 이벤트를 건너뛴다 — 확인

위치: `server/crates/persistence/src/lib.rs:285-288`

```rust
for event in &batch.events {
    let Some(recorded_at) = now_real_time() else {
        continue;
    };
```

확인한 방법(읽기 + 경로 추적, 실행 아님):

1. `now_real_time()`(`lib.rs:203-206`)은 `SystemTime` 이 UNIX epoch 이전이거나 `RealTime::from_unix` 가 연도 0..10000 밖을 만나면 `None`(`crates/contracts/src/primitives.rs:335-342`). 호스트 시계에 달려 있어 **실행으로 유도할 수단이 지금 코드에 없다**(시계 주입점 없음). 그래서 이 결함은 지금까지 어떤 테스트로도 실행된 적이 없다.
2. `continue` 뒤의 흐름: 그 이벤트의 INSERT 만 빠지고 루프가 계속된다 → `UPDATE worlds SET last_tick`(`lib.rs:316-323`)은 **그대로 실행** → `tx.commit()` 성공 → `commit_with_retry` 가 `persisted_total += batch.events.len()`(`lib.rs:238-240`) — **빠진 이벤트까지 센다.**
3. 결과는 세 겹: (a) 기록 누락, (b) `last_tick` 이 그 tick 을 "완료" 로 표시 — ADR-0013 §4 의 배치 멱등이 들어오면 **재시도도 그 tick 을 건너뛰어 구멍이 영구화**, (c) `persisted_total` 이 DB 행 수보다 커져 카운터가 거짓말한다. 경제 상태가 같은 트랜잭션에 들어오면 (a)는 "수량은 커밋, 기록은 누락" 이 된다.

재현 방법(S4 의 첫 RED, AC-5(d)): `commit` 에 시계 주입점을 만든다(`clock: &dyn Fn() -> Option<RealTime>`, 운영 경로는 `now_real_time`). **동작을 바꾸지 않은 채** `|| None` 을 주입하고 이벤트 1건 배치를 커밋하는 DB 통합 테스트 → 현재 코드는 `Ok`, `domain_events` 0 행, `last_tick` 전진 — 기대(`Err(Fatal)`, 0 행, `last_tick` 불변)와 달라 **RED**. 그 뒤 고친다. 테스트는 "이벤트 1 건이 들어간 배치였다"(입력 ≠ 0)를 함께 단언한다.

### 2.2 같은 모양의 두 번째 경로 — payload 직렬화 실패가 JSON `null` 로 저장된다 (architect 목록에 없던 것)

위치: `lib.rs:432-437`

```rust
fn extract_payload<T: serde::Serialize>(event: &T) -> Value {
    serde_json::to_value(event).ok()
        .and_then(|mut value| value.get_mut("payload").map(Value::take))
        .unwrap_or(Value::Null)
}
```

직렬화가 실패하면 `Value::Null` → `sqlx::types::Json(Value::Null)` 은 SQL NULL 이 아니라 **JSONB `null`** 로 바인딩되므로 `payload JSONB NOT NULL`(`migrations/0001…sql:36`)을 **통과한다.** `MINERAL_MINED` 가 payload 없이 커밋되고, 러너는 그것을 역직렬화하지 못해 정지한다(ADR-0014 §1 fail-stop) — 원인이 영속화인데 증상이 역사에서 난다. ADR-0013 §5 가 "payload 직렬화 실패 = 복구 불가" 라고 적었으므로 S4 가 `Result` 로 바꾼다. 재현: 위와 같은 주입 방식으로는 어렵다(계약 타입 직렬화는 사실상 실패하지 않는다) — **타입으로 막는다**: `contract_payload` 가 `Result<(&str, Value), FatalRecording>` 을 돌려주고 `unwrap_or` 경로를 없앤다. 검증은 코드 검사(해당 `unwrap_or(Value::Null)` 0 건)로.

### 2.3 제약 위반 → "그 배치만 포기하고 계속" — 확인

`lib.rs:250-261`. ADR-0013 §5 대로 정지로 바꾼다(K4·K5). AC-4(f)(sim 기억 우회 → PK 위반 → 정지)가 실행 증거다.

(참고, 결함 아님) `non_null_causation`(`lib.rs:421-430`)은 불변식 위반 시 자기 id 로 대체해 저장한다. `MINERAL_MINED` 에는 이 방식을 **쓰지 않는다** — causation 이 명령 id 여야 AC-18(b) 가 성립하므로, 없으면 복구 불가 기록 실패로 간다.

## 3. 스프린트 계약 server 항목

### 3.1 r0 에 대한 답 (2026-09-27, qa 에 SendMessage 로 전문 송부)

- **그대로 증명 가능**: SC-01~04, 08, 11, 12, 13, 14, 15, 16, 17, 19, 22, 26, 34, 36(§0.5 표 채운 뒤), 38, 39. 테스트 이름은 qa 제안 그대로.
- **r0 가 ADR-0013 개정 전이라 고칠 것**: SC-28·29·31 판정값 `persist_backlog` → `recording_lag`, + halted 거부(K5). AC-6(a2) 한가한 60 초 거부 0 신설. SC-09 사유 2~7(`CAPACITY_EXCEEDED` 경계값 주입 + 카운터), SC-10 쌍 6+7 추가, SC-33 에서 용량 초과 제외. AC-4(d2) 두 테스트(`mining::dup_other_actor_same_command_id`, `persistence::same_command_id_other_world_accepted`). SC-20 은 반환값 `AlreadyCommitted` + 카운터는 별도 `ambiguous_commit_retry_counts_once`(커밋 성공 뒤 Err 주입 래퍼). SC-27 짝 `payload_null_rejected_by_db`(23514 + object 양성 대조). AC-5(e) `no_commit_after_fatal`, AC-5(f) `error_classification`(양방향 SQLSTATE 표). §5a 제약 표 ↔ `pg_constraint` 실측(qa 게이트).
- **방법 조정**: SC-25 단위판 `cas_mismatch_halts` 단언 목록, 실서버판은 fatal 로그 한 줄(사유·키·기대값·DB값·`persist_fatal_total`)이 증거. SC-37 은 required+nullable 필드에 기존 `required_nullable` 을 써야 변이가 실패한다(serde `Option` 은 누락 키를 None 으로 받는다). SC-35 유효 fixture 46.
- **Q-2**: 리더 결정(2026-09-27)으로 대체 — 아래 §3.2 설계. (첫 답이었던 `#[sqlx::test]` 는 DB 가 없으면 무조건 패닉해 "로컬 무DB 건너뜀 + 건수 출력" 을 만들 수 없어 버린다.)
- **Q-5**: 가능. `data::reject_c01_*`~`c16_*` + 무변경 양성 대조 1. 기동 로그 파일마다 한 줄 `"데이터 파일 적재" data_file=<상대 경로>`(10 파일).
- **Q-6**: 가능. 겹침 = 같은 tick 에 (b) 새 ready 세션 ≥ 1 ∧ (c) LIVE ≥ 1. `/debug/stats` `notice_open_overlap_ticks_total`, 단위 테스트 `notice_no_gap_same_tick`(그 세션이 기록을 정확히 1 회 LIVE 로 받음, 카운터 1).
- 상태: r1 수신(2026-09-27).

### 3.3 r1 에 대한 답 (2026-09-27)

server 몫 전부 증명 가능. **서명 조건 2건**:
1. **SC-103 분리** — 실 DB 에서 `pg_terminate_backend` 가 sqlx 에 57P01 로 올지 Io 로 올지 타이밍에 달려 "57P01 ≥ 1" 이 결정적이지 않다. (a) `admin_shutdown_is_transient`: 가짜 `DatabaseError`(SQLSTATE 57P01)를 재시도 루프의 commit 주입점으로 첫 시도에만 → 재시도 커밋·fatal 0, 짝 22P02 → 정지. (b) `terminated_backend_is_retried`: 실제 terminate → 관측 일시 오류 ≥ 1(코드 출력), 같은 배치 커밋, fatal 0.
2. **SC-102** — 스윕은 gateway 런타임이라 persistence 테스트는 "스윕 모양 배치(SESSION_CLOSED ≥ 1)를 fatal 뒤에 → 행 0" 까지. "스윕이 실제로 만들었다" 는 SC-25 실서버 로그 `tick 루프 종료 — SERVER_SHUTDOWN 스윕 후 영속화 flush` 의 `sessions_closed=N ≥ 1` + 정지 tick 이후 행 0.

메모: SC-20 카운터는 재시도 루프 진입점에서 오른다 — `persisted_total == COUNT(*)` 는 `ambiguous_commit_retry_counts_once` 로 따로. SC-101 은 payload 생성 주입점. 0002 제약 이름은 ADR §5a-1 표 그대로(`payload_is_object`, `quantity_kg_check` …). SC-100 테스트 이름은 qa 안 채택. Q-2(testdb, 리더 확정)·Q-5·Q-6 답 재송(r1 과 엇갈림).

- 상태: **r2 에서 2건·메모·Q-2/5/6 반영 확인(SC-103/112 분리, SC-102 짝, `[DB]` 표식 목록 server 12 개) → 2026-09-27 §11 server 확인란 10 행 + "Q-2 헬퍼" 행(SC-114 `refuses_non_test_database`, SC-113 선행 실측) 서명.** 이름 확정: SC-05 대조군 `data::accepts_unmodified_copy`, SC-114 `starfall-testdb::refuses_non_test_database`.
- SC-113 선행 조건 메모: 헬퍼는 `eprintln!` 이 아니라 `std::io::stderr().write_all` 로 직접 쓴다(libtest 캡처는 print 매크로만 가로챈다는 판단 — **미검증**, Phase 4 T0 첫 RED 에서 `--nocapture` 없이 통과 테스트의 RAN 줄이 보이는지 실측해 qa 에 보고).

### 3.2 Q-2 설계 — DB 통합 테스트는 CI 에서 반드시 돌고, 증거 DB 를 건드리지 않는다 (리더 결정 2026-09-27, **이 안으로 확정** — 중간의 "`#[sqlx::test]` 확정" 통보는 리더가 착오로 정정함)

**전제(리더 결정)**: CI `server (rust)` job 에 `postgres:18.6-trixie` 서비스(qa, gates.yml). `#[ignore]` 금지. CI 에서 DB 없음 = 실패, 실행 수 0 = 실패. 로컬 무DB 만 건너뛰되 건수 출력. 로컬 `starfall.domain_events` 행 수가 테스트로 늘면 결함.

**설계 — 테스트 전용 크레이트 `server/crates/testdb`(`starfall-testdb`, server 소유, `publish = false`, dev-dependency 로만 쓴다)**

1. **격리 DB**: `TestDb::create(test_name).await -> Option<TestDb>`
   - 접속 대상 서버는 `DATABASE_URL`(로컬 `.env` 와 같은 값, CI 는 서비스 URL). 그 URL 의 **DB 이름을 무시하고** 관리 DB `postgres` 에 접속해 `CREATE DATABASE starfall_test_<UUIDv7 simple>` → 그 DB 로 새 풀 → `sqlx::migrate!("../../migrations")`(0001~0003 전부 — history 의 0003 도 같이 걸린다).
   - **증거 DB 를 구조적으로 못 건드린다**: 풀을 여는 함수가 하나뿐이고, 그 함수는 DB 이름이 `starfall_test_` 접두사가 아니면 **패닉**한다. 테스트 코드는 `TestDb` 의 풀만 받는다(원본 URL 을 넘기지 않는다).
   - 정리: 테스트 끝에 `db.drop().await`(`DROP DATABASE … WITH (FORCE)`). 패닉으로 남은 것은 **다음 `create` 가 청소**한다 — 이름의 UUIDv7 시각이 **1 시간 넘은** `starfall_test_%` 만 지운다(동시에 도는 다른 테스트 실행의 DB 를 지우지 않게).
   - 권한: `CREATEDB` 필요. docker compose·CI 이미지의 `POSTGRES_USER` 는 superuser 라 충족. 권한 부족이면 아래 모드 규칙대로(필수 모드 = 실패).
2. **모드**: 환경 변수 `STARFALL_DB_TESTS`
   - `required`(CI 가 설정): 접속·생성 실패 → **패닉(테스트 실패)**.
   - 미설정(로컬): 접속 실패 → `None` 반환, 테스트는 즉시 반환. 이때 `STARFALL_DB_TEST SKIPPED <테스트 이름> reason=<오류>` 한 줄.
   - 실행된 테스트는 `STARFALL_DB_TEST RAN <테스트 이름>` 한 줄.
   - **출력 경로**: libtest 는 `print!`/`eprint!` 매크로만 캡처하므로 `std::io::stderr().write_all` 로 직접 쓴다(통과한 테스트의 출력도 보인다). Phase 4 첫 RED 에서 이 출력이 `--nocapture` 없이 보이는지 **실측**하고, 안 보이면 방식을 바꿔 qa 에 알린다.
3. **CI 게이트(qa)**: `STARFALL_DB_TESTS=required cargo test --workspace --locked 2>&1` 에서 `RAN` 줄 수 ≥ 기대 목록 수(계약이 이름으로 지명한 DB 테스트: SC-18·20·21·25·27, d2, 5(e), 5(f), payload_null, ambiguous, history SC-51~57·62), `SKIPPED` = 0. 기대 목록의 이름이 RAN 에 없으면 FAIL(이름 목록 대조 — "개수만 맞음" 방지).
4. **증거 DB 불변 확인(qa 절차, 로컬)**: `cargo test` 전후 `SELECT count(*) FROM domain_events`(starfall DB) 가 같다. ⊘ *DB 테스트가 하나도 안 돌아 당연히 불변* → 같은 실행의 `RAN` ≥ 1 을 함께 찍는다. 양성 대조: 같은 실행 중 `starfall_test_%` DB 가 생겼다 사라졌음(`pg_database` 조회 또는 RAN 줄의 DB 이름).
5. **범위**: 기존 `gateway/tests/live_smoke.rs`(실서버 스모크, `#[ignore]`)는 이 결정의 대상이 아니다 — 실행 중인 서버가 필요한 스모크이고 p1-02 경로 테스트가 아니다. p1-02 의 DB 테스트는 전부 `testdb` 를 쓰며 `#[ignore]` 가 붙은 새 테스트는 0 이다(qa 가 grep 으로 부정 확인 가능, 긍정 증거는 3 의 RAN 목록).
6. **history 도 같은 도구를 쓴다**(H2 의 DB 테스트, `persistence/src/history.rs`). history 수락(2026-09-27).
   - `RAN`·`SKIPPED` 줄은 **헬퍼(`TestDb::create`)만 찍는다** — 테스트 쪽에서 따로 찍지 않는다(이중 줄은 게이트 대조를 흐린다). `create` 의 `name` 인자 = **계약이 지명한 테스트 이름 문자열 그대로**(게이트가 이 문자열로 대조한다).
7. 구현 시점: **T0 골격에 포함**한다 — S4·H2 의 첫 RED 가 이 도구 위에서 써진다.

### 3.0 초안 전 사전 메모 (기록)

미리 qa 에 알릴 것(스펙 AC 기준):

| AC | server 가 증명할 방법 | 주의 |
|---|---|---|
| AC-4(e) | DB 통합: 같은 배치 2회 커밋 → 둘째 `AlreadyCommitted`, 모든 표 행 수 불변, `persist_ambiguous_commits_total == 1` | 둘째가 "건너뜀" 경로를 탔음을 반환값으로 단언(K6) |
| AC-4(f) | DB 통합: sim 기억을 비운 채 같은 command_id 두 배치 → 둘째 `Err(Fatal)`, 인벤토리 불변 | K1 의 PK 모양에 따라 기대가 달라진다 |
| AC-5(b) | e2e: 실제 서버 + SQL 변조 | 종료 코드 ≠ 0 과 로그 사유, `persist_fatal_total ≥ 1` 을 **정지 직전에** 읽어야 한다(프로세스가 사라지면 `/debug/stats` 를 못 읽는다) — **로그 한 줄에 카운터 값을 함께 찍는다** |
| AC-5(d) | DB 통합 + 시계 주입(§2.1) | 실제 서버로는 불가능하다(호스트 시계). 단위/통합 수준으로 계약해 달라 |
| AC-6 | e2e: `docker compose stop/start postgres` | 판정 값은 `recording_lag`(K2). `stop` 이 커밋 도중이면 57P01 — K4 가 없으면 정지로 끝난다 |
| AC-6 ⊘ | — | K2 가 없으면 반대 방향 오류: DB 가 멀쩡한데 거부가 나온다. **"DB 정상 구간 거부 수 = 0"** 도 항목으로 넣자 |

## 4. T0 — history 제안 수락, 조정 3건

history 의 제안(`01_history_review.md` §10)을 거의 그대로 받는다. 우리 둘이 독립적으로 같은 모양(watch 알림, tick 드라이버가 직렬화 문맥, 목록이 `build` 인자)에 도달했다.

| 항목 | 합의안 |
|---|---|
| 의존 | persistence → history(코어) + contracts. gateway → contracts 만. 조립은 game-server |
| 커밋 알림 | `watch::Sender<Option<u64>>` — 영속화가 커밋 성공(건너뜀 포함) 뒤 `send_replace(Some(tick))`. 깨우기일 뿐, 워터마크는 DB `worlds.last_tick`. 러너는 `changed()` ∨ 1 s. 영속화가 끝나면(정상·정지 모두) Sender drop → 러너는 한 번 따라잡고 종료 |
| LIVE 채널 | bounded 256, 러너 → tick 드라이버(`try_recv` 매 tick). **조정 1** 아래 |
| 직렬화 문맥 | tick 드라이버 스레드(routes 소유). 한 tick 안에서 (a) route_outbound 중 SESSION_READY 받은 세션 ready 표시 → (b) 새 ready 세션에 목록 전체 BACKFILL → (c) LIVE 수신분을 목록에 추가하고 ready 세션 전체에 LIVE |
| 기동 | 마이그레이션 → 경제 상태 적재 → `history::load`(판정 상태·PUBLIC 목록·커서, 한 스냅샷) → `runtime::build(…, initial_backfill, live_rx)` → 영속화·러너 spawn → bind |
| 종료 | main 은 persistence 다음에 runner 를 await. 러너의 LIVE 송신이 수신단 닫힘으로 실패하면 **오류가 아니다**(드라이버가 먼저 끝난다) — 정지하지 말고 무시 |
| 관측 | `HistoryHandles`(records, conflicts, halted, cursor_tick, catchup_rows) → `/debug/stats`. 키 추가는 server(S5) |

**조정 1 — 채널 항목은 메시지가 아니라 기록이다.** NOTICE envelope 의 `tick` 은 "보낼 때의 게이트웨이 현재 tick"(계약 description)이고 `message_id` 는 송신마다 새로 만든다. 러너는 그 값을 알 수 없다. 그래서 채널 항목 = `historical_event`(계약의 `MINERAL_DISCOVERED` 레코드 타입), 드라이버가 `delivery`·`tick`·`message_id` 를 붙여 NOTICE 를 만든다. BACKFILL 목록도 같은 레코드 타입. 커버리지 스크립트의 NOTICE 생산자 위치가 gateway 가 되는데, 레코드 타입(`MINERAL_DISCOVERED`)의 생산자는 history 경로에 남는다 — history 확인 요청.

**조정 2 — BACKFILL 64 건 트리거가 송신 큐 용량과 같다.** `SEND_QUEUE_CAPACITY = 64`(`runtime.rs:104`), 포화 시 `SLOW_CONSUMER` 로 연결을 닫는다. 세션 열림 tick 에 SESSION_READY 1 + BACKFILL 64 = 65 → **64 건 트리거에 도달하는 순간 새 접속이 전부 끊긴다.** 이번 슬라이스는 ≤ 4 라 무관하지만 ADR-0014 §6 의 트리거를 **32**(또는 "송신 큐 용량의 절반")로 내리길 architect 에 요청한다. 드라이버에 "BACKFILL 수 + 1 ≤ 용량" 단언과 테스트를 둔다.

**조정 3 — 러너 정지는 서버를 멈추지 않는다.** ADR-0014 §4 대로 러너만 멈추고 경제는 돈다. 영속화의 `fatal` 과 섞지 않는다(별도 핸들).

합의 상태: **합의됨 (2026-09-27, history 가 조정 1~3 수락).**
- 조정 1 의 조건: 레지스트리 `HISTORICAL_EVENT_NOTICE` producer 를 history → server 로 바꾼다(`MINERAL_DISCOVERED` producer 는 history 유지). 거짓 생산(이름만 등장하는 코드)을 막기 위해서다. history 가 architect 에 요청한다 — 계약 변경이므로 S1 전에 반영돼야 한다.
- 조정 2: LIVE send 가 `Closed` 면 러너는 debug 로그만 남기고 계속한다(기록은 이미 커밋, 다음 기동 BACKFILL 로 전달).
- 조정 3: `history_detector_halted`·`history_conflicts_total` 과 `persist_fatal_total` 은 별도 핸들.
- T0 골격의 `load`·`run_runner` 시그니처는 `unimplemented!` 없이 빈 `Ok` 를 돌려 서로 독립 빌드한다(history 제안 (7)).
- `02_interface.md` 는 Phase 4 T0 에서 server 가 쓰고 history 절은 history 가 채운다.

## 5. 요청 정리

| 받는 쪽 | 요청 |
|---|---|
| architect | **완료(전부 수용).** ADR-0013 개정: K1(PK·기억 범위), K2(`recording_lag`), K3(state_writes), K4(일시 실패 허용 목록), K5(정지 뒤 커밋 없음), K6(건너뜀 관측), §3 끝줄(스펙 AC-4(g)와 일치), §7 순서. ADR-0014 §6 BACKFILL 트리거 64 → 32 |
| history | ~~T0 조정 1·3 확인~~ 완료(합의됨). 레지스트리 NOTICE producer 변경은 history → architect 요청 |
| architect | (2차) 계약 `HISTORICAL_EVENT_NOTICE` producer → server **수용**(S1 전 반영). ADR-0013 §5a #4 checked 덧셈 **수용**, 질문 2건: (1) 지금 쓸 기존 `reason_code`, (2) "디버그 panic / 릴리스 거절" 이원화 대신 두 빌드 동일 경로(카운터 + 거절) — 표준 게이트(디버그)가 운영 경로를 실행하게 하려고. **답: (1) 새 사유 `CAPACITY_EXCEEDED`(판정 7단계, RESOURCE_DEPLETED 다음·적용 직전, fixture 1건 추가 → 유효 46), (2) 동의 — panic 없음, 두 빌드 동일(거절 + `commands_rejected_total{CAPACITY_EXCEEDED}`). 수용.** S1 fixture 기대 수는 46 으로 |
| architect | (3차) ADR-0013 §5a 제약 단위 표 — **S4 의 0002 가 표의 제약 집합을 그대로 만든다. 수용, 이견 없음**: `inventory_items` 7(PK (world,actor,mineral), FK, NN 4, CHECK quantity 0..2^31−1), `deposit_states` 10(PK, FK, NN 5 — `first_extracted_tick` 포함(행은 첫 채굴 때만 생기므로 성립), CHECK remaining, CHECK as_of_tick, CHECK first_extracted_tick ≤ as_of_tick), `processed_commands` 8(PK (world,command_id), FK, NN 5, CHECK tick), `domain_events` + `CHECK (jsonb_typeof(payload)='object')`(ALTER 로 추가 — 기존 행 검증이 돌므로 적용 전 실측 4,805 행 전부 object 를 architect 가 확인). 완료 증거 = `pg_constraint` 실측이 표와 같음 |
| qa | §3 표 — 초안 도착 시 항목별 답. "DB 정상 구간 `RECORDING_BACKLOG` 0 건" 항목 추가 제안 |

# server의 스펙·ADR 검토 (p0-02)

- 작성: server (rust-server-engineer), 2026-09-18
- 대상: `docs/specs/p0-02-networking-spike.md`(draft), ADR-0005·0006·0007·0008(proposed), `01_architect_tasks.md` T1~T6
- 이 문서는 **검토와 실측 결과만** 담는다. `server/`·`docs/`·`contracts/`는 이번 턴에 한 글자도 고치지 않았다. 확인용 빌드·DB 탐침은 전부 스크래치에서 했고, DB에 만든 `review_probe` 스키마는 `DROP SCHEMA ... CASCADE`로 지웠다.
- 아래 "실측"으로 표시한 것은 전부 이 PC에서 오늘 실제로 돌린 결과다. 추측은 "미확인"으로 남겼다.

---

## 요약 — 착수 전에 architect 결정이 필요한 것

| # | 항목 | 성격 |
|---|------|------|
| **B-1** | tick 번호가 프로세스 재시작마다 0으로 돌아가 `UNIQUE (world_id, tick, sequence)`가 충돌하고, **그 tick 배치 전체가 유실**된다 | 차단. 설계 결정 필요 |
| **B-2** | 유효 fixture는 **12건**이다(실측). 스펙 4곳이 16건이라고 적고 있다 | 차단. 스펙·AC 정정 |
| **B-3** | 이 PC의 `tokio::time::sleep(50ms)`는 실제로 **61 ms**다(실측). 성능 게이트 3줄 중 2줄이 구현 품질과 무관하게 실패한다 | 차단(환경). 구현 방식 + 게이트 문구 |
| B-4~B-19 | 정의 누락·모순·측정 불가 항목 16건 | 착수 전 한 줄씩 결정하면 되는 것들 |

나머지 큰 결정(20 Hz·거부 정책·`starfall-domain` 미생성·의존성 없는 `/debug/stats`·append-only 지금)은 **전부 동의**한다. 근거는 §1에.

---

## 1. 동의하는 결정

### 1.1 20 Hz 고정 · tick 건너뛰기 없음 · 따라잡기 없음 (ADR-0006 §1·§2)

동의한다. 특히 "따라잡기를 하지 않는다"가 옳다. 따라잡기는 한 번의 지연이 CPU 스파이크를 만들고 그 스파이크가 다음 지연을 만드는 나선을 만든다는 ADR의 서술이 정확하고, `(world_id, tick, sequence)`가 촘촘해야 한다는 요구와도 맞는다.

다만 **"뒤처짐은 숨기지 않고 관측한다"가 이 PC에서는 예외 상황이 아니라 기본 상태**다 — B-3 참조. ADR-0006 §2의 "지속적으로 초과하면 게임 시간이 뒤처진다"는 문단이 corner case가 아니라 **Windows의 기본 동작**임을 알고 구현해야 한다.

### 1.2 큐 포화 시 거부 — 드롭도 블로킹도 아님 (ADR-0006 §5, I-22)

동의한다. 이 슬라이스는 손실을 측정하러 존재하는데 조용한 드롭은 손실의 원인을 지운다. 블로킹은 "클라이언트가 아무 신호도 못 받은 채 느려진다"를 만든다는 지적도 맞다. 거부가 곧 응답이므로 **AC-6(a)/AC-18(a)의 "보낸 명령 수 == 받은 COMMAND_RESULT 수"가 성립**하고, 이것이 이 슬라이스에서 가장 값싸고 강한 불변식이다.

큐 용량 3종에 대한 구체 검토는 §3.

### 1.3 `starfall-domain`을 만들지 않고 세션 모델을 `sim`에 둔다 (T2)

동의한다. "빈 크레이트는 링크 시간만 쓰고 아무것도 증명하지 않는다"는 p0-01의 판단이 그대로 유효하고, `sim`이 이미 IO-free·결정적이라 분리를 미뤄서 망가지는 성질이 없다. 월드 상태가 생기는 p1에 분리하는 것이 맞다.

**단, 조건 하나를 명시해 달라.** 세션 모델에 **송신 채널 핸들(`mpsc::Sender<Message>`)이나 소켓을 넣으면 안 된다.** 가장 자연스러운 구현이 `Session { actor_id, tx: Sender<Message>, ... }`인데, 그 순간 `sim`이 tokio 채널에 묶여 (a) IO-free 주장이 흐려지고, (b) **AC-5(a)의 수동 step 테스트가 런타임 없이 돌지 않는다.**

제안하는 경계:

```
sim 이 소유:      SessionId -> { actor_id, correlation_id, dedup_ring, in_flight }
gateway 가 소유:  SessionId -> mpsc::Sender<Outbound>
tick 의 반환값:   TickOutcome { tick, events: Vec<DomainEvent>, outbound: Vec<(SessionId, Message)> }
```

`sim::step()`이 순수 함수에 가까워지고(입력: 제출 목록, 출력: 이벤트 + 보낼 메시지), gateway가 라우팅만 한다. 이 형태여야 T2 지시의 "이 슬라이스의 아키텍처 전제를 증명하는 단 하나의 테스트"가 런타임·소켓 없이 성립한다.

### 1.4 `/debug/stats`를 의존성 없이 (ADR-0007 §8)

동의한다. `metrics` 크레이트는 그 자체로는 저장소가 없어서 `metrics-exporter-prometheus`를 함께 들여야 하고, 그러면 **스크레이퍼도 없는데 익스포터 트리(hyper 서버 한 벌 더, quanta, ahash…)를 지불**한다. `AtomicU64` 카운터 + 직렬화가 전부인 지금 표면에서는 직접 만드는 쪽이 싸다.

**단, "링 버퍼"는 바꿔 달라.** ADR-0007 §8의 "고정 크기 링 버퍼"로 분위수를 내면 **버퍼 길이 = 관측 창**이 된다. A 단계가 60초 = 1200 tick인데 버퍼가 1024면 p99가 "마지막 51초"의 p99가 되고, 리포트에 "A 단계 p99"라고 적는 순간 거짓이 된다. 고정 버킷 히스토그램(예: 마이크로초 로그 버킷 32개) + `max` + `count` + `sum`이면 **실행 전체**를 덮고 크기는 256바이트다. p1 이후 회귀 기준선으로 쓰려면 창이 고정이어야 한다.

### 1.5 append-only 트리거를 지금 건다 (ADR-0007 §3)

동의한다. 그리고 **이 PC의 PostgreSQL 18.6에서 실제로 동작함을 확인했다**(실측, 스크래치 스키마):

```
UPDATE  -> ERROR: domain_events is append-only (UPDATE)
DELETE  -> ERROR: domain_events is append-only (DELETE)
TRUNCATE-> TRUNCATE TABLE (막히지 않음 — ADR이 적은 대로, 의도)
```

비용은 여섯 줄이 맞고, AC-3이 요구하는 "메시지에 `append-only`가 들어 있다"도 그대로 만족한다.

### 1.6 `query!` 대신 런타임 검사 쿼리 + `migrate!` embed (ADR-0007 §5)

동의한다. `sqlx-cli` 미설치 + 쿼리 3~4종에서 `.sqlx/` 동기화 규율의 비용이 이득보다 크다는 판단이 맞다. feature 조합은 실측했다 — §6 U-4.

### 1.7 인증: 업그레이드 전 검증, 비밀 없으면 `/ws`만 503 (ADR-0008)

동의한다. "프로세스는 살고 인증이 필요한 표면만 죽는다"가 p0-01의 AC-2·AC-3을 깨지 않는 유일한 선택이다. §4의 "없는 것" 표가 이 ADR의 핵심이라는 서술에도 동의한다.

**정정할 사실 하나(좋은 쪽으로):** ADR-0008 "결과" 절의 "HMAC을 위해 작은 의존성(`hmac`, `sha2`)이 추가된다"는 **틀렸다. 추가되는 크레이트는 0개다.** `sqlx-postgres`가 SCRAM 인증 때문에 `hmac`·`sha2`·`subtle`을 이미 의존 트리에 갖고 있다(실측: `probe_ws` → `probe_full` 트리 diff에 hmac/sha2/subtle 없음). 상수 시간 비교도 `subtle::ConstantTimeEq`를 그대로 쓸 수 있다. ADR의 "감수할 점"에서 이 줄을 빼도 된다.

### 1.8 기록 범위 표 (ADR-0007 §1)

동의한다. "명령을 기록하지 않는 것이 이 표의 핵심"이라는 서술이 정확하다. 기록할 것을 억지로 늘리지 않고 세션 회전으로 부하를 만드는 설계도 옳다.

---

## 2. 수정 요청 — 차단 3건

### B-1 (차단) tick 번호가 재시작마다 0으로 돌아가 한 tick 배치 전체가 유실된다

**무엇이 문제인가.** ADR-0006 §2와 스펙 §2-3은 "tick 번호는 **프로세스가 살아 있는 동안** 1씩만 증가"라고 정한다. 어디에도 tick을 영속화하거나 재개하는 규칙이 없고, `worlds` 테이블에도 `last_tick` 열이 없다. 그러면 서버를 두 번째로 기동할 때 tick은 다시 0부터 시작한다. `world_id`는 시드 상수라 같다. 따라서 **두 실행이 같은 `(world_id, tick)`에 이벤트를 쓴다.**

**왜 조용히 넘어가지 않는가 (실측).** 삽입은 `ON CONFLICT (event_id) DO NOTHING`인데, 충돌하는 제약은 `event_id`가 아니라 `UNIQUE (world_id, tick, sequence)`다. `ON CONFLICT`가 그 제약을 가리키지 않으므로 **에러가 나고 트랜잭션 전체가 중단된다.** ADR-0007 §4는 tick 단위 1 트랜잭션이므로, **그 tick의 멀쩡한 다른 이벤트들까지 함께 사라진다.**

```
-- 이 PC PostgreSQL 18.6 에서 실행 (2026-09-18)
BEGIN;
INSERT ... (event_id=...002, world, tick=1, sequence=0) ON CONFLICT (event_id) DO NOTHING;
   -> ERROR: duplicate key value violates unique constraint "de_world_id_tick_sequence_key"
      DETAIL: Key (world_id, tick, sequence)=(01a0b1c2-...-3a4b, 1, 0) already exists.
INSERT ... (event_id=...003, world, tick=9, sequence=0) ON CONFLICT (event_id) DO NOTHING;
   -> ERROR: current transaction is aborted, commands ignored until end of transaction block
COMMIT;  -> ROLLBACK
SELECT count(*) -> 1     -- 두 번째(정상) INSERT 도 함께 날아갔다
```

**무엇이 무효가 되는가.**

- **AC-13**(재연결): "서버를 재시작하고 … DB에 세션 쌍이 2개 생긴다" — 재시작 후 tick이 낮은 값부터 다시 도므로 첫 실행과 겹치면 두 번째 쌍이 통째로 유실된다.
- **AC-7(d)**(정상 종료 후 재기동해 다음 항목 검증), **AC-2**(두 번째 기동), **AC-15(a)(b)**(세션 쌍 수·짝 대조), **AC-16(b)**(5초 내 전 행).
- **ADR-0007 §4의 "`UNIQUE` 위반은 정상 경로가 아니라 버그다"라는 신호 자체가 오염된다.** 재시작하면 항상 나는 에러가 되어, 진짜 버그(서로 다른 이벤트가 같은 자리를 주장)를 구분할 수 없게 된다.
- 충돌 여부가 **연결이 들어온 tick에 달려 있어 비결정적**이다. 어떤 실행은 통과하고 어떤 실행은 일부 행이 비는데, 원인이 "재시작" 쪽이 아니라 "봇 하네스나 네트워크"로 오진될 가능성이 매우 높다. 이 슬라이스에서 가장 비싼 종류의 버그다.

**선택지.**

| | 방식 | 평가 |
|---|------|------|
| (a) | 기동 시 `SELECT COALESCE(MAX(tick)+1, 0) FROM domain_events WHERE world_id = $1`로 tick 재개 | **추천.** 쿼리 1개, 기동 경로에만 있다. `(world_id, tick, sequence)`의 전역 유일성이 월드 수명 전체에서 성립한다. AC-15(c)의 빈틈 검사 SQL은 tick **연속성**을 요구하지 않고 tick 안의 `sequence` 연속성만 보므로 그대로 통과한다 |
| (b) | 모든 측정 실행 전에 프로젝트 범위 `docker compose down -v`로 DB를 비우고, **한 측정은 한 프로세스 안에서 끝낸다** | AC-13·AC-7(d)가 재시작을 요구해서 단독으로는 부족하다. AC-13을 "연결 강제 종료"로 좁히면 가능하지만 AC-2·AC-7(d)가 남는다 |
| (c) | 실행마다 새 `world_id` | `world_id`가 시드 상수라는 전제(AC-2, fixture)와 충돌. 반대 |
| (d) | `UNIQUE (world_id, tick, sequence)` 완화 | 반대. 마지막 방어선이다(I-18) |

**추천: (a) + I-17 문구 보강.** I-17을 "프로세스 생애 동안 정확히 1씩 증가하고, 재기동 시 그 월드에 기록된 마지막 tick 다음부터 재개한다. 뒤로 가지 않는다"로 바꾸면 불변식이 월드 단위가 되어 DB 제약과 의미가 같아진다. 부수 효과로 `occurred_at`이 다운타임만큼 건너뛰는데, 게임 시간이 **뒤로 가지 않는다**는 성질이 유지되므로 원칙 5와 충돌하지 않는다(스펙 §6에 한 줄 기록 제안).

(a)를 받으면 `/debug/stats`에 `start_tick`을 노출해야 한다 — B-11 참조.

### B-2 (차단) 유효 fixture는 12건이다. 16건이 아니다

**실측**(2026-09-18, `contracts/` 현재 상태):

```
COMMAND_RESULT: valid=2 invalid=2
PING_REPLY:     valid=2 invalid=3
PING_SERVER:    valid=2 invalid=4
SESSION_CLOSED: valid=2 invalid=2
SESSION_OPENED: valid=2 invalid=2
SESSION_READY:  valid=2 invalid=2
TOTAL           valid=12 invalid=15
```

스키마 파일은 11개(`registry/types.schema.json` 포함).

**반례 15는 맞다. 유효 16은 틀렸다.** 6타입 × 2 = 12이고, 스펙 §3이 스스로 "타입당 유효 fixture 2 + 반례 2"라고 쓴 것과도 12가 맞다. 16은 "신규 4타입의 유효 8" + "p0-01의 4" 대신 "8 + 8"로 잘못 더한 것으로 보인다.

**고쳐야 할 곳:** §5.3 표(유효 8건 → 총계 표기), §5.4 도입부("유효 fixture 16, 반례 15 … 유효 16건 통과"), **AC-9(a)**, **AC-11(b)**, **AC-11(f)**.

특히 AC-11(f)("유효 fixture를 16건 미만 발견하면 테스트가 실패한다")는 이 상태로 구현하면 **client의 테스트가 영원히 빨간불**이고, 그 원인을 찾는 데 시간이 든다. 그리고 §5.4의 "architect가 유효 16건 통과를 확인했다"는 문장은 존재하지 않는 4건에 대한 확인이라 **증거 문장 자체가 틀렸다** — 이 슬라이스가 "몇 건을 검사했는지가 증거에 드러나야 한다"를 요구하고 있으므로 그냥 넘기면 안 된다.

**부수 작업(T1 내 내 몫):** `server/crates/contracts/tests/contract_tests.rs`의 상수 3개가 p0-01 값 그대로다 — `EXPECTED_SCHEMA_COUNT = 7 → 11`, `EXPECTED_VALID_FIXTURES = 4 → 12`, `EXPECTED_INVALID_FIXTURES = 7 → 15`. 그리고 `SERDE_REJECTION_TABLE`에 신규 반례 8행 추가.

**지금 워크트리는 계약 테스트가 이미 빨간불이다**(코드 읽기로 확정): `registry/types.json`에 server 태그 타입이 6개인데 `CONTRACT_TYPES`에 2개라 `registry_server_types_mapped`가 실패하고, 스키마 개수 단언도 7 ≠ 11로 실패한다. 이것은 버그가 아니라 T1이 아직 없기 때문이고, **QA의 "구현 전 기준선"에 `cargo test`가 포함된다면 그 기준선은 빨간불로 기록되어야 한다.**

### B-3 (차단, 환경) 이 PC에서 `tokio::time::sleep(50ms)`는 61 ms다 — 성능 게이트 2줄이 측정 불가

**실측** (스크래치, tick 본문이 **비어 있는** 순수 타이머 루프):

| 방식 | p50 | p99 | max | 60초 루프의 실제 소요 | >50 ms 비율 |
|------|-----|-----|-----|---------------------|------------|
| `tokio::time::sleep` (멀티스레드 런타임) | **61.2 ms** | 63.6 ms | 66.2 ms | **73.5 s** (lag +13.5 s) | **100 %** |
| `tokio::time::sleep` 재측정 (n=400, 2회) | 61.25 / 61.23 ms | 63.2 / 64.2 ms | 65.1 / 74.8 ms | 24.51 / 24.52 s (이론 20.0 s) | 100 % |
| **전용 OS 스레드 + `std::thread::sleep`** | **50.33 ms** | 51.2 ms | 52.3 ms | 20.14 s (이론 20.0 s, lag +0.7 %) | 100 %* |
| sleep(남은−2 ms) 후 spin | 50.001 ms | 50.26 ms | 51.6 ms | 20.005 s | 99.75 %* |

Windows의 기본 시스템 타이머 해상도 15.625 ms 때문이다. 50 ms 요청이 4 × 15.625 = 62.5 ms로 올림된다. `std::thread::sleep`은 고해상도 대기 타이머를 쓰므로 영향을 받지 않는다. **재현 가능하고 게임 서버 코드와 무관하다.**

**두 가지를 고쳐야 한다.**

**(1) tick 루프를 tokio 태스크가 아니라 전용 OS 스레드 + `std::thread::sleep`으로 돌린다.**
게임 로직을 하나도 넣기 전에 이미 게임 시간이 실제 시간보다 22 % 뒤처지는 서버를 p1의 기준선으로 박아 둘 수는 없다. 이 변경은 §1.3의 경계(`sim`은 동기·IO-free, gateway가 라우팅)와도 자연스럽게 맞고, `timeBeginPeriod` 같은 전역 설정을 건드리지 않으며 새 의존성도 없다(워크스페이스가 `unsafe_code = "forbid"`라 `windows-sys` 경로는 별도 허용이 필요한데, 그럴 필요가 없다). 통신은 `tokio::sync::mpsc`의 `try_send`(게이트웨이 쪽, async) / `try_recv`(tick 스레드 쪽, 동기)로 양쪽 다 블로킹 없이 된다. **이 결정을 ADR-0006 §2에 한 줄로 적어 달라** — 적지 않으면 다음 사람이 "async 서버니까 tokio 타이머"로 되돌린다.

**(2) "tick 초과(>50 ms)"의 정의를 못박는다.**
위 표의 `*`가 보여주듯 **완벽한 타이머에서도 "tick 사이 간격 > 50 ms"는 100 %다.** 간격에는 언제나 오버헤드가 붙기 때문이다. ADR-0006 §7의 `tick_overrun_total`은 "**소요** > 50 ms"이고, 여기서 소요는 `run_tick` **본문**이라고 읽힌다. 스펙 §7 표의 "tick 초과(>50 ms) 비율"은 그 말이 없어 루프 주기로도 읽힌다. 제안:

- 게이트 = `tick_overrun_total / tick_total`, 정의는 **`run_tick()` 본문 소요 시간**. 이것만이 "설계 문제인가"를 답한다.
- 주기 쪽 드리프트는 `tick_lag_seconds`로 **기록만** 한다. 이 PC의 바닥값은 전용 스레드 기준 **+0.7 %/분**이고, 그보다 좋은 값은 나올 수 없다. 리포트에 이 바닥값을 같이 적어야 "0.7 %"가 문제인지 물리인지 구분된다.
- "단일 tick 최대 소요 ≤ 250 ms"도 본문 소요다. 간격으로 재면 GC도 없는 빈 루프가 66 ms를 찍는다.

---

## 3. 수정 요청 — architect가 한 줄씩 결정하면 되는 것

### B-4 큐 포화 시의 `COMMAND_RESULT`를 누가 만드는가 (ADR-0006 §5 ↔ I-13)

`SERVER_BUSY`·`TOO_MANY_IN_FLIGHT`는 **정의상 큐에 넣지 못했을 때** 나온다. 그러니 그 응답을 tick이 만들 수 없다 — 만들려면 큐에 넣어야 하는데 못 넣어서 거부한 것이다. `MALFORMED_COMMAND`(프레임 파싱 실패)도 마찬가지다. 따라서 **게이트웨이가 만든다.** 그러면 게이트웨이가 envelope의 `tick`과 `message_id`를 채워야 한다.

I-13("상태는 tick 안에서만 바뀐다")과 충돌하지 않는다 — 게이트웨이는 **읽기만** 하고 상태를 바꾸지 않는다. 하지만 지금 문서만 보면 T4 구현자가 "게이트웨이가 tick을 안다 = I-13 위반"으로 읽거나, 반대로 아무 생각 없이 `tick: 0`을 넣는다. ADR-0006 §5에 한 줄 요청:

> 큐 포화·프레임 파싱 실패로 큐에 넣지 못한 명령의 `COMMAND_RESULT`는 게이트웨이가 만든다. envelope의 `tick`은 게이트웨이가 공유 원자값으로 읽는 **현재 tick**이고, 이 읽기는 상태 변경이 아니므로 I-13을 위반하지 않는다. `DUPLICATE_COMMAND_ID`는 세션 중복 제거가 `sim`에 있으므로 tick이 만든다.

### B-5 세션 열기/닫기가 명령 큐와 같은 경로로 가면 I-16이 깨진다

ADR-0006 §4의 그림은 "전역 큐에 try_send"만 말하는데, 세션 수락과 세션 종료도 tick에서 처리되어야 한다(스펙 §2-6, §2-11). 이 둘이 **명령과 같은 bounded 4096 큐**를 쓰면, 큐가 가득 찬 순간 `SessionClose`가 거부되고 그 세션은 **`SESSION_OPENED`만 있고 `SESSION_CLOSED`가 없는 상태**로 남는다 — I-16 정면 위반이고, AC-15(b)의 "짝 없는 이벤트 0건"이 부하 상황에서만 깨진다.

요청: **제어 제출(SessionOpen/SessionClose)은 거부 대상이 아니다**를 명시하고, 전용 경로(용량 = 최대 세션 수, 또는 별도 채널)를 둔다. 명령과의 순서는 전역 단조 증가 제출 번호로 여전히 확정된다(제출 번호는 한 곳에서 발급).

### B-6 프레임 상한 초과의 close code가 두 문서에서 다르고, 라이브러리 한도 때문에 셀 수 없다

**모순:** ADR-0005 §2 표는 "프레임 상한 초과 → **1009**"인데, AC-7(a)는 "16 KiB 초과 → 프로토콜 위반으로 **계수**, 10초 창 8회 초과 시 close **1002**"다. 즉시 1009로 끊으면 8회를 셀 수 없고, 8회까지 세면 1009 행이 죽은 규칙이 된다. 하나로 정해 달라(제안: **계수 + 예산 초과 시 1002**로 통일하고 1009 행을 표에서 뺀다. 예산이 있는 편이 "한 번 실수한 클라이언트"와 "프로토콜을 못 지키는 클라이언트"를 구분한다).

**구현 제약(실측 기반):** axum 0.8.9의 `WebSocketUpgrade::max_message_size`/`max_frame_size`를 16 KiB로 설정하면 **tungstenite가 먼저 `Error::Capacity`로 연결을 끊어** 앱이 위반을 셀 기회가 없다. 그러면 AC-7(a)의 "8회"는 영원히 관측되지 않는다. 라이브러리 한도를 **앱 한도보다 높게**(예: 64 KiB) 두고, 16 KiB는 앱 코드가 재조립된 메시지 길이로 판정해야 한다. 조각난 프레임은 axum이 재조립해서 완성된 `Message`로 주므로 ADR-0005 §2의 "재조립 후 크기" 규칙은 자연스럽게 만족된다.

(axum 0.8.9 WS API는 스크래치에서 **컴파일로 확인**했다: 업그레이드 전 `HeaderMap` 추출, `max_message_size`/`max_frame_size`/`write_buffer_size`/`on_failed_upgrade`/`on_upgrade`, `routing::any`, `CloseFrame { code: u16, reason: Utf8Bytes }`, `Message::{Text(Utf8Bytes), Binary, Ping, Pong, Close}` 전부 존재.)

### B-7 `/readyz` 재시도 예산이 두 ADR에서 모순

ADR-0003 §3.1 제약 2 = "점검마다 2초 타임아웃"(현재 코드 `CHECK_TIMEOUT = 2s`). ADR-0007 §9 = "각 점검은 실패 시 즉시 1회 재시도하고 **총 예산 2초는 재시도를 포함**한다". 둘 다 만족하려면 점검당 1초가 되는데, 그러면 살아 있지만 느린 DB(1.5초)가 지금은 통과하다가 앞으로는 1차 실패 후 재시도로 넘어가게 되어 동작이 바뀐다.

**제안:** 재시도를 **타임아웃이 아닌 실패에만** 건다. p0-01이 겪은 실제 실패는 풀에 남은 끊긴 연결의 broken pipe이고, 그건 마이크로초 단위로 즉시 실패한다. "첫 시도가 타임아웃이면 재시도하지 않고 즉시 `unavailable`, 즉시 실패(연결 오류)면 1회만 재시도" — 그러면 총 소요는 사실상 2초를 넘지 않고 ADR-0003의 "점검당 2초"도 그대로다. ADR-0007 §9에 이 조건을 한 줄 추가해 달라.

### B-8 `SERVER_BUSY`는 30 연결에서 구조적으로 도달 불가능하다

세션 in-flight 상한이 64이므로 한 세션이 전역 큐에 동시에 올릴 수 있는 명령은 최대 64개다. 30 세션 × 64 = **1920 < 4096**. 즉 **이 슬라이스의 부하 실행에서 전역 큐는 절대 차지 않고, `SERVER_BUSY`는 한 번도 나오지 않는다.** AC-18(a)의 "`SERVER_BUSY` 또는 `TOO_MANY_IN_FLIGHT`"는 실제로는 언제나 후자다.

문제될 것은 없지만 **리포트가 "SERVER_BUSY 경로를 검증했다"고 읽히면 안 된다.** 둘 중 하나로:

- (a) 4096을 유지하고(=64 세션까지의 여유), `SERVER_BUSY` 경로는 **단위 테스트로만** 덮으며 QA 리포트에 "부하 실행으로는 미도달(구조적)"이라고 적는다 — **추천**. `contracts/fixtures/COMMAND_RESULT/rejected-server-busy.json`이 이미 그 모양을 고정하고 있으므로 계약 쪽은 덮인다.
- (b) 전역 용량을 1024로 낮춰 도달 가능하게 만든다. 30×64=1920 > 1024라 폭주 봇 + 정상 봇 조합에서 실제로 나온다. 다만 "정상 부하에서 SERVER_BUSY가 나오면 원인을 먼저 본다"는 ADR 문장과 부딪힐 여지가 있다.

어느 쪽이든 **ADR-0006 §5의 "용량 숫자는 잠정값"에 위 산수를 근거로 남겨 달라** — 첫 측정 후 조정할 때 `command_queue_depth` p99가 낮게 나오는 이유가 "부하가 약해서"가 아니라 "in-flight 상한이 먼저 걸려서"임을 알아야 한다.

### B-9 `ws_connections == opened − closed`는 항등식이라 아무것도 증명하지 않는다

AC-8(b)의 첫 항목은 세 값이 모두 같은 증감 지점(`sessions_opened_total++` / `sessions_closed_total++` / `ws_connections±`)에서 나오면 **코드가 어떻게 틀려도 항상 참**이다. 교차 검증이 되려면 출처가 독립이어야 한다.

요청: `ws_connections`를 **세션 레지스트리의 실제 길이**(`registry.len()`)로 계산한다고 명시. 그러면 이 항등식은 "레지스트리에서 지워졌는데 closed 카운터가 안 올랐다" 같은 실제 버그를 잡는다. ADR-0007 §8의 "서로 독립적인 세 출처가 같은 수를 말해야 한다"는 의도가 그때 비로소 성립한다.

### B-10 손실 0을 **서버 쪽에서** 증명할 메트릭이 없다

ADR-0006 §7 + T5 지시의 메트릭 목록에 **보낸 메시지 수가 없다**. 지금 목록으로는 `commands_received_total`까지만 있고, 그래서 AC-14(b)의 "손실 0"은 **봇의 관측만으로** 판정된다. 봇이 3,600건 보내고 3,598건 받았을 때, 서버가 3,600건을 만들었는데 소켓에서 샌 것인지 서버가 2건을 안 만든 것인지 구분할 수단이 없다. T14가 "3자 대조가 이 슬라이스의 핵심 증거"라고 하는데 명령 경로에는 3자가 없다.

요청: `messages_enqueued_total{message_type}`(송신 큐에 넣음) + `messages_written_total{message_type}`(소켓에 실제로 씀) 2종 추가. 두 값의 차이가 곧 "큐에 남은 것"이고, `commands_received_total − messages_enqueued_total{COMMAND_RESULT} = 0`이 **서버 내부의 1:1 불변식**이 된다.

### B-11 `tick_skipped_total`은 설계상 항상 0인 항진명제

AC-5(b)는 "`tick_skipped_total`이 0"을 요구하는데, ADR-0006 §2가 tick을 건너뛰지 않기로 정했으므로 이 값은 **상수 0을 반환하는 코드**가 된다. 검증되는 것이 없다.

요청: I-17을 실제로 검증하는 항등식으로 바꾼다. `/debug/stats`에 `tick`(현재), `start_tick`(이번 프로세스가 시작한 tick — B-1 (a)를 받으면 0이 아니다), `tick_total`을 노출하고 **`tick_total == tick − start_tick + 1`**을 AC로 삼는다. 이건 루프가 어딘가에서 tick을 두 번 올리거나 건너뛰면 즉시 깨진다.

### B-12 in-flight를 언제 해제하는지가 정해지지 않았다 — AC-6(c)의 도달 가능성이 여기 달려 있다

ADR-0006 §5는 "세션 in-flight(**미응답** 명령) 64"라고만 쓴다. 두 해석이 가능하고 결과가 다르다.

- **판정 시점에 해제**(= `COMMAND_RESULT`를 만든 tick에서): 클라이언트가 읽지 않아도 계속 보낼 수 있으므로 송신 큐(256)가 찬다 → **AC-6(c)의 `SLOW_CONSUMER`가 도달 가능**하다. 전역 큐 기여도 상한 1920 산수(B-8)도 이 해석에서 성립한다.
- **클라이언트가 받은 시점에 해제**: 읽지 않는 클라이언트는 64에서 막혀 응답 메시지를 최대 128개밖에 못 만든다. **128 < 256이므로 송신 큐는 절대 차지 않고, AC-6(c)는 어떤 클라이언트 동작으로도 재현 불가능하다.**

요청: ADR-0006 §5에 "in-flight는 서버가 그 명령의 `COMMAND_RESULT`를 **만든 시점**에 해제한다(전달 확인이 아니다)"를 명시.

### B-13 `COMMAND_RESULT`/`PING_REPLY`의 `correlation_id`를 서버가 무엇으로 채우는지 정해지지 않았다

스펙 §5.2 본문은 "`COMMAND_RESULT`·`PING_REPLY`에서 `null`일 수 있다"까지만 말하고, 같은 표의 `correlation_id` 행은 "세션 단위로 하나. `SESSION_OPENED`·`SESSION_CLOSED`·`SESSION_READY`가 공유한다"로 **세 타입만** 나열한다. fixture는 두 모양을 다 갖고 있다(`accepted.json`은 세션 correlation, `rejected-server-busy.json`은 `null`). 널 가능 필드의 스키마 커버리지로는 정당하지만, **서버가 실제로 무엇을 넣는지는 어디에도 없다.**

이게 비면 (a) 클라이언트가 correlation으로 세션을 묶으려다 절반만 묶이고, (b) AC-20 경계면 교차 검증에서 "3자 동일"을 판정할 기준이 없다. 스펙 §5.2에 한 줄 요청. **추천은 "항상 `null`"** — 스펙이 이미 "명령은 게임플레이 트랜잭션을 시작하지 않았다"고 근거를 대고 있고, I-12가 경고하는 "클라이언트가 서로 무관한 트랜잭션을 같은 correlation으로 묶는" 여지도 남지 않는다. 명령이 상태를 바꾸는 p1에 진짜 트랜잭션 correlation이 들어올 자리를 비워 두는 편이 깨끗하다.

### B-14 `SESSION_READY/tick-zero.json`이 I-19와 어긋난다

이 fixture는 `world_id = 01a0b1c2-3d4e-7f01-8a2b-9c0d1e2f3a4b`(스파이크 월드)에 `tick_hz: 1`을 쓴다. `basic.json`은 같은 `world_id`에 `tick_hz: 20`이다. I-19는 "파생 상수는 `worlds` 행에 있고 **그 월드의 수명 동안 불변**"이라고 못박았으므로, **같은 월드가 두 개의 `tick_hz`를 갖는 fixture 집합**은 불변식과 모순된다. AC-20(경계면 교차 검증)이 fixture 값을 월드 상수와 대조하면 걸린다.

요청: `tick-zero.json`의 `world_id`를 다른 UUIDv7로 바꾼다(경계값 `tick: 0`·`tick_hz: 1` 커버리지는 그대로 유지된다).

### B-15 AC-15(a)의 "180"은 이 PC에서 맞는 수가 아니다

전수 `select count(*) ... where event_type='SESSION_OPENED'`는 **DB에 쌓인 모든 실행의 합**을 센다. C 단계(30 세션 더), AC-17의 Unity(1), AC-12·AC-13의 단독 왕복, 그리고 이전 실행분이 전부 섞인다. 180이 맞으려면 "실행 직전에 DB를 비웠고 A·B 외에는 아무 세션도 열리지 않았다"가 전제인데, 그 전제는 AC 어디에도 없고 AC-3(append-only 탐침)이 넣은 행도 섞인다.

**정확하고 DB 청소와 무관한 대안:** 봇이 `SESSION_READY`를 받을 때 **envelope의 `correlation_id`와 payload의 `session_id`를 기록**한다(둘 다 이미 클라이언트에게 전달된다). QA는 그 집합으로 대조한다.

```sql
select count(*) from domain_events
where event_type = 'SESSION_OPENED' and correlation_id = any($1::uuid[]);
```

이러면 (a) "몇 건을 검사했는지"가 배열 길이로 정확히 드러나고, (b) 다른 실행·다른 클라이언트가 섞여도 무해하며, (c) AC-15(b)의 짝 대조도 같은 집합 안에서 닫힌다. AC-15(a)(b)의 문구를 "관측한 세션 수(=봇이 수집한 correlation_id 개수)"로 바꿔 달라.

### B-16 AC-3의 탐침 행이 AC-15(c)의 빈틈 검사를 깨뜨릴 수 있다

AC-3은 `domain_events`에 `ON CONFLICT` 재삽입과 `UNIQUE` 위반 시도를 하면서 **실제 행을 남긴다.** 그 행이 임의의 `(world_id, tick, sequence)`를 쓰면 AC-15(c)의 `count(*) <> max(sequence)+1` 검사가 그 tick에서 실패한다.

요청: AC-3에 "탐침 행은 이 실행이 쓰지 않는 전용 `tick` 값을 쓰고 `sequence`는 0부터 연속으로 넣는다"를 한 줄 추가. 한 행만 넣으면 `count=1, max(seq)+1=1, min=0`으로 검사를 그대로 통과한다.

### B-17 AC-7(d)의 Ctrl-C를 이 PC에서 보낼 방법이 정해져 있지 않다

Windows에는 SIGTERM이 없다. `taskkill /PID`(/F 없이)는 콘솔 앱에 닿지 않고, `/F`는 하드 킬이라 graceful shutdown 경로를 **전혀 타지 않는다**. 에이전트가 백그라운드로 띄운 서버 프로세스에는 CTRL_C_EVENT를 보낼 표준 경로가 없다(`GenerateConsoleCtrlEvent`는 같은 콘솔 그룹에만 닿는다).

즉 **AC-7(d)는 판정 방법이 없으면 "하드 킬 후 실패"로 오판정되거나 조용히 건너뛰어진다.** 그리고 하드 킬은 ADR-0007 §6이 기록한 "outbox 없음으로 인한 손실"을 그대로 재현하므로, 결과가 "graceful shutdown이 깨졌다"로 보인다.

선택지: (a) 사람이 실제 터미널에서 Ctrl-C를 누르고 터미널 전사를 증거로 남긴다(가장 정직, 재현은 수동), (b) 서버가 stdin에서 한 줄(`shutdown`)을 읽으면 Ctrl-C와 **같은 종료 경로**를 타게 한다 — 새 네트워크 표면이 없고 파이프로 띄운 프로세스에 그대로 쓸 수 있다, (c) `tokio::signal::windows::ctrl_break`를 함께 듣고 QA가 `GenerateConsoleCtrlEvent`를 P/Invoke로 호출한다(가장 번거로움).

**추천 (b) + (a) 병행.** (b)는 구현 5줄이고, 이것이 없으면 AC-7(d)는 이 PC에서 자동 검증이 불가능하다. 다만 (b)는 스펙 범위 밖 기능 추가이므로 **architect 승인 없이 넣지 않는다.**

### B-18 "이벤트를 버리지 않는다"의 실제 한계가 적혀 있지 않다

ADR-0007 §4는 영속화 채널이 포화하면 tick 루프가 "버리지 않고 제출을 미룬다"고 하고, 600 tick을 넘으면 새 연결 수락을 중단한다고 한다. **구현은 가능하다** — `AtomicU64 persist_backlog`를 tick 스레드가 갱신하고 `/ws` 핸들러가 업그레이드 전에 읽어 503을 주면 된다(인증 검사와 같은 자리, 분기 하나). 어려운 곳이 없다.

문제는 **그 다음이 없다는 것**이다. DB가 영영 돌아오지 않으면 미룬 이벤트가 메모리에 무한히 쌓인다. B 단계 회전 부하(초당 수십 건, 이벤트당 수백 바이트)면 분당 수 MB 수준이라 몇 시간은 버티지만, "버리지 않는다"가 곧 "언젠가 OOM"이라는 사실은 기록되어야 한다.

요청: ADR-0007 §4에 "백로그는 상한이 없다. DB가 복구되지 않으면 메모리가 계속 는다. 이 슬라이스는 그 한계를 감수하고 `persist_backlog`로 관측만 한다"를 명시. 그리고 **503 응답 본문을 정해 달라** — 지금 `/ws`의 503은 `{"status":"unavailable"}`(비밀 미설정)뿐이라, 같은 본문을 쓰면 "인증 미설정"과 "기록 백로그 초과"를 구분할 수 없다. 같은 본문으로 통일하든 다르게 하든 한 줄이면 된다.

덧붙여: **이 규칙을 검증하는 AC가 하나도 없다.** 매직 넘버 600이 들어간 죽은 코드가 된다. 검증은 쉽다 — A 단계 도중 `docker compose stop postgres` → 30초 후 새 연결이 503, 기존 세션은 유지 → `docker compose start postgres` → 백로그가 빠지고 **한 건도 잃지 않고** 전부 DB에 들어온다. 이게 "기록 시스템이 기록을 버리지 않는다"의 유일한 실증이고, Phase 0 종료 기준의 "실시간 이벤트 기록"에 정확히 해당한다. AC 추가를 제안한다(qa 부담 증가는 5분 수준).

### B-19 AC-2의 "볼륨 없는 상태"는 지금 참이 아니고, 마이그레이션 재실행 절차가 없다

- **실측:** `starfall_postgres-data` 볼륨이 **존재**한다(p0-01에서 생성). 다른 프로젝트 볼륨 90여 개가 같이 있고 컨테이너 5개가 돌고 있다. AC-2의 Given을 만족시키려면 레포 루트에서 `docker compose down -v`가 필요하고, `docker-compose.yml`의 `name: starfall` 덕분에 **이 명령은 starfall 프로젝트에만 작용한다**(안전). 스펙 §7 서두에 이 명령을 명시해 달라 — "전역 정리 금지"만 적혀 있어서 구현자가 볼륨 삭제 자체를 피할 위험이 있다.
- **실측:** 포트 8080은 현재 **비어 있다**. 다른 프로젝트 컨테이너는 4222/8222/9200/6333-6334/9001-9002를 쓰고 8080과 겹치지 않는다.
- **없는 절차:** `sqlx::migrate!`는 적용된 마이그레이션의 체크섬을 `_sqlx_migrations`에 저장한다. 개발 중 `0001_*.sql`을 한 글자라도 고치고 다시 기동하면 **"previously applied but has been modified"로 기동이 실패**한다. 기존 볼륨이 있는 지금 상태에서는 T3 착수 직후에 바로 겪는다. 스펙이나 T3 지시에 "마이그레이션을 고친 뒤에는 `docker compose down -v && docker compose up -d`"를 한 줄 적어 두면 한 시간을 아낀다.

---

## 4. 수용 기준 검토 — 측정 가능한가

"30 연결 성공"과 정확성 게이트를 내 구현이 실제로 증명할 수 있는지 항목별로.

| AC | 측정 가능? | 비고 / 문구 제안 |
|----|-----------|-----------------|
| AC-1 게이트 | **예** | 지금 워크트리는 이미 빨간불이다(B-2). `[lints] workspace = true` 옵트인은 신규 2크레이트에 반드시 넣고 T6에 적는다 |
| AC-2 마이그레이션 | **예, 단 B-19** | "볼륨 없는 상태"를 만드는 명령을 적어야 한다. `tick_hz` 불일치 기동 거부는 쉽다 |
| AC-3 append-only | **예 (실측 확인)** | B-16(탐침 행 위치) 한 줄 추가 필요 |
| AC-4 인증 | **예** | (c)의 "같은 프로세스에서 `/healthz` 200, `/readyz` 200"이 좋은 항목이다. 현재 구조로 그대로 성립한다 |
| AC-5 tick 판정 | (a) **예**, (b) **아니오**, (c) **예** | (b)의 `tick_skipped_total == 0`은 항진명제 → B-11의 항등식으로 교체. (a)는 §1.3의 경계(반환값 기반)를 지켜야 런타임 없이 돈다 |
| AC-6 큐·거부 | (a) **예**, (b) **예**, (c) **B-12에 달림** | (c)는 in-flight 해제 시점이 "판정 시점"일 때만 재현 가능 |
| AC-7 프레이밍·수명 | (a)(b) **B-6 해결 후**, (c) **예**, (d) **B-17 미해결 시 불가**, (e) **예** | |
| AC-8 운영 표면 | (a) **예(B-7 정리 후)**, (b) **현재 문구로는 항진** | B-9로 `ws_connections`를 레지스트리 길이로 |
| AC-9 계약 테스트 | **예** | 건수는 **12/15/11**이다(B-2). 테스트 출력에 건수를 찍는 구조는 p0-01에 이미 있다 |
| AC-14 정상 상태 30 | (a)(b)(c)(d) **예**, (e) **B-11** | (b)(c)(d)는 봇 단독 관측으로 성립한다. 서버 쪽 대조가 되려면 B-10 |
| AC-15 기록 무결성 | **B-1 해결 전에는 불가** | (a)의 "180"은 B-15로 교체. (c)는 B-1·B-16 해결 후 유효. (d) `occurred_at` 재계산은 **공식이 맞음을 실측 확인**(아래) |
| AC-16 실시간성 | (a) **예**, (b) **예, 단 측정 방법 지정 필요** | (b)를 `recorded_at`과 호스트 시계 비교로 재면 **컨테이너 시계 차이**가 섞인다. 호스트에서 `count(*)`를 폴링해 "봇 종료 후 몇 초 만에 기대 행 수에 도달했는가"로 재야 한다 |
| AC-18 백프레셔 격리 | (a) **예(단 SERVER_BUSY는 미도달, B-8)**, (b) **예**, (c) **예** | |
| AC-20 경계면 교차 | **예** | B-13(correlation 규칙)·B-14가 먼저 정리돼야 "3자 동일" 판정 기준이 생긴다 |

### `occurred_at` 파생 규칙 — 실측으로 확인함

ADR-0006 §3 공식 `game_seconds = (tick / tick_hz) * calendar_scale`(정수 나눗셈), `occurred_at = calendar_epoch + game_seconds`를 `tick_hz=20, calendar_scale=60, epoch=3800-01-01T00:00:00Z`로 계산한 결과가 **fixture 3건과 문자열까지 일치**한다(이 PC PostgreSQL에서 계산):

| tick | 계산값 | fixture |
|------|--------|---------|
| 0 | `3800-01-01T00:00:00Z` | — |
| 1, 19 | `3800-01-01T00:00:00Z` | (정수 나눗셈으로 20 tick이 같은 초에 매핑 — ADR이 적은 손실, 의도대로) |
| 1200 | `3800-01-01T01:00:00Z` | `SESSION_OPENED/basic.json` ✓ |
| 24000 | `3800-01-01T20:00:00Z` | `SESSION_CLOSED/client-closed.json` ✓ |
| 86400 | `3800-01-04T00:00:00Z` | `SESSION_OPENED/sequence-nonzero.json`, `SESSION_CLOSED/idle-timeout.json` ✓ |

**공식·상수·fixture 사이에 어긋남 없음.** 구현 시 주의할 것 두 가지만 기록한다.

1. `tick_hz`로 **먼저** 나눈다(ADR이 강조한 대로). `tick * calendar_scale / tick_hz`로 쓰면 같은 값이 나오지만 tick이 클 때 `u64` 곱이 넘칠 수 있고, 무엇보다 두 축의 분리라는 의도가 코드에서 사라진다.
2. 날짜 산술에 `chrono`를 쓸 필요가 없다. epoch 이후 초 → `YYYY-MM-DDTHH:MM:SSZ` 변환은 civil-from-days 15줄이면 되고, 그러면 `starfall-sim`이 **의존성 0개**로 남는다(스펙 §8의 "Cargo.toml 의존성 목록으로 검증 가능"이 더 강해진다). 같은 헬퍼를 `recorded_at`(RealTime) 포맷에도 재사용할 수 있다.

### envelope 서버 채움 규칙 — 계약과 어긋나지 않음 (확인)

`message-envelope.schema.json`에는 `sequence`·`world_id`·`occurred_at`·`recorded_at`이 **없다.** 스펙 §5.2 표가 이 필드들을 "서버가 채움"으로 한데 묶어 놓아 "서버 메시지에도 `sequence`를 붙이는가?"로 읽힐 수 있는데, 계약상 붙일 자리가 없다. 이게 중요한 이유: **메시지가 `sequence` 공간을 같이 쓰면 `domain_events`의 `sequence`에 구멍이 생겨 AC-15(c)가 항상 실패한다.** 현재 계약은 그 위험이 없다. T2 구현자가 착각하지 않도록 §5.2 표에 "`sequence`는 도메인 이벤트 전용. 서버 메시지 envelope에는 존재하지 않는다"를 한 줄 넣어 주면 좋겠다.

---

## 5. 내 태스크 실행 계획

권장 순서(T1 → T3 → T2 → T4 → T5 → T6)에 동의한다. 마이그레이션을 먼저 세우면 tick 루프 출력이 갈 곳이 생긴다는 근거가 맞다.

| 순서 | 태스크 | 예상 | 선행 차단 | 주요 위험 |
|-----|-------|------|----------|----------|
| 1 | **T1** 계약 타입 4종 + 테스트 상수/거부표 갱신 | 1.5~2 h | **B-2**(건수), B-13·B-14(선택) | 닫힌 열거형 4종의 `SCREAMING_SNAKE_CASE` 왕복. `actor_id` 비-`Option` 좁힘이 `required_nullable`과 섞이지 않게. 건수 상수 3개를 실제 파일 수로 맞추고 테스트 출력에 찍기 |
| 2 | **T3** 마이그레이션 0001 + `starfall-persistence` | 2.5~3 h | **B-1**(tick 재개 여부가 기동 경로를 바꾼다), B-19 | 마이그레이션 `.sql`을 LF로 저장하고 바이트 확인. build.rs(§6 U-4 실측). tick 단위 트랜잭션에서 `UNIQUE` 위반을 삼키지 않기. `recorded_at`을 `now()`로 할지 Rust에서 만들지 결정(§6) |
| 3 | **T2** `starfall-sim` | 3~4 h | **B-3**(전용 스레드), B-4·B-5·B-12 | `sim`에 의존성 0 유지. 수동 step 테스트가 런타임 없이 돌 것. 제출 번호 발급 지점을 한 곳으로. dedup 링에 "진짜 멱등성 아님" 주석 |
| 4 | **T4** 게이트웨이 + 인증 + 프레이밍 | 3~4 h | **B-6**(close code + 라이브러리 한도) | 라이브러리 한도 64 KiB / 앱 한도 16 KiB. 15 s ping / 30 s idle을 `select!`로. 자격 증명 추출을 한 함수로. `SESSION_READY`가 첫 계약 메시지임을 구조로 보장(게이트웨이는 tick이 돌려주기 전까지 아무것도 보내지 않는다) |
| 5 | **T5** 조립 + `/debug/stats` + `/readyz` 재시도 + graceful shutdown | 2~2.5 h | B-7, B-9·B-10·B-11, **B-17** | 종료 순서: 수락 중단 → 다음 tick에서 전 세션 `SERVER_SHUTDOWN` → 영속화 flush 대기 → 종료. `/debug/stats` 히스토그램 |
| 6 | **T6** 구현 요약 + 실측치 | 1 h | — | 클린 빌드 시간, U-3·U-4, lints 옵트인, build.rs 방식, 계약 테스트 검사 건수, 스펙에서 틀렸다고 본 것 |

합계 **13~17 시간**(에이전트 작업 기준). B-1·B-3의 답이 T3·T2 시작 전에 필요하다. B-2는 T1 시작 전에 필요하다.

**착수 전 내가 먼저 할 일:** 없음 — U-3·U-4를 이 검토에서 이미 실측했다(§6). T3 첫 작업은 마이그레이션 SQL 작성으로 바로 들어간다.

---

## 6. 이 PC에서 판정이 무효가 되는 조건

p0-01에서 2건 찾았던 자리다. 이번에 찾은 것은 **9건**이고, 이 중 3건은 "검사는 통과하는데 아무것도 증명하지 않는" 종류다.

| # | 조건 | 어떻게 무효가 되는가 | 방어 |
|---|------|---------------------|------|
| **1** | **Windows 타이머 해상도 15.625 ms** (실측: tokio sleep 50 ms → 61 ms, 60초 루프가 73.5초) | 성능 게이트 2줄이 **빈 루프에서도** 실패한다. "tick 초과율 100 %"를 보고 설계를 의심하게 된다 | 전용 OS 스레드 + `std::thread::sleep`(실측 p50 50.3 ms). 게이트 정의를 `run_tick` 본문 소요로 못박기 (B-3) |
| **2** | **다른 앱이 전역 타이머 해상도를 올리면 결과가 달라진다** | Chrome·Unity Editor가 떠 있을 때와 아닐 때 tokio 기반 루프의 수치가 달라져 **재현 불가능한 벤치마크**가 된다. AC-17(부하 중 Unity)이 정확히 그 조건이다 | 위와 같음. `std::thread::sleep`은 전역 설정과 무관하다. 리포트에 "측정 중 Unity Editor 실행 여부"를 반드시 기록 |
| **3** | **tick 리셋 + 기존 볼륨** (실측: `starfall_postgres-data` 존재) | 재시작이 섞인 실행에서 **tick 배치가 통째로 사라지고**, 원인이 봇·네트워크로 오진된다. 충돌 여부가 연결 타이밍에 달려 비결정적 | B-1 (a) tick 재개. 그 전까지는 모든 측정 실행 앞에 프로젝트 범위 `docker compose down -v` |
| **4** | **전수 `count(*)` 기반 판정** | DB가 실행마다 누적되므로 AC-15(a)의 "180"은 두 번째 실행부터 절대 맞지 않는다. 맞으면 오히려 이상하다 | B-15: 봇이 수집한 `correlation_id` 집합으로 대조. 검사 건수가 배열 길이로 드러난다 |
| **5** | **항등식·항진명제 검사** | `ws_connections == opened − closed`(B-9)와 `tick_skipped_total == 0`(B-11)은 **코드가 어떻게 틀려도 통과**한다. "20개 AC 중 2개는 항상 초록"인 상태로 Phase 0을 통과하게 된다 | 출처 독립화(레지스트리 길이) + 항등식 교체(`tick_total == tick − start_tick + 1`) |
| **6** | **Windows에 SIGTERM이 없다** | AC-7(d)를 에이전트가 하드 킬로 대체하면, outbox 없음으로 인한 손실이 나타나고 결과가 "graceful shutdown 버그"로 보인다. 또는 조용히 건너뛰어진다 | B-17. 판정 방법을 스펙에 적기 |
| **7** | **새 마이그레이션 파일 추가 시 재컴파일되지 않는다** (실측) | `0002_*.sql`을 추가해도 바이너리에 안 들어가고, "마이그레이션을 썼는데 테이블이 없다"가 된다. **기존 파일 수정은 재컴파일된다** — 그래서 더 헷갈린다 | `build.rs`에 `cargo:rerun-if-changed=migrations`(실측으로 해결 확인) |
| **8** | **마이그레이션 체크섬** | `0001_*.sql`을 고치고 기존 볼륨에 다시 기동하면 **기동 자체가 실패**한다. T3 착수 직후 바로 겪는다 | B-19. 수정 후 `down -v` 절차를 적어 두기 |
| **9** | **컨테이너 시계 vs 호스트 시계** | `recorded_at`을 `now()`(Postgres)로 채우고 호스트 시계와 비교해 AC-16(b)의 5초를 재면, WSL2 절전 복귀 후 시계 차이가 결과에 섞인다 | AC-16(b)는 호스트에서 `count(*)` 폴링으로 측정. `recorded_at`을 호스트 시계로 맞춰야 하면 Rust에서 포맷(civil-from-days 재사용) |

포트 8080은 **현재 비어 있다**(실측). 다른 프로젝트 컨테이너 5개와 포트 충돌 없음. 이것만은 p0-01과 달리 문제가 아니다.

---

## 7. 확인한 사실 / 미확인

### U-3 — axum 0.8.9 `ws` feature의 의존 트리와 빌드 시간 (**확인**)

스크래치에 현재 `server/Cargo.toml`과 같은 feature 조합의 최소 크레이트를 만들어 재고, `ws`만 켜서 다시 쟀다.

**추가되는 크레이트 8개:**
`tokio-tungstenite 0.29`, `tungstenite 0.29`, `sha1 0.10.7`, `data-encoding 2.11.1`, `rand 0.9.5`, `rand_chacha 0.9.0`, `rand_core 0.9.5`, `getrandom 0.3.4`
(normal edge 기준 유니크 크레이트 153 → 161)

**클린 빌드**(dev, `debug = "line-tables-only"` 동일 적용, 단일 바이너리 크레이트):

| 구성 | 클린 빌드 |
|------|----------|
| 현재 server와 같은 feature | **35.6 s** |
| `ws` + sqlx(`macros`,`migrate`,`uuid`,`json`,`chrono`) + hmac/sha2/subtle | **38.4 s** |

**증가분 약 +2.8 s (+8 %).** p0-01의 26초는 워크스페이스 3크레이트 전체(jsonschema 포함) 기준이라 직접 비교할 수 없지만, **feature 추가로 인한 의존성 컴파일 증가는 3초 안쪽**이다. 우려할 수준이 아니다. 실제 워크스페이스 클린 빌드 시간은 T6에서 측정해 26초와 비교한다.

**부수 관찰(결정성 위생):** `ws`가 `rand`를 의존 트리에 끌어들인다(tungstenite의 마스킹 키). `starfall-sim`은 `axum`을 의존하지 않으므로 `rand`에 손이 닿지 않는다 — 스펙 §8의 "`sim`의 Cargo.toml에 `sqlx`·`axum`·`redis`가 없어야 한다"가 결과적으로 `rand`까지 막는다. 워크스페이스 `[workspace.dependencies]`에 `rand`를 **추가하지 않는 것**을 규칙으로 유지하자고 제안한다.

### U-4 — sqlx 0.8.6의 `migrate!` feature 조합 (**확인**)

실제로 컴파일해서 확인했다.

| feature 조합 | 결과 |
|-------------|------|
| `runtime-tokio, postgres, migrate, uuid, json` | **컴파일 실패**: ``cannot find `migrate` in `sqlx` `` |
| `runtime-tokio, postgres, macros, migrate, uuid, json` | **성공** (16.8 s) |

**ADR-0007 §5의 "`macros`와 `migrate`를 요구한다"는 정확하다.** 추가로 확인한 것:

- `uuid` feature로 `Uuid` 바인딩, `json` feature로 `sqlx::types::Json` / JSONB 바인딩, `i64`로 `BIGINT` 바인딩이 전부 된다.
- **`chrono`·`time` feature는 필요 없다** — `recorded_at`을 SQL의 `now()`로 채우면 시간 타입을 Rust로 주고받을 일이 없다. 트레이드오프는 §6-9(컨테이너 시계)다. 대안은 civil-from-days 헬퍼로 Rust에서 `RealTime` 문자열을 만드는 것이고, 이 헬퍼는 `occurred_at` 때문에 어차피 필요하다. **후자를 추천**한다 — 시계 출처가 하나가 되고 `chrono`가 트리에서 빠진다.
- 추가되는 크레이트는 `chrono` 포함 4개(`chrono`, `heck`, `sqlx-macros`, `sqlx-macros-core`). `chrono`를 빼면 3개.
- **`hmac`·`sha2`·`subtle`은 추가 크레이트가 0개다** — `sqlx-postgres`가 SCRAM 때문에 이미 갖고 있다(§1.7).

### 추가 실측 (스펙·ADR에 없던 것)

- **마이그레이션 재컴파일**: 기존 `.sql` **수정**은 재컴파일된다. 새 `.sql` **추가**는 재컴파일되지 않는다. `build.rs`의 `cargo:rerun-if-changed=migrations`로 둘 다 해결되고, 변화가 없을 때 불필요한 재빌드도 일으키지 않음을 확인했다. → ADR-0007 §5의 "마이그레이션 파일만 바뀌었을 때 재컴파일되지 않는다"는 **"파일을 추가했을 때"로 좁히는 것이 정확**하다.
- **axum 0.8.9 WS API**: 업그레이드 전 `HeaderMap` 추출, `max_message_size`/`max_frame_size`/`write_buffer_size`/`on_failed_upgrade`/`on_upgrade`, `routing::any`, `CloseFrame { code: u16, reason: Utf8Bytes }` — **컴파일로 확인**.
- **append-only 트리거 / `ON CONFLICT` 동작 / `occurred_at` 공식**: 이 PC PostgreSQL 18.6에서 **실행으로 확인**(§1.5, B-1, §4).
- **타이머 지터 4종**: §B-3 표.

### 미확인으로 남긴 것

- 워크스페이스 전체(신규 2크레이트 포함) 클린 빌드 시간 — T6에서 측정. 위 +2.8 s는 의존성 부분만이다.
- 30 연결 + Docker PostgreSQL + Unity Editor 동시 실행 시 자원 여유(U-6, qa 소유). 다만 §6-2 때문에 **Unity Editor 실행 여부가 타이머 측정에 영향을 줄 수 있다**는 점은 qa에게 전달되어야 한다.
- tungstenite가 클라이언트 Ping에 자동으로 Pong을 보내는지 — 서버가 **보내는** Ping과 받는 Pong만 우리 로직이므로 이번 슬라이스 판정에는 영향이 없다. T4에서 실동작으로 확인하고 T6에 적는다.
- 영속화 백로그 600 tick 경로의 실동작(B-18). 구현은 가능하지만 **검증할 AC가 없다.**

---

## 8. architect에게 요청하는 계약 변경

`contracts/**`는 건드리지 않았다. 요청은 두 건뿐이고 둘 다 작다.

1. **`contracts/fixtures/SESSION_READY/invalid/` 아님 — `SESSION_READY/tick-zero.json`의 `world_id` 교체** (B-14). 스파이크 월드 id에 `tick_hz: 1`을 쓰면 I-19와 모순된다. 다른 UUIDv7이면 된다.
2. **스펙 §5.2에 두 줄 추가** (계약 파일 변경 아님): `COMMAND_RESULT`/`PING_REPLY`의 `correlation_id`를 서버가 무엇으로 채우는지(B-13, 추천 `null`), 그리고 "`sequence`는 도메인 이벤트 전용이며 서버 메시지 envelope에는 존재하지 않는다"(§4).

그 외 스키마·반례 구성은 검토 범위에서 문제를 찾지 못했다. 신규 4타입의 `enum` 4종, `actor_id` 좁힘, `unevaluatedProperties: false`, `additionalProperties: false`는 T1의 Rust 타입 설계에 그대로 옮길 수 있다.

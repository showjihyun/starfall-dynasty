# p1-02-mining — 역사 엔진 검토 (Phase 2, 태스크 #4)

작성: history · 2026-09-27
입력: `00_request.md`, `p1-01-ship-movement/05_summary.md`, ADR-0006 §4, ADR-0007 §2·§4·§6, `contracts/common/event-envelope.schema.json`, HSE §9–22·§28–34·§57–63·§70·§89–95, `historical-engine` 스킬·`references/data-model.md`
상태: **초안 v1 — 스펙(`docs/specs/p1-02-mining.md`)·설계(`docs/design/p1-02-*`) 도착 전에 규약과 기획안으로 먼저 쓴 기준안.** 스펙이 나오면 §9 "스펙 대조"를 채워 v2로 올린다.
**갱신(2026-09-27): 검토 종결.** §0~§8은 v1 기준안 원문 그대로 두었다. 확정된 값은 §9.1~§9.5가 우선한다 — 특히 규칙 이름 `mineral-discovery@1`, ID 파생식(ADR-0014 §1), 테이블 4개(participants JSONB, objects 없음), 규칙 파일 + 짝 게이트(§9.4a)가 그렇다.

이 문서가 정하는 것: 역사 엔진이 **무엇을 입력으로 받고, 언제·어디서 판정하고, 무엇을 남기며, 어떤 테스트로 그 성질을 증명하는가.** 채굴 판정·인벤토리·쿨다운은 server/designer의 몫이고 여기서 다루지 않는다.

---

## 0. 요약 (결정 권고)

| # | 항목 | 권고 |
|---|------|------|
| 1 | 판정 입력 | 채굴 도메인 이벤트(가칭 `MINERAL_MINED`) **한 종류뿐**. payload만으로 판정 가능해야 한다 — 라이브 `WorldState`를 읽지 않는다 |
| 2 | "처음"의 순서 | **`(world_id, tick, sequence)` 사전순 최소.** `event_id`(UUIDv7, 실시간 성분 포함)는 순서·판정·ID 파생에 쓰지 않는다 |
| 3 | 같은 tick 동시 채굴 | v1: `sequence`가 작은 쪽이 단독 발견자(= 게이트웨이 수신 순번, ADR-0006 §4). 공동 발견자 안은 designer 판단으로 열어 둠(§1.4) |
| 4 | `rule_version` | `"mineral_first_discovery@1"`. `historical_events.rule_version`, `evidence.rule_version`에 저장. 규칙 설정 파일 내용 해시를 버전에 고정하는 golden 테스트 |
| 5 | 판정 위치 | **tick 루프 밖, 커밋된 `domain_events`를 `(tick, sequence)` 순으로 당겨 읽는 같은 프로세스 안의 history 태스크.** `domain_events`가 곧 outbox다 |
| 6 | 멱등 | ① 역사 행 쓰기 + 커서 전진을 **한 트랜잭션** ② `historical_event_id`를 발견의 정체성 `(rule_id, world, system, mineral)`에서 **결정적으로 파생**(UUIDv5) → PK가 중복·재처리·경쟁을 모두 막는다 ③ `historical_event_sources` PK |
| 7 | Evidence | 발견 1건당 시스템 자동 증거 1건(`SHIP_LOG`, `automatic`, `VERIFIED`, `derived_from = {}`), 출처 도메인 이벤트 좌표를 가리킴. append-only |
| 8 | 테이블 | 이번에 세움: `historical_events`, `historical_event_sources`, `historical_event_participants`, `historical_event_objects`, `evidence`, `history_consumer_cursors`. 미룸: claims·interpretations·relations·chronicle 등(§4 근거) |
| 9 | 테스트 | H-01~H-14 (§6). 모든 항목에 **전제 발생 단언**(분모)을 붙였다 |

---

## 1. 중요도 판정 규칙 v1 — `MINERAL_DISCOVERED`

### 1.1 규칙 문장

> 월드 W의 성계 S에서 광물 M이 **양의 수량으로 실제 채굴된** 도메인 이벤트 중 `(tick, sequence)`가 가장 작은 것 하나가 `MINERAL_DISCOVERED`(Level 2 Regional)를 만든다. 그 뒤 같은 `(W, S, M)`의 채굴은 Level 0이다.

- `rule_id = "mineral_first_discovery"`, `rule_version = "mineral_first_discovery@1"`
- 발견의 정체성(identity) = `(rule_id, world_id, system_id, mineral_id)`. **어느 채굴 이벤트가 이겼는지는 정체성이 아니라 내용이다.**

### 1.2 입력 — 필요한 도메인 이벤트와 payload

판정은 아래 필드만으로 끝나야 한다. 하나라도 없으면 판정 로직에서 추측하지 않는다(§5.4 fail-stop). **architect에게 payload 필수 필드로 요청했다(§8).**

| 필드 | 출처 | 용도 |
|------|------|------|
| envelope `world_id`, `tick`, `sequence`, `occurred_at`, `event_id`, `correlation_id`, `actor_id` | 기존 envelope | 순서·게임 시간·증거 출처·발견자(플레이어) |
| `payload.system_id` | 신규 | 발견 범위("성계에서") |
| `payload.mineral_id` | 신규 (data/ 의 광물 id) | 발견 대상 |
| `payload.quantity` (정수, > 0) | 신규 | "실제로 채굴됨" 판정. 0 산출이 이벤트가 되는 설계면 제외 조건이 된다 |
| `payload.ship_id` | 신규 | 참여자(함선) — 이후 Ship Biography의 첫 행 |
| `payload.deposit_id` (또는 채굴 지점 id) | 신규 | 대상 객체·장소 |
| `payload.position` (선택) | 신규 | 증거의 장소 범위. 없으면 deposit 위치로 대신 |

**판정은 라이브 `WorldState`를 읽지 않는다.** HSE §59의 `evaluate(event, state)`는 동기(tick 안) 판정을 전제한 의사코드다. 비동기로 판정하면(§5) 소비 시점의 월드 상태는 이벤트 시점의 상태가 아니다 — 같은 로그를 다른 시각에 재처리하면 다른 답이 나온다(원칙 9 위반). 판정에 필요한 "그 시점의 사실"은 전부 payload에 실려야 하고, "이미 발견됐는가"는 **history 자신의 기록**(앞선 순서의 판정 결과)으로만 답한다.

### 1.3 "처음"을 무엇의 순서로 정하는가

| 후보 | 판정 | 이유 |
|------|------|------|
| `(tick, sequence)` | **채택** | 둘 다 시뮬레이션이 결정적으로 부여한다(ADR-0006 §4: tick 안 순서 = 전역 단조 제출 순번; ADR-0007 §4: tick 안 발행 순서대로 0부터). `UNIQUE (world_id, tick, sequence)`가 전순서를 보장한다 |
| `event_id` | 기각 | UUIDv7 — 실시간 밀리초 성분 + 난수. 같은 시뮬레이션을 다시 돌리면 값이 달라진다. 계약 envelope도 "재생·결정성 비교는 `(world_id, tick, sequence)`와 내용으로 하고 `event_id`로 하지 않는다"고 적고 있다 |
| `recorded_at` / `occurred_at` | 기각 | 전자는 실시간(감사 전용), 후자는 tick에서 파생되어 tick보다 해상도가 낮다 |
| 도착(소비) 순서 | 기각 | HSE §30. 도착 순서를 믿지 않는다 |

### 1.4 같은 tick에 두 명이 캐면

- v1 기준: 같은 tick 안에서는 `sequence`가 작은 쪽이 발견자다. 이는 **게이트웨이 수신 순번**이며 결정적이고 감사 가능하다. 다른 쪽은 Level 0 채굴이다.
- 게임 관점의 비용: 20 Hz에서 같은 tick = 50 ms 안. 승패가 네트워크 지연으로 갈린다. 채굴이 **여러 tick에 걸친 작업**(designer 설계)이라면 같은 tick 완료는 드물고 문제도 작다.
- 대안(designer 선택지): **같은 tick의 최초 채굴자 전원을 공동 발견자**로 한 Historical Event에 묶는다(participants 여럿, `source_event_ids` 여럿). tick 안 순서에 판정이 의존하지 않게 되어 "tick 안 순열 불변"이라는 더 강한 속성이 성립한다. 비용은 복수 참여자·복수 증거 처리.
- **권고**: v1은 단독 발견자(단순). designer가 공동 발견자를 원하면 v1에 바로 넣는다(나중에 바꾸면 `@2`로 올려야 하고, 두 규칙이 섞인 역사가 남는다). → designer에게 질의(§8).

### 1.5 출력

| 대상 | 값 |
|------|----|
| `event_type` | `MINERAL_DISCOVERED` |
| `importance_level` | 2 |
| `fact_status` | `CONFIRMED` (시뮬레이션 사실. 해석 상태와 별개 — HSE §13) |
| `visibility` | `PUBLIC` 권고 (designer 확정) |
| `historical_event_id` | `UUIDv5(NS_HISTORY, "mineral_first_discovery|{world_id}|{system_id}|{mineral_id}")` — 결정적 |
| `tick`, `occurred_at` | 이긴 채굴 이벤트의 값 복사 (`recorded_at`은 별도 실제 시각) |
| `source_event_ids` | `[이긴 채굴 이벤트의 event_id]` + 좌표 `(tick, sequence)` 병기 |
| participants | `(actor_id, PLAYER, discoverer)`, `(ship_id, SHIP, vessel)` |
| objects | `(mineral_id, MINERAL)`, `(system_id, STAR_SYSTEM)`, `(deposit_id, DEPOSIT)` |
| payload | 구조화 값만. **문장을 저장하지 않는다** — 알림·UI는 `headline_key + params`(로컬라이즈 키) |

### 1.6 규칙 설정과 `rule_version`

- 기준값은 코드 상수가 아니라 버전 붙은 데이터로 둔다: `data/history/significance_rules.json`(designer 소유) — `{rule_id, rule_version, importance_level: 2, visibility, evidence: {type, visibility}}`. 서버 기동 시 검증하고 적재한 `rule_version`을 로그로 남긴다.
- **버전을 올리지 않고 값만 바꾸는 사고를 막는다**: 설정 파일의 정규화 내용 해시를 `rule_version`에 묶은 golden 테스트(H-09). 값이 바뀌었는데 버전이 같으면 실패한다. golden 재생 파일과 같은 규율이다.
- 규칙이 `@2`가 되어도 **과거 이벤트를 재계산하지 않는다.** 정체성 키에 버전을 넣지 않으므로 `@2`가 같은 `(S, M)`를 "다시 발견"하지 않는다. 발견 범위 자체가 바뀌는 규칙(예: "성역 최초")은 새 `rule_id`다.

---

## 2. 멱등

"같은 도메인 이벤트를 두 번 처리해도 Historical Event는 한 건"을 **세 겹**으로 보장한다. 어느 한 겹이 빠져도 나머지가 막는다.

1. **커서와 쓰기의 원자성** — 역사 행(event·sources·participants·objects·evidence) 삽입과 `history_consumer_cursors` 전진을 한 트랜잭션에서 한다. 중간에 죽으면 둘 다 없고, 재시작 시 같은 이벤트부터 다시 한다.
2. **결정적 ID + PK** — `historical_event_id`와 `evidence_id`가 입력에서 파생되므로, 재처리·중복 배달·(잘못된) 병렬 소비자가 같은 발견을 두 번 쓰면 PK 충돌 → `ON CONFLICT DO NOTHING`. 둘째 채굴자의 이벤트도 같은 후보 ID를 만들어 "이미 존재 → Level 0"이 된다. **"처음" 판정과 멱등이 같은 제약 하나로 강제된다.**
3. **`historical_event_sources (source_event_id, detector_rule)` PK** — 범용 방어(이후 집계 규칙용)이자 도메인 이벤트 → 역사 이벤트 역조회.

`processed_events(consumer, event_id)` 테이블은 두지 않는다(스킬 기본안과 다름, 근거): history는 `domain_events`를 **정렬된 로그로 당겨 읽으므로** "어디까지 처리했나"는 `(tick, sequence)` 한 점이면 된다. 이벤트마다 처리 행을 쓰면 `domain_events`를 한 벌 더 복제하는 셈이고, 그 대부분(SESSION_*·SHIP_*·Level 0 채굴)은 역사가 아니다. NATS 등 순서 없는 배달로 바뀌는 시점에 `processed_events`를 도입한다(재검토 조건).

---

## 3. Evidence

| 필드 | MINERAL_DISCOVERED의 자동 증거 |
|------|------------------------------|
| `evidence_id` | `UUIDv5(NS_EVIDENCE, "{historical_event_id}|SHIP_LOG|{ship_id}")` — 결정적 |
| `evidence_type` | `SHIP_LOG` (채굴 함선의 채굴 기록) |
| `source_entity_id` | `ship_id` |
| `creation_method` | `automatic` |
| `authenticity_status` | `VERIFIED` (시스템 내부 진실) |
| `derived_from_evidence_ids` | `{}` — **원본 증거는 빈 배열이다.** 이것이 출처 계보의 뿌리 표지다 |
| 출처 | `source_domain_event_ids = [event_id]` + `(world_id, tick, sequence)` — 어떤 사실에서 나왔는가 |
| `related_historical_event_ids` | `[historical_event_id]` |
| `visibility` | `PARTICIPANTS_ONLY` 권고(함선 소유자의 로그). 사건은 PUBLIC이어도 원본 로그는 소유자만 — 이후 탈취·공개·위조가 **새 증거**(`derived_from`)로 붙을 자리다 (designer 확정) |
| `reliability_scope` | `{"location":"high","time":"high","discoverer":"high"}` — 진실도 점수가 아니라 범위(HSE §15) |
| `content` | 구조화 JSON(mineral_id, quantity, deposit_id, tick, position). 문장 없음 |
| `rule_version` | 생성 규칙 버전(= 판정 규칙과 같은 묶음) |

- **채굴마다 증거를 만들지 않는다.** 증거는 역사 이벤트 1건당 1건. Level 0 채굴의 사실은 이미 `domain_events`에 있다.
- 불변: `forbid_mutation()` 트리거를 `evidence`에 건다. 기밀 해제·편집·위조는 전부 새 행 + `derived_from_evidence_ids`.

---

## 4. Fact / Claim / Interpretation 분리 (원칙 6) — 이번 슬라이스의 테이블 범위

### 4.1 세우는 테이블

| 테이블 | 층 | 불변 | 비고 |
|--------|----|:---:|------|
| `historical_events` | Fact | 트리거 | `fact_status TEXT NOT NULL CHECK (fact_status = 'CONFIRMED')`, `rule_version`, `importance_level`, `visibility`, `world_id`, `tick`, `source_sequence`, `occurred_at`, `recorded_at`, `source_event_ids UUID[]`, `payload` |
| `historical_event_sources` | Fact(색인) | 트리거 | PK `(source_event_id, detector_rule)` |
| `historical_event_participants` | Fact | 트리거 | discoverer / vessel |
| `historical_event_objects` | Fact | 트리거 | mineral / system / deposit |
| `evidence` | Evidence | 트리거 | §3 |
| `history_consumer_cursors` | 소비자 상태 | **가변** | `(consumer, world_id) PK, last_tick, last_sequence`. 기록이 아니라 진행 위치다 — 트리거를 걸지 않는 유일한 표이고, 그 이유를 마이그레이션 주석에 적는다 |

`fact_status`는 값이 하나뿐이지만 지금 둔다: 컬럼이 없으면 나중에 누군가 "해석 상태"를 `historical_events`에 붙이고 싶어질 때 막을 표지가 없다. CHECK가 `'CONFIRMED'` 하나로 잠겨 있으면 값을 늘리는 것 자체가 마이그레이션 + ADR 검토 대상이 된다.

### 4.2 이번에 두지 않는 것과, 두지 않아도 나중에 분리가 깨지지 않는 근거

미루는 것: `claims`, `claim_evidence`, `interpretations`, `interpretation_status_changes`, `historical_event_relations`, `evidence_relations`, `chronicle_entries` 등 프로젝션, 플레이어 지식 상태.

근거 — 분리를 깨는 경로는 둘뿐이다: (a) 주장·해석이 사실 테이블에 섞여 들어오거나, (b) 사실 테이블이 주장·해석에 의존하게 되는 것. 이번 설계는 둘 다를 구조로 막는다.

1. **사실 테이블의 쓰기 경로는 하나다.** history 소비자가 `domain_events`로부터만 쓴다. 게이트웨이 명령·REST 어디에도 역사 테이블 쓰기가 없다. 플레이어 입력이 사실이 되는 경로가 코드에 존재하지 않는다(원칙 2의 LLM도 같다). → 리뷰 항목 + H-14.
2. **사실 테이블에 해석 컬럼이 없다.** `fact_status`는 CHECK로 한 값에 잠긴다. 해석의 상태(PROPOSED/DISPUTED)는 나중에 별도 테이블의 **상태 변화 기록**으로 온다(HSE §13).
3. **의존 방향이 한쪽이다.** 나중의 `claims`는 `about_event_id`·`evidence_id`로 사실을 가리키고, 사실은 주장을 가리키지 않는다. 그래서 claims는 기존 테이블을 ALTER하지 않는 **순수 추가 마이그레이션**이다.
4. **증거는 이미 출처 필드를 갖고 있다** (`creation_method`, `derived_from_evidence_ids`, `authenticity_status`). 플레이어 작성·편집·위조 증거가 들어올 때 기존 행을 바꾸지 않고 새 행으로 들어온다.
5. **ID가 결정적이라 재구축해도 참조가 끊기지 않는다.** 나중에 claim이 `historical_event_id`를 가리킨 뒤 프로젝션·역사를 재구축해도 ID가 같다. 무작위 ID였다면 재구축이 주장을 고아로 만든다 — 이것이 지금 결정적 ID를 고집하는 가장 큰 이유다.
6. `historical_event_relations`: 이번 인과는 "도메인 이벤트 → 역사 이벤트" 하나이고 `source_event_ids`로 충분하다. 역사 이벤트끼리의 관계가 생기는 첫 규칙(집계·결과)에서 추가 테이블로 들어온다.

---

## 5. 판정 위치와 시점

### 5.1 권고: tick 루프 밖, 커밋 뒤, 같은 프로세스

```
tick 루프 ──TickOutcome──▶ 영속화 태스크 ──(tick 단위 1 tx: domain_events + worlds.last_tick + 인벤토리)──▶ PostgreSQL
                                                                                                     │
history 태스크 ◀── SELECT … WHERE world_id=$w AND (tick,sequence) > cursor AND tick <= last_tick ─────┘
   │  ORDER BY tick, sequence LIMIT n
   ├─ 순수 판정 detect(events, prior_discoveries, rules) → 결과
   └─ 1 tx: 역사 행 INSERT … ON CONFLICT DO NOTHING + 커서 전진
        └─ 커밋 뒤: 게이트웨이에 알림 제출(PUBLIC → 월드 전 세션) — best-effort
```

| 기준 | tick 안 동기 판정 | **커밋 뒤 비동기 판정 (권고)** |
|------|------------------|------------------------------|
| 결정성 | 성립 (tick 안 상태) | 성립 — 입력이 정렬된 커밋 로그뿐이다 |
| 판정 대상의 확실성 | 영속 실패 시 **없어진 사실에 대한 역사**가 남을 수 있다(영속과 한 tx로 묶지 않으면) | 커밋된 사실만 판정한다 |
| 재처리·재구축 | sim 재실행이 필요 | `domain_events`만으로 커서 0부터 재생 |
| `starfall-sim` 경계 | IO-free·의존성 0 크레이트에 역사 개념이 들어간다. 규칙 변경 = sim 변경 = golden 재생 영향 | sim 불변. 규칙 변경이 물리 재생에 닿지 않는다 |
| tick 예산 | 판정 비용이 tick에 얹힘 | 무관 |
| 알림 지연 | 0 | 커밋 + 폴링 주기(목표 ≤ 1 s, 측정 대상). HSE §95: 역사는 결과적 일관성 허용 |
| 장애 | 영속과 같은 운명 | 소비자가 죽어도 로그가 남는다 → 재시작 시 커서부터 (HSE §93) |

- **ADR-0006 §4 도식의 "5. (역사 판정)" 위치(tick 안)를 옮기는 결정**이다 → architect가 ADR(신규 또는 0006 개정)로 기록해야 한다.
- `domain_events`가 outbox 역할을 한다: ADR-0007 §6의 마감 조건(상태 변경과 이벤트가 같은 tx)은 **인벤토리 갱신 + 채굴 도메인 이벤트를 같은 tx에 쓰는 것**으로 충족되고, 별도 `outbox_events` 테이블 없이 history는 그 로그를 당긴다. 이것은 server와 architect의 결정이며 history는 이 전제에 의존한다.
- **워터마크**: `tick <= worlds.last_tick`만 읽는다. 영속화가 단일 작성자·tick 순서 커밋인 한 보이는 행은 이미 연속이지만, 이 조건이 그 가정을 코드에 명시한다. 영속화가 병렬이 되면 이 가정을 재검토한다.

### 5.2 재구축

역사 테이블을 비우고(테스트 DB에서만) 커서 0부터 다시 돌리면 같은 행(시각 컬럼 제외)이 나와야 한다. 주 DB의 `domain_events`는 절대 비우지 않는다(CLAUDE.md). 재구축 테스트는 **복사한 로그**로 별도 DB/스키마에서 돈다.

### 5.3 알림

- 커밋 뒤 게이트웨이로 넘기는 서버 메시지(이름은 architect: 예 `HISTORICAL_EVENT_NOTICE`) — `historical_event_id`, `event_type`, `headline_key`, `headline_params`, `visibility`. **문장 없음.**
- 배달은 best-effort(오프라인 플레이어는 못 받는다). 원본은 DB. 이 한계는 Chronicle(p2)로 닫힌다 → 열린 질문.

### 5.4 판정 불가 이벤트 — fail-stop

payload 누락·알 수 없는 `schema_version`의 채굴 이벤트를 **건너뛰지 않는다.** 건너뛰면 그 뒤의 이벤트가 잘못 "처음"이 될 수 있고, 그 오류는 영구 기록이 된다. 해당 월드의 소비를 그 좌표에서 멈추고(커서 전진 없음) 에러 로그 + `history_detector_halted` 메트릭. 게임은 계속 돈다(역사만 지연). 업캐스팅은 스키마 v2가 생길 때.

---

## 6. 필수 속성 테스트 (qa가 Phase 3 계약에 그대로 옮길 수 있는 형태)

공통 규율: 모든 항목은 **관찰 대상 조건이 실제로 발생했음을 함께 단언**한다(CLAUDE.md 검증의 규율). 난수 생성 입력은 시드를 출력하고 실패 시 재현 가능해야 한다.

| ID | 이름 | 절차 | PASS 기준 | 전제 단언(분모) |
|----|------|------|-----------|----------------|
| H-01 | 멱등 — 반복 처리 | 발견을 만드는 채굴 이벤트 1건을 k∈{2,3,5}회 처리 | HE 1 · evidence 1 · sources 1 · participants 2 | 첫 처리 직후 HE = 1 (0이 아님) |
| H-02 | 멱등 — 커밋 전 장애 | 역사 행 INSERT 뒤 커밋 전에 오류 주입 → 재시작 | HE 1, 커서는 그 이벤트 뒤 | 주입 지점이 실제 실행됨(카운터 ≥ 1), 장애 직후 HE = 0 |
| H-03 | 순서 — 도착 순열 불변 | 생성 로그(광물 ≥ 2 × 채굴자 ≥ 3, 같은 tick 충돌 포함)를 무작위 순열 ≥ 200개로 소비자 입력에 넣음 | 모든 순열에서 발견 집합 `{(S,M) → (tick,seq)}`이 오라클(§6.1)과 같고 서로 같다 | 로그에 같은 tick 충돌 ≥ 1, **argmin 이벤트가 마지막에 도착한 순열 ≥ 1** |
| H-04 | 동시 발견 — tie-breaker | 같은 tick에 두 함선이 같은 광물 채굴, seq s1 < s2. 픽스처의 `event_id` 순서는 seq와 **반대**로 만든다 | 발견자 = s1의 채굴자. 제출 순서를 바꾸면 발견자도 바뀜 | 두 이벤트의 tick이 같음, `event_id` 순서 ≠ seq 순서 |
| H-05 | 결정성 — 해시 | 같은 로그를 새 역사 저장소 두 곳에서 처리 | 정규화 해시(시각 컬럼 제외, ID 포함, ID순 정렬) 동일 | 해시 입력 HE ≥ 1, evidence ≥ 1 |
| H-06 | 재생 — 재구축 | 봇 세션으로 쌓인 `domain_events`를 복사해 별도 DB에서 커서 0부터 재구축 | 라이브가 만든 역사와 해시 동일 | 라이브 HE ≥ 2 (광물 2종 이상 발견된 세션) |
| H-07 | 불변 | 역사 5개 표 각각에 UPDATE·DELETE | 전부 예외 | **대상 행이 존재함을 먼저 단언** — 행 단위 트리거는 0행 매치에서 발화하지 않아 "0행이라 통과"가 가능하다 |
| H-08 | 비역사 필터 | 채굴 N건, 서로 다른 (S,M) = K개 (K ≥ 2, N ≥ 3K) + SESSION_*·SHIP_* 이벤트 | HE = K, 채굴 외 타입에서 HE 0 | N, K를 출력. SESSION·SHIP 이벤트 ≥ 1 포함 |
| H-09 | 규칙 버전 | 모든 HE·evidence의 `rule_version` = 적재된 설정 버전. 설정 파일 해시 golden | 일치 / 값 변경 + 버전 동일 → 실패 | 검사한 HE 수 ≥ 1. golden 음성 사례(값만 바꾼 설정)가 실제로 실패함 |
| H-10 | fail-stop | 필드 누락 채굴 이벤트를 발견 후보 앞에 끼움 | 소비 정지, 커서가 그 앞, 뒤 이벤트로 HE 없음, 메트릭 +1 | 뒤에 발견 후보 이벤트가 실제로 존재 |
| H-11 | 워터마크 | `last_tick`보다 큰 tick의 행을 (테스트에서) 둠 | 판정 안 됨 | 그런 행 ≥ 1 |
| H-12 | 오라클 교차 (통합) | 실서버 + 봇 채굴 세션 후 SQL 오라클(§6.1)과 대조 | 역사의 발견 집합 = 오라클 집합, 양방향 차집합 0 | 오라클 집합 크기 ≥ 2 |
| H-13 | 수량 0 | (설계상 가능하다면) quantity 0 채굴 이벤트가 먼저 오고 양수가 뒤따름 | 양수 이벤트가 발견 | 수량 0 이벤트가 실제로 소비됨. 설계상 불가능하면 스키마 `minimum: 1` 확인으로 대체 |
| H-14 | 쓰기 경로 단일 | 역사 테이블 INSERT를 하는 코드 위치 목록 | history 소비자 모듈 외 0곳 | 소비자 모듈의 INSERT가 ≥ 1곳 검출됨 (규칙 9: 소스는 부정만 증명 — 이 항목은 H-12와 짝으로만 PASS) |

### 6.1 오라클 (qa용, 역사 코드와 독립)

```sql
-- 각 (world, system, mineral)의 첫 양수 채굴 = 기대 발견
SELECT DISTINCT ON (world_id, payload->>'system_id', payload->>'mineral_id')
       world_id, payload->>'system_id', payload->>'mineral_id', tick, sequence, event_id
FROM domain_events
WHERE event_type = '<채굴 이벤트 이름>' AND (payload->>'quantity')::bigint > 0
ORDER BY world_id, payload->>'system_id', payload->>'mineral_id', tick, sequence;
```

역사 쪽 `MINERAL_DISCOVERED`의 `(world, system, mineral, tick, source_sequence)` 집합과 양방향 차집합이 0이어야 한다. 분모(오라클 행 수)를 반드시 출력한다.

---

## 7. History → Content 점검 (스킬 §8)

| 질문 | 답 | 상태 |
|------|----|------|
| 누가 발견할 수 있나 | 발견 순간 월드에 접속한 전원(PUBLIC 알림). 발견자는 자기 함선 로그(증거) | 부분 — 오프라인 플레이어 경로 없음 |
| 발견한 플레이어가 할 수 있는 행동 | 그 성계로 가서 같은 광물을 캔다 | 약함 — 거래(p1-03)가 붙어야 의미가 커짐 |
| 그 행동이 다시 도메인 이벤트를 만드나 | 예 — 채굴 이벤트 | 충족 |

→ 오프라인·후속 접속 플레이어가 발견을 알 방법이 없다: 열린 질문 OQ-H1(아래).

---

## 8. 보낸 요청과 결과

| 대상 | 요청 | 결과 |
|------|------|------|
| architect | (1) 채굴 도메인 이벤트 payload 필수 필드(§1.2) (2) 판정 위치를 tick 밖·커밋 뒤로(§5.1) + ADR 기록 (3) `domain_events` = outbox 전제 확인(인벤토리와 같은 tx) (4) 역사 envelope·`MINERAL_DISCOVERED`·알림 메시지 계약, 결정적 ID 네임스페이스 (5) 열린 질문 OQ-H1·OQ-H2 스펙 반영 | 대기 |
| designer | (1) 같은 tick 동시 채굴: 단독(seq) vs 공동 발견자 (2) 사건·증거 가시성 (3) 수량 0 채굴이 이벤트가 되는지 (4) `data/history/significance_rules.json` 소유·형식 | 대기 |

### 열린 질문 (스펙에 합칠 것)

- **OQ-H1** 발견 시점에 오프라인이던 플레이어는 이 슬라이스에서 발견을 알 수 없다. 받아들이는가(Chronicle p2까지), 아니면 최소 조회(접속 시 최근 발견 목록 1개 메시지 / `GET /history/events`)를 넣는가? 권고: 받아들이고 qa는 SQL로 검증.
- **OQ-H2** 같은 tick 동시 채굴의 발견자 규칙(§1.4). 사용자 결정 가능 항목.

---

## 9. 스펙 대조

### 9.1 architect ADR-0014 골격 제안과의 합의 (2026-09-27)

architect 제안: `starfall-history` 순수 코어(contracts만 의존) + `starfall-persistence` 안의 러너(history 소유) + 마이그레이션 `0003_*`, 커밋된 `domain_events`를 커서로 소비, 한 tx, `dedupe_key` + `UNIQUE(world_id, event_type, dedupe_key)`, participants JSONB, `HISTORICAL_EVENT_NOTICE`(실시간 + 세션 시작 backfill), `rule_version = "mineral-discovery@1"`.

| 쟁점 | 이 문서의 v1 | architect 안 | history 답 |
|------|-------------|-------------|-----------|
| 판정 위치 | 커밋 뒤, 같은 프로세스 | 같음 | 합의 |
| rule_version 이름 | `mineral_first_discovery@1` | `mineral-discovery@1` | architect 안 채택 |
| "처음" 보장 | 결정적 ID의 PK | `dedupe_key` UNIQUE | 일반 열 UNIQUE 찬성. 키에 rule_version을 넣지 않는다 |
| `historical_event_id` | 결정적 UUIDv5 | 비교에서 제외(무작위 암시) | **수정 요청**: `UUIDv5(NS, "{world}|{event_type}|{dedupe_key}")`. 와이어·backfill·p2 claims 참조가 재구축 뒤에도 유지돼야 한다(§4.2-5) |
| participants/objects 테이블 | 정규화 테이블 | JSONB | JSONB 찬성. 정규화는 나중에 재구축 가능한 프로젝션으로 |
| 러너 위치 | — | persistence 안 | 찬성 (feature로 넣으면 순수성이 흐려지고, cfg로 막힌 코드가 게이트를 피할 수 있다) |
| 오프라인 플레이어 (OQ-H1) | 열린 질문 | 세션 시작 backfill | backfill로 닫힘 |
| 보강 요청 | — | — | B 라이브 WorldState 금지 불변식 · C fail-stop · D 메모리 집합과 UNIQUE가 어긋나면 버그 신호 · E `tick <= last_tick` 워터마크 · F evidence `derived_from`·`source_entity_id`·`visibility` · G H-01~H-14 채택 |

§4.1의 테이블 목록은 이 합의에 따라 `historical_events`(participants·objects JSONB, `dedupe_key`), `historical_event_sources`, `evidence`, `history_cursor`로 줄어든다. 결정적 ID 요청에 대한 architect의 답을 기다린다.

### 9.2 designer 설계 §4 규칙 초안과의 대조 (2026-09-27)

`docs/design/p1-02-mining-design.md` §3·§4·§9, 시나리오 S-2·S-4·S-7.

| 항목 | 설계 §4 | history 판정 |
|------|---------|-------------|
| 입력 | `MINERAL_MINED`(world, star_system_id, deposit_id, mineral_id, actor_id, ship_id, quantity_kg, remaining_after_kg) | 충분. `quantity_kg` 스키마 `minimum: 1` 요청(§3.1: 수량 0 성공은 규칙상 불가) → H-13은 스키마 확인으로 대체 |
| "처음" 순서 | `(tick, sequence)` 오름차순, 도착 순서 아님 | 일치 |
| 같은 tick | 수락 순서(= sequence), 공동 발견 없음 (§3.1, Q2) | 일치 → OQ-H2 닫힘. 채굴 명령도 ADR-0006 §4 수락 순서를 유지해야 한다(`ship_id` 순 금지) |
| 발견자 | actor (함선 아님) | 일치. 함선은 `vessel` 역할의 두 번째 참여자로 추가 제안(Ship Biography, 재계산 불가 정보) — designer 답 대기 |
| 레벨·가시성·상태 | L2 / PUBLIC / CONFIRMED | 일치 |
| 멱등 키 | `(world, system, mineral, MINERAL_DISCOVERED)` | = architect `dedupe_key "{star_system_id}/{mineral_id}"` + `UNIQUE(world_id, event_type, dedupe_key)` |
| `rule_version` | `mineral_discovery.v1` | `mineral-discovery@1`로 통일 요청 |
| importance_score + rarity 복사 | 100 + 희귀도 가산, 판정 당시 rarity 복사 | **v1에서 빼기 권고(A 안).** rarity는 도메인 이벤트 밖의 두 번째 입력이라, 데이터 변경 후 재구축하면 결정성이 깨진다. 레벨은 고정이라 소비자도 없다. 꼭 필요하면(B 안) rarity를 `MINERAL_MINED` payload에 싣고 가중치를 규칙 데이터로 옮긴다 — designer 답 대기 |
| Evidence | 추출 기록 1건, 1차 사료, 출처는 서버 시뮬레이션 | `SHIP_LOG` / automatic / VERIFIED. 가시성 PARTICIPANTS_ONLY 권고 — designer 답 대기 |
| 기록하지 않는 것 | 모든 채굴(L0), 광맥 드러남(시뮬레이션 상태), 고갈, 개인 첫 획득 | 동의. 드러남이 시뮬레이션 상태이고 역사 판정을 읽지 않는다는 것(Q3)은 "sim은 history를 입력으로 받지 않는다"는 경계와 맞다 |
| 발견자 이름 표시 | "A가 발견했다" | 역사 행에는 `actor_id`만. 이름은 알림 조립 시 게이트웨이가 붙인다(architect 제안) |

시나리오 → 테스트 연결: S-2 → H-03·H-04 (순열 수 출력, 같은 tick 전제 단언). S-7 → H-02의 라이브 짝(H-15 후보: 재기동 후 HE 수 불변 + 재기동 뒤 첫 신규 광물이 1건 추가되는 긍정 증거). S-4 → architect의 세션 시작 backfill.

### 9.3 스펙 초안 §4.5·§5.1·§6·AC-9~12 + ADR-0014(proposed) 검토 (2026-09-27)

수락: ADR-0014 §1(순수 코어, contracts만 의존, 러너는 persistence), §2(커밋된 행, 커서, 채널은 깨우기 + 1 s 폴링), §3(테이블 범위와 원칙 6을 구조로 지키기), §4(세 겹 멱등 + 충돌하면 러너만 정지), §6(LIVE는 커밋 뒤, BACKFILL), I-60~I-67, AC-9(f) 배치 경계 불변. 스펙 §9.3의 네 질문은 모두 답했다. 증거 타입은 `SHIP_LOG`.

architect에게 보낸 수정 요청:

| ID | 위치 | 요청 | 이유 |
|----|------|------|------|
| K1 | ADR §1·§5, I-64, AC-11, envelope | `historical_event_id = UUIDv5(NS_HISTORY, "{world}|{event_type}|{dedupe_key}")`, `evidence_id = UUIDv5(NS_EVIDENCE, "{hist_id}|SHIP_LOG")`. 결정성 비교에 id를 포함한다. primitives에 UuidV5 타입, NS 상수 | 와이어·I-65 중복 제거 키·p2 claims 참조가 재구축 뒤에도 유지돼야 한다(§4.2-5) |
| K2 | 스펙 §5.1 닫힌 집합 | `entity_kind` += `SHIP`, `role` += `VESSEL` | Ship Biography. 재계산 불가 |
| K3 | MINERAL_DISCOVERED payload | `discoverer_actor_id`와 participants 중복을 없앤다(payload에서 제거 권고) | 한 사실은 한 곳에 |
| K4 | MINERAL_MINED | `quantity` `minimum: 1` | 판정 조건을 스키마가 보장. H-13을 대체 |
| A1 | ADR §2 | fail-stop: 판정 대상 타입의 역직렬화 실패·모르는 schema_version → 정지(커서 유지, `history_detector_halted`) | 건너뛰면 뒤의 채굴이 잘못 "최초"가 되어 영구 기록이 된다(§5.4) |
| A2 | ADR §1 | 코어가 월드별 `(tick, sequence)` 역행 입력을 거부 | ORDER BY 누락 버그가 "틀린 발견자" 대신 "정지"로 나타나게. S-2 순열 테스트를 (i) 정렬 후 동일 + 순열 수 출력, (ii) 비정렬은 거부로 분해 |
| A3 | ADR §3, AC-10(b) | `historical_event_sources`에도 append-only 트리거 | 멱등 2층이 조용히 사라지는 것을 막는다 |
| A4 | ADR §5 | v1 기준값은 코드 상수 + 코어 출력 golden(`tests/data/golden/mineral-discovery@1.json`, BLESS 규율). 출력이 바뀌고 버전이 같으면 실패 | §1.6의 설정 해시 안을 대체한다. 더 단순하고 코드 변경도 잡는다 |
| A5 | ADR §6 | 게이트웨이의 "목록 추가 + LIVE 브로드캐스트"와 "세션 열림 → BACKFILL 송신"을 같은 직렬화 문맥에서 처리. 재기동 시 목록 적재 주체를 명시. "광물 3 이하"를 4종으로 수정 | 없으면 커밋 직후 열린 세션이 LIVE도 BACKFILL도 못 받는 누락 틈이 생긴다 |
| A6 | ADR §7 | 목표 p95 ≤ 1 s 측정, 실패 경계 5 s | designer §3.3의 두 박자 배너(1 s 가정)와 맞춘다 |
| AC | AC-9(d)(h)(i), AC-10 | (d) 픽스처의 event_id 순서를 seq와 반대로. (h) 역행 거부. (i) golden. AC-10에 fail-stop 항목(전제: 뒤의 후보가 존재). S-7 재기동 긍정 증거를 AC로 | §6 H-04·H-09·H-10·H-15 |

### 9.4 designer 최종 답 (2026-09-27)

- 발견자: 단독, `(tick, sequence)` 최소, 공동 발견 없음 — **v1 확정**(OQ-H2 닫힘). 진 쪽의 "한발 먼저" 표시는 클라이언트가 HE tick과 자기 채굴 tick으로 계산한다(새 사실 아님).
- 가시성: 사건 PUBLIC, 증거 `SHIP_LOG` PARTICIPANTS_ONLY. **열람 판정은 PLAYER 참가자(actor) 기준**이다. 함선은 영속하지 않기 때문이다(ADR-0011 §7). `source_entity_id = ship_id`는 유지.
- 수량 0 이벤트는 생기지 않는다(거절은 이벤트 없음, 성공은 ≥ 1). `quantity > 0` 조건은 방어용으로 둔다.
- 규칙 파일: 한때 `data/history/rules/mineral-discovery.json`이 만들어졌으나, 메시지가 엇갈린 뒤 앞선 보류 요청에 따라 **삭제되었다**(추적 전). **최종: A4 원안** — v1 기준값(L2, PUBLIC, evidence SHIP_LOG/PARTICIPANTS_ONLY)은 `starfall-history` 코드 상수로 두고, `rule_version`은 고정 입력에 대한 코어 출력 golden(BLESS 규율)으로 묶는다. `significance-rule` 데이터 스키마는 필요 없다(architect에게 정정 통보). 가중치가 생기는 규칙에서 데이터 파일을 다시 연다(파일 하나에 규칙 하나 — designer 제안 채택 예정).
- importance_score·rarity: v1에서 제외(designer 철회). 판정 입력 = 도메인 이벤트 + 규칙 파일.
- participants에 `{ship_id, SHIP, VESSEL}` 추가에 동의.
- 이름: `star_system_id`, `mineral_id`(data/minerals id), `deposit_id`, `quantity_kg`(스펙 초안 `quantity`와 불일치 → architect가 결정), `position` 없음.
- NOTICE 조건(S-4): LIVE와 BACKFILL 모두 tick, 발견자 표시 이름, mineral_id, deposit_id. 표시 이름은 알림을 조립할 때 actor로 조회해 붙인다. 발견자가 로그아웃한 뒤에도 조회할 수 있어야 한다 → architect에게 확인 요청.

§1의 규칙 이름 `mineral_first_discovery@1`, §1.4의 공동 발견 안은 위 결정으로 대체되었다.

#### 9.4a 규칙 파일 — 경위와 현재 상태 (리더 지적 반영, 2026-09-27)

- **현재 사실: `data/history/`는 존재하지 않는다**(리더가 확인). 규칙 파일은 없다.
- 경위(메시지 엇갈림): ① history가 `significance_rules.json`을 요청했다. ② designer가 `data/history/rules/…json`을 만들었다. ③ history가 A4(코드 상수 + golden)를 제안하며 보류를 요청했다. ④ designer가 ③에 따라 파일을 삭제했다(추적 전이라 git에 흔적이 없다). ⑤ ②의 통지가 늦게 도착해 history가 "파일을 쓴다"로 뒤집어 architect에게 보냈다. ⑥ ④의 통지를 받고 history가 다시 "A4 원안"으로 정정했다. 위 §9.4 "규칙 파일" 항목의 첫 판(파일 사용)은 ⑤, 현재 문구는 ⑥이다. 이 문서는 ⑤판의 원문을 보존하지 못했다(Edit로 교체했다) — 그 내용은 "기동 시 적재·검증 + golden이 파일 해시와 코어 출력을 `rule_version`에 묶는다"였다.
- **결정권: architect(ADR-0014 소유자). 판정 대기.** 그때까지 history는 designer에게 파일을 다시 만들라고 요청하지 않는다.
- **architect 판정 (ADR-0014 accepted, §5): 파일을 둔다.** `data/history/rules/mineral-discovery.json`(designer 소유, 스키마 `contracts/data/significance-rule.schema.json`, 레지스트리 `SIGNIFICANCE_RULE` consumer = history). 조립 바이너리가 기동 시 읽어 값으로 코어에 넘기고, golden이 (a) 파일 정규화 해시와 (b) 코어 출력을 `rule_version`에 묶는다. history가 요구한 "읽는 코드 + 게이트의 짝" 조건을 충족한다 → history 수락. 파일 생성 요청은 architect가 designer에게 했다.
- history가 architect에게 보낸 판단 입력(판정 전): 기준값이 L2·PUBLIC 두 상수뿐이면 코어 출력 golden만으로 규칙 버전이 코드 변경까지 잡는다. 파일을 둔다면 **읽는 코드와 검사하는 게이트가 짝지어져야** 하고, 그렇지 않으면 틀려도 아무것도 실패하지 않는 파일이 하나 더 생긴다.

### 9.5 계약 확정 대조 (2026-09-27) — 검토 종결

architect: §9.3의 K1~K4, A1~A6 전부 수용. ADR-0014 **accepted**. 계약은 `contracts/`에 있다.

| 항목 | 확인 |
|------|------|
| 결정적 ID | ADR-0014 §1. NS_HISTORY `29c482cc-…802d`, NS_EVIDENCE `503d3cb5-…d5c1`. **history가 fixture 두 개(`starfall-glass`, `glacine-seq-nonzero`)의 `historical_event_id`를 독립 계산(Python `uuid.uuid5`)으로 재현함 — 일치.** H1 단위 테스트로 Rust 파생 함수가 같은 값을 내는지 건다 |
| evidence_id | `UUIDv5(NS_EVIDENCE, "{hist_id}|{evidence_type}")` — 역사 이벤트 1건당 증거 1건이므로 ship_id가 없어도 유일하다. 수락 |
| objects 배열 제거 | **수락.** what은 타입별 payload(`mineral_id`, `deposit_id`, `quantity_kg`)에 한 번만 적힌다. objects 색인은 payload에서 파생 가능한 p2 프로젝션이다. `location`은 `star_system_id`만 둔다 |
| participants | 정확히 2개(DISCOVERER → VESSEL 순). `discoverer_actor_id` 중복 제거(K3) |
| MINERAL_MINED | `quantity_kg ≥ 1`(K4), `causation_id = command_id`. 판정은 `star_system_id`, `mineral_id`, `deposit_id`, `quantity_kg`, `ship_id`, envelope `actor_id`만 읽는다 |
| 반례 fixture | importance 0, discoverer-only, 서사 필드 주입, 해석 상태, v7 id, 빈 source, 번호 없는 rule_version, sequence 필드, MINED에 importance 주입, quantity 0 — §4.2의 구조적 분리 근거들이 스키마로 걸렸다 |

남은 것(검토 범위 밖, Phase 3~4로 넘김):
- 발견자 표시 이름: 로그아웃한 발견자를 BACKFILL에서 이름으로 보일 조회 경로가 이번 계약에 없다. 클라이언트는 `entity_id`로 표시한다(설계 §3.4 문구는 architect·client가 정한다). 이것은 역사 사실이 아니라 표현 계층의 일이다.
- 작업 분해(#5): H1 = 코어 + 규칙 적재 + golden, H2 = 마이그레이션 0003 + 러너 + NOTICE 생산. `persistence/src/history.rs`(history)와 `lib.rs`(server)의 경계 — 러너 기동과 커밋 알림 채널 인터페이스는 server와 먼저 합의한다.
- §6의 H-01~H-14 + H-15(재기동 긍정 증거)가 qa의 Phase 3 계약 입력이다. AC-9~AC-12와 겹치는 항목은 AC 번호를 우선한다.

#### 9.4b 규칙 파일 복원 (2026-09-27)

designer가 architect 판정(ADR-0014 §5, `contracts/data/significance-rule.schema.json`)에 따라 `data/history/rules/mineral-discovery.json`을 복원했다(스키마 검증 오류 0이라고 보고함). history는 이미 §9.4a에서 이 판정을 수락했으므로 이견은 없다. H1의 golden 테스트는 파일 정규화 해시와 코어 출력 둘 다를 `rule_version`에 묶는다.

## 10. Phase 3 — T0 인터페이스 제안 (history → server, 2026-09-27, 합의 대기)

정본은 T0 산출물 `02_interface.md`(server 작성, history 절 공동)다. 여기에는 history가 보낸 제안의 요지만 남긴다(Phase 4 인계용).

- 의존: persistence → history(코어) + contracts. gateway → contracts만. 조립은 game-server.
- 커밋 알림: `watch::Sender<Option<u64>>`(마지막 커밋 tick). 깨우기 신호일 뿐이고, 워터마크는 DB `worlds.last_tick`이다. 러너는 `changed()` 또는 1 s 타이머로 깬다. 영속화가 종료 시 Sender를 drop하면 러너는 마지막으로 한 번 따라잡은 뒤 종료한다. main은 persistence 다음에 runner를 await한다. 멈춘 러너도 종료 신호에는 응답한다.
- LIVE 채널: `mpsc<HistoricalEventNotice>`(bounded 256, delivery LIVE). 러너가 커밋 뒤 만들어 보낸다 — producer=history이므로 커버리지 스크립트가 history 경로에서 이 이름을 찾는다. 드라이버는 매 tick `try_recv`로 받는다.
- 직렬화 문맥: tick 드라이버 스레드(routes 소유)가 BACKFILL 목록을 가진다. 한 tick 안의 순서: (a) route_outbound를 하면서 SESSION_READY를 받은 세션에 ready 표시 → (b) 새로 ready가 된 세션에 목록 전체를 BACKFILL로 보냄 → (c) LIVE를 수신하면 목록에 추가하고 ready인 세션에 브로드캐스트. 이렇게 하면 누락 틈이 없다.
- 기동: 마이그레이션 → 상태 적재 → `history::load`(한 스냅샷에서 판정 상태 + 초기 목록 + 커서를 읽음) → `runtime::build(initial_backfill, history_rx)` → 영속화·러너 spawn → bind. 목록이 build 인자이므로 "목록 전 연결 수락"이 구조적으로 불가능하다.
- 관측: `HistoryHandles` → `/debug/stats`(records, conflicts, halted, cursor_tick, catchup_rows).

### 10.1 server 답과 합의 (2026-09-27)

server는 §10 제안을 수락하고 세 가지를 조정했다(`02_server_ack.md` §4). history는 셋 다 수락했다.
1. 채널 항목은 NOTICE가 아니라 **역사 기록 레코드**(계약 `MINERAL_DISCOVERED` 타입)다. BACKFILL 목록도 같은 타입이다. delivery·envelope tick·message_id는 드라이버가 붙인다. 이에 따라 레지스트리 `HISTORICAL_EVENT_NOTICE` producer를 history → server로 바꿔 달라고 architect에게 요청했다. 그대로 두면 커버리지 스크립트가 history 경로에서 NOTICE를 찾다 실패하고, 맞추려다 거짓 생산이 생긴다. `MINERAL_DISCOVERED` producer는 history로 유지한다.
2. 드라이버가 먼저 끝나서 LIVE 송신이 Closed로 실패하는 것은 오류가 아니다. 러너는 멈추지 않는다. 기록은 이미 커밋돼 있고 다음 기동의 BACKFILL로 전달된다.
3. 러너 halt 핸들과 영속화 fatal 핸들은 분리한다. 러너가 멈춰도 서버는 계속 돈다.
- 영속화 fatal로 watch가 drop되면 러너는 마지막 커밋까지 따라잡고 종료한다(I-62와 일치).
- `02_interface.md`는 Phase 4 T0에서 server가 쓰고, history 절은 history가 채운다.

### 10.2 architect 통보 (2026-09-27)

- ADR-0013 §7 기동 순서가 T0 합의대로 확정됐다: 경제 적재 → `history::load` → `runtime::build(목록 인자, tick 시작)` → 러너·영속화 spawn → bind. H2가 따른다.
- ADR-0014 §6 BACKFILL → 조회 API 전환 트리거를 64에서 32로 낮췄다. 64는 송신 큐 용량과 같아서, 새로 접속한 세션이 BACKFILL을 받다 SLOW_CONSUMER로 끊긴다. 이번 슬라이스의 상한은 4건이다. 수락.
- 레지스트리 `HISTORICAL_EVENT_NOTICE` producer는 확인 시점에 아직 `["history"]`였다 → 반영 여부를 architect에게 재확인 요청.
- (후속) architect가 반영했다: `HISTORICAL_EVENT_NOTICE` producers = `["server"]`, `MINERAL_DISCOVERED`는 history 유지, ADR-0014 §6 문장 수정. registry_version은 4 그대로. history가 레지스트리 파일에서 직접 확인했다.

## 11. Phase 3 — 스프린트 계약 r0 검토 (태스크 #8, 2026-09-27)

### 11.1 §6.1 오라클 정정 (원문은 고치지 않는다)

§6.1 오라클은 계약 확정 전에 써서 `payload->>'system_id'`·`'quantity'`와 가칭 이벤트 이름을 쓴다. 계약은 `star_system_id`·`quantity_kg`·`MINERAL_MINED`이다. 원문대로 돌리면 NULL 키 한 행만 나온다(qa 발견). **정본은 `02_sprint_contract.md` §0.10**이다. NULL 필드 양성 대조가 붙어 있다.

### 11.2 H-15

§6 표에 행이 없었다(누락). H-15 = 설계 S-7의 라이브 짝(§9.2): 재기동 뒤 기존 발견 수 불변 + 재기동 뒤 첫 신규 광물이 +1. 계약 SC-57(+ SC-99)에 대응한다.

### 11.3 qa에게 보낸 수정·보강

- 수정 1 — SC-47: (tick, seq)가 유일하므로 "정렬 후 동일"은 자명하게 성립하고, 정렬하는 쪽도 테스트 자신이다. 러너의 ORDER BY를 시험하는 DB 항목으로 옮긴다(삽입 순서 순열 ≥ 20). ⊘: argmin이 마지막에 삽입된 경우, 물리 순서 ≠ 정렬 순서, event_id 순서 ≠ seq 순서.
- 수정 2 — SC-95: 테스트 코드(SC-53의 직접 INSERT)를 제외 목록으로 명시한다. 여러 줄에 걸친 `INSERT\s+INTO` 패턴으로 찾는다.
- 수정 3 — SC-52: 거부의 원인이 트리거인지 단언한다(SQLSTATE P0001 + 메시지에 표 이름). → H2는 0003에 `TG_TABLE_NAME`을 쓰는 역사 전용 금지 함수를 만든다(0001의 `forbid_mutation()` 메시지는 'domain_events'가 하드코딩되어 있다).
- 수정 4 — Q-2: persistence DB 테스트 0개, CI에 postgres 없음 → 이대로면 SC-51~57·62가 CI에서 실행 0건으로 초록이 된다. 제안: `#[ignore]` 금지, DB URL이 없으면 SKIPPED 줄 출력, CI에 postgres + `STARFALL_REQUIRE_DB=1`(DB 없으면 panic), 판정 증거에 SKIPPED 수 = 0. 헬퍼는 H2에서 만들고 server와 공유한다.
- 보강 5~7: SC-41·51에 증거 레코드 단언 추가(starfall-glass fixture 기준 evidence_id `e9b611d1-637e-5287-862e-74eabffb0252`, Python으로 독립 계산). SC-50은 실제 규칙 파일의 sha256이 테스트 전후로 불변임을 증거에 넣는다. SC-59 재생 비교에 evidence를 포함한다.
- Q-3(conflict 정지를 항목으로): 지지. 판정은 architect.

### 11.4 스펙 AC-11(h) 신설 (qa Q-3, architect 통보 2026-09-27) — H2 대상

같은 dedupe_key에 다른 근거가 들어오면 러너만 정지하고 `history_conflicts_total`을 ≥ 1 올린다. 커서는 그 이벤트 앞에 남고, 역사 행 수는 불변이며, 채굴 커밋은 계속된다. 짝 항목: 같은 키·같은 근거의 재배달은 정지하지 않고 카운터도 올리지 않는다. 주입 지점 실행 카운터를 찍는다.
H2 구현 판별: ON CONFLICT로 0행이 되면 기존 행의 `source_event_ids`와 비교해, 같으면 조용히 커서를 전진하고 다르면 롤백 + 카운터 + 정지. 재배달 주입은 가변 `history_cursor`를 되감아 만든다. ADR-0014 §4의 "2·3 위반 = 버그 신호"를 두 경우로 나눠 적어 달라고 architect에게 요청했다.
- (후속) architect가 판별 기준을 승인하면서 넓혔다(ADR-0014 §4 70~73행). 비교 대상은 `source_event_ids`만이 아니라 **기록 내용 전체**(recorded_at만 제외)다. 같은 근거인데 내용이 다르면 golden이 놓친 규칙 변경이므로 충돌로 본다. evidence도 같은 규칙. 비교는 DB의 jsonb·배열 동등으로 한다.
- 주입 3종을 qa에게 전달했다. (i) 커서 되감기 재배달 → 조용함(양성 대조). (ii) 판정 상태를 비운 러너 → 다른 근거 충돌 → 정지. (iii) 같은 id·근거에 payload만 다른 행을 러너 처리 전에 SQL INSERT → 정지. append-only 트리거는 INSERT를 막지 않으므로 별도 스키마가 필요 없다.
- (후속) architect: 셋째 주입을 스펙 AC-11(h)(iii)으로 확정했다. jsonb 배열 동등은 순서를 보므로 코어는 participants를 늘 DISCOVERER → VESSEL 순서로 낸다(스펙에 명시). qa에게 SC-41 단언 추가를 요청했다.
- (후속) 리더 결정(server 전달): H2 DB 테스트는 `starfall-testdb`를 쓴다(`TestDb::create` / `drop`, 0003까지 적용됨, CI 필수 실행, `#[ignore]` 금지, 증거 DB 불가침; `02_server_ack.md` §3.2). §11.3 수정 4의 `STARFALL_REQUIRE_DB` 제안은 이것으로 대체된다. None 경로의 SKIPPED 줄 출력 주체를 server에게 확인 요청했다.
- (후속) server: `STARFALL_DB_TEST SKIPPED <name>` / `RAN <name>` 줄은 **헬퍼가 찍는다**. H2 테스트에서는 따로 찍지 않는다(중복 금지). `TestDb::create(name)`의 name은 계약에 적힌 테스트 이름과 **정확히 같게** 쓴다 — 게이트가 이 문자열로 대조한다.
- (후속, **위 두 줄을 대체**) 리더가 Q-2 방식을 하나로 통일했다: `#[sqlx::test]`(테스트마다 임시 DB를 만들고 지우며, 마이그레이션 0001~0003이 적용됨). DB가 없으면 **실패**하고 건너뛰지 않는다. SKIPPED 출력, `STARFALL_REQUIRE_DB`, `starfall-testdb` 헬퍼의 SKIPPED/RAN 규칙은 쓰지 않는다. CI에는 postgres 서비스가 붙고, 실행된 DB 테스트가 0건이면 게이트가 FAIL을 낸다. H2 역사 DB 테스트도 이 방식을 따른다. 주입(커서 되감기, 사전 INSERT)은 임시 DB 안에서 한다.
- (후속, 리더 정정) **`#[sqlx::test]` 통일 결정은 철회하고 `starfall-testdb`로 복귀한다**(`02_server_ack.md` §3.2). 그 앞의 server 합의 두 줄이 다시 유효하다: 헬퍼가 `STARFALL_DB_TEST RAN/SKIPPED <name>` 줄을 찍고, H2 테스트는 찍지 않는다. `TestDb::create(name)`의 name은 계약 테스트 이름과 글자 그대로 같게 한다. CI는 required 모드(DB가 없으면 실패)다. 주 DB(`starfall`)에 직접 붙는 테스트는 만들지 않는다.

### 11.5 계약 r1 확인 (2026-09-27)

- 수락: SC-105(충돌 → 정지, (iii) 주입을 ⊘에 추가 요청), SC-106(재배달 조용함, 커서 되감기와 다시 읽은 수 ≥ 1), SC-62(러너 → 채널 경계에서 커밋 뒤에만 send, 테스트가 Receiver 측에서 센다), SC-107(H2에서 pg_constraint를 실측해 architect에게 알린다. PG18은 NOT NULL도 contype 'n'으로 세므로 계수 기준 일치를 확인 요청).
- **r1에 미반영된 것(필수 4 + 답변 2)을 재송부했고 서명은 보류했다**: SC-47(자명 통과 → 러너 ORDER BY DB 항목), SC-95(테스트 제외 + 여러 줄 패턴), SC-52(P0001 + 표 이름), SC-41·51(participants 순서, 증거 단언), Q-4(H-15 = SC-57+SC-99), Q-2(testdb 확정 문구).

### 11.6 계약 r2 서명 (2026-09-27) — 태스크 #8 완료

r2에서 필수 4건(SC-47 러너 ORDER BY DB 항목, SC-95 여러 줄 패턴 + 테스트 제외, SC-52 P0001 + 표 이름, SC-41·51 순서·증거 단언), 보강(SC-50 규칙 파일 sha 전후 확인, SC-59 evidence 포함), AC-11(h) 3종(SC-105 · SC-106 · SC-111), H-15 → SC-57, Q-2 → §3.4(testdb)가 반영된 것을 행 단위로 확인했다. §11의 J·K·L·M(history)에 서명했다. history 몫은 21 + SC-111이다. H2의 DB 테스트 12개(SC-47·51~57·62·105·106·111)는 `TestDb::create(name)`의 name을 계약의 테스트 이름과 글자 그대로 같게 쓴다.

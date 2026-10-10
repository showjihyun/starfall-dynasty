# p1-05-basic-combat — 역사 연결 판단·검토 (history5)

- 상태: **v0 — 독립 판단.** architect5 스펙 초안·designer5 디자인 초안은 아직 없다(2026-10-10 시점 `docs/specs`·`docs/design` 에 p1-05 파일 없음). §5 는 초안 도착 뒤 채운다.
- 근거: CLAUDE.md 원칙 4·5·6·9, ADR-0014(전 절), ADR-0007 I-21·p1-01 개정, ADR-0011 §6(잔류), HSE §9–12·§60–63·§80–81·§84·§88–90, GDD §10·§12·§16–17·§36·§38·§43·§55, p1-02 디자인 §4(발견 규칙·증거), p1-02 스펙 Q4(인벤토리 actor 소유), p1-03 `02h_history_review.md`(R-1~R-9 형식), 코드 `server/crates/history/src/{core,input,output}.rs`, `server/crates/persistence/src/history.rs`(`load_discovered_set` 178·`decode_row` 950·`replay_and_compare` 799·삽입 SQL 515), `server/migrations/0003_history.sql`, `contracts/messages/HISTORICAL_EVENT_NOTICE.schema.json`, `contracts/registry/types.json`.

## 결론 (요약)

1. **역사가 되는 파괴는 하나뿐이다 — "역사적 인물의 첫 격추".** 새 규칙 `ship-destruction@1` 이 `SHIP_DESTROYED`(Historical, Level 1 제안, PUBLIC) 를 만든다. 조건: 다른 플레이어가 결정타를 낸 파괴이고, 피격 조종사가 **그 시점의 history 자신의 판정 상태에서** 이미 `MINERAL_DISCOVERED` 의 DISCOVERER 다. 같은 조종사에 대해 월드당 **한 번**(`dedupe_key = "{victim_actor_id}"`). 나머지 모든 파괴는 Level 0 도메인 이벤트로만 남는다(원칙 4).
2. **`PLAYER_DIED` 는 이 슬라이스에서 만들지 않는다.** 재생성되는 조종사는 죽지 않는다. GDD §16·HSE §84 의 `PLAYER_DIED` 는 캐릭터의 죽음(legacy 로 이어지는)이고, 지금 그 이름으로 "재생성되는 격추"를 기록하면 원칙 5 때문에 그 뜻을 영원히 고칠 수 없다. 이름은 캐릭터 사망·legacy 슬라이스를 위해 남겨 둔다.
3. **사실은 "결정타와 피해 기여"까지다.** "A 가 B 를 매복했다/먼저 도발했다/약탈했다/정당방위였다" 는 Claim(p2), "학살·영웅적 저항" 은 Interpretation 이다. 사실 테이블과 payload 에 `aggressor`·`intent`·`crime`·`justified` 같은 열을 만들지 않는다.
4. **현재 러너는 두 번째 역사 타입을 넣는 순간 깨진다**(코드 확인, §3.1). 특히 `load_discovered_set` 이 `event_type` 을 거르지 않고 모든 `dedupe_key` 를 `"성계/광물"` 로 파싱해, `SHIP_DESTROYED` 1 행이 생긴 뒤 **재기동하면 러너가 Corrupt 로 기동하지 못한다.** p1-05 에는 history 구현 태스크(코어 일반화 + 러너 일반화 + NOTICE union)가 생긴다. `mineral-discovery@1` 의 결과·golden 은 바이트 불변이어야 한다.
5. 미래 규칙(현상금·복수·전투 집계·잔해 조사)이 **도메인 이벤트 payload 만으로** 판정할 수 있도록 파괴 도메인 이벤트에 필드 15 개를 요구한다(§4). 특히 `victim_was_lingering`(오프라인 격추 여부)·피해 기여 목록·목격자는 지금 싣지 않으면 영원히 복원할 수 없다.

---

## 1. 어떤 파괴·사망이 Historical Event 인가

### 1.1 판단 기준

p1-03 과 같다 — historical-engine §8 의 세 질문(누가 이 기록을 발견하나 / 발견한 사람이 무엇을 할 수 있나 / 그 행동이 다시 도메인 이벤트를 만드나)과 HSE §80 "History = Noise" 경계. 여기에 전투 특유의 둘을 더한다.

- **담합으로 찍어낼 수 있는가.** 두 친구가 서로를 반복 격추하면 기록이 몇 개 생기나. 상한이 플레이어 행동 횟수에 비례하면 역사가 아니라 킬로그다.
- **BACKFILL 상한(ADR-0014 §6, 32 건) 안에 묶이는가.** 넘으면 조회 API·Chronicle(p2)이 먼저 와야 한다 — 그건 이 슬라이스 범위가 아니다.

### 1.2 후보와 판정

| 후보 | 판정 | 이유 |
|---|---|---|
| 모든 PvP 파괴 | **Level 0** (도메인 이벤트만) | 수가 교전 횟수에 비례한다. 30 명 세션 1 시간에 수십~수백 건, BACKFILL 32 를 바로 넘는다. HSE §9 의 "10 톤 채굴 = 일반 이벤트" 와 같은 자리다 |
| 성계 첫 격추("first blood") | **기록 안 함** | 월드당 1 건이라 소음은 아니지만 플레이테스트 월드에서는 첫 1 분 안에 시험 결투가 가져간다. 다른 플레이어가 그 흔적으로 할 수 있는 행동이 없다(§8 질문 2). p1-03 의 `FIRST_SALE` 기각과 같은 논리 |
| 플레이어 쌍별 첫 격추 | **기록 안 함** | 상한 N² (100 명이면 9,900). 담합으로 손쉽게 찍힌다 |
| 화물/고가치 손실이 있는 파괴 | **보류 — designer5 의 손실 규칙에 달림** | GDD §55 의 "B 가 광물을 가져간다" 장면의 핵심이지만, p1-02 Q4 로 인벤토리는 actor 소유·함선 손실과 무관하다. designer5 가 "파괴 시 화물 손실/잔해 투하" 를 넣으면 다시 본다. 그 경우에도 문턱값(kg·가치)은 데이터가 아니라 **payload 에 실린 손실량**으로 판정해야 한다(ADR-0014 §5) |
| **역사적 인물의 첫 격추** | **채택 — `ship-destruction@1`** | §1.3 |
| 다수 함선이 얽힌 교전(Battle) | **p2 — 집계 규칙** | HSE §60–62 의 Aggregate Event. Correlation Window 가 필요하고 ADR-0014 "다시 검토할 조건 (a)" 를 연다. 1 대 1 슬라이스에서는 조건이 발생하지 않아 테스트가 `0 == 0` 이 된다 |
| `PLAYER_DIED` | **이번엔 생산하지 않음** | §1.4 |
| 환경·자폭·경계 이탈 파괴 | Level 0 | 공격자가 없다. 규칙이 `attacker_actor_id` 가 null 이거나 피격자와 같으면 즉시 빈 결과 |

### 1.3 채택 규칙 — `ship-destruction@1`

**조건(명시적 Rust, 데이터는 출력 상수만):**

1. 입력이 파괴 도메인 이벤트다(이름은 architect5 — §4.1 의 명명 제약).
2. `attacker_actor_id` 가 존재하고 `victim_actor_id` 와 다르다.
3. 이 월드의 판정 상태에서, **이 이벤트의 `(tick, sequence)` 보다 앞서 판정된** `MINERAL_DISCOVERED` 중 `victim_actor_id` 가 DISCOVERER 인 것이 1 개 이상 있다(= 피격자가 역사적 인물이다).
4. 이 월드에서 같은 `victim_actor_id` 로 `SHIP_DESTROYED` 가 아직 없다.

**출력:**

| 항목 | 값 |
|---|---|
| `event_type` | `SHIP_DESTROYED` (HSE §89 10 종 안) |
| `rule_version` | `ship-destruction@1`, 규칙 파일 `data/history/rules/ship-destruction.json`(designer5 소유, 스키마 `SIGNIFICANCE_RULE` 그대로) |
| `dedupe_key` | `"{victim_actor_id}"` — id = UUIDv5(NS_HISTORY, `"{world_id}|SHIP_DESTROYED|{victim_actor_id}"`). `ship_id` 를 키로 쓰지 않는다: 함선은 재생성마다 새 id 라(ADR-0011 §6) 담합 반복 격추가 매번 새 기록이 된다 |
| `importance_level` | **1 제안**(Local). 발견(2)보다 가볍게 — 발견이 원인이고 격추가 그 인물의 이야기의 한 줄이다. designer5 결정 |
| `visibility` | PUBLIC (발견과 같은 전달 경로) |
| `source_event_ids` | `[파괴 도메인 이벤트 event_id]` — 정확히 1 개 |
| `participants` (순서 고정) | `[피격 조종사 PLAYER, 피격 함선 SHIP, 결정타 조종사 PLAYER, 결정타 함선 SHIP]`. 역할 이름은 **중립**으로 — 예 `LOST_PILOT`·`LOST_VESSEL`·`ATTACKER`·`ATTACKER_VESSEL`. `VICTIM`·`KILLER`·`MURDERER`·`PIRATE` 는 해석을 품으므로 쓰지 않는다(원칙 6) |
| `location` | `{star_system_id}` (+ 위치는 payload) |
| `payload` | `{ weapon_id, position_mm, notable_for: [historical_event_id…] }` — `notable_for` 는 조건 3 을 만족시킨 `MINERAL_DISCOVERED` id 들(오름차순). **왜 이 파괴가 역사인지**를 기록 자신이 말하게 한다(원칙 9 감사 가능성). 표현 계층은 이것으로 "Starfall Glass 의 발견자가 쓰러졌다" 를 만든다 |
| 증거 | `SHIP_LOG` 1 건, `source_entity_id = 피격 함선`, `PARTICIPANTS_ONLY` — 발견과 같은 비대칭(사건은 공개, 원본 로그는 당사자). GDD §55 "C 가 A 의 함선 로그를 발견한다" 의 자리다(p2: 잔해에서 로그 회수 = 새 증거, `derived_from`) |

**이 규칙을 고른 이유:**

- **GDD §55 의 한 장면과 같은 모양이다** — "A 가 희귀 광물을 발견 → B 가 A 를 공격 → A 의 함선 파괴 → Chronicle 기록". GDD §16 "**중요 인물의** 사망은 역사적 사건이 될 수 있다" 의 문장 그대로다.
- **역사 → 역사 연결의 첫 사례다.** 발견이 그 사람을 표적으로 만들고, 표적이 쓰러진 것이 다시 역사가 된다. p1-02~03 이 쌓은 기록이 전투에 의미를 준다 — "다른 플레이어가 흔적을 발견하는가" 의 답이 "발견자를 찾아가 격추한다" 로 플레이가 된다.
- **결정적이다.** 판정 입력은 도메인 이벤트 + history 자신의 앞선 판정 결과뿐이다(ADR-0014 §1 이 명시적으로 허용한 입력). 라이브 `WorldState` 도, sim 의 "인물 여부" 도 읽지 않는다. sim 은 누가 역사적 인물인지 몰라도 된다 — 알면 안 된다(p1-03 R-9 와 같은 선: 판정이 커밋 뒤 비동기인 역사 표를 읽으면 결정성이 깨진다).
- **상한이 있다.** 월드당 ≤ 발견자 수 ≤ (성계 × 광물) = 4. 발견 4 + 격추 4 = PUBLIC 8 건 < 32 — BACKFILL 경로 그대로 유지(ADR-0014 §6 다시 볼 조건 (b) 발동 안 함).
- **담합해도 얻는 게 1 건이다.** 친구 부계정이 발견자를 100 번 격추해도 기록은 하나. 그 하나를 조작으로 찍는 것은 막지 못하지만, 그건 p2 Claim("연출된 격추였다")의 자리다(GDD §44·HSE §79).

**규칙 간 의존 — 반드시 ADR 에 적을 것:**

- `ship-destruction@1` 의 "역사적 인물" 정의는 **`MINERAL_DISCOVERED` 의 DISCOVERER 참가자 집합**이다(어느 `mineral-discovery@n` 이 만들었든). 그래서 발견 규칙이 `@2` 로 바뀌면 같은 입력에서 격추 결과가 달라질 수 있다 — golden 이 이 의존을 묶어야 한다(§3.3 H-47).
- 두 규칙은 **한 코어·한 소비자 커서·한 번의 `(tick, sequence)` 순회**로 판정한다. 규칙마다 소비자를 나누면 격추 판정 시점에 발견 판정이 어디까지 왔는지가 러너 지연에 달린다 — 같은 로그에서 다른 역사(원칙 9 위반).
- 같은 이벤트에 여러 규칙이 걸릴 일은 지금 없지만(입력 타입이 다르다), 규칙 평가 순서는 코드에 고정 목록으로 둔다(`HashMap` 순회 금지).
- 격추가 공격자를 역사적 인물로 만들지 **않는다**(@1). 만들면 "인물 격추 → 격추자가 인물 → 그를 격추 → …" 사슬이 생기고 상한이 사라진다. 현상금(p2)이 그 사슬을 설계된 형태로 다룬다.

### 1.4 `PLAYER_DIED` 를 지금 만들지 않는 이유

- 이 슬라이스의 파괴는 **함선 파괴 → 같은 actor 재생성**이다. 캐릭터는 죽지 않는다. `PLAYER_DIED` 를 여기 쓰면 같은 사실(한 번의 격추)에 역사 기록이 둘(`SHIP_DESTROYED`+`PLAYER_DIED`) 생긴다.
- GDD §16·HSE §84 의 `PLAYER_DIED` 는 Biography·legacy·Faction 반응으로 이어지는 **캐릭터 사건**이다. 지금 "재생성되는 격추" 의 이름으로 쓰면, 캐릭터 사망이 생기는 날 옛 기록과 새 기록이 같은 이름으로 다른 뜻을 갖는다 — 원칙 5 로 옛 기록을 고칠 수 없다.
- **designer5 가 "조종사 사망"(재생성 = 새 캐릭터, 또는 사망 표지)을 이번에 넣기로 하면 다시 연다.** 그 경우에도 기록은 하나로 합친다(격추 기록의 참가자 역할로 사망을 표시하거나, `PLAYER_DIED` 단독) — 같은 근거 이벤트에서 역사 둘을 만들지 않는다.
- 도메인 쪽에도 "조종사 사망" 이벤트를 따로 만들 필요가 없다. 파괴 이벤트가 `victim_actor_id` 를 싣는다.

### 1.5 다른 플레이어가 흔적을 발견하는가

두 층으로 나눈다. **역사가 아니어도 되는 흔적이 대부분이다.**

| 층 | 무엇 | 성질 |
|---|---|---|
| 시뮬레이션 상태(Fact) | **잔해(wreck)** — 파괴 위치에 일정 tick 남는 오브젝트. 근처의 누구나 본다 | 사실이고 역사 판정을 거치지 않는다. 모든 파괴가 남긴다. p1-03 §1.3(눌린 시세)과 같은 자리. 수명·영속 여부는 designer5·architect5. 재기동에 사라져도 된다(정본 기록은 도메인 이벤트) |
| 역사(Historical) | 인물 격추 `SHIP_DESTROYED` — 월드 전체 NOTICE, 재접속 BACKFILL | 상한 4 |
| 감사(도메인) | 모든 파괴의 도메인 이벤트 | 플레이어에게 직접 보이지 않음. p2 Chronicle·집계·Biography 의 원천 |

- 잔해 오브젝트가 역사 id 를 들고 다니게 하지 않는다(sim 이 역사를 읽지 않는다). 클라이언트가 둘을 잇고 싶으면 잔해 id = 파괴된 `ship_id` 로 두고, NOTICE 의 `LOST_VESSEL` 참가자와 맞춘다 — 표현 계층의 결합이다.
- GDD §55 "3 일 후 사건 현장 조사" 는 잔해 영속 + 증거 회수(p2)가 필요하다. 이번엔 **그 문을 닫지 않는 것**까지만: payload 에 위치·함선 id·증거 행이 있으면 p2 가 "이 좌표의 잔해에서 이 증거를 회수" 를 새 레코드로 붙일 수 있다.

### 1.6 후속 플레이 연결(p2) — 지금 payload 가 열어 두는 것

| p2 시스템 | 무엇을 읽나 | 지금 필요한 것 |
|---|---|---|
| 현상금(GDD §12) | "누가 먼저 쐈나", 안전 등급 구역인가, 오프라인 격추인가 | `damage_contributions[].first_hit_tick`, `security_tier`, `victim_was_lingering` |
| 복수(GDD §17·HSE §84) | 피격자 → 결정타자 관계 | 참가자 역할, `attacker_actor_id`. **복수를 강제하지 않는다** — 관계는 사실로만 남기고 "원한" 같은 상태를 만들지 않는다 |
| Biography(GDD §15) | 두 사람의 한 줄씩 | 이 기록의 participants(PLAYER 2 명) |
| Claim(HSE §19) | "매복", "도발", "정당방위", "연출된 격추" | 사실은 결정타·피해 기여·tick 까지. 주장은 그 사실을 `about_event_id` 로 가리킨다 |
| 전투 집계(HSE §60–62) | 여러 파괴를 창으로 묶음 | 각 파괴의 `engagement_started_tick`(= 기여자들의 최초 피격 tick 최솟값)과 위치. 집계 규칙이 휴리스틱 창 대신 사실로 묶을 수 있다 |
| 잔해 조사·증거 회수 | 잔해 좌표·함선·화물 | `position_mm`, `victim_ship_id`, 손실 화물 목록(있다면) |

**소급 기본값(p1-03 §3.1 과 같음):** p2 의 새 규칙은 활성화 좌표에서 커서를 시작하고 소급하지 않는 것을 기본으로 한다. 그래서 payload 를 지금 완전하게 두는 것이 선택지를 남기는 가장 싼 방법이다.

---

## 2. Fact / Claim / Interpretation

"A 가 B 를 격추했다" 는 **서버가 판정한 사실**이다. 단, 어디까지가 사실인지 정확히 자른다.

| 문장 | 층 | 저장 위치 | 근거 |
|---|---|---|---|
| B 의 함선 S 가 tick T 에 선체 0 이 되어 파괴됐다 | Fact | 파괴 도메인 이벤트 | 서버 판정(원칙 1) |
| 결정타는 A 의 함선 S′ 가 무기 W 로 냈다 | Fact | 같은 이벤트 `attacker_*`, `weapon_id` | 서버 판정. "격추자 = 결정타" 는 **전투 규칙의 정의**이므로 `combat_version` 으로 함께 기록 |
| A 가 B 에게 총 X, C 가 Y 의 피해를 냈다 | Fact | `damage_contributions[]` | 결정타 정의가 나중에 바뀌어도(최다 피해자 등) 사실은 그대로 남는다 |
| A 의 첫 피격이 B 의 첫 피격보다 먼저였다 | Fact | 각 기여의 `first_hit_tick` | "누가 먼저 **맞혔나**" 까지만. 조준·접근·위협은 기록되지 않으므로 |
| **A 가 먼저 도발했다 / 매복했다** | Claim | p2 `claims` | 접근·의도는 사실 테이블에 없다. 첫 명중 순서로 "도발" 을 사실화하면 빗나간 첫 사격, 회피 기동 같은 맥락이 지워진다 |
| B 가 오프라인(잔류) 중에 격추됐다 | Fact | `victim_was_lingering` | 서버 상태(ADR-0011 §6) |
| **"비겁한 격추였다"** | Interpretation | p2 | 위 사실에 대한 평가 |
| A 가 B 의 광물을 빼앗았다 | Fact(손실·획득이 있다면) | 손실/약탈 도메인 이벤트 | 이동한 수량은 사실. "약탈"·"해적질" 이라는 **이름**은 해석 — 이벤트 이름도 중립으로(`CARGO_TRANSFERRED`/`CARGO_LOST` 류) |
| **A 는 해적이다 / 이 격추는 정당했다** | Interpretation / 판결 | p2(현상금·평판) | 평판 ≠ 진실(HSE §52) |
| "Starfall Glass 의 발견자가 쓰러졌다" | 표현 | 클라이언트 | 기록의 `notable_for` + 로컬라이즈 키. 사실 테이블에 문장 열 없음(ADR-0014 §3) |

**구조 요구:**

- 역사 기록·도메인 payload 에 `aggressor`·`hostile_intent`·`crime`·`justified`·`piracy` 같은 필드를 두지 않는다. `fact_status` 는 계속 `CONFIRMED` 하나(0003 의 CHECK 그대로) — 격추 사실에 "논쟁 중" 상태를 붙이고 싶어지는 순간이 Claim 테이블을 만들 때다.
- 참가자 역할 이름도 중립(§1.3). 역할 enum(`HistoricalRole`)은 닫힌 집합이므로 늘리는 것이 계약 변경이다.

---

## 3. 기존 러너·`mineral-discovery@1`·재생에 미치는 영향

### 3.1 코드 확인 — 두 번째 역사 타입을 막는 하드코딩

p1-03 은 새 타입이 `Unjudged` 로 통과하는 것만 확인하면 됐다. p1-05 는 **두 번째 판정 대상 + 두 번째 역사 타입**이다. 현재 코드는 단일 규칙·단일 출력 타입을 전제한다.

| 위치 | 현재 | 영향 | 등급 |
|---|---|---|---|
| `persistence/src/history.rs:178` `load_discovered_set` | `SELECT dedupe_key FROM historical_events WHERE world_id = $1` — **`event_type` 필터 없음**, 모든 키를 `"성계/광물"` 로 split·파싱 | `SHIP_DESTROYED` 1 행(키 `"{uuid}"`, `/` 없음)이 생긴 뒤 **재기동하면 `HistoryError::Corrupt` 로 러너 기동 실패** — 역사 영구 정지(경제는 돈다). 첫 기동 중에는 드러나지 않고 재기동에만 드러난다 | **치명** |
| `history.rs:515` 삽입 SQL | `event_type` 리터럴 `'MINERAL_DISCOVERED'` | 새 기록이 틀린 타입으로 저장되거나(그러면 UNIQUE 키 공간이 섞인다) 별도 경로가 필요 | 높음 |
| `history.rs:66·253·292·407` | `initial_backfill: Vec<MineralDiscoveredEvent>`, `live_tx: mpsc::Sender<MineralDiscoveredEvent>` | 전달 채널이 타입 하나 | 높음 |
| `history.rs:799` `replay_and_compare` | `HistoryCore::new(rule)` 단일 규칙, `row_to_mineral_discovered`, 반대 방향 검사가 저장된 모든 행을 대상 | 새 타입 행이 저장돼 있으면 "재생에서 안 나온 행" 으로 **불일치**(좋은 실패) — 일반화하지 않으면 재생 도구가 p1-05 월드에서 항상 FAIL | 높음 |
| `history.rs:950` `decode_row` | `MINERAL_MINED` 아니면 `unjudged` | 파괴 도메인 이벤트를 판정 대상으로 올려야 한다. 올리면 그 타입의 모르는 `schema_version` 은 fail-stop(ADR-0014 §1)으로 바뀐다 — p1-03 H-32 의 "판정 대상 아님 → 안 멈춤" 짝이 이 타입에서는 "멈춤" 이 된다 | 필수 변경 |
| `history/src/core.rs` | `HistoryCore::new(rule: SignificanceRuleTable)` 1 개, `judge → Vec<MineralDiscoveredRecord>`, `WorldState.discovered: HashSet<(성계, 광물)>` — **발견자 actor 를 들고 있지 않다** | 규칙 집합·출력 합 타입·발견자 집합(재구축 포함)이 필요 | 필수 변경 |
| `contracts/messages/HISTORICAL_EVENT_NOTICE.schema.json` | `historical_event` 가 `MINERAL_DISCOVERED` 직접 참조 + **"두 번째 역사 타입을 넣는 슬라이스가 판별 union 으로 바꿔야 한다"** 만기 표시 | **p1-05 가 그 슬라이스다.** `schema_version` 2 + 클라이언트가 모르는 `event_type` 을 받았을 때의 동작(무시 + 로그) | 필수 계약 변경 |
| `contracts/registry/types.json` | 이름 공간이 kind 를 가로질러 하나(`MINERAL_MINED`/`MINERAL_DISCOVERED` 처럼 도메인·역사 이름이 다르다) | 도메인 이벤트를 `SHIP_DESTROYED` 로 부르면 역사 타입(HSE §89 이름)과 충돌. HSE §62 가 `SHIP_DESTROYED` 를 도메인 예시로도 쓰지만 레지스트리는 둘을 허용하지 않는다 → §4.1 | 명명 |

→ **ADR 필요(architect5)**: ADR-0014 개정 또는 새 ADR "두 번째 규칙 — 규칙 집합·규칙 간 의존·NOTICE union". ADR-0014 "다시 검토할 조건 (a)"(규칙 > 5 또는 집계)는 발동하지 않는다 — 코어 모양(이벤트 1 건 → 결과 0..n)은 그대로 둘 수 있다. 바뀌는 것은 출력 타입이 합 타입이 되는 것뿐이다.

### 3.2 같은 tick 의 여러 이벤트·순번·판매와의 상호작용

| 상황 | 요구 | 이유 |
|---|---|---|
| 한 파괴가 여러 도메인 이벤트를 낸다(파괴 + 디스폰 + 화물 손실 + 재생성) | 각자 고유 `sequence`, **파괴가 먼저**, 나머지의 `causation_id` = 파괴 `event_id` | p1-03 R-2. 재생성 `SHIP_SPAWNED.causation_id = 파괴` 는 이 프로젝트의 첫 "죽음 → 재생" 인과 사슬이다 |
| **I-41 짝 불변식** (`SHIP_SPAWNED` 수 == `SHIP_DESPAWNED` 수, `ship_id` 기준) | 파괴된 함선의 종결 이벤트를 정한다: (가) 파괴 뒤 `SHIP_DESPAWNED{reason: DESTROYED}` 를 함께 낸다, 또는 (나) 파괴가 종결 이벤트이고 I-41 을 `SPAWNED == DESPAWNED + DESTROYED` 로 고친다. history 는 (가)를 권한다 — 기존 qa SQL·불변식이 그대로 살고, 디스폰은 판정 대상이 아니다 | 고치지 않으면 p1-01 AC-20(a) 가 p1-05 월드에서 FAIL |
| 같은 tick 상호 격추 | sim 이 **동시 해석**(그 tick 의 모든 사격을 먼저 계산하고 피해를 한꺼번에 적용)할지, 제출 순번으로 먼저 쏜 쪽만 살릴지 designer5·server 가 정한다. history 는 어느 쪽이든 결정적이면 된다. 둘 다 파괴되면 두 파괴 이벤트가 서로를 `attacker` 로 가리키고 sequence 로 정렬된다 — 각자 독립 사실, 각자 판정 | 순번으로 생사를 가르면 "명령을 먼저 보낸 쪽이 산다" 는 숨은 규칙이 역사에 새겨진다 — 디자인 결정으로 드러내 두라 |
| 파괴된 tick 에 같은 함선의 `MINE_RESOURCE`·`SELL_MINERAL` | tick 안 단계 순서(전투 해석 → 그 뒤 명령 처리, 또는 반대)를 스펙에 고정하고, 파괴 뒤의 함선 명령은 거부. 거부 사유 코드 | 파괴 뒤 채굴이 `MINERAL_MINED` 를 내면 **죽은 함선이 발견자**가 될 수 있다 |
| 화물 손실·약탈(넣는다면) | **약탈 획득은 `MINERAL_MINED` 가 아니다.** 손실은 `MINERAL_SOLD` 도 아니다. 별도 타입 | p1-03 R-3 의 전투판: 약탈자가 그 광물의 **최초 발견자로 영구 기록**될 수 있다 |
| 보존 법칙(p1-03 I-55/I-84) | 손실·약탈을 넣으면 `Σ 인벤토리 + Σ 광맥 잔량 + Σ SOLD (+ Σ 잔해 화물) = Σ 초기 + Σ 회복` 에 손실 항을 더한다 | 안 고치면 p1-02 AC-5(a)·p1-03 보존 AC 가 FAIL |
| 사격·명중 하나하나 | **도메인 이벤트로 남기지 않기를 권한다** — 파괴 이벤트가 피해 요약(`damage_contributions`)을 싣는다 | 발사율 × 인원 만큼 행이 쌓이는 시계열은 ADR-0007 I-21(위치 시계열 금지)과 같은 성격. 역사는 개별 명중이 필요 없다 — 요약이 payload 에 있으면 충분하다. 감사 필요 여부는 architect5·server 판단 |
| 피격 함선이 잔류(오프라인) 중 | 파괴 가능 여부는 디자인. 가능하면 `victim_was_lingering: true` | §1.6 |
| 러너 지연 예산(ADR-0014 §7) | 파괴 커밋 → NOTICE(LIVE) p95 ≤ 1 s 측정 | 격추 직후 "인물이 쓰러졌다" 배너가 두 박자로 와야 한다 |

### 3.3 history 쪽 테스트 요구 (Phase 3~4 history 구현 태스크 — 계약 단계에서 qa 와 맞춘다)

⊘ = 겨냥한 조건이 실제로 일어났음을 먼저 단언(CLAUDE.md 검증의 규율).

| ID | 층 | 입력 | 단언 |
|---|---|---|---|
| **H-40** | 코어 | 발견(A, t=10) → A 격추(공격 B, t=20) | ⊘ 입력에 발견 1·파괴 1. `SHIP_DESTROYED` 1 건, `notable_for = [발견 id]`, participants 순서 고정, id = UUIDv5 공식 |
| **H-41** | 코어(음성) | 비인물 C 격추(공격 B) | ⊘ 파괴 이벤트 1. 역사 0 건 — **양성 대조 H-40 과 같은 테스트 파일·같은 코어 인스턴스에서** |
| **H-42** | 코어 | A 격추 ×3(재생성 사이) | ⊘ 파괴 3. 역사 1 건(첫 번째 근거) |
| **H-43** | 코어(순서) | 같은 tick: A 의 발견 seq=5, A 격추 seq=3 | 격추가 **먼저**이므로 역사 0, 발견 1. 순서를 뒤집으면 1·1. "인물 여부는 그 좌표의 상태" 를 고정 |
| **H-44** | 코어 | 공격자 null / 공격자 = 피격자 | ⊘ 파괴 2. 역사 0 |
| **H-45** | 코어(결정성) | H-40~44 를 섞은 입력열을 배치 경계를 바꿔 여러 번 | 바이트 동일(SC-46 과 같은 방식) |
| **H-46** | DB 러너 | 새 월드에 발견 + 인물 격추 커밋 → 러너 1 회 → **러너 재기동** | ⊘ `historical_events` 에 두 타입 각 1 행. 재기동 성공(현재 코드에서 **RED**: `load_discovered_set` Corrupt — 수정 전 실패를 먼저 보인다), 재기동 뒤 같은 A 재격추 → 역사 추가 0(재구축된 상태가 "이미 격추됨" 을 안다) |
| **H-47** | golden | `ship-destruction@1` golden: 규칙 파일 해시 + **발견 이벤트가 섞인** 고정 입력열 출력 | 기존 `mineral-discovery@1` golden 파일 **sha256 불변**. 새 golden 은 `STARFALL_REPLAY_BLESS=1` 로만 생성 |
| **H-48** | 재생 | H-46 월드에 `history-replay` | 일치. 출력에 역사 타입별 재생 수(`MINERAL_DISCOVERED ≥ 1`, `SHIP_DESTROYED ≥ 1` — 0 이면 FAIL) |
| **H-49** | DB 러너(fail-stop) | 파괴 도메인 이벤트 `schema_version = 2` | 러너 정지, 커서 그 앞, `history_detector_halted` +1. p1-03 H-32(판정 대상 아님 → 안 멈춤)의 짝 |
| **H-50** | DB 러너(멱등) | 같은 파괴를 두 번 판정(커서 되감기) | 두 번째는 조용한 재처리 — 행 추가 0, `history_conflicts_total` 0 |
| **H-51** | E2E(qa, 봇) | 봇 A 가 새 광물 채굴(발견) → 봇 B 가 A 격추 → 봇 B 가 비인물 C 격추 → A 재격추 | ⊘ 파괴 도메인 이벤트 3. `SHIP_DESTROYED` NOTICE(LIVE) 정확히 1 건, 모든 세션 수신, 지연 p95 기록. 새 세션 BACKFILL 에 발견 1 + 격추 1 |

---

## 4. 파괴 도메인 이벤트가 담아야 할 필드

원칙(p1-03 §1.5): 판정은 payload 만 읽는다. 미래 규칙에 필요한 "그 시점의 사실" 이 없으면 **그 이전 파괴는 영원히 그 판정에서 빠진다.**

### 4.1 명명 제약

- 레지스트리 이름은 kind 를 가로질러 유일하다. 역사 타입은 HSE §89 의 `SHIP_DESTROYED` 를 쓰므로, **도메인 이벤트는 다른 이름**이어야 한다(`MINERAL_MINED`→`MINERAL_DISCOVERED` 처럼). 이름은 architect5 — 중립적인 이름이면 된다(예: `SHIP_HULL_BREACHED`·`SHIP_LOST`). "KILLED"·"MURDERED" 는 피한다.
- 레지스트리 `consumers` 에 **history 를 넣는다**(이번엔 판정 대상이다 — p1-03 확인 3 의 반대 경우). `MINERAL_MINED` 와 같다.

### 4.2 필드

| 필드 | 필수 | 이유 |
|---|---|---|
| envelope `actor_id` | 필수(비-null로 좁힘) | 결정타 조종사로 둘지 피격자로 둘지 architect5 가 정하되, **payload 에 둘 다 명시**해 envelope 해석에 기대지 않는다 |
| envelope `causation_id` | 필수 | 결정타를 낸 사격 명령/이벤트 |
| `victim_actor_id`, `victim_ship_id` | 필수 | 판정 조건 3·4, 참가자 |
| `attacker_actor_id`, `attacker_ship_id` | nullable(환경 파괴) | 판정 조건 2 |
| `weapon_id` | 필수(공격자 있을 때) | Biography·집계. 무기 데이터 값(피해량 등)은 싣지 말고 id 만 — 판정 입력에 데이터 표를 넣지 않는다(ADR-0014 §5) |
| `star_system_id` | 필수 | location. 데이터 조회로 얻지 않는다(p1-03 C-1 과 같은 이유) |
| `position_mm`(정수 3 성분) | 필수 | 잔해·조사·집계. 양자화 정수(ADR-0009) — `f64` 를 payload 에 싣지 않는다 |
| `damage_contributions[]` = `{actor_id, ship_id, damage_total, hit_count, first_hit_tick, last_hit_tick}`, `actor_id` 오름차순 | 필수(1 대 1 이면 1 원소) | 결정타 정의가 바뀌어도 사실 보존(§2), 현상금의 "먼저 맞힌 자", 집계 창. 정렬 기준 고정(결정성) |
| `engagement_started_tick` | 필수 | 기여자 `first_hit_tick` 최솟값. p2 집계의 묶음 기준 |
| `hull_max` 또는 피격자 함선 클래스 id | 필수 | "격추에 든 피해량" 의 분모. 데이터 값이 바뀌어도 그때 값 |
| `victim_was_lingering` | 필수 | 오프라인 격추 — 미래 규칙(오프라인 격추는 역사·현상금에서 제외 등)이 반드시 볼 축. 세션 상태는 역사 쪽에 다른 원천이 없다 |
| `security_tier`(성계/구역 안전 등급) | 데이터에 등급이 있으면 필수 | GDD §43 위험 지역 계층. p1 은 1 성계라 상수여도 싣는다 — 등급 체계가 생긴 뒤의 판정이 옛 파괴를 다르게 보지 않게 |
| `witnesses[]` = 파괴 tick 에 반경 R 안의 다른 함선 `{actor_id, ship_id}`, 오름차순, 상한 N | 권장 | GDD §16 "Witnesses". p2 Claim 의 신뢰성(목격자 주장 vs 비목격자 주장)을 가르는 사실이고, 그 tick 의 위치 시계열은 저장하지 않으므로(I-21) **지금 아니면 복원 불가**. R·N 은 designer5, 값 자체는 payload 에 `witness_radius_mm` 로 함께 |
| 손실·잔해 화물 `[{mineral_id, quantity_kg}]` (손실 규칙이 있다면) | 조건부 | 약탈·잔해 조사. 손실이 별도 이벤트라면 그쪽에 |
| `combat_version`(전투 규칙 식별 문자열) | 필수 | "결정타·피해" 의 정의를 감사. p1-03 `pricing_version` 과 같은 역할 |

**싣지 않는 것:** 공격 의도·적대 표지·범죄 판정(§2), 피격자의 지갑·인벤토리 전체(경제 감사 쪽 판단), 표시 이름(ADR-0014 §6 — 클라이언트 파생).

---

## 5. architect5·designer5 초안 검토

초안 도착 대기. 도착하면 이 절에 표로 붙인다. 확인할 것:

- architect5: §3.1 의 하드코딩 목록이 history 태스크로 분해됐는가, NOTICE union(`schema_version` 2) 과 클라이언트의 모르는 타입 처리, 명명(§4.1), I-41 처리(§3.2), tick 안 단계 순서, 약탈 ≠ `MINERAL_MINED` 불변식, ADR(규칙 집합·규칙 간 의존·단일 소비자), §4.2 필드.
- designer5: §1.3 규칙(또는 대안과 그 이유), importance 1, 역할 이름 중립, `PLAYER_DIED` 미생산(또는 사망 표지를 넣는다면 하나로 합침), 손실·잔해 규칙, 동시 격추 해석, 오프라인 격추 가능 여부, 목격자 R·N, "인물 격추" 배너의 UI-nn 표와 발견 배너와의 무게 차.

## 6. 최종 결론 (v0)

- **새 Historical Event 1 종: `SHIP_DESTROYED`** (`ship-destruction@1`, 역사적 인물(발견자)의 첫 격추, 월드당 인물당 1 건, Level 1 제안, PUBLIC, 상한 4). `PLAYER_DIED` 는 이 슬라이스에서 생산하지 않는다.
- 일반 파괴는 Level 0 도메인 이벤트 + 시뮬레이션 상태(잔해)로 흔적을 남긴다.
- `mineral-discovery@1` 의 출력·golden 은 바이트 불변이어야 한다. 그러나 **러너는 지금 상태로 두 번째 역사 타입을 견디지 못한다**(§3.1, 특히 재기동 Corrupt) — history 구현 태스크 필요, 테스트 H-40~H-51.
- 남은 결정(다른 팀원): 도메인 이벤트 이름·I-41 처리·tick 단계 순서(architect5), 중요도·손실 규칙·동시 격추·목격자 반경(designer5).

# STARFALL DYNASTY
# Historical Simulation Engine — 실제 개발 가능한 상세 설계 및 전문가 비판 리뷰

> Version: 0.1  
> 목적: 플레이어 행동 → 세계 상태 변화 → 역사적 사건 → 증거 → 해석 → 후속 사건의 연결을 실제 게임 서버에서 구현하기 위한 설계 문서
>
> 핵심 원칙:
>
> **Simulation이 사실을 만들고, History Engine이 그 사실의 역사적 의미를 구조화하며, 플레이어가 그 의미를 조사·해석·쟁탈한다.**

---

# 1. Executive Summary

STARFALL DYNASTY의 Historical Simulation Engine은 단순한 "로그 시스템"이 아니다.

다음 6개 계층을 가진다.

```text
1. World Simulation
       ↓
2. Domain Event
       ↓
3. Historical Event
       ↓
4. Evidence
       ↓
5. Claim / Interpretation
       ↓
6. Consequence / New Event
```

예:

```text
Player A mines rare mineral
        ↓
MINERAL_DISCOVERED
        ↓
Discovery becomes Historical Event
        ↓
Mining Log / Scanner Data / Witness
        ↓
Player B publishes "A discovered the field"
        ↓
Faction notices strategic value
        ↓
Territory dispute
        ↓
Blockade
        ↓
WAR_DECLARED
```

핵심은 **History Engine이 세계의 사실을 생성하는 시스템이 아니라는 것**이다.

정확한 책임 분리는 다음과 같다.

```text
Simulation
= What happened?

History Engine
= What is recorded about what happened?

Evidence
= What supports that record?

Claim
= What does someone say happened?

Interpretation
= How is the event understood?

Consequence Engine
= What happens because of it?
```

---

# 2. 전문가 비판 리뷰 — 가장 중요한 결론

현재 기획에서 Historical Engine을 잘못 구현할 가능성이 매우 높다.

## 위험한 설계

```text
Player Action
 ↓
LLM
 ↓
Historical Text
 ↓
History
```

이 구조는 사용하면 안 된다.

LLM이 사실을 생성하기 시작하면:

- 사실성 보장 불가능
- 재현성 저하
- 조작 가능성 증가
- 게임 상태와 역사 기록 불일치
- 운영자가 원인을 추적하기 어려움

이 발생한다.

## 권장

```text
Authoritative Simulation
        ↓
Immutable Domain Event
        ↓
Historical Event Projection
        ↓
Evidence
        ↓
Claims
        ↓
Player Interpretation
```

LLM은 가장 바깥쪽에서만 사용한다.

```text
History
 ↓
LLM
 ↓
Summary / News / Search / Dialogue
```

---

# 3. Historical Engine의 진짜 역할

Historical Engine은 다음 질문에 답할 수 있어야 한다.

### Fact

> 실제로 무엇이 일어났는가?

### Evidence

> 그것을 어떻게 알 수 있는가?

### Provenance

> 누가 언제 어떤 방식으로 기록했는가?

### Claim

> 누가 무엇이라고 주장하는가?

### Interpretation

> 서로 다른 기록은 사건을 어떻게 해석하는가?

### Consequence

> 이 사건 때문에 세계가 어떻게 변했는가?

---

# 4. 전체 Architecture

```text
                 ┌─────────────────────┐
                 │    Player / NPC      │
                 └──────────┬──────────┘
                            │ Command
                            ▼
                 ┌─────────────────────┐
                 │ Authoritative Server │
                 └──────────┬──────────┘
                            │
                            ▼
                 ┌─────────────────────┐
                 │   World Simulation   │
                 └──────────┬──────────┘
                            │
                       Domain Event
                            │
                            ▼
                 ┌─────────────────────┐
                 │ Historical Detector  │
                 └──────────┬──────────┘
                            │
                  Historical Event
                            │
              ┌─────────────┼─────────────┐
              ▼             ▼             ▼
          Evidence        Impact       Biography
              │             │
              ▼             ▼
           Claims       Consequences
              │             │
              └──────┬──────┘
                     ▼
              Historical Graph
                     │
          ┌──────────┼──────────┐
          ▼          ▼          ▼
       Chronicle    News      Research
```

---

# 5. 핵심 Data Model

최소 8개의 핵심 Entity를 정의한다.

```text
WorldState
DomainEvent
HistoricalEvent
Evidence
Claim
Interpretation
Consequence
HistoricalEntity
```

---

# 6. WorldState

현재 세계의 authoritative 상태.

예:

```rust
struct WorldState {
    tick: u64,
    current_time: GameTime,

    systems: HashMap<SystemId, StarSystem>,
    factions: HashMap<FactionId, Faction>,
    ships: HashMap<ShipId, Ship>,
    players: HashMap<PlayerId, Player>,
    markets: HashMap<MarketId, Market>,
}
```

중요:

> WorldState는 현재 상태이고 History는 과거의 인과관계다.

둘을 같은 데이터로 취급하지 않는다.

---

# 7. DomainEvent

게임 시스템에서 실제 발생한 원자적 사실.

예:

```text
SHIP_MOVED
MINERAL_MINED
ITEM_SOLD
SHIP_DAMAGED
SHIP_DESTROYED
PLAYER_KILLED
CONTRACT_CREATED
CONTRACT_COMPLETED
FACTION_RELATION_CHANGED
PLANET_CAPTURED
WAR_DECLARED
TREATY_SIGNED
ARTIFACT_DISCOVERED
```

Domain Event는 최대한 기계적으로 생성한다.

---

# 8. DomainEvent Schema

```json
{
  "event_id": "evt_01J...",
  "tick": 18273391,
  "occurred_at": "3827-04-13T18:32:11Z",
  "type": "SHIP_DESTROYED",

  "actor_ids": [
    "player_123",
    "player_456"
  ],

  "object_ids": [
    "ship_777"
  ],

  "location_id": "system_vesta",

  "cause_event_ids": [
    "evt_01J..."
  ],

  "payload": {
    "destroyer_ship_id": "ship_888",
    "weapon_type": "laser",
    "cargo_loss": 182
  }
}
```

---

# 9. Historical Event

모든 Domain Event가 역사적 사건은 아니다.

이 구분이 매우 중요하다.

```text
Domain Event
     │
     ▼
Historical Significance Detector
     │
     ├── insignificant
     │
     └── historically significant
              ↓
        Historical Event
```

예:

```text
MINERAL_MINED
```

10톤 채굴:

```text
일반 이벤트
```

새로운 전략 광물 최초 발견:

```text
Historical Event
```

---

# 10. Historical Significance Engine

역사적 중요도를 Rule Engine으로 결정한다.

예:

```text
IF first_discovery == true
    importance += 100

IF rare_resource == true
    importance += 50

IF territory_changed == true
    importance += 80

IF player_death == true
    importance += 40

IF faction_involved == true
    importance += 30

IF war_triggered == true
    importance += 100
```

최종:

```text
importance_score
```

를 계산한다.

단, 이 score는 플레이어에게 "역사적 가치 87점"처럼 노출할 필요는 없다.

내부 시스템 판단값이다.

---

# 11. 중요도 단계

```text
LEVEL 0
Ephemeral

LEVEL 1
Local Event

LEVEL 2
Regional Event

LEVEL 3
Political Event

LEVEL 4
Galactic Historical Event

LEVEL 5
Epoch-defining Event
```

예:

```text
Mineral mined
→ Level 0

First discovery
→ Level 2

Faction conflict
→ Level 3

War
→ Level 4

Civilization collapse
→ Level 5
```

---

# 12. HistoricalEvent Schema

```json
{
  "historical_event_id": "hist_001",

  "event_type": "MINERAL_DISCOVERED",

  "occurred_at": "3827-04-13T18:32:11Z",

  "location": {
    "system_id": "vesta",
    "planet_id": "vesta_03"
  },

  "participants": [
    {
      "entity_id": "player_123",
      "role": "discoverer"
    }
  ],

  "objects": [
    "mineral_x"
  ],

  "cause_event_ids": [
    "evt_001"
  ],

  "consequence_event_ids": [],

  "evidence_ids": [],

  "importance": 2,

  "visibility": "PUBLIC",

  "status": "CONFIRMED"
}
```

---

# 13. Event Lifecycle

```text
CREATED
   ↓
VALIDATED
   ↓
PUBLISHED
   ↓
INTERPRETED
   ↓
CONSEQUENCES GENERATED
   ↓
ARCHIVED
```

단, 사건 자체의 사실성 상태와 해석 상태는 분리한다.

예:

```text
Event Status
CONFIRMED

Interpretation
DISPUTED
```

---

# 14. Evidence

Evidence는 Historical Engine의 핵심이다.

예:

```text
SHIP_LOG
SCANNER_RECORD
MILITARY_REPORT
GOVERNMENT_ARCHIVE
PLAYER_TESTIMONY
NEWS_ARTICLE
PERSONAL_DIARY
ARTIFACT
ARCHAEOLOGICAL_SITE
VEYER_MEMORY
KHARZ_CONTRACT
```

---

# 15. Evidence는 "진실도"가 아니다

가장 위험한 설계:

```text
reliability = 95
```

만 저장하고 끝내는 것이다.

신뢰성은 상황에 따라 달라질 수 있다.

예:

```text
Military Report
```

전투 위치:

높은 신뢰

정치적 책임:

낮은 신뢰 가능

따라서 Evidence에는 provenance와 scope가 필요하다.

---

# 16. Evidence Schema

```json
{
  "evidence_id": "ev_001",

  "type": "SHIP_LOG",

  "source_entity_id": "ship_777",

  "created_at": "3827-04-13T18:40:00Z",

  "related_event_ids": [
    "hist_001"
  ],

  "provenance": {
    "origin": "ship_system",
    "creator": "ship_777",
    "creation_method": "automatic"
  },

  "integrity": {
    "tampered": false,
    "authenticity": "VERIFIED"
  },

  "visibility": "PUBLIC"
}
```

---

# 17. Evidence Provenance

모든 Evidence는 출처 계보를 가진다.

```text
Original Event
 ↓
Original Record
 ↓
Copied Document
 ↓
Published Article
 ↓
Historical Summary
```

이것을 추적할 수 있어야 한다.

```text
derived_from_evidence_ids[]
```

를 사용한다.

---

# 18. Evidence Mutation

흥미로운 게임 시스템으로 만들 수 있다.

예:

```text
Original Military Report
        ↓
Classified
        ↓
Stolen
        ↓
Edited
        ↓
Published
```

단,

> 원본 Evidence는 변경하지 않는다.

새로운 Evidence가 생성된다.

```text
Evidence A
original

Evidence B
edited copy
```

---

# 19. Claim

Claim은 사실 자체가 아니다.

```text
Evidence
   ↓
Player A:
"Faction X started the war."
```

이것이 Claim이다.

Schema:

```json
{
  "claim_id": "claim_001",

  "author_id": "player_123",

  "statement": {
    "subject": "faction_x",
    "predicate": "started_war",
    "object": "war_001"
  },

  "evidence_ids": [
    "ev_001",
    "ev_003"
  ],

  "counter_evidence_ids": [
    "ev_008"
  ],

  "created_at": "3828-02-11T10:00:00Z"
}
```

---

# 20. Claim은 구조화해야 한다

자연어만 저장하면 검색/판정이 어렵다.

가능하면:

```text
subject
predicate
object
qualifiers
```

구조를 사용한다.

예:

```text
subject = Faction A
predicate = DECLARED_WAR
object = Faction B
date = 3828
location = Vesta
```

자연어 표현은 별도 필드로 저장한다.

---

# 21. Interpretation

여러 Claim을 조합한 역사적 해석.

예:

```text
Interpretation A

"Vesta War was caused by resource scarcity."

Evidence:
E1
E4
E9

Claims:
C1
C7
C8
```

다른 플레이어:

```text
Interpretation B

"Vesta War was primarily a political expansion."
```

둘 다 존재할 수 있다.

---

# 22. 역사적 Truth와 Interpretation 분리

이 게임에서 중요한 원칙:

```text
FACT
≠
CLAIM
≠
INTERPRETATION
```

예:

```text
FACT:
Planet Vesta was captured on Day 1882.

CLAIM:
Faction A intentionally attacked civilians.

INTERPRETATION:
The war was fundamentally economic.
```

이 세 가지를 DB에서 분리한다.

---

# 23. Historical Graph

History는 그래프 형태로 모델링한다.

```text
Event A
   │
   ├── caused_by → Event B
   │
   ├── produced → Evidence C
   │
   ├── disputed_by → Claim D
   │
   └── caused → Event E
```

그래프 관계:

```text
CAUSED_BY
CAUSED
SUPPORTED_BY
CONTRADICTED_BY
DERIVED_FROM
PARTICIPATED_IN
OCCURRED_AT
OWNED_BY
DISCOVERED_BY
FOLLOWED_BY
```

---

# 24. DB 구현 전략

처음부터 Graph DB를 도입하지 않는다.

PostgreSQL에서:

```text
historical_events
evidence
claims
interpretations
event_relations
```

로 시작한다.

예:

```sql
CREATE TABLE event_relations (
    from_event_id UUID NOT NULL,
    relation_type TEXT NOT NULL,
    to_event_id UUID NOT NULL,
    PRIMARY KEY (
        from_event_id,
        relation_type,
        to_event_id
    )
);
```

History 규모가 커졌을 때 그래프 전용 저장소를 검토한다.

---

# 25. Causal Chain

역사의 핵심은 단순 timeline이 아니라 인과관계다.

예:

```text
Mineral Discovery
        ↓
Mineral Price Crash
        ↓
Mining Company Bankruptcy
        ↓
Unemployment
        ↓
Political Protest
        ↓
Faction Election
        ↓
Policy Change
        ↓
Trade Restriction
        ↓
Smuggling
        ↓
Border Conflict
        ↓
War
```

이 chain을 시스템이 추적해야 한다.

---

# 26. Consequence Engine

Historical Event 발생 후 후속 효과를 계산한다.

```text
Historical Event
        ↓
Rule Evaluation
        ↓
World State Mutation
        ↓
New Domain Events
```

예:

```text
WAR_DECLARED
 ↓
Trade embargo
 ↓
PRICE_CHANGED
 ↓
SHORTAGE_DETECTED
 ↓
MIGRATION_STARTED
```

---

# 27. Consequence는 직접 생성하지 말고 Event로 생성

잘못된 방법:

```text
WarEvent
 → directly modify 50 tables
```

권장:

```text
WAR_DECLARED
 ↓
Embargo System
 ↓
TRADE_RESTRICTION_CREATED
 ↓
Market Simulation
 ↓
PRICE_CHANGED
```

각 시스템이 자신의 책임으로 후속 이벤트를 만든다.

---

# 28. Event Bus

초기에는:

```text
PostgreSQL Outbox
```

를 사용한다.

규모가 커지면:

```text
NATS JetStream
```

으로 확장한다.

```text
World
 ↓
Outbox
 ↓
NATS
 ├── History
 ├── News
 ├── Economy
 ├── Analytics
 └── Notification
```

---

# 29. Idempotency

Historical Engine은 동일 이벤트를 두 번 받아도 결과가 중복되지 않아야 한다.

예:

```text
event_id = evt_123
```

처리 완료 기록:

```text
processed_events
```

또는 각 projection에 idempotency key를 둔다.

```text
UNIQUE(event_id)
```

이것은 필수다.

---

# 30. Ordering

분산 시스템에서는 이벤트가 항상 순서대로 도착한다고 가정하면 안 된다.

예:

```text
EVENT A
EVENT B
EVENT C
```

실제 도착:

```text
B
A
C
```

따라서:

```text
tick
occurred_at
sequence
cause_event_ids
```

를 사용한다.

---

# 31. Historical Event Versioning

게임은 수년간 운영될 수 있다.

Event schema가 바뀐다.

따라서:

```json
{
  "schema_version": 2
}
```

를 저장한다.

기존 이벤트를 함부로 수정하지 않는다.

Migration 또는 upcasting 전략을 사용한다.

---

# 32. Determinism

Historical Engine이 동일한 입력으로 다른 결과를 만들면 안 된다.

```text
World State
+
Event
+
Rule Version
=
Same Result
```

가능하면:

```text
random_seed
rule_version
simulation_tick
```

도 기록한다.

---

# 33. Rule Engine

초기에는 복잡한 범용 Rule Engine을 만들지 않는다.

Rust 코드 기반 명시적 Rule로 시작한다.

예:

```rust
fn evaluate_war_significance(event: &DomainEvent) -> Significance {
    // explicit rules
}
```

게임 규칙이 많아진 후 DSL 또는 데이터 기반 Rule로 이동한다.

---

# 34. Rule Versioning

역사 시스템에서 매우 중요하다.

예:

```text
Rule v1
War threshold = 100

Rule v2
War threshold = 80
```

과거 이벤트를 현재 규칙으로 재계산하면 역사 왜곡이 발생할 수 있다.

따라서 이벤트 생성 당시:

```text
rule_version
```

을 저장한다.

---

# 35. Game Time

실제 시간과 게임 시간을 분리한다.

```text
Real Time
2026-09-17

Game Time
3827-04-13
```

모든 역사적 사건은 Game Time을 가진다.

필요하면:

```text
real_timestamp
game_timestamp
```

둘 다 저장한다.

---

# 36. Calendar

게임이 장기 운영되면 달력도 중요해진다.

예:

```text
Year
Season
Month
Day
Tick
```

History UI에서는:

```text
Year 3827
Fourth Month
Day 13
```

같은 표현이 가능하다.

---

# 37. Historical Visibility

모든 사건이 모든 플레이어에게 공개되어서는 안 된다.

```text
PUBLIC
FACTION_ONLY
PARTICIPANTS_ONLY
CLASSIFIED
SECRET
DISCOVERABLE
```

중요:

> Visibility는 Evidence의 존재와 별개다.

비밀 사건도 Evidence가 존재할 수 있다.

---

# 38. Fog of History

이 시스템이 게임적으로 매우 중요하다.

예:

```text
Event 발생
 ↓
Only participants know
 ↓
Rumour appears
 ↓
Evidence discovered
 ↓
Public confirmation
```

즉:

> 세계의 실제 상태와 플레이어가 알고 있는 역사는 다를 수 있다.

---

# 39. Information Discovery

플레이어가 역사를 발견하는 방법:

```text
Journalist
→ interview

Historian
→ archive

Archaeologist
→ excavation

Spy
→ classified document

Explorer
→ ancient record

Trader
→ transaction record
```

History는 콘텐츠가 된다.

---

# 40. Knowledge State

플레이어마다 Knowledge State가 필요할 수 있다.

```text
PlayerKnowledge
 ├── known_events
 ├── known_evidence
 ├── known_claims
 └── discovered_locations
```

단, 모든 것을 DB에 개별 row로 저장하면 규모가 폭발할 수 있다.

초기에는 중요 발견만 persistence하고 나머지는 derived/cache로 관리한다.

---

# 41. Rumor System

"사실이 아닌 정보"도 게임 콘텐츠가 될 수 있다.

하지만 시스템적으로 구분한다.

```text
FACT
RUMOR
CLAIM
CONFIRMED
DISPUTED
FALSE
```

Rumor가 자동으로 Fact가 되면 안 된다.

---

# 42. Player-Generated History

플레이어는 다음을 만들 수 있다.

```text
Diary
Report
News
Memoir
Contract
Research Paper
Museum Description
Faction Archive
```

이것들은 모두 Evidence 또는 Claim의 형태로 저장될 수 있다.

---

# 43. Player Biography

Biography는 History Projection이다.

원본 데이터가 아니다.

```text
Historical Events
       ↓
Biography Projection
       ↓
Player Timeline
```

예:

```text
3827
Discovered Vesta-9

3828
Founded Vesta Mining Guild

3831
Participated in Vesta War

3834
Declared Missing
```

---

# 44. Ship Biography

우주선도 역사적 객체다.

```text
Ship
 ↓
Ownership History
 ↓
Battle History
 ↓
Cargo History
 ↓
Repair History
 ↓
Captains
 ↓
Final Fate
```

예:

```text
S-001 "Aurora"

Launched: 3827
First Captain: Arin Val
First Discovery: Vesta-9
Battle Count: 17
Destroyed: 3836
```

---

# 45. Institution History

Guild / Faction / Corporation도 동일하다.

```text
Founded
 ↓
Members
 ↓
Treaties
 ↓
Wars
 ↓
Economic Events
 ↓
Leadership Changes
 ↓
Dissolution
```

이것이 장기 MMO의 역사적 깊이를 만든다.

---

# 46. Historical Projection

History Engine은 여러 View를 생성한다.

```text
Historical Event
        │
        ├── Galactic Chronicle
        ├── Player Biography
        ├── Ship Biography
        ├── Faction History
        ├── News
        ├── Museum Record
        └── Research Timeline
```

원본 Event를 직접 UI에서 가공하지 말고 projection 계층을 둔다.

---

# 47. CQRS 적용

이 프로젝트에는 부분적인 CQRS가 적합하다.

```text
Command
 ↓
Simulation
 ↓
Write Model
 ↓
Event
 ↓
Read Models
```

Read Model:

```text
Chronicle
Biography
News
Research
Search
```

초기부터 복잡한 CQRS framework를 도입할 필요는 없다.

---

# 48. History Query API

예:

```http
GET /history/events/{id}

GET /history/events?system=vesta

GET /history/events?actor=player_123

GET /history/ships/{ship_id}

GET /history/factions/{faction_id}

GET /history/claims/{event_id}

GET /history/evidence/{event_id}

GET /history/timeline?from=3827&to=3830
```

---

# 49. Research API

Historian gameplay를 지원한다.

```http
POST /research/cases

POST /research/claims

POST /research/evidence

POST /research/interpretations

GET /research/cases/{id}
```

---

# 50. Research Case

Historian은 사건 하나를 조사할 수 있다.

```text
Research Case
 ├── Question
 ├── Events
 ├── Evidence
 ├── Claims
 ├── Counter Claims
 └── Conclusion
```

예:

> "Who actually started the Vesta War?"

게임플레이가 된다.

---

# 51. Historical Conflict

역사적 해석이 PvP 외의 경쟁이 될 수 있다.

```text
Historian A
vs
Historian B
```

경쟁 요소:

- Evidence discovery
- Archive access
- Research funding
- Reputation
- Publication
- Academic influence

단, 시스템은 어느 해석을 "정답"이라고 자동 판정하지 않는다.

---

# 52. Reputation

Historian Reputation도 별도 시스템이다.

하지만:

```text
Reputation = Truth
```

로 취급하면 안 된다.

예:

```text
Famous Historian
≠
Always Correct
```

게임적으로 매우 중요하다.

---

# 53. Historical Manipulation

플레이어는 역사를 조작하려고 할 수 있다.

예:

```text
destroy evidence
forge document
spread claim
hide archive
```

이 역시 게임 콘텐츠가 된다.

그러나 시스템 내부적으로:

```text
Original Evidence
Forged Evidence
```

를 명확히 구분해야 한다.

---

# 54. Forgery Model

```text
Evidence
 ├── original_evidence_id
 ├── creator
 ├── creation_method
 ├── authenticity_status
 └── provenance
```

예:

```text
Original Military Record
       ↓
Forged Copy
       ↓
Published Newspaper
```

후대 플레이어가 진위를 조사할 수 있다.

---

# 55. Historical Revision

새 Evidence가 발견되면 기존 Interpretation의 상태가 바뀔 수 있다.

```text
Interpretation A
"War was caused by resource shortage."

New Evidence
 ↓
Contradiction detected
 ↓
Interpretation becomes disputed
```

과거 기록을 삭제하지 않는다.

변화 자체가 역사다.

---

# 56. History Graph Example

```text
[MINERAL DISCOVERY]
        │
        ▼
[PRICE COLLAPSE]
        │
        ▼
[CORPORATE BANKRUPTCY]
        │
        ├───────────────┐
        ▼               ▼
[UNEMPLOYMENT]      [MIGRATION]
        │               │
        └──────┬────────┘
               ▼
       [POLITICAL RIOT]
               │
               ▼
         [COUP ATTEMPT]
               │
               ▼
          [CIVIL WAR]
               │
               ▼
        [REFUGEE CRISIS]
               │
               ▼
       [INTERSTELLAR WAR]
```

이 그래프가 게임 월드의 장기 콘텐츠가 된다.

---

# 57. Simulation Tick Architecture

추천:

```text
Input Phase
 ↓
Validation
 ↓
State Transition
 ↓
Domain Events
 ↓
Historical Detection
 ↓
Consequences
 ↓
Persistence
 ↓
Projection
```

한 tick에서 발생한 이벤트는 deterministic ordering을 가진다.

---

# 58. Event Processing Pipeline

Rust pseudo-code:

```rust
fn process_tick(
    state: &mut WorldState,
    commands: Vec<Command>,
) -> Vec<DomainEvent> {

    let mut events = Vec::new();

    for command in commands {
        let domain_events =
            execute_command(state, command);

        events.extend(domain_events);
    }

    events
}
```

그 다음:

```rust
for event in events {
    history_engine.process(event);
}
```

---

# 59. Historical Detector

```rust
struct HistoricalDetector;

impl HistoricalDetector {
    fn evaluate(
        event: &DomainEvent,
        state: &WorldState,
    ) -> Option<HistoricalEvent> {

        if is_first_discovery(event, state) {
            return Some(create_discovery_event(event));
        }

        if caused_war(event, state) {
            return Some(create_war_event(event));
        }

        None
    }
}
```

초기에는 이 정도의 명시적 코드가 오히려 좋다.

---

# 60. Event Correlation

하나의 Historical Event가 여러 Domain Event를 묶을 수 있다.

예:

```text
10 ships destroyed
3 faction members killed
cargo stolen
trade route disrupted
```

이 여러 이벤트가:

```text
BATTLE_OF_VESTA
```

라는 하나의 Historical Event가 될 수 있다.

---

# 61. Correlation Window

예:

```text
within 60 seconds
within 100km
same combat instance
same faction conflict
```

같은 이벤트를 묶는 기준이 필요하다.

```text
Battle Correlation Rule
```

을 둔다.

---

# 62. Aggregate Event

```text
Domain Events
 ├── SHIP_DESTROYED
 ├── SHIP_DESTROYED
 ├── PLAYER_KILLED
 ├── CARGO_CAPTURED
 └── FLEET_RETREATED
          ↓
      BATTLE_OCCURRED
```

Aggregate Event는 플레이어에게 보여주는 역사적 단위가 된다.

---

# 63. Historical Event Identity

Event ID와 Historical Event ID를 분리한다.

```text
evt_001
evt_002
evt_003
      ↓
hist_battle_001
```

이것이 나중에:

- Replay
- Audit
- Debug
- Investigation

에 매우 중요하다.

---

# 64. Replay

개발자에게 매우 중요한 기능.

```text
Historical Event
 ↓
Cause Events
 ↓
Simulation Snapshot
 ↓
Replay
```

버그가 발생하면:

> "왜 Vesta War가 발생했는가?"

를 재현할 수 있어야 한다.

---

# 65. Snapshot

Event만 무한히 replay하면 비용이 증가한다.

따라서:

```text
Snapshot 100000
Events
100001
100002
...
100500
```

형태로 저장한다.

History가 아니라 World Simulation snapshot에 적용한다.

---

# 66. Audit Trail

중요한 경제/정치 이벤트는 audit 가능해야 한다.

예:

```text
WHO
WHAT
WHEN
WHERE
WHY
CAUSED_BY
RESULT
```

운영자 도구에서 조회할 수 있어야 한다.

---

# 67. GM / Admin Tool

실제 운영에는 반드시 필요하다.

최소 기능:

```text
Search Event
Inspect Event
Inspect Cause
Inspect Evidence
Inspect Consequence
Inspect Player
Inspect Ship
Inspect Faction
Replay Event Chain
```

이것이 없으면 live service 운영이 매우 어려워진다.

---

# 68. Admin Mutation 제한

운영자가 역사 데이터를 직접 수정하는 기능은 극도로 제한한다.

권장:

```text
Admin Action
 ↓
Admin Domain Event
 ↓
Audit Log
```

예:

```text
ADMIN_EVENT_CORRECTION
```

으로 수정 이유를 남긴다.

원본 기록을 지우지 않는다.

---

# 69. Persistence Strategy

### PostgreSQL

Canonical:

```text
World entities
Historical Events
Evidence metadata
Claims
Relations
```

### Redis

```text
Hot cache
Session
Temporary knowledge
```

### NATS

```text
Event transport
```

### OpenSearch

```text
Search projection
```

### ClickHouse

```text
Analytics
```

### S3

```text
Documents
Raw logs
Archive
```

---

# 70. 최소 PostgreSQL Schema

```text
players
ships
factions
systems

domain_events

historical_events
historical_event_participants
historical_event_objects
historical_event_relations

evidence
evidence_relations

claims
claim_evidence

interpretations
interpretation_claims

research_cases

news_articles

outbox_events
processed_events
```

---

# 71. Index 전략

핵심:

```text
historical_events(event_type)
historical_events(occurred_at)
historical_events(location_id)
historical_event_participants(entity_id)
historical_event_objects(object_id)
event_relations(from_event_id)
event_relations(to_event_id)
claims(event_id)
evidence(event_id)
```

시간 기반 조회가 많기 때문에 `occurred_at` 인덱스가 중요하다.

---

# 72. Partitioning

History가 매우 커지면:

```text
historical_events_3827
historical_events_3828
historical_events_3829
```

같은 시간 기반 partition을 검토한다.

그러나 MVP부터 partitioning하지 않는다.

실제 데이터 규모를 측정한 후 결정한다.

---

# 73. Search Architecture

초기:

```text
PostgreSQL Full Text Search
```

중기:

```text
OpenSearch
```

검색 예:

> "Vesta War에서 Arin Val이 어떤 역할을 했는가?"

검색 결과:

```text
Events
Evidence
Claims
Ships
Factions
News
```

---

# 74. Natural Language Historical Search

LLM은 여기서 유용하다.

```text
User:
"Vesta 전쟁이 왜 시작됐어?"

        ↓

LLM Query Planner
        ↓
History Search
        ↓
Events + Evidence + Claims
        ↓
LLM Summary
```

LLM이 DB에 없는 사실을 만들어내면 안 된다.

---

# 75. RAG Grounding

답변에는 반드시 source reference를 포함한다.

예:

```text
According to:

Event #123
Evidence #888
Claim #991
```

게임 UI에서는:

> 근거 보기

버튼으로 연결한다.

---

# 76. News Generation

잘못된 구조:

```text
LLM → decides what happened
```

권장:

```text
Historical Event
 ↓
News Template / Structured Prompt
 ↓
LLM
 ↓
Article
```

Article은 원본 사건의 대체물이 아니다.

---

# 77. News Bias

Journalist gameplay를 위해 기사에 관점을 부여할 수 있다.

예:

```text
Faction A Newspaper
Faction B Newspaper
Independent Reporter
```

그러나 원본 Event와 기사 내용을 분리한다.

```text
Event = Fact Layer

Article = Interpretation / Presentation Layer
```

---

# 78. Historical API와 권한

예:

```text
GET /history/events/123
```

응답은 플레이어의 knowledge/visibility에 따라 달라질 수 있다.

```text
Public Event
→ public fields

Secret Event
→ hidden

Discovered Evidence
→ reveal additional fields
```

즉:

> History Query 자체도 게임 시스템이다.

---

# 79. Anti-Abuse

History 시스템은 악용 가능성이 높다.

예:

- 이벤트 spam
- 가짜 연구
- 가짜 뉴스
- 증거 무한 생성
- reputation farming
- 부계정 조작
- 경제적 사건 유도

필요:

```text
rate limit
cooldown
resource cost
reputation threshold
identity/account constraints
event significance threshold
```

---

# 80. 특히 주의할 문제: Historical Spam

모든 플레이어 행동을 역사로 기록하면:

```text
History = Noise
```

가 된다.

따라서:

```text
10,000,000 actions
       ↓
10,000 historical events
```

정도의 압축이 필요하다.

---

# 81. Historical Event Compression

예:

```text
Mineral mined 1
Mineral mined 2
Mineral mined 3
...
Mineral mined 10,000
```

을 그대로 보여주지 않는다.

대신:

```text
Vesta Mining Campaign
```

으로 aggregate할 수 있다.

단, 원자 이벤트는 audit/replay 목적에 남길 수 있다.

---

# 82. Event Importance는 동적일 수 있다

처음에는:

```text
Mineral Discovery
importance = 20
```

였는데 나중에:

```text
Discovery caused war
```

가 되면:

```text
importance = 100
```

으로 historical significance가 상승할 수 있다.

단,

> 원래 사건 자체를 수정하지 말고 현재 significance projection을 갱신한다.

---

# 83. Retcon 금지

운영자가 "스토리 때문에" 과거 사건을 수정하는 구조는 피한다.

원칙:

```text
Past Event
= immutable

New Evidence
= new event

New Interpretation
= new record
```

이것이 이 게임의 신뢰성을 만든다.

---

# 84. Death & Legacy

PLAYER_DIED는 특별 취급한다.

```text
PLAYER_DIED
 ↓
Historical Event
 ↓
Biography
 ↓
Ship History
 ↓
Faction Reaction
 ↓
Potential Revenge
```

그러나 Revenge는 강제하지 않는다.

플레이어 선택:

```text
Revenge
Forgiveness
Ignore
Negotiate
Memorialize
```

---

# 85. Historical Object Graph

캐릭터만 역사적 객체가 아니다.

```text
Player
Ship
Weapon
Artifact
Faction
Corporation
Planet
Star System
Institution
Treaty
War
Trade Route
Museum
```

모든 객체가 history를 가질 수 있다.

---

# 86. History as Content Generator

History Engine의 최종 목적은 데이터를 쌓는 것이 아니다.

```text
Past Event
 ↓
Discovery
 ↓
Quest
 ↓
Conflict
 ↓
Economic Opportunity
 ↓
Political Opportunity
 ↓
New Event
```

즉:

> **History → Content**

가 되어야 한다.

---

# 87. 예시: 1개의 역사 사건이 만드는 콘텐츠

```text
Artifact discovered
        ↓
Museum wants it
        ↓
Faction wants it
        ↓
Collector offers money
        ↓
Smuggler attempts theft
        ↓
Bounty issued
        ↓
Bounty Hunter tracks thief
        ↓
PvP
        ↓
Ship destroyed
        ↓
New Historical Event
```

하나의 사건이 여러 gameplay loop를 생성한다.

---

# 88. MVP Scope

처음부터 전체 Historical Engine을 구현하지 않는다.

### MVP

```text
Domain Event
Historical Event
Evidence
Claim
Biography
Chronicle
Basic Consequence
```

### Later

```text
Interpretation
Forgery
Rumor
Research Cases
Museums
Academic Competition
Advanced Causality
Historical Revision
```

---

# 89. MVP Historical Events

처음에는 10개 정도로 제한한다.

```text
MINERAL_DISCOVERED
ARTIFACT_DISCOVERED
SHIP_DESTROYED
PLAYER_DIED
PLAYER_CREATED_FACTION
TERRITORY_CAPTURED
CONTRACT_SIGNED
CONTRACT_BROKEN
WAR_DECLARED
TREATY_SIGNED
```

---

# 90. MVP Vertical Slice

반드시 다음을 구현한다.

```text
Player A
 ↓
Discover Mineral
 ↓
Server validates
 ↓
MINERAL_DISCOVERED
 ↓
Historical Event
 ↓
Evidence generated
 ↓
Chronicle updated
 ↓
Player B discovers information
 ↓
Player B creates Claim
 ↓
Faction reacts
 ↓
New event
```

이것이 성공하면 Historical Engine의 핵심 가설이 검증된다.

---

# 91. 테스트 전략

### Unit Test

Rule Engine

### Integration Test

Event → History

### Property Test

Idempotency

### Deterministic Test

Same input → same result

### Replay Test

Event chain → same world state

### Load Test

100 / 1,000 / 10,000 simulated actors

---

# 92. 중요한 Property Tests

예:

```text
Processing same event twice
must not create duplicate Historical Event.
```

또한:

```text
Event ordering variation within allowed constraints
must not produce invalid world state.
```

---

# 93. Failure Recovery

Historical Engine worker가 죽어도:

```text
Event
 ↓
Outbox
 ↓
Retry
```

가능해야 한다.

처리 성공 후:

```text
processed_events
```

를 기록한다.

---

# 94. Exactly-once에 대한 비판

분산 시스템에서 완벽한 exactly-once를 목표로 복잡도를 높이지 않는다.

실무적으로:

```text
At-least-once delivery
+
Idempotent processing
```

을 사용한다.

이 프로젝트에는 훨씬 현실적인 선택이다.

---

# 95. Consistency Model

### World State

Strong consistency가 필요한 부분:

- Money
- Inventory
- Ownership
- Contract settlement

### History

Eventual consistency 허용:

- Chronicle
- News
- Search
- Analytics

이 구분이 매우 중요하다.

---

# 96. Historical Engine 서비스 경계

MVP에서는:

```text
Rust Modular Monolith
```

내부 module:

```text
simulation/
history/
evidence/
research/
consequence/
projection/
```

으로 구현한다.

---

# 97. 미래의 분리

규모가 커지면:

```text
World Simulation
History Service
Research Service
Search Service
News Service
```

로 분리할 수 있다.

그러나 처음부터 분리하지 않는다.

---

# 98. 추천 Rust Module Structure

```text
server/
├── src/
│   ├── main.rs
│   │
│   ├── simulation/
│   │   ├── world.rs
│   │   ├── tick.rs
│   │   ├── command.rs
│   │   └── systems/
│   │
│   ├── domain/
│   │   ├── event.rs
│   │   ├── player.rs
│   │   ├── ship.rs
│   │   └── faction.rs
│   │
│   ├── history/
│   │   ├── detector.rs
│   │   ├── event.rs
│   │   ├── evidence.rs
│   │   ├── claim.rs
│   │   ├── interpretation.rs
│   │   ├── consequence.rs
│   │   ├── projection.rs
│   │   └── graph.rs
│   │
│   ├── economy/
│   ├── combat/
│   ├── diplomacy/
│   ├── research/
│   ├── news/
│   └── infrastructure/
│
└── migrations/
```

---

# 99. API / DB / Domain 분리

권장:

```text
HTTP DTO
   ↓
Application Command
   ↓
Domain
   ↓
Domain Event
   ↓
Persistence
```

DB model을 그대로 API response로 사용하지 않는다.

---

# 100. Event Contract

Unity와 Rust 사이에서 공유하는 데이터 계약이 필요하다.

추천:

```text
Protobuf
```

또는 초기에는:

```text
JSON Schema
```

이후 binary protocol로 변경한다.

---

# 101. Event Envelope

모든 이벤트에 공통 envelope을 둔다.

```json
{
  "event_id": "...",
  "event_type": "...",
  "schema_version": 1,
  "world_id": "...",
  "tick": 123,
  "occurred_at": "...",
  "correlation_id": "...",
  "causation_id": "...",
  "actor_id": "...",
  "payload": {}
}
```

특히:

```text
correlation_id
causation_id
```

가 중요하다.

---

# 102. Correlation vs Causation

### correlation_id

하나의 gameplay transaction을 묶는다.

```text
Trade Request
 ├── item removed
 ├── money transferred
 └── contract updated
```

### causation_id

현재 이벤트를 직접 유발한 이벤트.

```text
WAR_DECLARED
caused
TRADE_EMBARGO
```

둘을 혼동하면 안 된다.

---

# 103. Historical Event Naming

이름은 과장된 서사보다 기계적으로 유지한다.

좋음:

```text
BATTLE_OCCURRED
WAR_DECLARED
ARTIFACT_DISCOVERED
PLAYER_DIED
```

나쁨:

```text
THE_GREAT_TRAGEDY_OF_VESTA
```

후자는 Narrative Layer에서 생성한다.

---

# 104. Narrative Layer

역사 데이터와 서사 표현을 분리한다.

```text
Canonical Event
      ↓
Narrative Template
      ↓
LLM / Localization
      ↓
Human-readable History
```

예:

Canonical:

```text
WAR_DECLARED
Faction A
Faction B
3828-04-13
```

표현:

> "Vesta War officially began in 3828."

---

# 105. Localization

역사 데이터 자체는 언어 중립적으로 저장한다.

```text
event_type
entity_id
timestamp
```

문장은 별도 생성한다.

이렇게 하면:

```text
Korean
English
Japanese
French
```

지원이 쉬워진다.

---

# 106. Metrics

Historical Engine의 핵심 운영 지표:

```text
Historical Event Creation Rate
Evidence Discovery Rate
Claim Creation Rate
Research Completion Rate
Historical Search Rate
History → Gameplay Conversion
Historical Event → Consequence Conversion
```

특히:

## Historical Interaction Rate

```text
players who interact with history
/
active players
```

를 North Star Metric 후보로 둔다.

---

# 107. 가장 중요한 Product Metric

단순히:

```text
events_created = 10,000,000
```

은 의미가 없다.

좋은 지표:

```text
Event
 ↓
Discovered
 ↓
Investigated
 ↓
Discussed
 ↓
Used in gameplay
 ↓
Creates new event
```

즉:

> **History Loop Completion Rate**

를 측정한다.

---

# 108. 전문가가 가장 강하게 경고하는 부분

## 1. History Database가 되지 말 것

플레이어가 역사 UI를 한 번 보고 끝나면 실패다.

History는:

```text
Quest
Trade
PvP
Politics
Exploration
Research
```

를 유발해야 한다.

---

## 2. Causality를 과도하게 자동화하지 말 것

세계의 모든 사건을 AI가 인과관계로 연결하면:

```text
unexpected chain explosion
```

이 발생한다.

초기에는 명시적 causal rules만 사용한다.

---

## 3. 모든 사건을 영구 보존하지 말 것

Storage보다 중요한 것은:

```text
Signal-to-noise ratio
```

이다.

---

## 4. Player-generated content를 사실로 승격시키지 말 것

```text
Player Claim
≠
World Fact
```

---

## 5. 역사적 불확실성을 게임적으로 설계할 것

모든 것을 모호하게 만들면 재미없다.

권장:

```text
Known Facts
+
Unknowns
+
Disputes
+
Discoverable Evidence
```

의 균형이다.

---

# 109. 최종 권장 Architecture

```text
                 ┌───────────────┐
                 │ Unity Client  │
                 └───────┬───────┘
                         │
                       Command
                         │
                         ▼
                ┌──────────────────┐
                │ Rust Game Server │
                └────────┬─────────┘
                         │
                  World Simulation
                         │
                         ▼
                   Domain Events
                         │
              ┌──────────┴──────────┐
              │                     │
              ▼                     ▼
      Historical Detector      Other Systems
              │
              ▼
       Historical Events
              │
       ┌──────┼───────┐
       ▼      ▼       ▼
   Evidence Claims Consequences
       │      │       │
       └──────┼───────┘
              ▼
       Historical Graph
              │
       ┌──────┼────────┐
       ▼      ▼        ▼
   Chronicle Biography Research
       │                 │
       ▼                 ▼
      News          Player Gameplay
```

---

# 110. 개발 순서

## Sprint 1

```text
Domain Event
Event Envelope
PostgreSQL
Outbox
```

## Sprint 2

```text
Historical Event
Significance Detector
Chronicle
```

## Sprint 3

```text
Evidence
Provenance
Visibility
```

## Sprint 4

```text
Claim
Evidence Linking
Contradiction
```

## Sprint 5

```text
Consequence Engine
Causal Chain
```

## Sprint 6

```text
Biography
Ship History
Faction History
```

## Sprint 7

```text
Research Case
Historical Search
```

## Sprint 8

```text
News
LLM Summary
RAG
```

---

# 111. 구현 우선순위

### P0

```text
Domain Events
Historical Events
Persistence
Idempotency
Chronicle
```

### P1

```text
Evidence
Provenance
Claims
Consequences
Biography
```

### P2

```text
Research
Rumors
Forgery
Faction Archives
Museums
```

### P3

```text
Advanced AI
Historical Simulation
Complex Interpretations
Graph Analytics
```

---

# 112. 최종 기술 판단

## 반드시 유지

```text
Rust
PostgreSQL
Authoritative Simulation
Event-driven Architecture
Historical Event Model
Evidence / Claim Separation
```

## 단계적으로 도입

```text
NATS
OpenSearch
ClickHouse
S3 Archive
LLM/RAG
Advanced Causality
```

## 초기에는 피할 것

```text
Graph DB
Full Microservices
Full Event Sourcing
Kubernetes-first
LLM-driven simulation
Universal Rule Engine
```

---

# 113. 최종 전문가 평가

Historical Simulation Engine의 핵심 기술 난이도는 "이벤트를 저장하는 것"이 아니다.

진짜 난이도는:

```text
Event
 ↓
Significance
 ↓
Evidence
 ↓
Knowledge
 ↓
Interpretation
 ↓
Consequence
 ↓
New Event
```

의 **폐쇄 루프(closed loop)**를 안정적으로 만드는 것이다.

이 구조가 성공하면 STARFALL DYNASTY는 단순한 우주 MMO가 아니라:

> **플레이어 행동이 장기적으로 축적되고, 후대 플레이어가 그것을 발견하고 해석하며, 그 해석이 다시 새로운 플레이를 만드는 역사형 MMO**

라는 독특한 게임 구조를 가질 수 있다.

---

# 114. 최종 North Star

```text
PLAYER ACTION
      ↓
WORLD CHANGE
      ↓
DOMAIN EVENT
      ↓
HISTORICAL EVENT
      ↓
EVIDENCE
      ↓
KNOWLEDGE
      ↓
INTERPRETATION
      ↓
CONSEQUENCE
      ↓
NEW PLAYER ACTION
      ↓
NEW HISTORY
```

**이 루프를 구현하는 것이 Historical Simulation Engine의 최우선 목표다.**

그리고 가장 중요한 설계 원칙은 한 문장으로 정리된다.

> **"The simulation creates facts. Players create history."**

# STARFALL DYNASTY — Implementation Tech Stack
## 전문가 비판적 리뷰 및 권장 기술 아키텍처

> 목적: **역사 기반 Persistent Space MMO**를 실제 구현할 수 있는 기술 스택을 선정한다.
>
> 핵심 요구사항:
> - Unity 기반 3D 클라이언트
> - 1인 우주선 → 개인 함대 성장
> - 실시간 PvP / 함대전
> - 경제 / 무역 / 자원
> - 3종족 / 다수 직업
> - 플레이어 행동의 장기적 역사 기록
> - Historical Event / Evidence / Claim / Interpretation
> - 수천~수만 성계로 확장 가능한 구조
> - WebGL/WebAssembly 가능성
> - 서버 authoritative architecture

---

# 1. Executive Decision

## 권장 조합

| 영역 | 최종 권장 | 판단 |
|---|---|---|
| Client | **Unity 6 + C#** | 유지 |
| Rendering | **URP** | 초기 권장 |
| Massive Objects | **ECS / Jobs / Burst 선택적 적용** | 유지하되 과사용 금지 |
| Backend | **Rust** | 유지 |
| Web/API | **Rust + Axum** | 권장 |
| Realtime | **UDP/QUIC 계열 또는 WebSocket 혼합** | 상황별 적용 |
| Browser | **WebAssembly/WebGL** | 별도 최적화 타깃 |
| Database | **PostgreSQL** | 핵심 영속 DB |
| Cache | **Redis** | 보조 |
| Event Streaming | **NATS JetStream** | 초기~중기 권장 |
| Analytics | **ClickHouse** | 중기 도입 |
| Search | **OpenSearch** | 역사 검색 규모에 따라 도입 |
| Object Storage | **S3 호환** | 권장 |
| Auth | **OIDC/OAuth2 + short-lived token** | 권장 |
| Observability | **OpenTelemetry + Prometheus + Grafana** | 권장 |
| Container | **Docker** | 권장 |
| Orchestration | **Kubernetes** | 운영 규모가 커진 뒤 |
| CI/CD | **GitHub Actions** | 권장 |
| Load Test | **k6 + Rust bot clients** | 권장 |
| AI | **LLM + RAG** | 표현/검색 계층에 제한 |

---

# 2. 가장 중요한 전문가 판단

현재 기획에서 가장 위험한 것은 기술 자체가 아니다.

## 위험한 조합

```text
Unity
+ ECS
+ Rust
+ WebGL
+ MMO
+ Fleet Combat
+ Event Sourcing
+ NATS
+ PostgreSQL
+ Redis
+ ClickHouse
+ OpenSearch
+ Kubernetes
+ LLM
```

이것을 **MVP부터 모두 넣으면 과설계(over-engineering)**다.

기술적으로 만들 수 있다는 것과 작은 팀이 안정적으로 운영할 수 있다는 것은 완전히 다른 문제다.

따라서:

> **최종 아키텍처와 MVP 아키텍처를 분리해야 한다.**

---

# 3. MVP 권장 Stack

초기에는 다음으로 제한한다.

```text
Unity 6
C#
Rust
Axum
WebSocket
PostgreSQL
Redis
Docker
OpenTelemetry
GitHub Actions
```

그리고:

```text
NATS
ClickHouse
OpenSearch
Kubernetes
LLM
```

은 실제 필요가 확인된 뒤 단계적으로 추가한다.

---

# 4. Client — Unity 6

## 추천

**Unity 6 + C#**

### 담당

- 3D Rendering
- Ship
- Character
- UI
- Galaxy Map
- Combat Presentation
- Warp Effects
- Inventory
- Historical Chronicle
- Audio

### 판단

Unity는 이 프로젝트의 클라이언트 선택으로 적절하다.

하지만 Unity를 서버까지 담당시키는 구조는 피한다.

---

# 5. ECS / DOTS 비판적 평가

## 장점

대량 객체에 강하다.

특히:

- Fleet
- Drone
- Projectile
- NPC
- Asteroid
- Starfield objects

등에 유리하다.

## 문제

모든 것을 ECS로 만들면:

- 개발 복잡성 증가
- 디버깅 어려움
- 팀 학습 비용
- 일반적인 Unity ecosystem과의 통합 난이도

가 증가한다.

## 권장

### Hybrid Architecture

```text
MonoBehaviour / C#
        │
        ├── UI
        ├── Character
        ├── Quest
        ├── Inventory UI
        └── Presentation

ECS / Jobs / Burst
        │
        ├── Fleet
        ├── Projectile
        ├── NPC Crowd
        └── Massive Simulation
```

**ECS-first가 아니라 bottleneck-first**로 적용한다.

---

# 6. Rendering

## 초기

**URP**

를 권장한다.

이 게임의 중요한 것은 초고급 그래픽 자체보다:

- 많은 우주선
- 넓은 공간
- UI 정보량
- 성계 표현
- 함대전

이다.

그래픽 품질보다 CPU/GPU budget을 먼저 관리해야 한다.

---

# 7. Client Networking

가장 중요한 부분 중 하나다.

## 잘못된 접근

모든 것을:

```text
WebSocket
```

하나로 처리.

## 권장

논리적으로 분리한다.

### Reliable

- Login
- Inventory
- Trade
- Contract
- Historical Query
- Character Data

### Realtime

- Position
- Combat State
- Projectile
- Target
- Fleet Command

실시간 전투는 더 낮은 latency를 요구하므로 별도 transport 전략을 사용한다.

---

# 8. WebSocket vs UDP/QUIC

## WebSocket

### 장점
- 구현 단순
- 브라우저 지원
- 방화벽 친화적
- MVP에 적합

### 단점
- 실시간 대규모 전투에 최적이라고 보기는 어렵다.
- 모든 메시지가 동일한 TCP 계열 연결 특성을 가진다.

## QUIC / WebTransport 계열

장기적으로 검토할 가치가 있다.

하지만:

> **처음부터 transport layer를 복잡하게 만들 필요는 없다.**

### 권장

```text
MVP
WebSocket

↓

Combat Scale Test

↓

필요할 경우
QUIC / WebTransport / UDP 계열 도입
```

---

# 9. Server — Rust

## 추천

**Rust**

### 이유

이 게임의 서버는 단순 CRUD backend가 아니다.

```text
World
Combat
Economy
AI
Faction
Diplomacy
History
```

를 장시간 안정적으로 실행해야 한다.

Rust는:

- 메모리 안전성
- async concurrency
- 낮은 runtime overhead
- predictable performance

측면에서 적합하다.

---

# 10. Rust Framework

## HTTP/API

**Axum**

권장.

담당:

- Account
- Character
- Ship
- Inventory
- Market
- History
- Research
- Institution

---

# 11. Async Runtime

### Tokio

Rust 서버의 기본 async runtime으로 사용한다.

하지만:

> Tokio async task와 게임 시뮬레이션 tick을 동일하게 생각하면 안 된다.

---

# 12. Game Simulation

이 부분이 매우 중요하다.

## 추천 구조

```text
Realtime Gateway
       ↓
World Server
       ↓
Simulation Tick
       ↓
State Transition
       ↓
Event
```

게임 시뮬레이션은 명확한 tick/step 모델을 갖는다.

예:

```text
Tick 10001
Tick 10002
Tick 10003
```

이를 통해:

- 재현성
- 디버깅
- replay
- deterministic testing

을 확보한다.

---

# 13. Deterministic Simulation

특히 전투와 경제 계산에서는 가능한 범위에서 deterministic하게 만든다.

예:

```text
World Seed
+
Simulation Tick
+
Entity State
+
Action
=
Deterministic Result
```

이렇게 하면 버그 재현과 replay가 쉬워진다.

---

# 14. Database — PostgreSQL

**핵심 DB로 PostgreSQL을 유지한다.**

적합한 데이터:

- Account
- Character
- Ship
- Inventory
- Item
- Mineral
- Planet
- Star System
- Faction
- Contract
- Territory
- Artifact
- Historical Event
- Evidence
- Claim

---

# 15. Event Sourcing에 대한 비판

이 프로젝트에서 Event Sourcing은 매우 매력적이다.

그러나:

> **모든 게임 상태를 순수 Event Sourcing으로 구현하는 것은 추천하지 않는다.**

왜냐하면:

- 구현 복잡도 증가
- replay 비용 증가
- schema evolution 어려움
- debugging 복잡성
- 운영 난이도

가 커지기 때문이다.

## 권장

### Hybrid Event Architecture

```text
Current State
PostgreSQL

+

Important Domain Events
Event Store / Event Stream
```

즉:

> 모든 총알 발사까지 영구 이벤트로 저장하지 않는다.

---

# 16. 무엇을 History Event로 저장할 것인가?

### 저장

- Player Death
- Ship Destruction
- Mineral Discovery
- Artifact Discovery
- War Declaration
- Treaty
- Faction Creation
- Territory Capture
- Major Battle
- Political Assassination
- Major Economic Crisis

### 저장하지 않을 가능성이 높은 것

- 매 프레임 위치
- 일반적인 이동
- 모든 총알
- UI 행동
- 단순 NPC 이동

이 구분이 매우 중요하다.

---

# 17. Redis

Redis는:

> **Source of Truth가 아니다.**

사용:

- Session
- Presence
- Cache
- Temporary World State
- Rate Limit
- Distributed Lock이 필요한 제한적인 영역

영속 데이터는 PostgreSQL에 둔다.

---

# 18. NATS JetStream

## 초기에는 선택 사항

NATS를 넣으면:

```text
World
 ↓
NATS
 ├── History
 ├── News
 ├── Analytics
 └── Notification
```

구조를 만들기 쉽다.

하지만 MVP에서 서비스가 몇 개 안 된다면 PostgreSQL Outbox Pattern으로도 충분할 수 있다.

## 권장 전략

### MVP

```text
PostgreSQL
+
Transactional Outbox
```

### Scale-up

```text
PostgreSQL
 ↓
NATS JetStream
```

이렇게 단계적으로 간다.

---

# 19. Transactional Outbox

초기에는 이 패턴을 강하게 추천한다.

예:

```text
Transaction

1. Ship destroyed
2. Historical event inserted
3. Outbox event inserted

COMMIT
```

그 다음 background worker가:

```text
Outbox
 ↓
NATS
```

로 전달한다.

이렇게 하면 DB 저장과 이벤트 발행 사이의 불일치 위험을 줄일 수 있다.

---

# 20. ClickHouse

이 게임은 장기적으로 ClickHouse와 매우 잘 맞는다.

예:

```text
Billions of events

Player Actions
Market
Combat
Travel
Mining
Warp
Historical Events
```

분석:

- 어떤 광물이 가장 많이 채굴되는가?
- 어느 항로가 위험한가?
- 어느 직업이 많이 선택되는가?
- 어떤 전쟁이 경제에 영향을 주었는가?
- PvP hotspot은 어디인가?

### 단

MVP부터 ClickHouse를 넣지는 않는다.

---

# 21. OpenSearch

Historical Chronicle 검색이 커지면 도입한다.

예:

> "Vesta에서 발생한 전쟁을 찾아줘"

검색 대상:

- Event
- Character
- Ship
- Artifact
- Faction
- News
- Claim
- Evidence

초기 데이터가 작으면 PostgreSQL Full Text Search로 시작할 수 있다.

---

# 22. Object Storage

S3 호환 storage.

용도:

- Historical Documents
- Images
- Museum Assets
- Reports
- Large Logs
- User Generated Media

DB에 binary를 넣지 않는다.

---

# 23. AI / LLM

AI를 시스템의 중심에 두지 않는다.

### AI가 하면 안 되는 것

```text
LLM
 ↓
"전쟁이 발생했습니다."
```

### 올바른 구조

```text
Simulation
 ↓
WAR_DECLARED
 ↓
Historical Event
 ↓
LLM
 ↓
News / Summary / NPC Dialogue
```

### AI 역할

- News Generation
- Historical Summary
- NPC Dialogue
- Natural Language Search
- Research Assistant
- Translation
- Lore Explanation

---

# 24. Historical Engine

이 게임의 핵심 proprietary system.

## Data

```text
Event
Evidence
Claim
Interpretation
Consequence
```

## 구조

```text
ACTION
 ↓
EVENT
 ↓
EVIDENCE
 ↓
CLAIM
 ↓
INTERPRETATION
 ↓
CONSEQUENCE
```

예:

```text
Player destroys convoy
 ↓
SHIP_DESTROYED
 ↓
Ship Log
 ↓
Witness Testimony
 ↓
"Intentional Attack"
 ↓
Political Crisis
 ↓
War
```

---

# 25. History Engine은 AI보다 Rule Engine이 중심이어야 한다

### Rule-based

```text
IF
convoy_destroyed > threshold
AND
faction_relation < X

THEN
create_political_incident
```

LLM:

```text
"이 사건을 어떻게 설명할 것인가?"
```

이 구조가 안전하다.

---

# 26. Security

필수:

- Server Authority
- Input Validation
- Rate Limiting
- Replay Protection
- Anti-cheat telemetry
- Signed/validated commands
- Economy anomaly detection

특히 경제 MMO에서는:

> **Currency duplication**

이 게임의 가장 위험한 버그 중 하나다.

---

# 27. Economy Architecture

돈과 아이템 변경은 항상 서버에서 검증한다.

```text
Client
 ↓
Trade Request
 ↓
Server Validation
 ↓
DB Transaction
 ↓
Inventory Update
 ↓
Economy Event
```

절대:

```text
Client → "돈 +100000"
```

같은 구조를 허용하지 않는다.

---

# 28. Microservices에 대한 비판

처음부터:

```text
Auth Service
World Service
Combat Service
Economy Service
History Service
News Service
Research Service
Faction Service
```

등으로 쪼개는 것을 추천하지 않는다.

### 초기에는 Modular Monolith

```text
Rust Backend

├── auth
├── character
├── ship
├── economy
├── combat
├── history
└── faction
```

하나의 배포 단위로 시작한다.

---

# 29. 이후 Scale-out

필요할 때만:

```text
Gateway
World
Combat
Economy
History
```

로 분리한다.

서비스 분리는 기술적 멋보다 실제 병목과 장애 격리를 기준으로 한다.

---

# 30. Kubernetes에 대한 비판

Kubernetes는 강력하지만 MVP에 필요하지 않다.

초기:

```text
Docker
+
Cloud VM
```

정도로 시작한다.

사용자 수와 서비스 수가 증가하면:

```text
Kubernetes
```

를 도입한다.

---

# 31. Observability

이 프로젝트에서는 매우 중요하다.

추천:

### OpenTelemetry

trace

### Prometheus

metrics

### Grafana

dashboard

### Log system

structured JSON logs

---

# 32. 반드시 추적해야 하는 Trace

예:

```text
Player Mine Request
 ↓
Gateway
 ↓
World Server
 ↓
Mineral Validation
 ↓
Inventory Transaction
 ↓
Historical Event
 ↓
News Event
```

하나의 request_id / trace_id로 연결한다.

---

# 33. Load Testing

일반적인 HTTP 부하 테스트만으로는 부족하다.

### k6

API/connection load.

### Rust Bot Client

실제 게임 행동을 시뮬레이션한다.

예:

```text
10,000 Bots
 ├── 2,000 Miners
 ├── 2,000 Traders
 ├── 2,000 Explorers
 ├── 2,000 Combatants
 └── 2,000 NPC-like actors
```

이런 테스트가 필요하다.

---

# 34. WebGL/WebAssembly에 대한 비판

브라우저 버전은 매력적이지만:

> **PC Client와 동일한 기능/성능을 처음부터 목표로 잡으면 위험하다.**

브라우저에서는:

- 메모리
- 다운로드 크기
- CPU
- GPU
- 네트워크
- 브라우저 정책

등의 제약이 있다.

### 권장

PC:

**Primary**

Web:

**Secondary / Accessible Client**

로 설계한다.

---

# 35. Recommended Architecture — Phase 1

```text
                    Unity 6
                       │
                WebSocket / HTTPS
                       │
                Rust Modular Monolith
                       │
        ┌──────────────┼──────────────┐
        ▼              ▼              ▼
      World          Combat        Economy
        │              │              │
        └──────────────┼──────────────┘
                       ▼
                  PostgreSQL
                       │
                     Redis
                       │
                 Outbox Worker
```

이 정도면 충분하다.

---

# 36. Recommended Architecture — Phase 2

```text
                     Clients
                        │
                   Gateway
                        │
          ┌─────────────┼─────────────┐
          ▼             ▼             ▼
       World         Combat        API
          │             │             │
          └─────────────┼─────────────┘
                        ▼
                 Event / Outbox
                        │
                     NATS
                ┌───────┼───────┐
                ▼       ▼       ▼
             History   News   Analytics
                │       │       │
                ▼       ▼       ▼
           PostgreSQL OpenSearch ClickHouse
```

---

# 37. Recommended Architecture — Scale

```text
                      CDN
                       │
                   API Gateway
                       │
              ┌────────┴─────────┐
              │                  │
        Realtime Gateway       API
              │                  │
      ┌───────┼────────┐         │
      ▼       ▼        ▼         ▼
   World-A  World-B  World-C   Backend
      │       │        │
      └───────┼────────┘
              ▼
       Simulation Cluster
              │
              ▼
          Event Stream
              │
   ┌──────────┼───────────┐
   ▼          ▼           ▼
History     Economy    Analytics
   │          │           │
   ▼          ▼           ▼
Postgres    Postgres   ClickHouse
   │
   ▼
OpenSearch
```

---

# 38. World Sharding

대규모 MMO가 되면 모든 성계를 하나의 서버에서 처리하지 않는다.

예:

```text
Shard 01
Sol Sector

Shard 02
Orion Sector

Shard 03
Vesta Sector
```

플레이어가 워프하면:

```text
Shard A
 ↓
Transfer
 ↓
Shard B
```

가 필요하다.

---

# 39. 하지만 Sharding은 너무 일찍 하지 않는다

초기에는:

```text
1 World Server
```

로 시작한다.

성능 측정 후:

```text
Region / Star System partitioning
```

을 적용한다.

---

# 40. History Sharding

역사 데이터는 일반 게임 상태보다 수명이 길다.

따라서:

```text
Hot History
Recent Events

Cold History
Old Events
```

로 분리할 수 있다.

예:

PostgreSQL:

> 최근/중요 사건

Object Storage / Columnar archive:

> 오래된 원시 이벤트

OpenSearch:

> 검색용 index

---

# 41. Data Ownership

명확하게 정해야 한다.

### PostgreSQL

Canonical persistent state

### Redis

Temporary state/cache

### NATS

Event transport

### ClickHouse

Analytics

### OpenSearch

Search index

### S3

Large objects/archive

이 원칙을 지키면 시스템이 훨씬 단순해진다.

---

# 42. 기술 스택 최종안

## Phase 1

```text
Unity 6
C#
URP
Selective ECS/DOTS
Rust
Tokio
Axum
WebSocket
PostgreSQL
Redis
Docker
OpenTelemetry
Prometheus
Grafana
GitHub Actions
```

## Phase 2

```text
NATS JetStream
ClickHouse
OpenSearch
S3
Dedicated Game Servers
```

## Phase 3

```text
Kubernetes
QUIC/WebTransport
World Sharding
Simulation Cluster
Advanced Anti-cheat
LLM/RAG Platform
```

---

# 43. 가장 중요한 개발 원칙

## Rule 1

**Client is never authoritative.**

## Rule 2

**LLM never creates canonical game state.**

## Rule 3

**Redis is never the source of truth.**

## Rule 4

**Not every event is a historical event.**

## Rule 5

**Do not start with microservices.**

## Rule 6

**Do not start with Kubernetes.**

## Rule 7

**Do not use ECS everywhere.**

## Rule 8

**Do not optimize for 10,000 players before proving fun with 30–100 players.**

## Rule 9

**Historical systems must be deterministic and auditable.**

## Rule 10

**Every major technical choice must be validated by profiling/load testing.**

---

# 44. 전문가 최종 평가

현재 기술 방향은 **충분히 실현 가능한 방향**이지만, 원래 계획대로 모든 기술을 한꺼번에 적용하면 과설계 위험이 높다.

특히 가장 큰 위험은:

1. MMO 규모를 너무 빨리 가정하는 것
2. WebGL을 너무 일찍 핵심 플랫폼으로 고정하는 것
3. ECS를 모든 코드에 적용하는 것
4. Event Sourcing을 모든 state에 적용하는 것
5. Microservices/Kubernetes를 초기부터 도입하는 것
6. LLM을 Simulation Layer에 넣는 것
7. 10,000 Minerals 같은 숫자를 기술 목표로 착각하는 것

이다.

반대로 다음 구조는 상당히 강력하다.

```text
Unity
   +
Rust Authoritative Simulation
   +
PostgreSQL
   +
Event-driven History
   +
Evidence / Claim Model
```

이 다섯 가지가 **STARFALL DYNASTY의 기술적 핵심**이 되어야 한다.

---

# 45. 개발팀이 실제로 먼저 만들어야 하는 것

화려한 함대전보다 먼저 다음 Vertical Slice를 만든다.

```text
Player A
  ↓
Discover Rare Mineral
  ↓
Player B Tracks A
  ↓
PvP
  ↓
A's Ship Destroyed
  ↓
B Takes Cargo
  ↓
Historical Event Created
  ↓
Evidence Created
  ↓
Player C Investigates
  ↓
Historical Claim
  ↓
News Article
  ↓
Faction Reacts
```

이 하나의 시나리오가:

- Unity
- Rust
- Network
- PostgreSQL
- Event System
- History Engine
- PvP
- Economy
- AI

를 동시에 검증한다.

**이 Vertical Slice가 재미있고 안정적으로 동작한다면 그때 서버 규모와 기술을 확장한다.**

---

# 46. 최종 Tech Stack Decision

### 🟢 Keep

- Unity 6
- C#
- Rust
- PostgreSQL
- Redis
- Docker
- OpenTelemetry
- GitHub Actions

### 🟡 Introduce Later

- ECS/DOTS
- NATS
- ClickHouse
- OpenSearch
- S3
- Dedicated Server Cluster
- QUIC/WebTransport
- Kubernetes

### 🔴 Avoid Initially

- Full Microservices
- Full Event Sourcing
- Kubernetes-first
- ECS-everywhere
- LLM-driven simulation
- 10,000-player performance target
- WebGL-first architecture

---

# 47. North Star Architecture

최종적으로 STARFALL DYNASTY는 다음 구조를 목표로 한다.

```text
                PLAYER
                   │
                   ▼
              UNITY 6
                   │
                   ▼
          AUTHORITATIVE RUST
                   │
        ┌──────────┼──────────┐
        ▼          ▼          ▼
      WORLD      COMBAT    ECONOMY
        │          │          │
        └──────────┼──────────┘
                   ▼
              DOMAIN EVENT
                   │
                   ▼
          HISTORICAL ENGINE
                   │
        ┌──────────┼──────────┐
        ▼          ▼          ▼
     Evidence    Claims     Consequences
        │          │          │
        └──────────┼──────────┘
                   ▼
             GALACTIC HISTORY
                   │
                   ▼
              NEXT PLAYER
                   │
                   ▼
              NEW ACTION
```

## 이 구조가 STARFALL DYNASTY의 기술적 정체성이다.

**게임 서버가 플레이어의 행동을 시뮬레이션하고, 그 행동을 역사적 사건으로 변환하며, 그 역사 자체가 다시 다음 플레이어의 콘텐츠가 되는 것.**

이것이 단순한 "Unity 우주 MMO"와 이 프로젝트를 구분하는 가장 중요한 기술적 구조다.

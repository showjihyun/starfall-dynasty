# STARFALL DYNASTY
## 역사(History)를 플레이하는 3D 우주 MMO — Game Design & Technical Plan

> **THE GALAXY IS NOT A MAP. IT IS A HISTORY.**
>
> **당신은 역사를 읽는 사람이 아니다. 당신이 역사를 만든다.**

---

## 0. Executive Summary

### 한 줄 정의

**STARFALL DYNASTY**는 3개 종족, 다수의 직업, 개인 우주선 성장, 채굴·무역·탐험·전투·첩보·정치·고고학·역사학을 결합하고, 플레이어의 행동을 장기적인 **역사적 기록과 세계 변화**로 축적하는 온라인 3D 우주 게임이다.

### 핵심 차별점

기존 MMO의 핵심 루프:

`레벨업 → 장비 획득 → 더 강한 콘텐츠`

STARFALL DYNASTY:

`행동 → 결과 → 사건 → 기록 → 해석 → 새로운 갈등 → 새로운 역사`

게임의 핵심 메타 콘텐츠는 **Historical Consequence System**과 **Galactic Chronicle**이다.

---

# 1. Design Pillars

## P1. History First

Lore를 읽는 게임이 아니라 **역사를 발생시키는 게임**을 만든다.

## P2. Player Agency

플레이어는 광부, 상인, 탐험가, 군인, 해적, 정치인, 외교관, 역사학자 등 서로 다른 삶을 선택할 수 있다.

## P3. Persistent Consequence

중요 행동은 일회성 퀘스트 결과가 아니라 세계 상태와 기록에 영향을 준다.

## P4. Risk Creates Stories

높은 보상에는 위험이 따른다. 함선, 화물, 정보, 평판, 영토 등을 잃을 수 있어야 한다.

## P5. Three Species, Deep Civilisations

초기 종족은 3개로 제한한다. 대신 각 종족 내부의 국가·문화·가문·조직을 깊게 만든다.

## P6. Systemic Content

개발자가 퀘스트를 무한히 생산하는 방식이 아니라 경제·전쟁·외교·탐험·역사 시스템이 서로 새로운 콘텐츠를 만들어내도록 한다.

---

# 2. Three Species

## 2.1 HUMAN

### 문명 키워드
- 기록
- 산업
- 무역
- 정치
- 식민지
- 군사

### 역사관

**역사 = 기록**

문서, 정부 기록, 신문, 법률, 지도, 군사 보고서 등이 역사적 증거가 된다.

### 플레이 스타일 예시
- Trader
- Politician
- Engineer
- Admiral
- Historian
- Explorer

---

## 2.2 VEYR

### 문명 키워드
- 기억
- 집단 기억
- 장수
- 생체기술
- 기억 보존

### 역사관

**역사 = 기억**

중요한 사건이 개인 또는 집단의 기억에 저장된다.

인간 기록과 Veyr 기억이 충돌할 수 있다.

예:

> Human Archive: "Imperial forces initiated the attack."
>
> Veyr Memory: "The attack began three days earlier."

### 플레이 스타일 예시
- Memory Keeper
- Navigator
- Archaeologist
- Diplomat
- Scientist
- Archivist

---

## 2.3 KHARZ

### 문명 키워드
- 계약
- 혈통
- 명예
- 전쟁
- 채무
- 용병

### 역사관

**역사 = 계약과 혈통**

전쟁 기록, 혈맹, 계약, 가문 계보가 역사적 권리와 책임을 결정한다.

### 플레이 스타일 예시
- Mercenary
- Bounty Hunter
- Soldier
- Privateer
- Clan Leader
- War Historian

---

# 3. Civilization Design

종족과 국가를 동일시하지 않는다.

예:

`Human`
- Empire A
- Republic B
- Merchant League C

`Veyr`
- Memory Houses
- Independent Clans
- Research Orders

`Kharz`
- Warrior Houses
- Contract Guilds
- Mercenary Leagues

같은 종족도 서로 적대하거나 동맹할 수 있다.

---

# 4. Character System

## 4.1 Character ≠ Ship

플레이어 캐릭터와 우주선은 별도의 성장 객체다.

```text
PLAYER
 ├── Character
 │    ├── Skills
 │    ├── Reputation
 │    ├── Profession
 │    ├── Biography
 │    └── Historical Record
 │
 └── Starship
      ├── Hull
      ├── Reactor
      ├── Engine
      ├── Warp Core
      ├── Shield
      ├── Weapons
      ├── Cargo
      └── Modules
```

---

# 5. Profession System

초기 목표: **20~30개 직업/전문화**

## Exploration
1. Pilot
2. Explorer
3. Navigator
4. Courier
5. Scout

## Industry
6. Miner
7. Prospector
8. Engineer
9. Shipwright
10. Refinery Specialist

## Economy
11. Trader
12. Broker
13. Merchant
14. Banker

## Military
15. Soldier
16. Marine
17. Combat Pilot
18. Fleet Officer
19. Admiral

## Political / Social
20. Diplomat
21. Politician
22. Journalist
23. Intelligence Agent

## History / Science
24. Historian
25. Archaeologist
26. Linguist
27. Archivist
28. Genealogist
29. Cartographer
30. Scientist

### 중요한 원칙

직업은 단순한 스킬트리가 아니다.

각 직업은 **세계와 상호작용하는 고유한 활동**을 가진다.

예:

Historian → 증거 수집 → 사건 재구성 → 역사 논쟁

Archaeologist → 유적 발견 → 유물 발굴 → 감정 → 박물관/시장

Journalist → 취재 → 인터뷰 → 기사 → 여론 변화

Bounty Hunter → 현상금 추적 → PvP → 증거 제출 → 보상

---

# 6. Starship System

## 6.1 One-Person Ship Fantasy

게임 시작 시 플레이어는 작은 1인 우주선으로 시작한다.

예:

**Scout S-01**

- Crew: 1
- Cargo: 20t
- Weapon: 1
- Warp Range: 1.2 ly
- Shield: 100

## 6.2 Ship Growth

`Scout → Courier → Miner → Corvette → Frigate → Destroyer → Cruiser → Battlecruiser → Battleship → Dreadnought → Leviathan`

단순 레벨업이 아니라 모듈과 함체의 조합으로 성장한다.

## 6.3 Modules

- Hull
- Reactor
- Engine
- Warp Core
- Shield
- Armour
- Weapon
- Cargo
- Scanner
- Mining System
- Drone Bay
- Command Module

## 6.4 Ship Identity

각 함선은 영구적인 역사를 가진다.

예:

**SNS Aurora**

- Launch Date
- Previous Owners
- Discoveries
- Battles
- Captains
- Major Repairs
- Famous Cargo
- Destruction / Survival
- Historical Events

함선의 외관에도 전투 흔적과 개조 이력이 남는다.

---

# 7. Resource System

## 7.1 10,000+ Minerals

광물은 전부 수작업으로 정의하지 않는다.

절차적 생성 + 데이터 기반 분류를 사용한다.

주요 속성:

- Density
- Hardness
- Energy Capacity
- Thermal Resistance
- Radiation
- Conductivity
- Warp Compatibility
- Weapon Compatibility
- Industrial Value
- Rarity
- Market Value

## 7.2 Deterministic Generation

개념:

`Seed + Planet Type + Star Type + Depth + Radiation + Age + Geological Event`

동일한 seed와 조건이면 동일한 자원이 생성될 수 있어 검증과 재현이 가능하다.

## 7.3 Economy

가격은 고정값이 아니다.

`Supply + Demand + Production + War + Logistics + Local Scarcity`

에 의해 변한다.

---

# 8. Exploration

탐험 콘텐츠:

- 미발견 성계
- 고대 유적
- 폐허 행성
- 버려진 함선
- 이상 현상
- 고대 워프 게이트
- 희귀 광물
- 실종 문명
- 미확인 신호

탐험 결과는 **Historical Discovery**로 발전할 수 있다.

---

# 9. Warp System

## 단계

1. Normal Warp
2. Deep Warp
3. Galactic Warp
4. Endgame Unknown Warp

워프 기술이 발전할수록 이동 가능 거리와 위험도가 증가한다.

### 연출

- 중력장 형성
- 별빛 왜곡
- 엔진 압축
- 공간 균열
- 워프 터널
- 목적지 성계 출현

---

# 10. Combat

전투는 직접 조종과 함대 지휘를 단계적으로 확장한다.

### 초기
1인 함선 전투

### 중기
AI 승무원 + 드론

### 후기
개인 기함 + 호위함

### Endgame
개인 함대 + 대규모 함대전

전투 기록은 Historical Chronicle에 저장한다.

---

# 11. Aggressive Gameplay

이 게임의 공격적인 재미는 단순 PvP가 아니라 **높은 위험과 지속적인 결과**에서 나온다.

## 11.1 Pirate

상선과 희귀 화물을 노린다.

## 11.2 Bounty Hunter

현상금 대상 추적 및 체포/격파.

## 11.3 Smuggler

금지 자원, 유물, 기술 등을 위험 지역으로 운송.

## 11.4 Privateer

국가의 허가를 받아 적대 세력의 물류를 공격.

## 11.5 Spy

정보, 좌표, 군사 계획을 확보한다.

## 11.6 Saboteur

워프 게이트, 생산 시설, 함대 지원시설을 교란한다.

## 11.7 Political Assassin

게임 규칙으로 허용된 특정 정치적 목표에 대한 위험한 임무.

## 11.8 Betrayal

동맹과 계약을 깨는 행동 자체가 중요한 플레이가 된다.

---

# 12. Wanted & Bounty

심각한 범죄/적대 행동에는 현상금이 발생할 수 있다.

예:

```text
ARIN VAL
Wanted in 7 systems

Charges
- Convoy destruction
- Smuggling
- Restricted technology theft

Bounty
4,820,000 Credits
```

현상금은 다른 플레이어에게 새로운 콘텐츠를 만든다.

---

# 13. Historical Consequence System

## 핵심

플레이어 행동이 사건 데이터로 변환된다.

```text
Player Action
     ↓
World State Change
     ↓
Historical Event
     ↓
Evidence
     ↓
Historical Record
     ↓
Future Interpretation
```

---

# 14. Galactic Chronicle

모든 중요한 사건을 기록한다.

예:

## Battle of Orion

- Date
- Location
- Participants
- Ships
- Casualties
- Resources Consumed
- Territory Change
- Political Consequence
- Evidence

참여 플레이어와 함선도 기록한다.

---

# 15. Historical Biography

플레이어 캐릭터에게 자동 생성되는 역사 기록.

예:

```text
ARIN VAL

3827
Discovered Orion mineral field

3828
Founded Orion Mining Consortium

3831
Participated in Vesta War

3834
Accused of political betrayal

3835
Evidence recovered

3837
Killed at Vesta Corridor
```

플레이어의 "캐릭터 카드"가 시간이 지나면서 역사적 전기가 된다.

---

# 16. Death & Legacy

죽음은 단순한 respawn이 아니다.

중요 인물의 사망은 역사적 사건이 될 수 있다.

```text
THE DEATH OF ARIN VAL

Date
Location
Killer
Witnesses
Ship
Political Context
Evidence
```

이후 플레이어는 새로운 캐릭터를 만들 수 있으며, 가문·조직·재산 등 일부 legacy 시스템을 통해 이전 캐릭터와 연결될 수 있다.

---

# 17. Revenge & Conflict

예:

`Player A 죽음`

→ 친구/가문/조직의 복수

→ Player B와 충돌

→ 길드 개입

→ 영토 분쟁

→ 정치 개입

→ 전쟁

→ 역사적 사건

중요한 것은 게임이 복수를 강제하지 않는다는 점이다. 복수, 용서, 무관심, 정치적 협상 모두 플레이 선택이다.

---

# 18. Historical Evidence System

역사적 사실을 단일 DB의 "정답"으로 고정하지 않는다.

증거 유형:

- Government Archive
- Military Record
- Personal Diary
- Player Testimony
- Archaeological Evidence
- Veyr Memory
- Kharz Contract
- Ship Log
- News Report

각 자료에는 출처와 신뢰성 정보가 존재한다.

플레이어는 자료를 비교해 사건을 재구성한다.

---

# 19. Historical Interpretation

같은 사건에 여러 해석이 존재할 수 있다.

예:

### Imperial Archive
"Defensive War"

### Rebel Archive
"Occupation"

### Veyr Memory
"Third Betrayal"

### Archaeological Evidence
"Mass Evacuation"

플레이어 Historian은 자료를 연결해 자신의 연구를 발표할 수 있다.

---

# 20. Historian Gameplay

## 활동

1. 유적 조사
2. 문서 수집
3. 함선 로그 분석
4. 인터뷰
5. 언어 해독
6. 족보 연구
7. 오래된 지도 복원
8. 사건 재구성
9. 논문/연구 보고서 작성
10. 박물관/연구기관 운영

## 보상

- Credits
- Reputation
- Research Rights
- Museum Sponsorship
- Political Contracts
- Rare Artifact Access
- Historical Recognition

---

# 21. Archaeology & Artifact Economy

고대 유물은 거래 가능한 희귀 자산이 될 수 있다.

예:

**Crown of Orion**

플레이어 선택:

- 판매
- 박물관 기증
- 연구소 기증
- 개인 소장
- 정치 세력에 제공
- 밀수

유물이 사라지면 이후 다른 플레이어의 **Lost Artifact Hunt** 콘텐츠가 발생한다.

---

# 22. Information Warfare

전투는 물리적인 전투만 존재하지 않는다.

게임 내 시스템으로:

- 정보 탈취
- 기밀 문서 확보
- 기록 공개
- 첩보
- 언론 경쟁
- 연구 결과 경쟁
- 역사 해석 경쟁

등을 구현한다.

단, 현실의 해킹/범죄 방법을 재현하지 않고 게임 내부의 추상화된 메커니즘으로 구현한다.

---

# 23. Galactic Herald

게임 세계의 주요 사건을 자동으로 편집하는 뉴스 시스템.

예:

> GALACTIC HERALD
>
> ORION BLOCKADE ENTERS 17TH DAY
>
> Imperial forces have blockaded the Orion system following destruction of three military convoys.
>
> NEW HISTORICAL DISCOVERY
>
> Historian Elena Val uncovered evidence concerning the Vesta War.

Journalist 플레이어는 추가 취재와 기사 작성에 참여할 수 있다.

---

# 24. Museums & Institutions

플레이어가 기관을 설립할 수 있다.

예:

**Orion Historical Institute**

- 유물 전시
- 역사 연구
- 학술 자료
- 플레이어 기증품
- 전쟁 기록
- 교육 콘텐츠

게임 세계의 지식 인프라 자체를 플레이어가 만들 수 있다.

---

# 25. Historical Simulation Engine

세계 상태를 지속적으로 업데이트한다.

```text
Population
Economy
Resources
Technology
Politics
Diplomacy
Military
Migration
Culture
```

결과:

```text
Shortage
→ Inflation
→ Unemployment
→ Migration
→ Political Unrest
→ Coup
→ Civil War
→ Refugees
→ Diplomatic Crisis
→ War
→ New Historical Record
```

---

# 26. Server History Model

역사 데이터는 이벤트 기반으로 설계한다.

핵심 개념:

**Event Sourcing**

모든 중요한 상태 변화의 원인이 되는 이벤트를 기록한다.

예:

```text
EVENT: MINERAL_DISCOVERED
ACTOR: Player123
LOCATION: Orion-7
RESOURCE: X-847
TIME: 3827-03-12
```

```text
EVENT: SHIP_DESTROYED
ACTOR: Player456
TARGET: Aurora
LOCATION: Vesta
TIME: 3831-09-02
```

이 이벤트들을 이용해 현재 상태와 역사적 타임라인을 재구성한다.

---

# 27. Technical Architecture

## Client

### Unity 6

주요 기술:

- Unity 6
- C#
- Entities / ECS
- Jobs
- Burst
- Addressables
- Netcode for Entities 검토
- WebGL/WebAssembly

### 목표

- 대규모 객체 처리
- 함대전
- 3D 우주 렌더링
- 브라우저 클라이언트 확장

---

# 28. Server

### Rust

서버의 핵심 시뮬레이션과 authoritative logic에 사용한다.

추천 역할:

- World Simulation
- Combat Simulation
- Economy
- Event Processing
- Historical Engine
- Anti-Cheat Validation
- Match/Session Coordination

Rust의 장점:

- 메모리 안전성
- 낮은 런타임 오버헤드
- 높은 동시성
- 서버 장기 실행에 유리한 안정성

---

# 29. Backend Architecture

```text
                 Client
                   │
             API / Gateway
                   │
        ┌──────────┼──────────┐
        ▼          ▼          ▼
     Gateway     World      Combat
        │          │          │
        └──────┬───┴──────┬───┘
               ▼          ▼
             Event Bus / Stream
                    │
        ┌───────────┼───────────┐
        ▼           ▼           ▼
     Economy     History      Analytics
        │           │           │
        └───────────┼───────────┘
                    ▼
            PostgreSQL / Redis
```

---

# 30. Data Layer

## PostgreSQL

정합성이 필요한 영속 데이터:

- Account
- Character
- Ship
- Inventory
- Item
- Faction
- Contract
- Territory
- Historical Event
- Artifact
- Institution

## Redis

빠른 상태:

- Session
- Presence
- Cache
- Short-lived World State
- Rate Limit
- Match State

## Object Storage

- Historical documents
- screenshots
- generated reports
- large logs
- game assets metadata

---

# 31. Event Bus

초기에는 단순화한다.

### MVP

Redis Streams 또는 PostgreSQL 기반 Event Queue

### Scale-up

Kafka/Redpanda/NATS 계열을 검토

목적:

- Event processing
- History pipeline
- Analytics
- News generation
- Audit
- Async world simulation

---

# 32. API

### REST / HTTP

사용:

- Login
- Character
- Inventory
- Market
- History query
- Research
- Institution

### Realtime

게임 상태에는 WebSocket 기반 프로토콜부터 시작한다.

브라우저 환경의 transport 제약과 Unity WebGL 지원 상황을 실제 MVP에서 검증한 뒤 WebTransport 등의 적용 여부를 결정한다.

---

# 33. Server Authority

클라이언트는 신뢰하지 않는다.

예:

Client:

> Mine Resource X

Server:

1. 플레이어 위치 확인
2. 행성 상태 확인
3. 광물 존재 확인
4. 장비 확인
5. 채굴 가능 여부 확인
6. 경제/자원 상태 반영
7. Inventory 변경
8. Historical Event 생성

---

# 34. History Data Model

최소 핵심 구조:

```text
HistoricalEvent
 ├── event_id
 ├── timestamp
 ├── location_id
 ├── event_type
 ├── actors[]
 ├── objects[]
 ├── cause_event_ids[]
 ├── consequence_event_ids[]
 ├── evidence_ids[]
 ├── public_visibility
 └── importance
```

Evidence:

```text
Evidence
 ├── evidence_id
 ├── type
 ├── source_actor
 ├── creation_time
 ├── authenticity
 ├── reliability
 └── related_events[]
```

---

# 35. Critical Technical Decision

## 처음부터 거대한 MMO로 만들지 않는다.

가장 큰 위험은 다음이다.

- Unity WebGL
- 대규모 실시간 함대전
- MMO persistence
- 경제 시뮬레이션
- 역사 시뮬레이션
- 10,000 minerals
- 수천 성계

를 동시에 구현하려는 것.

이는 프로젝트 실패 확률을 크게 높인다.

### 따라서 Vertical Slice를 먼저 만든다.

---

# 36. MVP Scope

## MVP-1

### "One System, One History"

- 1 성계
- 1 행성
- 3 종족
- 5 직업
- 1인 우주선
- 10~30 광물
- 채굴
- 거래
- 기본 전투
- 10~30명 동시 접속
- Historical Event
- Galactic Chronicle

### 가장 중요한 테스트

**"플레이어가 행동했을 때 다른 플레이어가 그 행동의 흔적을 발견할 수 있는가?"**

---

# 37. MVP-2

- 5~10 성계
- 10+ 직업
- 100+ 광물
- 시장
- 길드
- 현상금
- 해적
- 탐험
- 유적
- 유물
- 기본 역사학
- 뉴스

---

# 38. Vertical Slice

## 목표

"한 명의 플레이어가 역사적 인물이 되는 경험"

시나리오:

```text
Start
 ↓
Scout Ship
 ↓
Mineral Discovery
 ↓
Rare Resource
 ↓
Trade
 ↓
Wealth
 ↓
Political Contract
 ↓
Betrayal
 ↓
PvP
 ↓
Ship Destroyed
 ↓
Historical Event
 ↓
Another Player Investigates
 ↓
Historical Biography
```

이 하나의 흐름이 재미있다면 확장한다.

---

# 39. Phase Roadmap

## Phase 0 — Pre-production
### 4~6주

- GDD
- Core Loop
- Technical Prototype
- Historical Event Schema
- Networking Spike
- Unity rendering benchmark
- Rust simulation benchmark
- WebGL feasibility test

### Exit Criteria

"30명 동시 접속 + 실시간 이벤트 기록" 검증.

---

## Phase 1 — Core Prototype
### 2~3개월

- 1 성계
- 1 행성
- 1인 우주선
- 이동
- 채굴
- 인벤토리
- 거래
- 전투
- 서버 authoritative logic
- Historical Event

---

## Phase 2 — Historical Vertical Slice
### 2~3개월

- Historian
- Archaeologist
- Evidence
- Historical Reconstruction
- Galactic Chronicle
- Biography
- Wanted
- Bounty
- Death Record
- News

### 핵심 목표

**"플레이어가 만든 사건을 다른 플레이어가 소비한다."**

---

## Phase 3 — Social World
### 3~4개월

- Guild
- Faction
- Contracts
- Diplomacy
- Political Influence
- Player Institution
- Museum
- Journalist
- Information Warfare

---

## Phase 4 — Galaxy Expansion
### 4~6개월

- 수십~수백 성계
- procedural resource
- exploration
- warp
- rare technology
- larger ships
- fleet combat

---

## Phase 5 — Persistent MMO
### 6~12개월+

- 수천 성계
- 대규모 경제
- 영토
- 국가
- 대규모 전쟁
- 개인 함대
- 고급 Historical Simulation
- large-scale fleet combat

실제 기간은 팀 규모와 기술 검증 결과에 따라 달라진다.

---

# 40. Recommended Team

초기 Vertical Slice 기준:

### Engineering

- Unity Client Engineer × 2
- Rust Backend Engineer × 2
- Gameplay/System Engineer × 1
- Technical Artist × 1

### Design

- Game Designer × 1
- Economy/System Designer × 1
- Narrative/History Designer × 1

### Art

- 3D Generalist × 1
- UI/UX × 1

### QA

- QA/Automation × 1

초기에는 일부 역할을 겸임한다.

---

# 41. Critical Expert Review

## 문제 1 — Scope Explosion

현재 기획은 매력적이지만 **위험할 정도로 크다.**

동시에 MMO + 4X + RPG + Space Sim + PvP + Economy + Historical Simulation을 구현하려 하면 실패하기 쉽다.

### 대응

첫 번째 제품은:

**"Space MMO"가 아니라 "Historical Multiplayer Sandbox"**

로 정의한다.

---

# 42. 문제 2 — 역사 시스템이 재미없는 데이터베이스가 될 위험

역사 기록만 많이 쌓이면 플레이어는 읽지 않는다.

### 대응

모든 Historical Event는 플레이 가능한 결과와 연결한다.

예:

`전투 기록`

→ 유물

→ 현상금

→ 새로운 퀘스트

→ 정치적 분쟁

→ 탐험

→ 후속 전쟁

---

# 43. 문제 3 — PvP가 신규 유저를 압살할 위험

공격적인 시스템은 재미있지만 신규 유저가 계속 털리면 이탈한다.

### 대응

위험 지역을 계층화한다.

### Core Systems
안전도가 높음

### Frontier
중간 위험

### Wild Space
고위험

### War Zone
전면 PvP

### Deep Space
최고 위험 / 최고 보상

---

# 44. 문제 4 — 플레이어가 역사 기록을 조작할 위험

플레이어가 허위 정보를 대량 생산하면 역사 시스템이 망가질 수 있다.

### 대응

**Evidence ≠ Claim**

플레이어가 말한 것은 "주장"이고 실제 증거와 분리한다.

Historical Database:

```text
Claim
Evidence
Source
Confidence
Contradiction
```

구조를 사용한다.

---

# 45. 문제 5 — AI가 역사를 마음대로 만들어버리는 위험

LLM을 사용해 모든 역사 기록을 생성하면 canonical truth가 흔들릴 수 있다.

### 대응

**Simulation State가 진실의 원천(Source of Truth)**

LLM은:

- 뉴스 문장 생성
- NPC 설명
- 요약
- 자연어 검색
- 역사 보고서 표현

에만 사용한다.

실제 사건 발생 여부는 서버 이벤트가 결정한다.

---

# 46. 문제 6 — 10,000 Minerals는 콘텐츠가 아닐 수 있다

숫자가 많다고 재미가 생기지 않는다.

### 대응

10,000개는:

**경제/제작/탐험/전략적 차이를 만드는 데이터**

여야 한다.

MVP에서는 10~30개만 만든다.

---

# 47. 문제 7 — WebGL을 처음부터 핵심 플랫폼으로 잡는 위험

대규모 MMO와 고밀도 3D 전투를 브라우저에서 동시에 처리하는 것은 기술적으로 까다롭다.

### 대응

처음부터 WebGL만 가정하지 않는다.

**PC Standalone을 기준 성능 타깃으로 삼고 WebGL/WebAssembly를 별도 최적화 타깃으로 검증**한다.

---

# 48. 문제 8 — Rust + Unity 이중 생태계의 복잡성

Unity C#과 Rust 양쪽에 핵심 로직이 복제되면 유지보수가 어려워진다.

### 대응

게임의 authoritative simulation을 Rust로 두되,

공통 데이터 정의를:

- Protobuf
- FlatBuffers
- JSON Schema

등으로 표준화한다.

Client는 presentation/prediction, Server는 authority를 담당한다.

---

# 49. 문제 9 — "역사"가 실제 역사학과 너무 멀어질 위험

역사라는 이름만 붙인 Lore 시스템이 되면 차별점이 약해진다.

### 대응

역사학의 실제 개념을 게임 메커니즘에 반영한다.

- Primary Source
- Secondary Source
- Provenance
- Bias
- Contradiction
- Chronology
- Archaeological Context
- Material Evidence
- Oral Tradition
- Historiography

즉, **역사를 읽는 콘텐츠가 아니라 역사 연구의 사고방식을 플레이하게 만든다.**

---

# 50. 최종 Product Positioning

## 장르

**Persistent Historical Space MMO / Multiplayer Sandbox**

## 핵심 판타지

> "내가 우주에서 살아간 기록이 역사책에 남는다."

## 핵심 차별점

### 1
3개 종족만으로 시작하지만 문명과 직업은 깊다.

### 2
1인 우주선이 플레이어의 평생 자산이 된다.

### 3
공격적 행동에 실제 위험과 결과가 따른다.

### 4
전쟁과 경제가 실제 역사 이벤트를 만든다.

### 5
역사학자와 고고학자가 실제 플레이 가능한 직업이다.

### 6
역사 기록은 증거 기반으로 재구성된다.

### 7
플레이어의 죽음, 배신, 발견, 전쟁이 장기적인 세계 기록으로 남는다.

---

# 51. 핵심 게임 루프

```text
EXPLORE
   ↓
DISCOVER
   ↓
EARN
   ↓
UPGRADE
   ↓
RISK
   ↓
CONFLICT
   ↓
CONSEQUENCE
   ↓
HISTORY
   ↓
INVESTIGATE
   ↓
REINTERPRET
   ↓
NEW CONFLICT
   ↓
NEW HISTORY
```

---

# 52. 최종 Vision

STARFALL DYNASTY가 성공하려면 "우주가 넓다"가 핵심이 되어서는 안 된다.

**"우주가 기억한다"가 핵심이어야 한다.**

플레이어는 다음 중 어느 것이든 될 수 있다.

- 은하 최고의 광부
- 거대한 무역상
- 악명 높은 해적
- 현상금 사냥꾼
- 함대 제독
- 외교관
- 정치가
- 기자
- 고고학자
- 역사학자
- 박물관 설립자
- 탐험가
- 밀수업자
- 첩보원

그러나 최종적으로 모든 플레이어에게 하나의 질문이 남는다.

> **"당신이 사라진 뒤, 은하는 당신을 어떻게 기억할 것인가?"**

---

# 53. 개발 우선순위

## 반드시 먼저

1. Historical Event Engine
2. Multiplayer Server Authority
3. One-Person Ship
4. Exploration
5. Risk / PvP
6. Evidence / Chronicle
7. Character Biography

## 이후

8. Economy
9. Profession Expansion
10. Factions
11. Archaeology
12. Politics
13. Fleet Combat
14. Large Galaxy
15. 10,000+ Minerals

### 가장 중요한 원칙

**10,000개 광물보다 하나의 역사적 사건이 재미있어야 한다.**

**1,000개 성계보다 하나의 성계에서 벌어진 전쟁이 기억에 남아야 한다.**

**100개의 직업보다 플레이어가 실제로 자신의 직업을 살아가는 느낌이 있어야 한다.**

---

# 54. North Star Metric

단순 DAU나 플레이타임만 보지 않는다.

핵심 제품 지표:

### Historical Interaction Rate

**플레이어가 다른 플레이어가 만든 역사적 사건을 발견·읽고·조사·개입하는 비율**

추가 지표:

- Player-created Historical Events
- Events with player witnesses
- Historical Investigations
- Evidence discovered
- Cross-player conflicts generated
- Historical News consumption
- Artifact circulation
- Long-term character legacy interactions

---

# 55. 최종 결론

이 프로젝트의 가장 큰 경쟁력은 종족 숫자나 그래픽 규모가 아니다.

**"플레이어 행동을 역사 데이터로 바꾸고, 그 역사를 다시 다음 플레이어의 콘텐츠로 되돌려주는 시스템"**이다.

따라서 개발 초기의 가장 중요한 프로토타입은 화려한 함대전이 아니다.

### 다음 한 장면을 성공시키는 것이 우선이다.

> Player A가 희귀 광물을 발견한다.
>
> Player B가 그 광물을 운송하는 A를 공격한다.
>
> A의 함선이 파괴된다.
>
> B가 광물을 가져간다.
>
> 사건이 Galactic Chronicle에 기록된다.
>
> Player C가 3일 후 사건 현장을 조사한다.
>
> C가 A의 함선 로그를 발견한다.
>
> C가 사건의 진실에 대한 연구를 발표한다.
>
> Player D가 그 연구를 반박한다.
>
> 두 세력이 정치적으로 충돌한다.
>
> 그리고 몇 달 후 이 사건이 새로운 전쟁의 원인이 된다.

**이 순간이 재미있다면 STARFALL DYNASTY는 개발할 가치가 있다.**

**이 순간이 재미없다면 광물 10,000개와 성계 10,000개를 추가해도 해결되지 않는다.**

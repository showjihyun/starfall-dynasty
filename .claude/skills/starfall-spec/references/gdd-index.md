# 기획안 절 색인

`기획안/` 폴더의 3개 문서에서 필요한 절만 찾아 읽기 위한 색인. 줄 번호는 2026-09-17 기준이며 문서가 바뀌면 달라진다. 정확한 위치는 `grep -n "^# {번호}\." "기획안/{파일}"`로 확인한다.

## 목차
1. 문서 개요
2. GDD — Game Design & Tech Roadmap
3. 역사 엔진 — Historical Simulation Engine Design
4. 기술 스택 — Implementation Tech Stack Expert Review
5. 주제별 바로가기

---

## 1. 문서 개요

| 약칭 | 파일 | 줄 수 | 성격 |
|------|------|------|------|
| **GDD** | `STARFALL_DYNASTY_Game_Design_Tech_Roadmap.md` | ~1,640 | 게임 비전, 시스템, MVP, 로드맵, 전문가 리뷰 |
| **HSE** | `STARFALL_DYNASTY_Historical_Simulation_Engine_Design.md` | ~3,300 | 역사 엔진 상세 설계 (데이터 모델, 파이프라인, 테스트) |
| **TECH** | `STARFALL_DYNASTY_Implementation_TechStack_Expert_Review.md` | ~1,470 | 기술 스택 결정, 단계별 아키텍처, 개발 원칙 |

## 2. GDD

| 절 | 제목 | 줄 | 요지 |
|----|------|----|------|
| 0 | Executive Summary | 10 | 행동 → 결과 → 사건 → 기록 → 해석 → 새 갈등 |
| 1 | Design Pillars | 30 | History First, Player Agency, Persistent Consequence, Risk Creates Stories, 3 Species, Systemic Content |
| 2 | Three Species | 58 | Human(기록), Veyr(기억), Kharz(계약·혈통) — 역사관이 다름 |
| 3 | Civilization Design | 145 | 종족 ≠ 국가 |
| 4 | Character System | 170 | Character ≠ Ship, 별도 성장 |
| 5 | Profession System | 198 | 30개 직업, 직업마다 고유 활동 |
| 6 | Starship System | 262 | Scout S-01 스탯, 성장 단계, 모듈, 함선 고유 역사·외관 흔적 |
| 7 | Resource System | 321 | 광물 속성, 결정적 생성(seed), 동적 가격 |
| 8 | Exploration | 361 | 탐험 대상 → Historical Discovery |
| 9 | Warp System | 379 | 워프 4단계, 연출 순서 |
| 10 | Combat | 401 | 1인 → AI 승무원 → 기함 → 함대 |
| 11 | Aggressive Gameplay | 421 | 해적, 현상금 사냥꾼, 밀수, 사략, 첩보, 파괴 공작, 배신 |
| 12 | Wanted & Bounty | 459 | 현상금 예시 |
| 13 | Historical Consequence System | 482 | 행동 → 세계 변화 → 사건 → 증거 → 기록 → 해석 |
| 14 | Galactic Chronicle | 504 | 전투 기록 항목 |
| 15 | Historical Biography | 526 | 자동 전기 |
| 16 | Death & Legacy | 558 | 죽음 = 역사 사건, legacy 연결 |
| 17 | Revenge & Conflict | 580 | 복수는 강제하지 않음 |
| 18 | Historical Evidence System | 604 | 증거 유형, 출처·신뢰성 |
| 19 | Historical Interpretation | 626 | 같은 사건의 복수 해석 |
| 20 | Historian Gameplay | 648 | 역사학자 활동·보상 |
| 21 | Archaeology & Artifact Economy | 675 | 유물 선택지, Lost Artifact Hunt |
| 22 | Information Warfare | 696 | 추상화된 정보전 (현실 해킹 재현 금지) |
| 23 | Galactic Herald | 716 | 자동 뉴스 |
| 24 | Museums & Institutions | 736 | 플레이어 기관 |
| 25 | Historical Simulation Engine | 755 | 인구·경제·정치 연쇄 |
| 26 | Server History Model | 789 | 이벤트 기반 |
| 27–34 | 기술 아키텍처 | 821–1037 | Unity 6, Rust, 백엔드 구조, 데이터 계층, 이벤트 버스, API, 서버 판정 8단계(§33), History 데이터 모델(§34) |
| 35 | Critical Technical Decision | 1039 | 처음부터 거대 MMO 금지 |
| 36 | MVP Scope | 1061 | **MVP-1 "One System, One History"** 범위와 핵심 테스트 |
| 37 | MVP-2 | 1086 | 5~10 성계, 길드, 현상금, 유적, 뉴스 |
| 38 | Vertical Slice | 1103 | 한 플레이어가 역사적 인물이 되는 흐름 |
| 39 | Phase Roadmap | 1143 | Phase 0~5 기간·범위·종료 기준 |
| 40 | Recommended Team | 1243 | 역할 구성 |
| 41–49 | 전문가 비판 리뷰 | 1273–1457 | 범위 폭발, 재미없는 DB, PvP 압살(위험 지역 계층), 기록 조작(Evidence ≠ Claim), AI가 역사 생성, 10,000 광물, WebGL, Rust+Unity 이중 생태계, 실제 역사학 개념 |
| 50 | Product Positioning | 1459 | 장르·판타지·차별점 |
| 51 | 핵심 게임 루프 | 1494 | EXPLORE → … → NEW HISTORY |
| 53 | 개발 우선순위 | 1553 | 반드시 먼저 1~7, 이후 8~15 |
| 54 | North Star Metric | 1586 | Historical Interaction Rate |
| 55 | 최종 결론 | 1609 | **Player A~D "한 장면"** — 최우선 검증 시나리오 |

## 3. HSE

| 절 | 제목 | 줄 | 요지 |
|----|------|----|------|
| 1–3 | 요약·비판·진짜 역할 | 13–165 | 6계층, 책임 분리(Simulation/History/Evidence/Claim/Interpretation/Consequence), LLM 바깥 배치 |
| 4 | 전체 Architecture | 167 | Detector → Evidence/Impact/Biography → Graph → Chronicle/News/Research |
| 5–8 | Data Model, WorldState, DomainEvent | 211–313 | 8개 엔티티, DomainEvent JSON 스키마 |
| 9–12 | Historical Event, 중요도 엔진·단계, 스키마 | 315–479 | Level 0~5, importance 규칙, HistoricalEvent JSON |
| 13 | Event Lifecycle | 481 | 사실 상태와 해석 상태 분리 |
| 14–18 | Evidence, 출처, 변형 | 511–654 | 신뢰성은 범위(scope)가 있음, provenance, 원본 불변 |
| 19–22 | Claim, Interpretation, Truth 분리 | 656–788 | 주어-술어-목적어 구조, FACT ≠ CLAIM ≠ INTERPRETATION |
| 23–25 | Historical Graph, DB 전략, 인과 사슬 | 790–890 | 관계 타입, PostgreSQL event_relations |
| 26–27 | Consequence Engine | 892–947 | 후속 효과도 이벤트로 |
| 28–32 | Event Bus, 멱등성, 순서, 버전, 결정성 | 949–1087 | Outbox, UNIQUE(event_id), tick/sequence, schema_version |
| 33–34 | Rule Engine, Rule Versioning | 1089–1131 | 명시적 Rust 규칙, rule_version 저장 |
| 35–36 | Game Time, Calendar | 1133–1182 | 실제 시간과 게임 시간 분리 |
| 37–42 | Visibility, Fog of History, 지식 상태, 소문, 플레이어 생성 역사 | 1184–1311 | 가시성 ≠ 증거 존재, 소문은 자동으로 사실이 되지 않음 |
| 43–47 | Biography, Ship/Institution History, Projection, CQRS | 1313–1451 | Biography는 프로젝션 |
| 48–50 | History Query API, Research API, Research Case | 1453–1515 | 엔드포인트 예시 |
| 51–55 | 역사 경쟁, 평판, 조작, 위조, 수정 | 1517–1634 | 평판 ≠ 진실, 원본·위조 구분 |
| 57–63 | Tick 구조, 파이프라인, Detector, 상관·집계, ID 분리 | 1672–1841 | Rust 의사코드, Correlation Window |
| 64–68 | Replay, Snapshot, Audit, GM 도구, 관리자 수정 제한 | 1843–1952 | ADMIN_EVENT_CORRECTION |
| 69–72 | 저장 전략, 최소 스키마, 인덱스, 파티셔닝 | 1954–2074 | **최소 PostgreSQL 테이블 목록(§70)** |
| 73–77 | 검색, 자연어 검색, RAG, 뉴스, 편향 | 2076–2196 | 근거 참조 필수 |
| 78–83 | 권한, 악용 방지, 스팸, 압축, 동적 중요도, Retcon 금지 | 2198–2348 | |
| 84–87 | Death & Legacy, Object Graph, History as Content | 2350–2460 | |
| 88–90 | **MVP Scope, MVP Historical Events 10종, MVP Vertical Slice** | 2462–2542 | |
| 91–95 | **테스트 전략, 속성 테스트**, 장애 복구, exactly-once 비판, 일관성 모델 | 2544–2654 | |
| 96–98 | 서비스 경계, Rust 모듈 구조 | 2656–2736 | |
| 99–105 | API/DB/Domain 분리, **Event Envelope**, Correlation vs Causation, 이름 규칙, Narrative Layer, Localization | 2738–2912 | |
| 106–108 | 지표, 전문가 경고 5가지 | 2914–3051 | History Loop Completion Rate |
| 110–111 | **개발 순서 Sprint 1~8, 구현 우선순위 P0~P3** | 3098–3206 | |

## 4. TECH

| 절 | 제목 | 줄 | 요지 |
|----|------|----|------|
| 1 | Executive Decision | 20 | 영역별 최종 권장 표 |
| 2–3 | 위험한 조합, MVP Stack | 49–111 | MVP 스택 vs 나중 도입 |
| 4–6 | Unity 6, ECS 비판(하이브리드), URP | 113–213 | ECS는 병목 우선 적용 |
| 7–8 | Client Networking, WebSocket vs QUIC | 215–292 | Reliable/Realtime 논리 분리, MVP는 WebSocket |
| 9–13 | Rust, Axum, Tokio, Simulation tick, 결정성 | 294–419 | async task ≠ 시뮬레이션 tick |
| 14–17 | PostgreSQL, 이벤트 소싱 비판, 저장할 이벤트, Redis | 421–528 | 하이브리드 이벤트 아키텍처 |
| 18–19 | NATS, **Transactional Outbox** | 530–600 | MVP는 PostgreSQL + Outbox |
| 20–23 | ClickHouse, OpenSearch, S3, AI/LLM | 602–710 | 나중 도입 |
| 24–25 | Historical Engine, Rule Engine 중심 | 712–784 | |
| 26–27 | **Security, Economy Architecture** | 786–832 | 화폐 복사가 최악의 버그 |
| 28–30 | 마이크로서비스·K8s 비판 | 834–909 | 모듈형 모놀리스 |
| 31–33 | Observability, Trace, **Load Testing(k6 + Rust 봇)** | 911–984 | |
| 34 | WebGL 비판 | 986 | PC Primary, Web Secondary |
| 35–37 | 단계별 권장 아키텍처 Phase 1/2/Scale | 1017–1102 | |
| 38–41 | Sharding, History Sharding, Data Ownership | 1104–1217 | |
| 42 | 기술 스택 최종안 Phase 1/2/3 | 1219 | |
| 43 | **가장 중요한 개발 원칙 Rule 1~10** | 1264 | CLAUDE.md 절대 원칙의 출처 |
| 45 | 먼저 만들 Vertical Slice | 1342 | |
| 46 | Keep / Introduce Later / Avoid Initially | 1390 | |

## 5. 주제별 바로가기

| 알고 싶은 것 | 읽을 절 |
|------------|--------|
| MVP에 무엇이 들어가나 | GDD §36, §39 / HSE §88–90 / TECH §3, §42 |
| 최우선 검증 시나리오 | GDD §55 / HSE §90 / TECH §45 |
| 서버 판정 절차 | GDD §33 / TECH §27 |
| 이벤트 envelope·스키마 | HSE §8, §12, §16, §19, §101–103 |
| DB 테이블 | HSE §24, §70–71 |
| 테스트 기준 | HSE §91–92 / TECH §33 |
| 함선·광물 수치 | GDD §6, §7 |
| 렌더링·클라이언트 방향 | TECH §4–8, §34 / GDD §9(워프 연출) |
| 경제 보안 | TECH §26–27 |

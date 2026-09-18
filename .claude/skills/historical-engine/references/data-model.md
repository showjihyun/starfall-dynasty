# 역사 엔진 데이터 모델

기획안 HSE §5–24, §70–71을 구현 가능한 형태로 정리한 기본안이다. 실제 컬럼은 스펙·계약(`contracts/events/historical/`, `contracts/history/`)과 함께 확정하고, 바꿀 때는 이 파일의 원칙(불변, 분리, 출처)을 유지한다.

## 목차
1. 엔티티 관계
2. 테이블 기본안
3. 관계 타입
4. 인덱스
5. MVP에서 미루는 것

---

## 1. 엔티티 관계

```
domain_events ──(source)──▶ historical_events ──▶ historical_event_participants
      │                          │  │                historical_event_objects
      │                          │  └──(relations)──▶ historical_event_relations ─▶ historical_events
      │                          ▼
      └──(자동 생성)──▶ evidence ◀──(derived_from)── evidence
                           ▲
                  claim_evidence (support | counter)
                           │
                        claims ◀── interpretation_claims ── interpretations

프로젝션(재구축 가능): chronicle_entries, biography_entries, ship_history_entries, significance_projection
```

## 2. 테이블 기본안

```sql
-- 원자적 사실. append-only (persistence-patterns §6 트리거)
CREATE TABLE domain_events (
    event_id       UUID PRIMARY KEY,
    event_type     TEXT NOT NULL,
    schema_version INT  NOT NULL,
    world_id       TEXT NOT NULL,
    tick           BIGINT NOT NULL,
    sequence       INT  NOT NULL,
    occurred_at    TEXT NOT NULL,          -- 게임 시간
    recorded_at    TIMESTAMPTZ NOT NULL,   -- 실제 시간
    correlation_id UUID NOT NULL,
    causation_id   UUID,
    actor_id       UUID,
    payload        JSONB NOT NULL,
    UNIQUE (world_id, tick, sequence)
);

-- 역사로 판정된 사건. append-only
CREATE TABLE historical_events (
    historical_event_id UUID PRIMARY KEY,
    event_type          TEXT NOT NULL,     -- MINERAL_DISCOVERED 등 기계적 이름
    schema_version      INT  NOT NULL,
    rule_version        TEXT NOT NULL,     -- 판정 당시 규칙 버전
    importance_level    SMALLINT NOT NULL CHECK (importance_level BETWEEN 0 AND 5),
    occurred_at         TEXT NOT NULL,
    tick                BIGINT NOT NULL,
    location_id         TEXT,
    visibility          TEXT NOT NULL,     -- PUBLIC | FACTION_ONLY | PARTICIPANTS_ONLY | CLASSIFIED | SECRET | DISCOVERABLE
    fact_status         TEXT NOT NULL,     -- CONFIRMED (시뮬레이션 사실). 해석 상태와 별개
    source_event_ids    UUID[] NOT NULL,   -- 근거 도메인 이벤트
    payload             JSONB NOT NULL,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now()
);
-- 같은 도메인 이벤트가 같은 규칙으로 역사 이벤트를 두 번 만들지 않게
CREATE TABLE historical_event_sources (
    source_event_id     UUID NOT NULL,
    detector_rule       TEXT NOT NULL,     -- 어떤 판정 규칙이 만들었나
    historical_event_id UUID NOT NULL REFERENCES historical_events,
    PRIMARY KEY (source_event_id, detector_rule)
);

CREATE TABLE historical_event_participants (
    historical_event_id UUID NOT NULL REFERENCES historical_events,
    entity_id           UUID NOT NULL,
    entity_kind         TEXT NOT NULL,     -- PLAYER | SHIP | FACTION | NPC | INSTITUTION
    role                TEXT NOT NULL,     -- discoverer | attacker | victim | witness ...
    PRIMARY KEY (historical_event_id, entity_id, role)
);

CREATE TABLE historical_event_objects (
    historical_event_id UUID NOT NULL REFERENCES historical_events,
    object_id           TEXT NOT NULL,     -- mineral_x, artifact id, system id
    object_kind         TEXT NOT NULL,
    PRIMARY KEY (historical_event_id, object_id)
);

-- 인과·파생 그래프. 관계 추가만 한다 (잘못된 관계는 retraction 관계를 추가)
CREATE TABLE historical_event_relations (
    from_id       UUID NOT NULL,
    relation_type TEXT NOT NULL,
    to_id         UUID NOT NULL,
    rule_version  TEXT,                    -- 자동 연결이면 규칙 버전, 플레이어 주장이면 NULL
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (from_id, relation_type, to_id)
);

-- 증거. append-only
CREATE TABLE evidence (
    evidence_id               UUID PRIMARY KEY,
    evidence_type             TEXT NOT NULL,  -- SHIP_LOG | SCANNER_RECORD | MILITARY_REPORT | PLAYER_TESTIMONY | NEWS_ARTICLE | PERSONAL_DIARY | ARTIFACT | VEYR_MEMORY | KHARZ_CONTRACT ...
    source_entity_id          UUID,
    creation_method           TEXT NOT NULL,  -- automatic | player_authored | copied | edited | forged | declassified
    derived_from_evidence_ids UUID[] NOT NULL DEFAULT '{}',
    authenticity_status       TEXT NOT NULL,  -- VERIFIED | UNVERIFIED | FORGED (시스템 내부 진실)
    reliability_scope         JSONB NOT NULL DEFAULT '{}', -- {"location": "high", "political_responsibility": "low"}
    related_event_ids         UUID[] NOT NULL DEFAULT '{}',
    visibility                TEXT NOT NULL,
    content_ref               TEXT,           -- 큰 본문은 오브젝트 스토리지 참조
    created_at_game           TEXT NOT NULL,
    created_at                TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- 주장. 수정 대신 철회·대체 레코드
CREATE TABLE claims (
    claim_id        UUID PRIMARY KEY,
    author_id       UUID NOT NULL,
    subject_id      TEXT NOT NULL,
    predicate       TEXT NOT NULL,          -- STARTED_WAR | DESTROYED | BETRAYED ...
    object_id       TEXT,
    qualifiers      JSONB NOT NULL DEFAULT '{}', -- {"game_time": "...", "location_id": "..."}
    statement_text  TEXT,                   -- 자연어 표현 (선택)
    about_event_id  UUID,
    supersedes_claim_id UUID,               -- 같은 저자의 수정판
    created_at_game TEXT NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE claim_evidence (
    claim_id    UUID NOT NULL REFERENCES claims,
    evidence_id UUID NOT NULL REFERENCES evidence,
    stance      TEXT NOT NULL CHECK (stance IN ('support', 'counter')),
    PRIMARY KEY (claim_id, evidence_id, stance)
);

-- (MVP 이후) 해석과 상태 변화 기록
CREATE TABLE interpretations (
    interpretation_id UUID PRIMARY KEY,
    author_id         UUID NOT NULL,
    about_event_id    UUID NOT NULL,
    thesis_text       TEXT NOT NULL,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE TABLE interpretation_status_changes (
    interpretation_id UUID NOT NULL REFERENCES interpretations,
    status            TEXT NOT NULL,        -- PROPOSED | SUPPORTED | DISPUTED
    cause_evidence_id UUID,
    changed_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (interpretation_id, changed_at)
);

-- 프로젝션 예: Chronicle (재구축 가능, 수정 가능)
CREATE TABLE chronicle_entries (
    historical_event_id UUID PRIMARY KEY,
    world_id            TEXT NOT NULL,
    occurred_at         TEXT NOT NULL,
    tick                BIGINT NOT NULL,
    importance_level    SMALLINT NOT NULL,  -- 현재 중요도 (동적 갱신 가능)
    visibility          TEXT NOT NULL,
    headline_key        TEXT NOT NULL,      -- 로컬라이즈 키 + 파라미터. 문장 자체를 저장하지 않음
    headline_params     JSONB NOT NULL,
    location_id         TEXT
);
```

설계 메모:
- 문장을 DB에 저장하지 않고 `headline_key + params`로 두는 이유: 언어 중립 저장(HSE §105)과 서사 계층 분리. LLM 요약을 쓰더라도 결과는 별도 캐시이고 원본이 아니다.
- `fact_status`와 해석 상태를 한 컬럼으로 합치지 않는다 (HSE §13).
- `rule_version`은 `"detector@3"`처럼 규칙 묶음 이름과 버전을 함께 쓴다.

## 3. 관계 타입

`CAUSED_BY`, `CAUSED`, `SUPPORTED_BY`, `CONTRADICTED_BY`, `DERIVED_FROM`, `PARTICIPATED_IN`, `OCCURRED_AT`, `OWNED_BY`, `DISCOVERED_BY`, `FOLLOWED_BY`, `AGGREGATES`, `RETRACTS`

자동 인과 연결은 명시적 규칙으로만 만든다. 모든 사건을 자동으로 인과 연결하면 연쇄 폭발이 생긴다 (HSE §108-2).

## 4. 인덱스

```sql
CREATE INDEX ON historical_events (event_type);
CREATE INDEX ON historical_events (occurred_at);
CREATE INDEX ON historical_events (tick);
CREATE INDEX ON historical_events (location_id);
CREATE INDEX ON historical_event_participants (entity_id);
CREATE INDEX ON historical_event_objects (object_id);
CREATE INDEX ON historical_event_relations (to_id, relation_type);
CREATE INDEX ON claim_evidence (evidence_id);
CREATE INDEX ON claims (about_event_id);
CREATE INDEX ON evidence USING GIN (related_event_ids);
```

## 5. MVP에서 미루는 것

Interpretation 경쟁, 위조 게임플레이, 소문, 연구 케이스, 박물관, 학술 경쟁, 고급 인과, 시간 파티셔닝, 그래프 DB, OpenSearch 색인 (HSE §88, §111). 테이블 기본안에 자리만 있고 구현은 해당 Phase에서 한다.

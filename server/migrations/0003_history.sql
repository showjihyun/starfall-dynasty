-- p1-02-mining / ADR-0014 — 역사 엔진 표(H2): historical_events · historical_event_sources ·
-- evidence · history_cursor.
--
-- ⚠ 이 파일을 적용한 뒤 한 글자라도 고치면 `_sqlx_migrations` 체크섬이 어긋나 기동이
--   실패한다. 고쳐야 하면 새 마이그레이션 파일을 추가한다(CLAUDE.md — 마이그레이션은
--   추가만 한다).
--
-- 범위는 ADR-0014 §3 이 정한 것만: participants/objects 정규화 테이블·claims·
-- interpretations·relations·chronicle_entries 는 미룬다("생산자가 없는 빈 테이블은 아무
-- 것도 증명하지 않는다").

-- 역사 이벤트(Historical Event) — 판정을 통과한 기록. 자연어 서사 열이 없다(원칙 2·6,
-- ADR-0014 §3) — 헤드라인·문장은 표현 계층이 event_type + payload 로 만든다.
--
-- `historical_event_id` 는 UUIDv5(NS_HISTORY, "{world_id}|{event_type}|{dedupe_key}") 로
-- 결정적이다(H1) — 그래서 PK 자체가 "의미 유일성"(ADR-0014 §4의 3)의 절반을 진다.
-- `dedupe_key` 에는 `rule_version` 이 들어가지 않는다 — 규칙이 @2 로 올라도 같은 사실을
-- "다시 발견"하지 않게.
CREATE TABLE historical_events (
    historical_event_id UUID        PRIMARY KEY,
    world_id             UUID        NOT NULL REFERENCES worlds (world_id),
    event_type           TEXT        NOT NULL,
    dedupe_key            TEXT        NOT NULL,
    rule_version          TEXT        NOT NULL,
    importance_level      INT         NOT NULL CHECK (importance_level BETWEEN 1 AND 5),
    tick                  BIGINT      NOT NULL CHECK (tick BETWEEN 0 AND 9007199254740991),
    occurred_at           TEXT        NOT NULL,
    -- 이 기록을 커밋한 실제 시각. 감사 전용 — 결정성 비교(재생·재처리)에서 제외한다
    -- (ADR-0014 §5).
    recorded_at           TIMESTAMPTZ NOT NULL,
    visibility            TEXT        NOT NULL,
    -- p1-02 는 CONFIRMED 하나뿐이다. 값을 늘리는 것 자체가 마이그레이션 + ADR 검토
    -- 대상이다(ADR-0014 §3).
    fact_status           TEXT        NOT NULL CHECK (fact_status = 'CONFIRMED'),
    -- 판정 근거가 된 커밋된 도메인 이벤트. 이 규칙은 언제나 정확히 1개다(H1).
    source_event_ids      UUID[]      NOT NULL CHECK (array_length(source_event_ids, 1) >= 1),
    -- HistoricalLocation·Vec<HistoricalParticipant>·타입별 payload — 계약 모양 그대로.
    location              JSONB       NOT NULL,
    participants          JSONB       NOT NULL,
    payload               JSONB       NOT NULL,
    -- 의미 유일성의 DB 쪽 절반(ADR-0014 §4의 3) — 서로 다른 두 채굴이 각각 "최초 발견"으로
    -- 판정되는 판정 상태 재적재 버그도 이 제약이 막는다.
    UNIQUE (world_id, event_type, dedupe_key)
);

CREATE INDEX historical_events_world_tick_idx ON historical_events (world_id, tick);

-- 근거 유일성(ADR-0014 §4의 2) — 같은 근거·같은 규칙에서 두 번째 행은 못 들어간다.
-- `detector_rule` 은 `rule_id`(버전 없이) — 규칙이 @2 로 올라도 같은 근거를 "다시" 근거로
-- 쓰지 않는다는 것을 이 표도 지킨다.
CREATE TABLE historical_event_sources (
    -- FK(architect 권고, 2026-09-28 수락) — I-62("source_event_ids 는 domain_events 에
    -- 존재하는 이벤트다")를 DB 구조로 강제한다. 지금까지는 SC-51 이 매 실행마다 다시
    -- 확인하는 관측이었다.
    source_event_id      UUID NOT NULL REFERENCES domain_events (event_id),
    detector_rule         TEXT NOT NULL,
    historical_event_id   UUID NOT NULL REFERENCES historical_events (historical_event_id),
    PRIMARY KEY (source_event_id, detector_rule)
);

CREATE INDEX historical_event_sources_event_idx
    ON historical_event_sources (historical_event_id);

-- 발견 1건마다 시스템이 자동으로 만드는 증거(HSE §90). `authenticity_status`·
-- `creation_method`·`derived_from_evidence_ids` 는 이번 규칙에서 전부 고정값이다 — 원본
-- 증거의 표지는 첫 행이 들어간 뒤에는 되돌릴 수 없는 모양이라 CHECK 으로 지금 고정한다
-- (history 검토 §11 SC-51).
CREATE TABLE evidence (
    evidence_id                UUID   PRIMARY KEY,
    historical_event_id        UUID   NOT NULL REFERENCES historical_events (historical_event_id),
    -- ADR-0014 §5: "모든 역사 기록(과 증거)은 rule_version 을 가진다"(architect 지적,
    -- 2026-09-28 — 첫 버전에 이 열이 빠져 있었다). 러너가 그 발견을 낸 규칙의
    -- rule_version 을 그대로 채운다 — historical_events.rule_version 과 항상 같다
    -- (같은 트랜잭션, 같은 record 에서 나온다).
    rule_version                 TEXT   NOT NULL,
    evidence_type               TEXT   NOT NULL,
    visibility                  TEXT   NOT NULL,
    source_entity_id             UUID   NOT NULL,
    authenticity_status          TEXT   NOT NULL DEFAULT 'VERIFIED' CHECK (authenticity_status = 'VERIFIED'),
    creation_method               TEXT   NOT NULL DEFAULT 'automatic' CHECK (creation_method = 'automatic'),
    derived_from_evidence_ids     UUID[] NOT NULL DEFAULT '{}' CHECK (derived_from_evidence_ids = '{}')
);

CREATE INDEX evidence_historical_event_idx ON evidence (historical_event_id);

-- 진행 위치 — **기록이 아니라 가변 상태다.** 역사 4표 중 트리거를 안 거는 유일한 표
-- (ADR-0014 §3). `consumer` 는 이 규칙 하나뿐이지만(`mineral-discovery`), 나중에 규칙이
-- 늘면 소비자별로 독립 커서가 필요해질 것을 대비해 첫날부터 둔다.
CREATE TABLE history_cursor (
    world_id UUID    NOT NULL REFERENCES worlds (world_id),
    consumer TEXT    NOT NULL,
    tick     BIGINT  NOT NULL CHECK (tick BETWEEN 0 AND 9007199254740991),
    sequence BIGINT  NOT NULL CHECK (sequence BETWEEN 0 AND 9007199254740991),
    PRIMARY KEY (world_id, consumer)
);

-- 추가 전용(원칙 5, I-20) — 세 표(historical_events·historical_event_sources·evidence)에
-- 건다. `history_cursor` 에는 걸지 않는다(위에서 이미 그 표만 가변이라고 적었다).
--
-- 0001 의 `forbid_mutation()` 과 별도 함수를 쓴다 — 그 함수는 메시지에 'domain_events' 를
-- 문자 그대로 박아 뒀다(사실이 아닌 표 이름을 보고할 것이다). 여기서는 `TG_TABLE_NAME` 으로
-- **실제로 거부된 표 이름**을 메시지에 넣는다(history 검토 §11 SC-52 — "거부가 트리거
-- 때문인가"를 SQLSTATE 뿐 아니라 표 이름으로도 확인할 수 있게).
CREATE FUNCTION forbid_history_mutation() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'history table % is append-only (% blocked). 정정은 새 레코드로 한다.',
        TG_TABLE_NAME, TG_OP;
END;
$$;

CREATE TRIGGER historical_events_append_only
    BEFORE UPDATE OR DELETE ON historical_events
    FOR EACH ROW EXECUTE FUNCTION forbid_history_mutation();

CREATE TRIGGER historical_event_sources_append_only
    BEFORE UPDATE OR DELETE ON historical_event_sources
    FOR EACH ROW EXECUTE FUNCTION forbid_history_mutation();

CREATE TRIGGER evidence_append_only
    BEFORE UPDATE OR DELETE ON evidence
    FOR EACH ROW EXECUTE FUNCTION forbid_history_mutation();

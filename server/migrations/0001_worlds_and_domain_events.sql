-- p0-02-networking-spike / ADR-0007 §2 — 프로젝트의 첫 마이그레이션.
--
-- 첫 테이블은 나중에 만드는 어떤 테이블보다 비싸다: 이후 모든 역사·증거·프로젝션이
-- 여기에 붙고, 이미 들어간 행은 원칙 5 에 따라 고칠 수 없다.
--
-- ⚠ 이 파일을 한 글자라도 고치면 `_sqlx_migrations` 의 체크섬과 어긋나 **기동이 실패한다**
--   ("previously applied but has been modified"). 고친 뒤에는 레포 루트에서
--   `docker compose down -v && docker compose up -d` 를 해야 한다.
--   이 명령은 `name: starfall` 덕분에 starfall 프로젝트에만 작용한다.

CREATE TABLE worlds (
    world_id       UUID PRIMARY KEY,
    name           TEXT NOT NULL,
    tick_hz        INT  NOT NULL CHECK (tick_hz BETWEEN 1 AND 1000),
    calendar_epoch TEXT NOT NULL,
    calendar_scale INT  NOT NULL CHECK (calendar_scale > 0),
    sim_version    INT  NOT NULL,
    -- 이 월드가 도달한 마지막 tick. 재기동 시 여기서 이어 간다 (ADR-0006 §2.3).
    -- 이벤트를 쓰는 트랜잭션 안에서 함께 갱신되므로 domain_events 와 어긋나지 않는다.
    last_tick      BIGINT CHECK (last_tick BETWEEN 0 AND 9007199254740991),
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE domain_events (
    event_id       UUID PRIMARY KEY,
    event_type     TEXT   NOT NULL,
    schema_version INT    NOT NULL CHECK (schema_version >= 1),
    world_id       UUID   NOT NULL REFERENCES worlds (world_id),
    tick           BIGINT NOT NULL CHECK (tick BETWEEN 0 AND 9007199254740991),
    sequence       BIGINT NOT NULL CHECK (sequence BETWEEN 0 AND 9007199254740991),
    occurred_at    TEXT   NOT NULL,
    recorded_at    TIMESTAMPTZ NOT NULL,
    correlation_id UUID   NOT NULL,
    causation_id   UUID,
    actor_id       UUID,
    payload        JSONB  NOT NULL,
    UNIQUE (world_id, tick, sequence)
);

CREATE INDEX domain_events_type_idx        ON domain_events (event_type, world_id, tick);
CREATE INDEX domain_events_correlation_idx ON domain_events (correlation_id);

-- append-only (원칙 5, I-20).
--
-- 알려진 구멍 둘, 둘 다 의도다:
--   1. TRUNCATE 는 행 단위 트리거로 막히지 않는다. 스파이크 DB 는 버릴 수 있어야 한다.
--   2. 슈퍼유저는 트리거를 DROP 할 수 있다. 이것은 실수를 막는 장치이지 악의를 막는
--      장치가 아니다.
CREATE FUNCTION forbid_mutation() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'domain_events is append-only (% blocked). 정정은 새 레코드로 한다.', TG_OP;
END;
$$;

CREATE TRIGGER domain_events_append_only
    BEFORE UPDATE OR DELETE ON domain_events
    FOR EACH ROW EXECUTE FUNCTION forbid_mutation();

-- 스파이크 월드. 값이 고정이라 QA 가 기대값으로 쓸 수 있고 계약 fixture 의 world_id 와 같다.
INSERT INTO worlds (world_id, name, tick_hz, calendar_epoch, calendar_scale, sim_version, last_tick)
VALUES (
    '01a0b1c2-3d4e-7f01-8a2b-9c0d1e2f3a4b',
    'spike',
    20,
    '3800-01-01T00:00:00Z',
    60,
    1,
    NULL
);

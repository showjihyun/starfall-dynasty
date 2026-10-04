-- p1-02-mining / ADR-0013 — 경제 상태(인벤토리·매장지·처리 장부).
--
-- ⚠ 이 파일을 적용한 뒤 한 글자라도 고치면 `_sqlx_migrations` 체크섬이 어긋나 기동이
--   실패한다. 고쳐야 하면 새 마이그레이션 파일을 추가한다(CLAUDE.md — 마이그레이션은
--   추가만 한다).
--
-- 제약 집합은 ADR-0013 §5a-1 표 그대로다(architect 승인, `02_server_ack.md` §5 3차 요청
-- 완료 — server 검토 K1~K6 반영):
--   inventory_items      7 (PK 1, FK 1, NOT NULL 4, CHECK 1)
--   deposit_states       10(PK 1, FK 1, NOT NULL 5, CHECK 3)
--   processed_commands   8 (PK 1, FK 1, NOT NULL 5, CHECK 1)
--   domain_events        +1 (CHECK payload는 object)
-- 완료 증거는 `pg_constraint` 실측이 이 표와 같음(qa 몫).

-- 행위자별 인벤토리 — 광물별 보유량. 인벤토리는 함선이 아니라 행위자 소유다
-- (ADR-0013 §1). 0kg 행은 만들지 않는다(계약 불변식 — server 가 지킨다, DB 제약이
-- 아니다: 0 자체는 유효한 값이어야 "채굴 직후 인벤토리가 0에서 시작"하는 비교 후
-- 쓰기의 `expected: Some(0)` 표현이 가능하다).
CREATE TABLE inventory_items (
    world_id    UUID   NOT NULL REFERENCES worlds (world_id),
    actor_id    UUID   NOT NULL,
    mineral_id  TEXT   NOT NULL,
    quantity_kg BIGINT NOT NULL CHECK (quantity_kg BETWEEN 0 AND 2147483647),
    PRIMARY KEY (world_id, actor_id, mineral_id)
);

-- 매장지 채굴 이력. 행은 **첫 채굴 때만** 생긴다(I-68 "드러남") — 그래서
-- first_extracted_tick 이 NOT NULL 이어도 성립한다(행이 있다는 것 자체가 드러났다는
-- 뜻이므로).
CREATE TABLE deposit_states (
    world_id             UUID   NOT NULL REFERENCES worlds (world_id),
    deposit_id           TEXT   NOT NULL,
    remaining_kg         BIGINT NOT NULL CHECK (remaining_kg BETWEEN 0 AND 2147483647),
    as_of_tick           BIGINT NOT NULL CHECK (as_of_tick BETWEEN 0 AND 9007199254740991),
    first_extracted_tick BIGINT NOT NULL CHECK (first_extracted_tick BETWEEN 0 AND 9007199254740991),
    PRIMARY KEY (world_id, deposit_id),
    CHECK (first_extracted_tick <= as_of_tick)
);

-- 상태 변경 명령의 지속 멱등 장부(K1, ADR-0013 §3) — **월드 범위**다(actor 범위가
-- 아니다). `causation_id ↔ command_id` 1:1 조인(AC-18(b), history 오라클)이 이 범위
-- 선택의 이유다: actor 범위였다면 한 사용자가 actor 둘로 같은 command_id 를 써서
-- 월드를 멈출 수 있었다(server 검토 K1 DoS 경로).
CREATE TABLE processed_commands (
    world_id     UUID   NOT NULL REFERENCES worlds (world_id),
    command_id   UUID   NOT NULL,
    actor_id     UUID   NOT NULL,
    command_type TEXT   NOT NULL,
    tick         BIGINT NOT NULL CHECK (tick BETWEEN 0 AND 9007199254740991),
    PRIMARY KEY (world_id, command_id)
);

CREATE INDEX processed_commands_world_actor_idx ON processed_commands (world_id, actor_id);

-- domain_events.payload 는 항상 JSON object 다 — 직렬화 실패가 `JSONB null` 로 조용히
-- 저장되던 결함(persistence 2.2 확인)을 막는 마지막 방어선. 적용 전 실측: 기존 4,805행
-- 전부 object(architect 확인, `02_server_ack.md` §5).
ALTER TABLE domain_events
    ADD CONSTRAINT domain_events_payload_is_object CHECK (jsonb_typeof(payload) = 'object');

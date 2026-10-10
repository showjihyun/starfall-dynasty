-- p1-03-trading / ADR-0016 — 지갑과 시장 상태.
--
-- ⚠ 이 파일을 적용한 뒤 한 글자라도 고치면 `_sqlx_migrations` 체크섬이 어긋나 기동이
--   실패한다. 고쳐야 하면 새 마이그레이션 파일을 추가한다(CLAUDE.md — 마이그레이션은
--   추가만 한다). 0001~0003 은 동결(ADR-0007 §5) — 이 파일이 **그 뒤 유일한 추가**다.
--
-- 제약 집합은 ADR-0016 §5 표 그대로다(architect 승인 대기 — T0 server 검토 뒤
-- accepted):
--   wallets        6 (PK 1, FK 1, NOT NULL 3, CHECK 1)
--   market_states  12(PK 1, FK 1, NOT NULL 7, CHECK 3)
-- 경제 54(0001·0002) + 6 + 12 = 72 제약(월드 정지 경로 분모). 완료 증거는
-- `pg_constraint` 실측이 이 표와 같음(qa 몫, `constraint_census.py`).

-- 행위자별 지갑 — actor 소유 상태 행(ADR-0016 §2, 인벤토리와 같은 이유: 함선은
-- 영속하지 않는다). 행은 **첫 판매에 생긴다** — 행이 없는 actor 의 잔액은 0 이다
-- (인벤토리의 "행 없음 = 0kg" 과 같은 규약). 행을 지우지 않는다. `actor_id` 에는
-- FK 가 없다(인벤토리와 같은 이유 — actor 테이블이 없다, 인증에서 온다).
--
-- 상한만 CHECK 한다(이번 슬라이스에 크레딧을 빼는 경로가 없다 — 하한 0 의 ②는
-- 크레딧 소비처가 생기는 슬라이스의 만기, `01_architect_tasks.md` 만기 표).
CREATE TABLE wallets (
    world_id   UUID   NOT NULL REFERENCES worlds (world_id),
    actor_id   UUID   NOT NULL,
    balance_cr BIGINT NOT NULL CHECK (balance_cr BETWEEN 0 AND 9007199254740991),
    PRIMARY KEY (world_id, actor_id)
);

-- 시장의 광물별 포화 상태. 행은 **그 광물의 첫 판매에 생긴다**(행 없음 = 포화 0,
-- 마지막 거래 없음 — ADR-0016 §4). 가격은 저장하지 않는다(포화와 `data/` 에서
-- 유도되는 값 — 저장하면 두 값이 갈라질 자리가 생긴다). 회복은 DB 쓰기를 만들지
-- 않는다(게으른 닫힌 식, sim 에만 있다 — K3: 영속화는 저장 표현 그대로의 동등
-- 비교만 한다). `market_id`·`mineral_id` 에는 FK 가 없다(판정 2단계
-- `TARGET_UNKNOWN` 을 통과한 `data/` 표의 id — DB 테이블이 아니다).
--
-- `last_seller_actor_id`·`last_sale_quantity_kg` 는 시장의 공개 게시판(판 사람이
-- 떠난 뒤에도 남는 흔적) — CAS 비교 대상이 아니라 매 판매마다 그냥 덮어쓰는 "새
-- 값"이다(매장지의 `first_extracted_tick` 과 다른 위치: 저 값은 불변이고 이 값은
-- 매번 갈아 끼운다).
CREATE TABLE market_states (
    world_id              UUID   NOT NULL REFERENCES worlds (world_id),
    market_id             TEXT   NOT NULL,
    mineral_id            TEXT   NOT NULL,
    saturation_kg         BIGINT NOT NULL CHECK (saturation_kg BETWEEN 0 AND 2147483647),
    as_of_tick            BIGINT NOT NULL CHECK (as_of_tick BETWEEN 0 AND 9007199254740991),
    last_seller_actor_id  UUID   NOT NULL,
    last_sale_quantity_kg BIGINT NOT NULL CHECK (last_sale_quantity_kg BETWEEN 1 AND 2147483647),
    PRIMARY KEY (world_id, market_id, mineral_id)
);

# 영속화 패턴 — Outbox, 멱등 명령, 화폐 원장

SQL은 PostgreSQL 기준 예시다. 실제 컬럼·타입은 스펙과 계약에 맞춰 조정하되, 각 패턴이 막는 문제(주석)는 유지한다.

## 목차
1. 멱등 명령 처리
2. 화폐 원장과 잔액
3. 인벤토리 변경과 잠금
4. Transactional Outbox
5. 멱등 소비자
6. 과거 기록 불변 강제

---

## 1. 멱등 명령 처리

막는 문제: 네트워크 재전송이나 악의적 재생으로 같은 거래·채굴이 두 번 적용되는 것.

```sql
CREATE TABLE processed_commands (
    command_id   UUID PRIMARY KEY,
    player_id    UUID NOT NULL,
    command_type TEXT NOT NULL,
    result       JSONB NOT NULL,          -- 재전송 시 같은 결과를 돌려주기 위해 저장
    processed_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

처리 트랜잭션의 **첫 문장**으로 삽입한다.

```sql
INSERT INTO processed_commands (command_id, player_id, command_type, result)
VALUES ($1, $2, $3, '{}'::jsonb)
ON CONFLICT (command_id) DO NOTHING
RETURNING command_id;
```

행이 반환되지 않으면 이미 처리된 명령이다. 저장된 `result`를 그대로 응답하고 상태를 바꾸지 않는다. 처리 성공 시 같은 트랜잭션에서 `result`를 갱신한다.

## 2. 화폐 원장과 잔액

막는 문제: 잔액만 두면 복사 버그가 생겨도 원인을 추적할 수 없다. 원장이 있으면 "잔액 = 원장 합계" 불변식으로 탐지할 수 있다.

```sql
CREATE TABLE wallets (
    owner_id UUID PRIMARY KEY,
    balance  BIGINT NOT NULL CHECK (balance >= 0),
    version  BIGINT NOT NULL DEFAULT 0
);

CREATE TABLE ledger_entries (
    entry_id       BIGSERIAL PRIMARY KEY,
    transfer_id    UUID NOT NULL,         -- 한 이체의 차변·대변을 묶음
    owner_id       UUID NOT NULL,
    amount         BIGINT NOT NULL,       -- 양수 입금, 음수 출금
    reason         TEXT NOT NULL,         -- TRADE, MINING_SALE, BOUNTY_PAYOUT, SYSTEM_FAUCET, SYSTEM_SINK
    correlation_id UUID NOT NULL,
    command_id     UUID,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX ON ledger_entries (owner_id, entry_id);
```

- 플레이어 간 이체는 한 `transfer_id`의 항목 합이 0이어야 한다. 화폐 발행(퀘스트 보상, NPC 매입)과 소각은 `SYSTEM_FAUCET`/`SYSTEM_SINK` 계정을 상대로 기록해 총량 변화를 추적한다.
- 속성 테스트와 운영 점검: `SUM(ledger)`와 `SUM(wallets.balance)`가 계정별로 같아야 한다.

잔액 차감은 조건부 UPDATE 한 문장으로 한다 (확인과 차감 사이에 다른 트랜잭션이 끼어들 틈을 없앤다).

```sql
UPDATE wallets SET balance = balance - $2, version = version + 1
WHERE owner_id = $1 AND balance >= $2
RETURNING balance;
```

행이 없으면 잔액 부족으로 거부한다.

## 3. 인벤토리 변경과 잠금

- 여러 행을 잠글 때는 **항상 같은 순서**(예: owner_id 오름차순)로 `SELECT ... FOR UPDATE` 해서 교착 상태를 피한다.
- 거래 두 당사자의 인벤토리·지갑을 한 트랜잭션에서 바꾼다. 한쪽만 커밋되는 경로가 있으면 복사나 소실이 생긴다.
- 스택 수량은 `CHECK (quantity >= 0)`.

## 4. Transactional Outbox

막는 문제: "DB에는 저장됐는데 이벤트는 발행 안 됨"(또는 반대). 상태 변경과 발행 예정 기록을 한 트랜잭션에 넣는다 (기획안 TECH §19).

```sql
CREATE TABLE outbox_events (
    id           BIGSERIAL PRIMARY KEY,   -- 발행 순서
    event_id     UUID NOT NULL UNIQUE,
    event_type   TEXT NOT NULL,
    envelope     JSONB NOT NULL,          -- contracts/common/event-envelope 형식 전체
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    published_at TIMESTAMPTZ
);
CREATE INDEX outbox_unpublished ON outbox_events (id) WHERE published_at IS NULL;
```

워커 루프 (MVP는 같은 프로세스 안의 소비자에게 전달, 규모가 커지면 NATS로 교체):

```sql
BEGIN;
SELECT id, event_id, event_type, envelope
FROM outbox_events
WHERE published_at IS NULL
ORDER BY id
LIMIT $1
FOR UPDATE SKIP LOCKED;
-- 소비자에게 전달 (각 소비자는 멱등)
UPDATE outbox_events SET published_at = now() WHERE id = ANY($2);
COMMIT;
```

- 전달 도중 워커가 죽으면 커밋되지 않았으므로 다시 전달된다. 즉 **최소 1회 전달**이고, 정확히 1회 효과는 소비자의 멱등성으로 만든다 (기획안 HSE §94).
- 발행 완료 행은 보존 기간 후 아카이브한다. 원본 이벤트는 `domain_events`에 남아 있다.

## 5. 멱등 소비자

```sql
CREATE TABLE processed_events (
    consumer     TEXT NOT NULL,           -- 'history.detector', 'projection.chronicle', 'news'
    event_id     UUID NOT NULL,
    processed_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (consumer, event_id)
);
```

소비자 트랜잭션: `INSERT INTO processed_events ... ON CONFLICT DO NOTHING RETURNING` → 행이 없으면 건너뜀 → 있으면 처리 결과를 같은 트랜잭션에 기록.

## 6. 과거 기록 불변 강제

역사·도메인 이벤트 테이블은 애플리케이션 규약만 믿지 않고 DB에서 막는다.

```sql
CREATE FUNCTION forbid_mutation() RETURNS trigger AS $$
BEGIN
  RAISE EXCEPTION 'table % is append-only', TG_TABLE_NAME;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER domain_events_append_only
BEFORE UPDATE OR DELETE ON domain_events
FOR EACH ROW EXECUTE FUNCTION forbid_mutation();
```

운영자의 정정은 `ADMIN_EVENT_CORRECTION` 같은 새 이벤트로 남긴다 (기획안 HSE §68).

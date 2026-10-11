# 0016. 크레딧과 광물 판매 — ADR-0013 의 장부를 그대로 쓰고, 지갑과 시장을 상태 행으로 더한다

- 상태: **accepted** (2026-10-05 — 가격 모델 designer3 확정(구간형 포화 + 회복, §4). server3·serverdb3 T0 검토 **동의, 수정 요청 없음**: §1~§7 전부 p1-02 의 `StateWrite`·compare-and-set 패턴의 반복으로 구현 가능함을 코드로 확인, 0004 제약 이름 18 개가 §5 와 일치(`_workspace/p1-03-trading/02_server_ack.md`, `02_interface.md`))
- 날짜: 2026-10-05
- 슬라이스: p1-03-trading
- 근거 기획안 절: TECH §26–27(화폐 복사가 최악의 버그, "Trade Request → Server Validation → DB Transaction → Inventory Update → Economy Event"), GDD §7.3(가격은 고정값이 아니다), GDD §33(서버 판정 8단계), GDD §36(MVP-1 "거래" — "시장" 은 MVP-2 §37)
- 관련: **ADR-0013**(전부 — 이 ADR 은 그 결정을 재사용하고 확장만 한다), ADR-0006 §4a(메시지 id 흐름), ADR-0007 §1(기록 범위 표), ADR-0007 §5·ADR-0013 §5a-1b(마이그레이션 0001~0003 동결), ADR-0015(데이터 배포)

## 맥락

p1-03 은 **두 번째 상태 변경 명령**이자 **첫 화폐**다. 판매 한 건은 세 상태를 동시에 바꾼다: 인벤토리(감소 — 처음으로 줄어든다), 지갑(증가 — 처음 생긴다), 그리고 시장(포화 — 가격을 움직이는 상태, 디자인 §3).

리더가 물은 것은 하나다: **p1-02 의 처리 장부·ADR-0013 패턴을 재사용할지 새로 둘지.** 재사용할 수 없는 이유가 있는지부터 본다.

- ADR-0013 §2 "tick 배치 트랜잭션이 곧 outbox" 는 명령 종류를 가정하지 않는다. 배치가 싣는 것은 "그 tick 이 바꾼 상태 행 + 그 tick 의 이벤트 + 수락된 상태 변경 명령의 장부 행 + `last_tick`" 이다.
- ADR-0013 §3 의 `processed_commands` 는 PK `(world_id, command_id)` 에 `command_type` 열이 있다 — 명령 종류가 늘어날 것을 이미 담고 있다.
- ADR-0013 §5 의 비교 후 쓰기는 "행마다 {기대값, 새 값}" 이고 행의 종류를 가정하지 않는다.
- ADR-0013 "버린 대안" 에 이미 이렇게 적혀 있다: *"`domain_events` 로 멱등 — 명령 하나가 이벤트 0개·여러 개를 내는 순간(거래) 성립하지 않는다."* — 거래가 올 것을 보고 고른 구조다.

재사용을 막는 것은 없다. **새 장부를 두면** 같은 사실(이 명령이 세계를 바꿨다)을 두 테이블이 말하게 되고, 두 명령이 같은 `command_id` 를 쓰는 경우(채굴 명령과 판매 명령이 같은 UUID — 클라이언트가 고른다)에 장부 하나는 막고 하나는 통과시킨다.

## 결정

### 1. 장부·배치·정지 경로는 ADR-0013 그대로다 — 새 장부를 만들지 않는다

- `SELL_MINERAL` 은 상태 변경 명령이다. 수락되면 그 tick 의 배치에 `processed_commands(world_id, command_id, actor_id, 'SELL_MINERAL', tick)` 이 들어간다. **같은 표, 같은 PK, 같은 월드 범위 기억**(ADR-0013 §3 K1). 따라서 `MINE_RESOURCE` 로 수락된 `command_id` 를 `SELL_MINERAL` 로 다시 보내면 `DUPLICATE_COMMAND_ID` 다 — 명령 종류가 달라도 같은 키는 한 번만 세계를 바꾼다.
- 게이트웨이의 `RECORDING_BACKLOG`(ADR-0013 §6)는 `SELL_MINERAL` 에도 건다(상태 변경 명령 목록에 추가). 손실 창 1 초는 판매에도 그대로다 — 크래시는 판매를 잃을 수는 있어도(인벤토리·지갑·이벤트·장부가 **함께**) 반만 남기지 않는다.
- 복구 불가 기록 실패 = 월드 정지(ADR-0013 §5, 사용자 결정 Q1)도 그대로다. 그래서 **새 테이블의 모든 제약이 정지 경로 표(§5a)에 행으로 들어간다**(§5).
- 기동 순서(ADR-0013 §7) 2단계 "경제 상태 적재" 에 지갑·시장 상태를 더한다. 4단계(tick 재개) 전이다.

### 2. 지갑은 actor 소유 상태 행이다

- `wallets(world_id, actor_id, balance_cr)` — 키 `(world_id, actor_id)`. 소유자는 인벤토리와 같은 이유로 actor 다(ADR-0013 §1 — 함선은 영속하지 않는다).
- **행은 첫 판매에 생긴다.** 행이 없는 actor 의 잔액은 0 이다(인벤토리의 "행 없음 = 0 kg" 과 같다). 비교 후 쓰기의 기대값은 "행 없음" 이다.
- 크레딧은 **정수, 1 credit = 최소 단위**, `0 ≤ balance_cr ≤ 2^53−1`(계약 `CreditAmount`). 열·필드 접미사는 `_cr`. 실수 없음. 가격 규칙의 모든 나눗셈은 **내림**(플레이어가 덜 받는 쪽)이다 — 반올림이 크레딧을 만들 수 있는 유일한 자리를 닫는다.
- **행을 지우지 않는다.** 인벤토리 행도 판매로 0 이 되면 0 으로 갱신하고 지우지 않는다(`INVENTORY_STATE` 는 0 kg 항목을 싣지 않는다 — 와이어 불변식은 그대로). 상태 표에 DELETE 경로가 생기면 "행 없음 = 0" 과 "행 = 0" 두 표현이 섞이고, 비교 후 쓰기의 기대값이 둘 중 하나를 틀린다.

### 3. 판매 한 건 = 도메인 이벤트 한 건(`MINERAL_SOLD`), 세 상태의 before/after 를 다 싣는다

- 판매는 하나의 사실이다: *"actor X 가 시장 M 에 광물 m 을 q kg 팔고 c 크레딧을 받았다."* 인벤토리 감소와 지갑 증가를 **두 이벤트**(같은 `correlation_id`)로 나누지 않는다. 나누면 한쪽만 커밋된 배치가 "구조적으로 표현 가능" 해지고, QA 의 짝 검사가 하나 늘며, 역사 판정기가 미래에 반쪽 거래를 읽을 자리가 생긴다. 한 tick 배치가 원자적이므로 원자성은 이미 있다 — 이벤트를 나눌 이유는 표현뿐인데 표현은 프로젝션의 일이다.
- payload 는 비교 후 쓰기와 QA 항등식이 필요로 하는 before/after 를 **전부** 싣는다: 인벤토리 `quantity_before/after_kg`, 지갑 `wallet_before/after_cr`, 받은 `proceeds_cr`, 시장 `saturation_before/after_kg`·`price_before/after_cr_per_kg`·`base_price_cr_per_kg`·`pricing_version`(§4). 정본은 `contracts/events/domain/MINERAL_SOLD.schema.json`. `causation_id` = `command_id`.
- **Level 0 — 역사가 아니다**(원칙 4). ADR-0007 §1 표에 "기록 — 역사 아님" 으로 들어간다. 역사 러너는 이 타입을 판정 대상으로 두지 않고 통과시킨다(레지스트리 consumers 에 `history` 를 넣지 않는다 — p1-02 스펙 §5.1).
- 거부된 판매는 기록하지 않는다(ADR-0007 §1). 지갑·인벤토리·시장·장부 어느 것도 바뀌지 않는다.

### 4. 시장 상태 — 광맥과 같은 모양(게으른 닫힌 식)

designer3 가 **구간형 포화 + tick 배수 회복**을 골랐다(디자인 `docs/design/p1-03-trading.md` §3 — 고정가는 "다른 플레이어가 흔적을 볼 수 있는가" 에 답하지 못해 버렸다). 이 ADR 이 요구하는 구조:

- 시장의 광물별 상태 **`market_states(world_id, market_id, mineral_id, saturation_kg, as_of_tick, last_seller_actor_id, last_sale_quantity_kg)`**, 키 `(world_id, market_id, mineral_id)`. **행은 그 광물의 첫 판매에 생긴다**(행 없음 = 포화 0, 마지막 거래 없음). `as_of_tick` 은 마지막 판매의 tick 이고 마지막 거래 표시의 tick 을 겸한다 — 행은 판매로만 쓰이므로 둘은 언제나 같다(같은 값을 두 열에 두지 않는다).
- **회복은 광맥 회복과 같은 게으른 닫힌 식**이다(ADR-0013 §1): `effective(t) = max(0, s − recovery_kg × (⌊t/I⌋ − ⌊as_of/I⌋))`, `I = recovery_interval_s × tick_hz`. 회복은 **DB 쓰기도 이벤트도 만들지 않는다.** 회복식은 **sim 에만** 있다(ADR-0013 §5 K3 — 영속화는 기대값을 저장 표현 그대로 받아 동등 비교만 한다).
- **가격은 저장하지 않는다.** 포화와 `data/` 에서 유도되는 값이다. 저장하면 두 값이 갈라질 자리가 생긴다. 와이어(`MARKET_STATE`)와 이벤트(`MINERAL_SOLD`)에는 그 시점에 유도한 값을 싣는다 — 이벤트의 값은 원장이고 QA 가 재계산해 대조한다.
- 판매 1 건의 대금은 구간을 걷는다 — 반복은 최대 `floor_band + 1` 회(디자인 §3.3)이고 kg 수와 무관하다. **kg 마다 도는 루프는 금지**(수량 상한이 2^31−1 kg).
- **가격 규칙은 버전을 가진다**: 시장 파일의 `pricing_version`(`{market id}@{n}`)을 모든 `MINERAL_SOLD` 에 싣고, golden 테스트가 그 버전을 가격 절의 정규화 해시와 고정 입력의 대금에 묶는다(ADR-0014 §5 와 같은 방식). 숫자를 바꾸고 버전을 안 올리면 golden 이 실패한다 — 과거 판매가 "어느 규칙으로 값이 매겨졌나" 를 잃지 않게.
- 마지막 거래(`last_seller_actor_id`·`last_sale_quantity_kg`·`as_of_tick`)는 **시장의 공개 게시판**이다(디자인 §4 — 판 사람이 떠난 뒤에도 남는 흔적). 영속 상태이고, 재기동을 넘는다.

### 5. 정지 경로 표(ADR-0013 §5a) 갱신 — 분모를 다시 센다

새 마이그레이션 **0004**(0001~0003 은 동결 — ADR-0007 §5)가 만드는 테이블의 제약 전부를 이름으로 적고 ①②③ 을 매긴다. 아래는 **요구 제약 집합**이다 — 구현 뒤 qa 의 `constraint_census.py` 가 `pg_constraint` 와 대조한다.

**`wallets` (0004, 6) — 키 `(world_id, actor_id)`**

| 제약 | ① | ② | ③ |
|---|---|---|---|
| `wallets_pkey` | 아니오 — `actor_id` 는 인증에서 | — | — |
| `wallets_world_id_fkey` | 아니오 | 기동 대조 | — |
| NOT NULL × 3 (`world_id`·`actor_id`·`balance_cr`) | 아니오 | — | — |
| **`wallets_balance_cr_check`** `balance_cr BETWEEN 0 AND 9007199254740991` | **누적**(상한) — 판매가 더한다. 하한 0 은 **아니오** — 이번 슬라이스에 크레딧을 빼는 경로가 없다 | 상한: sim 의 checked 덧셈 → **`CAPACITY_EXCEEDED` 거절**(의미 확장 — §6). 하한: 빼는 경로가 생기는 슬라이스가 판정 단계(잔액 부족 거절)를 함께 넣는다 | 경계값 주입 단위 테스트(지갑을 상한 − 1 로 두고 판매 → 거절, 네 상태 불변, `commands_rejected_total{CAPACITY_EXCEEDED}` +1) |

**`market_states` (0004, 12) — 키 `(world_id, market_id, mineral_id)`, 행은 첫 판매에 생긴다**

| 제약 | ① | ② | ③ |
|---|---|---|---|
| `market_states_pkey` | 아니오 — `market_id`·`mineral_id` 는 판정 2단계(`TARGET_UNKNOWN`)를 통과한 표의 id | `TARGET_UNKNOWN` | 스펙 AC-3(b) |
| `market_states_world_id_fkey` | 아니오 | 기동 대조 | — |
| NOT NULL × 7 (`world_id`·`market_id`·`mineral_id`·`saturation_kg`·`as_of_tick`·`last_seller_actor_id`·`last_sale_quantity_kg`) | 아니오 | — | — |
| `market_states_saturation_kg_check` `BETWEEN 0 AND 2147483647` | 아니오 — 포화는 `floor_band × band_kg`(포화 상한)에서 멈춘다(디자인 §3.3). 그 상한이 2^31−1 이하임은 **기동 검증**이다 | 기동 검증 + 구간 걷기 | 스펙 AC-2(b)·AC-4(d) |
| `market_states_as_of_tick_check` 범위 | 아니오 | — | — |
| `market_states_last_sale_quantity_kg_check` `BETWEEN 1 AND 2147483647` | 아니오 — 수락된 판매의 kg(스키마 `minimum: 1`) | 스키마 | — |

**기존 표의 재검토** (판매가 생기며 ① 이 바뀌는 것):

| 제약 | 이전 | 이후 | ② | ③ |
|---|---|---|---|---|
| `inventory_items_quantity_kg_check` 하한 0 | 아니오(빼는 경로 없음) | **예 — 클라이언트가 `quantity_kg` 를 고른다** | 판정 단계 "보유량 ≥ 요청량" → **`INSUFFICIENT_QUANTITY` 거절**, 그리고 checked 뺄셈(디버그·릴리스 같은 경로 — ADR-0013 §5a-2 #4 의 이유 그대로) | 보유량 + 1 을 요청하는 단위 테스트(거절·불변·카운터 +1) **와** 보유량 정확히를 요청하는 테스트(수락, 행 = 0, `INVENTORY_STATE` 에 그 항목 없음) — 경계 양쪽 |
| `processed_commands_pkey` | 예 | 예 — 판매도 같은 키 공간 | 월드 범위 기억이 명령 종류와 무관하게 막는다 | AC 의 "채굴로 수락된 `command_id` 를 판매로 재전송" 경로 |
| 정지 경로 #4(누적 넘침) | 인벤토리만 | 인벤토리 + **지갑** | 둘 다 checked 덧셈 → `CAPACITY_EXCEEDED` | 위 두 경계값 테스트 |

> ⚠ 위 두 표의 소제목 괄호 숫자는 ADR-0013 §5a-1 과 같은 형식이다 — qa 의 `constraint_census.py` 가 이 ADR 도 파싱하게 하려면 qa 가 대상 파일 목록에 이 ADR 을 더한다(형식을 바꾸지 않는다).

**집계(요구)**: 경제 54(0001·0002) + `wallets` 6 + `market_states` 12 = **72 제약(월드 정지 경로)** + 트리거 1 / 역사 50 + 트리거 3 → **분모 합계 122 제약 + 트리거 4.** 0004 는 트리거를 만들지 않는다(상태 표는 추가 전용이 아니다). **① 예 = 2**(`processed_commands_pkey`, `inventory_items_quantity_kg_check` 하한), **① 누적 = 2**(인벤토리 상한, 지갑 상한). 넷 다 ② 와 ③ 이 있다. **① 예·누적이면서 ② 가 없는 제약 = 0.**

**독립 대조 (architect3, 2026-10-05 — ADR-0013 §5a-1b 와 같은 이유: 구현자의 실측만 옮기면 census 가 자기 일치가 된다).** serverdb3 실측(`03_serverdb_impl.md` §1b)과 별개로, 임시 DB `starfall_test_architect_census_p103` 에 0001~0004 를 `psql` 로 직접 적용해 `pg_constraint` 를 종류별로 셌다 → 적용 뒤 `DROP`(증거 DB `starfall` 은 읽지도 쓰지도 않음). 결과: `worlds` 11 · `domain_events` 17 · `inventory_items` 7 · `deposit_states` 11 · `processed_commands` 8 · **`wallets` 6**(p1·f1·n3·c1) · **`market_states` 12**(p1·f1·n7·c3) = **경제 72** / `historical_events` 22 · `historical_event_sources` 6 · `evidence` 14 · `history_cursor` 8 = **역사 50** / 사용자 트리거 **4**. 이름 8 개(NOT NULL 제외)가 위 표와 같고, CHECK 범위(`balance_cr` 0~2^53−1, `saturation_kg` 0~2^31−1, `as_of_tick` 0~2^53−1, `last_sale_quantity_kg` 1~2^31−1)가 0004 파일과 같다. **분모 122 + 트리거 4 확정.**

### 6. 거절 사유

- **`CAPACITY_EXCEEDED` 의 의미를 넓힌다**: "보유량 — 광물 인벤토리 또는 **지갑** — 이 표현 가능한 최대를 넘게 된다". 새 코드를 만들지 않는 이유: 플레이어에게는 같은 뜻(보유 한도)이고, 실전 도달은 둘 다 없으며(지갑 2^53−1 은 월드 전체 공급으로 수만 년), 경계값 주입으로만 실행된다. 클라이언트 표시가 "어느 쪽 한도" 를 구분해야 하면 그때 나눈다.
- **`INSUFFICIENT_QUANTITY`**(신규): 요청한 kg 이 보유량보다 많다. `RESOURCE_DEPLETED`(광맥이 비었다)와 다른 뜻이다 — 재사용하면 클라이언트 표시와 QA 판정이 둘 다 틀린다(ADR-0013 §5a-2 의 `CAPACITY_EXCEEDED` 논리).
- 그 밖의 사유는 **재사용**한다(디자인 Q3 채택): 없는 시장·상장 없는 광물 = `TARGET_UNKNOWN`, 판매 쿨다운 = `COOLDOWN_ACTIVE`, 도킹 구역 밖 = `TARGET_OUT_OF_RANGE`, 너무 빠름 = `SHIP_TOO_FAST` — 같은 뜻이고 대상만 광맥에서 시장으로 바뀐다. 새 코드를 만들면 클라이언트 분기가 같은 표시를 두 번 갖는다(클라이언트 문장은 명령 종류로 가를 수 있다). 판정 순서의 정본은 스펙 p1-03 §4.2. 가격 하한(`min_proceeds`)은 두지 않는다 — 대금이 분할 무관이고(디자인 §3.4) 같은 tick 경쟁은 제출 순번으로 결정되며, 실제 대금은 `SALE_RECEIPT` 로 즉시 보인다.

### 7. 결정성

- 판매 판정과 총액은 무작위가 없다. 입력은 `data/` 의 가격 규칙, 시장 상태, 인벤토리, 요청 kg 뿐이다.
- 같은 tick 의 판매는 **제출 순번 순**이다(p1-02 I-71 과 같은 이유 — `ship_id` 순이면 작은 id 가 높은 구간을 항상 먼저 쓴다).
- 채굴 golden·이동 golden 은 재생성 없이 통과해야 한다. 새 메시지(`WALLET_STATE`, 시장 메시지)는 **메시지 id 흐름**에서 id 를 꺼낸다(ADR-0006 §4a) — 세계 id 시퀀스가 밀리지 않는다.

## 검토한 대안과 버린 이유

- **판매 전용 장부(`processed_sales`)**: §1 맥락 — 같은 `command_id` 를 두 장부가 각자 판정한다. 채굴로 수락된 키를 판매로 다시 쓰는 것이 통과한다.
- **이벤트 둘(`MINERAL_REMOVED` + `CREDITS_ADDED`, 같은 correlation)**: §3. 원자성은 배치가 이미 주고, 둘로 나누면 짝 검사와 반쪽 거래 표현만 생긴다.
- **지갑을 인벤토리의 한 항목(`mineral_id = 'credits'`)으로**: 단위가 kg 이 아니고(MassKg 2^31−1 상한이 화폐에 맞지 않다), 판매가 "같은 표의 두 행을 바꾸는" 모양이 되어 비교 후 쓰기를 접는 규칙(K3)이 거래 한 건 안의 두 행을 같은 행으로 착각할 자리가 생긴다.
- **크레딧을 델타로 쓰기(`credits = credits + n`)**: ADR-0013 "버린 대안" 그대로 — 모호한 커밋 재시도의 이중 적용에 두 번째 방어가 없다.
- **가격을 저장**: §4. 상태에서 유도되는 값을 저장하면 두 값이 갈라진다.
- **크레딧에 소수 최소 단위(1/100)**: 쓰는 곳이 없다. 가격 규칙에서 1 credit 이 너무 거칠면 designer 가 가격을 "톤당" 으로 정하면 된다 — 최소 단위를 바꾸는 것은 나중에 저장된 모든 값의 의미를 바꾸는 일이라 지금 결정한다: **1 credit 이 최소 단위다.**

## 결과

- 좋은 점: 복사 방어 네 겹(메모리 기억·PK·배치 멱등·비교 후 쓰기)이 판매에 **코드 추가 없이** 걸린다. 원장 항등식이 SQL 로 잰다: 인벤토리 = Σ채굴 − Σ판매, 지갑 = Σ판매 총액, 그리고 연쇄(before/after). 화폐 총량 = Σ 지갑 = Σ 판매 총액(소비처가 없으므로 정확히 같다 — 소비처가 생기면 항등식에 항이 붙는다).
- 감수할 점: 판매가 실패하면 월드가 멈춘다(ADR-0013 의 대가 그대로). 시장 회복식이 두 번째 게으른 식이 되고 QA 의 재계산 도구(포화 되감기)가 하나 는다.
- 다시 검토할 조건: (a) 크레딧 소비처(구매·수리·수수료)가 생기는 슬라이스 — 지갑 하한 ② 와 잔액 부족 사유, (b) 플레이어 간 거래 — 두 actor 의 행을 한 명령이 바꾸는 첫 경우(비교 후 쓰기의 순서와 교착), (c) 시장이 둘 이상 — 시장 사이 차익의 불변식.

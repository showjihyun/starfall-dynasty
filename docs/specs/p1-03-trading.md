# p1-03-trading: 첫 화폐 — 캔 광물을 성계 거래소에 판다

- 상태: **agreed** (2026-10-05 — 사용자 결정 Q1~Q3 전부 추천안. designer3 디자인·history3 검토(#42 완료) 반영. ADR-0016 accepted — server T0 검토 동의)
- 로드맵 Phase: p1 (코어 프로토타입) — 셋째 슬라이스. GDD §36 MVP-1 의 "거래"
- 근거 기획안 절: GDD §7.3(가격은 고정값이 아니다 — Supply·Local Scarcity 두 입력만 구현), GDD §33(서버 판정 8단계), GDD §36(MVP-1 "거래", **핵심 테스트: 다른 플레이어가 그 행동의 흔적을 발견할 수 있는가**), GDD §37(시장·주문장은 MVP-2), TECH §26–27(화폐 복사가 최악의 버그, Trade Request → Server Validation → DB Transaction → Inventory Update → Economy Event), HSE §9(일반 거래는 Level 0), HSE §89(MVP 역사 10종에 거래 없음)
- 관련 ADR: **0015**(클라이언트 데이터 배포 — 신규, accepted. p1-02 만기 부채), **0016**(크레딧과 판매 장부 — 신규, accepted 2026-10-05 server T0 검토). 기반: **0013**(경제 영속·지속 멱등 — 그대로 재사용), 0006 §4a(메시지 id 흐름), 0007 §1·§5(기록 범위·마이그레이션 동결), 0014 §1·§6(러너 통과·세션 시작 전달)
- 입력: `docs/design/p1-03-trading.md`(designer3 — **수치·규칙·시나리오 T-1~T-12·화면 UI-01~UI-13 의 정본**), `_workspace/p1-03-trading/02h_history_review.md`(history3 — 역사 판단·R-1~R-9·H-30~H-34 의 정본)
- **이 스펙은 수치를 정하지 않고 모양·한계·불변식만 정한다**(p1-01·p1-02 와 같은 분업). 수치는 `data/` 와 디자인 문서에 있다. 아래에 보이는 숫자 예(2,000 kg → 20,000 cr 등)는 디자인 §3.6·부록 B 에서 온 **기대값**이고, 디자인이 바뀌면 함께 바뀐다.

## 1. 목표

이 슬라이스가 끝나면 플레이어는 **채굴한 광물을 항성 위 거래소에 가져가 팔아 크레딧을 벌 수 있고, 자기 판매가 모두가 보는 시세를 움직여, 판 사람이 떠난 뒤에도 늦게 온 사람이 그 흔적(눌린 시세·마지막 거래 줄)을 보고 다른 광물을 캐러 간다.**

증명할 것은 넷이다.

1. **첫 화폐가 서버 권위로 생긴다.** 클라이언트는 "이 시장에 이 광물을 이만큼 팔겠다" 만 보낸다. 가격·대금·잔액은 어휘에 없다.
2. **돈과 물건이 한 트랜잭션 + 도메인 이벤트 하나로 바뀐다** — 이후 현상금·계약이 설 바닥. 같은 명령은 어떤 경로로도 한 번만 반영되고(복사 불가), 크레딧은 정수·음수 불가다.
3. **가격이 결정적으로 움직이고 회복한다** — 무작위 없이 정수 식으로, 저장 없이 게으른 식으로.
4. **다른 플레이어가 흔적을 본다**(GDD §36) — 역사 기록이 아니라 **시뮬레이션 상태**(시장 포화·마지막 거래)로. 이 슬라이스는 새 Historical Event 를 만들지 않는다(§6).

designer 의 재미 가설(디자인 §0.1): *시장 가격은 모든 사람이 같이 쓰는 기억이다. 누가 Ferrosite 를 쏟아부으면 값이 떨어지고, 그 자리는 판 사람이 떠난 뒤에도 십여 분 남는다.*

그리고 p1-02 의 만기 부채 하나를 닫는다: **클라이언트 빌드에 광맥의 정답(광물·매장량)이 들어가는 누출**(ADR-0015, 분모 2 → 0).

## 2. 플레이 흐름

1. **개발자**가 서버를 띄운다. 서버는 p1-02 의 `data/` 에 더해 `data/market/cradle-exchange.json` 과 분할된 `data/world/deposits/cradle.json`(공개부, schema_version 2)·`data/world/deposit-reserves/cradle.json`(서버 전용)을 읽고 §4.8 의 검산을 한다. 어긋나면 **기동 거부**.
2. **서버**가 그 월드의 인벤토리·광맥·처리 장부에 더해 **지갑·시장 상태**를 DB 에서 적재한 뒤 tick 을 재개한다(ADR-0013 §7 2단계 확장).
3. **플레이어 A** 가 접속한다. `SESSION_READY` → `INVENTORY_STATE` → `WALLET_STATE`(새 actor 는 `balance_cr: 0`) → `DEPOSIT_FIELD_STATE` → `MARKET_STATE`(새 월드에선 `listings: []` — 아직 아무 광물도 드러나지 않았다) → 역사가 있으면 `HISTORICAL_EVENT_NOTICE{BACKFILL}`.
4. **A** 가 Inner Belt 1 에서 Ferrosite 를 캔다(p1-02). 첫 채굴이 광맥을 드러내므로 서버는 그 tick 에 `MARKET_STATE` 를 월드 전원에게 다시 보낸다 — 이제 Ferrosite 가 상장돼 있다(포화 0, 10 cr/kg). A 의 화면에 "새 상장" 알림(UI-10).
5. **A** 가 2,000 kg 을 들고 항성 위 거래소로 날아가 도킹 구역(표면 + 200 m) 안에서 10 m/s 이하로 멈춘다. 시장 패널(UI-07)이 열린다. 판매량을 고르고 Enter. 클라이언트는 `SELL_MINERAL{market_id: "cradle-exchange", mineral_id: "ferrosite", quantity_kg: 2000}` 하나를 보낸다 — **가격·대금·잔액·위치를 보내지 않는다.**
6. **서버**의 tick 이 판정한다(§4.2): 처리한 `command_id` 인가 → 시장·상장이 있는가 → 판매 쿨다운이 끝났는가 → 서버가 아는 함선 위치가 도킹 구역 안인가 → 충분히 느린가 → 보유량 ≥ 요청량인가 → 지갑이 넘치지 않는가. 통과하면 포화를 회복식으로 맞춘 뒤 구간을 걸어 대금을 정수로 계산하고(§4.2a), 인벤토리를 줄이고, 지갑을 늘리고, 포화·마지막 거래를 갱신하고, `MINERAL_SOLD` 1건을 발행한다. `causation_id` = 그 `command_id`.
7. **서버**가 A 에게 `COMMAND_RESULT{ACCEPTED}` → `SALE_RECEIPT{대금 20,000, 10 → 9 cr/kg}` → `INVENTORY_STATE`(Ferrosite 항목 없음) → `WALLET_STATE{20,000}` 를, 월드 전원에게 `MARKET_STATE`(Ferrosite 9 cr/kg, 포화 2,000, 마지막 거래 A · 2,000 kg)를 보낸다. 거부면 `COMMAND_RESULT{REJECTED, 사유}` 뿐이고 **아무것도 바뀌지 않는다 — 쿨다운도.**
8. **영속화**가 그 tick 의 이벤트 + 인벤토리 행 + 지갑 행 + 시장 행 + 처리 장부 + `last_tick` 을 **한 트랜잭션**으로 커밋한다(ADR-0013 §2, ADR-0016 §1).
9. **역사 러너**는 `MINERAL_SOLD` 를 판정 대상이 아닌 행으로 **통과**시키고 커서를 넘긴다. 역사는 생기지 않는다(원칙 4).
10. **A** 가 광맥 둘을 비워 Ferrosite 14,000 kg 을 한 번에 판다 → 시세 바닥(3 cr/kg), 포화 상한. A 가 접속을 끊고 잔류 30 초 뒤 함선이 디스폰된다.
11. **플레이어 B** 가 1 분 안에 처음 접속한다. 첫 `MARKET_STATE` 에서 Ferrosite 3 cr/kg(▼70%), `마지막 거래: Pilot-xxxx · 14,000 kg · 1분 전`. **A 는 월드에 없는데 B 는 A 의 흔적을 본다.** B 는 Glacine 을 캐러 간다.
12. 60 초마다(월드 tick 이 1,200 의 배수일 때) 포화가 회복한다 — DB 쓰기도 이벤트도 없이. 실효 포화가 바뀐 tick 에 `MARKET_STATE` 가 다시 나간다. 14 분 뒤 Ferrosite 는 기준가로 돌아온다. **마지막 거래 줄은 다음 판매까지 남는다.**
13. **A 의 조작된 클라이언트**가 같은 `command_id` 를 반복한다(재접속·재기동 뒤에도) → `DUPLICATE_COMMAND_ID`. 지갑은 한 번만 늘어 있다. 채굴로 수락된 `command_id` 를 판매로 보내도 같다.
14. **QA** 가 `psql` 로 확인한다: 지갑 = Σ 대금, 인벤토리 = Σ 채굴 − Σ 판매(연쇄 포함), 광물별 질량 보존(판매 항 포함), 포화 재계산 일치, 화폐 총량 = Σ 대금.

## 3. 범위

**포함**

- ADR-0015(신규, accepted)·ADR-0016(신규, accepted), ADR-0007 §1 표에 `MINERAL_SOLD` 행, ADR-0014 §6 개정 메모(세션 시작 메시지 수 — history R-6), 이 스펙, 작업 분해
- 계약(§5): 명령 `SELL_MINERAL`, 도메인 이벤트 `MINERAL_SOLD`, 서버 메시지 `WALLET_STATE`·`SALE_RECEIPT`·`MARKET_STATE`, 데이터 `MARKET`·`DEPOSIT_RESERVE`, `DEPOSIT_FIELD` schema_version 2, `COMMAND_RESULT.reason_code` += `INSUFFICIENT_QUANTITY`, primitive `CreditAmount`, 레지스트리 `data_files` 필드(registry_version 5)
- 서버: 시장 데이터 적재·검산, 광맥 표 분할 적재, 판매 판정·대금 계산, 지갑·시장 상태(메모리 사본 + PG 정본), 포화의 게으른 회복, 판매 쿨다운(메모리), 마이그레이션 **0004**(`wallets`·`market_states`), tick 배치 확장(지갑·시장 행 비교 후 쓰기, 인벤토리 감소), `RECORDING_BACKLOG` 대상에 판매, 세 메시지 송신, 가격 golden(`pricing_version`)
- 역사: **코드 변경 없음.** 러너 통과 테스트 H-30~H-33(history 소유 테스트 파일)
- 클라이언트(그레이박스, 디자인 §8): UI-01~UI-13, 판매 키(↑↓←→ Enter — 패널이 열려 있을 때만), 새 DTO, 데이터 사본을 ADR-0015 규칙으로(광맥 표 공개부·시장 표 추가, `history/rules/` 제외), SC-50 개정
- 데이터(designer): `data/market/cradle-exchange.json`, 광맥 표 기계적 분할(값 불변)
- QA: 봇 판매, 복사 경로 전수(명령 종류 교차 포함), 원장·질량·화폐·포화 항등식, 흔적 시나리오(T-3), 경쟁·부하(T-4·T-11), 화면 요소별 자동 항목(screen-elements S2), r1 전 사람 사전 세션(S4), `round_configs/p1-03-trading.json`

**제외 (다음 슬라이스로)**

- **구매(시장 → 플레이어)** — 살 이유가 없고, 되팔기가 유일한 용도면 차익 고리다(디자인 §3.7). 크레딧 사용처가 생기는 슬라이스에서 "무엇을 사는가" 로 다시 본다
- **크레딧 사용처**(함선 구매·수리·모듈·수수료) — 이번 범위는 "번다" 까지(Q1). 그래서 크레딧 차감 경로가 없고, 지갑 하한 0 의 ② 는 차감이 생기는 슬라이스가 넣는다(ADR-0016 §5)
- 적재 상한(GDD §6.1 20 t) — Q2. `MINE_RESOURCE` 판정을 다시 열지 않는다
- 플레이어 간 직접 거래·경매·주문장(GDD §37 MVP-2 "시장"), 시장 둘 이상, 가격 입력 중 수요·생산·전쟁·물류(읽을 시스템이 없다 — 디자인 §3.1)
- 가격 하한 보호(`min_proceeds`) — 대금이 분할 무관이고 같은 tick 경쟁은 제출 순번으로 정해지며 실제 대금은 `SALE_RECEIPT` 로 바로 보인다(ADR-0016 §6)
- Historical Event 신설 — 아래 표(history3 §1.2, R-8)
- 판매 쿨다운 영속화(재기동이 초기화한다 — 플레이어가 일으킬 수 없다), 계정·표시 이름(`Pilot-xxxx` 표지 유지), WebGL, techart

**기록하지 않기로 한 거래 사건** (history3 §1.2 — 다음 슬라이스가 같은 논의를 처음부터 하지 않게):

| 후보 | 판정 | 다시 볼 조건 |
|---|---|---|
| 모든 판매 | Level 0 — 원장이다 | — |
| 광물별 성계 첫 판매(`FIRST_SALE`) | 기록 안 함 — 첫 판매자는 거의 항상 발견자다. "첫 상장" 장면은 시장 목록 줄(UI-10)이 이미 만든다 | 발견과 판매가 다른 사람에게 일어나는 구조(운반·거래 전문 직업)가 생길 때 |
| 최대 거래·누적 거래량(`TRADE_RECORD`) | 기록 안 함 — 순위표는 프로젝션이고 쪼개기로 조작 가능한 기준이다 | 없음(프로젝션으로 만든다) |
| 시장 붕괴(`MARKET_CRASH`) | **이번엔 기록 안 함, p2 후보** — 유일하게 후속 플레이(Herald 기사·시장 조작 Claim·길드 독점)가 있다. 지금은 읽을 자리(Chronicle·Herald)가 없고, 30명 세션 초반 광물마다 한 번씩 뜨는 배너가 발견(Level 2)의 무게를 희석한다(디자인 §5.2). `MINERAL_SOLD` payload 가 판정에 필요한 사실을 **지금** 싣는다(§5.1) | Chronicle·Herald 가 생기는 p2 + 집계 규칙(Correlation Window) ADR(ADR-0014 재검토 조건 (a)). **"한 판매가 기준가에서 바닥까지" 조건은 그대로 쓰지 않는다** — 분할 무관(I-92)과 충돌해 14 t 한 번은 기록되고 7 t 두 번은 안 되는 숨은 규칙이 된다; 정직한 붕괴는 여러 판매를 창으로 묶는 집계 사건이다(디자인 §5.2-5). p2 기본값(디자인 §5.1, history3): 소급하지 않고 새 소비자 커서를 규칙 활성화 좌표에서 시작, 멱등 키는 바닥 진입 판매의 `(tick:sequence)`(서수는 쓰지 않는다) |
| 유물 거래(소유 이력) | 범위 밖 | `ARTIFACT_DISCOVERED` 이후 |

## 4. 규칙과 불변식

p0-01 I-1~I-9, p0-02 I-10~I-25, p1-01 I-26~I-47, p1-02 I-48~I-73 은 계속 유효하다(I-54·I-55 는 §4.3 이 확장한다). 수치(도킹 200 m·10 m/s, 판매 쿨다운 1 s = 20 tick, 구간·바닥·회복)는 `data/market/cradle-exchange.json` 이 정본이다.

### 4.1 클라이언트 권위 부정 (원칙 1)

- **I-74 `SELL_MINERAL` 의 payload 는 `{market_id, mineral_id, quantity_kg}` 다.** 가격·대금·잔액·위치·대상 actor 는 **어휘에 없다**(`additionalProperties: false` + `deny_unknown_fields` → `MALFORMED_COMMAND`). 가격은 서버가 시장 포화와 데이터에서 계산하고, 도킹은 서버의 `f64` 함선 상태로 잰다. 남의 인벤토리·지갑을 가리킬 필드가 없다 — 시도할 방법이 없다.
- **I-75 `quantity_kg` 는 의도다.** 서버는 그것을 자기 인벤토리 사본과 대조할 뿐(판정 6) 어디에도 더하지 않는다. 0 은 의도가 아니다(스키마 `minimum: 1`).
- **I-76 클라이언트는 인벤토리를 계산하지 않는다**(p1-02 I-49 를 감소에도). 예상 대금 미리보기(UI-08)로 보유량을 미리 깎지 않는다 — 판매 뒤 `INVENTORY_STATE` 로만 줄어든다.
- **I-77 클라이언트는 잔액을 계산하지 않는다.** 표시하는 잔액은 언제나 마지막 `WALLET_STATE` 다 — "옛 잔액 + 본 가격" 이 아니다. 판매 알림(UI-09)의 금액은 `SALE_RECEIPT` 의 실제 대금이다.
- **I-78 클라이언트는 현재 시세·포화를 계산하지 않는다.** 시세 티커(UI-05)·시장 패널(UI-07)의 현재 값은 마지막 `MARKET_STATE` 다. 예외는 **예상 대금 미리보기(UI-08)** 하나뿐이고, 그것은 같은 메시지의 포화를 입력으로 디자인 §3.3 순수 함수를 돌린 **표시용** 값이며 "예상" 으로 표기하고 어떤 명령·상태에도 쓰지 않는다. 클라이언트는 데이터 사본의 시장 표를 읽어도 된다(공개 종류 — §4.7) — 읽는 것은 위치·반지름·도킹 규칙(표시)과 상장 상수(기준가 대비 ▼%, 미리보기)다.
- 모르는 거절 사유(p1-02 I-73)는 그대로다. `INSUFFICIENT_QUANTITY` 는 이번에 아는 값이 된다.

### 4.2 판매 판정 (GDD §33 의 이번 구현)

- **I-79 판정 순서는 고정이고, 거부 사유는 그 순서의 첫 실패다. 거부는 아무것도 바꾸지 않는다** — 인벤토리·지갑·포화·마지막 거래·판매 쿨다운·처리 장부 어느 것도. `domain_events` 에도 남지 않는다(ADR-0007 §1). 순서는 designer 가 정했다(디자인 §2.1, `market_id` 단계만 architect 가 더함).

  | # | 검사 | 실패 사유 | 비고 |
  |---|------|-----------|------|
  | 0 | 게이트웨이: 스키마·크기·큐 | `MALFORMED_COMMAND` 등 기존 | ADR-0006 |
  | 0b | 게이트웨이: 상태 변경 명령이고 `recording_lag` > 20 | `RECORDING_BACKLOG` | **판매를 상태 변경 명령 목록에 더한다**(ADR-0016 §1) |
  | 1 | 같은 `command_id` 가 처리된 적 있다 | `DUPLICATE_COMMAND_ID` | 세션 기억 + 월드 지속 기억. **채굴과 같은 장부** — 명령 종류가 달라도 같은 키는 중복(I-82) |
  | 2 | `market_id` 가 함선이 있는 성계의 시장이고, 그 시장에 `mineral_id` 의 상장(데이터)이 있다 | `TARGET_UNKNOWN` | 둘 다 같은 사유(디자인 Q3). 데이터 상장은 모든 광물에 있으므로(§4.8) 상장 없음 = 그런 광물이 없음 |
  | 3 | 이 actor 의 판매 쿨다운이 끝났다: `tick ≥ 마지막 수락 판매 tick + sell_cooldown_ticks` | `COOLDOWN_ACTIVE` | 채굴 쿨다운과 **별개**. 수락된 판매만 시작한다. 메모리 상태 |
  | 4 | `|함선 − 시장|² ≤ (radius_m + dock_range_from_surface_m)²` | `TARGET_OUT_OF_RANGE` | tick 시작 시점(이번 tick 적분 전)의 서버 `f64` 상태. 제곱 비교 |
  | 5 | `|함선 속도|² ≤ max_ship_speed_mps²` | `SHIP_TOO_FAST` | 같은 시점 |
  | 6 | 인벤토리의 그 광물 ≥ `quantity_kg` | **`INSUFFICIENT_QUANTITY`** (신규) | checked 뺄셈. `inventory_items` 하한 CHECK 에 닿기 전의 거름 장치(ADR-0016 §5) |
  | 7 | 지갑 + 대금이 `CreditAmount` 상한(2^53−1)을 넘지 않는다 — checked 덧셈 | `CAPACITY_EXCEEDED` | 의미 확장(ADR-0016 §6). 실전 도달 없음 — 경계값 주입으로 실행 증거 |
  | 8 | 적용 | — | §4.2a |

  - *"상장됐는가(드러난 광물인가)" 는 판정 단계가 아니다*: 인벤토리에 있는 광물은 캔 적이 있고, 캔 적이 있으면 드러나 있다. 미드러난 광물을 팔려는 명령은 6 에서 `INSUFFICIENT_QUANTITY`(보유 0)다. 구조적으로 도달할 수 없는 사유는 만들지 않는다(p1-02 §4.2).
  - *함선이 없는 경우는 표에 없다*: 열린 세션은 언제나 함선 1척을 조종한다(p1-01 I-29).
- **I-71 확장 — 같은 tick 의 명령은 채굴·판매가 섞여도 제출 순번 순이다.** 한 tick 안의 단계: ① 회복(광맥 잔량·시장 포화 — 게으른 식이라 실제 작업은 없다) → ② 명령 처리 → ③ 이동 적분. 같은 tick 같은 광물의 두 판매는 먼저 처리된 쪽이 높은 구간을 쓴다(디자인 T-4: 20,000 / 18,000).
- `SELL_MINERAL` 도 tick 당 세션 명령 상한(8, I-47)에 포함된다.

### 4.2a 대금 — 구간 걷기 (정본: 디자인 §3.3, `data/market/*.json`)

- 정수 연산만. `floor_band = ⌈(100 − price_floor_pct) / price_drop_pct_per_band⌉`, `band(s) = min(s div band_kg, floor_band)`, `price(b) = base × max(floor_pct, 100 − b × drop_pct) div 100`. 대금은 포화 `s`(회복 적용 후)에서 `q` kg 을 구간별로 걸으며 `take × price(b)` 를 더하고, 바닥 구간에 닿으면 남은 양 전부를 바닥값으로 계산하며 포화는 상한(`floor_band × band_kg`)에 머문다.
- **I-92 가격 규칙의 성질** (디자인 §3.4 — 서버 단위 테스트가 그대로 단언한다): (1) **분할 무관** — 회복 배수 tick 을 넘지 않는 한 `sell(s, a+b)` 의 대금 = `sell(s, a)` 뒤 `sell(s', b)` 의 대금 합(쪼개 팔기·연타로 이득 없음). (2) **단조** — 같은 양이면 포화가 높을수록 대금 ≤, 같은 포화면 양이 클수록 대금 ≥. (3) **유계** — `0 ≤ s ≤ floor_band × band_kg`. (4) **대금 ≥ 판매량 ≥ 1** — 바닥값 ≥ 1 cr/kg 이 기동 검증이다. (5) **반복 ≤ floor_band + 1** — kg 수와 무관(ADR-0016 §4).
- 모든 나눗셈은 음이 아닌 피연산자의 0 쪽 버림이다 — 반올림이 크레딧을 만들 수 있는 자리가 없다. 곱은 `i64`, 상한 검산은 기동 시(§4.8).

### 4.3 멱등과 원자성 (ADR-0013 재사용, ADR-0016)

- **I-82 한 `command_id` 는 세계를 최대 1회 바꾼다 — 세션·재접속·재기동, 그리고 명령 종류를 넘어서.** 판매는 채굴과 같은 `processed_commands`(PK `(world_id, command_id)`)와 같은 월드 범위 기억을 쓴다. `MINE_RESOURCE` 로 수락된 키를 `SELL_MINERAL` 로 보내면(또는 그 반대) `DUPLICATE_COMMAND_ID` 다. 거절된 명령은 장부에 남지 않는다(p1-02 I-52 그대로).
- **I-83 판매 ⇔ `MINERAL_SOLD` 1건, 같은 커밋.** 인벤토리 행·지갑 행·시장 행·처리 장부·그 tick 의 이벤트·`last_tick` 은 한 트랜잭션이다. 판매는 이벤트를 **하나만** 낸다(ADR-0016 §3, history R-2). 거래는 **tick 배치로만** 커밋된다 — 두 번째 `domain_events` 작성자를 만들지 않는다(history R-1). 거래는 `MINERAL_MINED` 를 내지 않는다(history R-3).
- **I-80 원장 항등식** — p1-02 I-54 의 확장:
  - 각 `(world_id, actor_id, mineral_id)`: 인벤토리 행 **=** `Σ MINERAL_MINED.quantity_kg − Σ MINERAL_SOLD.quantity_kg` **=** 두 타입을 합친 `(tick, sequence)` 최대 이벤트의 `quantity_after_kg`.
  - 각 `(world_id, actor_id)`: 지갑 행(없으면 0) **=** `Σ MINERAL_SOLD.proceeds_cr` **=** 최대 `MINERAL_SOLD` 의 `wallet_after_cr`.
  - 각 월드: **화폐 총량** `Σ wallets.balance_cr = Σ MINERAL_SOLD.proceeds_cr`(이번 슬라이스엔 다른 출처·사용처가 없다 — 생기면 항이 붙는다).
- **I-81 연쇄** — 같은 `(actor, mineral)` 의 `MINERAL_MINED`·`MINERAL_SOLD` 를 합쳐 `(tick, sequence)` 순으로 놓으면 이웃한 두 이벤트에서 `앞.quantity_after_kg = 뒤.quantity_before_kg`, 첫 이벤트의 `before = 0`. 같은 actor 의 `MINERAL_SOLD` 끼리 `앞.wallet_after_cr = 뒤.wallet_before_cr`, 첫 판매의 `wallet_before_cr = 0`. 각 판매 안에서 `quantity_after = before − quantity`, `wallet_after = before + proceeds`.
- **I-84 질량 보존** — p1-02 I-55 의 확장(디자인 §2.4, history 확인 2): 각 `(world_id, mineral_id)` 에 대해 `Σ 인벤토리 + Σ 광맥 잔량 + Σ MINERAL_SOLD.quantity_kg = Σ 초기 매장량 + Σ 실제 회복량`. 시장은 **소멸점**이다(사기가 없다). **고치지 않으면 p1-02 의 AC-5(a) 판정 도구가 판매가 있는 월드에서 거짓 FAIL 한다** — qa 가 도구를 갱신한다.
- **I-85 포화 재계산** — 각 `(world_id, market_id, mineral_id)` 의 `MINERAL_SOLD` 를 `(tick, sequence)` 순으로 되감으며: 첫 판매의 `saturation_before_kg = 0`(회복 적용 후이므로 0 에서 0), 이웃한 두 판매에서 `뒤.saturation_before_kg = recover(앞.saturation_after_kg, 앞.tick → 뒤.tick)`, 각 판매의 `proceeds_cr`·`saturation_after_kg`·`price_*` 가 §4.2a 를 그 판매의 `pricing_version` 규칙으로 다시 계산한 값과 같다. 마지막 판매의 `saturation_after_kg`·tick 이 `market_states` 행과 같다. 다르면 결함이다.
  - **모든 항등식은 판매가 0 건이면 FAIL 로 인쇄한다**(CLAUDE.md). 검사한 키 수·이벤트 수·판매 수를 출력한다. 판매가 0 인 광물도 행으로 출력해 `0 = 0` 이 "검사했다" 와 구별되게 한다.
- **I-93 상태 행은 비교 후 쓰기다**(p1-02 I-56 그대로) — 지갑·시장 행의 기대값은 sim 이 저장 표현 그대로 싣는다(행 없음 포함). 괴리는 복구 불가 기록 실패 → **월드 정지**. **상태 행을 지우지 않는다**: 인벤토리가 0 이 되면 행은 0 으로 남는다(`INVENTORY_STATE` 는 0 kg 항목을 싣지 않는다 — 와이어 불변식 그대로).
- 배치 tick 멱등(p1-02 I-57)은 그대로다.

### 4.4 결정성 (원칙 9)

- 판매 판정·대금에 무작위가 없다. 입력은 데이터 표, 시장 포화, 인벤토리, 지갑, 요청 kg 뿐이다(디자인 §3.4-3 — "필요 없는 무작위는 넣지 않는다").
- **I-59 확장** — 같은 초기 상태 + 같은 명령열(채굴·판매 포함) = 같은 이벤트 내용과 같은 메시지(`MINERAL_SOLD` 와 `SALE_RECEIPT`·`INVENTORY_STATE`·`WALLET_STATE`·`MARKET_STATE` payload)가 두 프로세스에서 바이트 동일. **이동 golden(`sim/tests/data/replay/`)·채굴 golden(`replay_mining/`)·역사 golden 은 한 바이트도 바뀌지 않는다.** 새 메시지 셋은 메시지 id 흐름에서 id 를 꺼낸다(ADR-0006 §4a).
- **I-90 가격 규칙은 버전을 가진다.** 모든 `MINERAL_SOLD` 는 시장 파일의 `pricing_version` 을 싣는다. golden 테스트가 `pricing_version` 을 시장 파일 가격 절(rules 의 가격 필드 + listings)의 정규화 해시와 고정 입력(포화·양)의 대금 표에 묶는다(ADR-0014 §5 방식). **숫자를 바꾸고 버전을 안 올리면 golden 이 실패한다**(음성 대조가 AC-4(h)).
- 크레딧·kg 은 정수다(primitives `CreditAmount`·`MassKg`). 데이터 표의 `_cr`·`_cr_per_kg`·`_pct` 필드도 정수다.

### 4.5 시장 상태와 흔적 (GDD §36 핵심 테스트)

- **I-86 `MARKET_STATE.listings` 는 그 성계에 드러난 광맥이 하나라도 있는 광물만 싣는다.** 판단은 시뮬레이션 상태(`first_extracted_tick`, 광맥 표의 서버 전용부)이고 **역사 표를 읽지 않는다**(history R-9, p1-02 I-68 과 같은 선). 미드러난 광물의 이름·값은 어떤 서버 메시지에도 없다. 첫 채굴로 광물이 드러난 tick 에 `MARKET_STATE` 를 월드 전원에게 보낸다(UI-10 의 근거).
- **I-87 회복은 게으른 닫힌 식이다** — `effective(t) = max(0, s − recovery_kg × (⌊t/I⌋ − ⌊as_of/I⌋))`, `I = recovery_interval_s × tick_hz`(1,200). **DB 쓰기도 도메인 이벤트도 만들지 않는다.** 월드 tick 이 `I` 의 배수이고 실효 포화가 실제로 바뀐 상장이 하나라도 있으면 그 tick 에 `MARKET_STATE` 를 월드 전원에게 보낸다(없으면 보내지 않는다). 서버가 내려가 있는 동안은 회복도 없다.
- **I-88 마지막 거래는 영속 상태다.** `market_states` 의 `last_seller_actor_id`·`last_sale_quantity_kg`·`as_of_tick`(= 마지막 판매 tick). 재기동과 판매자의 퇴장을 넘는다. 클라이언트는 `actor_id` 를 p1-02 I-66 의 `Pilot-xxxx` 표지로 보이고(Q3), 표지는 표시 전용이다.
- 시장 판정(가격·허용)은 역사 표를 읽지 않는다(history R-9). 역사 → 플레이 방향의 연결 — 시장 행에 기존 `MINERAL_DISCOVERED` 의 발견자 표지를 보이는 것 — 은 **UI-13** 이 구현한다(디자인 §5.3). 출처는 클라이언트가 이미 받은 `HISTORICAL_EVENT_NOTICE`(BACKFILL·LIVE)뿐이고 `MARKET_STATE` 가 아니다 — 서버·DB 변경, 새 메시지 없음. 이 슬라이스의 **유일한 역사 → 플레이 연결**이다.

### 4.6 역사 (원칙 4·5, history3 검토)

- **I-89 이 슬라이스는 Historical Event 를 만들지 않는다.** `MINERAL_SOLD` 는 Level 0 이고 역사 러너는 그것을 `unjudged` 로 통과시켜 커서를 넘긴다(판정 대상이 아닌 타입은 payload·`schema_version` 을 보지 않는다 — 버전이 달라도 역사 정지 사유가 아니다, H-32). 레지스트리 `consumers` 에 `history` 를 넣지 않는다(통과는 소비가 아니다). `HISTORICAL_EVENT_NOTICE` 판별 union 만기는 **발동하지 않는다.**
- ADR-0014 §6 의 "세션 시작 메시지 3 건" 가정은 이제 5 건(`SESSION_READY`·`INVENTORY_STATE`·`WALLET_STATE`·`DEPOSIT_FIELD_STATE`·`MARKET_STATE`)이다 — BACKFILL 트리거 32 는 유지(history R-6, ADR-0014 개정 메모).

### 4.7 데이터 배포 (ADR-0015)

- **I-91 클라이언트 사본 = 레지스트리에서 `consumers` 에 `client` 가 있는 data 종류의 `data_files` 가 가리키는 파일 전부, 그리고 그것뿐이다.** 바이트 동일. 기대 집합이 비면 FAIL. `data/**/*.json` 은 각각 정확히 하나의 data 종류 glob 에 걸린다(0 개·2 개 = FAIL). **사본에 서버 전용 종류의 파일 0** — 이번에 `DEPOSIT_RESERVE`(광물·매장량)와 `SIGNIFICANCE_RULE` 이 사본에서 빠진다. 만기 표의 닫힘 조건 "서버 전용 필드 분모 2 → 0" 은 이 불변식의 실행 증거로 닫는다.
- 시장 표(`MARKET`)는 **공개 종류**다. 미상장 광물의 이름·값을 화면에 안 보이는 것(UI-06)은 p1-02 의 "미발견 M종" 과 같은 **표시 규칙**이고 — 광물 이름·rarity 는 이미 공개 `MINERAL` 표에 있다 — 빌드 파일 수준의 비밀이 아니다(디자인 §4 의 "누출 수용" 문장을 이 판단으로 대체한다).
- `tests/e2e/validate_data_files.py` 의 `TARGETS` 는 레지스트리 `data_files` 에서 유도한다(손으로 적은 두 번째 목록을 없앤다).

### 4.8 데이터 표 검증 (전부 기동 실패 — 로그에 파일과 필드)

스키마(§5) 통과에 더해:

- **시장**(디자인 §7): `star_system_id` 가 로드된 성계 / `pricing_version` 이 `id + "@"` 로 시작 / 모든 광물에 상장이 정확히 하나, 모든 상장의 `mineral_id` 가 광물 표에 있다 / `|position| + radius_m + dock_range_from_surface_m ≤ soft_boundary_radius_m` / 도킹 구역이 어느 광맥 채굴 구역과도 겹치지 않는다(`거리 > 두 반지름 + 도킹 사거리 + 채굴 사거리`) / 모든 상장의 바닥값 `base × floor_pct div 100 ≥ 1` / 모든 상장의 포화 상한 `floor_band × band_kg ≤ 2^31−1` / 시장 id 중복 없음.
- **기동 검산이 아니라 계약이 보장하는 것**(qa3 Q-1 판정, 2026-10-05 — 구조적으로 실패할 수 없는 검산은 두지 않는다. p1-02 §4.2 의 "도달할 수 없는 사유 코드는 만들지 않는다" 와 같은 원리): (1)(2) `sell_cooldown_s × tick_hz`·`recovery_interval_s × tick_hz` 가 정수 — 스키마가 초를 **정수**로 강제하고 `tick_hz` 도 정수(`worlds.tick_hz`)라 항등식이다. 소수 초는 **스키마층 기동 실패**로 막힌다(계약 SC-05 m08·m09 가 그 경로를 잰다). (3) `(2^31−1) × max(base_price_cr_per_kg) ≤ 2^53−1` — 스키마 `base_price_cr_per_kg ≤ 1,000,000` 에서 좌변 최대 ≈ 2.15 × 10^15 < 9.0 × 10^15. **이 상한을 올리는 계약 변경이 이 부등식을 다시 계산할 의무를 진다**(스키마 description 에 적었다). 대금 계산 자체는 checked 산술이고 넘침은 `CAPACITY_EXCEEDED` 로 간다(판정 7)
- **광맥 표 분할**(ADR-0015 §2): 같은 `star_system_id` 의 `DEPOSIT_FIELD`(v2)·`DEPOSIT_RESERVE` 가 짝으로 있고 `deposit_id` 집합이 **양방향으로 같다** / `DEPOSIT_FIELD` 의 `schema_version` 이 1 이면 기동 실패 / p1-02 §4.7 의 기존 검산 전부는 합친 모양에 그대로.

### 4.9 월드는 경제의 단위다 — 테스트의 격리

- 포화·마지막 거래·지갑은 월드마다 영속한다. **시세 숫자나 "첫 판매" 를 단언하는 검증은 새 월드에서 한다**(p1-02 I-70 의 `tests/e2e/new_world.py`). 기본 월드에서 하려면 시작 시점의 `MARKET_STATE` 를 먼저 읽고 기대값을 그 위에서 계산한다(디자인 §6 전제).

## 5. 데이터 계약

### 5.1 타입 (`registry_version` 4 → **5**, 타입 23 → **30**)

| 타입 | kind | 생산자 | 소비자 | 요지 |
|------|------|--------|--------|------|
| `SELL_MINERAL` | command | client, bots | server | `{market_id, mineral_id, quantity_kg ≥1}` 뿐 |
| `MINERAL_SOLD` | domain_event | server | server | `actor_id`·`causation_id` 비-null. payload `{ship_id, session_id, star_system_id, market_id, mineral_id, pricing_version, quantity_kg ≥1, proceeds_cr ≥1, base_price_cr_per_kg, floor_price_cr_per_kg, price_before_cr_per_kg, price_after_cr_per_kg, saturation_before_kg, saturation_after_kg, quantity_before_kg ≥1, quantity_after_kg, wallet_before_cr, wallet_after_cr ≥1}` — 디자인 §9.3 + history R-7 의 필드 전부 |
| `WALLET_STATE` | server_message | server | client, bots | `{actor_id, balance_cr}` — 세션 시작·변경 tick |
| `SALE_RECEIPT` | server_message | server | client, bots | `{command_id, market_id, mineral_id, quantity_kg, proceeds_cr, price_before_cr_per_kg, price_after_cr_per_kg}` — 잔액 없음(잔액의 출처는 `WALLET_STATE` 하나) |
| `MARKET_STATE` | server_message | server | client, bots | `{market_id, listings[{mineral_id, saturation_kg, price_cr_per_kg ≥1, last_sale: {actor_id, quantity_kg, tick} \| null}]}` — `mineral_id` 오름차순, 드러난 광물만(I-86) |
| `MARKET` | data | — | server, client | `data/market/*.json` — 공개 |
| `DEPOSIT_RESERVE` | data | — | **server** | `data/world/deposit-reserves/*.json` — 서버 전용(ADR-0015) |
| `DEPOSIT_FIELD` | 변경 → **schema_version 2** | — | server, client | `mineral_id`·`initial_reserve_kg` 제거 |
| `COMMAND_RESULT` | 변경 | server | client, bots | `reason_code` += `INSUFFICIENT_QUANTITY`. `CAPACITY_EXCEEDED` 의미 확장(지갑 포함). 같은 `schema_version` 의 호환 값 추가 |
| `SHIP_CLASS`·`STAR_SYSTEM` | 레지스트리만 | — | **+client** | 클라이언트가 p1-01 부터 읽었다 — `consumers` 가 사본 범위를 정하게 되어 실제에 맞춤 |
| (레지스트리) | — | — | — | data 종류마다 **`data_files`**(필수, glob) 신설 |

- 새 primitive: `CreditAmount`(정수 0~2^53−1, 1 credit = 최소 단위, 접미사 `_cr`). 기존 `MoneyMinor`(부호 있음)는 쓰지 않는다 — 음수가 의미 있는 화폐 값이 아직 없다.
- 닫힌 값 집합 추가 없음(`reason_code` 1 값 외). 모르는 값 처리 규약은 p1-02 §5.1 그대로.
- `MARKET_STATE.last_sale` 은 **필수이면서 nullable 인 중첩 객체**다 — 생성기는 `Required.AllowNull` 중첩 클래스를 만든다. 중첩 클래스 제목은 `MarketLastSale` 이다 — 속성 `LastSale` 과 겹치지 않게(아래 실측, ADR-0002 §4).

### 5.1a 명령 → 기대 응답 (레지스트리 `responses` 가 정본)

| 명령 | `always` | `accepted` (송신 순서) | 비고 |
|------|----------|----------|------|
| `PING_SERVER` | `COMMAND_RESULT` | `PING_REPLY` | 기존 |
| `SET_SHIP_CONTROL` | `COMMAND_RESULT` | — | 기존 |
| `MINE_RESOURCE` | `COMMAND_RESULT` | `INVENTORY_STATE` | 기존 |
| `SELL_MINERAL` | `COMMAND_RESULT` | **`SALE_RECEIPT` → `INVENTORY_STATE` → `WALLET_STATE`** — 같은 tick, 결과 뒤 | `MARKET_STATE`(월드 브로드캐스트)는 **응답이 아니다** |

게이트 의무 그대로: 유도 항등식(`accepted SELL_MINERAL 수 == 결과 뒤 SALE_RECEIPT 수 == 그 뒤 WALLET_STATE 수`)은 **판매가 0 건이면 FAIL**.

세션 시작 송신 순서: `SESSION_READY` → `INVENTORY_STATE` → `WALLET_STATE` → `DEPOSIT_FIELD_STATE` → `MARKET_STATE` → (`HISTORICAL_EVENT_NOTICE{BACKFILL}` …).

### 5.2 파일과 건수 (architect 실측, 2026-10-05)

| 파일 | 상태 |
|------|------|
| `contracts/common/primitives.schema.json` | `CreditAmount` 추가 |
| `contracts/commands/SELL_MINERAL.schema.json` | 신규 |
| `contracts/events/domain/MINERAL_SOLD.schema.json` | 신규 |
| `contracts/messages/{WALLET_STATE,SALE_RECEIPT,MARKET_STATE}.schema.json` | 신규 3 |
| `contracts/messages/COMMAND_RESULT.schema.json` | `INSUFFICIENT_QUANTITY` 추가, 설명 확장 |
| `contracts/data/market.schema.json`, `contracts/data/deposit-reserve.schema.json` | 신규 2 |
| `contracts/data/deposit-field.schema.json` | schema_version 2(`const`), 서버 전용 2 필드 제거 |
| `contracts/registry/types.json` | `registry_version` 5, 타입 30, `data_files` 9 종, `SELL_MINERAL.responses`, `SHIP_CLASS`·`STAR_SYSTEM` consumers +client |
| `contracts/registry/types.schema.json` | `data_files`(data 종류에 필수, 그 밖엔 금지 — `if/then/else`) |
| `contracts/fixtures/**` | 신규 유효 11 / 반례 39(+ `DEPOSIT_FIELD` 반례 1 삭제·2 추가) |

**건수 (Python `jsonschema` 4.23 실측)**: 스키마 36 개 전부 메타 스키마 통과, 레지스트리가 자기 스키마 통과. **유효 fixture 57(= 46 + 11) 전부 통과, 반례 113 전부 거부.** `DEPOSIT_FIELD` 의 유효 fixture 는 v2 로 다시 썼다(같은 파일 이름).

**코드 생성기 (실측)**: **생성은 됐으나 C# 컴파일에서 중첩 이름 충돌 1건 → 스키마 title 로 해결, 생성기는 충돌 시 실패하도록 검사 추가.** 2026-10-05 architect 실측은 생성(신규 5 DTO, 기존 16 unchanged, `ContractTypes.cs` 만 변경)까지였고 **컴파일하지 않았다** — 생성만 하고 컴파일하지 않은 확인은 확인이 아니다(검증 규율). client4 가 Unity 컴파일에서 CS0102 를 찾았다: `MARKET_STATE` 의 `$defs/LastSale` 이 `MarketListingState` 안의 중첩 클래스 `LastSale` 로, `last_sale` 속성도 `LastSale` 로 생성되어 이름이 겹친다. **해결(리더 결정 2026-10-06)**: 제목을 `MarketLastSale` 로 바꿨다(와이어 모양·fixture 불변). 생성기는 이름 규칙을 바꾸지 않고 **충돌을 감지하면 실패하는 검사**만 넣는다(client4) — ADR-0002 §4 에 규칙 한 줄. 전 스키마에서 같은 꼴은 이 1건뿐이었다(architect 스캔). 수정 뒤 생성물 22 개를 `netstandard2.1` + Newtonsoft 13 스크래치 프로젝트로 컴파일해 오류 0·경고 0(Unity 컴파일러가 아니다 — 판정은 client4 의 Unity 컴파일·EditMode 실행). 정수 매핑: `quantity_kg`·`saturation_kg` → `int`, `*_cr`·`*_cr_per_kg` → `long`.

**커버리지 기준선** (`check_contract_coverage.py --strict`, 구현 전): **types 30, errors 17, warnings 0** — 신규 타입의 코드 참조 없음 17건. 0 이 되는 것이 통과 조건이다.

**예측 (구현자가 실측으로 확인)**: Rust 레지스트리 하드코딩 23 → 30, fixture 테스트 46/74 → 57/113 로 **예상대로 RED**. C# EditMode 유효 fixture 발견 수 46 → 57, 왕복 36 → **45**(데이터 종류 12 건 제외 — 예측).

### 5.3 반례 fixture 의 층별 거부 책임

규약은 p1-02 §5.3 그대로다: 스키마 열은 전부 거부(architect 실측 113/113), Rust serde 열은 server 가 실측으로 채우고 하나라도 통과하면 serde 타입의 구멍이다, C# 열은 client 가 전수 역직렬화로 **거부 / 감지 불가 / 계층 없음** 을 실측해 테스트 상수로 박는다. 예측과 다르면 예측을 고친다.

**이번 변경분 C# 예측: 거부 19 / 감지 불가 10 / 계층 없음 10(+ `DEPOSIT_FIELD` 순증 1)** → 합계(기존 실측 34 / 23 / 17 위에) 예측 거부 53 / 감지 불가 33 / 계층 없음 27 = 113.

**실측 (client4, Unity EditMode 2026-10-06 — 정본): 거부 51 / 감지 불가 35 / 계층 없음 27 = 113.** 예측과 다른 칸은 2 건 — `MINERAL_SOLD/invalid/proceeds-fractional.json`·`WALLET_STATE/invalid/fractional-credits.json` 은 **감지 불가**다. Newtonsoft 는 소수 토큰을 `int` 필드에서는 `JsonReaderException` 으로 거부하지만(p1-02 의 소수 kg 반례는 전부 `int` 필드였다) `long` 필드(`CreditAmount`)에서는 받아들인다. 그 밖의 111 칸은 예측과 같다(`quantity-above-mass-max` 는 거부이고 예외가 리더 수준 `JsonReaderException` 임까지 확인). 영향: 소수 크레딧을 막는 층은 스키마·Rust serde 이고, 서버가 생산하는 메시지에서 소수가 나올 경로는 없다(서버 타입이 정수다). 클라이언트는 수신 경로의 방어선이 아니다 — 규약(§5.3 머리) 그대로 "감지 불가" 로 기록한다.

| fixture | 스키마 | Rust serde (예측) | C# Strict (예측) | 비고 |
|---|---|---|---|---|
| `SELL_MINERAL/invalid/price-field-injected.json` | 거부 | 거부 | 거부 | **대표 반례** — 가격은 어휘에 없다(I-74) |
| `SELL_MINERAL/invalid/proceeds-field-injected.json` | 거부 | 거부 | 거부 | |
| `SELL_MINERAL/invalid/balance-field-injected.json` | 거부 | 거부 | 거부 | |
| `SELL_MINERAL/invalid/position-field-injected.json` | 거부 | 거부 | 거부 | |
| `SELL_MINERAL/invalid/actor-field-injected.json` | 거부(`unevaluatedProperties`) | 거부 | 거부 | envelope 수준 주입 |
| `SELL_MINERAL/invalid/missing-market-id.json` | 거부 | 거부 | 거부 | |
| `SELL_MINERAL/invalid/quantity-fractional.json` | 거부 | 거부 | 거부(타입) | |
| `SELL_MINERAL/invalid/quantity-above-mass-max.json` | 거부(`maximum`) | 거부 | 거부(`int` 넘침) | |
| `SELL_MINERAL/invalid/quantity-zero.json` | 거부(`minimum: 1`) | 거부 | 감지 불가 | 0 은 의도가 아니다(I-75) |
| `MINERAL_SOLD/invalid/importance-level-injected.json` | 거부 | 거부 | 거부 | **원칙 4 의 계약상 경계** |
| `MINERAL_SOLD/invalid/causation-id-null.json` | 거부(좁힘) | 거부 | 거부 | |
| `MINERAL_SOLD/invalid/actor-id-null.json` | 거부(좁힘) | 거부 | 거부 | |
| `MINERAL_SOLD/invalid/proceeds-fractional.json` | 거부 | 거부 | ~~거부(타입)~~ **감지 불가(실측)** | 소수 크레딧 금지. `long` 필드는 Newtonsoft 가 소수 토큰을 받아들인다 |
| `MINERAL_SOLD/invalid/missing-pricing-version.json` | 거부 | 거부 | 거부 | I-90 |
| `MINERAL_SOLD/invalid/missing-floor-price.json` | 거부 | 거부 | 거부 | history3 C-1 — 바닥 판정을 payload 만으로 |
| `MINERAL_SOLD/invalid/proceeds-zero.json` | 거부 | 거부 | 감지 불가 | I-92(4) |
| `MINERAL_SOLD/invalid/quantity-after-negative.json` | 거부 | 거부 | 감지 불가 | |
| `MINERAL_SOLD/invalid/wallet-after-zero.json` | 거부 | 거부 | 감지 불가 | |
| `WALLET_STATE/invalid/fractional-credits.json` | 거부 | 거부 | ~~거부(타입)~~ **감지 불가(실측)** | 같은 이유(`long`) |
| `WALLET_STATE/invalid/negative-credits.json` | 거부 | 거부 | 감지 불가 | 음수 잔액 |
| `WALLET_STATE/invalid/above-safe-integer.json` | 거부 | 거부 | 감지 불가 | `long` 에 들어간다 |
| `SALE_RECEIPT/invalid/balance-field-injected.json` | 거부 | 거부 | 거부 | 잔액의 출처는 하나 |
| `SALE_RECEIPT/invalid/missing-command-id.json` | 거부 | 거부 | 거부 | |
| `SALE_RECEIPT/invalid/proceeds-zero.json` | 거부 | 거부 | 감지 불가 | |
| `MARKET_STATE/invalid/unlisted-hint-injected.json` | 거부 | 거부 | 거부 | 누출 경로(I-86) |
| `MARKET_STATE/invalid/missing-last-sale-key.json` | 거부(필수) | 거부 | 거부 | 필수 nullable 키 누락 — p1-02 `missing-mineral-key` 실측과 같은 꼴 |
| `MARKET_STATE/invalid/price-zero.json` | 거부 | 거부 | 감지 불가 | |
| `MARKET_STATE/invalid/saturation-negative.json` | 거부 | 거부 | 감지 불가 | |
| `MARKET_STATE/invalid/last-sale-quantity-zero.json` | 거부 | 거부 | 감지 불가 | 중첩 nullable 객체 안의 검증 |
| `DEPOSIT_FIELD/invalid/server-only-mineral-field.json` | 거부 | 거부 | (계층 없음) | **누출 재발 방지 대표 반례**(ADR-0015) |
| `DEPOSIT_FIELD/invalid/schema-version-1.json` | 거부(`const`) | 거부 | (계층 없음) | 옛 모양으로 기동하지 않는다 |
| `DEPOSIT_RESERVE/invalid/{reserve-fractional,reserve-zero,layout-field-injected}.json` | 거부 | 거부 | (계층 없음) | 공개부 필드가 서버 전용 표로 섞이지 않는다 |
| `MARKET/invalid/{price-fractional,price-zero,no-listings,missing-pricing-version,state-field-injected,floor-pct-zero}.json` | 거부 | 거부 | (계층 없음) | `state-field-injected` = 데이터 표에 현재 상태를 넣는 실수 |

`DEPOSIT_FIELD/invalid/no-deposits.json`·`position-two-components.json` 은 v2 모양으로 다시 썼고 `reserve-fractional.json` 은 `DEPOSIT_RESERVE` 로 옮겼다.

## 6. 역사 연결

| 이벤트 | 중요도 | 생성 조건 | 남는 증거 | 이어지는 플레이 |
|--------|:---:|------|------|------|
| `MINERAL_SOLD` (도메인) | **0 — 역사 아님** | 수락된 판매마다 | 자기 자신(`domain_events` 행) — 원장 | 지갑·인벤토리가 왜 그 숫자인지의 유일한 근거. **흔적은 역사가 아니라 시뮬레이션 상태로 남는다**: 눌린 시세(UI-05, 13~14 분), 마지막 거래 줄(UI-12, 다음 판매까지), 새 상장(UI-10, 영구). 늦게 온 사람이 그것을 보고 다른 광물로 간다(T-3). **p2**: payload 가 `MARKET_CRASH@1` 판정에 필요한 사실(기준가·바닥가·전후 가격·전후 포화·`pricing_version`)을 이미 싣는다 — Herald 기사, Biography 줄, "일부러 쏟았다" 는 시장 조작 Claim |
| `MINERAL_DISCOVERED` (기존, 역사) | 2 | p1-02 그대로 | p1-02 그대로 | **이번 슬라이스에서 처음 값을 가진다**: 발견 직후 시장에 새 상장이 생기고(시뮬레이션 — 드러남), 발견자가 그 광물의 첫 공급자가 된다(디자인 §4) |

**"다른 플레이어가 이 행동의 흔적을 발견할 수 있는가?"** — 예. 판매자가 떠난 뒤 접속한 플레이어가 그 판매의 결과(시세·포화)와 판매자 표지(마지막 거래)를 본다. AC-19 가 A(판매자, 떠남 — 디스폰 단언)·B(뒤에 첫 접속, 별도 프로세스)로 잰다. 흔적이 역사가 아닌 이유는 §3 표다: 같은 것을 역사로 한 번 더 쓰면 "기록이 새 플레이를 만든다" 가 아니라 "같은 것을 두 번 보여준다" 가 된다(디자인 §5.2).

**Fact / Claim / Interpretation (원칙 6)** — 이번에 테이블 변화 없음. 시장 상태는 사실(시뮬레이션 상태)이고, "누가 시장을 망쳤다" 는 플레이어 머리 안의 해석이며 게임은 그 문장을 만들지 않는다.

## 7. 수용 기준

모든 Then 은 실행 증거다. 실행하지 못한 항목은 PASS 가 아니라 **"미검증(환경)"** 이다. **각 항목의 ⊘ 는 그 항목을 자명하게 통과시키는 상태이고, 항목은 그 상태를 같은 실행에서 배제하는 단언을 포함한다.** 기대 숫자는 `contracts/fixtures/` 와 디자인 §3.6·부록 B(테스트 출력과 독립된 출처)에서 오고, 판정 도구는 같은 수를 출력에서 베끼지 않는다(qa-throughput R4). history 의 H-30~H-34 가 AC-12 의 세부 절차이고 designer 의 T-1~T-12 가 AC-3·AC-4·AC-18~AC-21 의 세부 절차다 — qa 가 스프린트 계약에서 항목으로 펼친다.

### 서버

- **AC-1 (server) 게이트와 순수성.** `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace --locked` 종료 코드 0. `starfall-sim`·`starfall-history` 의 금지 의존(p1-02 AC-1) 그대로. **`starfall-history` 소스에 diff 0**(이번 슬라이스는 역사 코드를 바꾸지 않는다 — `git diff --stat` 출력). ⊘ *검사 대상 크레이트가 목록에 없음* → 검사한 크레이트 이름을 찍는다.
- **AC-2 (server) 데이터 표 검증.** (a) 정상 `data/` 로 기동하면 `/debug/stats`(또는 기동 로그)에 시장 1(id·`pricing_version`·상장 4), 광맥 8·예비 8, 광물 4 가 나오고 파일과 일치한다. (b) §4.8 의 조건 **각각**을 하나씩 깨뜨린 `STARFALL_DATA_DIR` 에서 전부 기동 실패이고 로그에 파일·필드가 나온다 — 광맥 짝 불일치는 **양방향**(공개부에만·예비에만 있는 광맥) 각각, `DEPOSIT_FIELD` v1 파일 하나. ⊘ *기동 실패 원인이 주입한 결함이 아님* → 각 경우의 로그에서 주입한 필드 이름을 단언하고, 같은 디렉터리의 결함을 되돌리면 기동함을 짝으로 보인다(최소 1 경우).
- **AC-3 (server) 판매 판정** (디자인 T-1·T-5 의 단위 판). 수동 step 통합 테스트(소켓·DB 없이). (a) 모든 조건 충족 → 수락: Ferrosite 2,000 kg @포화 0 → 인벤토리 −2,000, 지갑 +20,000, 포화 2,000, 시세 10 → 9, `MINERAL_SOLD` 1건의 모든 필드, `causation_id = command_id`, 마지막 거래 = 그 actor·2,000 kg·그 tick. (b) §4.2 표의 사유 **2~7 각각**(2 는 없는 시장과 상장 없는 광물 **둘 다**, 7 은 지갑을 `2^53−1 − 대금 + 1` 로 주입)에서 거부되고 **인벤토리·지갑·포화·마지막 거래·판매 쿨다운·처리 장부가 바뀌지 않는다**, `commands_rejected_total{사유}` 가 각각 +1. (c) 두 사유가 동시에 성립(쿨다운 중 + 도킹 밖)하면 표 순서의 앞 사유. (d) 거부 직후 tick 에 조건을 만족시키면 **판매 쿨다운에 막히지 않는다**. (e) **경계 양쪽**: 보유량 정확히 → 수락, 인벤토리 행 0(지워지지 않음), `INVENTORY_STATE` 에 그 항목 없음 / 보유량 + 1 → `INSUFFICIENT_QUANTITY`. (f) 같은 tick 두 판매가 **제출 순서**대로 — 20,000 / 18,000, 제출 순서를 바꾸면 결과가 바뀐다. (g) 같은 tick 의 채굴과 판매가 제출 순번으로 섞여 처리된다. (h) 판매 쿨다운: t 에 수락 → t+19 `COOLDOWN_ACTIVE` → t+20 수락, 그리고 판매 쿨다운이 **채굴 쿨다운과 독립**(판매 직후 채굴 수락, 채굴 직후 판매 수락). ⊘ *모든 경우가 거부로 끝남* → (a) 의 수락 수 ≥ 1 출력.
- **AC-4 (server) 가격 규칙** (순수 함수, I-92). (a) 디자인 §3.6·부록 B 의 대금 표(Ferrosite 12,000 → 90,000 / 14,000 → 98,000, Glacine 10,000 → 96,500, Cobaltine 3,200 → 100,000, Starfall Glass 500 → 100,000·700 → 122,500, Ferrosite 2,000 → 20,000 이어서 2,000 → 18,000)를 **디자인 문서에서 옮긴 상수**로 단언. (b) **분할 무관**: 같은 포화에서 무작위 분할 N 개(시드 출력, 구간 경계를 걸치는 분할 포함)의 대금 합 = 한 번 판매. (c) 단조. (d) 유계·바닥: 포화 상한을 넘는 양을 팔면 남은 양 전부가 바닥값, 포화는 상한. (e) 회복: **식(I-87)과 tick 별 시뮬레이션이 같은 값**(바닥 → 1,200 tick 뒤 13,000 → 시세 4 — 디자인 T-8). (f) 대금 ≥ 판매량 ≥ 1. (g) 반복 수: `quantity_kg = 2^31−1` 판매의 구간 반복이 `floor_band + 1` 이하(카운터 출력) — kg 루프가 아님. (h) **golden**: `pricing_version` 이 가격 절 정규화 해시 + 고정 입력 대금 표에 묶여 있고, **가격 값 하나만 바꾼 시장 파일이 실제로 실패한다**(음성 대조). ⊘ *대금이 전부 0* → (a) 가 양성 대조.
- **AC-5 (server) 복사 경로 — 같은 `command_id` 는 한 번.** 경로마다 **첫 전송이 `ACCEPTED` 였고 지갑이 늘었음**을 먼저 단언(⊘ *첫 전송부터 거부*). (a) 한 세션 2회. (b) 잔류 창 안 재접속 뒤. (c) **서버 정상 종료(stdin `shutdown`) → 재기동 → 같은 actor → 같은 명령** → `DUPLICATE_COMMAND_ID`, 지갑·인벤토리·`processed_commands`·`market_states` 행 불변. (d) 같은 actor 의 두 연결이 같은 tick 에 → 수락 1·중복 1. (d2) 다른 actor 가 같은 키 → 수락 1·중복 1·서버 생존. (e) **명령 종류 교차**: `MINE_RESOURCE` 로 수락된 키를 `SELL_MINERAL` 로, 그리고 판매로 수락된 키를 채굴로 → 둘 다 `DUPLICATE_COMMAND_ID`(I-82). (f) sim 기억을 우회해 중복 판매가 DB 에 도달 → **PK 위반 → 정지**, 지갑 불변. (g) 세션 안 거절 명령 재전송 → `DUPLICATE_COMMAND_ID`. 각 경로의 최종 지갑이 **대금 × 1**, 인벤토리 감소가 **× 1**.
- **AC-6 (server) 원장·보존·포화 항등식과 비교 후 쓰기.** (a) 봇 2 이상이 채굴·판매를 반복한 새 월드에서 I-80·I-81·I-84·I-85 가 SQL(+ 재계산 도구)로 성립. **판매한 actor ≥ 2, 판매 ≥ 10, 판매된 광물 ≥ 2, 구간을 걸친 판매 ≥ 1, 두 판매 사이에 회복이 끼어든 쌍 ≥ 1 이 아니면 FAIL** — 각 수를 출력. (b) 서버가 떠 있는 동안 한 actor 의 **지갑 행**을 SQL 로 바꾼 뒤 그 actor 가 판매 → **정지**: 0 아닌 종료 코드, 로그에 비교 후 쓰기 실패와 키, `persist_fatal_total ≥ 1`, 그 tick 의 `MINERAL_SOLD` 가 DB 에 없다. **`market_states` 행**에 대해 같은 것 1 회. ⊘ *다른 이유로 죽음* → 변조 전 같은 actor 의 판매가 수락·커밋됐음과 종료 로그의 사유를 단언. (c) 재기동하면 변조된 값으로 적재되고 다음 판매가 그 위에서 수락된다 — DB 가 정본. (d) 상태 표에 DELETE 경로가 없다: 전량 판매 뒤 `inventory_items` 행이 `quantity_kg = 0` 으로 존재(행 수 출력).
- **AC-7 (server) 기록 지연 거부.** p1-02 AC-6 을 판매로: DB 정지 중 `recording_lag > 20` 뒤의 `SELL_MINERAL` 은 `RECORDING_BACKLOG`, 같은 구간 `SET_SHIP_CONTROL` 수락, DB 재기동 뒤 판매 수락, 정지 전·중 수락된 판매 전부가 DB 에 있고 항등식 성립. ⊘ *DB 정지 동안 판매를 안 보냄* → 거부 수 ≥ 1 과 정지 전 판매 수락 ≥ 1 을 함께. *(`stop` 은 볼륨을 지우지 않는다 — `down -v` 금지)*
- **AC-8 (server) 결정성.** (a) 판매를 포함한 입력열(함선 2척, 채굴·판매, 쿨다운·도킹 밖·속도 초과·보유 부족, 구간 걸침, 회복 배수 경계)을 **서로 다른 프로세스에서 2회** 재생 → 매 tick 의 이벤트 내용(`event_id`·`recorded_at` 제외)과 `SALE_RECEIPT`·`INVENTORY_STATE`·`WALLET_STATE`·`MARKET_STATE` payload 가 바이트 동일, 비교한 `MINERAL_SOLD` 수(≥ 1) 출력. (b) **이동·채굴 golden 과 역사 golden 이 재생성 없이 통과** — 파일 sha256 을 작업 전후로 찍는다. ⊘ *golden 을 BLESS 로 다시 만듦* → 해시 불변이 판정.
- **AC-9 (server) 계약 테스트.** `cargo test -p starfall-contracts --locked` 가 타입 30 을 덮는다: 유효 **57** 왕복, 반례 **113** 거부, 반례의 serde 결과가 §5.3 과 일치(신규 29 typed 의 거부 사유가 의도한 위반을 가리킴), `required` 제거 변이 실패, 레지스트리 `responses` 가 명령 4종에 있고 `data_files` 가 data 9종에 있다. 검사 건수 출력.
- **AC-10 (server) 누출 없음과 상장.** 새 월드에서 (a) 첫 채굴 전 모든 세션이 받은 `MARKET_STATE` 의 `listings` 가 비어 있다. (b) **양성 대조**: Ferrosite 첫 채굴 tick 에 `MARKET_STATE` 가 월드 전원에게 나가고 `ferrosite` 가 나타난다, 다른 미드러난 광물은 여전히 없다(디자인 T-10 — Starfall Glass 로 같은 것을 한 번 더). (c) 세션 시작 송신 순서가 §5.1a 그대로.
- **AC-11 (server) 시장 상태의 송신과 영속.** (a) 판매 tick 에 열린 모든 세션이 `MARKET_STATE` 1건을 받는다(세션 수 = 수신 수, 세션 ≥ 2). (b) 회복 배수 tick: 실효 포화가 바뀐 상장이 있으면 1건, 전부 0 이면 0건 — **두 경우를 같은 실행에서 각각 관찰**(관찰 수 출력). (c) 회복 tick 에 `market_states`·`domain_events` 행 수가 변하지 않는다(I-87). (d) 정상 재기동 → 포화·마지막 거래 보존, 재기동 뒤 첫 배수 tick 에 식대로 회복(디자인 T-8).

### 역사

- **AC-12 (history) 러너 통과** — H-30~H-33(`02h_history_review.md` §2.3). (a) H-30: 채굴 → 판매 3 → 다른 광물 채굴 구간에서 커서가 마지막 판매 뒤로, `MINERAL_DISCOVERED` 2(판매 **뒤**의 판정이 양성 대조), `historical_event_sources` 에 `MINERAL_SOLD` id 0(분모 3), 정지·충돌 카운터 0. (b) H-31: 판매만 있는 구간에서도 커서 전진. (c) H-32: `schema_version = 2` 인 `MINERAL_SOLD` 가 러너를 멈추지 않는다 — 짝: 같은 자리의 `MINERAL_MINED` v2 는 정지(p1-02 AC-11(e)). (d) H-33: `history-replay` 일치, 재생 행 중 `MINERAL_SOLD` ≥ 1(0 이면 FAIL). ⊘ 각 항목의 "`MINERAL_SOLD` 행이 실제로 커밋됨" 선단언.

### 데이터 배포

- **AC-13 (client·qa) 사본 범위** (I-91, ADR-0015). (a) SC-50 개정: 레지스트리에서 유도한 기대 집합과 사본 집합을 **독립 순회 두 번**으로 비교 — 사본 전용 0, 누락 0, 바이트 동일, 기대 집합 크기 출력(> 0, 아니면 FAIL). (b) 사본에 `consumers` 에 `client` 가 없는 종류의 파일 0 — 특히 `world/deposit-reserves/`·`history/rules/` 가 없다. **음성 대조**: 사본에 `deposit-reserves/cradle.json` 을 임시로 넣으면 (a)(b) 가 실패한다. (c) `data/**/*.json` 각각이 정확히 하나의 data 종류 glob 에 걸린다(분모 = 파일 수 출력, 0 개·2 개 걸림 = FAIL). (d) `validate_data_files.py` 가 레지스트리 `data_files` 에서 대상을 유도하고 새 파일(시장·예비) 포함 전부 0 오류. 음성 대조: 시장 파일 기준가를 소수로 바꾸면 1 오류.

### 클라이언트

- **AC-14 (client) 생성기·EditMode 계약.** (a) 신규 DTO 5 생성, 2회 멱등, 기존 DTO 바이트 동일(§5.2 실측 재현). (b) 유효 fixture 발견 수·왕복 수를 상수로(예측 57/45 — 실측으로 확정), §5.3 C# 열을 실측으로 채워 상수로, `Runtime` 프로필에서 모르는 `reason_code` 에 죽지 않음. (c) `MARKET_STATE` 의 `last_sale` null·비-null 왕복.
- **AC-15 (client) 화면 요소 — 디자인 §8 의 UI-01~UI-13 각각** (screen-elements S2: 요소마다 ① 표시 로직 EditMode 테스트 ② PlayMode 존재 단언 ③ 사람 체크 한 줄 — **사람 관찰 하나로 뭉치지 않는다**). 표시 로직의 기대값은 디자인 §8 의 문구·규칙과 §3.6 숫자에서 독립적으로 정한다.

  | UI | ① 표시 로직(EditMode — 순수 함수) | ② 존재 단언(PlayMode) — 분모 |
  |---|---|---|
  | UI-01 거래소 3D 표식 | 위치·반지름·도킹 구역 반지름(320)을 시장 표에서; 도킹 표시 두 ● 초록이면 구역 색 변경 | 표식 GameObject 정확히 1(= 시장 수) |
  | UI-02 거리 라벨 + 화면 밖 방향 | 표면 거리 문구(`4.2 km`, < 2 km 면 m), 화면 밖·뒤일 때 가장자리 화살표 판정 | 라벨 1, 화면 밖 지시자 존재 |
  | UI-03 도킹 판정 표시 | 거리·속도 ● 색(경계값 320/10 의 양쪽), 2 km 밖 숨김 | HUD 행 존재 |
  | UI-04 크레딧 잔액 | `₵ 20,000` 천 단위 구분, 0 이면 `₵ 0`, **`WALLET_STATE` 로만 바뀜** | 세션 시작 직후 존재 |
  | UI-05 시세 티커 | 행 문구(`Ferrosite  3 cr/kg ▼70%`), 상장 0 이면 안내 문구, 행 순서 | 행 수 = 받은 `listings` 수 |
  | UI-06 미상장 비공개 | `미상장 N종` = 광물 수 − 상장 수, 미상장 광물 이름이 어떤 행에도 없음 | 문구 존재 |
  | UI-07 시장 패널 | 열림 = 두 표시 조건 참, 행 내용(시세·포화 막대·`이 값으로 N kg 더`/`바닥가`·보유량), 보유 0 흐림, ↑↓ 선택 이동 | 도킹 표시 참일 때 패널 활성, 거짓일 때 비활성 |
  | UI-08 프리셋 + 예상 대금 | "이 값까지" 양 = `(band+1)×band_kg − s`, 바닥이면 프리셋 하나, **예상 대금 = 디자인 §3.3 함수**(AC-4(a) 와 같은 상수표), 보유 0 이면 `팔 것 없음`·Enter 무효 | ←→ 입력이 프리셋을 바꾼다 |
  | UI-09 판매 결과 알림 | 문구 = `SALE_RECEIPT` 의 실제 값(예상과 다른 입력에서 실제를 쓴다), 4 s. **둘째 줄**: 예상 > 실제면 `예상보다 ₵N 적다 — 직전에 다른 판매가 있었다`(N = 차이), 예상 ≤ 실제면 둘째 줄 없음 — 두 경우 각각 단언(디자인 §2.1a, `min_proceeds` 를 두지 않은 대신) | `SALE_RECEIPT` 주입 → 알림 생성 |
  | UI-10 상장 알림 | 직전 메시지에 없던 광물만, 세션 첫 메시지는 알림 없음 | `MARKET_STATE` 두 번 주입 → 알림 1 |
  | UI-11 거절 사유 | 사유별 문장(사거리·속도·보유·쿨다운·기타는 코드 그대로·모르는 코드) | 거절 결과 주입 → 줄 생성 |
  | UI-12 마지막 거래 줄 | `Pilot-xxxx`(I-66 규칙)·kg·경과 시간, 내 것이면 `나`, null 이면 `거래 없음` | 행마다 줄 존재 |
| UI-13 발견자 표지(역사 → 시장) | 표지는 `MINERAL_DISCOVERED` NOTICE(BACKFILL·LIVE)에서만 — `MARKET_STATE` 에서 읽지 않는다(같은 입력에 `last_sale` 만 바꿔도 표지 불변), 상장됐는데 NOTICE 가 없으면 `발견: 기록 중…`, NOTICE 도착 뒤 `발견: Pilot-xxxx`, 내가 발견자면 `발견: 나`, 한 번 붙으면 사라지지 않음 | 상장 행 N + 발견 NOTICE N 주입 → 표지 N. ⊘ 두 가지(history3 S-2): 상장을 NOTICE 보다 **먼저** 주입해 `발견: 기록 중…` 상태를 실제로 거쳤음을 단언한 뒤 표지로 바뀜을 단언, 그리고 재접속(새 세션)에서 **BACKFILL** 로만 받은 NOTICE 로도 표지가 붙음 |

  ③ 사람 체크 문장은 디자인 §8.3 끝의 **13 문장**을 그대로 쓴다. ⊘ *요소가 0 개여도 통과* → ② 의 분모(기대 개수)를 단언한다.
- **AC-16 (client) 클라이언트는 계산하지 않는다** (I-76·I-77·I-78). (a) 인벤토리·잔액·현재 시세가 서버 메시지 수신에서만 바뀐다 — EditMode: 예상 대금을 계산한 뒤에도 표시 잔액·보유량 불변, `WALLET_STATE`·`INVENTORY_STATE` 주입 뒤에만 바뀜. (b) 소스 검사(부정 증명 — 짝 (a) 와 함께만 PASS): 잔액·보유량에 더하거나 빼는 경로 없음.
- **AC-17 (client) 키.** ↑↓←→ Enter 는 시장 패널이 열려 있을 때만 명령을 만든다(닫혀 있을 때 Enter → 송신 0, 열려 있을 때 → `SELL_MINERAL` 1). 기존 바인딩(디자인 §8.1 표)과 겹치지 않음을 바인딩 목록 테스트로.

### QA

- **AC-18 (qa) 치트 전수** (디자인 T-5·T-6). 시도 수와 막은 수. (a) 가격·대금·잔액·위치·actor 주입, `quantity_kg` 0·1.5 → 전부 `MALFORMED_COMMAND`, 상태 불변. (b) 도킹 밖 500 m → `TARGET_OUT_OF_RANGE`. (c) 30 m/s 통과 중 → `SHIP_TOO_FAST`. (d) 보유 + 1 → `INSUFFICIENT_QUANTITY`. (e) 성공 직후 0.5 s → `COOLDOWN_ACTIVE`, 연결 유지. (f) 없는 시장·없는 광물 → `TARGET_UNKNOWN`. (g) AC-5 (a)(b)(c)(e) 를 봇으로. **사유별 관찰 수 > 0 출력** — 0 인 사유가 있으면 그 경로를 안 탄 실행. 그동안 지갑·인벤토리·포화·`MINERAL_SOLD` 행 수 불변.
- **AC-19 (qa) "B 가 흔적을 본다" — 슬라이스의 핵심** (디자인 T-3). 새 월드. (a) A 가 Ferrosite 14,000 kg 을 한 번에 판다 → 바닥(`price_after = 3`). (b) A 가 접속을 끊고 **A 의 함선 디스폰을 단언**한 뒤, **A 의 판매 tick 부터 1,200 tick 안에** B 가 처음 접속. (c) B 가 받은 첫 `MARKET_STATE` 에서 Ferrosite 시세 < 기준가, `last_sale.actor_id = A`, `quantity_kg = 14,000`. (d) 봇 정책 기준선: B 의 첫 판매 광물(사람 세션 비교용, 판정 아님). ⊘ *A 가 아직 월드에 있음* → 디스폰 이벤트 선단언. ⊘ *B 가 A 와 같은 프로세스라 기억으로 앎* → B 는 별도 프로세스, 판정 입력은 B 가 받은 메시지뿐. ⊘ *회복이 이미 기준가로 돌려놓음* → (b) 의 tick 차이 출력.
- **AC-20 (qa) 경쟁과 부하** (T-4·T-11, H-34). (a) 두 봇이 같은 목표 tick 에 Ferrosite 2,000 kg 판매 → **두 `MINERAL_SOLD` 가 같은 tick 임을 먼저 단언**(아니면 무효), `sequence` 작은 쪽 20,000·큰 쪽 18,000, 두 지갑 합 = 두 대금 합. (b) 31 연결(봇 30 + Unity 1 또는 봇 31)이 채굴·운반·판매 20 분: tick 초과 비율 ≤ 0.5 %, 항등식 전부 성립, `RECORDING_BACKLOG` 거부 0, **바닥에 닿은 광물 ≥ 1**, 회복이 관찰된 광물 ≥ 1, 판매 수·광물별 판매량 출력, 역사 알림 지연 p95(H-34 — 거래가 섞인 상태, 기록만, 실패 경계 5 s)와 영속화 커밋 p50/p99 를 p1-02 기준선과 나란히. 도달 가능성(R4): 초기 매장 Ferrosite 33 t ≥ 포화 상한 14 t 이므로 20 분 30 봇에서 바닥 도달은 공급상 가능하다 — 계약에서 qa 가 다시 계산한다.
- **AC-21 (qa) 기록 무결성.** (a) 이번 실행 tick 구간 `event_type` distinct = `SESSION_*`·`SHIP_*`·`MINERAL_MINED`·`MINERAL_SOLD`. (b) `MINERAL_MINED`·`MINERAL_SOLD` 의 `causation_id` 와 `processed_commands.command_id` 가 양방향 누락 0, 그리고 `command_type` 이 이벤트 타입과 맞는다(SELL ↔ SOLD). (c) `(world_id, tick)` 별 `sequence` 빈틈 없음. (d) 자기 참조 결함 장부 7 건 그대로. (e) **제약 census**: `pg_constraint` 실측이 ADR-0013 §5a + ADR-0016 §5 의 요구 집합(경제 72 + 역사 50 = 122, 트리거 4)과 같다 — 다르면 FAIL. ⊘ (b) *둘 다 0 행* → 행 수 출력, 0 이면 FAIL.
- **AC-22 (qa) 계약 커버리지·CI.** (a) `check_contract_coverage.py --strict` 종료 코드 0(기준선 errors 17). (b) 신규 타입의 스키마·Rust·C# 필드별 표, 불일치 0. (c) 읽히지 않는 데이터 파일 0 — `data/**/*.json` 각 파일마다 읽는 코드와 게이트의 짝(분모 출력). (d) `gates.yml` 출처 게이트가 p1-03 계약을 가리키고 분모가 p1-03 계약 항목 수. (e) Unity EditMode 는 CI 밖이므로 병합 조건으로 사람이 돌린 결과 파일을 증거에 남긴다.
- **AC-23 (리더) 사람 사전 세션** (screen-elements S4). 화면 요소 모듈 완료 뒤, **r1 동결 전에** `unity_session.py start --slice p1-03-trading --new-world` 로 15 분 세션. UI-01~UI-13 체크 문장 + "거래소까지 몇 초 만에 찾았는가"(목표 ≤ 60 s, 가정) + T-3 의 B 질문. "아니오" 는 r1 전 결함 목록. 판정은 r1 이후 최종 세션 — 입력은 요소별 예/아니오 + 서버 기록(스크린샷 선택).

## 8. 비기능 요구

- **동시 연결 31** 유지(원칙 8). 판매는 쿨다운 1 s 이지만 실제 빈도는 운반 왕복(수십 초)에 묶인다.
- **tick 본문**: 판매 판정 O(1) + 대금 O(구간 수 ≤ 8). 회복은 게으른 식이라 비용 없음. tick 초과 기준 ≤ 0.5 %(p1-01 그대로).
- **메시지**: `MARKET_STATE` 는 상장 수(≤ 4)에 비례, 1 KiB 미만 × (판매 tick + 회복 tick ≤ 분당 1) × 31 세션. 세션 시작 메시지 5 건 + BACKFILL(≤ 4).
- **손실 창**: 크래시 시 잃는 판매 ≤ `RECORDING_BACKLOG` 임계(1 초). 잃을 때 인벤토리·지갑·시장·이벤트·장부가 **함께** 사라진다 — 반쪽 거래는 없다(ADR-0013 §6).
- **WebGL**: 영향 없음(PC 전용 유지).

## 9. 열린 질문

### 9.1 사용자 결정 (2026-10-05) — **Q1~Q3 전부 추천안으로 결정됨**

아래 표는 결정 당시의 질문과 추천이다(원칙 5의 정신 — 결정 근거를 덮어쓰지 않는다). 결정: Q1 크레딧 사용처 **넣지 않는다**, Q2 적재 상한 **넣지 않는다**, Q3 마지막 거래 줄에 판매자 표지를 **보인다**.

| # | 질문 | 추천 | 다른 답이면 |
|---|------|------|-----------|
| Q1 | 크레딧 사용처를 이번에 넣는가 | **넣지 않는다** — "번다" 까지. 사용처의 질문은 "무엇을 사는가" 이고 함선 모듈·현상금과 함께 정해야 한다 | 차감 경로·잔액 부족 사유·지갑 하한 ② 가 들어오고 화폐 총량 항등식에 항이 붙는다 |
| Q2 | 적재 상한(Scout 20 t) | **넣지 않는다** — 운반 압력은 시장 포화가 이미 만든다. 넣으면 `MINE_RESOURCE` 판정을 다시 연다 | p1-02 계약·테스트 재개방 |
| Q3 | 마지막 거래 줄에 판매자 표지(`Pilot-xxxx`)를 모두에게 보이는가 | **보인다** — 거래소의 공개 게시판, 흔적이 "누가" 를 가져야 이야기가 된다 | 익명 — Fog of History(p2)의 첫 적용, `last_sale.actor_id` 를 빼는 계약 변경 |

### 9.2 리더 판단으로 받을 것 (architect 결정, 보고)

- **ADR-0015 의 구현을 이번 슬라이스에 넣는다.** p1-02 만기("p1-03 착수 전 ADR accepted")는 지났다 — `00_request.md` 생성 시점에 0015 가 없었으므로 **비-통과**로 기록하고, ADR 은 Phase 2 안에서 썼다. 닫힘 조건(사본의 서버 전용 필드 0)은 AC-13 이 잰다. 추가 작업: 광맥 표 분할(designer, 값 불변), 서버 적재 변경, 클라이언트 사본·SC-50 개정, 검증 도구 유도.
- 시장 표는 공개 종류(§4.7) — 디자인의 "누출 수용" 문장을 대체.
- `SELL_MINERAL` 에 `market_id` 를 둔다(디자인 초안에 없음 — 시장이 둘이 될 때 schema_version 인상을 피한다).
- 실제 대금 전달은 `SALE_RECEIPT`(디자인 Q5 — `COMMAND_RESULT` 모양은 그대로).
- 판매 쿨다운 1 s 를 둔다(디자인 §2.1 — 연타의 이익은 없지만 tick 마다 DB 행을 만드는 스팸을 막는다).

### 9.3 남은 확인 (차단 없음)

- `data/market/cradle-exchange.json` 작성·광맥 표 분할 — designer3(값은 디자인 §1.3·§3.6), architect 가 스키마 검증으로 확인.
- 층별 거부 표(Rust serde·C#)는 구현 중 실측으로 채운다.
- ~~ADR-0016 은 server 검토 뒤 accepted~~ — **accepted 2026-10-05**(server3·serverdb3 T0 동의, `02_server_ack.md`).

## 10. QA 가 알아야 할 환경 사실

p1-01 §11 의 1~8, p1-02 §10 의 9~13 은 계속 참이다. 추가:

| # | 조건 | 판정을 어떻게 무효로 만드나 | 방어 |
|---|------|------|------|
| 14 | **시장 포화·마지막 거래·지갑은 월드마다 영속한다** (I-88, §4.9) | 두 번째 실행부터 시세가 기준가에서 시작하지 않아 기대 대금이 틀린다 — 환경 때문의 빨간불, 또는 이전 실행의 마지막 거래를 이번 실행의 흔적으로 오독한 초록불 | 시세·흔적을 재는 항목은 새 월드. 기본 월드면 시작 `MARKET_STATE` 를 먼저 읽어 기대값을 계산 |
| 15 | **회복은 월드 tick 이 1,200 의 배수일 때다** | "판매 1 분 뒤" 로 손계산하면 배수 경계가 어긋난다(같은 1 분 안에 0 회 또는 1 회) | 기대값은 식(I-87)에 **실제 tick** 을 넣어 계산 |
| 16 | **상장은 드러남을 따른다** — 새 월드에서 그 광물을 한 번 캐기 전엔 시장에 없다 | 판매 시나리오가 "시장에 없음" 으로 실패 | 시나리오 전제: 판매 전 그 광물 채굴 1 회(디자인 §6 전제) |
| 17 | **p1-02 의 질량 보존 도구(`mining_ledger.py`)는 판매 항을 모른다** | 판매가 있는 월드에서 거짓 FAIL | I-84 로 갱신(판매 항) — 갱신 전 결과는 판정에 쓰지 않는다 |
| 18 | 판매 쿨다운은 메모리 상태다 | 재기동 직후의 즉시 판매를 쿨다운 위반으로 오독 | 재기동이 쿨다운을 초기화하는 것은 설계(§3 제외) |

## 변경 기록

| 날짜 | 변경 | 이유 |
|------|------|------|
| 2026-10-06 | §5.3 C# 열 실측 확정: 거부 51 / 감지 불가 35 / 계층 없음 27 = 113(예측 53/33/27 — 소수 크레딧 2 건이 `long` 필드라 감지 불가) | client4 C1 EditMode 실측 |
| 2026-10-06 | **최종**: 리더가 스키마 제목 수정(`MarketLastSale`)을 채택하고 "스키마를 바꾸지 마라" 를 취소(메시지 엇갈림). 제목을 다시 `MarketLastSale` 로. 생성기는 충돌 감지 실패 검사만 추가(client4), ADR-0002 §4 에 규칙 한 줄 | 리더 결정 |
| 2026-10-06 | §5.1·§5.2 생성기 실측 정정: "수정 없이 동작" → "생성은 됐으나 C# 컴파일에서 중첩 이름 충돌 1건(CS0102, `MARKET_STATE` `LastSale`) → 생성기 수정". 한때 계약 제목을 `MarketLastSale` 로 바꿨으나 **되돌렸다** — 리더 결정: 계약은 그대로, client4 가 생성기에서 고친다 | client4 C1 발견, 리더 결정 |
| 2026-10-05 | §4.8 정리(qa3 Q-1): 스키마가 보장해 실패할 수 없는 검산 셋(정수 초 × tick_hz 둘, 대금 상한 부등식)을 기동 검산에서 빼고 "계약이 보장하는 것" 으로 옮김. 계약 `market.schema.json` 설명 동기화 | qa3 `02_sprint_contract.md` §11 Q-1 |
| 2026-10-05 | **agreed** — 사용자 결정 Q1(크레딧 사용처 없음)·Q2(적재 상한 없음)·Q3(판매자 표지 공개) 전부 추천안, §9.1 에 기록 | 리더 전달 |
| 2026-10-05 | history3 S-2·S-3: UI-13 존재 단언의 ⊘ 둘(`기록 중…` 경유, BACKFILL 경로), §3 `MARKET_CRASH` 행에 한 판매 조건의 결함과 p2 기본값 | history3 `02h_history_review.md` §4 |
| 2026-10-05 | **리뷰 반영**: `MINERAL_SOLD` 에 `floor_price_cr_per_kg`(필수) + 반례 1 → 반례 113(history3 C-1). UI-13 발견자 표지를 §4.5·AC-15 에 행으로(유일한 역사 → 플레이 연결 — 빠지면 p1-02 의 "요소 없음" 재발), UI-09 둘째 줄(예상 > 실제) 단언(designer3 A·B) | history3 §3.2, designer3 스펙 검토 |
| 2026-10-05 | 최초 작성 (draft). ADR-0015(데이터 배포, accepted — p1-02 만기 부채)·ADR-0016(크레딧과 판매 장부, proposed) 신설. 계약: 타입 23 → 30, 유효 57·반례 112 실측, 생성기 무수정 동작 실측, 커버리지 기준선 errors 17. designer3 디자인(구간형 포화 + 회복, 사기 제외, 판정 순서, UI-01~12) 채택 + `market_id`·`SALE_RECEIPT`·`pricing_version`·시장 표 공개 종류. history3 검토(새 역사 타입 없음, R-1~R-9, H-30~H-34) 반영 | p1-03 Phase 2 |

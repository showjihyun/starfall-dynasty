# p1-02-mining: 채굴, 인벤토리, 그리고 첫 역사

- 상태: **implemented** (2026-10-05 마감 — r3 PASS 112 / FAIL 0, SC-98 CI 첫 실행 PASS(PR #5), SC-68 사람 세션 2차 확인. 사용자 결정 Q1~Q4·리더 판단 Q5·Q6, designer 설계·history 검토 반영)
- 로드맵 Phase: p1 (코어 프로토타입) — 둘째 슬라이스
- 근거 기획안 절: GDD §7(Resource System), GDD §8(Exploration → Historical Discovery), GDD §33(서버 판정 8단계), GDD §36(MVP-1, **핵심 테스트: "다른 플레이어가 그 행동의 흔적을 발견할 수 있는가"**), GDD §46(10,000 광물 경고), TECH §19(Transactional Outbox), TECH §26–27(경제 — 화폐 복사가 최악의 버그), HSE §9–13(Historical Event·중요도·스키마·Lifecycle), HSE §22(Truth 분리), HSE §28–34(멱등·순서·결정성·규칙 버전), HSE §88–91(MVP 범위·10종·Vertical Slice·테스트)
- 관련 ADR: **0013**(경제 상태 영속화·지속 멱등성 — 신규, proposed), **0014**(역사 엔진 골격 — 신규, accepted). 개정: 0006 §4(역사 판정 위치), 0007 §1(기록 범위 표에 `MINERAL_MINED`). 기반: 0006 §6, 0010 §4, 0011 §7, 0012 §7
- 입력: `docs/design/p1-02-mining-design.md`(designer — **수치·규칙·시나리오의 정본**), `_workspace/p1-02-mining/01_history_review.md`(history — 판정 규칙·테스트 H-01~H-15의 정본)
- **이 스펙은 수치를 정하지 않고 모양·한계·불변식만 정한다**(p1-01 과 같은 분업). 수치는 `data/` 와 디자인 문서에 있다.

## 1. 목표

이 슬라이스가 끝나면 플레이어는 **성계의 광맥을 찾아가 광물을 캐서 인벤토리에 쌓을 수 있고, 어떤 광물을 그 성계에서 처음 캔 플레이어가 역사로 남아 그 뒤에 온 모든 플레이어가 그것을 본다.**

p1 의 종료 기준은 *"핵심 행동이 서버 판정 + 역사 기록으로 동작"* 이다. p1-01 은 서버 판정은 했지만 역사가 없었다(이동은 역사가 아니다). 이 슬라이스가 증명할 것은 넷이다.

1. **첫 경제 상태**가 서버 권위로 바뀐다. 클라이언트는 "저 광맥을 캔다"는 의도만 보낸다.
2. **같은 명령은 한 번만 반영된다** — 세션 안, 재접속, 서버 재기동 어디서도. 인벤토리와 기록은 같은 커밋에 있다.
3. **역사 엔진이 처음 동작한다.** 커밋된 도메인 이벤트 → 중요도 판정(`rule_version`) → Historical Event + Evidence. 결정적·멱등·추가 전용.
4. **다른 플레이어가 그 흔적을 발견한다**(GDD §36). 발견 순간 접속해 있던 플레이어도, 발견자가 떠난 뒤 접속한 플레이어도.

designer 의 재미 가설(디자인 §0.1): *"내가 캔 것이 세상에 남는다"가 첫 채굴에서 바로 느껴지고, 나중에 온 사람이 그 흔적을 보고 움직이면 역사 엔진은 콘텐츠를 만든다.* 채굴은 재미의 원천이 아니라 **최초 발견이라는 사건을 만드는 가장 싼 행동**이다.

이것은 기능이면서 **바닥**이다. 여기서 정하는 것 — 상태와 기록의 원자성, 지속 멱등성, 역사 엔진의 입력·테이블·멱등·전달 — 위에 거래(p1-03)·전투(p1-04)·Chronicle(p2)이 올라간다.

## 2. 플레이 흐름

1. **개발자**가 서버를 띄운다. 서버는 p1-01 의 `data/` 에 더해 `data/minerals/*.json`(4)·`data/world/deposits/cradle.json`·`data/mining/mining-rules.json`·`data/history/rules/mineral-discovery.json` 을 읽어 스키마와 유도값(§4.7)을 검증한다. 어긋나면 **기동 거부**.
2. **서버**가 그 월드의 인벤토리·광맥 상태·처리한 명령 장부를 DB 에서 **적재한 뒤** tick 을 재개하고, 역사 러너가 판정 상태와 PUBLIC 기록 목록을 재구축해 게이트웨이에 넘긴 **뒤에** 연결을 받는다(ADR-0013 §7).
3. **플레이어 A**가 접속한다. `SESSION_READY` 뒤에 `INVENTORY_STATE`(자기 인벤토리), `DEPOSIT_FIELD_STATE`(광맥 8개의 동적 상태 — 새 월드에선 전부 **미확인**: 광물·매장량이 `null`), 그리고 그 월드에 역사가 있으면 `HISTORICAL_EVENT_NOTICE{BACKFILL}` 을 받는다. 광맥의 위치·반지름·이름은 클라이언트의 데이터 사본에서 읽는다 — **광물 종류는 사본에서 읽지 않는다.**
4. **A**가 미확인 광맥 Far Reach 로 날아가 속도를 줄이고 채굴 키를 누른다. 클라이언트는 `MINE_RESOURCE{deposit_id}` 하나를 보낸다 — **수량도, 광물 종류도, 위치도 보내지 않는다.**
5. **서버**의 tick 이 판정한다(§4.2 의 순서): 처리한 적 있는 `command_id` 인가 → 광맥이 있는가 → 쿨다운이 끝났는가 → **서버가 아는 함선 위치**에서 광맥 표면 사거리 안인가 → 함선이 충분히 느린가 → 잔량이 있는가. 통과하면 산출(결정적, 난수 없음)을 인벤토리에 더하고, 광맥 잔량을 줄이고, 광맥을 **드러내고**, `MINERAL_MINED` 를 발행한다. `causation_id` = 그 명령의 `command_id`.
6. **서버**가 A 에게 `COMMAND_RESULT{ACCEPTED}` → `INVENTORY_STATE` 를, 월드 전원에게 `DEPOSIT_FIELD_STATE`(Far Reach 가 이제 `Starfall Glass · 475 / 500 kg`)를 보낸다. 거부면 `COMMAND_RESULT{REJECTED, 사유}` 뿐이고 **아무것도 바뀌지 않는다 — 쿨다운도.**
7. **영속화**가 그 tick 의 이벤트 + 인벤토리 행 + 광맥 행 + 처리 장부 + `last_tick` 을 **한 트랜잭션**으로 커밋한다(ADR-0013 §2).
8. **역사 러너**가 커밋 알림을 받아 커밋된 `MINERAL_MINED` 를 `(tick, sequence)` 순으로 읽는다. 그 월드·성계에서 **Starfall Glass 의 첫 채굴**이므로 `MINERAL_DISCOVERED`(Level 2, `mineral-discovery@1`)와 자동 증거 1건을 만들고 커서와 함께 커밋한다.
9. **A 와, 그때 접속해 있던 B** 가 `HISTORICAL_EVENT_NOTICE{LIVE}` 를 받는다. A 의 화면에는 산출 알림과 **다른 모양의** 발견 배너가 "두 박자" 로 온다(디자인 §3.3): 첫 박자는 "캤다", 둘째 박자는 "세상이 기억했다". 문장은 클라이언트가 만든다 — DB 에 문장은 없다.
10. **B** 가 Far Reach 로 방향을 틀어 같은 광물을 캔다. 인벤토리는 늘지만 **역사는 생기지 않는다**(원칙 4). B 의 화면에는 `이 광물은 Pilot-3f9a 가 N분 전에 발견했다` — 받은 역사 기록과 자기 채굴 tick 으로 계산한 **표현**이지 새 사실이 아니다.
11. **A** 의 조작된 클라이언트가 같은 `command_id` 를 반복한다(또는 재접속 뒤 다시 보낸다). `DUPLICATE_COMMAND_ID`. 인벤토리는 한 번만 늘어 있다.
12. **A** 가 접속을 끊고, 잔류 30 초가 지나 A 의 함선이 디스폰된다. **플레이어 C** 가 그 뒤 처음 접속한다. BACKFILL 로 A 의 발견을, `DEPOSIT_FIELD_STATE` 로 드러난 Far Reach 를 받는다. **A 는 월드에 없는데 C 는 A 의 흔적을 본다.**
13. 30 분 뒤 Far Reach 는 월드 tick 배수마다 조금씩 회복한다. 광맥은 다시 차지만 **누가 처음 캤는지는 지워지지 않는다.**
14. **QA** 가 `psql` 로 확인한다: 인벤토리 = `MINERAL_MINED` 합, 광물별 보존 법칙, `MINERAL_DISCOVERED` 는 광물당 최대 1건이고 그 근거는 오라클의 최초 채굴, 증거 1건, 역사·증거 테이블은 UPDATE/DELETE 를 거부한다.

## 3. 범위

**포함**

- ADR-0013·0014 (신규), ADR-0006 §4·ADR-0007 §1 개정, 이 스펙, 작업 분해
- 계약(§5): 명령 `MINE_RESOURCE`, 도메인 이벤트 `MINERAL_MINED`, 역사 이벤트 `MINERAL_DISCOVERED` + 역사 envelope, 서버 메시지 `INVENTORY_STATE`·`DEPOSIT_FIELD_STATE`·`HISTORICAL_EVENT_NOTICE`, 데이터 `MINERAL`·`DEPOSIT_FIELD`·`MINING_RULES`·`SIGNIFICANCE_RULE`, `COMMAND_RESULT.reason_code` 6값 추가, primitives `UuidV5`·`MassKg`·`RuleVersion`, 레지스트리 `responses` 필드
- 서버: 데이터 4종 로딩·유도값 검산, 채굴 판정, actor 인벤토리와 광맥 상태(메모리 사본 + PG 정본), 게으른 회복, 드러남, 월드 범위 지속 중복 제거(세션 기억과 함께), tick 배치 트랜잭션 확장(상태 행·처리 장부·배치 멱등·비교 후 쓰기), 복구 불가 기록 실패 시 정지, `RECORDING_BACKLOG`, 기동 적재 순서, `INVENTORY_STATE`·`DEPOSIT_FIELD_STATE` 송신
- 역사: `starfall-history` 크레이트(순수 코어), `mineral-discovery@1`, golden, 러너(커서·트랜잭션·재구축·fail-stop), 역사 테이블 마이그레이션(추가 전용 트리거), `HISTORICAL_EVENT_NOTICE` 생산(LIVE·BACKFILL)
- 클라이언트(그레이박스, 디자인 §8): 광맥 표식(미확인/드러남·잔량·최초 발견자), 채굴 가능 표시(거리·속도·쿨다운 — 표시용, 판정은 서버), 인벤토리 패널(`INVENTORY_STATE` 만 반영), 거부 사유, 산출 알림과 발견 배너(두 종), 발견 목록(`N / 4`, 미발견 `M종`)
- QA: 봇 채굴, 복사 경로 전수, 원장·보존 항등식, 역사 오라클 교차, 역사 재생 결정성, 누출 검사, "C 가 흔적을 본다", 부하 회귀, CI 출처 게이트를 p1-02 계약으로

**제외 (다음 슬라이스로)**

- 거래·가격·시장(p1-03), 전투(p1-04). **인벤토리에서 빠져나가는 경로가 하나도 없다** — 그래서 원장 항등식이 "합 = 잔량" 으로 단순하다. 거래가 들어오면 항등식이 바뀐다
- 적재 상한·함선 화물칸 — 인벤토리는 actor 소유, 상한 없음(§9.1 Q4)
- Chronicle·Biography 화면, 역사 조회 REST API, 뉴스·LLM 요약(p2). 전달은 NOTICE 로만(ADR-0014 §6)
- Claim·Interpretation 테이블과 행동(p2 Historian) — **생산자가 없는 테이블은 만들지 않는다**(ADR-0014 §3)
- 공동 발견, 발견의 동적 중요도, 역사 인과 그래프, 발견이 같은 광물의 다른 광맥까지 드러내는 것(Consequence Engine 첫 경로 — 디자인 §9 Q3)
- 광맥 고갈·개인 첫 획득의 역사화(디자인 §4.2 — Level 0 유지)
- 절차적 광물 생성(GDD §7.2), 광물 속성 11종(읽는 코드가 없다), 산출의 무작위성
- 계정·표시 이름 — 발견자는 `actor_id` 에서 만든 짧은 표지로 보인다(§4.5 I-66)
- 데이터 배포 경로(클라이언트 사본에서 서버 전용 값 제거) — §9.1 Q3
- 클라이언트 재접속 재전송(ADR-0005 §5 유지), 쿨다운 영속화(재기동이 초기화한다 — 플레이어가 일으킬 수 없다), techart, WebGL

## 4. 규칙과 불변식

p0-01 I-1~I-9, p0-02 I-10~I-25, p1-01 I-26~I-47 은 계속 유효하다. 수치(사거리 150 m, 속도 10 m/s, 쿨다운 3 s = 60 tick, 산출·회복·매장량)는 `data/` 가 정본이다.

### 4.1 클라이언트 권위 부정 (원칙 1)

- **I-48 `MINE_RESOURCE` 의 payload 는 `deposit_id` 하나다.** 수량·광물 종류·위치·거리는 **어휘에 없다**(`additionalProperties: false` + `deny_unknown_fields` → `MALFORMED_COMMAND`). 광물은 서버가 광맥에서 유도하고, 거리·속도는 서버의 `f64` 함선 상태로 잰다. 명령에 대상 actor·인벤토리를 가리키는 필드도 없다 — 남의 인벤토리에 넣는 것은 시도할 방법이 없다.
- **I-73 모르는 거절 사유를 받은 클라이언트는 "알 수 없는 사유로 거절됨" 을 표시하고, 그 명령을 거절된 것으로 처리하며, 죽거나 수락으로 가정하지 않는다.** 생성된 C# 의 `ReasonCode` 는 `string` 이다(codegen 이 `enum` 을 문자열로 매핑 — architect 가 생성물에서 확인: `CommandResultMessage.cs` `public string ReasonCode`). 그래서 역직렬화 단계에서는 죽을 수 없고, 이 불변식이 거는 곳은 **UI·명령 대기 목록의 분기**다. `delivery`·`visibility`·`entity_kind`·`role`·`rarity` 도 같다: 모르는 `delivery` 의 NOTICE 는 표시하지 않고 로그를 남긴다.
- **I-49 클라이언트는 인벤토리를 계산하지 않는다.** 표시하는 수량은 언제나 마지막으로 받은 `INVENTORY_STATE` 다. 이동과 달리 인벤토리는 **틀렸다가 고쳐지는 것 자체가 버그로 보이는** 값이고, 왕복 한 번 늦는 비용이 작다. 채굴 가능 표시(거리·속도·쿨다운)도 **표시용**이고 판정하지 않는다.

### 4.2 채굴 판정 (GDD §33 서버 판정 8단계의 이번 구현)

- **I-50 판정 순서는 고정이고, 거부 사유는 그 순서의 첫 실패다.** 같은 상태·같은 명령이면 같은 사유가 나온다. 순서는 designer 가 정했다(디자인 §2.1).

  | # | 검사 | 실패 사유 | 비고 |
  |---|------|-----------|------|
  | 0 | 게이트웨이: 스키마·크기·큐 | `MALFORMED_COMMAND` 등 기존 | ADR-0006 |
  | 0b | 게이트웨이: 상태 변경 명령이고 `recording_lag`(커밋 대기 중인 가장 오래된 배치의 나이, tick — ADR-0013 §6 K2b) > 20 | **`RECORDING_BACKLOG`** | ADR-0013 §6. 이동 명령은 받지 않는다 |
  | 1 | 같은 `command_id` 가 처리된 적 있다 | `DUPLICATE_COMMAND_ID` | 세션 안: 모든 재전송. 세션·actor·재기동을 넘어: 이 월드에서 **수락된** 명령(I-52) |
  | 2 | `deposit_id` 가 이 성계의 광맥이다 | **`TARGET_UNKNOWN`** | 스키마상 유효한 `DataId` 여도 없는 id 면 여기 |
  | 3 | 이 actor 의 쿨다운이 끝났다: `tick ≥ 마지막 수락 tick + cooldown_ticks` | **`COOLDOWN_ACTIVE`** | 쿨다운은 **수락된** 채굴만 시작한다 |
  | 4 | `|함선 − 광맥|² ≤ (radius_m + mining_range_from_surface_m)²` | **`TARGET_OUT_OF_RANGE`** | tick 시작 시점(이번 tick 적분 전)의 서버 `f64` 상태. 제곱 비교 — sqrt 없음 |
  | 5 | `|함선 속도|² ≤ max_ship_speed_mps²` | **`SHIP_TOO_FAST`** | 같은 시점 |
  | 6 | 광맥 잔량(회복 적용 후) > 0 | **`RESOURCE_DEPLETED`** | |
  | 7 | 인벤토리에 더해도 `MassKg` 상한(2^31−1 kg)을 넘지 않는다 — checked 덧셈 | **`CAPACITY_EXCEEDED`** | 설계 상한은 없다(Q4). 이 행은 **무결성 CHECK 위반이 월드 정지(Q1)로 가지 않게** 막는 자리다 — 월드 공급률로 3.4 년 이상이라 실전 도달은 없고 경계값 주입으로 실행 증거를 낸다. 디버그·릴리스 같은 경로(ADR-0013 §5a) |
  | 8 | 적용 | — | 산출 = min(광물의 `yield_per_extraction_kg`, 잔량) — 마지막 몇 kg 은 부분 산출(디자인 §2.3) |

  - *1 이 2 보다 앞인 이유*: 중복은 무엇보다 먼저 중복이다. 두 번째 도착을 현재 세계로 다시 판정하면 같은 `command_id` 가 처음엔 수락, 다음엔 `COOLDOWN_ACTIVE` 처럼 **다른 답**을 받는다.
  - *함선이 없는 경우는 표에 없다*: 열린 세션은 언제나 `ACTIVE` 함선 1척을 조종한다(p1-01 I-29, I-40). 구조적으로 도달할 수 없는 사유 코드는 만들지 않는다(p1-01 의 `TOO_MANY_IN_FLIGHT` 교훈).
- **I-51 거부는 아무것도 바꾸지 않는다.** 인벤토리·광맥·쿨다운·처리 장부 어느 것도. `domain_events` 에도 남지 않는다(ADR-0007 §1). **거리 밖에서 누른 것이 벌이 되지 않는다**(디자인 §2.1).
- **I-71 같은 tick 의 채굴은 제출 순번 순이다.** `ship_id` 순이 아니다 — 작은 id 가 모든 동률 경쟁(마지막 kg, 최초 발견)을 영구히 이기는 불공정을 만든다(디자인 §2.3). 제출 순번은 이미 결정적이다(ADR-0006 §4). 한 tick 안의 단계: ① 회복(게으른 계산이라 실제 작업은 없다) → ② 명령 처리 → ③ 이동 적분.
- **I-47 과의 관계**: `MINE_RESOURCE` 도 tick 당 세션 명령 상한(8)에 포함된다.

### 4.3 멱등과 원자성 (ADR-0013)

- **I-52 한 `command_id` 는 인벤토리를 최대 1회 바꾼다 — 세션·재접속·서버 재기동을 넘어서.** 상태 변경 명령의 지속 중복 기억은 세션이 아니라 **월드 범위**이고(PK `(world_id, command_id)` 와 같은 범위 — ADR-0013 §3), 기동 시 `processed_commands` 에서 적재한다. 세션 기억도 함께 본다. DB PK 가 마지막 방어선이다. **거절된 명령은 장부에 남지 않는다**: 세션 안에서는 어떤 재전송이든 `DUPLICATE_COMMAND_ID` 지만, 재접속·재기동 뒤 예전에 거절된 `command_id` 는 새로 판정된다 — 상태를 바꾼 적이 없으므로 복사가 아니고, 거절까지 영속하면 스팸 1건마다 DB 쓰기가 생긴다(designer 초안 §2.2 와 다르게 정한 것, 이유 포함 통보함).
- **I-53 인벤토리 변화 ⇔ `MINERAL_MINED` 1건, 같은 커밋.** 인벤토리 행·광맥 행·처리 장부·그 tick 의 이벤트·`last_tick` 은 한 트랜잭션이다. 크래시는 이것들을 **함께** 잃는다 — 잃을 수는 있어도 복사는 없다.
- **I-54 원장 항등식** (각 `(world_id, actor_id, mineral_id)`):
  - 인벤토리 행 **=** `Σ MINERAL_MINED.quantity_kg` **=** `(tick, sequence)` 최대 이벤트의 `quantity_after_kg`
  - 연쇄: 이웃한 두 이벤트에서 `앞.quantity_after_kg = 뒤.quantity_before_kg`, 첫 이벤트의 `quantity_before_kg = 0`
- **I-55 보존 법칙** (각 `(world_id, mineral_id)`, 디자인 §2.5): `Σ 인벤토리 + Σ 광맥 잔량 = Σ 초기 매장량 + Σ 실제 회복량`. 광맥별로는 연쇄: 같은 광맥의 이웃한 두 이벤트에서 `뒤.deposit_remaining_before_kg = regen(앞.deposit_remaining_after_kg, 앞.tick → 뒤.tick)`, 첫 이벤트의 `before = 초기 매장량`. 회복량은 이벤트를 tick 순으로 되감으며 회복식(I-69)으로 **재계산할 수 있어야 한다** — 재계산한 잔량이 DB 와 다르면 결함이다.
  - **두 항등식은 채굴이 0 건이면 FAIL 로 인쇄한다**(CLAUDE.md). 검사한 키 수와 이벤트 수를 출력한다. 채굴 0 회 광물도 행으로 출력하되 채굴 수를 같이 찍어 `0 = 0` 이 "검사했다" 와 구별되게 한다.
- **I-56 상태 행은 비교 후 쓰기다.** DB 의 현재 값이 이벤트의 `*_before_kg` 와 다르면(광맥은 회복식 적용 후 비교) 그 배치는 실패하고, **복구 불가 기록 실패는 월드를 멈춘다**(ADR-0013 §5). 조용히 덮어쓰지 않는다.
- **I-57 배치는 tick 단위로 멱등이다.** 이미 커밋된 tick 의 배치를 다시 쓰면 아무것도 바뀌지 않고 성공한다(ADR-0013 §4).

### 4.4 결정성 (원칙 9)

- **I-58 채굴 결과에 무작위가 없다.** 입력은 데이터 표 값과 광맥 상태뿐이다(디자인 §2.6 — "무작위를 시드로 재현한다" 가 아니라 "무작위가 없다"). 무작위를 넣고 싶어지면 입력을 `H(world_seed ‖ deposit_id ‖ extraction_ordinal)` 로 명시하는 ADR-0010 §4 방식으로만.
- **I-59 같은 초기 상태 + 같은 명령열 = 같은 이벤트 내용과 같은 메시지.** p1-01 I-37 의 확장이다. 채굴 명령을 포함한 재생에서 `MINERAL_MINED` 내용(`event_id`·`recorded_at` 제외)과 `DEPOSIT_FIELD_STATE`·`INVENTORY_STATE` payload 가 두 프로세스에서 바이트 동일하다. **그리고 기존 이동 golden 재생 파일은 한 바이트도 바뀌지 않는다** — 채굴이 물리를 건드리지 않았다는 증거다(CLAUDE.md, ADR-0010 §3.1).
- **I-72 수량은 정수 kg 이다.** 계약 `MassKg`(0 ~ 2^31−1), 데이터 표의 `_kg` 필드도 정수(ADR-0009 §2 SI 실수 예외에 들지 않는다). 인벤토리·광맥 잔량은 음수가 되지 않는다.

### 4.5 역사 (원칙 2·4·5·6·9, ADR-0014)

- **I-60 `MINERAL_MINED` 는 역사가 아니다(Level 0).** 역사 기록은 판정 규칙을 통과한 것뿐이고, 저장·송신되는 모든 역사 기록의 `importance_level` 은 **1 이상**이다(계약 `minimum: 1`).
- **I-61 한 `(world_id, star_system_id, mineral_id)` 에 `MINERAL_DISCOVERED` 는 최대 1건이고, 그 근거는 그 키의 `MINERAL_MINED` 중 `(tick, sequence)` 최소인 것 하나다.** 같은 tick 이면 `sequence` 가 작은 쪽이 단독 발견자다(공동 발견 없음 — designer 확정). 판정 상태(코드)와 `UNIQUE (world_id, event_type, dedupe_key)` + 결정적 PK(DB)가 각각 막는다.
- **I-62 역사의 근거는 커밋된 사실뿐이다.** 러너는 `tick ≤ worlds.last_tick` 인 커밋된 행을 `(tick, sequence)` 순으로 읽고, 코어는 **역행 입력을 거부**한다. 판정은 라이브 `WorldState` 를 읽지 않는다 — 입력은 이벤트 payload + 규칙 파일 + 자기의 앞선 판정뿐이다. 모든 `source_event_ids` 는 `domain_events` 에 존재한다.
- **I-63 역사·증거는 추가 전용이다.** `historical_events`·`historical_event_sources`·`evidence` 에 UPDATE/DELETE 트리거가 걸린다. 사실 테이블에 **자연어 서사 열이 없고** `fact_status` 는 `CHECK = 'CONFIRMED'` 다. 역사 테이블의 쓰기 경로는 러너 하나다.
- **I-64 역사 판정은 결정적·멱등이다.** `historical_event_id = UUIDv5(NS_HISTORY, "{world}|{event_type}|{dedupe_key}")`. 같은 `domain_events` 열을 빈 판정 상태에서 재생하면 저장된 역사 집합과 **id 까지** 같다(`recorded_at` 만 제외). 같은 배치를 N 번 처리해도 1 번과 같고, 배치 경계를 어디서 자르든 같다. 판정 대상 타입인데 판정할 수 없는 이벤트는 **건너뛰지 않고 멈춘다**(커서 유지).
- **I-65 알림은 커밋 뒤에만 나간다.** `LIVE` 는 역사 트랜잭션 커밋 후, `BACKFILL` 은 세션 시작 시 그 월드의 기존 PUBLIC 기록 전부를 `tick` 오름차순으로. 목록 추가·LIVE 송신과 세션 시작·BACKFILL 송신은 같은 직렬화 문맥이라 **둘 다 못 받는 세션이 없다**. 둘 다 받을 수는 있다 — 클라이언트는 `historical_event_id` 로 걸러 **한 번만** 표시한다.
- **I-66 발견자는 actor 다.** 함선은 `VESSEL` 역할의 두 번째 참여자로 **지금** 기록한다(Ship Biography 의 첫 줄 — 나중에 재계산할 수 없다). 표시 이름 체계가 없으므로 클라이언트가 `actor_id` 에서 결정적 짧은 표지를 만든다. **규칙(확정): `"Pilot-"` + 정규 소문자 하이픈 표기 `actor_id` 문자열의 마지막 4 글자**(예: `…-89abcdef0123` → `Pilot-0123`). **앞 4 글자가 아닌 이유**: UUIDv7 의 앞부분은 생성 시각(밀리초)이라 비슷한 시각에 만든 actor 들이 같은 표지를 갖는다 — 끝부분은 난수 비트다. 봇 로그도 같은 규칙을 쓴다. 표현이지 사실이 아니며 DB 에 없다. 충돌(1/65536)은 표현의 한계로 감수한다. **표지는 표시 전용이며 식별 키로 쓰지 않는다** — 대조·집계·중복 제거의 키는 언제나 `actor_id` 다. 개발용 subject 는 순번형이라 실제로 충돌한다: Unity `DefaultSubject` `…-000000000001` 과 `bot-001` `…-000000000001` 이 둘 다 `Pilot-0001`, `SecondObserverSubject` `…-000000000002` 와 `bot-002` 가 둘 다 `Pilot-0002`.
- **I-67 역사가 멈춰도 경제는 돈다. 경제 기록이 멈추면 월드가 멈춘다.** 러너의 충돌(같은 키·다른 근거)·판정 불가 이벤트는 러너만 멈추고 채굴은 계속된다. 경제 배치의 복구 불가 실패는 서버 전체를 멈춘다(I-56). 역사는 늦어도 되지만 틀리면 안 되고, 인벤토리는 늦어서도 안 된다.

### 4.6 광맥 — 드러남과 회복

- **I-68 미확인 광맥의 광물·매장량은 어떤 서버 메시지에도 실리지 않는다.** 드러남은 광맥별 영속 상태(`first_extracted_tick`) 하나이고 첫 채굴 tick 에 한 번 쓰인다. 미확인 광맥의 `DEPOSIT_FIELD_STATE` 항목은 `mineral_id`·`initial_reserve_kg`·`remaining_kg`·`first_extracted_tick` 이 **전부** `null` 이고, 드러난 광맥은 **전부** 비-null 이며 영원히 드러나 있다(회복으로 잔량이 초기값에 돌아와도). 드러남은 **광맥 단위 시뮬레이션 상태**이고 발견은 **광물 단위 역사 판정**이다 — 시뮬레이션은 역사 판정 결과를 읽지 않는다. **알려진 누출**: 클라이언트의 `data/` 사본에 광맥 표가 들어간다(ADR-0012 §7). 클라이언트 코드는 그 표에서 `mineral_id`·`initial_reserve_kg` 를 읽지 않는다 — §9.1 Q3.
- **I-69 회복은 게으른 닫힌 식이다.** 월드 tick 이 광물의 `regen_interval_ticks` 배수일 때마다 `regen_kg`, 초기 매장량이 상한: `effective(t) = min(initial, remaining + regen_kg × (⌊t/I⌋ − ⌊as_of/I⌋))`. 광맥별 타이머가 없고, **DB 쓰기도 도메인 이벤트도 만들지 않는다**(ADR-0013 §1). 서버가 내려가 있는 동안은 tick 이 흐르지 않으므로 회복도 없다.

### 4.7 데이터 표 검증 (I-38 확장 — 전부 기동 실패)

스키마 4종(§5) 통과에 더해: 모든 광맥의 `mineral_id` 가 광물 표에 있다 / **모든 광물이 광맥을 최소 1개 가진다** / 광맥 표의 `star_system_id` 가 로드된 성계다 / `|position| + radius_m + mining_range_from_surface_m ≤ soft_boundary_radius_m` / `cooldown_s × tick_hz` 와 각 `regen_interval_s × tick_hz` 가 정수 / `regen_kg ≤ initial_reserve_kg` / 광물·광맥 `id` 중복 없음 / 두 광맥의 채굴 구역이 겹치지 않는다(`거리 > 두 반지름 합 + 2 × 사거리`) / 규칙 파일의 `rule_version` 이 `rule_id + "@"` 로 시작하고 `event_type` 이 레지스트리의 `historical_event` 다. 로그에 파일과 필드가 나온다.

### 4.8 월드는 역사의 단위다 — 테스트의 격리

- **I-70 "최초 발견" 은 월드마다 광물당 한 번이다.** 개발 DB 의 기본 월드에서 한 광물이 발견되면 그 월드에서 다시는 발견되지 않고, `domain_events` 는 지우지 않는다(CLAUDE.md). 그러므로 **발견을 관측해야 하는 모든 검증은 새 월드에서 한다**: `worlds` 에 새 행을 INSERT 하고(`worlds` 는 추가 전용 대상이 아니지만 **지우지 않는다**) `STARFALL_WORLD_ID` 로 그 월드를 띄운다. 새 행의 `tick_hz = 20`, `calendar_epoch = 3800-01-01T00:00:00Z`, `calendar_scale = 60`, `sim_version = 1`, `last_tick = NULL` — 서버가 기동 시 대조한다. 인벤토리·광맥·처리 장부·역사 기록은 전부 `world_id` 를 키에 가진다. 서버에 "새 월드 만들기" 기능을 넣지 않는다 — SQL 한 줄이면 되고, 서버에 넣으면 운영 경로에 월드 생성이 생긴다.

## 5. 데이터 계약

### 5.1 타입 (`registry_version` 3 → **4**, 타입 13 → **23**)

| 타입 | kind | 생산자 | 소비자 | 요지 |
|------|------|--------|--------|------|
| `MINE_RESOURCE` | command | client, bots | server | `{deposit_id}` 뿐 |
| `MINERAL_MINED` | domain_event | server | server, history | `actor_id`·`causation_id`(= `command_id`, **명령이 원인인 첫 이벤트**) 비-null. payload `{ship_id, session_id, star_system_id, deposit_id, mineral_id, quantity_kg ≥1, quantity_before_kg, quantity_after_kg ≥1, deposit_remaining_before_kg ≥1, deposit_remaining_after_kg}` |
| `MINERAL_DISCOVERED` | historical_event | history | client, bots | 역사 envelope + participants 정확히 2(발견자 PLAYER/DISCOVERER, 함선 SHIP/VESSEL) + source 정확히 1 + payload `{mineral_id, deposit_id, quantity_kg}` |
| `INVENTORY_STATE` | server_message | server | client, bots | `{actor_id, items[{mineral_id, quantity_kg ≥1}]}` — `mineral_id` 오름차순, 0 kg 항목 없음, 상한 필드 없음 |
| `DEPOSIT_FIELD_STATE` | server_message | server | client, bots | `{star_system_id, deposits[{deposit_id, mineral_id?, initial_reserve_kg?, remaining_kg?, first_extracted_tick?}]}` — `deposit_id` 오름차순, 미확인은 넷 다 `null` |
| `HISTORICAL_EVENT_NOTICE` | server_message | **server** | client, bots | `{delivery: LIVE \| BACKFILL, historical_event: MINERAL_DISCOVERED}`. 러너가 **기록**을 만들고 게이트웨이가 **NOTICE 로 포장**한다(T0 합의 — 생산자 history → server 개정) |
| `MINERAL` | data | — | server, client | `data/minerals/{id}.json` |
| `DEPOSIT_FIELD` | data | — | server, client | `data/world/deposits/{star_system_id}.json`. 클라이언트는 id·이름·위치·반지름만 읽는다 |
| `MINING_RULES` | data | — | server, client | `data/mining/mining-rules.json` |
| `SIGNIFICANCE_RULE` | data | — | history | `data/history/rules/{rule_id}.json` — 규칙 하나당 파일 하나 |
| `COMMAND_RESULT` | 변경 | server | client, bots | `reason_code` += `TARGET_UNKNOWN`·`COOLDOWN_ACTIVE`·`TARGET_OUT_OF_RANGE`·`SHIP_TOO_FAST`·`RESOURCE_DEPLETED`·`RECORDING_BACKLOG`·`CAPACITY_EXCEEDED`. 같은 `schema_version` 의 호환 변경(ADR-0005 §4-2) |

- **역사 envelope** `contracts/common/historical-event-envelope.schema.json`: `historical_event_id`(UuidV5), `event_type`, `schema_version`, `world_id`, `rule_version`, `importance_level`(1~5), `tick`, `occurred_at`, `recorded_at`, `visibility`, `fact_status`, `source_event_ids`(≥1, unique), `location{star_system_id}`, `participants[]`, `payload`. 도메인 envelope 과 달리 `sequence`·`correlation_id`·`causation_id` 가 없다. **서사 텍스트 필드가 없다**(반례 fixture 가 그것을 건다). envelope 은 generic 한 *누가*·*어디서* 를, payload 는 타입별 *무엇* 을 싣는다 — HSE §12 의 `objects` 배열은 payload 와 같은 사실을 두 번 적게 되므로 두지 않는다(p2 정규화 때 payload 에서 파생).
- **`HISTORICAL_EVENT_NOTICE` 의 만기**: `historical_event` 가 `MINERAL_DISCOVERED` 를 직접 참조한다(역사 타입이 하나뿐이다). **두 번째 역사 타입을 넣는 슬라이스가 이 필드를 판별 union 으로 바꿔야 한다**(`schema_version` 2 또는 codegen 확장). 계약 description 에 적혀 있다.
- `evidence` 는 **와이어 계약이 없다**(DB 전용, 마이그레이션이 모양의 정본). 소비자가 없다. p2 에서 `contracts/history/` 에 올린다.
- 기존 도메인 이벤트 4종의 소비자에 `history` 를 **넣지 않는다.** 러너는 그 행들을 읽고 커서만 넘긴다 — 소비가 아니라 통과다. 넣으면 커버리지 스크립트가 거짓 소비 코드를 요구한다.
- 닫힌 값 집합(Rust 닫힌 열거형 / C# `string`, 모르는 값에 죽지 않는다): `delivery` = `LIVE`|`BACKFILL`, `visibility` = 6값(이번엔 사건 `PUBLIC`, 증거 `PARTICIPANTS_ONLY` 만 생산), `fact_status` = `CONFIRMED`, `entity_kind` = `PLAYER`|`SHIP`, `role` = `DISCOVERER`|`VESSEL`, `rarity` = `common`|`uncommon`|`rare`.
- 새 primitives: `UuidV5`(역사 id 전용), `MassKg`(정수 kg, 0~2^31−1), `RuleVersion`(`{rule}@{n}`).

### 5.1a 명령 → 기대 응답 — 이제 레지스트리의 `responses` 필드가 정본이다

p1-01 §5.1a 가 발동 조건으로 적은 *"세 번째 명령 타입이 들어오는 슬라이스에서 다음 `registry_version` 인상과 함께"* 가 지금이다. 레지스트리 항목에 선택 필드 `responses: [{type, when}]`(`when` = `always`|`accepted`, 송신 순서대로)를 두었고, 세 명령 모두 채웠다. **아래 표는 그 필드의 사람이 읽는 사본이다.**

| 명령 | `always` | `accepted` | 비고 |
|------|----------|------------|------|
| `PING_SERVER` | `COMMAND_RESULT` | `PING_REPLY` | 기존 |
| `SET_SHIP_CONTROL` | `COMMAND_RESULT` | — | 기존 |
| `MINE_RESOURCE` | `COMMAND_RESULT` | **`INVENTORY_STATE`** — 같은 tick, 결과 뒤 | `DEPOSIT_FIELD_STATE`(월드 브로드캐스트)와 `HISTORICAL_EVENT_NOTICE`(비동기, 대부분 없음)는 **응답이 아니다** |

게이트를 쓸 때의 의무는 그대로다: 이 필드에서 유도한 항등식(`accepted MINE_RESOURCE 수 == 결과 뒤 INVENTORY_STATE 수`)은 **채굴이 0 건이면 FAIL** 로 인쇄한다.

### 5.2 파일과 건수 (architect 실측, 2026-09-27)

| 파일 | 상태 |
|------|------|
| `contracts/common/primitives.schema.json` | `UuidV5`·`MassKg`·`RuleVersion` 추가 |
| `contracts/common/historical-event-envelope.schema.json` | 신규 |
| `contracts/commands/MINE_RESOURCE.schema.json` | 신규 |
| `contracts/events/domain/MINERAL_MINED.schema.json` | 신규 |
| `contracts/events/historical/MINERAL_DISCOVERED.schema.json` | 신규 (`events/historical/` 의 첫 파일) |
| `contracts/messages/{INVENTORY_STATE,DEPOSIT_FIELD_STATE,HISTORICAL_EVENT_NOTICE}.schema.json` | 신규 3 |
| `contracts/messages/COMMAND_RESULT.schema.json` | `reason_code` 6값 추가 |
| `contracts/data/{mineral,deposit-field,mining-rules,significance-rule}.schema.json` | 신규 4 |
| `contracts/registry/types.json` | `registry_version` 3 → 4, 타입 13 → 23, `responses` 3건 |
| `contracts/registry/types.schema.json` | `responses` 선택 필드 |
| `contracts/fixtures/**` | 신규 **유효 18 / 반례 40** |

**건수 (Python `jsonschema` 로 실측)**: 스키마 29개 전부 메타 스키마 통과, 레지스트리가 자기 스키마 통과. **유효 fixture 46(= 기존 27 + 19) 전부 통과, 반례 74(= 34 + 40) 전부 거부.** *(`CAPACITY_EXCEEDED` 추가로 45 → 46, 2026-09-27)* 실제 `data/` 새 파일 7개(minerals 4, deposits 1, mining-rules 1, history/rules 1) 스키마 오류 0.

**코드 생성기 (실측)**: `tools/codegen` 을 **수정 없이** 임시 디렉터리로 돌리면 17 파일을 쓴다 — 신규 6(`MineResourceCommand`·`MineralMinedEvent`·`MineralDiscoveredEvent`·`InventoryStateMessage`·`DepositFieldStateMessage`·`HistoricalEventNoticeMessage`), 기존 10 **바이트 동일**, `ContractTypes.cs` 만 변경. 데이터 4종은 건너뛴다. NOTICE 안의 역사 이벤트는 중첩 클래스로 생성된다(`MINERAL_DISCOVERED` 스키마에 `type: object` 를 둔 이유).

**커버리지 기준선** (`check_contract_coverage.py --strict`, 구현 전): **types 23, errors 25, warnings 0** — 신규 타입의 코드 참조 없음 25건. 0 이 되는 것이 통과 조건이다.

**예측 (구현자가 실측으로 확인)**: Rust `registry_consistency` 의 하드코딩 13 → 23, fixture 테스트 27/34 → 46/74 로 **예상대로 RED**. C# EditMode 의 유효 fixture 발견 수 27 → 46, 왕복 21 → **36**(데이터 kind 10 건 제외 — 예측).

**반례가 거는 불변식** (층별 거부 책임은 server·client 가 구현 중 실측해 표로 채운다 — p1-01 §5.4 형식): 명령에 수량·광물·위치·actor 주입(I-48), 역사 envelope 의 서사 필드·`sequence`·Level 0·`DISPUTED`·v7 id·빈 근거·버전 없는 규칙(I-60·I-63·I-64), 도메인 이벤트에 `importance_level` 주입(I-60, 원칙 4 의 계약상 경계), 인벤토리 0 kg 항목·상한 필드, 광맥 상태의 힌트 필드, 데이터 표의 소수 kg·읽히지 않는 속성·다른 표의 입력(`rarity_weight`).

### 5.3 반례 fixture 의 층별 거부 책임 — **규약**

p0-01 §5 · p0-02 §5.4 · p1-01 §5.4 의 규약을 그대로 잇는다(client 확인 요청에 대한 답):

- **스키마 열**: 반례는 **전부** 스키마 검증에서 거부된다. 이것은 architect 가 실측했다(74/74).
- **Rust serde 열**: 운영 중 입력을 막는 것은 serde 다. 예측은 전부 거부이고 **server 가 실측으로 채운다** — 하나라도 통과하면 그것은 serde 타입의 구멍이다(AC-8).
- **C# Strict 열**: **"반례 = 역직렬화 실패" 가 규약이 아니다.** C# 은 범위·패턴·열거값을 검사하지 않는다(`enum` → `string`, 정수 범위는 타입 폭만 — ADR-0002 §4, ADR-0005 §4-2). 그래서 C# 은 반례마다 **거부 / 감지 불가 / 계층 없음(데이터 kind)** 중 하나이고, **client 가 전수 역직렬화해 실측값으로 이 열을 채우고, 그 결과를 테스트 상수로 박는다**(p1-01: 거부 18 / 감지 불가 10 / 계층 없음 6). 예측과 다르면 예측을 고치는 것이지 테스트를 예측에 맞추는 것이 아니다 — 차이는 client 가 architect 에게 알린다.
- **이번 40 건의 C# 예측 집계: 거부 15 / 감지 불가 13 / 불확실 1 / 계층 없음 11.** 기존 34 건과 합치면 거부 33 / 감지 불가 23 / 불확실 1 / 계층 없음 17 = 74.
- **실측 (client-2, Unity EditMode, `03_client_impl.md` §3·§4 — 정본): 거부 16 / 감지 불가 13 / 계층 없음 11**, 합계 **거부 34 / 감지 불가 23 / 계층 없음 17 = 74.** 예측과 다른 칸은 불확실 1 건뿐이고 **거부**로 확정됐다(`missing-mineral-key` — 생성 DTO 의 `Required.Always` 가 키 누락을 잡는다). 그 밖의 39 칸은 예측과 같다. 소수 kg 2 건(`quantity-fractional`·`fractional-quantity`)은 예측대로 거부였다 — Newtonsoft 가 정수 필드의 소수 토큰에서 `JsonReaderException` 을 던진다(반올림하지 않는다). Rust serde 열은 server 실측 40/40 거부, 사유가 의도한 위반을 가리킴 40/40(qa r2 SC-36).

| fixture | 스키마 | Rust serde (예측) | C# Strict (예측) | 비고 |
|---|---|---|---|---|
| `MINE_RESOURCE/invalid/quantity-field-injected.json` | 거부 | 거부 | 거부 | **대표 반례.** 수량은 어휘에 없다(I-48) |
| `MINE_RESOURCE/invalid/mineral-field-injected.json` | 거부 | 거부 | 거부 |  |
| `MINE_RESOURCE/invalid/position-field-injected.json` | 거부 | 거부 | 거부 |  |
| `MINE_RESOURCE/invalid/actor-field-injected.json` | 거부 (`unevaluatedProperties`) | 거부 | 거부 | envelope 수준 주입 |
| `MINE_RESOURCE/invalid/missing-deposit-id.json` | 거부 (필수) | 거부 | 거부 |  |
| `MINE_RESOURCE/invalid/deposit-id-not-kebab.json` | 거부 (`pattern`) | 거부 (`DataId`) | 감지 불가 | 명령은 클라이언트가 생산한다 — C# 역직렬화는 테스트 경로뿐 |
| `MINERAL_MINED/invalid/causation-id-null.json` | 거부 (좁힘) | 거부 | 거부 | 명령이 원인인 첫 이벤트 |
| `MINERAL_MINED/invalid/actor-id-null.json` | 거부 (좁힘) | 거부 | 거부 |  |
| `MINERAL_MINED/invalid/quantity-zero.json` | 거부 (`minimum: 1`) | 거부 | 감지 불가 | 역사 판정의 "실제로 캤다" 조건(K4) |
| `MINERAL_MINED/invalid/quantity-fractional.json` | 거부 (`integer`) | 거부 | 거부 (타입) | 소수 kg 금지(I-72) |
| `MINERAL_MINED/invalid/remaining-negative.json` | 거부 (`minimum`) | 거부 | 감지 불가 |  |
| `MINERAL_MINED/invalid/importance-level-injected.json` | 거부 | 거부 | 거부 | **원칙 4 의 계약상 경계** — 도메인 이벤트엔 중요도 자리가 없다 |
| `MINERAL_DISCOVERED/invalid/narrative-field-injected.json` | 거부 | 거부 | 거부 | **원칙 2·6** — 사실에 문장 없음 |
| `MINERAL_DISCOVERED/invalid/domain-envelope-field-sequence.json` | 거부 | 거부 | 거부 | 역사 envelope ≠ 도메인 envelope |
| `MINERAL_DISCOVERED/invalid/importance-level-zero.json` | 거부 (`minimum: 1`) | 거부 | 감지 불가 | I-60 |
| `MINERAL_DISCOVERED/invalid/participants-discoverer-only.json` | 거부 (`minItems: 2`) | 거부 | 감지 불가 | 함선 VESSEL 누락(K2) |
| `MINERAL_DISCOVERED/invalid/fact-status-interpretation.json` | 거부 (`enum`) | 거부 (닫힌 열거형) | 감지 불가 | 사실 상태에 해석 값 |
| `MINERAL_DISCOVERED/invalid/historical-id-v7-not-derived.json` | 거부 (`UuidV5` pattern) | 거부 | 감지 불가 | C# 는 `Guid` 로 파싱된다 — 버전을 보지 않는다 |
| `MINERAL_DISCOVERED/invalid/source-event-ids-empty.json` | 거부 (`minItems`) | 거부 | 감지 불가 | I-62 |
| `MINERAL_DISCOVERED/invalid/rule-version-without-number.json` | 거부 (`pattern`) | 거부 | 감지 불가 |  |
| `INVENTORY_STATE/invalid/capacity-field-injected.json` | 거부 | 거부 | 거부 | 상한 없음(Q4) |
| `INVENTORY_STATE/invalid/fractional-quantity.json` | 거부 | 거부 | 거부 (타입) |  |
| `INVENTORY_STATE/invalid/zero-quantity-item.json` | 거부 (`minimum: 1`) | 거부 | 감지 불가 | 0 kg 항목은 없다 |
| `DEPOSIT_FIELD_STATE/invalid/hint-field-injected.json` | 거부 | 거부 | 거부 | 누출 경로(I-68) |
| `DEPOSIT_FIELD_STATE/invalid/missing-mineral-key.json` | 거부 (필수) | 거부 | **거부 (실측 — 예측 불확실)** | **필수이면서 nullable** 인 필드의 키 누락 — p1-01 에 같은 모양의 반례가 없다. C# 이 `null` 로 채우면 감지 불가. client 가 실측으로 확정한다 |
| `DEPOSIT_FIELD_STATE/invalid/remaining-negative.json` | 거부 | 거부 | 감지 불가 |  |
| `HISTORICAL_EVENT_NOTICE/invalid/sentence-injected.json` | 거부 | 거부 | 거부 |  |
| `HISTORICAL_EVENT_NOTICE/invalid/unknown-delivery.json` | 거부 (`enum`) | 거부 (닫힌 열거형) | 감지 불가 | enum → C# `string` 의 의도된 대가 |
| `HISTORICAL_EVENT_NOTICE/invalid/nested-level-zero.json` | 거부 | 거부 | 감지 불가 | 중첩된 역사 이벤트 안의 검증이 동작하는지 |
| 데이터 4종의 반례 — `MINERAL` 3 · `DEPOSIT_FIELD` 3 · `MINING_RULES` 2 · `SIGNIFICANCE_RULE` 3 = 11 건 | 거부 | 거부 | (계층 없음) | 소수 kg·읽히지 않는 속성·다른 표의 입력(`rarity_weight`) 등 |

## 6. 역사 연결

| 이벤트 | 중요도 | 생성 조건 | 남는 증거 | 이어지는 플레이 |
|--------|:---:|------|------|------|
| `MINERAL_MINED` (도메인) | **0 — 역사 아님** | 수락된 채굴마다 | 자기 자신(`domain_events` 행) | 원장이다. 인벤토리가 왜 그 수량인지의 유일한 근거. 거래(p1-03)에서 **"이 광물은 어디서 왔나"** 의 첫 고리 |
| `MINERAL_DISCOVERED` (역사) | **2 Regional** | `mineral-discovery@1`: 그 월드·성계에서 그 광물의 `(tick, sequence)` 최소 `MINERAL_MINED` | `evidence` 1건 — `SHIP_LOG`, `automatic`, `VERIFIED`, `derived_from = {}`(원본), 출처 = 그 `MINERAL_MINED`, **`PARTICIPANTS_ONLY`**(사건은 모두가 알지만 원본 로그는 발견자만 — p2 에서 공개·판매·탈취·위조가 전부 새 증거로 붙을 자리) | **이번 슬라이스**: 발견 배너가 접속자 전원을 그 광맥으로 부르고, 나중에 온 사람은 라벨·발견 목록으로 흔적을 보며, `미발견 N종` 이 남은 탐사를 가리킨다. **p1-03**: 발견된 광물이 처음 값을 가진다 — 발견자가 첫 공급자. **p1-04**: 유일한 귀한 광맥(Far Reach, 500 kg)이 첫 분쟁 지점. **p2**: Chronicle 첫 항목, Biography·Ship Biography 의 "첫 발견" 줄, "사실 내가 먼저 찾았다" 는 Claim 이 증거(추출 tick)로 다투어지는 첫 역사 논쟁 |

**"다른 플레이어가 이 행동의 흔적을 발견할 수 있는가?"(MVP 핵심 테스트)** — 예, 세 겹이다: 발견 순간의 LIVE 배너, 나중 접속의 BACKFILL, 그리고 **드러난·비어 가는 광맥 자체**(`Starfall Glass · 0 / 500 kg — 고갈` 은 "누가 이미 다녀갔다" 이다). AC-16 이 A(발견자, 떠남)·B(동시 접속)·C(A 가 떠난 뒤 첫 접속)로 잰다.

**원칙 4 의 경계는 표와 계약 둘로 긋는다.** 표: ADR-0007 §1 에 `MINERAL_MINED` 를 **"기록 — 역사 아님"** 으로 추가했다. 계약: 도메인 이벤트는 `events/domain/`, 역사 이벤트는 `events/historical/` 에 살고 envelope 이 다르다 — `MINERAL_MINED` 에 `importance_level` 을 넣으면 스키마가 거부하고(반례 fixture), 역사 envelope 은 `importance_level ≥ 1` 을 요구한다. **기록하지 않기로 한 것**(디자인 §4.2): 모든 `MINERAL_MINED`(L0), 광맥의 첫 추출 = 드러남(시뮬레이션 상태), 광맥 고갈(도메인 이벤트의 `remaining_after = 0` 으로만), 개인 첫 획득(클라이언트 표현). **판정기의 기본값은 판정하지 않음이다.**

**Fact / Claim / Interpretation (원칙 6) — 이번에 둔 테이블과 이유** (history 합의, ADR-0014 §3):

| 층 | 이번 | 이유 |
|---|---|---|
| Fact | `domain_events`(기존) + `historical_events` + `historical_event_sources` + `evidence` (+ 가변 `history_cursor`) | 생산자가 있다: sim, 러너 |
| Claim | **없음** | 주장을 쓰는 플레이어 행동이 이번에 없다. 빈 테이블은 모양이 틀려도 드러나지 않고, 틀린 모양은 첫 행 뒤에 못 고친다 |
| Interpretation | **없음** | 같다. HSE §88 도 Interpretation 을 "Later" 에 둔다 |

분리는 테이블 수가 아니라 **구조**로 지킨다: 사실 테이블에 서사 텍스트 열이 없고, `fact_status` 가 `CHECK` 로 한 값에 잠겨 해석 상태를 겸할 수 없고, 사실 테이블의 쓰기 경로가 러너 하나이며, 나중의 `claims` 는 사실을 가리킬 뿐 사실이 주장을 가리키지 않으므로 **p2 는 순수 추가 마이그레이션**이다. 결정적 id 가 재구축 뒤에도 claim 의 참조를 지킨다.

## 7. 수용 기준

모든 Then 은 실행 증거다. 실행하지 못한 항목은 PASS 가 아니라 **"미검증(환경)"** 이다. **각 항목의 `⊘` 는 그 항목을 자명하게 통과시키는 상태이고(p1-01 계약 §7b 규칙 1), 항목은 그 상태를 같은 실행에서 배제하는 단언을 포함한다.** 기대 숫자는 `contracts/fixtures/` 에서 오고 `data/` 에서 오지 않는다(p1-01 §11-2) — 단 AC-2 와 사람 세션은 실제 `data/` 를 쓴다. history 의 테스트 H-01~H-15(`01_history_review.md` §6)가 AC-9~AC-12 의 세부 절차이고, designer 의 시나리오 S-1~S-10 이 AC-15~AC-17 의 세부 절차다 — QA 가 스프린트 계약에서 항목으로 펼친다.

### 서버

- **AC-1 (server) 게이트와 순수성.** `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace --locked` 종료 코드 0. `starfall-sim`·**`starfall-history`** 의 `Cargo.toml` 에 `axum`·`sqlx`·`redis`·`rand`·`chrono`·`tokio` 가 없다. `starfall-history` 소스에 `SystemTime`·`Instant` 호출이 없다. ⊘ *검사 대상 크레이트가 목록에 없음* → 검사한 크레이트 이름 2개를 찍는다.
- **AC-2 (server) 데이터 표 검증.** (a) 정상 `data/` 로 기동하면 `/debug/stats` 에 광물 4·광맥 8·적재한 `rule_version` 이 나오고 파일과 일치한다. (b) §4.7 의 조건 각각을 하나씩 깨뜨린 `STARFALL_DATA_DIR` 에서 **전부 기동 실패**이고 로그에 파일·필드가 나온다. ⊘ *기동 실패 원인이 주입한 결함이 아님(DB 미기동 등)* → 각 경우의 로그에서 **주입한 필드 이름**을 단언한다.
- **AC-3 (server) 채굴 판정.** 수동 step 통합 테스트(소켓·DB 없이). (a) 모든 조건 충족 → 수락, 인벤토리가 정확히 산출만큼, `MINERAL_MINED` 1건, `causation_id = command_id`, before/after 가 맞고, 광맥이 드러난다. (b) §4.2 표의 사유 **2~7 각각**(7 은 적재량을 상한 − 1 로 주입)을 만드는 상태에서 거부되고 **인벤토리·광맥·쿨다운·처리 장부가 바뀌지 않는다**. (c) 두 사유가 동시에 성립하는 상태(쿨다운 중 + 사거리 밖)에서 **표 순서의 앞 사유**가 나온다. (d) 거부 직후 tick 에 다른 조건을 만족시키면 **쿨다운에 막히지 않는다**. (e) 잔량 < 산출량이면 부분 산출이고 다음 명령은 `RESOURCE_DEPLETED`(디자인 S-3 의 단위 판). (f) 같은 tick 의 두 채굴이 **제출 순서**대로 처리된다 — 제출 순서를 바꾸면 결과가 바뀐다. (g) 회복: 소진된 광맥이 `regen_interval` 배수 tick 을 지나면 `regen_kg` 만큼, 초기값을 넘지 않게 — **식(I-69)과 tick 별 시뮬레이션이 같은 값**. ⊘ *모든 경우가 거부로 끝남* → (a) 의 **수락 수 ≥ 1** 을 출력한다.
- **AC-4 (server) 복사 경로 — 같은 `command_id` 는 한 번.** 경로마다 **첫 전송이 `ACCEPTED` 였고 인벤토리가 늘었음**을 먼저 단언한다(⊘ *첫 전송부터 거부되어 두 번째도 당연히 안 늘어남*).
  (a) 한 세션에서 같은 명령 2회 → 둘째 `DUPLICATE_COMMAND_ID`.
  (b) 연결을 끊고 잔류 창 안에 재접속(새 세션, 같은 actor) → 같은 명령 → `DUPLICATE_COMMAND_ID`.
  (c) **서버 정상 종료(stdin `shutdown`) → 재기동 → 같은 actor 로 접속 → 같은 명령** → `DUPLICATE_COMMAND_ID`, 인벤토리 행·`processed_commands` 행 수 불변.
  (d) 같은 actor 의 두 연결(넘겨받기 경로, p1-01 I-29)이 **같은 tick 에** 같은 `command_id` 를 보냄 → 수락 1, 중복 1.
  (d2) **다른 actor**(토큰 둘)가 같은 `command_id` 를 보냄 → 수락 1, `DUPLICATE_COMMAND_ID` 1, **서버가 멈추지 않는다**(ADR-0013 §3 K1 — 기억과 PK 의 범위가 같다). 다른 월드에서 같은 `command_id` 는 수락된다.
  (e) **이미 커밋된 tick 배치를 영속화에 다시 넣음**(DB 통합 테스트) → 행 변화 0, 에러 없음, **`persist_ambiguous_commits_total` +1**(I-57 — 건너뜀 경로가 실제로 실행됐다는 증거).
  (f) sim 의 기억을 우회해 중복이 DB 에 도달하게 만든 테스트 → **PK 위반 → 정지 경로**, 인벤토리 불변(ADR-0013 §3 의 마지막 방어선이 실제로 걸린다 — 규칙 6).
  (g) 세션 안에서 **거절된** 명령의 재전송 → `DUPLICATE_COMMAND_ID`(그 사이 조건이 풀렸어도). 재접속 뒤의 거절 명령 재전송은 새로 판정된다(I-52) — **어느 쪽이든 수락은 최대 1회**.
  각 경로의 최종 인벤토리가 **산출 × 1** 이다.
- **AC-5 (server) 원장·보존 항등식과 비교 후 쓰기.** (a) 봇 2 이상 × 채굴 N 회 뒤 I-54·I-55 가 SQL 로 성립한다. **검사한 (actor, mineral) 키 ≥ 2, 이벤트 ≥ 10 이 아니면 FAIL.** (b) 서버가 떠 있는 동안 **한 actor 의 인벤토리 행을 SQL 로 바꾼 뒤** 그 actor 가 채굴 → 서버가 **정지**한다: 0 아닌 종료 코드, 로그에 비교 후 쓰기 실패와 키, `persist_fatal_total ≥ 1`, 그 tick 의 `MINERAL_MINED` 가 DB 에 **없다**. ⊘ *서버가 다른 이유로 죽음* → 변조 전에 같은 actor 의 채굴이 수락·커밋됐음과 종료 로그의 사유를 단언한다. (c) 재기동하면 DB 값(변조된 값)으로 적재되고 다음 채굴이 그 위에서 수락된다 — **DB 가 정본**. (d) `recorded_at` 생성 실패 주입과 **payload 직렬화 실패 주입** 각각에서 이벤트를 건너뛰거나 JSONB `null` 을 저장하지 않고 배치가 실패해 월드가 멈춘다(현재 코드의 `continue` 와 `extract_payload` 의 `null` — ADR-0013 §5). (e) 정지가 결정된 뒤 종료 스윕의 `SESSION_CLOSED` 가 **커밋되지 않는다**(K5). (f) 일시 실패 분류: `57P01`(관리자 종료)을 주입하면 재시도이지 정지가 아니다 — AC-6 의 `compose stop` 이 이 경로다.
- **AC-6 (server) 기록 지연 거부.** 봇이 채굴·이동 중에 `docker compose stop postgres` → (a) `recording_lag` > 20 tick 이 된 뒤의 `MINE_RESOURCE` 는 `RECORDING_BACKLOG`, (a2) **DB 가 살아 있는 한가한 서버에서는 60 초 동안 `RECORDING_BACKLOG` 0 건**(하트비트 톱니에 걸리지 않는다 — K2 의 대조), (b) 같은 구간 `SET_SHIP_CONTROL` 은 수락되고 스냅샷이 흐른다, (c) `docker compose start postgres` 후 백로그가 빠지고 채굴이 다시 수락된다, (d) 정지 전·중에 수락된 채굴이 **전부** DB 에 있고 항등식이 성립한다. ⊘ *DB 정지 동안 채굴 명령을 한 번도 안 보냄* → (a) 의 거부 수 ≥ 1 과 **정지 전 수락 수 ≥ 1** 을 함께 찍는다. *(`stop` 은 볼륨을 지우지 않는다 — CLAUDE.md 금지는 `down -v`)*
- **AC-7 (server) 결정성.** (a) 채굴 명령을 포함한 기록된 입력열(함선 2척, 광맥 2곳, 쿨다운·사거리 밖·속도 초과·부분 산출·소진·회복 경계 포함)을 **서로 다른 프로세스에서 2회** 재생 → 매 tick 의 이벤트 내용(`event_id`·`recorded_at` 제외)과 스냅샷·`DEPOSIT_FIELD_STATE`·`INVENTORY_STATE` payload 가 **바이트 동일**. 비교한 `MINERAL_MINED` 수(≥ 1)를 찍는다. (b) **기존 이동 golden(`server/crates/sim/tests/data/replay/`)이 재생성 없이 통과한다** — 파일 해시를 작업 전후로 찍는다. ⊘ *golden 을 `STARFALL_REPLAY_BLESS=1` 로 다시 만듦* → 해시 불변이 판정이다.
- **AC-8 (server) 계약 테스트.** `cargo test -p starfall-contracts --locked` 가 타입 23종 전부를 덮는다: 유효 fixture **46** 왕복, 반례 **74** 스키마 거부, 반례의 serde 결과가 층별 표와 일치, `required` 제거 변이 실패, 레지스트리 `responses` 가 명령 3종에 있다. **검사 건수를 출력한다.**
- **AC-9 (server) 누출 없음.** 새 월드에서 첫 채굴 전 세션이 받은 모든 메시지에 미확인 광맥의 `mineral_id`·매장량이 없다. **양성 대조**: 첫 채굴 뒤 그 광맥의 `mineral_id` 가 실제로 나타난다(나타나지 않으면 검사가 필드를 잘못 찾는 것이다). 클라이언트 쪽은 AC-14(f).

### 역사

- **AC-10 (history) 판정 코어 성질.** DB 없이(H-01·H-03·H-04·H-08·H-09·H-13). (a) 첫 채굴 → 기록 1건(Level 2, `mineral-discovery@1`, 근거 = 그 이벤트, 참가자 = actor·ship, id = fixture 의 UUIDv5). (b) 같은 광물 두 번째 채굴 → 0 건 — **같은 테스트에서 (a) 의 1 건을 먼저 단언한다.** (c) 다른 광물 → 1 건. (d) 같은 tick 두 actor → `sequence` 작은 쪽. **fixture 의 `event_id` 순서는 `sequence` 와 반대로** 만든다. (e) 멱등: 1 회/k 회 처리 → 같은 결과. (f) 배치 경계 불변: 정렬된 입력을 무작위 지점(시드 출력)에서 잘라 처리 → 같은 결과. (g) **비정렬 입력은 거부**된다 — 정렬 후 동일(순열 수 출력)과 비정렬 거부를 두 절로 나눈다. (h) Level 0 타입만의 입력 → 0 건, 입력 수를 찍는다. (i) golden: `rule_version` 에 규칙 파일 정규화 해시와 고정 입력의 출력이 묶여 있고, **값만 바꾼 규칙 파일이 실제로 실패한다**(음성 대조). ⊘ *판정기가 아무것도 안 냄* → (a)(c) 가 양성 대조.
- **AC-11 (history) DB 통합.** 새 월드에서(H-02·H-07·H-10·H-11). (a) 첫 채굴 커밋 뒤 `historical_events`·`historical_event_sources`·`evidence` 각 1행·커서 전진, `source_event_ids` 가 `domain_events` 에 존재. (b) 역사 3 테이블에 UPDATE/DELETE → 거부 — **대상 행이 존재함을 먼저 단언**(행 단위 트리거는 0 행 매치에서 발화하지 않는다). (c) 같은 `dedupe_key` 로 두 번째 행을 **SQL 로 직접** INSERT → UNIQUE 위반. (d) 커밋 직전 장애 주입 → 재시작 → 기록 1, 커서가 그 뒤(주입 지점이 실제로 실행됐다는 카운터 ≥ 1). (e) 판정 불가 `MINERAL_MINED` 를 발견 후보 **앞에** 끼움 → 러너 정지, 커서가 그 앞, 뒤의 후보로 기록 없음, `history_detector_halted` +1, **같은 시간 채굴은 계속 수락·커밋**(I-67). (f) `last_tick` 보다 큰 tick 의 행은 판정되지 않는다. (h) **충돌 = 러너 정지, 재처리 = 조용함**(ADR-0014 §4, I-61·I-67 — 방어를 넣었으면 그것이 걸리는 입력에서 실제로 걸리는지 보인다, p1-01 §7b 규칙 6): 테스트 DB 에서 한 키에 대해 **근거가 다른** `MINERAL_DISCOVERED` 를 러너가 쓰게 만든다(예: 판정 상태를 비운 러너가 두 번째 채굴을 "최초" 로 판정하도록 주입) → 러너 정지, `history_conflicts_total ≥ 1`, 커서가 그 이벤트 앞, 역사 행 수 불변, **같은 시간 채굴은 계속 수락·커밋**. **짝(음성 대조)**: 같은 키·**같은 근거**의 재처리(재시작 재배달)는 정지도 카운터 증가도 없다 — 이 짝이 없으면 "무엇이든 멈춘다" 와 "충돌만 멈춘다" 를 구분하지 못한다. 주입 지점이 실제로 실행됐다는 카운터를 함께 찍는다. **(h)(iii) 같은 근거·다른 내용**: 러너가 그 채굴을 처리하기 **전에** 결정적 id·`dedupe_key`·`source_event_ids` 는 같고 payload 만 다른 행을 SQL 로 INSERT 해 둔다(추가 전용 트리거는 INSERT 를 막지 않는다) → 러너 처리 시 충돌로 정지. ⊘: 주입 행의 존재와 "러너 출력과 payload 만 다르다" 를 먼저 단언. 비교는 DB 의 `jsonb` 동등·배열 동등이지 문자열 비교가 아니다(키 순서·공백으로 거짓 충돌) — **배열 순서는 비교에 포함되므로 participants 순서(DISCOVERER → VESSEL)는 계약대로 고정이어야 한다.** (g) **재기동**: 발견 2건이 있는 월드에서 정상 종료·재기동 → 기록 여전히 2, 그리고 재기동 뒤 첫 신규 광물 채굴이 1건을 **추가**한다(디자인 S-7 — 판정기가 살아 있다는 긍정 증거).
- **AC-12 (history) 오라클 교차와 재생 결정성.** (a) 실서버 + 봇 채굴 세션 뒤 `01_history_review.md` §6.1 의 SQL 오라클(각 키의 첫 양수 채굴)과 역사의 발견 집합이 양방향 차집합 0. **오라클 행 수 ≥ 2 가 아니면 FAIL.** (b) 그 월드의 `domain_events` 를 **읽기 전용으로** 빈 판정 상태의 코어에 재생 → 저장된 기록과 id 까지 같다(`recorded_at` 제외). 재생한 이벤트 수와 비교한 기록 수를 찍는다. 재구축 테스트가 테이블을 비워야 한다면 **복사한 로그로 별도 스키마/DB** 에서 한다 — 주 DB 의 역사 테이블은 비우지 않는다.
- **AC-13 (history) 전달.** (a) 러너 커밋 뒤 열린 모든 세션이 `LIVE` 1건을 받는다. (b) 이후 접속한 세션이 `SESSION_READY` 뒤 `BACKFILL` 로 받는다. (c) **커밋 전 송신이 없다**: 러너 커밋을 실패시키는 주입에서 NOTICE 0 건 — 같은 테스트에서 주입 없는 경우 1 건을 먼저 단언한다. (d) **누락 틈 없음**: 커밋과 세션 열림이 겹치게 만든 반복에서 LIVE·BACKFILL 을 **둘 다 못 받은 세션 0**(겹침이 실제로 일어난 반복 수를 찍는다). (e) `MINERAL_MINED` 커밋 → LIVE 수신 지연 p50/p95/max — **p95 ≤ 1 초 목표(기록), 5 초 초과는 FAIL**.

### 클라이언트

- **AC-14 (client) 생성기·EditMode·그레이박스.** (a) 신규 DTO 생성, 생성 2회 멱등, 기존 DTO 바이트 동일(§5.2 의 실측을 구현 환경에서 재현). (b) EditMode: 유효 fixture 발견 수·왕복 수를 테스트가 상수로 단언(예측 46/36 — 실측으로 확정), `INVENTORY_STATE` 빈·다항목, `DEPOSIT_FIELD_STATE` 전부 미확인·혼합, NOTICE LIVE/BACKFILL 왕복, `Runtime` 프로필에서 모르는 `reason_code`·`delivery`·`visibility`·`entity_kind`·`role` 값에 죽지 않는다. (c) 사람이 그레이박스에서 디자인 §8 의 여섯 요소를 확인하고 스크린샷을 남긴다 — 특히 **산출 알림과 발견 배너가 다른 것으로 읽히는가**. 에이전트는 Play 를 누를 수 없다 → 사람 세션. (d) 인벤토리 패널이 **서버 응답 뒤에** 바뀐다 — 코드에 인벤토리 수량을 더하는 경로가 없음을 함께 보인다(I-49). (e) 같은 기록을 LIVE·BACKFILL 로 둘 다 받아도 한 번만 표시(EditMode). (f) **광맥 표식의 광물 이름은 `DEPOSIT_FIELD_STATE` 에서만 온다** — 클라이언트 코드가 데이터 사본의 `mineral_id`·`initial_reserve_kg` 를 읽지 않음을 보인다(소스 검사는 부정만 증명한다 — 규칙 9. 짝으로 EditMode: 사본의 광물을 바꿔도 표식이 메시지를 따른다).

### QA

- **AC-15 (qa) 치트 전수** (디자인 S-5·S-6). 시도 수와 막은 수를 적는다. (a) `quantity_kg`·`mineral_id`·위치·`actor_id` 주입 → 전부 `MALFORMED_COMMAND`, 인벤토리 불변. (b) 사거리 밖 반복 → `TARGET_OUT_OF_RANGE`. (c) 속도 30 m/s 로 지나가며 → `SHIP_TOO_FAST`. (d) 쿨다운보다 10 배 빠르게 60 초 → 수락 ≤ ⌈60 / 3⌉ + 1, 나머지 `COOLDOWN_ACTIVE`, **연결 유지**. (e) 없는 `deposit_id` → `TARGET_UNKNOWN`. (f) AC-4 (a)~(c) 를 봇으로. (g) 남의 인벤토리 → **어휘에 없어 시도 불가**. 사유별 관찰 수 > 0 을 출력한다 — 0 인 사유가 있으면 그 경로를 안 탄 실행이다.
- **AC-16 (qa) "C 가 흔적을 본다" — 슬라이스의 핵심** (디자인 S-1·S-4). 새 월드. (a) A·B 접속, A 가 광물 X 를 처음 캠 → A·B 모두 `LIVE` 수신, 발견자 = A. (b) B 가 X 를 캠 → **수락됐고** 인벤토리가 늘었는데 새 역사 기록 0 건. (c) A 가 접속을 끊고 **A 의 함선이 디스폰된 것을 단언**한 뒤 C 가 처음 접속 → `BACKFILL` 로 A 의 발견 수신 + `DEPOSIT_FIELD_STATE` 에서 그 광맥이 드러나 있음. (d) DB: X 에 대한 `MINERAL_DISCOVERED` 정확히 1 행, 근거 = A 의 첫 `MINERAL_MINED`, 증거 1 행. ⊘ (b) *B 의 채굴이 거부되어 역사가 없음* → B 의 수락을 단언. ⊘ (c) *A 가 아직 월드에 있음* → 디스폰 이벤트를 먼저 단언. ⊘ (c) *C 가 A·B 와 같은 봇 프로세스라 기억으로 앎* → C 는 별도 프로세스, 판정 입력은 C 가 받은 메시지뿐.
- **AC-17 (qa) 경쟁과 부하** (디자인 S-2·S-3·S-8·S-10). (a) S-2: 두 봇이 같은 목표 tick 에 같은 광물의 다른 광맥을 캠 → **두 `MINERAL_MINED` 가 같은 tick 임을 먼저 단언**(아니면 이 실행은 "경쟁 조건 미발생" 으로 무효 — 초록 아님), 발견자 = `sequence` 작은 쪽. (b) S-3: 잔량 100 kg 광맥에 세 봇이 같은 tick → 처리 직전 잔량 100 을 단언한 뒤 100 / `RESOURCE_DEPLETED` / `RESOURCE_DEPLETED`. (c) S-8: 보존 법칙이 모든 광물에 대해 성립(광물별 좌·우변과 채굴 수 출력), 회복이 실제로 일어난 광맥 ≥ 1. (d) 31 연결(봇 30 + Unity 1 또는 봇 31)이 이동 + 쿨다운마다 채굴 10 분: tick 초과 비율 ≤ 0.5 %(p1-01 기준 그대로), 항등식 성립, `RECORDING_BACKLOG` 거부 0(정상 부하에서 1 건이라도 나오면 영속화가 문제다), `MINERAL_DISCOVERED` 수 = 채굴된 광물 종류 수 ≤ 4, 고갈 발생 ≥ 1, 역사 알림 지연·영속화 커밋 소요 p50/p99 를 p1-01 기준선과 나란히.
- **AC-18 (qa) 기록 무결성.** (a) 이번 실행 tick 구간의 `event_type` distinct 가 기대 집합(`SESSION_*`·`SHIP_*`·`MINERAL_MINED`)이다. (b) 모든 `MINERAL_MINED.causation_id` 가 `processed_commands.command_id` 에 있고 그 역도 성립(양방향 누락 0). (c) `(world_id, tick)` 별 `sequence` 빈틈 없음. (d) p1-01 §11-8 의 자기 참조 결함 장부 7 건이 **그대로** 있다. (e) 역사 테이블 쓰기 코드 위치가 러너 모듈 하나다(H-14 — 소스 검사이므로 (a)~(c) 와 AC-12(a) 의 실행 증거와 짝으로만 PASS). ⊘ (b) *둘 다 0 행* → 행 수를 찍고 0 이면 FAIL.
- **AC-19 (qa) 계약 커버리지·경계면·CI.** (a) `check_contract_coverage.py --strict` 종료 코드 0(기준선 errors 25). (b) 신규 타입의 스키마·Rust·C# 필드별 표, 불일치 0. (c) 실제 `data/` 새 파일 전부가 스키마를 통과하고 `tests/e2e/validate_data_files.py` 의 `TARGETS` 가 새 4 경로를 포함한다 — **지금은 둘 다 모른다**(designer 가 소스로 확인한 부정). (c2) **읽히지 않는 데이터 파일 0**: `data/**/*.json` 각 파일(이번 슬라이스 10개)마다 "기동 시 읽는 코드(서버 로그의 해석 경로)" 와 "검사하는 게이트(스키마 검증 + 유도값 검산 또는 golden)" 가 짝으로 적힌 표를 낸다. 짝이 없는 파일이 있으면 FAIL — **틀려도 아무것도 실패하지 않는 파일**이다. 분모(파일 수)를 찍는다. (c3) 클라이언트 사본(`client/Assets/_Project/Data`)이 새 파일을 포함하고 SC-50(`ClientDataCopy_MatchesRepositoryOriginal`)이 초록 — **CI 가 Unity EditMode 를 돌리지 않으므로**(`gates.yml`) 이 슬라이스 PR 의 병합 조건으로 사람이 EditMode 를 돌려 결과 파일을 증거에 남긴다. (d) `.github/workflows/gates.yml` 의 출처 게이트가 **p1-02 계약**을 가리키고 PR 에서 초록이며, 그 게이트가 찍은 항목 분모가 p1-02 계약의 항목 수다(p1-01 의 90 이 찍히면 FAIL).

## 8. 비기능 요구

- **동시 연결 31** 유지(원칙 8). 채굴은 쿨다운 3 초라 actor 당 초당 0.33 건 — tick 부하는 거의 없다. 늘어나는 것은 영속화 트랜잭션의 행 수, 역사 러너의 읽기, `DEPOSIT_FIELD_STATE` 브로드캐스트(광맥 8개, 1 KiB 안팎 × 채굴·회복 tick 마다 × 31 세션).
- **tick 본문**: 채굴 판정은 O(1)(광맥 조회 + 제곱 비교 둘). 회복은 게으른 식이라 tick 비용이 없다. 영속화·역사는 tick 밖 태스크다. tick 초과 판정 기준은 p1-01 그대로 ≤ 0.5 %.
- **손실 창**: 크래시 시 잃는 경제 행동 ≤ `RECORDING_BACKLOG` 임계(20 tick = 1 초) < 쿨다운(3 초) — actor 당 최대 1 회 채굴(ADR-0013 §6).
- **역사 지연**: `MINERAL_MINED` 커밋 → LIVE p95 ≤ 1 초 목표, 5 초 실패 경계(ADR-0014 §7).
- **메시지 크기**: `INVENTORY_STATE` 는 광물 수(≤ 4)에 비례, `HISTORICAL_EVENT_NOTICE` 는 1 KiB 미만, BACKFILL 은 월드당 ≤ 4 건. 32 건을 넘는 슬라이스에서 조회 API 로(ADR-0014 §6 — 64 는 송신 큐 용량과 같아 새 접속을 끊는다).
- **WebGL**: 영향 없음(PC 전용 유지).

## 9. 열린 질문

### 9.1 사용자 결정 (2026-09-27) — **Q1~Q4 전부 추천안으로 결정됨**

| # | 결정 | 반영 |
|---|------|------|
| Q1 | 경제 기록 복구 불가 실패 → **서버 전체 정지** | ADR-0013 §5, I-56, AC-5(b) |
| Q2 | 크래시 시 ≤ 1 초 채굴 손실 **감수**, 크래시는 결함으로 다룬다 | ADR-0013 §6, §8 손실 창 |
| Q3 | 클라이언트 사본의 정답 누출 **p1-02 에서 수용**, 데이터 배포 경로 ADR 을 **다음 슬라이스 착수 전에** | I-68, 만기 표(`01_architect_tasks.md`) |
| Q4 | 인벤토리 **캐릭터(actor) 소유, 적재 상한 없음** | ADR-0013 §1, I-49, `INVENTORY_STATE` 에 상한 필드 없음 |
| Q6 | 광물 **4종** — 리더가 §9.2 판단으로 받음 | `data/minerals/*.json` |
| Q5 | 매장량 회복 **포함** — 리더 판단(designer·architect 일치, 사용자에게 보고됨) | I-69, I-55 회복 항, AC-3(g), AC-17(c) |

아래는 결정 당시의 질문 원문이다(원칙 5 의 정신 — 결정의 근거를 덮어쓰지 않는다).

- **Q1 경제 기록이 복구 불가하게 실패하면 서버 전체를 멈춘다(ADR-0013 §5).** 대안은 "그 배치를 버리고 계속" 인데, 그러면 메모리 인벤토리와 DB 가 갈라진 채 플레이가 이어지고 그 위의 모든 채굴이 틀린 전제를 딛는다. 가용성보다 무결성을 고르는 결정이다. **추천(architect): 멈춘다.**
- **Q2 크래시 시 최근 ≤ 1 초의 채굴이 사라질 수 있다.** 플레이어는 수락·인벤토리 증가·(드물게) 광맥 드러남을 봤는데 재접속하면 없다. 역사는 커밋된 사실만 보므로 **"사라진 채굴에 대한 발견" 은 생기지 않는다.** 없애려면 채굴마다 DB 커밋을 기다려야 하고(tick 이 DB 를 기다림) 그것은 ADR-0007 이 버린 구조다. **추천(architect): 감수하고, 크래시 자체를 결함으로 다룬다.**
- **Q3 클라이언트 데이터 사본에 "어느 광맥이 어느 광물인가" 가 들어간다**(디자인 §9 Q7). 서버는 미확인 광맥의 광물을 보내지 않고 클라이언트 코드는 사본에서 읽지 않지만, 빌드 파일을 뜯으면 정답이 보인다. 대안: (a) 서버 전용 값을 `data/` 밖으로 — "data/ 는 한 곳" 이 깨진다 (b) 전부 공개 — 발견이 탐사가 아니라 경주가 된다. **추천(designer·architect): 개발 플레이테스트인 p1-02 에서는 누출을 수용하고, 데이터 배포 경로 ADR(ADR-0012 재검토 조건 (d) — `data/` 가 3 → 10 파일로 이 조건이 충족됐다)을 다음 슬라이스 전에 쓴다.** 대안 (c) — 광맥 파일을 **공개부**(id·이름·위치·반지름, 복사함)와 **서버 전용부**(광물·매장량, 복사 안 함)로 쪼갠다: 누출은 막히지만 한 광맥이 두 파일에 걸쳐 참조 무결성 검사가 하나 늘고, SC-50 의 "`data/**` 전부 복사" 규칙에 예외가 생긴다. 이번 슬라이스에서 할 수 있는 가장 작은 차단책이다.
- **Q5 매장량 회복을 이 슬라이스에 넣는다**(designer 안 — architect 초기 권고 "유한 + 회복 없음" 과 다름). 회복이 없으면 지우지 않는 월드가 플레이테스트 몇 번 만에 영구히 빈다. 게으른 닫힌 식이라 DB 쓰기·이벤트·광맥별 타이머가 없다(I-69). 대가: 보존 법칙에 "실제 회복량" 항이 붙어 QA 의 재계산 도구가 하나 는다. **추천(designer, architect 동의): 넣는다.**
- **Q6 광물 4종**(designer 안 — architect 권고 "3 이하" 와 다름). 흔함 2 + 중간 1 + 희귀 1 이어야 "조금 더 멀리 가면 다른 것이 있다" 를 가르칠 단계가 생긴다. 3종으로 줄이려면 `cobaltine.json` 과 Outer Field 광맥 2개만 빼면 되고 계약·코드는 바뀌지 않는다. **추천(designer, architect 동의): 4.**
- **Q4 인벤토리는 actor(캐릭터) 소유이고 적재 상한이 없다.** 함선이 영속하지 않아(재기동·잔류 만료로 사라짐) 기술적으로 강제되는 소유자이고, 팔 곳(p1-03)이 없는 지금 상한은 "다 채운 뒤 할 일이 없음" 만 만든다. 결과: 함선을 잃어도 광물을 잃지 않는다. 나중의 "화물 약탈(전투)" 은 함선 화물칸을 전제하므로 그때 다시 연다. **추천(designer·architect): actor 소유, 상한 없음.**

### 9.2 리더 판단 (2026-09-27 — 전부 architect 안대로 받음)

- 규칙 설정 파일을 **둔다**(`data/history/rules/mineral-discovery.json`) — history·designer 사이에 세 번 뒤집힌 것을 architect 가 한 번 정했다(ADR-0014 §5). 조건: 기동 시 서버가 읽어 코어에 넘기고(AC-2(a) 가 적재한 `rule_version` 을 찍는다), golden 이 파일 해시와 코어 출력을 `rule_version` 에 묶는다(AC-10(i)). **읽는 코드와 검사하는 게이트가 짝이다.**
- 발견자 표시 이름 없음 — `Pilot-xxxx` 표지(I-66). 계정 슬라이스에서 NOTICE 조립 시 이름을 붙인다.
- 새 월드는 SQL INSERT 로 만든다(I-70). 서버 기능으로 넣지 않는다.
- 발견이 같은 광물의 다른 광맥까지 드러내지 않는다(디자인 Q3 — 시뮬레이션이 역사 판정을 입력으로 받는 첫 경로가 된다).

### 9.3 남은 확인 (차단 없음)

- ~~`data/history/rules/mineral-discovery.json` 작성~~ — designer 완료, 스키마 오류 0(architect 실측).
- 층별 거부 표(Rust serde / C#)는 구현 중 실측으로 채운다.

## 10. QA 가 알아야 할 환경 사실

p1-01 §11 의 1~8 은 계속 참이다. 추가:

| # | 조건 | 판정을 어떻게 무효로 만드나 | 방어 |
|---|------|------|------|
| 9 | **기본 월드에서 최초 발견은 광물당 한 번뿐이다** (I-70) | 두 번째 실행부터 "첫 채굴 → 발견" 이 관측되지 않아 **환경 때문에** 빨간불이거나, 이미 있던 기록을 BACKFILL 로 받아 **이번 실행이 만든 것처럼** 초록불이 된다 | 발견을 재는 항목은 **새 월드 행**에서. 월드 id 와 INSERT 문을 증거에 남긴다 |
| 10 | 기본 월드의 `domain_events` **4,805 행**(2026-09-27 실측: SESSION_OPENED 2258, SESSION_CLOSED 2254, SHIP_SPAWNED 148, SHIP_DESPAWNED 144, QA_APPEND_ONLY_PROBE 1)은 역사 러너가 **처음 기동할 때 전부 읽는다** | 첫 기동의 따라잡기 소요를 역사 지연으로 오독 | 따라잡기 완료 로그(읽은 행 수)를 본 뒤부터 잰다 |
| 11 | 인벤토리·광맥·처리 장부 테이블은 **추가 전용이 아니다**(상태 테이블) | AC-5(b) 의 변조가 "불변성 위반" 으로 오독 | 불변성은 `domain_events` 와 역사 3 테이블에만 건다. 상태 테이블의 진실성은 항등식이 잰다 |
| 12 | **회복은 월드 tick 배수에서 일어난다** — 기본 월드의 `last_tick` 은 166만대다 | 손계산한 잔량이 "몇 tick 뒤" 로 계산되면 배수 경계가 어긋나 틀린다 | 회복 기대값은 식(I-69)에 **실제 tick** 을 넣어 계산한다. 단위 테스트는 tick 0 기준 메모리 시뮬레이션 |
| 13 | 발견 배너 지연 p95 ≤ 1 초는 **목표**이지 판정이 아니다 | 1 초를 넘은 것을 FAIL 로 읽음 | 판정 경계는 5 초(AC-13(e)). 1 초는 designer 의 체감 가정이고 사람 세션이 확인한다 |

## 변경 기록

| 날짜 | 변경 | 이유 |
|------|------|------|
| 2026-09-27 | 최초 작성 (draft). ADR-0013·0014 신설, 불변식·수용 기준 초안 | p1-02 Phase 2 |
| 2026-09-27 | **계약 변경**: `SIGNIFICANCE_RULE.event_type` → `produces_event_type`(스키마·fixture 4). 최상위 판별자 이름(`command_type`·`message_type`·`event_type`)을 판별자 외에 쓰지 않는 규칙을 ADR-0002 §1a 로 신설 + 레지스트리 테스트 기계 검사(S1). 데이터 파일은 designer, 클라이언트 사본 재복사는 client | client `03_client_impl.md` §5 |
| 2026-09-27 | **qa 계약 r0 판정 2건**: AC-11(h) 신설 — 역사 충돌(같은 키·다른 근거) = 러너 정지 + `history_conflicts_total`, 짝으로 같은 근거 재처리는 조용함(Q-3). 출처 게이트 슬라이스 표지 방식 승인(Q-1 — 스펙 변경 없음, 계약 §3.3) | qa `02_sprint_contract.md` r0 |
| 2026-09-27 | **정지 경로 제약 표(ADR-0013 §5a) — 분모 78 제약 + 트리거 4**, ① 클라이언트 도달 예 1(`processed_commands_pkey`)·누적 1(`inventory_items_quantity_kg_check`), 둘 다 거름 장치와 테스트가 있다. **계약 변경**: `reason_code` += `CAPACITY_EXCEEDED`(판정 7단계, 넘침을 월드 정지 대신 거절로 — server 질문 1), fixture +1 → 유효 46. 디버그·릴리스 같은 경로(server 질문 2). 0002 가 `domain_events` 에 `CHECK (jsonb_typeof(payload)='object')` 추가 — JSONB `null` 이 `NOT NULL` 을 통과하던 구멍 | 리더·server |
| 2026-09-27 | **계약 변경 1건**: `HISTORICAL_EVENT_NOTICE` producers `history` → `server`(T0 합의 — envelope tick·`message_id`·`delivery` 는 게이트웨이만 안다). `MINERAL_DISCOVERED` 는 history 유지. `registry_version` 4 그대로(같은 슬라이스 안, 미구현 상태의 태그 정정). 재실측: 유효 45 통과 / 반례 74 거부, 커버리지 errors 25 그대로. **정지 경로 전수표**를 ADR-0013 §5a 로 | history·server T0 |
| 2026-10-05 | **implemented** — r3 PASS 112/FAIL 0 · SC-98 CI 첫 실행 PASS(PR #5) · SC-68 사람 세션 2차에서 사람 진술로 확인(스크린샷 없음, 1차 FAIL로 §8 표시 보강 C4) | 리더 마감 결정 |
| 2026-10-03 | **§5.3 C# 열을 실측으로 확정**(거부 16 / 감지 불가 13 / 계층 없음 11, 불확실 1 건 → 거부) — r1·r2 동결 동안 미뤄 둔 기록. 그 사이 판정: ADR-0006 §4a(메시지 id 흐름 분리), ADR-0013 K2b 직접 기준 성립(r1), ADR-0014 §4-2 `detector_rule` = `rule_version`, ADR-0007 §5 마이그레이션 동결 규칙 | client-2 실측, qa r1·r2 |
| 2026-09-30 | **`recording_lag` 정의 정정(K2b)**: "마지막 투입 − 마지막 커밋" 은 한가한 서버에서도 하트비트 간격(20)까지 커져 임계에 여유 0 — "커밋 대기 중 가장 오래된 배치의 나이" 로. 임계 20 유지 | qa 계약 외 발견(SC-104 실행) |
| 2026-09-27 | **server ADR-0013 검토 K1~K6 + 추가 1~3 반영**: 지속 기억·PK 를 월드 범위로(`(world_id, command_id)`, AC-4(d2)), `RECORDING_BACKLOG` 판정을 `recording_lag` 로(AC-6(a2)), 비교 후 쓰기의 기대값은 sim 이 저장 표현으로 싣는다, 일시 실패 허용 목록(AC-5(f)), 정지 후 배치 커밋 금지(AC-5(e)), 모호한 커밋 카운터(AC-4(e)), 세션 기억과 지속 기억 둘 다, 기동 순서(목록을 런타임 인자로), BACKFILL 트리거 64 → 32. payload 직렬화 `null` 저장 경로(server 발견)를 AC-5(d) 에 | server `02_server_ack.md` §1 |
| 2026-09-27 | **client 사전 검토 답변 3건**: §5.3 반례 층별 표 규약(C# 열은 실측으로 채우는 열 — "반례 = 역직렬화 실패" 가 아니다) + 신규 40 건 예측(`missing-mineral-key` 하나 불확실), I-73(모르는 거절 사유 = "알 수 없는 사유로 거절", 생성물이 `string` 임을 확인), I-66 표지 규칙 확정(끝 4 글자 — UUIDv7 앞부분은 시각). world mirror 는 기존 `PredictedShipController.CurrentState` 로 충분(선행 태스크 없음) | client `02_client_ack.md` |
| 2026-09-27 | **designer 설계·history 검토 반영, 계약 확정.** 광맥 8·광물 4·회복(게으른 식)·드러남·속도 조건 채택 → `DEPOSIT_FIELD_STATE` 신설, `CAPACITY_EXCEEDED` 삭제(상한 없음), 판정 순서를 designer 안으로(쿨다운 → 사거리 → 속도 → 소진), 같은 tick 은 제출 순번. history K1~K4·A1~A6 수용: 결정적 UUIDv5 id(결정성 비교에 id 포함), participants 에 함선 VESSEL, 역행 입력 거부, fail-stop, `historical_event_sources` 트리거, 규칙 파일 + golden, 누락 틈 없는 전달, 지연 p95 1 초 목표/5 초 경계. envelope 에 `objects` 를 두지 않음(payload 와 중복). 거절 명령은 영속 장부에 넣지 않음(designer 초안과 다름 — 통보). 레지스트리 `responses` 필드 도입(p1-01 §5.1a 의 발동 조건 충족). 계약 23 타입·유효 45·반례 74 실측, 생성기 무수정 동작 실측, 커버리지 기준선 errors 25. 사용자 질문 Q1~Q4 | designer `docs/design/p1-02-mining-design.md`, history `01_history_review.md` §9 |

# p1-03-trading 태스크 분해

- 작성: architect3, 2026-10-05
- 스펙: `docs/specs/p1-03-trading.md` (**agreed** — 사용자 결정 Q1~Q3 전부 추천안, 2026-10-05)
- ADR: `docs/adr/0015`(**accepted** — 데이터 배포, p1-02 만기 부채), `docs/adr/0016`(**accepted** — 크레딧과 판매 장부. server3·serverdb3 T0 검토 동의, `02_server_ack.md`). 개정: 0007 §1(표 3 행), 0014 §6(세션 시작 메시지 5 건 메모)
- 입력: `docs/design/p1-03-trading.md`(designer3 — 수치·T-1~T-12·UI-01~UI-13 의 정본), `02h_history_review.md`(history3 — R-1~R-9, H-30~H-34)
- **Phase 2 산출: 계약·ADR 확정.** 다음은 qa 스프린트 계약 → 구현(Sonnet + TDD).

## 작업 방식

**TDD.** 태스크마다 "먼저 쓸 실패 테스트" 를 지정했다. 먼저 쓰고, **실패를 확인**하고(빨간불 로그를 `03_*_impl.md` 에 남긴다), 최소 구현, 정리.

**모듈 완료 = 계약 자기 점검 통과**(qa-throughput R3): 담당 SC 마다 ① 이름 실행(계약의 필터 그대로 1건 이상, 다른 SC 섞임 없음) ② 구현 변이(구현을 망가뜨려 실패 — 기대값을 바꾸는 것은 변이가 아니다) ③ sha256 원복 증명 ④ 지명된 도구·카운터 존재 ⑤ DB 테스트 이름을 `TestDb::create` 에 그대로 — 표로 `03_*_impl.md` 에. **화면을 건드린 모듈은 추가로**(screen-elements S3) 담당 `UI-nn` 마다 표시 로직 테스트와 PlayMode 존재 단언이 계약 이름으로 실행되고 변이로 실패함을 보이고, "사람이 확인할 항목" 에 체크 문장과 조작 키를 적는다 — **"없음" 으로 둘 수 없다.**

**계약이 코드보다 앞서 있다.** 지금 `cargo test -p starfall-contracts` 와 Unity EditMode 는 **빨간불이 정상**이다(레지스트리 23 → 30, fixture 46/74 → 57/113, `DEPOSIT_FIELD` v2). 서버도 **지금 기동하지 못한다** — `data/world/deposits/cradle.json` 이 v2 로 바뀌었다(designer3 분할 완료, 값 불변 architect 확인). S2 가 고칠 때까지 기동 거부가 정상이다. architect 실측 기준선: `check_contract_coverage.py --strict` **types 30 / errors 17 / warnings 0** — 0 이 되는 것이 AC-22(a).

## ⚠ 시작 전에 알아야 할 것

1. **시세·흔적·첫 판매를 재는 실행은 새 월드에서**(스펙 §4.9, §10-14). `python tests/e2e/new_world.py --tag <SC> --evidence <경로>`.
2. **`docker compose down -v` 금지**, 서버 하드 킬 금지(stdin `shutdown`), golden 손 수정·BLESS 금지 — **이동·채굴·역사 golden 은 한 바이트도 바뀌면 안 된다**(AC-8(b)). 판매는 물리도 채굴도 역사 판정도 건드리지 않는다.
3. **마이그레이션 0001~0003 동결**(ADR-0007 §5). 새 테이블은 **0004** 하나에. 제약은 ADR-0016 §5 표의 요구 집합(이름)과 **정확히** 같아야 한다 — 표에 없는 제약을 넣고 싶으면 먼저 architect 에 요청(검토 없는 제약 = 검토 없는 정지 경로).
4. **원장 도구가 판매를 모르면 거짓 FAIL** — p1-02 의 질량 보존 도구(`tests/e2e/mining_ledger.py`)는 판매 항이 없다(스펙 I-84, §10-17).
5. **테스트 기대 숫자의 출처**: 계약 fixture, 그리고 대금은 **디자인 §3.6·부록 B**(architect 가 실제 `data/` 로 독립 재계산해 7 건 전부 일치 확인 — 2,000→20,000·이어서 18,000, 12,000→90,000, 14,000→98,000, Glacine 10,000→96,500, Cobaltine 3,200→100,000, Glass 500→100,000·700→122,500, `2^31−1` kg 판매의 구간 반복 8). 테스트 출력을 기대값으로 옮겨 적지 않는다.
6. **codegen: 생성은 됐으나 C# 컴파일에서 중첩 이름 충돌 1건 → 스키마 title 로 해결(`MarketLastSale`), 생성기는 충돌 시 실패하도록 검사 추가**(C1, client4). architect 의 2026-10-05 실측은 생성(신규 5 DTO, 기존 16 unchanged, `ContractTypes.cs` 만 변경)까지였고 **컴파일하지 않았다** — 확인이 아니었다. client4 가 Unity 컴파일에서 CS0102(`MARKET_STATE` 의 중첩 클래스 `LastSale` = 속성 `LastSale`)를 찾았다. 해결은 스키마 `title`(`MarketLastSale`, 와이어 불변)이고, 생성기는 이름 규칙을 바꾸지 않고 충돌 시 실패만 한다(리더 결정 2026-10-06, ADR-0002 §4). 판정 입력은 Unity 컴파일 성공이다.
7. **역사 코드는 바뀌지 않는다**(history3 판단). `starfall-history` 와 `persistence/src/history.rs` 에 diff 가 생기면 그것이 발견이다.

## 파일 소유권

| 경로 | 소유 | 비고 |
|------|------|------|
| `contracts/**`, `docs/{adr,specs}/**` | architect | 변경은 SendMessage 로 요청 |
| `data/**`, `docs/design/**` | designer | 이번 슬라이스 data 작업은 **완료**(시장 파일 1, 광맥 분할 2) |
| `server/Cargo.toml`·`Cargo.lock`, `server/crates/{contracts,sim,gateway}/**`, `server/bins/**` | server | |
| `server/migrations/0004_*.sql`, `server/crates/persistence/src/lib.rs`, `server/crates/persistence/tests/{economic_state,restart_economic_state,trading_*}.rs` | server-db | |
| `server/crates/persistence/tests/history_runner.rs` | history | **테스트만**(H-30~H-33). `persistence/src/history.rs`·`server/crates/history/**` 는 history 소유이되 **변경 예정 없음** |
| `client/Assets/_Project/Scripts/Contracts/Generated/**` | 생성물 | 손으로 고치지 않는다 |
| `client/Assets/_Project/Data/**`, `client/Assets/_Project/Scripts/{Contracts,Sim}/**`(Generated 제외), `Scripts/Greybox/GreyboxDataLoader.cs`, `Tests/EditMode/{ContractFixture*,GreyboxMiningData*}.cs` | client (C1) | |
| `client/Assets/_Project/Scripts/Trading/**`(신규), `Tests/EditMode/Trading*.cs`(신규) | client (C2) | 순수 로직 — MonoBehaviour 없음 |
| `client/Assets/_Project/Scripts/Greybox/GreyboxTradingSession.cs`(신규)·`GreyboxSession.cs`(연결부)·`UI/**`, `Tests/PlayMode/Trading*.cs`(신규) | client (C3) | |
| `tools/bots/**`, `tests/e2e/**`, `.github/workflows/**`, `.github/pull_request_template.md` | qa | |

**두 담당자가 같은 파일을 고치지 않는다.** 겹칠 자리(sim 이 만드는 배치 ↔ 영속화가 쓰는 배치, 기동 적재 ↔ sim 초기값)는 **T0 가 먼저 정한다.**

## 태스크

| ID | 태스크 | 담당 | 수정 경로 | 선행 | 수용 기준 | 먼저 쓸 실패 테스트 |
|----|-------|------|---------|------|----------|-----------------|
| **T0** | **경계 합의와 ADR-0016 검토** — (1) 배치에 실릴 상태 쓰기: 지갑 `{expected: 없음 \| balance, new}`, 시장 `{expected: 없음 \| (saturation, as_of_tick, last_seller, last_qty), new}`, 인벤토리 감소(기대값 저장 표현 그대로, ADR-0013 §5 K3). (2) 기동 적재 → sim 초기값 주입 API(지갑·시장 상태). (3) ADR-0016 §5 제약 표 12 + 6 의 이름 확정. (4) 게이트웨이의 상태 변경 명령 목록에 `SELL_MINERAL`. 결과를 `02_server_ack.md`(ADR-0016 검토 포함)와 `02_interface.md` 에 server·server-db 공동으로 | server + server-db | `_workspace/p1-03-trading/02_*.md` | 계약 ✓ | — | — |
| S1 | 계약 Rust 타입 — 신규 7 타입(`SELL_MINERAL`·`MINERAL_SOLD`·`WALLET_STATE`·`SALE_RECEIPT`·`MARKET_STATE`·`MARKET`·`DEPOSIT_RESERVE`), `DEPOSIT_FIELD` v2, primitive `CreditAmount`, `reason_code` += `INSUFFICIENT_QUANTITY`, 레지스트리 `data_files`·`responses`, fixture 테스트 57/113, 반례 serde 열 실측(스펙 §5.3) | server | `server/crates/contracts/**` | — | AC-9 | `registry_consistency` 를 30 기대로 바꿔 RED |
| S2 | 데이터 적재·검산 — 시장 파일, 광맥 분할(공개부 v2 + 예비, 짝 **양방향** 대조, v1 이면 거부), 스펙 §4.8 전부(기동 실패, 로그에 파일·필드), `/debug/stats` 에 시장 id·`pricing_version`·상장 수·예비 수 | server | `server/bins/game-server/src/data.rs`, `server/crates/contracts/src/data.rs` | S1 | AC-2 | 예비 파일에만 있는 광맥 → 기동 거부 |
| S3 | **sim 판매** — 판정 순서(스펙 §4.2), 구간 걷기 대금(§4.2a — kg 루프 금지, 반복 ≤ floor_band+1), checked 뺄셈·덧셈(`INSUFFICIENT_QUANTITY`·`CAPACITY_EXCEEDED` — 디버그·릴리스 같은 경로), 판매 쿨다운(메모리, 채굴과 독립), 포화 게으른 회복, 마지막 거래, 같은 장부의 지속 중복 기억(명령 종류 교차), `MINERAL_SOLD`(전 필드, causation = command_id), 응답 `SALE_RECEIPT`→`INVENTORY_STATE`→`WALLET_STATE`, `MARKET_STATE` 브로드캐스트(판매 tick·드러남 tick·바뀐 회복 tick), 세션 시작 순서(§5.1a), 새 메시지는 메시지 id 흐름(ADR-0006 §4a), 같은 tick 제출 순번, 배치에 T0 의 상태 쓰기 | server | `server/crates/sim/src/trading/`(신규, `price.rs` 제외), `server/crates/sim/src/simulation.rs`(명령 분기·세션 시작·드러남 tick 브로드캐스트), `server/crates/sim/src/lib.rs`, `server/crates/sim/tests/trading_*.rs`(신규) | S1, T0 | AC-3, AC-5(a)(d)(d2)(e)(g), AC-8, AC-10, AC-11(a)(b) | "쿨다운 중 + 도킹 밖 → `COOLDOWN_ACTIVE`" (판정 순서) |
| S3b | **가격 규칙 순수 함수 + golden** — 디자인 대금 표 상수, 분할 무관(시드 출력), 단조, 유계, 대금 ≥ 양, 반복 수 카운터, 회복 식 = tick 별 시뮬레이션, `pricing_version` golden(가격 절 정규화 해시 + 고정 입력 대금 표) + 음성 대조 | server | `server/crates/sim/src/trading/price.rs`(S3 의 `trading` 모듈 아래 — 같은 담당자라 순서만 지킨다: S3b 가 먼저 `price` 를 만든다), `server/crates/sim/tests/pricing_golden.rs`·`tests/data/pricing_golden/`(신규) | S1 | AC-4 | Ferrosite 포화 0 에서 12,000 kg → 90,000 |
| S4 | **영속화** — 마이그레이션 **0004**(`wallets` 6·`market_states` 12, ADR-0016 §5 이름 그대로), 배치에 지갑·시장 행 비교 후 쓰기와 인벤토리 감소(행을 지우지 않는다 — 0 으로 갱신), 괴리 = 정지, 기동 시 지갑·시장 적재 → sim 초기값 | server-db | `server/migrations/0004_*.sql`, `server/crates/persistence/src/lib.rs`, `server/crates/persistence/tests/trading_*.rs`(신규). **`main.rs` 는 고치지 않는다** — 적재 함수를 내놓고, 기동 순서에 끼우는 호출은 server 가 S2 에서 한다(T0 에서 함수 모양 합의) | T0, S3 | AC-5(c)(f), AC-6, AC-7(d), AC-11(c)(d), AC-21(e) | 지갑 행을 SQL 로 바꾼 뒤 판매 배치 커밋 → 정지 경로 (DB 통합) |
| S5 | **게이트웨이** — `SELL_MINERAL` 을 상태 변경 명령(`RECORDING_BACKLOG`)에, 새 메시지 라우팅, `commands_rejected_total{INSUFFICIENT_QUANTITY}` | server | `server/crates/gateway/**` | S1, T0 | AC-7(a)(b)(c) | `recording_lag` 21 에서 `SELL_MINERAL` → `RECORDING_BACKLOG`, 같은 상태 `SET_SHIP_CONTROL` → 수락 |
| H1 | **러너 통과 테스트** H-30~H-33 — 코드 변경 없음 예상(실패하면 그것이 발견 — architect·server 에 알린다) | history | `server/crates/persistence/tests/history_runner.rs` | S1(계약 타입), S4(0004 가 testdb 마이그레이션에 포함) | AC-12 | H-30: 판매 3 행 뒤 커서가 마지막 판매 좌표 이상 |
| C1 | **계약·데이터 사본** — 생성기 실행(신규 5 DTO), EditMode fixture 발견 57·왕복 45(예측 — 실측으로 확정), §5.3 C# 열 실측 상수, **데이터 사본을 ADR-0015 규칙으로**(시장 파일 추가, 광맥 공개부 v2 교체, `deposit-reserves/`·`history/rules/` 제외), **SC-50 개정**(레지스트리 `data_files`·`consumers` 에서 기대 집합 유도, 독립 순회 두 번, 음성 대조), 광맥 로더가 v2 를 읽는다 | client | 위 소유권 표 C1 행 | 계약 ✓ | AC-13(a)(b), AC-14 | SC-50 이 레지스트리 유도 집합으로 바뀐 뒤 `history/rules/mineral-discovery.json` 이 사본에 있어 RED |
| C2 | **판매 표시 로직(순수)** — UI-01~UI-13 의 ① 표시 로직(스펙 AC-15 표), 예상 대금 함수(디자인 §3.3 — AC-4(a) 와 같은 상수표로 단언), 시장·지갑 상태 미러(서버 메시지로만 바뀜), `Pilot-xxxx` 재사용 | client | `Scripts/Trading/**`, `Tests/EditMode/Trading*.cs` | C1 | AC-15 ①, AC-16(a) | 예상 대금(Ferrosite 포화 2,000, 2,000 kg) = 18,000 |
| C3 | **그레이박스 판매 화면** — UI-01~UI-13 를 씬에, 키 ↑↓←→ Enter(패널 열림일 때만), `SELL_MINERAL` 송신, 거절 문장, PlayMode 존재 단언(분모 = 기대 개수), 기존 바인딩 무충돌 테스트, `03_client_impl.md` 의 "사람이 확인할 항목" 에 UI 별 체크 문장·키 | client | 위 소유권 표 C3 행 | C2 | AC-15 ②③, AC-16(b), AC-17 | 패널이 닫혀 있을 때 Enter → 송신 0 |
| Q1 | **스프린트 계약** — AC-1~AC-23 을 항목으로, 화면 요소는 UI 마다 자동 항목 둘 + 사람 체크 한 줄(screen-elements S2), ⊘ 마다 **도달 가능성 계산**(R4 — 예: AC-20 바닥 도달), 테스트 이름은 실제 cargo/Unity 필터로 잡히는 형태, R3 증거 형식을 계약 §0 에, H-30~H-34·T-1~T-12 매핑 | qa | `_workspace/p1-03-trading/02_sprint_contract.md` | 스펙 | — | — |
| Q2 | **도구** — `validate_data_files.py` 를 레지스트리 `data_files` 유도로(+ 0·2 종 걸림 FAIL, 음성 대조), 원장·질량·화폐·포화 재계산 도구(판매 0 이면 FAIL), `constraint_census.py` 에 ADR-0016 표, `round_configs/p1-03-trading.json`, `gates.yml` 출처 게이트를 p1-03 계약으로 | qa | `tests/e2e/**`, `.github/**` | Q1 | AC-6(a), AC-13(c)(d), AC-21, AC-22 | 판매 0 건 DB 에서 원장 도구가 FAIL 인쇄 |
| Q3 | **봇** — `SELL_MINERAL` 송신, 세 메시지 수신, 거래소 항법(도킹·감속), T-1~T-11(같은 tick 겨냥, 흔적 시나리오의 별도 프로세스 B) | qa | `tools/bots/**` | S1 | AC-18, AC-19, AC-20 | — |
| Q4 | 실행 평가 — `run_round.py` 한 번으로 r1, 리포트 | qa | `_workspace/p1-03-trading/04_*`, `evidence/**` | 전부 + AC-23 사전 세션 | 전부 | — |
| L1 | **사람 사전 세션**(AC-23) — 화면 모듈(C3) 완료 뒤 r1 동결 전 | 리더 | `evidence/presession/notes.md` | C3, S3·S4·S5 | AC-23 | — |

**순서**: T0 ∥ S1 ∥ S3b ∥ C1 ∥ Q1 → S2 ∥ S3 ∥ C2 ∥ Q2 ∥ Q3 → S4 ∥ S5 → H1 ∥ C3 → L1 → Q4.

**임계 경로는 S1 → S3 → S4 → H1** 이다. S4 는 S3 의 배치 모양을 쓰고, H1 은 0004 가 테스트 DB 에 들어간 뒤에야 판매 행을 커밋할 수 있다. S3b 와 C2 는 디자인 상수표만으로 독립 진행한다 — **같은 상수표**(작업 전 주의 5)를 쓰므로 서버 대금과 클라이언트 미리보기가 같은 입력에서 같은 값을 내는지가 경계면 검사가 된다(AC-15 UI-08).

## 경계면 — 한쪽만 알면 버그가 되는 것

| 경계 | 생산 | 소비 | 합의 위치 |
|------|------|------|----------|
| `MINERAL_SOLD` payload·`causation_id`·`pricing_version` | S3 | S4(저장), H1(통과), Q2(원장·재계산) | 계약 |
| 배치의 지갑·시장·인벤토리 감소 쓰기(기대값 표현) | S3 | S4 | T0 |
| 기동 적재 → sim 초기값(지갑·시장·마지막 거래) | S4 | S3 | T0 |
| 응답 순서 `COMMAND_RESULT`→`SALE_RECEIPT`→`INVENTORY_STATE`→`WALLET_STATE` | S3 | C3, Q3 | 레지스트리 `responses` |
| 세션 시작 5 메시지 순서 | S3 | C3, Q3, history(ADR-0014 §6 합) | 스펙 §5.1a |
| `MARKET_STATE` 상장 = 드러난 광물만 | S3 | C2(UI-06), Q2(누출 검사) | 스펙 I-86 |
| 가격 규칙(구간 걷기) | S3b(서버) | C2(미리보기), Q2(재계산) | 디자인 §3.3 + 상수표 |
| 사본 범위 = 레지스트리 `consumers`·`data_files` | architect(레지스트리) | C1(SC-50), Q2(검증 도구), S2(서버 적재 경로) | ADR-0015 |

## 만기 (날짜가 아니라 게이트)

| 미뤄둠 | 만기 | 만기가 지나면 |
|---|---|---|
| **데이터 배포 경로 ADR**(p1-02 만기 표 2행) | **ADR: 이번 Phase 2 에서 작성 — 기계적 판정은 이미 비-통과**(`00_request.md` 생성 시점에 없었다). **닫힘: 사본의 서버 전용 필드 0**(AC-13(b)) | 이 슬라이스 r1 에서 AC-13 FAIL = 만기 미해소로 리포트 요약에 비-통과 |
| `HISTORICAL_EVENT_NOTICE.historical_event` 판별 union | 두 번째 역사 타입이 계약에 들어오는 시점 — **이번에 발동하지 않음**(새 역사 타입 없음) | 그대로 |
| 설계 적재 상한(p1-02 만기 표) | "적재 상한 또는 거래가 들어오는 슬라이스" — **거래가 들어왔지만 상한은 넣지 않는다**(스펙 Q2). 정지 경로 #4 행은 다시 셌다(ADR-0016 §5 — 인벤토리 + 지갑) | 적재 상한을 넣는 슬라이스(함선 구매와 함께)에서 `CAPACITY_EXCEEDED` 판정 위치를 앞당긴다 |
| 지갑 하한 0 의 ②(잔액 부족 거절) | **크레딧 차감 경로가 생기는 슬라이스** | 그 슬라이스의 판정 표에 잔액 부족 사유 + ADR-0016 §5 표의 `wallets_balance_cr_check` ① 을 "예" 로 다시 매긴다 |
| `MARKET_CRASH` 역사 사건 | p2 Chronicle·Herald + 집계 규칙 ADR | 스펙 §3 표 |
| ~~ADR-0016 accepted~~ | **닫힘 2026-10-05** — server T0 검토 동의 | — |
| `processed_commands` 전량 적재(ADR-0013) | 한 월드 100 만 행 — 판매가 같은 장부를 쓰므로 증가 속도가 늘었다(판매는 운반 왕복에 묶여 채굴보다 드물다) | 적재 창 ADR |
| 기존 메시지 id 를 메시지 흐름으로(ADR-0006 §4a) | 이동 golden 이 물리 변경으로 BLESS 되는 슬라이스 — 이번 아님 | 그대로 |

## 구현자가 architect 에게 가져올 것

계약 모양이 구현과 맞지 않으면 추측으로 맞추지 말고 SendMessage 로 요청한다. 수정 후 architect 는 레지스트리의 **모든** 생산자·소비자(server·client·bots(qa), 해당 시 history)에게 알린다. 예상되는 것:

- serde 로 반례 중 하나가 통과(§5.3 Rust 열) — serde 타입의 구멍이거나 스키마가 과한 것. 어느 쪽인지 architect 가 판정한다.
- `MARKET_STATE.last_sale` 의 C# 처리(`Required.AllowNull` 중첩 객체 — 생성은 실측했지만 역직렬화·키 누락 반례는 client 실측).
- 0004 에 표 밖 제약이 필요해 보이는 경우(예: `last_sale_quantity_kg <= ...`) — 넣기 전에 요청.

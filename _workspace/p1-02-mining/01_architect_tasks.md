# p1-02-mining 태스크 분해

- 작성: architect, 2026-09-27
- 스펙: `docs/specs/p1-02-mining.md` (**agreed** — 사용자 결정 Q1~Q4 전부 추천안, 2026-09-27. Q5 회복 포함은 리더 판단)
- ADR: `docs/adr/0013`(**accepted** — 사용자 결정 Q1·Q2, server 검토는 개정으로 받는다), `docs/adr/0014`(accepted — history 검토 반영). 개정: 0006 §4, 0007 §1
- 입력: `docs/design/p1-02-mining-design.md`(designer — 수치·시나리오 S-1~S-10), `01_history_review.md`(history — 규칙·테스트 H-01~H-15·오라클 SQL)
- **Phase 2 산출: 계약·ADR 확정.** 다음은 qa 스프린트 계약 → 구현(Sonnet + TDD).

## 작업 방식 (p1-01 과 같다)

**구현자는 TDD 로 작업한다.** 태스크마다 **"먼저 쓸 실패 테스트"** 를 한 줄로 지정했다. 그 테스트를 먼저 쓰고 **실패하는 것을 확인**하고(빨간불 로그를 구현 요약에 남긴다) → 최소 구현 → 정리.

**계약이 코드보다 앞서 있다.** 지금 `cargo test -p starfall-contracts` 와 Unity EditMode 는 **빨간불이 정상**이다(레지스트리 13 → 23, fixture 27/34 → 46/74). architect 실측 기준선: `check_contract_coverage.py --strict` **types 23 / errors 25 / warnings 0** — 0 이 되는 것이 AC-19(a).

## ⚠ 시작 전에 알아야 할 것

1. **발견을 재는 모든 실행은 새 월드에서 한다**(스펙 I-70, §10-9). 기본 월드에서 광물이 한 번 발견되면 다시는 발견되지 않고 `domain_events` 는 지우지 않는다. `INSERT INTO worlds (world_id, name, tick_hz, calendar_epoch, calendar_scale, sim_version, last_tick) VALUES ('<새 UUIDv7>', 'qa-<실행>', 20, '3800-01-01T00:00:00Z', 60, 1, NULL)` 뒤 `STARFALL_WORLD_ID=<그 id>`.
2. **`docker compose down -v` 금지**(CLAUDE.md). 역사 테이블도, 광맥 상태도 지우지 않는다. 재구축 테스트는 **복사한 로그**로 별도 스키마/DB 에서.
3. **기존 이동 golden(`server/crates/sim/tests/data/replay/`)은 한 바이트도 바뀌면 안 된다**(AC-7(b)). 채굴은 물리를 건드리지 않는다. 바뀌었다면 `BLESS` 하지 말고 원인을 찾는다.
4. **테스트의 기대 숫자는 `contracts/fixtures/` 에서만 온다**(p1-01 §11-2). `data/` 는 AC-2(기동 경로)와 사람 세션에만.
5. **`server/crates/persistence/src/lib.rs` 의 `commit()` 은 `recorded_at` 을 못 만들면 이벤트를 `continue` 로 건너뛴다** — 경제 상태가 들어오면 "수량은 커밋, 기록은 누락" 이 된다. S4 가 고친다(ADR-0013 §5, AC-5(d)). 그리고 **제약 위반 시 "그 배치만 포기하고 계속"** 도 월드 정지로 바뀐다.
6. **codegen 은 수정 없이 동작한다**(architect 실측: 신규 6 DTO, 기존 10 바이트 동일, `ContractTypes.cs` 만 변경). p1-01 과 달리 생성기 확장이 선행 조건이 아니다.
7. `data/history/rules/mineral-discovery.json` 은 designer 가 만들었고 스키마 오류 0 이다(D1 완료). **값을 바꾸려면 `rule_version` 을 올려야 한다** — H1 golden 이 그것을 강제한다.

## 파일 소유권

| 경로 | 소유 | 비고 |
|------|------|------|
| `contracts/**`, `docs/{adr,specs}/**` | architect | 변경은 SendMessage 로 요청 |
| `data/**`, `docs/design/**` | designer | |
| `server/Cargo.toml`, `server/Cargo.lock` | server | **T0 에서 history 크레이트 멤버·의존을 server 가 추가한다.** 이후 history 가 자기 크레이트의 의존을 바꿔야 하면 server 에 요청 |
| `server/crates/{contracts,sim,gateway}/**`, `server/bins/**` | server | |
| `server/crates/persistence/src/lib.rs` 및 history 외 파일 | server | |
| `server/migrations/0002_*.sql` | server | 인벤토리·광맥·처리 장부 |
| `server/crates/history/**` | history | 순수 코어(ADR-0014 §1) |
| `server/crates/persistence/src/history.rs`(및 `history/**`) | history | 러너·역사 저장소(ADR-0014 §1). `lib.rs` 에는 `mod history;` 한 줄 — T0 에서 server 가 넣는다 |
| `server/migrations/0003_*.sql` | history | 역사 테이블 |
| `client/Assets/_Project/Scripts/Contracts/Generated/**` | 생성물 | 손으로 고치지 않는다 |
| `client/Assets/_Project/{Scripts,UI,Tests}/**`, `tools/codegen/**` | client | 새 코드는 `Scripts/Mining/**`·`Scripts/History/**`(asmdef 는 ADR-0001 §3 이름 규약) |
| `tools/bots/**`, `tests/e2e/**`, `.github/workflows/**` | qa | |

**두 담당자가 같은 파일을 고치지 않는다.** 겹칠 수밖에 없는 자리(러너 기동, 커밋 알림, 알림 중계)는 **T0 가 인터페이스를 먼저 정한다.**

## 태스크

| ID | 태스크 | 담당 | 수정 경로 | 선행 | 수용 기준 | 먼저 쓸 실패 테스트 |
|----|-------|------|---------|------|----------|-----------------|
| **T0** | **경계 인터페이스 합의·골격** — (1) `server/Cargo.toml` 에 `crates/history` 멤버 + `uuid` `v5` feature(history 만), 빈 `starfall-history` lib. (2) 영속화 → 러너 **커밋 알림** 채널 타입(`tick` 하나). (3) 러너 → 게이트웨이 **역사 기록 채널** 타입(기동 시 목록 1회 + 이후 LIVE 1건씩) 과 게이트웨이의 직렬화 문맥(ADR-0014 §6 누락 틈 없음). (4) 기동 순서(ADR-0013 §7: 마이그레이션 → 상태 적재 → tick 재개 → 러너 재구축·목록 전달 → 연결 수락). (5) **테스트 전용 크레이트 `starfall-testdb`**(계약 §3.4, 리더 Q-2): 테스트마다 `starfall_test_<uuidv7>` DB 생성 → 0001~0003 마이그레이션 → `DROP`, 접두사가 아니면 패닉(증거 DB 보호, SC-114), `STARFALL_DB_TESTS=required` 모드, `STARFALL_DB_TEST RAN|SKIPPED <이름>` 줄. **S4·H2 의 DB 테스트 전부가 이것에 선다.** 결과를 `_workspace/p1-02-mining/02_interface.md` 에 server·history 공동으로 적는다 | server (history 합의) | `server/Cargo.toml`, `server/crates/testdb/**`(신규), `server/crates/history/Cargo.toml`·`src/lib.rs`(빈 골격), `server/crates/persistence/src/lib.rs`(`mod history;` 와 알림 송신 자리), `server/bins/game-server/src/main.rs`(기동 순서) | 계약 확정 ✓ | — (인터페이스) | `cargo test --workspace` 가 새 멤버로 빌드된다 + 기동 순서 테스트: 러너가 목록을 넘기기 전에는 `/ws` 가 연결을 받지 않는다 |
| S1 | 계약 Rust 타입 — 신규 10 타입·primitives 3·`reason_code` 7·레지스트리 `responses`, fixture 테스트 46/74, 레지스트리 23 | server | `server/crates/contracts/**` | — | AC-8 | `registry_consistency` 23 기대로 바꿔 RED 확인 → 타입 추가 |
| S2 | 데이터 4종 로딩과 §4.7 유도값 검산, 기동 거부 로그(파일·필드), `/debug/stats` 에 광물·광맥 수·`rule_version` | server | `server/bins/game-server/src/data.rs`, `server/crates/contracts/src/data.rs` | S1 | AC-2 | 광맥이 없는 광물을 넣은 `STARFALL_DATA_DIR` 로 기동 → 거부를 기대하는 테스트 |
| S3 | **sim 채굴** — 판정 순서(§4.2), 인벤토리 **checked 덧셈** → `CAPACITY_EXCEEDED`(디버그·릴리스 같은 경로, panic 없음 — ADR-0013 §5a), 월드 범위 지속 중복 기억 + 세션 기억(초기값 주입 가능, ADR-0013 §3 K1), 영속화용 `state_writes{expected, new}` 생산(K3), 쿨다운, 사거리·속도 제곱 비교, 부분 산출, 드러남, 게으른 회복(I-69), `MINERAL_MINED`(causation = command_id), `INVENTORY_STATE`(응답)·`DEPOSIT_FIELD_STATE`(브로드캐스트) 생산, 같은 tick 은 제출 순번, 세션 시작 시 두 메시지 | server | `server/crates/sim/src/mining/**`(신규), `server/crates/sim/src/simulation.rs`(명령 분기·세션 시작) | S1 | AC-3, AC-4(a)(d)(d2)(g), AC-7, AC-9 | "쿨다운 중 + 사거리 밖 → `COOLDOWN_ACTIVE`" (판정 순서) |
| S4 | **영속화** — 마이그레이션 0002(인벤토리·광맥·`processed_commands`, `world_id` 키, **제약은 ADR-0013 §5a 표의 요구 집합 그대로** + `domain_events` 에 `CHECK (jsonb_typeof(payload)='object')`), tick 배치에 상태 행·장부 포함, **배치 멱등**(`last_tick` 잠금 비교), **비교 후 쓰기**, 복구 불가 실패 → 정지(`persist_fatal_total`), `recorded_at` 건너뛰기 제거, 기동 시 상태 적재 → sim 초기값 | server | `server/migrations/0002_*.sql`, `server/crates/persistence/src/lib.rs`(history 모듈 제외), `server/bins/game-server/src/main.rs` | T0(`starfall-testdb` 포함), S3 | AC-4(c)(e)(f), AC-5, AC-18(b) | 이미 커밋된 tick 배치를 다시 커밋 → 행 변화 0 (DB 통합) |
| S5 | **게이트웨이** — `RECORDING_BACKLOG`(상태 변경 명령, `recording_lag` > 20 tick — ADR-0013 §6 K2), 역사 기록 채널 수신 → LIVE 브로드캐스트·BACKFILL 목록 보관(같은 직렬화 문맥), `SESSION_READY` 뒤 BACKFILL 송신 | server | `server/crates/gateway/**` | T0, S1 | AC-6, AC-13(a)(b)(d) | 백로그 21 tick 에서 `MINE_RESOURCE` → `RECORDING_BACKLOG`, 같은 상태에서 `SET_SHIP_CONTROL` → 수락 |
| H1 | **역사 코어** — `mineral-discovery@1`(규칙 파일 값 + Rust 조건), UUIDv5 id(fixture 의 id 재현), 역행 입력 거부, 판정 불가 fail-stop, 멱등·배치 경계 불변, golden(규칙 파일 해시 + 고정 입력 출력) | history | `server/crates/history/**` | T0, S1(계약 타입), D1(golden 만) | AC-1(history 부분), AC-10 | `fixtures/MINERAL_DISCOVERED/starfall-glass.json` 의 `historical_event_id` 를 `fixtures/MINERAL_MINED/first-extraction.json` 에서 재현 |
| H2 | **러너·역사 저장소** — 마이그레이션 0003(`historical_events`·`historical_event_sources`·`evidence` 추가 전용 트리거, `CHECK fact_status`, `UNIQUE(world_id, event_type, dedupe_key)`, 가변 `history_cursor`), 커서·워터마크 읽기, 한 트랜잭션 쓰기, 충돌 = 정지, 기동 재구축, NOTICE 생산(LIVE 는 커밋 뒤), 재생 도구(읽기 전용) | history | `server/migrations/0003_*.sql`, `server/crates/persistence/src/history.rs`(및 `history/**`) | T0, H1, S4(0002 적용 순서) | AC-11(a)~(h), AC-12, AC-13(c)(e), AC-18(e) | 역사 테이블에 행이 **있는 상태에서** UPDATE → 예외 (0 행 통과 방지) |
| D1 | ~~`data/history/rules/mineral-discovery.json` 작성~~ **완료**(스키마 오류 0) | designer | `data/history/rules/**` | 계약 ✓ | AC-2(a), AC-10(i) | — |
| C1 | 생성기 실행·EditMode — 신규 DTO, fixture 발견 46·왕복 36(예측 — 실측으로 확정), 모르는 닫힌 값 생존, LIVE·BACKFILL 중복 제거 | client | `Scripts/Contracts/Generated/**`(생성), `Tests/EditMode/**` | 계약 ✓ | AC-14(a)(b)(e) | `ContractFixtureTests` 의 발견 수 상수를 46 으로 바꿔 RED |
| C2 | **`data/` 재복사 — 새 7 파일**(minerals 4·deposits·mining-rules·history/rules)을 `client/Assets/_Project/Data` 로. **지금 SC-50(`ClientDataCopy_MatchesRepositoryOriginal`)은 빨간불일 것이다**(designer 가 소스로 판단, 미실행). **CI 는 EditMode 를 돌리지 않는다** — 이 슬라이스 PR 의 병합 조건은 사람이 돌린 EditMode 결과 파일(AC-19(c3)). + 로더 — 광맥 표에서 **id·이름·위치·반지름만** 읽는다(광물·매장량을 읽는 코드 없음), 광물·채굴 규칙 로딩, 사본 동일성 테스트 확장 | client | `Scripts/Greybox/GreyboxDataLoader.cs`, 사본 위치(ADR-0012 §7), `Tests/EditMode/**` | C1 | AC-14(f) | 사본의 광물을 바꿔도 표식이 `DEPOSIT_FIELD_STATE` 를 따른다 |
| C3 | 그레이박스 채굴 — 광맥 표식(미확인/드러남·잔량·최초 발견자), 채굴 가능 표시(표시용), `MINE_RESOURCE` 전송, 인벤토리 패널(`INVENTORY_STATE` 만), 거부 사유 문장, 산출 알림 ≠ 발견 배너, 발견 목록 `N / 4`, `Pilot-xxxx` 표지, "한발 먼저"·"N분 전 발견" 표현 | client | `Scripts/Mining/**`, `Scripts/History/**`(신규), `Scripts/Greybox/GreyboxSession.cs`(연결부), `UI/**` | C1, C2 | AC-14(c)(d) | 인벤토리 패널이 `INVENTORY_STATE` 수신 전에는 바뀌지 않는다(EditMode) |
| Q1 | **스프린트 계약** — **qa 계약 후보로 넘기는 항목: "읽히지 않는 데이터 파일 0"**(스펙 AC-19(c2) — 리더 제안, 판단은 qa 와 함께. 분모 = `data/**/*.json` 10 파일, 파일마다 읽는 코드 × 게이트 짝) — `02_sprint_contract.md` — AC-1~AC-19 를 항목으로, p1-01 §7b 규칙 1~9 적용(자명 통과 상태·판정 기준 선기록·출처·만기), history H-01~H-15·designer S-1~S-10 매핑. **"읽히지 않는 데이터 파일 0" 항목(AC-19(c2))** — 파일마다 읽는 코드와 게이트의 짝. **Unity EditMode 는 CI 밖이므로 PR 템플릿 수동 체크 + 증거 파일로 병합 조건화(AC-19(c3)).** **`.github/workflows/gates.yml` 출처 게이트를 이 계약 경로로** | qa | `_workspace/p1-02-mining/02_sprint_contract.md`, `.github/workflows/gates.yml` | 스펙 | AC-19(d) | 게이트가 p1-02 계약 항목 수를 분모로 찍는다(p1-01 의 90 이 찍히면 FAIL) |
| Q1b | **CI 출처 게이트 경로 교체** — `.github/workflows/gates.yml` 의 "출처 게이트 (계약 p1-01)" 단계가 `--contract _workspace/p1-01-ship-movement/02_sprint_contract.md` 를 가리킨다(100–101행). p1-02 계약 경로로 바꾸고 단계 이름도 고친다. **job 이름은 바꾸지 않는다**(필수 체크 이름 = job 이름 — p1-01 기술 부채). **계약 r2 §3.4 추가**: `server (rust)` job 에 `services: postgres: image: postgres:18.6-trixie` + env `STARFALL_DB_TESTS=required`·`DATABASE_URL` + SC-113 이름 대조 단계(`db_test_census.py`), PR 템플릿에 EditMode 증거·새 월드 id 두 줄. **architect 승인(qa Q-1)**: 이 슬라이스부터 verdict 라벨에 슬라이스 표지(`"p1-02 SC-nn"`, Rust `ContractRef::Verdict("p1-02 SC-nn")`), 표지 없는 라벨 = p1-01(소급 없음), 게이트 `--slice`, judgment job 안에 단계 둘(p1-02 판정 + p1-01 회귀), selftest 규칙 6 대조 4건. 같은 태스크에서 `rust_case_refs.py` ⑤ 유령 지명 검출기 사망(known 에 든 토큰만 돌려줘 늘 0) 수정 | qa | `.github/workflows/gates.yml`, `.github/pull_request_template.md`, `tests/e2e/db_test_census.py` | Q1, T0(`starfall-testdb` 의 RAN 줄) | AC-19(d), SC-98·113 | 바꾼 직후 게이트 출력의 항목 분모가 p1-02 계약 항목 수인지 확인(90 이면 FAIL) |
| Q2 | 봇 — `MINE_RESOURCE` 송신, `INVENTORY_STATE`·`DEPOSIT_FIELD_STATE`·`HISTORICAL_EVENT_NOTICE` 수신, 시나리오 S-1~S-10(같은 tick 겨냥 포함), 새 월드 인자 | qa | `tools/bots/**` | S1 | AC-15, AC-16, AC-17 | — |
| Q3 | e2e — `validate_data_files.py` `TARGETS` 에 4 경로, 원장·보존 SQL, 오라클 교차, 누출 검사(양성 대조), 새 월드 생성 헬퍼, 기록 무결성 | qa | `tests/e2e/**` | 계약 ✓ | AC-9, AC-12(a), AC-18, AC-19(c) | 채굴 0 건 DB 에서 원장 도구가 FAIL 을 인쇄 |
| Q4 | 실행 평가 — 블록별 절차서, 리포트 | qa | `_workspace/p1-02-mining/0x_qa_report_*.md`, `evidence/**` | 전부 | 전부 | — |

**순서**: T0(경계 인터페이스 + `starfall-testdb`) ∥ S1 ∥ C1 ∥ Q1b ∥ D1 → S2 ∥ S3 ∥ H1 ∥ C2 ∥ Q3 → S4 → S5 ∥ H2 ∥ C3 ∥ Q2 → Q4.

**임계 경로는 S3 → S4 → H2** 다. H2 는 커밋된 `MINERAL_MINED` 가 있어야 통합 테스트가 의미가 있고(없으면 "기록 0 건" 이 자명 통과), S4 는 S3 의 이벤트 모양을 쓴다. H1 은 계약 fixture 만으로 독립 진행한다.

## 경계면 — 한쪽만 알면 버그가 되는 것

| 경계 | 생산 | 소비 | 합의 위치 |
|------|------|------|----------|
| `MINERAL_MINED` payload·`causation_id` | S3 | S4(저장), H1·H2(판정), Q3(오라클) | 계약 |
| 커밋 알림(tick) | S4 | H2 | T0 |
| 역사 기록 채널(목록 + LIVE) | H2 | S5 | T0 |
| `processed_commands` 적재 → sim 기억 | S4 | S3 | S3 이 초기값 주입 API 를 먼저 만든다 |
| 광맥 상태 적재 `(remaining, as_of_tick, first_extracted_tick)` → sim | S4 | S3 | 같음 |
| `DEPOSIT_FIELD_STATE` 의 "넷 다 null" 규칙 | S3 | C3, Q2·Q3(누출 검사) | 계약 description + I-68 |
| `Pilot-xxxx` 표지 규칙 | C3 | (사람 세션, 봇 로그) | 스펙 I-66 — 규칙을 바꾸면 봇 로그와 대조가 깨진다 |
| 규칙 파일 해시 → `rule_version` | D1 | H1 golden | ADR-0014 §5. **designer 가 값을 바꾸면 버전을 올려야 한다** — 안 올리면 H1 golden 이 실패하는 것이 설계다 |

## 만기 (p1-01 §7b 규칙 8 — 날짜가 아니라 게이트)

| 미뤄둠 | 만기 | 만기가 지나면 |
|---|---|---|
| `HISTORICAL_EVENT_NOTICE.historical_event` 를 판별 union 으로 | **두 번째 역사 이벤트 타입이 계약에 들어오는 시점** | 그 슬라이스의 계약 태스크가 `schema_version` 2 또는 codegen 확장을 선행한다 |
| **데이터 배포 경로 ADR**(사용자 결정 Q3 의 후속 — 클라이언트 사본에서 서버 전용 값을 뺀다. ADR-0012 재검토 조건 (d)) | **다음 슬라이스(p1-03) 착수 전** — 기계적 판정: p1-03 의 `00_request.md` 가 생기는 시점에 `docs/adr/` 에 이 ADR 이 `accepted` 로 있는가 | **분모 = 클라이언트 사본에 들어가는 서버 전용 필드 수. 지금 2**(`deposits/cradle.json` 의 `mineral_id`·`initial_reserve_kg` × 광맥 8행). ADR 없이 게이트를 지나면 리더의 p1-03 착수 보고에 **비-통과**로 올라온다. **ADR 이 있어도 분모가 0 이 되기 전에는 닫히지 않는다** — 결정은 쓰였는데 빌드가 여전히 정답을 싣는 상태가 "닫혔다"로 읽히지 않게. 담당 architect |
| `processed_commands` 전량 적재 | **한 월드 100 만 행** | 적재 창 ADR(ADR-0013 §3) |
| 정지 경로 제약 표(ADR-0013 §5a)의 **분모를 실측과 대조** — 0002·0003 이 생기면 `pg_constraint` 행 수가 표의 요구 수(경제 54 / 역사 50 — 2026-09-28 실측으로 확정)와 같은가 | **S4·H2 마이그레이션이 처음 적용되는 시점** | 다르면 FAIL — 검토 없는 새 제약이거나 빠진 제약. qa 계약 항목 후보 |
| 설계 적재 상한(`CAPACITY_EXCEEDED` 를 판정 앞쪽에서 쓰게 됨) | **적재 상한 또는 거래가 들어오는 슬라이스** | 정지 경로 표 #4 행을 다시 센다 |
| 기존 메시지 id 를 메시지 흐름으로 옮기기(ADR-0006 §4a) | **이동 golden 이 물리 변경으로 정당하게 다시 BLESS 되는 슬라이스** | 그 슬라이스의 server 태스크가 한 번에 옮기고 §4a 를 닫는다. 그 전에 옮기면 golden 이 물리 무관하게 바뀐다 |
| BACKFILL → 조회 API | **한 월드 PUBLIC 역사 32 건**(송신 큐 64 의 절반 — server 추가 3) | ADR-0014 §6 |
| 함선 클래스 2 개 이상(p1-01 이월) | `data/ships/*.json` 2 개 | p1-01 §7b 표 그대로 — **이번 슬라이스는 함선을 늘리지 않는다** |
| ADR-0004 본문에 증거 CSV LFS 패턴 반영(p1-01 이월) | **p1-02 슬라이스 종료 전** | architect 몫. 리포트 요약에 비-통과 |

## 구현자가 architect 에게 가져올 것

계약 모양이 구현과 맞지 않으면 **추측으로 맞추지 말고** SendMessage 로 요청한다. 수정 후 architect 는 레지스트리의 **모든** 생산자·소비자(server·history·client·bots(qa))에게 알린다. 특히 예상되는 것:

- serde 로 `DEPOSIT_FIELD_STATE` 의 "넷 다 null 또는 넷 다 값" 을 타입으로 표현하고 싶으면(`Option<Revealed>`) 와이어 모양은 그대로 두고 Rust 내부 표현으로 해결한다 — 와이어를 바꾸자는 요청이면 architect 에게.
- `HISTORICAL_EVENT_NOTICE` 의 envelope `tick` 을 게이트웨이가 어떻게 읽는지(현재 tick 원자값 — gateway 거부 응답과 같은 방식, ADR-0006 §4).

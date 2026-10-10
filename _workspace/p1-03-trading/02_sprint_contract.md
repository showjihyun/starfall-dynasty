# p1-03-trading 스프린트 계약

- 작성: qa3, 2026-10-05 (구현 착수 전 / Phase 3) — **agreed (2026-10-06, 리더 판정 합의)**
- 스펙: `docs/specs/p1-03-trading.md` (**agreed**, AC-1~AC-23, 불변식 I-74~I-93. 사용자 결정 Q1~Q3 추천안)
- ADR: `0015`(클라이언트 데이터 배포 — accepted) · `0016`(크레딧과 판매 장부 — accepted, server3 T0 검토 뒤). 기반 `0013` · `0014` · `0006` §4a · `0007` §1·§5
- 세부 절차 정본: `docs/design/p1-03-trading.md` §6(T-1~T-12)·§8.3(UI-01~UI-13 + 사람 체크 13 문장)·§9(지표) · `_workspace/p1-03-trading/02h_history_review.md` §2.3(H-30~H-34)
- 태스크: `01_architect_tasks.md` (T0 · S1~S5 · S3b · H1 · C1~C3 · Q1~Q4 · L1)
- 계약 데이터 (architect 실측, 스펙 §5.2): **스키마 36 / 유효 fixture 57 / 반례 113 / 레지스트리 타입 30(`registry_version: 5`)**, data 종류 9(`data_files` 필수)
- 합의: **server3 ☑ · serverdb3 ☑ · client4 ☑ · qa3 ☑ · architect3(판정) ☑ · history(H1) — architect3 이름 확인, H1 투입 시 history 확인** — §11 확인란. 리더 판정 합의 2026-10-06
- 항목 수: **SC-01 ~ SC-133 = 133** (첫 담당 기준 — server 38 · qa 34 · client 34 · 사람 14 · serverdb 8 · history 4 · 리더 1 = 133. 공동 행 15: SC-04·05·45·46·47 server·qa, SC-24 serverdb·server, SC-26 server·serverdb, SC-34~37·51 serverdb·qa, SC-55 history·qa, SC-98·131 사람·qa) + 기록 항목 **M-1~M-10**(판정 제외)

**이 문서의 구속력.** Phase 5 평가(`04_qa_report_r{N}.md`)는 이 표의 항목으로만 한다. 스펙 §7 의 AC 와 그 AC 가 세부 절차로 지정한 T-1~T-12·H-30~H-34·UI-01~UI-13 을 실행 가능한 관찰로 옮긴 것이고, **그 밖의 요구는 넣지 않았다.** 스펙 불변식 중 AC 가 없는 것은 항목으로 만들지 않는다(§11 쟁점으로 묻는다). 평가 중 발견한 그 밖의 문제는 리포트의 "계약 외 발견" 에 적고 판정에 쓰지 않는다.

**p1-02 에서 그대로 가져온 것**: §0.3 판정 칸, §8.2 규칙 1~9(p1-01 §7b 원문), 새 월드 절차, 슬라이스 표지(`p1-03 SC-nn`), `[DB]` 표식, 반례 층별 표의 독립 기대 열 규칙. **가져오지 않은 것**: SC 번호. 이 계약의 SC-30 은 p1-02 의 SC-30 과 아무 관계가 없다.

---

## 0. 공통 실행 전제

### 0.1 환경

```bash
export PATH="$HOME/.cargo/bin:$HOME/.dotnet/tools:/c/Users/CHOISOOYEON/AppData/Local/Unity/bin:$PATH"
```

- cargo 명령은 `server/`(봇은 `tools/bots/`)에서, 나머지는 레포 루트에서. `.env` 에 `STARFALL_DEV_AUTH_SECRET` 이 있다 — **값을 증거·로그·메시지에 찍지 않는다.**
- SQL: `docker compose exec -T postgres psql -U starfall -d starfall -At -F' | ' -c "..."`. **모든 SQL 은 `world_id` 를 조건에 넣는다.** 판정 도구의 SQL 은 SELECT/WITH 만(세션 read-only — `run_round.py` 규약).
- 서버는 stdin 에 `shutdown` 한 줄로 끝낸다. 하드 킬이 필요했다면 그 사실 자체가 평가 대상이다. 예외: SC-34·35 는 서버가 **스스로** 멈추는 경로다(비교 후 쓰기 괴리 → 정지, ADR-0013 §5).
- `docker compose stop postgres` / `start postgres` 는 SC-39 에서만(사람 단계 `db_stop`). **`down -v`·`system prune`·`volume prune` 금지.**
- Unity 프로젝트는 단일 인스턴스다. Editor 가 열려 있으면 `unity test` 가 돌지 않는다(E3).
- golden 재생 파일은 손으로 고치지 않는다. 자식 환경에서 `STARFALL_REPLAY_BLESS` 를 지운다.

### 0.2 새 월드 — **실서버를 띄우는 모든 실행의 첫 단계** (스펙 §4.9, §10-14)

이번 슬라이스는 두 이유로 **실서버 실행 전부**를 새 월드에서 한다(§1 `불가` 칸의 `W`).

1. **포화·마지막 거래·지갑은 월드마다 영속한다**(I-88). 두 번째 실행부터 시세가 기준가에서 시작하지 않아 기대 대금이 틀리고(환경 때문의 빨간불), 이전 실행의 마지막 거래를 이번 흔적으로 오독한다(공짜 초록불). 상장은 드러남을 따르므로 "첫 채굴 → 상장" 도 새 월드에서만 관측된다.
2. **증거 DB `starfall` 의 기존 월드 행에 쓰지 않는다**(리더 지시 — 새 월드 INSERT 만).

**절차** — `python tests/e2e/new_world.py --tag <SC 또는 라운드 태그> --evidence <경로>` (p1-02 도구 그대로):

0. **마이그레이션 동결 선행 확인** (ADR-0007 §5, p1-02 §0.2-0 승계). 0004 가 생기면 증거 DB 로 서버를 띄우는 순간 적용되고 그 파일은 동결된다. `server_boot.py` 의 모든 `spawn` 이 `_sqlx_migrations` 와 `server/migrations/*.sql` 을 대조하고, 미적용 파일이 있으면 **소유자의 동결 선언 줄**(`_workspace/p1-03-trading/migration_freeze.md` 에 `frozen: 0004 — serverdb3, <YYYY-MM-DD>`)이 있을 때만 띄운다(없으면 exit 4). 선언 위치를 증거(`migration_freeze`)에 싣는다. `server_boot.py` 는 이미 `_workspace/*/migration_freeze.md` 전부를 읽는다(`FREEZE_FILE_GLOB`, qa3 확인 2026-10-05). **현재: `0004_wallets_and_markets.sql` 은 디스크에 있고 선언 0 줄** — 증거 DB 로 서버를 띄우는 실행은 exit 4 로 멈추는 것이 정상이다(serverdb3 는 스크래치 DB 에만 적용했다 — `03_serverdb_impl.md` §1b).
1. UUIDv7 생성 → `worlds` INSERT(값은 p1-02 I-70 그대로, `name = 'qa-p1-03-<태그>-<YYYYMMDDHHMM>'`, `last_tick NULL`).
2. `STARFALL_WORLD_ID=<id>` 로 기동.
3. **증거**: 월드 id, INSERT 문 전문, INSERT 직후 `domain_events`·`wallets`·`market_states` 그 월드 행 수 = **0, 0, 0**(0004 적용 전이면 앞의 하나만 — 적용 여부를 같이 찍는다).

- 회복 경계는 **`⌊t / 1200⌋` 의 배수**다(I-87, 스펙 §10-15). 실서버 기대값은 **실제 tick 을 식에 넣어** 계산한다. "1 분 뒤" 손계산을 쓰지 않는다.
- 판매 쿨다운은 메모리 상태다 — 재기동 직후의 즉시 판매가 수락되는 것은 설계다(스펙 §10-18).

### 0.3 증거 기준

| 판정 | 조건 |
|------|------|
| **PASS** | 명령과 출력 요약(종료 코드 포함), 테스트 이름, 또는 파일:라인이 리포트에 있다. **그리고 그 행의 `⊘` 칸이 요구한 배제 단언이 같은 실행의 출력에 있다.** 정적 읽기만으로는 PASS 가 아니다 |
| **FAIL** | 기대 관찰이 나오지 않음. **간헐 실패도 FAIL**(비결정성 이슈로 기록, 재시도로 덮지 않는다). **구현이 없어 실행 못 한 것도 FAIL** |
| **미검증(환경)** | §7 의 E-조건. PASS 로 올리지 않는다 |
| **미검증(증거 요건)** | 관찰은 수행됐고 기대와 어긋나지 않았으나 `⊘` 배제 단언이나 분모가 출력에 없다. **비-통과**. 실패를 이 칸으로 옮기는 것은 금지 |
| **무효** | 겨냥한 조건이 이번 실행에서 만들어지지 않았다(예: SC-116 의 두 판매가 같은 tick 이 아님). 재실행하고 시도 횟수를 적는다. 슬라이스 종료 시 무효로 남으면 비-통과 |
| **대기** | 선행 태스크 미완(E9). 그 라운드 판정에서 제외 |
| **기록** | 판정하지 않는다(§6 M-n) |

**검사 건수 원칙.** 순회·집합 항목은 **실제로 순회한 개수**가 출력에 있어야 한다 — 결과의 분모와 판정 근거의 분모를 둘 다. **분모가 0 이면 FAIL 로 인쇄한다**(CLAUDE.md 검증의 규율). 항등식은 판매가 0 건이면 FAIL 이다(스펙 I-85 끝 문장).

**바이너리 출처.** 실서버·봇·Unity 로 판정하는 항목의 증거에 `git rev-parse HEAD` + `git status --porcelain` 공백 여부(더러우면 목록) + 서버 바이너리 sha256 앞 16자(`server_boot.py` 의 `source_provenance()`).

**판정 출처는 도구 라벨이다.** 도구가 내는 verdict 는 `"item": "p1-03 SC-nn …"`(Python) 또는 `ContractRef::Verdict("p1-03 SC-nn …")`(봇)로 시작한다. 이 계약이 그 항목에 **지명한 도구**만 그 번호로 verdict 를 낸다(§8.2 규칙 7). 관측용은 `ContractRef::Reference`/`"참고"`.

### 0.3a 모듈 완료 자기 점검 — **R3 증거 형식** (qa-throughput R3, screen-elements S3)

구현자는 "모듈 완료" 를 보내기 전에 **담당 SC 마다** 아래 다섯(화면 모듈은 일곱) 칸을 `03_{server,serverdb,client,history}_impl.md` 에 표로 남긴다. qa3 는 알림을 받으면 이 표를 **실행으로 다시 확인**한다(같은 명령을 qa 가 돌린다). 이 형식은 지금 정한다 — r1 에서 처음 따지지 않는다.

| 칸 | 요구 | 아닌 것 |
|---|---|---|
| ① 이름 실행 | §1 방법 칸의 명령·필터를 **글자 그대로** 실행 → `N passed` 의 N ≥ 1, 그리고 잡힌 테스트 이름 목록. **다른 SC 의 테스트가 섞이지 않는다**(qa 는 `cargo_sc_map.py run` 의 `also_named_by` 로 본다) | 다른 필터로 돌린 결과, 워크스페이스 전체 `cargo test` 의 초록 |
| ② 구현 변이 | **구현 코드**를 잠깐 망가뜨려(예: 판정 순서 두 줄 교환, `checked_sub` → 포화 뺄셈, 구간 경계 `+1`) 그 테스트가 **실패**함을 보인다. 변이 위치(파일:라인), 실패 출력 발췌, 원복 뒤 초록 | 기대값·단언을 바꾸는 것(그것은 테스트가 테스트를 재는 것이다) |
| ③ 원복 증명 | 변이를 건 파일마다 **변이 전·원복 후 sha256 이 같다**(`sha256sum <파일>` 두 줄) | `git diff` 공백 — 이 슬라이스 변경은 커밋 전이고 신규 파일은 untracked 라 증명이 안 된다 |
| ④ 도구 존재 | 계약이 지명한 바이너리·스크립트·카운터·`/debug/stats` 필드가 실제로 있다(경로 또는 출력 한 줄) | "추가 예정" |
| ⑤ DB 테스트 이름 | `TestDb::create("<계약 이름>")` 에 §1 의 `[DB]` 이름을 **글자 그대로**(모듈 경로 접두사 금지). stderr 의 `STARFALL_DB_TEST RAN <이름>` 줄 발췌 | 함수 이름만 맞추고 `create` 인자는 다른 것 |
| ⑥ (화면) 표시 로직·존재 단언 | 담당 `UI-nn` 의 EditMode·PlayMode 이름이 결과 xml 에 있고(① 과 같은 기준), 표시 로직을 망가뜨리면(② 와 같은 기준) 실패 | PlayMode 를 안 돌림 |
| ⑦ (화면) 사람 확인 항목 | "사람이 확인할 항목" 에 담당 `UI-nn` 의 **체크 문장 + 조작 키 + 그 장면을 만드는 동작** | "없음" — 화면을 건드린 모듈은 비울 수 없다 |

- 변이는 SC 마다 **하나 이상**. 여러 SC 를 한 변이로 덮으면 SC 마다 실패한 테스트 이름을 따로 적는다.
- 테스트가 계약 이름과 다르게 만들어졌으면 **코드가 아니라 계약을 고치자고 qa3 에 먼저 말한다**(p1-02 r3 의 이름 정합 왕복을 Phase 4 에서 끝낸다). qa3 가 계약을 고치고 §12 에 기록한다 — 판정 기준·⊘ 는 바꾸지 않는다.

### 0.4 하드 게이트와 기록의 분리

| 구분 | 대상 | 판정 |
|------|------|------|
| **정확성 하드 게이트** | SC-01 ~ SC-133 중 SC-117·121 제외 전부 | PASS / FAIL / 미검증 / 무효 |
| **성능 (판정)** | SC-117 `tick 초과 비율 ≤ 0.5 %` · SC-121 `역사 알림 지연 max ≤ 5 초`(H-34 실패 경계) | 같은 판정 칸 |
| **기록만** | M-1 ~ M-10 | 값이 안 나와도 FAIL 아님 |

### 0.5 반례 113건의 층별 거부 책임 (SC-43 의 채점 기준) — **구현 중 채운다**

규약은 p1-02 §0.5 그대로: ① 스키마 열 = architect 실측(113/113 거부). ② Rust serde 열과 ③ C# 열은 server(S1)·client(C1) 가 실측으로 채운다. **기대 열의 출처는 스펙 §5.3 의 "Rust serde (예측)"·"C# Strict (예측)" 열**이다(architect 가 반례 설계와 함께 쓴 것 — 테스트 출력이 아니다). 판정 대상은 두 대조다: (1) 기대 ≠ 실측인 칸, (2) 거부됐지만 **사유가 의도한 위반을 가리키지 않는** 칸. 어느 쪽이든 FAIL 이 아니라 **architect 통지**(§8.1).

- 이번 변경분 대상: 신규 반례 **39 + `DEPOSIT_FIELD` 순증 1**(스펙 §5.2 "반례 39(+ `DEPOSIT_FIELD` 반례 1 삭제·2 추가)"). 의도한 위반 열은 qa3 가 각 반례를 같은 타입의 가장 가까운 유효 fixture 와 필드 단위로 비교해 만든다(p1-02 `fxdiff` 방식) — **S1 완료 알림 전에** 이 절에 붙인다(§9 만기).
- C# 예측 합계(스펙 §5.3): **거부 53 / 감지 불가 33 / 계층 없음 27 = 113.**

**② 열 — 이번 변경분 42 행 (qa3 2026-10-06, S1 완료 알림 확인 때 채움)**

- 기대 열 출처: 스펙 §5.3 "Rust serde (예측)" 열(전부 "거부"). 의도한 위반 열: 반례마다 같은 타입의 유효 fixture 중 필드 차이가 가장 작은 것과 평탄화 비교(qa3 스크립트 — 테스트 출력과 독립). 실측 열: `cargo test -p starfall-contracts invalid_fixtures_layered -- --nocapture` 출력을 S1 `CONTRACT_TYPES` 순서 × 파일 이름 정렬과 한 줄씩 정렬 대조(113 행 전부 정렬 일치)한 뒤 이번 변경 9 타입 행만 옮겼다. **실측 열은 기대가 아니다.**
- 행 수 42 = 새 파일 40(`DEPOSIT_FIELD` 신규 2 + 그 밖 7 타입 38 — `reserve-fractional` 은 `DEPOSIT_RESERVE` 로 옮겨 새 파일) + `DEPOSIT_FIELD` 다시 쓴 2(`no-deposits`·`position-two-components`). 반례 수 74 → 113 의 순증 39 = 새 파일 40 − 지운 `DEPOSIT_FIELD/invalid/reserve-fractional.json` 1(스펙 §5.2 와 일치).
- **대조 (1) 기대 = 실측 42 / 42. 대조 (2) 사유가 의도한 위반을 가리킴 42 / 42.** 차이가 여럿인 행(`no-deposits`·`no-listings`·`layout-field-injected`·`position-*`)은 한 배열의 길이·원소 변화이고 메시지가 그 배열(`deposits`·`listings`·`position_m`)을 가리킨다. `WALLET_STATE` 세 행의 둘째 차이는 `message_id`(우연한 차이, 의도 아님) — 메시지가 `balance_cr` 범위·타입을 가리켜 의도와 일치.
- ③ 열(C#) **실측 (client4 C1, 2026-10-06)**: **거부 51 / 감지 불가 35 / 계층 없음 27 = 113** — 예측(스펙 §5.3) 53 / 33 / 27 과 **2 건 다르다**: `MINERAL_SOLD/proceeds-fractional.json`·`WALLET_STATE/fractional-credits.json` 은 예측 "거부(타입)" 였으나 C# `long` 필드가 소수 JSON 값을 받아 Strict 가 **수락**한다(`int` 필드인 `quantity_kg` 류는 거부). §0.5 머리 규칙대로 **FAIL 이 아니라 architect 통지**(client4 가 통지함) — 예측 열은 고치지 않고 이 실측 줄로 남긴다(실측으로 기대를 덮어쓰면 자기 일치가 된다, p1-02 §0.5 정정 선례). SC-61 의 C# 상수는 실측 51/35/27 로 단언하고, 예측과의 차 2 건은 M-6 기록 + architect3 판정(스펙 §5.3 예측 정정 여부)으로 넘긴다.
- **SC-43 은 ② 열로 판정**(③ 은 SC-61 의 몫).

| 타입 | 반례 | 의도한 위반 (가장 가까운 유효 fixture 대비) | ② 기대 (스펙 §5.3) | ② 실측 (S1, 2026-10-06) | 거부 사유 (serde 메시지) | 사유 ↔ 의도 |
|---|---|---|---|---|---|---|
| DEPOSIT_FIELD | `no-deposits.json` | `example-two-deposits.json` 대비 15 곳: deposits#len: 2→0; deposits[0].display_name: "Belt A"→"∅"; deposits[0].id: "belt-a"→"∅" … | 거부 | 거부 | deposits 는 1 ..= 64 개여야 한다 (받음: 0) | 일치 |
| DEPOSIT_FIELD | `position-two-components.json` | `example-two-deposits.json` 대비 4 곳: deposits[0].position_m#len: 3→2; deposits[0].position_m[0]: -3200.0→1.0; deposits[0].position_m[1]: 150.0→2.0 … | 거부 | 거부 | invalid length 2, expected an array of length 3 | 일치 |
| DEPOSIT_FIELD | `schema-version-1.json` | `example-two-deposits.json` 대비 1 곳: schema_version: 2→1 | 거부 | 거부 | schema_version 은 2 여야 한다 (받음: 1) | 일치 |
| DEPOSIT_FIELD | `server-only-mineral-field.json` | `example-two-deposits.json` 대비 1 곳: deposits[0].mineral_id: "∅"→"example-ore" | 거부 | 거부 | unknown field `mineral_id`, expected one of `id`, `display_name`, `position_m`, `radius_m` | 일치 |
| SELL_MINERAL | `actor-field-injected.json` | `basic.json` 대비 1 곳: actor_id: "∅"→"01a0b1c2-2c02-7b46-9c68-9abcdef01234" | 거부 | 거부 | unknown field `actor_id`, expected one of `command_id`, `command_type`, `schema_version`, `client_sent_at`, `p | 일치 |
| SELL_MINERAL | `balance-field-injected.json` | `basic.json` 대비 1 곳: payload.balance_cr: "∅"→1000000 | 거부 | 거부 | unknown field `balance_cr`, expected one of `market_id`, `mineral_id`, `quantity_kg` | 일치 |
| SELL_MINERAL | `missing-market-id.json` | `basic.json` 대비 1 곳: payload.market_id: "cradle-exchange"→"∅" | 거부 | 거부 | missing field `market_id` | 일치 |
| SELL_MINERAL | `position-field-injected.json` | `basic.json` 대비 4 곳: payload.position_m#len: "∅"→3; payload.position_m[0]: "∅"→0.0; payload.position_m[1]: "∅"→1500.0 … | 거부 | 거부 | unknown field `position_m`, expected one of `market_id`, `mineral_id`, `quantity_kg` | 일치 |
| SELL_MINERAL | `price-field-injected.json` | `basic.json` 대비 1 곳: payload.price_cr_per_kg: "∅"→99999 | 거부 | 거부 | unknown field `price_cr_per_kg`, expected one of `market_id`, `mineral_id`, `quantity_kg` | 일치 |
| SELL_MINERAL | `proceeds-field-injected.json` | `basic.json` 대비 1 곳: payload.proceeds_cr: "∅"→1000000 | 거부 | 거부 | unknown field `proceeds_cr`, expected one of `market_id`, `mineral_id`, `quantity_kg` | 일치 |
| SELL_MINERAL | `quantity-above-mass-max.json` | `basic.json` 대비 1 곳: payload.quantity_kg: 2000→2147483648 | 거부 | 거부 | invalid value: integer `2147483648`, expected i32 | 일치 |
| SELL_MINERAL | `quantity-fractional.json` | `basic.json` 대비 1 곳: payload.quantity_kg: 2000→1.5 | 거부 | 거부 | invalid type: floating point `1.5`, expected i32 | 일치 |
| SELL_MINERAL | `quantity-zero.json` | `basic.json` 대비 1 곳: payload.quantity_kg: 2000→0 | 거부 | 거부 | MassKg 는 이 필드에서 0일 수 없다 (최소 1) | 일치 |
| MINERAL_SOLD | `actor-id-null.json` | `first-sale-sells-all.json` 대비 1 곳: actor_id: "01a0b1c2-2c01-7a45-8b67-89abcdef0123"→null | 거부 | 거부 | invalid type: null, expected a string | 일치 |
| MINERAL_SOLD | `causation-id-null.json` | `first-sale-sells-all.json` 대비 1 곳: causation_id: "01a0b1c2-9c02-7d12-8e22-3f3140516172"→null | 거부 | 거부 | invalid type: null, expected a string | 일치 |
| MINERAL_SOLD | `importance-level-injected.json` | `first-sale-sells-all.json` 대비 1 곳: importance_level: "∅"→0 | 거부 | 거부 | unknown field `importance_level`, expected one of `event_id`, `event_type`, `schema_version`, `world_id`, `tic | 일치 |
| MINERAL_SOLD | `missing-floor-price.json` | `first-sale-sells-all.json` 대비 1 곳: payload.floor_price_cr_per_kg: 3→"∅" | 거부 | 거부 | missing field `floor_price_cr_per_kg` | 일치 |
| MINERAL_SOLD | `missing-pricing-version.json` | `first-sale-sells-all.json` 대비 1 곳: payload.pricing_version: "cradle-exchange@1"→"∅" | 거부 | 거부 | missing field `pricing_version` | 일치 |
| MINERAL_SOLD | `proceeds-fractional.json` | `first-sale-sells-all.json` 대비 1 곳: payload.proceeds_cr: 20000→20000.5 | 거부 | 거부 | invalid type: floating point `20000.5`, expected i64 | 일치 |
| MINERAL_SOLD | `proceeds-zero.json` | `first-sale-sells-all.json` 대비 1 곳: payload.proceeds_cr: 20000→0 | 거부 | 거부 | CreditAmount 는 이 필드에서 0일 수 없다 (최소 1) | 일치 |
| MINERAL_SOLD | `quantity-after-negative.json` | `first-sale-sells-all.json` 대비 1 곳: payload.quantity_after_kg: 0→-1 | 거부 | 거부 | MassKg 는 0 ..= 2147483647 범위여야 한다 (받음: -1) | 일치 |
| MINERAL_SOLD | `wallet-after-zero.json` | `first-sale-sells-all.json` 대비 1 곳: payload.wallet_after_cr: 20000→0 | 거부 | 거부 | CreditAmount 는 이 필드에서 0일 수 없다 (최소 1) | 일치 |
| WALLET_STATE | `above-safe-integer.json` | `zero.json` 대비 2 곳: message_id: "01a0b1c2-6c41-7122-9233-4a5b6c7d8ea0"→"01a0b1c2-6c41-7122-9233-4a5b6c7d8ea4"; payload.balance_cr: 0→9007199254740992 | 거부 | 거부 | CreditAmount 는 0 ..= 9007199254740991 범위여야 한다 (받음: 9007199254740992) | 일치 |
| WALLET_STATE | `fractional-credits.json` | `zero.json` 대비 2 곳: message_id: "01a0b1c2-6c41-7122-9233-4a5b6c7d8ea0"→"01a0b1c2-6c41-7122-9233-4a5b6c7d8ea3"; payload.balance_cr: 0→12.5 | 거부 | 거부 | invalid type: floating point `12.5`, expected i64 | 일치 |
| WALLET_STATE | `negative-credits.json` | `zero.json` 대비 2 곳: message_id: "01a0b1c2-6c41-7122-9233-4a5b6c7d8ea0"→"01a0b1c2-6c41-7122-9233-4a5b6c7d8ea2"; payload.balance_cr: 0→-1 | 거부 | 거부 | CreditAmount 는 0 ..= 9007199254740991 범위여야 한다 (받음: -1) | 일치 |
| SALE_RECEIPT | `balance-field-injected.json` | `basic.json` 대비 1 곳: payload.balance_cr: "∅"→20000 | 거부 | 거부 | unknown field `balance_cr`, expected one of `command_id`, `market_id`, `mineral_id`, `quantity_kg`, `proceeds_ | 일치 |
| SALE_RECEIPT | `missing-command-id.json` | `basic.json` 대비 1 곳: payload.command_id: "01a0b1c2-9c02-7d12-8e22-3f3140516172"→"∅" | 거부 | 거부 | missing field `command_id` | 일치 |
| SALE_RECEIPT | `proceeds-zero.json` | `basic.json` 대비 1 곳: payload.proceeds_cr: 20000→0 | 거부 | 거부 | CreditAmount 는 이 필드에서 0일 수 없다 (최소 1) | 일치 |
| MARKET_STATE | `last-sale-quantity-zero.json` | `one-crashed-one-fresh.json` 대비 1 곳: payload.listings[0].last_sale.quantity_kg: 14000→0 | 거부 | 거부 | MassKg 는 이 필드에서 0일 수 없다 (최소 1) | 일치 |
| MARKET_STATE | `missing-last-sale-key.json` | `one-crashed-one-fresh.json` 대비 1 곳: payload.listings[1].last_sale: null→"∅" | 거부 | 거부 | missing field `last_sale` | 일치 |
| MARKET_STATE | `price-zero.json` | `one-crashed-one-fresh.json` 대비 1 곳: payload.listings[1].price_cr_per_kg: 250→0 | 거부 | 거부 | CreditAmount 는 이 필드에서 0일 수 없다 (최소 1) | 일치 |
| MARKET_STATE | `saturation-negative.json` | `one-crashed-one-fresh.json` 대비 1 곳: payload.listings[0].saturation_kg: 14000→-1 | 거부 | 거부 | MassKg 는 0 ..= 2147483647 범위여야 한다 (받음: -1) | 일치 |
| MARKET_STATE | `unlisted-hint-injected.json` | `one-crashed-one-fresh.json` 대비 2 곳: payload.unlisted_minerals#len: "∅"→1; payload.unlisted_minerals[0]: "∅"→"cobaltine" | 거부 | 거부 | unknown field `unlisted_minerals`, expected `market_id` or `listings` | 일치 |
| MARKET | `floor-pct-zero.json` | `example-two-listings.json` 대비 1 곳: rules.price_floor_pct: 30→0 | 거부 | 거부 | 비율은 1 ..= 100 이어야 한다 (받음: 0) | 일치 |
| MARKET | `missing-pricing-version.json` | `example-two-listings.json` 대비 1 곳: pricing_version: "example-exchange@1"→"∅" | 거부 | 거부 | missing field `pricing_version` | 일치 |
| MARKET | `no-listings.json` | `example-two-listings.json` 대비 9 곳: listings#len: 2→0; listings[0].band_kg: 2000→"∅"; listings[0].base_price_cr_per_kg: 10→"∅" … | 거부 | 거부 | listings 는 1 ..= 64 개여야 한다 (받음: 0) | 일치 |
| MARKET | `price-fractional.json` | `example-two-listings.json` 대비 1 곳: listings[0].base_price_cr_per_kg: 10→10.5 | 거부 | 거부 | invalid type: floating point `10.5`, expected i64 | 일치 |
| MARKET | `price-zero.json` | `example-two-listings.json` 대비 1 곳: listings[0].base_price_cr_per_kg: 10→0 | 거부 | 거부 | base_price_cr_per_kg 는 1 ..= 1000000 이어야 한다 (받음: 0) | 일치 |
| MARKET | `state-field-injected.json` | `example-two-listings.json` 대비 1 곳: listings[0].current_saturation_kg: "∅"→0 | 거부 | 거부 | unknown field `current_saturation_kg`, expected one of `mineral_id`, `base_price_cr_per_kg`, `band_kg`, `recov | 일치 |
| DEPOSIT_RESERVE | `layout-field-injected.json` | `example-two-deposits.json` 대비 4 곳: deposits[0].position_m#len: "∅"→3; deposits[0].position_m[0]: "∅"→-3200.0; deposits[0].position_m[1]: "∅"→150.0 … | 거부 | 거부 | unknown field `position_m`, expected one of `deposit_id`, `mineral_id`, `initial_reserve_kg` | 일치 |
| DEPOSIT_RESERVE | `reserve-fractional.json` | `example-two-deposits.json` 대비 1 곳: deposits[0].initial_reserve_kg: 3200→3200.5 | 거부 | 거부 | invalid type: floating point `3200.5`, expected i32 | 일치 |
| DEPOSIT_RESERVE | `reserve-zero.json` | `example-two-deposits.json` 대비 1 곳: deposits[0].initial_reserve_kg: 3200→0 | 거부 | 거부 | MassKg 는 이 필드에서 0일 수 없다 (최소 1) | 일치 |


### 0.6 기대 숫자의 출처 — `data/` 에서 읽지 않는다 (p1-02 §0.6 승계)

- 단위·통합 테스트의 기대 숫자는 **`contracts/fixtures/`** 와 **디자인 §3.6·부록 B 의 대금 표**에서만 온다. 테스트 출력을 기대값으로 옮겨 적지 않는다.
- 디자인 대금 표 (qa3 손계산으로 재검산 — 일치 10/10. 구간 걷기 식 = 스펙 §4.2a):

  | # | 광물 | 포화 | 판매 kg | 대금 | qa3 재검산 |
  |---|------|---:|---:|---:|---|
  | P1 | Ferrosite (10, 2,000) | 0 | 12,000 | **90,000** | 2,000 × (10+9+8+7+6+5) |
  | P2 | Ferrosite | 0 | 14,000 | **98,000** | 2,000 × 49 |
  | P3 | Glacine (14, 1,500) | 0 | 10,000 | **96,500** | 1,500 × (14+12+11+9+8+7) + 1,000 × 5 |
  | P4 | Glacine | 0 | 10,500 | **99,000** | + 1,500 × 5 |
  | P5 | Cobaltine (40, 600) | 0 | 3,200 | **100,000** | 600 × 160 + 200 × 20 |
  | P6 | Cobaltine | 0 | 2,400 | **81,600** | 600 × 136 |
  | P7 | Starfall Glass (250, 100) | 0 | 500 | **100,000** | 100 × 1,000 |
  | P8 | Starfall Glass | 0 | 700 | **122,500** | 100 × 1,225 |
  | P9 | Ferrosite | 0 | 2,000 | **20,000** | 2,000 × 10 |
  | P10 | Ferrosite | 2,000 (P9 뒤) | 2,000 | **18,000** | 2,000 × 9 |

  구간별 가격(디자인 §3.6): Ferrosite 10 9 8 7 6 5 4 **3** · Glacine 14 12 11 9 8 7 5 **4** · Cobaltine 40 36 32 28 24 20 16 **12** · Glass 250 225 200 175 150 125 100 **75**. 포화 상한 = 7 × 구간(14,000 / 10,500 / 4,200 / 700). 회복 T-8: 포화 14,000 → 배수 tick 1 회 → 13,000(시세 4) → 1 회 더 → 12,000.
- **입력과 기대 숫자를 구분한다.** fixture 가 없는 성질(분할 무관의 무작위 분할, 경계 kg)은 **합성 입력**을 쓰고 리포트에 "테스트 내장 리터럴" 또는 "시드 N" 으로 출처를 적는다. 기대 **숫자**는 위 표나 식에서 독립적으로 온다.
- `data/` 는 **기동 경로(SC-04·05)와 실서버·사람 세션 관측**에만 쓴다. 그때 그 시점의 `data/` 값(파일 sha256)을 증거에 함께 적는다.

### 0.7 구현 전 기준선 — **판정에 쓰지 않는다** (qa3 실측 2026-10-05, HEAD `b3bc8f2`, 워킹트리 변경 36 항목)

빨간불이 정상이다(계약이 코드보다 앞서 있다 — 01_architect_tasks "계약이 코드보다 앞서 있다").

| 항목 | 기준선 | 확인 |
|------|--------|------|
| 계약 커버리지 `--strict` | **types 30 / errors 17 / warnings 0**, `RESULT: FAIL` | qa3 실행 — architect 실측과 일치 |
| `cargo test -p starfall-contracts --locked` | `contract_tests`: **4 passed / 8 failed**(`registry_responses`·`registry_server_types_mapped`·`registry_consistency`·`schema_ids_match_paths`·`fixtures_roundtrip`·`schemas_valid_offline`·`invalid_fixtures_layered`·`invalid_rejected_by_schema`), lib 36 passed, EXIT 101 | qa3 실행 |
| 서버 기동 | **거부가 정상**(`DEPOSIT_FIELD` v2 — S2 전) | architect 기록, qa 미실행 |
| 이동 golden sha256 (SC-41) | `replay/initial.json` `824a6489…097f` · `inputs.jsonl` `c924e59f…eb08` · `snapshots.jsonl` `3e00e527…fe` | qa3 `sha256sum` |
| 채굴 golden sha256 (SC-41) | `replay_mining/events.jsonl` `798fce4f…5d9b` · `initial.json` `d0bf2c49…3950` · `inputs.jsonl` `0a80a880…f428` · `snapshots.jsonl` `d6f6c0a2…177e` | qa3 `sha256sum` |
| 역사 golden sha256 (SC-41) | `history/tests/data/golden/mineral-discovery@1.hash.txt` `9f5738dd…f61e` · `…output.txt` `559348e5…5180` · `first-extraction-evidence-id.txt` `730e290e…a0c4` | qa3 `sha256sum` |
| `data/**/*.json` | **12 파일**(p1-02 10 + `market/cradle-exchange.json` + `world/deposit-reserves/cradle.json`) | qa3 `find` |
| 클라이언트 사본 `client/Assets/_Project/Data` | **10 파일** — 그중 `history/rules/mineral-discovery.json` 은 이번에 **빠져야 할** 파일이고, `market/cradle-exchange.json` 은 **없어서 들어와야 할** 파일. 사본의 `world/deposits/cradle.json` 은 아직 v1(서버 전용 필드 2 × 광맥 8 — p1-02 만기 분모 2) | qa3 `find` |

전체 sha256(64자)은 `evidence/baseline_20261005/golden_sha256.txt` 에 qa3 가 Q2 에서 남긴다(SC-41 의 비교 입력).

### 0.8 측정 환경과 빌드 금지 (p1-02 §0.8 승계)

실서버 판정 실행 중에는 `client/`·`server/` 소스를 저장하지 않는다(라운드 동결 = `_workspace/p1-03-trading/FREEZE`, R6). Unity Editor 를 연 채로 서버 범위 단계를 돌리는 것은 기록만 한다(`run_round.py` `STAGE_WATCH` 완화).

### 0.9 이 슬라이스가 **증명하지 못하는 것** (리포트 요약에 반드시 싣는다)

1. **사람이 시세 흔적을 보고 광물을 바꾸는가**(재미 가설). AC-19 는 B 가 흔적을 **받았음**만 잰다. 봇 정책 기준선(M-3)과 사람 세션 답(T-3 질문)은 판정이 아니다.
2. 30 명 세션의 경제 균형(광물 분산·바닥 빈도) — 봇 정책은 사람 정책이 아니다(M-4).
3. 지갑 상한(2^53−1)·인벤토리 상한의 **실전** 도달 — 경계값 주입으로만 실행된다(SC-07 사유 7).
4. 크레딧 차감 경로(지갑 하한 ②) — 이번 범위에 없다(스펙 §3, 만기 표).

### 0.10 판정 SQL·도구는 **계약의 필드 이름**으로 쓴다

`MINERAL_SOLD` payload 의 필드 이름은 **`contracts/events/domain/MINERAL_SOLD.schema.json`** 이 정본이다(스펙 §5.1 표의 이름과 대조: `quantity_before_kg`·`quantity_after_kg`·`wallet_before_cr`·`wallet_after_cr`·`saturation_before_kg`·`saturation_after_kg`·`price_before_cr_per_kg`·`price_after_cr_per_kg`·`base_price_cr_per_kg`·`floor_price_cr_per_kg`·`pricing_version`·`proceeds_cr`). 디자인 §9.3 의 작업 이름(`inventory_before_kg` 등)을 SQL 에 쓰면 판정 입력이 NULL 이 된다(p1-02 §0.10 의 전제 오류형). Q2 의 `trade_ledger.py` 는 스키마 파일에서 필드 이름을 읽어 쓰는지 selftest 로 보인다.

### 0.11 대조 키는 `actor_id` 다 — `Pilot-xxxx` 가 아니다 (p1-02 §0.11 승계)

`last_sale.actor_id`·`MINERAL_SOLD.actor_id` 대조는 전체 UUID 로 한다. `Pilot-xxxx` 는 표시 전용이다(I-66, I-88). 개발 ID 에서 끝 4 글자 충돌이 실제로 있다(p1-02 SC-110).

---

## 1. 검증 항목

`불가` 칸: E1 cargo · E2 Docker/PG · E3 Unity · E4 .NET · E5 서버 환경 기동 실패 · E6 봇 빌드 · E7 부하 자원 · E9 선행 미완 · **W 새 월드(§0.2)** · **H 사람 필요**. 방법 칸의 `` `이름` [DB] `` 은 DB 통합 테스트(§3.4).

### A. 서버 게이트와 순수성 (AC-1) — server

| ID | 검증 항목 | 검증 방법 | ⊘ 자명 통과 → 배제 단언 | 담당 | 근거 | 불가 |
|----|----------|----------|------------------------|------|------|------|
| SC-01 | `cargo fmt --all --check` · `cargo clippy --workspace --all-targets -- -D warnings` · `cargo test --workspace --locked` · `cargo test -p starfall-sim --release --locked` 넷 전부 종료 코드 0, 실패 0 | 네 명령 그대로(`run_round.py` gates 단계). 크레이트별 `N passed` 를 증거에 | **새 테스트 파일이 워크스페이스에 없어 0 건 통과** → 크레이트별 통과 수를 §0.7 기준선과 나란히 찍고, `starfall-sim` 의 `trading_judgement`·`pricing_golden`·`trading_replay` 테스트 바이너리가 출력에 있고 각 `passed ≥ 1` | server | AC-1 | E1 |
| SC-02 | `starfall-sim`·`starfall-history` 의 `[dependencies]` 에 `axum`·`sqlx`·`redis`·`rand`·`chrono`·`tokio` 없음 | `grep -nE "axum\|sqlx\|redis\|rand\|chrono\|tokio" server/crates/sim/Cargo.toml server/crates/history/Cargo.toml` → 0 매칭 | **파일이 없어 0 매칭** → 두 경로를 `ls` 로 먼저 찍고 검사한 크레이트 이름 2 개 출력. 양성 대조: 같은 grep 을 `server/crates/persistence/Cargo.toml` 에 → `sqlx` ≥ 1 | server | AC-1 | E1 |
| SC-03 | **역사 코드 diff 0** — `server/crates/history/**` 와 `server/crates/persistence/src/history.rs` 가 HEAD `b3bc8f2` 와 같다(이번 슬라이스는 역사 코드를 바꾸지 않는다, history3 판단) | 도구 없음(사람 관찰) — git 출력을 증거에 붙인다: `git diff --stat b3bc8f2 -- server/crates/history server/crates/persistence/src/history.rs` → 빈 출력 **그리고** `git status --porcelain -- server/crates/history` → 빈 출력(untracked 신규 파일도 잡는다) | **경로 오타로 빈 출력** → `git ls-files server/crates/history \| wc -l` ≥ 1 과 `persistence/src/history.rs` 존재를 같은 실행에서 찍는다. 양성 대조: 같은 명령을 `contracts/` 에 → 비어 있지 않다(이 슬라이스가 바꾼 경로) | server | AC-1, 스펙 §3 | E1 |

### B. 데이터 표와 기동 거부 (AC-2) — server · qa

| ID | 검증 항목 | 검증 방법 | ⊘ 자명 통과 → 배제 단언 | 담당 | 근거 | 불가 |
|----|----------|----------|------------------------|------|------|------|
| SC-04 | 정상 `data/` 로 기동 → `/debug/stats`(또는 기동 로그)에 **시장 1(id `cradle-exchange`·`pricing_version`·상장 4) · 광맥 8 · 예비 8 · 광물 4**, 각각 파일과 일치 | 새 월드 실서버 기동 — `python tests/e2e/server_boot.py p1-03-boot --world <id> --log <기동 로그> --evidence <json>`(Q2 가 하위 명령 추가). 기대값은 **같은 실행에서** 파일로 계산: `jq '.listings\|length' data/market/cradle-exchange.json`·`jq -r .pricing_version …`·`jq '.deposits\|length' data/world/deposits/cradle.json`·`jq '.deposits\|length' data/world/deposit-reserves/cradle.json`\|length' data/world/deposit-reserves/cradle.json`·`ls data/minerals/*.json \| wc -l` | **stats 필드가 없어 파서 기본값 0 과 0 비교** → 다섯 필드가 stats JSON(또는 로그 줄)에 **존재**함을 먼저 단언, 파일 기대값이 0 이 아님을 찍는다. 필드 이름(server3 Q-7 답): `markets_loaded`(1)·`sole_market_id`·`sole_market_pricing_version`·`sole_market_listings_loaded`(4)·`deposits_loaded`(8)·`deposit_reserves_loaded`(8)·`minerals_loaded`(4) | server · qa | AC-2(a) | E2 E5 W |
| SC-05 | 스펙 §4.8 의 조건을 **하나씩** 깨뜨린 `STARFALL_DATA_DIR` 에서 **전부 기동 실패**, 로그에 **파일과 필드**. 경우(16): **시장** m01 `star_system_id` ≠ 로드된 성계 · m02 `pricing_version` 이 `id@` 로 시작 안 함 · m03 상장 없는 광물(glacine 상장 삭제) · m04 한 광물에 상장 둘 · m05 광물 표에 없는 `mineral_id` 상장 · m06 소프트 경계 초과(위치 (0, 9800, 0)) · m07 도킹 구역이 광맥 채굴 구역과 겹침(위치를 Inner Belt 1 옆으로) · m08 `sell_cooldown_s` 소수(1.5) · m09 `recovery_interval_s` 소수(60.5) · m10 바닥값 0(`base 1`, floor 30 → 1×30 div 100 = 0) · m11 포화 상한 > 2^31−1(`band_kg` 400,000,000 → 7 × = 2.8 × 10^9) · m12 시장 id 중복(같은 id 두 파일) · **광맥 분할** d01 공개부에만 있는 광맥 · d02 예비에만 있는 광맥 · d03 `DEPOSIT_FIELD` `schema_version` 1 · d04 예비 파일 없음(짝 없음) — **16 경우**. *m08·m09 는 소수 초가 **스키마층에서 기동 실패**로 막히는 경로를 잰다. "× tick_hz 비정수" 검산과 대금 상한 부등식은 스키마가 보장하는 항등식이라 스펙 §4.8 에서 "계약이 보장하는 것" 으로 옮겨졌다(architect3 Q-1 판정 2026-10-06) — 그 검산의 테스트(m13)는 두지 않는다* | server: `cargo test -p starfall-game-server data::tests::market_reject_` (`market_reject_m01_…` ~ `market_reject_m12_…` · `deposit_split_reject_d01_…` ~ `d04_…` 는 **같은 필터 밖** → 둘째 명령 `cargo test -p starfall-game-server data::tests::deposit_split_reject_`) + 양성 대조 `cargo test -p starfall-game-server data::tests::accepts_unmodified_copy`. qa: `python tests/e2e/server_boot.py p1-03-reject --workdir <tmp> --evidence <json>` 로 **실바이너리 4 경우**(m03·m10·d02·d03) — 로그 줄을 증거에 | **기동 실패 원인이 주입한 결함이 아님**(DB 미기동·경로 오류) → 각 경우의 로그에서 **주입한 필드 이름**(스키마층 m08·m09·d03 포함)을 단언. **양성 대조**: 같은 하네스로 무변경 사본 기동 성공. 경우 수 16 출력. 도달 가능성(R4): 16 경우 각각 스키마가 허용하는 값으로 만들 수 있음을 위 괄호 값으로 계산했다(m10: `base ≥ 1` 허용, m11: `MassKg ≤ 2^31−1` 허용, m06: 위치 ±20,000 허용) | server · qa | AC-2(b) | E1 E2 |

### C. 판매 판정 — 수동 step 통합 테스트 (AC-3, 디자인 T-1·T-5) — server

소켓·DB 없이 sim 을 tick 단위로 민다. 파일은 `server/crates/sim/tests/trading_judgement.rs`(S3). **거부 상태 비교는 "수락 1 건이 이미 있는 상태"에서**(빈 상태의 "안 바뀜" 은 공짜다).

| ID | 검증 항목 | 검증 방법 | ⊘ 자명 통과 → 배제 단언 | 담당 | 근거 | 불가 |
|----|----------|----------|------------------------|------|------|------|
| SC-06 | 모든 조건 충족 → **수락**: Ferrosite 2,000 kg @포화 0 → 인벤토리 −2,000, 지갑 +**20,000**(P9), 포화 2,000, 시세 10 → 9, `MINERAL_SOLD` **1 건**의 **모든 필드**(fixture `MINERAL_SOLD/*.json` 의 키 집합과 같음), `causation_id = command_id`, `pricing_version = cradle-exchange@1`, 마지막 거래 = 그 actor·2,000·그 tick, 응답 `COMMAND_RESULT`→`SALE_RECEIPT`→`INVENTORY_STATE`→`WALLET_STATE` 순 | `cargo test -p starfall-sim sale_accepted_` | **모든 경우가 거부로 끝나 수락 경로 미실행** → 수락 수 ≥ 1 과 `MINERAL_SOLD` 수 = 1 을 출력. 필드 비교는 키 **집합 같음**(부분집합 아님) | server | AC-3(a) | E1 |
| SC-07 | 스펙 §4.2 사유 **2~7 각각**(2 는 없는 시장 **과** 상장 없는 광물 둘 다 → 7 경우)에서 거부, **인벤토리·지갑·포화·마지막 거래·판매 쿨다운·처리 장부 불변**, `commands_rejected_total{사유}` +1. 사유 7 은 지갑 = `2^53−1 − 대금 + 1` 주입. **디버그·릴리스 같은 결과**(checked 연산) | ① sim: `cargo test -p starfall-sim sale_rejection_reason_` (7 경우 — 이름에 사유 번호) + `cargo test -p starfall-sim --release sale_rejection_reason_` ② gateway 카운터: `cargo test -p starfall-gateway sell_mineral_rejection_counter_` (사유 6 `INSUFFICIENT_QUANTITY`·7 `CAPACITY_EXCEEDED` — 소켓 → sim → `record_rejection` → `/debug/stats`) ③ 사유 2~5 의 카운터 +1 은 실서버 SC-107~111 의 `stats_delta.py` 델타 = 봇 관찰 수(§11 Q-2) | **비교 대상이 비어 "안 바뀜" 이 공짜** → 거부 전 상태에 수락 판매 1 건이 있어 지갑 > 0·포화 > 0·마지막 거래 존재·장부 ≥ 1 임을 먼저 단언(사유 3 외는 쿨다운이 끝난 시점에서). 사유별 관찰 수 7 출력. 사유 7 은 주입값 = `2^53−1 − 대금 + 1` 이고 덧셈이 실제로 넘친다는 것을 입력 단계에서 단언(도달 가능성: 대금 ≥ 1 이므로 주입값 ≤ 2^53−1, 지갑 행 CHECK 범위 안) | server | AC-3(b), ADR-0016 §5·§6 | E1 |
| SC-08 | 두 사유 동시 성립(쿨다운 중 **+** 도킹 밖) → **`COOLDOWN_ACTIVE`**(표 순서의 앞) | `cargo test -p starfall-sim sale_judgement_order_cooldown_before_dock` | **도킹 밖이 아니라 쿨다운 하나만 성립** → 같은 tick 의 함선 거리² > 320² 와 쿨다운 잔여 > 0 을 둘 다 단언 | server | AC-3(c) | E1 |
| SC-09 | 거부 직후 tick 에 조건을 만족시키면 **판매 쿨다운에 막히지 않고 수락** | `cargo test -p starfall-sim sale_rejection_does_not_start_cooldown` | **직전 수락 판매의 쿨다운이 이미 끝나 있어 자명** → 거부 tick 과 다음 tick 이 직전 수락 + 20 tick **안**에 있지 않음을 단언, 거부 tick+1 수락 | server | AC-3(d), I-79 | E1 |
| SC-10 | **경계 양쪽**: 보유량 정확히 → 수락, `inventory_items` 대응 상태 0(지워지지 않음), `INVENTORY_STATE` 에 그 항목 없음 / 보유량 + 1 → `INSUFFICIENT_QUANTITY` | `cargo test -p starfall-sim sale_holding_boundary_` (정확히·+1 두 테스트) | **보유 0 에서 +1 → 공짜 거절** → 보유량 > 0 에서 요청 = 보유 + 1 임을 단언. 정확히 쪽은 배치의 인벤토리 쓰기가 **0 갱신**(삭제 아님)임을 단언 | server | AC-3(e), I-93 | E1 |
| SC-11 | 같은 tick 두 판매(포화 0, Ferrosite 2,000 씩)가 **제출 순서대로** 20,000 / 18,000(P9·P10), 제출 순서를 바꾸면 결과도 바뀐다 | `cargo test -p starfall-sim sale_same_tick_submission_order_` | **두 판매가 다른 tick** → 두 `MINERAL_SOLD.tick` 이 같음을 먼저 단언. 순서 교환 실행에서 actor 별 대금이 서로 바뀜 | server | AC-3(f), I-71 | E1 |
| SC-12 | 같은 tick 의 채굴과 판매가 **제출 순번으로 섞여** 처리된다(`sequence` 가 제출 순) | `cargo test -p starfall-sim sale_and_mine_same_tick_interleave_by_submission` | **채굴·판매가 다른 tick** → 같은 tick, 그리고 제출 순 `[판매, 채굴, 판매]` 처럼 종류가 교차하는 입력임을 단언 | server | AC-3(g), I-71 | E1 |
| SC-13 | 판매 쿨다운: t 수락 → t+19 `COOLDOWN_ACTIVE` → t+20 수락. **채굴 쿨다운과 독립**(판매 직후 채굴 수락, 채굴 직후 판매 수락) | `cargo test -p starfall-sim sale_cooldown_` (`sale_cooldown_t19_rejects_t20_accepts`·`sale_cooldown_is_independent_of_mining_cooldown`) | **t+20 이 다른 이유로 수락**(쿨다운 미구현) → t+19 거부를 같은 테스트에서 먼저 단언. 독립성은 두 방향 각각의 수락 1 | server | AC-3(h) | E1 |

### D. 가격 규칙 — 순수 함수 (AC-4, I-92, 디자인 T-7·T-8) — server

`server/crates/sim/src/trading/price.rs` 단위 테스트 + `server/crates/sim/tests/pricing_golden.rs`(S3b).

| ID | 검증 항목 | 검증 방법 | ⊘ 자명 통과 → 배제 단언 | 담당 | 근거 | 불가 |
|----|----------|----------|------------------------|------|------|------|
| SC-14 | §0.6 대금 표 **P1~P10** 을 **디자인 문서에서 옮긴 상수**로 단언 | `cargo test -p starfall-sim trading::price::tests::design_table_` | **대금이 전부 0** → 이 항목 자체가 양성 대조(기대값 > 0). 상수 10 개를 테스트가 출력, 상수의 출처 주석 = "디자인 §3.6·부록 B"(데이터 파일에서 읽지 않는다 — 테스트가 `data/` 를 열지 않음을 리뷰에서 확인) | server | AC-4(a) | E1 |
| SC-15 | **분할 무관**: 같은 포화에서 무작위 분할 N 개의 대금 합 = 한 번 판매. 구간 경계를 걸치는 분할(1,500 + 1,000 등) 포함 | `cargo test -p starfall-sim trading::price::tests::split_invariance_` | **분할이 전부 구간 안이라 자명** → 경계를 걸친 분할 수 ≥ 1 과 시드·N 을 출력 | server | AC-4(b), T-7 | E1 |
| SC-16 | **단조**: 같은 양 → 포화 ↑ 대금 ≤ / 같은 포화 → 양 ↑ 대금 ≥ | `cargo test -p starfall-sim trading::price::tests::monotonic_` | **비교 쌍이 같은 구간 안에만** → 구간이 다른 쌍 ≥ 1 출력 | server | AC-4(c) | E1 |
| SC-17 | **유계·바닥**: 포화 상한을 넘는 양 → 넘는 양 전부 바닥값, 포화 = 상한(14,000 등), `0 ≤ s ≤ 상한` | `cargo test -p starfall-sim trading::price::tests::floor_and_cap_` | **바닥에 안 닿는 양만 시험** → 입력 양이 상한 − 시작 포화보다 큼을 단언, 바닥 구간 take > 0 | server | AC-4(d) | E1 |
| SC-18 | **회복: 식(I-87) = tick 별 시뮬레이션**, 바닥 14,000 → 1,200 tick 뒤 13,000 → 시세 4(T-8) | `cargo test -p starfall-sim trading::price::tests::recovery_formula_equals_per_tick_simulation` | **구간이 배수 경계를 안 걸침** → 비교 tick 범위가 ≥ 2 개의 1,200 배수를 포함함을 단언, 비교한 tick 수 출력 | server | AC-4(e), T-8 | E1 |
| SC-19 | **대금 ≥ 판매량 ≥ 1** — 네 상장 × 경계 포화(0, 상한 − 1, 상한) × 양(1, 구간, 2^31−1) | `cargo test -p starfall-sim trading::price::tests::proceeds_at_least_quantity_` | **바닥값 1 미만 상장이 없어 자명?** — 이 성질은 기동 검증(SC-05 m10)과 짝이다. 검사한 조합 수(36) 출력 | server | AC-4(f) | E1 |
| SC-20 | **반복 수**: `quantity_kg = 2^31−1` 판매의 구간 반복 ≤ `floor_band + 1`(= 8), **카운터 출력** — kg 루프가 아님 | `cargo test -p starfall-sim trading::price::tests::band_walk_iterations_` | **카운터가 증가하지 않아 0 ≤ 8** → 같은 테스트에서 포화 0·2,000 kg 판매의 반복 = 1 과 포화 0·14,001 kg 의 반복 = 8 을 단언(양성) | server | AC-4(g), ADR-0016 §4 | E1 |
| SC-21 | **golden**: `pricing_version` 이 가격 절 정규화 해시 + 고정 입력 대금 표에 묶임. **가격 값 하나만 바꾼 시장 파일이 실제로 실패**(음성 대조) | `cargo test -p starfall-sim pricing_golden_` (`pricing_golden_binds_version_to_hash_and_table`·`pricing_golden_rejects_changed_price_without_version_bump`) | **golden 파일이 BLESS 로 매번 다시 생김** → 음성 대조 테스트가 같은 실행에서 실패를 관측(바꾼 필드 이름 출력). golden 파일 sha256 을 작업 전후로 | server | AC-4(h), I-90 | E1 |

### E. 복사 경로 — 같은 `command_id` 는 한 번 (AC-5) — server · serverdb

**모든 경로에서 첫 전송이 `ACCEPTED` 였고 지갑이 늘었음을 먼저 단언**(⊘ 공통). 각 경로의 최종 지갑 = **대금 × 1**, 인벤토리 감소 = **× 1**.

| ID | 검증 항목 | 검증 방법 | ⊘ 자명 통과 → 배제 단언 | 담당 | 근거 | 불가 |
|----|----------|----------|------------------------|------|------|------|
| SC-22 | (a) 한 세션 2 회 → 둘째 `DUPLICATE_COMMAND_ID` | `cargo test -p starfall-sim sale_dup_same_session` | 첫 전송 수락·지갑 증가 선단언 | server | AC-5(a) | E1 |
| SC-23 | (b) 잔류 창 안 재접속 뒤 같은 명령 → 중복 | `cargo test -p starfall-sim sale_dup_after_reconnect_in_linger` | 같은 + 재접속 세션 id 가 다름을 단언 | server | AC-5(b) | E1 |
| SC-24 | (c) **정상 종료 → 재기동 → 같은 actor → 같은 명령** → 중복, 지갑·인벤토리·`processed_commands`·`market_states` 행 불변 | `cargo test -p starfall-persistence sale_dup_across_restart_is_rejected` [DB] (적재 → sim 기억 주입 경로) | 같은 + 재기동 전 네 표의 행 수·값을 찍고 후와 같음. 기동 적재가 실제로 그 키를 읽었음(적재된 장부 수 ≥ 1) | serverdb · server | AC-5(c), ADR-0013 §7 | E1 E2 |
| SC-25 | (d) 같은 actor 의 두 연결이 같은 tick → 수락 1·중복 1 | `cargo test -p starfall-sim sale_dup_two_connections_same_tick` | **두 명령이 다른 tick** → 같은 tick 단언 | server | AC-5(d) | E1 |
| SC-26 | (d2) 다른 actor 가 같은 키 → 수락 1·중복 1·**서버 생존**(정지 없음) | `cargo test -p starfall-sim sale_dup_cross_actor` + `cargo test -p starfall-persistence sale_dup_cross_actor_no_halt` [DB] | **두 프레임의 actor 가 같음** → actor 다름·키 같음 단언. DB 쪽은 `persist_fatal_total = 0` 과 다음 배치 커밋(`last_tick` 전진) | server · serverdb | AC-5(d2), ADR-0013 §5a | E1 E2 |
| SC-27 | (e) **명령 종류 교차**: `MINE_RESOURCE` 로 수락된 키를 `SELL_MINERAL` 로, 판매로 수락된 키를 채굴로 → 둘 다 `DUPLICATE_COMMAND_ID`(I-82) | `cargo test -p starfall-sim sale_dup_cross_kind_` (두 방향 두 테스트) | **첫 명령이 거부돼 장부에 없음** → 첫 채굴 수락(인벤토리 증가)·첫 판매 수락(지갑 증가) 각각 선단언 | server | AC-5(e), I-82 | E1 |
| SC-28 | (f) sim 기억을 우회해 중복 판매가 DB 에 도달 → **PK 위반 → 정지**, 지갑 불변 | `cargo test -p starfall-persistence sale_dup_reaching_db_halts` [DB] | **다른 이유로 정지** → 정지 사유가 `processed_commands_pkey` 위반이고 같은 배치의 `MINERAL_SOLD` 가 DB 에 없음, 지갑 행 = 첫 판매 뒤 값 | serverdb | AC-5(f) | E1 E2 |
| SC-29 | (g) 세션 안 **거절된** 명령 재전송 → `DUPLICATE_COMMAND_ID`(세션 기억), 장부에 없음 | `cargo test -p starfall-sim sale_dup_rejected_resend_in_session` | **첫 전송이 수락** → 첫 전송의 거절 사유를 단언, 장부 행 수 불변 | server | AC-5(g), I-52 | E1 |

### F. 원장·보존·포화 항등식과 비교 후 쓰기 (AC-6, 디자인 T-9) — qa · serverdb

**SC-30~33 의 실행 모양 요건(AC-6(a))** — 같은 월드에서 `trade_ledger.py --world <id> shape` 가 출력: **판매 actor ≥ 2 · 판매 ≥ 10 · 판매된 광물 ≥ 2 · 구간을 걸친 판매 ≥ 1 · 두 판매 사이에 회복이 끼어든 쌍 ≥ 1**. 하나라도 미달이면 SC-30~33 전부 FAIL(무효 아님 — 스펙이 FAIL 로 정했다).
- **도달 가능성(R4)**: 봇 케이스 `trade-cycle`(봇 A·B Ferrosite, 봇 C Glacine). 한 사이클 = 2,000 kg 채굴(200 kg × 10, 쿨다운 3 s → 30 s) + 왕복 ≈ 67 s(디자인 §3.6) ≈ 100 s. A·B 각 5 사이클 ≈ 8.5 분 → 판매 10, 필요 Ferrosite 20 t ≤ 초기 33 t(IB1 12 · IB2 9 · IB3 12). C 의 Glacine 판매 ≥ 1 → 광물 2. 같은 광물 연속 판매 간격 ≥ 60 s(= 1,200 tick) → 회복이 끼어든 쌍은 둘째 Ferrosite 판매부터 생긴다. 회복 뒤 포화(예: 2,000 − 1,000 = 1,000)에서 2,000 kg 판매는 구간 0·1 을 걸친다. 실행 길이 상한 15 분.
- "구간을 걸친 판매" = `band(saturation_before) ≠ band(saturation_after − 1)`. "회복이 끼어든 쌍" = 같은 (시장, 광물) 이웃 판매에서 `⌊t2/1200⌋ > ⌊t1/1200⌋` **그리고** `saturation_before(t2) < saturation_after(t1)`.

| ID | 검증 항목 | 검증 방법 | ⊘ 자명 통과 → 배제 단언 | 담당 | 근거 | 불가 |
|----|----------|----------|------------------------|------|------|------|
| SC-30 | **I-80 원장**: 각 `(actor, mineral)` 인벤토리 행 = Σ채굴 − Σ판매 = 최대 이벤트의 `quantity_after_kg`; 각 actor 지갑 행(없으면 0) = Σ`proceeds_cr` = 최대 `MINERAL_SOLD.wallet_after_cr`; 월드 화폐 총량 Σ지갑 = Σ대금 | `python tests/e2e/trade_ledger.py --world <trade-cycle 월드> ledger` (Q2 신규, 라벨 `p1-03 SC-30`) | **판매 0 건이면 0 = 0** → 판매 0 이면 FAIL 인쇄. 검사한 키 수·이벤트 수·판매 수 출력, 판매 0 인 광물도 행으로. 실행 모양 요건 출력 | qa | AC-6(a), I-80 | E2 E5 E6 W |
| SC-31 | **I-81 연쇄**: `(actor, mineral)` 의 채굴·판매 합친 `(tick, sequence)` 순 이웃 `앞.quantity_after = 뒤.quantity_before`, 첫 before = 0; 판매끼리 `wallet` 연쇄, 첫 `wallet_before = 0`; 각 판매 안 `after = before − quantity`·`wallet_after = before + proceeds` | `trade_ledger.py --world <id> chain` | **이웃 쌍 0** → 검사한 이웃 쌍 수(채굴→판매 전이 ≥ 1 포함) 출력, 0 이면 FAIL | qa | AC-6(a), I-81 | E2 E5 E6 W |
| SC-32 | **I-84 질량 보존**: 각 광물 Σ인벤토리 + Σ광맥 잔량 + Σ`MINERAL_SOLD.quantity_kg` = Σ초기 매장량 + Σ실제 회복량 | `trade_ledger.py --world <id> conservation` — 초기 매장량은 `data/world/deposit-reserves/cradle.json`(값·sha256 을 증거에), 회복량은 p1-02 `mining_ledger.py` 의 재계산 함수 재사용 | **판매 항이 0 이라 p1-02 식과 같음** → 광물별 판매 항 > 0 인 광물 ≥ 2. **음성 대조**: 같은 월드에서 `trade_ledger.py --world <id> conservation --without-sales-term`(판매 항 없는 p1-02 식)이 FAIL 함을 찍는다(스펙 §10-17 — 판매 항을 모르는 식은 실패해야 한다. p1-02 `mining_ledger.py` 는 광맥 표 v2 에서 `mineral_id` 를 못 읽어 적재부터 실패하므로 음성 대조로 쓸 수 없다) | qa | AC-6(a), I-84 | E2 E5 E6 W |
| SC-33 | **I-85 포화 재계산**: `(market, mineral)` 의 판매를 `(tick, sequence)` 순으로 되감아 첫 `saturation_before = 0`, 이웃 `뒤.before = recover(앞.after, 앞.tick → 뒤.tick)`, 각 판매의 `proceeds_cr`·`saturation_after_kg`·`price_*` = §4.2a 재계산(파이썬 독립 구현), 마지막 판매의 `saturation_after`·tick = `market_states` 행 | `trade_ledger.py --world <id> saturation` | **재계산기가 서버 값을 읽어 자기 일치** → 재계산 입력은 이벤트의 `quantity_kg`·tick 과 시장 표 상수뿐임을 selftest 로 고정(서버 `proceeds_cr` 를 입력에 넣으면 selftest 실패). 재계산 기대가 §0.6 P9·P10 과 일치함을 selftest 로. 회복 끼어든 쌍 ≥ 1 출력 | qa | AC-6(a), I-85 | E2 E5 E6 W |
| SC-34 | (b) 서버가 떠 있는 동안 한 actor 의 **지갑 행**을 SQL 로 바꾼 뒤 그 actor 가 판매 → **정지**: 0 아닌 종료 코드, 로그에 비교 후 쓰기 실패와 키, `persist_fatal_total ≥ 1`, 그 tick 의 `MINERAL_SOLD` 가 DB 에 없다 | `cargo test -p starfall-persistence wallet_cas_mismatch_halts` [DB] + 실서버 `python tests/e2e/restart_cases.py --out <dir> sell-cas` (봇 `sell-cas-halt`, 변조 전 판매 1 → SQL 변조 → 판매) | **다른 이유로 죽음** → 변조 전 같은 actor 의 판매가 수락·커밋됐음(DB 행)과 종료 로그의 사유 = 지갑 행 비교 실패를 단언. **양성 짝**: 같은 절차에서 변조만 뺀 control 실행은 생존 | serverdb · qa | AC-6(b), I-93 | E1 E2 E5 E6 W |
| SC-35 | (b) **`market_states` 행** 변조로 같은 것 1 회 | `cargo test -p starfall-persistence market_cas_mismatch_halts` [DB] + `restart_cases.py --out <dir> sell-cas-market` | SC-34 와 같음(변조 대상 = 그 판매의 `(market, mineral)` 행, 사유 = 시장 행 비교 실패) | serverdb · qa | AC-6(b), I-93 | E1 E2 E5 E6 W |
| SC-36 | (c) 재기동하면 **변조된 값으로 적재**되고 다음 판매가 그 위에서 수락된다 — DB 가 정본 | `cargo test -p starfall-persistence restart_loads_tampered_wallet_and_sells_on_top` [DB] + `restart_cases.py sell-cas` 의 reload 단계(봇 `sell-cas-reload`) | **변조 값과 원래 값이 같음** → 변조 전·후 값이 다름을 찍고, 재기동 뒤 판매의 `wallet_before_cr` = 변조 값 | serverdb · qa | AC-6(c) | E1 E2 E5 E6 W |
| SC-37 | (d) 상태 표에 DELETE 경로가 없다: 전량 판매 뒤 `inventory_items` 행이 `quantity_kg = 0` 으로 존재 | `cargo test -p starfall-persistence full_sale_keeps_zero_inventory_row` [DB] + `trade_ledger.py --world <id> zero-rows`(실서버 월드에서 0 행 수 출력) | **전량 판매가 없음** → 직전 이벤트 `quantity_after_kg = 0` 인 판매 ≥ 1 과 그 키의 행 수 = 1·값 0 | serverdb · qa | AC-6(d), I-93 | E1 E2 W |

### G. 기록 지연 거부 (AC-7) — server · qa

| ID | 검증 항목 | 검증 방법 | ⊘ 자명 통과 → 배제 단언 | 담당 | 근거 | 불가 |
|----|----------|----------|------------------------|------|------|------|
| SC-38 | 게이트웨이: `recording_lag` 21 에서 `SELL_MINERAL` → `RECORDING_BACKLOG`, 같은 상태 `SET_SHIP_CONTROL` → 수락, `recording_lag` 20 에서 판매 → 이 사유로 거부되지 않음 | `cargo test -p starfall-gateway sell_mineral_recording_backlog_` | **판매가 다른 사유로 거부** → 사유가 정확히 `RECORDING_BACKLOG`. 20 쪽은 사유 ≠ `RECORDING_BACKLOG` | server | AC-7, ADR-0016 §1 | E1 |
| SC-39 | 실서버: DB 정지 중 `recording_lag > 20` 뒤 판매 → `RECORDING_BACKLOG`, 같은 구간 `SET_SHIP_CONTROL` 수락, DB 재기동 뒤 판매 수락, 정지 전·중 수락된 판매 전부 DB 에 있고 SC-30 항등식 성립 | 사람 단계 `db_stop`: `bots probe --case sell-backlog`(qa 가 `docker compose stop postgres` → `start`) + `trade_ledger.py --world <id> accepted --from <봇 수락 SELL command_id 파일>`(수락 ⊆ `processed_commands`·`MINERAL_SOLD` + 원장) | **DB 정지 동안 판매를 안 보냄** → 거부 수 ≥ 1 **과** 정지 전 판매 수락 ≥ 1 을 함께. `recording_lag` 최대값 출력. *(`stop` 은 볼륨을 지우지 않는다)* | qa | AC-7 | E2 E5 E6 W H |

### H. 결정성 (AC-8) — server

| ID | 검증 항목 | 검증 방법 | ⊘ 자명 통과 → 배제 단언 | 담당 | 근거 | 불가 |
|----|----------|----------|------------------------|------|------|------|
| SC-40 | 판매를 포함한 입력열(함선 2, 채굴·판매, 쿨다운·도킹 밖·속도 초과·보유 부족, 구간 걸침, 회복 배수 경계)을 **서로 다른 프로세스에서 2 회** 재생 → 매 tick 이벤트 내용(`event_id`·`recorded_at` 제외)과 `SALE_RECEIPT`·`INVENTORY_STATE`·`WALLET_STATE`·`MARKET_STATE` payload 바이트 동일 | `cargo test -p starfall-sim sc40_two_process_replay_with_trading_is_byte_identical` (자식 프로세스 2 — 자식 `trading_replay_worker` 는 `#[ignore]`, 부모가 부른다) | **입력열이 판매를 안 만들어 비교 0** → 비교한 `MINERAL_SOLD` 수 ≥ 1, 거부 사유별 수(쿨다운·도킹 밖·속도·보유 각 ≥ 1), 회복 배수 tick 통과 수 ≥ 1 을 출력 | server | AC-8(a), I-59 확장 | E1 |
| SC-41 | **이동·채굴·역사 golden 이 재생성 없이 통과**하고 9 파일 sha256 이 §0.7 기준선과 같다 | `cargo test -p starfall-sim --test determinism` · `cargo test -p starfall-sim --test mining_replay` · `cargo test -p starfall-history --test mineral_discovery` + `sha256sum` 9 파일 재측정(`run_round.py` offline `sha_compare`) | **BLESS 로 다시 만듦** → 판정은 테스트 통과가 아니라 **해시 불변**. `git status --porcelain server/crates/sim/tests/data server/crates/history/tests/data` 공백도 | server | AC-8(b), CLAUDE.md | E1 |

### I. 계약 ↔ Rust (AC-9) — server

| ID | 검증 항목 | 검증 방법 | ⊘ 자명 통과 → 배제 단언 | 담당 | 근거 | 불가 |
|----|----------|----------|------------------------|------|------|------|
| SC-42 | `cargo test -p starfall-contracts --locked` 가 **타입 30** 을 덮는다: 유효 **57** 왕복, 반례 **113** 스키마 거부, 스키마 **36** 메타 검증, 레지스트리 30 | `cargo test -p starfall-contracts fixtures_roundtrip` · `cargo test -p starfall-contracts invalid_rejected_by_schema` · `cargo test -p starfall-contracts schemas_valid_offline` · `cargo test -p starfall-contracts registry_consistency` | **하드코딩 기대값만 바꾸고 순회 그대로** → 테스트가 디렉터리 순회로 센 수와 하드코딩 수를 둘 다 찍고 같음 단언. §0.7 기준선 RED 8 건이 전부 초록으로 바뀐 것을 테스트 이름으로 | server | AC-9 | E1 |
| SC-43 | 반례의 serde 결과가 §0.5 표 ② 열(기대 = 스펙 §5.3 예측)과 일치, **신규 typed 반례의 거부 사유가 의도한 위반을 가리킴** | `cargo test -p starfall-contracts invalid_fixtures_layered -- --nocapture` (반례별 결과·사유를 표로 출력) | **표가 비어 비교 0** → §0.5 표가 채워지기 전에는 **대기**. 비교한 반례 수(113, 그중 이번 변경분 40) 출력 | server | AC-9, 스펙 §5.3 | E1 E9 |
| SC-44 | `required` 제거 변이가 실패, 레지스트리 `responses` 가 명령 **4** 종에, `data_files` 가 data **9** 종에 있다(그리고 그 밖 kind 에는 없다) | `cargo test -p starfall-contracts required_removal_mutations` · `cargo test -p starfall-contracts registry_responses` · `cargo test -p starfall-contracts registry_data_files` | **명령·data 종류를 0 개 순회** → 순회한 명령 수 4·data 종류 수 9 출력, `SELL_MINERAL.responses` 의 accepted 순서 `[SALE_RECEIPT, INVENTORY_STATE, WALLET_STATE]` 단언 | server | AC-9 | E1 |

### J. 누출 없음과 상장 (AC-10, 디자인 T-10) — server · qa

| ID | 검증 항목 | 검증 방법 | ⊘ 자명 통과 → 배제 단언 | 담당 | 근거 | 불가 |
|----|----------|----------|------------------------|------|------|------|
| SC-45 | (a) 새 월드 첫 채굴 전 **모든 세션**이 받은 `MARKET_STATE.listings` 가 비어 있다 | `cargo test -p starfall-sim market_listing_empty_before_first_extraction` + 실서버 `bots probe --case leak-scan-market` (a) 단계(원문 프레임 저장·검색, 세션 2) | **`MARKET_STATE` 를 아예 안 받음** → 세션마다 세션 시작 `MARKET_STATE` 수신 ≥ 1 을 먼저 단언. 프레임 원문에서 네 광물 id 문자열 검색 = 0 | server · qa | AC-10(a), I-86 | E1 E2 E5 E6 W |
| SC-46 | (b) **양성 대조**: Ferrosite 첫 채굴 tick 에 `MARKET_STATE` 가 월드 전원에게, `ferrosite` 가 나타나고 다른 미드러난 광물은 없다. **Starfall Glass 로 한 번 더**(T-10) | `cargo test -p starfall-sim market_listing_appears_on_first_extraction_` + `leak-scan-market` (b)(c) 단계 | **이미 드러난 월드** → 새 월드 + 첫 채굴 직전 `listings` 비어 있음(SC-45). 수신 세션 수 = 열린 세션 수(≥ 2), 그 tick = 첫 `MINERAL_MINED` tick | server · qa | AC-10(b), I-86 | E1 E2 E5 E6 W |
| SC-47 | (c) 세션 시작 송신 순서 `SESSION_READY` → `INVENTORY_STATE` → `WALLET_STATE` → `DEPOSIT_FIELD_STATE` → `MARKET_STATE` (→ BACKFILL) | `cargo test -p starfall-sim session_start_sends_five_messages_in_order` + `bots probe --case leak-scan-market` 의 두 세션 첫 5 프레임 타입 | **메시지가 빠져도 순서 비교는 통과** → 첫 5 프레임 타입 **목록이 정확히 같음**(부분 순서 아님). 봇 세션 수 출력 | server · qa | AC-10(c), 스펙 §5.1a | E1 E2 E5 E6 W |

### K. 시장 상태의 송신과 영속 (AC-11) — server · serverdb · qa

| ID | 검증 항목 | 검증 방법 | ⊘ 자명 통과 → 배제 단언 | 담당 | 근거 | 불가 |
|----|----------|----------|------------------------|------|------|------|
| SC-48 | (a) 판매 tick 에 열린 **모든 세션**이 `MARKET_STATE` **1 건** | `cargo test -p starfall-sim market_state_sale_tick_sent_to_every_open_session` | **세션 1 개라 "모두 = 1"** → 세션 ≥ 2(판매자 아닌 세션 ≥ 1), 세션 수 = 수신 수 출력 | server | AC-11(a), 디자인 Q4 | E1 |
| SC-49 | (b) 회복 배수 tick: 실효 포화가 바뀐 상장이 있으면 **1 건**, 전부 그대로면 **0 건** — **두 경우를 같은 실행에서 각각 관찰** | `cargo test -p starfall-sim market_state_recovery_tick_` | **한 경우만 관찰** → 관찰 수(바뀜 tick 수 ≥ 1, 안 바뀜 tick 수 ≥ 1)를 출력. 도달 가능성: 포화 2,000 판매 뒤 배수 tick 3 개 = 2,000 → 1,000(바뀜) → 0(바뀜) → 0(안 바뀜), 3,600 tick 안 | server | AC-11(b), I-87 | E1 |
| SC-50 | (c) 회복 tick 에 `market_states`·`domain_events` 행 수가 변하지 않는다 | `cargo test -p starfall-persistence recovery_tick_writes_no_market_row_or_event` [DB] | **회복 tick 에 실효 포화가 안 바뀜** → 그 tick 에 `MARKET_STATE` 가 나갔다(포화가 실제로 줄었다)는 것을 같은 테스트에서 단언, 전후 행 수 출력 | serverdb | AC-11(c), I-87 | E1 E2 |
| SC-51 | (d) 정상 재기동 → 포화·마지막 거래 보존, **재기동 뒤 첫 배수 tick 에 식대로 회복**(T-8: 14,000 → 13,000 → 재기동 → 12,000) | `cargo test -p starfall-persistence restart_preserves_saturation_and_last_sale` [DB] + 적재 함수 층 `cargo test -p starfall-persistence load_economic_state_matches_wallets_and_market_states_in_the_database` [DB] · `cargo test -p starfall-persistence load_economic_state_on_a_world_with_no_sales_returns_empty_wallets_and_markets` [DB] (serverdb3 기존, `trading_load_state.rs`) + 실서버 `restart_cases.py --out <dir> sell-recover`(봇 `sell-recover-restart`) | **재기동 전에 회복이 끝남** → 재기동 직전 포화 > 0 과 재기동 직후 세션 시작 `MARKET_STATE` 의 포화·`last_sale` = 재기동 전 값, 그다음 배수 tick 의 값 = 식(실제 tick) | serverdb · qa | AC-11(d), I-88, T-8 | E1 E2 E5 E6 W |

### L. 역사 러너 통과 (AC-12, H-30~H-33) — history · qa

H1 은 Phase 4 중간에 투입되는 history 구현자가 `server/crates/persistence/tests/history_runner.rs` 에 쓴다. **코드 변경은 없을 것으로 예상** — 실패하면 그것이 발견이다(architect·server 통지). ⊘ 공통: **`MINERAL_SOLD` 행이 실제로 커밋됐음**을 각 테스트가 먼저 단언(분모 출력).

| ID | 검증 항목 | 검증 방법 | ⊘ 자명 통과 → 배제 단언 | 담당 | 근거 | 불가 |
|----|----------|----------|------------------------|------|------|------|
| SC-52 | H-30: 채굴(A, t=10) → 판매 3(t=11..13, payload = 계약 fixture) → 채굴(B, t=14) 뒤 러너 1 회: 커서 = (14, seq) > 마지막 판매 좌표, `MINERAL_DISCOVERED` 2(B 는 판매 **뒤**의 판정 — 양성 대조), `historical_event_sources` 중 `MINERAL_SOLD` id 0(분모 3), `history_detector_halted`·`history_conflicts_total` 0 | `cargo test -p starfall-persistence h30_runner_passes_sales_and_judges_after` [DB] | 판매 3 행 커밋 선단언. 발견 2 가 아니라 1 이면 B 가 판정되지 않은 것(커서 정지) | history | AC-12(a) | E1 E2 |
| SC-53 | H-31: 판매만 있는 구간(판매 2)에서도 커서가 마지막 판매 좌표로 전진, 역사 0 행 | `cargo test -p starfall-persistence h31_runner_advances_over_sales_only` [DB] | 판매 2 행 선단언, 러너 구동 전 커서 < 판매 좌표 | history | AC-12(b) | E1 E2 |
| SC-54 | H-32: `schema_version = 2` 인 `MINERAL_SOLD`(SQL 직접 INSERT) + 그 뒤 첫 채굴 → 러너가 **멈추지 않고** 발견 1. **짝**: 같은 자리의 `MINERAL_MINED` v2 는 정지 | `cargo test -p starfall-persistence h32_sold_v2_does_not_halt_runner` [DB] + 짝 `cargo test -p starfall-persistence history_fail_stop` [DB] (p1-02 기존) | v2 행이 실제로 `schema_version = 2` 로 들어갔음을 SELECT 로 단언. 짝 테스트가 같은 실행에서 정지를 관측 | history | AC-12(c), ADR-0014 §1 | E1 E2 |
| SC-55 | H-33: `history-replay` 일치, 재생 행 중 `MINERAL_SOLD` ≥ 1(0 이면 FAIL) | `cargo test -p starfall-persistence h33_replay_matches_with_sales` [DB] + 실서버 월드: `cargo run -p starfall-persistence --bin history-replay -- --world <trade-cycle 월드>` (qa) | **판매 없는 월드에서 재생** → 출력의 `domain_events_replayed` 와 그중 `MINERAL_SOLD` 수를 찍고 0 이면 FAIL | history · qa | AC-12(d), R-4 | E1 E2 W |

### M. 데이터 배포 (AC-13, I-91, ADR-0015) — client · qa

| ID | 검증 항목 | 검증 방법 | ⊘ 자명 통과 → 배제 단언 | 담당 | 근거 | 불가 |
|----|----------|----------|------------------------|------|------|------|
| SC-56 | (a) SC-50(p1-02) 개정: **레지스트리에서 유도한 기대 집합**(`consumers` 에 `client` 가 있는 data 종류의 `data_files` glob)과 사본 집합을 **독립 순회 두 번**으로 비교 — 사본 전용 0, 누락 0, 바이트 동일, 기대 집합 크기 출력 | EditMode `ClientDataCopy_MatchesRegistryDerivedSet` + 음성 대조 `ClientDataCopy_ServerOnlyKindFile_IsDetected_NegativeControl`(임시 사본에 `world/deposit-reserves/cradle.json` 을 넣으면 실패) | **기대 집합이 비어 0 = 0** → 기대 집합 크기 > 0 단언(오늘 계산: SHIP_CLASS 1 · STAR_SYSTEM 1 · SYNC_TUNING 1 · MINERAL 4 · DEPOSIT_FIELD 1 · MINING_RULES 1 · MARKET 1 = **10**). **기대 집합을 사본에서 유도** → 원본 `data/` 순회와 사본 순회를 따로 찍는다 | client | AC-13(a) | E3 |
| SC-57 | (b) 사본에 `consumers` 에 `client` 가 없는 종류의 파일 **0** — 특히 `world/deposit-reserves/`·`history/rules/` 없음. 만기 "서버 전용 필드 분모 2 → 0" 의 실행 증거 | qa 독립 도구 `python tests/e2e/data_copy_scope.py` (Q2 신규 — 레지스트리 유도, 클라이언트 테스트와 독립 구현) + 음성 대조 `data_copy_scope.py --selftest`(임시 사본에 예비 파일 → FAIL) | **사본 경로 오타로 0 파일** → 사본 파일 수(기대 10) 출력, 0 이면 FAIL. 서버 전용 종류 목록(`DEPOSIT_RESERVE`·`SIGNIFICANCE_RULE`) 크기 2 출력 | qa | AC-13(b), ADR-0015 | — |
| SC-58 | (c) `data/**/*.json` **각각**이 정확히 하나의 data 종류 glob 에 걸린다 — 분모 = 파일 수 | `python tests/e2e/validate_data_files.py --glob-census` (Q2) | **파일 0 개** → 파일 수(오늘 12) 출력, 0 개·2 개 걸림 파일 목록. 음성 대조(selftest): 아무 glob 에도 안 걸리는 `data/unused/x.json`·두 glob 에 걸리는 파일 → FAIL | qa | AC-13(c), I-91 | — |
| SC-59 | (d) `validate_data_files.py` 의 대상이 레지스트리 `data_files` 에서 유도되고(손 목록 없음) **새 파일(시장·예비) 포함 12 파일 전부 0 오류** | `python tests/e2e/validate_data_files.py` → 종료 0, `files_checked = 12` + 음성 대조 `python tests/e2e/validate_data_files.py --selftest`(시장 파일 기준가를 소수로 바꾼 임시 사본 → 1 오류) | **glob 0 매치** → glob 별 파일 수 출력, 0 인 glob 오류. 파일 수 12 가 아니면 FAIL 이 아니라 architect·designer 통지(p1-02 SC-06 선례). `TARGETS` 손 목록이 소스에 없음은 grep 으로 부정(실행 짝 = 위 실행) | qa | AC-13(d) | — |

### N. 클라이언트 생성기·EditMode 계약 (AC-14) — client

| ID | 검증 항목 | 검증 방법 | ⊘ 자명 통과 → 배제 단언 | 담당 | 근거 | 불가 |
|----|----------|----------|------------------------|------|------|------|
| SC-60 | (a) 신규 DTO **5**(`SellMineralCommand`·`MineralSoldEvent`·`WalletStateMessage`·`SaleReceiptMessage`·`MarketStateMessage`) 생성, **2 회 멱등**, 기존 16 DTO 바이트 동일, `ContractTypes.cs` 만 변경 | `dotnet run ContractsCodegen.cs` 2 회 + 파일별 sha256 전후(`run_round.py` offline `codegen_compare`) | **생성기가 아무것도 안 씀** → 쓴 파일 수와 신규 5 의 존재 출력. **생성만 하고 컴파일하지 않음**(스펙 §5.2 정정 — 판정 기준은 Unity 컴파일 성공) → 같은 생성물로 돈 `unity test client --mode EditMode` 의 `test-results.xml`(SC-61 의 이름들이 실행됨 = 어셈블리 컴파일 성공)을 짝으로. CS0102(중첩 클래스·프로퍼티 이름 충돌)는 스키마 title `MarketLastSale` 로 풀고, 생성기는 이름 규칙을 바꾸지 않고 **충돌 감지 검사만** 더한다(리더 최종 결정) — 그래서 **기존 16 DTO 바이트 동일**이 생성기 변경의 회귀 확인이고, 새 감지 검사는 충돌하는 입력(title 을 되돌린 임시 스키마 사본)에서 **실제로 실패**함을 같은 증거에(규칙 6) | client | AC-14(a) | E3 E4 |
| SC-61 | (b) 유효 fixture **발견 수·왕복 수를 상수로**(예측 57/45 — 실측으로 확정), §0.5 C# 열을 실측 상수로(예측 53/33/27 = 113), `Runtime` 프로필에서 모르는 `reason_code` 에 죽지 않음 | EditMode `Fixtures_RoundTrip_Found*` · `CSharpLayerSplit_Totals*` · `UnknownClosedValue_ReasonCode_SurvivesRuntime` | **상수만 바꾸고 순회 0** → 순회한 파일 수를 `TestContext.WriteLine` 으로 찍고 상수와 같음 단언. 모르는 값이 입력 JSON 에 실제로 있음을 단언 | client | AC-14(b) | E3 |
| SC-62 | (c) `MARKET_STATE.last_sale` **null·비-null** 왕복(필수 nullable 중첩 객체) | EditMode `MarketState_LastSale_NullAndNonNull_RoundTrip` | **두 입력이 같은 모양** → null 입력과 객체 입력을 각각 단언, 키 누락 반례(`missing-last-sale-key.json`)는 거부(SC-61 의 C# 열) | client | AC-14(c) | E3 |

### O. 화면 요소 — UI-01~UI-13 각각 (AC-15, screen-elements S2) — client · 사람

요소마다 **① 표시 로직(EditMode, 순수 함수 — `client/Assets/_Project/Tests/EditMode/Trading*.cs`, C2) ② 존재 단언(PlayMode — 세션이 시작된 씬, `Tests/PlayMode/Trading*.cs`, C3) ③ 사람 체크 한 줄**. 이름 끝의 `*` 는 접두 일치(테스트 여럿 허용 — 결과 xml 의 메서드 이름이 그 접두로 시작). **기대값은 디자인 §8.3 의 문구·규칙과 §3.6 숫자에서 독립적으로** 정한다.

**③ 사람 체크의 판정 입력** (AC-15, AC-23, screen-elements S4): r1 이후 **최종 사람 세션**(`python tests/e2e/unity_session.py start --slice p1-03-trading --tag final-session --new-world`)에서 요소별 **예/아니오** 답(리더가 `evidence/final-session/answers.md` 에 원문) **+ 서버 기록**(`unity_session.py status` 의 이 월드 이벤트 종류별 수와 `/debug/stats` 델타). 스크린샷은 선택. ⊘ 공통: **그 장면을 만드는 사건이 서버 기록에 있어야** 답이 판정이 된다(각 행의 ⊘ 칸). 사전 세션(SC-133)의 답은 판정이 아니다. 다른 사람의 판매가 필요한 장면(UI-12)은 qa 가 세션 월드에 봇 `sell-once` 를 붙인다.

| ID | 검증 항목 | 검증 방법 | ⊘ 자명 통과 → 배제 단언 | 담당 | 근거 | 불가 |
|----|----------|----------|------------------------|------|------|------|
| SC-63 | **UI-01 ①** 거래소 표식 기하: 위치 (0, 1500, 0)·반지름 120·도킹 구역 반지름 **320**(= 120 + 200)을 시장 표에서, 도킹 표시 두 ● 초록이면 구역 색 변경 | EditMode `TradingUi01_*` | **상수 하드코딩으로 통과** → 입력 표의 반지름·사거리를 바꾼 두 번째 입력에서 구역 반지름이 따라 바뀜(입력 의존 단언) | client | AC-15 UI-01 ① | E3 |
| SC-64 | **UI-01 ②** 표식 GameObject **정확히 1**(= 시장 수) | PlayMode `TradingPlay_Ui01_*` | **0 개여도 통과** → 기대 개수 = 시장 표 파일 수(1)를 분모로 `== 1` 단언 | client | AC-15 UI-01 ② | E3 |
| SC-65 | **UI-01 ③** "별 위에 채굴 지점과 다르게 생긴 구조물이 보이는가" | 도구 없음(사람 관찰) — 최종 세션 답 + 서버 기록 | 세션이 실제로 열렸다(`SESSION_OPENED` ≥ 1, `live_connections` 1) | 사람 | AC-15 UI-01 ③ | E3 H W |
| SC-66 | **UI-02 ①** 표면 거리 문구(`Cradle Exchange · 4.2 km`, **< 2 km 이면 m**), 화면 밖·뒤일 때 가장자리 화살표 판정 | EditMode `TradingUi02_*` | **경계 한쪽만** → 1,999 m / 2,000 m 양쪽과 화면 앞·옆·뒤 세 방향을 각각 단언 | client | AC-15 UI-02 ① | E3 |
| SC-67 | **UI-02 ②** 거리 라벨 1, 화면 밖 지시자 존재 | PlayMode `TradingPlay_Ui02_*` | 라벨 수 `== 1`, 카메라를 거래소 반대로 돌린 상태에서 지시자 활성 | client | AC-15 UI-02 ② | E3 |
| SC-68 | **UI-02 ③** "거래소가 화면 밖일 때 가장자리에 방향과 거리가 보이는가" | 도구 없음(사람 관찰) — 최종 세션 답 + 서버 기록 | 같은 세션에서 함선이 이동했다(`SHIP_SPAWNED` ≥ 1 + 사람 답에 "돌아섰다" 동작) | 사람 | AC-15 UI-02 ③ | E3 H W |
| SC-69 | **UI-03 ①** 거리·속도 ● 색 — 경계값 **320 m / 10 m/s 양쪽**, 표면 거리 > 2 km 이면 숨김 | EditMode `TradingUi03_*` | 경계 네 점(319.9·320·320.1 / 10.0·10.1) 각각 단언 | client | AC-15 UI-03 ① | E3 |
| SC-70 | **UI-03 ②** 도킹 HUD 행 존재(2 km 안) | PlayMode `TradingPlay_Ui03_*` | **항상 숨김이라 존재 0** → 2 km 안으로 둔 상태에서 활성 1, 밖에서 비활성(두 상태) | client | AC-15 UI-03 ② | E3 |
| SC-71 | **UI-03 ③** "거래소에 가까워지면 도킹 거리·속도 표시가 나타나고 멈추면 둘 다 초록이 되는가" | 도구 없음(사람 관찰) — 최종 세션 답 + 서버 기록 | 그 actor 의 수락된 `MINERAL_SOLD` ≥ 1(도킹에 실제로 성공) | 사람 | AC-15 UI-03 ③ | E3 H W |
| SC-72 | **UI-04 ①** `₵ 20,000` 천 단위 구분, 0 이면 `₵ 0`, **`WALLET_STATE` 로만 바뀜** | EditMode `TradingUi04_*` | **표시가 아무 입력에도 안 바뀜** → `WALLET_STATE` 주입 **후** 바뀜(양성)과 `SALE_RECEIPT` 만 주입 시 불변(음성)을 둘 다 | client | AC-15 UI-04 ①, I-77 | E3 |
| SC-73 | **UI-04 ②** 세션 시작 직후 잔액 라벨 존재 | PlayMode `TradingPlay_Ui04_*` | 세션 시작 메시지 5 건 주입 뒤 라벨 1, 텍스트 = `₵ 0`(새 actor) | client | AC-15 UI-04 ② | E3 |
| SC-74 | **UI-04 ③** "판매 직후 크레딧 숫자가 오르는가" | 도구 없음(사람 관찰) — 최종 세션 답 + 서버 기록 | 그 actor 의 `MINERAL_SOLD` ≥ 1 | 사람 | AC-15 UI-04 ③ | E3 H W |
| SC-75 | **UI-05 ①** 행 문구 `Ferrosite  3 cr/kg ▼70%`(기준가 대비), 상장 0 이면 `상장된 광물 없음 — 먼저 캐야 한다`, 행 순서 = `mineral_id` 오름차순 | EditMode `TradingUi05_*` | ▼% 는 시장 표 기준가에서(10 → 3 = 70 %) — 다른 광물 1 개 이상으로도 단언 | client | AC-15 UI-05 ① | E3 |
| SC-76 | **UI-05 ②** 티커 행 수 = 받은 `listings` 수 | PlayMode `TradingPlay_Ui05_*` | **listings 0 이면 0 = 0** → 상장 2 개 `MARKET_STATE` 주입, 행 수 `== 2` | client | AC-15 UI-05 ② | E3 |
| SC-77 | **UI-05 ③** "어디서나 상장 광물의 시세가 보이고, 판매 후 그 값이 내려가는가" | 도구 없음(사람 관찰) — 최종 세션 답 + 서버 기록 | 판매 tick 의 `MARKET_STATE` 에서 그 광물 `price_after < price_before`(서버 기록 `MINERAL_SOLD.price_*`) | 사람 | AC-15 UI-05 ③ | E3 H W |
| SC-78 | **UI-06 ①** `미상장 N종` = 광물 수 − 상장 수, **미상장 광물 이름이 어떤 행에도 없음** | EditMode `TradingUi06_*` | **광물 표가 비어 N = 0** → 광물 4·상장 1 → `미상장 3종`, 나머지 셋의 `display_name` 이 티커·패널 문자열 어디에도 없음 | client | AC-15 UI-06 ① | E3 |
| SC-79 | **UI-06 ②** `미상장 N종` 문구 존재 | PlayMode `TradingPlay_Ui06_*` | 상장 1 주입 뒤 문구 1, 전부 상장 주입 뒤 사라짐(두 상태) | client | AC-15 UI-06 ② | E3 |
| SC-80 | **UI-06 ③** "캐 본 적 없는 광물의 이름이 시장 화면에 없는가" | 도구 없음(사람 관찰) — 최종 세션 답 + 서버 기록 | 새 월드 + 세션 중 그 월드의 드러난 광물 수 < 4(서버 기록 — 광물별 첫 `MINERAL_MINED`) | 사람 | AC-15 UI-06 ③ | E3 H W |
| SC-81 | **UI-07 ①** 열림 = 두 표시 조건 참, 행 내용(시세·포화 막대·`이 값으로 N kg 더`/`바닥가`·보유량), 보유 0 흐림, ↑↓ 선택 이동 | EditMode `TradingUi07_*` | 바닥 행·비바닥 행·보유 0 행을 각각 포함한 입력 | client | AC-15 UI-07 ① | E3 |
| SC-82 | **UI-07 ②** 도킹 표시 참일 때 패널 활성, 거짓일 때 비활성 | PlayMode `TradingPlay_Ui07_*` | 두 상태 각각 단언(한 상태만이면 무효) | client | AC-15 UI-07 ② | E3 |
| SC-83 | **UI-07 ③** "도킹 조건을 만족하면 패널이 열리고 벗어나면 닫히는가, ↑↓로 행이 바뀌는가" | 도구 없음(사람 관찰) — 최종 세션 답 + 서버 기록 | 수락된 판매 ≥ 1 그리고 세션 상장 ≥ 2(행 이동이 가능) | 사람 | AC-15 UI-07 ③ | E3 H W |
| SC-84 | **UI-08 ①** "이 값까지" 양 = `(band + 1) × band_kg − s`, 바닥이면 프리셋 하나, **예상 대금 = 디자인 §3.3 함수**(§0.6 P1~P10 과 같은 상수표 — 서버 SC-14 와 같은 입력에서 같은 값), 보유 0 이면 `팔 것 없음`·Enter 무효 | EditMode `TradingUi08_*` | **서버와 클라이언트가 같은 틀린 함수** → 기대값은 §0.6 표(디자인 손계산)에서. 프리셋 양 단언은 s 가 구간 경계가 아닌 입력(예: s = 600 → 1,400 kg — 디자인 §8.3 예) | client | AC-15 UI-08 ①, T-2 | E3 |
| SC-85 | **UI-08 ②** ←→ 입력이 프리셋을 바꾼다 | PlayMode `TradingPlay_Ui08_*` | 보유 > 프리셋 양인 상태(두 프리셋이 다름)에서 → 입력 후 선택 프리셋 변경 | client | AC-15 UI-08 ② | E3 |
| SC-86 | **UI-08 ③** "←→로 판매량이 바뀌고 예상 금액이 따라 바뀌는가" | 도구 없음(사람 관찰) — 최종 세션 답 + 서버 기록 | 그 세션의 판매 중 "이 값까지" 프리셋 사용 ≥ 1 은 요구하지 않는다(사람 답). 서버 기록: 수락 판매 ≥ 1 | 사람 | AC-15 UI-08 ③ | E3 H W |
| SC-87 | **UI-09 ①** 알림 문구 = `SALE_RECEIPT` 의 **실제** 값(예상과 다른 입력에서 실제를 씀), 4 s. **둘째 줄**: 예상 > 실제면 `예상보다 ₵N 적다 — 직전에 다른 판매가 있었다`(N = 차이), 예상 ≤ 실제면 없음 — **두 경우 각각** | EditMode `TradingUi09_*` | **예상 = 실제 입력만** → 예상 20,000·실제 18,000(N = 2,000) 과 예상 18,000·실제 20,000(둘째 줄 없음) 두 입력 | client | AC-15 UI-09 ①, 디자인 §2.1a | E3 |
| SC-88 | **UI-09 ②** `SALE_RECEIPT` 주입 → 알림 생성 | PlayMode `TradingPlay_Ui09_*` | 주입 전 알림 0, 후 1 | client | AC-15 UI-09 ② | E3 |
| SC-89 | **UI-09 ③** "Enter 뒤 금액 알림이 화면 아래에 뜨는가" | 도구 없음(사람 관찰) — 최종 세션 답 + 서버 기록 | 그 actor 의 `MINERAL_SOLD` ≥ 1 | 사람 | AC-15 UI-09 ③ | E3 H W |
| SC-90 | **UI-10 ①** 직전 메시지에 없던 광물만 알림, **세션 첫 메시지는 알림 없음** | EditMode `TradingUi10_*` | 첫 메시지에 상장 2 → 알림 0, 둘째에 1 추가 → 알림 1 | client | AC-15 UI-10 ① | E3 |
| SC-91 | **UI-10 ②** `MARKET_STATE` 두 번 주입 → 알림 **1** | PlayMode `TradingPlay_Ui10_*` | 두 메시지의 상장 집합이 다름(1 개 추가)을 단언 | client | AC-15 UI-10 ② | E3 |
| SC-92 | **UI-10 ③** "처음 캔 광물이 시장에 '새 상장'으로 뜨는가" (+ T-12: 새 상장 알림과 발견 배너의 순서가 어색했는가 — 기록 M-8) | 도구 없음(사람 관찰) — 최종 세션 답 + 서버 기록 | 새 월드에서 그 actor 가 광물의 첫 `MINERAL_MINED`(그 광물 `MINERAL_DISCOVERED` 의 근거) | 사람 | AC-15 UI-10 ③, T-12 | E3 H W |
| SC-93 | **UI-11 ①** 사유별 문장(사거리·속도·보유·쿨다운, 기타 = 코드 그대로, **모르는 코드**) | EditMode `TradingUi11_*` | 아는 사유 4 개가 서로 다른 비어 있지 않은 문장, 모르는 코드에서 예외 없음 | client | AC-15 UI-11 ① | E3 |
| SC-94 | **UI-11 ②** 거절 결과 주입 → 줄 생성 | PlayMode `TradingPlay_Ui11_*` | 주입 전 줄 0, 후 1, 다음 판매 시도에 지워짐 | client | AC-15 UI-11 ② | E3 |
| SC-95 | **UI-11 ③** "너무 빠를 때 Enter를 누르면 이유가 한 줄로 보이는가" | 도구 없음(사람 관찰) — 최종 세션 답 + 서버 기록 | `/debug/stats` `commands_rejected_total{SHIP_TOO_FAST}` 세션 중 델타 ≥ 1 | 사람 | AC-15 UI-11 ③ | E3 H W |
| SC-96 | **UI-12 ①** `Pilot-xxxx`(I-66 끝 4 글자)·kg·경과 시간, **내 것이면 `나`**, null 이면 `거래 없음` | EditMode `TradingUi12_*` | 세 경우(남·나·null) 각각, 경과 시간은 tick 차에서 | client | AC-15 UI-12 ① | E3 |
| SC-97 | **UI-12 ②** 행마다 마지막 거래 줄 존재 | PlayMode `TradingPlay_Ui12_*` | 상장 2 행 → 줄 2(하나는 `거래 없음`) | client | AC-15 UI-12 ② | E3 |
| SC-98 | **UI-12 ③** "다른 사람이 판 광물 행에 그 사람 표지와 경과 시간이 보이는가" | 도구 없음(사람 관찰) — 최종 세션 답 + 서버 기록 + 봇 `bots probe --case sell-once`(qa 가 세션 월드에 붙임, 관측용) | 세션 월드에 **다른 actor** 의 `MINERAL_SOLD` ≥ 1(봇) | 사람 · qa | AC-15 UI-12 ③ | E3 E6 H W |
| SC-99 | **UI-13 ①** 표지는 `MINERAL_DISCOVERED` NOTICE(BACKFILL·LIVE)에서만 — **같은 입력에 `last_sale` 만 바꿔도 표지 불변**, 상장됐는데 NOTICE 없으면 `발견: 기록 중…`, NOTICE 뒤 `발견: Pilot-xxxx`, 내가 발견자면 `발견: 나`, 한 번 붙으면 사라지지 않음 | EditMode `TradingUi13_*` | **`MARKET_STATE` 에서 읽어도 통과** → `last_sale.actor_id` 를 발견자와 다르게 둔 입력에서 표지 = NOTICE 의 발견자 | client | AC-15 UI-13 ①, 디자인 §5.3 | E3 |
| SC-100 | **UI-13 ②** 상장 N + 발견 NOTICE N 주입 → 표지 N. **⊘ 둘(history3 S-2)**: 상장을 NOTICE 보다 **먼저** 주입해 `발견: 기록 중…` 을 실제로 거친 뒤 표지로 바뀜, 그리고 **재접속(새 세션)에서 BACKFILL 로만 받은 NOTICE** 로도 표지 | PlayMode `TradingPlay_Ui13_*` | **NOTICE 를 먼저 넣어 `기록 중…` 을 안 거침** → 중간 상태 문자열을 단언. N ≥ 2. BACKFILL 경로는 LIVE 를 한 번도 주입하지 않은 새 세션 | client | AC-15 UI-13 ② | E3 |
| SC-101 | **UI-13 ③** "시장 패널의 광물 이름 옆에 그 광물을 처음 발견한 사람의 표지가 보이는가" | 도구 없음(사람 관찰) — 최종 세션 답 + 서버 기록 | 세션 월드에 `MINERAL_DISCOVERED` ≥ 1(`historical_events`) | 사람 | AC-15 UI-13 ③ | E3 H W |

### P. 클라이언트는 계산하지 않는다 (AC-16, I-76·I-77·I-78) — client

| ID | 검증 항목 | 검증 방법 | ⊘ 자명 통과 → 배제 단언 | 담당 | 근거 | 불가 |
|----|----------|----------|------------------------|------|------|------|
| SC-102 | (a) 인벤토리·잔액·현재 시세가 **서버 메시지 수신에서만** 바뀐다 — 예상 대금 계산 뒤에도 표시 잔액·보유량 불변, `WALLET_STATE`·`INVENTORY_STATE` 주입 뒤에만 바뀜 | EditMode `Trading_ClientNeverComputesBalanceOrHoldings` | **아무것도 안 바뀌는 구현** → 메시지 주입 **후** 바뀜(양성)을 같은 테스트에서 | client | AC-16(a) | E3 |
| SC-103 | (b) 소스 검사(부정 증명): 잔액·보유량에 더하거나 빼는 경로 없음 | `grep -rnE "(BalanceCr\|balance_cr\|QuantityKg\|quantity_kg)\s*[+\-]=" client/Assets/_Project/Scripts` → 0 매칭 | **패턴이 우리 서식을 못 잡음** → 같은 패턴을 테스트 소스의 의도적 표본(또는 `+=` 가 있는 임의 파일)에 돌려 ≥ 1(양성 대조). **SC-102 와 짝으로만 PASS**(규칙 9) | client | AC-16(b) | — |

### Q. 키 (AC-17) — client

| ID | 검증 항목 | 검증 방법 | ⊘ 자명 통과 → 배제 단언 | 담당 | 근거 | 불가 |
|----|----------|----------|------------------------|------|------|------|
| SC-104 | ↑↓←→ Enter 는 시장 패널이 열려 있을 때만 명령을 만든다: 닫혀 있을 때 Enter → 송신 **0**, 열려 있을 때 → `SELL_MINERAL` **1**(payload 키 = `{market_id, mineral_id, quantity_kg}` 뿐) | PlayMode `TradingPlay_EnterSendsOnlyWhenPanelOpen` (FakeRealtimeTransport 송신 기록) | **송신 경로가 죽어 0** → 열림 상태 송신 1 을 같은 테스트에서. payload 키 집합 같음 | client | AC-17, I-74 | E3 |
| SC-105 | 새 키가 기존 바인딩(디자인 §8.1 표 — W/S/A/D/R/F/Q/E/X/Z/H/G)과 겹치지 않음 | EditMode `TradingKeys_NoOverlapWithExistingBindings` (바인딩 목록 테스트) | **기존 목록을 비워 공짜 통과** → 기존 바인딩 수 12 를 단언하고 그 목록을 소스에서 수집(하드코딩 아님 — 수집 방법을 테스트 주석에) | client | AC-17 | E3 |

### R. 치트 전수 (AC-18, 디자인 T-5·T-6) — qa

**시도 수와 막은 수**를 항목마다 찍는다. 사유별 관찰 수가 0 이면 그 경로를 안 탄 실행 → **무효**. 각 케이스는 그동안 **지갑·인벤토리·포화·`MINERAL_SOLD` 행 수 불변**을 SQL·수신 메시지로 단언하고, `/debug/stats` `commands_rejected_total{사유}` 델타 = 봇이 받은 그 사유 수(`stats_delta.py`, SC-07 ③). 모든 케이스는 **판매 전 정상 판매 1 회**로 상태를 0 아닌 값으로 만든다(공짜 "불변" 배제).

| ID | 검증 항목 | 검증 방법 | ⊘ 자명 통과 → 배제 단언 | 담당 | 근거 | 불가 |
|----|----------|----------|------------------------|------|------|------|
| SC-106 | (a) 가격·대금·잔액·위치·actor 주입, `quantity_kg` 0·1.5 → **7 경우 전부** `MALFORMED_COMMAND`, 상태 불변 | `bots probe --case cheat-sell-inject` | **명령이 서버에 도달하지 않음**(봇 직렬화가 필드를 버림) → 보낸 **원문 프레임**에 주입 필드·값이 있음을 봇 로그에 | qa | AC-18(a), I-74·I-75 | E2 E5 E6 W |
| SC-107 | (b) 도킹 밖 500 m → `TARGET_OUT_OF_RANGE` | `bots probe --case cheat-sell-range` | **다른 사유로 거부** → 사유 코드 정확히, 그 시점 서버 스냅샷 거리 > 320 m(표면 + 200)·속도 ≤ 10 | qa | AC-18(b) | E2 E5 E6 W |
| SC-108 | (c) 30 m/s 통과 중 → `SHIP_TOO_FAST` | `bots probe --case cheat-sell-fast` | **실제 속도 ≤ 한계** → 명령 tick 스냅샷 속도 > 10, 도킹 구역 **안**(아니면 사거리 사유가 먼저) | qa | AC-18(c) | E2 E5 E6 W |
| SC-109 | (d) 보유 + 1 → `INSUFFICIENT_QUANTITY` | `bots probe --case cheat-sell-overqty` | **보유 0** → 직전 `INVENTORY_STATE` 의 보유 > 0, 요청 = 보유 + 1 | qa | AC-18(d) | E2 E5 E6 W |
| SC-110 | (e) 성공 직후 0.5 s → `COOLDOWN_ACTIVE`, **연결 유지** | `bots probe --case cheat-sell-cooldown` | 직전 판매 수락 tick 과 재시도 tick 차 < 20, 종료 시 연결 열림, `RATE_LIMITED` 0 | qa | AC-18(e) | E2 E5 E6 W |
| SC-111 | (f) 없는 시장·없는 광물 → 둘 다 `TARGET_UNKNOWN` | `bots probe --case cheat-sell-unknown` | **스키마 위반 id 라 `MALFORMED_COMMAND`** → `DataId` 형식에 맞는 없는 id 를 쓴다. 두 경우 각각 관찰 수 ≥ 1 | qa | AC-18(f) | E2 E5 E6 W |
| SC-112 | (g) AC-5 (a)(b)(c)(e) 를 봇으로: 같은 세션 재전송 · 잔류 창 안 재접속 재전송 · **정상 종료 → 재기동 → 재전송** · 명령 종류 교차(채굴 키 → 판매, 판매 키 → 채굴) → 전부 `DUPLICATE_COMMAND_ID`, 지갑 대금 × 1, 재기동 경로는 `processed_commands` 행 수 불변 | `bots probe --case sell-dup` · `bots probe --case sell-dup-reconnect` · `restart_cases.py --out <dir> sell-dup`(봇 `sell-dup-restart`) · `bots probe --case sell-dup-cross-kind` | **첫 전송이 거부됨** → 경로마다 첫 전송 `ACCEPTED`·지갑 증가 선단언. 재기동은 stdin `shutdown`. 네 경로 각각 중복 관찰 수 ≥ 1 | qa | AC-18(g), AC-5, T-6 | E2 E5 E6 W |

### S. "B 가 흔적을 본다" — 슬라이스의 핵심 (AC-19, 디자인 T-3) — qa · 새 월드

**도달 가능성(R4)**: A 가 Ferrosite 14,000 kg 을 모은다 — IB1(12,000) 에서 채굴 속도 200 kg/3 s > 회복 200 kg/30 s 이므로 약 200 s 에 ≈ 13,300 kg, 나머지 ≈ 700 kg 은 IB3 에서(≤ 60 s + 이동). 판매 1 회 = 포화 0 → 14,000(= 상한) → `price_after = 3`(P2, 대금 98,000). B 의 접속 창: 판매 tick t → A 종료 → 잔류 600 tick(30 s) → 디스폰 → B 접속 ≤ t + 1,200 이므로 **창 ≈ 600 − (A 종료 지연) tick ≈ 29 s**. B 는 미리 빌드한 별도 프로세스라 접속 < 1 s. 창을 놓치면 **무효**(재시도는 새 월드).

| ID | 검증 항목 | 검증 방법 | ⊘ 자명 통과 → 배제 단언 | 담당 | 근거 | 불가 |
|----|----------|----------|------------------------|------|------|------|
| SC-113 | (a) A 가 Ferrosite 14,000 kg 을 **한 번에** 판다 → 바닥(`price_after_cr_per_kg = 3`), 대금 98,000(P2) | `bots probe --case trace-sell-a` | **이미 포화된 월드** → 새 월드, 판매 직전 `MARKET_STATE` 의 Ferrosite 포화 0, 판매 `quantity_kg = 14,000` 이 1 건 | qa | AC-19(a), T-3 | E2 E5 E6 W |
| SC-114 | (b) A 가 접속을 끊고 **A 의 함선 디스폰을 단언**한 뒤, **A 의 판매 tick 부터 1,200 tick 안에** B 가 처음 접속 | `trace-sell-a` 종료 단계 + **별도 프로세스** `bots probe --case trace-sell-b --seller-label <A> --world <id>` | **A 가 아직 월드에 있음** → `SHIP_DESPAWNED{LINGER_EXPIRED}`(A 의 함선) 행 tick < B 의 `SESSION_OPENED` tick. **tick 차(B 접속 − 판매) 출력**, > 1,200 이면 무효. B 의 actor 가 그 월드에 `SESSION_OPENED` 이력 0 | qa | AC-19(b) | E2 E5 E6 W |
| SC-115 | (c) B 가 받은 **첫** `MARKET_STATE` 에서 Ferrosite 시세 < 기준가(10), `last_sale.actor_id = A`(전체 UUID), `quantity_kg = 14,000` | `bots probe --case trace-sell-b` 판정(입력 = B 가 받은 메시지뿐) | **B 가 A 와 같은 프로세스라 기억으로 앎** → B 는 별도 프로세스, 판정 입력은 B 의 수신 프레임뿐(B 로그에 원문). **회복이 이미 기준가로 돌려놓음** → (b) 의 tick 차와 B 가 본 포화 출력 | qa | AC-19(c), GDD §36 | E2 E5 E6 W |

### T. 경쟁과 부하 (AC-20, 디자인 T-4·T-11, H-34) — qa

| ID | 검증 항목 | 검증 방법 | ⊘ 자명 통과 → 배제 단언 | 담당 | 근거 | 불가 |
|----|----------|----------|------------------------|------|------|------|
| SC-116 | (a) 두 봇이 같은 목표 tick 에 Ferrosite 2,000 kg 판매 → `sequence` 작은 쪽 **20,000**·큰 쪽 **18,000**(P9·P10), 두 지갑 합 = 두 대금 합 = 38,000 | `bots probe --case race-sell-same-tick` | **두 `MINERAL_SOLD` 가 같은 tick 이 아님** → 그 단언이 먼저, 아니면 **무효**(시도·무효 횟수 출력). **포화 0 이 아님** → 새 월드, 직전 `MARKET_STATE` Ferrosite 포화 0. 도달 가능성: 각 봇 2,000 kg = 30 s 채굴(IB1·IB2), 같은 tick 겨냥은 p1-02 SC-84 에서 성립. 재시도마다 새 월드(포화 4,000 → 0 회복은 4 분이라 새 월드가 빠르다) | qa | AC-20(a), T-4 | E2 E5 E6 W |
| SC-117 | (b) **31 연결**(봇 30 + Unity 1 또는 봇 31)이 채굴·운반·판매 **20 분**: tick 초과 비율 **≤ 0.5 %** | `bots run --stage sell-load`(새 월드, 원본 `data/`) + `stats_delta.py` | **31 연결이 실제로 유지 안 됨 / 판매가 일어나지 않음** → 실행 중 `live_connections` 최솟값 = 31, 판매 수·광물별 판매량 출력(판매 0 이면 FAIL) | qa | AC-20(b), T-11 | E2 E5 E6 E7 W |
| SC-118 | (b) 같은 부하 월드에서 항등식 전부(SC-30~33 의 네 하위 명령) 성립 | `trade_ledger.py --world <load 월드> ledger` · `chain` · `conservation` · `saturation` | SC-30 의 실행 모양 요건을 이 월드에도(판매 actor ≥ 2 · 판매 ≥ 10 · 광물 ≥ 2 · 구간 걸침 ≥ 1 · 회복 쌍 ≥ 1) | qa | AC-20(b) | E2 E5 E6 E7 W |
| SC-119 | (b) 정상 부하에서 **`RECORDING_BACKLOG` 거부 0** | `sell-load` 사유별 집계 + `recording_lag` 최대(`/debug/stats`) | **백로그 경로가 죽어 0** → SC-38·39 가 같은 빌드에서 PASS 여야 이 항목이 PASS(검출기 생존). `recording_lag` 최대값 출력 | qa | AC-20(b) | E2 E5 E6 E7 W |
| SC-120 | (b) **바닥에 닿은 광물 ≥ 1**, **회복이 관찰된 광물 ≥ 1** | `trade_ledger.py --world <load 월드> market-events`(판매 중 `price_after = 바닥가` 인 광물 수, 이웃 판매 사이 회복 쌍이 있는 광물 수) | **붕괴·회복 경로를 안 탄 실행** → 이 둘이 판정 조건 자체. 도달 가능성(R4): Ferrosite 초기 33 t ≥ 포화 상한 14 t, 30 봇 중 Ferrosite 봇 ~15 대가 사이클(≈ 100 s)마다 2,000 kg → 분당 ≈ 18 t 공급 > 시장 회복 1 t/분 → 첫 4~5 분 안에 바닥 도달. 회복은 바닥 뒤 60 s 이상 지나 같은 광물이 다시 팔리면 관찰된다(20 분 실행). 봇 정책은 §3.1 `sell-load` 에 고정 | qa | AC-20(b), T-11 | E2 E5 E6 E7 W |
| SC-121 | H-34: 거래가 섞인 상태에서 새 광물 발견 `HISTORICAL_EVENT_NOTICE{LIVE}` 수신, 지연 **max ≤ 5 초**(실패 경계), p95 기록 | `bots probe --case trade-cycle` (A·B 판매 반복 중 C 가 Glacine 첫 채굴 — SC-30~33 과 같은 월드) | **판매 없는 구간의 발견** → 그 발견 tick 이전 같은 월드 `MINERAL_SOLD` ≥ 1 을 먼저 단언. 수신 세션 수(≥ 2)와 지연 표본 수 출력 | qa | AC-20(b), H-34 | E2 E5 E6 W |

### U. 기록 무결성 (AC-21) — qa

대상 월드: `trade-cycle`·`cases`(SC-106~112)·`sell-load`. 월드마다 따로 판정한다.

| ID | 검증 항목 | 검증 방법 | ⊘ 자명 통과 → 배제 단언 | 담당 | 근거 | 불가 |
|----|----------|----------|------------------------|------|------|------|
| SC-122 | (a) 그 월드의 `event_type` distinct = **기대 집합** `SESSION_OPENED`·`SESSION_CLOSED`·`SHIP_SPAWNED`·`SHIP_DESPAWNED`·`MINERAL_MINED`·`MINERAL_SOLD`(같음 — 부분집합 아님) | SQL `SELECT event_type, count(*) FROM domain_events WHERE world_id = '<id>' GROUP BY 1` | **판매를 안 해 5 종만** → 6 종 전부 ≥ 1 이 판정 조건 | qa | AC-21(a) | E2 W |
| SC-123 | (b) `MINERAL_MINED`·`MINERAL_SOLD` 의 `causation_id` 와 `processed_commands.command_id` 양방향 누락 0, `command_type` 이 이벤트 타입과 맞는다(`SELL_MINERAL` ↔ `MINERAL_SOLD`, `MINE_RESOURCE` ↔ `MINERAL_MINED`) | `trade_ledger.py --world <id> causation` | **둘 다 0 행** → 행 수 출력, 0 이면 FAIL. 두 방향 누락 수, 종류 불일치 수 각각. `SELL_MINERAL` 장부 행 ≥ 1 | qa | AC-21(b), I-82 | E2 W |
| SC-124 | (c) `(world_id, tick)` 별 `sequence` 빈틈 없음 | `python tests/e2e/check_sequence_gaps.py --world <id>` | **검사 tick 0** → tick 수·행 수, 판매 tick(이벤트 ≥ 1 인 판매 tick) ≥ 1 | qa | AC-21(c) | E2 W |
| SC-125 | (d) 자기 참조 결함 장부 **7 건 그대로** — `causation_id = event_id` 집합이 동결 장부와 정확히 같다 | SQL `SELECT event_id FROM domain_events WHERE causation_id = event_id ORDER BY 1` 을 p1-02 r2 증거(`_workspace/p1-02-mining/evidence/r2_20261003/offline/sc94.txt`)와 비교 | **질의 0 행** → 결과 7 행이 판정 조건. 6 = 추가 전용 위반, 8 = 새 결함 | qa | AC-21(d), 원칙 5 | E2 |
| SC-126 | (e) **제약 census**: `pg_constraint` 실측 = ADR-0013 §5a + ADR-0016 §5 요구 집합 — 경제 **72**(0001·0002 54 + `wallets` 6 + `market_states` 12) + 역사 **50** = **122**, 트리거 **4**. 이름 집합도 같다 | `python tests/e2e/constraint_census.py --slice p1-03` (ADR-0016 §5 를 합쳐 수 + 0004 이름 집합 대조) | **0004 미적용 DB 에서 판정** → `_sqlx_migrations` 적용 버전 목록을 먼저 찍고 0004 까지가 아니면 판정하지 않는다(exit 4). 테이블별 수와 요구 수 나란히, 이름 차집합 양방향 | qa | AC-21(e), ADR-0016 §5 | E2 |

### V. 계약 커버리지·경계면·CI (AC-22) — qa · 사람

| ID | 검증 항목 | 검증 방법 | ⊘ 자명 통과 → 배제 단언 | 담당 | 근거 | 불가 |
|----|----------|----------|------------------------|------|------|------|
| SC-127 | (a) `check_contract_coverage.py --strict` **종료 코드 0** (기준선 errors 17) | `python .claude/skills/integration-qa/scripts/check_contract_coverage.py --strict` | **타입이 줄어 errors 0** → `types: 30` 을 판정 조건에 | qa | AC-22(a) | E9 |
| SC-128 | (b) 신규 타입(`SELL_MINERAL`·`MINERAL_SOLD`·`WALLET_STATE`·`SALE_RECEIPT`·`MARKET_STATE`·`MARKET`·`DEPOSIT_RESERVE` + 변경 `DEPOSIT_FIELD`·`COMMAND_RESULT`)의 스키마·Rust·C# **필드별 표**, 불일치 0 — §4 의 고정 대상 전부 | `python tests/e2e/interface_matrix.py --slice p1-03` (Q2 — p1-03 타입 목록) + qa 가 세 파일을 함께 읽어 표 | **행이 스키마에서만 뽑혀 Rust·C# 누락이 안 보임** → 행 = 세 쪽 필드의 **합집합**, 타입별 필드 수 세 쪽 각각 | qa | AC-22(b) | E9 |
| SC-129 | (c) **읽히지 않는 데이터 파일 0** — `data/**/*.json` 각 파일마다 ① 서버 기동 로그의 해석 경로 줄(실행 증거) 또는 클라이언트 사본 소비 테스트 ② 게이트(스키마 + 검산 테스트 또는 golden) — §2 표 | `python tests/e2e/data_file_pairs.py --boot-log <SC-04 로그> --cargo-log <로그>` (Q2 — 12 파일) | **분모 0** → 같은 실행에서 `find data -name '*.json' \| wc -l` = 12 를 찍는다. 음성 대조: 사본 `data/unused/x.json` → FAIL. "읽는 코드" 를 소스 grep 으로 채우지 않는다(규칙 9) | qa | AC-22(c) | E2 E5 |
| SC-130 | (d) `gates.yml` 출처 게이트가 **이 계약**을 가리키고 PR 에서 초록, 찍힌 **항목 분모 = 133** | PR 의 `judgment gates (python)` job 로그: `python tests/e2e/check_item_sources.py --contract _workspace/p1-03-trading/02_sprint_contract.md --slice p1-03` | **다른 슬라이스 라벨을 전부 제외해 볼 것 0 인 채 exit 0** → "이 슬라이스 라벨 수 / 제외한 다른 슬라이스 라벨 수" 둘 다. Phase 5 에서 이 슬라이스 라벨 수 ≥ 1. `KNOWN_SLICES` 에 `p1-03` 추가(Q2) | qa | AC-22(d) | — |
| SC-131 | (e) Unity EditMode·PlayMode 는 CI 밖 → **병합 조건으로 사람이 돌린** `test-results.xml` 이 `_workspace/p1-03-trading/evidence/` 에 있고, 스위트별 total·passed·failed·skipped 가 PR 본문에 | PR 템플릿 체크 줄(§3.3) + 파일 존재 + 파일의 HEAD 표기 | **옛 결과 파일** → xml 생성 시각 > 마지막 `client/` 변경 시각, xml 의 테스트 수 ≥ SC-61·63~105 의 Unity 이름 수 | 사람 · qa | AC-22(e) | E3 H |

### W. 응답 항등식과 사람 사전 세션 (스펙 §5.1a, AC-23) — qa · 리더

| ID | 검증 항목 | 검증 방법 | ⊘ 자명 통과 → 배제 단언 | 담당 | 근거 | 불가 |
|----|----------|----------|------------------------|------|------|------|
| SC-132 | 스펙 §5.1a 유도 항등식: **수락된 `SELL_MINERAL` 수 == 결과 뒤 `SALE_RECEIPT` 수 == 그 뒤 `WALLET_STATE` 수**, 순서 `COMMAND_RESULT` → `SALE_RECEIPT` → `INVENTORY_STATE` → `WALLET_STATE` 같은 tick — 봇이 받은 메시지로 | 봇 공통 집계 요약 줄 — `bots probe --case trade-cycle` · `bots probe --case race-sell-same-tick` · `bots run --stage sell-load` 실행마다 | **판매 0 건이면 0 == 0** → 수락 0 이면 FAIL 인쇄(스펙 §5.1a 명시). 순서 위반 수 출력 | qa | 스펙 §5.1a, AC-18·AC-20 | E2 E5 E6 W |
| SC-133 | **사람 사전 세션이 r1 동결 전에 있었다**: `evidence/presession/notes.md` 에 UI-01~UI-13 **요소별 예/아니오** 13 답 + "거래소까지 몇 초"(목표 ≤ 60 s) + T-3 B 질문 답 + T-12 알림 순서 답, "아니오" 는 r1 전 결함 목록으로 | `unity_session.py start --slice p1-03-trading --tag presession --new-world` 의 `session.json`·`notes.md` | **동결 뒤에 함** → `notes.md` mtime < `FREEZE` 의 시작 시각. 세션이 실제로 열렸다(`status` 의 `live_connections ≥ 1`·`SESSION_OPENED ≥ 1`). 답 13 개(빈칸 0) | 리더 | AC-23, screen-elements S4 | E3 H W |

---

## 2. 데이터 파일 짝 표 (SC-129 의 채점 기준 — AC-22(c))

**분모 = `data/**/*.json` 12 파일** (qa3 `find` 2026-10-05). ① 은 **실행 증거**(서버 기동 로그의 해석 경로 줄 — p1-02 S2 의 `데이터 파일 적재 data_file=…` 형식, 또는 클라이언트 전용 소비는 EditMode 테스트 이름)여야 하고 소스 읽기로 채우지 않는다(규칙 9).

| # | 파일 | data 종류 | 소비자(레지스트리) | ① 읽는 코드 (기대 — 로그로 확인) | ② 게이트 |
|---|------|---------|----------|------------------------------|---------|
| 1 | `data/ships/scout-s01.json` | SHIP_CLASS | server, client | 서버 기동 로그 | 스키마 + p1-01 테스트 |
| 2 | `data/world/systems/cradle.json` | STAR_SYSTEM | server, client | 서버 기동 로그 | 스키마 + p1-01 테스트 |
| 3 | `data/movement/sync-tuning.json` | SYNC_TUNING | server, client | 서버 기동 로그 | 스키마 |
| 4~7 | `data/minerals/*.json` | MINERAL | server, client | 서버 기동 로그 | 스키마 + p1-02 SC-05 계열 |
| 8 | `data/world/deposits/cradle.json` (v2) | DEPOSIT_FIELD | server, client | 서버 기동 로그 | 스키마 + SC-05 d01·d03 |
| 9 | `data/world/deposit-reserves/cradle.json` | DEPOSIT_RESERVE | **server** | 서버 기동 로그(S2 신규) | 스키마 + SC-05 d02·d04 |
| 10 | `data/mining/mining-rules.json` | MINING_RULES | server, client | 서버 기동 로그 | 스키마 |
| 11 | `data/history/rules/mineral-discovery.json` | SIGNIFICANCE_RULE | **history** | 서버 기동 로그(history 코어에 값 전달) | 스키마 + 역사 golden(SC-41) |
| 12 | `data/market/cradle-exchange.json` | MARKET | server, client | 서버 기동 로그(S2 신규) | 스키마 + SC-05 m01~m12 + **SC-21 golden** |

---

## 3. QA 소유 도구와 CI 게이트 (Q2·Q3)

### 3.1 봇 케이스 (`tools/bots/` — Q3)

`bots probe --case <이름>` 로 부른다. **이름은 §1 행의 백틱 안 토큰과 글자 그대로 같아야** 출처 게이트의 Rust 판정이 잇는다(`rust_case_refs.py` — 완전 일치). verdict 라벨은 `ContractRef::Verdict("p1-03 SC-nn …")`.

| 케이스 | 하는 일 | verdict 항목 |
|--------|--------|-------------|
| `trade-cycle` | 봇 A·B Ferrosite 채굴 2,000 kg → 거래소 항법(도킹·감속) → 판매 반복, 봇 C Glacine 첫 채굴(판매 ≥ 1 뒤) → 판매. 15 분 상한 | SC-121 · SC-132 (월드는 SC-30~33·55·122~124 입력) |
| `leak-scan-market` | 새 월드 세션 2 개, 첫 채굴 전·Ferrosite 첫 채굴·Starfall Glass 첫 채굴 원문 프레임 저장·검색, 세션 시작 첫 5 프레임 | SC-45 · SC-46 · SC-47 |
| `cheat-sell-inject` · `cheat-sell-range` · `cheat-sell-fast` · `cheat-sell-overqty` · `cheat-sell-cooldown` · `cheat-sell-unknown` | AC-18 (a)~(f) | SC-106~111 |
| `sell-dup` · `sell-dup-reconnect` · `sell-dup-cross-kind` · (`sell-dup-restart` — `restart_cases.py sell-dup` 이 부르는 관측용 단계, 재기동 경로 판정은 restart_cases.py) | AC-18 (g) | SC-112 |
| `sell-cas-halt` / `sell-cas-reload` (`restart_cases.py sell-cas`·`sell-cas-market` 이 부름 — 시장 행 변조도 같은 `sell-cas-halt` 단계) | 판매 → 지갑/시장 행 SQL 변조 → 판매 → 정지 / 재기동 뒤 판매. **봇 단계는 관측용(`Reference`) — 판정은 `restart_cases.py`**(§3.2) | SC-34 · SC-35 · SC-36 (판정: restart_cases.py) |
| `sell-recover-restart` (`restart_cases.py sell-recover`) | 바닥까지 판매 → 배수 tick 1 → 재기동 → 첫 배수 tick 관측. 봇 단계는 관측용 | SC-51 (판정: restart_cases.py) |
| `sell-backlog` | 판매 중 qa 가 postgres 정지·재개(사람 단계 `db_stop`) | SC-39 |
| `trace-sell-a` / `trace-sell-b` | A 14,000 kg 판매 → 종료·디스폰 / (별도 프로세스) B 첫 접속 | SC-113 · SC-114 · SC-115 |
| `race-sell-same-tick` | 두 봇 같은 목표 tick 판매 | SC-116 · SC-132 |
| `bots run --stage sell-load` | 31 연결 20 분, 정책: 광맥 흩어 채굴 → 인벤토리 > 2,000 kg 이면 거래소 전량 판매 | SC-117 · SC-119 · SC-132 (월드는 SC-118·120 입력) |
| `sell-once` | 지정 월드에서 Ferrosite 1 회 채굴·판매(사람 세션 보조) | 없음 — 관측용(`Reference`) |

- **같은 tick 겨냥**: 스냅샷의 서버 tick 을 보고 목표 tick 에 맞춰 보낸다. 같은 tick 이 안 되면 **무효**로 세고 재시도(새 월드). 성공 확률을 계약이 가정하지 않는다.
- **거래소 항법**: 도킹 판정은 서버 `f64` 상태(tick 시작 시점)다. 봇은 스냅샷으로 도킹 구역(320 m) 안 속도 ≤ 10 m/s 를 확인한 뒤 보낸다. 봇의 거리 계산은 판정이 아니다(판정은 서버 응답).

### 3.2 도구 표 (출처 게이트가 읽는다 — `| 스크립트 | 항목 |`)

| 스크립트 | 항목 |
|---------|------|
| `new_world.py` | 없음 — 준비 도구 |
| `server_boot.py` | SC-04 · SC-05 |
| `trade_ledger.py` (신규) | SC-30 · SC-31 · SC-32 · SC-33 · SC-37 · SC-39 · SC-118 · SC-120 · SC-123 |
| `restart_cases.py` | SC-34 · SC-35 · SC-36 · SC-51 · SC-112 |
| `data_copy_scope.py` (신규) | SC-57 |
| `validate_data_files.py` | SC-58 · SC-59 |
| `data_file_pairs.py` | SC-129 |
| `constraint_census.py` | SC-126 |
| `check_sequence_gaps.py` | SC-124 |
| `stats_delta.py` | SC-117 |
| `interface_matrix.py` | SC-128 |
| `db_test_census.py` | 없음 — §3.4 이름 대조(이 계약의 판정 항목이 아니라 CI 게이트) |
| `unity_session.py` | 없음 — 사람 세션 준비 |

### 3.3 CI 게이트 조정 (Q2)

- `check_item_sources.py` 의 `KNOWN_SLICES` 에 `p1-03` 추가. `gates.yml` 의 `judgment` job 에 `출처 게이트 (계약 p1-03)`(`--slice p1-03`, 분모 133 — SC-130 의 대상)를 더하고, p1-02·p1-01 단계는 **회귀**로 둔다. job 이름(`server (rust)`·`bots (rust)`·`judgment gates (python)`)은 바꾸지 않는다(필수 체크 이름).
- `gates.yml` 의 p1-02 단계 `data/ 스키마 (p1-02 SC-06)` 는 `validate_data_files.py` 가 레지스트리 유도로 바뀌면서 12 파일을 보게 된다 — 이름을 `(p1-03 SC-59)` 로 바꾸고 p1-02 회귀 의미는 같은 단계가 계속 덮는다.
- `DB 테스트 이름 대조` 단계의 `--contract` 를 이 계약으로(§3.4). p1-02 목록은 회귀로 한 단계 더.
- **PR 템플릿**에 두 줄: `[ ] p1-03 병합 조건: unity test EditMode·PlayMode test-results.xml 을 _workspace/p1-03-trading/evidence/ 에 두고 스위트별 수를 적었다(SC-131)` · `[ ] 실서버 실행이면 새 월드 id 와 INSERT 문을 증거에 남겼다(계약 §0.2)`.
- **CI 가 보지 않는 것** (리포트 요약에 싣는다): Unity EditMode·PlayMode(SC-56·61~105) · 실서버·봇(SC-04·05 qa 분·30~39·45~47 봇 분·51·55 qa 분·106~126·132) · 사람(SC-65·68·…·101 ③ 행, SC-131·133).

### 3.4 DB 통합 테스트 — CI 필수 실행 (p1-02 리더 결정 Q-2 승계)

- 헬퍼 `starfall-testdb`(`TestDb::create(<이름>)`), `STARFALL_DB_TESTS=required`, stderr `STARFALL_DB_TEST RAN <이름>`. **`<이름>` 은 §1 의 `[DB]` 이름과 글자 그대로**(R3 ⑤).
- 대조 목록은 `db_test_census.py` 가 §1 의 ` [DB]` 표식에서 뽑는다 — 이 계약 **16 개** — qa3 실측(`db_test_census.py --contract <이 문서>`, 2026-10-05): serverdb 11(`sale_dup_across_restart_is_rejected`·`sale_dup_cross_actor_no_halt`·`sale_dup_reaching_db_halts`·`wallet_cas_mismatch_halts`·`market_cas_mismatch_halts`·`restart_loads_tampered_wallet_and_sells_on_top`·`full_sale_keeps_zero_inventory_row`·`recovery_tick_writes_no_market_row_or_event`·`restart_preserves_saturation_and_last_sale`·`load_economic_state_matches_wallets_and_market_states_in_the_database`·`load_economic_state_on_a_world_with_no_sales_returns_empty_wallets_and_markets`) / history 5(`h30_…`·`h31_…`·`h32_…`·`h33_…` + 회귀 짝 `history_fail_stop`). 도구의 verdict 라벨이 `p1-02 SC-113` 으로 고정돼 있다 — Q2 에서 슬라이스 인자로 바꾼다(이 계약에는 대응 SC 가 없으므로 라벨은 관측용).
- 0004 가 testdb 마이그레이션에 들어가야 H1 이 판매 행을 커밋할 수 있다(01_architect_tasks 임계 경로).

---

## 4. 경계면 비교표의 고정 대상 (SC-128 의 채점 기준 + Phase 4 점진 검증)

양쪽을 **함께** 읽는다. 한쪽 존재 확인은 검증이 아니다. 모듈 완료 알림마다 해당 행을 그때 본다.

| 경계면 | 생산자 | 소비자 | 비교할 것 | 볼 시점 |
|-------|-------|-------|---------|--------|
| 계약 ↔ Rust ↔ C# | 신규 스키마 5 + data 2 + 변경 2 | `server/crates/contracts` · `client/.../Generated` | 필드명·필수·nullable·정수 폭(`quantity_kg`·`saturation_kg` → `i32`/`int`, `*_cr*` → `i64`/`long`)·`deny_unknown_fields`, **`MARKET_STATE.last_sale` 필수 nullable** | S1·C1 완료 |
| 명령 | 클라이언트·봇의 `SELL_MINERAL` 생성부 | 서버 역직렬화·판정 | payload 키 = `{market_id, mineral_id, quantity_kg}` 뿐, 서버가 클라이언트 값을 믿는 필드 0(I-74) | S3·C3·Q3 |
| 즉시 응답 ↔ 최종 결과 | `COMMAND_RESULT{ACCEPTED}` → `SALE_RECEIPT` → `INVENTORY_STATE` → `WALLET_STATE` | UI-04·UI-09·인벤토리 패널 | 접수를 완료로 취급하지 않음, 잔액 출처는 `WALLET_STATE` 하나(SC-72·102) | S3·C3 |
| 세션 시작 5 메시지 | sim 세션 시작 | C3·봇·history(ADR-0014 §6 합 37 < 64) | 순서·개수(SC-47) | S3 |
| 상장 = 드러난 광물만 | sim `MARKET_STATE` | C2(UI-06)·Q2 누출 검사 | 판단 입력이 sim 상태(`first_extracted_tick`)이고 역사 표가 아님(R-9) | S3 |
| 가격 규칙 | S3b `price.rs` | C2 미리보기(UI-08)·Q2 `trade_ledger.py` 재계산 | **같은 입력 → 같은 값** — 세 구현을 §0.6 표 하나로 각각 단언(서로를 기대값으로 쓰지 않는다) | S3b·C2·Q2 |
| sim 배치 → 영속화 | S3 의 배치(지갑·시장 `{expected, new}`, 인벤토리 감소) | S4 비교 후 쓰기 | T0 `02_interface.md` 의 기대값 표현(행 없음 포함) | T0·S4 |
| 기동 적재 → sim | S4 적재 함수 | S3 초기값 주입·S2 기동 순서 | 지갑·시장·마지막 거래·장부, 적재 순서(ADR-0013 §7) | S4·S2 |
| 도메인 → 역사 | `MINERAL_SOLD` | 러너 `Unjudged` 통과 | 레지스트리 `consumers` 에 `history` 없음, 러너가 payload 를 안 읽음(H-32) | H1 |
| DB ↔ 코드 | `0004_*.sql` | sqlx 구조체 | 컬럼·nullable·CHECK 이름이 ADR-0016 §5 표와 정확히 같음(SC-126) | S4 |
| 데이터 → 클라이언트 | `data/market/*.json`·`deposits` v2 | `GreyboxDataLoader`·Trading | 읽는 키 ⊆ 공개 필드, 예비 파일을 읽지 않음(사본에 없음) | C1 |

---

## 5. 신규 경로 "실제로 탔는가" 표 (리포트마다 채운다)

**통과한 테스트 수는 경로가 실행됐다는 증거가 아니다.** 다음을 **실행됨 / 미실행**과 근거(카운터 델타·이벤트 행·테스트 출력 건수)로 적는다. 미실행이 있으면 그 경로를 판정하는 SC 는 PASS 가 될 수 없다.

판매 수락 · `TARGET_UNKNOWN`(시장 / 광물 각각) · `COOLDOWN_ACTIVE`(판매) · `TARGET_OUT_OF_RANGE`(도킹) · `SHIP_TOO_FAST`(도킹) · `INSUFFICIENT_QUANTITY` · `CAPACITY_EXCEEDED`(지갑) · `RECORDING_BACKLOG`(판매) · `DUPLICATE_COMMAND_ID`(세션 / 재접속 / 재기동 / 종류 교차 / 다른 actor 각각) · 구간 걸침 · 바닥 도달 · 포화 상한 고정 · 회복 배수 tick(바뀜 / 안 바뀜) · 상장 브로드캐스트(첫 채굴) · 세션 시작 5 메시지 · 지갑 비교 후 쓰기 정지 · 시장 비교 후 쓰기 정지 · PK 위반 정지(판매) · 지갑·시장 적재 · 인벤토리 0 행 유지 · 러너 `Unjudged` 통과 · 러너 v2 `MINERAL_SOLD` 무정지 · `history-replay` 판매 포함 · 클라이언트 `발견: 기록 중…` 상태 · BACKFILL 만으로 표지

---

## 6. 기록 항목 (판정하지 않음)

| ID | 무엇 | 출처 |
|----|------|------|
| M-1 | 부하(SC-117) tick 본문 p50/p99/max, 영속화 커밋 p50/p99 — **p1-02 기준선과 나란히**(판매가 배치 행을 늘리는 것은 예상) | 스펙 AC-20(b) |
| M-2 | 역사 알림 지연 p50/p95(SC-121 의 표본) — p95 ≤ 1 초 목표 대비 | H-34, ADR-0014 §7 |
| M-3 | AC-19(d) 봇 정책 기준선: B 의 첫 판매 광물(사람 세션 비교용) | 스펙 AC-19(d) |
| M-4 | 디자인 §9.1 지표: 흔적 반응률 · 첫 판매까지 시간 · 광물 분산(Ferrosite 비중) · 판매 크기·"이 값까지" 프리셋 비율(클라이언트 로그) · 광물별 바닥 횟수 — 봇 값은 참고치 | 디자인 §9.1 |
| M-5 | 판매 거절 비율(사유별 / 전체, 분모) — 첫 5 분 이후 `TARGET_OUT_OF_RANGE + SHIP_TOO_FAST` < 30 % 목표 | 디자인 §9.2 |
| M-6 | 반례 층별 표(§0.5)의 C# "감지 불가" 건수 | 스펙 §5.3 |
| M-7 | `MARKET_STATE` 바이트 × 송신 수(판매 tick + 회복 tick) / 세션 | 스펙 §8 |
| M-8 | 사람 세션: 거래소 찾기 시간(목표 ≤ 60 s), T-3 B 질문 답, T-12 상장 알림 ↔ 발견 배너 순서 답 | 디자인 §6 T-12, §9.1 |
| M-9 | `processed_commands` 적재 소요(기동 로그) — 100 만 행 만기 추적(판매가 증가 속도를 늘림) | 01_architect_tasks 만기 표 |
| M-10 | 사람 세션의 `data/` 수치 소감(도킹 200 m·10 m/s·쿨다운 1 s·바닥 30 %·회복 60 s — 전부 `가정:`) | 디자인 부록 A |

---

## 7. "미검증(환경)" 처리 기준

| 코드 | 조건 | 영향 |
|------|------|------|
| E1 | cargo 툴체인 사용 불가 | A·C·D·E·H·I·L, F·K 의 DB 테스트 |
| E2 | Docker/PostgreSQL 미가동, 포트 점유 | DB·실서버 항목 |
| E3 | Unity Editor 라이선스·CLI 실패, Editor 가 열려 있음 | M(SC-56)·N·O·P·Q, SC-131·133 |
| E4 | .NET SDK 미가용 | SC-60 |
| E5 | 서버가 **환경 문제로** 기동 못 함(`netstat` 첨부) | 실서버 항목 |
| E6 | 봇 하네스 빌드 불가 | 봇 항목 |
| E7 | 31 연결 + Unity + Docker 동시 실행 자원 부족 | SC-117~120 |
| E9 | 선행 태스크 미완 | **대기** — SC-43(§0.5 표)·127·128 |
| H | 사람 필요(Play 버튼·DB 정지 승인) | ③ 행 13 · SC-39 · SC-131 · SC-133 — 사람이 없으면 **미검증(사람 대기)**, PASS 아님 |

**환경 문제와 구현 부재를 섞지 않는다. 구현이 없으면 FAIL 이다.**

---

## 8. 판정·라운드 규칙

### 8.1 라운드

- 라운드 **최대 3 회**. 결과는 `04_qa_report_r{N}.md`. **라운드마다 새 qa** 가 `RESUME.md` 와 이전 리포트로 이어받는다(R7).
- 실행은 `python tests/e2e/run_round.py --slice p1-03 --contract _workspace/p1-03-trading/02_sprint_contract.md --round r{N}` 한 번(R5). 설정 `tests/e2e/round_configs/p1-03-trading.json`(Q2). 사람 단계(`db_stop`) 앞에서만 멈춘다. 리포트는 `evidence/r{N}_<날짜>/summary.json` 을 보고 쓴다.
- **r2 부터의 판정 대상**: FAIL·미검증 항목 + 고친 파일에 걸린 항목(§1 방법 칸의 테스트 이름 → 정의 파일, `cargo_sc_map.py scan` 의 `defined_in` 으로 뽑는다). 나머지는 실행기의 자동 회귀가 덮는다.
- FAIL 은 **파일:라인 + 재현 명령 + 기대/실제**를 담아 담당자에게. qa 는 구현 코드를 고치지 않는다(`tools/bots/`·`tests/e2e/`·`.github/` 만 qa 소유).
- §0.5 표와 결과가 다르면 FAIL 이 아니라 **architect 통지**.
- 리포트는 정확성 판정 / 성능 / 기록(M) 을 별도 절로, 요약에 **§0.9**(증명하지 못하는 것)와 **§5**(실제로 탔는가)를 싣는다.

### 8.2 p1-01 §7b 규칙 1~9 — **그대로 승계** (원문과 근거 사례: p1-01 계약 §7b, 요약: p1-02 계약 §8.2)

1. 새 항목의 각 절 옆에 "이 절을 자명하게 통과시키는 상태" 를 적는다 — **§1 의 `⊘` 칸이 이 규칙의 이행이다.** 자명 통과와 자명 불통과 둘 다 본다.
2. 판정 기준은 **관측 전에** 적는다.
3. 기준을 만드는 사람도 자기 기준에 1 번을 돌린다 — 도구와 문서에도.
4. 항목이 읽는 산출물의 각 필드가 어느 계산 층에서 나왔는지와 임계를 한 줄로.
5. **SC 하나 = verdict 하나.** (그래서 AC-6(a) 를 넷, AC-20(b) 를 넷, AC-15 를 39 로 갈랐다.)
6. 방어를 넣었으면 그 방어가 걸려야 할 입력에서 **실제로 걸리는지** 같은 실행에서 보인다(양성·음성 대조).
7. 어떤 SC 번호로 verdict 를 내는 도구는 **이 계약이 지명한 도구**여야 한다(§3.2 + §1 의 백틱 케이스 이름). 관측 표식은 위반이 아니다.
8. 미루는 것에는 **만기**(게이트)를 적는다 — §9.
9. **소스는 부정을 증명할 수 있지만 긍정은 증명하지 못한다.** 이 계약에서 소스 검사로 판정하는 항목(SC-02·03·103)은 실행 짝을 지정했다.

**형태 셋**: (가) 조건 미발생형 → 조건 발생의 별도 관측 · (나) 검출기 사망형 → 양성 대조 · (다) 전제 오류형 → 판정 입력의 층·임계 검증. **이 계약에서 (다) 의 후보로 이미 본 것**: §0.10 `MINERAL_SOLD` 필드 이름(디자인 작업 이름으로 쓰면 NULL), §4.8 의 도달 불가 조건 셋(SC-05, §11 Q-1).

---

## 9. 미뤄둠 (만기) — 규칙 8

| 미뤄둠 | 만기 | 만기가 지나면 |
|---|---|---|
| §0.5 ③ 열(C#) 실측 — ② 열·의도한 위반 열은 2026-10-06 채움(42/42 일치) | **C1 완료 알림 시점** | SC-61 의 C# 층 상수가 근거 없는 수가 된다 — SC-61 FAIL(미기재) |
| §3.4 `[DB]` 이름 16 의 CI 반영(`gates.yml` 의 `--contract` 를 이 계약으로) | **Q2 완료** | CI 이름 대조가 p1-02 목록으로만 돈다 — 리포트 요약에 비-통과 |
| 0004 동결 선언(`_workspace/p1-03-trading/migration_freeze.md`, serverdb3) | **증거 DB 로 서버를 띄우는 첫 실행 전**(Q3 봇 첫 실서버 실행) | `server_boot.py` exit 4 — 실서버 항목 전부 실행 불가. 선언 없이 우회하면 그 실행 무효 |
| p1-02 만기 이월: 데이터 배포 ADR — **비-통과로 기록됨**(01_architect_tasks 만기 표). 닫힘 = SC-57 PASS | **이 슬라이스 r1** | SC-57 FAIL = 만기 미해소로 r1 요약에 비-통과 |
| `HISTORICAL_EVENT_NOTICE` 판별 union | 두 번째 역사 타입 — 이번 발동 없음 | 그대로 |
| 지갑 하한 0 의 ②(잔액 부족 거절) | 크레딧 차감 경로가 생기는 슬라이스 | 그 슬라이스 계약에 ADR-0016 §5 `wallets_balance_cr_check` ① "예" 재평가 항목 |

---

## 10. 세부 절차 매핑

### 10.1 designer T-1~T-12 → 항목

| T | 항목 |
|---|------|
| T-1 첫 돈 | SC-06 (단위) · SC-30·132 (`trade-cycle` 첫 판매) · 사람 SC-74·89 |
| T-2 내 판매가 값을 움직인다 | SC-84 (프리셋 양·예상 대금) · SC-14 P9·P10 · SC-33 (실서버 재계산 — 판매 직전 포화 기준) |
| T-3 떠난 사람의 흔적 | SC-113 · SC-114 · SC-115 (+ M-3, 사람 M-8) |
| T-4 같은 tick 경쟁 | SC-116 (+ 단위 SC-11) |
| T-5 서버가 판정한다 | SC-106~111 (+ 단위 SC-07) |
| T-6 같은 판매 두 번 | SC-112 (+ 단위 SC-22~29) |
| T-7 쪼개 팔아도 같다 | SC-15 |
| T-8 회복과 재기동 | SC-18 · SC-51 |
| T-9 보존 법칙 | SC-30~33 · SC-118 |
| T-10 미발견 광물 비누출 | SC-45 · SC-46 |
| T-11 30 명 | SC-117~120 |
| T-12 사람 세션 | SC-133 (사전) · SC-65~101 ③ 행 (최종) · M-8 |

### 10.2 history H-30~H-34 → 항목

| H | 항목 |
|---|------|
| H-30 | SC-52 |
| H-31 | SC-53 |
| H-32 | SC-54 (짝 `history_fail_stop`) |
| H-33 | SC-55 |
| H-34 | SC-121 (+ M-2) |

### 10.3 AC → 항목 (누락 대조용)

AC-1 → SC-01~03 · AC-2 → 04·05 · AC-3(a)~(h) → 06~13 · AC-4(a)~(h) → 14~21 · AC-5(a)(b)(c)(d)(d2)(e)(f)(g) → 22~29 · AC-6(a) → 30~33, (b) → 34·35, (c) → 36, (d) → 37 · AC-7 → 38·39 · AC-8(a)(b) → 40·41 · AC-9 → 42~44 · AC-10(a)(b)(c) → 45~47 · AC-11(a)~(d) → 48~51 · AC-12(a)~(d) → 52~55 · AC-13(a)~(d) → 56~59 · AC-14(a)~(c) → 60~62 · AC-15 UI-01~13 ①②③ → 63~101 · AC-16(a)(b) → 102·103 · AC-17 → 104·105 · AC-18(a)~(g) → 106~112 · AC-19(a)(b)(c) → 113~115, (d) → M-3 · AC-20(a) → 116, (b) → 117~121 · AC-21(a)~(e) → 122~126 · AC-22(a)~(e) → 127~131 · AC-23 → 133 · 스펙 §5.1a → 132.

---

## 11. 구현자 확인란과 쟁점

| 영역 | 항목 | 담당 | 확인 | 서명/날짜 |
|------|------|------|------|----------|
| A 게이트·순수성 | SC-01~03 | server3 | ☑ | server3 2026-10-06 — A~K 전 행 "이 방법으로 증명할 수 있다", 이름 그대로 사용 |
| B 데이터·기동 거부 | SC-04~05 (qa 분 포함) | server3 | ☑ | server3 2026-10-06 — Q-7 stats 필드 5 개 이름 확정(SC-04 칸 반영) |
| C 판매 판정 | SC-06~13 | server3 | ☑ | server3 2026-10-06 — Q-2: 사유 2~5 sim 판정 + 실서버 stats_delta, 6·7 gateway 플럼빙 단위 |
| D 가격 규칙 | SC-14~21 | server3 | ☑ | server3 2026-10-06 |
| E 복사 경로 | SC-22·23·25·26(sim)·27·29 | server3 | ☑ | server3 2026-10-06 |
| E 복사 경로 DB | SC-24·26(DB)·28 | serverdb3 | ☑ | serverdb3 2026-10-05 — `[DB]` 11 개 수락 |
| F 비교 후 쓰기 DB | SC-34~37 의 `[DB]` | serverdb3 | ☑ | serverdb3 2026-10-05 — Q-6 순서(판매1 → SQL 변조 → 판매2 → 정지) 동의. 0004 동결 선언은 S4 `[DB]` 초록 뒤, 첫 실서버 봇 실행 전 |
| G 기록 지연 | SC-38 | server3 | ☑ | server3 2026-10-06 |
| H 결정성 | SC-40·41 | server3 | ☑ | server3 2026-10-06 |
| I 계약 ↔ Rust | SC-42~44 | server3 | ☑ | server3 2026-10-06 — S1 완료, R3 표를 qa3 가 실행으로 확인(필터 8 개 각 1 건, §0.5 ② 42/42) |
| J 상장 (sim) | SC-45~47 의 sim 테스트 | server3 | ☑ | server3 2026-10-06 |
| K 시장 상태 | SC-48·49 (server3) · SC-50·51 `[DB]` (serverdb3) | server3 · serverdb3 | ☑ | server3 2026-10-06 · serverdb3 2026-10-05 — SC-51 적재 함수 두 이름은 `trading_load_state.rs` 에 이미 같은 글자로 있고 통과 |
| L 러너 통과 | SC-52~55 | history (H1 투입 시) — 그 전엔 architect3 가 이름만 확인 | 이름 ☑ · history ☐ | architect3 이름 확인(Q-5, `h30_`~`h33_` 승인 2026-10-06). H1 투입 시 history 확인 |
| M 사본 범위 | SC-56 | client4 | ☑ | client4 2026-10-05 — Q-4 ②: 이름 `ClientDataCopy_MatchesRegistryDerivedSet` 동의 |
| N 생성기·EditMode | SC-60~62 | client4 | ☑ | client4 2026-10-05 |
| O 화면 ①② | SC-63~100 의 ①② 26 행 | client4 | ☑ | client4 2026-10-05 — Q-4 ①: FakeRealtimeTransport 주입 |
| P·Q 계산 안 함·키 | SC-102~105 | client4 | ☑ | client4 2026-10-05 — Q-4 ③: SC-105 기존 바인딩은 소스 정규식으로 유도(12 하드코딩 안 함) |
| **판정 (스펙 대응)** | AC-1~AC-23 전 절 → SC 대응(§10.3) — 누락 0 인지 | architect3 | ☑ | architect3 2026-10-05 — 스펙 절 단위 전수 대조 누락 0(AC-19(d) → M-3 기록 항목은 스펙상 "판정 아님" 과 일치). Q-1 판정: m08·m09 유지(스키마층 기동 실패 경로), **m13 삭제** — 스펙 §4.8 에서 세 조건을 "계약이 보장하는 것" 으로 옮김. Q-5 이름 `h30_`~`h33_` 승인 |

### 쟁점 (구현자·architect 답을 받아 반영한다)

**전부 답을 받았다(2026-10-05~06)** — Q-1 architect3: m08·m09 유지, m13 삭제(세 조건은 스펙 §4.8 "계약이 보장하는 것") · Q-2 server3: qa 안 그대로(사유 2~5 sim + 실서버 stats_delta, 6·7 gateway 플럼빙 단위) · Q-3 server3·serverdb3: 이름 그대로 · Q-4 client4: §11 M·O·P·Q 행 · Q-5 architect3: `h30_`~`h33_` 승인 · Q-6 serverdb3: 순서 동의 · Q-7 server3: SC-04 칸 반영. 아래는 원문.

- **Q-1 (architect3) — §4.8 의 도달 불가 조건 셋.** (1)(2) "`sell_cooldown_s × tick_hz`·`recovery_interval_s × tick_hz` 가 정수" 는 스키마가 초를 **정수**로 강제하고 tick_hz = 20 이라 **기동 경로로 만들 수 없다**. 같은 의도의 도달 가능한 반례(소수 초 → 스키마층 기동 실패)로 SC-05 m08·m09 를 두었다 — p1-02 SC-05 ⑤⑥ 리더 방침과 같은 처리. (3) "`(2^31−1) × max(base_price) ≤ 2^53−1`" 은 스키마 `base_price_cr_per_kg ≤ 1,000,000` 에서 좌변 최대 ≈ 2.15 × 10^15 < 2^53−1 ≈ 9.0 × 10^15 라 **역시 도달 불가**다. 검산 함수 단위 테스트(스키마를 거치지 않는 표 — `data::tests::market_check_m13_proceeds_overflow_bound`)로만 실행을 보이는 안을 넣었다. 둘 다 이대로 좋은지, 아니면 스키마 상한을 그 검산의 근거로 삼아 검산 자체를 빼는지 판정 부탁.
- **Q-2 (server3) — AC-3(b) "`commands_rejected_total{사유}` 각각 +1" 의 관측 위치.** 카운터는 게이트웨이 소유라 수동 step sim 테스트에서는 안 보인다. 안: 사유 6·7 은 게이트웨이 테스트 `sell_mineral_rejection_counter_*`(소켓 경유), 사유 2~5 는 실서버 SC-107~111 의 `stats_delta.py` 델타(= 봇 관찰 수). 게이트웨이 테스트로 2~7 전부를 덮을 수 있다면(함선 위치·속도를 테스트 서버에서 만들 수 있으면) 그쪽이 낫다 — 가능 여부를 알려 달라.
- **Q-3 (server3·serverdb3) — §1 테스트 이름.** 방법 칸의 필터·이름은 qa3 가 정한 안이다. 다른 이름이 자연스러우면 **지금** 바꿔 달라(계약을 고친다). 기준: ① 필터가 그 SC 의 테스트만 잡는다(부분 문자열 — 다른 SC 이름의 일부가 되지 않게) ② `[DB]` 이름 = `TestDb::create` 인자 ③ 통합 테스트 파일 위치는 01_architect_tasks 의 경로.
- **Q-4 (client4) — UI 테스트 이름과 PlayMode 주입.** `TradingUi{nn}_*`(EditMode)·`TradingPlay_Ui{nn}_*`(PlayMode)는 접두 일치다. PlayMode 존재 단언은 p1-02 `GreyboxMiningSessionPlayModeTests` 처럼 FakeRealtimeTransport 로 메시지를 주입하는 방식을 가정했다(SC-64 의 "표식 1" 은 세션 시작 뒤 씬 탐색). SC-56 의 `ClientDataCopy_MatchesRegistryDerivedSet` 은 p1-02 `ClientDataCopy_MatchesRepositoryOriginal` 을 개정하면서 이름을 바꾸는 안이다 — 이름을 유지하고 싶으면 알려 달라. SC-105 의 기존 바인딩 수 12 를 소스에서 수집하는 방법이 있는지.
- **Q-5 (history, H1 투입 시) — H-30~H-33 이름** `h30_`~`h33_` 접두. 투입 전까지 architect3 가 이름만 확인한다.
- **Q-6 (serverdb3) — SC-34·35 의 실서버 절차.** p1-02 `restart_cases.py cas` 와 같은 모양(서버 기동 → 판매 1 → SQL 변조 → 판매 → 정지 관측)을 지갑·시장 행에 쓴다. 변조 SQL 은 qa 가 쓰는 유일한 쓰기다(**새 월드 행에만**). 비교 후 쓰기의 기대값이 "행 없음" 인 첫 판매 직후에 변조하면 다른 경로를 탄다 — **변조는 둘째 판매 전**에 한다. 이 순서가 S4 설계와 맞는지.
- **Q-7 (server3) — SC-04 의 `/debug/stats` 필드 이름**(시장 id·`pricing_version`·상장 수·예비 수). T0 `02_interface.md` 에 적어 주면 `server_boot.py p1-03-boot` 가 그 이름으로 읽는다.

---

## 12. 계약 변경 이력

| 날짜 | 변경 | 근거 |
|------|------|------|
| 2026-10-07 | SC-40 방법 칸 필터를 실제 이름 `sc40_two_process_replay_with_trading_is_byte_identical` 로(옛 필터 `trading_replay_` 는 `#[ignore]` 자식만 잡아 실행 0 — 계약 필터 실행에서 발견). 판정 기준·⊘ 불변, 코드 불변 | qa3 필터 실행(phase4_s5) |
| 2026-10-06 | SC-13 방법 칸 예시 이름을 실제 이름 `sale_cooldown_is_independent_of_mining_cooldown` 으로(필터 `sale_cooldown_` 불변 — 계약 필터 실행에서 이미 잡힘). 판정 기준·⊘ 불변 | qa3 필터 실행(phase4_s3s4) |
| 2026-10-06 | §0.5 ③ 열 C# 실측 51 / 35 / 27 기록(예측 53 / 33 / 27 과 2 건 차 — `long` 필드가 소수 수락) — 예측 열은 바꾸지 않음, architect 통지 사안 | client4 C1 실측 |
| 2026-10-06 | SC-60 ⊘ 에 "생성만 하고 컴파일하지 않음" 배제 추가(EditMode 결과 짝), 불가 칸에 E3 — 스펙 §5.2 정정(생성기 실측 기준 = Unity 컴파일 성공, architect3), 최종: 스키마 title `MarketLastSale`(architect3 재반영) + 생성기는 충돌 감지 검사만(리더 최종 결정) — SC-60 기준 "생성 2회 동일 + Unity 컴파일 + EditMode". 판정 기준 불변 | architect3 요청 |
| 2026-10-06 | **리더 판정 합의 2026-10-06** — §11 확인란 채움(server3·serverdb3·client4·architect3 회신, history 행은 이름만 architect3 확인). **SC-05 m13 삭제**(architect3 Q-1 판정 — 스펙 §4.8 에서 정수 초 곱·대금 상한 검산을 "계약이 보장하는 것" 으로 옮김. 경우 수 16 불변 — m13 은 원래 16 밖). SC-04 stats 필드 이름(server3 Q-7). ADR-0016 accepted. 판정 기준·⊘ 불변 | 리더 판정, 구현자·architect 회신 |
| 2026-10-06 | Phase 4 Q2·Q3 반영(기준·⊘ 불변): §0.2 동결 선언 사실(0004 디스크에 있음·선언 0), §0.5 ② 열 42 행 채움(S1 실측 — 42/42 일치), §3.4 `[DB]` 16 개 실측, SC-04 jq 경로(`.deposits`), SC-32 음성 대조를 `trade_ledger.py --without-sales-term` 으로(p1-02 도구는 v2 광맥 표를 못 읽음), SC-39 판정 명령을 `trade_ledger.py accepted`, SC-47 에 `leak-scan-market` 지명, 사람 ③ 행·SC-03 에 `도구 없음(사람 관찰)` 표기(출처 게이트), SC-115·132 봇 명령 형식, §3.1 재기동 단계는 관측용·판정은 restart_cases.py | qa3, 출처 게이트 실측 |
| 2026-10-05 | 초안 r0 — AC-1~23 을 SC-01~133 으로(화면 39 = UI 13 × ①②③), T-1~T-12·H-30~H-34 매핑, R3 증거 형식(§0.3a), 수치 ⊘ 도달 가능성 계산(SC-05·07·30~33·49·113~116·120), §4.8 도달 불가 3 조건 발견(Q-1), 기준선 실측(커버리지 errors 17, contracts RED 8, golden sha256 9) | 스펙 agreed, 리더 지시 2026-10-05 |

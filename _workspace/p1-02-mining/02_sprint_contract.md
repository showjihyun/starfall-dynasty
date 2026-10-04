# p1-02-mining 스프린트 계약

- 작성: qa, 2026-09-27 (구현 착수 전 / Phase 3) — **r2** (server·history·client 항목별 답, 리더 Q-2 결정, AC-11(h)(iii) 반영)
- 스펙: `docs/specs/p1-02-mining.md` (**agreed**, AC-1~AC-19, 불변식 I-48~I-72. 사용자 결정 Q1~Q4 추천안 확정, Q5 회복 포함·Q6 광물 4종)
- ADR: `0013`(경제 상태 영속화·지속 멱등 — accepted, server 검토는 개정으로 받음) · `0014`(역사 엔진 골격 — accepted). 개정 `0006` §4 · `0007` §1
- 세부 절차 정본: `_workspace/p1-02-mining/01_history_review.md` §6(H-01~H-15)·§6.1(오라클) · `docs/design/p1-02-mining-design.md` §5(S-1~S-10)·§7(지표)
- 태스크: `01_architect_tasks.md` (T0 · S1~S5 · H1~H2 · C1~C3 · Q1~Q4)
- 계약 데이터 (architect 실측, 스펙 §5.2): **스키마 29 / 유효 fixture 46 / 반례 74 / 레지스트리 타입 23(`registry_version: 4`)** — r1: `reason_code` += `CAPACITY_EXCEEDED`(유효 +1), `HISTORICAL_EVENT_NOTICE` producers history → **server**(커버리지 기준선 errors 25 그대로 — architect 재실측)
- 합의: **server ☑ · history ☑ · client ☑ · qa ☑** (2026-09-27, r2) · **architect(판정) ☑** (2026-09-27, r2 — Q-1·Q-3·Q-8 판정, 스펙 AC-1~AC-19 전 절의 SC 대응을 기계 대조로 확인, 누락 0) — §11 확인란
- 항목 수: **SC-01 ~ SC-114 = 114** (첫 담당 기준 server 37 / history 22 / client 9 / qa 46 — 공동 행: SC-05·25·26·63 server·qa, SC-59 history·qa, SC-114 qa·server. `CONTRACT_ROW` 로 센 114 와 일치) + 기록 항목 **M-1~M-12**(판정 제외). **r1 신설 SC-100~110 은 번호를 끝에 붙였다** — r0 번호로 이미 오간 메시지가 있어 기존 번호를 바꾸지 않는다. 표 안에서는 관련 절에 끼워 두었다

**이 문서의 구속력.** Phase 5 평가(`04_qa_report_r{N}.md`)는 이 표의 항목으로만 한다. 스펙 §7 의 AC 와 그 AC 가 세부 절차로 지정한 H-01~H-15·S-1~S-10 을 실행 가능한 관찰로 옮긴 것이고, **그 셋에 없는 요구는 넣지 않았다.** 스펙 불변식 중 AC 가 없는 것은 §11 쟁점으로 architect 에게 묻고, 답이 오기 전에는 항목으로 만들지 않는다. 평가 중 발견한 그 밖의 문제는 리포트의 "계약 외 발견"에 적고 판정에 쓰지 않는다.

**이 계약이 p1-01 에서 그대로 가져온 것**: §7b 규칙 1~9(본문 §8.2 에 요약, 원문은 `_workspace/p1-01-ship-movement/02_sprint_contract.md` §7b), §0.3 증거 기준의 판정 칸 다섯, "미검증(환경)" 처리 원칙, `docker compose down -v` 금지와 하드 킬 금지(CLAUDE.md). **가져오지 않은 것**: p1-01 의 SC 번호. 이 계약의 SC-12 는 p1-01 의 SC-12 와 아무 관계가 없다 — 그래서 §3.3 이 도구 라벨에 슬라이스 표지를 붙인다.

---

## 0. 공통 실행 전제

### 0.1 환경

```bash
export PATH="$HOME/.cargo/bin:$HOME/.dotnet/tools:/c/Users/CHOISOOYEON/AppData/Local/Unity/bin:$PATH"
```

- cargo 명령은 `server/`(봇은 `tools/bots/`)에서, 나머지는 레포 루트에서. `.env` 에 `STARFALL_DEV_AUTH_SECRET` 이 있다.
- SQL: `docker compose exec -T postgres psql -U starfall -d starfall -At -F' | ' -c "..."`. **모든 SQL 은 `world_id` 를 조건에 넣는다** — 기본 월드와 새 월드가 한 테이블에 산다(§0.2).
- 서버는 stdin 에 `shutdown` 한 줄로 끝낸다. **하드 킬이 필요했다면 그 사실 자체가 평가 대상이다.** 예외 하나: SC-25 는 서버가 **스스로** 멈추는 경로다(ADR-0013 §5) — 운영자가 죽이는 것이 아니다.
- `docker compose stop postgres` / `start postgres` 는 SC-29~32 에서만 쓴다. 볼륨을 지우지 않는다. **`down -v`·`system prune`·`volume prune` 금지.**
- Unity 프로젝트는 단일 인스턴스다. Editor 가 열려 있으면 `unity test` 가 돌지 않는다.

### 0.2 새 월드 — **발견을 재는 모든 실행의 첫 단계** (스펙 I-70, §10-9)

기본 월드(`worlds.last_tick` 166만대, `domain_events` 4,805행 — 스펙 §10-10 실측)에서 광물은 **한 번만** 발견된다. 두 번째 실행부터 "첫 채굴 → 발견" 은 관측되지 않거나(환경 때문에 빨간불), 이미 있던 기록을 BACKFILL 로 받아 **이번 실행이 만든 것처럼** 초록이 된다. 그래서 다음 항목은 **실행마다 새 월드 행**에서 한다: **SC-04, SC-23·24, SC-40, SC-51~64, SC-80~92, SC-99** (§1 행의 `불가` 칸에 `W` 로 표시했다).

**절차 (도구: `tests/e2e/new_world.py` — qa 가 Q3 에서 만든다)**

0. **마이그레이션 동결 선행 확인** (ADR-0007 §5, 리더 규칙 2026-09-28). 증거 DB `starfall` 로 서버를 띄우면 서버가 미적용 마이그레이션을 **적용해 버린다** — 적용된 파일은 그 순간 동결이다. 기동 전에 `starfall._sqlx_migrations` 의 version 목록과 `server/migrations/*.sql` 을 대조하고, 미적용 파일이 있으면 **소유자의 동결 선언**이 있을 때만 띄운다. 선언은 **추적 파일의 한 줄**이다 — 소유자가 `_workspace/<slice>/migration_freeze.md` 에 `frozen: 0004 — <소유자>, <YYYY-MM-DD>` 를 적는다(환경 변수 선언은 기동하는 사람이 스스로 만들 수 있어 쓰지 않는다 — architect 제안·리더 채택 2026-09-29). `server_boot.py` 의 모든 실서버 기동(`spawn`)이 이 대조를 먼저 하고 어긋나면 exit 4 로 멈추며, 통과하면 **선언 줄의 경로:줄**을 증거(`migration_freeze`)에 싣는다(selftest 11 경우 — 파일 0 개·적용된 파일 소실·다른 번호 선언·형식 불완전 줄도 중단). 이 단계는 새 월드 여부와 무관하게 **증거 DB 로 서버를 띄우는 모든 실행**에 적용한다. 현재 동결: 0001~0003(2026-09-28 13:15 UTC 적용).
1. UUIDv7 을 만든다(서버가 `STARFALL_WORLD_ID` 에 소문자 하이픈 UUIDv7 만 받는다 — `config.rs:47`).
2. 다음 한 줄을 실행한다. 값은 스펙 I-70 이 고정했다 — 서버가 기동 시 대조한다.
   ```sql
   INSERT INTO worlds (world_id, name, tick_hz, calendar_epoch, calendar_scale, sim_version, last_tick)
   VALUES ('<uuidv7>', 'qa-p1-02-<항목>-<YYYYMMDDHHMM>', 20, '3800-01-01T00:00:00Z', 60, 1, NULL);
   ```
3. `STARFALL_WORLD_ID=<uuidv7>` 로 서버를 띄운다.
4. **증거에 남긴다**: 월드 id, INSERT 문 전문, INSERT 직후 `SELECT count(*) FROM domain_events WHERE world_id = '<id>'` = **0** (새 월드가 정말 비어 있었다는 단언 — 이것이 없으면 "새 월드" 는 이름뿐이다).

- `worlds` 행은 **지우지 않는다**(추가 전용 대상은 아니지만 감사 기록이다). 서버에 "새 월드 만들기" 기능을 넣지 않는다(스펙 §9.2).
- 새 월드의 tick 은 0 부터다. **회복 경계는 `⌊t / I⌋` 의 배수**이므로(스펙 I-69) 실서버 기대값은 **실제 tick 을 식에 넣어** 계산한다(스펙 §10-12). tick 0 기준 손계산을 기본 월드에 쓰지 않는다.
- 발견을 재지 **않는** 항목(치트·부하 일부)은 기본 월드에서 돌려도 되지만, **어느 월드를 썼는지 증거에 적는다.**

### 0.3 증거 기준 (p1-01 §0.3 승계)

| 판정 | 조건 |
|------|------|
| **PASS** | 명령과 출력 요약(종료 코드 포함), 테스트 이름, 또는 파일:라인이 리포트에 있다. **그리고 그 행의 `⊘` 칸이 요구한 배제 단언이 같은 실행의 출력에 있다.** 정적 읽기만으로는 PASS 가 아니다 |
| **FAIL** | 기대 관찰이 나오지 않음. **간헐 실패도 FAIL**(비결정성 이슈로 기록, 재시도로 덮지 않는다). **구현이 없어 실행 못 한 것도 FAIL** |
| **미검증(환경)** | §7 의 E-조건. PASS 로 올리지 않는다 |
| **미검증(증거 요건)** | 관찰은 수행됐고 기대와 어긋나지 않았으나 **`⊘` 배제 단언이나 분모가 출력에 없다.** **비-통과다** — 슬라이스 종료를 FAIL 과 똑같이 막는다. 실패를 이 칸으로 옮기는 것은 금지(테스트가 실패한 것은 FAIL) |
| **무효** | 겨냥한 조건이 이번 실행에서 **만들어지지 않았다**(예: SC-84 의 두 채굴이 같은 tick 이 아니었다). 판정이 아니다 — 재실행하고 시도 횟수를 적는다. 슬라이스 종료 시점에 무효로 남은 항목은 비-통과 |
| **대기** | 선행 태스크 미완(E9). 그 라운드 판정에서 제외 |
| **기록** | 판정하지 않고 사실만 남긴다(§6 M-1~M-12) |

**검사 건수 원칙.** 순회·집합 항목은 **실제로 순회한 개수**가 증거에 드러나야 한다 — cargo `N passed` 와 테스트가 찍은 건수, SQL 은 조인·집계된 행 수, 봇은 보낸·받은 메시지 수. **결과의 분모와 판정 근거의 분모를 둘 다 찍는다.** 예: 원장 항등식은 "성립한 키 수 / 검사한 키 수"(결과의 분모)와 "그 키들을 만든 `MINERAL_MINED` 수"(판정 근거의 분모)를 둘 다 낸다. **분모가 0 이면 FAIL 로 인쇄한다**(CLAUDE.md 검증의 규율 — 빈 집합에 대한 전칭명제는 참이다).

**바이너리 출처 (규칙 9 이행).** 실서버·봇·Unity 로 판정하는 항목의 증거에 `git rev-parse HEAD` + `git status --porcelain` 공백 여부(더러우면 목록 그대로) + 서버 바이너리 sha256 앞 16자를 싣는다. `server_boot.py` 의 `source_provenance()` 를 재사용한다.

### 0.4 하드 게이트와 기록의 분리

| 구분 | 대상 | 판정 |
|------|------|------|
| **정확성 하드 게이트** | SC-01 ~ SC-114 중 SC-87 제외 전부 | PASS / FAIL / 미검증 / 무효 |
| **성능** | **판정하는 것은 둘: SC-87 `tick 초과 비율 ≤ 0.5 %`, SC-64 `역사 알림 지연 max ≤ 5 초`** | 나머지는 M-1~M-6 |
| **기록만** | M-1 ~ M-12 | 값이 안 나와도 FAIL 아님 |

p1-01 기준선(비교용, 판정 아님): p1-01 `05_summary.md` 의 부하 수치를 M-1 옆에 나란히 적는다. **채굴이 들어오면 영속화 트랜잭션의 행 수가 늘어나는 것은 회귀가 아니라 예상**이다(스펙 §8).

### 0.5 반례 40건의 층별 거부 책임 (SC-36 의 채점 기준) — **구현 중 채운다**

스펙 §5.2 가 "층별 거부 책임은 server·client 가 구현 중 실측해 표로 채운다(p1-01 §5.4 형식)" 로 정했다. **이 표가 채워지기 전에는 SC-36 은 대기**다. 채운 뒤 결과가 표와 다르면 **FAIL 이 아니라 architect 통지**(계약 설계 변경).

| 층 | 기대 | 검사 건수 | 채우는 사람 |
|----|------|---------|-----------|
| ① 스키마 검증 | **40건 전부 거부** (기존 34 포함 74) | 74 | architect 실측 완료(스펙 §5.2) |
| ② Rust serde(운영 경로) | 반례별 "거부 / 통과(서버 판정이 잡음) / 대상 외(데이터 kind 는 로더가)" | 40 | server (S1) |
| ③ C# `Strict` / `Runtime` | 반례별 "거부 / 감지 불가 / 계층 없음(데이터 4종)" — **스펙 §5.3 규약**(C# 열은 실측으로 채우는 열이다. "반례 = 역직렬화 실패" 가 아니다). architect 예측: 거부 15 · 감지 불가 13 · 계층 없음 11 · 불확실 1(`DEPOSIT_FIELD_STATE` missing-mineral-key) | 40 | client (C1) |

**② 열 — 기대(독립 출처)와 실측 기록 (qa 2026-09-30, 리더 정정 반영)**

- **기대 열의 출처**: 스펙 §5.3 표의 "Rust serde (예측)" 열(architect, 반례 fixture 설계와 같이 쓴 것)과 그 규약 문장 "예측은 전부 거부이고 … 하나라도 통과하면 그것은 serde 타입의 구멍이다(AC-8)". 데이터 4종 11 건도 같은 표의 마지막 행이 "거부"로 예측한다. **테스트 출력에서 오지 않는다.**
- **의도한 위반 열의 출처**: 각 반례를 같은 타입의 유효 fixture 중 가장 가까운 것과 필드 단위로 비교한 변형(`fxdiff.py` — 증거 `evidence/r2_sc36/fxdiff.txt`). 40 건 중 33 건은 변형이 1 필드, 6 건은 한 배열의 길이·원소 변화(`participants`·`source_event_ids`·`items` 2·`deposits`·`position_m`), 1 건(`quantity-zero`)은 두 수량 필드를 함께 0 으로 바꾼 것이다. 이것도 테스트 출력과 독립이다.
- **실측 기록 열**: r1 `cargo test -p starfall-contracts -- --nocapture` 의 `invalid_serde_matrix` 출력(총 74 = 기존 34 + 신규 40) 전사. 행 순서·파일은 테스트 순회(레지스트리 `CONTRACT_TYPES` 순 × 파일 이름 정렬)와 한 줄씩 대조(불일치 0). 증거 `evidence/r1_20260930/sc35_contracts_nocapture.log`. **이 열은 기대가 아니다.**
- **판정 대상은 두 대조**다: (1) 기대 ≠ 실측인 칸, (2) 거부됐지만 **사유가 의도한 위반을 가리키지 않는** 칸(다른 우연한 결함 때문에 거부된 반례는 의도한 불변식을 재지 않는다). 둘 중 하나라도 있으면 §0.5 머리 문단대로 architect 통지.

| 타입 | 반례 | 의도한 위반 (유효 fixture 대비) | ② 기대 (스펙 §5.3) | ② 실측 기록 (r1) | 거부 사유 (serde 메시지) | 사유 ↔ 의도 |
|---|---|---|---|---|---|---|
| MINE_RESOURCE | `actor-field-injected.json` | envelope `actor_id` 추가 | 거부 | 거부 | unknown field `actor_id`, expected one of `command_id`, `command_type`, `schema_version`, `client_sent_at`, `p | 일치 |
| MINE_RESOURCE | `deposit-id-not-kebab.json` | `deposit_id` far-reach→Far_Reach | 거부 | 거부 | DataId 패턴에 맞지 않는다: Far_Reach | 일치 |
| MINE_RESOURCE | `mineral-field-injected.json` | `payload.mineral_id` 추가 | 거부 | 거부 | unknown field `mineral_id`, expected `deposit_id` | 일치 |
| MINE_RESOURCE | `missing-deposit-id.json` | `payload.deposit_id` 삭제 | 거부 | 거부 | missing field `deposit_id` | 일치 |
| MINE_RESOURCE | `position-field-injected.json` | `payload.position_x_mm` 추가 | 거부 | 거부 | unknown field `position_x_mm`, expected `deposit_id` | 일치 |
| MINE_RESOURCE | `quantity-field-injected.json` | `payload.quantity_kg` 추가 | 거부 | 거부 | unknown field `quantity_kg`, expected `deposit_id` | 일치 |
| MINERAL_MINED | `actor-id-null.json` | `actor_id` → null (단일 변형) | 거부 | 거부 | invalid type: null, expected a string | 일치 |
| MINERAL_MINED | `causation-id-null.json` | `causation_id` → null (단일 변형) | 거부 | 거부 | invalid type: null, expected a string | 일치 |
| MINERAL_MINED | `importance-level-injected.json` | `importance_level` 추가 | 거부 | 거부 | unknown field `importance_level`, expected one of `event_id`, `event_type`, `schema_version`, `world_id`, `tic | 일치 |
| MINERAL_MINED | `quantity-fractional.json` | `quantity_kg` 25→25.5 | 거부 | 거부 | invalid type: floating point `25.5`, expected i32 | 일치 |
| MINERAL_MINED | `quantity-zero.json` | `quantity_kg`·`quantity_after_kg` 25→0 | 거부 | 거부 | MassKg 는 이 필드에서 0일 수 없다 (최소 1) | 일치 |
| MINERAL_MINED | `remaining-negative.json` | `deposit_remaining_after_kg` 475→-25 | 거부 | 거부 | MassKg 는 0 ..= 2147483647 범위여야 한다 (받음: -25) | 일치 |
| MINERAL_DISCOVERED | `domain-envelope-field-sequence.json` | `sequence` 추가 | 거부 | 거부 | unknown field `sequence`, expected one of `historical_event_id`, `event_type`, `schema_version`, `world_id`, ` | 일치 |
| MINERAL_DISCOVERED | `fact-status-interpretation.json` | `fact_status` CONFIRMED→DISPUTED | 거부 | 거부 | unknown variant `DISPUTED`, expected `CONFIRMED` | 일치 |
| MINERAL_DISCOVERED | `historical-id-v7-not-derived.json` | `historical_event_id` v5→v7 | 거부 | 거부 | UuidV5 는 버전 5이어야 한다 (받은 값의 버전: 7, 값: 01a0b1c2-a001-7e01-8f11-505162738495) | 일치 |
| MINERAL_DISCOVERED | `importance-level-zero.json` | `importance_level` 2→0 | 거부 | 거부 | importance_level 은 1 ..= 5 여야 한다 (받음: 0) | 일치 |
| MINERAL_DISCOVERED | `narrative-field-injected.json` | `headline` 추가 | 거부 | 거부 | unknown field `headline`, expected one of `historical_event_id`, `event_type`, `schema_version`, `world_id`, ` | 일치 |
| MINERAL_DISCOVERED | `participants-discoverer-only.json` | `participants` 2→1 (SHIP 제거) | 거부 | 거부 | participants 는 정확히 2개여야 한다 (받음: 1) | 일치 |
| MINERAL_DISCOVERED | `rule-version-without-number.json` | `rule_version` `@1` 제거 | 거부 | 거부 | RuleVersion 패턴에 맞지 않는다: mineral-discovery | 일치 |
| MINERAL_DISCOVERED | `source-event-ids-empty.json` | `source_event_ids` 1→0 | 거부 | 거부 | source_event_ids 는 정확히 1개여야 한다 (받음: 0) | 일치 |
| INVENTORY_STATE | `capacity-field-injected.json` | `payload.capacity_kg` 추가 | 거부 | 거부 | unknown field `capacity_kg`, expected `actor_id` or `items` | 일치 |
| INVENTORY_STATE | `fractional-quantity.json` | 항목 `quantity_kg` 200.5 | 거부 | 거부 | invalid type: floating point `200.5`, expected i32 | 일치 |
| INVENTORY_STATE | `zero-quantity-item.json` | 항목 `quantity_kg` 0 | 거부 | 거부 | MassKg 는 이 필드에서 0일 수 없다 (최소 1) | 일치 |
| DEPOSIT_FIELD_STATE | `hint-field-injected.json` | 광맥 `rarity` 추가 | 거부 | 거부 | unknown field `rarity`, expected one of `deposit_id`, `mineral_id`, `initial_reserve_kg`, `remaining_kg`, `fir | 일치 |
| DEPOSIT_FIELD_STATE | `missing-mineral-key.json` | 광맥 `mineral_id` 키 삭제(null 아님) | 거부 | 거부 | missing field `mineral_id` | 일치 |
| DEPOSIT_FIELD_STATE | `remaining-negative.json` | 광맥 `remaining_kg` null→-1 | 거부 | 거부 | MassKg 는 0 ..= 2147483647 범위여야 한다 (받음: -1) | 일치 |
| HISTORICAL_EVENT_NOTICE | `nested-level-zero.json` | 중첩 `importance_level` 2→0 | 거부 | 거부 | importance_level 은 1 ..= 5 여야 한다 (받음: 0) | 일치 |
| HISTORICAL_EVENT_NOTICE | `sentence-injected.json` | `payload.headline` 추가 | 거부 | 거부 | unknown field `headline`, expected `delivery` or `historical_event` | 일치 |
| HISTORICAL_EVENT_NOTICE | `unknown-delivery.json` | `delivery` LIVE→REPLAY | 거부 | 거부 | unknown variant `REPLAY`, expected `LIVE` or `BACKFILL` | 일치 |
| MINERAL | `regen-interval-fractional.json` | `regen_interval_s` 60→1.5 | 거부 | 거부 | invalid type: floating point `1.5`, expected i64 | 일치 |
| MINERAL | `unread-property.json` | `density_kg_m3` 추가 | 거부 | 거부 | unknown field `density_kg_m3`, expected one of `schema_version`, `id`, `display_name`, `rarity`, `designer_not | 일치 |
| MINERAL | `yield-fractional.json` | `yield_per_extraction_kg` 80→80.5 | 거부 | 거부 | invalid type: floating point `80.5`, expected i32 | 일치 |
| DEPOSIT_FIELD | `no-deposits.json` | `deposits` 2→0 | 거부 | 거부 | deposits 는 1 ..= 64 개여야 한다 (받음: 0) | 일치 |
| DEPOSIT_FIELD | `position-two-components.json` | `position_m` 성분 3→2 | 거부 | 거부 | invalid length 2, expected an array of length 3 | 일치 |
| DEPOSIT_FIELD | `reserve-fractional.json` | `initial_reserve_kg` 3200→3200.5 | 거부 | 거부 | invalid type: floating point `3200.5`, expected i32 | 일치 |
| MINING_RULES | `cooldown-fractional.json` | `cooldown_s` 3→2.5 | 거부 | 거부 | invalid type: floating point `2.5`, expected i64 | 일치 |
| MINING_RULES | `range-zero.json` | `mining_range_from_surface_m` 150→0 | 거부 | 거부 | de_mining_range_from_surface_m 는 (0 .. 5000 범위를 벗어난다 (받음: 0) | 일치 |
| SIGNIFICANCE_RULE | `foreign-input-rarity.json` | `rarity_weight` 추가 | 거부 | 거부 | unknown field `rarity_weight`, expected one of `schema_version`, `rule_id`, `rule_version`, `produces_event_ty | 일치 |
| SIGNIFICANCE_RULE | `importance-level-zero.json` | `importance_level` 2→0 | 거부 | 거부 | importance_level 은 1 ..= 5 여야 한다 (받음: 0) | 일치 |
| SIGNIFICANCE_RULE | `rule-version-malformed.json` | `rule_version` → `v1` | 거부 | 거부 | RuleVersion 패턴에 맞지 않는다: v1 | 일치 |

집계 (r1 실측 기준): 비교 반례 **40**. 대조 (1) 기대 = 실측 **40 / 40**(기대 거부 40, 실측 거부 40, 불일치 0). 대조 (2) 사유가 의도한 위반을 가리킴 **40 / 40** — 메시지가 필드를 이름으로 찍지 않는 3 건(`actor-id-null`·`causation-id-null` 의 `invalid type: null`, `quantity-zero` 의 "0일 수 없다")은 변형이 그 필드 하나뿐(또는 최소 1 규칙이 걸리는 필드가 `quantity_kg` 하나뿐)이라 다른 원인이 없다. **r2 에서 새 빌드로 다시 재고 이 두 대조를 다시 한다** — 이 표의 실측은 r1 기록이다.

- 기대의 선행성 (architect 확인 2026-09-30): §5.3 "Rust serde (예측)" 열은 2026-09-27 client 사전 검토(`02_client_ack.md`) 답변 때, **S1 구현 전**에 썼다. 출처는 측정이 아니라 p1-01 serde 관례(`deny_unknown_fields`·비-Option 좁힘·범위·패턴 newtype·`data.rs` 의 `de_*_range`)를 각 반례의 위반 종류에 대입한 것이다. 스펙 파일이 미추적이라 git 으로는 증명되지 않는다 — 증거 등급은 "팀 메시지 기록상 S1 완료 보고보다 앞선다" 까지다. serde 를 통과시키고 뒤 층이 잡도록 설계한 반례: **none**. 단서: 데이터 kind 11 건의 serde 거부 예측은 범위·개수 검사가 역직렬화기 안에 있다는 전제다 — 검사가 기동 검산으로 옮겨지면 층 이름이 달라진다(r2 실측에서 사유 열로 확인).

- 반례가 거는 불변식 목록(스펙 §5.2 끝 문단)은 표의 **행 제목**이 된다: 명령에 수량·광물·위치·actor 주입(I-48) · 역사 envelope 의 서사 필드·`sequence`·Level 0·`DISPUTED`·v7 id·빈 근거·버전 없는 규칙(I-60·I-63·I-64) · 도메인 이벤트에 `importance_level` 주입(I-60) · 인벤토리 0 kg·상한 필드 · 광맥 상태 힌트 필드 · 데이터 표의 소수 kg·읽히지 않는 속성·`rarity_weight`.

### 0.6 기대 숫자의 출처 — **`data/` 에서 읽지 않는다** (p1-01 §0.7 승계)

- 단위·통합 테스트의 기대 숫자는 **`contracts/fixtures/` 에서만** 온다. 특히 `MINERAL_DISCOVERED/starfall-glass.json` 의 `historical_event_id` 는 `MINERAL_MINED/first-extraction.json` 에서 **재현**하는 것이 H1 의 첫 실패 테스트다.
- **입력과 기대 숫자를 구분한다.** fixture 에 쓸 입력이 없는 성질(SC-67 모르는 닫힌 값 — fixture 에는 스키마가 막는 반례만 있다, SC-70 같은 id 의 LIVE·BACKFILL — 두 fixture 는 id 가 다르다)은 **합성 입력**을 쓴다(client 제안 수락). 리포트에 입력 출처를 **"fixture 변형"**(어느 fixture 의 어느 필드를 바꿨나) 또는 **"테스트 내장 리터럴"** 로 적는다. 기대 **숫자**는 여전히 fixture 에서 온다.
- `data/` 는 **기동 경로(SC-04·05)와 실서버·사람 세션 관측(SC-80~92)에만** 쓴다. 그때는 **그 시점의 `data/` 값을 증거에 함께 적는다**(수치가 바뀌면 리포트가 스스로 설명하게). 예: SC-85 의 "잔량 100" 은 `data/` 의 산출량에서 유도되므로 그 파일 값을 옆에 적는다.

### 0.7 구현 전 기준선 — **판정에 쓰지 않는다** (qa 실측 2026-09-27, HEAD `e0037a4`, 워킹트리 미커밋 39건)

빨간불이 정상이다. **전부 "코드가 계약을 못 따라온 것"이지 계약 결함이 아니다** — 이것이 server·client TDD 의 출발점이다.

| 항목 | 기준선 | 확인 |
|------|--------|------|
| 계약 커버리지 `--strict` | **types 23 / errors 25 / warnings 0**, `RESULT: FAIL` | qa 실행 (architect 실측과 일치) |
| `cargo test -p starfall-contracts --locked` (측정 당시 유효 fixture 45 — r1 계약 변경으로 **46**, `registry_consistency` 의 left 도 46 이 된다) | **5 FAILED / 5 passed** (lib 단위 33 passed 별도): `registry_server_types_mapped`(`:502`, MINE_RESOURCE 대응표 없음) · `registry_consistency`(`:637`, fixture **45 vs 기대 27**) · `schema_ids_match_paths`(`:682`, **29 vs 18**) · `fixtures_roundtrip`(`:323`, `COOLDOWN_ACTIVE` 미정의) · **`schemas_valid_offline`**(`:298`, **29 vs 18**) | qa 실행. **리더 실측은 4건이었다 — 다섯째 `schemas_valid_offline` 은 같은 원인(스키마 수 하드코딩)이다** |
| 이동 golden sha256 (SC-34 의 기준) | `initial.json` `824a6489f262367cac5662e61e2b91ec40f07838f05a308d14ffa24cbf42097f` · `inputs.jsonl` `c924e59ff6b19aa56ee6d6ad24bb59ad516bf0b95005767027c87acbfa11eb08` · `snapshots.jsonl` `3e00e527c116f2b514df3f3d950f1f891a926748bb08f76814ae32ab6daeb1fe` | qa `sha256sum` |
| `data/**/*.json` | **10 파일** (기존 3 + 신규 7) | qa `find` |
| 클라이언트 사본 `client/Assets/_Project/Data/**/*.json` | **3 파일** (신규 7 없음 → SC-50 테스트는 빨간불일 것 — **미실행**, 소스로 본 예측) | qa `find` |
| `validate_data_files.py` `TARGETS` | 3 glob (신규 4 없음 — `:28-32`) | qa 소스 읽기(부정) |
| `server/bins/game-server/src/data.rs` `load()` | `ships` · `world/systems` · `movement/sync-tuning.json` 만 읽는다(`:372-379`) — 신규 7 파일을 읽는 코드 없음 | qa 소스 읽기(부정) |
| Unity EditMode 상수 | `ExpectedValidFixtureCount = 27` · `ExpectedRoundTrippableValidFixtureCount = 21` (`ContractFixtures.cs:31,51`) | qa 소스 읽기 |
| 출처 게이트 (p1-01 계약) | `90 / 90`, 도구 29, Rust verdict 6 · reference 11, **위반 없음** | qa 실행 |

### 0.8 측정 환경과 빌드 금지 (p1-01 §0.8 승계)

- **측정 중 어떤 빌드도 돌리지 않는다**(`cargo`·Unity 임포트·`dotnet run`).
- 리포트에 적는다: Unity Editor 실행 여부, 동시 컨테이너 수, 봇 시드, 서버 빌드 프로필, 단계별 시각, **사용한 월드 id**.

### 0.9 이 슬라이스가 **증명하지 못하는 것** (리포트 요약에 반드시 싣는다)

1. **클라이언트 빌드의 정답 누출**(사용자 결정 Q3). 서버는 미확인 광맥의 광물을 보내지 않고(SC-39·40) 클라이언트 코드는 사본에서 읽지 않지만(SC-71), **`client/Assets/_Project/Data/world/deposits/cradle.json` 에 8 광맥 × `mineral_id`·`initial_reserve_kg` 가 그대로 실린다.** 자동 검증은 전부 초록이어도 빌드를 뜯으면 답이 보인다. 만기는 §9.
2. **크래시 손실 ≤ 1 초는 "없음" 이 아니다**(Q2). 이 계약은 **복사가 없다**(SC-16~22)를 증명하지 **손실이 없다**를 증명하지 않는다. 하드 킬 시험은 하지 않는다.
3. **"두 박자" 발견 배너의 체감**(1 초 목표)은 사람 세션(SC-68)만 본다. SC-64 의 5 초 경계는 "영원히 안 온다" 를 잡는 것이지 재미를 재지 않는다.
4. **Unity EditMode 는 CI 밖이다.** 클라이언트 사본 재복사(SC-72)·DTO 왕복(SC-66)은 사람이 돌린 결과 파일로만 닫힌다(§3.3).

### 0.10 오라클 SQL 은 **계약의 필드 이름**으로 쓴다 — 검토 문서 원문을 그대로 쓰면 조용히 틀린다

`01_history_review.md` §6.1 의 오라클은 계약 확정 **전에** 쓰여 `payload->>'system_id'`·`payload->>'quantity'`·`'<채굴 이벤트 이름>'` 을 쓴다. **계약은 `star_system_id`·`quantity_kg`·`MINERAL_MINED` 다.** 원문 그대로 돌리면 `->>` 가 전부 NULL 을 내고, `DISTINCT ON (NULL, NULL)` 이 **한 행**을 내며, 그 한 행은 "오라클 행 수 ≥ 2" 에서만 걸린다 — 분모 단언이 없었다면 초록이었을 모양이다. SC-58 의 도구는 아래를 쓴다:

```sql
SELECT DISTINCT ON (world_id, payload->>'star_system_id', payload->>'mineral_id')
       world_id, payload->>'star_system_id' AS star_system_id, payload->>'mineral_id' AS mineral_id,
       tick, sequence, event_id
FROM domain_events
WHERE world_id = '<새 월드>' AND event_type = 'MINERAL_MINED' AND (payload->>'quantity_kg')::bigint > 0
ORDER BY world_id, payload->>'star_system_id', payload->>'mineral_id', tick, sequence;
```

그리고 **양성 대조**: 같은 도구가 `payload->>'star_system_id' IS NULL` 인 `MINERAL_MINED` 행 수를 찍고, 0 이 아니면 FAIL(필드 이름을 잘못 찾고 있다).

### 0.11 대조 키는 `actor_id` 다 — `Pilot-xxxx` 가 아니다 (스펙 I-66, 리더 확인)

표지는 `actor_id` 끝 4 글자이고 **개발 ID 에서 실제로 충돌한다**: Unity `DefaultSubject` `01a0b1c2-7e57-7c11-8e57-000000000001` 과 bot-001 `01a0b1c2-b010-7000-8000-000000000001` 이 둘 다 `Pilot-0001`(`tools/bots/src/token.rs:11`, `client/.../DevAuthToken.cs:41,52`). **모든 항목·봇 로그·SQL 대조의 키는 `actor_id` 다.** 표지는 SC-110(표시 규칙)에서만 쓰고, 거기서도 충돌 쌍을 **음성 입력**으로 넣는다.

### 0.12 정지로 끝난 실행에는 `SESSION_CLOSED`·`SHIP_DESPAWNED` 가 없다 (ADR-0013 §5 K5)

SC-25 처럼 서버가 스스로 멈춘 실행은 종료 스윕이 커밋되지 않는다(SC-102 가 그것을 요구한다). **그 월드·그 구간에 p1-01 의 짝 검사(I-41 `SHIP_SPAWNED` ↔ `SHIP_DESPAWNED`, 세션 짝)를 걸면 거짓 빨간불이다.** 정지 실행은 **전용 새 월드**에서 하고, 그 월드 id 를 짝 검사 대상에서 **명시적으로 제외한 목록**으로 증거에 남긴다(조용히 빼지 않는다).

---

## 1. 검증 항목

**열 읽는 법.** `⊘` 칸 = 그 항목을 **자명하게 통과시키는 상태**(규칙 1)와 **같은 실행에서 그것을 배제하는 단언·분모**. PASS 는 이 칸의 단언이 출력에 있을 때만이다. `불가` 칸 = §7 의 E-코드, `W` = §0.2 새 월드 필수.

### A. 서버 게이트와 순수성 (AC-1) — server

| ID | 검증 항목 | 검증 방법 | ⊘ 자명 통과 → 배제 단언 | 담당 | 근거 | 불가 |
|----|----------|----------|------------------------|------|------|------|
| SC-01 | `cargo fmt --all --check` · `cargo clippy --workspace --all-targets -- -D warnings` · `cargo test --workspace --locked` · `cargo test -p starfall-sim --release --locked` 넷 전부 종료 코드 0, 실패 0 | 네 명령 그대로. 크레이트별 `N passed` 를 증거에 | **새 크레이트가 워크스페이스에 없어 0 건 통과** → `cargo test` 출력에 `starfall-history` 의 테스트 바이너리가 있고 `passed ≥ 1` 임을 찍는다. 크레이트별 통과 수를 기준선(§0.7)과 나란히 | server | AC-1 | E1 |
| SC-02 | `starfall-sim`·`starfall-history` 의 `Cargo.toml` `[dependencies]` 에 `axum`·`sqlx`·`redis`·`rand`·`chrono`·`tokio` 가 없다. history 의 의존은 `starfall-contracts`(+ `uuid` v5) 뿐(ADR-0014 §1) | `grep -nE "axum\|sqlx\|redis\|rand\|chrono\|tokio" server/crates/sim/Cargo.toml server/crates/history/Cargo.toml` → 0 매칭. 두 `[dependencies]` 전문을 증거에 | **검사 대상 파일이 없어 grep 이 0 매칭** → 두 파일 경로를 `ls` 로 먼저 찍고 **검사한 크레이트 이름 2개**를 출력한다. 양성 대조: 같은 grep 을 `server/crates/persistence/Cargo.toml` 에 돌려 `sqlx` 가 ≥ 1 매칭 | server | AC-1 | E1 |
| SC-03 | `starfall-history` 소스에 `SystemTime`·`Instant` 호출 0, `HashMap` 순회 결과를 출력 순서로 쓰는 곳 0 | `grep -rnE "SystemTime\|Instant::\|HashMap" server/crates/history/src` → `SystemTime`·`Instant` 0. `HashMap` 이 나오면 그 줄마다 순회 여부를 파일:라인으로 설명 | **소스 디렉터리 경로 오타로 0 매칭** → 같은 grep 을 `server/crates/gateway/src` 에 돌려 `Instant` ≥ 1 매칭(양성 대조). 소스는 부정만 증명한다(규칙 9) — 긍정 짝은 SC-45·46·59 의 결정성 실행 | server | AC-1, ADR-0014 §1 | E1 |

### B. 데이터 표와 기동 거부 (AC-2, AC-19 c·c2) — server · qa

| ID | 검증 항목 | 검증 방법 | ⊘ 자명 통과 → 배제 단언 | 담당 | 근거 | 불가 |
|----|----------|----------|------------------------|------|------|------|
| SC-04 | 정상 `data/` 로 기동 → `/debug/stats` 에 **광물 4 · 광맥 8 · `rule_version = mineral-discovery@1`**, 이 셋이 파일과 일치 | 새 월드(§0.2)로 실서버 기동 — `python tests/e2e/server_boot.py p1-02-boot --world <id> --log <기동 로그>`(stats 수집 + stdin `shutdown`, 기동 로그는 SC-07 입력), `/debug/stats`. 기대값은 `find data/minerals -name '*.json' \| wc -l`·`jq '.deposits \| length' data/world/deposits/cradle.json`·`jq -r .rule_version data/history/rules/mineral-discovery.json` 로 **같은 실행에서** 계산 | **stats 필드가 없어 파서가 기본값 0 과 0 을 비교** → 세 필드가 stats JSON 에 **존재**함을 먼저 단언하고, 파일에서 계산한 기대값이 0 이 아님을 찍는다 | server | AC-2(a) | E2 E5 W |
| SC-05 | 스펙 §4.7 의 조건을 **하나씩** 깨뜨린 `STARFALL_DATA_DIR` 에서 **전부 기동 실패**, 로그에 **파일과 필드**. 경우: ① 광맥 `mineral_id` 가 광물 표에 없음 ② 광맥 없는 광물 ③ 광맥 `star_system_id` ≠ 로드된 성계 ④ 소프트 경계 초과 ⑤ `cooldown_s × tick_hz` 비정수 ⑥ `regen_interval_s × tick_hz` 비정수 ⑦ `regen_kg > initial_reserve_kg` ⑧ 광물 id 중복 ⑨ 광맥 id 중복 ⑩ 채굴 구역 겹침 ⑪ `rule_version` 이 `rule_id@` 로 시작 안 함 ⑫ 규칙 `event_type` 이 레지스트리의 `historical_event` 아님 ⑬~⑯ 신규 4 스키마 각 1건 위반 = **16 경우**. *(r2 보강 2026-09-28, 리더 방침 — ⑤⑥ 의 원문("초 × tick_hz 가 비정수")은 스키마가 초를 정수로 강제해 **도달 불가**다. 같은 의도의 **도달 가능한** 반례로 바꾼다: ⑤ `mining-rules.json` 의 `cooldown_s` 를 소수(예: 3.5)로 ⑥ 광물 파일의 `regen_interval_s` 를 소수로 → **기동 실패**, 로그에 그 필드 이름. 분모 **16 유지**. 유도 곱셈의 양성 짝은 `data::cooldown_ticks_is_exactly_derived`)* | server: `cargo test -p starfall-game-server data::tests::reject_c` — `data::tests::reject_c01_deposit_unknown_mineral` … `data::tests::reject_c16_*`(경우 번호가 이름에) + 양성 대조 `data::tests::accepts_unmodified_copy`. qa: `server_boot.py` 로 실바이너리 3 경우(①·⑦·⑪) — 로그 줄을 증거에 | **기동 실패 원인이 주입한 결함이 아님**(DB 미기동·경로 오류) → 각 경우의 로그에서 **주입한 필드 이름**을 단언 — **스키마층 ⑬~⑯ 도 포함**(오류 종류 `Schema` 만 보면 p1-01 규칙 9 근거 사례처럼 메시지에 필드가 없어도 통과한다). **양성 대조**: 같은 하네스로 **무변경 사본** 기동이 성공(실패 0 인 대조가 없으면 "하네스가 늘 실패" 와 구분 안 된다). 경우 수 16 을 출력 | server · qa | AC-2(b), I-38 확장 | E1 E2 |
| SC-06 | 실제 `data/` 의 **10 파일 전부**가 스키마를 통과하고 `validate_data_files.py` `TARGETS` 가 신규 4 경로(`data/minerals/*.json`·`data/world/deposits/*.json`·`data/mining/mining-rules.json`·`data/history/rules/*.json`)를 포함 | `python tests/e2e/validate_data_files.py` → 종료 코드 0, `files_checked = 10` | **glob 0 매치** → 도구가 glob 별 파일 수를 찍고 0 인 glob 은 오류(이미 있는 동작, `:92-96`). **파일 수 10 을 판정 조건에 넣는다**(`≥ 1` 이 아니다 — 광물 한 파일이 사라져도 통과하면 안 된다. 단 designer 의 정상 추가를 막지 않도록 **"10 이 아니면 FAIL 이 아니라 architect·designer 통지"** — p1-01 §9 의 SC-07 교훈) | qa | AC-19(c) | — |
| SC-07 | **읽히지 않는 데이터 파일 0.** `data/**/*.json` 각 파일마다 ① 기동 시 읽는 코드 — **서버 기동 로그에 그 파일의 해석 경로가 찍힌 줄**(실행 증거) ② 검사하는 게이트 — 스키마 검증 + 유도값 검산 테스트 또는 golden 이름. 짝이 없는 파일이 1 개라도 있으면 FAIL. §2 표가 판정 대상 | `python tests/e2e/data_file_pairs.py --boot-log <SC-04 의 서버 로그>` → 파일별 (로그 줄, 게이트) 표와 `짝 있음 N / 파일 M`. 종료 코드 0 ⇔ N == M | **분모가 0**(glob 오류) → `M = find data -name '*.json' \| wc -l` 을 같은 실행에서 세어 찍는다(오늘 10). **음성 대조**: `data/` 사본에 아무도 읽지 않는 `unused/x.json` 을 넣은 실행에서 **FAIL** 이 나온다(규칙 6). "읽는 코드" 를 소스 grep 으로 채우지 않는다 — **규칙 9: 소스는 긍정을 증명하지 못한다** | qa | AC-19(c2) | E2 E5 |

### C. 채굴 판정 — 수동 step 통합 테스트 (AC-3) — server

소켓·DB 없이 `starfall-sim` 을 직접 step 한다. 기대 숫자는 fixture 에서(§0.6).

| ID | 검증 항목 | 검증 방법 | ⊘ 자명 통과 → 배제 단언 | 담당 | 근거 | 불가 |
|----|----------|----------|------------------------|------|------|------|
| SC-08 | 모든 조건 충족 → **수락**, 인벤토리 = 정확히 산출, `MINERAL_MINED` **1건**, `causation_id = command_id`, `quantity_before/after`·`deposit_remaining_before/after` 가 맞고, 광맥이 **드러난다**(`first_extracted_tick = 그 tick`) | `cargo test -p starfall-sim mining::accept_*` | **모든 경우가 거부로 끝나 수락 경로 미실행** → 테스트가 **수락 수 ≥ 1** 과 이벤트 수를 찍는다 | server | AC-3(a) | E1 |
| SC-09 | §4.2 표 사유 **2~7 각각**(`TARGET_UNKNOWN`·`COOLDOWN_ACTIVE`·`TARGET_OUT_OF_RANGE`·`SHIP_TOO_FAST`·`RESOURCE_DEPLETED`·`CAPACITY_EXCEEDED`)을 만드는 상태에서 거부되고 **인벤토리·광맥·쿨다운 기준 tick·처리 장부가 바뀌지 않는다**. 7 은 적재량을 `MassKg` 상한 − 1 로 주입(ADR-0013 §5a-1 `quantity_kg_check` 의 ③) | ① sim: `cargo test -p starfall-sim mining::each_rejection_reason` (테스트 2개가 사유 6개를 나눠 맡는다: `each_rejection_reason_leaves_prior_accepted_state_untouched` = 2·4·5, `…_reasons_3_6_7` = 3·6·7. 판정은 사유 2~7 **각각**의 단언과 ⊘ 기준).<br>② gateway: `cargo test -p starfall-gateway mine_resource_capacity_exceeded_is_counted_in_commands_rejected_total` — 7 의 `commands_rejected_total{CAPACITY_EXCEEDED}` +1 단언(카운터가 `starfall-gateway::Stats` 소유. 소켓 → sim 판정 7 거부 → `runtime.rs` 송신 경로의 `record_rejection` → `/debug/stats`). **디버그·릴리스 두 프로필에서** 같은 결과(`cargo test -p starfall-sim --release` 에도 포함 — ADR-0013 §5a-2 #4: panic 을 두지 않는다) | **비교 대상 상태가 비어 있어 "안 바뀜" 이 공짜**(빈 인벤토리·쿨다운 기준 없음) → 거부 전 상태에 **수락 1건이 이미 있어** 인벤토리 > 0, 쿨다운 기준 tick 존재, 장부 ≥ 1 행임을 먼저 단언(쿨다운 사유 외 경우는 쿨다운이 이미 끝난 시점에서). 사유별 관찰 수 6개를 찍는다. 7 은 **주입값이 정말 상한 − 1** 이고 산출 ≥ 2 라 덧셈이 넘친다는 것을 입력 단계에서 단언 | server | AC-3(b), I-51, ADR-0013 §5a | E1 |
| SC-10 | 두 사유가 동시에 성립하면 **표 순서의 앞 사유**. 필수 쌍: 쿨다운 중 + 사거리 밖 → `COOLDOWN_ACTIVE`(스펙 명시). I-50 의 순서 전체를 위해 이웃한 쌍 다섯(2+3, 3+4, 4+5, 5+6, 6+7)도 | `cargo test -p starfall-sim mining::reject_order_*` (4+5·6+7·2+3·5+6) + `cargo test -p starfall-sim mining::cooldown_before_range_in_judgement_order` (필수 쌍 3+4) | **"동시 성립" 이 실제로는 한 조건만 성립** → 같은 테스트에서 각 조건을 **단독으로** 만든 상태가 각자의 사유로 거부됨을 먼저 단언한다(쌍마다 3 단언) | server | AC-3(c), I-50 | E1 |
| SC-11 | 거부는 쿨다운을 시작하지 않는다: 쿨다운 외 사유(예: 사거리 밖)로 거부 → 그 직후 tick 에 조건을 만족시키면 **수락** | `cargo test -p starfall-sim mining::reject_does_not_start_cooldown` | **거부와 수락 사이가 `cooldown_ticks` 이상이라 쿨다운이 어차피 끝남** → `수락 tick − 거부 tick < cooldown_ticks` 를 단언. 그리고 **그 거부 전에 수락이 한 번도 없는 actor** 로(이전 수락이 쿨다운을 걸고 있지 않게) | server | AC-3(d), I-51 | E1 |
| SC-12 | 잔량 < 산출량이면 **부분 산출**(= 잔량), 다음 명령은 `RESOURCE_DEPLETED` (S-3 단위판) | `cargo test -p starfall-sim mining::partial_then_depleted` + fixture `MINERAL_MINED/partial-last-kg-depletes.json` | **잔량이 산출량의 배수라 부분 산출 경로 미실행** → 처리 직전 `0 < 잔량 < yield` 를 단언하고 산출 = 잔량 ≠ yield 를 찍는다 | server | AC-3(e) | E1 |
| SC-13 | 같은 tick 의 두 채굴은 **제출 순서**대로 처리된다 — 제출 순서를 바꾸면 **결과가 바뀐다** (`ship_id` 순이 아니다, I-71) | `cargo test -p starfall-sim mining::same_tick_submission_order` — 잔량이 한 번 분량뿐인 광맥에 두 함선 | **두 순서 모두 같은 결과**(경쟁 없는 상태) → 두 실행의 결과가 **다름**을 단언(한쪽은 A 수락·B 소진, 다른 쪽은 반대). **`ship_id` 가 작은 쪽이 나중에 제출된 경우를 포함**한다(아니면 `ship_id` 순과 구분 안 된다) | server | AC-3(f), I-71 | E1 |
| SC-14 | 회복: 소진된 광맥이 `regen_interval` 배수 tick 을 지나면 `regen_kg` 만큼, 초기값 상한 — **닫힌 식(I-69)과 tick 별 메모리 시뮬레이션이 모든 tick 에서 같은 값** | `cargo test -p starfall-sim mining::tests::regen_closed_form_matches_stepwise` | **시험 구간이 배수 경계를 안 지남 / 상한에 안 닿음** → 지난 경계 수 ≥ 2, **상한 도달 ≥ 1**, 비교한 tick 수를 찍는다. 시작 `as_of` 가 경계 **사이**인 경우 포함(`⌊as_of/I⌋` 항이 0 이 아닌 경우) | server | AC-3(g), I-69 | E1 |
| SC-15 | 사거리·속도 판정은 **tick 시작 시점(이번 tick 적분 전)** 의 서버 `f64` 상태로 한다 — 이번 tick 적분 뒤에야 사거리에 들어오는 함선은 `TARGET_OUT_OF_RANGE` | `cargo test -p starfall-sim mining::range_uses_pre_integration_state` | **적분 전후 모두 사거리 밖/안**(시점이 결과를 안 가름) → 적분 전 거리² > 한계², 적분 후 거리² ≤ 한계² 임을 먼저 단언 | server | AC-3, §4.2 표 4·5행 | E1 |

### D. 복사 경로 — 같은 `command_id` 는 한 번 (AC-4) — server

**모든 경로 공통 ⊘**: *첫 전송부터 거부되어 두 번째도 당연히 안 늘어남* → **첫 전송이 `ACCEPTED` 였고 인벤토리가 늘었음을 먼저 단언**한다. 각 경로의 최종 인벤토리 = **산출 × 1**.

| ID | 검증 항목 | 검증 방법 | ⊘ 자명 통과 → 배제 단언 | 담당 | 근거 | 불가 |
|----|----------|----------|------------------------|------|------|------|
| SC-16 | 한 세션에서 같은 명령 2회 → 둘째 `DUPLICATE_COMMAND_ID`, 인벤토리 산출 × 1 | `cargo test -p starfall-sim mining::dup_same_session` | 공통 ⊘ + 둘째 도착이 **쿨다운 안**이라 "쿨다운이 막았다" 로도 설명되는 경우 → 사유 코드가 `COOLDOWN_ACTIVE` 가 **아니라** `DUPLICATE_COMMAND_ID` 임을 단언(I-50: 1 이 3 보다 앞) | server | AC-4(a), I-52 | E1 |
| SC-17 | 끊고 **잔류 창 안** 재접속(새 세션, 같은 actor) → 같은 명령 → `DUPLICATE_COMMAND_ID` | `cargo test -p starfall-sim mining::dup_across_reconnect` (세션 교체를 sim 수준에서) | 공통 ⊘ + **재접속이 같은 세션으로 처리됨** → 두 명령의 `session_id` 가 다름을 단언 | server | AC-4(b) | E1 |
| SC-18 | **정상 종료(stdin `shutdown`) → 재기동 → 같은 actor 접속 → 같은 명령** → `DUPLICATE_COMMAND_ID`, 인벤토리 행·`processed_commands` 행 수 불변 | server: DB 통합 테스트(`cargo test -p starfall-persistence restart_reloads_economic_state_so_cas_succeeds_and_duplicates_are_rejected` [DB] — 적재 → sim 기억 주입 경로, S8). qa 실서버판은 SC-78 | 공통 ⊘ + **재기동 전 배치가 커밋되지 않아 장부가 비어 있음** → 재기동 전에 `processed_commands` 에 그 `command_id` 행이 **있음**을 SQL 로 단언 | server | AC-4(c), I-52 | E1 E2 |
| SC-19 | 같은 actor 의 두 연결(넘겨받기, p1-01 I-29)이 **같은 tick 에** 같은 `command_id` → 수락 1, 중복 1 | `cargo test -p starfall-sim mining::dup_two_sessions_same_tick` | 공통 ⊘ + **두 명령이 다른 tick 에 처리됨**(그러면 SC-16 과 같은 경로) → 두 `COMMAND_RESULT` 의 envelope `tick` 이 같음을 단언 | server | AC-4(d) | E1 |
| SC-20 | **이미 커밋된 tick 배치를 영속화에 다시 넣음** → 모든 테이블 행 변화 0, 에러 없음, 둘째 커밋의 반환값이 **`AlreadyCommitted`**, **`persist_ambiguous_commits_total` +1**(I-57 — 건너뜀 경로가 실제로 실행됐다는 증거, ADR-0013 K6) | `cargo test -p starfall-persistence batch_recommit_is_noop` [DB] — 재시도 루프 진입점을 두 번 호출(반환값·카운터 +1). **둘째 줄** `ambiguous_commit_retry_counts_once` [DB] — 커밋에 성공하고도 `Err` 를 돌려주는 주입 래퍼 → `persist_ambiguous_commits_total == 1` 이고 `persisted_total == COUNT(*)`(첫 테스트 모양은 `persisted_total` 을 두 번 더하므로 그 항등식을 거기서 보지 않는다 — server) | **첫 커밋이 실제로 안 됐음** → 재투입 전 `last_tick ≥ batch.tick` 이고 그 배치의 이벤트·상태·장부 행이 **각 ≥ 1** 임을 단언. **"아무것도 안 했다" 와 "건너뛰었다" 가 행 수로는 같다** → 반환값과 카운터 +1 이 판정 조건(행 수 불변만으로는 PASS 아님) | server | AC-4(e), ADR-0013 §4 | E1 E2 |
| SC-21 | sim 의 기억을 우회해 중복이 DB 에 도달 → **PK `(world_id, command_id)` 위반 → 정지 경로**(둘째 배치 `Err(Fatal)`), 인벤토리 불변(마지막 방어선이 실제로 걸린다) | `cargo test -p starfall-persistence dup_reaching_db_halts` [DB]. 재기동을 넘는 같은 방어선: `cargo test -p starfall-persistence dup_across_restart` [DB](sim 기억 없이 재전송 → PK 정지) | **sim 기억이 막아 DB 에 도달조차 안 함** → 테스트가 PK 위반 에러(SQLSTATE 23505)를 **관측**하고 `persist_fatal_total` 이 +1 됐음을 단언 | server | AC-4(f), ADR-0013 §3 | E1 E2 |
| SC-100 | **다른 actor**(토큰 둘)가 같은 `command_id` → 수락 1, `DUPLICATE_COMMAND_ID` 1, **서버가 멈추지 않는다**. **다른 월드**에서 같은 `command_id` 는 수락 — 지속 기억과 PK 가 모두 월드 범위 `(world_id, command_id)`(ADR-0013 §3 K1) | `cargo test -p starfall-sim duplicate_command_id_across_different_sessions_is_rejected`(actor id(1)·id(3) — r2 이름 정정, server 제안: 파일의 다른 sim 테스트와 같은 서술형) + DB 통합 `cargo test -p starfall-persistence dup_cross_actor_no_halt` [DB](커밋까지 가서 PK 가 걸리지 않음) · `cross_world_same_command_id_accepted` [DB]. qa 실서버판은 SC-108 | 공통 ⊘ + **두 actor 가 실제로 다름** → 두 `COMMAND_RESULT` 의 세션 actor 가 다름을 단언. **정지 없음**은 `persist_fatal_total` = 0 **과** 그 뒤 배치가 커밋됨(다음 tick `last_tick` 전진)으로 — 실패가 조용히 삼켜져도 카운터는 0 일 수 있다 | server | AC-4(d2), ADR-0013 §3 K1 | E1 E2 |
| SC-22 | 세션 안에서 **거절된** 명령의 재전송 → `DUPLICATE_COMMAND_ID`(그 사이 조건이 풀렸어도). 재접속 뒤의 거절 명령 재전송은 새로 판정된다. **어느 쪽이든 수락 ≤ 1** | `cargo test -p starfall-sim mining::rejected_resend_*` (두 갈래) | 이 경로만 공통 ⊘ 의 예외(첫 전송이 거절이 정상). 대신 **"그 사이 조건이 풀렸다"** 를 단언: 같은 상태에서 **새** `command_id` 는 수락된다. 재접속 갈래는 재판정이 실제로 **수락**됐음을 찍는다(수락 1, 그 뒤 보존 법칙 성립) | server | AC-4(g), I-52 | E1 |

### E. 원장·보존·비교 후 쓰기 (AC-5) — server · qa

| ID | 검증 항목 | 검증 방법 | ⊘ 자명 통과 → 배제 단언 | 담당 | 근거 | 불가 |
|----|----------|----------|------------------------|------|------|------|
| SC-23 | **원장 항등식(I-54)**: 각 `(world, actor, mineral)` 에 인벤토리 행 = `Σ quantity_kg` = 최신 `quantity_after_kg`, 연쇄(`앞.after = 뒤.before`, 첫 `before = 0`) | 봇 ≥ 2 × 채굴 N 회 뒤 `python tests/e2e/mining_ledger.py --world <id> ledger` | **채굴 0 건이면 `0 = 0`** → **검사한 키 ≥ 2, `MINERAL_MINED` ≥ 10 이 아니면 FAIL**. 키 수·이벤트 수·연쇄 이음 수를 찍는다. 음성 대조(selftest): 합성 행에서 연쇄 한 칸을 어긋나게 하면 FAIL | qa | AC-5(a), I-54 | E2 E6 W |
| SC-24 | **보존 법칙(I-55)**: 각 `(world, mineral)` 에 `Σ 인벤토리 + Σ 광맥 잔량 = Σ 초기 매장량 + Σ 실제 회복량`, 광맥별 연쇄 `뒤.before = regen(앞.after, 앞.tick → 뒤.tick)` — **회복량은 이벤트를 되감아 식(I-69)으로 재계산**하고 재계산 잔량이 DB 와 다르면 FAIL | `python tests/e2e/mining_ledger.py --world <id> conservation` (초기 매장량·회복 매개변수는 그 시점 `data/` 값 — 증거에 함께) | **채굴 0 광물에서 `0 = 0`** → 채굴 0 회 광물도 행으로 찍되 **채굴 수**를 옆에 적는다. 채굴이 있는 광물 ≥ 2 가 아니면 FAIL. 연쇄 검사한 광맥 수·이음 수 출력. 음성 대조(selftest): 회복을 한 경계 늦게 계산하면 FAIL | qa | AC-5(a), I-55 | E2 E6 W |
| SC-25 | 서버가 떠 있는 동안 **한 actor 의 인벤토리 행을 SQL 로 바꾼 뒤** 그 actor 가 채굴 → 서버가 **스스로 정지**: 0 아닌 종료 코드, 로그에 비교 후 쓰기 실패와 키, `persist_fatal_total ≥ 1` — **정지 로그 한 줄에 카운터 값과 키가 함께 찍힌다**(프로세스가 사라진 뒤에는 `/debug/stats` 를 못 읽는다 — server 제안), **그 tick 의 `MINERAL_MINED` 가 DB 에 없다** | qa 절차: 새 월드 → `bots probe --case cas-halt`(채굴 1 회 → psql 로 커밋 확인 → qa 가 `UPDATE inventory ...` → 채굴) → 종료 코드·로그 수집. server 가 같은 경로의 DB 통합 테스트(`cargo test -p starfall-persistence cas_mismatch_halts` [DB] — `Fatal` 반환 · run 루프 종료 · fatal 신호 발화 · `persist_fatal_total == 1` · 그 tick 이벤트 0 행). **실서버 fatal 로그 한 줄의 필드**: 사유, 키(`actor_id`, `mineral_id`), 기대값, DB 값, `persist_fatal_total` | **서버가 다른 이유로 죽음** → 변조 **전에** 같은 actor 의 채굴이 수락·커밋됐음(SQL 로 행 확인)과 종료 로그의 사유 문자열(비교 후 쓰기)을 단언. **변조 없는 대조 실행**에서 같은 절차가 정지하지 않음 | server · qa | AC-5(b), I-56 | E2 E5 W |
| SC-26 | 재기동하면 DB 값(변조된 값)으로 적재되고 다음 채굴이 **그 위에서** 수락된다(DB 가 정본) | SC-25 직후 같은 월드로 재기동 → `bots probe --case cas-reload` 로 채굴 → psql 로 새 `MINERAL_MINED.quantity_before_kg` = 변조 값. DB 층(server): `cargo test -p starfall-persistence restart_reloads_economic_state_so_cas_succeeds_and_duplicates_are_rejected` [DB] · 적재 함수 `cargo test -p starfall-persistence load_economic_state_matches_the_database` [DB] · 빈 월드 `cargo test -p starfall-persistence load_economic_state_on_empty_world_returns_empty_lists` [DB] | **변조 값이 원래 값과 같음** → 변조 값 ≠ 정지 전 메모리 값임을 SC-25 증거에서 인용 | server · qa | AC-5(c) | E2 E5 W |
| SC-27 | `recorded_at` 생성 실패 주입 시 이벤트를 **건너뛰지 않고** 배치가 실패 → 정지 경로(ADR-0013 §5 가 고치라고 한 `continue`) | `cargo test -p starfall-persistence recorded_at_failure_fails_batch` [DB] — **시계 주입점을 둔 DB 통합 테스트**(실서버는 호스트 시계라 주입 불가 — server) | **주입 지점이 실행되지 않음** → 주입 카운터 ≥ 1, 그리고 배치 실패 뒤 그 tick 의 이벤트·상태 행이 **둘 다 0**(한쪽만 커밋되지 않았음) | server | AC-5(d), ADR-0013 §5 | E1 E2 |
| SC-101 | **payload 직렬화 실패** 주입 시 JSONB `null` 을 저장하지 않고 배치가 실패 → 정지(현재 `extract_payload` 의 `null` 경로 — server 발견) | `cargo test -p starfall-persistence payload_serialize_failure_fails_batch` [DB](payload 생성 함수의 주입점 — 계약 타입 직렬화는 사실상 실패하지 않으므로) + `payload_null_rejected_by_db` [DB](JSONB `null` INSERT → 23514, 양성 대조 `object` 통과) + 부정 짝 `grep -rn "unwrap_or(Value::Null)\|unwrap_or(serde_json::Value::Null)" server/crates/persistence/src` → 0 | **주입 지점 미실행** → 주입 카운터 ≥ 1, 그 tick 의 이벤트·상태 행 둘 다 0. grep 0 은 부정만 증명한다(규칙 9) — 앞의 두 DB 테스트와 짝으로만 PASS | server | AC-5(d), ADR-0013 §5 | E1 E2 |
| SC-102 | 정지가 결정된 뒤 **종료 스윕의 `SESSION_CLOSED` 가 커밋되지 않는다**(정지 후 배치 커밋 금지, K5) | `cargo test -p starfall-persistence no_commit_after_fatal` [DB] — fatal 뒤에 배치 2개(일반 1 + `SESSION_CLOSED` ≥ 1 을 담은 스윕 모양 1)를 넣고 행 변화 0. **스윕이 실제로 돌았다는 증거**는 SC-25 실서버 실행의 기존 로그 `tick 루프 종료 — SERVER_SHUTDOWN 스윕 후 영속화 flush` 의 `sessions_closed=N`(≥ 1) + 정지 tick 이후 그 월드 `domain_events` 0 행(psql) — 둘이 짝 | **fatal 이 실제로 안 났음** → `Fatal` 반환과 `persist_fatal_total == 1` 을 먼저 단언. **스윕이 만들 것이 없었음** → 실서버 로그 `sessions_closed ≥ 1`(persistence 단위 테스트는 게이트웨이 런타임의 스윕을 볼 수 없다 — server) | server | AC-5(e), ADR-0013 §5 K5 | E1 E2 |
| SC-103 | **일시 실패 분류 (a) 단위**: SQLSTATE `57P01` 을 주입하면 **재시도**이고 정지가 아니다. 짝: 허용 목록 밖 SQLSTATE(`22P02`)는 정지 — 같은 테스트 | `cargo test -p starfall-persistence admin_shutdown_is_transient` — `DatabaseError` 를 구현한 가짜 오류, 재시도 루프 commit 주입점으로 첫 시도만 실패 | **주입이 안 걸려 재시도할 일이 없음** → 57P01 주입 횟수 ≥ 1, 재시도 뒤 같은 배치 커밋, `persist_fatal_total` = 0. 짝의 22P02 에서 정지 관측("무엇이든 재시도" 배제) | server | AC-5(f), ADR-0013 §5 K4 | E1 |
| SC-112 | **일시 실패 분류 (b) 실제 DB**: `pg_terminate_backend` 로 영속화 연결을 끊는다 → 재시도, 같은 배치 커밋, 정지 없음. sqlx 가 받는 오류가 `57P01`·`Io` 중 무엇일지는 타이밍이 정한다(server) — 어느 쪽이든 허용 목록 안이어야 한다 | `cargo test -p starfall-persistence terminated_backend_is_retried` [DB] | **끊기가 커밋과 안 겹쳐 오류 0** → 관측한 일시 오류 ≥ 1 과 그 **코드**를 찍는다. 0 이면 무효 | server | AC-5(f) | E1 E2 |

### F. 기록 지연 거부 (AC-6) — server · qa

| ID | 검증 항목 | 검증 방법 | ⊘ 자명 통과 → 배제 단언 | 담당 | 근거 | 불가 |
|----|----------|----------|------------------------|------|------|------|
| SC-28 | 게이트웨이 단위: **`recording_lag`**(넣었지만 미커밋 tick, ADR-0013 §6 K2) **21** tick 에서 `MINE_RESOURCE` → `RECORDING_BACKLOG`, 같은 상태 `SET_SHIP_CONTROL` → 수락. **20** tick 에서 `MINE_RESOURCE` → 게이트웨이 통과. `persist_backlog` = 20 이지만 `recording_lag` = 0 인 상태(하트비트 톱니 꼭대기)에서 `MINE_RESOURCE` → **통과**. 그리고 **영속화 halted(fatal) 상태**에서 `MINE_RESOURCE` → `RECORDING_BACKLOG`, `SET_SHIP_CONTROL` → 수락(K5) | `cargo test -p starfall-gateway recording_lag_boundary_*` (lag 21 거부 · 같은 상태 `SET_SHIP_CONTROL` 수락 · halted 거부·이동 수락) + `cargo test -p starfall-gateway mine_resource_is_not_rejected_with_recording_backlog_when_lag_is_within_the_limit` (lag 20 통과) + `cargo test -p starfall-gateway mine_resource_passes_when_persist_backlog_is_high_but_recording_lag_is_zero` (persist 20 · lag 0 통과) | **임계가 반대 방향으로 구현돼도 한쪽만 시험하면 통과** → 20/21 경계 **양쪽**. **옛 지표로 판정해도 통과하는 입력만 쓰면 K2 를 못 가른다** → 두 지표가 갈리는 입력(persist 20 · lag 0)을 반드시 포함 | server | AC-6, I-50 0b행, ADR-0013 §5 K5 | E1 |
| SC-29 | 실서버: 봇이 채굴·이동 중 `docker compose stop postgres` → `recording_lag > 20` 이 된 뒤의 `MINE_RESOURCE` 는 `RECORDING_BACKLOG` | `bots probe --case backlog` + qa 가 `stop`/`start` 를 절차서대로 | **DB 정지 동안 채굴 명령을 한 번도 안 보냄** → `RECORDING_BACKLOG` 거부 수 ≥ 1 **과** 정지 **전** 수락 수 ≥ 1 을 함께 찍는다. 거부 시점의 `recording_lag` 값(> 20)을 `/debug/stats` 로 | qa | AC-6(a) | E2 E5 E6 |
| SC-30 | 같은 구간 `SET_SHIP_CONTROL` 은 수락되고 스냅샷이 흐른다 | `bots probe --case backlog` 같은 실행의 봇 로그: 구간 내 `SET_SHIP_CONTROL` 수락 수, 받은 `WORLD_SNAPSHOT` 수 | **구간 동안 이동 명령을 안 보냄 / 구간이 0 초** → 구간 길이(초)와 그 안의 수락 ≥ 1, 스냅샷 ≥ 1 | qa | AC-6(b), p1-01 I-31 | E2 E5 E6 |
| SC-31 | `docker compose start postgres` 후 백로그가 빠지고 채굴이 **다시 수락**된다 | `bots probe --case backlog` 같은 실행: 재개 후 `recording_lag`(`/debug/stats`)가 20 이하로 내려간 시점 이후 수락 ≥ 1 | **재개 후 채굴을 안 보냄** → 재개 뒤 수락 수 ≥ 1 과 그 시점 `persist_backlog` 값 | qa | AC-6(c) | E2 E5 E6 |
| SC-32 | 정지 전·중에 **수락된** 채굴이 **전부** DB 에 있고 항등식(SC-23·24)이 성립 | 봇이 기록한 수락 `command_id` 집합 ⊆ `processed_commands` 와 `MINERAL_MINED.causation_id` 를 `mining_ledger.py --world <id> accepted --from <봇 로그>` 로 | **수락 집합이 비어 있음** → 봇 수락 수(정지 중 수락 ≥ 1 포함)를 찍고, 누락 0 을 판정 | qa | AC-6(d), I-53 | E2 E5 E6 W |
| SC-104 | **DB 가 살아 있는 한가한 서버에서 60 초 동안 `RECORDING_BACKLOG` 0 건**(하트비트 톱니에 걸리지 않는다 — K2 의 대조) | `bots probe --case backlog-idle`: 봇 1대가 쿨다운마다 채굴만 한다(이동 없음, 다른 연결 없음) | **채굴을 안 보냄 / 톱니가 실제로 안 생김** → 보낸 `MINE_RESOURCE` ≥ 15, 그리고 **같은 구간 `persist_backlog` 최대 ≥ 20**(`/debug/stats` 폴링 — 옛 지표였다면 거부됐을 조건이 실제로 있었다). 톱니 최대 < 20 이면 **무효** | qa | AC-6(a2), ADR-0013 §6 K2 | E2 E5 E6 |

### G. 결정성 (AC-7) — server

| ID | 검증 항목 | 검증 방법 | ⊘ 자명 통과 → 배제 단언 | 담당 | 근거 | 불가 |
|----|----------|----------|------------------------|------|------|------|
| SC-33 | 채굴을 포함한 기록된 입력열(함선 2척, 광맥 2곳, 쿨다운·사거리 밖·속도 초과·부분 산출·소진·회복 경계 포함. **`CAPACITY_EXCEEDED` 는 제외** — 2^31 kg 은 재생으로 도달하지 않는다. 그 경로의 증거는 SC-09)을 **서로 다른 프로세스에서 2회** 재생 → 매 tick 의 이벤트 내용(`event_id`·`recorded_at` 제외)과 스냅샷·`DEPOSIT_FIELD_STATE`·`INVENTORY_STATE` payload 가 **바이트 동일** | `cargo test -p starfall-sim --test mining_replay` (자식 프로세스 2개, p1-01 AC-8 방식) | **입력열이 주장한 조건을 실제로 안 만듦** → 재생 출력에서 사유별 거부 수(쿨다운·사거리·속도·소진 각 ≥ 1), 부분 산출 ≥ 1, 회복 경계 통과 ≥ 1, **비교한 `MINERAL_MINED` 수 ≥ 1** 을 찍는다. 음성 대조: 한 프로세스의 입력 한 줄을 바꾸면 불일치 | server | AC-7(a), I-59 | E1 |
| SC-34 | **기존 이동 golden 이 재생성 없이 통과**하고 세 파일 sha256 이 §0.7 기준선과 같다 | `cargo test -p starfall-sim` 의 이동 재생 테스트 + `sha256sum server/crates/sim/tests/data/replay/*` 를 작업 후 다시 | **`STARFALL_REPLAY_BLESS=1` 로 다시 만듦**(p1-01 §0.12) → 판정은 테스트 통과가 아니라 **해시 불변**이다. `git status --porcelain server/crates/sim/tests/data/replay/` 공백도 함께 | server | AC-7(b), CLAUDE.md | E1 |

### H. 계약 ↔ Rust (AC-8) — server

| ID | 검증 항목 | 검증 방법 | ⊘ 자명 통과 → 배제 단언 | 담당 | 근거 | 불가 |
|----|----------|----------|------------------------|------|------|------|
| SC-35 | `cargo test -p starfall-contracts --locked` 가 **타입 23 전부**를 덮는다: 유효 fixture **46** 왕복, 반례 **74** 스키마 거부(그중 `MINERAL_MINED/invalid/quantity-zero.json` — H-13 의 대체 확인), 스키마 **29**, 레지스트리 23 | 그 명령. 테스트가 **검사 건수를 출력**한다 | **하드코딩 기대값만 바꾸고 순회는 그대로** → 테스트가 `find` 결과로 센 파일 수와 하드코딩 수를 **둘 다** 찍고 같음을 단언. 기준선 RED 5건(§0.7)이 전부 초록으로 바뀐 것을 테스트 이름으로 | server | AC-8 | E1 |
| SC-36 | 신규 반례 40건의 serde 결과가 §0.5 표 ② 열과 일치 | `cargo test -p starfall-contracts invalid_fixtures_layered` — 반례별 결과를 표로 출력 | **표가 비어 있어 비교 0 건** → §0.5 표가 채워지기 전에는 **대기**. 비교한 반례 수 40 을 찍는다 | server | AC-8 | E1 E9 |
| SC-37 | 신규 10 타입의 `required` 필드를 하나씩 제거한 변이가 **실패**한다 | `cargo test -p starfall-contracts required_removal_mutations` — "required + nullable" 필드(`DEPOSIT_FIELD_STATE` 의 넷 등)는 기존 `required_nullable` `deserialize_with` 로 **키가 빠지면 실패**하게 한다(serde `Option` 은 빠진 키를 `None` 으로 받아 변이가 공짜로 통과한다 — server) | **변이가 0 개 생성됨** → 타입별 변이 수와 실패 수를 찍고 둘이 같음·합 ≥ 10 을 단언 | server | AC-8 | E1 |
| SC-38 | 레지스트리 `responses` 가 명령 3종(`PING_SERVER`·`SET_SHIP_CONTROL`·`MINE_RESOURCE`)에 있고 스펙 §5.1a 표와 같다 | `cargo test -p starfall-contracts registry_responses` | **`responses` 가 없는 명령을 "선택 필드라 없음" 으로 통과** → 명령 kind 인 타입 수(3)와 `responses` 가 있는 수(3)를 둘 다 찍는다 | server | AC-8, §5.1a | E1 |

### I. 누출 없음 (AC-9) — server · qa

| ID | 검증 항목 | 검증 방법 | ⊘ 자명 통과 → 배제 단언 | 담당 | 근거 | 불가 |
|----|----------|----------|------------------------|------|------|------|
| SC-39 | sim 이 만드는 `DEPOSIT_FIELD_STATE`: 미확인 광맥은 `mineral_id`·`initial_reserve_kg`·`remaining_kg`·`first_extracted_tick` **넷 다 null**, 드러난 광맥은 **넷 다 비-null**, 회복으로 잔량이 초기값에 돌아와도 **드러난 채로** | `cargo test -p starfall-sim mining::deposit_field_state_reveal` | **전부 미확인 상태만 시험** → 같은 테스트에 미확인 ≥ 1 과 드러남 ≥ 1 이 **공존**하는 메시지, 그리고 잔량 = 초기값인 드러난 광맥 ≥ 1 | server | AC-9, I-68 | E1 |
| SC-40 | 실서버·새 월드: 첫 채굴 **전** 세션이 받은 **모든** 메시지에 미확인 광맥의 `mineral_id` 값·매장량이 없다. **양성 대조**: 첫 채굴 **뒤** 그 광맥의 `mineral_id` 가 실제로 나타난다 | `bots probe --case leak-scan` — 받은 원문 프레임 전부를 저장하고, 광맥 표의 8 `mineral_id` 문자열·매장량 수치를 원문에서 검색. **그리고 구조 검사**: 첫 채굴 전 `DEPOSIT_FIELD_STATE` 의 모든 항목이 네 필드 `null`(키는 있음) | **검색이 필드를 잘못 찾음 / 메시지를 하나도 안 받음** → 받은 프레임 수와 타입별 수를 찍고, 양성 대조에서 **검색이 ≥ 1 적중**함을 같은 실행에서 보인다(적중 0 이면 FAIL — 검출기 사망). **(r2 보강 — ADR-0006 §4a, 세션 시작 송신 확정)** 첫 채굴 전 구간에 `SESSION_READY` 직후의 `INVENTORY_STATE` ≥ 1 과 `DEPOSIT_FIELD_STATE` ≥ 1(광맥 8 항목)이 **실제로 있음**을 먼저 단언한다 — 누출이 가장 날 만한 두 메시지를 안 받은 실행은 **무효** | qa | AC-9, S-9 | E2 E5 E6 W |

### J. 역사 판정 코어 (AC-10) — history

DB 없이. H-01·H-03·H-04·H-08·H-09·H-13 의 세부 절차. **(a)(c) 가 이 절 전체의 양성 대조**다.

| ID | 검증 항목 | 검증 방법 | ⊘ 자명 통과 → 배제 단언 | 담당 | 근거 | 불가 |
|----|----------|----------|------------------------|------|------|------|
| SC-41 | 첫 채굴 → 기록 **1건**: Level 2, `rule_version = mineral-discovery@1`, 근거 = 그 이벤트, 참가자 = actor(PLAYER/DISCOVERER)·ship(SHIP/VESSEL) **이 순서로**(jsonb 배열 동등은 순서를 본다 — 어긋나면 재배달이 거짓 충돌로 러너를 멈춘다, SC-106), `historical_event_id` = fixture `MINERAL_DISCOVERED/starfall-glass.json` 의 UUIDv5. **증거 1건**: `evidence_id = UUIDv5(NS_EVIDENCE, "{historical_event_id}\|SHIP_LOG")`, `SHIP_LOG`, `PARTICIPANTS_ONLY` | `cargo test -p starfall-history first_extraction_discovers` (입력 `MINERAL_MINED/first-extraction.json`, 기대 `evidence_id` 는 `server/crates/history/tests/data/` 의 파일 — history 가 Python `uuid5` 로 따로 계산한 `e9b611d1-637e-5287-862e-74eabffb0252`) | **판정기가 아무것도 안 냄** → 기록 수 = 1 을 단언(≥ 0 이 아니다). id 는 fixture·테스트 데이터 파일에서 읽고 코드에 복사하지 않는다. **기대 id 를 코드와 같은 함수로 계산하면 자기 일치다** → 독립 계산값(Python)을 쓴다 | history | AC-10(a) | E1 |
| SC-42 | 같은 광물 두 번째 채굴 → **0 건** | `cargo test -p starfall-history second_extraction_is_silent` | **첫 채굴부터 0 건이라 둘째도 0** → **같은 테스트에서 첫 채굴의 1 건을 먼저 단언**. 처리한 `MINERAL_MINED` 수(분모 2)를 찍는다 | history | AC-10(b) | E1 |
| SC-43 | 다른 광물 → **1 건** 추가(총 2) | `cargo test -p starfall-history other_mineral_discovers` | 같다 — 기록 수가 1 → 2 로 **변함**을 단언 | history | AC-10(c) | E1 |
| SC-44 | 같은 tick 두 actor → `sequence` 작은 쪽이 단독 발견자(H-04). fixture 의 `event_id` 순서는 `sequence` 와 **반대** | `cargo test -p starfall-history same_tick_lower_sequence_wins` (+ `MINERAL_DISCOVERED/glacine-seq-nonzero.json`) | **`event_id` 순서와 `sequence` 순서가 같아 어느 쪽 규칙인지 구분 안 됨** → 두 이벤트의 tick 이 같음, `event_id` 순서 ≠ `sequence` 순서를 입력 단계에서 단언 | history | AC-10(d), H-04 | E1 |
| SC-45 | 멱등: 같은 입력을 k ∈ {2, 3, 5} 회 처리 → 1 회와 같은 결과(기록·증거·근거·참가자 수 1·1·1·2) (H-01) | `cargo test -p starfall-history reprocessing_is_idempotent` | **첫 처리부터 0 건** → 첫 처리 직후 기록 = 1 을 단언 | history | AC-10(e), H-01 | E1 |
| SC-46 | 배치 경계 불변: 정렬된 입력을 **무작위 지점**에서 잘라 처리 → 같은 결과. **시드를 출력**한다 | `cargo test -p starfall-history batch_boundary_invariance` | **자른 지점이 발견 이벤트와 무관한 곳뿐** → 자른 지점 수와, 발견 이벤트 **바로 앞·바로 뒤** 에서 자른 경우 ≥ 1 씩을 찍는다. 입력에 발견 ≥ 2 | history | AC-10(f) | E1 |
| SC-47 | **러너 정렬 경로**: 같은 채굴 로그(광물 ≥ 2 × 채굴자 ≥ 3, 같은 tick 충돌 포함)를 새 테스트 DB 마다 `domain_events` 에 **삽입 순서만 섞어** 넣고(순열 ≥ 20) 러너가 만든 발견 집합이 오라클과 같다(H-03, S-2 결정성 보강). **순열 수를 출력**한다 | `cargo test -p starfall-persistence history_runner_ordering_permutations` [DB] — history r0 검토: 코어에 **정렬한 입력**을 넣는 형태는 `(tick, sequence)` 가 유일해 정렬 후 입력이 바이트 동일해지고 정렬하는 쪽이 테스트 자신이라 **코드를 시험하지 않는다**(자명 통과). 실제 정렬 경로는 러너의 SQL `ORDER BY` 다 | ⊘ 셋: (i) **argmin 이벤트가 마지막에 삽입된 순열 ≥ 1** (ii) **`ORDER BY` 없는 SELECT 결과가 정렬 순서와 다른 순열 ≥ 1** — 물리 순서가 실제로 달라야 `ORDER BY` 가 일을 한 것이다 (iii) **`event_id` 순서 ≠ `(tick, sequence)` 순서인 쌍 ≥ 1** | history | AC-10(g), H-03 | E1 E2 |
| SC-48 | **비정렬 입력은 거부**된다(역행 입력 — ADR-0014 §1) | `cargo test -p starfall-history out_of_order_input_rejected` | **거부 대신 조용히 정렬해 통과** → 결과가 `Err` 이고 판정 상태가 **바뀌지 않았음**(이후 정상 입력의 결과가 역행 입력을 안 본 것과 같음)을 단언 | history | AC-10(g) | E1 |
| SC-49 | Level 0 타입만의 입력(`SESSION_*`·`SHIP_*`) → **0 건**. 채굴 N 건·서로 다른 (성계, 광물) K 개(K ≥ 2, N ≥ 3K) 섞은 입력 → 기록 = K (H-08) | `cargo test -p starfall-history level0_filter` | **입력이 비어 0 건** → 입력 수(타입별)를 찍고 `SESSION_*`·`SHIP_*` ≥ 1, N·K 를 출력 | history | AC-10(h), H-08 | E1 |
| SC-50 | golden: `rule_version` 에 규칙 파일 **정규화 해시**와 고정 입력의 출력이 묶여 있다(`server/crates/history/tests/data/golden/`). **값만 바꾼 규칙 파일이 실제로 실패한다** | `cargo test -p starfall-history rule_golden` + 음성 대조 테스트 `rule_golden_detects_value_change_without_version_bump` | **golden 을 `STARFALL_REPLAY_BLESS=1` 로 다시 만듦 / 음성 대조가 시험되지 않음** → 음성 대조가 **같은 실행에서 실패를 관측**(`should_panic` 이 아니라 결과 `Err` 단언). golden 파일 sha256 과 **실제 `data/history/rules/mineral-discovery.json` sha256 을 테스트 전후** 증거에(음성 대조가 쓰는 "값만 바꾼 규칙 파일" 은 임시 사본이어야 한다) | history | AC-10(i), H-09 | E1 |

### K. 역사 DB 통합 (AC-11) — history · 새 월드

| ID | 검증 항목 | 검증 방법 | ⊘ 자명 통과 → 배제 단언 | 담당 | 근거 | 불가 |
|----|----------|----------|------------------------|------|------|------|
| SC-51 | 첫 채굴 커밋 뒤 `historical_events`·`historical_event_sources`·`evidence` **각 1행**, 커서 전진, `source_event_ids` 가 `domain_events` 에 존재. 증거 행: `SHIP_LOG` · `PARTICIPANTS_ONLY` · `derived_from_evidence_ids = '{}'`(원본) · authenticity `VERIFIED` · `source_entity_id = ship_id` · creation_method `automatic` — **원본 증거 표지는 첫 행 뒤에 되돌릴 수 없는 모양이라 지금 확인한다**(history) | `cargo test -p starfall-persistence history_first_discovery_rows` [DB] | **테스트 월드에 이미 기록이 있어 "1행" 이 다른 실행의 것** → 시작 시 세 테이블의 그 월드 행 수 = 0 을 단언 | history | AC-11(a), H-02 | E1 E2 |
| SC-52 | 역사 3 테이블에 UPDATE / DELETE → **전부 거부**(6 시도) | `cargo test -p starfall-persistence history_tables_append_only` [DB] | **0 행 매치라 행 단위 트리거가 발화하지 않음** → 각 시도 직전 대상 행 존재(`count ≥ 1`)를 단언하고, 시도 후 행 수·내용 불변. **거부가 트리거 때문인가**(오타·권한·잠금도 에러를 낸다 — history) → 각 에러의 SQLSTATE = **`P0001`** 이고 메시지에 **대상 표 이름**이 들어 있음(0003 은 `TG_TABLE_NAME` 을 쓰는 역사 전용 함수 — 0001 의 `forbid_mutation()` 은 `domain_events` 를 하드코딩했다). **양성 대조**: 가변 표 `history_cursor` 의 UPDATE 는 **성공**(트리거가 "모든 UPDATE 를 막는 버그" 가 아님) | history | AC-11(b), H-07, I-63 | E1 E2 |
| SC-53 | 같은 `dedupe_key` 로 두 번째 행을 **SQL 로 직접** INSERT → UNIQUE 위반 | `cargo test -p starfall-persistence history_dedupe_unique` [DB] | **첫 행이 없어 둘째가 첫째가 됨** → 첫 행 존재 단언 후, SQLSTATE 23505 를 관측 | history | AC-11(c), I-61 | E1 E2 |
| SC-54 | 커밋 직전 장애 주입 → 재시작 → 기록 1, 커서가 그 이벤트 뒤 (H-02) | `cargo test -p starfall-persistence history_crash_before_commit` [DB] | **주입 지점이 실행 안 됨** → 주입 카운터 ≥ 1, **장애 직후 기록 = 0**(주입이 커밋 전이었다) | history | AC-11(d), H-02 | E1 E2 |
| SC-55 | 판정 불가 `MINERAL_MINED` 를 발견 후보 **앞에** 끼움 → 러너 정지, 커서가 그 앞, 뒤 후보로 기록 없음, `history_detector_halted` +1, **같은 시간 채굴은 계속 수락·커밋**(I-67) | `cargo test -p starfall-persistence history_fail_stop` [DB] | **뒤에 발견 후보가 없음** → 뒤 후보 이벤트 존재를 단언. 정지 **후** 커밋된 `MINERAL_MINED` ≥ 1(경제가 돈다) | history | AC-11(e), H-10 | E1 E2 |
| SC-56 | `last_tick` 보다 큰 tick 의 행은 판정되지 않는다(워터마크) | `cargo test -p starfall-persistence history_watermark` [DB] | **그런 행이 없음** → `tick > last_tick` 인 발견 후보 행 ≥ 1 을 단언 | history | AC-11(f), H-11 | E1 E2 |
| SC-57 | 재기동: 발견 2건이 있는 월드에서 정상 종료·재기동 → 기록 여전히 **2**, 재기동 뒤 첫 신규 광물 채굴이 **1건 추가**(3) (S-7, H-15) | `cargo test -p starfall-persistence history_restart_rebuild` [DB] | **재기동 뒤 판정기가 죽어 "2 유지" 가 공짜** → 재기동 뒤 **3 이 됨**이 이 항목의 양성 대조. 재기동 전 기록 = 2 도 단언 | history | AC-11(g), S-7, H-15 | E1 E2 |
| SC-105 | **AC-11(h)(ii) 다른 근거 충돌 = 러너 정지**: 판정 상태를 비운 러너가 두 번째 채굴을 "최초" 로 판정하게 주입 → `ON CONFLICT` 0 행 → 기존 기록과 **내용 전체**(`recorded_at` 만 제외, jsonb 동등) 비교가 다름 → 롤백, `history_conflicts_total ≥ 1`, **러너만** 정지, 커서가 그 이벤트 앞, 역사 행 수 불변, **같은 시간 채굴은 계속 수락·커밋** | `cargo test -p starfall-persistence history_conflict_halts_runner_only` [DB] — SC-106·SC-111 과 **한 테스트 파일, 같은 실행** | **주입 지점 미실행** → 주입 카운터 ≥ 1. **이미 1 행이 있는 키**여야 충돌이다 → 주입 전 그 키의 행 = 1. UNIQUE 충돌이 실제로 났음(23505 관측 ≥ 1 — history). 정지 **후** 커밋된 `MINERAL_MINED` ≥ 1. 주입 없는 실행의 카운터 0 은 SC-106 이 준다 | history | AC-11(h)(ii), ADR-0014 §4, I-67 | E1 E2 |
| SC-106 | **AC-11(h)(i) 재배달 = 조용함**(SC-105·111 의 양성 대조 — "무조건 멈추는 버그" 배제): `history_cursor` 를 UPDATE 로 되감아 같은 근거·같은 내용을 재처리 → 정지 없음, `history_conflicts_total` 증가 0, 역사·증거 행 수 불변 | `cargo test -p starfall-persistence history_redelivery_is_silent` [DB] — SC-105·111 과 같은 실행 | **재처리가 실제로 일어나지 않음**(커서가 이미 지나 읽지 않음) → 되감기 실행 ≥ 1, **재처리된 이벤트 수 ≥ 1** 을 단언. 이 짝이 없으면 "무엇이든 멈춘다" 와 "충돌만 멈춘다" 가 구분되지 않는다 | history | AC-11(h)(i) | E1 E2 |
| SC-111 | **AC-11(h)(iii) 같은 근거·다른 내용 = 러너 정지**: 러너가 처리하기 **전에** 같은 id·`dedupe_key`·근거에 **payload 만 다른** 행을 SQL 로 INSERT 해 둔다(추가 전용 트리거는 INSERT 를 막지 않는다) → 처리 시 내용 비교가 다름 → 정지, `history_conflicts_total ≥ 1`, 커서 유지. golden 이 놓친 규칙 변경을 잡는 대조다(architect) — SC-105 와 성질이 달라 verdict 를 가른다(규칙 5) | `cargo test -p starfall-persistence history_same_source_different_content_halts` [DB] — SC-105·106 과 같은 실행 | **주입 행이 없음 / 주입 행이 러너 출력과 다른 곳이 payload 가 아님**(그러면 다른 근거 충돌 = SC-105 를 재는 것) → 주입 행 존재와, 러너 출력과의 차이가 **payload 뿐**임을 먼저 단언 | history | AC-11(h)(iii), ADR-0014 §4 | E1 E2 |

### L. 오라클 교차와 재생 결정성 (AC-12) — qa · history

| ID | 검증 항목 | 검증 방법 | ⊘ 자명 통과 → 배제 단언 | 담당 | 근거 | 불가 |
|----|----------|----------|------------------------|------|------|------|
| SC-58 | 실서버 + 봇 채굴 세션 뒤 SQL 오라클(§0.10)과 역사의 발견 집합이 **양방향 차집합 0** (H-12) | `python tests/e2e/history_oracle.py --world <id>` | **오라클 행 수 ≥ 2 가 아니면 FAIL.** 오라클 행 수·역사 행 수·차집합 두 방향 크기를 찍는다. §0.10 의 NULL 필드 양성 대조 | qa | AC-12(a), H-12 | E2 E5 E6 W |
| SC-59 | 그 월드의 `domain_events` 를 **읽기 전용**으로 빈 판정 상태 코어에 재생 → 저장된 기록과 **evidence 행**이 **id 까지** 같다(`recorded_at` 등 시각 제외) (H-05·H-06). 증거까지 비교해야 "역사 이벤트는 같고 증거 id 만 달라지는" 회귀를 잡는다(history) | `cargo run -p starfall-persistence --bin history-replay -- --world <id> --read-only` (재생 도구, H2) | **재생 대상 0 건** → 재생한 이벤트 수와 비교한 기록 수(≥ 2)를 찍는다. **주 DB 의 역사 테이블을 비우지 않는다** — 비워야 하면 복사한 로그로 별도 스키마/DB | history · qa | AC-12(b), H-05, H-06 | E1 E2 W |

### M. 전달 (AC-13) — server · history · qa

| ID | 검증 항목 | 검증 방법 | ⊘ 자명 통과 → 배제 단언 | 담당 | 근거 | 불가 |
|----|----------|----------|------------------------|------|------|------|
| SC-60 | 러너 커밋 뒤 **열린 모든 세션**이 `LIVE` 1건을 받는다 | `bots probe --case trace-abc` 의 (a) 단계 — 열린 세션 수와 받은 세션 수 | **세션이 1 개뿐** → 열린 세션 ≥ 2, 받은 수 = 열린 수 | qa | AC-13(a), I-65 | E2 E5 E6 W |
| SC-61 | 이후 접속한 세션이 `SESSION_READY` **뒤** `BACKFILL` 로 받는다 | `bots probe --case trace-c` 의 수신 순서 (`trace-abc` 실행의 (c) 단계) | **BACKFILL 이 READY 앞에 옴 / 기록이 없는 월드** → 수신 순서 인덱스(READY < BACKFILL)와 BACKFILL 건수 ≥ 1 | qa | AC-13(b) | E2 E5 E6 W |
| SC-62 | **커밋 전 송신이 없다**: 러너 커밋을 실패시키는 주입에서 NOTICE **0** 건 | `cargo test -p starfall-persistence history_notice_after_commit_only` [DB] (러너 → 채널 경계) | **주입 없는 경우에도 0 건**(송신 경로 사망) → **같은 테스트에서 주입 없는 경우 1 건을 먼저 단언** | history | AC-13(c), I-65 | E1 E2 |
| SC-63 | **누락 틈 없음**: 커밋과 세션 열림이 겹치게 만든 반복에서 LIVE·BACKFILL 을 **둘 다 못 받은 세션 0** | server: `cargo test -p starfall-gateway notice_no_gap_same_tick`(겹침 tick 을 강제로 만들고 새 세션이 그 기록을 LIVE 로 **정확히 1회** 받음 + 카운터 1). qa: `bots probe --case notice-gap` 반복 + `/debug/stats` `notice_open_overlap_ticks_total` | **겹침이 실제로 한 번도 안 일어남** → **겹침이 일어난 반복 수 ≥ 1** 을 찍는다(겹침 정의 — server T0: **같은 tick 에 새로 ready 가 된 세션 ≥ 1 과 LIVE 수신 ≥ 1 이 함께 있다**. 그 tick 안의 순서는 BACKFILL 송신 → LIVE 목록 추가·브로드캐스트). 봇 반복에서 `notice_open_overlap_ticks_total` 델타 0 이면 **무효** | server · qa | AC-13(d), ADR-0014 §6 | E1 E2 E6 |
| SC-64 | `MINERAL_MINED` 커밋 → LIVE 수신 지연 p50/p95/max. **max ≤ 5 초**(판정). p95 ≤ 1 초는 **기록**(M-2) | `bots probe --case trace-abc` + `bots run --stage mine-load` 에서 커밋 시각(psql `recorded_at`)과 봇 수신 시각 | **러너 첫 기동 따라잡기를 지연으로 오독** (스펙 §10-10) → 따라잡기 완료 로그(읽은 행 수) **뒤**의 표본만. 표본 수 ≥ 4(광물 4종) 를 찍는다 | qa | AC-13(e), ADR-0014 §7 | E2 E5 E6 W |

### N. 클라이언트 (AC-14, AC-19 c3) — client

| ID | 검증 항목 | 검증 방법 | ⊘ 자명 통과 → 배제 단언 | 담당 | 근거 | 불가 |
|----|----------|----------|------------------------|------|------|------|
| SC-65 | 생성기: 신규 DTO **6** 생성(`MineResourceCommand`·`MineralMinedEvent`·`MineralDiscoveredEvent`·`InventoryStateMessage`·`DepositFieldStateMessage`·`HistoricalEventNoticeMessage`), **2회 실행 멱등**, 기존 10 DTO **바이트 동일**, `ContractTypes.cs` 만 변경 | `dotnet run ContractsCodegen.cs` 2회 + 파일별 sha256 전후 비교 | **생성기가 아무것도 안 씀** → 쓴 파일 수 17 과 신규 6 의 존재를 찍는다 | client | AC-14(a) | E4 |
| SC-66 | EditMode: 유효 fixture **발견 수·왕복 수를 상수로 단언**(예측 46 / 36 — 실측으로 확정). 테스트가 fixture 디렉터리를 순회한 수를 `TestContext.WriteLine` 으로 찍는다, `INVENTORY_STATE` 빈·다항목, `DEPOSIT_FIELD_STATE` 전부 미확인·혼합, NOTICE LIVE/BACKFILL 왕복 | `unity test client --mode EditMode` → `test-results.xml` 스위트별. 반례 40 건의 C# 3층 분류(거부·감지 불가·계층 없음)를 같은 테스트가 실측해 §0.5 ③ 열에 | **상수만 바꾸고 순회가 0**(p1-01 SC-41 의 교훈) → 테스트가 발견한 파일 수를 로그에 찍고, 상수와 **같음**을 단언 | client | AC-14(b) | E3 |
| SC-67 | `Runtime` 프로필에서 **모르는 값**에 죽지 않는다: `reason_code`·`delivery`·`visibility`·`entity_kind`·`role` 다섯 필드 각각 | `unity test client --mode EditMode` 의 `UnknownClosedValue_*` 5 케이스 | **모르는 값을 안 넣음** → 입력 JSON 에 목록 밖 값이 실제로 있음을 테스트가 단언하고, 그 값이 문자열 그대로 보존됨을 확인 | client | AC-14(b) | E3 |
| SC-68 | 사람이 그레이박스에서 디자인 §8 의 **여섯 요소**를 확인하고 스크린샷 — 특히 **산출 알림과 발견 배너가 다른 것으로 읽히는가**, 배너가 1 초 안에 오는 체감 | 도구 없음(사람 관찰) — 스크린샷 6장 + 한 줄 소감. 에이전트는 Play 를 누를 수 없다 | **스크린샷이 발견 없는 월드에서 찍힘** → 새 월드(§0.2)에서, 발견 배너 스크린샷에 광물 이름과 `Pilot-xxxx` 가 보일 것 | client(사람) | AC-14(c) | E3 W |
| SC-69 | 인벤토리 패널이 **서버 응답 뒤에만** 바뀐다(I-49) | EditMode `InventoryPanel_ChangesOnlyOnInventoryState` + 소스 검사 `grep -rnE "quantity_kg\s*[+\-]=\|QuantityKg\s*[+\-]=" client/Assets/_Project/Scripts` → 0 | **패널이 아무 입력에도 안 바뀜** → 같은 테스트에서 `INVENTORY_STATE` 수신 **후** 값이 바뀜을 단언(양성). 소스 검사는 부정만(규칙 9) — EditMode 와 짝으로만 PASS | client | AC-14(d), I-49 | E3 |
| SC-70 | 같은 기록을 LIVE·BACKFILL 로 **둘 다** 받아도 **한 번만** 표시 | `unity test client --mode PlayMode` 의 FakeTransport 시나리오 `DiscoveryFeed_DedupesByHistoricalEventId` — 같은 id 를 LIVE 뒤 BACKFILL 로 재생(fixture 둘은 id 가 달라 이 성질을 못 보인다, client 지적) | **두 메시지의 id 가 달라 원래 두 건** → 입력 두 메시지의 `historical_event_id` 가 같음을 단언, 표시 1. 음성 대조: id 가 다르면 2 | client | AC-14(e), I-65 | E3 |
| SC-110 | `Pilot-xxxx` 표지 = **`actor_id` 문자열 끝 4 글자**(I-66, architect 확정) — 그리고 **표지는 표시 전용**: 끝 4 글자가 같은 두 actor(개발 ID 에서 실제로 충돌 — Unity `…-000000000001` 과 bot-001 `…-000000000001`)가 각각 발견하면 **발견 목록에 서로 다른 발견자 둘**로 남는다 | EditMode `PilotLabel_LastFourChars` + `DiscoveryList_KeysByActorIdNotLabel`(충돌 쌍 입력) | **입력 두 actor 의 표지가 달라 합칠 기회가 없음** → 두 입력의 표지가 **같음**을 먼저 단언(음성 입력), 그 위에서 목록 항목 2 · `DiscovererActorId` 둘이 서로 다름. *(r2 정정, qa: 옛 대조 "같은 actor_id 둘은 1" 은 광물별 목록인 C3 설계에 맞지 않아 뺐다 — 같은 actor 가 두 광물을 발견해도 항목은 2 다)* | client | AC-14(c), I-66 | E3 |
| SC-71 | **광맥 표식의 광물 이름은 `DEPOSIT_FIELD_STATE` 에서만 온다** — 사본의 `mineral_id`·`initial_reserve_kg` 를 읽지 않는다. **두 방향**(client 제안): (A) 로컬 데이터 모델(`Scripts/Greybox/`)에 그 두 필드가 **아예 없다** — 읽을 수 없게 만든다 (B) 사본 값을 바꿔치기해도 표시가 메시지만 따른다 — 실제로 안 읽는다 | (A) EditMode 리플렉션 테스트 `DepositTableEntry_HasNoServerOnlyFields`(타입의 필드·프로퍼티 이름에 두 이름이 없음) (B) `unity test client --mode PlayMode` `DepositMarker_FollowsMessageNotDataCopy` — 바꾼 사본 + `all-unrevealed.json` → 미확인, 이어 `revealed-and-depleted.json` → 그 메시지의 광물 | (A) **리플렉션이 엉뚱한 타입을 봄** → 같은 테스트가 그 타입에 `position_m` 대응 필드가 **있음**을 먼저 단언(양성 대조). (B) **사본 값 = 메시지 값이라 출처 구분 불가** → 사본 값 ≠ 메시지 값인 입력을 쓰고 표식 = 메시지 값. **(A) 만으로는 다른 경로로 읽는 것을, (B) 만으로는 우연히 안 읽은 것을 못 가른다 — 둘 다 통과해야 PASS** | client | AC-14(f), I-68, 사용자 결정 Q3 | E3 |
| SC-72 | 클라이언트 사본 `client/Assets/_Project/Data` 가 `data/` 10 파일을 전부 포함하고 `ClientDataCopy_MatchesRepositoryOriginal` 초록. **사람이 돌린 EditMode(PlayMode 를 쓴 항목이 있으면 PlayMode 도) `test-results.xml` 을 증거로** — PR 병합 조건(§3.3) | `unity test client --mode EditMode` → 결과 파일을 `_workspace/p1-02-mining/evidence/editmode-<date>.xml` 로 | **테스트가 사본 파일 목록을 `data/` 가 아니라 사본에서 세어 둘 다 3 으로 일치** → 원본(`data/`)과 사본을 **각각 독립적으로 순회**해 두 수를 따로 찍고(원본 10) 같음을 단언(client Q-7 답) | client | AC-19(c3) | E3 |

### O. 치트 전수 (AC-15) — qa

**시도 수와 막은 수**를 항목마다 찍는다. 사유별 관찰 수가 0 인 항목은 그 경로를 안 탄 실행이다 → **무효**.

| ID | 검증 항목 | 검증 방법 | ⊘ 자명 통과 → 배제 단언 | 담당 | 근거 | 불가 |
|----|----------|----------|------------------------|------|------|------|
| SC-73 | `MINE_RESOURCE` 에 `quantity_kg`·`mineral_id`·위치·`actor_id` 주입 → **4 경우 전부** `MALFORMED_COMMAND`, 인벤토리 불변 | `bots probe --case cheat-mine-inject` | **명령이 서버에 도달하지 않음**(봇 직렬화가 필드를 버림) → 보낸 **원문 프레임**에 주입 필드가 있음을 봇 로그에 찍는다. 인벤토리 불변은 "주입 전 인벤토리 > 0" 에서(주입 전에 정상 채굴 1 회) | qa | AC-15(a), I-48 | E2 E5 E6 |
| SC-74 | 사거리 밖 반복 → `TARGET_OUT_OF_RANGE` | `bots probe --case cheat-mine-range` | **다른 사유로 거부**(쿨다운 등) → 사유 코드가 정확히 `TARGET_OUT_OF_RANGE`, 관찰 수 ≥ 1, 그 시점 서버 스냅샷 거리 > 한계 | qa | AC-15(b) | E2 E5 E6 |
| SC-75 | 속도 30 m/s 로 지나가며 → `SHIP_TOO_FAST` | `bots probe --case cheat-mine-fast` | **실제 속도가 한계 이하**(가속 중) → 명령 tick 의 서버 스냅샷 속도 > `max_ship_speed_mps` 를 찍는다. 사거리 **안**에서 보냈음도(아니면 사거리 사유가 먼저) | qa | AC-15(c) | E2 E5 E6 |
| SC-76 | 쿨다운보다 10 배 빠르게 60 초 → 수락 ≤ ⌈60 / 3⌉ + 1 = **21**, 나머지 `COOLDOWN_ACTIVE`, **연결 유지** | `bots probe --case cheat-mine-cooldown` | **전송이 레이트 리밋에 먼저 걸림 / 전송 수가 적음** → 보낸 수(≈ 200), `COOLDOWN_ACTIVE` 수 ≥ 1, `RATE_LIMITED` 0, 종료 시 연결 열림을 찍는다. 수락 ≥ 2(쿨다운 뒤 재수락이 실제로 됨) | qa | AC-15(d) | E2 E5 E6 |
| SC-77 | 없는 `deposit_id` → `TARGET_UNKNOWN` | `bots probe --case cheat-mine-unknown` | **스키마 위반 id 라 `MALFORMED_COMMAND`** → 스키마상 유효한 `DataId` 형식의 없는 id 를 쓴다. 사유 코드 정확히 `TARGET_UNKNOWN` | qa | AC-15(e) | E2 E5 E6 |
| SC-78 | AC-4 (a)~(c) 를 봇으로: 같은 세션 재전송 · 잔류 창 안 재접속 재전송 · **정상 종료 → 재기동 → 재전송** → 전부 `DUPLICATE_COMMAND_ID`, 인벤토리 산출 × 1, 재기동 경로는 `processed_commands` 행 수 불변 | `bots probe --case mine-dup` · `bots probe --case mine-dup-reconnect` · `bots probe --case mine-dup-restart` | **첫 전송이 거부됨** → 첫 전송 `ACCEPTED` 와 인벤토리 증가를 경로마다 먼저 단언. 재기동은 stdin `shutdown`(하드 킬 아님) | qa | AC-15(f), AC-4 | E2 E5 E6 |
| SC-108 | **클라이언트가 고를 수 있는 값으로 월드를 멈출 수 없다**(ADR-0013 §5a — ① 도달 "예" 1 건 `processed_commands_pkey`): 실서버에서 **다른 actor 둘이 같은 `command_id`** → 둘째 `DUPLICATE_COMMAND_ID`, **서버 프로세스 생존 + `persist_fatal_total` 0**. 누적 도달 1 건(`quantity_kg_check`)은 실서버로 만들 수 없어 SC-09 의 경계값 주입이 맡는다 | `bots probe --case mine-dup-cross-actor` + `/debug/stats` | **보내 봤는가** → 두 원문 프레임의 `command_id` 가 같고 actor 가 다름을 봇 로그에. **거절됐는가** → 둘째 사유 코드. **살아 있는가** → 10 초 뒤 `/debug/stats` 응답과 `last_tick` 전진(커밋이 계속됨) | qa | AC-4(d2), ADR-0013 §5a | E2 E5 E6 |
| SC-79 | 남의 인벤토리에 넣기는 **어휘에 없어 시도 불가** | `MINE_RESOURCE.schema.json` 의 `properties` 키 집합 = `{deposit_id}` 이고 `additionalProperties: false` 임을 `jq` 로 찍는다 + `bots probe --case cheat-mine-inject` 의 `actor_id` 주입 실행 결과(SC-73) | 스키마 읽기는 부정만 증명한다(규칙 9) — **SC-73 의 `actor_id` 주입 실행이 긍정 짝**이다. SC-73 이 PASS 가 아니면 이 항목도 PASS 가 아니다 | qa | AC-15(g), I-48 | E2 |

### P. "C 가 흔적을 본다" — 슬라이스의 핵심 (AC-16) — qa · 새 월드

| ID | 검증 항목 | 검증 방법 | ⊘ 자명 통과 → 배제 단언 | 담당 | 근거 | 불가 |
|----|----------|----------|------------------------|------|------|------|
| SC-80 | A·B 접속, A 가 광물 X 를 처음 캠 → **A·B 모두** `LIVE` 수신, 발견자 = A | `bots probe --case trace-abc` (a) 단계 | **이미 X 가 발견된 월드** → 새 월드(§0.2) + A 채굴 직전 그 월드 `historical_events` 0 행 | qa | AC-16(a), S-1 | E2 E5 E6 W |
| SC-81 | B 가 X 를 캠 → **수락**됐고 인벤토리가 늘었는데 새 역사 기록 **0** 건 | `trace-abc` (b) 단계 + SQL 행 수 | **B 의 채굴이 거부되어 역사가 없음** → B 의 `ACCEPTED` 와 인벤토리 증가를 단언. 처리된 X 의 `MINERAL_MINED` 수(분모 ≥ 2) | qa | AC-16(b), 원칙 4 | E2 E5 E6 W |
| SC-82 | A 가 접속을 끊고 **A 의 함선이 디스폰된 것을 단언**한 뒤, C 가 **처음** 접속 → `BACKFILL` 로 A 의 발견 + `DEPOSIT_FIELD_STATE` 에서 그 광맥이 드러나 있음 | `trace-abc` (c) 단계 — **C 는 별도 프로세스**(`bots probe --case trace-c` 를 따로 실행) | **A 가 아직 월드에 있음** → `SHIP_DESPAWNED{LINGER_EXPIRED}`(A 의 함선) 행을 먼저 단언. **C 가 기억으로 앎** → C 는 별도 프로세스, 판정 입력은 C 가 받은 메시지뿐. C 의 actor 가 그 월드에 `SESSION_OPENED` 이력 0 이었음 | qa | AC-16(c), S-4, GDD §36 | E2 E5 E6 W |
| SC-83 | DB: X 에 대한 `MINERAL_DISCOVERED` **정확히 1 행**, 근거 = A 의 **첫** `MINERAL_MINED`, 증거 1 행 | `python tests/e2e/history_oracle.py --world <id> --mineral <X>` | **X 의 채굴이 1 건뿐이라 "첫" 이 자명** → X 의 `MINERAL_MINED` ≥ 2(SC-81 의 B) 중 근거가 `(tick, sequence)` 최소임을 단언 | qa | AC-16(d) | E2 W |

### Q. 경쟁과 부하 (AC-17) — qa

| ID | 검증 항목 | 검증 방법 | ⊘ 자명 통과 → 배제 단언 | 담당 | 근거 | 불가 |
|----|----------|----------|------------------------|------|------|------|
| SC-84 | S-2: 두 봇이 같은 목표 tick 에 같은 광물의 **다른 광맥**을 캠 → 발견자 = `sequence` 작은 쪽, 기록 1 | `bots probe --case race-same-tick` | **두 `MINERAL_MINED` 가 같은 tick 이 아님** → 그 단언이 먼저. 아니면 이 실행은 **무효**(초록 아님) — 시도 횟수와 무효 횟수를 찍는다 | qa | AC-17(a), S-2 | E2 E5 E6 W |
| SC-85 | S-3: 잔량 100 kg 광맥에 세 봇이 같은 tick → **100 / `RESOURCE_DEPLETED` / `RESOURCE_DEPLETED`**, 인벤토리 합 증가 = 100 = 광맥 감소 | `bots probe --case last-kg` (잔량 100 을 만드는 선행 채굴 포함) | **처리 직전 잔량이 100 이 아님**(회복 경계가 끼어듦) → 처리 tick 의 식(I-69) 잔량 = 100 을 **실제 tick** 으로 계산해 단언. 세 명령이 같은 tick. 아니면 무효 | qa | AC-17(b), S-3 | E2 E5 E6 W |
| SC-86 | S-8: 보존 법칙이 **모든 광물**에 대해 성립(광물별 좌·우변과 채굴 수 출력), **회복이 실제로 일어난 광맥 ≥ 1** | 부하 실행(SC-87) 뒤 `mining_ledger.py --world <id> conservation --require-regen` | **회복 0 인 실행에서 회복 항이 0 = 0** → 재계산 회복량 > 0 인 광맥 ≥ 1 이 판정 조건 | qa | AC-17(c), S-8 | E2 E5 E6 W |
| SC-87 | 31 연결(봇 30 + Unity 1 또는 봇 31)이 이동 + 쿨다운마다 채굴 **10 분**: **tick 초과 비율 ≤ 0.5 %** | `bots run --stage mine-load` + `/debug/stats` 델타(`stats_delta.py`). 서버는 **부하 시험용 매장량 확대 데이터 사본**(`data/` 복사본에서 광맥 `initial_reserve_kg` 만 확대, `STARFALL_DATA_DIR` 로 지정, 원본 `data/` 불변)으로 띄운다 — 조건(architect): ① 사본은 `data/` 원본과 **별도 디렉터리**(원본 변경은 designer 소유이고 SC-72 사본 동일성을 깬다) ② 사본도 서버 기동 검증(스펙 §4.7)을 **통과한 상태로** 뜬다. 증거 README 에 사본 경로·sha·바꾼 필드·배율·기동 로그의 적재 줄, 그리고 **실행 전 도달 가능성 계산 한 줄**(수락 ≥ 3000 이 사본 매장량으로 가능함) | **31 연결이 실제로 안 유지됨 / 채굴을 거의 안 함** → 측정 구간의 `ws_connections` 최소 ≥ 31, 수락 채굴 수 ≥ 30 × (600 / 3) × 0.5 를 찍는다(절반 미만이면 부하가 아니다) | qa | AC-17(d), 스펙 §8 | E2 E5 E6 E7 W |
| SC-88 | 같은 부하 실행 뒤 원장·보존 항등식 성립 | `mining_ledger.py --world <id> ledger` · `conservation` | SC-23·24 와 같은 분모 단언(키 ≥ 30) | qa | AC-17(d) | E2 E5 E6 E7 W |
| SC-89 | 정상 부하에서 **`RECORDING_BACKLOG` 거부 0** | `bots run --stage mine-load` 의 사유별 집계 + `recording_lag` 최대값(`/debug/stats`) | **백로그 경로가 죽어 있어 0** → SC-29 가 같은 빌드에서 PASS 여야 이 항목이 PASS(검출기 생존 증거). `recording_lag` 최대값을 찍는다 | qa | AC-17(d), ADR-0013 §6 | E2 E5 E6 E7 W |
| SC-90 | `MINERAL_DISCOVERED` 수 = 채굴된 광물 종류 수 ≤ 4, **고갈 발생 ≥ 1** | SQL: 광물 distinct 수(`MINERAL_MINED`) vs 역사 행 수, `deposit_remaining_after_kg = 0` 행 수 — 이 SQL 을 `mining_ledger.py --world <id> discoveries` 가 실행한다 | **아무 광물도 안 캐 0 = 0 / 고갈 경로 미실행** → 채굴 광물 종류 ≥ 2, 고갈 ≥ 1 이 판정 조건 | qa | AC-17(d), S-10 | E2 E5 E6 E7 W |

### R. 기록 무결성 (AC-18) — qa

| ID | 검증 항목 | 검증 방법 | ⊘ 자명 통과 → 배제 단언 | 담당 | 근거 | 불가 |
|----|----------|----------|------------------------|------|------|------|
| SC-91 | 이번 실행 월드의 `event_type` distinct 가 **기대 집합**(`SESSION_OPENED`·`SESSION_CLOSED`·`SHIP_SPAWNED`·`SHIP_DESPAWNED`·`MINERAL_MINED`) 이다 | SQL `SELECT event_type, count(*) FROM domain_events WHERE world_id = '<id>' GROUP BY 1` | **실행이 채굴을 안 해 4 종만** → 5 종 전부 ≥ 1 이 판정 조건(부분집합이 아니라 같음) | qa | AC-18(a) | E2 W |
| SC-92 | 모든 `MINERAL_MINED.causation_id` 가 `processed_commands.command_id` 에 있고 **그 역도** 성립 | `mining_ledger.py --world <id> causation` | **둘 다 0 행** → 행 수를 찍고 0 이면 FAIL. 두 방향 누락 수 각각 | qa | AC-18(b), ADR-0013 §3 | E2 W |
| SC-93 | `(world_id, tick)` 별 `sequence` 빈틈 없음 | `python tests/e2e/check_sequence_gaps.py --world <id>` (p1-01 도구에 `--world` 추가) | **검사한 tick 수 0** → tick 수·행 수를 찍는다. 채굴 tick(이벤트 ≥ 2 인 tick) ≥ 1 | qa | AC-18(c) | E2 W |
| SC-94 | p1-01 §11-8 의 **자기 참조 결함 장부 7 건이 그대로** 있다 — 기본 월드 `causation_id = event_id` 집합이 장부 7 개와 **정확히 같다** | SQL `SELECT event_id FROM domain_events WHERE causation_id = event_id ORDER BY 1` 을 장부와 비교 | **질의가 0 행**(테이블·조건 오류) → 결과 7 행이 판정 조건. 6 이면 추가 전용 위반, 8 이면 새 결함 | qa | AC-18(d), 원칙 5 | E2 |
| SC-95 | 역사 테이블 쓰기(INSERT) 코드 위치가 **러너 모듈 하나**다 (H-14) | `rg -U --pcre2 'INSERT\s+INTO\s+(historical_events\|historical_event_sources\|evidence)\b' server/` — **여러 줄 패턴**(sqlx 쿼리는 `INSERT INTO` 와 표 이름 사이에 줄바꿈이 올 수 있다 — 줄 단위 grep 은 밖의 쓰기를 놓쳐 **거짓 PASS**). **제외: 테스트 코드**(`tests/` 디렉터리, `#[cfg(test)]` 모듈) — SC-53 이 SQL 로 직접 INSERT 하므로 넣으면 **거짓 FAIL**(history). 제외한 파일 목록을 출력한다 | **러너 모듈에서도 0 곳**(패턴 오류) → 같은 패턴으로 러너 모듈 안 ≥ 1 곳이 판정 조건 — **이것이 패턴이 우리 코드 서식을 실제로 잡는다는 양성 대조**다. 소스는 부정만 증명한다 — **SC-91·92·93 과 SC-58 의 실행 증거와 짝으로만 PASS** | qa | AC-18(e), H-14 | — |
| SC-107 | **정지 경로 제약 분모 대조**: 0002·0003 이 적용된 DB 에서 테이블별 `pg_constraint` 행 수가 ADR-0013 §5a-1 표의 요구 수와 같다 — 경제 **53**(`worlds` 11 · `domain_events` 17 · `inventory_items` 7 · `deposit_states` 10 · `processed_commands` 8), 역사 **25**(history 가 H2 에서 실측 확정), 추가 전용 트리거 **4**. 어긋나면 FAIL(검토 없는 새 제약 또는 빠진 제약). **판정은 0001~0003 이 전부 적용된 DB 에서만**(architect r2 정정: 0001 실측은 `domain_events` **16** = p 1 · u 1 · f 1 · c 3 · n 10, 17 은 0002 의 `payload_is_object` 를 더한 뒤다 — 적용 전 DB 에서 17 을 기대하면 거짓 FAIL, 적용 후에 16 을 기대하면 CHECK 누락을 놓친다). 도구는 `_sqlx_migrations` 의 적용 버전 목록을 먼저 찍고 0003 까지가 아니면 **판정하지 않는다**(exit 4) | `python tests/e2e/constraint_census.py` — ADR 표에서 읽은 요구 수와 psql `pg_constraint`·`pg_trigger` 실측을 테이블별로 나란히 | **표 파싱이 0 행 / 테이블 이름 오타로 실측 0 과 요구 0 이 일치** → 파싱한 테이블 수와 요구 합(78 + 트리거 4)을 찍고, 어느 테이블이든 요구 0 이면 FAIL. PG18 `contype = 'n'`(NOT NULL) 포함을 출력에 명시. **세는 기준을 하나로**: 도구가 테이블별로 **contype 별 수**(p·u·f·c·n)를 찍고, 합이 ADR 표의 요구 수와 같은지 본다 — ADR 의 `worlds` 11·`domain_events` 16 실측이 `n` 을 포함한 수임을 첫 실행에서 contype 별 수로 확인한다(history 지적) | qa | ADR-0013 §5a-1 | E2 |
| SC-109 | **"제약이 있다 ≠ 막는다"**: 0002 가 추가한 `domain_events` `CHECK (jsonb_typeof(payload) = 'object')` 가 JSONB `null` 직접 INSERT 를 **거부**한다 | psql 로 qa 탐침 월드에 `payload = 'null'::jsonb` 행 INSERT → `check_violation`(23514) | **INSERT 가 다른 이유로 실패**(FK·NOT NULL·UNIQUE) → 에러 코드 23514 와 제약 이름 `payload_is_object` 를 단언. **짝(무엇이 거부됐는가)**: 거부된 문장과 payload 만 다른 문장이 **이 CHECK 에 걸리지 않음**을 `EXPLAIN` 없이 보이려면 행이 남으므로(추가 전용) **짝은 탐침 월드 1 행만** — p1-01 `QA_APPEND_ONLY_PROBE` 관례. 탐침 행을 만들지 않기로 하면 짝은 에러 코드·제약 이름 단언으로 대신하고 그 선택을 증거에 적는다 | qa | ADR-0013 §5a-1, AC-5(d) | E2 |
| SC-113 | **CI 의 DB 테스트가 실제로 돈다**(리더 결정 Q-2): `server (rust)` job 에 `postgres:18.6-trixie` 서비스 + `STARFALL_DB_TESTS=required` + `DATABASE_URL`. 게이트가 stderr 의 `STARFALL_DB_TEST RAN <이름>` 줄을 **§3.4 의 DB 테스트 이름 목록과 이름으로 대조** — 빠진 이름 ≥ 1 이거나 `STARFALL_DB_TEST SKIPPED` ≥ 1 이면 FAIL | `python tests/e2e/db_test_census.py --contract <이 문서> --log <cargo test 로그>` (CI `server (rust)` job 의 한 단계) | **개수만 세면 다른 테스트가 빈자리를 채운다 / RAN 줄이 캡처돼 안 보여 0 = 0** → 목록 수(§3.4)·RAN 수·목록∩RAN·빠진 이름·SKIPPED 수를 찍고, 목록 수 0 이면 FAIL(표 파싱 사망). **선행 조건**: server 가 Phase 4 첫 RED 에서 **통과한 테스트의 RAN 줄도 로그에 보이는지** 실측한다(libtest 는 통과 테스트의 `eprintln!` 을 캡처한다 — fd 2 에 직접 쓰거나 `--nocapture` 가 필요할 수 있다). 실측 전에는 이 항목이 **대기** | qa | 리더 결정 Q-2, AC-1 | E1 E2 |
| SC-114 | **DB 테스트는 증거 DB 를 건드리지 않는다**: 로컬에서 `STARFALL_DB_TESTS=required cargo test --workspace` 전후로 `starfall` DB 의 `domain_events` 전체 행 수가 **같다**. 헬퍼는 DB 이름이 `starfall_test_` 접두사가 아니면 패닉한다 | psql `SELECT count(*) FROM domain_events` 전·후 + 같은 실행 로그의 RAN 수 + server 단위 테스트 `cargo test -p starfall-testdb refuses_non_test_database`(`starfall` 이름으로 풀 열기 → 패닉) | **DB 테스트가 한 건도 안 돌아 행 수가 공짜로 같음** → 같은 실행의 RAN ≥ 1(§3.4 목록 전부면 더 좋다). **방어가 실제로 걸리는가**(규칙 6) → 접두사 없는 이름에서 패닉을 관측 | qa · server | 리더 결정 Q-2, CLAUDE.md 금지 | E1 E2 |

### S. 계약 커버리지·경계면·CI (AC-19 a·b·d, 스펙 §5.1a) — qa

| ID | 검증 항목 | 검증 방법 | ⊘ 자명 통과 → 배제 단언 | 담당 | 근거 | 불가 |
|----|----------|----------|------------------------|------|------|------|
| SC-96 | `check_contract_coverage.py --strict` **종료 코드 0** (기준선 errors 25) | `python .claude/skills/integration-qa/scripts/check_contract_coverage.py --strict` | **레지스트리 타입이 줄어 errors 0** → `types: 23` 을 판정 조건에 넣는다 | qa | AC-19(a) | E9 |
| SC-97 | 신규 타입의 스키마·Rust·C# **필드별 표**, 불일치 0 — §4 의 고정 대상 전부 | qa 가 세 파일을 **함께 읽어** 표를 쓴다(`python tests/e2e/interface_matrix.py --slice p1-02` 가 필드 목록을 뽑고 사람이 대조) | **표의 행이 스키마에서만 뽑혀 Rust·C# 누락이 안 보임** → 행 = 세 쪽 필드의 **합집합**. 타입별 필드 수(세 쪽 각각)를 찍는다 | qa | AC-19(b) | E9 |
| SC-98 | `.github/workflows/gates.yml` 의 출처 게이트가 **이 계약**을 가리키고 PR 에서 초록이며, 그 게이트가 찍은 **항목 분모 = 114**(p1-01 의 90 이 찍히면 FAIL) | PR 의 `judgment gates (python)` job 로그에서 `python tests/e2e/check_item_sources.py --contract _workspace/p1-02-mining/02_sprint_contract.md --slice p1-02` 단계 출력 | **다른 슬라이스 라벨을 전부 제외해 볼 것이 0 인 채 exit 0** → 게이트가 "이 슬라이스 라벨 수 / 제외한 다른 슬라이스 라벨 수" 를 둘 다 찍는다(§3.3). Phase 5 에서는 이 슬라이스 라벨 수 ≥ 1 | qa | AC-19(d) | — |
| SC-99 | 스펙 §5.1a 의 `responses` 유도 항등식: **수락된 `MINE_RESOURCE` 수 == 그 결과 뒤 같은 tick 의 `INVENTORY_STATE` 수** — 봇이 받은 메시지로 | 봇 공통 집계 — `bots probe --case trace-abc` 와 `bots run --stage mine-load` 실행마다 요약 줄 | **채굴 0 건이면 `0 == 0`** → **수락 0 이면 FAIL 로 인쇄**(스펙 §5.1a 명시). 결과가 `COMMAND_RESULT` **뒤**(순서)임도 | qa | §5.1a, AC-15·17 | E2 E5 E6 W |

---

## 2. 데이터 파일 짝 표 (SC-07 의 채점 기준 — AC-19 c2)

**분모 = `data/**/*.json` 10 파일** (qa `find` 2026-09-27). ① 은 **서버 기동 로그의 해석 경로 줄**(실행 증거)이어야 하고 소스 읽기로 채우지 않는다(규칙 9). 오늘의 ① 칸은 전부 "구현 후 로그로" 이고, **오늘 로그로 확인되는 것은 0 / 10**(서버가 아직 파일 경로를 로그에 찍지 않는다 — 찍는 것 자체가 S2 의 몫이다).

| # | 파일 | ① 읽는 코드 (기대 — 로그로 확인) | ② 게이트 | 오늘 |
|---|------|------------------------------|---------|------|
| 1 | `data/ships/scout-s01.json` | `data.rs::load_ship_classes` | `validate_data_files.py` + `data.rs` 테스트 | ② 있음 · ① 로그 없음 |
| 2 | `data/world/systems/cradle.json` | `data.rs::load_star_system` | 같음 | ② 있음 · ① 로그 없음 |
| 3 | `data/movement/sync-tuning.json` | `data.rs::load_sync_tuning` | 같음 | ② 있음 · ① 로그 없음 |
| 4~7 | `data/minerals/{cobaltine,ferrosite,glacine,starfall-glass}.json` | S2 의 광물 로더 | 스키마(SC-06) + §4.7 유도값(SC-05 ②⑤~⑧) | **둘 다 없음** |
| 8 | `data/world/deposits/cradle.json` | S2 의 광맥 로더 (클라이언트는 id·이름·위치·반지름만 — C2) | 스키마 + SC-05 ①③④⑨⑩ | **둘 다 없음** |
| 9 | `data/mining/mining-rules.json` | S2 의 규칙 로더 | 스키마 + SC-05 ④⑤⑩ | **둘 다 없음** |
| 10 | `data/history/rules/mineral-discovery.json` | S2 가 읽어 history 코어에 값으로 넘김(ADR-0014 §5) | 스키마 + SC-05 ⑪⑫ + **SC-50 golden** | **둘 다 없음** |

- **`data/movement/sync-tuning.json` 에 주의**: p1-01 에서 "데이터 파일의 값만 보고 폴백을 중계했는데 서버 코드가 그 값을 읽는 곳이 없었다"(p1-01 §7b 규칙 9 근거 사례 ①)는 이 파일 계열에서 나왔다. 파일 단위로 읽힌다고 **필드** 단위로 읽힌다는 뜻은 아니다 — 이 항목은 **파일** 단위만 판정한다(스펙 AC-19(c2) 문언). 필드 단위는 계약 외.

---

## 3. QA 소유 도구와 CI 게이트 (Q1b·Q2·Q3)

### 3.1 봇 케이스 (`tools/bots/` — Q2)

`bots probe --case <이름>` 로 부른다. **이름은 §1 행의 백틱 안 토큰과 글자 그대로 같아야** 출처 게이트의 Rust 판정이 잇는다(`rust_case_refs.py` — 완전 일치만).

| 케이스 | 하는 일 | verdict 항목 |
|--------|--------|-------------|
| `leak-scan` | 새 월드 첫 채굴 전후의 원문 프레임 저장·검색 | SC-40 |
| `backlog` | 채굴·이동 중 qa 가 postgres 를 멈췄다 재개(절차서 신호) | SC-29~32 |
| `backlog-idle` | DB 정상·한가한 서버에서 쿨다운마다 채굴 60 초, `persist_backlog` 톱니 관측 | SC-104 |
| `cas-halt` / `cas-reload` | 인벤토리 변조 전 채굴·변조 후 채굴 / 재기동 뒤 채굴 | SC-25 · SC-26 |
| `trace-abc` / `trace-c` | A·B 동시 접속 → A 발견 → B 채굴 → A 종료·디스폰 → (별도 프로세스) C 첫 접속 | SC-60·61·64·80~82·99 |
| `notice-gap` | 러너 커밋 순간에 세션을 여는 반복 | SC-63 |
| `cheat-mine-inject` · `cheat-mine-range` · `cheat-mine-fast` · `cheat-mine-cooldown` · `cheat-mine-unknown` | AC-15 (a)~(e) | SC-73~77 |
| `mine-dup` · `mine-dup-reconnect` · `mine-dup-restart` | AC-15 (f) | SC-78 |
| `mine-dup-cross-actor` | 다른 actor 둘이 같은 `command_id` — 정지 경로 DoS 시도 | SC-108 |
| `race-same-tick` · `last-kg` | S-2 · S-3 | SC-84 · SC-85 |
| `bots run --stage mine-load` | 31 연결 10 분 | SC-87~90·99 |

- **같은 tick 겨냥**: 봇은 스냅샷의 서버 tick 을 보고 목표 tick 에 맞춰 보낸다. 같은 tick 이 **안 되면 무효**로 세고 재시도한다 — 성공 확률을 계약이 가정하지 않는다.
- 봇 verdict 라벨은 `ContractRef::Verdict("p1-02 SC-nn …")` 형식(§3.3). 관측용 참조는 `ContractRef::Reference`.

### 3.2 도구 표 (출처 게이트가 읽는다 — `| 스크립트 | 항목 |`)

| 스크립트 | 항목 |
|---------|------|
| `new_world.py` | 없음 — 준비 도구, verdict 를 내지 않는다 |
| `validate_data_files.py` | SC-06 |
| `data_file_pairs.py` | SC-07 |
| `server_boot.py` | SC-05 |
| `mining_ledger.py` | SC-23 · SC-24 · SC-32 · SC-86 · SC-88 · SC-90 · SC-92 |
| `history_oracle.py` | SC-58 · SC-83 |
| `constraint_census.py` | SC-107 |
| `db_test_census.py` | SC-113 |
| `check_sequence_gaps.py` | SC-93 |
| `stats_delta.py` | SC-87 |
| `interface_matrix.py` | SC-97 |

### 3.3 CI 게이트 조정 — **p1-01 도구의 라벨이 이 계약에서 전부 위반이 된다** (Q1b)

**문제 (qa 실측 2026-09-27 — 이 초안을 `--contract` 로 준 실행: exit 1).** 분모는 맞게 나온다(`계약 항목: 99`, `도구가 지명된 항목: 99 / 99`). 그러나 `check_item_sources.py` 는 `tests/e2e/*.py` 전부의 verdict 라벨(`"item": "SC-nn …"`)과 `tools/bots/src` 의 `ContractRef::Verdict` 팔을 **주어진 계약 하나**에 대조한다. 경로만 p1-02 계약으로 바꾸면 **p1-01 도구 12 개의 라벨 33 건이 "남의 번호" 위반**이 되고(`ship_events.py` 10 · `block8_invariants.py` 5 · …), Rust verdict 6 팔은 케이스 이름이 이 계약 §1 에 없어 **미지명 verdict ③ 6 건**이 된다 → exit 1, CI 빨간불. 같은 순간 p1-01 계약으로 돌린 게이트는 `위반 없음` 이다 — **도구는 그대로인데 계약만 바꿔 빨간불이 켜진다.** 반대로 그 도구들을 지우거나 라벨을 떼면 **p1-01 증거에서 항목으로 가는 길이 사라진다**(p1-01 R23 의 "고치는 것이 재는 것을 망가뜨리는 형태").

**결정 (qa 안 — architect 승인 2026-09-27, 조건 1 — 아래 2 의 끝 문장):**

1. **슬라이스 표지.** 이 슬라이스부터 verdict 라벨은 **`p1-02 SC-nn`** 으로 시작한다(`.py` 는 `"item": "p1-02 SC-23 (AC-5a) …"`, Rust 는 `ContractRef::Verdict("p1-02 SC-73 …")`). 표지 없는 라벨은 **p1-01 라벨**로 읽는다(소급하지 않는다 — 규칙 9 의 적용 시점 원칙과 같다).
2. **게이트에 `--slice <표지>`.** `--slice p1-02` 는 표지가 `p1-02` 인 라벨만 대조하고, 나머지는 **세어서 찍고 판정에서 뺀다**("다른 슬라이스 라벨 N — 이 계약으로 판정하지 않음"). `--slice` 없이 돌면 지금처럼 **표지 없는 라벨만** 대조한다(p1-01 모드 — 표지 있는 라벨은 세어서 뺀다). **빼는 것을 찍지 않으면 "볼 게 없었다" 와 "통과했다" 가 같은 출력이 된다**(p1-01 R23). **architect 조건: 표지가 붙은 라벨의 슬라이스가 알려진 목록(`p1-01`, `p1-02`)에 없으면 FAIL**(exit 4 — 판정 불가). 오타 표지(`p1-2 SC-`)가 조용히 "다른 슬라이스" 로 빠지면 그것이 새 검출기 사망 경로다.
3. **CI 단계 둘.** `judgment` job 안에 `출처 게이트 (계약 p1-02)`(`--slice p1-02`, **분모 114** — SC-98 의 판정 대상)와 `출처 게이트 (계약 p1-01, 회귀)`(표지 없는 라벨의 p1-01 대조 — 닫힌 계약의 라벨이 나중에 깨지는 것을 잡는다)를 둔다. **job 이름(`server (rust)`·`bots (rust)`·`judgment gates (python)`)은 바꾸지 않는다** — 필수 체크 이름이 job 이름이라 바꾸면 브랜치 보호 설정도 바꿔야 하고, 안 바꾸면 병합이 영원히 막힌다.
4. **규칙 6 대조 (selftest 에 추가)**: ① `--slice p1-02` 에서 표지 없는 라벨 `SC-11` → 위반 아님·제외 1 로 셈 ② `--slice p1-02` 에서 `p1-02 SC-999`(계약에 없는 번호) → 위반 ③ p1-01 모드에서 `p1-02 SC-11` → 제외(위반 아님) ④ Rust 팔 `p1-02 SC-73` 이 백틱 `cheat-mine-inject` 로 지명됐는데 번호가 다르면 위반 ⑤ **알 수 없는 표지 `p1-2 SC-11` → exit 4**(architect 조건). **네 대조가 고치기 전 게이트에서 실제로 다른 결과를 내는지**(①·③은 오늘 위반으로 나와야 한다)를 먼저 보인다.
5. **함께 고치는 p1-01 도구 결함 하나 (계약 외 발견 → 만기 §9).** `rust_check()` 의 **유령 지명 검사(⑤)는 구조적으로 늘 0 이다** — `contract_named_cases()` 가 `known`(= `as_str()` 이름)에 있는 토큰만 담아 돌려주므로 `set(named) - set(names.values())` 는 공집합이다(`rust_case_refs.py:154`, `check_item_sources.py:180`). **검출기 사망형**이다. 이 계약의 §1 이 아직 없는 케이스(`trace-abc` 등)를 백틱으로 적었으므로 **오늘 그 검사가 살아 있었다면 **유령 지명 19 건**을 냈을 것이다 — **고친 게이트 실측**(2026-09-27, Q1b): 같은 계약에서 HEAD 의 옛 게이트 ⑤ = **0**, 고친 게이트 ⑤ = **19**(아직 없는 probe 케이스 이름. r1 전 손으로 센 18 은 SC-78 의 `mine-dup-reconnect`·`mine-dup-restart` 가 `--case` 없이 적혀 있던 것까지 셌고, 게이트는 `--case` 뒤 토큰만 센다 — 그 두 칸을 `bots probe --case …` 로 고쳤다)** — 구현 전에는 정상이고, Phase 5 에서는 0 이어야 한다. 고친 뒤 셀프테스트에 "없는 케이스를 백틱으로 지명 → ⑤ ≥ 1" 을 넣는다.

**PR 템플릿 (`.github/pull_request_template.md`)** — "CI 가 보지 않는 게이트" 에 두 줄 추가:

- [ ] **p1-02 병합 조건**: `unity test client --mode EditMode`(PlayMode 테스트가 있으면 `--mode PlayMode` 도) 의 `test-results.xml` 을 `_workspace/p1-02-mining/evidence/` 에 두었고, **스위트별** total · passed · failed · skipped 와 `ClientDataCopy_MatchesRepositoryOriginal` 결과를 적었다(SC-72)
- [ ] 발견을 재는 실행이면 새 월드 id 와 INSERT 문을 증거에 남겼다(계약 §0.2)

**CI 가 보지 않는 것 (리포트 요약에 싣는다):** Unity EditMode·PlayMode(SC-65~72·110) · 실서버·봇 세션(SC-04·25·26·29~32·40·58~61·64·73~94·99·104·108) · 로컬 증거 DB 불변(SC-114). **DB 통합 테스트는 이제 CI 가 본다**(리더 결정 Q-2 → §3.4) — 단 SC-113 이 PASS 일 때만 그 말이 참이다.

### 3.4 DB 통합 테스트 — CI 필수 실행 (리더 결정 Q-2, server `02_server_ack.md` §3.2)

- **헬퍼**: 테스트 전용 크레이트 `starfall-testdb`(server, T0). 테스트마다 `CREATE DATABASE starfall_test_<uuidv7>` → 마이그레이션 0001~0003 → 사용 → `DROP`. 풀을 여는 함수는 DB 이름이 `starfall_test_` 접두사가 아니면 **패닉** — 증거 DB `starfall` 에 붙는 경로가 구조적으로 없다(SC-114 가 그 방어를 실행으로 확인). 패닉으로 남은 DB 는 다음 실행이 1 시간 넘은 것만 지운다.
- **모드**: `STARFALL_DB_TESTS=required`(CI) → 접속 실패는 테스트 실패. 미설정(로컬) → 건너뛰되 줄을 남긴다. **`#[ignore]` 는 쓰지 않는다**(p1-02 신규 DB 테스트 0 건. 기존 `live_smoke.rs` 는 실서버 스모크라 대상 밖).
- **줄 형식**: 각 테스트가 stderr 에 `STARFALL_DB_TEST RAN <이름>` 또는 `STARFALL_DB_TEST SKIPPED <이름> reason=…`. **`<이름>` 은 이 계약 §1 의 테스트 이름과 글자 그대로 같다**(`TestDb::create` 인자 — server·history 수락).
- **대조 목록은 손으로 유지하지 않는다**: `db_test_census.py` 가 §1 행에서 **백틱 테스트 이름 바로 뒤에 ` [DB]` 표식이 붙은 것**을 뽑는다(r2 기준 **24 개** — server 12 · history 12). 손으로 쓴 두 번째 목록은 p1-01 의 `RUST_KNOWN_MISMATCHES = 8` 처럼 관측을 앞질러 낡는다. 표식이 빠진 DB 테스트는 목록에서 빠지므로, **Phase 5 에서 `RAN` 에만 있고 목록에 없는 이름**도 출력한다(표식 누락 탐지 — 역방향).
- **CI 조정(Phase 4 첫 작업, Q1b 와 함께)**: `server (rust)` job 에 `services: postgres: image: postgres:18.6-trixie`(로컬 `docker-compose.yml:22` 와 같은 이미지), env `STARFALL_DB_TESTS=required`·`DATABASE_URL`(관리 연결 — 헬퍼가 임시 DB 를 만드는 데만 쓴다), 그리고 SC-113 단계. **job 이름 `server (rust)` 는 바꾸지 않는다.**
- **반영 상태 (qa, 2026-09-27 — server T0 실측 뒤)**: `gates.yml` 에 postgres 서비스, `test` 단계 한정 env, `tee` 로그, 별도 단계 `DB 테스트 이름 대조 (p1-02 SC-113)`(`if: success() || failure()` — cargo test 실패와 구분)를 넣었다. 로컬 확인: server smoke 모양의 로그로 census → `FAIL · 목록 24 · RAN 1 · 누락 24 · 계약 밖 RAN = [testdb::smoke::creates_migrates_and_drops_an_isolated_database]` — **역방향(계약 밖 이름) 탐지 경로가 산다는 양성 증거.** **CI 첫 실행에서 확인할 것**(act 없이 로컬에서 볼 수 없다): ① 서비스 컨테이너가 헬스체크를 통과하고 `localhost:5432` 로 붙는다 ② `$RUNNER_TEMP/cargo-test.log` 가 census 단계에서 읽힌다 ③ `python3` 이 러너에 있다 ④ `pipefail` 로 cargo test 실패가 `test` 단계 실패로 남는다 ⑤ smoke 의 RAN 줄이 CI 로그에도 보인다(로컬 실측의 CI 판)

---

## 4. 경계면 비교표의 고정 대상 (SC-97 의 채점 기준)

양쪽을 **함께** 읽는다. 한쪽 존재 확인은 검증이 아니다.

| 경계면 | 생산자 | 소비자 | 비교할 것 |
|-------|-------|-------|---------|
| 계약 ↔ Rust ↔ C# | 신규 스키마 10(데이터 4 는 Rust 만) | `server/crates/contracts` · `client/.../Generated` | 필드명·필수·nullable·열거값·`rename_all`, **`DEPOSIT_FIELD_STATE` 의 "넷 다 null 또는 넷 다 값"**(Rust 내부 `Option<Revealed>` 여도 와이어는 넷) |
| 명령 | 클라이언트·봇의 `MINE_RESOURCE` 생성부 | 서버 역직렬화·판정 | 페이로드 키 = `{deposit_id}` 뿐, 서버가 클라이언트 값을 믿는 필드 0 |
| 즉시 응답 ↔ 최종 결과 | `COMMAND_RESULT{ACCEPTED}` → `INVENTORY_STATE` | 클라이언트 인벤토리 패널 | 접수를 완료로 취급하지 않음(SC-69) |
| 도메인 → 역사 | `MINERAL_MINED` payload | history 코어의 입력 파싱 | 코어가 쓰는 필드 ⊆ payload 필수 필드, **오라클 SQL 필드 이름**(§0.10) |
| 영속화 → 러너 | 커밋 알림(tick) | 러너 워터마크 | T0 `02_interface.md` 의 채널 타입 |
| 러너 → 게이트웨이 | 역사 기록 채널(목록 + LIVE) | 게이트웨이 BACKFILL 목록·브로드캐스트 | 같은 직렬화 문맥(SC-63) |
| DB ↔ 코드 | `0002_*.sql`·`0003_*.sql` | sqlx 구조체 | 컬럼·nullable·`CHECK fact_status`·`UNIQUE (world_id, event_type, dedupe_key)`·트리거 |
| 데이터 → 클라이언트 | `deposits/cradle.json` | `GreyboxDataLoader` | 읽는 키 = id·이름·위치·반지름뿐(SC-71) |

---

## 5. 신규 경로 "실제로 탔는가" 표 (p1-01 §7 승계 — 리포트마다 채운다)

**통과한 테스트 수는 경로가 실행됐다는 증거가 아니다.** 다음을 **실행됨 / 미실행**과 근거(카운터 델타·이벤트 행·테스트 출력의 건수)로 적는다. 미실행이 있으면 그 경로를 판정하는 SC 는 PASS 가 될 수 없다.

수락 · `TARGET_UNKNOWN` · `COOLDOWN_ACTIVE` · `TARGET_OUT_OF_RANGE` · `SHIP_TOO_FAST` · `RESOURCE_DEPLETED` · `RECORDING_BACKLOG` · `DUPLICATE_COMMAND_ID`(세션 안 / 재접속 / 재기동 각각) · 부분 산출 · 드러남 · 회복 경계 통과 · 배치 재커밋 no-op · 비교 후 쓰기 정지 · PK 위반 정지 · `recorded_at` 실패 정지 · 러너 LIVE · 러너 BACKFILL · 러너 fail-stop · 러너 워터마크 · 러너 재기동 재구축 · 러너 충돌 정지(`history_conflicts_total`, SC-105) · 러너 재처리 무음(SC-106) · `CAPACITY_EXCEEDED` · 다른 actor 중복(월드 정지 없음) · 모호한 커밋 건너뜀(`persist_ambiguous_commits_total`) · payload 직렬화 실패 정지 · 정지 후 스윕 미커밋 · `57P01` 재시도

---

## 6. 기록 항목 (판정하지 않음)

| ID | 무엇 | 출처 |
|----|------|------|
| M-1 | 부하 실행의 tick 본문 p50/p99/max, 영속화 커밋 소요 p50/p99 — p1-01 기준선과 나란히 | 스펙 AC-17(d) |
| M-2 | 역사 알림 지연 p50/p95 — **p95 ≤ 1 초 목표** 대비 | 스펙 §10-13 |
| M-3 | `persist_backlog` 최대·평균 | ADR-0013 §6 |
| M-4 | `DEPOSIT_FIELD_STATE` 브로드캐스트 바이트/초·세션 | 스펙 §8 |
| M-5 | `processed_commands` 적재 소요(기동 로그) — 100 만 행 만기 추적 | ADR-0013 §3 |
| M-6 | 러너 첫 기동 따라잡기 소요와 읽은 행 수 | 스펙 §10-10 |
| M-7 | 거절 비율(사유별 / 전체 명령, 분모 출력) — 첫 5 분 이후 `TARGET_OUT_OF_RANGE + SHIP_TOO_FAST` < 30 % 목표 | 디자인 §7.2 |
| M-8 | 발견까지 시간(월드 시작 tick → 광물별 발견 tick) | 디자인 §7.1 |
| M-9 | 발견 후 유입: 발견 10 분 안에 그 광물을 캔 발견자 아닌 actor 수, 첫 추종자까지 시간 | 디자인 §7.1 (봇 세션은 참고치 — 봇은 배너에 반응하지 않는다) |
| M-10 | 고갈 목격: 빈 광맥에 도착한 actor 수 | 디자인 §7.1 |
| M-11 | 반례 층별 표(§0.5)의 "감지 불가" 건수 | 스펙 §5.2 |
| M-12 | 사람 세션의 `data/` 수치 소감(사거리 150 m·속도 10 m/s·쿨다운 3 s — designer 가 "약하게 확신") | `01_designer_notes.md` |

---

## 7. "미검증(환경)" 처리 기준

| 코드 | 조건 | 영향 |
|------|------|------|
| E1 | cargo 툴체인 사용 불가 | A·C·D·G·H·J절 |
| E2 | Docker/PostgreSQL 미가동, 포트 점유 | DB·실서버 항목 |
| E3 | Unity Editor 라이선스·CLI 실패, Editor 가 열려 있음 | N절 |
| E4 | .NET SDK 미가용 | SC-65 |
| E5 | 서버가 **환경 문제로** 기동 못 함 (`netstat` 첨부) | 실서버 항목 |
| E6 | 봇 하네스 빌드 불가 | 봇 항목 |
| E7 | 31 연결 + Unity + Docker 동시 실행 자원 부족 | SC-87~90 |
| E9 | 선행 태스크 미완 | **대기**(판정 제외) — SC-36·96·97 |

**환경 문제와 구현 부재를 섞지 않는다. 구현이 없으면 FAIL 이다.**

---

## 8. 판정·라운드 규칙

### 8.1 라운드

- 라운드 **최대 3회**. 결과는 `04_qa_report_r{N}.md`.
- FAIL 은 **파일:라인 + 재현 명령 + 기대/실제**를 담아 담당자에게. QA 는 구현 코드를 고치지 않는다(`tools/bots/`·`tests/e2e/`·`.github/` 만 QA 소유).
- §0.5 표와 결과가 다르면 FAIL 이 아니라 **architect 통지**.
- 리포트는 **정확성 판정 / 성능 기록 / designer 지표**를 별도 절로 쓰고, 요약에 **§0.9(증명하지 못하는 것)** 와 **§5(실제로 탔는가)** 를 싣는다.

### 8.2 p1-01 §7b 규칙 1~9 — **그대로 승계** (원문과 근거 사례는 p1-01 계약 §7b)

1. 새 항목의 각 절 옆에 **"이 절을 자명하게 통과시키는 상태"** 를 한 줄로 적고, 그 상태가 가능하면 절을 고친다 — **이 계약 §1 의 `⊘` 칸이 이 규칙의 이행이다.** 시험은 양쪽을 본다: 자명 통과(초록이 공짜)와 **자명 불통과**(빨간불이 영원하다).
2. 판정 기준은 **관측 전에** 적는다 — 옳은지 확신하지 못해도.
3. 판정 기준을 만드는 사람도 자기 기준에 1 번을 돌린다. **자기 문장은 의도로 읽히고, 자기 검사는 통과한 것으로 보인다** — 도구와 문서에도 걸린다(§3.3 ⑤ 가 qa 자기 도구에 돌린 결과다).
4. 항목이 읽는 산출물의 각 필드가 ① 어느 계산 층에서 나왔는지와 ② 그 값을 가르는 임계를 한 줄로 적는다. 못 적으면 아직 판정 기준이 아니다.
5. 한 도구 출력이 여러 SC 를 한 불리언으로 묶지 않는다 — **SC 하나 = verdict 하나.** (그래서 AC-6·AC-17(d) 를 넷으로 갈랐다.)
6. 방어를 넣었으면 그 방어가 걸려야 할 입력에서 **실제로 걸리는지 같은 실행에서** 보인다(양성·음성 대조).
7. 어떤 SC 번호로 verdict 를 내는 도구는 **계약이 그 항목에 지명한 도구**여야 한다(§3.2 표 + §1 행의 `.py`·백틱 케이스 이름). 관측 표식(`관측용`·`참고`·`참조`, Rust `ContractRef::Reference`)은 위반이 아니다. **모르면 비워 두는 것이 기본값** — 짐작으로 메운 칸은 비어 있던 칸보다 나쁘다. 분모를 찍는 것이 절반이고 **분자의 출처를 묻는 것이 나머지 절반**이다.
8. "사소"·"다음 라운드에" 로 미루는 것에는 **만기**(날짜가 아니라 게이트)를 적는다. 형식: `미뤄둠(만기: <게이트>) — <무엇을> · <만기가 지나면 무엇이 되는가>`. 만기가 지나면 리포트 요약에 비-통과로 올라온다.
9. **소스는 부정을 증명할 수 있지만 긍정은 증명하지 못한다.** 긍정의 단언은 실행 증거로만 닫는다. doc 주석·타입·형식 문자열·데이터 파일의 값은 가설이다. 이행: 바이너리 판정 증거에 `git rev-parse HEAD` + `git status --porcelain` 공백 여부(§0.3). **이 계약에서 소스 검사로 판정하는 항목(SC-02·03·69·71·79·95)은 전부 실행 짝을 지정했다** — 짝이 PASS 가 아니면 그 항목도 PASS 가 아니다.

**형태 셋 (p1-01 §7b 14차)**: (가) 조건 미발생형 → 조건 발생의 별도 관측 · (나) 검출기 사망형 → 양성 대조 · (다) 전제 오류형 → 판정 입력의 층·임계 검증(규칙 4). **이 계약에서 (다) 의 후보로 이미 본 것**: §0.10 오라클 필드 이름(검토 문서 원문대로면 판정 입력이 NULL).

---

## 9. 미뤄둠 (만기) — 규칙 8

| 미뤄둠 | 만기 | 만기가 지나면 |
|---|---|---|
| `rust_check()` 유령 지명 검사(⑤)가 구조적으로 늘 0 (§3.3 ⑤) — qa 도구 결함 | **Q1b 게이트 조정 PR 병합 전** | 리포트 요약에 비-통과. 그 전까지 ⑤ 의 `0` 은 판정 근거로 쓰지 않는다 |
| §0.5 반례 층별 표 ②·③ 열 | **S1·C1 완료 알림 시점** | SC-36 이 대기에서 FAIL(미기재)로 바뀐다 |
| 데이터 배포 경로 ADR (사용자 결정 Q3 후속, architect 만기 표 그대로) | **p1-03 `00_request.md` 생성 시점** — 그때 ADR 이 accepted 이고 **클라이언트 사본의 서버 전용 필드 수가 0** | 분모 오늘 **2**(`mineral_id`·`initial_reserve_kg` × 광맥 8). 리더의 p1-03 착수 보고에 비-통과 |
| `HISTORICAL_EVENT_NOTICE.historical_event` 판별 union | 두 번째 역사 타입이 계약에 들어오는 시점 | 그 슬라이스의 계약 태스크가 선행 (architect 만기 표) |
| p1-01 이월: ADR-0004 에 증거 CSV LFS 패턴 | **p1-02 슬라이스 종료 전** | 리포트 요약에 비-통과 (architect 몫) |
| BACKFILL → 조회 API (ADR-0014 §6) | **한 월드 PUBLIC 역사 32 건**(r1: 64 → 32 — 64 는 송신 큐 용량과 같아 새 접속을 끊는다) | 그 슬라이스가 조회 API 를 선행 |
| p1-01 이월: 함선 클래스 2 개 이상 | `data/ships/*.json` 2 개 | 이 슬라이스는 함선을 늘리지 않는다 — 열리지 않음 |

---

## 10. 세부 절차 매핑

### 10.1 history H-01~H-15 → 항목

| H | 항목 | 비고 |
|---|------|------|
| H-01 멱등 반복 | SC-45 | |
| H-02 커밋 전 장애 | SC-54 (+ SC-51) | |
| H-03 순열 불변 | SC-47 | r2: **러너 DB 테스트**로 — 삽입 순서만 섞어 SQL `ORDER BY` 를 시험한다(코어에 정렬한 입력을 넣는 형태는 자명 통과 — history). 코어의 비정렬 거부는 SC-48 |
| H-04 tie-breaker | SC-44 | |
| H-05 해시 결정성 | SC-59 | 두 저장소 비교는 재생 비교로 합쳤다(AC-12(b)) |
| H-06 재구축 | SC-59 | 주 DB 비우지 않음 |
| H-07 불변 | SC-52 | "역사 5개 표" 원문 → 이번엔 **3 표**(+ 가변 커서). 스펙 AC-11(b) 기준 |
| H-08 비역사 필터 | SC-49 | |
| H-09 규칙 버전 | SC-50 | |
| H-10 fail-stop | SC-55 | |
| H-11 워터마크 | SC-56 | |
| H-12 오라클 교차 | SC-58 | 필드 이름 §0.10 |
| H-13 수량 0 | SC-35 | 스키마 `minimum: 1` 로 대체(설계상 불가) |
| H-14 쓰기 경로 단일 | SC-95 | 실행 짝 필수 |
| H-15 재기동 뒤 판정기 생존 | SC-57 | `01_history_review.md` §9.2(§6 표 누락 — history 확인). S-7 의 라이브 짝 |

### 10.2 designer S-1~S-10 → 항목

| S | 항목 |
|---|------|
| S-1 첫 채굴과 첫 발견 | SC-80 (+ 사람 SC-68) |
| S-2 같은 tick 경쟁 | SC-84 (+ 단위 SC-13·44) |
| S-3 마지막 kg | SC-85 (+ 단위 SC-12) |
| S-4 나중에 온 사람 | SC-82 |
| S-5 서버 판정 | SC-73~77 |
| S-6 같은 명령 두 번 | SC-78 · SC-108 (+ 단위 SC-16~22 · SC-100) |
| S-7 재기동 | SC-57 |
| S-8 회복·보존 | SC-86 (+ SC-24) |
| S-9 누출 | SC-40 (+ SC-39) |
| S-10 30 명 | SC-87~90 |

---

## 11. 구현자 확인란과 쟁점

| 영역 | 항목 | 담당 | 확인 | 서명/날짜 |
|------|------|------|------|----------|
| A 게이트·순수성 | SC-01~03 | server | ☑ | server 2026-09-27 (r2) |
| B 데이터·기동 거부 | SC-04~05 (SC-06·07 qa) | server | ☑ | server 2026-09-27 (r2) |
| C 채굴 판정 | SC-08~15 | server | ☑ | server 2026-09-27 (r2) |
| D 복사 경로 | SC-16~22 · SC-100 | server | ☑ | server 2026-09-27 (r2) |
| E 원장·CAS | SC-25~27 · SC-101~103 · SC-112 (SC-23·24 qa) | server | ☑ | server 2026-09-27 (r2) |
| F 기록 지연 | SC-28 (SC-29~32·104 qa) | server | ☑ | server 2026-09-27 (r2) |
| G 결정성 | SC-33~34 | server | ☑ | server 2026-09-27 (r2) |
| H 계약 ↔ Rust | SC-35~38 | server | ☑ | server 2026-09-27 (r2) |
| I 누출 | SC-39 (SC-40 qa) | server | ☑ | server 2026-09-27 (r2) |
| M 전달 | SC-63 (server 몫) | server | ☑ | server 2026-09-27 (r2) |
| Q-2 헬퍼 | SC-114 의 `refuses_non_test_database` · SC-113 선행(RAN 줄 가시성 Phase 4 첫 RED 실측) | server | ☑ | server 2026-09-27 (r2) |
| J 판정 코어 | SC-41~50 | history | ☑ | history 2026-09-27 (r2) — SC-47 은 r2 에서 DB 테스트(`starfall-persistence`)로 바뀌었음을 확인 |
| K DB 통합 | SC-51~57 · SC-105·106·111 | history | ☑ | history 2026-09-27 (r2) — **SC-111 포함**(이 행의 범위 표기에 없어 덧붙임) |
| L 재생 | SC-59 | history | ☑ | history 2026-09-27 (r2) |
| M 전달 | SC-62 | history | ☑ | history 2026-09-27 (r2) |
| N 클라이언트 | SC-65~72 · SC-110 | client | ☑ | client 2026-09-27 (r2) — `02_client_ack.md` 최하단에도 서명 |
| **판정 (스펙 대응)** | AC-1~AC-19 전 절 → SC 대응. 대조 방법: 스펙 §7 에서 AC 별 절 표지 `(a)`… 를 뽑아 이 계약의 `AC-n(x)` 언급과 교집합 — **누락 0**(AC-4(d2)·AC-6(a2)·AC-19(c2)(c3) 포함; AC-9 본문의 `(f)` 는 AC-14(f) 참조라 대상 아님). AC-11(h)(i)(ii)(iii) → SC-106·105·111. Q-8 은 AC 로 올리지 않는다(아래) | architect | ☑ | architect 2026-09-27 (r2) |

### 쟁점 (구현자·architect 답을 받아 이 절에 반영한다)

- ~~**Q-1 (architect)**~~ **→ 승인(2026-09-27), 조건 1: 알 수 없는 표지 = exit 4 — §3.3 에 반영.** 원문: §3.3 의 슬라이스 표지 + `--slice` + CI 단계 둘. 대안은 "p1-01 도구를 `tests/e2e/p1_01/` 로 옮기고 `--tools-dir` 로 가른다" 인데, 이미 p1-01 증거·리포트·절차서가 **경로**로 그 도구들을 가리키고 있어 옮기면 닫힌 증거가 끊긴다. 표지 쪽을 추천.
- ~~**Q-2**~~ **→ 리더 결정(2026-09-27): CI 에 postgres, `starfall-testdb`, 이름 대조 게이트 — §3.4, SC-113·114.** 원문: DB 통합 테스트는 `#[ignore]` 인가, 환경 변수로 건너뛰는가? CI 에 postgres 가 없는 채로 `cargo test --workspace` 가 초록이면 SC-18·20·21·25·27·51~57·62 는 CI 에서 **실행 0 건**이다. 건너뛴 테스트 수를 출력하게 하거나 CI 에 postgres 서비스를 붙일지(qa 가 `gates.yml` 을 고칠 수 있다) 결정이 필요하다.
- ~~**Q-3 (architect)**~~ **→ 스펙 AC-11(h) 신설(2026-09-27) — SC-105(충돌 = 정지)·SC-106(재처리 = 무음, 음성 대조)으로 판정 항목화.** 원문: 스펙 I-61·I-67 과 ADR-0014 §4 가 "같은 `dedupe_key`·다른 근거 → 러너 정지 + `history_conflicts_total`" 을 정하지만 **AC 가 없다**(AC-11(c) 는 SQL 직접 INSERT 의 UNIQUE 만). 이 경로를 항목으로 만들지 묻는다. 답이 오기 전에는 §5 표에 "기록" 으로만 둔다.
- ~~**Q-4**~~ **→ H-15 = `01_history_review.md` §9.2, SC-57(§10.1).** 원문: 리더 지시의 "H-01~H-15" 와 `01_history_review.md` §6 의 H-01~H-14. H-15 가 다른 곳에 있으면 알려 달라.
- ~~**Q-5**~~ **→ 된다: `data::reject_c01…c16` + `accepts_unmodified_copy`, 기동 로그 `데이터 파일 적재` `data_file=<data 기준 상대 경로>` 10 줄(SC-05·07).** 원문: SC-05 의 16 경우를 `data.rs` 단위 테스트로 나누는 것이 가능한가(오늘 `data.rs` 테스트는 `loads_the_real_data_directory` 류). 그리고 기동 로그에 **읽은 파일 경로를 찍는 것**(SC-07 ①)을 S2 에 넣을 수 있는가.
- ~~**Q-6**~~ **→ 된다: 겹침 = 같은 tick 에 새 ready 세션 ≥ 1 과 LIVE ≥ 1, `notice_open_overlap_ticks_total`, `notice_no_gap_same_tick`(SC-63).** 원문: SC-63 의 "겹침이 실제로 일어났다" 를 봇이 관측하려면 서버가 알려 줘야 한다(세션 열림과 목록 추가의 순서). 테스트 훅/카운터 이름을 T0 인터페이스에 넣을 수 있는가 — 안 되면 SC-63 은 게이트웨이 단위 테스트로만 판정하고 봇 반복은 기록.
- **Q-8 (client, 리더 전달)** client 제안 "`data/history/rules/mineral-discovery.json` 을 client 가 파싱만 하고 소비하지 않음" 은 **스펙 AC 에 없다**(AC-19(c2) 는 서버가 읽는가를 본다 — 그 파일은 이미 §2 의 10 행 중 하나다). 이번 계약에 항목으로 넣지 않고 client 의 자체 테스트로 두기를 제안 — 넣어야 하면 architect 에게 AC 를 요청한다. **→ architect 판정(2026-09-27): AC 로 올리지 않는다 — qa 안 그대로.** 스펙의 요구는 "서버 전용 값(광물·매장량)을 클라이언트가 읽지 않는다"(I-68, AC-14(f))이고, 규칙 파일은 서버 전용 값이 아니라 클라이언트와 무관한 파일이다. 사본에 들어가는 것은 "`data/**` 전부 복사"(SC-72) 규칙 때문일 뿐이다. client 자체 테스트로 충분하다.
- ~~**Q-7**~~ **→ 된다: 순회 수를 `TestContext.WriteLine`, SC-72 는 원본·사본 독립 순회(SC-66·72).** 원문(r0 — 당시 예측 35, 지금 36): SC-66 의 왕복 예측 35 와 SC-72 의 "테스트가 비교한 파일 수를 로그에" 가 현재 테스트 구조에서 가능한가.

---

## 12. 계약 변경 이력

| 날짜 | 변경 | 근거 |
|------|------|------|
| 2026-10-04 | **SC-28 방법 칸 이름 정합 (r3 동결 중, 문서만, 리더 결정 A)** — 존재하지 않는 테스트 이름 `recording_halted_rejects_state_change`(실행 0, r3 발견)를 실제 이름 셋으로 교체: `recording_lag_boundary_*`(21·이동·halted) + `mine_resource_is_not_rejected_with_recording_backlog_when_lag_is_within_the_limit`(20) + `mine_resource_passes_when_persist_backlog_is_high_but_recording_lag_is_zero`(persist 20·lag 0). **판정 기준·⊘ 불변, 코드 불변.** 각 명령 그대로 실행 ≥ 1: `evidence/r3_20261004/filters_final/` | 리더 결정 A(r3), qa-r3 실행 확인 |
| 2026-10-04 | **SC-10 방법 칸 이름 정합 (r3 동결 중, 문서만, 리더 결정 A)** — 스펙 필수 쌍 3+4 를 맡은 `mining::cooldown_before_range_in_judgement_order` 를 추가(옛 필터 `mining::reject_order_*` 는 이 테스트를 실행하지 않았다). **판정 기준·⊘ 불변, 코드 불변.** 실행 ≥ 1: `evidence/r3_20261004/filters_final/` | 리더 결정 A(r3), qa-r3 실행 확인 |
| 2026-10-04 | **SC-09 방법 칸 두 줄로 (r3 동결 중, 문서만, 리더 지시)** — ① sim 필터 ② gateway 필터(`mine_resource_capacity_exceeded_is_counted_in_commands_rejected_total`, 사유 7 카운터 +1). `--test ws_integration` 을 빼 계약 명령 그대로 추출·실행되게 함. 코드 불변. 실행 ≥ 1: `evidence/r3_20261004/filters_final/` | 리더 지시(r3), qa-r3 실행 확인 |
| 2026-10-04 | **SC-09 방법 칸 필터 교체 (r3 동결 중, 문서만, 리더 결정)** — `mining::reject_*` → `mining::each_rejection_reason`. 옛 와일드카드는 SC-10(`reject_order_*`)·SC-11(`reject_does_not_start_cooldown`)의 올바른 이름까지 구조적으로 잡고 SC-09 의 두 테스트는 잡지 못했다(r2 §3). 함께 정정: "(사유당 1개, 6개)" → 실제 구성(테스트 2개가 사유 2~7 을 나눠 맡음, 판정은 사유별 단언·⊘ 기준), 7 의 카운터 +1 테스트 위치(게이트웨이 통합 테스트) 명기. **판정 기준·⊘ 불변**. 코드 변경 없음. 바꾼 필터가 SC-09 테스트만 잡는지 실행으로 확인: `evidence/r3_20261004/filters/` | 리더 결정(r3 지시), server R2-S 보고, qa-r3 실행 확인 |
| 2026-10-03 | **SC-15 경로 정정 (r2 동결 중, 문서만)** — server 가 R1-S-2 에서 테스트 모듈 `mining_tests` → `mining` 으로 바꿔(이름 정합, 동결 전) 계약의 `mining_tests::range_uses_pre_integration_state` 가 r2 필터 실행에서 0 건이 됐다. 계약 쪽 표기를 `mining::range_uses_pre_integration_state` 로 맞췄다(코드 변경 없음). 정정 뒤 `cargo test -p starfall-sim mining::range_uses_pre_integration_state` 실행 1 건 확인(evidence/r2_20261003/filters/SC-15_recheck.log) | qa 2026-10-03 |
| 2026-09-30 | **§0.5 ② 열 정정 (리더 지시)** — 바로 아래 행에서 "기대" 칸을 `invalid_serde_matrix` 출력에서 전사했다. 실측을 베낀 기대이므로 "기대 = 실측 40/40" 은 **자기 일치이고 아무것도 재지 않았다.** 정정: 전사한 열을 **"실측 기록 (r1)"** 로 개명하고, 기대 열을 **독립 출처**(스펙 §5.3 "Rust serde (예측)" 열 — architect 의 반례 설계)에서 다시 세웠다. 그리고 각 반례의 **의도한 위반**(유효 fixture 대비 필드 변형)을 새 열로 세워 거부 사유와 대조한다 — 거부되기만 하고 엉뚱한 이유로 거부된 반례를 잡기 위해서다. 결과: 기대 = 실측 40/40, 사유 ↔ 의도 40/40 (r1 기록 기준, r2 에서 재측정) | 리더 2026-09-30 |
| 2026-09-30 | **r2 보강 (r1 뒤, 리더 지시)** — §0.5 ② 열(Rust serde)을 r1 실측으로 채움: `invalid_serde_matrix` 의 `--nocapture` 출력(반례 74 중 신규 40)을 전사, 테스트 순회 순서와 파일 이름을 한 줄씩 대조(불일치 0). 신규 40 전부 기대 = 실측 = 거부. 이로써 SC-36 의 "표가 채워지기 전에는 대기" 조건이 풀린다. ③ 열(C#)은 client 몫으로 남는다 | qa 2026-09-30 |
| 2026-09-30 | **r2 보강 (r1 뒤)** — (1) 계약 쪽 **경로 표기 오류** 정정(r1 발견, qa 몫): SC-05 `data::reject_c` → `data::tests::reject_c`(크레이트 이름도 `starfall-game-server`), SC-14 `mining::regen_…` → `mining::tests::regen_…`, SC-15 `mining::range_…` → `mining_tests::range_…` — cargo 필터는 부분 문자열이라 옛 표기는 0 개를 돌렸다. 말단 이름·기준 불변. (2) SC-87 방법 칸에 architect 조건 둘(별도 디렉터리, 사본 기동 검증 통과)과 증거 요건(적재 줄·도달 가능성 계산) | qa · architect 2026-09-30 |
| 2026-09-30 | **r2 보강 (Phase 5 r1 중, 리더 결정 (d))** — SC-87 방법 칸에 "부하 시험용 매장량 확대 데이터 사본" 한 줄. **⊘·판정 기준 불변**(수락 ≥ 3000, tick 초과 ≤ 0.5 %, RECORDING_BACKLOG 0). r1 부하 실행(원본 data/)은 매장량 총 ~59,100 kg 로 수락 512 에서 멈춰 ⊘ 미충족 → 미검증으로 기록하고, 확대 사본 새 실행을 SC-87·89 판정으로 쓴다. 게임 데이터(designer 소유)는 바꾸지 않는다 | 리더 결정 2026-09-30 |
| 2026-09-30 | **r2 보강 (Phase 5 진입 전)** — SC-90 방법 칸의 "SQL:" 을 실행할 도구를 지명: `mining_ledger.py --world <id> discoveries`(라벨 `p1-02 SC-90`). 도구 표 §3.2 에 SC-90 추가. 판정 기준·⊘ 는 계약 칸 그대로(발견 행 수 = 채굴 광물 종류 수 ≤ 4, 고갈 ≥ 1 / 종류 ≥ 2·고갈 ≥ 1). 지명이 없으면 규칙 7 로 어떤 도구도 SC-90 을 판정할 수 없었다. **r1 도중 편집(r1 기록 시작 23:18:38Z 직전·직후), 기준 불변, 도구 지명만** — r1 의 SC-90 판정은 도구가 계약 칸과 같은 SQL 을 실행한 결과이고 이 칸의 문언 변화에 의존하지 않는다(편집 전 칸으로도 같은 SQL·같은 기준) | 리더 결정 2026-09-30 (a) |
| 2026-09-29 | **r2 보강 (Phase 4) — [DB] 이름 정정** (리더 위임): S8(기동 시 경제 상태 적재)이 계약이 SC-18 에 적은 "적재 → sim 기억 주입 경로" 를 처음 구현했다. 그 경로의 DB 테스트 이름은 `restart_reloads_economic_state_so_cas_succeeds_and_duplicates_are_rejected`(server) 이므로 SC-18 의 [DB] 이름을 그것으로 바꾼다. 옛 이름 `dup_across_restart`(server-db)는 실제로 **sim 기억 없이 재전송 → PK 정지**(마지막 방어선이 재기동을 넘어서도 선다)를 재므로 SC-21 로 옮긴다. SC-26 의 DB 층 근거로 위 재기동 테스트와 적재 함수 테스트 2 개(`load_economic_state_*`)를 더한다. 판정 기준 불변. [DB] 이름 24 → 27 | qa · 리더 2026-09-29 |
| 2026-09-29 | **r2 보강 (Phase 4)** — §0.2 에 선행 단계 0 "마이그레이션 동결 확인" 추가(ADR-0007 §5, 리더 규칙). 선언은 추적 파일 줄(architect 제안), 위치를 증거에. 판정 기준 불변 — 증거 DB 를 바꾸는 순서 위험만 막는다 | 리더·architect 2026-09-28 |
| 2026-09-28 | **r2 보강 (Phase 4)** — SC-05: ⑤⑥ 원문은 구조적 도달 불가 → 리더 방침으로 **같은 의도의 도달 가능한 반례**(데이터 파일에 소수 초 → 기동 실패)로 교체, 분모 16 유지(처음 적은 "14 + 두 짝" 안을 같은 날 대체). 스키마층 ⑬~⑯ 도 필드 이름 단언 대상임을 명시. 판정 기준(모든 §4.7 조건이 기동 실패로 막힌다) 불변 | server S2 실측 |
| 2026-09-28 | **r2 보강 (Phase 4)** — SC-40 ⊘ 에 "세션 시작 `INVENTORY_STATE`·`DEPOSIT_FIELD_STATE` 를 실제로 받았다" 선행 단언과 구조 검사(넷 다 null) 추가. architect #24 판정(ADR-0006 §4a: `SESSION_READY` 직후 두 메시지 송신)으로 첫 채굴 전 누출 표면이 확정됐기 때문. 판정 기준(누출 0 + 양성 대조) 불변 | 리더 2026-09-28 |
| 2026-09-27 | **r2 사소 정정 (Phase 4)** — SC-110 ⊘ 칸의 대조 "같은 actor_id 둘은 1" 삭제(발견 목록이 광물별이라 성립하지 않는 대조였다 — C3 검증 중 qa 발견). 판정 기준(충돌 쌍 → 발견자 둘)은 그대로 | qa |
| 2026-09-27 | **r2 확정** — server·history·client 서명(§11). 사소 정리: SC-107 은 0001~0003 적용 DB 에서만 판정(architect: `domain_events` 0001 실측 16, 0002 후 17), §11 K 행에 SC-111, Q-7 원문 주석(client 지적). **H-15 매핑은 SC-57 만이다** — 리더 지시문의 "SC-57·SC-99" 는 r1 의 제 오기(SC-57 에 "실서버판은 SC-99")를 따른 것으로 보인다. SC-99 는 §5.1a 응답 항등식이고 실서버 재기동 발견 항목은 이 계약에 없다(스펙 AC-11(g) 는 DB 통합으로 충분) | 서명 3, architect |
| 2026-09-27 | **r2** — **server**: SC-05 `reject_c01~c16`+대조, SC-20 둘째 테스트 `ambiguous_commit_retry_counts_once`, SC-25 fatal 로그 필드, SC-28 halted 상태(K5), SC-33 `CAPACITY_EXCEEDED` 재생 제외, SC-37 `required_nullable`, SC-63 겹침 정의·카운터, SC-101 DB 짝·grep 부정, SC-102 스윕 증거 = 실서버 `sessions_closed`, SC-103 → (a) 단위 + **SC-112** (b) `pg_terminate_backend`. **history**: SC-47 을 러너 DB 테스트로(코어 정렬 입력은 자명 통과였다 — ⊘ 셋), SC-95 여러 줄 패턴 + 테스트 코드 제외(거짓 PASS·거짓 FAIL 둘 다), SC-52 `P0001` + 표 이름, SC-41 participants 순서·증거 id(독립 계산값), SC-51 증거 표지 4 필드, SC-50 규칙 파일 해시, SC-59 증거 행, SC-105/106 을 AC-11(h)(ii)/(i) 로 정리 + **SC-111** (iii) 같은 근거·다른 내용, H-15 → SC-57, SC-57 의 잘못된 "SC-99 실서버판" 참조 삭제(SC-99 는 §5.1a 항등식이다 — qa 오기). **client**: 합성 입력 수락(§0.6), SC-66·72 순회 수 로그. **리더 Q-2**: §3.4, **SC-113** CI DB 테스트 이름 대조, **SC-114** 증거 DB 불변 + 헬퍼 방어. DB 테스트 표식 ` [DB]` 24 개. 항목 110 → **114**, 출처 게이트 실측 `114 / 114` | server·history·client·리더·architect 2026-09-27 |
| 2026-09-27 | **r1** — 스펙 개정(server ADR-0013 검토 K1~K6·추가, client 사전 검토, 정지 경로표) 반영: SC-09 사유 7(`CAPACITY_EXCEEDED`, 디버그·릴리스), SC-20 `AlreadyCommitted`+`persist_ambiguous_commits_total`, SC-21 PK `(world_id, command_id)`, SC-25 증거 = 정지 로그 한 줄, SC-27 시계 주입 DB 통합, SC-28·29·31·89 `recording_lag`, SC-35·66 유효 46·왕복 36, SC-70 PlayMode, SC-71 두 방향(리플렉션 + 바꿔치기). **신설 11**: SC-100 AC-4(d2) · SC-101~103 AC-5(d)(e)(f) · SC-104 AC-6(a2) · SC-105·106 AC-11(h) · SC-107 제약 분모 · SC-108 DoS 시도 실서버 · SC-109 JSONB null CHECK · SC-110 Pilot 표지 충돌 쌍. §0.11 대조 키 = `actor_id`, §0.12 정지 실행은 짝 검사 제외 목록. Q-1 승인(알 수 없는 표지 exit 4), Q-3 → AC-11(h). 항목 99 → **110**, 출처 게이트 실측 `110 / 110` | architect·server·client·리더 2026-09-27 |
| 2026-09-27 | 초안 r0 — AC-1~19 를 SC-01~99 로, H-01~H-14·S-1~S-10 매핑, 새 월드 절차, 기준선 실측(RED 5건 — 리더 실측 4건 + `schemas_valid_offline`), 오라클 필드 이름 정정, 출처 게이트 슬라이스 표지 제안, 유령 지명 검사 사망 발견 | 스펙 agreed, 리더 지시 2026-09-27 |

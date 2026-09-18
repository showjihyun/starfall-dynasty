---
name: integration-qa
description: "STARFALL DYNASTY 통합 QA·평가 절차. 스프린트 계약(구현 전 완료 기준) 작성, 모듈 완료 직후의 점진적 경계면 교차 검증(계약 스키마↔Rust serde↔C# DTO, REST·WebSocket↔Unity 클라이언트, 도메인 이벤트↔역사 판정↔프로젝션↔Chronicle UI), 게임 특화 위험 점검(서버 판정 우회, 화폐·아이템 복사, 멱등성, 결정성, 기록 불변, 가시성 누출), 계약 커버리지 스크립트, 봇 시나리오 테스트, 증거 기반 평가 리포트 형식을 정의한다. 검증, QA, 테스트 실행, 통합 점검, 평가, 회귀 확인, 스프린트 계약, '제대로 동작하는지 확인해줘' 요청 시 반드시 사용."
---

# Integration QA — 양쪽을 함께 읽고, 실제로 실행해 본 것만 통과시킨다

버그는 대부분 두 모듈이 만나는 곳에서 생긴다. 서버도 "맞게" 짰고 클라이언트도 "맞게" 짰는데 서로 다른 것을 기대하는 경우다. 각자 따로 검증하면 둘 다 통과한다. 그래서 이 스킬의 핵심은 **생산자와 소비자를 동시에 열어 비교하는 것**과 **실행 증거**다.

## 1. 언제 무엇을 하나

| 시점 | 할 일 | 산출물 |
|------|------|-------|
| Phase 3 (구현 전) | 스펙 수용 기준 → 실행 가능한 검증 항목. 구현자 확인 받기 | `02_sprint_contract.md` |
| Phase 4 (모듈 완료 알림마다) | 해당 모듈의 경계면만 즉시 교차 검증, 결과를 구현자에게 전송 | 메시지 (심각하면 리포트에 기록) |
| Phase 5 (평가) | 계약 전 항목 실행 평가, FAIL → 수정 요청 → 재평가 (최대 3라운드) | `04_qa_report_r{N}.md` |
| VERIFY 유형 | 현재 코드 전체에 5·6절 점검 | `_workspace/verify-{YYYYMMDD}/04_qa_report_r1.md` |

## 2. 스프린트 계약 형식 — `02_sprint_contract.md`

```markdown
# {slice-id} 스프린트 계약

- 스펙: docs/specs/{slice-id}.md
- 합의: server ✅ history ✅ client ✅ techart — (미투입)

| ID | 검증 항목 | 검증 방법 (재현 가능한 명령·테스트·관찰) | 담당 | 근거 AC |
|----|----------|--------------------------------------|------|--------|
| SC-01 | 같은 command_id의 MINE_RESOURCE 2회 전송 시 인벤토리 1회만 증가 | `cargo test -p sim mining_idempotent` + 봇 시나리오 `tools/bots mining_retry` | server | AC-2 |
| SC-02 | 최초 발견 시 MINERAL_DISCOVERED 역사 이벤트 정확히 1건, rule_version 기록 | `cargo test -p history first_discovery_once` | history | AC-3 |
| SC-03 | 모든 신규 계약 fixture를 Rust·C# 양쪽이 역직렬화 | `cargo test -p contracts fixtures` + `unity test client --mode EditMode --filter Contracts` | server, client | AC-5 |
| SC-04 | 계약 커버리지 스크립트 오류 0 | `python .claude/skills/integration-qa/scripts/check_contract_coverage.py --strict` | qa | — |
```

좋은 검증 항목의 조건:
- **관찰 가능한 결과**를 말한다 ("잘 동작한다" ✗ → "DB `historical_events`에 1행, `rule_version = detector@1`" ✓).
- 누가 실행해도 같은 결과가 나오는 방법이 있다. Editor 수동 확인이 필요한 항목은 무엇을 보면 되는지 적는다.
- 스펙의 수용 기준과 연결된다. 스펙에 없는 요구를 계약에서 새로 만들지 않는다.

## 3. 경계면 교차 검증 — 이 프로젝트의 경계면

각 행은 **왼쪽과 오른쪽을 같이 읽고** 비교한다. 한쪽만 존재를 확인하는 것은 검증이 아니다.

| 경계면 | 생산자 (왼쪽) | 소비자 (오른쪽) | 비교할 것 |
|-------|-------------|---------------|----------|
| 계약 ↔ Rust | `contracts/**/*.schema.json`, `registry/types.json` | `server/crates/contracts` serde 타입 | 필드명·필수 여부·타입·열거값·`rename_all` |
| 계약 ↔ C# | 같은 스키마 | `client/.../Contracts/Generated`, `[JsonProperty]` | 필드명 매핑 누락, nullable 처리 |
| REST | Axum Router 경로·메서드·상태 코드·응답 타입 | 클라이언트 `IApiClient` 호출 URL·메서드·응답 파싱 | 경로 오타, 래핑 응답(`{items: []}`) vs 배열 기대, 에러 코드 처리 |
| WebSocket | 서버 송신 메시지 enum 전체 | 클라이언트 디스패처 등록 목록 | 처리되지 않는 메시지 타입, 모르는 타입 처리 |
| 명령 | 클라이언트 명령 생성부 | 서버 명령 역직렬화·검증 | `command_id` 재사용 규칙, 서버가 클라이언트 값을 믿는 필드 |
| 도메인 → 역사 | 서버가 발행하는 도메인 이벤트 타입 목록 | Historical Detector의 match 분기 | 판정 대상인데 분기 없음, 분기 있는데 발행 안 됨 |
| 역사 → 프로젝션 → UI | 역사 이벤트 타입 | Chronicle/Biography 프로젝션 핸들러 → 클라이언트 렌더러 | 프로젝션 누락, `headline_key` 로컬라이즈 누락 |
| DB ↔ 코드 | 마이그레이션 컬럼·제약 | sqlx 쿼리 구조체, API DTO | 컬럼명·nullable·타입 불일치, 누락된 UNIQUE/CHECK |
| 즉시 응답 ↔ 최종 결과 | `COMMAND_RESULT: accepted` | 클라이언트가 인벤토리를 언제 갱신하나 | 접수를 완료로 취급 |
| 상태 머신 | 스펙의 상태 전이 (예: 계약 SIGNED → BROKEN) | 코드의 모든 상태 변경 지점 | 정의되지 않은 전이, 도달 불가 상태 |

계약 레지스트리와 코드의 1차 대응은 스크립트로 확인하고(8절), 필드 수준 비교는 직접 읽는다.

## 4. 게임 특화 위험 점검

| 위험 | 점검 방법 |
|------|----------|
| **서버 판정 우회** | 명령 payload 중 위치·수량·가격·피해량·잔액처럼 서버가 계산해야 할 값을 서버가 그대로 쓰는 곳을 찾는다. 명령 처리기가 판정 8단계(`rust-authoritative-server` §3)를 따르는지 대조 |
| **화폐·아이템 복사** | 트랜잭션 밖 read-modify-write, 조건 없는 UPDATE, 한쪽만 커밋되는 거래 경로 검색. 원장 합계 = 잔액 합계 속성 테스트 존재·통과 확인. 동시 요청 두 개로 같은 아이템 판매 시나리오 |
| **멱등성** | 같은 명령 2회, 같은 도메인 이벤트 2회 처리 테스트. `processed_commands`/`processed_events` 삽입이 트랜잭션 첫 단계인지 |
| **결정성** | 시뮬레이션·역사 크레이트에서 `SystemTime`, `Instant`, `thread_rng`, `HashMap` 순회 의존 검색. 같은 시드·명령열 두 번 실행 → 상태 해시 비교 테스트 |
| **기록 불변** | 역사·도메인 이벤트·증거 테이블에 UPDATE/DELETE 경로가 코드에 없는지, DB 트리거로 막히는지 실제로 시도 |
| **Fact/Claim 혼합** | 플레이어 입력이 `historical_events`나 `fact_status`로 들어가는 경로 검색 |
| **가시성 누출** | 비공개(`SECRET`, `CLASSIFIED`, `PARTICIPANTS_ONLY`) 사건이 다른 플레이어의 조회·검색·Chronicle·WebSocket 브로드캐스트로 새는지 |
| **LLM 사실 생성** | LLM 호출 결과가 canonical 테이블에 쓰이는 경로 |
| **클라이언트 확정 계산** | 클라이언트가 서버 결과 전에 인벤토리·잔액을 바꾸는 코드 |

## 5. 실행 평가

가능한 것은 모두 실제로 실행하고, 명령과 결과 요약을 리포트에 붙인다.

```bash
# 계약 대응 (항상 실행 가능 — Python 표준 라이브러리만 사용)
python .claude/skills/integration-qa/scripts/check_contract_coverage.py --strict

# 서버
cargo fmt --check --manifest-path server/Cargo.toml
cargo clippy --manifest-path server/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path server/Cargo.toml --workspace

# 로컬 인프라가 필요한 통합 테스트
docker compose up -d && docker compose ps

# 클라이언트
unity test client --mode EditMode --report-format junit --output _workspace/{slice-id}/unity-tests/editmode.xml
unity test client --mode PlayMode --report-format junit --output _workspace/{slice-id}/unity-tests/playmode.xml
```

환경이 없어 실행하지 못한 항목은 "미검증(환경)"으로 두고 필요한 설치를 적는다. 정적 코드 읽기만으로 실행 항목을 PASS 처리하지 않는다.

### 버티컬 슬라이스 시나리오 (봇)

기획안의 최우선 검증 장면(GDD §55)을 봇 테스트로 만든다. `tools/bots/`에 Rust 봇 클라이언트를 두고 실제 WebSocket/REST로 서버에 붙는다.

```
봇 A: 희귀 광물 발견 → 채굴 → 운송 출발
봇 B: A 추적 → 공격 → A 함선 파괴 → 화물 획득
검증: SHIP_DESTROYED·PLAYER_DIED 역사 이벤트, 증거(함선 로그) 생성, Chronicle 항목 존재
봇 C: (게임 시간 경과 후) 사건 현장 조사 → A의 함선 로그 발견 → Claim 게시
검증: C의 지식 상태에 증거 추가, Claim이 Fact로 승격되지 않음
```

슬라이스 범위에 해당하는 구간만 구현하고, 범위가 넓어질 때마다 이어 붙인다. designer의 재미 검증 시나리오가 이 봇 테스트의 원본이다.

## 6. 평가 리포트 형식 — `04_qa_report_r{N}.md`

```markdown
# {slice-id} QA 리포트 — 라운드 {N}

- 일시: {YYYY-MM-DD HH:MM}
- 기준: 02_sprint_contract.md
- 요약: PASS {p} / FAIL {f} / 미검증(환경) {u} / 전체 {t}

## 항목별 결과
| ID | 결과 | 증거 | 비고 |
|----|------|------|------|
| SC-01 | PASS | `cargo test -p sim mining_idempotent` → 1 passed | |
| SC-02 | FAIL | server/crates/history/src/detector/mining.rs:42 — 같은 source_event_id로 2건 생성 | 수정 요청 → history |
| SC-03 | 미검증(환경) | Unity CLI 없음 | 설치 후 `unity test client --mode EditMode` |

## 수정 요청
### SC-02 → history
- 위치: 파일:라인
- 재현: 명령 또는 입력
- 기대: 스프린트 계약 문구
- 실제: 관찰 결과

## 경계면 점검 결과 (3절)
| 경계면 | 결과 | 메모 |

## 게임 특화 위험 (4절)
| 위험 | 결과 | 메모 |

## 계약 외 발견
스프린트 계약에 없지만 중요한 문제. 판정에 넣지 않고 리더가 다음 작업으로 판단하게 한다.

## 이전 라운드 대비
FAIL → PASS로 바뀐 항목, 새로 깨진 항목(회귀).
```

판정 규칙:
- **PASS**는 증거가 있을 때만. 증거는 명령과 결과 요약, 테스트 이름, 또는 파일:라인.
- 간헐적으로 실패하는 테스트는 PASS가 아니다. 비결정성 이슈로 FAIL 처리한다.
- 3라운드가 끝났는데 FAIL이 남으면 원인 추정과 선택지(범위 축소, 스펙 수정, 추가 라운드)를 리더에게 보고한다.

## 7. 수정 요청 방식

- 구현 코드를 직접 고치지 않는다. 담당자에게 파일:라인, 재현, 기대/실제를 보낸다. 평가자가 고치면 같은 사람이 만들고 평가하는 구조가 되어 평가 의미가 사라진다.
- 경계면 불일치는 **생산자와 소비자 양쪽**에 보내고, 계약 자체가 모호하면 architect에게도 보낸다.
- 테스트 코드(`tests/e2e/`, `tools/bots/`)는 qa 소유이므로 직접 작성·수정한다.

## 8. 계약 커버리지 스크립트

`scripts/check_contract_coverage.py` — 레지스트리(`contracts/registry/types.json`)를 기준으로 다음을 검사한다.
1. 각 타입의 스키마 파일 존재·JSON 파싱, 스키마의 타입 상수가 이름과 일치
2. 각 타입의 fixture 1개 이상 존재·파싱, fixture의 타입 필드가 이름과 일치
3. `producers`/`consumers` 태그별로 코드에 타입이 등장하는지 (`SCREAMING_SNAKE` 이름, 또는 `PascalCase` 식별자 — `MineResourceCommand`처럼 대문자로 시작하는 접미사 허용)
   - `server`: 경로에 `history`가 없는 `server/**/*.rs` / `history`: 경로에 `history`가 있는 `server/**/*.rs` / `client`: `client/Assets/**/*.cs` / `bots`: `tools/bots/**/*.rs`
4. 레지스트리에 없는 스키마 파일(고아)

```bash
python .claude/skills/integration-qa/scripts/check_contract_coverage.py            # 코드 폴더가 없으면 경고
python .claude/skills/integration-qa/scripts/check_contract_coverage.py --strict   # 경고도 실패로 (Phase 5 평가용)
python .claude/skills/integration-qa/scripts/check_contract_coverage.py --json     # 기계 판독용
```

종료 코드: 0 = 오류 없음, 1 = 오류 있음, 2 = 레지스트리 없음·파싱 실패. 이 스크립트는 "코드에 이름이 등장한다"까지만 본다. 필드 수준 일치는 3절 방법으로 직접 비교한다.

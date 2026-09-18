---
name: starfall-dev
description: "STARFALL DYNASTY 게임 개발 에이전트 팀 오케스트레이터. 기획안 기반 기능 구현, 버티컬 슬라이스, 마일스톤/Phase 진행, 프로젝트 초기 세팅(부트스트랩), 스펙·설계, 서버(Rust)·클라이언트(Unity)·역사 엔진·3D 렌더링 작업, 통합 검증을 팀으로 조율한다. '채굴 기능 만들어줘', 'Chronicle 구현', 'Phase 1 시작', '현상금 시스템 설계', '다음 슬라이스 진행'처럼 게임 개발을 요청하면 반드시 이 스킬을 사용할 것. 후속 작업도 포함: 슬라이스 다시 실행, 이어서 진행, 수정, 보완, 업데이트, QA 다시, '서버 부분만 다시', '이전 결과 기반으로 개선', 실패 항목 재작업 요청 시에도 사용. 단순 개념 질문이나 한 줄 수정에는 쓰지 않는다."
---

# STARFALL Dev Orchestrator

기획안을 **스펙 → 완료 기준 합의 → 병렬 구현 + 점진 QA → 실행 기반 평가 루프**로 플레이 가능한 슬라이스로 만든다. 이 스킬을 실행하는 메인 세션이 **팀 리더**다. 리더는 조율·판단·보고를 하고, 직접 구현하지 않는다. 리더가 구현을 시작하면 팀원과 파일이 충돌하고 평가의 독립성이 깨진다.

## 실행 모드: 하이브리드 (에이전트 팀 기본)

| 단계 | 모드 | 이유 |
|------|------|------|
| 스펙·계약 (Phase 2) | 에이전트 팀 | 기획 의도와 기술 제약을 실시간으로 조율해야 한다 |
| 스프린트 계약 (Phase 3) | 에이전트 팀 | 구현자와 평가자가 "완료"의 정의를 직접 합의한다 |
| 구현 + 점진 QA (Phase 4) | 에이전트 팀 | 서버·역사·클라이언트가 계약 질문을 서로 직접 주고받는다 |
| 최종 게이트 (Phase 5, 선택) | 서브 에이전트 | 팀 대화에 영향받지 않은 독립 평가 |
| 단일 영역·검증·버그 작업 | 서브 에이전트 | 팀 통신이 필요 없는 집중 작업 |

## 에이전트 구성

| 팀원 이름 | 에이전트 정의 (`subagent_type`) | 역할 | 주 스킬 | 주 산출물 |
|----------|------------------------------|------|--------|----------|
| `architect` | game-architect | 스펙, ADR, 계약, 작업 분해 | starfall-spec, event-contracts | `docs/specs/`, `docs/adr/`, `contracts/`, `01_architect_tasks.md` |
| `designer` | game-designer | 규칙, 수치, 데이터, 재미 시나리오 | starfall-spec | `docs/design/`, `data/` |
| `server` | rust-server-engineer | 시뮬레이션, 서버 판정, DB, API | rust-authoritative-server | `server/`(history 제외) |
| `history` | history-engine-engineer | 역사 판정, 증거·주장, 프로젝션 | historical-engine | `server/**/history/**` |
| `client` | unity-client-engineer | 네트워킹, 게임플레이 표현, UI | unity-client + Unity 공식 스킬 | `client/Assets/_Project/{Scripts,UI,Tests}` |
| `techart` | unity-tech-artist | 우주 렌더링, 함선 비주얼, VFX, 성능 | space-3d-rendering + Unity 공식 스킬 | `client/Assets/_Project/{Art,Shaders,VFX,Rendering}` |
| `qa` | qa-integration-engineer | 스프린트 계약, 경계면 검증, 실행 평가 | integration-qa | `02_sprint_contract.md`, `04_qa_report_r{N}.md`, `tests/e2e/`, `tools/bots/` |

모든 에이전트 호출에 `model: "opus"`를 지정한다.

## 파일 소유권

두 팀원이 같은 파일을 고치면 덮어쓰기가 생긴다. 소유자만 수정하고, 나머지는 소유자에게 SendMessage로 요청한다.

| 경로 | 소유자 |
|------|-------|
| `docs/specs/`, `docs/adr/`, `contracts/` | architect |
| `docs/design/`, `data/` | designer |
| `server/` (history 모듈 제외), `server/migrations/`(역사 테이블 제외), `docker-compose.yml` | server |
| `server/**/history/**`, 역사 테이블 마이그레이션 | history |
| `client/Assets/_Project/{Scripts,UI,Tests}`, 게임플레이 씬 | client |
| `client/Assets/_Project/{Art,Shaders,VFX,Rendering}` | techart |
| `tests/e2e/`, `tools/bots/`, `_workspace/**/02_*`, `_workspace/**/04_*` | qa |
| `_workspace/STATUS.md`, `_workspace/{slice}/00_*`, `05_*` | 리더 |

레포 구조는 부트스트랩 슬라이스의 ADR-0001에서 확정한다. ADR이 경로를 바꾸면 이 표를 함께 갱신한다.

## 작업 유형 라우팅

| 유형 | 신호 | 실행 | 투입 |
|------|------|------|------|
| **BOOT** 부트스트랩 | `server/`·`client/`·`contracts/`가 없음, "초기 세팅", "Phase 0" | 팀, Phase 2~6 | architect, server, client, qa |
| **SLICE** 기능 슬라이스 | 기획안 기능 구현, "버티컬 슬라이스", "다음 슬라이스" | 팀, Phase 2~6 | 변경되는 레이어의 담당자만 (아래 규칙) |
| **SPEC** 설계만 | "설계해줘", "스펙", "기획 구체화" | 팀, Phase 2만 | architect, designer (+history) |
| **SOLO** 단일 영역 | 한 레이어 안의 작은 작업 ("워프 셰이더만", "outbox 워커 수정") | 서브 에이전트 1명 | 해당 전문가 → 끝나면 qa 서브 에이전트로 경계 점검 |
| **VERIFY** 검증만 | "점검", "QA", "테스트 돌려" | 서브 에이전트 | qa |
| **BUG** 버그 | "안 돼", "에러", "크래시" | 서브 에이전트 + `diagnose` 스킬 | 해당 영역 전문가. 경계면 버그로 보이면 양쪽 담당 + qa로 팀 구성 |

SLICE 팀원 선택 규칙: 서버 로직이 바뀌면 `server`, 역사 이벤트·증거·연대기가 걸리면 `history`, 화면·조작이 바뀌면 `client`, 새 비주얼·VFX·렌더 성능이 걸리면 `techart`. `architect`와 `qa`는 항상 포함한다. 동시에 활성인 팀원은 3~6명으로 유지한다.

## 워크플로우

### Phase 0: 컨텍스트 확인

1. `_workspace/STATUS.md`를 읽는다. 없으면 초기 상태다.
2. 요청이 가리키는 슬라이스를 특정하고 `_workspace/{slice-id}/` 존재 여부로 모드를 정한다.
   - **없음** → 초기 실행. Phase 1로.
   - **있음 + 부분 수정·재작업 요청**("서버만 다시", "QA 실패 항목 고쳐") → 부분 재실행. 관련 팀원만 스폰하고, 스폰 프롬프트에 이전 산출물 경로와 사용자 피드백을 넣는다. 해당 Phase부터 재개.
   - **있음 + 같은 슬라이스를 새 요구로 다시** → 기존 폴더를 `_workspace/_archive/{slice-id}_{YYYYMMDD_HHMMSS}/`로 옮기고 초기 실행.
   - **있음 + "이어서 진행"** → STATUS.md의 마지막 Phase부터 재개.
3. 세션을 `/resume`으로 이어받았다면 이전 팀원은 복원되지 않는다. 필요한 팀원을 새로 스폰하고 `_workspace/` 파일로 맥락을 넘긴다.
4. 에이전트 팀 사용 가능 여부를 확인한다: Agent 도구에 `name` 파라미터가 있고 SendMessage·Task 도구가 보이면 팀 모드. 없으면 **서브 에이전트 대체 모드**(아래)로 같은 Phase 구조를 수행한다.

### Phase 1: 요청 분석·범위 게이트

1. 작업 유형을 라우팅 표로 분류한다.
2. `starfall-spec` 스킬의 **Phase 범위 게이트**로 요청이 현재 로드맵 단계에 맞는지 확인한다. 맞지 않으면 조용히 거절하지 말고, 사용자에게 현재 단계에 맞는 축소안을 제시해 확인받는다.
3. 슬라이스 ID를 정한다: `p{로드맵 Phase}-{두 자리 순번}-{kebab}` (예: `p0-01-bootstrap`, `p1-03-mining`).
4. `_workspace/{slice-id}/00_request.md`에 사용자 요청 원문, 분류, 범위 판단, 투입 팀원을 기록하고 `_workspace/STATUS.md`에 행을 추가한다.

### Phase 2: 스펙·계약 — 에이전트 팀

1. 스폰 대상: SLICE·SPEC은 `architect`, `designer`(역사 관련이면 `history`도). BOOT는 `architect`만 먼저 스폰하고, ADR 초안이 나오면 `server`·`client`를 스폰해 레포 구조·툴체인 ADR을 검토하게 한다 (게임 규칙이 없으므로 designer 불필요). 스폰 프롬프트는 `references/spawn-prompts.md` 템플릿을 채운다. 팀원은 리더의 대화 기록을 받지 못하므로 슬라이스 ID, 읽을 파일, 소유 경로, 다른 팀원 이름을 반드시 넣는다.
2. TaskCreate로 태스크를 등록한다. SLICE 기준: ① 스펙 초안(architect) ② 규칙·데이터·재미 시나리오(designer) ③ 계약 스키마·fixture(architect, ①에 의존) ④ 역사 규칙 검토(history, ②에 의존) ⑤ 태스크 분해 `01_architect_tasks.md`(architect, ③④에 의존). BOOT 기준: ① ADR-0001~0003 초안(architect) ② ADR 검토(server, client, ①에 의존) ③ `contracts/` 골격과 envelope 스키마(architect) ④ 태스크 분해(architect, ②③에 의존).
3. 팀원이 SendMessage로 조율하는 동안 리더는 기다린다. 스펙의 "열린 질문" 중 사용자 결정이 필요한 것은 모아서 한 번에 묻는다.
4. **완료 조건:** `docs/specs/{slice-id}.md`, `contracts/` 변경, `01_architect_tasks.md` 존재. SPEC 유형은 여기서 Phase 6으로 간다.
5. 구현에 더 필요 없는 `designer`에게 종료를 요청한다 (밸런스 조정이 이어지면 유지).

### Phase 3: 스프린트 계약 — 에이전트 팀

Anthropic 하네스 설계 원칙: **코드를 쓰기 전에 구현자와 평가자가 "완료"가 무엇인지 합의한다.** 스펙은 넓고, 계약은 이번 작업에서 증명할 검증 항목이다.

1. 투입할 구현 팀원(`server`/`history`/`client`/`techart`)과 `qa`를 스폰한다.
2. `qa`가 스펙 수용 기준을 검증 항목으로 바꾼 `02_sprint_contract.md` 초안을 쓴다 (형식: `integration-qa` 스킬).
3. 각 구현자는 자기 항목을 읽고 "이 방법으로 완료를 증명할 수 있다"거나 수정안을 `qa`에게 SendMessage로 보낸다. 합의가 안 되면 `architect`가 스펙 기준으로 결정한다.
4. **완료 조건:** 모든 항목에 담당자·검증 방법이 있고, 구현자 전원이 확인한 계약.

### Phase 4: 구현 + 점진 QA — 에이전트 팀

1. `01_architect_tasks.md`의 태스크를 TaskCreate로 등록한다 (담당자, 선행 태스크 포함). 계약이 확정되어 있으므로 서버·역사·클라이언트·테크아트는 병렬로 진행한다. 클라이언트는 서버가 준비되기 전에 `contracts/fixtures/`로 개발한다.
2. 팀원 간 통신 규칙:
   - 계약 변경이 필요하면 → `architect`에게 요청. architect는 수정 후 모든 소비자에게 알린다.
   - 모듈 하나가 끝나면 → `qa`에게 "모듈 완료: 경로, 실행 방법" 알림. `qa`는 즉시 해당 경계면을 검증한다.
   - 서버 엔드포인트·메시지가 준비되면 → `server`가 `client`에게 목록과 예시 payload를 보낸다.
3. 리더는 TaskList로 진행을 보고, 멈춘 팀원에게 SendMessage로 상태를 묻거나 태스크를 재할당한다. 팀원이 태스크 완료 처리를 빠뜨려 의존 태스크가 막히면 실제 완료 여부를 확인 후 TaskUpdate로 정리한다.
4. 각 구현자는 `03_{server|history|client|techart}_impl.md`를 남긴다.
5. **완료 조건:** 구현 태스크 전부 완료 + 각 구현자의 자체 테스트 통과 보고.

### Phase 5: 평가 루프

1. `qa`가 스프린트 계약 전 항목을 실행 평가해 `04_qa_report_r1.md`를 쓴다.
2. FAIL 항목은 `qa`가 담당 구현자에게 파일:라인·재현·기대 동작과 함께 보낸다. 구현자가 고치면 `qa`가 `r2`로 재평가한다. **최대 3라운드.**
3. 3라운드 후에도 FAIL이 남으면 리더가 남은 항목, 원인 추정, 선택지(범위 축소·스펙 수정·추가 라운드)를 사용자에게 보고하고 결정을 받는다.
4. **선택 — 독립 최종 게이트:** 로드맵 Phase 종료 기준 슬라이스나 사용자가 요청한 경우, 팀 대화에 참여하지 않은 `qa-integration-engineer`를 이름 없이(서브 에이전트로) 호출해 계약 전체를 다시 평가한다. 팀 QA와 결과가 다르면 둘 다 보고한다.
5. **완료 조건:** 계약 항목 전부 PASS, 또는 사용자가 남은 FAIL/미검증 항목을 수용.

### Phase 6: 정리·보고·피드백

1. 남은 팀원 전원에게 SendMessage로 종료를 요청한다. 세션 종료 시 팀 디렉토리는 자동 정리된다.
2. `_workspace/{slice-id}/05_summary.md`를 쓴다: 구현된 것, 증거(QA 리포트 경로), 미검증 항목과 필요한 환경, 기술 부채, 다음 슬라이스 추천.
3. `_workspace/STATUS.md`의 슬라이스 상태를 갱신한다. `_workspace/`는 지우지 않는다 (감사·재개용).
4. 사용자에게 요약을 보고하고 피드백을 한 번 묻는다: "결과에서 개선할 점이나, 팀 구성·진행 방식에서 바꾸고 싶은 점이 있나요?" 피드백이 하네스 수정으로 이어지면 `harness` 스킬의 진화 절차를 따르고 CLAUDE.md 변경 이력에 기록한다.

## 에이전트 팀 사용법 (Claude Code v2.1.178 이후)

- `TeamCreate`/`TeamDelete`는 더 이상 없다. `CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS=1`이면 **Agent 도구에 `name`을 주어 호출하는 것이 곧 팀원 스폰**이다: `Agent({ name: "server", subagent_type: "rust-server-engineer", model: "opus", description: "...", prompt: "..." })`. `isolation`을 주면 팀원이 아니게 되므로 팀원에는 쓰지 않는다.
- 이름 없이 호출한 Agent는 일반 서브 에이전트다 (독립 최종 게이트, SOLO/VERIFY/BUG 유형에 사용).
- 메시지: `SendMessage({ to: "<팀원 이름>", message: "..." })`. 전원 브로드캐스트는 없으니 수신자마다 보낸다.
- 공유 태스크: TaskCreate / TaskList / TaskGet / TaskUpdate. 선행 태스크가 끝나야 의존 태스크를 claim할 수 있다.
- 팀원이 턴을 마치면 리더에게 유휴 알림과 최종 답이 자동으로 온다. 폴링하지 않는다.
- 세션당 팀은 하나이고 팀원은 팀원을 만들 수 없다. Phase 간 구성 변경은 "필요 없는 팀원 종료 → 새 팀원 스폰"으로 한다.
- 팀원 권한 요청은 리더 세션에 뜬다. Windows Terminal에서는 split-pane 모드를 쓸 수 없으니 기본 in-process 모드를 쓴다.

## 서브 에이전트 대체 모드

팀 도구가 없는 세션(환경 변수 미적용, `-p` 비대화형 실행 등)에서는 같은 Phase를 서브 에이전트로 수행한다.
- 팀원 간 메시지는 파일로 대체한다: `_workspace/{slice-id}/messages.md`에 `[from → to] 내용`을 추가하고, 리더가 다음 호출 프롬프트에 해당 메시지를 넣어 중계한다.
- Phase 2~3은 순차 호출(architect → designer → architect 계약 확정 → qa 계약 초안 → 구현자 확인 1회 일괄 호출), Phase 4는 구현자를 `run_in_background: true`로 병렬 호출, 완료마다 qa를 호출한다.
- 산출물 경로·형식·평가 루프(최대 3라운드)는 팀 모드와 동일하다.

## 데이터 흐름

```
사용자 요청 → [리더] 00_request.md, STATUS.md
  → Phase 2 [architect ⇄ designer ⇄ history] → docs/specs, docs/design, data/, contracts/, 01_architect_tasks.md
  → Phase 3 [qa ⇄ 구현자들] → 02_sprint_contract.md
  → Phase 4 [server ⇄ history ⇄ client ⇄ techart] ─모듈 완료→ [qa 점진 검증]
             → server/, client/, 03_*_impl.md
  → Phase 5 [qa] → 04_qa_report_r{N}.md ─FAIL→ 담당 구현자 → 재평가 (≤3)
  → Phase 6 [리더] → 05_summary.md, STATUS.md, 사용자 보고
```

## 에러 핸들링

| 상황 | 전략 |
|------|------|
| 팀원 1명 중단·오류 | SendMessage로 상태 확인 → 같은 이름으로 재스폰(이전 산출물 경로 포함). 재실패 시 태스크를 다른 적임자에게 재할당하거나 해당 범위를 요약에 누락으로 명시 |
| 팀원 과반 실패 | 중단하고 사용자에게 알린 뒤 진행 여부 확인 |
| 툴체인 없음 (Rust, Unity CLI/Editor, Docker) | 코드·테스트 작성은 진행, 실행 검증은 "미검증(환경)"으로 분리. 설치 필요 목록을 사용자에게 보고. 통과로 간주하지 않는다 |
| 계약 충돌 (서버·클라이언트가 다르게 이해) | architect가 계약 기준으로 판정, 양쪽에 통지. 데이터·의견을 지우지 않고 근거를 병기 |
| 범위 밖 요청 | Phase 1에서 축소안 제시 후 사용자 확인 |
| 절대 원칙 위반 구현 발견 | qa가 FAIL 처리, architect가 대안 제시. 원칙 변경이 필요하면 ADR + 사용자 결정 |
| QA 3라운드 초과 | Phase 5-3 절차로 사용자 결정 |
| 태스크 상태 지연 | 실제 산출물 확인 후 리더가 TaskUpdate |

## 테스트 시나리오

### 정상 흐름 — "Phase 1 채굴 기능 구현해줘"
1. Phase 0: `_workspace/p1-02-mining/` 없음 → 초기 실행. 팀 도구 확인됨.
2. Phase 1: SLICE로 분류, Phase 1 범위(채굴·인벤토리) 안 → `p1-02-mining`. 투입: architect, designer, server, history(최초 발견 → MINERAL_DISCOVERED), client, qa.
3. Phase 2: 스펙, 광물 데이터 10종, `MINE_RESOURCE`(명령)/`MINERAL_MINED`/`MINERAL_DISCOVERED` 계약과 fixture, 태스크 분해. designer 종료.
4. Phase 3: qa가 "같은 채굴 명령 2회 전송 시 인벤토리 1회만 증가", "최초 발견 시 Historical Event 1건", "클라이언트 fixture 역직렬화" 등 계약 초안 → 구현자 확인.
5. Phase 4: 병렬 구현, 모듈 완료마다 qa 경계면 검증.
6. Phase 5: r1에서 멱등성 1건 FAIL → server 수정 → r2 전부 PASS.
7. Phase 6: 팀원 종료, `05_summary.md`, STATUS 갱신, 보고 + 피드백 요청.

### 에러 흐름 — 툴체인 없음 + 팀원 중단
1. Phase 4에서 `server`가 "cargo 없음"으로 멈춤 → 리더가 사용자에게 Rust 설치 필요를 알리고, 코드 작성은 계속하도록 지시.
2. `client`가 오류로 중단 → 같은 이름으로 재스폰, `03_client_impl.md`와 TaskList로 재개.
3. Phase 5: 실행 항목은 "미검증(환경)", 정적 교차 비교와 계약 커버리지 스크립트 항목만 판정.
4. Phase 6 요약에 미검증 항목과 설치 후 재검증 명령을 명시. 슬라이스 상태는 `done`이 아니라 `needs-env-verification`.

### 후속 흐름 — "p1-02 QA 실패한 것만 다시 해줘"
1. Phase 0: 폴더 존재 + 부분 재작업 → 최신 `04_qa_report_r{N}.md`의 FAIL 담당자와 `qa`만 스폰.
2. Phase 5부터 재개, 리포트 번호 이어서 `r{N+1}`.

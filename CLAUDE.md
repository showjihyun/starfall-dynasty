# STARFALL DYNASTY

플레이어의 행동이 역사 기록으로 쌓이고, 그 기록이 다음 플레이어의 콘텐츠가 되는 3D 우주 멀티플레이 게임.
Unity 6 클라이언트 + Rust 서버(서버가 최종 판정) + PostgreSQL. 버티컬 슬라이스("한 성계, 한 역사")부터 만든다.

**제품 의도의 원천:** `기획안/` 3개 문서. 문서를 통째로 읽지 말고, `starfall-spec` 스킬의 `references/gdd-index.md`에서 필요한 절을 찾아 그 부분만 읽는다.

## 절대 원칙 (위반 시 설계를 멈추고 ADR로 논의)

1. 클라이언트는 절대 권위자가 아니다 — 돈·인벤토리·위치·전투 결과는 서버가 검증·결정한다.
2. LLM은 게임의 사실(canonical state)을 만들지 않는다 — 요약·뉴스·검색 등 표현 계층에서만 쓴다.
3. Redis는 진실의 원천이 아니다 — 영속 데이터는 PostgreSQL.
4. 모든 이벤트가 역사적 사건은 아니다 — Historical Event는 중요도 판정을 통과한 것만.
5. 과거 기록은 수정하지 않는다 — 정정·위조·재해석은 새 레코드로 남긴다.
6. Fact ≠ Claim ≠ Interpretation — DB와 코드에서 분리한다.
7. 마이크로서비스·Kubernetes·전면 ECS·전면 이벤트 소싱으로 시작하지 않는다 — 모듈형 모놀리스부터.
8. 30~100명으로 재미를 증명하기 전에 대규모 최적화를 하지 않는다.
9. 역사 시스템은 결정적(deterministic)이고 감사 가능해야 한다 — 같은 입력이면 같은 결과, `rule_version` 기록.
10. 큰 기술 선택은 프로파일링·부하 테스트로 검증한다.

## 검증의 규율

**IMPORTANT: 관찰이 겨냥한 조건이 실제로 발생했는지를 함께 단언하지 않으면 초록불은 아무것도 뜻하지 않는다.** 카운터 항등식은 입력이 전부 0일 때 반드시 실패해야 한다.

p1-01에서 같은 형태로 네 번 데였다: `world_full` 게이트가 만들어지고 한 번도 실행되지 않음 · `SET_SHIP_CONTROL`이 게이트웨이에서 전부 거부되는데 테스트 154개가 초록 · 봇 짝 게이트가 `0 == 0`으로 통과 · flaky 테스트가 서버 지연이 아니라 자기 폴링 비용을 쟀음. **통과한 테스트 수는 경로가 실행됐다는 증거가 아니다.**

## 하지 말 것 (되돌릴 수 없다)

- **`docker compose down -v` 금지.** `domain_events`는 추가 전용 기록이고 슬라이스를 넘어 증거로 쓰인다(p0-02 프로브, 자기 참조 결함 7건의 동결 장부). 전역 `system prune`·`volume prune`도, 다른 프로젝트 컨테이너도 건드리지 않는다. 재시작은 `docker compose up -d`.
- **서버를 하드 킬하지 않는다** — stdin에 `shutdown` 한 줄. 하드 킬이 필요했다면 그 사실 자체가 평가 대상이다.
- **golden 재생 파일**(`server/crates/sim/tests/data/replay/`)을 손으로 고치지 않는다. 재생성은 `STARFALL_REPLAY_BLESS=1`일 때만이고, 내용이 달라졌다면 물리를 건드렸다는 신호다(ADR-0010 §3.1).
- **커밋은 사용자가 요청할 때만.** 팀원 에이전트는 커밋하지 않는다(리더가 Phase 6에서 한다).

## 명령과 환경

Bash 앞에 한 번: `export PATH="$HOME/.cargo/bin:$HOME/.dotnet/tools:/c/Users/CHOISOOYEON/AppData/Local/Unity/bin:$PATH"`

| 목적 | 명령 (cargo는 `server/`에서, 나머지는 레포 루트에서) |
|------|------|
| 서버 게이트 | `cargo test --workspace --locked` · `cargo fmt --all --check` · `cargo clippy --workspace --all-targets -- -D warnings` |
| 릴리스 전용 테스트 | `cargo test -p starfall-sim --release` — `#[cfg(not(debug_assertions))]` 테스트는 표준 게이트에서 **컴파일조차 되지 않는다** |
| Unity 테스트 | `unity test client --mode EditMode` |
| 계약 DTO 생성 | `cd tools/codegen && dotnet run ContractsCodegen.cs -- --contracts ../../contracts --out ../../client/Assets/_Project/Scripts/Contracts/Generated` |
| 인프라 | PostgreSQL **15432** / Redis **16379**. SQL은 `docker compose exec -T postgres psql -U starfall -d starfall -At -F' \| ' -c "..."` |

- `.env`에 `STARFALL_DEV_AUTH_SECRET`이 있다.
- **Unity 프로젝트는 단일 인스턴스다** — Editor가 열려 있으면 `unity test`가 돌지 않는다. 에이전트는 Play 버튼을 누를 수 없다(사람이 필요한 검증은 사람에게 넘긴다).
- `_workspace/`는 **추적되는 감사 기록**이다. 지우지 않는다. 화면 녹화(`*.mp4`)만 무시한다.

## 하네스: STARFALL 게임 개발 팀

**목표:** 기획안을 스펙 → 완료 기준 합의 → 병렬 구현 → 실제 실행 검증의 반복으로 플레이 가능한 버티컬 슬라이스로 만든다.

**트리거:** 게임 기능 구현, 스펙·설계, 버티컬 슬라이스, 마일스톤/Phase 진행, 서버·클라이언트·역사 엔진·렌더링 작업, 통합 검증 요청 시 `starfall-dev` 스킬을 사용하라. 단순 질문이나 한두 줄 수정은 직접 응답·수정해도 된다.

**에이전트 팀 전제:** 프로젝트 `.claude/settings.json`에 `CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS=1`이 설정되어 있다. 팀원 도구가 없는 세션이면 `starfall-dev`의 서브 에이전트 대체 모드를 따른다.

**변경 이력:**
| 날짜 | 변경 내용 | 사유 |
|------|----------|------|
| 2026-09-17 | 초기 구성 (에이전트 7, 스킬 8, 프로젝트 플러그인 4) | 기획안 기반 게임 개발 하네스 도입 |
| 2026-09-19 | 구현 역할(server·history·client·techart)은 Sonnet 5 + TDD, 판단 역할(architect·designer·qa)은 Opus 5 | 근거·예외·재검토 조건: `.claude/skills/starfall-dev/references/model-assignment.md` |
| 2026-09-23 | 검증 규율·금지 사항·명령/환경 절 신설 | p1-01에서 에이전트마다 반복 설명해야 했던 것, 그리고 네 번 반복된 "초록불인데 아무것도 재지 않는" 결함 |

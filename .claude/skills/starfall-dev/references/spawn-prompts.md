# 스폰 프롬프트 템플릿과 상태 파일 형식

팀원은 리더의 대화 기록을 받지 못한다. CLAUDE.md, 프로젝트 스킬, MCP 설정만 자동으로 로드된다. 그래서 스폰 프롬프트에 **이번 작업에 필요한 맥락 전부**를 넣는다. 빠뜨리면 팀원이 추측으로 채운다.

## 목차
1. 공통 스폰 프롬프트
2. 팀원별 추가 지시
3. 부분 재실행용 추가 문단
4. 서브 에이전트(독립 최종 게이트) 프롬프트
5. `_workspace/STATUS.md` 형식
6. `00_request.md` 형식

---

## 1. 공통 스폰 프롬프트

```
너는 STARFALL DYNASTY 개발 팀의 `{팀원 이름}` 팀원이다. 역할과 원칙은 에이전트 정의를 따른다.

## 이번 작업
- 슬라이스: {slice-id} — {한 줄 목표}
- 현재 Phase: {2 스펙·계약 | 3 스프린트 계약 | 4 구현 | 5 평가}
- 작업 유형: {BOOT | SLICE | SPEC}

## 먼저 읽을 것
- CLAUDE.md (절대 원칙)
- _workspace/{slice-id}/00_request.md
- {docs/specs/{slice-id}.md, contracts/…, 01_architect_tasks.md, 02_sprint_contract.md 중 현재 Phase에 존재하는 것}

## 사용할 스킬
- Skill 도구로 `{스킬 이름}`을 호출해 규약을 따른다. {Unity 작업이면: Unity 공식 스킬 `/unity:…`도 필요 시 호출}

## 소유권
- 네가 수정할 수 있는 경로: {경로 목록}
- 그 밖의 파일이 바뀌어야 하면 소유자에게 SendMessage로 요청한다.

## 팀
- 현재 팀원: {architect, designer, server, history, client, techart, qa 중 활성 이름}
- 태스크: TaskList에서 담당자가 `{팀원 이름}`인 항목을 claim해서 진행하고, 끝나면 TaskUpdate로 완료 처리한다.

## 완료 보고
- 산출물: {예: _workspace/{slice-id}/03_server_impl.md}
- 모듈이 끝날 때마다 `qa`에게 "모듈 완료: {경로}, 실행 방법: {명령}"을 보낸다.
- 턴을 마칠 때 최종 답에 요약(한 일, 테스트 결과, 미해결 문제)을 쓴다. 리더에게 자동 전달된다.
- 실행하지 못한 검증은 통과로 쓰지 말고 "미검증(환경)"으로 쓴다.
```

## 2. 팀원별 추가 지시

| 팀원 | 추가 문단 |
|------|----------|
| architect | "계약을 바꾸면 영향받는 모든 팀원에게 변경 파일과 요지를 SendMessage로 알린다. 사용자 결정이 필요한 열린 질문은 스펙 '열린 질문' 절에 모으고 리더에게 알린다." |
| designer | "MVP 수량 제한을 지킨다. 수치는 data/ 테이블로 빼고 가정은 `가정:`으로 표시한다. 재미 검증 시나리오를 qa에게 보낸다." |
| server | "상태 변경은 tick 루프에서만. 완료 전 cargo fmt / clippy -D warnings / test 통과. 엔드포인트가 준비되면 client에게 목록과 예시 payload를 보낸다." |
| history | "기록 불변, 멱등, 결정성, rule_version. 속성 테스트(중복 처리·순서 변화·재생)를 qa와 공유한다." |
| client | "클라이언트는 판정하지 않는다. 서버 준비 전에는 contracts/fixtures로 개발한다. Editor가 켜져 있으면 씬·프리팹을 unity CLI/MCP로 조작한다." |
| techart | "초기에는 그레이박스로 가독성 우선. 성능 주장에는 측정값. WebGL 대체 경로를 함께 적는다. 프리팹 소유권은 client와 먼저 합의한다." |
| qa | "PASS에는 증거. 모듈 완료 알림을 받으면 즉시 경계면 검증. 경계면 불일치는 생산자와 소비자 양쪽에 알린다. 평가 루프는 최대 3라운드." |

## 3. 부분 재실행용 추가 문단

```
## 이전 산출물 (부분 재실행)
- 이전 결과: {_workspace/{slice-id}/03_…md, 04_qa_report_r{N}.md}
- 사용자 피드백: "{원문}"
- 이전 결과를 먼저 읽고, 피드백과 FAIL 항목에 해당하는 부분만 수정한다. 무관한 변경을 섞지 않는다.
```

## 4. 서브 에이전트(독립 최종 게이트) 프롬프트

이름 없이 `Agent({ subagent_type: "qa-integration-engineer", model: "opus", ... })`로 호출한다.

```
너는 이 슬라이스의 팀 대화에 참여하지 않은 독립 평가자다.
- 슬라이스: {slice-id}
- 평가 기준: _workspace/{slice-id}/02_sprint_contract.md (이 밖의 기준을 추가하지 않는다)
- 참고만 할 것: 팀 QA의 최신 리포트 {04_qa_report_r{N}.md} — 결론을 그대로 믿지 말고 직접 재실행한다.
- Skill 도구로 `integration-qa`를 호출해 절차를 따른다.
- 출력: _workspace/{slice-id}/04_qa_report_final.md
- 팀 QA 결론과 다른 항목은 "불일치" 절에 양쪽 근거와 함께 적는다.
```

## 5. `_workspace/STATUS.md` 형식

```markdown
# STARFALL 슬라이스 현황

| 슬라이스 | 유형 | 상태 | 마지막 Phase | 최신 QA | 갱신일 |
|---------|------|------|-------------|---------|-------|
| p0-01-bootstrap | BOOT | done | 6 | r2 PASS 12/12 | 2026-09-20 |
| p1-02-mining | SLICE | eval | 5 | r1 FAIL 2/15 | 2026-09-24 |
```

상태 값: `spec` → `contract` → `build` → `eval` → `done` | `needs-env-verification` | `blocked`(사유를 비고에)

## 6. `00_request.md` 형식

```markdown
# {slice-id}

## 요청 원문
{사용자 메시지 그대로}

## 분류
- 유형: {BOOT|SLICE|SPEC|SOLO|VERIFY|BUG}
- 로드맵 Phase: {p0|p1|p2|…}
- 범위 판단: {범위 안 | 축소안 합의 내용}

## 투입
- 팀원: {이름 목록}
- 제외한 레이어와 이유: {예: techart — 새 비주얼 없음}
```

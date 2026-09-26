## 요약

<!-- 무엇을 왜 바꿨는가 -->

## CI 가 보지 않는 게이트 — 해당하면 직접 돌리고 체크

CI(`gates`)는 server·bots 의 cargo 게이트와 Python 판정 게이트만 돈다. 아래는 사람이 돌린다.
**통과한 테스트 수는 경로가 실행됐다는 증거가 아니다** — 결과는 exit 코드가 아니라 산출물로 판정한다.

- [ ] `client/` 를 바꿨다면: `unity test client --mode EditMode` 를 돌렸고, **`test-results.xml` 의 스위트별 결과**로 판정했다(exit 0 과 콘솔 출력은 무엇이 돌았는지 말해 주지 않는다). total · passed · failed · skipped:
- [ ] 실서버·봇 세션이 필요한 변경이면: 세션을 돌렸고 서버는 stdin `shutdown` 으로 정상 종료했다
- [ ] 바이너리로 판정한 증거에 `git rev-parse HEAD` 와 `git status --porcelain` 공백 여부를 남겼다(계약 §7b 규칙 9)
- [ ] 해당 없음

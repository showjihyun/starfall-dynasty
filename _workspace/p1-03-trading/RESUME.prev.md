# RESUME — p1-03-trading (qa-r1)

- 갱신: 2026-10-10 16:05 qa-r1
- 현재 단계: **Phase 5 r1 실행 중.** dry-run 12 DRY 통과(job tmp `dry_r1_qar1`). 본 실행 16:04 시작, freeze PASS, gates 진행 중.
- 기다리는 사람: qa-r1(감시), 리더(db_stop 승인 `go_db_stop`)
- 기준: `02_sprint_contract.md`(SC-01~133). 리포트는 `04_qa_report_r1.md`.
- 실행 전 확인(16:04): FREEZE 있음(16:03, 리더), Unity Editor 없음(Hub 만, Lockfile 없음), 8080 LISTEN 0줄, postgres/redis healthy.

## 결과 위치

- `_workspace/p1-03-trading/evidence/r1_20261010/summary.json` (끝나면 `DONE` 줄), `progress.txt`
- 실행기 stdout: job tmp `r1_run_qar1.out`

## 다음 명령

- 실행이 죽었으면: 같은 명령에 `--out _workspace/p1-03-trading/evidence/r1_20261010b` 를 붙여 다시 돈다.

```
python tests/e2e/run_round.py --slice p1-03 --contract _workspace/p1-03-trading/02_sprint_contract.md --config tests/e2e/round_configs/p1-03-trading.json --round r1 --gate-timeout 7200
```

- 끝났으면: summary 를 보고 `04_qa_report_r1.md` 작성.

## 미검증으로 둘 항목 (이전 인계 그대로)

| 항목 | r1 표기 |
|---|---|
| SC-65·68·71·74·77·80·83·86·89·92·95·98·101 (UI ③ 사람 체크) | 미검증(사람 — 최종 세션) |
| SC-133 | 미검증 — 사용자 결정으로 건너뜀(2026-10-10) |
| SC-39 | db_stop 승인 없으면 미검증(사람 대기) |
| CI 첫 실행 | 리포트 "CI 가 보지 않는 것" 절 |

## 계약 외 발견 (리포트로)

- `HistoryHandles.detector_halted_total` 죽은 카운터
- 호스트 일시정지 뒤 tick 따라잡기 폭주(evidence/presession/server.log: pending_batches 4148, stdin shutdown 시간 초과로 도구가 terminate)
- trading_replay.rs 주석 정정(판정 영향 없음)

## 알려진 비결정성·Phase 4 증거 경로

이전 인계(qa3)와 같다: phase4_NOTES.md, phase4_live1/NOTES.md(bot-116~118), live5 포트 점유, 절전 무효(live6·7). SC-117·119 는 r1 load 그룹이 첫 유효 숫자.

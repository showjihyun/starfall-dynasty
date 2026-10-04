# RESUME — p1-02-mining (qa-r3 → 리더)

- 갱신: 2026-10-04 00:55 qa-r3
- 현재 단계: **Phase 5 r3(마지막) 판정 완료.** 리포트 `04_qa_report_r3.md`, 증거 `evidence/r3_20261004/`
- 결과: **PASS 112 / FAIL 0 / 미검증 2.**
- 동결: `FREEZE` 는 아직 있다. 해제는 리더가 한다. r3 판정에 더 필요한 소스 실행은 없다.

## r3 에서 한 계약 정정 (리더 결정·지시, 문서만, 코드·판정 기준·⊘ 불변 — §12 변경 이력 4행)
- SC-09: 필터 `mining::each_rejection_reason` 로 교체(00:23), 이어서 방법 칸을 ① sim ② gateway 두 줄로 나눴다(00:4x, gateway = `mine_resource_capacity_exceeded_is_counted_in_commands_rejected_total`).
- SC-10: `mining::cooldown_before_range_in_judgement_order` 추가(필수 쌍 3+4).
- SC-28: 유령 이름 `recording_halted_rejects_state_change` 를 실제 이름 셋으로 교체(`recording_lag_boundary_*` · `mine_resource_is_not_rejected_with_recording_backlog_when_lag_is_within_the_limit` · `mine_resource_passes_when_persist_backlog_is_high_but_recording_lag_is_zero`).
- 정정 뒤 고친 스캔으로 전수를 다시 돌렸다. 유령 0, Rust 필터 69개 각 실행 ≥ 1, 실패 0(`filters_final/filters_summary.tsv`, `scan_final.txt`).

## 남은 미검증
- SC-68: 사람 Unity 세션(`sc68_human_session_procedure.md`)
- SC-98 + 태스크 #21: Phase 6 커밋·푸시 뒤 첫 CI 실행. `mine_resource_capacity_exceeded_is_counted_in_commands_rejected_total` 의 시간 여유도 이때 지켜본다(리포트 §7-1).

## qa 도구 — 동결 해제 뒤 할 일 (리포트 §3a)
- `tests/e2e/cargo_sc_map.py`: 맨 백틱 테스트 이름을 줍고, `--test X` 같은 값 받는 플래그 뒤의 필터를 줍게 고친다. selftest 에 두 케이스를 추가한다. 참고 구현은 `evidence/r3_20261004/scripts/contract_scan_fixed.py`.
- `run_filters.sh`: 입력의 CR 제거와 빈 열 자리 채움을 넣는다(참고: `scripts/run_filters_final.sh`). CRLF 결함이 2 라운드 연속으로 났다.
- 스크립트 사본은 `evidence/r3_20261004/scripts/` 에 있다(원본은 job tmp): `r1_gates.sh` · `flaky2.sh` · `run_filters.sh` · `run_filters_final.sh` · `contract_scan_fixed.py` · `mutate.py` · `ghost.py` · `r3_edits.py` · `pairs.py` · `fp3.sh` · `r3_all.sh`.

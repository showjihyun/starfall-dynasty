# p1-02-mining — Phase 5 판정 대기 목록 (qa)

Phase 4 경계면 검증 중 발견했지만 **지금 판정하지 않고** Phase 5 리포트(`04_qa_report_r1.md`)에서 계약 문구대로 판정할 항목. 계약에 새 요구를 더하지 않는다 — 전부 기존 SC 행의 문구와 구현을 대조한 것이다.

| SC | 계약 문구 | 지금 구현 | 출처 |
|----|----------|----------|------|
| SC-20 | 둘째 커밋의 **반환값이 `AlreadyCommitted`** | `batch_recommit_is_noop` 은 `ambiguous_commits_total == 1` 카운터로만 본다 — 반환값 직접 단언 없음 | S6 검증, 리더 2026-09-29 |
| SC-20 | **모든 테이블** 행 변화 0 | `domain_events` 1 · 인벤토리 25 만 단언 — `deposit_states`·`processed_commands` 불변 단언 없음 | 같음 |
| SC-20 ⊘ | 재투입 **전** `last_tick ≥ batch.tick`, 이벤트·상태·장부 행 각 ≥ 1 | 두 배치를 한 번에 `run_batches` 로 넣어 사이 단언이 없다(사후 events=1 로 간접) | 같음 |
| SC-26 · SC-78(restart) · AC-11(g) · SC-108 재기동 변형 | 재기동 뒤 DB 값 적재·`DUPLICATE_COMMAND_ID`·발견 계속 | 기동 시 경제 상태 적재 미구현 → S8(#27). S8 뒤 실서버 봇으로 판정 | qa 2026-09-29 |
| (계약 외) | — | 워크스페이스 테스트 빌드가 `target/debug/starfall-game-server.exe` 에 test-hooks 를 넣는다(feature 통합). `server_boot.py` 가 기동 전 거부 | qa 2026-09-29 |
| (계약 외) | — | `persisted_total` 은 모호한 커밋 재시도 때 DB 행 수보다 커진다(ADR-0013 §4 의도). `persisted_total == COUNT` 를 쓰는 도구는 `persist_ambiguous_commits_total > 0` 이면 그 항등식을 적용하지 않는다 | 같음 |

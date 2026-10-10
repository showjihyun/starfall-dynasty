# Phase 4 봇 첫 실서버 실행 — 단일 봇 판매 케이스 9 (qa3, 2026-10-07)

판정이 아니다(r1 은 새 qa 가 run_round.py 로 한다). Q3 봇이 실서버에서 도는지 보는 점검 실행이다.

- 월드: `world_id` 파일(새 월드, new_world.py). 서버: server_boot.py serve → stop 파일 → stdin shutdown, `SHUTDOWN exit=0`.
- HEAD b3bc8f2 + 작업 트리(커밋 전). 서버·봇은 같은 시각 dev 빌드.

| 케이스 | 1차 | 재실행 | 비고 |
|---|---|---|---|
| sell-dup | PASS | — | 영수증 1, 지갑 2,000 = 대금 합 |
| sell-dup-reconnect | rc 0 | — | |
| sell-dup-cross-kind | rc 0 | — | |
| cheat-sell-inject | rc 0 | — | |
| cheat-sell-range | rc 0 | — | |
| cheat-sell-fast | PASS | — | 보낸 순간 거리 218 m · 속력 31.5 m/s → SHIP_TOO_FAST |
| cheat-sell-overqty | **미검증(조건 미발생)** | PASS | 1차: bot-116 세션이 457 tick 만에 **SLOW_CONSUMER** 로 닫힘(DB SESSION_CLOSED) — 판매 0 |
| cheat-sell-cooldown | **미검증** | PASS | 1차: bot-117 **세션 열림 없음**(SESSION_OPENED 0) — 접속 실패 |
| cheat-sell-unknown | **미검증** | PASS | 1차: bot-118 같은 증상, 117 과 같은 초에 종료 |

## 비결정성 이슈(재시도로 덮지 않는다)

1차 실행 끝의 세 케이스가 연달아 실패했다: 116 은 서버가 SLOW_CONSUMER 로 닫았고, 117·118 은 세션을 열지 못했다(봇 JSON 에 actor_id 없음). 같은 서버·같은 빌드로 곧바로 다시 돌린 세 케이스는 전부 PASS.
원인은 아직 모른다. 봇이 읽기를 멈춘 구간이 있었거나(호스트 정지?) 접속 단계 실패다. 다음 실행부터 원인을 같은 실행에서 볼 수 있게 봇 JSON 에 `connect_error`·`close`·`bot_errors`·`wire_errors` 를 더했다(trade.rs run_single).
r1 에서 같은 증상이 나오면 FAIL(비결정성)로 기록하고, 이 메모를 근거로 원인을 좁힌다.

## 같은 월드 SQL(참고)

- `trade_ledger.py causation`: PASS — 채굴 18 · 판매 11, 양방향 누락 0, 종류 불일치 0.
- `ledger` 의 문제 0 이지만 실행 모양 미달(이 월드는 trade-cycle 이 아니다 — 판매 광물 1, 구간 걸침 0). 예상대로.
- `zero-rows`: 이 월드엔 전량 판매가 없다 → round config 의 SC-37 대상을 cycle 월드로 고쳤다.

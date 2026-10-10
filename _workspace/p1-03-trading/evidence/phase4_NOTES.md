# Phase 4 봇 실서버 점검 실행 요약 (qa3, 2026-10-07) — 판정 아님

r1 은 새 qa 가 `run_round.py` 로 한다. 이 실행들은 Q3 봇·Q2 도구가 실서버에서 실제로 도는지와, r1 전에 잡을 결함을 찾으려는 것이다. 모든 실행은 새 월드, 서버 종료는 stop 파일 → stdin shutdown.

| 디렉터리 | 무엇 | 결과 |
|---|---|---|
| phase4_s2 | `server_boot.py p1-03-reject` 실바이너리 4 경우 | 4/4 + 음성 대조 PASS |
| phase4_s3s4 | 계약 필터 전수(cargo_sc_map run) · SC-04 기동 · SC-126 census | S4 `[DB]` 11 RAN · SC-04 PASS · census 122/4 PASS |
| phase4_s5 | 계약 필터 전수 재실행 | 실행 ≥ 1 = 64/65(남은 1 = SC-40 이름 — 계약 정정) · 실패 0 |
| phase4_live1 | 단일 판매 케이스 9 | 1차 6 PASS·3 미검증(비결정성 — NOTES.md), 재실행 3 PASS |
| phase4_live2 | trade-cycle + 원장 도구 전부 + history-replay | 전부 PASS, 실행 모양 충족(판매 actor 3·판매 12·광물 2·걸침 6·회복 쌍 7), 음성 대조 FAIL(정상) |
| phase4_live3 | leak-scan-market · trace-sell-a/b · race-sell-same-tick | 흔적·경쟁 PASS. leak 1차 FAIL = 봇 결함(SESSION_READY 를 첫 타입 목록에 안 넣음) → 고친 뒤 rerun PASS |
| phase4_live4 | restart_cases sell-dup · sell-cas | **서버 결함 발견**: 기동 때 지갑·시장 상태를 sim 에 주입하지 않는다(main.rs:249-263, seed_wallet/seed_market_state 루프 없음). sell-dup FAIL(재기동 뒤 WALLET_STATE 0), sell-cas reload 단계에서 재기동 서버가 첫 판매 커밋 실패로 정지. sell-cas(정지 경로) 자체는 PASS. sell-cas-market 은 도구 결함(월드 태그) → 고침, 서버 수정 뒤 재실행 |
| phase4_live5 | sell-load 31 연결 20 분 | 1차 = 포트 점유로 기동 실패(attempt1). 2차: 측정 중 tick_overrun 0·ws 31·recording_lag 0 관측, 그러나 봇이 창 뒤에도 사이클을 계속 돌아 바깥 timeout(rc 124)으로 JSON 미작성 → 사이클 수·유예 고침. 같은 월드 SQL: 판매 257·actor 31·광물 3, 원장·연쇄·보존·포화·인과 PASS, 바닥 도달 광물 3·회복 관찰 3 |

| phase4_live6 | restart_cases 넷 — **server3 seed 수정 뒤** | sell-dup PASS(재기동 뒤 지갑 2,000) · sell-cas PASS + reload PASS(새 판매 1 행, wallet_before 2,777, 서버 정상 종료) · sell-recover PASS(13,000 보존 → 첫 배수 12,000 = 식) · sell-cas-market 1차 무효(대조 실행 중 **호스트 절전** 추정 — `pool timed out`, IdleTimeout elapsed 2,221 만 ms, 봇 GoTo 실패) → 재실행 PASS(시장 행 비교 실패로 정지, 대조 생존) |
| phase4_live7 | sell-load 재실행(사이클 수 고친 봇) | **무효(호스트 절전)** — 서버 기동 2026-10-07T20:49Z, 종료 10-08T11:09Z 인데 마지막 tick 14,758(≈ 12 분 분량): 실행 중 약 14 시간 프로세스가 멈췄다. 깨어나는 순간 `pool timed out` 2 건·SLOW_CONSUMER 2 건·RECORDING_BACKLOG 38 건·recording_lag 51, stats 전후 스냅샷 실패(ticks None). 봇 쪽 고침(사이클·유예)은 동작했다 — 31 연결 모두 끝나 JSON 작성, 응답 항등식 77/77. **이 실행의 SC-117·119 숫자는 판정에 쓰지 않는다** |

## 환경 주의 (r1 담당에게)
- **호스트 절전**이 긴 실행을 무효로 만든다(phase4_live6 sell-cas-market 1차). r1 동안 절전을 끈다 — screen-elements S1·unity_session 의 절전 주의와 같은 원인.
- 직전 서버가 내려가는 중에 다음 서버를 띄우면 포트 점유로 기동이 실패한다(phase4_live5 attempt1). 그룹 사이에 포트 비어 있음을 확인한다.

## 도구 쪽 고친 것 (모두 qa 소유)
- `server_boot.py judge_refusal`: 스키마층 거부 `error=<필드>: … rule=schema` 형식도 주입 필드로 읽음(+ 필드 없는 스키마 거부는 FAIL 음성 대조).
- 봇 `conn.rs`: SESSION_READY 를 첫 타입 목록에 기록(+ 테스트 `session_start_order_requires_session_ready_first`).
- 봇 `trade.rs`: 단일 케이스 JSON 에 접속 오류·닫힘·봇 오류. sell-load 사이클 수 창에 맞춤, 창 + 5 분 뒤 남은 연결 중단·`connections_unfinished`.
- `restart_cases.py`: cas-reload 판정이 "수락" 이 아니라 **새 MINERAL_SOLD 1 행 커밋 + 재기동 서버 정상 종료** 를 요구(수락 뒤 커밋 실패 정지 사례). 시장 변조 월드 태그 수정.
- round config: SC-37 을 cycle 월드로, history-replay `--read-only`, SC-55 판매 행 수 SQL, load 시간 한도.

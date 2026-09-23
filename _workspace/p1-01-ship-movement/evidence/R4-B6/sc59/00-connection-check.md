# 블록 6 사전 — Unity 실서버 연결 확인 (2026-09-23)

**수행**: 리더(에이전트 주간 한도로 qa2·client 불가). **판정하지 않는다 — 관찰 기록이다.** 판정은 qa2 몫.

## 환경
- 서버: `tests/e2e/server_boot.py serve`, `start_tick = 443801`, healthz 200
- 인프라: 재부팅으로 내려가 있던 컨테이너를 `docker compose up -d`로 복구(**`down` 계열 미사용 — G-a 준수**)
- 기록 보존 확인: `domain_events` 4,527행, `QA_APPEND_ONLY_PROBE` 1건, **자기 참조 인과 7건(동결 장부) 그대로**
- Unity: `unity open client -e "…/6000.6.1f1/Editor/Unity.exe"`, `STARFALL_NET_AUTOCONNECT=1` `STARFALL_GREYBOX_AUTOBUILD=1`, 사람이 Play 누름

## 두 출처 대조 (같은 시점 ±수 초)

| 관찰 | 화면(HUD) | 서버 `/debug/stats` · DB |
|---|---|---|
| 연결 | 접속됨 | `ws_connections = 1` |
| 함선 | `visible_ships = 1` | `ships_active = 1`, DB `SHIP_SPAWNED` 1 / `SESSION_OPENED` 1 (tick ≥ 443801) |
| 명령 왕복 | `ack_input_seq = 265` | `commands_received_total = 226`, `RATE_LIMITED = 19` |
| 스냅샷 | tick 449076이 서버와 함께 진행 | `snapshots_sent_total = 417`, tick 449643 |

**RATE_LIMITED 19가 실제로 계수된다** — R3 §4.5에서 "tick 층 거부가 영원히 0"이던 결함이 실서버에서 해소됨을 보인다(SC-33 보강 증거).

## HUD 원문 (정지 상태, 조작 전)
```
tick=449076
ack_input_seq=265
speed_mps=0.0
origin_distance_m=2512.5
predict_error_m=0.0000
predict_error_deg=0.0001
reconcile_hard_snap_total=1
reconcile_error_m   p50=0.0000 p99=0.0000 max=0.0000 n=77
reconcile_error_deg p50=0.0001 p99=34.1309 max=34.1309 n=77
cl2_reconcile_has_error_total=77
visible_ships=1
nearest_ship_m=-
thrust=(0,0,0) roll=0 brake=False assist=True
aim_target=(561886,-77687,53114,821844)
```

## qa2가 판정할 것 (리더는 판정하지 않는다)
1. **SC-56 관찰 ① — `cl2_reconcile_has_error_total = 77`(비-0).** 계약은 "0이면 FAIL"이었다. `Reconcile()`이 **실데이터로 실제 77회 탔다** — 자명한 통과가 아님이 실서버에서 확인됐다.
2. **`reconcile_hard_snap_total = 1`** — M-9의 목표값은 0이다. 세션 시작 시 1회인지(예측 이력이 없는 첫 스냅샷) 다른 원인인지 판정 필요.
3. **`reconcile_error_deg p99 = max = 34.1309` (n=77), p50 = 0.0001** — 단일 이상치로 보이며 위 1건의 hard snap과 같은 사건일 가능성. 위치 오차는 p50/p99/max 전부 0.0000. **SC-56(b)의 판정 대상이고, 임계를 넘으면 임계값을 늘리지 말 것(계약 §7).**
4. `aim_target` 줄이 HUD에 실제로 찍힌다(client가 R1에서 추가한 항목 — 이것이 없으면 SC-59 녹화가 부호를 증명하지 못한다).

**미수행**: SC-59 클립 a~d(조작·녹화). 이 기록은 연결 확인까지다.

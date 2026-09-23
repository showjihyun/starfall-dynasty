# SC-59 클립 a 연장 — 최고속 도달 + 재조정 지표 (2026-09-23)

**기록자**: 리더. **관찰자**: 사용자. **판정하지 않는다 — 관찰 기록이다.**

## 절차
정지에서 W를 약 8초 연속으로 누른 뒤 HUD 캡처.

## HUD 원문 (W 8초 직후)
```
tick=455892
ack_input_seq=7081
speed_mps=135.8
origin_distance_m=1599.4
predict_error_m=0.0005
predict_error_deg=0.0000
reconcile_hard_snap_total=5
reconcile_error_m   p50=0.0004 p99=0.0012 max=56.3096 n=1013
reconcile_error_deg p50=0.0001 p99=1.0999 max=80.5663 n=1013
cl2_reconcile_has_error_total=1013
visible_ships=1
nearest_ship_m=-
thrust=(0,0,0) roll=0 brake=False assist=True
aim_target=(-90080,-548419,-59577,829200)
```
화면: 자기 함선(캡슐) 외에 **타원체 오브젝트 1개가 시야에 보인다**(기준 마커로 추정 — 하이어라키의
`ReferenceMarkers (client-only, never sent, never collided)`).

## 1. 속도 — 설계대로 (designer 신호 해소)
- `speed_mps = 135.8` ≈ `max_speed_mps = 140`. **가속·최고속은 설계대로 동작한다.**
- 따라서 사용자의 "조금 천천히 움직이는 느낌"(클립 a)은 **가속 문제가 아니라 공간 스케일**이다.
  디자이너 결정 2("보이는 목표까지 10~30초", 기준물 4.2 km 내 배치)의 체감 결과다.
- **designer 재조정 입력**: 체감이 느리다면 고칠 값은 `main_thrust_mps2`가 아니라 마커 배치 또는 `max_speed_mps`다.
  M-7·M-8과 같은 계열의 신호이며 데이터 파일만 고쳐 조정 가능(스펙 §10).

## 2. ⚠ 재조정 지표 — qa2·client 확인 필요 (리더는 판정하지 않는다)

| 지표 | 정지 시(직전 기록) | W 8초 뒤 | 목표(M-9) |
|---|---|---|---|
| `reconcile_hard_snap_total` | 1 | **5** | **0** |
| `reconcile_error_m` p99 / max | 0.0000 / 0.0000 | 0.0012 / **56.3096** | p99 < 0.25 m |
| `reconcile_error_deg` p99 / max | 0.0001 / 34.1309 | 1.0999 / **80.5663** | (임계 없음 — 출력만) |
| `cl2_reconcile_has_error_total` | 77 | **1013** | 0이면 FAIL |
| `predict_error_m` / `_deg`(순간) | 0.0000 / 0.0001 | 0.0005 / 0.0000 | — |

**관찰된 형태**: p50·p99는 사실상 0인데 **최댓값만 크게 튄다**. 드문 대형 이탈이 있고 그때 hard snap이
걸리는 것으로 보인다(8초 비행 중 hard snap 1 → 5).

**왜 중요한가**: 00_request가 이 슬라이스로 증명하겠다고 한 네 가지 중 **3번("예측·보정으로 조작이 즉각
반응하면서도 서버 값과 어긋나지 않는다")**에 직접 걸린다. 또한 M-9는 `reconcile_hard_snap_total`의
목표를 **0**으로 적었다.

**확인할 것 (제안이지 판정이 아니다)**:
- hard snap 5건이 **언제** 발생했는가 — 세션 시작 1건 외 4건의 tick과 그때의 입력·네트워크 상태
- 56 m / 80.6°의 단일 이탈이 hard snap과 **같은 사건인지**
- `RATE_LIMITED`(서버 tick 상한 초과로 버려진 명령)와 상관이 있는지 — 연결 확인 시점에 19건 계수됨
- 측정 방식 문제인지(예: 세션 시작·재조정 직후 표본이 섞였는지) 실제 이탈인지
- **계약 §7: SC-51/52 계열에서 임계를 넘으면 임계값을 늘리지 말고 architect에게 알린다.**

## 3. 클립 b (기준 마커 대비 이동) — 부수 관찰
- `origin_distance_m`이 2512.5 → **1599.4**로 이동과 함께 변했다.
- 시야에 정적 오브젝트 1개가 보인다(마커 추정).
- 계약이 요구하는 "마커 4개 중 최소 1개가 항상 보인다"의 **1개 관찰은 충족**. 마커 정체 확인은 qa2 몫.

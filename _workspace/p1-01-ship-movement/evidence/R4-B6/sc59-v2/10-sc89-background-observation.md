# SC-89 실서버 관측 — 수정 후 백그라운드 세션 (2026-09-23)

**수행**: 리더(환경) + 사용자(Play 시작·정지). **판정하지 않는다 — 관측 기록이다.** 판정은 qa3 몫.

## 어떻게 생겼나 (의도한 실험이 아니다)

SC-59 재촬영을 준비하던 중 **다른 게임(LoL)이 전체화면 전경을 차지**해 Unity가 백그라운드로 밀렸다.
촬영은 불가능해졌지만, **그 상태가 정확히 SC-89의 재현 조건**(Editor 백그라운드)이어서 그대로 두고 관측했다.
즉 **재현 벡터를 사람이 만든 것이 아니라 상황이 만들었다** — 어제 결함이 터진 조건과 같은 종류다.

## 환경
- 서버 인스턴스 `start_tick = 475001`, 바이너리 `e84cd098c06fce26`
- 클라이언트: client R11 트리(H-1~H-7, H-10′·H-14·H-16·H-17 적용, `while (_tickAccumulator` 제거됨), EditMode 212/210/실패 0, **동결 상태**
- **미적용**: H-9(수신 쪽 스냅샷 중복), H-13(주기적 로그), H-18

## 세션 (DB, `tick >= 475001`)

| tick | 사건 |
|---|---|
| 478683 | `SESSION_OPENED` + `SHIP_SPAWNED` (1차, 짧음) |
| 478801 | `SESSION_CLOSED{CLIENT_CLOSED}` |
| 479401 | `SHIP_DESPAWNED` (종료 후 **정확히 600 tick**) |
| **479634** | `SESSION_OPENED` + `SHIP_SPAWNED` (2차 — 관측 대상) |
| **493040** | `SESSION_CLOSED{CLIENT_CLOSED}` — 사용자가 Play 정지 |

**2차 세션 수명 = 13,406 tick = 670초 ≈ 11.2분**, **정상 종료**. 대부분 구간에서 Unity는 백그라운드였다.

## 클라이언트 세션 종료 로그 (2차)
```
SC-89 session-end send burst stats -
  max_ticks_drained_per_update=20
  max_sends_per_frame=1
  max_sends_per_trailing_1s=21
  catchup_carry_forward_ticks_total=51
  catchup_dormant_ticks_total=45
  catchup_truncated_total=5
  reconcile_forced_after_hitch_total=0
  prediction_history_overflow_total=0
  outbound_queue_full_total=0
```
(1차 세션: drain 20 / sends_per_frame 1 / carry 11 / dormant 12 / truncated 1 / 나머지 0)

## 서버 (`/debug/stats`, 인스턴스 전체)
`commands_received_total = 367` · **`protocol_violations_total = 0`** · **`commands_dropped_over_tick_cap_total = 0`** · 거부 8라벨 **전부 0**

## 수정 전과의 대조 (같은 조건)

| | 어제(수정 전) | 오늘(수정 후) |
|---|---|---|
| 조건 | Editor 백그라운드 | Editor 백그라운드 |
| 세션 수명 | **459초(7.6분)에 강제 종료** | **670초(11.2분), 정상 종료** |
| 종료 사유 | **`PROTOCOL_VIOLATION`** | `CLIENT_CLOSED` |
| 위반 | 예산 8 초과 | **0** |
| tick 상한 드롭 | 9 | **0** |
| `RATE_LIMITED` | 19 | **0** |

## qa3가 볼 것 — 계약 SC-89의 짝 단언이 그대로 들어 있다

- **(b) 짝 단언**: `max_ticks_drained_per_update = 20` (**히치가 실제로 있었다** — 조건 발생) **그리고**
  `max_sends_per_frame = 1` (**그런데도 프레임당 송신은 1건** — 성질 성립). K-7이 요구한 형태 그대로다.
- **(a)**: 실서버에서 `PROTOCOL_VIOLATION` 0건 · 드롭 0 · `RATE_LIMITED` 0 · `outbound queue full` 0.
- **throttle/pause 판별**: `catchup_truncated_total = 5`는 **M(20 tick = 1초)을 넘는 히치가 5회** 있었다는 뜻이다.
  `max_ticks_drained_per_update`가 M 상한 20에 붙어 있으므로 **실제 히치는 1초보다 컸다**(상한에서 잘렸다).
  체크리스트 §8.2의 판별 기준으로는 **pause 쪽에 가깝다** — 다만 이건 리더의 읽기이고 판정이 아니다.
- **⚠ `catchup_truncated_total`이 0이 아니다.** architect는 "정상 플레이에서 발동하면 그 자체가 발견"이라고 적었다.
  이번 구간은 "다른 게임이 전경인 백그라운드"라 정상 플레이로 볼지 판단이 필요하다.
- **미적용 항목 주의**: H-9(수신 쪽)이 안 들어갔다. `reconcile_forced_after_hitch_total = 0`이지만
  **재조정 지표(SC-56)는 H-10′ 뒤 세션 범위로 따로 재야 한다**(architect 정밀화).

## 한계 (정직하게)
- **의도한 실험이 아니다.** 히치의 크기·빈도를 통제하지 않았고, 전경/백그라운드 전환 시각도 기록하지 않았다.
- **어제 세션과 조건이 완전히 같지는 않다**(어제는 터미널·브라우저, 오늘은 전체화면 게임).
- 11.2분이 7.6분보다 길다는 것은 **"같은 조건에서 더 오래 버텼다"**를 보이지만, 위반이 0인 이유가
  수정 때문인지 히치 양상이 달라서인지는 **이 관측만으로 단정할 수 없다.** (b)의 짝 단언이 그 간극을 메운다.

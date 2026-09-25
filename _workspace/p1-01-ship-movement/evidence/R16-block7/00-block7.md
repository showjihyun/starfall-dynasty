# 블록 7 — SC-61 · SC-62 · SC-64 · SC-65 (2026-09-25)

- 서버: tick 1545881 기동 → 1568108 정상 종료(stdin `shutdown`)
- 하네스: `TwoSessionHarness`(A 조종 / B 비대화형), `STARFALL_TWO_SESSION_AUTOBUILD=1`
- 조종: 사용자. 정지 → `W` 30초 → `X` → 정지. 마우스 미사용
- 원본: `observer-a.csv`, `observer-b.csv`, `Editor-session.log`, `compare.json`, `s3.json`

## 1. 비행 전 확인 여섯 줄 — 전부 통과

Play 직후 확인했다(절차서 §2). subject 2종 상이 · 두 세션 `session ready` · actor_id 상이 ·
`SUPERSEDED`/4001 없음 · 두 CSV **12열** · **`observer-b.csv`의 `ship_id` 2종**(B가 A를 본다).
서버 `ws_connections=2` · `ships_active=2`로 교차 확인.

## 2. 판정

| SC | 판정 | 핵심 수치 |
|----|------|----------|
| **SC-61** | **PASS** | 순변위 **5,125.307 m**, 경로 5,270.984 m, 역주행(>1 m) **0회**, 표본 1,547, span 3,092 tick |
| **SC-62** | **PASS** | B 최대 이탈 **0.0 mm**(허용 50 mm), `exceeds_one_lsb_1mm` **false**, 표본 1,486 |
| **SC-64** | **PASS** | 정지 시 두 화면 차이 최대 **0.175 m**(허용 2.0 m), 표본 991 |
| **SC-65** | **PASS** | 뒤쪽 **27.156 m**(기대 28.0 ± 12.0), 부호 **뒤쪽**, 표본 325 |

판정 명령은 계약이 요구한 형태로 돌렸다 — `--b-own-tolerance-m 0.05` 명시(생략 시 exit 4).
`judgment_inputs` 7개가 출력에 남는다: `0.05`·`400`·`0.9`·`100.0`·`140.0`·`1.05`·`1000`.

### 겨냥한 조건이 실제로 발생했다 (§7a)

- **SC-65**: `speed ≥ 100 m/s` 표본 **325행**, 최고속 140.0 m/s. 0이면 자명 통과였다.
- **SC-62**: A 순변위 **5,125 m** ≥ 대조 요건 100 m. A가 안 날았으면 `미검증(대조 없음)`이다.
- **SC-64**: `speed < 1 m/s` 표본 676행(A) / 1,171행(B).
- **SC-65의 27.156 m는 설계값의 재현이다** — `remote_interp_delay_ms = 200` × 140 m/s = 28 m.

## 3. ⚠ 적어야 하는 관측 — B 기록에 2초 구멍이 있다

```
max_gap_between_a_ship_rows : 2  tick   (정상 = 스냅샷 간격)
max_gap_between_b_own_rows  : 40 tick   ← 정상의 20배
expected_gap_ticks          : 2
```

qa r12가 계약에 없는 판정을 새로 걸지 않고 **관측만** 내게 한 값이고, 리포트 게재를 의무로
걸어 둔 값이다. 사유는 이것이다:

> 계약 SC-62 (i)은 **스팬과 커버리지로 양 끝만 본다.** 창 한가운데가 통째로 비어도 통과한다
> (qa selftest에서 242 tick 구멍이 PASS를 내는 것을 실행으로 확인해 뒀다).

**즉 네 항목의 PASS는 유효하되, B의 기록은 중간에 2초가 비어 있다.**
원인은 확인하지 않았다 — 추측하지 않고 미확인으로 남긴다.
`runInBackground: 1`이므로 포커스 상실은 원인이 아니다.

## 4. SC-56 (e) 세션 종료 덤프 — 이번 세션은 전부 0이다

```
observer.A: render_smooth_band_total=4, render_offset_nonzero_frames_total=0,
            render_offset_max_m=0.0000 (n=109960), render_offset_decay_frames_total=0
observer.B: render_smooth_band_total=0, ... 전부 0
```

**밴드 분류는 4회 일어났는데 오프셋이 한 번도 0이 아닌 적이 없고 감쇠도 0회다.**
architect가 요구한 네 갈래 판정표의 입력이며, 값만으로는 갈리지 않는다.

참고 — **같은 로그의 앞선 세션에는 정상값이 있다**:
```
render_smooth_band_total=43, nonzero_frames_total=1219,
render_offset_max_m=0.6330 (n=26683), decay_frames_total=1203
```
즉 **미구현은 아니다.** 이번 세션에서 왜 0인지는 qa 판정 사항이다.
**리더는 원인을 단정하지 않는다.**

## 5. 판정하지 않은 것

- **SC-63은 이 세션의 항목이 아니다**(계약 13차 → 블록 10, 봇 2대 원시 CSV).
  블록 7 리포트에 SC-63 verdict를 적지 않는다.
- §3의 2초 구멍과 §4의 0 세 개는 **관측이며 판정이 아니다.** qa가 판정한다.

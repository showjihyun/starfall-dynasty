"""SC-61·62 — **A가 움직이면 B가 본다** (AC-16 + designer S-3) · SC-63 — 봇 2대 원시 대조.

CSV 형식은 client·봇이 같다(계약 §3.1, 12열):
`tick,observer_actor_id,ship_id,presence,px_mm,py_mm,pz_mm,vx_mm_s,vy_mm_s,vz_mm_s,render_offset_mm,render_offset_deg`

| 하위 명령 | 블록 | 항목 | 판정 입력 |
|----------|------|------|----------|
| `compare` | 7 | SC-61(단조·이동 거리) · SC-62(B 정지) | Unity 관측자 A·B 의 Observer CSV |
| `s3`      | 7 | SC-64(정지 ≤ 2 m) · SC-65(**뒤쪽 28 ± 12 m, 부호가 핵심**) | 같은 두 CSV |
| `raw2`    | **10** | **SC-63**(같은 tick 원시 정수 일치) | **봇 실행 디렉터리**(`snapshots.csv` + `summary.json`) |
| `selftest`| — | 도구 검증 — 검출기·헤더 가드·**verdict 분리**·**블록 10 방어 6절** |

**SC-63 은 13차 개정으로 블록 7 에서 떨어져 나갔다.** 판정 입력이 Unity 관측자 CSV 가 아니라
봇 2대의 원시 스냅샷 CSV 다 — 그 파일에는 원시 층이 한 열도 없고(자기 함선 행 = 예측 + 렌더
평활화 오프셋, 타 함선 행 = 보간), 두 층의 차이가 곧 SC-65 가 28 ± 12 m 로 요구하는 양이라
같은 파일 위에서 SC-63 과 SC-65 는 **논리적으로 양립 불가**다(qa r11 §1, 20/20 실측).

**그러므로 `compare` 는 SC-63 을 계산하지 않는다.** 계산해 두고 "참고"로 싣는 형태는 리포트를
읽는 사람이 verdict 로 읽으므로, 코드에서 없앴다 — 재결합이 **구조적으로 불가능**해야 한다
(계약 13차 개정 SC-63 (f), architect `## R11 후속 판정 (R13)` §1.2).

    python tests/e2e/two_client_view.py compare --a <A.csv> --b <B.csv> --ship <A의 ship_id>
    python tests/e2e/two_client_view.py s3 --a <A.csv> --b <B.csv> --ship <uuid> [--speed-mps 140]
    python tests/e2e/two_client_view.py raw2 --bot-out-dir <봇 --out 디렉터리>
    python tests/e2e/two_client_view.py selftest

종료 코드: 0 통과 / 1 FAIL / 2 미검증(환경) / 3 FAIL(구현 없음) / **4 미검증(증거 요건·표본)**
"""

from __future__ import annotations

import argparse
import csv
import json
import math
import sys
from pathlib import Path

import db

# 계약 §0.3 의 `미검증(증거 요건)` 계열. db 의 0~3 과 섞이지 않게 여기서만 쓴다 —
# **FAIL 이 아니다**(잰 것이 없다). 그러나 §0.3 에 따라 비-통과이므로 0 도 아니다.
EXIT_EVIDENCE = 4

# 계약 SC-62 의 자명 통과 방어 (i)(ii) 가 고정한 표본 조건. **이것들은 임계가 아니라 표본
# 요건이고 계약 행이 수를 적고 있다** — 임계(`0.05 m`)와 달리 판정 기준이 아니라 "이 관측이
# 성립했는가"를 가른다. 계약 행이 바뀌면 여기도 바뀐다.
B_OWN_MIN_SPAN_TICKS = 400          # 20초 @ 20 Hz
B_OWN_MIN_SPAN_RATIO = 0.90         # A 행 tick 스팬의 90 % 이상을 덮는다
A_MIN_NET_DISPLACEMENT_M = 100.0    # (ii) 대조 — `path_mm > 0` 은 이 자리를 못 막는다

COLUMNS = [
    "tick", "observer_actor_id", "ship_id", "presence",
    "px_mm", "py_mm", "pz_mm", "vx_mm_s", "vy_mm_s", "vz_mm_s",
    # R23: 이 두 열은 ObserverCsv.cs 의 Header 와 **이름·순서·개수가 일치해야 한다**
    # (ObserverCsv.cs:49-51 이 그 계약이고 불일치를 NotImplementedYet 으로 거부한다).
    # 같은 헤더의 생산자가 넷이다 — 이 파일, ObserverCsv.cs(client),
    # tools/bots/src/snapshot.rs(봇 CSV), 그리고 계약 문서 §3.1.
    # 봇은 평활화가 없으므로 두 열이 상수 0 이다.
    "render_offset_mm", "render_offset_deg",
]


def read_rows(path: str | Path) -> list[dict]:
    """헤더를 검증하고 **행 순서 그대로** 돌려준다. 관측자가 섞인 파일도 읽는다."""
    p = Path(path)
    if not p.is_file():
        raise db.EnvironmentProblem(f"CSV 가 없다: {p}")
    out: list[dict] = []
    with p.open(encoding="utf-8", newline="") as f:
        reader = csv.DictReader(f)
        if reader.fieldnames != COLUMNS:
            raise db.NotImplementedYet(
                f"{p}: 컬럼이 계약 §3.1 과 다르다.\n  기대: {COLUMNS}\n  실제: {reader.fieldnames}"
            )
        for row in reader:
            out.append({
                "tick": int(row["tick"]),
                "observer": row["observer_actor_id"],
                "ship_id": row["ship_id"],
                "presence": row["presence"],
                "p": (int(row["px_mm"]), int(row["py_mm"]), int(row["pz_mm"])),
                "v": (int(row["vx_mm_s"]), int(row["vy_mm_s"]), int(row["vz_mm_s"])),
                # R23 (architect R10 후속 §2): SC-64 예산 2 m 중 평활화 오프셋이 최대 16 %
                # 를 먹는다. 2 m 를 넘었을 때 **평활화 탓인지 두 화면이 실제로 다른 탓인지**
                # 이 값 없이는 답할 수 없다. 실어만 두고 읽지 않으면 열이 아니라 주석이다.
                "render_offset_mm": int(row["render_offset_mm"]),
                # **정수가 아니다.** client 는 `F4` 로 쓴다(`ObserverCsv.cs:89`, 예: `0.0000`),
                # 봇은 정수 리터럴 `0` 을 쓴다(`snapshot.rs:209`). int() 로 읽으면 Unity CSV 에서
                # 즉시 ValueError 다 — 이 열이 판정에 쓰인 적이 없어 드러나지 않았다.
                "render_offset_deg": float(row["render_offset_deg"]),
                # 그 표기 차이 자체가 생산자의 지문이다(블록 10 (c) 보조 게이트).
                "render_offset_deg_literal": row["render_offset_deg"],
            })
    if not out:
        raise db.EnvironmentProblem(f"{p}: 행이 없다 (0건 대조는 검증이 아니다)")
    return out


def load(path: str | Path) -> dict[tuple[int, str], dict]:
    """블록 7 용 — **관측자 1명**의 CSV 를 `(tick, ship_id)` 로 싣는다.

    관측자가 둘 이상 섞여 있으면 거부한다. 봇 하네스의 `snapshots.csv` 는 한 실행의 모든
    세션을 **한 파일에 합쳐** 쓰므로(`report.rs:247-253`), 그 파일을 블록 7 에 먹이면 키가
    충돌해 조용히 절반이 덮인다. 13차 개정 이후 두 CSV 가 **같은 헤더**라 형식으로는
    구분되지 않으므로, 이 가드가 방향을 지킨다(블록 10 → 블록 7 오투입).
    """
    rows = read_rows(path)
    observers = sorted({r["observer"] for r in rows})
    if len(observers) > 1:
        raise db.NotImplementedYet(
            f"{path}: 한 파일에 관측자가 {len(observers)} 명이다 {observers}. "
            "블록 7 은 관측자 1명당 CSV 1개를 받는다 — 봇 하네스의 합본 snapshots.csv 라면 "
            "`raw2 --bot-out-dir` (블록 10)로 보내라."
        )
    return {(r["tick"], r["ship_id"]): r for r in rows}


def dist_mm(a: tuple[int, int, int], b: tuple[int, int, int]) -> float:
    return math.dist(a, b)


def speed_mps(v: tuple[int, int, int]) -> float:
    return math.dist((0, 0, 0), v) / 1000.0


# ─────────────────────────────────────────────────────────────────────────────
# 블록 7 — SC-61 · SC-62 (SC-63 은 여기 없다)
# ─────────────────────────────────────────────────────────────────────────────


def evaluate_compare(
    a: dict, b: dict, ship: str, *, speed_cap_mps: float, b_own_tolerance_m: float | None
) -> dict:
    """**항목마다 독립 verdict.** 하나의 `ok` 불리언으로 묶지 않는다.

    묶여 있던 동안 (1) SC-63 의 FAIL 이 SC-61·62 의 FAIL 로 인쇄됐고, (2) SC-62 는
    `ok` 에 **한 번도 들어가 있지 않아** 변위가 얼마든 다른 항목의 초록불을 그대로 받았다
    (자명 통과 — 계약 §7b 규칙 1). 분리가 그 둘을 동시에 드러낸다.
    """
    # ── SC-61: B 가 본 A 의 위치가 단조적으로 변하고 총 이동 거리가 설명 가능한가
    b_rows = sorted(((t, r) for (t, s), r in b.items() if s == ship), key=lambda x: x[0])
    if not b_rows:
        raise db.NotImplementedYet(f"B 의 CSV 에 함선 {ship} 이 없다 — B 가 A 를 보지 못했다")
    path_mm = sum(dist_mm(b_rows[i][1]["p"], b_rows[i + 1][1]["p"]) for i in range(len(b_rows) - 1))
    disp_mm = dist_mm(b_rows[0][1]["p"], b_rows[-1][1]["p"])
    ticks = b_rows[-1][0] - b_rows[0][0]
    seconds = ticks / 20.0
    max_possible_m = speed_cap_mps * seconds
    monotonic_back = sum(
        1
        for i in range(len(b_rows) - 1)
        if dist_mm(b_rows[0][1]["p"], b_rows[i + 1][1]["p"])
        < dist_mm(b_rows[0][1]["p"], b_rows[i][1]["p"]) - 1000  # 1 m 이상 되돌아감
    )
    # **C-1 (architect R17)**: `path_mm > 0` 은 1 mm 로도 참이라 **A 가 0.95 m 만 기어간 세션이
    # SC-61 PASS 를 받는다.** SC-62 (ii) 가 실행 전체는 막지만(미검증이면 블록이 안 닫힌다)
    # **SC-61 의 PASS 는 여전히 인쇄되고 그 PASS 는 거짓이다** — 나중에 읽는 사람에게
    # "SC-61 은 통과했다"로 남는 것이 이 슬라이스가 반복해서 다친 형태다.
    # SC-61 과 SC-62 는 **같은 조인·같은 창**을 보므로 조건이 갈릴 이유가 없다: 같은 수,
    # 같은 칸(`미검증(대조 없음)`), 같은 사유 이름. **새 상수 0개** — 아래 (ii) 의 것을 쓴다.
    a_net_disp_m = disp_mm / 1000.0
    a_flew = a_net_disp_m >= A_MIN_NET_DISPLACEMENT_M
    if len(b_rows) < 2:
        sc61 = "미검증(표본 없음)"
    elif not a_flew:
        sc61 = "미검증(대조 없음)"
    elif monotonic_back == 0 and path_mm > 0 and (path_mm / 1000.0) <= max_possible_m * 1.05:
        sc61 = "PASS"
    else:
        sc61 = "FAIL"

    # ── SC-62: B 자신의 함선이 스폰 근처에 머무는가 (B 의 관측자 actor 의 함선)
    #
    # **기대 변위는 "근처"가 아니라 정확히 0 이다** (architect `## R14 판정 — SC-62 의 수치 임계`):
    # 스폰이 `ShipPhysicsState::at_rest`(속도 0·각속도 0, `simulation.rs:798`)이고 B 는 추력 0·
    # 롤 0·목표 자세 = 현재 자세만 보내며 soft 경계 안이라 경계력도 없다. **가속원이 없으므로
    # 위치 정수는 상수여야 한다.** 임계 0.05 m 는 위치 양자화 LSB(1 mm)의 50배로 유도된 값이고,
    # **SC-64 의 2 m 를 빌려 오면 안 된다** — SC-64 는 *두 관측 경로의 차이*(기대값이 0 이 아니다)
    # 를 재고 SC-62 는 *한 물체의 물리적 변위*(기대값이 0)를 잰다. 2 m 는 SC-62 에서 30초 기준
    # 0.067 m/s 의 실제 표류를 통과시킨다.
    b_actor = next(iter(b.values()))["observer"]
    b_own = sorted(
        ((t, r) for (t, s), r in b.items() if r["observer"] == b_actor and s != ship),
        key=lambda x: x[0],
    )
    # **재는 양은 끝점 거리가 아니라 첫 표본 대비 최대 이탈** `max_i dist(p_i, p_0)` (계약 15차).
    # 끝점 거리는 **왕복 이탈을 0 으로 읽는다** — 나갔다 돌아온 함선이 "머물렀다"가 된다.
    b_own_max_dev_m = (
        max(dist_mm(r["p"], b_own[0][1]["p"]) for _, r in b_own) / 1000.0
        if len(b_own) >= 2 else None
    )
    b_own_end_disp_m = (
        dist_mm(b_own[0][1]["p"], b_own[-1][1]["p"]) / 1000.0 if len(b_own) >= 2 else None
    )

    # ── 자명 통과 방어 3절 (계약 SC-62, 전부 필수). 판정 순서도 계약 순서다.
    a_span = b_rows[-1][0] - b_rows[0][0]
    own_span = (b_own[-1][0] - b_own[0][0]) if len(b_own) >= 2 else 0
    # (i) 표본: 행 ≥ 2 · tick 스팬 ≥ 400(20초) · A 행 스팬의 90 % 이상을 덮는다.
    gate_i = (
        len(b_own) >= 2
        and own_span >= B_OWN_MIN_SPAN_TICKS
        and a_span > 0
        and own_span >= a_span * B_OWN_MIN_SPAN_RATIO
    )
    # (ii) 대조: 같은 창에서 **A 의 순변위 ≥ 100 m**. `path_mm > 0` 은 이 자리를 못 막는다 —
    #      1 mm 로도 참이라 아무도 움직이지 않은 세션이 SC-61 PASS + SC-62 PASS 를 같이 낸다.
    gate_ii = a_flew          # SC-61 과 **같은 값**을 쓴다 — 두 번 계산하면 갈릴 수 있다

    # **기록(판정 아님)** — 창 **안쪽**의 기록 중단. (i) 은 계약대로 스팬 양 끝과 비율만 보므로
    # **한가운데가 통째로 비어도 통과한다.** Unity 의 `runInBackground: 0` 때문에 조종사가 창을
    # 전환하면 `Update()` 가 멈추고 CSV 가 끊기는데(R15 세션에서 실제로 일어났다), 그 세션은
    # 스팬도 비율도 멀쩡하다. 계약에 없는 판정을 새로 걸지 않고 **관측만 낸다** — 값이 크면
    # 리포트가 그 사실을 적는다(§0.4 기록).
    def _max_gap(rows: list) -> int:
        return max(
            (rows[i + 1][0] - rows[i][0] for i in range(len(rows) - 1)), default=0
        )
    a_row_gap = _max_gap(b_rows)
    own_row_gap = _max_gap(b_own)

    if not gate_i:
        sc62 = "미검증(표본 없음)"
    elif not gate_ii:
        sc62 = "미검증(대조 없음)"
    elif b_own_tolerance_m is None:
        # (iii) 도구는 숫자를 지어내지 않는다. 계약 행에 있는 수의 **사본을 코드에 두지 않는다** —
        # 사본을 두면 계약이 바뀌었을 때 도구가 옛 수로 조용히 초록을 낸다(architect R15).
        sc62 = "미검증(판정 기준 미지정)"
    else:
        sc62 = "PASS" if b_own_max_dev_m <= b_own_tolerance_m else "FAIL"

    return {
        "item": "SC-61/62 (AC-16) 두 관측자 대조 — 블록 7",
        "SC-61_verdict": sc61,
        "SC-62_verdict": sc62,
        # architect R16: **필요한 성질은 "갈릴 수 없다"가 아니라 "갈림이 조용할 수 없다"다.**
        # 임계(verdict 를 뒤집는 수)는 인자로 받고, 표본 요건은 코드 상수로 두되 **도구가 자기가
        # 쓴 판정 입력값을 전부 여기에 찍는다** — 그러면 계약 행과의 대조가 기계로 가능해지고,
        # 인자를 넷으로 늘리는 것보다 싸면서 같은 성질을 준다.
        "judgment_inputs": {
            "sc62_tolerance_m": b_own_tolerance_m,          # 인자 (계약 SC-62 (iii))
            "sc62_min_span_ticks": B_OWN_MIN_SPAN_TICKS,    # 코드 상수 (계약 SC-62 (i))
            "sc62_min_coverage_ratio": B_OWN_MIN_SPAN_RATIO,
            "sc62_a_min_net_displacement_m": A_MIN_NET_DISPLACEMENT_M,   # 계약 SC-62 (ii)
            "sc61_speed_cap_mps": speed_cap_mps,
            "sc61_path_slack": 1.05,
            "sc61_backward_threshold_mm": 1000,
        },
        "join_key": "(tick, ship_id)",
        "rows_a": len(a), "rows_b": len(b),
        "pairs_compared": len(set(a) & set(b)),
        "SC-61": {
            "ship": ship,
            "a_net_displacement_m": round(a_net_disp_m, 3),
            "contrast_required_m": A_MIN_NET_DISPLACEMENT_M,
            "contrast_ok": a_flew,
            "samples": len(b_rows),
            "tick_span": ticks,
            "seconds": round(seconds, 2),
            "path_length_m": round(path_mm / 1000.0, 3),
            "displacement_m": round(disp_mm / 1000.0, 3),
            "max_possible_m_at_max_speed": round(max_possible_m, 1),
            "backward_steps_over_1m": monotonic_back,
        },
        # 계약 방법 칸이 요구하는 다섯: 최대 이탈 · 표본 수 · tick 스팬 · 같은 창의 A 순변위 ·
        # **넘긴 임계값**. 마지막 것이 (iii) 의 증거다 — 무슨 수로 판정했는지가 출력에 남는다.
        "SC-62": {
            "max_deviation_m": b_own_max_dev_m,
            "max_deviation_mm": None if b_own_max_dev_m is None else round(b_own_max_dev_m * 1000.0, 1),
            "samples": len(b_own),
            "tick_span": own_span,
            "a_net_displacement_m": round(a_net_disp_m, 3),
            "tolerance_m_passed_in": b_own_tolerance_m,
            "expected_m": 0.0,
            # 참고값. **판정에 쓰지 않는다** — 왕복 이탈을 0 으로 읽는 양이다.
            "endpoint_displacement_m": b_own_end_disp_m,
            # 관대한 임계 뒤에 실제 신호를 숨기지 않는다: LSB 1개(1 mm)를 넘으면 통과하더라도
            # 리포트에 한 문장으로 설명한다.
            "exceeds_one_lsb_1mm": (
                None if b_own_max_dev_m is None else (b_own_max_dev_m * 1000.0 > 1.0)
            ),
            "defense_i_sampling": {
                "rows": len(b_own),
                "own_tick_span": own_span,
                "min_span_ticks": B_OWN_MIN_SPAN_TICKS,
                "a_tick_span": a_span,
                "coverage_ratio": round(own_span / a_span, 3) if a_span else None,
                "min_coverage_ratio": B_OWN_MIN_SPAN_RATIO,
                "ok": gate_i,
            },
            # 기록. 판정에 쓰지 않는다 — 계약 (i) 은 스팬과 비율만 요구한다.
            "recorded_row_gaps_ticks": {
                "max_gap_between_a_ship_rows": a_row_gap,
                "max_gap_between_b_own_rows": own_row_gap,
                "expected_gap_ticks": 2,
                "note": (
                    "**두 값은 서로 다른 기록 층이고 같은 잣대로 읽으면 안 된다**(qa r13 §3, "
                    "실측으로 정정). `max_gap_between_a_ship_rows` 는 **원격 함선 행** — "
                    "`ObserverSession.OnWorldSnapshot()` 이 **스냅샷 메시지마다** 쓴다. "
                    "**이것이 기록 연속성의 지표다**: 2 보다 크면 CSV 가 실제로 끊긴 것이다. "
                    "`max_gap_between_b_own_rows` 는 **자기 함선 행** — F-2 배치 수정 이후 "
                    "`ApplyPendingRebase()` 가 **`Update()` 당 최대 한 행**만, 그 프레임에 "
                    "드레인된 스냅샷 중 **가장 높은 tick** 하나에 대해 쓴다"
                    "(`ObserverSession.cs:262-271`). 한 `Update()` 에 스냅샷이 N 개 들어오면 "
                    "원격 행은 N 개, 자기 함선 행은 1 개다 — **설계상 그렇다.** 따라서 이 값이 "
                    "크다는 것은 **기록이 끊긴 것이 아니라 메인 스레드가 그만큼 지연됐다**는 뜻이고, "
                    "둘을 가르는 것은 **같은 창의 원격 행 간격**이다(그것이 2 면 기록은 멀쩡하다). "
                    "스팬·비율 게이트는 두 경우를 모두 통과시키므로 리포트가 값을 적는다(§0.4 기록)."
                ),
            },
            "defense_ii_contrast": {
                "a_net_displacement_m": round(a_net_disp_m, 3),
                "required_m": A_MIN_NET_DISPLACEMENT_M,
                "ok": gate_ii,
                "why_not_path_gt_0": (
                    "`path_mm > 0` 은 1 mm 로도 참이라 아무도 움직이지 않은 세션이 "
                    "SC-61 PASS + SC-62 PASS 를 동시에 인쇄한다(architect R15)"
                ),
            },
            "note": (
                "기대값은 **정확히 0** 이다 — B 는 at_rest 로 스폰하고 가속원이 하나도 없다. "
                "임계는 **인자로만** 받는다(계약 (iii)): 계약 행에 있는 수의 사본을 코드에 두면 "
                "한 수에 출처가 둘이 되고, 계약이 바뀌었을 때 도구가 옛 수로 조용히 초록을 낸다."
            ),
        },
        "SC-63": (
            "이 명령은 SC-63 을 판정하지 않는다 (13차 개정, 블록 10). "
            "판정 입력은 봇 2대의 원시 스냅샷 CSV 이고 명령은 `raw2 --bot-out-dir` 다. "
            "이 CSV 에는 원시 층이 한 열도 없다."
        ),
    }


def cmd_compare(args: argparse.Namespace) -> int:
    result = evaluate_compare(
        load(args.a), load(args.b), args.ship,
        speed_cap_mps=args.speed_mps,
        b_own_tolerance_m=args.b_own_tolerance_m,
    )
    db.emit(result, args.evidence)
    verdicts = [result["SC-61_verdict"], result["SC-62_verdict"]]
    if "FAIL" in verdicts:
        return db.EXIT_FAIL
    if any(v.startswith("미검증") for v in verdicts):
        return EXIT_EVIDENCE
    return db.EXIT_OK


def cmd_s3(args: argparse.Namespace) -> int:
    """designer S-3: 정지 비교(≤ 2 m)와 **부호 있는** 이동 비교(뒤쪽 28 ± 12 m)."""
    a = load(args.a)
    b = load(args.b)
    ship = args.ship

    still, moving = [], []
    for (tick, sid) in sorted(set(a) & set(b)):
        if sid != ship:
            continue
        ra, rb = a[(tick, sid)], b[(tick, sid)]
        sp = speed_mps(ra["v"])
        diff = tuple(rb["p"][i] - ra["p"][i] for i in range(3))  # B가 본 것 − A가 본 것
        gap_m = math.dist((0, 0, 0), diff) / 1000.0
        if sp < args.still_speed_mps:
            still.append({
                "tick": tick,
                "gap_m": round(gap_m, 3),
                "speed_mps": round(sp, 2),
                # 두 화면 각각의 오프셋 중 큰 쪽 = gap 에 기여할 수 있는 상한.
                "render_offset_mm": max(ra["render_offset_mm"], rb["render_offset_mm"]),
            })
        elif sp >= args.moving_speed_mps:
            # 진행 방향 단위 벡터에 투영 → **부호**가 판정의 핵심
            vlen = math.dist((0, 0, 0), ra["v"])
            proj_m = sum(diff[i] * ra["v"][i] for i in range(3)) / vlen / 1000.0 if vlen else 0.0
            moving.append({
                "tick": tick,
                "along_track_m": round(proj_m, 3),
                "gap_m": round(gap_m, 3),
                "speed_mps": round(sp, 2),
            })

    still_max = max((s["gap_m"] for s in still), default=None)
    behind = [-m["along_track_m"] for m in moving]  # 뒤쪽이 양수가 되게 부호를 뒤집는다
    behind_mean = sum(behind) / len(behind) if behind else None

    still_offset_max = max((s["render_offset_mm"] for s in still), default=None)
    sc64 = "미검증(표본 없음)" if not still else ("PASS" if still_max <= args.still_tolerance_m else "FAIL")

    # R23: SC-64 초과의 귀속. §7a — 값만 싣고 해석을 사람에게 미루면 "추론으로 판정"하는
    # 자리가 하나 더 생긴다. 초과분을 평활화가 덮을 수 있는지를 도구가 말한다.
    if not still or still_max is None:
        still_attribution = None
    elif sc64 != "FAIL":
        still_attribution = "해당 없음 (초과 없음)"
    else:
        excess_m = round(still_max - args.still_tolerance_m, 3)
        offset_m = (still_offset_max or 0) / 1000.0
        if offset_m >= excess_m:
            still_attribution = f"평활화로 설명 가능 — 초과분 {excess_m} m <= 관측 최대 오프셋 {offset_m} m"
        else:
            still_attribution = (
                f"평활화로 설명되지 않는다 — 초과분 {excess_m} m > 관측 최대 오프셋 {offset_m} m. "
                "두 화면이 실제로 다르다"
            )
    if not moving:
        sc65 = "미검증(표본 없음)"
    else:
        lo, hi = args.behind_m - args.behind_tol_m, args.behind_m + args.behind_tol_m
        sc65 = "PASS" if behind_mean is not None and lo <= behind_mean <= hi else "FAIL"

    # R24: 자기 함선 행에 오프셋이 실제로 실렸는가. **전 행 0 은 세 원인을 구분하지 못한다**
    # (보정 미발생 / Update() 감쇠 누락 / client 미구현) — 값으로 못 가르므로 관측 수만 낸다.
    off_rows_a = sum(1 for r in a.values() if r["render_offset_mm"] != 0 or r["render_offset_deg"] != 0)
    off_rows_b = sum(1 for r in b.values() if r["render_offset_mm"] != 0 or r["render_offset_deg"] != 0)

    db.emit(
        {
            "item": "SC-64/65 (designer S-3) 두 화면 비교",
            "SC-64_verdict": sc64,
            "SC-65_verdict": sc65,
            "SC-64": {
                "samples": len(still),
                "max_gap_m": still_max,
                "tolerance_m": args.still_tolerance_m,
                "max_render_offset_mm": still_offset_max,
                "render_offset_attribution": still_attribution,
                "note": "정지 상태에서는 예측도 보간도 같은 값을 내므로 지연이 오차를 만들지 않는다",
            },
            "SC-65": {
                "samples": len(moving),
                "behind_mean_m": round(behind_mean, 3) if behind_mean is not None else None,
                "expected_m": f"{args.behind_m} ± {args.behind_tol_m} (뒤쪽이 양수)",
                "sign": (
                    "뒤쪽(기대)" if behind_mean and behind_mean > 0
                    else ("앞쪽 — 외삽 과다이거나 보간 버퍼가 비었다" if behind_mean else None)
                ),
                "samples_head": moving[:5],
                "warning": (
                    "관측자 B 가 봇이면 이 수치는 무의미하다 — 봇은 보간이 없어 부호가 뒤집힌다"
                    "(계약 §0.11). B 는 보간 파이프라인이어야 한다."
                ),
            },
            "render_offset_presence": {
                "rows_a": len(a), "nonzero_offset_rows_a": off_rows_a,
                "rows_b": len(b), "nonzero_offset_rows_b": off_rows_b,
                "max_offset_mm_a": max((r["render_offset_mm"] for r in a.values()), default=None),
                "max_offset_mm_b": max((r["render_offset_mm"] for r in b.values()), default=None),
                "note": (
                    "전 행 0 이면 **이 값만으로 판정하지 않는다** — (i) 보정 미발생 "
                    "(ii) Update() 감쇠 누락 (iii) client 미구현을 값으로 구분할 수 없다. "
                    "절차서의 세 갈래 구분법(재조정 카운터·계측 로그·코드 확인)을 따른다."
                ),
            },
        },
        args.evidence,
    )
    bad = (sc64 == "FAIL") or (sc65 == "FAIL")
    return db.EXIT_FAIL if bad else db.EXIT_OK


# ─────────────────────────────────────────────────────────────────────────────
# 블록 10 — SC-63 (봇 2대 원시 스냅샷 대조)
# ─────────────────────────────────────────────────────────────────────────────


def evaluate_raw2(out_dir: Path) -> tuple[dict, str]:
    """계약 SC-63 의 **자명 통과 방어 6절 (a)~(f)** 를 도구가 검사한다.

    입력을 **봇 실행 디렉터리**로 받는 것 자체가 (c) 의 뼈대다. 13차 개정 이후 봇 CSV 와
    Unity CSV 는 **같은 12열 헤더**라, 파일 경로를 받으면 실수로 섞어도 도구가 받아들인다.
    디렉터리를 받으면 `summary.json`(봇 하네스만 쓴다, `report.rs:230`)이 동반돼야 하고,
    그 안의 `snapshots_per_bot[].observer_actor_id` 집합이 CSV 의 관측자 집합과 같아야 한다.
    **Unity 는 이 파일을 쓰지 않는다** — 경로 하나를 바꿔 끼우는 사고가 구조적으로 막힌다.
    """
    csv_path = out_dir / "snapshots.csv"
    summary_path = out_dir / "summary.json"
    sessions_path = out_dir / "sessions.json"

    # ── 출처 게이트 P1: 봇 하네스 산출물인가
    for p in (csv_path, summary_path):
        if not p.is_file():
            raise db.EnvironmentProblem(
                f"{p} 가 없다. `raw2` 는 봇 하네스의 --out 디렉터리를 받는다 "
                "(snapshots.csv + summary.json). CSV 경로만으로는 출처를 증명할 수 없다."
            )
    summary = json.loads(summary_path.read_text(encoding="utf-8"))
    per_bot = summary.get("snapshots_per_bot")
    if not isinstance(per_bot, list):
        raise db.NotImplementedYet(
            f"{summary_path}: snapshots_per_bot 이 없다 — 봇 하네스 산출물이 아니다"
        )
    producer = {
        "out_dir": str(out_dir),
        "snapshots_csv": str(csv_path),
        "summary_json": str(summary_path),
        "stage": summary.get("stage"),
        "url": summary.get("url"),
        "bots": summary.get("bots"),
        "seed": summary.get("seed"),
        "duration_secs": summary.get("duration_secs"),
        "sessions_json_present": sessions_path.is_file(),
    }

    rows = read_rows(csv_path)
    observers_csv = sorted({r["observer"] for r in rows})
    observers_summary = sorted(
        {s["observer_actor_id"] for s in per_bot if s.get("observer_actor_id")}
    )
    own_ships = sorted(
        {s["controlled_ship_id"] for s in per_bot if s.get("controlled_ship_id")}
    )

    # ── 출처 게이트 P2: CSV 의 관측자 집합 == summary 의 관측자 집합
    if observers_csv != observers_summary:
        raise db.NotImplementedYet(
            "CSV 의 관측자 집합이 summary.json 과 다르다 — 다른 실행의 파일이 섞였다.\n"
            f"  csv     : {observers_csv}\n  summary : {observers_summary}"
        )
    structural = {
        "observers_in_csv": observers_csv,
        "observer_count": len(observers_csv),
        "own_ship_ids": own_ships,
        "rows_total": len(rows),
    }
    if len(observers_csv) < 2:
        return (
            {"gate": "P2", "detail": "관측자가 2명 미만이다 — 봇 2대 동시 세션이 아니다",
             "producer": producer, "structural": structural},
            "미검증(환경, E10)",
        )

    # ── (c) 출처 게이트: 봇에는 평활화가 없으므로 뒤 두 열이 **모든 행에서** 0 이다.
    #    한 행이라도 비-0 이면 Unity CSV 를 먹인 것이므로 **미검증(증거 요건)** 이다 — FAIL 이 아니다.
    nonzero = [
        {k: v for k, v in r.items() if k != "render_offset_deg_literal"}
        for r in rows
        if r["render_offset_mm"] != 0 or r["render_offset_deg"] != 0.0
    ]
    # 보조 지문: client 는 `F4`(`0.0000`), 봇은 정수 리터럴(`0`) 로 쓴다. **값이 같아도 표기가
    # 다르다** — 13차 개정으로 두 CSV 의 헤더가 같아진 뒤 남은 유일한 텍스트 수준 구분자다.
    deg_literals = sorted({r["render_offset_deg_literal"] for r in rows})
    decimal_literals = [v for v in deg_literals if "." in v]
    gate_c = {
        "nonzero_offset_rows": len(nonzero),
        "sample": nonzero[:3],
        "render_offset_deg_literals": deg_literals[:5],
        "decimal_formatted_literals": decimal_literals[:5],
        "note": (
            "값 게이트(전 행 0)는 **역방향 보증만 한다** — Unity 세션도 보정이 없으면 0 일 수 "
            "있다. 그래서 표기 지문을 함께 본다: `0.0000` 처럼 소수점이 있으면 client 의 "
            "`ObserverCsv.ToCsvLine()`(F4) 이 쓴 것이고 봇(`snapshot.rs` 의 상수 `,0,0`)이 "
            "아니다. 정방향 보증은 P1·P2(봇 산출물 동반·관측자 집합 일치)와 (d) 가 진다."
        ),
    }
    if nonzero:
        return (
            {"gate": "(c) 출처 — 값", "detail": "render_offset 이 비-0 인 행이 있다 — 봇 CSV 가 아니다",
             "producer": producer, "structural": structural, "gate_c_source": gate_c},
            "미검증(증거 요건)",
        )
    if decimal_literals:
        return (
            {"gate": "(c) 출처 — 표기", "detail":
                f"render_offset_deg 이 소수 표기다 {decimal_literals[:3]} — client 의 F4 출력이다",
             "producer": producer, "structural": structural, "gate_c_source": gate_c},
            "미검증(증거 요건)",
        )

    # ── 대조: 두 관측자를 짝지어 같은 (tick, ship_id) 의 정수 6개를 비교
    by_obs: dict[str, dict[tuple[int, str], dict]] = {}
    for r in rows:
        by_obs.setdefault(r["observer"], {})[(r["tick"], r["ship_id"])] = r
    oa, ob = observers_csv[0], observers_csv[1]
    a, b = by_obs[oa], by_obs[ob]
    common = sorted(set(a) & set(b))

    per_ship: dict[str, int] = {}
    mismatch = []
    max_speed = 0.0
    moving_ticks = 0
    for key in common:
        per_ship[key[1]] = per_ship.get(key[1], 0) + 1
        ra, rb = a[key], b[key]
        sp = speed_mps(ra["v"])
        if sp > 0:
            moving_ticks += 1
        max_speed = max(max_speed, sp)
        if ra["p"] != rb["p"] or ra["v"] != rb["v"]:
            mismatch.append(
                {"tick": key[0], "ship_id": key[1], "a": [ra["p"], ra["v"]], "b": [rb["p"], rb["v"]]}
            )

    # ── (a) 교집합이 비면 `0 == 0` 이다. 두 봇의 **자기 함선 각각**에 쌍이 있어야 한다.
    ships_with_pairs = sorted(per_ship)
    required = own_ships if own_ships else ships_with_pairs
    missing = [s for s in required if per_ship.get(s, 0) == 0]
    gate_a = {
        "pairs_compared": len(common),
        "ships_with_pairs": len(ships_with_pairs),
        "pairs_per_ship": per_ship,
        "required_ships": required,
        "ships_without_pairs": missing,
        "note": (
            "required_ships 는 summary.json 의 controlled_ship_id 둘이다. 거기 없으면 "
            "교집합의 함선 목록으로 대체하고, 그 경우 2척 미만이면 표본 없음으로 닫는다."
        ),
    }
    # ── (b) 정지 세션만으로 닫으면 정말로 항진명제가 된다.
    gate_b = {
        "max_speed_mps": round(max_speed, 3),
        "ticks_with_speed_gt_0": moving_ticks,
    }

    detail = {
        "producer": producer,               # (d)
        "structural": structural,
        "gate_a_sampling": gate_a,
        "gate_b_motion": gate_b,
        "gate_c_source": gate_c,
        "observer_a": oa, "observer_b": ob,
        "mismatching_pairs": len(mismatch),
        "mismatch_head": mismatch[:5],
        "positive_control_one_mm_offset_mismatches": _selfcheck_one_mm(),  # (e)
    }

    if len(common) == 0 or len(required) < 2 or missing:
        return detail, "미검증(표본 없음)"
    if moving_ticks == 0 or max_speed <= 0.0:
        return detail, "미검증(표본 없음)"
    if detail["positive_control_one_mm_offset_mismatches"] != 20:
        return detail, "미검증(증거 요건)"   # 검출기가 죽었다 — 0건을 통과로 읽지 않는다
    return detail, ("PASS" if not mismatch else "FAIL")


def cmd_raw2(args: argparse.Namespace) -> int:
    detail, verdict = evaluate_raw2(Path(args.bot_out_dir))
    db.emit(
        {
            "item": "SC-63 (AC-16 c/d) 봇 2대 원시 스냅샷 대조 — 블록 10",
            "SC-63_verdict": verdict,
            "contract": "13차 개정 — 판정 입력은 봇 2대의 원시 CSV. Unity 관측자 CSV 가 아니다",
            **detail,
        },
        args.evidence,
    )
    if verdict == "PASS":
        return db.EXIT_OK
    if verdict == "FAIL":
        return db.EXIT_FAIL
    return EXIT_EVIDENCE


# ─────────────────────────────────────────────────────────────────────────────
# selftest — 도구가 살아 있는가
# ─────────────────────────────────────────────────────────────────────────────

_SHIP = "01a0c000-0000-7000-8000-00000000000a"
_SHIP_B = "01a0c000-0000-7000-8000-00000000000b"
_ACTOR_A = "aaaa0000-0000-7000-8000-000000000001"
_ACTOR_B = "bbbb0000-0000-7000-8000-000000000002"
NL = chr(10)
# 계약 SC-62 (i) 이 B 자기 행의 tick 스팬 >= 400(20초, @20 Hz)을 요구한다. 픽스처가 그 아래면
# 방어 (i) 이 늘 걸려 **임계와 (ii) 를 한 번도 시험하지 못한다** — 방어가 다른 대조를 가리는
# 형태다. 20 표본 × 22 tick = 418 tick 스팬.
_TICK_STEP = 22


def _write(path: Path, actor: str, offset_mm: int, behind_mm: int, *, ship: str = _SHIP) -> None:
    with path.open("w", encoding="utf-8", newline="") as f:
        f.write(",".join(COLUMNS) + NL)
        for i in range(20):
            tick = 100 + i * _TICK_STEP
            z = 1_000_000 + i * 7_000 + offset_mm - behind_mm
            # 끝 두 값이 R23 신설 열(render_offset_mm, render_offset_deg)이다. selftest 는
            # 평활화를 흉내내지 않으므로 0 이다 — 봇 CSV 가 0 인 것과 같은 이유다.
            f.write(f"{tick},{actor},{ship},ACTIVE,0,0,{z},0,0,140000,0,0" + NL)


def _selfcheck_one_mm() -> int:
    """(e) 양성 대조 — 1 mm 를 어긋뜨렸을 때 정수 비교가 20건 전부 잡는가."""
    import tempfile

    tmp = Path(tempfile.mkdtemp(prefix="starfall-posctl-"))
    fa, fb = tmp / "a.csv", tmp / "b.csv"
    _write(fa, _ACTOR_A, 0, 0)
    _write(fb, _ACTOR_B, 1, 0)
    return _mismatches(fa, fb, _SHIP)


def _write_bot_dir(
    root: Path,
    *,
    observers: list[str],
    ships: list[str],
    own: list[str],
    speed_mm_s: int,
    offset_mm: int = 0,
    mismatch_mm: int = 0,
    ticks: int = 20,
) -> Path:
    """봇 하네스 산출물을 흉내낸다 — `snapshots.csv`(합본) + `summary.json`."""
    root.mkdir(parents=True, exist_ok=True)
    with (root / "snapshots.csv").open("w", encoding="utf-8", newline="") as f:
        f.write(",".join(COLUMNS) + NL)
        for oi, obs in enumerate(observers):
            for i in range(ticks):
                tick = 100 + i * 2
                for sid in ships:
                    # 두 번째 관측자만 mismatch_mm 만큼 어긋난다(FAIL 대조용).
                    z = 1_000_000 + i * speed_mm_s // 10 + (mismatch_mm if oi else 0)
                    f.write(
                        f"{tick},{obs},{sid},ACTIVE,0,0,{z},0,0,{speed_mm_s},"
                        f"{offset_mm if oi else 0},0" + NL
                    )
    summary = {
        "stage": "e-fly", "url": "ws://127.0.0.1:8080/ws", "bots": len(observers),
        "seed": 42, "duration_secs": 30,
        "snapshots_per_bot": [
            {"observer_actor_id": o, "controlled_ship_id": c}
            for o, c in zip(observers, own)
        ],
    }
    (root / "summary.json").write_text(json.dumps(summary), encoding="utf-8")
    return root


def cmd_selftest(args: argparse.Namespace) -> int:
    """도구 검증. **여섯 갈래**:
    ① 정수 비교 검출기 ② SC-65 부호 ③ 헤더 가드(양성·음성 대조)
    ④ **verdict 분리**(SC-63 만 FAIL 인 입력에서 SC-61·62 가 PASS 로 인쇄되는가)
    ⑤ 그 분리의 **음성 대조**(되돌아가는 입력에서 SC-61 이 FAIL 인가)
    ⑥ **블록 10 방어 6절**(a)(b)(c)와 P1·P2, 그리고 진짜 불일치가 FAIL 로 나오는가
    """
    import tempfile

    tmp = Path(tempfile.mkdtemp(prefix="starfall-2cv-"))

    # ① 두 파일이 완전히 같으면 불일치 0
    fa, fb = tmp / "a.csv", tmp / "b.csv"
    _write(fa, _ACTOR_A, 0, 0)
    _write(fb, _ACTOR_B, 0, 0)
    same = _mismatches(fa, fb, _SHIP)

    # ② B 를 1 mm 어긋뜨리면 전부 불일치로 잡힌다
    fb2 = tmp / "b_off.csv"
    _write(fb2, _ACTOR_B, 1, 0)
    off = _mismatches(fa, fb2, _SHIP)

    # ③ 부호: B 가 28 m 뒤처지면 along-track 투영이 **뒤쪽 양수**로 나온다
    fb3 = tmp / "b_behind.csv"
    _write(fb3, _ACTOR_B, 0, 28_000)
    behind = _behind_mean(fa, fb3, _SHIP)
    fb4 = tmp / "b_ahead.csv"
    _write(fb4, _ACTOR_B, 0, -28_000)
    ahead = _behind_mean(fa, fb4, _SHIP)

    # ④ 헤더 불일치 검출기의 **양성 대조**. ObserverCsv.cs:49-51 이 "이름·순서·개수가 COLUMNS 와
    #    정확히 같아야 하고 불일치는 NotImplementedYet 으로 거부한다"를 계약으로 적고 있다.
    #    **COLUMNS 에서 파생시키므로 열이 추가·개명돼도 이 대조는 따라 움직인다.**
    header_cases: dict[str, str] = {}

    def _write_with_header(path: Path, cols: list[str]) -> None:
        with path.open("w", encoding="utf-8", newline="") as f:
            f.write(",".join(cols) + NL)
            pad = ",".join(["0"] * (len(cols) - 4))
            for i in range(20):
                f.write(f"{100 + i * 2},{_ACTOR_A},{_SHIP},ACTIVE,{pad}" + NL)

    renamed = list(COLUMNS)
    renamed[4] = renamed[4] + "_x"                      # 이름만 다르다
    reordered = list(COLUMNS)
    reordered[-1], reordered[-2] = reordered[-2], reordered[-1]   # 순서만 다르다
    appended = list(COLUMNS) + ["unexpected_extra_col"]  # 개수만 다르다 (= 한쪽만 고친 상태)
    truncated = list(COLUMNS)[:-1]                       # 개수만 다르다 (반대 방향)
    for name, cols in (
        ("renamed", renamed),
        ("reordered", reordered),
        ("appended", appended),
        ("truncated", truncated),
        ("exact_match", list(COLUMNS)),                  # 음성 대조 — 이것까지 거부하면 도구가 고장이다
    ):
        fp = tmp / f"hdr_{name}.csv"
        _write_with_header(fp, cols)
        try:
            load(fp)
            header_cases[name] = "accepted"
        except db.NotImplementedYet:
            header_cases[name] = "rejected(NotImplementedYet)"
        except Exception as exc:  # noqa: BLE001 - 어떤 예외였는지 증거에 그대로 남긴다
            header_cases[name] = f"{type(exc).__name__}"
    header_guard_ok = (
        all(header_cases[k] == "rejected(NotImplementedYet)"
            for k in ("renamed", "reordered", "appended", "truncated"))
        and header_cases["exact_match"] == "accepted"
    )

    # ⑤ **verdict 분리의 양성 대조** (계약 SC-63 (f), 리더 R14 작업 1 §7a).
    #    "SC-63 만 FAIL 인 입력"을 만든다: A 와 B 가 같은 함선에 대해 **1 mm 어긋난** 값을
    #    보지만(옛 묶음의 SC-63 은 20/20 FAIL), B 가 본 A 의 궤적은 단조·설명 가능하고
    #    (SC-61 PASS) B 자신의 함선은 제자리다(SC-62 PASS). 묶여 있던 동안 이 입력은
    #    **세 칸이 전부 FAIL** 로 인쇄됐다 — 그것이 이 수정의 존재 이유다.
    sep_a, sep_b = tmp / "sep_a.csv", tmp / "sep_b.csv"
    _write(sep_a, _ACTOR_A, 0, 0)
    with sep_b.open("w", encoding="utf-8", newline="") as f:
        f.write(",".join(COLUMNS) + NL)
        for i in range(20):
            tick = 100 + i * _TICK_STEP
            z = 1_000_000 + i * 7_000 + 1          # A 의 함선: 1 mm 어긋남 → 옛 SC-63 FAIL
            f.write(f"{tick},{_ACTOR_B},{_SHIP},ACTIVE,0,0,{z},0,0,140000,0,0" + NL)
            # B 자신의 함선은 제자리 (SC-62 표본, 변위 0)
            f.write(f"{tick},{_ACTOR_B},{_SHIP_B},ACTIVE,0,0,5000,0,0,0,0,0" + NL)
    sep = evaluate_compare(
        load(sep_a), load(sep_b), _SHIP, speed_cap_mps=140.0, b_own_tolerance_m=2.0
    )
    sep_legacy_mismatches = _mismatches(sep_a, sep_b, _SHIP)   # 옛 묶음이 봤을 값
    separation = {
        "legacy_sc63_mismatches_on_this_input": sep_legacy_mismatches,
        "SC-61_verdict": sep["SC-61_verdict"],
        "SC-62_verdict": sep["SC-62_verdict"],
        "sc63_key_is_prose_not_verdict": isinstance(sep["SC-63"], str),
    }
    separation_ok = (
        sep_legacy_mismatches == 20                 # 겨냥한 조건이 실제로 발생했다
        and sep["SC-61_verdict"] == "PASS"
        and sep["SC-62_verdict"] == "PASS"
        and separation["sc63_key_is_prose_not_verdict"]
    )
    # 음성 대조 — 분리가 "무조건 PASS 인쇄"가 아님을 보인다: A 가 되돌아가면 SC-61 이 FAIL 이다.
    back_a, back_b = tmp / "back_a.csv", tmp / "back_b.csv"
    _write(back_a, _ACTOR_A, 0, 0)
    with back_b.open("w", encoding="utf-8", newline="") as f:
        f.write(",".join(COLUMNS) + NL)
        for i in range(20):
            tick = 100 + i * _TICK_STEP
            # 15번째에서 되돌아가되 **순변위는 135 m 로 남긴다**(C-1 의 대조 문턱 위).
            # 순변위를 0 으로 두면 이 대조가 재려던 것(단조 위반)이 아니라 대조 게이트를 잰다.
            z = 1_000_000 + (i if i < 15 else 29 - i) * 15_000
            f.write(f"{tick},{_ACTOR_B},{_SHIP},ACTIVE,0,0,{z},0,0,140000,0,0" + NL)
            f.write(f"{tick},{_ACTOR_B},{_SHIP_B},ACTIVE,0,0,5000,0,0,0,0,0" + NL)
    back = evaluate_compare(
        load(back_a), load(back_b), _SHIP, speed_cap_mps=140.0, b_own_tolerance_m=2.0
    )
    separation["negative_control_SC-61_verdict_on_backtracking_input"] = back["SC-61_verdict"]

    # SC-62 의 임계와 자명 통과 방어 둘 (architect `## R14 판정 — SC-62 의 수치 임계`).
    # **각 방어가 걸려야 할 입력에서 실제로 걸리는가** — 방어를 넣어 두고 시험하지 않으면
    # 그 방어 자체가 자명 통과다.
    def _sc62_case(
        *, drift_mm: int, own_rows: int = 20, a_moves: bool = True,
        a_step_mm: int | None = None, return_to_start: bool = False,
        drop_middle: bool = False, tol: float | None = 0.05,
    ) -> dict:
        step = a_step_mm if a_step_mm is not None else (7_000 if a_moves else 0)
        tag = f"{drift_mm}_{own_rows}_{step}_{return_to_start}_{drop_middle}_{tol}"
        fa2, fb2x = tmp / f"s62_a_{tag}.csv", tmp / f"s62_b_{tag}.csv"
        with fa2.open("w", encoding="utf-8", newline="") as f:
            f.write(",".join(COLUMNS) + NL)
            for i in range(20):
                if drop_middle and 5 <= i <= 14:
                    continue
                z = 1_000_000 + i * step
                f.write(f"{100 + i * _TICK_STEP},{_ACTOR_A},{_SHIP},ACTIVE,0,0,{z},0,0,140000,0,0" + NL)
        with fb2x.open("w", encoding="utf-8", newline="") as f:
            f.write(",".join(COLUMNS) + NL)
            for i in range(20):
                if drop_middle and 5 <= i <= 14:
                    continue
                tick = 100 + i * _TICK_STEP
                z = 1_000_000 + i * step
                f.write(f"{tick},{_ACTOR_B},{_SHIP},ACTIVE,0,0,{z},0,0,140000,0,0" + NL)
                # B 자기 함선: own_rows 개만 쓴다 → 스팬·커버리지가 모자란 상태를 만든다.
                if i < own_rows:
                    n = max(own_rows - 1, 1)
                    if return_to_start:
                        # 나갔다 **돌아온다** — 끝점 거리는 0, 최대 이탈은 drift_mm.
                        drift = drift_mm * (i if i <= n // 2 else n - i) * 2 // n
                    else:
                        drift = drift_mm * i // n
                    f.write(f"{tick},{_ACTOR_B},{_SHIP_B},ACTIVE,0,0,{5000 + drift},0,0,0,0,0" + NL)
        return evaluate_compare(
            load(fa2), load(fb2x), _SHIP, speed_cap_mps=140.0, b_own_tolerance_m=tol
        )

    round_trip = _sc62_case(drift_mm=60, return_to_start=True)
    crawl = _sc62_case(drift_mm=0, a_step_mm=50)
    gapped = _sc62_case(drift_mm=0, drop_middle=True)
    sc62 = {
        # 임계 양쪽: 40 mm 통과, 60 mm FAIL. **0.05 가 실제로 판정 식에 들어갔다**는 단언이고,
        # SC-62 가 원래 앓던 병(판정 식에 항목이 아예 없었다)의 재발을 막는 대조다.
        "drift_40mm_tol_0.05": _sc62_case(drift_mm=40)["SC-62_verdict"],
        "drift_60mm_tol_0.05": _sc62_case(drift_mm=60)["SC-62_verdict"],
        # **최대 이탈이 끝점 거리와 다르다**: 60 mm 나갔다 돌아오면 끝점 거리는 0 이다.
        # 끝점 거리로 판정하면 이 입력이 PASS 가 된다 — 계약 15차가 고친 바로 그 형태.
        "round_trip_verdict": round_trip["SC-62_verdict"],
        "round_trip_max_deviation_mm": round_trip["SC-62"]["max_deviation_mm"],
        "round_trip_endpoint_mm": round(
            (round_trip["SC-62"]["endpoint_displacement_m"] or 0.0) * 1000.0, 1),
        # LSB 경고: 0 이 아니면 통과하더라도 리포트에 적어야 한다.
        "exceeds_one_lsb_at_40mm": _sc62_case(drift_mm=40)["SC-62"]["exceeds_one_lsb_1mm"],
        "exceeds_one_lsb_at_0mm": _sc62_case(drift_mm=0)["SC-62"]["exceeds_one_lsb_1mm"],
        # (i) 표본 — 스팬이 400 tick 에 못 미치거나 A 스팬의 90 % 를 못 덮는다
        "defense_i_half_window": _sc62_case(drift_mm=0, own_rows=10)["SC-62_verdict"],
        "defense_i_no_own_rows": _sc62_case(drift_mm=0, own_rows=0)["SC-62_verdict"],
        # (ii) 대조 — A 순변위가 100 m 에 못 미친다. **path_mm > 0 은 참인데도 걸려야 한다**
        "defense_ii_a_still": _sc62_case(drift_mm=0, a_moves=False)["SC-62_verdict"],
        "defense_ii_a_crawled": crawl["SC-62_verdict"],
        "defense_ii_crawl_sc61_path_m": crawl["SC-61"]["path_length_m"],
        # C-3 (architect R17): 같은 입력에서 **SC-61 도** 같은 칸을 내는가.
        # 이 줄이 붙기 전에는 이 입력이 **SC-61 PASS** 를 인쇄했고, 그 PASS 는 거짓이었다.
        "crawl_sc61_verdict": crawl["SC-61_verdict"],
        "crawl_a_net_displacement_m": crawl["SC-61"]["a_net_displacement_m"],
        # (iii) 임계를 안 주면 숫자를 지어내지 않는다 + 쓰인 임계가 출력에 남는가
        "defense_iii_no_tolerance": _sc62_case(drift_mm=0, tol=None)["SC-62_verdict"],
        "tolerance_echoed_in_output": _sc62_case(drift_mm=40)["SC-62"]["tolerance_m_passed_in"],
        # 기록 — **창 한가운데가 비어도 (i) 은 통과한다.** 그것이 관측을 낸 이유다.
        "mid_window_gap_still_passes_gate_i": gapped["SC-62_verdict"],
        "recorded_max_gap_ticks": gapped["SC-62"]["recorded_row_gaps_ticks"][
            "max_gap_between_b_own_rows"],
        "clean_session_max_gap_ticks": _sc62_case(drift_mm=0)["SC-62"][
            "recorded_row_gaps_ticks"]["max_gap_between_b_own_rows"],
    }
    separation["SC-62_threshold_and_defenses"] = sc62
    sc62_ok = (
        sc62["drift_40mm_tol_0.05"] == "PASS"
        and sc62["drift_60mm_tol_0.05"] == "FAIL"
        and sc62["round_trip_verdict"] == "FAIL"          # 끝점 거리였다면 PASS 였다
        and sc62["round_trip_max_deviation_mm"] > 50.0   # 임계 50 mm 위로 나갔다
        and sc62["round_trip_endpoint_mm"] == 0.0
        and sc62["exceeds_one_lsb_at_40mm"] is True
        and sc62["exceeds_one_lsb_at_0mm"] is False
        and sc62["defense_i_half_window"] == "미검증(표본 없음)"
        and sc62["defense_i_no_own_rows"] == "미검증(표본 없음)"
        and sc62["defense_ii_a_still"] == "미검증(대조 없음)"
        and sc62["defense_ii_a_crawled"] == "미검증(대조 없음)"
        and sc62["defense_ii_crawl_sc61_path_m"] > 0      # path_mm > 0 은 참인데도 걸렸다
        and sc62["crawl_sc61_verdict"] == "미검증(대조 없음)"   # C-1 전에는 PASS 였다
        and sc62["crawl_a_net_displacement_m"] < A_MIN_NET_DISPLACEMENT_M
        and sc62["defense_iii_no_tolerance"] == "미검증(판정 기준 미지정)"
        and sc62["tolerance_echoed_in_output"] == 0.05
        # 창 한가운데가 통째로 비어도 (i) 은 통과한다 — 관측이 유일한 단서다
        and sc62["mid_window_gap_still_passes_gate_i"] == "PASS"
        and sc62["recorded_max_gap_ticks"] > sc62["clean_session_max_gap_ticks"]
        and sc62["clean_session_max_gap_ticks"] == _TICK_STEP
    )
    separation_ok = (
        separation_ok
        and back["SC-61_verdict"] == "FAIL"
        and sc62_ok
    )

    # ⑥ **블록 10 방어 6절의 자체 검증.** 각 방어가 *걸려야 할 입력*에서 실제로 걸리는가.
    b10: dict[str, str] = {}
    o1, o2 = _ACTOR_A, _ACTOR_B

    def _verdict(root: Path) -> str:
        try:
            return evaluate_raw2(root)[1]
        except (db.NotImplementedYet, db.EnvironmentProblem) as exc:
            return f"rejected({type(exc).__name__})"

    # 정상 — 두 봇, 두 함선, 움직임 있음, 오프셋 0
    good = _write_bot_dir(tmp / "b10_good", observers=[o1, o2], ships=[_SHIP, _SHIP_B],
                          own=[_SHIP, _SHIP_B], speed_mm_s=140_000)
    b10["good_pass"] = _verdict(good)
    # (a) 교집합 공집합 — 두 봇이 서로 다른 함선만 본다
    empty = tmp / "b10_empty"
    empty.mkdir(parents=True, exist_ok=True)
    with (empty / "snapshots.csv").open("w", encoding="utf-8", newline="") as f:
        f.write(",".join(COLUMNS) + NL)
        for i in range(20):
            tick = 100 + i * 2
            f.write(f"{tick},{o1},{_SHIP},ACTIVE,0,0,{1_000_000 + i * 7000},0,0,140000,0,0" + NL)
            f.write(f"{tick},{o2},{_SHIP_B},ACTIVE,0,0,{2_000_000 + i * 7000},0,0,140000,0,0" + NL)
    (empty / "summary.json").write_text(json.dumps({
        "stage": "e-fly", "bots": 2, "seed": 42, "duration_secs": 30,
        "snapshots_per_bot": [
            {"observer_actor_id": o1, "controlled_ship_id": _SHIP},
            {"observer_actor_id": o2, "controlled_ship_id": _SHIP_B},
        ],
    }), encoding="utf-8")
    b10["a_empty_intersection"] = _verdict(empty)
    # (b) 정지 세션 — 속도가 전부 0
    still = _write_bot_dir(tmp / "b10_still", observers=[o1, o2], ships=[_SHIP, _SHIP_B],
                           own=[_SHIP, _SHIP_B], speed_mm_s=0)
    b10["b_all_still"] = _verdict(still)
    # (c) Unity CSV 재투입 — render_offset 이 비-0
    unity = _write_bot_dir(tmp / "b10_unity", observers=[o1, o2], ships=[_SHIP, _SHIP_B],
                           own=[_SHIP, _SHIP_B], speed_mm_s=140_000, offset_mm=315)
    b10["c_nonzero_render_offset"] = _verdict(unity)
    # (c) 보조 — **값이 0 이어도** client 의 F4 표기(`0.0000`)면 봇 CSV 가 아니다.
    #     값 게이트만으로는 "보정이 한 번도 안 일어난 Unity 세션"을 통과시킨다.
    f4 = _write_bot_dir(tmp / "b10_f4", observers=[o1, o2], ships=[_SHIP, _SHIP_B],
                        own=[_SHIP, _SHIP_B], speed_mm_s=140_000)
    p = f4 / "snapshots.csv"
    p.write_text(
        p.read_text(encoding="utf-8").replace(",0,0" + NL, ",0,0.0000" + NL), encoding="utf-8"
    )
    b10["c_f4_decimal_literal"] = _verdict(f4)
    # P1 출처 — summary.json 이 없다 (CSV 만 있는 디렉터리 = Unity CSV 를 끼워 넣은 형태)
    bare = _write_bot_dir(tmp / "b10_bare", observers=[o1, o2], ships=[_SHIP, _SHIP_B],
                          own=[_SHIP, _SHIP_B], speed_mm_s=140_000)
    (bare / "summary.json").unlink()
    b10["p1_no_summary"] = _verdict(bare)
    # P2 출처 — 관측자 집합 불일치 (다른 실행의 CSV 를 덮어 끼웠다)
    mixed = _write_bot_dir(tmp / "b10_mixed", observers=[o1, o2], ships=[_SHIP, _SHIP_B],
                           own=[_SHIP, _SHIP_B], speed_mm_s=140_000)
    (mixed / "summary.json").write_text(json.dumps({
        "stage": "e-fly", "bots": 2, "snapshots_per_bot": [
            {"observer_actor_id": o1, "controlled_ship_id": _SHIP},
            {"observer_actor_id": "cccc0000-0000-7000-8000-000000000003",
             "controlled_ship_id": _SHIP_B},
        ],
    }), encoding="utf-8")
    b10["p2_observer_set_mismatch"] = _verdict(mixed)
    # FAIL 대조 — 진짜 불일치가 FAIL 로 나오는가 (검출기 사망형 방어)
    bad = _write_bot_dir(tmp / "b10_bad", observers=[o1, o2], ships=[_SHIP, _SHIP_B],
                         own=[_SHIP, _SHIP_B], speed_mm_s=140_000, mismatch_mm=1)
    b10["fail_on_1mm_mismatch"] = _verdict(bad)
    # 블록 7 은 **진짜 Unity CSV** 를 읽는다 — `render_offset_deg` 가 `F4` 표기다
    # (`ObserverCsv.cs:89`). 정수로 읽으면 실세션 첫 행에서 ValueError 로 죽는다.
    # 이 열이 판정에 쓰인 적이 없어 R23 헤더 동기화만으로는 드러나지 않았다.
    f4row = tmp / "unity_f4.csv"
    with f4row.open("w", encoding="utf-8", newline="") as f:
        f.write(",".join(COLUMNS) + NL)
        for i in range(3):
            f.write(f"{100 + i * 2},{_ACTOR_A},{_SHIP},ACTIVE,0,0,{i},0,0,0,315,0.4213" + NL)
    try:
        _rows = read_rows(f4row)
        b10["unity_f4_row_parses"] = (
            "ok" if _rows[0]["render_offset_deg"] == 0.4213 else "wrong_value"
        )
    except Exception as exc:  # noqa: BLE001
        b10["unity_f4_row_parses"] = f"{type(exc).__name__}"

    # 블록 7 오투입 방어 — 합본 CSV 를 load() 에 먹이면 거부되는가
    try:
        load(good / "snapshots.csv")
        b10["block7_rejects_merged_csv"] = "accepted(도구 고장)"
    except db.NotImplementedYet:
        b10["block7_rejects_merged_csv"] = "rejected(NotImplementedYet)"

    b10_ok = (
        b10["good_pass"] == "PASS"
        and b10["a_empty_intersection"] == "미검증(표본 없음)"
        and b10["b_all_still"] == "미검증(표본 없음)"
        and b10["c_nonzero_render_offset"] == "미검증(증거 요건)"
        and b10["c_f4_decimal_literal"] == "미검증(증거 요건)"
        and b10["unity_f4_row_parses"] == "ok"
        and b10["p1_no_summary"].startswith("rejected(")
        and b10["p2_observer_set_mismatch"].startswith("rejected(")
        and b10["fail_on_1mm_mismatch"] == "FAIL"
        and b10["block7_rejects_merged_csv"] == "rejected(NotImplementedYet)"
    )

    ok = (
        same == 0 and off == 20
        and behind is not None and behind > 20
        and ahead is not None and ahead < -20
        and header_guard_ok
        and separation_ok
        and b10_ok
    )
    db.emit(
        {
            "item": "two_client_view 자체 검증 (SC-85 계열)",
            "verdict": "PASS" if ok else "FAIL",
            "identical_files_mismatches": same,
            "one_mm_offset_mismatches": off,
            "behind_28m_along_track_m": round(behind, 2) if behind is not None else None,
            "ahead_28m_along_track_m": round(ahead, 2) if ahead is not None else None,
            "columns_count": len(COLUMNS),
            "columns": list(COLUMNS),
            "header_guard": header_cases,
            "header_guard_ok": header_guard_ok,
            "verdict_separation": separation,
            "verdict_separation_ok": separation_ok,
            "block10_defenses": b10,
            "block10_defenses_ok": b10_ok,
            "meaning": (
                "1 mm 를 어긋뜨렸을 때 20건 전부 잡히면 정수 비교가 살아 있다는 뜻이고, "
                "뒤/앞 부호가 반대로 나오면 SC-65 의 부호 판정이 동작한다는 뜻이다. "
                "header_guard 의 네 변형이 전부 rejected 이고 exact_match 만 accepted 여야 "
                "C#(ObserverCsv.Header) 한쪽만 고쳤을 때 조용히 지나가지 않는다. "
                "verdict_separation 은 **SC-63 만 FAIL 인 입력에서 SC-61·62 가 PASS 로 인쇄되는가** "
                "를 본다 — legacy_sc63_mismatches 가 20 이라는 것이 '겨냥한 조건이 실제로 "
                "발생했다'의 단언이고, 되돌아가는 입력에서 SC-61 이 FAIL 이라는 것이 "
                "'분리가 무조건 PASS 를 인쇄하는 것이 아니다'의 단언이다. "
                "block10_defenses 는 (a)(b)(c)와 P1·P2 가 각각 걸려야 할 입력에서 걸리는지, "
                "그리고 진짜 불일치가 FAIL 로 나오는지를 같은 실행 안에서 보인다."
            ),
        },
        args.evidence,
    )
    return db.EXIT_OK if ok else db.EXIT_FAIL


def _mismatches(fa: Path, fb: Path, ship: str) -> int:
    a, b = load(fa), load(fb)
    return sum(
        1
        for k in set(a) & set(b)
        if k[1] == ship and (a[k]["p"] != b[k]["p"] or a[k]["v"] != b[k]["v"])
    )


def _behind_mean(fa: Path, fb: Path, ship: str) -> float | None:
    a, b = load(fa), load(fb)
    vals = []
    for k in set(a) & set(b):
        if k[1] != ship:
            continue
        ra, rb = a[k], b[k]
        diff = tuple(rb["p"][i] - ra["p"][i] for i in range(3))
        vlen = math.dist((0, 0, 0), ra["v"])
        if vlen:
            vals.append(-sum(diff[i] * ra["v"][i] for i in range(3)) / vlen / 1000.0)
    return sum(vals) / len(vals) if vals else None


def main() -> int:
    ap = argparse.ArgumentParser(description="두 관측자 스냅샷 대조")
    sub = ap.add_subparsers(dest="cmd", required=True)

    for name, fn in (("compare", cmd_compare), ("s3", cmd_s3)):
        p = sub.add_parser(name)
        p.add_argument("--a", required=True, help="A(예측 관측자)의 CSV")
        p.add_argument("--b", required=True, help="B(보간 관측자)의 CSV")
        p.add_argument("--ship", required=True, help="A 의 ship_id")
        p.add_argument("--speed-mps", type=float, default=140.0)
        p.add_argument("--evidence")
        p.set_defaults(fn=fn)
        if name == "compare":
            # **기본값을 두지 않는다 (계약 SC-62 (iii), architect R15).** 임계는 계약 행에
            # 있으므로 도구에 사본을 두면 한 수에 출처가 둘이 된다 — 계약의 수가 바뀌고 도구가
            # 안 바뀌면 **옛 수로 조용히 초록**이 나온다. 인자를 빠뜨리면 exit 4 가 그 자리에서
            # 알려 준다: **막힌 것보다 안 보이는 것이 나쁘다.**
            p.add_argument("--b-own-tolerance-m", type=float, default=None,
                           help="계약 SC-62 의 임계(현재 0.05). 없으면 미검증(판정 기준 미지정)")
        if name == "s3":
            p.add_argument("--still-speed-mps", type=float, default=1.0)
            p.add_argument("--moving-speed-mps", type=float, default=100.0)
            p.add_argument("--still-tolerance-m", type=float, default=2.0)
            p.add_argument("--behind-m", type=float, default=28.0)
            p.add_argument("--behind-tol-m", type=float, default=12.0)

    r2 = sub.add_parser("raw2", help="SC-63 — 봇 2대 원시 스냅샷 대조 (블록 10)")
    r2.add_argument("--bot-out-dir", required=True,
                    help="봇 하네스 `--out` 디렉터리 (snapshots.csv + summary.json)")
    r2.add_argument("--evidence")
    r2.set_defaults(fn=cmd_raw2)

    st = sub.add_parser("selftest")
    st.add_argument("--evidence")
    st.set_defaults(fn=cmd_selftest)

    args = ap.parse_args()
    return db.main_guard(lambda: args.fn(args))


if __name__ == "__main__":
    sys.exit(main())

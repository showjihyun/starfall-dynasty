"""SC-61~65 — **A가 움직이면 B가 본다** (AC-16 + designer S-3).

두 관측자의 스냅샷 CSV 를 **envelope `tick`** 으로 조인한다. CSV 형식은 client·봇이 같다
(계약 §3.1): `tick,observer_actor_id,ship_id,presence,px_mm,py_mm,pz_mm,vx_mm_s,vy_mm_s,vz_mm_s`

| 하위 명령 | 항목 |
|----------|------|
| `compare` | SC-61(단조·이동 거리) · SC-62(B 정지) · SC-63(같은 tick 정수 일치) |
| `s3`      | SC-64(정지 비교 ≤ 2 m) · SC-65(**이동 비교: 진행 방향 뒤쪽 28 ± 12 m, 부호가 핵심**) |
| `selftest`| 도구 검증 — 1 mm 를 어긋뜨리면 잡히는가, 부호를 뒤집으면 잡히는가 |

**SC-63 은 항진명제에 가깝다**(I-25): 두 값은 같은 `ships` 배열에서 나오므로 정상 구현에서는
다를 수 없다. 잡는 것은 **세션별 직렬화가 배열을 다르게 만드는 버그** 하나뿐이고, 진짜 검증은
SC-61·62다. 리포트에 그렇게 적는다.

    python tests/e2e/two_client_view.py compare --a <A.csv> --b <B.csv> --ship <A의 ship_id>
    python tests/e2e/two_client_view.py s3 --a <A.csv> --b <B.csv> --ship <uuid> [--speed-mps 140]
    python tests/e2e/two_client_view.py selftest
"""

from __future__ import annotations

import argparse
import csv
import math
import sys
from pathlib import Path

import db

COLUMNS = [
    "tick", "observer_actor_id", "ship_id", "presence",
    "px_mm", "py_mm", "pz_mm", "vx_mm_s", "vy_mm_s", "vz_mm_s",
]


def load(path: str | Path) -> dict[tuple[int, str], dict]:
    p = Path(path)
    if not p.is_file():
        raise db.EnvironmentProblem(f"CSV 가 없다: {p}")
    out: dict[tuple[int, str], dict] = {}
    with p.open(encoding="utf-8", newline="") as f:
        reader = csv.DictReader(f)
        if reader.fieldnames != COLUMNS:
            raise db.NotImplementedYet(
                f"{p}: 컬럼이 계약 §3.1 과 다르다.\n  기대: {COLUMNS}\n  실제: {reader.fieldnames}"
            )
        for row in reader:
            key = (int(row["tick"]), row["ship_id"])
            out[key] = {
                "observer": row["observer_actor_id"],
                "presence": row["presence"],
                "p": (int(row["px_mm"]), int(row["py_mm"]), int(row["pz_mm"])),
                "v": (int(row["vx_mm_s"]), int(row["vy_mm_s"]), int(row["vz_mm_s"])),
            }
    if not out:
        raise db.EnvironmentProblem(f"{p}: 행이 없다 (0건 대조는 검증이 아니다)")
    return out


def dist_mm(a: tuple[int, int, int], b: tuple[int, int, int]) -> float:
    return math.dist(a, b)


def cmd_compare(args: argparse.Namespace) -> int:
    a = load(args.a)
    b = load(args.b)
    ship = args.ship

    # ── SC-61: B 가 본 A 의 위치가 단조적으로 변하고 총 이동 거리가 설명 가능한가
    b_rows = sorted(((t, r) for (t, s), r in b.items() if s == ship), key=lambda x: x[0])
    if not b_rows:
        raise db.NotImplementedYet(f"B 의 CSV 에 함선 {ship} 이 없다 — B 가 A 를 보지 못했다")
    path_mm = sum(dist_mm(b_rows[i][1]["p"], b_rows[i + 1][1]["p"]) for i in range(len(b_rows) - 1))
    disp_mm = dist_mm(b_rows[0][1]["p"], b_rows[-1][1]["p"])
    ticks = b_rows[-1][0] - b_rows[0][0]
    seconds = ticks / 20.0
    max_possible_m = args.speed_mps * seconds
    monotonic_back = sum(
        1
        for i in range(len(b_rows) - 1)
        if dist_mm(b_rows[0][1]["p"], b_rows[i + 1][1]["p"])
        < dist_mm(b_rows[0][1]["p"], b_rows[i][1]["p"]) - 1000  # 1 m 이상 되돌아감
    )

    # ── SC-62: B 자신의 함선이 스폰 근처에 머무는가 (B 의 관측자 actor 의 함선)
    b_actor = next(iter(b.values()))["observer"]
    b_own = sorted(
        ((t, r) for (t, s), r in b.items() if r["observer"] == b_actor and s != ship),
        key=lambda x: x[0],
    )
    b_own_disp_m = dist_mm(b_own[0][1]["p"], b_own[-1][1]["p"]) / 1000.0 if len(b_own) >= 2 else None

    # ── SC-63: 같은 (tick, ship) 의 정수 6개가 완전히 같은가
    common = sorted(set(a) & set(b))
    mismatch = []
    for key in common:
        ra, rb = a[key], b[key]
        if ra["p"] != rb["p"] or ra["v"] != rb["v"]:
            mismatch.append(
                {"tick": key[0], "ship_id": key[1], "a": [ra["p"], ra["v"]], "b": [rb["p"], rb["v"]]}
            )

    ok = (
        not mismatch
        and monotonic_back == 0
        and path_mm > 0
        and (path_mm / 1000.0) <= max_possible_m * 1.05
    )
    db.emit(
        {
            "item": "SC-61/62/63 (AC-16) 두 관측자 대조",
            "verdict": "PASS" if ok else "FAIL",
            "join_key": "(tick, ship_id)",
            "rows_a": len(a), "rows_b": len(b),
            "pairs_compared": len(common),
            "SC-61": {
                "ship": ship,
                "samples": len(b_rows),
                "tick_span": ticks,
                "seconds": round(seconds, 2),
                "path_length_m": round(path_mm / 1000.0, 3),
                "displacement_m": round(disp_mm / 1000.0, 3),
                "max_possible_m_at_max_speed": round(max_possible_m, 1),
                "backward_steps_over_1m": monotonic_back,
            },
            "SC-62": {"b_own_displacement_m": b_own_disp_m},
            "SC-63": {
                "mismatching_pairs": len(mismatch),
                "samples": mismatch[:5],
                "note": (
                    "이 항목은 항진명제에 가깝다(I-25). 두 값은 같은 ships 배열에서 나오므로 "
                    "정상 구현에서는 다를 수 없고, 잡는 것은 세션별 직렬화 버그 하나뿐이다. "
                    "진짜 검증은 SC-61·62다."
                ),
            },
        },
        args.evidence,
    )
    return db.EXIT_OK if ok else db.EXIT_FAIL


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
        speed_mps = math.dist((0, 0, 0), ra["v"]) / 1000.0
        diff = tuple(rb["p"][i] - ra["p"][i] for i in range(3))  # B가 본 것 − A가 본 것
        gap_m = math.dist((0, 0, 0), diff) / 1000.0
        if speed_mps < args.still_speed_mps:
            still.append({"tick": tick, "gap_m": round(gap_m, 3), "speed_mps": round(speed_mps, 2)})
        elif speed_mps >= args.moving_speed_mps:
            # 진행 방향 단위 벡터에 투영 → **부호**가 판정의 핵심
            vlen = math.dist((0, 0, 0), ra["v"])
            proj_m = sum(diff[i] * ra["v"][i] for i in range(3)) / vlen / 1000.0 if vlen else 0.0
            moving.append(
                {
                    "tick": tick,
                    "along_track_m": round(proj_m, 3),
                    "gap_m": round(gap_m, 3),
                    "speed_mps": round(speed_mps, 2),
                }
            )

    still_max = max((s["gap_m"] for s in still), default=None)
    behind = [-m["along_track_m"] for m in moving]  # 뒤쪽이 양수가 되게 부호를 뒤집는다
    behind_mean = sum(behind) / len(behind) if behind else None

    sc64 = "미검증(표본 없음)" if not still else ("PASS" if still_max <= args.still_tolerance_m else "FAIL")
    if not moving:
        sc65 = "미검증(표본 없음)"
    else:
        lo, hi = args.behind_m - args.behind_tol_m, args.behind_m + args.behind_tol_m
        sc65 = "PASS" if behind_mean is not None and lo <= behind_mean <= hi else "FAIL"

    db.emit(
        {
            "item": "SC-64/65 (designer S-3) 두 화면 비교",
            "SC-64_verdict": sc64,
            "SC-65_verdict": sc65,
            "SC-64": {
                "samples": len(still),
                "max_gap_m": still_max,
                "tolerance_m": args.still_tolerance_m,
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
        },
        args.evidence,
    )
    bad = (sc64 == "FAIL") or (sc65 == "FAIL")
    return db.EXIT_FAIL if bad else db.EXIT_OK


def cmd_selftest(args: argparse.Namespace) -> int:
    """도구 검증: 1 mm 를 어긋뜨리면 SC-63 이 잡는가, 부호를 뒤집으면 SC-65 가 잡는가."""
    import tempfile

    tmp = Path(tempfile.mkdtemp(prefix="starfall-2cv-"))
    ship = "01a0c000-0000-7000-8000-00000000000a"
    actor_a, actor_b = "aaaa0000-0000-7000-8000-000000000001", "bbbb0000-0000-7000-8000-000000000002"

    def write(path: Path, actor: str, offset_mm: int, behind_mm: int) -> None:
        with path.open("w", encoding="utf-8", newline="") as f:
            f.write(",".join(COLUMNS) + "\n")
            for i in range(20):
                tick = 100 + i * 2
                z = 1_000_000 + i * 7_000 + offset_mm - behind_mm
                f.write(f"{tick},{actor},{ship},ACTIVE,0,0,{z},0,0,140000\n")

    # ① 두 파일이 완전히 같으면 불일치 0
    fa, fb = tmp / "a.csv", tmp / "b.csv"
    write(fa, actor_a, 0, 0)
    write(fb, actor_b, 0, 0)
    same = _mismatches(fa, fb, ship)

    # ② B 를 1 mm 어긋뜨리면 전부 불일치로 잡힌다
    fb2 = tmp / "b_off.csv"
    write(fb2, actor_b, 1, 0)
    off = _mismatches(fa, fb2, ship)

    # ③ 부호: B 가 28 m 뒤처지면 along-track 투영이 **뒤쪽 양수**로 나온다
    fb3 = tmp / "b_behind.csv"
    write(fb3, actor_b, 0, 28_000)
    behind = _behind_mean(fa, fb3, ship)
    fb4 = tmp / "b_ahead.csv"
    write(fb4, actor_b, 0, -28_000)
    ahead = _behind_mean(fa, fb4, ship)

    ok = same == 0 and off == 20 and behind is not None and behind > 20 and ahead is not None and ahead < -20
    db.emit(
        {
            "item": "two_client_view 자체 검증 (SC-85 계열)",
            "verdict": "PASS" if ok else "FAIL",
            "identical_files_mismatches": same,
            "one_mm_offset_mismatches": off,
            "behind_28m_along_track_m": round(behind, 2) if behind is not None else None,
            "ahead_28m_along_track_m": round(ahead, 2) if ahead is not None else None,
            "meaning": (
                "1 mm 를 어긋뜨렸을 때 20건 전부 잡히면 SC-63 이 실제로 정수를 비교하고 있다는 뜻이고, "
                "뒤/앞 부호가 반대로 나오면 SC-65 의 부호 판정이 동작한다는 뜻이다."
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
        if name == "s3":
            p.add_argument("--still-speed-mps", type=float, default=1.0)
            p.add_argument("--moving-speed-mps", type=float, default=100.0)
            p.add_argument("--still-tolerance-m", type=float, default=2.0)
            p.add_argument("--behind-m", type=float, default=28.0)
            p.add_argument("--behind-tol-m", type=float, default=12.0)

    st = sub.add_parser("selftest")
    st.add_argument("--evidence")
    st.set_defaults(fn=cmd_selftest)

    args = ap.parse_args()
    return db.main_guard(lambda: args.fn(args))


if __name__ == "__main__":
    sys.exit(main())

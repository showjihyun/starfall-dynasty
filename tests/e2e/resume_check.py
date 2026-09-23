#!/usr/bin/env python
"""SC-11 (3) — 재개 직후 상태를 **qa 독립 계산**과 대조한다.

입력은 `bots resume` 이 남긴 JSON(T0 = 끊기기 직전 마지막 자기 스냅샷, T1 = 재개 후 첫 자기 스냅샷,
관측자가 본 그 함선의 행)이다. 독립 계산은 `tests/e2e/resume_predict`(client 의 C# 적분기)가 한다 —
서버가 계산한 값을 서버가 확인하면 I-25 위반이다.

두 가지를 따로 보고한다
-----------------------
1. **계약 문구 그대로**: "재개 직후 스냅샷의 (p, v, q, ω_aim, ω_roll) 이 끊기기 직전 스냅샷 상태에
   N tick 적분한 값과 **양자화 정수로 같다**(허용 오차 없음)" → 14개 정수 필드의 완전 일치 여부.
2. **양자화 봉투 안인가**: 서버 상태는 `f64` 이고 양자화는 스냅샷을 만들 때만 일어난다
   (`server/crates/sim/src/world/ship.rs` 주석, ADR-0009 §2). T0 스냅샷은 서버 상태를 반올림한 것이라,
   **스냅샷에서 출발한 계산은 서버 상태에서 출발한 계산과 비트가 같을 수 없다.** 그래서 T0 의 각 정수를
   ±0.5 양자 흔든 시작점들로 같은 적분을 돌려 [min, max] 봉투를 만들고, 관측값이 그 안에 있는지 본다.
   이것은 허용 오차를 **고르는** 것이 아니라 계약의 전제(양자화)에서 **유도**하는 것이다.

**음성 대조(RED)**: 휴면 입력을 일부러 틀리게(`flight_assist=false`) 계산한 예측이 **봉투 밖으로 나가는지**
본다. 나가지 않으면 이 대조는 틀린 모델도 통과시키는 것이다(계약 §7a).

종료 코드: 0 = 봉투 대조 성립 + 음성 대조가 빨간불, 1 = 아니다, 2 = 입력·하네스 문제.
"""
from __future__ import annotations

import argparse
import json
import subprocess
import sys
from pathlib import Path

for _stream in (sys.stdout, sys.stderr):
    try:
        _stream.reconfigure(encoding="utf-8")  # type: ignore[union-attr]
    except (AttributeError, ValueError):
        pass

REPO = Path(__file__).resolve().parents[2]
FIELDS = [
    "position_x_mm", "position_y_mm", "position_z_mm",
    "velocity_x_mm_s", "velocity_y_mm_s", "velocity_z_mm_s",
    "orientation_x_micro", "orientation_y_micro", "orientation_z_micro", "orientation_w_micro",
    "angular_velocity_x_mdeg_s", "angular_velocity_y_mdeg_s", "angular_velocity_z_mdeg_s",
    "angular_velocity_roll_mdeg_s",
]
CSV_FIELDS = ["position_x_mm", "position_y_mm", "position_z_mm",
              "velocity_x_mm_s", "velocity_y_mm_s", "velocity_z_mm_s"]


def own_to_wire(s: dict) -> dict:
    """bots `OwnSample` → 와이어 필드 이름."""
    p, v, q, w = s["position_mm"], s["velocity_mm_s"], s["orientation_micro"], s["angular_velocity_mdeg_s"]
    return {
        "position_x_mm": p[0], "position_y_mm": p[1], "position_z_mm": p[2],
        "velocity_x_mm_s": v[0], "velocity_y_mm_s": v[1], "velocity_z_mm_s": v[2],
        "orientation_x_micro": q[0], "orientation_y_micro": q[1],
        "orientation_z_micro": q[2], "orientation_w_micro": q[3],
        "angular_velocity_x_mdeg_s": w[0], "angular_velocity_y_mdeg_s": w[1],
        "angular_velocity_z_mdeg_s": w[2],
        "angular_velocity_roll_mdeg_s": s["angular_velocity_roll_mdeg_s"],
    }


def run_harness(exe: Path, req: dict, work: Path, name: str) -> dict:
    path = work / f"{name}.request.json"
    path.write_text(json.dumps(req), encoding="utf-8")
    r = subprocess.run([str(exe), str(path)], capture_output=True, text=True, encoding="utf-8")
    if r.returncode != 0:
        raise RuntimeError(f"harness rc={r.returncode}: {r.stderr[:500]}")
    (work / f"{name}.response.json").write_text(r.stdout, encoding="utf-8")
    return json.loads(r.stdout)


def compare_point(obs: dict, center: dict, env: dict, fields: list[str]) -> dict:
    exact = sum(1 for f in fields if obs[f] == center[f])
    outside = [f for f in fields if not (env[f][0] <= obs[f] <= env[f][1])]
    dev = {f: obs[f] - center[f] for f in fields}
    return {"exact_fields": exact, "fields": len(fields), "outside_envelope": outside, "deviation": dev}


def main() -> int:
    ap = argparse.ArgumentParser(description="SC-11 (3): 재개 상태 vs qa 독립 계산(client C# 적분기)")
    ap.add_argument("--resume-json", required=True, help="bots resume 의 --out")
    ap.add_argument("--harness", required=True, help="tests/e2e/resume_predict 빌드 산출 exe")
    ap.add_argument("--ship-class", default=str(REPO / "data/ships/scout-s01.json"))
    ap.add_argument("--star-system", default=str(REPO / "data/world/systems/cradle.json"))
    ap.add_argument("--carry-forward-max-ticks", type=int, required=True,
                    help="data/movement/sync-tuning.json 의 값 — 측정 시점 값을 증거에 남긴다(계약 §0.7)")
    ap.add_argument("--perturb", type=int, default=64, help="봉투를 만들 흔든 시작점 수")
    ap.add_argument("--evidence", required=True)
    args = ap.parse_args()

    doc = json.loads(Path(args.resume_json).read_text(encoding="utf-8"))
    ev = Path(args.evidence)
    ev.mkdir(parents=True, exist_ok=True)
    exe = Path(args.harness)
    if not exe.is_file():
        print(f"미검증(환경): 하네스가 없다 {exe}", file=sys.stderr)
        return 2
    leg1, leg2 = doc["leg1"], doc["leg2"]
    t0, t1 = leg1.get("t0"), leg2.get("t1")
    if not t0 or not t1:
        print("FAIL: T0 또는 T1 스냅샷이 없다 — 재개가 성사되지 않았다", file=sys.stderr)
        return 1
    tick_hz = leg1["session"]["tick_hz"]
    n = t1["tick"] - t0["tick"]
    last = leg1.get("last_input_result") or {}
    last_tick = last.get("tick")
    carry = args.carry_forward_max_ticks
    # 마지막 실제 입력이 last_tick 에 적용됐다면 last+1 .. last+carry 가 이월, 그 뒤가 휴면이다.
    carry_remaining = max(0, (last_tick + carry) - t0["tick"]) if last_tick is not None else None
    same_ship = leg1["controlled_ship_id"] is not None and leg1["controlled_ship_id"] == leg2["controlled_ship_id"]

    base = {
        "ship_class_path": args.ship_class, "star_system_path": args.star_system,
        "tick_hz": tick_hz, "ticks": n, "start": own_to_wire(t0),
    }
    if carry_remaining:
        # 이 시나리오는 settle > 이월 창이라 여기 오지 않아야 한다. 오면 이월 입력을 넣는다.
        base["carry"] = {"ticks": carry_remaining, "payload": {
            "input_seq": 1, "thrust_x_milli": 0, "thrust_y_milli": 0, "thrust_z_milli": 1000,
            "roll_milli": 1000, "aim_x_micro": 0, "aim_y_micro": 0, "aim_z_micro": 0,
            "aim_w_micro": 1_000_000, "brake": False, "flight_assist": False}}
    good = run_harness(exe, {**base, "perturb": {"count": args.perturb, "seed": 7}}, ev, "predict")
    wrong = run_harness(exe, {**base, "dormant_flight_assist": False}, ev, "predict-wrong-model")

    at_t1 = compare_point(own_to_wire(t1), good["states"][n - 1], good["envelope"][n - 1], FIELDS)
    wrong_t1 = compare_point(own_to_wire(t1), wrong["states"][n - 1], good["envelope"][n - 1], FIELDS)
    wrong_center_outside = [f for f in FIELDS
                            if not (good["envelope"][n - 1][f][0] <= wrong["states"][n - 1][f]
                                    <= good["envelope"][n - 1][f][1])]

    # 관측자 행: T0 < tick <= T1 인 그 함선의 p, v.
    rows = []
    for line in doc.get("observer_rows", []):
        c = line.split(",")
        tick = int(c[0])
        if t0["tick"] < tick <= t1["tick"]:
            rows.append({"tick": tick, "presence": c[3], **dict(zip(CSV_FIELDS, map(int, c[4:10])))})
    obs_checked = 0
    obs_outside = 0
    obs_wrong_outside = 0
    max_dev = {f: 0 for f in CSV_FIELDS}
    presences: dict[str, int] = {}
    for r in rows:
        k = r["tick"] - t0["tick"] - 1
        cmp = compare_point(r, good["states"][k], good["envelope"][k], CSV_FIELDS)
        obs_checked += 1
        obs_outside += 1 if cmp["outside_envelope"] else 0
        for f in CSV_FIELDS:
            max_dev[f] = max(max_dev[f], abs(cmp["deviation"][f]))
        wrong_pt = wrong["states"][k]
        if any(not (good["envelope"][k][f][0] <= wrong_pt[f] <= good["envelope"][k][f][1]) for f in CSV_FIELDS):
            obs_wrong_outside += 1
        presences[r["presence"]] = presences.get(r["presence"], 0) + 1

    seq1 = leg2.get("seq1_result") or {}
    result = {
        "ship_id_same": same_ship, "ship_id": leg1["controlled_ship_id"],
        "t0_tick": t0["tick"], "t1_tick": t1["tick"], "n_ticks": n, "tick_hz": tick_hz,
        "last_input_tick": last_tick, "carry_forward_max_ticks": carry, "carry_remaining_at_t0": carry_remaining,
        "t1_contract_exact_fields": f"{at_t1['exact_fields']}/{at_t1['fields']}",
        "t1_contract_exact": at_t1["exact_fields"] == at_t1["fields"],
        "t1_outside_envelope": at_t1["outside_envelope"],
        "t1_deviation_from_center": at_t1["deviation"],
        "envelope_width_at_t1": {f: good["envelope"][n - 1][f][1] - good["envelope"][n - 1][f][0] for f in FIELDS},
        "observer_points_checked": obs_checked, "observer_points_outside_envelope": obs_outside,
        "observer_max_abs_deviation": max_dev, "observer_presence_counts": presences,
        "negative_control": {
            "model": "dormant flight_assist=false (틀린 모델)",
            "t1_fields_outside_envelope": len(wrong_center_outside),
            "observer_points_outside_envelope": obs_wrong_outside,
            "t1_exact_fields_vs_observed": f"{wrong_t1['exact_fields']}/{wrong_t1['fields']}",
        },
        "e2_seq1_status": seq1.get("status"), "e2_ack_last": leg2.get("ack_last"),
    }
    envelope_ok = (not at_t1["outside_envelope"]) and obs_checked > 0 and obs_outside == 0
    red_ok = len(wrong_center_outside) > 0 or obs_wrong_outside > 0
    result["envelope_consistent"] = envelope_ok
    result["negative_control_red"] = red_ok
    (ev / "resume_check.json").write_text(json.dumps(result, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps(result, ensure_ascii=False, indent=2))
    return 0 if (envelope_ok and red_ok and same_ship) else 1


if __name__ == "__main__":
    sys.exit(main())

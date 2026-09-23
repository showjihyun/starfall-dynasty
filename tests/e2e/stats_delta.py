#!/usr/bin/env python
"""SC-33 — `/debug/stats` 전후 델타를 **라벨 전부** 펼쳐 본다.

계약 SC-33: "존재만으로 PASS 주지 않는다 — 델타를 확인한다." 그리고 라운드 3 §4.5 가 보인 것처럼
**키가 있고 값이 0 인 라벨은 "아무 일도 없었다"와 "세지 않는다"를 구분하지 못한다.** 그래서 이 도구는
봇이 받은 `COMMAND_RESULT` 거부 수(독립 출처 — I-25)를 `--expect LABEL=N` 으로 받아 **라벨별로 일치**를
단언하고, 기대가 0 보다 큰 라벨이 서버에서 0 이면 그 자체를 실패로 적는다(계약 §7a).

    python tests/e2e/stats_delta.py snap --out before.json
    ... 부하/치트 ...
    python tests/e2e/stats_delta.py snap --out after.json
    python tests/e2e/stats_delta.py diff before.json after.json --expect STALE_INPUT=40 --expect RATE_LIMITED=6

종료 코드: 0 = 전부 성립, 1 = 불일치, 2 = 서버에 닿지 못함.
"""
from __future__ import annotations

import argparse
import json
import sys
import urllib.error
import urllib.request
from pathlib import Path

for _stream in (sys.stdout, sys.stderr):
    try:
        _stream.reconfigure(encoding="utf-8")  # type: ignore[union-attr]
    except (AttributeError, ValueError):
        pass

NEW_SCALARS = [
    "snapshots_sent_total", "snapshot_bytes_total", "send_queue_bytes", "send_queue_bytes_max",
    "input_superseded_total", "input_carried_forward_total", "aim_degenerate_total",
]
GAUGES = ["ships_active", "ships_lingering"]
REJECT_LABELS_WANT = 8
MESSAGE_LABELS_WANT = 4


def labels(doc: dict, key: str) -> dict[str, int]:
    return {row["label"]: int(row["count"]) for row in doc.get(key, [])}


def cmd_snap(args) -> int:
    try:
        with urllib.request.urlopen(f"http://{args.addr}/debug/stats", timeout=5) as r:
            body = r.read().decode("utf-8")
    except (urllib.error.URLError, OSError, TimeoutError) as e:
        print(f"미검증(환경): /debug/stats 에 닿지 못했다: {e}", file=sys.stderr)
        return 2
    Path(args.out).write_text(body, encoding="utf-8")
    d = json.loads(body)
    print(f"snap tick={d.get('tick')} start_tick={d.get('start_tick')} -> {args.out}")
    return 0


def cmd_diff(args) -> int:
    a = json.loads(Path(args.before).read_text(encoding="utf-8"))
    b = json.loads(Path(args.after).read_text(encoding="utf-8"))
    fails: list[str] = []
    out: dict = {"before_tick": a.get("tick"), "after_tick": b.get("tick"),
                 "start_tick": [a.get("start_tick"), b.get("start_tick")]}
    if a.get("start_tick") != b.get("start_tick"):
        fails.append("start_tick 이 다르다 — 두 스냅이 다른 서버 인스턴스다")

    scal = {}
    for k in NEW_SCALARS + GAUGES:
        if k not in b:
            fails.append(f"키 없음: {k}")
            continue
        scal[k] = {"before": a.get(k), "after": b[k], "delta": b[k] - (a.get(k) or 0)}
    out["scalars"] = scal

    arrays = {}
    for key, want in (("commands_rejected_total", REJECT_LABELS_WANT),
                      ("messages_enqueued_total", MESSAGE_LABELS_WANT),
                      ("messages_written_total", MESSAGE_LABELS_WANT)):
        la, lb = labels(a, key), labels(b, key)
        if len(lb) != want:
            fails.append(f"{key}: 라벨 {len(lb)}개 (계약 {want})")
        arrays[key] = {lab: {"after": lb[lab], "delta": lb[lab] - la.get(lab, 0)} for lab in lb}
    out["labels"] = arrays

    snap_delta = scal.get("snapshots_sent_total", {}).get("delta")
    ws_delta = arrays.get("messages_written_total", {}).get("WORLD_SNAPSHOT", {}).get("delta")
    out["snapshots_sent_vs_written_world_snapshot"] = [snap_delta, ws_delta]
    if snap_delta != ws_delta:
        fails.append(f"snapshots_sent_total 델타 {snap_delta} ≠ messages_written_total{{WORLD_SNAPSHOT}} 델타 {ws_delta}")
    if b.get("snapshots_sent_total") != labels(b, "messages_written_total").get("WORLD_SNAPSHOT"):
        fails.append("snapshots_sent_total 절대값 ≠ messages_written_total{WORLD_SNAPSHOT} 절대값")

    expect = {}
    for item in args.expect or []:
        lab, _, n = item.partition("=")
        expect[lab] = int(n)
    rej = arrays.get("commands_rejected_total", {})
    checks = []
    for lab, n in expect.items():
        got = rej.get(lab, {}).get("delta")
        ok = got == n
        checks.append({"label": lab, "bot_observed": n, "server_delta": got, "match": ok})
        if not ok:
            fails.append(f"commands_rejected_total{{{lab}}} 델타 {got} ≠ 봇이 받은 거부 {n}")
        if n == 0:
            # 기대 0 은 대조가 아니다 — 경로가 탔다는 증거가 없다(§7a).
            checks[-1]["note"] = "기대 0 — 이 라벨의 경로는 이 실행에서 타지 않았다(대조 아님)"
    # 기대에 없는 라벨이 움직였으면 드러낸다(봇이 못 본 거부 = 1:1 위반 후보).
    for lab, v in rej.items():
        if lab not in expect and v["delta"] != 0 and args.expect:
            fails.append(f"기대에 없는 라벨이 움직였다: {lab} +{v['delta']}")
    out["reject_checks"] = checks
    out["unchanged_new_scalars"] = [k for k in NEW_SCALARS if scal.get(k, {}).get("delta") == 0
                                    and k not in ("send_queue_bytes",)]
    out["fails"] = fails
    text = json.dumps(out, ensure_ascii=False, indent=2)
    if args.out:
        Path(args.out).write_text(text, encoding="utf-8")
    print(text)
    return 0 if not fails else 1


def main() -> int:
    ap = argparse.ArgumentParser(description="SC-33: /debug/stats 전후 델타 (라벨 전부)")
    sub = ap.add_subparsers(dest="cmd", required=True)
    s = sub.add_parser("snap")
    s.add_argument("--addr", default="127.0.0.1:8080")
    s.add_argument("--out", required=True)
    d = sub.add_parser("diff")
    d.add_argument("before")
    d.add_argument("after")
    d.add_argument("--expect", action="append", help="LABEL=N — 봇이 받은 그 사유의 거부 수")
    d.add_argument("--out")
    args = ap.parse_args()
    return cmd_snap(args) if args.cmd == "snap" else cmd_diff(args)


if __name__ == "__main__":
    sys.exit(main())

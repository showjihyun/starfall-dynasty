"""SC-56 / SC-30 / SC-31 — 부하 전체(A+B+C+Unity)의 3자 대조.

`three_way.py` 는 단계 하나를 본다. 부하를 A→B→C 로 이어 돌리면 서버 카운터는 누적이므로
**세 단계를 합쳐** 봇 관측과 맞춰야 한다. 그 합산을 여기서 한다.

세 출처:
  1. 봇 파일   `{a,b,c}/summary.json` (우리가 보내고 받은 것)
  2. 서버 메트릭 `/debug/stats` 의 (after − before) 델타
  3. DB 행     correlation 집합

    python tests/e2e/three_way_load.py --load-dir <evidence/load> --unity-corr <file> --evidence <out.json>
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

import db
from poll_until import DEFAULT_STATS, fetch_stats
from three_way import metric


def label_sum(stats: dict, key: str) -> int | None:
    v = stats.get(key)
    if isinstance(v, list):
        return sum(int(r.get("count", 0)) for r in v if isinstance(r, dict))
    return v if isinstance(v, int) else None


def run(args: argparse.Namespace) -> int:
    load = Path(args.load_dir)
    phases = {}
    for name in ("a", "b", "c"):
        p = load / name / "summary.json"
        if p.is_file():
            phases[name] = json.loads(p.read_text(encoding="utf-8"))
    if not phases:
        raise db.EnvironmentProblem(f"{load} 아래에 단계 요약이 없다")

    bots = {
        "sessions": sum(x["gates"]["sessions_ready"] for x in phases.values()),
        "sent": sum(x["aggregate"]["sent_total"] for x in phases.values()),
        "results": sum(x["aggregate"]["results_total"] for x in phases.values()),
        "replies": sum(x["aggregate"]["replies_total"] for x in phases.values()),
        "accepted": sum(x["aggregate"]["accepted"] for x in phases.values()),
        "rejected": sum(x["aggregate"]["rejected"] for x in phases.values()),
        "missing_results": sum(x["aggregate"]["missing_results"] for x in phases.values()),
        "order_violations": sum(x["aggregate"]["order_violations"] for x in phases.values()),
        "tick_mismatches": sum(x["aggregate"]["tick_mismatches"] for x in phases.values()),
        "server_initiated_closes": sum(x["gates"]["server_initiated_closes"] for x in phases.values()),
    }
    unity_sessions = 0
    corr_files = [load / name / "correlations.txt" for name in phases]
    if args.unity_corr and Path(args.unity_corr).is_file():
        corr_files.append(Path(args.unity_corr))
        unity_sessions = len(
            [x for x in Path(args.unity_corr).read_text(encoding="utf-8").splitlines() if x.strip()]
        )

    before = json.loads((load / "stats-before.json").read_text(encoding="utf-8"))
    after = fetch_stats(args.stats)

    def delta(key: str, sub: str | None = None):
        a = metric(after, key, sub)
        b = metric(before, key, sub)
        return a - b if isinstance(a, int) and isinstance(b, int) else None

    def delta_all(key: str):
        a, b = label_sum(after, key), label_sum(before, key)
        return a - b if isinstance(a, int) and isinstance(b, int) else None

    # DB: 모든 단계의 correlation 합집합
    corr, seen = [], set()
    for f in corr_files:
        for line in f.read_text(encoding="utf-8").splitlines():
            v = line.strip()
            if v and v not in seen:
                seen.add(v)
                corr.append(v)
    arr = db.uuid_array_literal(corr)
    counts = dict(
        (r[0], int(r[1]))
        for r in db.psql_rows(
            "select event_type, count(*) from domain_events "
            f"where correlation_id = any('{arr}'::uuid[]) group by 1;"
        )
    )

    expected_sessions = bots["sessions"] + unity_sessions
    rows = [
        {"quantity": "세션 수(OPENED)", "bots": expected_sessions,
         "server_metric": delta("sessions_opened_total"), "db": counts.get("SESSION_OPENED", 0)},
        {"quantity": "세션 종료 수(CLOSED)", "bots": expected_sessions,
         "server_metric": delta("sessions_closed_total"), "db": counts.get("SESSION_CLOSED", 0)},
        {"quantity": "명령 수", "bots": bots["sent"],
         "server_metric": delta("commands_received_total"), "db": None},
        {"quantity": "COMMAND_RESULT 수", "bots": bots["results"],
         "server_metric": delta("messages_enqueued_total", "COMMAND_RESULT"), "db": None},
        {"quantity": "PING_REPLY 수", "bots": bots["replies"],
         "server_metric": delta("messages_enqueued_total", "PING_REPLY"), "db": None},
        {"quantity": "SESSION_READY 수", "bots": expected_sessions,
         "server_metric": delta("messages_enqueued_total", "SESSION_READY"), "db": None},
        {"quantity": "거부 수", "bots": bots["rejected"],
         "server_metric": delta_all("commands_rejected_total"), "db": None},
    ]
    mismatches = [
        r["quantity"] for r in rows
        if len({v for v in (r["bots"], r["server_metric"], r["db"]) if isinstance(v, int)}) > 1
    ]

    # SC-31: 회계 항등식 (정지 시점)
    enq, wr = label_sum(after, "messages_enqueued_total"), label_sum(after, "messages_written_total")
    dropped, depth = after.get("messages_dropped_total"), after.get("send_queue_depth")
    residual = None
    if all(isinstance(v, int) for v in (enq, wr, dropped, depth)):
        residual = enq - wr - dropped - depth
    recv = after.get("commands_received_total")
    enq_cr = metric(after, "messages_enqueued_total", "COMMAND_RESULT")
    internal = recv - enq_cr if isinstance(recv, int) and isinstance(enq_cr, int) else None

    # SC-30: 정지 시점 ws_connections
    ws, live = after.get("ws_connections"), after.get("live_connections")
    opened_t, closed_t = after.get("sessions_opened_total"), after.get("sessions_closed_total")
    sc30 = {
        "ws_connections": ws, "live_connections": live,
        "sessions_opened_total": opened_t, "sessions_closed_total": closed_t,
        "opened_minus_closed": (opened_t - closed_t) if isinstance(opened_t, int) and isinstance(closed_t, int) else None,
        "db_open_in_set": counts.get("SESSION_OPENED", 0) - counts.get("SESSION_CLOSED", 0),
    }
    sc30["holds"] = ws == sc30["opened_minus_closed"] == sc30["db_open_in_set"]

    verdict = "PASS" if not mismatches and bots["missing_results"] == 0 else "FAIL"
    db.emit(
        {
            "item": "계약 외 검사 — 3자 대조, 부하 전체(A+B+C+Unity)",
            "label_history": "18차까지 라벨이 p0-02 번호였다(계약 §7b 규칙 7)",
            "verdict": verdict,
            "phases": sorted(phases),
            "correlations_checked": len(corr),
            "bots_totals": bots,
            "table": rows,
            "mismatching_quantities": mismatches,
            "SC-30": sc30,
            "SC-31": {
                "commands_received - enqueued{COMMAND_RESULT}": internal,
                "messages_enqueued_all": enq,
                "messages_written_all": wr,
                "messages_dropped_total": dropped,
                "send_queue_depth": depth,
                "residual(enq - wr - dropped - depth)": residual,
            },
            "persist": {
                "persist_backlog": after.get("persist_backlog"),
                "persist_backlog_limit": after.get("persist_backlog_limit"),
                "domain_events_persisted_total": after.get("domain_events_persisted_total"),
                "domain_events_persist_failed_total": after.get("domain_events_persist_failed_total"),
            },
            "tick": {
                "start_tick": after.get("start_tick"), "tick": after.get("tick"),
                "tick_total": after.get("tick_total"),
                "identity_holds": after.get("tick_total") == after.get("tick") - after.get("start_tick") + 1,
                "tick_overrun_total": after.get("tick_overrun_total"),
                "tick_overrun_threshold_us": after.get("tick_overrun_threshold_us"),
                "tick_body_us": after.get("tick_body_us"),
                "tick_lag_seconds": after.get("tick_lag_seconds"),
                "command_queue_depth_max": after.get("command_queue_depth_max"),
                "send_queue_depth_max": after.get("send_queue_depth_max"),
            },
        },
        args.evidence,
    )
    return db.EXIT_OK if verdict == "PASS" else db.EXIT_FAIL


def main() -> int:
    ap = argparse.ArgumentParser(description="부하 전체 3자 대조")
    ap.add_argument("--load-dir", required=True)
    ap.add_argument("--unity-corr")
    ap.add_argument("--stats", default=DEFAULT_STATS)
    ap.add_argument("--evidence")
    args = ap.parse_args()
    return db.main_guard(lambda: run(args))


if __name__ == "__main__":
    sys.exit(main())

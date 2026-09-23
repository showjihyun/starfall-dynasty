"""SC-70~73 — 스냅샷 손실·대역폭 (AC-18 b~e). **두 출처가 독립일 때만 검증이다**(I-25).

| 출처 | 무엇 |
|------|------|
| 봇 `summary.json` | `snapshots.snapshots_received`, `snapshot_bytes_received` — 소켓에서 **직접** 잰 값 |
| `/debug/stats` 델타 | `snapshots_sent_total`, `snapshot_bytes_total` — 서버가 **소켓에 쓴 뒤** 센 값(`ws.rs:278`) |

**무엇을 쟀는지 반드시 적는다**: 서버는 **WebSocket 페이로드 바이트**를 센다. 봇이 TCP 바이트를
재면 프레임 헤더가 더해진다(15 KiB 에서 4 B, 0.03 % 미만). 봇은 **페이로드 길이**를 센다.

    python tests/e2e/bandwidth.py --bots-out <dir> --stats-before <before.json> [--evidence <p>.json]
    python tests/e2e/bandwidth.py --selftest
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

import db
from poll_until import DEFAULT_STATS, fetch_stats
from three_way import metric

# ADR-0011 §2 산출 (fixture 실측 바이트 기반)
ADR_SHIPSTATE_B = 523
ADR_SNAPSHOT_31_B = 16_566
ADR_PER_SESSION_KIB_S = 161.8
ADR_TOTAL_MBIT_S = 42.5
EGRESS_BUDGET_KIB_S = 192  # data/movement/sync-tuning.json


def compare(bot_recv: int, server_sent: int, label: str) -> dict:
    delta = server_sent - bot_recv
    pct = (abs(delta) / server_sent * 100.0) if server_sent else None
    return {
        "quantity": label,
        "bots": bot_recv,
        "server_metric": server_sent,
        "delta(server-bots)": delta,
        "delta_pct": round(pct, 3) if pct is not None else None,
    }


def run(args: argparse.Namespace) -> int:
    out = Path(args.bots_out)
    summary = json.loads((out / "summary.json").read_text(encoding="utf-8"))
    snap = summary.get("snapshots", {})
    bot_count = int(snap.get("snapshots_received", 0))
    bot_bytes = int(snap.get("snapshot_bytes_received", 0))
    sessions = int(summary.get("gates", {}).get("sessions_ready", 0))
    duration_s = float(summary.get("duration_secs", 0)) or None

    after = fetch_stats(args.stats)
    before = json.loads(Path(args.stats_before).read_text(encoding="utf-8")) if args.stats_before else {}

    def delta(key: str, sub: str | None = None):
        a, b = metric(after, key, sub), metric(before, key, sub) if before else None
        return a - b if isinstance(a, int) and isinstance(b, int) else a

    sent = delta("snapshots_sent_total")
    sent_bytes = delta("snapshot_bytes_total")
    written_label = delta("messages_written_total", "WORLD_SNAPSHOT")

    rows = [
        compare(bot_count, sent if isinstance(sent, int) else 0, "스냅샷 건수"),
        compare(bot_bytes, sent_bytes if isinstance(sent_bytes, int) else 0, "스냅샷 바이트"),
    ]

    # 세션당 대역폭 (봇 실측 기준)
    per_session_kib_s = None
    total_mbit_s = None
    if duration_s and sessions:
        per_session_kib_s = bot_bytes / sessions / duration_s / 1024.0
        total_mbit_s = sent_bytes * 8 / duration_s / 1e6 if isinstance(sent_bytes, int) else None

    adr_gap_pct = (
        abs(per_session_kib_s - ADR_PER_SESSION_KIB_S) / ADR_PER_SESSION_KIB_S * 100.0
        if per_session_kib_s
        else None
    )

    loss_ok = isinstance(sent, int) and sent == bot_count
    budget_ok = per_session_kib_s is None or per_session_kib_s <= EGRESS_BUDGET_KIB_S
    slow_consumer = args.slow_consumer_closes

    db.emit(
        {
            "item": "SC-70~73 (AC-18 b~e) 스냅샷 손실·대역폭",
            "verdict": "PASS" if loss_ok and budget_ok and slow_consumer == 0 else "FAIL",
            "measured_what": "봇 = WebSocket 페이로드 길이 / 서버 = 소켓 기록 후 페이로드 바이트 (같은 것을 잰다)",
            "sessions": sessions,
            "duration_s": duration_s,
            "table": rows,
            "SC-70_snapshot_loss_zero": loss_ok,
            "cross_check_written_label": written_label,
            "SC-71_bandwidth": {
                "per_session_KiB_s": round(per_session_kib_s, 2) if per_session_kib_s else None,
                "total_Mbit_s": round(total_mbit_s, 2) if total_mbit_s else None,
                "adr_per_session_KiB_s": ADR_PER_SESSION_KIB_S,
                "adr_total_Mbit_s": ADR_TOTAL_MBIT_S,
                "gap_pct_vs_adr": round(adr_gap_pct, 2) if adr_gap_pct is not None else None,
                "action_if_over_20pct": "ADR-0011 §2 의 표를 고친다(계약 SC-71)",
            },
            "SC-72_budget": {
                "egress_budget_KiB_s_per_session": EGRESS_BUDGET_KIB_S,
                "within_budget": budget_ok,
            },
            "SC-73_queue": {
                "send_queue_bytes": after.get("send_queue_bytes"),
                "send_queue_bytes_max": after.get("send_queue_bytes_max"),
                "slow_consumer_closes": slow_consumer,
                "note": "정상 부하에서 SLOW_CONSUMER 는 0이어야 한다. 0이 아니면 용량이 아니라 소비자가 문제다",
            },
            "observed_ship_bytes_hint": {
                "adr_ShipState_B": ADR_SHIPSTATE_B,
                "adr_31ships_snapshot_B": ADR_SNAPSHOT_31_B,
                "measured_mean_snapshot_B": round(bot_bytes / bot_count) if bot_count else None,
                "max_ships_seen": snap.get("max_ships_seen"),
            },
        },
        args.evidence,
    )
    return db.EXIT_OK if loss_ok and budget_ok and slow_consumer == 0 else db.EXIT_FAIL


def selftest(args: argparse.Namespace) -> int:
    """계측 산수가 맞는가 — 알려진 값으로 확인한다."""
    # 31 세션이 10초 동안 10 Hz 로 16,566 B 스냅샷을 받는다.
    sessions, duration, hz = 31, 10.0, 10
    bot_bytes = ADR_SNAPSHOT_31_B * hz * sessions * int(duration)
    per = bot_bytes / sessions / duration / 1024.0
    total = bot_bytes * 8 / duration / 1e6
    # 세션당 값은 ADR 과 정확히 같아야 한다. 합계는 ADR 이 COMMAND_RESULT 등을 포함해 조금 크다.
    ok = abs(per - ADR_PER_SESSION_KIB_S) < 0.5 and 38.0 <= total <= ADR_TOTAL_MBIT_S + 0.5
    db.emit(
        {
            "item": "bandwidth 자체 검증",
            "verdict": "PASS" if ok else "FAIL",
            "synthetic": "31세션 × 10 Hz × 10초 × 16,566 B",
            "per_session_KiB_s": round(per, 2),
            "adr_per_session_KiB_s": ADR_PER_SESSION_KIB_S,
            "total_Mbit_s": round(total, 2),
            "adr_total_Mbit_s": ADR_TOTAL_MBIT_S,
            "meaning": "ADR-0011 §2 의 산출을 같은 입력으로 재현한다 — 산수가 어긋나면 실측 해석도 어긋난다",
        },
        args.evidence,
    )
    return db.EXIT_OK if ok else db.EXIT_FAIL


def main() -> int:
    ap = argparse.ArgumentParser(description="스냅샷 손실·대역폭 대조")
    ap.add_argument("--bots-out")
    ap.add_argument("--stats", default=DEFAULT_STATS)
    ap.add_argument("--stats-before")
    ap.add_argument("--slow-consumer-closes", type=int, default=0)
    ap.add_argument("--selftest", action="store_true")
    ap.add_argument("--evidence")
    args = ap.parse_args()
    if args.selftest:
        return db.main_guard(lambda: selftest(args))
    if not args.bots_out:
        ap.error("--bots-out 또는 --selftest 가 필요하다")
    return db.main_guard(lambda: run(args))


if __name__ == "__main__":
    sys.exit(main())

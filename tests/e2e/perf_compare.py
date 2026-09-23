"""SC-75 · M-1~M-6 — p0-02 기준선 대비 **회귀 비교** (AC-19).

**판정하는 것은 하나뿐이다: tick 초과 비율 ≤ 0.5 %.** 나머지는 기록이다.
게임 로직과 31벌 브로드캐스트가 들어갔으므로 **tick 본문 소요가 늘어나는 것은 회귀가 아니라 예상**이다.

**`tick_body_us` 를 p0-02 와 직접 비교하지 않는다**(계약 §0.4) — 스냅샷이 2 tick 마다라 본문
소요가 **이봉분포**가 되어 p50 은 낮은 쪽만, p99 는 높은 쪽만 본다. `snapshot_build_us` 를
분리해 "스냅샷 조립 X µs / 나머지 Y µs" 로 적는다.

    python tests/e2e/perf_compare.py --stats-before <before.json> [--bots-out <dir>] [--evidence <p>.json]
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

import db
from poll_until import DEFAULT_STATS, fetch_stats

# p0-02 라운드 1 실측 (04_qa_report_r1.md)
BASELINE = {
    "tick_overrun_ratio_pct": 0.000,
    "tick_body_max_ms": 21.65,
    "tick_body_p50_le_us": 100,
    "tick_body_p99_le_us": 100,
    "rtt_p99_ms": 50.373,
    "rtt_p50_ms": 28.206,
    "rss_mb": 16.6,
    "command_queue_depth_max": 0,
    "send_queue_depth_max": 141,
    "tick_lag_seconds": -0.044,
}
OVERRUN_GATE_PCT = 0.5
WINDOWS_TIMER_FLOOR = "+0.7 %/분 (전용 OS 스레드 기준, ADR-0006 §2.1)"


def hist_summary(h) -> dict | None:
    if not isinstance(h, dict):
        return None
    return {
        "count": h.get("count"),
        "max_us": h.get("max_us"),
        "p50_le_us": h.get("p50_le_us"),
        "p90_le_us": h.get("p90_le_us"),
        "p99_le_us": h.get("p99_le_us"),
        "mean_us": (
            round(h["sum_us"] / h["count"], 1)
            if isinstance(h.get("sum_us"), int) and h.get("count")
            else None
        ),
    }


def run(args: argparse.Namespace) -> int:
    after = fetch_stats(args.stats)
    before = json.loads(Path(args.stats_before).read_text(encoding="utf-8")) if args.stats_before else {}

    def d(key: str):
        a, b = after.get(key), before.get(key)
        return a - b if isinstance(a, int) and isinstance(b, int) else a

    tick_total = d("tick_total")
    overrun = d("tick_overrun_total")
    ratio = (overrun / tick_total * 100.0) if isinstance(tick_total, int) and tick_total else None
    gate_ok = ratio is not None and ratio <= OVERRUN_GATE_PCT

    body = hist_summary(after.get("tick_body_us"))
    snap_build = hist_summary(after.get("snapshot_build_us"))
    rtt = None
    if args.bots_out:
        p = Path(args.bots_out) / "summary.json"
        if p.is_file():
            rtt = json.loads(p.read_text(encoding="utf-8")).get("aggregate", {}).get("rtt")

    rows = [
        {"metric": "tick 초과 비율 (%)", "p0-02": BASELINE["tick_overrun_ratio_pct"],
         "p1-01": round(ratio, 4) if ratio is not None else None, "판정": "**게이트 ≤ 0.5 %**"},
        {"metric": "tick 본문 max (ms)", "p0-02": BASELINE["tick_body_max_ms"],
         "p1-01": round(body["max_us"] / 1000.0, 3) if body and body.get("max_us") else None,
         "판정": "기록 — 직접 비교하지 않는다(이봉분포)"},
        {"metric": "tick 본문 p50 (≤µs)", "p0-02": BASELINE["tick_body_p50_le_us"],
         "p1-01": body.get("p50_le_us") if body else None, "판정": "기록"},
        {"metric": "tick 본문 p99 (≤µs)", "p0-02": BASELINE["tick_body_p99_le_us"],
         "p1-01": body.get("p99_le_us") if body else None, "판정": "기록"},
        {"metric": "**snapshot_build p99 (≤µs)**", "p0-02": "없음(스냅샷 없음)",
         "p1-01": snap_build.get("p99_le_us") if snap_build else None,
         "판정": "기록 — 본문에서 분리해 적는다"},
        {"metric": "왕복 p50 (ms)", "p0-02": BASELINE["rtt_p50_ms"],
         "p1-01": rtt.get("p50_ms") if rtt else None, "판정": "기록"},
        {"metric": "왕복 p99 (ms)", "p0-02": BASELINE["rtt_p99_ms"],
         "p1-01": rtt.get("p99_ms") if rtt else None, "판정": "기록"},
        {"metric": "command_queue_depth_max", "p0-02": BASELINE["command_queue_depth_max"],
         "p1-01": after.get("command_queue_depth_max"), "판정": "기록"},
        {"metric": "send_queue_depth_max", "p0-02": BASELINE["send_queue_depth_max"],
         "p1-01": after.get("send_queue_depth_max"), "판정": "기록 — 용량이 256→64로 줄었다"},
        {"metric": "send_queue_bytes_max", "p0-02": "없음", "p1-01": after.get("send_queue_bytes_max"),
         "판정": "기록(신규)"},
        {"metric": "tick_lag_seconds", "p0-02": BASELINE["tick_lag_seconds"],
         "p1-01": after.get("tick_lag_seconds"), "판정": f"기록 — 바닥값 {WINDOWS_TIMER_FLOOR}"},
        {"metric": "서버 RSS (MB)", "p0-02": BASELINE["rss_mb"], "p1-01": args.rss_mb,
         "판정": "기록"},
    ]

    db.emit(
        {
            "item": "SC-75 (AC-19) 성능 회귀 — **판정은 tick 초과 비율 하나뿐**",
            "verdict": "PASS" if gate_ok else ("FAIL" if ratio is not None else "미검증(메트릭 없음)"),
            "gate": {
                "tick_overrun_ratio_pct": round(ratio, 4) if ratio is not None else None,
                "threshold_pct": OVERRUN_GATE_PCT,
                "tick_total": tick_total,
                "tick_overrun_total": overrun,
            },
            "table": rows,
            "notes": [
                "tick 본문 소요 증가는 회귀가 아니라 예상이다(게임 로직 + 31벌 브로드캐스트).",
                "tick_body_us 는 스냅샷이 2 tick 마다라 이봉분포다 — p0-02 와 직접 비교하지 않는다.",
                "눈에 띄게 나빠진 항목에는 원인 가설을 리포트에 적는다.",
                "측정 환경(Unity Editor 상태·동시 컨테이너 수·빌드 프로필)을 반드시 함께 적는다.",
            ],
            "persist": {
                "domain_events_persisted_total": after.get("domain_events_persisted_total"),
                "domain_events_persist_failed_total": after.get("domain_events_persist_failed_total"),
                "persist_backlog": after.get("persist_backlog"),
            },
            "new_path_counters": {
                k: after.get(k)
                for k in (
                    "input_superseded_total",
                    "input_carried_forward_total",
                    "aim_degenerate_total",
                    "ships_active",
                    "ships_lingering",
                    "snapshots_sent_total",
                )
            },
        },
        args.evidence,
    )
    return db.EXIT_OK if gate_ok else db.EXIT_FAIL


def main() -> int:
    ap = argparse.ArgumentParser(description="p0-02 기준선 대비 성능 회귀")
    ap.add_argument("--stats", default=DEFAULT_STATS)
    ap.add_argument("--stats-before")
    ap.add_argument("--bots-out")
    ap.add_argument("--rss-mb", type=float)
    ap.add_argument("--evidence")
    args = ap.parse_args()
    return db.main_guard(lambda: run(args))


if __name__ == "__main__":
    sys.exit(main())

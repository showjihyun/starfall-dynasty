"""SC-56 (AC-15e / AC-9c) — **3자 대조**: 봇 관측 / 서버 메트릭 / DB 행.

세 값이 같은 증감 지점에서 나오면 그 등식은 코드가 어떻게 틀려도 참이다(I-25). 그래서
출처가 서로 독립인 셋을 나란히 놓는다:

  1. 봇 파일   `summary.json` — 우리가 보낸 것과 받은 것
  2. 서버 메트릭 `/debug/stats` — 서버가 받았다고/보냈다고 말하는 것
  3. DB 행     correlation 집합 — 실제로 기록된 것

하나라도 다르면 **그 차이가 곧 발견이다**. 이 스크립트는 차이를 계산만 하고 원인을 추측하지 않는다.

    python tests/e2e/three_way.py --bots-out <dir> [--stats-before <json>] [--evidence <path>]
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

import db
from poll_until import DEFAULT_STATS, fetch_stats


def load_bots_summary(out_dir: Path) -> dict:
    p = out_dir / "summary.json"
    if not p.is_file():
        raise db.EnvironmentProblem(f"봇 요약이 없다: {p} (bots run 을 먼저 돌린다)")
    return json.loads(p.read_text(encoding="utf-8"))


def metric(stats: dict, key: str, sub: str | None = None):
    """`/debug/stats` 값 하나를 꺼낸다 (server 정본 모양, `03_server_impl.md` §5).

    라벨이 붙은 것은 **`[{"label": ..., "count": ...}]` 배열**이다(dict 가 아니다).
    `commands_rejected_total`(6라벨) · `messages_enqueued_total`/`messages_written_total`(3라벨) ·
    `upgrade_rejected_total`(5라벨). 라벨이 없으면 스칼라다.
    없는 키는 **None 으로 기록만** 한다 — 판정을 지어내지 않는다."""
    value = stats.get(key)
    if sub is None:
        return value
    if isinstance(value, list):
        for row in value:
            if isinstance(row, dict) and row.get("label") == sub:
                return row.get("count")
        return 0  # 라벨 배열에 없으면 그 라벨은 0건이다
    if isinstance(value, dict):
        return value.get(sub)
    return value


def run(args: argparse.Namespace) -> int:
    out_dir = Path(args.bots_out)
    bots = load_bots_summary(out_dir)
    agg = bots.get("aggregate", {})
    gates = bots.get("gates", {})

    stats = fetch_stats(args.stats)
    before = {}
    if args.stats_before:
        before = json.loads(Path(args.stats_before).read_text(encoding="utf-8"))

    def delta(key: str, sub: str | None = None):
        now = metric(stats, key, sub)
        was = metric(before, key, sub) if before else None
        if isinstance(now, (int, float)) and isinstance(was, (int, float)):
            return now - was
        return now

    db.require_tables("domain_events")
    corr = db.read_correlations(out_dir / "correlations.txt")
    arr = db.uuid_array_literal(corr)
    counts = dict(
        (r[0], int(r[1]))
        for r in db.psql_rows(
            "select event_type, count(*) from domain_events "
            f"where correlation_id = any('{arr}'::uuid[]) group by 1;"
        )
    )

    rows = [
        {
            "quantity": "세션 수",
            "bots": gates.get("sessions_ready"),
            "server_metric": delta("sessions_opened_total"),
            "db": counts.get("SESSION_OPENED", 0),
        },
        {
            "quantity": "세션 종료 수",
            "bots": gates.get("sessions_ready"),
            "server_metric": delta("sessions_closed_total"),
            "db": counts.get("SESSION_CLOSED", 0),
        },
        {
            "quantity": "명령 수",
            "bots": agg.get("sent_total"),
            "server_metric": delta("commands_received_total", "PING_SERVER"),
            "db": None,  # 명령은 기록하지 않는다 (ADR-0007 §1) — DB 열이 비는 것이 정상이다
        },
        {
            "quantity": "COMMAND_RESULT 수",
            "bots": agg.get("results_total"),
            "server_metric": delta("messages_enqueued_total", "COMMAND_RESULT"),
            "db": None,
        },
        {
            "quantity": "PING_REPLY 수",
            "bots": agg.get("replies_total"),
            "server_metric": delta("messages_enqueued_total", "PING_REPLY"),
            "db": None,
        },
    ]

    mismatches = []
    for r in rows:
        values = [v for v in (r["bots"], r["server_metric"], r["db"]) if isinstance(v, int)]
        if len(values) >= 2 and len(set(values)) > 1:
            mismatches.append(r["quantity"])

    # AC-9(c) 의 서버 내부 1:1 불변식. 키가 없으면 판정하지 않고 기록한다.
    recv = metric(stats, "commands_received_total")          # 스칼라 (server 정본)
    enq = metric(stats, "messages_enqueued_total", "COMMAND_RESULT")
    written = metric(stats, "messages_written_total", "COMMAND_RESULT")
    internal = None
    if isinstance(recv, int) and isinstance(enq, int):
        internal = recv - enq

    # SC-31 (server 가 준 회계 항등식):
    #   messages_enqueued_all == messages_written_all + messages_dropped_total + send_queue_depth
    # 정지 시점에는 dropped·depth 가 0 이므로 enqueued == written 이 된다. **세 값을 함께 적는다** —
    # dropped > 0 은 회계가 틀린 게 아니라 연결이 비정상 종료됐다는 뜻이다.
    enq_all = metric(stats, "messages_enqueued_all")
    wr_all = metric(stats, "messages_written_all")
    dropped = metric(stats, "messages_dropped_total")
    depth = metric(stats, "send_queue_depth")
    accounting = None
    if all(isinstance(v, int) for v in (enq_all, wr_all, dropped, depth)):
        accounting = {
            "messages_enqueued_all": enq_all,
            "messages_written_all": wr_all,
            "messages_dropped_total": dropped,
            "send_queue_depth": depth,
            "residual(enqueued - written - dropped - depth)": enq_all - wr_all - dropped - depth,
        }

    verdict = "PASS" if not mismatches and gates.get("all_ok") else "FAIL"
    db.emit(
        {
            "item": "계약 외 검사 — 서버·클라이언트·봇 3자 대조",
            "label_history": (
                "18차까지 라벨이 p0-02 번호였다. 이 슬라이스에서 그 번호는 client 의 "
                "재조정 항목이고 이 도구가 아니다(계약 §7b 규칙 7)."
            ),
            "verdict": verdict,
            "sources": {
                "bots": str(out_dir / "summary.json"),
                "server_metrics": args.stats,
                "db": f"correlation 집합 {len(corr)}건",
            },
            "table": rows,
            "mismatching_quantities": mismatches,
            "server_internal_1to1_delta(commands_received - enqueued{COMMAND_RESULT})": internal,
            "enqueued_minus_written{COMMAND_RESULT}": (
                enq - written if isinstance(enq, int) and isinstance(written, int) else None
            ),
            "SC-31_send_accounting": accounting,
            "persist_backlog": metric(stats, "persist_backlog"),
            "persist_backlog_limit": metric(stats, "persist_backlog_limit"),
            "domain_events_persisted_total": metric(stats, "domain_events_persisted_total"),
            # server: 0 이 아니면 버그다 (제약 위반으로 버린 이벤트).
            "domain_events_persist_failed_total": metric(stats, "domain_events_persist_failed_total"),
            "ws_connections": metric(stats, "ws_connections"),
            "live_connections": metric(stats, "live_connections"),
            "upgrade_rejected_total": stats.get("upgrade_rejected_total"),
            "bot_gates": gates,
            "note": (
                "명령/응답의 DB 열이 비는 것은 정상이다 — 명령과 응답은 기록 대상이 아니다"
                " (ADR-0007 §1). ws_connections 는 라우팅 표의 실제 길이이고 live_connections 는"
                " 소켓 수라, 종료 중에는 live 가 잠깐 더 크다(server §5). persist_backlog 는"
                " 정상 동작에서도 0~20 을 오간다 — 0 이 아닌 것이 이상이 아니다."
            ),
        },
        args.evidence,
    )
    return db.EXIT_OK if verdict == "PASS" else db.EXIT_FAIL


def main() -> int:
    ap = argparse.ArgumentParser(description="봇 / 서버 메트릭 / DB 3자 대조")
    ap.add_argument("--bots-out", required=True, help="bots run --out 으로 쓴 디렉토리")
    ap.add_argument("--stats", default=DEFAULT_STATS)
    ap.add_argument("--stats-before", help="부하 시작 전에 저장해 둔 /debug/stats JSON")
    ap.add_argument("--evidence")
    args = ap.parse_args()
    return db.main_guard(lambda: run(args))


if __name__ == "__main__":
    sys.exit(main())

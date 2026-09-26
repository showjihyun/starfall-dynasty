"""SC-59 (AC-16c) — 각 `(world_id, tick)` 의 `sequence` 가 0..n−1 로 빈틈없는가.

스펙 AC-16(c) 의 SQL 을 **원문 그대로** 쓴다. 조건절이 없다는 점이 중요하다 — 전 테이블이
대상이므로 AC-4 의 탐침 행도 규칙을 지켜야 한다(계약 §B 탐침 규칙: 전용 tick + sequence 0부터 연속).

    python tests/e2e/check_sequence_gaps.py
"""

from __future__ import annotations

import argparse
import sys

import db

# 스펙 §7 AC-16(c) 원문
GAP_SQL = (
    "select world_id, tick from domain_events "
    "group by world_id, tick "
    "having count(*) <> max(sequence) + 1 "
    "or min(sequence) <> 0 "
    "or count(distinct sequence) <> count(*);"
)


def run(args: argparse.Namespace) -> int:
    db.require_tables("domain_events")
    total = db.scalar_int("select count(*) from domain_events;")
    ticks = db.scalar_int("select count(distinct (world_id, tick)) from domain_events;")
    bad = db.psql_rows(GAP_SQL)

    detail = []
    for world_id, tick in bad[:20]:
        seqs = db.psql(
            "select string_agg(sequence::text, ',' order by sequence) from domain_events "
            f"where world_id = '{world_id}' and tick = {int(tick)};"
        )
        detail.append({"world_id": world_id, "tick": int(tick), "sequences": seqs})

    ok = not bad and total > 0
    db.emit(
        {
            "item": "SC-79 (AC-20d) sequence 빈틈 검사 — 전 테이블",
            "verdict": "PASS" if ok else ("FAIL" if bad else "FAIL(빈 테이블 — 검사할 것이 없다)"),
            "rows_total": total,
            "distinct_world_tick_groups": ticks,
            "violating_groups": len(bad),
            "violations": detail,
            "sql": GAP_SQL,
        },
        args.evidence,
    )
    return db.EXIT_OK if ok else db.EXIT_FAIL


def main() -> int:
    ap = argparse.ArgumentParser(description="sequence 빈틈 검사 (전 테이블)")
    ap.add_argument("--evidence")
    args = ap.parse_args()
    return db.main_guard(lambda: run(args))


if __name__ == "__main__":
    sys.exit(main())

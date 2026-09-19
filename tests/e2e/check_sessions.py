"""SC-57 / SC-58 / SC-52 — correlation 집합으로 세션 쌍을 대조한다 (AC-16a·b, AC-15a).

전수 `count(*)` 로 판정하지 **않는다**. DB 는 실행마다 누적되고 탐침 행·단독 왕복이 섞인다
(계약 §0.6, 스펙 §7 "세션 집합의 정의"). 배열 길이가 곧 "몇 건을 검사했는지"다.

    python tests/e2e/check_sessions.py --corr <dir>/correlations.txt [--expect 31]
"""

from __future__ import annotations

import argparse
import sys

import db


def run(args: argparse.Namespace) -> int:
    db.require_tables("domain_events")
    corr = db.read_correlations(args.corr)
    arr = db.uuid_array_literal(corr)

    counts = dict(
        (r[0], int(r[1]))
        for r in db.psql_rows(
            "select event_type, count(*) from domain_events "
            f"where correlation_id = any('{arr}'::uuid[]) group by 1;"
        )
    )
    opened = counts.get("SESSION_OPENED", 0)
    closed = counts.get("SESSION_CLOSED", 0)
    other_types = {k: v for k, v in counts.items() if k not in ("SESSION_OPENED", "SESSION_CLOSED")}

    # SC-58: 짝 없는/중복인 correlation (0행이어야 한다)
    broken = db.psql_rows(
        "select correlation_id, "
        "  count(*) filter (where event_type='SESSION_OPENED'), "
        "  count(*) filter (where event_type='SESSION_CLOSED') "
        f"from domain_events where correlation_id = any('{arr}'::uuid[]) "
        "group by 1 "
        "having count(*) filter (where event_type='SESSION_OPENED') <> 1 "
        "    or count(*) filter (where event_type='SESSION_CLOSED') <> 1 "
        "order by 1;"
    )
    # 집합에 있는데 DB 에 아예 없는 correlation (가장 나쁜 경우 — 기록 유실)
    present = {r[0] for r in db.psql_rows(
        f"select distinct correlation_id from domain_events where correlation_id = any('{arr}'::uuid[]);"
    )}
    absent = [c for c in corr if c not in present]

    # SC-52 보조: 이 집합의 종료 사유 분포. 정상 상태 실행이면 전부 CLIENT_CLOSED 여야 한다.
    reasons = dict(
        (r[0], int(r[1]))
        for r in db.psql_rows(
            "select payload->>'close_reason', count(*) from domain_events "
            f"where event_type='SESSION_CLOSED' and correlation_id = any('{arr}'::uuid[]) "
            "group by 1 order by 1;"
        )
    )
    # 이벤트 쌍이 actor_id 를 실제로 갖는가 (좁힘: 비-null — I-10)
    null_actor = db.scalar_int(
        "select count(*) from domain_events "
        f"where correlation_id = any('{arr}'::uuid[]) and actor_id is null;"
    )

    expect = args.expect if args.expect is not None else len(corr)
    ok = (
        opened == len(corr)
        and closed == len(corr)
        and not broken
        and not absent
        and null_actor == 0
        and opened == expect
    )
    result = {
        "item": "SC-57/58 (AC-16a·b) 세션 쌍 대조",
        "verdict": "PASS" if ok else "FAIL",
        "correlations_checked": len(corr),
        "expected": expect,
        "session_opened_rows": opened,
        "session_closed_rows": closed,
        "unpaired_or_duplicated": [{"correlation_id": r[0], "opened": int(r[1]), "closed": int(r[2])}
                                   for r in broken],
        "correlations_absent_from_db": absent,
        "rows_with_null_actor_id": null_actor,
        "close_reasons": reasons,
        "other_event_types_in_set": other_types,
    }
    db.emit(result, args.evidence)
    return db.EXIT_OK if ok else db.EXIT_FAIL


def main() -> int:
    ap = argparse.ArgumentParser(description="correlation 집합 기반 세션 쌍 대조")
    ap.add_argument("--corr", required=True)
    ap.add_argument("--expect", type=int, help="기대 세션 수 (기본: 집합 크기)")
    ap.add_argument("--evidence")
    args = ap.parse_args()
    return db.main_guard(lambda: run(args))


if __name__ == "__main__":
    sys.exit(main())

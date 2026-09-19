"""SC-61 (AC-17a) — **A 단계가 도는 동안** 기록이 이미 보이는가.

이 항목은 부하가 끝나면 **증명할 수 없다**. 끝난 뒤에는 열린 세션이 없으므로
"열려 있는 동안 OPENED 가 보이고 CLOSED 는 안 보인다"를 관찰할 방법이 사라진다.
그래서 `run_block.py load` 가 A 실행 중간에 이것을 부른다.

입력은 **봇이 `SESSION_READY` 수신 즉시 쓴** `correlations.live.txt`(+ Unity 것)다.
실행이 끝난 뒤 쓰는 `correlations.txt` 로는 이 시점에 조회할 대상이 없다.

    python tests/e2e/check_in_flight.py --corr <evidence>/mid/correlations.merged.txt
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

    expect_open = args.expect if args.expect is not None else len(corr)
    ok = opened == expect_open and closed == 0
    db.emit(
        {
            "item": "SC-61 (AC-17a) 실행 중 가시성",
            "verdict": "PASS" if ok else "FAIL",
            "measured_when": "A 단계 진행 중 (연결이 열려 있는 동안)",
            "correlations_in_set": len(corr),
            "expected_opened": expect_open,
            "session_opened_rows": opened,
            "session_closed_rows": closed,
            "meaning": (
                "OPENED 가 이미 있고 CLOSED 가 0 이면 기록이 종료 시 몰아쓰기가 아니다. "
                "OPENED 가 0 이면 tick 배치가 아직 커밋되지 않았거나 영속화가 멈춰 있다."
            ),
        },
        args.evidence,
    )
    return db.EXIT_OK if ok else db.EXIT_FAIL


def main() -> int:
    ap = argparse.ArgumentParser(description="SC-61 실행 중 가시성")
    ap.add_argument("--corr", required=True, help="실행 중에 갱신되는 correlations.live.txt")
    ap.add_argument("--expect", type=int, help="기대 OPENED 수 (기본: 집합 크기)")
    ap.add_argument("--evidence")
    args = ap.parse_args()
    return db.main_guard(lambda: run(args))


if __name__ == "__main__":
    sys.exit(main())

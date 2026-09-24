"""SC-62 (AC-17b) / SC-68 (AC-19c) — **호스트 쪽 폴링**으로 "몇 초 만에 도달했는가"를 잰다.

`recorded_at` 과 호스트 시계를 비교하지 않는다. 컨테이너 시계와 호스트 시계가 어긋나면
(WSL2 절전 복귀 등) 그 차이가 5초 판정에 섞인다 — server 검토 §6-9, architect 지시.
여기서는 **호스트에서 `count(*)` 를 폴링**하고 호스트의 단조 시계로만 경과를 잰다.

    # 마지막 봇이 끊긴 직후 (AC-17b)
    python tests/e2e/poll_until.py rows --corr <dir>/correlations.txt --expect 62 --timeout 15

    # DB 복구 후 백로그 소진 (AC-19c) — 정상 대역은 0 이 아니라 0~20 이다(server §5)
    python tests/e2e/poll_until.py backlog --target 20 --timeout 180
"""

from __future__ import annotations

import argparse
import json
import sys
import time
import urllib.error
import urllib.request

import db

DEFAULT_STATS = "http://127.0.0.1:8080/debug/stats"


def fetch_stats(url: str, timeout: float = 2.0) -> dict:
    try:
        with urllib.request.urlopen(url, timeout=timeout) as resp:  # noqa: S310 (로컬 고정 URL)
            return json.loads(resp.read().decode("utf-8"))
    except urllib.error.URLError as exc:
        raise db.EnvironmentProblem(f"{url} 에 접근할 수 없다: {exc}") from exc
    except json.JSONDecodeError as exc:
        raise db.NotImplementedYet(f"{url} 응답이 JSON 이 아니다: {exc}") from exc


def poll(measure, predicate, timeout: float, interval: float) -> tuple[bool, float, list]:
    """`predicate(value)` 가 참이 될 때까지 폴링. (도달여부, 경과초, 표본) 을 돌려준다."""
    start = time.perf_counter()
    trail = []
    while True:
        value = measure()
        elapsed = time.perf_counter() - start
        trail.append({"t_s": round(elapsed, 3), "value": value})
        if predicate(value):
            return True, elapsed, trail
        if elapsed >= timeout:
            return False, elapsed, trail
        time.sleep(interval)


def run_rows(args: argparse.Namespace) -> int:
    db.require_tables("domain_events")
    corr = db.read_correlations(args.corr)
    arr = db.uuid_array_literal(corr)
    expect = args.expect if args.expect is not None else len(corr) * 2  # OPENED + CLOSED

    sql = (
        "select count(*) from domain_events "
        f"where correlation_id = any('{arr}'::uuid[]) "
        "and event_type in ('SESSION_OPENED','SESSION_CLOSED');"
    )
    reached, elapsed, trail = poll(
        lambda: db.scalar_int(sql),
        lambda v: v >= expect,
        args.timeout,
        args.interval,
    )
    final = trail[-1]["value"]
    verdict = "PASS" if reached and elapsed <= args.deadline else "FAIL"
    db.emit(
        {
            "item": "계약 외 검사 — 마지막 봇 종료 후 모든 행 가시까지 (대기 헬퍼)",
            "verdict": verdict,
            "measured_how": "호스트에서 count(*) 폴링 (recorded_at 과 호스트 시계를 비교하지 않는다)",
            "correlations_checked": len(corr),
            "expected_rows": expect,
            "final_rows": final,
            "reached": reached,
            "elapsed_s": round(elapsed, 3),
            "deadline_s": args.deadline,
            "poll_interval_s": args.interval,
            "samples": trail[:60],
        },
        args.evidence,
    )
    return db.EXIT_OK if verdict == "PASS" else db.EXIT_FAIL


def run_backlog(args: argparse.Namespace) -> int:
    def measure() -> int:
        stats = fetch_stats(args.stats)
        if "persist_backlog" not in stats:
            raise db.NotImplementedYet("/debug/stats 에 persist_backlog 가 없다 (server T5)")
        return int(stats["persist_backlog"])

    if args.above is not None:
        reached, elapsed, trail = poll(
            measure, lambda v: v > args.above, args.timeout, args.interval
        )
        item = f"계약 외 검사 — persist_backlog > {args.above} 도달 대기 (대기 헬퍼)"
    else:
        reached, elapsed, trail = poll(
            measure, lambda v: v <= args.target, args.timeout, args.interval
        )
        item = (
            f"계약 외 검사 — persist_backlog <= {args.target} 복귀 "
            "(정상 대역 0~20 — 20 tick 주기 커밋이라 0 이 아닌 것이 이상이 아니다, server §5)"
        )

    db.emit(
        {
            "item": item,
            "verdict": "PASS" if reached else "FAIL",
            "reached": reached,
            "elapsed_s": round(elapsed, 3),
            "final_value": trail[-1]["value"],
            "poll_interval_s": args.interval,
            "samples": trail[:120],
        },
        args.evidence,
    )
    return db.EXIT_OK if reached else db.EXIT_FAIL


def main() -> int:
    ap = argparse.ArgumentParser(description="호스트 쪽 폴링 측정")
    sub = ap.add_subparsers(dest="mode", required=True)

    rows = sub.add_parser("rows", help="correlation 집합의 세션 행이 기대 수에 도달할 때까지")
    rows.add_argument("--corr", required=True)
    rows.add_argument("--expect", type=int, help="기대 행 수 (기본: 집합 크기 × 2)")
    rows.add_argument("--timeout", type=float, default=15.0)
    rows.add_argument("--deadline", type=float, default=5.0, help="AC-17b 의 하드 게이트 (기본 5초)")
    rows.add_argument("--interval", type=float, default=0.2, help="폴링 간격 (기본 200 ms)")
    rows.add_argument("--evidence")

    backlog = sub.add_parser("backlog", help="/debug/stats 의 persist_backlog")
    # 정상 대역이 0~20 이므로 기본 목표는 20 이다. 0 을 기다리면 영영 안 끝날 수 있다.
    backlog.add_argument("--target", type=int, default=20)
    backlog.add_argument("--above", type=int, help="이 값을 넘을 때까지 기다린다 (AC-19a)")
    backlog.add_argument("--timeout", type=float, default=120.0)
    backlog.add_argument("--interval", type=float, default=1.0)
    backlog.add_argument("--stats", default=DEFAULT_STATS)
    backlog.add_argument("--evidence")

    args = ap.parse_args()
    if args.mode == "rows":
        return db.main_guard(lambda: run_rows(args))
    return db.main_guard(lambda: run_backlog(args))


if __name__ == "__main__":
    sys.exit(main())

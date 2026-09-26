"""SC-60 (AC-16d): `occurred_at` 이 `tick` 에서 결정적으로 파생됐는가.

ADR-0006 §3:

    game_seconds = (tick / tick_hz) * calendar_scale      # 정수 나눗셈(내림)
    occurred_at  = calendar_epoch + game_seconds          # UTC, 초 단위, GameTime 형식

**`tick_hz` 로 먼저 나눈다.** `tick * calendar_scale / tick_hz` 는 같은 값을 주지만 두 축의
분리라는 의도가 사라지고 tick 이 클 때 넘칠 수 있다.

이 구현은 서버 코드와 **독립**이다(civil-from-days 를 여기서 다시 쓴다). 같은 코드로 만든 값을
같은 코드로 확인하면 아무것도 증명하지 못한다.

    python tests/e2e/check_occurred_at.py --selftest
    python tests/e2e/check_occurred_at.py --corr <dir>/correlations.txt --sample 10
"""

from __future__ import annotations

import argparse
import re
import sys

import db

GAME_TIME_RE = re.compile(r"^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z$")

# 스파이크 월드의 상수 (ADR-0007 §2 시드). 실행 시 worlds 행에서 읽어 대조한다.
DEFAULT_EPOCH = "3800-01-01T00:00:00Z"
DEFAULT_TICK_HZ = 20
DEFAULT_SCALE = 60


def civil_from_days(z: int) -> tuple[int, int, int]:
    """Howard Hinnant 의 civil_from_days. 1970-01-01 기준 일수 → (y, m, d)."""
    z += 719468
    era = (z if z >= 0 else z - 146096) // 146097
    doe = z - era * 146097
    yoe = (doe - doe // 1460 + doe // 36524 - doe // 146096) // 365
    y = yoe + era * 400
    doy = doe - (365 * yoe + yoe // 4 - yoe // 100)
    mp = (5 * doy + 2) // 153
    d = doy - (153 * mp + 2) // 5 + 1
    m = mp + 3 if mp < 10 else mp - 9
    return (y + (1 if m <= 2 else 0), m, d)


def days_from_civil(y: int, m: int, d: int) -> int:
    y -= 1 if m <= 2 else 0
    era = (y if y >= 0 else y - 399) // 400
    yoe = y - era * 400
    mp = m - 3 if m > 2 else m + 9
    doy = (153 * mp + 2) // 5 + d - 1
    doe = yoe * 365 + yoe // 4 - yoe // 100 + doy
    return era * 146097 + doe - 719468


def parse_game_time(s: str) -> int:
    """GameTime 문자열 → epoch(1970) 기준 초."""
    if not GAME_TIME_RE.match(s):
        raise ValueError(f"GameTime 형식이 아니다: {s!r}")
    y, mo, d = int(s[0:4]), int(s[5:7]), int(s[8:10])
    h, mi, sec = int(s[11:13]), int(s[14:16]), int(s[17:19])
    return days_from_civil(y, mo, d) * 86400 + h * 3600 + mi * 60 + sec


def format_game_time(total_seconds: int) -> str:
    days, rem = divmod(total_seconds, 86400)
    y, mo, d = civil_from_days(days)
    h, rem = divmod(rem, 3600)
    mi, sec = divmod(rem, 60)
    return f"{y:04d}-{mo:02d}-{d:02d}T{h:02d}:{mi:02d}:{sec:02d}Z"


def occurred_at_for(tick: int, tick_hz: int, scale: int, epoch: str) -> str:
    game_seconds = (tick // tick_hz) * scale          # tick_hz 로 먼저 나눈다
    return format_game_time(parse_game_time(epoch) + game_seconds)


def selftest() -> int:
    """server 가 이 PC PostgreSQL 에서 확인한 fixture 값 3건과 대조한다(검토 문서 §4)."""
    cases = [
        (0, "3800-01-01T00:00:00Z"),
        (19, "3800-01-01T00:00:00Z"),       # 정수 나눗셈으로 20 tick 이 같은 초에 매핑
        (1200, "3800-01-01T01:00:00Z"),     # SESSION_OPENED/basic.json
        (24000, "3800-01-01T20:00:00Z"),    # SESSION_CLOSED/client-closed.json
        (86400, "3800-01-04T00:00:00Z"),    # SESSION_OPENED/sequence-nonzero.json
    ]
    bad = 0
    for tick, want in cases:
        got = occurred_at_for(tick, DEFAULT_TICK_HZ, DEFAULT_SCALE, DEFAULT_EPOCH)
        ok = got == want
        bad += 0 if ok else 1
        print(f"tick={tick:<8} expected={want} got={got} {'ok' if ok else 'MISMATCH'}")
    # 왕복: 포맷 → 파싱 → 포맷
    for s in ("3800-01-01T00:00:00Z", "3827-04-13T18:32:11Z", "1970-01-01T00:00:00Z"):
        rt = format_game_time(parse_game_time(s))
        if rt != s:
            print(f"round-trip MISMATCH {s} -> {rt}")
            bad += 1
    print(f"selftest cases={len(cases) + 3} mismatches={bad}")
    return db.EXIT_OK if bad == 0 else db.EXIT_FAIL


def run(args: argparse.Namespace) -> int:
    db.require_tables("worlds", "domain_events")
    rows = db.psql_rows(
        "select world_id, tick_hz, calendar_epoch, calendar_scale from worlds order by world_id;"
    )
    if not rows:
        raise db.NotImplementedYet("worlds 에 행이 없다")
    worlds = {r[0]: (int(r[1]), r[2], int(r[3])) for r in rows}

    corr = db.read_correlations(args.corr)
    arr = db.uuid_array_literal(corr)
    sample = db.psql_rows(
        "select world_id, tick, occurred_at from domain_events "
        f"where correlation_id = any('{arr}'::uuid[]) order by tick, sequence limit {args.sample};"
    )
    if not sample:
        raise db.NotImplementedYet(
            "집합에 해당하는 domain_events 행이 없다 (부하 실행이 기록을 남기지 않았다)"
        )

    results = []
    mismatches = 0
    bad_pattern = 0
    for world_id, tick_s, occurred in sample:
        tick = int(tick_s)
        if world_id not in worlds:
            raise db.NotImplementedYet(f"worlds 에 없는 world_id: {world_id}")
        tick_hz, epoch, scale = worlds[world_id]
        expected = occurred_at_for(tick, tick_hz, scale, epoch)
        ok = expected == occurred
        pattern_ok = bool(GAME_TIME_RE.match(occurred))
        mismatches += 0 if ok else 1
        bad_pattern += 0 if pattern_ok else 1
        results.append(
            {"world_id": world_id, "tick": tick, "stored": occurred,
             "recomputed": expected, "match": ok, "pattern_ok": pattern_ok}
        )

    # 전체 행의 패턴은 표본이 아니라 전수로 본다(싸다).
    total = db.scalar_int(
        f"select count(*) from domain_events where correlation_id = any('{arr}'::uuid[]);"
    )
    bad_all = db.scalar_int(
        "select count(*) from domain_events "
        f"where correlation_id = any('{arr}'::uuid[]) "
        "and occurred_at !~ '^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z$';"
    )

    verdict = "PASS" if mismatches == 0 and bad_pattern == 0 and bad_all == 0 else "FAIL"
    db.emit(
        {
            "item": "계약 외 검사 — occurred_at 재계산 대조",
            "label_history": "18차까지 라벨이 p0-02 번호였다. p1-01 계약에 이 성질의 SC 행이 없다",
            # R18 라벨 정정: **p0-02 의 번호였다.** p1-01 계약에서 이 번호는 전혀 다른 항목이다 — 라벨로 집계하면 그 항목이 치르지 않은 verdict 를 받는다(architect R18, 계약 §7b 규칙 7). qa 가 리포트 12건을 훑어 **수확된 곳이 없음**을 확인했다.,
            "verdict": verdict,
            "correlations_checked": len(corr),
            "rows_in_set": total,
            "rows_failing_game_time_pattern": bad_all,
            "sample_size": len(results),
            "sample_mismatches": mismatches,
            "world_constants": {k: {"tick_hz": v[0], "calendar_epoch": v[1], "calendar_scale": v[2]}
                                for k, v in worlds.items()},
            "sample": results,
        },
        args.evidence,
    )
    return db.EXIT_OK if verdict == "PASS" else db.EXIT_FAIL


def main() -> int:
    ap = argparse.ArgumentParser(description="SC-60 occurred_at 재계산 대조")
    ap.add_argument("--selftest", action="store_true", help="DB 없이 공식만 검증한다")
    ap.add_argument("--corr", help="correlations.txt 경로")
    ap.add_argument("--sample", type=int, default=10, help="표본 수 (기본 10 — AC-16d)")
    ap.add_argument("--evidence", help="증거 JSON 을 쓸 경로")
    args = ap.parse_args()
    if args.selftest:
        return selftest()
    if not args.corr:
        ap.error("--corr 또는 --selftest 가 필요하다")
    return db.main_guard(lambda: run(args))


if __name__ == "__main__":
    sys.exit(main())

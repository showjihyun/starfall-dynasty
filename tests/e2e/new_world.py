"""새 월드 행 만들기 — p1-02 계약 §0.2 (스펙 I-70). **준비 도구이고 verdict 를 내지 않는다.**

기본 월드에서 광물은 한 번만 발견된다. 발견을 재는 모든 실행은 새 `worlds` 행에서 한다.
서버에 "새 월드" 기능을 넣지 않는다(스펙 §9.2) — SQL 한 줄이다.

    python tests/e2e/new_world.py --tag SC-80 --evidence <경로>     # INSERT 하고 증거를 남긴다
    python tests/e2e/new_world.py --tag SC-80 --dry-run             # SQL 만 찍는다
    python tests/e2e/new_world.py selftest

증거에 남기는 것: 월드 id, INSERT 문 전문, **INSERT 직후 그 월드의 `domain_events` 행 수 = 0**
(새 월드가 정말 비어 있었다는 단언 — 없으면 "새 월드" 는 이름뿐이다). 0 이 아니면 exit 1.

행은 지우지 않는다(감사 기록). 값은 스펙 I-70 이 고정했고 서버가 기동 시 대조한다.
"""

from __future__ import annotations

import argparse
import datetime as dt
import re
import secrets
import sys
import time

import db

UUIDV7 = re.compile(r"^[0-9a-f]{8}-[0-9a-f]{4}-7[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$")
TAG = re.compile(r"^[A-Za-z0-9-]{1,40}$")


def uuid7(now_ms: int | None = None) -> str:
    """RFC 9562 UUIDv7 — 48 bit 유닉스 ms + ver 7 + variant 10 + 무작위."""
    ms = int(time.time() * 1000) if now_ms is None else now_ms
    rand_a = secrets.randbits(12)
    rand_b = secrets.randbits(62)
    v = (ms & ((1 << 48) - 1)) << 80 | 0x7 << 76 | rand_a << 64 | 0b10 << 62 | rand_b
    h = f"{v:032x}"
    return f"{h[:8]}-{h[8:12]}-{h[12:16]}-{h[16:20]}-{h[20:]}"


def insert_sql(world_id: str, tag: str, stamp: str) -> str:
    return ("INSERT INTO worlds (world_id, name, tick_hz, calendar_epoch, calendar_scale, sim_version, last_tick) "
            f"VALUES ('{world_id}', 'qa-p1-02-{tag}-{stamp}', 20, '3800-01-01T00:00:00Z', 60, 1, NULL);")


def run(args: argparse.Namespace) -> int:
    if not TAG.match(args.tag):
        raise db.EnvironmentProblem(f"--tag 모양이 아니다: {args.tag!r}")
    wid = uuid7()
    stamp = dt.datetime.now().strftime("%Y%m%d%H%M")
    sql = insert_sql(wid, args.tag, stamp)
    if args.dry_run:
        print(sql)
        print(f"STARFALL_WORLD_ID={wid}")
        return db.EXIT_OK
    db.require_tables("worlds", "domain_events")
    out = db.psql_strict(sql)
    if not out.startswith("INSERT 0 1"):
        raise db.EnvironmentProblem(f"INSERT 가 한 행을 넣지 않았다: {out!r}")
    events = db.scalar_int_strict(f"select count(*) from domain_events where world_id = '{wid}';")
    row = db.psql_strict(f"select name, tick_hz, calendar_epoch, calendar_scale, sim_version, "
                  f"coalesce(last_tick::text, 'NULL') from worlds where world_id = '{wid}';")
    db.emit({"purpose": "새 월드 (계약 §0.2) — 준비 도구, 판정 아님", "world_id": wid,
             "insert_sql": sql, "row": row, "domain_events_in_new_world": events,
             "empty": events == 0, "env": f"STARFALL_WORLD_ID={wid}"}, args.evidence)
    return db.EXIT_OK if events == 0 else db.EXIT_FAIL


def selftest() -> int:
    ids = [uuid7() for _ in range(200)]
    cases = [
        ("200 개 전부 소문자 하이픈 UUIDv7 모양", all(UUIDV7.match(i) for i in ids)),
        ("200 개 전부 서로 다르다", len(set(ids)) == 200),
        ("시각 비트가 앞에 있어 ms 순으로 정렬된다", uuid7(1_000) < uuid7(2_000)),
        ("음성 대조: v4 모양은 걸린다", not UUIDV7.match("01a0b1c2-3d4e-4f01-8a2b-9c0d1e2f3a4b")),
        ("음성 대조: 대문자는 걸린다(서버 config.rs 규칙)", not UUIDV7.match(ids[0].upper())),
        ("INSERT 문이 스펙 I-70 값을 싣는다",
         all(x in insert_sql(ids[0], "t", "s") for x in (", 20, '3800-01-01T00:00:00Z', 60, 1, NULL)",))),
    ]
    fails = 0
    for name, ok in cases:
        print(f"{'OK  ' if ok else 'FAIL'} {name}")
        fails += 0 if ok else 1
    print(f"selftest: {'PASS' if fails == 0 else f'FAIL ({fails})'}  케이스={len(cases)}")
    return 0 if fails == 0 else 1


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("mode", nargs="?", choices=["selftest"])
    ap.add_argument("--tag", default="run", help="월드 이름에 들어갈 항목 표지(예: SC-80)")
    ap.add_argument("--dry-run", action="store_true")
    ap.add_argument("--evidence")
    args = ap.parse_args()
    if args.mode == "selftest":
        return selftest()
    return db.main_guard(lambda: run(args))


if __name__ == "__main__":
    sys.exit(main())

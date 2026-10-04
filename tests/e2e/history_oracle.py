"""p1-02 역사 오라클 교차 — 계약 SC-58 (AC-12a, H-12) · SC-83 (AC-16d).

    python tests/e2e/history_oracle.py --world <id>                    # SC-58
    python tests/e2e/history_oracle.py --world <id> --mineral <X>      # SC-83
    python tests/e2e/history_oracle.py selftest

**오라클은 역사 코드와 독립이다**: `domain_events` 만 보고 각 `(world, star_system, mineral)` 의
`(tick, sequence)` 최소 양수 `MINERAL_MINED` 를 "기대 발견" 으로 삼는다(`01_history_review.md`
§6.1 — **단 필드 이름은 계약의 것**: `star_system_id`·`quantity_kg`. 검토 문서 원문의
`system_id`·`quantity` 로 돌리면 `->>` 가 전부 NULL 이 되어 NULL 키 한 행만 나온다 — 계약 §0.10).

그 집합과 `historical_events`(`MINERAL_DISCOVERED`) + `historical_event_sources` 의 근거 집합을
**양방향**으로 뺀다. 판정 조건:
  - 오라클 행 ≥ 2 (SC-58) — 아니면 FAIL(분모)
  - 두 차집합 모두 0
  - **양성 대조**: `star_system_id` 또는 `mineral_id` 가 NULL 인 `MINERAL_MINED` 행 = 0
    (0 이 아니면 필드 이름을 잘못 찾고 있다 — 검출기 사망형)
SC-83 은 광물 X 하나에 대해: 역사 행 정확히 1 · 근거 = 오라클의 첫 채굴 · 증거 1 행 ·
그리고 **X 의 `MINERAL_MINED` ≥ 2**(1 건이면 "첫" 이 자명하다).

⚠ 역사 테이블의 열 이름은 마이그레이션 0003(history, H2)이 정본이다. 아래 `HIST_SQL`·`EVIDENCE_SQL`
은 ADR-0014 §3 과 계약 fixture 에서 가정한 모양이고, 0003 이 들어오면 경계면 검증에서 맞춘다.

종료 코드: db.py 규약.
"""

from __future__ import annotations

import argparse
import json
import sys

import db

LABEL_CROSS = {"item": "p1-02 SC-58 (AC-12a, H-12) 오라클 ↔ 역사 발견 집합 양방향 차집합 0"}
LABEL_ONE = {"item": "p1-02 SC-83 (AC-16d) 광물 X 의 MINERAL_DISCOVERED 정확히 1 · 근거 = 첫 채굴 · 증거 1"}

MIN_ORACLE_ROWS = 2   # 계약 SC-58

ORACLE_SQL = (
    "select distinct on (payload->>'star_system_id', payload->>'mineral_id') "
    "payload->>'star_system_id', payload->>'mineral_id', tick, sequence, event_id "
    "from domain_events where world_id = '{w}' and event_type = 'MINERAL_MINED' "
    "and (payload->>'quantity_kg')::bigint > 0 "
    "order by payload->>'star_system_id', payload->>'mineral_id', tick, sequence;"
)
NULL_PROBE_SQL = (
    "select count(*) from domain_events where world_id = '{w}' and event_type = 'MINERAL_MINED' "
    "and (payload->>'star_system_id' is null or payload->>'mineral_id' is null);"
)
MINED_COUNT_SQL = (
    "select count(*) from domain_events where world_id = '{w}' and event_type = 'MINERAL_MINED' "
    "and payload->>'mineral_id' = '{m}';"
)
# 가정한 0003 모양 — 경계면 검증 대상
HIST_SQL = (
    "select h.location->>'star_system_id', h.payload->>'mineral_id', s.source_event_id, h.historical_event_id "
    "from historical_events h join historical_event_sources s on s.historical_event_id = h.historical_event_id "
    "where h.world_id = '{w}' and h.event_type = 'MINERAL_DISCOVERED' "
    "order by 1, 2;"
)
EVIDENCE_SQL = "select count(*) from evidence where historical_event_id = '{h}';"


def cross(oracle: list[tuple], history: list[tuple]) -> dict:
    """oracle: (system, mineral, event_id) / history: (system, mineral, source_event_id, hist_id)."""
    o = {(s, m): e for s, m, e in oracle}
    h: dict[tuple, list] = {}
    for s, m, src, hid in history:
        h.setdefault((s, m), []).append(src)
    dup = {k: v for k, v in h.items() if len(v) > 1}
    only_oracle = sorted(k for k in o if k not in h)
    only_history = sorted(k for k in h if k not in o)
    wrong_source = sorted(k for k in o if k in h and h[k] != [o[k]])
    return {
        "oracle_rows": len(o), "history_rows": sum(len(v) for v in h.values()),
        "only_in_oracle": [list(k) for k in only_oracle],
        "only_in_history": [list(k) for k in only_history],
        "wrong_source": [{"key": list(k), "oracle": o[k], "history": h[k]} for k in wrong_source],
        "duplicate_keys_in_history": {"/".join(k): v for k, v in dup.items()},
        "ok_sets": not (only_oracle or only_history or wrong_source or dup),
    }


def run(args: argparse.Namespace) -> int:
    w = args.world
    db.uuid_array_literal([w])
    db.require_tables("domain_events", "historical_events", "historical_event_sources", "evidence")
    # `quantity_kg` 가 없으면 `(null)::bigint > 0` 이 행을 조용히 뺀다 — NULL 프로브는 두 키만 봤다.
    db.require_payload_keys("domain_events", "payload", "MINERAL_MINED",
                            ["star_system_id", "mineral_id", "quantity_kg"], f"world_id = '{w}'",
                            allow_empty=True)
    db.require_payload_keys("historical_events", "location", "MINERAL_DISCOVERED", ["star_system_id"],
                            f"world_id = '{w}'", allow_empty=True)
    db.require_payload_keys("historical_events", "payload", "MINERAL_DISCOVERED", ["mineral_id"],
                            f"world_id = '{w}'", allow_empty=True)
    null_rows = db.scalar_int_strict(NULL_PROBE_SQL.format(w=w))
    oracle = [(s, m, e) for s, m, _t, _q, e in db.psql_rows_strict(ORACLE_SQL.format(w=w))]
    history = [tuple(r) for r in db.psql_rows_strict(HIST_SQL.format(w=w))]
    res = cross(oracle, history)
    res["null_field_mineral_mined_rows"] = null_rows
    res["world_id"] = w
    if args.mineral:
        m = args.mineral
        if not m.replace("-", "").isalnum():
            raise db.EnvironmentProblem(f"광물 id 모양이 아니다: {m!r}")
        mined = db.scalar_int_strict(MINED_COUNT_SQL.format(w=w, m=m))
        rows = [r for r in history if r[1] == m]
        orow = [r for r in oracle if r[1] == m]
        ev = db.scalar_int_strict(EVIDENCE_SQL.format(h=rows[0][3])) if len(rows) == 1 else None
        ok = (null_rows == 0 and mined >= 2 and len(rows) == 1 and len(orow) == 1
              and rows[0][2] == orow[0][2] and ev == 1)
        out = dict(LABEL_ONE)
        out.update({"mineral_id": m, "mineral_mined_rows_for_x": mined, "discovered_rows": len(rows),
                    "source_equals_oracle_first": bool(rows and orow and rows[0][2] == orow[0][2]),
                    "evidence_rows": ev, "null_field_mineral_mined_rows": null_rows})
        out["verdict"] = "PASS" if ok else ("FAIL(분모 — X 의 채굴이 2 건 미만)" if mined < 2 else "FAIL")
    else:
        ok = res["ok_sets"] and res["oracle_rows"] >= MIN_ORACLE_ROWS and null_rows == 0
        out = dict(LABEL_CROSS)
        out.update(res)
        out["verdict"] = ("PASS" if ok else
                          "FAIL(분모 — 오라클 행 < 2)" if res["oracle_rows"] < MIN_ORACLE_ROWS else
                          "FAIL(필드 이름 — NULL 키 MINERAL_MINED 가 있다)" if null_rows else "FAIL")
    out["oracle_sql"] = ORACLE_SQL.format(w=w)
    db.emit(out, args.evidence)
    return db.EXIT_OK if out["verdict"] == "PASS" else db.EXIT_FAIL


def selftest() -> int:
    O = [("cradle", "ferrosite", "e1"), ("cradle", "glacine", "e7")]
    cases = [
        ("일치 → 차집합 0", cross(O, [("cradle", "ferrosite", "e1", "h1"), ("cradle", "glacine", "e7", "h2")])["ok_sets"], True),
        ("역사에 발견 하나 빠짐 → 불일치", cross(O, [("cradle", "ferrosite", "e1", "h1")])["ok_sets"], False),
        ("역사가 두 번째 채굴을 근거로 → 불일치", cross(O, [("cradle", "ferrosite", "e2", "h1"), ("cradle", "glacine", "e7", "h2")])["ok_sets"], False),
        ("같은 키에 역사 두 행 → 불일치", cross(O, [("cradle", "ferrosite", "e1", "h1"), ("cradle", "ferrosite", "e3", "h9"), ("cradle", "glacine", "e7", "h2")])["ok_sets"], False),
        ("오라클에 없는 발견 → 불일치", cross(O, [("cradle", "ferrosite", "e1", "h1"), ("cradle", "glacine", "e7", "h2"), ("cradle", "cobaltine", "e9", "h3")])["ok_sets"], False),
        # 검토 문서 원문 SQL 이 만들던 모양: NULL 키 한 행. 집합 비교만으로는 "일치" 가 될 수 있다 —
        # 그래서 판정에 분모(≥ 2)와 NULL 탐침을 따로 둔다.
        ("NULL 키 한 행끼리는 집합이 일치해 버린다(그래서 분모·NULL 탐침이 필요하다)",
         cross([("", "", "e1")], [("", "", "e1", "h1")])["ok_sets"], True),
    ]
    # 없는 열 조회는 **오류로 끝나야** 한다 — 0 행·1 행 결과로 읽히면 안 된다(리더 지시).
    err = "ERROR:  column h.location does not exist" + chr(10) + "LINE 1: select h.location->>..."
    cases += [
        ("없는 열 → NotImplementedYet(구현·도구 모양 불일치), 결과 행이 아니다",
         type(db.classify_psql_failure(err)).__name__, "NotImplementedYet"),
        ("문법 오류 → QueryError (main_guard 가 FAIL 로)", type(db.classify_psql_failure("ERROR:  syntax error at or near")).__name__, "QueryError"),
        ("대조: 옛 psql_rows 경로는 오류 문자열을 **행으로** 쪼갠다(그래서 strict 를 쓴다)",
         len([l.split("|") for l in err.splitlines()]) > 0, True),
    ]
    fails = 0
    for name, got, want in cases:
        ok = got == want
        print(f"{'OK  ' if ok else 'FAIL'} {name} :: {got} 기대={want}")
        fails += 0 if ok else 1
    print(f"selftest: {'PASS' if fails == 0 else f'FAIL ({fails})'}  케이스={len(cases)}")
    return 0 if fails == 0 else 1


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("mode", nargs="?", choices=["selftest"])
    ap.add_argument("--world")
    ap.add_argument("--mineral")
    ap.add_argument("--evidence")
    args = ap.parse_args()
    if args.mode == "selftest":
        return selftest()
    if not args.world:
        ap.error("--world 가 필요하다(계약 §0.2 — 발견은 새 월드에서만 잰다)")
    return db.main_guard(lambda: run(args))


if __name__ == "__main__":
    sys.exit(main())

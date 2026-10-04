"""p1-02 계약 SC-107 — 정지 경로 제약 분모 대조 (ADR-0013 §5a-1).

복구 불가 기록 실패 = 월드 정지(사용자 결정 Q1)이므로 **표에 없는 제약은 검토되지 않은 정지 경로**다.
ADR 표가 요구하는 테이블별 제약 수와 DB 의 `pg_constraint` 실측 수가 같아야 한다.

    python tests/e2e/constraint_census.py          # DB 대조 (0001~0003 적용 DB 에서만 판정)
    python tests/e2e/constraint_census.py selftest # ADR 파싱 + 비교 로직의 음성·양성 대조

**세는 기준**: PostgreSQL 18 은 NOT NULL 도 `pg_constraint` 에 `contype = 'n'` 로 적는다. ADR 의
`worlds` 11 · `domain_events` 16 실측이 `n` 을 포함한 수다(architect: domain_events = p1 u1 f1 c3 n10).
그래서 도구는 **contype 별 수를 따로 찍고** 합을 비교한다.

**적용 상태를 가른다**(architect r2 정정): 0002 전에는 `domain_events` 16, 뒤에는 17 이다. 0001~0003 이
전부 적용된 DB 가 아니면 **판정하지 않는다**(exit 4) — 적용 전에 17 을 기대하면 거짓 FAIL, 적용 뒤에
16 을 기대하면 CHECK 누락을 놓친다.

종료 코드: 0 PASS / 1 FAIL / 2 미검증(환경) / 3 구현 없음 / **4 판정 불가(마이그레이션 미적용·ADR 파싱 실패)**.
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

import db

LABEL = {"item": "p1-02 SC-107 (ADR-0013 §5a-1) 정지 경로 제약 분모 — pg_constraint 실측 = ADR 요구"}
EXIT_UNDECIDABLE = 4
ADR = Path("docs/adr/0013-economic-state-persistence-and-durable-idempotency.md")
HISTORY_TABLES = ("historical_events", "historical_event_sources", "evidence", "history_cursor")
REQUIRED_MIGRATIONS = {1, 2, 3}

# 소제목 뒤에 `— 키 …` 설명이 붙는 행이 있다 — 닫는 `**` 를 요구하면 5 표 중 2 표만 읽힌다(selftest 가 잡았다).
TABLE_HEAD = re.compile(r"^\*\*`(\w+)` \(([^)]*)\)")
# 소제목은 "(0003, 요구 25 — …)"(초안) 에서 "(0003, 48 — …)"(실측) 로 바뀌었다 — 괄호 안 **첫 정수**가 요구 수다(architect 규약).
HISTORY_HEAD = re.compile(r"^\*\*역사 테이블 \(0003,\s*(?:요구\s*)?(\d+)")
TOTALS = re.compile(r"분모 합계 = (\d+) \+ (\d+) = (\d+) 제약 \+ 트리거 (\d+)")


def parse_adr(text: str) -> dict:
    """§5a-1 의 소제목 괄호 안 **마지막 정수**가 그 테이블의 요구 수다(`0001 16 — 실측 + 0002 추가 1 = 17` → 17)."""
    per_table: dict[str, int] = {}
    history_total = None
    totals = None
    in_block = False
    for line in text.splitlines():
        if line.startswith("#### 5a-1"):
            in_block = True
            continue
        if in_block and line.startswith("#### "):
            break
        if not in_block:
            continue
        m = TABLE_HEAD.match(line)
        if m:
            nums = re.findall(r"\d+", m.group(2))
            per_table[m.group(1)] = int(nums[-1])
        m = HISTORY_HEAD.match(line)
        if m:
            history_total = int(m.group(1))
        m = TOTALS.search(line)
        if m:
            totals = tuple(int(x) for x in m.groups())
    return {"per_table": per_table, "history_total": history_total, "totals": totals}


def compare(req: dict, actual: dict[str, dict[str, int]], triggers: int) -> dict:
    problems = []
    if not req["per_table"] or req["history_total"] is None or req["totals"] is None:
        return {"ok": False, "undecidable": True, "problems": ["ADR 표 파싱 실패 — 0 행이거나 집계 줄이 없다"]}
    econ, hist, total, trig = req["totals"]
    if sum(req["per_table"].values()) != econ:
        problems.append(f"ADR 자기 모순: 테이블 합 {sum(req['per_table'].values())} ≠ 집계 {econ}")
    if req["history_total"] != hist or econ + hist != total:
        problems.append(f"ADR 자기 모순: 역사 {req['history_total']} / 집계 {hist}, {econ}+{hist} ≠ {total}")
    rows = []
    for t, want in sorted(req["per_table"].items()):
        got = sum(actual.get(t, {}).values())
        if want == 0:
            problems.append(f"{t}: 요구 0 — 파싱 오류로 본다")
        if got != want:
            problems.append(f"{t}: 실측 {got} ≠ 요구 {want}")
        rows.append({"table": t, "required": want, "actual": got, "by_contype": actual.get(t, {})})
    hist_got = sum(sum(actual.get(t, {}).values()) for t in HISTORY_TABLES)
    if hist_got != req["history_total"]:
        problems.append(f"역사 4 표: 실측 {hist_got} ≠ 요구 {req['history_total']}")
    rows.append({"table": "+".join(HISTORY_TABLES), "required": req["history_total"], "actual": hist_got,
                 "by_contype": {t: actual.get(t, {}) for t in HISTORY_TABLES}})
    if triggers != trig:
        problems.append(f"추가 전용 트리거: 실측 {triggers} ≠ 요구 {trig}")
    return {"ok": not problems, "undecidable": False, "tables_parsed": len(req["per_table"]),
            "required_total": total, "required_triggers": trig, "actual_triggers": triggers,
            "rows": rows, "problems": problems}


def run(args: argparse.Namespace) -> int:
    req = parse_adr((db.REPO_ROOT / ADR).read_text(encoding="utf-8"))
    db.require_tables("_sqlx_migrations")
    applied = {int(v) for (v,) in db.psql_rows_strict("select version from _sqlx_migrations where success;")}
    out = dict(LABEL)
    out["migrations_applied"] = sorted(applied)
    if not REQUIRED_MIGRATIONS <= applied:
        out["verdict"] = f"판정 불가 — 0001~0003 이 전부 적용되지 않았다(적용 {sorted(applied)})"
        db.emit(out, args.evidence)
        return EXIT_UNDECIDABLE
    tables = list(req["per_table"]) + list(HISTORY_TABLES)
    names = ",".join(f"'{t}'" for t in tables)
    actual: dict[str, dict[str, int]] = {}
    for rel, contype, n in db.psql_rows_strict(
            "select c.conrelid::regclass::text, c.contype, count(*) from pg_constraint c "
            f"where c.conrelid::regclass::text in ({names}) group by 1, 2 order by 1, 2;"):
        actual.setdefault(rel, {})[contype] = int(n)
    triggers = db.scalar_int_strict(
        "select count(*) from pg_trigger t where not t.tgisinternal "
        f"and t.tgrelid::regclass::text in ({names});")
    res = compare(req, actual, triggers)
    out.update(res)
    out["counting_basis"] = "pg_constraint 전 contype(p·u·f·c·n 포함), pg_trigger 비내부"
    out["verdict"] = "PASS" if res["ok"] else ("판정 불가" if res["undecidable"] else "FAIL")
    db.emit(out, args.evidence)
    return db.EXIT_OK if res["ok"] else (EXIT_UNDECIDABLE if res["undecidable"] else db.EXIT_FAIL)


def selftest() -> int:
    req = parse_adr((db.REPO_ROOT / ADR).read_text(encoding="utf-8"))
    good = {"worlds": {"p": 1, "c": 3, "n": 7}, "domain_events": {"p": 1, "u": 1, "f": 1, "c": 4, "n": 10},
            "inventory_items": {"p": 1, "f": 1, "c": 1, "n": 4}, "deposit_states": {"p": 1, "f": 1, "c": 4, "n": 5},
            "processed_commands": {"p": 1, "f": 1, "c": 1, "n": 5},
            "historical_events": {"n": 17, "p": 1, "u": 1, "f": 1, "c": 2}, "historical_event_sources": {"p": 1, "n": 4},
            "evidence": {"p": 1, "f": 1, "n": 8, "c": 3}, "history_cursor": {"p": 1, "n": 5, "c": 2}}
    # 역사 쪽은 합계만 판정하므로 ADR 의 현재 역사 합계에 맞춘다(48 → 50 갱신에서 낡았다).
    if req["history_total"] is not None:
        rest = sum(sum(good[t].values()) for t in HISTORY_TABLES if t != "historical_events")
        good["historical_events"] = {**good["historical_events"],
                                     "n": req["history_total"] - rest - 5}
    missing_check = {**good, "domain_events": {"p": 1, "u": 1, "f": 1, "c": 3, "n": 10}}   # 0002 CHECK 없음
    cases = [
        # 숫자를 박지 않고 **ADR 자신과의 정합**을 본다 — 표가 갱신될 때마다 selftest 가 낡지 않게
        # (25 → 48 갱신에서 실제로 낡았다). 단 domain_events 17 은 "0002 적용 뒤" 라는 판정 조건의
        # 전제라 고정해 둔다(architect r2 정정).
        ("실제 ADR 파싱: 경제 5 표 · 역사 소제목 읽힘 · 테이블 합 = 집계 경제 · 경제 + 역사 = 합계",
         len(req["per_table"]) == 5 and req["history_total"] is not None
         and sum(req["per_table"].values()) == req["totals"][0]
         and req["history_total"] == req["totals"][1]
         and req["totals"][0] + req["totals"][1] == req["totals"][2]),
        ("domain_events 는 17(0002 후)로 읽힌다 — 16 이 아니다", req["per_table"].get("domain_events") == 17),
        ("요구와 같은 실측 → PASS", compare(req, good, 4)["ok"]),
        ("음성 대조: payload_is_object CHECK 가 빠진 DB(16) → FAIL", not compare(req, missing_check, 4)["ok"]),
        ("음성 대조: 트리거 3 → FAIL", not compare(req, good, 3)["ok"]),
        ("음성 대조: 검토 없는 새 제약(+1) → FAIL",
         not compare(req, {**good, "inventory_items": {"p": 1, "f": 1, "c": 2, "n": 4}}, 4)["ok"]),
        ("파싱 0 행 → 판정 불가(초록 아님)", compare(parse_adr("아무 표도 없다"), good, 4)["undecidable"]),
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
    ap.add_argument("--evidence")
    args = ap.parse_args()
    if args.mode == "selftest":
        return selftest()
    return db.main_guard(lambda: run(args))


if __name__ == "__main__":
    sys.exit(main())

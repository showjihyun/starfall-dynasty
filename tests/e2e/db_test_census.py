"""p1-02 계약 SC-113 — CI 의 DB 통합 테스트가 **이름으로** 실제로 돌았는가 (리더 결정 Q-2, 계약 §3.4).

    python tests/e2e/db_test_census.py --contract <02_sprint_contract.md> --log <cargo test 로그>
    python tests/e2e/db_test_census.py selftest

**대조 목록은 손으로 유지하지 않는다**: 계약 §1 행에서 **백틱 테스트 이름 바로 뒤에 ` [DB]` 표식**이
붙은 것을 뽑는다(`cargo test -p <crate> <name>` 형태든 이름만이든). 헬퍼 `starfall-testdb` 가
stderr 에 `STARFALL_DB_TEST RAN <이름>` / `STARFALL_DB_TEST SKIPPED <이름> reason=…` 을 찍는다.

판정(FAIL 조건):
  - 목록 수 0 (표 파싱 사망 — 0 = 0 은 검사가 아니다)
  - 목록에 있는데 RAN 이 없는 이름 ≥ 1 (**개수만 세면 다른 테스트가 빈자리를 채운다**)
  - SKIPPED ≥ 1
역방향: RAN 에만 있고 목록에 없는 이름은 **출력만** 한다(계약 행의 ` [DB]` 표식 누락 신호).

선행 조건(계약 SC-113): 통과한 테스트의 RAN 줄도 로그에 보여야 한다 — libtest 는 통과 테스트의
출력을 캡처할 수 있다. server 가 T0 첫 RED 에서 실측하기 전에는 이 항목이 **대기**다.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

LABEL = {"item": "p1-02 SC-113 (Q-2) CI DB 테스트 RAN 이름 대조 — 누락 0 · SKIPPED 0"}

CONTRACT_ROW = re.compile(r"^\|\s*SC-(\d+)\s*\|")
DB_NAME = re.compile(r"`(?:cargo test -p [\w-]+ )?([a-z_][a-z0-9_]*)` \[DB\]")
RAN = re.compile(r"STARFALL_DB_TEST RAN (\S+)")
SKIPPED = re.compile(r"STARFALL_DB_TEST SKIPPED (\S+)")

for _s in (sys.stdout, sys.stderr):
    try:
        _s.reconfigure(encoding="utf-8")
    except (AttributeError, ValueError):
        pass


def contract_db_tests(text: str) -> dict[str, list[int]]:
    out: dict[str, list[int]] = {}
    section = None
    for line in text.splitlines():
        if line.startswith("## "):
            section = line[3:].strip()
        m = CONTRACT_ROW.match(line)
        if not m or not (section and section.startswith("1.")):
            continue
        for name in DB_NAME.findall(line):
            out.setdefault(name, []).append(int(m.group(1)))
    return out


def judge(expected: dict[str, list[int]], log: str) -> dict:
    ran = set(RAN.findall(log))
    skipped = SKIPPED.findall(log)
    missing = sorted(set(expected) - ran)
    extra = sorted(ran - set(expected))
    ok = bool(expected) and not missing and not skipped
    return {"expected": len(expected), "ran_lines": len(ran), "expected_and_ran": len(set(expected) & ran),
            "missing": [{"name": n, "sc": expected[n]} for n in missing],
            "skipped": skipped, "ran_but_not_in_contract": extra, "ok": ok}


def selftest() -> int:
    contract = ("## 1. 검증 항목\n"
                "| SC-20 | x | `cargo test -p starfall-persistence batch_recommit_is_noop` [DB] + `other_unit` | server | AC | E1 |\n"
                "| SC-51 | x | `cargo test -p starfall-persistence history_first_discovery_rows` [DB] | history | AC | E1 |\n"
                "## 9. 이력\n"
                "| SC-99 | `not_counted` [DB] | — |\n")
    exp = contract_db_tests(contract)
    good = "STARFALL_DB_TEST RAN batch_recommit_is_noop\nSTARFALL_DB_TEST RAN history_first_discovery_rows\n"
    cases = [
        ("§1 의 [DB] 표식 2 개만 뽑는다(§1 밖·표식 없는 이름 제외)", sorted(exp) == ["batch_recommit_is_noop", "history_first_discovery_rows"]),
        ("둘 다 RAN → PASS", judge(exp, good)["ok"]),
        ("음성: 하나 누락 → FAIL(이름으로)", judge(exp, good.splitlines()[0])["missing"][0]["name"] == "history_first_discovery_rows"),
        ("음성: 개수는 같지만 다른 이름이 채움 → FAIL",
         not judge(exp, "STARFALL_DB_TEST RAN batch_recommit_is_noop\nSTARFALL_DB_TEST RAN something_else\n")["ok"]),
        ("음성: SKIPPED 1 → FAIL", not judge(exp, good + "STARFALL_DB_TEST SKIPPED x reason=no db\n")["ok"]),
        ("음성: 목록 0(파싱 사망) → FAIL", not judge({}, good)["ok"]),
        ("역방향: 목록 밖 RAN 은 출력된다", judge(exp, good + "STARFALL_DB_TEST RAN extra_one\n")["ran_but_not_in_contract"] == ["extra_one"]),
    ]
    real = Path(__file__).resolve().parents[2] / "_workspace/p1-02-mining/02_sprint_contract.md"
    if real.is_file():
        n = len(contract_db_tests(real.read_text(encoding="utf-8")))
        print(f"참고: 현재 계약의 [DB] 테스트 이름 = {n}")
    fails = 0
    for name, ok in cases:
        print(f"{'OK  ' if ok else 'FAIL'} {name}")
        fails += 0 if ok else 1
    print(f"selftest: {'PASS' if fails == 0 else f'FAIL ({fails})'}  케이스={len(cases)}")
    return 0 if fails == 0 else 1


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("mode", nargs="?", choices=["selftest"])
    ap.add_argument("--contract")
    ap.add_argument("--log")
    ap.add_argument("--evidence")
    args = ap.parse_args()
    if args.mode == "selftest":
        return selftest()
    if not (args.contract and args.log):
        ap.error("--contract 와 --log 가 필요하다")
    res = judge(contract_db_tests(Path(args.contract).read_text(encoding="utf-8")),
                Path(args.log).read_text(encoding="utf-8", errors="replace"))
    out = dict(LABEL)
    out["verdict"] = "PASS" if res["ok"] else "FAIL"
    out.update(res)
    text = json.dumps(out, indent=2, ensure_ascii=False)
    print(text)
    if args.evidence:
        Path(args.evidence).write_text(text + "\n", encoding="utf-8")
    return 0 if res["ok"] else 1


if __name__ == "__main__":
    sys.exit(main())

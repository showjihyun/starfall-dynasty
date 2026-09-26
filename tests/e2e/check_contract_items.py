"""계약 표와 리포트 판정의 **대조** — 항목이 집 없이 판정되는 것을 막는다.

p1-01 라운드 4에서 **`SC-87`이 세 라운드 동안 계약 표에 없는 채로 네 번 판정됐다**(r4 §5.12).
계약 머리는 *"Phase 5 평가는 이 표의 항목으로만 한다"*고 적고 있으므로, 그 판정들은
**합격 기준이 지난 리포트 본문에만 있는 상태**였다 — 무엇과 대조해야 하는지 아무도 알 수 없었다.

## 왜 "빈 번호 검사"가 아니라 이 방향인가

처음 제안은 *"SC 번호를 훑어 빈 번호와 중복을 드러낸다"*였다. **그것은 이번 건을 우연히만 잡는다** —
`SC-87`이 86과 88 사이에 있었기 때문에 빈 번호가 보였을 뿐이고, r2가 그 검사를 `SC-90`으로
붙였다면 **빈 번호가 안 생겨 검사는 초록**이었다. 계약 §7b(자명 통과 시험)를 그 게이트에 적용하면
바로 나온다:

    자명 통과 상태 = "계약에 없는 항목의 번호가 연속 범위 밖이다" → 실제로 가능하다 → 절을 고친다.

**판정 가능한 것은 역방향이다**: *리포트가 판정한 모든 항목이 계약 표에 있는가.* 이 방향은
번호 배치와 무관하게 **r2 당시에 즉시** 잡았을 것이다.

순방향(계약에 있는데 그 라운드 리포트에 안 나온 항목)은 **위반이 아니다** — 블록이 나뉘어 돌고
`대기`가 정상이기 때문이다. 그래서 **세기만 하고 판정하지 않는다.**

사용법:

    python tests/e2e/check_contract_items.py --contract <02_*.md> --report <04_*.md> [--report ...]
    python tests/e2e/check_contract_items.py --selftest

종료 코드: 0 통과 / 1 위반(집 없는 판정) / 2 사용법 오류
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

# Windows 콘솔 기본 코드페이지(cp949)에서 한글·em dash 가 깨지지 않게 한다.
for _stream in (sys.stdout, sys.stderr):
    try:
        _stream.reconfigure(encoding="utf-8")
    except (AttributeError, OSError):  # 파이프·리다이렉트 등
        pass

# 계약 §1의 항목 행: 표의 첫 칸이 SC-번호다.
CONTRACT_ROW = re.compile(r"^\|\s*\*{0,2}SC-(\d+)\*{0,2}\s*\|", re.M)
# 리포트의 판정표 행: 첫 칸이 SC-번호. 본문 산문 속 언급은 세지 않는다 —
# 판정은 표로 하기 때문이고, 산문까지 세면 "다음 라운드에 볼 것" 같은 참조가 위반으로 잡힌다.
REPORT_ROW = re.compile(r"^\|\s*\*{0,2}(SC-\d+)")


def contract_items(text: str) -> set[int]:
    return {int(m) for m in CONTRACT_ROW.findall(text)}


def judged_items(text: str) -> set[int]:
    found: set[int] = set()
    for line in text.splitlines():
        m = REPORT_ROW.match(line.strip())
        if m:
            found.add(int(m.group(1).removeprefix("SC-")))
    return found


def check(contract_text: str, report_texts: list[str]) -> tuple[set[int], set[int], set[int]]:
    """(계약 항목, 판정된 항목, 위반=집 없이 판정된 항목)"""
    in_contract = contract_items(contract_text)
    judged: set[int] = set()
    for t in report_texts:
        judged |= judged_items(t)
    return in_contract, judged, judged - in_contract


def selftest() -> int:
    """**양방향**: 알려진 위반을 잡고, 깨끗한 입력을 통과시킨다 (계약 §3.3)."""
    contract = "| SC-01 | a | b |\n| **SC-86** | a | b |\n| SC-88 | a | b |\n"
    cases = [
        (
            "위반 — 리포트가 SC-87을 판정했는데 계약에 없다 (p1-01 실제 사례)",
            ["| **SC-86** | cmd | ok | **PASS** |\n| SC-87 | cmd | ok | **PASS** |"],
            {87},
        ),
        (
            "깨끗 — 판정된 것이 전부 계약에 있다",
            ["| SC-01 | cmd | ok | **PASS** |\n| **SC-86** | cmd | ok | **PASS** |"],
            set(),
        ),
        (
            "순방향은 위반이 아니다 — 계약에 있으나 이번 라운드에 안 나온 항목(대기)",
            ["| SC-01 | cmd | ok | **PASS** |"],
            set(),
        ),
        (
            "산문 속 언급은 세지 않는다 — 'SC-99는 다음 라운드에' 같은 참조",
            ["SC-99 는 다음 라운드에 본다. | SC-01 | cmd | ok | **PASS** |"],
            set(),
        ),
        (
            "리포트 여러 개를 합쳐 본다",
            ["| SC-01 | a | b | **PASS** |", "| SC-87 | a | b | **PASS** |"],
            {87},
        ),
    ]
    failures = 0
    for name, reports, expected in cases:
        _, _, violations = check(contract, reports)
        ok = violations == expected
        print(f"{'OK  ' if ok else 'FAIL'} {name} :: 위반={sorted(violations)} 기대={sorted(expected)}")
        if not ok:
            failures += 1
    print(f"selftest: {'PASS' if failures == 0 else f'FAIL ({failures})'}  케이스={len(cases)}")
    return 0 if failures == 0 else 1


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--contract")
    ap.add_argument("--report", action="append", default=[])
    ap.add_argument("--selftest", action="store_true")
    args = ap.parse_args()

    if args.selftest:
        return selftest()
    if not args.contract or not args.report:
        ap.print_usage()
        print("오류: --contract 와 --report 가 필요하다 (또는 --selftest)", file=sys.stderr)
        return 2

    contract_text = Path(args.contract).read_text(encoding="utf-8")
    report_texts = [Path(p).read_text(encoding="utf-8") for p in args.report]
    in_contract, judged, violations = check(contract_text, report_texts)

    print(f"계약 항목: {len(in_contract)}")
    print(f"리포트가 판정표 행으로 다룬 항목: {len(judged)}")
    print(f"계약에 있으나 이 리포트들에 안 나온 항목: {len(in_contract - judged)} (위반 아님 — 대기·블록 분할)")
    if violations:
        print()
        print("!! 위반 — 계약 표에 집이 없는 채로 판정된 항목:")
        for n in sorted(violations):
            print(f"   SC-{n}")
        print()
        print("계약 머리: \"Phase 5 평가는 이 표의 항목으로만 한다.\"")
        print("합격 기준이 리포트 본문에만 있으면 다음 사람이 무엇과 대조할지 알 수 없다.")
        return 1
    print("위반 없음 — 판정된 항목이 전부 계약 표에 있다.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

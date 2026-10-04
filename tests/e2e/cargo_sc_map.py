"""계약 §1 행의 `cargo test -p <crate> <필터>` 를 한 번의 `cargo test` 로그와 맞대어, SC 마다 **매칭된 테스트
이름·결과**를 찍는다 — **준비·집계 도구이고 verdict 를 내지 않는다**(판정은 리포트가 계약 문구로 한다).

    python tests/e2e/cargo_sc_map.py --contract <계약> --log <cargo test 로그> [--json <출력>]

매칭 규칙: 필터 문자열(`*` 는 임의 문자열)이 테스트 경로(`test <경로> ... ok`)에 **부분 문자열로** 들어가면 매칭.
매칭 0 인 필터는 따로 찍는다 — "테스트가 없다" 와 "테스트가 초록이다" 를 섞지 않는다(CLAUDE.md 검증의 규율).
로그의 테스트 줄은 크레이트를 적지 않으므로 `-p` 는 기록만 하고 매칭에 쓰지 않는다(이름 충돌은 출력에 그대로 보인다).
"""
from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

for _s in (sys.stdout, sys.stderr):
    try:
        _s.reconfigure(encoding="utf-8")
    except (AttributeError, ValueError):
        pass

ROW = re.compile(r"^\|\s*\*{0,2}SC-(\d+)\*{0,2}\s*\|")
CARGO = re.compile(r"`cargo test -p ([\w-]+)(?: --[\w-]+)* ([^`\s]+)`")
TEST_LINE = re.compile(r"^test (\S+)(?: - should panic)? \.\.\. (ok|FAILED|ignored)")


def contract_filters(text: str) -> dict[int, list[tuple[str, str]]]:
    out: dict[int, list[tuple[str, str]]] = {}
    section = None
    for line in text.splitlines():
        if line.startswith("## "):
            section = line[3:].strip()
        m = ROW.match(line)
        if not m or not (section and section.startswith("1.")):
            continue
        for crate, filt in CARGO.findall(line):
            if filt.startswith("--"):      # 플래그(--locked·--release)는 필터가 아니다
                continue
            out.setdefault(int(m.group(1)), []).append((crate, filt))
    return out


def log_results(log: str) -> list[tuple[str, str]]:
    return [(m.group(1), m.group(2)) for m in (TEST_LINE.match(l) for l in log.splitlines()) if m]


def match(filt: str, results: list[tuple[str, str]]) -> list[tuple[str, str]]:
    rx = re.compile(".*".join(re.escape(p) for p in filt.split("*")))
    return [(n, st) for n, st in results if rx.search(n)]


def build(text: str, log: str) -> dict:
    res = log_results(log)
    rows = {}
    for sc, fl in sorted(contract_filters(text).items()):
        items = []
        for crate, f in fl:
            m = match(f, res)
            items.append({"crate": crate, "filter": f, "matched": len(m),
                          "ok": sum(1 for _, s in m if s == "ok"),
                          "failed": [n for n, s in m if s == "FAILED"],
                          "ignored": [n for n, s in m if s == "ignored"],
                          "names": [n for n, _ in m][:12]})
        rows[f"SC-{sc:02d}"] = items
    return {"log_test_lines": len(res), "rows": rows,
            "zero_match_filters": [(k, i["filter"]) for k, v in rows.items() for i in v if i["matched"] == 0]}


def selftest() -> int:
    text = ("## 1. 항목\n| SC-05 | x | `cargo test -p starfall-game-server data::tests::reject_c` + `cargo test -p x none_here` | a |\n"
            "| SC-20 | y | `cargo test -p starfall-persistence batch_recommit_is_noop` [DB] | b |\n")
    log = ("test data::tests::reject_c01_x ... ok\ntest data::tests::reject_c02_y ... FAILED\n"
           "test batch_recommit_is_noop ... ok\ntest tests::refuses_x - should panic ... ok\n")
    r = build(text, log)
    cases = [
        ("SC-05 필터가 두 테스트에 매칭, 실패 1 을 이름으로", r["rows"]["SC-05"][0]["matched"] == 2
         and r["rows"]["SC-05"][0]["failed"] == ["data::tests::reject_c02_y"]),
        ("매칭 0 필터가 따로 찍힘", ("SC-05", "none_here") in r["zero_match_filters"]),
        ("SC-20 정확 이름 매칭", r["rows"]["SC-20"][0]["ok"] == 1),
        ("should panic 줄도 테스트로 읽는다", any(n == "tests::refuses_x" for n, _ in log_results(log))),
    ]
    bad = 0
    for name, ok in cases:
        print(("OK   " if ok else "FAIL ") + name)
        bad += 0 if ok else 1
    print(f"selftest: {'PASS' if not bad else f'FAIL ({bad})'}  케이스={len(cases)}")
    return 0 if not bad else 1


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("mode", nargs="?", choices=["selftest"])
    ap.add_argument("--contract")
    ap.add_argument("--log")
    ap.add_argument("--json")
    a = ap.parse_args()
    if a.mode == "selftest":
        return selftest()
    r = build(Path(a.contract).read_text(encoding="utf-8"), Path(a.log).read_text(encoding="utf-8", errors="replace"))
    text = json.dumps(r, ensure_ascii=False, indent=2)
    if a.json:
        Path(a.json).write_text(text, encoding="utf-8")
    print(f"로그 테스트 줄 {r['log_test_lines']} · SC {len(r['rows'])} · 매칭 0 필터 {len(r['zero_match_filters'])}")
    for k, f in r["zero_match_filters"]:
        print(f"  매칭 0: {k} {f}")
    return 0


if __name__ == "__main__":
    sys.exit(main())

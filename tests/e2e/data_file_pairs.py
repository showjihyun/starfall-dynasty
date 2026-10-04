"""p1-02 계약 SC-07 (AC-19 c2) — **읽히지 않는 데이터 파일 0.**

`data/**/*.json` 각 파일마다 두 짝이 있어야 한다:
  ① **읽는 코드** — 서버 기동 로그의 `데이터 파일 적재` 줄에 그 파일의 `data_file=<data 기준 상대 경로>`
     (server Q-5 답, S2). **실행 증거로만 채운다** — 소스 grep 으로 채우지 않는다(규칙 9: 소스는
     긍정을 증명하지 못한다).
  ② **검사하는 게이트** — (a) `validate_data_files.py` 의 `TARGETS` glob 이 그 파일을 덮는다 **그리고**
     (b) 유도값·golden 테스트가 `--cargo-log` 에서 `ok` 로 찍혔다(아래 `DERIVED_GATES`).

짝이 하나라도 없는 파일이 있으면 FAIL — **틀려도 아무것도 실패하지 않는 파일**이다.
분모(파일 수)는 같은 실행에서 `find data -name '*.json'` 과 같은 방법으로 센다.

    python tests/e2e/data_file_pairs.py --boot-log <서버 로그> --cargo-log <cargo test 로그>
    python tests/e2e/data_file_pairs.py selftest

`--cargo-log` 가 없으면 ② (b) 를 볼 수 없으므로 PASS 를 내지 않는다(미검증(증거 요건)).
"""

from __future__ import annotations

import argparse
import fnmatch
import json
import re
import sys
from pathlib import Path

import db
import validate_data_files as VDF

LABEL = {"item": "p1-02 SC-07 (AC-19c2) 읽히지 않는 데이터 파일 0 — 파일마다 읽는 코드 × 게이트"}

# 서버 로그 형식은 pretty·json 둘 다 받는다. 메시지 문구는 server 가 Q-5 에서 정했다.
LOAD_LINE = re.compile(r"데이터 파일 적재")
DATA_FILE = re.compile(r"data_file[\"']?\s*[=:]\s*[\"']?([^\s\"',}]+)")
CARGO_OK = re.compile(r"^test (\S+) \.\.\. ok\s*$", re.M)

# data 기준 glob → 그 파일의 유도값·golden 게이트가 되는 cargo 테스트 이름 패턴(fnmatch).
# 계약 §2 표의 ② 칸을 옮긴 것이다. 손으로 유지하는 표이므로 **패턴이 아무 테스트도 못 맞추면
# 그 자체가 FAIL** 로 드러난다(패턴이 낡으면 조용히 통과하지 않는다).
DERIVED_GATES = [
    ("ships/*.json", "*data::*"),
    ("world/systems/*.json", "*data::*"),
    ("movement/sync-tuning.json", "*data::*"),
    ("minerals/*.json", "*data::*reject_c*"),
    ("world/deposits/*.json", "*data::*reject_c*"),
    ("mining/mining-rules.json", "*data::*reject_c*"),
    ("history/rules/*.json", "*rule_golden*"),
]


def data_files(root: Path) -> list[str]:
    return sorted(p.relative_to(root / "data").as_posix() for p in (root / "data").rglob("*.json"))


def loaded_from_log(text: str) -> set[str]:
    out = set()
    for line in text.splitlines():
        if LOAD_LINE.search(line):
            m = DATA_FILE.search(line)
            if m:
                out.add(m.group(1).replace("\\", "/"))
    return out


def schema_covered(rel: str) -> bool:
    return any(fnmatch.fnmatch("data/" + rel, g) for g, _ in VDF.TARGETS)


def derived_ok(rel: str, passed_tests: set[str]) -> tuple[str | None, bool]:
    for g, pat in DERIVED_GATES:
        if fnmatch.fnmatch(rel, g):
            return pat, any(fnmatch.fnmatch(t, pat) for t in passed_tests)
    return None, False


def judge(files: list[str], loaded: set[str], passed_tests: set[str] | None) -> dict:
    rows, unpaired = [], []
    for f in files:
        pat, dok = derived_ok(f, passed_tests or set())
        row = {"file": f, "read_by_server_log": f in loaded, "schema_gate": schema_covered(f),
               "derived_gate_pattern": pat, "derived_gate_passed": dok if passed_tests is not None else None}
        paired = row["read_by_server_log"] and row["schema_gate"] and (dok if passed_tests is not None else False)
        row["paired"] = paired
        rows.append(row)
        if not paired:
            unpaired.append(f)
    ghost_loaded = sorted(loaded - set(files))   # 로그엔 있는데 파일이 없다 — 경로 표기 불일치 신호
    return {"files_denominator": len(files), "paired": len(files) - len(unpaired), "unpaired": unpaired,
            "loaded_lines": len(loaded), "loaded_but_not_a_data_file": ghost_loaded,
            "cargo_log_given": passed_tests is not None, "rows": rows,
            "ok": bool(files) and not unpaired and not ghost_loaded and passed_tests is not None}


def run(args: argparse.Namespace) -> int:
    root = db.REPO_ROOT
    files = data_files(root)
    loaded = loaded_from_log(Path(args.boot_log).read_text(encoding="utf-8", errors="replace"))
    passed = (set(CARGO_OK.findall(Path(args.cargo_log).read_text(encoding="utf-8", errors="replace")))
              if args.cargo_log else None)
    res = judge(files, loaded, passed)
    out = dict(LABEL)
    out["verdict"] = ("PASS" if res["ok"] else
                      "미검증(증거 요건) — --cargo-log 없음" if not res["cargo_log_given"] else
                      "FAIL(분모 0)" if not files else "FAIL")
    out.update(res)
    db.emit(out, args.evidence)
    return db.EXIT_OK if res["ok"] else db.EXIT_FAIL


def selftest() -> int:
    files = ["ships/a.json", "minerals/x.json", "history/rules/r.json"]
    log = "\n".join(f'INFO 데이터 파일 적재 data_file={f}' for f in files)
    # 실제 cargo 출력 모양(`data::tests::reject_c01_…`, `golden::rule_golden`)을 쓴다 — 처음 쓴 합성 이름
    # (`game_server::data::reject_c01`)은 실제와 달라 패턴 결함(`*data::reject_c*`)을 가렸다(qa 규칙 3 실례).
    tests = {"data::tests::reject_c01_x", "golden::rule_golden", "data::tests::loads"}
    cases = [
        ("모두 짝 → PASS", judge(files, loaded_from_log(log), tests)["ok"], True),
        ("json 로그 형식도 읽는다",
         loaded_from_log('{"message":"데이터 파일 적재","data_file":"minerals/x.json"}') == {"minerals/x.json"}, True),
        ("음성 대조: 아무도 안 읽는 unused/x.json → FAIL",
         judge(files + ["unused/x.json"], loaded_from_log(log), tests)["unpaired"] == ["unused/x.json"], True),
        ("음성 대조: 로그에 한 파일 빠짐 → FAIL", judge(files, loaded_from_log(log.replace("data_file=minerals/x.json", "")), tests)["ok"], False),
        ("음성 대조: golden 테스트가 안 돌았음 → 규칙 파일 짝 없음",
         judge(files, loaded_from_log(log), tests - {"golden::rule_golden"})["unpaired"] == ["history/rules/r.json"], True),
        ("cargo 로그 없으면 PASS 를 내지 않는다", judge(files, loaded_from_log(log), None)["ok"], False),
        ("분모 0 → FAIL", judge([], set(), tests)["ok"], False),
    ]
    fails = 0
    for name, got, want in cases:
        ok = got == want
        print(f"{'OK  ' if ok else 'FAIL'} {name} :: {got} 기대={want}")
        fails += 0 if ok else 1
    # 실제 레포 분모 — 이 selftest 가 도는 트리의 data/ 파일 수를 같이 찍는다(오늘 10)
    print(f"참고: 이 트리의 data/**/*.json = {len(data_files(db.REPO_ROOT))}")
    print(f"selftest: {'PASS' if fails == 0 else f'FAIL ({fails})'}  케이스={len(cases)}")
    return 0 if fails == 0 else 1


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("mode", nargs="?", choices=["selftest"])
    ap.add_argument("--boot-log")
    ap.add_argument("--cargo-log")
    ap.add_argument("--evidence")
    args = ap.parse_args()
    if args.mode == "selftest":
        return selftest()
    if not args.boot_log:
        ap.error("--boot-log <서버 기동 로그> 가 필요하다")
    return db.main_guard(lambda: run(args))


if __name__ == "__main__":
    sys.exit(main())

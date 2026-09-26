#!/usr/bin/env python
"""SV-1 재판정 — `graceful_client_close_is_prompt` 를 **CPU 부하 아래에서** 반복하고 옛 형태·새 형태를 나란히 본다.

architect R3 판정 1: 재판정 증거는 "연속 N회 통과"가 아니라 **"터뜨리던 조건을 만들었는데 안 터진다"** 이다.
원래 flaky 를 터뜨린 조건은 극단적 과다구독이 아니라 **워크스페이스 병렬 + Unity 빌드** — 코어 수의 몇 배
수준의 경합이다. 그래서 배수를 **0·1·2·4·8** 처럼 현실 구간부터 잡는다.

부하는 `python -c "while True: pass"` 프로세스 (코어 수 × 배수) 개. 서버 코드·테스트 코드를 고치지 않고,
**이미 빌드된** 테스트 바이너리를 직접 실행한다(측정 중 빌드 금지, 계약 G-c).
테스트가 찍는 줄 `폴 N회 / 상한 M). 벽시계 S초` 에서 폴 횟수와 벽시계를 읽는다.

    python tests/e2e/sv1_load_repro.py --test-exe <ws_integration-*.exe> --multipliers 0,1,2,4,8 --runs 10 --out <json>
"""
from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
import time

for _stream in (sys.stdout, sys.stderr):
    try:
        _stream.reconfigure(encoding="utf-8")  # type: ignore[union-attr]
    except (AttributeError, ValueError):
        pass

LINE = re.compile(r"폴 (\d+)회 / 상한 (\d+)\)\. 벽시계 ([\d.]+)초")
OLD_BUDGET_S = 2.0   # 옛 형태 `elapsed < 2s`
NEW_BUDGET_POLLS = 10  # 새 형태 `polls <= GRACEFUL_CLOSE_POLL_BUDGET`


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--test-exe", required=True)
    ap.add_argument("--test", default="graceful_client_close_is_prompt")
    ap.add_argument("--multipliers", default="0,1,2,4,8")
    ap.add_argument("--runs", type=int, default=10)
    ap.add_argument("--warmup-s", type=float, default=2.0)
    ap.add_argument("--out", required=True)
    ap.add_argument("--full-suite", action="store_true",
                    help="필터 없이 바이너리 전체를 기본 병렬로 — **원래 flaky 가 난 조건**(스위트 병렬)")
    args = ap.parse_args()
    cores = os.cpu_count() or 1
    results = []
    for k in [int(x) for x in args.multipliers.split(",")]:
        hogs = [subprocess.Popen([sys.executable, "-c", "while True: pass"],
                                 stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
                for _ in range(cores * k)]
        time.sleep(args.warmup_s if hogs else 0)
        rows = []
        try:
            for i in range(args.runs):
                t = time.monotonic()
                cmd = ([args.test_exe, "--nocapture"] if args.full_suite
                       else [args.test_exe, args.test, "--exact", "--nocapture", "--test-threads", "1"])
                r = subprocess.run(cmd,
                                   capture_output=True, text=True, encoding="utf-8", errors="replace",
                                   timeout=120)
                m = LINE.search(r.stdout)
                polls = int(m.group(1)) if m else None
                elapsed = float(m.group(3)) if m else None
                rows.append({
                    "run": i, "test_rc": r.returncode, "polls": polls, "elapsed_s": elapsed,
                    "old_form_pass": elapsed is not None and elapsed < OLD_BUDGET_S,
                    "new_form_pass": polls is not None and polls <= NEW_BUDGET_POLLS,
                    "wall_s": round(time.monotonic() - t, 2),
                })
                print(f"x{k} run{i}: rc={r.returncode} polls={polls} elapsed={elapsed}", flush=True)
        finally:
            for h in hogs:
                h.kill()
            for h in hogs:
                h.wait()
        parsed = [r for r in rows if r["polls"] is not None]
        results.append({
            "multiplier": k, "hog_processes": cores * k, "runs": len(rows), "parsed": len(parsed),
            "polls": [r["polls"] for r in rows], "elapsed_s": [r["elapsed_s"] for r in rows],
            "old_form_fail": sum(1 for r in parsed if not r["old_form_pass"]),
            "new_form_fail": sum(1 for r in parsed if not r["new_form_pass"]),
            "test_rc_nonzero": sum(1 for r in rows if r["test_rc"] != 0),
            "rows": rows,
        })
    doc = {"full_suite": args.full_suite, "cores": cores, "old_budget_s": OLD_BUDGET_S, "new_budget_polls": NEW_BUDGET_POLLS, "results": results}
    with open(args.out, "w", encoding="utf-8") as f:
        json.dump(doc, f, ensure_ascii=False, indent=2)
    for r in results:
        print(f"x{r['multiplier']:>2} ({r['hog_processes']} hogs): polls={r['polls']} "
              f"elapsed max={max((e for e in r['elapsed_s'] if e is not None), default=None)} "
              f"old FAIL {r['old_form_fail']}/{r['parsed']} new FAIL {r['new_form_fail']}/{r['parsed']} rc!=0 {r['test_rc_nonzero']}")
    return 0


if __name__ == "__main__":
    sys.exit(main())

#!/usr/bin/env python3
"""Performance budget gate (docs/STUDIO_PIPELINE.md): fails when the game got slower than the last
accepted numbers on this machine.

    python tools/perfcheck.py             # sim bench (CPU, headless)
    python tools/perfcheck.py --client    # + client --bench (GPU; opens a window ~25 s per run)
    python tools/perfcheck.py --update    # accept the current numbers as this machine's baseline

Each bench runs RUNS times and the best run counts (other programs only ever add time); the client
bench also throws away one warm-up run (the first run after a build is slow while shaders compile).
A metric fails above baseline x (1 + TOLERANCE[sim|client]). Baselines are per machine in tools/perf_baseline.json
(times from different hardware mean nothing), so a machine without one, like CI, reports SKIP.
Update the baseline only in the commit that makes the game faster, or slower on purpose with a reason.
"""
import json
import os
import platform
import re
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
BASELINE = os.path.join(ROOT, "tools", "perf_baseline.json")
EXE = ".exe" if os.name == "nt" else ""
RUNS = 3
# Allowed slowdown per metric before it fails. Measured 2026-10-20 on the dev PC, best of 3 against best
# of 3: sim within 7%, client frame time within 17% (GPU clocks, window focus), so the client gets more room.
TOLERANCE = {"sim": 0.20, "client": 0.25}


def run(cmd, env=None):
    r = subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True, encoding="utf-8", errors="replace", env={**os.environ, **(env or {})})
    return r.stdout + r.stderr


def sim_bench():
    out = run([os.path.join(ROOT, "target", "release", "examples", "bench" + EXE), "1000"])
    m = re.search(r"tick avg ([\d.]+) ms, p95 ([\d.]+) ms", out)
    if not m:
        raise RuntimeError("sim bench printed no tick line:\n" + out[-1500:])
    return {"sim_tick_ms": float(m.group(1)), "sim_tick_p95_ms": float(m.group(2))}


def client_bench():
    out = run([os.path.join(ROOT, "target", "release", "client" + EXE), "--bench", "15"], {"STARFALL_BENCH_SCALE": "15"})
    m = re.search(r"BENCH frames \d+ \| avg ([\d.]+) ms .*?\| 1% low ([\d.]+) ms", out)
    if not m:
        raise RuntimeError("client bench printed no BENCH line:\n" + out[-1500:])
    return {"client_frame_ms": float(m.group(1)), "client_1pct_low_ms": float(m.group(2))}


def best(fn, runs, warmup=0):
    for _ in range(warmup):
        fn()
    results = [fn() for _ in range(runs)]
    return {k: min(r[k] for r in results) for k in results[0]}


def machine():
    # Hardware only, no host name: the baseline file is published with the repository.
    return f"{platform.system()} | {platform.processor() or platform.machine()} | {os.cpu_count()} threads"


def main(argv):
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
    builds = [["cargo", "build", "-q", "--release", "-p", "sim", "--example", "bench"]]
    if "--client" in argv:
        builds.append(["cargo", "build", "-q", "--release", "-p", "client"])
    for b in builds:
        if subprocess.run(b, cwd=ROOT).returncode != 0:
            print("PERF FAIL: build failed")
            return 1
    now = best(sim_bench, RUNS)
    if "--client" in argv:
        now.update(best(client_bench, RUNS, warmup=1))
    key = machine()
    data = json.load(open(BASELINE, encoding="utf-8")) if os.path.exists(BASELINE) else {}
    base = data.get(key, {}).get("metrics", {})
    failed = 0
    print(f"machine: {key}")
    for k, v in now.items():
        if k not in base:
            print(f"  {k:22} {v:8.2f} ms   (no baseline)")
            continue
        limit = base[k] * (1 + TOLERANCE[k.split("_")[0]])
        bad = v > limit
        failed += bad
        print(f"  {k:22} {v:8.2f} ms   baseline {base[k]:.2f}, limit {limit:.2f}  {'FAIL' if bad else 'ok'}")
    if "--update" in argv:
        entry = data.setdefault(key, {"metrics": {}})
        entry["metrics"].update(now)
        with open(BASELINE, "w", encoding="utf-8") as f:
            json.dump(data, f, indent=1, ensure_ascii=False)
            f.write("\n")
        print("baseline updated for this machine")
        return 0
    if not base:
        print("PERF SKIP: no baseline for this machine (python tools/perfcheck.py --update to record one)")
        return 0
    print("PERF OK" if not failed else f"PERF FAIL: {failed} metric(s) slower than allowed")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))

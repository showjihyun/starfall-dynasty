#!/usr/bin/env python3
"""One verification gate for every change (docs/STUDIO_PIPELINE.md): looks at what changed, runs the
checks CLAUDE.md asks for that change, and prints one summary — the studio's "green build" before a
commit, for people and AI agents alike.

    python tools/gate.py              # changes since HEAD (staged, unstaged, untracked)
    python tools/gate.py --base REF   # changes since REF (e.g. a branch point)
    python tools/gate.py --all        # every headless check, whatever changed
    python tools/gate.py --visual     # also run artcheck when visuals changed (opens client windows)
    python tools/gate.py --list       # only print which checks would run

Headless by default (no game windows): artcheck is listed as TODO for visual changes unless
--visual is given (artcheck needs a GPU window; run it only in rounds that change the screen).
Writes target/gate/last.json. Exit 1 if any check failed.
"""
import json
import os
import subprocess
import sys
import time

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
CLIPPY_BASELINE = 43  # warning sites, whole workspace, 2026-10-20; lower it when you clear some, never raise it silently


def sh(cmd, **kw):
    """Runs one of the fixed check commands below (never user input); a list runs without a shell."""
    return subprocess.run(cmd, cwd=ROOT, shell=isinstance(cmd, str), capture_output=True, text=True, encoding="utf-8", errors="replace", **kw)


def changed_files(base):
    out = sh(["git", "diff", "--name-only", base, "--"]).stdout.splitlines()
    out += sh(["git", "ls-files", "--others", "--exclude-standard"]).stdout.splitlines()
    return sorted(set(f.strip() for f in out if f.strip()))


def any_in(files, *prefixes):
    return any(f.startswith(p) for f in files for p in prefixes)


# (name, why it runs, command, success test). Commands are the ones CLAUDE.md documents.
def plan(files, everything, visual):
    rust_logic = everything or any_in(files, "crates/shared/", "crates/history/", "crates/sim/", "crates/view/", "tuning/", "Cargo.")
    rules = everything or any_in(files, "crates/shared/", "crates/sim/", "tuning/")
    client = everything or any_in(files, "crates/client/", "crates/view/", "crates/shared/", "Cargo.")
    network = everything or any_in(files, "crates/server/", "crates/bot/", "crates/shared/src/protocol", "crates/sim/src/game/")
    any_rust = everything or any_in(files, "crates/", "Cargo.")
    assets = everything or any_in(files, "crates/client/assets/models/", "tools/asset", "tools/blender/")
    looks = any_in(files, "crates/client/src/", "crates/client/assets/", "crates/view/src/palette", "tools/blender/", "docs/art/")
    steps = []
    if rust_logic:
        steps.append(("logic tests", "rules/view code or tuning changed", "cargo test --release -p shared -p history -p sim -p view", None))
    if rules:
        steps.append(("balance sheet in sync", "rules or numbers changed: docs/BALANCE.md must be regenerated", "BALANCE", None))
    if client:
        steps.append(("client ECS tests", "client glue changed", "cargo test --release -p client", None))
    if network:
        steps.append(("network + bot tests", "server/protocol/game changed", "SERVER", None))
    if any_rust:
        steps.append(("clippy (no new warnings)", f"baseline {CLIPPY_BASELINE}", "cargo clippy --release --workspace --all-targets", "CLIPPY"))
    if assets:
        steps.append(("asset budgets", "models or budgets changed", "python tools/assetcheck.py", None))
    if looks:
        if visual:
            steps.append(("artcheck", "rendering changed", "python tools/artcheck.py", None))
        else:
            steps.append(("artcheck", "rendering changed — run `python tools/gate.py --visual` (opens windows)", "TODO", None))
    return steps


def run_step(cmd, test):
    if cmd == "TODO":
        return "TODO", ""
    if cmd == "BALANCE":
        tmp = os.path.join(ROOT, "target", "gate", "BALANCE.md")
        os.makedirs(os.path.dirname(tmp), exist_ok=True)
        r = sh(["cargo", "run", "-q", "-p", "sim", "--release", "--example", "balance", "--", "--write", tmp])
        if r.returncode != 0:
            return "FAIL", r.stderr[-2000:]
        same = open(tmp, encoding="utf-8").read() == open(os.path.join(ROOT, "docs", "BALANCE.md"), encoding="utf-8").read()
        return ("PASS", "") if same else ("FAIL", "docs/BALANCE.md is stale: cargo run -p sim --release --example balance -- --write docs/BALANCE.md, then read the diff")
    if cmd == "SERVER":
        # Windows locks a running server binary (CLAUDE.md): stop it before rebuilding.
        sh("taskkill /F /IM server.exe" if os.name == "nt" else "pkill -x server")
        r = sh("cargo build --release -p server")
        if r.returncode != 0:
            return "FAIL", r.stderr[-2000:]
        cmd = "cargo test --release -p bot"
    r = sh(cmd)
    text = r.stdout + r.stderr
    if test == "CLIPPY":
        warnings = sorted(set(l.strip() for l in text.splitlines() if l.strip().startswith("-->")))
        if r.returncode != 0:
            return "FAIL", text[-2000:]
        if len(warnings) > CLIPPY_BASELINE:
            return "FAIL", f"{len(warnings)} warning sites > baseline {CLIPPY_BASELINE}"
        return "PASS", f"{len(warnings)} warning sites (baseline {CLIPPY_BASELINE})"
    if r.returncode != 0:
        fails = [l for l in text.splitlines() if "FAIL" in l or "panicked" in l or l.startswith("error")]
        return "FAIL", "\n".join(fails[:20]) or text[-2000:]
    return "PASS", ""


def main(argv):
    # Korean Windows consoles default to cp949; check output carries Korean and symbols.
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
    base =argv[argv.index("--base") + 1] if "--base" in argv else "HEAD"
    everything, visual = "--all" in argv, "--visual" in argv
    files = changed_files(base)
    steps = plan(files, everything, visual)
    print(f"gate: {len(files)} changed file(s) since {base}{' (--all)' if everything else ''}")
    if not steps:
        print("nothing to check (docs-only change)")
        return 0
    for name, why, cmd, _ in steps:
        print(f"  - {name:26} {why}")
    if "--list" in argv:
        return 0
    report, failed = [], 0
    for name, why, cmd, test in steps:
        t0 = time.time()
        print(f"\n>> {name} ...", flush=True)
        verdict, detail = run_step(cmd, test)
        secs = time.time() - t0
        failed += verdict == "FAIL"
        print(f"  {verdict} ({secs:.0f} s){' — ' + detail if detail and verdict != 'FAIL' else ''}")
        if verdict == "FAIL":
            print("    " + detail.replace("\n", "\n    "))
        report.append({"check": name, "why": why, "verdict": verdict, "secs": round(secs, 1), "detail": detail})
    os.makedirs(os.path.join(ROOT, "target", "gate"), exist_ok=True)
    with open(os.path.join(ROOT, "target", "gate", "last.json"), "w", encoding="utf-8") as f:
        json.dump({"base": base, "files": files, "steps": report}, f, ensure_ascii=False, indent=1)
    print("\n" + "\n".join(f"{r['verdict']:4} {r['check']}" for r in report))
    print("GATE OK" if not failed else f"GATE FAIL: {failed}")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))

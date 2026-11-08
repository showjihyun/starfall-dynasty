#!/usr/bin/env python3
"""Release package (docs/STUDIO_PIPELINE.md "배포·패키징"): the folder a player installs — and the
content root of a Steam depot — built from the release binaries, the assets and the tuning.

    python tools/package.py                         # dist/StarfallDynasty/
    python tools/package.py --server wss://host:443 # the server the game connects to by default
    python tools/package.py --zip                   # + dist/StarfallDynasty-<commit>.zip
    python tools/package.py --smoke                 # + start the packaged game and server once (opens a window)
    python tools/package.py --no-build              # reuse target/release

Layout:
    StarfallDynasty.exe        the game (client)
    server.txt                 default server address (`--server` overrides it)
    assets/                    models, music, sound, voice, cinematics (+ their CREDITS)
    tuning/                    gameplay numbers (the server sends its own to clients on login)
    server/server.exe          dedicated server, for hosts
    THIRD_PARTY.txt            Rust crates shipped and their licenses
Steam: tools/steam/*.vdf point at this folder (fill in the app and depot ids, then steamcmd +run_app_build).
"""
import json
import os
import shutil
import socket
import subprocess
import sys
import time

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
EXE = ".exe" if os.name == "nt" else ""
NAME = "StarfallDynasty"


def run(cmd, **kw):
    print("$", " ".join(cmd))
    r = subprocess.run(cmd, cwd=kw.pop("cwd", ROOT), **kw)
    if r.returncode != 0:
        sys.exit(f"failed: {' '.join(cmd)}")
    return r


def third_party(out_path):
    """Every crate in the client and server builds, with its license (cargo metadata)."""
    meta = json.loads(subprocess.run(["cargo", "metadata", "--format-version", "1", "--locked"], cwd=ROOT, capture_output=True, text=True, encoding="utf-8").stdout)
    ours = {p["id"] for p in meta["packages"] if p.get("source") is None}
    rows = sorted({(p["name"], p["version"], p.get("license") or "see crate") for p in meta["packages"] if p["id"] not in ours})
    with open(out_path, "w", encoding="utf-8") as f:
        f.write("STARFALL DYNASTY — third-party software\n\n")
        f.write("Rust crates compiled into the game and server (name, version, license):\n\n")
        for n, v, lic in rows:
            f.write(f"{n} {v} — {lic}\n")
        f.write("\nMusic, sound effects and voice: see assets/music/CREDITS.md, assets/sfx/CREDITS.md, assets/voice/CREDITS.md.\n")
    return len(rows)


def smoke(pkg):
    """The packaged game renders its opening from its own folder; the packaged server accepts connections."""
    shot = os.path.join(pkg, "smoke.png")
    env = {**os.environ, "STARFALL_FIXED_FPS": "60"}
    env.pop("STARFALL_TUNING", None)
    subprocess.run([os.path.join(pkg, NAME + EXE), "--cinematic", "opening", "--shot", shot, "--shot-at", "4"], cwd=pkg, env=env, capture_output=True, timeout=180)
    ok_shot = os.path.exists(shot) and os.path.getsize(shot) > 50_000
    port = 7799
    srv = subprocess.Popen([os.path.join(pkg, "server", "server" + EXE), "--port", str(port), "--data", os.path.join(pkg, "smoke_data")], cwd=os.path.join(pkg, "server"), stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    ok_srv = False
    for _ in range(20):
        time.sleep(0.5)
        try:
            socket.create_connection(("127.0.0.1", port), timeout=1).close()
            ok_srv = True
            break
        except OSError:
            pass
    srv.kill()
    srv.wait()
    for p in [shot, os.path.join(pkg, "smoke_data")]:
        if os.path.isdir(p):
            shutil.rmtree(p, ignore_errors=True)
        elif os.path.exists(p):
            os.remove(p)
    print(f"{'PASS' if ok_shot else 'FAIL'} packaged game renders the opening from its own folder")
    print(f"{'PASS' if ok_srv else 'FAIL'} packaged server listens")
    return ok_shot and ok_srv


def main(argv):
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
    arg = lambda k: argv[argv.index(k) + 1] if k in argv and argv.index(k) + 1 < len(argv) else None
    dist = os.path.join(ROOT, arg("--out") or "dist")
    pkg = os.path.join(dist, NAME)
    commit = subprocess.run(["git", "rev-parse", "--short", "HEAD"], cwd=ROOT, capture_output=True, text=True).stdout.strip() or "dev"
    if "--no-build" not in argv:
        # Crash reports name the build (client/src/crash.rs reads STARFALL_COMMIT at compile time).
        run(["cargo", "build", "--release", "-p", "client", "-p", "server"], env={**os.environ, "STARFALL_COMMIT": commit})
    if os.path.isdir(pkg):
        shutil.rmtree(pkg)
    os.makedirs(os.path.join(pkg, "server"))
    rel = os.path.join(ROOT, "target", "release")
    shutil.copy2(os.path.join(rel, "client" + EXE), os.path.join(pkg, NAME + EXE))
    shutil.copy2(os.path.join(rel, "server" + EXE), os.path.join(pkg, "server", "server" + EXE))
    shutil.copytree(os.path.join(ROOT, "crates", "client", "assets"), os.path.join(pkg, "assets"), ignore=shutil.ignore_patterns("*.blend", "*.blend1", "__pycache__"))
    shutil.copytree(os.path.join(ROOT, "tuning"), os.path.join(pkg, "tuning"))
    # The dedicated server finds tuning next to its own folder's parent (shared::tuning::locate walks up).
    with open(os.path.join(pkg, "server.txt"), "w", encoding="utf-8") as f:
        f.write((arg("--server") or "ws://127.0.0.1:7777") + "\n")
    crates = third_party(os.path.join(pkg, "THIRD_PARTY.txt"))
    size = sum(os.path.getsize(os.path.join(d, f)) for d, _, fs in os.walk(pkg) for f in fs)
    print(f"package: {pkg} — {size / 1e6:.0f} MB, {crates} third-party crates listed, default server {open(os.path.join(pkg, 'server.txt'), encoding='utf-8').read().strip()}")
    if "--zip" in argv:
        z = shutil.make_archive(os.path.join(dist, f"{NAME}-{commit}"), "zip", dist, NAME)
        print(f"zip: {z} — {os.path.getsize(z) / 1e6:.0f} MB")
    if "--smoke" in argv and not smoke(pkg):
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))

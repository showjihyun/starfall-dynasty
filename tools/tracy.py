"""Profiles the client with Tracy (BSD, github.com/wolfpld/tracy) without opening the profiler UI.

    python tools/tracy.py                 # 1000-ship bench (client --bench 15, STARFALL_BENCH_SCALE=15)
    python tools/tracy.py -- --showroom 7 --shot x.png --shot-at 8     # any client arguments
    python tools/tracy.py --top 60        # longer table

Builds `client --features profile` (Bevy's trace_tracy: a zone for every engine and game system),
fetches the Tracy command-line tools matching the client's bundled Tracy version into
target/tools/ (checked against a pinned SHA-256), records the run with tracy-capture, exports zones
with tracy-csvexport and prints the systems that cost the most per frame. The trace stays in
target/tracy/run.tracy for the Tracy UI (target/tools/tracy-*/tracy-profiler.exe) when a table is
not enough. Windows only for now (the pinned download is the Windows build).
"""

import hashlib
import os
import subprocess
import sys
import time
import urllib.request
import zipfile

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
# Must match the Tracy protocol bundled by tracy-client-sys in Cargo.lock (0.30.x -> Tracy 0.14.1).
VERSION = "0.14.1"
ZIP_SHA256 = "f7499d74914aa3ba94a2c1ce72f36477d7b61d9d0f7c9790e05274c258c97fb5"
TOOLS = os.path.join(ROOT, "target", "tools", f"tracy-{VERSION}")
OUT = os.path.join(ROOT, "target", "tracy")


def tools():
    exe = os.path.join(TOOLS, "tracy-capture.exe")
    if os.path.exists(exe):
        return
    if sys.platform != "win32":
        sys.exit("tracy: only the Windows tools are pinned; build tracy-capture/csvexport from source elsewhere")
    os.makedirs(TOOLS, exist_ok=True)
    z = os.path.join(TOOLS, f"windows-{VERSION}.zip")
    url = f"https://github.com/wolfpld/tracy/releases/download/v{VERSION}/windows-{VERSION}.zip"
    print(f"tracy: downloading {url}")
    urllib.request.urlretrieve(url, z)
    got = hashlib.sha256(open(z, "rb").read()).hexdigest()
    if got != ZIP_SHA256:
        os.remove(z)
        sys.exit(f"tracy: checksum mismatch ({got}), not using the download")
    zipfile.ZipFile(z).extractall(TOOLS)


def zones(csv_path):
    """(name, total_ns, count) per zone. Names may contain commas, so split from the right."""
    out = []
    for line in open(csv_path, encoding="utf-8", errors="replace").read().splitlines()[1:]:
        p = line.rsplit(",", 8)
        if len(p) == 9:
            out.append((p[0].rsplit(",", 1)[0], int(p[2]), int(p[4])))
    return out


def main(argv):
    top = int(argv[argv.index("--top") + 1]) if "--top" in argv else 35
    client_args = argv[argv.index("--") + 1:] if "--" in argv else ["--bench", "15"]
    env = dict(os.environ)
    if "--" not in argv:
        env.setdefault("STARFALL_BENCH_SCALE", "15")
    env["STARFALL_PROF"] = "0"
    tools()
    subprocess.run(["cargo", "build", "-q", "--release", "-p", "client", "--features", "profile"], cwd=ROOT, check=True)
    os.makedirs(OUT, exist_ok=True)
    trace = os.path.join(OUT, "run.tracy")
    cap = subprocess.Popen([os.path.join(TOOLS, "tracy-capture.exe"), "-o", trace, "-f"], stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
    time.sleep(1)
    client = subprocess.run([os.path.join(ROOT, "target", "release", "client.exe"), *client_args], cwd=ROOT, env=env, capture_output=True, text=True)
    log = cap.communicate(timeout=120)[0]
    for line in (client.stdout + client.stderr).splitlines():
        if line.startswith("BENCH frames"):
            print(line)
    frames = next((int(l.split(":")[1].replace(",", "")) for l in log.splitlines() if l.startswith("Frames:")), 0)
    csv_path = os.path.join(OUT, "zones.csv")
    with open(csv_path, "w", encoding="utf-8") as f:
        subprocess.run([os.path.join(TOOLS, "tracy-csvexport.exe"), trace], stdout=f, check=True)
    rows = zones(csv_path)
    if not frames:
        sys.exit("tracy: no frames captured\n" + log[-800:])
    def show(title, picked):
        print(f"\n{title} (ms per frame over {frames} frames; inclusive, threads overlap)")
        for name, total, count in sorted(picked, key=lambda r: -r[1])[:top]:
            print(f"{total / 1e6 / frames:8.3f}  x{count / frames:7.1f}  {name[:110]}")
    show("schedules", [r for r in rows if r[0].startswith(("schedule{", "sub app{", "main app", "render thread"))])
    show("systems", [r for r in rows if r[0].startswith("system{")])
    print(f"\ntrace: {trace}  zones: {csv_path}")


main(sys.argv[1:])

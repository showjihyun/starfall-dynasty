"""Generates the voice lines for the cutscenes and comms calls (docs/STORY.md §8).

    python tools/voice.py            # voice every new or changed line, drop lines no longer used
    python tools/voice.py --all      # regenerate everything (after changing a preset)
    python tools/voice.py --check    # only list what is missing

What to say comes from the game's own data (`cargo run -p view --example voice_lines` reads
opening.ron and comms.ron); presets and the pronunciation table are crates/client/assets/voice/voices.json.
Speech is MeloTTS (MIT) in Docker (tools/voice/Dockerfile, image starfall-voice:1), then ffmpeg gives
each preset its pitch and effect. Output: crates/client/assets/voice/*.ogg and manifest.json, which
`cargo test -p view --test voice` checks against the subtitles.
"""

import hashlib
import json
import os
import shutil
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
ASSETS = os.path.join(ROOT, "crates", "client", "assets", "voice")
WORK = os.path.join(ROOT, "target", "voice")
IMAGE = "starfall-voice:2"


def wanted():
    out = subprocess.check_output(["cargo", "run", "-q", "--release", "-p", "view", "--example", "voice_lines"], cwd=ROOT)
    return json.loads(out)


def file_name(voice, text):
    return f"{voice}_{hashlib.sha1((voice + chr(0) + text).encode()).hexdigest()[:10]}.ogg"


def spoken(text, say):
    for a, b in say:
        text = text.replace(a, b)
    return " ".join(text.split())


def main():
    args = sys.argv[1:]
    voices = json.load(open(os.path.join(ASSETS, "voices.json"), encoding="utf-8"))
    presets = {p["id"]: p for p in voices["voices"]}
    path = os.path.join(ASSETS, "manifest.json")
    manifest = json.load(open(path, encoding="utf-8")) if os.path.exists(path) else {"lines": []}
    have = {(e["voice"], e["text"]): e for e in manifest["lines"]}
    want = wanted()
    todo = [w for w in want if "--all" in args or (w["voice"], w["text"]) not in have or not os.path.exists(os.path.join(ASSETS, have[(w["voice"], w["text"])]["file"]))]
    unknown = sorted({w["voice"] for w in want} - presets.keys())
    if unknown:
        sys.exit(f"voice: no preset for {unknown} in voices.json")
    print(f"voice: {len(want)} lines, {len(todo)} to generate")
    if "--check" in args:
        for w in todo:
            print(f"  {w['voice']}: {w['text']}")
        sys.exit(1 if todo else 0)
    results = {}
    if todo:
        shutil.rmtree(WORK, ignore_errors=True)
        os.makedirs(os.path.join(WORK, "out"))
        jobs = [{"out": file_name(w["voice"], w["text"]), "say": spoken(w["text"], voices["say"]), **{k: presets[w["voice"]][k] for k in ("speed", "pitch", "effect")}} for w in todo]
        json.dump(jobs, open(os.path.join(WORK, "jobs.json"), "w", encoding="utf-8"), ensure_ascii=False, indent=1)
        if subprocess.run(["docker", "image", "inspect", IMAGE], capture_output=True).returncode != 0:
            subprocess.run(["docker", "build", "-t", IMAGE, os.path.join(ROOT, "tools", "voice")], check=True)
        env = dict(os.environ, MSYS_NO_PATHCONV="1")
        subprocess.run(["docker", "run", "--rm", "-v", f"{WORK}:/work", "-v", "starfall-voice-cache:/root/.cache", IMAGE], check=True, env=env)
        res = json.load(open(os.path.join(WORK, "results.json"), encoding="utf-8"))
        results = res["secs"]
        manifest["generator"] = res["generator"]
        for name in results:
            shutil.copy(os.path.join(WORK, "out", name), os.path.join(ASSETS, name))
    lines = []
    for w in want:
        name = file_name(w["voice"], w["text"])
        secs = results.get(name) or have.get((w["voice"], w["text"]), {}).get("secs")
        lines.append({"voice": w["voice"], "text": w["text"], "file": name, "secs": secs})
    # Lines nobody says any more go away with their files.
    keep = {e["file"] for e in lines}
    for f in os.listdir(ASSETS):
        if f.endswith(".ogg") and f not in keep:
            os.remove(os.path.join(ASSETS, f))
            print(f"voice: removed {f}")
    manifest["lines"] = lines
    json.dump(manifest, open(path, "w", encoding="utf-8"), ensure_ascii=False, indent=1)
    total = sum(os.path.getsize(os.path.join(ASSETS, e["file"])) for e in lines)
    print(f"voice: {len(lines)} lines, {sum(e['secs'] for e in lines):.0f} s, {total / 1e6:.2f} MB")


main()

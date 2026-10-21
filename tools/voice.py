"""Generates the voice lines for the cutscenes and comms calls (docs/STORY.md §8).

    python tools/voice.py            # voice every new or changed line, drop lines no longer used
    python tools/voice.py --all      # regenerate everything (after changing a preset)
    python tools/voice.py --retake   # also redo lines Whisper misheard (or never checked)
    python tools/voice.py --check    # only list what is missing

What to say comes from the game's own data (`cargo run -p view --example voice_lines` reads
opening.ron and comms.ron); presets and the pronunciation table are crates/client/assets/voice/voices.json.
Speech is Chatterbox Multilingual V3 (Resemble AI, MIT) in Docker (tools/voice/Dockerfile), each preset
with its reference voice (tools/voice/refs: our own MeloTTS takes), expressiveness and pacing, then a
light ffmpeg effect (radio / computer / reverb). Whisper listens to every take; one it mishears (error
rate above voices.json "check".max_cer) is generated again with the next seed, up to "check".takes times,
and the best take is kept. About 25 s per take on the CPU. Output: crates/client/assets/voice/*.ogg and manifest.json, which
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
IMAGE = "starfall-voice:4"


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
    # Korean lines in the report; the Windows console defaults to cp949.
    sys.stdout.reconfigure(encoding="utf-8")
    args = sys.argv[1:]
    voices = json.load(open(os.path.join(ASSETS, "voices.json"), encoding="utf-8"))
    presets = {p["id"]: p for p in voices["voices"]}
    path = os.path.join(ASSETS, "manifest.json")
    manifest = json.load(open(path, encoding="utf-8")) if os.path.exists(path) else {"lines": []}
    have = {(e["voice"], e["text"]): e for e in manifest["lines"]}
    want = wanted()
    check = voices["check"]

    def stale(w):
        e = have.get((w["voice"], w["text"]))
        if e is None or not os.path.exists(os.path.join(ASSETS, e["file"])):
            return True
        return "--retake" in args and (e.get("cer") is None or e["cer"] > check["max_cer"])

    todo = [w for w in want if "--all" in args or stale(w)]
    unknown = sorted({w["voice"] for w in want} - presets.keys())
    if unknown:
        sys.exit(f"voice: no preset for {unknown} in voices.json")
    print(f"voice: {len(want)} lines, {len(todo)} to generate")
    if "--check" in args:
        for w in todo:
            print(f"  {w['voice']}: {w['text']}")
        sys.exit(1 if todo else 0)
    # Whisper writes numbers as digits but Latin names in Hangul: a third spelling to score against.
    names = [r for r in voices["say"] if not any(c.isdigit() for c in r[0])]
    results = {}
    if todo:
        shutil.rmtree(WORK, ignore_errors=True)
        os.makedirs(os.path.join(WORK, "out"))
        jobs = [{"out": file_name(w["voice"], w["text"]), "say": spoken(w["text"], voices["say"]), "text": w["text"], "names": spoken(w["text"], names), **{k: presets[w["voice"]][k] for k in ("ref", "exaggeration", "cfg", "speed", "effect")}} for w in todo]
        json.dump(jobs, open(os.path.join(WORK, "jobs.json"), "w", encoding="utf-8"), ensure_ascii=False, indent=1)
        json.dump(check, open(os.path.join(WORK, "check.json"), "w", encoding="utf-8"))
        if subprocess.run(["docker", "image", "inspect", IMAGE], capture_output=True).returncode != 0:
            subprocess.run(["docker", "build", "-t", IMAGE, os.path.join(ROOT, "tools", "voice")], check=True)
        env = dict(os.environ, MSYS_NO_PATHCONV="1")
        subprocess.run(["docker", "run", "--rm", "-v", f"{WORK}:/work", "-v", "starfall-voice-cache:/root/.cache", IMAGE], check=True, env=env)
        res = json.load(open(os.path.join(WORK, "results.json"), encoding="utf-8"))
        results = res["lines"]
        manifest["generator"] = res["generator"]
        for name in results:
            shutil.copy(os.path.join(WORK, "out", name), os.path.join(ASSETS, name))
    lines = []
    for w in want:
        name = file_name(w["voice"], w["text"])
        old = have.get((w["voice"], w["text"]), {})
        r = results.get(name) or {k: old.get(k) for k in ("secs", "take", "cer", "heard")}
        lines.append({"voice": w["voice"], "text": w["text"], "file": name, **r})
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
    rates = [e["cer"] for e in lines if e.get("cer") is not None]
    if rates:
        print(f"voice: Whisper error rate mean {sum(rates) / len(rates):.3f}, {sum(1 for e in lines if e.get('take'))} lines kept a retake")
    for e in lines:
        if e.get("cer") is not None and e["cer"] > check["max_cer"]:
            print(f"voice: still misheard after {check['takes']} takes ({e['voice']}, CER {e['cer']:.3f}): \"{e['text']}\" heard \"{e['heard']}\" — listen, or reword the line")


main()

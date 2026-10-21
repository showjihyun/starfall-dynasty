"""Runs inside the starfall-voice container (tools/voice/Dockerfile).

Reads /work/jobs.json: [{"out": "name.ogg", "say": "text to speak", "speed", "pitch", "effect"}],
writes /work/out/<out> (mono Vorbis, loudness-normalised) and /work/results.json with each file's
length in seconds, plus the MeloTTS version used.
"""

import json
import subprocess
from importlib import metadata

from melo.api import TTS

RATE = 44100
# Post-processing per preset effect (after the pitch shift).
EFFECTS = {
    "none": "",
    # Comms link: telephone band, a little squeeze.
    "radio": "highpass=f=320,lowpass=f=3400,acompressor=threshold=0.1:ratio=4,",
    # Ship computer: wide band with a short metallic comb.
    "computer": "highpass=f=180,lowpass=f=7000,aecho=0.8:0.55:9|17:0.35|0.25,",
    # Memory / the Aster: a large hall.
    "reverb": "aecho=0.8:0.85:110|230|370:0.38|0.24|0.14,",
}


def main():
    jobs = json.load(open("/work/jobs.json", encoding="utf-8"))
    tts = TTS(language="KR", device="cpu")
    speaker = tts.hps.data.spk2id["KR"]
    results = {}
    for j in jobs:
        wav = "/tmp/line.wav"
        tts.tts_to_file(j["say"], speaker, wav, speed=j["speed"], quiet=True)
        r = 2 ** (j["pitch"] / 12.0)
        # Pitch without changing length: resample up/down, then stretch back.
        chain = f"asetrate={int(RATE * r)},aresample={RATE},atempo={1 / r:.5f}," + EFFECTS[j["effect"]] + "loudnorm=I=-18:TP=-2,aresample=44100"
        out = "/work/out/" + j["out"]
        subprocess.run(["ffmpeg", "-y", "-loglevel", "error", "-i", wav, "-af", chain, "-ac", "1", "-c:a", "libvorbis", "-q:a", "4", out], check=True)
        secs = float(subprocess.check_output(["ffprobe", "-v", "error", "-show_entries", "format=duration", "-of", "csv=p=0", out]).decode().strip())
        results[j["out"]] = round(secs, 2)
        print(f"{secs:5.2f} s  {j['out']}", flush=True)
    try:
        version = "MeloTTS " + metadata.version("melotts")
    except metadata.PackageNotFoundError:
        version = "MeloTTS (version unknown)"
    json.dump({"secs": results, "generator": version}, open("/work/results.json", "w", encoding="utf-8"))


main()

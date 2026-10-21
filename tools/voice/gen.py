"""Runs inside the starfall-voice container (tools/voice/Dockerfile).

Reads /work/jobs.json: [{"out": "name.ogg", "say": "text to speak", "ref": "mid|low|high|builtin",
"exaggeration", "cfg", "speed", "effect"}], writes /work/out/<out> (mono Vorbis, loudness-normalised)
and /work/results.json with each file's length in seconds and the generator used.
Each line is seeded from its file name, so the same job gives the same take.
"""

import json
import subprocess
import zlib
from importlib import metadata

import torch
import torchaudio as ta
from chatterbox.mtl_tts import ChatterboxMultilingualTTS

# Light post-processing per preset effect: no pitch shifting (it made the voices sound synthetic).
EFFECTS = {
    "none": "",
    # Comms link: a wide telephone band, barely squeezed.
    "radio": "highpass=f=180,lowpass=f=5200,acompressor=threshold=0.15:ratio=2.5,",
    # Ship computer: clean and close, a touch of brightness.
    "computer": "highpass=f=120,treble=g=2:f=5000,",
    # Memory / the Aster: a soft hall.
    "reverb": "aecho=0.8:0.6:90|170:0.22|0.12,",
}


def main():
    jobs = json.load(open("/work/jobs.json", encoding="utf-8"))
    model = ChatterboxMultilingualTTS.from_pretrained(device="cpu", t3_model="v3")
    results = {}
    for j in jobs:
        torch.manual_seed(zlib.crc32(j["out"].encode()))
        kw = dict(language_id="ko", exaggeration=j["exaggeration"], cfg_weight=j["cfg"])
        if j["ref"] != "builtin":
            kw["audio_prompt_path"] = f"/refs/{j['ref']}.wav"
        wav = model.generate(j["say"], **kw)
        raw = "/tmp/line.wav"
        ta.save(raw, wav, model.sr)
        tempo = f"atempo={j['speed']:.3f}," if abs(j["speed"] - 1.0) > 1e-3 else ""
        chain = tempo + EFFECTS[j["effect"]] + "loudnorm=I=-18:TP=-2,aresample=44100"
        out = "/work/out/" + j["out"]
        subprocess.run(["ffmpeg", "-y", "-loglevel", "error", "-i", raw, "-af", chain, "-ac", "1", "-c:a", "libvorbis", "-q:a", "4", out], check=True)
        secs = float(subprocess.check_output(["ffprobe", "-v", "error", "-show_entries", "format=duration", "-of", "csv=p=0", out]).decode().strip())
        results[j["out"]] = round(secs, 2)
        print(f"{secs:5.2f} s  {j['out']}", flush=True)
    try:
        version = "Chatterbox Multilingual V3 (chatterbox-tts " + metadata.version("chatterbox-tts") + ")"
    except metadata.PackageNotFoundError:
        version = "Chatterbox Multilingual V3"
    json.dump({"secs": results, "generator": version}, open("/work/results.json", "w", encoding="utf-8"))


main()

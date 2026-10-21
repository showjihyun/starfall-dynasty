"""Runs inside the starfall-voice container (tools/voice/Dockerfile).

Reads /work/jobs.json: [{"out": "name.ogg", "say": "text to speak", "text": "on-screen text", "names": "text with only the names spelled out", "ref": "mid|low|high|builtin",
"exaggeration", "cfg", "speed", "effect"}] and /work/check.json ({"max_cer", "takes"}), writes /work/out/<out>
(mono Vorbis, loudness-normalised) and /work/results.json with each file's length, take, Whisper error rate
and what Whisper heard, plus the generator used.
Take n of a line is seeded from its file name and n, so the same job gives the same take. A take Whisper
mishears (character error rate above max_cer) is generated again with the next seed, up to `takes` times;
the take heard best is kept.
"""

import json
import subprocess
import zlib
from importlib import metadata

import torch
import torchaudio as ta
from chatterbox.mtl_tts import ChatterboxMultilingualTTS
from faster_whisper import WhisperModel

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
# Lines this short (letters and digits) are not checked: one misheard syllable is already a large rate.
MIN_CHECKED = 5


def norm(s):
    return "".join(c for c in s if c.isalnum())


def cer(ref, heard):
    a, b = norm(ref), norm(heard)
    d = list(range(len(b) + 1))
    for i, ca in enumerate(a, 1):
        p, d[0] = d[0], i
        for j, cb in enumerate(b, 1):
            p, d[j] = d[j], min(d[j] + 1, d[j - 1] + 1, p + (ca != cb))
    return d[-1] / max(1, len(a))


def main():
    jobs = json.load(open("/work/jobs.json", encoding="utf-8"))
    check = json.load(open("/work/check.json", encoding="utf-8"))
    model = ChatterboxMultilingualTTS.from_pretrained(device="cpu", t3_model="v3")
    asr = WhisperModel("small", device="cpu", compute_type="int8")
    results = {}
    for j in jobs:
        kw = dict(language_id="ko", exaggeration=j["exaggeration"], cfg_weight=j["cfg"])
        if j["ref"] != "builtin":
            kw["audio_prompt_path"] = f"/refs/{j['ref']}.wav"
        checked = len(norm(j["say"])) >= MIN_CHECKED
        best = None
        for take in range(check["takes"] if checked else 1):
            torch.manual_seed(zlib.crc32(j["out"].encode()) + take)
            wav = model.generate(j["say"], **kw)
            raw = f"/tmp/take{take}.wav"
            ta.save(raw, wav, model.sr)
            heard, rate = "", 0.0
            if checked:
                heard = "".join(s.text for s in asr.transcribe(raw, language="ko", beam_size=5)[0]).strip()
                # Whisper writes numbers and Latin names either way ("400년" / "사백 년"): score the closest spelling.
                rate = min(cer(j[k], heard) for k in ("say", "text", "names"))
                print(f"       take {take}: CER {rate:.2f}  heard: {heard}", flush=True)
            if best is None or rate < best[1]:
                best = (raw, rate, heard, take)
            if rate <= check["max_cer"]:
                break
        raw, rate, heard, take = best
        tempo = f"atempo={j['speed']:.3f}," if abs(j["speed"] - 1.0) > 1e-3 else ""
        chain = tempo + EFFECTS[j["effect"]] + "loudnorm=I=-18:TP=-2,aresample=44100"
        out = "/work/out/" + j["out"]
        subprocess.run(["ffmpeg", "-y", "-loglevel", "error", "-i", raw, "-af", chain, "-ac", "1", "-c:a", "libvorbis", "-q:a", "4", out], check=True)
        secs = float(subprocess.check_output(["ffprobe", "-v", "error", "-show_entries", "format=duration", "-of", "csv=p=0", out]).decode().strip())
        results[j["out"]] = {"secs": round(secs, 2), "take": take, "cer": round(rate, 3), "heard": heard}
        flag = "  ** still misheard" if rate > check["max_cer"] else ""
        print(f"{secs:5.2f} s  {j['out']}  take {take}  CER {rate:.2f}{flag}", flush=True)
    try:
        version = "Chatterbox Multilingual V3 (chatterbox-tts " + metadata.version("chatterbox-tts") + ")"
    except metadata.PackageNotFoundError:
        version = "Chatterbox Multilingual V3"
    json.dump({"lines": results, "generator": version}, open("/work/results.json", "w", encoding="utf-8"), ensure_ascii=False)


main()

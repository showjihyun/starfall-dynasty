# Voice generator for tools/voice.py: MeloTTS (MIT, Korean) + ffmpeg, CPU only.
# Built once as starfall-voice:2; models download into the starfall-voice-cache volume on first run.
FROM python:3.10-slim
RUN apt-get update -qq && apt-get install -y -qq --no-install-recommends git build-essential ffmpeg mecab libmecab-dev \
    && rm -rf /var/lib/apt/lists/*
RUN pip install -q --no-cache-dir git+https://github.com/myshell-ai/MeloTTS.git python-mecab-ko && python -m unidic download
# python-mecab-ko: the Korean g2p (g2pkk) needs it for pronunciation rules, or it warns and guesses.
COPY gen.py /gen.py
ENTRYPOINT ["python", "/gen.py"]

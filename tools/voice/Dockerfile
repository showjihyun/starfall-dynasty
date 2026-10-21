# Voice generator for tools/voice.py: Chatterbox Multilingual V3 (Resemble AI, MIT code and weights,
# Korean) + faster-whisper (MIT, checks each take) + ffmpeg, CPU only. Built once as starfall-voice:4; weights download into the
# starfall-voice-cache volume on first run. Reference voices are in tools/voice/refs (our own
# MeloTTS output, so no real person's voice is cloned).
FROM python:3.11-slim
RUN apt-get update -qq && apt-get install -y -qq --no-install-recommends git ffmpeg build-essential \
    && rm -rf /var/lib/apt/lists/*
RUN pip install -q --no-cache-dir torch torchaudio --index-url https://download.pytorch.org/whl/cpu
RUN pip install -q --no-cache-dir git+https://github.com/resemble-ai/chatterbox.git
RUN pip install -q --no-cache-dir faster-whisper
COPY gen.py /gen.py
COPY refs /refs
ENTRYPOINT ["python", "/gen.py"]

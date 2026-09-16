# syntax=docker/dockerfile:1
FROM python:3.12-slim-bookworm AS runtime

# libsndfile1 is the runtime dependency behind `soundfile`; ffmpeg is the
# fallback decoder for AAC/M4A (see music_fingerprint/audio.py). Both are
# invoked with explicit argv lists internally, never a shell string.
RUN apt-get update && apt-get install -y --no-install-recommends \
    libsndfile1 ffmpeg curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --create-home --uid 10001 appuser

WORKDIR /app

COPY processor/pyproject.toml ./
COPY processor/src ./src
COPY processor/README.md ./README.md
RUN pip install --no-cache-dir --upgrade pip && pip install --no-cache-dir .

USER appuser
ENV PROCESSOR_HOST=0.0.0.0 \
    PROCESSOR_PORT=8001
EXPOSE 8001

HEALTHCHECK --interval=10s --timeout=3s --start-period=10s --retries=5 \
    CMD curl -sf http://localhost:8001/internal/v1/health || exit 1

CMD ["music-fingerprint", "serve"]

"""High-level extraction entry points combining decode + fingerprint
generation. This is what the HTTP service and ingestion CLI call."""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path

from music_fingerprint.audio import decode_audio_bytes, decode_audio_file
from music_fingerprint.config import (
    DEFAULT_CONFIG,
    FINGERPRINT_ALGORITHM_VERSION,
    FingerprintConfig,
)
from music_fingerprint.fingerprint import Fingerprint, generate_fingerprints

#: Default caps applied unless a caller overrides them explicitly. Query
#: requests use the tighter `max_query_duration_seconds`; ingestion uses
#: `max_ingest_duration_seconds` (see config.py for rationale).
DEFAULT_MAX_QUERY_BYTES = 20 * 1024 * 1024  # 20 MB: generous headroom over a 15s PCM16 WAV
DEFAULT_MAX_INGEST_BYTES = 500 * 1024 * 1024  # 500 MB: covers uncompressed WAV of long tracks


@dataclass(frozen=True, slots=True)
class ExtractionResult:
    algorithm_version: int
    sample_rate: int
    duration_ms: int
    fingerprints: list[Fingerprint]


def extract_from_bytes(
    data: bytes,
    config: FingerprintConfig = DEFAULT_CONFIG,
    max_bytes: int = DEFAULT_MAX_QUERY_BYTES,
    max_duration_seconds: float | None = None,
    filename_hint: str | None = None,
) -> ExtractionResult:
    max_seconds = max_duration_seconds or config.max_query_duration_seconds
    audio = decode_audio_bytes(data, config, max_bytes, max_seconds, filename_hint=filename_hint)
    fingerprints = generate_fingerprints(audio, config)
    return ExtractionResult(
        algorithm_version=FINGERPRINT_ALGORITHM_VERSION,
        sample_rate=config.sample_rate,
        duration_ms=round(audio.duration_seconds * 1000),
        fingerprints=fingerprints,
    )


def extract_from_file(
    path: Path,
    config: FingerprintConfig = DEFAULT_CONFIG,
    max_bytes: int = DEFAULT_MAX_INGEST_BYTES,
    max_duration_seconds: float | None = None,
) -> ExtractionResult:
    max_seconds = max_duration_seconds or config.max_ingest_duration_seconds
    audio = decode_audio_file(path, config, max_bytes, max_seconds)
    fingerprints = generate_fingerprints(audio, config)
    return ExtractionResult(
        algorithm_version=FINGERPRINT_ALGORITHM_VERSION,
        sample_rate=config.sample_rate,
        duration_ms=round(audio.duration_seconds * 1000),
        fingerprints=fingerprints,
    )

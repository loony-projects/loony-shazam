"""Audio decoding, mono-conversion, resampling, and normalization.

Produces a single canonical representation — mono float32 PCM at
`config.sample_rate` — regardless of the input file's original format,
channel count, or sample rate. Everything downstream (spectrogram.py
onward) only ever sees this canonical form.
"""

from __future__ import annotations

import io
import subprocess
import tempfile
from dataclasses import dataclass
from pathlib import Path

import numpy as np
import soundfile as sf
from scipy.signal import resample_poly

from music_fingerprint.config import FingerprintConfig
from music_fingerprint.validation import (
    AudioDecodeError,
    AudioEmptyError,
    validate_decoded_duration,
    validate_payload_size,
)

# Anything libsndfile can't decode falls back to ffmpeg (see
# `_decode_with_ffmpeg`), invoked as an argv list — never through a shell —
# so there is no command-injection surface even though the input path/bytes
# are untrusted. The fallback is NOT gated on the file's extension: real-
# world files are routinely mislabeled (a ".mp3" that's actually M4A/AAC
# content is common from some download tools) and we explicitly never
# trust a filename extension for anything (see product spec "AUDIO
# SECURITY"). ffmpeg probes actual content bytes to pick a demuxer, so it
# doesn't need the extension to be correct either.


@dataclass(frozen=True, slots=True)
class AudioBuffer:
    """Canonical decoded audio: mono float32 samples at a known sample rate."""

    samples: np.ndarray  # shape (n,), dtype float32, range approximately [-1, 1]
    sample_rate: int
    duration_seconds: float


def _to_mono(samples: np.ndarray) -> np.ndarray:
    if samples.ndim == 1:
        return samples
    return samples.mean(axis=1)


def _resample(samples: np.ndarray, orig_sr: int, target_sr: int) -> np.ndarray:
    if orig_sr == target_sr:
        return samples
    # Polyphase resampling: deterministic, good stopband attenuation, no
    # ringing artifacts that would perturb peak locations.
    gcd = np.gcd(orig_sr, target_sr)
    up, down = target_sr // gcd, orig_sr // gcd
    return resample_poly(samples, up, down).astype(np.float32)


def _normalize_peak(samples: np.ndarray) -> np.ndarray:
    """Peak-normalize so downstream dB thresholds are comparable across
    tracks regardless of source loudness/mastering."""
    peak = float(np.max(np.abs(samples))) if samples.size else 0.0
    if peak < 1e-9:
        return samples  # silence: leave as-is, nothing to normalize
    return (samples / peak).astype(np.float32)


def _decode_with_soundfile(data: bytes) -> tuple[np.ndarray, int]:
    with sf.SoundFile(io.BytesIO(data)) as f:
        samples = f.read(dtype="float32", always_2d=False)
        return samples, f.samplerate


def _decode_with_ffmpeg(data: bytes, suffix: str) -> tuple[np.ndarray, int]:
    """Fallback decoder for anything libsndfile can't read (e.g. AAC/M4A
    content, including files mislabeled with an unrelated extension).

    Invoked with an explicit argv list (no shell=True, no string
    interpolation of any user-controlled value) to avoid any command
    injection surface, and writes to a private NamedTemporaryFile rather
    than a user-supplied path.
    """
    target_sr = 11025  # arbitrary intermediate rate; final resample happens in _resample
    with tempfile.NamedTemporaryFile(suffix=suffix) as src:
        src.write(data)
        src.flush()
        try:
            proc = subprocess.run(
                [
                    "ffmpeg",
                    "-nostdin",
                    "-y",
                    "-i",
                    src.name,
                    "-f",
                    "f32le",
                    "-ac",
                    "1",
                    "-ar",
                    str(target_sr),
                    "-loglevel",
                    "error",
                    "pipe:1",
                ],
                capture_output=True,
                timeout=60,
                check=True,
            )
        except (subprocess.CalledProcessError, subprocess.TimeoutExpired, FileNotFoundError) as exc:
            raise AudioDecodeError(f"ffmpeg fallback failed: {exc}") from exc
    samples = np.frombuffer(proc.stdout, dtype=np.float32)
    return samples, target_sr


def decode_audio_bytes(
    data: bytes,
    config: FingerprintConfig,
    max_bytes: int,
    max_duration_seconds: float,
    filename_hint: str | None = None,
) -> AudioBuffer:
    """Decode arbitrary (untrusted) audio bytes into a canonical AudioBuffer.

    `filename_hint` is used only to pick an ffmpeg-fallback container guess;
    it is never trusted for anything security-relevant and is not required.
    """
    validate_payload_size(len(data), max_bytes)

    try:
        samples, orig_sr = _decode_with_soundfile(data)
    except (sf.LibsndfileError, RuntimeError, ValueError) as exc:
        suffix = Path(filename_hint or "").suffix.lower()
        try:
            samples, orig_sr = _decode_with_ffmpeg(data, suffix or ".bin")
        except AudioDecodeError:
            raise AudioDecodeError(str(exc)) from exc

    if samples.size == 0:
        raise AudioEmptyError()

    mono = _to_mono(np.asarray(samples, dtype=np.float32))
    resampled = _resample(mono, orig_sr, config.sample_rate)
    normalized = _normalize_peak(resampled)

    duration_seconds = validate_decoded_duration(
        len(normalized), config.sample_rate, max_duration_seconds
    )
    return AudioBuffer(
        samples=normalized, sample_rate=config.sample_rate, duration_seconds=duration_seconds
    )


def decode_audio_file(
    path: Path,
    config: FingerprintConfig,
    max_bytes: int,
    max_duration_seconds: float,
) -> AudioBuffer:
    size = path.stat().st_size
    validate_payload_size(size, max_bytes)
    data = path.read_bytes()
    return decode_audio_bytes(
        data, config, max_bytes, max_duration_seconds, filename_hint=path.name
    )

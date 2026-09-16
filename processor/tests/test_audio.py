from __future__ import annotations

import io

import numpy as np
import pytest
import soundfile as sf

from music_fingerprint.audio import decode_audio_bytes
from music_fingerprint.config import FingerprintConfig
from music_fingerprint.validation import (
    AudioDecodeError,
    AudioEmptyError,
    AudioTooLargeError,
    AudioTooLongError,
)


def _wav_bytes(samples: np.ndarray, sample_rate: int, channels: int = 1) -> bytes:
    buf = io.BytesIO()
    data = samples if channels == 1 else np.column_stack([samples] * channels)
    sf.write(buf, data, sample_rate, format="WAV", subtype="PCM_16")
    return buf.getvalue()


def test_decode_resamples_and_downmixes_to_mono():
    sr_in = 44100
    t = np.arange(sr_in * 2) / sr_in
    tone = 0.5 * np.sin(2 * np.pi * 440 * t).astype(np.float32)
    data = _wav_bytes(tone, sr_in, channels=2)

    config = FingerprintConfig(sample_rate=11025)
    buf = decode_audio_bytes(data, config, max_bytes=10_000_000, max_duration_seconds=15)

    assert buf.sample_rate == 11025
    assert buf.samples.ndim == 1
    assert abs(buf.duration_seconds - 2.0) < 0.05


def test_decode_empty_raises():
    config = FingerprintConfig()
    empty_wav = _wav_bytes(np.zeros(0, dtype=np.float32), 11025)
    with pytest.raises(AudioEmptyError):
        decode_audio_bytes(empty_wav, config, max_bytes=10_000_000, max_duration_seconds=15)


def test_decode_garbage_raises_decode_error():
    config = FingerprintConfig()
    with pytest.raises(AudioDecodeError):
        decode_audio_bytes(
            b"not audio data" * 20, config, max_bytes=10_000_000, max_duration_seconds=15
        )


def test_payload_too_large_rejected():
    config = FingerprintConfig()
    data = _wav_bytes(np.zeros(1000, dtype=np.float32), 11025)
    with pytest.raises(AudioTooLargeError):
        decode_audio_bytes(data, config, max_bytes=10, max_duration_seconds=15)


def test_duration_too_long_rejected():
    config = FingerprintConfig()
    sr = 11025
    data = _wav_bytes(np.zeros(sr * 20, dtype=np.float32), sr)
    with pytest.raises(AudioTooLongError):
        decode_audio_bytes(data, config, max_bytes=100_000_000, max_duration_seconds=5)


def test_normalization_peaks_near_unity():
    config = FingerprintConfig()
    sr = 11025
    t = np.arange(sr) / sr
    quiet_tone = 0.01 * np.sin(2 * np.pi * 300 * t).astype(np.float32)
    data = _wav_bytes(quiet_tone, sr)
    buf = decode_audio_bytes(data, config, max_bytes=10_000_000, max_duration_seconds=15)
    assert np.max(np.abs(buf.samples)) > 0.9

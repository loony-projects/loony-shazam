from __future__ import annotations

import numpy as np

from music_fingerprint.audio import AudioBuffer
from music_fingerprint.config import FingerprintConfig
from music_fingerprint.spectrogram import compute_spectrogram


def _tone_buffer(freq: float, duration_s: float, config: FingerprintConfig) -> AudioBuffer:
    t = np.arange(int(duration_s * config.sample_rate)) / config.sample_rate
    samples = (0.8 * np.sin(2 * np.pi * freq * t)).astype(np.float32)
    return AudioBuffer(samples=samples, sample_rate=config.sample_rate, duration_seconds=duration_s)


def test_spectrogram_band_limited_shape():
    config = FingerprintConfig()
    audio = _tone_buffer(440, 3.0, config)
    spec = compute_spectrogram(audio, config)

    assert spec.bin_offset == config.min_freq_bin
    assert spec.num_bins == (
        min(config.max_freq_bin, config.fft_size // 2) - config.min_freq_bin + 1
    )
    assert spec.num_frames > 0


def test_pure_tone_energy_at_expected_bin():
    config = FingerprintConfig()
    freq = 1000.0
    audio = _tone_buffer(freq, 2.0, config)
    spec = compute_spectrogram(audio, config)

    expected_bin = round(freq / config.freq_bin_hz)
    # Average across frames, find the strongest row, compare to expectation.
    mean_energy = spec.magnitude_db.mean(axis=1)
    strongest_row = int(np.argmax(mean_energy))
    strongest_bin = strongest_row + spec.bin_offset

    assert abs(strongest_bin - expected_bin) <= 2


def test_short_audio_produces_one_padded_frame():
    config = FingerprintConfig()
    audio = _tone_buffer(440, 0.05, config)  # much shorter than fft_size
    spec = compute_spectrogram(audio, config)
    assert spec.num_frames >= 1

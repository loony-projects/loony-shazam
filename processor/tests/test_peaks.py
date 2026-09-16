from __future__ import annotations

import numpy as np

from music_fingerprint.audio import AudioBuffer
from music_fingerprint.config import FingerprintConfig
from music_fingerprint.peaks import detect_peaks
from music_fingerprint.spectrogram import compute_spectrogram


def test_pure_tone_yields_peaks_near_expected_bin():
    config = FingerprintConfig()
    freq = 1000.0
    sr = config.sample_rate
    duration = 3.0
    t = np.arange(int(duration * sr)) / sr
    samples = (0.8 * np.sin(2 * np.pi * freq * t)).astype(np.float32)
    audio = AudioBuffer(samples=samples, sample_rate=sr, duration_seconds=duration)

    spec = compute_spectrogram(audio, config)
    peaks = detect_peaks(spec, config, duration)

    assert len(peaks) > 0
    expected_bin = round(freq / config.freq_bin_hz)
    assert all(abs(p.frequency_bin - expected_bin) <= 3 for p in peaks)


def test_silence_yields_no_peaks():
    config = FingerprintConfig()
    sr = config.sample_rate
    duration = 2.0
    samples = np.zeros(int(duration * sr), dtype=np.float32)
    audio = AudioBuffer(samples=samples, sample_rate=sr, duration_seconds=duration)

    spec = compute_spectrogram(audio, config)
    peaks = detect_peaks(spec, config, duration)

    assert peaks == []


def test_peak_density_is_capped():
    config = FingerprintConfig(max_peaks_per_second=5)
    sr = config.sample_rate
    duration = 4.0
    rng = np.random.default_rng(42)
    samples = rng.uniform(-1, 1, int(duration * sr)).astype(np.float32)
    audio = AudioBuffer(samples=samples, sample_rate=sr, duration_seconds=duration)

    spec = compute_spectrogram(audio, config)
    peaks = detect_peaks(spec, config, duration)

    assert len(peaks) <= config.max_peaks_per_second * duration


def test_peaks_sorted_by_time_then_frequency():
    config = FingerprintConfig()
    sr = config.sample_rate
    duration = 3.0
    t = np.arange(int(duration * sr)) / sr
    samples = (0.6 * np.sin(2 * np.pi * 500 * t) + 0.6 * np.sin(2 * np.pi * 1500 * t)).astype(
        np.float32
    )
    audio = AudioBuffer(samples=samples, sample_rate=sr, duration_seconds=duration)

    spec = compute_spectrogram(audio, config)
    peaks = detect_peaks(spec, config, duration)

    keys = [(p.time_bin, p.frequency_bin) for p in peaks]
    assert keys == sorted(keys)

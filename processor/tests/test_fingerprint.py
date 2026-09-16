from __future__ import annotations

import json
from pathlib import Path

import numpy as np
import pytest
from conftest import synth_track

from music_fingerprint.audio import AudioBuffer
from music_fingerprint.config import FingerprintConfig
from music_fingerprint.fingerprint import generate_fingerprints

GOLDEN_DIR = Path(__file__).resolve().parents[2] / "testdata" / "fingerprints"


def _buffer(samples: np.ndarray, sr: int) -> AudioBuffer:
    return AudioBuffer(samples=samples, sample_rate=sr, duration_seconds=len(samples) / sr)


def test_determinism_same_audio_same_fingerprints(sample_rate):
    samples = synth_track(1, 6.0, sample_rate)
    audio = _buffer(samples, sample_rate)
    config = FingerprintConfig()

    fp1 = generate_fingerprints(audio, config)
    fp2 = generate_fingerprints(audio, config)

    assert fp1 == fp2
    assert len(fp1) > 0


def test_different_songs_produce_mostly_different_hashes(song_a, song_b, sample_rate):
    config = FingerprintConfig()
    fp_a = {f.hash for f in generate_fingerprints(_buffer(song_a, sample_rate), config)}
    fp_b = {f.hash for f in generate_fingerprints(_buffer(song_b, sample_rate), config)}

    overlap = len(fp_a & fp_b) / max(1, len(fp_a))
    # Both fixtures are synthetic tone sequences built from the same
    # harmonic recipe, so some coincidental hash collisions are expected;
    # the bar here is "clearly distinguishable", not "disjoint".
    assert overlap < 0.35


def test_silence_yields_zero_fingerprints(sample_rate):
    samples = np.zeros(sample_rate * 3, dtype=np.float32)
    audio = _buffer(samples, sample_rate)
    fingerprints = generate_fingerprints(audio, FingerprintConfig())
    assert fingerprints == []


def test_clipped_audio_does_not_crash(sample_rate):
    samples = synth_track(1, 4.0, sample_rate)
    clipped = np.clip(samples * 5.0, -1.0, 1.0)  # heavy hard clipping
    audio = _buffer(clipped, sample_rate)
    fingerprints = generate_fingerprints(audio, FingerprintConfig())
    assert isinstance(fingerprints, list)  # must not raise


def test_offsets_are_monotonic_nondecreasing(sample_rate):
    samples = synth_track(1, 6.0, sample_rate)
    audio = _buffer(samples, sample_rate)
    fingerprints = generate_fingerprints(audio, FingerprintConfig())
    offsets = [f.offset_ms for f in fingerprints]
    assert offsets == sorted(offsets)


@pytest.mark.skipif(not GOLDEN_DIR.exists(), reason="golden fixtures not generated")
def test_golden_fixture_simple_tone_matches_reference(sample_rate):
    golden_path = GOLDEN_DIR / "golden_simple_tone.json"
    if not golden_path.exists():
        pytest.skip("golden_simple_tone.json not generated yet")

    with golden_path.open() as f:
        expected = json.load(f)

    config = FingerprintConfig(**expected["config_overrides"])
    t = np.arange(int(expected["duration_s"] * sample_rate)) / sample_rate
    samples = (0.8 * np.sin(2 * np.pi * expected["freq"] * t)).astype(np.float32)
    audio = _buffer(samples, sample_rate)

    fingerprints = generate_fingerprints(audio, config)
    actual = [[f.hash, f.offset_ms] for f in fingerprints]

    assert actual == expected["fingerprints"]

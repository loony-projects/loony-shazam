"""End-to-end DSP quality check: fingerprint two synthetic reference
"songs", then verify that fingerprints extracted from transformed query
clips still identify the correct song via a minimal offset-voting matcher.

This minimal matcher is a test-only stand-in for the real matching engine
(which lives in Rust, see backend/src/matching) — it exists purely to
validate that the *fingerprints themselves* carry enough temporally
consistent signal to survive realistic distortions. It is intentionally not
shipped as part of the `music_fingerprint` package.
"""

from __future__ import annotations

import shutil
import subprocess
import tempfile
from collections import Counter
from pathlib import Path

import numpy as np
import pytest
import soundfile as sf
from conftest import synth_track

from music_fingerprint.audio import AudioBuffer, decode_audio_bytes
from music_fingerprint.config import FingerprintConfig
from music_fingerprint.fingerprint import Fingerprint, generate_fingerprints

OFFSET_BUCKET_MS = 100


def _buffer(samples: np.ndarray, sr: int) -> AudioBuffer:
    return AudioBuffer(samples=samples, sample_rate=sr, duration_seconds=len(samples) / sr)


def _build_index(
    fingerprints_by_song: dict[str, list[Fingerprint]],
) -> dict[int, list[tuple[str, int]]]:
    index: dict[int, list[tuple[str, int]]] = {}
    for song_id, fps in fingerprints_by_song.items():
        for fp in fps:
            index.setdefault(fp.hash, []).append((song_id, fp.offset_ms))
    return index


def _recognize(
    query_fingerprints: list[Fingerprint], index: dict[int, list[tuple[str, int]]]
) -> tuple[str | None, int]:
    """Minimal offset-histogram voting matcher, mirroring the approach
    documented in docs/fingerprinting.md (simplified for test purposes)."""
    votes: Counter[tuple[str, int]] = Counter()
    for qfp in query_fingerprints:
        for song_id, ref_offset_ms in index.get(qfp.hash, []):
            offset = ref_offset_ms - qfp.offset_ms
            bucket = round(offset / OFFSET_BUCKET_MS)
            votes[(song_id, bucket)] += 1

    if not votes:
        return None, 0

    (best_song, _bucket), best_votes = votes.most_common(1)[0]
    if best_votes < 5:  # matches the backend's default minimum-dominant-votes threshold
        return None, best_votes
    return best_song, best_votes


@pytest.fixture(scope="module")
def catalog_index(sample_rate):
    config = FingerprintConfig()
    fps_a = generate_fingerprints(_buffer(synth_track(1, 20.0, sample_rate), sample_rate), config)
    fps_b = generate_fingerprints(_buffer(synth_track(2, 20.0, sample_rate), sample_rate), config)
    return _build_index({"song_a": fps_a, "song_b": fps_b}), config


def _query_from(
    samples: np.ndarray, sample_rate: int, config: FingerprintConfig
) -> list[Fingerprint]:
    return generate_fingerprints(_buffer(samples, sample_rate), config)


def test_original_clip_matches(catalog_index, sample_rate):
    index, config = catalog_index
    clip = synth_track(1, 20.0, sample_rate)[sample_rate * 4 : sample_rate * 12]  # 8s clip
    song, votes = _recognize(_query_from(clip, sample_rate, config), index)
    assert song == "song_a"
    assert votes >= 5


def test_volume_reduced_clip_still_matches(catalog_index, sample_rate):
    index, config = catalog_index
    clip = synth_track(1, 20.0, sample_rate)[sample_rate * 4 : sample_rate * 12] * 0.3
    song, votes = _recognize(_query_from(clip.astype(np.float32), sample_rate, config), index)
    assert song == "song_a"


def test_noisy_clip_still_matches(catalog_index, sample_rate):
    index, config = catalog_index
    clip = synth_track(1, 20.0, sample_rate)[sample_rate * 4 : sample_rate * 12]
    rng = np.random.default_rng(7)
    # std=0.2 against a peak-normalized ([-1,1]) signal is substantial
    # additive white noise; peak-detector params were tuned (see
    # config.py / docs/performance.md) to keep a clear vote margin up to
    # this level.
    noise = rng.normal(0, 0.2, size=clip.shape).astype(np.float32)
    noisy = (clip + noise).astype(np.float32)
    song, votes = _recognize(_query_from(noisy, sample_rate, config), index)
    assert song == "song_a"


def test_clip_with_silence_padding_still_matches(catalog_index, sample_rate):
    index, config = catalog_index
    clip = synth_track(1, 20.0, sample_rate)[sample_rate * 4 : sample_rate * 12]
    padded = np.concatenate(
        [np.zeros(sample_rate, dtype=np.float32), clip, np.zeros(sample_rate, dtype=np.float32)]
    )
    song, votes = _recognize(_query_from(padded, sample_rate, config), index)
    assert song == "song_a"


def test_different_starting_offset_still_matches(catalog_index, sample_rate):
    index, config = catalog_index
    clip = synth_track(1, 20.0, sample_rate)[sample_rate * 9 : sample_rate * 17]
    song, votes = _recognize(_query_from(clip, sample_rate, config), index)
    assert song == "song_a"


def test_unrelated_audio_does_not_match(catalog_index, sample_rate):
    index, config = catalog_index
    rng = np.random.default_rng(99)
    noise = rng.normal(0, 0.3, size=sample_rate * 8).astype(np.float32)
    song, votes = _recognize(_query_from(noise, sample_rate, config), index)
    assert song is None


@pytest.mark.skipif(shutil.which("ffmpeg") is None, reason="ffmpeg not available")
def test_mp3_compressed_clip_still_matches(catalog_index, sample_rate):
    index, config = catalog_index
    clip = synth_track(1, 20.0, sample_rate)[sample_rate * 4 : sample_rate * 12]

    with tempfile.TemporaryDirectory() as tmp:
        wav_path = Path(tmp) / "clip.wav"
        mp3_path = Path(tmp) / "clip.mp3"
        sf.write(wav_path, clip, sample_rate)
        subprocess.run(
            ["ffmpeg", "-nostdin", "-y", "-i", str(wav_path), "-b:a", "128k", str(mp3_path)],
            capture_output=True,
            check=True,
            timeout=30,
        )
        mp3_bytes = mp3_path.read_bytes()

    audio = decode_audio_bytes(mp3_bytes, config, max_bytes=50_000_000, max_duration_seconds=15)
    song, votes = _recognize(generate_fingerprints(audio, config), index)
    assert song == "song_a"

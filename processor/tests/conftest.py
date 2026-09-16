"""Shared synthetic-audio fixtures for tests.

We never use copyrighted music in this repository. All "songs" used in
tests are deterministically synthesized tone sequences that are complex
enough to exercise the peak detector (multiple harmonics, a changing
melody) while remaining exactly reproducible and tiny.
"""

from __future__ import annotations

import numpy as np
import pytest

NOTE_SECONDS = 0.4
SEMITONE = 2.0 ** (1.0 / 12.0)


def synth_track(song_index: int, duration_s: float, sample_rate: int) -> np.ndarray:
    """Deterministically synthesize a distinct "song" for a given index.

    Each song is a sequence of notes (fundamental + 2 harmonics, short
    fade in/out to avoid clicks) whose pitch sequence is derived from
    `song_index` so different indices produce clearly different, but
    equally "musical", signals.
    """
    n_notes = max(1, int(duration_s / NOTE_SECONDS))
    samples = np.zeros(int(duration_s * sample_rate), dtype=np.float64)
    t_note = np.arange(int(NOTE_SECONDS * sample_rate)) / sample_rate
    fade = min(200, len(t_note) // 4)
    envelope = np.ones_like(t_note)
    if fade > 0:
        ramp = np.linspace(0, 1, fade)
        envelope[:fade] = ramp
        envelope[-fade:] = ramp[::-1]

    for note_idx in range(n_notes):
        # Knuth multiplicative-hash style mixing so the pitch sequence does
        # not repeat on a short period (a naive `% 24` cycle would alias
        # against itself in the offset histogram and make matching tests
        # meaningless — different slices of the "song" would look
        # identical).
        mixed = ((note_idx + 1) * 2654435761 + (song_index + 1) * 2246822519) & 0xFFFFFFFF
        semitone_step = ((mixed >> 8) % 24) - 12
        base_freq = 220.0 * (SEMITONE**semitone_step)
        note = (
            1.00 * np.sin(2 * np.pi * base_freq * t_note)
            + 0.5 * np.sin(2 * np.pi * base_freq * 2 * t_note)
            + 0.25 * np.sin(2 * np.pi * base_freq * 3 * t_note)
        )
        note = note * envelope
        start = note_idx * len(t_note)
        end = start + len(t_note)
        if end > len(samples):
            note = note[: len(samples) - start]
            end = len(samples)
        samples[start:end] += note[: end - start]

    peak = np.max(np.abs(samples))
    if peak > 0:
        samples = samples / peak * 0.9
    return samples.astype(np.float32)


@pytest.fixture(scope="module")
def sample_rate() -> int:
    return 11025


@pytest.fixture
def song_a(sample_rate: int) -> np.ndarray:
    return synth_track(song_index=1, duration_s=20.0, sample_rate=sample_rate)


@pytest.fixture
def song_b(sample_rate: int) -> np.ndarray:
    return synth_track(song_index=2, duration_s=20.0, sample_rate=sample_rate)

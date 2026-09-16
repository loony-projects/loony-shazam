#!/usr/bin/env python3
"""Generates a small catalog of synthetic, deterministic "songs" (tone
sequences — no copyrighted or even realistic music) for local dev / e2e
testing. See docs/ingestion.md.

Usage:
    python3 scripts/dev/generate_test_catalog.py <output_dir> [--num-songs N] [--duration S]
"""

from __future__ import annotations

import argparse
from pathlib import Path

import numpy as np
import soundfile as sf

SAMPLE_RATE = 44100
NOTE_SECONDS = 0.4
SEMITONE = 2.0 ** (1.0 / 12.0)
NAMES = ["Alpha", "Beta", "Gamma", "Delta", "Epsilon", "Zeta", "Eta", "Theta"]


def synth_track(song_index: int, duration_s: float, sample_rate: int) -> np.ndarray:
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
        end = min(start + len(t_note), len(samples))
        samples[start:end] += note[: end - start]

    peak = np.max(np.abs(samples))
    if peak > 0:
        samples = samples / peak * 0.9
    return samples.astype(np.float32)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("output_dir", type=Path)
    parser.add_argument("--num-songs", type=int, default=5)
    parser.add_argument("--duration", type=float, default=25.0)
    args = parser.parse_args()

    args.output_dir.mkdir(parents=True, exist_ok=True)
    for i in range(args.num_songs):
        name = NAMES[i % len(NAMES)]
        track = synth_track(i, args.duration, SAMPLE_RATE)
        path = args.output_dir / f"Test Artist - Synthetic {name}.wav"
        sf.write(path, track, SAMPLE_RATE)
        print(f"wrote {path} ({args.duration:.1f}s)")


if __name__ == "__main__":
    main()

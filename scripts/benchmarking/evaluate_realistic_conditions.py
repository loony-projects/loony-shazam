#!/usr/bin/env python3
"""Evaluation harness for the actual "Shazam" use case: a song playing out
loud, picked up by a phone microphone — as opposed to
evaluate_robustness.py, which tests clean-cut/lightly-distorted digital
clips. This one simulates the acoustic chain a real query goes through:

    reference audio -> room reverberation -> speaker/mic frequency
    response -> ambient noise -> phone upload

using real audio files (not synthetic tones), against a live, already-
running backend. Every distortion here is a documented approximation, not
a measured real room — see the module docstring on each simulation
function for exactly what it does and does not model.

Usage:
    python3 scripts/benchmarking/evaluate_realistic_conditions.py \
        --audio-dir ~/Music/Resona \
        --backend-url http://localhost:8080 \
        --num-songs 5
"""

from __future__ import annotations

import argparse
import io
import random
import subprocess
import sys
import time
from pathlib import Path

import httpx
import numpy as np
import soundfile as sf
from scipy.signal import butter, fftconvolve, sosfilt

SAMPLE_RATE = 44100
CLIP_DURATION_S = 10.0


def _decode_with_ffmpeg(path: Path, sample_rate: int = SAMPLE_RATE) -> np.ndarray:
    """Reuses the same content-probing decode strategy as the processor
    itself (see processor/src/music_fingerprint/audio.py) — real files in
    the wild are routinely mislabeled (see docs/ingestion.md), so we don't
    trust the extension here either."""
    proc = subprocess.run(
        [
            "ffmpeg",
            "-nostdin",
            "-y",
            "-i",
            str(path),
            "-f",
            "f32le",
            "-ac",
            "1",
            "-ar",
            str(sample_rate),
            "-loglevel",
            "error",
            "pipe:1",
        ],
        capture_output=True,
        timeout=60,
        check=True,
    )
    return np.frombuffer(proc.stdout, dtype=np.float32).copy()


def simulate_room_reverb(samples: np.ndarray, sample_rate: int, decay_s: float = 0.4) -> np.ndarray:
    """Approximates small-room reverberation by convolving with a
    synthetic impulse response: exponentially-decaying filtered noise,
    the standard cheap approximation used when a measured room impulse
    response isn't available. Not a substitute for a real IR, but far
    closer to "played out loud in a room" than a bone-dry clip."""
    ir_len = int(decay_s * sample_rate)
    t = np.arange(ir_len) / sample_rate
    rng = np.random.default_rng(42)
    noise = rng.normal(0, 1, ir_len)
    envelope = np.exp(-t / (decay_s / 4))
    impulse_response = (noise * envelope).astype(np.float32)
    impulse_response[0] = 1.0  # preserve a direct-path (dry) component
    impulse_response /= np.max(np.abs(impulse_response))

    wet = fftconvolve(samples, impulse_response, mode="full")[: len(samples)]
    # Mix mostly-dry + reverb tail, like a small room rather than a cathedral.
    mixed = 0.7 * samples + 0.3 * wet
    peak = np.max(np.abs(mixed))
    return (mixed / peak * 0.9 if peak > 0 else mixed).astype(np.float32)


def simulate_speaker_mic_response(samples: np.ndarray, sample_rate: int) -> np.ndarray:
    """Approximates the combined frequency response of a small phone/
    laptop speaker playing out loud and a phone microphone picking it
    back up: both roll off bass heavily and have a reduced treble
    ceiling relative to a direct digital signal. Modeled as a bandpass
    filter, roughly 150Hz-7000Hz — a documented approximation of typical
    small-driver speaker + phone mic response, not a measured device."""
    nyquist = sample_rate / 2
    low, high = 150 / nyquist, min(7000 / nyquist, 0.99)
    sos = butter(4, [low, high], btype="band", output="sos")
    filtered = sosfilt(sos, samples).astype(np.float32)
    # Mild soft clipping: small speakers distort at the volume needed to
    # be "heard across a room" — tanh is a standard cheap soft-clip model.
    return np.tanh(filtered * 1.5).astype(np.float32) * 0.8


def simulate_ambient_noise(samples: np.ndarray, snr_db: float, seed: int = 7) -> np.ndarray:
    """Adds white noise at a target signal-to-noise ratio — a stand-in for
    ambient room/environmental noise (not any specific real noise
    recording)."""
    rng = np.random.default_rng(seed)
    signal_power = np.mean(samples**2)
    noise_power = signal_power / (10 ** (snr_db / 10))
    noise = rng.normal(0, np.sqrt(noise_power), samples.shape).astype(np.float32)
    return samples + noise


def simulate_realistic_playback(
    samples: np.ndarray, sample_rate: int, snr_db: float = 15.0
) -> np.ndarray:
    """The full chain: reverb -> speaker/mic response -> ambient noise —
    the closest approximation in this harness to "someone actually held
    their phone up to a speaker in a room"."""
    reverberant = simulate_room_reverb(samples, sample_rate)
    colored = simulate_speaker_mic_response(reverberant, sample_rate)
    noisy = simulate_ambient_noise(colored, snr_db)
    peak = np.max(np.abs(noisy))
    return (noisy / peak * 0.9 if peak > 0 else noisy).astype(np.float32)


def wav_bytes(samples: np.ndarray, sample_rate: int) -> bytes:
    buf = io.BytesIO()
    sf.write(buf, samples, sample_rate, format="WAV")
    return buf.getvalue()


def recognize(backend_url: str, audio_bytes: bytes) -> dict:
    files = {"audio": ("clip.wav", audio_bytes, "audio/wav")}
    started = time.perf_counter()
    resp = httpx.post(f"{backend_url}/api/v1/recognitions/audio", files=files, timeout=30)
    latency_ms = (time.perf_counter() - started) * 1000
    resp.raise_for_status()
    body = resp.json()
    body["_client_latency_ms"] = round(latency_ms, 1)
    return body


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--audio-dir", required=True, type=Path, help="Directory of already-ingested audio files"
    )
    parser.add_argument("--backend-url", default="http://localhost:8080")
    parser.add_argument("--num-songs", type=int, default=5)
    parser.add_argument("--clip-start-s", type=float, default=40.0)
    parser.add_argument("--out", default="testdata/expected/realistic_conditions_report.csv")
    args = parser.parse_args()

    print(f"Waiting for backend at {args.backend_url} to be ready...")
    for _ in range(15):
        try:
            if httpx.get(f"{args.backend_url}/ready", timeout=2).status_code == 200:
                break
        except httpx.RequestError:
            pass
        time.sleep(2)
    else:
        print("backend never became ready", file=sys.stderr)
        sys.exit(1)

    candidates = sorted(
        p
        for p in args.audio_dir.iterdir()
        if p.suffix.lower() in {".mp3", ".m4a", ".flac", ".wav", ".ogg"}
    )
    if not candidates:
        print(f"No audio files found in {args.audio_dir}", file=sys.stderr)
        sys.exit(1)

    rng = random.Random(11)
    selected = rng.sample(candidates, min(args.num_songs, len(candidates)))

    header = [
        "file",
        "condition",
        "recognized",
        "recognized_title",
        "score",
        "confidence",
        "matched_fingerprints",
        "query_fingerprints",
        "latency_ms",
    ]
    rows = [header]
    print(" | ".join(f"{h:<22}" for h in header))
    print("-" * 190)

    for path in selected:
        try:
            full = _decode_with_ffmpeg(path)
        except subprocess.CalledProcessError as exc:
            print(f"  (skip {path.name}: decode failed: {exc})")
            continue

        start_sample = int(args.clip_start_s * SAMPLE_RATE)
        end_sample = start_sample + int(CLIP_DURATION_S * SAMPLE_RATE)
        if end_sample > len(full):
            start_sample = max(0, len(full) // 3)
            end_sample = start_sample + int(CLIP_DURATION_S * SAMPLE_RATE)
        clip = full[start_sample:end_sample]
        if len(clip) < SAMPLE_RATE:
            print(f"  (skip {path.name}: too short)")
            continue

        conditions = {
            "clean_clip": clip,
            "room_reverb_only": simulate_room_reverb(clip, SAMPLE_RATE),
            "speaker_mic_response_only": simulate_speaker_mic_response(clip, SAMPLE_RATE),
            "realistic_playback_snr15db": simulate_realistic_playback(
                clip, SAMPLE_RATE, snr_db=15.0
            ),
            "realistic_playback_snr5db": simulate_realistic_playback(clip, SAMPLE_RATE, snr_db=5.0),
        }

        for condition_name, variant in conditions.items():
            # Paced to stay under the backend's default per-IP rate limit
            # (RATE_LIMIT_RPM=60, see backend/src/routes/rate_limit.rs) —
            # a real client would never fire requests back-to-back like
            # this harness otherwise would.
            time.sleep(1.1)
            result = recognize(args.backend_url, wav_bytes(variant, SAMPLE_RATE))
            song_title = result.get("song", {}).get("title") if result.get("song") else None
            match = result.get("match", {})
            row = [
                path.name,
                condition_name,
                result["recognized"],
                song_title,
                round(match.get("score", 0), 2),
                round(match.get("confidence", 0), 3),
                match.get("matched_fingerprints", 0),
                match.get("query_fingerprints", 0),
                result.get("_client_latency_ms"),
            ]
            rows.append(row)
            print(" | ".join(f"{str(v):<22}" for v in row))

    out_path = Path(args.out)
    out_path.parent.mkdir(parents=True, exist_ok=True)
    import csv

    with out_path.open("w", newline="") as f:
        csv.writer(f).writerows(rows)
    print(f"\nWrote {out_path}")

    total = len(rows) - 1
    recognized_count = sum(1 for r in rows[1:] if r[2] is True)
    print(f"\n{recognized_count}/{total} recognized across all conditions")


if __name__ == "__main__":
    main()

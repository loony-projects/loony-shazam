#!/usr/bin/env python3
"""Evaluation harness: builds a small synthetic reference catalog, ingests
it into a running backend, generates realistic distortions of one
reference track, sends each variant to the real recognition API, and
prints/saves a report of expected vs. actual outcomes.

This exercises the full stack over real HTTP (Android's own integration
point), unlike the pytest integration tests which call the DSP/matching
code in-process — see processor/tests/test_integration_recognition.py for
that faster, code-level equivalent.

Usage (with the stack already running, e.g. `make dev`):
    python3 scripts/benchmarking/evaluate_robustness.py \
        --backend-url http://localhost:8080 \
        --database-url postgresql://postgres:devpass@localhost:5432/loony_shazam
"""

from __future__ import annotations

import argparse
import csv
import io
import subprocess
import sys
import tempfile
import time
from pathlib import Path

import httpx
import numpy as np
import psycopg
import soundfile as sf

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "processor" / "src"))
sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "processor" / "tests"))

from conftest import synth_track  # noqa: E402

SAMPLE_RATE = 44100
REFERENCE_DURATION_S = 25.0
CLIP_DURATION_S = 8.0
CLIP_START_S = 6.0
EVAL_ARTIST = "Eval Harness Artist"
# Deliberately distinctive song indices (the deterministic synth generator
# in conftest.py/generate_test_catalog.py produces identical audio for the
# same index+duration prefix regardless of which script calls it) so this
# harness's reference/decoy tracks can never accidentally collide with
# leftover synthetic songs from an unrelated test run sharing the same
# database (e.g. `make e2e`, which uses indices 0-2).
REFERENCE_SONG_INDEX = 501
DECOY_SONG_INDEX = 502


def make_variants(reference: np.ndarray, sr: int) -> dict[str, np.ndarray]:
    clip_start = int(CLIP_START_S * sr)
    clip_end = clip_start + int(CLIP_DURATION_S * sr)
    base_clip = reference[clip_start:clip_end].copy()

    variants: dict[str, np.ndarray] = {"original": base_clip}

    variants["volume_-10db"] = base_clip * (10 ** (-10 / 20))
    variants["volume_-20db"] = base_clip * (10 ** (-20 / 20))

    rng = np.random.default_rng(42)
    for label, std in [("noise_light", 0.05), ("noise_moderate", 0.15), ("noise_heavy", 0.3)]:
        noise = rng.normal(0, std, base_clip.shape).astype(np.float32)
        variants[label] = base_clip + noise

    variants["trimmed_4s"] = base_clip[: int(4 * sr)]

    silence = np.zeros(int(1.0 * sr), dtype=np.float32)
    variants["silence_before_and_after"] = np.concatenate([silence, base_clip, silence])

    different_offset_start = int(15 * sr)
    variants["different_offset"] = reference[
        different_offset_start : different_offset_start + int(CLIP_DURATION_S * sr)
    ]

    return {k: v.astype(np.float32) for k, v in variants.items()}


def mp3_roundtrip(samples: np.ndarray, sr: int, tmp_dir: Path) -> bytes | None:
    wav_path = tmp_dir / "in.wav"
    mp3_path = tmp_dir / "out.mp3"
    sf.write(wav_path, samples, sr)
    try:
        subprocess.run(
            ["ffmpeg", "-nostdin", "-y", "-i", str(wav_path), "-b:a", "128k", str(mp3_path)],
            capture_output=True,
            check=True,
            timeout=30,
        )
    except (subprocess.CalledProcessError, FileNotFoundError):
        return None
    return mp3_path.read_bytes()


def wav_bytes(samples: np.ndarray, sr: int) -> bytes:
    buf = io.BytesIO()
    sf.write(buf, samples, sr, format="WAV")
    return buf.getvalue()


def recognize(backend_url: str, audio_bytes: bytes, content_type: str) -> dict:
    files = {"audio": ("clip", audio_bytes, content_type)}
    started = time.perf_counter()
    resp = httpx.post(f"{backend_url}/api/v1/recognitions/audio", files=files, timeout=30)
    client_latency_ms = (time.perf_counter() - started) * 1000
    resp.raise_for_status()
    body = resp.json()
    body["_client_latency_ms"] = round(client_latency_ms, 1)
    return body


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--backend-url", default="http://localhost:8080")
    parser.add_argument("--admin-key", default="dev-admin-key")
    parser.add_argument(
        "--database-url",
        default="postgresql://postgres:devpass@localhost:5432/loony_shazam",
        help="Used only to make this harness idempotent (clears its own prior "
        "rows by artist name before re-ingesting) — never for anything else.",
    )
    parser.add_argument("--out", default="testdata/expected/evaluation_report.csv")
    args = parser.parse_args()

    print(f"Clearing any previous '{EVAL_ARTIST}' rows for a clean, repeatable run...")
    with psycopg.connect(args.database_url, autocommit=True) as conn:
        conn.execute("DELETE FROM songs WHERE artist = %s", (EVAL_ARTIST,))

    print(f"Waiting for backend at {args.backend_url} to be ready...")
    for _ in range(30):
        try:
            r = httpx.get(f"{args.backend_url}/ready", timeout=2)
            if r.status_code == 200:
                break
        except httpx.RequestError:
            pass
        time.sleep(2)
    else:
        print("backend never became ready", file=sys.stderr)
        sys.exit(1)

    with tempfile.TemporaryDirectory() as tmp:
        tmp_dir = Path(tmp)

        print("Generating and ingesting reference catalog...")
        reference = synth_track(REFERENCE_SONG_INDEX, REFERENCE_DURATION_S, SAMPLE_RATE)
        decoy = synth_track(DECOY_SONG_INDEX, REFERENCE_DURATION_S, SAMPLE_RATE)

        catalog_dir = tmp_dir / "catalog"
        catalog_dir.mkdir()
        sf.write(catalog_dir / f"{EVAL_ARTIST} - Reference Track.wav", reference, SAMPLE_RATE)
        sf.write(catalog_dir / f"{EVAL_ARTIST} - Decoy Track.wav", decoy, SAMPLE_RATE)

        import os

        subprocess.run(
            [sys.executable, "-m", "music_fingerprint.cli", "ingest", str(catalog_dir)],
            check=True,
            env={**os.environ, "DATABASE_URL": args.database_url},
            cwd=str(Path(__file__).resolve().parents[2] / "processor"),
        )

        print("Generating variants and querying the live recognition API...\n")
        variants = make_variants(reference, SAMPLE_RATE)

        rows = []
        header = [
            "test_case",
            "expected_song",
            "recognized_song",
            "recognized",
            "score",
            "confidence",
            "matched_fingerprints",
            "latency_ms",
        ]
        print(" | ".join(f"{h:<20}" for h in header))
        print("-" * 150)

        for label, samples in variants.items():
            data = wav_bytes(samples, SAMPLE_RATE)
            result = recognize(args.backend_url, data, "audio/wav")
            row = _row(label, "Reference Track", result)
            rows.append(row)
            print(" | ".join(f"{str(v):<20}" for v in row))

        mp3_data = mp3_roundtrip(variants["original"], SAMPLE_RATE, tmp_dir)
        if mp3_data:
            result = recognize(args.backend_url, mp3_data, "audio/mp3")
            row = _row("mp3_128k", "Reference Track", result)
            rows.append(row)
            print(" | ".join(f"{str(v):<20}" for v in row))
        else:
            print("(skipped mp3_128k: ffmpeg not available)")

        rng = np.random.default_rng(99)
        unrelated = rng.normal(0, 0.3, int(CLIP_DURATION_S * SAMPLE_RATE)).astype(np.float32)
        result = recognize(args.backend_url, wav_bytes(unrelated, SAMPLE_RATE), "audio/wav")
        row = _row("unrelated_white_noise", None, result)
        rows.append(row)
        print(" | ".join(f"{str(v):<20}" for v in row))

        out_path = Path(args.out)
        out_path.parent.mkdir(parents=True, exist_ok=True)
        with out_path.open("w", newline="") as f:
            writer = csv.writer(f)
            writer.writerow(header)
            writer.writerows(rows)
        print(f"\nWrote {out_path}")


def _row(label: str, expected_song: str | None, result: dict) -> list:
    song_title = result.get("song", {}).get("title") if result.get("song") else None
    match = result.get("match", {})
    return [
        label,
        expected_song,
        song_title,
        result["recognized"],
        round(match.get("score", 0), 2),
        round(match.get("confidence", 0), 3),
        match.get("matched_fingerprints", 0),
        result.get("_client_latency_ms", match.get("latency_ms")),
    ]


if __name__ == "__main__":
    main()

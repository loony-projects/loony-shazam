#!/usr/bin/env python3
"""Measures per-stage latency of the fingerprint extraction pipeline on
synthetic audio of various durations. Prints a report and writes JSON
results next to this script. Run with `make benchmark` or directly:

    python processor/benchmarks/run_benchmarks.py
"""

from __future__ import annotations

import json
import statistics
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "src"))
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "tests"))

from conftest import synth_track  # noqa: E402

from music_fingerprint.audio import AudioBuffer  # noqa: E402
from music_fingerprint.config import FingerprintConfig  # noqa: E402
from music_fingerprint.constellation import build_constellation  # noqa: E402
from music_fingerprint.fingerprint import generate_fingerprints  # noqa: E402
from music_fingerprint.peaks import detect_peaks  # noqa: E402
from music_fingerprint.spectrogram import compute_spectrogram  # noqa: E402

SAMPLE_RATE = 11025
DURATIONS_S = [5, 10, 30, 60, 180]
REPEATS = 5


def percentile(values: list[float], p: float) -> float:
    values = sorted(values)
    k = (len(values) - 1) * p
    f, c = int(k), min(int(k) + 1, len(values) - 1)
    if f == c:
        return values[f]
    return values[f] + (values[c] - values[f]) * (k - f)


def bench_stage(fn, repeats: int = REPEATS) -> dict:
    samples = []
    for _ in range(repeats):
        start = time.perf_counter()
        result = fn()
        samples.append((time.perf_counter() - start) * 1000)
    return {
        "p50_ms": round(statistics.median(samples), 3),
        "p95_ms": round(percentile(samples, 0.95), 3),
        "p99_ms": round(percentile(samples, 0.99), 3),
        "min_ms": round(min(samples), 3),
        "max_ms": round(max(samples), 3),
    }, result


def main() -> None:
    config = FingerprintConfig()
    report: dict[str, dict] = {}

    for duration_s in DURATIONS_S:
        samples = synth_track(1, duration_s, SAMPLE_RATE)
        audio = AudioBuffer(samples=samples, sample_rate=SAMPLE_RATE, duration_seconds=duration_s)

        spec_stats, spec = bench_stage(lambda audio=audio: compute_spectrogram(audio, config))
        peaks_stats, peaks = bench_stage(
            lambda spec=spec, duration_s=duration_s: detect_peaks(spec, config, duration_s)
        )
        constellation_stats, constellation = bench_stage(
            lambda audio=audio: build_constellation(audio, config)
        )
        fingerprint_stats, fingerprints = bench_stage(
            lambda audio=audio: generate_fingerprints(audio, config)
        )

        n_fp = len(fingerprints)
        fp_per_sec = (
            n_fp / (fingerprint_stats["p50_ms"] / 1000) if fingerprint_stats["p50_ms"] > 0 else 0
        )

        report[f"{duration_s}s"] = {
            "num_peaks": len(peaks),
            "num_fingerprints": n_fp,
            "spectrogram": spec_stats,
            "peak_detection": peaks_stats,
            "constellation_end_to_end": constellation_stats,
            "fingerprint_generation_end_to_end": fingerprint_stats,
            "fingerprints_per_second_generated": round(fp_per_sec, 1),
        }

        print(f"\n=== {duration_s}s synthetic audio ===")
        print(f"  peaks: {len(peaks)}, fingerprints: {n_fp}")
        print(f"  spectrogram:     p50={spec_stats['p50_ms']}ms p95={spec_stats['p95_ms']}ms")
        print(f"  peak_detection:  p50={peaks_stats['p50_ms']}ms p95={peaks_stats['p95_ms']}ms")
        fp_p50, fp_p99 = fingerprint_stats["p50_ms"], fingerprint_stats["p99_ms"]
        print(f"  full pipeline:   p50={fp_p50}ms p95={fp_p99}ms")
        print(f"  throughput:      {fp_per_sec:.0f} fingerprints/sec generated")

    out_path = Path(__file__).parent / "results.json"
    out_path.write_text(json.dumps(report, indent=2))
    print(f"\nWrote {out_path}")


if __name__ == "__main__":
    main()

# Performance

Every number on this page was actually measured on this development
machine (not estimated/claimed) — see "How to reproduce" at the bottom.
Absolute numbers will vary by hardware; the *shape* of the results (what
dominates cost, how it scales) is the useful part.

## Fingerprint extraction (Python processor)

`processor/benchmarks/run_benchmarks.py`, synthetic audio, 5 repeats per
stage, this machine (see reproduction section for spec):

| Duration | Peaks | Fingerprints | Spectrogram p50 | Peak detection p50 | Full pipeline p50 | Full pipeline p95 | Throughput |
|---|---|---|---|---|---|---|---|
| 5s | 48 | 220 | 3.9ms | 5.5ms | 10.0ms | 10.4ms | 22,042 fp/s |
| 10s | 98 | 470 | 8.1ms | 11.7ms | 20.5ms | 20.8ms | 22,930 fp/s |
| 30s | 288 | 1,420 | 25.6ms | 37.4ms | 64.6ms | 64.8ms | 21,965 fp/s |
| 60s | 562 | 2,790 | 105.2ms | 78.9ms | 144.6ms | 148.6ms | 19,290 fp/s |
| 180s | 1,685 | 8,405 | 240.0ms | 251.0ms | 409.4ms | 448.4ms | 20,531 fp/s |

An 8–12 second query clip (the Android capture window) extracts in
roughly **15–20ms** end to end — negligible next to the network round trip
to upload the audio in the first place.

### A real fix along the way: local-median → local-mean

The first working version of `peaks.py` used `scipy.ndimage.median_filter`
to estimate each pixel's local background level (see
[fingerprinting.md](fingerprinting.md) §3). That was correct but
**~400x slower than it needed to be**: profiling a ~10s-clip-sized
spectrogram array showed

```
median_filter(size=(9,25)):   6.08s
uniform_filter(size=(9,25)):  0.015s   (mean, separable box filter)
```

`scipy`'s median filter has no separable fast path for a 2D rectangular
window; a local mean does (it reduces to a cumulative-sum box filter).
Switching `peaks.py` to `uniform_filter` (documented in the code as a
local-mean approximation of the original local-median background
estimate) cut peak detection from **484ms → 5.5ms** on a 5-second clip
with no change in recognition quality — the full Python test suite
(including the noise/compression robustness tests in
`test_integration_recognition.py`) was re-run after the change and all 33
tests still pass. This is the kind of thing you only catch by measuring,
which is the whole point of this document.

## Matching engine (Rust backend + Postgres)

`backend/examples/bench_matching.rs`: seeds a synthetic catalog directly
into Postgres via bulk insert (4,000 fingerprints/song, representative of
a several-minute track), then times `MatchingEngine::recognize` for 30
queries (300-fingerprint queries, representative of an 8s clip) per
catalog size tier. Catalog size is **cumulative** across tiers (each tier
adds to the previous one, so the "1,000" row is actually searching against
~1,110 songs) — this is intentional, it's closer to how a real catalog
grows than resetting between tiers.

| Catalog (songs, cumulative) | Seed time | Lookup+match p50 | p95 | p99 | Correctness |
|---|---|---|---|---|---|
| 10 | 0.6s | 1.8ms | 7.8ms | 11.8ms | 30/30 |
| ~110 | 7.0s | 2.2ms | 3.6ms | 4.5ms | 30/30 |
| ~1,110 | 76.1s | 3.8ms | 6.4ms | 7.9ms | 30/30 |
| ~7,338 (partial 10k tier, stopped early) | ~830s and climbing | *(not measured — see below)* | | | |

The 10,000-song tier was stopped partway through (at ~6,200 of the
planned 10,000 additional songs, ~7,338 total) rather than left running
indefinitely. **The reason is itself a real finding, not just "it was
slow":** `docker stats` on the benchmark's Postgres container showed block
I/O climbing to ~230GB written against a working set that should be well
under 1GB of raw fingerprint data — roughly 350x write amplification, and
seed throughput visibly degrading as the table grew (from ~7.6ms/song at
1,000 songs to well over 100ms/song by ~7,300). The likely cause: this
benchmark seeds via a per-song `UNNEST`-based `INSERT` (see
`FingerprintRepository::bulk_insert`) with **fully random** hash values,
which is close to a worst case for maintaining a B-tree index — every
insert touches a effectively-random leaf page, so index pages that would
stay hot/cached under realistic (more clustered, quantization-bucketed)
hash distributions instead churn constantly, on an out-of-the-box
`postgres:16-alpine` container with no `shared_buffers`/checkpoint tuning.
This is exactly why [docs/ingestion.md](ingestion.md) and
[database.md](database.md) insist real catalog ingestion uses `COPY`
(bulk-loads *outside* the index, which is then built/extended far more
efficiently) rather than incremental per-row/per-song inserts, and why
production Postgres needs real tuning (`shared_buffers`,
`maintenance_work_mem`, checkpoint settings) before loading a large
catalog — see [deployment.md](deployment.md). The 10/100/1,000-song tiers
above remain valid and were completed cleanly; extrapolating them to
100k+ songs without also fixing the seeding methodology (batch via `COPY`,
tune Postgres) would not be a trustworthy claim, so this document doesn't
make one.

Even at ~1,100 songs (~4.4M fingerprint rows), the full recognize path —
bulk `UNNEST` lookup, offset-histogram voting, scoring, and a song
metadata fetch — stays in single-digit milliseconds at p99. This matches
the design intent in [database.md](database.md): a covering B-tree index
on `(hash, algorithm_version)` comfortably serves this catalog range, and
the scaling path to 100k+ songs (partitioning, read replicas) is documented
there rather than needed yet.

**Debugging note**: the first run of this benchmark showed a suspicious
20/30 failure rate at the 100-song tier with `AmbiguousMargin` rejections.
Investigation traced it to a bug in the benchmark's own synthetic-hash RNG
seeding (reset to the same constant per tier, producing byte-identical
fingerprint sets for equivalently-indexed songs in different tiers, which
the matcher correctly refused to disambiguate) — a genuine reminder that
synthetic test data needs the same scrutiny as production data. Fixed by
using one continuing RNG stream for the whole run; see the comment in
`bench_matching.rs`.

## End-to-end evaluation (real HTTP, full stack)

`scripts/benchmarking/evaluate_robustness.py` drives the actual running
system over real HTTP — Android's own integration point — rather than
calling library code directly: ingests a small reference catalog through
the real ingestion CLI, generates realistic distortions of one reference
track, and posts each to `POST /api/v1/recognitions/audio` on a live
`docker compose` stack.

| Test case | Expected | Recognized | Score | Confidence | Matched FPs | Latency |
|---|---|---|---|---|---|---|
| original | Reference Track | ✅ Reference Track | 102.6 | 0.66 | 166 | 684ms |
| volume -10dB | Reference Track | ✅ Reference Track | 102.6 | 0.66 | 166 | 783ms |
| volume -20dB | Reference Track | ✅ Reference Track | 102.6 | 0.66 | 166 | 641ms |
| noise (light, σ=0.05) | Reference Track | ✅ Reference Track | 73.6 | 0.48 | 143 | 1005ms |
| noise (moderate, σ=0.15) | Reference Track | ✅ Reference Track | 60.9 | 0.42 | 120 | 1006ms |
| noise (heavy, σ=0.3) | Reference Track | ✅ Reference Track | 47.5 | 0.31 | 100 | 987ms |
| trimmed to 4s | Reference Track | ✅ Reference Track | 47.4 | 0.53 | 79 | 480ms |
| silence before/after | Reference Track | ✅ Reference Track | 63.3 | 0.48 | 172 | 1095ms |
| different start offset | Reference Track | ✅ Reference Track | 215.9 | 0.94 | 253 | 991ms |
| MP3 128kbps (real ffmpeg) | Reference Track | ✅ Reference Track | 102.6 | 0.66 | 166 | 980ms |
| unrelated white noise | *(none)* | ✅ NOT_RECOGNIZED | 0.0 | 0.0 | 0 | 978ms |

Raw data: `testdata/expected/evaluation_report.csv` (regenerated each run).
End-to-end latency here (~0.5–1.1s) is dominated by the HTTP round trip +
processor extraction + Python process/interpreter overhead for a
short-lived script making sequential requests, not by the matching engine
itself (which the table above shows completing in single-digit ms) — a
production client making one request at a time over a real network would
see broadly similar latency, dominated by upload time and processor
extraction, not database lookup.

**A real bug this harness caught**: the first run of this evaluation
showed the *original* (unmodified) clip failing to recognize while a
*different-offset* clip of the same track succeeded — backwards from what
should be possible (an exact clip should be at least as easy to recognize
as any other slice). Root cause: the harness had accidentally reused a
`docker compose` Postgres volume left over from a previous `make e2e` run,
and its synthetic reference track collided byte-for-byte with a leftover
test song generated with the same deterministic seed — a legitimate
ambiguous-match case (two near-identical songs), correctly rejected by the
matcher's margin check, but not the clean accuracy test intended. Fixed by
giving the harness its own distinctive synthetic song indices and making
it clear its own prior rows before each run (see `EVAL_ARTIST` /
`--database-url` cleanup in the script). Documented here because it's a
better illustration of "measure, don't assume" than a clean run would be.

## Known limitations of these measurements

- Single-machine, single-run measurements, not averaged across hardware or
  repeated over days — treat absolute numbers as indicative, not SLA-grade.
- Synthetic audio only (see [ingestion.md](ingestion.md) for why — no
  copyrighted music in this repo). Real-world recordings (phone mic
  through room acoustics, compressed streaming audio, etc.) will show
  different — likely somewhat lower — recognition scores than the clean
  synthetic signals used here, though the *mechanism* being validated
  (temporal consistency survives noise/compression/trimming) is the same.
- `score`/`confidence` are ranking heuristics, not calibrated
  probabilities (see [fingerprinting.md](fingerprinting.md) §9) — no
  calibration study against a labeled real-world query corpus has been
  done. That would be the natural next step before using `confidence` for
  anything user-facing beyond "recognized" vs. "not recognized".
- The matching benchmark's catalog is synthetic random hashes, not
  DSP-derived ones — it measures the lookup/voting/scoring *system*
  honestly, but not end-to-end extraction-to-match latency at scale (the
  evaluation harness above covers that, at a much smaller catalog size).

## How to reproduce

```bash
# Fingerprint extraction benchmark
cd processor && source .venv/bin/activate && python3 benchmarks/run_benchmarks.py

# Matching engine benchmark (needs a scratch Postgres — do not point this
# at a database you care about, it seeds and drops a "*_bench" database)
cd backend && DATABASE_URL=postgresql://postgres:devpass@localhost:5432/loony_shazam_bench \
  cargo run --release --example bench_matching
# Optional: BENCH_TIERS=10,100 to run a subset, BENCH_DEBUG=1 for per-miss diagnostics

# End-to-end evaluation (needs the stack running, e.g. `make dev`)
python3 scripts/benchmarking/evaluate_robustness.py \
  --backend-url http://localhost:8080 \
  --database-url postgresql://postgres:devpass@localhost:5432/loony_shazam
```

Machine used for the numbers above: Linux, the environment this repository
was developed in (see git history / CI for a second data point over time —
consider wiring this benchmark into CI as a regression check, it is not
currently gated on).

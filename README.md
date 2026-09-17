# Loony Shazam

A Shazam-style audio recognition system: record a few seconds of audio on
Android, identify the song via classic spectral-peak / landmark-hash
fingerprinting (Wang, 2003 — no neural networks in the recognition path),
and get back its metadata.

## 1. Overview

```
Android mic --> Rust API --> Python DSP service --> fingerprint index
                    |                                      |
                    +--------------- PostgreSQL <----------+
```

- **Android** (Kotlin, Jetpack Compose): captures audio, uploads it,
  displays the result, keeps local recognition history.
- **Rust backend** (Axum): HTTP API, validation, the matching engine
  (temporal offset voting, ranking, thresholds), database access, caching,
  rate limiting, observability.
- **Python processor** (FastAPI): the reference DSP implementation — STFT,
  peak detection, landmark hashing — run as a persistent service, not
  spawned per request.
- **PostgreSQL**: fingerprint index and song catalog. **Redis**: optional
  metadata cache, never on the fingerprint-lookup hot path.

See [docs/architecture.md](docs/architecture.md) for the full system design
and [docs/fingerprinting.md](docs/fingerprinting.md) for exactly how the
algorithm works, including the parameter-tuning data behind the defaults.

## 2. Requirements

Docker is convenient but **not required** — see "Running without Docker"
below for the alternative.

- Rust (stable, 2021 edition) + `cargo` — the backend
- Python 3.11+ — the processor / ingestion CLI
- PostgreSQL (any 14+ instance you can reach — Docker, a system package, a
  VM, a managed service) and, optionally, Redis
- Android Studio / JDK 17 + Android SDK (compileSdk 35) — for the Android app
- `psql` client (handy for inspecting the database; required by the
  no-Docker path's readiness check)
- Docker + Docker Compose (v2, the `docker compose` subcommand) — only if
  you want Postgres/Redis/processor/backend containerized

## 3. Quick start

**With Docker:**
```bash
cp .env.example .env
# edit .env and set a real ADMIN_API_KEY

make dev          # docker compose up --build: postgres, redis, processor, backend
```

**Without Docker** (uses Postgres/Redis you already have running):
```bash
cp .env.example .env
# edit .env: set DATABASE_URL to a Postgres you can already reach,
# REDIS_URL if you have one (optional), and a real ADMIN_API_KEY

make dev-local     # builds + starts processor and backend as local processes
# make dev-local-stop to stop them, make dev-local-logs to tail their output
```
`make dev-local` applies schema migrations automatically (the backend does
this on every startup, idempotently) — no separate migration step needed
for a fresh database. See [docs/deployment.md](docs/deployment.md)
"Running without Docker" for details and troubleshooting.

Either way, wait for `curl http://localhost:8080/ready` to report
`"status":"ready"`, then in another terminal:

```bash
# Generate a small synthetic test catalog (no copyrighted music — see
# docs/ingestion.md) and ingest it
python3 scripts/dev/generate_test_catalog.py /tmp/test_catalog --num-songs 3
cd processor && python3 -m venv .venv && source .venv/bin/activate && pip install -e ".[dev]"
DATABASE_URL=postgresql://postgres:devpass@localhost:5432/loony_shazam \
  music-fingerprint ingest /tmp/test_catalog

# Recognize a clip of one of them
curl -X POST http://localhost:8080/api/v1/recognitions/audio \
  -F "audio=@/tmp/test_catalog/Test Artist - Synthetic Alpha.wav;type=audio/wav"
```

Or run the whole thing (stack up, ingest, recognize, verify) as one
command: `make e2e`.

## 4. Running each component

`make dev-local` (see Quick Start) does the processor + backend steps
below for you in one command. This section is for running things
individually — useful for iterating on one component, or if you'd rather
not use the wrapper script at all.

### PostgreSQL / Redis
`docker compose up postgres redis` — or just point `DATABASE_URL`/
`REDIS_URL` at instances you already have running (a system package
install, a VM, a managed service). Nothing else in this repo assumes
Postgres/Redis are containerized. Migrations in `database/migrations` are
applied automatically by the backend on startup either way.

### Python processor
```bash
cd processor
python3 -m venv .venv && source .venv/bin/activate
pip install -e ".[dev]"
music-fingerprint serve --port 8001
```

### Rust backend
```bash
cd backend
DATABASE_URL=postgresql://postgres:devpass@localhost:5432/loony_shazam \
PROCESSOR_URL=http://localhost:8001 \
ADMIN_API_KEY=dev-admin-key \
cargo run
```

### Android app
See [docs/android.md](docs/android.md). Short version — with a device
connected (USB debugging on) or an emulator running, and the backend
already up (`make dev` / `make dev-local`):
```bash
make install-android    # builds, adb reverse's to the backend, installs, launches
```
The debug build talks to `http://localhost:8080/` on the device, forwarded
to this machine via `adb reverse` — that's what `install-android` sets up
automatically. Re-run it (or just `adb reverse tcp:8080 tcp:8080`) if the
device reboots or reconnects. `make uninstall-android` removes it.

## 5. Ingesting music

```bash
DATABASE_URL=postgresql://postgres:devpass@localhost:5432/loony_shazam \
  music-fingerprint ingest /path/to/your/licensed/music
# or: make ingest MUSIC_DIR=/path/to/your/licensed/music
```
Idempotent (safe to re-run), uses bulk `COPY` for fingerprint inserts. See
[docs/ingestion.md](docs/ingestion.md). **No copyrighted music is included
in or downloaded by this repository** — you provide files you have the
rights to.

## 6. Tests

```bash
make test                 # both of the below
make test-backend         # cargo test (needs Postgres — see tests/common/mod.rs)
make test-processor       # pytest
```
21 Rust tests (unit + HTTP-layer + matching-engine integration against a
real Postgres) and 33 Python tests (DSP unit tests, determinism/golden
fixtures, and noise/compression/offset robustness integration tests) all
currently pass — see [docs/performance.md](docs/performance.md) for what
was actually measured, not just asserted.

## 7. Benchmarks

```bash
make benchmark             # processor/benchmarks/run_benchmarks.py
cd backend && DATABASE_URL=... cargo run --release --example bench_matching
```
Results and analysis: [docs/performance.md](docs/performance.md).

## 8. API examples

See [docs/api.md](docs/api.md) for the full reference. The core call:
```bash
curl -X POST http://localhost:8080/api/v1/recognitions/audio \
  -F "audio=@clip.wav;type=audio/wav"
```

## 9. Configuration

Everything is environment-variable driven — see [.env.example](.env.example)
for the full list (sizes/timeouts/thresholds/secrets). Nothing is
hardcoded, nothing is committed as a real secret.

## 10. Troubleshooting

- **`backend` can't reach `processor`**: check `PROCESSOR_URL` — inside
  Docker Compose it must be the service name (`http://processor:8001`),
  never `localhost`.
- **`ADMIN_API_KEY` unset error on `docker compose up`**: copy
  `.env.example` to `.env` and set a value.
- **Recognition always returns `NOT_RECOGNIZED`**: confirm the catalog was
  actually ingested (`psql ... -c "select count(*) from songs;"`) and that
  the query clip actually overlaps a track you ingested — a completely
  unrelated clip is *supposed* to return `NOT_RECOGNIZED`, that's not a bug.
- **`cargo build` fails on an old Rust toolchain**: the Cargo.lock in this
  repo may resolve crates requiring a fairly recent stable Rust (edition
  2024 dependencies); update your toolchain (`rustup update stable`).
- **Android app can't reach the backend / recognition always errors out**:
  `adb reverse` rules don't survive a device reboot or USB reconnect —
  re-run `make install-android` (or just `adb reverse tcp:8080 tcp:8080`).
- **`make dev-local` fails at "could not connect using DATABASE_URL"**:
  Postgres isn't reachable at the host/port/credentials in `.env` — the
  script doesn't start Postgres for you (unlike `make dev`, which
  containerizes it). Start your own Postgres first, or use `make dev`.
- **`make dev-local` schema errors on a database that already has these
  tables** (e.g. you applied `database/migrations/*.sql` by hand first):
  the backend tracks applied migrations in `_sqlx_migrations` and expects
  to have been the one to create them — either let it manage the schema
  from an empty database, or see `docs/deployment.md` "Running without
  Docker" for how to back-fill that tracking table for a schema applied
  another way.

## Documentation index

- [docs/how-song-recognition-works.md](docs/how-song-recognition-works.md) — start here: the complete, from-first-principles explanation of how recognition actually works, start to finish
- [docs/architecture.md](docs/architecture.md) — system design, data flow, tradeoffs
- [docs/fingerprinting.md](docs/fingerprinting.md) — the algorithm, in depth
- [docs/database.md](docs/database.md) — schema and scaling path
- [docs/api.md](docs/api.md) — full endpoint reference
- [docs/ingestion.md](docs/ingestion.md) — catalog ingestion pipeline
- [docs/android.md](docs/android.md) — Android app architecture
- [docs/deployment.md](docs/deployment.md) — what production deployment needs beyond this repo
- [docs/performance.md](docs/performance.md) — measured benchmark results

## License

MIT — see [LICENSE](LICENSE). No copyrighted music is included in this
repository (see [docs/ingestion.md](docs/ingestion.md)).

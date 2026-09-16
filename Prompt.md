You are a principal software architect and senior full-stack engineer specializing in:
- audio fingerprinting
- digital signal processing
- distributed backend systems
- Rust
- Python
- Android/Kotlin
- high-performance search/indexing
- production-grade developer tooling

Your task is to build a complete, production-quality music recognition application similar in functionality to Shazam.

IMPORTANT:
Do not build a toy/demo implementation unless explicitly stated below.
Build the complete system described in this specification.

The product must:
1. Capture audio from an Android device microphone.
2. Upload/process a short audio sample.
3. Extract robust audio fingerprints.
4. Search a large fingerprint database.
5. Determine the matching song using temporal consistency / offset voting.
6. Return the song's metadata.
7. Display the result in a polished Android application.
8. Provide tooling for ingesting a licensed/local music catalog.
9. Include tests, benchmarks, documentation, observability, Docker support, and deployment configuration.

TECHNOLOGY REQUIREMENTS
========================

Backend:
- Rust
- Tokio
- Axum
- Serde
- SQLx
- PostgreSQL
- Redis where useful
- REST/JSON API
- Rust 2021 or newer stable edition
- Cargo workspace

Audio/data processing:
- Python 3.12+
- NumPy
- SciPy
- librosa and/or equivalent DSP libraries where appropriate
- soundfile
- pyFFTW where beneficial
- pytest
- typed Python code
- CLI tooling

Android:
- Kotlin
- Android Studio compatible project
- Jetpack Compose
- Kotlin Coroutines
- Android AudioRecord
- OkHttp/Retrofit or a similarly robust HTTP client
- Jetpack ViewModel
- Navigation Compose
- Material 3
- minimum Android version should be chosen sensibly and documented

Infrastructure:
- PostgreSQL
- Redis
- Docker
- Docker Compose for local development
- Prometheus-compatible metrics where practical
- structured logging
- health/readiness endpoints

Do NOT use:
- Node.js for backend
- TypeScript backend
- Firebase as the primary backend
- iOS implementation
- React Native
- Flutter
- a neural-network-only recognition system

The core recognition algorithm MUST be traditional audio fingerprinting based on spectral peaks / constellation maps / landmark hashes.

PRODUCT SCOPE
============

The application should behave conceptually like this:

Android microphone
        |
        v
Audio capture
        |
        v
Local preprocessing
        |
        v
5-12 second audio sample
        |
        v
Backend API
        |
        v
Audio preprocessing
        |
        v
STFT / spectrogram
        |
        v
Peak detection
        |
        v
Constellation map
        |
        v
Landmark fingerprint hashes
        |
        v
Fingerprint index lookup
        |
        v
Temporal offset voting
        |
        v
Candidate ranking
        |
        v
Confidence estimation
        |
        v
Song metadata
        |
        v
Android result screen

ARCHITECTURE
============

Create a monorepo with approximately this structure:

/
├── README.md
├── LICENSE
├── .gitignore
├── .env.example
├── docker-compose.yml
├── Makefile
├── justfile                    # optional if useful
│
├── backend/
│   ├── Cargo.toml
│   ├── Cargo.lock
│   ├── src/
│   │   ├── main.rs
│   │   ├── config.rs
│   │   ├── error.rs
│   │   ├── state.rs
│   │   ├── routes/
│   │   ├── handlers/
│   │   ├── services/
│   │   ├── repositories/
│   │   ├── models/
│   │   ├── matching/
│   │   ├── fingerprint/
│   │   ├── storage/
│   │   └── telemetry/
│   └── tests/
│
├── processor/
│   ├── pyproject.toml
│   ├── README.md
│   ├── src/
│   │   └── music_fingerprint/
│   │       ├── __init__.py
│   │       ├── audio.py
│   │       ├── spectrogram.py
│   │       ├── peaks.py
│   │       ├── constellation.py
│   │       ├── fingerprint.py
│   │       ├── hashing.py
│   │       ├── extraction.py
│   │       ├── ingestion.py
│   │       ├── validation.py
│   │       └── cli.py
│   ├── tests/
│   └── benchmarks/
│
├── android/
│   ├── settings.gradle.kts
│   ├── build.gradle.kts
│   ├── gradle.properties
│   ├── app/
│   │   ├── build.gradle.kts
│   │   └── src/
│   │       ├── main/
│   │       │   ├── AndroidManifest.xml
│   │       │   └── java/...
│   │       └── test/
│
├── database/
│   ├── migrations/
│   └── seed/
│
├── scripts/
│   ├── dev/
│   ├── ingestion/
│   ├── benchmarking/
│   └── deployment/
│
├── testdata/
│   ├── audio/
│   ├── fingerprints/
│   └── expected/
│
├── docs/
│   ├── architecture.md
│   ├── fingerprinting.md
│   ├── api.md
│   ├── database.md
│   ├── android.md
│   ├── ingestion.md
│   ├── deployment.md
│   └── performance.md
│
└── infra/
    ├── docker/
    └── monitoring/

You may improve this structure if you have a strong technical reason, but maintain clear separation between:
- Android client
- Rust backend
- Python DSP/data-processing pipeline
- database
- infrastructure
- tests
- documentation

CORE AUDIO FINGERPRINTING SYSTEM
================================

Implement a Shazam-style fingerprinting algorithm.

Do NOT compare raw audio waveforms.

The system should use:

1. audio normalization
2. mono conversion
3. resampling
4. STFT
5. magnitude spectrogram
6. optional log scaling
7. local spectral peak detection
8. constellation map
9. peak pairing / landmark generation
10. compact deterministic hash generation
11. fingerprint storage
12. inverted index lookup
13. temporal offset voting
14. candidate scoring
15. confidence estimation

Design this carefully.

AUDIO NORMALIZATION
-------------------

Normalize all reference and query audio into a consistent representation.

Recommended starting configuration:

sample rate:
    11025 Hz or 16000 Hz

channels:
    mono

sample format:
    float32 internally

STFT:
    configurable FFT size
    configurable hop size
    configurable window

Do NOT hard-code these values throughout the application.

Create a central configuration structure.

Example configuration concept:

sample_rate = 11025
fft_size = 4096
hop_size = 512
min_frequency = 40
max_frequency = 5000
peak_neighborhood = ...
target_zone_time = ...
fanout = ...

Tune parameters experimentally and document why the defaults were selected.

SPECTROGRAM
-----------

Compute the STFT magnitude spectrum.

Use numerically stable processing.

Consider:
- Hann window
- logarithmic magnitude
- dB conversion where appropriate
- numerical floor
- frequency filtering

Do not unnecessarily preserve frequencies that contribute little to recognition.

PEAK DETECTION
--------------

Detect strong local maxima in the spectrogram.

The detector should:
- suppress weak peaks
- enforce local time/frequency neighborhoods
- prevent excessive peak density
- retain stable dominant spectral components
- be configurable

Represent a peak approximately as:

Peak {
    time_bin
    frequency_bin
    amplitude
}

or equivalent.

CONSTELLATION MAP
-----------------

Build a sparse constellation map from the strongest spectral peaks.

The algorithm should be deterministic.

Do not depend on machine learning.

LANDMARK GENERATION
-------------------

Generate fingerprint landmarks by pairing an anchor peak with nearby target peaks.

Conceptually:

anchor:
    (t1, f1)

target:
    (t2, f2)

fingerprint:
    hash(f1, f2, delta_t)

Store the anchor time separately.

Use a configurable target zone and fan-out.

Avoid generating pathological numbers of fingerprints.

The implementation must protect against:
- extremely dense peak regions
- silence
- clipping
- malformed audio
- huge files
- adversarially large inputs

HASH DESIGN
-----------

Use a compact deterministic representation.

For example, encode quantized:
- anchor frequency
- target frequency
- delta time

into a fixed-width integer or compact byte representation.

Hash collisions should be minimized.

Document:
- bit allocation
- quantization
- expected collision behavior
- versioning

IMPORTANT:
Fingerprint hashes need a version.

Example:

fingerprint_algorithm_version = 1

If the algorithm changes, old fingerprints must not silently become incompatible.

DATABASE
========

Use PostgreSQL as the system of record.

Design tables similar to:

songs
-----
id
title
artist
album
album_artist
duration_ms
release_date
isrc
artwork_url
source
created_at
updated_at

fingerprints
------------
hash
song_id
offset_ms

metadata should be normalized appropriately.

Do not blindly create a PostgreSQL row for every fingerprint if a more efficient representation is practical.

The expected fingerprint count can become extremely large.

Design the storage layer with scale in mind.

Consider:
- BYTEA / BIGINT representation for hashes
- indexes
- partitioning
- bulk COPY ingestion
- composite indexes
- table clustering
- PostgreSQL hash/B-tree indexes where appropriate
- read replicas later
- Redis caching where useful

The database design must be able to evolve from:
10 songs
to:
1,000 songs
to:
100,000+ songs

without requiring a total rewrite.

FINGERPRINT INDEX
=================

The critical query is:

Given a set of query fingerprints:

    hash_1
    hash_2
    hash_3
    ...

find reference fingerprints with matching hashes.

Return:

(hash, song_id, reference_offset)

Then calculate:

offset = reference_offset - query_offset

For each song, build an offset histogram.

Example:

song A:
    +12.1 sec -> 1 vote
    +12.2 sec -> 18 votes
    +12.3 sec -> 4 votes

song B:
    +41.0 sec -> 2 votes

Song A has strong temporal consistency.

MATCHING ENGINE
===============

Implement the matching engine in Rust.

It should:

1. receive query fingerprints
2. perform indexed lookup
3. group matches by song
4. calculate temporal offsets
5. construct an offset histogram
6. identify dominant offset clusters
7. calculate score
8. calculate confidence
9. return top candidates

The matching algorithm must NOT simply count total hash matches.

Temporal consistency is essential.

Use configurable histogram resolution, e.g.:

offset_bucket_ms = 100

or another sensible value.

Consider tolerance for:
- microphone latency
- preprocessing differences
- small clock drift
- imperfect peak detection

MATCH SCORE
===========

Design a scoring function that considers at least:

- number of matching fingerprints
- unique query fingerprints matched
- dominant offset votes
- ratio of dominant votes to total votes
- coverage of query duration
- quality/strength of matched peaks if available
- candidate uniqueness
- collision penalties

Do not expose a fake "99.9% confidence" unless the value has been calibrated.

Call it a score or confidence only when appropriately justified.

Document what confidence means.

UNKNOWN RESULT
==============

The system must support:

"Song not recognized"

Do not force a match.

Use thresholds based on:
- minimum matching fingerprints
- minimum dominant offset votes
- minimum score
- minimum coverage
- margin over second-best candidate

These thresholds must be configurable.

QUERY PROCESSING
=================

The Android app should record approximately 5-12 seconds.

Prefer local preprocessing if practical, but keep the authoritative fingerprint extraction compatible with the Python reference implementation.

You need to decide whether the Android client:
A. sends PCM/WAV/encoded audio to Rust
or
B. generates fingerprints locally.

For the first production iteration:

Use:
Android:
    capture compressed or PCM audio
    send audio sample

Rust:
    accept the audio
    pass it to the Python fingerprint service OR use a clearly defined processing boundary

Python:
    perform DSP/fingerprint extraction

Rust:
    perform database lookup and matching

However, do NOT make Rust spawn a Python process for every request.

Design a persistent Python processing service.

Recommended architecture:

Android
  |
  | HTTPS
  v
Rust API
  |
  | internal RPC/HTTP
  v
Python fingerprint service
  |
  v
fingerprints
  |
  v
Rust matching service
  |
  v
PostgreSQL / Redis

Define the internal protocol cleanly.

For performance, consider gRPC or another efficient internal protocol.

The Python service should support:
- health check
- fingerprint extraction
- batch extraction
- algorithm version
- validation errors

INGESTION PIPELINE
==================

Create a Python CLI capable of ingesting a directory of licensed/local audio.

Example:

python -m music_fingerprint ingest ./music

It should:

1. discover audio files
2. validate files
3. decode audio
4. normalize
5. calculate duration
6. extract fingerprints
7. generate song metadata
8. upload metadata to Rust/backend or directly to a controlled ingestion API
9. bulk insert fingerprints
10. report progress
11. support resume/retry
12. avoid duplicate ingestion
13. produce useful logs

Support common formats where practical:
- WAV
- FLAC
- MP3
- AAC/M4A if decoding libraries permit

Do not assume all audio is valid.

Handle:
- corrupted files
- unsupported codecs
- zero-length audio
- silence
- extremely long tracks
- duplicate files

INGESTION SHOULD BE IDEMPOTENT.

If the same file is ingested twice, it should not create duplicate catalog entries.

Calculate a stable source/file identity, such as:
- SHA-256 of file
- source ID
- metadata combination

Make this configurable.

BULK INSERT
===========

Do not perform:

INSERT INTO fingerprints (...) VALUES (...)
one row at a time

for millions of fingerprints.

Use efficient bulk insertion.

PostgreSQL COPY is preferred.

Build ingestion batching.

Provide:
- batch size configuration
- transaction boundaries
- retry handling
- progress reporting

BACKEND API
===========

Build a clean REST API.

Suggested endpoints:

GET /health
GET /ready

POST /api/v1/recognitions
POST /api/v1/recognitions/audio

GET /api/v1/songs/{id}

GET /api/v1/recognitions/{id}

POST /api/v1/admin/songs
POST /api/v1/admin/ingest

Exact endpoint design can be improved.

Recognition endpoint should accept an audio sample and return:

{
  "recognized": true,
  "song": {
    "id": "...",
    "title": "...",
    "artist": "...",
    "album": "...",
    "duration_ms": 123456,
    "artwork_url": "..."
  },
  "match": {
    "score": 123.4,
    "confidence": 0.94,
    "matched_fingerprints": 82,
    "query_fingerprints": 141,
    "offset_ms": 42130,
    "algorithm_version": 1
  }
}

For unknown:

{
  "recognized": false,
  "reason": "NO_MATCH",
  "match": {
    ...
  }
}

Do not leak internal database details.

API SECURITY
============

Implement sensible production security.

At minimum:
- request size limits
- audio duration limits
- MIME validation
- timeout limits
- rate limiting architecture
- structured error responses
- authentication for admin endpoints
- separate public recognition endpoints from admin endpoints
- secrets loaded from environment variables
- no secrets committed to Git

For development, provide simple local configuration.

For production, document:
- API authentication
- TLS
- reverse proxy
- rate limiting
- abuse prevention

AUDIO SECURITY
==============

Treat uploaded audio as untrusted input.

Protect against:
- enormous files
- decompression bombs
- malicious metadata
- unsupported codecs
- path traversal
- shell injection
- resource exhaustion

Never execute user-provided filenames as shell commands.

Never trust:
- MIME type
- filename extension
- duration metadata

Validate actual decoded audio properties.

RUST BACKEND DESIGN
===================

Use idiomatic Rust.

Use:
- Axum
- Tokio
- tracing
- thiserror
- anyhow only where appropriate
- serde
- sqlx
- tower middleware
- tower-http where useful

Separate:
- transport layer
- business logic
- repository layer
- matching engine
- configuration
- telemetry

Avoid putting business logic inside HTTP handlers.

Create domain types.

Do not use strings everywhere.

Examples:
SongId
FingerprintHash
AlgorithmVersion
DurationMs

Use strongly typed representations where useful.

Handle errors explicitly.

No unwrap() in production request paths unless mathematically/safely justified.

ANDROID APP
===========

Build a native Kotlin Android application.

Use Jetpack Compose.

Main screen should have:

- app name
- large microphone/listen button
- listening animation/state
- permission handling
- recent recognition history

States:

IDLE
LISTENING
PROCESSING
RESULT
NOT_FOUND
ERROR

Flow:

User taps microphone
        ↓
request RECORD_AUDIO permission
        ↓
capture 5-12 sec
        ↓
show listening UI
        ↓
upload
        ↓
show processing UI
        ↓
receive recognition result
        ↓
show song

RESULT SCREEN
=============

Display:

- album artwork
- song title
- artist
- album
- match status
- recognized time
- optional offset

Provide buttons for:
- search again
- share result
- open external music service if configured
- add to history/favorite

Do not implement Spotify/Apple Music integration unless it can be done cleanly without requiring unavailable credentials.

Create an abstraction:

MusicProvider

with future implementations possible.

HISTORY
=======

Persist recognition history locally on Android.

Use Room.

Store:
- song ID
- title
- artist
- album
- artwork URL
- recognized timestamp

History screen:
- list recognitions
- empty state
- tap to open details

No account system is required for the initial version.

AUDIO CAPTURE
=============

Use Android AudioRecord.

Build a robust audio recorder abstraction.

Requirements:
- correct microphone permission handling
- detect unavailable microphone
- buffer safely
- stop recording cleanly
- handle lifecycle changes
- avoid memory leaks
- avoid blocking the UI thread

Prefer coroutines.

Implement configurable:
- sample rate
- channel count
- PCM format
- max recording duration

The app should gracefully degrade if a particular sample rate isn't supported.

NETWORKING
==========

Implement:
- HTTP timeouts
- upload progress if useful
- cancellation
- retry policy only where safe
- offline error handling
- server error handling
- malformed response handling

Do not retry recognition uploads indefinitely.

CONFIGURATION
=============

The Android API base URL must be configurable.

Development:
http://10.0.2.2:...

Production:
HTTPS URL

Do not hard-code production secrets.

Provide build configurations if useful:
- debug
- release

TESTING
=======

Testing is a first-class requirement.

PYTHON TESTS
============

Create deterministic unit tests for:

- audio loading
- resampling
- mono conversion
- STFT
- peak detection
- constellation map
- hash generation
- fingerprint determinism
- silence handling
- clipping handling

Create integration tests:

Reference audio
      ↓
fingerprint
      ↓
insert into test DB
      ↓
query modified audio
      ↓
recognition

Test transformations:

1. original audio
2. volume change
3. white noise
4. speech/noise overlay
5. MP3 compression
6. resampling
7. truncation
8. different starting offsets
9. silence before music
10. silence after music

The original should reliably match.

The modified versions should match above a documented threshold.

Unrelated audio should not match.

RUST TESTS
==========

Test:
- API validation
- repository layer
- fingerprint lookup
- offset histogram
- ranking
- confidence
- unknown detection
- malformed requests
- size limits

ANDROID TESTS
=============

Include:
- ViewModel tests
- repository tests
- recorder tests where feasible
- UI tests for major states
- permission behavior where practical

END-TO-END TEST
===============

Create an end-to-end test:

1. Start PostgreSQL
2. Start Redis if required
3. Start Python processor
4. Start Rust backend
5. Ingest test catalog
6. Send query audio
7. Verify expected song
8. Verify unknown audio returns unknown

Make this runnable with one command.

BENCHMARKING
============

Create benchmarks for fingerprint extraction.

Measure:

- decode time
- preprocessing time
- STFT time
- peak detection time
- fingerprint generation time
- database lookup time
- matching time
- end-to-end latency

Test catalog sizes:
- 10 songs
- 100 songs
- 1,000 songs
- 10,000 songs where practical

Record:
- p50
- p95
- p99 latency
- memory usage
- fingerprints generated per second

Do not make performance claims without measurement.

Create a performance report documenting actual benchmark results.

CACHING
=======

Use Redis only where it materially improves performance.

Potential candidates:
- frequently queried fingerprint hashes
- song metadata
- recognition result caching if appropriate

Do not introduce Redis unnecessarily into the critical path.

The database remains authoritative.

OBSERVABILITY
=============

Rust:
- tracing
- JSON structured logs
- request IDs
- recognition latency
- DB latency
- processor latency
- error counters

Python:
- structured logs
- processing latency
- audio decode failures
- fingerprint counts

Metrics where practical:
- requests_total
- recognition_success_total
- recognition_failure_total
- recognition_latency
- fingerprint_lookup_latency
- fingerprint_extraction_latency
- db_query_latency

Never log raw microphone audio.

Never log user-uploaded audio contents.

PRIVACY
=======

The system should minimize retention of user query audio.

Default behavior:
- process audio
- extract fingerprints
- discard uploaded query audio
- retain only recognition metadata/history where appropriate

Document this behavior.

Do not store microphone recordings by default.

DATABASE MIGRATIONS
===================

Use SQL migrations.

Include:
- initial schema
- indexes
- constraints
- future-proof algorithm version support

Foreign keys should be used appropriately.

Add indexes based on actual query patterns.

DOCUMENTATION
=============

Write excellent documentation.

README must include:

1. Project overview
2. Architecture
3. Requirements
4. Quick start
5. Running PostgreSQL
6. Running Redis
7. Running Python fingerprint service
8. Running Rust backend
9. Running Android app
10. Ingesting music
11. Running tests
12. Running benchmarks
13. API examples
14. Configuration
15. Troubleshooting

Create:

docs/architecture.md

Explain the complete system.

Create:

docs/fingerprinting.md

Explain:
- STFT
- spectrogram
- peak detection
- constellation map
- landmark hashes
- inverted index
- temporal offset voting
- candidate ranking

Include diagrams using Mermaid where appropriate.

Create:

docs/performance.md

Include measured benchmark results.

Create:

docs/api.md

Document all public/admin endpoints.

Create:

docs/ingestion.md

Explain how to legally provide a licensed/local catalog and ingest it.

IMPORTANT COPYRIGHT REQUIREMENT
================================

Do not include commercial copyrighted music in the repository.

Do not download copyrighted songs automatically.

Use:
- generated test signals
- public-domain audio
- user-owned/licensed audio
- tiny synthetic fixtures where appropriate

The ingestion pipeline should operate on files supplied by the user/operator.

Do not build scraping/downloading functionality for commercial music catalogs.

ALGORITHM VERSIONING
====================

Every fingerprint must include an algorithm version.

For example:

algorithm_version = 1

The backend must reject incompatible fingerprint versions or handle them explicitly.

The database should support multiple versions if necessary.

API responses should report the algorithm version used.

CONFIGURATION
=============

Use environment variables for deployment settings.

Example:

DATABASE_URL=
REDIS_URL=
RUST_LOG=
PROCESSOR_URL=
API_HOST=
API_PORT=
MAX_AUDIO_BYTES=
MAX_AUDIO_DURATION_SECONDS=
FINGERPRINT_ALGORITHM_VERSION=

Do not commit .env files containing secrets.

Provide .env.example.

DOCKER
======

Create Dockerfiles for:

- Rust backend
- Python processor

Create docker-compose.yml for local development.

Services:

postgres
redis
processor
backend

Android runs separately.

Add health checks.

Ensure service startup ordering is robust.

Do not assume localhost between containers.

Use Docker service names.

CI/CD
=====

Create a GitHub Actions workflow.

At minimum:

On pull request:
- Rust formatting
- Rust clippy
- Rust tests
- Python formatting/linting
- Python tests
- Android compilation/tests
- basic Docker build

Suggested tools:
Rust:
    cargo fmt --check
    cargo clippy
    cargo test

Python:
    ruff
    pytest

Android:
    ./gradlew test
    ./gradlew assembleDebug

Do not make CI depend on proprietary services.

CODE QUALITY
============

Follow these principles:

- clear naming
- small modules
- explicit interfaces
- deterministic algorithms
- type safety
- meaningful comments
- no unnecessary abstraction
- no premature microservices
- no copy-paste architecture
- no dead code
- no fake implementations
- no TODO placeholders for core functionality

If you need to make a tradeoff, document it.

DO NOT:
- pretend something is production-ready when it isn't
- silently ignore errors
- silently fall back to an inferior algorithm
- hard-code paths
- hard-code credentials
- use global mutable state unnecessarily
- write enormous functions
- put everything in main.rs
- put all Python logic in one file
- put all Android logic in one Activity

DEVELOPMENT WORKFLOW
====================

Before writing code:

STEP 1 — INSPECT
----------------

Inspect the entire repository.

Determine:
- existing files
- existing project structure
- existing code
- build systems
- available tooling
- existing tests
- existing configuration

Do not destroy existing work without understanding it.

If this is an empty repository, create the project structure.

STEP 2 — ARCHITECT
------------------

Create/update:

docs/architecture.md

First establish:
- component boundaries
- data flow
- APIs
- database schema
- fingerprint representation
- internal processor protocol
- Android architecture

Then implement.

STEP 3 — BUILD VERTICAL SLICE
-----------------------------

Do not implement every UI screen first.

First make this complete path work:

test audio
  ↓
Python fingerprint extraction
  ↓
database
  ↓
Rust matcher
  ↓
Rust recognition API
  ↓
Android request
  ↓
Android result

Get this working end-to-end.

STEP 4 — HARDEN
---------------

Then add:
- error handling
- validation
- security
- rate limiting
- caching
- observability
- retries
- lifecycle handling
- database optimizations

STEP 5 — TEST
-------------

Run all tests.

Fix failures instead of merely reporting them.

STEP 6 — BENCHMARK
------------------

Run benchmarks.

Optimize actual bottlenecks.

Do not optimize based solely on assumptions.

STEP 7 — DOCUMENT
-----------------

Update all documentation based on the final implementation.

STEP 8 — FINAL VALIDATION
-------------------------

Run:

Rust:
    cargo fmt --check
    cargo clippy --all-targets --all-features -- -D warnings
    cargo test --all

Python:
    ruff check .
    pytest

Android:
    ./gradlew test
    ./gradlew assembleDebug

Docker:
    docker compose config
    docker compose build

Then run an actual end-to-end recognition test.

IMPLEMENTATION ORDER
====================

Implement in this order unless repository constraints justify a change:

1. repository inspection
2. architecture document
3. database migrations
4. Python DSP core
5. Python fingerprint tests
6. Python fingerprint service
7. Rust project
8. Rust database layer
9. Rust matching engine
10. Rust API
11. ingestion API
12. Docker environment
13. end-to-end backend recognition
14. Android project
15. Android audio recorder
16. Android networking
17. Android recognition screen
18. Android history
19. error handling
20. observability
21. security hardening
22. benchmarks
23. CI
24. documentation
25. final end-to-end validation

PYTHON/RUST RESPONSIBILITY BOUNDARY
====================================

Keep responsibilities explicit.

Python owns:
- audio decoding where required
- DSP
- STFT
- spectrogram generation
- peak detection
- constellation map
- fingerprint generation
- ingestion processing

Rust owns:
- HTTP API
- authentication/authorization
- request validation
- orchestration
- fingerprint lookup
- matching
- temporal voting
- ranking
- metadata
- database access
- caching
- rate limiting
- observability

Android owns:
- microphone capture
- permissions
- UI
- network communication
- local history
- user interaction

Avoid implementing the same fingerprint algorithm independently in Rust and Python unless there is a compelling measured reason.

REFERENCE IMPLEMENTATION
=========================

The Python implementation is the reference implementation for fingerprint generation.

It must be deterministic.

Given the same normalized audio and configuration:

fingerprint(audio) == fingerprint(audio)

across runs.

Create golden test fixtures.

Store expected fingerprints for small synthetic/reference audio examples.

If a later optimization changes fingerprints, require an explicit algorithm version change.

SEARCH OPTIMIZATION
===================

The recognition query may contain hundreds or thousands of fingerprint hashes.

Do not generate an enormous SQL query containing thousands of OR clauses.

Use an efficient strategy.

Consider:
- temporary tables
- PostgreSQL UNNEST
- bulk parameter arrays
- COPY to temporary staging table
- joining against fingerprint index
- batching
- prepared statements

Benchmark alternatives.

Use the measured result.

For large catalogs, consider a dedicated inverted-index abstraction.

Design the Rust matcher so storage implementation can later be replaced without rewriting the ranking logic.

MATCHING PSEUDOCODE
===================

Implement approximately:

query_fingerprints = fingerprint(query_audio)

matches = lookup(query_fingerprints)

votes = {}

for match in matches:
    offset = match.reference_offset - match.query_offset

    bucket = quantize(offset)

    votes[(match.song_id, bucket)] += 1

rank candidates using:
    dominant_votes
    total_votes
    query_coverage
    unique_hashes
    score
    second_best_margin

select candidate only if:
    score >= threshold
    dominant_votes >= threshold
    coverage >= threshold
    confidence/margin criteria pass

otherwise:
    NOT_RECOGNIZED

Improve this design as necessary.

AUDIO ROBUSTNESS
================

The system must attempt to recognize recordings played through:
- laptop speakers
- phone speakers
- headphones into another microphone
- moderate environmental noise

Do not claim perfect recognition.

Build an evaluation harness.

Generate test variants from reference audio.

Example:

reference.wav

variants:
reference_volume_-10db.wav
reference_noise_10db.wav
reference_noise_0db.wav
reference_mp3_128k.mp3
reference_trimmed.wav
reference_with_silence.wav

Run the recognizer against all variants.

Produce an evaluation report:

test_case
expected_song
recognized_song
recognized
score
matched_fingerprints
latency_ms

ACCEPTANCE CRITERIA
===================

The project is complete only when:

1. `docker compose up` starts backend dependencies.
2. Python processor starts successfully.
3. Rust backend starts successfully.
4. Database migrations execute successfully.
5. A local/licensed test catalog can be ingested.
6. Fingerprints are stored.
7. Android can record audio.
8. Android can send audio to backend.
9. Backend extracts fingerprints.
10. Backend searches fingerprint index.
11. Matcher correctly identifies known test songs.
12. Unknown audio can return NOT_RECOGNIZED.
13. Noise/compression tests exist.
14. Rust tests pass.
15. Python tests pass.
16. Android tests/build pass.
17. CI configuration exists.
18. Documentation is complete.
19. No copyrighted music is included.
20. No secrets are committed.
21. No major TODOs remain in core functionality.
22. End-to-end recognition works on a clean checkout.

DEVELOPER EXPERIENCE
====================

Provide simple commands.

For example:

make dev

make test

make ingest MUSIC_DIR=./testdata/audio

make benchmark

make lint

make e2e

Android:

./gradlew assembleDebug

Document exact commands in README.

FINAL RESPONSE
==============

At the end of implementation, provide a concise engineering report containing:

1. What was implemented
2. Final architecture
3. Fingerprinting algorithm
4. Database design
5. API endpoints
6. Android features
7. Test results
8. Benchmark results
9. Known limitations
10. How to run locally
11. How to ingest music
12. Recommended next improvements

Do not claim successful tests unless you actually ran them.

If a test cannot run because a dependency/tool is unavailable, explicitly state:
- what failed
- why
- what was still validated

IMPORTANT EXECUTION BEHAVIOR
============================

You are operating as an autonomous senior engineer.

Do not stop after creating scaffolding.

Do not merely provide instructions.

Actually create and modify the files.

After each major implementation stage:
- run relevant tests
- inspect failures
- fix them
- continue

When you discover an architectural problem:
- explain it briefly
- choose the technically sound solution
- implement it
- update documentation

Prefer a working vertical slice over large amounts of unfinished code.

Prioritize correctness of the fingerprinting/matching algorithm above visual polish.

Do not ask me to make routine engineering decisions.
Make reasonable senior-level decisions yourself and document important ones.

Only ask for clarification if a decision genuinely requires information that cannot reasonably be inferred.

Begin now with repository inspection.
# Database Design

PostgreSQL is the system of record. Migrations live in
[`database/migrations`](../database/migrations) as plain versioned SQL
files, applied with `sqlx migrate run` (also invoked automatically by the
backend on startup in development — see `backend/src/storage`).

## Schema

### `songs`
One row per distinct recording. `id` is a `BIGSERIAL` used as the stable
foreign key everywhere else; it is exposed to clients as an opaque string.

### `fingerprints`
The hot table. One row per landmark hash: `(hash, song_id, offset_ms,
algorithm_version)`. Deliberately has **no surrogate primary key** — at
catalog scale (100k+ songs) this table holds hundreds of millions to
billions of rows, and a redundant identity column plus its index would cost
real disk and cache pressure for no query benefit. It is populated
exclusively through `COPY` (see `processor`'s ingestion CLI and
`backend`'s admin ingest endpoint), never row-by-row `INSERT`.

The single index that matters is
`idx_fingerprints_hash_lookup (hash, algorithm_version) INCLUDE (song_id,
offset_ms)` — a covering index so the recognition hot path
(`WHERE hash = ANY($1) AND algorithm_version = $2`) is satisfiable as an
index-only scan without touching the heap.

### `ingested_sources`
Idempotency ledger for the ingestion CLI. Keyed by `sha256` of the source
file's bytes (not filename/mtime, which are unreliable identity signals).
Re-running ingestion over a directory skips any file whose hash is already
present.

### `recognitions`
Append-only audit log of recognition attempts (metadata only — the query
audio itself is never persisted, see the privacy section of
[architecture.md](architecture.md)). Used for observability and to power
future "trending" features; not required for the recognition path itself.

## Scaling path: 10 → 1,000 → 100,000+ songs

| Catalog size | Approx. fingerprint rows | Strategy |
|---|---|---|
| 10 | ~10⁵ | Default schema as-is; fits entirely in shared_buffers. |
| 1,000 | ~10⁷ | Default schema; index fits in RAM on a modest instance. `VACUUM`/`ANALYZE` tuning starts to matter. |
| 100,000+ | ~10⁹ | Convert `fingerprints` to a `LIST` partition on `algorithm_version` (there are only ever a handful of live versions), optionally `HASH` sub-partitioned on `hash` to bound individual partition/index size. Move to read replicas for the lookup path; ingestion writes stay on the primary. Consider `pg_repack`/`CLUSTER` on the hash index periodically to keep it physically ordered. |

This is an additive migration path — nothing in the application layer
(`FingerprintIndex` trait in the Rust matcher) assumes an unpartitioned
table, so partitioning can be introduced later without an application
rewrite.

## Why Postgres and not a bespoke index from day one

A B-tree covering index on `(hash, algorithm_version)` already gives
O(log n) lookup per hash with hundreds of query hashes batched into one
`UNNEST`-based query — this comfortably serves the stated 100k-song target
range. A dedicated in-memory inverted index (e.g. a custom Rust structure
sharded across processes) would remove a network hop but introduces its own
replication/durability problem that Postgres solves for free. The
`FingerprintIndex` trait exists specifically so this can change later
without touching scoring/ranking code.

## Redis usage

Redis is used only where it removes load from Postgres without becoming
the source of truth:
- `song:{id}` metadata cache (TTL, invalidated on admin song updates).
- Optional recognition result cache keyed by a fingerprint-set fingerprint,
  for identical repeated queries within a short window (e.g. a demo booth
  replaying the same clip).

Redis is never consulted for the fingerprint lookup itself — that always
goes to Postgres, which remains authoritative.

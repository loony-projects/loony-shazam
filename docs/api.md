# API Reference

Base URL: `http://localhost:8080` in local dev (`docker compose`), a real
HTTPS URL in production (see [deployment.md](deployment.md)). All request/
response bodies are JSON except the audio upload, which is
`multipart/form-data`.

Error responses always look like:
```json
{ "error": "SOME_CODE", "message": "human-readable description" }
```
Internal errors (`AppError::Internal`/`Database`) always return a generic
message — SQL errors, file paths, and stack traces never reach the client
(see `backend/src/error.rs`).

## Public endpoints

### `GET /health`
Liveness only — never touches the database or processor. Always `200` if
the process is up.
```json
{ "status": "ok" }
```

### `GET /ready`
Readiness — checks Postgres and the processor service are both reachable.
Redis is reported but never required (it's an optional cache, see
[database.md](database.md)).
```json
{ "status": "ready", "database": "ok", "processor": "ok", "redis": "disabled" }
```
Returns `503` with `status: "not_ready"` if either the database or the
processor is unreachable.

### `GET /metrics`
Prometheus text-format exposition (`requests_total`,
`recognition_success_total`, `recognition_failure_total{reason=...}`,
`recognition_latency_ms`, `fingerprint_lookup_latency_ms`,
`fingerprint_extraction_latency_ms`, `db_query_latency_ms{query=...}`).

### `POST /api/v1/recognitions/audio`
The primary recognition path. `multipart/form-data` with one field named
`audio` (any format the processor can decode — WAV/FLAC/MP3 directly via
libsndfile, AAC/M4A via an `ffmpeg` fallback). Capped at `MAX_AUDIO_BYTES`
(default 20MB) and `MAX_AUDIO_DURATION_SECONDS` (default 15s, enforced by
the processor using the value the backend passes through).

```bash
curl -X POST http://localhost:8080/api/v1/recognitions/audio \
  -F "audio=@clip.wav;type=audio/wav"
```

Recognized:
```json
{
  "recognized": true,
  "recognition_id": "42",
  "song": {
    "id": "3",
    "title": "Song Alpha",
    "artist": "Test Artist",
    "album": null,
    "duration_ms": 20000,
    "artwork_url": null
  },
  "match": {
    "score": 119.46,
    "confidence": 0.49,
    "matched_fingerprints": 145,
    "query_fingerprints": 294,
    "offset_ms": 9000,
    "algorithm_version": 1,
    "latency_ms": 666
  }
}
```

Not recognized (still HTTP `200` — this is a normal, expected outcome, not
an error):
```json
{
  "recognized": false,
  "recognition_id": "43",
  "reason": "NO_MATCH",
  "match": { "score": 0.0, "confidence": 0.0, "matched_fingerprints": 0, "query_fingerprints": 0, "offset_ms": null, "algorithm_version": 1, "latency_ms": 649 }
}
```

See [fingerprinting.md](fingerprinting.md) "Scoring" for what `score` and
`confidence` do and don't mean. `song.id` is a string (stringified
internal ID).

Errors: `422 MISSING_AUDIO` / `INVALID_AUDIO` / `PROCESSOR_REJECTED`,
`413 AUDIO_TOO_LARGE`, `502 PROCESSOR_UNAVAILABLE`, `409
ALGORITHM_VERSION_MISMATCH`, `429 RATE_LIMITED`.

Both this endpoint and `/api/v1/recognitions` (below) include a
`recognition_id` field (a stringified id) in every response — pass it to
`GET /api/v1/recognitions/{id}` to retrieve the same outcome again later.

### `POST /api/v1/recognitions`
Same matching logic, but accepts already-extracted fingerprints directly as
JSON instead of raw audio — bypasses the processor round trip entirely.
Used by tests and programmatic/offline clients; the Android app always
uses `/recognitions/audio`.

```json
{
  "algorithm_version": 1,
  "fingerprints": [{ "hash": 6000, "offset_ms": 0 }, { "hash": 6001, "offset_ms": 100 }]
}
```
Response shape is identical to `/recognitions/audio`.

### `GET /api/v1/recognitions/{id}`
Retrieves a previously-recorded recognition attempt from the audit log
(metadata only — the query audio itself was never persisted, see
[architecture.md](architecture.md)).
```json
{
  "id": "42",
  "recognized": true,
  "song_id": "3",
  "reason": null,
  "score": 119.46,
  "confidence": 0.49,
  "matched_fingerprints": 145,
  "query_fingerprints": 294,
  "offset_ms": 9000,
  "algorithm_version": 1,
  "latency_ms": 666,
  "requested_at": "2026-09-16T18:30:00Z"
}
```
`404` if the id doesn't exist.

### `GET /api/v1/songs/{id}`
```json
{ "id": "3", "title": "Song Alpha", "artist": "Test Artist", "album": null, "duration_ms": 20000, "artwork_url": null }
```
`404 SONG_NOT_FOUND` if the ID doesn't exist. `400 BAD_REQUEST` for a
non-positive/malformed ID. Backed by an optional Redis cache (5 minute
TTL) — transparent to the caller either way.

## Admin endpoints

Require `Authorization: Bearer $ADMIN_API_KEY` (see `.env.example`).
Missing/incorrect token → `401 UNAUTHORIZED`. Kept under a separate
`admin_auth` middleware from the public endpoints (never shares a route
group with them).

### `POST /api/v1/admin/ingest`
Small/API-driven ingest of one song plus its pre-extracted fingerprints —
intended for a handful of songs at a time (a demo catalog, a single
correction). **Not** the path for bulk catalog ingestion — see
[ingestion.md](ingestion.md) for why (COPY-based bulk insert via the
Python CLI is the real ingestion path; this endpoint is capped at 200,000
fingerprints per request and uses a batched `UNNEST` insert, not `COPY`).

```json
{
  "title": "Song Alpha",
  "artist": "Test Artist",
  "album": null,
  "album_artist": null,
  "duration_ms": 20000,
  "isrc": null,
  "artwork_url": null,
  "algorithm_version": 1,
  "fingerprints": [{ "hash": 6000, "offset_ms": 0 }]
}
```
```json
{ "song_id": "7", "fingerprints_inserted": 1 }
```

### `DELETE /api/v1/admin/songs/{id}/fingerprints`
Drops all fingerprints for a song (the song row itself is untouched) —
e.g. to re-ingest it cleanly under a new algorithm version.
```json
{ "song_id": "7", "fingerprints_deleted": 1759 }
```

## Rate limiting

A per-IP token bucket (`RATE_LIMIT_RPM` / `RATE_LIMIT_BURST`, defaults 60
req/min, burst 10) applies to every `/api/v1/*` route. `/health`, `/ready`,
and `/metrics` are exempt (orchestrator probes and Prometheus scrapes must
not be throttled). This is a single-instance, in-memory limiter — see
[deployment.md](deployment.md) for what changes in a multi-instance
deployment.

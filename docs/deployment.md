# Deployment

## Local development

`docker compose up --build` (or `make dev`) starts `postgres`, `redis`,
`processor`, and `backend` with Docker healthchecks and correct startup
ordering (`backend` waits on `postgres` and `processor` being healthy —
see `docker-compose.yml`). The backend applies database migrations
automatically on startup (`sqlx::migrate!`, embedded at compile time — see
`backend/src/storage/db.rs`).

Copy `.env.example` to `.env` first and set a real `ADMIN_API_KEY` — the
compose file refuses to start the backend without one
(`${ADMIN_API_KEY:?set ADMIN_API_KEY in .env}`).

Android points at the backend via `http://10.0.2.2:8080/` from an emulator
(the standard alias for the host machine's localhost) — see
[android.md](android.md).

## Production

This repository ships a working local/dev deployment; the following is
what changes for a real production deployment and is **not** implemented
here (documented, not built, per the project's scope):

### TLS
Terminate TLS in front of the backend — a reverse proxy (nginx, Caddy,
Envoy) or a cloud load balancer. The backend itself speaks plain HTTP; it
is not designed to be exposed directly to the internet.

### Reverse proxy / API gateway
Put one in front of `backend` for: TLS termination, request buffering,
additional abuse protection, and — critically — **shared-state rate
limiting**. The in-memory limiter in `backend/src/routes/rate_limit.rs` is
explicitly per-instance; it's fine for a single container but does not
coordinate across replicas. For a multi-instance deployment, either move
rate limiting to the gateway/proxy layer or replace the in-memory bucket
with a Redis-backed one (the `RedisCache` wrapper already in the codebase
would be the natural place to add this).

### Authentication
The recognition/song endpoints are deliberately public (this mirrors how
Shazam-like clients work — no per-user auth for a "what song is this"
query). If usage-based quotas or per-client API keys become a requirement,
add them at the gateway layer rather than baking client auth into the
matching path. Admin endpoints already require a bearer token
(`ADMIN_API_KEY`) — rotate it via your secret manager of choice, never
commit it.

### Secrets
All configuration is environment-variable-driven (see `.env.example` for
the full list) and nothing is hardcoded. In production, inject these via
your platform's secret manager (e.g. AWS Secrets Manager, Vault, k8s
Secrets) rather than plain environment variables in a compose file.

### Database
- Run managed Postgres (RDS/Cloud SQL/etc.) with automated backups and
  point-in-time recovery.
- Follow the partitioning path in [database.md](database.md) once the
  catalog approaches ~10k songs / tens of millions of fingerprints.
- Read replicas for the fingerprint-lookup hot path once a single primary
  can't keep up; ingestion writes should stay pinned to the primary.

### Observability
- Point Prometheus at `GET /metrics` on each backend instance.
- Ship structured JSON logs (already the format `tracing-subscriber`
  emits) to your log aggregator of choice.
- Add alerting on `recognition_failure_total{reason=...}` rate spikes and
  `fingerprint_lookup_latency_ms`/`recognition_latency_ms` p95/p99
  regressions (see [performance.md](performance.md) for baseline numbers).

### Scaling the processor
The processor is stateless (no DB connection of its own on the request
path) and horizontally scalable — run multiple replicas behind a simple
round-robin/least-connections load balancer and point `PROCESSOR_URL` at
that balancer's address, or run one processor replica per backend replica
as a sidecar.

### Abuse prevention beyond rate limiting
- Enforce `MAX_AUDIO_BYTES`/`MAX_AUDIO_DURATION_SECONDS` at the CDN/proxy
  layer too, not only in the application (defense in depth).
- Consider a WAF in front of the public endpoints if exposed directly to
  consumer internet traffic.

### CI/CD
`.github/workflows/ci.yml` runs formatting/lint/tests/build for all three
components plus a Docker build check on every PR. A production deployment
pipeline (build → push images → deploy) is not included — wire the
existing Docker images into whatever deployment target you use (ECS, k8s,
Cloud Run, etc.); the images are already built to be portable (no
host-specific assumptions).

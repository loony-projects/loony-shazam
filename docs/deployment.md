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

## Running without Docker

Nothing in this system actually requires Docker — it's a convenience for
bundling Postgres/Redis/processor/backend together, not a dependency any
of them have on each other. The processor is a plain Python/FastAPI
process, the backend is a plain Rust binary, and both just need a
reachable Postgres (and optionally Redis) via `DATABASE_URL`/`REDIS_URL`.

```bash
cp .env.example .env
# set DATABASE_URL to a Postgres you already have running (a system
# package install, a VM, a managed instance — anything), REDIS_URL if you
# have one (optional — the app runs fine without it), and a real
# ADMIN_API_KEY

make dev-local        # scripts/dev/run_local.sh
```

`scripts/dev/run_local.sh`:
1. Checks `DATABASE_URL` is reachable (`psql ... -c "SELECT 1"`) and warns
   (but doesn't fail) if `REDIS_URL` is set but unreachable.
2. Creates `processor/.venv` and installs dependencies if this is the
   first run.
3. Starts the processor (`music-fingerprint serve`) and waits for
   `/internal/v1/health`.
4. Builds the backend release binary if needed and starts it — which
   applies schema migrations automatically and idempotently
   (`sqlx::migrate!`, tracked in `_sqlx_migrations`) — and waits for
   `/ready`.

Both processes run in the background with PID files and logs under
`.run/` (gitignored). Stop them with `make dev-local-stop`; tail their
output with `make dev-local-logs`.

This intentionally does **not** try to install or start Postgres/Redis
themselves — unlike `docker compose up`, which containerizes fresh
instances, this path assumes you're pointing at something that already
exists. If `DATABASE_URL` isn't reachable, the script fails fast with a
clear message rather than doing anything destructive.

### Adopting a schema that was applied another way

The backend's migration tracking (`_sqlx_migrations`) expects to be the
one that ran `CREATE TABLE songs`, etc. If you've already applied
`database/migrations/*.sql` by hand (e.g. via `make migrate`, or your own
tooling) against a database that doesn't have `_sqlx_migrations` yet, the
backend's next startup will try to re-run migration 1 and fail with
`relation "songs" already exists`. To adopt that schema instead of
recreating it: compute the checksums the embedded migrator expects and
back-fill `_sqlx_migrations` with them, once:

```bash
# Prints "<version> <description> <sqlx-checksum-hex>" per migration
cat > /tmp/print_checksums.rs <<'RS'
fn main() {
    let migrator = sqlx::migrate!("../database/migrations");
    for m in migrator.iter() {
        let hex: String = m.checksum.iter().map(|b| format!("{b:02x}")).collect();
        println!("{}\t{}\t{}", m.version, m.description, hex);
    }
}
RS
cp /tmp/print_checksums.rs backend/examples/print_migration_checksums.rs
(cd backend && cargo run --release --example print_migration_checksums)
rm backend/examples/print_migration_checksums.rs
```

Then, for each printed row, insert it into `_sqlx_migrations` (creating
the table first if needed — see the standard sqlx schema: `version
BIGINT PRIMARY KEY, description TEXT NOT NULL, installed_on TIMESTAMPTZ
NOT NULL DEFAULT now(), success BOOLEAN NOT NULL, checksum BYTEA NOT
NULL, execution_time BIGINT NOT NULL`), with `success = true` and
`checksum = decode('<hex>', 'hex')`. This is a one-time bookkeeping fix,
not something `run_local.sh` does automatically — it only applies when
you've deliberately applied the schema outside the backend's own startup
path.

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

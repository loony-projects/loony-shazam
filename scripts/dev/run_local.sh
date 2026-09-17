#!/usr/bin/env bash
# Runs the processor + backend as plain local processes, no Docker
# involved. Assumes Postgres and (optionally) Redis are already running
# somewhere reachable (a system-installed instance, a VM, whatever —
# anything DATABASE_URL/REDIS_URL points at). Schema migrations are
# applied automatically by the backend itself on startup (idempotent, see
# backend/src/storage/db.rs) — this script does not touch the database
# directly beyond a reachability check.
#
# Usage:
#   scripts/dev/run_local.sh          # start processor + backend in the background
#   scripts/dev/stop_local.sh         # stop them
#   tail -f .run/*.log                # watch logs
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT_DIR"

ENV_FILE=.env
if [ ! -f "$ENV_FILE" ]; then
  echo "No .env found — copy .env.example to .env and fill in DATABASE_URL/ADMIN_API_KEY first."
  exit 1
fi
set -a
# shellcheck disable=SC1090
source <(grep -v '^#' "$ENV_FILE" | grep -v '^\s*$')
set +a

: "${DATABASE_URL:?DATABASE_URL must be set in .env}"
: "${ADMIN_API_KEY:?ADMIN_API_KEY must be set in .env}"
PROCESSOR_PORT="${PROCESSOR_PORT:-8001}"
API_PORT="${API_PORT:-8080}"
PROCESSOR_URL="${PROCESSOR_URL:-http://localhost:$PROCESSOR_PORT}"

# Resolve to an absolute path and export it: the processor and backend are
# launched from different working directories below (processor/, backend/),
# so a relative ARTWORK_DIR (e.g. the "./data/artwork" default in
# .env.example) would silently resolve to two DIFFERENT directories — one
# under each subdirectory — if left as-is. Both processes must agree on
# literally the same directory (the processor writes cover art into it
# during ingestion; the backend serves it back out at GET /artwork/*).
ARTWORK_DIR="${ARTWORK_DIR:-./data/artwork}"
if [[ "$ARTWORK_DIR" != /* ]]; then
  ARTWORK_DIR="$ROOT_DIR/${ARTWORK_DIR#./}"
fi
export ARTWORK_DIR
mkdir -p "$ARTWORK_DIR"

RUN_DIR="$ROOT_DIR/.run"
mkdir -p "$RUN_DIR"

echo "==> Checking Postgres at DATABASE_URL is reachable"
if ! psql "$DATABASE_URL" -c "SELECT 1" >/dev/null 2>&1; then
  echo "FAILED: could not connect using DATABASE_URL=$DATABASE_URL"
  echo "Postgres must already be running (a system install, a VM, docker run — anything)."
  echo "Check the host/port/user/password in .env, or start Postgres and retry."
  exit 1
fi
echo "OK"

if [ -n "${REDIS_URL:-}" ]; then
  echo "==> Checking Redis at REDIS_URL (optional — the app runs fine without it)"
  if command -v redis-cli >/dev/null 2>&1 && redis-cli -u "$REDIS_URL" ping >/dev/null 2>&1; then
    echo "OK"
  else
    echo "WARNING: Redis not reachable at $REDIS_URL — continuing without it (metadata cache disabled, everything else works)."
  fi
fi

is_running() {
  local pid_file="$1"
  [ -f "$pid_file" ] && kill -0 "$(cat "$pid_file")" 2>/dev/null
}

echo "==> Processor (Python extraction service)"
if is_running "$RUN_DIR/processor.pid"; then
  echo "already running (pid $(cat "$RUN_DIR/processor.pid"))"
else
  if [ ! -d processor/.venv ]; then
    echo "   creating processor/.venv and installing dependencies (first run only)"
    python3 -m venv processor/.venv
    (cd processor && source .venv/bin/activate && pip install --quiet --upgrade pip && pip install --quiet -e ".[dev]")
  fi
  (
    cd processor
    source .venv/bin/activate
    nohup music-fingerprint serve --port "$PROCESSOR_PORT" >"$RUN_DIR/processor.log" 2>&1 &
    echo $! >"$RUN_DIR/processor.pid"
  )
  echo "   started (pid $(cat "$RUN_DIR/processor.pid")), logs: .run/processor.log"
fi

echo "==> Waiting for processor readiness"
for _ in $(seq 1 30); do
  curl -sf "$PROCESSOR_URL/internal/v1/health" >/dev/null 2>&1 && break
  sleep 1
done
curl -sf "$PROCESSOR_URL/internal/v1/health" >/dev/null || {
  echo "FAILED: processor did not become healthy — see .run/processor.log"
  exit 1
}
echo "OK"

echo "==> Backend (Rust API)"
if is_running "$RUN_DIR/backend.pid"; then
  echo "already running (pid $(cat "$RUN_DIR/backend.pid"))"
else
  echo "   building release binary (cargo build --release — first run is slow, later ones are cached)"
  (cd backend && cargo build --release --quiet)
  (
    cd backend
    nohup ./target/release/backend >"$RUN_DIR/backend.log" 2>&1 &
    echo $! >"$RUN_DIR/backend.pid"
  )
  echo "   started (pid $(cat "$RUN_DIR/backend.pid")), logs: .run/backend.log"
fi

echo "==> Waiting for backend readiness"
for _ in $(seq 1 30); do
  curl -sf "http://localhost:$API_PORT/ready" >/dev/null 2>&1 && break
  sleep 1
done
READY_BODY=$(curl -sf "http://localhost:$API_PORT/ready" || true)
if [ -z "$READY_BODY" ]; then
  echo "FAILED: backend did not become ready — see .run/backend.log"
  exit 1
fi
echo "$READY_BODY"

echo ""
echo "==> Running (no Docker):"
echo "    processor: $PROCESSOR_URL (pid $(cat "$RUN_DIR/processor.pid"))"
echo "    backend:   http://localhost:$API_PORT (pid $(cat "$RUN_DIR/backend.pid"))"
echo "    artwork:   $ARTWORK_DIR (pass the same ARTWORK_DIR to 'music-fingerprint ingest'"
echo "               if you run it manually instead of via 'make ingest')"
echo ""
echo "Stop with:  scripts/dev/stop_local.sh"
echo "Logs:       tail -f .run/*.log"

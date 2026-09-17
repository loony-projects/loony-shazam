.PHONY: dev dev-down dev-local dev-local-stop dev-local-logs test test-backend test-processor \
	lint lint-backend lint-processor ingest backfill-artwork benchmark e2e fmt migrate \
	install-android uninstall-android

# Start the full backend stack (postgres, redis, processor, backend) via Docker Compose.
dev:
	docker compose up --build

dev-down:
	docker compose down

# Start processor + backend as plain local processes, no Docker at all.
# Requires Postgres (and optionally Redis) already reachable — see .env and
# docs/deployment.md "Running without Docker".
dev-local:
	scripts/dev/run_local.sh

dev-local-stop:
	scripts/dev/stop_local.sh

dev-local-logs:
	tail -f .run/*.log

# Apply database migrations against a running local Postgres. The backend
# also applies them automatically on every startup (see
# backend/src/storage/db.rs) — this target is for applying them without
# starting the full server, e.g. before running the ingestion CLI standalone.
migrate:
	@url=$${DATABASE_URL:-postgresql://postgres:devpass@localhost:5432/loony_shazam}; \
	for f in database/migrations/*.sql; do \
		echo "applying $$f"; \
		psql "$$url" -f $$f || exit 1; \
	done

test: test-processor test-backend

test-backend:
	cd backend && cargo test

test-processor:
	cd processor && . .venv/bin/activate && pytest

lint: lint-backend lint-processor

lint-backend:
	cd backend && cargo fmt --check && cargo clippy --all-targets --all-features -- -D warnings

lint-processor:
	cd processor && . .venv/bin/activate && ruff check . && ruff format --check .

fmt:
	cd backend && cargo fmt
	cd processor && . .venv/bin/activate && ruff format .

# Usage: make ingest MUSIC_DIR=./testdata/audio
# ARTWORK_DIR defaults to an absolute path at the repo root so it matches
# whatever `make dev-local` / `make dev` set up for the backend to serve
# from, regardless of which directory this runs from.
ingest:
	cd processor && . .venv/bin/activate && \
		DATABASE_URL=$${DATABASE_URL:-postgresql://postgres:devpass@localhost:5432/loony_shazam} \
		ARTWORK_DIR=$${ARTWORK_DIR:-$(CURDIR)/data/artwork} \
		music-fingerprint ingest $(MUSIC_DIR)

# Usage: make backfill-artwork MUSIC_DIR=./testdata/audio
backfill-artwork:
	cd processor && . .venv/bin/activate && \
		DATABASE_URL=$${DATABASE_URL:-postgresql://postgres:devpass@localhost:5432/loony_shazam} \
		ARTWORK_DIR=$${ARTWORK_DIR:-$(CURDIR)/data/artwork} \
		music-fingerprint backfill-artwork $(MUSIC_DIR)

benchmark:
	cd processor && . .venv/bin/activate && python benchmarks/run_benchmarks.py

e2e:
	scripts/dev/run_e2e.sh

# Build, install (adb reverse'd to a local backend), and launch the debug
# app on a connected device or emulator. See scripts/dev/install_android.sh
# --help for flags (e.g. --device <id> with multiple devices attached).
install-android:
	scripts/dev/install_android.sh

uninstall-android:
	scripts/dev/uninstall_android.sh

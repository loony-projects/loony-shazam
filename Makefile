.PHONY: dev dev-down test test-backend test-processor lint lint-backend lint-processor \
	ingest benchmark e2e fmt migrate

# Start the full backend stack (postgres, redis, processor, backend) via Docker Compose.
dev:
	docker compose up --build

dev-down:
	docker compose down

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
ingest:
	cd processor && . .venv/bin/activate && \
		DATABASE_URL=$${DATABASE_URL:-postgresql://postgres:devpass@localhost:5432/loony_shazam} \
		music-fingerprint ingest $(MUSIC_DIR)

benchmark:
	cd processor && . .venv/bin/activate && python benchmarks/run_benchmarks.py

e2e:
	scripts/dev/run_e2e.sh

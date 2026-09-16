# syntax=docker/dockerfile:1
FROM rust:1-slim-bookworm AS builder
WORKDIR /build

RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config libssl-dev ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Cache dependency compilation separately from application code.
COPY backend/Cargo.toml backend/Cargo.lock ./backend/
COPY database ./database
RUN mkdir -p backend/src && echo "fn main() {}" > backend/src/main.rs \
    && echo "" > backend/src/lib.rs \
    && cd backend && cargo build --release --locked || true

COPY backend ./backend
# Force a real rebuild now that real sources are present (the dummy build
# above only warms the dependency cache).
RUN touch backend/src/main.rs backend/src/lib.rs
RUN cd backend && cargo build --release --locked

FROM debian:bookworm-slim AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --create-home --uid 10001 appuser

# Migrations are embedded into the binary at compile time by `sqlx::migrate!`
# (see backend/src/storage/db.rs), so the runtime image doesn't need the
# migrations directory on disk.
COPY --from=builder /build/backend/target/release/backend /usr/local/bin/backend

WORKDIR /app
USER appuser

ENV RUST_LOG=info
EXPOSE 8080

HEALTHCHECK --interval=10s --timeout=3s --start-period=10s --retries=5 \
    CMD curl -sf http://localhost:8080/health || exit 1

ENTRYPOINT ["/usr/local/bin/backend"]

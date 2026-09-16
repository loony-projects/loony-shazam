-- Tracks provenance of ingested files so re-running ingestion on the same
-- directory is idempotent: the same file (by content hash) is never
-- fingerprinted or inserted twice.
CREATE TABLE ingested_sources (
    id                  BIGSERIAL PRIMARY KEY,
    sha256              TEXT NOT NULL UNIQUE,
    file_path           TEXT NOT NULL,
    song_id             BIGINT REFERENCES songs(id) ON DELETE SET NULL,
    algorithm_version   SMALLINT NOT NULL,
    fingerprint_count   INTEGER NOT NULL DEFAULT 0,
    ingested_at         TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_ingested_sources_song_id ON ingested_sources (song_id);

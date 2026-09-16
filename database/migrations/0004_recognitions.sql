-- Audit/history of recognition requests. Stores only metadata about the
-- match outcome, never the uploaded query audio itself (see docs/architecture.md
-- privacy notes). song_id is nullable because unrecognized queries are
-- still recorded for observability.
CREATE TABLE recognitions (
    id                      BIGSERIAL PRIMARY KEY,
    song_id                 BIGINT REFERENCES songs(id) ON DELETE SET NULL,
    recognized              BOOLEAN NOT NULL,
    reason                  TEXT,
    score                   DOUBLE PRECISION,
    confidence              DOUBLE PRECISION,
    matched_fingerprints    INTEGER,
    query_fingerprints      INTEGER,
    offset_ms               INTEGER,
    algorithm_version       SMALLINT NOT NULL,
    latency_ms              INTEGER,
    requested_at            TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_recognitions_requested_at ON recognitions (requested_at DESC);
CREATE INDEX idx_recognitions_song_id ON recognitions (song_id) WHERE song_id IS NOT NULL;

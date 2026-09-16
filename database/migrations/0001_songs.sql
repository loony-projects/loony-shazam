-- Catalog of recognizable songs. One row per distinct recording.
CREATE TABLE songs (
    id              BIGSERIAL PRIMARY KEY,
    title           TEXT NOT NULL,
    artist          TEXT NOT NULL,
    album           TEXT,
    album_artist    TEXT,
    duration_ms     INTEGER NOT NULL CHECK (duration_ms > 0),
    release_date    DATE,
    isrc            TEXT,
    artwork_url     TEXT,
    source          TEXT NOT NULL DEFAULT 'local',
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_songs_artist_title ON songs (artist, title);
CREATE INDEX idx_songs_isrc ON songs (isrc) WHERE isrc IS NOT NULL;

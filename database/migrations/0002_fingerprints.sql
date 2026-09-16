-- The hot table: one row per landmark hash generated during ingestion.
-- At catalog scale this table is expected to hold hundreds of millions of
-- rows, so it deliberately has NO surrogate primary key (saves 8 bytes/row
-- + a redundant index) and is populated exclusively via bulk COPY, never
-- row-by-row INSERT.
--
-- `algorithm_version` is duplicated here (it is also encoded in the top 4
-- bits of `hash`, see docs/fingerprinting.md) so it can be used as a plain
-- filter/partition key without unpacking the hash.
CREATE TABLE fingerprints (
    hash                BIGINT NOT NULL,
    song_id             BIGINT NOT NULL REFERENCES songs(id) ON DELETE CASCADE,
    offset_ms           INTEGER NOT NULL,
    algorithm_version   SMALLINT NOT NULL
);

-- Composite covering index: the recognition hot path is
--   SELECT song_id, offset_ms FROM fingerprints
--   WHERE hash = ANY($1) AND algorithm_version = $2
-- The INCLUDE columns let Postgres satisfy this with an index-only scan.
CREATE INDEX idx_fingerprints_hash_lookup
    ON fingerprints (hash, algorithm_version) INCLUDE (song_id, offset_ms);

-- Needed for efficient `ON DELETE CASCADE` when a song is removed and for
-- "re-ingest this song" workflows that delete-then-reinsert.
CREATE INDEX idx_fingerprints_song_id ON fingerprints (song_id);

-- Scaling note (see docs/database.md): once the catalog grows past roughly
-- 10k songs / tens of millions of fingerprints, convert this table to a
-- LIST partition on `algorithm_version` (there are only ever a handful of
-- live algorithm versions at once) and consider a HASH sub-partition on
-- `hash` to keep individual partitions and their indexes small enough to
-- stay cached. This migration intentionally ships unpartitioned to keep
-- the initial schema simple; partitioning is a additive migration later.

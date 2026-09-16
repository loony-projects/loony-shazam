use sqlx::PgPool;

use crate::error::AppError;
use crate::models::{AlgorithmVersion, FingerprintHash, OffsetMs, SongId};

#[derive(Clone)]
pub struct FingerprintRepository {
    pool: PgPool,
}

impl FingerprintRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Bulk insert via `UNNEST` over parallel arrays — a single round trip
    /// regardless of batch size, not one `INSERT` per row. This is the path
    /// used by the small/API-driven admin ingest endpoint; true
    /// catalog-scale ingestion (hundreds of thousands+ of fingerprints per
    /// song) goes through the Python CLI's direct `COPY` (see
    /// processor/src/music_fingerprint/ingestion.py and docs/ingestion.md)
    /// rather than round-tripping through HTTP at all.
    pub async fn bulk_insert(
        &self,
        song_id: SongId,
        fingerprints: &[(FingerprintHash, OffsetMs)],
        algorithm_version: AlgorithmVersion,
    ) -> Result<u64, AppError> {
        if fingerprints.is_empty() {
            return Ok(0);
        }

        let hashes: Vec<i64> = fingerprints.iter().map(|(h, _)| h.0).collect();
        let offsets: Vec<i32> = fingerprints.iter().map(|(_, o)| o.0).collect();
        let song_ids: Vec<i64> = vec![song_id.0; fingerprints.len()];
        let versions: Vec<i16> = vec![algorithm_version.0; fingerprints.len()];

        let result = sqlx::query(
            r#"
            INSERT INTO fingerprints (hash, song_id, offset_ms, algorithm_version)
            SELECT * FROM UNNEST($1::bigint[], $2::bigint[], $3::int[], $4::smallint[])
            "#,
        )
        .bind(&hashes)
        .bind(&song_ids)
        .bind(&offsets)
        .bind(&versions)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }

    pub async fn delete_for_song(&self, song_id: SongId) -> Result<u64, AppError> {
        let result = sqlx::query("DELETE FROM fingerprints WHERE song_id = $1")
            .bind(song_id.0)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected())
    }
}

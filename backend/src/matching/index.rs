//! Fingerprint lookup, abstracted behind a trait so the storage backend can
//! be swapped later (see docs/architecture.md "Matching engine") without
//! touching voting/scoring/ranking logic.

use async_trait::async_trait;
use sqlx::PgPool;

use crate::error::AppError;
use crate::models::{AlgorithmVersion, FingerprintHash, OffsetMs, SongId};

#[derive(Debug, Clone, Copy)]
pub struct FingerprintMatch {
    pub hash: FingerprintHash,
    pub song_id: SongId,
    pub offset_ms: OffsetMs,
}

#[async_trait]
pub trait FingerprintIndex: Send + Sync {
    /// Bulk lookup: given a batch of query hashes, return every reference
    /// fingerprint row whose hash appears in that batch (for the given
    /// algorithm version). One round trip regardless of batch size.
    async fn lookup(
        &self,
        hashes: &[FingerprintHash],
        algorithm_version: AlgorithmVersion,
    ) -> Result<Vec<FingerprintMatch>, AppError>;
}

pub struct PostgresFingerprintIndex {
    pool: PgPool,
}

impl PostgresFingerprintIndex {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl FingerprintIndex for PostgresFingerprintIndex {
    async fn lookup(
        &self,
        hashes: &[FingerprintHash],
        algorithm_version: AlgorithmVersion,
    ) -> Result<Vec<FingerprintMatch>, AppError> {
        if hashes.is_empty() {
            return Ok(Vec::new());
        }

        // UNNEST($1) joined against the covering index, not a giant
        // `IN (...)` list or one query per hash — see docs/architecture.md
        // "Search optimization" and docs/database.md.
        let raw_hashes: Vec<i64> = hashes.iter().map(|h| h.0).collect();
        let started = std::time::Instant::now();

        #[derive(sqlx::FromRow)]
        struct Row {
            hash: i64,
            song_id: i64,
            offset_ms: i32,
        }

        // Not a compile-time-checked `sqlx::query!` macro on purpose: that
        // would require a live database (or a checked-in `.sqlx` cache) at
        // `cargo build` time, which we don't want to force on every
        // contributor/CI runner. Runtime `query_as` still gets full
        // parameter binding safety (no string interpolation of hashes).
        let rows: Vec<Row> = sqlx::query_as(
            r#"
            SELECT f.hash, f.song_id, f.offset_ms
            FROM fingerprints f
            INNER JOIN UNNEST($1::bigint[]) AS q(hash) ON f.hash = q.hash
            WHERE f.algorithm_version = $2
            "#,
        )
        .bind(&raw_hashes)
        .bind(algorithm_version.0)
        .fetch_all(&self.pool)
        .await?;

        metrics::histogram!(crate::telemetry::metric_names::FINGERPRINT_LOOKUP_LATENCY)
            .record(started.elapsed().as_millis() as f64);

        Ok(rows
            .into_iter()
            .map(|r| FingerprintMatch {
                hash: FingerprintHash(r.hash),
                song_id: SongId(r.song_id),
                offset_ms: OffsetMs(r.offset_ms),
            })
            .collect())
    }
}

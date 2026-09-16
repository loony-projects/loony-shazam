use sqlx::PgPool;

use crate::error::AppError;
use crate::models::SongId;

/// A previously-recorded recognition attempt, as returned by
/// `GET /api/v1/recognitions/{id}`.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct RecognitionRecord {
    pub id: i64,
    pub song_id: Option<i64>,
    pub recognized: bool,
    pub reason: Option<String>,
    pub score: Option<f64>,
    pub confidence: Option<f64>,
    pub matched_fingerprints: Option<i32>,
    pub query_fingerprints: Option<i32>,
    pub offset_ms: Option<i32>,
    pub algorithm_version: i16,
    pub latency_ms: i32,
    pub requested_at: chrono::DateTime<chrono::Utc>,
}

/// Audit-log-only write: metadata about a recognition attempt, never the
/// uploaded query audio itself (see docs/architecture.md privacy notes).
#[derive(Debug, Clone)]
pub struct NewRecognitionRecord {
    pub song_id: Option<SongId>,
    pub recognized: bool,
    pub reason: Option<&'static str>,
    pub score: Option<f64>,
    pub confidence: Option<f64>,
    pub matched_fingerprints: Option<i32>,
    pub query_fingerprints: Option<i32>,
    pub offset_ms: Option<i32>,
    pub algorithm_version: i16,
    pub latency_ms: i32,
}

#[derive(Clone)]
pub struct RecognitionRepository {
    pool: PgPool,
}

impl RecognitionRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn record(&self, record: NewRecognitionRecord) -> Result<i64, AppError> {
        let row: (i64,) = sqlx::query_as(
            r#"
            INSERT INTO recognitions
                (song_id, recognized, reason, score, confidence, matched_fingerprints,
                 query_fingerprints, offset_ms, algorithm_version, latency_ms)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            RETURNING id
            "#,
        )
        .bind(record.song_id.map(|s| s.0))
        .bind(record.recognized)
        .bind(record.reason)
        .bind(record.score)
        .bind(record.confidence)
        .bind(record.matched_fingerprints)
        .bind(record.query_fingerprints)
        .bind(record.offset_ms)
        .bind(record.algorithm_version)
        .bind(record.latency_ms)
        .fetch_one(&self.pool)
        .await?;
        Ok(row.0)
    }

    pub async fn find_by_id(&self, id: i64) -> Result<Option<RecognitionRecord>, AppError> {
        let record = sqlx::query_as::<_, RecognitionRecord>(
            r#"
            SELECT id, song_id, recognized, reason, score, confidence, matched_fingerprints,
                   query_fingerprints, offset_ms, algorithm_version, latency_ms, requested_at
            FROM recognitions
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(record)
    }
}

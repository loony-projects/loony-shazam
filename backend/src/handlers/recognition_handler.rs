use axum::extract::{Multipart, State};
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};
use crate::matching::QueryFingerprint;
use crate::models::{FingerprintHash, OffsetMs, RecognitionOutcome, SongResponse};
use crate::state::AppState;

#[derive(Serialize)]
pub struct MatchInfo {
    pub score: f64,
    pub confidence: f64,
    pub matched_fingerprints: u32,
    pub query_fingerprints: u32,
    pub offset_ms: Option<i64>,
    pub algorithm_version: i16,
    pub latency_ms: u64,
}

#[derive(Serialize)]
pub struct RecognitionResponse {
    pub recognized: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recognition_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub song: Option<SongResponse>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<&'static str>,
    #[serde(rename = "match")]
    pub match_info: MatchInfo,
}

impl From<RecognitionOutcome> for RecognitionResponse {
    fn from(outcome: RecognitionOutcome) -> Self {
        let recognized = outcome.recognized();
        let reason = outcome.reason.map(|r| r.as_api_str());
        Self {
            recognized,
            recognition_id: outcome.recognition_id.map(|id| id.to_string()),
            match_info: MatchInfo {
                score: outcome.score,
                confidence: outcome.confidence,
                matched_fingerprints: outcome.matched_fingerprints,
                query_fingerprints: outcome.query_fingerprints,
                offset_ms: outcome.offset_ms,
                algorithm_version: outcome.algorithm_version.0,
                latency_ms: outcome.latency_ms,
            },
            song: outcome.song.map(SongResponse::from),
            reason,
        }
    }
}

/// `POST /api/v1/recognitions/audio` — the primary path. Accepts a
/// multipart upload with a single `audio` field (any format the processor
/// can decode: WAV/FLAC/MP3 directly, anything else via an ffmpeg
/// fallback that probes content rather than trusting the file's extension).
pub async fn recognize_audio(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> AppResult<Json<RecognitionResponse>> {
    let mut audio_bytes: Option<Vec<u8>> = None;
    let mut content_type: Option<String> = None;
    let mut filename: Option<String> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::BadRequest(format!("malformed multipart body: {e}")))?
    {
        if field.name() == Some("audio") {
            content_type = field.content_type().map(|s| s.to_string());
            filename = field.file_name().map(|s| s.to_string());
            let bytes = field
                .bytes()
                .await
                .map_err(|e| AppError::BadRequest(format!("failed to read audio field: {e}")))?;
            audio_bytes = Some(bytes.to_vec());
        }
    }

    let audio = audio_bytes.ok_or(AppError::MissingAudio)?;

    // Cheap defense-in-depth MIME check before doing any decode work. The
    // real, trustworthy validation still happens in the processor (which
    // only believes what it can actually decode — see product spec "AUDIO
    // SECURITY": never trust a claimed MIME type alone), but rejecting an
    // obviously-wrong content type here avoids a wasted round trip.
    if let Some(ct) = content_type.as_deref() {
        if !ct.is_empty() && !ct.starts_with("audio/") && ct != "application/octet-stream" {
            return Err(AppError::InvalidAudio(format!(
                "unsupported content type '{ct}', expected an audio/* type"
            )));
        }
    }

    let service = crate::services::RecognitionService::new(
        state.processor.clone(),
        state.matching_engine.clone(),
        state.recognitions.clone(),
        state.config.clone(),
    );

    let outcome = service
        .recognize_audio(&audio, content_type.as_deref(), filename.as_deref())
        .await?;

    record_metrics(&outcome);

    Ok(Json(outcome.into()))
}

#[derive(Deserialize)]
pub struct RecognizeFingerprintsRequest {
    pub algorithm_version: i16,
    pub fingerprints: Vec<FingerprintPair>,
}

#[derive(Deserialize)]
pub struct FingerprintPair {
    pub hash: i64,
    pub offset_ms: i32,
}

/// `POST /api/v1/recognitions` — accepts already-extracted fingerprints
/// directly (JSON). Intended for programmatic clients / testing that want
/// to bypass audio upload and the processor round trip entirely; the
/// Android client always uses `/recognitions/audio`.
pub async fn recognize_fingerprints(
    State(state): State<AppState>,
    Json(req): Json<RecognizeFingerprintsRequest>,
) -> AppResult<Json<RecognitionResponse>> {
    let query_fingerprints: Vec<QueryFingerprint> = req
        .fingerprints
        .into_iter()
        .map(|f| QueryFingerprint {
            hash: FingerprintHash(f.hash),
            offset_ms: OffsetMs(f.offset_ms),
        })
        .collect();

    let outcome = state
        .matching_engine
        .recognize(
            query_fingerprints,
            crate::models::AlgorithmVersion(req.algorithm_version),
            std::time::Instant::now(),
        )
        .await?;

    let service = crate::services::RecognitionService::new(
        state.processor.clone(),
        state.matching_engine.clone(),
        state.recognitions.clone(),
        state.config.clone(),
    );
    let outcome = service.finalize(outcome).await;

    record_metrics(&outcome);

    Ok(Json(outcome.into()))
}

#[derive(Serialize)]
pub struct RecognitionRecordResponse {
    pub id: String,
    pub recognized: bool,
    pub song_id: Option<String>,
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

impl From<crate::repositories::RecognitionRecord> for RecognitionRecordResponse {
    fn from(r: crate::repositories::RecognitionRecord) -> Self {
        Self {
            id: r.id.to_string(),
            recognized: r.recognized,
            song_id: r.song_id.map(|id| id.to_string()),
            reason: r.reason,
            score: r.score,
            confidence: r.confidence,
            matched_fingerprints: r.matched_fingerprints,
            query_fingerprints: r.query_fingerprints,
            offset_ms: r.offset_ms,
            algorithm_version: r.algorithm_version,
            latency_ms: r.latency_ms,
            requested_at: r.requested_at,
        }
    }
}

/// `GET /api/v1/recognitions/{id}` — retrieves a previously-recorded
/// recognition attempt from the audit log (see docs/api.md). Note this is
/// metadata only; the query audio itself was never persisted (see
/// docs/architecture.md privacy notes).
pub async fn get_recognition(
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<i64>,
) -> AppResult<Json<RecognitionRecordResponse>> {
    if id <= 0 {
        return Err(AppError::BadRequest("invalid recognition id".into()));
    }
    let record = state
        .recognitions
        .find_by_id(id)
        .await?
        .ok_or(AppError::RecognitionNotFound)?;
    Ok(Json(record.into()))
}

fn record_metrics(outcome: &RecognitionOutcome) {
    use crate::telemetry::metric_names;
    if outcome.recognized() {
        metrics::counter!(metric_names::RECOGNITION_SUCCESS_TOTAL).increment(1);
    } else {
        let reason = outcome
            .reason
            .map(|r| r.as_metric_label())
            .unwrap_or("unknown");
        metrics::counter!(metric_names::RECOGNITION_FAILURE_TOTAL, "reason" => reason).increment(1);
    }
    metrics::histogram!(metric_names::RECOGNITION_LATENCY).record(outcome.latency_ms as f64);
}

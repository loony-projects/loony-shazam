//! Business logic for the recognition path: audio -> processor -> matching
//! engine -> audit log. Deliberately holds no HTTP concepts (no
//! `StatusCode`, no `Json<...>`) so it can be tested without spinning up
//! Axum, per "Avoid putting business logic inside HTTP handlers."

use std::time::Instant;

use crate::config::AppConfig;
use crate::error::AppError;
use crate::fingerprint::ProcessorClient;
use crate::matching::{MatchingEngine, QueryFingerprint};
use crate::models::{AlgorithmVersion, FingerprintHash, OffsetMs, RecognitionOutcome};
use crate::repositories::{NewRecognitionRecord, RecognitionRepository};

pub struct RecognitionService {
    processor: ProcessorClient,
    matching_engine: std::sync::Arc<MatchingEngine>,
    recognitions: RecognitionRepository,
    config: std::sync::Arc<AppConfig>,
}

impl RecognitionService {
    pub fn new(
        processor: ProcessorClient,
        matching_engine: std::sync::Arc<MatchingEngine>,
        recognitions: RecognitionRepository,
        config: std::sync::Arc<AppConfig>,
    ) -> Self {
        Self {
            processor,
            matching_engine,
            recognitions,
            config,
        }
    }

    pub async fn recognize_audio(
        &self,
        audio: &[u8],
        content_type: Option<&str>,
        filename_hint: Option<&str>,
    ) -> Result<RecognitionOutcome, AppError> {
        if audio.is_empty() {
            return Err(AppError::MissingAudio);
        }
        if audio.len() > self.config.max_audio_bytes {
            return Err(AppError::AudioTooLarge {
                size: audio.len(),
                limit: self.config.max_audio_bytes,
            });
        }

        let started = Instant::now();

        let extraction = self
            .processor
            .extract(
                audio,
                content_type,
                filename_hint,
                self.config.max_audio_duration_seconds,
            )
            .await?;

        tracing::debug!(
            duration_ms = extraction.duration_ms,
            sample_rate = extraction.sample_rate,
            num_fingerprints = extraction.fingerprints.len(),
            "extraction complete"
        );

        if extraction.algorithm_version != self.config.fingerprint_algorithm_version {
            return Err(AppError::AlgorithmVersionMismatch {
                expected: self.config.fingerprint_algorithm_version,
                actual: extraction.algorithm_version,
            });
        }

        let algorithm_version = AlgorithmVersion(extraction.algorithm_version);
        let query_fingerprints: Vec<QueryFingerprint> = extraction
            .fingerprints
            .into_iter()
            .map(|f| QueryFingerprint {
                hash: FingerprintHash(f.hash),
                offset_ms: OffsetMs(f.offset_ms),
            })
            .collect();

        let outcome = self
            .matching_engine
            .recognize(query_fingerprints, algorithm_version, started)
            .await?;

        Ok(self.finalize(outcome).await)
    }

    /// Persists the audit-log record for a recognition outcome (already
    /// computed by the matching engine, from either the audio or the
    /// JSON-fingerprints entry point) and stamps the resulting row id onto
    /// the outcome so `GET /api/v1/recognitions/{id}` can retrieve it
    /// later. Audit logging never fails the recognition response itself —
    /// a DB write failure here is logged and swallowed.
    pub async fn finalize(&self, mut outcome: RecognitionOutcome) -> RecognitionOutcome {
        let record = NewRecognitionRecord {
            song_id: outcome.song.as_ref().map(|s| s.id),
            recognized: outcome.recognized(),
            reason: outcome.reason.map(|r| r.as_api_str()),
            score: Some(outcome.score),
            confidence: Some(outcome.confidence),
            matched_fingerprints: Some(outcome.matched_fingerprints as i32),
            query_fingerprints: Some(outcome.query_fingerprints as i32),
            offset_ms: outcome.offset_ms.map(|o| o as i32),
            algorithm_version: outcome.algorithm_version.0,
            latency_ms: outcome.latency_ms as i32,
        };
        match self.recognitions.record(record).await {
            Ok(id) => outcome.recognition_id = Some(id),
            Err(err) => tracing::warn!(error = %err, "failed to record recognition audit log"),
        }
        outcome
    }
}

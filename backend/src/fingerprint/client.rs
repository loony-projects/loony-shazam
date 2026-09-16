//! HTTP client for the persistent Python fingerprint extraction service.
//! See docs/architecture.md "Processor service internal protocol" — this
//! is a pooled, timeout-bounded client to a long-lived service, never a
//! per-request process spawn.

use std::time::Duration;

use base64::Engine;
use serde::{Deserialize, Serialize};

use crate::error::AppError;

#[derive(Debug, Clone, Deserialize)]
pub struct ExtractedFingerprint {
    pub hash: i64,
    pub offset_ms: i32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ExtractionResponse {
    pub algorithm_version: i16,
    pub sample_rate: u32,
    pub duration_ms: i32,
    pub fingerprints: Vec<ExtractedFingerprint>,
}

#[derive(Debug, Serialize)]
struct ExtractRequest<'a> {
    audio_base64: String,
    content_type: Option<&'a str>,
    filename_hint: Option<&'a str>,
    max_duration_seconds: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct ProcessorErrorBody {
    detail: Option<String>,
}

#[derive(Clone)]
pub struct ProcessorClient {
    http: reqwest::Client,
    base_url: String,
}

impl ProcessorClient {
    pub fn new(base_url: String, timeout: Duration) -> anyhow::Result<Self> {
        let http = reqwest::Client::builder().timeout(timeout).build()?;
        Ok(Self { http, base_url })
    }

    pub async fn health(&self) -> bool {
        self.http
            .get(format!("{}/internal/v1/health", self.base_url))
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }

    pub async fn extract(
        &self,
        audio: &[u8],
        content_type: Option<&str>,
        filename_hint: Option<&str>,
        max_duration_seconds: f64,
    ) -> Result<ExtractionResponse, AppError> {
        let body = ExtractRequest {
            audio_base64: base64::engine::general_purpose::STANDARD.encode(audio),
            content_type,
            filename_hint,
            max_duration_seconds: Some(max_duration_seconds),
        };

        let started = std::time::Instant::now();
        let response = self
            .http
            .post(format!("{}/internal/v1/extract", self.base_url))
            .json(&body)
            .send()
            .await
            .map_err(|e| AppError::ProcessorUnavailable(e.to_string()))?;
        metrics::histogram!(crate::telemetry::metric_names::FINGERPRINT_EXTRACTION_LATENCY)
            .record(started.elapsed().as_millis() as f64);

        if response.status() == reqwest::StatusCode::UNPROCESSABLE_ENTITY {
            let detail = response
                .json::<ProcessorErrorBody>()
                .await
                .ok()
                .and_then(|b| b.detail)
                .unwrap_or_else(|| "audio rejected by processor".to_string());
            return Err(AppError::ProcessorRejected(detail));
        }

        if !response.status().is_success() {
            return Err(AppError::ProcessorUnavailable(format!(
                "processor returned status {}",
                response.status()
            )));
        }

        response
            .json::<ExtractionResponse>()
            .await
            .map_err(|e| AppError::ProcessorUnavailable(format!("invalid processor response: {e}")))
    }
}

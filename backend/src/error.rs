//! Central error type. Every fallible path in the application converges on
//! `AppError` so HTTP responses are consistent and internal details (SQL
//! errors, stack traces, file paths) never leak to clients.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("audio payload too large: {size} bytes exceeds limit of {limit} bytes")]
    AudioTooLarge { size: usize, limit: usize },

    #[error("invalid audio: {0}")]
    InvalidAudio(String),

    #[error(
        "fingerprint algorithm version mismatch: processor produced v{actual}, backend expects v{expected}"
    )]
    AlgorithmVersionMismatch { expected: i16, actual: i16 },

    #[error("no audio provided")]
    MissingAudio,

    #[error("song not found")]
    SongNotFound,

    #[error("recognition not found")]
    RecognitionNotFound,

    #[error("processor service unavailable: {0}")]
    ProcessorUnavailable(String),

    #[error("processor rejected audio: {0}")]
    ProcessorRejected(String),

    #[error("unauthorized")]
    Unauthorized,

    #[error("rate limit exceeded")]
    RateLimited,

    #[error("invalid request: {0}")]
    BadRequest(String),

    #[error("internal error")]
    Internal(#[from] anyhow::Error),

    #[error("database error")]
    Database(#[from] sqlx::Error),
}

#[derive(Serialize)]
struct ErrorBody {
    error: &'static str,
    message: String,
}

impl AppError {
    fn code(&self) -> &'static str {
        match self {
            AppError::AudioTooLarge { .. } => "AUDIO_TOO_LARGE",
            AppError::InvalidAudio(_) => "INVALID_AUDIO",
            AppError::AlgorithmVersionMismatch { .. } => "ALGORITHM_VERSION_MISMATCH",
            AppError::MissingAudio => "MISSING_AUDIO",
            AppError::SongNotFound => "SONG_NOT_FOUND",
            AppError::RecognitionNotFound => "RECOGNITION_NOT_FOUND",
            AppError::ProcessorUnavailable(_) => "PROCESSOR_UNAVAILABLE",
            AppError::ProcessorRejected(_) => "PROCESSOR_REJECTED",
            AppError::Unauthorized => "UNAUTHORIZED",
            AppError::RateLimited => "RATE_LIMITED",
            AppError::BadRequest(_) => "BAD_REQUEST",
            AppError::Internal(_) => "INTERNAL_ERROR",
            AppError::Database(_) => "INTERNAL_ERROR",
        }
    }

    fn status(&self) -> StatusCode {
        match self {
            AppError::AudioTooLarge { .. } => StatusCode::PAYLOAD_TOO_LARGE,
            AppError::InvalidAudio(_) | AppError::MissingAudio => StatusCode::UNPROCESSABLE_ENTITY,
            AppError::SongNotFound | AppError::RecognitionNotFound => StatusCode::NOT_FOUND,
            AppError::ProcessorUnavailable(_) => StatusCode::BAD_GATEWAY,
            AppError::ProcessorRejected(_) => StatusCode::UNPROCESSABLE_ENTITY,
            AppError::AlgorithmVersionMismatch { .. } => StatusCode::CONFLICT,
            AppError::Unauthorized => StatusCode::UNAUTHORIZED,
            AppError::RateLimited => StatusCode::TOO_MANY_REQUESTS,
            AppError::BadRequest(_) => StatusCode::BAD_REQUEST,
            AppError::Internal(_) | AppError::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    /// The message actually shown to the client. Internal errors are
    /// deliberately generic — the real detail goes to `tracing::error!`
    /// only, never the response body.
    fn public_message(&self) -> String {
        match self {
            AppError::Internal(_) | AppError::Database(_) => {
                "an internal error occurred".to_string()
            }
            other => other.to_string(),
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        if matches!(self, AppError::Internal(_) | AppError::Database(_)) {
            tracing::error!(error = %self, "internal error");
        }
        let status = self.status();
        let body = ErrorBody {
            error: self.code(),
            message: self.public_message(),
        };
        (status, Json(body)).into_response()
    }
}

pub type AppResult<T> = Result<T, AppError>;

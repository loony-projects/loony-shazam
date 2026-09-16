//! Central application configuration, loaded once from environment
//! variables at startup. Nothing outside this module should read `env::var`
//! directly — that keeps every tunable documented and defaultable in one
//! place (mirrors the approach taken in `processor/config.py`).

use std::env;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub database_url: String,
    pub redis_url: Option<String>,
    pub processor_url: String,
    pub api_host: String,
    pub api_port: u16,

    /// Hard cap on the raw audio upload size for the public recognition
    /// endpoint. Rejected before we even attempt to read the full body.
    pub max_audio_bytes: usize,
    /// Hard cap on decoded audio duration accepted for a *query* (not
    /// ingestion, which goes through the Python CLI directly).
    pub max_audio_duration_seconds: f64,

    pub fingerprint_algorithm_version: i16,

    pub processor_timeout: Duration,
    pub http_request_timeout: Duration,

    /// Admin endpoints require this bearer token. There is no default in
    /// production; startup fails loudly if unset outside of `dev` env.
    pub admin_api_key: Option<String>,

    pub rate_limit_requests_per_minute: u32,
    pub rate_limit_burst: u32,

    pub matching: MatchingConfig,
}

/// Tunables for the offset-voting matcher (see backend/src/matching).
/// Mirrors the "MATCH SCORE" / "UNKNOWN RESULT" sections of the product
/// spec: thresholds are explicit and configurable, never hard-coded deep
/// inside the scoring function.
#[derive(Debug, Clone)]
pub struct MatchingConfig {
    /// Width of an offset-histogram bucket in milliseconds. Buckets
    /// tolerate mic latency, preprocessing jitter, and small clock drift
    /// between query and reference frame grids without splitting what
    /// should be one dominant offset into several.
    pub offset_bucket_ms: i64,
    /// A candidate must accumulate at least this many votes in its
    /// dominant offset bucket to be considered at all.
    pub min_dominant_votes: u32,
    /// A candidate's score (see matching::scoring) must reach this value.
    pub min_score: f64,
    /// Fraction of the query's own duration that must be "covered" by
    /// matched landmarks (approximated via unique matched query hashes /
    /// total query hashes).
    pub min_coverage: f64,
    /// The best candidate's dominant-vote count must exceed the runner-up's
    /// by at least this ratio, so two similar-sounding songs don't produce
    /// an unstable/ambiguous result.
    pub min_margin_ratio: f64,
}

impl Default for MatchingConfig {
    fn default() -> Self {
        Self {
            offset_bucket_ms: 100,
            min_dominant_votes: 5,
            min_score: 4.0,
            min_coverage: 0.03,
            min_margin_ratio: 1.3,
        }
    }
}

impl AppConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        let database_url = require_env("DATABASE_URL")?;
        let redis_url = env::var("REDIS_URL").ok();
        let processor_url =
            env::var("PROCESSOR_URL").unwrap_or_else(|_| "http://localhost:8001".to_string());
        let api_host = env::var("API_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
        let api_port = parse_env("API_PORT", 8080u16)?;

        let max_audio_bytes = parse_env("MAX_AUDIO_BYTES", 20 * 1024 * 1024usize)?;
        let max_audio_duration_seconds = parse_env("MAX_AUDIO_DURATION_SECONDS", 15.0f64)?;
        let fingerprint_algorithm_version = parse_env("FINGERPRINT_ALGORITHM_VERSION", 1i16)?;

        let processor_timeout_ms = parse_env("PROCESSOR_TIMEOUT_MS", 8_000u64)?;
        let http_request_timeout_ms = parse_env("HTTP_REQUEST_TIMEOUT_MS", 15_000u64)?;

        let admin_api_key = env::var("ADMIN_API_KEY").ok();

        let rate_limit_requests_per_minute = parse_env("RATE_LIMIT_RPM", 60u32)?;
        let rate_limit_burst = parse_env("RATE_LIMIT_BURST", 10u32)?;

        let matching = MatchingConfig {
            offset_bucket_ms: parse_env("MATCH_OFFSET_BUCKET_MS", 100i64)?,
            min_dominant_votes: parse_env("MATCH_MIN_DOMINANT_VOTES", 5u32)?,
            min_score: parse_env("MATCH_MIN_SCORE", 4.0f64)?,
            min_coverage: parse_env("MATCH_MIN_COVERAGE", 0.03f64)?,
            min_margin_ratio: parse_env("MATCH_MIN_MARGIN_RATIO", 1.3f64)?,
        };

        Ok(Self {
            database_url,
            redis_url,
            processor_url,
            api_host,
            api_port,
            max_audio_bytes,
            max_audio_duration_seconds,
            fingerprint_algorithm_version,
            processor_timeout: Duration::from_millis(processor_timeout_ms),
            http_request_timeout: Duration::from_millis(http_request_timeout_ms),
            admin_api_key,
            rate_limit_requests_per_minute,
            rate_limit_burst,
            matching,
        })
    }
}

fn require_env(key: &str) -> anyhow::Result<String> {
    env::var(key).map_err(|_| anyhow::anyhow!("required environment variable {key} is not set"))
}

fn parse_env<T>(key: &str, default: T) -> anyhow::Result<T>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    match env::var(key) {
        Ok(raw) => raw
            .parse::<T>()
            .map_err(|e| anyhow::anyhow!("invalid value for {key}: {e}")),
        Err(_) => Ok(default),
    }
}

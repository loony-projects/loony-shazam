use std::sync::Arc;

use metrics_exporter_prometheus::PrometheusHandle;
use sqlx::PgPool;

use crate::config::AppConfig;
use crate::fingerprint::ProcessorClient;
use crate::matching::MatchingEngine;
use crate::repositories::{FingerprintRepository, RecognitionRepository, SongRepository};
use crate::routes::RateLimiter;
use crate::storage::RedisCache;

/// Shared application state, cheaply cloneable (everything inside is
/// already an `Arc`/pool handle) and injected into every Axum handler.
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<AppConfig>,
    pub db: PgPool,
    pub redis: RedisCache,
    pub processor: ProcessorClient,
    pub songs: SongRepository,
    pub fingerprints: FingerprintRepository,
    pub recognitions: RecognitionRepository,
    pub matching_engine: Arc<MatchingEngine>,
    pub metrics_handle: PrometheusHandle,
    pub rate_limiter: Arc<RateLimiter>,
}

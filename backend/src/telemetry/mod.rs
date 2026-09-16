//! Structured logging + Prometheus-compatible metrics setup.
//!
//! Logging: JSON structured logs via `tracing`, one line per request with
//! a request id, latency, and status — never raw audio bytes or uploaded
//! file contents (see docs/architecture.md privacy notes).
//!
//! Metrics: counters/histograms named per the product spec
//! (`requests_total`, `recognition_success_total`,
//! `recognition_failure_total`, `recognition_latency`,
//! `fingerprint_lookup_latency`, `fingerprint_extraction_latency`,
//! `db_query_latency`), exported on `GET /metrics` in Prometheus text
//! format.

use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

pub fn init_tracing() {
    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info,sqlx=warn"));

    tracing_subscriber::registry()
        .with(env_filter)
        .with(tracing_subscriber::fmt::layer().json().with_target(true))
        .init();
}

pub fn init_metrics() -> PrometheusHandle {
    PrometheusBuilder::new()
        .install_recorder()
        .expect("failed to install Prometheus metrics recorder")
}

/// For integration tests only: a `PrometheusHandle` that is *not*
/// installed as the global recorder (the global recorder can only be
/// installed once per process, and integration tests build multiple
/// `AppState`s). `metrics::counter!`/`histogram!` calls become no-ops in
/// this configuration, which is fine — tests assert on behavior, not on
/// exported metric text.
pub fn test_metrics_handle() -> PrometheusHandle {
    PrometheusBuilder::new().build_recorder().handle()
}

pub mod metric_names {
    pub const REQUESTS_TOTAL: &str = "requests_total";
    pub const RECOGNITION_SUCCESS_TOTAL: &str = "recognition_success_total";
    pub const RECOGNITION_FAILURE_TOTAL: &str = "recognition_failure_total";
    pub const RECOGNITION_LATENCY: &str = "recognition_latency_ms";
    pub const FINGERPRINT_LOOKUP_LATENCY: &str = "fingerprint_lookup_latency_ms";
    pub const FINGERPRINT_EXTRACTION_LATENCY: &str = "fingerprint_extraction_latency_ms";
    pub const DB_QUERY_LATENCY: &str = "db_query_latency_ms";
}

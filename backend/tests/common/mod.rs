//! Shared setup for integration tests. Requires a reachable Postgres at
//! `TEST_DATABASE_URL` (falls back to a sensible local default matching
//! `docker-compose.yml`). Each test gets its own fully-migrated schema in
//! a throwaway database so tests can run concurrently without clashing.

use std::sync::Arc;

use loony_shazam_backend::config::{AppConfig, MatchingConfig};
use loony_shazam_backend::fingerprint::ProcessorClient;
use loony_shazam_backend::matching::{MatchingEngine, PostgresFingerprintIndex};
use loony_shazam_backend::repositories::{
    FingerprintRepository, RecognitionRepository, SongRepository,
};
use loony_shazam_backend::routes::RateLimiter;
use loony_shazam_backend::state::AppState;
use loony_shazam_backend::storage::{self, RedisCache};
use sqlx::PgPool;

pub fn base_database_url() -> String {
    std::env::var("TEST_DATABASE_URL").unwrap_or_else(|_| {
        "postgresql://postgres:devpass@localhost:55432/loony_shazam".to_string()
    })
}

/// Creates a fresh, uniquely-named database on the same server, runs
/// migrations, and returns a pool pointed at it. The caller is responsible
/// for nothing else — throwaway test databases are cheap and we don't
/// bother dropping them (a local/dev/test-only Postgres instance).
pub async fn setup_test_db() -> PgPool {
    let base_url = base_database_url();
    let db_name = format!("test_{}", uuid::Uuid::new_v4().simple());

    let admin_pool = PgPool::connect(&base_url)
        .await
        .expect("connect to admin/base database for test setup");
    sqlx::query(&format!("CREATE DATABASE \"{db_name}\""))
        .execute(&admin_pool)
        .await
        .expect("create test database");
    admin_pool.close().await;

    let mut url = url::Url::parse(&base_url).expect("parse TEST_DATABASE_URL");
    url.set_path(&format!("/{db_name}"));

    let pool = storage::create_pool(url.as_str())
        .await
        .expect("connect to fresh test database");
    storage::run_migrations(&pool)
        .await
        .expect("run migrations on test database");
    pool
}

// Not every integration test binary that includes this module uses every
// helper in it (each `tests/*.rs` file compiles its own copy of
// `tests/common/`) — allow dead code rather than fork the module.
#[allow(dead_code)]
pub async fn test_app_state(pool: PgPool, matching: MatchingConfig) -> AppState {
    let config = Arc::new(AppConfig {
        database_url: String::new(),
        redis_url: None,
        processor_url: "http://localhost:1".to_string(), // deliberately unreachable; not used by JSON-fingerprint tests
        api_host: "127.0.0.1".to_string(),
        api_port: 0,
        max_audio_bytes: 20 * 1024 * 1024,
        max_audio_duration_seconds: 15.0,
        fingerprint_algorithm_version: 1,
        processor_timeout: std::time::Duration::from_millis(100),
        http_request_timeout: std::time::Duration::from_secs(5),
        admin_api_key: Some("test-admin-key".to_string()),
        rate_limit_requests_per_minute: 100_000,
        rate_limit_burst: 100_000,
        artwork_dir: {
            let dir = std::env::temp_dir().join("loony-shazam-test-artwork");
            std::fs::create_dir_all(&dir).expect("create test artwork dir");
            dir
        },
        matching,
    });

    let songs = SongRepository::new(pool.clone());
    let fingerprints = FingerprintRepository::new(pool.clone());
    let recognitions = RecognitionRepository::new(pool.clone());
    let index = Arc::new(PostgresFingerprintIndex::new(pool.clone()));
    let matching_engine = Arc::new(MatchingEngine::new(
        index,
        songs.clone(),
        config.matching.clone(),
    ));
    let processor = ProcessorClient::new(config.processor_url.clone(), config.processor_timeout)
        .expect("build processor client");
    let rate_limiter = RateLimiter::new(
        config.rate_limit_requests_per_minute,
        config.rate_limit_burst,
    );

    AppState {
        config,
        db: pool,
        redis: RedisCache::connect(None).await,
        processor,
        songs,
        fingerprints,
        recognitions,
        matching_engine,
        metrics_handle: loony_shazam_backend::telemetry::test_metrics_handle(),
        rate_limiter,
    }
}

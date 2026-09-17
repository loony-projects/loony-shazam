use std::net::SocketAddr;
use std::sync::Arc;

use loony_shazam_backend::config::AppConfig;
use loony_shazam_backend::fingerprint::ProcessorClient;
use loony_shazam_backend::matching::{MatchingEngine, PostgresFingerprintIndex};
use loony_shazam_backend::repositories::{
    FingerprintRepository, RecognitionRepository, SongRepository,
};
use loony_shazam_backend::routes::{self, RateLimiter};
use loony_shazam_backend::state::AppState;
use loony_shazam_backend::storage::{self, RedisCache};
use loony_shazam_backend::telemetry;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // .env is optional (docker-compose / prod inject real env vars); local
    // dev can copy .env.example -> .env.
    let _ = dotenvy::dotenv();

    telemetry::init_tracing();
    let metrics_handle = telemetry::init_metrics();

    let config = Arc::new(AppConfig::from_env()?);
    tracing::info!(
        api_port = config.api_port,
        processor_url = %config.processor_url,
        "starting loony-shazam backend"
    );

    let db = storage::create_pool(&config.database_url).await?;
    storage::run_migrations(&db).await?;
    tracing::info!("database migrations applied");

    std::fs::create_dir_all(&config.artwork_dir)?;

    let redis = RedisCache::connect(config.redis_url.as_deref()).await;

    let processor = ProcessorClient::new(config.processor_url.clone(), config.processor_timeout)?;
    if !processor.health().await {
        tracing::warn!("processor service not reachable at startup; will retry per-request");
    }

    let songs = SongRepository::new(db.clone());
    let fingerprints = FingerprintRepository::new(db.clone());
    let recognitions = RecognitionRepository::new(db.clone());

    let index = Arc::new(PostgresFingerprintIndex::new(db.clone()));
    let matching_engine = Arc::new(MatchingEngine::new(
        index,
        songs.clone(),
        config.matching.clone(),
    ));

    let rate_limiter = RateLimiter::new(
        config.rate_limit_requests_per_minute,
        config.rate_limit_burst,
    );
    routes::spawn_eviction_task(rate_limiter.clone());

    let state = AppState {
        config: config.clone(),
        db,
        redis,
        processor,
        songs,
        fingerprints,
        recognitions,
        matching_engine,
        metrics_handle,
        rate_limiter,
    };

    let app = routes::build_router(state);

    let addr: SocketAddr = format!("{}:{}", config.api_host, config.api_port).parse()?;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!(%addr, "listening");

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await?;

    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    tracing::info!("shutdown signal received");
}

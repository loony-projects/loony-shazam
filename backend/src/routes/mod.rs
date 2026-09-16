mod admin_auth;
mod rate_limit;

pub use rate_limit::{spawn_eviction_task, RateLimiter};

use std::time::Duration;

use axum::extract::DefaultBodyLimit;
use axum::http::StatusCode;
use axum::middleware::{self, Next};
use axum::response::Response;
use axum::routing::{delete, get, post};
use axum::Router;
use tower_http::cors::CorsLayer;
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;

use crate::handlers::{admin_handler, health_handler, recognition_handler, song_handler};
use crate::state::AppState;

pub fn build_router(state: AppState) -> Router {
    let admin_routes = Router::new()
        .route("/api/v1/admin/ingest", post(admin_handler::admin_ingest))
        .route(
            "/api/v1/admin/songs/:id/fingerprints",
            delete(admin_handler::admin_delete_song_fingerprints),
        )
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            admin_auth::require_admin,
        ));

    // The audio-upload route gets its own, larger body-size limit (bounded
    // by MAX_AUDIO_BYTES) instead of Axum's default 2MB request body cap.
    let recognition_audio_routes = Router::new()
        .route(
            "/api/v1/recognitions/audio",
            post(recognition_handler::recognize_audio),
        )
        .layer(DefaultBodyLimit::max(state.config.max_audio_bytes));

    let api_routes = Router::new()
        .route(
            "/api/v1/recognitions",
            post(recognition_handler::recognize_fingerprints),
        )
        .route(
            "/api/v1/recognitions/:id",
            get(recognition_handler::get_recognition),
        )
        .route("/api/v1/songs/:id", get(song_handler::get_song))
        .merge(recognition_audio_routes)
        .merge(admin_routes)
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            rate_limit::rate_limit_middleware,
        ));

    // Health/readiness/metrics are never rate-limited: orchestrators
    // (k8s liveness/readiness probes, Prometheus scrapes) poll these
    // frequently and must not be throttled.
    let ops_routes = Router::new()
        .route("/health", get(health_handler::health))
        .route("/ready", get(health_handler::ready))
        .route("/metrics", get(health_handler::metrics));

    Router::new()
        .merge(api_routes)
        .merge(ops_routes)
        .layer(middleware::from_fn(requests_total_middleware))
        .layer(TraceLayer::new_for_http())
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            state.config.http_request_timeout,
        ))
        .layer(cors_layer())
        .with_state(state)
}

/// `requests_total` — the coarse, always-on request counter from the
/// product spec's observability section, labeled by route and status
/// class so it stays low-cardinality even under heavy traffic.
async fn requests_total_middleware(request: axum::extract::Request, next: Next) -> Response {
    let path = request
        .extensions()
        .get::<axum::extract::MatchedPath>()
        .map(|p| p.as_str().to_string())
        .unwrap_or_else(|| request.uri().path().to_string());
    let method = request.method().to_string();

    let response = next.run(request).await;

    let status_class = match response.status().as_u16() {
        200..=299 => "2xx",
        300..=399 => "3xx",
        400..=499 => "4xx",
        _ => "5xx",
    };
    metrics::counter!(
        crate::telemetry::metric_names::REQUESTS_TOTAL,
        "path" => path,
        "method" => method,
        "status" => status_class,
    )
    .increment(1);

    response
}

fn cors_layer() -> CorsLayer {
    // Permissive by default (this is a public read/recognize API consumed
    // by a mobile client, not a browser session with cookies); tighten
    // `allow_origin` to a specific origin list if a web client is added.
    CorsLayer::permissive().max_age(Duration::from_secs(3600))
}

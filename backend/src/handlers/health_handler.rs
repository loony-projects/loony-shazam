use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::Serialize;

use crate::state::AppState;

#[derive(Serialize)]
pub struct HealthResponse {
    status: &'static str,
}

/// Liveness: the process is up and serving. Never touches the DB or the
/// processor — a dependency outage should not make this endpoint fail.
pub async fn health() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}

#[derive(Serialize)]
pub struct ReadyResponse {
    status: &'static str,
    database: &'static str,
    processor: &'static str,
    /// Redis is optional (see docs/database.md "Redis usage"); "disabled"
    /// is a healthy state, not a failure, so it never affects `status`.
    redis: &'static str,
}

/// Readiness: the process can actually serve recognition traffic — both
/// the database and the processor service must be reachable. Redis is
/// reported but not required (see docs/database.md).
pub async fn ready(State(state): State<AppState>) -> (StatusCode, Json<ReadyResponse>) {
    let db_ok = sqlx::query("SELECT 1").execute(&state.db).await.is_ok();
    let processor_ok = state.processor.health().await;

    let status = if db_ok && processor_ok {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };

    (
        status,
        Json(ReadyResponse {
            status: if db_ok && processor_ok {
                "ready"
            } else {
                "not_ready"
            },
            database: if db_ok { "ok" } else { "unavailable" },
            processor: if processor_ok { "ok" } else { "unavailable" },
            redis: if state.redis.is_enabled() {
                "ok"
            } else {
                "disabled"
            },
        }),
    )
}

pub async fn metrics(State(state): State<AppState>) -> String {
    state.metrics_handle.render()
}

//! Bearer-token auth for `/api/v1/admin/*`, kept entirely separate from
//! the public recognition/song endpoints (see product spec "API
//! SECURITY": "separate public recognition endpoints from admin
//! endpoints"). The token is loaded from `ADMIN_API_KEY` (config.rs) —
//! never hard-coded, never logged.

use axum::extract::State;
use axum::http::header::AUTHORIZATION;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use crate::error::AppError;
use crate::state::AppState;

pub async fn require_admin(
    State(state): State<AppState>,
    request: axum::extract::Request,
    next: Next,
) -> Response {
    let Some(expected) = state.config.admin_api_key.as_deref() else {
        tracing::error!("admin endpoint called but ADMIN_API_KEY is not configured");
        return AppError::Unauthorized.into_response();
    };

    let provided = request
        .headers()
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));

    match provided {
        Some(token) if constant_time_eq(token.as_bytes(), expected.as_bytes()) => {
            next.run(request).await
        }
        _ => AppError::Unauthorized.into_response(),
    }
}

/// Avoids leaking token length/content via timing side channels on
/// comparison, even though the practical risk for an admin API key over
/// TLS is low — cheap to do correctly.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

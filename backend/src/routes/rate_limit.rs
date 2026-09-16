//! Minimal in-memory per-IP token-bucket rate limiter.
//!
//! Deliberately simple rather than pulling in a full rate-limiting crate:
//! it protects a single backend instance from casual abuse/misbehaving
//! clients. It is explicitly **not** sufficient for a multi-instance
//! production deployment (each instance has its own independent bucket
//! per IP) — see docs/deployment.md, which documents fronting the fleet
//! with a shared-state limiter (e.g. Redis-backed, or at a reverse
//! proxy/API gateway) for that case.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::extract::{ConnectInfo, State};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use tokio::sync::Mutex;

use crate::error::AppError;
use crate::state::AppState;

struct Bucket {
    tokens: f64,
    last_refill: Instant,
}

pub struct RateLimiter {
    buckets: Mutex<HashMap<IpAddr, Bucket>>,
    capacity: f64,
    refill_per_second: f64,
}

impl RateLimiter {
    pub fn new(requests_per_minute: u32, burst: u32) -> Arc<Self> {
        Arc::new(Self {
            buckets: Mutex::new(HashMap::new()),
            capacity: burst.max(1) as f64,
            refill_per_second: requests_per_minute.max(1) as f64 / 60.0,
        })
    }

    async fn check(&self, ip: IpAddr) -> bool {
        let mut buckets = self.buckets.lock().await;
        let now = Instant::now();
        let bucket = buckets.entry(ip).or_insert_with(|| Bucket {
            tokens: self.capacity,
            last_refill: now,
        });

        let elapsed = now.duration_since(bucket.last_refill).as_secs_f64();
        bucket.tokens = (bucket.tokens + elapsed * self.refill_per_second).min(self.capacity);
        bucket.last_refill = now;

        if bucket.tokens >= 1.0 {
            bucket.tokens -= 1.0;
            true
        } else {
            false
        }
    }

    /// Prevents unbounded memory growth from a long-running process seeing
    /// many distinct IPs; called periodically from a background task.
    pub async fn evict_idle(&self, idle_for: Duration) {
        let mut buckets = self.buckets.lock().await;
        let now = Instant::now();
        buckets.retain(|_, b| now.duration_since(b.last_refill) < idle_for);
    }
}

pub async fn rate_limit_middleware(
    State(state): State<AppState>,
    connect_info: Option<ConnectInfo<std::net::SocketAddr>>,
    request: axum::extract::Request,
    next: Next,
) -> Response {
    // ConnectInfo is populated by `into_make_service_with_connect_info`
    // (see main.rs) for every real connection. It's only absent when a
    // handler is invoked directly without going through that service
    // (e.g. `tower::ServiceExt::oneshot` in tests) — fail open rather than
    // 500 in that case.
    let Some(ConnectInfo(addr)) = connect_info else {
        return next.run(request).await;
    };

    let allowed = state.rate_limiter.check(addr.ip()).await;
    if !allowed {
        return AppError::RateLimited.into_response();
    }
    next.run(request).await
}

pub fn spawn_eviction_task(limiter: Arc<RateLimiter>) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(300));
        loop {
            interval.tick().await;
            limiter.evict_idle(Duration::from_secs(3600)).await;
        }
    });
}

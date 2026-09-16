//! Thin Redis wrapper used only where it removes load from Postgres
//! without becoming a second source of truth (song metadata cache today).
//! The fingerprint lookup itself never touches Redis — see
//! docs/database.md "Redis usage". Redis is optional: if `REDIS_URL` is
//! unset, `RedisCache::disabled()` produces a no-op cache and the backend
//! runs fine without it (just always hitting Postgres for metadata).

use redis::aio::ConnectionManager;
use serde::{de::DeserializeOwned, Serialize};

#[derive(Clone)]
pub struct RedisCache {
    manager: Option<ConnectionManager>,
}

impl RedisCache {
    pub async fn connect(redis_url: Option<&str>) -> Self {
        let Some(url) = redis_url else {
            return Self { manager: None };
        };
        match redis::Client::open(url) {
            Ok(client) => match ConnectionManager::new(client).await {
                Ok(manager) => {
                    tracing::info!("redis cache connected");
                    Self {
                        manager: Some(manager),
                    }
                }
                Err(err) => {
                    tracing::warn!(error = %err, "redis unavailable at startup, running without cache");
                    Self { manager: None }
                }
            },
            Err(err) => {
                tracing::warn!(error = %err, "invalid REDIS_URL, running without cache");
                Self { manager: None }
            }
        }
    }

    pub async fn get_json<T: DeserializeOwned>(&self, key: &str) -> Option<T> {
        let mut manager = self.manager.clone()?;
        let raw: Option<String> = redis::AsyncCommands::get(&mut manager, key).await.ok()?;
        raw.and_then(|s| serde_json::from_str(&s).ok())
    }

    pub async fn set_json<T: Serialize>(&self, key: &str, value: &T, ttl_seconds: u64) {
        let Some(mut manager) = self.manager.clone() else {
            return;
        };
        if let Ok(serialized) = serde_json::to_string(value) {
            let _: Result<(), _> =
                redis::AsyncCommands::set_ex(&mut manager, key, serialized, ttl_seconds).await;
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.manager.is_some()
    }
}

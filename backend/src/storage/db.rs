use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

pub async fn create_pool(database_url: &str) -> anyhow::Result<PgPool> {
    let pool = PgPoolOptions::new()
        .max_connections(20)
        .acquire_timeout(std::time::Duration::from_secs(10))
        .connect(database_url)
        .await?;
    Ok(pool)
}

/// Applies every migration in `database/migrations` that hasn't run yet,
/// tracked in sqlx's own `_sqlx_migrations` table. Safe to call on every
/// startup — a no-op once the schema is current.
pub async fn run_migrations(pool: &PgPool) -> anyhow::Result<()> {
    sqlx::migrate!("../database/migrations").run(pool).await?;
    Ok(())
}

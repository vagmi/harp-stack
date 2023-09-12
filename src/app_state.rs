use anyhow::Result;
use sqlx::postgres::PgPoolOptions;

#[derive(Debug, Clone)]
pub struct AppState {
    pub pool: sqlx::PgPool,
}

impl AppState {
    pub async fn new() -> Result<Self> {
        tracing::info!(commit_sha = std::env::var("COMMIT_SHA").unwrap_or(String::from("development")), "initializing...");
        tracing::info!(database_url = std::env::var("DATABASE_URL")?, "Connecting to database");
        let pool = PgPoolOptions::new()
            .acquire_timeout(std::time::Duration::from_secs(30))
            .max_connections(5)
            .connect(&std::env::var("DATABASE_URL")?)
            .await?;
        tracing::info!("Running migrations");
        sqlx::migrate!("./migrations").run(&pool).await?;
        tracing::info!("Migration complete");
        Ok(Self { pool })
    }
}

use anyhow::Result;
use sqlx::PgPool;
use tracing::info;

pub struct Migration {}

impl Migration {
    pub async fn run(pool: &PgPool) -> Result<()> {
        info!("Running database migrations...");

        sqlx::migrate!("./migrations").run(pool).await?;

        info!("Migrations completed successfully");
        Ok(())
    }
}

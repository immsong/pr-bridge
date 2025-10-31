use anyhow::Result;
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;

pub struct Pool {}

impl Pool {
    pub async fn create_pool(database_url: &str) -> Result<PgPool> {
        PgPoolOptions::new()
            .max_connections(10)
            .connect(database_url)
            .await
            .map_err(Into::into)
    }
}

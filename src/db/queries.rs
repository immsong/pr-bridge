use crate::db::{Repository, models::SystemSetting};
use anyhow::Result;
use sqlx::PgPool;
use std::collections::HashMap;

pub struct Queries {}

impl Queries {
    pub async fn get_repositories(pool: &PgPool) -> Result<Vec<Repository>> {
        let repositories = sqlx::query_as!(Repository, "SELECT * FROM repositories")
            .fetch_all(pool)
            .await?;
        Ok(repositories)
    }

    pub async fn get_system_settings(pool: &PgPool) -> Result<HashMap<String, String>> {
        let system_settings = sqlx::query_as!(SystemSetting, "SELECT * FROM system_settings")
            .fetch_all(pool)
            .await?;
        Ok(system_settings
            .into_iter()
            .map(|s| (s.key, s.value))
            .collect())
    }
}

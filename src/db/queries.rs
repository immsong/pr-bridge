//! DB 쿼리 모듈
//!
//! ## 개발 환경 설정
//! - DB 실행 후에도 SQLx 관련 경고가 있을 경우: rust-analyzer 재시작 필요
//! - Ctrl+Shift+P → "rust-analyzer: Restart Server"

use crate::db::{
    SystemSetting, Tag,
    models::{Branch, Repository},
};
use anyhow::Result;
use sqlx::PgPool;
use std::collections::HashMap;

pub struct Queries {}

impl Queries {
    // repository 관련
    pub async fn create_repository(
        pool: &PgPool,
        owner: String,
        name: String,
        poll_interval_seconds: Option<i32>,
    ) -> Result<Repository> {
        let poll_interval = match poll_interval_seconds {
            Some(val) => val,
            None => {
                let default_val = sqlx::query_scalar!(
                    "SELECT value FROM system_settings WHERE key = 'github_api_poll_interval'"
                )
                .fetch_one(pool)
                .await?;

                default_val.parse::<i32>().unwrap_or(300) // 파싱 실패 시 300 사용
            }
        };

        let repository = sqlx::query_as!(Repository, "INSERT INTO repositories (owner, name, poll_interval_seconds) VALUES ($1, $2, $3) RETURNING *", owner, name, poll_interval)
            .fetch_one(pool)
            .await?;
        Ok(repository)
    }

    pub async fn update_repository(
        pool: &PgPool,
        repo_id: i32,
        is_active: Option<bool>,
        poll_interval_seconds: Option<i32>,
    ) -> Result<Repository> {
        let result = sqlx::query!(
            "UPDATE repositories SET is_active = $1, poll_interval_seconds = $2 WHERE id = $3",
            is_active,
            poll_interval_seconds,
            repo_id
        )
        .execute(pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(anyhow::anyhow!("Failed to update repository"));
        }

        let repository = sqlx::query_as!(
            Repository,
            "SELECT * FROM repositories WHERE id = $1",
            repo_id
        )
        .fetch_one(pool)
        .await?;

        Ok(repository)
    }

    pub async fn delete_repository(pool: &PgPool, repo_id: i32) -> Result<()> {
        let result = sqlx::query!("DELETE FROM repositories WHERE id = $1", repo_id)
            .execute(pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(anyhow::anyhow!("Failed to delete repository"));
        }

        Ok(())
    }

    pub async fn get_repositories(pool: &PgPool) -> Result<Vec<Repository>> {
        let repositories = sqlx::query_as!(Repository, "SELECT * FROM repositories")
            .fetch_all(pool)
            .await?;
        Ok(repositories)
    }

    pub async fn get_repository(pool: &PgPool, repo_id: i32) -> Result<Repository> {
        let repository = sqlx::query_as!(
            Repository,
            "SELECT * FROM repositories WHERE id = $1",
            repo_id
        )
        .fetch_one(pool)
        .await?;
        Ok(repository)
    }

    // branch 관련
    pub async fn get_branches(pool: &PgPool) -> Result<Vec<Branch>> {
        let branches = sqlx::query_as!(Branch, "SELECT * FROM branches")
            .fetch_all(pool)
            .await?;
        Ok(branches)
    }

    pub async fn insert_branch(
        pool: &PgPool,
        repository_id: i32,
        branch_names: String,
        branch_head_shas: String,
    ) -> Result<Branch> {
        let branch = sqlx::query_as!(
            Branch,
            "INSERT INTO branches (repository_id, name, head_sha) VALUES ($1, $2, $3) RETURNING *",
            repository_id,
            branch_names,
            branch_head_shas,
        )
        .fetch_one(pool)
        .await?;
    
        Ok(branch)
    }

    // tag 관련
    pub async fn get_tags(pool: &PgPool) -> Result<Vec<Tag>> {
        let tags = sqlx::query_as!(Tag, "SELECT * FROM tags")
            .fetch_all(pool)
            .await?;
        Ok(tags)
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

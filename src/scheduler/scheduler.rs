use crate::{db, git};
use anyhow::Result;
use sqlx::PgPool;
use tokio::time::{Duration, interval};
use tracing::{error, info};

pub struct Scheduler {
    pool: PgPool,
    pr_poll_interval: Duration,
    refs_sync_interval: Duration,
    github_token: Option<String>,
}

impl Scheduler {
    pub fn new(
        pool: PgPool,
        pr_interval: u64,
        refs_interval: u64,
        github_token: Option<String>,
    ) -> Self {
        Self {
            pool,
            pr_poll_interval: Duration::from_secs(pr_interval),
            refs_sync_interval: Duration::from_secs(refs_interval),
            github_token: github_token,
        }
    }

    /// 스케줄러 시작 - 두 개의 독립적인 태스크 실행
    pub async fn start(self) -> Result<()> {
        info!("Starting scheduler...");

        let pool1 = self.pool.clone();
        let pool2 = self.pool.clone();

        // PR 폴링 태스크 (60초)
        let pr_task =
            tokio::spawn(async move { Self::pr_polling_loop(pool1, self.pr_poll_interval).await });

        // refs 동기화 태스크 (300초)
        let refs_task = tokio::spawn(async move {
            Self::refs_sync_loop(pool2, self.refs_sync_interval, self.github_token.clone()).await
        });

        // 둘 중 하나라도 종료되면 에러
        let _ = tokio::try_join!(pr_task, refs_task)?;

        Ok(())
    }

    /// PR 폴링 루프
    async fn pr_polling_loop(pool: PgPool, interval_duration: Duration) -> Result<()> {
        let mut ticker = interval(interval_duration);

        loop {
            ticker.tick().await;
            info!("PR polling cycle...");

            // TODO: PR 폴링 로직

            info!("PR polling completed");
        }
    }

    /// Refs 동기화 루프
    async fn refs_sync_loop(
        pool: PgPool,
        interval_duration: Duration,
        github_token: Option<String>,
    ) -> Result<()> {
        let mut ticker = interval(interval_duration);

        loop {
            ticker.tick().await;
            info!("Refs sync cycle...");

            let repositories = db::Queries::get_repositories(&pool).await?;
            for repository in repositories {
                info!("Repository: {:?}", repository);

                let url = if let Some(token) = github_token.clone() {
                    format!(
                        "https://{}@github.com/{}/{}.git",
                        token, repository.owner, repository.name
                    )
                } else {
                    format!(
                        "https://github.com/{}/{}.git",
                        repository.owner, repository.name
                    )
                };

                let refs = git::Client::ls_remote(&url).await;
                match refs {
                    Ok(refs) => {
                        info!("Refs: {:?}", refs);
                    }
                    Err(e) => {
                        error!("Error: {:?}", e);
                    }
                }
            }

            info!("Refs sync completed");
        }
    }
}

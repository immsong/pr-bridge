use std::path::Path;

use crate::{db, git::GitClient};
use anyhow::Result;
use futures::future::try_join_all;
use sqlx::PgPool;
use tokio::time::{Duration, interval};
use tracing::{error, info};

#[derive(Debug, Clone)]
pub struct Scheduler {
    pub pool: PgPool,
    pub pr_poll_interval: Duration,
    pub refs_sync_interval: Duration,
    pub git_client: GitClient,
}

impl Scheduler {
    pub fn new(pool: PgPool, pr_interval: u64, refs_interval: u64, git_client: GitClient) -> Self {
        Self {
            pool,
            pr_poll_interval: Duration::from_secs(pr_interval),
            refs_sync_interval: Duration::from_secs(refs_interval),
            git_client: git_client,
        }
    }

    /// 스케줄러 시작
    pub async fn start(self) -> Result<()> {
        info!("Starting scheduler...");

        let mut join_handlers = Vec::new();

        // schedule 1 - repo sync
        {
            let mut ticker = interval(self.refs_sync_interval);

            let pool = self.pool.clone();
            let git_client = self.git_client.clone();
            let join_handler = tokio::spawn(async move {
                loop {
                    let repositories = db::Queries::get_repositories(&pool).await;
                    let branches = db::Queries::get_branches(&pool).await;

                    if repositories.is_err() || branches.is_err() {
                        error!(
                            "Failed to get repositories or branches: {:?}",
                            repositories.err().unwrap()
                        );
                        error!(
                            "Failed to get repositories or branches: {:?}",
                            branches.err().unwrap()
                        );
                        continue;
                    }

                    let repositories = repositories.unwrap();
                    let branches = branches.unwrap();

                    for repo in &repositories {
                        info!("Repository: {:?}", repo);
                        let mut is_synced = false;
                        for branch in &branches {
                            if branch.repository_id == repo.id {
                                is_synced = true;
                                break;
                            }
                        }

                        if !is_synced {
                            let path = format!("repositories/{}/{}", repo.owner, repo.name);
                            let dir_path = Path::new(&path);
                            if !dir_path.exists() {
                                let result = git_client
                                    .clone_repository(
                                        &format!(
                                            "https://github.com/{}/{}.git",
                                            repo.owner, repo.name
                                        ),
                                        &path,
                                    )
                                    .await;
                                if result.is_err() {
                                    error!(
                                        "Failed to clone repository: {:?}",
                                        result.err().unwrap()
                                    );
                                    continue;
                                }
                            }

                            let branches =
                                git_client.get_branches(&dir_path.to_str().unwrap()).await;
                            if branches.is_err() {
                                error!("Failed to get branches: {:?}", branches.err().unwrap());
                                continue;
                            }

                            for branch in branches.unwrap() {
                                let result =
                                    db::Queries::insert_branch(&pool, repo.id, branch.0, branch.1)
                                        .await;

                                if result.is_err() {
                                    error!("Failed to insert branch: {:?}", result.err().unwrap());
                                    continue;
                                } else {
                                    let branch = result.unwrap();
                                    info!("Inserted branch: {:?}", branch);
                                }
                            }
                        }
                        // for branch in branches.unwrap().clone() {
                        //     if branch.repository_id == repo.id {
                        //         info!("Branch: {:?}", branch);
                        //     }
                        // }
                    }

                    // for repo in repositories.unwrap().clone() {
                    //     for branch in branches.unwrap().clone() {
                    //         if branch.repository_id == repo.id {
                    //             info!("Branch: {:?}", branch);
                    //         }
                    //     }
                    // }

                    // let refs = git_client.clone_repository(&url, &format!("repositories/{}", repository.id)).await;
                    // match refs {
                    //     Ok(refs) => {
                    //         info!("Refs: {:?}", refs.head().unwrap().target().unwrap());
                    //     }
                    //     Err(e) => {
                    //         error!("Error: {:?}", e);
                    //     }
                    // }
                    // }

                    ticker.tick().await;
                }
            });
            join_handlers.push(join_handler);
        }

        let _ = try_join_all(join_handlers).await?;

        Ok(())
    }

    // /// PR 폴링 루프
    // async fn pr_polling_loop(pool: PgPool, interval_duration: Duration) -> Result<()> {
    //     let mut ticker = interval(interval_duration);

    //     loop {
    //         ticker.tick().await;
    //         info!("PR polling cycle...");

    //         // TODO: PR 폴링 로직

    //         info!("PR polling completed");
    //     }
    // }

    // /// Refs 동기화 루프
    // async fn refs_sync_loop(
    //     pool: PgPool,
    //     interval_duration: Duration,
    //     github_token: Option<String>,
    // ) -> Result<()> {
    //     let mut ticker = interval(interval_duration);

    //     loop {
    //         ticker.tick().await;
    //         info!("Refs sync cycle...");

    //         let repositories = db::Queries::get_repositories(&pool).await?;
    //         for repository in repositories {
    //             info!("Repository: {:?}", repository);

    //             let url = if let Some(token) = github_token.clone() {
    //                 format!(
    //                     "https://{}@github.com/{}/{}.git",
    //                     token, repository.owner, repository.name
    //                 )
    //             } else {
    //                 format!(
    //                     "https://github.com/{}/{}.git",
    //                     repository.owner, repository.name
    //                 )
    //             };

    //             let refs = git::Client::ls_remote(&url).await;
    //             match refs {
    //                 Ok(refs) => {
    //                     info!("Refs: {:?}", refs);
    //                 }
    //                 Err(e) => {
    //                     error!("Error: {:?}", e);
    //                 }
    //             }
    //         }

    //         info!("Refs sync completed");
    //     }
    // }
}

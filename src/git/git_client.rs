use crate::{
    config::Config,
    db::{self, Branch},
};
use anyhow::Result;
use git2::{BranchType, Cred, FetchOptions, RemoteCallbacks, Repository};
use std::path::Path;
use tracing::info;

#[derive(Debug, Clone)]
pub struct GitClient {
    pub config: Config,
}

impl GitClient {
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    pub async fn clone_repository(&self, url: &str, path: &str) -> Result<Repository> {
        let url = url.to_string();
        let path = path.to_string();
        let config = self.config.clone();

        tokio::task::spawn_blocking(move || {
            let mut callbacks = RemoteCallbacks::new();

            callbacks.credentials(|_url, username_from_url, _allowed_types| {
                let username = username_from_url.unwrap_or("git");

                if config.git_use_ssh_agent {
                    // SSH 키 인증
                    if let Some(key_path) = &config.git_ssh_key_path {
                        Cred::ssh_key(
                            username,
                            None,
                            key_path.as_path(),
                            config.git_ssh_key_passphrase.as_deref(),
                        )
                    } else {
                        Cred::ssh_key_from_agent(username)
                    }
                } else {
                    // GitHub 토큰 (HTTPS)
                    if let Some(token) = &config.github_token {
                        Cred::userpass_plaintext("git", token)
                    } else {
                        Cred::default()
                    }
                }
            });

            let mut fo = FetchOptions::new();
            fo.remote_callbacks(callbacks);

            let mut builder = git2::build::RepoBuilder::new();
            builder.fetch_options(fo);

            builder.clone(&url, Path::new(&path))
        })
        .await?
        .map_err(|e| anyhow::Error::new(e))
    }

    pub async fn get_branches(&self, path: &str) -> Result<Vec<(String, String)>> {
        let repository = Repository::open(Path::new(path))?;
        let git2_branches: git2::Branches = repository.branches(Some(BranchType::Remote)).unwrap();

        let mut branches: Vec<(String, String)> = Vec::new();
        for branch in git2_branches {
            let branch = branch.unwrap();
            if branch.0.is_head() {
                continue;
            }

            let (br, _branch_type) = branch;
            let name = br.name().unwrap().unwrap();

            if name.is_empty() {
                continue;
            }

            if name == "origin/HEAD" {
                continue;
            }

            let mut head_sha = String::new();
            if let Some(commit) = br.get().peel_to_commit().ok() {
                head_sha = commit.id().to_string();
            } else {
                continue;
            }

            branches.push((name.to_string(), head_sha));
        }
        Ok(branches)
    }
}

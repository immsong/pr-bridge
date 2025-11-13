use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct Repository {
    pub id: i32,
    pub owner: String,
    pub name: String,
    pub poll_interval_seconds: Option<i32>,
    pub is_active: bool,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct RepositoryPollingHistory {
    pub id: i32,
    pub repository_id: i32,
    pub polled_at: NaiveDateTime,
    pub created_at: NaiveDateTime,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, PartialEq)]
pub struct Branch {
    pub id: i32,
    pub repository_id: i32,
    pub name: String,
    pub head_sha: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct Tag {
    pub id: i32,
    pub repository_id: i32,
    pub name: String,
    pub commit_sha: String,
    pub created_at: NaiveDateTime,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct PullRequest {
    pub id: i32,
    pub repository_id: i32,
    pub pr_number: i32,
    pub title: String,
    pub author: Option<String>,
    pub source_branch_id: i32,
    pub target_branch_id: i32,
    pub head_sha: String,
    pub status: String,
    pub last_polled_at: Option<NaiveDateTime>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct BuildTrigger {
    pub id: i32,
    pub pull_request_id: i32,
    pub commit_sha: String,
    pub jenkins_build_number: Option<i32>,
    pub jenkins_build_url: Option<String>,
    pub trigger_status: String,
    pub trigger_message: Option<String>,
    pub triggered_at: NaiveDateTime,
    pub completed_at: Option<NaiveDateTime>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct JenkinsMapping {
    pub id: i32,
    pub repository_id: i32,
    pub jenkins_job_name: String,
    pub jenkins_url: Option<String>,
    pub auto_trigger: bool,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct SystemSetting {
    pub key: String,
    pub value: String,
    pub description: Option<String>,
    pub updated_at: NaiveDateTime,
}

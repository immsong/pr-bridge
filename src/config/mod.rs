use serde::Deserialize;
use anyhow::Result;
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    // Server
    #[serde(default = "default_host")]
    pub server_host: String,
    
    #[serde(default = "default_port")]
    pub server_port: u16,
    
    // Database
    pub database_url: String,
    
    // GitHub
    pub github_token: Option<String>,
    
    // Git SSH (선택적)
    #[serde(default)]
    pub git_ssh_key_path: Option<PathBuf>,
    
    #[serde(default)]
    pub git_ssh_key_passphrase: Option<String>,
    
    #[serde(default = "default_use_ssh_agent")]
    pub git_use_ssh_agent: bool,
    
    // Jenkins
    pub jenkins_url: String,
    pub jenkins_user: String,
    pub jenkins_token: String,
    
    // Polling
    #[serde(default = "default_poll_interval")]
    pub poll_interval_seconds: u64,
}

fn default_host() -> String {
    "0.0.0.0".to_string()
}

fn default_port() -> u16 {
    8080
}

fn default_poll_interval() -> u64 {
    60
}

fn default_use_ssh_agent() -> bool {
    false
}

impl Config {
    pub fn from_env() -> Result<Self> {
        dotenv::dotenv().ok();
        
        let config = envy::from_env::<Config>()?;
        // use ssh 옵션이 false 인데 token 이 있으면 에러 발생
        if !config.git_use_ssh_agent && config.github_token.is_none() {
            return Err(anyhow::anyhow!("GitHub token is required when use ssh agent is false"));
        }
        // use ssh 옵션이 true 인데 ssh key path 가 없으면 에러 발생
        if config.git_use_ssh_agent && config.git_ssh_key_path.is_none() {
            return Err(anyhow::anyhow!("SSH key path is required when use ssh agent is true"));
        }
        
        Ok(config)
    }
    
    pub fn server_addr(&self) -> String {
        format!("{}:{}", self.server_host, self.server_port)
    }
}
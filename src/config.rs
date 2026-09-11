use anyhow::{Context, Result, anyhow};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildConfig {
    pub api_url: String,
    pub nickname: String,
    pub token: Option<String>,
}

impl BuildConfig {
    pub fn from_env() -> Result<Self> {
        let api_url = std::env::var("CARGO_LEADERBOARD_API_URL")
            .context("CARGO_LEADERBOARD_API_URL is not set")?;
        let nickname = std::env::var("CARGO_LEADERBOARD_NICKNAME")
            .context("CARGO_LEADERBOARD_NICKNAME is not set")?;
        let token = std::env::var("CARGO_LEADERBOARD_TOKEN").ok();

        if nickname.trim().is_empty() {
            return Err(anyhow!("CARGO_LEADERBOARD_NICKNAME must not be empty"));
        }

        Ok(Self {
            api_url,
            nickname,
            token,
        })
    }
}

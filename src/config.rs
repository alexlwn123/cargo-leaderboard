use std::io::{IsTerminal, Write};
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

pub const DEFAULT_API_URL: &str = "https://cargo-leaderboard.vercel.app";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildConfig {
    pub api_url: String,
    pub nickname: String,
    pub token: Option<String>,
}

#[derive(Default, Deserialize, Serialize)]
struct SavedConfig {
    nickname: Option<String>,
    api_url: Option<String>,
}

fn config_path() -> Result<PathBuf> {
    let directory = if let Some(path) = std::env::var_os("CARGO_LEADERBOARD_CONFIG_DIR") {
        PathBuf::from(path)
    } else if cfg!(windows) {
        PathBuf::from(
            std::env::var_os("APPDATA")
                .context("APPDATA is not set; set CARGO_LEADERBOARD_CONFIG_DIR")?,
        )
        .join("cargo-leaderboard")
    } else {
        let base = match std::env::var_os("XDG_CONFIG_HOME") {
            Some(path) if PathBuf::from(&path).is_absolute() => PathBuf::from(path),
            _ => PathBuf::from(
                std::env::var_os("HOME")
                    .context("HOME is not set; set CARGO_LEADERBOARD_CONFIG_DIR")?,
            )
            .join(".config"),
        };
        base.join("cargo-leaderboard")
    };
    if directory.as_os_str().is_empty() {
        bail!("CARGO_LEADERBOARD_CONFIG_DIR must not be empty");
    }
    Ok(directory.join("config.json"))
}

fn read_saved() -> Result<SavedConfig> {
    let path = config_path()?;
    match std::fs::read(&path) {
        Ok(data) => serde_json::from_slice(&data)
            .with_context(|| format!("Invalid configuration at {}. Run cargo leaderboard setup --nickname YOUR_NAME to replace it", path.display())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(SavedConfig::default()),
        Err(error) => Err(error).with_context(|| format!("Cannot read {}", path.display())),
    }
}

fn validate_nickname(nickname: &str) -> Result<()> {
    if nickname.trim().is_empty()
        || nickname.chars().count() > 40
        || nickname.chars().any(char::is_control)
    {
        bail!("Nickname must contain 1–40 characters and no control characters");
    }
    Ok(())
}

fn validate_url(value: &str) -> Result<String> {
    let url = url::Url::parse(value).context("Server URL must be an absolute HTTP(S) URL")?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        bail!("Server URL must be HTTP(S), without credentials, query parameters, or a fragment");
    }
    Ok(url.as_str().trim_end_matches('/').to_owned())
}

impl BuildConfig {
    pub fn from_env() -> Result<Self> {
        let env_nickname = std::env::var("CARGO_LEADERBOARD_NICKNAME").ok();
        let env_url = std::env::var("CARGO_LEADERBOARD_API_URL").ok();
        let saved = if env_nickname.is_some() && env_url.is_some() {
            SavedConfig::default()
        } else {
            read_saved()?
        };
        let nickname = env_nickname.or(saved.nickname).context(
            "Choose a public nickname first: cargo leaderboard setup\nFor scripts: cargo leaderboard setup --nickname YOUR_NAME\nTo measure without publishing: cargo leaderboard build --no-submit",
        )?;
        validate_nickname(&nickname)?;
        let api_url = validate_url(
            &env_url
                .or(saved.api_url)
                .unwrap_or_else(|| DEFAULT_API_URL.to_owned()),
        )?;
        Ok(Self {
            api_url,
            nickname,
            token: std::env::var("CARGO_LEADERBOARD_TOKEN").ok(),
        })
    }
}

pub fn setup(nickname: Option<String>, api_url: Option<String>) -> Result<()> {
    println!(
        "Your nickname, project label, measurements, platform and tool versions will be public.\nSource code and local paths stay on your machine. Use --repo to replace a private project label, or --no-submit to keep a run local.\n"
    );
    let nickname = match nickname {
        Some(value) => value,
        None => {
            if !std::io::stdin().is_terminal() {
                bail!("Noninteractive setup requires --nickname YOUR_NAME");
            }
            print!("Public nickname: ");
            std::io::stdout().flush()?;
            let mut value = String::new();
            std::io::stdin().read_line(&mut value)?;
            value.trim().to_owned()
        }
    };
    validate_nickname(&nickname)?;
    // Setup deliberately replaces broken configuration and defaults to the public server.
    let api_url = validate_url(api_url.as_deref().unwrap_or(DEFAULT_API_URL))?;
    let path = config_path()?;
    let directory = path
        .parent()
        .context("Configuration directory is missing")?;
    std::fs::create_dir_all(directory)
        .with_context(|| format!("Cannot create {}", directory.display()))?;
    let mut file = tempfile::NamedTempFile::new_in(directory)?;
    serde_json::to_writer_pretty(
        &mut file,
        &SavedConfig {
            nickname: Some(nickname.clone()),
            api_url: Some(api_url.clone()),
        },
    )?;
    file.write_all(b"\n")?;
    file.as_file().sync_all()?;
    file.persist(&path)
        .with_context(|| format!("Cannot save {}", path.display()))?;
    println!(
        "Saved nickname: {nickname}\nServer: {api_url}\nConfiguration: {}",
        path.display()
    );
    if ["CARGO_LEADERBOARD_NICKNAME", "CARGO_LEADERBOARD_API_URL"]
        .iter()
        .any(|name| std::env::var_os(name).is_some())
    {
        println!(
            "Note: CARGO_LEADERBOARD_NICKNAME and CARGO_LEADERBOARD_API_URL environment variables override saved settings. Unset them to use this configuration."
        );
    }
    println!(
        "\nReady. Inside a Rust project, run: cargo leaderboard build\nCheck your connection: cargo leaderboard doctor"
    );
    Ok(())
}

pub async fn doctor() -> Result<()> {
    let config = BuildConfig::from_env()?;
    println!(
        "Nickname: {}\nServer: {}\nConfiguration: {}",
        config.nickname,
        config.api_url,
        config_path()?.display()
    );
    let cargo = tokio::process::Command::new("cargo").arg("--version").output().await
        .context("Cargo is missing from PATH. Install Rust from https://rustup.rs, then reopen your terminal")?;
    if !cargo.status.success() {
        bail!(
            "Cargo is installed but its toolchain is unavailable. Run rustup show to diagnose it"
        );
    }
    println!("{}", String::from_utf8_lossy(&cargo.stdout).trim());
    let response = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()?
        .get(format!(
            "{}/v1/leaderboard?metric=largest_build&limit=1",
            config.api_url
        ))
        .send()
        .await
        .context("Cannot reach the leaderboard. Check your connection and server URL")?
        .error_for_status()
        .context("Leaderboard returned an error")?;
    let body: serde_json::Value = response
        .json()
        .await
        .context("Server did not return a leaderboard JSON response")?;
    if !body.get("entries").is_some_and(serde_json::Value::is_array) {
        bail!("Server response is not a compatible leaderboard (missing entries)");
    }
    println!(
        "Server connection OK. No measurements were submitted. Submission credentials and rate limits are checked when you submit a run."
    );
    Ok(())
}

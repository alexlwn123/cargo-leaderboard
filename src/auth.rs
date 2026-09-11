use crate::config::{BuildConfig, DEFAULT_API_URL, SavedConfig, read_saved, save, validate_url};
use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::time::{Duration, Instant};

fn client() -> Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::none())
        .build()?)
}
#[derive(Deserialize)]
pub(crate) struct Account {
    pub github_login: String,
}
#[derive(Deserialize)]
struct Me {
    user: Account,
}
pub(crate) async fn account(config: &BuildConfig) -> Result<Account> {
    let response = client()?
        .get(format!("{}/auth/me", config.api_url))
        .bearer_auth(
            config
                .token
                .as_deref()
                .context("Run cargo leaderboard login")?,
        )
        .send()
        .await?
        .error_for_status()
        .context("GitHub login expired or was revoked. Run cargo leaderboard login")?;
    Ok(response.json::<Me>().await?.user)
}
#[derive(Deserialize)]
struct Device {
    device_code: String,
    user_code: String,
    verification_uri_complete: String,
    expires_in: u64,
    interval: u64,
}
#[derive(Deserialize)]
struct Credential {
    token: String,
    github_login: String,
}

pub async fn login(api_url: Option<String>, no_browser: bool) -> Result<()> {
    let saved = read_saved()?;
    let api_url = validate_url(
        &api_url
            .or_else(|| std::env::var("CARGO_LEADERBOARD_API_URL").ok())
            .or(saved.api_url)
            .unwrap_or_else(|| DEFAULT_API_URL.to_owned()),
    )?;
    let server = url::Url::parse(&api_url)?;
    if server.scheme() != "https"
        && !matches!(server.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"))
    {
        bail!("Login requires HTTPS (HTTP is allowed only for loopback development)");
    }
    let client = client()?;
    let response = client
        .post(format!("{api_url}/auth/device-start"))
        .json(&serde_json::json!({}))
        .send()
        .await?
        .error_for_status()
        .context("Cannot start GitHub login. Check the server URL and try again")?;
    let device: Device = response
        .json()
        .await
        .context("Server does not support CLI login; update the server or check its URL")?;
    let approval = url::Url::parse(&device.verification_uri_complete)?;
    if approval.origin() != server.origin()
        || !approval.username().is_empty()
        || approval.password().is_some()
    {
        bail!("Server returned an approval URL on a different origin; refusing to open it");
    }
    if device.user_code.len() > 32 || device.user_code.chars().any(char::is_control) {
        bail!("Server returned an invalid approval code");
    }
    println!(
        "Connect GitHub at: {}\nConfirmation code: {}\nApprove only if this code matches the browser. Your GitHub account and submitted measurements will be public.\nWaiting for browser approval…",
        approval, device.user_code
    );
    if !no_browser {
        open_browser(approval.as_str());
    }
    let deadline = Instant::now() + Duration::from_secs(device.expires_in.clamp(1, 600));
    let mut interval = device.interval.clamp(5, 60);
    while Instant::now() < deadline {
        tokio::time::sleep(Duration::from_secs(interval)).await;
        let response = client
            .post(format!("{api_url}/auth/device-poll"))
            .json(&serde_json::json!({"device_code": device.device_code}))
            .send()
            .await?;
        match response.status().as_u16() {
            202 => continue,
            429 => {
                interval = (interval + 5).min(60);
                continue;
            }
            200 => {
                let credential: Credential = response.json().await?;
                if !credential.token.starts_with("clb_cli_")
                    || credential.token.len() > 256
                    || credential.token.chars().any(char::is_control)
                    || credential.github_login.is_empty()
                    || credential.github_login.len() > 39
                    || !credential
                        .github_login
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '-')
                {
                    bail!("Server returned an invalid credential");
                }
                save(&SavedConfig {
                    api_url: Some(api_url.clone()),
                    nickname: Some(credential.github_login.clone()),
                    github_login: Some(credential.github_login.clone()),
                    token: Some(credential.token),
                })?;
                println!(
                    "Logged in as @{} on {api_url}.\nReady: cargo leaderboard build",
                    credential.github_login
                );
                if std::env::var_os("CARGO_LEADERBOARD_TOKEN").is_some()
                    || std::env::var("CARGO_LEADERBOARD_API_URL")
                        .is_ok_and(|url| url.trim_end_matches('/') != api_url)
                {
                    println!(
                        "Note: existing CARGO_LEADERBOARD_TOKEN or CARGO_LEADERBOARD_API_URL overrides may replace this login. Unset them to use the saved account."
                    );
                }
                return Ok(());
            }
            _ => bail!(
                "Login expired, was rejected, or the server is unavailable (HTTP {}). Run cargo leaderboard login again",
                response.status()
            ),
        }
    }
    bail!("Login expired waiting for browser approval. Run cargo leaderboard login again")
}
fn open_browser(url: &str) {
    #[cfg(target_os = "macos")]
    let result = std::process::Command::new("open").arg(url).spawn();
    #[cfg(target_os = "windows")]
    let result = std::process::Command::new("rundll32.exe")
        .args(["url.dll,FileProtocolHandler", url])
        .spawn();
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let result = std::process::Command::new("xdg-open").arg(url).spawn();
    if result.is_err() {
        println!("Open the URL above in your browser to continue.");
    }
}
pub async fn logout() -> Result<()> {
    let mut saved = read_saved()?;
    let (Some(token), Some(api_url)) = (&saved.token, &saved.api_url) else {
        println!("No saved CLI login. Environment credentials, if any, must be unset separately.");
        return Ok(());
    };
    let api_url = validate_url(api_url)?;
    let response = client()?
        .post(format!("{api_url}/auth/cli-logout"))
        .bearer_auth(token)
        .send()
        .await
        .context("Could not revoke access. Retry, or revoke CLI access on the website")?;
    if !response.status().is_success() && response.status().as_u16() != 401 {
        bail!(
            "Could not revoke access (HTTP {}). Retry, or revoke CLI access on the website",
            response.status()
        );
    }
    saved.token = None;
    saved.github_login = None;
    save(&saved)?;
    println!(
        "CLI access revoked and saved credential removed. Environment credentials, if any, must be unset separately."
    );
    Ok(())
}

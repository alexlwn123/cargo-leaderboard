use std::process::Stdio;
use std::time::Instant;

use anyhow::{Context, Result};
use chrono::Utc;
use reqwest::Client;
use tokio::process::Command;
use uuid::Uuid;

use crate::config::BuildConfig;
use crate::measurement::{measure, project};
use crate::repo::resolve_repo_slug;
use crate::types::BuildEvent;

pub async fn run_command(
    name: &str,
    cargo_args: Vec<String>,
    no_submit: bool,
    repo: Option<String>,
) -> Result<u8> {
    // Help and clean dry-runs must never appear as measured work.
    if cargo_args
        .iter()
        .any(|arg| ["--help", "-h", "--dry-run"].contains(&arg.as_str()))
    {
        return Ok(exit_code(spawn_cargo(name, &cargo_args).await?));
    }
    let config = if no_submit {
        None
    } else {
        Some(BuildConfig::from_env()?)
    };
    let project = project(&cargo_args).await;
    if let Err(error) = &project {
        eprintln!("warning: measurement unavailable: {error:#}");
    }
    let before = project
        .as_ref()
        .ok()
        .map(|project| measure(&project.roots))
        .transpose();
    if let Err(error) = &before {
        eprintln!("warning: measurement unavailable: {error:#}");
    }
    let repo_slug = repo.unwrap_or_else(|| {
        project
            .as_ref()
            .map(|p| resolve_repo_slug(&p.workspace))
            .unwrap_or_else(|_| "unknown".into())
    });
    let cargo_version = cargo_version().await.unwrap_or_else(|_| "unknown".into());
    let started_at = Utc::now();
    let started = Instant::now();
    let status = spawn_cargo(name, &cargo_args).await?;
    let duration_ms = started.elapsed().as_millis() as i64;
    let finished_at = Utc::now();
    if status.success()
        && let (Ok(project), Ok(Some(before))) = (project, before)
    {
        match measure(&project.roots) {
            Ok(after) => {
                let bytes = if name == "clean" {
                    (before.bytes - after.bytes).max(0)
                } else {
                    after.bytes
                };
                let files = if name == "clean" {
                    (before.files - after.files).max(0)
                } else {
                    after.files
                };
                let event = BuildEvent {
                    event_id: Uuid::new_v4(),
                    nickname: config
                        .as_ref()
                        .map(|c| c.nickname.clone())
                        .unwrap_or_else(|| "local".into()),
                    repo_slug,
                    command: name.into(),
                    started_at,
                    finished_at,
                    duration_ms,
                    success: true,
                    cargo_version,
                    client_version: env!("CARGO_PKG_VERSION").into(),
                    bytes,
                    bytes_before: before.bytes,
                    bytes_after: after.bytes,
                    file_count: files,
                    profile: profile(&cargo_args, name),
                    platform: format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH),
                };
                eprintln!(
                    "leaderboard: {:.2} GiB {} ({files} files), {:.2}s",
                    bytes as f64 / 1_073_741_824.0,
                    if name == "clean" {
                        "reclaimed"
                    } else {
                        "total footprint"
                    },
                    duration_ms as f64 / 1000.0
                );
                if let Some(config) = config {
                    if let Err(error) = event.validate().map_err(anyhow::Error::msg) {
                        eprintln!("warning: event not submitted: {error}");
                    } else if let Err(error) = report_event(&config, &event).await {
                        eprintln!("warning: failed to report event: {error:#}");
                    } else {
                        eprintln!("leaderboard: submitted to {}", config.api_url);
                    }
                }
            }
            Err(error) => eprintln!("warning: event not submitted: {error:#}"),
        }
    }
    Ok(exit_code(status))
}

fn profile(args: &[String], command: &str) -> String {
    for (i, arg) in args.iter().enumerate() {
        if let Some(value) = arg.strip_prefix("--profile=") {
            return value.into();
        }
        if arg == "--profile" {
            return args.get(i + 1).cloned().unwrap_or_else(|| "unknown".into());
        }
    }
    if args.iter().any(|a| a == "--release" || a == "-r") {
        "release".into()
    } else if command == "clean" {
        "all".into()
    } else {
        "debug".into()
    }
}

async fn spawn_cargo(name: &str, args: &[String]) -> Result<std::process::ExitStatus> {
    Command::new("cargo")
        .arg(name)
        .args(args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .await
        .context("failed to execute cargo")
}

async fn cargo_version() -> Result<String> {
    let output = Command::new("cargo")
        .arg("--version")
        .output()
        .await
        .context("failed to determine cargo version")?;

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

async fn report_event(config: &BuildConfig, event: &BuildEvent) -> Result<()> {
    let client = Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .context("failed to construct HTTP client")?;

    let endpoint = format!("{}/v1/build-events", config.api_url.trim_end_matches('/'));
    let mut request = client.post(endpoint).json(event);

    if let Some(token) = &config.token {
        request = request.bearer_auth(token);
    }

    let response = request.send().await.context("request failed")?;
    if response.status().as_u16() == 401 {
        anyhow::bail!("GitHub login expired or is missing. Run cargo leaderboard login");
    }
    if response.status().as_u16() == 429 {
        anyhow::bail!("Submission rate limit reached. Try again next hour");
    }
    response
        .error_for_status()
        .context("server rejected build event")?;

    Ok(())
}

fn exit_code(status: std::process::ExitStatus) -> u8 {
    if let Some(code) = status.code() {
        return code as u8;
    }

    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        status
            .signal()
            .map(|signal| (128 + signal) as u8)
            .unwrap_or(1)
    }

    #[cfg(not(unix))]
    {
        1
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use chrono::Utc;

    use crate::repo::resolve_repo_slug;
    use crate::types::BuildEvent;

    #[test]
    fn resolves_repo_slug_from_directory_name_when_no_project_exists() {
        let path = PathBuf::from("/tmp/example-project");
        assert_eq!(resolve_repo_slug(&path), "example-project");
    }

    #[test]
    fn build_event_serializes_expected_shape() {
        let event = BuildEvent {
            event_id: uuid::Uuid::nil(),
            nickname: "alex".to_string(),
            repo_slug: "acme/widget".to_string(),
            command: "build".to_string(),
            started_at: Utc::now(),
            finished_at: Utc::now(),
            duration_ms: 42,
            success: true,
            cargo_version: "cargo 1.0.0".to_string(),
            client_version: "0.1.0".to_string(),
            bytes: 0,
            bytes_before: 0,
            bytes_after: 0,
            file_count: 0,
            profile: "debug".into(),
            platform: "test".into(),
        };

        let value = serde_json::to_value(event).expect("event to serialize");
        assert_eq!(value["command"], "build");
        assert_eq!(value["success"], true);
    }
}

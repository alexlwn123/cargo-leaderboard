use std::fs;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use anyhow::Result;
use assert_cmd::Command;
use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::post;
use axum::{Json, Router};
use cargo_leaderboard::types::BuildEvent;
use tempfile::TempDir;

#[tokio::test(flavor = "multi_thread")]
async fn successful_build_reports_one_event() -> Result<()> {
    let events = Arc::new(Mutex::new(Vec::<BuildEvent>::new()));
    let addr = spawn_capture_server(events.clone()).await?;
    let fixture = fixture_manifest("success-project");

    Command::cargo_bin("cargo-leaderboard")?
        .env("CARGO_LEADERBOARD_API_URL", format!("http://{addr}"))
        .env("CARGO_LEADERBOARD_NICKNAME", "alex")
        .arg("build")
        .arg("--manifest-path")
        .arg(fixture)
        .assert()
        .success();

    let captured = events.lock().expect("events lock");
    assert_eq!(captured.len(), 1);
    assert_eq!(captured[0].nickname, "alex");
    assert_eq!(captured[0].command, "build");
    assert!(captured[0].duration_ms >= 0);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn failed_build_does_not_report_event() -> Result<()> {
    let events = Arc::new(Mutex::new(Vec::<BuildEvent>::new()));
    let addr = spawn_capture_server(events.clone()).await?;
    let fixture = fixture_manifest("failing-project");

    Command::cargo_bin("cargo-leaderboard")?
        .env("CARGO_LEADERBOARD_API_URL", format!("http://{addr}"))
        .env("CARGO_LEADERBOARD_NICKNAME", "alex")
        .arg("build")
        .arg("--manifest-path")
        .arg(fixture)
        .assert()
        .failure();

    assert!(events.lock().expect("events lock").is_empty());
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn reporting_failure_does_not_fail_build() -> Result<()> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    drop(listener);
    let fixture = fixture_manifest("success-project");

    Command::cargo_bin("cargo-leaderboard")?
        .env("CARGO_LEADERBOARD_API_URL", format!("http://{addr}"))
        .env("CARGO_LEADERBOARD_NICKNAME", "alex")
        .arg("build")
        .arg("--manifest-path")
        .arg(fixture)
        .assert()
        .success();

    Ok(())
}

#[test]
fn repo_slug_falls_back_to_workspace_name_without_git_remote() -> Result<()> {
    let dir = TempDir::new()?;
    fs::write(
        dir.path().join("Cargo.toml"),
        "[package]\nname = \"fallback-name\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )?;
    fs::create_dir(dir.path().join("src"))?;
    fs::write(dir.path().join("src/main.rs"), "fn main() {}\n")?;

    let slug = cargo_leaderboard::repo::resolve_repo_slug(dir.path());
    assert_eq!(slug, "fallback-name");
    Ok(())
}

async fn spawn_capture_server(events: Arc<Mutex<Vec<BuildEvent>>>) -> Result<SocketAddr> {
    let app = Router::new()
        .route(
            "/v1/build-events",
            post(
                |State(events): State<Arc<Mutex<Vec<BuildEvent>>>>,
                 Json(event): Json<BuildEvent>| async move {
                    events.lock().expect("events lock").push(event);
                    StatusCode::CREATED
                },
            ),
        )
        .with_state(events);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    tokio::spawn(async move {
        axum::serve(listener, app).await.expect("server to run");
    });
    Ok(addr)
}

fn fixture_manifest(name: &str) -> String {
    format!(
        "{}/tests/fixtures/{name}/Cargo.toml",
        env!("CARGO_MANIFEST_DIR")
    )
}

#[tokio::test(flavor = "multi_thread")]
async fn build_then_clean_measures_custom_directories_and_cargo_invocation() -> Result<()> {
    let events = Arc::new(Mutex::new(Vec::<BuildEvent>::new()));
    let addr = spawn_capture_server(events.clone()).await?;
    let temp = TempDir::new()?;
    let target = temp.path().join("artifacts with spaces");
    let intermediate = temp.path().join("intermediate");
    let config = format!(
        "build.build-dir={}",
        serde_json::to_string(&intermediate.to_string_lossy())?
    );
    let binary = assert_cmd::cargo::cargo_bin!("cargo-leaderboard");
    let path = std::env::join_paths(std::iter::once(binary.parent().unwrap().to_owned()).chain(
        std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()),
    ))?;
    for action in ["build", "clean"] {
        Command::new("cargo")
            .env("PATH", &path)
            .env("CARGO_LEADERBOARD_API_URL", format!("http://{addr}"))
            .env("CARGO_LEADERBOARD_NICKNAME", "tester")
            .args([
                "leaderboard",
                action,
                "--repo",
                "test/custom-dirs",
                "--",
                "--manifest-path",
            ])
            .arg(fixture_manifest("success-project"))
            .arg("--target-dir")
            .arg(&target)
            .arg("--config")
            .arg(&config)
            .assert()
            .success();
    }
    let captured = events.lock().unwrap();
    assert_eq!(captured.len(), 2);
    let (build, clean) = (&captured[0], &captured[1]);
    assert!(build.bytes > 0);
    assert_eq!(build.bytes, build.bytes_after);
    assert_eq!(build.bytes_before, 0);
    assert!(build.file_count > 0);
    assert_eq!(clean.command, "clean");
    assert_eq!(clean.bytes, build.bytes_after);
    assert_eq!(clean.bytes_after, 0);
    assert_eq!(clean.file_count, build.file_count);
    assert_eq!(build.repo_slug, "test/custom-dirs");
    Ok(())
}

#[test]
fn no_submit_works_without_configuration_and_help_is_forwarded() -> Result<()> {
    let temp = TempDir::new()?;
    Command::cargo_bin("cargo-leaderboard")?
        .env_remove("CARGO_LEADERBOARD_API_URL")
        .env_remove("CARGO_LEADERBOARD_NICKNAME")
        .args(["build", "--no-submit", "--", "--manifest-path"])
        .arg(fixture_manifest("success-project"))
        .arg("--target-dir")
        .arg(temp.path().join("target"))
        .assert()
        .success();
    Command::cargo_bin("cargo-leaderboard")?
        .env_remove("CARGO_LEADERBOARD_NICKNAME")
        .args(["clean", "--", "--help"])
        .assert()
        .success();
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn manifest_path_resolves_project_label_and_dry_run_does_not_report() -> Result<()> {
    let events = Arc::new(Mutex::new(Vec::<BuildEvent>::new()));
    let addr = spawn_capture_server(events.clone()).await?;
    let temp = TempDir::new()?;
    fs::create_dir(temp.path().join("src"))?;
    fs::write(
        temp.path().join("Cargo.toml"),
        "[package]\nname = \"separate-project\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )?;
    fs::write(temp.path().join("src/main.rs"), "fn main() {}")?;
    for action in ["build", "clean"] {
        let mut command = Command::cargo_bin("cargo-leaderboard")?;
        command
            .env("CARGO_LEADERBOARD_API_URL", format!("http://{addr}"))
            .env("CARGO_LEADERBOARD_NICKNAME", "tester")
            .arg(action)
            .arg("--manifest-path")
            .arg(temp.path().join("Cargo.toml"));
        if action == "clean" {
            command.arg("--dry-run");
        }
        command.assert().success();
    }
    let captured = events.lock().unwrap();
    assert_eq!(captured.len(), 1);
    assert_eq!(captured[0].repo_slug, "separate-project");
    assert!(
        temp.path()
            .join(format!(
                "target/debug/separate-project{}",
                std::env::consts::EXE_SUFFIX
            ))
            .exists()
    );
    Ok(())
}

fn isolated_cli(dir: &std::path::Path) -> Result<Command> {
    let mut command = Command::cargo_bin("cargo-leaderboard")?;
    command
        .env("CARGO_LEADERBOARD_CONFIG_DIR", dir)
        .env_remove("CARGO_LEADERBOARD_NICKNAME")
        .env_remove("CARGO_LEADERBOARD_API_URL")
        .env_remove("CARGO_LEADERBOARD_TOKEN");
    Ok(command)
}

#[test]
fn setup_saves_defaults_updates_atomically_and_rejects_invalid_values() -> Result<()> {
    let dir = TempDir::new()?;
    let path = dir.path().join("config.json");
    isolated_cli(dir.path())?
        .args(["setup", "--nickname", "first"])
        .assert()
        .success();
    let saved: serde_json::Value = serde_json::from_slice(&fs::read(&path)?)?;
    assert_eq!(saved["nickname"], "first");
    assert_eq!(saved["api_url"], "https://cargo-leaderboard.vercel.app");
    isolated_cli(dir.path())?
        .args([
            "setup",
            "--nickname",
            "second",
            "--api-url",
            "http://localhost:3000/",
        ])
        .assert()
        .success();
    let previous = fs::read(&path)?;
    for args in [
        vec!["setup", "--nickname", ""],
        vec!["setup", "--nickname", "valid", "--api-url", "file:///tmp"],
        vec![
            "setup",
            "--nickname",
            "valid",
            "--api-url",
            "https://user:secret@example.com",
        ],
    ] {
        isolated_cli(dir.path())?.args(args).assert().failure();
        assert_eq!(fs::read(&path)?, previous);
    }
    // An explicit setup repairs a corrupt file without needing manual edits.
    fs::write(&path, "broken")?;
    isolated_cli(dir.path())?
        .args(["setup", "--nickname", "repaired"])
        .assert()
        .success();
    let saved: serde_json::Value = serde_json::from_slice(&fs::read(path)?)?;
    assert_eq!(saved["nickname"], "repaired");
    assert!(saved.get("token").is_none());
    Ok(())
}

#[test]
fn first_run_and_noninteractive_setup_explain_next_steps() -> Result<()> {
    let dir = TempDir::new()?;
    let output = isolated_cli(dir.path())?.arg("build").output()?;
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("cargo leaderboard setup"));
    let output = isolated_cli(dir.path())?
        .arg("setup")
        .write_stdin("")
        .output()?;
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("--nickname"));
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn saved_setup_is_used_across_processes_and_environment_overrides_it() -> Result<()> {
    let events = Arc::new(Mutex::new(Vec::<BuildEvent>::new()));
    let addr = spawn_capture_server(events.clone()).await?;
    let dir = TempDir::new()?;
    isolated_cli(dir.path())?
        .args([
            "setup",
            "--nickname",
            "saved-name",
            "--api-url",
            &format!("http://{addr}"),
        ])
        .assert()
        .success();
    for override_name in [None, Some("env-name")] {
        let mut command = isolated_cli(dir.path())?;
        if let Some(name) = override_name {
            command.env("CARGO_LEADERBOARD_NICKNAME", name);
        }
        command
            .args(["build", "--manifest-path"])
            .arg(fixture_manifest("success-project"))
            .arg("--target-dir")
            .arg(dir.path().join("target"))
            .assert()
            .success();
    }
    let captured = events.lock().unwrap();
    assert_eq!(captured.len(), 2);
    assert_eq!(captured[0].nickname, "saved-name");
    assert_eq!(captured[1].nickname, "env-name");
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn doctor_checks_response_shape_and_never_submits() -> Result<()> {
    let app = Router::new().route(
        "/v1/leaderboard",
        axum::routing::get(|| async { Json(serde_json::json!({"entries": []})) }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let dir = TempDir::new()?;
    isolated_cli(dir.path())?
        .args([
            "setup",
            "--nickname",
            "tester",
            "--api-url",
            &format!("http://{addr}"),
        ])
        .assert()
        .success();
    let output = isolated_cli(dir.path())?.arg("doctor").output()?;
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("Server connection OK"));
    isolated_cli(dir.path())?
        .env(
            "CARGO_LEADERBOARD_API_URL",
            format!("http://{addr}/invalid"),
        )
        .arg("doctor")
        .assert()
        .failure();
    Ok(())
}

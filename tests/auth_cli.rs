use anyhow::Result;
use assert_cmd::cargo::cargo_bin_cmd;
use axum::{
    Json, Router,
    extract::State,
    http::HeaderMap,
    routing::{get, post},
};
use serde_json::json;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use tempfile::TempDir;

fn cli(directory: &std::path::Path) -> assert_cmd::Command {
    let mut command = cargo_bin_cmd!("cargo-leaderboard");
    command
        .env("CARGO_LEADERBOARD_CONFIG_DIR", directory)
        .env_remove("CARGO_LEADERBOARD_API_URL")
        .env_remove("CARGO_LEADERBOARD_NICKNAME")
        .env_remove("CARGO_LEADERBOARD_TOKEN");
    command
}
#[tokio::test(flavor = "multi_thread")]
async fn login_stores_private_scoped_token_doctor_verifies_and_logout_revokes() -> Result<()> {
    let dir = TempDir::new()?;
    let revoked = Arc::new(AtomicUsize::new(0));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let origin = format!("http://{}", listener.local_addr()?);
    let approval = format!("{origin}/account.html?code=ABCDE12345");
    let app = Router::new()
        .route("/auth/device-start", post(move || { let approval = approval.clone(); async move {
            Json(json!({"device_code": "device", "user_code":"ABCDE-12345", "verification_uri_complete":approval,"expires_in":30,"interval":1}))
        }}))
        .route("/auth/device-poll", post(|| async {
            Json(json!({"token":"clb_cli_test-credential", "github_login":"verified-alex"}))
        }))
        .route("/auth/me", get(|headers: HeaderMap| async move {
            assert_eq!(headers.get("authorization").unwrap(), "Bearer clb_cli_test-credential");
            Json(json!({"user":{"github_login":"verified-alex"}}))
        }))
        .route("/v1/leaderboard", get(|| async { Json(json!({"entries":[]})) }))
        .route("/auth/cli-logout", post(|State(revoked): State<Arc<AtomicUsize>>, headers: HeaderMap| async move {
            assert_eq!(headers.get("authorization").unwrap(), "Bearer clb_cli_test-credential");
            revoked.fetch_add(1, Ordering::SeqCst);
            Json(json!({"ok":true}))
        })).with_state(revoked.clone());
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let output = cli(dir.path())
        .args(["login", "--no-browser", "--api-url", &origin])
        .output()?;
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Logged in as @verified-alex"));
    assert!(!stdout.contains("clb_cli_test-credential"));
    let path = dir.path().join("config.json");
    let saved: serde_json::Value = serde_json::from_slice(&std::fs::read(&path)?)?;
    assert_eq!(saved["token"], "clb_cli_test-credential");
    assert_eq!(saved["api_url"], origin);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&path)?.permissions().mode() & 0o777,
            0o600
        );
    }
    let doctor = cli(dir.path()).arg("doctor").output()?;
    assert!(doctor.status.success());
    assert!(
        String::from_utf8_lossy(&doctor.stdout).contains("Verified GitHub account: @verified-alex")
    );
    cli(dir.path()).arg("logout").assert().success();
    assert_eq!(revoked.load(Ordering::SeqCst), 1);
    let saved: serde_json::Value = serde_json::from_slice(&std::fs::read(path)?)?;
    assert!(saved.get("token").is_none());
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn saved_token_is_never_forwarded_to_another_server() -> Result<()> {
    let dir = TempDir::new()?;
    std::fs::write(dir.path().join("config.json"), json!({
        "api_url":"https://cargo-leaderboard.vercel.app", "nickname":"alex", "github_login":"alex", "token":"clb_cli_secret"
    }).to_string())?;
    let app = Router::new().route(
        "/v1/leaderboard",
        get(|headers: HeaderMap| async move {
            assert!(!headers.contains_key("authorization"));
            Json(json!({"entries":[]}))
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let origin = format!("http://{}", listener.local_addr()?);
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    cli(dir.path())
        .env("CARGO_LEADERBOARD_API_URL", origin)
        .arg("doctor")
        .assert()
        .success();
    Ok(())
}

use axum::body::Body;
use axum::http::{Request, StatusCode};
use chrono::{Duration, Utc};
use sqlx::SqlitePool;
use tower::ServiceExt;
use uuid::Uuid;

use crate::server::{AppState, app, init_db, insert_build_event};
use crate::types::BuildEvent;

#[tokio::test]
async fn leaderboard_returns_longest_builds_first() {
    let pool = SqlitePool::connect("sqlite::memory:")
        .await
        .expect("in-memory db");
    init_db(&pool).await.expect("schema to initialize");

    let now = Utc::now();
    insert_build_event(
        &pool,
        &BuildEvent {
            event_id: Uuid::new_v4(),
            nickname: "slow".to_string(),
            repo_slug: "acme/slow".to_string(),
            command: "build".to_string(),
            started_at: now,
            finished_at: now + Duration::seconds(3),
            duration_ms: 3_000,
            success: true,
            cargo_version: "cargo test".to_string(),
            client_version: "0.1.0".to_string(),
            bytes: 0,
            bytes_before: 0,
            bytes_after: 0,
            file_count: 0,
            profile: "debug".into(),
            platform: "test".into(),
            benchmark: None,
        },
    )
    .await
    .expect("first event inserted");

    insert_build_event(
        &pool,
        &BuildEvent {
            event_id: Uuid::new_v4(),
            nickname: "fast".to_string(),
            repo_slug: "acme/fast".to_string(),
            command: "build".to_string(),
            started_at: now,
            finished_at: now + Duration::seconds(1),
            duration_ms: 1_000,
            success: true,
            cargo_version: "cargo test".to_string(),
            client_version: "0.1.0".to_string(),
            bytes: 0,
            bytes_before: 0,
            bytes_after: 0,
            file_count: 0,
            profile: "debug".into(),
            platform: "test".into(),
            benchmark: None,
        },
    )
    .await
    .expect("second event inserted");

    let app = app(AppState::new(pool, None));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/leaderboard?metric=longest_single_build&limit=2")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body");
    let payload: serde_json::Value = serde_json::from_slice(&body).expect("json payload");
    assert_eq!(payload["entries"][0]["nickname"], "slow");
    assert_eq!(payload["entries"][1]["nickname"], "fast");
}

#[tokio::test]
async fn invalid_metric_returns_bad_request() {
    let pool = SqlitePool::connect("sqlite::memory:")
        .await
        .expect("in-memory db");
    init_db(&pool).await.expect("schema to initialize");

    let app = app(AppState::new(pool, None));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/leaderboard?metric=average_wait")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

fn sized_event(nickname: &str, command: &str, bytes: i64) -> BuildEvent {
    BuildEvent {
        event_id: Uuid::new_v4(),
        nickname: nickname.into(),
        repo_slug: "acme/project".into(),
        command: command.into(),
        started_at: Utc::now(),
        finished_at: Utc::now(),
        duration_ms: 10,
        success: true,
        cargo_version: "cargo 1.95".into(),
        client_version: "0.1.0".into(),
        bytes,
        bytes_before: if command == "clean" { bytes } else { 0 },
        bytes_after: if command == "build" { bytes } else { 0 },
        file_count: 1,
        profile: "debug".into(),
        platform: "test".into(),
        benchmark: None,
    }
}

#[tokio::test]
async fn size_boards_keep_personal_best_and_separate_cleans() {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    init_db(&pool).await.unwrap();
    for event in [
        sized_event("alice", "build", 10),
        sized_event("alice", "build", 30),
        sized_event("bob", "build", 20),
        sized_event("alice", "clean", 50),
        sized_event("zero", "clean", 0),
    ] {
        insert_build_event(&pool, &event).await.unwrap();
    }
    let router = app(AppState::new(pool, None));
    for (metric, expected) in [("largest_build", vec![30, 20]), ("largest_clean", vec![50])] {
        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/v1/leaderboard?metric={metric}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let bytes: Vec<_> = payload["entries"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["bytes"].as_i64().unwrap())
            .collect();
        assert_eq!(bytes, expected);
    }
}

#[tokio::test]
async fn submissions_validate_auth_and_allow_idempotent_retries() {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    init_db(&pool).await.unwrap();
    let router = app(AppState::new(pool.clone(), Some("secret".into())));
    let event = sized_event("alice", "clean", 100);
    for (token, payload, status) in [
        (
            "wrong",
            serde_json::to_value(&event).unwrap(),
            StatusCode::UNAUTHORIZED,
        ),
        (
            "secret",
            {
                let mut v = serde_json::to_value(&event).unwrap();
                v["bytes"] = (-1).into();
                v
            },
            StatusCode::BAD_REQUEST,
        ),
        (
            "secret",
            serde_json::to_value(&event).unwrap(),
            StatusCode::CREATED,
        ),
        (
            "secret",
            serde_json::to_value(&event).unwrap(),
            StatusCode::CREATED,
        ),
    ] {
        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/build-events")
                    .header("content-type", "application/json")
                    .header("authorization", format!("Bearer {token}"))
                    .body(Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), status);
    }
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM build_events")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count.0, 1);
}

#[tokio::test]
async fn migration_preserves_legacy_duration_events() {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    sqlx::query("CREATE TABLE build_events (event_id TEXT PRIMARY KEY, nickname TEXT NOT NULL, repo_slug TEXT NOT NULL, command TEXT NOT NULL, started_at TEXT NOT NULL, finished_at TEXT NOT NULL, duration_ms INTEGER NOT NULL, success INTEGER NOT NULL, cargo_version TEXT NOT NULL, client_version TEXT NOT NULL)").execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO build_events VALUES ('old', 'old', 'old', 'build', '2026-01-01T00:00:00Z', '2026-01-01T00:00:01Z', 1000, 1, 'old', 'old')").execute(&pool).await.unwrap();
    init_db(&pool).await.unwrap();
    init_db(&pool).await.unwrap();
    let entries = crate::server::leaderboard_entries(&pool, 10).await.unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].bytes, None);
}

#[tokio::test]
async fn fresh_scores_never_mix_with_accumulated_folders() {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    init_db(&pool).await.unwrap();
    init_db(&pool).await.unwrap();
    let old = sized_event("alice", "build", 1000);
    insert_build_event(&pool, &old).await.unwrap();
    let mut fresh = sized_event("alice", "build", 20);
    fresh.benchmark = Some(crate::types::BenchmarkMetadata {
        rustc_version: "rustc 1.95.0".into(),
        revision: Some("a".repeat(40)),
        dirty: Some(false),
        cargo_args: vec!["--bins".into()],
    });
    assert!(fresh.validate().is_ok());
    insert_build_event(&pool, &fresh).await.unwrap();
    let router = app(AppState::new(pool, None));
    for (metric, expected) in [("largest_fresh_build", 20), ("largest_build", 1000)] {
        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/v1/leaderboard?metric={metric}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let board: crate::types::LeaderboardResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(board.entries.len(), 1);
        assert_eq!(board.entries[0].bytes, Some(expected));
        assert_eq!(
            board.entries[0].benchmark.is_some(),
            metric == "largest_fresh_build"
        );
    }
    fresh.bytes_before = 1;
    assert!(fresh.validate().is_err());
    fresh.bytes_before = 0;
    fresh.profile = "release".into();
    assert!(fresh.validate().is_err());
}

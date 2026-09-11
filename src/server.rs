use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::{Result, anyhow};
use axum::extract::{DefaultBodyLimit, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{Html, IntoResponse};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::{SqlitePool, query, query_as};
use std::str::FromStr;

use crate::types::{BuildEvent, LeaderboardEntry, LeaderboardResponse};

#[derive(Clone)]
pub struct AppState {
    pool: SqlitePool,
    auth_token: Option<Arc<str>>,
}

impl AppState {
    pub(crate) fn new(pool: SqlitePool, auth_token: Option<String>) -> Self {
        Self {
            pool,
            auth_token: auth_token.map(Arc::<str>::from),
        }
    }
}

pub async fn run_server(
    bind: SocketAddr,
    database_url: &str,
    auth_token: Option<String>,
) -> Result<()> {
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(SqliteConnectOptions::from_str(database_url)?.create_if_missing(true))
        .await?;

    init_db(&pool).await?;

    let app = app(AppState::new(pool, auth_token));

    let listener = tokio::net::TcpListener::bind(bind).await?;
    eprintln!(
        "Leaderboard API listening on http://{}",
        listener.local_addr()?
    );
    axum::serve(listener, app).await?;
    Ok(())
}

pub fn app(state: AppState) -> Router {
    Router::new()
        .route(
            "/",
            get(|| async { Html(include_str!("../public/index.html")) }),
        )
        .route(
            "/styles.css",
            get(|| async {
                (
                    [("content-type", "text/css")],
                    include_str!("../public/styles.css"),
                )
            }),
        )
        .route(
            "/app.js",
            get(|| async {
                (
                    [("content-type", "text/javascript")],
                    include_str!("../public/app.js"),
                )
            }),
        )
        .route(
            "/favicon.svg",
            get(|| async {
                (
                    [("content-type", "image/svg+xml")],
                    include_str!("../public/favicon.svg"),
                )
            }),
        )
        .route(
            "/install.sh",
            get(|| async {
                (
                    [("content-type", "text/plain")],
                    include_str!("../public/install.sh"),
                )
            }),
        )
        .route(
            "/install.ps1",
            get(|| async {
                (
                    [("content-type", "text/plain")],
                    include_str!("../public/install.ps1"),
                )
            }),
        )
        .route(
            "/agents.md",
            get(|| async {
                (
                    [("content-type", "text/plain; charset=utf-8")],
                    include_str!("../public/agents.md"),
                )
            }),
        )
        .route(
            "/llms.txt",
            get(|| async {
                (
                    [("content-type", "text/plain; charset=utf-8")],
                    include_str!("../public/llms.txt"),
                )
            }),
        )
        .route("/v1/build-events", post(create_build_event))
        .route("/v1/leaderboard", get(get_leaderboard))
        .layer(DefaultBodyLimit::max(16_384))
        .with_state(state)
}

pub async fn init_db(pool: &SqlitePool) -> Result<()> {
    query(
        r#"
        CREATE TABLE IF NOT EXISTS build_events (
            event_id TEXT PRIMARY KEY,
            nickname TEXT NOT NULL,
            repo_slug TEXT NOT NULL,
            command TEXT NOT NULL,
            started_at TEXT NOT NULL,
            finished_at TEXT NOT NULL,
            duration_ms INTEGER NOT NULL,
            success INTEGER NOT NULL,
            cargo_version TEXT NOT NULL,
            client_version TEXT NOT NULL
        )
        "#,
    )
    .execute(pool)
    .await?;

    // Upgrade the original duration-only prototype without discarding its events.
    for (name, definition) in [
        ("bytes", "INTEGER"),
        ("bytes_before", "INTEGER"),
        ("bytes_after", "INTEGER"),
        ("file_count", "INTEGER"),
        ("profile", "TEXT NOT NULL DEFAULT 'unknown'"),
        ("platform", "TEXT NOT NULL DEFAULT 'unknown'"),
    ] {
        let columns: Vec<(i64, String, String, i64, Option<String>, i64)> =
            sqlx::query_as("PRAGMA table_info(build_events)")
                .fetch_all(pool)
                .await?;
        if !columns.iter().any(|c| c.1 == name) {
            query(&format!(
                "ALTER TABLE build_events ADD COLUMN {name} {definition}"
            ))
            .execute(pool)
            .await?;
        }
    }
    query("CREATE INDEX IF NOT EXISTS events_ranking ON build_events(command, bytes DESC, duration_ms DESC)").execute(pool).await?;
    Ok(())
}

async fn create_build_event(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(event): Json<BuildEvent>,
) -> Result<impl IntoResponse, ApiError> {
    state.authorize(&headers)?;

    event.validate().map_err(ApiError::bad_request)?;

    insert_build_event(&state.pool, &event).await?;
    Ok(StatusCode::CREATED)
}

async fn get_leaderboard(
    State(state): State<AppState>,
    Query(params): Query<LeaderboardParams>,
) -> Result<Json<LeaderboardResponse>, ApiError> {
    let metric = params
        .metric
        .unwrap_or_else(|| "longest_single_build".to_string());
    if !["longest_single_build", "largest_build", "largest_clean"].contains(&metric.as_str()) {
        return Err(ApiError::bad_request("unsupported metric"));
    }

    let limit = params.limit.unwrap_or(50).clamp(1, 200);
    let entries = ranked_entries(&state.pool, limit, &metric).await?;

    Ok(Json(LeaderboardResponse { metric, entries }))
}

pub async fn insert_build_event(pool: &SqlitePool, event: &BuildEvent) -> Result<()> {
    query(
        r#"
        INSERT INTO build_events (
            event_id, nickname, repo_slug, command, started_at, finished_at,
            duration_ms, success, cargo_version, client_version, bytes, bytes_before, bytes_after, file_count, profile, platform
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)
        ON CONFLICT(event_id) DO NOTHING
        "#,
    )
    .bind(event.event_id.to_string())
    .bind(&event.nickname)
    .bind(&event.repo_slug)
    .bind(&event.command)
    .bind(event.started_at.to_rfc3339())
    .bind(event.finished_at.to_rfc3339())
    .bind(event.duration_ms)
    .bind(event.success)
    .bind(&event.cargo_version)
    .bind(&event.client_version)
    .bind(event.bytes).bind(event.bytes_before).bind(event.bytes_after).bind(event.file_count).bind(&event.profile).bind(&event.platform)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn leaderboard_entries(pool: &SqlitePool, limit: i64) -> Result<Vec<LeaderboardEntry>> {
    ranked_entries(pool, limit, "longest_single_build").await
}

async fn ranked_entries(
    pool: &SqlitePool,
    limit: i64,
    metric: &str,
) -> Result<Vec<LeaderboardEntry>> {
    let (command, column, filter) = match metric {
        "largest_clean" => ("clean", "bytes", "AND bytes > 0"),
        "largest_build" => ("build", "bytes", "AND bytes > 0"),
        _ => ("build", "duration_ms", ""),
    };
    let sql = format!(
        r#"
        WITH ranked AS (
            SELECT *, ROW_NUMBER() OVER (
                PARTITION BY nickname, repo_slug ORDER BY {column} DESC, finished_at DESC, event_id DESC
            ) AS rank FROM build_events WHERE success = 1 AND command = ?1 {filter}
        )
        SELECT event_id, nickname, repo_slug, bytes, file_count, profile, platform, duration_ms, finished_at
        FROM ranked WHERE rank = 1 ORDER BY {column} DESC, finished_at DESC, event_id DESC LIMIT ?2
    "#
    );
    Ok(query_as::<_, LeaderboardEntry>(&sql)
        .bind(command)
        .bind(limit)
        .fetch_all(pool)
        .await?)
}

#[derive(Debug, Deserialize)]
struct LeaderboardParams {
    metric: Option<String>,
    limit: Option<i64>,
}

#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
        }
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(error: anyhow::Error) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: {
                eprintln!("database error: {error:#}");
                "internal server error".into()
            },
        }
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(error: sqlx::Error) -> Self {
        anyhow!(error).into()
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        (self.status, self.message).into_response()
    }
}

impl AppState {
    fn authorize(&self, headers: &HeaderMap) -> Result<(), ApiError> {
        let Some(expected) = &self.auth_token else {
            return Ok(());
        };

        let provided = headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.strip_prefix("Bearer "));

        if provided == Some(expected.as_ref()) {
            Ok(())
        } else {
            Err(ApiError {
                status: StatusCode::UNAUTHORIZED,
                message: "missing or invalid bearer token".to_string(),
            })
        }
    }
}

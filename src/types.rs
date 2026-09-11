use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BuildEvent {
    pub event_id: Uuid,
    pub nickname: String,
    pub repo_slug: String,
    pub command: String,
    pub started_at: DateTime<Utc>,
    pub finished_at: DateTime<Utc>,
    pub duration_ms: i64,
    pub success: bool,
    pub cargo_version: String,
    pub client_version: String,
    pub bytes: i64,
    pub bytes_before: i64,
    pub bytes_after: i64,
    pub file_count: i64,
    pub profile: String,
    pub platform: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, PartialEq, Eq)]
pub struct LeaderboardEntry {
    pub event_id: String,
    pub bytes: Option<i64>,
    pub file_count: Option<i64>,
    pub profile: String,
    pub platform: String,
    pub nickname: String,
    pub repo_slug: String,
    pub duration_ms: i64,
    pub finished_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LeaderboardResponse {
    pub metric: String,
    pub entries: Vec<LeaderboardEntry>,
}

impl BuildEvent {
    pub fn validate(&self) -> Result<(), &'static str> {
        if !["build", "clean"].contains(&self.command.as_str()) || !self.success {
            return Err("only successful build and clean events are accepted");
        }
        for (value, max) in [
            (&self.nickname, 40),
            (&self.repo_slug, 160),
            (&self.profile, 40),
            (&self.platform, 80),
            (&self.cargo_version, 120),
            (&self.client_version, 40),
        ] {
            if value.trim().is_empty()
                || value.chars().count() > max
                || value.chars().any(char::is_control)
            {
                return Err("invalid or overlong event label");
            }
        }
        if [
            self.bytes,
            self.bytes_before,
            self.bytes_after,
            self.file_count,
            self.duration_ms,
        ]
        .iter()
        .any(|n| *n < 0 || *n > 9_007_199_254_740_991)
        {
            return Err("measurements must be nonnegative safe integers");
        }
        let expected = if self.command == "build" {
            self.bytes_after
        } else {
            (self.bytes_before - self.bytes_after).max(0)
        };
        if self.bytes != expected {
            return Err("bytes do not match before/after measurements");
        }
        if self.finished_at < self.started_at
            || self.finished_at > Utc::now() + chrono::Duration::minutes(5)
        {
            return Err("invalid event timestamps");
        }
        Ok(())
    }
}

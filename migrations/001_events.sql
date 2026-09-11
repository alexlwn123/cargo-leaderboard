CREATE TABLE IF NOT EXISTS build_events (
  event_id UUID PRIMARY KEY,
  nickname TEXT NOT NULL CHECK (char_length(nickname) BETWEEN 1 AND 40),
  repo_slug TEXT NOT NULL CHECK (char_length(repo_slug) BETWEEN 1 AND 160),
  command TEXT NOT NULL CHECK (command IN ('build', 'clean')),
  started_at TIMESTAMPTZ NOT NULL,
  finished_at TIMESTAMPTZ NOT NULL,
  duration_ms BIGINT NOT NULL CHECK (duration_ms BETWEEN 0 AND 9007199254740991),
  success BOOLEAN NOT NULL CHECK (success),
  cargo_version TEXT NOT NULL,
  client_version TEXT NOT NULL,
  bytes BIGINT NOT NULL CHECK (bytes BETWEEN 0 AND 9007199254740991),
  bytes_before BIGINT NOT NULL CHECK (bytes_before BETWEEN 0 AND 9007199254740991),
  bytes_after BIGINT NOT NULL CHECK (bytes_after BETWEEN 0 AND 9007199254740991),
  file_count BIGINT NOT NULL CHECK (file_count BETWEEN 0 AND 9007199254740991),
  profile TEXT NOT NULL,
  platform TEXT NOT NULL,
  received_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  CHECK (finished_at >= started_at),
  CHECK (bytes = CASE WHEN command = 'build' THEN bytes_after ELSE GREATEST(0, bytes_before - bytes_after) END)
);
CREATE INDEX IF NOT EXISTS events_size_ranking ON build_events(command, bytes DESC, finished_at DESC);
CREATE INDEX IF NOT EXISTS events_time_ranking ON build_events(command, duration_ms DESC, finished_at DESC);
CREATE TABLE IF NOT EXISTS submission_limits (
  client_hash TEXT PRIMARY KEY,
  window_start TIMESTAMPTZ NOT NULL,
  count INTEGER NOT NULL
);

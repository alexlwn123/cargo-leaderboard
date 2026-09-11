CREATE TABLE IF NOT EXISTS github_users (
  github_id BIGINT PRIMARY KEY CHECK (github_id > 0),
  github_login TEXT NOT NULL CHECK (char_length(github_login) BETWEEN 1 AND 39),
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
ALTER TABLE build_events ADD COLUMN IF NOT EXISTS github_id BIGINT REFERENCES github_users(github_id);
CREATE INDEX IF NOT EXISTS events_github_ranking ON build_events(github_id, repo_slug, command);
CREATE TABLE IF NOT EXISTS auth_tokens (
  token_hash TEXT PRIMARY KEY,
  github_id BIGINT NOT NULL REFERENCES github_users(github_id),
  kind TEXT NOT NULL CHECK (kind IN ('session', 'cli')),
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  expires_at TIMESTAMPTZ NOT NULL
);
CREATE INDEX IF NOT EXISTS auth_tokens_account ON auth_tokens(github_id, kind);
CREATE INDEX IF NOT EXISTS auth_tokens_expiry ON auth_tokens(expires_at);
CREATE TABLE IF NOT EXISTS oauth_states (
  state_hash TEXT PRIMARY KEY,
  verifier TEXT NOT NULL,
  user_code TEXT,
  expires_at TIMESTAMPTZ NOT NULL DEFAULT NOW() + INTERVAL '10 minutes'
);
CREATE INDEX IF NOT EXISTS oauth_states_expiry ON oauth_states(expires_at);
CREATE TABLE IF NOT EXISTS device_logins (
  device_hash TEXT PRIMARY KEY,
  user_code TEXT NOT NULL UNIQUE,
  github_id BIGINT REFERENCES github_users(github_id),
  expires_at TIMESTAMPTZ NOT NULL DEFAULT NOW() + INTERVAL '10 minutes',
  next_poll_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS device_logins_expiry ON device_logins(expires_at);

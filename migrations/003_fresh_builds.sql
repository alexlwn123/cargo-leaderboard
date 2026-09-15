-- Legacy events remain accumulated build-folder scores. No scores are reclassified.
ALTER TABLE build_events ADD COLUMN IF NOT EXISTS benchmark JSONB;
CREATE INDEX IF NOT EXISTS events_fresh_ranking ON build_events(bytes DESC, finished_at DESC) WHERE benchmark IS NOT NULL;

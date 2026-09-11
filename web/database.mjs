import { neon } from "@neondatabase/serverless";
import { metrics } from "./contract.mjs";
let client;
export function database() {
  if (!process.env.DATABASE_URL)
    throw new Error("DATABASE_URL is not configured");
  return (client ??= neon(process.env.DATABASE_URL));
}
export async function leaderboard(metric, limit) {
  const { command, column } = metrics[metric];
  const sql = database();
  const entries = await sql.query(
    `
    WITH ranked AS (
      SELECT *, ROW_NUMBER() OVER (PARTITION BY nickname, repo_slug ORDER BY ${column} DESC, finished_at DESC, event_id DESC) AS rank
      FROM build_events WHERE command = $1 AND success AND ${column} > 0
    ) SELECT event_id, nickname, repo_slug, bytes, file_count, duration_ms, profile, platform, finished_at
      FROM ranked WHERE rank = 1 ORDER BY ${column} DESC, finished_at DESC, event_id DESC LIMIT $2
  `,
    [command, limit],
  );
  return {
    metric,
    entries: entries.map((e) => ({
      ...e,
      bytes: Number(e.bytes),
      file_count: Number(e.file_count),
      duration_ms: Number(e.duration_ms),
    })),
  };
}
export async function submit(event, clientHash) {
  const sql = database();
  const existing =
    await sql`SELECT event_id FROM build_events WHERE event_id = ${event.event_id}`;
  if (existing.length) return "duplicate";
  // The upsert serializes concurrent submissions from one client. Raw IPs are never stored.
  const quota = await sql`
    INSERT INTO submission_limits (client_hash, window_start, count) VALUES (${clientHash}, date_trunc('hour', NOW()), 1)
    ON CONFLICT (client_hash) DO UPDATE SET
      count = CASE WHEN submission_limits.window_start < date_trunc('hour', NOW()) THEN 1 ELSE submission_limits.count + 1 END,
      window_start = date_trunc('hour', NOW())
    WHERE submission_limits.window_start < date_trunc('hour', NOW()) OR submission_limits.count < 30
    RETURNING count`;
  if (!quota.length) return "limited";
  await sql`
    INSERT INTO build_events (event_id, nickname, repo_slug, command, started_at, finished_at, duration_ms, success, cargo_version, client_version, bytes, bytes_before, bytes_after, file_count, profile, platform)
    VALUES (${event.event_id}, ${event.nickname}, ${event.repo_slug}, ${event.command}, ${event.started_at}, ${event.finished_at}, ${event.duration_ms}, ${event.success}, ${event.cargo_version}, ${event.client_version}, ${event.bytes}, ${event.bytes_before}, ${event.bytes_after}, ${event.file_count}, ${event.profile}, ${event.platform})
    ON CONFLICT (event_id) DO NOTHING`;
  // Keep the abuse-control table bounded over time.
  await sql`DELETE FROM submission_limits WHERE window_start < NOW() - INTERVAL '2 days'`;
  return "created";
}

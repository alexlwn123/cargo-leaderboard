import { neon } from "@neondatabase/serverless";
import { postgres } from "./local-database.mjs";
import { metrics } from "./contract.mjs";
let client;
export function database() {
  if (!process.env.DATABASE_URL)
    throw new Error("DATABASE_URL is not configured");
  const url = new URL(process.env.DATABASE_URL);
  return (client ??= !process.env.VERCEL && ["localhost", "127.0.0.1"].includes(url.hostname)
    ? postgres(process.env.DATABASE_URL) : neon(process.env.DATABASE_URL));
}
export async function leaderboard(metric, limit, sql = database()) {
  const { command, column, fresh } = metrics[metric];
  const scope = fresh === undefined ? "" : `AND benchmark IS ${fresh ? "NOT " : ""}NULL`;
  const entries = await sql.query(
    `
    WITH ranked AS (
      SELECT *, ROW_NUMBER() OVER (PARTITION BY github_id, repo_slug ORDER BY ${column} DESC, finished_at DESC, event_id DESC) AS rank
      FROM build_events WHERE github_id IS NOT NULL AND command = $1 AND success AND ${column} > 0 ${scope}
    ) SELECT event_id, github_users.github_id::text, github_login, repo_slug, bytes, file_count, duration_ms, profile, platform, finished_at, benchmark
      FROM ranked JOIN github_users USING (github_id) WHERE rank = 1 ORDER BY ${column} DESC, finished_at DESC, event_id DESC LIMIT $2
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
export async function submit(event, user, sql = database()) {
  const existing = await sql`SELECT event_id FROM build_events WHERE event_id = ${event.event_id} AND github_id = ${user.github_id}`;
  if (existing.length) return "duplicate";
  await sql`
    INSERT INTO build_events (event_id, nickname, github_id, repo_slug, command, started_at, finished_at, duration_ms, success, cargo_version, client_version, bytes, bytes_before, bytes_after, file_count, profile, platform, benchmark)
    VALUES (${event.event_id}, ${user.github_login}, ${user.github_id}, ${event.repo_slug}, ${event.command}, ${event.started_at}, ${event.finished_at}, ${event.duration_ms}, ${event.success}, ${event.cargo_version}, ${event.client_version}, ${event.bytes}, ${event.bytes_before}, ${event.bytes_after}, ${event.file_count}, ${event.profile}, ${event.platform}, ${event.benchmark ? JSON.stringify(event.benchmark) : null})
    ON CONFLICT (event_id) DO NOTHING`;
  return "created";
}

// Explicit integration check: inserts only uniquely identified test rows, then removes them.
import assert from "node:assert/strict";
import { randomUUID } from "node:crypto";
import { database, submit, leaderboard } from "../web/database.mjs";
import { validateEvent } from "../web/contract.mjs";
const sql = database();
const nickname = `test-${randomUUID().slice(0, 8)}`;
const client = `test-${randomUUID()}`;
const ids = [];
function event(command, bytes) {
  const e = validateEvent({
    event_id: randomUUID(),
    nickname,
    repo_slug: "integration/disposable",
    command,
    success: true,
    started_at: "2026-01-01T00:00:00Z",
    finished_at: "2026-01-01T00:00:01Z",
    duration_ms: 1000,
    bytes,
    bytes_before: command === "clean" ? bytes : 0,
    bytes_after: command === "build" ? bytes : 0,
    file_count: 1,
    profile: "debug",
    platform: "test",
    cargo_version: "test",
    client_version: "test",
  });
  ids.push(e.event_id);
  return e;
}
try {
  const first = event("build", 40);
  assert.equal(await submit(first, client), "created");
  assert.equal(await submit(first, client), "duplicate");
  await submit(event("build", 20), client);
  await submit(event("clean", 30), client);
  const builds = (await leaderboard("largest_build", 200)).entries.filter(
    (e) => e.nickname === nickname,
  );
  const cleans = (await leaderboard("largest_clean", 200)).entries.filter(
    (e) => e.nickname === nickname,
  );
  assert.deepEqual(
    builds.map((e) => e.bytes),
    [40],
  );
  assert.deepEqual(
    cleans.map((e) => e.bytes),
    [30],
  );
  await sql`UPDATE submission_limits SET count = 29 WHERE client_hash = ${client}`;
  const results = await Promise.all([
    submit(event("build", 50), client),
    submit(event("build", 60), client),
  ]);
  assert.deepEqual(results.sort(), ["created", "limited"]);
  const persisted =
    await sql`SELECT COUNT(*)::int AS count FROM build_events WHERE nickname = ${nickname}`;
  assert.equal(persisted[0].count, 4);
  console.log(
    "Real database: persistence, ranking, deduplication and concurrent rate limiting passed.",
  );
} finally {
  await sql.query("DELETE FROM build_events WHERE event_id = ANY($1::uuid[])", [
    ids,
  ]);
  await sql`DELETE FROM submission_limits WHERE client_hash = ${client}`;
  console.log("Disposable test rows removed.");
}

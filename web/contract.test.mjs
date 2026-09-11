import test from "node:test";
import assert from "node:assert/strict";
import { randomUUID } from "node:crypto";
import { validateEvent, parseQuery } from "./contract.mjs";
import { createEventsHandler } from "../api/build-events.mjs";
const handler = createEventsHandler({ authenticate: async () => ({ github_id: "1", github_login: "alex" }), consume: async () => {} });
import { clientHash } from "./http.mjs";
const event = () => ({
  event_id: randomUUID(),
  nickname: "alex",
  repo_slug: "acme/rust",
  command: "build",
  success: true,
  started_at: "2026-01-01T00:00:00Z",
  finished_at: "2026-01-01T00:00:01Z",
  duration_ms: 1000,
  bytes: 42,
  bytes_before: 0,
  bytes_after: 42,
  file_count: 1,
  profile: "debug",
  platform: "linux-x86_64",
  cargo_version: "cargo 1.95.0",
  client_version: "0.1.0",
});
test("valid build and clean have consistent scores", () => {
  assert.equal(validateEvent(event()).bytes, 42);
  assert.equal(
    validateEvent({
      ...event(),
      command: "clean",
      bytes_before: 52,
      bytes_after: 10,
    }).bytes,
    42,
  );
  assert.throws(
    () => validateEvent({ ...event(), bytes: 100 }),
    /before\/after/,
  );
});
test("rejects invalid, oversized, and unsafe fields", () => {
  for (const patch of [
    { bytes: -1 },
    { bytes_after: NaN },
    { duration_ms: Number.MAX_SAFE_INTEGER + 1 },
    { file_count: 1.5 },
    { nickname: " " },
    { nickname: "x".repeat(41) },
    { repo_slug: "a\nb" },
    { success: false },
    { event_id: "bad-id" },
    { command: "delete" },
    { started_at: "2026-02-01T00:00:00Z" },
    { finished_at: "9999-01-01T00:00:00Z" },
    { finished_at: "yesterday" },
  ])
    assert.throws(() => validateEvent({ ...event(), ...patch }));
});
test("query validation never interpolates an untrusted metric", () => {
  assert.deepEqual(parseQuery("/"), { metric: "largest_build", limit: 100 });
  assert.equal(parseQuery("/?limit=900").limit, 200);
  assert.equal(parseQuery("/?limit=0").limit, 1);
  for (const query of [
    "?metric=__proto__",
    "?metric=bytes;DROP",
    "?limit=foo",
    "?limit=-1",
    "?limit=1.5",
  ])
    assert.throws(() => parseQuery("/" + query));
});
test("client hash does not trust forwarded-for", () => {
  const base = { headers: {}, socket: { remoteAddress: "127.0.0.1" } };
  assert.equal(clientHash(base), clientHash({ ...base, headers: { "x-forwarded-for": "1.2.3.4" } }));
});
test("HTTP boundary rejects method, content type, bad JSON and oversized events before database access", async () => {
  for (const [req, status] of [
    [{ method: "GET", headers: {} }, 405],
    [{ method: "POST", headers: {} }, 415],
    [
      {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: "{",
      },
      400,
    ],
    [
      {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: "x".repeat(17000),
      },
      413,
    ],
    [
      {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: {},
      },
      400,
    ],
  ]) {
    const response = {
      setHeader() {},
      end(value) {
        this.body = value;
      },
    };
    await handler(req, response);
    assert.equal(response.statusCode, status);
    assert.equal(typeof JSON.parse(response.body).error, "string");
  }
});

test("Vercel lazy body-parser errors remain HTTP 400", async () => {
  const req = {
    method: "POST",
    headers: { "content-type": "application/json" },
    get body() {
      throw Object.assign(new Error("Invalid JSON"), { statusCode: 400 });
    },
  };
  const res = {
    setHeader() {},
    end(value) {
      this.body = value;
    },
  };
  await handler(req, res);
  assert.equal(res.statusCode, 400);
  assert.match(JSON.parse(res.body).error, /JSON/);
});

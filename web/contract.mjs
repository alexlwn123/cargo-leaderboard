export const metrics = {
  largest_build: { command: "build", column: "bytes", fresh: false },
  largest_fresh_build: { command: "build", column: "bytes", fresh: true },
  largest_clean: { command: "clean", column: "bytes" },
  longest_single_build: { command: "build", column: "duration_ms", fresh: false },
};
export class HttpError extends Error {
  constructor(status, message) {
    super(message);
    this.status = status;
  }
}
export function validateEvent(value, now = Date.now()) {
  const fail = (message) => {
    throw new HttpError(400, message);
  };
  if (!value || typeof value !== "object" || Array.isArray(value))
    fail("Expected an event object.");
  const e = {};
  for (const [field, max] of Object.entries({
    nickname: 40,
    repo_slug: 160,
    profile: 40,
    platform: 80,
    cargo_version: 120,
    client_version: 40,
  })) {
    if (
      typeof value[field] !== "string" ||
      !value[field].trim() ||
      [...value[field]].length > max ||
      /[\p{Cc}]/u.test(value[field])
    )
      fail(`Invalid ${field}.`);
    e[field] = value[field];
  }
  if (!["build", "clean"].includes(value.command) || value.success !== true)
    fail("Only successful builds and cleans are accepted.");
  if (
    typeof value.event_id !== "string" ||
    !/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(
      value.event_id,
    )
  )
    fail("Invalid event_id.");
  for (const field of [
    "bytes",
    "bytes_before",
    "bytes_after",
    "file_count",
    "duration_ms",
  ]) {
    if (!Number.isSafeInteger(value[field]) || value[field] < 0)
      fail(`Invalid ${field}.`);
    e[field] = value[field];
  }
  for (const field of ["started_at", "finished_at"]) {
    if (
      typeof value[field] !== "string" ||
      !/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d+)?(Z|[+-]\d{2}:\d{2})$/.test(
        value[field],
      ) ||
      !Number.isFinite(Date.parse(value[field]))
    )
      fail(`Invalid ${field}.`);
    e[field] = new Date(value[field]).toISOString();
  }
  if (
    Date.parse(e.finished_at) < Date.parse(e.started_at) ||
    Date.parse(e.finished_at) > now + 300_000
  )
    fail("Invalid event timestamps.");
  const expected =
    value.command === "build"
      ? e.bytes_after
      : Math.max(0, e.bytes_before - e.bytes_after);
  if (e.bytes !== expected)
    fail("Bytes do not match before/after measurements.");
  if (value.benchmark != null) {
    const b = value.benchmark;
    if (value.command !== "build" || e.bytes_before !== 0 || e.profile !== "debug")
      fail("Benchmarks require a fresh debug build.");
    if (typeof b !== "object" || Array.isArray(b) ||
        typeof b.rustc_version !== "string" || !b.rustc_version || b.rustc_version.length > 120 || /[\p{Cc}]/u.test(b.rustc_version) ||
        (b.revision !== null && !/^(?:[0-9a-f]{40}|[0-9a-f]{64})$/i.test(b.revision)) ||
        (b.dirty !== null && typeof b.dirty !== "boolean")) fail("Invalid benchmark context.");
    validateBenchmarkArgs(b.cargo_args, fail);
    e.benchmark = { rustc_version: b.rustc_version, revision: b.revision, dirty: b.dirty, cargo_args: b.cargo_args };
  }
  return {
    ...e,
    command: value.command,
    success: true,
    event_id: value.event_id.toLowerCase(),
  };
}
export function parseQuery(url) {
  const params = new URL(url, "http://localhost").searchParams;
  const metric = params.get("metric") || "largest_fresh_build";
  if (!Object.hasOwn(metrics, metric))
    throw new HttpError(400, "Unsupported metric.");
  const raw = params.get("limit") ?? "100";
  if (!/^\d+$/.test(raw) || !Number.isSafeInteger(Number(raw)))
    throw new HttpError(400, "Invalid limit.");
  const limit = Math.max(1, Math.min(200, Number(raw)));
  const rawPage = params.get("page") ?? "1";
  const page = Number(rawPage);
  if (!/^\d+$/.test(rawPage) || !Number.isSafeInteger(page) || page < 1 ||
      !Number.isSafeInteger((page - 1) * limit))
    throw new HttpError(400, "Invalid page.");
  return { metric, limit, page };
}

// Matches src/benchmark.rs. Reject profile, config and output-directory overrides.
function validateBenchmarkArgs(args, fail) {
  if (!Array.isArray(args) || args.length > 64) fail("Invalid benchmark arguments.");
  const flags = ["--all-features", "--no-default-features", "--workspace", "--bins", "--lib", "--locked", "--offline", "--frozen"];
  const options = ["--features", "-F", "--package", "-p", "--exclude", "--target", "--jobs", "-j"];
  const valid = v => typeof v === "string" && v.length > 0 && v.length <= 200 && !/[\p{Cc}]/u.test(v);
  for (let i = 0; i < args.length; i++) {
    const arg = args[i];
    if (!valid(arg)) fail("Invalid benchmark argument.");
    const eq = arg.indexOf("=");
    const key = eq < 0 ? arg : arg.slice(0, eq);
    if (eq < 0 && flags.includes(key)) continue;
    if (!options.includes(key)) fail("Unsupported benchmark option.");
    const value = eq < 0 ? args[++i] : arg.slice(eq + 1);
    if (!valid(value) || value.startsWith("-") || (key === "--target" && /[/.\\]/.test(value))) fail("Invalid benchmark option value.");
  }
}

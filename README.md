# Cargo Leaderboard

A heavyweight Rust competition: compare enormous build directories, satisfying cleans, and long waits for the compiler.

**[View the leaderboard](https://cargo-leaderboard.vercel.app)**

## Submit a project

Install with a current stable Rust toolchain and Git:

```sh
cargo install --git https://github.com/alexlwn123/cargo-leaderboard --locked
export CARGO_LEADERBOARD_NICKNAME="your-name"
export CARGO_LEADERBOARD_API_URL="https://cargo-leaderboard.vercel.app"

# Run from your Rust project:
cargo leaderboard build
cargo leaderboard clean
```

`clean` runs the real `cargo clean` and deletes its build artifacts. The wrapper never cleans automatically before a build.

Pass normal Cargo flags after `--`. Put wrapper options (`--repo`, `--no-submit`) before that separator:

```sh
cargo leaderboard build -- --release --features full
cargo leaderboard build -- --manifest-path ../other/Cargo.toml --target-dir /tmp/rust-output
cargo leaderboard clean -- --release
cargo leaderboard build --repo my-private-project
cargo leaderboard build --no-submit
```

PowerShell configuration:

```powershell
$env:CARGO_LEADERBOARD_NICKNAME = "your-name"
$env:CARGO_LEADERBOARD_API_URL = "https://cargo-leaderboard.vercel.app"
```

Both `cargo leaderboard …` and `cargo-leaderboard …` work. A failed Cargo command retains its exit code and is never submitted. Measurement or network failures produce a warning without turning a successful Cargo command into a failure. Submission configuration is checked before running Cargo. `--no-submit` requires no configuration. Help and clean dry-runs never submit events. Reports are best-effort; there is no offline retry queue.

## What counts

| Board | Score |
| --- | --- |
| Biggest builds | Total file bytes in Cargo's artifact and intermediate build directories after a successful build |
| Biggest cleans | Nonnegative difference between directory sizes before and after a successful clean |
| Longest waits | Wall-clock milliseconds spent running a successful `cargo build` |

The board keeps each nickname/project pair's highest score for each metric. Ties use the newest finish time, then event ID. Empty footprints and zero-byte cleans don't rank. Repeated event IDs are idempotent.

Measurements include accumulated dependencies, incremental caches, other profiles and, when shared, other projects. A release build's footprint can therefore include existing debug artifacts. The profile identifies the command, not the contents of the entire directory. This is a fun comparison, not a controlled benchmark.

Cargo metadata resolves workspaces, manifest paths, environment configuration, custom target directories and separate intermediate build directories. We don't follow symlinks within those directories. On Unix we count hard-linked files once per inode; on other platforms file paths are counted separately. Sizes are logical bytes, not allocated blocks or filesystem compression savings. File counts follow the same rule. Before/after subtraction can be distorted by concurrent builds, so avoid other writes while measuring. Time excludes metadata resolution, scanning and network submission. Build flags and CPU details are not collected.

## Privacy and trust

Submissions publish a nickname, project label, before/after byte counts, scored bytes, file count, command, duration, timestamps, profile, OS/architecture, and Cargo/client versions. No source code, remote URL credentials or local paths are uploaded. The default project label comes from `origin`'s owner/repository, then Cargo metadata, then the directory name. Use `--repo` to replace a private label, or `--no-submit` to keep all measurements local.

Nicknames and scores are **self-reported and unverified**. There are no accounts or ownership claims. Public submissions have validation, a 16 KiB body limit, UUID deduplication and a persistent limit of 30 submissions per IP per hour. The rate limiter stores an HMAC of the IP, not the raw IP, and removes stale buckets after two days when new submissions arrive. Hosting providers may retain their own request logs. Administrators can remove abusive events by UUID using SQL. A private installation can set `CARGO_LEADERBOARD_TOKEN` on the server and client to require a bearer token.

## Run locally

The Rust binary includes the entire website and a SQLite API. No Node or external database is required:

```sh
cargo run -- serve --bind 127.0.0.1:3000
# In a second terminal:
export CARGO_LEADERBOARD_API_URL="http://127.0.0.1:3000"
export CARGO_LEADERBOARD_NICKNAME="local"
cargo run -- build
```

Open http://127.0.0.1:3000. The server creates `leaderboard.db` on first launch and migrates databases from the original duration-only prototype. Legacy timing records remain visible in the API's timing board with unknown size. Use `--database-url` to select another SQLite database. `--auth-token` protects local submissions. The SQLite server is intended for local/self-hosted use and does not implement the hosted IP rate limiter.

For work on the Vercel handlers, use Node 22.16+ or 24 and the project database:

```sh
npm ci
npx vercel@latest link --project cargo-leaderboard --scope alexlwn123-s-team
npx vercel@latest env pull .env.local
npm run db:migrate
npm run dev
```

The development server loads `.env.local`; never commit it. Local SQLite and hosted Postgres are separate databases and do not sync. The static UI uses the same `/v1` contract with either backend.

## API

- `POST /v1/build-events`: accept one validated successful build or clean event. Hosted responses are `201` (created), `200` (duplicate), `400` (invalid event), `401` (private server auth), `413` (body too large), `415` (wrong content type), or `429` (rate limited). Server/storage failures return `503` on Vercel. SQLite returns `201` for duplicate IDs and uses its native error statuses.
- `GET /v1/leaderboard?metric=largest_build&limit=100`: public ranked results. Other metrics: `largest_clean`, `longest_single_build`. Limits are clamped to 1–200. Hosted defaults to size; the SQLite API retains its prototype default of build duration. The UI always selects an explicit metric. Hosted rankings may take up to 45 seconds to refresh because of CDN caching.

The event shape is defined in `src/types.rs` and validated by `web/contract.mjs` on Vercel. Measurements must be nonnegative safe integers, and scored bytes must match the before/after definition. Event UUIDs cannot be reused to overwrite scores.

## Deploy on Vercel

The Vercel project uses the **Other** preset: static files in `public/` and Node functions in `api/`. It does not run the persistent Rust/SQLite server. Neon Postgres is provisioned through Vercel Marketplace for durable submissions.

1. Link the project to your Vercel team and install the Neon integration.
2. Pull the generated environment variables. Set `RATE_LIMIT_SALT` to a random secret in production and preview. Optionally set `CARGO_LEADERBOARD_TOKEN` for a private board.
3. Run `npm run db:migrate` against the intended database before deployment. Migrations are explicit and idempotent, not run during every request or build. Use separate databases/Neon branches for isolated development and previews.
4. Run `npm test && npm run build`, then `npx vercel@latest deploy --prod`.
5. Set the public API URL in the CLI instructions if using another domain. Connect the GitHub repository in Vercel for automatic deployments from `main`.

## Checks

```sh
cargo fmt --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
npm test
npm run build
```

GitHub Actions runs the Rust and JavaScript checks. Rust integration tests exercise real builds and cleans in isolated temporary directories, custom artifact/intermediate directories, Cargo subcommand invocation, privacy overrides and network-failure behavior. API tests cover validation, authentication, ranking, legacy migration and duplicate submissions.

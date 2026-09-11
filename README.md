# Cargo Leaderboard

A heavyweight Rust competition: compare enormous build directories, satisfying cleans, and long waits for the compiler.

**[View the leaderboard](https://cargo-leaderboard.vercel.app)**

## Set up with your coding agent

Open your Rust project in an agent with terminal access and paste:

> Set up Cargo Leaderboard in this Rust project using https://cargo-leaderboard.vercel.app/agents.md, then run a build and verify the submission.

The [agent guide](https://cargo-leaderboard.vercel.app/agents.md) covers installation, GitHub browser approval, existing settings, and verifying a real submission. Installation comes first: the agent installs the CLI, the CLI starts browser approval, then the agent verifies the login and submission. The homepage starts with the agent prompt; manual setup is available below. Agents discovering the site can start at [llms.txt](https://cargo-leaderboard.vercel.app/llms.txt).

## Manual setup

Install [Rust](https://rustup.rs) if you don't already have it. Download the CLI without compiling it:

**macOS / Linux** (Apple Silicon or Intel; Linux ARM64 or x64):

```sh
curl -fsSL https://cargo-leaderboard.vercel.app/install.sh -o /tmp/cargo-leaderboard-install.sh &&
sh /tmp/cargo-leaderboard-install.sh
```

**Windows x64** (PowerShell):

```powershell
& {
  Invoke-WebRequest https://cargo-leaderboard.vercel.app/install.ps1 -OutFile "$env:TEMP\cargo-leaderboard-install.ps1" -ErrorAction Stop
  powershell -ExecutionPolicy Bypass -File "$env:TEMP\cargo-leaderboard-install.ps1"
}
```

You can inspect the downloaded script before running it. Installers download the latest [GitHub release](https://github.com/alexlwn123/cargo-leaderboard/releases/latest), verify its SHA-256 checksum, and install to `~/.cargo/bin` (or `$CARGO_HOME/bin`). They never require sudo or edit your shell profile. If that directory isn't in PATH, add it and reopen your terminal. Archives and `SHA256SUMS` are also available for manual installation. Linux binaries are statically linked with musl.

**Then, on any platform:**

```sh
cargo leaderboard login
# Sign in with GitHub and approve the code shown in your terminal.
cargo leaderboard doctor

# Run from your Rust project:
cargo leaderboard build
cargo leaderboard clean
```

For coding agents or SSH, use `cargo leaderboard login --no-browser`: open the printed URL yourself, sign in with GitHub, and approve the matching code. The CLI saves its own revocable token for future terminals. No repository scopes are requested. Existing nickname-only clients must update and log in; anonymous public submissions are no longer accepted.

**Build from source** with a current stable Rust toolchain and Git (also the fallback for unsupported platforms):

```sh
cargo install --git https://github.com/alexlwn123/cargo-leaderboard --tag v0.3.0 --locked
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

Both `cargo leaderboard …` and `cargo-leaderboard …` work. A failed Cargo command retains its exit code and is never submitted. Measurement or network failures produce a warning without turning a successful Cargo command into a failure. Submission configuration is checked before running Cargo. `--no-submit` requires no configuration. Help and clean dry-runs never submit events. Reports are best-effort; there is no offline retry queue.

## Configuration, updates, and troubleshooting

Run `cargo leaderboard login` to connect or switch your GitHub account. Add `--api-url https://your-board.example` for another authenticated board. Tokens expire after 90 days. `cargo leaderboard logout` revokes this machine's token; the [account page](https://cargo-leaderboard.vercel.app/account.html) can revoke all CLI access. Signing out of the website leaves CLI credentials active.

The standalone SQLite server retains nickname setup:

```sh
cargo leaderboard setup --nickname local --api-url http://127.0.0.1:3000
```

Settings and CLI credentials live at `$XDG_CONFIG_HOME/cargo-leaderboard/config.json` (otherwise `~/.config/cargo-leaderboard/config.json`) on macOS/Linux, or `%APPDATA%\cargo-leaderboard\config.json` on Windows. `CARGO_LEADERBOARD_CONFIG_DIR` overrides that directory. Files are replaced atomically and use owner-only permissions on Unix; Windows inherits the user's directory ACL. Keep this file private. Local nickname setup replaces saved login settings.

`CARGO_LEADERBOARD_API_URL` and `CARGO_LEADERBOARD_TOKEN` override saved values. Saved credentials are never forwarded to a different server. For CI, supply a Cargo Leaderboard CLI token through your CI secret store. `CARGO_LEADERBOARD_NICKNAME` still works for local SQLite boards; it cannot override your verified GitHub identity on the public server.

- **Update:** run the installer again. It preserves your settings and keeps the existing binary if a download or checksum fails. Stop running CLI/server processes first on Windows.
- **Pin a version:** pass `--version v0.3.0` to the shell installer or `-Version v0.3.0` to PowerShell. Choose another directory with `--bin-dir DIRECTORY` / `-BinDir DIRECTORY`.
- **Command not found:** check that the installer directory is in PATH and reopen the terminal. `cargo --version` should work too; `rustup show` diagnoses a missing toolchain.
- **Connection/configuration trouble:** `cargo leaderboard doctor` verifies the saved GitHub credential, server URL, Cargo, and connectivity without publishing anything. Quotas are checked when submitting. Unset old environment overrides if setup changes don't take effect.
- **Remove:** delete `cargo-leaderboard` (Windows: `cargo-leaderboard.exe`) from the install directory. Optionally delete the config file above. For source installations, use `cargo uninstall cargo-leaderboard`.

## Releasing the CLI

Update the package version and lockfile, run the checks below, then push a matching `vX.Y.Z` tag. `.github/workflows/release.yml` tests and builds on native macOS ARM64/Intel, Linux ARM64/x64, and Windows x64 runners, tests installation/upgrades/checksum rejection, and publishes archives with `SHA256SUMS` only after all five builds pass. Keep the source-install tag in this README and the website in sync with releases.

## What counts

| Board | Score |
| --- | --- |
| Biggest builds | Total file bytes in Cargo's artifact and intermediate build directories after a successful build |
| Biggest cleans | Nonnegative difference between directory sizes before and after a successful clean |
| Longest waits | Wall-clock milliseconds spent running a successful `cargo build` |

The board keeps each GitHub account/project pair's highest score for each metric. Ties use the newest finish time, then event ID. Empty footprints and zero-byte cleans don't rank. Repeated event IDs are idempotent.

Measurements include accumulated dependencies, incremental caches, other profiles and, when shared, other projects. A release build's footprint can therefore include existing debug artifacts. The profile identifies the command, not the contents of the entire directory. This is a fun comparison, not a controlled benchmark.

Cargo metadata resolves workspaces, manifest paths, environment configuration, custom target directories and separate intermediate build directories. We don't follow symlinks within those directories. On Unix we count hard-linked files once per inode; on other platforms file paths are counted separately. Sizes are logical bytes, not allocated blocks or filesystem compression savings. File counts follow the same rule. Before/after subtraction can be distorted by concurrent builds, so avoid other writes while measuring. Time excludes metadata resolution, scanning and network submission. Build flags and CPU details are not collected.

## Privacy and trust

Submissions publish a GitHub account, project label, before/after byte counts, scored bytes, file count, command, duration, timestamps, profile, OS/architecture, and Cargo/client versions. No source code, remote URL credentials or local paths are uploaded. The default project label comes from `origin`'s owner/repository, then Cargo metadata, then the directory name. Use `--repo` to replace a private label, or `--no-submit` to keep all measurements local.

The public server verifies identity through GitHub OAuth (state binding and PKCE), keys scores by GitHub's stable account ID, and ignores client-supplied nicknames. GitHub usernames update when users sign in again. Scores and repository ownership are still self-reported: authentication is not proof of a measurement or ownership of its project label.

Submissions require a signed, revocable CLI token and have validation, a 16 KiB body limit, UUID deduplication, and persistent limits of **30 requests per account per hour** and **120 per IP per hour**. Failed/repeated submission requests also consume quota. Login starts are limited to 30 per IP per hour; code approvals to 15 per account per hour. Device codes expire after ten minutes, are single-use, and poll at most every five seconds. Account limits are shared across tokens. Browser mutations require same-origin CSRF proof. Random or missing submission credentials are rejected before database access.

Only hashes of app credentials and IPs are stored. GitHub access tokens are used for a single public identity lookup, then discarded. Browser sessions expire after 30 days. Expired auth records and stale quota buckets are pruned when new logins start. GitHub authentication reduces anonymous write abuse; it does not eliminate denial of service, distributed account abuse, or fabricated measurements. Keep Vercel's edge protections enabled and add WAF rate limits if traffic warrants them. Hosting providers may retain request logs.

Pre-authentication events are retained with no account ID and excluded from the verified board. Never automatically claim them by matching a nickname. An administrator may attach specific, independently verified historical event IDs to an account; no public claiming endpoint exists.

## Run locally

The Rust binary includes the entire website and a SQLite API. No Node or external database is required:

```sh
cargo run -- serve --bind 127.0.0.1:3000
# In a second terminal:
export CARGO_LEADERBOARD_API_URL="http://127.0.0.1:3000"
export CARGO_LEADERBOARD_NICKNAME="local"
cargo run -- build
```

Open http://127.0.0.1:3000. The server creates `leaderboard.db` on first launch and migrates databases from the original duration-only prototype. Legacy timing records remain visible in the API's timing board with unknown size. Use `--database-url` to select another SQLite database. `--auth-token` protects local submissions. The SQLite server is intended for local/self-hosted use and does not implement GitHub authentication or the hosted rate limiter.

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

- `POST /v1/build-events`: accept one validated successful build or clean event. Hosted responses are `201` (created), `200` (duplicate), `400` (invalid event), `401` (missing, invalid, expired or revoked CLI login), `413` (body too large), `415` (wrong content type), or `429` (rate limited). Server/storage failures return `503` on Vercel. SQLite returns `201` for duplicate IDs and uses its native error statuses.
- `GET /v1/leaderboard?metric=largest_build&limit=100`: public ranked results. Other metrics: `largest_clean`, `longest_single_build`. Limits are clamped to 1–200. Hosted defaults to size; the SQLite API retains its prototype default of build duration. The UI always selects an explicit metric. Hosted rankings may take up to 45 seconds to refresh because of CDN caching.

The event shape is defined in `src/types.rs` and validated by `web/contract.mjs` on Vercel. Measurements must be nonnegative safe integers, and scored bytes must match the before/after definition. Event UUIDs cannot be reused to overwrite scores.

## Deploy on Vercel

The Vercel project uses the **Other** preset: static files in `public/` and Node functions in `api/`. It does not run the persistent Rust/SQLite server. Neon Postgres is provisioned through Vercel Marketplace for durable submissions.

1. Link the project to your Vercel team and install the Neon integration.
2. Pull the generated environment variables. Set `RATE_LIMIT_SALT` to a random secret in production and preview. Set `APP_ORIGIN`, `GITHUB_CLIENT_ID`, `GITHUB_CLIENT_SECRET`, and a random `AUTH_SECRET` of at least 32 characters (see `.env.example`). Register a GitHub OAuth app with callback `https://YOUR_DOMAIN/auth/callback` and homepage `https://YOUR_DOMAIN`. Request no scopes. Use separate credentials and origins for previews.
3. Run `npm run db:migrate` against the intended database before deployment. Migrations are explicit and idempotent, not run during every request or build. Use separate databases/Neon branches for isolated development and previews.
4. Run `npm test && npm run build`, then `npx vercel@latest deploy --prod`.
5. The website adds its own URL to the login command for self-hosted boards. Connect the GitHub repository in Vercel for automatic deployments from `main`.

## Checks

```sh
cargo fmt --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
npm test
npm run build
```

GitHub Actions runs the Rust and JavaScript checks. Rust integration tests exercise real builds and cleans in isolated temporary directories, custom artifact/intermediate directories, Cargo subcommand invocation, privacy overrides and network-failure behavior. API tests cover validation, CSRF, state replay, device code reuse/expiry, account identity, token revocation, and concurrent rate limiting. Set `TEST_DATABASE_URL` to an isolated disposable PostgreSQL database to run the full auth integration suite; it truncates only that test database. CI provisions PostgreSQL 17 and always runs it.

### Authentication endpoints

`GET /auth/login` starts GitHub sign-in; `/auth/callback` completes it. `GET /auth/session` returns the browser account and CSRF proof (never a CLI token). `POST /auth/device-start` returns the CLI's secret device code and public approval code. `POST /auth/device-poll` exchanges an approved device code once for a CLI credential. `POST /auth/device-approve` requires a browser session and CSRF proof. `GET /auth/me` validates a CLI bearer token. `POST /auth/cli-logout` revokes that token; `/auth/revoke-all` revokes all of the browser account's CLI tokens. `/auth/logout` ends only the browser session. All auth responses are uncached.

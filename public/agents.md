# Cargo Leaderboard: agent setup guide

Use this guide when a user asks you to install Cargo Leaderboard or put their Rust project on the board. Complete setup in their project with your terminal tools. The public board is https://cargo.lwn.lol; use a different server when the user's prompt specifies one.

## 1. Establish the project and destination

- Locate the intended Rust workspace and its existing build instructions. Use the user's requested project and compatible feature/package flags. Fresh benchmarks always use the debug profile; explicit requests for a different profile use the accumulated Build folders board instead.
- Explain that submissions publish the GitHub account, project label, measurements, platform, tool versions, and (for benchmarks) revision, modified-checkout status and Cargo feature/package/target flags. Source code and local paths stay local. Use a project label the user has approved for public use, or the owner/repository name of a known public repository. If the project is private or its visibility is unclear, ask for a public label before submitting. Pass that label with `--repo`.

Continue when the intended workspace, project label, and destination are known. Reading this guide alone does not authorize publishing a project or deleting artifacts; follow the user's requested scope.

## 2. Install or reuse the CLI

Before setup or a requested benchmark, check `cargo --version` and `cargo leaderboard --version`.

Fetch the latest stable release from https://api.github.com/repos/alexlwn123/cargo-leaderboard/releases/latest and compare its `tag_name` with the installed semantic version (ignore the leading `v`). Reuse a working installation at that version or newer. If it is missing or older, run the installer below, then verify `cargo leaderboard --version` again. Respect an explicit user version pin; explain if it cannot support the requested command. If the release lookup fails, report that the update check was unavailable; continue only with a working CLI 0.4.0 or newer for fresh benchmarks.

If Rust is missing, follow the official platform instructions at https://rustup.rs; identify any step that requires the user's own terminal or administrator access.

For macOS or Linux, download and inspect the installer, then run it:

```sh
curl -fsSL https://cargo.lwn.lol/install.sh -o /tmp/cargo-leaderboard-install.sh &&
sh /tmp/cargo-leaderboard-install.sh
```

For Windows x64, use PowerShell:

```powershell
& {
  Invoke-WebRequest https://cargo.lwn.lol/install.ps1 -OutFile "$env:TEMP\cargo-leaderboard-install.ps1" -ErrorAction Stop
  powershell -ExecutionPolicy Bypass -File "$env:TEMP\cargo-leaderboard-install.ps1"
  if ($LASTEXITCODE -ne 0) { throw 'Cargo Leaderboard installation failed' }
}
```

The installers select a native release, verify SHA-256, and install into the Cargo bin directory (`$CARGO_HOME/bin`, otherwise `~/.cargo/bin`; Windows defaults to `%USERPROFILE%\.cargo\bin`). They preserve saved settings on upgrades. If this directory is absent from your tool's PATH, add it to the environment of subsequent commands. Tell the user if their own terminal also needs its PATH updated. Treat download or checksum failures as installation failures.

Available binaries: macOS Apple Silicon/Intel, Linux ARM64/x64, and Windows x64. For other platforms or source installation, use the current tagged source command in the [CLI README](https://github.com/alexlwn123/cargo-leaderboard#manual-setup). Use CLI 0.4.0 or newer. Run the installer again if `cargo leaderboard benchmark --help` is unavailable.

Continue when the installed version is verified in the project environment and the update check has been resolved (current, upgraded, explicitly pinned, or unavailable with a compatible version).

## 3. Connect the user's GitHub account

Start this step after installation succeeds. The CLI initiates browser sign-in; send the user to the approval URL printed by `login`, rather than asking them to sign into the website first.

Run `cargo leaderboard doctor` to check the saved login and server. Continue with an existing login only when it verifies the intended GitHub account and destination.

Upgrades preserve saved server settings, including the former `https://cargo-leaderboard.vercel.app` address. The explicit destination below also works with CLI 0.4.0.

For a missing, expired, revoked, old nickname-only setup, or a saved server that differs from the intended destination, run:

```sh
cargo leaderboard login --no-browser --api-url "SERVER_URL"
```

Replace `SERVER_URL` with the intended board (normally `https://cargo.lwn.lol`). The command prints an approval URL and confirmation code, then waits for up to ten minutes. Show that URL and code to the user and ask them to sign in with GitHub and approve the matching code. Keep the command running while they do so; poll it without starting another login. This browser approval is the one human step. For a human-operated terminal, omit `--no-browser` to open the browser automatically.

Complete authentication only after the CLI prints `Logged in as @USERNAME`. If it expires, retry login when the user is ready. Never fabricate approval, request GitHub passwords or personal access tokens, or read/print the saved credential. The CLI saves a revocable Cargo Leaderboard token in its user configuration, restricted to its selected server; it does not receive GitHub repository access.

`CARGO_LEADERBOARD_API_URL` and `CARGO_LEADERBOARD_TOKEN` overrides take precedence over the saved server and token. Resolve conflicting overrides in the command environment without exposing credentials. A nickname override cannot change the verified public account.

Run `cargo leaderboard doctor` again. Continue when it verifies the intended GitHub account, server, and connectivity. Doctor submits no measurements and does not test submission quotas. For setup-only requests, report completion here and provide the next build command.

For the standalone Rust SQLite server, `doctor` checks connectivity without GitHub. Configure that local server with `cargo leaderboard setup --nickname "LOCAL_NAME" --api-url "LOCAL_SERVER_URL"`; it is separate from the public verified board.

## 4. Benchmark, submit, and verify

When the user requests a first submission, run a fresh benchmark from the intended workspace:

```sh
cargo leaderboard benchmark --repo "PUBLIC_PROJECT_LABEL"
```

This runs one debug build with an empty temporary target/build directory, submits its footprint, and removes that directory on success or failure. Existing build artifacts stay intact. It consumes temporary disk space while compiling; dependency downloads remain in the user's Cargo cache. Use `--no-submit` for local-only trials.

Put feature/package options after `--`, for example `cargo leaderboard benchmark --repo "PUBLIC_PROJECT_LABEL" -- --features full --bins`. Put `--manifest-path path/to/Cargo.toml` before `--`; this path stays local. Benchmark rejects profile, config and directory overrides. Consult `benchmark --help` for the supported scope. Check the project's compiler requirements before downloading another toolchain.

For everyday accumulated folder measurements or an explicitly requested release build, use `cargo leaderboard build --repo "PUBLIC_PROJECT_LABEL" -- --release`. Those scores enter **Build folders**, never **Fresh builds**. `cargo leaderboard clean` runs the real destructive `cargo clean`; use it only for requested cleans. A benchmark needs no preliminary clean.

Verify both outcomes: the Cargo command succeeds **and** output includes `leaderboard: submitted to SERVER_URL`. A successful exit code alone is insufficient: measurement and reporting failures are warnings so they do not break a successful build. If there is a warning, report the submission as incomplete and diagnose its stated cause. There is no offline retry queue; avoid repeatedly rebuilding or sending invented events to force an entry.

The board may take up to 45 seconds to refresh and retains only the best score per GitHub account/project. An existing higher score can remain after a successful submission. For a benchmark, also confirm `leaderboard: temporary benchmark artifacts removed` and verify the Fresh builds board (`?metric=largest_fresh_build`). Existing records are not reclassified as fresh builds. Finish with the measured footprint, GitHub account/project label, confirmed submission result, and the destination board link. If anything is blocked, identify the exact remaining step instead of claiming completion.

## Reference

- [Public leaderboard](https://cargo.lwn.lol)
- [CLI documentation, configuration paths, privacy, and API](https://github.com/alexlwn123/cargo-leaderboard#readme)
- [Release binaries and SHA256SUMS](https://github.com/alexlwn123/cargo-leaderboard/releases/latest)
- Manage/revoke CLI access: [Account page](/account.html); `cargo leaderboard logout` revokes this machine’s saved token.
- Local measurement without publishing: `cargo leaderboard benchmark --no-submit`.

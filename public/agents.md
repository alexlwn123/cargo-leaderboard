# Cargo Leaderboard: agent setup guide

Use this guide when a user asks you to install Cargo Leaderboard or put their Rust project on the board. Complete setup in their project with your terminal tools. The public board is https://cargo-leaderboard.vercel.app; use a different server when the user's prompt specifies one.

## 1. Establish the project and destination

- Locate the intended Rust workspace and its existing build instructions. Use the user's requested project, profile, and flags.
- Explain that submissions publish the GitHub account, project label, measurements, platform, and tool versions. Source code and local paths stay local. Use a project label the user has approved for public use, or the owner/repository name of a known public repository. If the project is private or its visibility is unclear, ask for a public label before submitting. Pass that label with `--repo`.

Continue when the intended workspace, project label, and destination are known. Reading this guide alone does not authorize publishing a project or deleting artifacts; follow the user's requested scope.

## 2. Install or reuse the CLI

Check `cargo --version` and `cargo leaderboard --version`. Reuse a working current installation. If Rust is missing, follow the official platform instructions at https://rustup.rs; identify any step that requires the user's own terminal or administrator access.

For macOS or Linux, download and inspect the installer, then run it:

```sh
curl -fsSL https://cargo-leaderboard.vercel.app/install.sh -o /tmp/cargo-leaderboard-install.sh &&
sh /tmp/cargo-leaderboard-install.sh
```

For Windows x64, use PowerShell:

```powershell
& {
  Invoke-WebRequest https://cargo-leaderboard.vercel.app/install.ps1 -OutFile "$env:TEMP\cargo-leaderboard-install.ps1" -ErrorAction Stop
  powershell -ExecutionPolicy Bypass -File "$env:TEMP\cargo-leaderboard-install.ps1"
  if ($LASTEXITCODE -ne 0) { throw 'Cargo Leaderboard installation failed' }
}
```

The installers select a native release, verify SHA-256, and install into the Cargo bin directory (`$CARGO_HOME/bin`, otherwise `~/.cargo/bin`; Windows defaults to `%USERPROFILE%\.cargo\bin`). They preserve saved settings on upgrades. If this directory is absent from your tool's PATH, add it to the environment of subsequent commands. Tell the user if their own terminal also needs its PATH updated. Treat download or checksum failures as installation failures.

Available binaries: macOS Apple Silicon/Intel, Linux ARM64/x64, and Windows x64. For other platforms or source installation, use the current tagged source command in the [CLI README](https://github.com/alexlwn123/cargo-leaderboard#manual-setup). Use CLI 0.3.0 or newer. Run the installer again if `cargo leaderboard login --help` is unavailable.

Continue when `cargo leaderboard --version` runs successfully in the environment you will use for the project.

## 3. Connect the user's GitHub account

Run `cargo leaderboard doctor` to check the saved login and server. Continue with an existing login only when it verifies the intended GitHub account and destination.

For a missing, expired, revoked, or old nickname-only setup, run:

```sh
cargo leaderboard login --no-browser --api-url "SERVER_URL"
```

Replace `SERVER_URL` with the intended board (normally `https://cargo-leaderboard.vercel.app`). The command prints an approval URL and confirmation code, then waits for up to ten minutes. Show that URL and code to the user and ask them to sign in with GitHub and approve the matching code. Keep the command running while they do so; poll it without starting another login. This browser approval is the one human step. For a human-operated terminal, omit `--no-browser` to open the browser automatically.

Complete authentication only after the CLI prints `Logged in as @USERNAME`. If it expires, retry login when the user is ready. Never fabricate approval, request GitHub passwords or personal access tokens, or read/print the saved credential. The CLI saves a revocable Cargo Leaderboard token in its user configuration, restricted to its selected server; it does not receive GitHub repository access.

`CARGO_LEADERBOARD_API_URL` and `CARGO_LEADERBOARD_TOKEN` overrides take precedence over the saved server and token. Resolve conflicting overrides in the command environment without exposing credentials. A nickname override cannot change the verified public account.

Run `cargo leaderboard doctor` again. Continue when it verifies the intended GitHub account, server, and connectivity. Doctor submits no measurements and does not test submission quotas. For setup-only requests, report completion here and provide the next build command.

For the standalone Rust SQLite server, `doctor` checks connectivity without GitHub. Configure that local server with `cargo leaderboard setup --nickname "LOCAL_NAME" --api-url "LOCAL_SERVER_URL"`; it is separate from the public verified board.

## 4. Build, submit, and verify

When the user has requested a first submission, run a build from the intended workspace:

```sh
cargo leaderboard build --repo "PUBLIC_PROJECT_LABEL"
```

For requested Cargo options, keep wrapper options before `--`:

```sh
cargo leaderboard build --repo "PUBLIC_PROJECT_LABEL" -- --release
```

Use the project's normal build profile unless the user asked for another. `cargo leaderboard clean` runs the real destructive `cargo clean`; run it only when the user explicitly requests a clean. A first submission needs only a build.

Verify both outcomes: the Cargo command succeeds **and** output includes `leaderboard: submitted to SERVER_URL`. A successful exit code alone is insufficient: measurement and reporting failures are warnings so they do not break a successful build. If there is a warning, report the submission as incomplete and diagnose its stated cause. There is no offline retry queue; avoid repeatedly rebuilding or sending invented events to force an entry.

The board may take up to 45 seconds to refresh and retains only the best score per GitHub account/project. An existing higher score can remain after a successful submission. Finish with the measured footprint, GitHub account/project label, confirmed submission result, and the destination board link. If anything is blocked, identify the exact remaining step instead of claiming completion.

## Reference

- [Public leaderboard](https://cargo-leaderboard.vercel.app)
- [CLI documentation, configuration paths, privacy, and API](https://github.com/alexlwn123/cargo-leaderboard#readme)
- [Release binaries and SHA256SUMS](https://github.com/alexlwn123/cargo-leaderboard/releases/latest)
- Manage/revoke CLI access: [Account page](/account.html); `cargo leaderboard logout` revokes this machine’s saved token.
- Local measurement without publishing: `cargo leaderboard build --no-submit`.

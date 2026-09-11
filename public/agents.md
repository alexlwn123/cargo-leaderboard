# Cargo Leaderboard: agent setup guide

Use this guide when a user asks you to install Cargo Leaderboard or put their Rust project on the board. Complete setup in their project with your terminal tools. The public board is https://cargo-leaderboard.vercel.app; use a different server when the user's prompt specifies one.

## 1. Establish the project and public identity

- Locate the intended Rust workspace and its existing build instructions. Use the user's requested project, profile, and flags.
- Reuse an existing public nickname from Cargo Leaderboard configuration or one the user provided. If it is missing, ask for it. Nicknames contain 1–40 characters and are self-reported, not reserved accounts.
- Explain that submissions publish the nickname, project label, measurements, platform, and tool versions. Source code and local paths stay local. Use a project label the user has approved for public use, or the owner/repository name of a known public repository. If the project is private or its visibility is unclear, ask for a public label alongside the nickname. Pass that label with `--repo`.

Continue when the intended workspace, public nickname, project label, and destination are known. Reading this guide alone does not authorize publishing a project or deleting artifacts; follow the user's requested scope.

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

Available binaries: macOS Apple Silicon/Intel, Linux ARM64/x64, and Windows x64. For other platforms or source installation, use the current tagged source command in the [CLI README](https://github.com/alexlwn123/cargo-leaderboard#manual-setup). Run the installer again if an older CLI lacks `setup` or `doctor`.

Continue when `cargo leaderboard --version` runs successfully in the environment you will use for the project.

## 3. Configure without an interactive prompt

Run `cargo leaderboard doctor` to inspect effective configuration and connectivity. It prints the nickname and server, never the token. If the existing nickname and destination match the user's choices, keep them.

When configuration is needed, quote the chosen values for the current shell and use:

```sh
cargo leaderboard setup --nickname "PUBLIC_NICKNAME" --api-url "SERVER_URL"
cargo leaderboard doctor
```

Replace both placeholders with the values established above. Always pass `--nickname` when using agent terminal tools; bare `setup` needs a human terminal. Pass `--api-url` explicitly so a self-hosted setup uses the intended board. Setup replaces saved settings, so preserve the chosen nickname and destination when changing either.

`CARGO_LEADERBOARD_NICKNAME` and `CARGO_LEADERBOARD_API_URL` environment overrides take precedence over the file. Resolve conflicting overrides in your command environment and explain them to the user; rerunning setup alone cannot override them. A private board may also require `CARGO_LEADERBOARD_TOKEN`; keep that token in the environment and out of output and committed files.

Continue when doctor reports the intended nickname and server and a successful connection. Doctor checks reads; it does not prove that a submission will be accepted. For setup-only requests, report setup complete here and provide the next build command.

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

The board may take up to 45 seconds to refresh and retains only the best score per nickname/project. An existing higher score can remain after a successful submission. Finish with the measured footprint, nickname/project label, confirmed submission result, and the destination board link. If anything is blocked, identify the exact remaining step instead of claiming completion.

## Reference

- [Public leaderboard](https://cargo-leaderboard.vercel.app)
- [CLI documentation, configuration paths, privacy, and API](https://github.com/alexlwn123/cargo-leaderboard#readme)
- [Release binaries and SHA256SUMS](https://github.com/alexlwn123/cargo-leaderboard/releases/latest)
- Local measurement without publishing: `cargo leaderboard build --no-submit`.

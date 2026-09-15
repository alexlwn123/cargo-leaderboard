use std::path::PathBuf;

use anyhow::{Context, Result, bail, ensure};
use tokio::process::Command;

use crate::build::run_measured_command;
use crate::measurement::project;
use crate::types::BenchmarkMetadata;

// Keep this allowlist in sync with web/contract.mjs. In particular, callers cannot
// redirect output into an existing directory or change the debug profile.
pub(crate) fn validate_args(args: &[String]) -> Result<()> {
    ensure!(args.len() <= 64, "too many benchmark arguments");
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        ensure!(
            arg.len() <= 200 && !arg.chars().any(char::is_control),
            "invalid benchmark argument"
        );
        let (key, inline) = arg
            .split_once('=')
            .map_or((arg.as_str(), None), |(k, v)| (k, Some(v)));
        if [
            "--all-features",
            "--no-default-features",
            "--workspace",
            "--bins",
            "--lib",
            "--locked",
            "--offline",
            "--frozen",
        ]
        .contains(&key)
            && inline.is_none()
        {
            continue;
        }
        ensure!(
            [
                "--features",
                "-F",
                "--package",
                "-p",
                "--exclude",
                "--target",
                "--jobs",
                "-j"
            ]
            .contains(&key),
            "unsupported benchmark option {key}; use feature/package/target/jobs flags, with a fixed debug profile"
        );
        let value = inline
            .or_else(|| args.next().map(String::as_str))
            .context("missing benchmark option value")?;
        ensure!(
            !value.is_empty()
                && value.len() <= 200
                && !value.starts_with('-')
                && !value.chars().any(char::is_control),
            "invalid benchmark option value"
        );
        if key == "--target" {
            ensure!(
                !value.contains(['/', '\\', '.']),
                "benchmark target must be a target triple, not a local JSON path"
            );
        }
    }
    Ok(())
}

pub async fn run(
    cargo_args: Vec<String>,
    manifest: Option<PathBuf>,
    no_submit: bool,
    repo: Option<String>,
) -> Result<u8> {
    validate_args(&cargo_args)?;
    let temp = tempfile::Builder::new()
        .prefix("cargo-leaderboard-benchmark-")
        .tempdir()?;
    let root = temp.path().canonicalize()?;
    let target = root.join("target");
    let mut args = cargo_args.clone();
    if let Some(manifest) = manifest {
        args.extend([
            "--manifest-path".into(),
            manifest.to_string_lossy().into_owned(),
        ]);
    }
    // Command-line overrides take precedence over workspace and environment config.
    // Both final and intermediate artifacts live inside the owned temporary root.
    args.extend([
        "--profile".into(),
        "dev".into(),
        "--target-dir".into(),
        target.to_string_lossy().into_owned(),
        "--config".into(),
        format!(
            "build.build-dir={}",
            serde_json::to_string(&target.to_string_lossy())?
        ),
    ]);
    let resolved = project(&args).await?;
    ensure!(
        resolved.roots.iter().all(|p| p.starts_with(&root)),
        "Cargo resolved a build directory outside the benchmark directory"
    );
    let rustc = Command::new("rustc").arg("--version").output().await?;
    ensure!(rustc.status.success(), "could not determine Rust version");
    let revision = git(&resolved.workspace, &["rev-parse", "HEAD"]).await;
    let dirty = git(&resolved.workspace, &["status", "--porcelain"])
        .await
        .map(|s| !s.is_empty());
    let metadata = BenchmarkMetadata {
        rustc_version: String::from_utf8(rustc.stdout)?.trim().into(),
        revision,
        dirty,
        cargo_args,
    };
    eprintln!(
        "leaderboard: fresh debug build in {}; temporary artifacts will be removed",
        root.display()
    );
    let result = run_measured_command("build", args, no_submit, repo, Some(metadata)).await;
    // Explicit close reports cleanup errors; RAII also handles earlier failures.
    if let Err(error) = temp.close() {
        bail!(
            "could not remove benchmark directory {}: {error}",
            root.display()
        );
    }
    eprintln!("leaderboard: temporary benchmark artifacts removed");
    result
}

async fn git(workspace: &std::path::Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(workspace)
        .args(args)
        .output()
        .await
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

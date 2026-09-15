use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use tokio::process::Command;

#[derive(Debug, Default, Clone, Copy)]
pub struct Footprint {
    pub bytes: i64,
    pub files: i64,
}

#[derive(Deserialize)]
struct Metadata {
    target_directory: PathBuf,
    build_directory: Option<PathBuf>,
    workspace_root: PathBuf,
}

pub struct Project {
    pub roots: Vec<PathBuf>,
    pub workspace: PathBuf,
}

/// Ask Cargo to resolve configuration so workspace, environment and config paths agree.
pub async fn project(args: &[String]) -> Result<Project> {
    let mut command = Command::new("cargo");
    command.args(["metadata", "--no-deps", "--format-version", "1"]);
    let mut target_dir = None;
    let mut index = 0;
    while index < args.len() {
        let arg = &args[index];
        let key = arg.split('=').next().unwrap_or(arg);
        if ["--manifest-path", "--config", "-Z", "--target-dir"].contains(&key) {
            let value = if let Some((_, value)) = arg.split_once('=') {
                value.to_owned()
            } else {
                index += 1;
                args.get(index)
                    .context("missing Cargo option value")?
                    .clone()
            };
            if key == "--target-dir" {
                target_dir = Some(value);
            } else {
                command.arg(key).arg(value);
            }
        } else if ["--locked", "--offline", "--frozen"].contains(&key)
            || (arg.starts_with("-Z") && arg.len() > 2)
        {
            command.arg(arg);
        }
        index += 1;
    }
    // metadata has no --target-dir flag. CARGO_TARGET_DIR is special: it also
    // overrides --config, so replace it when emulating an explicit CLI path.
    if let Some(directory) = target_dir {
        command.env("CARGO_TARGET_DIR", &directory);
        command.arg("--config").arg(format!(
            "build.target-dir={}",
            serde_json::to_string(&directory)?
        ));
    }
    let output = command
        .output()
        .await
        .context("failed to run cargo metadata")?;
    if !output.status.success() {
        bail!(
            "cannot resolve Cargo directories: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let metadata: Metadata = serde_json::from_slice(&output.stdout)?;
    let mut roots = vec![metadata.target_directory];
    if let Some(build) = metadata.build_directory {
        roots.push(build);
    }
    // Canonicalize existing roots, then discard duplicates and nested roots.
    let roots: Vec<_> = roots
        .into_iter()
        .map(|p| fs::canonicalize(&p).unwrap_or(p))
        .collect();
    let roots = roots
        .iter()
        .enumerate()
        .filter(|(i, path)| {
            !roots.iter().enumerate().any(|(j, parent)| {
                i != &j && path.starts_with(parent) && (path != &parent || j < *i)
            })
        })
        .map(|(_, p)| p.clone())
        .collect();
    Ok(Project {
        roots,
        workspace: metadata.workspace_root,
    })
}

pub fn measure(roots: &[PathBuf]) -> Result<Footprint> {
    let mut result = Footprint::default();
    let mut pending = roots.to_vec();
    let mut seen = HashSet::new();
    while let Some(path) = pending.pop() {
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                return Err(error).with_context(|| format!("cannot measure {}", path.display()));
            }
        };
        if metadata.is_symlink() {
            continue;
        }
        if metadata.is_dir() {
            for entry in
                fs::read_dir(&path).with_context(|| format!("cannot read {}", path.display()))?
            {
                pending.push(entry?.path());
            }
        } else if metadata.is_file() && seen.insert(file_identity(&path, &metadata)) {
            result.bytes = result
                .bytes
                .checked_add(i64::try_from(metadata.len())?)
                .context("footprint overflow")?;
            result.files += 1;
        }
    }
    Ok(result)
}

#[cfg(unix)]
fn file_identity(_: &Path, metadata: &fs::Metadata) -> (u64, u64) {
    use std::os::unix::fs::MetadataExt;
    (metadata.dev(), metadata.ino())
}
#[cfg(not(unix))]
fn file_identity(path: &Path, _: &fs::Metadata) -> PathBuf {
    path.to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn measures_files_and_missing_roots() -> Result<()> {
        let temp = tempfile::tempdir()?;
        fs::create_dir(temp.path().join("nested"))?;
        fs::write(temp.path().join("a"), b"12345")?;
        fs::write(temp.path().join("nested/b"), b"123")?;
        let result = measure(&[temp.path().to_owned(), temp.path().join("missing")])?;
        assert_eq!((result.bytes, result.files), (8, 2));
        Ok(())
    }
    #[cfg(unix)]
    #[test]
    fn does_not_follow_symlinks_or_double_count_hardlinks() -> Result<()> {
        let temp = tempfile::tempdir()?;
        fs::write(temp.path().join("a"), b"12345")?;
        fs::hard_link(temp.path().join("a"), temp.path().join("b"))?;
        std::os::unix::fs::symlink(temp.path(), temp.path().join("loop"))?;
        let result = measure(&[temp.path().to_owned()])?;
        assert_eq!((result.bytes, result.files), (5, 1));
        Ok(())
    }
}

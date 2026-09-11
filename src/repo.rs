use std::path::Path;
use std::process::Command;

use cargo_metadata::MetadataCommand;
use url::Url;

pub fn resolve_repo_slug(cwd: &Path) -> String {
    git_remote_slug(cwd)
        .or_else(|| cargo_metadata_slug(cwd))
        .unwrap_or_else(|| {
            cwd.file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("unknown")
                .to_owned()
        })
}

fn git_remote_slug(cwd: &Path) -> Option<String> {
    let output = Command::new("git")
        .arg("remote")
        .arg("get-url")
        .arg("origin")
        .current_dir(cwd)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    parse_remote_slug(String::from_utf8_lossy(&output.stdout).trim())
}

fn cargo_metadata_slug(cwd: &Path) -> Option<String> {
    let metadata = MetadataCommand::new()
        .current_dir(cwd)
        .no_deps()
        .exec()
        .ok()?;

    if let Some(root) = metadata.root_package() {
        return Some(root.name.to_string());
    }

    metadata
        .workspace_root
        .file_name()
        .map(|name| name.to_string())
}

fn parse_remote_slug(remote: &str) -> Option<String> {
    if remote.is_empty() {
        return None;
    }

    if let Ok(url) = Url::parse(remote) {
        return path_to_slug(url.path());
    }

    let (_, path) = remote.split_once(':')?;
    path_to_slug(path)
}

fn path_to_slug(path: &str) -> Option<String> {
    let cleaned = path.trim_start_matches('/').trim_end_matches(".git");
    let mut segments = cleaned.split('/').filter(|segment| !segment.is_empty());
    let owner = segments.next()?;
    let repo = segments.next_back().unwrap_or(owner);
    Some(format!("{owner}/{repo}"))
}

#[cfg(test)]
mod tests {
    use super::parse_remote_slug;

    #[test]
    fn parses_https_remote() {
        assert_eq!(
            parse_remote_slug("https://github.com/acme/widget.git"),
            Some("acme/widget".to_string())
        );
    }

    #[test]
    fn parses_ssh_remote() {
        assert_eq!(
            parse_remote_slug("git@github.com:acme/widget.git"),
            Some("acme/widget".to_string())
        );
    }
}

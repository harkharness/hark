//! Working-tree status of configured repositories via the `git` CLI.

use crate::domain::prompt::RepoStatus;
use crate::ports::RepoCollector;

/// Pure assembly from raw command outputs.
pub fn status_from_raw(path: &str, branch: &str, porcelain: &str, log: &str) -> RepoStatus {
    RepoStatus {
        path: path.to_string(),
        branch: branch.trim().to_string(),
        dirty_files: porcelain.lines().filter(|l| !l.trim().is_empty()).count(),
        recent_commits: log.lines().map(|l| l.trim().to_string()).collect(),
    }
}

pub struct GitCli;

impl GitCli {
    fn git(path: &str, args: &[&str]) -> Option<String> {
        let out = std::process::Command::new("git")
            .arg("-C")
            .arg(path)
            .args(args)
            .output()
            .ok()?;
        out.status
            .success()
            .then(|| String::from_utf8_lossy(&out.stdout).to_string())
    }
}

impl RepoCollector for GitCli {
    /// Unreachable repos are silently skipped; the snapshot just has less data.
    fn collect(&self, repos: &[String]) -> Vec<RepoStatus> {
        repos
            .iter()
            .filter_map(|path| {
                let branch = Self::git(path, &["branch", "--show-current"])?;
                let porcelain = Self::git(path, &["status", "--porcelain"])?;
                let log = Self::git(path, &["log", "--oneline", "-3"])?;
                Some(status_from_raw(path, &branch, &porcelain, &log))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assembles_status_from_raw_outputs() {
        let status = status_from_raw(
            "/home/dev/proj",
            "feat/webhook\n",
            " M src/main.rs\n?? new.txt\n",
            "abc123 feat: add handler\ndef456 fix: typo\n",
        );
        assert_eq!(status.branch, "feat/webhook");
        assert_eq!(status.dirty_files, 2);
        assert_eq!(status.recent_commits.len(), 2);
        assert_eq!(status.recent_commits[0], "abc123 feat: add handler");
    }
}

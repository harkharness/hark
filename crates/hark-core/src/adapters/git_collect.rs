//! Working-tree status of configured repositories via the `git` CLI.

use crate::domain::prompt::RepoStatus;
use crate::domain::repo::{FileChange, RepoState, parse_numstat, parse_shortstat, parse_status_v2, untracked_v2};
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

impl GitCli {
    /// The state of the repository a directory belongs to, for the UI.
    ///
    /// Resolves the ROOT first: a task's workspace is often a subdirectory
    /// of the project (or of the repo), and reporting the branch of "the
    /// project path" would quietly describe the wrong tree — or nothing,
    /// when the path is not a repository at all.
    pub fn work_tree(dir: &str) -> Option<RepoState> {
        let root = Self::git(dir, &["rev-parse", "--show-toplevel"])?;
        let root = root.trim();
        if root.is_empty() {
            return None;
        }
        let status = Self::git(root, &["status", "--porcelain=v2", "--branch"])?;
        let mut state = parse_status_v2(&status);
        state.root = root.to_string();
        // A repository with no commits yet has no HEAD to diff against;
        // the counts stay zero rather than the whole state going missing.
        if let Some(stat) = Self::git(root, &["diff", "--shortstat", "HEAD"]) {
            let (added, removed) = parse_shortstat(&stat);
            state.added = added;
            state.removed = removed;
        }
        Some(state)
    }
}

impl GitCli {
    /// Every changed file of the repository `dir` belongs to: the diff
    /// against HEAD plus the untracked files the status lists. Returns the
    /// root too, so the caller can key what it caches by it.
    pub fn changes(dir: &str) -> Option<(String, Vec<FileChange>)> {
        let root = Self::git(dir, &["rev-parse", "--show-toplevel"])?;
        let root = root.trim().to_string();
        if root.is_empty() {
            return None;
        }
        let mut changes = Self::git(&root, &["diff", "--numstat", "HEAD"]).map(|o| parse_numstat(&o)).unwrap_or_default();
        if let Some(status) = Self::git(&root, &["status", "--porcelain=v2", "--branch"]) {
            for path in untracked_v2(&status) {
                changes.push(FileChange { path, added: 0, removed: 0, untracked: true, binary: false });
            }
        }
        Some((root, changes))
    }

    /// The unified diff of one file against HEAD; an untracked file reads
    /// whole as additions (`git diff --no-index` exits 1 when the files
    /// differ, which here is the point, so its status is not an error).
    pub fn diff_file(root: &str, path: &str, untracked: bool) -> Option<String> {
        if untracked {
            let out = std::process::Command::new("git")
                .arg("-C")
                .arg(root)
                .args(["diff", "--no-index", "--", "/dev/null", path])
                .output()
                .ok()?;
            return Some(String::from_utf8_lossy(&out.stdout).to_string());
        }
        Self::git(root, &["diff", "HEAD", "--", path])
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

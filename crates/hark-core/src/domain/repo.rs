//! What a working tree looks like from the outside: which branch, how far
//! from its upstream, how much is uncommitted. Pure parsing of `git`
//! output — the shell that runs the commands lives in the adapter.

use serde::{Deserialize, Serialize};

/// The state of one repository, as a card or a chip needs it.
///
/// Serializable on purpose: unlike `RepoStatus` (which only ever reached
/// the prompt) this crosses the IPC boundary to the UI.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct RepoState {
    /// Absolute path of the repository ROOT, which is often an ancestor of
    /// the task's own workspace — a task frequently runs in a subdirectory.
    pub root: String,
    pub branch: String,
    /// `origin/main`, when the branch tracks anything.
    pub upstream: Option<String>,
    /// Commits the branch has that its upstream does not, and vice versa.
    pub ahead: u32,
    pub behind: u32,
    /// Files with any change at all — staged, unstaged or untracked.
    pub dirty: u32,
    /// Lines against HEAD, when git could count them.
    pub added: u32,
    pub removed: u32,
}

impl RepoState {
    /// Nothing to commit and nothing to push: the quiet state.
    pub fn clean(&self) -> bool {
        self.dirty == 0 && self.ahead == 0
    }
}

/// Parse `git status --porcelain=v2 --branch`: one call carries the branch,
/// the upstream, the ahead/behind pair and every changed file.
pub fn parse_status_v2(out: &str) -> RepoState {
    let mut state = RepoState::default();
    for line in out.lines() {
        if let Some(head) = line.strip_prefix("# branch.head ") {
            // A detached HEAD reports the literal "(detached)".
            state.branch = head.trim().to_string();
        } else if let Some(up) = line.strip_prefix("# branch.upstream ") {
            state.upstream = Some(up.trim().to_string());
        } else if let Some(ab) = line.strip_prefix("# branch.ab ") {
            for part in ab.split_whitespace() {
                let (sign, n) = part.split_at(1);
                let n: u32 = n.parse().unwrap_or(0);
                match sign {
                    "+" => state.ahead = n,
                    "-" => state.behind = n,
                    _ => {}
                }
            }
        } else if line.starts_with("1 ")
            || line.starts_with("2 ")
            || line.starts_with("u ")
            || line.starts_with("? ")
        {
            // 1 = changed, 2 = renamed/copied, u = unmerged, ? = untracked.
            state.dirty += 1;
        }
    }
    state
}

/// Parse `git diff --shortstat HEAD`:
/// " 12 files changed, 345 insertions(+), 67 deletions(-)". Either half can
/// be missing when a change is only additions or only deletions.
pub fn parse_shortstat(out: &str) -> (u32, u32) {
    let mut added = 0;
    let mut removed = 0;
    for chunk in out.trim().split(',') {
        let mut words = chunk.split_whitespace();
        let n: u32 = words.next().and_then(|w| w.parse().ok()).unwrap_or(0);
        match words.next() {
            Some(w) if w.starts_with("insertion") => added = n,
            Some(w) if w.starts_with("deletion") => removed = n,
            _ => {}
        }
    }
    (added, removed)
}

/// One file of the working tree's diff against HEAD, as the panel lists
/// it. Untracked files are changes too: git's diff does not list them,
/// the status does.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileChange {
    pub path: String,
    pub added: u32,
    pub removed: u32,
    pub untracked: bool,
    /// Git could not count lines (`-\t-`): an image, a binary.
    pub binary: bool,
}

/// Parse `git diff --numstat HEAD`: `added\tremoved\tpath` per line, `-`
/// for a binary, `{old => new}` inside a renamed path (kept as the new
/// name).
pub fn parse_numstat(out: &str) -> Vec<FileChange> {
    out.lines()
        .filter_map(|line| {
            let mut parts = line.splitn(3, '\t');
            let a = parts.next()?.trim();
            let r = parts.next()?.trim();
            let path = parts.next()?.trim();
            if path.is_empty() {
                return None;
            }
            let binary = a == "-" || r == "-";
            Some(FileChange {
                path: rename_target(path),
                added: a.parse().unwrap_or(0),
                removed: r.parse().unwrap_or(0),
                untracked: false,
                binary,
            })
        })
        .collect()
}

/// `src/{old => new}/a.ts` → `src/new/a.ts`; `old => new` → `new`.
fn rename_target(path: &str) -> String {
    if let (Some(open), Some(close)) = (path.find('{'), path.find('}')) {
        if open < close {
            let inner = &path[open + 1..close];
            if let Some((_, new)) = inner.split_once(" => ") {
                return format!("{}{}{}", &path[..open], new, &path[close + 1..]);
            }
        }
    }
    path.split_once(" => ").map(|(_, new)| new.to_string()).unwrap_or_else(|| path.to_string())
}

/// The untracked files of `git status --porcelain=v2`: the `? path` lines.
pub fn untracked_v2(out: &str) -> Vec<String> {
    out.lines().filter_map(|l| l.strip_prefix("? ").map(|p| p.trim().to_string())).collect()
}

/// Lines added and removed over every change.
pub fn totals(changes: &[FileChange]) -> (u32, u32) {
    changes.iter().fold((0, 0), |(a, r), c| (a + c.added, r + c.removed))
}

#[cfg(test)]
mod tests {
    use super::*;

    const FULL: &str = "# branch.oid 0f3c9a1\n\
        # branch.head feat/assinaturas/hangfire-uat-apps\n\
        # branch.upstream origin/feat/assinaturas/hangfire-uat-apps\n\
        # branch.ab +3 -1\n\
        1 .M N... 100644 100644 100644 aaa bbb src/main.rs\n\
        1 M. N... 100644 100644 100644 ccc ddd README.md\n\
        ? notes.txt\n";

    #[test]
    fn reads_branch_upstream_and_distance_from_one_call() {
        let state = parse_status_v2(FULL);
        assert_eq!(state.branch, "feat/assinaturas/hangfire-uat-apps");
        assert_eq!(state.upstream.as_deref(), Some("origin/feat/assinaturas/hangfire-uat-apps"));
        assert_eq!(state.ahead, 3);
        assert_eq!(state.behind, 1);
    }

    #[test]
    fn counts_every_kind_of_change_as_dirty() {
        // staged, unstaged and untracked all mean "not committed yet".
        assert_eq!(parse_status_v2(FULL).dirty, 3);
    }

    #[test]
    fn a_branch_with_no_upstream_has_no_distance() {
        let out = "# branch.head local-only\n";
        let state = parse_status_v2(out);
        assert_eq!(state.branch, "local-only");
        assert_eq!(state.upstream, None);
        assert_eq!((state.ahead, state.behind), (0, 0));
    }

    #[test]
    fn a_clean_tree_is_clean_and_a_dirty_one_is_not() {
        let clean = parse_status_v2("# branch.head main\n# branch.ab +0 -0\n");
        assert!(clean.clean());
        assert!(!parse_status_v2(FULL).clean());
    }

    #[test]
    fn unpushed_commits_are_not_clean_either() {
        // Nothing to commit, but the work is still only on this machine.
        let ahead = parse_status_v2("# branch.head main\n# branch.ab +2 -0\n");
        assert_eq!(ahead.dirty, 0);
        assert!(!ahead.clean());
    }

    #[test]
    fn reads_both_halves_of_a_shortstat() {
        let (a, r) = parse_shortstat(" 12 files changed, 345 insertions(+), 67 deletions(-)\n");
        assert_eq!((a, r), (345, 67));
    }

    #[test]
    fn survives_a_shortstat_with_only_one_half() {
        assert_eq!(parse_shortstat(" 1 file changed, 5 insertions(+)\n"), (5, 0));
        assert_eq!(parse_shortstat(" 1 file changed, 9 deletions(-)\n"), (0, 9));
        assert_eq!(parse_shortstat(""), (0, 0));
    }

    #[test]
    fn a_numstat_is_one_change_per_line_with_binaries_counted_as_zero() {
        let out = "128\t0\tsrc/components/ReportDialog.tsx\n210\t19\tsrc-tauri/src/lib.rs\n-\t-\tdocs/img/shot.png\n3\t1\tsrc/{old => new}/a.ts\n";
        let changes = parse_numstat(out);
        assert_eq!(changes.len(), 4);
        assert_eq!(changes[0], FileChange { path: "src/components/ReportDialog.tsx".into(), added: 128, removed: 0, untracked: false, binary: false });
        assert_eq!(changes[2], FileChange { path: "docs/img/shot.png".into(), added: 0, removed: 0, untracked: false, binary: true });
        assert_eq!(changes[3].path, "src/new/a.ts", "a rename shows where the file is now");
        assert!(parse_numstat("").is_empty());
    }

    #[test]
    fn untracked_files_come_from_the_status_and_count_as_new() {
        let out = "# branch.head main\n1 .M N... 100644 100644 100644 aaa bbb src/main.rs\n? notes.txt\n? docs/new.md\n";
        assert_eq!(untracked_v2(out), vec!["notes.txt", "docs/new.md"]);
    }

    #[test]
    fn the_totals_sum_every_change() {
        let changes = vec![
            FileChange { path: "a".into(), added: 2, removed: 1, untracked: false, binary: false },
            FileChange { path: "b".into(), added: 5, removed: 0, untracked: true, binary: false },
        ];
        assert_eq!(totals(&changes), (7, 1));
    }
}

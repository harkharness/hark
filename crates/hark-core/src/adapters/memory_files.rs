//! File-based memory under `<root>/.hark/`: the Q&A journal now, workspace
//! state and dispatch briefs next.

use crate::domain::memory::journal_tail;
use crate::ports::Journal;
use std::path::{Path, PathBuf};

pub struct HarkDir;

/// The per-project memory dir, migrating a legacy `.vox/` on first touch.
fn hark_dir(root: &Path) -> PathBuf {
    let new = root.join(".hark");
    let old = root.join(".vox");
    if old.is_dir() && !new.exists() {
        let _ = std::fs::rename(&old, &new);
    }
    new
}

fn journal_path(root: &Path) -> PathBuf {
    hark_dir(root).join("journal.md")
}

impl Journal for HarkDir {
    fn tail(&self, root: &Path, n: usize) -> Vec<String> {
        std::fs::read_to_string(journal_path(root))
            .map(|content| journal_tail(&content, n))
            .unwrap_or_default()
    }

    fn append(&self, root: &Path, entry: &str) -> anyhow::Result<()> {
        let path = journal_path(root);
        std::fs::create_dir_all(path.parent().expect(".hark parent"))?;
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;
        file.write_all(entry.as_bytes())?;
        Ok(())
    }
}

/// Append one line to `<root>/.hark/state.md` (workspace dispatch log).
pub fn append_state(root: &Path, line: &str) -> anyhow::Result<()> {
    let path = hark_dir(root).join("state.md");
    std::fs::create_dir_all(path.parent().expect(".hark parent"))?;
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new().create(true).append(true).open(path)?;
    file.write_all(line.as_bytes())?;
    file.write_all(b"\n")?;
    Ok(())
}

/// Write the full dispatch brief to `<root>/.hark/briefs/<task_id>.md`.
pub fn write_brief(root: &Path, task_id: &str, content: &str) -> anyhow::Result<PathBuf> {
    let dir = hark_dir(root).join("briefs");
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{task_id}.md"));
    std::fs::write(&path, content)?;
    Ok(path)
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
            .append(dir.path(), &journal_entry("2026-08-14T10:00:00Z", "q1", "a1"))
            .unwrap();
        journal
            .append(dir.path(), &journal_entry("2026-08-14T11:00:00Z", "q2", "a2"))
            .unwrap();

        let tail = journal.tail(dir.path(), 1);
        assert_eq!(tail.len(), 1);
        assert!(tail[0].contains("q2"));
        assert!(dir.path().join(".hark").join("journal.md").exists());
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
}

//! File-based memory under `<root>/.vox/`: the Q&A journal now, workspace
//! state and dispatch briefs next.

use crate::domain::memory::journal_tail;
use crate::ports::Journal;
use std::path::{Path, PathBuf};

pub struct VoxDir;

fn journal_path(root: &Path) -> PathBuf {
    root.join(".vox").join("journal.md")
}

impl Journal for VoxDir {
    fn tail(&self, root: &Path, n: usize) -> Vec<String> {
        std::fs::read_to_string(journal_path(root))
            .map(|content| journal_tail(&content, n))
            .unwrap_or_default()
    }

    fn append(&self, root: &Path, entry: &str) -> anyhow::Result<()> {
        let path = journal_path(root);
        std::fs::create_dir_all(path.parent().expect(".vox parent"))?;
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;
        file.write_all(entry.as_bytes())?;
        Ok(())
    }
}

/// Append one line to `<root>/.vox/state.md` (workspace dispatch log).
pub fn append_state(root: &Path, line: &str) -> anyhow::Result<()> {
    let path = root.join(".vox").join("state.md");
    std::fs::create_dir_all(path.parent().expect(".vox parent"))?;
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new().create(true).append(true).open(path)?;
    file.write_all(line.as_bytes())?;
    file.write_all(b"\n")?;
    Ok(())
}

/// Write the full dispatch brief to `<root>/.vox/briefs/<task_id>.md`.
pub fn write_brief(root: &Path, task_id: &str, content: &str) -> anyhow::Result<PathBuf> {
    let dir = root.join(".vox").join("briefs");
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
    fn state_and_brief_land_under_vox_dir() {
        let dir = tempfile::tempdir().unwrap();
        append_state(dir.path(), "- t1 running").unwrap();
        append_state(dir.path(), "- t1 done").unwrap();
        let state = std::fs::read_to_string(dir.path().join(".vox/state.md")).unwrap();
        assert_eq!(state.lines().count(), 2);

        let brief = write_brief(dir.path(), "t1", "# task\ndo the thing").unwrap();
        assert!(brief.ends_with(".vox/briefs/t1.md"));
        assert!(std::fs::read_to_string(brief).unwrap().contains("do the thing"));
    }

    #[test]
    fn appends_and_reads_tail() {
        let dir = tempfile::tempdir().unwrap();
        let journal = VoxDir;

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
        assert!(dir.path().join(".vox").join("journal.md").exists());
    }
}

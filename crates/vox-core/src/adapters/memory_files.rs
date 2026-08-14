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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::memory::journal_entry;

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

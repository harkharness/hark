//! The app's own log on disk: what Hark did, scrubbed as it is written.
//!
//! Until now the app kept nothing: forty lines of a worker's stderr in
//! memory, a raw feed per thread that died with the window. A bug report
//! needs a record, and the record must be safe to paste into a public
//! issue on the day it is needed — so every line is passed through
//! `domain::redact` on the way in, never trusted to be scrubbed later.
//! One file, rotated once: `logs/hark.log` and `logs/hark.log.1`.

use crate::domain::redact::redact;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Rotate when the file grows past this.
pub const MAX_BYTES: u64 = 1_048_576;

pub struct DiagLog {
    path: PathBuf,
    max_bytes: u64,
    lock: std::sync::Mutex<()>,
}

impl DiagLog {
    /// `<data_dir>/logs/hark.log`, the folder created if missing.
    pub fn open(data_dir: &Path) -> Self {
        let dir = data_dir.join("logs");
        let _ = std::fs::create_dir_all(&dir);
        Self { path: dir.join("hark.log"), max_bytes: MAX_BYTES, lock: std::sync::Mutex::new(()) }
    }

    /// A smaller rotation threshold, for tests.
    pub fn with_max_bytes(mut self, max_bytes: u64) -> Self {
        self.max_bytes = max_bytes;
        self
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// One line, stamped and scrubbed. Never fails loudly: a log that
    /// cannot be written must not take the app with it.
    pub fn append(&self, line: &str) {
        let _guard = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        let stamped = format!(
            "{} {}\n",
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
            redact(line.trim_end()).replace('\n', " ⏎ ")
        );
        let full = std::fs::metadata(&self.path).map(|m| m.len() >= self.max_bytes).unwrap_or(false);
        if full {
            let _ = std::fs::rename(&self.path, self.previous());
        }
        if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(&self.path) {
            let _ = file.write_all(stamped.as_bytes());
        }
    }

    /// The newest `n` lines, oldest first: the live file, and the rotated
    /// one before it when the live file is still young.
    pub fn tail(&self, n: usize) -> Vec<String> {
        let _guard = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        let mut lines: Vec<String> = read_lines(&self.path);
        if lines.len() < n {
            let mut older = read_lines(&self.previous());
            older.append(&mut lines);
            lines = older;
        }
        let skip = lines.len().saturating_sub(n);
        lines.into_iter().skip(skip).collect()
    }

    fn previous(&self) -> PathBuf {
        self.path.with_extension("log.1")
    }
}

fn read_lines(path: &Path) -> Vec<String> {
    std::fs::read_to_string(path)
        .map(|s| s.lines().map(str::to_string).collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_is_appended_comes_back_newest_last() {
        let dir = tempfile::tempdir().unwrap();
        let log = DiagLog::open(dir.path());
        for i in 1..=5 {
            log.append(&format!("line {i}"));
        }
        let tail = log.tail(3);
        assert_eq!(tail.len(), 3);
        assert!(tail[0].ends_with("line 3") && tail[2].ends_with("line 5"), "{tail:?}");
        assert!(tail[0].len() > "line 3".len(), "a timestamp leads the line");
        assert!(log.path().ends_with("logs/hark.log"));
    }

    #[test]
    fn lines_are_scrubbed_on_the_way_in() {
        let dir = tempfile::tempdir().unwrap();
        let log = DiagLog::open(dir.path());
        log.append("spawn cwd=/Users/ana/proj token sk-ant-api03-abcdefghijklmnopqrstuvwxyz");
        let raw = std::fs::read_to_string(log.path()).unwrap();
        assert!(raw.contains("cwd=~/proj") && raw.contains("<redacted>"), "{raw}");
        assert!(!raw.contains("/Users/ana") && !raw.contains("sk-ant"), "{raw}");
    }

    #[test]
    fn the_file_rotates_once_and_the_tail_spans_both() {
        let dir = tempfile::tempdir().unwrap();
        let log = DiagLog::open(dir.path()).with_max_bytes(200);
        for i in 1..=40 {
            log.append(&format!("line {i:02}"));
        }
        assert!(dir.path().join("logs/hark.log.1").exists(), "one previous file is kept");
        assert!(std::fs::metadata(log.path()).unwrap().len() <= 400, "the live file stays small");
        let tail = log.tail(40);
        assert!(tail.last().unwrap().ends_with("line 40"));
        assert!(tail.len() >= 10, "the tail reads back across the rotation: {}", tail.len());
    }

    #[test]
    fn an_empty_log_has_an_empty_tail() {
        let dir = tempfile::tempdir().unwrap();
        assert!(DiagLog::open(dir.path()).tail(10).is_empty());
    }
}

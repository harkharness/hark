//! Reads of a session log that never load the whole file: a desktop
//! session can be hundreds of megabytes, and a chat opens its tail.

use crate::domain::transcript::{mirror_start, tail_entries, Entry};
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

/// What grew since `from`, as complete lines: the text, the byte it
/// started at (after the ceiling and any partial line it skipped), and
/// the byte after the last complete line (the next offset).
pub struct Growth {
    pub text: String,
    pub from: u64,
    pub next: u64,
}

/// The log from `from`, capped at `max` bytes before its end. When the
/// cap moves the start, the partial line it lands on is skipped: the
/// mirror shows whole lines or nothing.
pub fn read_since(path: &Path, from: u64, max: u64) -> std::io::Result<Growth> {
    let mut file = std::fs::File::open(path)?;
    let len = file.metadata()?.len();
    let start = mirror_start(from, len, max);
    file.seek(SeekFrom::Start(start))?;
    let mut buf = String::new();
    file.read_to_string(&mut buf)?;
    // The ceiling moved the start into the middle of a line: drop it.
    let skipped = if start != from && start > 0 {
        buf.find('\n').map(|i| i + 1).unwrap_or(buf.len())
    } else {
        0
    };
    let body = &buf[skipped..];
    // A line still being written has no newline yet: next time.
    let complete = body.rfind('\n').map(|i| i + 1).unwrap_or(0);
    Ok(Growth {
        text: body[..complete].to_string(),
        from: start + skipped as u64,
        next: start + (skipped + complete) as u64,
    })
}

/// The newest `limit` entries, reading from the end in growing windows,
/// plus the file length (where a mirror starts from).
pub fn read_tail(path: &Path, limit: usize) -> std::io::Result<(Vec<Entry>, u64)> {
    let len = std::fs::metadata(path)?.len();
    // 4 MB holds a few hundred turns of a normal session; a window that
    // comes up short grows fourfold until it is the whole file.
    let mut window: u64 = 4 << 20;
    loop {
        let g = read_since(path, 0, window)?;
        let entries = tail_entries(g.text.lines(), limit);
        if entries.len() >= limit || window >= len {
            return Ok((entries, len));
        }
        window = window.saturating_mul(4);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn line(i: usize) -> String {
        format!(
            r#"{{"type":"user","timestamp":"2026-11-10T10:{:02}:00Z","message":{{"role":"user","content":"turn {i}"}}}}"#,
            i % 60
        )
    }

    fn log(n: usize) -> (tempfile::TempDir, std::path::PathBuf, Vec<String>) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("s.jsonl");
        let lines: Vec<String> = (0..n).map(line).collect();
        let mut f = std::fs::File::create(&path).unwrap();
        for l in &lines {
            writeln!(f, "{l}").unwrap();
        }
        (dir, path, lines)
    }

    #[test]
    fn growth_is_read_from_where_the_mirror_left_off() {
        let (_d, path, lines) = log(3);
        let first = lines[0].len() as u64 + 1;
        let g = read_since(&path, first, 1 << 20).unwrap();
        assert_eq!(g.from, first);
        assert_eq!(g.text, format!("{}\n{}\n", lines[1], lines[2]));
        assert_eq!(g.next, std::fs::metadata(&path).unwrap().len());
    }

    #[test]
    fn a_line_still_being_written_waits_for_the_next_read() {
        let (_d, path, lines) = log(2);
        let mut f = std::fs::OpenOptions::new().append(true).open(&path).unwrap();
        write!(f, "{{\"type\":\"user\",\"partial").unwrap();
        let g = read_since(&path, 0, 1 << 20).unwrap();
        assert_eq!(g.text, format!("{}\n{}\n", lines[0], lines[1]));
        assert_eq!(g.next, g.text.len() as u64);
    }

    #[test]
    fn a_huge_gap_is_read_from_the_ceiling_on_a_whole_line() {
        // Six lines of ~100 bytes; a ceiling of 250 bytes lands inside a
        // line, which is skipped: only whole lines come back.
        let (_d, path, lines) = log(6);
        let len = std::fs::metadata(&path).unwrap().len();
        let g = read_since(&path, 0, 250).unwrap();
        assert!(g.from > len - 250, "started after the ceiling, past the cut line");
        assert!(g.text.starts_with('{'), "whole lines only: {:?}", &g.text[..20]);
        let got: Vec<&str> = g.text.lines().collect();
        assert_eq!(got, &lines[4..]);
        assert_eq!(g.next, len);
    }

    #[test]
    fn the_tail_comes_back_without_reading_the_whole_file() {
        let (_d, path, _lines) = log(50);
        let (entries, len) = read_tail(&path, 5).unwrap();
        assert_eq!(entries.len(), 5);
        assert_eq!(entries[0].text, "turn 45");
        assert_eq!(entries[4].text, "turn 49");
        assert_eq!(len, std::fs::metadata(&path).unwrap().len());
        // Fewer lines than asked: all of them, in order.
        let (all, _) = read_tail(&path, 500).unwrap();
        assert_eq!(all.len(), 50);
        assert_eq!(all[0].text, "turn 0");
    }
}

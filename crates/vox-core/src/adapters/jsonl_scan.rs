//! Incremental scanner for `~/.claude/projects/**/*.jsonl`.
//! Resumes each file from its stored byte offset (the logs are append-only)
//! and skips files whose mtime is unchanged.

use crate::domain::session_log::parse_line;
use crate::domain::snapshot::SessionSummary;
use crate::ports::SessionStore;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::Path;

/// Walk every `*/*.jsonl` under `projects_dir`, fold new lines into the
/// stored summaries. Returns how many files had new content.
pub fn refresh_index(
    projects_dir: &Path,
    store: &mut (impl SessionStore + ?Sized),
) -> anyhow::Result<usize> {
    let files = jsonl_files(projects_dir)?;
    let mut processed = 0;
    for path in files {
        if index_file(&path, store)? {
            processed += 1;
        }
    }
    Ok(processed)
}

fn jsonl_files(projects_dir: &Path) -> anyhow::Result<Vec<std::path::PathBuf>> {
    let mut files = Vec::new();
    for project in std::fs::read_dir(projects_dir)? {
        let project = project?.path();
        if !project.is_dir() {
            continue;
        }
        for entry in std::fs::read_dir(&project)? {
            let path = entry?.path();
            if path.extension().and_then(|e| e.to_str()) == Some("jsonl") {
                files.push(path);
            }
        }
    }
    Ok(files)
}

/// Returns true when the file had new content to fold.
fn index_file(path: &Path, store: &mut (impl SessionStore + ?Sized)) -> anyhow::Result<bool> {
    let path_str = path.to_string_lossy().to_string();
    let meta = std::fs::metadata(path)?;
    let mtime = mtime_of(&meta);
    let len = meta.len();

    let stored = store.file_state(&path_str)?;
    let (summary, offset) = match stored {
        Some((_, _, stored_mtime)) if stored_mtime == mtime => return Ok(false),
        // Truncated or rotated file: refold from scratch.
        Some((_, offset, _)) if offset > len => (fresh_summary(path), 0),
        Some((summary, offset, _)) => (summary, offset),
        None => (fresh_summary(path), 0),
    };

    let mut reader = BufReader::new(std::fs::File::open(path)?);
    reader.seek(SeekFrom::Start(offset))?;
    let summary = reader
        .lines()
        .map_while(Result::ok)
        .filter_map(|line| parse_line(&line))
        .fold(summary, SessionSummary::apply);

    store.save_file_state(&path_str, &summary, len, mtime)?;
    Ok(true)
}

fn fresh_summary(path: &Path) -> SessionSummary {
    let session_id = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    SessionSummary::new(session_id)
}

fn mtime_of(meta: &std::fs::Metadata) -> i64 {
    use std::os::unix::fs::MetadataExt;
    meta.mtime()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::sqlite_store::SqliteStore;
    use crate::ports::SessionStore;
    use std::io::Write;

    fn user_line(ts: &str, text: &str) -> String {
        format!(
            r#"{{"type":"user","timestamp":"{ts}","cwd":"/home/dev/proj","gitBranch":"main","message":{{"role":"user","content":"{text}"}}}}"#
        )
    }

    fn write_session(dir: &std::path::Path, name: &str, lines: &[String]) -> std::path::PathBuf {
        let project = dir.join("-home-dev-proj");
        std::fs::create_dir_all(&project).unwrap();
        let path = project.join(name);
        let mut f = std::fs::File::create(&path).unwrap();
        for l in lines {
            writeln!(f, "{l}").unwrap();
        }
        path
    }

    #[test]
    fn indexes_new_session_files() {
        let dir = tempfile::tempdir().unwrap();
        write_session(
            dir.path(),
            "sess-1.jsonl",
            &[
                user_line("2026-08-14T10:00:00.000Z", "start migration"),
                user_line("2026-08-14T10:05:00.000Z", "open the PR"),
            ],
        );
        let mut store = SqliteStore::in_memory().unwrap();

        let processed = refresh_index(dir.path(), &mut store).unwrap();

        assert_eq!(processed, 1);
        let sessions = store.sessions_since("2026-08-14T00:00:00.000Z").unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].session_id, "sess-1");
        assert_eq!(sessions[0].last_prompt.as_deref(), Some("open the PR"));
        assert_eq!(sessions[0].recent_prompts.len(), 2);
    }

    #[test]
    fn skips_unchanged_files_and_resumes_from_offset() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_session(
            dir.path(),
            "sess-1.jsonl",
            &[user_line("2026-08-14T10:00:00.000Z", "start migration")],
        );
        let mut store = SqliteStore::in_memory().unwrap();
        refresh_index(dir.path(), &mut store).unwrap();

        // Second pass with nothing new: no files processed.
        assert_eq!(refresh_index(dir.path(), &mut store).unwrap(), 0);

        // Append one line; only the tail is parsed and state advances.
        let (_, offset_before, _) = store.file_state(path.to_str().unwrap()).unwrap().unwrap();
        let mut f = std::fs::OpenOptions::new().append(true).open(&path).unwrap();
        writeln!(f, "{}", user_line("2026-08-14T11:00:00.000Z", "now run tests")).unwrap();
        f.sync_all().unwrap();
        drop(f);
        filetime_bump(&path);

        assert_eq!(refresh_index(dir.path(), &mut store).unwrap(), 1);
        let (summary, offset_after, _) =
            store.file_state(path.to_str().unwrap()).unwrap().unwrap();
        assert!(offset_after > offset_before);
        assert_eq!(summary.last_prompt.as_deref(), Some("now run tests"));
        assert_eq!(summary.recent_prompts.len(), 2);
    }

    /// Ensure mtime visibly changes even on coarse-grained filesystems.
    fn filetime_bump(path: &std::path::Path) {
        let meta = std::fs::metadata(path).unwrap();
        let mtime = filetime_of(&meta) + 2;
        let ft = std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(mtime as u64);
        let f = std::fs::OpenOptions::new().write(true).open(path).unwrap();
        f.set_times(std::fs::FileTimes::new().set_modified(ft)).unwrap();
    }

    fn filetime_of(meta: &std::fs::Metadata) -> i64 {
        use std::os::unix::fs::MetadataExt;
        meta.mtime()
    }
}

//! Incremental scanner for `~/.claude/projects/**/*.jsonl`.
//! Resumes each file from its stored byte offset (the logs are append-only)
//! and skips files whose mtime is unchanged. The SAME pass feeds two
//! stores: the session index and the token spend ledger (source=jsonl).

use crate::domain::claude_event::TokenUsage;
use crate::domain::session_log::{parse_line, SessionEvent};
use crate::domain::snapshot::SessionSummary;
use crate::domain::spend::{SpendKind, SpendRow, SpendSource};
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
    // A machine that never ran Claude Code has no projects dir yet; that is
    // an empty history, not an error.
    if !projects_dir.exists() {
        return Ok(files);
    }
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
    let mut spend_rows = Vec::new();
    let session_id = summary.session_id.clone();
    let workspace = summary.cwd.clone();
    let summary = reader
        .lines()
        .map_while(Result::ok)
        .filter_map(|line| parse_line(&line))
        .inspect(|event| {
            if let Some(row) = spend_row(event, &session_id, workspace.as_deref()) {
                spend_rows.push(row);
            }
        })
        .fold(summary, SessionSummary::apply);

    store.record_spend(&spend_rows)?;
    store.save_file_state(&path_str, &summary, len, mtime)?;
    Ok(true)
}

/// Ledger row for one assistant message (tokens only; session files never
/// carry USD). The synthetic request id keeps rebuilds idempotent even on
/// lines without one.
fn spend_row(event: &SessionEvent, session_id: &str, workspace: Option<&str>) -> Option<SpendRow> {
    let SessionEvent::AssistantUsage { ts, request_id, model, usage, is_sidechain } = event else {
        return None;
    };
    // Synthetic/zero rows are CLI plumbing, not spend.
    if usage.input + usage.output + usage.cache_read + usage.cache_created == 0 {
        return None;
    }
    let model = model.clone().unwrap_or_else(|| "unknown".into());
    if model == "<synthetic>" {
        return None;
    }
    Some(SpendRow {
        ts: ts.clone(),
        kind: SpendKind::Session,
        source: SpendSource::Jsonl,
        task_id: None,
        label: None,
        session_id: Some(session_id.to_string()),
        workspace: workspace.map(String::from),
        model: model.clone(),
        usage: TokenUsage { ..*usage },
        cost_usd: None,
        duration_ms: None,
        is_error: false,
        is_sidechain: *is_sidechain,
        context_window: None,
        request_id: Some(
            request_id
                .clone()
                .unwrap_or_else(|| format!("{session_id}:{ts}:{model}")),
        ),
        outcome: None,
    })
}

/// Full retroactive sweep: parse EVERY line of every session file and
/// insert token rows (idempotent through the request_id unique index).
/// Never touches the incremental byte offsets of the summary index.
pub fn rebuild_spend(
    projects_dir: &Path,
    store: &mut (impl SessionStore + ?Sized),
) -> anyhow::Result<usize> {
    let files = jsonl_files(projects_dir)?;
    let mut scanned = 0usize;
    for path in &files {
        let session_id = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        let reader = BufReader::new(std::fs::File::open(path)?);
        let mut cwd: Option<String> = None;
        let mut rows = Vec::new();
        for line in reader.lines().map_while(Result::ok) {
            match parse_line(&line) {
                Some(SessionEvent::UserPrompt { cwd: seen, .. }) => {
                    if seen.is_some() {
                        cwd = seen;
                    }
                }
                Some(event @ SessionEvent::AssistantUsage { .. }) => {
                    if let Some(row) = spend_row(&event, &session_id, cwd.as_deref()) {
                        rows.push(row);
                    }
                }
                _ => {}
            }
        }
        store.record_spend(&rows)?;
        scanned += 1;
    }
    Ok(scanned)
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
    fn missing_projects_dir_is_an_empty_index_not_an_error() {
        // A fresh machine has no ~/.claude/projects yet; asking must not break.
        let mut store = SqliteStore::in_memory().unwrap();
        let ghost = std::path::Path::new("/nonexistent/hark-test/projects");
        assert_eq!(refresh_index(ghost, &mut store).unwrap(), 0);
        assert_eq!(rebuild_spend(ghost, &mut store).unwrap(), 0);
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

    #[test]
    fn same_pass_feeds_the_spend_ledger_idempotently() {
        use crate::domain::spend::SpendSource;
        use crate::ports::{SpendGroup, SpendLedger, SpendQuery};
        let assistant = |ts: &str, req: &str| {
            format!(
                r#"{{"type":"assistant","timestamp":"{ts}","requestId":"{req}","message":{{"role":"assistant","model":"claude-sonnet-5","content":[],"usage":{{"input_tokens":3,"output_tokens":7,"cache_read_input_tokens":11,"cache_creation_input_tokens":13}}}}}}"#
            )
        };
        let dir = tempfile::tempdir().unwrap();
        write_session(
            dir.path(),
            "sess-2.jsonl",
            &[
                user_line("2026-08-14T10:00:00.000Z", "work"),
                assistant("2026-08-14T10:00:05.000Z", "req-a"),
                // Same requestId repeated on a second line (real CLI behavior).
                assistant("2026-08-14T10:00:05.000Z", "req-a"),
                assistant("2026-08-14T10:00:09.000Z", "req-b"),
            ],
        );
        let mut store = SqliteStore::in_memory().unwrap();
        refresh_index(dir.path(), &mut store).unwrap();

        let agg = store
            .spend_summary(&SpendQuery {
                since: None,
                group: SpendGroup::Session,
                source: SpendSource::Jsonl,
                workspace: None,
            })
            .unwrap();
        assert_eq!(agg.len(), 1);
        assert_eq!(agg[0].key, "sess-2");
        assert_eq!(agg[0].usage.input, 6, "duplicate requestId collapses");
        assert_eq!(agg[0].usage.output, 14);
        assert_eq!(agg[0].usage.cache_read, 22);
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

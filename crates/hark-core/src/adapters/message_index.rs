//! Reads what was said in the session files into the full-text index
//! (fase 16). Every indexer already lists its files in the store; this
//! walks the ones the message index has not read to the end, newest
//! session first, from where it stopped, keeping only prose
//! (`domain::history_search::prose_of`). Any agent's history is covered:
//! the line parser knows claude's dialect and Hark's own records.

use crate::domain::history_search::{prose_of, IndexedMessage};
use crate::ports::SessionStore;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::time::{Duration, Instant};

/// A cursor past any file: a file that is gone is not asked for again.
const GONE: u64 = i64::MAX as u64;

/// Read every file the message index is behind on, newest session first.
/// `budget` bounds the pass for a caller that cannot wait (an ask reads
/// what changed in the last minutes and moves on): at least one file is
/// always read, and the rest wait for the next pass. Returns how many
/// messages were indexed.
pub fn catch_up(store: &mut dyn SessionStore, budget: Option<Duration>) -> anyhow::Result<usize> {
    let started = Instant::now();
    let mut indexed = 0;
    for (i, (path, cursor)) in store.message_backlog()?.into_iter().enumerate() {
        if i > 0 && budget.is_some_and(|b| started.elapsed() >= b) {
            break;
        }
        let Ok(meta) = std::fs::metadata(&path) else {
            // Deleted since it was indexed: never asked for again.
            store.add_messages(&path, &[], GONE)?;
            continue;
        };
        let from = if cursor > meta.len() {
            // Truncated or rotated: what it said before is not there now.
            store.clear_messages(&path)?;
            0
        } else {
            cursor
        };
        let (messages, read_to) = read_prose(&path, from)?;
        indexed += messages.len();
        store.add_messages(&path, &messages, read_to)?;
    }
    Ok(indexed)
}

/// The prose of a file from byte `from` on, and the byte after the last
/// COMPLETE line: a line the agent is still writing has no newline yet
/// and is left for the next pass.
fn read_prose(path: &str, from: u64) -> anyhow::Result<(Vec<IndexedMessage>, u64)> {
    let mut reader = BufReader::new(std::fs::File::open(path)?);
    reader.seek(SeekFrom::Start(from))?;
    let mut messages = Vec::new();
    let mut at = from;
    let mut line = String::new();
    loop {
        line.clear();
        // A line that is not UTF-8 ends the pass for this file; the
        // cursor stays before it and the bytes are tried again later.
        let Ok(read) = reader.read_line(&mut line) else { break };
        if read == 0 || !line.ends_with('\n') {
            break;
        }
        at += read as u64;
        if let Some(message) = prose_of(line.trim_end()) {
            messages.push(message);
        }
    }
    Ok((messages, at))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::sqlite_store::SqliteStore;
    use crate::domain::snapshot::SessionSummary;
    use std::io::Write;

    const USER: &str = r#"{"type":"user","timestamp":"2026-11-17T10:00:00Z","message":{"role":"user","content":"migra o logstash pro cluster novo"}}"#;
    const TOOL: &str = r#"{"type":"user","timestamp":"2026-11-17T10:00:40Z","message":{"role":"user","content":[{"type":"tool_result","content":"pod/logstash-0 Running"}]}}"#;
    const DONE: &str = r#"{"type":"assistant","timestamp":"2026-11-17T10:01:00Z","message":{"role":"assistant","content":[{"type":"text","text":"Migração concluída, falta desligar a fila antiga."}]}}"#;

    fn session(store: &mut SqliteStore, path: &std::path::Path, id: &str) {
        let summary = SessionSummary {
            session_id: id.into(),
            title: Some("LogStash".into()),
            last_ts: Some("2026-11-17T10:00:00Z".into()),
            ..Default::default()
        };
        let len = std::fs::metadata(path).unwrap().len();
        store.save_file_state(&path.to_string_lossy(), &summary, len, 1).unwrap();
    }

    #[test]
    fn a_session_file_is_read_once_and_only_its_prose_is_kept() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.jsonl");
        std::fs::write(&path, format!("{USER}\n{TOOL}\n{DONE}\n")).unwrap();
        let mut store = SqliteStore::in_memory().unwrap();
        session(&mut store, &path, "a");

        assert_eq!(catch_up(&mut store, None).unwrap(), 2);
        assert_eq!(store.search_messages("\"desligar\"", 5, None).unwrap().len(), 1);
        // The tool's output is not conversation.
        assert!(store.search_messages("\"Running\"", 5, None).unwrap().is_empty());
        // Caught up: a second pass reads nothing and doubles nothing.
        assert_eq!(catch_up(&mut store, None).unwrap(), 0);
        assert_eq!(store.search_messages("\"logstash\"", 5, None).unwrap().len(), 1);
    }

    #[test]
    fn what_is_said_later_is_read_from_where_the_file_stopped() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.jsonl");
        std::fs::write(&path, format!("{USER}\n")).unwrap();
        let mut store = SqliteStore::in_memory().unwrap();
        session(&mut store, &path, "a");
        assert_eq!(catch_up(&mut store, None).unwrap(), 1);

        let mut file = std::fs::OpenOptions::new().append(true).open(&path).unwrap();
        // A line still being written has no newline yet: it waits.
        write!(file, "{DONE}").unwrap();
        session(&mut store, &path, "a");
        assert_eq!(catch_up(&mut store, None).unwrap(), 0);
        writeln!(file).unwrap();
        session(&mut store, &path, "a");
        assert_eq!(catch_up(&mut store, None).unwrap(), 1);
        assert_eq!(store.search_messages("\"desligar\"", 5, None).unwrap().len(), 1);
    }

    #[test]
    fn a_file_that_shrank_is_read_again_and_one_that_is_gone_is_let_go() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.jsonl");
        std::fs::write(&path, format!("{USER}\n{DONE}\n")).unwrap();
        let mut store = SqliteStore::in_memory().unwrap();
        session(&mut store, &path, "a");
        catch_up(&mut store, None).unwrap();

        std::fs::write(&path, format!("{DONE}\n")).unwrap();
        session(&mut store, &path, "a");
        // The cursor is past the new end: the store says "behind" only
        // when asked to look again, which a rotated file is.
        store.clear_messages(&path.to_string_lossy()).unwrap();
        assert_eq!(catch_up(&mut store, None).unwrap(), 1);
        assert!(store.search_messages("\"cluster\"", 5, None).unwrap().is_empty());

        let gone = dir.path().join("gone.jsonl");
        std::fs::write(&gone, format!("{USER}\n")).unwrap();
        session(&mut store, &gone, "gone");
        std::fs::remove_file(&gone).unwrap();
        assert_eq!(catch_up(&mut store, None).unwrap(), 0);
        assert!(store.message_backlog().unwrap().is_empty());
    }

    #[test]
    fn a_budget_stops_between_files_and_the_newest_was_read_first() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = SqliteStore::in_memory().unwrap();
        for (name, ts) in [("old", "2026-11-10T10:00:00Z"), ("new", "2026-11-17T10:00:00Z")] {
            let path = dir.path().join(format!("{name}.jsonl"));
            std::fs::write(&path, format!("{USER}\n")).unwrap();
            let summary = SessionSummary { session_id: name.into(), last_ts: Some(ts.into()), ..Default::default() };
            store.save_file_state(&path.to_string_lossy(), &summary, std::fs::metadata(&path).unwrap().len(), 1).unwrap();
        }
        // No time at all: one file still gets read (the newest), so a
        // caller with a tight budget always makes progress.
        assert_eq!(catch_up(&mut store, Some(Duration::ZERO)).unwrap(), 1);
        let left = store.message_backlog().unwrap();
        assert_eq!(left.len(), 1);
        assert!(left[0].0.ends_with("old.jsonl"));
    }
}

//! The recorder: Hark's own history for an agent that leaves none on disk.
//!
//! One append-only file per session, `<data_dir>/sessions/<agent>/<id>.jsonl`,
//! in the `domain::recorded` format — so the transcript reader, the
//! session index (`refresh`, the same incremental fold claude's files
//! get), the search, the funnel, the viewer, the mirror and the brief all
//! work for a session the agent itself never wrote down. Honest limit: a
//! session opened outside Hark is invisible; only what passed through
//! Hark is here.

use crate::domain::recorded::{self, Header, Line, Usage};
use crate::domain::snapshot::SessionSummary;
use crate::domain::transcript::{Entry, Role};
use crate::ports::{HistoryIndexer, SessionStore};
use std::io::{BufRead, BufReader, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// RFC 3339 UTC with milliseconds — the same clock the ledger uses.
pub fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

/// Where a session's record lives.
pub fn session_file(data_dir: &Path, agent: &str, session_id: &str) -> PathBuf {
    let safe: String = session_id
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' { c } else { '_' })
        .collect();
    data_dir.join("sessions").join(agent).join(format!("{safe}.jsonl"))
}

struct Inner {
    /// Known once the agent has answered with a session id.
    path: Option<PathBuf>,
    /// Lines said before the id was known (the opening prompt).
    pending: Vec<String>,
}

/// One session's pen. Cheap to share (`Arc`); every write is one append.
pub struct Recorder {
    data_dir: PathBuf,
    agent: String,
    cwd: String,
    inner: Mutex<Inner>,
}

impl Recorder {
    pub fn new(data_dir: &Path, agent: &str, cwd: &Path) -> Self {
        Self {
            data_dir: data_dir.to_path_buf(),
            agent: agent.to_string(),
            cwd: cwd.display().to_string(),
            inner: Mutex::new(Inner { path: None, pending: Vec::new() }),
        }
    }

    /// The session has an id: open its file (a header when the file is
    /// new — a resumed session appends under the one it has) and write
    /// down whatever was said before the id arrived.
    pub fn started(&self, session_id: &str) {
        let path = session_file(&self.data_dir, &self.agent, session_id);
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let mut lines = Vec::new();
        if !path.exists() {
            lines.push(recorded::render(&Line::Header(Header {
                agent: self.agent.clone(),
                session_id: session_id.to_string(),
                cwd: self.cwd.clone(),
                ts: now_iso(),
            })));
        }
        lines.append(&mut inner.pending);
        inner.path = Some(path.clone());
        drop(inner);
        append(&path, &lines);
    }

    /// The user's words — the prompts are what the index titles and
    /// searches by.
    pub fn user(&self, text: &str) {
        self.line(Line::Entry(Entry { ts: now_iso(), role: Role::User, text: text.to_string(), tool: None, is_error: false }));
    }

    pub fn entry(&self, entry: Entry) {
        self.line(Line::Entry(entry));
    }

    pub fn usage(&self, usage: Usage) {
        self.line(Line::Usage(usage));
    }

    pub fn path(&self) -> Option<PathBuf> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner()).path.clone()
    }

    fn line(&self, line: Line) {
        let text = recorded::render(&line);
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        match &inner.path {
            Some(path) => {
                let path = path.clone();
                drop(inner);
                append(&path, &[text]);
            }
            None => inner.pending.push(text),
        }
    }
}

/// Append lines; a record that cannot be written is a record lost, never
/// a turn lost — the chat goes on.
fn append(path: &Path, lines: &[String]) {
    if lines.is_empty() {
        return;
    }
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        for line in lines {
            let _ = writeln!(file, "{line}");
        }
    }
}

/// Fold every record under `<data_dir>/sessions` into the index — the
/// same incremental walk claude's files get (offset + mtime; a file that
/// shrank is refolded). Returns how many files had new lines.
pub fn refresh(data_dir: &Path, store: &mut (impl SessionStore + ?Sized)) -> anyhow::Result<usize> {
    let mut changed = 0;
    for path in record_files(data_dir) {
        if index_file(&path, store)? {
            changed += 1;
        }
    }
    Ok(changed)
}

fn record_files(data_dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let Ok(agents) = std::fs::read_dir(data_dir.join("sessions")) else {
        return files;
    };
    for agent in agents.flatten() {
        let Ok(records) = std::fs::read_dir(agent.path()) else { continue };
        files.extend(
            records
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|x| x == "jsonl")),
        );
    }
    files.sort();
    files
}

fn index_file(path: &Path, store: &mut (impl SessionStore + ?Sized)) -> anyhow::Result<bool> {
    let path_str = path.to_string_lossy().to_string();
    let meta = std::fs::metadata(path)?;
    let mtime = meta
        .modified()
        .ok()
        .and_then(|m| m.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let len = meta.len();
    let stem = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let (summary, offset) = match store.file_state(&path_str)? {
        // Same size AND same second: nothing new. (Size too — two appends
        // within one second share an mtime.)
        Some((_, offset, stored_mtime)) if stored_mtime == mtime && offset == len => return Ok(false),
        Some((_, offset, _)) if offset > len => (SessionSummary::new(stem), 0),
        Some((summary, offset, _)) => (summary, offset),
        None => (SessionSummary::new(stem), 0),
    };

    let mut reader = BufReader::new(std::fs::File::open(path)?);
    reader.seek(SeekFrom::Start(offset))?;
    let mut rows = Vec::new();
    let summary = reader
        .lines()
        .map_while(Result::ok)
        .filter_map(|line| recorded::parse(&line))
        .fold(summary, |summary, line| {
            if let Line::Usage(u) = &line {
                rows.extend(recorded::spend_row(u, &summary.session_id, summary.cwd.as_deref()));
            }
            recorded::fold(summary, &line)
        });
    store.record_spend(&rows)?;
    store.save_file_state(&path_str, &summary, len, mtime)?;
    Ok(true)
}

/// The indexer seam for records: `projects_dir` is claude's and ignored.
pub struct RecordedHistory {
    pub data_dir: PathBuf,
}

impl HistoryIndexer for RecordedHistory {
    fn refresh(&self, _projects_dir: &Path, store: &mut dyn SessionStore) -> anyhow::Result<usize> {
        refresh(&self.data_dir, store)
    }
}

/// Both histories, one seam: claude's files and Hark's records.
pub struct Histories<'a>(pub Vec<&'a dyn HistoryIndexer>);

impl HistoryIndexer for Histories<'_> {
    fn refresh(&self, projects_dir: &Path, store: &mut dyn SessionStore) -> anyhow::Result<usize> {
        let mut total = 0;
        for indexer in &self.0 {
            total += indexer.refresh(projects_dir, store)?;
        }
        Ok(total)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::sqlite_store::SqliteStore;
    use crate::domain::spend::SpendSource;
    use crate::ports::{SpendGroup, SpendLedger as _, SpendQuery};

    /// The file-fed token rows of one session, as the ledger aggregates them.
    fn jsonl_rows(store: &SqliteStore, session_id: &str) -> Vec<crate::ports::SpendAgg> {
        store
            .spend_summary(&SpendQuery { since: None, group: SpendGroup::Session, source: SpendSource::Jsonl, workspace: None })
            .unwrap()
            .into_iter()
            .filter(|a| a.key == session_id)
            .collect()
    }
    use crate::domain::transcript;
    use hark_agent::TokenUsage;

    fn usage(model: &str) -> Usage {
        Usage { ts: now_iso(), model: model.into(), usage: TokenUsage { input: 10, output: 5, cache_read: 0, cache_created: 0 }, cost_usd: Some(0.01) }
    }

    #[test]
    fn a_session_is_recorded_from_its_first_word_and_indexed_by_its_id() {
        let dir = tempfile::tempdir().unwrap();
        let rec = Recorder::new(dir.path(), "gemini", Path::new("/p/hark"));
        // The opening prompt is said before the agent answers with an id.
        rec.user("abre o PR do webhook");
        rec.started("s-1");
        rec.entry(Entry { ts: now_iso(), role: Role::Assistant, text: "abrindo".into(), tool: None, is_error: false });
        rec.usage(usage("gemini-2.5-pro"));

        let path = session_file(dir.path(), "gemini", "s-1");
        assert_eq!(rec.path(), Some(path.clone()));
        let text = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 4, "{text}");
        assert!(matches!(recorded::parse(lines[0]), Some(Line::Header(h)) if h.cwd == "/p/hark" && h.agent == "gemini" && h.session_id == "s-1"));
        // The viewer, the mirror and the brief read it with the reader they have.
        let entries = transcript::tail_entries(text.lines(), 10);
        assert_eq!(entries.iter().map(|e| e.role).collect::<Vec<_>>(), vec![Role::User, Role::Assistant]);
        assert!(!transcript::brief(&entries, 500).is_empty());

        // The index knows the session by its id, titled by its first prompt.
        let mut store = SqliteStore::in_memory().unwrap();
        assert_eq!(refresh(dir.path(), &mut store).unwrap(), 1);
        assert_eq!(store.session_path("s-1").unwrap().as_deref(), Some(path.to_str().unwrap()));
        let (summary, _, _) = store.file_state(path.to_str().unwrap()).unwrap().expect("indexed");
        assert_eq!(summary.title.as_deref(), Some("abre o PR do webhook"));
        assert_eq!(summary.cwd.as_deref(), Some("/p/hark"));
        assert!(store.search_sessions(&["webhook".into()], 5).unwrap().iter().any(|h| h.session_id == "s-1"));
        // The tokens landed as file-fed rows (source jsonl, no dollars), so
        // the machine-wide token table counts this session; the live rows
        // of the same turn are the ledger's dollars, and never summed with these.
        let rows = jsonl_rows(&store, "s-1");
        assert_eq!(rows.len(), 1, "{rows:?}");
        assert_eq!(rows[0].turns, 1);
        assert_eq!(rows[0].usage.input, 10);
        assert_eq!(rows[0].cost_usd, 0.0);
        assert!(store.spend_rows_session("s-1").unwrap().is_empty(), "no live row was written by the index");
    }

    #[test]
    fn a_second_refresh_is_idle_until_new_lines_arrive() {
        let dir = tempfile::tempdir().unwrap();
        let rec = Recorder::new(dir.path(), "codex", Path::new("/p/x"));
        rec.started("s-2");
        rec.user("primeira");
        let mut store = SqliteStore::in_memory().unwrap();
        assert_eq!(refresh(dir.path(), &mut store).unwrap(), 1);
        assert_eq!(refresh(dir.path(), &mut store).unwrap(), 0, "nothing new, nothing folded");
        // Appended within the same second: the size says so even when the mtime cannot.
        rec.user("segunda");
        assert_eq!(refresh(dir.path(), &mut store).unwrap(), 1);
        let path = session_file(dir.path(), "codex", "s-2");
        let (summary, _, _) = store.file_state(path.to_str().unwrap()).unwrap().unwrap();
        assert_eq!(summary.recent_prompts.len(), 2);
        assert_eq!(summary.last_prompt.as_deref(), Some("segunda"));
        // A usage line rebuilt twice is one row (the synthetic request id).
        rec.usage(usage("gpt-5"));
        refresh(dir.path(), &mut store).unwrap();
        refresh(dir.path(), &mut store).unwrap();
        assert_eq!(jsonl_rows(&store, "s-2").first().map(|a| a.turns), Some(1));
    }

    #[test]
    fn reopening_a_recorded_session_appends_under_the_same_header() {
        let dir = tempfile::tempdir().unwrap();
        Recorder::new(dir.path(), "gemini", Path::new("/p/hark")).started("s-3");
        let again = Recorder::new(dir.path(), "gemini", Path::new("/p/hark"));
        again.started("s-3");
        again.user("de volta");
        let text = std::fs::read_to_string(session_file(dir.path(), "gemini", "s-3")).unwrap();
        assert_eq!(text.lines().filter(|l| l.contains(r#""hark":"session""#)).count(), 1, "{text}");
        assert_eq!(text.lines().count(), 2);
    }

    #[test]
    fn an_id_with_a_slash_cannot_escape_the_sessions_dir() {
        let path = session_file(Path::new("/d"), "gemini", "../../etc/passwd");
        assert_eq!(path, Path::new("/d/sessions/gemini/.._.._etc_passwd.jsonl"));
    }

    #[test]
    fn both_histories_answer_as_one() {
        let dir = tempfile::tempdir().unwrap();
        Recorder::new(dir.path(), "gemini", Path::new("/p")).started("s-4");
        let recorded = RecordedHistory { data_dir: dir.path().to_path_buf() };
        let mut store = SqliteStore::in_memory().unwrap();
        // The claude side is stood in for by the recorded one twice: the seam sums.
        let both = Histories(vec![&recorded, &recorded]);
        assert_eq!(both.refresh(Path::new("/nowhere"), &mut store).unwrap(), 1);
    }
}

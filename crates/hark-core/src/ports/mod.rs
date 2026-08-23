//! Ports: traits implemented by adapters. The domain and app layers depend
//! only on these abstractions, never on concrete adapters.

use hark_agent::{AgentEvent, TurnResult};
use crate::domain::prompt::{LiveSession, RepoStatus};
use crate::domain::snapshot::SessionSummary;

/// One fast-mode turn to run.
pub struct TurnRequest<'a> {
    pub prompt: &'a str,
    /// Pasted screenshots: ordered (media_type, base64) blocks.
    pub images: &'a [(String, String)],
    /// Model alias chosen by the router or the user.
    pub model: &'a str,
}

/// Runs one question through the agent backend, streaming events.
pub trait AgentRunner {
    fn ask(
        &self,
        request: &TurnRequest,
        on_event: &mut dyn FnMut(&AgentEvent),
    ) -> anyhow::Result<TurnResult>;
}

/// Refreshes the on-disk session-history index before a snapshot. The
/// agent plugin implements this — it is the one that knows the backend's
/// history layout; the core only consumes neutral SessionEvents.
pub trait HistoryIndexer {
    fn refresh(
        &self,
        projects_dir: &std::path::Path,
        store: &mut dyn SessionStore,
    ) -> anyhow::Result<usize>;
}

/// The persistent token/cost ledger. Every Claude turn lands here — the
/// window closing must never erase what was spent.
pub trait SpendLedger {
    fn record_spend(&mut self, rows: &[crate::domain::spend::SpendRow]) -> anyhow::Result<()>;
    /// Aggregate over a window. USD aggregations must filter source=live;
    /// token aggregations source=jsonl (never sum across sources).
    fn spend_summary(&self, query: &SpendQuery) -> anyhow::Result<Vec<SpendAgg>>;
    /// Most expensive sessions in USD (live rows), newest window first.
    fn spend_top_sessions(&self, since: &str, limit: usize) -> anyhow::Result<Vec<SpendAgg>>;
    /// Raw ledger rows since an ISO instant, oldest first — the export
    /// path (`hark spend --export`), for team-side aggregation.
    fn spend_rows(
        &self,
        since: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<crate::domain::spend::SpendRow>>;
}

/// Ledger aggregation request.
pub struct SpendQuery {
    /// ISO-8601 lower bound (inclusive); None = everything.
    pub since: Option<String>,
    pub group: SpendGroup,
    pub source: crate::domain::spend::SpendSource,
    /// Project root: keeps only rows whose workspace is that directory or
    /// lives under it. None = every project.
    pub workspace: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpendGroup {
    Kind,
    Model,
    Label,
    Workspace,
    Day,
    Session,
    /// By task id — the mother's chat ("hark-chat") reads its own line.
    Task,
}

/// What the newest turn of a session weighs, split into the parts that
/// occupy the context window (output is charged but not carried).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ContextWeight {
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_created: u64,
    /// input + cache_read + cache_created: what the next turn drags along.
    pub total: u64,
    pub context_window: Option<u64>,
}

/// One aggregated ledger bucket.
#[derive(Debug, Clone, PartialEq)]
pub struct SpendAgg {
    pub key: String,
    pub cost_usd: f64,
    pub usage: hark_agent::TokenUsage,
    pub turns: u64,
    pub errors: u64,
}

/// Persistent index of session summaries and file read offsets.
pub trait SessionStore: SpendLedger {
    /// Stored fold state for a log file: (summary so far, byte offset, mtime).
    fn file_state(&self, path: &str) -> anyhow::Result<Option<(SessionSummary, u64, i64)>>;
    fn save_file_state(
        &mut self,
        path: &str,
        summary: &SessionSummary,
        offset: u64,
        mtime: i64,
    ) -> anyhow::Result<()>;
    /// Sessions with activity at or after the given ISO-8601 instant,
    /// newest first.
    fn sessions_since(&self, iso_ts: &str) -> anyhow::Result<Vec<SessionSummary>>;
    /// Sessions matching ANY of the topic terms (title, last prompt or any
    /// stored prompt), best match first, regardless of age.
    fn search_sessions(&self, terms: &[String], limit: usize) -> anyhow::Result<Vec<SessionSummary>>;
    /// Log file backing a session, for the read-only viewer.
    fn session_path(&self, session_id: &str) -> anyhow::Result<Option<String>>;
    /// The invisible kanban.
    fn board(&self) -> anyhow::Result<Vec<crate::domain::board::Task>>;
    fn save_board(&mut self, tasks: &[crate::domain::board::Task]) -> anyhow::Result<()>;
}

/// Discovers live Claude Code sessions.
pub trait LiveSessions {
    fn list(&self) -> anyhow::Result<Vec<LiveSession>>;
}

/// Collects working-tree status from configured repositories.
pub trait RepoCollector {
    fn collect(&self, repos: &[String]) -> Vec<RepoStatus>;
}

/// Hark's Q&A journal, one per root directory (`<root>/.hark/journal.md`).
pub trait Journal {
    fn tail(&self, root: &std::path::Path, n: usize) -> Vec<String>;
    fn append(&self, root: &std::path::Path, entry: &str) -> anyhow::Result<()>;
}

/// Records one utterance: 16kHz mono f32 until end-of-speech (VAD) or stop.
pub trait AudioIn {
    fn record_utterance(&self) -> anyhow::Result<Vec<f32>>;
}

/// Speech to text.
pub trait Stt {
    fn transcribe(&self, samples: &[f32]) -> anyhow::Result<String>;
}

/// Text to speech plus short cue sounds.
pub trait Tts {
    fn speak(&self, text: &str) -> anyhow::Result<()>;
    fn beep(&self, kind: Cue);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cue {
    /// Recording started, go ahead and talk.
    Listening,
    /// Utterance captured.
    Captured,
    /// Something failed.
    Error,
}

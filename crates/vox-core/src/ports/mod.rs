//! Ports: traits implemented by adapters. The domain and app layers depend
//! only on these abstractions, never on concrete adapters.

use crate::domain::claude_event::{ClaudeEvent, TurnResult};
use crate::domain::prompt::{LiveSession, RepoStatus};
use crate::domain::snapshot::SessionSummary;

/// One fast-mode turn to run.
pub struct TurnRequest<'a> {
    pub prompt: &'a str,
    /// Optional (media_type, base64) attachment.
    pub image: Option<(&'a str, &'a str)>,
    /// Model alias chosen by the router or the user.
    pub model: &'a str,
}

/// Runs one question through Claude, streaming events as they arrive.
pub trait AgentRunner {
    fn ask(
        &self,
        request: &TurnRequest,
        on_event: &mut dyn FnMut(&ClaudeEvent),
    ) -> anyhow::Result<TurnResult>;
}

/// Persistent index of session summaries and file read offsets.
pub trait SessionStore {
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

/// Vox's Q&A journal, one per root directory (`<root>/.vox/journal.md`).
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

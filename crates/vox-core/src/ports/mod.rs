//! Ports: traits implemented by adapters. The domain and app layers depend
//! only on these abstractions, never on concrete adapters.

use crate::domain::claude_event::{ClaudeEvent, TurnResult};
use crate::domain::prompt::{LiveSession, RepoStatus};
use crate::domain::snapshot::SessionSummary;

/// Runs one question through Claude, streaming events as they arrive.
pub trait AgentRunner {
    fn ask(
        &self,
        prompt: &str,
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
}

/// Discovers live Claude Code sessions.
pub trait LiveSessions {
    fn list(&self) -> anyhow::Result<Vec<LiveSession>>;
}

/// Collects working-tree status from configured repositories.
pub trait RepoCollector {
    fn collect(&self, repos: &[String]) -> Vec<RepoStatus>;
}

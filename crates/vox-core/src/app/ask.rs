//! Use case: answer one question with a fresh context snapshot.

use crate::adapters::jsonl_scan::refresh_index;
use crate::config::Config;
use crate::domain::claude_event::{ClaudeEvent, TurnResult};
use crate::domain::prompt::{self, Snapshot};
use crate::ports::{AgentRunner, LiveSessions, RepoCollector, SessionStore};
use chrono::{Duration, SecondsFormat, Utc};

pub struct AskDeps<'a> {
    pub config: &'a Config,
    pub store: &'a mut dyn SessionStore,
    pub live: &'a dyn LiveSessions,
    pub repos: &'a dyn RepoCollector,
    pub runner: &'a dyn AgentRunner,
}

/// Refresh the index, assemble the snapshot, ask Claude.
pub fn ask(
    question: &str,
    deps: &mut AskDeps,
    on_event: &mut dyn FnMut(&ClaudeEvent),
) -> anyhow::Result<TurnResult> {
    let snapshot = build_snapshot(deps)?;
    let prompt = prompt::build(question, &snapshot);
    deps.runner.ask(&prompt, on_event)
}

/// Index refresh + context gathering, shared with future `sessions` command.
pub fn build_snapshot(deps: &mut AskDeps) -> anyhow::Result<Snapshot> {
    refresh_index(&deps.config.projects_dir, deps.store)?;
    let since = (Utc::now() - Duration::hours(deps.config.hours_back))
        .to_rfc3339_opts(SecondsFormat::Millis, true);
    Ok(Snapshot {
        generated_at: Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
        sessions: deps.store.sessions_since(&since)?,
        live: deps.live.list().unwrap_or_default(),
        repos: deps.repos.collect(&deps.config.repos),
    })
}

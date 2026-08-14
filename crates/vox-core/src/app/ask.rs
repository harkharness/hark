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
/// The lookback window adapts to the question ("hoje", "semana", "mês").
pub fn ask(
    question: &str,
    deps: &mut AskDeps,
    on_event: &mut dyn FnMut(&ClaudeEvent),
) -> anyhow::Result<TurnResult> {
    let hours = crate::domain::intent::window_hours(question, deps.config.hours_back);
    let snapshot = build_snapshot_hours(deps, hours)?;
    let prompt = prompt::build(question, &snapshot);
    deps.runner.ask(&prompt, on_event)
}

/// Index refresh + context gathering, shared with the `sessions` command.
pub fn build_snapshot(deps: &mut AskDeps) -> anyhow::Result<Snapshot> {
    let hours = deps.config.hours_back;
    build_snapshot_hours(deps, hours)
}

fn build_snapshot_hours(deps: &mut AskDeps, hours: i64) -> anyhow::Result<Snapshot> {
    refresh_index(&deps.config.projects_dir, deps.store)?;
    let since =
        (Utc::now() - Duration::hours(hours)).to_rfc3339_opts(SecondsFormat::Millis, true);
    Ok(Snapshot {
        generated_at: Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
        sessions: deps.store.sessions_since(&since)?,
        live: deps.live.list().unwrap_or_default(),
        repos: deps.repos.collect(&deps.config.repos),
    })
}

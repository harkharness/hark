//! Use case: answer one question with a fresh context snapshot.

use crate::adapters::jsonl_scan::refresh_index;
use crate::config::Config;
use crate::domain::claude_event::{ClaudeEvent, TurnResult};
use crate::domain::context::{self, ContextDef};
use crate::domain::prompt::{self, Snapshot};
use crate::ports::{AgentRunner, LiveSessions, RepoCollector, SessionStore};
use chrono::{Duration, SecondsFormat, Utc};

/// How many journal entries feed back into the snapshot.
const JOURNAL_TAIL: usize = 8;

pub struct AskDeps<'a> {
    pub config: &'a Config,
    /// Context chosen via `vox use` (from global state), if any.
    pub active_context: Option<String>,
    /// Machine-wide dispatched workers (loaded from global state).
    pub workers: Vec<crate::domain::memory::WorkerRecord>,
    pub store: &'a mut dyn SessionStore,
    pub live: &'a dyn LiveSessions,
    pub repos: &'a dyn RepoCollector,
    pub journal: &'a dyn crate::ports::Journal,
    pub runner: &'a dyn AgentRunner,
}

/// Refresh the index, assemble the snapshot, ask Claude, journal the answer.
/// The lookback window adapts to the question ("hoje", "semana", "mês");
/// the context comes from a question hint, else `vox use`, else the default.
pub fn ask(
    question: &str,
    deps: &mut AskDeps,
    on_event: &mut dyn FnMut(&ClaudeEvent),
) -> anyhow::Result<TurnResult> {
    let context = resolve_context(deps, Some(question));
    let snapshot = snapshot_for_question(deps, question)?;
    let prompt = prompt::build(question, &snapshot);
    let result = deps.runner.ask(&prompt, on_event)?;

    if let Some(reply) = &result.reply {
        let now = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
        let entry = crate::domain::memory::journal_entry(&now, question, &reply.fala);
        // Journaling must never break the answer flow.
        let _ = deps.journal.append(&journal_root(deps, context.as_ref()), &entry);
    }
    Ok(result)
}

/// Where the resolved context keeps its `.vox/`: first repo of the context,
/// or the global data dir when unfocused ("all").
fn journal_root(deps: &AskDeps, context: Option<&ContextDef>) -> std::path::PathBuf {
    context
        .and_then(|c| c.repos.first().cloned())
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| deps.config.data_dir())
}

/// Snapshot exactly as `ask` would assemble it (window + context from the
/// question). Also used by the `prompt` debug command.
pub fn snapshot_for_question(deps: &mut AskDeps, question: &str) -> anyhow::Result<Snapshot> {
    let hours = crate::domain::intent::window_hours(question, deps.config.hours_back);
    let context = resolve_context(deps, Some(question));
    build_snapshot_with(deps, hours, context.as_ref())
}

/// Snapshot for dispatch target hunting: context from the instruction,
/// explicit (wide) window.
pub fn snapshot_for_target(
    deps: &mut AskDeps,
    instruction: &str,
    hours: i64,
) -> anyhow::Result<Snapshot> {
    let context = resolve_context(deps, Some(instruction));
    build_snapshot_with(deps, hours, context.as_ref())
}

/// Snapshot with NO context filter (used when the user names a session
/// explicitly; an explicit id must never be hidden by the active context).
pub fn snapshot_unfiltered(deps: &mut AskDeps, hours: i64) -> anyhow::Result<Snapshot> {
    build_snapshot_with(deps, hours, None)
}

/// hint in the question > active (vox use) > config default > None ("all").
fn resolve_context(deps: &AskDeps, question: Option<&str>) -> Option<ContextDef> {
    let names = deps.config.context_names();
    let hinted = question.and_then(|q| context::hint(q, &names));
    let name = hinted
        .or_else(|| deps.active_context.clone())
        .unwrap_or_else(|| deps.config.default_context.clone());
    deps.config.context(&name)
}

/// Index refresh + context gathering, shared with the `sessions` command.
pub fn build_snapshot(deps: &mut AskDeps) -> anyhow::Result<Snapshot> {
    let hours = deps.config.hours_back;
    let context = resolve_context(deps, None);
    build_snapshot_with(deps, hours, context.as_ref())
}

fn build_snapshot_with(
    deps: &mut AskDeps,
    hours: i64,
    context: Option<&ContextDef>,
) -> anyhow::Result<Snapshot> {
    refresh_index(&deps.config.projects_dir, deps.store)?;
    let since =
        (Utc::now() - Duration::hours(hours)).to_rfc3339_opts(SecondsFormat::Millis, true);
    let sessions = deps
        .store
        .sessions_since(&since)?
        .into_iter()
        .filter(|s| context.is_none_or(|c| c.matches(s.cwd.as_deref())))
        .collect();
    let repo_paths = context.map_or_else(|| deps.config.repos.clone(), |c| c.repos.clone());
    Ok(Snapshot {
        generated_at: Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
        sessions,
        live: deps.live.list().unwrap_or_default(),
        repos: deps.repos.collect(&repo_paths),
        journal: deps.journal.tail(&journal_root(deps, context), JOURNAL_TAIL),
        workers: deps.workers.clone(),
    })
}

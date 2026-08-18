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
    /// Context chosen via `vox use` (from global state), if any (CLI legacy).
    pub active_context: Option<String>,
    /// Registered projects; their names work as one-turn hints in questions.
    pub projects: Vec<crate::domain::project::Project>,
    /// Project the UI is focused on right now: clicking a chat, resuming a
    /// session or starting a new one sets this — the scope follows the work.
    pub active_project: Option<crate::domain::project::Project>,
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
    ask_with_image(question, &[], deps, on_event)
}

/// `ask` with any pasted screenshots (media type + base64, in order).
/// The model is routed per question (explicit request > heuristic > default).
pub fn ask_with_image(
    question: &str,
    images: &[(String, String)],
    deps: &mut AskDeps,
    on_event: &mut dyn FnMut(&ClaudeEvent),
) -> anyhow::Result<TurnResult> {
    let context = resolve_context(deps, Some(question));

    // THE THREE LAYERS: local (zero tokens) > mini-format (light model,
    // minimal context) > full snapshot. Images always take the full path.
    let plan = if !images.is_empty() {
        crate::domain::answer::AnswerPlan::FullAsk
    } else {
        let board = deps.store.board().unwrap_or_default();
        let facts = crate::domain::answer::LocalFacts {
            board: &board,
            workers: &deps.workers,
            spend_day_usd: spent_since(deps, 24),
            spend_week_usd: spent_since(deps, 24 * 7),
        };
        crate::domain::answer::plan_answer(question, &facts)
    };

    if let crate::domain::answer::AnswerPlan::Local(reply) = plan {
        return Ok(finish_local(question, reply, deps, &context));
    }

    let (prompt, model, outcome) = match &plan {
        crate::domain::answer::AnswerPlan::MiniFormat { context: mini } => (
            crate::domain::answer::mini_prompt(mini, question),
            deps.config.models().light,
            Some("mini_format"),
        ),
        _ => {
            let (snapshot, topical) = snapshot_for_question(deps, question)?;
            let budget = prompt::PromptBudget {
                max_chars: deps.config.prompt_budget_chars,
            };
            (
                prompt::build_budgeted(question, &snapshot, &topical, &budget),
                crate::domain::intent::model_for(question, &deps.config.models()),
                None,
            )
        }
    };
    let request = crate::ports::TurnRequest {
        prompt: &prompt,
        images,
        model: &model,
    };
    let mut turn_session: Option<String> = None;
    let result = deps.runner.ask(&request, &mut |event| {
        if let ClaudeEvent::SessionStarted(id) = event {
            turn_session = Some(id.clone());
        }
        on_event(event);
    })?;

    // Ledger first, reply later: failed turns burn tokens too.
    {
        let now = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
        let meta = crate::domain::spend::SpendMeta {
            label: Some("vox (perguntas)"),
            session_id: turn_session.as_deref(),
            workspace: context.as_ref().and_then(|c| c.repos.first()).map(String::as_str),
            outcome,
            ..Default::default()
        };
        let rows = crate::domain::spend::rows_from_turn(
            &now,
            crate::domain::spend::SpendKind::Ask,
            &meta,
            &result,
        );
        let _ = deps.store.record_spend(&rows);
    }

    if let Some(reply) = &result.reply {
        let now = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
        let entry = crate::domain::memory::journal_entry(&now, question, &reply.fala);
        // Journaling and board updates must never break the answer flow.
        let _ = deps.journal.append(&journal_root(deps, context.as_ref()), &entry);
        if !reply.board.is_empty() {
            let workspace = context.as_ref().and_then(|c| c.repos.first().cloned());
            let _ = update_board(deps, &reply.board, &now, workspace.as_deref(), None);
        }
    }
    Ok(result)
}

/// Measured USD in the last N hours (live rows only), for local answers.
fn spent_since(deps: &AskDeps, hours: i64) -> Option<f64> {
    let since = (Utc::now() - Duration::hours(hours)).to_rfc3339();
    deps.store
        .spend_summary(&crate::ports::SpendQuery {
            since: Some(since),
            group: crate::ports::SpendGroup::Kind,
            source: crate::domain::spend::SpendSource::Live,
            workspace: None,
        })
        .ok()
        .map(|aggs| aggs.iter().map(|a| a.cost_usd).sum())
}

/// A layer-1 answer: journal it, ledger it (cost zero, model "local"),
/// and hand back a synthetic turn that walks the normal pipeline.
fn finish_local(
    question: &str,
    reply: crate::domain::claude_event::VoiceReply,
    deps: &mut AskDeps,
    context: &Option<ContextDef>,
) -> TurnResult {
    let now = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
    let entry = crate::domain::memory::journal_entry(&now, question, &reply.fala);
    let _ = deps.journal.append(&journal_root(deps, context.as_ref()), &entry);
    let turn = TurnResult {
        is_error: false,
        raw: reply.fala.clone(),
        reply: Some(reply),
        cost_usd: Some(0.0),
        duration_ms: Some(0),
        model: Some("local".into()),
        usage: vec![crate::domain::claude_event::ModelUsage {
            model: "local".into(),
            usage: crate::domain::claude_event::TokenUsage::default(),
            cost_usd: Some(0.0),
            context_window: None,
        }],
    };
    let rows = crate::domain::spend::rows_from_turn(
        &now,
        crate::domain::spend::SpendKind::Ask,
        &crate::domain::spend::SpendMeta {
            label: Some("vox (local)"),
            outcome: Some("local"),
            ..Default::default()
        },
        &turn,
    );
    let _ = deps.store.record_spend(&rows);
    turn
}

/// Merge board updates and persist (used by ask and by the dispatcher).
pub fn update_board(
    deps: &mut AskDeps,
    updates: &[crate::domain::board::BoardUpdate],
    now: &str,
    workspace: Option<&str>,
    session_id: Option<&str>,
) -> anyhow::Result<()> {
    let merged = crate::domain::board::apply_updates(
        deps.store.board()?,
        updates,
        now,
        workspace,
        session_id,
    );
    deps.store.save_board(&merged)
}

/// Where the resolved context keeps its `.vox/`: first repo of the context,
/// or the global data dir when unfocused ("all").
fn journal_root(deps: &AskDeps, context: Option<&ContextDef>) -> std::path::PathBuf {
    context
        .and_then(|c| c.repos.first().cloned())
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| deps.config.data_dir())
}

/// Topic-matched sessions merged into the snapshot regardless of age.
const TOPIC_HITS: usize = 5;

/// Snapshot exactly as `ask` would assemble it: recent window + context
/// filter + topic search over the FULL index (a question about "webhook"
/// must surface the webhook sessions even if untouched for weeks).
/// Returns the topic-hit ids too: those sessions exist BECAUSE the
/// question asked, so the budget cut must spare them first.
/// Also used by the `prompt` debug command.
pub fn snapshot_for_question(
    deps: &mut AskDeps,
    question: &str,
) -> anyhow::Result<(Snapshot, Vec<String>)> {
    let hours = crate::domain::intent::window_hours(question, deps.config.hours_back);
    let context = resolve_context(deps, Some(question));
    let mut snapshot = build_snapshot_with(deps, hours, context.as_ref())?;

    let terms = crate::domain::dispatch::significant_terms(question);
    let topical = deps.store.search_sessions(&terms, TOPIC_HITS)?;
    let mut topical_ids = Vec::new();
    for session in topical {
        let in_context = context.as_ref().is_none_or(|c| c.matches(session.cwd.as_deref()));
        if !in_context {
            continue;
        }
        topical_ids.push(session.session_id.clone());
        let already_in = snapshot.sessions.iter().any(|s| s.session_id == session.session_id);
        if !already_in {
            snapshot.sessions.push(session);
        }
    }
    Ok((snapshot, topical_ids))
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

/// hint in the question (project or config context) > focused project >
/// active (vox use, CLI legacy) > config default > None ("all").
fn resolve_context(deps: &AskDeps, question: Option<&str>) -> Option<ContextDef> {
    use crate::domain::project;
    let mut names = deps.config.context_names();
    names.extend(deps.projects.iter().map(|p| p.name.clone()));
    if let Some(hinted) = question.and_then(|q| context::hint(q, &names)) {
        if let Some(p) = project::find(&deps.projects, &hinted) {
            return Some(project::as_context(p));
        }
        if let Some(c) = deps.config.context(&hinted) {
            return Some(c);
        }
    }
    if let Some(p) = &deps.active_project {
        return Some(project::as_context(p));
    }
    let name = deps
        .active_context
        .clone()
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
        board: crate::domain::board::render(&deps.store.board().unwrap_or_default()),
    })
}

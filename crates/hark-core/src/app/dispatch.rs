//! Use case: route an instruction to an existing session (the orchestrator).

use crate::app::ask::AskDeps;
use crate::domain::dispatch::{resolve_target, Target};
use crate::domain::snapshot::SessionSummary;

/// A dispatch ready to execute: everything the shell needs to spawn a worker.
#[derive(Debug, Clone)]
pub struct Planned {
    pub session: SessionSummary,
    pub workspace_root: std::path::PathBuf,
}

#[derive(Debug)]
pub enum Plan {
    Ready(Planned),
    /// Candidates for the user to pick from (best first).
    NeedsChoice(Vec<SessionSummary>),
    /// Target session is open in an interactive terminal right now.
    TargetBusy(SessionSummary),
    NoMatch,
}

/// Search window when hunting for the target session: wide by design, the
/// task you're resuming is often days old.
const TARGET_WINDOW_HOURS: i64 = 24 * 14;

/// Plan a dispatch: find the target session in the resolved context and make
/// sure it is safe to resume (not currently open anywhere).
pub fn plan(
    deps: &mut AskDeps,
    instruction: &str,
    session_override: Option<&str>,
) -> anyhow::Result<Plan> {
    // An explicit --session must never be hidden by the active context.
    let snapshot = match session_override {
        Some(_) => super::ask::snapshot_unfiltered(deps, TARGET_WINDOW_HOURS)?,
        None => super::ask::snapshot_for_target(deps, instruction, TARGET_WINDOW_HOURS)?,
    };

    let chosen_id = match session_override {
        Some(id) => Some(id.to_string()),
        None => match resolve_target(instruction, &snapshot.sessions) {
            Target::Chosen(id) => Some(id),
            Target::Ambiguous(ids) => {
                let candidates = ids
                    .iter()
                    .filter_map(|id| snapshot.sessions.iter().find(|s| &s.session_id == id))
                    .cloned()
                    .collect();
                return Ok(Plan::NeedsChoice(candidates));
            }
            Target::None => None,
        },
    };
    let Some(chosen_id) = chosen_id else {
        return Ok(Plan::NoMatch);
    };
    let Some(session) = snapshot
        .sessions
        .iter()
        .find(|s| s.session_id == chosen_id)
        .cloned()
    else {
        return Ok(Plan::NoMatch);
    };

    // Refuse sessions currently open in an interactive terminal: two writers
    // on one session file is undefined behavior (see spikes/FINDINGS.md).
    // ONE rule decides who holds a session (domain::owner) — matching any
    // listed session also caught our own headless runs, which hold nothing.
    let live = deps.live.list().unwrap_or_default();
    let owners = crate::domain::owner::terminal_owners(&live, &[]);
    let busy = crate::domain::owner::owner_of(&owners, &session.session_id).is_some();
    let Some(cwd) = session.cwd.clone() else {
        return Ok(Plan::NoMatch);
    };
    if busy {
        return Ok(Plan::TargetBusy(session));
    }
    Ok(Plan::Ready(Planned {
        workspace_root: cwd.into(),
        session,
    }))
}

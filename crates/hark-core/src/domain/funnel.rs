//! The spoken-utterance decision funnel, as pure logic.
//!
//! This is the order that decides what a sentence DOES — the same order the
//! HUD runs (src-tauri/src/voice.rs::plan_utterance): explicit address wins,
//! then questions, then the chat on screen, then search. It lives here so
//! the voice corpus (tests/voice_corpus.rs) exercises the real funnel, not a
//! copy that drifts: the 24/08 incident (10 turns, $0.36, zero work) was a
//! routing bug nobody could have caught without replaying real sentences.
//!
//! Local commands (task_command::parse) and permission verdicts run BEFORE
//! this in the shell; both are already pure and separately tested.

use super::board::Task;
use super::matching::{self, Match};
use super::project::Project;

/// A session candidate as the funnel sees it (the index row, no I/O).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionLead {
    pub session_id: String,
    pub title: String,
    pub cwd: Option<String>,
    pub last_ts: Option<String>,
    pub last_prompt: Option<String>,
}

/// Everything the funnel may consult. Data in, decision out — the shell
/// fills this from the board/index/window state; tests fill it by hand.
pub struct World<'a> {
    /// The chat on screen: (task title, session id).
    pub active: Option<(&'a str, &'a str)>,
    /// The active window's project, for Work plans bound to the context.
    pub active_project: Option<(&'a str, &'a str)>,
    pub board: &'a [Task],
    pub projects: &'a [Project],
    /// Topic search over the session index (shell: SQL recall; the funnel
    /// re-ranks with the shared scorer for precision).
    pub search: &'a dyn Fn(&str) -> Vec<SessionLead>,
}

/// One pickable destination when the funnel cannot decide alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub title: String,
    pub session_id: Option<String>,
    pub workspace: Option<String>,
}

/// Where one utterance lands. Mirrors the HUD's VoicePlan minus the parts
/// that need live state (permissions, prechecks, command payloads).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Plan {
    /// Read-only question for the ask pipeline.
    Question { question: String },
    /// Real work with a resolved destination.
    Work {
        instruction: String,
        task_title: Option<String>,
        session_id: Option<String>,
        workspace: Option<String>,
        /// True = fresh session in a project (no task addressed).
        new_task: bool,
        /// "high" = on-screen chat or unique explicit address (silence
        /// confirms); "low" = search-resolved (spoken yes required).
        confidence: &'static str,
    },
    /// Too close to call: options, never a silent guess (19/08 incident).
    Candidates { instruction: String, options: Vec<Candidate> },
    /// Work with no resolvable destination: the HUD asks for an address.
    NoTarget { instruction: String },
}

fn task_option(task: &Task) -> Candidate {
    Candidate {
        title: task.title.clone(),
        session_id: task.session_ids.last().cloned(),
        workspace: task.workspace.clone(),
    }
}

fn session_option(hit: &SessionLead) -> Candidate {
    Candidate {
        title: hit.title.clone(),
        session_id: Some(hit.session_id.clone()),
        workspace: hit.cwd.clone(),
    }
}

/// Rank session candidates with the shared scorer — title, path and last
/// prompt are the haystack, recency only breaks exact ties.
pub fn rank_sessions(query: &str, hits: Vec<SessionLead>) -> Match<SessionLead> {
    matching::rank(
        query,
        hits,
        |h| {
            format!(
                "{} {} {}",
                h.title,
                h.cwd.as_deref().unwrap_or_default(),
                h.last_prompt.as_deref().unwrap_or_default()
            )
        },
        |h| h.last_ts.clone().unwrap_or_default(),
    )
}

fn work_at_task(task: &Task, instruction: String, confidence: &'static str) -> Plan {
    Plan::Work {
        instruction,
        task_title: Some(task.title.clone()),
        session_id: task.session_ids.last().cloned(),
        workspace: task.workspace.clone(),
        new_task: false,
        confidence,
    }
}

fn work_at_session(hit: &SessionLead, instruction: String, confidence: &'static str) -> Plan {
    Plan::Work {
        instruction,
        task_title: Some(hit.title.clone()),
        session_id: Some(hit.session_id.clone()),
        workspace: hit.cwd.clone(),
        new_task: false,
        confidence,
    }
}

/// The funnel: address → question → active chat → search.
pub fn plan(text: &str, world: &World) -> Plan {
    // 1. Explicit address: "na task X…", "no projeto Y…". Board first
    //    (titles the user knows), then the whole index. Ambiguity becomes
    //    options on screen, never a silent guess.
    let addr = super::address::parse(text);
    if let Some(task_query) = &addr.task {
        match super::board::find_ranked(world.board, task_query) {
            Match::Hit(task) => return work_at_task(&task, addr.instruction, "high"),
            Match::Ambiguous(tasks) => {
                return Plan::Candidates {
                    instruction: addr.instruction,
                    options: tasks.iter().map(task_option).collect(),
                }
            }
            Match::None => {}
        }
        return match rank_sessions(task_query, (world.search)(task_query)) {
            Match::Hit(hit) => work_at_session(&hit, addr.instruction, "high"),
            Match::Ambiguous(hits) => Plan::Candidates {
                instruction: addr.instruction,
                options: hits.iter().map(session_option).collect(),
            },
            Match::None => Plan::NoTarget { instruction: text.to_string() },
        };
    }
    if let Some(project_query) = &addr.project {
        let hit = super::project::find(world.projects, project_query)
            .or_else(|| super::project::find_spoken(world.projects, project_query));
        return match hit {
            Some(p) => Plan::Work {
                instruction: addr.instruction,
                task_title: None,
                session_id: None,
                workspace: Some(p.path.clone()),
                // A fresh session is never silent-confirmed.
                new_task: true,
                confidence: "low",
            },
            None => Plan::NoTarget { instruction: text.to_string() },
        };
    }

    // 2. No address: questions go to ask…
    if super::intent::route(text) == super::intent::Route::Ask {
        return Plan::Question { question: text.to_string() };
    }

    // 3. …work goes to the ACTIVE chat (what the user is looking at)…
    if let Some((title, session)) = world.active {
        return Plan::Work {
            instruction: text.to_string(),
            task_title: Some(title.to_string()),
            session_id: Some(session.to_string()),
            workspace: world.active_project.map(|(_, path)| path.to_string()),
            new_task: false,
            confidence: "high",
        };
    }

    // 4. …else to the best session match, spoken-yes required.
    match rank_sessions(text, (world.search)(text)) {
        Match::Hit(hit) => work_at_session(&hit, text.to_string(), "low"),
        Match::Ambiguous(hits) => Plan::Candidates {
            instruction: text.to_string(),
            options: hits.iter().map(session_option).collect(),
        },
        Match::None => Plan::NoTarget { instruction: text.to_string() },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::board::{Task, TaskStatus};

    fn task(title: &str, session: &str, ws: &str) -> Task {
        Task {
            title: title.into(),
            status: TaskStatus::Doing,
            workspace: Some(ws.into()),
            session_ids: vec![session.into()],
            updated_at: "2026-08-25T10:00:00Z".into(),
            note: None,
            pinned: false,
            subtasks: Vec::new(),
        }
    }

    fn lead(id: &str, title: &str) -> SessionLead {
        SessionLead {
            session_id: id.into(),
            title: title.into(),
            cwd: Some("/Users/dev/workspace-fabrica".into()),
            last_ts: Some("2026-08-24T10:00:00Z".into()),
            last_prompt: None,
        }
    }

    #[test]
    fn explicit_task_address_beats_everything() {
        let board = [task("Alertas do cluster migração", "s-alertas", "/w/fabrica")];
        let search = |_: &str| Vec::new();
        let world = World {
            active: Some(("Outra task", "s-outra")),
            active_project: None,
            board: &board,
            projects: &[],
            search: &search,
        };
        let got = plan("na task alertas do cluster roda os testes", &world);
        match got {
            Plan::Work { session_id, confidence, instruction, .. } => {
                assert_eq!(session_id.as_deref(), Some("s-alertas"));
                assert_eq!(confidence, "high");
                assert_eq!(instruction, "roda os testes");
            }
            other => panic!("expected work at the addressed task, got {other:?}"),
        }
    }

    #[test]
    fn questions_go_to_ask_even_with_a_chat_open() {
        let search = |_: &str| Vec::new();
        let world = World {
            active: Some(("Alertas", "s-1")),
            active_project: None,
            board: &[],
            projects: &[],
            search: &search,
        };
        assert_eq!(
            plan("quais as pendências de hoje?", &world),
            Plan::Question { question: "quais as pendências de hoje?".into() }
        );
    }

    #[test]
    fn unaddressed_work_lands_on_the_chat_on_screen() {
        let search = |_: &str| panic!("the active chat answers; search must not run");
        let world = World {
            active: Some(("Alertas do cluster", "s-alertas")),
            active_project: Some(("workspace-fabrica", "/w/fabrica")),
            board: &[],
            projects: &[],
            search: &search,
        };
        match plan("roda essa verificação de DNS", &world) {
            Plan::Work { session_id, confidence, workspace, .. } => {
                assert_eq!(session_id.as_deref(), Some("s-alertas"));
                assert_eq!(confidence, "high");
                assert_eq!(workspace.as_deref(), Some("/w/fabrica"));
            }
            other => panic!("expected work at the active chat, got {other:?}"),
        }
    }

    #[test]
    fn search_resolved_work_is_never_silent() {
        let search = |_: &str| vec![lead("s-webhook", "migração do webhook")];
        let world = World {
            active: None,
            active_project: None,
            board: &[],
            projects: &[],
            search: &search,
        };
        match plan("continua a migração do webhook", &world) {
            Plan::Work { confidence, session_id, .. } => {
                assert_eq!(confidence, "low", "search hit must require a spoken yes");
                assert_eq!(session_id.as_deref(), Some("s-webhook"));
            }
            other => panic!("expected search-resolved work, got {other:?}"),
        }
    }

    #[test]
    fn no_context_and_no_match_asks_for_an_address() {
        let search = |_: &str| Vec::new();
        let world = World {
            active: None,
            active_project: None,
            board: &[],
            projects: &[],
            search: &search,
        };
        assert_eq!(
            plan("roda os testes", &world),
            Plan::NoTarget { instruction: "roda os testes".into() }
        );
    }
}

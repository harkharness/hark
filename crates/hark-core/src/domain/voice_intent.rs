//! Turning natural speech into an ACTION.
//!
//! The first design classified intent with hand-written verb lists. It
//! cannot work: you cannot predict word order ("eu quero que você faça
//! X"), nor which conjugation someone reaches for ("atualiza" vs
//! "atualize"), nor how much context a request carries ("atualiza o
//! artefato daquela conversa"). Anything the lists missed fell through to
//! a prose answer — the mother described the work instead of doing it, at
//! four cents a turn.
//!
//! So the fallback stops being prose and becomes a CHEAP CLASSIFIER: the
//! light model, a schema, and a catalog of what actually exists on this
//! machine. It returns an action or an honest question, never a paragraph.
//!
//! Two invariants make it safe to act on:
//!   1. Every id the model returns is checked against the catalog. A
//!      session it invented is dropped, never opened.
//!   2. Ambiguity is a first-class answer. Two plausible sessions produce
//!      a question with candidates, never a coin flip.

use serde::{Deserialize, Serialize};

/// One session the model is allowed to choose.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogSession {
    pub id: String,
    pub title: String,
    pub project: Option<String>,
}

/// Everything that exists, as the model gets to see it. Nothing outside
/// this may be acted on.
#[derive(Debug, Clone, Default)]
pub struct IntentCatalog {
    /// Registered project names.
    pub projects: Vec<String>,
    /// Recent sessions, newest first.
    pub sessions: Vec<CatalogSession>,
    /// The window the user is looking at (resolves "nesse chat").
    pub focused_project: Option<String>,
    pub focused_session: Option<String>,
    /// Last lines of the spoken conversation, oldest first — this is what
    /// makes "eu disse sim" and "a última mensagem" resolvable at all.
    pub recent_exchange: Vec<String>,
}

/// What the classifier decided, after validation against the catalog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpokenPlan {
    /// Front a project's window; optionally dispatch into it.
    OpenProject {
        project: String,
        instruction: Option<String>,
    },
    /// Front an existing session; optionally dispatch into it.
    OpenSession {
        session_id: String,
        title: String,
        instruction: Option<String>,
    },
    /// Work in a session already resolved (focused, or named).
    Dispatch {
        session_id: Option<String>,
        instruction: String,
    },
    /// Read-only: let the normal ask answer it.
    Question,
    /// Cannot be resolved without the user. NEVER a guess.
    Clarify {
        question: String,
        options: Vec<CatalogSession>,
    },
}

/// Raw model output. Every field is optional on purpose: a model that
/// omits something must degrade into a question, not a panic.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct IntentDecision {
    pub kind: String,
    pub project: Option<String>,
    pub session_id: Option<String>,
    pub instruction: Option<String>,
    pub question: Option<String>,
    pub candidates: Vec<String>,
}

pub const INTENT_SYSTEM_PROMPT: &str = "\
You route ONE spoken sentence from a developer to an action in their local \
coding cockpit. You never do the work and never explain: you only decide \
where the sentence goes.\n\
Rules that matter more than being helpful:\n\
- Only ever name a session_id that appears in the catalog, copied exactly. \
If the right session is not in the catalog, do not invent one.\n\
- If two or more sessions plausibly match, answer kind=\"clarify\" with \
their ids in candidates and a short question. Guessing a target is the \
worst outcome available to you.\n\
- instruction must be the user's own request, in their words, with the \
addressing removed (\"no chat X, atualiza o artefato\" -> \"atualiza o \
artefato\"). Never expand it, never add steps.\n\
- kind=\"question\" is for anything read-only: status, history, spend, \
\"what was I doing\". Someone asking about work is not asking for work.\n\
- The sentence may refer to what was just said; the recent exchange is \
there for exactly that.";

/// Structured output contract. Names are English like the rest of the
/// plugin contract; the spoken language is irrelevant to the shape.
pub const INTENT_SCHEMA: &str = r#"{
  "type": "object",
  "properties": {
    "kind": {
      "type": "string",
      "enum": ["open_project", "open_session", "dispatch", "question", "clarify"],
      "description": "Where the sentence goes. 'dispatch' means work in the session already in focus."
    },
    "project": { "type": "string", "description": "Project name copied from the catalog." },
    "session_id": { "type": "string", "description": "Session id copied EXACTLY from the catalog. Never invented." },
    "instruction": { "type": "string", "description": "The work to do, in the user's own words, without the addressing." },
    "question": { "type": "string", "description": "What to ask the user when kind=clarify." },
    "candidates": {
      "type": "array",
      "items": { "type": "string" },
      "description": "Session ids from the catalog when more than one matches."
    }
  },
  "required": ["kind"]
}"#;

/// Render the catalog and the sentence. Compact on purpose: this runs on
/// the light tier and pays for itself only while it stays cheap.
pub fn build_prompt(utterance: &str, catalog: &IntentCatalog) -> String {
    let mut out = String::new();

    if !catalog.recent_exchange.is_empty() {
        out.push_str("## Conversa recente (mais antiga primeiro)\n");
        for line in catalog.recent_exchange.iter().rev().take(6).rev() {
            out.push_str(&format!("- {line}\n"));
        }
        out.push('\n');
    }

    out.push_str("## Onde a pessoa está agora\n");
    out.push_str(&format!(
        "- projeto em foco: {}\n- sessão em foco: {}\n\n",
        catalog.focused_project.as_deref().unwrap_or("nenhum"),
        catalog.focused_session.as_deref().unwrap_or("nenhuma"),
    ));

    if !catalog.projects.is_empty() {
        out.push_str("## Projetos registrados\n");
        for p in &catalog.projects {
            out.push_str(&format!("- {p}\n"));
        }
        out.push('\n');
    }

    if !catalog.sessions.is_empty() {
        out.push_str("## Sessões (id | título | projeto)\n");
        for s in catalog.sessions.iter().take(20) {
            out.push_str(&format!(
                "- {} | {} | {}\n",
                s.id,
                s.title,
                s.project.as_deref().unwrap_or("-")
            ));
        }
        out.push('\n');
    }

    out.push_str(&format!("## Frase falada\n{}\n", utterance.trim()));
    out
}

/// Validate the decision against the catalog. This is the half that makes
/// acting on a model's output defensible.
pub fn resolve(decision: &IntentDecision, catalog: &IntentCatalog) -> SpokenPlan {
    let instruction = decision
        .instruction
        .as_deref()
        .map(str::trim)
        .filter(|i| !i.is_empty())
        .map(str::to_string);

    let known = |id: &str| catalog.sessions.iter().find(|s| s.id == id).cloned();

    // Candidates the model offered that actually exist.
    let options: Vec<CatalogSession> =
        decision.candidates.iter().filter_map(|id| known(id)).collect();

    match decision.kind.as_str() {
        "clarify" => clarify(decision, options),
        "open_project" => match decision
            .project
            .as_deref()
            .and_then(|p| catalog.projects.iter().find(|k| k.eq_ignore_ascii_case(p)))
        {
            Some(project) => SpokenPlan::OpenProject {
                project: project.clone(),
                instruction,
            },
            // A project nobody registered is not a project.
            None => clarify(decision, options),
        },
        "open_session" => match decision.session_id.as_deref().and_then(known) {
            Some(session) => SpokenPlan::OpenSession {
                session_id: session.id,
                title: session.title,
                instruction,
            },
            None => clarify(decision, options),
        },
        "dispatch" => {
            let target = decision
                .session_id
                .as_deref()
                .and_then(known)
                .map(|s| s.id)
                .or_else(|| catalog.focused_session.clone());
            match instruction {
                // Work with nothing to do is not work.
                None => SpokenPlan::Question,
                Some(instruction) => SpokenPlan::Dispatch {
                    session_id: target,
                    instruction,
                },
            }
        }
        // Unknown kinds included: an answer we do not understand is a
        // question, which costs a turn — never a wrong action.
        _ => SpokenPlan::Question,
    }
}

/// Ambiguity, or a target that failed validation. Without candidates and
/// without a question there is nothing to ask, so it degrades to the ask.
fn clarify(decision: &IntentDecision, options: Vec<CatalogSession>) -> SpokenPlan {
    match decision.question.as_deref().map(str::trim) {
        Some(question) if !question.is_empty() => SpokenPlan::Clarify {
            question: question.to_string(),
            options,
        },
        _ if !options.is_empty() => SpokenPlan::Clarify {
            question: "Qual sessão?".to_string(),
            options,
        },
        _ => SpokenPlan::Question,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(id: &str, title: &str) -> CatalogSession {
        CatalogSession {
            id: id.into(),
            title: title.into(),
            project: Some("workspace-alpha".into()),
        }
    }

    fn catalog() -> IntentCatalog {
        IntentCatalog {
            projects: vec!["workspace-alpha".into(), "webhook-api".into()],
            sessions: vec![
                session("s-1", "Pin Vigia chart last version"),
                session("s-2", "Vigia rollout notes"),
            ],
            focused_project: Some("workspace-alpha".into()),
            focused_session: Some("s-9".into()),
            recent_exchange: vec!["você: e o artefato?".into()],
        }
    }

    fn decision(kind: &str) -> IntentDecision {
        IntentDecision {
            kind: kind.into(),
            ..Default::default()
        }
    }

    #[test]
    fn an_invented_session_is_never_opened() {
        // The one failure mode that must not exist: acting on an id the
        // model made up. It degrades to a question, not a wrong window.
        let mut d = decision("open_session");
        d.session_id = Some("s-does-not-exist".into());
        assert_eq!(resolve(&d, &catalog()), SpokenPlan::Question);
    }

    #[test]
    fn an_unregistered_project_is_not_a_project() {
        let mut d = decision("open_project");
        d.project = Some("some-repo-i-imagined".into());
        assert_eq!(resolve(&d, &catalog()), SpokenPlan::Question);
    }

    #[test]
    fn a_known_session_opens_and_carries_the_work() {
        let mut d = decision("open_session");
        d.session_id = Some("s-1".into());
        d.instruction = Some("atualiza o último artefato".into());
        assert_eq!(
            resolve(&d, &catalog()),
            SpokenPlan::OpenSession {
                session_id: "s-1".into(),
                title: "Pin Vigia chart last version".into(),
                instruction: Some("atualiza o último artefato".into()),
            }
        );
    }

    #[test]
    fn two_plausible_targets_ask_instead_of_guessing() {
        let mut d = decision("clarify");
        d.candidates = vec!["s-1".into(), "s-2".into()];
        d.question = Some("Qual das duas sessões do Vigia?".into());
        match resolve(&d, &catalog()) {
            SpokenPlan::Clarify { question, options } => {
                assert_eq!(options.len(), 2);
                assert!(question.contains("Vigia"));
            }
            other => panic!("ambiguity must ask, got {other:?}"),
        }
    }

    #[test]
    fn candidates_that_do_not_exist_are_dropped_from_the_question() {
        let mut d = decision("clarify");
        d.candidates = vec!["s-1".into(), "ghost".into()];
        d.question = Some("Qual sessão?".into());
        match resolve(&d, &catalog()) {
            SpokenPlan::Clarify { options, .. } => {
                assert_eq!(options.len(), 1, "only the real one survives");
                assert_eq!(options[0].id, "s-1");
            }
            other => panic!("expected a question, got {other:?}"),
        }
    }

    #[test]
    fn dispatch_without_a_named_target_lands_on_the_focused_session() {
        // "faz essa alteração dentro da sessão" — the target is the window
        // the person is looking at.
        let mut d = decision("dispatch");
        d.instruction = Some("atualiza o artefato pro time".into());
        assert_eq!(
            resolve(&d, &catalog()),
            SpokenPlan::Dispatch {
                session_id: Some("s-9".into()),
                instruction: "atualiza o artefato pro time".into(),
            }
        );
    }

    #[test]
    fn dispatch_with_nothing_to_do_is_not_work() {
        assert_eq!(resolve(&decision("dispatch"), &catalog()), SpokenPlan::Question);
    }

    #[test]
    fn a_kind_we_do_not_understand_costs_a_turn_not_a_wrong_action() {
        assert_eq!(resolve(&decision("teleport"), &catalog()), SpokenPlan::Question);
        assert_eq!(resolve(&decision("question"), &catalog()), SpokenPlan::Question);
    }

    #[test]
    fn the_prompt_carries_the_catalog_and_the_focus() {
        let p = build_prompt("atualiza o artefato daquela conversa", &catalog());
        assert!(p.contains("s-1 | Pin Vigia chart last version"), "{p}");
        assert!(p.contains("sessão em foco: s-9"), "{p}");
        assert!(p.contains("workspace-alpha"), "{p}");
        assert!(p.contains("e o artefato?"), "recent exchange is context: {p}");
        assert!(p.contains("atualiza o artefato daquela conversa"), "{p}");
    }
}

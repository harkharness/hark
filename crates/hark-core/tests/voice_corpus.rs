//! The voice-fidelity harness (FASE 8.1).
//!
//! Every line of tests/corpus/voice_corpus.jsonl is a sentence that was
//! actually spoken or typed at Hark — incidents included — with the plan it
//! SHOULD produce. The runner replays them through the same decision funnel
//! the HUD runs, over a fixed world, so a routing change shows its blast
//! radius before it ships: the 24/08 incident (10 turns, $0.36, zero work)
//! was exactly a change nobody could measure.
//!
//! The funnel order mirrors src-tauri/src/voice.rs::plan_utterance and
//! src-tauri/src/lib.rs::task_command (the two shell steps are thin glue
//! over the domain functions used here — if you change their ORDER there,
//! change it here too):
//!
//!   1. task_command::parse, then the "abre <registered name>" fallback
//!      (project::find_spoken) — local commands, zero tokens
//!   2. domain::funnel::plan — address → question → active chat → search
//!
//! Corpus schema per line:
//!   say      the sentence, verbatim
//!   active   "alertas" | "assinaturas" — which chat is on screen (default none)
//!   expect   question | work_active | work_task | work_search |
//!            work_project | candidates | no_target | command:<kind>
//!   target   session id / project name the plan must land on (optional)
//!   classify should_classify's expected verdict (optional; gate must never
//!            pay the model for a plain question)
//!   miss     true = KNOWN BUG: the funnel gets this wrong today. The
//!            runner asserts it still fails — fixing it flips the flag.

use hark_core::domain::board::{Task, TaskStatus};
use hark_core::domain::funnel::{self, Plan, SessionLead};
use hark_core::domain::project::Project;
use hark_core::domain::task_command::{self, TaskCommand};
use hark_core::domain::voice_intent::{self, CatalogSession, IntentCatalog};

// ---------------------------------------------------------------- fixture

fn projects() -> Vec<Project> {
    let mk = |name: &str, path: &str| Project { name: name.into(), path: path.into() };
    vec![
        mk("hark", "/Users/dev/Projects/hark"),
        mk("workspace-fabrica", "/Users/dev/Projects/workspace-fabrica"),
        mk("workspace-codigo", "/Users/dev/Projects/workspace-codigo"),
    ]
}

fn board() -> Vec<Task> {
    let mk = |title: &str, session: &str| Task {
        title: title.into(),
        status: TaskStatus::Doing,
        workspace: Some("/Users/dev/Projects/workspace-fabrica".into()),
        session_ids: vec![session.into()],
        updated_at: "2026-08-25T10:00:00Z".into(),
        note: None,
        pinned: false,
        subtasks: Vec::new(),
    };
    vec![
        mk("Alertas do cluster migração e correções", "s-alertas"),
        mk("Migração Assinaturas Core", "s-assinaturas"),
    ]
}

/// The session index. `search` returns everything — recall is the SQL
/// layer's job in the shell; precision belongs to the funnel's ranker,
/// which is what the corpus is exercising.
fn leads() -> Vec<SessionLead> {
    let mk = |id: &str, title: &str, prompt: &str| SessionLead {
        session_id: id.into(),
        title: title.into(),
        cwd: Some("/Users/dev/Projects/workspace-fabrica".into()),
        last_ts: Some(format!("2026-08-2{}T10:00:00Z", id.len() % 5)),
        last_prompt: Some(prompt.into()),
    };
    vec![
        mk("s-alertas", "Alertas do cluster migração e correções", "threshold do certmanager"),
        mk("s-assinaturas", "Migração Assinaturas Core", "migra os serviços de assinaturas"),
        mk("s-webhook", "migração do webhook", "continua a migração do webhook"),
        mk("s-dns", "PR do DNS antigo", "abre o PR do DNS antigo"),
    ]
}

fn catalog() -> IntentCatalog {
    IntentCatalog {
        projects: projects().into_iter().map(|p| p.name).collect(),
        sessions: leads()
            .into_iter()
            .map(|l| CatalogSession {
                id: l.session_id,
                title: l.title,
                project: Some("workspace-fabrica".into()),
            })
            .collect(),
        ..Default::default()
    }
}

fn active_pair(name: &str) -> (&'static str, &'static str) {
    match name {
        "alertas" => ("Alertas do cluster migração e correções", "s-alertas"),
        "assinaturas" => ("Migração Assinaturas Core", "s-assinaturas"),
        other => panic!("corpus names unknown active chat {other:?}"),
    }
}

// ----------------------------------------------------------------- runner

#[derive(Debug)]
struct Entry {
    say: String,
    active: Option<String>,
    expect: String,
    target: Option<String>,
    classify: Option<bool>,
    miss: bool,
    line: usize,
}

fn corpus() -> Vec<Entry> {
    let raw = include_str!("corpus/voice_corpus.jsonl");
    raw.lines()
        .enumerate()
        .filter(|(_, l)| !l.trim().is_empty())
        .map(|(i, l)| {
            let v: serde_json::Value = serde_json::from_str(l)
                .unwrap_or_else(|e| panic!("corpus line {}: bad json: {e}", i + 1));
            Entry {
                say: v["say"].as_str().expect("say").to_string(),
                active: v["active"].as_str().map(String::from),
                expect: v["expect"].as_str().expect("expect").to_string(),
                target: v["target"].as_str().map(String::from),
                classify: v["classify"].as_bool(),
                miss: v["miss"].as_bool().unwrap_or(false),
                line: i + 1,
            }
        })
        .collect()
}

/// What one sentence resolves to, shell order: local command, then funnel.
fn decide(say: &str, active: Option<&str>) -> (String, Option<String>) {
    // Step 1 — local commands (src-tauri/src/lib.rs::task_command).
    if let Some(cmd) = task_command::parse(say) {
        return command_outcome(cmd);
    }
    // Its fallback: "abre <name>" with a REGISTERED project named in the
    // sentence — then unregistered directories on disk (the Intel 25/08
    // dead end: one match registers and opens, several become an offer).
    let lower = say.to_lowercase();
    if ["abre", "abra", "abrir"].iter().any(|v| lower.contains(v)) {
        if let Some(hit) = hark_core::domain::project::find_spoken(&projects(), say) {
            return ("command:open_project".into(), Some(hit.name.clone()));
        }
        let disk = ["workspace-fabrica", "notas-pessoais", "api-gateway"];
        let matches = hark_core::domain::project::dirs_matching_speech(say, &disk);
        match matches.len() {
            0 => {}
            1 => return ("command:open_project".into(), Some(matches[0].clone())),
            _ => return ("command:project_offer".into(), None),
        }
    }
    // Step 2 — the pure funnel.
    let board = board();
    let projects = projects();
    let search = |_q: &str| leads();
    let pair = active.map(active_pair);
    let world = funnel::World {
        active: pair,
        active_project: active.map(|_| ("workspace-fabrica", "/Users/dev/Projects/workspace-fabrica")),
        board: &board,
        projects: &projects,
        search: &search,
    };
    match funnel::plan(say, &world) {
        Plan::Question { .. } => ("question".into(), None),
        Plan::Work { session_id, workspace, new_task, confidence, .. } => {
            if new_task {
                ("work_project".into(), workspace)
            } else if active.is_some() && confidence == "high" && session_id.as_deref() == pair.map(|(_, s)| s) {
                ("work_active".into(), session_id)
            } else if confidence == "high" {
                ("work_task".into(), session_id)
            } else {
                ("work_search".into(), session_id)
            }
        }
        Plan::Candidates { .. } => ("candidates".into(), None),
        Plan::NoTarget { .. } => ("no_target".into(), None),
    }
}

fn command_outcome(cmd: TaskCommand) -> (String, Option<String>) {
    let (kind, target) = match cmd {
        TaskCommand::Open(q) => ("open", Some(q)),
        TaskCommand::Switch { query, .. } => ("switch", Some(query)),
        TaskCommand::Rename { query, .. } => ("rename", Some(query)),
        TaskCommand::Pin(q) => ("pin", Some(q)),
        TaskCommand::Archive(q) => ("archive", Some(q)),
        TaskCommand::OpenFile { project, .. } => ("open_file", project),
        TaskCommand::AddProject { .. } => ("add_project", None),
        TaskCommand::NewChat { project, .. } => ("new_chat", Some(project)),
        TaskCommand::OpenProject { query, .. } => ("open_project", Some(query)),
        TaskCommand::OpenHq { .. } => ("open_hq", None),
        TaskCommand::FindSession { .. } => ("find_session", None),
        TaskCommand::Compact => ("compact", None),
        TaskCommand::SetMode { .. } => ("set_mode", None),
        TaskCommand::OpenSettings => ("open_settings", None),
        TaskCommand::Handoff { target } => ("handoff", Some(target)),
        TaskCommand::Done(q) => ("done", q),
    };
    (format!("command:{kind}"), target)
}

/// Loose target check: command targets are spoken queries ("workspace
/// código"), not canonical names — matching is the funnel's job downstream,
/// so the corpus accepts either the exact id or a resolvable spoken form.
fn target_matches(got: Option<&str>, want: &str) -> bool {
    match got {
        None => false,
        Some(g) if g == want => true,
        Some(g) => hark_core::domain::project::find_spoken(&projects(), g)
            .map(|p| p.name == want)
            .unwrap_or(false),
    }
}

#[test]
fn corpus_replays_clean() {
    let entries = corpus();
    assert!(entries.len() >= 30, "the corpus only means something with volume");
    let catalog = catalog();

    let mut failures = Vec::new();
    let mut fixed_misses = Vec::new();
    let mut by_kind: std::collections::BTreeMap<String, (usize, usize)> = Default::default();

    for e in &entries {
        let (kind, target) = decide(&e.say, e.active.as_deref());
        let kind_ok = kind == e.expect;
        let target_ok = match &e.target {
            None => true,
            Some(want) => target_matches(target.as_deref(), want),
        };
        // The free gate must agree where the corpus states it: a plain
        // question or small talk must never pay the classifier.
        let gate_ok = match e.classify {
            None => true,
            Some(want) => voice_intent::should_classify(&e.say, &catalog) == want,
        };
        let ok = kind_ok && target_ok && gate_ok;

        let slot = by_kind.entry(e.expect.clone()).or_default();
        slot.1 += 1;
        if ok {
            slot.0 += 1;
        }

        if e.miss {
            if ok {
                fixed_misses.push(format!("line {}: {:?} now passes — drop its miss flag", e.line, e.say));
            }
        } else if !ok {
            failures.push(format!(
                "line {}: {:?} (active={:?})\n  expected {} target={:?}\n  got      {} target={:?} gate_ok={}",
                e.line, e.say, e.active, e.expect, e.target, kind, target, gate_ok
            ));
        }
    }

    // The scoreboard prints on every run: `cargo test corpus -- --nocapture`.
    let total = entries.len();
    let passed = total - failures.len() - entries.iter().filter(|e| e.miss).count()
        + fixed_misses.len();
    println!("voice corpus: {passed}/{total} sentences land where they should");
    for (kind, (ok, all)) in &by_kind {
        println!("  {kind:<22} {ok}/{all}");
    }

    assert!(
        failures.is_empty(),
        "the funnel regressed on {} real sentence(s):\n\n{}",
        failures.len(),
        failures.join("\n\n")
    );
    assert!(
        fixed_misses.is_empty(),
        "good news that must be recorded:\n{}",
        fixed_misses.join("\n")
    );
}

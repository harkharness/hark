//! Pure logic of the invisible kanban: tasks born from conversations,
//! merged by title similarity, linked to the sessions that ran them.

use crate::domain::dispatch::significant_terms;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Backlog,
    Doing,
    Waiting,
    Done,
}

/// A task on the invisible board.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Task {
    pub title: String,
    pub status: TaskStatus,
    pub workspace: Option<String>,
    /// Sessions where this task was discussed or executed.
    pub session_ids: Vec<String>,
    pub updated_at: String,
    pub note: Option<String>,
    /// Kept at the top of the sidebar.
    #[serde(default)]
    pub pinned: bool,
    /// The approved plan as a visible checklist (user-toggled).
    #[serde(default)]
    pub subtasks: Vec<Subtask>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Subtask {
    pub text: String,
    #[serde(default)]
    pub done: bool,
}

/// An approved plan (ExitPlanMode's markdown) becomes a checklist: the
/// `##` step headers, or the top-level numbered/bulleted items when the
/// plan has fewer than two headers. Meta sections (context, verification,
/// risks, notes) stay out. Capped and clipped — a checklist, not a doc.
pub fn plan_to_subtasks(plan: &str) -> Vec<Subtask> {
    const META: &[&str] = &[
        "context", "contexto", "verifica", "verification", "risco", "risk", "nota", "note",
    ];
    let clip = |s: &str| {
        let s = s.trim();
        if s.chars().count() > 80 {
            format!("{}…", s.chars().take(80).collect::<String>())
        } else {
            s.to_string()
        }
    };
    let headers: Vec<String> = plan
        .lines()
        .filter_map(|l| l.strip_prefix("## "))
        .filter(|h| {
            let low = h.to_lowercase();
            !META.iter().any(|m| low.starts_with(m))
        })
        .map(clip)
        .collect();
    let steps: Vec<String> = if headers.len() >= 2 {
        headers
    } else {
        plan.lines()
            .filter_map(|l| {
                // Top level only: indented sub-items are detail, not steps.
                if l.starts_with(' ') || l.starts_with('\t') {
                    return None;
                }
                let t = l.trim_start();
                t.strip_prefix("- ")
                    .or_else(|| t.strip_prefix("* "))
                    .or_else(|| {
                        t.split_once(". ").and_then(|(n, rest)| {
                            n.chars().all(|c| c.is_ascii_digit()).then_some(rest)
                        })
                    })
                    .map(clip)
            })
            .collect()
    };
    steps
        .into_iter()
        .filter(|s| !s.is_empty())
        .take(12)
        .map(|text| Subtask { text, done: false })
        .collect()
}

/// One board change proposed by Claude inside a structured reply
/// (Portuguese field names to match the response schema).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoardUpdate {
    pub titulo: String,
    pub status: TaskStatus,
    #[serde(default)]
    pub nota: Option<String>,
    /// Session id this task came from, copied from the snapshot context.
    #[serde(default)]
    pub sessao: Option<String>,
}

/// Two titles are the same task when the significant terms of one are
/// (almost) contained in the other's.
fn same_task(a: &str, b: &str) -> bool {
    let ta = significant_terms(a);
    let tb = significant_terms(b);
    if ta.is_empty() || tb.is_empty() {
        return false;
    }
    let (small, big) = if ta.len() <= tb.len() { (&ta, &tb) } else { (&tb, &ta) };
    let contained = small.iter().filter(|t| big.contains(t)).count();
    contained * 10 >= small.len() * 8 // >= 80% of the smaller side
}

/// Merge proposed updates into the board. Similar titles update in place
/// (status, note, timestamp, linked session); new topics become new tasks.
pub fn apply_updates(
    mut tasks: Vec<Task>,
    updates: &[BoardUpdate],
    now: &str,
    workspace: Option<&str>,
    session_id: Option<&str>,
) -> Vec<Task> {
    for update in updates {
        // The update's own session reference (from the model) wins over the
        // caller-level one (from a dispatch).
        let sid = update.sessao.as_deref().or(session_id);
        let link = |ids: &mut Vec<String>| {
            if let Some(sid) = sid {
                if !ids.iter().any(|i| i == sid) {
                    ids.push(sid.to_string());
                }
            }
        };
        // A task already linked to this session IS this task, whatever it
        // is called now — matching by title first would fork a renamed
        // card into a duplicate on the next update from the same session.
        let position = sid
            .and_then(|s| {
                tasks
                    .iter()
                    .position(|t| t.session_ids.iter().any(|i| i == s))
            })
            .or_else(|| tasks.iter().position(|t| same_task(&t.title, &update.titulo)));
        match position.map(|i| &mut tasks[i]) {
            Some(task) => {
                task.status = update.status;
                task.updated_at = now.to_string();
                if update.nota.is_some() {
                    task.note = update.nota.clone();
                }
                if task.workspace.is_none() {
                    task.workspace = workspace.map(String::from);
                }
                link(&mut task.session_ids);
            }
            None => {
                let mut session_ids = Vec::new();
                link(&mut session_ids);
                tasks.push(Task {
                    title: update.titulo.clone(),
                    status: update.status,
                    workspace: workspace.map(String::from),
                    session_ids,
                    updated_at: now.to_string(),
                    note: update.nota.clone(),
                    pinned: false,
                    subtasks: Vec::new(),
                });
            }
        }
    }
    tasks
}

/// A session recovered from history (the palette, the HUD) becomes a card
/// of ITS folder. Only the session id matches: `apply_updates` also
/// matches by title for the model's updates, and a look-alike title put a
/// terminal session on another project's card, where no window of its own
/// project listed it (25/09). Returns the card as saved, so the caller
/// focuses the title the board really has.
pub fn adopt_session(
    mut tasks: Vec<Task>,
    label: &str,
    now: &str,
    cwd: Option<&str>,
    session_id: &str,
) -> (Vec<Task>, Task) {
    let covers = |ws: &str, dir: &str| {
        let ws = ws.trim_end_matches('/');
        dir == ws || dir.starts_with(&format!("{ws}/"))
    };
    if let Some(task) = tasks.iter_mut().find(|t| t.session_ids.iter().any(|s| s == session_id)) {
        task.status = TaskStatus::Doing;
        task.updated_at = now.to_string();
        if let Some(dir) = cwd {
            if !task.workspace.as_deref().is_some_and(|ws| covers(ws, dir)) {
                task.workspace = Some(dir.to_string());
            }
        }
        let card = task.clone();
        return (tasks, card);
    }
    // The title is the board's key: a new card never takes one in use.
    let mut title = label.to_string();
    let mut n = 2;
    while tasks.iter().any(|t| t.title == title) {
        title = format!("{label} ({n})");
        n += 1;
    }
    let card = Task {
        title,
        status: TaskStatus::Doing,
        workspace: cwd.map(String::from),
        session_ids: vec![session_id.to_string()],
        updated_at: now.to_string(),
        note: Some("sessão recuperada".into()),
        pinned: false,
        subtasks: Vec::new(),
    };
    tasks.push(card.clone());
    (tasks, card)
}

/// Manual move (drag on the board): exact title match, new status, no LLM.
pub fn set_status(mut tasks: Vec<Task>, title: &str, status: TaskStatus, now: &str) -> Vec<Task> {
    if let Some(task) = tasks.iter_mut().find(|t| t.title == title) {
        task.status = status;
        task.updated_at = now.to_string();
    }
    tasks
}

/// Manual archive: removes the task from the board entirely.
pub fn archive(tasks: Vec<Task>, title: &str) -> Vec<Task> {
    tasks.into_iter().filter(|t| t.title != title).collect()
}

/// Rename in place, keeping status, sessions and note.
pub fn rename(mut tasks: Vec<Task>, title: &str, new_title: &str, now: &str) -> Vec<Task> {
    if let Some(task) = tasks.iter_mut().find(|t| t.title == title) {
        task.title = new_title.to_string();
        task.updated_at = now.to_string();
    }
    tasks
}

/// Toggle the pin that keeps a task at the top of the sidebar.
pub fn toggle_pin(mut tasks: Vec<Task>, title: &str) -> Vec<Task> {
    if let Some(task) = tasks.iter_mut().find(|t| t.title == title) {
        task.pinned = !task.pinned;
    }
    tasks
}

/// Rank tasks against a spoken query: whole words on folded tokens,
/// rare-term weighting, and Ambiguous when it is too close to call —
/// substring hits + recency were how "Assinaturas Core" opened a hark card.
pub fn find_ranked(tasks: &[Task], query: &str) -> crate::domain::matching::Match<Task> {
    crate::domain::matching::rank(
        query,
        tasks.to_vec(),
        |t| format!("{} {}", t.title, t.note.clone().unwrap_or_default()),
        |t| t.updated_at.clone(),
    )
}

/// Best task matching a spoken query — Hit-only view of `find_ranked`
/// for callers without a disambiguation surface.
pub fn find(tasks: &[Task], query: &str) -> Option<Task> {
    match find_ranked(tasks, query) {
        crate::domain::matching::Match::Hit(task) => Some(task),
        _ => None,
    }
}

/// Render the board for the prompt/screen: open tasks grouped by status,
/// plus only the most recently finished ones.
pub fn render(tasks: &[Task]) -> String {
    if tasks.is_empty() {
        return String::new();
    }
    let group = |status: TaskStatus| -> Vec<&Task> {
        tasks.iter().filter(|t| t.status == status).collect()
    };
    let mut done = group(TaskStatus::Done);
    done.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    done.truncate(3);

    let section = |label: &str, items: Vec<&Task>| -> String {
        if items.is_empty() {
            return String::new();
        }
        let lines: Vec<String> = items
            .iter()
            .map(|t| {
                format!(
                    "- {} (atualizado {}{}{})",
                    t.title,
                    t.updated_at,
                    t.session_ids
                        .last()
                        .map(|s| format!(", sessao {s}"))
                        .unwrap_or_default(),
                    t.note.as_deref().map(|n| format!(": {n}")).unwrap_or_default(),
                )
            })
            .collect();
        format!("[{label}]\n{}", lines.join("\n"))
    };

    [
        section("doing", group(TaskStatus::Doing)),
        section("waiting", group(TaskStatus::Waiting)),
        section("backlog", group(TaskStatus::Backlog)),
        section("done recente", done),
    ]
    .iter()
    .filter(|s| !s.is_empty())
    .cloned()
    .collect::<Vec<_>>()
    .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn update(title: &str, status: TaskStatus) -> BoardUpdate {
        BoardUpdate {
            titulo: title.into(),
            status,
            nota: Some("nota".into()),
            sessao: None,
        }
    }

    fn card(title: &str, updated_at: &str) -> Task {
        Task {
            title: title.into(),
            status: TaskStatus::Doing,
            workspace: None,
            session_ids: vec![],
            updated_at: updated_at.into(),
            note: None,
            pinned: false,
            subtasks: Vec::new(),
        }
    }

    fn linked(title: &str, workspace: &str, session: &str) -> Task {
        Task {
            workspace: Some(workspace.into()),
            session_ids: vec![session.into()],
            ..card(title, "2026-09-01T10:00:00Z")
        }
    }

    const NOW: &str = "2026-09-25T12:00:00Z";

    #[test]
    fn a_recovered_session_gets_its_own_card_in_its_own_folder() {
        // 25/09: a terminal session opened from the history palette was
        // matched BY TITLE to another project's card, so the chat showed
        // in neither list of its own window. Only the session id may
        // match a recovered session.
        let board = vec![linked("deploy phase 8 production", "/home/dev/other", "s-old")];
        let (tasks, card) = adopt_session(board, "deploy-phase-9-production", NOW, Some("/home/dev/ws"), "s-9");
        assert_eq!(card.title, "deploy-phase-9-production");
        assert_eq!(card.workspace.as_deref(), Some("/home/dev/ws"));
        assert_eq!(card.session_ids, vec!["s-9".to_string()]);
        assert_eq!(card.status, TaskStatus::Doing);
        assert_eq!(tasks.len(), 2, "the look-alike card is someone else's");
        assert_eq!(tasks[0], linked("deploy phase 8 production", "/home/dev/other", "s-old"));
    }

    #[test]
    fn a_session_already_on_a_card_reopens_that_card() {
        let board = vec![linked("Renomeado pelo usuário", "/home/dev/ws", "s-9")];
        let (tasks, card) = adopt_session(board, "deploy-phase-9-production", NOW, Some("/home/dev/ws"), "s-9");
        assert_eq!(tasks.len(), 1);
        assert_eq!(card.title, "Renomeado pelo usuário", "the card keeps the name it was given");
        assert_eq!(card.updated_at, NOW);
    }

    #[test]
    fn a_card_outside_the_sessions_folder_follows_the_session() {
        let board = vec![linked("chat", "/home/dev/other", "s-9")];
        let (_, moved) = adopt_session(board, "chat", NOW, Some("/home/dev/ws"), "s-9");
        assert_eq!(moved.workspace.as_deref(), Some("/home/dev/ws"));
        // A card on the project root already covers a session in a subfolder.
        let board = vec![linked("chat", "/home/dev/ws", "s-9")];
        let (_, kept) = adopt_session(board, "chat", NOW, Some("/home/dev/ws/apps/game"), "s-9");
        assert_eq!(kept.workspace.as_deref(), Some("/home/dev/ws"));
    }

    #[test]
    fn a_new_card_never_takes_a_title_already_on_the_board() {
        // The title is the board's key: a clash would fail the save.
        let board = vec![linked("deploy", "/home/dev/ws", "s-1")];
        let (tasks, card) = adopt_session(board, "deploy", NOW, Some("/home/dev/ws"), "s-2");
        assert_eq!(card.title, "deploy (2)");
        assert_eq!(tasks.len(), 2);
    }

    #[test]
    fn plan_headers_become_subtasks_meta_sections_stay_out() {
        let plan = "# Migração\n\n## Context\nblah\n\n## Mover o webhook\ndetalhe\n\n## Trocar o DNS\n\n## Verificação\nteste\n\n## Riscos\n";
        let subs = plan_to_subtasks(plan);
        assert_eq!(
            subs.iter().map(|s| s.text.as_str()).collect::<Vec<_>>(),
            vec!["Mover o webhook", "Trocar o DNS"],
        );
        assert!(subs.iter().all(|s| !s.done));
    }

    #[test]
    fn bullet_plans_fall_back_to_top_level_items() {
        let plan = "# Plano\n\n1. Ler o template da thread\n2. Montar o draft\n   - detalhe interno fica fora\n3. Postar no canal\n";
        let subs = plan_to_subtasks(plan);
        assert_eq!(
            subs.iter().map(|s| s.text.as_str()).collect::<Vec<_>>(),
            vec!["Ler o template da thread", "Montar o draft", "Postar no canal"],
        );
    }

    #[test]
    fn subtasks_are_capped_and_clipped() {
        let long = (0..30).map(|i| format!("## Passo {i} {}", "x".repeat(200))).collect::<Vec<_>>().join("\n");
        let subs = plan_to_subtasks(&long);
        assert!(subs.len() <= 12, "cap");
        assert!(subs.iter().all(|s| s.text.chars().count() <= 81), "clip");
        assert!(plan_to_subtasks("sem estrutura nenhuma").is_empty());
    }

    #[test]
    fn find_ranked_prefers_task_with_more_matched_terms() {
        let tasks = vec![
            card("Migração de alertas", "2026-08-19T09:00:00Z"),
            card("Migração Assinaturas Core serviços", "2026-08-01T09:00:00Z"),
        ];
        let hit = match find_ranked(&tasks, "migração dos serviços assinaturas core") {
            crate::domain::matching::Match::Hit(t) => t.title,
            other => panic!("expected hit, got {other:?}"),
        };
        assert_eq!(hit, "Migração Assinaturas Core serviços");
    }

    #[test]
    fn junk_single_term_hit_on_fresh_task_loses() {
        // The 19/08 incident: the freshest card grabbing one junk term
        // ("core" inside "hark-core") must NOT win an Assinaturas query.
        let tasks = vec![card("hark-core targeting refactor", "2026-08-19T12:00:00Z")];
        assert_eq!(
            find_ranked(&tasks, "migração dos serviços Assinaturas Core"),
            crate::domain::matching::Match::None,
        );
        assert_eq!(find(&tasks, "migração dos serviços Assinaturas Core"), None);
    }

    #[test]
    fn near_tie_returns_ambiguous() {
        let tasks = vec![
            card("Migração Assinaturas uat", "2026-08-10T09:00:00Z"),
            card("Migração Assinaturas prod", "2026-08-18T09:00:00Z"),
        ];
        let crate::domain::matching::Match::Ambiguous(list) =
            find_ranked(&tasks, "migração assinaturas")
        else {
            panic!("expected ambiguous");
        };
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].title, "Migração Assinaturas prod"); // newest first
    }

    #[test]
    fn model_provided_session_links_the_task() {
        let tasks = apply_updates(
            Vec::new(),
            &[BoardUpdate {
                titulo: "Migração webhook".into(),
                status: TaskStatus::Waiting,
                nota: None,
                sessao: Some("sess-from-model".into()),
            }],
            "2026-08-15T10:00:00Z",
            None,
            None, // no caller session: the model's reference must be used
        );
        assert_eq!(tasks[0].session_ids, vec!["sess-from-model"]);
    }

    #[test]
    fn a_rename_survives_later_updates_from_the_same_session() {
        // The user renamed the card; the next turn of the same session
        // upserts under the session-derived title. Matching by linked
        // session must win over title similarity — otherwise every rename
        // gets shadowed by a duplicate card.
        let existing = apply_updates(
            Vec::new(),
            &[update("Alertas do cluster migração e correções", TaskStatus::Doing)],
            "2026-08-17T10:00:00Z",
            Some("/home/dev/proj"),
            Some("sess-alerts"),
        );
        let renamed = rename(
            existing,
            "Alertas do cluster migração e correções",
            "Decom Carteira",
            "2026-08-17T11:00:00Z",
        );
        let tasks = apply_updates(
            renamed,
            &[update("Alertas do cluster migração e correções", TaskStatus::Doing)],
            "2026-08-17T12:00:00Z",
            Some("/home/dev/proj"),
            Some("sess-alerts"),
        );
        assert_eq!(tasks.len(), 1, "must update the renamed card, not fork it");
        assert_eq!(tasks[0].title, "Decom Carteira");
        assert_eq!(tasks[0].updated_at, "2026-08-17T12:00:00Z");
    }

    #[test]
    fn creates_tasks_from_updates() {
        let tasks = apply_updates(
            Vec::new(),
            &[update("Migração do webhook Carteira", TaskStatus::Waiting)],
            "2026-08-15T10:00:00Z",
            Some("/home/dev/proj"),
            Some("sess-1"),
        );
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].title, "Migração do webhook Carteira");
        assert_eq!(tasks[0].status, TaskStatus::Waiting);
        assert_eq!(tasks[0].session_ids, vec!["sess-1"]);
        assert_eq!(tasks[0].workspace.as_deref(), Some("/home/dev/proj"));
    }

    #[test]
    fn merges_similar_titles_instead_of_duplicating() {
        let existing = apply_updates(
            Vec::new(),
            &[update("Migração do webhook Carteira", TaskStatus::Doing)],
            "2026-08-14T10:00:00Z",
            None,
            Some("sess-1"),
        );
        let tasks = apply_updates(
            existing,
            &[update("migração webhook", TaskStatus::Waiting)],
            "2026-08-15T10:00:00Z",
            None,
            Some("sess-2"),
        );
        assert_eq!(tasks.len(), 1, "must merge, not duplicate");
        assert_eq!(tasks[0].status, TaskStatus::Waiting);
        assert_eq!(tasks[0].updated_at, "2026-08-15T10:00:00Z");
        // Keeps both linked sessions, no dup.
        assert_eq!(tasks[0].session_ids, vec!["sess-1", "sess-2"]);
    }

    #[test]
    fn different_topics_stay_separate() {
        let existing = apply_updates(
            Vec::new(),
            &[update("Migração do webhook", TaskStatus::Doing)],
            "2026-08-14T10:00:00Z",
            None,
            None,
        );
        let tasks = apply_updates(
            existing,
            &[update("Migração de Pagamentos", TaskStatus::Doing)],
            "2026-08-15T10:00:00Z",
            None,
            None,
        );
        assert_eq!(tasks.len(), 2);
    }

    #[test]
    fn renames_pins_and_finds_by_spoken_query() {
        let tasks = apply_updates(
            Vec::new(),
            &[
                update("Migração do webhook Carteira", TaskStatus::Doing),
                update("Padrão de alertas", TaskStatus::Backlog),
            ],
            "2026-08-15T10:00:00Z",
            None,
            None,
        );

        // Spoken query hits the right task without the exact title.
        let found = find(&tasks, "webhook").expect("match");
        assert_eq!(found.title, "Migração do webhook Carteira");
        assert!(find(&tasks, "kafka").is_none());

        let tasks = rename(tasks, &found.title, "Decom Carteira", "2026-08-15T11:00:00Z");
        assert!(tasks.iter().any(|t| t.title == "Decom Carteira"));

        let tasks = toggle_pin(tasks, "Decom Carteira");
        assert!(tasks.iter().find(|t| t.title == "Decom Carteira").unwrap().pinned);
    }

    #[test]
    fn manual_move_and_archive_by_exact_title() {
        let tasks = apply_updates(
            Vec::new(),
            &[
                update("Migração webhook", TaskStatus::Doing),
                update("Alertas playbooks", TaskStatus::Backlog),
            ],
            "2026-08-15T10:00:00Z",
            None,
            None,
        );

        let moved = set_status(tasks, "Migração webhook", TaskStatus::Done, "2026-08-15T11:00:00Z");
        let webhook = moved.iter().find(|t| t.title == "Migração webhook").unwrap();
        assert_eq!(webhook.status, TaskStatus::Done);
        assert_eq!(webhook.updated_at, "2026-08-15T11:00:00Z");

        let archived = archive(moved, "Alertas playbooks");
        assert_eq!(archived.len(), 1);
        assert_eq!(archived[0].title, "Migração webhook");
    }

    #[test]
    fn renders_open_tasks_grouped_and_hides_old_done() {
        let mut tasks = Vec::new();
        for (title, status) in [
            ("A fazer X", TaskStatus::Backlog),
            ("Rodando Y", TaskStatus::Doing),
            ("Esperando Z", TaskStatus::Waiting),
            ("Feito W", TaskStatus::Done),
        ] {
            tasks = apply_updates(tasks, &[update(title, status)], "2026-08-15T10:00:00Z", None, None);
        }
        let text = render(&tasks);
        assert!(text.contains("doing"));
        assert!(text.contains("Rodando Y"));
        assert!(text.contains("Esperando Z"));
        assert!(text.contains("A fazer X"));
        assert!(text.contains("Feito W")); // recent done still visible
    }
}

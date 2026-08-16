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
}

/// One board change proposed by Claude inside a structured reply
/// (Portuguese field names to match the response schema).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
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
        match tasks.iter_mut().find(|t| same_task(&t.title, &update.titulo)) {
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
                });
            }
        }
    }
    tasks
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

/// Best task matching a spoken query (term overlap on title and note).
pub fn find(tasks: &[Task], query: &str) -> Option<Task> {
    let terms = significant_terms(query);
    if terms.is_empty() {
        return None;
    }
    tasks
        .iter()
        .map(|task| {
            let haystack = format!("{} {}", task.title, task.note.clone().unwrap_or_default())
                .to_lowercase();
            let hits = terms.iter().filter(|t| haystack.contains(t.as_str())).count();
            (hits, task)
        })
        .filter(|(hits, _)| *hits > 0)
        .max_by_key(|(hits, task)| (*hits, task.updated_at.clone()))
        .map(|(_, task)| task.clone())
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

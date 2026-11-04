//! Notifications: the bell is the only interruption. Every event that
//! would make Hark talk becomes a row behind the bell instead, grouped per
//! chat like Slack; opening the chat or seeing the row marks it read; a
//! read row leaves the panel after a while, an unread one never does.
//! Pure: the shell produces and stores, the window shows.

use crate::domain::lang::Lang;
use chrono::DateTime;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    TurnEnded,
    PermissionAsked,
    ArtifactReady,
    Reminder,
    Scheduled,
}

/// What the row points at: "ir" goes there, opening it marks the row read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Target {
    Chat { title: String, session_id: Option<String> },
    Page { space: String, id: String },
    Request { id: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Notification {
    pub id: String,
    pub kind: Kind,
    pub target: Target,
    /// The project path or the artifact space: the colour it wears.
    pub project: Option<String>,
    /// One line, in the person's language.
    pub text: String,
    pub at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub read_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dismissed_at: Option<String>,
}

impl Notification {
    /// The name of what it points at, for the row and for grouping.
    pub fn target_title(&self) -> &str {
        match &self.target {
            Target::Chat { title, .. } => title,
            Target::Page { id, .. } => id,
            Target::Request { id } => id,
        }
    }
    fn target_key(&self) -> String {
        match &self.target {
            Target::Chat { title, .. } => format!("chat:{title}"),
            Target::Page { space, id } => format!("page:{space}/{id}"),
            Target::Request { id } => format!("req:{id}"),
        }
    }
}

/// What the shell saw happen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    TurnEnded { task: String, session: Option<String>, project: Option<String>, text: String, is_error: bool },
    PermissionAsked { task: String, tool: String, project: Option<String> },
    ArtifactWritten { space: String, id: String, title: String, task: Option<String> },
    RequestDue { id: String, text: String },
}

/// One row from one event, worded for the person.
pub fn from_event(ev: &Event, at: &str, lang: Lang) -> Notification {
    let (kind, target, project, text) = match ev {
        Event::TurnEnded { task, session, project, text, is_error } => {
            let head = if *is_error { lang.pick("falhou", "failed") } else { lang.pick("terminou o turno", "finished its turn") };
            let gist = first_line(text, 72);
            let line = if gist.is_empty() { head.to_string() } else { format!("{head} · {gist}") };
            (Kind::TurnEnded, Target::Chat { title: task.clone(), session_id: session.clone() }, project.clone(), line)
        }
        Event::PermissionAsked { task, tool, project } => (
            Kind::PermissionAsked,
            Target::Chat { title: task.clone(), session_id: None },
            project.clone(),
            format!("{} {tool}", lang.pick("pediu permissão:", "asked for permission:")),
        ),
        Event::ArtifactWritten { space, id, title, task: _ } => (
            Kind::ArtifactReady,
            Target::Page { space: space.clone(), id: id.clone() },
            Some(space.clone()),
            format!("{title} {}", lang.pick("pronto para revisar", "ready for review")),
        ),
        Event::RequestDue { id, text } => (
            Kind::Reminder,
            Target::Request { id: id.clone() },
            None,
            format!("{} {text}", lang.pick("lembrete:", "reminder:")),
        ),
    };
    let mut n = Notification { id: String::new(), kind, target, project, text, at: at.to_string(), read_at: None, dismissed_at: None };
    n.id = format!("{}:{}:{at}", kind_key(kind), n.target_key());
    n
}

fn kind_key(kind: Kind) -> &'static str {
    match kind {
        Kind::TurnEnded => "turn",
        Kind::PermissionAsked => "perm",
        Kind::ArtifactReady => "page",
        Kind::Reminder => "reminder",
        Kind::Scheduled => "scheduled",
    }
}

fn first_line(text: &str, max: usize) -> String {
    let line = text.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("");
    let line: String = line.chars().filter(|c| *c != '*' && *c != '`').collect();
    let line = line.trim();
    if line.chars().count() > max {
        let cut: String = line.chars().take(max - 1).collect();
        format!("{}…", cut.trim_end())
    } else {
        line.to_string()
    }
}

/// One group per target, as Slack folds a channel: newest group first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Group {
    pub key: String,
    pub title: String,
    pub project: Option<String>,
    pub count: usize,
    pub unread: usize,
    pub latest: Notification,
}

pub fn group(list: &[Notification]) -> Vec<Group> {
    let mut groups: Vec<Group> = Vec::new();
    for n in list.iter().filter(|n| n.dismissed_at.is_none()) {
        let key = n.target_key();
        match groups.iter_mut().find(|g| g.key == key) {
            Some(g) => {
                g.count += 1;
                if n.read_at.is_none() {
                    g.unread += 1;
                }
                if n.at > g.latest.at {
                    g.latest = n.clone();
                }
            }
            None => groups.push(Group {
                key,
                title: n.target_title().to_string(),
                project: n.project.clone(),
                count: 1,
                unread: usize::from(n.read_at.is_none()),
                latest: n.clone(),
            }),
        }
    }
    groups.sort_by(|a, b| b.latest.at.cmp(&a.latest.at));
    groups
}

/// Opening a chat reads everything about it, on the spot.
pub fn mark_read_on_open(list: &mut [Notification], chat_title: &str, at: &str) {
    for n in list.iter_mut() {
        if n.read_at.is_none() && matches!(&n.target, Target::Chat { title, .. } if title == chat_title) {
            n.read_at = Some(at.to_string());
        }
    }
}

/// Seen in the panel: these rows are read.
pub fn mark_read(list: &mut [Notification], ids: &[String], at: &str) {
    for n in list.iter_mut() {
        if n.read_at.is_none() && ids.contains(&n.id) {
            n.read_at = Some(at.to_string());
        }
    }
}

/// A read row leaves the panel after this long; unread never leave.
pub const DECAY_MINUTES: i64 = 10;

pub fn visible<'a>(list: &'a [Notification], now: &str) -> Vec<&'a Notification> {
    let now = DateTime::parse_from_rfc3339(now).ok();
    let mut out: Vec<&Notification> = list
        .iter()
        .filter(|n| n.dismissed_at.is_none())
        .filter(|n| match (&n.read_at, now) {
            (None, _) => true,
            (Some(read), Some(now)) => DateTime::parse_from_rfc3339(read)
                .map(|r| (now - r).num_minutes() < DECAY_MINUTES)
                .unwrap_or(false),
            (Some(_), None) => false,
        })
        .collect();
    out.sort_by(|a, b| b.at.cmp(&a.at));
    out
}

/// What the bell shows: the unread, minus the chat on screen (you are
/// looking at it; its news is not news).
pub fn unread_count(list: &[Notification], looking_at: Option<&str>) -> usize {
    list.iter()
        .filter(|n| n.read_at.is_none() && n.dismissed_at.is_none())
        .filter(|n| !matches!((&n.target, looking_at), (Target::Chat { title, .. }, Some(seen)) if title == seen))
        .count()
}

/// When Hark may say a notification out loud.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Moment {
    /// The chat in focus finished its turn: the person asked to be told.
    Return,
    /// A scheduled line (the 13:40 summary, the end of the day).
    Scheduled,
    Other,
}

/// Never with the window in front (text does that job). Behind it, a
/// return moment or a scheduled line is always said; anything else only
/// when no focus is on — a focus is a turn the person asked not to
/// interrupt, and the bell holds the rest.
pub fn may_speak(window_front: bool, focused: bool, moment: Moment) -> bool {
    !window_front && (matches!(moment, Moment::Return | Moment::Scheduled) || !focused)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::lang::Lang;

    fn turn(task: &str, at: &str) -> Notification {
        from_event(&Event::TurnEnded { task: task.into(), session: Some("s1".into()), project: Some("/p/hark".into()), text: "PR #142 aberto".into(), is_error: false }, at, Lang::Pt)
    }

    #[test]
    fn an_event_becomes_a_notification_that_names_its_target() {
        let n = turn("Billing v2", "2026-11-04T11:03:00-03:00");
        assert_eq!(n.kind, Kind::TurnEnded);
        assert_eq!(n.target, Target::Chat { title: "Billing v2".into(), session_id: Some("s1".into()) });
        assert_eq!(n.text, "terminou o turno · PR #142 aberto");
        assert_eq!(n.project.as_deref(), Some("/p/hark"));
        assert!(n.read_at.is_none());
        let p = from_event(&Event::PermissionAsked { task: "Billing v2".into(), tool: "Bash".into(), project: None }, "2026-11-04T09:14:00-03:00", Lang::En);
        assert_eq!(p.text, "asked for permission: Bash");
        let a = from_event(&Event::ArtifactWritten { space: "landing".into(), id: "deck".into(), title: "Home v3".into(), task: Some("Redesign".into()) }, "2026-11-04T10:00:00-03:00", Lang::Pt);
        assert_eq!(a.target, Target::Page { space: "landing".into(), id: "deck".into() });
        assert_eq!(a.text, "Home v3 pronto para revisar");
        assert_ne!(n.id, p.id);
    }

    #[test]
    fn rows_group_per_target_newest_first_and_count_the_unread() {
        let mut b2 = turn("Billing v2", "2026-11-04T11:03:00-03:00");
        b2.read_at = Some("2026-11-04T11:10:00-03:00".into());
        let list = vec![
            turn("Billing v2", "2026-11-04T09:14:00-03:00"),
            turn("Redesign", "2026-11-04T10:00:00-03:00"),
            b2,
        ];
        let g = group(&list);
        assert_eq!(g.len(), 2);
        assert_eq!(g[0].title, "Billing v2");
        assert_eq!((g[0].count, g[0].unread), (2, 1));
        assert_eq!(g[0].latest.at, "2026-11-04T11:03:00-03:00");
        assert_eq!(g[1].title, "Redesign");
    }

    #[test]
    fn opening_the_chat_reads_its_rows_and_read_rows_fade_after_ten_minutes() {
        let mut list = vec![turn("Billing v2", "2026-11-04T11:03:00-03:00"), turn("Redesign", "2026-11-04T10:00:00-03:00")];
        mark_read_on_open(&mut list, "Billing v2", "2026-11-04T11:05:00-03:00");
        assert!(list[0].read_at.is_some() && list[1].read_at.is_none());
        let soon = visible(&list, "2026-11-04T11:10:00-03:00");
        assert_eq!(soon.len(), 2, "read five minutes ago still shows");
        let later = visible(&list, "2026-11-04T11:20:00-03:00");
        assert_eq!(later.len(), 1, "read fifteen minutes ago is gone; unread never");
        assert_eq!(later[0].target_title(), "Redesign");
    }

    #[test]
    fn the_bell_counts_the_unread_except_the_chat_you_are_looking_at() {
        let list = vec![turn("Billing v2", "2026-11-04T11:03:00-03:00"), turn("Redesign", "2026-11-04T10:00:00-03:00")];
        assert_eq!(unread_count(&list, None), 2);
        assert_eq!(unread_count(&list, Some("Billing v2")), 1);
    }

    #[test]
    fn hark_speaks_only_behind_the_window_and_never_into_a_focus() {
        assert!(may_speak(false, true, Moment::Return));
        assert!(may_speak(false, true, Moment::Scheduled));
        assert!(!may_speak(false, true, Moment::Other), "a focus is not interrupted: the bell holds it");
        assert!(may_speak(false, false, Moment::Other), "no focus, window behind: the old announcement stands");
        assert!(!may_speak(true, false, Moment::Return), "the window in front shows text, never speaks");
    }
}

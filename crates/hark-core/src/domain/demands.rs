//! Demandas: what asks for the person today, from four sources — the board,
//! the agenda, the artifacts and the requests they made ("me lembra de…").
//! The team's board stays the team's; a demand is the person's and may
//! point at a task without depending on it. Pure: the shell gathers, this
//! decides and orders.

use crate::domain::artifacts::Node;
use crate::domain::board::{Task, TaskStatus};
use crate::domain::lang::Lang;
use chrono::{DateTime, Datelike, Duration, FixedOffset, NaiveTime, SecondsFormat, TimeZone, Weekday};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    Board,
    Agenda,
    Artifact,
    Request,
}

/// Most pressing first: what stopped on you, what is due today, what
/// stalled, what waits for a look, what you asked to be reminded of.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Urgency {
    NeedsYou,
    Scheduled,
    Stalled,
    Review,
    Reminder,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Demand {
    pub id: String,
    pub source: Source,
    /// The project path (board) or the artifact space: the colour it wears.
    pub project: Option<String>,
    pub title: String,
    /// One line, in the person's language: why it is on the list.
    pub why: String,
    pub urgency: Urgency,
    /// Where "ir" goes: a `hark://` link for a page, else the task title.
    pub link: Option<String>,
    /// When it last moved (ISO), for the order inside one urgency.
    pub at: String,
}

/// Where a request was said.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Said {
    Voice,
    Text,
}

/// "Me lembra de…": the person's own item, kept locally, no session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Request {
    pub id: String,
    pub text: String,
    pub said_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub due: Option<String>,
    pub origin: Said,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub done_at: Option<String>,
}

/// A task counts as stalled after this many days without a turn.
const STALLED_DAYS: i64 = 3;
/// And as forgotten after this many: not a demand of the day anymore.
const STALLED_MAX_DAYS: i64 = 14;
/// A page younger than this, written by an agent, is waiting for a look.
const REVIEW_HOURS: i64 = 24;

/// The list of the day: board (with the titles whose permission waits),
/// pages by space, requests. Ordered by urgency, then newest first.
pub fn collect(
    board: &[Task],
    permissions_waiting: &[&str],
    pages: &[(String, Node)],
    requests: &[Request],
    now: DateTime<FixedOffset>,
    lang: Lang,
) -> Vec<Demand> {
    let mut out = Vec::new();
    for task in board {
        let (urgency, why) = if permissions_waiting.contains(&task.title.as_str()) {
            (Urgency::NeedsYou, lang.pick("parou numa permissão", "stopped at a permission").to_string())
        } else {
            match task.status {
                TaskStatus::Waiting => (Urgency::NeedsYou, lang.pick("esperando você", "waiting for you").to_string()),
                TaskStatus::Doing => {
                    let days = age_days(&task.updated_at, now);
                    if !(STALLED_DAYS..=STALLED_MAX_DAYS).contains(&days) {
                        continue;
                    }
                    (Urgency::Stalled, format!("{} {days} {}", lang.pick("parado há", "stalled for"), lang.pick("dias", "days")))
                }
                TaskStatus::Backlog | TaskStatus::Done => continue,
            }
        };
        out.push(Demand {
            id: format!("task:{}", task.title),
            source: Source::Board,
            project: task.workspace.clone(),
            title: task.title.clone(),
            why,
            urgency,
            link: None,
            at: task.updated_at.clone(),
        });
    }
    for (space, page) in pages {
        if page.origin.is_none() || age_hours(&page.updated_at, now) >= REVIEW_HOURS {
            continue;
        }
        out.push(Demand {
            id: format!("page:{space}/{}", page.id),
            source: Source::Artifact,
            project: Some(space.clone()),
            title: page.title.clone(),
            why: lang.pick("pronto para você revisar", "ready for your review").to_string(),
            urgency: Urgency::Review,
            link: Some(format!("hark://artifacts/{space}/{}", page.id)),
            at: page.updated_at.clone(),
        });
    }
    for req in requests {
        if req.done_at.is_some() {
            continue;
        }
        let (urgency, why) = match req.due.as_deref().and_then(parse_ts) {
            Some(due) if due.date_naive() <= now.date_naive() => (
                Urgency::Scheduled,
                format!("{} {}", lang.pick("hoje às", "today at"), due.format("%H:%M")),
            ),
            Some(_) => continue,
            None => (Urgency::Reminder, format!("{} {}", lang.pick("você pediu", "you asked"), said_when(&req.said_at, now, lang))),
        };
        out.push(Demand {
            id: format!("req:{}", req.id),
            source: Source::Request,
            project: None,
            title: req.text.clone(),
            why,
            urgency,
            link: None,
            at: req.due.clone().unwrap_or_else(|| req.said_at.clone()),
        });
    }
    out.sort_by(|a, b| a.urgency.cmp(&b.urgency).then_with(|| b.at.cmp(&a.at)));
    out
}

fn parse_ts(s: &str) -> Option<DateTime<FixedOffset>> {
    DateTime::parse_from_rfc3339(s).ok()
}

fn age_days(ts: &str, now: DateTime<FixedOffset>) -> i64 {
    parse_ts(ts).map(|t| (now - t).num_days()).unwrap_or(0)
}

fn age_hours(ts: &str, now: DateTime<FixedOffset>) -> i64 {
    parse_ts(ts).map(|t| (now - t).num_hours()).unwrap_or(i64::MAX)
}

/// "ontem, 18:10" / "hoje, 08:00" / "03/11, 18:10".
fn said_when(ts: &str, now: DateTime<FixedOffset>, lang: Lang) -> String {
    let Some(t) = parse_ts(ts) else { return String::new() };
    let t = t.with_timezone(now.offset());
    let days = (now.date_naive() - t.date_naive()).num_days();
    let day = match days {
        0 => lang.pick("hoje", "today").to_string(),
        1 => lang.pick("ontem", "yesterday").to_string(),
        _ => t.format("%d/%m").to_string(),
    };
    format!("{day}, {}", t.format("%H:%M"))
}

const PT_PREFIXES: [&str; 5] = ["me lembra de ", "me lembre de ", "lembra de ", "lembrete: ", "lembrete "];
const EN_PREFIXES: [&str; 3] = ["remind me to ", "remind me of ", "remind me "];
const WEEKDAYS: [(&str, Weekday); 19] = [
    ("segunda", Weekday::Mon), ("terça", Weekday::Tue), ("terca", Weekday::Tue), ("quarta", Weekday::Wed),
    ("quinta", Weekday::Thu), ("sexta", Weekday::Fri), ("sábado", Weekday::Sat), ("sabado", Weekday::Sat),
    ("domingo", Weekday::Sun), ("monday", Weekday::Mon), ("tuesday", Weekday::Tue), ("wednesday", Weekday::Wed),
    ("thursday", Weekday::Thu), ("friday", Weekday::Fri), ("saturday", Weekday::Sat), ("sunday", Weekday::Sun),
    ("seg", Weekday::Mon), ("sex", Weekday::Fri), ("qui", Weekday::Thu),
];
/// The hour a day-only request is due.
const MORNING: (u32, u32) = (9, 0);

/// "me lembra de X [às 14h | amanhã | na sexta]" / "remind me to X [at 2pm |
/// tomorrow | on friday]" → a request, or None when the sentence is not one.
pub fn parse_request(text: &str, now: DateTime<FixedOffset>, origin: Said) -> Option<Request> {
    let lower = text.trim().to_lowercase();
    let rest = PT_PREFIXES
        .iter()
        .chain(EN_PREFIXES.iter())
        .find_map(|p| lower.strip_prefix(p))?
        .trim()
        .to_string();
    if rest.is_empty() {
        return None;
    }
    let words: Vec<&str> = rest.split_whitespace().collect();
    let mut cut = words.len();
    let mut day: Option<chrono::NaiveDate> = None;
    let mut time: Option<NaiveTime> = None;
    for (i, w) in words.iter().enumerate() {
        let w = w.trim_matches(|c: char| c == ',' || c == '.');
        if w == "amanhã" || w == "amanha" || w == "tomorrow" {
            day = Some(now.date_naive() + Duration::days(1));
            cut = cut.min(i);
        } else if let Some((_, wd)) = WEEKDAYS.iter().find(|(n, _)| *n == w) {
            let today = now.date_naive();
            let ahead = (wd.num_days_from_monday() as i64 - today.weekday().num_days_from_monday() as i64 + 7) % 7;
            day = Some(today + Duration::days(if ahead == 0 { 7 } else { ahead }));
            // "na sexta" / "on friday": the preposition goes with the day.
            cut = cut.min(if i > 0 && ["na", "no", "on", "em"].contains(&words[i - 1]) { i - 1 } else { i });
        } else if (w == "às" || w == "as" || w == "at") && i + 1 < words.len() {
            if let Some(t) = parse_time(words[i + 1].trim_matches(|c: char| c == ',' || c == '.')) {
                time = Some(t);
                cut = cut.min(i);
            }
        }
    }
    let text = words[..cut].join(" ");
    if text.is_empty() {
        return None;
    }
    let due = match (day, time) {
        (None, None) => None,
        (d, t) => {
            let t = t.unwrap_or_else(|| NaiveTime::from_hms_opt(MORNING.0, MORNING.1, 0).unwrap());
            let mut d = d.unwrap_or(now.date_naive());
            // A time already gone today means tomorrow.
            if day.is_none() && t <= now.time() {
                d += Duration::days(1);
            }
            now.offset().from_local_datetime(&d.and_time(t)).single().map(|dt| dt.to_rfc3339_opts(SecondsFormat::Secs, false))
        }
    };
    Some(Request {
        id: format!("req-{}", now.timestamp_millis()),
        text,
        said_at: now.to_rfc3339_opts(SecondsFormat::Secs, false),
        due,
        origin,
        done_at: None,
    })
}

/// "14h", "14", "14:30", "2pm", "9:30am".
fn parse_time(token: &str) -> Option<NaiveTime> {
    let t = token.trim_end_matches('h');
    let (t, pm) = if let Some(x) = t.strip_suffix("pm") { (x, Some(true)) } else if let Some(x) = t.strip_suffix("am") { (x, Some(false)) } else { (t, None) };
    let (h, m) = match t.split_once(':') {
        Some((h, m)) => (h.parse::<u32>().ok()?, m.parse::<u32>().ok()?),
        None => (t.parse::<u32>().ok()?, 0),
    };
    let h = match pm {
        Some(true) if h < 12 => h + 12,
        Some(false) if h == 12 => 0,
        _ => h,
    };
    NaiveTime::from_hms_opt(h, m, 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::artifacts::{Kind, Node, Origin};
    use crate::domain::board::{Task, TaskStatus};
    use crate::domain::lang::Lang;
    use chrono::{DateTime, FixedOffset};

    fn at(s: &str) -> DateTime<FixedOffset> {
        DateTime::parse_from_rfc3339(s).unwrap()
    }
    fn task(title: &str, status: TaskStatus, updated: &str, ws: &str) -> Task {
        Task { title: title.into(), status, workspace: Some(ws.into()), session_ids: vec![], updated_at: updated.into(), note: None, pinned: false, subtasks: vec![] }
    }
    fn page(id: &str, updated: &str, origin: bool) -> Node {
        Node { id: id.into(), title: id.into(), path: format!("{id}.html"), parent: None, kind: Kind::Slides, origin: origin.then(|| Origin { session_id: None, task: Some("Redesign".into()) }), links: vec![], updated_at: updated.into() }
    }

    #[test]
    fn a_task_stopped_at_a_permission_comes_first_and_says_so() {
        let now = at("2026-11-04T10:00:00-03:00");
        let board = vec![
            task("Quick-add", TaskStatus::Doing, "2026-10-31T09:00:00-03:00", "/p/tulipa"),
            task("Billing v2", TaskStatus::Doing, "2026-11-04T09:50:00-03:00", "/p/hark"),
        ];
        let out = collect(&board, &["Billing v2"], &[], &[], now, Lang::Pt);
        assert_eq!(out[0].title, "Billing v2");
        assert_eq!(out[0].urgency, Urgency::NeedsYou);
        assert_eq!(out[0].source, Source::Board);
        assert_eq!(out[0].project.as_deref(), Some("/p/hark"));
        assert_eq!(out[1].title, "Quick-add");
        assert_eq!(out[1].urgency, Urgency::Stalled);
        assert_eq!(out[1].why, "parado há 4 dias");
    }

    #[test]
    fn work_abandoned_for_weeks_is_not_a_demand_of_the_day() {
        // Thirteen "parado há 65 dias" rows buried the real list: after two
        // weeks a doing task is forgotten work, not something asking today.
        let now = at("2026-11-04T10:00:00-03:00");
        let board = vec![
            task("Old migration", TaskStatus::Doing, "2026-08-31T09:00:00-03:00", "/p/hark"),
            task("Last week", TaskStatus::Doing, "2026-10-26T09:00:00-03:00", "/p/hark"),
        ];
        let out = collect(&board, &[], &[], &[], now, Lang::Pt);
        assert_eq!(out.iter().map(|d| d.title.as_str()).collect::<Vec<_>>(), vec!["Last week"]);
    }

    #[test]
    fn done_tasks_and_fresh_work_are_not_demands() {
        let now = at("2026-11-04T10:00:00-03:00");
        let board = vec![
            task("Shipped", TaskStatus::Done, "2026-11-01T09:00:00-03:00", "/p/hark"),
            task("Fresh", TaskStatus::Doing, "2026-11-04T09:00:00-03:00", "/p/hark"),
            task("Someday", TaskStatus::Backlog, "2026-10-01T09:00:00-03:00", "/p/hark"),
        ];
        assert!(collect(&board, &[], &[], &[], now, Lang::Pt).is_empty());
    }

    #[test]
    fn a_waiting_task_needs_you_and_a_page_born_today_asks_for_review() {
        let now = at("2026-11-04T10:00:00-03:00");
        let board = vec![task("Retry", TaskStatus::Waiting, "2026-11-03T09:00:00-03:00", "/p/hark")];
        let pages = vec![("landing".to_string(), page("deck", "2026-11-03T17:40:00-03:00", true)), ("landing".to_string(), page("old", "2026-10-20T17:40:00-03:00", true)), ("landing".to_string(), page("home", "2026-11-04T09:00:00-03:00", false))];
        let out = collect(&board, &[], &pages, &[], now, Lang::Pt);
        assert_eq!(out.len(), 2);
        assert_eq!((out[0].title.as_str(), out[0].urgency), ("Retry", Urgency::NeedsYou));
        assert_eq!((out[1].title.as_str(), out[1].urgency, out[1].source), ("deck", Urgency::Review, Source::Artifact));
        assert_eq!(out[1].link.as_deref(), Some("hark://artifacts/landing/deck"));
    }

    #[test]
    fn a_request_is_a_reminder_when_due_or_undated_and_silent_until_then() {
        let now = at("2026-11-04T10:00:00-03:00");
        let reqs = vec![
            Request { id: "r1".into(), text: "responder o contador".into(), said_at: "2026-11-03T18:10:00-03:00".into(), due: None, origin: Said::Text, done_at: None },
            Request { id: "r2".into(), text: "ligar pro banco".into(), said_at: "2026-11-04T08:00:00-03:00".into(), due: Some("2026-11-04T14:00:00-03:00".into()), origin: Said::Voice, done_at: None },
            Request { id: "r3".into(), text: "depois".into(), said_at: "2026-11-04T08:00:00-03:00".into(), due: Some("2026-11-06T09:00:00-03:00".into()), origin: Said::Voice, done_at: None },
            Request { id: "r4".into(), text: "feito".into(), said_at: "2026-11-01T08:00:00-03:00".into(), due: None, origin: Said::Text, done_at: Some("2026-11-02T08:00:00-03:00".into()) },
        ];
        let out = collect(&[], &[], &[], &reqs, now, Lang::Pt);
        let titles: Vec<&str> = out.iter().map(|d| d.title.as_str()).collect();
        assert_eq!(titles, vec!["ligar pro banco", "responder o contador"]);
        assert_eq!(out[0].urgency, Urgency::Scheduled);
        assert_eq!(out[0].why, "hoje às 14:00");
        assert_eq!(out[1].urgency, Urgency::Reminder);
        assert_eq!(out[1].why, "você pediu ontem, 18:10");
    }

    #[test]
    fn me_lembra_de_becomes_a_request_with_the_time_it_names() {
        let now = at("2026-11-04T10:00:00-03:00");
        let r = parse_request("me lembra de ligar pro banco às 14h", now, Said::Voice).unwrap();
        assert_eq!(r.text, "ligar pro banco");
        assert_eq!(r.due.as_deref(), Some("2026-11-04T14:00:00-03:00"));
        let r = parse_request("lembra de responder o contador", now, Said::Text).unwrap();
        assert_eq!((r.text.as_str(), r.due), ("responder o contador", None));
        let r = parse_request("remind me to call the bank tomorrow", now, Said::Text).unwrap();
        assert_eq!((r.text.as_str(), r.due.as_deref()), ("call the bank", Some("2026-11-05T09:00:00-03:00")));
        let r = parse_request("me lembra de revisar o deck na sexta", now, Said::Text).unwrap();
        assert_eq!(r.due.as_deref(), Some("2026-11-06T09:00:00-03:00"));
        let r = parse_request("remind me to stretch at 9:30", now, Said::Text).unwrap();
        assert_eq!(r.due.as_deref(), Some("2026-11-05T09:30:00-03:00"), "9:30 already passed today");
        assert!(parse_request("continua a migração do webhook", now, Said::Text).is_none());
    }
}

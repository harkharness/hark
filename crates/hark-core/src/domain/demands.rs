//! Demandas: what asks for the person today, from five sources — the board,
//! the agenda, the artifacts, the requests they made ("me lembra de…") and
//! the chats of the day, where the agent's last word means the ball is
//! with the person.
//! The team's board stays the team's; a demand is the person's and may
//! point at a task without depending on it. Pure: the shell gathers, this
//! decides and orders.

use crate::domain::artifacts::Node;
use crate::domain::board::{Task, TaskStatus};
use crate::domain::deep_link;
use crate::domain::lang::Lang;
use crate::domain::snapshot::SessionSummary;
use chrono::{DateTime, Datelike, Duration, FixedOffset, NaiveTime, SecondsFormat, TimeZone, Weekday};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    Board,
    Agenda,
    Artifact,
    Request,
    /// A chat of the day, from the index: not a card, the conversation itself.
    Session,
}

/// Most pressing first: what stopped on you, what is due today, what is
/// being worked on, what stalled, what waits for a look, what you asked
/// to be reminded of.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Urgency {
    NeedsYou,
    Scheduled,
    /// A chat where you spoke last: the agent is on it, nothing to do yet.
    Working,
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
/// A chat with a turn inside this window is a chat of the day.
const SESSION_LIVE_HOURS: i64 = 24;
/// Where the forgotten-cards line points.
pub const BOARD_LINK: &str = "hark://board";

/// The list of the day: board (with the titles whose permission waits),
/// pages by space, requests. Ordered by urgency, then newest first.
pub fn collect(
    board: &[Task],
    permissions_waiting: &[&str],
    pages: &[(String, Node)],
    requests: &[Request],
    sessions: &[SessionSummary],
    now: DateTime<FixedOffset>,
    lang: Lang,
) -> Vec<Demand> {
    let mut out = Vec::new();
    let live: Vec<&SessionSummary> = sessions
        .iter()
        .filter(|s| s.last_ts.as_deref().is_some_and(|ts| age_hours(ts, now) < SESSION_LIVE_HOURS))
        .collect();
    let is_live = |task: &Task| task.session_ids.iter().any(|id| live.iter().any(|s| s.session_id == *id));
    // Cards that keep the floor: their chats stay off the list.
    let mut covered: Vec<&str> = Vec::new();
    let mut forgotten = 0usize;
    for task in board {
        let (urgency, why) = if permissions_waiting.contains(&task.title.as_str()) {
            (Urgency::NeedsYou, lang.pick("parou numa permissão", "stopped at a permission").to_string())
        } else {
            match task.status {
                TaskStatus::Waiting => (Urgency::NeedsYou, lang.pick("esperando você", "waiting for you").to_string()),
                // A doing card with a chat of the day is told by the chat.
                TaskStatus::Doing if is_live(task) => continue,
                TaskStatus::Doing => {
                    let days = age_days(&task.updated_at, now);
                    if days > STALLED_MAX_DAYS {
                        forgotten += 1;
                        continue;
                    }
                    if days < STALLED_DAYS {
                        continue;
                    }
                    (Urgency::Stalled, format!("{} {days} {}", lang.pick("parado há", "stalled for"), lang.pick("dias", "days")))
                }
                TaskStatus::Backlog | TaskStatus::Done => continue,
            }
        };
        covered.extend(task.session_ids.iter().map(String::as_str));
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
    if forgotten > 0 {
        out.push(Demand {
            id: "board:forgotten".into(),
            source: Source::Board,
            project: None,
            title: format!("{forgotten} {}", lang.pick("cards parados há semanas", "cards stalled for weeks")),
            why: format!("{} {STALLED_MAX_DAYS} {}", lang.pick("mais de", "over"), lang.pick("dias sem um turno", "days without a turn")),
            urgency: Urgency::Stalled,
            link: Some(BOARD_LINK.into()),
            at: String::new(),
        });
    }
    for session in &live {
        if covered.contains(&session.session_id.as_str()) {
            continue;
        }
        let spoke = session.recent_prompts.iter().map(|p| p.ts.as_str()).max();
        let replied = session.last_reply.as_ref().map(|r| r.ts.as_str());
        let (urgency, why, at) = match (spoke, replied) {
            (Some(s), Some(r)) if r < s => (
                Urgency::Working,
                format!("{} {}, {}", lang.pick("você falou às", "you spoke at"), clock(s, now), lang.pick("agente trabalhando", "agent working")),
                s,
            ),
            (_, Some(r)) => (
                Urgency::NeedsYou,
                format!("{} {}, {}", lang.pick("agente respondeu às", "agent replied at"), clock(r, now), lang.pick("esperando você", "waiting for you")),
                r,
            ),
            (Some(s), None) => (
                Urgency::Working,
                format!("{} {}, {}", lang.pick("você falou às", "you spoke at"), clock(s, now), lang.pick("agente trabalhando", "agent working")),
                s,
            ),
            (None, None) => continue,
        };
        let card = board.iter().find(|t| t.session_ids.contains(&session.session_id));
        let title = card
            .map(|t| t.title.clone())
            .or_else(|| session.title.clone())
            .or_else(|| session.last_prompt.clone())
            .unwrap_or_else(|| session.session_id.clone());
        out.push(Demand {
            id: format!("session:{}", session.session_id),
            source: Source::Session,
            project: card.and_then(|t| t.workspace.clone()).or_else(|| session.cwd.clone()),
            title,
            why,
            urgency,
            link: Some(deep_link::chat_link(&session.session_id)),
            at: at.to_string(),
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

/// "08:52" in the person's zone.
fn clock(ts: &str, now: DateTime<FixedOffset>) -> String {
    parse_ts(ts).map(|t| t.with_timezone(now.offset()).format("%H:%M").to_string()).unwrap_or_default()
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
    use crate::domain::snapshot::{RecentPrompt, SessionSummary};
    use chrono::{DateTime, FixedOffset};

    fn at(s: &str) -> DateTime<FixedOffset> {
        DateTime::parse_from_rfc3339(s).unwrap()
    }
    fn task(title: &str, status: TaskStatus, updated: &str, ws: &str) -> Task {
        Task { title: title.into(), status, workspace: Some(ws.into()), session_ids: vec![], updated_at: updated.into(), note: None, pinned: false, subtasks: vec![] }
    }
    fn session(id: &str, title: &str, prompt_ts: &str, reply_ts: &str, cwd: &str) -> SessionSummary {
        SessionSummary {
            session_id: id.into(),
            cwd: Some(cwd.into()),
            git_branch: None,
            title: Some(title.into()),
            last_prompt: Some("…".into()),
            last_ts: Some(prompt_ts.max(reply_ts).into()),
            recent_prompts: vec![RecentPrompt { ts: prompt_ts.into(), text: "…".into() }],
            last_reply: Some(RecentPrompt { ts: reply_ts.into(), text: "…".into() }),
        }
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
        let out = collect(&board, &["Billing v2"], &[], &[], &[], now, Lang::Pt);
        assert_eq!(out[0].title, "Billing v2");
        assert_eq!(out[0].urgency, Urgency::NeedsYou);
        assert_eq!(out[0].source, Source::Board);
        assert_eq!(out[0].project.as_deref(), Some("/p/hark"));
        assert_eq!(out[1].title, "Quick-add");
        assert_eq!(out[1].urgency, Urgency::Stalled);
        assert_eq!(out[1].why, "parado há 4 dias");
    }

    #[test]
    fn forgotten_cards_collapse_into_one_line_instead_of_burying_the_list() {
        // Thirteen "parado há 65 dias" rows buried the real list; hiding
        // them left "nada pede você hoje" over a full board (05/11). One
        // line counts them and points at the board.
        let now = at("2026-11-04T10:00:00-03:00");
        let board = vec![
            task("Old migration", TaskStatus::Doing, "2026-08-31T09:00:00-03:00", "/p/hark"),
            task("Older chart", TaskStatus::Doing, "2026-08-31T09:00:00-03:00", "/p/tulipa"),
            task("Last week", TaskStatus::Doing, "2026-10-26T09:00:00-03:00", "/p/hark"),
        ];
        let out = collect(&board, &[], &[], &[], &[], now, Lang::Pt);
        assert_eq!(out.iter().map(|d| d.title.as_str()).collect::<Vec<_>>(), vec!["Last week", "2 cards parados há semanas"]);
        assert_eq!((out[1].urgency, out[1].source, out[1].link.as_deref()), (Urgency::Stalled, Source::Board, Some("hark://board")));
    }

    #[test]
    fn a_session_whose_last_word_is_the_agents_asks_for_you() {
        // The real list of the day: three chats where the agent answered
        // and waits. None was on the board, so Demandas showed nothing.
        let now = at("2026-11-05T09:00:00-03:00");
        let sessions = vec![session("s1", "Patch de segurança", "2026-11-05T08:40:00-03:00", "2026-11-05T08:52:00-03:00", "/p/infra")];
        let out = collect(&[], &[], &[], &[], &sessions, now, Lang::Pt);
        assert_eq!(out.len(), 1);
        assert_eq!((out[0].source, out[0].urgency), (Source::Session, Urgency::NeedsYou));
        assert_eq!(out[0].title, "Patch de segurança");
        assert_eq!(out[0].why, "agente respondeu às 08:52, esperando você");
        assert_eq!(out[0].link.as_deref(), Some("hark://chat/s1"));
        assert_eq!(out[0].project.as_deref(), Some("/p/infra"));
    }

    #[test]
    fn a_session_where_you_spoke_last_is_working_not_asking() {
        let now = at("2026-11-05T09:00:00-03:00");
        let sessions = vec![session("s1", "Patch", "2026-11-05T08:55:00-03:00", "2026-11-05T08:52:00-03:00", "/p/infra")];
        let out = collect(&[], &[], &[], &[], &sessions, now, Lang::En);
        assert_eq!(out[0].urgency, Urgency::Working);
        assert_eq!(out[0].why, "you spoke at 08:55, agent working");
    }

    #[test]
    fn a_session_silent_for_a_day_is_not_a_demand() {
        let now = at("2026-11-05T09:00:00-03:00");
        let sessions = vec![session("s1", "Yesterday", "2026-11-04T08:00:00-03:00", "2026-11-04T08:30:00-03:00", "/p/infra")];
        assert!(collect(&[], &[], &[], &[], &sessions, now, Lang::Pt).is_empty());
    }

    #[test]
    fn a_card_tied_to_a_live_session_yields_to_the_session_and_lends_its_name() {
        // The card says "parado há 5 dias"; the chat says the agent spoke
        // ten minutes ago. The chat is the truth, the card names it.
        let now = at("2026-11-05T09:00:00-03:00");
        let mut card = task("KR 1.3 incidentes", TaskStatus::Doing, "2026-10-31T09:00:00-03:00", "/p/sre");
        card.session_ids = vec!["s1".into()];
        let sessions = vec![session("s1", "kr 1.3 estrutura", "2026-11-05T08:40:00-03:00", "2026-11-05T08:50:00-03:00", "/p/sre")];
        let out = collect(&[card], &[], &[], &[], &sessions, now, Lang::Pt);
        assert_eq!(out.len(), 1);
        assert_eq!((out[0].source, out[0].title.as_str()), (Source::Session, "KR 1.3 incidentes"));
    }

    #[test]
    fn a_card_stopped_at_a_permission_keeps_the_floor_over_its_session() {
        let now = at("2026-11-05T09:00:00-03:00");
        let mut card = task("Billing v2", TaskStatus::Doing, "2026-11-05T08:50:00-03:00", "/p/hark");
        card.session_ids = vec!["s1".into()];
        let sessions = vec![session("s1", "billing", "2026-11-05T08:40:00-03:00", "2026-11-05T08:50:00-03:00", "/p/hark")];
        let out = collect(&[card], &["Billing v2"], &[], &[], &sessions, now, Lang::Pt);
        assert_eq!(out.len(), 1);
        assert_eq!((out[0].source, out[0].why.as_str()), (Source::Board, "parou numa permissão"));
    }

    #[test]
    fn done_tasks_and_fresh_work_are_not_demands() {
        let now = at("2026-11-04T10:00:00-03:00");
        let board = vec![
            task("Shipped", TaskStatus::Done, "2026-11-01T09:00:00-03:00", "/p/hark"),
            task("Fresh", TaskStatus::Doing, "2026-11-04T09:00:00-03:00", "/p/hark"),
            task("Someday", TaskStatus::Backlog, "2026-10-01T09:00:00-03:00", "/p/hark"),
        ];
        assert!(collect(&board, &[], &[], &[], &[], now, Lang::Pt).is_empty());
    }

    #[test]
    fn a_waiting_task_needs_you_and_a_page_born_today_asks_for_review() {
        let now = at("2026-11-04T10:00:00-03:00");
        let board = vec![task("Retry", TaskStatus::Waiting, "2026-11-03T09:00:00-03:00", "/p/hark")];
        let pages = vec![("landing".to_string(), page("deck", "2026-11-03T17:40:00-03:00", true)), ("landing".to_string(), page("old", "2026-10-20T17:40:00-03:00", true)), ("landing".to_string(), page("home", "2026-11-04T09:00:00-03:00", false))];
        let out = collect(&board, &[], &pages, &[], &[], now, Lang::Pt);
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
        let out = collect(&[], &[], &[], &reqs, &[], now, Lang::Pt);
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

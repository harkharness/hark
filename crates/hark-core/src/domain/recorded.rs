//! Hark's own session record: the history of an agent that leaves none on
//! disk (an ACP agent), written by Hark as the turns happen. Pure — the
//! line format, and how a record folds into the same index and the same
//! transcript reader claude's files feed.
//!
//! One JSON object per line, three kinds: a header (who, where, when), the
//! conversation as `transcript::Entry` written verbatim — so the viewer,
//! the mirror, the brief and the crossref read a record with the reader
//! they already have — and one usage line per model per turn.

use crate::domain::snapshot::SessionSummary;
use crate::domain::spend::{SpendKind, SpendRow, SpendSource};
use crate::domain::transcript::{Entry, Role};
use hark_agent::{SessionEvent, TokenUsage};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The first line of a record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Header {
    /// Registry id of the agent that ran the session.
    pub agent: String,
    pub session_id: String,
    /// The project the session ran in.
    pub cwd: String,
    pub ts: String,
}

/// One model's tokens for one turn.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Usage {
    pub ts: String,
    pub model: String,
    pub usage: TokenUsage,
    /// What the agent said the turn cost, when it did. Kept for the
    /// record; the ledger's dollars come from the live row, not from here.
    pub cost_usd: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Line {
    Header(Header),
    Entry(Entry),
    Usage(Usage),
}

/// One line of text, no newline.
pub fn render(line: &Line) -> String {
    match line {
        Line::Header(h) => {
            let mut v = serde_json::to_value(h).unwrap_or(Value::Null);
            v["hark"] = Value::from("session");
            v.to_string()
        }
        Line::Entry(e) => serde_json::to_value(e).unwrap_or(Value::Null).to_string(),
        Line::Usage(u) => {
            let mut v = serde_json::to_value(u).unwrap_or(Value::Null);
            v["hark"] = Value::from("usage");
            v.to_string()
        }
    }
}

/// Read one line back. Anything that is not ours — a claude line, noise —
/// is None, so a reader can run over any file without harm.
pub fn parse(text: &str) -> Option<Line> {
    let v: Value = serde_json::from_str(text).ok()?;
    match v.get("hark").and_then(Value::as_str) {
        Some("session") => serde_json::from_value(v).ok().map(Line::Header),
        Some("usage") => serde_json::from_value(v).ok().map(Line::Usage),
        Some(_) => None,
        // An entry is recognised by its role; a claude line has none at the top.
        None if v.get("role").is_some() => serde_json::from_value(v).ok().map(Line::Entry),
        None => None,
    }
}

/// What the index makes of a line. The user's words are a prompt (the
/// title, the search, the funnel); the agent's words only move the clock;
/// a usage line is the token history. The header is folded separately —
/// it carries the cwd, which no prompt line repeats.
pub fn session_event(line: &Line, session_id: &str) -> Option<SessionEvent> {
    match line {
        Line::Header(_) => None,
        Line::Entry(Entry { ts, role: Role::User, text, .. }) => Some(SessionEvent::UserPrompt {
            ts: ts.clone(),
            text: text.clone(),
            cwd: None,
            git_branch: None,
        }),
        Line::Entry(Entry { ts, .. }) => Some(SessionEvent::Activity { ts: ts.clone() }),
        Line::Usage(u) => Some(SessionEvent::AssistantUsage {
            ts: u.ts.clone(),
            request_id: Some(request_id(session_id, u)),
            model: Some(u.model.clone()),
            usage: TokenUsage { ..u.usage },
            is_sidechain: false,
        }),
    }
}

/// The dedup key of a usage line: a record is rebuilt idempotently.
fn request_id(session_id: &str, u: &Usage) -> String {
    format!("{session_id}:{}:{}", u.ts, u.model)
}

/// Fold one line into the summary the index keeps — the same summary
/// claude's files fold into, so search, funnel and viewer see one kind.
pub fn fold(summary: SessionSummary, line: &Line) -> SessionSummary {
    match line {
        Line::Header(h) => SessionSummary {
            // The file is named after the id, sanitised; the header has it verbatim.
            session_id: h.session_id.clone(),
            cwd: Some(h.cwd.clone()),
            last_ts: summary.last_ts.max(Some(h.ts.clone())),
            ..summary
        },
        other => match session_event(other, &summary.session_id.clone()) {
            Some(event) => summary.apply(event),
            None => summary,
        },
    }
}

/// The ledger row of a usage line: tokens only, source "jsonl", like the
/// rows claude's files feed — the live row already carries the dollars,
/// and the two sources are never summed. Nothing for a line with no
/// tokens.
pub fn spend_row(u: &Usage, session_id: &str, workspace: Option<&str>) -> Option<SpendRow> {
    if u.usage.input + u.usage.output + u.usage.cache_read + u.usage.cache_created == 0 {
        return None;
    }
    Some(SpendRow {
        ts: u.ts.clone(),
        kind: SpendKind::Session,
        source: SpendSource::Jsonl,
        task_id: None,
        label: None,
        session_id: Some(session_id.to_string()),
        workspace: workspace.map(String::from),
        model: u.model.clone(),
        usage: TokenUsage { ..u.usage },
        cost_usd: None,
        duration_ms: None,
        is_error: false,
        is_sidechain: false,
        context_window: None,
        request_id: Some(request_id(session_id, u)),
        outcome: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::snapshot::SessionSummary;
    use crate::domain::spend::{SpendKind, SpendSource};
    use crate::domain::transcript::{self, Entry, Role};
    use hark_agent::{SessionEvent, TokenUsage};

    fn header() -> Line {
        Line::Header(Header {
            agent: "gemini".into(),
            session_id: "s-1".into(),
            cwd: "/p/hark".into(),
            ts: "2026-09-14T12:00:00.000Z".into(),
        })
    }
    fn said(role: Role, text: &str) -> Line {
        Line::Entry(Entry { ts: "2026-09-14T12:00:01.000Z".into(), role, text: text.into(), tool: None, is_error: false })
    }
    fn usage() -> Line {
        Line::Usage(Usage {
            ts: "2026-09-14T12:00:02.000Z".into(),
            model: "gemini-2.5-pro".into(),
            usage: TokenUsage { input: 10, output: 20, cache_read: 0, cache_created: 0 },
            cost_usd: Some(0.01),
        })
    }

    #[test]
    fn every_line_kind_round_trips_through_its_own_text() {
        for line in [header(), said(Role::User, "abre o PR"), usage()] {
            let text = render(&line);
            assert!(!text.contains('\n'), "one line each: {text}");
            assert_eq!(parse(&text), Some(line), "{text}");
        }
    }

    #[test]
    fn a_recorded_entry_reads_back_through_the_transcript_parser() {
        // The viewer, the mirror, the brief and the crossref all read files
        // through transcript::parse_entry. A recorded line is an Entry as
        // written — no second reader, no path sniffing.
        let tool = Line::Entry(Entry {
            ts: "t".into(), role: Role::ToolUse, text: r#"{"command":"ls"}"#.into(), tool: Some("execute".into()), is_error: false,
        });
        for line in [said(Role::User, "abre o PR"), said(Role::Assistant, "abrindo"), tool] {
            let Line::Entry(expected) = &line else { unreachable!() };
            assert_eq!(transcript::parse_entry(&render(&line)).as_ref(), Some(expected));
        }
        // Lines that are not conversation are not entries.
        assert_eq!(transcript::parse_entry(&render(&header())), None);
        assert_eq!(transcript::parse_entry(&render(&usage())), None);
    }

    #[test]
    fn a_user_line_is_a_prompt_and_a_usage_line_is_usage() {
        assert_eq!(
            session_event(&said(Role::User, "abre o PR"), "s-1"),
            Some(SessionEvent::UserPrompt { ts: "2026-09-14T12:00:01.000Z".into(), text: "abre o PR".into(), cwd: None, git_branch: None })
        );
        // The agent's words only move the clock; the tokens feed the ledger.
        assert_eq!(session_event(&said(Role::Assistant, "abrindo"), "s-1"), Some(SessionEvent::Activity { ts: "2026-09-14T12:00:01.000Z".into() }));
        assert_eq!(
            session_event(&usage(), "s-1"),
            Some(SessionEvent::AssistantUsage {
                ts: "2026-09-14T12:00:02.000Z".into(),
                request_id: Some("s-1:2026-09-14T12:00:02.000Z:gemini-2.5-pro".into()),
                model: Some("gemini-2.5-pro".into()),
                usage: TokenUsage { input: 10, output: 20, cache_read: 0, cache_created: 0 },
                is_sidechain: false,
            })
        );
        assert_eq!(session_event(&header(), "s-1"), None);
    }

    #[test]
    fn the_fold_takes_the_cwd_from_the_header_and_the_title_from_the_first_prompt() {
        let summary = [header(), said(Role::User, "abre o PR do webhook"), said(Role::Assistant, "abrindo"), usage()]
            .iter()
            .fold(SessionSummary::new("s-1"), fold);
        assert_eq!(summary.cwd.as_deref(), Some("/p/hark"));
        assert_eq!(summary.title.as_deref(), Some("abre o PR do webhook"));
        assert_eq!(summary.last_prompt.as_deref(), Some("abre o PR do webhook"));
        assert_eq!(summary.recent_prompts.len(), 1);
        // The clock follows the newest line, whatever its kind.
        assert_eq!(summary.last_ts.as_deref(), Some("2026-09-14T12:00:02.000Z"));
    }

    #[test]
    fn a_usage_line_becomes_a_token_row_without_dollars() {
        // Same shape as claude's file-fed rows: source jsonl, tokens only —
        // the live ledger already holds this turn's dollars, and the two
        // sources are never summed.
        let Line::Usage(u) = usage() else { unreachable!() };
        let row = spend_row(&u, "s-1", Some("/p/hark")).expect("a row");
        assert_eq!(row.kind, SpendKind::Session);
        assert_eq!(row.source, SpendSource::Jsonl);
        assert_eq!(row.cost_usd, None);
        assert_eq!(row.session_id.as_deref(), Some("s-1"));
        assert_eq!(row.workspace.as_deref(), Some("/p/hark"));
        assert_eq!(row.request_id.as_deref(), Some("s-1:2026-09-14T12:00:02.000Z:gemini-2.5-pro"));
        let zero = Usage { usage: TokenUsage::default(), ..u };
        assert_eq!(spend_row(&zero, "s-1", None), None);
    }

    #[test]
    fn foreign_lines_are_nothing() {
        // A claude session line, and noise: not ours.
        assert_eq!(parse(r#"{"type":"user","message":{"role":"user","content":"oi"},"timestamp":"t"}"#), None);
        assert_eq!(parse("not json"), None);
        assert_eq!(parse(r#"{"hark":"something-else"}"#), None);
    }
}

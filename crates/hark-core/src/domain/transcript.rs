//! Pure reader of a session log into something a human can read back.
//! Used by the read-only thread viewer: no LLM, no tokens, just the file.

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    User,
    Assistant,
    ToolUse,
    ToolResult,
    /// The summary the CLI writes to itself when it compacts. A plain
    /// `user` line in the log, and pages long: it belongs to the history
    /// but not to the conversation, so the reader folds it away.
    Compaction,
}

/// One readable line of a past conversation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub ts: String,
    pub role: Role,
    /// Prose, or the tool input JSON for `ToolUse`.
    pub text: String,
    pub tool: Option<String>,
    pub is_error: bool,
}

/// Notes the apps that drive a session append to a person's message, by
/// tag: a hint about the widgets on screen, a reminder, what the editor
/// has open or selected. Named one by one: any other markup in a message
/// is the person's own.
const APP_NOTES: [&str; 4] = ["widget_context_hint", "system-reminder", "ide_selection", "ide_opened_file"];

/// What a person typed, out of a user message as the session file holds
/// it: the text without the notes an app appended for the model. None
/// when nothing typed is left, or when the whole message is an injected
/// payload (a command echo, a skill body), which starts with a tag.
///
/// A session talked to in the Claude app and opened here showed its
/// widget hint at the end of every message of the person's.
pub fn typed_text(text: &str) -> Option<String> {
    let mut out = text.to_string();
    for tag in APP_NOTES {
        let (open, close) = (format!("<{tag}>"), format!("</{tag}>"));
        while let Some(start) = out.find(&open) {
            // Never closed: a word in a sentence, not a note.
            let Some(len) = out[start..].find(&close) else { break };
            out.replace_range(start..start + len + close.len(), "");
        }
    }
    let out = out.trim();
    (!out.is_empty() && !out.starts_with('<')).then(|| out.to_string())
}

/// Parse one session-log line, skipping everything that is not conversation.
///
/// Two dialects, told apart per line: claude's own session file, and
/// Hark's record of an agent that leaves no file (`domain::recorded`),
/// whose conversation lines are an `Entry` written verbatim — recognised
/// by a top-level `role`, which a claude line never has.
pub fn parse_entry(line: &str) -> Option<Entry> {
    let v: Value = serde_json::from_str(line).ok()?;
    if v.get("role").is_some() && v.get("type").is_none() {
        return serde_json::from_value(v).ok();
    }
    let ts = v.get("timestamp")?.as_str()?.to_string();
    if v.get("isSidechain").and_then(Value::as_bool) == Some(true) {
        return None;
    }
    let content = v.get("message")?.get("content")?;
    let entry = |role: Role, text: String, tool: Option<String>, is_error: bool| {
        Some(Entry {
            ts: ts.clone(),
            role,
            text,
            tool,
            is_error,
        })
    };

    match v.get("type")?.as_str()? {
        "user" if v.get("isCompactSummary").and_then(Value::as_bool) == Some(true) => {
            entry(Role::Compaction, content.as_str().unwrap_or_default().to_string(), None, false)
        }
        "user" => match content {
            // Human prompt, unless it is an injected command/skill payload.
            Value::String(text) => entry(Role::User, typed_text(text)?, None, false),
            Value::Array(blocks) => blocks.iter().find_map(|b| {
                match b.get("type").and_then(Value::as_str)? {
                    "tool_result" => entry(
                        Role::ToolResult,
                        match b.get("content") {
                            Some(Value::String(s)) => s.clone(),
                            Some(other) => other.to_string(),
                            None => String::new(),
                        },
                        None,
                        b.get("is_error").and_then(Value::as_bool).unwrap_or(false),
                    ),
                    "text" => {
                        entry(Role::User, typed_text(b.get("text")?.as_str()?)?, None, false)
                    }
                    _ => None,
                }
            }),
            _ => None,
        },
        "assistant" => content.as_array()?.iter().find_map(|b| {
            match b.get("type").and_then(Value::as_str)? {
                "text" => {
                    let text = b.get("text")?.as_str()?;
                    (!text.trim().is_empty())
                        .then(|| entry(Role::Assistant, text.to_string(), None, false))?
                }
                "tool_use" => entry(
                    Role::ToolUse,
                    b.get("input")?.to_string(),
                    b.get("name").and_then(Value::as_str).map(String::from),
                    false,
                ),
                _ => None,
            }
        }),
        _ => None,
    }
}

/// Parse a whole log and keep the newest `n` entries, in order.
pub fn tail_entries<'a>(lines: impl Iterator<Item = &'a str>, n: usize) -> Vec<Entry> {
    let entries: Vec<Entry> = lines.filter_map(parse_entry).collect();
    let skip = entries.len().saturating_sub(n);
    entries.into_iter().skip(skip).collect()
}

/// Where a mirror read starts: from the offset it kept, never more than
/// `max` bytes before the end. A log truncated or replaced under us
/// (offset past the end) starts over; a chat held at a terminal can be a
/// 237 MB desktop log, and reading it from zero froze the window.
pub fn mirror_start(from: u64, len: u64, max: u64) -> u64 {
    let from = if from > len { 0 } else { from };
    from.max(len.saturating_sub(max))
}

/// A LOCAL, zero-token summary of a conversation: the user/assistant turns
/// (tool noise dropped), newest kept when the budget cuts. Feeds the
/// "restart light" flow — a fresh session opens with this instead of a
/// paid re-read of a heavy history.
pub fn brief(entries: &[Entry], max_chars: usize) -> String {
    let lines: Vec<String> = entries
        .iter()
        .filter(|e| matches!(e.role, Role::User | Role::Assistant))
        .map(|e| {
            let who = if e.role == Role::User { "você" } else { "assistente" };
            let text: String = e.text.split_whitespace().collect::<Vec<_>>().join(" ");
            let clipped: String = text.chars().take(300).collect();
            format!("{who}: {clipped}")
        })
        .collect();
    // Keep the newest lines that fit, then restore chronological order.
    let mut kept: Vec<&String> = Vec::new();
    let mut used = 0;
    for line in lines.iter().rev() {
        let cost = line.chars().count() + 1;
        if used + cost > max_chars {
            break;
        }
        used += cost;
        kept.push(line);
    }
    kept.reverse();
    kept.iter().map(|s| s.as_str()).collect::<Vec<_>>().join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(role: Role, text: &str) -> Entry {
        Entry {
            ts: "2026-08-21T10:00:00Z".into(),
            role,
            text: text.into(),
            tool: None,
            is_error: false,
        }
    }

    /// A compaction is a wall of text the CLI writes to itself: the log
    /// line is a plain `user` message, so reopening a chat pasted the
    /// whole summary into the conversation as if the user had typed it.
    /// Line shape taken from a real session (v2.1.220).
    #[test]
    fn a_compaction_summary_is_marked_not_mistaken_for_the_user() {
        let line = r#"{"type":"user","isCompactSummary":true,"timestamp":"2026-08-25T11:35:40.000Z",
            "message":{"role":"user","content":"This session is being continued from a previous conversation that ran out of context. The summary below covers …"}}"#;
        let entry = parse_entry(line).expect("the summary is part of the history");
        assert_eq!(entry.role, Role::Compaction);
        assert!(entry.text.contains("continued from a previous"), "text is kept, just folded");
    }

    #[test]
    fn the_boundary_line_and_command_echo_stay_out() {
        // The boundary carries no prose (the summary above is the record).
        assert_eq!(
            parse_entry(
                r#"{"type":"system","subtype":"compact_boundary","content":"Conversation compacted",
                "timestamp":"2026-08-25T11:35:39.737Z","compactMetadata":{"preTokens":659311}}"#
            ),
            None
        );
        // The CLI's own echo of the command is markup, not conversation.
        assert_eq!(
            parse_entry(
                r#"{"type":"user","timestamp":"2026-08-25T11:35:41.000Z","message":{"role":"user",
                "content":"<command-name>/compact</command-name>"}}"#
            ),
            None
        );
    }

    #[test]
    fn brief_keeps_conversation_drops_tool_noise_and_fits_budget() {
        let entries = vec![
            e(Role::User, "migra o webhook pro cluster novo"),
            e(Role::ToolUse, r#"{"command":"kubectl get pods"}"#),
            e(Role::ToolResult, "pod-1 Running\npod-2 Running"),
            e(Role::Assistant, "Migrei o service e o ingress; falta o DNS."),
            e(Role::User, "segue com o DNS então"),
        ];
        let out = brief(&entries, 2000);
        assert!(out.contains("migra o webhook"), "user turns survive");
        assert!(out.contains("falta o DNS"), "assistant turns survive");
        assert!(!out.contains("kubectl get pods"), "tool payloads stay out");
        assert!(out.contains("você:") && out.contains("assistente:"), "labeled");
    }

    #[test]
    fn brief_clips_to_budget_keeping_the_newest_turns() {
        let mut entries = vec![e(Role::User, "primeira instrução antiga")];
        for i in 0..80 {
            entries.push(e(Role::Assistant, &format!("resposta longa número {i} {}", "x".repeat(120))));
        }
        entries.push(e(Role::User, "instrução mais recente importante"));
        let out = brief(&entries, 1200);
        assert!(out.chars().count() <= 1300, "hard budget");
        assert!(out.contains("instrução mais recente"), "newest survives the cut");
        assert!(!out.contains("primeira instrução antiga"), "oldest is dropped first");
    }

    #[test]
    fn brief_of_nothing_is_empty() {
        assert_eq!(brief(&[], 500), "");
    }

    #[test]
    fn reads_human_prompts_and_assistant_prose() {
        let lines = [
            r#"{"type":"user","timestamp":"2026-08-15T10:00:00.000Z","message":{"role":"user","content":"migra o webhook"}}"#,
            r#"{"type":"assistant","timestamp":"2026-08-15T10:00:05.000Z","message":{"role":"assistant","content":[{"type":"text","text":"Vou começar pelo values."}]}}"#,
        ];
        let entries: Vec<_> = lines.iter().filter_map(|l| parse_entry(l)).collect();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].role, Role::User);
        assert_eq!(entries[0].text, "migra o webhook");
        assert_eq!(entries[0].ts, "2026-08-15T10:00:00.000Z");
        assert_eq!(entries[1].role, Role::Assistant);
        assert_eq!(entries[1].text, "Vou começar pelo values.");
    }

    #[test]
    fn reads_tool_calls_and_results() {
        let call = r#"{"type":"assistant","timestamp":"2026-08-15T10:01:00.000Z","message":{"role":"assistant","content":[{"type":"tool_use","name":"Bash","input":{"command":"git status"}}]}}"#;
        let result = r#"{"type":"user","timestamp":"2026-08-15T10:01:02.000Z","message":{"role":"user","content":[{"type":"tool_result","content":"clean","is_error":false}]}}"#;

        let call = parse_entry(call).unwrap();
        assert_eq!(call.role, Role::ToolUse);
        assert_eq!(call.tool.as_deref(), Some("Bash"));
        assert!(call.text.contains("git status"));

        let result = parse_entry(result).unwrap();
        assert_eq!(result.role, Role::ToolResult);
        assert_eq!(result.text, "clean");
        assert!(!result.is_error);
    }

    #[test]
    fn skips_noise_that_is_not_conversation() {
        // Sidechains, injected command tags, and bookkeeping lines.
        for line in [
            r#"{"type":"user","timestamp":"t","isSidechain":true,"message":{"role":"user","content":"subagent"}}"#,
            r#"{"type":"user","timestamp":"t","message":{"role":"user","content":"<command-name>/model</command-name>"}}"#,
            r#"{"type":"queue-operation","timestamp":"t"}"#,
            "not json",
        ] {
            assert_eq!(parse_entry(line), None, "should skip: {line}");
        }
    }

    // The apps that drive a session append notes of their own to what the
    // person typed, in the same message: a hint about widgets, a reminder,
    // what the editor has selected. They are for the model, not for the page.
    const HINT: &str = "<widget_context_hint>Interactive widgets in this conversation: draft. To read a widget's current state, call read_widget_context.</widget_context_hint>";

    #[test]
    fn a_note_the_app_appended_to_a_message_is_not_what_the_person_typed() {
        let text = format!("posta 1 linha só\n\n{HINT}");
        assert_eq!(typed_text(&text).as_deref(), Some("posta 1 linha só"));
    }

    #[test]
    fn every_kind_of_app_note_is_taken_out_wherever_it_sits() {
        let text = "<system-reminder>be brief</system-reminder>\nolha isso\n<ide_selection>fn main() {}</ide_selection>\ne aquilo\n<ide_opened_file>src/main.rs</ide_opened_file>";
        assert_eq!(typed_text(text).as_deref(), Some("olha isso\n\ne aquilo"));
    }

    #[test]
    fn a_message_that_was_only_app_notes_is_no_message() {
        assert_eq!(typed_text(HINT), None);
        assert_eq!(typed_text(&format!("  {HINT}\n<system-reminder>x</system-reminder> ")), None);
    }

    #[test]
    fn a_message_left_as_typed_when_the_app_added_nothing() {
        assert_eq!(typed_text("git status").as_deref(), Some("git status"));
        // A blank line the person typed between two paragraphs stays.
        assert_eq!(typed_text("um\n\ndois").as_deref(), Some("um\n\ndois"));
    }

    #[test]
    fn markup_the_person_typed_is_theirs() {
        // Only the apps' own notes go: a tag in the middle of a question
        // about html is the question.
        let text = "por que o <widget> não fecha? veja <b>isso</b>";
        assert_eq!(typed_text(text).as_deref(), Some(text));
    }

    #[test]
    fn a_note_never_closed_takes_nothing_with_it() {
        let text = "falei de <system-reminder> ontem";
        assert_eq!(typed_text(text).as_deref(), Some(text));
    }

    #[test]
    fn an_injected_payload_is_still_no_message() {
        // A command echo or a skill body: a tag first, nothing typed.
        assert_eq!(typed_text("<command-name>/model</command-name>"), None);
        assert_eq!(typed_text("  <local-command-stdout>ok</local-command-stdout>"), None);
    }

    #[test]
    fn the_page_shows_a_message_without_the_note_appended_to_it() {
        let line = serde_json::json!({
            "type": "user", "timestamp": "t",
            "message": { "role": "user", "content": format!("era o relatório da 1h?\n\n{HINT}") }
        })
        .to_string();
        assert_eq!(parse_entry(&line).unwrap().text, "era o relatório da 1h?");
    }

    #[test]
    fn a_note_in_a_block_of_its_own_is_skipped_and_the_typed_block_read() {
        let line = serde_json::json!({
            "type": "user", "timestamp": "t",
            "message": { "role": "user", "content": [
                { "type": "text", "text": HINT },
                { "type": "text", "text": format!("ótimo, assim não fica poluído\n\n{HINT}") }
            ] }
        })
        .to_string();
        assert_eq!(parse_entry(&line).unwrap().text, "ótimo, assim não fica poluído");
    }

    #[test]
    fn keeps_only_the_last_n_entries() {
        let lines: Vec<String> = (0..10)
            .map(|i| {
                format!(
                    r#"{{"type":"user","timestamp":"2026-08-15T10:0{i}:00.000Z","message":{{"role":"user","content":"p{i}"}}}}"#
                )
            })
            .collect();
        let tail = tail_entries(lines.iter().map(String::as_str), 3);
        assert_eq!(tail.len(), 3);
        assert_eq!(tail[0].text, "p7");
        assert_eq!(tail[2].text, "p9");
    }

    /// The mirror follows a log from a byte offset. A chat held at a
    /// terminal can be a 237 MB log (a desktop session of 70k lines): read
    /// from zero, it parsed every line and froze the window. The read
    /// starts at most `max` bytes before the end, whatever the offset.
    #[test]
    fn the_mirror_never_reads_more_than_its_ceiling_in_one_go() {
        assert_eq!(mirror_start(0, 237_000_000, 8_000_000), 229_000_000);
        assert_eq!(mirror_start(100, 200, 8_000_000), 100, "small growth reads from where it left");
        assert_eq!(mirror_start(0, 8_000_000, 8_000_000), 0, "exactly the ceiling is fine");
        assert_eq!(mirror_start(500, 200, 8_000_000), 0, "truncated under us: start over");
    }
}

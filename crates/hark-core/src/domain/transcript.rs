//! Pure reader of a session log into something a human can read back.
//! Used by the read-only thread viewer: no LLM, no tokens, just the file.

use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Entry {
    pub ts: String,
    pub role: Role,
    /// Prose, or the tool input JSON for `ToolUse`.
    pub text: String,
    pub tool: Option<String>,
    pub is_error: bool,
}

/// Parse one session-log line, skipping everything that is not conversation.
pub fn parse_entry(line: &str) -> Option<Entry> {
    let v: Value = serde_json::from_str(line).ok()?;
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
            Value::String(text) if !text.trim_start().starts_with('<') => {
                entry(Role::User, text.clone(), None, false)
            }
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
                        let text = b.get("text")?.as_str()?;
                        (!text.trim_start().starts_with('<'))
                            .then(|| entry(Role::User, text.to_string(), None, false))?
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
}

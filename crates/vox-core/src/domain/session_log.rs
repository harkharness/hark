//! Pure parser for Claude Code session log lines (`~/.claude/projects/**/*.jsonl`).
//!
//! Each line is an independent JSON object. We extract only what the index
//! needs and ignore everything else.

use serde_json::Value;

/// One fact extracted from a session log line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionEvent {
    UserPrompt {
        ts: String,
        text: String,
        cwd: Option<String>,
        git_branch: Option<String>,
    },
    /// Any assistant output; only the timestamp matters (session freshness).
    Activity { ts: String },
    /// User-assigned session title.
    Title(String),
}

/// Parse a single jsonl line into an event, if it carries anything we index.
pub fn parse_line(line: &str) -> Option<SessionEvent> {
    let v: Value = serde_json::from_str(line).ok()?;
    let str_of = |key: &str| v.get(key).and_then(Value::as_str).map(String::from);
    match v.get("type")?.as_str()? {
        "user" => {
            if v.get("isSidechain").and_then(Value::as_bool) == Some(true) {
                return None;
            }
            let text = prompt_text(v.get("message")?.get("content")?)?;
            // Injected skill/command payloads start with a tag; not human input.
            if text.trim_start().starts_with('<') {
                return None;
            }
            Some(SessionEvent::UserPrompt {
                ts: str_of("timestamp")?,
                text,
                cwd: str_of("cwd"),
                git_branch: str_of("gitBranch"),
            })
        }
        "assistant" => Some(SessionEvent::Activity {
            ts: str_of("timestamp")?,
        }),
        "custom-title" => Some(SessionEvent::Title(str_of("customTitle")?)),
        _ => None,
    }
}

/// Extract the human-written text from a message content value, which is
/// either a plain string or an array of typed blocks.
fn prompt_text(content: &Value) -> Option<String> {
    match content {
        Value::String(s) => Some(s.clone()),
        Value::Array(blocks) => {
            let text: Vec<&str> = blocks
                .iter()
                .filter(|b| b.get("type").and_then(Value::as_str) == Some("text"))
                .filter_map(|b| b.get("text").and_then(Value::as_str))
                .collect();
            (!text.is_empty()).then(|| text.join("\n"))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_user_prompt_line() {
        let line = r#"{"type":"user","timestamp":"2026-08-14T10:00:00.000Z","cwd":"/home/dev/proj","gitBranch":"main","isSidechain":false,"message":{"role":"user","content":"what is pending today?"}}"#;
        let event = parse_line(line);
        assert_eq!(
            event,
            Some(SessionEvent::UserPrompt {
                ts: "2026-08-14T10:00:00.000Z".into(),
                text: "what is pending today?".into(),
                cwd: Some("/home/dev/proj".into()),
                git_branch: Some("main".into()),
            })
        );
    }

    #[test]
    fn parses_user_prompt_with_content_blocks() {
        let line = r#"{"type":"user","timestamp":"2026-08-14T10:01:00.000Z","message":{"role":"user","content":[{"type":"text","text":"fix the login bug"}]}}"#;
        let event = parse_line(line);
        assert_eq!(
            event,
            Some(SessionEvent::UserPrompt {
                ts: "2026-08-14T10:01:00.000Z".into(),
                text: "fix the login bug".into(),
                cwd: None,
                git_branch: None,
            })
        );
    }

    #[test]
    fn ignores_sidechain_user_lines() {
        let line = r#"{"type":"user","timestamp":"2026-08-14T10:02:00.000Z","isSidechain":true,"message":{"role":"user","content":"subagent internal prompt"}}"#;
        assert_eq!(parse_line(line), None);
    }

    #[test]
    fn ignores_injected_system_text() {
        // Skill/command injections and tool results are not human prompts.
        let tagged = r#"{"type":"user","timestamp":"2026-08-14T10:03:00.000Z","message":{"role":"user","content":"<command-name>/model</command-name>"}}"#;
        let tool_result = r#"{"type":"user","timestamp":"2026-08-14T10:04:00.000Z","message":{"role":"user","content":[{"type":"tool_result","content":"ok"}]}}"#;
        assert_eq!(parse_line(tagged), None);
        assert_eq!(parse_line(tool_result), None);
    }

    #[test]
    fn parses_custom_title() {
        let line = r#"{"type":"custom-title","customTitle":"Webhook migration","sessionId":"abc"}"#;
        assert_eq!(
            parse_line(line),
            Some(SessionEvent::Title("Webhook migration".into()))
        );
    }

    #[test]
    fn parses_assistant_activity_timestamp() {
        let line = r#"{"type":"assistant","timestamp":"2026-08-14T11:00:00.000Z","message":{"role":"assistant","content":[]}}"#;
        assert_eq!(
            parse_line(line),
            Some(SessionEvent::Activity {
                ts: "2026-08-14T11:00:00.000Z".into()
            })
        );
    }

    #[test]
    fn ignores_garbage_and_other_types() {
        assert_eq!(parse_line("not json at all"), None);
        assert_eq!(parse_line(r#"{"type":"queue-operation"}"#), None);
        assert_eq!(parse_line(r#"{"no_type":true}"#), None);
    }
}

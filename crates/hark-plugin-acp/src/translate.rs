//! ACP wire → `AgentEvent`. Pure: one JSON message in, one event out.
//!
//! Provenance matters here, so it is marked per case. `auth_error` was
//! recorded from gemini-cli 0.46.0 (see `spikes/acp/FINDINGS.md`); the
//! `session/update` shapes follow the ACP v1 method names and are
//! re-checked against real traffic the moment an authenticated agent is
//! available — the tests say which is which.

use hark_agent::{AgentEvent, AgentPhase};

/// Prefix the UI already knows how to render as "this agent needs a
/// login" (the claude plugin emits the same shape).
pub const AUTH_PREFIX: &str = "agent_auth: ";

/// Does this JSON-RPC error mean "not logged in"?
///
/// OBSERVED: `session/new` against an unauthenticated gemini answers
/// `-32000 "Gemini API key is missing or not configured."`. There is no
/// auth_required notification to wait for — the failure IS the answer,
/// and it arrives on the first thing Hark asks for.
pub fn auth_error(err: &serde_json::Value) -> Option<String> {
    let message = err.get("message").and_then(|m| m.as_str())?;
    let low = message.to_lowercase();
    let smells_of_auth = ["api key", "not configured", "unauthenticated", "not authenticated",
        "login", "log in", "credential", "auth"]
        .iter()
        .any(|needle| low.contains(needle));
    smells_of_auth.then(|| message.to_string())
}

/// One `session/update` notification's params, or a request, as an event.
///
/// Anything unrecognised becomes `Ignored` rather than an error: an
/// agent that grows a new update kind must not break a running turn.
pub fn update(params: &serde_json::Value) -> AgentEvent {
    let update = params.get("update").unwrap_or(params);
    let kind = update.get("sessionUpdate").and_then(|k| k.as_str()).unwrap_or_default();
    match kind {
        "agent_message_chunk" => match text_of(update) {
            Some(text) if !text.is_empty() => AgentEvent::AssistantText(text),
            _ => AgentEvent::Ignored,
        },
        // Reasoning: the phase, not the words. Hark narrates "thinking",
        // it does not paste a model's scratchpad into the transcript.
        "agent_thought_chunk" => AgentEvent::Status(AgentPhase::Thinking),
        "tool_call" => AgentEvent::ToolUse {
            name: update
                .get("title")
                .or_else(|| update.get("kind"))
                .and_then(|t| t.as_str())
                .unwrap_or("tool")
                .to_string(),
            input: update.get("rawInput").map(|i| i.to_string()).unwrap_or_default(),
        },
        "tool_call_update" => {
            let status = update.get("status").and_then(|s| s.as_str()).unwrap_or("");
            // In flight: nothing to show yet, and emitting an empty
            // result would close the tool unit in the UI too early.
            if status != "completed" && status != "failed" {
                return AgentEvent::Ignored;
            }
            AgentEvent::ToolResult {
                content: update
                    .get("content")
                    .and_then(|c| c.as_array())
                    .map(|items| {
                        items.iter().filter_map(text_of).collect::<Vec<_>>().join("\n")
                    })
                    .unwrap_or_default(),
                is_error: status == "failed",
            }
        }
        _ => AgentEvent::Ignored,
    }
}

/// ACP content blocks nest text one level down; a plain string is
/// accepted too, because that is what half the shapes look like.
fn text_of(node: &serde_json::Value) -> Option<String> {
    if let Some(s) = node.as_str() {
        return Some(s.to_string());
    }
    if let Some(s) = node.get("text").and_then(|t| t.as_str()) {
        return Some(s.to_string());
    }
    node.get("content").and_then(text_of)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// OBSERVED — the exact error from gemini-cli 0.46.0.
    #[test]
    fn the_real_gemini_auth_failure_is_recognised() {
        let err = serde_json::json!({
            "code": -32000,
            "message": "Gemini API key is missing or not configured."
        });
        assert_eq!(
            auth_error(&err).as_deref(),
            Some("Gemini API key is missing or not configured.")
        );
    }

    /// OBSERVED — codex-acp 1.11.0 with no ChatGPT login, at session/new.
    /// A pin: the words are the adapter's; the card's runnable fix is the
    /// registry's `codex login`.
    #[test]
    fn the_real_codex_auth_failure_is_recognised() {
        let err = serde_json::json!({ "code": -32000, "message": "Authentication required" });
        assert_eq!(auth_error(&err).as_deref(), Some("Authentication required"));
    }

    /// OBSERVED — gemini-cli 0.46.0 on an individual account, at
    /// session/new. Same error code as an auth failure, but a deprecation:
    /// no login fixes it, an update does. It must NOT become a login card.
    #[test]
    fn the_real_gemini_deprecation_is_not_an_auth_problem() {
        let err = serde_json::json!({
            "code": -32000,
            "message": "This client is no longer supported for Gemini Code Assist for individuals. \
                        To continue using Gemini, please migrate to the Antigravity suite of products: https://antigravity.google"
        });
        assert_eq!(auth_error(&err), None);
    }

    #[test]
    fn an_unrelated_failure_is_not_an_auth_problem() {
        // Otherwise every crash would ask the user to log in again.
        let err = serde_json::json!({ "code": -32603, "message": "internal error" });
        assert_eq!(auth_error(&err), None);
    }

    /// SPEC — re-check against real traffic once an agent is logged in.
    #[test]
    fn a_message_chunk_becomes_prose() {
        let params = serde_json::json!({
            "update": { "sessionUpdate": "agent_message_chunk",
                        "content": { "type": "text", "text": "olá" } }
        });
        assert_eq!(update(&params), AgentEvent::AssistantText("olá".into()));
    }

    #[test]
    fn a_thought_is_a_phase_not_a_paste() {
        let params = serde_json::json!({
            "update": { "sessionUpdate": "agent_thought_chunk",
                        "content": { "type": "text", "text": "hmm" } }
        });
        assert_eq!(update(&params), AgentEvent::Status(AgentPhase::Thinking));
    }

    #[test]
    fn a_tool_call_carries_its_name_and_raw_input() {
        let params = serde_json::json!({
            "update": { "sessionUpdate": "tool_call", "toolCallId": "c1",
                        "title": "Read", "kind": "read",
                        "rawInput": { "path": "/tmp/x" }, "status": "pending" }
        });
        match update(&params) {
            AgentEvent::ToolUse { name, input } => {
                assert_eq!(name, "Read");
                assert!(input.contains("/tmp/x"));
            }
            other => panic!("expected ToolUse, got {other:?}"),
        }
    }

    #[test]
    fn a_tool_still_running_emits_nothing() {
        let params = serde_json::json!({
            "update": { "sessionUpdate": "tool_call_update", "toolCallId": "c1",
                        "status": "in_progress" }
        });
        // Closing the unit here would show a result that does not exist.
        assert_eq!(update(&params), AgentEvent::Ignored);
    }

    #[test]
    fn a_finished_tool_becomes_its_result() {
        let params = serde_json::json!({
            "update": { "sessionUpdate": "tool_call_update", "toolCallId": "c1",
                        "status": "completed",
                        "content": [{ "type": "content",
                                      "content": { "type": "text", "text": "12 linhas" } }] }
        });
        assert_eq!(
            update(&params),
            AgentEvent::ToolResult { content: "12 linhas".into(), is_error: false }
        );
    }

    #[test]
    fn a_failed_tool_is_marked_as_one() {
        let params = serde_json::json!({
            "update": { "sessionUpdate": "tool_call_update", "status": "failed",
                        "content": [{ "type": "text", "text": "boom" }] }
        });
        match update(&params) {
            AgentEvent::ToolResult { is_error, content } => {
                assert!(is_error);
                assert_eq!(content, "boom");
            }
            other => panic!("expected ToolResult, got {other:?}"),
        }
    }

    #[test]
    fn an_update_kind_hark_never_heard_of_is_ignored_not_fatal() {
        let params = serde_json::json!({ "update": { "sessionUpdate": "some_new_thing" } });
        assert_eq!(update(&params), AgentEvent::Ignored);
        assert_eq!(update(&serde_json::json!({})), AgentEvent::Ignored);
    }
}

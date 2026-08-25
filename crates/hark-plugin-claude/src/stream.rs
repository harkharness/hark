//! Pure parser for the Claude CLI stream-json output (one JSON per line),
//! translating it into the neutral `hark_agent::AgentEvent` vocabulary.
//! Shapes verified empirically against CLI v2.1.220 (see spikes/FINDINGS.md).

use serde_json::Value;

pub use hark_agent::{
    AgentEvent, AgentPhase, ModelUsage, PermissionDecision, RateLimitInfo, TokenUsage, TurnResult,
};

/// Historical alias — this crate's parser used to own the event enum.
pub type ClaudeEvent = AgentEvent;

/// Parse one stdout line from the CLI.
pub fn parse(line: &str) -> AgentEvent {
    let Ok(v) = serde_json::from_str::<Value>(line) else {
        return ClaudeEvent::Ignored;
    };
    match v.get("type").and_then(Value::as_str) {
        Some("assistant") => parse_tool_use(&v)
            .or_else(|| parse_assistant_text(&v))
            .unwrap_or(ClaudeEvent::Ignored),
        Some("user") => parse_tool_result(&v).unwrap_or(ClaudeEvent::Ignored),
        Some("result") => parse_result(&v).map(ClaudeEvent::Result).unwrap_or(ClaudeEvent::Ignored),
        Some("control_request") => parse_permission(&v).unwrap_or(ClaudeEvent::Ignored),
        Some("system") => match v.get("subtype").and_then(Value::as_str) {
            Some("init") => parse_init(&v).unwrap_or(ClaudeEvent::Ignored),
            Some("status") => parse_status(&v).unwrap_or(ClaudeEvent::Ignored),
            _ => ClaudeEvent::Ignored,
        },
        Some("stream_event") => parse_stream_phase(&v).unwrap_or(ClaudeEvent::Ignored),
        Some("rate_limit_event") => parse_rate_limit(&v).unwrap_or(ClaudeEvent::Ignored),
        _ => ClaudeEvent::Ignored,
    }
}

fn parse_init(v: &Value) -> Option<ClaudeEvent> {
    Some(ClaudeEvent::SessionStarted {
        session_id: v.get("session_id")?.as_str()?.to_string(),
        slash_commands: v
            .get("slash_commands")
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect())
            .unwrap_or_default(),
    })
}

/// The CLI's own status line. Only the states Hark can state as fact are
/// mapped; a new one the CLI invents stays Ignored rather than guessed.
fn parse_status(v: &Value) -> Option<ClaudeEvent> {
    match v.get("status")?.as_str()? {
        "requesting" | "stream_request_start" => {
            Some(ClaudeEvent::Status(AgentPhase::Requesting))
        }
        _ => None,
    }
}

/// Partial-message deltas, which is where a long turn spends its life:
/// reasoning tokens arrive as `thinking_delta` for as long as the model
/// thinks, and prose as `text_delta`. Content is deliberately dropped —
/// this is a heartbeat, and the full blocks arrive on their own lines.
fn parse_stream_phase(v: &Value) -> Option<ClaudeEvent> {
    let event = v.get("event")?;
    if event.get("type")?.as_str()? != "content_block_delta" {
        return None;
    }
    match event.get("delta")?.get("type")?.as_str()? {
        "thinking_delta" => Some(ClaudeEvent::Status(AgentPhase::Thinking)),
        "text_delta" => Some(ClaudeEvent::Status(AgentPhase::Writing)),
        _ => None,
    }
}

fn parse_assistant_text(v: &Value) -> Option<ClaudeEvent> {
    let text: Vec<&str> = v
        .get("message")?
        .get("content")?
        .as_array()?
        .iter()
        .filter(|b| b.get("type").and_then(Value::as_str) == Some("text"))
        .filter_map(|b| b.get("text").and_then(Value::as_str))
        .collect();
    let joined = text.join("\n");
    (!joined.trim().is_empty()).then_some(ClaudeEvent::AssistantText(joined))
}

fn parse_tool_result(v: &Value) -> Option<ClaudeEvent> {
    let block = v
        .get("message")?
        .get("content")?
        .as_array()?
        .iter()
        .find(|b| b.get("type").and_then(Value::as_str) == Some("tool_result"))?;
    let content = match block.get("content") {
        Some(Value::String(s)) => s.clone(),
        Some(other) => other.to_string(),
        None => String::new(),
    };
    Some(ClaudeEvent::ToolResult {
        content,
        is_error: block.get("is_error").and_then(Value::as_bool).unwrap_or(false),
    })
}

/// Build a stream-json user message line. Pasted screenshots become
/// ordered image blocks (media type + base64) before the text, so
/// "[image N]" in the prose refers to the N-th one.
pub fn user_message(text: &str, images: &[(String, String)]) -> String {
    let mut content = Vec::new();
    for (media_type, data) in images {
        content.push(serde_json::json!({
            "type": "image",
            "source": { "type": "base64", "media_type": media_type, "data": data }
        }));
    }
    content.push(serde_json::json!({ "type": "text", "text": text }));
    serde_json::json!({
        "type": "user",
        "message": { "role": "user", "content": content }
    })
    .to_string()
}

/// Serialize the user's decision into the control protocol response line.
pub fn permission_response(request_id: &str, decision: PermissionDecision) -> String {
    let inner = match decision {
        PermissionDecision::Allow => serde_json::json!({ "behavior": "allow" }),
        PermissionDecision::Deny => serde_json::json!({
            "behavior": "deny",
            "message": "User rejected this action from Hark."
        }),
    };
    serde_json::json!({
        "type": "control_response",
        "response": {
            "subtype": "success",
            "request_id": request_id,
            "response": inner,
        }
    })
    .to_string()
}

fn parse_tool_use(v: &Value) -> Option<ClaudeEvent> {
    let block = v
        .get("message")?
        .get("content")?
        .as_array()?
        .iter()
        .find(|b| b.get("type").and_then(Value::as_str) == Some("tool_use"))?;
    Some(ClaudeEvent::ToolUse {
        name: block.get("name")?.as_str()?.to_string(),
        input: block.get("input")?.to_string(),
    })
}

fn parse_result(v: &Value) -> Option<TurnResult> {
    let raw = v.get("result").and_then(Value::as_str).unwrap_or_default().to_string();
    let is_error = v.get("is_error").and_then(Value::as_bool).unwrap_or(false)
        || v.get("subtype").and_then(Value::as_str) != Some("success");
    Some(TurnResult {
        is_error,
        // Structured replies stay raw JSON here; the product layer decides
        // the shape (Hark parses them as VoiceReply in hark-core).
        reply: (!is_error)
            .then(|| serde_json::from_str::<Value>(&raw).ok())
            .flatten(),
        raw,
        cost_usd: v.get("total_cost_usd").and_then(Value::as_f64),
        duration_ms: v.get("duration_ms").and_then(Value::as_u64),
        model: main_model(v),
        usage: parse_usage(v),
    })
}

/// Per-model tokens from `modelUsage` (camelCase); falls back to one
/// "unknown" entry from the aggregate `usage` (snake_case) on old CLIs.
fn parse_usage(v: &Value) -> Vec<ModelUsage> {
    let num = |m: &Value, key: &str| m.get(key).and_then(Value::as_u64).unwrap_or(0);
    if let Some(models) = v.get("modelUsage").and_then(Value::as_object) {
        return models
            .iter()
            .map(|(model, m)| ModelUsage {
                model: model.clone(),
                usage: TokenUsage {
                    input: num(m, "inputTokens"),
                    output: num(m, "outputTokens"),
                    cache_read: num(m, "cacheReadInputTokens"),
                    cache_created: num(m, "cacheCreationInputTokens"),
                },
                cost_usd: m.get("costUSD").and_then(Value::as_f64),
                context_window: m.get("contextWindow").and_then(Value::as_u64).filter(|w| *w > 0),
            })
            .collect();
    }
    let Some(aggregate) = v.get("usage") else {
        return Vec::new();
    };
    vec![ModelUsage {
        model: "unknown".into(),
        usage: TokenUsage {
            input: num(aggregate, "input_tokens"),
            output: num(aggregate, "output_tokens"),
            cache_read: num(aggregate, "cache_read_input_tokens"),
            cache_created: num(aggregate, "cache_creation_input_tokens"),
        },
        cost_usd: v.get("total_cost_usd").and_then(Value::as_f64),
        context_window: None,
    }]
}

fn parse_rate_limit(v: &Value) -> Option<ClaudeEvent> {
    let info = v.get("rate_limit_info")?;
    Some(ClaudeEvent::RateLimit(RateLimitInfo {
        status: info.get("status")?.as_str()?.to_string(),
        resets_at: info.get("resetsAt").and_then(Value::as_u64),
        kind: info
            .get("rateLimitType")
            .and_then(Value::as_str)
            .map(String::from),
        overage: info
            .get("isUsingOverage")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    }))
}

/// The model that did the real work: highest-cost entry in modelUsage
/// (sidecar models like the haiku classifier stay out of the label).
fn main_model(v: &Value) -> Option<String> {
    let usage = v.get("modelUsage")?.as_object()?;
    usage
        .iter()
        .max_by(|a, b| {
            let cost = |m: &Value| m.get("costUSD").and_then(Value::as_f64).unwrap_or(0.0);
            cost(a.1).total_cmp(&cost(b.1))
        })
        .map(|(name, _)| name.clone())
}

fn parse_permission(v: &Value) -> Option<ClaudeEvent> {
    let request = v.get("request")?;
    if request.get("subtype").and_then(Value::as_str) != Some("can_use_tool") {
        return None;
    }
    Some(ClaudeEvent::PermissionRequest {
        request_id: v.get("request_id")?.as_str()?.to_string(),
        tool_name: request.get("tool_name")?.as_str()?.to_string(),
        input: request.get("input")?.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_tool_use_for_narration() {
        let line = r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","name":"Bash","input":{"command":"git status"}}]}}"#;
        assert_eq!(
            parse(line),
            ClaudeEvent::ToolUse {
                name: "Bash".into(),
                input: r#"{"command":"git status"}"#.into(),
            }
        );
    }

    #[test]
    fn parses_successful_result_into_voice_reply() {
        let line = r#"{"type":"result","subtype":"success","is_error":false,"result":"{\"fala\":\"Duas pendências hoje.\",\"detalhes\":\"PR aberto e teste falhando\",\"itens\":[\"revisar PR\"]}","total_cost_usd":0.009,"duration_ms":4700,"modelUsage":{"claude-haiku-4-5":{"costUSD":0.004},"claude-sonnet-5":{"costUSD":0.035}}}"#;
        let ClaudeEvent::Result(result) = parse(line) else {
            panic!("expected result event");
        };
        assert_eq!(result.cost_usd, Some(0.009));
        // Highest-cost model wins the label, sidecars ignored.
        assert_eq!(result.model.as_deref(), Some("claude-sonnet-5"));
        let reply = result.reply.expect("structured reply");
        assert_eq!(reply["fala"], "Duas pendências hoje.");
        assert_eq!(reply["detalhes"], "PR aberto e teste falhando");
        assert_eq!(reply["itens"][0], "revisar PR");
    }

    #[test]
    fn parses_error_result_without_reply() {
        let line = r#"{"type":"result","subtype":"error_during_execution","is_error":true,"result":"something broke","total_cost_usd":0.001}"#;
        let ClaudeEvent::Result(result) = parse(line) else {
            panic!("expected result event");
        };
        assert!(result.is_error);
        assert_eq!(result.reply, None);
        assert_eq!(result.raw, "something broke");
    }

    #[test]
    fn parses_assistant_text_for_live_transcript() {
        let line = r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"Vou começar pelo parser."}]}}"#;
        assert_eq!(
            parse(line),
            ClaudeEvent::AssistantText("Vou começar pelo parser.".into())
        );
    }

    #[test]
    fn parses_tool_results_for_live_transcript() {
        let line = r#"{"type":"user","message":{"role":"user","content":[{"type":"tool_result","content":"3 files changed","is_error":false,"tool_use_id":"t1"}]}}"#;
        assert_eq!(
            parse(line),
            ClaudeEvent::ToolResult {
                content: "3 files changed".into(),
                is_error: false,
            }
        );
    }

    #[test]
    fn parses_permission_request() {
        let line = r#"{"type":"control_request","request_id":"abc-123","request":{"subtype":"can_use_tool","tool_name":"Bash","input":{"command":"rm -rf /tmp/x"}}}"#;
        assert_eq!(
            parse(line),
            ClaudeEvent::PermissionRequest {
                request_id: "abc-123".into(),
                tool_name: "Bash".into(),
                input: r#"{"command":"rm -rf /tmp/x"}"#.into(),
            }
        );
    }

    #[test]
    fn builds_user_message_with_any_number_of_images() {
        let plain: serde_json::Value =
            serde_json::from_str(&user_message("oi", &[])).unwrap();
        assert_eq!(plain["type"], "user");
        assert_eq!(plain["message"]["content"][0]["type"], "text");
        assert_eq!(plain["message"]["content"][0]["text"], "oi");

        // Several pasted screenshots become ordered blocks before the
        // text, so "[image 2]" in the prose points at the second one.
        let imgs = vec![
            ("image/png".to_string(), "aGVsbG8=".to_string()),
            ("image/jpeg".to_string(), "d29ybGQ=".to_string()),
        ];
        let msg: serde_json::Value =
            serde_json::from_str(&user_message("compara [image 1] com [image 2]", &imgs))
                .unwrap();
        assert_eq!(msg["message"]["content"][0]["type"], "image");
        assert_eq!(msg["message"]["content"][0]["source"]["media_type"], "image/png");
        assert_eq!(msg["message"]["content"][1]["type"], "image");
        assert_eq!(msg["message"]["content"][1]["source"]["data"], "d29ybGQ=");
        assert_eq!(msg["message"]["content"][2]["type"], "text");
    }

    /// Lines captured from CLI v2.1.220 with --include-partial-messages.
    /// Before this, a turn that thought for a minute emitted NOTHING the
    /// window could see: the first `assistant` line only lands once the
    /// whole block is finished, so the chat sat dead while the model
    /// worked. The CLI says what it is doing the entire time.
    #[test]
    fn the_cli_announces_what_it_is_doing() {
        assert_eq!(
            parse(r#"{"type":"system","subtype":"status","status":"requesting","uuid":"57fe","session_id":"ae17"}"#),
            AgentEvent::Status(AgentPhase::Requesting)
        );
        assert_eq!(
            parse(
                r#"{"type":"stream_event","event":{"type":"content_block_delta","index":0,
                "delta":{"type":"thinking_delta","thinking":"","estimated_tokens":null}},
                "session_id":"ae17"}"#
            ),
            AgentEvent::Status(AgentPhase::Thinking)
        );
        assert_eq!(
            parse(
                r#"{"type":"stream_event","event":{"type":"content_block_delta","index":1,
                "delta":{"type":"text_delta","text":"391"}},"session_id":"ae17"}"#
            ),
            AgentEvent::Status(AgentPhase::Writing)
        );
    }

    /// Nothing is invented from a partial stream: only the two deltas that
    /// name a phase count, and an unknown status stays unknown.
    #[test]
    fn partial_stream_noise_stays_ignored() {
        for line in [
            r#"{"type":"stream_event","event":{"type":"message_start","message":{"usage":{}}}}"#,
            r#"{"type":"stream_event","event":{"type":"content_block_stop","index":0}}"#,
            r#"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"signature_delta","signature":"x"}}}"#,
            r#"{"type":"system","subtype":"post_turn_summary","status_category":"review_ready"}"#,
            r#"{"type":"system","subtype":"status","status":"something_new"}"#,
        ] {
            assert_eq!(parse(line), AgentEvent::Ignored, "line: {line}");
        }
    }

    #[test]
    fn other_lines_are_ignored_but_tagged() {
        assert_eq!(parse(r#"{"type":"system","subtype":"init"}"#), ClaudeEvent::Ignored);
        assert_eq!(parse("garbage"), ClaudeEvent::Ignored);
    }

    #[test]
    fn result_extracts_per_model_token_usage() {
        let line = r#"{"type":"result","subtype":"success","is_error":false,"result":"ok",
            "total_cost_usd":0.0491,"duration_ms":8480,
            "usage":{"input_tokens":4,"output_tokens":268,"cache_creation_input_tokens":4660,"cache_read_input_tokens":54366},
            "modelUsage":{
              "claude-haiku-4-5":{"inputTokens":774,"outputTokens":14,"cacheReadInputTokens":0,"cacheCreationInputTokens":0,"costUSD":0.000844,"contextWindow":200000},
              "claude-sonnet-5":{"inputTokens":4,"outputTokens":268,"cacheReadInputTokens":54366,"cacheCreationInputTokens":4660,"costUSD":0.0483,"contextWindow":1000000}
            }}"#;
        let ClaudeEvent::Result(result) = parse(line) else {
            panic!("expected result");
        };
        assert_eq!(result.usage.len(), 2);
        let sonnet = result.usage.iter().find(|m| m.model == "claude-sonnet-5").unwrap();
        assert_eq!(sonnet.usage.input, 4);
        assert_eq!(sonnet.usage.output, 268);
        assert_eq!(sonnet.usage.cache_read, 54366);
        assert_eq!(sonnet.usage.cache_created, 4660);
        assert_eq!(sonnet.context_window, Some(1_000_000));
        let haiku = result.usage.iter().find(|m| m.model == "claude-haiku-4-5").unwrap();
        assert_eq!(haiku.usage.input, 774);
        let total: f64 = result.usage.iter().filter_map(|m| m.cost_usd).sum();
        assert!((total - 0.049144).abs() < 1e-6, "per-model costs sum to ~total");
    }

    #[test]
    fn result_without_model_usage_falls_back_to_aggregate() {
        let line = r#"{"type":"result","subtype":"success","is_error":false,"result":"ok",
            "total_cost_usd":0.01,
            "usage":{"input_tokens":10,"output_tokens":20,"cache_creation_input_tokens":30,"cache_read_input_tokens":40}}"#;
        let ClaudeEvent::Result(result) = parse(line) else {
            panic!("expected result");
        };
        assert_eq!(result.usage.len(), 1);
        assert_eq!(result.usage[0].model, "unknown");
        assert_eq!(result.usage[0].usage.input, 10);
        assert_eq!(result.usage[0].usage.output, 20);
        assert_eq!(result.usage[0].usage.cache_created, 30);
        assert_eq!(result.usage[0].usage.cache_read, 40);
        assert_eq!(result.usage[0].cost_usd, Some(0.01));
    }

    #[test]
    fn error_results_still_carry_usage() {
        let line = r#"{"type":"result","subtype":"error_during_execution","is_error":true,"result":"boom",
            "total_cost_usd":0.02,
            "modelUsage":{"claude-sonnet-5":{"inputTokens":5,"outputTokens":6,"cacheReadInputTokens":0,"cacheCreationInputTokens":0,"costUSD":0.02,"contextWindow":1000000}}}"#;
        let ClaudeEvent::Result(result) = parse(line) else {
            panic!("expected result");
        };
        assert!(result.is_error);
        assert_eq!(result.usage.len(), 1);
        assert_eq!(result.usage[0].usage.output, 6);
    }

    #[test]
    fn parses_rate_limit_events() {
        let full = r#"{"type":"rate_limit_event","rate_limit_info":{"status":"allowed","resetsAt":1786728000,"rateLimitType":"five_hour","overageStatus":"rejected","isUsingOverage":false}}"#;
        assert_eq!(
            parse(full),
            ClaudeEvent::RateLimit(RateLimitInfo {
                status: "allowed".into(),
                resets_at: Some(1786728000),
                kind: Some("five_hour".into()),
                overage: false,
            })
        );
        let minimal = r#"{"type":"rate_limit_event","rate_limit_info":{"status":"allowed_warning"}}"#;
        assert_eq!(
            parse(minimal),
            ClaudeEvent::RateLimit(RateLimitInfo {
                status: "allowed_warning".into(),
                resets_at: None,
                kind: None,
                overage: false,
            })
        );
    }

    #[test]
    fn init_with_session_id_reports_the_session() {
        assert_eq!(
            parse(r#"{"type":"system","subtype":"init","session_id":"abc-123","cwd":"/p"}"#),
            ClaudeEvent::SessionStarted { session_id: "abc-123".into(), slash_commands: vec![] }
        );
        // Other system subtypes stay ignored.
        assert_eq!(
            parse(r#"{"type":"system","subtype":"hook","session_id":"abc"}"#),
            ClaudeEvent::Ignored
        );
    }

    #[test]
    fn init_carries_the_sessions_slash_commands() {
        // The CLI announces which slash commands this session accepts;
        // they feed the "/" palette without any directory scanning.
        assert_eq!(
            parse(
                r#"{"type":"system","subtype":"init","session_id":"abc","slash_commands":["compact","usage","design"]}"#
            ),
            ClaudeEvent::SessionStarted {
                session_id: "abc".into(),
                slash_commands: vec!["compact".into(), "usage".into(), "design".into()],
            }
        );
    }

    #[test]
    fn permission_responses_serialize_to_protocol_shape() {
        let parsed = |s: String| serde_json::from_str::<serde_json::Value>(&s).unwrap();

        let allow = parsed(permission_response("abc-123", PermissionDecision::Allow));
        assert_eq!(allow["type"], "control_response");
        assert_eq!(allow["response"]["subtype"], "success");
        assert_eq!(allow["response"]["request_id"], "abc-123");
        assert_eq!(allow["response"]["response"]["behavior"], "allow");

        let deny = parsed(permission_response("abc-123", PermissionDecision::Deny));
        assert_eq!(deny["response"]["response"]["behavior"], "deny");
        assert!(deny["response"]["response"]["message"].is_string());
    }
}

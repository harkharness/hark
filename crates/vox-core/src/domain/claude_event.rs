//! Pure parser for the Claude CLI stream-json output (one JSON per line).
//! Shapes verified empirically against CLI v2.1.220 (see spikes/FINDINGS.md).

use serde::Deserialize;
use serde_json::Value;

/// Claude's structured answer, constrained by `prompt::RESPONSE_SCHEMA`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct VoiceReply {
    pub fala: String,
    pub detalhes: String,
    #[serde(default)]
    pub itens: Vec<String>,
    /// Board updates proposed by the model (the invisible kanban feed).
    #[serde(default)]
    pub board: Vec<crate::domain::board::BoardUpdate>,
}

/// Token counters of one model in one turn. This is the raw material of
/// the spend ledger: without measuring, nothing can be saved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TokenUsage {
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_created: u64,
}

/// Per-model usage of a turn, straight from the CLI's `modelUsage`.
#[derive(Debug, Clone, PartialEq)]
pub struct ModelUsage {
    pub model: String,
    pub usage: TokenUsage,
    pub cost_usd: Option<f64>,
    /// The model's context window, reported for free by the CLI.
    pub context_window: Option<u64>,
}

/// Final outcome of one turn.
#[derive(Debug, Clone, PartialEq)]
pub struct TurnResult {
    pub is_error: bool,
    /// Parsed structured reply, when the run used the response schema.
    pub reply: Option<VoiceReply>,
    /// Raw result string as emitted by the CLI (error text or raw JSON).
    pub raw: String,
    pub cost_usd: Option<f64>,
    pub duration_ms: Option<u64>,
    /// Main model that produced the turn (highest-cost entry in modelUsage).
    pub model: Option<String>,
    /// Token usage per model (empty only when the CLI reported nothing).
    pub usage: Vec<ModelUsage>,
}

/// Subscription window signal emitted by the CLI on every turn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RateLimitInfo {
    /// allowed | allowed_warning | rejected
    pub status: String,
    /// Epoch seconds when the current window resets.
    pub resets_at: Option<u64>,
    /// five_hour | seven_day | seven_day_opus | ...
    pub kind: Option<String>,
    pub overage: bool,
}

/// One line of CLI output, reduced to what Vox reacts to.
#[derive(Debug, Clone, PartialEq)]
pub enum ClaudeEvent {
    /// Assistant called a tool; narrated in the UI/TTS while waiting.
    ToolUse { name: String, input: String },
    /// Assistant prose (live transcript between tool calls).
    AssistantText(String),
    /// A tool finished; shown in the live transcript.
    ToolResult { content: String, is_error: bool },
    /// Turn finished.
    Result(TurnResult),
    /// CLI asks whether a tool may run (`--permission-prompt-tool stdio`).
    PermissionRequest {
        request_id: String,
        tool_name: String,
        input: String,
    },
    /// The CLI announced which session this process writes to. Essential
    /// for brand-new sessions, whose id only exists after spawn.
    SessionStarted(String),
    /// Subscription window status (five_hour/seven_day), one per turn.
    RateLimit(RateLimitInfo),
    /// Anything else (thinking estimates, partial deltas we don't use yet).
    Ignored,
}

/// User's verdict on a permission request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionDecision {
    Allow,
    Deny,
}

/// Parse one stdout line from the CLI.
pub fn parse(line: &str) -> ClaudeEvent {
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
        Some("system") => v
            .get("subtype")
            .and_then(Value::as_str)
            .filter(|s| *s == "init")
            .and_then(|_| v.get("session_id").and_then(Value::as_str))
            .map(|s| ClaudeEvent::SessionStarted(s.to_string()))
            .unwrap_or(ClaudeEvent::Ignored),
        Some("rate_limit_event") => parse_rate_limit(&v).unwrap_or(ClaudeEvent::Ignored),
        _ => ClaudeEvent::Ignored,
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

/// Build a stream-json user message line, optionally carrying one image
/// block (media type + base64 data) before the text.
pub fn user_message(text: &str, image: Option<(&str, &str)>) -> String {
    let mut content = Vec::new();
    if let Some((media_type, data)) = image {
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
            "message": "User rejected this action from Vox."
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
        reply: (!is_error)
            .then(|| serde_json::from_str::<VoiceReply>(&raw).ok())
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
        assert_eq!(reply.fala, "Duas pendências hoje.");
        assert_eq!(reply.detalhes, "PR aberto e teste falhando");
        assert_eq!(reply.itens, vec!["revisar PR"]);
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
    fn builds_user_message_with_optional_image() {
        let plain: serde_json::Value =
            serde_json::from_str(&user_message("oi", None)).unwrap();
        assert_eq!(plain["type"], "user");
        assert_eq!(plain["message"]["content"][0]["type"], "text");
        assert_eq!(plain["message"]["content"][0]["text"], "oi");

        let img: serde_json::Value = serde_json::from_str(&user_message(
            "qual a cor?",
            Some(("image/png", "aGVsbG8=")),
        ))
        .unwrap();
        assert_eq!(img["message"]["content"][0]["type"], "image");
        assert_eq!(img["message"]["content"][0]["source"]["media_type"], "image/png");
        assert_eq!(img["message"]["content"][0]["source"]["data"], "aGVsbG8=");
        assert_eq!(img["message"]["content"][1]["type"], "text");
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
            ClaudeEvent::SessionStarted("abc-123".into())
        );
        // Other system subtypes stay ignored.
        assert_eq!(
            parse(r#"{"type":"system","subtype":"hook","session_id":"abc"}"#),
            ClaudeEvent::Ignored
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

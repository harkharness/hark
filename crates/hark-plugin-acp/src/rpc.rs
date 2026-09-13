//! JSON-RPC 2.0 framing for ACP: one JSON object per line on stdio.
//!
//! Pure — parse a line, build a line. The protocol's METHODS and their
//! shapes live in `session.rs`; this module only knows envelopes, so the
//! agent's side of a conversation can be replayed in tests with the same
//! functions Hark uses to speak.

use serde_json::Value;

/// One line from the other side, sorted by what it wants from us.
#[derive(Debug, Clone, PartialEq)]
pub enum Incoming {
    /// The other side answered one of OUR requests. Hark only ever sends
    /// numeric ids, so a response id is a number or it is noise.
    Response { id: u64, result: Option<Value>, error: Option<Value> },
    /// The other side asks us something and expects an answer. The id is
    /// kept verbatim — the agent picks its own shape and gets it back.
    Request { id: Value, method: String, params: Value },
    /// Fire-and-forget from the other side (`session/update`).
    Notification { method: String, params: Value },
}

/// Sort one line. Anything that is not a JSON-RPC message — an agent's
/// log line on stdout, a blank — is `None`, never an error: the stream
/// must survive whatever a CLI prints while warming up.
pub fn parse(line: &str) -> Option<Incoming> {
    let v: Value = serde_json::from_str(line.trim()).ok()?;
    let obj = v.as_object()?;
    let method = obj.get("method").and_then(Value::as_str);
    let id = obj.get("id").filter(|id| !id.is_null());
    let params = || obj.get("params").cloned().unwrap_or(Value::Null);
    match (method, id) {
        (Some(method), Some(id)) => {
            Some(Incoming::Request { id: id.clone(), method: method.to_string(), params: params() })
        }
        (Some(method), None) => Some(Incoming::Notification { method: method.to_string(), params: params() }),
        (None, Some(id)) => {
            // Our ids are numbers; an agent echoing one as a string is
            // still answering us.
            let id = id.as_u64().or_else(|| id.as_str().and_then(|s| s.parse().ok()))?;
            let result = obj.get("result").cloned();
            let error = obj.get("error").cloned();
            if result.is_none() && error.is_none() {
                return None;
            }
            Some(Incoming::Response { id, result, error })
        }
        (None, None) => None,
    }
}

/// A request WE send: `id` is ours to correlate the answer.
pub fn request(id: u64, method: &str, params: Value) -> String {
    serde_json::json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }).to_string()
}

/// A notification we send (`session/cancel`): no id, no answer expected.
pub fn notification(method: &str, params: Value) -> String {
    serde_json::json!({ "jsonrpc": "2.0", "method": method, "params": params }).to_string()
}

/// Our answer to the agent's request, echoing ITS id untouched.
pub fn response(id: &Value, result: Value) -> String {
    serde_json::json!({ "jsonrpc": "2.0", "id": id, "result": result }).to_string()
}

/// Our refusal of the agent's request (`-32601` for a method we do not
/// serve), echoing its id.
pub fn error_response(id: &Value, code: i64, message: &str) -> String {
    serde_json::json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_result_answers_our_request() {
        let got = parse(r#"{"jsonrpc":"2.0","id":1,"result":{"ok":true}}"#);
        assert_eq!(
            got,
            Some(Incoming::Response { id: 1, result: Some(json!({"ok": true})), error: None })
        );
    }

    #[test]
    fn an_error_answers_our_request_too() {
        let got = parse(r#"{"jsonrpc":"2.0","id":2,"error":{"code":-32000,"message":"nope"}}"#);
        match got {
            Some(Incoming::Response { id: 2, result: None, error: Some(err) }) => {
                assert_eq!(err["code"], -32000);
                assert_eq!(err["message"], "nope");
            }
            other => panic!("expected an error response, got {other:?}"),
        }
    }

    #[test]
    fn an_id_with_a_method_is_a_request_from_the_agent() {
        let got = parse(
            r#"{"jsonrpc":"2.0","id":7,"method":"session/request_permission","params":{"a":1}}"#,
        );
        assert_eq!(
            got,
            Some(Incoming::Request {
                id: json!(7),
                method: "session/request_permission".into(),
                params: json!({"a": 1}),
            })
        );
        // Agents may use string ids; they come back exactly as sent.
        let got = parse(r#"{"jsonrpc":"2.0","id":"req-a","method":"fs/read_text_file","params":{}}"#);
        assert!(matches!(got, Some(Incoming::Request { id, .. }) if id == json!("req-a")));
    }

    #[test]
    fn no_id_is_a_notification() {
        let got = parse(r#"{"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"s"}}"#);
        assert_eq!(
            got,
            Some(Incoming::Notification {
                method: "session/update".into(),
                params: json!({"sessionId": "s"}),
            })
        );
    }

    #[test]
    fn a_line_that_is_not_jsonrpc_is_nothing_not_an_error() {
        assert_eq!(parse("Loading extensions..."), None);
        assert_eq!(parse(""), None);
        assert_eq!(parse("{}"), None);
        // A request without params still has a method: params default empty.
        assert!(matches!(
            parse(r#"{"jsonrpc":"2.0","method":"ping"}"#),
            Some(Incoming::Notification { method, params }) if method == "ping" && params.is_null()
        ));
    }

    #[test]
    fn an_outgoing_request_is_one_line_carrying_the_envelope() {
        let line = request(3, "session/prompt", json!({"sessionId": "s-1"}));
        assert!(!line.contains('\n'), "one message per line: {line:?}");
        let v: Value = serde_json::from_str(&line).expect("valid json");
        assert_eq!(v["jsonrpc"], "2.0");
        assert_eq!(v["id"], 3);
        assert_eq!(v["method"], "session/prompt");
        assert_eq!(v["params"]["sessionId"], "s-1");
    }

    #[test]
    fn a_notification_has_no_id_at_all() {
        let line = notification("session/cancel", json!({"sessionId": "s-1"}));
        let v: Value = serde_json::from_str(&line).expect("valid json");
        assert!(v.get("id").is_none(), "an id would make it a request: {line}");
        assert_eq!(v["method"], "session/cancel");
    }

    #[test]
    fn a_response_echoes_the_agents_id_untouched() {
        let line = response(&json!("abc"), json!({"outcome": {"outcome": "cancelled"}}));
        let v: Value = serde_json::from_str(&line).expect("valid json");
        assert_eq!(v["id"], "abc");
        assert_eq!(v["result"]["outcome"]["outcome"], "cancelled");
        assert!(v.get("error").is_none());
    }

    #[test]
    fn an_error_response_carries_code_and_message() {
        let line = error_response(&json!(9), -32601, "method not found");
        let v: Value = serde_json::from_str(&line).expect("valid json");
        assert_eq!(v["id"], 9);
        assert_eq!(v["error"]["code"], -32601);
        assert_eq!(v["error"]["message"], "method not found");
        assert!(v.get("result").is_none());
    }
}

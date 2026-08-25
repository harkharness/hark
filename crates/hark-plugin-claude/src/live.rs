//! Live Claude Code sessions via `claude agents --json`.

use hark_core::domain::prompt::LiveSession;
use hark_core::ports::LiveSessions;
use serde_json::Value;

/// Pure parser for the `claude agents --json` payload.
pub fn parse_agents_json(json: &str) -> Vec<LiveSession> {
    serde_json::from_str::<Value>(json)
        .ok()
        .and_then(|v| v.as_array().cloned())
        .unwrap_or_default()
        .iter()
        .filter_map(|item| {
            Some(LiveSession {
                name: item.get("name")?.as_str()?.to_string(),
                cwd: item.get("cwd")?.as_str()?.to_string(),
                status: item
                    .get("status")
                    .and_then(Value::as_str)
                    .map(String::from),
                session_id: item
                    .get("sessionId")
                    .and_then(Value::as_str)
                    .map(String::from),
                // Ownership signals: which process holds the session, and
                // whether a human is sitting at it.
                pid: item.get("pid").and_then(Value::as_i64).map(|p| p as i32),
                kind: item.get("kind").and_then(Value::as_str).map(String::from),
                started_at: item.get("startedAt").and_then(Value::as_i64),
            })
        })
        .collect()
}

/// Shell adapter: spawns the `claude` binary.
pub struct ClaudeAgentsCli {
    pub claude_bin: String,
}

impl LiveSessions for ClaudeAgentsCli {
    fn list(&self) -> anyhow::Result<Vec<LiveSession>> {
        // A missing or failing CLI means "no live sessions", not a broken
        // app: this feeds a status panel, and the install problem is
        // reported by the paths that actually need the binary.
        let Ok(out) = std::process::Command::new(&self.claude_bin)
            .args(["agents", "--json"])
            .output()
        else {
            return Ok(Vec::new());
        };
        Ok(parse_agents_json(&String::from_utf8_lossy(&out.stdout)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_agents_payload() {
        // Real payload from v2.1.220 (`claude agents --json`), which is
        // also how Hark learns that a session is held at a terminal:
        // pid + kind decide, so both have to survive the parse.
        let json = r#"[
            {"pid":3702,"cwd":"/home/dev/proj","kind":"interactive","startedAt":1787664465560,"sessionId":"s1","name":"proj-96","status":"idle"},
            {"pid":2,"cwd":"/home/dev/other","kind":"interactive","sessionId":"s2","name":"other-b2"}
        ]"#;
        let live = parse_agents_json(json);
        assert_eq!(live.len(), 2);
        assert_eq!(live[0].name, "proj-96");
        assert_eq!(live[0].status.as_deref(), Some("idle"));
        assert_eq!(live[0].pid, Some(3702));
        assert_eq!(live[0].kind.as_deref(), Some("interactive"));
        assert_eq!(live[0].started_at, Some(1_787_664_465_560));
        assert_eq!(live[1].status, None);
        assert_eq!(live[1].started_at, None);
    }

    #[test]
    fn tolerates_garbage() {
        assert!(parse_agents_json("nope").is_empty());
        assert!(parse_agents_json("{}").is_empty());
    }
}

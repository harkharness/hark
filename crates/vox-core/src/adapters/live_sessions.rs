//! Live Claude Code sessions via `claude agents --json`.

use crate::domain::prompt::LiveSession;
use crate::ports::LiveSessions;
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
        let out = std::process::Command::new(&self.claude_bin)
            .args(["agents", "--json"])
            .output()?;
        Ok(parse_agents_json(&String::from_utf8_lossy(&out.stdout)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_agents_payload() {
        let json = r#"[
            {"pid":1,"cwd":"/home/dev/proj","kind":"interactive","sessionId":"s1","name":"proj-96","status":"idle"},
            {"pid":2,"cwd":"/home/dev/other","kind":"interactive","sessionId":"s2","name":"other-b2"}
        ]"#;
        let live = parse_agents_json(json);
        assert_eq!(live.len(), 2);
        assert_eq!(live[0].name, "proj-96");
        assert_eq!(live[0].status.as_deref(), Some("idle"));
        assert_eq!(live[1].status, None);
    }

    #[test]
    fn tolerates_garbage() {
        assert!(parse_agents_json("nope").is_empty());
        assert!(parse_agents_json("{}").is_empty());
    }
}

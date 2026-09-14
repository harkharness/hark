//! What an ACP agent says it can do, translated into Hark's own sheet.
//!
//! The input is the `initialize` result, verbatim from the wire. Every
//! field read here was OBSERVED (see `fixtures/`, and the spike notes in
//! `spikes/acp/FINDINGS.md`) rather than taken from the spec — an agent
//! that answers more than the minimum is the normal case.

use hark_agent::Capabilities;

/// Who is on the other end, for the log and for fixture provenance.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AgentInfo {
    pub name: String,
    pub title: String,
    pub version: String,
}

/// How the agent says one can log in. Ids come from the agent, so the
/// hint Hark shows is the agent's own vocabulary, not our guess.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthMethod {
    pub id: String,
    pub name: String,
}

/// The negotiated sheet: capabilities plus the two things that are not
/// capabilities but decide what the UI may offer.
#[derive(Debug, Clone, PartialEq)]
pub struct Negotiated {
    pub caps: Capabilities,
    pub info: AgentInfo,
    pub auth: Vec<AuthMethod>,
    /// Protocol version the agent answered with.
    pub protocol: u32,
    /// The agent takes images in a prompt (pasted screenshots).
    pub images: bool,
    /// The agent is the Claude adapter (claude-agent-acp), which signs its
    /// initialize with `agentCapabilities._meta.claudeCode`. Only it
    /// understands the `_meta` hark may put on session/new — a replacement
    /// system prompt, SDK options such as the budget ceiling.
    pub claude_code: bool,
}

fn s(v: &serde_json::Value, key: &str) -> String {
    v.get(key).and_then(|x| x.as_str()).unwrap_or_default().to_string()
}

/// Translate an `initialize` result.
///
/// The defaults are deliberately pessimistic: anything the agent does
/// not claim is treated as absent, so a surface degrades instead of
/// failing at the first turn.
pub fn negotiate(result: &serde_json::Value, memory_file: Option<String>) -> Negotiated {
    let agent_caps = result.get("agentCapabilities");
    let prompt = agent_caps.and_then(|c| c.get("promptCapabilities"));

    let caps = Capabilities {
        // The agent's own `session/load`.
        resume: agent_caps.and_then(|c| c.get("loadSession")).and_then(|v| v.as_bool()).unwrap_or(false),
        // ACP has session/request_permission; that is the whole point of
        // driving an agent from a client.
        permissions: true,
        // No --json-schema equivalent: the ask lane must extract JSON
        // from prose for these backends (F9.5).
        structured_output: false,
        // Flipped on only when a turn actually reports cost.
        cost_reporting: false,
        // No session files on disk that Hark could index: the recorder
        // (F9.4) is what gives these backends a history.
        history: false,
        // No equivalent of `claude agents --json`.
        live_list: false,
        // Announced per session, not at initialize.
        slash_commands: false,
        memory_file,
        // ACP tool names are the agent's own; the production gate learns
        // them from the session's tool calls, not from a fixed list.
        shell_tools: Vec::new(),
        // Nothing in ACP forks a session into a new id.
        fork: false,
        // Directives cross only through what the agent OFFERS in its
        // session/new answer (modes, config options) — unknown until then,
        // so the sheet says no until `directives::Offer` says otherwise.
        directive_mode: false,
        directive_model: false,
        directive_effort: false,
    };

    Negotiated {
        caps,
        claude_code: agent_caps.and_then(|c| c.pointer("/_meta/claudeCode")).is_some_and(|v| v.is_object()),
        info: result
            .get("agentInfo")
            .map(|i| AgentInfo { name: s(i, "name"), title: s(i, "title"), version: s(i, "version") })
            .unwrap_or_default(),
        auth: result
            .get("authMethods")
            .and_then(|a| a.as_array())
            .map(|list| {
                list.iter().map(|m| AuthMethod { id: s(m, "id"), name: s(m, "name") }).collect()
            })
            .unwrap_or_default(),
        protocol: result.get("protocolVersion").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
        images: prompt.and_then(|p| p.get("image")).and_then(|v| v.as_bool()).unwrap_or(false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The real answer from gemini-cli 0.46.0 on this machine.
    const GEMINI: &str = include_str!("../fixtures/initialize.gemini-0.46.0.json");

    fn gemini() -> Negotiated {
        negotiate(&serde_json::from_str(GEMINI).expect("fixture parses"), Some("GEMINI.md".into()))
    }

    #[test]
    fn reads_the_agent_and_its_version_from_the_wire() {
        let n = gemini();
        assert_eq!(n.info.name, "gemini-cli");
        assert_eq!(n.info.version, "0.46.0");
        assert_eq!(n.protocol, 1);
    }

    #[test]
    fn load_session_becomes_resume() {
        // gemini announces loadSession, so resuming is the AGENT's job.
        assert!(gemini().caps.resume);
    }

    #[test]
    fn a_pasted_screenshot_is_supported() {
        assert!(gemini().images);
    }

    #[test]
    fn history_and_live_list_stay_absent_so_the_ui_degrades() {
        let caps = gemini().caps;
        // No files on disk to index and no way to list sessions started
        // outside Hark: the recorder and the mirror must know this.
        assert!(!caps.history);
        assert!(!caps.live_list);
        assert!(!caps.fork);
        assert!(!caps.structured_output);
    }

    #[test]
    fn permissions_are_always_available_over_acp() {
        assert!(gemini().caps.permissions);
    }

    #[test]
    fn cost_starts_off_and_is_earned_by_a_turn_that_reports_it() {
        assert!(!gemini().caps.cost_reporting);
        // Before session/new nothing is known about modes or config options:
        // the pills must know, or they take the click and do nothing.
        assert!(!gemini().caps.directive_mode && !gemini().caps.directive_model && !gemini().caps.directive_effort);
    }

    #[test]
    fn the_login_hint_can_come_from_the_agents_own_methods() {
        let auth = gemini().auth;
        let ids: Vec<&str> = auth.iter().map(|a| a.id.as_str()).collect();
        assert!(ids.contains(&"oauth-personal"), "got {ids:?}");
        assert_eq!(auth.len(), 4);
    }

    #[test]
    fn an_agent_that_claims_nothing_gets_nothing() {
        // A minimal answer must not accidentally grant capabilities.
        let n = negotiate(&serde_json::json!({}), None);
        assert!(!n.caps.resume);
        assert!(!n.images);
        assert_eq!(n.protocol, 0);
        assert!(n.auth.is_empty());
        assert_eq!(n.info.version, "");
        // …except permissions, which ACP always has.
        assert!(n.caps.permissions);
    }

    /// Recorded from claude-agent-acp 0.76.0 on 14/09/2026.
    const CLAUDE_INIT: &str = include_str!("../fixtures/initialize.claude-agent-acp-0.76.0.json");

    #[test]
    fn the_claude_adapter_announces_itself_at_initialize() {
        // agentCapabilities._meta.claudeCode is the adapter's signature. It
        // is what licenses the `_meta` it understands on session/new — the
        // lean ask, the budget ceiling — and nothing else does.
        let claude = negotiate(&serde_json::from_str(CLAUDE_INIT).unwrap(), Some("CLAUDE.md".into()));
        assert!(claude.claude_code);
        assert!(claude.caps.resume, "loadSession is announced");
        assert!(!gemini().claude_code, "gemini is not the Claude adapter");
    }
}

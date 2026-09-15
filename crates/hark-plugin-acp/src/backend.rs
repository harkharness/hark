//! The ACP plugin as an `AgentBackend`: one instance per registry entry,
//! spawning `cmd args…` in the task's cwd and driving it over stdio.

use crate::caps::Negotiated;
use hark_core::ports::{AgentBackend, AgentRunner, AgentSession, EventRx, SessionSpec};
use std::sync::{Arc, Mutex};

/// A configured ACP agent: what the registry line says plus what the last
/// handshake taught us about it.
pub struct AcpBackend {
    pub id: String,
    pub cmd: String,
    pub args: Vec<String>,
    /// Base env for every process (gateway URLs, keys the user chose to
    /// keep in config). Per-session envs from the spec are added on top.
    pub env: Vec<(String, String)>,
    pub memory_file: Option<String>,
    pub login_hint: Option<String>,
    /// Neutral cwd for the one-shot runner (ask/gate) — the data dir,
    /// never a project, so no project file leaks into a cheap question.
    pub work_dir: std::path::PathBuf,
    /// The sheet the agent answered at its last handshake, once it has.
    negotiated: Mutex<Option<Negotiated>>,
}

impl AcpBackend {
    pub fn new(
        id: impl Into<String>,
        cmd: impl Into<String>,
        args: Vec<String>,
        env: Vec<(String, String)>,
        memory_file: Option<String>,
        login_hint: Option<String>,
        work_dir: std::path::PathBuf,
    ) -> Self {
        Self {
            id: id.into(),
            cmd: cmd.into(),
            args,
            env,
            memory_file,
            login_hint,
            work_dir,
            negotiated: Mutex::new(None),
        }
    }
}

impl AgentBackend for AcpBackend {
    fn id(&self) -> &str {
        &self.id
    }

    fn capabilities(&self) -> hark_agent::Capabilities {
        // Before any handshake the sheet is the pessimistic default: ACP
        // always has permissions, everything else must be earned.
        self.negotiated
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
            .map(|n| n.caps.clone())
            .unwrap_or_else(|| crate::caps::negotiate(&serde_json::json!({}), self.memory_file.clone()).caps)
    }

    fn spawn(&self, spec: &SessionSpec) -> anyhow::Result<(Arc<dyn AgentSession>, EventRx)> {
        // Directives ride the opening: said in the agent's own ids for
        // whatever its session/new offers (modes, config options), and
        // dropped — with the sheet saying so — for what it does not. A
        // fork (`spec.fork` on an existing session) is `session/fork`,
        // which the handshake refuses by name on an agent that does not
        // announce it.
        let mut child = std::process::Command::new(&self.cmd)
            .args(&self.args)
            .current_dir(&spec.cwd)
            .envs(self.env.iter().cloned())
            .envs(spec.envs.iter().cloned())
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| anyhow::anyhow!("{}", spawn_error(&self.cmd, &e)))?;
        let stdin = child.stdin.take().expect("piped stdin");
        let stdout = child.stdout.take().expect("piped stdout");
        let stderr = child.stderr.take();
        let opening = crate::session::Opening {
            agent: &self.id,
            cwd: &spec.cwd,
            session_id: &spec.session_id,
            instruction: &spec.instruction,
            images: &[],
            memory_file: self.memory_file.clone(),
            directives: &spec.directives,
            limits: &spec.limits,
            lean: None,
            fork: spec.fork,
        };
        let connected = crate::session::connect(
            crate::session::Wire { reader: Box::new(stdout), writer: Box::new(stdin) },
            Some(child),
            stderr,
            &opening,
        )
        .map_err(|err| {
            // The registry knows how one logs in to THIS agent.
            let msg = err.to_string();
            match (&self.login_hint, msg.starts_with(crate::translate::AUTH_PREFIX)) {
                (Some(hint), true) => anyhow::anyhow!("{msg} — {hint}"),
                _ => err,
            }
        })?;
        *self.negotiated.lock().unwrap_or_else(|e| e.into_inner()) = Some(connected.negotiated);
        Ok((connected.session, connected.events))
    }

    fn runner(&self) -> Box<dyn AgentRunner + Send + Sync> {
        // No schema mode over ACP: the schema goes into the prompt and the
        // answer is read leniently (ask.rs).
        Box::new(crate::ask::AcpRunner {
            id: self.id.clone(),
            cmd: self.cmd.clone(),
            args: self.args.clone(),
            env: self.env.clone(),
            work_dir: self.work_dir.clone(),
        })
    }
}

/// The same health codes the claude plugin uses, so the windows render
/// a missing agent the same way whatever speaks it.
pub(crate) fn spawn_error(cmd: &str, err: &std::io::Error) -> String {
    match err.kind() {
        std::io::ErrorKind::NotFound => format!("agent_missing: {cmd} ({err})"),
        std::io::ErrorKind::PermissionDenied => format!("agent_blocked: {cmd} ({err})"),
        _ => format!("agent_failed: {cmd} ({err})"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hark_core::ports::AgentBackend;

    fn spec() -> SessionSpec {
        SessionSpec {
            agent: "ghost".into(),
            cwd: std::env::temp_dir(),
            session_id: String::new(),
            instruction: "oi".into(),
            directives: Default::default(),
            limits: Default::default(),
            envs: Vec::new(),
            fork: false,
        }
    }

    #[test]
    fn a_missing_binary_is_reported_as_missing_with_its_name() {
        let backend = AcpBackend::new("ghost", "hark-no-such-agent-xyz", vec![], vec![], None, None, std::env::temp_dir());
        let Err(err) = backend.spawn(&spec()) else { panic!("cannot spawn") };
        let err = err.to_string();
        assert!(err.starts_with("agent_missing: hark-no-such-agent-xyz"), "{err}");
    }

    #[test]
    fn before_any_handshake_the_sheet_is_the_pessimistic_default() {
        let backend = AcpBackend::new("x", "x", vec![], vec![], Some("X.md".into()), None, std::env::temp_dir());
        let caps = backend.capabilities();
        assert!(caps.permissions);
        assert!(!caps.history && !caps.resume && !caps.fork);
        assert_eq!(caps.memory_file.as_deref(), Some("X.md"));
    }

    /// The real thing, on this machine: `cargo test -p hark-plugin-acp
    /// -- --ignored gemini --nocapture`. Whatever the agent answers at
    /// session/new must come back as one of Hark's health codes — logged
    /// in, the opening prompt streams a real turn instead.
    ///
    /// OBSERVED 13/09/2026, gemini-cli 0.46.0, individual account:
    /// `agent_failed: session/new: This client is no longer supported for
    /// Gemini Code Assist for individuals. To continue using Gemini,
    /// please migrate to the Antigravity suite of products` — a
    /// deprecation, not an auth failure, and correctly not classified as
    /// one. The API-key route (`GEMINI_API_KEY` in the entry's env) is
    /// what remains for individuals on this version.
    /// The real adapter, the user's own subscription, ONE short turn on
    /// the light model at low effort. Run by hand, recording the wire:
    ///
    ///   HARK_ACP_TRACE=/tmp/claude-acp.trace \
    ///   cargo test -p hark-plugin-acp -- --ignored claude_agent_acp --nocapture
    ///
    /// What it proves that no fake can: the adapter prices the turn, its
    /// session/new offers modes and the model/effort selectors, and a live
    /// set_mode is taken without a reopen.
    #[test]
    #[ignore = "spawns the real claude-agent-acp binary and spends a turn"]
    fn claude_agent_acp_on_this_machine_prices_a_turn_and_takes_directives() {
        use hark_core::domain::directives::{Directives, Effort, Mode};
        use hark_core::ports::LiveDirectives;

        let backend = AcpBackend::new(
            "claude-acp",
            "claude-agent-acp",
            vec![],
            vec![],
            Some("CLAUDE.md".into()),
            Some("claude /login".into()),
            std::env::temp_dir(),
        );
        let spec = SessionSpec {
            agent: "claude-acp".into(),
            cwd: std::env::temp_dir(),
            session_id: String::new(),
            instruction: "Responda apenas com a palavra: ok".into(),
            directives: Directives { mode: Some(Mode::Manual), effort: Some(Effort::Low), model: Some("haiku".into()) },
            limits: Default::default(),
            envs: Vec::new(),
            fork: false,
        };
        let (session, events) = backend.spawn(&spec).expect("spawns and handshakes");
        let caps = backend.capabilities();
        eprintln!("negotiated sheet: {caps:?}");

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(180);
        let mut result = None;
        while std::time::Instant::now() < deadline {
            match events.recv_timeout(std::time::Duration::from_secs(5)) {
                Ok(hark_agent::AgentEvent::Result(t)) => {
                    result = Some(t);
                    break;
                }
                Ok(ev) => eprintln!("event: {ev:?}"),
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
                Err(_) => break,
            }
        }
        let turn = result.expect("a result within three minutes");
        eprintln!("turn: {turn:?}");
        assert!(!turn.is_error, "{}", turn.raw);
        assert!(turn.cost_usd.is_some_and(|c| c > 0.0), "the adapter prices the turn: {turn:?}");
        assert!(caps.directive_mode && caps.directive_model && caps.directive_effort, "{caps:?}");

        let live = session
            .set_directives(&Directives { mode: Some(Mode::AcceptEdits), effort: None, model: None })
            .expect("set_mode on the live session");
        assert!(matches!(live, LiveDirectives::Applied(a) if a.mode), "{live:?}");
        session.shutdown();
    }

    /// The cheap lane on the real adapter: our persona as the system
    /// prompt, no settings, no tools, one turn, haiku at low effort. The
    /// chat opening above cost $0.057 for one word; this is what the lean
    /// `_meta` buys. Run by hand:
    ///
    ///   HARK_ACP_TRACE=/tmp/claude-acp-ask.trace \
    ///   cargo test -p hark-plugin-acp -- --ignored lean_ask --nocapture
    #[test]
    #[ignore = "spawns the real claude-agent-acp binary and spends a turn"]
    fn claude_agent_acp_answers_a_lean_ask_for_cents() {
        use hark_core::ports::{AgentRunner as _, TurnRequest};
        let runner = crate::ask::AcpRunner {
            id: "claude-acp".into(),
            cmd: "claude-agent-acp".into(),
            args: vec![],
            env: vec![],
            work_dir: std::env::temp_dir(),
        };
        let request = TurnRequest {
            prompt: "Quantas letras tem a palavra 'hark'? Responda em uma frase curta.",
            images: &[],
            model: "haiku",
            system_prompt: "Você é o Hark, assistente de voz. Responda em português, em uma frase.",
            schema: r#"{"type":"object","properties":{"fala":{"type":"string"}},"required":["fala"]}"#,
            effort: "low",
        };
        let turn = runner.ask(&request, &mut |ev| eprintln!("event: {ev:?}")).expect("answers");
        eprintln!("turn: {turn:?}");
        assert!(!turn.is_error, "{}", turn.raw);
        assert!(turn.reply.as_ref().and_then(|r| r.get("fala")).is_some(), "structured reply: {turn:?}");
        let cost = turn.cost_usd.expect("priced");
        assert!(cost < 0.02, "a lean ask costs cents, not {cost}");
    }

    #[test]
    #[ignore = "spawns the real gemini binary"]
    fn gemini_on_this_machine_reports_auth_or_talks() {
        let backend = AcpBackend::new(
            "gemini",
            "gemini",
            vec!["--acp".into()],
            vec![],
            Some("GEMINI.md".into()),
            Some("gemini".into()),
            std::env::temp_dir(),
        );
        match backend.spawn(&spec()) {
            Err(err) => {
                let msg = err.to_string();
                eprintln!("spawn refused: {msg}");
                assert!(
                    ["agent_auth: ", "agent_missing: ", "agent_failed: "].iter().any(|code| msg.starts_with(code)),
                    "a refusal must carry a health code: {msg}"
                );
                if msg.starts_with("agent_auth: ") {
                    assert!(msg.ends_with("— gemini"), "the registry's login hint rides along: {msg}");
                }
            }
            Ok((session, events)) => {
                let mut seen = Vec::new();
                for ev in events.iter() {
                    let done = matches!(ev, hark_agent::AgentEvent::Result(_));
                    seen.push(ev);
                    if done {
                        break;
                    }
                }
                session.shutdown();
                eprintln!("{seen:#?}");
                assert!(seen.iter().any(|e| matches!(e, hark_agent::AgentEvent::SessionStarted { .. })));
                assert!(seen.iter().any(|e| matches!(e, hark_agent::AgentEvent::Result(_))));
            }
        }
    }
}

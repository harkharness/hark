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
    ) -> Self {
        Self {
            id: id.into(),
            cmd: cmd.into(),
            args,
            env,
            memory_file,
            login_hint,
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
        // Directives (mode, effort, model) have no ACP translation in v1
        // and are dropped, as the seam allows; fork is gated by caps.fork.
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
            memory_file: self.memory_file.clone(),
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
        Box::new(Unstructured { id: self.id.clone() })
    }
}

/// The same health codes the claude plugin uses, so the windows render
/// a missing agent the same way whatever speaks it.
fn spawn_error(cmd: &str, err: &std::io::Error) -> String {
    match err.kind() {
        std::io::ErrorKind::NotFound => format!("agent_missing: {cmd} ({err})"),
        std::io::ErrorKind::PermissionDenied => format!("agent_blocked: {cmd} ({err})"),
        _ => format!("agent_failed: {cmd} ({err})"),
    }
}

/// ACP has no schema-constrained one-shot. Until the lenient-extraction
/// lane lands (F9.5), the ask says so instead of pretending.
struct Unstructured {
    id: String,
}

impl AgentRunner for Unstructured {
    fn ask(
        &self,
        _request: &hark_core::ports::TurnRequest,
        _on_event: &mut dyn FnMut(&hark_agent::AgentEvent),
    ) -> anyhow::Result<hark_agent::TurnResult> {
        anyhow::bail!("agent_failed: {} não responde perguntas estruturadas (ask/gate) ainda", self.id)
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
        let backend = AcpBackend::new("ghost", "hark-no-such-agent-xyz", vec![], vec![], None, None);
        let Err(err) = backend.spawn(&spec()) else { panic!("cannot spawn") };
        let err = err.to_string();
        assert!(err.starts_with("agent_missing: hark-no-such-agent-xyz"), "{err}");
    }

    #[test]
    fn before_any_handshake_the_sheet_is_the_pessimistic_default() {
        let backend = AcpBackend::new("x", "x", vec![], vec![], Some("X.md".into()), None);
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

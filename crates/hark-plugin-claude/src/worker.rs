//! Worker runner: resumes an existing Claude Code session with the user's
//! FULL settings (CLAUDE.md, MCP, skills) inside the target workspace, and
//! routes permission requests to a decision callback (terminal y/n today,
//! voice tomorrow). This is the arm of the orchestrator.

use crate::stream::{parse, permission_response, user_message};
use hark_agent::{AgentEvent, PermissionDecision, TurnResult};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

/// Hard ceilings per worker process (contract type) — the post-$18-incident
/// guardrail. The CLI aborts the turn with an error result when a cap hits
/// (`--max-budget-usd` / `--max-turns`).
pub use hark_agent::SpawnLimits;

#[derive(Clone)]
pub struct WorkerSpawn {
    pub claude_bin: String,
    pub cwd: PathBuf,
    /// Session to resume; EMPTY starts a brand-new session in `cwd` (the
    /// real id arrives later via `AgentEvent::SessionStarted`).
    pub session_id: String,
    pub instruction: String,
    /// Session directives (permission mode, effort, model), all optional:
    /// omitted flags keep the user's own Claude Code defaults.
    pub directives: hark_core::domain::directives::Directives,
    pub limits: SpawnLimits,
    /// Per-PROCESS env vars (eco tools: ponytail/caveman/tokensave modes)
    /// — global settings are never touched.
    pub envs: Vec<(String, String)>,
}

impl WorkerSpawn {
    /// `--resume <id>` when continuing a session; nothing when starting fresh.
    fn resume_args(&self) -> Vec<String> {
        if self.session_id.is_empty() {
            Vec::new()
        } else {
            vec!["--resume".into(), self.session_id.clone()]
        }
    }

    /// CLI flags for the current directives.
    fn directive_args(&self) -> Vec<String> {
        let mut args = Vec::new();
        if let Some(mode) = self.directives.mode {
            args.push("--permission-mode".into());
            args.push(mode.as_flag().into());
        }
        if let Some(effort) = self.directives.effort {
            args.push("--effort".into());
            args.push(effort.as_flag().into());
        }
        if let Some(model) = &self.directives.model {
            args.push("--model".into());
            args.push(model.clone());
        }
        args
    }

    /// The COMPLETE argument list every worker path must use. One source
    /// of truth: the one-shot runner used to skip directive flags entirely
    /// (a spoken "rapidinho" was silently ignored).
    pub fn cli_args(&self, partial_messages: bool) -> Vec<String> {
        let mut args: Vec<String> = vec!["-p".into(), "--input-format".into(), "stream-json".into()];
        args.extend(self.resume_args());
        args.extend([
            "--output-format".into(),
            "stream-json".into(),
            "--verbose".into(),
        ]);
        if partial_messages {
            args.push("--include-partial-messages".into());
        }
        args.extend(["--permission-prompt-tool".into(), "stdio".into()]);
        args.extend(self.directive_args());
        if let Some(budget) = self.limits.max_budget_usd.filter(|b| *b > 0.0) {
            args.push("--max-budget-usd".into());
            args.push(format!("{budget}"));
        }
        if let Some(turns) = self.limits.max_turns.filter(|t| *t > 0) {
            args.push("--max-turns".into());
            args.push(turns.to_string());
        }
        // bypassPermissions never asks, so production tooling is denied
        // outright (the gate's hard floor — see domain::prodgate).
        if self.directives.mode == Some(hark_core::domain::directives::Mode::Bypass) {
            args.push("--disallowedTools".into());
            args.extend(
                hark_core::domain::prodgate::BYPASS_DENY_RULES
                    .iter()
                    .map(|r| r.to_string()),
            );
        }
        args
    }
}

pub struct RunningWorker {
    pub pid: u32,
}

/// A long-lived conversational worker: the claude process stays alive
/// between turns, accepting follow-up messages over stream-json stdin.
pub struct PersistentWorker {
    pub pid: u32,
    stdin: std::sync::Mutex<Option<std::process::ChildStdin>>,
    child: std::sync::Mutex<std::process::Child>,
}

impl PersistentWorker {
    /// Spawn the worker and send the opening instruction. Returns the handle
    /// plus the stdout to be consumed by a reader loop on the caller's thread.
    pub fn spawn(spawn: &WorkerSpawn) -> anyhow::Result<(Self, std::process::ChildStdout)> {
        let mut child = std::process::Command::new(&spawn.claude_bin)
            .current_dir(&spawn.cwd)
            .envs(spawn.envs.iter().cloned())
            .args(spawn.cli_args(true))
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::inherit())
            .spawn()?;

        let mut stdin = child.stdin.take().expect("piped stdin");
        stdin.write_all(user_message(&spawn.instruction, &[]).as_bytes())?;
        stdin.write_all(b"\n")?;
        stdin.flush()?;
        let stdout = child.stdout.take().expect("piped stdout");
        Ok((
            Self {
                pid: child.id(),
                stdin: std::sync::Mutex::new(Some(stdin)),
                child: std::sync::Mutex::new(child),
            },
            stdout,
        ))
    }

    fn write_line(&self, line: &str) -> anyhow::Result<()> {
        let mut guard = self.stdin.lock().unwrap();
        let stdin = guard.as_mut().ok_or_else(|| anyhow::anyhow!("worker already closed"))?;
        stdin.write_all(line.as_bytes())?;
        stdin.write_all(b"\n")?;
        stdin.flush()?;
        Ok(())
    }

    /// Follow-up user message (with any pasted screenshots).
    pub fn send_text(&self, text: &str, images: &[(String, String)]) -> anyhow::Result<()> {
        self.write_line(&user_message(text, images))
    }

    /// Answer a pending permission request.
    pub fn respond_permission(
        &self,
        request_id: &str,
        decision: PermissionDecision,
    ) -> anyhow::Result<()> {
        self.write_line(&permission_response(request_id, decision))
    }

    /// Graceful shutdown: EOF on stdin ends the conversation; the reader
    /// loop sees the stream close. Kills after that as a safety net.
    pub fn shutdown(&self) {
        self.stdin.lock().unwrap().take();
        std::thread::sleep(std::time::Duration::from_millis(300));
        let mut child = self.child.lock().unwrap();
        let _ = child.kill();
        let _ = child.wait();
    }
}

/// Spawn and stream one worker turn. `decide` is called for every permission
/// request; `on_event` sees every parsed event (for narration/logging).
/// `on_spawn` receives the pid right after spawn (for the registry).
pub fn run(
    spawn: &WorkerSpawn,
    on_spawn: &mut dyn FnMut(RunningWorker),
    decide: &mut dyn FnMut(&str, &str) -> PermissionDecision,
    on_event: &mut dyn FnMut(&AgentEvent),
) -> anyhow::Result<TurnResult> {
    // The stdio permission channel only stays open with stream-json INPUT
    // (spikes/FINDINGS.md): the instruction goes as a user message on stdin,
    // never as a CLI argument.
    let mut child = std::process::Command::new(&spawn.claude_bin)
        .current_dir(&spawn.cwd)
        .envs(spawn.envs.iter().cloned())
        .args(spawn.cli_args(false))
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;

    on_spawn(RunningWorker { pid: child.id() });
    let mut stdin = child.stdin.take().expect("piped stdin");
    let stdout = child.stdout.take().expect("piped stdout");

    let first_message = serde_json::json!({
        "type": "user",
        "message": { "role": "user", "content": [{ "type": "text", "text": spawn.instruction }] }
    });
    stdin.write_all(first_message.to_string().as_bytes())?;
    stdin.write_all(b"\n")?;
    stdin.flush()?;

    let debug = std::env::var_os("HARK_DEBUG").is_some();
    let mut result = None;
    for line in BufReader::new(stdout).lines().map_while(Result::ok) {
        if debug {
            eprintln!("[worker] {line}");
        }
        let event = parse(&line);
        on_event(&event);
        match event {
            AgentEvent::PermissionRequest {
                ref request_id,
                ref tool_name,
                ref input,
            } => {
                let decision = decide(tool_name, input);
                let response = permission_response(request_id, decision);
                stdin.write_all(response.as_bytes())?;
                stdin.write_all(b"\n")?;
                stdin.flush()?;
            }
            AgentEvent::Result(r) => {
                result = Some(r);
                break;
            }
            _ => {}
        }
    }

    drop(stdin);
    let stderr_tail = child
        .stderr
        .take()
        .map(|s| {
            use std::io::Read;
            let mut buf = String::new();
            let _ = std::io::BufReader::new(s).read_to_string(&mut buf);
            buf.chars().rev().take(500).collect::<Vec<_>>().into_iter().rev().collect::<String>()
        })
        .unwrap_or_default();
    let status = child.wait()?;
    match result {
        Some(r) => Ok(r),
        None => anyhow::bail!(
            "worker exited ({status}) without a result event; stderr: {}",
            stderr_tail.trim()
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hark_core::domain::directives::{Directives, Effort, Mode};

    fn spawn(session: &str, directives: Directives) -> WorkerSpawn {
        WorkerSpawn {
            claude_bin: "claude".into(),
            cwd: PathBuf::from("/tmp"),
            session_id: session.into(),
            instruction: "go".into(),
            directives,
            limits: SpawnLimits::default(),
            envs: Vec::new(),
        }
    }

    #[test]
    fn bypass_mode_still_blocks_production_tools() {
        // bypassPermissions never asks, so the infra CLIs must be denied
        // outright — the production gate's hard floor survives the mode.
        let args = spawn(
            "s-1",
            Directives { mode: Some(Mode::Bypass), ..Directives::default() },
        )
        .cli_args(true);
        let flag = args.iter().position(|a| a == "--disallowedTools").expect("deny rules");
        assert!(args[flag + 1..].contains(&"Bash(kubectl:*)".to_string()));
        assert!(args[flag + 1..].contains(&"Bash(terraform:*)".to_string()));

        // Every other mode asks through stdio: no blanket denies there.
        for mode in [None, Some(Mode::Auto), Some(Mode::AcceptEdits), Some(Mode::Manual)] {
            let args = spawn("s-1", Directives { mode, ..Directives::default() }).cli_args(true);
            assert!(!args.contains(&"--disallowedTools".to_string()), "{mode:?}");
        }
    }

    #[test]
    fn cli_args_carry_directives_on_every_path() {
        let directives = Directives {
            mode: Some(Mode::Plan),
            effort: Some(Effort::Low),
            model: Some("haiku".into()),
        };
        // Both the persistent (partial messages) and one-shot paths must
        // apply spoken directives — the one-shot used to drop them.
        for partial in [true, false] {
            let args = spawn("s-1", directives.clone()).cli_args(partial);
            assert!(args.contains(&"--resume".to_string()));
            assert!(args.contains(&"--permission-mode".to_string()));
            assert!(args.contains(&"--effort".to_string()));
            assert!(args.contains(&"low".to_string()));
            assert!(args.contains(&"--model".to_string()));
            assert_eq!(partial, args.contains(&"--include-partial-messages".to_string()));
        }
    }

    #[test]
    fn empty_session_id_means_a_fresh_session() {
        let args = spawn("", Directives::default()).cli_args(true);
        assert!(!args.contains(&"--resume".to_string()));
    }

    #[test]
    fn hard_limits_become_cli_caps() {
        let mut s = spawn("s-1", Directives::default());
        s.limits = SpawnLimits { max_budget_usd: Some(2.0), max_turns: Some(30) };
        let args = s.cli_args(true);
        assert!(args.contains(&"--max-budget-usd".to_string()));
        assert!(args.contains(&"2".to_string()));
        assert!(args.contains(&"--max-turns".to_string()));
        assert!(args.contains(&"30".to_string()));
        // Zero means disabled, not "cap at zero".
        s.limits = SpawnLimits { max_budget_usd: Some(0.0), max_turns: Some(0) };
        let args = s.cli_args(true);
        assert!(!args.contains(&"--max-budget-usd".to_string()));
        assert!(!args.contains(&"--max-turns".to_string()));
    }
}

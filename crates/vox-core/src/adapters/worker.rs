//! Worker runner: resumes an existing Claude Code session with the user's
//! FULL settings (CLAUDE.md, MCP, skills) inside the target workspace, and
//! routes permission requests to a decision callback (terminal y/n today,
//! voice tomorrow). This is the arm of the orchestrator.

use crate::domain::claude_event::{
    parse, permission_response, user_message, ClaudeEvent, PermissionDecision, TurnResult,
};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

pub struct WorkerSpawn {
    pub claude_bin: String,
    pub cwd: PathBuf,
    pub session_id: String,
    pub instruction: String,
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
            .args([
                "-p",
                "--input-format",
                "stream-json",
                "--resume",
                &spawn.session_id,
                "--output-format",
                "stream-json",
                "--verbose",
                "--include-partial-messages",
                "--permission-prompt-tool",
                "stdio",
            ])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::inherit())
            .spawn()?;

        let mut stdin = child.stdin.take().expect("piped stdin");
        stdin.write_all(user_message(&spawn.instruction, None).as_bytes())?;
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

    /// Follow-up user message (with optional pasted image).
    pub fn send_text(&self, text: &str, image: Option<(&str, &str)>) -> anyhow::Result<()> {
        self.write_line(&user_message(text, image))
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
    on_event: &mut dyn FnMut(&ClaudeEvent),
) -> anyhow::Result<TurnResult> {
    // The stdio permission channel only stays open with stream-json INPUT
    // (spikes/FINDINGS.md): the instruction goes as a user message on stdin,
    // never as a CLI argument.
    let mut child = std::process::Command::new(&spawn.claude_bin)
        .current_dir(&spawn.cwd)
        .args([
            "-p",
            "--input-format",
            "stream-json",
            "--resume",
            &spawn.session_id,
            "--output-format",
            "stream-json",
            "--verbose",
            "--permission-prompt-tool",
            "stdio",
        ])
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

    let debug = std::env::var_os("VOX_DEBUG").is_some();
    let mut result = None;
    for line in BufReader::new(stdout).lines().map_while(Result::ok) {
        if debug {
            eprintln!("[worker] {line}");
        }
        let event = parse(&line);
        on_event(&event);
        match event {
            ClaudeEvent::PermissionRequest {
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
            ClaudeEvent::Result(r) => {
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

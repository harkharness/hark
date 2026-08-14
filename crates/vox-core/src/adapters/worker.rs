//! Worker runner: resumes an existing Claude Code session with the user's
//! FULL settings (CLAUDE.md, MCP, skills) inside the target workspace, and
//! routes permission requests to a decision callback (terminal y/n today,
//! voice tomorrow). This is the arm of the orchestrator.

use crate::domain::claude_event::{
    parse, permission_response, ClaudeEvent, PermissionDecision, TurnResult,
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
    let status = child.wait()?;
    match result {
        Some(r) => Ok(r),
        None => anyhow::bail!("worker exited ({status}) without a result event"),
    }
}

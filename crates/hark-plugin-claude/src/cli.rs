//! Spawns the `claude` binary for one fast-mode question and streams events.
//!
//! Flags per spikes/FINDINGS.md: lean context ($0.009/turn measured) with a
//! JSON-schema-constrained reply. Spawned from a neutral cwd so no project
//! CLAUDE.md is picked up. No Anthropic API is ever called directly.

use crate::stream::{parse, user_message};
use hark_agent::{AgentEvent, TurnResult};
use hark_core::domain::prompt::{RESPONSE_SCHEMA, VOICE_SYSTEM_PROMPT};
use hark_core::ports::{AgentRunner, TurnRequest};
use std::io::{BufRead, BufReader, Write};

pub struct ClaudeCli {
    pub claude_bin: String,
    /// Neutral working directory for the spawned process.
    pub work_dir: std::path::PathBuf,
}

impl AgentRunner for ClaudeCli {
    fn ask(
        &self,
        request: &TurnRequest,
        on_event: &mut dyn FnMut(&AgentEvent),
    ) -> anyhow::Result<TurnResult> {
        // stream-json input so the prompt can carry image blocks (pasted
        // screenshots) exactly like the worker path.
        let mut child = std::process::Command::new(&self.claude_bin)
            .current_dir(&self.work_dir)
            .args([
                "-p",
                "--input-format",
                "stream-json",
                "--model",
                request.model,
                // Fast mode: minimal reasoning keeps latency and cost flat.
                "--effort",
                "low",
                "--output-format",
                "stream-json",
                "--verbose",
                "--json-schema",
                RESPONSE_SCHEMA,
                "--tools",
                "",
                "--setting-sources",
                "",
                "--system-prompt",
                VOICE_SYSTEM_PROMPT,
            ])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()?;

        let mut stdin = child.stdin.take().expect("piped stdin");
        stdin.write_all(user_message(request.prompt, request.images).as_bytes())?;
        stdin.write_all(b"\n")?;
        stdin.flush()?;
        drop(stdin); // one-shot turn: EOF ends the conversation after the result

        let debug = std::env::var_os("HARK_DEBUG").is_some();
        let stdout = child.stdout.take().expect("piped stdout");
        let result = BufReader::new(stdout)
            .lines()
            .map_while(Result::ok)
            .inspect(|line| {
                if debug {
                    eprintln!("[debug] {line}");
                }
            })
            .map(|line| parse(&line))
            .inspect(|event| on_event(event))
            .find_map(|event| match event {
                AgentEvent::Result(r) => Some(r),
                _ => None,
            });

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
                "claude exited ({status}) without a result event; stderr: {}",
                stderr_tail.trim()
            ),
        }
    }
}

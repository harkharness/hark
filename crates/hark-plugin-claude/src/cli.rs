//! Spawns the `claude` binary for one fast-mode question and streams events.
//!
//! Flags per docs/FINDINGS.md: lean context ($0.009/turn measured) with a
//! JSON-schema-constrained reply. Spawned from a neutral cwd so no project
//! CLAUDE.md is picked up. No Anthropic API is ever called directly.

use crate::stream::{parse, user_message};
use hark_agent::{AgentEvent, TurnResult};
use hark_core::ports::{AgentRunner, TurnRequest};
use std::io::{BufRead, BufReader, Write};

pub struct ClaudeCli {
    pub claude_bin: String,
    /// Neutral working directory for the spawned process.
    pub work_dir: std::path::PathBuf,
    /// The registry entry's base env (a gateway URL and token, say):
    /// the ask and the gate go through the same door as the workers.
    pub envs: Vec<(String, String)>,
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
            .envs(self.envs.iter().cloned())
            .args([
                "-p",
                "--input-format",
                "stream-json",
                "--model",
                request.model,
                "--effort",
                request.effort,
                "--output-format",
                "stream-json",
                "--verbose",
                "--json-schema",
                request.schema,
                "--tools",
                "",
                "--setting-sources",
                "",
                "--system-prompt",
                request.system_prompt,
            ])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| anyhow::anyhow!(crate::health::spawn_error(&self.claude_bin, &e)))?;

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
            // No result event means the turn never happened. WHY it never
            // happened is the only useful thing we can say.
            None => anyhow::bail!(crate::health::exit_error(
                &self.claude_bin,
                &status.to_string(),
                &stderr_tail
            )),
        }
    }
}

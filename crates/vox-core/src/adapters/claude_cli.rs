//! Spawns the `claude` binary for one fast-mode question and streams events.
//!
//! Flags per spikes/FINDINGS.md: lean context ($0.009/turn measured) with a
//! JSON-schema-constrained reply. Spawned from a neutral cwd so no project
//! CLAUDE.md is picked up. No Anthropic API is ever called directly.

use crate::domain::claude_event::{parse, user_message, ClaudeEvent, TurnResult};
use crate::domain::prompt::{RESPONSE_SCHEMA, VOICE_SYSTEM_PROMPT};
use crate::ports::AgentRunner;
use std::io::{BufRead, BufReader, Write};

pub struct ClaudeCli {
    pub claude_bin: String,
    pub model: String,
    /// Neutral working directory for the spawned process.
    pub work_dir: std::path::PathBuf,
}

impl AgentRunner for ClaudeCli {
    fn ask(
        &self,
        prompt: &str,
        image: Option<(&str, &str)>,
        on_event: &mut dyn FnMut(&ClaudeEvent),
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
                &self.model,
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
        stdin.write_all(user_message(prompt, image).as_bytes())?;
        stdin.write_all(b"\n")?;
        stdin.flush()?;
        drop(stdin); // one-shot turn: EOF ends the conversation after the result

        let debug = std::env::var_os("VOX_DEBUG").is_some();
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
                ClaudeEvent::Result(r) => Some(r),
                _ => None,
            });

        let status = child.wait()?;
        match result {
            Some(r) => Ok(r),
            None => anyhow::bail!("claude exited ({status}) without a result event"),
        }
    }
}

//! The Claude Code CLI as a Hark agent plugin.
//!
//! Everything that knows the `claude` binary, its stream-json dialect, its
//! control protocol, its on-disk session layout (~/.claude/projects) and
//! its plugin ecosystem lives HERE — hark-core stays agent-neutral and
//! speaks only the `hark-agent` contract.

pub mod backend;
pub mod bridge;
pub mod cli;
pub mod eco;
pub mod health;
pub mod history;
pub mod live;
pub mod log;
pub mod statusline;
pub mod stream;
pub mod worker;

/// What the Claude Code backend can do, in contract terms.
pub fn capabilities() -> hark_agent::Capabilities {
    hark_agent::Capabilities {
        resume: true,
        permissions: true,
        structured_output: true,
        cost_reporting: true,
        history: true,
        live_list: true,
        slash_commands: true,
        memory_file: Some("CLAUDE.md".into()),
        shell_tools: vec!["Bash".into()],
        fork: true,
    }
}

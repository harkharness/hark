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
pub mod trust;
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
        // --permission-mode, --model, --effort: every directive is a flag.
        directive_mode: true,
        directive_model: true,
        directive_effort: true,
    }
}

#[cfg(test)]
mod capability_sheet {
    #[test]
    fn the_native_plugin_carries_every_directive_and_forks() {
        // The sheet is what the UI consults before offering a control. The
        // CLI takes --permission-mode, --model and --effort, and forks a
        // session; an ACP agent, today, takes none of those from Hark.
        let caps = super::capabilities();
        assert!(caps.directive_mode && caps.directive_model && caps.directive_effort, "mode/model/effort reach the CLI");
        assert!(caps.fork);
    }
}

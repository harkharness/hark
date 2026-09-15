//! Which registry entries have their binary on this machine.

use crate::config::Config;
use crate::domain::agents::AgentEntry;

/// Claude keeps Hark's own resolution of the binary (config can point at
/// an absolute path; nvm shadows are skipped): (found, the binary).
pub fn claude(config: &Config) -> (bool, String) {
    let bin = config.claude_bin_resolved();
    let ok = std::path::Path::new(&bin).is_absolute() || which(&bin);
    (ok, bin)
}

fn which(cmd: &str) -> bool {
    std::process::Command::new("which")
        .arg(cmd)
        .output()
        .is_ok_and(|out| out.status.success())
}

/// The registry ids whose binary answers `which` — claude through its
/// own resolution, everything else by its command.
pub fn detected(config: &Config, entries: &[AgentEntry]) -> Vec<String> {
    entries
        .iter()
        .filter(|e| {
            if e.plugin == "claude" && e.cmd == "claude" {
                claude(config).0
            } else {
                which(&e.cmd)
            }
        })
        .map(|e| e.id.clone())
        .collect()
}

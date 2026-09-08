//! Deciding what PATH a spawned agent should get.
//!
//! A GUI app on macOS is started by launchd, not by a shell, so it gets
//! `/usr/bin:/bin:/usr/sbin:/sbin` and nothing else — no Homebrew, no
//! nvm, no ~/.local/bin. Every child it spawns inherits that, and the
//! agent CLI we spawn then fails to run the user's own hooks and MCP
//! launchers with "command not found" for tools that plainly exist.
//!
//! Pure. Reading `$SHELL` and running it lives in the shell.

/// Directories a launchd-started process gets for free. Everything else
/// in a PATH had to be put there by a shell profile.
const SYSTEM_DIRS: &[&str] = &["/usr/bin", "/bin", "/usr/sbin", "/sbin", "/usr/local/bin"];

fn is_system_dir(entry: &str) -> bool {
    SYSTEM_DIRS.contains(&entry)
        || entry.starts_with("/System/")
        || entry.starts_with("/var/run/com.apple")
}

/// True when nothing but system directories is on the PATH — the
/// signature of a process no shell ever touched.
pub fn looks_minimal(path: &str) -> bool {
    path.split(':').map(str::trim).filter(|e| !e.is_empty()).all(is_system_dir)
}

/// The shell's PATH first (it is the user's real answer), then whatever
/// we already had, then known bin dirs the caller confirmed exist.
/// Order is preserved and nothing is listed twice.
pub fn merge(current: &str, login: &str, extras: &[String]) -> String {
    let mut out: Vec<String> = Vec::new();
    let all = login
        .split(':')
        .chain(current.split(':'))
        .chain(extras.iter().map(String::as_str));
    for entry in all.map(str::trim).filter(|e| !e.is_empty()) {
        if !out.iter().any(|kept| kept == entry) {
            out.push(entry.to_string());
        }
    }
    out.join(":")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_gui_launch_carries_nothing_but_system_directories() {
        assert!(looks_minimal("/usr/bin:/bin:/usr/sbin:/sbin"));
        assert!(looks_minimal("/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin"));
        // An empty PATH is as impoverished as it gets.
        assert!(looks_minimal(""));
    }

    #[test]
    fn a_shell_launched_process_is_left_alone() {
        assert!(!looks_minimal("/opt/homebrew/bin:/usr/bin:/bin"));
        assert!(!looks_minimal("/Users/x/.nvm/versions/node/v24.7.0/bin:/usr/bin"));
        assert!(!looks_minimal("/usr/bin:/bin:/Users/x/.local/bin"));
    }

    #[test]
    fn merging_leads_with_the_shells_answer_and_keeps_what_we_had() {
        let out = merge(
            "/usr/bin:/bin",
            "/opt/homebrew/bin:/usr/bin",
            &["/Users/x/.cargo/bin".to_string()],
        );
        assert_eq!(out, "/opt/homebrew/bin:/usr/bin:/bin:/Users/x/.cargo/bin");
    }

    #[test]
    fn no_directory_is_listed_twice() {
        // Every source repeats an entry the others already have.
        let out = merge("/bin:/usr/bin", "/opt/homebrew/bin:/bin", &["/usr/bin".to_string()]);
        assert_eq!(out, "/opt/homebrew/bin:/bin:/usr/bin");
    }

    #[test]
    fn blank_and_padded_entries_never_survive() {
        // A profile that ends in ":" is common; so is one with stray spaces.
        assert_eq!(merge("", "/opt/homebrew/bin: :/bin:", &[]), "/opt/homebrew/bin:/bin");
    }

    #[test]
    fn a_shell_that_answered_nothing_leaves_the_path_as_it_was() {
        assert_eq!(merge("/usr/bin:/bin", "", &[]), "/usr/bin:/bin");
    }
}

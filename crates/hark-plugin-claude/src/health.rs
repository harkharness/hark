//! What went wrong with the agent binary, said in a way someone can act on.
//!
//! A missing CLI used to surface as "No such file or directory (os error
//! 2)": true, and useless — it names neither the binary nor the fix. The
//! two failures that actually happen to a new user are the CLI not being
//! installed and the CLI not being logged in, and they need different
//! answers.
//!
//! Codes, not prose: the window owns the wording (and the language), the
//! plugin owns the diagnosis — the same split the mic errors use.

/// The CLI is not where we looked.
pub const AGENT_MISSING: &str = "agent_missing";
/// It is there, but it will not run (permissions, quarantine).
pub const AGENT_BLOCKED: &str = "agent_blocked";
/// It ran and told us the user is not authenticated.
pub const AGENT_AUTH: &str = "agent_auth";
/// It failed some other way; the tail says how.
pub const AGENT_FAILED: &str = "agent_failed";

/// Diagnose a failed spawn of the agent binary.
pub fn spawn_error(bin: &str, err: &std::io::Error) -> String {
    let code = match err.kind() {
        std::io::ErrorKind::NotFound => AGENT_MISSING,
        std::io::ErrorKind::PermissionDenied => AGENT_BLOCKED,
        _ => AGENT_FAILED,
    };
    format!("{code}: {bin} ({err})")
}

/// Does this stderr read like the CLI refusing for lack of a login?
///
/// Deliberately narrow. A false positive tells someone to re-authenticate
/// when the real problem is elsewhere, which is a worse waste of their
/// time than a generic error.
pub fn looks_unauthenticated(stderr: &str) -> bool {
    let s = stderr.to_lowercase();
    const PHRASES: &[&str] = &[
        "not logged in",
        "please log in",
        "please login",
        "run `claude login`",
        "claude login",
        "authentication required",
        "authentication failed",
        "unauthorized",
        "invalid api key",
        "oauth token",
        "credentials",
        "sessão expirada",
        "sessao expirada",
    ];
    PHRASES.iter().any(|p| s.contains(p))
}

/// Turn a non-zero exit into a coded, actionable error.
pub fn exit_error(bin: &str, status: &str, stderr: &str) -> String {
    let tail: String = stderr.trim().chars().take(300).collect();
    if looks_unauthenticated(&tail) {
        return format!("{AGENT_AUTH}: {bin} ({tail})");
    }
    format!("{AGENT_FAILED}: {bin} saiu {status} ({tail})")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn io(kind: std::io::ErrorKind) -> std::io::Error {
        std::io::Error::new(kind, "boom")
    }

    #[test]
    fn a_missing_binary_is_named_as_missing() {
        let msg = spawn_error("/usr/local/bin/claude", &io(std::io::ErrorKind::NotFound));
        assert!(msg.starts_with(AGENT_MISSING), "{msg}");
        // The path matters: the fix is usually "it is somewhere else".
        assert!(msg.contains("/usr/local/bin/claude"), "{msg}");
    }

    #[test]
    fn an_unexecutable_binary_is_not_a_missing_one() {
        let msg = spawn_error("claude", &io(std::io::ErrorKind::PermissionDenied));
        assert!(msg.starts_with(AGENT_BLOCKED), "{msg}");
    }

    #[test]
    fn a_login_complaint_is_recognised() {
        for stderr in [
            "Error: Not logged in. Run `claude login` to continue.",
            "authentication failed",
            "401 Unauthorized",
            "Invalid API key provided",
        ] {
            assert!(looks_unauthenticated(stderr), "{stderr:?}");
        }
    }

    #[test]
    fn ordinary_failures_are_not_blamed_on_the_login() {
        // Telling someone to re-authenticate when the real problem is a
        // full disk wastes more of their time than a generic error.
        for stderr in [
            "error: unknown flag --wat",
            "ENOSPC: no space left on device",
            "TypeError: cannot read property of undefined",
            "",
        ] {
            assert!(!looks_unauthenticated(stderr), "{stderr:?}");
        }
    }

    #[test]
    fn an_exit_carries_its_diagnosis_and_its_tail() {
        let auth = exit_error("claude", "exit status: 1", "Error: not logged in");
        assert!(auth.starts_with(AGENT_AUTH), "{auth}");
        let other = exit_error("claude", "exit status: 2", "unknown flag");
        assert!(other.starts_with(AGENT_FAILED), "{other}");
        assert!(other.contains("unknown flag"), "{other}");
    }
}

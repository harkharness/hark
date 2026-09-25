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
        "failed to authenticate",
        "unauthorized",
        "invalid api key",
        // The CLI says "OAuth session expired and could not be refreshed";
        // match the protocol name alone so rewordings keep landing here —
        // "oauth" appears in no other error family the CLI emits.
        "oauth",
        "credentials",
        "sessão expirada",
        "sessao expirada",
    ];
    PHRASES.iter().any(|p| s.contains(p))
}

/// Did the CLI stop the turn at `--max-budget-usd`? Its result carries no
/// text; the stream parser puts the subtype first instead.
pub fn budget_stop(raw: &str) -> bool {
    raw.starts_with("error_max_budget_usd")
}

/// Is this the text the stream parser wrote in place of an empty error
/// result (`error_max_turns: …`)? The CLI's subtype, not anything the
/// agent said.
pub fn subtype_text(raw: &str) -> bool {
    let head = raw.split(':').next().unwrap_or_default();
    head.starts_with("error_") && head.chars().all(|c| c.is_ascii_lowercase() || c == '_')
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

    /// Caught in the wild (26/08): the CLI's real wording inverts ours —
    /// "Failed to authenticate", "OAuth session" — and the narrow list let
    /// it fall through as a raw agent_failed with nothing actionable.
    #[test]
    fn the_real_oauth_expiry_is_classified_as_auth() {
        let real = "Failed to authenticate: OAuth session expired and could not be refreshed";
        assert!(looks_unauthenticated(real));
        assert!(exit_error("claude", "1", real).starts_with("agent_auth:"));
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
    fn a_budget_stop_is_told_apart_from_other_failures() {
        // The parser names the subtype when the CLI gives no text.
        assert!(budget_stop("error_max_budget_usd"));
        assert!(budget_stop("error_max_budget_usd: Reached maximum budget"));
        assert!(!budget_stop("error_during_execution: boom"));
        assert!(!budget_stop("the budget of the project was discussed"));
    }

    #[test]
    fn the_parsers_stand_in_text_is_told_apart_from_what_an_agent_said() {
        // A stopped turn ends as error_during_execution with no text; the
        // parser names the subtype, and a stop must not show it as a reply.
        assert!(subtype_text("error_during_execution"));
        assert!(subtype_text("error_max_turns: Reached maximum number of turns"));
        assert!(!subtype_text("error handling is fine now"));
        assert!(!subtype_text("Error: boom"));
        assert!(!subtype_text(""));
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

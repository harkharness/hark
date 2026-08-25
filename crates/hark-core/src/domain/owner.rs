//! Who is holding a session right now.
//!
//! A session's transcript has exactly one writer. When a human resumes it
//! at a terminal — ours or any other — Hark must stop writing and follow
//! the file instead, or two writers interleave into one history.

use super::prompt::LiveSession;
use serde::Serialize;

/// A session someone else is holding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Owner {
    pub session_id: String,
    /// The backend's own name for the session ("workspace-fabrica-c7").
    pub name: String,
    pub cwd: String,
    pub pid: Option<i32>,
    pub started_at: Option<i64>,
}

/// Sessions held by a human at a terminal — ours, another app's, tmux, it
/// makes no difference. Two things are deliberately NOT owners: a headless
/// run (Hark's own workers appear in the same listing) and a session Hark
/// is already driving, since neither is a takeover.
pub fn terminal_owners(live: &[LiveSession], ours: &[String]) -> Vec<Owner> {
    live.iter()
        .filter(|s| s.kind.as_deref() == Some("interactive"))
        .filter_map(|s| {
            let id = s.session_id.clone()?;
            (!ours.iter().any(|m| *m == id)).then(|| Owner {
                session_id: id,
                name: s.name.clone(),
                cwd: s.cwd.clone(),
                pid: s.pid,
                started_at: s.started_at,
            })
        })
        .collect()
}

/// The owner of one session, if any.
pub fn owner_of<'a>(owners: &'a [Owner], session_id: &str) -> Option<&'a Owner> {
    owners.iter().find(|o| o.session_id == session_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn live(name: &str, id: Option<&str>, kind: Option<&str>) -> LiveSession {
        LiveSession {
            name: name.into(),
            cwd: "/Users/dev/proj".into(),
            status: None,
            session_id: id.map(String::from),
            pid: Some(4242),
            kind: kind.map(String::from),
            started_at: Some(1_787_664_465_560),
        }
    }

    #[test]
    fn a_human_at_a_terminal_owns_the_session() {
        let owners = terminal_owners(&[live("proj-c7", Some("s-1"), Some("interactive"))], &[]);
        assert_eq!(owners.len(), 1);
        assert_eq!(owners[0].session_id, "s-1");
        assert_eq!(owners[0].name, "proj-c7");
        assert_eq!(owners[0].pid, Some(4242));
        assert_eq!(owners[0].started_at, Some(1_787_664_465_560));
    }

    /// Hark's own workers show up in the same listing. Treating one as an
    /// owner would lock the user out of the chat that Hark itself is
    /// driving — both the kind and the id we already hold rule it out.
    #[test]
    fn our_own_runs_are_never_owners() {
        let listing = vec![
            live("proj-a1", Some("s-worker"), Some("print")),
            live("proj-b2", Some("s-mine"), Some("interactive")),
        ];
        let owners = terminal_owners(&listing, &["s-mine".to_string()]);
        assert!(owners.is_empty(), "headless run and a session we drive: neither owns");
    }

    #[test]
    fn entries_without_a_session_are_ignored() {
        let owners = terminal_owners(&[live("proj-c7", None, Some("interactive"))], &[]);
        assert!(owners.is_empty());
        // An unreported kind is not a claim of ownership either.
        assert!(terminal_owners(&[live("x", Some("s-2"), None)], &[]).is_empty());
    }

    #[test]
    fn finds_the_owner_of_one_session() {
        let listing = vec![
            live("a", Some("s-1"), Some("interactive")),
            live("b", Some("s-2"), Some("interactive")),
        ];
        let owners = terminal_owners(&listing, &[]);
        assert_eq!(owner_of(&owners, "s-2").map(|o| o.name.as_str()), Some("b"));
        assert!(owner_of(&owners, "s-9").is_none());
    }
}

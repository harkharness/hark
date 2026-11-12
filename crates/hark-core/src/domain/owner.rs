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

/// How long a session our worker just let go still counts as ours: the
/// CLI's listing lags its process by a few seconds.
pub const GRACE_MS: u64 = 15_000;

/// Sessions our workers let go recently. `claude agents` keeps listing a
/// session for a few seconds after its process is gone; read as a
/// takeover, that lag switched the chat to mirror mode and replayed the
/// turn it had just shown (02/11).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Released {
    rows: Vec<(String, u64)>,
}

impl Released {
    /// Our worker on `session` exited at `at_ms`. Releasing again moves
    /// its clock; what expired already is pruned on the way.
    pub fn release(&mut self, session: &str, at_ms: u64) {
        self.rows.retain(|(s, t)| s != session && at_ms.saturating_sub(*t) <= GRACE_MS);
        self.rows.push((session.to_string(), at_ms));
    }

    /// The sessions still inside their grace at `now_ms`.
    pub fn still_ours(&self, now_ms: u64, grace_ms: u64) -> Vec<String> {
        self.rows.iter().filter(|(_, t)| now_ms.saturating_sub(*t) <= grace_ms).map(|(s, _)| s.clone()).collect()
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
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
            (!ours.contains(&id)).then(|| Owner {
                session_id: id,
                name: s.name.clone(),
                cwd: s.cwd.clone(),
                pid: s.pid,
                started_at: s.started_at,
            })
        })
        .collect()
}

/// Whether `session_id` is held by someone other than Hark right now: an
/// interactive listing that is not one of ours. Sessions Hark drives
/// itself (`ours`: the registry's workers, the chat's own session, the
/// ones let go inside the grace) never read as held.
pub fn holds(live: &[LiveSession], ours: &[String], session_id: &str) -> bool {
    owner_of(&terminal_owners(live, ours), session_id).is_some()
}

/// The owner of one session, if any.
pub fn owner_of<'a>(owners: &'a [Owner], session_id: &str) -> Option<&'a Owner> {
    owners.iter().find(|o| o.session_id == session_id)
}

/// Hark's own machinery sessions (ask one-shots, the hark-chat worker) run
/// with cwd inside the data dir. They are real transcripts, but never
/// catalog material: offering them as targets self-references — the user
/// quotes Hark's answer and the search finds the session that SAID it.
pub fn is_machinery_cwd(cwd: Option<&str>, data_dir: &str) -> bool {
    let Some(cwd) = cwd else { return false };
    cwd == data_dir || cwd.starts_with(&format!("{}/", data_dir.trim_end_matches('/')))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn listed(id: &str, kind: &str) -> LiveSession {
        LiveSession {
            name: "x".into(),
            cwd: "/p".into(),
            status: None,
            session_id: Some(id.into()),
            pid: Some(1),
            kind: Some(kind.into()),
            started_at: None,
        }
    }

    #[test]
    fn a_session_hark_itself_holds_is_not_busy_for_a_dispatch() {
        // The planner used to ask "who holds it?" with an empty list of
        // ours, so a chat Hark was driving read as "open in a terminal"
        // and the message was refused (12/11, the model switch).
        let live = vec![listed("s1", "interactive")];
        assert!(holds(&live, &[], "s1"));
        assert!(!holds(&live, &["s1".to_string()], "s1"));
    }

    #[test]
    fn a_headless_run_holds_nothing() {
        let live = vec![listed("s2", "headless")];
        assert!(!holds(&live, &[], "s2"));
    }

    #[test]
    fn a_session_our_worker_just_let_go_is_still_ours_for_a_grace_period() {
        // `claude agents` keeps listing a session for a few seconds after
        // its process is gone: reading that lag as a takeover switched the
        // chat to mirror mode and replayed the turn it had just shown.
        let mut r = Released::default();
        r.release("s1", 10_000);
        assert_eq!(r.still_ours(12_000, 15_000), vec!["s1"]);
        assert!(r.still_ours(30_000, 15_000).is_empty());
        // A new release prunes what already expired.
        r.release("s2", 40_000);
        assert_eq!(r.still_ours(41_000, 15_000), vec!["s2"]);
        assert_eq!(r.len(), 1);
        // Releasing the same session again moves its clock.
        r.release("s2", 50_000);
        assert_eq!(r.still_ours(64_000, 15_000), vec!["s2"]);
    }


    #[test]
    fn harks_own_machinery_sessions_are_never_catalog_material() {
        // Ask one-shots run with cwd = the data dir (~/.hark); their
        // transcripts quote Hark's own answers, so any topic search that
        // includes them self-references (incident 30/08: three ".hark"
        // sessions offered as dispatch destinations).
        let data = "/Users/dev/.hark";
        assert!(is_machinery_cwd(Some("/Users/dev/.hark"), data));
        assert!(is_machinery_cwd(Some("/Users/dev/.hark/sub"), data));
        assert!(!is_machinery_cwd(Some("/Users/dev/Projects/web"), data));
        assert!(!is_machinery_cwd(Some("/Users/dev/.harkness"), data));
        assert!(!is_machinery_cwd(None, data));
    }

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

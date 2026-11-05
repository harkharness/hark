//! Pure aggregation of session events into the summaries the prompt uses.

use hark_agent::SessionEvent;

/// Display cap: how many of the newest prompts the PROMPT RENDER shows.
/// The index itself stores every prompt (topic search needs full history).
pub const MAX_RECENT_PROMPTS: usize = 5;

/// How much of the agent's last reply the index keeps: enough for a
/// closing summary ("done X, pending Y"), not a transcript.
pub const LAST_REPLY_CHARS: usize = 1500;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecentPrompt {
    pub ts: String,
    pub text: String,
}

/// Everything the prompt builder needs to describe one session.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SessionSummary {
    pub session_id: String,
    pub cwd: Option<String>,
    pub git_branch: Option<String>,
    pub title: Option<String>,
    /// `title` came from the user (a rename), not from a model or the
    /// first prompt: a generated name must not replace it.
    pub titled_by_user: bool,
    pub last_prompt: Option<String>,
    pub last_ts: Option<String>,
    pub recent_prompts: Vec<RecentPrompt>,
    /// The agent's newest prose: what it said last is the session's status.
    pub last_reply: Option<RecentPrompt>,
}

/// Clip to `max` chars, marking the cut.
fn clip(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let mut out: String = text.chars().take(max).collect();
    out.push('…');
    out
}

/// A name out of an opening prompt: its first non-empty line, clipped.
fn opening_title(text: &str) -> Option<String> {
    let line = text.lines().map(str::trim).find(|l| !l.is_empty())?;
    let clipped: String = line.chars().take(60).collect();
    Some(if line.chars().count() > 60 {
        format!("{clipped}…")
    } else {
        clipped
    })
}

impl SessionSummary {
    pub fn new(session_id: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
            ..Self::default()
        }
    }

    /// Fold one event into the summary, returning the updated summary.
    /// ISO-8601 timestamps compare correctly as strings, so `max` keeps
    /// `last_ts` monotonic without date parsing.
    pub fn apply(self, event: SessionEvent) -> Self {
        match event {
            SessionEvent::UserPrompt {
                ts,
                text,
                cwd,
                git_branch,
            } => {
                let recent_prompts: Vec<_> = self
                    .recent_prompts
                    .into_iter()
                    .chain(std::iter::once(RecentPrompt {
                        ts: ts.clone(),
                        text: text.clone(),
                    }))
                    .collect();
                // Most sessions never receive a custom-title: the CLI names
                // them after the opening line, and so do we. An unnamed
                // session cannot be recognised when you go looking for it.
                let title = self.title.or_else(|| opening_title(&text));
                Self {
                    last_ts: self.last_ts.max(Some(ts)),
                    last_prompt: Some(text),
                    cwd: cwd.or(self.cwd),
                    git_branch: git_branch.or(self.git_branch),
                    recent_prompts,
                    title,
                    ..self
                }
            }
            // Usage/compaction events only refresh the activity timestamp
            // here; their token payload feeds the spend ledger separately.
            SessionEvent::Activity { ts }
            | SessionEvent::AssistantUsage { ts, .. }
            | SessionEvent::CompactBoundary { ts, .. } => Self {
                last_ts: self.last_ts.max(Some(ts)),
                ..self
            },
            SessionEvent::AssistantText { ts, text } => Self {
                last_ts: self.last_ts.clone().max(Some(ts.clone())),
                last_reply: Some(RecentPrompt {
                    ts,
                    text: clip(&text, LAST_REPLY_CHARS),
                }),
                ..self
            },
            SessionEvent::Title(title) => Self {
                title: Some(title),
                titled_by_user: true,
                ..self
            },
            // A model's name fills the gap and replaces an older model's
            // name; a name the user typed is never overwritten.
            SessionEvent::GeneratedTitle(title) => Self {
                title: if self.titled_by_user { self.title } else { Some(title) },
                ..self
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hark_agent::SessionEvent;

    fn prompt(ts: &str, text: &str) -> SessionEvent {
        SessionEvent::UserPrompt {
            ts: ts.into(),
            text: text.into(),
            cwd: Some("/home/dev/proj".into()),
            git_branch: Some("main".into()),
        }
    }

    #[test]
    fn keeps_the_last_words_of_the_agent_as_the_reply() {
        let s = vec![
            prompt("2026-08-14T10:00:00.000Z", "start the migration"),
            SessionEvent::AssistantText { ts: "2026-08-14T10:01:00.000Z".into(), text: "Looking at the values.".into() },
            SessionEvent::AssistantText { ts: "2026-08-14T10:09:00.000Z".into(), text: "Done: values migrated, PR open.".into() },
        ]
        .into_iter()
        .fold(SessionSummary::new("abc"), SessionSummary::apply);
        assert_eq!(
            s.last_reply,
            Some(RecentPrompt { ts: "2026-08-14T10:09:00.000Z".into(), text: "Done: values migrated, PR open.".into() })
        );
        assert_eq!(s.last_ts.as_deref(), Some("2026-08-14T10:09:00.000Z"));

        // A reply is clipped at the fold: the index is not a transcript.
        let long = "x".repeat(LAST_REPLY_CHARS + 50);
        let s = SessionSummary::new("abc").apply(SessionEvent::AssistantText { ts: "t".into(), text: long });
        assert_eq!(s.last_reply.unwrap().text.chars().count(), LAST_REPLY_CHARS + 1, "clipped plus the ellipsis");
    }

    #[test]
    fn folds_events_into_summary() {
        let events = vec![
            prompt("2026-08-14T10:00:00.000Z", "start the migration"),
            SessionEvent::Activity {
                ts: "2026-08-14T10:05:00.000Z".into(),
            },
            SessionEvent::Title("Webhook migration".into()),
            prompt("2026-08-14T10:10:00.000Z", "now open the PR"),
        ];
        let s = events
            .into_iter()
            .fold(SessionSummary::new("abc"), SessionSummary::apply);

        assert_eq!(s.session_id, "abc");
        assert_eq!(s.title.as_deref(), Some("Webhook migration"));
        assert_eq!(s.cwd.as_deref(), Some("/home/dev/proj"));
        assert_eq!(s.git_branch.as_deref(), Some("main"));
        assert_eq!(s.last_prompt.as_deref(), Some("now open the PR"));
        assert_eq!(s.last_ts.as_deref(), Some("2026-08-14T10:10:00.000Z"));
        assert_eq!(s.recent_prompts.len(), 2);
    }

    #[test]
    fn a_generated_title_names_a_nameless_session_but_never_over_the_users() {
        // The CLI (or Hark) writes a generated name: better than the first
        // sixty characters of the opening message, worse than a name the
        // user typed — whichever order the lines arrive in.
        let generated_only = vec![
            prompt("2026-08-14T10:00:00.000Z", "Alerts migration to the new team"),
            SessionEvent::GeneratedTitle("Alerts migration".into()),
        ]
        .into_iter()
        .fold(SessionSummary::new("abc"), SessionSummary::apply);
        assert_eq!(generated_only.title.as_deref(), Some("Alerts migration"));

        let user_first = vec![
            SessionEvent::Title("Cluster alerts".into()),
            SessionEvent::GeneratedTitle("Alerts migration".into()),
        ]
        .into_iter()
        .fold(SessionSummary::new("abc"), SessionSummary::apply);
        assert_eq!(user_first.title.as_deref(), Some("Cluster alerts"));

        let user_last = vec![
            SessionEvent::GeneratedTitle("Alerts migration".into()),
            SessionEvent::Title("Cluster alerts".into()),
        ]
        .into_iter()
        .fold(SessionSummary::new("abc"), SessionSummary::apply);
        assert_eq!(user_last.title.as_deref(), Some("Cluster alerts"));

        // A newer generated name replaces an older generated one (the CLI
        // keeps the last), but never a user's.
        let regenerated = vec![
            SessionEvent::GeneratedTitle("First guess".into()),
            SessionEvent::GeneratedTitle("Better guess".into()),
        ]
        .into_iter()
        .fold(SessionSummary::new("abc"), SessionSummary::apply);
        assert_eq!(regenerated.title.as_deref(), Some("Better guess"));
    }

    #[test]
    fn names_untitled_sessions_after_their_first_prompt() {
        // Most sessions never get a custom-title: the CLI names them after
        // the opening message, and so must we — an unnamed session cannot
        // be recognised in a recovery list.
        let s = vec![
            prompt("2026-08-14T10:00:00.000Z", "Alerts migration to the new team"),
            prompt("2026-08-14T10:10:00.000Z", "now open the PR"),
        ]
        .into_iter()
        .fold(SessionSummary::new("abc"), SessionSummary::apply);
        assert_eq!(s.title.as_deref(), Some("Alerts migration to the new team"));

        // A real title always wins, whenever it arrives.
        let titled = vec![
            prompt("2026-08-14T10:00:00.000Z", "Alerts migration to the new team"),
            SessionEvent::Title("Cluster alerts".into()),
        ]
        .into_iter()
        .fold(SessionSummary::new("abc"), SessionSummary::apply);
        assert_eq!(titled.title.as_deref(), Some("Cluster alerts"));
    }

    #[test]
    fn keeps_every_prompt_for_topic_search() {
        let events: Vec<_> = (0..10)
            .map(|i| prompt(&format!("2026-08-14T10:0{i}:00.000Z"), &format!("p{i}")))
            .collect();
        let s = events
            .into_iter()
            .fold(SessionSummary::new("abc"), SessionSummary::apply);

        // The fold no longer caps; MAX_RECENT_PROMPTS is a render-time cap.
        assert_eq!(s.recent_prompts.len(), 10);
        assert_eq!(s.recent_prompts.last().map(|p| p.text.as_str()), Some("p9"));
    }

    #[test]
    fn activity_only_advances_timestamp_forward() {
        let s = SessionSummary::new("abc")
            .apply(prompt("2026-08-14T10:10:00.000Z", "hello"))
            .apply(SessionEvent::Activity {
                ts: "2026-08-14T09:00:00.000Z".into(),
            });
        // An older activity line must not move last_ts backwards.
        assert_eq!(s.last_ts.as_deref(), Some("2026-08-14T10:10:00.000Z"));
    }
}

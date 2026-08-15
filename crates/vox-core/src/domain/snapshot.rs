//! Pure aggregation of session events into the summaries the prompt uses.

use crate::domain::session_log::SessionEvent;

/// Display cap: how many of the newest prompts the PROMPT RENDER shows.
/// The index itself stores every prompt (topic search needs full history).
pub const MAX_RECENT_PROMPTS: usize = 5;

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
    pub last_prompt: Option<String>,
    pub last_ts: Option<String>,
    pub recent_prompts: Vec<RecentPrompt>,
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
                Self {
                    last_ts: self.last_ts.max(Some(ts)),
                    last_prompt: Some(text),
                    cwd: cwd.or(self.cwd),
                    git_branch: git_branch.or(self.git_branch),
                    recent_prompts,
                    ..self
                }
            }
            SessionEvent::Activity { ts } => Self {
                last_ts: self.last_ts.max(Some(ts)),
                ..self
            },
            SessionEvent::Title(title) => Self {
                title: Some(title),
                ..self
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::session_log::SessionEvent;

    fn prompt(ts: &str, text: &str) -> SessionEvent {
        SessionEvent::UserPrompt {
            ts: ts.into(),
            text: text.into(),
            cwd: Some("/home/dev/proj".into()),
            git_branch: Some("main".into()),
        }
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

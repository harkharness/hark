//! Where a message lands: the scope chip of the single window.
//!
//! One window, one composer, many chats. The scope says what the composer
//! is pointed at: every project (nothing selected), one project, one chat
//! (a session), or one artifact page. It replaces "which window is focused"
//! as the thing voice and keyboard both consult.

use super::project::Project;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Scope {
    /// Nothing selected: questions go to the local index, work is resolved
    /// by the funnel over every project.
    All,
    /// A project is selected but no chat: work opens a fresh chat there.
    Project { name: String, path: String },
    /// A chat is open: work goes there, no resolution needed.
    Chat {
        title: String,
        session_id: Option<String>,
        project: Option<Project>,
    },
    /// An artifact page is open: it rides along in the prompt.
    Page {
        title: String,
        path: String,
        space: String,
    },
}

impl Scope {
    /// The chat the funnel treats as "on screen": only a chat that already
    /// has a session can receive work without resolution.
    pub fn active(&self) -> Option<(&str, &str)> {
        match self {
            Scope::Chat { title, session_id: Some(sid), .. } => Some((title.as_str(), sid.as_str())),
            _ => None,
        }
    }

    /// The project a Work plan binds to: (name, path).
    pub fn active_project(&self) -> Option<(&str, &str)> {
        match self {
            Scope::Project { name, path } => Some((name.as_str(), path.as_str())),
            Scope::Chat { project: Some(p), .. } => Some((p.name.as_str(), p.path.as_str())),
            _ => None,
        }
    }

    /// Where files, shells and `@` mentions open. A page has no project of
    /// its own: it belongs to a space.
    pub fn project_path(&self) -> Option<&str> {
        self.active_project().map(|(_, path)| path)
    }

    /// What the chip and the title bar print, outermost first. Empty for
    /// "everything": nobody is in command, so nothing is named.
    pub fn breadcrumb(&self) -> Vec<&str> {
        match self {
            Scope::All => vec![],
            Scope::Project { name, .. } => vec![name.as_str()],
            Scope::Chat { title, project, .. } => {
                let mut v: Vec<&str> = project.iter().map(|p| p.name.as_str()).collect();
                v.push(title.as_str());
                v
            }
            Scope::Page { title, space, .. } => vec![space.as_str(), title.as_str()],
        }
    }

    /// The window query of today (`?project=&name=&task=&session=`) read as
    /// a scope, so the project window keeps opening exactly as before.
    pub fn from_query(pairs: &[(&str, &str)]) -> Scope {
        let get = |k: &str| pairs.iter().find(|(key, _)| *key == k).map(|(_, v)| *v).filter(|v| !v.is_empty());
        let project = get("project").map(|path| Project {
            name: get("name").map(str::to_string).unwrap_or_else(|| super::project::derive_name(path)),
            path: path.to_string(),
        });
        match (get("task"), project) {
            (Some(title), project) => Scope::Chat {
                title: title.to_string(),
                session_id: get("session").map(str::to_string),
                project,
            },
            (None, Some(p)) => Scope::Project { name: p.name, path: p.path },
            (None, None) => Scope::All,
        }
    }
}

/// The colour slot of the project a path belongs to: the project's position
/// in the registry, stable for as long as the registry order holds. A path
/// is inside a project when it IS the project or sits under it; a sibling
/// that merely shares the prefix is not.
pub fn colour_index(path: &str, projects: &[Project]) -> Option<usize> {
    if path.is_empty() {
        return None;
    }
    projects.iter().position(|p| {
        let root = p.path.trim_end_matches('/');
        path == root || path.strip_prefix(root).is_some_and(|rest| rest.starts_with('/'))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::funnel::{plan, Plan, SessionLead, World};

    fn hark() -> Project {
        Project { name: "hark".into(), path: "/Users/x/Projects/hark".into() }
    }

    #[test]
    fn a_chat_scope_is_the_funnel_s_active_chat() {
        let scope = Scope::Chat {
            title: "Billing endpoint v2".into(),
            session_id: Some("s-bill".into()),
            project: Some(hark()),
        };
        let search = |_: &str| Vec::<SessionLead>::new();
        let projects = vec![hark()];
        let world = World {
            active: scope.active(),
            active_project: scope.active_project(),
            board: &[],
            projects: &projects,
            search: &search,
        };
        match plan("aplica a proposta que a gente discutiu", &world) {
            Plan::Work { session_id, confidence, workspace, .. } => {
                assert_eq!(session_id.as_deref(), Some("s-bill"));
                assert_eq!(confidence, "high");
                assert_eq!(workspace.as_deref(), Some("/Users/x/Projects/hark"));
            }
            other => panic!("expected work on the open chat, got {other:?}"),
        }
    }

    #[test]
    fn the_all_scope_has_no_active_chat_so_work_is_resolved_by_search() {
        let scope = Scope::All;
        assert_eq!(scope.active(), None);
        assert_eq!(scope.active_project(), None);
        let hit = SessionLead {
            session_id: "s-retry".into(),
            title: "Retry queue fix".into(),
            cwd: Some("/Users/x/Projects/hark".into()),
            last_ts: None,
            last_prompt: None,
        };
        let search = move |_: &str| vec![hit.clone()];
        let projects = vec![hark()];
        let world = World { active: None, active_project: None, board: &[], projects: &projects, search: &search };
        match plan("continua o retry queue fix", &world) {
            Plan::Work { session_id, confidence, .. } => {
                assert_eq!(session_id.as_deref(), Some("s-retry"));
                assert_eq!(confidence, "low", "a search hit still asks for a yes");
            }
            other => panic!("expected search-resolved work, got {other:?}"),
        }
    }

    #[test]
    fn a_chat_without_a_session_yet_is_not_an_active_chat() {
        let scope = Scope::Chat { title: "Nova".into(), session_id: None, project: Some(hark()) };
        assert_eq!(scope.active(), None);
        assert_eq!(scope.active_project(), Some(("hark", "/Users/x/Projects/hark")));
    }

    #[test]
    fn the_window_url_of_today_reads_as_a_scope() {
        let q = |s: &str| Scope::from_query(&s.split('&').filter_map(|kv| kv.split_once('=')).collect::<Vec<_>>());
        assert_eq!(q(""), Scope::All);
        assert_eq!(
            q("project=/Users/x/Projects/hark&name=hark"),
            Scope::Project { name: "hark".into(), path: "/Users/x/Projects/hark".into() }
        );
        assert_eq!(
            q("project=/Users/x/Projects/hark&name=hark&task=Billing endpoint v2&session=s-bill"),
            Scope::Chat {
                title: "Billing endpoint v2".into(),
                session_id: Some("s-bill".into()),
                project: Some(hark()),
            }
        );
        // A task without a project is still a chat: the window will find
        // the project from the session's cwd, as it does today.
        assert_eq!(
            q("task=Alerts cleanup"),
            Scope::Chat { title: "Alerts cleanup".into(), session_id: None, project: None }
        );
    }

    #[test]
    fn the_breadcrumb_is_what_the_chip_and_the_title_bar_print() {
        assert!(Scope::All.breadcrumb().is_empty());
        assert_eq!(
            Scope::Project { name: "hark".into(), path: "/p".into() }.breadcrumb(),
            vec!["hark"]
        );
        assert_eq!(
            Scope::Chat { title: "Billing endpoint v2".into(), session_id: None, project: Some(hark()) }.breadcrumb(),
            vec!["hark", "Billing endpoint v2"]
        );
        assert_eq!(
            Scope::Chat { title: "Solta".into(), session_id: None, project: None }.breadcrumb(),
            vec!["Solta"]
        );
        assert_eq!(
            Scope::Page { title: "ADR 004".into(), path: "decisoes/adr-004.md".into(), space: "hark".into() }.breadcrumb(),
            vec!["hark", "ADR 004"]
        );
    }

    #[test]
    fn the_project_path_of_a_scope_is_where_files_and_shells_open() {
        assert_eq!(Scope::All.project_path(), None);
        assert_eq!(Scope::Project { name: "hark".into(), path: "/p".into() }.project_path(), Some("/p"));
        assert_eq!(
            Scope::Chat { title: "t".into(), session_id: None, project: Some(hark()) }.project_path(),
            Some("/Users/x/Projects/hark")
        );
        assert_eq!(Scope::Page { title: "t".into(), path: "a.md".into(), space: "hark".into() }.project_path(), None);
    }

    #[test]
    fn a_project_colour_follows_registration_order_and_covers_subpaths() {
        let projects = vec![
            hark(),
            Project { name: "landing".into(), path: "/Users/x/Projects/landing".into() },
        ];
        assert_eq!(colour_index("/Users/x/Projects/hark", &projects), Some(0));
        assert_eq!(colour_index("/Users/x/Projects/landing/src", &projects), Some(1));
        assert_eq!(colour_index("/Users/x/Projects/hark-landing", &projects), None, "a sibling with the same prefix is not inside");
        assert_eq!(colour_index("/elsewhere", &projects), None);
        assert_eq!(colour_index("", &projects), None, "no workspace, no colour");
    }
}

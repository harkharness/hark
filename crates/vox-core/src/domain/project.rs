//! Projects: the directories Vox works in. Each project groups the chats
//! (tasks) that ran inside it, exactly like directory groups in a session
//! sidebar. The list is user-editable (click or voice) and lives in the
//! global state file.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Project {
    /// Short display name (last path component by default).
    pub name: String,
    /// Absolute or `~/`-relative directory path.
    pub path: String,
}

/// Display name for a directory path: its last non-empty component.
pub fn derive_name(path: &str) -> String {
    path.trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or(path)
        .to_string()
}

/// Turn a SPOKEN path into a filesystem candidate. Speech has no slashes:
/// "home projects vox" means `~/projects/vox` (macOS paths are
/// case-insensitive, so lowercase is fine). Typed paths pass through.
pub fn path_from_speech(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.contains('/') || trimmed.starts_with('~') {
        return trimmed.to_string();
    }
    let tokens: Vec<&str> = trimmed.split_whitespace().collect();
    match tokens.split_first() {
        Some((first, rest)) if first.eq_ignore_ascii_case("home") => {
            format!("~/{}", rest.join("/").to_lowercase())
        }
        Some(_) => format!("~/{}", tokens.join("/").to_lowercase()),
        None => String::new(),
    }
}

/// Add a project to the list, deduplicating by path. Returns the final list
/// and the entry that now represents this path.
pub fn add(mut projects: Vec<Project>, name: &str, path: &str) -> (Vec<Project>, Project) {
    if let Some(existing) = projects.iter().find(|p| p.path == path) {
        let found = existing.clone();
        return (projects, found);
    }
    let entry = Project {
        name: name.to_string(),
        path: path.to_string(),
    };
    projects.push(entry.clone());
    (projects, entry)
}

/// Remove a project by path or name.
pub fn remove(projects: Vec<Project>, key: &str) -> Vec<Project> {
    projects
        .into_iter()
        .filter(|p| p.path != key && p.name != key)
        .collect()
}

/// Find a project by (partial, case-insensitive) name.
pub fn find<'a>(projects: &'a [Project], query: &str) -> Option<&'a Project> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return None;
    }
    projects
        .iter()
        .find(|p| p.name.to_lowercase() == q)
        .or_else(|| projects.iter().find(|p| p.name.to_lowercase().contains(&q)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derives_name_from_last_component() {
        assert_eq!(derive_name("~/Projects/vox"), "vox");
        assert_eq!(derive_name("/a/b/c/"), "c");
        assert_eq!(derive_name("solo"), "solo");
    }

    #[test]
    fn spoken_paths_become_home_relative() {
        assert_eq!(path_from_speech("home projects vox"), "~/projects/vox");
        assert_eq!(path_from_speech("Projects workspace-fabrica"), "~/projects/workspace-fabrica");
        // Typed paths pass through untouched.
        assert_eq!(path_from_speech("~/Projects/vox"), "~/Projects/vox");
        assert_eq!(path_from_speech("/abs/dir"), "/abs/dir");
    }

    #[test]
    fn add_dedups_by_path_and_remove_accepts_name_or_path() {
        let (list, first) = add(vec![], "vox", "/p/vox");
        let (list, again) = add(list, "vox-dup", "/p/vox");
        assert_eq!(list.len(), 1);
        assert_eq!(first, again);

        let (list, _) = add(list, "other", "/p/other");
        assert_eq!(remove(list.clone(), "other").len(), 1);
        assert_eq!(remove(list, "/p/vox").len(), 1);
    }

    #[test]
    fn finds_projects_by_partial_name() {
        let projects = vec![
            Project { name: "vox".into(), path: "/p/vox".into() },
            Project { name: "workspace-fabrica".into(), path: "/p/wu".into() },
        ];
        assert_eq!(find(&projects, "vox").unwrap().path, "/p/vox");
        assert_eq!(find(&projects, "fabrica").unwrap().path, "/p/wu");
        assert!(find(&projects, "nope").is_none());
        assert!(find(&projects, "").is_none());
    }
}

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
    // "barra" is how a person SAYS the slash — it separates, never names.
    let tokens: Vec<&str> = trimmed
        .split_whitespace()
        .filter(|w| !w.eq_ignore_ascii_case("barra"))
        .collect();
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

/// A project seen as a snapshot scope: sessions whose cwd lives under the
/// project directory, repos = the directory itself. This is what replaced
/// the manual kubectl-style context select in the window.
pub fn as_context(project: &Project) -> crate::domain::context::ContextDef {
    crate::domain::context::ContextDef {
        name: project.name.clone(),
        match_cwd: vec![project.path.clone()],
        repos: vec![project.path.clone()],
    }
}

/// Speech-friendly normalization: lowercase, pt-BR accents stripped, and
/// `-`/`_`/`.` become spaces so "workspace código" equals "workspace-codigo".
fn normalize(text: &str) -> String {
    text.chars()
        .map(|c| match c.to_lowercase().next().unwrap_or(c) {
            'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            '-' | '_' | '.' => ' ',
            other => other,
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Find a project by (partial, accent/separator-insensitive) name.
pub fn find<'a>(projects: &'a [Project], query: &str) -> Option<&'a Project> {
    let q = normalize(query);
    if q.is_empty() {
        return None;
    }
    projects
        .iter()
        .find(|p| normalize(&p.name) == q)
        .or_else(|| projects.iter().find(|p| normalize(&p.name).contains(&q)))
}

/// Find a project whose name appears (as whole words) anywhere in a spoken
/// sentence: "ok, então abra workspace código" hits `workspace-codigo`.
/// Longest name wins when several match; unknown names never guess.
pub fn find_spoken<'a>(projects: &'a [Project], utterance: &str) -> Option<&'a Project> {
    let spoken: Vec<String> = normalize(utterance)
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_string)
        .collect();
    projects
        .iter()
        .filter_map(|p| {
            let name: Vec<String> =
                normalize(&p.name).split(' ').map(str::to_string).collect();
            let hit = !name.is_empty() && spoken.windows(name.len()).any(|w| w == name);
            hit.then_some((name.len(), p))
        })
        .max_by_key(|(len, p)| (*len, p.name.len()))
        .map(|(_, p)| p)
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
        assert_eq!(path_from_speech("Projects workspace-codigo"), "~/projects/workspace-codigo");
        // Typed paths pass through untouched.
        assert_eq!(path_from_speech("~/Projects/vox"), "~/Projects/vox");
        assert_eq!(path_from_speech("/abs/dir"), "/abs/dir");
    }

    #[test]
    fn spoken_barra_is_a_separator_not_a_directory() {
        // "Projects barra workspace" is how a person SAYS the slash.
        assert_eq!(path_from_speech("Projects barra workspace"), "~/projects/workspace");
        assert_eq!(path_from_speech("home barra projects barra vox"), "~/projects/vox");
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
    fn projects_scope_like_contexts() {
        let p = Project { name: "vox".into(), path: "/p/vox".into() };
        let ctx = as_context(&p);
        assert_eq!(ctx.name, "vox");
        assert!(ctx.matches(Some("/p/vox")));
        assert!(ctx.matches(Some("/p/vox/src")));
        assert!(!ctx.matches(Some("/p/other")));
        assert_eq!(ctx.repos, vec!["/p/vox".to_string()]);
    }

    #[test]
    fn finds_projects_by_partial_name() {
        let projects = vec![
            Project { name: "vox".into(), path: "/p/vox".into() },
            Project { name: "workspace-codigo".into(), path: "/p/wu".into() },
        ];
        assert_eq!(find(&projects, "vox").unwrap().path, "/p/vox");
        assert_eq!(find(&projects, "codigo").unwrap().path, "/p/wu");
        assert!(find(&projects, "nope").is_none());
        assert!(find(&projects, "").is_none());
    }

    #[test]
    fn finds_projects_ignoring_accents_and_separators() {
        // STT writes natural Portuguese ("workspace código"); names on disk
        // use hyphens and no accents. Both sides must normalize.
        let projects = vec![
            Project { name: "workspace-codigo".into(), path: "/p/wu".into() },
            Project { name: "vox".into(), path: "/p/vox".into() },
        ];
        assert_eq!(find(&projects, "workspace código").unwrap().path, "/p/wu");
        assert_eq!(find(&projects, "Workspace Código").unwrap().path, "/p/wu");
        assert_eq!(find(&projects, "código").unwrap().path, "/p/wu");
    }

    #[test]
    fn spots_project_names_inside_sentences() {
        let projects = vec![
            Project { name: "workspace-codigo".into(), path: "/p/wu".into() },
            Project { name: "vox".into(), path: "/p/vox".into() },
        ];
        assert_eq!(
            find_spoken(&projects, "ok, então abra workspace código").unwrap().path,
            "/p/wu"
        );
        assert_eq!(find_spoken(&projects, "abre o vox aí").unwrap().path, "/p/vox");
        // No registered name in the sentence: never guess.
        assert!(find_spoken(&projects, "abre o PR do DNS antigo").is_none());
        // Partial words don't count ("voxel" is not "vox").
        assert!(find_spoken(&projects, "abre o voxel").is_none());
    }

    #[test]
    fn spoken_match_prefers_the_longest_name() {
        let projects = vec![
            Project { name: "vox".into(), path: "/p/vox".into() },
            Project { name: "vox-docs".into(), path: "/p/voxdocs".into() },
        ];
        assert_eq!(
            find_spoken(&projects, "abre o vox docs").unwrap().path,
            "/p/voxdocs"
        );
    }
}

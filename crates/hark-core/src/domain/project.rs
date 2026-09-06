//! Projects: the directories Hark works in. Each project groups the chats
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
/// "home projects hark" means `~/projects/hark` (macOS paths are
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

/// Directories on disk that a spoken sentence plausibly names — the rescue
/// for "abra o Projects workspace" when nothing registered matches: instead
/// of a dead end ("cadastre o path"), the caller offers what IS there.
///
/// A dir qualifies when one of its name's words (split on -, _ and case
/// folds) is said as a whole word. Sentence glue and generic filler
/// ("projects", "abre", "o") never count, so a stopword-named directory
/// cannot hijack every sentence. Ranked by how many words hit; ties keep
/// input order. The caller registers a UNIQUE hit and offers the rest.
pub fn dirs_matching_speech(utterance: &str, dirs: &[&str]) -> Vec<String> {
    const GLUE: &[&str] = &[
        "abre", "abra", "abrir", "abre-me", "projeto", "projetos", "projects",
        "o", "a", "os", "as", "um", "uma", "de", "do", "da", "em", "no", "na",
        "novo", "nova", "pra", "para", "e",
    ];
    let spoken: Vec<String> = normalize(utterance)
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() > 1 && !GLUE.contains(w))
        .map(str::to_string)
        .collect();
    let mut hits: Vec<(usize, String)> = dirs
        .iter()
        .filter_map(|dir| {
            let words: Vec<String> = normalize(dir)
                .split(|c: char| !c.is_alphanumeric())
                .filter(|w| w.len() > 1 && !GLUE.contains(w))
                .map(str::to_string)
                .collect();
            let count = words.iter().filter(|w| spoken.contains(w)).count();
            (count > 0).then(|| (count, dir.to_string()))
        })
        .collect();
    hits.sort_by_key(|&(count, _)| std::cmp::Reverse(count));
    hits.into_iter().map(|(_, d)| d).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The Intel 25/08 dead end: "abra o Projects workspace" matched no
    /// REGISTERED project and the answer was "cadastre o path na mão" —
    /// with ~/Projects/workspace-fabrica sitting right there on disk.
    #[test]
    fn spoken_words_find_unregistered_directories() {
        let dirs = ["workspace-fabrica", "hark", "reliability-tools", "notas"];
        assert_eq!(
            dirs_matching_speech("abra o Projects workspace", &dirs),
            vec!["workspace-fabrica"]
        );
        assert_eq!(
            dirs_matching_speech("abre o reliability", &dirs),
            vec!["reliability-tools"]
        );
        // Accents and case fold like everywhere else.
        assert_eq!(dirs_matching_speech("abre a fábrica", &["workspace-fabrica"]), vec!["workspace-fabrica"]);
    }

    #[test]
    fn ambiguous_dirs_come_back_ranked_never_guessed() {
        let dirs = ["workspace-fabrica", "workspace-codigo", "hark"];
        let got = dirs_matching_speech("abra o workspace", &dirs);
        assert_eq!(got.len(), 2, "both candidates surface — the caller offers, never picks");
        assert!(got.contains(&"workspace-fabrica".to_string()));
        assert!(got.contains(&"workspace-codigo".to_string()));
    }

    #[test]
    fn glue_words_never_match_directories() {
        // "o", "projects", "abre" are sentence glue: a dir named "projects"
        // or a short stopword must not turn every sentence into an offer.
        let dirs = ["o-liveiro", "projects", "de-para"];
        assert!(dirs_matching_speech("abre o Projects workspace", &dirs).is_empty());
        assert!(dirs_matching_speech("abre alguma coisa", &dirs).is_empty());
    }

    #[test]
    fn derives_name_from_last_component() {
        assert_eq!(derive_name("~/Projects/hark"), "hark");
        assert_eq!(derive_name("/a/b/c/"), "c");
        assert_eq!(derive_name("solo"), "solo");
    }

    #[test]
    fn spoken_paths_become_home_relative() {
        assert_eq!(path_from_speech("home projects hark"), "~/projects/hark");
        assert_eq!(path_from_speech("Projects workspace-codigo"), "~/projects/workspace-codigo");
        // Typed paths pass through untouched.
        assert_eq!(path_from_speech("~/Projects/hark"), "~/Projects/hark");
        assert_eq!(path_from_speech("/abs/dir"), "/abs/dir");
    }

    #[test]
    fn spoken_barra_is_a_separator_not_a_directory() {
        // "Projects barra workspace" is how a person SAYS the slash.
        assert_eq!(path_from_speech("Projects barra workspace"), "~/projects/workspace");
        assert_eq!(path_from_speech("home barra projects barra hark"), "~/projects/hark");
    }

    #[test]
    fn add_dedups_by_path_and_remove_accepts_name_or_path() {
        let (list, first) = add(vec![], "hark", "/p/hark");
        let (list, again) = add(list, "hark-dup", "/p/hark");
        assert_eq!(list.len(), 1);
        assert_eq!(first, again);

        let (list, _) = add(list, "other", "/p/other");
        assert_eq!(remove(list.clone(), "other").len(), 1);
        assert_eq!(remove(list, "/p/hark").len(), 1);
    }

    #[test]
    fn projects_scope_like_contexts() {
        let p = Project { name: "hark".into(), path: "/p/hark".into() };
        let ctx = as_context(&p);
        assert_eq!(ctx.name, "hark");
        assert!(ctx.matches(Some("/p/hark")));
        assert!(ctx.matches(Some("/p/hark/src")));
        assert!(!ctx.matches(Some("/p/other")));
        assert_eq!(ctx.repos, vec!["/p/hark".to_string()]);
    }

    #[test]
    fn finds_projects_by_partial_name() {
        let projects = vec![
            Project { name: "hark".into(), path: "/p/hark".into() },
            Project { name: "workspace-codigo".into(), path: "/p/wu".into() },
        ];
        assert_eq!(find(&projects, "hark").unwrap().path, "/p/hark");
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
            Project { name: "hark".into(), path: "/p/hark".into() },
        ];
        assert_eq!(find(&projects, "workspace código").unwrap().path, "/p/wu");
        assert_eq!(find(&projects, "Workspace Código").unwrap().path, "/p/wu");
        assert_eq!(find(&projects, "código").unwrap().path, "/p/wu");
    }

    #[test]
    fn spots_project_names_inside_sentences() {
        let projects = vec![
            Project { name: "workspace-codigo".into(), path: "/p/wu".into() },
            Project { name: "hark".into(), path: "/p/hark".into() },
        ];
        assert_eq!(
            find_spoken(&projects, "ok, então abra workspace código").unwrap().path,
            "/p/wu"
        );
        assert_eq!(find_spoken(&projects, "abre o hark aí").unwrap().path, "/p/hark");
        // No registered name in the sentence: never guess.
        assert!(find_spoken(&projects, "abre o PR do DNS antigo").is_none());
        // Partial words don't count ("voxel" is not "hark").
        assert!(find_spoken(&projects, "abre o voxel").is_none());
    }

    #[test]
    fn spoken_match_prefers_the_longest_name() {
        let projects = vec![
            Project { name: "hark".into(), path: "/p/hark".into() },
            Project { name: "hark-docs".into(), path: "/p/voxdocs".into() },
        ];
        assert_eq!(
            find_spoken(&projects, "abre o hark docs").unwrap().path,
            "/p/voxdocs"
        );
    }
}

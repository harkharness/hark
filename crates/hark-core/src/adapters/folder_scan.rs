//! Reads the folders the "add a project" field offers while a path is
//! typed (`domain::folder_complete` decides what to ask and what comes
//! first). Read-only, names only, never a file's contents.

use crate::domain::folder_complete::{self, Candidate, Query};
use serde::Serialize;
use std::path::{Path, PathBuf};

/// Never searched by name: dependency and build output.
const SKIP_DIRS: &[&str] = &[
    "node_modules", "target", "dist", "build", "vendor", "__pycache__", ".venv",
];

/// The personal folders of a home directory. A desktop asks the person
/// for permission the moment an app looks inside one, so a search by name
/// stays out of them, and they are not probed for a repository. Typing
/// the path into one is the person's own choice and lists it normally.
const PERSONAL_DIRS: &[&str] = &[
    "Desktop", "Documents", "Downloads", "Library", "Movies", "Music", "Pictures",
    "Public", "Videos", "Applications",
];

/// How far below a root a bare name is looked for: far enough for an app
/// inside a monorepo (`root/repo/apps/name`), the budget keeps it short.
const NAME_DEPTH: usize = 3;
/// Folders visited in one search by name, across every root.
const VISIT_BUDGET: usize = 4000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FolderHit {
    pub name: String,
    /// The absolute path, what `project_add` takes.
    pub path: String,
    /// The same path as the field shows it, home as `~`.
    pub shown: String,
    pub git: bool,
    /// Already a project: set by the caller, who knows the list.
    pub known: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Completion {
    pub hits: Vec<FolderHit>,
    /// What was typed is itself a folder: Enter takes this, whatever the
    /// spelling in the field ("name/" is wherever the name was found).
    pub folder: Option<FolderHit>,
}

/// The folders to offer for `typed`. `roots` are where a bare name is
/// looked for (the parents of the projects already registered); home is
/// always one of them.
pub fn complete(typed: &str, home: &Path, roots: &[PathBuf], limit: usize) -> Completion {
    let home_str = home.display().to_string();
    let hit = |c: Candidate| FolderHit {
        shown: folder_complete::tilde(&c.path, &home_str),
        name: c.name,
        path: c.path,
        git: c.git,
        known: false,
    };
    // Every folder a search by name can reach, across the roots and home.
    let reachable = || {
        let mut found = Vec::new();
        let mut budget = VISIT_BUDGET;
        for root in roots.iter().map(PathBuf::as_path).chain([home]) {
            walk(root, home, &mut found, &mut budget);
        }
        found
    };
    match folder_complete::parse(typed) {
        Query::Path { dir, prefix } => {
            let dir = expand(&dir, home);
            let hits = folder_complete::rank_names(&subdirs(&dir), &prefix, limit)
                .into_iter()
                .map(|name| hit(candidate(&dir, name, 1, home)))
                .collect();
            let folder = itself(&dir, &prefix, home).map(hit);
            Completion { hits, folder }
        }
        Query::Under { head, rest, prefix } => {
            // Under home when it is there, as a path typed without the
            // tilde; else wherever a folder of that name was found.
            let at_home = home.join(&head);
            let bases: Vec<PathBuf> = if at_home.is_dir() {
                vec![at_home]
            } else {
                folder_complete::heads(reachable(), &head)
                    .into_iter()
                    .map(|c| PathBuf::from(c.path))
                    .collect()
            };
            let mut hits = Vec::new();
            let mut folder = None;
            for base in &bases {
                let dir = base.join(&rest);
                if folder.is_none() {
                    folder = itself(&dir, &prefix, home).map(hit);
                }
                let room = limit.saturating_sub(hits.len());
                hits.extend(
                    folder_complete::rank_names(&subdirs(&dir), &prefix, room)
                        .into_iter()
                        .map(|name| hit(candidate(&dir, name, 1, home))),
                );
            }
            Completion { hits, folder }
        }
        Query::Name(text) => {
            let hits = folder_complete::rank_found(reachable(), &text, limit).into_iter().map(hit).collect();
            Completion { hits, folder: None }
        }
    }
}

/// What was typed, as a folder of its own, when it is one: what Enter
/// takes when the text in the field already names a folder.
fn itself(dir: &Path, prefix: &str, home: &Path) -> Option<Candidate> {
    // A dot is the start of a hidden name, or a way up: a fragment.
    if prefix == "." || prefix == ".." {
        return None;
    }
    // Rebuilt from its parts: a trailing separator is not part of a name.
    let path: PathBuf = dir.join(prefix).components().collect();
    if !path.is_dir() {
        return None;
    }
    let name = path.file_name()?.to_string_lossy().to_string();
    Some(candidate(path.parent()?, name, 1, home))
}

/// `~` at the start is the home given, not the process's: the caller says
/// whose home it is.
fn expand(path: &str, home: &Path) -> PathBuf {
    match path.strip_prefix('~') {
        Some(rest) => home.join(rest.trim_start_matches(['/', '\\'])),
        None => PathBuf::from(path),
    }
}

/// The folders directly inside `dir`, by name. A link to a folder counts.
fn subdirs(dir: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
    entries
        .map_while(Result::ok)
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect()
}

fn personal(dir: &Path, name: &str, home: &Path) -> bool {
    dir == home && PERSONAL_DIRS.contains(&name)
}

fn candidate(dir: &Path, name: String, depth: usize, home: &Path) -> Candidate {
    let path = dir.join(&name);
    let git = !personal(dir, &name, home) && path.join(".git").exists();
    Candidate { name, path: path.display().to_string(), depth, git }
}

/// Every folder up to `NAME_DEPTH` levels under `root`, breadth first so
/// a tight budget is spent on the top levels.
fn walk(root: &Path, home: &Path, found: &mut Vec<Candidate>, budget: &mut usize) {
    let mut queue = std::collections::VecDeque::from([(root.to_path_buf(), 1usize)]);
    while let Some((dir, depth)) = queue.pop_front() {
        let mut names = subdirs(&dir);
        names.sort();
        for name in names {
            if *budget == 0 {
                return;
            }
            if name.starts_with('.') || SKIP_DIRS.contains(&name.as_str()) {
                continue;
            }
            *budget -= 1;
            if depth < NAME_DEPTH && !personal(&dir, &name, home) {
                queue.push_back((dir.join(&name), depth + 1));
            }
            found.push(candidate(&dir, name, depth, home));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// home/
    ///   work/ alpha(.git) alpha-old/ beta(.git)/inner/deep-alpha/too-deep-alpha notes.txt
    ///         node_modules/alpha-dep .hidden-alpha
    ///   Documents/ alpha-private(.git)
    ///   solo(.git)
    fn tree() -> tempfile::TempDir {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        for dir in [
            "work/alpha/.git",
            "work/alpha-old",
            "work/beta/.git",
            "work/beta/inner/deep-alpha/too-deep-alpha",
            "work/node_modules/alpha-dep",
            "work/.hidden-alpha",
            "Documents/alpha-private/.git",
            "solo/.git",
        ] {
            fs::create_dir_all(home.join(dir)).unwrap();
        }
        fs::write(home.join("work/notes.txt"), "x").unwrap();
        fs::write(home.join("work/alpha.txt"), "x").unwrap();
        tmp
    }

    fn shown(c: &Completion) -> Vec<&str> {
        c.hits.iter().map(|h| h.shown.as_str()).collect()
    }

    #[test]
    fn a_typed_path_offers_the_folders_under_it_and_never_a_file() {
        let tmp = tree();
        let got = complete("~/work/al", tmp.path(), &[], 10);
        assert_eq!(shown(&got), vec!["~/work/alpha", "~/work/alpha-old"]);
        assert!(got.folder.is_none());
        let alpha = &got.hits[0];
        assert_eq!(alpha.name, "alpha");
        assert_eq!(alpha.path, tmp.path().join("work/alpha").display().to_string());
        assert!(alpha.git, "a folder with .git is a repository");
        assert!(!got.hits[1].git);
    }

    #[test]
    fn a_trailing_separator_lists_the_folder_itself() {
        let tmp = tree();
        let got = complete("~/work/", tmp.path(), &[], 10);
        // A typed path is a plain listing: what is there is shown, build
        // output included, hidden folders aside.
        assert_eq!(
            shown(&got),
            vec!["~/work/alpha", "~/work/alpha-old", "~/work/beta", "~/work/node_modules"]
        );
        let itself = got.folder.as_ref().expect("~/work/ is a folder, Enter may take it");
        assert_eq!(itself.shown, "~/work");
        assert_eq!(itself.path, tmp.path().join("work").display().to_string());
    }

    #[test]
    fn a_whole_folder_typed_without_the_separator_is_a_folder_too() {
        let tmp = tree();
        let got = complete("~/work/alpha", tmp.path(), &[], 10);
        assert_eq!(got.folder.as_ref().map(|f| f.shown.as_str()), Some("~/work/alpha"));
        assert_eq!(shown(&got), vec!["~/work/alpha", "~/work/alpha-old"]);
    }

    #[test]
    fn an_absolute_path_outside_home_keeps_its_own_spelling() {
        let tmp = tree();
        let elsewhere = tempfile::tempdir().unwrap();
        fs::create_dir_all(elsewhere.path().join("svc")).unwrap();
        let typed = format!("{}/s", elsewhere.path().display());
        let got = complete(&typed, tmp.path(), &[], 10);
        assert_eq!(got.hits.len(), 1);
        assert_eq!(got.hits[0].shown, elsewhere.path().join("svc").display().to_string());
    }

    #[test]
    fn a_folder_that_does_not_exist_offers_nothing() {
        let tmp = tree();
        let got = complete("~/nowhere/x", tmp.path(), &[], 10);
        assert!(got.hits.is_empty());
        assert!(got.folder.is_none());
    }

    #[test]
    fn a_bare_name_is_found_under_the_roots_three_levels_down_at_most() {
        let tmp = tree();
        let roots = vec![tmp.path().join("work")];
        let got = complete("alpha", tmp.path(), &roots, 10);
        // The repository first, then the name that starts with the text,
        // then the one that holds it, three levels down (an app inside a
        // monorepo). Not the fourth level, not dependencies, not a hidden
        // folder.
        assert_eq!(
            shown(&got),
            vec!["~/work/alpha", "~/work/alpha-old", "~/work/beta/inner/deep-alpha"]
        );
        assert!(got.folder.is_none());
    }

    #[test]
    fn a_bare_name_reaches_the_second_level() {
        let tmp = tree();
        let roots = vec![tmp.path().join("work")];
        let got = complete("inner", tmp.path(), &roots, 10);
        assert_eq!(shown(&got), vec!["~/work/beta/inner"]);
    }

    #[test]
    fn a_separator_after_a_name_lists_the_folder_that_name_was_found_as() {
        let tmp = tree();
        let roots = vec![tmp.path().join("work")];
        // "beta" is ~/work/beta, not ~/beta: the name picked from the
        // list keeps meaning the same folder once "/" is typed after it.
        let got = complete("beta/", tmp.path(), &roots, 10);
        assert_eq!(shown(&got), vec!["~/work/beta/inner"]);
        let itself = got.folder.as_ref().expect("beta/ is a folder, Enter may take it");
        assert_eq!(itself.shown, "~/work/beta");
        assert_eq!(itself.path, tmp.path().join("work/beta").display().to_string());
        assert!(itself.git);
    }

    #[test]
    fn a_lone_dot_is_the_start_of_a_hidden_name_not_the_folder_itself() {
        let tmp = tree();
        let roots = vec![tmp.path().join("work")];
        let got = complete("~/work/.", tmp.path(), &roots, 10);
        assert_eq!(shown(&got), vec!["~/work/.hidden-alpha"]);
        assert!(got.folder.is_none());
        assert!(complete("beta/.", tmp.path(), &roots, 10).folder.is_none());
    }

    #[test]
    fn a_relative_path_goes_on_below_the_folder_found() {
        let tmp = tree();
        let roots = vec![tmp.path().join("work")];
        let got = complete("beta/inner/de", tmp.path(), &roots, 10);
        assert_eq!(shown(&got), vec!["~/work/beta/inner/deep-alpha"]);
        assert!(got.folder.is_none());
    }

    #[test]
    fn a_relative_path_that_exists_under_home_is_taken_from_home() {
        let tmp = tree();
        let got = complete("work/al", tmp.path(), &[], 10);
        assert_eq!(shown(&got), vec!["~/work/alpha", "~/work/alpha-old"]);
    }

    #[test]
    fn a_relative_path_under_a_name_nobody_has_offers_nothing() {
        let tmp = tree();
        let got = complete("nowhere/", tmp.path(), &[], 10);
        assert!(got.hits.is_empty());
        assert!(got.folder.is_none());
    }

    #[test]
    fn home_is_searched_but_its_personal_folders_are_left_alone() {
        let tmp = tree();
        let by_name = complete("alpha", tmp.path(), &[], 10);
        // ~/work/alpha is two levels under home; ~/Documents is not entered.
        assert_eq!(shown(&by_name), vec!["~/work/alpha", "~/work/alpha-old"]);
        let solo = complete("solo", tmp.path(), &[], 10);
        assert_eq!(shown(&solo), vec!["~/solo"]);
        assert!(solo.hits[0].git);
    }

    #[test]
    fn typing_into_a_personal_folder_lists_it_as_any_other() {
        let tmp = tree();
        let got = complete("~/Documents/", tmp.path(), &[], 10);
        assert_eq!(shown(&got), vec!["~/Documents/alpha-private"]);
        assert!(got.hits[0].git);
    }

    #[test]
    fn listing_home_does_not_probe_inside_personal_folders() {
        let tmp = tree();
        fs::create_dir_all(tmp.path().join("Documents/.git")).unwrap();
        let got = complete("~/", tmp.path(), &[], 10);
        let docs = got.hits.iter().find(|h| h.name == "Documents").unwrap();
        assert!(!docs.git, "no look inside a personal folder nobody asked about");
        assert!(got.hits.iter().find(|h| h.name == "solo").unwrap().git);
    }

    #[test]
    fn nothing_typed_offers_the_repositories_nearest_the_roots() {
        let tmp = tree();
        let roots = vec![tmp.path().join("work")];
        let got = complete("", tmp.path(), &roots, 3);
        assert_eq!(got.hits.len(), 3);
        assert!(got.hits.iter().all(|h| h.git), "repositories come before plain folders: {:?}", shown(&got));
    }

    #[test]
    fn the_list_is_capped() {
        let tmp = tree();
        let got = complete("~/work/", tmp.path(), &[], 2);
        assert_eq!(got.hits.len(), 2);
    }
}
